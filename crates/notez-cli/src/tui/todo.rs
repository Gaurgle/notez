//! The todoz board TUI, ported from notez-cli.
//!
//! Only the terminal layer lives here: rendering, key and mouse handling,
//! filter state, and the help overlay. All board semantics (parsing,
//! hierarchy, drag rules, mutation, saving) come from [`notez_core::todo`],
//! and filter parsing from [`notez_core::filter`]. The caller assembles the
//! board (see `commands::todo`) and persists only the sources reported
//! dirty in the returned [`BoardOutcome`]; files the user never touched are
//! never rewritten (rewriting drops any non-todo text in them).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

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
use super::header::{self, SyncState};
use super::help::{self, HelpState};
use super::sync::VaultSync;
use super::{VimCommandMode, VimKey, text, theme, todo_rows};
#[cfg(test)]
use super::todo_rows::{branch_prefix, flags_slots, later_siblings};

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
    key("R", "reload", "reload the board from disk, saving unsaved edits first (it also reloads by itself, within 2 s of idle, when a shown TODO.md changes, unless it has unsaved edits here)", theme::SAPPHIRE, Group::View, BOARD, Slot::HelpOnly, None),
    key("g", "sync", "sync the vault now, as quitting does: save edits, commit, pull, push (in the background; the header shows syncing, then the result, and the board reloads if a shown TODO.md changed)", theme::SAPPHIRE, Group::View, BOARD, Slot::HelpOnly, None),
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
    let max_cols =
        width.saturating_sub(WARNING_PREFIX.len() + right_cols + QUIT_HINT_RESERVED_COLS + 1);
    let text = footer::clip_to_cols(warning, max_cols);
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
        + Span::raw(warning).width()
        + span_cols(&right)
        + QUIT_HINT_RESERVED_COLS
        + 1;
    if needed > width { Vec::new() } else { right }
}

/// What the header shows and which global-only features are enabled.
pub struct BoardContext {
    /// Global board: category creation (`N`) and reload-after-create work.
    pub global: bool,
    pub title: String,
    pub path_display: String,
    /// Shown in the footer's warning slot for the whole session, ahead of
    /// the prose warning when both apply.
    pub warning: Option<String>,
    /// The vault's sync state as the session opened, for the header.
    pub sync: SyncState,
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
    // The in-session sync (`g`). It is waited for only after the TUI has
    // left, on every path out, so a hung network never freezes raw mode.
    let mut vault_sync = VaultSync::new(config.notez_root_path(), ctx.sync);
    let mut terminal = super::enter().context("failed to enter TUI")?;
    let result = event_loop(&mut terminal, items, ctx, config, prose_sources, &mut vault_sync);
    // A quit that waited for a sync has already left the TUI.
    let left = if crossterm::terminal::is_raw_mode_enabled().unwrap_or(true) {
        super::leave().context("failed to leave TUI")
    } else {
        Ok(())
    };
    vault_sync.finish();
    left?;
    result
}

/// Whether quit may go ahead, or the footer message to go back to the board
/// with. A running sync is waited for outside the TUI (so a hung network
/// leaves a usable terminal, not raw mode); if this sync or one that ended
/// unprobed (`sync_pulled`) moved `HEAD` and changed a file with unsaved
/// edits here, see [`pulled_over_edits`], the TUI is entered again.
fn quit_after_sync(
    terminal: &mut super::TuiTerminal,
    vault_sync: &mut VaultSync,
    sync_pulled: &mut bool,
    probe: &mut ProbeSnapshot,
    dirty: &HashSet<PathBuf>,
    held: &mut HashSet<PathBuf>,
) -> Result<Option<String>> {
    let left = vault_sync.running();
    if left {
        super::leave().context("failed to leave TUI")?;
        if let Some(done) = vault_sync.finish() {
            *sync_pulled |= done.moved;
        }
    }
    let moved = std::mem::take(sync_pulled);
    let paths: Vec<PathBuf> = dirty.iter().cloned().collect();
    let reading = if moved { probe_snapshot(&paths) } else { ProbeSnapshot::new() };
    let newly = pulled_over_edits(moved, probe, reading, dirty, held);
    if newly.is_empty() {
        return Ok(None);
    }
    if left {
        *terminal = super::enter().context("failed to re-enter TUI")?;
    }
    Ok(Some(held_message(&newly)))
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

/// The single row the board shows instead of an empty list when the filter
/// `search_buffer` hides every item: [`text::NO_MATCHES`]. `None` while a
/// row is visible, with no filter, or on a board with no items.
fn no_matches_row(items: &[Task], visible: &[usize], search_buffer: &str) -> Option<&'static str> {
    (!items.is_empty() && visible.is_empty() && !filter::parse(search_buffer).is_empty())
        .then_some(text::NO_MATCHES)
}

/// Per-item keep mask: an item is kept when it matches directly or any
/// descendant matches, and every match pulls in its ancestor chain (parent
/// todos and the section header) so results keep their context. Text
/// tokens match as fuzzy subsequences (see [`text::filter_matches`]).
fn compute_filter_keep(items: &[Task], f: &Filter) -> Vec<bool> {
    let n = items.len();
    let mut keep = vec![false; n];
    for (i, item) in items.iter().enumerate() {
        if !item.is_header && text::filter_matches(f, &item.text, item.flags) {
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

/// The column of a list row's first tag dot, relative to the list: rows
/// start with the one-column gutter drawn by [`todo_rows::flags_slots`], and the list
/// draws no highlight symbol.
const FIRST_DOT_COL: u16 = 1;

/// What the filter strip draws before its five dots, as wide as
/// [`FIRST_DOT_COL`] so the strip's dots sit over the rows' dots.
const FILTER_STRIP_LEAD: &str = " ";

/// Map a mouse column onto a tag-dot index (0..=4) on a list row or the
/// filter strip, which share the geometry: dot 0 sits at
/// `area_x + FIRST_DOT_COL` and the dots are contiguous.
fn mouse_x_to_dot(mouse_col: u16, area_x: u16) -> Option<u8> {
    let dot_start = area_x.saturating_add(FIRST_DOT_COL);
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

/// Every row of `visible` drawn for a list `list_width` columns wide:
/// the dot field (with the hover preview of `hover_flag`), the tree
/// drawing in the tree's dim colour, then the header label, or the
/// checkbox and text of a todo, wrapping long todos under their text.
/// Nothing is highlighted: the rows as drawn with no filter.
fn board_rows(
    items: &[Task],
    visible: &[usize],
    hover_flag: Option<(usize, u8)>,
    list_width: u16,
) -> Vec<Text<'static>> {
    board_rows_matching(items, visible, hover_flag, list_width, "")
}

/// [`board_rows`] with the characters of each todo's text that the text
/// tokens of the filter `search_buffer` matched in [`theme::filter_match`],
/// on every line of a wrapped todo. Section headers are never matched, so
/// they are never lit.
fn board_rows_matching(
    items: &[Task],
    visible: &[usize],
    hover_flag: Option<(usize, u8)>,
    list_width: u16,
    search_buffer: &str,
) -> Vec<Text<'static>> {
    todo_rows::rows(items, visible, hover_flag, list_width, search_buffer)
}

/// The board's rows as list items. The `selected` row (clamped to the last
/// row, as the list clamps it) gets [`theme::selected_row`] as its item
/// style, under the spans, so tag and check-mark colours survive on the
/// cursor row, as in the tree browser. While dragging, the dragged row
/// (`drag_start`) is dimmed and the drop target (`drag_target`) brighter;
/// the cursor row's style goes over either, as the list highlight did.
fn board_items(
    rows: Vec<Text<'static>>,
    visible: &[usize],
    selected: Option<usize>,
    drag_start: Option<usize>,
    drag_target: Option<usize>,
) -> Vec<ListItem<'static>> {
    let selected = selected.map(|s| s.min(rows.len().saturating_sub(1)));
    rows.into_iter()
        .enumerate()
        .map(|(pos, row)| {
            let real_idx = visible.get(pos).copied();
            let mut style = Style::default();
            if real_idx.is_some() && real_idx == drag_start {
                style = style.bg(theme::SURFACE);
            } else if real_idx.is_some() && real_idx == drag_target {
                style = style.bg(theme::OVERLAY);
            }
            if Some(pos) == selected {
                style = style.patch(theme::selected_row());
            }
            ListItem::new(row).style(style)
        })
        .collect()
}

// --- Reload (`R` and the file-change probe) ---
//
// The probe mirrors the tree's (`tree::model`); its helpers are private to
// that module, so the three small ones are duplicated here.

/// How long the event loop waits for input before it probes the disk.
const PROBE_INTERVAL: Duration = Duration::from_secs(2);

/// The modified time of each probed path; `None` when it could not be read
/// (the path is gone or unreadable).
type ProbeSnapshot = HashMap<PathBuf, Option<SystemTime>>;

/// The paths the probe stats: every distinct `TODO.md` on the board (a
/// header's source counts, so an empty category's missing file is watched
/// for its creation), plus `todos_dir` (`<notez_root>/_todos` on the global
/// board), whose own time changes when a category folder is added or
/// removed. Code TODOs are never probed. A project `TODO.md` that is not on
/// the board yet shows on the next reload (`R`, or one the probe triggers).
fn probe_paths(items: &[Task], todos_dir: Option<&Path>) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = items
        .iter()
        .filter(|t| !t.is_code_todo)
        .map(|t| t.source.clone())
        .chain(todos_dir.map(Path::to_path_buf))
        .collect();
    paths.sort();
    paths.dedup();
    paths
}

/// The modified time of each of `paths`, from `metadata`: one `stat` per
/// path, no file is opened or read.
fn probe_snapshot(paths: &[PathBuf]) -> ProbeSnapshot {
    paths
        .iter()
        .map(|p| (p.clone(), std::fs::metadata(p).and_then(|m| m.modified()).ok()))
        .collect()
}

/// Whether `new` shows a change against `old`: a path probed in both whose
/// modified time differs, which covers a file that appeared or vanished. A
/// path in only one of them is a new baseline, not a change.
fn probe_changed(old: &ProbeSnapshot, new: &ProbeSnapshot) -> bool {
    new.iter()
        .any(|(path, time)| old.get(path).is_some_and(|before| before != time))
}

/// What an idle probe asks the event loop to do.
#[derive(Debug, PartialEq, Eq)]
enum ProbeStep {
    /// Nothing on the board changed on disk.
    Unchanged,
    /// Clean files changed and no unsaved edit is at risk: save the dirty
    /// files and reload. The change stays pending in the snapshot until the
    /// caller takes a fresh one after the reload succeeds, so a failed save
    /// is retried by the next probe.
    Reload,
    /// A file with unsaved edits here changed on disk, now or earlier: an
    /// automatic save would overwrite that outside edit, so nothing is saved
    /// or reloaded until `R`. Lists the files newly found so, for the footer;
    /// empty when they were reported before.
    Hold(Vec<PathBuf>),
}

/// Compare `reading` with the snapshot `last` and decide what to do.
/// Paths new to the probe become baselines. A dirty path that changed is
/// taken into `last` and added to `held`, so it is reported once and a
/// later change to it is still seen; while any held path is dirty, no
/// change reloads by itself. The caller clears `held` whenever a reload
/// clears `dirty`.
fn probe_step(
    last: &mut ProbeSnapshot,
    reading: ProbeSnapshot,
    dirty: &HashSet<PathBuf>,
    held: &mut HashSet<PathBuf>,
) -> ProbeStep {
    let changed = probe_changed(last, &reading);
    let mut newly_held: Vec<PathBuf> = Vec::new();
    for (path, time) in reading {
        match last.get(&path) {
            None => {
                last.insert(path, time);
            }
            Some(before) if *before != time && dirty.contains(&path) => {
                held.insert(path.clone());
                newly_held.push(path.clone());
                last.insert(path, time);
            }
            Some(_) => {}
        }
    }
    newly_held.sort();
    if !changed {
        ProbeStep::Unchanged
    } else if !newly_held.is_empty() || held.iter().any(|p| dirty.contains(p)) {
        ProbeStep::Hold(newly_held)
    } else {
        ProbeStep::Reload
    }
}

/// At quit, after waiting for a sync: the files with unsaved edits here
/// that the sync's pull changed on disk (`moved`: it moved `HEAD`), whose
/// time in `reading` differs from the probe baseline `last`. Saving those
/// would write a board parsed before the pull over the pulled file, and the
/// exit sync would push the revert, so quit goes back to the board with
/// [`held_message`] instead. As in [`probe_step`], each is taken into
/// `last` and `held`, so it is reported once and a second quit saves.
fn pulled_over_edits(
    moved: bool,
    last: &mut ProbeSnapshot,
    reading: ProbeSnapshot,
    dirty: &HashSet<PathBuf>,
    held: &mut HashSet<PathBuf>,
) -> Vec<PathBuf> {
    if !moved {
        return Vec::new();
    }
    let mut newly_held: Vec<PathBuf> = Vec::new();
    for (path, time) in reading {
        if dirty.contains(&path) && last.get(&path).is_some_and(|before| *before != time) {
            held.insert(path.clone());
            newly_held.push(path.clone());
            last.insert(path, time);
        }
    }
    newly_held.sort();
    newly_held
}

/// The footer message for files [`ProbeStep::Hold`] newly found.
fn held_message(paths: &[PathBuf]) -> String {
    let names: Vec<String> = paths.iter().map(|p| tilde::contract(p)).collect();
    format!(
        "{} changed on disk, unsaved edits here: R to save and reload",
        names.join(", ")
    )
}

/// Whether the idle probe may run: no prompt, mode, help overlay or mouse
/// drag is open. Focus is a view, not an input mode, so it does not block.
fn probe_allowed(flags: &InputFlags, help_open: bool, dragging: bool) -> bool {
    let InputFlags {
        confirm_delete,
        flag_mode,
        search_mode,
        edit_mode,
        subtask_mode,
        category_mode,
        input_mode,
        vim_active,
        focus_active: _,
    } = *flags;
    !(confirm_delete
        || flag_mode
        || search_mode
        || edit_mode
        || subtask_mode
        || category_mode
        || input_mode
        || vim_active
        || help_open
        || dragging)
}

/// The board as it is on disk now. The global board is listed again from
/// the registry, as after `N`; a single-scope board keeps its synthesized
/// header and code TODOs and reads each section's file again.
fn board_from_disk(items: &[Task], global: bool, config: &Config) -> Vec<Task> {
    if global {
        let registry = ProjectRegistry::load().unwrap_or_default();
        todo::load_board(config, &registry)
    } else {
        reload_sections(items, todo::load_single_todo)
    }
}

/// `items` with every file-backed section's rows replaced by what `load`
/// reads for that header's source and section. Headers and code TODOs stay.
fn reload_sections(items: &[Task], load: impl Fn(&Path, &str) -> Vec<Task>) -> Vec<Task> {
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        if item.is_header && !item.is_code_todo {
            out.push(item.clone());
            out.extend(load(&item.source, &item.section));
        } else if item.is_header || item.is_code_todo {
            out.push(item.clone());
        }
    }
    out
}

/// What identifies a row across a reload: its file, section, kind, depth
/// and text. Indices shift when rows are added elsewhere; these do not.
fn same_row(a: &Task, b: &Task) -> bool {
    a.is_header == b.is_header
        && a.is_code_todo == b.is_code_todo
        && a.source == b.source
        && a.section == b.section
        && a.depth == b.depth
        && a.text == b.text
}

/// The visible position the cursor goes to after a reload: the row that was
/// selected (`old_real`, at visible position `old_sel`) when `new_items`
/// still has it visible (the nearest match by index when the text repeats),
/// else the same position clamped to the visible rows. `None` on an empty
/// board.
fn keep_row(
    old_items: &[Task],
    old_real: Option<usize>,
    old_sel: usize,
    new_items: &[Task],
    new_visible: &[usize],
) -> Option<usize> {
    if new_visible.is_empty() {
        return None;
    }
    let kept = old_real.and_then(|real| {
        let row = old_items.get(real)?;
        new_visible
            .iter()
            .enumerate()
            .filter(|&(_, &i)| same_row(&new_items[i], row))
            .min_by_key(|&(_, &i)| i.abs_diff(real))
            .map(|(pos, _)| pos)
    });
    Some(kept.unwrap_or(old_sel.min(new_visible.len() - 1)))
}

/// Give each header and parent in `new` the fold state its match in `old`
/// had, so a reload never opens or closes what the user left. Rows new to
/// the board keep the loader's state.
fn carry_collapsed(old: &[Task], new: &mut [Task]) {
    for item in new.iter_mut().filter(|t| t.is_header || t.has_subtasks) {
        if let Some(before) = old.iter().find(|o| same_row(o, item)) {
            item.collapsed = before.collapsed;
        }
    }
}

/// Focus mode's saved header fold states, re-indexed from `old` to `new`.
/// A header that is gone drops out.
fn remap_pre_focus(old: &[Task], pre_focus: &[(usize, bool)], new: &[Task]) -> Vec<(usize, bool)> {
    pre_focus
        .iter()
        .filter_map(|&(idx, collapsed)| {
            let header = old.get(idx)?;
            let at = new.iter().position(|n| same_row(n, header))?;
            Some((at, collapsed))
        })
        .collect()
}

/// Reload the board in place. Dirty edits are written first through `save`
/// and `dirty` cleared; if that fails nothing is reloaded and the message
/// is returned. Then `fresh` lists the board again, and the fold state, the
/// focus restore list and the cursor carry over (the filter `search_buffer`
/// is the caller's and stays).
fn reload_board(
    items: &mut Vec<Task>,
    dirty: &mut HashSet<PathBuf>,
    state: &mut ListState,
    pre_focus_collapsed: &mut Vec<(usize, bool)>,
    search_buffer: &str,
    save: impl FnOnce(&[Task], &HashSet<PathBuf>) -> std::io::Result<()>,
    fresh: impl FnOnce(&[Task]) -> Vec<Task>,
) -> std::result::Result<(), String> {
    if !dirty.is_empty() {
        save(items, dirty).map_err(|e| format!("save failed, not reloaded: {e}"))?;
        dirty.clear();
    }
    let old_visible = compute_visible(items, search_buffer);
    let old_sel = state.selected().unwrap_or(0);
    let old_real = old_visible.get(old_sel).copied();

    let mut new_items = fresh(items);
    carry_collapsed(items, &mut new_items);
    *pre_focus_collapsed = remap_pre_focus(items, pre_focus_collapsed, &new_items);
    let new_visible = compute_visible(&new_items, search_buffer);
    state.select(keep_row(items, old_real, old_sel, &new_items, &new_visible));
    *items = new_items;
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn event_loop(
    terminal: &mut super::TuiTerminal,
    mut items: Vec<Task>,
    ctx: &BoardContext,
    config: &Config,
    mut prose_sources: HashSet<PathBuf>,
    vault_sync: &mut VaultSync,
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
    // The header's count of uncommitted vault files. Checks and edits stay
    // in memory until the board saves on exit, so only creating a category,
    // which writes files, marks it stale; plain navigation never runs git.
    let mut vault_dirty = header::vault_dirty_count(config.notez_root_path(), ctx.sync);
    // A one-off footer message (a reload's outcome); each key press clears it.
    let mut status_message: Option<String> = None;
    let todos_dir = ctx.global.then(|| config.notez_root_path().join("_todos"));
    // The file-change probe's last reading, taken fresh after every reload.
    let mut probe = probe_snapshot(&probe_paths(&items, todos_dir.as_deref()));
    // Dirty files the probe saw change on disk: no automatic reload until
    // a reload (`R`) saves over them and clears `dirty`.
    let mut held: HashSet<PathBuf> = HashSet::new();
    // A sync moved `HEAD` and the probe has not looked since: quit checks
    // the dirty files before saving over them.
    let mut sync_pulled = false;

    loop {
        todo::derive_parent_states(&mut items);
        todo::derive_header_flags(&mut items);
        // No `git status` while a sync runs git in the vault.
        let vault_dirty_files = if vault_sync.running() {
            vault_dirty.cached()
        } else {
            vault_dirty.get()
        };
        let sync_state = vault_sync.state();

        terminal
            .draw(|frame| {
                let full = frame.area();
                let area = Rect::new(
                    full.x + 2,
                    full.y + 1,
                    full.width.saturating_sub(4),
                    full.height.saturating_sub(2),
                );
                // The header line above the board; filled in once the
                // counts are known.
                let [header_area, area] =
                    Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(area);

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

                // The list sits inside the frame's borders and padding.
                let rows = board_rows_matching(
                    &items,
                    &visible,
                    hover_flag,
                    area.width.saturating_sub(4),
                    &search_buffer,
                );
                let no_matches = no_matches_row(&items, &visible, &search_buffer);
                row_counts_for_mouse = rows.iter().map(|r| r.height() as u16).collect();
                let list_items =
                    board_items(rows, &visible, state.selected(), drag_start, drag_target);

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

                let header_line = header::line(
                    &header::Header {
                        title: &ctx.title,
                        path: &ctx.path_display,
                        sync: sync_state,
                        dirty: vault_dirty_files,
                        counts: header::todo_counts(todo_count, done_count),
                    },
                    usize::from(header_area.width),
                );
                frame.render_widget(Paragraph::new(header_line), header_area);

                // Filter strip: 5 tag dots (lit when in the active filter)
                // followed by the search input or hint. The dots align with
                // the dot column on todo rows.
                let active_tags = filter::active_tag_bits(&search_buffer);
                let mut filter_spans: Vec<Span> = Vec::new();
                filter_spans.push(Span::raw(FILTER_STRIP_LEAD));
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
                            "  fuzzy text + #tag or click a dot",
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
                    .borders(Borders::ALL)
                    // The board is the only pane and always has focus, so
                    // it takes the focused border, as the tree's list does.
                    .border_style(theme::border_focused())
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

                if let Some(text) = no_matches {
                    frame.render_widget(
                        Paragraph::new(Line::from(Span::styled(
                            format!("  {text}"),
                            Style::default().fg(theme::OVERLAY),
                        ))),
                        list_area,
                    );
                } else {
                    frame.render_stateful_widget(List::new(list_items), list_area, &mut state);
                }

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
                        prose_warning_sections(&items, &dirty, &prose_sources);
                    let warning = footer_warning(ctx.warning.as_deref(), &prose_warn);
                    let right = if scroll_info.is_empty() {
                        Vec::new()
                    } else {
                        vec![Span::styled(scroll_info, Style::default().fg(theme::OVERLAY))]
                    };
                    match warning {
                        _ if status_message.is_some() => Line::from(Span::styled(
                            format!(" {}", status_message.as_deref().unwrap_or_default()),
                            Style::default().fg(theme::PEACH),
                        )),
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

        // Wait for input. Each `PROBE_INTERVAL` without any, probe the disk
        // unless a prompt, mode, the help overlay or a drag is open; only a
        // change ends the wait without an event, to reload and redraw.
        let flags = InputFlags {
            confirm_delete,
            flag_mode,
            search_mode,
            edit_mode,
            subtask_mode,
            category_mode,
            input_mode,
            vim_active: vim.active,
            focus_active,
        };
        let can_probe = probe_allowed(&flags, help.open, drag_candidate.is_some());
        enum Wake {
            Input(Event),
            Reload,
            Redraw,
        }
        // A running sync shortens the wait so its result shows promptly and
        // holds the probe off; once it ends the probe runs at once, so files
        // the pull changed reload (or hold) as any outside change does.
        let wake = loop {
            let wait = vault_sync.poll_interval(PROBE_INTERVAL);
            if event::poll(wait).context("failed to poll for events")? {
                break Wake::Input(event::read().context("failed to read event")?);
            }
            let finished = vault_sync.take_finished();
            if let Some(done) = &finished {
                status_message = Some(done.message());
                vault_dirty.mark_stale();
                sync_pulled |= done.moved;
            }
            if vault_sync.running() || !can_probe {
                if finished.is_some() {
                    break Wake::Redraw;
                }
                continue;
            }
            sync_pulled = false;
            let reading = probe_snapshot(&probe_paths(&items, todos_dir.as_deref()));
            match probe_step(&mut probe, reading, &dirty, &mut held) {
                ProbeStep::Reload => break Wake::Reload,
                // The board stays as it is; only the footer message changes.
                ProbeStep::Hold(newly) if !newly.is_empty() => {
                    let hold = held_message(&newly);
                    status_message = Some(match finished {
                        Some(done) => format!("{}; {hold}", done.message()),
                        None => hold,
                    });
                    break Wake::Redraw;
                }
                ProbeStep::Hold(_) | ProbeStep::Unchanged => {}
            }
            if finished.is_some() {
                break Wake::Redraw;
            }
        };
        let ev = match wake {
            Wake::Input(ev) => ev,
            Wake::Redraw => continue,
            Wake::Reload => {
                match reload_board(
                    &mut items,
                    &mut dirty,
                    &mut state,
                    &mut pre_focus_collapsed,
                    &search_buffer,
                    todo::save_todos_for,
                    |old| board_from_disk(old, ctx.global, config),
                ) {
                    Ok(()) => {
                        vault_dirty.mark_stale();
                        prose_sources = detect_prose_sources(&items);
                        probe = probe_snapshot(&probe_paths(&items, todos_dir.as_deref()));
                        held.clear();
                        hover_flag = None;
                        if status_message.is_none() {
                            status_message = Some("reloaded (files changed)".to_string());
                        }
                    }
                    // The change stays pending in `probe`; the next probe retries.
                    Err(message) => status_message = Some(message),
                }
                continue;
            }
        };

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
            if let Some(hold) =
                quit_after_sync(terminal, vault_sync, &mut sync_pulled, &mut probe, &dirty, &mut held)?
            {
                status_message = Some(hold);
                continue;
            }
            break;
        }

        // Help overlay: only `?` and Esc close it; it swallows other keys.
        if help.handle_key(key) {
            continue;
        }

        status_message = None;

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
                            vault_dirty.mark_stale();
                            let registry = ProjectRegistry::load().unwrap_or_default();
                            items = todo::load_board(config, &registry);
                            probe = probe_snapshot(&probe_paths(&items, todos_dir.as_deref()));
                            held.clear();
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
            VimKey::Command(cmd) if VimCommandMode::is_quit(&cmd) => {
                if let Some(hold) =
                    quit_after_sync(terminal, vault_sync, &mut sync_pulled, &mut probe, &dirty, &mut held)?
                {
                    status_message = Some(hold);
                    continue;
                }
                break;
            }
            VimKey::Command(_) | VimKey::Consumed => continue,
            VimKey::NotConsumed => {}
        }

        let visible = compute_visible(&items, &search_buffer);
        let vis_sel = state.selected().unwrap_or(0);
        let real_idx = visible.get(vis_sel).copied().unwrap_or(0);

        match key.code {
            KeyCode::Char('q') => {
                if let Some(hold) =
                    quit_after_sync(terminal, vault_sync, &mut sync_pulled, &mut probe, &dirty, &mut held)?
                {
                    status_message = Some(hold);
                    continue;
                }
                break;
            }
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

            KeyCode::Char('g') => {
                let started = match vault_sync.refusal() {
                    Some(why) => Err(why.message().to_string()),
                    None => save_before_sync(&items, &mut dirty, &mut held, &mut probe, todo::save_todos_for)
                        .and_then(|()| vault_sync.start().map_err(|why| why.message().to_string())),
                };
                status_message = Some(match started {
                    Ok(()) => "syncing the vault".to_string(),
                    Err(message) => message,
                });
            }

            KeyCode::Char('R') => match reload_board(
                &mut items,
                &mut dirty,
                &mut state,
                &mut pre_focus_collapsed,
                &search_buffer,
                todo::save_todos_for,
                |old| board_from_disk(old, ctx.global, config),
            ) {
                Ok(()) => {
                    vault_dirty.mark_stale();
                    prose_sources = detect_prose_sources(&items);
                    probe = probe_snapshot(&probe_paths(&items, todos_dir.as_deref()));
                    held.clear();
                    hover_flag = None;
                    status_message = Some("reloaded".to_string());
                }
                Err(message) => status_message = Some(message),
            },

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

/// Before `g`'s sync: write the dirty files through `save`, as quit would,
/// so the sync commits them. Then `dirty` and `held` clear and the saved
/// files' new times go into the probe's reading `probe`, so the save does
/// not read as an outside change. A failed save changes nothing and
/// returns the footer message.
fn save_before_sync(
    items: &[Task],
    dirty: &mut HashSet<PathBuf>,
    held: &mut HashSet<PathBuf>,
    probe: &mut ProbeSnapshot,
    save: impl FnOnce(&[Task], &HashSet<PathBuf>) -> std::io::Result<()>,
) -> std::result::Result<(), String> {
    if dirty.is_empty() {
        return Ok(());
    }
    save(items, dirty).map_err(|e| format!("save failed, not synced: {e}"))?;
    let saved: Vec<PathBuf> = dirty.drain().collect();
    probe.extend(probe_snapshot(&saved));
    held.clear();
    Ok(())
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
    use ratatui::widgets::StatefulWidget;
    use std::path::{Path, PathBuf};

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
            due: None,
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
    fn the_filter_finds_todos_by_subsequence() {
        let items = vec![
            header("A"),
            task("try fleetz", 0, 0),
            task("banana", 0, FLAG_PRIO),
        ];
        assert_eq!(compute_visible(&items, "flz"), vec![0, 1]);
        assert_eq!(compute_visible(&items, "bnn #prio"), vec![0, 2]);
        assert!(compute_visible(&items, "flz #prio").is_empty());
    }

    #[test]
    fn a_filter_that_hides_every_todo_shows_one_no_matches_row() {
        let items = vec![header("A"), task("apple", 0, 0)];
        let hidden = compute_visible(&items, "zzz");
        assert_eq!(no_matches_row(&items, &hidden, "zzz"), Some("no matches"));
        let shown = compute_visible(&items, "apl");
        assert_eq!(no_matches_row(&items, &shown, "apl"), None);
        assert_eq!(no_matches_row(&[], &[], "zzz"), None, "an empty board");
        assert_eq!(no_matches_row(&items, &[], ""), None, "no filter");
    }

    #[test]
    fn matched_characters_are_lit_on_every_line_of_a_todo() {
        let items = vec![
            header("A"),
            task("fleetz", 0, 0),
            task("a fleet of z", 0, 0),
        ];
        let lit = theme::filter_match();
        let lit_text = |rows: &[Text<'static>], row: usize| -> String {
            rows[row]
                .lines
                .iter()
                .flat_map(|l| l.spans.iter())
                .filter(|s| s.style.fg == lit.fg && s.style.add_modifier.contains(Modifier::BOLD))
                .map(|s| s.content.to_string())
                .collect()
        };
        let visible = compute_visible(&items, "flz");
        assert_eq!(visible, vec![0, 1, 2]);
        let rows = board_rows_matching(&items, &visible, None, 80, "flz");
        assert_eq!(lit_text(&rows, 0), "", "headers are never lit");
        assert_eq!(lit_text(&rows, 1), "flz");
        assert_eq!(lit_text(&rows, 2), "flz");
        // Wrapped onto several lines, each line lights its own matches.
        let narrow = board_rows_matching(&items, &visible, None, 20, "flz");
        assert!(narrow[2].lines.len() > 1, "{:?}", narrow[2]);
        assert_eq!(lit_text(&narrow, 2), "flz");
        let last = narrow[2].lines.last().unwrap();
        assert!(last
            .spans
            .iter()
            .any(|s| s.content == "z" && s.style.fg == lit.fg));
        // No filter: the rows as before, nothing lit.
        let plain = board_rows(&items, &visible, None, 80);
        assert_eq!(lit_text(&plain, 1), "");
    }

    #[test]
    fn dot_mapping_covers_five_contiguous_columns() {
        // Dots start at area_x + 1, after the gutter, and are contiguous.
        // They moved from area_x + 5 when the 4-column highlight symbol
        // went away (NZ-41).
        assert_eq!(mouse_x_to_dot(1, 0), Some(0));
        assert_eq!(mouse_x_to_dot(5, 0), Some(4));
        assert_eq!(mouse_x_to_dot(0, 0), None);
        assert_eq!(mouse_x_to_dot(6, 0), None);
        assert_eq!(mouse_x_to_dot(13, 10), Some(2));
    }

    /// A small board: a section, a parent with two subtasks (the first
    /// checked and tagged), a grandchild under the second, and a last
    /// top-level todo.
    fn nested_board() -> Vec<Task> {
        let mut items = vec![
            header("WORK"),
            task("parent", 0, 0),
            task("first child", 1, FLAG_IMPORTANT),
            task("second child", 1, 0),
            task("grandchild", 2, 0),
            task("last", 0, 0),
        ];
        items[1].has_subtasks = true;
        items[3].has_subtasks = true;
        items[2].state = CheckState::Checked;
        items
    }

    /// `rows` drawn as a list `width` columns wide with the cursor on
    /// `selected`, as the event loop draws them: each cell's symbol and
    /// style, row by row.
    fn render_board(rows: Vec<Text<'static>>, visible: &[usize], selected: Option<usize>, width: u16) -> Vec<Vec<(String, Style)>> {
        use ratatui::buffer::Buffer;
        let height: u16 = rows.iter().map(|r| r.height() as u16).sum();
        let area = Rect::new(0, 0, width, height);
        let mut buf = Buffer::empty(area);
        let list = List::new(board_items(rows, visible, selected, None, None));
        let mut state = ListState::default();
        state.select(selected);
        StatefulWidget::render(list, area, &mut buf, &mut state);
        (0..height)
            .map(|y| {
                (0..width)
                    .map(|x| {
                        let cell = &buf[(x, y)];
                        (cell.symbol().to_string(), cell.style())
                    })
                    .collect()
            })
            .collect()
    }

    fn row_text(cells: &[(String, Style)]) -> String {
        cells.iter().map(|c| c.0.as_str()).collect::<String>().trim_end().to_string()
    }

    #[test]
    fn the_cursor_row_is_bold_on_the_selection_background_and_keeps_its_colours() {
        let items = nested_board();
        let visible = compute_visible(&items, "");
        let rows = board_rows(&items, &visible, None, 80);
        // Cursor on "first child" (visible position 2).
        let cells = render_board(rows, &visible, Some(2), 80);
        let cursor = &cells[2];
        for (x, (_, style)) in cursor.iter().enumerate() {
            assert_eq!(style.bg, theme::selected_row().bg, "column {x}");
            assert!(style.add_modifier.contains(Modifier::BOLD), "column {x}");
        }
        assert_eq!(cursor[0].0, " ", "the gutter is blank: no highlight symbol");
        assert_eq!(cursor[1].0, "●");
        assert_eq!(cursor[1].1.fg, Some(theme::FLAG_COLORS[0]), "the tag dot keeps its colour");
        let check = row_text(cursor).find('x').map(|b| row_text(cursor)[..b].chars().count()).unwrap();
        assert_eq!(cursor[check].1.fg, Some(theme::SAPPHIRE), "the check mark keeps its colour");
        // Other rows carry no selection style.
        for (y, row) in cells.iter().enumerate().filter(|&(y, _)| y != 2) {
            assert!(row.iter().all(|(_, s)| s.bg != theme::selected_row().bg), "row {y}");
            assert!(row.iter().all(|(_, s)| !s.add_modifier.contains(Modifier::BOLD) || y == 0), "row {y}");
        }
    }

    #[test]
    fn a_drag_dims_the_dragged_row_lights_the_target_and_the_cursor_style_stays_on_top() {
        let items = nested_board();
        let visible = compute_visible(&items, "");
        let rows = || board_rows(&items, &visible, None, 40);
        let bg_of = |items: Vec<ListItem<'static>>, y: u16| {
            use ratatui::buffer::Buffer;
            let area = Rect::new(0, 0, 40, 6);
            let mut buf = Buffer::empty(area);
            StatefulWidget::render(List::new(items), area, &mut buf, &mut ListState::default());
            buf[(10, y)].style().bg
        };
        let dragging = || board_items(rows(), &visible, None, Some(5), Some(1));
        assert_eq!(bg_of(dragging(), 5), Some(theme::SURFACE), "dragged row");
        assert_eq!(bg_of(dragging(), 1), Some(theme::OVERLAY), "drop target");
        let on_itself = board_items(rows(), &visible, None, Some(5), Some(5));
        assert_eq!(bg_of(on_itself, 5), Some(theme::SURFACE), "a target on the dragged row stays dim");
        let selected = board_items(rows(), &visible, Some(5), Some(5), Some(1));
        assert_eq!(bg_of(selected, 5), theme::selected_row().bg, "the cursor row wins");
    }

    #[test]
    fn nested_todos_draw_the_tree_branch_lines() {
        let items = nested_board();
        let visible = compute_visible(&items, "");
        let later = later_siblings(&items, &visible);
        let g = &theme::TREE_GLYPHS;
        assert_eq!(branch_prefix(&items, 0, &later), g.section_open);
        assert_eq!(branch_prefix(&items, 1, &later), format!("{}{}", g.branch, g.folder_open));
        assert_eq!(branch_prefix(&items, 2, &later), format!("{}{}{}", g.ancestor_bar, g.branch, g.file));
        assert_eq!(branch_prefix(&items, 3, &later), format!("{}{}{}", g.ancestor_bar, g.last_branch, g.folder_open));
        assert_eq!(
            branch_prefix(&items, 4, &later),
            format!("{}{}{}{}", g.ancestor_bar, g.ancestor_blank, g.last_branch, g.file)
        );
        assert_eq!(branch_prefix(&items, 5, &later), format!("{}{}", g.last_branch, g.file));

        let rows = board_rows(&items, &visible, None, 80);
        let cells = render_board(rows, &visible, None, 80);
        assert!(row_text(&cells[0]).starts_with(&format!(" ····· {}WORK ", g.section_open)), "{:?}", row_text(&cells[0]));
        assert_eq!(row_text(&cells[1]), " ····· ├─▾ [ ] parent");
        assert_eq!(row_text(&cells[4]), " ····· │   └─  [ ] grandchild");
        assert_eq!(row_text(&cells[5]), " ····· └─  [ ] last");
        // The branch lines are drawn in the tree's dim colour.
        assert_eq!(cells[4][TREE_COL].1.fg, Some(theme::SURFACE));
        assert_eq!(cells[4][TREE_COL + 4].1.fg, Some(theme::SURFACE));
    }

    #[test]
    fn a_collapsed_parent_shows_the_closed_fold_mark() {
        let mut items = nested_board();
        items[1].collapsed = true;
        let visible = compute_visible(&items, "");
        let later = later_siblings(&items, &visible);
        let g = &theme::TREE_GLYPHS;
        assert_eq!(branch_prefix(&items, 1, &later), format!("{}{}", g.branch, g.folder_closed));
        items[0].collapsed = true;
        assert_eq!(branch_prefix(&items, 0, &later), g.section_closed);
    }

    /// The column where every row's tree drawing starts: after the
    /// gutter, the five tag dots and their space.
    const TREE_COL: usize = 7;

    #[test]
    fn a_click_on_a_drawn_tag_dot_maps_to_that_tag_and_the_strip_dots_line_up() {
        let mut items = nested_board();
        items[5].flags = FLAG_DEFS[3].bit;
        let visible = compute_visible(&items, "");
        let rows = board_rows(&items, &visible, None, 80);
        let cells = render_board(rows, &visible, None, 80);
        let last = &cells[5];
        for x in 0..80u16 {
            let drawn_dot = (1..=5).contains(&x);
            assert_eq!(mouse_x_to_dot(x, 0).is_some(), drawn_dot, "column {x}");
            if let Some(d) = mouse_x_to_dot(x, 0) {
                let glyph = &last[x as usize].0;
                assert!(glyph == "●" || glyph == "·", "column {x}: {glyph:?}");
                assert_eq!(glyph == "●", d == 3, "column {x}");
            }
        }
        assert_eq!(last[TREE_COL - 1].0, " ", "one space before the tree");
        // The filter strip's first dot sits over the rows' first dot.
        assert_eq!(Span::raw(FILTER_STRIP_LEAD).width(), 1);
        assert_eq!(mouse_x_to_dot(Span::raw(FILTER_STRIP_LEAD).width() as u16, 0), Some(0));
    }

    #[test]
    fn a_wrapped_todo_continues_under_its_text_with_the_branch_line_carried_on() {
        let mut items = nested_board();
        items[2].text = "word ".repeat(30).trim_end().to_string();
        let visible = compute_visible(&items, "");
        let rows = board_rows(&items, &visible, None, 60);
        assert!(rows[2].height() > 1, "the long todo wraps");
        let cells = render_board(rows, &visible, None, 60);
        let first = row_text(&cells[2]);
        let second = row_text(&cells[3]);
        let text_col = |s: &str| s.chars().position(|c| c == 'w').unwrap();
        assert_eq!(text_col(&first), text_col(&second), "{first:?} / {second:?}");
        assert!(second.starts_with("       │ │"), "{second:?}");
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
    fn warning_with_wide_and_combining_text_keeps_the_quit_hint_on_its_column() {
        let warnings = [
            "pull failed for /Users/\u{5b89}\u{5fb7}\u{70c8}/\u{7b14}\u{8bb0}/\u{65e5}\u{8bb0}.md \u{1f4a5}\u{1f525} retry later",
            "\u{1f4a5}\u{1f4a5}\u{1f4a5}\u{1f4a5}\u{1f4a5}\u{1f4a5}\u{1f4a5}\u{1f4a5}\u{1f4a5}\u{1f4a5}\u{1f4a5}\u{1f4a5}",
            "cafe\u{301} nai\u{308}ve re\u{301}sume\u{301} e\u{301}e\u{301}e\u{301}e\u{301}e\u{301}e\u{301}e\u{301}e\u{301}",
        ];
        for warning in warnings {
            for width in 12usize..=80 {
                for scroll in [None, Some(" 3/36 ")] {
                    let right =
                        warning_right(warning, scroll.map(Span::raw).into_iter().collect(), width);
                    let lead = warning_lead(warning, span_cols(&right), width);
                    let shown = lead[1].content.to_string();
                    assert!(warning.starts_with(&shown));
                    assert!(
                        !warning[shown.len()..].starts_with(['\u{301}', '\u{308}']),
                        "width {width}: combining mark split from its base in {shown:?}"
                    );
                    let line = status_line(lead, false, Mode::Normal, &[], right, width);
                    assert!(line.width() <= width, "width {width}: {:?}", text_of(&line));
                    let q = line.spans.iter().rposition(|s| s.content == "q").unwrap();
                    let q_col: usize = line.spans[..q].iter().map(Span::width).sum();
                    assert_eq!(
                        q_col,
                        width - QUIT_HINT_RESERVED_COLS,
                        "width {width}: {:?}",
                        text_of(&line)
                    );
                    assert_eq!(line.spans[q + 1].content, "uit", "width {width}");
                }
            }
        }
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

    // --- Reload (`R` and the file-change probe) ---

    fn in_file(mut t: Task, source: &str, section: &str) -> Task {
        t.source = PathBuf::from(source);
        t.section = section.to_string();
        t
    }

    fn code_todo(text: &str, source: &str) -> Task {
        Task {
            is_code_todo: true,
            ..in_file(task(text, 0, 0), source, "code")
        }
    }

    #[test]
    fn probe_paths_are_the_distinct_todo_files_plus_the_categories_dir() {
        let items = vec![
            in_file(header("A"), "/n/TODO.md", "global"),
            in_file(task("one", 0, 0), "/n/TODO.md", "global"),
            in_file(task("two", 0, 0), "/n/TODO.md", "global"),
            // An empty category: only its header carries the path.
            in_file(header("WORK"), "/n/_todos/work/TODO.md", "work"),
            code_todo("main.rs:3 fix", "src/main.rs"),
        ];
        assert_eq!(
            probe_paths(&items, None),
            vec![PathBuf::from("/n/TODO.md"), PathBuf::from("/n/_todos/work/TODO.md")],
            "deduped, headers count, code TODOs are never probed"
        );
        let todos = PathBuf::from("/n/_todos");
        assert_eq!(
            probe_paths(&items, Some(&todos)),
            vec![
                PathBuf::from("/n/TODO.md"),
                PathBuf::from("/n/_todos"),
                PathBuf::from("/n/_todos/work/TODO.md"),
            ],
        );
    }

    #[test]
    fn probe_sees_a_changed_vanished_or_created_file_once() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.md");
        let b = dir.path().join("b.md");
        std::fs::write(&a, "- [ ] a\n").unwrap();
        let paths = vec![a.clone(), b.clone()];
        let first = probe_snapshot(&paths);
        assert!(!probe_changed(&first, &probe_snapshot(&paths)));

        std::fs::write(&b, "- [ ] b\n").unwrap();
        let created = probe_snapshot(&paths);
        assert!(probe_changed(&first, &created), "a file that appeared");
        assert!(!probe_changed(&created, &probe_snapshot(&paths)), "reported once");

        std::fs::remove_file(&a).unwrap();
        assert!(probe_changed(&created, &probe_snapshot(&paths)), "a file that vanished");

        let only_b = probe_snapshot(std::slice::from_ref(&b));
        assert!(!probe_changed(&only_b, &created), "a newly probed path is a baseline");
    }

    #[test]
    fn probe_runs_only_while_no_input_mode_is_open() {
        let none = InputFlags::default();
        assert!(probe_allowed(&none, false, false));
        assert!(
            probe_allowed(&InputFlags { focus_active: true, ..none }, false, false),
            "focus is a view, not an input mode"
        );
        let open = [
            InputFlags { confirm_delete: true, ..none },
            InputFlags { flag_mode: true, ..none },
            InputFlags { search_mode: true, ..none },
            InputFlags { edit_mode: true, ..none },
            InputFlags { subtask_mode: true, ..none },
            InputFlags { category_mode: true, ..none },
            InputFlags { input_mode: true, ..none },
            InputFlags { vim_active: true, ..none },
        ];
        for flags in open {
            assert!(!probe_allowed(&flags, false, false), "{flags:?}");
        }
        assert!(!probe_allowed(&none, true, false), "help overlay open");
        assert!(!probe_allowed(&none, false, true), "mouse drag in progress");
    }

    #[test]
    fn single_board_reload_rereads_each_section_and_keeps_code_todos() {
        let code_header = Task {
            is_header: true,
            ..code_todo("code TODOs", "")
        };
        let items = [
            in_file(header("proj (local)"), "/p/TODO.md", "proj"),
            in_file(task("old", 0, 0), "/p/TODO.md", "proj"),
            code_header,
            code_todo("main.rs:3 fix", "src/main.rs"),
        ];
        let fresh = reload_sections(&items, |path, section| {
            assert_eq!((path, section), (Path::new("/p/TODO.md"), "proj"));
            vec![
                in_file(task("new one", 0, 0), "/p/TODO.md", "proj"),
                in_file(task("new two", 0, 0), "/p/TODO.md", "proj"),
            ]
        });
        let texts: Vec<&str> = fresh.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, ["proj (local)", "new one", "new two", "code TODOs", "main.rs:3 fix"]);
    }

    #[test]
    fn reload_keeps_the_selected_task_by_source_and_text() {
        let old = vec![
            in_file(header("A"), "/a.md", "a"),
            in_file(task("x", 0, 0), "/a.md", "a"),
            in_file(task("y", 0, 0), "/a.md", "a"),
        ];
        // Someone added a row above `y`.
        let new = vec![
            in_file(header("A"), "/a.md", "a"),
            in_file(task("x", 0, 0), "/a.md", "a"),
            in_file(task("w", 0, 0), "/a.md", "a"),
            in_file(task("y", 0, 0), "/a.md", "a"),
        ];
        let new_vis = vec![0, 1, 2, 3];
        assert_eq!(keep_row(&old, Some(2), 2, &new, &new_vis), Some(3));
        // Same text in another file is a different task.
        let other = vec![
            in_file(header("A"), "/a.md", "a"),
            in_file(task("y", 0, 0), "/b.md", "b"),
        ];
        assert_eq!(keep_row(&old, Some(2), 2, &other, &[0, 1]), Some(1), "falls back");
    }

    #[test]
    fn reload_falls_back_to_the_nearest_visible_row_when_the_task_is_gone() {
        let old = vec![
            in_file(header("A"), "/a.md", "a"),
            in_file(task("x", 0, 0), "/a.md", "a"),
            in_file(task("y", 0, 0), "/a.md", "a"),
            in_file(task("z", 0, 0), "/a.md", "a"),
        ];
        let new = vec![in_file(header("A"), "/a.md", "a"), in_file(task("x", 0, 0), "/a.md", "a")];
        assert_eq!(keep_row(&old, Some(3), 3, &new, &[0, 1]), Some(1), "clamped to the last row");
        assert_eq!(keep_row(&old, Some(1), 1, &new, &[0, 1]), Some(1));
        assert_eq!(keep_row(&old, Some(2), 2, &[], &[]), None, "empty board");
        assert_eq!(keep_row(&[], None, 0, &new, &[0, 1]), Some(0));
    }

    #[test]
    fn reload_saves_dirty_edits_first_and_keeps_view_state() {
        let mut items = vec![
            in_file(header("A"), "/a.md", "a"),
            in_file(task("parent", 0, 0), "/a.md", "a"),
            in_file(task("kid", 1, 0), "/a.md", "a"),
            in_file(header("B"), "/b.md", "b"),
            in_file(task("b1", 0, 0), "/b.md", "b"),
        ];
        items[1].has_subtasks = true;
        items[1].collapsed = true;
        items[3].collapsed = true;
        let mut dirty: HashSet<PathBuf> = [PathBuf::from("/a.md")].into_iter().collect();
        let mut state = ListState::default();
        state.select(Some(2)); // visible: A, parent, B -> B
        let mut pre_focus = vec![(0, false), (3, false)];
        let mut saved: Vec<PathBuf> = Vec::new();

        // The fresh board gained a row in A, so B moved down one; the
        // loader's own fold state (all open) must not win.
        let fresh = {
            let mut f = items.clone();
            f.insert(3, in_file(task("new", 0, 0), "/a.md", "a"));
            for t in &mut f {
                t.collapsed = false;
            }
            f
        };
        let result = reload_board(
            &mut items,
            &mut dirty,
            &mut state,
            &mut pre_focus,
            "",
            |_, d| {
                saved.extend(d.iter().cloned());
                Ok(())
            },
            |_| fresh,
        );
        assert_eq!(result, Ok(()));
        assert_eq!(saved, vec![PathBuf::from("/a.md")], "the dirty file is saved first");
        assert!(dirty.is_empty(), "memory matches disk after the reload");
        assert_eq!(items.len(), 6);
        assert!(items[1].collapsed, "the parent stays folded");
        assert!(items[4].is_header && items[4].collapsed, "B stays folded");
        assert_eq!(compute_visible(&items, ""), vec![0, 1, 3, 4]);
        assert_eq!(state.selected(), Some(3), "still on B");
        assert_eq!(pre_focus, vec![(0, false), (4, false)], "focus restore follows B");
    }

    #[test]
    fn reload_does_not_run_when_the_save_fails() {
        let mut items = vec![in_file(header("A"), "/a.md", "a"), in_file(task("x", 0, 0), "/a.md", "a")];
        let before = items.clone();
        let mut dirty: HashSet<PathBuf> = [PathBuf::from("/a.md")].into_iter().collect();
        let mut state = ListState::default();
        state.select(Some(1));
        let result = reload_board(
            &mut items,
            &mut dirty,
            &mut state,
            &mut Vec::new(),
            "",
            |_, _| Err(std::io::Error::other("disk full")),
            |_| panic!("must not reload after a failed save"),
        );
        let err = result.unwrap_err();
        assert!(err.contains("disk full") && err.contains("not reloaded"), "{err}");
        assert_eq!(dirty.len(), 1, "the edits stay pending");
        assert_eq!(items.iter().map(|t| &t.text).collect::<Vec<_>>(), before.iter().map(|t| &t.text).collect::<Vec<_>>());
        assert_eq!(state.selected(), Some(1));
    }

    #[test]
    fn reload_with_nothing_dirty_does_not_save() {
        let mut items = vec![in_file(header("A"), "/a.md", "a")];
        let result = reload_board(
            &mut items,
            &mut HashSet::new(),
            &mut ListState::default(),
            &mut Vec::new(),
            "",
            |_, _| panic!("nothing to save"),
            |old| old.to_vec(),
        );
        assert_eq!(result, Ok(()));
    }

    fn at(secs: u64) -> Option<SystemTime> {
        Some(SystemTime::UNIX_EPOCH + Duration::from_secs(secs))
    }

    fn snap(entries: &[(&str, Option<SystemTime>)]) -> ProbeSnapshot {
        entries.iter().map(|&(p, t)| (PathBuf::from(p), t)).collect()
    }

    #[test]
    fn a_changed_dirty_file_holds_the_reload_and_is_reported_once() {
        let mut last = snap(&[("/a.md", at(1)), ("/b.md", at(1))]);
        let dirty: HashSet<PathBuf> = [PathBuf::from("/a.md")].into_iter().collect();
        let mut held = HashSet::new();

        let reading = snap(&[("/a.md", at(2)), ("/b.md", at(1))]);
        assert_eq!(
            probe_step(&mut last, reading.clone(), &dirty, &mut held),
            ProbeStep::Hold(vec![PathBuf::from("/a.md")]),
            "no save, no reload: the outside edit and the unsaved one both survive"
        );
        assert_eq!(last[Path::new("/a.md")], at(2), "the held path's change is taken in");
        assert_eq!(
            probe_step(&mut last, reading, &dirty, &mut held),
            ProbeStep::Unchanged,
            "the next probe does not report it again"
        );
        let later = snap(&[("/a.md", at(3)), ("/b.md", at(1))]);
        assert_eq!(
            probe_step(&mut last, later, &dirty, &mut held),
            ProbeStep::Hold(vec![PathBuf::from("/a.md")]),
            "a later change to the same file is noticed"
        );
    }

    #[test]
    fn a_clean_change_never_auto_saves_over_a_held_file() {
        let mut last = snap(&[("/a.md", at(1)), ("/b.md", at(1))]);
        let dirty: HashSet<PathBuf> = [PathBuf::from("/a.md")].into_iter().collect();
        let mut held = HashSet::new();
        probe_step(&mut last, snap(&[("/a.md", at(2)), ("/b.md", at(1))]), &dirty, &mut held);

        // b changes; reloading would save a over its outside edit.
        let reading = snap(&[("/a.md", at(2)), ("/b.md", at(2))]);
        assert_eq!(probe_step(&mut last, reading, &dirty, &mut held), ProbeStep::Hold(vec![]));
        assert_eq!(last[Path::new("/b.md")], at(1), "b stays pending for the reload `R` does");
    }

    #[test]
    fn a_changed_clean_file_reloads_and_stays_pending_until_the_reload_succeeds() {
        let mut last = snap(&[("/a.md", at(1)), ("/b.md", at(1))]);
        let dirty: HashSet<PathBuf> = [PathBuf::from("/a.md")].into_iter().collect();
        let mut held = HashSet::new();
        let reading = snap(&[("/a.md", at(1)), ("/b.md", at(2)), ("/new.md", at(5))]);

        assert_eq!(probe_step(&mut last, reading.clone(), &dirty, &mut held), ProbeStep::Reload);
        assert_eq!(last[Path::new("/b.md")], at(1), "not taken in before the reload");
        assert_eq!(last[Path::new("/new.md")], at(5), "a newly probed path is a baseline");
        // The save failed, so the caller did not refresh: the next probe retries.
        assert_eq!(probe_step(&mut last, reading, &dirty, &mut held), ProbeStep::Reload);
        assert!(held.is_empty());
    }

    #[test]
    fn an_unchanged_reading_is_quiet() {
        let mut last = snap(&[("/a.md", at(1)), ("/gone.md", None)]);
        let reading = snap(&[("/a.md", at(1)), ("/gone.md", None)]);
        let dirty: HashSet<PathBuf> = [PathBuf::from("/a.md")].into_iter().collect();
        assert_eq!(
            probe_step(&mut last, reading, &dirty, &mut HashSet::new()),
            ProbeStep::Unchanged
        );
    }

    #[test]
    fn the_hold_message_names_the_files_and_the_way_out() {
        let msg = held_message(&[PathBuf::from("/x/TODO.md")]);
        assert_eq!(msg, "/x/TODO.md changed on disk, unsaved edits here: R to save and reload");
    }

    #[test]
    fn reload_key_is_listed_for_the_board() {
        let r = TODO_KEYS.iter().find(|k| k.key == "R").expect("R in TODO_KEYS");
        assert_eq!(r.group, Group::View);
        assert_eq!(r.modes, BOARD);
    }

    #[test]
    fn sync_key_is_listed_once_for_the_board_and_in_its_help() {
        let rows: Vec<usize> = (0..TODO_KEYS.len()).filter(|&i| TODO_KEYS[i].key == "g").collect();
        assert_eq!(rows.len(), 1);
        let g = &TODO_KEYS[rows[0]];
        assert_eq!((g.group, g.modes, g.slot), (Group::View, BOARD, Slot::HelpOnly));
        assert!(help::rows(TODO_KEYS).contains(&help::Row::Key(rows[0])));
    }

    #[test]
    fn the_sync_saves_dirty_edits_first_and_the_save_is_not_an_outside_change() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.md");
        std::fs::write(&a, "# TODO\n").unwrap();
        let items = vec![in_file(header("A"), a.to_str().unwrap(), "a")];
        let mut probe = probe_snapshot(&[a.clone()]);
        let mut dirty: HashSet<PathBuf> = [a.clone()].into_iter().collect();
        let mut held: HashSet<PathBuf> = [a.clone()].into_iter().collect();
        let mut saved = Vec::new();

        let result = save_before_sync(&items, &mut dirty, &mut held, &mut probe, |_, d| {
            saved.extend(d.iter().cloned());
            let file = std::fs::File::options().write(true).open(&a)?;
            file.set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(42))
        });

        assert_eq!(result, Ok(()));
        assert_eq!(saved, vec![a.clone()]);
        assert!(dirty.is_empty() && held.is_empty());
        let reading = probe_snapshot(&[a.clone()]);
        assert!(!probe_changed(&probe, &reading), "our own save does not reload");
    }

    #[test]
    fn quit_after_a_sync_that_pulled_over_unsaved_edits_goes_back_to_the_board() {
        let dirty: HashSet<PathBuf> = [PathBuf::from("/a.md")].into_iter().collect();
        let base = snap(&[("/a.md", at(1)), ("/b.md", at(1))]);
        let pulled_a = snap(&[("/a.md", at(2))]);

        // The sync moved HEAD and A, edited here, changed on disk: hold.
        let mut probe = base.clone();
        let mut held = HashSet::new();
        assert_eq!(
            pulled_over_edits(true, &mut probe, pulled_a.clone(), &dirty, &mut held),
            vec![PathBuf::from("/a.md")]
        );
        assert!(held.contains(Path::new("/a.md")));
        assert_eq!(probe[Path::new("/a.md")], at(2), "a second quit saves over it, knowingly");
        assert!(
            pulled_over_edits(true, &mut probe, pulled_a.clone(), &dirty, &mut held).is_empty(),
            "reported once"
        );

        // HEAD did not move: an outside change is NZ-42's business, not quit's.
        let mut probe = base.clone();
        assert!(pulled_over_edits(false, &mut probe, pulled_a, &dirty, &mut HashSet::new()).is_empty());
        assert_eq!(probe, base);

        // The pull left A alone: quit goes ahead.
        let mut probe = base.clone();
        let same = snap(&[("/a.md", at(1))]);
        assert!(pulled_over_edits(true, &mut probe, same, &dirty, &mut HashSet::new()).is_empty());

        // Only a clean file changed: nothing unsaved is at risk.
        let mut probe = base.clone();
        let pulled_b = snap(&[("/a.md", at(1)), ("/b.md", at(2))]);
        assert!(pulled_over_edits(true, &mut probe, pulled_b, &dirty, &mut HashSet::new()).is_empty());
    }

    #[test]
    fn a_failed_save_starts_no_sync_and_keeps_the_edits_dirty() {
        let mut dirty: HashSet<PathBuf> = [PathBuf::from("/a.md")].into_iter().collect();
        let result = save_before_sync(
            &[],
            &mut dirty,
            &mut HashSet::new(),
            &mut ProbeSnapshot::new(),
            |_, _| Err(std::io::Error::other("disk full")),
        );
        let err = result.unwrap_err();
        assert!(err.contains("disk full") && err.contains("not synced"), "{err}");
        assert_eq!(dirty.len(), 1);
        let nothing = save_before_sync(
            &[],
            &mut HashSet::new(),
            &mut HashSet::new(),
            &mut ProbeSnapshot::new(),
            |_, _| panic!("nothing to save"),
        );
        assert_eq!(nothing, Ok(()));
    }
}
