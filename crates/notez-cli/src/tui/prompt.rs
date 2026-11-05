//! The popup prompt: one typed value in a titled box drawn centred over the
//! list pane (pane 1), never over the preview. The footer under it keeps its hints, so
//! typing never moves it. Key handling stays in each TUI's loop; this only
//! draws.

use ratatui::prelude::*;
use ratatui::widgets::{Block, BorderType, Borders, Clear, Padding, Paragraph};

use super::footer::clip_to_cols;
use super::theme;

/// The rows inside the border: the input, the error (blank while there is
/// none, so the box does not jump when one appears) and the hint.
const ROWS: u16 = 3;

/// The narrowest the box's text area gets, so a short value still reads as
/// a dialog rather than a tooltip.
const MIN_TEXT_COLS: usize = 40;

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
    /// The box, centred in `full` and clipped to it.
    pub fn area(&self, full: Rect) -> Rect {
        let widest = [
            self.title.chars().count() + 2,
            self.hint.chars().count(),
            self.buffer.chars().count() + 1,
            self.error.map_or(0, |e| e.chars().count()),
        ]
        .into_iter()
        .max()
        .unwrap_or(0);
        centred(widest, full)
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
    /// The box, centred in `full` and clipped to it.
    pub fn area(&self, full: Rect) -> Rect {
        let widest = [self.title.chars().count() + 2, self.hint.chars().count(), self.body.chars().count()]
            .into_iter()
            .max()
            .unwrap_or(0);
        centred(widest, full)
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

/// A box `widest` text columns wide (at least [`MIN_TEXT_COLS`]) and
/// [`ROWS`] high, centred in `full` and clipped to it.
fn centred(widest: usize, full: Rect) -> Rect {
    // Borders (2) and one column of padding each side (2).
    let widest = widest.max(MIN_TEXT_COLS);
    let w = u16::try_from(widest + 4).unwrap_or(u16::MAX).min(full.width);
    let h = (ROWS + 2).min(full.height);
    let x = full.x + full.width.saturating_sub(w) / 2;
    let y = full.y + full.height.saturating_sub(h) / 2;
    Rect::new(x, y, w, h)
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

    #[test]
    fn the_box_is_centred_in_the_frame() {
        let prompt = due("+1d", None, None);
        let full = Rect::new(0, 0, 100, 30);
        let area = prompt.area(full);
        assert_eq!((area.width, area.height), (44, 5));
        assert_eq!((area.x, area.y), (28, 12));
        let offset = prompt.area(Rect::new(10, 4, 100, 30));
        assert_eq!((offset.x, offset.y), (38, 16));
    }

    /// NZ-72: the TUIs pass the list pane's rect, so the box sits centred in
    /// it, never over the preview beside it, and a narrow pane clips it.
    #[test]
    fn the_box_is_centred_in_a_pane_and_clipped_to_it() {
        let prompt = due("+1d", None, None);
        let inside = |area: Rect, pane: Rect| {
            area.x >= pane.x && area.y >= pane.y && area.right() <= pane.right() && area.bottom() <= pane.bottom()
        };
        let pane = Rect::new(0, 1, 60, 29);
        let area = prompt.area(pane);
        assert!(inside(area, pane), "{area:?}");
        assert_eq!((area.x, area.y), (8, 13), "centred: (60 - 44) / 2, 1 + (29 - 5) / 2");
        assert_eq!(area.x - pane.x, pane.right() - area.right());

        let right_pane = Rect::new(61, 1, 50, 29);
        let area = prompt.area(right_pane);
        assert!(inside(area, right_pane), "{area:?}");
        assert_eq!(area.x, 64);

        for pane in [Rect::new(7, 2, 30, 20), Rect::new(7, 2, 30, 3), Rect::new(7, 2, 1, 1), Rect::new(7, 2, 0, 0)] {
            let area = prompt.area(pane);
            assert!(inside(area, pane), "{pane:?}: {area:?}");
            assert_eq!(area.width, pane.width.min(44), "{pane:?}: the box fills a narrow pane");
        }
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        let pane = Rect::new(7, 2, 30, 20);
        terminal.draw(|frame| prompt.render(frame, pane)).unwrap();
        let buf = terminal.backend().buffer().clone();
        let area = prompt.area(pane);
        assert_eq!(buf[(pane.x, area.y)].symbol(), "\u{256d}", "the border starts at the pane's edge");
        assert_eq!(buf[(pane.right() - 1, area.y)].symbol(), "\u{256e}");
        assert_eq!(buf[(pane.right(), area.y)].symbol(), " ", "nothing drawn past the pane");
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
        let buf = draw(&due("+1d", None, Some(theme::GREEN)), 80, 20);
        let x = find(&buf, 8, "+1d").expect("value");
        assert_eq!(buf[(x, 8)].fg, theme::GREEN);
        let inside: String = row_text(&buf, 9).trim().trim_matches('\u{2502}').to_string();
        assert!(inside.trim().is_empty(), "no error, a blank row: {inside:?}");

        let buf = draw(&due("draft", None, None), 80, 20);
        let x = find(&buf, 8, "draft").expect("value");
        assert_eq!(buf[(x, 8)].fg, theme::TEXT);
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
        let buf = draw(&due("+1d", None, None), 20, 3);
        assert_eq!(row_text(&buf, 0).chars().next(), Some('\u{256d}'), "the box fills the frame");
    }

    fn confirm(body: &str) -> Confirm<'_> {
        Confirm { title: "delete?", body, hint: "y / enter delete, n / esc cancel" }
    }

    /// NZ-77: the confirm box draws its red title, the body and the dim
    /// hint in the prompt's box, centred in the pane.
    #[test]
    fn the_confirm_box_draws_a_red_title_the_body_and_the_hint() {
        let dialog = confirm("call the bank");
        let pane = Rect::new(5, 2, 60, 20);
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|frame| dialog.render(frame, pane)).unwrap();
        let buf = terminal.backend().buffer().clone();
        let area = dialog.area(pane);
        assert_eq!(area, prompt_area_like(pane), "the same box as a prompt");
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
        assert_eq!(area.width, 50, "the box fills the narrow frame");
        let row = row_text(&buf, area.y + 1);
        assert!(!row.contains("TAIL"), "{row:?}");
        assert_eq!(find(&buf, area.y + 1, "\u{2026}"), Some(area.right() - 3), "cut with an ellipsis inside the padding");
        assert_eq!(buf[(area.right() - 1, area.y + 1)].symbol(), "\u{2502}", "the border is kept");

        for (w, h) in [(20, 3), (10, 10), (1, 1), (0, 0)] {
            let full = Rect::new(0, 0, w, h);
            let area = dialog.area(full);
            assert!(area.right() <= full.right() && area.bottom() <= full.bottom(), "{w}x{h}: {area:?}");
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
