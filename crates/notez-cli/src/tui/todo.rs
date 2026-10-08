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

use super::footer::{self, Group, KeyHint, Mode, QUIT_HINT_RESERVED_COLS, Slot, Toggle, span_cols};
use super::help::{self, HelpState};
use super::{VimCommandMode, VimKey, theme};

const BOARD: &[Mode] = &[Mode::Normal, Mode::Focus];
const BOARD_AND_TAG: &[Mode] = &[Mode::Normal, Mode::Focus, Mode::Tag];
const TAGGING: &[Mode] = &[Mode::Tag];
const FILTERING: &[Mode] = &[Mode::Filter];
const TEXT_ENTRY: &[Mode] = &[Mode::NewItem, Mode::NewCategory, Mode::AddSubtask, Mode::EditText];
const COMMAND: &[Mode] = &[Mode::VimCommand];
const CONFIRMING: &[Mode] = &[Mode::ConfirmDelete];

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

/// Every key, mouse action and command the todo board handles, in footer
/// order. Kept in step with `event_loop`.
const TODO_KEYS: &[KeyHint] = &[
    key("j/k", "move", "move down / up (also Down / Up)", theme::TEXT, Group::Navigate, BOARD_AND_TAG, Slot::HelpOnly, None),
    key("h/l", "fold", "collapse / expand (also Left / Right)", theme::TEXT, Group::Navigate, BOARD_AND_TAG, Slot::HelpOnly, None),
    key("J/K", "reorder", "move todo down / up", theme::TEXT, Group::Navigate, BOARD, Slot::HelpOnly, None),
    key("wheel", "move", "mouse wheel moves the selection", theme::TEXT, Group::Navigate, BOARD, Slot::HelpOnly, None),
    key("click", "select", "click a row to select it and fold a header or parent", theme::TEXT, Group::Navigate, BOARD, Slot::HelpOnly, None),
    key("drag", "reorder", "mouse drag to reorder", theme::TEXT, Group::Navigate, BOARD, Slot::HelpOnly, None),
    key("x", "check", "check / uncheck (also Space, Enter)", theme::SAPPHIRE, Group::Edit, BOARD, Slot::Priority(1), None),
    key("n", "new", "new todo", theme::GREEN, Group::Edit, BOARD, Slot::Priority(2), None),
    key("e", "edit", "edit text", theme::MAUVE, Group::Edit, BOARD, Slot::Priority(3), None),
    key("t", "tags", "tag mode on / off", theme::PEACH, Group::Edit, BOARD_AND_TAG, Slot::Priority(4), Some(Toggle::Tag)),
    key("s", "subtask", "add subtask", theme::LAVENDER, Group::Edit, BOARD, Slot::Priority(7), None),
    key("d", "delete", "delete (asks to confirm)", theme::RED, Group::Edit, BOARD, Slot::Priority(8), None),
    key("a", "almost", "almost done [/]", theme::YELLOW, Group::Edit, BOARD, Slot::Priority(9), None),
    key("N", "category", "new category (global board only)", theme::GREEN, Group::Edit, BOARD, Slot::HelpOnly, None),
    key("1-5", "toggle", "tag mode: toggle tag 1 to 5 on the todo", theme::PEACH, Group::Edit, TAGGING, Slot::Priority(12), None),
    key("esc", "close", "tag mode: close", theme::PEACH, Group::Edit, TAGGING, Slot::Priority(3), None),
    key("click dot", "tag", "click a todo's tag dot to toggle that tag", theme::PEACH, Group::Edit, BOARD, Slot::HelpOnly, None),
    key("y", "yes", "delete: confirm (also Enter)", theme::RED, Group::Edit, CONFIRMING, Slot::Priority(1), None),
    key("n", "no", "delete: cancel (any other key)", theme::SAPPHIRE, Group::Edit, CONFIRMING, Slot::Priority(2), None),
    key("enter", "save", "new / subtask / edit / category: save", theme::GREEN, Group::Edit, TEXT_ENTRY, Slot::Priority(1), None),
    key("esc", "cancel", "new / subtask / edit / category: cancel", theme::PEACH, Group::Edit, TEXT_ENTRY, Slot::Priority(2), None),
    key("\u{2190}/\u{2192}", "cursor", "text input: move the cursor", theme::TEXT, Group::Edit, TEXT_ENTRY, Slot::Priority(3), None),
    key("bksp", "delete", "text input: delete the char before the cursor", theme::TEXT, Group::Edit, TEXT_ENTRY, Slot::Priority(4), None),
    key("/", "filter", "filter: fuzzy text + #tagname (starts a new filter)", theme::SAPPHIRE, Group::Filter, BOARD_AND_TAG, Slot::Priority(6), Some(Toggle::Filter)),
    key("enter", "keep", "filter: keep the filter, back to the list", theme::GREEN, Group::Filter, FILTERING, Slot::Priority(1), None),
    key("esc", "clear", "filter: clear it and close", theme::PEACH, Group::Filter, FILTERING, Slot::Priority(2), None),
    key("\u{2190}/\u{2192}", "cursor", "filter: move the cursor", theme::TEXT, Group::Filter, FILTERING, Slot::Priority(3), None),
    key("bksp", "delete", "filter: delete the char before the cursor; at the start, clear the filter and close", theme::TEXT, Group::Filter, FILTERING, Slot::Priority(4), None),
    key("esc", "clear", "clear the filter", theme::PEACH, Group::Filter, BOARD, Slot::HelpOnly, None),
    key("click bar", "filter", "click the filter bar to filter, a dot to filter by that tag", theme::SAPPHIRE, Group::Filter, BOARD, Slot::HelpOnly, None),
    key("f", "focus", "focus the current section (again to leave)", theme::GREEN, Group::View, BOARD, Slot::Priority(5), Some(Toggle::Focus)),
    key("v", "view all", "expand all / collapse all", theme::SAPPHIRE, Group::View, BOARD, Slot::Priority(10), Some(Toggle::ExpandAll)),
    key("?", "help", "this help (? or esc closes)", theme::MAUVE, Group::View, BOARD, Slot::Pinned, Some(Toggle::Help)),
    key(":q", "quit", "vim-style quit (also :wq, :qa, :q!)", theme::MAUVE, Group::View, BOARD, Slot::HelpOnly, None),
    key("enter", "run", ":command: run it", theme::GREEN, Group::View, COMMAND, Slot::Priority(1), None),
    key("esc", "cancel", ":command: close the command line, nothing else", theme::PEACH, Group::View, COMMAND, Slot::Priority(2), None),
    key("bksp", "delete", ":command: delete the last char; deleting the : closes it", theme::TEXT, Group::View, COMMAND, Slot::Priority(3), None),
    key("q", "quit", "quit (also :q)", theme::PEACH, Group::View, BOARD, Slot::Quit, None),
];

/// The board's input flags, read each frame to pick the footer mode.
#[derive(Debug, Default, Clone, Copy)]
struct InputFlags {
    confirm_delete: bool,
    flag_mode: bool,
    search_mode: bool,
    edit_mode: bool,
    subtask_mode: bool,
    category_mode: bool,
    input_mode: bool,
    vim_active: bool,
    focus_active: bool,
}

impl InputFlags {
    /// The footer mode, most specific first, in the order `event_loop`
    /// checks the flags when handling a key.
    fn footer_mode(&self) -> Mode {
        if self.confirm_delete {
            Mode::ConfirmDelete
        } else if self.flag_mode {
            Mode::Tag
        } else if self.search_mode {
            Mode::Filter
        } else if self.category_mode {
            Mode::NewCategory
        } else if self.input_mode {
            Mode::NewItem
        } else if self.subtask_mode {
            Mode::AddSubtask
        } else if self.edit_mode {
            Mode::EditText
        } else if self.vim_active {
            Mode::VimCommand
        } else if self.focus_active {
            Mode::Focus
        } else {
            Mode::Normal
        }
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

/// True when any header or parent is collapsed: the condition `v` uses to
/// choose between expanding and collapsing everything.
fn any_collapsed(items: &[Task]) -> bool {
    items
        .iter()
        .any(|i| (i.is_header || i.has_subtasks) && i.collapsed)
}

/// Whether `v` is lit: at least one header or parent exists and none is
/// collapsed. The `v` key itself only checks `any_collapsed`.
fn view_all_lit(items: &[Task]) -> bool {
    items.iter().any(|i| i.is_header || i.has_subtasks) && !any_collapsed(items)
}

/// The board's footer line: `footer::status_line` over `TODO_KEYS`.
fn status_line(
    lead: Vec<Span<'static>>,
    hints: bool,
    mode: Mode,
    on: &[Toggle],
    right: Vec<Span<'static>>,
    width: usize,
) -> Line<'static> {
    footer::status_line(TODO_KEYS, lead, hints, mode, on, right, width)
}

/// The warning prefix (" ! ") and the warning text, truncated so the right
/// part and the quit hint still fit with at least one space before them.
fn warning_lead(warning: &str, right_cols: usize, width: usize) -> Vec<Span<'static>> {
    let max_chars =
        width.saturating_sub(WARNING_PREFIX.len() + right_cols + QUIT_HINT_RESERVED_COLS + 1);
    let text: String = warning.chars().take(max_chars).collect();
    vec![
        Span::styled(
            WARNING_PREFIX,
            Style::default().fg(theme::RED).add_modifier(Modifier::BOLD),
        ),
        Span::styled(text, Style::default().fg(theme::YELLOW)),
    ]
}

const WARNING_PREFIX: &str = " ! ";

/// The right part (scroll info) to show beside `warning`: dropped when the
/// whole warning would not fit with it, since the warning matters more.
fn warning_right(warning: &str, right: Vec<Span<'static>>, width: usize) -> Vec<Span<'static>> {
    let needed = WARNING_PREFIX.len()
        + warning.chars().count()
        + span_cols(&right)
        + QUIT_HINT_RESERVED_COLS
        + 1;
    if needed > width { Vec::new() } else { right }
}

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

/// `Esc` in browse mode: clear the filter if there is one, otherwise do
/// nothing. `Esc` never quits; `q` and `:q` are the ways out.
fn browse_escape(search_buffer: &mut String) {
    search_buffer.clear();
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
    let mut help = HelpState::default();
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
                let width = chunks[1].width as usize;
                let mode = InputFlags {
                    confirm_delete,
                    flag_mode,
                    search_mode,
                    edit_mode,
                    subtask_mode,
                    category_mode,
                    input_mode,
                    vim_active: vim.active,
                    focus_active,
                }
                .footer_mode();
                let toggles = footer_toggles(
                    focus_active,
                    search_mode || !search_buffer.is_empty(),
                    flag_mode,
                    view_all_lit(&items),
                    help.open,
                );
                let hint_line = |lead: Vec<Span<'static>>| {
                    status_line(lead, true, mode, &toggles, Vec::new(), width)
                };
                let status = if confirm_delete {
                    hint_line(vec![Span::styled(
                        " delete this todo?",
                        Style::default().fg(theme::TEXT),
                    )])
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
                    hint_line(spans)
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
                    hint_line(spans)
                } else if vim.active {
                    hint_line(vec![Span::styled(
                        vim.buffer.clone(),
                        Style::default().fg(theme::MAUVE),
                    )])
                } else {
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
                    let right = if scroll_info.is_empty() {
                        Vec::new()
                    } else {
                        vec![Span::styled(scroll_info, Style::default().fg(theme::OVERLAY))]
                    };
                    match warning {
                        Some(text) => {
                            let right = warning_right(&text, right, width);
                            let lead = warning_lead(&text, span_cols(&right), width);
                            status_line(lead, false, mode, &toggles, right, width)
                        }
                        None => status_line(Vec::new(), true, mode, &toggles, right, width),
                    }
                };
                frame.render_widget(Paragraph::new(status), chunks[1]);

                if help.open {
                    help::render(frame, full, TODO_KEYS, &mut help);
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

        // Help overlay: only `?` and Esc close it; it swallows other keys.
        if help.handle_key(key) {
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

        match vim.handle_key(key) {
            VimKey::Command(cmd) if VimCommandMode::is_quit(&cmd) => break,
            VimKey::Command(_) | VimKey::Consumed => continue,
            VimKey::NotConsumed => {}
        }

        let visible = compute_visible(&items, &search_buffer);
        let vis_sel = state.selected().unwrap_or(0);
        let real_idx = visible.get(vis_sel).copied().unwrap_or(0);

        match key.code {
            KeyCode::Char('q') => break,
            KeyCode::Esc => browse_escape(&mut search_buffer),

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
                let any_collapsed = any_collapsed(&items);
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
                help.open();
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

#[cfg(test)]
mod tests {
    use super::*;
    use notez_core::tags::{FLAG_BLOCKED, FLAG_IMPORTANT, FLAG_PRIO};
    use std::path::PathBuf;

    #[test]
    fn esc_clears_the_filter_and_otherwise_does_nothing() {
        let mut search = "abc".to_string();
        browse_escape(&mut search);
        assert!(search.is_empty());
        browse_escape(&mut search);
        assert!(search.is_empty(), "with no filter Esc changes nothing and never quits");
    }

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

    fn text_of(line: &Line) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    fn footer_keys(mode: Mode, on: &[Toggle], width: usize) -> Vec<&'static str> {
        footer::select(TODO_KEYS, mode, on, width)
            .left
            .iter()
            .map(|&(i, _)| TODO_KEYS[i].key)
            .collect()
    }

    fn quit_shown(mode: Mode) -> bool {
        footer::select(TODO_KEYS, mode, &[], 200).quit.is_some()
    }

    #[test]
    fn footer_mode_follows_the_input_flags_most_specific_first() {
        let none = InputFlags::default();
        assert_eq!(none.footer_mode(), Mode::Normal);
        let focus = InputFlags { focus_active: true, ..none };
        assert_eq!(focus.footer_mode(), Mode::Focus);
        assert_eq!(InputFlags { vim_active: true, ..focus }.footer_mode(), Mode::VimCommand);
        assert_eq!(InputFlags { edit_mode: true, ..focus }.footer_mode(), Mode::EditText);
        assert_eq!(InputFlags { subtask_mode: true, ..focus }.footer_mode(), Mode::AddSubtask);
        assert_eq!(InputFlags { input_mode: true, ..focus }.footer_mode(), Mode::NewItem);
        assert_eq!(InputFlags { category_mode: true, ..focus }.footer_mode(), Mode::NewCategory);
        assert_eq!(InputFlags { search_mode: true, ..focus }.footer_mode(), Mode::Filter);
        assert_eq!(InputFlags { flag_mode: true, search_mode: true, ..focus }.footer_mode(), Mode::Tag);
        let all = InputFlags {
            confirm_delete: true,
            flag_mode: true,
            search_mode: true,
            edit_mode: true,
            subtask_mode: true,
            category_mode: true,
            input_mode: true,
            vim_active: true,
            focus_active: true,
        };
        assert_eq!(all.footer_mode(), Mode::ConfirmDelete);
    }

    #[test]
    fn footer_hints_follow_the_mode() {
        let normal = footer_keys(Mode::Normal, &[], 200);
        assert_eq!(normal, vec!["x", "n", "e", "t", "s", "d", "a", "/", "f", "v", "?"]);
        assert_eq!(footer_keys(Mode::Focus, &[], 200), normal);
        assert!(quit_shown(Mode::Normal) && quit_shown(Mode::Focus));

        assert_eq!(footer_keys(Mode::Tag, &[], 200), vec!["t", "1-5", "esc", "/"]);
        assert_eq!(footer_keys(Mode::Filter, &[], 200), vec!["enter", "esc", "\u{2190}/\u{2192}", "bksp"]);
        for mode in [Mode::NewItem, Mode::NewCategory, Mode::AddSubtask, Mode::EditText] {
            assert_eq!(footer_keys(mode, &[], 200), vec!["enter", "esc", "\u{2190}/\u{2192}", "bksp"], "{mode:?}");
        }
        assert_eq!(footer_keys(Mode::VimCommand, &[], 200), vec!["enter", "esc", "bksp"]);
        assert_eq!(footer_keys(Mode::ConfirmDelete, &[], 200), vec!["y", "n"]);
        // `q` and `?` are typed text or swallowed outside the board modes.
        for mode in [Mode::Tag, Mode::Filter, Mode::NewItem, Mode::VimCommand, Mode::ConfirmDelete] {
            assert!(!quit_shown(mode), "{mode:?}");
        }
    }

    #[test]
    fn footer_lights_the_keys_whose_mode_is_on() {
        let lit = |mode: Mode, on: &[Toggle]| -> Vec<&'static str> {
            footer::select(TODO_KEYS, mode, on, 200)
                .left
                .iter()
                .filter(|&&(_, lit)| lit)
                .map(|&(i, _)| TODO_KEYS[i].key)
                .collect()
        };
        assert!(lit(Mode::Normal, &[]).is_empty());
        assert_eq!(lit(Mode::Focus, &footer_toggles(true, false, false, false, false)), vec!["f"]);
        assert_eq!(lit(Mode::Normal, &footer_toggles(false, true, false, false, false)), vec!["/"]);
        assert_eq!(lit(Mode::Tag, &footer_toggles(false, false, true, false, false)), vec!["t"]);
        assert_eq!(lit(Mode::Normal, &footer_toggles(false, false, false, true, false)), vec!["v"]);
        assert_eq!(lit(Mode::Normal, &footer_toggles(false, false, false, false, true)), vec!["?"]);

        // `v` is lit exactly when its handler would collapse everything.
        let mut items = vec![header("A"), task("one", 0, 0)];
        assert!(!any_collapsed(&items));
        items[0].collapsed = true;
        assert!(any_collapsed(&items));
    }

    #[test]
    fn view_all_is_lit_only_with_a_collapsible_row_and_none_collapsed() {
        assert!(!view_all_lit(&[]));
        assert!(!view_all_lit(&[task("one", 0, 0), task("two", 0, 0)]));
        let mut items = vec![header("A"), task("one", 0, 0)];
        assert!(view_all_lit(&items));
        items[0].collapsed = true;
        assert!(!view_all_lit(&items));
        let mut parent = task("parent", 0, 0);
        parent.has_subtasks = true;
        assert!(view_all_lit(&[parent.clone()]));
        parent.collapsed = true;
        assert!(!view_all_lit(&[parent]));
    }

    #[test]
    fn footer_drops_low_priority_hints_and_keeps_help_and_quit() {
        let wide = footer_keys(Mode::Normal, &[], 200);
        let mut previous = wide.len();
        for width in (0..=120).rev() {
            let keys = footer_keys(Mode::Normal, &[], width);
            assert!(keys.contains(&"?"), "width {width}");
            assert!(quit_shown(Mode::Normal));
            assert!(keys.len() <= previous, "width {width}");
            previous = keys.len();
            let sel = footer::select(TODO_KEYS, Mode::Normal, &[], width);
            assert!(sel.quit.is_some(), "width {width}");
            if sel.left.len() > 1 {
                assert!(sel.left_cols + QUIT_HINT_RESERVED_COLS + 1 <= width, "width {width}");
            }
            for mode in [Mode::Normal, Mode::Tag, Mode::Filter, Mode::NewItem, Mode::ConfirmDelete] {
                let _ = status_line(Vec::new(), true, mode, &[Toggle::Help], Vec::new(), width);
                let lead = warning_lead("pull failed", 7, width);
                let _ = status_line(lead, false, mode, &[], Vec::new(), width);
            }
        }
        // The scroll info gives way before `?` or quit would be cut off.
        for width in 12..=120 {
            let line = status_line(Vec::new(), true, Mode::Normal, &[], vec![Span::raw(" 12/240 ")], width);
            let rendered = text_of(&line);
            assert!(rendered.chars().count() <= width, "width {width}: {rendered:?}");
            assert!(rendered.contains("? help") && rendered.ends_with("quit"), "width {width}: {rendered:?}");
        }
        // Check, new and edit outlast the low-priority keys.
        let narrow = footer_keys(Mode::Normal, &[], 40);
        assert_eq!(narrow, vec!["x", "n", "e", "t", "?"]);
        assert_eq!(footer_keys(Mode::Normal, &[], 0), vec!["?"]);
    }

    #[test]
    fn status_line_matches_the_shared_footer_without_extras() {
        for width in [0usize, 20, 40, 100, 160] {
            for mode in [Mode::Normal, Mode::Focus, Mode::Filter, Mode::Tag] {
                assert_eq!(
                    text_of(&status_line(Vec::new(), true, mode, &[], Vec::new(), width)),
                    text_of(&footer::line(TODO_KEYS, mode, &[], width)),
                    "width {width}, {mode:?}"
                );
            }
        }
    }

    #[test]
    fn warning_and_scroll_info_keep_the_quit_hint_on_the_shared_column() {
        let q_col = |line: &Line| {
            let rendered = text_of(line);
            rendered.chars().count() - rendered.chars().rev().position(|c| c == 'q').unwrap() - 1
        };
        let short = "pull failed";
        let long = "x".repeat(300);
        for width in [40usize, 60, 100, 120] {
            for mode in [Mode::Normal, Mode::Focus] {
                let shared = footer::line(TODO_KEYS, mode, &[], width);
                let shared_col = q_col(&shared);
                assert_eq!(footer::select(TODO_KEYS, mode, &[], width).quit_col, Some(shared_col));
                assert_eq!(shared_col, width - QUIT_HINT_RESERVED_COLS);
                for scroll in ["", " 12/240 "] {
                    let right = || {
                        if scroll.is_empty() { Vec::new() } else { vec![Span::raw(scroll)] }
                    };
                    let hints = status_line(Vec::new(), true, mode, &[], right(), width);
                    assert_eq!(q_col(&hints), shared_col, "width {width}, {mode:?}, scroll {scroll:?}");
                    assert!(text_of(&hints).contains(scroll));
                    for warning in [short, long.as_str()] {
                        let lead = warning_lead(warning, scroll.len(), width);
                        let line = status_line(lead, false, mode, &[], right(), width);
                        let rendered = text_of(&line);
                        assert_eq!(q_col(&line), shared_col, "width {width}, {mode:?}, scroll {scroll:?}");
                        assert!(rendered.starts_with(" ! "));
                        assert!(rendered.contains(scroll));
                        assert_eq!(rendered.chars().count(), width);
                    }
                }
            }
        }
        // The warning outranks the scroll info: when both do not fit, the
        // scroll info goes and the warning stays whole.
        let warning = "non-todo text in HOME will be dropped on save";
        let scroll = || vec![Span::raw(" 3/36 ")];
        assert_eq!(warning_right(warning, scroll(), 100).len(), 1);
        assert_eq!(warning_right(warning, scroll(), 59).len(), 1);
        for width in [53usize, 58] {
            let right = warning_right(warning, scroll(), width);
            assert!(right.is_empty(), "width {width}");
            let lead = warning_lead(warning, span_cols(&right), width);
            let rendered = text_of(&status_line(lead, false, Mode::Normal, &[], right, width));
            assert!(rendered.contains(warning), "width {width}: {rendered:?}");
            assert!(rendered.ends_with("quit"), "width {width}: {rendered:?}");
        }
        // A short warning is shown whole, exactly as `footer_warning` built it.
        let line = status_line(warning_lead(short, 0, 100), false, Mode::Normal, &[], Vec::new(), 100);
        assert!(text_of(&line).starts_with(" ! pull failed "));
    }

    #[test]
    fn status_line_never_overflows_and_scroll_info_ends_on_the_edge_without_quit() {
        let modes = [
            Mode::Normal,
            Mode::Filter,
            Mode::Tag,
            Mode::Rename,
            Mode::Focus,
            Mode::VimCommand,
            Mode::NewItem,
            Mode::NewCategory,
            Mode::AddSubtask,
            Mode::EditText,
            Mode::ConfirmDelete,
        ];
        let scroll = " 13/120 ";
        for width in 12usize..=200 {
            for mode in modes {
                for with_scroll in [false, true] {
                    let right = if with_scroll { vec![Span::raw(scroll)] } else { Vec::new() };
                    let rendered = text_of(&status_line(Vec::new(), true, mode, &[], right, width));
                    let cols = rendered.chars().count();
                    assert!(cols <= width, "width {width}, {mode:?}, scroll {with_scroll}: {rendered:?}");
                    if with_scroll && !quit_shown(mode) && rendered.contains(scroll) {
                        assert_eq!(cols, width, "width {width}, {mode:?}: {rendered:?}");
                        assert!(rendered.ends_with(scroll), "width {width}, {mode:?}: {rendered:?}");
                    }
                }
            }
        }
        // The reported widths keep the scroll info whole on the right edge.
        for width in [19usize, 30, 42, 55] {
            let right = vec![Span::raw(scroll)];
            let rendered = text_of(&status_line(Vec::new(), true, Mode::Filter, &[], right, width));
            assert!(rendered.ends_with(scroll), "width {width}: {rendered:?}");
            assert_eq!(rendered.chars().count(), width, "width {width}");
        }
    }

    #[test]
    fn help_lists_every_todo_key_exactly_once() {
        let rows = help::rows(TODO_KEYS);
        for i in 0..TODO_KEYS.len() {
            let n = rows.iter().filter(|r| **r == help::Row::Key(i)).count();
            assert_eq!(n, 1, "key {} ({})", TODO_KEYS[i].key, TODO_KEYS[i].help);
        }
        let headings: Vec<Group> = rows
            .iter()
            .filter_map(|r| match r {
                help::Row::Heading(g) => Some(*g),
                _ => None,
            })
            .collect();
        assert_eq!(headings, Group::ALL.to_vec());
    }
}
