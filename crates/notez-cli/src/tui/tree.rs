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
use notez_core::filter::{self, Filter};
use notez_core::note_tags;
use notez_core::tags::FLAG_DEFS;

use super::footer::{self, Group, KeyHint, Mode, QUIT_HINT_RESERVED_COLS, Slot, Toggle};
use super::help::{self, HelpState};
use super::{VimCommandMode, VimKey, theme};
use crate::commands::rename;

/// What the title bar shows.
pub struct TreeContext {
    pub title: String,
    pub path_display: String,
    /// Shown in the status bar for the whole session, whenever nothing
    /// transient is using the line.
    pub warning: Option<String>,
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
}

/// Run the browser to completion. Returns `(root, final_map)` pairs for
/// every tag root whose `.tags` content changed; the caller persists them.
/// A quit without tag edits returns an empty list (nothing gets written).
pub fn run_tree(
    sections: Vec<SectionSpec>,
    ctx: &TreeContext,
    config: &Config,
) -> Result<Vec<(PathBuf, HashMap<String, u8>)>> {
    let (mut nodes, tag_roots) = build_forest(&sections);
    let initial: Vec<HashMap<String, u8>> =
        tag_roots.iter().map(|r| note_tags::load_tags(r)).collect();
    apply_tags(&mut nodes, &tag_roots, &initial);

    let mut terminal = super::enter().context("failed to enter TUI")?;
    let result = event_loop(&mut terminal, &mut nodes, ctx, config);
    super::leave().context("failed to leave TUI")?;
    result?;

    Ok(changed_tag_maps(&nodes, &tag_roots, &initial))
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

    for spec in sections {
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
        });
        emit_children(&tmp, &spec.root, 1, wrapper_idx, root_idx, &mut nodes);
    }

    (nodes, tag_roots)
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

/// Final per-root tag maps: start from the loaded map (so keys this view
/// never showed survive untouched) and overlay every file node's current
/// flags. Returns only the roots whose map differs from the loaded one.
fn changed_tag_maps(
    nodes: &[TreeNode],
    tag_roots: &[PathBuf],
    initial: &[HashMap<String, u8>],
) -> Vec<(PathBuf, HashMap<String, u8>)> {
    let mut finals: Vec<HashMap<String, u8>> = initial.to_vec();
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

// --- Keys: one table for the footer and the help overlay ---

const BROWSE: &[Mode] = &[Mode::Normal, Mode::Focus];
const BROWSE_AND_TAG: &[Mode] = &[Mode::Normal, Mode::Focus, Mode::Tag];
const FILTERING: &[Mode] = &[Mode::Filter];
const TAGGING: &[Mode] = &[Mode::Tag];
const RENAMING: &[Mode] = &[Mode::Rename];
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
    nodes: &mut Vec<TreeNode>,
    ctx: &TreeContext,
    config: &Config,
) -> Result<()> {
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

                        let mut spans = flags_slots(node.flags);
                        spans.push(Span::styled(
                            format!("{}{}", indent, icon),
                            Style::default().fg(theme::SURFACE),
                        ));
                        if !node.scope_icon.is_empty() {
                            spans.push(Span::styled(
                                format!("{} ", node.scope_icon),
                                Style::default().fg(theme::OVERLAY),
                            ));
                        }
                        if node.is_dir {
                            spans.push(Span::styled(
                                node.name.clone(),
                                Style::default().fg(theme::SAPPHIRE),
                            ));
                            if node.child_count > 0 {
                                let count_str = format!("{}", node.child_count);
                                let scope_len =
                                    if node.scope_icon.is_empty() { 0 } else { 2 };
                                let prefix_len = 7
                                    + indent.len()
                                    + icon.len()
                                    + node.name.chars().count()
                                    + scope_len;
                                let avail = inner_width
                                    .saturating_sub(prefix_len + count_str.len() + 2);
                                if avail > 3 {
                                    spans.push(Span::styled(
                                        format!(" {} ", "·".repeat(avail)),
                                        Style::default().fg(theme::SURFACE),
                                    ));
                                } else {
                                    spans.push(Span::raw(" "));
                                }
                                spans.push(Span::styled(
                                    count_str,
                                    Style::default().fg(theme::OVERLAY),
                                ));
                            }
                        } else {
                            spans.push(Span::styled(
                                node.name.clone(),
                                Style::default().fg(theme::TEXT),
                            ));
                        }
                        ListItem::new(Line::from(spans))
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

                let list = List::new(items)
                    .highlight_style(theme::selected())
                    .highlight_symbol("  ▸ ");
                frame.render_stateful_widget(list, inner_chunks[2], &mut state);

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
        let expected = vec!["o", "t", "r", "/", "f", "v", "?", "q"];
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
        assert_eq!(shown_keys(Mode::Normal, &[], 59), vec!["o", "t", "r", "/", "f", "v", "?", "q"]);
        assert_eq!(shown_keys(Mode::Normal, &[], 58), vec!["o", "t", "/", "f", "v", "?", "q"]);
        assert_eq!(shown_keys(Mode::Normal, &[], 50), vec!["o", "t", "/", "f", "?", "q"]);
        assert_eq!(shown_keys(Mode::Normal, &[], 40), vec!["o", "t", "f", "?", "q"]);
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
}
