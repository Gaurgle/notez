//! The todoz board TUI, ported from notez-cli.
//!
//! Only the terminal layer lives here: rendering, key and mouse handling,
//! filter state, and the help overlay. All board semantics (parsing,
//! hierarchy, drag rules, mutation, saving) come from [`notez_core::todo`],
//! and filter parsing from [`notez_core::filter`]. The caller assembles the
//! board (see `commands::todo`) and persists only the sources reported
//! dirty in the returned [`BoardOutcome`]; files the user never touched are
//! never rewritten (rewriting drops any non-todo text in them).

use std::collections::HashSet;
use std::path::PathBuf;

use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, MouseButton, MouseEventKind};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Padding, Paragraph};

use notez_core::config::{Config, ProjectRegistry};
use notez_core::filter::{self, Filter};
use notez_core::tags::FLAG_DEFS;
use notez_core::todo::{self, CheckState, Task};
use notez_core::util::tilde;

use super::{VimCommandMode, theme};

/// What the title bar shows and which global-only features are enabled.
pub struct BoardContext {
    /// Global board: category creation (`N`) and reload-after-create work.
    pub global: bool,
    pub title: String,
    pub path_display: String,
    /// Shown in the footer's warning slot for the whole session, ahead of
    /// the prose warning when both apply.
    pub warning: Option<String>,
}

/// The edited board plus the source files whose persisted state actually
/// changed. Only those files should be written back.
pub struct BoardOutcome {
    pub items: Vec<Task>,
    pub dirty: HashSet<PathBuf>,
}

/// Run the board TUI to completion. The caller persists the outcome's dirty
/// sources; this function never writes TODO.md itself except when creating
/// a new category file (global board only).
pub fn run_board(
    items: Vec<Task>,
    ctx: &BoardContext,
    config: &Config,
) -> Result<BoardOutcome> {
    let prose_sources = detect_prose_sources(&items);
    let mut terminal = super::enter().context("failed to enter TUI")?;
    let result = event_loop(&mut terminal, items, ctx, config, &prose_sources);
    super::leave().context("failed to leave TUI")?;
    result
}

/// Source files whose current on-disk content contains lines the parser
/// does not model (prose, extra headers). Saving over them is lossy, so
/// the TUI warns when one of them becomes dirty.
fn detect_prose_sources(items: &[Task]) -> HashSet<PathBuf> {
    let mut checked: HashSet<PathBuf> = HashSet::new();
    let mut prose: HashSet<PathBuf> = HashSet::new();
    for item in items {
        if item.is_code_todo || !checked.insert(item.source.clone()) {
            continue;
        }
        if let Ok(content) = std::fs::read_to_string(&item.source) {
            if todo::has_non_todo_content(&content) {
                prose.insert(item.source.clone());
            }
        }
    }
    prose
}

/// Section names (header labels) of dirty files that carry non-todo text,
/// for the footer warning. Falls back to the file path when a source has
/// no header row.
fn prose_warning_sections(
    items: &[Task],
    dirty: &HashSet<PathBuf>,
    prose_sources: &HashSet<PathBuf>,
) -> Vec<String> {
    let mut out = Vec::new();
    for src in dirty.intersection(prose_sources) {
        let label = items
            .iter()
            .find(|i| i.is_header && !i.is_code_todo && &i.source == src)
            .map(|h| h.text.clone())
            .unwrap_or_else(|| tilde::contract(src));
        if !out.contains(&label) {
            out.push(label);
        }
    }
    out.sort();
    out
}

/// The footer's warning text: the session warning (a stopped pull) first,
/// then the prose loss warning for `prose_sections`. When both apply they
/// share the line, so neither hides the other.
fn footer_warning(session: Option<&str>, prose_sections: &[String]) -> Option<String> {
    let prose = (!prose_sections.is_empty()).then(|| {
        format!(
            "non-todo text in {} will be dropped on save",
            prose_sections.join(", ")
        )
    });
    match (session, prose) {
        (Some(session), Some(prose)) => Some(format!("{session}; {prose}")),
        (Some(session), None) => Some(session.to_string()),
        (None, prose) => prose,
    }
}

/// Filter-aware visible indices: collapse-aware order, then the filter's
/// keep-mask applied on top. Must stay in sync with the render pass so
/// keyboard navigation always matches what is on screen.
fn compute_visible(items: &[Task], search_buffer: &str) -> Vec<usize> {
    let f = filter::parse(search_buffer);
    let mut v = todo::get_visible_indices(items);
    if f.is_empty() {
        return v;
    }
    let keep = compute_filter_keep(items, &f);
    v.retain(|&i| keep[i]);
    v
}

/// Per-item keep mask: an item is kept when it matches directly or any
/// descendant matches, and every match pulls in its ancestor chain (parent
/// todos and the section header) so results keep their context.
fn compute_filter_keep(items: &[Task], f: &Filter) -> Vec<bool> {
    let n = items.len();
    let mut keep = vec![false; n];
    for (i, item) in items.iter().enumerate() {
        if !item.is_header && f.matches(&item.text, item.flags) {
            keep[i] = true;
        }
    }
    for i in 0..n {
        if !keep[i] {
            continue;
        }
        let mut needed_depth = items[i].depth;
        let mut j = i;
        while j > 0 {
            j -= 1;
            if items[j].is_header {
                keep[j] = true;
                break;
            }
            if items[j].depth < needed_depth {
                keep[j] = true;
                needed_depth = items[j].depth;
                if needed_depth == 0 {
                    for k in (0..j).rev() {
                        if items[k].is_header {
                            keep[k] = true;
                            break;
                        }
                    }
                    break;
                }
            }
        }
    }
    keep
}

/// Map a mouse column onto a tag-dot index (0..=4) on a list row. Rows
/// start with the 4-column highlight symbol plus 1 flag-leading space, so
/// dot 0 sits at `area_x + 5` and the dots are contiguous.
fn mouse_x_to_dot(mouse_col: u16, area_x: u16) -> Option<u8> {
    let dot_start = area_x.saturating_add(5);
    let dot_end = dot_start + 4;
    if mouse_col >= dot_start && mouse_col <= dot_end {
        Some((mouse_col - dot_start) as u8)
    } else {
        None
    }
}

/// Map a mouse row onto the real item index under it, accounting for the
/// list scroll offset and wrapped rows.
fn mouse_y_to_real_idx(
    mouse_row: u16,
    list_area: Rect,
    state_offset: usize,
    visible: &[usize],
    row_counts: &[u16],
) -> Option<usize> {
    if mouse_row < list_area.y || mouse_row >= list_area.y.saturating_add(list_area.height) {
        return None;
    }
    let mut list_row = (mouse_row - list_area.y) as usize;
    for vis_idx in state_offset..visible.len() {
        let rows = row_counts.get(vis_idx).copied().unwrap_or(1) as usize;
        if list_row < rows {
            return Some(visible[vis_idx]);
        }
        list_row -= rows;
    }
    None
}

/// Render the 5 fixed tag-dot slots (leading + trailing space included).
/// `hover_dot` previews an unset slot in its tag color while the mouse
/// hovers over it.
fn flags_slots_with_hover(flags: u8, hover_dot: Option<u8>) -> Vec<Span<'static>> {
    let mut spans: Vec<Span<'static>> = vec![Span::raw(" ")];
    for (i, def) in FLAG_DEFS.iter().enumerate() {
        let is_set = flags & def.bit != 0;
        let is_hovered = hover_dot == Some(i as u8);
        if is_set {
            spans.push(Span::styled(
                "●",
                Style::default().fg(theme::FLAG_COLORS[i]),
            ));
        } else if is_hovered {
            spans.push(Span::styled(
                "●",
                Style::default()
                    .fg(theme::FLAG_COLORS[i])
                    .add_modifier(Modifier::DIM),
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

fn flags_slots(flags: u8) -> Vec<Span<'static>> {
    flags_slots_with_hover(flags, None)
}

#[allow(clippy::too_many_lines)]
fn event_loop(
    terminal: &mut super::TuiTerminal,
    mut items: Vec<Task>,
    ctx: &BoardContext,
    config: &Config,
    prose_sources: &HashSet<PathBuf>,
) -> Result<BoardOutcome> {
    use super::text::{next_char_boundary, prev_char_boundary};

    // Source files whose persisted state changed; the only ones saved.
    let mut dirty: HashSet<PathBuf> = HashSet::new();

    let mut state = ListState::default();
    if !items.is_empty() {
        state.select(Some(0));
    }
    let mut vim = VimCommandMode::new();
    let mut input_mode = false;
    let mut subtask_mode = false;
    let mut edit_mode = false;
    let mut edit_idx: usize = 0;
    let mut input_buffer = String::new();
    let mut flag_mode = false;
    let mut search_mode = false;
    let mut search_buffer = String::new();
    let mut cursor_pos: usize = 0;
    let mut confirm_delete = false;
    let mut focus_active = false;
    let mut pre_focus_collapsed: Vec<(usize, bool)> = Vec::new();
    let mut show_help = false;
    let mut category_mode = false;
    let mut category_error: Option<String> = None;

    // Mouse-drag reorder state: "click candidate" is set on Down, "drag
    // active" once a Drag event fires. Up with no drag is a plain click.
    let mut drag_candidate: Option<usize> = None;
    let mut drag_active: bool = false;
    let mut drag_start: Option<usize> = None;
    let mut drag_target: Option<usize> = None;
    let mut list_area: Rect = Rect::default();
    let mut filter_strip_area: Rect = Rect::default();
    let mut visible_for_mouse: Vec<usize> = Vec::new();
    let mut row_counts_for_mouse: Vec<u16> = Vec::new();
    // Tag dot under the cursor (real_idx, dot) driving the hover preview.
    let mut hover_flag: Option<(usize, u8)> = None;
    // Previous filter buffer; on change, sections/parents containing a match
    // auto-expand so results are not hidden behind collapsed rows.
    let mut prev_filter = String::new();

    loop {
        todo::derive_parent_states(&mut items);
        todo::derive_header_flags(&mut items);

        terminal
            .draw(|frame| {
                let full = frame.area();
                let area = Rect::new(
                    full.x + 2,
                    full.y + 1,
                    full.width.saturating_sub(4),
                    full.height.saturating_sub(2),
                );

                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(1), Constraint::Length(1)])
                    .split(area);

                let cur_filter = filter::parse(&search_buffer);
                let filter_changed = search_buffer != prev_filter;
                if !cur_filter.is_empty() && filter_changed {
                    let keep = compute_filter_keep(&items, &cur_filter);
                    for (i, k) in keep.iter().enumerate() {
                        if *k && (items[i].is_header || items[i].has_subtasks) {
                            items[i].collapsed = false;
                        }
                    }
                }
                prev_filter = search_buffer.clone();

                let visible: Vec<usize> = compute_visible(&items, &search_buffer);
                visible_for_mouse = visible.clone();

                let mut list_items: Vec<ListItem> = visible
                    .iter()
                    .map(|&idx| {
                        let item = &items[idx];
                        if item.is_header {
                            let path_display = item
                                .source
                                .canonicalize()
                                .unwrap_or_else(|_| item.source.clone())
                                .parent()
                                .map(|p| tilde::contract(p))
                                .unwrap_or_default();
                            let header_color = if item.is_code_todo {
                                theme::YELLOW
                            } else {
                                theme::MAUVE
                            };
                            let collapse_icon = if item.collapsed { "▶ " } else { "▼ " };
                            let mut spans = Vec::new();
                            spans.extend(flags_slots(item.flags));
                            spans.push(Span::styled(
                                collapse_icon,
                                Style::default().fg(theme::SURFACE),
                            ));
                            spans.push(Span::styled(
                                format!("{} ", item.text),
                                Style::default()
                                    .fg(header_color)
                                    .add_modifier(Modifier::BOLD),
                            ));
                            spans.push(Span::styled(
                                path_display,
                                Style::default().fg(theme::OVERLAY),
                            ));
                            ListItem::new(Line::from(spans))
                        } else if item.is_code_todo {
                            let mut spans = Vec::new();
                            spans.extend(flags_slots(0));
                            spans.push(Span::styled("   ", Style::default()));
                            spans.push(Span::styled(
                                item.text.clone(),
                                Style::default().fg(theme::OVERLAY),
                            ));
                            ListItem::new(Line::from(spans))
                        } else {
                            let (indent, collapse_icon) = if item.has_subtasks {
                                let pad = format!(" {}", "  ".repeat(item.depth as usize));
                                let icon = if item.collapsed { "▶ " } else { "▼ " };
                                (pad, icon)
                            } else {
                                let pad =
                                    format!(" {}", "  ".repeat(item.depth as usize + 1));
                                (pad, "")
                            };
                            let (mark, mark_color, bracket_color, style) = match item.state {
                                CheckState::Checked => (
                                    "x",
                                    theme::SAPPHIRE,
                                    theme::OVERLAY,
                                    Style::default().fg(theme::OVERLAY),
                                ),
                                CheckState::Half => (
                                    "/",
                                    theme::YELLOW,
                                    theme::OVERLAY,
                                    Style::default().fg(theme::SUBTEXT),
                                ),
                                CheckState::Unchecked => (
                                    " ",
                                    theme::SURFACE,
                                    match item.depth {
                                        0 => theme::SAPPHIRE,
                                        1 => Color::Rgb(86, 169, 206),
                                        _ => Color::Rgb(56, 139, 176),
                                    },
                                    Style::default().fg(theme::TEXT),
                                ),
                            };
                            let checkbox_spans = vec![
                                Span::styled("[", Style::default().fg(bracket_color)),
                                Span::styled(mark, Style::default().fg(mark_color)),
                                Span::styled("] ", Style::default().fg(bracket_color)),
                            ];
                            let prefix_len = indent.len() + collapse_icon.len() + 7 + 4;
                            let text_width =
                                (area.width as usize).saturating_sub(prefix_len + 8);

                            let hover_dot = hover_flag
                                .and_then(|(hi, d)| if hi == idx { Some(d) } else { None });

                            if text_width > 0 && item.text.chars().count() > text_width {
                                // Wrap on char boundaries; slicing mid-char
                                // (å, ö, icons) would panic on tiny widths.
                                let mut lines = vec![];
                                let mut remaining = item.text.as_str();
                                let mut first = true;
                                while !remaining.is_empty() {
                                    let split_at = remaining
                                        .char_indices()
                                        .nth(text_width)
                                        .map(|(i, _)| i)
                                        .unwrap_or(remaining.len());
                                    let split_at = if split_at < remaining.len() {
                                        remaining[..split_at]
                                            .rfind(' ')
                                            .map(|i| i + 1)
                                            .unwrap_or(split_at)
                                    } else {
                                        split_at
                                    };
                                    let (chunk, rest) = remaining.split_at(split_at);
                                    let rest = rest.trim_start();

                                    if first {
                                        let mut spans = Vec::new();
                                        spans.extend(flags_slots_with_hover(
                                            item.flags, hover_dot,
                                        ));
                                        spans.push(Span::styled(
                                            indent.clone(),
                                            Style::default(),
                                        ));
                                        spans.push(Span::styled(
                                            collapse_icon,
                                            Style::default().fg(theme::SURFACE),
                                        ));
                                        spans.extend(checkbox_spans.clone());
                                        spans.push(Span::styled(chunk.to_string(), style));
                                        lines.push(Line::from(spans));
                                        first = false;
                                    } else {
                                        let wrap_indent = " ".repeat(prefix_len);
                                        lines.push(Line::from(vec![
                                            Span::styled(wrap_indent, Style::default()),
                                            Span::styled(chunk.to_string(), style),
                                        ]));
                                    }
                                    remaining = rest;
                                }
                                ListItem::new(lines)
                            } else {
                                let mut spans = Vec::new();
                                spans.extend(flags_slots_with_hover(item.flags, hover_dot));
                                spans.push(Span::styled(indent, Style::default()));
                                spans.push(Span::styled(
                                    collapse_icon,
                                    Style::default().fg(theme::SURFACE),
                                ));
                                spans.extend(checkbox_spans);
                                spans.push(Span::styled(item.text.clone(), style));
                                ListItem::new(Line::from(spans))
                            }
                        }
                    })
                    .collect();

                row_counts_for_mouse =
                    list_items.iter().map(|li| li.height() as u16).collect();

                // Drag visualization: dragged row dim, drop target brighter.
                let style_at =
                    |list_items: &mut Vec<ListItem>, real_idx: usize, style: Style| {
                        if let Some(pos) = visible.iter().position(|&i| i == real_idx) {
                            let placeholder = ListItem::new("");
                            let original =
                                std::mem::replace(&mut list_items[pos], placeholder);
                            list_items[pos] = original.style(style);
                        }
                    };
                if let Some(start) = drag_start {
                    style_at(&mut list_items, start, Style::default().bg(theme::SURFACE));
                }
                if let Some(target) = drag_target {
                    if drag_start != Some(target) {
                        style_at(&mut list_items, target, Style::default().bg(theme::OVERLAY));
                    }
                }

                let todo_count = items
                    .iter()
                    .filter(|i| {
                        !i.is_header
                            && i.depth == 0
                            && !i.is_code_todo
                            && i.state != CheckState::Checked
                    })
                    .count();
                let done_count = items
                    .iter()
                    .filter(|i| {
                        !i.is_header
                            && i.depth == 0
                            && !i.is_code_todo
                            && i.state == CheckState::Checked
                    })
                    .count();

                let title = Line::from(vec![
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
                    Span::styled("- ", Style::default().fg(theme::SURFACE)),
                    Span::styled(
                        format!("{} pending", todo_count),
                        Style::default().fg(theme::SAPPHIRE),
                    ),
                    Span::styled(" · ", Style::default().fg(theme::SURFACE)),
                    Span::styled(
                        format!("{} done ", done_count),
                        Style::default().fg(theme::GREEN),
                    ),
                ]);

                // Filter strip: 5 tag dots (lit when in the active filter)
                // followed by the search input or hint. The dots align with
                // the dot column on todo rows.
                let active_tags = filter::active_tag_bits(&search_buffer);
                let mut filter_spans: Vec<Span> = Vec::new();
                filter_spans.push(Span::raw("     "));
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
                    .title(title)
                    .borders(Borders::ALL)
                    .border_style(theme::border())
                    .border_type(ratatui::widgets::BorderType::Rounded)
                    .padding(Padding::new(1, 1, 1, 0));

                // Inner area: [filter strip, divider, list].
                let inner = block.inner(chunks[0]);
                let inner_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(1),
                        Constraint::Length(1),
                        Constraint::Min(1),
                    ])
                    .split(inner);
                filter_strip_area = inner_chunks[0];
                let divider_rect = inner_chunks[1];
                list_area = inner_chunks[2];

                frame.render_widget(block, chunks[0]);
                frame.render_widget(
                    Paragraph::new(Line::from(filter_spans)),
                    filter_strip_area,
                );
                let divider_line = Line::from(Span::styled(
                    "─".repeat(divider_rect.width as usize),
                    Style::default().fg(Color::Rgb(50, 50, 65)),
                ));
                frame.render_widget(Paragraph::new(divider_line), divider_rect);

                let list = List::new(list_items)
                    .highlight_style(theme::selected())
                    .highlight_symbol("  ▸ ");
                frame.render_stateful_widget(list, list_area, &mut state);

                // Status bar.
                let bold = Modifier::BOLD;
                let status = if confirm_delete {
                    Line::from(vec![
                        Span::styled(
                            " delete this todo? ",
                            Style::default().fg(theme::TEXT),
                        ),
                        Span::styled("y", Style::default().fg(theme::RED).add_modifier(bold)),
                        Span::styled("es  ", Style::default().fg(theme::OVERLAY)),
                        Span::styled(
                            "n",
                            Style::default().fg(theme::SAPPHIRE).add_modifier(bold),
                        ),
                        Span::styled("o", Style::default().fg(theme::OVERLAY)),
                    ])
                } else if flag_mode {
                    let vis = compute_visible(&items, &search_buffer);
                    let vs = state.selected().unwrap_or(0);
                    let ri = vis.get(vs).copied().unwrap_or(0);
                    let cur_flags = if ri < items.len() { items[ri].flags } else { 0 };
                    let mut spans = vec![Span::styled(
                        " tags: ",
                        Style::default().fg(Color::Rgb(205, 152, 115)),
                    )];
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
                        spans.push(Span::styled(" ", Style::default()));
                    }
                    Line::from(spans)
                } else if input_mode || subtask_mode || edit_mode || category_mode {
                    let (label, label_color) = if edit_mode {
                        (" edit: ", Color::Rgb(165, 133, 202))
                    } else if subtask_mode {
                        (" subtask: ", Color::Rgb(148, 157, 210))
                    } else if category_mode {
                        (" new category: ", Color::Rgb(136, 190, 132))
                    } else {
                        (" new: ", Color::Rgb(136, 190, 132))
                    };
                    let (before, after) =
                        input_buffer.split_at(cursor_pos.min(input_buffer.len()));
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
                    let mut spans = vec![
                        Span::styled(label, Style::default().fg(label_color)),
                        Span::styled(before.to_string(), Style::default().fg(theme::TEXT)),
                        Span::styled(
                            cursor_char,
                            Style::default().fg(theme::BASE).bg(theme::SAPPHIRE),
                        ),
                        Span::styled(rest.to_string(), Style::default().fg(theme::TEXT)),
                    ];
                    if let Some(err) = &category_error {
                        spans.push(Span::styled(
                            format!("  <- {}", err),
                            Style::default().fg(theme::RED),
                        ));
                    }
                    Line::from(spans)
                } else if vim.active {
                    Line::from(vec![Span::styled(
                        vim.buffer.as_str(),
                        Style::default().fg(theme::MAUVE),
                    )])
                } else {
                    let width = chunks[1].width as usize;
                    let list_height = chunks[0].height.saturating_sub(4) as usize;
                    let scroll_info = if visible.len() > list_height {
                        format!(" {}/{} ", state.selected().unwrap_or(0) + 1, visible.len())
                    } else {
                        String::new()
                    };
                    // A dirty file with non-todo text loses that text on
                    // save; keep the warning up so the loss is never silent.
                    let prose_warn =
                        prose_warning_sections(&items, &dirty, prose_sources);
                    let warning = footer_warning(ctx.warning.as_deref(), &prose_warn);
                    let left = match warning {
                        None => Vec::new(),
                        Some(text) => vec![
                            Span::styled(" ! ", Style::default().fg(theme::RED).add_modifier(bold)),
                            Span::styled(text, Style::default().fg(theme::YELLOW)),
                        ],
                    };
                    let left_len = if left.is_empty() {
                        " ? help".len()
                    } else {
                        left.iter().map(|s| s.content.chars().count()).sum()
                    };
                    let right_len = " q quit ".len() + scroll_info.len();
                    let padding = width.saturating_sub(left_len + right_len);
                    let mut spans = if left.is_empty() {
                        vec![
                            Span::styled(" ", Style::default()),
                            Span::styled(
                                "?",
                                Style::default().fg(theme::YELLOW).add_modifier(bold),
                            ),
                            Span::styled(" help", Style::default().fg(theme::OVERLAY)),
                        ]
                    } else {
                        left
                    };
                    spans.push(Span::styled(" ".repeat(padding), Style::default()));
                    spans.push(Span::styled(
                        scroll_info,
                        Style::default().fg(theme::OVERLAY),
                    ));
                    spans.push(Span::styled(" ", Style::default()));
                    spans.push(Span::styled(
                        "q",
                        Style::default().fg(theme::PEACH).add_modifier(bold),
                    ));
                    spans.push(Span::styled(" quit ", Style::default().fg(theme::OVERLAY)));
                    Line::from(spans)
                };
                frame.render_widget(Paragraph::new(status), chunks[1]);

                if show_help {
                    render_help(frame, full);
                }
            })
            .context("failed to draw")?;

        let ev = event::read().context("failed to read event")?;

        // Mouse: scroll, dot clicks, click-to-collapse, drag-to-reorder.
        if let Event::Mouse(mouse) = ev {
            let vis_sel = state.selected().unwrap_or(0);
            match mouse.kind {
                MouseEventKind::ScrollDown => {
                    if vis_sel + 1 < visible_for_mouse.len() {
                        state.select(Some(vis_sel + 1));
                    }
                }
                MouseEventKind::ScrollUp => {
                    if vis_sel > 0 {
                        state.select(Some(vis_sel - 1));
                    }
                }
                MouseEventKind::Down(MouseButton::Left) => {
                    if mouse.row == filter_strip_area.y {
                        if let Some(d) = mouse_x_to_dot(mouse.column, filter_strip_area.x) {
                            search_buffer =
                                filter::toggle_tag_in_buffer(&search_buffer, d as usize);
                            cursor_pos = search_buffer.len();
                            continue;
                        }
                        search_mode = true;
                        cursor_pos = search_buffer.len();
                        continue;
                    }
                    if let Some(real_idx) = mouse_y_to_real_idx(
                        mouse.row,
                        list_area,
                        state.offset(),
                        &visible_for_mouse,
                        &row_counts_for_mouse,
                    ) {
                        if let Some(pos) =
                            visible_for_mouse.iter().position(|&i| i == real_idx)
                        {
                            state.select(Some(pos));
                        }
                        let it = &items[real_idx];
                        if let Some(d) = mouse_x_to_dot(mouse.column, list_area.x) {
                            if !it.is_header && !it.is_code_todo {
                                let flags = items[real_idx].flags ^ FLAG_DEFS[d as usize].bit;
                                dirty.insert(items[real_idx].source.clone());
                                todo::set_flags(&mut items, real_idx, flags);
                                continue;
                            }
                        }
                        drag_candidate = Some(real_idx);
                        drag_active = false;
                        drag_start = None;
                        drag_target = None;
                    }
                }
                MouseEventKind::Moved => {
                    // Hover preview only while not dragging, so the drag
                    // highlight stays clean.
                    if drag_candidate.is_none() {
                        let new_hover = mouse_y_to_real_idx(
                            mouse.row,
                            list_area,
                            state.offset(),
                            &visible_for_mouse,
                            &row_counts_for_mouse,
                        )
                        .and_then(|idx| {
                            let it = &items[idx];
                            if it.is_header || it.is_code_todo {
                                return None;
                            }
                            mouse_x_to_dot(mouse.column, list_area.x).map(|d| (idx, d))
                        });
                        if new_hover != hover_flag {
                            hover_flag = new_hover;
                        }
                    }
                }
                MouseEventKind::Drag(MouseButton::Left) => {
                    if let Some(start) = drag_candidate {
                        if !items[start].is_header && !items[start].is_code_todo {
                            drag_active = true;
                            drag_start = Some(start);
                            if let Some(real_idx) = mouse_y_to_real_idx(
                                mouse.row,
                                list_area,
                                state.offset(),
                                &visible_for_mouse,
                                &row_counts_for_mouse,
                            ) {
                                drag_target = Some(real_idx);
                            }
                        }
                    }
                }
                MouseEventKind::Up(MouseButton::Left) => {
                    if drag_active {
                        if let (Some(start), Some(target)) = (drag_start, drag_target) {
                            if start != target && todo::can_drag(&items, start, target) {
                                dirty.insert(items[start].source.clone());
                                let new_start =
                                    todo::perform_drag_move(&mut items, start, target);
                                let new_vis = compute_visible(&items, &search_buffer);
                                if let Some(pos) =
                                    new_vis.iter().position(|&i| i == new_start)
                                {
                                    state.select(Some(pos));
                                }
                            }
                        }
                    } else if let Some(real_idx) = drag_candidate {
                        // Plain click: toggle collapse on headers and parents.
                        if items[real_idx].is_header || items[real_idx].has_subtasks {
                            items[real_idx].collapsed = !items[real_idx].collapsed;
                        }
                    }
                    drag_candidate = None;
                    drag_active = false;
                    drag_start = None;
                    drag_target = None;
                }
                _ => {}
            }
            continue;
        }

        let Event::Key(key) = ev else { continue };
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

        // Help overlay: any key closes it.
        if show_help {
            show_help = false;
            continue;
        }

        if confirm_delete {
            if matches!(key.code, KeyCode::Char('y') | KeyCode::Enter) {
                let vis = compute_visible(&items, &search_buffer);
                let vs = state.selected().unwrap_or(0);
                let ri = vis.get(vs).copied().unwrap_or(0);
                if ri < items.len() && !items[ri].is_header {
                    dirty.insert(items[ri].source.clone());
                    todo::remove_task(&mut items, ri);
                    let new_vis = compute_visible(&items, &search_buffer);
                    if vs >= new_vis.len() && !new_vis.is_empty() {
                        state.select(Some(new_vis.len() - 1));
                    }
                }
            }
            confirm_delete = false;
            continue;
        }

        // Flag mode stays open until `t` or Esc so several tasks can be
        // tagged in one go (j/k navigation passes through).
        if flag_mode {
            let mut consumed = true;
            match key.code {
                KeyCode::Char(c @ '1'..='5') => {
                    let idx = (c as u8 - b'1') as usize;
                    let vis = compute_visible(&items, &search_buffer);
                    let vs = state.selected().unwrap_or(0);
                    let ri = vis.get(vs).copied().unwrap_or(0);
                    if ri < items.len() && !items[ri].is_header && !items[ri].is_code_todo {
                        let flags = items[ri].flags ^ FLAG_DEFS[idx].bit;
                        dirty.insert(items[ri].source.clone());
                        todo::set_flags(&mut items, ri, flags);
                    }
                }
                KeyCode::Char('t') | KeyCode::Esc => {
                    flag_mode = false;
                }
                KeyCode::Char('/') => {
                    flag_mode = false;
                    consumed = false;
                }
                KeyCode::Char('j')
                | KeyCode::Down
                | KeyCode::Char('k')
                | KeyCode::Up
                | KeyCode::Char('h')
                | KeyCode::Left
                | KeyCode::Char('l')
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
                    cursor_pos = prev_char_boundary(&search_buffer, cursor_pos);
                }
                KeyCode::Right => {
                    cursor_pos = next_char_boundary(&search_buffer, cursor_pos);
                }
                KeyCode::Backspace => {
                    if cursor_pos > 0 {
                        let prev = prev_char_boundary(&search_buffer, cursor_pos);
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

        // New top-level category prompt (global board only).
        if category_mode {
            match key.code {
                KeyCode::Enter => {
                    let name = input_buffer.trim().to_string();
                    if name.is_empty() {
                        category_error = Some("name cannot be empty".into());
                    } else if name.contains('/') || name.contains('\\') {
                        category_error = Some("name cannot contain '/' or '\\'".into());
                    } else {
                        let cat_dir = config.notez_root_path().join("_todos").join(&name);
                        if cat_dir.exists() {
                            category_error =
                                Some(format!("category '{}' already exists", name));
                        } else {
                            std::fs::create_dir_all(&cat_dir).ok();
                            std::fs::write(cat_dir.join("TODO.md"), "# TODO\n\n").ok();
                            // Persist the dirty in-memory edits, then reload
                            // so the new category appears in its slot. After
                            // the reload memory matches disk again.
                            todo::save_todos_for(&items, &dirty).ok();
                            dirty.clear();
                            let registry = ProjectRegistry::load().unwrap_or_default();
                            items = todo::load_board(config, &registry);
                            input_buffer.clear();
                            cursor_pos = 0;
                            category_mode = false;
                            category_error = None;
                            if let Some(real_idx) = items
                                .iter()
                                .position(|i| i.is_header && i.section == name)
                            {
                                let new_vis = compute_visible(&items, &search_buffer);
                                if let Some(pos) =
                                    new_vis.iter().position(|&i| i == real_idx)
                                {
                                    state.select(Some(pos));
                                }
                            }
                        }
                    }
                }
                KeyCode::Esc => {
                    input_buffer.clear();
                    cursor_pos = 0;
                    category_mode = false;
                    category_error = None;
                }
                KeyCode::Left => {
                    cursor_pos = prev_char_boundary(&input_buffer, cursor_pos);
                }
                KeyCode::Right => {
                    cursor_pos = next_char_boundary(&input_buffer, cursor_pos);
                }
                KeyCode::Backspace => {
                    if cursor_pos > 0 {
                        let prev = prev_char_boundary(&input_buffer, cursor_pos);
                        input_buffer.remove(prev);
                        cursor_pos = prev;
                    }
                }
                KeyCode::Char(c) => {
                    input_buffer.insert(cursor_pos, c);
                    cursor_pos += c.len_utf8();
                    category_error = None;
                }
                _ => {}
            }
            continue;
        }

        // New-todo prompt: on Enter the task lands at the end of the
        // selected item's section.
        if input_mode {
            match key.code {
                KeyCode::Enter => {
                    if !input_buffer.is_empty() {
                        let vis = compute_visible(&items, &search_buffer);
                        let vs = state.selected().unwrap_or(0);
                        let ri = vis.get(vs).copied().unwrap_or(0);
                        let mut insert_at = ri + 1;
                        while insert_at < items.len() && !items[insert_at].is_header {
                            insert_at += 1;
                        }
                        let at = todo::add_task(
                            &mut items,
                            insert_at.saturating_sub(1),
                            0,
                            input_buffer.clone(),
                        );
                        dirty.insert(items[at].source.clone());
                        let new_vis = compute_visible(&items, &search_buffer);
                        if let Some(pos) = new_vis.iter().position(|&i| i == at) {
                            state.select(Some(pos));
                        }
                    }
                    input_buffer.clear();
                    cursor_pos = 0;
                    input_mode = false;
                }
                KeyCode::Esc => {
                    input_buffer.clear();
                    cursor_pos = 0;
                    input_mode = false;
                }
                KeyCode::Left => {
                    cursor_pos = prev_char_boundary(&input_buffer, cursor_pos);
                }
                KeyCode::Right => {
                    cursor_pos = next_char_boundary(&input_buffer, cursor_pos);
                }
                KeyCode::Backspace => {
                    if cursor_pos > 0 {
                        let prev = prev_char_boundary(&input_buffer, cursor_pos);
                        input_buffer.remove(prev);
                        cursor_pos = prev;
                    }
                }
                KeyCode::Char(c) => {
                    input_buffer.insert(cursor_pos, c);
                    cursor_pos += c.len_utf8();
                }
                _ => {}
            }
            continue;
        }

        // Subtask prompt: inserts after the parent's last child.
        if subtask_mode {
            match key.code {
                KeyCode::Enter => {
                    if !input_buffer.is_empty() {
                        let vis = compute_visible(&items, &search_buffer);
                        let vs = state.selected().unwrap_or(0);
                        let ri = vis.get(vs).copied().unwrap_or(0);
                        if items[ri].is_header || items[ri].depth >= 2 {
                            input_buffer.clear();
                            subtask_mode = false;
                            continue;
                        }
                        let child_depth = items[ri].depth + 1;
                        let end = todo::block_end(&items, ri);
                        let at = todo::add_task(
                            &mut items,
                            end - 1,
                            child_depth,
                            input_buffer.clone(),
                        );
                        dirty.insert(items[at].source.clone());
                        items[ri].collapsed = false;
                        let new_vis = compute_visible(&items, &search_buffer);
                        if let Some(pos) = new_vis.iter().position(|&i| i == at) {
                            state.select(Some(pos));
                        }
                    }
                    input_buffer.clear();
                    cursor_pos = 0;
                    subtask_mode = false;
                }
                KeyCode::Esc => {
                    input_buffer.clear();
                    cursor_pos = 0;
                    subtask_mode = false;
                }
                KeyCode::Left => {
                    cursor_pos = prev_char_boundary(&input_buffer, cursor_pos);
                }
                KeyCode::Right => {
                    cursor_pos = next_char_boundary(&input_buffer, cursor_pos);
                }
                KeyCode::Backspace => {
                    if cursor_pos > 0 {
                        let prev = prev_char_boundary(&input_buffer, cursor_pos);
                        input_buffer.remove(prev);
                        cursor_pos = prev;
                    }
                }
                KeyCode::Char(c) => {
                    input_buffer.insert(cursor_pos, c);
                    cursor_pos += c.len_utf8();
                }
                _ => {}
            }
            continue;
        }

        if edit_mode {
            match key.code {
                KeyCode::Enter => {
                    if !input_buffer.is_empty() && edit_idx < items.len() {
                        dirty.insert(items[edit_idx].source.clone());
                        todo::edit_text(&mut items, edit_idx, input_buffer.clone());
                    }
                    input_buffer.clear();
                    cursor_pos = 0;
                    edit_mode = false;
                }
                KeyCode::Esc => {
                    input_buffer.clear();
                    cursor_pos = 0;
                    edit_mode = false;
                }
                KeyCode::Left => {
                    cursor_pos = prev_char_boundary(&input_buffer, cursor_pos);
                }
                KeyCode::Right => {
                    cursor_pos = next_char_boundary(&input_buffer, cursor_pos);
                }
                KeyCode::Backspace => {
                    if cursor_pos > 0 {
                        let prev = prev_char_boundary(&input_buffer, cursor_pos);
                        input_buffer.remove(prev);
                        cursor_pos = prev;
                    }
                }
                KeyCode::Char(c) => {
                    input_buffer.insert(cursor_pos, c);
                    cursor_pos += c.len_utf8();
                }
                _ => {}
            }
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

        let visible = compute_visible(&items, &search_buffer);
        let vis_sel = state.selected().unwrap_or(0);
        let real_idx = visible.get(vis_sel).copied().unwrap_or(0);

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
                if vis_sel + 1 < visible.len() {
                    let target_real = visible[vis_sel + 1];
                    navigate(
                        &mut items,
                        &mut state,
                        focus_active,
                        real_idx,
                        target_real,
                        vis_sel + 1,
                        &search_buffer,
                    );
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if vis_sel > 0 {
                    let target_real = visible[vis_sel - 1];
                    navigate(
                        &mut items,
                        &mut state,
                        focus_active,
                        real_idx,
                        target_real,
                        vis_sel - 1,
                        &search_buffer,
                    );
                }
            }

            KeyCode::Char('l') | KeyCode::Right => {
                if real_idx < items.len() && items[real_idx].collapsed {
                    items[real_idx].collapsed = false;
                    focus_active = false;
                }
            }
            KeyCode::Char('h') | KeyCode::Left => {
                if real_idx < items.len()
                    && !items[real_idx].collapsed
                    && (items[real_idx].has_subtasks || items[real_idx].is_header)
                {
                    items[real_idx].collapsed = true;
                    focus_active = false;
                }
            }

            KeyCode::Char('v') => {
                let current_header = (0..=real_idx.min(items.len().saturating_sub(1)))
                    .rev()
                    .find(|&i| items[i].is_header)
                    .unwrap_or(0);
                let any_collapsed = items
                    .iter()
                    .any(|i| (i.is_header || i.has_subtasks) && i.collapsed);
                todo::set_all_collapsed(&mut items, !any_collapsed);
                let new_vis = compute_visible(&items, &search_buffer);
                if let Some(pos) = new_vis.iter().position(|&i| i == current_header) {
                    *state.offset_mut() = 0;
                    state.select(Some(pos));
                }
                focus_active = false;
            }

            KeyCode::Char(' ') | KeyCode::Char('x') | KeyCode::Enter => {
                if real_idx < items.len()
                    && !items[real_idx].is_header
                    && !items[real_idx].is_code_todo
                {
                    dirty.insert(items[real_idx].source.clone());
                    todo::toggle_done(&mut items, real_idx);
                }
            }

            KeyCode::Char('a') => {
                if real_idx < items.len()
                    && !items[real_idx].is_header
                    && !items[real_idx].has_subtasks
                    && !items[real_idx].is_code_todo
                {
                    let target = if items[real_idx].state == CheckState::Half {
                        CheckState::Unchecked
                    } else {
                        CheckState::Half
                    };
                    dirty.insert(items[real_idx].source.clone());
                    todo::set_state(&mut items, real_idx, target);
                }
            }

            KeyCode::Char('n') => {
                input_mode = true;
                input_buffer.clear();
                cursor_pos = 0;
            }

            // New category: only meaningful on the global board, where
            // categories live as <notez_root>/_todos/<name>/TODO.md.
            KeyCode::Char('N') => {
                if ctx.global {
                    category_mode = true;
                    input_buffer.clear();
                    cursor_pos = 0;
                    category_error = None;
                }
            }

            KeyCode::Char('s') => {
                if real_idx < items.len()
                    && !items[real_idx].is_header
                    && !items[real_idx].is_code_todo
                    && items[real_idx].depth < 2
                {
                    subtask_mode = true;
                    input_buffer.clear();
                    cursor_pos = 0;
                }
            }

            KeyCode::Char('d') => {
                if real_idx < items.len()
                    && !items[real_idx].is_header
                    && !items[real_idx].is_code_todo
                {
                    confirm_delete = true;
                }
            }

            KeyCode::Char('e') => {
                if real_idx < items.len()
                    && !items[real_idx].is_header
                    && !items[real_idx].is_code_todo
                {
                    edit_mode = true;
                    edit_idx = real_idx;
                    input_buffer = items[real_idx].text.clone();
                    cursor_pos = input_buffer.len();
                }
            }

            KeyCode::Char('t') => {
                flag_mode = true;
            }

            KeyCode::Char('f') => {
                if real_idx < items.len() {
                    if focus_active {
                        let current_header = (0..=real_idx)
                            .rev()
                            .find(|&i| items[i].is_header)
                            .unwrap_or(real_idx);
                        for &(idx, was_collapsed) in &pre_focus_collapsed {
                            if idx < items.len() {
                                items[idx].collapsed = was_collapsed;
                            }
                        }
                        let new_vis = compute_visible(&items, &search_buffer);
                        if let Some(pos) = new_vis.iter().position(|&i| i == current_header)
                        {
                            state.select(Some(pos));
                        }
                        focus_active = false;
                    } else {
                        pre_focus_collapsed = items
                            .iter()
                            .enumerate()
                            .filter(|(_, item)| item.is_header)
                            .map(|(i, item)| (i, item.collapsed))
                            .collect();
                        let focused_header =
                            (0..=real_idx).rev().find(|&i| items[i].is_header);
                        for i in 0..items.len() {
                            if items[i].is_header {
                                items[i].collapsed = Some(i) != focused_header;
                            }
                        }
                        focus_active = true;
                    }
                }
            }

            KeyCode::Char('/') => {
                search_mode = true;
                search_buffer.clear();
                cursor_pos = 0;
            }

            KeyCode::Char('J') => {
                if real_idx < items.len()
                    && !items[real_idx].is_header
                    && !items[real_idx].is_code_todo
                {
                    let new_idx = todo::move_task(&mut items, real_idx, false);
                    if new_idx != real_idx {
                        dirty.insert(items[new_idx].source.clone());
                    }
                    let new_vis = compute_visible(&items, &search_buffer);
                    if let Some(pos) = new_vis.iter().position(|&i| i == new_idx) {
                        state.select(Some(pos));
                    }
                }
            }
            KeyCode::Char('K') => {
                if real_idx < items.len()
                    && !items[real_idx].is_header
                    && !items[real_idx].is_code_todo
                {
                    let new_idx = todo::move_task(&mut items, real_idx, true);
                    if new_idx != real_idx {
                        dirty.insert(items[new_idx].source.clone());
                    }
                    let new_vis = compute_visible(&items, &search_buffer);
                    if let Some(pos) = new_vis.iter().position(|&i| i == new_idx) {
                        state.select(Some(pos));
                    }
                }
            }

            KeyCode::Char('?') => {
                show_help = true;
            }

            _ => {}
        }
    }

    Ok(BoardOutcome { items, dirty })
}

/// j/k step that, in focus mode, closes the section being left and opens
/// the one being entered.
fn navigate(
    items: &mut [Task],
    state: &mut ListState,
    focus_active: bool,
    from_real: usize,
    target_real: usize,
    fallback_vis: usize,
    search_buffer: &str,
) {
    if focus_active {
        let new_header = (0..=target_real).rev().find(|&i| items[i].is_header);
        let old_header = (0..=from_real).rev().find(|&i| items[i].is_header);
        if new_header != old_header {
            if let Some(oh) = old_header {
                items[oh].collapsed = true;
            }
            if let Some(nh) = new_header {
                items[nh].collapsed = false;
            }
            let new_vis = compute_visible(items, search_buffer);
            if let Some(pos) = new_vis.iter().position(|&i| i == target_real) {
                state.select(Some(pos));
            }
            return;
        }
    }
    state.select(Some(fallback_vis));
}

fn render_help(frame: &mut Frame, full: Rect) {
    let key_line = |k: &str, color: Color, desc: &str| {
        Line::from(vec![
            Span::styled(format!("  {}", k), Style::default().fg(color)),
            Span::styled(
                format!("{}{}", " ".repeat(19_usize.saturating_sub(k.len())), desc),
                Style::default().fg(theme::TEXT),
            ),
        ])
    };
    let help_text = vec![
        Line::from(Span::styled(
            "  keybindings",
            Style::default().fg(theme::MAUVE).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        key_line("x / space / enter", theme::SAPPHIRE, "check/uncheck"),
        key_line("a", theme::YELLOW, "almost done [/]"),
        key_line("n", theme::GREEN, "new todo"),
        key_line("N", theme::GREEN, "new category"),
        key_line("s", theme::LAVENDER, "add subtask"),
        key_line("e", theme::MAUVE, "edit text"),
        key_line("d", theme::RED, "delete"),
        key_line("t", theme::PEACH, "tags (1-5 to toggle)"),
        key_line("f", theme::GREEN, "focus section"),
        key_line("/", theme::SAPPHIRE, "filter: fuzzy text + #tagname"),
        key_line("v", theme::SAPPHIRE, "view all / collapse all"),
        key_line("j/k", theme::TEXT, "navigate"),
        key_line("h/l", theme::TEXT, "collapse / expand"),
        key_line("J/K", theme::TEXT, "move todo up / down"),
        key_line("drag", theme::TEXT, "mouse drag to reorder"),
        key_line("q", theme::PEACH, "quit"),
        Line::from(""),
        Line::from(Span::styled(
            "  press any key to close",
            Style::default().fg(theme::OVERLAY),
        )),
    ];
    let help_h = help_text.len() as u16 + 2;
    let help_w = 42_u16;
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
    use notez_core::tags::{FLAG_BLOCKED, FLAG_IMPORTANT, FLAG_PRIO};
    use std::path::PathBuf;

    fn task(text: &str, depth: u8, flags: u8) -> Task {
        Task {
            text: text.to_string(),
            state: CheckState::Unchecked,
            source: PathBuf::from("/tmp/TODO.md"),
            section: "s".to_string(),
            is_header: false,
            depth,
            has_subtasks: false,
            collapsed: false,
            is_code_todo: false,
            flags,
        }
    }

    fn header(label: &str) -> Task {
        Task {
            is_header: true,
            collapsed: false,
            ..task(label, 0, 0)
        }
    }

    #[test]
    fn filter_keeps_matches_and_their_ancestors() {
        let mut items = vec![
            header("SECTION"),
            task("parent", 0, 0),
            task("child match", 1, 0),
            task("other", 0, 0),
        ];
        items[1].has_subtasks = true;
        let f = filter::parse("match");
        let keep = compute_filter_keep(&items, &f);
        assert_eq!(keep, vec![true, true, true, false]);
    }

    #[test]
    fn filter_by_tag_uses_flag_bits() {
        let items = vec![
            header("SECTION"),
            task("tagged", 0, FLAG_PRIO),
            task("untagged", 0, 0),
        ];
        let f = filter::parse("#prio");
        let keep = compute_filter_keep(&items, &f);
        assert_eq!(keep, vec![true, true, false]);
    }

    #[test]
    fn filter_and_across_tokens() {
        let items = vec![
            header("SECTION"),
            task("both", 0, FLAG_PRIO | FLAG_BLOCKED),
            task("only prio", 0, FLAG_PRIO),
        ];
        let f = filter::parse("#prio #blocked");
        let keep = compute_filter_keep(&items, &f);
        assert_eq!(keep, vec![true, true, false]);
    }

    #[test]
    fn compute_visible_with_empty_filter_matches_core() {
        let items = vec![header("A"), task("t1", 0, 0), task("t2", 0, 0)];
        assert_eq!(
            compute_visible(&items, ""),
            todo::get_visible_indices(&items)
        );
    }

    #[test]
    fn compute_visible_applies_filter() {
        let items = vec![
            header("A"),
            task("apple", 0, 0),
            task("banana", 0, 0),
        ];
        let v = compute_visible(&items, "apple");
        assert_eq!(v, vec![0, 1]);
    }

    #[test]
    fn headers_do_not_match_text_directly() {
        // A header matching the query must not pull in its whole section.
        let items = vec![header("apple SECTION"), task("banana", 0, 0)];
        let v = compute_visible(&items, "apple");
        assert!(v.is_empty());
    }

    #[test]
    fn dot_mapping_covers_five_contiguous_columns() {
        // Dots start at area_x + 5 and are contiguous.
        assert_eq!(mouse_x_to_dot(5, 0), Some(0));
        assert_eq!(mouse_x_to_dot(9, 0), Some(4));
        assert_eq!(mouse_x_to_dot(4, 0), None);
        assert_eq!(mouse_x_to_dot(10, 0), None);
    }

    #[test]
    fn mouse_y_maps_through_wrapped_rows() {
        let list_area = Rect::new(0, 10, 80, 20);
        let visible = vec![0, 1, 2];
        // Item 0 renders as 2 rows (wrapped), items 1 and 2 as 1 row each.
        let row_counts = vec![2, 1, 1];
        assert_eq!(mouse_y_to_real_idx(10, list_area, 0, &visible, &row_counts), Some(0));
        assert_eq!(mouse_y_to_real_idx(11, list_area, 0, &visible, &row_counts), Some(0));
        assert_eq!(mouse_y_to_real_idx(12, list_area, 0, &visible, &row_counts), Some(1));
        assert_eq!(mouse_y_to_real_idx(13, list_area, 0, &visible, &row_counts), Some(2));
        assert_eq!(mouse_y_to_real_idx(14, list_area, 0, &visible, &row_counts), None);
        assert_eq!(mouse_y_to_real_idx(9, list_area, 0, &visible, &row_counts), None);
    }

    #[test]
    fn mouse_y_respects_scroll_offset() {
        let list_area = Rect::new(0, 0, 80, 5);
        let visible = vec![0, 1, 2, 3];
        let row_counts = vec![1, 1, 1, 1];
        // Scrolled past the first item: row 0 is item 1.
        assert_eq!(mouse_y_to_real_idx(0, list_area, 1, &visible, &row_counts), Some(1));
    }

    #[test]
    fn flags_slots_render_five_dots_plus_padding() {
        let spans = flags_slots(FLAG_IMPORTANT | FLAG_BLOCKED);
        // Leading space + 5 dots + trailing space.
        assert_eq!(spans.len(), 7);
        assert_eq!(spans[1].content, "●");
        assert_eq!(spans[2].content, "·");
        assert_eq!(spans[5].content, "●");
    }

    #[test]
    fn prose_warning_names_only_dirty_prose_sections() {
        let mut items = vec![header("ALPHA"), task("a", 0, 0), header("BETA")];
        items[2].source = PathBuf::from("/tmp/beta/TODO.md");

        let prose: HashSet<PathBuf> = [
            PathBuf::from("/tmp/TODO.md"),
            PathBuf::from("/tmp/beta/TODO.md"),
        ]
        .into();

        // Nothing dirty: no warning.
        assert!(prose_warning_sections(&items, &HashSet::new(), &prose).is_empty());

        // Only the dirty prose file is named, via its header label.
        let dirty: HashSet<PathBuf> = [PathBuf::from("/tmp/TODO.md")].into();
        assert_eq!(prose_warning_sections(&items, &dirty, &prose), vec!["ALPHA"]);

        // A dirty file without prose stays silent.
        let clean_dirty: HashSet<PathBuf> = [PathBuf::from("/tmp/clean/TODO.md")].into();
        assert!(prose_warning_sections(&items, &clean_dirty, &prose).is_empty());
    }

    #[test]
    fn footer_shows_the_pull_warning_and_the_prose_warning_together() {
        let pull = "vault pull hit a conflict, rebase aborted";
        let prose = vec!["ALPHA".to_string()];

        assert_eq!(footer_warning(None, &[]), None);
        assert_eq!(footer_warning(Some(pull), &[]).as_deref(), Some(pull));
        assert_eq!(
            footer_warning(None, &prose).as_deref(),
            Some("non-todo text in ALPHA will be dropped on save")
        );

        let both = footer_warning(Some(pull), &prose).unwrap();
        assert!(both.starts_with(pull), "pull warning first: {both}");
        assert!(
            both.contains("non-todo text in ALPHA will be dropped on save"),
            "prose warning must not be hidden: {both}"
        );
    }
}
