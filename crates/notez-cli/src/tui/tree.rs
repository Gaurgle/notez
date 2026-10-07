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

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, MouseButton, MouseEventKind};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Padding, Paragraph};

use notez_core::config::Config;
use notez_core::core::Scope;
use notez_core::filter::{self, Filter};
use notez_core::note_tags;
use notez_core::tags::FLAG_DEFS;

use super::footer::{self, Group, KeyHint, Mode, QUIT_HINT_RESERVED_COLS, Slot, Toggle};
use super::help::{self, HelpState};
use super::{VimCommandMode, VimKey, theme};
use crate::commands::{add, rename};

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

    let mut terminal = super::enter().context("failed to enter TUI")?;
    let result = event_loop(&mut terminal, &mut forest, &mut retired, ctx, config, rebuild);
    let Forest { nodes, tag_roots, initial, .. } = forest;
    super::leave().context("failed to leave TUI")?;
    result?;

    Ok(changed_tag_maps_retiring(&nodes, &tag_roots, &initial, &retired))
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
    /// listed ancestor, skipping rows the filter `search` hides. An emptied
    /// folder is not listed, since folders come from the files in them.
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

    fn file_count(&self) -> usize {
        self.files.len() + self.dirs.values().map(DirTmp::file_count).sum::<usize>()
    }
}

/// Legacy sort rule: `NN_` numbered dirs sort before other dirs.
fn is_numbered(name: &str) -> bool {
    let b = name.as_bytes();
    b.len() >= 3 && b[0].is_ascii_digit() && b[1].is_ascii_digit() && b[2] == b'_'
}

/// Build the flattened forest: one depth-0 wrapper node per section, with
/// intermediate directories derived from the files' relative paths. Returns
/// the nodes plus the dedup'd tag-root list they index into.
fn build_forest(sections: &[SectionSpec]) -> (Vec<TreeNode>, Vec<PathBuf>) {
    let mut nodes: Vec<TreeNode> = Vec::new();
    let mut tag_roots: Vec<PathBuf> = Vec::new();

    for (section_idx, spec) in sections.iter().enumerate() {
        if spec.files.is_empty() {
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

/// Contiguous 5-dot geometry shared with the todoz board: dot 0 sits at
/// `area_x + 5` (4-column highlight symbol plus one leading space).
fn mouse_x_to_dot(mouse_col: u16, area_x: u16) -> Option<u8> {
    let dot_start = area_x.saturating_add(5);
    let dot_end = dot_start + 4;
    if mouse_col >= dot_start && mouse_col <= dot_end {
        Some((mouse_col - dot_start) as u8)
    } else {
        None
    }
}

/// The 5 fixed tag-dot slots with leading space, matching the todoz rows.
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

/// The colour of a section's badge, scope icon and scope word. A docs
/// section is published with the repository, so it takes the public colour.
fn section_color(spec: &SectionSpec) -> Color {
    theme::scope_color(if spec.is_doc { Scope::Public } else { spec.scope })
}

/// The one-column badge after a row's tag dots: the section's icon in its
/// colour on every file and folder row, a blank on a section header (which
/// shows the icon next to its label). Render only: the filter, preview,
/// mouse hit testing and tag keys never see it, and the tag dots before it
/// keep their columns.
fn row_badge(node: &TreeNode, spec: Option<&SectionSpec>) -> Span<'static> {
    match spec {
        Some(spec) if node.depth > 0 && !spec.icon.is_empty() => {
            Span::styled(spec.icon, Style::default().fg(section_color(spec)))
        }
        _ => Span::raw(" "),
    }
}

/// One list row: the tag dots, the scope badge, the tree indentation and
/// branch glyph, the name, and on a directory row a dotted leader to its
/// file count. A section header also shows its scope icon before the label
/// and the scope word after it, both in the scope colour. `spec` is the
/// row's section and `inner_width` the list pane's text width.
fn row_line(node: &TreeNode, spec: Option<&SectionSpec>, inner_width: usize) -> Line<'static> {
    let indent = "  ".repeat(node.depth);
    let icon = if node.depth == 0 {
        if node.is_dir {
            if node.expanded { "▼ " } else { "▶ " }
        } else {
            "  "
        }
    } else if node.is_dir {
        if node.expanded { "├─▼ " } else { "├─▶ " }
    } else {
        "│   "
    };

    let header_color = spec.map_or(theme::OVERLAY, section_color);
    let mut spans = flags_slots(node.flags);
    spans.push(row_badge(node, spec));
    spans.push(Span::styled(
        format!("{}{}", indent, icon),
        Style::default().fg(theme::SURFACE),
    ));
    if !node.scope_icon.is_empty() {
        spans.push(Span::styled(
            format!("{} ", node.scope_icon),
            Style::default().fg(header_color),
        ));
    }
    if node.is_dir {
        spans.push(Span::styled(node.name.clone(), Style::default().fg(theme::SAPPHIRE)));
        let scope_word = spec.filter(|_| node.depth == 0).map(|s| s.scope.label());
        if let Some(word) = scope_word {
            spans.push(Span::raw(" "));
            spans.push(Span::styled(word, Style::default().fg(header_color)));
        }
        if node.child_count > 0 {
            let count_str = format!("{}", node.child_count);
            let scope_len = if node.scope_icon.is_empty() { 0 } else { 2 };
            let word_len = scope_word.map_or(0, |w| w.chars().count() + 1);
            let badge_len = 1;
            let prefix_len = 7
                + badge_len
                + indent.len()
                + icon.len()
                + node.name.chars().count()
                + scope_len
                + word_len;
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
    if let Some(buffer) = rename {
        StatusSlot::Rename(buffer)
    } else if let Some(message) = message {
        StatusSlot::Message(message)
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
struct NewNotePrompt {
    target: NewNoteTarget,
    origin: NewNoteTarget,
    project: Option<String>,
    buffer: String,
}

impl NewNotePrompt {
    fn open(target: NewNoteTarget, project: Option<String>) -> Self {
        Self { origin: target.clone(), target, project, buffer: String::new() }
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

/// The new-note prompt that leads the footer while a title is typed.
fn new_note_lead(label: &str, buffer: &str) -> Vec<Span<'static>> {
    vec![
        Span::styled(format!(" new note in {label}: "), Style::default().fg(theme::MAUVE)),
        Span::styled(buffer.to_string(), Style::default().fg(theme::TEXT)),
        Span::styled("_", Style::default().fg(theme::OVERLAY)),
    ]
}

// --- Delete ---

/// The footer message for `d` on a folder row.
const FOLDER_DELETE_UNAVAILABLE: &str = "folder delete is not available yet";

/// The open delete confirmation for one note: its path, its path when the
/// session started (a rename moves `path` only), the tag root its `.tags`
/// key lives under, its path relative to its section's root, and the scope
/// it is deleted from with that scope's label.
#[derive(Debug, Clone)]
struct DeletePrompt {
    path: PathBuf,
    origin: PathBuf,
    tag_root: PathBuf,
    rel: String,
    scope: Scope,
    label: String,
}

/// What `d` does with the cursor on `row`: `Ok(Some)` opens the prompt on a
/// note, `Err` is the footer message for a folder row, and `Ok(None)` (no
/// row under the cursor) does nothing.
fn delete_request(
    nodes: &[TreeNode],
    sections: &[SectionSpec],
    row: Option<usize>,
    current_project: Option<&str>,
) -> std::result::Result<Option<DeletePrompt>, &'static str> {
    let Some(node) = row.and_then(|i| nodes.get(i)) else {
        return Ok(None);
    };
    if node.is_dir {
        return Err(FOLDER_DELETE_UNAVAILABLE);
    }
    let Some(spec) = sections.get(node.section) else {
        return Ok(None);
    };
    let rel = node.path.strip_prefix(&spec.root).unwrap_or(&node.path);
    Ok(Some(DeletePrompt {
        path: node.path.clone(),
        origin: node.origin.clone(),
        tag_root: spec.tag_root.clone(),
        rel: rel.to_string_lossy().into_owned(),
        scope: spec.scope,
        label: scope_label(spec.scope, spec.project.as_deref(), current_project),
    }))
}

/// The confirmation question. Local scratch notes are not in any
/// repository, so the question says a delete there cannot be undone.
fn delete_question(prompt: &DeletePrompt) -> String {
    let warning = if prompt.scope == Scope::Local { " (not recoverable)" } else { "" };
    format!("delete {} from {}?{warning} y/n", prompt.rel, prompt.label)
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
}

/// Answer the open delete prompt with `key`. Anything but `y` cancels and
/// returns `None` without touching the disk or the forest. On `y` the note
/// is removed, its `.tags` keys (current and original path) are added to
/// `retired`, and the forest is rebuilt from `rebuild` whether or not the
/// removal worked, so the tree matches the disk. If `rebuild` fails, the
/// current rows minus the deleted note are used instead.
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
    let removed = std::fs::remove_file(&prompt.path);
    let mut message = match &removed {
        Ok(()) => {
            for path in [&prompt.path, &prompt.origin] {
                if let Some(tag_key) = rel_key(&prompt.tag_root, path) {
                    if !retired.iter().any(|(r, k)| *r == prompt.tag_root && *k == tag_key) {
                        retired.push((prompt.tag_root.clone(), tag_key));
                    }
                }
            }
            format!("deleted {}", prompt.rel)
        }
        Err(e) => format!("delete failed: {e}"),
    };
    let sections = rebuild().unwrap_or_else(|e| {
        message = format!("{message}, but the list could not be refreshed: {e:#}");
        let gone = removed.is_ok().then_some(prompt.path.as_path());
        current_sections_without(forest, gone)
    });
    let row = forest.rebuild_after_delete(sections, &prompt.path, search);
    Some(DeleteOutcome { row, message })
}

/// The forest's sections listing its current file rows, minus `gone`: the
/// fallback listing when the rebuild closure fails. Takes the sections out
/// of `forest`; the caller puts a rebuilt set back.
fn current_sections_without(forest: &mut Forest, gone: Option<&Path>) -> Vec<SectionSpec> {
    let mut sections = std::mem::take(&mut forest.sections);
    for (i, spec) in sections.iter_mut().enumerate() {
        spec.files = forest
            .nodes
            .iter()
            .filter(|n| !n.is_dir && n.section == i && Some(n.path.as_path()) != gone)
            .map(|n| n.path.clone())
            .collect();
    }
    sections
}

// --- Rebuild ---

/// [`carry_state`], then expand the ancestors of `created` so it is
/// visible; its index is returned.
fn restore_state(old: &[TreeNode], new: &mut [TreeNode], created: &Path) -> Option<usize> {
    carry_state(old, new);
    let idx = new.iter().position(|n| !n.is_dir && n.path == created)?;
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

// --- Keys: one table for the footer and the help overlay ---

const BROWSE: &[Mode] = &[Mode::Normal, Mode::Focus];
const BROWSE_AND_TAG: &[Mode] = &[Mode::Normal, Mode::Focus, Mode::Tag];
const FILTERING: &[Mode] = &[Mode::Filter];
const TAGGING: &[Mode] = &[Mode::Tag];
const RENAMING: &[Mode] = &[Mode::Rename];
const NEW_NOTE: &[Mode] = &[Mode::NewItem];
const CONFIRMING: &[Mode] = &[Mode::ConfirmDelete];
const COMMAND: &[Mode] = &[Mode::VimCommand];

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
    key("j/k", "move", "move down / up (also Down / Up)", theme::TEXT, Group::Navigate, BROWSE_AND_TAG, Slot::HelpOnly, None),
    key("l", "expand", "expand directory (also Right)", theme::MAUVE, Group::Navigate, BROWSE_AND_TAG, Slot::HelpOnly, None),
    key("h", "collapse", "collapse directory / go to parent (also Left)", theme::MAUVE, Group::Navigate, BROWSE_AND_TAG, Slot::HelpOnly, None),
    key("J/K", "preview", "scroll preview down / up", theme::TEXT, Group::Navigate, BROWSE, Slot::HelpOnly, None),
    key("wheel", "preview", "mouse wheel scrolls the preview", theme::TEXT, Group::Navigate, BROWSE, Slot::HelpOnly, None),
    key("click", "select", "click a row to select it and toggle a directory", theme::TEXT, Group::Navigate, BROWSE, Slot::HelpOnly, None),
    key("o", "open", "open file / toggle directory (also Enter)", theme::GREEN, Group::Edit, BROWSE, Slot::Priority(1), None),
    key("t", "tags", "tag mode on / off", theme::PEACH, Group::Edit, BROWSE_AND_TAG, Slot::Priority(2), Some(Toggle::Tag)),
    key("r", "rename", "rename note", theme::MAUVE, Group::Edit, BROWSE, Slot::Priority(6), None),
    key("1-5", "toggle", "tag mode: toggle tag 1 to 5 on the note", theme::PEACH, Group::Edit, TAGGING, Slot::Priority(1), None),
    key("esc", "close", "tag mode: close", theme::PEACH, Group::Edit, TAGGING, Slot::Priority(3), None),
    key("click dot", "tag", "click a note's tag dot to toggle that tag", theme::PEACH, Group::Edit, BROWSE, Slot::HelpOnly, None),
    key("enter", "confirm", "rename: confirm", theme::GREEN, Group::Edit, RENAMING, Slot::Priority(1), None),
    key("esc", "cancel", "rename: cancel", theme::PEACH, Group::Edit, RENAMING, Slot::Priority(2), None),
    key("bksp", "delete", "rename: delete the last char", theme::TEXT, Group::Edit, RENAMING, Slot::Priority(3), None),
    key("n", "new", "new note in the folder under the cursor (the prompt names the scope)", theme::GREEN, Group::Edit, BROWSE, Slot::Priority(3), None),
    key("enter", "create", "new note: create it and open it in the editor", theme::GREEN, Group::Edit, NEW_NOTE, Slot::Priority(1), None),
    key("esc", "cancel", "new note: cancel, nothing is created", theme::PEACH, Group::Edit, NEW_NOTE, Slot::Priority(2), None),
    key("tab", "scope", "new note: next scope (personal, public, local, global), at its root", theme::SAPPHIRE, Group::Edit, NEW_NOTE, Slot::Priority(3), None),
    key("bksp", "delete", "new note: delete the last char", theme::TEXT, Group::Edit, NEW_NOTE, Slot::Priority(4), None),
    key("d", "delete", "delete the note under the cursor (asks first; no undo)", theme::RED, Group::Edit, BROWSE, Slot::Priority(7), None),
    key("y", "confirm", "delete: yes, delete the note", theme::RED, Group::Edit, CONFIRMING, Slot::Priority(1), None),
    key("n/esc", "cancel", "delete: cancel (any other key too), nothing is deleted", theme::PEACH, Group::Edit, CONFIRMING, Slot::Priority(2), None),
    key("/", "filter", "filter: text and #tag (starts a new filter)", theme::YELLOW, Group::Filter, BROWSE_AND_TAG, Slot::Priority(4), Some(Toggle::Filter)),
    key("enter", "keep", "filter: keep the filter, back to the list", theme::GREEN, Group::Filter, FILTERING, Slot::Priority(1), None),
    key("esc", "clear", "filter: clear it and close", theme::PEACH, Group::Filter, FILTERING, Slot::Priority(2), None),
    key("\u{2190}/\u{2192}", "cursor", "filter: move the cursor", theme::TEXT, Group::Filter, FILTERING, Slot::Priority(3), None),
    key("bksp", "delete", "filter: delete the char before the cursor; at the start, clear the filter and close", theme::TEXT, Group::Filter, FILTERING, Slot::Priority(4), None),
    key("esc", "clear", "clear the filter; with no filter, quit", theme::PEACH, Group::Filter, BROWSE, Slot::HelpOnly, None),
    key("click bar", "filter", "click the filter bar to filter, a dot to filter by that tag", theme::YELLOW, Group::Filter, BROWSE, Slot::HelpOnly, None),
    key("f", "focus", "focus the current section (again to leave)", theme::GREEN, Group::View, BROWSE, Slot::Priority(3), Some(Toggle::Focus)),
    key("v", "view all", "expand all / collapse all sections", theme::SAPPHIRE, Group::View, BROWSE, Slot::Priority(5), Some(Toggle::ExpandAll)),
    key("?", "help", "this help (? or esc closes)", theme::MAUVE, Group::View, BROWSE, Slot::Pinned, Some(Toggle::Help)),
    key(":q", "quit", "vim-style quit (also :wq, :qa, :q!)", theme::MAUVE, Group::View, BROWSE, Slot::HelpOnly, None),
    key("enter", "run", ":command: run it", theme::GREEN, Group::View, COMMAND, Slot::Priority(1), None),
    key("esc", "cancel", ":command: close the command line, nothing else", theme::PEACH, Group::View, COMMAND, Slot::Priority(2), None),
    key("bksp", "delete", ":command: delete the last char; deleting the : closes it", theme::TEXT, Group::View, COMMAND, Slot::Priority(3), None),
    key("q", "quit", "quit", theme::PEACH, Group::View, BROWSE, Slot::Quit, None),
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
    ctx: &TreeContext,
    config: &Config,
    rebuild: &dyn Fn() -> Result<Vec<SectionSpec>>,
) -> Result<()> {
    let mut new_note: Option<NewNotePrompt> = None;
    let mut confirm_delete: Option<DeletePrompt> = None;
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
    let mut status_message: Option<String> = None;
    let mut preview_scroll: u16 = 0;
    let mut last_preview_idx: usize = usize::MAX;
    let mut filter_strip_area: Rect = Rect::default();
    let mut list_inner_area: Rect = Rect::default();
    let mut visible_for_mouse: Vec<usize> = Vec::new();
    let mut prev_filter_buffer = String::new();

    loop {
        let nodes = &mut forest.nodes;
        let sections = &forest.sections;
        derive_dir_flags(nodes);

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
                let cols = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                    .split(rows[0]);
                let inner_width = cols[0].width.saturating_sub(6) as usize;

                let items: Vec<ListItem> = visible
                    .iter()
                    .map(|&idx| {
                        let node = &nodes[idx];
                        ListItem::new(row_line(node, sections.get(node.section), inner_width))
                    })
                    .collect();

                let header = Line::from(vec![
                    Span::styled(
                        format!(" {} ", ctx.title),
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

                let block = Block::default()
                    .title(header)
                    .borders(Borders::ALL)
                    .border_style(theme::border())
                    .border_type(ratatui::widgets::BorderType::Rounded)
                    .padding(Padding::new(1, 1, 1, 0));
                let inner = block.inner(cols[0]);
                let inner_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(1),
                        Constraint::Length(1),
                        Constraint::Min(1),
                    ])
                    .split(inner);
                filter_strip_area = inner_chunks[0];
                list_inner_area = inner_chunks[2];
                visible_for_mouse = visible.clone();

                frame.render_widget(block, cols[0]);
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
                    let list = List::new(items)
                        .highlight_style(theme::selected())
                        .highlight_symbol("  ▸ ");
                    frame.render_stateful_widget(list, inner_chunks[2], &mut state);
                }

                if real_idx != last_preview_idx {
                    preview_scroll = 0;
                    last_preview_idx = real_idx;
                }

                // Preview pane: file content, or a directory listing.
                let preview_lines: Vec<Line> = if real_idx < nodes.len()
                    && !nodes[real_idx].is_dir
                {
                    match std::fs::read_to_string(&nodes[real_idx].path) {
                        Ok(content) => content
                            .lines()
                            .map(|line| {
                                let owned = line.to_string();
                                if owned.starts_with('#') {
                                    Line::from(Span::styled(
                                        owned,
                                        Style::default()
                                            .fg(theme::MAUVE)
                                            .add_modifier(Modifier::BOLD),
                                    ))
                                } else if owned.starts_with("- [") {
                                    Line::from(Span::styled(
                                        owned,
                                        Style::default().fg(theme::SAPPHIRE),
                                    ))
                                } else if owned.starts_with("- ") || owned.starts_with("* ")
                                {
                                    Line::from(Span::styled(
                                        owned,
                                        Style::default().fg(theme::TEXT),
                                    ))
                                } else {
                                    Line::from(Span::styled(
                                        owned,
                                        Style::default().fg(theme::SUBTEXT),
                                    ))
                                }
                            })
                            .collect(),
                        Err(_) => vec![Line::from(Span::styled(
                            "  unable to read file",
                            Style::default().fg(theme::OVERLAY),
                        ))],
                    }
                } else if real_idx < nodes.len() {
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

                let total_lines = preview_lines.len() as u16;
                let preview_height = cols[1].height.saturating_sub(2);
                let max_scroll = total_lines.saturating_sub(preview_height);
                if preview_scroll > max_scroll {
                    preview_scroll = max_scroll;
                }

                let mut preview_title_spans = flags_slots(if real_idx < nodes.len() {
                    nodes[real_idx].flags
                } else {
                    0
                });
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
                    .title(Line::from(preview_title_spans))
                    .title_bottom(Line::from(Span::styled(
                        real_path_display,
                        Style::default().fg(theme::OVERLAY),
                    )))
                    .borders(Borders::ALL)
                    .border_style(theme::border())
                    .border_type(ratatui::widgets::BorderType::Rounded)
                    .padding(Padding::new(1, 1, 0, 0));
                frame.render_widget(
                    Paragraph::new(preview_lines)
                        .block(preview_block)
                        .scroll((preview_scroll, 0)),
                    cols[1],
                );

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
                let status = match slot {
                    _ if confirm_delete.is_some() => {
                        let prompt = confirm_delete.as_ref().expect("checked by the guard");
                        lead_with_hints(delete_lead(prompt), Mode::ConfirmDelete, &toggles, width)
                    }
                    _ if new_note.is_some() => {
                        let prompt = new_note.as_ref().expect("checked by the guard");
                        let lead = new_note_lead(&prompt.target.label, &prompt.buffer);
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
                        footer::line(TREE_KEYS, mode, &toggles, width)
                    }
                };
                frame.render_widget(Paragraph::new(status), rows[1]);

                if help.open {
                    help::render(frame, full, TREE_KEYS, &mut help);
                }
            })
            .context("failed to draw")?;

        let ev = event::read().context("failed to read event")?;

        if let Event::Mouse(mouse) = ev {
            match mouse.kind {
                MouseEventKind::ScrollDown => {
                    preview_scroll = preview_scroll.saturating_add(3);
                }
                MouseEventKind::ScrollUp => {
                    preview_scroll = preview_scroll.saturating_sub(3);
                }
                MouseEventKind::Down(MouseButton::Left) => {
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
                                if let Some(d) =
                                    mouse_x_to_dot(mouse.column, list_inner_area.x)
                                {
                                    nodes[real].flags ^= FLAG_DEFS[d as usize].bit;
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
                pre_focus_expanded =
                    remap_rows(&old_nodes, &forest.nodes, &pre_focus_expanded);
                status_message = Some(outcome.message);
                let visible = compute_visible(&forest.nodes, &search_buffer);
                let pos = outcome.row.and_then(|r| visible.iter().position(|&i| i == r));
                state.select(Some(pos.unwrap_or(0)));
            }
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
                    let title = std::mem::take(buffer);
                    rename_buffer = None;
                    let visible = compute_visible(nodes, &search_buffer);
                    let vs = state.selected().unwrap_or(0);
                    if let Some(&ri) = visible.get(vs) {
                        match rename::rename_note(&nodes[ri].path, &title) {
                            Ok(new_path) => {
                                nodes[ri].name = file_name_of(&new_path);
                                nodes[ri].path = new_path;
                            }
                            Err(e) => status_message = Some(format!("rename failed: {e}")),
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

        match key.code {
            KeyCode::Char('q') => break,
            KeyCode::Esc => {
                if !search_buffer.is_empty() {
                    search_buffer.clear();
                } else {
                    break;
                }
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
            KeyCode::Char('/') => {
                search_mode = true;
                search_buffer.clear();
                cursor_pos = 0;
            }
            KeyCode::Char('t') => {
                flag_mode = true;
            }
            KeyCode::Char('r') => {
                if selected < visible.len() && !nodes[real_idx].is_dir {
                    rename_buffer = Some(rename::editable_title(&nodes[real_idx].name));
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
                preview_scroll = preview_scroll.saturating_add(1);
            }
            KeyCode::Char('K') => {
                preview_scroll = preview_scroll.saturating_sub(1);
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
    use notez_core::tags::{FLAG_IMPORTANT, FLAG_PRIO};

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
        let expected = vec!["o", "t", "r", "n", "d", "/", "f", "v", "?", "q"];
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

    /// Width of the list pane's text in an 80-column list, the way the
    /// event loop derives it from the pane width.
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

    /// `line` drawn as the selected row of an 80-column list, the way the
    /// event loop draws it: the cells of that row.
    fn render_row(line: Line<'static>) -> Vec<(String, Option<Color>)> {
        use ratatui::buffer::Buffer;
        let area = Rect::new(0, 0, 80, 1);
        let mut buf = Buffer::empty(area);
        let list = List::new(vec![ListItem::new(line)]).highlight_symbol("  ▸ ");
        let mut state = ListState::default();
        state.select(Some(0));
        StatefulWidget::render(list, area, &mut buf, &mut state);
        (0..80)
            .map(|x| {
                let cell = &buf[(x, 0)];
                (cell.symbol().to_string(), cell.style().fg)
            })
            .collect()
    }

    /// Highlight symbol plus the tag dots: the badge sits in this column.
    const BADGE_COL: usize = 4 + 7;

    #[test]
    fn every_file_row_has_a_badge_in_its_scope_colour_and_the_rest_unchanged() {
        let (sections, nodes) = badged_forest();
        let cases = [
            ("/n/personal/proj/top.md", Scope::Personal.icon(), Scope::Personal, 1),
            ("/p/notez/plans/b.md", Scope::Public.icon(), Scope::Public, 2),
            ("/p/docs/design/c.md", "\u{f02d}", Scope::Public, 2),
            ("/p/.notez/d.md", Scope::Local.icon(), Scope::Local, 1),
            ("/n/e.md", Scope::Global.icon(), Scope::Global, 1),
        ];
        for (path, icon, colour_scope, depth) in cases {
            let node = &nodes[row(&nodes, path)];
            let cells = render_row(row_line(node, sections.get(node.section), LIST_TEXT_WIDTH));
            assert_eq!(cells[BADGE_COL].0, icon, "{path}");
            assert_eq!(cells[BADGE_COL].1, Some(theme::scope_color(colour_scope)), "{path}");
            let rest: String = cells[BADGE_COL + 1..].iter().map(|c| c.0.as_str()).collect();
            let expected = format!("{}│   {}", "  ".repeat(depth), node.name);
            assert_eq!(rest.trim_end(), expected, "{path}");
        }
    }

    #[test]
    fn folder_rows_have_the_badge_too() {
        let (sections, nodes) = badged_forest();
        let node = &nodes[row(&nodes, "/n/personal/proj/ideas")];
        let line = row_line(node, sections.get(node.section), LIST_TEXT_WIDTH);
        let cells = render_row(line);
        assert_eq!(cells[BADGE_COL].0, Scope::Personal.icon());
        assert_eq!(cells[BADGE_COL].1, Some(theme::scope_color(Scope::Personal)));
        let rest: String = cells[BADGE_COL + 1..].iter().map(|c| c.0.as_str()).collect();
        assert!(rest.starts_with("  ├─▼ ideas "), "{rest:?}");
        assert!(rest.trim_end().ends_with('1'), "{rest:?}");
    }

    #[test]
    fn a_section_header_shows_its_scope_word_in_the_scope_colour() {
        let (sections, nodes) = badged_forest();
        for (i, spec) in sections.iter().enumerate() {
            let node = nodes.iter().find(|n| n.depth == 0 && n.section == i).unwrap();
            let line = row_line(node, Some(spec), LIST_TEXT_WIDTH);
            let word = spec.scope.label();
            let colour = Some(theme::scope_color(spec.scope));
            let word_span = line.spans.iter().find(|s| s.content == word).expect(word);
            assert_eq!(word_span.style.fg, colour, "{word}");
            let icon_span =
                line.spans.iter().find(|s| s.content.starts_with(spec.icon)).expect("icon");
            assert_eq!(icon_span.style.fg, colour, "{word} icon");

            let cells = render_row(line);
            assert_eq!(cells[BADGE_COL].0, " ", "{word}: no second icon on the header");
            let rest: String = cells[BADGE_COL + 1..].iter().map(|c| c.0.as_str()).collect();
            let expected = format!("▼ {} {} {word} ", spec.icon, spec.label);
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
        let cells = render_row(row_line(&nodes[idx], sections.get(nodes[idx].section), LIST_TEXT_WIDTH));
        let lit = cells.iter().position(|c| c.0 == "●").expect("a lit dot");
        assert_eq!(mouse_x_to_dot(lit as u16, 0), Some(prio as u8));
        assert_eq!(mouse_x_to_dot(BADGE_COL as u16, 0), None, "the badge is no dot");
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
    fn d_on_a_folder_or_an_empty_tree_changes_nothing() {
        let sections = project_sections();
        let (nodes, _) = build_forest(&sections);
        for path in ["/n/personal/proj", "/n/personal/proj/ideas", "/p/docs/design", "/n"] {
            let request = delete_request(&nodes, &sections, Some(row(&nodes, path)), None);
            assert_eq!(request.err(), Some(FOLDER_DELETE_UNAVAILABLE), "{path}");
        }
        assert!(matches!(delete_request(&nodes, &sections, None, None), Ok(None)));
        assert!(matches!(delete_request(&nodes, &sections, Some(nodes.len()), None), Ok(None)));
        assert!(matches!(delete_request(&[], &[], Some(0), None), Ok(None)));
        assert!(matches!(delete_request(&[], &[], None, None), Ok(None)));
        assert!(FOLDER_DELETE_UNAVAILABLE.contains("folder delete is not available yet"));
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
}
