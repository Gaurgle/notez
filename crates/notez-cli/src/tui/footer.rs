//! Shared key table and one-line footer for the TUI views.
//!
//! Each view declares one static `&[KeyHint]` table. The footer and the help
//! overlay (`super::help`) both render from that table, so the keys they show
//! cannot disagree. Hint selection (`select`) is pure and testable without a
//! terminal; `render` only turns a `Selection` into a ratatui `Line`.
//!
//! The footer hints the keys that apply in the current `Mode`. A key whose
//! action has an on state (`Toggle`) is lit in `theme::GREEN` while it is on.
//! When the hints do not fit, they drop from the low-priority end; pinned keys
//! (`?` help, `q` quit) always stay, and quit sits right-aligned on the column
//! given by `quit_column`.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use super::theme;

/// The input state a view is in. A key lists the modes in which it applies.
/// The text-entry modes cover the todo board's prompts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    /// Typing a filter / search.
    Filter,
    /// Toggling tags (flags) on the selected entry.
    Tag,
    Rename,
    /// A section is focused; otherwise behaves like `Normal`.
    Focus,
    /// Typing a `:` command.
    VimCommand,
    NewItem,
    NewCategory,
    AddSubtask,
    EditText,
    ConfirmDelete,
}

/// An on/off state that a key's action reflects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Toggle {
    Focus,
    Filter,
    Tag,
    ExpandAll,
    Help,
}

/// Help overlay group. The overlay lists groups in declaration order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Navigate,
    Edit,
    Filter,
    View,
}

impl Group {
    pub const ALL: [Group; 4] = [Group::Navigate, Group::Edit, Group::Filter, Group::View];

    pub fn title(self) -> &'static str {
        match self {
            Group::Navigate => "navigate",
            Group::Edit => "edit",
            Group::Filter => "filter",
            Group::View => "view",
        }
    }
}

/// Where, if anywhere, a key appears in the footer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    /// Listed in the help overlay only.
    HelpOnly,
    /// Shown in the footer; a lower number is kept longer when space runs out.
    Priority(u8),
    /// Always kept (the `?` help hint).
    Pinned,
    /// Always kept and right-aligned at `quit_column`.
    Quit,
}

/// One key of a view. `desc` is the short footer word; when it starts with
/// `key` (as `o` / "open") the footer colours that letter inside the word,
/// otherwise it draws `key desc`. `help` is the longer overlay text.
#[derive(Debug, Clone, Copy)]
pub struct KeyHint {
    pub key: &'static str,
    pub desc: &'static str,
    pub help: &'static str,
    pub color: Color,
    pub group: Group,
    pub modes: &'static [Mode],
    pub slot: Slot,
    pub toggle: Option<Toggle>,
}

impl KeyHint {
    pub fn applies_in(&self, mode: Mode) -> bool {
        self.modes.contains(&mode)
    }

    fn is_on(&self, on: &[Toggle]) -> bool {
        self.toggle.is_some_and(|t| on.contains(&t))
    }

    /// The footer text of this hint, split into the coloured key part and the
    /// dim remainder.
    fn footer_parts(&self) -> (&'static str, String) {
        if self.key.chars().count() == 1 && self.desc.starts_with(self.key) {
            (self.key, self.desc[self.key.len()..].to_string())
        } else {
            (self.key, format!(" {}", self.desc))
        }
    }

    fn footer_cols(&self) -> usize {
        let (key, rest) = self.footer_parts();
        key.chars().count() + rest.chars().count()
    }
}

/// Columns kept free at the right edge for the quit hint ("quit"). The hint's
/// `q` starts this many columns from the right edge. Every footer-line variant
/// with a right-aligned quit hint (the hints line, a warning line) uses this.
pub const QUIT_HINT_RESERVED_COLS: usize = 4;

const LEADING_COLS: usize = 1;
const GAP: &str = "  ";

/// The column the quit hint's `q` starts on in a `width`-wide line whose left
/// part takes `left_cols` columns: `QUIT_HINT_RESERVED_COLS` from the right
/// edge, but never closer than one space after the left part.
pub fn quit_column(width: usize, left_cols: usize) -> usize {
    width
        .saturating_sub(QUIT_HINT_RESERVED_COLS)
        .max(left_cols + 1)
}

/// The footer hints chosen for one mode and width.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    /// Left-aligned hints as (table index, lit), in table order.
    pub left: Vec<(usize, bool)>,
    /// The right-aligned quit hint, if one applies in this mode.
    pub quit: Option<(usize, bool)>,
    /// Columns taken by the leading space and the left hints.
    pub left_cols: usize,
    /// Column of the quit hint's first char, when there is a quit hint.
    pub quit_col: Option<usize>,
}

fn left_cols(table: &[KeyHint], indices: &[usize]) -> usize {
    let hints: usize = indices.iter().map(|&i| table[i].footer_cols()).sum();
    LEADING_COLS + hints + GAP.len() * indices.len().saturating_sub(1)
}

/// Picks the hints to show for `mode` in `width` columns. Hints that do not
/// fit drop from the low-priority end (highest `Priority` number first, later
/// table rows first on ties); `Pinned` and `Quit` keys are never dropped, even
/// if that overflows the width. A key is lit when its toggle is in `on`.
pub fn select(table: &[KeyHint], mode: Mode, on: &[Toggle], width: usize) -> Selection {
    let mut left: Vec<usize> = Vec::new();
    let mut quit = None;
    for (i, hint) in table.iter().enumerate() {
        if !hint.applies_in(mode) {
            continue;
        }
        match hint.slot {
            Slot::HelpOnly => {}
            Slot::Quit => quit = quit.or(Some(i)),
            Slot::Priority(_) | Slot::Pinned => left.push(i),
        }
    }
    let quit_cols = if quit.is_some() { QUIT_HINT_RESERVED_COLS + 1 } else { 0 };
    while left_cols(table, &left) + quit_cols > width {
        let droppable = left
            .iter()
            .enumerate()
            .filter_map(|(pos, &i)| match table[i].slot {
                Slot::Priority(p) => Some((p, pos)),
                _ => None,
            })
            .max();
        match droppable {
            Some((_, pos)) => {
                left.remove(pos);
            }
            None => break,
        }
    }
    let used = left_cols(table, &left);
    Selection {
        left: left.iter().map(|&i| (i, table[i].is_on(on))).collect(),
        quit: quit.map(|i| (i, table[i].is_on(on))),
        left_cols: used,
        quit_col: quit.map(|_| quit_column(width, used)),
    }
}

/// The two spans of one hint (the key, then its word), lit or not. For views
/// that lay out extra parts around the hints themselves.
pub fn hint_spans(hint: &KeyHint, lit: bool) -> [Span<'static>; 2] {
    let (key, rest) = hint.footer_parts();
    let key_color = if lit { theme::GREEN } else { hint.color };
    let rest_color = if lit { theme::GREEN } else { theme::OVERLAY };
    [
        Span::styled(key, Style::default().fg(key_color).add_modifier(Modifier::BOLD)),
        Span::styled(rest, Style::default().fg(rest_color)),
    ]
}

/// Draws a `Selection` made by `select` from the same `table`.
pub fn render(table: &[KeyHint], selection: &Selection) -> Line<'static> {
    let mut spans = vec![Span::raw(" ".repeat(LEADING_COLS))];
    for (n, &(i, lit)) in selection.left.iter().enumerate() {
        if n > 0 {
            spans.push(Span::raw(GAP));
        }
        spans.extend(hint_spans(&table[i], lit));
    }
    if let (Some((i, lit)), Some(col)) = (selection.quit, selection.quit_col) {
        spans.push(Span::raw(" ".repeat(col.saturating_sub(selection.left_cols))));
        spans.extend(hint_spans(&table[i], lit));
    }
    Line::from(spans)
}

/// `select` then `render`: the footer line for `mode` in `width` columns.
pub fn line(table: &[KeyHint], mode: Mode, on: &[Toggle], width: usize) -> Line<'static> {
    render(table, &select(table, mode, on, width))
}

/// Columns taken by `spans`.
pub fn span_cols(spans: &[Span]) -> usize {
    spans.iter().map(|s| s.content.chars().count()).sum()
}

/// One footer line: `lead` (a prompt, legend, command line or warning) first,
/// then, when `hints` is set, the `table` hints for `mode` that fit in the
/// space the lead leaves, then `right` (for example scroll info).
///
/// The lead is never cut here; hints drop first. With a quit hint, `right`
/// sits just before it and the quit hint starts on `quit_column`, so it lines
/// up with `line` whatever the lead and right parts are. Without a quit hint,
/// `right` ends on the right edge. When the kept hints leave no room for
/// `right`, it is left out. With an empty lead and right part and `hints`
/// set, this draws the same text as `line`.
pub fn status_line(
    table: &[KeyHint],
    lead: Vec<Span<'static>>,
    hints: bool,
    mode: Mode,
    on: &[Toggle],
    right: Vec<Span<'static>>,
    width: usize,
) -> Line<'static> {
    let lead_cols = span_cols(&lead);
    let right_cols = span_cols(&right);
    let sel = select(table, mode, on, width.saturating_sub(lead_cols + right_cols));
    let mut spans = lead;
    let mut used = lead_cols;
    // An empty selection still counts its leading space. After a lead, skip
    // it so a lead that fills the width does not overflow; without a lead,
    // keep it so this matches `line`.
    if hints && (!sel.left.is_empty() || lead_cols == 0) {
        let left = render(table, &Selection { quit: None, quit_col: None, ..sel.clone() });
        spans.extend(left.spans);
        used += sel.left_cols;
    }
    let quit_cols = if sel.quit.is_some() { QUIT_HINT_RESERVED_COLS + 1 } else { 0 };
    let (right, right_cols) = if used + right_cols + quit_cols > width {
        (Vec::new(), 0)
    } else {
        (right, right_cols)
    };
    match sel.quit {
        Some((i, lit)) => {
            let quit_col = quit_column(width, used + right_cols);
            spans.push(Span::raw(" ".repeat(quit_col - used - right_cols)));
            spans.extend(right);
            spans.extend(hint_spans(&table[i], lit));
        }
        None if !right.is_empty() => {
            spans.push(Span::raw(" ".repeat(width - used - right_cols)));
            spans.extend(right);
        }
        None => {}
    }
    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: &[Mode] = &[Mode::Normal];
    const TABLE: &[KeyHint] = &[
        KeyHint { key: "a", desc: "alpha", help: "", color: theme::GREEN, group: Group::Edit, modes: ALL, slot: Slot::Priority(1), toggle: None },
        KeyHint { key: "b", desc: "bravo", help: "", color: theme::GREEN, group: Group::Edit, modes: ALL, slot: Slot::Priority(2), toggle: Some(Toggle::Focus) },
        KeyHint { key: "?", desc: "help", help: "", color: theme::MAUVE, group: Group::View, modes: ALL, slot: Slot::Pinned, toggle: Some(Toggle::Help) },
        KeyHint { key: "q", desc: "quit", help: "", color: theme::PEACH, group: Group::View, modes: ALL, slot: Slot::Quit, toggle: None },
    ];

    fn text(line: &Line) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn embeds_a_one_letter_key_in_its_word_and_prefixes_others() {
        let line = line(TABLE, Mode::Normal, &[], 40);
        let rendered = text(&line);
        assert_eq!(rendered, format!(" alpha  bravo  ? help{}quit", " ".repeat(15)));
        assert_eq!(rendered.chars().count(), 40);
    }

    #[test]
    fn quit_starts_reserved_cols_from_the_right_edge() {
        for width in [30usize, 40, 100] {
            let sel = select(TABLE, Mode::Normal, &[], width);
            assert_eq!(sel.quit_col, Some(width - QUIT_HINT_RESERVED_COLS));
            let rendered = text(&render(TABLE, &sel));
            assert_eq!(rendered.find('q'), Some(width - QUIT_HINT_RESERVED_COLS));
        }
    }

    #[test]
    fn drops_the_highest_priority_number_first_and_keeps_pinned() {
        let indices = |w| select(TABLE, Mode::Normal, &[], w).left.iter().map(|x| x.0).collect::<Vec<_>>();
        assert_eq!(indices(26), vec![0, 1, 2]);
        assert_eq!(indices(25), vec![0, 2]);
        assert_eq!(indices(12), vec![2]);
        assert_eq!(indices(0), vec![2]);
        assert!(select(TABLE, Mode::Normal, &[], 0).quit.is_some());
    }

    #[test]
    fn zero_and_tiny_widths_do_not_panic() {
        for width in 0..12 {
            let _ = line(TABLE, Mode::Normal, &[Toggle::Help], width);
        }
    }

    #[test]
    fn lit_follows_the_toggle_set() {
        let sel = select(TABLE, Mode::Normal, &[Toggle::Focus], 80);
        assert_eq!(sel.left, vec![(0, false), (1, true), (2, false)]);
        let rendered = render(TABLE, &sel);
        let b = rendered.spans.iter().find(|s| s.content == "b").unwrap();
        assert_eq!(b.style.fg, Some(theme::GREEN));
        assert!(rendered.spans.iter().any(|s| s.content == "ravo" && s.style.fg == Some(theme::GREEN)));
        let off = render(TABLE, &select(TABLE, Mode::Normal, &[], 80));
        assert!(off.spans.iter().any(|s| s.content == "ravo" && s.style.fg == Some(theme::OVERLAY)));
    }

    #[test]
    fn keys_outside_the_mode_are_not_shown() {
        let sel = select(TABLE, Mode::Filter, &[], 80);
        assert!(sel.left.is_empty());
        assert_eq!(sel.quit, None);
        assert_eq!(sel.quit_col, None);
    }
}
