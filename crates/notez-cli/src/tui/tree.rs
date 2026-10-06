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

use super::{VimCommandMode, theme};
use crate::commands::rename;

/// What the title bar shows.
pub struct TreeContext {
    pub title: String,
    pub path_display: String,
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
    let mut show_help = false;
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
                let status = if let Some(buffer) = &rename_buffer {
                    Line::from(vec![
                        Span::styled(" rename: ", Style::default().fg(theme::MAUVE)),
                        Span::styled(buffer.clone(), Style::default().fg(theme::TEXT)),
                        Span::styled("_", Style::default().fg(theme::OVERLAY)),
                    ])
                } else if let Some(message) = &status_message {
                    Line::from(Span::styled(
                        format!(" {message}"),
                        Style::default().fg(theme::PEACH),
                    ))
                } else if vim.active {
                    Line::from(vec![Span::styled(
                        vim.buffer.clone(),
                        theme::command_line(),
                    )])
                } else if flag_mode {
                    let cur_flags = if real_idx < nodes.len() {
                        nodes[real_idx].flags
                    } else {
                        0
                    };
                    let mut spans =
                        vec![Span::styled(" tags: ", Style::default().fg(theme::MAUVE))];
                    for (idx, def) in FLAG_DEFS.iter().enumerate() {
                        let active = cur_flags & def.bit != 0;
                        let color = theme::FLAG_COLORS[idx];
                        spans.push(Span::styled(
                            format!("{}", idx + 1),
                            Style::default().fg(color),
                        ));
                        spans.push(Span::styled(":", Style::default().fg(theme::OVERLAY)));
                        spans.push(Span::styled(
                            format!("{} ", def.label),
                            Style::default().fg(if active { color } else { theme::OVERLAY }),
                        ));
                        spans.push(Span::raw(" "));
                    }
                    Line::from(spans)
                } else {
                    let bold = Modifier::BOLD;
                    let width = area.width as usize;
                    let left = " open  tags  rename  focus  view all";
                    let padding = width.saturating_sub(left.len() + 4);
                    Line::from(vec![
                        Span::raw(" "),
                        Span::styled("o", Style::default().fg(theme::GREEN).add_modifier(bold)),
                        Span::styled("pen  ", Style::default().fg(theme::OVERLAY)),
                        Span::styled("t", Style::default().fg(theme::PEACH).add_modifier(bold)),
                        Span::styled("ags  ", Style::default().fg(theme::OVERLAY)),
                        Span::styled("r", Style::default().fg(theme::MAUVE).add_modifier(bold)),
                        Span::styled("ename  ", Style::default().fg(theme::OVERLAY)),
                        Span::styled("f", Style::default().fg(theme::GREEN).add_modifier(bold)),
                        Span::styled("ocus  ", Style::default().fg(theme::OVERLAY)),
                        Span::styled(
                            "v",
                            Style::default().fg(theme::SAPPHIRE).add_modifier(bold),
                        ),
                        Span::styled("iew all", Style::default().fg(theme::OVERLAY)),
                        Span::raw(" ".repeat(padding)),
                        Span::styled("q", Style::default().fg(theme::PEACH).add_modifier(bold)),
                        Span::styled("uit ", Style::default().fg(theme::OVERLAY)),
                    ])
                };
                frame.render_widget(Paragraph::new(status), rows[1]);

                if show_help {
                    render_help(frame, full);
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

        if show_help {
            show_help = false;
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

        if let Some(cmd) = vim.handle_key(key) {
            if VimCommandMode::is_quit(&cmd) {
                break;
            }
            continue;
        }
        if vim.active {
            continue;
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
                let any_collapsed = nodes
                    .iter()
                    .any(|n| n.is_dir && n.depth == 0 && !n.expanded);
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
                show_help = true;
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

fn render_help(frame: &mut Frame, full: Rect) {
    let rows: [(&str, Color, &str); 12] = [
        ("o / enter", theme::GREEN, "open file / toggle dir"),
        ("l", theme::MAUVE, "expand directory"),
        ("h", theme::MAUVE, "collapse / go to parent"),
        ("f", theme::GREEN, "focus section"),
        ("/", theme::YELLOW, "filter: text + #tag, or click a dot"),
        ("t", theme::PEACH, "tag mode (1-5 toggle, t to close)"),
        ("r", theme::MAUVE, "rename note (enter confirms, esc cancels)"),
        ("v", theme::SAPPHIRE, "view all / collapse all"),
        ("j/k", theme::TEXT, "navigate"),
        ("J/K", theme::TEXT, "scroll preview"),
        (":q", theme::MAUVE, "vim-style quit"),
        ("q", theme::PEACH, "quit"),
    ];
    let mut help_text = vec![
        Line::from(Span::styled(
            "  keybindings",
            Style::default()
                .fg(theme::MAUVE)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];
    for (key, color, desc) in rows {
        help_text.push(Line::from(vec![
            Span::styled(format!("  {:<10}", key), Style::default().fg(color)),
            Span::styled(desc.to_string(), Style::default().fg(theme::TEXT)),
        ]));
    }
    help_text.push(Line::from(""));
    help_text.push(Line::from(Span::styled(
        "  press any key to close",
        Style::default().fg(theme::OVERLAY),
    )));

    let help_h = help_text.len() as u16 + 2;
    let help_w = 50_u16;
    let hx = full.x + (full.width.saturating_sub(help_w)) / 2;
    let hy = full.y + (full.height.saturating_sub(help_h)) / 2;
    let help_area = Rect::new(hx, hy, help_w, help_h);
    let help_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::SURFACE))
        .border_type(ratatui::widgets::BorderType::Rounded)
        .style(Style::default().bg(theme::BASE));
    frame.render_widget(ratatui::widgets::Clear, help_area);
    frame.render_widget(Paragraph::new(help_text).block(help_block), help_area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use notez_core::tags::{FLAG_IMPORTANT, FLAG_PRIO};

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
