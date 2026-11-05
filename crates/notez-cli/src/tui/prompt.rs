//! The popup prompt: one typed value in a titled box at the bottom of the
//! list pane (pane 1), across its full width, never over the preview. The
//! footer under it keeps its hints, so typing never moves it. Key handling
//! stays in each TUI's loop; this only draws.

use ratatui::prelude::*;
use ratatui::widgets::{Block, BorderType, Borders, Clear, Padding, Paragraph};

use super::footer::clip_to_cols;
use super::theme;

/// The rows inside the border: the input, the error (blank while there is
/// none, so the box does not jump when one appears) and the hint.
const ROWS: u16 = 3;

/// One open popup prompt.
pub struct Prompt<'a> {
    /// The label on the box's top border (`rename`, `due`).
    pub title: &'a str,
    /// The dim line naming the accepted forms.
    pub hint: &'a str,
    /// The typed value.
    pub buffer: &'a str,
    /// The cursor's byte offset into `buffer`.
    pub cursor: usize,
    /// Why Enter refused the value; drawn red beneath the input.
    pub error: Option<&'a str>,
    /// The typed value's colour from the prompt's validator, if it has one.
    pub color: Option<Color>,
}

impl Prompt<'_> {
    /// The box at the bottom of the bordered pane `pane`, see [`docked`].
    pub fn area(&self, pane: Rect) -> Rect {
        docked(pane)
    }

    /// Draw the box over whatever is under it.
    pub fn render(&self, frame: &mut Frame, full: Rect) {
        let area = self.area(full);
        let text_cols = usize::from(area.width.saturating_sub(4));
        let error = match self.error {
            Some(message) => Line::from(Span::styled(message.to_string(), Style::default().fg(theme::RED))),
            None => Line::from(""),
        };
        let text = vec![
            self.input_line(text_cols),
            error,
            Line::from(Span::styled(self.hint.to_string(), Style::default().fg(theme::OVERLAY))),
        ];
        draw_box(frame, area, self.title, theme::MAUVE, text);
    }

    /// The typed value with the cursor block, scrolled so the cursor stays
    /// inside `cols` columns.
    fn input_line(&self, cols: usize) -> Line<'static> {
        let cursor = self.cursor.min(self.buffer.len());
        let (before, after) = match (self.buffer.get(..cursor), self.buffer.get(cursor..)) {
            (Some(before), Some(after)) => (before, after),
            _ => (self.buffer, ""),
        };
        let mut chars = after.chars();
        let under = chars.next().map_or_else(|| " ".to_string(), String::from);
        let rest: String = chars.collect();
        let before_cols = before.chars().count();
        let skip = (before_cols + 1).saturating_sub(cols.max(1));
        let value = Style::default().fg(self.color.unwrap_or(theme::TEXT));
        Line::from(vec![
            Span::styled(before.chars().skip(skip).collect::<String>(), value),
            Span::styled(under, Style::default().fg(theme::BASE).bg(theme::SAPPHIRE)),
            Span::styled(rest, value),
        ])
    }
}

/// One open yes/no question in the same box as [`Prompt`]: a red title, a
/// body line naming what it acts on and the keys that answer it. Key
/// handling stays in the TUI's loop.
pub struct Confirm<'a> {
    /// The question on the box's top border (`delete?`).
    pub title: &'a str,
    /// What the answer acts on; cut to the box's width.
    pub body: &'a str,
    /// The dim line naming the answering keys.
    pub hint: &'a str,
}

impl Confirm<'_> {
    /// The box at the bottom of the bordered pane `pane`, see [`docked`].
    pub fn area(&self, pane: Rect) -> Rect {
        docked(pane)
    }

    /// Draw the box over whatever is under it.
    pub fn render(&self, frame: &mut Frame, full: Rect) {
        let area = self.area(full);
        let text_cols = usize::from(area.width.saturating_sub(4));
        let body = if Span::raw(self.body).width() > text_cols {
            let mut cut = clip_to_cols(self.body, text_cols.saturating_sub(1));
            cut.push('\u{2026}');
            cut
        } else {
            self.body.to_string()
        };
        let text = vec![
            Line::from(Span::styled(body, Style::default().fg(theme::TEXT))),
            Line::from(""),
            Line::from(Span::styled(self.hint.to_string(), Style::default().fg(theme::OVERLAY))),
        ];
        draw_box(frame, area, self.title, theme::RED, text);
    }
}

/// The box inside the bordered pane `pane`: [`ROWS`] rows and its own
/// border high, as wide as the pane's inside, its bottom edge on the last
/// row above the pane's bottom border. It sits inside the pane's border
/// rather than over it, so the pane keeps its frame (and its editing
/// colour) all round. A pane too short or narrow for it clips it.
fn docked(pane: Rect) -> Rect {
    let x = pane.x + pane.width.min(1);
    let w = pane.width.saturating_sub(2);
    let top = pane.y + pane.height.min(1);
    let bottom = pane.bottom().saturating_sub(1).max(top);
    let h = (ROWS + 2).min(bottom - top);
    Rect::new(x, bottom - h, w, h)
}

/// The rounded, titled box with `text` inside, drawn over what is under it.
fn draw_box(frame: &mut Frame, area: Rect, title: &str, title_color: Color, text: Vec<Line<'static>>) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme::SURFACE))
        .title(Span::styled(
            format!(" {title} "),
            Style::default().fg(title_color).add_modifier(Modifier::BOLD),
        ))
        .padding(Padding::horizontal(1))
        .style(Style::default().bg(theme::BASE));
    frame.render_widget(Clear, area);
    frame.render_widget(Paragraph::new(text).block(block), area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;

    fn due<'a>(buffer: &'a str, error: Option<&'a str>, color: Option<Color>) -> Prompt<'a> {
        Prompt {
            title: "due",
            hint: "2026-10-20, +3d, +1w or clear",
            buffer,
            cursor: buffer.len(),
            error,
            color,
        }
    }

    fn draw(prompt: &Prompt, width: u16, height: u16) -> Buffer {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| {
                let full = frame.area();
                prompt.render(frame, full);
            })
            .unwrap();
        terminal.backend().buffer().clone()
    }

    fn row_text(buf: &Buffer, y: u16) -> String {
        (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect()
    }

    /// The column where `needle` starts on row `y`, counted in cells.
    fn find(buf: &Buffer, y: u16, needle: &str) -> Option<u16> {
        let cells: Vec<&str> = (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect();
        let want: Vec<String> = needle.chars().map(String::from).collect();
        (0..cells.len().saturating_sub(want.len() - 1))
            .find(|&x| want.iter().enumerate().all(|(i, c)| cells[x + i] == c))
            .map(|x| x as u16)
    }

    fn inside(area: Rect, pane: Rect) -> bool {
        area.x >= pane.x && area.y >= pane.y && area.right() <= pane.right() && area.bottom() <= pane.bottom()
    }

    /// NZ-77a: the box sits at the bottom of the bordered pane it is given,
    /// inside the pane's left, right and bottom borders, across its width.
    #[test]
    fn the_box_is_docked_at_the_bottom_of_the_pane_across_its_width() {
        let prompt = due("+1d", None, None);
        let area = prompt.area(Rect::new(0, 0, 100, 30));
        assert_eq!(area, Rect::new(1, 24, 98, 5), "one cell in from the left, right and bottom");
        let offset = prompt.area(Rect::new(10, 4, 100, 30));
        assert_eq!(offset, Rect::new(11, 28, 98, 5));
        assert_eq!(offset.bottom(), Rect::new(10, 4, 100, 30).bottom() - 1, "just above the bottom border");
        let long = Prompt { buffer: &"x".repeat(300), ..due("", None, None) };
        assert_eq!(long.area(Rect::new(0, 0, 100, 30)), area, "the value does not size the box");
    }

    /// NZ-72, NZ-77a: the TUIs pass the list pane's rect, so the box sits at
    /// its bottom, never over the preview beside it, and a small pane clips
    /// it.
    #[test]
    fn the_box_is_docked_in_a_pane_and_clipped_to_it() {
        let prompt = due("+1d", None, None);
        let pane = Rect::new(0, 1, 60, 29);
        let area = prompt.area(pane);
        assert!(inside(area, pane), "{area:?}");
        assert_eq!(area, Rect::new(1, 24, 58, 5), "rows 24..29 over the border on row 29");

        let right_pane = Rect::new(61, 1, 50, 29);
        let area = prompt.area(right_pane);
        assert!(inside(area, right_pane), "{area:?}");
        assert_eq!((area.x, area.width), (62, 48));

        for (pane, height) in [
            (Rect::new(7, 2, 30, 20), 5),
            (Rect::new(7, 2, 30, 7), 5),
            (Rect::new(7, 2, 30, 6), 4),
            (Rect::new(7, 2, 30, 3), 1),
            (Rect::new(7, 2, 2, 2), 0),
            (Rect::new(7, 2, 1, 1), 0),
            (Rect::new(7, 2, 0, 0), 0),
        ] {
            let area = prompt.area(pane);
            assert!(inside(area, pane), "{pane:?}: {area:?}");
            assert_eq!(area.width, pane.width.saturating_sub(2), "{pane:?}: the pane's inside width");
            assert_eq!(area.height, height, "{pane:?}: clipped to the rows inside the pane's borders");
            if pane.height >= 2 {
                assert_eq!(area.bottom(), pane.bottom() - 1, "{pane:?}: on the bottom border");
                assert!(area.y > pane.y, "{pane:?}: below the top border");
            }
        }
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        let pane = Rect::new(7, 2, 30, 20);
        terminal.draw(|frame| prompt.render(frame, pane)).unwrap();
        let buf = terminal.backend().buffer().clone();
        let area = prompt.area(pane);
        assert_eq!(buf[(pane.x + 1, area.y)].symbol(), "\u{256d}", "the border starts inside the pane's");
        assert_eq!(buf[(pane.right() - 2, area.y)].symbol(), "\u{256e}");
        assert_eq!(buf[(pane.x + 1, area.bottom() - 1)].symbol(), "\u{2570}");
        assert_eq!(buf[(pane.right() - 2, pane.bottom() - 2)].symbol(), "\u{256f}", "on the row above the pane's border");
        for (x, y) in [(pane.x, area.y), (pane.right() - 1, area.y), (pane.x + 1, pane.bottom() - 1), (pane.right(), area.y)] {
            assert_eq!(buf[(x, y)].symbol(), " ", "({x}, {y}): the pane's border cells are left alone");
        }
    }

    #[test]
    fn it_draws_title_value_in_the_validator_colour_error_and_hint() {
        let prompt = due("=1d", Some("due: \"=1d\" is not a date"), Some(theme::RED));
        let buf = draw(&prompt, 80, 20);
        let area = prompt.area(Rect::new(0, 0, 80, 20));
        assert!(row_text(&buf, area.y).contains(" due "), "{:?}", row_text(&buf, area.y));
        assert_eq!(buf[(area.x, area.y)].symbol(), "\u{256d}");

        let input = area.y + 1;
        let x = find(&buf, input, "=1d").expect("value on the first row");
        assert_eq!(x, area.x + 2, "after the border and the padding");
        for i in 0..3 {
            assert_eq!(buf[(x + i, input)].fg, theme::RED, "value cell {i}");
        }
        assert_eq!(buf[(x + 3, input)].bg, theme::SAPPHIRE, "the cursor follows the value");

        let error = area.y + 2;
        let ex = find(&buf, error, "due: \"=1d\" is not a date").expect("error beneath the input");
        assert_eq!(buf[(ex, error)].fg, theme::RED);

        let hint = area.y + 3;
        let hx = find(&buf, hint, "2026-10-20, +3d, +1w or clear").expect("hint line");
        assert_eq!(buf[(hx, hint)].fg, theme::OVERLAY);
    }

    #[test]
    fn a_valid_value_takes_the_green_and_no_validator_the_text_colour() {
        let area = due("", None, None).area(Rect::new(0, 0, 80, 20));
        let (input, error) = (area.y + 1, area.y + 2);
        let buf = draw(&due("+1d", None, Some(theme::GREEN)), 80, 20);
        let x = find(&buf, input, "+1d").expect("value");
        assert_eq!(buf[(x, input)].fg, theme::GREEN);
        let inside: String = row_text(&buf, error).trim().trim_matches('\u{2502}').to_string();
        assert!(inside.trim().is_empty(), "no error, a blank row: {inside:?}");

        let buf = draw(&due("draft", None, None), 80, 20);
        let x = find(&buf, input, "draft").expect("value");
        assert_eq!(buf[(x, input)].fg, theme::TEXT);
    }

    #[test]
    fn a_small_terminal_clips_the_box_to_the_frame() {
        for (w, h) in [(20, 3), (10, 10), (1, 1), (0, 0), (44, 5)] {
            let prompt = due("+1d", Some("due: \"x\" is not a date"), Some(theme::RED));
            let full = Rect::new(0, 0, w, h);
            let area = prompt.area(full);
            assert!(area.right() <= full.right() && area.bottom() <= full.bottom(), "{w}x{h}: {area:?}");
            draw(&prompt, w, h);
        }
        let buf = draw(&due("+1d", None, None), 20, 7);
        assert_eq!(row_text(&buf, 1), format!(" \u{256d} due {} ", "\u{2500}".repeat(11) + "\u{256e}"), "the box fills the pane's inside");
        assert_eq!(row_text(&buf, 5).chars().nth(1), Some('\u{2570}'));
    }

    fn confirm(body: &str) -> Confirm<'_> {
        Confirm { title: "delete?", body, hint: "y / enter delete, n / esc cancel" }
    }

    /// NZ-77: the confirm box draws its red title, the body and the dim
    /// hint in the prompt's box, docked at the bottom of the pane (NZ-77a).
    #[test]
    fn the_confirm_box_draws_a_red_title_the_body_and_the_hint() {
        let dialog = confirm("call the bank");
        let pane = Rect::new(5, 2, 60, 20);
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|frame| dialog.render(frame, pane)).unwrap();
        let buf = terminal.backend().buffer().clone();
        let area = dialog.area(pane);
        assert_eq!(area, prompt_area_like(pane), "the same box as a prompt");
        assert_eq!(area, Rect::new(6, 16, 58, 5), "across the pane, just above its bottom border");
        assert_eq!(buf[(area.x, area.bottom() - 1)].symbol(), "\u{2570}");
        assert_eq!(buf[(area.x - 1, area.y)].symbol(), " ", "the pane's border cell is left alone");
        assert_eq!(buf[(area.x, area.y)].symbol(), "\u{256d}");
        let tx = find(&buf, area.y, "delete?").expect("title on the top border");
        assert_eq!(buf[(tx, area.y)].fg, theme::RED);

        let bx = find(&buf, area.y + 1, "call the bank").expect("body on the first row");
        assert_eq!(bx, area.x + 2);
        assert_eq!(buf[(bx, area.y + 1)].fg, theme::TEXT);
        let hx = find(&buf, area.y + 3, "y / enter delete, n / esc cancel").expect("hint row");
        assert_eq!(buf[(hx, area.y + 3)].fg, theme::OVERLAY);
    }

    fn prompt_area_like(pane: Rect) -> Rect {
        Prompt { title: "delete?", hint: "", buffer: "", cursor: 0, error: None, color: None }.area(pane)
    }

    #[test]
    fn a_long_confirm_body_is_cut_to_the_box_and_a_tiny_pane_clips_it() {
        let long = "b".repeat(80) + "TAIL";
        let dialog = confirm(&long);
        let buf = draw_confirm(&dialog, 50, 10);
        let area = dialog.area(Rect::new(0, 0, 50, 10));
        assert_eq!(area.width, 48, "the box fills the pane's inside");
        let row = row_text(&buf, area.y + 1);
        assert!(!row.contains("TAIL"), "{row:?}");
        assert_eq!(find(&buf, area.y + 1, "\u{2026}"), Some(area.right() - 3), "cut with an ellipsis inside the padding");
        assert_eq!(buf[(area.right() - 1, area.y + 1)].symbol(), "\u{2502}", "the border is kept");

        for (w, h) in [(20, 3), (10, 10), (2, 2), (1, 1), (0, 0)] {
            let full = Rect::new(0, 0, w, h);
            let area = dialog.area(full);
            assert!(inside(area, full), "{w}x{h}: {area:?}");
            assert_eq!(area, prompt_area_like(full), "{w}x{h}: clipped as a prompt is");
            draw_confirm(&dialog, w, h);
        }
    }

    fn draw_confirm(dialog: &Confirm, width: u16, height: u16) -> Buffer {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| dialog.render(frame, frame.area())).unwrap();
        terminal.backend().buffer().clone()
    }

    #[test]
    fn a_long_value_scrolls_to_keep_the_cursor_in_the_box() {
        let long = "a".repeat(60) + "END";
        let prompt = Prompt { cursor: long.len(), ..due(&long, None, None) };
        let buf = draw(&prompt, 30, 10);
        let area = prompt.area(Rect::new(0, 0, 30, 10));
        let x = find(&buf, area.y + 1, "END").expect("the value's end stays visible");
        assert_eq!(buf[(x + 3, area.y + 1)].bg, theme::SAPPHIRE);
        assert!(x + 3 < area.right() - 1);
    }
}
