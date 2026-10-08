//! The tree-browser TUI, ported from notez-cli.
//!
//! Only the terminal layer lives here: rendering, key and mouse handling,
//! filter state, preview pane, and the help overlay. The forest is built
//! from [`SectionSpec`]s the caller assembles out of
//! `notez_core::core::aggregate` entries (see `commands::tree`), so the
//! browser shows exactly what the aggregator knows: no symlink walking.
//! Tag flags come from per-root `.tags` files via `notez_core::note_tags`;
//! on quit only roots whose tag maps actually changed are reported back,
//! and unknown keys in an existing `.tags` (entries for files outside this
//! view) are preserved rather than dropped.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result};
use crossterm::event::{
    self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEventKind,
};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Padding, Paragraph};

use notez_core::config::Config;
use notez_core::core::Scope;
use notez_core::filter::{self, Filter};
use notez_core::note_tags;
use notez_core::tags::FLAG_DEFS;
use notez_core::util::sanitize;

use super::footer::{self, Group, KeyHint, Mode, QUIT_HINT_RESERVED_COLS, Slot, Toggle};
use super::help::{self, HelpState};
use super::highlight::{self, Language};
use super::markdown;
use super::move_path;
use super::panes::{self, Pane, Panes, Press};
use super::{VimCommandMode, VimKey, theme};
use crate::commands::{add, mkdir, rename};

/// What the title bar shows.
pub struct TreeContext {
    pub title: String,
    pub path_display: String,
    /// Shown in the status bar for the whole session, whenever nothing
    /// transient is using the line.
    pub warning: Option<String>,
    /// The project the browser was opened in, if any. Section prompts name
    /// the project only when it differs from this one.
    pub current_project: Option<String>,
    /// Where new notes can go beyond the folder under the cursor: the
    /// scope roots `Tab` cycles through in the new-note prompt, and the
    /// target when the tree has no rows.
    pub new_note_roots: NewNoteRoots,
}

/// The scope roots a new note can target.
#[derive(Debug, Clone, Default)]
pub struct NewNoteRoots {
    /// The global store, `<notez_root>/`.
    pub global: PathBuf,
    /// Each known project's stores in `Tab` order: personal, public, local.
    /// A project whose repository is unknown lists only its personal root.
    pub projects: HashMap<String, Vec<(Scope, PathBuf)>>,
}

/// One top-level section of the forest: a walk root, its display label and
/// icon, and the files (from the aggregator) that live under it. `tag_root`
/// is where this section's `.tags` lives; for personal and global sections
/// that is the notez root itself so keys match the desktop app and the
/// migrated data (`personal/<name>/...`), while public/local/docs stores
/// keep their own per-store `.tags` like notez-cli did.
pub struct SectionSpec {
    pub root: PathBuf,
    pub tag_root: PathBuf,
    pub label: String,
    pub icon: &'static str,
    pub is_doc: bool,
    pub files: Vec<PathBuf>,
    /// Every directory under `root` (absolute paths, hidden names and
    /// symlinks skipped), so a folder with no notes in it still gets a row.
    pub dirs: Vec<PathBuf>,
    /// The scope a note created in this section gets.
    pub scope: Scope,
    /// The project the section belongs to; `None` for the global store.
    pub project: Option<String>,
    /// Where a new note goes when the cursor is on the section itself. Equal
    /// to `root` for note stores; a docs section is not a note store, so for
    /// it this is the project's personal root and nothing is published by
    /// accident.
    pub new_note_root: PathBuf,
    /// Whether the section belongs to the repository the browser was
    /// opened in. The global section and other projects' sections are not.
    pub is_current: bool,
}

/// A row in the flattened forest. Hierarchy is positional via `parent_idx`,
/// like the legacy browser.
#[derive(Debug, Clone)]
struct TreeNode {
    name: String,
    path: PathBuf,
    /// Path when the browser loaded, so a rename can retire the old `.tags` key.
    origin: PathBuf,
    is_dir: bool,
    depth: usize,
    expanded: bool,
    child_count: usize,
    parent_idx: Option<usize>,
    flags: u8,
    scope_icon: &'static str,
    /// Index into the dedup'd tag-root list.
    tag_root: usize,
    /// Index of the [`SectionSpec`] this row belongs to.
    section: usize,
}

/// Run the browser to completion. Returns `(root, final_map)` pairs for
/// every tag root whose `.tags` content changed; the caller persists them.
/// A quit without tag edits returns an empty list (nothing gets written).
/// `rebuild` lists the sections again; the browser calls it after creating
/// or deleting a note so the tree matches the disk.
pub fn run_tree(
    sections: Vec<SectionSpec>,
    ctx: &TreeContext,
    config: &Config,
    rebuild: &dyn Fn() -> Result<Vec<SectionSpec>>,
) -> Result<Vec<(PathBuf, HashMap<String, u8>)>> {
    let (mut nodes, tag_roots) = build_forest(&sections);
    open_current_sections(&mut nodes, &sections);
    let initial: Vec<HashMap<String, u8>> =
        tag_roots.iter().map(|r| note_tags::load_tags(r)).collect();
    apply_tags(&mut nodes, &tag_roots, &initial);
    let mut forest = Forest { sections, nodes, tag_roots, initial };
    let mut retired = Vec::new();
    let mut carried = Vec::new();

    let mut terminal = super::enter().context("failed to enter TUI")?;
    let result =
        event_loop(&mut terminal, &mut forest, &mut retired, &mut carried, ctx, config, rebuild);
    super::leave().context("failed to leave TUI")?;
    result?;

    Ok(exit_tag_maps(&forest, &retired, &carried))
}

/// What the event loop browses, kept together so a rebuild can replace it:
/// the sections, their rows, and the tag roots with their tags as on disk
/// at the start of the session.
struct Forest {
    sections: Vec<SectionSpec>,
    nodes: Vec<TreeNode>,
    /// Every tag root seen this session. A root whose last note was deleted
    /// stays listed (with no rows), so its retired keys still get written.
    tag_roots: Vec<PathBuf>,
    initial: Vec<HashMap<String, u8>>,
}

impl Forest {
    /// Swap in freshly listed `sections`, keeping the session state (see
    /// [`restore_state`]). Returns the row of `created`, if listed.
    fn rebuild(&mut self, sections: Vec<SectionSpec>, created: &Path) -> Option<usize> {
        let old = self.replace(sections);
        restore_state(&old, &mut self.nodes, created)
    }

    /// Swap in `sections` listed after `deleted` was removed, keeping the
    /// session state (see [`carry_state`]). Returns the row the cursor goes
    /// to: `deleted` itself if it is still listed (the delete failed), else
    /// the next note in its folder, else the one before it, else the nearest
    /// listed ancestor, skipping rows the filter `search` hides. A folder
    /// the delete emptied stays listed (sections list their folders), so it
    /// is that nearest ancestor.
    fn rebuild_after_delete(
        &mut self,
        sections: Vec<SectionSpec>,
        deleted: &Path,
        search: &str,
    ) -> Option<usize> {
        let candidates = cursor_candidates(&self.nodes, deleted);
        let old = self.replace(sections);
        carry_state(&old, &mut self.nodes);
        derive_dir_flags(&mut self.nodes);
        let visible = compute_visible(&self.nodes, search);
        candidates
            .iter()
            .find_map(|path| visible.iter().copied().find(|&i| self.nodes[i].path == *path))
    }

    /// Replace the sections and rows; the new rows carry no session state
    /// yet. Returns the old rows.
    fn replace(&mut self, sections: Vec<SectionSpec>) -> Vec<TreeNode> {
        let (mut nodes, mut tag_roots) = build_forest(&sections);
        let mut initial = carry_initial_tags(&self.tag_roots, &self.initial, &tag_roots);
        apply_tags(&mut nodes, &tag_roots, &initial);
        for (root, map) in self.tag_roots.iter().zip(&self.initial) {
            if !tag_roots.contains(root) {
                tag_roots.push(root.clone());
                initial.push(map.clone());
            }
        }
        self.sections = sections;
        self.tag_roots = tag_roots;
        self.initial = initial;
        std::mem::replace(&mut self.nodes, nodes)
    }
}

/// Paths the cursor may land on once `deleted` is gone, best first:
/// `deleted` itself, its following siblings, its preceding siblings
/// nearest first, then its ancestors upward.
fn cursor_candidates(nodes: &[TreeNode], deleted: &Path) -> Vec<PathBuf> {
    let Some(idx) = nodes.iter().position(|n| n.path == deleted) else {
        return vec![deleted.to_path_buf()];
    };
    let parent = nodes[idx].parent_idx;
    let siblings: Vec<usize> = (0..nodes.len()).filter(|&i| nodes[i].parent_idx == parent).collect();
    let pos = siblings.iter().position(|&i| i == idx).unwrap_or(0);
    let mut out = vec![deleted.to_path_buf()];
    out.extend(siblings[pos + 1..].iter().map(|&i| nodes[i].path.clone()));
    out.extend(siblings[..pos].iter().rev().map(|&i| nodes[i].path.clone()));
    let mut up = parent;
    while let Some(i) = up {
        out.push(nodes[i].path.clone());
        up = nodes[i].parent_idx;
    }
    out
}

// --- Forest construction ---

/// Intermediate per-directory grouping used to rebuild the tree shape from
/// the aggregator's flat file list.
#[derive(Default)]
struct DirTmp {
    dirs: BTreeMap<String, DirTmp>,
    files: Vec<String>,
}

impl DirTmp {
    fn insert(&mut self, comps: &[String]) {
        match comps {
            [file] => self.files.push(file.clone()),
            [dir, rest @ ..] => self.dirs.entry(dir.clone()).or_default().insert(rest),
            [] => {}
        }
    }

    /// Add the directory at `comps` (and every directory above it), with or
    /// without files in it.
    fn insert_dir(&mut self, comps: &[String]) {
        if let [dir, rest @ ..] = comps {
            self.dirs.entry(dir.clone()).or_default().insert_dir(rest);
        }
    }

    fn file_count(&self) -> usize {
        self.files.len() + self.dirs.values().map(DirTmp::file_count).sum::<usize>()
    }
}

/// Legacy sort rule: `NN_` numbered dirs sort before other dirs.
fn is_numbered(name: &str) -> bool {
    let b = name.as_bytes();
    b.len() >= 3 && b[0].is_ascii_digit() && b[1].is_ascii_digit() && b[2] == b'_'
}

/// Build the flattened forest: one depth-0 wrapper node per section, with a
/// folder row for every listed directory and every directory a file sits
/// in. Returns the nodes plus the dedup'd tag-root list they index into.
fn build_forest(sections: &[SectionSpec]) -> (Vec<TreeNode>, Vec<PathBuf>) {
    let mut nodes: Vec<TreeNode> = Vec::new();
    let mut tag_roots: Vec<PathBuf> = Vec::new();

    for (section_idx, spec) in sections.iter().enumerate() {
        if spec.files.is_empty() && spec.dirs.is_empty() {
            continue;
        }
        let root_idx = match tag_roots.iter().position(|r| r == &spec.tag_root) {
            Some(i) => i,
            None => {
                tag_roots.push(spec.tag_root.clone());
                tag_roots.len() - 1
            }
        };

        let mut tmp = DirTmp::default();
        for file in &spec.files {
            let Ok(rel) = file.strip_prefix(&spec.root) else {
                continue;
            };
            let comps: Vec<String> = rel
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect();
            tmp.insert(&comps);
        }
        for dir in &spec.dirs {
            let Ok(rel) = dir.strip_prefix(&spec.root) else {
                continue;
            };
            let comps: Vec<String> = rel
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect();
            tmp.insert_dir(&comps);
        }

        let wrapper_idx = nodes.len();
        nodes.push(TreeNode {
            name: spec.label.clone(),
            path: spec.root.clone(),
            origin: spec.root.clone(),
            is_dir: true,
            depth: 0,
            expanded: false,
            child_count: tmp.file_count(),
            parent_idx: None,
            flags: 0,
            scope_icon: spec.icon,
            tag_root: root_idx,
            section: section_idx,
        });
        emit_children(&tmp, &spec.root, 1, wrapper_idx, root_idx, &mut nodes);
        for node in &mut nodes[wrapper_idx..] {
            node.section = section_idx;
        }
    }

    (nodes, tag_roots)
}

/// The section rows' state when the browser opens: the sections of the
/// repository it was opened in are expanded, every other section stays
/// collapsed as built. Only the first build calls this; a rebuild after a
/// create or delete carries the user's expansion instead.
fn open_current_sections(nodes: &mut [TreeNode], sections: &[SectionSpec]) {
    for node in nodes.iter_mut().filter(|n| n.depth == 0) {
        if sections.get(node.section).is_some_and(|s| s.is_current) {
            node.expanded = true;
        }
    }
}

fn emit_children(
    tmp: &DirTmp,
    dir_path: &Path,
    depth: usize,
    parent_idx: usize,
    tag_root: usize,
    nodes: &mut Vec<TreeNode>,
) {
    let (numbered, other): (Vec<_>, Vec<_>) =
        tmp.dirs.iter().partition(|(name, _)| is_numbered(name));
    for (name, sub) in numbered.into_iter().chain(other) {
        let idx = nodes.len();
        let path = dir_path.join(name);
        nodes.push(TreeNode {
            name: name.clone(),
            path: path.clone(),
            origin: path.clone(),
            is_dir: true,
            depth,
            expanded: false,
            child_count: sub.file_count(),
            parent_idx: Some(parent_idx),
            flags: 0,
            scope_icon: "",
            tag_root,
            section: 0,
        });
        emit_children(sub, &path, depth + 1, idx, tag_root, nodes);
    }

    let mut files = tmp.files.clone();
    files.sort();
    for name in files {
        nodes.push(TreeNode {
            name: name.clone(),
            path: dir_path.join(&name),
            origin: dir_path.join(&name),
            is_dir: false,
            depth,
            expanded: false,
            child_count: 0,
            parent_idx: Some(parent_idx),
            flags: 0,
            scope_icon: "",
            tag_root,
            section: 0,
        });
    }
}

// --- Tags ---

fn rel_key(root: &Path, path: &Path) -> Option<String> {
    path.strip_prefix(root)
        .ok()
        .map(|r| r.to_string_lossy().to_string())
        .filter(|s| !s.is_empty())
}

/// Light up file nodes from their root's loaded `.tags` map.
fn apply_tags(nodes: &mut [TreeNode], tag_roots: &[PathBuf], maps: &[HashMap<String, u8>]) {
    for node in nodes.iter_mut() {
        if node.is_dir {
            continue;
        }
        if let Some(key) = rel_key(&tag_roots[node.tag_root], &node.path) {
            if let Some(&flags) = maps[node.tag_root].get(&key) {
                node.flags = flags;
            }
        }
    }
}

/// [`changed_tag_maps_retiring`] with no deleted notes.
#[cfg(test)]
fn changed_tag_maps(
    nodes: &[TreeNode],
    tag_roots: &[PathBuf],
    initial: &[HashMap<String, u8>],
) -> Vec<(PathBuf, HashMap<String, u8>)> {
    changed_tag_maps_retiring(nodes, tag_roots, initial, &[])
}

/// Final per-root tag maps: start from the loaded map (so keys this view
/// never showed survive untouched), drop the `retired` `(tag root, key)`
/// pairs of notes deleted this session, and overlay every file node's
/// current flags. Returns only the roots whose map differs from the loaded
/// one. Retired keys go first, so a note created again at a deleted path
/// keeps the flags its new row has.
fn changed_tag_maps_retiring(
    nodes: &[TreeNode],
    tag_roots: &[PathBuf],
    initial: &[HashMap<String, u8>],
    retired: &[(PathBuf, String)],
) -> Vec<(PathBuf, HashMap<String, u8>)> {
    let mut finals: Vec<HashMap<String, u8>> = initial.to_vec();
    for (root, key) in retired {
        if let Some(i) = tag_roots.iter().position(|r| r == root) {
            finals[i].remove(key);
        }
    }
    for node in nodes {
        if node.is_dir {
            continue;
        }
        let Some(key) = rel_key(&tag_roots[node.tag_root], &node.path) else {
            continue;
        };
        if node.origin != node.path {
            if let Some(old_key) = rel_key(&tag_roots[node.tag_root], &node.origin) {
                finals[node.tag_root].remove(&old_key);
            }
        }
        if node.flags == 0 {
            finals[node.tag_root].remove(&key);
        } else {
            finals[node.tag_root].insert(key, node.flags);
        }
    }
    tag_roots
        .iter()
        .zip(finals)
        .zip(initial)
        .filter(|((_, fin), init)| fin != *init)
        .map(|((root, fin), _)| (root.clone(), fin))
        .collect()
}

/// Recompute directory flags as the OR of their descendants' flags. Must
/// reassign (not OR-accumulate) so bits drop when a child loses a tag.
fn derive_dir_flags(nodes: &mut [TreeNode]) {
    let len = nodes.len();
    for i in (0..len).rev() {
        if !nodes[i].is_dir {
            continue;
        }
        let mut agg: u8 = 0;
        for j in (i + 1)..len {
            if nodes[j].depth <= nodes[i].depth {
                break;
            }
            if !nodes[j].is_dir {
                agg |= nodes[j].flags;
            }
        }
        nodes[i].flags = agg;
    }
}

// --- Visibility & filtering ---

fn get_visible_nodes(nodes: &[TreeNode]) -> Vec<usize> {
    let mut visible = Vec::new();
    for (idx, node) in nodes.iter().enumerate() {
        if node.depth == 0 {
            visible.push(idx);
            continue;
        }
        let mut ancestor_expanded = true;
        let mut check = node.parent_idx;
        while let Some(p) = check {
            if !nodes[p].expanded {
                ancestor_expanded = false;
                break;
            }
            check = nodes[p].parent_idx;
        }
        if ancestor_expanded {
            visible.push(idx);
        }
    }
    visible
}

/// Keep-mask for the filter: a node matches on name + flags, and every
/// match pulls in its ancestor chain so results keep their tree context.
fn compute_filter_keep(nodes: &[TreeNode], f: &Filter) -> Vec<bool> {
    let n = nodes.len();
    let mut keep = vec![false; n];
    for (i, node) in nodes.iter().enumerate() {
        if f.matches(&node.name, node.flags) {
            keep[i] = true;
        }
    }
    for i in 0..n {
        if !keep[i] {
            continue;
        }
        let mut cur = nodes[i].parent_idx;
        while let Some(p) = cur {
            if keep[p] {
                break;
            }
            keep[p] = true;
            cur = nodes[p].parent_idx;
        }
    }
    keep
}

/// Visible-list builder shared by the render pass and the key handler so
/// cursor positions always match the rendered rows.
fn compute_visible(nodes: &[TreeNode], search_buffer: &str) -> Vec<usize> {
    let f = filter::parse(search_buffer);
    let mut v = get_visible_nodes(nodes);
    if f.is_empty() {
        return v;
    }
    let keep = compute_filter_keep(nodes, &f);
    v.retain(|&i| keep[i]);
    v
}

fn find_top_dir(nodes: &[TreeNode], idx: usize) -> Option<usize> {
    if idx >= nodes.len() {
        return None;
    }
    if nodes[idx].depth == 0 {
        return Some(idx);
    }
    let mut cur = nodes[idx].parent_idx;
    while let Some(p) = cur {
        if nodes[p].depth == 0 {
            return Some(p);
        }
        cur = nodes[p].parent_idx;
    }
    None
}

/// The filter strip's contiguous 5-dot geometry, shared with the todoz
/// board: dot 0 sits at `area_x + 5`. List rows draw their dots elsewhere
/// (see [`mouse_x_to_row_tag`]).
fn mouse_x_to_dot(mouse_col: u16, area_x: u16) -> Option<u8> {
    let dot_start = area_x.saturating_add(5);
    let dot_end = dot_start + 4;
    if mouse_col >= dot_start && mouse_col <= dot_end {
        Some((mouse_col - dot_start) as u8)
    } else {
        None
    }
}

/// The tags set in `flags`, as indices into [`FLAG_DEFS`], in the order a
/// row draws their dots.
fn set_tags(flags: u8) -> impl Iterator<Item = usize> {
    FLAG_DEFS.iter().enumerate().filter(move |(_, def)| flags & def.bit != 0).map(|(i, _)| i)
}

/// How many columns the tag field of the rows `visible` takes: the most
/// tags set on any of them, 0 when none has a tag.
fn tag_field_width(nodes: &[TreeNode], visible: &[usize]) -> usize {
    visible.iter().map(|&i| set_tags(nodes[i].flags).count()).max().unwrap_or(0)
}

/// A list row's tag field: the dots of the tags set in `flags`,
/// left-aligned in their colours, padded with spaces to `width`, then one
/// space before the tree. Nothing at all when `width` is 0.
fn tag_field(flags: u8, width: usize) -> Vec<Span<'static>> {
    if width == 0 {
        return Vec::new();
    }
    let mut spans: Vec<Span<'static>> = set_tags(flags)
        .map(|i| Span::styled("●", Style::default().fg(theme::FLAG_COLORS[i])))
        .collect();
    let padding = width.saturating_sub(spans.len()) + 1;
    spans.push(Span::raw(" ".repeat(padding)));
    spans
}

/// The tag whose dot a list row with `flags` draws at `mouse_col`, as an
/// index into [`FLAG_DEFS`]: the n-th dot after the one-column gutter at
/// `area_x` is the row's n-th set tag. `None` off the dots.
fn mouse_x_to_row_tag(mouse_col: u16, area_x: u16, flags: u8) -> Option<usize> {
    let first = area_x.checked_add(1)?;
    let n = mouse_col.checked_sub(first)?;
    set_tags(flags).nth(usize::from(n))
}

/// For each row, whether a later row of `visible` (rows in tree order)
/// shares its parent: the row draws `├─` and its descendants a bar at its
/// level. Rows outside `visible`, such as rows the filter hides, neither
/// get one nor count as a later sibling. Indexed like `nodes`.
fn later_siblings(nodes: &[TreeNode], visible: &[usize]) -> Vec<bool> {
    let mut later = vec![false; nodes.len()];
    let mut seen: HashSet<Option<usize>> = HashSet::new();
    for &i in visible.iter().rev() {
        later[i] = !seen.insert(nodes[i].parent_idx);
    }
    later
}

/// The tree drawing before row `idx`'s badge, from [`theme::TREE_GLYPHS`]. A
/// section row shows its expand mark. A nested row shows, for each ancestor
/// below the section, a bar if that ancestor has a later sibling and a
/// blank otherwise, then its own branch (`├─`, or `└─` when it is the last
/// child), then a folder's expand mark or a file's blank. `later` comes
/// from [`later_siblings`].
fn branch_prefix(nodes: &[TreeNode], idx: usize, later: &[bool]) -> String {
    let g = &theme::TREE_GLYPHS;
    let node = &nodes[idx];
    if node.depth == 0 {
        return if !node.is_dir {
            "  ".to_string()
        } else if node.expanded {
            g.section_open.to_string()
        } else {
            g.section_closed.to_string()
        };
    }
    let mut levels = Vec::new();
    let mut up = node.parent_idx;
    while let Some(p) = up.filter(|&p| nodes[p].depth > 0) {
        levels.push(if later[p] { g.ancestor_bar } else { g.ancestor_blank });
        up = nodes[p].parent_idx;
    }
    levels.reverse();
    let mut prefix = levels.concat();
    prefix.push_str(if later[idx] { g.branch } else { g.last_branch });
    prefix.push_str(match (node.is_dir, node.expanded) {
        (true, true) => g.folder_open,
        (true, false) => g.folder_closed,
        (false, _) => g.file,
    });
    prefix
}

/// Every row of `visible` drawn for a list `inner_width` columns wide:
/// branch lines and the tag field measured over these rows, marked rows
/// drawn as marked.
fn list_lines(
    nodes: &[TreeNode],
    sections: &[SectionSpec],
    visible: &[usize],
    marks: &HashSet<PathBuf>,
    inner_width: usize,
) -> Vec<Line<'static>> {
    let later = later_siblings(nodes, visible);
    let dot_width = tag_field_width(nodes, visible);
    visible
        .iter()
        .map(|&idx| {
            let node = &nodes[idx];
            let branch = branch_prefix(nodes, idx, &later);
            let line = row_line(node, sections.get(node.section), &branch, dot_width, inner_width);
            if is_marked(marks, node) { mark_row(line) } else { line }
        })
        .collect()
}

/// The 5 fixed tag-dot slots with leading space, as the preview's title
/// shows them.
fn flags_slots(flags: u8) -> Vec<Span<'static>> {
    let mut spans: Vec<Span<'static>> = vec![Span::raw(" ")];
    for (i, def) in FLAG_DEFS.iter().enumerate() {
        if flags & def.bit != 0 {
            spans.push(Span::styled(
                "●",
                Style::default().fg(theme::FLAG_COLORS[i]),
            ));
        } else {
            spans.push(Span::styled(
                "·",
                Style::default().fg(Color::Rgb(50, 50, 65)),
            ));
        }
    }
    spans.push(Span::raw(" "));
    spans
}

/// The colour of a section's badge and scope icon. A docs
/// section is published with the repository, so it takes the public colour.
fn section_color(spec: &SectionSpec) -> Color {
    theme::scope_color(if spec.is_doc { Scope::Public } else { spec.scope })
}

/// The two-column badge directly before a nested row's name, after its
/// indentation and branch glyph: the section's icon in its colour and a
/// space, like a section header's `icon label`, on every file and folder
/// row; blanks on a section header (which shows the icon next to its
/// label). A todo row (the board's store, a row under it, or a file named
/// exactly `TODO.md`) shows [`theme::ICON_TODO`] instead, in the same
/// colour and width. Render only: the filter, preview, mouse hit testing
/// and tag keys never see it, and the tag dots keep their columns.
fn row_badge(node: &TreeNode, spec: Option<&SectionSpec>) -> Span<'static> {
    match spec {
        Some(spec) if node.depth > 0 && !spec.icon.is_empty() => {
            let is_todo_file = !node.is_dir && node.path.file_name().is_some_and(|name| name == "TODO.md");
            let icon =
                if is_todo_file || in_section_todo_store(node, spec) { theme::ICON_TODO } else { spec.icon };
            Span::styled(format!("{icon} "), Style::default().fg(section_color(spec)))
        }
        _ => Span::raw("  "),
    }
}

/// The columns a list row may fill in a list pane `pane_width` wide: the
/// pane less its two borders and its one-column padding on each side. The
/// list draws no highlight symbol, so a row starts at the padding.
fn list_text_width(pane_width: u16) -> usize {
    pane_width.saturating_sub(4) as usize
}

/// The list pane's frame without its title and border colour: rounded
/// borders and the padding that `list_chunks` measures inside.
fn list_block() -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .padding(Padding::new(1, 1, 1, 0))
}

/// The list pane's inside, top to bottom: the filter strip, the separator
/// and the rows. The mouse hit tests use the same rects the draw used.
fn list_chunks(list_area: Rect) -> std::rc::Rc<[Rect]> {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1), Constraint::Min(1)])
        .split(list_block().inner(list_area))
}

/// One list row: a one-column gutter (blank, or the mark of a marked row),
/// the tag field `dot_width` columns wide (see [`tag_field`]), the `branch`
/// drawing from [`branch_prefix`], the scope badge on a nested row, the
/// name, and on a directory row a dotted leader to its file count, which
/// ends at `inner_width` in display columns. A section header also shows
/// its scope icon in the scope colour before the label; the icon and colour
/// name the scope, so no scope word follows. `spec` is the row's section
/// and `inner_width` the list pane's text width.
fn row_line(
    node: &TreeNode,
    spec: Option<&SectionSpec>,
    branch: &str,
    dot_width: usize,
    inner_width: usize,
) -> Line<'static> {
    let header_color = spec.map_or(theme::OVERLAY, section_color);
    let mut spans = vec![Span::raw(" ")];
    spans.extend(tag_field(node.flags, dot_width));
    spans.push(Span::styled(branch.to_string(), Style::default().fg(theme::SURFACE)));
    if node.depth > 0 {
        spans.push(row_badge(node, spec));
    }
    if !node.scope_icon.is_empty() {
        spans.push(Span::styled(
            format!("{} ", node.scope_icon),
            Style::default().fg(header_color),
        ));
    }
    if node.is_dir {
        spans.push(Span::styled(node.name.clone(), Style::default().fg(theme::SAPPHIRE)));
        if node.child_count > 0 {
            let count_str = format!("{}", node.child_count);
            let prefix_len: usize = spans.iter().map(Span::width).sum();
            let avail = inner_width.saturating_sub(prefix_len + count_str.len() + 2);
            if avail > 3 {
                spans.push(Span::styled(
                    format!(" {} ", "·".repeat(avail)),
                    Style::default().fg(theme::SURFACE),
                ));
            } else {
                spans.push(Span::raw(" "));
            }
            spans.push(Span::styled(count_str, Style::default().fg(theme::OVERLAY)));
        }
    } else {
        spans.push(Span::styled(node.name.clone(), Style::default().fg(theme::TEXT)));
    }
    Line::from(spans)
}

/// What the status bar shows, highest priority first.
#[derive(Debug, PartialEq, Eq)]
enum StatusSlot<'a> {
    Rename(&'a str),
    Message(&'a str),
    VimCommand,
    Tags,
    Warning(&'a str),
    Hints,
}

/// Pick the status bar's content. The session warning ranks below every
/// transient use of the line, so it comes back once they end, and it is kept
/// apart from the one-off `message` that each key press clears.
fn status_slot<'a>(
    rename: Option<&'a str>,
    message: Option<&'a str>,
    vim_active: bool,
    flag_mode: bool,
    warning: Option<&'a str>,
) -> StatusSlot<'a> {
    // A message outranks the rename prompt: a refused name keeps the prompt
    // open, and its message shows until the next key brings the prompt back.
    if let Some(message) = message {
        StatusSlot::Message(message)
    } else if let Some(buffer) = rename {
        StatusSlot::Rename(buffer)
    } else if vim_active {
        StatusSlot::VimCommand
    } else if flag_mode {
        StatusSlot::Tags
    } else if let Some(warning) = warning {
        StatusSlot::Warning(warning)
    } else {
        StatusSlot::Hints
    }
}

/// The rename prompt that leads the footer in rename mode.
fn rename_lead(buffer: &str) -> Vec<Span<'static>> {
    vec![
        Span::styled(" rename: ", Style::default().fg(theme::MAUVE)),
        Span::styled(buffer.to_string(), Style::default().fg(theme::TEXT)),
        Span::styled("_", Style::default().fg(theme::OVERLAY)),
    ]
}

/// The `:` command buffer that leads the footer in command mode.
fn command_lead(buffer: &str) -> Vec<Span<'static>> {
    vec![Span::styled(buffer.to_string(), theme::command_line())]
}

/// The tag legend that leads the footer in tag mode; the tags set in `flags`
/// are coloured.
fn tag_legend(flags: u8) -> Vec<Span<'static>> {
    let mut spans = vec![Span::styled(" tags: ", Style::default().fg(theme::MAUVE))];
    for (idx, def) in FLAG_DEFS.iter().enumerate() {
        let active = flags & def.bit != 0;
        let color = theme::FLAG_COLORS[idx];
        spans.push(Span::styled(format!("{}", idx + 1), Style::default().fg(color)));
        spans.push(Span::styled(":", Style::default().fg(theme::OVERLAY)));
        spans.push(Span::styled(
            format!("{} ", def.label),
            Style::default().fg(if active { color } else { theme::OVERLAY }),
        ));
        spans.push(Span::raw(" "));
    }
    spans
}

/// `lead` first, then the `TREE_KEYS` hints for `mode` that fit after it.
fn lead_with_hints(
    lead: Vec<Span<'static>>,
    mode: Mode,
    on: &[Toggle],
    width: usize,
) -> Line<'static> {
    footer::status_line(TREE_KEYS, lead, true, mode, on, Vec::new(), width)
}

// --- New note ---

/// Where a note created from the browser goes: the directory, the scope the
/// note gets (it decides the scratch gitignore step) and the label the
/// prompt shows, so the target is named before anything is created.
#[derive(Debug, Clone, PartialEq, Eq)]
struct NewNoteTarget {
    dir: PathBuf,
    scope: Scope,
    label: String,
}

/// The open new-note prompt. It owns its target, so switching the scope
/// replaces `target` and leaves the typed title alone. `origin` is the
/// target the prompt opened on, which `Tab` returns to after a full cycle,
/// and `project` the project whose scopes `Tab` cycles through.
/// `is_folder` marks the same prompt opened by `N`, which creates a folder
/// named by the buffer instead of a note.
struct NewNotePrompt {
    target: NewNoteTarget,
    origin: NewNoteTarget,
    project: Option<String>,
    buffer: String,
    is_folder: bool,
}

impl NewNotePrompt {
    fn open(target: NewNoteTarget, project: Option<String>) -> Self {
        Self { origin: target.clone(), target, project, buffer: String::new(), is_folder: false }
    }
}

/// The target of `n` when the tree has no rows: the current project's
/// personal root inside a project, the global root outside one. Personal,
/// not public, so nothing is published by accident.
fn empty_tree_target(roots: &NewNoteRoots, current_project: Option<&str>) -> NewNoteTarget {
    let Some(project) = current_project else {
        return NewNoteTarget {
            dir: roots.global.clone(),
            scope: Scope::Global,
            label: scope_label(Scope::Global, None, None),
        };
    };
    let dir = roots
        .projects
        .get(project)
        .and_then(|stores| stores.iter().find(|(s, _)| *s == Scope::Personal))
        .map(|(_, dir)| dir.clone())
        .unwrap_or_else(|| roots.global.join("personal").join(project));
    NewNoteTarget {
        dir,
        scope: Scope::Personal,
        label: scope_label(Scope::Personal, Some(project), current_project),
    }
}

/// The list pane's text when the view has no notes at all.
const EMPTY_STATE: &str = "no notes here yet: n creates one";

/// The empty-state line, shown in place of the list when there are no rows.
fn empty_state_line(nodes: &[TreeNode]) -> Option<&'static str> {
    nodes.is_empty().then_some(EMPTY_STATE)
}

/// The empty-state text for the view titled `title`. A narrowed view
/// (`-p`, `-l`, `-g`) is titled `<scope icon> notez (...)` by
/// `commands::tree`, so its scope is read from that icon and named:
/// `no scratch notes here yet: n creates one`. The all view's title starts
/// with `notez` and keeps [`EMPTY_STATE`].
fn empty_state_text(title: &str) -> String {
    let narrowed = [Scope::Personal, Scope::Public, Scope::Local, Scope::Global]
        .into_iter()
        .find(|scope| title.starts_with(scope.icon()));
    // `Scope::label()` calls the global scope `notez`; "global" reads better here.
    let word = match narrowed {
        Some(Scope::Global) => "global",
        Some(scope) => scope.label(),
        None => return EMPTY_STATE.to_string(),
    };
    format!("no {word} notes here yet: n creates one")
}

/// The prompt `n` opens for the row at `row`: the folder under the cursor
/// in that row's section, or [`empty_tree_target`] when no row is under
/// the cursor (an empty tree, or a filter that hides every row).
fn open_new_note_prompt(
    nodes: &[TreeNode],
    sections: &[SectionSpec],
    row: Option<usize>,
    ctx: &TreeContext,
) -> NewNotePrompt {
    let current = ctx.current_project.as_deref();
    let at_row = row.and_then(|i| {
        let target = new_note_target(nodes, sections, i, current)?;
        Some((target, sections[nodes[i].section].project.clone()))
    });
    match at_row {
        Some((target, project)) => NewNotePrompt::open(target, project),
        None => NewNotePrompt::open(
            empty_tree_target(&ctx.new_note_roots, current),
            ctx.current_project.clone(),
        ),
    }
}

/// The target after one `Tab` in the prompt. The scopes cycle in the order
/// personal, public, local, global, limited to the ones that apply: the
/// project's known stores plus global, or global alone for a row outside
/// any project. Each step targets the scope's root; arriving back at the
/// origin's scope restores the origin itself, folder included.
fn next_scope_target(
    current: &NewNoteTarget,
    origin: &NewNoteTarget,
    project: Option<&str>,
    roots: &NewNoteRoots,
    current_project: Option<&str>,
) -> NewNoteTarget {
    let mut cycle: Vec<(Scope, PathBuf)> = project
        .and_then(|p| roots.projects.get(p))
        .cloned()
        .unwrap_or_default();
    cycle.push((Scope::Global, roots.global.clone()));
    if !cycle.iter().any(|(s, _)| *s == origin.scope) {
        cycle.insert(0, (origin.scope, origin.dir.clone()));
    }
    let pos = cycle.iter().position(|(s, _)| *s == current.scope).unwrap_or(0);
    let (scope, dir) = cycle[(pos + 1) % cycle.len()].clone();
    if scope == origin.scope {
        return origin.clone();
    }
    NewNoteTarget { label: scope_label(scope, project, current_project), dir, scope }
}

/// Human name of a scope in the prompt. Public says what it means: those
/// notes are committed with the project repository. A project other than
/// `current_project` is named, so the global view is unambiguous.
fn scope_label(scope: Scope, project: Option<&str>, current_project: Option<&str>) -> String {
    let other = project.filter(|p| Some(*p) != current_project);
    match (scope, other) {
        (Scope::Public, None) => "public (committed with the project)".to_string(),
        (Scope::Public, Some(p)) => format!("public (committed with {p})"),
        (Scope::Personal, None) => "personal".to_string(),
        (Scope::Personal, Some(p)) => format!("personal ({p})"),
        (Scope::Local, None) => "local scratch".to_string(),
        (Scope::Local, Some(p)) => format!("local scratch ({p})"),
        (Scope::Global, _) => "global".to_string(),
    }
}

/// Resolve the row at `idx` to the new note's target: a directory row is the
/// target itself, a file row its parent directory, a section row its root.
/// In a docs section the target is the project's personal root instead.
/// `None` when there is no such row.
fn new_note_target(
    nodes: &[TreeNode],
    sections: &[SectionSpec],
    idx: usize,
    current_project: Option<&str>,
) -> Option<NewNoteTarget> {
    let node = nodes.get(idx)?;
    let spec = sections.get(node.section)?;
    let project = spec.project.as_deref();
    if spec.is_doc {
        return Some(NewNoteTarget {
            dir: spec.new_note_root.clone(),
            scope: Scope::Personal,
            label: scope_label(Scope::Personal, project, current_project),
        });
    }
    let dir = if node.is_dir {
        node.path.clone()
    } else {
        node.path.parent().unwrap_or(&spec.root).to_path_buf()
    };
    let mut label = scope_label(spec.scope, project, current_project);
    if let Ok(rel) = dir.strip_prefix(&spec.root) {
        let rel = rel.to_string_lossy();
        if !rel.is_empty() {
            label = format!("{label}/{rel}");
        }
    }
    Some(NewNoteTarget { dir, scope: spec.scope, label })
}

// --- Typed names ---
//
// The prompts `n`, `N` and `r` refuse a typed name that `sanitize::name`
// would drop characters from (`_`, `.`), with what it would have become,
// instead of changing it without a word. Lowercasing and blanks to `-` stay
// silent (NZ-26). The CLI commands keep sanitizing.

/// The soft form of a typed name: `sanitize::name`'s steps before its
/// character filter (trim, lowercase, whitespace runs to `-`), in its order,
/// so filtering this form gives exactly `sanitize::name(input)`.
fn soft_name(input: &str) -> String {
    input.trim().to_lowercase().split_whitespace().collect::<Vec<_>>().join("-")
}

/// What `sanitize::name` makes of `input` when that drops characters from
/// its [`soft_name`] form; `None` when the name is taken (lowercasing and
/// blanks to `-` are silent). A name that sanitizes to nothing is `None`
/// too: the prompts' empty-name paths answer it.
fn name_would_change(input: &str) -> Option<String> {
    let cleaned = sanitize::name(input);
    (!cleaned.is_empty() && cleaned != soft_name(input)).then_some(cleaned)
}

/// The footer message for a typed name [`name_would_change`] refuses.
fn altered_name_message(cleaned: &str) -> String {
    format!("name would become {cleaned}; use letters, digits and -")
}

/// The footer message for a new-note title that sanitizes to nothing. An
/// empty title still makes an `untitled` note.
const NOTE_NAME_EMPTY: &str = "new note: the name is empty";

/// Why `Enter` in the `n` or `N` prompt creates nothing and leaves the
/// prompt open with the typed name: an altered name (see
/// [`name_would_change`]), or a note title that is not empty but sanitizes
/// to nothing. `None` goes on to create.
fn new_item_refusal(prompt: &NewNotePrompt) -> Option<String> {
    let buffer = prompt.buffer.as_str();
    if !prompt.is_folder && !buffer.trim().is_empty() && sanitize::name(buffer).is_empty() {
        return Some(NOTE_NAME_EMPTY.to_string());
    }
    name_would_change(buffer).map(|cleaned| altered_name_message(&cleaned))
}

/// The new-note prompt that leads the footer while a title is typed.
fn new_note_lead(label: &str, buffer: &str) -> Vec<Span<'static>> {
    vec![
        Span::styled(format!(" new note in {label}: "), Style::default().fg(theme::MAUVE)),
        Span::styled(buffer.to_string(), Style::default().fg(theme::TEXT)),
        Span::styled("_", Style::default().fg(theme::OVERLAY)),
    ]
}

// --- New folder ---

/// The footer message for `N` in a docs section: those are the repository's
/// own files, and `n` there writes to the personal store instead.
const FOLDER_IN_DOCS: &str = "new folder: not in a docs section";

/// The footer message for a folder name that sanitizes to nothing.
const FOLDER_NAME_EMPTY: &str = "new folder: the name is empty";

/// The prompt `N` opens for the row at `row`: the new-note prompt on the
/// same target (see [`open_new_note_prompt`]), creating a folder instead.
/// `Err` is the footer message for a row in a docs section.
fn open_new_folder_prompt(
    nodes: &[TreeNode],
    sections: &[SectionSpec],
    row: Option<usize>,
    ctx: &TreeContext,
) -> std::result::Result<NewNotePrompt, &'static str> {
    let spec = row.and_then(|i| nodes.get(i)).and_then(|n| sections.get(n.section));
    if spec.is_some_and(|s| s.is_doc) {
        return Err(FOLDER_IN_DOCS);
    }
    let mut prompt = open_new_note_prompt(nodes, sections, row, ctx);
    prompt.is_folder = true;
    Ok(prompt)
}

/// The new-folder prompt that leads the footer while a name is typed.
fn new_folder_lead(label: &str, buffer: &str) -> Vec<Span<'static>> {
    vec![
        Span::styled(format!(" new folder in {label}: "), Style::default().fg(theme::MAUVE)),
        Span::styled(buffer.to_string(), Style::default().fg(theme::TEXT)),
        Span::styled("_", Style::default().fg(theme::OVERLAY)),
    ]
}

/// What a confirmed new folder leaves: the row for the cursor, if the
/// folder is listed, and the footer message, if any.
#[derive(Debug)]
struct FolderOutcome {
    row: Option<usize>,
    message: Option<String>,
    /// Whether the forest was listed again from disk; only then may the
    /// file-change probe take a fresh reading ([`refresh_probe`]).
    relisted: bool,
}

/// Create the folder `name` in `target` through [`mkdir::create_in_dir`],
/// the path `notez mkdir` takes. A name that sanitizes to nothing, or to
/// the name of anything already in the target (file or folder; a
/// case-insensitive file system matches regardless of case), is refused
/// before anything is created, so nothing is merged or overwritten. On
/// success the forest is rebuilt from `rebuild` and the new folder's row,
/// expanded with its ancestors, is returned.
fn create_folder(
    forest: &mut Forest,
    target: &NewNoteTarget,
    name: &str,
    rebuild: &dyn Fn() -> Result<Vec<SectionSpec>>,
) -> FolderOutcome {
    let refuse = |message: String| FolderOutcome { row: None, message: Some(message), relisted: false };
    let cleaned = sanitize::name(name);
    if cleaned.is_empty() {
        return refuse(FOLDER_NAME_EMPTY.to_string());
    }
    if std::fs::symlink_metadata(target.dir.join(&cleaned)).is_ok() {
        return refuse(format!("new folder: {cleaned} already exists in {}", target.label));
    }
    let path = match mkdir::create_in_dir(&target.dir, name, target.scope) {
        Ok(path) => path,
        Err(e) => return refuse(format!("new folder failed: {e:#}")),
    };
    let sections = match rebuild() {
        Ok(sections) => sections,
        Err(e) => {
            return refuse(format!(
                "created {}, but the list could not be refreshed: {e:#}",
                path.display()
            ));
        }
    };
    match forest.rebuild(sections, &path) {
        Some(row) => FolderOutcome { row: Some(row), message: None, relisted: true },
        None => FolderOutcome { row: None, message: Some(format!("created {}", path.display())), relisted: true },
    }
}

// --- The todo board's store ---
//
// `<notez root>/_todos` holds the todo board's lists (`notez_core::todo`
// reads it). The browser lists it in the global section but leaves it to
// the todo view: `d`, `r`, `m` and `S` refuse it and every row under it,
// and it is never a move destination. `n` and `N` there work as anywhere.

/// The store's folder name under the notez root.
const TODO_STORE: &str = "_todos";

/// What the refusals below say after their verb.
const TODO_STORE_MANAGED: &str = "the todo board's store is managed by the todo view";

/// The footer message for `d` on the store or a row under it.
const TODOS_DELETE: &str = "delete: the todo board's store is managed by the todo view";

/// The footer message for `r` on the store or a row under it.
const TODOS_RENAME: &str = "rename: the todo board's store is managed by the todo view";

/// The footer message for `m` on the store or a row under it.
const TODOS_MOVE: &str = "move: the todo board's store is managed by the todo view";

/// The footer message for `S` on the store or a row under it.
const TODOS_SET_SCOPE: &str = "set scope: the todo board's store is managed by the todo view";

/// Whether `path` is `<notez_root>/_todos` or lies under it. The first step
/// below the root counts as the store when it is spelled `_todos` or is the
/// same directory entry the board reads as `_todos` (another spelling on a
/// case-insensitive file system). A sibling such as `_todos-archive` is not
/// the store.
fn in_todo_store(path: &Path, notez_root: &Path) -> bool {
    let Ok(rel) = path.strip_prefix(notez_root) else {
        return false;
    };
    let Some(first) = rel.components().next() else {
        return false;
    };
    let top = notez_root.join(first);
    let store = notez_root.join(TODO_STORE);
    top == store || is_same_entry(&top, &store)
}

/// Whether `node` is the todo board's store or a row under it. Only the
/// global section's root is the notez root (`commands::tree` gives the
/// global section `notez_root` itself), so a `_todos` folder in a project's
/// store is an ordinary folder.
fn is_todo_row(node: &TreeNode, sections: &[SectionSpec]) -> bool {
    sections.get(node.section).is_some_and(|spec| in_section_todo_store(node, spec))
}

/// Whether `node`, a row of `spec`, is the todo board's store or a row
/// under it; see [`is_todo_row`].
fn in_section_todo_store(node: &TreeNode, spec: &SectionSpec) -> bool {
    spec.scope == Scope::Global && spec.project.is_none() && !spec.is_doc && in_todo_store(&node.path, &spec.root)
}

// --- Delete ---

/// The footer message for `d` on a section row.
const SECTION_DELETE: &str = "delete: a section cannot be deleted";

/// The footer message for `d` on a folder in a docs section: those are the
/// repository's own files, which the browser does not remove wholesale.
const DOCS_FOLDER_DELETE: &str = "delete: not for folders in a docs section";

/// The footer message for `d` on a folder that holds another section.
const FOLDER_HOLDS_SECTION: &str = "delete: this folder holds another section";

/// The open delete confirmation for one note or folder: its path, its path
/// when the session started (a rename moves `path` only), the tag root its
/// `.tags` keys live under, its path relative to its section's root, and
/// the scope it is deleted from with that scope's label. For a folder,
/// `folder` holds what the question counts, and `section_root` with
/// `section_roots` is what [`remove_folder`] checks before removing.
#[derive(Debug, Clone)]
struct DeletePrompt {
    path: PathBuf,
    origin: PathBuf,
    tag_root: PathBuf,
    rel: String,
    scope: Scope,
    label: String,
    folder: Option<FolderContents>,
    section_root: PathBuf,
    /// The root of every section in the view.
    section_roots: Vec<PathBuf>,
}

/// What a folder holds, for the delete question: the markdown notes the
/// tree lists (regular `.md` files, not hidden, at any depth) and whether
/// anything else would go with them (other files, hidden entries,
/// symlinks). Folders themselves are not counted.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct FolderContents {
    notes: usize,
    has_other_files: bool,
}

/// Count what `dir` holds (see [`FolderContents`]). Symlinks are not
/// followed, like the aggregator's walk; an unreadable directory adds
/// nothing.
fn folder_contents(dir: &Path) -> FolderContents {
    let mut contents = FolderContents::default();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return contents;
    };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            contents.has_other_files = true;
            continue;
        };
        let path = entry.path();
        let hidden = entry.file_name().to_string_lossy().starts_with('.');
        let is_note = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("md"));
        if hidden {
            contents.has_other_files = true;
        } else if kind.is_dir() {
            let below = folder_contents(&path);
            contents.notes += below.notes;
            contents.has_other_files |= below.has_other_files;
        } else if kind.is_file() && is_note {
            contents.notes += 1;
        } else {
            contents.has_other_files = true;
        }
    }
    contents
}

/// Whether a folder delete or rename may touch `path`: it lies strictly
/// inside `section_root` through plain names only (no `.` or `..` step),
/// and it is not, and does not hold, the root of any section in
/// `section_roots`. This keeps a section root and everything outside the
/// row's section out of reach.
fn folder_change_allowed(path: &Path, section_root: &Path, section_roots: &[PathBuf]) -> bool {
    let Ok(rel) = path.strip_prefix(section_root) else {
        return false;
    };
    let mut steps = rel.components().peekable();
    if steps.peek().is_none() || !steps.all(|c| matches!(c, std::path::Component::Normal(_))) {
        return false;
    }
    !section_roots.iter().any(|root| root.starts_with(path))
}

/// Remove the folder at `path` with everything in it, after
/// [`folder_change_allowed`] passes; otherwise nothing is touched. The only
/// place the browser calls `remove_dir_all`.
fn remove_folder(path: &Path, section_root: &Path, section_roots: &[PathBuf]) -> std::io::Result<()> {
    if !folder_change_allowed(path, section_root, section_roots) {
        return Err(std::io::Error::other(format!(
            "{} is not a folder inside its section",
            path.display()
        )));
    }
    std::fs::remove_dir_all(path)
}

/// What `d` does with the cursor on `row`: `Ok(Some)` opens the prompt on a
/// note or a folder, `Err` is the footer message for a section row, a row
/// in the todo board's store ([`is_todo_row`]), a docs folder, or a folder
/// holding another section, and `Ok(None)` (no row
/// under the cursor) does nothing. A folder's contents are counted from
/// the disk when the prompt opens.
fn delete_request(
    nodes: &[TreeNode],
    sections: &[SectionSpec],
    row: Option<usize>,
    current_project: Option<&str>,
) -> std::result::Result<Option<DeletePrompt>, &'static str> {
    let Some(node) = row.and_then(|i| nodes.get(i)) else {
        return Ok(None);
    };
    if node.depth == 0 {
        return Err(SECTION_DELETE);
    }
    let Some(spec) = sections.get(node.section) else {
        return Ok(None);
    };
    if is_todo_row(node, sections) {
        return Err(TODOS_DELETE);
    }
    let section_roots: Vec<PathBuf> = sections.iter().map(|s| s.root.clone()).collect();
    let folder = if node.is_dir {
        if spec.is_doc {
            return Err(DOCS_FOLDER_DELETE);
        }
        if !folder_change_allowed(&node.path, &spec.root, &section_roots) {
            return Err(FOLDER_HOLDS_SECTION);
        }
        Some(folder_contents(&node.path))
    } else {
        None
    };
    let rel = node.path.strip_prefix(&spec.root).unwrap_or(&node.path);
    Ok(Some(DeletePrompt {
        path: node.path.clone(),
        origin: node.origin.clone(),
        tag_root: spec.tag_root.clone(),
        rel: rel.to_string_lossy().into_owned(),
        scope: spec.scope,
        label: scope_label(spec.scope, spec.project.as_deref(), current_project),
        folder,
        section_root: spec.root.clone(),
        section_roots,
    }))
}

/// The confirmation question. For a folder it names what goes with it: `delete
/// ideas/ and its 2 notes from personal? y/n` (`1 note`, `no notes`), with
/// `and other files` when it holds anything but notes. Local scratch is not
/// in any repository, so the question says a delete there cannot be undone.
fn delete_question(prompt: &DeletePrompt) -> String {
    let warning = if prompt.scope == Scope::Local { " (not recoverable)" } else { "" };
    let what = match prompt.folder {
        None => prompt.rel.clone(),
        Some(contents) => {
            let notes = match contents.notes {
                0 => " (no notes)".to_string(),
                1 => " and its 1 note".to_string(),
                n => format!(" and its {n} notes"),
            };
            let other = if contents.has_other_files { " and other files" } else { "" };
            format!("{}/{notes}{other}", prompt.rel)
        }
    };
    format!("delete {what} from {}?{warning} y/n", prompt.label)
}

/// The delete confirmation that leads the footer while it is open.
fn delete_lead(prompt: &DeletePrompt) -> Vec<Span<'static>> {
    vec![Span::styled(format!(" {}", delete_question(prompt)), Style::default().fg(theme::PEACH))]
}

/// What a confirmed delete leaves: the row for the cursor and the footer
/// message.
#[derive(Debug)]
struct DeleteOutcome {
    row: Option<usize>,
    message: String,
    /// Whether `rebuild` listed the forest again (not the fallback).
    relisted: bool,
}

/// Answer the open delete prompt with `key`. Anything but `y` cancels and
/// returns `None` without touching the disk or the forest. On `y` the note
/// is removed (or the folder, through [`remove_folder`]), the `.tags` keys
/// of what went are added to `retired` (see [`retire_folder_keys`] for a
/// folder; a note's current and original path), and the forest is rebuilt
/// from `rebuild` whether or not the removal worked, so the tree matches
/// the disk. If `rebuild` fails, the current rows minus what went are used
/// instead.
fn answer_delete(
    key: KeyCode,
    forest: &mut Forest,
    retired: &mut Vec<(PathBuf, String)>,
    prompt: &DeletePrompt,
    search: &str,
    rebuild: &dyn Fn() -> Result<Vec<SectionSpec>>,
) -> Option<DeleteOutcome> {
    if key != KeyCode::Char('y') {
        return None;
    }
    let is_folder = prompt.folder.is_some();
    let removed = remove_and_retire(forest, retired, prompt);
    let mut message = match &removed {
        Ok(()) if is_folder => format!("deleted {}/", prompt.rel),
        Ok(()) => format!("deleted {}", prompt.rel),
        Err(e) => format!("delete failed: {e}"),
    };
    let mut relisted = true;
    let sections = rebuild().unwrap_or_else(|e| {
        relisted = false;
        message = format!("{message}, but the list could not be refreshed: {e:#}");
        if is_folder {
            let gone = |p: &Path| p.starts_with(&prompt.path) && std::fs::symlink_metadata(p).is_err();
            current_sections_without(forest, &gone)
        } else {
            let gone = |p: &Path| removed.is_ok() && p == prompt.path;
            current_sections_without(forest, &gone)
        }
    });
    let row = forest.rebuild_after_delete(sections, &prompt.path, search);
    Some(DeleteOutcome { row, message, relisted })
}

/// Remove what `prompt` names, a note or (through [`remove_folder`]) a
/// folder, and add the `.tags` keys of what went to `retired`: a note's
/// current and original key once it is gone, a folder's keys through
/// [`retire_folder_keys`] whenever its removal was allowed to start. The
/// forest is not rebuilt; the caller does that once.
fn remove_and_retire(
    forest: &Forest,
    retired: &mut Vec<(PathBuf, String)>,
    prompt: &DeletePrompt,
) -> std::io::Result<()> {
    let is_folder = prompt.folder.is_some();
    let removed = if is_folder {
        remove_folder(&prompt.path, &prompt.section_root, &prompt.section_roots)
    } else {
        std::fs::remove_file(&prompt.path)
    };
    // A refused folder delete touched nothing, so nothing is retired.
    if is_folder && folder_change_allowed(&prompt.path, &prompt.section_root, &prompt.section_roots) {
        retire_folder_keys(forest, prompt, retired);
    }
    if removed.is_ok() && !is_folder {
        for path in [&prompt.path, &prompt.origin] {
            if let Some(tag_key) = rel_key(&prompt.tag_root, path) {
                retire(retired, &prompt.tag_root, tag_key);
            }
        }
    }
    removed
}

/// Add `(tag_root, key)` to `retired` unless it is listed already.
fn retire(retired: &mut Vec<(PathBuf, String)>, tag_root: &Path, key: String) {
    if !retired.iter().any(|(r, k)| r == tag_root && *k == key) {
        retired.push((tag_root.to_path_buf(), key));
    }
}

/// Retire the `.tags` keys of every note a folder delete removed, keys
/// relative to the section's tag root: for each note row under the folder
/// that is gone from the disk, its current and original key, and every key
/// of the session's loaded map under the folder whose file is gone (notes
/// tagged on disk but not listed). Called after the removal, so a delete
/// that failed midway retires exactly the notes that went.
fn retire_folder_keys(forest: &Forest, prompt: &DeletePrompt, retired: &mut Vec<(PathBuf, String)>) {
    let tag_root = &prompt.tag_root;
    let gone = |p: &Path| std::fs::symlink_metadata(p).is_err();
    for node in &forest.nodes {
        if node.is_dir || !node.path.starts_with(&prompt.path) || !gone(&node.path) {
            continue;
        }
        for path in [&node.path, &node.origin] {
            if let Some(key) = rel_key(tag_root, path) {
                retire(retired, tag_root, key);
            }
        }
    }
    let Some(folder_key) = rel_key(tag_root, &prompt.path) else {
        return;
    };
    let Some(i) = forest.tag_roots.iter().position(|r| r == tag_root) else {
        return;
    };
    let mut keys: Vec<&String> = forest.initial[i]
        .keys()
        .filter(|k| Path::new(k.as_str()).starts_with(&folder_key) && gone(&tag_root.join(k.as_str())))
        .collect();
    keys.sort();
    for key in keys {
        retire(retired, tag_root, key.clone());
    }
}

/// The forest's sections listing its current file rows and their listed
/// folders, minus the paths `gone` matches: the fallback listing when the
/// rebuild closure fails. Takes the sections out of `forest`; the caller
/// puts a rebuilt set back.
fn current_sections_without(forest: &mut Forest, gone: &dyn Fn(&Path) -> bool) -> Vec<SectionSpec> {
    let mut sections = std::mem::take(&mut forest.sections);
    for (i, spec) in sections.iter_mut().enumerate() {
        spec.files = forest
            .nodes
            .iter()
            .filter(|n| !n.is_dir && n.section == i && !gone(&n.path))
            .map(|n| n.path.clone())
            .collect();
        spec.dirs.retain(|d| !gone(d));
    }
    sections
}

// --- Marks ---
//
// `Space` marks note and folder rows by path for the session; nothing is
// persisted. Marks are matched by path, so they survive navigation, the
// filter and a rebuild, and a row whose path changes (rename, move) or that
// is no longer listed loses its mark (see [`prune_marks`]). With marks
// present, `d`, `m` and `S` act on the [`action_set`] instead of the cursor
// row.

/// The footer message for `Space` on a section row.
const SECTION_MARK: &str = "mark: a section cannot be marked";

/// Drawn in the gutter of a marked row, the column before its tag dots.
const MARK_GLYPH: &str = "▌";

/// Toggle the mark on `row`: `Ok(true)` when a note or folder row was
/// marked or unmarked, `Ok(false)` when there is no row, `Err` with the
/// footer message on a section row.
fn toggle_mark(
    marks: &mut HashSet<PathBuf>,
    nodes: &[TreeNode],
    row: Option<usize>,
) -> std::result::Result<bool, &'static str> {
    let Some(node) = row.and_then(|i| nodes.get(i)) else {
        return Ok(false);
    };
    if node.depth == 0 {
        return Err(SECTION_MARK);
    }
    if !marks.remove(&node.path) {
        marks.insert(node.path.clone());
    }
    Ok(true)
}

/// `Space` in browse mode: toggle the mark on the cursor row, then move the
/// cursor down one row (as `j` does) unless it is on the last one. Returns
/// the footer message of a refusal; a refused row keeps the cursor.
fn press_space(
    marks: &mut HashSet<PathBuf>,
    nodes: &mut [TreeNode],
    state: &mut ListState,
    visible: &[usize],
    focus_active: bool,
) -> Option<&'static str> {
    let selected = state.selected().unwrap_or(0);
    match toggle_mark(marks, nodes, visible.get(selected).copied()) {
        Ok(true) if selected + 1 < visible.len() => {
            navigate(nodes, state, visible, selected, visible[selected], focus_active, 1);
            None
        }
        Ok(_) => None,
        Err(message) => Some(message),
    }
}

/// `Esc` in browse mode: clear the marks if there are any and do nothing
/// else; otherwise clear the filter if there is one; otherwise do nothing.
/// `Esc` never quits, so a stray press after a prompt closes is harmless;
/// `q` and `:q` are the ways out.
fn browse_escape(marks: &mut HashSet<PathBuf>, search_buffer: &mut String) {
    if !marks.is_empty() {
        marks.clear();
    } else {
        search_buffer.clear();
    }
}

/// Drop every mark whose path no note or folder row lists any more.
fn prune_marks(marks: &mut HashSet<PathBuf>, nodes: &[TreeNode]) {
    if marks.is_empty() {
        return;
    }
    let listed: HashSet<&Path> =
        nodes.iter().filter(|n| n.depth > 0).map(|n| n.path.as_path()).collect();
    marks.retain(|p| listed.contains(p.as_path()));
}

/// Whether `node` is drawn and acted on as marked. Section rows never are,
/// even when a folder row elsewhere shares their path.
fn is_marked(marks: &HashSet<PathBuf>, node: &TreeNode) -> bool {
    node.depth > 0 && marks.contains(&node.path)
}

/// The rows a bulk action works on, in tree order: every marked note or
/// folder row minus each row with a marked folder above it in its section,
/// so a folder and one of its own notes act once, on the folder. A marked
/// path no row lists is left out.
fn action_set(marks: &HashSet<PathBuf>, nodes: &[TreeNode]) -> Vec<usize> {
    (0..nodes.len())
        .filter(|&i| is_marked(marks, &nodes[i]))
        .filter(|&i| {
            let mut up = nodes[i].parent_idx;
            while let Some(p) = up {
                if is_marked(marks, &nodes[p]) {
                    return false;
                }
                up = nodes[p].parent_idx;
            }
            true
        })
        .collect()
}

/// A row's `line` drawn as marked: the gutter before the tag dots shows
/// [`MARK_GLYPH`] in the header colour and the whole row is bold. Every
/// other column stays where it was, so the tag dots keep their click
/// positions.
fn mark_row(mut line: Line<'static>) -> Line<'static> {
    for span in &mut line.spans {
        span.style = span.style.add_modifier(Modifier::BOLD);
    }
    if let Some(gutter) = line.spans.first_mut() {
        *gutter = Span::styled(MARK_GLYPH, theme::header());
    }
    line
}

/// The mark count that leads the footer while any row is marked.
fn marked_lead(count: usize) -> Vec<Span<'static>> {
    vec![Span::styled(format!(" {count} marked "), theme::header())]
}

/// One row of a bulk delete: what the question counts and the single
/// delete's prompt for it, or the footer message `d` gives on that row
/// alone (the guard that refuses it), which the delete reports as its
/// failure.
#[derive(Debug, Clone)]
struct BulkItem {
    path: PathBuf,
    /// Path relative to its section's root.
    rel: String,
    is_dir: bool,
    scope: Scope,
    /// The scope's label, as the single delete question names it.
    label: String,
    /// The notes a folder holds; 0 for a note.
    notes_inside: usize,
    /// A folder holds something besides notes (see [`FolderContents`]).
    has_other_files: bool,
    prompt: std::result::Result<DeletePrompt, &'static str>,
}

impl BulkItem {
    /// The name a failure message uses: the relative path, a folder's with
    /// a trailing `/`.
    fn display_name(&self) -> String {
        if self.is_dir { format!("{}/", self.rel) } else { self.rel.clone() }
    }
}

/// What `d` opens with rows marked: the [`bulk_items`] of the marks, or,
/// when the guard refuses any of them, the footer message for the first
/// one, its name in front (`personal/: delete: this folder holds another
/// section`). A refused set is refused whole, before anything is asked, so
/// the confirm only counts items that will be attempted.
fn bulk_delete_request(
    nodes: &[TreeNode],
    sections: &[SectionSpec],
    marks: &HashSet<PathBuf>,
    current_project: Option<&str>,
) -> std::result::Result<Vec<BulkItem>, String> {
    let items = bulk_items(nodes, sections, marks, current_project);
    if let Some(item) = items.iter().find(|i| i.prompt.is_err()) {
        let refusal = item.prompt.as_ref().err().copied().unwrap_or_default();
        return Err(format!("{}: {refusal}", item.display_name()));
    }
    Ok(items)
}

/// One item per row of the [`action_set`], each checked by
/// [`delete_request`] as if the cursor were on it. A folder's contents are
/// counted from the disk now.
fn bulk_items(
    nodes: &[TreeNode],
    sections: &[SectionSpec],
    marks: &HashSet<PathBuf>,
    current_project: Option<&str>,
) -> Vec<BulkItem> {
    let mut items = Vec::new();
    for i in action_set(marks, nodes) {
        let node = &nodes[i];
        let Some(spec) = sections.get(node.section) else {
            continue;
        };
        let prompt = match delete_request(nodes, sections, Some(i), current_project) {
            Ok(Some(prompt)) => Ok(prompt),
            Ok(None) => continue,
            Err(message) => Err(message),
        };
        let contents = match &prompt {
            _ if !node.is_dir => FolderContents::default(),
            Ok(DeletePrompt { folder: Some(contents), .. }) => *contents,
            _ => folder_contents(&node.path),
        };
        let rel = node.path.strip_prefix(&spec.root).unwrap_or(&node.path);
        items.push(BulkItem {
            path: node.path.clone(),
            rel: rel.to_string_lossy().into_owned(),
            is_dir: node.is_dir,
            scope: spec.scope,
            label: scope_label(spec.scope, spec.project.as_deref(), current_project),
            notes_inside: contents.notes,
            has_other_files: contents.has_other_files,
            prompt,
        });
    }
    items
}

/// `1 note`, `2 notes`.
fn count_of(count: usize, noun: &str) -> String {
    if count == 1 { format!("1 {noun}") } else { format!("{count} {noun}s") }
}

/// The bulk delete question: `delete 2 notes and 1 folder (3 notes inside)
/// from personal, global? y/n`. The note clause is left out when no note is
/// in the set and the folder clause when no folder is; each scope label is
/// named once, in tree order. As in the single delete, the folder clause
/// adds `and other files` when a folder holds anything but notes, and a set
/// with anything in local scratch says it cannot be undone.
fn bulk_delete_question(items: &[BulkItem]) -> String {
    let notes = items.iter().filter(|i| !i.is_dir).count();
    let folders = items.iter().filter(|i| i.is_dir).count();
    let mut what = Vec::new();
    if notes > 0 {
        what.push(count_of(notes, "note"));
    }
    if folders > 0 {
        let inside: usize = items.iter().map(|i| i.notes_inside).sum();
        let inside = if inside == 0 { "no notes".to_string() } else { count_of(inside, "note") };
        let other = if items.iter().any(|i| i.has_other_files) { " and other files" } else { "" };
        what.push(format!("{} ({inside} inside){other}", count_of(folders, "folder")));
    }
    let mut labels: Vec<&str> = Vec::new();
    for item in items {
        if !labels.contains(&item.label.as_str()) {
            labels.push(&item.label);
        }
    }
    let warning = if items.iter().any(|i| i.scope == Scope::Local) { " (not recoverable)" } else { "" };
    format!("delete {} from {}?{warning} y/n", what.join(" and "), labels.join(", "))
}

/// The bulk delete confirmation that leads the footer while it is open.
fn bulk_delete_lead(items: &[BulkItem]) -> Vec<Span<'static>> {
    vec![Span::styled(format!(" {}", bulk_delete_question(items)), Style::default().fg(theme::PEACH))]
}

/// Answer the open bulk delete with `key`. Anything but `y` cancels and
/// returns `None` without touching the disk or the forest. On `y` every
/// item is deleted in order exactly as a single delete would
/// ([`remove_and_retire`]); an item its guard refuses, or whose removal
/// fails, is counted as failed and the rest go on. Then the forest is
/// rebuilt once (falling back to the current rows minus what went, as
/// [`answer_delete`] does), and the cursor row follows the last deleted
/// item by [`Forest::rebuild_after_delete`]'s rule.
fn answer_bulk_delete(
    key: KeyCode,
    forest: &mut Forest,
    retired: &mut Vec<(PathBuf, String)>,
    items: &[BulkItem],
    search: &str,
    rebuild: &dyn Fn() -> Result<Vec<SectionSpec>>,
) -> Option<DeleteOutcome> {
    if key != KeyCode::Char('y') {
        return None;
    }
    let mut deleted = 0;
    let mut failed = 0;
    let mut first_failure: Option<String> = None;
    let mut last_deleted: Option<&Path> = None;
    for item in items {
        let result = match &item.prompt {
            Ok(prompt) => remove_and_retire(forest, retired, prompt).map_err(|e| e.to_string()),
            Err(message) => Err((*message).to_string()),
        };
        match result {
            Ok(()) => {
                deleted += 1;
                last_deleted = Some(&item.path);
            }
            Err(e) => {
                failed += 1;
                first_failure.get_or_insert_with(|| format!("{} ({e})", item.display_name()));
            }
        }
    }
    let mut message = match first_failure {
        None => format!("deleted {deleted}"),
        Some(first) => format!("deleted {deleted}, failed {failed}: {first}"),
    };
    let mut relisted = true;
    let sections = rebuild().unwrap_or_else(|e| {
        relisted = false;
        message = format!("{message}, but the list could not be refreshed: {e:#}");
        let gone = |p: &Path| {
            items.iter().any(|i| p.starts_with(&i.path)) && std::fs::symlink_metadata(p).is_err()
        };
        current_sections_without(forest, &gone)
    });
    let anchor = last_deleted.or_else(|| items.last().map(|i| i.path.as_path()));
    let row = forest.rebuild_after_delete(sections, anchor.unwrap_or(Path::new("")), search);
    Some(DeleteOutcome { row, message, relisted })
}

// --- Rename ---

/// The footer message for `r` on a section row.
const SECTION_RENAME: &str = "rename: a section cannot be renamed";

/// The footer message for a folder name that sanitizes to nothing.
const FOLDER_RENAME_EMPTY: &str = "rename: the name is empty";

/// What `r` does with the cursor on `row`: `Ok(Some)` opens the rename
/// prompt with that text (a note's editable title, a folder's name), `Err`
/// is the footer message for a section row or a row in the todo board's
/// store ([`is_todo_row`]), and `Ok(None)` does nothing:
/// no row under the cursor, or a folder in a docs section.
fn rename_request(
    nodes: &[TreeNode],
    sections: &[SectionSpec],
    row: Option<usize>,
) -> std::result::Result<Option<String>, &'static str> {
    let Some(node) = row.and_then(|i| nodes.get(i)) else {
        return Ok(None);
    };
    if node.depth == 0 {
        return Err(SECTION_RENAME);
    }
    if is_todo_row(node, sections) {
        return Err(TODOS_RENAME);
    }
    if !node.is_dir {
        return Ok(Some(rename::editable_title(&node.name)));
    }
    let is_doc = sections.get(node.section).map_or(true, |s| s.is_doc);
    Ok((!is_doc).then(|| node.name.clone()))
}

/// Rename the folder row at `idx` to `name` within its parent, sanitized
/// like `notez mkdir` names. Refused, with nothing changed, for an empty
/// name, a row [`folder_change_allowed`] rejects, or a target that already
/// exists (file or folder). On a case-insensitive file system a case-only
/// change finds the folder itself at the target; that is renamed, and
/// refused if the file system keeps the old spelling. On success every row
/// under the folder, and the stored sections' listings, take the new path;
/// rows keep their `origin`, so the exit write moves their `.tags` keys
/// and no `.tags` file is touched here. The rows stay where they are, so
/// the cursor stays on the folder.
fn rename_folder(
    nodes: &mut [TreeNode],
    sections: &mut [SectionSpec],
    idx: usize,
    name: &str,
) -> std::result::Result<(), String> {
    let Some(node) = nodes.get(idx).filter(|n| n.is_dir && n.depth > 0) else {
        return Ok(());
    };
    let Some(spec) = sections.get(node.section) else {
        return Ok(());
    };
    // Enter on the untouched prompt keeps the name as it is, even one
    // sanitizing would change (`00_quick-notes`, `_todos`, `IDEAS`).
    if name.trim() == node.name {
        return Ok(());
    }
    let cleaned = sanitize::name(name);
    if cleaned.is_empty() {
        return Err(FOLDER_RENAME_EMPTY.to_string());
    }
    let old = node.path.clone();
    let section_roots: Vec<PathBuf> = sections.iter().map(|s| s.root.clone()).collect();
    if !folder_change_allowed(&old, &spec.root, &section_roots) {
        return Err(format!("rename: {} is not a folder inside its section", old.display()));
    }
    let new = old.with_file_name(&cleaned);
    if new == old {
        return Ok(());
    }
    let case_only = std::fs::symlink_metadata(&new).is_ok();
    if case_only && !is_same_entry(&old, &new) {
        return Err(format!("rename: {cleaned} already exists"));
    }
    std::fs::rename(&old, &new).map_err(|e| format!("rename failed: {e}"))?;
    if case_only && !has_entry_named(new.parent().unwrap_or(&new), &cleaned) {
        return Err(format!("rename: the file system kept the name {}", node.name));
    }
    for node in nodes.iter_mut() {
        if let Some(moved) = moved_path(&node.path, &old, &new) {
            node.path = moved;
        }
    }
    nodes[idx].name = cleaned;
    for spec in sections.iter_mut() {
        for path in spec.files.iter_mut().chain(spec.dirs.iter_mut()) {
            if let Some(moved) = moved_path(path, &old, &new) {
                *path = moved;
            }
        }
    }
    Ok(())
}

/// What `Enter` in the rename prompt did.
#[derive(Debug, PartialEq, Eq)]
enum RenameEnter {
    /// The typed name is refused: the prompt stays open with it and the
    /// footer shows the message.
    Keep(String),
    /// The prompt closes, with the footer message if there is one.
    Done(Option<String>),
}

/// `Enter` in the rename prompt opened on the row at `idx` showing `shown`
/// (the text the prompt was prefilled with), with `typed` in the buffer.
/// `typed` trimmed equal to `shown` changes nothing: no file, heading or
/// `.tags` key moves, even for a name sanitizing would alter (`My_Note`,
/// `x.MD`). Otherwise a name [`name_would_change`] alters is refused before
/// any disk work, and the rest renames the folder ([`rename_folder`]) or
/// the note (`notez rename`'s [`rename::rename_note`]).
fn enter_rename(
    nodes: &mut [TreeNode],
    sections: &mut [SectionSpec],
    idx: usize,
    shown: &str,
    typed: &str,
) -> RenameEnter {
    if typed.trim() == shown {
        return RenameEnter::Done(None);
    }
    if let Some(cleaned) = name_would_change(typed) {
        return RenameEnter::Keep(altered_name_message(&cleaned));
    }
    let Some(node) = nodes.get(idx) else {
        return RenameEnter::Done(None);
    };
    if node.is_dir {
        return RenameEnter::Done(rename_folder(nodes, sections, idx, typed).err());
    }
    match rename::rename_note(&node.path, typed) {
        Ok(new_path) => {
            nodes[idx].name = file_name_of(&new_path);
            nodes[idx].path = new_path;
            RenameEnter::Done(None)
        }
        Err(e) => RenameEnter::Done(Some(format!("rename failed: {e}"))),
    }
}

/// `path` with its `old` prefix replaced by `new`, if it lies under `old`.
fn moved_path(path: &Path, old: &Path, new: &Path) -> Option<PathBuf> {
    let rest = path.strip_prefix(old).ok()?;
    Some(if rest.as_os_str().is_empty() { new.to_path_buf() } else { new.join(rest) })
}

/// Whether `a` and `b` name the same directory entry (the case-insensitive
/// match of a case-only rename).
#[cfg(unix)]
fn is_same_entry(a: &Path, b: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    match (std::fs::symlink_metadata(a), std::fs::symlink_metadata(b)) {
        (Ok(x), Ok(y)) => x.dev() == y.dev() && x.ino() == y.ino(),
        _ => false,
    }
}

/// Without inode numbers a match cannot be told from another entry, so an
/// existing target is always refused.
#[cfg(not(unix))]
fn is_same_entry(_a: &Path, _b: &Path) -> bool {
    false
}

/// Whether `dir` lists an entry spelled exactly `name`.
fn has_entry_named(dir: &Path, name: &str) -> bool {
    std::fs::read_dir(dir)
        .map(|entries| entries.flatten().any(|e| e.file_name() == name))
        .unwrap_or(false)
}

// --- Move ---

/// The footer message for `m` on a section row.
const SECTION_MOVE: &str = "move: a section cannot be moved";

/// The footer message for `m` on a row in a docs section: those are the
/// repository's own files. Docs sections are never a destination either.
const DOCS_MOVE: &str = "move: not for a docs section";

/// The footer message for `m` on a folder that holds another section.
const FOLDER_HOLDS_SECTION_MOVE: &str = "move: this folder holds another section";

/// The footer message for `m` with no row under the cursor.
const NOTHING_TO_MOVE: &str = "move: no note under the cursor";

/// The footer message for `S` on a section row.
const SECTION_SET_SCOPE: &str = "set scope: a section has no scope to change";

/// The footer message for `S` on a row in a docs section.
const DOCS_SET_SCOPE: &str = "set scope: not for a docs section";

/// The footer message for `S` on a folder that holds another section.
const FOLDER_HOLDS_SECTION_SET_SCOPE: &str = "set scope: this folder holds another section";

/// The footer message for `S` with no row under the cursor.
const NOTHING_TO_SET_SCOPE: &str = "set scope: no note under the cursor";

/// The open move prompt for one note or folder. `scope` is the new-note
/// prompt's machinery: its target is the root of the destination scope,
/// which `Tab` cycles as it does for `n`, and its buffer is the destination
/// folder relative to that root, prefilled with the row's current folder.
/// `src` is the note or folder, `rel` its path relative to its section root
/// and `from` its scope. `notes` is `Some` for a folder: the notes it
/// holds, counted when the prompt opens, for the question. `fixed` marks
/// the prompt `S` opens: the buffer stays the row's current folder (typing
/// does nothing) and only the scope changes.
struct MovePrompt {
    scope: NewNotePrompt,
    src: PathBuf,
    rel: String,
    from: Scope,
    notes: Option<usize>,
    fixed: bool,
}

impl MovePrompt {
    /// The word footer messages about this prompt start with.
    fn verb(&self) -> &'static str {
        if self.fixed { "set scope" } else { "move" }
    }
}

/// What `m` does with the cursor on `row`: `Ok` opens the move prompt on a
/// note or a folder, `Err` is the footer message for a section row, a row
/// in a docs section or in the todo board's store ([`is_todo_row`]), a
/// folder holding another section, or no row at all.
/// The scopes `Tab` offers are the row's project's plus global; for a
/// global row, the project the browser was opened in, if any.
fn move_request(
    nodes: &[TreeNode],
    sections: &[SectionSpec],
    row: Option<usize>,
    ctx: &TreeContext,
) -> std::result::Result<MovePrompt, &'static str> {
    open_move(nodes, sections, row, ctx, false)
}

/// What `S` does with the cursor on `row`: the prompt `m` opens, with the
/// folder fixed to the row's current one (see [`MovePrompt`]), refused for
/// the same rows with `set scope:` messages.
fn set_scope_request(
    nodes: &[TreeNode],
    sections: &[SectionSpec],
    row: Option<usize>,
    ctx: &TreeContext,
) -> std::result::Result<MovePrompt, &'static str> {
    open_move(nodes, sections, row, ctx, true)
}

/// The prompt behind [`move_request`] (`fixed` false) and
/// [`set_scope_request`] (`fixed` true).
fn open_move(
    nodes: &[TreeNode],
    sections: &[SectionSpec],
    row: Option<usize>,
    ctx: &TreeContext,
    fixed: bool,
) -> std::result::Result<MovePrompt, &'static str> {
    let pick = |for_move: &'static str, for_scope: &'static str| if fixed { for_scope } else { for_move };
    let Some(node) = row.and_then(|i| nodes.get(i)) else {
        return Err(pick(NOTHING_TO_MOVE, NOTHING_TO_SET_SCOPE));
    };
    if node.depth == 0 {
        return Err(pick(SECTION_MOVE, SECTION_SET_SCOPE));
    }
    let Some(spec) = sections.get(node.section) else {
        return Err(pick(NOTHING_TO_MOVE, NOTHING_TO_SET_SCOPE));
    };
    if spec.is_doc {
        return Err(pick(DOCS_MOVE, DOCS_SET_SCOPE));
    }
    if is_todo_row(node, sections) {
        return Err(pick(TODOS_MOVE, TODOS_SET_SCOPE));
    }
    let notes = if node.is_dir {
        let section_roots: Vec<PathBuf> = sections.iter().map(|s| s.root.clone()).collect();
        if !folder_change_allowed(&node.path, &spec.root, &section_roots) {
            return Err(pick(FOLDER_HOLDS_SECTION_MOVE, FOLDER_HOLDS_SECTION_SET_SCOPE));
        }
        Some(folder_contents(&node.path).notes)
    } else {
        None
    };
    let current = ctx.current_project.as_deref();
    let project = spec.project.clone().or_else(|| ctx.current_project.clone());
    let root = NewNoteTarget {
        dir: spec.root.clone(),
        scope: spec.scope,
        label: scope_label(spec.scope, project.as_deref(), current),
    };
    let rel_to_root = |path: &Path| {
        path.strip_prefix(&spec.root).map(|r| r.to_string_lossy().into_owned()).ok()
    };
    let mut scope = NewNotePrompt::open(root, project);
    scope.buffer = node.path.parent().and_then(rel_to_root).unwrap_or_default();
    Ok(MovePrompt {
        scope,
        src: node.path.clone(),
        rel: rel_to_root(&node.path).unwrap_or_else(|| node.name.clone()),
        from: spec.scope,
        notes,
        fixed,
    })
}

/// The prompt `S` leads the footer with: `set scope of <name>: <scope>
/// (Tab cycles, Enter applies)`.
fn set_scope_lead(prompt: &MovePrompt) -> Vec<Span<'static>> {
    vec![Span::styled(
        format!(
            " set scope of {}: {} (Tab cycles, Enter applies)",
            file_name_of(&prompt.src),
            prompt.scope.target.label
        ),
        Style::default().fg(theme::MAUVE),
    )]
}

/// The move prompt that leads the footer while the folder is typed:
/// `move <name> to <scope>/<folder>_`.
fn move_lead(prompt: &MovePrompt) -> Vec<Span<'static>> {
    vec![
        Span::styled(
            format!(" move {} to {}/", file_name_of(&prompt.src), prompt.scope.target.label),
            Style::default().fg(theme::MAUVE),
        ),
        Span::styled(prompt.scope.buffer.clone(), Style::default().fg(theme::TEXT)),
        Span::styled("_", Style::default().fg(theme::OVERLAY)),
    ]
}

/// A resolved move: the note or folder at `src` goes to `dst`. `rel` is its
/// path relative to its section root, `to_root` the destination scope's
/// root and `display` the destination as the prompt names it. `notes` is
/// `Some` for a folder (see [`MovePrompt`]).
#[derive(Debug, Clone, PartialEq, Eq)]
struct MovePlan {
    src: PathBuf,
    dst: PathBuf,
    rel: String,
    from: Scope,
    to: Scope,
    to_root: PathBuf,
    display: String,
    notes: Option<usize>,
}

impl MovePlan {
    /// The scope changes, so the move asks first.
    fn needs_confirm(&self) -> bool {
        self.from != self.to
    }

    /// What moves, as messages name it: the note's `rel`, or a folder's
    /// `rel` with a trailing `/`.
    fn what(&self) -> String {
        match self.notes {
            None => self.rel.clone(),
            Some(_) => format!("{}/", self.rel),
        }
    }

    /// The confirmation question (see [`move_question`]). A public store is
    /// `<repo>/notez`, so the repository is named by its directory. A
    /// folder is named `<rel>/` with its notes counted as the delete
    /// question counts them (`and its 2 notes`, `(no notes)`).
    fn question(&self) -> String {
        let repo = repo_name(&self.to_root);
        let what = match self.notes {
            None => self.rel.clone(),
            Some(0) => format!("{}/ (no notes)", self.rel),
            Some(1) => format!("{}/ and its 1 note", self.rel),
            Some(n) => format!("{}/ and its {n} notes", self.rel),
        };
        move_question(&what, &self.display, self.from, self.to, &repo)
    }
}

/// The repository a public store `<repo>/notez` belongs to, by its
/// directory name.
fn repo_name(store_root: &Path) -> String {
    store_root.parent().and_then(Path::file_name).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Resolve the move prompt's `Enter`: the folder typed in the buffer under
/// the chosen scope's root. Nothing on disk is touched. `Err` is the footer
/// message: a folder that does not exist (or is hidden, a symlink, or
/// steps out with `..`), a global destination inside `personal/` (those
/// are the projects' stores) or inside the todo board's store, the row's
/// own folder, a destination inside
/// the folder being moved, or a destination name that is taken (never
/// overwritten). The messages start with the prompt's verb (`move:` or
/// `set scope:`).
fn resolve_move(prompt: &MovePrompt, roots: &NewNoteRoots) -> std::result::Result<MovePlan, String> {
    let verb = prompt.verb();
    let target = &prompt.scope.target;
    let (dir, display) = resolve_folder(verb, target, &prompt.scope.buffer, roots)?;
    let name = file_name_of(&prompt.src);
    let dst = dir.join(&name);
    if dst == prompt.src {
        return Err(format!("{verb}: {name} is already in {display}"));
    }
    if prompt.notes.is_some() && dst.starts_with(&prompt.src) {
        return Err(format!("{verb}: {name}/ cannot go inside itself"));
    }
    if std::fs::symlink_metadata(&dst).is_ok() {
        return Err(format!("{verb}: {name} already exists in {display}"));
    }
    Ok(MovePlan {
        src: prompt.src.clone(),
        dst,
        rel: prompt.rel.clone(),
        from: prompt.from,
        to: target.scope,
        to_root: target.dir.clone(),
        display,
        notes: prompt.notes,
    })
}

/// The destination folder `buffer` names under `target`'s root, and the
/// destination as the prompt shows it (`personal/plans`). `Err` is the
/// footer message, starting with `verb`: a folder that does not exist
/// under exactly that spelling, is hidden, a symlink, or steps out with
/// `..`, or a global destination inside `personal/` or inside the todo
/// board's store (see [`in_todo_store`]).
fn resolve_folder(
    verb: &str,
    target: &NewNoteTarget,
    buffer: &str,
    roots: &NewNoteRoots,
) -> std::result::Result<(PathBuf, String), String> {
    let folder: Vec<&str> = buffer.split('/').filter(|c| !c.is_empty()).collect();
    let display = if folder.is_empty() {
        target.label.clone()
    } else {
        format!("{}/{}", target.label, folder.join("/"))
    };
    let no_folder = || format!("{verb}: no folder {display}");
    if folder.iter().any(|c| c.starts_with('.')) {
        return Err(no_folder());
    }
    // Walk the typed folder one step at a time: each step must be listed
    // under exactly that spelling (a case-insensitive file system would
    // otherwise resolve `Personal` or `Plans`, and the tag keys would take
    // the typed spelling) and must be a real folder, not a symlink.
    if !std::fs::symlink_metadata(&target.dir).is_ok_and(|m| m.is_dir()) {
        return Err(no_folder());
    }
    let mut dir = target.dir.clone();
    for step in &folder {
        if !has_entry_named(&dir, step) {
            return Err(no_folder());
        }
        dir = dir.join(step);
        let Ok(meta) = std::fs::symlink_metadata(&dir) else {
            return Err(no_folder());
        };
        if meta.file_type().is_symlink() {
            return Err(format!("{verb}: {display} is not a plain folder"));
        }
        if !meta.is_dir() {
            return Err(no_folder());
        }
    }
    if target.scope == Scope::Global && dir.starts_with(roots.global.join("personal")) {
        return Err(format!("{verb}: {display} holds the projects' personal notes, not global ones"));
    }
    if target.scope == Scope::Global && in_todo_store(&dir, &roots.global) {
        return Err(format!("{verb}: {TODO_STORE_MANAGED}"));
    }
    Ok((dir, display))
}

/// A typing key in the move prompt: `Backspace` and characters edit the
/// destination folder; the prompt `S` opens has a fixed folder and ignores
/// them, as it ignores every other key.
fn type_into_move(prompt: &mut MovePrompt, code: KeyCode) {
    if prompt.fixed {
        return;
    }
    match code {
        KeyCode::Backspace => {
            prompt.scope.buffer.pop();
        }
        KeyCode::Char(c) => prompt.scope.buffer.push(c),
        _ => {}
    }
}

/// `Enter` in the move or set-scope prompt: `Ok(None)` when there is
/// nothing to do (`S` on the scope the row is already in), otherwise
/// [`resolve_move`].
fn resolve_enter(prompt: &MovePrompt, roots: &NewNoteRoots) -> std::result::Result<Option<MovePlan>, String> {
    if prompt.fixed && prompt.scope.target.scope == prompt.from {
        return Ok(None);
    }
    resolve_move(prompt, roots).map(Some)
}

/// The question a move across scopes asks: `move <rel> to <scope>/<folder>?
/// y/n`, with one clause in parentheses per thing the scope change does,
/// public first: into public it names the repository and says public; out
/// of public the note stays in the repository's history; into local
/// scratch it is not synced and not recoverable; out of the vault
/// (personal or global) into a repository the deletion syncs on exit.
fn move_question(rel: &str, display: &str, from: Scope, to: Scope, repo: &str) -> String {
    format!("move {rel} to {display}?{} y/n", move_clauses(&[(from, to)], repo))
}

/// The clauses of [`move_question`] for every `(from, to)` transition in
/// `transitions`, each clause once and in its fixed order (public first),
/// as ` (<clause>)` pieces; empty when no transition changes anything.
fn move_clauses(transitions: &[(Scope, Scope)], repo: &str) -> String {
    let in_vault = |scope: Scope| matches!(scope, Scope::Personal | Scope::Global);
    let any = |applies: &dyn Fn(Scope, Scope) -> bool| transitions.iter().any(|&(from, to)| applies(from, to));
    let mut clauses = Vec::new();
    if any(&|from, to| to == Scope::Public && from != Scope::Public) {
        clauses.push(format!("it will be in the {repo} repository, public, not yet committed"));
    }
    if any(&|from, to| from == Scope::Public && to != Scope::Public) {
        clauses.push("it stays in the repository's git history".to_string());
    }
    if any(&|from, to| to == Scope::Local && from != Scope::Local) {
        clauses.push("scratch is not synced and not recoverable".to_string());
    }
    if any(&|from, to| in_vault(from) && !in_vault(to)) {
        clauses.push("it leaves the vault; the deletion syncs on exit".to_string());
    }
    clauses.iter().map(|c| format!(" ({c})")).collect()
}

/// The move confirmation that leads the footer while it is open.
fn move_confirm_lead(plan: &MovePlan) -> Vec<Span<'static>> {
    vec![Span::styled(format!(" {}", plan.question()), Style::default().fg(theme::PEACH))]
}

/// What a move leaves: the row for the cursor and the footer message.
#[derive(Debug)]
struct MoveOutcome {
    row: Option<usize>,
    message: String,
    /// Whether `rebuild` listed the forest again (not the fallback).
    relisted: bool,
}

/// A moved note the rebuilt list does not show (its destination is outside
/// the view): its destination tag root, new path and flags, so the exit
/// write still carries its tags.
type CarriedTags = (PathBuf, PathBuf, u8);

/// Whether `key` answers the open move confirmation with yes. Only `y`
/// does; any other key cancels and nothing moves.
fn move_confirmed(key: KeyCode) -> bool {
    key == KeyCode::Char('y')
}

/// The `.tags` root of the destination: its section's, or, for a store with
/// no section in the view, the root that section would have (the notez root
/// for personal and global, the store itself for public and local).
fn destination_tag_root(plan: &MovePlan, sections: &[SectionSpec], roots: &NewNoteRoots) -> PathBuf {
    if let Some(spec) = sections.iter().find(|s| !s.is_doc && s.root == plan.to_root) {
        return spec.tag_root.clone();
    }
    match plan.to {
        Scope::Personal | Scope::Global => roots.global.clone(),
        Scope::Public | Scope::Local => plan.to_root.clone(),
    }
}

/// Move the note or folder through [`move_path::move_path`]. On failure
/// nothing is retired, and the list is read again (a copy that failed
/// midway leaves the destination on disk too). On success every moved
/// note row (the note itself, or each note at any depth under the folder)
/// has its old `.tags` keys (current and original path, in its old tag
/// root) retired and takes its new path as both `path` and `origin`, the
/// destination's tag root and section, and keeps its flags; every folder
/// row under a moved folder takes its new path and section. Tagged keys
/// under a moved folder that no row lists (notes in hidden folders) are
/// retired and go to `carried` at their new path, and `carried` entries
/// an earlier move left under the source take the new path and tag root.
/// No `.tags` file is written here. The forest is then rebuilt with the cursor on the moved
/// row, its section and folders expanded. A moved row the rebuilt list
/// does not show is reported by path, and every moved note it does not
/// list goes to `carried`.
fn apply_move(
    forest: &mut Forest,
    retired: &mut Vec<(PathBuf, String)>,
    carried: &mut Vec<CarriedTags>,
    plan: &MovePlan,
    roots: &NewNoteRoots,
    rebuild: &dyn Fn() -> Result<Vec<SectionSpec>>,
) -> MoveOutcome {
    let moved = match move_and_repoint(forest, retired, carried, plan, roots) {
        Ok(moved) => moved,
        Err(e) => {
            let (row, relisted) = match rebuild() {
                Ok(sections) => (forest.rebuild(sections, &plan.src), true),
                Err(_) => (forest.nodes.iter().position(|n| n.path == plan.src), false),
            };
            return MoveOutcome { row, message: format!("move failed: {e}"), relisted };
        }
    };
    let mut message = format!("moved {} to {}", plan.what(), plan.display);
    let mut refreshed = true;
    let sections = rebuild().unwrap_or_else(|e| {
        refreshed = false;
        message = format!("{message}, but the list could not be refreshed: {e:#}");
        sections_after_moves(forest, std::slice::from_ref(plan))
    });
    let row = forest.rebuild(sections, &plan.dst);
    carry_unlisted_moved(forest, carried, &moved);
    if row.is_none() && refreshed {
        message = format!("moved to {}", plan.dst.display());
    }
    MoveOutcome { row, message, relisted: refreshed }
}

/// What [`move_and_repoint`] moved: the destination tag root and every
/// moved note row's new path with its flags.
struct Moved {
    tag_root: PathBuf,
    notes: Vec<(PathBuf, u8)>,
}

/// The fallback listing when the rebuild after `plans` fails: the current
/// rows (already repointed) minus anything still under a source, plus every
/// folder row now under a destination.
fn sections_after_moves(forest: &mut Forest, plans: &[MovePlan]) -> Vec<SectionSpec> {
    let mut sections = current_sections_without(forest, &|p| plans.iter().any(|plan| p.starts_with(&plan.src)));
    for node in forest.nodes.iter().filter(|n| {
        n.is_dir && n.depth > 0 && plans.iter().any(|plan| n.path.starts_with(&plan.dst))
    }) {
        if let Some(spec) = sections.get_mut(node.section) {
            if !spec.dirs.contains(&node.path) {
                spec.dirs.push(node.path.clone());
            }
        }
    }
    sections
}

/// After the rebuild: every moved note the list does not show goes to
/// `carried`, so the exit write still carries its tags.
fn carry_unlisted_moved(forest: &Forest, carried: &mut Vec<CarriedTags>, moved: &Moved) {
    for (path, flags) in &moved.notes {
        if !forest.nodes.iter().any(|n| !n.is_dir && n.path == *path) {
            carried.push((moved.tag_root.clone(), path.clone(), *flags));
        }
    }
}

/// The part of [`apply_move`] before its rebuild: move the note or folder
/// through [`move_path::move_path`], then retire and repoint as
/// [`apply_move`] describes. On failure nothing is retired or repointed.
fn move_and_repoint(
    forest: &mut Forest,
    retired: &mut Vec<(PathBuf, String)>,
    carried: &mut Vec<CarriedTags>,
    plan: &MovePlan,
    roots: &NewNoteRoots,
) -> std::result::Result<Moved, move_path::MoveError> {
    move_path::move_path(&plan.src, &plan.dst)?;
    let tag_root = destination_tag_root(plan, &forest.sections, roots);
    let tag_idx = match forest.tag_roots.iter().position(|r| *r == tag_root) {
        Some(i) => i,
        None => {
            forest.initial.push(note_tags::load_tags(&tag_root));
            forest.tag_roots.push(tag_root.clone());
            forest.tag_roots.len() - 1
        }
    };
    let section = forest.sections.iter().position(|s| !s.is_doc && s.root == plan.to_root);
    let src_tag_idx = forest.nodes.iter().find(|n| n.depth > 0 && n.path == plan.src).map(|n| n.tag_root);
    let mut moved_notes: Vec<(PathBuf, u8)> = Vec::new();
    for node in forest.nodes.iter_mut().filter(|n| n.depth > 0) {
        let Some(new_path) = moved_path(&node.path, &plan.src, &plan.dst) else {
            continue;
        };
        if !node.is_dir {
            let old_root = &forest.tag_roots[node.tag_root];
            for path in [&node.path, &node.origin] {
                if let Some(key) = rel_key(old_root, path) {
                    retire(retired, old_root, key);
                }
            }
            node.origin = new_path.clone();
            node.tag_root = tag_idx;
            moved_notes.push((new_path.clone(), node.flags));
        }
        node.path = new_path;
        if let Some(section) = section {
            node.section = section;
        }
    }
    // Tags carried by an earlier move (a hidden note, or a note out of the
    // view) follow this move too, so they stay on the file's current path.
    for (root, path, _) in carried.iter_mut() {
        if let Some(new_path) = moved_path(path, &plan.src, &plan.dst) {
            *path = new_path;
            *root = tag_root.clone();
        }
    }
    if let (Some(_), Some(old_idx)) = (plan.notes, src_tag_idx) {
        carry_unlisted_keys(forest, retired, carried, plan, old_idx, &tag_root);
    }
    Ok(Moved { tag_root, notes: moved_notes })
}

/// For a moved folder: the keys of the source tag root's session map that
/// lie under the folder, belong to no row and name a file that is now under
/// the destination (a tagged note the tree does not list, such as one in a
/// hidden folder). Each is retired from the old tag root and goes to
/// `carried` at its new path with its flags, so the exit write moves it.
/// Keys a row already retired are left to that row.
fn carry_unlisted_keys(
    forest: &Forest,
    retired: &mut Vec<(PathBuf, String)>,
    carried: &mut Vec<CarriedTags>,
    plan: &MovePlan,
    old_idx: usize,
    tag_root: &Path,
) {
    let old_root = &forest.tag_roots[old_idx];
    let Some(folder_key) = rel_key(old_root, &plan.src) else {
        return;
    };
    let mut keys: Vec<(&String, u8)> = forest.initial[old_idx]
        .iter()
        .filter(|(k, _)| Path::new(k.as_str()).starts_with(&folder_key))
        .filter(|(k, _)| !retired.iter().any(|(r, key)| r == old_root && key == *k))
        .map(|(k, f)| (k, *f))
        .collect();
    keys.sort();
    for (key, flags) in keys {
        let Some(new_path) = moved_path(&old_root.join(key), &plan.src, &plan.dst) else {
            continue;
        };
        if !std::fs::symlink_metadata(&new_path).is_ok_and(|m| m.is_file()) {
            continue;
        }
        retire(retired, old_root, key.clone());
        carried.push((tag_root.to_path_buf(), new_path, flags));
    }
}

/// The tag maps to write on exit ([`changed_tag_maps_retiring`]), with the
/// `carried` tags of moved notes the list no longer shows added as rows of
/// their own. A carried note that is listed again keeps its listed row.
fn exit_tag_maps(
    forest: &Forest,
    retired: &[(PathBuf, String)],
    carried: &[CarriedTags],
) -> Vec<(PathBuf, HashMap<String, u8>)> {
    let mut nodes = forest.nodes.clone();
    let mut tag_roots = forest.tag_roots.clone();
    let mut initial = forest.initial.clone();
    for (root, path, flags) in carried {
        if nodes.iter().any(|n| !n.is_dir && n.path == *path) {
            continue;
        }
        let tag_root = match tag_roots.iter().position(|r| r == root) {
            Some(i) => i,
            None => {
                tag_roots.push(root.clone());
                initial.push(note_tags::load_tags(root));
                tag_roots.len() - 1
            }
        };
        nodes.push(TreeNode {
            name: file_name_of(path),
            path: path.clone(),
            origin: path.clone(),
            is_dir: false,
            depth: 1,
            expanded: false,
            child_count: 0,
            parent_idx: None,
            flags: *flags,
            scope_icon: "",
            tag_root,
            section: 0,
        });
    }
    changed_tag_maps_retiring(&nodes, &tag_roots, &initial, retired)
}

// --- Bulk move and set scope ---
//
// With rows marked, `m` and `S` open their prompt once for the
// [`action_set`]. The prompt is the first item's, so `Tab` cycles that
// item's project's scopes; one prompt per item, as `m` or `S` would open it
// on that row, keeps what each item needs. The whole set is resolved before
// anything moves and refused as a whole on the first problem; the moves
// then run one by one, a failure leaving its item in place, followed by one
// rebuild.

/// A marked row's name in footer messages: its path relative to its
/// section root, a folder's with a trailing `/`.
fn marked_name(node: &TreeNode, sections: &[SectionSpec]) -> String {
    let rel = sections
        .get(node.section)
        .and_then(|spec| node.path.strip_prefix(&spec.root).ok())
        .map_or_else(|| node.name.clone(), |rel| rel.to_string_lossy().into_owned());
    if node.is_dir { format!("{rel}/") } else { rel }
}

/// What `m` (`fixed` false) or `S` (`fixed` true) opens with rows marked:
/// the shared prompt and one prompt per item of the [`action_set`], in tree
/// order. `Err` is the footer message: the guard refusal of the first item
/// [`open_move`] refuses (a docs row, a folder holding a section), its name
/// in front, or `<verb>: marked items span projects` when the items do not
/// all offer the same project's scopes.
fn bulk_move_request(
    nodes: &[TreeNode],
    sections: &[SectionSpec],
    marks: &HashSet<PathBuf>,
    ctx: &TreeContext,
    fixed: bool,
) -> std::result::Result<(MovePrompt, Vec<MovePrompt>), String> {
    let rows = action_set(marks, nodes);
    let mut items = Vec::new();
    for &i in &rows {
        match open_move(nodes, sections, Some(i), ctx, fixed) {
            Ok(item) => items.push(item),
            Err(refusal) => return Err(format!("{}: {refusal}", marked_name(&nodes[i], sections))),
        }
    }
    let prompt = open_move(nodes, sections, rows.first().copied(), ctx, fixed).map_err(str::to_string)?;
    if items.iter().any(|item| item.scope.project != prompt.scope.project) {
        return Err(format!("{}: marked items span projects", prompt.verb()));
    }
    Ok((prompt, items))
}

/// The prompt that leads the footer for a marked set: `move <n> items to
/// <scope>/<folder>_`, or `set scope of <n> items: <scope> (Tab cycles,
/// Enter applies)`.
fn bulk_move_lead(prompt: &MovePrompt, count: usize) -> Vec<Span<'static>> {
    let items = count_of(count, "item");
    let label = &prompt.scope.target.label;
    if prompt.fixed {
        return vec![Span::styled(
            format!(" set scope of {items}: {label} (Tab cycles, Enter applies)"),
            Style::default().fg(theme::MAUVE),
        )];
    }
    vec![
        Span::styled(format!(" move {items} to {label}/"), Style::default().fg(theme::MAUVE)),
        Span::styled(prompt.scope.buffer.clone(), Style::default().fg(theme::TEXT)),
        Span::styled("_", Style::default().fg(theme::OVERLAY)),
    ]
}

/// A resolved bulk move: one plan per item that moves, in tree order, and
/// the destination as the question names it (the scope and typed folder for
/// `m`, the scope alone for `S`, where each item keeps its own folder).
#[derive(Debug, Clone, PartialEq, Eq)]
struct BulkMovePlan {
    plans: Vec<MovePlan>,
    display: String,
}

impl BulkMovePlan {
    /// Some item changes scope, so the set asks first.
    fn needs_confirm(&self) -> bool {
        self.plans.iter().any(MovePlan::needs_confirm)
    }

    /// `move <n> items to <destination>? y/n` with the clauses of every
    /// scope change in the set, each once, public first (see
    /// [`move_clauses`]).
    fn question(&self) -> String {
        let transitions: Vec<(Scope, Scope)> = self.plans.iter().map(|p| (p.from, p.to)).collect();
        let repo = self.plans.first().map(|p| repo_name(&p.to_root)).unwrap_or_default();
        let clauses = move_clauses(&transitions, &repo);
        format!("move {} to {}?{clauses} y/n", count_of(self.plans.len(), "item"), self.display)
    }
}

/// Resolve `Enter` in the bulk prompt for `items` (see
/// [`bulk_move_request`]). Nothing on disk is touched. Each item goes to
/// `<destination folder>/<its own name>`: the typed folder for `m`, its own
/// folder under the chosen scope for `S`. An item already at its
/// destination is left out; `Ok(None)` when that leaves nothing for `S`
/// (`Enter` on the scope every item is in). `Err` is the footer message for
/// the first problem in tree order, and refuses the whole set: a folder
/// [`resolve_folder`] refuses, a folder going inside itself or inside
/// another marked folder, two items with the same destination name, an
/// existing destination, or (`m` only) nothing left to move.
fn resolve_bulk_move(
    prompt: &MovePrompt,
    items: &[MovePrompt],
    roots: &NewNoteRoots,
) -> std::result::Result<Option<BulkMovePlan>, String> {
    let verb = prompt.verb();
    let target = &prompt.scope.target;
    let display = if prompt.fixed {
        target.label.clone()
    } else {
        resolve_folder(verb, target, &prompt.scope.buffer, roots)?.1
    };
    let mut plans: Vec<MovePlan> = Vec::new();
    for item in items {
        let buffer = if prompt.fixed { &item.scope.buffer } else { &prompt.scope.buffer };
        let (dir, shown) = resolve_folder(verb, target, buffer, roots)?;
        let name = file_name_of(&item.src);
        let dst = dir.join(&name);
        if dst == item.src {
            continue;
        }
        if item.notes.is_some() && dst.starts_with(&item.src) {
            return Err(format!("{verb}: {name}/ cannot go inside itself"));
        }
        if let Some(holder) = items.iter().find(|o| o.notes.is_some() && dst.starts_with(&o.src)) {
            return Err(format!("{verb}: {shown}/{name} is inside the marked folder {}/", holder.rel));
        }
        if plans.iter().any(|p| p.dst == dst) {
            return Err(format!("{verb}: two marked items are named {name}"));
        }
        if std::fs::symlink_metadata(&dst).is_ok() {
            return Err(format!("{verb}: {shown}/{name} already exists"));
        }
        plans.push(MovePlan {
            src: item.src.clone(),
            dst,
            rel: item.rel.clone(),
            from: item.from,
            to: target.scope,
            to_root: target.dir.clone(),
            display: shown,
            notes: item.notes,
        });
    }
    if !plans.is_empty() {
        Ok(Some(BulkMovePlan { plans, display }))
    } else if prompt.fixed {
        Ok(None)
    } else {
        Err(format!("{verb}: the marked items are already in {display}"))
    }
}

/// The bulk move confirmation that leads the footer while it is open.
fn bulk_move_confirm_lead(plan: &BulkMovePlan) -> Vec<Span<'static>> {
    vec![Span::styled(format!(" {}", plan.question()), Style::default().fg(theme::PEACH))]
}

/// Run the resolved bulk move: each plan in order through the single
/// move's [`move_and_repoint`], so every item retires, repoints and carries
/// its tags exactly as [`apply_move`] does; a failing item stays in place,
/// is counted and the rest go on. Then the forest is rebuilt once (falling
/// back to the current rows as [`apply_move`] does) with the first moved
/// item revealed; its row is the outcome's, `None` when nothing moved or it
/// is not listed. The message is `moved <ok>` or `moved <ok>, failed <bad>:
/// <first failing name> (<error>)`.
fn apply_bulk_move(
    forest: &mut Forest,
    retired: &mut Vec<(PathBuf, String)>,
    carried: &mut Vec<CarriedTags>,
    plans: &[MovePlan],
    roots: &NewNoteRoots,
    rebuild: &dyn Fn() -> Result<Vec<SectionSpec>>,
) -> MoveOutcome {
    let mut done: Vec<MovePlan> = Vec::new();
    let mut moved: Vec<Moved> = Vec::new();
    let mut failed = 0;
    let mut first_failure: Option<String> = None;
    for plan in plans {
        match move_and_repoint(forest, retired, carried, plan, roots) {
            Ok(m) => {
                done.push(plan.clone());
                moved.push(m);
            }
            Err(e) => {
                failed += 1;
                first_failure.get_or_insert_with(|| format!("{} ({e})", plan.what()));
            }
        }
    }
    let mut message = match first_failure {
        None => format!("moved {}", done.len()),
        Some(first) => format!("moved {}, failed {failed}: {first}", done.len()),
    };
    let mut relisted = true;
    let sections = rebuild().unwrap_or_else(|e| {
        relisted = false;
        message = format!("{message}, but the list could not be refreshed: {e:#}");
        sections_after_moves(forest, &done)
    });
    let anchor = done.first().map_or(Path::new(""), |p| p.dst.as_path());
    let row = forest.rebuild(sections, anchor);
    for m in &moved {
        carry_unlisted_moved(forest, carried, m);
    }
    MoveOutcome { row, message, relisted }
}

// --- Rebuild ---

/// [`carry_state`], then expand the ancestors of `created` (a note or a
/// folder) so it is visible, and a created folder itself; its index is
/// returned.
fn restore_state(old: &[TreeNode], new: &mut [TreeNode], created: &Path) -> Option<usize> {
    carry_state(old, new);
    let idx = new.iter().position(|n| n.depth > 0 && n.path == created)?;
    if new[idx].is_dir {
        new[idx].expanded = true;
    }
    let mut parent = new[idx].parent_idx;
    while let Some(p) = parent {
        new[p].expanded = true;
        parent = new[p].parent_idx;
    }
    Some(idx)
}

/// Carry the session state over to a freshly built forest: directories keep
/// their expanded state and files their tags (including edits not yet
/// saved) and original path, matched by path.
fn carry_state(old: &[TreeNode], new: &mut [TreeNode]) {
    let by_path: HashMap<&Path, &TreeNode> =
        old.iter().map(|n| (n.path.as_path(), n)).collect();
    for node in new.iter_mut() {
        let Some(prev) = by_path.get(node.path.as_path()) else {
            continue;
        };
        if node.is_dir {
            node.expanded = prev.expanded;
        } else {
            node.flags = prev.flags;
            node.origin = prev.origin.clone();
        }
    }
}

/// Map row indices recorded against `old` (focus mode's saved expansion) to
/// the same paths in `new`, dropping rows that no longer exist.
fn remap_rows(old: &[TreeNode], new: &[TreeNode], rows: &[(usize, bool)]) -> Vec<(usize, bool)> {
    rows.iter()
        .filter_map(|&(i, v)| {
            let path = &old.get(i)?.path;
            new.iter().position(|n| &n.path == path).map(|j| (j, v))
        })
        .collect()
}

/// Tag maps as they were on disk when the session started, for the roots
/// of a rebuilt forest: known roots keep their snapshot, new ones load.
fn carry_initial_tags(
    old_roots: &[PathBuf],
    old_initial: &[HashMap<String, u8>],
    new_roots: &[PathBuf],
) -> Vec<HashMap<String, u8>> {
    new_roots
        .iter()
        .map(|r| match old_roots.iter().position(|o| o == r) {
            Some(i) => old_initial[i].clone(),
            None => note_tags::load_tags(r),
        })
        .collect()
}

// --- Reload (`R` and the file-change probe) ---

/// How long the event loop waits for input before it probes the disk.
const PROBE_INTERVAL: Duration = Duration::from_secs(2);

/// The modified time of each probed directory; `None` when it could not be
/// read (the path is gone or unreadable).
type ProbeSnapshot = HashMap<PathBuf, Option<SystemTime>>;

/// List the sections again through `rebuild` and swap them in, keeping the
/// session state the delete rebuild keeps: expanded folders, unsaved tag
/// edits and original paths, matched by path (see [`carry_state`]). Unlike
/// the post-create rebuild nothing is expanded, so the folder under the
/// cursor stays as it was. Returns the row the cursor goes to: `cursor`
/// itself while it is listed, else its nearest listed neighbour as after a
/// delete, among the rows the filter `search` shows. A failed listing leaves
/// the forest untouched and returns the footer message.
fn reload_forest(
    forest: &mut Forest,
    rebuild: &dyn Fn() -> Result<Vec<SectionSpec>>,
    cursor: &Path,
    search: &str,
) -> std::result::Result<Option<usize>, String> {
    let sections = rebuild().map_err(|e| format!("reload failed: {e:#}"))?;
    Ok(forest.rebuild_after_delete(sections, cursor, search))
}

/// The directories the probe stats: every section root, listed as a row or
/// not, and every expanded folder row. A collapsed folder is not probed, so
/// a change inside it shows on the next reload (`R`, or one the probe
/// triggers elsewhere).
fn probe_paths(sections: &[SectionSpec], nodes: &[TreeNode]) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = sections.iter().map(|s| s.root.clone()).collect();
    paths.extend(nodes.iter().filter(|n| n.depth > 0 && n.is_dir && n.expanded).map(|n| n.path.clone()));
    paths.sort();
    paths.dedup();
    paths
}

/// The modified time of each of `paths`, from `metadata`: one `stat` per
/// path, no file is opened or read. Symlinks are followed, so a section
/// root or folder that is a link reports its target directory's time and a
/// change under the target is seen.
fn probe_snapshot(paths: &[PathBuf]) -> ProbeSnapshot {
    paths
        .iter()
        .map(|p| (p.clone(), std::fs::metadata(p).and_then(|m| m.modified()).ok()))
        .collect()
}

/// Whether `new` shows a change on disk against `old`: a path probed in both
/// whose modified time differs, which covers a path that appeared or
/// vanished. A path in only one of them is no change: its folder was just
/// expanded (a new baseline) or collapsed (no longer probed).
fn probe_changed(old: &ProbeSnapshot, new: &ProbeSnapshot) -> bool {
    new.iter().any(|(path, time)| old.get(path).is_some_and(|before| before != time))
}

/// Merge a fresh reading of `forest` into the probe's last one. Call it
/// only right after the forest was listed again from disk (`R`, or a
/// create, delete, rename or move whose `rebuild` succeeded), so the next
/// idle probe does not reload for that again. Never after a refused,
/// failed or in-place action: a change made elsewhere while its prompt
/// was open (the probe does not run then) would be absorbed unseen.
fn refresh_probe(probe: &mut Option<ProbeSnapshot>, forest: &Forest) {
    let reading = probe_snapshot(&probe_paths(&forest.sections, &forest.nodes));
    probe.get_or_insert_with(HashMap::new).extend(reading);
}

/// After `Enter` in the rename prompt on row `idx`, whose path was
/// `before`. A rename renames in place without listing again, so one that
/// moved the row (its path changed) lists the forest again through
/// [`reload_view_at`], cursor on the renamed row, and on success refreshes
/// the probe. A refused, failed or unchanged rename does neither and
/// returns `None`; otherwise the reload's result.
fn relist_after_rename(
    forest: &mut Forest,
    rebuild: &dyn Fn() -> Result<Vec<SectionSpec>>,
    state: &mut ListState,
    pre_focus_expanded: &mut Vec<(usize, bool)>,
    search: &str,
    probe: &mut Option<ProbeSnapshot>,
    idx: usize,
    before: &Path,
) -> Option<std::result::Result<Option<usize>, String>> {
    let after = forest.nodes.get(idx).map(|n| n.path.clone()).filter(|p| p != before)?;
    let result = reload_view_at(forest, rebuild, state, pre_focus_expanded, search, after);
    if result.is_ok() {
        refresh_probe(probe, forest);
    }
    Some(result)
}

/// `R` and a probe-triggered reload in the event loop: [`reload_forest`]
/// from the cursor row, then the cursor goes to the returned row and focus
/// mode's saved expansion follows its paths. Returns the cursor's new row
/// when it stayed on the same path, so the preview keeps its scroll.
fn reload_view(
    forest: &mut Forest,
    rebuild: &dyn Fn() -> Result<Vec<SectionSpec>>,
    state: &mut ListState,
    pre_focus_expanded: &mut Vec<(usize, bool)>,
    search: &str,
) -> std::result::Result<Option<usize>, String> {
    let visible = compute_visible(&forest.nodes, search);
    let cursor = visible
        .get(state.selected().unwrap_or(0))
        .map(|&i| forest.nodes[i].path.clone())
        .unwrap_or_default();
    reload_view_at(forest, rebuild, state, pre_focus_expanded, search, cursor)
}

/// [`reload_view`] with the cursor on `cursor` rather than the selected row.
fn reload_view_at(
    forest: &mut Forest,
    rebuild: &dyn Fn() -> Result<Vec<SectionSpec>>,
    state: &mut ListState,
    pre_focus_expanded: &mut Vec<(usize, bool)>,
    search: &str,
    cursor: PathBuf,
) -> std::result::Result<Option<usize>, String> {
    let old_nodes = forest.nodes.clone();
    let row = reload_forest(forest, rebuild, &cursor, search)?;
    *pre_focus_expanded = remap_rows(&old_nodes, &forest.nodes, pre_focus_expanded);
    let visible = compute_visible(&forest.nodes, search);
    let pos = row.and_then(|r| visible.iter().position(|&i| i == r));
    state.select(Some(pos.unwrap_or(0)));
    Ok(row.filter(|&r| forest.nodes[r].path == cursor))
}

// --- Preview scrolling ---

/// Lines one mouse wheel notch scrolls the preview.
const WHEEL_STEP: i32 = 3;

/// The preview scroll offset after moving `delta` lines from `current`,
/// clamped to `0..=max`, where `max` is the last offset that still fills the
/// pane. Every preview scroll input (`J`/`K`, Shift+Down/Up, PgDn/PgUp, the
/// wheel) goes through here.
fn scrolled(current: u16, delta: i32, max: u16) -> u16 {
    let target = i64::from(current) + i64::from(delta);
    target.clamp(0, i64::from(max)) as u16
}

/// Lines PgDn/PgUp scroll a preview pane `height` lines tall: a page minus
/// one, so the line at the edge stays in view; at least one.
fn preview_page(height: u16) -> i32 {
    i32::from(height.saturating_sub(1).max(1))
}

// --- Preview rendering ---

/// How the preview shows a markdown note. Session state of one browser run:
/// remembered across selections, never saved. Other files ignore it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum PreviewMode {
    #[default]
    Rendered,
    Raw,
}

impl PreviewMode {
    /// The footer word of the toggle hint: the mode `p` switches to.
    fn toggle_desc(self) -> &'static str {
        match self {
            PreviewMode::Rendered => "raw",
            PreviewMode::Raw => "rendered",
        }
    }
}

/// Whether `path` is a markdown note: a `.md` extension, any case.
fn is_markdown(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
}

/// The language a file's extension maps to (`.md` is markdown).
fn file_language(path: &Path) -> Option<Language> {
    path.extension()
        .and_then(|ext| ext.to_str())
        .and_then(Language::from_extension)
}

/// Whether a file of `len` bytes is too large to highlight.
fn is_too_large_to_highlight(len: u64) -> bool {
    len > highlight::MAX_HIGHLIGHT_BYTES
}

/// The file type the footer names for a row: the highlighting language's
/// name when the extension maps to one (`rust`, `kotlin`, `markdown`),
/// otherwise the lowercase extension, `file` when there is none. A language
/// whose grammar failed to load reads `<name> (highlighter unavailable)`, a
/// file over the highlighting limit `<name> (not highlighted, large)`.
/// Folder and section rows have none.
fn file_type(path: &Path, is_dir: bool) -> Option<String> {
    if is_dir {
        return None;
    }
    if let Some(language) = file_language(path) {
        let name = language.name();
        if !highlight::language_available(language) {
            return Some(format!("{name} (highlighter unavailable)"));
        }
        let len = std::fs::metadata(path).map(|meta| meta.len()).unwrap_or(0);
        if is_too_large_to_highlight(len) {
            return Some(format!("{name} (not highlighted, large)"));
        }
        return Some(name.to_string());
    }
    let ext = path.extension().map(|ext| ext.to_string_lossy().to_lowercase());
    Some(ext.filter(|ext| !ext.is_empty()).unwrap_or_else(|| "file".to_string()))
}

/// What a cached file preview was built from. Any difference rebuilds it:
/// another file, a resize (`width`), the toggle (`rendered`), the file
/// being written (`modified`, `len`), or the language the whole file is
/// highlighted as (`language`, `None` for rendered markdown, plain text and
/// files over the limit).
#[derive(Debug, Clone, PartialEq, Eq)]
struct PreviewKey {
    path: PathBuf,
    width: u16,
    rendered: bool,
    modified: Option<std::time::SystemTime>,
    len: u64,
    language: Option<Language>,
}

impl PreviewKey {
    /// The key of `path` as it is on disk now; `None` when it cannot be read.
    fn of(path: &Path, width: u16, rendered: bool) -> Option<Self> {
        let meta = std::fs::metadata(path).ok()?;
        let len = meta.len();
        let language = file_language(path).filter(|_| !rendered && !is_too_large_to_highlight(len));
        Some(PreviewKey {
            path: path.to_path_buf(),
            width,
            rendered,
            modified: meta.modified().ok(),
            len,
            language,
        })
    }
}

/// The last file preview built, so a draw that changes none of its inputs
/// neither reads nor renders the file again.
#[derive(Default)]
struct PreviewCache {
    key: Option<PreviewKey>,
    lines: Vec<Line<'static>>,
}

impl PreviewCache {
    /// The lines for `key`, from `build` when `key` differs from the cached
    /// one or is `None` (a file that cannot be read is never cached).
    fn get(
        &mut self,
        key: Option<PreviewKey>,
        build: impl FnOnce() -> Vec<Line<'static>>,
    ) -> &[Line<'static>] {
        if key.is_none() || self.key != key {
            self.lines = build();
            self.key = key;
        }
        &self.lines
    }
}

/// The preview's rendering mode and its cached file lines.
#[derive(Default)]
struct Preview {
    mode: PreviewMode,
    cache: PreviewCache,
}

impl Preview {
    /// `p`: switch between rendered and raw, only while a markdown note is
    /// selected (`path`), since nothing else shows a difference.
    fn toggle(&mut self, path: Option<&Path>) {
        if path.is_some_and(is_markdown) {
            self.mode = match self.mode {
                PreviewMode::Rendered => PreviewMode::Raw,
                PreviewMode::Raw => PreviewMode::Rendered,
            };
        }
    }

    /// The preview lines of the file at `path` for a pane `width` columns
    /// wide inside its borders and padding: rendered markdown, already
    /// wrapped to `width`, or the raw lines unwrapped, highlighted when the
    /// extension maps to a language and the file is within the limit.
    fn file_lines(&mut self, path: &Path, width: u16) -> &[Line<'static>] {
        let rendered = self.mode == PreviewMode::Rendered && is_markdown(path);
        let key = PreviewKey::of(path, width, rendered);
        let language = key.as_ref().and_then(|key| key.language);
        self.cache.get(key, || file_preview_lines(path, rendered, width, language))
    }
}

/// Reads `path` and builds its preview lines; see [`Preview::file_lines`].
fn file_preview_lines(
    path: &Path,
    rendered: bool,
    width: u16,
    language: Option<Language>,
) -> Vec<Line<'static>> {
    match std::fs::read_to_string(path) {
        Ok(content) if rendered => markdown::render_markdown(&content, width),
        Ok(content) => match language {
            Some(language) => highlighted_preview_lines(&content, language),
            None => content.lines().map(raw_preview_line).collect(),
        },
        Err(_) => vec![Line::from(Span::styled(
            "  unable to read file",
            Style::default().fg(theme::OVERLAY),
        ))],
    }
}

/// The raw preview of `content` highlighted as `language`. A line no
/// capture touched looks exactly as [`raw_preview_line`] draws it; in a
/// touched line the captured text has its capture style and the rest takes
/// the line's raw style (so a markdown heading's text stays a heading).
fn highlighted_preview_lines(content: &str, language: Language) -> Vec<Line<'static>> {
    let highlighted = highlight::highlight_lines(content, language);
    if highlighted.len() != content.lines().count() {
        return content.lines().map(raw_preview_line).collect();
    }
    content
        .lines()
        .zip(highlighted)
        .map(|(text, spans)| {
            if spans.iter().all(|span| span.style == Style::default()) {
                return raw_preview_line(text);
            }
            let base = raw_line_style(text);
            let spans: Vec<Span<'static>> = spans
                .into_iter()
                .map(|span| {
                    if span.style == Style::default() {
                        span.style(base)
                    } else {
                        span
                    }
                })
                .collect();
            Line::from(spans)
        })
        .collect()
}

/// One line of the raw preview, coloured by its leading markdown syntax.
fn raw_preview_line(line: &str) -> Line<'static> {
    Line::from(Span::styled(line.to_string(), raw_line_style(line)))
}

/// The raw preview style of a line, by its leading markdown syntax.
fn raw_line_style(line: &str) -> Style {
    if line.starts_with('#') {
        Style::default().fg(theme::MAUVE).add_modifier(Modifier::BOLD)
    } else if line.starts_with("- [") {
        Style::default().fg(theme::SAPPHIRE)
    } else if line.starts_with("- ") || line.starts_with("* ") {
        Style::default().fg(theme::TEXT)
    } else {
        Style::default().fg(theme::SUBTEXT)
    }
}

/// The key of the preview toggle in `TREE_KEYS`.
const PREVIEW_TOGGLE_KEY: &str = "p";

/// `TREE_KEYS` with the preview toggle's footer hint filled in. `toggle` is
/// the preview mode while a markdown note is selected: the hint then shows
/// the mode `p` switches to and drops first when space runs out. Otherwise
/// the toggle stays in the help overlay only. Same rows in the same order as
/// `TREE_KEYS`, so the footer and the help overlay still agree.
fn tree_keys(toggle: Option<PreviewMode>) -> Vec<KeyHint> {
    TREE_KEYS
        .iter()
        .map(|hint| match toggle {
            Some(mode) if hint.key == PREVIEW_TOGGLE_KEY => KeyHint {
                desc: mode.toggle_desc(),
                slot: Slot::Priority(12),
                ..*hint
            },
            _ => *hint,
        })
        .collect()
}

/// The key of the `1`/`2` focus row in `TREE_KEYS`.
const PANE_FOCUS_KEY: &str = "1/2";

/// The key of the browse-mode `tab` row in `TREE_KEYS`.
const PANE_CYCLE_KEY: &str = "tab";

/// The footer rows for the focused pane. With the list focused, `keys`
/// unchanged. With the preview focused, only the preview set: `j/k scroll`,
/// `PgDn/PgUp page`, `2 fold`, `1/tab list`, the `p` toggle when `keys`
/// shows it (a markdown note), `?` and `q`; every other row is help only.
/// Same rows in the same order as `keys`, so the footer and the help
/// overlay still agree and help lists each key once.
fn pane_keys(keys: Vec<KeyHint>, focus: Pane) -> Vec<KeyHint> {
    if focus == Pane::List {
        return keys;
    }
    keys.into_iter()
        .map(|hint| {
            // Prompt rows keep their slots; only the browse footer changes.
            if !hint.applies_in(Mode::Normal) {
                return hint;
            }
            let preview_hint = |key, desc, priority| KeyHint { key, desc, slot: Slot::Priority(priority), ..hint };
            match (hint.key, hint.slot) {
                ("j/k", _) => preview_hint("j/k", "scroll", 1),
                ("PgDn/PgUp", _) => preview_hint("PgDn/PgUp", "page", 2),
                (PANE_FOCUS_KEY, _) => preview_hint("2", "fold", 3),
                (PANE_CYCLE_KEY, _) => preview_hint("1/tab", "list", 4),
                (PREVIEW_TOGGLE_KEY, Slot::Priority(_)) => KeyHint { slot: Slot::Priority(5), ..hint },
                (_, Slot::Pinned | Slot::Quit) => hint,
                _ => KeyHint { slot: Slot::HelpOnly, ..hint },
            }
        })
        .collect()
}

/// What a browse key does while the preview is focused, before the list's
/// own handling: scroll the preview by a number of lines, nothing, or the
/// same as with the list focused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PreviewFocusKey {
    Scroll(i32),
    Inert,
    Pass,
}

/// The preview-focused meaning of `code`; `page` is the PgDn/PgUp step.
/// `j`/`k`/Down/Up (Shift or not) scroll a line and PgDn/PgUp a page. The
/// keys that act on the list's cursor row or tree shape are inert: `h`,
/// `l`, Left, Right, Enter, `o` and Space. Everything else passes through
/// to the list's handling, and a key that opens a prompt moves focus there.
fn preview_focus_key(code: KeyCode, page: i32) -> PreviewFocusKey {
    match code {
        KeyCode::Char('j') | KeyCode::Down => PreviewFocusKey::Scroll(1),
        KeyCode::Char('k') | KeyCode::Up => PreviewFocusKey::Scroll(-1),
        KeyCode::PageDown => PreviewFocusKey::Scroll(page),
        KeyCode::PageUp => PreviewFocusKey::Scroll(-page),
        KeyCode::Char('h' | 'l' | 'o' | ' ') | KeyCode::Left | KeyCode::Right | KeyCode::Enter => {
            PreviewFocusKey::Inert
        }
        _ => PreviewFocusKey::Pass,
    }
}

/// The title of a pane: its number (the key that focuses it) in the pane
/// number style, then `rest`.
fn pane_title(pane: Pane, focus: Pane, rest: Vec<Span<'static>>) -> Line<'static> {
    let mut spans = vec![Span::styled(format!(" {} ", pane.number()), theme::pane_number(pane == focus))];
    spans.extend(rest);
    Line::from(spans)
}

/// The border style of `pane`: highlighted while it has focus.
fn pane_border(pane: Pane, focus: Pane) -> Style {
    if pane == focus { theme::border_focused() } else { theme::border() }
}

/// The browsing footer: the selected file's type and the mark count, each
/// when there is one, then the `table` hints for `mode` that fit after them.
fn browse_footer(
    table: &[KeyHint],
    file_type: Option<&str>,
    marks: usize,
    mode: Mode,
    on: &[Toggle],
    width: usize,
) -> Line<'static> {
    let mut lead = Vec::new();
    if let Some(file_type) = file_type {
        lead.push(Span::styled(format!(" {file_type} "), Style::default().fg(theme::SAPPHIRE)));
    }
    if marks > 0 {
        lead.extend(marked_lead(marks));
    }
    footer::status_line(table, lead, true, mode, on, Vec::new(), width)
}

// --- Keys: one table for the footer and the help overlay ---

const BROWSE: &[Mode] = &[Mode::Normal, Mode::Focus];
const BROWSE_AND_TAG: &[Mode] = &[Mode::Normal, Mode::Focus, Mode::Tag];
const FILTERING: &[Mode] = &[Mode::Filter];
const TAGGING: &[Mode] = &[Mode::Tag];
const RENAMING: &[Mode] = &[Mode::Rename];
const NEW_NOTE: &[Mode] = &[Mode::NewItem];
const CONFIRMING: &[Mode] = &[Mode::ConfirmDelete];
const COMMAND: &[Mode] = &[Mode::VimCommand];
const MOVING: &[Mode] = &[Mode::Move];
const SETTING_SCOPE: &[Mode] = &[Mode::SetScope];
const CONFIRMING_MOVE: &[Mode] = &[Mode::ConfirmMove];

const fn key(
    key: &'static str,
    desc: &'static str,
    help: &'static str,
    color: Color,
    group: Group,
    modes: &'static [Mode],
    slot: Slot,
    toggle: Option<Toggle>,
) -> KeyHint {
    KeyHint { key, desc, help, color, group, modes, slot, toggle }
}

/// Every key, mouse action and command the tree browser handles, in footer
/// order. Kept in step with `event_loop`.
const TREE_KEYS: &[KeyHint] = &[
    key("j/k", "move", "move down / up (also Down / Up); with the preview focused, scroll it", theme::TEXT, Group::Navigate, BROWSE_AND_TAG, Slot::HelpOnly, None),
    key("l", "expand", "expand directory (also Right)", theme::MAUVE, Group::Navigate, BROWSE_AND_TAG, Slot::HelpOnly, None),
    key("h", "collapse", "collapse directory / go to parent (also Left)", theme::MAUVE, Group::Navigate, BROWSE_AND_TAG, Slot::HelpOnly, None),
    key("wheel", "scroll", "mouse wheel scrolls the pane under the pointer: the preview, or the list's cursor", theme::TEXT, Group::Navigate, BROWSE, Slot::HelpOnly, None),
    key("click", "select", "click a pane to focus it; a row to select it and toggle a directory", theme::TEXT, Group::Navigate, BROWSE, Slot::HelpOnly, None),
    key("drag", "split", "drag the grip on the border between the panes to resize them", theme::LAVENDER, Group::View, BROWSE, Slot::HelpOnly, None),
    key("o", "open", "open file / toggle directory (also Enter)", theme::GREEN, Group::Edit, BROWSE, Slot::Priority(1), None),
    key("t", "tags", "tag mode on / off", theme::PEACH, Group::Edit, BROWSE_AND_TAG, Slot::Priority(2), Some(Toggle::Tag)),
    key("r", "rename", "rename note or folder", theme::MAUVE, Group::Edit, BROWSE, Slot::Priority(6), None),
    key("1-5", "toggle", "tag mode: toggle tag 1 to 5 on the note", theme::PEACH, Group::Edit, TAGGING, Slot::Priority(1), None),
    key("esc", "close", "tag mode: close", theme::PEACH, Group::Edit, TAGGING, Slot::Priority(3), None),
    key("click dot", "tag", "click a note's tag dot to toggle that tag", theme::PEACH, Group::Edit, BROWSE, Slot::HelpOnly, None),
    key("enter", "confirm", "rename: confirm", theme::GREEN, Group::Edit, RENAMING, Slot::Priority(1), None),
    key("esc", "cancel", "rename: cancel", theme::PEACH, Group::Edit, RENAMING, Slot::Priority(2), None),
    key("bksp", "delete", "rename: delete the last char", theme::TEXT, Group::Edit, RENAMING, Slot::Priority(3), None),
    key("n", "new", "new note in the folder under the cursor (the prompt names the scope)", theme::GREEN, Group::Edit, BROWSE, Slot::Priority(3), None),
    key("N", "folder", "new folder in the folder under the cursor (the prompt names the scope)", theme::GREEN, Group::Edit, BROWSE, Slot::Priority(8), None),
    key("m", "move", "move note or folder", theme::MAUVE, Group::Edit, BROWSE, Slot::Priority(9), None),
    key("S", "scope", "set scope", theme::MAUVE, Group::Edit, BROWSE, Slot::Priority(10), None),
    key("enter", "create", "new note or folder: create it (a note opens in the editor)", theme::GREEN, Group::Edit, NEW_NOTE, Slot::Priority(1), None),
    key("esc", "cancel", "new note or folder: cancel, nothing is created", theme::PEACH, Group::Edit, NEW_NOTE, Slot::Priority(2), None),
    key("tab", "scope", "new note or folder: next scope (personal, public, local, global), at its root", theme::SAPPHIRE, Group::Edit, NEW_NOTE, Slot::Priority(3), None),
    key("bksp", "delete", "new note or folder: delete the last char", theme::TEXT, Group::Edit, NEW_NOTE, Slot::Priority(4), None),
    key("space", "mark", "mark", theme::LAVENDER, Group::Edit, BROWSE, Slot::Priority(8), None),
    key("d", "delete", "delete note or folder under the cursor (asks first; no undo)", theme::RED, Group::Edit, BROWSE, Slot::Priority(7), None),
    key("y", "confirm", "delete: yes, delete it", theme::RED, Group::Edit, CONFIRMING, Slot::Priority(1), None),
    key("n/esc", "cancel", "delete: cancel (any other key too), nothing is deleted", theme::PEACH, Group::Edit, CONFIRMING, Slot::Priority(2), None),
    key("enter", "move", "move: move it to the typed folder (a new scope asks first)", theme::GREEN, Group::Edit, MOVING, Slot::Priority(1), None),
    key("esc", "cancel", "move: cancel, nothing moves", theme::PEACH, Group::Edit, MOVING, Slot::Priority(2), None),
    key("tab", "scope", "move: next scope (personal, public, local, global), the typed folder stays", theme::SAPPHIRE, Group::Edit, MOVING, Slot::Priority(3), None),
    key("bksp", "delete", "move: delete the last char", theme::TEXT, Group::Edit, MOVING, Slot::Priority(4), None),
    key("enter", "apply", "set scope: move it to the same folder in the shown scope (asks first)", theme::GREEN, Group::Edit, SETTING_SCOPE, Slot::Priority(1), None),
    key("esc", "cancel", "set scope: cancel, nothing moves", theme::PEACH, Group::Edit, SETTING_SCOPE, Slot::Priority(2), None),
    key("tab", "scope", "set scope: next scope (personal, public, local, global)", theme::SAPPHIRE, Group::Edit, SETTING_SCOPE, Slot::Priority(3), None),
    key("y", "confirm", "move to another scope: yes, move it", theme::RED, Group::Edit, CONFIRMING_MOVE, Slot::Priority(1), None),
    key("n/esc", "cancel", "move to another scope: cancel (any other key too), nothing moves", theme::PEACH, Group::Edit, CONFIRMING_MOVE, Slot::Priority(2), None),
    key("/", "filter", "filter: text and #tag (starts a new filter)", theme::YELLOW, Group::Filter, BROWSE_AND_TAG, Slot::Priority(4), Some(Toggle::Filter)),
    key("enter", "keep", "filter: keep the filter, back to the list", theme::GREEN, Group::Filter, FILTERING, Slot::Priority(1), None),
    key("esc", "clear", "filter: clear it and close", theme::PEACH, Group::Filter, FILTERING, Slot::Priority(2), None),
    key("\u{2190}/\u{2192}", "cursor", "filter: move the cursor", theme::TEXT, Group::Filter, FILTERING, Slot::Priority(3), None),
    key("bksp", "delete", "filter: delete the char before the cursor; at the start, clear the filter and close", theme::TEXT, Group::Filter, FILTERING, Slot::Priority(4), None),
    key("esc", "clear", "clear marks; with none, clear the filter",theme::PEACH, Group::Filter, BROWSE, Slot::HelpOnly, None),
    key("click bar", "filter", "click the filter bar to filter, a dot to filter by that tag", theme::YELLOW, Group::Filter, BROWSE, Slot::HelpOnly, None),
    key("f", "focus", "focus the current section (again to leave)", theme::GREEN, Group::View, BROWSE, Slot::Priority(3), Some(Toggle::Focus)),
    key("v", "view all", "expand all / collapse all sections", theme::SAPPHIRE, Group::View, BROWSE, Slot::Priority(5), Some(Toggle::ExpandAll)),
    key("R", "reload", "reload the tree from disk (it also reloads by itself, within 2 s of idle, when a shown folder changes)", theme::SAPPHIRE, Group::View, BROWSE, Slot::HelpOnly, None),
    // In the footer only while a markdown note is selected; see `tree_keys`.
    key(PREVIEW_TOGGLE_KEY, "raw", "toggle rendered / raw preview", theme::SAPPHIRE, Group::View, BROWSE, Slot::HelpOnly, None),
    key("?", "help", "this help (? or esc closes)", theme::MAUVE, Group::View, BROWSE, Slot::Pinned, Some(Toggle::Help)),
    key(":q", "quit", "vim-style quit (also :wq, :qa, :q!)", theme::MAUVE, Group::View, BROWSE, Slot::HelpOnly, None),
    key("enter", "run", ":command: run it", theme::GREEN, Group::View, COMMAND, Slot::Priority(1), None),
    key("esc", "cancel", ":command: close the command line, nothing else", theme::PEACH, Group::View, COMMAND, Slot::Priority(2), None),
    key("bksp", "delete", ":command: delete the last char; deleting the : closes it", theme::TEXT, Group::View, COMMAND, Slot::Priority(3), None),
    // Navigate keys, then the pane keys, listed last so they end the
    // footer's hints. The pane keys drop first, then "J/K preview".
    key("J/K", "preview", "scroll preview down / up (also Shift+Down/Up)", theme::TEXT, Group::Navigate, BROWSE, Slot::Priority(11), None),
    key("PgDn/PgUp", "page", "scroll preview a page", theme::TEXT, Group::Navigate, BROWSE, Slot::HelpOnly, None),
    // With the preview focused, `pane_keys` swaps in the preview footer.
    key(PANE_FOCUS_KEY, "focus", "focus the list / the preview; 2 on the focused preview folds it, 2 on a folded one unfolds and focuses it", theme::LAVENDER, Group::View, BROWSE, Slot::Priority(13), None),
    key(PANE_CYCLE_KEY, "pane", "focus the other pane (in a prompt, tab cycles the scope instead)", theme::LAVENDER, Group::View, BROWSE, Slot::Priority(14), None),
    key("</>", "split", "narrow / widen the list by 5 points (both panes keep a usable width)", theme::LAVENDER, Group::View, BROWSE, Slot::Priority(15), None),
    key("=", "reset", "reset the split to 50/50", theme::LAVENDER, Group::View, BROWSE, Slot::Priority(16), None),
    key("q", "quit", "quit (also :q)", theme::PEACH, Group::View, BROWSE, Slot::Quit, None),
];

/// The footer mode for the tree's input state, most specific first.
fn footer_mode(
    renaming: bool,
    vim_active: bool,
    search_mode: bool,
    flag_mode: bool,
    focus_active: bool,
) -> Mode {
    if renaming {
        Mode::Rename
    } else if vim_active {
        Mode::VimCommand
    } else if search_mode {
        Mode::Filter
    } else if flag_mode {
        Mode::Tag
    } else if focus_active {
        Mode::Focus
    } else {
        Mode::Normal
    }
}

/// Toggles that are on; their keys are lit in the footer.
fn footer_toggles(
    focus_active: bool,
    filter_on: bool,
    flag_mode: bool,
    all_expanded: bool,
    help_open: bool,
) -> Vec<Toggle> {
    [
        (focus_active, Toggle::Focus),
        (filter_on, Toggle::Filter),
        (flag_mode, Toggle::Tag),
        (all_expanded, Toggle::ExpandAll),
        (help_open, Toggle::Help),
    ]
    .into_iter()
    .filter_map(|(on, toggle)| on.then_some(toggle))
    .collect()
}

/// True when some top-level section is collapsed: `v` then expands all,
/// otherwise it collapses all, and its footer hint is lit.
fn any_top_collapsed(nodes: &[TreeNode]) -> bool {
    nodes.iter().any(|n| n.is_dir && n.depth == 0 && !n.expanded)
}

/// Whether `v` is lit: at least one top-level directory exists and none is
/// collapsed. The `v` key itself only checks `any_top_collapsed`.
fn view_all_lit(nodes: &[TreeNode]) -> bool {
    nodes.iter().any(|n| n.is_dir && n.depth == 0) && !any_top_collapsed(nodes)
}

// --- Event loop ---

#[allow(clippy::too_many_lines)]
fn event_loop(
    terminal: &mut super::TuiTerminal,
    forest: &mut Forest,
    retired: &mut Vec<(PathBuf, String)>,
    carried: &mut Vec<CarriedTags>,
    ctx: &TreeContext,
    config: &Config,
    rebuild: &dyn Fn() -> Result<Vec<SectionSpec>>,
) -> Result<()> {
    let mut new_note: Option<NewNotePrompt> = None;
    let mut confirm_delete: Option<DeletePrompt> = None;
    let mut confirm_bulk_delete: Option<Vec<BulkItem>> = None;
    let mut marks: HashSet<PathBuf> = HashSet::new();
    let mut move_prompt: Option<MovePrompt> = None;
    // With rows marked, `move_prompt` is the shared prompt and this holds
    // one prompt per item of the set.
    let mut move_set: Option<Vec<MovePrompt>> = None;
    let mut confirm_move: Option<MovePlan> = None;
    let mut confirm_bulk_move: Option<BulkMovePlan> = None;
    let mut state = ListState::default();
    state.select(Some(0));
    let mut vim = VimCommandMode::new();
    let mut search_mode = false;
    let mut search_buffer = String::new();
    let mut cursor_pos: usize = 0;
    let mut focus_active = false;
    let mut pre_focus_expanded: Vec<(usize, bool)> = Vec::new();
    let mut help = HelpState::default();
    let mut flag_mode = false;
    let mut rename_buffer: Option<String> = None;
    // The text the open rename prompt was prefilled with: Enter on it
    // unchanged renames nothing.
    let mut rename_shown = String::new();
    let mut status_message: Option<String> = None;
    let mut preview_scroll: u16 = 0;
    // Set on every draw: the last scroll offset that still fills the preview
    // pane, and the pane's inner height, for the scroll keys.
    let mut preview_max: u16 = 0;
    let mut preview_height: u16 = 0;
    let mut last_preview_idx: usize = usize::MAX;
    let mut preview = Preview::default();
    // The split, focus, fold and drag; session only. `body_area` is the rect
    // the two panes shared at the last draw, for the split keys' clamp and
    // the mouse hit tests.
    let mut panes = Panes::default();
    let mut body_area: Rect = Rect::default();
    let mut filter_strip_area: Rect = Rect::default();
    let mut list_inner_area: Rect = Rect::default();
    let mut visible_for_mouse: Vec<usize> = Vec::new();
    let mut prev_filter_buffer = String::new();
    // The file-change probe's last reading; `None` until the first probe,
    // which only records it. Paths no longer probed keep their entry, so a
    // folder expanded again compares against its last reading.
    let mut probe: Option<ProbeSnapshot> = None;

    loop {
        let nodes = &mut forest.nodes;
        let sections = &forest.sections;
        derive_dir_flags(nodes);
        prune_marks(&mut marks, nodes);

        // Auto-expand directories that contain filter matches whenever the
        // filter changes; matches inside collapsed dirs would stay hidden.
        let f = filter::parse(&search_buffer);
        if !f.is_empty() && search_buffer != prev_filter_buffer {
            let keep = compute_filter_keep(nodes, &f);
            for (i, k) in keep.iter().enumerate() {
                if *k && nodes[i].is_dir {
                    nodes[i].expanded = true;
                }
            }
        }
        prev_filter_buffer = search_buffer.clone();

        let visible = compute_visible(nodes, &search_buffer);
        let sel = state.selected().unwrap_or(0);
        let real_idx = visible.get(sel).copied().unwrap_or(0);

        // Prompts, confirms, the filter, tag mode and the `:` line belong to
        // the list: opening one with the preview focused moves focus there.
        let input_open = new_note.is_some()
            || rename_buffer.is_some()
            || move_prompt.is_some()
            || confirm_delete.is_some()
            || confirm_bulk_delete.is_some()
            || confirm_move.is_some()
            || confirm_bulk_move.is_some()
            || search_mode
            || flag_mode
            || vim.active;
        if input_open {
            panes.focus(Pane::List);
        }

        terminal
            .draw(|frame| {
                let full = frame.area();
                let area = Rect::new(
                    full.x + 2,
                    full.y + 1,
                    full.width.saturating_sub(4),
                    full.height.saturating_sub(2),
                );
                let rows = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(1), Constraint::Length(1)])
                    .split(area);
                // Too narrow for both panes: the preview folds on its own.
                panes.fit(rows[0].width);
                // The border strip between the panes carries only the grip.
                let (list_area, _, preview_area) = panes.layout(rows[0]);
                body_area = rows[0];
                let inner_width = list_text_width(list_area.width);

                let items: Vec<ListItem> = list_lines(nodes, sections, &visible, &marks, inner_width)
                    .into_iter()
                    .map(ListItem::new)
                    .collect();

                let header = pane_title(Pane::List, panes.focus, vec![
                    Span::styled(
                        format!("{} ", ctx.title),
                        Style::default()
                            .fg(theme::LAVENDER)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled("- ", Style::default().fg(theme::SURFACE)),
                    Span::styled(
                        format!("{} ", ctx.path_display),
                        Style::default().fg(theme::OVERLAY),
                    ),
                ]);

                // Filter strip (mirrors the todoz board).
                let active_tags = filter::parse(&search_buffer)
                    .tag_sets
                    .iter()
                    .fold(0u8, |a, s| a | s);
                let mut filter_spans: Vec<Span> = vec![Span::raw("     ")];
                for (i, def) in FLAG_DEFS.iter().enumerate() {
                    let style = if active_tags & def.bit != 0 {
                        Style::default()
                            .fg(theme::FLAG_COLORS[i])
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(theme::dim_color(theme::FLAG_COLORS[i]))
                    };
                    filter_spans.push(Span::styled("●", style));
                }
                filter_spans.push(Span::raw("  "));
                if search_mode {
                    let (before, after) =
                        search_buffer.split_at(cursor_pos.min(search_buffer.len()));
                    let cursor_char = after
                        .chars()
                        .next()
                        .map(|c| c.to_string())
                        .unwrap_or_else(|| " ".to_string());
                    let rest = if after.len() > cursor_char.len() {
                        &after[cursor_char.len()..]
                    } else {
                        ""
                    };
                    filter_spans.push(Span::styled("/", Style::default().fg(theme::YELLOW)));
                    filter_spans.push(Span::styled(
                        before.to_string(),
                        Style::default().fg(theme::TEXT),
                    ));
                    filter_spans.push(Span::styled(
                        cursor_char,
                        Style::default().fg(theme::BASE).bg(theme::SAPPHIRE),
                    ));
                    filter_spans.push(Span::styled(
                        rest.to_string(),
                        Style::default().fg(theme::TEXT),
                    ));
                    if search_buffer.is_empty() {
                        filter_spans.push(Span::styled(
                            "  text + #tag or click a dot",
                            Style::default().fg(Color::Rgb(80, 80, 95)),
                        ));
                    }
                } else if !search_buffer.is_empty() {
                    filter_spans.push(Span::styled("/", Style::default().fg(theme::YELLOW)));
                    for word in search_buffer.split(' ') {
                        if word.is_empty() {
                            continue;
                        }
                        let mut tag_color: Option<Color> = None;
                        if let Some(name) = word.strip_prefix('#') {
                            for (idx, def) in FLAG_DEFS.iter().enumerate() {
                                if def.key.eq_ignore_ascii_case(name) {
                                    tag_color = Some(theme::FLAG_COLORS[idx]);
                                    break;
                                }
                            }
                        }
                        let style = match tag_color {
                            Some(c) => {
                                Style::default().fg(c).add_modifier(Modifier::BOLD)
                            }
                            None => Style::default().fg(theme::YELLOW),
                        };
                        filter_spans.push(Span::styled(format!("{} ", word), style));
                    }
                    filter_spans.push(Span::styled(
                        " esc to clear ",
                        Style::default().fg(theme::OVERLAY),
                    ));
                } else {
                    filter_spans.push(Span::styled("/", Style::default().fg(theme::YELLOW)));
                    filter_spans.push(Span::styled(
                        "filter",
                        Style::default().fg(theme::OVERLAY),
                    ));
                }

                let block = list_block()
                    .title(header)
                    .border_style(pane_border(Pane::List, panes.focus));
                let inner_chunks = list_chunks(list_area);
                filter_strip_area = inner_chunks[0];
                list_inner_area = inner_chunks[2];
                visible_for_mouse = visible.clone();

                frame.render_widget(block, list_area);
                frame.render_widget(
                    Paragraph::new(Line::from(filter_spans)),
                    inner_chunks[0],
                );
                frame.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        "─".repeat(inner_chunks[1].width as usize),
                        Style::default().fg(Color::Rgb(50, 50, 65)),
                    ))),
                    inner_chunks[1],
                );

                if empty_state_line(nodes).is_some() {
                    frame.render_widget(
                        Paragraph::new(Line::from(Span::styled(
                            format!("  {}", empty_state_text(&ctx.title)),
                            Style::default().fg(theme::OVERLAY),
                        ))),
                        inner_chunks[2],
                    );
                } else {
                    let list = List::new(items).highlight_style(theme::selected_row());
                    frame.render_stateful_widget(list, inner_chunks[2], &mut state);
                }

                if real_idx != last_preview_idx {
                    preview_scroll = 0;
                    last_preview_idx = real_idx;
                }

                // A folded preview is neither read nor drawn; its cache entry
                // stays for the unfold. Nothing is left to scroll meanwhile.
                if let Some(preview_area) = preview_area {
                    // The pane number replaces the dots' leading space.
                    let flags = if real_idx < nodes.len() { nodes[real_idx].flags } else { 0 };
                    let mut preview_title_spans: Vec<Span<'static>> =
                        flags_slots(flags).into_iter().skip(1).collect();
                    preview_title_spans.push(Span::styled(
                        if real_idx < nodes.len() {
                            format!("{} ", nodes[real_idx].name)
                        } else {
                            String::new()
                        },
                        Style::default().fg(theme::OVERLAY),
                    ));
                    let real_path_display = if real_idx < nodes.len() {
                        format!(
                            " {} ",
                            notez_core::util::tilde::contract(&nodes[real_idx].path)
                        )
                    } else {
                        String::new()
                    };
                    let preview_block = Block::default()
                        .title(pane_title(Pane::Preview, panes.focus, preview_title_spans))
                        .title_bottom(Line::from(Span::styled(
                            real_path_display,
                            Style::default().fg(theme::OVERLAY),
                        )))
                        .borders(Borders::ALL)
                        .border_style(pane_border(Pane::Preview, panes.focus))
                        .border_type(ratatui::widgets::BorderType::Rounded)
                        .padding(Padding::new(1, 1, 0, 0));
                    let preview_width = preview_block.inner(preview_area).width;

                    // Preview pane: file content (cached), or a directory listing.
                    let dir_lines: Vec<Line>;
                    let preview_lines: &[Line] = if real_idx < nodes.len()
                        && !nodes[real_idx].is_dir
                    {
                        preview.file_lines(&nodes[real_idx].path, preview_width)
                    } else {
                        dir_lines = if real_idx < nodes.len() {
                            match std::fs::read_dir(&nodes[real_idx].path) {
                                Ok(entries) => {
                                    // Match the tree rows: infrastructure dotfiles
                                    // (.git, .tags, .notez-config.toml) are not notes.
                                    let mut names: Vec<String> = entries
                                        .flatten()
                                        .map(|e| e.file_name().to_string_lossy().to_string())
                                        .filter(|n| !n.starts_with('.'))
                                        .collect();
                                    names.sort();
                                    names
                                        .iter()
                                        .map(|n| {
                                            let color = if n.ends_with(".md") {
                                                theme::TEXT
                                            } else {
                                                theme::SAPPHIRE
                                            };
                                            Line::from(Span::styled(
                                                format!("  {}", n),
                                                Style::default().fg(color),
                                            ))
                                        })
                                        .collect()
                                }
                                Err(_) => vec![],
                            }
                        } else {
                            vec![]
                        };
                        &dir_lines
                    };

                    // Past u16::MAX lines the scroll offset cannot reach anyway.
                    let total_lines = u16::try_from(preview_lines.len()).unwrap_or(u16::MAX);
                    preview_height = preview_area.height.saturating_sub(2);
                    preview_max = total_lines.saturating_sub(preview_height);
                    preview_scroll = scrolled(preview_scroll, 0, preview_max);

                    // Only the visible lines are handed over, so a long cached
                    // preview is not copied whole on every frame. Same picture
                    // as scrolling the whole text: nothing wraps here.
                    let first = usize::from(preview_scroll).min(preview_lines.len());
                    let last = (first + usize::from(preview_height)).min(preview_lines.len());
                    frame.render_widget(
                        Paragraph::new(preview_lines[first..last].to_vec()).block(preview_block),
                        preview_area,
                    );
                } else {
                    preview_height = 0;
                    preview_max = 0;
                }

                // The grip on the border strip, from the same layout the
                // drag hit test uses; lit while it is being dragged.
                if let Some(grip) = panes.grip(rows[0]) {
                    for y in grip.y..grip.y + grip.height {
                        if let Some(cell) = frame.buffer_mut().cell_mut((grip.x, y)) {
                            cell.set_symbol(panes::GRIP);
                            cell.set_style(theme::grip(panes.dragging));
                        }
                    }
                }

                // Status bar.
                let slot = status_slot(
                    rename_buffer.as_deref(),
                    status_message.as_deref(),
                    vim.active,
                    flag_mode,
                    ctx.warning.as_deref(),
                );
                let width = area.width as usize;
                let toggles = footer_toggles(
                    focus_active,
                    search_mode || !search_buffer.is_empty(),
                    flag_mode,
                    view_all_lit(nodes),
                    help.open,
                );
                // The selected file's type and, for a markdown note, the
                // preview toggle lead and join the browsing hints.
                let selected_file =
                    nodes.get(real_idx).filter(|n| !n.is_dir).map(|n| n.path.as_path());
                let row_type = selected_file.and_then(|path| file_type(path, false));
                let keys = pane_keys(
                    tree_keys(selected_file.filter(|p| is_markdown(p)).map(|_| preview.mode)),
                    panes.focus,
                );
                let status = match slot {
                    _ if confirm_delete.is_some() => {
                        let prompt = confirm_delete.as_ref().expect("checked by the guard");
                        lead_with_hints(delete_lead(prompt), Mode::ConfirmDelete, &toggles, width)
                    }
                    _ if confirm_bulk_delete.is_some() => {
                        let items = confirm_bulk_delete.as_deref().expect("checked by the guard");
                        lead_with_hints(bulk_delete_lead(items), Mode::ConfirmDelete, &toggles, width)
                    }
                    _ if confirm_move.is_some() => {
                        let plan = confirm_move.as_ref().expect("checked by the guard");
                        lead_with_hints(move_confirm_lead(plan), Mode::ConfirmMove, &toggles, width)
                    }
                    _ if confirm_bulk_move.is_some() => {
                        let plan = confirm_bulk_move.as_ref().expect("checked by the guard");
                        lead_with_hints(bulk_move_confirm_lead(plan), Mode::ConfirmMove, &toggles, width)
                    }
                    _ if move_prompt.is_some() => {
                        let prompt = move_prompt.as_ref().expect("checked by the guard");
                        let mode = if prompt.fixed { Mode::SetScope } else { Mode::Move };
                        if let Some(items) = &move_set {
                            lead_with_hints(bulk_move_lead(prompt, items.len()), mode, &toggles, width)
                        } else if prompt.fixed {
                            lead_with_hints(set_scope_lead(prompt), Mode::SetScope, &toggles, width)
                        } else {
                            lead_with_hints(move_lead(prompt), Mode::Move, &toggles, width)
                        }
                    }
                    // A refused name's message shows in place of the prompt.
                    _ if new_note.is_some() && status_message.is_none() => {
                        let prompt = new_note.as_ref().expect("checked by the guard");
                        let lead = if prompt.is_folder {
                            new_folder_lead(&prompt.target.label, &prompt.buffer)
                        } else {
                            new_note_lead(&prompt.target.label, &prompt.buffer)
                        };
                        lead_with_hints(lead, Mode::NewItem, &toggles, width)
                    }
                    StatusSlot::Rename(buffer) => {
                        lead_with_hints(rename_lead(buffer), Mode::Rename, &toggles, width)
                    }
                    StatusSlot::Message(message) => Line::from(Span::styled(
                        format!(" {message}"),
                        Style::default().fg(theme::PEACH),
                    )),
                    StatusSlot::VimCommand => lead_with_hints(
                        command_lead(&vim.buffer),
                        Mode::VimCommand,
                        &toggles,
                        width,
                    ),
                    StatusSlot::Tags => {
                        let cur_flags = if real_idx < nodes.len() {
                            nodes[real_idx].flags
                        } else {
                            0
                        };
                        lead_with_hints(tag_legend(cur_flags), Mode::Tag, &toggles, width)
                    }
                    // While rows are marked the count leads the hints, over
                    // the session warning, which comes back once they clear.
                    StatusSlot::Warning(_) | StatusSlot::Hints if !marks.is_empty() => {
                        let mode = footer_mode(
                            rename_buffer.is_some(),
                            vim.active,
                            search_mode,
                            flag_mode,
                            focus_active,
                        );
                        browse_footer(&keys, row_type.as_deref(), marks.len(), mode, &toggles, width)
                    }
                    StatusSlot::Warning(warning) => {
                        let (text, padding) = warning_layout(warning, area.width as usize);
                        Line::from(vec![
                            Span::styled(
                                " ! ",
                                Style::default().fg(theme::RED).add_modifier(Modifier::BOLD),
                            ),
                            Span::styled(text, Style::default().fg(theme::YELLOW)),
                            Span::raw(" ".repeat(padding)),
                            Span::styled(
                                "q",
                                Style::default().fg(theme::PEACH).add_modifier(Modifier::BOLD),
                            ),
                            Span::styled("uit ", Style::default().fg(theme::OVERLAY)),
                        ])
                    }
                    StatusSlot::Hints => {
                        let mode = footer_mode(
                            rename_buffer.is_some(),
                            vim.active,
                            search_mode,
                            flag_mode,
                            focus_active,
                        );
                        browse_footer(&keys, row_type.as_deref(), 0, mode, &toggles, width)
                    }
                };
                frame.render_widget(Paragraph::new(status), rows[1]);

                if help.open {
                    help::render(frame, full, TREE_KEYS, &mut help);
                }
            })
            .context("failed to draw")?;

        // Wait for input. Each `PROBE_INTERVAL` without any, probe the disk
        // unless a prompt, mode, the help overlay or a drag is open; only a
        // change ends the wait without an event, to reload and redraw.
        let can_probe = !(input_open || help.open || panes.dragging);
        let ev = loop {
            if event::poll(PROBE_INTERVAL).context("failed to poll for events")? {
                break Some(event::read().context("failed to read event")?);
            }
            if !can_probe {
                continue;
            }
            let reading = probe_snapshot(&probe_paths(sections, nodes));
            let Some(last) = probe.as_mut() else {
                probe = Some(reading);
                continue;
            };
            let changed = probe_changed(last, &reading);
            last.extend(reading);
            if changed {
                break None;
            }
        };
        let Some(ev) = ev else {
            match reload_view(forest, rebuild, &mut state, &mut pre_focus_expanded, &search_buffer) {
                Ok(kept) => {
                    if let Some(row) = kept {
                        last_preview_idx = row;
                    }
                    // A message already showing (an action's outcome) stays.
                    if status_message.is_none() {
                        status_message = Some("reloaded (files changed)".to_string());
                    }
                }
                Err(message) => status_message = Some(message),
            }
            continue;
        };

        if let Event::Mouse(mouse) = ev {
            // A drag owns every mouse event until the button comes up,
            // wherever the pointer wanders. Anything but a drag ends it: a
            // release outside the terminal can swallow the Up event.
            if panes.dragging {
                match mouse.kind {
                    MouseEventKind::Drag(MouseButton::Left) => panes.drag_to(body_area, mouse.column),
                    _ => panes.dragging = false,
                }
                continue;
            }
            match mouse.kind {
                // The wheel scrolls the pane under the pointer: the preview
                // by `WHEEL_STEP` lines, the list by moving the cursor a row,
                // as `j`/`k` do (not while a prompt or mode owns the list).
                MouseEventKind::ScrollDown | MouseEventKind::ScrollUp => {
                    let down = mouse.kind == MouseEventKind::ScrollDown;
                    match panes.pane_at(body_area, mouse.column, mouse.row) {
                        Some(Pane::Preview) => {
                            let delta = if down { WHEEL_STEP } else { -WHEEL_STEP };
                            preview_scroll = scrolled(preview_scroll, delta, preview_max);
                        }
                        Some(Pane::List) if !input_open => {
                            if down && sel + 1 < visible.len() {
                                navigate(nodes, &mut state, &visible, sel, real_idx, focus_active, 1);
                            } else if !down && sel > 0 && sel < visible.len() {
                                navigate(nodes, &mut state, &visible, sel, real_idx, focus_active, -1);
                            }
                        }
                        _ => {}
                    }
                }
                // Grabbing the border starts a drag before any other click
                // handling. Otherwise the click focuses the pane under it; a
                // preview click does nothing else, a list click goes on to
                // the filter strip, the rows and the tag dots.
                MouseEventKind::Down(MouseButton::Left) => {
                    if panes.press(body_area, mouse.column, mouse.row) != Press::Pane(Pane::List) {
                        continue;
                    }
                    if mouse.row == filter_strip_area.y {
                        if let Some(d) = mouse_x_to_dot(mouse.column, filter_strip_area.x) {
                            search_buffer = filter::toggle_tag_in_buffer(
                                &search_buffer,
                                d as usize,
                            );
                            cursor_pos = search_buffer.len();
                            continue;
                        }
                        search_mode = true;
                        cursor_pos = search_buffer.len();
                        continue;
                    }
                    if mouse.row >= list_inner_area.y
                        && mouse.row < list_inner_area.y.saturating_add(list_inner_area.height)
                        && mouse.column >= list_inner_area.x
                        && mouse.column
                            < list_inner_area.x.saturating_add(list_inner_area.width)
                    {
                        let list_row = (mouse.row - list_inner_area.y) as usize;
                        let vis_idx = state.offset() + list_row;
                        if let Some(&real) = visible_for_mouse.get(vis_idx) {
                            state.select(Some(vis_idx));
                            if !nodes[real].is_dir {
                                if let Some(tag) = mouse_x_to_row_tag(
                                    mouse.column,
                                    list_inner_area.x,
                                    nodes[real].flags,
                                ) {
                                    nodes[real].flags ^= FLAG_DEFS[tag].bit;
                                    continue;
                                }
                            }
                            if nodes[real].is_dir {
                                nodes[real].expanded = !nodes[real].expanded;
                            }
                        }
                    }
                }
                _ => {}
            }
            continue;
        }

        let Event::Key(key) = ev else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }

        if key.code == KeyCode::Char('c')
            && key
                .modifiers
                .contains(crossterm::event::KeyModifiers::CONTROL)
        {
            break;
        }

        if help.handle_key(key) {
            continue;
        }

        status_message = None;

        if let Some(prompt) = confirm_delete.take() {
            let old_nodes = forest.nodes.clone();
            if let Some(outcome) =
                answer_delete(key.code, forest, retired, &prompt, &search_buffer, rebuild)
            {
                if outcome.relisted {
                    refresh_probe(&mut probe, forest);
                }
                pre_focus_expanded =
                    remap_rows(&old_nodes, &forest.nodes, &pre_focus_expanded);
                status_message = Some(outcome.message);
                let visible = compute_visible(&forest.nodes, &search_buffer);
                let pos = outcome.row.and_then(|r| visible.iter().position(|&i| i == r));
                state.select(Some(pos.unwrap_or(0)));
            }
            continue;
        }

        if let Some(items) = confirm_bulk_delete.take() {
            let old_nodes = forest.nodes.clone();
            if let Some(outcome) =
                answer_bulk_delete(key.code, forest, retired, &items, &search_buffer, rebuild)
            {
                if outcome.relisted {
                    refresh_probe(&mut probe, forest);
                }
                marks.clear();
                pre_focus_expanded =
                    remap_rows(&old_nodes, &forest.nodes, &pre_focus_expanded);
                status_message = Some(outcome.message);
                let visible = compute_visible(&forest.nodes, &search_buffer);
                let pos = outcome.row.and_then(|r| visible.iter().position(|&i| i == r));
                state.select(Some(pos.unwrap_or(0)));
            }
            continue;
        }

        // The move runs below, once, whether it was confirmed or needed no
        // question; every other path through the prompt ends here.
        let mut run_move: Option<MovePlan> = None;
        let mut run_bulk_move: Option<BulkMovePlan> = None;
        if let Some(plan) = confirm_move.take() {
            if !move_confirmed(key.code) {
                continue;
            }
            run_move = Some(plan);
        } else if let Some(plan) = confirm_bulk_move.take() {
            // Cancelling keeps the marks.
            if !move_confirmed(key.code) {
                continue;
            }
            run_bulk_move = Some(plan);
        } else if let Some(prompt) = move_prompt.as_mut() {
            match key.code {
                KeyCode::Esc => {
                    move_prompt = None;
                    move_set = None;
                }
                KeyCode::Tab => {
                    let scope = &mut prompt.scope;
                    scope.target = next_scope_target(
                        &scope.target,
                        &scope.origin,
                        scope.project.as_deref(),
                        &ctx.new_note_roots,
                        ctx.current_project.as_deref(),
                    );
                }
                KeyCode::Enter => {
                    let prompt = move_prompt.take().expect("the prompt is open");
                    if let Some(items) = move_set.take() {
                        match resolve_bulk_move(&prompt, &items, &ctx.new_note_roots) {
                            Err(message) => status_message = Some(message),
                            Ok(None) => {}
                            Ok(Some(plan)) if plan.needs_confirm() => confirm_bulk_move = Some(plan),
                            Ok(Some(plan)) => run_bulk_move = Some(plan),
                        }
                    } else {
                        match resolve_enter(&prompt, &ctx.new_note_roots) {
                            Err(message) => status_message = Some(message),
                            Ok(None) => {}
                            Ok(Some(plan)) if plan.needs_confirm() => confirm_move = Some(plan),
                            Ok(Some(plan)) => run_move = Some(plan),
                        }
                    }
                }
                code => type_into_move(prompt, code),
            }
            if run_move.is_none() && run_bulk_move.is_none() {
                continue;
            }
        }
        if let Some(plan) = run_bulk_move {
            // The cursor goes to the first moved item when the rebuilt list
            // shows it, else it stays on the row it was on.
            let old_nodes = forest.nodes.clone();
            let was_on = compute_visible(&old_nodes, &search_buffer)
                .get(state.selected().unwrap_or(0))
                .map(|&i| old_nodes[i].path.clone());
            let outcome =
                apply_bulk_move(forest, retired, carried, &plan.plans, &ctx.new_note_roots, rebuild);
            if outcome.relisted {
                refresh_probe(&mut probe, forest);
            }
            marks.clear();
            pre_focus_expanded = remap_rows(&old_nodes, &forest.nodes, &pre_focus_expanded);
            status_message = Some(outcome.message);
            let mut visible = compute_visible(&forest.nodes, &search_buffer);
            let pos = match outcome.row {
                Some(row) => {
                    if !visible.contains(&row) {
                        search_buffer.clear();
                        cursor_pos = 0;
                        search_mode = false;
                        visible = compute_visible(&forest.nodes, &search_buffer);
                    }
                    visible.iter().position(|&i| i == row)
                }
                None => was_on.and_then(|path| visible.iter().position(|&i| forest.nodes[i].path == path)),
            };
            let last = visible.len().saturating_sub(1);
            state.select(Some(pos.unwrap_or_else(|| state.selected().unwrap_or(0).min(last))));
            continue;
        }
        if let Some(plan) = run_move {
            let old_nodes = forest.nodes.clone();
            let outcome =
                apply_move(forest, retired, carried, &plan, &ctx.new_note_roots, rebuild);
            if outcome.relisted {
                refresh_probe(&mut probe, forest);
            }
            pre_focus_expanded = remap_rows(&old_nodes, &forest.nodes, &pre_focus_expanded);
            status_message = Some(outcome.message);
            let Some(row) = outcome.row else {
                continue;
            };
            let mut visible = compute_visible(&forest.nodes, &search_buffer);
            if !visible.contains(&row) {
                search_buffer.clear();
                cursor_pos = 0;
                search_mode = false;
                visible = compute_visible(&forest.nodes, &search_buffer);
            }
            state.select(visible.iter().position(|&i| i == row));
            continue;
        }

        if let Some(prompt) = new_note.as_mut() {
            match key.code {
                KeyCode::Esc => new_note = None,
                KeyCode::Tab => {
                    prompt.target = next_scope_target(
                        &prompt.target,
                        &prompt.origin,
                        prompt.project.as_deref(),
                        &ctx.new_note_roots,
                        ctx.current_project.as_deref(),
                    );
                }
                KeyCode::Enter if new_item_refusal(prompt).is_some() => {
                    status_message = new_item_refusal(prompt);
                }
                KeyCode::Enter if prompt.is_folder => {
                    let NewNotePrompt { target, buffer, .. } =
                        new_note.take().expect("the prompt is open");
                    let old_nodes = forest.nodes.clone();
                    let outcome = create_folder(forest, &target, &buffer, rebuild);
                    if outcome.relisted {
                        refresh_probe(&mut probe, forest);
                    }
                    pre_focus_expanded =
                        remap_rows(&old_nodes, &forest.nodes, &pre_focus_expanded);
                    status_message = outcome.message;
                    let Some(row) = outcome.row else {
                        continue;
                    };
                    let mut visible = compute_visible(&forest.nodes, &search_buffer);
                    if !visible.contains(&row) {
                        search_buffer.clear();
                        cursor_pos = 0;
                        search_mode = false;
                        visible = compute_visible(&forest.nodes, &search_buffer);
                        status_message =
                            Some("filter cleared to show the new folder".to_string());
                    }
                    state.select(visible.iter().position(|&i| i == row));
                }
                KeyCode::Enter => {
                    let NewNotePrompt { target, buffer, .. } =
                        new_note.take().expect("the prompt is open");
                    let words = buffer.split_whitespace().map(String::from).collect();
                    let created = match add::create_in_dir(words, &target.dir, target.scope) {
                        Ok(created) => created,
                        Err(e) => {
                            status_message = Some(format!("new note failed: {e:#}"));
                            continue;
                        }
                    };
                    super::leave().context("failed to leave TUI")?;
                    add::open_created(&created.path, config);
                    *terminal = super::enter().context("failed to re-enter TUI")?;

                    let sections = match rebuild() {
                        Ok(sections) => sections,
                        Err(e) => {
                            status_message = Some(format!(
                                "created {}, but the list could not be refreshed: {e:#}",
                                created.path.display()
                            ));
                            continue;
                        }
                    };
                    let old_nodes = forest.nodes.clone();
                    let row = forest.rebuild(sections, &created.path);
                    refresh_probe(&mut probe, forest);
                    pre_focus_expanded =
                        remap_rows(&old_nodes, &forest.nodes, &pre_focus_expanded);
                    let Some(row) = row else {
                        status_message =
                            Some(format!("created {}", created.path.display()));
                        continue;
                    };
                    let mut visible = compute_visible(&forest.nodes, &search_buffer);
                    if !visible.contains(&row) {
                        search_buffer.clear();
                        cursor_pos = 0;
                        search_mode = false;
                        visible = compute_visible(&forest.nodes, &search_buffer);
                        status_message =
                            Some("filter cleared to show the new note".to_string());
                    }
                    state.select(visible.iter().position(|&i| i == row));
                }
                KeyCode::Backspace => {
                    prompt.buffer.pop();
                }
                KeyCode::Char(c) => prompt.buffer.push(c),
                _ => {}
            }
            continue;
        }

        if let Some(buffer) = rename_buffer.as_mut() {
            match key.code {
                KeyCode::Esc => rename_buffer = None,
                KeyCode::Enter => {
                    let visible = compute_visible(nodes, &search_buffer);
                    let vs = state.selected().unwrap_or(0);
                    let Some(&ri) = visible.get(vs) else {
                        rename_buffer = None;
                        continue;
                    };
                    let before = nodes[ri].path.clone();
                    match enter_rename(nodes, &mut forest.sections, ri, &rename_shown, buffer) {
                        RenameEnter::Keep(message) => status_message = Some(message),
                        RenameEnter::Done(message) => {
                            rename_buffer = None;
                            if message.is_none() && nodes[ri].is_dir {
                                let visible = compute_visible(nodes, &search_buffer);
                                if let Some(pos) = visible.iter().position(|&i| i == ri) {
                                    state.select(Some(pos));
                                }
                            }
                            status_message = message;
                            match relist_after_rename(
                                forest,
                                rebuild,
                                &mut state,
                                &mut pre_focus_expanded,
                                &search_buffer,
                                &mut probe,
                                ri,
                                &before,
                            ) {
                                Some(Ok(Some(row))) => last_preview_idx = row,
                                Some(Err(failure)) => status_message = Some(failure),
                                Some(Ok(None)) | None => {}
                            }
                        }
                    }
                }
                KeyCode::Backspace => {
                    buffer.pop();
                }
                KeyCode::Char(c) => buffer.push(c),
                _ => {}
            }
            continue;
        }

        if flag_mode {
            let mut consumed = true;
            match key.code {
                KeyCode::Char(c @ '1'..='5') => {
                    let idx = (c as u8 - b'1') as usize;
                    let visible = compute_visible(nodes, &search_buffer);
                    let vs = state.selected().unwrap_or(0);
                    let ri = visible.get(vs).copied().unwrap_or(0);
                    if ri < nodes.len() && !nodes[ri].is_dir {
                        nodes[ri].flags ^= FLAG_DEFS[idx].bit;
                    }
                }
                KeyCode::Char('t') | KeyCode::Esc => {
                    flag_mode = false;
                }
                // `/` exits flag mode AND falls through to the filter handler.
                KeyCode::Char('/') => {
                    flag_mode = false;
                    consumed = false;
                }
                KeyCode::Char('j' | 'k' | 'h' | 'l')
                | KeyCode::Down
                | KeyCode::Up
                | KeyCode::Left
                | KeyCode::Right => {
                    consumed = false;
                }
                _ => {}
            }
            if consumed {
                continue;
            }
        }

        if search_mode {
            match key.code {
                KeyCode::Enter | KeyCode::Esc => {
                    search_mode = false;
                    cursor_pos = 0;
                    if key.code == KeyCode::Esc {
                        search_buffer.clear();
                    }
                }
                KeyCode::Left => {
                    cursor_pos = super::text::prev_char_boundary(&search_buffer, cursor_pos);
                }
                KeyCode::Right => {
                    cursor_pos = super::text::next_char_boundary(&search_buffer, cursor_pos);
                }
                KeyCode::Backspace => {
                    if cursor_pos > 0 {
                        let prev =
                            super::text::prev_char_boundary(&search_buffer, cursor_pos);
                        search_buffer.remove(prev);
                        cursor_pos = prev;
                    } else {
                        search_buffer.clear();
                        search_mode = false;
                    }
                }
                KeyCode::Char(c) => {
                    search_buffer.insert(cursor_pos, c);
                    cursor_pos += c.len_utf8();
                }
                _ => {}
            }
            state.select(Some(0));
            continue;
        }

        match vim.handle_key(key) {
            VimKey::Command(cmd) if VimCommandMode::is_quit(&cmd) => break,
            VimKey::Command(_) | VimKey::Consumed => continue,
            VimKey::NotConsumed => {}
        }

        let visible = compute_visible(nodes, &search_buffer);
        let selected = state.selected().unwrap_or(0);
        let real_idx = visible.get(selected).copied().unwrap_or(0);

        if panes.focus == Pane::Preview {
            match preview_focus_key(key.code, preview_page(preview_height)) {
                PreviewFocusKey::Scroll(delta) => {
                    preview_scroll = scrolled(preview_scroll, delta, preview_max);
                    continue;
                }
                PreviewFocusKey::Inert => continue,
                PreviewFocusKey::Pass => {}
            }
        }
        if panes.handle_key(key.code, body_area.width) {
            continue;
        }

        match key.code {
            KeyCode::Char('q') => break,
            KeyCode::Esc => browse_escape(&mut marks, &mut search_buffer),
            KeyCode::Char(' ') => {
                if let Some(message) =
                    press_space(&mut marks, nodes, &mut state, &visible, focus_active)
                {
                    status_message = Some(message.to_string());
                }
            }
            // Shift+Down/Up scroll the preview; matched before the plain
            // arrows, which move the cursor.
            KeyCode::Down if key.modifiers.contains(KeyModifiers::SHIFT) => {
                preview_scroll = scrolled(preview_scroll, 1, preview_max);
            }
            KeyCode::Up if key.modifiers.contains(KeyModifiers::SHIFT) => {
                preview_scroll = scrolled(preview_scroll, -1, preview_max);
            }
            KeyCode::PageDown => {
                preview_scroll = scrolled(preview_scroll, preview_page(preview_height), preview_max);
            }
            KeyCode::PageUp => {
                preview_scroll = scrolled(preview_scroll, -preview_page(preview_height), preview_max);
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if selected + 1 < visible.len() {
                    navigate(nodes, &mut state, &visible, selected, real_idx, focus_active, 1);
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if selected > 0 {
                    navigate(nodes, &mut state, &visible, selected, real_idx, focus_active, -1);
                }
            }
            KeyCode::Char('l') | KeyCode::Right => {
                if selected < visible.len() {
                    let idx = visible[selected];
                    if nodes[idx].is_dir && !nodes[idx].expanded {
                        nodes[idx].expanded = true;
                        focus_active = false;
                    }
                }
            }
            KeyCode::Char('h') | KeyCode::Left => {
                if selected < visible.len() {
                    let idx = visible[selected];
                    if nodes[idx].is_dir && nodes[idx].expanded {
                        nodes[idx].expanded = false;
                        focus_active = false;
                    } else if let Some(parent) = nodes[idx].parent_idx {
                        let vis = get_visible_nodes(nodes);
                        if let Some(pos) = vis.iter().position(|&i| i == parent) {
                            state.select(Some(pos));
                        }
                    }
                }
            }
            KeyCode::Enter | KeyCode::Char('o') => {
                if selected < visible.len() {
                    let idx = visible[selected];
                    if nodes[idx].is_dir {
                        nodes[idx].expanded = !nodes[idx].expanded;
                    } else {
                        let path = nodes[idx].path.clone();
                        super::open_in_editor(&config.editor.command, &path).ok();
                        *terminal = super::enter().context("failed to re-enter TUI")?;
                    }
                }
            }
            KeyCode::Char('f') => {
                if real_idx < nodes.len() {
                    if focus_active {
                        let current_top = find_top_dir(nodes, real_idx);
                        for &(idx, was_expanded) in &pre_focus_expanded {
                            if idx < nodes.len() {
                                nodes[idx].expanded = was_expanded;
                            }
                        }
                        let new_vis = get_visible_nodes(nodes);
                        if let Some(pos) =
                            new_vis.iter().position(|&i| i == current_top.unwrap_or(0))
                        {
                            *state.offset_mut() = 0;
                            state.select(Some(pos));
                        }
                        focus_active = false;
                    } else {
                        pre_focus_expanded = nodes
                            .iter()
                            .enumerate()
                            .filter(|(_, n)| n.is_dir && n.depth == 0)
                            .map(|(i, n)| (i, n.expanded))
                            .collect();
                        let focused_top = find_top_dir(nodes, real_idx);
                        for (i, node) in nodes.iter_mut().enumerate() {
                            if node.is_dir && node.depth == 0 {
                                node.expanded = Some(i) == focused_top;
                            }
                        }
                        focus_active = true;
                    }
                }
            }
            KeyCode::Char('v') => {
                let current_top = find_top_dir(nodes, real_idx).unwrap_or(0);
                let any_collapsed = any_top_collapsed(nodes);
                for node in nodes.iter_mut() {
                    if node.is_dir && node.depth == 0 {
                        node.expanded = any_collapsed;
                    }
                }
                let new_vis = get_visible_nodes(nodes);
                if let Some(pos) = new_vis.iter().position(|&i| i == current_top) {
                    *state.offset_mut() = 0;
                    state.select(Some(pos));
                }
                focus_active = false;
            }
            KeyCode::Char('R') => {
                match reload_view(forest, rebuild, &mut state, &mut pre_focus_expanded, &search_buffer) {
                    Ok(kept) => {
                        if let Some(row) = kept {
                            last_preview_idx = row;
                        }
                        status_message = Some("reloaded".to_string());
                        refresh_probe(&mut probe, forest);
                    }
                    Err(message) => status_message = Some(message),
                }
            }
            KeyCode::Char('/') => {
                search_mode = true;
                search_buffer.clear();
                cursor_pos = 0;
            }
            KeyCode::Char('t') => {
                flag_mode = true;
            }
            KeyCode::Char('r') => {
                match rename_request(nodes, &forest.sections, visible.get(selected).copied()) {
                    Ok(text) => {
                        rename_shown = text.clone().unwrap_or_default();
                        rename_buffer = text;
                    }
                    Err(message) => status_message = Some(message.to_string()),
                }
            }
            KeyCode::Char('n') => {
                new_note = Some(open_new_note_prompt(
                    nodes,
                    &forest.sections,
                    visible.get(selected).copied(),
                    ctx,
                ));
            }
            KeyCode::Char('N') => {
                match open_new_folder_prompt(
                    nodes,
                    &forest.sections,
                    visible.get(selected).copied(),
                    ctx,
                ) {
                    Ok(prompt) => new_note = Some(prompt),
                    Err(message) => status_message = Some(message.to_string()),
                }
            }
            KeyCode::Char(code @ ('m' | 'S')) if !marks.is_empty() => {
                match bulk_move_request(nodes, &forest.sections, &marks, ctx, code == 'S') {
                    Ok((prompt, items)) => {
                        move_prompt = Some(prompt);
                        move_set = Some(items);
                    }
                    Err(message) => status_message = Some(message),
                }
            }
            KeyCode::Char('d') if !marks.is_empty() => {
                match bulk_delete_request(
                    nodes,
                    &forest.sections,
                    &marks,
                    ctx.current_project.as_deref(),
                ) {
                    Ok(items) if !items.is_empty() => confirm_bulk_delete = Some(items),
                    Ok(_) => {}
                    Err(message) => status_message = Some(message),
                }
            }
            KeyCode::Char('m') => {
                match move_request(nodes, &forest.sections, visible.get(selected).copied(), ctx) {
                    Ok(prompt) => move_prompt = Some(prompt),
                    Err(message) => status_message = Some(message.to_string()),
                }
            }
            KeyCode::Char('S') => {
                match set_scope_request(nodes, &forest.sections, visible.get(selected).copied(), ctx) {
                    Ok(prompt) => move_prompt = Some(prompt),
                    Err(message) => status_message = Some(message.to_string()),
                }
            }
            KeyCode::Char('d') => {
                match delete_request(
                    nodes,
                    &forest.sections,
                    visible.get(selected).copied(),
                    ctx.current_project.as_deref(),
                ) {
                    Ok(prompt) => confirm_delete = prompt,
                    Err(message) => status_message = Some(message.to_string()),
                }
            }
            KeyCode::Char('J') => {
                preview_scroll = scrolled(preview_scroll, 1, preview_max);
            }
            KeyCode::Char('K') => {
                preview_scroll = scrolled(preview_scroll, -1, preview_max);
            }
            KeyCode::Char('p') => {
                let path = nodes.get(real_idx).filter(|n| !n.is_dir).map(|n| n.path.as_path());
                preview.toggle(path);
            }
            KeyCode::Char('?') => {
                help.open();
            }
            _ => {}
        }
    }

    Ok(())
}

fn file_name_of(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// j/k step that, in focus mode, hops the exclusive expansion from one
/// top-level section to the next as the cursor crosses section boundaries.
fn navigate(
    nodes: &mut [TreeNode],
    state: &mut ListState,
    visible: &[usize],
    selected: usize,
    real_idx: usize,
    focus_active: bool,
    step: isize,
) {
    let next_sel = selected.saturating_add_signed(step);
    let target = visible[next_sel];
    if focus_active {
        let old_top = find_top_dir(nodes, real_idx);
        let new_top = find_top_dir(nodes, target);
        if old_top != new_top {
            if let Some(ot) = old_top {
                nodes[ot].expanded = false;
            }
            if let Some(nt) = new_top {
                nodes[nt].expanded = true;
            }
            let new_vis = get_visible_nodes(nodes);
            if let Some(pos) = new_vis.iter().position(|&i| i == target) {
                state.select(Some(pos));
            }
            return;
        }
    }
    state.select(Some(next_sel));
}

/// Columns taken by the " ! " prefix of the warning footer.
const WARNING_PREFIX_COLS: usize = 3;

/// Lays out the warning footer for `width` columns: returns the warning text,
/// truncated by chars if needed, and the padding that puts the quit hint on the
/// same column as the hints footer (`footer::QUIT_HINT_RESERVED_COLS` from the
/// right edge, the column `footer::quit_column` gives). At least one space
/// always separates text and hint.
fn warning_layout(warning: &str, width: usize) -> (String, usize) {
    let max_chars = width.saturating_sub(WARNING_PREFIX_COLS + QUIT_HINT_RESERVED_COLS + 1);
    let text: String = warning.chars().take(max_chars).collect();
    let used = WARNING_PREFIX_COLS + text.chars().count() + QUIT_HINT_RESERVED_COLS;
    (text, width.saturating_sub(used))
}

#[cfg(test)]
mod tests {
    use super::*;
    use notez_core::tags::{FLAG_BLOCKED, FLAG_IDEA, FLAG_IMPORTANT, FLAG_LONGTERM, FLAG_PRIO};

    #[test]
    fn warning_layout_puts_the_quit_hint_four_columns_from_the_right_edge() {
        let (text, padding) = warning_layout("pull failed", 40);
        assert_eq!(text, "pull failed");
        assert_eq!(WARNING_PREFIX_COLS + text.chars().count() + padding, 40 - 4);
        assert!(padding >= 1);
    }

    #[test]
    fn warning_footer_quit_hint_lines_up_with_the_normal_footer() {
        // Compare with the column where the real hints footer draws `q`, in
        // the modes in which the warning can share the line with hints.
        let short = "pull failed";
        let long = "x".repeat(300);
        for width in [40usize, 60, 100, 120] {
            for mode in [Mode::Normal, Mode::Focus] {
                let line = footer::line(TREE_KEYS, mode, &[], width);
                let rendered: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
                let hints_q_col = rendered.chars().position(|c| c == 'q').unwrap();
                let quit_col = footer::select(TREE_KEYS, mode, &[], width).quit_col;
                assert_eq!(quit_col, Some(hints_q_col), "width {width}, {mode:?}");
                for warning in [short, long.as_str()] {
                    let (text, padding) = warning_layout(warning, width);
                    assert_eq!(
                        WARNING_PREFIX_COLS + text.chars().count() + padding,
                        hints_q_col,
                        "width {width}, {mode:?}, warning of {} chars",
                        warning.len()
                    );
                }
            }
            let (text, padding) = warning_layout(&long, width);
            assert_eq!(text.chars().count(), width - 8);
            assert!(padding >= 1);
        }
    }

    #[test]
    fn warning_layout_truncates_on_a_char_boundary_and_keeps_the_hint() {
        let warning = "vault pull failed \u{e5}\u{e4}\u{f6} \u{1f4a5} and then some more text";
        let (text, padding) = warning_layout(warning, 24);
        assert!(warning.starts_with(&text));
        assert_eq!(
            text.chars().count(),
            24 - WARNING_PREFIX_COLS - QUIT_HINT_RESERVED_COLS - 1
        );
        assert_eq!(padding, 1);
        assert_eq!(WARNING_PREFIX_COLS + text.chars().count() + padding, 24 - 4);

        let (cut, _) = warning_layout("ab\u{1f4a5}cd", 3 + 3 + 1 + 4);
        assert_eq!(cut, "ab\u{1f4a5}");
    }

    #[test]
    fn warning_layout_survives_zero_width() {
        assert_eq!(warning_layout("anything", 0), (String::new(), 0));
    }

    fn shown_keys(mode: Mode, on: &[Toggle], width: usize) -> Vec<&'static str> {
        let sel = footer::select(TREE_KEYS, mode, on, width);
        sel.left
            .iter()
            .chain(sel.quit.iter())
            .map(|&(i, _)| TREE_KEYS[i].key)
            .collect()
    }

    fn lit_keys(mode: Mode, on: &[Toggle]) -> Vec<&'static str> {
        let sel = footer::select(TREE_KEYS, mode, on, 200);
        sel.left
            .iter()
            .chain(sel.quit.iter())
            .filter(|&&(_, lit)| lit)
            .map(|&(i, _)| TREE_KEYS[i].key)
            .collect()
    }

    #[test]
    fn footer_mode_follows_the_input_state_most_specific_first() {
        assert_eq!(footer_mode(false, false, false, false, false), Mode::Normal);
        assert_eq!(footer_mode(false, false, false, false, true), Mode::Focus);
        assert_eq!(footer_mode(false, false, false, true, true), Mode::Tag);
        assert_eq!(footer_mode(false, false, true, false, true), Mode::Filter);
        assert_eq!(footer_mode(false, true, true, false, false), Mode::VimCommand);
        assert_eq!(footer_mode(true, true, true, true, true), Mode::Rename);
    }

    #[test]
    fn normal_and_focus_footers_hint_the_browse_keys() {
        let expected = vec![
            "o", "t", "r", "n", "N", "m", "S", "space", "d", "/", "f", "v", "?", "J/K", "1/2", "tab", "</>", "=", "q",
        ];
        assert_eq!(shown_keys(Mode::Normal, &[], 200), expected);
        assert_eq!(shown_keys(Mode::Focus, &[], 200), expected);
    }

    #[test]
    fn filter_footer_hints_the_filter_input_keys_and_no_quit() {
        assert_eq!(
            shown_keys(Mode::Filter, &[], 200),
            vec!["enter", "esc", "\u{2190}/\u{2192}", "bksp"]
        );
    }

    #[test]
    fn tag_rename_and_command_footers_hint_their_own_keys() {
        assert_eq!(shown_keys(Mode::Tag, &[], 200), vec!["t", "1-5", "esc", "/"]);
        assert_eq!(shown_keys(Mode::Rename, &[], 200), vec!["enter", "esc", "bksp"]);
        assert_eq!(shown_keys(Mode::VimCommand, &[], 200), vec!["enter", "esc", "bksp"]);
    }

    #[test]
    fn footer_lights_the_keys_whose_state_is_on() {
        assert!(lit_keys(Mode::Normal, &[]).is_empty());
        assert_eq!(lit_keys(Mode::Focus, &[Toggle::Focus]), vec!["f"]);
        assert_eq!(lit_keys(Mode::Normal, &[Toggle::Filter]), vec!["/"]);
        assert_eq!(lit_keys(Mode::Tag, &[Toggle::Tag]), vec!["t"]);
        assert_eq!(lit_keys(Mode::Normal, &[Toggle::ExpandAll]), vec!["v"]);
        assert_eq!(lit_keys(Mode::Normal, &[Toggle::Help]), vec!["?"]);
    }

    #[test]
    fn footer_toggles_map_each_state_to_its_toggle() {
        assert!(footer_toggles(false, false, false, false, false).is_empty());
        assert_eq!(
            footer_toggles(true, true, true, true, true),
            vec![Toggle::Focus, Toggle::Filter, Toggle::Tag, Toggle::ExpandAll, Toggle::Help]
        );
    }

    #[test]
    fn narrow_footer_drops_low_priority_hints_but_keeps_help_and_quit() {
        let all = vec!["o", "t", "r", "n", "d", "/", "f", "v", "?", "q"];
        assert_eq!(shown_keys(Mode::Normal, &[], 72), all);
        assert_eq!(shown_keys(Mode::Normal, &[], 71), vec!["o", "t", "r", "n", "/", "f", "v", "?", "q"]);
        assert_eq!(shown_keys(Mode::Normal, &[], 64), vec!["o", "t", "r", "n", "/", "f", "v", "?", "q"]);
        assert_eq!(shown_keys(Mode::Normal, &[], 63), vec!["o", "t", "n", "/", "f", "v", "?", "q"]);
        assert_eq!(shown_keys(Mode::Normal, &[], 55), vec!["o", "t", "n", "/", "f", "?", "q"]);
        assert_eq!(shown_keys(Mode::Normal, &[], 45), vec!["o", "t", "n", "f", "?", "q"]);
        assert_eq!(shown_keys(Mode::Normal, &[], 35), vec!["o", "t", "n", "?", "q"]);
        assert_eq!(shown_keys(Mode::Normal, &[], 28), vec!["o", "t", "?", "q"]);
        assert_eq!(shown_keys(Mode::Normal, &[], 18), vec!["o", "?", "q"]);
        assert_eq!(shown_keys(Mode::Normal, &[], 17), vec!["?", "q"]);
        assert_eq!(shown_keys(Mode::Normal, &[], 0), vec!["?", "q"]);
        for width in 0..120 {
            let line = footer::line(TREE_KEYS, Mode::Normal, &[], width);
            let cols: usize = line.spans.iter().map(|s| s.content.chars().count()).sum();
            // From 12 columns (" ? help" plus a space and "quit") the line
            // fills the width exactly, with `q` four columns from the edge.
            if width >= 12 {
                assert_eq!(cols, width, "width {width}");
            }
        }
    }

    #[test]
    fn help_lists_every_tree_key_exactly_once() {
        let rows = help::rows(TREE_KEYS);
        for i in 0..TREE_KEYS.len() {
            let count = rows.iter().filter(|r| **r == help::Row::Key(i)).count();
            assert_eq!(count, 1, "key {} ({})", TREE_KEYS[i].key, TREE_KEYS[i].help);
        }
        for group in Group::ALL {
            assert!(rows.contains(&help::Row::Heading(group)), "{group:?}");
        }
    }

    #[test]
    fn scrolled_clamps_at_the_top() {
        assert_eq!(scrolled(0, -1, 10), 0);
        assert_eq!(scrolled(2, -3, 10), 0);
        assert_eq!(scrolled(0, i32::MIN, 10), 0);
    }

    #[test]
    fn scrolled_clamps_at_the_end() {
        assert_eq!(scrolled(10, 1, 10), 10);
        assert_eq!(scrolled(9, 3, 10), 10);
        assert_eq!(scrolled(0, 5, 0), 0);
        assert_eq!(scrolled(u16::MAX, i32::MAX, u16::MAX), u16::MAX);
        // A stale offset past a shrunken end comes back to it.
        assert_eq!(scrolled(15, 0, 10), 10);
    }

    #[test]
    fn scrolled_moves_one_line_for_j_and_k() {
        assert_eq!(scrolled(4, 1, 10), 5);
        assert_eq!(scrolled(4, -1, 10), 3);
    }

    #[test]
    fn wheel_step_is_three_lines() {
        assert_eq!(scrolled(4, WHEEL_STEP, 100), 7);
        assert_eq!(scrolled(4, -WHEEL_STEP, 100), 1);
    }

    #[test]
    fn page_step_is_the_pane_height_minus_one_line() {
        assert_eq!(preview_page(20), 19);
        assert_eq!(scrolled(0, preview_page(20), 100), 19);
        assert_eq!(scrolled(19, -preview_page(20), 100), 0);
        assert_eq!(scrolled(90, preview_page(20), 100), 100);
        // A pane one line tall (or none) still pages by a line.
        assert_eq!(preview_page(1), 1);
        assert_eq!(preview_page(0), 1);
    }

    #[test]
    fn preview_scroll_keys_are_listed_and_j_k_shows_in_the_footer_first_to_drop() {
        let jk = TREE_KEYS.iter().find(|k| k.key == "J/K").unwrap();
        assert_eq!(jk.help, "scroll preview down / up (also Shift+Down/Up)");
        assert_eq!(jk.group, Group::Navigate);
        assert_eq!(jk.modes, BROWSE);
        let page = TREE_KEYS.iter().find(|k| k.key == "PgDn/PgUp").unwrap();
        assert_eq!(page.help, "scroll preview a page");
        assert_eq!(page.group, Group::Navigate);
        assert_eq!(page.modes, BROWSE);
        let Slot::Priority(jk_priority) = jk.slot else {
            panic!("J/K must show in the footer, not {:?}", jk.slot);
        };
        // NZ-4: the pane keys drop before J/K; every other key after it.
        for other in TREE_KEYS.iter().filter(|k| k.key != "J/K") {
            if let Slot::Priority(p) = other.slot {
                if PANE_KEYS.contains(&other.key) && other.modes == BROWSE {
                    assert!(p > jk_priority, "{} drops after J/K", other.key);
                } else {
                    assert!(p < jk_priority, "{} drops before J/K", other.key);
                }
            }
        }
        let first_width = |key: &str| (0..300).find(|&w| shown_keys(Mode::Normal, &[], w).contains(&key)).unwrap();
        assert!(first_width("J/K") > first_width("space"), "J/K drops before space");
        assert!(first_width("J/K") > first_width("S"), "J/K drops first of the list keys");
        for pane_key in PANE_KEYS {
            assert!(first_width(pane_key) > first_width("J/K"), "{pane_key} drops after J/K");
        }
    }

    // --- Panes: split, focus and fold (NZ-4) ---

    /// The footer keys of the pane rows, in table order.
    const PANE_KEYS: [&str; 4] = ["1/2", "tab", "</>", "="];

    #[test]
    fn the_pane_keys_are_browse_rows_in_the_view_group_listed_once() {
        for key in PANE_KEYS {
            let rows: Vec<_> = TREE_KEYS.iter().filter(|k| k.key == key && k.modes == BROWSE).collect();
            assert_eq!(rows.len(), 1, "{key}");
            assert_eq!(rows[0].group, Group::View, "{key}");
            assert!(matches!(rows[0].slot, Slot::Priority(_)), "{key}");
        }
        // The keys were free: no other browse row names `<`, `>`, `=`, `1`
        // or `2`, and `tab` is otherwise only a prompt key.
        for hint in TREE_KEYS.iter().filter(|k| !PANE_KEYS.contains(&k.key)) {
            let parts: Vec<&str> = hint.key.split('/').collect();
            let browse = hint.modes.iter().any(|m| BROWSE.contains(m));
            for key in ["<", ">", "=", "1", "2", "tab"] {
                assert!(!(browse && parts.contains(&key)), "{} ({}) also binds {key}", hint.key, hint.help);
            }
        }
        for prompt in [NEW_NOTE, MOVING, SETTING_SCOPE] {
            assert!(TREE_KEYS.iter().any(|k| k.key == "tab" && k.modes == prompt));
        }
    }

    #[test]
    fn the_pane_keys_drop_first_and_in_reverse_table_order() {
        let first_width = |key: &str| (0..300).find(|&w| shown_keys(Mode::Normal, &[], w).contains(&key)).unwrap();
        let widths: Vec<usize> = PANE_KEYS.iter().map(|key| first_width(key)).collect();
        assert!(widths.windows(2).all(|w| w[0] < w[1]), "{widths:?}");
        // Above the pane keys' widths the footer still fills the line.
        for width in 0..160 {
            let line = footer::line(TREE_KEYS, Mode::Normal, &[], width);
            let cols: usize = line.spans.iter().map(|s| s.content.chars().count()).sum();
            if width >= 12 {
                assert_eq!(cols, width, "width {width}");
            }
        }
    }

    fn pane_footer(focus: Pane, toggle: Option<PreviewMode>, width: usize) -> Vec<String> {
        let keys = pane_keys(tree_keys(toggle), focus);
        let sel = footer::select(&keys, Mode::Normal, &[], width);
        sel.left
            .iter()
            .chain(sel.quit.iter())
            .map(|&(i, _)| format!("{} {}", keys[i].key, keys[i].desc))
            .collect()
    }

    #[test]
    fn the_list_focused_footer_is_the_browse_footer() {
        let keys = pane_keys(tree_keys(None), Pane::List);
        let shown: Vec<&str> = footer::select(&keys, Mode::Normal, &[], 200).left.iter().map(|&(i, _)| keys[i].key).collect();
        assert_eq!(shown, shown_keys(Mode::Normal, &[], 200)[..shown.len()]);
    }

    #[test]
    fn the_preview_focused_footer_shows_the_preview_set() {
        assert_eq!(
            pane_footer(Pane::Preview, None, 200),
            vec!["j/k scroll", "? help", "PgDn/PgUp page", "2 fold", "1/tab list", "q quit"]
        );
        assert_eq!(
            pane_footer(Pane::Preview, Some(PreviewMode::Rendered), 200),
            vec!["j/k scroll", "p raw", "? help", "PgDn/PgUp page", "2 fold", "1/tab list", "q quit"]
        );
        assert_eq!(
            pane_footer(Pane::Preview, Some(PreviewMode::Raw), 200),
            vec!["j/k scroll", "p rendered", "? help", "PgDn/PgUp page", "2 fold", "1/tab list", "q quit"]
        );
        // Narrow, the scroll hints are the last to go before help and quit.
        assert_eq!(pane_footer(Pane::Preview, Some(PreviewMode::Raw), 30), vec!["j/k scroll", "? help", "q quit"]);
    }

    #[test]
    fn the_preview_footer_keeps_the_same_rows_so_help_lists_each_key_once() {
        for toggle in [None, Some(PreviewMode::Rendered)] {
            let keys = pane_keys(tree_keys(toggle), Pane::Preview);
            assert_eq!(keys.len(), TREE_KEYS.len());
            for (shown, row) in keys.iter().zip(TREE_KEYS) {
                assert_eq!(shown.help, row.help);
                assert_eq!(shown.modes, row.modes);
            }
        }
        // The prompt `tab` rows keep their own footer slots.
        let keys = pane_keys(tree_keys(None), Pane::Preview);
        let prompt_tab = keys.iter().find(|k| k.key == "tab" && k.modes == NEW_NOTE).unwrap();
        assert_eq!(prompt_tab.slot, Slot::Priority(3));
    }

    #[test]
    fn with_the_preview_focused_j_k_arrows_and_pages_scroll_it() {
        assert_eq!(preview_focus_key(KeyCode::Char('j'), 9), PreviewFocusKey::Scroll(1));
        assert_eq!(preview_focus_key(KeyCode::Down, 9), PreviewFocusKey::Scroll(1));
        assert_eq!(preview_focus_key(KeyCode::Char('k'), 9), PreviewFocusKey::Scroll(-1));
        assert_eq!(preview_focus_key(KeyCode::Up, 9), PreviewFocusKey::Scroll(-1));
        assert_eq!(preview_focus_key(KeyCode::PageDown, 9), PreviewFocusKey::Scroll(9));
        assert_eq!(preview_focus_key(KeyCode::PageUp, 9), PreviewFocusKey::Scroll(-9));
    }

    #[test]
    fn with_the_preview_focused_the_list_cursor_keys_are_inert() {
        for code in [
            KeyCode::Char('h'),
            KeyCode::Char('l'),
            KeyCode::Left,
            KeyCode::Right,
            KeyCode::Enter,
            KeyCode::Char('o'),
            KeyCode::Char(' '),
        ] {
            assert_eq!(preview_focus_key(code, 9), PreviewFocusKey::Inert, "{code:?}");
        }
    }

    #[test]
    fn with_the_preview_focused_every_other_key_passes_to_the_list() {
        for c in ['J', 'K', 'p', 'q', '?', '/', 't', 'n', 'N', 'r', 'm', 'S', 'd', 'f', 'v', ':', '1', '2', '<', '>', '='] {
            assert_eq!(preview_focus_key(KeyCode::Char(c), 9), PreviewFocusKey::Pass, "{c}");
        }
        for code in [KeyCode::Tab, KeyCode::Esc] {
            assert_eq!(preview_focus_key(code, 9), PreviewFocusKey::Pass, "{code:?}");
        }
    }

    #[test]
    fn the_focused_pane_has_the_highlighted_border_and_both_titles_carry_their_number() {
        assert_eq!(pane_border(Pane::List, Pane::List), theme::border_focused());
        assert_eq!(pane_border(Pane::Preview, Pane::List), theme::border());
        assert_eq!(pane_border(Pane::Preview, Pane::Preview), theme::border_focused());
        assert_ne!(theme::border_focused(), theme::border());
        let list = pane_title(Pane::List, Pane::List, vec![Span::raw("notes ")]);
        assert_eq!(text_of(&list), " 1 notes ");
        assert_eq!(list.spans[0].style, theme::pane_number(true));
        let preview = pane_title(Pane::Preview, Pane::List, vec![Span::raw("preview ")]);
        assert_eq!(text_of(&preview), " 2 preview ");
        assert_eq!(preview.spans[0].style, theme::pane_number(false));
    }

    #[test]
    fn the_list_text_width_follows_the_split() {
        let total = Rect::new(2, 1, 120, 30);
        let mut panes = Panes::default();
        let (list, _, _) = panes.layout(total);
        assert_eq!(list_text_width(list.width), 60 - 4);
        panes.handle_key(KeyCode::Char('<'), total.width);
        let (narrower, _, _) = panes.layout(total);
        assert_eq!(list_text_width(narrower.width), 54 - 4);
        panes.handle_key(KeyCode::Char('2'), total.width);
        panes.handle_key(KeyCode::Char('2'), total.width);
        let (folded, _, preview) = panes.layout(total);
        assert!(preview.is_none());
        assert_eq!(list_text_width(folded.width), 120 - 4);
    }

    #[test]
    fn the_filter_strip_rows_and_tag_dots_take_clicks_at_a_30_and_a_70_split() {
        let total = Rect::new(2, 1, 120, 30);
        for split in [30u16, 70] {
            let mut panes = Panes { split, focus: Pane::Preview, ..Panes::default() };
            panes.clamp(total.width);
            let (list, _, _) = panes.layout(total);
            let chunks = list_chunks(list);
            let (strip, rows) = (chunks[0], chunks[2]);
            assert!(rows.height > 0 && strip.y < rows.y, "split {split}");
            // Every cell of the strip and the rows reaches the list's click
            // handling: none starts a drag, and each focuses the list.
            for (area, y) in [(strip, strip.y), (rows, rows.y), (rows, rows.y + rows.height - 1)] {
                for x in area.x..area.x + area.width {
                    let mut clicked = panes;
                    assert_eq!(clicked.press(total, x, y), Press::Pane(Pane::List), "split {split} ({x}, {y})");
                    assert_eq!(clicked.focus, Pane::List);
                    assert!(!clicked.dragging);
                }
            }
            // The strip's five dots map to their tags, as at 50/50, and so
            // do a row's dots with every tag set, right after its gutter.
            for dot in 0..5u8 {
                assert_eq!(mouse_x_to_dot(strip.x + 5 + u16::from(dot), strip.x), Some(dot), "split {split}");
                let col = rows.x + 1 + u16::from(dot);
                assert_eq!(mouse_x_to_row_tag(col, rows.x, 0b1_1111), Some(usize::from(dot)), "split {split}");
            }
            // A row fills exactly the text width the split gives it.
            assert_eq!(usize::from(rows.width), list_text_width(list.width), "split {split}");
        }
    }

    #[test]
    fn the_grip_is_furniture_at_rest_and_lit_while_dragging() {
        assert_eq!(theme::grip(false), theme::border());
        assert_eq!(theme::grip(true), theme::border_focused());
    }

    // --- Preview rendering (NZ-25) ---

    fn plain(lines: &[Line]) -> Vec<String> {
        lines.iter().map(text_of).collect()
    }

    #[test]
    fn preview_mode_defaults_to_rendered_and_p_toggles_it_on_a_markdown_note() {
        let dir = tempfile::tempdir().unwrap();
        let note = dir.path().join("a.md");
        let mut preview = Preview::default();
        assert_eq!(preview.mode, PreviewMode::Rendered);
        preview.toggle(Some(&note));
        assert_eq!(preview.mode, PreviewMode::Raw);
        preview.toggle(Some(&note));
        assert_eq!(preview.mode, PreviewMode::Rendered);
    }

    #[test]
    fn p_does_nothing_on_a_non_markdown_file_a_folder_or_no_row() {
        let dir = tempfile::tempdir().unwrap();
        let mut preview = Preview::default();
        preview.toggle(Some(&dir.path().join("Cargo.toml")));
        preview.toggle(None);
        assert_eq!(preview.mode, PreviewMode::Rendered);
        preview.toggle(Some(&dir.path().join("notes.MD")));
        assert_eq!(preview.mode, PreviewMode::Raw, ".MD is markdown too");
    }

    #[test]
    fn the_preview_mode_persists_across_selection_changes() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.md");
        let b = dir.path().join("b.md");
        std::fs::write(&a, "# A\n").unwrap();
        std::fs::write(&b, "# B\n\n**bold**\n").unwrap();
        let mut preview = Preview::default();
        assert_eq!(plain(preview.file_lines(&a, 40)), vec!["A"]);
        preview.toggle(Some(&a));
        assert_eq!(plain(preview.file_lines(&b, 40)), vec!["# B", "", "**bold**"], "raw on the next note");
        assert_eq!(plain(preview.file_lines(&a, 40)), vec!["# A"], "and back on the first");
        preview.toggle(Some(&b));
        assert_eq!(plain(preview.file_lines(&b, 40)), vec!["B", "", "bold"]);
    }

    #[test]
    fn a_non_markdown_file_shows_raw_whatever_the_mode() {
        let dir = tempfile::tempdir().unwrap();
        let toml = dir.path().join("c.toml");
        std::fs::write(&toml, "# comment\n[a]\n").unwrap();
        let mut preview = Preview::default();
        assert_eq!(plain(preview.file_lines(&toml, 40)), vec!["# comment", "[a]"]);
        preview.mode = PreviewMode::Raw;
        assert_eq!(plain(preview.file_lines(&toml, 40)), vec!["# comment", "[a]"]);
    }

    #[test]
    fn rendered_lines_wrap_to_the_given_width_so_their_count_is_the_scroll_count() {
        let dir = tempfile::tempdir().unwrap();
        let note = dir.path().join("w.md");
        std::fs::write(&note, "one two three four five six\n").unwrap();
        let mut preview = Preview::default();
        let lines = plain(preview.file_lines(&note, 10));
        assert!(lines.len() > 1, "{lines:?}");
        assert!(lines.iter().all(|l| l.chars().count() <= 10), "{lines:?}");
        preview.mode = PreviewMode::Raw;
        assert_eq!(preview.file_lines(&note, 10).len(), 1, "raw is never wrapped");
    }

    #[test]
    fn an_unreadable_file_shows_the_unreadable_line() {
        let dir = tempfile::tempdir().unwrap();
        let mut preview = Preview::default();
        assert_eq!(plain(preview.file_lines(&dir.path().join("gone.md"), 40)), vec!["  unable to read file"]);
    }

    #[test]
    fn the_cache_reuses_lines_for_the_same_key_and_rebuilds_on_any_change() {
        let dir = tempfile::tempdir().unwrap();
        let note = dir.path().join("a.md");
        std::fs::write(&note, "# A\n").unwrap();
        let key = |width: u16, rendered: bool| PreviewKey::of(&note, width, rendered);
        assert!(key(40, true).is_some());
        assert_eq!(key(40, true), key(40, true));
        assert_ne!(key(40, true), key(41, true), "width");
        assert_ne!(key(40, true), key(40, false), "mode");
        assert_eq!(PreviewKey::of(&dir.path().join("gone.md"), 40, true), None);

        let mut cache = PreviewCache::default();
        let mut builds = 0;
        let mut get = |cache: &mut PreviewCache, key: Option<PreviewKey>| {
            cache.get(key, || {
                builds += 1;
                vec![Line::from("x")]
            })
            .len()
        };
        get(&mut cache, key(40, true));
        get(&mut cache, key(40, true));
        get(&mut cache, key(40, true));
        get(&mut cache, key(30, true));
        get(&mut cache, key(30, false));
        get(&mut cache, key(30, true));
        get(&mut cache, None);
        get(&mut cache, None);
        assert_eq!(builds, 6, "three draws of one key build once; each change and each unkeyed draw builds");
    }

    #[test]
    fn the_cache_rebuilds_when_the_file_changes() {
        let dir = tempfile::tempdir().unwrap();
        let note = dir.path().join("a.md");
        std::fs::write(&note, "# A\n").unwrap();
        let mut preview = Preview::default();
        assert_eq!(plain(preview.file_lines(&note, 40)), vec!["A"]);
        std::fs::write(&note, "# Longer title\n").unwrap();
        assert_eq!(plain(preview.file_lines(&note, 40)), vec!["Longer title"]);
    }

    #[test]
    fn file_type_names_markdown_the_lowercase_extension_or_file() {
        assert_eq!(file_type(Path::new("/n/a.md"), false).as_deref(), Some("markdown"));
        assert_eq!(file_type(Path::new("/n/A.MD"), false).as_deref(), Some("markdown"));
        assert_eq!(file_type(Path::new("/n/Cargo.toml"), false).as_deref(), Some("toml"));
        assert_eq!(file_type(Path::new("/n/main.RS"), false).as_deref(), Some("rust"));
        assert_eq!(file_type(Path::new("/n/notes.txt"), false).as_deref(), Some("txt"));
        assert_eq!(file_type(Path::new("/n/Makefile"), false).as_deref(), Some("file"));
        assert_eq!(file_type(Path::new("/n/.gitignore"), false).as_deref(), Some("file"));
        assert_eq!(file_type(Path::new("/n/trailing."), false).as_deref(), Some("file"));
        assert_eq!(file_type(Path::new("/n/ideas.md"), true), None, "a folder");
        assert_eq!(file_type(Path::new("/n/ideas"), true), None, "a folder or section row");
    }

    #[test]
    fn file_type_names_the_highlighting_language() {
        assert_eq!(file_type(Path::new("/n/main.rs"), false).as_deref(), Some("rust"));
        assert_eq!(file_type(Path::new("/n/App.kt"), false).as_deref(), Some("kotlin"));
        assert_eq!(file_type(Path::new("/n/build.gradle.kts"), false).as_deref(), Some("kotlin"));
        assert_eq!(file_type(Path::new("/n/a.md"), false).as_deref(), Some("markdown"));
        assert_eq!(file_type(Path::new("/n/a.py"), false).as_deref(), Some("python"));
        assert_eq!(file_type(Path::new("/n/a.txt"), false).as_deref(), Some("txt"));
        assert_eq!(file_type(Path::new("/n/a.yaml"), false).as_deref(), Some("yaml"));
    }

    #[test]
    fn a_file_over_the_limit_is_shown_plain_and_the_footer_says_so() {
        let dir = tempfile::tempdir().unwrap();
        let big = dir.path().join("big.rs");
        let line = "fn f() {}\n";
        let count = highlight::MAX_HIGHLIGHT_BYTES as usize / line.len() + 1;
        std::fs::write(&big, line.repeat(count)).unwrap();
        assert_eq!(file_type(&big, false).as_deref(), Some("rust (not highlighted, large)"));
        let mut preview = Preview::default();
        let lines = preview.file_lines(&big, 40);
        assert_eq!(lines.len(), count);
        assert_eq!(lines[0], raw_preview_line("fn f() {}"));
        assert_eq!(PreviewKey::of(&big, 40, false).unwrap().language, None);

        let small = dir.path().join("small.rs");
        std::fs::write(&small, line).unwrap();
        assert_eq!(file_type(&small, false).as_deref(), Some("rust"));
        let big_md = dir.path().join("big.md");
        std::fs::write(&big_md, "text\n".repeat(count * 2)).unwrap();
        assert_eq!(file_type(&big_md, false).as_deref(), Some("markdown (not highlighted, large)"));
    }

    fn style_of(line: &Line<'static>, text: &str) -> Style {
        line.spans
            .iter()
            .find(|span| span.content == text)
            .unwrap_or_else(|| panic!("no span {text:?} in {line:?}"))
            .style
    }

    #[test]
    fn a_rust_file_is_highlighted_whole() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("main.rs");
        std::fs::write(&file, "// note\nfn main() {\n    let s = \"x\";\n}\n").unwrap();
        let mut preview = Preview::default();
        let lines = preview.file_lines(&file, 40).to_vec();
        assert_eq!(plain(&lines), vec!["// note", "fn main() {", "    let s = \"x\";", "}"]);
        assert_eq!(style_of(&lines[0], "// note"), theme::syntax_comment());
        assert_eq!(style_of(&lines[1], "fn"), theme::syntax_keyword());
        assert_eq!(style_of(&lines[2], "let"), theme::syntax_keyword());
        assert_eq!(style_of(&lines[2], "\"x\""), theme::syntax_string());
        assert_eq!(style_of(&lines[2], "    "), raw_line_style("    let"), "uncaptured text takes the raw style");
    }

    /// A guard, not a benchmark: selecting a 200 KB Rust file highlights it
    /// once; drawing it again comes from the cache.
    #[test]
    fn a_200_kb_rust_file_highlights_on_selection_and_redraws_from_the_cache() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("big.rs");
        let block = "/// Doc comment.\n#[derive(Debug)]\npub struct Item { id: u32, name: String }\n\nfn build(n: u32) -> Vec<Item> {\n    (0..n).map(|id| Item { id, name: format!(\"item {id}\") }).collect()\n}\n\n";
        std::fs::write(&file, block.repeat(200 * 1024 / block.len() + 1)).unwrap();
        let mut preview = Preview::default();
        let started = std::time::Instant::now();
        let count = preview.file_lines(&file, 80).len();
        let first = started.elapsed();
        let started = std::time::Instant::now();
        assert_eq!(preview.file_lines(&file, 80).len(), count);
        let again = started.elapsed();
        eprintln!("200 KB rust: {count} lines, first {first:?}, cached {again:?}");
        assert!(first.as_secs_f64() < 5.0, "first selection took {first:?}");
        assert!(again < first, "cached {again:?} vs first {first:?}");
    }

    #[test]
    fn a_kotlin_file_is_highlighted_whole() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("App.kt");
        std::fs::write(&file, "fun main() {\n    val s = \"x\"\n}\n").unwrap();
        let mut preview = Preview::default();
        let lines = preview.file_lines(&file, 40).to_vec();
        assert_eq!(plain(&lines), vec!["fun main() {", "    val s = \"x\"", "}"]);
        assert_eq!(style_of(&lines[0], "fun"), theme::syntax_keyword());
        assert_eq!(style_of(&lines[1], "val"), theme::syntax_keyword());
        assert_eq!(style_of(&lines[1], "\"x\""), theme::syntax_string());
    }

    #[test]
    fn raw_markdown_is_highlighted_and_untouched_lines_keep_the_raw_style() {
        let dir = tempfile::tempdir().unwrap();
        let note = dir.path().join("n.md");
        std::fs::write(&note, "# Title\n\nplain words\n\n```rust\nlet x = 1;\n```\n").unwrap();
        let mut preview = Preview { mode: PreviewMode::Raw, ..Preview::default() };
        let lines = preview.file_lines(&note, 40).to_vec();
        assert_eq!(plain(&lines), vec!["# Title", "", "plain words", "", "```rust", "let x = 1;", "```"]);
        assert_eq!(lines[2], raw_preview_line("plain words"), "a line no capture touched");
        assert_eq!(style_of(&lines[0], " Title"), raw_line_style("# Title"), "heading text keeps the heading style");
        assert_eq!(style_of(&lines[5], "let"), theme::syntax_keyword());
    }

    #[test]
    fn a_file_without_a_language_stays_raw() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("notes.txt");
        std::fs::write(&file, "# not markdown\nfn main() {}\n").unwrap();
        let mut preview = Preview::default();
        let lines = preview.file_lines(&file, 40).to_vec();
        assert_eq!(lines, vec![raw_preview_line("# not markdown"), raw_preview_line("fn main() {}")]);
        assert_eq!(PreviewKey::of(&file, 40, false).unwrap().language, None);
    }

    #[test]
    fn the_cache_key_carries_the_language_so_a_different_one_rebuilds() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("main.rs");
        std::fs::write(&file, "fn main() {}\n").unwrap();
        let key = PreviewKey::of(&file, 40, false).unwrap();
        assert_eq!(key.language, Some(Language::Rust));
        let note = dir.path().join("n.md");
        std::fs::write(&note, "# A\n").unwrap();
        assert_eq!(PreviewKey::of(&note, 40, false).unwrap().language, Some(Language::Markdown));
        assert_eq!(PreviewKey::of(&note, 40, true).unwrap().language, None, "rendered highlights fences itself");

        let other = PreviewKey { language: Some(Language::Kotlin), ..key.clone() };
        assert_ne!(key, other);
        let mut cache = PreviewCache::default();
        let mut builds = 0;
        for key in [key.clone(), key.clone(), other, key] {
            cache.get(Some(key), || {
                builds += 1;
                Vec::new()
            });
        }
        assert_eq!(builds, 3, "same key reuses, a language change rebuilds");
    }

    #[test]
    fn the_toggle_key_is_a_free_browse_key_listed_once_in_the_view_group() {
        let rows: Vec<&KeyHint> = TREE_KEYS.iter().filter(|k| k.key == "p").collect();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].help, "toggle rendered / raw preview");
        assert_eq!(rows[0].group, Group::View);
        assert_eq!(rows[0].modes, BROWSE);
        for mode in [Mode::Tag, Mode::Filter, Mode::Rename, Mode::NewItem, Mode::ConfirmDelete, Mode::VimCommand, Mode::Move, Mode::SetScope, Mode::ConfirmMove] {
            assert!(!TREE_KEYS.iter().any(|k| k.key == "p" && k.modes.contains(&mode)), "{mode:?}");
        }
    }

    fn shown_with(table: &[KeyHint], width: usize) -> Vec<&'static str> {
        footer::select(table, Mode::Normal, &[], width).left.iter().map(|&(i, _)| table[i].key).collect()
    }

    #[test]
    fn the_toggle_hint_names_the_action_shows_only_for_markdown_and_drops_first() {
        assert!(!shown_with(&tree_keys(None), 300).contains(&"p"), "hidden when the row is not markdown");
        let rendered = tree_keys(Some(PreviewMode::Rendered));
        let raw = tree_keys(Some(PreviewMode::Raw));
        assert_eq!(rendered.len(), TREE_KEYS.len(), "same rows, so help and footer indices agree");
        let line = |table: &[KeyHint]| text_of(&footer::line(table, Mode::Normal, &[], 300));
        assert!(line(&rendered).contains("  p raw  "), "{}", line(&rendered));
        assert!(line(&raw).contains("  p rendered  "), "{}", line(&raw));
        let first_width = |key: &str| (0..300).find(|&w| shown_with(&rendered, w).contains(&key)).unwrap();
        assert!(first_width("p") > first_width("J/K"), "p drops before J/K");
        for mode in [Mode::Filter, Mode::Tag, Mode::Rename, Mode::NewItem] {
            let sel = footer::select(&rendered, mode, &[], 300);
            assert!(!sel.left.iter().any(|&(i, _)| rendered[i].key == "p"), "{mode:?}");
        }
    }

    #[test]
    fn the_browse_footer_leads_with_the_file_type_then_the_mark_count() {
        let keys = tree_keys(Some(PreviewMode::Rendered));
        let both = text_of(&browse_footer(&keys, Some("markdown"), 3, Mode::Normal, &[], 200));
        assert!(both.starts_with(" markdown  3 marked  open  tags"), "{both}");
        let type_only = text_of(&browse_footer(&keys, Some("toml"), 0, Mode::Normal, &[], 200));
        assert!(type_only.starts_with(" toml  open  tags"), "{type_only}");
        let marks_only = text_of(&browse_footer(&keys, None, 2, Mode::Normal, &[], 200));
        assert!(marks_only.starts_with(" 2 marked  open  tags"), "{marks_only}");
        let neither = browse_footer(&TREE_KEYS.to_vec(), None, 0, Mode::Normal, &[], 120);
        assert_eq!(text_of(&neither), text_of(&footer::line(TREE_KEYS, Mode::Normal, &[], 120)));
        for width in 0..30 {
            let _ = browse_footer(&keys, Some("markdown"), 12, Mode::Normal, &[], width);
        }
    }

    fn dir_node(depth: usize) -> TreeNode {
        TreeNode {
            name: "dir".into(),
            path: PathBuf::from("dir"),
            origin: PathBuf::from("dir"),
            is_dir: true,
            depth,
            expanded: false,
            child_count: 0,
            parent_idx: None,
            flags: 0,
            scope_icon: "",
            tag_root: 0,
            section: 0,
        }
    }

    #[test]
    fn any_top_collapsed_decides_the_view_all_toggle() {
        let mut nodes = vec![dir_node(0), dir_node(0)];
        nodes[0].expanded = true;
        assert!(any_top_collapsed(&nodes));
        nodes[1].expanded = true;
        assert!(!any_top_collapsed(&nodes));
    }

    #[test]
    fn view_all_is_lit_only_with_a_top_dir_and_none_collapsed() {
        assert!(!view_all_lit(&[]));
        let mut file = dir_node(0);
        file.is_dir = false;
        assert!(!view_all_lit(&[file]));
        assert!(!view_all_lit(&[dir_node(1)]));
        let mut nodes = vec![dir_node(0), dir_node(0)];
        nodes[0].expanded = true;
        assert!(!view_all_lit(&nodes));
        nodes[1].expanded = true;
        assert!(view_all_lit(&nodes));
    }

    fn text_of(line: &Line) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn tag_footer_draws_the_legend_then_its_hints_with_t_lit() {
        let legend = tag_legend(0b1);
        let legend_text = text_of(&Line::from(legend.clone()));
        let line = lead_with_hints(legend, Mode::Tag, &[Toggle::Tag], 100);
        let rendered = text_of(&line);
        assert!(rendered.starts_with(&legend_text), "{rendered:?}");
        let rest = &rendered[legend_text.len()..];
        for hint in ["tags", "1-5 toggle", "esc close"] {
            assert!(rest.contains(hint), "{hint}: {rendered:?}");
        }
        let lead_spans = tag_legend(0b1).len();
        let t = line.spans[lead_spans..].iter().find(|s| s.content == "t").unwrap();
        assert_eq!(t.style.fg, Some(theme::GREEN));
        assert!(rendered.chars().count() <= 100);
    }

    #[test]
    fn rename_footer_draws_the_prompt_then_its_hints() {
        let rendered = text_of(&lead_with_hints(rename_lead("draft"), Mode::Rename, &[], 100));
        assert!(rendered.starts_with(" rename: draft_ "), "{rendered:?}");
        for hint in ["enter confirm", "esc cancel", "bksp delete"] {
            assert!(rendered.contains(hint), "{hint}: {rendered:?}");
        }
        assert!(!rendered.contains("quit"));
    }

    #[test]
    fn command_footer_draws_the_buffer_then_its_hints() {
        let rendered = text_of(&lead_with_hints(command_lead(":wq"), Mode::VimCommand, &[], 100));
        assert!(rendered.starts_with(":wq "), "{rendered:?}");
        for hint in ["enter run", "esc cancel", "bksp delete"] {
            assert!(rendered.contains(hint), "{hint}: {rendered:?}");
        }
    }

    #[test]
    fn narrow_lead_footers_keep_the_lead_and_drop_hints_first() {
        let cases = [
            (tag_legend(0), Mode::Tag),
            (rename_lead("a fairly long note name"), Mode::Rename),
            (command_lead(":something"), Mode::VimCommand),
        ];
        for (lead, mode) in cases {
            let lead_text = text_of(&Line::from(lead.clone()));
            let lead_cols = lead_text.chars().count();
            let full = text_of(&lead_with_hints(lead.clone(), mode, &[], 200));
            let mut previous = full.chars().count();
            for width in (0..=lead_cols + 40).rev() {
                let rendered = text_of(&lead_with_hints(lead.clone(), mode, &[], width));
                assert!(rendered.starts_with(&lead_text), "{mode:?} width {width}: {rendered:?}");
                assert!(rendered.chars().count() <= width.max(lead_cols), "{mode:?} width {width}");
                assert!(rendered.chars().count() <= previous, "{mode:?} width {width}");
                previous = rendered.chars().count();
            }
            assert_eq!(text_of(&lead_with_hints(lead.clone(), mode, &[], lead_cols)), lead_text);
            assert!(full.chars().count() > lead_cols, "{mode:?} shows hints when wide");
        }
    }

    fn spec(root: &str, label: &str, files: &[&str]) -> SectionSpec {
        SectionSpec {
            root: PathBuf::from(root),
            tag_root: PathBuf::from(root),
            label: label.to_string(),
            icon: "",
            is_doc: false,
            files: files.iter().map(|f| PathBuf::from(root).join(f)).collect(),
            dirs: Vec::new(),
            scope: Scope::Global,
            project: None,
            new_note_root: PathBuf::from(root),
            is_current: false,
        }
    }

    #[test]
    fn a_pull_warning_stays_up_while_one_off_statuses_come_and_go() {
        let warning = Some("vault pull hit a conflict, rebase aborted");
        // The event loop clears the one-off message on every key press; the
        // warning lives in the context and must survive that.
        let mut message: Option<&str> = None;
        assert_eq!(
            status_slot(None, message, false, false, warning),
            StatusSlot::Warning(warning.unwrap()),
            "the warning shows as the session opens"
        );
        message = Some("rename failed: exists");
        assert_eq!(
            status_slot(None, message, false, false, warning),
            StatusSlot::Message("rename failed: exists")
        );
        message = None;
        assert_eq!(
            status_slot(None, message, false, false, warning),
            StatusSlot::Warning(warning.unwrap()),
            "a one-off status clearing must not take the warning with it"
        );
        assert_eq!(
            status_slot(Some("new"), message, false, false, warning),
            StatusSlot::Rename("new")
        );
        assert_eq!(
            status_slot(None, message, true, false, warning),
            StatusSlot::VimCommand
        );
        assert_eq!(
            status_slot(None, message, false, true, warning),
            StatusSlot::Tags
        );
        assert_eq!(
            status_slot(None, message, false, false, warning),
            StatusSlot::Warning(warning.unwrap()),
            "the warning comes back once rename, vim and tag mode end"
        );
        assert_eq!(
            status_slot(None, None, false, false, None),
            StatusSlot::Hints
        );
    }

    #[test]
    fn forest_orders_numbered_dirs_then_other_dirs_then_files() {
        let s = spec(
            "/r",
            "S",
            &[
                "zzz-other/z.md",
                "00_quick-notes/a.md",
                "top.md",
                "01_daily-logs/b.md",
            ],
        );
        let (nodes, _) = build_forest(&[s]);
        let names: Vec<&str> = nodes.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "S",
                "00_quick-notes",
                "a.md",
                "01_daily-logs",
                "b.md",
                "zzz-other",
                "z.md",
                "top.md",
            ],
        );
    }

    #[test]
    fn forest_depth_and_parents_are_positional() {
        let s = spec("/r", "S", &["dir/sub/deep.md"]);
        let (nodes, _) = build_forest(&[s]);
        // wrapper(0) > dir(1) > sub(2) > deep.md(3)
        assert_eq!(nodes[1].depth, 1);
        assert_eq!(nodes[2].depth, 2);
        assert_eq!(nodes[3].depth, 3);
        assert_eq!(nodes[3].parent_idx, Some(2));
        assert_eq!(nodes[2].parent_idx, Some(1));
        assert_eq!(nodes[1].parent_idx, Some(0));
        assert!(!nodes[3].is_dir);
        assert_eq!(nodes[0].child_count, 1);
    }

    #[test]
    fn visibility_respects_collapsed_wrappers() {
        let s = spec("/r", "S", &["dir/a.md", "b.md"]);
        let (mut nodes, _) = build_forest(&[s]);
        // Everything starts collapsed: only the wrapper shows.
        assert_eq!(get_visible_nodes(&nodes), vec![0]);
        nodes[0].expanded = true;
        let vis = get_visible_nodes(&nodes);
        // Wrapper, dir (collapsed), b.md; not dir/a.md.
        assert_eq!(vis.len(), 3);
    }

    #[test]
    fn filter_keeps_matches_and_ancestors() {
        let s = spec("/r", "S", &["dir/target.md", "dir/other.md"]);
        let (nodes, _) = build_forest(&[s]);
        let f = filter::parse("target");
        let keep = compute_filter_keep(&nodes, &f);
        let kept: Vec<&str> = nodes
            .iter()
            .zip(&keep)
            .filter(|(_, k)| **k)
            .map(|(n, _)| n.name.as_str())
            .collect();
        assert!(kept.contains(&"target.md"));
        assert!(kept.contains(&"dir"));
        assert!(kept.contains(&"S"));
        assert!(!kept.contains(&"other.md"));
    }

    #[test]
    fn derive_dir_flags_aggregates_and_clears() {
        let s = spec("/r", "S", &["dir/a.md"]);
        let (mut nodes, _) = build_forest(&[s]);
        let file = nodes.iter().position(|n| n.name == "a.md").unwrap();
        nodes[file].flags = FLAG_PRIO;
        derive_dir_flags(&mut nodes);
        assert_eq!(nodes[0].flags, FLAG_PRIO);
        nodes[file].flags = 0;
        derive_dir_flags(&mut nodes);
        assert_eq!(nodes[0].flags, 0, "stale bits must drop");
    }

    #[test]
    fn changed_tag_maps_reports_nothing_without_edits() {
        let s = spec("/r", "S", &["a.md"]);
        let (mut nodes, roots) = build_forest(&[s]);
        let initial = vec![HashMap::from([("a.md".to_string(), FLAG_PRIO)])];
        apply_tags(&mut nodes, &roots, &initial);
        assert!(changed_tag_maps(&nodes, &roots, &initial).is_empty());
    }

    #[test]
    fn changed_tag_maps_preserves_unknown_keys() {
        let s = spec("/r", "S", &["a.md"]);
        let (mut nodes, roots) = build_forest(&[s]);
        // An entry for a file this view does not show (e.g. another scope).
        let initial = vec![HashMap::from([(
            "elsewhere/hidden.md".to_string(),
            FLAG_IMPORTANT,
        )])];
        apply_tags(&mut nodes, &roots, &initial);
        let file = nodes.iter().position(|n| n.name == "a.md").unwrap();
        nodes[file].flags = FLAG_PRIO;
        let changed = changed_tag_maps(&nodes, &roots, &initial);
        assert_eq!(changed.len(), 1);
        let map = &changed[0].1;
        assert_eq!(map.get("a.md"), Some(&FLAG_PRIO));
        assert_eq!(
            map.get("elsewhere/hidden.md"),
            Some(&FLAG_IMPORTANT),
            "keys outside the view must survive a save",
        );
    }

    #[test]
    fn sections_share_a_dedup_tag_root() {
        let a = spec("/notez", "GLOBAL", &["top.md"]);
        let mut b = spec("/notez/personal/p", "p (personal)", &["n.md"]);
        b.tag_root = PathBuf::from("/notez");
        let (nodes, roots) = build_forest(&[a, b]);
        assert_eq!(roots.len(), 1);
        assert!(nodes.iter().all(|n| n.tag_root == 0));
    }

    #[test]
    fn renamed_note_moves_its_tag_key() {
        let s = spec("/r", "S", &["2026-10-06-untitled.md"]);
        let (mut nodes, roots) = build_forest(&[s]);
        let initial = vec![HashMap::from([(
            "2026-10-06-untitled.md".to_string(),
            FLAG_PRIO,
        )])];
        apply_tags(&mut nodes, &roots, &initial);

        nodes[1].path = PathBuf::from("/r/2026-10-06-real.md");
        let changed = changed_tag_maps(&nodes, &roots, &initial);

        assert_eq!(changed.len(), 1);
        assert_eq!(
            changed[0].1,
            HashMap::from([("2026-10-06-real.md".to_string(), FLAG_PRIO)])
        );
    }

    // --- New note ---

    fn scoped(root: &str, scope: Scope, is_doc: bool, files: &[&str]) -> SectionSpec {
        let project = (scope != Scope::Global).then(|| "proj".to_string());
        SectionSpec {
            scope,
            project,
            is_doc,
            new_note_root: if is_doc {
                PathBuf::from("/n/personal/proj")
            } else {
                PathBuf::from(root)
            },
            ..spec(root, root, files)
        }
    }

    /// A project view plus the global store: personal, public, docs,
    /// scratch and global sections.
    fn project_sections() -> Vec<SectionSpec> {
        vec![
            scoped("/n/personal/proj", Scope::Personal, false, &["ideas/a.md", "top.md"]),
            scoped("/p/notez", Scope::Public, false, &["plans/b.md"]),
            scoped("/p/docs", Scope::Public, true, &["design/c.md"]),
            scoped("/p/.notez", Scope::Local, false, &["d.md"]),
            scoped("/n", Scope::Global, false, &["e.md"]),
        ]
    }

    fn row(nodes: &[TreeNode], path: &str) -> usize {
        nodes
            .iter()
            .position(|n| n.path == Path::new(path))
            .unwrap_or_else(|| panic!("no row {path}"))
    }

    fn target_at(path: &str, current: Option<&str>) -> NewNoteTarget {
        let sections = project_sections();
        let (nodes, _) = build_forest(&sections);
        new_note_target(&nodes, &sections, row(&nodes, path), current).unwrap()
    }

    fn target(dir: &str, scope: Scope, label: &str) -> NewNoteTarget {
        NewNoteTarget { dir: PathBuf::from(dir), scope, label: label.to_string() }
    }

    #[test]
    fn new_note_targets_the_folder_under_the_cursor_in_its_section() {
        let here = Some("proj");
        let ideas = target("/n/personal/proj/ideas", Scope::Personal, "personal/ideas");
        assert_eq!(target_at("/n/personal/proj/ideas", here), ideas);
        assert_eq!(target_at("/n/personal/proj/ideas/a.md", here), ideas);
        let personal_root = target("/n/personal/proj", Scope::Personal, "personal");
        assert_eq!(target_at("/n/personal/proj", here), personal_root);
        assert_eq!(target_at("/n/personal/proj/top.md", here), personal_root);
        assert_eq!(
            target_at("/p/notez/plans/b.md", here),
            target("/p/notez/plans", Scope::Public, "public (committed with the project)/plans")
        );
        assert_eq!(
            target_at("/p/notez", here),
            target("/p/notez", Scope::Public, "public (committed with the project)")
        );
        assert_eq!(target_at("/p/.notez/d.md", here), target("/p/.notez", Scope::Local, "local scratch"));
        assert_eq!(target_at("/n/e.md", here), target("/n", Scope::Global, "global"));
        assert_eq!(target_at("/n", here), target("/n", Scope::Global, "global"));
    }

    #[test]
    fn new_note_from_a_docs_section_goes_to_the_personal_root() {
        for path in ["/p/docs", "/p/docs/design", "/p/docs/design/c.md"] {
            assert_eq!(
                target_at(path, Some("proj")),
                target("/n/personal/proj", Scope::Personal, "personal"),
                "{path}"
            );
        }
    }

    #[test]
    fn new_note_label_names_a_project_other_than_the_current_one() {
        for current in [None, Some("other")] {
            assert_eq!(target_at("/n/personal/proj/ideas", current).label, "personal (proj)/ideas");
            assert_eq!(
                target_at("/p/notez/plans", current).label,
                "public (committed with proj)/plans"
            );
            assert_eq!(target_at("/p/.notez", current).label, "local scratch (proj)");
            assert_eq!(target_at("/n", current).label, "global");
        }
    }

    #[test]
    fn new_note_label_says_public_for_every_public_row() {
        let sections = project_sections();
        let (nodes, _) = build_forest(&sections);
        for (i, node) in nodes.iter().enumerate() {
            for current in [None, Some("proj"), Some("other")] {
                let t = new_note_target(&nodes, &sections, i, current).unwrap();
                if t.scope == Scope::Public {
                    assert!(t.label.starts_with("public (committed with"), "{:?}", node.path);
                    assert!(t.dir.starts_with("/p/notez"), "{:?}", node.path);
                }
            }
        }
    }

    #[test]
    fn new_note_target_is_none_past_the_last_row() {
        let sections = project_sections();
        let (nodes, _) = build_forest(&sections);
        assert_eq!(new_note_target(&nodes, &sections, nodes.len(), None), None);
        assert_eq!(new_note_target(&[], &[], 0, None), None);
    }

    #[test]
    fn rebuild_keeps_expansion_and_tags_and_selects_the_new_note() {
        let before = vec![spec("/r", "S", &["a/one.md", "b/two.md", "c/three.md"])];
        let (mut old, _) = build_forest(&before);
        let wrapper = row(&old, "/r");
        old[wrapper].expanded = true;
        let b = row(&old, "/r/b");
        old[b].expanded = true;
        let two = row(&old, "/r/b/two.md");
        old[two].flags = FLAG_IMPORTANT;
        let three = row(&old, "/r/c/three.md");
        old[three].origin = PathBuf::from("/r/c/old-three.md");

        let after = vec![spec("/r", "S", &["a/new.md", "a/one.md", "b/two.md", "c/three.md"])];
        let (mut new, _) = build_forest(&after);
        let selected = restore_state(&old, &mut new, Path::new("/r/a/new.md"));

        assert_eq!(selected, Some(row(&new, "/r/a/new.md")));
        assert!(new[row(&new, "/r")].expanded);
        assert!(new[row(&new, "/r/a")].expanded, "the new note's folder opens");
        assert!(new[row(&new, "/r/b")].expanded, "an open folder stays open");
        assert!(!new[row(&new, "/r/c")].expanded, "a closed folder stays closed");
        assert_eq!(new[row(&new, "/r/b/two.md")].flags, FLAG_IMPORTANT);
        assert_eq!(new[row(&new, "/r/c/three.md")].origin, PathBuf::from("/r/c/old-three.md"));
        assert!(get_visible_nodes(&new).contains(&selected.unwrap()));
    }

    #[test]
    fn rebuild_keeps_unsaved_tag_edits_for_the_save_on_exit() {
        let before = vec![spec("/r", "S", &["one.md"])];
        let (mut old, roots) = build_forest(&before);
        let initial = vec![HashMap::new()];
        apply_tags(&mut old, &roots, &initial);
        let one = row(&old, "/r/one.md");
        old[one].flags = FLAG_PRIO;

        let mut forest = Forest { sections: before, nodes: old, tag_roots: roots, initial };
        let created = forest.rebuild(vec![spec("/r", "S", &["new.md", "one.md"])], Path::new("/r/new.md"));

        assert_eq!(created, Some(row(&forest.nodes, "/r/new.md")));
        let changed = changed_tag_maps(&forest.nodes, &forest.tag_roots, &forest.initial);
        assert_eq!(changed, vec![(PathBuf::from("/r"), HashMap::from([("one.md".to_string(), FLAG_PRIO)]))]);
    }

    #[test]
    fn rebuild_without_the_new_note_selects_nothing() {
        let sections = vec![spec("/r", "S", &["one.md"])];
        let (old, _) = build_forest(&sections);
        let (mut new, _) = build_forest(&sections);
        assert_eq!(restore_state(&old, &mut new, Path::new("/r/missing.md")), None);
    }

    #[test]
    fn focus_rows_follow_their_paths_across_a_rebuild() {
        let (old, _) = build_forest(&[spec("/r", "S", &["b/x.md"])]);
        let (new, _) = build_forest(&[spec("/r", "S", &["a/y.md", "b/x.md"])]);
        let rows = vec![(row(&old, "/r/b"), true), (99, false)];
        assert_eq!(remap_rows(&old, &new, &rows), vec![(row(&new, "/r/b"), true)]);
    }

    #[test]
    fn new_note_footer_draws_the_prompt_then_its_hints() {
        assert_eq!(shown_keys(Mode::NewItem, &[], 200), vec!["enter", "esc", "tab", "bksp"]);
        let lead = new_note_lead("public (committed with the project)/plans", "draft");
        let rendered = text_of(&lead_with_hints(lead, Mode::NewItem, &[], 120));
        assert!(
            rendered.starts_with(" new note in public (committed with the project)/plans: draft_"),
            "{rendered}"
        );
        assert!(rendered.contains("create") && rendered.contains("cancel"), "{rendered}");
    }

    #[test]
    fn n_is_a_browse_key_listed_in_help() {
        let rows: Vec<&KeyHint> = TREE_KEYS.iter().filter(|k| k.key == "n").collect();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].modes, BROWSE);
        assert_eq!(rows[0].group, Group::Edit);
        let prompt_keys: Vec<&str> = TREE_KEYS
            .iter()
            .filter(|k| k.modes.contains(&Mode::NewItem))
            .map(|k| k.key)
            .collect();
        assert_eq!(prompt_keys, vec!["enter", "esc", "tab", "bksp"]);
    }

    /// `n` is a primary action: the normal footer shows it, and on a narrow
    /// terminal it drops after the other keys but before open and tags.
    #[test]
    fn n_shows_in_the_footer_and_drops_before_open_and_tags() {
        assert!(shown_keys(Mode::Normal, &[], 200).contains(&"n"));
        assert!(shown_keys(Mode::Focus, &[], 200).contains(&"n"));
        assert!(!shown_keys(Mode::Tag, &[], 200).contains(&"n"));
        let mut saw_n_without_r = false;
        for width in 20..200 {
            let keys = shown_keys(Mode::Normal, &[], width);
            if keys.contains(&"n") {
                assert!(keys.contains(&"o") && keys.contains(&"t"), "width {width}: {keys:?}");
                saw_n_without_r |= !keys.contains(&"r");
            }
        }
        assert!(saw_n_without_r, "rename drops before new");
    }

    // --- Tab scope cycling and the empty tree ---

    fn roots() -> NewNoteRoots {
        NewNoteRoots {
            global: PathBuf::from("/n"),
            projects: HashMap::from([(
                "proj".to_string(),
                vec![
                    (Scope::Personal, PathBuf::from("/n/personal/proj")),
                    (Scope::Public, PathBuf::from("/p/notez")),
                    (Scope::Local, PathBuf::from("/p/.notez")),
                ],
            )]),
        }
    }

    /// Press `Tab` `presses` times on the prompt `n` opens at `path`; returns
    /// the target after each press.
    fn tab_through(path: &str, current: Option<&str>, presses: usize) -> Vec<NewNoteTarget> {
        let sections = project_sections();
        let (nodes, _) = build_forest(&sections);
        let ctx = ctx_with(current, roots());
        let mut prompt = open_new_note_prompt(&nodes, &sections, Some(row(&nodes, path)), &ctx);
        (0..presses)
            .map(|_| {
                prompt.target = next_scope_target(
                    &prompt.target,
                    &prompt.origin,
                    prompt.project.as_deref(),
                    &ctx.new_note_roots,
                    current,
                );
                prompt.target.clone()
            })
            .collect()
    }

    fn ctx_with(current: Option<&str>, new_note_roots: NewNoteRoots) -> TreeContext {
        TreeContext {
            title: String::new(),
            path_display: String::new(),
            warning: None,
            current_project: current.map(str::to_string),
            new_note_roots,
        }
    }

    #[test]
    fn tab_cycles_scope_roots_and_returns_to_the_original_folder() {
        let public = "public (committed with the project)";
        assert_eq!(
            tab_through("/n/personal/proj/ideas/a.md", Some("proj"), 4),
            vec![
                target("/p/notez", Scope::Public, public),
                target("/p/.notez", Scope::Local, "local scratch"),
                target("/n", Scope::Global, "global"),
                target("/n/personal/proj/ideas", Scope::Personal, "personal/ideas"),
            ]
        );
        assert_eq!(
            tab_through("/p/notez/plans", Some("proj"), 4),
            vec![
                target("/p/.notez", Scope::Local, "local scratch"),
                target("/n", Scope::Global, "global"),
                target("/n/personal/proj", Scope::Personal, "personal"),
                target("/p/notez/plans", Scope::Public, &format!("{public}/plans")),
            ]
        );
    }

    #[test]
    fn tab_from_a_docs_section_starts_at_the_personal_root() {
        let cycle = tab_through("/p/docs/design/c.md", Some("proj"), 4);
        assert_eq!(cycle[0].scope, Scope::Public);
        assert_eq!(cycle[3], target("/n/personal/proj", Scope::Personal, "personal"));
    }

    #[test]
    fn tab_names_the_project_in_another_projects_cycle() {
        let cycle = tab_through("/p/.notez/d.md", None, 2);
        assert_eq!(cycle[0], target("/n", Scope::Global, "global"));
        assert_eq!(cycle[1], target("/n/personal/proj", Scope::Personal, "personal (proj)"));
    }

    #[test]
    fn tab_outside_any_project_stays_global() {
        assert_eq!(
            tab_through("/n/e.md", Some("proj"), 2),
            vec![target("/n", Scope::Global, "global"); 2]
        );
    }

    /// A project the browser knows no repository for offers its personal
    /// store and global only: no public or scratch path is guessed.
    #[test]
    fn tab_without_a_known_repository_offers_personal_and_global() {
        let sections = project_sections();
        let (nodes, _) = build_forest(&sections);
        let ctx = ctx_with(None, NewNoteRoots { global: PathBuf::from("/n"), ..NewNoteRoots::default() });
        let prompt = open_new_note_prompt(&nodes, &sections, Some(row(&nodes, "/n/personal/proj")), &ctx);
        let next = next_scope_target(&prompt.target, &prompt.origin, Some("proj"), &ctx.new_note_roots, None);
        assert_eq!(next, target("/n", Scope::Global, "global"));
        let back = next_scope_target(&next, &prompt.origin, Some("proj"), &ctx.new_note_roots, None);
        assert_eq!(back, prompt.origin);
    }

    #[test]
    fn n_on_an_empty_tree_targets_the_personal_root_in_a_project() {
        let (nodes, _) = build_forest(&[]);
        assert!(nodes.is_empty());
        let in_project = open_new_note_prompt(&nodes, &[], None, &ctx_with(Some("proj"), roots()));
        assert_eq!(in_project.target, target("/n/personal/proj", Scope::Personal, "personal"));
        assert_eq!(in_project.project.as_deref(), Some("proj"));
        let outside = open_new_note_prompt(&nodes, &[], None, &ctx_with(None, roots()));
        assert_eq!(outside.target, target("/n", Scope::Global, "global"));
        let unknown = open_new_note_prompt(&nodes, &[], Some(0), &ctx_with(Some("x"), roots()));
        assert_eq!(unknown.target, target("/n/personal/x", Scope::Personal, "personal"));
    }

    /// The helpers the event loop calls on every key and frame accept an
    /// empty forest without indexing into it.
    #[test]
    fn empty_tree_helpers_do_not_panic() {
        let (mut nodes, roots) = build_forest(&[]);
        assert!(roots.is_empty());
        assert!(get_visible_nodes(&nodes).is_empty());
        assert!(compute_visible(&nodes, "").is_empty());
        assert!(compute_visible(&nodes, "#1 x").is_empty());
        assert_eq!(find_top_dir(&nodes, 0), None);
        assert!(!any_top_collapsed(&nodes));
        assert!(!view_all_lit(&nodes));
        assert_eq!(restore_state(&[], &mut nodes, Path::new("/n/a.md")), None);
        assert!(remap_rows(&[], &nodes, &[(0, true)]).is_empty());
        assert_eq!(empty_state_line(&nodes), Some(EMPTY_STATE));
        let (some, _) = build_forest(&project_sections());
        assert_eq!(empty_state_line(&some), None);
    }

    #[test]
    fn a_narrowed_empty_view_names_its_scope() {
        let cases = [
            (Scope::Personal, "no personal notes here yet: n creates one"),
            (Scope::Public, "no public notes here yet: n creates one"),
            (Scope::Local, "no scratch notes here yet: n creates one"),
            (Scope::Global, "no global notes here yet: n creates one"),
        ];
        for (scope, expected) in cases {
            let title = format!("{} notez (proj)", scope.icon());
            assert_eq!(empty_state_text(&title), expected);
        }
    }

    #[test]
    fn the_all_view_keeps_the_plain_empty_state() {
        assert_eq!(empty_state_text("notez (proj)"), EMPTY_STATE);
        assert_eq!(empty_state_text("notez"), EMPTY_STATE);
        assert_eq!(empty_state_text(""), EMPTY_STATE);
    }

    // --- Initial expansion ---

    /// The section rows of `nodes` with their expanded state.
    fn section_rows(nodes: &[TreeNode]) -> Vec<(&str, bool)> {
        nodes
            .iter()
            .filter(|n| n.depth == 0)
            .map(|n| (path_str(&n.path), n.expanded))
            .collect()
    }

    #[test]
    fn the_current_repositorys_sections_open_expanded_and_the_rest_collapsed() {
        let mut sections = project_sections();
        sections[0].is_current = true;
        sections[1].is_current = true;
        let (mut nodes, _) = build_forest(&sections);
        open_current_sections(&mut nodes, &sections);
        assert_eq!(
            section_rows(&nodes),
            vec![
                ("/n/personal/proj", true),
                ("/p/notez", true),
                ("/p/docs", false),
                ("/p/.notez", false),
                ("/n", false),
            ],
        );
        assert!(
            nodes.iter().filter(|n| n.depth > 0 && n.is_dir).all(|n| !n.expanded),
            "folders inside a section stay collapsed",
        );
    }

    #[test]
    fn with_no_current_section_every_section_opens_as_before() {
        let sections = project_sections();
        let (mut nodes, _) = build_forest(&sections);
        open_current_sections(&mut nodes, &sections);
        let (built, _) = build_forest(&sections);
        assert_eq!(section_rows(&nodes), section_rows(&built));
    }

    #[test]
    fn a_rebuild_keeps_the_users_expansion_over_the_initial_one() {
        let current = || {
            let mut sections = project_sections();
            sections[0].is_current = true;
            sections
        };
        let sections = current();
        let (mut nodes, tag_roots) = build_forest(&sections);
        open_current_sections(&mut nodes, &sections);
        let initial = vec![HashMap::new(); tag_roots.len()];
        let mut forest = Forest { sections, nodes, tag_roots, initial };
        let personal = row(&forest.nodes, "/n/personal/proj");
        let global = row(&forest.nodes, "/n");
        forest.nodes[personal].expanded = false;
        forest.nodes[global].expanded = true;

        forest.rebuild(current(), Path::new("/n/none.md"));

        assert!(!forest.nodes[row(&forest.nodes, "/n/personal/proj")].expanded);
        assert!(forest.nodes[row(&forest.nodes, "/n")].expanded);
    }

    // --- Scope badges ---

    /// A fixed list text width for row tests. It fits the 80-column
    /// [`render_row`] list; the event loop uses [`list_text_width`].
    const LIST_TEXT_WIDTH: usize = 80 - 6;

    /// [`project_sections`] with the icons the real listing gives them, and
    /// every row expanded.
    fn badged_forest() -> (Vec<SectionSpec>, Vec<TreeNode>) {
        let mut sections = project_sections();
        for spec in &mut sections {
            spec.icon = if spec.is_doc { "\u{f02d}" } else { spec.scope.icon() };
        }
        let (mut nodes, _) = build_forest(&sections);
        for node in &mut nodes {
            node.expanded = true;
        }
        (sections, nodes)
    }

    /// Row `idx` as the event loop draws it in a list `width` columns wide,
    /// with the rows `compute_visible` gives for no filter and no marks.
    fn line_at_width(sections: &[SectionSpec], nodes: &[TreeNode], idx: usize, width: usize) -> Line<'static> {
        let visible = compute_visible(nodes, "");
        let pos = visible.iter().position(|&i| i == idx).expect("a visible row");
        list_lines(nodes, sections, &visible, &HashSet::new(), width).swap_remove(pos)
    }

    /// [`line_at_width`] at [`LIST_TEXT_WIDTH`].
    fn line_of(sections: &[SectionSpec], nodes: &[TreeNode], idx: usize) -> Line<'static> {
        line_at_width(sections, nodes, idx, LIST_TEXT_WIDTH)
    }

    /// `line` drawn as the selected row of an 80-column list, the way the
    /// event loop draws it: the cells of that row.
    fn render_row(line: Line<'static>) -> Vec<(String, Option<Color>)> {
        render_row_styled(line).into_iter().map(|(symbol, style)| (symbol, style.fg)).collect()
    }

    /// [`render_row`] with each cell's whole style.
    fn render_row_styled(line: Line<'static>) -> Vec<(String, Style)> {
        use ratatui::buffer::Buffer;
        let area = Rect::new(0, 0, 80, 1);
        let mut buf = Buffer::empty(area);
        let list = List::new(vec![ListItem::new(line)]).highlight_style(theme::selected_row());
        let mut state = ListState::default();
        state.select(Some(0));
        StatefulWidget::render(list, area, &mut buf, &mut state);
        (0..80)
            .map(|x| {
                let cell = &buf[(x, 0)];
                (cell.symbol().to_string(), cell.style())
            })
            .collect()
    }

    /// The first column of a row: blank, or the mark of a marked row.
    const GUTTER_COL: usize = 0;

    #[test]
    fn every_file_row_has_a_badge_in_its_scope_colour_and_the_rest_unchanged() {
        let (sections, nodes) = badged_forest();
        let cases = [
            ("/n/personal/proj/top.md", Scope::Personal.icon(), Scope::Personal, "└─  "),
            ("/p/notez/plans/b.md", Scope::Public.icon(), Scope::Public, "  └─  "),
            ("/p/docs/design/c.md", "\u{f02d}", Scope::Public, "  └─  "),
            ("/p/.notez/d.md", Scope::Local.icon(), Scope::Local, "└─  "),
            ("/n/e.md", Scope::Global.icon(), Scope::Global, "└─  "),
        ];
        for (path, icon, colour_scope, branch) in cases {
            let node = &nodes[row(&nodes, path)];
            let cells = render_row(line_of(&sections, &nodes, row(&nodes, path)));
            assert_eq!(cells[GUTTER_COL].0, " ", "{path}: the gutter keeps a blank");
            let badge = GUTTER_COL + 1 + Span::raw(branch).width();
            assert_eq!(cells[badge].0, icon, "{path}");
            assert_eq!(cells[badge].1, Some(theme::scope_color(colour_scope)), "{path}");
            let rest: String = cells[GUTTER_COL + 1..].iter().map(|c| c.0.as_str()).collect();
            let expected = format!("{branch}{icon} {}", node.name);
            assert_eq!(rest.trim_end(), expected, "{path}");
        }
    }

    #[test]
    fn folder_rows_have_the_badge_too() {
        let (sections, nodes) = badged_forest();
        let line = line_of(&sections, &nodes, row(&nodes, "/n/personal/proj/ideas"));
        let cells = render_row(line);
        assert_eq!(cells[GUTTER_COL].0, " ", "the gutter keeps a blank");
        let badge = GUTTER_COL + 1 + 4;
        assert_eq!(cells[badge].0, Scope::Personal.icon());
        assert_eq!(cells[badge].1, Some(theme::scope_color(Scope::Personal)));
        let rest: String = cells[GUTTER_COL + 1..].iter().map(|c| c.0.as_str()).collect();
        let expected = format!("├─▾ {} ideas ", Scope::Personal.icon());
        assert!(rest.starts_with(&expected), "{rest:?}");
        assert!(rest.trim_end().ends_with('1'), "{rest:?}");
    }

    #[test]
    fn a_section_header_shows_its_scope_by_icon_and_colour_without_the_scope_word() {
        let (sections, nodes) = badged_forest();
        for (i, spec) in sections.iter().enumerate() {
            let idx = nodes.iter().position(|n| n.depth == 0 && n.section == i).unwrap();
            let node = &nodes[idx];
            let line = line_of(&sections, &nodes, idx);
            let word = spec.scope.label();
            let colour = Some(theme::scope_color(spec.scope));
            assert!(line.spans.iter().all(|s| s.content != word), "{word}: {:?}", span_texts(&line));
            let icon_span =
                line.spans.iter().find(|s| s.content.starts_with(spec.icon)).expect("icon");
            assert_eq!(icon_span.style.fg, colour, "{word} icon");

            let cells = render_row(line);
            assert_eq!(cells[GUTTER_COL].0, " ", "{word}: no second icon on the header");
            let rest: String = cells[GUTTER_COL + 1..].iter().map(|c| c.0.as_str()).collect();
            let expected = format!("▼ {} {} ··", spec.icon, spec.label);
            assert!(rest.starts_with(&expected), "{rest:?}");
            assert!(rest.trim_end().ends_with(&node.child_count.to_string()), "{rest:?}");
        }
    }

    #[test]
    fn a_click_on_a_tag_dot_still_toggles_that_tag_past_the_badge() {
        let (sections, mut nodes) = badged_forest();
        let idx = row(&nodes, "/p/.notez/d.md");
        let prio = FLAG_DEFS.iter().position(|d| d.bit == FLAG_PRIO).unwrap();
        nodes[idx].flags = FLAG_PRIO;
        let cells = render_row(line_of(&sections, &nodes, idx));
        let lit = cells.iter().position(|c| c.0 == "●").expect("a lit dot");
        assert_eq!(mouse_x_to_row_tag(lit as u16, 0, FLAG_PRIO), Some(prio));
        let badge = cells.iter().position(|c| c.0 == Scope::Local.icon()).expect("the badge");
        assert_eq!(mouse_x_to_row_tag(badge as u16, 0, FLAG_PRIO), None, "the badge is no dot");
    }

    // --- Todo icon (NZ-24) ---

    /// A personal section with a project `TODO.md`, a lowercase `todo.md`
    /// and an ordinary note, and the global store with the todo board's
    /// `_todos` (a note and a category folder) beside an ordinary note;
    /// icons as the real listing gives them, every row expanded.
    fn todo_icon_forest() -> (Vec<SectionSpec>, Vec<TreeNode>) {
        let mut personal =
            scoped("/n/personal/proj", Scope::Personal, false, &["TODO.md", "todo.md", "a.md"]);
        personal.icon = Scope::Personal.icon();
        let mut global = scoped("/n", Scope::Global, false, &["_todos/t.md", "_todos/work/w.md", "e.md"]);
        global.icon = Scope::Global.icon();
        let sections = vec![personal, global];
        let (mut nodes, _) = build_forest(&sections);
        for node in &mut nodes {
            node.expanded = true;
        }
        (sections, nodes)
    }

    /// The badge `path`'s row draws right before its name: text and colour.
    fn badge_of(sections: &[SectionSpec], nodes: &[TreeNode], path: &str) -> (String, Option<Color>) {
        let node = &nodes[row(nodes, path)];
        let line = line_of(sections, nodes, row(nodes, path));
        let badge = &line.spans[name_index(&line, node) - 1];
        (badge.content.to_string(), badge.style.fg)
    }

    #[test]
    fn the_todo_store_and_every_row_under_it_wear_the_todo_icon_in_the_scope_colour() {
        let (sections, nodes) = todo_icon_forest();
        let todo = (format!("{} ", theme::ICON_TODO), Some(theme::scope_color(Scope::Global)));
        for path in ["/n/_todos", "/n/_todos/t.md", "/n/_todos/work", "/n/_todos/work/w.md"] {
            assert_eq!(badge_of(&sections, &nodes, path), todo, "{path}");
        }
        let cells = render_row(line_of(&sections, &nodes, row(&nodes, "/n/_todos/t.md")));
        let badge = GUTTER_COL + 1 + 6;
        assert_eq!(cells[badge].0, theme::ICON_TODO);
        assert_eq!(cells[badge].1, Some(theme::scope_color(Scope::Global)));
        let rest: String = cells[GUTTER_COL + 1..].iter().map(|c| c.0.as_str()).collect();
        assert_eq!(rest.trim_end(), format!("│ └─  {} t.md", theme::ICON_TODO));
    }

    #[test]
    fn a_project_todo_md_wears_the_todo_icon_and_a_lowercase_todo_md_does_not() {
        let (sections, nodes) = todo_icon_forest();
        let personal = Some(theme::scope_color(Scope::Personal));
        let todo = format!("{} ", theme::ICON_TODO);
        let scope_badge = format!("{} ", Scope::Personal.icon());
        assert_eq!(badge_of(&sections, &nodes, "/n/personal/proj/TODO.md"), (todo, personal));
        assert_eq!(badge_of(&sections, &nodes, "/n/personal/proj/todo.md"), (scope_badge.clone(), personal));
        assert_eq!(badge_of(&sections, &nodes, "/n/personal/proj/a.md"), (scope_badge, personal));
        let global = (format!("{} ", Scope::Global.icon()), Some(theme::scope_color(Scope::Global)));
        assert_eq!(badge_of(&sections, &nodes, "/n/e.md"), global);
    }

    #[test]
    fn section_headers_keep_their_scope_icon_next_to_the_todo_rows() {
        let (sections, nodes) = todo_icon_forest();
        for (idx, node) in nodes.iter().enumerate().filter(|(_, n)| n.depth == 0) {
            let line = line_of(&sections, &nodes, idx);
            let texts = span_texts(&line);
            let icon = sections[node.section].icon;
            assert!(texts.contains(&format!("{icon} ")), "{texts:?}");
            assert!(texts.iter().all(|t| !t.contains(theme::ICON_TODO)), "{texts:?}");
        }
    }

    #[test]
    fn the_todo_icon_is_as_wide_as_the_scope_icons_and_todo_counts_still_align() {
        let width = Span::raw(theme::ICON_TODO).width();
        assert_eq!(width, 1);
        for scope in [Scope::Local, Scope::Personal, Scope::Public, Scope::Global] {
            assert_eq!(Span::raw(scope.icon()).width(), width, "{scope:?}");
        }
        let (sections, nodes) = todo_icon_forest();
        for path in ["/n/_todos", "/n/_todos/work"] {
            let node = &nodes[row(&nodes, path)];
            let line = line_of(&sections, &nodes, row(&nodes, path));
            assert_eq!(line.width(), LIST_TEXT_WIDTH, "{path}: {:?}", span_texts(&line));
            assert_eq!(line.spans.last().unwrap().content, node.child_count.to_string(), "{path}");
        }
    }

    // --- Row alignment ---

    /// One personal section with folders at depth 1 and 2, a file beside
    /// the depth 2 folder, and folder names with multi-byte and wide
    /// characters; badges as the real listing gives them, every row expanded.
    fn aligned_forest() -> (Vec<SectionSpec>, Vec<TreeNode>) {
        let mut spec = scoped(
            "/n/personal/proj",
            Scope::Personal,
            false,
            &["ideas/deep/x.md", "ideas/a.md", "åäö/b.md", "日本語/c.md"],
        );
        spec.icon = Scope::Personal.icon();
        let sections = vec![spec];
        let (mut nodes, _) = build_forest(&sections);
        for node in &mut nodes {
            node.expanded = true;
        }
        (sections, nodes)
    }

    fn span_texts(line: &Line<'static>) -> Vec<String> {
        line.spans.iter().map(|s| s.content.to_string()).collect()
    }

    /// The screen column where `line`'s span `index` starts.
    fn column_of(line: &Line<'static>, index: usize) -> usize {
        line.spans[..index].iter().map(Span::width).sum()
    }

    fn name_index(line: &Line<'static>, node: &TreeNode) -> usize {
        line.spans.iter().position(|s| s.content == node.name).expect("name span")
    }

    #[test]
    fn a_nested_row_draws_its_badge_right_before_the_name_and_keeps_the_gutter() {
        let (sections, nodes) = aligned_forest();
        let colour = Some(theme::scope_color(Scope::Personal));
        for (idx, node) in nodes.iter().enumerate().filter(|(_, n)| n.depth > 0) {
            let line = line_of(&sections, &nodes, idx);
            let name = name_index(&line, node);
            let path = node.path.display();
            let badge = format!("{} ", Scope::Personal.icon());
            assert_eq!(line.spans[name - 1].content, badge, "{path}");
            assert_eq!(line.spans[name - 1].style.fg, colour, "{path}");
            assert_eq!(line.spans[name - 2].content, aligned_branch(node), "{path}");
            assert_eq!(line.spans[0].content, " ", "{path}: the gutter keeps a blank");
            assert_eq!(name - 2, 1, "{path}: no tag field when no row has tags");
        }
    }

    /// The branch drawing of each nested row of [`aligned_forest`].
    fn aligned_branch(node: &TreeNode) -> &'static str {
        let rel = node.path.strip_prefix("/n/personal/proj").unwrap().to_str().unwrap();
        match rel {
            "ideas" => "├─▾ ",
            "ideas/deep" => "│ ├─▾ ",
            "ideas/deep/x.md" => "│ │ └─  ",
            "ideas/a.md" => "│ └─  ",
            "åäö" => "├─▾ ",
            "åäö/b.md" => "│ └─  ",
            "日本語" => "└─▾ ",
            "日本語/c.md" => "  └─  ",
            other => panic!("no row {other}"),
        }
    }

    #[test]
    fn a_nested_folder_shows_tee_or_corner_by_its_later_siblings_and_its_open_state() {
        let (sections, mut nodes) = aligned_forest();
        let branch = |nodes: &[TreeNode], path: &str| {
            let idx = row(nodes, path);
            let line = line_of(&sections, nodes, idx);
            line.spans[name_index(&line, &nodes[idx]) - 2].content.to_string()
        };
        assert_eq!(branch(&nodes, "/n/personal/proj/ideas"), "├─▾ ", "a later sibling follows");
        assert_eq!(branch(&nodes, "/n/personal/proj/日本語"), "└─▾ ", "the last child");
        let last = row(&nodes, "/n/personal/proj/日本語");
        nodes[last].expanded = false;
        assert_eq!(branch(&nodes, "/n/personal/proj/日本語"), "└─▸ ", "closed");
        let ideas = row(&nodes, "/n/personal/proj/ideas");
        nodes[ideas].expanded = false;
        assert_eq!(branch(&nodes, "/n/personal/proj/ideas"), "├─▸ ", "closed");
    }

    #[test]
    fn a_nested_file_at_depth_2_as_the_last_child_draws_a_bar_for_its_parent_and_a_corner() {
        let (sections, nodes) = aligned_forest();
        let idx = row(&nodes, "/n/personal/proj/ideas/a.md");
        assert_eq!(nodes[idx].depth, 2);
        let rest: String = render_row(line_of(&sections, &nodes, idx))[GUTTER_COL + 1..]
            .iter()
            .map(|c| c.0.as_str())
            .collect();
        assert_eq!(rest.trim_end(), format!("│ └─  {} a.md", Scope::Personal.icon()));
        let under_last = row(&nodes, "/n/personal/proj/日本語/c.md");
        let line = line_of(&sections, &nodes, under_last);
        assert_eq!(line.spans[name_index(&line, &nodes[under_last]) - 2].content, "  └─  ", "no bar under the last folder");
    }

    #[test]
    fn later_siblings_counts_only_the_visible_rows_under_the_same_parent() {
        let (_, nodes) = aligned_forest();
        let all = compute_visible(&nodes, "");
        let later = later_siblings(&nodes, &all);
        let at = |later: &[bool], path: &str| later[row(&nodes, path)];
        assert!(at(&later, "/n/personal/proj/ideas"));
        assert!(at(&later, "/n/personal/proj/ideas/deep"));
        assert!(!at(&later, "/n/personal/proj/ideas/a.md"));
        assert!(!at(&later, "/n/personal/proj/ideas/deep/x.md"));
        assert!(at(&later, "/n/personal/proj/åäö"));
        assert!(!at(&later, "/n/personal/proj/日本語"));
        assert!(!at(&later, "/n/personal/proj"), "the only section");

        // The filter keeps x.md and its ancestors: the siblings it hides no
        // longer count, so the path down to x.md is all corners and blanks.
        let filtered = compute_visible(&nodes, "x.md");
        assert_eq!(filtered.len(), 4);
        let later = later_siblings(&nodes, &filtered);
        assert!(!at(&later, "/n/personal/proj/ideas"));
        assert!(!at(&later, "/n/personal/proj/ideas/deep"));
        assert!(!at(&later, "/n/personal/proj/åäö"), "a hidden row gets none");
        let x = row(&nodes, "/n/personal/proj/ideas/deep/x.md");
        assert_eq!(branch_prefix(&nodes, x, &later), "    └─  ");
        assert_eq!(branch_prefix(&nodes, x, &later_siblings(&nodes, &all)), "│ │ └─  ");
    }

    #[test]
    fn every_directory_count_ends_at_the_text_width_at_every_depth() {
        let (sections, nodes) = aligned_forest();
        let dirs: Vec<usize> = (0..nodes.len()).filter(|&i| nodes[i].is_dir).collect();
        assert_eq!(dirs.iter().map(|&i| nodes[i].depth).max(), Some(2));
        for idx in dirs {
            let node = &nodes[idx];
            let line = line_of(&sections, &nodes, idx);
            let path = node.path.display();
            assert_eq!(line.width(), LIST_TEXT_WIDTH, "{path}: {:?}", span_texts(&line));
            let count = node.child_count.to_string();
            assert_eq!(line.spans.last().unwrap().content, count, "{path}");
            let cells = render_row(line);
            let last = LIST_TEXT_WIDTH - 1;
            assert_eq!(cells[last].0, count, "{path}");
            assert!(cells[last + 1..].iter().all(|c| c.0 == " "), "{path}");
        }
    }

    #[test]
    fn a_folder_name_with_multi_byte_or_wide_characters_still_aligns_its_count() {
        let (sections, nodes) = aligned_forest();
        for path in ["/n/personal/proj/åäö", "/n/personal/proj/日本語"] {
            let line = line_of(&sections, &nodes, row(&nodes, path));
            assert_eq!(line.width(), LIST_TEXT_WIDTH, "{path}: {:?}", span_texts(&line));
        }
    }

    /// A file's blank after the branch is as wide as a folder's expand
    /// mark, so siblings' badges share a column and a file's name starts
    /// two columns after its sibling folder's badge.
    #[test]
    fn a_file_name_starts_two_columns_after_its_sibling_folders_badge() {
        let (sections, nodes) = aligned_forest();
        let folder_idx = row(&nodes, "/n/personal/proj/ideas/deep");
        let file_idx = row(&nodes, "/n/personal/proj/ideas/a.md");
        let folder_line = line_of(&sections, &nodes, folder_idx);
        let file_line = line_of(&sections, &nodes, file_idx);
        let folder_badge = column_of(&folder_line, name_index(&folder_line, &nodes[folder_idx]) - 1);
        let file_name = column_of(&file_line, name_index(&file_line, &nodes[file_idx]));
        assert_eq!(file_name, folder_badge + 2);
    }

    /// A section row: the gutter, no tag field while no row has tags, the
    /// expand mark at column 1, then the scope icon, the label and the
    /// leader to the count at the text width; no scope word.
    #[test]
    fn a_section_row_keeps_its_spans() {
        let (sections, nodes) = aligned_forest();
        let line = line_of(&sections, &nodes, 0);
        let dots = |n: usize| format!(" {} ", "·".repeat(n));
        let base = [" ", "▼ ", "\u{f007} ", "/n/personal/proj"];
        let mut expected: Vec<String> = base.iter().map(|s| s.to_string()).collect();
        expected.push(dots(32 + 2 + 7 + 9));
        expected.push("4".to_string());
        assert_eq!(span_texts(&line), expected);
        assert_eq!(column_of(&line, 1), 1, "the expand mark starts at column 1");
    }

    #[test]
    fn a_section_count_ends_at_the_last_text_column_of_the_padded_list_pane() {
        use ratatui::buffer::Buffer;
        use ratatui::widgets::BorderType;
        let (sections, nodes) = aligned_forest();
        let pane = Rect::new(0, 0, 80, 3);
        let line = line_at_width(&sections, &nodes, 0, list_text_width(pane.width));
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .padding(Padding::new(1, 1, 0, 0));
        let mut buf = Buffer::empty(pane);
        let list = List::new(vec![ListItem::new(line)]).highlight_style(theme::selected_row());
        let mut state = ListState::default();
        state.select(Some(0));
        StatefulWidget::render(list, block.inner(pane), &mut buf, &mut state);
        assert_eq!(buf[(77, 1)].symbol(), "4", "the count is not clipped");
        assert_eq!(buf[(76, 1)].symbol(), " ");
        assert_eq!(buf[(2, 1)].symbol(), " ", "the gutter right after the padding");
        assert_eq!(buf[(3, 1)].symbol(), "▼", "the expand mark right after the gutter");
    }

    #[test]
    fn the_selected_row_is_shown_by_its_style_across_the_whole_line_without_a_symbol() {
        let (sections, nodes) = aligned_forest();
        let plain = span_texts(&line_of(&sections, &nodes, 0)).concat();
        let cells = render_row_styled(line_of(&sections, &nodes, 0));
        let drawn: String = cells.iter().map(|c| c.0.as_str()).collect();
        assert!(drawn.starts_with(&plain), "no symbol shifts the row: {drawn:?}");
        for (x, (_, style)) in cells.iter().enumerate() {
            assert_eq!(style.bg, theme::selected_row().bg, "column {x}");
            assert!(style.add_modifier.contains(Modifier::BOLD), "column {x}");
        }
    }

    // --- Compact tag field ---

    /// [`aligned_forest`] with three tags on `ideas/a.md` and one on
    /// `åäö/b.md`; the other rows have none.
    fn tagged_forest() -> (Vec<SectionSpec>, Vec<TreeNode>) {
        let (sections, mut nodes) = aligned_forest();
        let a = row(&nodes, "/n/personal/proj/ideas/a.md");
        nodes[a].flags = FLAG_IMPORTANT | FLAG_LONGTERM | FLAG_BLOCKED;
        let b = row(&nodes, "/n/personal/proj/åäö/b.md");
        nodes[b].flags = FLAG_IDEA;
        (sections, nodes)
    }

    /// The tag field of `path`'s row: the spans between the gutter and the
    /// branch drawing, text and colour.
    fn field_of(sections: &[SectionSpec], nodes: &[TreeNode], path: &str) -> Vec<(String, Option<Color>)> {
        let idx = row(nodes, path);
        let line = line_of(sections, nodes, idx);
        let branch = line.spans.iter().position(|s| s.style.fg == Some(theme::SURFACE)).expect("branch");
        line.spans[1..branch].iter().map(|s| (s.content.to_string(), s.style.fg)).collect()
    }

    #[test]
    fn the_tag_field_is_as_wide_as_the_most_tags_on_a_visible_row() {
        let (sections, mut nodes) = tagged_forest();
        let dot = |i: usize| ("●".to_string(), Some(theme::FLAG_COLORS[i]));
        let blank = |n: usize| (" ".repeat(n), None);
        assert_eq!(
            field_of(&sections, &nodes, "/n/personal/proj/ideas/a.md"),
            vec![dot(0), dot(2), dot(4), blank(1)],
            "three tags fill the field"
        );
        assert_eq!(field_of(&sections, &nodes, "/n/personal/proj/åäö/b.md"), vec![dot(3), blank(3)], "one tag");
        assert_eq!(field_of(&sections, &nodes, "/n/personal/proj/日本語/c.md"), vec![blank(4)], "no tag");
        assert_eq!(field_of(&sections, &nodes, "/n/personal/proj"), vec![blank(4)], "the section row");
        for idx in (0..nodes.len()).filter(|&i| nodes[i].is_dir) {
            let line = line_of(&sections, &nodes, idx);
            assert_eq!(line.width(), LIST_TEXT_WIDTH, "{:?}", span_texts(&line));
        }

        // Folding `ideas` hides the three-tag row: the field shrinks to one.
        let ideas = row(&nodes, "/n/personal/proj/ideas");
        nodes[ideas].expanded = false;
        assert_eq!(field_of(&sections, &nodes, "/n/personal/proj/åäö/b.md"), vec![dot(3), blank(1)]);
        assert_eq!(field_of(&sections, &nodes, "/n/personal/proj/日本語/c.md"), vec![blank(2)]);

        // With no tag on any visible row there is no field at all.
        let accented = row(&nodes, "/n/personal/proj/åäö");
        nodes[accented].expanded = false;
        assert_eq!(field_of(&sections, &nodes, "/n/personal/proj/日本語/c.md"), vec![]);
        for idx in (0..nodes.len()).filter(|&i| nodes[i].is_dir && compute_visible(&nodes, "").contains(&i)) {
            let line = line_of(&sections, &nodes, idx);
            assert_eq!(line.width(), LIST_TEXT_WIDTH, "{:?}", span_texts(&line));
        }
    }

    #[test]
    fn a_tagless_forest_draws_no_tag_placeholders_and_the_section_mark_follows_the_gutter() {
        let (sections, nodes) = aligned_forest();
        assert!(nodes.iter().all(|n| n.flags == 0));
        for idx in 0..nodes.len() {
            let line = line_of(&sections, &nodes, idx);
            assert_eq!(line.spans[0].content, " ", "the gutter");
            // Below depth 1 a blank ancestor level may lead the tree drawing.
            if nodes[idx].depth <= 1 {
                let first = &line.spans[1];
                assert!(!first.content.starts_with(' '), "nothing between gutter and tree: {:?}", span_texts(&line));
            }
            let before_name: String =
                line.spans[..name_index(&line, &nodes[idx])].iter().map(|s| s.content.as_ref()).collect();
            assert!(!before_name.contains('·') && !before_name.contains('●'), "{before_name:?}");
        }
        assert_eq!(line_of(&sections, &nodes, 0).spans[1].content, "▼ ");
    }

    #[test]
    fn a_click_on_the_nth_dot_toggles_the_rows_nth_set_tag() {
        let (sections, nodes) = tagged_forest();
        let a = row(&nodes, "/n/personal/proj/ideas/a.md");
        let cells = render_row(line_of(&sections, &nodes, a));
        let flags = nodes[a].flags;
        for (col, tag) in [(1u16, 0usize), (2, 2), (3, 4)] {
            assert_eq!(cells[usize::from(col)].0, "●", "column {col}");
            assert_eq!(mouse_x_to_row_tag(col, 0, flags), Some(tag), "column {col}");
            assert_eq!(cells[usize::from(col)].1, Some(theme::FLAG_COLORS[tag]), "column {col}");
        }
        assert_eq!(mouse_x_to_row_tag(0, 0, flags), None, "the gutter");
        assert_eq!(mouse_x_to_row_tag(4, 0, flags), None, "past the dots");

        let b = row(&nodes, "/n/personal/proj/åäö/b.md");
        let flags = nodes[b].flags;
        assert_eq!(mouse_x_to_row_tag(1, 0, flags), Some(3));
        for col in [0u16, 2, 3, 4, 5] {
            assert_eq!(mouse_x_to_row_tag(col, 0, flags), None, "column {col}: the blank field");
        }
        // Offset by the list's left edge, and a toggle clears the clicked tag.
        let tag = mouse_x_to_row_tag(10 + 2, 10, nodes[a].flags).unwrap();
        assert_eq!(nodes[a].flags ^ FLAG_DEFS[tag].bit, FLAG_IMPORTANT | FLAG_BLOCKED);
        assert_eq!(mouse_x_to_row_tag(5, 0, 0), None, "a row without tags has no dot");
    }

    #[test]
    fn a_marked_row_with_tags_keeps_the_mark_in_the_gutter_before_its_dots() {
        let (sections, nodes) = tagged_forest();
        let a = row(&nodes, "/n/personal/proj/ideas/a.md");
        let marked = render_row(mark_row(line_of(&sections, &nodes, a)));
        let texts: Vec<&str> = marked[..6].iter().map(|c| c.0.as_str()).collect();
        assert_eq!(texts, [MARK_GLYPH, "●", "●", "●", " ", "│"]);
    }

    // --- Delete ---

    const SCOPES: [(Scope, &str); 4] = [
        (Scope::Personal, "personal"),
        (Scope::Public, "public"),
        (Scope::Local, "local"),
        (Scope::Global, "global"),
    ];

    /// Every `.md` file under `dir`, skipping dot entries like `.tags`: what
    /// the aggregator lists for a store.
    fn md_files(dir: &Path) -> Vec<PathBuf> {
        let mut out = Vec::new();
        let Ok(entries) = std::fs::read_dir(dir) else {
            return out;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if entry.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            if path.is_dir() {
                out.extend(md_files(&path));
            } else if path.extension().is_some_and(|e| e == "md") {
                out.push(path);
            }
        }
        out.sort();
        out
    }

    /// A temp tree with one store per scope, each holding `ideas/a.md`,
    /// `ideas/b.md` and `c.md`.
    fn temp_tree() -> (tempfile::TempDir, Vec<(Scope, PathBuf)>) {
        let dir = tempfile::tempdir().unwrap();
        let mut roots = Vec::new();
        for (scope, name) in SCOPES {
            let root = dir.path().join(name);
            std::fs::create_dir_all(root.join("ideas")).unwrap();
            for file in ["ideas/a.md", "ideas/b.md", "c.md"] {
                std::fs::write(root.join(file), "# note\n").unwrap();
            }
            roots.push((scope, root));
        }
        (dir, roots)
    }

    /// The rebuild closure's work: list the stores again from disk.
    fn list_sections(roots: &[(Scope, PathBuf)]) -> Vec<SectionSpec> {
        roots
            .iter()
            .map(|(scope, root)| SectionSpec {
                root: root.clone(),
                tag_root: root.clone(),
                label: format!("{scope:?}"),
                icon: "",
                is_doc: false,
                files: md_files(root),
                dirs: Vec::new(),
                scope: *scope,
                project: (*scope != Scope::Global).then(|| "proj".to_string()),
                new_note_root: root.clone(),
                is_current: false,
            })
            .collect()
    }

    fn all_files(roots: &[(Scope, PathBuf)]) -> Vec<PathBuf> {
        roots.iter().flat_map(|(_, root)| md_files(root)).collect()
    }

    fn forest_of(sections: Vec<SectionSpec>) -> Forest {
        let (mut nodes, tag_roots) = build_forest(&sections);
        let initial: Vec<HashMap<String, u8>> =
            tag_roots.iter().map(|r| note_tags::load_tags(r)).collect();
        apply_tags(&mut nodes, &tag_roots, &initial);
        Forest { sections, nodes, tag_roots, initial }
    }

    fn path_str(path: &Path) -> &str {
        path.to_str().unwrap()
    }

    /// The prompt `d` opens with the cursor on `path`, whose folders are
    /// expanded as they are when the cursor can reach it.
    fn prompt_at(forest: &mut Forest, path: &Path) -> DeletePrompt {
        let i = row(&forest.nodes, path_str(path));
        let mut up = forest.nodes[i].parent_idx;
        while let Some(p) = up {
            forest.nodes[p].expanded = true;
            up = forest.nodes[p].parent_idx;
        }
        delete_request(&forest.nodes, &forest.sections, Some(i), Some("proj"))
            .expect("a note row")
            .expect("a row under the cursor")
    }

    #[test]
    fn d_then_y_deletes_exactly_that_note_in_every_scope() {
        for (scope, name) in SCOPES {
            let (_dir, roots) = temp_tree();
            let rebuild = || Ok(list_sections(&roots));
            let mut forest = forest_of(list_sections(&roots));
            let mut retired = Vec::new();
            let target = roots.iter().find(|(s, _)| *s == scope).unwrap().1.join("ideas/a.md");
            let before = all_files(&roots);

            let prompt = prompt_at(&mut forest, &target);
            let outcome = answer_delete(KeyCode::Char('y'), &mut forest, &mut retired, &prompt, "", &rebuild)
                .expect("y confirms");

            let expected: Vec<PathBuf> = before.into_iter().filter(|p| *p != target).collect();
            assert_eq!(all_files(&roots), expected, "{name}");
            assert!(!forest.nodes.iter().any(|n| n.path == target), "{name}: row gone");
            assert_eq!(outcome.message, "deleted ideas/a.md", "{name}");
        }
    }

    #[test]
    fn any_answer_but_y_deletes_nothing() {
        let (_dir, roots) = temp_tree();
        let rebuild = || Ok(list_sections(&roots));
        let mut forest = forest_of(list_sections(&roots));
        let mut retired = Vec::new();
        let target = roots[0].1.join("ideas/a.md");
        let prompt = prompt_at(&mut forest, &target);
        let before = all_files(&roots);
        let rows = forest.nodes.len();
        for code in [KeyCode::Char('n'), KeyCode::Esc, KeyCode::Char('x'), KeyCode::Enter, KeyCode::Char('Y'), KeyCode::Char('d')] {
            assert!(
                answer_delete(code, &mut forest, &mut retired, &prompt, "", &rebuild).is_none(),
                "{code:?}"
            );
            assert_eq!(all_files(&roots), before, "{code:?}");
            assert_eq!(forest.nodes.len(), rows, "{code:?}");
        }
        assert!(retired.is_empty());
    }

    #[test]
    fn delete_prompt_names_the_file_and_the_scope() {
        let sections = project_sections();
        let (nodes, _) = build_forest(&sections);
        let question = |path: &str, current: Option<&str>| {
            let p = delete_request(&nodes, &sections, Some(row(&nodes, path)), current)
                .unwrap()
                .unwrap();
            delete_question(&p)
        };
        assert_eq!(
            question("/n/personal/proj/ideas/a.md", Some("proj")),
            "delete ideas/a.md from personal? y/n"
        );
        let public = question("/p/notez/plans/b.md", Some("proj"));
        assert_eq!(public, "delete plans/b.md from public (committed with the project)? y/n");
        assert!(question("/p/notez/plans/b.md", None).contains("public"));
        assert!(question("/p/docs/design/c.md", Some("proj")).contains("public"));
        let local = question("/p/.notez/d.md", Some("proj"));
        assert!(local.starts_with("delete d.md from local scratch?"), "{local}");
        assert!(local.contains("not recoverable"), "{local}");
        assert_eq!(question("/n/e.md", Some("proj")), "delete e.md from global? y/n");
        assert_eq!(
            question("/n/personal/proj/top.md", None),
            "delete top.md from personal (proj)? y/n"
        );
        for path in ["/n/personal/proj/top.md", "/p/notez/plans/b.md", "/n/e.md"] {
            assert!(!question(path, Some("proj")).contains("not recoverable"), "{path}");
        }
    }

    #[test]
    fn d_on_a_section_a_docs_folder_or_an_empty_tree_changes_nothing() {
        let sections = project_sections();
        let (nodes, _) = build_forest(&sections);
        for path in ["/n/personal/proj", "/p/docs", "/p/.notez", "/n"] {
            let request = delete_request(&nodes, &sections, Some(row(&nodes, path)), None);
            assert_eq!(request.err(), Some(SECTION_DELETE), "{path}");
        }
        let docs = delete_request(&nodes, &sections, Some(row(&nodes, "/p/docs/design")), None);
        assert_eq!(docs.err(), Some(DOCS_FOLDER_DELETE));
        let folder = delete_request(&nodes, &sections, Some(row(&nodes, "/n/personal/proj/ideas")), None);
        assert!(folder.unwrap().unwrap().folder.is_some(), "a note folder opens the folder prompt");
        assert!(matches!(delete_request(&nodes, &sections, None, None), Ok(None)));
        assert!(matches!(delete_request(&nodes, &sections, Some(nodes.len()), None), Ok(None)));
        assert!(matches!(delete_request(&[], &[], Some(0), None), Ok(None)));
        assert!(matches!(delete_request(&[], &[], None, None), Ok(None)));
    }

    fn delete_and_select(
        before: &[&str],
        after: &[&str],
        deleted: &str,
        expand: &[&str],
        search: &str,
    ) -> (Forest, Option<usize>) {
        let (mut nodes, roots) = build_forest(&[spec("/r", "S", before)]);
        for path in expand {
            let i = row(&nodes, path);
            nodes[i].expanded = true;
        }
        let initial = vec![HashMap::new(); roots.len()];
        let mut forest = Forest { sections: vec![spec("/r", "S", before)], nodes, tag_roots: roots, initial };
        let selected = forest.rebuild_after_delete(vec![spec("/r", "S", after)], Path::new(deleted), search);
        (forest, selected)
    }

    #[test]
    fn delete_selects_the_next_note_then_the_previous_then_the_folder() {
        let open = ["/r", "/r/ideas"];
        let files = ["ideas/a.md", "ideas/b.md", "ideas/c.md", "z.md"];

        let (f, sel) = delete_and_select(&files, &["ideas/a.md", "ideas/c.md", "z.md"], "/r/ideas/b.md", &open, "");
        assert_eq!(sel, Some(row(&f.nodes, "/r/ideas/c.md")), "the row that followed");

        let (f, sel) = delete_and_select(&files, &["ideas/a.md", "ideas/b.md", "z.md"], "/r/ideas/c.md", &open, "");
        assert_eq!(sel, Some(row(&f.nodes, "/r/ideas/b.md")), "last in its folder: the one before");

        let (f, sel) = delete_and_select(
            &["ideas/sub/x.md", "ideas/only.md"],
            &["ideas/sub/x.md"],
            "/r/ideas/only.md",
            &open,
            "",
        );
        assert_eq!(sel, Some(row(&f.nodes, "/r/ideas/sub")), "the one before can be a folder");

        let (f, sel) = delete_and_select(&["ideas/a.md", "z.md"], &["z.md"], "/r/ideas/a.md", &open, "");
        assert_eq!(
            sel,
            Some(row(&f.nodes, "/r")),
            "an emptied folder leaves the tree, so its nearest listed ancestor is selected"
        );

        let (f, sel) = delete_and_select(&["ideas/a.md"], &[], "/r/ideas/a.md", &open, "");
        assert!(f.nodes.is_empty());
        assert_eq!(sel, None, "nothing left to select");
    }

    #[test]
    fn delete_keeps_expansion_unsaved_tags_and_the_filter() {
        let before = ["ideas/a.md", "ideas/b.md", "ideas/c-a.md", "plans/p.md", "shut/s.md", "z.md"];
        let (mut nodes, roots) = build_forest(&[spec("/r", "S", &before)]);
        for path in ["/r", "/r/ideas", "/r/plans"] {
            let i = row(&nodes, path);
            nodes[i].expanded = true;
        }
        let p = row(&nodes, "/r/plans/p.md");
        nodes[p].flags = FLAG_PRIO;
        let z = row(&nodes, "/r/z.md");
        nodes[z].origin = PathBuf::from("/r/old-z.md");
        let initial = vec![HashMap::new()];
        let mut forest = Forest { sections: vec![spec("/r", "S", &before)], nodes, tag_roots: roots, initial };

        let after = ["ideas/b.md", "ideas/c-a.md", "plans/p.md", "shut/s.md", "z.md"];
        // The filter "a" hides b.md, the row that followed a.md, so the
        // cursor goes on to the next row it shows.
        let search = "a";
        let sel = forest.rebuild_after_delete(vec![spec("/r", "S", &after)], Path::new("/r/ideas/a.md"), search);

        let n = &forest.nodes;
        assert!(!compute_visible(n, search).contains(&row(n, "/r/ideas/b.md")));
        assert_eq!(sel, Some(row(n, "/r/ideas/c-a.md")));
        assert!(compute_visible(n, search).contains(&sel.unwrap()));
        assert!(n[row(n, "/r")].expanded && n[row(n, "/r/ideas")].expanded && n[row(n, "/r/plans")].expanded);
        assert!(!n[row(n, "/r/shut")].expanded, "a closed folder stays closed");
        assert_eq!(n[row(n, "/r/plans/p.md")].flags, FLAG_PRIO, "unsaved tag edits survive");
        assert_eq!(n[row(n, "/r/z.md")].origin, PathBuf::from("/r/old-z.md"));
    }

    #[test]
    fn retired_keys_leave_the_tags_map_and_nothing_else_changes() {
        let s = spec("/r", "S", &["b.md"]);
        let (mut nodes, roots) = build_forest(&[s]);
        let initial = vec![HashMap::from([
            ("a.md".to_string(), FLAG_PRIO),
            ("b.md".to_string(), FLAG_IMPORTANT),
            ("elsewhere/hidden.md".to_string(), FLAG_IMPORTANT),
        ])];
        apply_tags(&mut nodes, &roots, &initial);
        let retired = vec![(PathBuf::from("/r"), "a.md".to_string())];

        let changed = changed_tag_maps_retiring(&nodes, &roots, &initial, &retired);
        assert_eq!(
            changed,
            vec![(
                PathBuf::from("/r"),
                HashMap::from([
                    ("b.md".to_string(), FLAG_IMPORTANT),
                    ("elsewhere/hidden.md".to_string(), FLAG_IMPORTANT),
                ])
            )]
        );

        let untagged = vec![(PathBuf::from("/r"), "never-tagged.md".to_string())];
        assert!(
            changed_tag_maps_retiring(&nodes, &roots, &initial, &untagged).is_empty(),
            "an untagged note's delete leaves .tags alone"
        );
        let unknown_root = vec![(PathBuf::from("/elsewhere"), "a.md".to_string())];
        assert!(changed_tag_maps_retiring(&nodes, &roots, &initial, &unknown_root).is_empty());
    }

    #[test]
    fn deleting_a_renamed_note_retires_its_original_key() {
        let (_dir, roots) = temp_tree();
        let root = roots[0].1.clone();
        std::fs::write(root.join(".tags"), "ideas/a.md:1\n").unwrap();
        let rebuild = || Ok(list_sections(&roots));
        let mut forest = forest_of(list_sections(&roots));
        let a = row(&forest.nodes, path_str(&root.join("ideas/a.md")));
        assert_ne!(forest.nodes[a].flags, 0, "the fixture's .tags loaded");
        // As a rename in this session leaves it: new path, old origin.
        std::fs::rename(root.join("ideas/a.md"), root.join("ideas/renamed.md")).unwrap();
        forest.nodes[a].path = root.join("ideas/renamed.md");
        let mut retired = Vec::new();

        let prompt = prompt_at(&mut forest, &root.join("ideas/renamed.md"));
        answer_delete(KeyCode::Char('y'), &mut forest, &mut retired, &prompt, "", &rebuild).unwrap();

        let changed = changed_tag_maps_retiring(&forest.nodes, &forest.tag_roots, &forest.initial, &retired);
        assert_eq!(changed, vec![(root.clone(), HashMap::new())]);
    }

    #[test]
    fn deleting_the_last_note_of_a_tag_root_still_retires_its_key() {
        let (_dir, roots) = temp_tree();
        let local = roots.iter().find(|(s, _)| *s == Scope::Local).unwrap().1.clone();
        std::fs::remove_file(local.join("ideas/b.md")).unwrap();
        std::fs::remove_file(local.join("c.md")).unwrap();
        std::fs::write(local.join(".tags"), "ideas/a.md:1\nkept.md:2\n").unwrap();
        let rebuild = || Ok(list_sections(&roots));
        let mut forest = forest_of(list_sections(&roots));
        let mut retired = Vec::new();

        let prompt = prompt_at(&mut forest, &local.join("ideas/a.md"));
        answer_delete(KeyCode::Char('y'), &mut forest, &mut retired, &prompt, "", &rebuild).unwrap();

        assert!(!forest.nodes.iter().any(|n| n.path.starts_with(&local)), "the section is gone");
        let changed = changed_tag_maps_retiring(&forest.nodes, &forest.tag_roots, &forest.initial, &retired);
        let initial_kept = note_tags::load_tags(&local).get("kept.md").copied();
        assert_eq!(changed, vec![(local.clone(), HashMap::from([("kept.md".to_string(), initial_kept.unwrap())]))]);
    }

    #[test]
    fn delete_of_a_note_already_gone_reports_and_rebuilds() {
        let (_dir, roots) = temp_tree();
        let rebuild = || Ok(list_sections(&roots));
        let mut forest = forest_of(list_sections(&roots));
        let mut retired = Vec::new();
        let target = roots[1].1.join("ideas/a.md");
        let prompt = prompt_at(&mut forest, &target);
        std::fs::remove_file(&target).unwrap();
        let before = all_files(&roots);

        let outcome = answer_delete(KeyCode::Char('y'), &mut forest, &mut retired, &prompt, "", &rebuild).unwrap();

        assert!(outcome.message.starts_with("delete failed: "), "{}", outcome.message);
        assert_eq!(all_files(&roots), before, "nothing else removed");
        assert!(!forest.nodes.iter().any(|n| n.path == target), "the display matches the disk");
        assert!(retired.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn delete_without_permission_reports_and_keeps_the_note_selected() {
        use std::os::unix::fs::PermissionsExt;
        let (_dir, roots) = temp_tree();
        let ideas = roots[0].1.join("ideas");
        let rebuild = || Ok(list_sections(&roots));
        let mut forest = forest_of(list_sections(&roots));
        let mut retired = Vec::new();
        let target = ideas.join("a.md");
        let prompt = prompt_at(&mut forest, &target);
        let before = all_files(&roots);

        std::fs::set_permissions(&ideas, std::fs::Permissions::from_mode(0o555)).unwrap();
        let outcome = answer_delete(KeyCode::Char('y'), &mut forest, &mut retired, &prompt, "", &rebuild);
        std::fs::set_permissions(&ideas, std::fs::Permissions::from_mode(0o755)).unwrap();
        let outcome = outcome.unwrap();

        assert!(outcome.message.starts_with("delete failed: "), "{}", outcome.message);
        assert_eq!(all_files(&roots), before);
        assert!(retired.is_empty());
        assert_eq!(outcome.row, Some(row(&forest.nodes, path_str(&target))));
    }

    #[test]
    fn a_failed_rebuild_still_drops_the_deleted_row() {
        let (_dir, roots) = temp_tree();
        let rebuild = || -> Result<Vec<SectionSpec>> { anyhow::bail!("listing broke") };
        let mut forest = forest_of(list_sections(&roots));
        let mut retired = Vec::new();
        let target = roots[0].1.join("ideas/a.md");
        let prompt = prompt_at(&mut forest, &target);

        let outcome = answer_delete(KeyCode::Char('y'), &mut forest, &mut retired, &prompt, "", &rebuild).unwrap();

        assert!(!target.exists());
        assert!(outcome.message.contains("listing broke"), "{}", outcome.message);
        assert!(!forest.nodes.iter().any(|n| n.path == target));
        assert_eq!(outcome.row, Some(row(&forest.nodes, path_str(&roots[0].1.join("ideas/b.md")))));
    }

    #[test]
    fn delete_keys_are_in_the_table_with_their_modes() {
        let d: Vec<&KeyHint> = TREE_KEYS.iter().filter(|k| k.key == "d").collect();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].modes, BROWSE);
        assert_eq!(d[0].group, Group::Edit);
        assert_eq!(shown_keys(Mode::ConfirmDelete, &[], 200), vec!["y", "n/esc"]);
        let confirm: Vec<&str> = TREE_KEYS
            .iter()
            .filter(|k| k.modes.contains(&Mode::ConfirmDelete))
            .map(|k| k.key)
            .collect();
        assert_eq!(confirm, vec!["y", "n/esc"]);
        let sections = project_sections();
        let (nodes, _) = build_forest(&sections);
        let prompt = delete_request(&nodes, &sections, Some(row(&nodes, "/n/e.md")), None).unwrap().unwrap();
        let rendered = text_of(&lead_with_hints(delete_lead(&prompt), Mode::ConfirmDelete, &[], 120));
        assert!(rendered.starts_with(" delete e.md from global? y/n"), "{rendered}");
        assert!(rendered.contains("y confirm") && rendered.contains("n/esc cancel"), "{rendered}");
        assert!(!rendered.contains("quit"));
    }

    // --- Folders ---

    /// [`spec`] with `dirs` (relative to `root`) listed as its directories.
    fn spec_with_dirs(root: &str, files: &[&str], dirs: &[&str]) -> SectionSpec {
        SectionSpec {
            dirs: dirs.iter().map(|d| PathBuf::from(root).join(d)).collect(),
            ..spec(root, "S", files)
        }
    }

    #[test]
    fn a_listed_directory_is_a_folder_row_with_or_without_files() {
        let s = spec_with_dirs("/r", &["full/a.md"], &["full", "empty", "outer/inner"]);
        let (nodes, _) = build_forest(&[s]);
        for (path, depth, count) in
            [("/r/full", 1, 1), ("/r/empty", 1, 0), ("/r/outer", 1, 0), ("/r/outer/inner", 2, 0)]
        {
            let node = &nodes[row(&nodes, path)];
            assert!(node.is_dir, "{path}");
            assert_eq!((node.depth, node.child_count), (depth, count), "{path}");
        }
        let inner = row(&nodes, "/r/outer/inner");
        assert_eq!(nodes[inner].parent_idx, Some(row(&nodes, "/r/outer")));
        assert_eq!(nodes.iter().filter(|n| n.path == Path::new("/r/full")).count(), 1);
    }

    #[test]
    fn a_section_with_only_empty_folders_still_has_a_row() {
        let (nodes, roots) = build_forest(&[spec_with_dirs("/r", &[], &["empty"])]);
        assert_eq!(nodes.len(), 2);
        assert_eq!(roots, vec![PathBuf::from("/r")]);
        assert_eq!(nodes[1].path, PathBuf::from("/r/empty"));
    }

    #[test]
    fn a_directory_outside_the_section_root_is_ignored() {
        let mut s = spec("/r", "S", &["a.md"]);
        s.dirs = vec![PathBuf::from("/elsewhere/x")];
        let (nodes, _) = build_forest(&[s]);
        assert!(!nodes.iter().any(|n| n.path.starts_with("/elsewhere")));
    }

    /// The prompt `N` opens at `path` in [`project_sections`].
    fn folder_prompt_at(path: &str, current: Option<&str>) -> Result<NewNotePrompt, &'static str> {
        let sections = project_sections();
        let (nodes, _) = build_forest(&sections);
        open_new_folder_prompt(&nodes, &sections, Some(row(&nodes, path)), &ctx_with(current, roots()))
    }

    #[test]
    fn new_folder_targets_the_folder_under_the_cursor_like_n() {
        let here = Some("proj");
        let cases = [
            ("/n/personal/proj/ideas", target("/n/personal/proj/ideas", Scope::Personal, "personal/ideas")),
            ("/n/personal/proj/ideas/a.md", target("/n/personal/proj/ideas", Scope::Personal, "personal/ideas")),
            ("/n/personal/proj", target("/n/personal/proj", Scope::Personal, "personal")),
            ("/p/notez", target("/p/notez", Scope::Public, "public (committed with the project)")),
            ("/p/notez/plans/b.md", target("/p/notez/plans", Scope::Public, "public (committed with the project)/plans")),
            ("/p/.notez", target("/p/.notez", Scope::Local, "local scratch")),
            ("/p/.notez/d.md", target("/p/.notez", Scope::Local, "local scratch")),
            ("/n", target("/n", Scope::Global, "global")),
            ("/n/e.md", target("/n", Scope::Global, "global")),
        ];
        for (path, expected) in cases {
            let prompt = folder_prompt_at(path, here).expect(path);
            assert!(prompt.is_folder, "{path}");
            assert_eq!(prompt.target, expected, "{path}");
            assert_eq!(prompt.origin, expected, "{path}");
        }
    }

    #[test]
    fn new_folder_tab_cycles_the_scopes_as_n_does() {
        let sections = project_sections();
        let (nodes, _) = build_forest(&sections);
        let ctx = ctx_with(Some("proj"), roots());
        let at = Some(row(&nodes, "/n/personal/proj/ideas/a.md"));
        let note = open_new_note_prompt(&nodes, &sections, at, &ctx);
        let folder = open_new_folder_prompt(&nodes, &sections, at, &ctx).unwrap();
        assert_eq!(folder.project, note.project);
        let (mut a, mut b) = (folder.target.clone(), note.target.clone());
        for _ in 0..4 {
            a = next_scope_target(&a, &folder.origin, folder.project.as_deref(), &ctx.new_note_roots, Some("proj"));
            b = next_scope_target(&b, &note.origin, note.project.as_deref(), &ctx.new_note_roots, Some("proj"));
            assert_eq!(a, b);
        }
    }

    #[test]
    fn new_folder_in_a_docs_section_is_refused() {
        for path in ["/p/docs", "/p/docs/design", "/p/docs/design/c.md"] {
            assert_eq!(folder_prompt_at(path, Some("proj")).err(), Some(FOLDER_IN_DOCS), "{path}");
        }
    }

    #[test]
    fn new_folder_on_an_empty_tree_targets_what_n_targets() {
        let ctx = ctx_with(Some("proj"), roots());
        let folder = open_new_folder_prompt(&[], &[], None, &ctx).unwrap();
        let note = open_new_note_prompt(&[], &[], None, &ctx);
        assert_eq!(folder.target, note.target);
        let past_end = open_new_folder_prompt(&[], &[], Some(0), &ctx).unwrap();
        assert_eq!(past_end.target, note.target);
    }

    #[test]
    fn new_folder_lead_names_the_scope_and_folder() {
        let lead = new_folder_lead("personal/ideas", "drafts");
        let rendered = text_of(&lead_with_hints(lead, Mode::NewItem, &[], 120));
        assert!(rendered.starts_with(" new folder in personal/ideas: drafts_"), "{rendered}");
        assert!(rendered.contains("create") && rendered.contains("cancel"), "{rendered}");
    }

    /// A temp tree ([`temp_tree`]) listed with its directories, as the real
    /// listing does.
    fn list_sections_with_dirs(roots: &[(Scope, PathBuf)]) -> Vec<SectionSpec> {
        let mut sections = list_sections(roots);
        for spec in &mut sections {
            spec.dirs = all_dirs(&spec.root);
        }
        sections
    }

    fn all_dirs(dir: &Path) -> Vec<PathBuf> {
        let mut out = Vec::new();
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            if entry.file_type().unwrap().is_dir() && !entry.file_name().to_string_lossy().starts_with('.') {
                out.push(entry.path());
                out.extend(all_dirs(&entry.path()));
            }
        }
        out
    }

    fn folder_target(roots: &[(Scope, PathBuf)], scope: Scope, sub: &str) -> NewNoteTarget {
        let root = &roots.iter().find(|(s, _)| *s == scope).unwrap().1;
        NewNoteTarget { dir: root.join(sub), scope, label: format!("{scope:?}") }
    }

    #[test]
    fn enter_creates_exactly_that_folder_selected_and_expanded_in_every_scope() {
        for (scope, name) in SCOPES {
            for sub in ["", "ideas"] {
                let (_dir, roots) = temp_tree();
                let rebuild = || Ok(list_sections_with_dirs(&roots));
                let mut forest = forest_of(list_sections_with_dirs(&roots));
                let target = folder_target(&roots, scope, sub);
                let before = all_files(&roots);

                let outcome = create_folder(&mut forest, &target, "New Stuff", &rebuild);

                let made = target.dir.join("new-stuff");
                assert!(made.is_dir(), "{name}/{sub}");
                assert_eq!(std::fs::read_dir(&made).unwrap().count(), 0, "{name}/{sub}");
                assert_eq!(all_files(&roots), before, "{name}/{sub}");
                assert_eq!(outcome.message, None, "{name}/{sub}");
                let row_idx = outcome.row.expect("the new folder is listed");
                let node = &forest.nodes[row_idx];
                assert_eq!(node.path, made, "{name}/{sub}");
                assert!(node.is_dir && node.expanded, "{name}/{sub}");
                assert!(get_visible_nodes(&forest.nodes).contains(&row_idx), "{name}/{sub}");
            }
        }
    }

    #[test]
    fn a_new_local_folder_gets_the_scratch_gitignore_step() {
        let repo = tempfile::tempdir().unwrap();
        std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(repo.path())
            .status()
            .unwrap();
        let store = repo.path().join(".notez");
        std::fs::create_dir_all(&store).unwrap();
        std::fs::write(store.join("s.md"), "# s\n").unwrap();
        let roots = vec![(Scope::Local, store.clone())];
        let rebuild = || Ok(list_sections_with_dirs(&roots));
        let mut forest = forest_of(list_sections_with_dirs(&roots));
        let target = folder_target(&roots, Scope::Local, "");

        create_folder(&mut forest, &target, "scratchpad", &rebuild);

        assert!(store.join("scratchpad").is_dir());
        let ignore = std::fs::read_to_string(repo.path().join(".gitignore")).unwrap();
        assert!(ignore.lines().any(|l| l == ".notez"), "{ignore:?}");
    }

    #[test]
    fn an_empty_name_creates_nothing_and_says_so() {
        let (_dir, roots) = temp_tree();
        let rebuild = || -> Result<Vec<SectionSpec>> { panic!("nothing to rebuild") };
        let mut forest = forest_of(list_sections_with_dirs(&roots));
        let rows_before = forest.nodes.len();
        let target = folder_target(&roots, Scope::Personal, "");
        let dirs_before = all_dirs(&target.dir);

        for name in ["", "   ", "?!"] {
            let outcome = create_folder(&mut forest, &target, name, &rebuild);
            assert_eq!(outcome.message.as_deref(), Some(FOLDER_NAME_EMPTY), "{name:?}");
            assert_eq!(outcome.row, None);
        }
        assert_eq!(all_dirs(&target.dir), dirs_before);
        assert_eq!(forest.nodes.len(), rows_before);
    }

    #[test]
    fn an_existing_name_is_refused_and_nothing_is_merged_or_overwritten() {
        let (_dir, roots) = temp_tree();
        let rebuild = || -> Result<Vec<SectionSpec>> { panic!("nothing to rebuild") };
        let mut forest = forest_of(list_sections_with_dirs(&roots));
        let target = folder_target(&roots, Scope::Personal, "");
        std::fs::write(target.dir.join("plain"), "not a note").unwrap();
        let files_before = all_files(&roots);
        let dirs_before = all_dirs(&target.dir);

        for name in ["ideas", "Ideas", "plain"] {
            let outcome = create_folder(&mut forest, &target, name, &rebuild);
            let message = outcome.message.expect("a refusal");
            assert!(message.contains("already exists"), "{name}: {message}");
            assert_eq!(outcome.row, None);
        }
        assert_eq!(all_files(&roots), files_before);
        assert_eq!(all_dirs(&target.dir), dirs_before);
        assert_eq!(std::fs::read_to_string(target.dir.join("plain")).unwrap(), "not a note");
    }

    #[test]
    fn a_folder_the_rebuild_does_not_list_is_reported_by_path() {
        let (_dir, roots) = temp_tree();
        let rebuild = || Ok(list_sections(&roots));
        let mut forest = forest_of(list_sections(&roots));
        let target = folder_target(&roots, Scope::Global, "");

        let outcome = create_folder(&mut forest, &target, "unlisted", &rebuild);

        assert!(target.dir.join("unlisted").is_dir());
        assert_eq!(outcome.row, None);
        assert!(outcome.message.unwrap().contains("unlisted"));
    }

    #[test]
    fn new_folder_key_is_a_browse_key_in_the_table_and_the_help() {
        let rows: Vec<(usize, &KeyHint)> =
            TREE_KEYS.iter().enumerate().filter(|(_, k)| k.key == "N").collect();
        assert_eq!(rows.len(), 1);
        let (idx, hint) = rows[0];
        assert_eq!(hint.modes, BROWSE);
        assert_eq!(hint.group, Group::Edit);
        assert!(hint.help.starts_with("new folder"), "{}", hint.help);
        assert!(help::rows(TREE_KEYS).contains(&help::Row::Key(idx)));
        let n = TREE_KEYS.iter().position(|k| k.key == "n").unwrap();
        assert_eq!(idx, n + 1, "listed right after n");
        assert!(shown_keys(Mode::Normal, &[], 200).contains(&"N"));
        assert!(shown_keys(Mode::Focus, &[], 200).contains(&"N"));
        for mode in [Mode::Tag, Mode::Filter, Mode::Rename, Mode::NewItem, Mode::ConfirmDelete, Mode::VimCommand] {
            assert!(!shown_keys(mode, &[], 200).contains(&"N"), "{mode:?}");
        }
    }

    /// The listing's directories survive a rebuild after a note delete: a
    /// folder emptied by the delete keeps its row and takes the cursor.
    #[test]
    fn deleting_the_last_note_in_a_listed_folder_keeps_the_folder() {
        let (_dir, roots) = temp_tree();
        let personal = roots[0].1.clone();
        std::fs::create_dir_all(personal.join("solo")).unwrap();
        std::fs::write(personal.join("solo/only.md"), "# o\n").unwrap();
        let rebuild = || Ok(list_sections_with_dirs(&roots));
        let mut forest = forest_of(list_sections_with_dirs(&roots));
        let mut retired = Vec::new();
        let prompt = prompt_at(&mut forest, &personal.join("solo/only.md"));

        let outcome = answer_delete(KeyCode::Char('y'), &mut forest, &mut retired, &prompt, "", &rebuild).unwrap();

        let solo = row(&forest.nodes, path_str(&personal.join("solo")));
        assert_eq!(outcome.row, Some(solo));
    }

    // --- Folder rename and delete ---

    /// A vault laid out like the real one: the notez root `n` is the tag
    /// root of the personal section `n/personal/proj`, so its `.tags` keys
    /// read `personal/proj/...`. The section holds `ideas/a.md`,
    /// `ideas/deep/deeper/n.md` (two levels under `ideas`), the empty
    /// `ideas/empty/`, `plans/p.md` and `top.md`; the global section holds
    /// `g.md`. Returns the temp dir, the notez root and the section root.
    fn vault() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let notez = dir.path().join("n");
        let root = notez.join("personal/proj");
        for sub in ["ideas/deep/deeper", "ideas/empty", "plans"] {
            std::fs::create_dir_all(root.join(sub)).unwrap();
        }
        for file in ["ideas/a.md", "ideas/deep/deeper/n.md", "plans/p.md", "top.md"] {
            std::fs::write(root.join(file), "# note\n").unwrap();
        }
        std::fs::write(notez.join("g.md"), "# g\n").unwrap();
        std::fs::write(
            notez.join(".tags"),
            "personal/proj/ideas/a.md:1\n\
             personal/proj/ideas/deep/deeper/n.md:2\n\
             personal/proj/ideas/gone-before.md:1\n\
             personal/proj/plans/p.md:4\n\
             personal/proj/ideas-other.md:1\n\
             g.md:1\n",
        )
        .unwrap();
        (dir, notez, root)
    }

    /// The rebuild closure's work for [`vault`]: the personal section with
    /// its folders, then the global section (which leaves `personal/` out).
    fn vault_sections(notez: &Path) -> Vec<SectionSpec> {
        let root = notez.join("personal/proj");
        let personal = SectionSpec {
            root: root.clone(),
            tag_root: notez.to_path_buf(),
            label: "proj (personal)".to_string(),
            icon: "",
            is_doc: false,
            files: md_files(&root),
            dirs: all_dirs(&root),
            scope: Scope::Personal,
            project: Some("proj".to_string()),
            new_note_root: root.clone(),
            is_current: true,
        };
        let global = SectionSpec {
            root: notez.to_path_buf(),
            tag_root: notez.to_path_buf(),
            label: "NOTEZ".to_string(),
            icon: "",
            is_doc: false,
            files: vec![notez.join("g.md")],
            dirs: Vec::new(),
            scope: Scope::Global,
            project: None,
            new_note_root: notez.to_path_buf(),
            is_current: false,
        };
        vec![personal, global]
    }

    /// Every entry under `dir`, files and folders, relative to it.
    fn disk_entries(dir: &Path) -> Vec<String> {
        let mut out = Vec::new();
        let mut stack = vec![dir.to_path_buf()];
        while let Some(at) = stack.pop() {
            for entry in std::fs::read_dir(&at).unwrap().flatten() {
                let path = entry.path();
                out.push(path.strip_prefix(dir).unwrap().to_string_lossy().into_owned());
                if entry.file_type().unwrap().is_dir() {
                    stack.push(path);
                }
            }
        }
        out.sort();
        out
    }

    /// `rename_request` then `rename_folder` with the cursor on `path`, as
    /// `r`, a typed name and `Enter` do.
    fn rename_at(forest: &mut Forest, path: &Path, name: &str) -> std::result::Result<(), String> {
        let i = row(&forest.nodes, path_str(path));
        let shown = rename_request(&forest.nodes, &forest.sections, Some(i))
            .expect("a folder row opens the prompt")
            .expect("a row under the cursor");
        assert_eq!(shown, forest.nodes[i].name, "the prompt shows the current name");
        rename_folder(&mut forest.nodes, &mut forest.sections, i, name)
    }

    /// Enter on the untouched prompt keeps a name `sanitize::name` would
    /// change (underscores, capitals): nothing on disk, in the rows or in
    /// the tags moves.
    #[test]
    fn folder_rename_with_the_shown_name_unchanged_changes_nothing() {
        let (_dir, notez, root) = vault();
        for sub in ["00_quick-notes", "_todos/IDEAS"] {
            std::fs::create_dir_all(root.join(sub)).unwrap();
        }
        for file in ["00_quick-notes/q.md", "_todos/t.md", "_todos/IDEAS/i.md"] {
            std::fs::write(root.join(file), "# note\n").unwrap();
        }
        let mut forest = forest_of(vault_sections(&notez));
        let before = disk_entries(&notez);
        let paths = |f: &Forest| f.nodes.iter().map(|n| n.path.clone()).collect::<Vec<_>>();
        let rows = paths(&forest);

        for sub in ["00_quick-notes", "_todos", "_todos/IDEAS"] {
            let path = root.join(sub);
            let shown = forest.nodes[row(&forest.nodes, path_str(&path))].name.clone();
            assert_eq!(rename_at(&mut forest, &path, &shown), Ok(()), "{sub}");
            assert_eq!(disk_entries(&notez), before, "{sub}");
            assert_eq!(paths(&forest), rows, "{sub}");
        }
        assert!(changed_tag_maps(&forest.nodes, &forest.tag_roots, &forest.initial).is_empty());
    }

    #[test]
    fn folder_rename_moves_every_note_and_the_tags_follow_on_exit() {
        let (_dir, notez, root) = vault();
        let mut forest = forest_of(vault_sections(&notez));
        let initial = note_tags::load_tags(&notez);
        let ideas = row(&forest.nodes, path_str(&root.join("ideas")));

        rename_at(&mut forest, &root.join("ideas"), "Thoughts").expect("renamed");

        let thoughts = root.join("thoughts");
        assert!(!root.join("ideas").exists());
        for file in ["a.md", "deep/deeper/n.md"] {
            assert!(thoughts.join(file).is_file(), "{file}");
        }
        assert!(thoughts.join("empty").is_dir());
        assert_eq!(forest.nodes[ideas].path, thoughts, "the cursor row is the renamed folder");
        assert_eq!(forest.nodes[ideas].name, "thoughts");
        let deep = forest.nodes.iter().find(|n| n.name == "n.md").unwrap();
        assert_eq!(deep.path, thoughts.join("deep/deeper/n.md"));
        assert_eq!(deep.origin, root.join("ideas/deep/deeper/n.md"), "origin is kept");
        assert!(forest.nodes.iter().all(|n| !n.path.starts_with(root.join("ideas"))));
        assert!(forest.sections[0].dirs.contains(&thoughts.join("deep/deeper")));
        assert!(forest.sections[0].files.contains(&thoughts.join("a.md")));

        let mut expected = initial.clone();
        for (old, new) in [("ideas/a.md", "thoughts/a.md"), ("ideas/deep/deeper/n.md", "thoughts/deep/deeper/n.md")] {
            let flags = expected.remove(&format!("personal/proj/{old}")).unwrap();
            expected.insert(format!("personal/proj/{new}"), flags);
        }
        let changed = changed_tag_maps(&forest.nodes, &forest.tag_roots, &forest.initial);
        assert_eq!(changed, vec![(notez.clone(), expected.clone())]);
        assert!(expected.contains_key("personal/proj/ideas/gone-before.md"), "keys without a row stay");
        assert!(expected.contains_key("personal/proj/ideas-other.md"), "a look-alike prefix stays");

        // A later rebuild (a new note, say) lists the new paths and keeps
        // the origins, so the exit write is the same.
        forest.rebuild(vault_sections(&notez), Path::new("/nothing/created"));
        let after = changed_tag_maps(&forest.nodes, &forest.tag_roots, &forest.initial);
        assert_eq!(after, vec![(notez.clone(), expected)]);
    }

    #[test]
    fn folder_rename_onto_an_existing_name_changes_nothing() {
        let (_dir, notez, root) = vault();
        std::fs::write(root.join("plain"), "not a note").unwrap();
        let mut forest = forest_of(vault_sections(&notez));
        let disk = disk_entries(&notez);
        let rows: Vec<PathBuf> = forest.nodes.iter().map(|n| n.path.clone()).collect();

        // A sibling folder (any case), a sibling file, and a nested folder
        // onto its own sibling.
        for (folder, name) in [("ideas", "plans"), ("ideas", "Plans"), ("ideas", "plain"), ("ideas/deep", "empty")] {
            let message = rename_at(&mut forest, &root.join(folder), name).expect_err(name);
            assert!(message.contains("already exists"), "{name}: {message}");
        }
        assert_eq!(disk_entries(&notez), disk, "nothing moved");
        let now: Vec<PathBuf> = forest.nodes.iter().map(|n| n.path.clone()).collect();
        assert_eq!(now, rows, "no row path changed");
        assert!(changed_tag_maps(&forest.nodes, &forest.tag_roots, &forest.initial).is_empty());
    }

    #[test]
    fn folder_rename_refuses_an_empty_name_and_keeps_the_same_name() {
        let (_dir, notez, root) = vault();
        let mut forest = forest_of(vault_sections(&notez));
        let disk = disk_entries(&notez);
        for name in ["", "  ", "?!"] {
            assert_eq!(rename_at(&mut forest, &root.join("ideas"), name), Err(FOLDER_RENAME_EMPTY.to_string()));
        }
        assert_eq!(rename_at(&mut forest, &root.join("ideas"), "Ideas"), Ok(()), "same name: nothing to do");
        assert_eq!(disk_entries(&notez), disk);
        assert!(changed_tag_maps(&forest.nodes, &forest.tag_roots, &forest.initial).is_empty());
    }

    /// `Ideas` to `ideas`: on a case-insensitive file system the target
    /// "exists" as the folder itself, which is no reason to refuse.
    #[test]
    fn folder_rename_can_change_only_the_case() {
        let (_dir, notez, root) = vault();
        std::fs::create_dir_all(root.join("Caps")).unwrap();
        std::fs::write(root.join("Caps/c.md"), "# c\n").unwrap();
        let mut forest = forest_of(vault_sections(&notez));

        rename_at(&mut forest, &root.join("Caps"), "caps").expect("a case-only rename");

        let names: Vec<String> = std::fs::read_dir(&root)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert!(names.contains(&"caps".to_string()) && !names.contains(&"Caps".to_string()), "{names:?}");
        assert!(forest.nodes.iter().any(|n| n.path == root.join("caps/c.md")));
    }

    #[test]
    fn r_on_a_section_a_docs_folder_or_an_empty_tree_changes_nothing() {
        let sections = project_sections();
        let (nodes, _) = build_forest(&sections);
        for path in ["/n/personal/proj", "/p/docs", "/n"] {
            assert_eq!(rename_request(&nodes, &sections, Some(row(&nodes, path))), Err(SECTION_RENAME), "{path}");
        }
        assert_eq!(rename_request(&nodes, &sections, Some(row(&nodes, "/p/docs/design"))), Ok(None));
        assert_eq!(
            rename_request(&nodes, &sections, Some(row(&nodes, "/p/docs/design/c.md"))),
            Ok(Some("c".to_string())),
            "a docs note renames as before"
        );
        assert_eq!(
            rename_request(&nodes, &sections, Some(row(&nodes, "/n/personal/proj/ideas"))),
            Ok(Some("ideas".to_string()))
        );
        assert_eq!(rename_request(&nodes, &sections, None), Ok(None));
        assert_eq!(rename_request(&nodes, &sections, Some(nodes.len())), Ok(None));
        assert_eq!(rename_request(&[], &[], Some(0)), Ok(None));
        assert_eq!(rename_request(&[], &[], None), Ok(None));
    }

    /// The delete prompt `d` opens on the folder at `path`, whose parents
    /// are expanded as they are when the cursor can reach it.
    fn folder_prompt(forest: &mut Forest, path: &Path) -> DeletePrompt {
        let i = row(&forest.nodes, path_str(path));
        let mut up = forest.nodes[i].parent_idx;
        while let Some(p) = up {
            forest.nodes[p].expanded = true;
            up = forest.nodes[p].parent_idx;
        }
        delete_request(&forest.nodes, &forest.sections, Some(i), Some("proj"))
            .expect("a folder row")
            .expect("a row under the cursor")
    }

    #[test]
    fn d_then_y_on_a_folder_removes_it_and_nothing_else_and_retires_its_keys() {
        let (_dir, notez, root) = vault();
        let rebuild = || Ok(vault_sections(&notez));
        let mut forest = forest_of(vault_sections(&notez));
        let mut retired = Vec::new();
        let initial = note_tags::load_tags(&notez);
        let ideas = root.join("ideas");
        let disk: Vec<String> = disk_entries(&notez)
            .into_iter()
            .filter(|e| !Path::new(e).starts_with("personal/proj/ideas"))
            .collect();

        let prompt = folder_prompt(&mut forest, &ideas);
        assert_eq!(delete_question(&prompt), "delete ideas/ and its 2 notes from personal? y/n");
        let outcome = answer_delete(KeyCode::Char('y'), &mut forest, &mut retired, &prompt, "", &rebuild)
            .expect("y confirms");

        assert!(!ideas.exists());
        assert_eq!(disk_entries(&notez), disk, "only the folder went");
        assert_eq!(outcome.message, "deleted ideas/");
        assert!(!forest.nodes.iter().any(|n| n.path.starts_with(&ideas)));
        assert_eq!(outcome.row, Some(row(&forest.nodes, path_str(&root.join("plans")))), "the next sibling");

        let changed = changed_tag_maps_retiring(&forest.nodes, &forest.tag_roots, &forest.initial, &retired);
        let mut expected = initial.clone();
        expected.retain(|k, _| !Path::new(k).starts_with("personal/proj/ideas"));
        assert!(expected.contains_key("personal/proj/ideas-other.md"));
        assert!(expected.contains_key("g.md") && expected.contains_key("personal/proj/plans/p.md"));
        assert_eq!(changed, vec![(notez.clone(), expected)]);
    }

    #[test]
    fn deleting_a_renamed_folder_retires_the_original_keys() {
        let (_dir, notez, root) = vault();
        let rebuild = || Ok(vault_sections(&notez));
        let mut forest = forest_of(vault_sections(&notez));
        let mut retired = Vec::new();
        rename_at(&mut forest, &root.join("ideas"), "thoughts").unwrap();

        let prompt = folder_prompt(&mut forest, &root.join("thoughts"));
        answer_delete(KeyCode::Char('y'), &mut forest, &mut retired, &prompt, "", &rebuild).unwrap();

        let changed = changed_tag_maps_retiring(&forest.nodes, &forest.tag_roots, &forest.initial, &retired);
        let (_, map) = &changed[0];
        assert!(!map.keys().any(|k| k.contains("/thoughts/")), "{map:?}");
        for old in ["personal/proj/ideas/a.md", "personal/proj/ideas/deep/deeper/n.md"] {
            assert!(!map.contains_key(old), "{old}: {map:?}");
        }
        assert!(map.contains_key("personal/proj/ideas-other.md"));
    }

    #[test]
    fn the_folder_prompt_counts_notes_and_mentions_other_files() {
        let (_dir, notez, root) = vault();
        std::fs::create_dir_all(root.join("one")).unwrap();
        std::fs::write(root.join("one/x.md"), "# x\n").unwrap();
        std::fs::create_dir_all(root.join("mixed/sub")).unwrap();
        std::fs::write(root.join("mixed/sub/m.md"), "# m\n").unwrap();
        std::fs::write(root.join("mixed/sub/upper.MD"), "# m\n").unwrap();
        std::fs::write(root.join("mixed/pic.png"), "png").unwrap();
        std::fs::create_dir_all(root.join("hidden-only")).unwrap();
        std::fs::write(root.join("hidden-only/.DS_Store"), "").unwrap();
        let mut forest = forest_of(vault_sections(&notez));
        let mut question = |sub: &str| delete_question(&folder_prompt(&mut forest, &root.join(sub)));

        assert_eq!(question("ideas/empty"), "delete ideas/empty/ (no notes) from personal? y/n");
        assert_eq!(question("one"), "delete one/ and its 1 note from personal? y/n");
        assert_eq!(question("ideas"), "delete ideas/ and its 2 notes from personal? y/n");
        assert_eq!(question("mixed"), "delete mixed/ and its 2 notes and other files from personal? y/n");
        assert_eq!(question("hidden-only"), "delete hidden-only/ (no notes) and other files from personal? y/n");
    }

    /// A symlink inside the folder goes as a link: its target, and the
    /// notes in it, stay; the prompt counts it as another file.
    #[cfg(unix)]
    #[test]
    fn a_folder_delete_removes_a_symlink_inside_it_but_not_its_target() {
        let (_dir, notez, root) = vault();
        let rebuild = || Ok(vault_sections(&notez));
        std::os::unix::fs::symlink(root.join("plans"), root.join("ideas/link")).unwrap();
        let mut forest = forest_of(vault_sections(&notez));
        let mut retired = Vec::new();

        let prompt = folder_prompt(&mut forest, &root.join("ideas"));
        assert_eq!(delete_question(&prompt), "delete ideas/ and its 2 notes and other files from personal? y/n");
        answer_delete(KeyCode::Char('y'), &mut forest, &mut retired, &prompt, "", &rebuild).unwrap();

        assert!(!root.join("ideas").exists());
        assert!(root.join("plans/p.md").is_file(), "the link's target is untouched");
        assert!(!retired.iter().any(|(_, k)| k.contains("plans")), "{retired:?}");
    }

    #[test]
    fn a_local_folder_prompt_says_not_recoverable() {
        let (_dir, roots) = temp_tree();
        let mut forest = forest_of(list_sections_with_dirs(&roots));
        for (scope, _) in SCOPES {
            let root = &roots.iter().find(|(s, _)| *s == scope).unwrap().1;
            let q = delete_question(&folder_prompt(&mut forest, &root.join("ideas")));
            assert_eq!(q.contains("(not recoverable)"), scope == Scope::Local, "{q}");
            assert!(q.starts_with("delete ideas/ and its 2 notes from "), "{q}");
        }
    }

    #[test]
    fn any_answer_but_y_on_a_folder_removes_nothing() {
        let (_dir, notez, root) = vault();
        let rebuild = || Ok(vault_sections(&notez));
        let mut forest = forest_of(vault_sections(&notez));
        let mut retired = Vec::new();
        let prompt = folder_prompt(&mut forest, &root.join("ideas"));
        let disk = disk_entries(&notez);
        for code in [KeyCode::Char('n'), KeyCode::Esc, KeyCode::Enter, KeyCode::Char('Y')] {
            assert!(answer_delete(code, &mut forest, &mut retired, &prompt, "", &rebuild).is_none());
        }
        assert_eq!(disk_entries(&notez), disk);
        assert!(retired.is_empty());
    }

    #[test]
    fn remove_dir_all_is_unreachable_for_a_section_root_or_a_path_outside_it() {
        let (_dir, notez, root) = vault();
        let roots = vec![root.clone(), notez.clone()];
        let refused = [
            (root.clone(), root.clone(), "the section root"),
            (notez.clone(), root.clone(), "above the section root"),
            (notez.join("elsewhere"), root.clone(), "outside the section"),
            (root.join("ideas/../plans"), root.clone(), "a parent step"),
            (root.join(".."), root.clone(), "a parent step alone"),
            (notez.join("personal"), notez.clone(), "a folder holding another section"),
            (notez.join("personal/proj"), notez.clone(), "another section's root"),
        ];
        std::fs::create_dir_all(notez.join("elsewhere")).unwrap();
        let disk = disk_entries(&notez);
        for (path, section_root, why) in refused {
            assert!(!folder_change_allowed(&path, &section_root, &roots), "{why}");
            let err = remove_folder(&path, &section_root, &roots).expect_err(why);
            assert!(err.to_string().contains("not a folder inside its section"), "{why}: {err}");
        }
        assert_eq!(disk_entries(&notez), disk, "nothing removed");
        assert!(folder_change_allowed(&root.join("ideas/deep"), &root, &roots));
    }

    #[test]
    fn a_prompt_aimed_at_a_section_root_removes_nothing() {
        let (_dir, notez, root) = vault();
        let rebuild = || Ok(vault_sections(&notez));
        let mut forest = forest_of(vault_sections(&notez));
        let mut retired = Vec::new();
        let mut prompt = folder_prompt(&mut forest, &root.join("ideas"));
        prompt.path = root.clone();
        let disk = disk_entries(&notez);

        let outcome = answer_delete(KeyCode::Char('y'), &mut forest, &mut retired, &prompt, "", &rebuild).unwrap();

        assert!(outcome.message.starts_with("delete failed: "), "{}", outcome.message);
        assert_eq!(disk_entries(&notez), disk);
        assert!(retired.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn a_folder_delete_that_fails_midway_reports_rebuilds_and_retires_only_what_went() {
        use std::os::unix::fs::PermissionsExt;
        let (_dir, notez, root) = vault();
        let locked = root.join("ideas/deep/deeper");
        let rebuild = || Ok(vault_sections(&notez));
        let mut forest = forest_of(vault_sections(&notez));
        let mut retired = Vec::new();
        let prompt = folder_prompt(&mut forest, &root.join("ideas"));

        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o555)).unwrap();
        let outcome = answer_delete(KeyCode::Char('y'), &mut forest, &mut retired, &prompt, "", &rebuild);
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
        let outcome = outcome.unwrap();

        assert!(outcome.message.starts_with("delete failed: "), "{}", outcome.message);
        assert!(locked.join("n.md").is_file(), "the locked note is left");
        assert!(root.join("plans/p.md").is_file() && root.join("top.md").is_file());
        assert!(forest.nodes.iter().any(|n| n.path == locked.join("n.md")), "the tree shows what is left");
        assert_eq!(outcome.row, Some(row(&forest.nodes, path_str(&root.join("ideas")))));
        let keys: Vec<&str> = retired.iter().map(|(_, k)| k.as_str()).collect();
        assert!(!keys.contains(&"personal/proj/ideas/deep/deeper/n.md"), "{keys:?}");
        assert_eq!(
            keys.contains(&"personal/proj/ideas/a.md"),
            !root.join("ideas/a.md").exists(),
            "a note's key goes exactly when the note went: {keys:?}"
        );
        assert!(keys.contains(&"personal/proj/ideas/gone-before.md"), "a key whose note is not on disk goes");
    }

    #[test]
    fn a_failed_rebuild_after_a_folder_delete_drops_the_folder_rows() {
        let (_dir, notez, root) = vault();
        let rebuild = || -> Result<Vec<SectionSpec>> { anyhow::bail!("listing broke") };
        let mut forest = forest_of(vault_sections(&notez));
        let mut retired = Vec::new();
        let prompt = folder_prompt(&mut forest, &root.join("ideas"));

        let outcome = answer_delete(KeyCode::Char('y'), &mut forest, &mut retired, &prompt, "", &rebuild).unwrap();

        assert!(!root.join("ideas").exists());
        assert!(outcome.message.contains("listing broke"), "{}", outcome.message);
        assert!(!forest.nodes.iter().any(|n| n.path.starts_with(root.join("ideas"))));
        assert!(forest.nodes.iter().any(|n| n.path == root.join("plans/p.md")));
    }

    #[test]
    fn r_and_d_help_cover_folders_and_keep_their_footer_slots() {
        let hint = |k: &str| TREE_KEYS.iter().find(|h| h.key == k && h.modes == BROWSE).unwrap();
        assert!(hint("r").help.starts_with("rename note or folder"), "{}", hint("r").help);
        assert!(hint("d").help.starts_with("delete note or folder"), "{}", hint("d").help);
        assert_eq!(hint("r").slot, Slot::Priority(6));
        assert_eq!(hint("d").slot, Slot::Priority(7));
    }

    // --- Move ---

    /// A notez root `n` and the repository `p` of the project `proj`, with
    /// one store per scope: personal `n/personal/proj` and global `n` (both
    /// keyed in `n/.tags`), public `p/notez` and local `p/.notez` (each with
    /// its own `.tags`), plus `p/docs/d.md`. Every store holds `ideas/a.md`
    /// (tagged), `ideas/b.md` (untagged), `c.md` (tagged) and the empty
    /// folder `plans/`. Returns the temp dir, the notez root and the stores.
    fn move_fixture() -> (tempfile::TempDir, PathBuf, Vec<(Scope, PathBuf)>) {
        let dir = tempfile::tempdir().unwrap();
        let notez = dir.path().join("n");
        let repo = dir.path().join("p");
        let roots = vec![
            (Scope::Personal, notez.join("personal/proj")),
            (Scope::Public, repo.join("notez")),
            (Scope::Local, repo.join(".notez")),
            (Scope::Global, notez.clone()),
        ];
        for (_, root) in &roots {
            std::fs::create_dir_all(root.join("ideas")).unwrap();
            std::fs::create_dir_all(root.join("plans")).unwrap();
            for file in ["ideas/a.md", "ideas/b.md", "c.md"] {
                std::fs::write(root.join(file), "# note\n").unwrap();
            }
        }
        std::fs::create_dir_all(repo.join("docs")).unwrap();
        std::fs::write(repo.join("docs/d.md"), "# doc\n").unwrap();
        std::fs::write(
            notez.join(".tags"),
            "personal/proj/ideas/a.md:1\npersonal/proj/c.md:2\nideas/a.md:4\nc.md:8\nother.md:16\n",
        )
        .unwrap();
        for store in ["notez", ".notez"] {
            std::fs::write(repo.join(store).join(".tags"), "ideas/a.md:3\nc.md:2\n").unwrap();
        }
        (dir, notez, roots)
    }

    /// The tag root a store of `scope` keys its notes from, as
    /// `commands::tree` sets it.
    fn move_tag_root(roots: &[(Scope, PathBuf)], scope: Scope) -> PathBuf {
        let store = |s: Scope| roots.iter().find(|(x, _)| *x == s).unwrap().1.clone();
        match scope {
            Scope::Personal | Scope::Global => store(Scope::Global),
            other => store(other),
        }
    }

    /// The rebuild closure's work for [`move_fixture`]: personal, public,
    /// docs, local and global sections, listed from the disk.
    fn move_sections(roots: &[(Scope, PathBuf)]) -> Vec<SectionSpec> {
        let notez = move_tag_root(roots, Scope::Global);
        let personal_dir = notez.join("personal");
        let docs = move_tag_root(roots, Scope::Public).parent().unwrap().join("docs");
        let section = |scope: Scope, root: &Path, is_doc: bool| {
            let outside = |p: &PathBuf| scope == Scope::Global && p.starts_with(&personal_dir);
            SectionSpec {
                root: root.to_path_buf(),
                tag_root: if is_doc { root.to_path_buf() } else { move_tag_root(roots, scope) },
                label: format!("{scope:?}"),
                icon: "",
                is_doc,
                files: md_files(root).into_iter().filter(|p| !outside(p)).collect(),
                dirs: all_dirs(root).into_iter().filter(|p| !outside(p)).collect(),
                scope,
                project: (scope != Scope::Global).then(|| "proj".to_string()),
                new_note_root: if is_doc { roots[0].1.clone() } else { root.to_path_buf() },
                is_current: scope != Scope::Global,
            }
        };
        let mut out: Vec<SectionSpec> = roots
            .iter()
            .filter(|(s, _)| *s != Scope::Global)
            .map(|(s, root)| section(*s, root, false))
            .collect();
        out.insert(2, section(Scope::Public, &docs, true));
        out.push(section(Scope::Global, &notez, false));
        out
    }

    fn move_ctx(roots: &[(Scope, PathBuf)]) -> TreeContext {
        let stores = roots.iter().filter(|(s, _)| *s != Scope::Global).cloned().collect();
        let new_note_roots = NewNoteRoots {
            global: move_tag_root(roots, Scope::Global),
            projects: HashMap::from([("proj".to_string(), stores)]),
        };
        ctx_with(Some("proj"), new_note_roots)
    }

    fn store_of(roots: &[(Scope, PathBuf)], scope: Scope) -> PathBuf {
        roots.iter().find(|(s, _)| *s == scope).unwrap().1.clone()
    }

    /// `m` on `src`, `Tab` until the prompt targets `scope`, the buffer set
    /// to `folder`, then `Enter`: the resolved move or the footer message.
    fn move_plan(
        forest: &Forest,
        ctx: &TreeContext,
        src: &Path,
        scope: Scope,
        folder: &str,
    ) -> std::result::Result<MovePlan, String> {
        let i = row(&forest.nodes, path_str(src));
        let mut prompt = move_request(&forest.nodes, &forest.sections, Some(i), ctx).expect("a note row");
        for _ in 0..4 {
            if prompt.scope.target.scope == scope {
                break;
            }
            let p = &prompt.scope;
            prompt.scope.target = next_scope_target(
                &p.target,
                &p.origin,
                p.project.as_deref(),
                &ctx.new_note_roots,
                ctx.current_project.as_deref(),
            );
        }
        assert_eq!(prompt.scope.target.scope, scope, "Tab reaches {scope:?}");
        prompt.scope.buffer = folder.to_string();
        resolve_move(&prompt, &ctx.new_note_roots)
    }

    /// Every final tag map after exit, by tag root: the changed ones as the
    /// exit write reports them, the rest as loaded from the disk.
    fn final_maps(
        forest: &Forest,
        retired: &[(PathBuf, String)],
        carried: &[CarriedTags],
        roots: &[(Scope, PathBuf)],
    ) -> HashMap<PathBuf, HashMap<String, u8>> {
        let changed: HashMap<PathBuf, HashMap<String, u8>> =
            exit_tag_maps(forest, retired, carried).into_iter().collect();
        [Scope::Global, Scope::Public, Scope::Local]
            .into_iter()
            .map(|s| move_tag_root(roots, s))
            .map(|root| {
                let map = changed.get(&root).cloned().unwrap_or_else(|| note_tags::load_tags(&root));
                (root, map)
            })
            .collect()
    }

    fn rel_entries(dir: &Path, entries: Vec<String>, from: &Path, to: &Path) -> Vec<String> {
        let from = from.strip_prefix(dir).unwrap().to_string_lossy().into_owned();
        let to = to.strip_prefix(dir).unwrap().to_string_lossy().into_owned();
        let mut out: Vec<String> = entries.into_iter().map(|e| if e == from { to.clone() } else { e }).collect();
        out.sort();
        out
    }

    #[test]
    fn m_moves_a_note_to_a_folder_and_to_the_root_within_every_scope() {
        for (scope, name) in SCOPES {
            let (dir, _notez, roots) = move_fixture();
            let rebuild = || Ok(move_sections(&roots));
            let ctx = move_ctx(&roots);
            let mut forest = forest_of(move_sections(&roots));
            let (mut retired, mut carried) = (Vec::new(), Vec::new());
            let store = store_of(&roots, scope);

            let mut src = store.join("ideas/a.md");
            let i = row(&forest.nodes, path_str(&src));
            let prompt = move_request(&forest.nodes, &forest.sections, Some(i), &ctx).unwrap();
            assert_eq!(prompt.scope.buffer, "ideas", "{name}: prefilled with the note's folder");

            for (folder, dst) in [("plans", store.join("plans/a.md")), ("", store.join("a.md"))] {
                let before = disk_entries(dir.path());
                let plan = move_plan(&forest, &ctx, &src, scope, folder).unwrap();
                assert!(!plan.needs_confirm(), "{name}: same scope, no question");
                assert_eq!(disk_entries(dir.path()), before, "{name}: resolving touches nothing");
                let outcome = apply_move(&mut forest, &mut retired, &mut carried, &plan, &ctx.new_note_roots, &rebuild);

                assert_eq!(disk_entries(dir.path()), rel_entries(dir.path(), before, &src, &dst), "{name} {folder:?}");
                let row = outcome.row.expect("the moved note is listed");
                assert_eq!(forest.nodes[row].path, dst, "{name}: the cursor is on the moved note");
                assert!(get_visible_nodes(&forest.nodes).contains(&row), "{name}: its folders are expanded");
                let label = scope_label(scope, (scope != Scope::Global).then_some("proj"), Some("proj"));
                let shown = if folder.is_empty() { label } else { format!("{label}/{folder}") };
                assert_eq!(outcome.message, format!("moved {} to {shown}", plan.rel), "{name}");
                src = dst;
            }
            assert!(carried.is_empty());
        }
    }

    #[test]
    fn a_missing_folder_a_taken_name_or_the_same_folder_moves_nothing_and_says_so() {
        let (dir, _notez, roots) = move_fixture();
        let ctx = move_ctx(&roots);
        let forest = forest_of(move_sections(&roots));
        let personal = store_of(&roots, Scope::Personal);
        let src = personal.join("ideas/a.md");
        std::fs::write(personal.join("plans/a.md"), "taken").unwrap();
        let before = disk_entries(dir.path());

        let plan = |folder: &str| move_plan(&forest, &ctx, &src, Scope::Personal, folder).unwrap_err();
        assert_eq!(plan("nope"), "move: no folder personal/nope");
        assert_eq!(plan("ideas/a.md"), "move: no folder personal/ideas/a.md", "a file is no folder");
        assert_eq!(plan("../proj/plans"), "move: no folder personal/../proj/plans");
        assert_eq!(plan(".hidden"), "move: no folder personal/.hidden");
        assert_eq!(plan("plans"), "move: a.md already exists in personal/plans");
        assert_eq!(plan("ideas/"), "move: a.md is already in personal/ideas");
        let global = move_plan(&forest, &ctx, &src, Scope::Global, "personal/proj").unwrap_err();
        assert!(global.starts_with("move: global/personal/proj holds the projects' personal"), "{global}");
        let public = move_plan(&forest, &ctx, &src, Scope::Public, "nope").unwrap_err();
        assert_eq!(public, "move: no folder public (committed with the project)/nope");

        assert_eq!(disk_entries(dir.path()), before, "nothing moved");

        std::fs::remove_dir_all(store_of(&roots, Scope::Local)).unwrap();
        let local = move_plan(&forest, &ctx, &src, Scope::Local, "").unwrap_err();
        assert_eq!(local, "move: no folder local scratch", "a missing store root");
        assert!(src.is_file());
    }

    #[test]
    fn a_move_that_fails_retires_nothing_and_keeps_the_note() {
        let (_dir, _notez, roots) = move_fixture();
        let rebuild = || Ok(move_sections(&roots));
        let ctx = move_ctx(&roots);
        let mut forest = forest_of(move_sections(&roots));
        let (mut retired, mut carried) = (Vec::new(), Vec::new());
        let src = store_of(&roots, Scope::Personal).join("ideas/a.md");
        let plan = move_plan(&forest, &ctx, &src, Scope::Public, "plans").unwrap();
        std::fs::write(&plan.dst, "appeared meanwhile").unwrap();

        let outcome = apply_move(&mut forest, &mut retired, &mut carried, &plan, &ctx.new_note_roots, &rebuild);
        assert!(outcome.message.starts_with("move failed: ") && outcome.message.contains("already exists"), "{}", outcome.message);
        assert!(retired.is_empty() && carried.is_empty());
        assert_eq!(std::fs::read_to_string(&plan.dst).unwrap(), "appeared meanwhile");
        assert!(src.is_file());
        assert_eq!(forest.nodes[outcome.row.unwrap()].path, src, "the cursor stays on the note");
        assert!(final_maps(&forest, &retired, &carried, &roots)
            .into_iter()
            .all(|(root, map)| map == note_tags::load_tags(&root)));
    }

    /// Every ordered pair of different scopes: a tagged and an untagged note
    /// move; the tagged note's key leaves the source tag root and arrives in
    /// the destination one with its flags, the untagged one gets no key, and
    /// every other key stays.
    #[test]
    fn a_move_across_scopes_carries_the_tags_for_every_pair() {
        for (from, from_name) in SCOPES {
            for (to, to_name) in SCOPES {
                if from == to {
                    continue;
                }
                let pair = format!("{from_name} to {to_name}");
                let (_dir, _notez, roots) = move_fixture();
                let rebuild = || Ok(move_sections(&roots));
                let ctx = move_ctx(&roots);
                let mut forest = forest_of(move_sections(&roots));
                let (mut retired, mut carried) = (Vec::new(), Vec::new());
                let (src_store, dst_store) = (store_of(&roots, from), store_of(&roots, to));
                let (src_tags, dst_tags) = (move_tag_root(&roots, from), move_tag_root(&roots, to));
                let initial = final_maps(&forest, &[], &[], &roots);
                let flags = forest.nodes[row(&forest.nodes, path_str(&src_store.join("ideas/a.md")))].flags;
                assert_ne!(flags, 0, "{pair}");

                for file in ["a.md", "b.md"] {
                    let src = src_store.join("ideas").join(file);
                    let plan = move_plan(&forest, &ctx, &src, to, "plans").unwrap();
                    assert!(plan.needs_confirm(), "{pair}");
                    let outcome = apply_move(&mut forest, &mut retired, &mut carried, &plan, &ctx.new_note_roots, &rebuild);
                    let dst = dst_store.join("plans").join(file);
                    assert!(dst.is_file() && !src.exists(), "{pair} {file}");
                    let moved = &forest.nodes[outcome.row.expect("listed")];
                    assert_eq!(moved.path, dst, "{pair}");
                    assert_eq!(forest.sections[moved.section].root, dst_store, "{pair}: section");
                    assert_eq!(forest.tag_roots[moved.tag_root], dst_tags, "{pair}: tag root");
                }

                let mut expected = initial.clone();
                let key = |root: &Path, store: &Path, file: &str| rel_key(root, &store.join(file)).unwrap();
                expected.get_mut(&src_tags).unwrap().remove(&key(&src_tags, &src_store, "ideas/a.md"));
                expected.get_mut(&dst_tags).unwrap().insert(key(&dst_tags, &dst_store, "plans/a.md"), flags);
                let finals = final_maps(&forest, &retired, &carried, &roots);
                assert_eq!(finals, expected, "{pair}");
                for map in finals.values() {
                    assert!(!map.keys().any(|k| k.ends_with("b.md")), "{pair}: the untagged note has no key");
                }
                assert_eq!(finals[&move_tag_root(&roots, Scope::Global)].get("other.md"), Some(&16), "{pair}");
            }
        }
    }

    /// A destination outside the view (a one-scope view) is reported by
    /// path, and the exit write still moves the tags.
    #[test]
    fn a_move_out_of_the_view_reports_the_path_and_keeps_the_tags() {
        let (_dir, notez, roots) = move_fixture();
        let personal_only = || move_sections(&roots).into_iter().take(1).collect::<Vec<_>>();
        let rebuild = || Ok(personal_only());
        let ctx = move_ctx(&roots);
        let mut forest = forest_of(personal_only());
        let (mut retired, mut carried) = (Vec::new(), Vec::new());
        let personal = store_of(&roots, Scope::Personal);
        let public = store_of(&roots, Scope::Public);

        for file in ["a.md", "b.md"] {
            let plan = move_plan(&forest, &ctx, &personal.join("ideas").join(file), Scope::Public, "plans").unwrap();
            let outcome = apply_move(&mut forest, &mut retired, &mut carried, &plan, &ctx.new_note_roots, &rebuild);
            assert_eq!(outcome.row, None);
            assert_eq!(outcome.message, format!("moved to {}", public.join("plans").join(file).display()));
        }
        let finals = final_maps(&forest, &retired, &carried, &roots);
        assert_eq!(finals[&notez].get("personal/proj/ideas/a.md"), None);
        assert_eq!(finals[&public].get("plans/a.md"), Some(&1));
        assert_eq!(finals[&public].get("plans/b.md"), None);
        assert_eq!(finals[&public].get("ideas/a.md"), Some(&3), "other keys stay");
    }

    #[test]
    fn a_move_whose_list_refresh_fails_still_shows_the_note_at_its_new_place() {
        let (_dir, notez, roots) = move_fixture();
        let rebuild = || -> Result<Vec<SectionSpec>> { Err(anyhow::anyhow!("listing broke")) };
        let ctx = move_ctx(&roots);
        let mut forest = forest_of(move_sections(&roots));
        let (mut retired, mut carried) = (Vec::new(), Vec::new());
        let src = store_of(&roots, Scope::Personal).join("ideas/a.md");
        let plan = move_plan(&forest, &ctx, &src, Scope::Global, "plans").unwrap();

        let outcome = apply_move(&mut forest, &mut retired, &mut carried, &plan, &ctx.new_note_roots, &rebuild);
        assert!(outcome.message.contains("could not be refreshed: listing broke"), "{}", outcome.message);
        let moved = &forest.nodes[outcome.row.expect("listed from the current rows")];
        assert_eq!(moved.path, notez.join("plans/a.md"));
        let finals = final_maps(&forest, &retired, &carried, &roots);
        assert_eq!(finals[&notez].get("plans/a.md"), Some(&1));
        assert_eq!(finals[&notez].get("personal/proj/ideas/a.md"), None);
    }

    #[test]
    fn the_move_question_names_each_transition_public_first() {
        use Scope::{Global, Local, Personal, Public};
        let into_public = "(it will be in the repo repository, public, not yet committed)";
        let out_of_public = "(it stays in the repository's git history)";
        let into_local = "(scratch is not synced and not recoverable)";
        let leaves_vault = "(it leaves the vault; the deletion syncs on exit)";
        let cases: [(Scope, Scope, Vec<&str>); 12] = [
            (Personal, Public, vec![into_public, leaves_vault]),
            (Personal, Local, vec![into_local, leaves_vault]),
            (Personal, Global, vec![]),
            (Public, Personal, vec![out_of_public]),
            (Public, Local, vec![out_of_public, into_local]),
            (Public, Global, vec![out_of_public]),
            (Local, Personal, vec![]),
            (Local, Public, vec![into_public]),
            (Local, Global, vec![]),
            (Global, Personal, vec![]),
            (Global, Public, vec![into_public, leaves_vault]),
            (Global, Local, vec![into_local, leaves_vault]),
        ];
        for (from, to, clauses) in cases {
            let mut expected = "move ideas/a.md to dest/plans?".to_string();
            for clause in &clauses {
                expected = format!("{expected} {clause}");
            }
            expected.push_str(" y/n");
            assert_eq!(move_question("ideas/a.md", "dest/plans", from, to, "repo"), expected, "{from:?} to {to:?}");
            assert_eq!(expected.contains("public,"), to == Public, "{from:?} to {to:?}");
        }
    }

    #[test]
    fn the_confirm_opens_only_for_a_new_scope_names_the_repository_and_takes_only_y() {
        let (_dir, _notez, roots) = move_fixture();
        let ctx = move_ctx(&roots);
        let forest = forest_of(move_sections(&roots));
        let src = store_of(&roots, Scope::Personal).join("ideas/a.md");
        assert!(!move_plan(&forest, &ctx, &src, Scope::Personal, "plans").unwrap().needs_confirm());
        let plan = move_plan(&forest, &ctx, &src, Scope::Public, "plans").unwrap();
        assert!(plan.needs_confirm());
        assert_eq!(
            plan.question(),
            "move ideas/a.md to public (committed with the project)/plans? \
             (it will be in the p repository, public, not yet committed) \
             (it leaves the vault; the deletion syncs on exit) y/n"
        );
        let rendered = text_of(&lead_with_hints(move_confirm_lead(&plan), Mode::ConfirmMove, &[], 400));
        assert!(rendered.starts_with(&format!(" {}", plan.question())), "{rendered}");
        assert!(rendered.contains("y confirm") && rendered.contains("n/esc cancel"), "{rendered}");
        assert!(move_confirmed(KeyCode::Char('y')));
        for key in [KeyCode::Char('n'), KeyCode::Esc, KeyCode::Char('Y'), KeyCode::Enter] {
            assert!(!move_confirmed(key), "{key:?}");
        }
    }

    #[test]
    fn the_move_prompt_tabs_through_the_scopes_and_keeps_the_typed_folder() {
        let (_dir, notez, roots) = move_fixture();
        let ctx = move_ctx(&roots);
        let forest = forest_of(move_sections(&roots));
        let src = store_of(&roots, Scope::Personal).join("ideas/a.md");
        let mut prompt =
            move_request(&forest.nodes, &forest.sections, Some(row(&forest.nodes, path_str(&src))), &ctx).unwrap();
        assert_eq!(text_of(&Line::from(move_lead(&prompt))), " move a.md to personal/ideas_");
        prompt.scope.buffer = "plans".to_string();
        let mut seen = Vec::new();
        for _ in 0..4 {
            let p = &prompt.scope;
            prompt.scope.target = next_scope_target(&p.target, &p.origin, p.project.as_deref(), &ctx.new_note_roots, Some("proj"));
            seen.push((prompt.scope.target.scope, prompt.scope.target.dir.clone()));
            assert_eq!(prompt.scope.buffer, "plans");
        }
        assert_eq!(seen, vec![
            (Scope::Public, store_of(&roots, Scope::Public)),
            (Scope::Local, store_of(&roots, Scope::Local)),
            (Scope::Global, notez.clone()),
            (Scope::Personal, store_of(&roots, Scope::Personal)),
        ]);
        let rendered = text_of(&lead_with_hints(move_lead(&prompt), Mode::Move, &[], 200));
        assert!(rendered.starts_with(" move a.md to personal/plans_"), "{rendered}");

        // A global note offers the scopes of the project the browser is in.
        let global = notez.join("c.md");
        let mut prompt =
            move_request(&forest.nodes, &forest.sections, Some(row(&forest.nodes, path_str(&global))), &ctx).unwrap();
        assert_eq!(prompt.scope.buffer, "", "a note at the root has an empty folder");
        let p = &prompt.scope;
        prompt.scope.target = next_scope_target(&p.target, &p.origin, p.project.as_deref(), &ctx.new_note_roots, Some("proj"));
        assert_eq!(prompt.scope.target.scope, Scope::Personal);
        assert!(!seen.iter().any(|(_, dir)| dir.ends_with("docs")), "docs are never offered");
    }

    #[test]
    fn m_on_a_section_a_docs_row_or_an_empty_tree_is_refused() {
        let sections = project_sections();
        let (nodes, _) = build_forest(&sections);
        let ctx = ctx_with(Some("proj"), roots());
        let at = |path: &str| move_request(&nodes, &sections, Some(row(&nodes, path)), &ctx).err();
        assert_eq!(at("/n/personal/proj"), Some(SECTION_MOVE));
        assert_eq!(at("/p/docs/design/c.md"), Some(DOCS_MOVE));
        assert_eq!(at("/p/docs/design"), Some(DOCS_MOVE));
        assert!(at("/n/personal/proj/ideas").is_none(), "a folder opens the prompt");
        assert!(at("/n/personal/proj/ideas/a.md").is_none());
        assert_eq!(move_request(&[], &[], None, &ctx).err(), Some(NOTHING_TO_MOVE));
        assert_eq!(move_request(&[], &[], Some(0), &ctx).err(), Some(NOTHING_TO_MOVE));
        assert_eq!(move_request(&nodes, &sections, Some(nodes.len()), &ctx).err(), Some(NOTHING_TO_MOVE));
    }

    #[test]
    fn s_on_a_section_a_docs_row_or_an_empty_tree_is_refused() {
        let sections = project_sections();
        let (nodes, _) = build_forest(&sections);
        let ctx = ctx_with(Some("proj"), roots());
        let at = |path: &str| set_scope_request(&nodes, &sections, Some(row(&nodes, path)), &ctx).err();
        assert_eq!(at("/n/personal/proj"), Some(SECTION_SET_SCOPE));
        assert_eq!(at("/p/docs/design/c.md"), Some(DOCS_SET_SCOPE));
        assert_eq!(at("/p/docs/design"), Some(DOCS_SET_SCOPE));
        assert!(at("/n/personal/proj/ideas").is_none(), "a folder opens the prompt");
        assert!(at("/n/personal/proj/ideas/a.md").is_none());
        assert_eq!(set_scope_request(&[], &[], None, &ctx).err(), Some(NOTHING_TO_SET_SCOPE));
        assert_eq!(set_scope_request(&[], &[], Some(0), &ctx).err(), Some(NOTHING_TO_SET_SCOPE));
        assert_eq!(set_scope_request(&nodes, &sections, Some(nodes.len()), &ctx).err(), Some(NOTHING_TO_SET_SCOPE));
    }

    /// A folder that holds another section's root (a global folder over a
    /// project's personal store) is refused by both keys.
    #[test]
    fn m_and_s_refuse_a_folder_that_holds_another_section() {
        let (_dir, notez, roots) = move_fixture();
        let ctx = move_ctx(&roots);
        let mut sections = move_sections(&roots);
        let global = sections.iter_mut().find(|s| s.scope == Scope::Global).unwrap();
        global.dirs.push(notez.join("personal"));
        let (nodes, _) = build_forest(&sections);
        let i = Some(row(&nodes, path_str(&notez.join("personal"))));
        assert_eq!(move_request(&nodes, &sections, i, &ctx).err(), Some(FOLDER_HOLDS_SECTION_MOVE));
        assert_eq!(set_scope_request(&nodes, &sections, i, &ctx).err(), Some(FOLDER_HOLDS_SECTION_SET_SCOPE));
    }

    #[test]
    fn m_is_a_browse_key_after_n_with_its_prompt_and_confirm_keys() {
        let rows: Vec<(usize, &KeyHint)> = TREE_KEYS.iter().enumerate().filter(|(_, k)| k.key == "m").collect();
        assert_eq!(rows.len(), 1);
        let (idx, hint) = rows[0];
        assert_eq!(hint.modes, BROWSE);
        assert_eq!(hint.group, Group::Edit);
        assert_eq!(hint.help, "move note or folder");
        assert_eq!(hint.slot, Slot::Priority(9));
        assert_eq!(idx, TREE_KEYS.iter().position(|k| k.key == "N").unwrap() + 1);
        assert!(help::rows(TREE_KEYS).contains(&help::Row::Key(idx)));
        assert!(shown_keys(Mode::Normal, &[], 200).contains(&"m"));
        assert!(shown_keys(Mode::Focus, &[], 200).contains(&"m"));
        for mode in [Mode::Tag, Mode::Filter, Mode::Rename, Mode::NewItem, Mode::ConfirmDelete, Mode::VimCommand, Mode::Move, Mode::ConfirmMove] {
            assert!(!shown_keys(mode, &[], 200).contains(&"m"), "{mode:?}");
        }
        let keys_in = |mode: Mode| TREE_KEYS.iter().filter(|k| k.modes.contains(&mode)).map(|k| k.key).collect::<Vec<_>>();
        assert_eq!(keys_in(Mode::Move), vec!["enter", "esc", "tab", "bksp"]);
        assert_eq!(keys_in(Mode::ConfirmMove), vec!["y", "n/esc"]);
        let with_n = (0..200).find(|&w| shown_keys(Mode::Normal, &[], w).contains(&"N")).unwrap();
        let with_m = (0..200).find(|&w| shown_keys(Mode::Normal, &[], w).contains(&"m")).unwrap();
        assert!(with_m > with_n, "m drops before N");
    }

    // --- Move: folders and set scope ---

    /// The note `ideas/deep/deeper/n.md` two levels down a moved folder, the
    /// empty folder `ideas/deep/empty` and the hidden note
    /// `ideas/.hidden/h.md` the tree does not list, added to every store of
    /// [`move_fixture`]. The deep note is tagged 5 and the hidden one 6 in
    /// every tag root (global's own keys 9 and 10).
    fn folder_fixture() -> (tempfile::TempDir, PathBuf, Vec<(Scope, PathBuf)>) {
        let (dir, notez, roots) = move_fixture();
        for (_, root) in &roots {
            std::fs::create_dir_all(root.join("ideas/deep/deeper")).unwrap();
            std::fs::create_dir_all(root.join("ideas/deep/empty")).unwrap();
            std::fs::create_dir_all(root.join("ideas/.hidden")).unwrap();
            std::fs::write(root.join("ideas/deep/deeper/n.md"), "# deep\n").unwrap();
            std::fs::write(root.join("ideas/.hidden/h.md"), "# hidden\n").unwrap();
        }
        let append = |root: &Path, lines: &str| {
            let tags = root.join(".tags");
            let old = std::fs::read_to_string(&tags).unwrap();
            std::fs::write(&tags, format!("{old}{lines}")).unwrap();
        };
        append(
            &notez,
            "personal/proj/ideas/deep/deeper/n.md:5\npersonal/proj/ideas/.hidden/h.md:6\n\
             ideas/deep/deeper/n.md:9\nideas/.hidden/h.md:10\n",
        );
        for scope in [Scope::Public, Scope::Local] {
            append(&store_of(&roots, scope), "ideas/deep/deeper/n.md:5\nideas/.hidden/h.md:6\n");
        }
        (dir, notez, roots)
    }

    /// `Tab` in `prompt` until it targets `scope`.
    fn tab_to(prompt: &mut MovePrompt, scope: Scope, ctx: &TreeContext) {
        for _ in 0..4 {
            if prompt.scope.target.scope == scope {
                return;
            }
            let p = &prompt.scope;
            prompt.scope.target = next_scope_target(
                &p.target,
                &p.origin,
                p.project.as_deref(),
                &ctx.new_note_roots,
                ctx.current_project.as_deref(),
            );
        }
        assert_eq!(prompt.scope.target.scope, scope, "Tab reaches {scope:?}");
    }

    /// Every pair of scopes, the same one included: `ideas/` with its three
    /// listed notes (one two levels down), an empty folder and a hidden
    /// tagged note moves into `plans/`. Every note row and folder row takes
    /// its new path, section and tag root; every tagged key leaves the
    /// source tag root and arrives in the destination one with its flags;
    /// the untagged note gets no key and every other key stays.
    #[test]
    fn m_moves_a_folder_with_every_note_and_its_tags_within_and_across_scopes() {
        let files = ["a.md", "b.md", "deep/deeper/n.md", ".hidden/h.md"];
        for (from, from_name) in SCOPES {
            for (to, to_name) in SCOPES {
                let pair = format!("{from_name} to {to_name}");
                let (_dir, _notez, roots) = folder_fixture();
                let rebuild = || Ok(move_sections(&roots));
                let ctx = move_ctx(&roots);
                let mut forest = forest_of(move_sections(&roots));
                let (mut retired, mut carried) = (Vec::new(), Vec::new());
                let (src_store, dst_store) = (store_of(&roots, from), store_of(&roots, to));
                let (src_tags, dst_tags) = (move_tag_root(&roots, from), move_tag_root(&roots, to));
                let src = src_store.join("ideas");
                let dst = dst_store.join("plans/ideas");
                let initial = final_maps(&forest, &[], &[], &roots);

                let prompt = move_request(&forest.nodes, &forest.sections, Some(row(&forest.nodes, path_str(&src))), &ctx).unwrap();
                assert_eq!(prompt.scope.buffer, "", "{pair}: prefilled with the folder's parent");
                let plan = move_plan(&forest, &ctx, &src, to, "plans").unwrap();
                assert_eq!(plan.dst, dst, "{pair}");
                assert_eq!(plan.needs_confirm(), from != to, "{pair}");
                assert!(plan.question().starts_with("move ideas/ and its 3 notes to "), "{pair}: {}", plan.question());

                let outcome = apply_move(&mut forest, &mut retired, &mut carried, &plan, &ctx.new_note_roots, &rebuild);
                assert!(!src.exists(), "{pair}");
                for file in files {
                    assert!(dst.join(file).is_file(), "{pair}: {file}");
                }
                assert!(dst.join("deep/empty").is_dir(), "{pair}");
                let label = scope_label(to, (to != Scope::Global).then_some("proj"), Some("proj"));
                assert_eq!(outcome.message, format!("moved ideas/ to {label}/plans"), "{pair}");
                let at = outcome.row.expect("the moved folder is listed");
                assert_eq!(forest.nodes[at].path, dst, "{pair}: the cursor is on the moved folder");
                assert!(forest.nodes[at].is_dir);
                assert!(get_visible_nodes(&forest.nodes).contains(&at), "{pair}: its section and parent are expanded");
                assert!(!forest.nodes.iter().any(|n| n.path.starts_with(&src)), "{pair}: no row at the old place");
                for path in ["a.md", "b.md", "deep/deeper/n.md", "deep", "deep/deeper", "deep/empty"] {
                    let node = &forest.nodes[row(&forest.nodes, path_str(&dst.join(path)))];
                    assert_eq!(forest.sections[node.section].root, dst_store, "{pair}: {path} section");
                    if !node.is_dir {
                        assert_eq!(forest.tag_roots[node.tag_root], dst_tags, "{pair}: {path} tag root");
                    }
                }

                let mut expected = initial.clone();
                let key = |root: &Path, path: &Path| rel_key(root, path).unwrap();
                let mut moved = Vec::new();
                for file in ["a.md", "deep/deeper/n.md", ".hidden/h.md"] {
                    let old_key = key(&src_tags, &src.join(file));
                    let flags = expected[&src_tags].get(&old_key).copied().expect("tagged in the fixture");
                    expected.get_mut(&src_tags).unwrap().remove(&old_key);
                    moved.push((key(&dst_tags, &dst.join(file)), flags));
                }
                for (new_key, flags) in moved {
                    expected.get_mut(&dst_tags).unwrap().insert(new_key, flags);
                }
                let finals = final_maps(&forest, &retired, &carried, &roots);
                assert_eq!(finals, expected, "{pair}");
                assert!(!finals[&dst_tags].keys().any(|k| k.ends_with("plans/ideas/b.md")), "{pair}: untagged");
            }
        }
    }

    #[test]
    fn a_folder_move_into_itself_onto_a_taken_name_or_a_missing_folder_moves_nothing() {
        let (dir, _notez, roots) = folder_fixture();
        let ctx = move_ctx(&roots);
        let forest = forest_of(move_sections(&roots));
        let personal = store_of(&roots, Scope::Personal);
        let src = personal.join("ideas");
        std::fs::create_dir(personal.join("plans/ideas")).unwrap();
        let before = disk_entries(dir.path());

        let plan = |scope: Scope, folder: &str| move_plan(&forest, &ctx, &src, scope, folder).unwrap_err();
        assert_eq!(plan(Scope::Personal, "ideas"), "move: ideas/ cannot go inside itself");
        assert_eq!(plan(Scope::Personal, "ideas/deep/deeper"), "move: ideas/ cannot go inside itself");
        assert_eq!(plan(Scope::Personal, ""), "move: ideas is already in personal");
        assert_eq!(plan(Scope::Personal, "plans"), "move: ideas already exists in personal/plans");
        assert_eq!(plan(Scope::Personal, "nope"), "move: no folder personal/nope");
        assert_eq!(
            plan(Scope::Public, ""),
            "move: ideas already exists in public (committed with the project)"
        );
        assert_eq!(disk_entries(dir.path()), before, "nothing moved");
    }

    #[test]
    fn a_folder_move_that_fails_retires_nothing_and_keeps_every_note() {
        let (_dir, _notez, roots) = folder_fixture();
        let rebuild = || Ok(move_sections(&roots));
        let ctx = move_ctx(&roots);
        let mut forest = forest_of(move_sections(&roots));
        let (mut retired, mut carried) = (Vec::new(), Vec::new());
        let src = store_of(&roots, Scope::Personal).join("ideas");
        let plan = move_plan(&forest, &ctx, &src, Scope::Public, "plans").unwrap();
        std::fs::create_dir(&plan.dst).unwrap();

        let outcome = apply_move(&mut forest, &mut retired, &mut carried, &plan, &ctx.new_note_roots, &rebuild);
        assert!(outcome.message.starts_with("move failed: "), "{}", outcome.message);
        assert!(retired.is_empty() && carried.is_empty());
        assert!(src.join("deep/deeper/n.md").is_file() && src.join(".hidden/h.md").is_file());
        assert_eq!(forest.nodes[outcome.row.unwrap()].path, src, "the cursor stays on the folder");
        assert!(final_maps(&forest, &retired, &carried, &roots)
            .into_iter()
            .all(|(root, map)| map == note_tags::load_tags(&root)));
    }

    #[test]
    fn a_folder_moved_out_of_the_view_is_reported_and_carries_every_key() {
        let (_dir, notez, roots) = folder_fixture();
        let personal_only = || move_sections(&roots).into_iter().take(1).collect::<Vec<_>>();
        let rebuild = || Ok(personal_only());
        let ctx = move_ctx(&roots);
        let mut forest = forest_of(personal_only());
        let (mut retired, mut carried) = (Vec::new(), Vec::new());
        let public = store_of(&roots, Scope::Public);
        let src = store_of(&roots, Scope::Personal).join("ideas");

        let plan = move_plan(&forest, &ctx, &src, Scope::Public, "plans").unwrap();
        let outcome = apply_move(&mut forest, &mut retired, &mut carried, &plan, &ctx.new_note_roots, &rebuild);
        assert_eq!(outcome.row, None);
        assert_eq!(outcome.message, format!("moved to {}", public.join("plans/ideas").display()));
        let finals = final_maps(&forest, &retired, &carried, &roots);
        for key in ["ideas/a.md", "ideas/deep/deeper/n.md", "ideas/.hidden/h.md"] {
            assert_eq!(finals[&notez].get(&format!("personal/proj/{key}")), None, "{key}");
        }
        assert_eq!(finals[&public].get("plans/ideas/a.md"), Some(&1));
        assert_eq!(finals[&public].get("plans/ideas/deep/deeper/n.md"), Some(&5));
        assert_eq!(finals[&public].get("plans/ideas/.hidden/h.md"), Some(&6));
        assert_eq!(finals[&public].get("plans/ideas/b.md"), None);
        assert_eq!(finals[&public].get("ideas/a.md"), Some(&3), "other keys stay");
        assert_eq!(finals[&notez].get("ideas/a.md"), Some(&4), "other keys stay");
    }

    #[test]
    fn a_folder_move_whose_list_refresh_fails_lists_the_folder_at_its_new_place() {
        let (_dir, notez, roots) = folder_fixture();
        let rebuild = || -> Result<Vec<SectionSpec>> { Err(anyhow::anyhow!("listing broke")) };
        let ctx = move_ctx(&roots);
        let mut forest = forest_of(move_sections(&roots));
        let (mut retired, mut carried) = (Vec::new(), Vec::new());
        let src = store_of(&roots, Scope::Personal).join("ideas");
        let dst = notez.join("plans/ideas");
        let plan = move_plan(&forest, &ctx, &src, Scope::Global, "plans").unwrap();

        let outcome = apply_move(&mut forest, &mut retired, &mut carried, &plan, &ctx.new_note_roots, &rebuild);
        assert!(outcome.message.contains("could not be refreshed: listing broke"), "{}", outcome.message);
        assert_eq!(forest.nodes[outcome.row.expect("listed from the current rows")].path, dst);
        for path in ["a.md", "deep/deeper/n.md", "deep/empty"] {
            let node = &forest.nodes[row(&forest.nodes, path_str(&dst.join(path)))];
            assert_eq!(forest.sections[node.section].root, notez, "{path}");
        }
        assert!(!forest.nodes.iter().any(|n| n.path.starts_with(&src)), "no row at the old place");
        let finals = final_maps(&forest, &retired, &carried, &roots);
        assert_eq!(finals[&notez].get("plans/ideas/deep/deeper/n.md"), Some(&5));
        assert_eq!(finals[&notez].get("personal/proj/ideas/deep/deeper/n.md"), None);
    }

    /// A typed folder must exist under exactly that spelling and through
    /// real folders only: a case variant (which a case-insensitive file
    /// system would resolve, reaching the projects' `personal/` stores from
    /// global) and a symlinked folder at any step are refused, nothing moved.
    #[test]
    fn a_case_variant_or_a_symlinked_folder_in_the_typed_path_is_refused() {
        let (dir, notez, roots) = folder_fixture();
        let ctx = move_ctx(&roots);
        let personal = store_of(&roots, Scope::Personal);
        let outside = dir.path().join("outside");
        std::fs::create_dir_all(outside.join("sub")).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, personal.join("link")).unwrap();
        let forest = forest_of(move_sections(&roots));
        let before = disk_entries(dir.path());
        let tags_before = final_maps(&forest, &[], &[], &roots);

        let global_note = notez.join("c.md");
        let refused = move_plan(&forest, &ctx, &global_note, Scope::Global, "Personal/proj/plans").unwrap_err();
        assert_eq!(refused, "move: no folder global/Personal/proj/plans");
        let refused = move_plan(&forest, &ctx, &global_note, Scope::Global, "PERSONAL").unwrap_err();
        assert_eq!(refused, "move: no folder global/PERSONAL");
        for (scope, label) in [(Scope::Personal, "personal"), (Scope::Global, "global")] {
            let src = store_of(&roots, scope).join("c.md");
            let refused = move_plan(&forest, &ctx, &src, scope, "Plans").unwrap_err();
            assert_eq!(refused, format!("move: no folder {label}/Plans"));
            let refused = move_plan(&forest, &ctx, &src, scope, "ideas/Deep").unwrap_err();
            assert_eq!(refused, format!("move: no folder {label}/ideas/Deep"));
        }
        let folder = personal.join("ideas");
        assert_eq!(
            move_plan(&forest, &ctx, &folder, Scope::Personal, "Plans").unwrap_err(),
            "move: no folder personal/Plans"
        );
        #[cfg(unix)]
        {
            let src = personal.join("c.md");
            for typed in ["link/sub", "link"] {
                let refused = move_plan(&forest, &ctx, &src, Scope::Personal, typed).unwrap_err();
                assert_eq!(refused, format!("move: personal/{typed} is not a plain folder"));
            }
        }
        assert_eq!(disk_entries(dir.path()), before, "nothing moved");
        assert_eq!(final_maps(&forest, &[], &[], &roots), tags_before, "no tag changes");
    }

    /// A hidden tagged note the tree does not list keeps its tag when its
    /// folder moves twice: out to `plans/` and back to the root, within a
    /// scope and through another scope.
    #[test]
    fn a_hidden_note_keeps_its_tag_when_its_folder_moves_out_and_back() {
        for (to, to_name) in [(Scope::Personal, "personal"), (Scope::Public, "public")] {
            let (_dir, notez, roots) = folder_fixture();
            let rebuild = || Ok(move_sections(&roots));
            let ctx = move_ctx(&roots);
            let mut forest = forest_of(move_sections(&roots));
            let (mut retired, mut carried) = (Vec::new(), Vec::new());
            let personal = store_of(&roots, Scope::Personal);
            let away = store_of(&roots, to).join("plans/ideas");
            let away_tags = move_tag_root(&roots, to);

            let plan = move_plan(&forest, &ctx, &personal.join("ideas"), to, "plans").unwrap();
            apply_move(&mut forest, &mut retired, &mut carried, &plan, &ctx.new_note_roots, &rebuild);
            assert!(away.join(".hidden/h.md").is_file(), "{to_name}");
            let plan = move_plan(&forest, &ctx, &away, Scope::Personal, "").unwrap();
            apply_move(&mut forest, &mut retired, &mut carried, &plan, &ctx.new_note_roots, &rebuild);
            assert!(personal.join("ideas/.hidden/h.md").is_file(), "{to_name}");

            let finals = final_maps(&forest, &retired, &carried, &roots);
            assert_eq!(finals[&notez].get("personal/proj/ideas/.hidden/h.md"), Some(&6), "{to_name}");
            assert_eq!(finals[&notez].get("personal/proj/ideas/a.md"), Some(&1), "{to_name}");
            let away_key = rel_key(&away_tags, &away.join(".hidden/h.md")).unwrap();
            assert_eq!(finals[&away_tags].get(&away_key), None, "{to_name}: no key at the intermediate path");
            assert!(carried.iter().all(|(_, path, _)| path.exists()), "{to_name}: carried paths are current");
        }
    }

    /// `S`, `Tab` to each other scope `NewNoteRoots` offers, `Enter`: a note
    /// and a folder move to the same folder there, with their tags
    /// (including an edit not yet saved). Typing does not change the folder.
    #[test]
    fn s_tab_enter_moves_a_note_and_a_folder_across_every_scope_pair_keeping_the_folder() {
        for (from, from_name) in SCOPES {
            for (to, to_name) in SCOPES {
                if from == to {
                    continue;
                }
                let pair = format!("{from_name} to {to_name}");
                let (_dir, _notez, roots) = move_fixture();
                let (src_store, dst_store) = (store_of(&roots, from), store_of(&roots, to));
                std::fs::create_dir_all(src_store.join("ideas/sub")).unwrap();
                std::fs::write(src_store.join("ideas/s.md"), "# s\n").unwrap();
                std::fs::write(src_store.join("ideas/sub/x.md"), "# x\n").unwrap();
                let rebuild = || Ok(move_sections(&roots));
                let ctx = move_ctx(&roots);
                let mut forest = forest_of(move_sections(&roots));
                let (mut retired, mut carried) = (Vec::new(), Vec::new());
                let (src_tags, dst_tags) = (move_tag_root(&roots, from), move_tag_root(&roots, to));
                for (file, flags) in [("ideas/s.md", 7), ("ideas/sub/x.md", 3)] {
                    let i = row(&forest.nodes, path_str(&src_store.join(file)));
                    forest.nodes[i].flags = flags;
                }
                let initial = final_maps(&forest, &[], &[], &roots);

                for (name, is_folder) in [("s.md", false), ("sub", true)] {
                    let src = src_store.join("ideas").join(name);
                    let i = row(&forest.nodes, path_str(&src));
                    let mut prompt = set_scope_request(&forest.nodes, &forest.sections, Some(i), &ctx).unwrap();
                    assert!(prompt.fixed && prompt.scope.buffer == "ideas", "{pair}");
                    for code in [KeyCode::Char('z'), KeyCode::Backspace, KeyCode::Backspace] {
                        type_into_move(&mut prompt, code);
                    }
                    assert_eq!(prompt.scope.buffer, "ideas", "{pair}: typing does nothing");
                    assert_eq!(resolve_enter(&prompt, &ctx.new_note_roots), Ok(None), "{pair}: same scope");
                    tab_to(&mut prompt, to, &ctx);
                    let plan = resolve_enter(&prompt, &ctx.new_note_roots).unwrap().expect("a move");
                    assert!(plan.needs_confirm(), "{pair}");
                    let dst = dst_store.join("ideas").join(name);
                    assert_eq!(plan.dst, dst, "{pair}");
                    if is_folder {
                        assert!(plan.question().starts_with("move ideas/sub/ and its 1 note to "), "{}", plan.question());
                    }
                    let outcome = apply_move(&mut forest, &mut retired, &mut carried, &plan, &ctx.new_note_roots, &rebuild);
                    assert!(!src.exists() && dst.exists(), "{pair} {name}");
                    assert_eq!(forest.nodes[outcome.row.expect("listed")].path, dst, "{pair} {name}");
                }

                let mut expected = initial.clone();
                for (file, flags) in [("ideas/s.md", 7), ("ideas/sub/x.md", 3)] {
                    expected.get_mut(&src_tags).unwrap().remove(&rel_key(&src_tags, &src_store.join(file)).unwrap());
                    expected.get_mut(&dst_tags).unwrap().insert(rel_key(&dst_tags, &dst_store.join(file)).unwrap(), flags);
                }
                assert_eq!(final_maps(&forest, &retired, &carried, &roots), expected, "{pair}");
            }
        }
    }

    #[test]
    fn s_on_the_original_scope_does_nothing_and_a_missing_folder_or_taken_name_refuses() {
        let (dir, _notez, roots) = move_fixture();
        let ctx = move_ctx(&roots);
        let personal = store_of(&roots, Scope::Personal);
        std::fs::create_dir_all(personal.join("only/f")).unwrap();
        std::fs::write(personal.join("only/z.md"), "# z\n").unwrap();
        let forest = forest_of(move_sections(&roots));
        let before = disk_entries(dir.path());
        let open = |path: &Path| {
            set_scope_request(&forest.nodes, &forest.sections, Some(row(&forest.nodes, path_str(path))), &ctx).unwrap()
        };

        let mut prompt = open(&personal.join("ideas/a.md"));
        assert_eq!(text_of(&Line::from(set_scope_lead(&prompt))), " set scope of a.md: personal (Tab cycles, Enter applies)");
        assert_eq!(resolve_enter(&prompt, &ctx.new_note_roots), Ok(None));
        for _ in 0..4 {
            let p = &prompt.scope;
            prompt.scope.target = next_scope_target(&p.target, &p.origin, p.project.as_deref(), &ctx.new_note_roots, Some("proj"));
        }
        assert_eq!(resolve_enter(&prompt, &ctx.new_note_roots), Ok(None), "a full cycle is back at the start");

        let refused = |path: &Path, scope: Scope| {
            let mut prompt = open(path);
            tab_to(&mut prompt, scope, &ctx);
            resolve_enter(&prompt, &ctx.new_note_roots).unwrap_err()
        };
        let public = "public (committed with the project)";
        assert_eq!(refused(&personal.join("ideas/a.md"), Scope::Public), format!("set scope: a.md already exists in {public}/ideas"));
        assert_eq!(refused(&personal.join("ideas"), Scope::Local), "set scope: ideas already exists in local scratch");
        assert_eq!(refused(&personal.join("only/z.md"), Scope::Public), format!("set scope: no folder {public}/only"));
        assert_eq!(refused(&personal.join("only/f"), Scope::Global), "set scope: no folder global/only");
        assert_eq!(disk_entries(dir.path()), before, "nothing moved, no folder created");

        let rendered = text_of(&lead_with_hints(set_scope_lead(&prompt), Mode::SetScope, &[], 200));
        assert!(rendered.starts_with(" set scope of a.md: personal (Tab cycles"), "{rendered}");
        assert!(rendered.contains("enter apply") && rendered.contains("esc cancel") && rendered.contains("tab scope"), "{rendered}");
        assert!(!rendered.contains("bksp"), "{rendered}");
    }

    #[test]
    fn s_is_a_browse_key_after_m_with_its_prompt_keys() {
        let rows: Vec<(usize, &KeyHint)> = TREE_KEYS.iter().enumerate().filter(|(_, k)| k.key == "S").collect();
        assert_eq!(rows.len(), 1);
        let (idx, hint) = rows[0];
        assert_eq!(hint.modes, BROWSE);
        assert_eq!(hint.group, Group::Edit);
        assert_eq!(hint.help, "set scope");
        assert_eq!(hint.slot, Slot::Priority(10));
        assert_eq!(idx, TREE_KEYS.iter().position(|k| k.key == "m").unwrap() + 1);
        assert!(help::rows(TREE_KEYS).contains(&help::Row::Key(idx)));
        assert!(shown_keys(Mode::Normal, &[], 200).contains(&"S"));
        assert!(shown_keys(Mode::Focus, &[], 200).contains(&"S"));
        for mode in [Mode::Tag, Mode::Filter, Mode::Rename, Mode::NewItem, Mode::ConfirmDelete, Mode::VimCommand, Mode::Move, Mode::SetScope, Mode::ConfirmMove] {
            assert!(!shown_keys(mode, &[], 200).contains(&"S"), "{mode:?}");
            assert!(!shown_keys(mode, &[], 200).contains(&"m"), "{mode:?}");
        }
        let keys_in = |mode: Mode| TREE_KEYS.iter().filter(|k| k.modes.contains(&mode)).map(|k| k.key).collect::<Vec<_>>();
        assert_eq!(keys_in(Mode::SetScope), vec!["enter", "esc", "tab"]);
        let with_m = (0..200).find(|&w| shown_keys(Mode::Normal, &[], w).contains(&"m")).unwrap();
        let with_s = (0..200).find(|&w| shown_keys(Mode::Normal, &[], w).contains(&"S")).unwrap();
        assert!(with_s > with_m, "S drops before m");
    }

    // --- Marks ---

    fn marks_of(paths: &[&str]) -> HashSet<PathBuf> {
        paths.iter().map(PathBuf::from).collect()
    }

    fn action_paths(marks: &HashSet<PathBuf>, nodes: &[TreeNode]) -> Vec<String> {
        action_set(marks, nodes)
            .iter()
            .map(|&i| nodes[i].path.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn space_marks_and_unmarks_notes_and_folders_and_refuses_a_section() {
        let sections = project_sections();
        let (nodes, _) = build_forest(&sections);
        let mut marks = HashSet::new();
        let note = row(&nodes, "/n/personal/proj/top.md");
        let folder = row(&nodes, "/n/personal/proj/ideas");
        assert_eq!(toggle_mark(&mut marks, &nodes, Some(note)), Ok(true));
        assert_eq!(toggle_mark(&mut marks, &nodes, Some(folder)), Ok(true));
        assert_eq!(marks, marks_of(&["/n/personal/proj/top.md", "/n/personal/proj/ideas"]));
        assert_eq!(toggle_mark(&mut marks, &nodes, Some(note)), Ok(true));
        assert_eq!(marks, marks_of(&["/n/personal/proj/ideas"]), "a second Space unmarks");
        for section in ["/n/personal/proj", "/p/docs", "/p/.notez", "/n"] {
            let refused = toggle_mark(&mut marks, &nodes, Some(row(&nodes, section)));
            assert_eq!(refused, Err(SECTION_MARK), "{section}");
        }
        assert_eq!(toggle_mark(&mut marks, &nodes, None), Ok(false));
        assert_eq!(toggle_mark(&mut marks, &nodes, Some(nodes.len())), Ok(false));
        assert_eq!(toggle_mark(&mut HashSet::new(), &[], Some(0)), Ok(false));
        assert_eq!(marks, marks_of(&["/n/personal/proj/ideas"]), "refusals change nothing");
    }

    #[test]
    fn space_moves_the_cursor_down_one_row_and_stays_on_a_refusal_or_the_last_row() {
        let sections = vec![spec("/r", "S", &["ideas/a.md", "z.md"])];
        let (mut nodes, _) = build_forest(&sections);
        for node in &mut nodes {
            node.expanded = true;
        }
        let visible = compute_visible(&nodes, "");
        let mut marks = HashSet::new();
        let mut state = ListState::default();
        let at = |nodes: &[TreeNode], path: &str| visible.iter().position(|&i| i == row(nodes, path));

        state.select(at(&nodes, "/r"));
        assert_eq!(press_space(&mut marks, &mut nodes, &mut state, &visible, false), Some(SECTION_MARK));
        assert_eq!(state.selected(), at(&nodes, "/r"), "a refusal does not move the cursor");

        state.select(at(&nodes, "/r/ideas"));
        assert_eq!(press_space(&mut marks, &mut nodes, &mut state, &visible, false), None);
        assert_eq!(state.selected(), at(&nodes, "/r/ideas/a.md"));
        assert_eq!(press_space(&mut marks, &mut nodes, &mut state, &visible, false), None);
        assert_eq!(state.selected(), at(&nodes, "/r/z.md"));
        assert_eq!(press_space(&mut marks, &mut nodes, &mut state, &visible, false), None);
        assert_eq!(state.selected(), at(&nodes, "/r/z.md"), "the last row keeps the cursor");
        assert_eq!(marks, marks_of(&["/r/ideas", "/r/ideas/a.md", "/r/z.md"]));
    }

    #[test]
    fn esc_clears_the_marks_first_then_the_filter_then_does_nothing() {
        let mut marks = marks_of(&["/r/a.md", "/r/b.md"]);
        let mut search = "a".to_string();
        browse_escape(&mut marks, &mut search);
        assert!(marks.is_empty());
        assert_eq!(search, "a", "with marks, Esc clears only the marks");
        browse_escape(&mut marks, &mut search);
        assert!(search.is_empty(), "then the filter");
        browse_escape(&mut marks, &mut search);
        assert!(marks.is_empty() && search.is_empty(), "then Esc does nothing and never quits");
    }

    #[test]
    fn marks_survive_filter_collapse_and_a_rebuild_and_go_with_their_row() {
        let files = ["ideas/a.md", "ideas/b.md", "z.md"];
        let (nodes, tag_roots) = build_forest(&[spec("/r", "S", &files)]);
        let initial = vec![HashMap::new(); tag_roots.len()];
        let mut forest = Forest { sections: vec![spec("/r", "S", &files)], nodes, tag_roots, initial };
        let mut marks = marks_of(&["/r/ideas", "/r/ideas/a.md", "/r/ideas/b.md", "/r/z.md"]);

        let ideas = row(&forest.nodes, "/r/ideas");
        forest.nodes[ideas].expanded = false;
        assert!(!compute_visible(&forest.nodes, "z").contains(&row(&forest.nodes, "/r/ideas/a.md")));
        prune_marks(&mut marks, &forest.nodes);
        assert_eq!(marks.len(), 4, "collapsed and filtered rows keep their marks");

        forest.rebuild(vec![spec("/r", "S", &["ideas/a.md", "ideas/b.md"])], Path::new(""));
        prune_marks(&mut marks, &forest.nodes);
        assert_eq!(marks, marks_of(&["/r/ideas", "/r/ideas/a.md", "/r/ideas/b.md"]), "a row the rebuild drops loses its mark");

        let b = row(&forest.nodes, "/r/ideas/b.md");
        forest.nodes[b].path = PathBuf::from("/r/ideas/renamed.md");
        prune_marks(&mut marks, &forest.nodes);
        assert_eq!(marks, marks_of(&["/r/ideas", "/r/ideas/a.md"]), "a renamed row loses its mark");

        marks.insert(PathBuf::from("/r"));
        prune_marks(&mut marks, &forest.nodes);
        assert!(!marks.contains(Path::new("/r")), "a section row is never marked");
    }

    #[test]
    fn the_action_set_drops_every_row_under_a_marked_folder() {
        let files = ["ideas/a.md", "ideas/deep/m.md", "ideas/deep/n.md", "plans/p.md", "z.md"];
        let (nodes, _) = build_forest(&[spec("/r", "S", &files), spec("/q", "Q", &["ideas/a.md"])]);
        assert!(nodes.iter().all(|n| !n.expanded), "every folder is collapsed");

        let folder_and_note = marks_of(&["/r/ideas", "/r/ideas/a.md"]);
        assert_eq!(action_paths(&folder_and_note, &nodes), vec!["/r/ideas"]);

        let nested = marks_of(&["/r/ideas/deep", "/r/ideas", "/r/ideas/deep/n.md"]);
        assert_eq!(action_paths(&nested, &nodes), vec!["/r/ideas"]);

        let disjoint = marks_of(&["/q/ideas/a.md", "/r/z.md", "/r/plans", "/r/ideas/deep/n.md"]);
        assert_eq!(
            action_paths(&disjoint, &nodes),
            vec!["/r/ideas/deep/n.md", "/r/plans", "/r/z.md", "/q/ideas/a.md"],
            "tree order"
        );

        let unlisted = marks_of(&["/r/gone.md", "/r/z.md", "/r"]);
        assert_eq!(action_paths(&unlisted, &nodes), vec!["/r/z.md"], "unlisted paths and sections drop");
        assert!(action_set(&HashSet::new(), &nodes).is_empty());
    }

    fn bulk_item(rel: &str, folder_notes: Option<usize>, scope: Scope, label: &str) -> BulkItem {
        BulkItem {
            path: PathBuf::from("/x").join(rel),
            rel: rel.to_string(),
            is_dir: folder_notes.is_some(),
            scope,
            label: label.to_string(),
            notes_inside: folder_notes.unwrap_or(0),
            has_other_files: false,
            prompt: Err("unused"),
        }
    }

    #[test]
    fn the_bulk_question_counts_notes_and_folders_and_names_each_scope_once() {
        let note = |rel, scope, label| bulk_item(rel, None, scope, label);
        let folder = |rel, notes, scope, label| bulk_item(rel, Some(notes), scope, label);
        let public = "public (committed with the project)";

        let notes_only = [note("a.md", Scope::Personal, "personal"), note("b.md", Scope::Personal, "personal")];
        assert_eq!(bulk_delete_question(&notes_only), "delete 2 notes from personal? y/n");
        assert_eq!(bulk_delete_question(&[note("e.md", Scope::Global, "global")]), "delete 1 note from global? y/n");

        let folders_only = [folder("ideas", 2, Scope::Personal, "personal"), folder("plans", 0, Scope::Global, "global")];
        assert_eq!(bulk_delete_question(&folders_only), "delete 2 folders (2 notes inside) from personal, global? y/n");
        assert_eq!(
            bulk_delete_question(&[folder("empty", 0, Scope::Personal, "personal")]),
            "delete 1 folder (no notes inside) from personal? y/n"
        );
        assert_eq!(
            bulk_delete_question(&[folder("one", 1, Scope::Public, public)]),
            format!("delete 1 folder (1 note inside) from {public}? y/n")
        );

        let mixed = [
            note("c.md", Scope::Public, public),
            folder("ideas", 3, Scope::Personal, "personal"),
            note("d.md", Scope::Personal, "personal"),
        ];
        assert_eq!(
            bulk_delete_question(&mixed),
            format!("delete 2 notes and 1 folder (3 notes inside) from {public}, personal? y/n")
        );

        let local = [note("a.md", Scope::Personal, "personal"), folder("x", 1, Scope::Local, "local scratch")];
        assert_eq!(
            bulk_delete_question(&local),
            "delete 1 note and 1 folder (1 note inside) from personal, local scratch? (not recoverable) y/n"
        );
        for set in [&notes_only[..], &folders_only[..], &mixed[..]] {
            assert!(!bulk_delete_question(set).contains("not recoverable"));
            assert!(!bulk_delete_question(set).contains("other files"));
        }
    }

    #[test]
    fn the_bulk_question_says_other_files_when_a_folder_holds_more_than_notes() {
        let mut odd = bulk_item("assets", Some(1), Scope::Personal, "personal");
        odd.has_other_files = true;
        let set = [bulk_item("a.md", None, Scope::Personal, "personal"), bulk_item("ideas", Some(2), Scope::Personal, "personal"), odd];
        assert_eq!(
            bulk_delete_question(&set),
            "delete 1 note and 2 folders (3 notes inside) and other files from personal? y/n"
        );

        let (_dir, roots) = temp_tree();
        let personal = roots[0].1.clone();
        std::fs::write(personal.join("ideas/picture.png"), "png").unwrap();
        let forest = forest_of(list_sections_with_dirs(&roots));
        let items = bulk_at(&forest, &[personal.join("ideas"), personal.join("c.md")]);
        assert!(bulk_delete_question(&items).contains("(2 notes inside) and other files from"), "{}", bulk_delete_question(&items));
    }

    /// Mark `paths` and open the confirm as `d` does with marks present.
    fn bulk_at(forest: &Forest, paths: &[PathBuf]) -> Vec<BulkItem> {
        let marks: HashSet<PathBuf> = paths.iter().cloned().collect();
        bulk_delete_request(&forest.nodes, &forest.sections, &marks, Some("proj")).expect("no guard refuses the set")
    }

    #[test]
    fn bulk_delete_across_two_scopes_removes_exactly_the_set_and_retires_its_keys() {
        let (_dir, roots) = temp_tree();
        let personal = roots[0].1.clone();
        let public = roots[1].1.clone();
        std::fs::write(personal.join(".tags"), "ideas/a.md:1\nideas/b.md:2\nc.md:4\n").unwrap();
        std::fs::write(public.join(".tags"), "c.md:1\nideas/a.md:2\n").unwrap();
        let rebuild = || Ok(list_sections_with_dirs(&roots));
        let mut forest = forest_of(list_sections_with_dirs(&roots));
        for node in &mut forest.nodes {
            node.expanded = true;
        }
        let mut retired = Vec::new();
        let before = all_files(&roots);
        let gone = [personal.join("ideas/a.md"), personal.join("ideas/b.md"), public.join("c.md")];

        let items = bulk_at(&forest, &[personal.join("ideas"), personal.join("ideas/a.md"), public.join("c.md")]);
        assert_eq!(
            bulk_delete_question(&items),
            "delete 1 note and 1 folder (2 notes inside) from personal, public (committed with the project)? y/n"
        );
        let outcome = answer_bulk_delete(KeyCode::Char('y'), &mut forest, &mut retired, &items, "", &rebuild)
            .expect("y confirms");

        let expected: Vec<PathBuf> = before.into_iter().filter(|p| !gone.contains(p)).collect();
        assert_eq!(all_files(&roots), expected, "exactly the set went");
        assert!(!personal.join("ideas").exists());
        assert_eq!(outcome.message, "deleted 2");
        assert!(!forest.nodes.iter().any(|n| n.path.starts_with(personal.join("ideas")) || n.path == public.join("c.md")));
        assert_eq!(
            outcome.row,
            Some(row(&forest.nodes, path_str(&public.join("ideas")))),
            "the last deleted row had no next sibling, so the one before it"
        );

        let changed = changed_tag_maps_retiring(&forest.nodes, &forest.tag_roots, &forest.initial, &retired);
        let mut personal_map = note_tags::load_tags(&personal);
        personal_map.retain(|k, _| k == "c.md");
        let mut public_map = note_tags::load_tags(&public);
        public_map.retain(|k, _| k == "ideas/a.md");
        assert_eq!(changed.len(), 2, "{changed:?}");
        assert!(changed.contains(&(personal.clone(), personal_map)), "{changed:?}");
        assert!(changed.contains(&(public.clone(), public_map)), "{changed:?}");
    }

    #[test]
    fn any_answer_but_y_to_the_bulk_question_deletes_nothing() {
        let (_dir, roots) = temp_tree();
        let rebuild = || Ok(list_sections_with_dirs(&roots));
        let mut forest = forest_of(list_sections_with_dirs(&roots));
        let mut retired = Vec::new();
        let items = bulk_at(&forest, &[roots[0].1.join("ideas"), roots[1].1.join("c.md")]);
        let before = all_files(&roots);
        for code in [KeyCode::Char('n'), KeyCode::Esc, KeyCode::Char(' '), KeyCode::Enter, KeyCode::Char('Y')] {
            assert!(answer_bulk_delete(code, &mut forest, &mut retired, &items, "", &rebuild).is_none(), "{code:?}");
        }
        assert_eq!(all_files(&roots), before);
        assert!(retired.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn a_failing_item_is_reported_and_the_rest_of_the_set_is_deleted() {
        use std::os::unix::fs::PermissionsExt;
        let (_dir, roots) = temp_tree();
        let personal = roots[0].1.clone();
        let global = roots[3].1.clone();
        std::fs::write(personal.join(".tags"), "ideas/a.md:1\nc.md:2\n").unwrap();
        let rebuild = || Ok(list_sections_with_dirs(&roots));
        let mut forest = forest_of(list_sections_with_dirs(&roots));
        let mut retired = Vec::new();
        let locked = personal.join("ideas");
        let items = bulk_at(&forest, &[global.join("c.md"), locked.clone(), personal.join("c.md")]);

        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o555)).unwrap();
        let outcome = answer_bulk_delete(KeyCode::Char('y'), &mut forest, &mut retired, &items, "", &rebuild);
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
        let outcome = outcome.unwrap();

        assert!(outcome.message.starts_with("deleted 2, failed 1: ideas/ ("), "{}", outcome.message);
        assert!(locked.join("a.md").is_file() && locked.join("b.md").is_file(), "the locked folder is left whole");
        assert!(!personal.join("c.md").exists() && !global.join("c.md").exists(), "the rest went");
        assert!(forest.nodes.iter().any(|n| n.path == locked.join("a.md")), "the tree shows what is left");
        assert!(!retired.iter().any(|(_, k)| k.starts_with("ideas/")), "{retired:?}");
        assert!(retired.contains(&(personal.clone(), "c.md".to_string())), "{retired:?}");
        assert!(retired.contains(&(global.clone(), "c.md".to_string())), "{retired:?}");
    }

    #[test]
    fn the_guards_hold_per_item_in_a_bulk_delete() {
        let (_dir, notez, root) = vault();
        let sections = || {
            let mut sections = vault_sections(&notez);
            sections[1].dirs = vec![notez.join("personal")];
            sections
        };
        let rebuild = || Ok(sections());
        let mut forest = forest_of(sections());
        let mut retired = Vec::new();
        let holder = notez.join("personal");
        assert!(forest.nodes.iter().any(|n| n.path == holder && n.depth > 0), "the global section lists the folder");
        let untouched = disk_entries(&notez);
        let disk: Vec<String> = disk_entries(&notez).into_iter().filter(|e| e != "personal/proj/top.md").collect();

        // The guard refuses the whole set before anything is asked.
        let marks: HashSet<PathBuf> = [root.join("top.md"), holder.clone()].into_iter().collect();
        assert_eq!(
            bulk_delete_request(&forest.nodes, &forest.sections, &marks, Some("proj")).unwrap_err(),
            format!("personal/: {FOLDER_HOLDS_SECTION}")
        );
        assert_eq!(disk_entries(&notez), untouched, "a refused set deletes nothing");

        // The guard runs again per item when the set is deleted.
        let items = bulk_items(&forest.nodes, &forest.sections, &marks, Some("proj"));
        let outcome = answer_bulk_delete(KeyCode::Char('y'), &mut forest, &mut retired, &items, "", &rebuild).unwrap();

        assert_eq!(outcome.message, format!("deleted 1, failed 1: personal/ ({FOLDER_HOLDS_SECTION})"));
        assert_eq!(disk_entries(&notez), disk, "only the note went");
    }

    #[test]
    fn space_is_a_browse_key_whose_footer_hint_drops_after_m_and_before_n() {
        let rows: Vec<&KeyHint> = TREE_KEYS.iter().filter(|k| k.key == "space").collect();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].modes, BROWSE);
        assert_eq!(rows[0].group, Group::Edit);
        assert_eq!(rows[0].help, "mark");
        let first_width = |key: &str| (0..200).find(|&w| shown_keys(Mode::Normal, &[], w).contains(&key)).unwrap();
        assert!(first_width("m") > first_width("space"), "m drops before space");
        assert!(first_width("space") > first_width("N"), "space drops before N");
        for mode in [Mode::Tag, Mode::Filter, Mode::Rename, Mode::NewItem, Mode::ConfirmDelete, Mode::VimCommand, Mode::Move, Mode::SetScope, Mode::ConfirmMove] {
            assert!(!TREE_KEYS.iter().any(|k| k.key == "space" && k.modes.contains(&mode)), "{mode:?}");
        }
        let esc = TREE_KEYS.iter().find(|k| k.key == "esc" && k.modes == BROWSE).unwrap();
        assert!(esc.help.starts_with("clear marks"), "{}", esc.help);
    }

    #[test]
    fn the_footer_leads_with_the_mark_count_then_the_hints() {
        let rendered = text_of(&lead_with_hints(marked_lead(3), Mode::Normal, &[], 200));
        assert!(rendered.starts_with(" 3 marked  open  tags"), "{rendered}");
        assert!(rendered.contains("space mark") && rendered.trim_end().ends_with("quit"), "{rendered}");
        let filtering = text_of(&lead_with_hints(marked_lead(1), Mode::Filter, &[], 200));
        assert!(filtering.starts_with(" 1 marked  enter keep"), "{filtering}");
    }

    /// The gutter: the row's first column, before the tag dots.
    const MARK_COL: usize = 0;

    #[test]
    fn a_marked_row_shows_the_mark_in_the_gutter_and_keeps_every_other_column() {
        let (sections, nodes) = badged_forest();
        for path in ["/p/.notez/d.md", "/n/personal/proj/ideas"] {
            let plain = render_row(line_of(&sections, &nodes, row(&nodes, path)));
            let line = mark_row(line_of(&sections, &nodes, row(&nodes, path)));
            let marked = render_row(line.clone());
            assert_eq!(plain[MARK_COL].0, " ", "{path}");
            assert_eq!(marked[MARK_COL].0, MARK_GLYPH, "{path}");
            for x in (0..80).filter(|&x| x != MARK_COL) {
                assert_eq!(marked[x].0, plain[x].0, "{path} column {x}");
            }
            assert!(line.spans.iter().all(|s| s.style.add_modifier.contains(Modifier::BOLD)), "{path}");
        }
        assert_eq!(mouse_x_to_row_tag(MARK_COL as u16, 0, 0b1_1111), None, "the mark is no tag dot");
    }

    // --- Marks: bulk move and set scope ---

    /// Mark `paths` and press `m` (`fixed` false) or `S` (`fixed` true).
    fn bulk_move_at(
        forest: &Forest,
        ctx: &TreeContext,
        paths: &[PathBuf],
        fixed: bool,
    ) -> std::result::Result<(MovePrompt, Vec<MovePrompt>), String> {
        let marks: HashSet<PathBuf> = paths.iter().cloned().collect();
        bulk_move_request(&forest.nodes, &forest.sections, &marks, ctx, fixed)
    }

    /// `m` on the marked `paths`, `Tab` to `scope`, `folder` typed, `Enter`.
    fn bulk_move_plan(
        forest: &Forest,
        ctx: &TreeContext,
        paths: &[PathBuf],
        scope: Scope,
        folder: &str,
    ) -> std::result::Result<Option<BulkMovePlan>, String> {
        let (mut prompt, items) = bulk_move_at(forest, ctx, paths, false)?;
        tab_to(&mut prompt, scope, ctx);
        prompt.scope.buffer = folder.to_string();
        resolve_bulk_move(&prompt, &items, &ctx.new_note_roots)
    }

    #[test]
    fn bulk_move_across_scopes_lands_each_item_under_its_own_name_and_the_tags_follow() {
        let (_dir, notez, roots) = move_fixture();
        let rebuild = || Ok(move_sections(&roots));
        let ctx = move_ctx(&roots);
        let mut forest = forest_of(move_sections(&roots));
        let (mut retired, mut carried) = (Vec::new(), Vec::new());
        let (personal, public, local) =
            (store_of(&roots, Scope::Personal), store_of(&roots, Scope::Public), store_of(&roots, Scope::Local));
        let marked = [local.join("ideas"), notez.join("c.md"), personal.join("ideas/a.md")];
        let initial = final_maps(&forest, &[], &[], &roots);

        let (prompt, items) = bulk_move_at(&forest, &ctx, &marked, false).unwrap();
        assert_eq!(items.iter().map(|i| i.src.clone()).collect::<Vec<_>>(), vec![marked[2].clone(), marked[0].clone(), marked[1].clone()], "tree order");
        assert_eq!(text_of(&Line::from(bulk_move_lead(&prompt, items.len()))), " move 3 items to personal/ideas_");
        let rendered = text_of(&lead_with_hints(bulk_move_lead(&prompt, items.len()), Mode::Move, &[], 200));
        assert!(rendered.contains("enter move") && rendered.contains("tab scope"), "{rendered}");

        let plan = bulk_move_plan(&forest, &ctx, &marked, Scope::Public, "plans").unwrap().expect("a move");
        let dst = public.join("plans");
        assert_eq!(
            plan.plans.iter().map(|p| p.dst.clone()).collect::<Vec<_>>(),
            vec![dst.join("a.md"), dst.join("ideas"), dst.join("c.md")]
        );
        assert!(plan.needs_confirm());
        assert_eq!(
            plan.question(),
            "move 3 items to public (committed with the project)/plans? \
             (it will be in the p repository, public, not yet committed) \
             (it leaves the vault; the deletion syncs on exit) y/n",
            "each clause once"
        );
        let rendered = text_of(&lead_with_hints(bulk_move_confirm_lead(&plan), Mode::ConfirmMove, &[], 400));
        assert!(rendered.starts_with(&format!(" {}", plan.question())) && rendered.contains("n/esc cancel"), "{rendered}");

        let outcome = apply_bulk_move(&mut forest, &mut retired, &mut carried, &plan.plans, &ctx.new_note_roots, &rebuild);
        assert_eq!(outcome.message, "moved 3");
        for file in ["a.md", "c.md", "ideas/a.md", "ideas/b.md"] {
            assert!(dst.join(file).is_file(), "{file}");
        }
        for src in &marked {
            assert!(!src.exists(), "{}", src.display());
        }
        assert_eq!(forest.nodes[outcome.row.expect("listed")].path, dst.join("a.md"), "the cursor is on the first moved item");
        assert!(carried.is_empty());

        let notez_tags = move_tag_root(&roots, Scope::Global);
        let mut expected = initial.clone();
        for key in ["personal/proj/ideas/a.md", "c.md"] {
            expected.get_mut(&notez_tags).unwrap().remove(key);
        }
        expected.get_mut(&local).unwrap().remove("ideas/a.md");
        let public_map = expected.get_mut(&public).unwrap();
        public_map.insert("plans/a.md".to_string(), 1);
        public_map.insert("plans/c.md".to_string(), 8);
        public_map.insert("plans/ideas/a.md".to_string(), 3);
        assert_eq!(final_maps(&forest, &retired, &carried, &roots), expected);
    }

    #[test]
    fn a_bulk_move_is_refused_whole_before_anything_moves() {
        let (dir, notez, roots) = move_fixture();
        let ctx = move_ctx(&roots);
        let forest = forest_of(move_sections(&roots));
        let (personal, public, local) =
            (store_of(&roots, Scope::Personal), store_of(&roots, Scope::Public), store_of(&roots, Scope::Local));
        let before = disk_entries(dir.path());
        let refused = |paths: &[PathBuf], scope: Scope, folder: &str| {
            bulk_move_plan(&forest, &ctx, paths, scope, folder).unwrap_err()
        };

        assert_eq!(
            refused(&[public.join("c.md"), notez.join("c.md")], Scope::Personal, "plans"),
            "move: two marked items are named c.md"
        );
        assert_eq!(
            refused(&[personal.join("c.md"), public.join("ideas/a.md")], Scope::Personal, "ideas"),
            "move: personal/ideas/a.md already exists",
            "the first collision in tree order is named"
        );
        assert_eq!(
            refused(&[public.join("c.md"), local.join("ideas")], Scope::Local, "ideas"),
            "move: local scratch/ideas/c.md is inside the marked folder ideas/"
        );
        assert_eq!(refused(&[local.join("ideas")], Scope::Local, "ideas/"), "move: ideas/ cannot go inside itself");
        assert_eq!(refused(&[personal.join("c.md")], Scope::Personal, "nope"), "move: no folder personal/nope");
        assert_eq!(
            refused(&[personal.join("ideas/a.md"), personal.join("ideas/b.md")], Scope::Personal, "ideas"),
            "move: the marked items are already in personal/ideas"
        );

        let docs = public.parent().unwrap().join("docs/d.md");
        let opened = bulk_move_at(&forest, &ctx, &[personal.join("c.md"), docs.clone()], false).err();
        assert_eq!(opened.as_deref(), Some(format!("d.md: {DOCS_MOVE}").as_str()));
        let opened = bulk_move_at(&forest, &ctx, &[docs], true).err();
        assert_eq!(opened.as_deref(), Some(format!("d.md: {DOCS_SET_SCOPE}").as_str()));

        let mut sections = move_sections(&roots);
        sections[3].project = Some("other".to_string());
        let other = forest_of(sections);
        let opened = bulk_move_at(&other, &ctx, &[personal.join("c.md"), local.join("c.md")], false).err();
        assert_eq!(opened.as_deref(), Some("move: marked items span projects"));
        let opened = bulk_move_at(&other, &ctx, &[personal.join("c.md"), local.join("c.md")], true).err();
        assert_eq!(opened.as_deref(), Some("set scope: marked items span projects"));

        assert_eq!(disk_entries(dir.path()), before, "nothing moved");
    }

    #[test]
    fn a_failing_item_in_a_bulk_move_is_reported_and_the_rest_move() {
        let (_dir, notez, roots) = move_fixture();
        let rebuild = || Ok(move_sections(&roots));
        let ctx = move_ctx(&roots);
        let mut forest = forest_of(move_sections(&roots));
        let (mut retired, mut carried) = (Vec::new(), Vec::new());
        let (personal, public) = (store_of(&roots, Scope::Personal), store_of(&roots, Scope::Public));
        let marked = [personal.join("ideas/a.md"), public.join("c.md")];
        let plan = bulk_move_plan(&forest, &ctx, &marked, Scope::Personal, "plans").unwrap().unwrap();
        std::fs::write(personal.join("plans/a.md"), "appeared meanwhile").unwrap();

        let outcome = apply_bulk_move(&mut forest, &mut retired, &mut carried, &plan.plans, &ctx.new_note_roots, &rebuild);
        assert!(outcome.message.starts_with("moved 1, failed 1: ideas/a.md ("), "{}", outcome.message);
        assert!(outcome.message.contains("already exists"), "{}", outcome.message);
        assert_eq!(std::fs::read_to_string(personal.join("plans/a.md")).unwrap(), "appeared meanwhile");
        assert!(marked[0].is_file(), "the failing item stays");
        assert!(!marked[1].exists() && personal.join("plans/c.md").is_file(), "the rest moved");
        assert_eq!(forest.nodes[outcome.row.unwrap()].path, personal.join("plans/c.md"), "the first moved item");
        assert!(forest.nodes.iter().any(|n| n.path == marked[0]), "the failing item is still listed");

        let maps = final_maps(&forest, &retired, &carried, &roots);
        assert_eq!(maps[&notez].get("personal/proj/ideas/a.md"), Some(&1), "the failing item keeps its tags");
        assert_eq!(maps[&notez].get("personal/proj/plans/c.md"), Some(&2), "the moved item's tags follow");
        assert_eq!(maps[&public].get("c.md"), None);
    }

    #[test]
    fn bulk_set_scope_keeps_each_items_folder_and_lists_each_transition_once() {
        let (dir, _notez, roots) = move_fixture();
        let (personal, public, local) =
            (store_of(&roots, Scope::Personal), store_of(&roots, Scope::Public), store_of(&roots, Scope::Local));
        std::fs::write(personal.join("ideas/s.md"), "# s\n").unwrap();
        std::fs::write(public.join("plans/p.md"), "# p\n").unwrap();
        std::fs::write(local.join("ideas/l.md"), "# l\n").unwrap();
        std::fs::create_dir_all(personal.join("only")).unwrap();
        std::fs::write(personal.join("only/z.md"), "# z\n").unwrap();
        let rebuild = || Ok(move_sections(&roots));
        let ctx = move_ctx(&roots);
        let mut forest = forest_of(move_sections(&roots));
        let (mut retired, mut carried) = (Vec::new(), Vec::new());
        for (path, flags) in [(personal.join("ideas/s.md"), 7), (public.join("plans/p.md"), 3)] {
            let i = row(&forest.nodes, path_str(&path));
            forest.nodes[i].flags = flags;
        }
        let initial = final_maps(&forest, &[], &[], &roots);
        let enter = |paths: &[PathBuf], scope: Scope| {
            let (mut prompt, items) = bulk_move_at(&forest, &ctx, paths, true).unwrap();
            for code in [KeyCode::Char('z'), KeyCode::Backspace] {
                type_into_move(&mut prompt, code);
            }
            tab_to(&mut prompt, scope, &ctx);
            resolve_bulk_move(&prompt, &items, &ctx.new_note_roots)
        };

        let before = disk_entries(dir.path());
        let all_personal = [personal.join("ideas/a.md"), personal.join("c.md")];
        assert_eq!(enter(&all_personal, Scope::Personal), Ok(None), "Enter on the scope every item is in");
        assert_eq!(
            enter(&[personal.join("only/z.md"), personal.join("c.md")], Scope::Public).unwrap_err(),
            "set scope: no folder public (committed with the project)/only"
        );
        assert_eq!(
            enter(&[personal.join("ideas/s.md"), personal.join("ideas/a.md")], Scope::Local).unwrap_err(),
            "set scope: local scratch/ideas/a.md already exists"
        );
        assert_eq!(disk_entries(dir.path()), before, "nothing moved");

        let marked = [personal.join("ideas/s.md"), public.join("plans/p.md"), local.join("ideas/l.md")];
        let (prompt, items) = bulk_move_at(&forest, &ctx, &marked, true).unwrap();
        assert_eq!(
            text_of(&Line::from(bulk_move_lead(&prompt, items.len()))),
            " set scope of 3 items: personal (Tab cycles, Enter applies)"
        );
        let plan = enter(&marked, Scope::Local).unwrap().expect("a move");
        assert_eq!(
            plan.plans.iter().map(|p| p.dst.clone()).collect::<Vec<_>>(),
            vec![local.join("ideas/s.md"), local.join("plans/p.md")],
            "each keeps its folder; the item already in local scratch stays"
        );
        assert_eq!(
            plan.question(),
            "move 2 items to local scratch? (it stays in the repository's git history) \
             (scratch is not synced and not recoverable) \
             (it leaves the vault; the deletion syncs on exit) y/n"
        );

        let outcome = apply_bulk_move(&mut forest, &mut retired, &mut carried, &plan.plans, &ctx.new_note_roots, &rebuild);
        assert_eq!(outcome.message, "moved 2");
        assert!(local.join("ideas/s.md").is_file() && local.join("plans/p.md").is_file() && local.join("ideas/l.md").is_file());
        assert!(!marked[0].exists() && !marked[1].exists());
        assert_eq!(forest.nodes[outcome.row.unwrap()].path, local.join("ideas/s.md"));

        let notez_tags = move_tag_root(&roots, Scope::Global);
        let mut expected = initial.clone();
        expected.get_mut(&notez_tags).unwrap().remove("personal/proj/ideas/s.md");
        expected.get_mut(&public).unwrap().remove("plans/p.md");
        expected.get_mut(&local).unwrap().insert("ideas/s.md".to_string(), 7);
        expected.get_mut(&local).unwrap().insert("plans/p.md".to_string(), 3);
        assert_eq!(final_maps(&forest, &retired, &carried, &roots), expected);
    }

    // --- Refused names (NZ-20) ---

    /// Typed names sanitizing would drop characters from, each with what it
    /// becomes.
    const ALTERED: [(&str, &str); 2] = [("00_quick", "00quick"), ("a.b", "ab")];

    /// Typed names the prompts take as they are.
    const ACCEPTED: [&str; 3] = ["00-quick", "my-note", "ideas"];

    /// Typed names the prompts accept although sanitizing lowercases them or
    /// turns their blanks into `-` (NZ-26), each with what it becomes.
    const SOFTENED: [(&str, &str); 2] = [("My Note", "my-note"), ("Ideas", "ideas")];

    #[test]
    fn soft_name_lowercases_and_hyphenates_without_dropping_anything() {
        assert_eq!(soft_name("My Note"), "my-note");
        assert_eq!(soft_name("  My \t  Big\nNote  "), "my-big-note");
        assert_eq!(soft_name("00_quick"), "00_quick");
        assert_eq!(soft_name("a.b"), "a.b");
        assert_eq!(soft_name("\u{c4}"), "\u{e4}");
        assert_eq!(soft_name("   "), "");
        for typed in ["00_quick", "a.b", "My Note", "Ideas", "!!!", "\u{c4}", "a\u{308}", "  x  y "] {
            let soft = soft_name(typed);
            let filtered: String = soft.chars().filter(|c| c.is_alphanumeric() || *c == '-').collect();
            assert_eq!(filtered, sanitize::name(typed), "soft form then filter is sanitize::name: {typed}");
        }
    }

    #[test]
    fn name_would_change_names_what_sanitizing_makes_of_an_altered_name() {
        for (typed, cleaned) in ALTERED {
            assert_eq!(name_would_change(typed).as_deref(), Some(cleaned), "{typed}");
        }
        for (typed, _) in SOFTENED {
            assert_eq!(name_would_change(typed), None, "{typed}: lowercasing and hyphens are silent");
        }
        assert_eq!(name_would_change("\u{c4}"), None, "capital A umlaut lowercases silently");
        // A decomposed umlaut loses its combining mark, as before NZ-26.
        assert_eq!(name_would_change("a\u{308}").as_deref(), Some("a"), "decomposed a umlaut");
        for typed in ACCEPTED {
            assert_eq!(name_would_change(typed), None, "{typed}");
        }
        assert_eq!(name_would_change("  ideas  "), None, "surrounding blanks are trimmed");
        assert_eq!(name_would_change("\u{e5}\u{e4}\u{f6}"), None, "lowercase letters beyond ASCII");
        assert_eq!(name_would_change(""), None, "an empty name is the empty-name path's");
        assert_eq!(name_would_change("!!!"), None, "so is one that sanitizes to nothing");
        assert_eq!(
            altered_name_message("00quick"),
            "name would become 00quick; use letters, digits and -"
        );
    }

    #[test]
    fn new_note_and_new_folder_enter_refuses_altered_names_and_keeps_the_rest() {
        let (_dir, notez, root) = vault();
        let target = NewNoteTarget { dir: root.clone(), scope: Scope::Personal, label: "personal".to_string() };
        for is_folder in [false, true] {
            let mut prompt = NewNotePrompt::open(target.clone(), Some("proj".to_string()));
            prompt.is_folder = is_folder;
            for (typed, cleaned) in ALTERED {
                prompt.buffer = typed.to_string();
                assert_eq!(new_item_refusal(&prompt), Some(altered_name_message(cleaned)), "{typed}");
            }
            let softened = SOFTENED.map(|(typed, _)| typed);
            for typed in ACCEPTED.into_iter().chain(softened).chain(["", "  ideas  "]) {
                prompt.buffer = typed.to_string();
                assert_eq!(new_item_refusal(&prompt), None, "{typed}, folder {is_folder}");
            }
        }
        let mut note = NewNotePrompt::open(target.clone(), None);
        note.buffer = "!!!".to_string();
        assert_eq!(new_item_refusal(&note).as_deref(), Some(NOTE_NAME_EMPTY));
        let mut folder = NewNotePrompt::open(target.clone(), None);
        folder.is_folder = true;
        folder.buffer = "!!!".to_string();
        assert_eq!(new_item_refusal(&folder), None, "create_folder answers FOLDER_NAME_EMPTY");

        // An accepted folder name is created under exactly that name.
        let mut forest = forest_of(vault_sections(&notez));
        let rebuild = || Ok(vault_sections(&notez));
        let plans = NewNoteTarget { dir: root.join("plans"), ..target };
        for name in ACCEPTED {
            let outcome = create_folder(&mut forest, &plans, name, &rebuild);
            assert_eq!(outcome.message, None, "{name}");
            assert!(has_entry_named(&plans.dir, name), "{name}");
        }
    }

    /// NZ-26: `n` takes `My Note` as `notez add` does (file `my-note`,
    /// heading as typed), `N` takes `Big Plans` as the folder `big-plans`;
    /// `00_quick` stays refused in both.
    #[test]
    fn new_note_and_new_folder_enter_accept_a_title_with_capitals_and_blanks() {
        let (_dir, notez, root) = vault();
        let target = NewNoteTarget { dir: root.clone(), scope: Scope::Personal, label: "personal".to_string() };
        let mut note = NewNotePrompt::open(target.clone(), None);
        note.buffer = "00_quick".to_string();
        assert_eq!(new_item_refusal(&note), Some(altered_name_message("00quick")));
        note.buffer = "My Note".to_string();
        assert_eq!(new_item_refusal(&note), None);
        let words = note.buffer.split_whitespace().map(String::from).collect();
        let created = add::create_in_dir(words, &target.dir, target.scope).unwrap();
        let file = file_name_of(&created.path);
        assert!(file.ends_with("my-note.md"), "{file}");
        let content = std::fs::read_to_string(&created.path).unwrap();
        assert!(content.starts_with("# My Note\n"), "{content}");

        let mut forest = forest_of(vault_sections(&notez));
        let rebuild = || Ok(vault_sections(&notez));
        let mut folder = NewNotePrompt::open(target.clone(), None);
        folder.is_folder = true;
        folder.buffer = "00_quick".to_string();
        assert_eq!(new_item_refusal(&folder), Some(altered_name_message("00quick")));
        folder.buffer = "Big Plans".to_string();
        assert_eq!(new_item_refusal(&folder), None);
        let outcome = create_folder(&mut forest, &target, &folder.buffer, &rebuild);
        assert_eq!(outcome.message, None);
        assert!(root.join("big-plans").is_dir());
        assert!(!has_entry_named(&root, "00quick") && !has_entry_named(&root, "00_quick"));
    }

    /// `r` with the cursor on `path`, the buffer set to `typed`, then
    /// `Enter`, as the event loop runs it.
    fn rename_enter_at(forest: &mut Forest, path: &Path, typed: &str) -> RenameEnter {
        let i = row(&forest.nodes, path_str(path));
        let shown = rename_request(&forest.nodes, &forest.sections, Some(i))
            .expect("the row opens the prompt")
            .expect("a row under the cursor");
        enter_rename(&mut forest.nodes, &mut forest.sections, i, &shown, typed)
    }

    #[test]
    fn rename_enter_refuses_altered_names_for_notes_and_folders_and_keeps_the_prompt() {
        let (_dir, notez, root) = vault();
        let mut forest = forest_of(vault_sections(&notez));
        let before = disk_entries(&notez);
        for path in [root.join("ideas"), root.join("top.md")] {
            for (typed, cleaned) in ALTERED {
                assert_eq!(
                    rename_enter_at(&mut forest, &path, typed),
                    RenameEnter::Keep(altered_name_message(cleaned)),
                    "{typed} on {}",
                    path.display()
                );
            }
        }
        assert_eq!(disk_entries(&notez), before, "nothing renamed");

        assert_eq!(rename_enter_at(&mut forest, &root.join("ideas"), "my-ideas"), RenameEnter::Done(None));
        assert!(root.join("my-ideas/a.md").is_file());
        assert_eq!(rename_enter_at(&mut forest, &root.join("top.md"), "00-quick"), RenameEnter::Done(None));
        assert!(root.join("00-quick.md").is_file() && !root.join("top.md").exists());
        let renamed = row(&forest.nodes, path_str(&root.join("00-quick.md")));
        assert_eq!(forest.nodes[renamed].name, "00-quick.md");
    }

    /// NZ-26: `r` takes `My Note` on a note (file `my-note.md`, heading as
    /// typed) and `My Ideas` on a folder (`my-ideas`); `Ideas` on the folder
    /// `ideas` sanitizes to its own name and changes nothing, silently.
    #[test]
    fn rename_enter_accepts_capitals_and_blanks_for_notes_and_folders() {
        let (_dir, notez, root) = vault();
        let mut forest = forest_of(vault_sections(&notez));
        let before = disk_entries(&notez);
        assert_eq!(rename_enter_at(&mut forest, &root.join("ideas"), "Ideas"), RenameEnter::Done(None));
        assert_eq!(disk_entries(&notez), before, "Ideas on ideas: nothing to do");
        assert!(has_entry_named(&root, "ideas"));
        assert!(changed_tag_maps(&forest.nodes, &forest.tag_roots, &forest.initial).is_empty());

        assert_eq!(
            rename_enter_at(&mut forest, &root.join("top.md"), "00_quick"),
            RenameEnter::Keep(altered_name_message("00quick"))
        );
        assert_eq!(
            rename_enter_at(&mut forest, &root.join("ideas"), "00_quick"),
            RenameEnter::Keep(altered_name_message("00quick"))
        );
        assert_eq!(disk_entries(&notez), before, "nothing renamed");

        assert_eq!(rename_enter_at(&mut forest, &root.join("top.md"), "My Note"), RenameEnter::Done(None));
        let note = root.join("my-note.md");
        assert!(note.is_file() && !root.join("top.md").exists());
        assert_eq!(std::fs::read_to_string(&note).unwrap(), "# My Note\n");
        assert_eq!(forest.nodes[row(&forest.nodes, path_str(&note))].name, "my-note.md");

        assert_eq!(rename_enter_at(&mut forest, &root.join("ideas"), "My Ideas"), RenameEnter::Done(None));
        assert!(root.join("my-ideas/a.md").is_file() && !root.join("ideas").exists());
    }

    #[test]
    fn note_rename_with_the_shown_title_unchanged_changes_nothing() {
        let (_dir, notez, root) = vault();
        let content = "# My_Note\n\nbody\n";
        for name in ["2026-10-06-My_Note.md", "x.MD"] {
            std::fs::write(root.join(name), content).unwrap();
        }
        std::fs::write(notez.join(".tags"), "personal/proj/2026-10-06-My_Note.md:3\npersonal/proj/x.MD:1\n").unwrap();
        let mut sections = vault_sections(&notez);
        sections[0].files.extend([root.join("2026-10-06-My_Note.md"), root.join("x.MD")]);
        let mut forest = forest_of(sections);
        let before = disk_entries(&notez);
        let tags = std::fs::read_to_string(notez.join(".tags")).unwrap();

        for (name, shown) in [("2026-10-06-My_Note.md", "My_Note"), ("x.MD", "x.MD")] {
            let path = root.join(name);
            for typed in [shown.to_string(), format!("  {shown} ")] {
                assert_eq!(rename_enter_at(&mut forest, &path, &typed), RenameEnter::Done(None), "{name}");
            }
            assert_eq!(std::fs::read_to_string(&path).unwrap(), content, "{name}: the heading stays");
            assert_eq!(forest.nodes[row(&forest.nodes, path_str(&path))].name, name);
        }
        assert_eq!(disk_entries(&notez), before);
        assert!(changed_tag_maps(&forest.nodes, &forest.tag_roots, &forest.initial).is_empty());
        assert_eq!(std::fs::read_to_string(notez.join(".tags")).unwrap(), tags);
    }

    // --- The todo board's store (NZ-20) ---

    /// [`move_fixture`] plus the todo board's store `_todos/` (with a note
    /// and a category folder), a sibling `_todos-archive/` and a personal
    /// `_todos/` folder, neither of which is the store.
    fn todo_fixture() -> (tempfile::TempDir, PathBuf, Vec<(Scope, PathBuf)>) {
        let (dir, notez, roots) = move_fixture();
        let personal = store_of(&roots, Scope::Personal);
        for sub in ["_todos/work", "_todos-archive"] {
            std::fs::create_dir_all(notez.join(sub)).unwrap();
        }
        std::fs::create_dir_all(personal.join("_todos")).unwrap();
        for file in [notez.join("_todos/t.md"), notez.join("_todos/work/w.md"), notez.join("_todos-archive/a.md"), personal.join("_todos/p.md")] {
            std::fs::write(file, "# todo\n").unwrap();
        }
        (dir, notez, roots)
    }

    #[test]
    fn delete_rename_move_and_set_scope_refuse_the_todo_store_and_what_is_in_it() {
        let (dir, notez, roots) = todo_fixture();
        let ctx = move_ctx(&roots);
        let forest = forest_of(move_sections(&roots));
        let before = disk_entries(dir.path());
        let (nodes, sections) = (&forest.nodes, &forest.sections);

        for sub in ["_todos", "_todos/t.md", "_todos/work", "_todos/work/w.md"] {
            let at = Some(row(nodes, path_str(&notez.join(sub))));
            assert_eq!(delete_request(nodes, sections, at, Some("proj")).err(), Some(TODOS_DELETE), "{sub}");
            assert_eq!(rename_request(nodes, sections, at).err(), Some(TODOS_RENAME), "{sub}");
            assert_eq!(move_request(nodes, sections, at, &ctx).err(), Some(TODOS_MOVE), "{sub}");
            assert_eq!(set_scope_request(nodes, sections, at, &ctx).err(), Some(TODOS_SET_SCOPE), "{sub}");
        }
        assert_eq!(TODOS_DELETE, "delete: the todo board's store is managed by the todo view");

        let personal = store_of(&roots, Scope::Personal);
        for path in [notez.join("_todos-archive"), notez.join("_todos-archive/a.md"), personal.join("_todos/p.md")] {
            let at = Some(row(nodes, path_str(&path)));
            assert!(matches!(delete_request(nodes, sections, at, Some("proj")), Ok(Some(_))), "{}", path.display());
            assert!(matches!(rename_request(nodes, sections, at), Ok(Some(_))), "{}", path.display());
            assert!(move_request(nodes, sections, at, &ctx).is_ok(), "{}", path.display());
            assert!(set_scope_request(nodes, sections, at, &ctx).is_ok(), "{}", path.display());
        }
        assert_eq!(disk_entries(dir.path()), before);
    }

    #[test]
    fn a_bulk_action_with_a_todo_store_row_is_refused_whole() {
        let (dir, notez, roots) = todo_fixture();
        let ctx = move_ctx(&roots);
        let forest = forest_of(move_sections(&roots));
        let before = disk_entries(dir.path());
        let set = [notez.join("c.md"), notez.join("_todos/work/w.md")];
        let marks: HashSet<PathBuf> = set.iter().cloned().collect();

        let refused = bulk_delete_request(&forest.nodes, &forest.sections, &marks, Some("proj")).err();
        assert_eq!(refused, Some(format!("_todos/work/w.md: {TODOS_DELETE}")));
        assert_eq!(bulk_move_at(&forest, &ctx, &set, false).err(), Some(format!("_todos/work/w.md: {TODOS_MOVE}")));
        assert_eq!(bulk_move_at(&forest, &ctx, &set, true).err(), Some(format!("_todos/work/w.md: {TODOS_SET_SCOPE}")));
        let folder: HashSet<PathBuf> = [notez.join("_todos")].into_iter().collect();
        let refused = bulk_delete_request(&forest.nodes, &forest.sections, &folder, Some("proj")).err();
        assert_eq!(refused, Some(format!("_todos/: {TODOS_DELETE}")));
        assert_eq!(disk_entries(dir.path()), before);
    }

    #[test]
    fn the_todo_store_is_never_a_move_destination() {
        let (dir, notez, roots) = todo_fixture();
        let ctx = move_ctx(&roots);
        let forest = forest_of(move_sections(&roots));
        let before = disk_entries(dir.path());
        let personal = store_of(&roots, Scope::Personal);
        let refusal = format!("move: {}", TODO_STORE_MANAGED);

        assert_eq!(move_plan(&forest, &ctx, &notez.join("c.md"), Scope::Global, "_todos"), Err(refusal.clone()));
        assert_eq!(move_plan(&forest, &ctx, &personal.join("c.md"), Scope::Global, "_todos/work"), Err(refusal.clone()));
        let set = [personal.join("c.md"), personal.join("ideas/a.md")];
        assert_eq!(bulk_move_plan(&forest, &ctx, &set, Scope::Global, "_todos").err(), Some(refusal));
        assert_eq!(disk_entries(dir.path()), before);

        let plan = move_plan(&forest, &ctx, &personal.join("c.md"), Scope::Global, "_todos-archive").expect("not the store");
        assert_eq!(plan.dst, notez.join("_todos-archive/c.md"));
        let plan = move_plan(&forest, &ctx, &notez.join("c.md"), Scope::Personal, "_todos").expect("a personal folder");
        assert_eq!(plan.dst, personal.join("_todos/c.md"));
    }

    #[test]
    fn the_todo_store_is_matched_as_the_file_system_resolves_it() {
        let dir = tempfile::tempdir().unwrap();
        let notez = dir.path().join("n");
        std::fs::create_dir_all(notez.join("_TODOS")).unwrap();
        std::fs::create_dir_all(notez.join("_todos-archive")).unwrap();
        // The board reads `<notez root>/_todos`; on a case-insensitive file
        // system that is `_TODOS`, on a case-sensitive one it is not.
        let is_store = notez.join("_todos").exists();
        assert_eq!(in_todo_store(&notez.join("_TODOS/x.md"), &notez), is_store);
        assert!(in_todo_store(&notez.join("_todos"), &notez));
        assert!(in_todo_store(&notez.join("_todos/a/b.md"), &notez));
        assert!(!in_todo_store(&notez.join("_todos-archive/a.md"), &notez));
        assert!(!in_todo_store(&notez, &notez));
        assert!(!in_todo_store(&notez.join("personal/proj/_todos"), &notez));
    }

    // --- NZ-34: reload on demand and when files change ---

    #[test]
    fn r_after_an_external_create_lists_the_note_and_keeps_cursor_expansion_filter_and_marks() {
        let (_dir, roots) = temp_tree();
        let rebuild = || Ok(list_sections(&roots));
        let mut forest = forest_of(list_sections(&roots));
        let root = roots[0].1.clone();
        let p = |rel: &str| root.join(rel).to_string_lossy().into_owned();
        for path in [p(""), p("ideas")] {
            let i = row(&forest.nodes, path.trim_end_matches('/'));
            forest.nodes[i].expanded = true;
        }
        let c = row(&forest.nodes, &p("c.md"));
        forest.nodes[c].flags = FLAG_PRIO;
        let mut marks = marks_of(&[&p("ideas/a.md"), &p("ideas/b.md")]);
        let search = "a";

        // Another shell adds a note and removes a marked one.
        std::fs::write(root.join("ideas/a2.md"), "# new\n").unwrap();
        std::fs::remove_file(root.join("ideas/b.md")).unwrap();

        let cursor = PathBuf::from(p("ideas/a.md"));
        let sel = reload_forest(&mut forest, &rebuild, &cursor, search).expect("the listing works");
        prune_marks(&mut marks, &forest.nodes);

        let n = &forest.nodes;
        let visible = compute_visible(n, search);
        assert_eq!(sel, Some(row(n, &p("ideas/a.md"))), "the cursor stays on its row");
        assert!(visible.contains(&row(n, &p("ideas/a2.md"))), "the new note is listed and passes the filter");
        assert!(!n.iter().any(|node| node.path == root.join("ideas/b.md")));
        assert!(n[row(n, root.to_str().unwrap())].expanded && n[row(n, &p("ideas"))].expanded);
        assert!(
            !n[row(n, roots[1].1.to_str().unwrap())].expanded,
            "a closed section stays closed"
        );
        assert_eq!(n[row(n, &p("c.md"))].flags, FLAG_PRIO, "unsaved tag edits survive");
        assert_eq!(marks, marks_of(&[&p("ideas/a.md")]), "a mark on a vanished row is pruned");
    }

    #[test]
    fn r_does_not_expand_the_folder_under_the_cursor() {
        let (_dir, roots) = temp_tree();
        let rebuild = || Ok(list_sections(&roots));
        let mut forest = forest_of(list_sections(&roots));
        let ideas = roots[0].1.join("ideas");
        let section = row(&forest.nodes, roots[0].1.to_str().unwrap());
        forest.nodes[section].expanded = true;
        let sel = reload_forest(&mut forest, &rebuild, &ideas, "").unwrap();
        let n = &forest.nodes;
        assert_eq!(sel, Some(row(n, ideas.to_str().unwrap())));
        assert!(!n[row(n, ideas.to_str().unwrap())].expanded, "the cursor folder stays collapsed");
    }

    #[test]
    fn r_moves_the_cursor_to_the_next_note_when_its_note_vanished() {
        let (_dir, roots) = temp_tree();
        let rebuild = || Ok(list_sections(&roots));
        let mut forest = forest_of(list_sections(&roots));
        let root = &roots[0].1;
        for path in [root.clone(), root.join("ideas")] {
            let i = row(&forest.nodes, path.to_str().unwrap());
            forest.nodes[i].expanded = true;
        }
        std::fs::remove_file(root.join("ideas/a.md")).unwrap();
        let sel = reload_forest(&mut forest, &rebuild, &root.join("ideas/a.md"), "").unwrap();
        assert_eq!(sel, Some(row(&forest.nodes, root.join("ideas/b.md").to_str().unwrap())));
    }

    #[test]
    fn a_failed_reload_keeps_the_tree_and_names_the_error() {
        let (_dir, roots) = temp_tree();
        let mut forest = forest_of(list_sections(&roots));
        let before: Vec<PathBuf> = forest.nodes.iter().map(|n| n.path.clone()).collect();
        let failing = || -> Result<Vec<SectionSpec>> { Err(anyhow::anyhow!("disk gone")) };
        let err = reload_forest(&mut forest, &failing, Path::new(""), "").unwrap_err();
        assert_eq!(err, "reload failed: disk gone");
        let after: Vec<PathBuf> = forest.nodes.iter().map(|n| n.path.clone()).collect();
        assert_eq!(before, after);
    }

    fn at(secs: u64) -> Option<SystemTime> {
        Some(SystemTime::UNIX_EPOCH + Duration::from_secs(secs))
    }

    fn snapshot(entries: &[(&str, Option<SystemTime>)]) -> ProbeSnapshot {
        entries.iter().map(|(p, t)| (PathBuf::from(p), *t)).collect()
    }

    #[test]
    fn the_probe_sees_a_changed_root_or_expanded_folder_and_nothing_else() {
        let old = snapshot(&[("/r", at(1)), ("/r/ideas", at(2))]);
        assert!(!probe_changed(&old, &old.clone()), "no change");
        assert!(probe_changed(&old, &snapshot(&[("/r", at(5)), ("/r/ideas", at(2))])), "root mtime");
        assert!(probe_changed(&old, &snapshot(&[("/r", at(1)), ("/r/ideas", at(5))])), "folder mtime");
        assert!(probe_changed(&old, &snapshot(&[("/r", at(1)), ("/r/ideas", None)])), "folder vanished");
        let missing = snapshot(&[("/r", None)]);
        assert!(probe_changed(&missing, &snapshot(&[("/r", at(1))])), "root appeared");
    }

    #[test]
    fn a_path_probed_on_one_side_only_is_no_change() {
        let old = snapshot(&[("/r", at(1))]);
        let expanded = snapshot(&[("/r", at(1)), ("/r/ideas", at(9))]);
        assert!(!probe_changed(&old, &expanded), "a folder just expanded is a new baseline");
        assert!(!probe_changed(&expanded, &old), "a folder just collapsed is not probed");
    }

    #[test]
    fn the_probe_paths_are_every_section_root_and_every_expanded_folder() {
        let files = ["ideas/a.md", "ideas/deep/x.md", "plans/p.md", "z.md"];
        let sections = vec![spec("/r", "S", &files), spec("/empty", "E", &[])];
        let (mut nodes, _) = build_forest(&sections);
        let i = row(&nodes, "/r/ideas");
        nodes[i].expanded = true;
        let paths = probe_paths(&sections, &nodes);
        let mut got: Vec<&str> = paths.iter().map(|p| p.to_str().unwrap()).collect();
        got.sort();
        assert_eq!(got, ["/empty", "/r", "/r/ideas"], "collapsed folders and files are not probed");
    }

    #[test]
    fn a_collapsed_folders_change_is_ignored_and_an_expanded_ones_is_seen() {
        let (_dir, roots) = temp_tree();
        std::fs::create_dir_all(roots[0].1.join("plans")).unwrap();
        std::fs::write(roots[0].1.join("plans/p.md"), "# p\n").unwrap();
        let sections = list_sections(&roots);
        let (mut nodes, _) = build_forest(&sections);
        let ideas = roots[0].1.join("ideas");
        let plans = roots[0].1.join("plans");
        let i = row(&nodes, ideas.to_str().unwrap());
        nodes[i].expanded = true;
        let touch = |dir: &Path, secs: u64| {
            let when = SystemTime::UNIX_EPOCH + Duration::from_secs(secs);
            std::fs::File::open(dir).unwrap().set_modified(when).unwrap();
        };
        touch(&ideas, 1_000);
        touch(&plans, 1_000);
        let base = probe_snapshot(&probe_paths(&sections, &nodes));

        touch(&plans, 2_000);
        assert!(!probe_changed(&base, &probe_snapshot(&probe_paths(&sections, &nodes))), "plans is collapsed");

        touch(&ideas, 2_000);
        assert!(probe_changed(&base, &probe_snapshot(&probe_paths(&sections, &nodes))), "ideas is expanded");
    }

    #[test]
    fn a_refreshed_probe_sees_no_change_until_the_disk_changes_again() {
        let (_dir, roots) = temp_tree();
        let mut forest = forest_of(list_sections(&roots));
        let ideas = roots[0].1.join("ideas");
        let i = row(&forest.nodes, ideas.to_str().unwrap());
        forest.nodes[i].expanded = true;
        let touch = |secs: u64| {
            let when = SystemTime::UNIX_EPOCH + Duration::from_secs(secs);
            std::fs::File::open(&ideas).unwrap().set_modified(when).unwrap();
        };
        let fresh = |forest: &Forest| probe_snapshot(&probe_paths(&forest.sections, &forest.nodes));
        touch(1_000);
        let mut probe = None;
        refresh_probe(&mut probe, &forest);

        // The browser's own change, then the refresh it makes after it.
        touch(2_000);
        assert!(probe_changed(probe.as_ref().unwrap(), &fresh(&forest)), "unrefreshed, the probe would reload");
        refresh_probe(&mut probe, &forest);
        assert!(!probe_changed(probe.as_ref().unwrap(), &fresh(&forest)), "refreshed, no reload for it");

        touch(3_000);
        assert!(probe_changed(probe.as_ref().unwrap(), &fresh(&forest)), "a later change is still seen");
    }

    #[test]
    fn reload_view_keeps_the_filter_and_focus_expansion_and_says_whether_the_cursor_path_was_kept() {
        let (_dir, roots) = temp_tree();
        let rebuild = || Ok(list_sections(&roots));
        let mut forest = forest_of(list_sections(&roots));
        let root = &roots[0].1;
        let path = |rel: &str| root.join(rel);
        for dir in [root.clone(), path("ideas")] {
            let i = row(&forest.nodes, dir.to_str().unwrap());
            forest.nodes[i].expanded = true;
        }
        let search = "b";
        let b = row(&forest.nodes, path("ideas/b.md").to_str().unwrap());
        let mut state = ListState::default();
        state.select(compute_visible(&forest.nodes, search).iter().position(|&i| i == b));
        // Focus mode saved the second section's row as collapsed.
        let other = row(&forest.nodes, roots[1].1.to_str().unwrap());
        let mut saved = vec![(other, false)];

        // A note listed before the cursor shifts every row index after it.
        std::fs::write(path("ideas/ab.md"), "# ab\n").unwrap();
        let kept = reload_view(&mut forest, &rebuild, &mut state, &mut saved, search).unwrap();

        let n = &forest.nodes;
        let b = row(n, path("ideas/b.md").to_str().unwrap());
        assert_eq!(kept, Some(b), "the cursor stayed on its path, at its new row");
        assert_eq!(compute_visible(n, search)[state.selected().unwrap()], b);
        assert!(compute_visible(n, search).contains(&row(n, path("ideas/ab.md").to_str().unwrap())));
        assert_eq!(saved, vec![(row(n, roots[1].1.to_str().unwrap()), false)], "focus expansion follows its path");

        std::fs::remove_file(path("ideas/b.md")).unwrap();
        let kept = reload_view(&mut forest, &rebuild, &mut state, &mut saved, search).unwrap();
        assert_eq!(kept, None, "the cursor's note vanished, so its path was not kept");
        assert!(state.selected().is_some());
    }

    /// A temp tree with the first store's `ideas` folder expanded, its mtime
    /// set to 1000 s and recorded by the probe, then set to 2000 s: a change
    /// made elsewhere while a prompt was open, which the probe still owes.
    fn probe_with_a_pending_change() -> (tempfile::TempDir, Vec<(Scope, PathBuf)>, Forest, Option<ProbeSnapshot>) {
        let (dir, roots) = temp_tree();
        let mut forest = forest_of(list_sections(&roots));
        let ideas = roots[0].1.join("ideas");
        let i = row(&forest.nodes, ideas.to_str().unwrap());
        forest.nodes[i].expanded = true;
        let touch = |secs: u64| {
            let when = SystemTime::UNIX_EPOCH + Duration::from_secs(secs);
            std::fs::File::open(&ideas).unwrap().set_modified(when).unwrap();
        };
        touch(1_000);
        let mut probe = None;
        refresh_probe(&mut probe, &forest);
        touch(2_000);
        (dir, roots, forest, probe)
    }

    fn still_owed(probe: &Option<ProbeSnapshot>, forest: &Forest) -> bool {
        probe_changed(probe.as_ref().unwrap(), &probe_snapshot(&probe_paths(&forest.sections, &forest.nodes)))
    }

    #[test]
    fn a_no_op_rename_a_refused_folder_and_a_cancelled_or_unlisted_delete_leave_the_probe_reading() {
        let (_dir, roots, mut forest, mut probe) = probe_with_a_pending_change();
        let rebuild = || Ok(list_sections(&roots));
        let failing = || -> Result<Vec<SectionSpec>> { Err(anyhow::anyhow!("disk gone")) };
        let ideas = roots[0].1.join("ideas");
        let mut state = ListState::default();
        let mut saved = Vec::new();

        let i = row(&forest.nodes, ideas.to_str().unwrap());
        let before = forest.nodes[i].path.clone();
        let entered = enter_rename(&mut forest.nodes, &mut forest.sections, i, "ideas", "ideas");
        assert_eq!(entered, RenameEnter::Done(None));
        let relisted = relist_after_rename(&mut forest, &rebuild, &mut state, &mut saved, "", &mut probe, i, &before);
        assert!(relisted.is_none(), "an unchanged rename lists nothing again");
        assert!(still_owed(&probe, &forest), "no-op rename");

        for name in ["", "ideas"] {
            let outcome = create_folder(&mut forest, &folder_target(&roots, roots[0].0, ""), name, &rebuild);
            assert!(outcome.message.is_some(), "{name:?} is refused");
            assert!(!outcome.relisted, "refused folder {name:?}");
            assert!(still_owed(&probe, &forest), "refused folder {name:?}");
        }

        let note = roots[0].1.join("c.md");
        let prompt = prompt_at(&mut forest, &note);
        let mut retired = Vec::new();
        assert!(answer_delete(KeyCode::Char('n'), &mut forest, &mut retired, &prompt, "", &rebuild).is_none());
        assert!(still_owed(&probe, &forest), "cancelled delete");

        let outcome = answer_delete(KeyCode::Char('y'), &mut forest, &mut retired, &prompt, "", &failing).unwrap();
        assert!(!outcome.relisted, "a delete whose listing failed used the fallback");
    }

    #[test]
    fn a_successful_rename_lists_again_keeps_the_cursor_on_it_and_refreshes_the_probe() {
        let (_dir, roots, mut forest, mut probe) = probe_with_a_pending_change();
        let rebuild = || Ok(list_sections(&roots));
        let root = &roots[0].1;
        let i = row(&forest.nodes, root.to_str().unwrap());
        forest.nodes[i].expanded = true;
        let ideas = root.join("ideas");
        let mut state = ListState::default();
        let mut saved = Vec::new();

        let i = row(&forest.nodes, ideas.to_str().unwrap());
        let entered = enter_rename(&mut forest.nodes, &mut forest.sections, i, "ideas", "thoughts");
        assert_eq!(entered, RenameEnter::Done(None));
        let result = relist_after_rename(&mut forest, &rebuild, &mut state, &mut saved, "", &mut probe, i, &ideas);

        let n = &forest.nodes;
        let thoughts = row(n, root.join("thoughts").to_str().unwrap());
        assert_eq!(result, Some(Ok(Some(thoughts))), "the cursor row is the renamed folder");
        assert_eq!(compute_visible(n, "")[state.selected().unwrap()], thoughts);
        assert!(n[thoughts].expanded, "the folder keeps its open state");
        assert!(!still_owed(&probe, &forest), "the relist is the probe's new reading");
    }

    #[cfg(unix)]
    #[test]
    fn a_symlinked_section_root_is_probed_at_its_target() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target");
        std::fs::create_dir_all(&target).unwrap();
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let paths = vec![link];
        let touch = |secs: u64| {
            let when = SystemTime::UNIX_EPOCH + Duration::from_secs(secs);
            std::fs::File::open(&target).unwrap().set_modified(when).unwrap();
        };
        touch(1_000);
        let base = probe_snapshot(&paths);
        touch(2_000);
        assert!(probe_changed(&base, &probe_snapshot(&paths)), "a change under the link's target is seen");
    }

    #[test]
    fn r_is_a_help_only_browse_key_in_the_view_group_and_free_in_every_other_mode() {
        let rows: Vec<(usize, &KeyHint)> = TREE_KEYS.iter().enumerate().filter(|(_, k)| k.key == "R").collect();
        assert_eq!(rows.len(), 1);
        let (idx, hint) = rows[0];
        assert_eq!(hint.modes, BROWSE);
        assert_eq!(hint.group, Group::View);
        assert_eq!(hint.slot, Slot::HelpOnly);
        assert!(help::rows(TREE_KEYS).contains(&help::Row::Key(idx)));
        assert_eq!(preview_focus_key(KeyCode::Char('R'), 10), PreviewFocusKey::Pass);
        assert!(!Panes::default().handle_key(KeyCode::Char('R'), 100));
    }
}
