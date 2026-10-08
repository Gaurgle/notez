//! The tree browser's two panes: the split between the list and the
//! preview, which one has focus, and whether the preview is folded.
//!
//! Pure state and geometry, no drawing and no I/O, so the rules are unit
//! tested here and `tree::event_loop` only asks for the rects it draws into.
//! Mirrors the fleetz panes: a one-column strip between the panes is the
//! border a drag grabs, with a grip mark centred on it (`grip`), the split
//! is the list's share in percent, and folding a pane hands its width to the
//! other.
//!
//! Two folds: the user's own (`folded`, the `2` key) and the automatic one
//! (`auto_folded`, set by `fit` on every draw when the body is narrower than
//! both minimum widths). Either hides the preview. They never overwrite each
//! other: while auto-folded the preview keys are refused, and when room
//! returns the preview comes back exactly as the user left it.
//!
//! Session state only: the split, focus and fold are never saved, so every
//! run starts at 50/50 with the list focused and the preview open.

use crossterm::event::KeyCode;
use ratatui::layout::{Position, Rect};

/// The list's share of the width when the browser opens and after `=`.
pub const DEFAULT_SPLIT: u16 = 50;

/// Points one `<` or `>` moves the split.
pub const SPLIT_STEP: u16 = 5;

/// Narrowest the list pane gets, borders included: room for the tag dots,
/// the highlight symbol and a short name.
pub const MIN_LIST_WIDTH: u16 = 24;

/// Narrowest the preview pane gets, borders included.
pub const MIN_PREVIEW_WIDTH: u16 = 20;

/// Columns of the border strip between the two panes.
pub const BORDER_WIDTH: u16 = 1;

/// Columns either side of the border strip that still grab it, as fleetz
/// `GRAB_SLOP`: a single column is too small a target to hit reliably. The
/// slop lands on the two panes' own border lines, never on a row or the
/// filter strip.
pub const GRAB_SLOP: u16 = 1;

/// Rows of the grip mark: fewer reads as a speck (fleetz `GRIP_ROWS`).
pub const GRIP_ROWS: u16 = 3;

/// The grip mark, one per row: a full braille cell, which stacks into a bar
/// two dots thick (fleetz `GRIP_VERTICAL`).
pub const GRIP: &str = "⠿";

/// Which pane has focus. The number is the key that focuses it and the
/// number its title shows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Pane {
    #[default]
    List,
    Preview,
}

impl Pane {
    /// The pane's number: its focus key and the number in its title.
    pub fn number(self) -> u8 {
        match self {
            Pane::List => 1,
            Pane::Preview => 2,
        }
    }
}

/// The split, focus and fold of the tree browser.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Panes {
    /// The list's share of the width in percent. Kept as asked; `layout`
    /// clamps the widths it yields, and `clamp` snaps it into range.
    pub split: u16,
    pub focus: Pane,
    /// The user folded the preview (`2`): the list takes the full width and
    /// the preview is neither drawn nor read.
    pub folded: bool,
    /// The body is too narrow for both panes, so the preview is folded
    /// whatever `folded` says. Set by `fit` only.
    pub auto_folded: bool,
    /// The border grip is being dragged: the grip is lit and every mouse
    /// event goes to the drag until the button comes up.
    pub dragging: bool,
}

impl Default for Panes {
    fn default() -> Self {
        Self { split: DEFAULT_SPLIT, focus: Pane::List, folded: false, auto_folded: false, dragging: false }
    }
}

/// What a left press over the body hit, checked in this order: the border
/// grab zone, then the pane under the pointer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    /// The border strip or its slop: a drag started.
    Grab,
    /// A pane, now focused.
    Pane(Pane),
    /// Neither: outside the body.
    Outside,
}

impl Panes {
    /// Whether the preview is hidden, by the user or for lack of room.
    pub fn is_folded(&self) -> bool {
        self.folded || self.auto_folded
    }

    /// Fold the preview automatically while `total_width` cannot hold both
    /// panes (`fits`), and unfold it when it can again. Called on every draw.
    /// The user's `folded` is left alone either way. An auto fold moves focus
    /// to the list, the only pane left, and ends a drag.
    pub fn fit(&mut self, total_width: u16) {
        self.auto_folded = !Self::fits(total_width);
        if self.auto_folded {
            self.focus = Pane::List;
            self.dragging = false;
        }
    }

    /// `<`: the list loses `SPLIT_STEP` points. Not clamped to a width; the
    /// caller follows with `clamp`.
    pub fn narrow(&mut self) {
        self.split = self.split.saturating_sub(SPLIT_STEP);
    }

    /// `>`: the list gains `SPLIT_STEP` points, at most 100.
    pub fn widen(&mut self) {
        self.split = (self.split + SPLIT_STEP).min(100);
    }

    /// `=`: back to `DEFAULT_SPLIT`.
    pub fn reset(&mut self) {
        self.split = DEFAULT_SPLIT;
    }

    /// Snap `split` to the nearest percentage whose widths in `total_width`
    /// columns keep the list at `MIN_LIST_WIDTH` or more and the preview at
    /// `MIN_PREVIEW_WIDTH` or more. When `total_width` cannot hold both, the
    /// list wins. Snapping the number itself (rather than only the widths)
    /// means a `>` after a run of `<` past the limit moves at once.
    pub fn clamp(&mut self, total_width: u16) {
        if total_width == 0 {
            return;
        }
        let (low, high) = split_bounds(total_width);
        self.split = self.split.clamp(low, high.max(low));
    }

    /// Whether `total_width` columns hold both panes at their minimum widths
    /// and the border between them. Below this the preview should fold.
    pub fn fits(total_width: u16) -> bool {
        total_width >= MIN_LIST_WIDTH + BORDER_WIDTH + MIN_PREVIEW_WIDTH
    }

    /// `Tab`: focus the other pane. With the preview folded there is no
    /// other pane, so focus stays on the list.
    pub fn cycle_focus(&mut self) {
        self.focus = match self.focus {
            Pane::List if !self.is_folded() => Pane::Preview,
            _ => Pane::List,
        };
    }

    /// Focus `pane`. Focusing a folded preview unfolds it, unless it is
    /// auto-folded: then there is no room for it and nothing changes.
    pub fn focus(&mut self, pane: Pane) {
        if pane == Pane::Preview {
            if self.auto_folded {
                return;
            }
            self.folded = false;
        }
        self.focus = pane;
    }

    /// Fold or unfold the preview. Folding moves focus to the list, the only
    /// pane left; unfolding leaves focus where it is.
    pub fn toggle_fold(&mut self) {
        self.folded = !self.folded;
        if self.folded {
            self.focus = Pane::List;
        }
    }

    /// `2`: focus the preview; on the focused preview, fold it; on a folded
    /// preview, unfold and focus it. Refused while auto-folded, so the
    /// user's fold choice is not flipped by a key that shows nothing.
    pub fn press_preview_key(&mut self) {
        if self.auto_folded {
            return;
        }
        if self.focus == Pane::Preview && !self.folded {
            self.toggle_fold();
        } else {
            self.focus(Pane::Preview);
        }
    }

    /// The pane keys of browse mode: `<` `>` `=` the split, `1` `2` `Tab`
    /// the focus and fold. Returns false for any other key, which the caller
    /// handles. `total_width` is the width both panes share, for `clamp`.
    pub fn handle_key(&mut self, code: KeyCode, total_width: u16) -> bool {
        match code {
            KeyCode::Char('<') => {
                self.narrow();
                self.clamp(total_width);
            }
            KeyCode::Char('>') => {
                self.widen();
                self.clamp(total_width);
            }
            KeyCode::Char('=') => self.reset(),
            KeyCode::Char('1') => self.focus(Pane::List),
            KeyCode::Char('2') => self.press_preview_key(),
            KeyCode::Tab => self.cycle_focus(),
            _ => return false,
        }
        true
    }

    /// Width of the list pane in `total_width` columns: `split` percent,
    /// rounded half up (the rounding the old 50/50 `Layout` used), then held
    /// between the minimum widths, the list winning when both cannot fit.
    /// A folded preview gives the list the full width.
    pub fn list_width(&self, total_width: u16) -> u16 {
        if self.is_folded() {
            return total_width;
        }
        let wanted = percent_of(total_width, self.split);
        let max = total_width.saturating_sub(BORDER_WIDTH + MIN_PREVIEW_WIDTH);
        wanted.min(max).max(MIN_LIST_WIDTH).min(total_width)
    }

    /// The list rect, the border strip between the panes (one column, full
    /// height) and the preview rect, `None` when folded. Folded, the border
    /// is an empty rect on the right edge. The three widths add up to
    /// `total.width`.
    pub fn layout(&self, total: Rect) -> (Rect, Rect, Option<Rect>) {
        let list_width = self.list_width(total.width);
        let list = Rect { width: list_width, ..total };
        if self.is_folded() {
            let border = Rect { x: total.x + total.width, width: 0, ..total };
            return (list, border, None);
        }
        let border_width = BORDER_WIDTH.min(total.width - list_width);
        let border = Rect { x: total.x + list_width, width: border_width, ..total };
        let preview = Rect {
            x: border.x + border_width,
            width: total.width - list_width - border_width,
            ..total
        };
        (list, border, Some(preview))
    }

    /// Whether the cell at (`x`, `y`) is on the border strip of `total`'s
    /// layout. Never true while the preview is folded.
    pub fn border_hit(&self, total: Rect, x: u16, y: u16) -> bool {
        let (_, border, _) = self.layout(total);
        x >= border.x && x < border.x + border.width && y >= border.y && y < border.y + border.height
    }

    /// Whether a press at (`x`, `y`) grabs the border: the strip itself or
    /// `GRAB_SLOP` columns either side, over the full body height. Never
    /// while the preview is folded.
    pub fn grab_hit(&self, total: Rect, x: u16, y: u16) -> bool {
        let (_, border, preview) = self.layout(total);
        preview.is_some()
            && border.width > 0
            && x.abs_diff(border.x) <= GRAB_SLOP
            && y >= border.y
            && y < border.y + border.height
    }

    /// The pane containing (`x`, `y`) in `total`'s layout, for routing the
    /// wheel and clicks. The border strip belongs to neither pane.
    pub fn pane_at(&self, total: Rect, x: u16, y: u16) -> Option<Pane> {
        let (list, _, preview) = self.layout(total);
        let at = Position { x, y };
        if list.contains(at) {
            Some(Pane::List)
        } else if preview.is_some_and(|preview| preview.contains(at)) {
            Some(Pane::Preview)
        } else {
            None
        }
    }

    /// A left press at (`x`, `y`): grabbing the border starts a drag and
    /// leaves focus alone; otherwise the pane under the pointer is focused.
    pub fn press(&mut self, total: Rect, x: u16, y: u16) -> Press {
        if self.grab_hit(total, x, y) {
            self.dragging = true;
            return Press::Grab;
        }
        match self.pane_at(total, x, y) {
            Some(pane) => {
                self.focus(pane);
                Press::Pane(pane)
            }
            None => Press::Outside,
        }
    }

    /// While dragging, the pointer is at column `x`: the split becomes the
    /// share of `total` left of it, rounded to the nearest percent and then
    /// clamped, so a pointer past either edge parks the border at its limit.
    /// Focus and fold are untouched.
    pub fn drag_to(&mut self, total: Rect, x: u16) {
        if total.width == 0 {
            return;
        }
        let width = u32::from(total.width);
        let offset = u32::from(x.saturating_sub(total.x)).min(width);
        self.split = ((offset * 100 + width / 2) / width) as u16;
        self.clamp(total.width);
    }

    /// The cells of the grip mark: a `GRIP_ROWS` tall column on the border
    /// strip, centred on the body. `None` while the preview is folded or the
    /// body is shorter than the grip. Drawn from the same `layout` that
    /// `grab_hit` tests, so the mark never sits on a cell that grabs nothing.
    pub fn grip(&self, total: Rect) -> Option<Rect> {
        let (_, border, preview) = self.layout(total);
        if preview.is_none() || border.width == 0 || total.height < GRIP_ROWS {
            return None;
        }
        let y = total.y + (total.height - GRIP_ROWS) / 2;
        Some(Rect { y, height: GRIP_ROWS, ..border })
    }
}

/// `percent` of `width`, rounded half up.
fn percent_of(width: u16, percent: u16) -> u16 {
    ((u32::from(width) * u32::from(percent) + 50) / 100) as u16
}

/// The lowest and highest split whose list width in `total_width` columns
/// stays within the minimum widths. `low > high` when both cannot fit.
fn split_bounds(total_width: u16) -> (u16, u16) {
    let max_list = total_width.saturating_sub(BORDER_WIDTH + MIN_PREVIEW_WIDTH);
    let low = (0..=100)
        .find(|&p| percent_of(total_width, p) >= MIN_LIST_WIDTH)
        .unwrap_or(100);
    let high = (0..=100)
        .rev()
        .find(|&p| percent_of(total_width, p) <= max_list)
        .unwrap_or(0);
    (low, high)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::{Constraint, Direction, Layout};

    fn area(width: u16) -> Rect {
        Rect::new(2, 1, width, 30)
    }

    fn widths(panes: &Panes, width: u16) -> (u16, u16, u16) {
        let (list, border, preview) = panes.layout(area(width));
        (list.width, border.width, preview.map_or(0, |p| p.width))
    }

    #[test]
    fn defaults_are_an_open_50_50_split_with_the_list_focused() {
        let panes = Panes::default();
        assert_eq!(panes.split, 50);
        assert_eq!(panes.focus, Pane::List);
        assert!(!panes.folded);
    }

    #[test]
    fn narrow_and_widen_step_five_points_and_reset_returns_to_50() {
        let mut panes = Panes::default();
        panes.narrow();
        assert_eq!(panes.split, 45);
        panes.narrow();
        assert_eq!(panes.split, 40);
        panes.widen();
        assert_eq!(panes.split, 45);
        panes.reset();
        assert_eq!(panes.split, 50);
        panes.split = 98;
        panes.widen();
        assert_eq!(panes.split, 100);
        panes.split = 3;
        panes.narrow();
        assert_eq!(panes.split, 0);
    }

    #[test]
    fn the_default_list_width_matches_the_old_50_50_layout() {
        for width in 0..=300u16 {
            let old = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(area(width));
            // Below the minimum list width the clamp takes over on purpose.
            if old[0].width < MIN_LIST_WIDTH {
                continue;
            }
            let (list, _, _) = Panes::default().layout(area(width));
            assert_eq!(list, old[0], "width {width}");
        }
    }

    #[test]
    fn clamp_keeps_both_panes_usable_at_several_widths() {
        for width in [45u16, 60, 80, 120, 200, 400] {
            for split in [0u16, 5, 20, 50, 80, 95, 100] {
                let mut panes = Panes { split, ..Panes::default() };
                panes.clamp(width);
                let (list, border, preview) = widths(&panes, width);
                assert!(list >= MIN_LIST_WIDTH, "width {width} split {split}: list {list}");
                assert!(preview >= MIN_PREVIEW_WIDTH, "width {width} split {split}: preview {preview}");
                assert_eq!(list + border + preview, width);
                // The snapped split yields that width on its own.
                assert_eq!(list, percent_of(width, panes.split), "width {width} split {split}");
            }
        }
    }

    #[test]
    fn clamp_leaves_a_split_already_in_range_alone() {
        let mut panes = Panes { split: 35, ..Panes::default() };
        panes.clamp(120);
        assert_eq!(panes.split, 35);
    }

    #[test]
    fn a_widen_after_narrowing_past_the_limit_moves_at_once() {
        let mut panes = Panes::default();
        for _ in 0..20 {
            assert!(panes.handle_key(KeyCode::Char('<'), 80));
        }
        let floor = widths(&panes, 80).0;
        assert_eq!(floor, MIN_LIST_WIDTH);
        panes.handle_key(KeyCode::Char('>'), 80);
        assert!(widths(&panes, 80).0 > floor);
    }

    #[test]
    fn the_list_wins_when_the_width_cannot_hold_both() {
        for width in [0u16, 10, 24, 30, 44] {
            let mut panes = Panes::default();
            panes.clamp(width);
            let (list, border, preview) = widths(&panes, width);
            assert_eq!(list, MIN_LIST_WIDTH.min(width), "width {width}");
            assert_eq!(list + border + preview, width, "width {width}");
        }
    }

    #[test]
    fn layout_clamps_even_an_unclamped_split() {
        let panes = Panes { split: 100, ..Panes::default() };
        assert_eq!(widths(&panes, 100), (79, 1, 20));
        let panes = Panes { split: 0, ..Panes::default() };
        assert_eq!(widths(&panes, 100), (24, 1, 75));
    }

    #[test]
    fn fits_needs_both_minimums_and_the_border() {
        assert!(!Panes::fits(0));
        assert!(!Panes::fits(44));
        assert!(Panes::fits(45));
        assert!(Panes::fits(200));
    }

    #[test]
    fn layout_widths_add_up_and_the_border_sits_between_the_panes() {
        for width in [45u16, 76, 77, 120, 201] {
            for split in [30u16, 50, 70] {
                let panes = Panes { split, ..Panes::default() };
                let total = area(width);
                let (list, border, preview) = panes.layout(total);
                let preview = preview.expect("open preview");
                assert_eq!(list.width + border.width + preview.width, width);
                assert_eq!(list.x, total.x);
                assert_eq!(border.x, list.x + list.width);
                assert_eq!(border.width, 1);
                assert_eq!(preview.x, border.x + 1);
                for rect in [list, border, preview] {
                    assert_eq!((rect.y, rect.height), (total.y, total.height));
                }
            }
        }
    }

    #[test]
    fn a_folded_preview_has_no_rect_and_the_list_takes_the_full_width() {
        let panes = Panes { folded: true, ..Panes::default() };
        let total = area(120);
        let (list, border, preview) = panes.layout(total);
        assert_eq!(list, total);
        assert_eq!(border.width, 0);
        assert!(preview.is_none());
    }

    #[test]
    fn border_hit_is_the_strip_only_and_never_while_folded() {
        let total = area(100);
        let panes = Panes::default();
        let (list, border, _) = panes.layout(total);
        assert!(panes.border_hit(total, border.x, total.y));
        assert!(panes.border_hit(total, border.x, total.y + total.height - 1));
        assert!(!panes.border_hit(total, border.x - 1, total.y + 5));
        assert!(!panes.border_hit(total, border.x + 1, total.y + 5));
        assert!(!panes.border_hit(total, border.x, total.y + total.height));
        assert!(!panes.border_hit(total, border.x, total.y - 1));
        assert_eq!(border.x, list.x + list.width);
        let folded = Panes { folded: true, ..panes };
        for x in 0..total.x + total.width + 1 {
            assert!(!folded.border_hit(total, x, total.y + 5), "x {x}");
        }
    }

    #[test]
    fn tab_cycles_focus_and_stays_on_the_list_while_folded() {
        let mut panes = Panes::default();
        panes.cycle_focus();
        assert_eq!(panes.focus, Pane::Preview);
        panes.cycle_focus();
        assert_eq!(panes.focus, Pane::List);
        panes.folded = true;
        panes.cycle_focus();
        assert_eq!(panes.focus, Pane::List);
        assert!(panes.folded);
    }

    #[test]
    fn folding_moves_focus_to_the_list_and_unfolding_leaves_it() {
        let mut panes = Panes { focus: Pane::Preview, ..Panes::default() };
        panes.toggle_fold();
        assert!(panes.folded);
        assert_eq!(panes.focus, Pane::List);
        panes.toggle_fold();
        assert!(!panes.folded);
        assert_eq!(panes.focus, Pane::List);
    }

    #[test]
    fn focusing_a_folded_preview_unfolds_it() {
        let mut panes = Panes { folded: true, ..Panes::default() };
        panes.focus(Pane::List);
        assert!(panes.folded);
        panes.focus(Pane::Preview);
        assert!(!panes.folded);
        assert_eq!(panes.focus, Pane::Preview);
    }

    fn after(keys: &[KeyCode]) -> Panes {
        let mut panes = Panes::default();
        for &code in keys {
            assert!(panes.handle_key(code, 120), "{code:?} not handled");
        }
        panes
    }

    #[test]
    fn keys_1_2_and_tab_set_the_focus() {
        assert_eq!(after(&[KeyCode::Char('2')]).focus, Pane::Preview);
        assert_eq!(after(&[KeyCode::Char('2'), KeyCode::Char('1')]).focus, Pane::List);
        assert_eq!(after(&[KeyCode::Char('1')]).focus, Pane::List);
        assert_eq!(after(&[KeyCode::Tab]).focus, Pane::Preview);
        assert_eq!(after(&[KeyCode::Tab, KeyCode::Tab]).focus, Pane::List);
        assert!(!after(&[KeyCode::Char('2'), KeyCode::Char('1')]).folded);
    }

    #[test]
    fn key_2_twice_folds_and_a_third_time_unfolds_and_focuses() {
        let folded = after(&[KeyCode::Char('2'), KeyCode::Char('2')]);
        assert!(folded.folded);
        assert_eq!(folded.focus, Pane::List);
        let reopened = after(&[KeyCode::Char('2'), KeyCode::Char('2'), KeyCode::Char('2')]);
        assert!(!reopened.folded);
        assert_eq!(reopened.focus, Pane::Preview);
        // `1` on a folded preview keeps it folded.
        let still = after(&[KeyCode::Char('2'), KeyCode::Char('2'), KeyCode::Char('1')]);
        assert!(still.folded);
    }

    #[test]
    fn split_keys_move_clamp_and_reset() {
        assert_eq!(after(&[KeyCode::Char('<')]).split, 45);
        assert_eq!(after(&[KeyCode::Char('>')]).split, 55);
        assert_eq!(after(&[KeyCode::Char('>'), KeyCode::Char('=')]).split, 50);
        let wide = after(&[KeyCode::Char('>'); 20]);
        let (list, _, preview) = widths(&wide, 120);
        // The split moves in whole percent, so the snapped preview lands
        // within one percent step (two columns at 120) of its minimum.
        assert!((MIN_PREVIEW_WIDTH..MIN_PREVIEW_WIDTH + 2).contains(&preview), "preview {preview}");
        assert_eq!(list + 1 + preview, 120);
    }

    // --- Pass 2: grip, drag, mouse routing, auto-fold ---

    #[test]
    fn grip_cells_all_grab_the_border_and_sit_centred() {
        for (width, height) in [(45u16, 3u16), (80, 4), (120, 30), (201, 51)] {
            for split in [30u16, 50, 70] {
                let mut panes = Panes { split, ..Panes::default() };
                panes.clamp(width);
                let total = Rect::new(2, 1, width, height);
                let grip = panes.grip(total).expect("open preview, tall enough");
                assert_eq!((grip.width, grip.height), (1, GRIP_ROWS));
                for y in grip.y..grip.y + grip.height {
                    assert!(panes.border_hit(total, grip.x, y), "{width}x{height} split {split} y {y}");
                    assert!(panes.grab_hit(total, grip.x, y));
                    assert_eq!(panes.pane_at(total, grip.x, y), None);
                }
                let above = grip.y - total.y;
                let below = total.y + total.height - (grip.y + grip.height);
                assert!(above == below || above + 1 == below, "{width}x{height}: {above} above, {below} below");
            }
        }
    }

    #[test]
    fn no_grip_while_folded_or_too_short() {
        let total = Rect::new(0, 0, 120, 30);
        assert!(Panes { folded: true, ..Panes::default() }.grip(total).is_none());
        assert!(Panes { auto_folded: true, ..Panes::default() }.grip(total).is_none());
        assert!(Panes::default().grip(Rect::new(0, 0, 120, 2)).is_none());
    }

    #[test]
    fn grab_hit_is_the_strip_with_one_column_of_slop_each_side() {
        let total = area(100);
        let panes = Panes::default();
        let (list, border, preview) = panes.layout(total);
        let preview = preview.unwrap();
        let y = total.y + 5;
        assert!(panes.grab_hit(total, border.x - 1, y), "the list's right border line");
        assert!(panes.grab_hit(total, border.x, y));
        assert!(panes.grab_hit(total, border.x + 1, y), "the preview's left border line");
        assert!(!panes.grab_hit(total, border.x - 2, y));
        assert!(!panes.grab_hit(total, border.x + 2, y));
        assert!(!panes.grab_hit(total, border.x, total.y - 1));
        assert!(!panes.grab_hit(total, border.x, total.y + total.height));
        // The slop covers only the two border lines, never pane content.
        assert_eq!(border.x - 1, list.x + list.width - 1);
        assert_eq!(border.x + 1, preview.x);
        for folded in [Panes { folded: true, ..panes }, Panes { auto_folded: true, ..panes }] {
            for x in 0..total.x + total.width + 2 {
                assert!(!folded.grab_hit(total, x, y), "x {x}");
            }
        }
    }

    #[test]
    fn a_press_on_the_border_starts_a_drag_and_keeps_focus() {
        let total = area(100);
        for focus in [Pane::List, Pane::Preview] {
            let mut panes = Panes { focus, ..Panes::default() };
            let (_, border, _) = panes.layout(total);
            assert_eq!(panes.press(total, border.x - 1, total.y + 3), Press::Grab);
            assert!(panes.dragging);
            assert_eq!(panes.focus, focus);
        }
    }

    #[test]
    fn a_press_in_a_pane_focuses_it() {
        let total = area(100);
        let mut panes = Panes::default();
        let (list, _, preview) = panes.layout(total);
        let preview = preview.unwrap();
        assert_eq!(panes.press(total, preview.x + 5, total.y + 4), Press::Pane(Pane::Preview));
        assert_eq!(panes.focus, Pane::Preview);
        assert!(!panes.dragging);
        assert_eq!(panes.press(total, list.x + 3, total.y + 4), Press::Pane(Pane::List));
        assert_eq!(panes.focus, Pane::List);
        assert_eq!(panes.press(total, 0, 0), Press::Outside);
        assert_eq!(panes.focus, Pane::List);
        // Folded, every column of the body is the list.
        let mut folded = Panes { folded: true, focus: Pane::List, ..Panes::default() };
        let x = total.x + total.width - 1;
        assert_eq!(folded.press(total, x, total.y + 4), Press::Pane(Pane::List));
        assert!(folded.folded);
    }

    #[test]
    fn pane_at_routes_by_rect_at_any_split() {
        let total = area(120);
        for split in [30u16, 50, 70] {
            let panes = Panes { split, ..Panes::default() };
            let (list, border, preview) = panes.layout(total);
            let preview = preview.unwrap();
            let y = total.y + 10;
            assert_eq!(panes.pane_at(total, list.x, y), Some(Pane::List));
            assert_eq!(panes.pane_at(total, list.x + list.width - 1, y), Some(Pane::List));
            assert_eq!(panes.pane_at(total, border.x, y), None);
            assert_eq!(panes.pane_at(total, preview.x, y), Some(Pane::Preview));
            assert_eq!(panes.pane_at(total, preview.x + preview.width - 1, y), Some(Pane::Preview));
            assert_eq!(panes.pane_at(total, preview.x + preview.width, y), None);
            assert_eq!(panes.pane_at(total, list.x, total.y + total.height), None);
        }
        let folded = Panes { folded: true, ..Panes::default() };
        assert_eq!(folded.pane_at(total, total.x + total.width - 1, total.y), Some(Pane::List));
    }

    #[test]
    fn dragging_puts_the_border_under_the_pointer() {
        let total = Rect::new(2, 1, 100, 30);
        let mut panes = Panes::default();
        for x in [40u16, 52, 60, 75] {
            panes.drag_to(total, x);
            let (_, border, _) = panes.layout(total);
            assert_eq!(border.x, x, "x {x}");
        }
        // At 120 columns a percent is 1.2 columns, so within one column.
        let total = Rect::new(2, 1, 120, 30);
        for x in [40u16, 53, 61, 77, 90] {
            panes.drag_to(total, x);
            let (_, border, _) = panes.layout(total);
            assert!(border.x.abs_diff(x) <= 1, "x {x}: border at {}", border.x);
        }
    }

    #[test]
    fn dragging_past_either_edge_clamps() {
        let total = Rect::new(2, 1, 100, 30);
        let mut panes = Panes::default();
        for x in [0u16, 1, 2, 10] {
            panes.drag_to(total, x);
            assert_eq!(widths(&panes, 100).0, MIN_LIST_WIDTH, "x {x}");
        }
        for x in [95u16, 101, 102, 500, u16::MAX] {
            panes.drag_to(total, x);
            assert_eq!(widths(&panes, 100).2, MIN_PREVIEW_WIDTH, "x {x}");
        }
        // The split itself is snapped, so a key moves at once afterwards.
        let snapped = panes.split;
        panes.handle_key(KeyCode::Char('<'), 100);
        assert_eq!(panes.split, snapped - SPLIT_STEP);
    }

    #[test]
    fn dragging_leaves_focus_and_fold_alone() {
        let total = area(100);
        let mut panes = Panes { focus: Pane::Preview, dragging: true, ..Panes::default() };
        panes.drag_to(total, 30);
        assert_eq!(panes.focus, Pane::Preview);
        assert!(!panes.folded && panes.dragging);
    }

    #[test]
    fn fit_folds_when_narrow_and_unfolds_when_room_returns() {
        let mut panes = Panes { focus: Pane::Preview, ..Panes::default() };
        panes.fit(44);
        assert!(panes.auto_folded && panes.is_folded());
        assert!(!panes.folded, "the user's fold is not touched");
        assert_eq!(panes.focus, Pane::List);
        assert!(panes.layout(area(44)).2.is_none());
        panes.fit(45);
        assert!(!panes.auto_folded && !panes.is_folded());
        assert!(panes.layout(area(45)).2.is_some());
        // Room returning does not take focus back to the preview.
        assert_eq!(panes.focus, Pane::List);
    }

    #[test]
    fn a_user_fold_survives_the_auto_fold_and_room_returning() {
        let mut panes = after(&[KeyCode::Char('2'), KeyCode::Char('2')]);
        assert!(panes.folded);
        panes.fit(30);
        panes.fit(120);
        assert!(panes.folded && !panes.auto_folded && panes.is_folded());
    }

    #[test]
    fn the_preview_keys_are_refused_while_auto_folded() {
        for folded in [false, true] {
            let mut panes = Panes { folded, ..Panes::default() };
            panes.fit(30);
            for code in [KeyCode::Char('2'), KeyCode::Tab] {
                assert!(panes.handle_key(code, 30), "{code:?} is still a pane key");
                assert_eq!(panes.focus, Pane::List, "{code:?}");
                assert_eq!(panes.folded, folded, "{code:?} changed the user's fold");
            }
            panes.focus(Pane::Preview);
            assert_eq!(panes.focus, Pane::List);
            assert_eq!(panes.folded, folded);
            // Room returns: the preview is back as the user left it.
            panes.fit(120);
            assert_eq!(panes.is_folded(), folded);
        }
    }

    #[test]
    fn an_auto_fold_ends_a_drag() {
        let mut panes = Panes { dragging: true, ..Panes::default() };
        panes.fit(120);
        assert!(panes.dragging);
        panes.fit(30);
        assert!(!panes.dragging);
    }

    #[test]
    fn other_keys_are_not_pane_keys() {
        let mut panes = Panes::default();
        for code in [KeyCode::Char('3'), KeyCode::Char('j'), KeyCode::Enter, KeyCode::BackTab] {
            assert!(!panes.handle_key(code, 120), "{code:?}");
        }
        assert_eq!(panes, Panes::default());
    }
}
