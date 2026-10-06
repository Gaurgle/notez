//! Shared help overlay: every key of a view's `KeyHint` table, grouped under
//! `Group` headings, in a centred rounded box. It renders from the same table
//! as the footer, so the two cannot disagree. Only `?` and `Esc` close it;
//! `j`/`k`/Down/Up/PgDn/PgUp scroll it when it is taller than the terminal,
//! and every other key is swallowed while it is open.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};

use super::footer::{Group, KeyHint};
use super::theme;

/// One line of the overlay body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    Heading(Group),
    /// Index into the key table.
    Key(usize),
    Blank,
}

/// The overlay body for `table`: each non-empty group in `Group::ALL` order,
/// a heading then its keys in table order, groups separated by a blank row.
/// Every table entry appears exactly once.
pub fn rows(table: &[KeyHint]) -> Vec<Row> {
    let mut out = Vec::new();
    for group in Group::ALL {
        let keys: Vec<usize> = (0..table.len()).filter(|&i| table[i].group == group).collect();
        if keys.is_empty() {
            continue;
        }
        if !out.is_empty() {
            out.push(Row::Blank);
        }
        out.push(Row::Heading(group));
        out.extend(keys.into_iter().map(Row::Key));
    }
    out
}

/// Largest useful scroll offset: the body never scrolls past its last row.
pub fn clamp_scroll(scroll: usize, body_rows: usize, visible_rows: usize) -> usize {
    scroll.min(body_rows.saturating_sub(visible_rows))
}

/// Open state and scroll position of a view's help overlay.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct HelpState {
    pub open: bool,
    pub scroll: usize,
    /// Body rows visible at the last draw; the PgDn/PgUp step.
    page: usize,
}

impl HelpState {
    pub fn open(&mut self) {
        self.open = true;
        self.scroll = 0;
    }

    /// Handles a key while help is open: `?` and `Esc` close, the scroll keys
    /// move, anything else is ignored. Returns false (key not consumed) only
    /// when help is closed, so the view handles the key itself.
    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        if !self.open {
            return false;
        }
        let page = self.page.max(1);
        match key.code {
            KeyCode::Char('?') | KeyCode::Esc => self.open = false,
            KeyCode::Char('j') | KeyCode::Down => self.scroll = self.scroll.saturating_add(1),
            KeyCode::Char('k') | KeyCode::Up => self.scroll = self.scroll.saturating_sub(1),
            KeyCode::PageDown => self.scroll = self.scroll.saturating_add(page),
            KeyCode::PageUp => self.scroll = self.scroll.saturating_sub(page),
            _ => {}
        }
        true
    }
}

const TITLE_ROWS: usize = 2;
const CLOSE_HINT: &str = "  ? or esc to close, j/k to scroll";

fn key_col_width(table: &[KeyHint]) -> usize {
    table.iter().map(|k| k.key.chars().count()).max().unwrap_or(0) + 2
}

fn body_line(table: &[KeyHint], row: Row, key_width: usize) -> Line<'static> {
    match row {
        Row::Blank => Line::from(""),
        Row::Heading(group) => Line::from(Span::styled(
            format!("  {}", group.title()),
            Style::default().fg(theme::SUBTEXT).add_modifier(Modifier::BOLD),
        )),
        Row::Key(i) => {
            let hint = &table[i];
            Line::from(vec![
                Span::styled(
                    format!("    {:<key_width$}", hint.key),
                    Style::default().fg(hint.color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(hint.help, Style::default().fg(theme::TEXT)),
            ])
        }
    }
}

/// Draws the overlay for `table` centred in `full`, clamping `state.scroll`
/// to the body and recording the page size for PgDn/PgUp.
pub fn render(frame: &mut Frame, full: Rect, table: &[KeyHint], state: &mut HelpState) {
    let key_width = key_col_width(table);
    let body = rows(table);
    let widest_help = table.iter().map(|k| k.help.chars().count()).max().unwrap_or(0);
    let content_w = (4 + key_width + widest_help).max(CLOSE_HINT.len()) + 2;

    // Borders (2) + title (2) + body + blank and close hint (2).
    let max_h = full.height as usize;
    let visible = body.len().min(max_h.saturating_sub(2 + TITLE_ROWS + 2));
    state.page = visible;
    state.scroll = clamp_scroll(state.scroll, body.len(), visible);

    let mut text = vec![
        Line::from(Span::styled(
            "  keybindings",
            Style::default().fg(theme::MAUVE).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];
    text.extend(
        body.iter()
            .skip(state.scroll)
            .take(visible)
            .map(|&row| body_line(table, row, key_width)),
    );
    text.push(Line::from(""));
    text.push(Line::from(Span::styled(CLOSE_HINT, Style::default().fg(theme::OVERLAY))));

    let help_h = (text.len() as u16 + 2).min(full.height);
    let help_w = (content_w as u16 + 2).min(full.width);
    let hx = full.x + full.width.saturating_sub(help_w) / 2;
    let hy = full.y + full.height.saturating_sub(help_h) / 2;
    let area = Rect::new(hx, hy, help_w, help_h);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme::SURFACE))
        .border_type(BorderType::Rounded)
        .style(Style::default().bg(theme::BASE));
    frame.render_widget(Clear, area);
    frame.render_widget(Paragraph::new(text).block(block), area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::footer::{Mode, Slot};
    use crossterm::event::KeyModifiers;

    const M: &[Mode] = &[Mode::Normal];
    const TABLE: &[KeyHint] = &[
        KeyHint { key: "v", desc: "view", help: "view all", color: theme::SAPPHIRE, group: Group::View, modes: M, slot: Slot::Priority(1), toggle: None },
        KeyHint { key: "j", desc: "down", help: "down", color: theme::TEXT, group: Group::Navigate, modes: M, slot: Slot::HelpOnly, toggle: None },
        KeyHint { key: "q", desc: "quit", help: "quit", color: theme::PEACH, group: Group::View, modes: M, slot: Slot::Quit, toggle: None },
    ];

    fn press(state: &mut HelpState, code: KeyCode) -> bool {
        state.handle_key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    #[test]
    fn rows_group_in_fixed_order_and_skip_empty_groups() {
        assert_eq!(
            rows(TABLE),
            vec![
                Row::Heading(Group::Navigate),
                Row::Key(1),
                Row::Blank,
                Row::Heading(Group::View),
                Row::Key(0),
                Row::Key(2),
            ]
        );
    }

    #[test]
    fn clamp_scroll_stops_at_the_last_page() {
        assert_eq!(clamp_scroll(0, 30, 10), 0);
        assert_eq!(clamp_scroll(25, 30, 10), 20);
        assert_eq!(clamp_scroll(5, 8, 10), 0);
        assert_eq!(clamp_scroll(5, 8, 0), 5);
        assert_eq!(clamp_scroll(usize::MAX, 0, 0), 0);
    }

    #[test]
    fn only_question_mark_and_esc_close_help() {
        let mut state = HelpState::default();
        assert!(!press(&mut state, KeyCode::Char('x')));
        state.open();
        for code in [KeyCode::Char('x'), KeyCode::Char('q'), KeyCode::Enter, KeyCode::Char('j')] {
            assert!(press(&mut state, code));
            assert!(state.open, "{code:?} must not close help");
        }
        assert!(press(&mut state, KeyCode::Esc));
        assert!(!state.open);
        state.open();
        assert!(press(&mut state, KeyCode::Char('?')));
        assert!(!state.open);
    }

    #[test]
    fn scroll_keys_move_and_never_underflow() {
        let mut state = HelpState::default();
        state.open();
        press(&mut state, KeyCode::Up);
        assert_eq!(state.scroll, 0);
        press(&mut state, KeyCode::Down);
        press(&mut state, KeyCode::Char('j'));
        assert_eq!(state.scroll, 2);
        press(&mut state, KeyCode::Char('k'));
        assert_eq!(state.scroll, 1);
        press(&mut state, KeyCode::PageUp);
        assert_eq!(state.scroll, 0);
        state.page = 5;
        press(&mut state, KeyCode::PageDown);
        assert_eq!(state.scroll, 5);
    }
}
