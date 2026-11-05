//! The copy chord's shared parts: the footer message and the cursor row's
//! flash after a copy (the sweep, then the fade), used by the tree browser
//! and the todo board alike.

use std::time::{Duration, Instant};

use crossterm::event::{Event, KeyEventKind, MouseEventKind};
use ratatui::prelude::*;
use ratatui::widgets::ListState;

use super::notice::Notice;
use super::theme;

/// Show a copy's footer message: a copy flashes, a failure stays. True
/// for a copy, which also flashes the cursor row (see [`YankFlash`]).
pub(super) fn show_yank(status: &mut Notice, message: Option<String>, now: Instant) -> bool {
    match message {
        Some(text) if text.starts_with("copied") => {
            status.flash(text, now);
            true
        }
        other => {
            status.set(other);
            false
        }
    }
}

/// How long a copy's [`theme::yanked_row`] takes to sweep over the cursor
/// row: `yy` from the left edge, `yp` from the right.
pub(super) const YANK_SWEEP: Duration = Duration::from_millis(100);
/// How long the swept row then takes to fade back to
/// [`theme::selected_row`].
pub(super) const YANK_FADE: Duration = Duration::from_millis(200);
/// The whole copy flash: the sweep, then the fade.
pub(super) const YANK_FLASH: Duration = YANK_SWEEP.saturating_add(YANK_FADE);
/// The redraw interval while the flash is live: 8 frames over the sweep.
pub(super) const YANK_FRAME: Duration = Duration::from_millis(12);

/// The cursor row's one flash after a copy: when it started and which way
/// it sweeps. Read at draw time; while it lasts the event loop cuts its
/// wait to the next frame with [`YankFlash::wait`] and redraws when
/// [`YankFlash::tick`] says so.
#[derive(Debug, Default)]
pub(super) struct YankFlash {
    start: Option<Instant>,
    from_right: bool,
}

impl YankFlash {
    /// Start the flash at `now` when `copied`, sweeping from the right edge
    /// when `from_right`; a failed copy ends any flash.
    pub(super) fn show(&mut self, copied: bool, from_right: bool, now: Instant) {
        self.start = copied.then_some(now);
        self.from_right = from_right;
    }

    pub(super) fn clear(&mut self) {
        self.start = None;
    }

    /// Time into a live flash at `now`, or `None` once it is over.
    fn elapsed(&self, now: Instant) -> Option<Duration> {
        let elapsed = now.saturating_duration_since(self.start?);
        (elapsed < YANK_FLASH).then_some(elapsed)
    }

    /// The cursor row's style at `now`: [`theme::selected_row`] under the
    /// sweep (see [`YankFlash::sweep`]), then fading from
    /// [`theme::yanked_row`] back to it.
    pub(super) fn row_style(&self, now: Instant) -> Style {
        match self.elapsed(now) {
            Some(elapsed) if elapsed >= YANK_SWEEP => theme::fading_row(
                (elapsed - YANK_SWEEP).as_secs_f64() / YANK_FADE.as_secs_f64(),
            ),
            _ => theme::selected_row(),
        }
    }

    /// How far the sweep has filled the cursor row at `now`; `None` before
    /// a copy and once the row is full.
    pub(super) fn sweep(&self, now: Instant) -> Option<Sweep> {
        let elapsed = self.elapsed(now).filter(|&e| e < YANK_SWEEP)?;
        Some(Sweep {
            reached: elapsed.as_secs_f64() / YANK_SWEEP.as_secs_f64(),
            from_right: self.from_right,
        })
    }

    /// True when the view should redraw at `now`: the flash is live, or its
    /// time just ran out, which ends it.
    pub(super) fn tick(&mut self, now: Instant) -> bool {
        if self.start.is_none() {
            return false;
        }
        if self.elapsed(now).is_none() {
            self.start = None;
        }
        true
    }

    /// `wait`, or no longer than the next frame while the flash lasts, or
    /// no wait at all once its time is up but [`YankFlash::tick`] has not
    /// ended it yet, so a late frame does not leave the row a shade off.
    pub(super) fn wait(&self, wait: Duration, now: Instant) -> Duration {
        match self.elapsed(now) {
            Some(elapsed) => wait.min(YANK_FRAME).min(YANK_FLASH - elapsed),
            None if self.start.is_some() => Duration::ZERO,
            None => wait,
        }
    }
}

/// True for the input that ends a copy flash: a key press, a click or a
/// scroll. Pointer motion (reported with mouse capture on), resize, focus
/// and key releases leave it running.
pub(super) fn ends_yank_flash(ev: &Event) -> bool {
    match ev {
        Event::Key(key) => key.kind == KeyEventKind::Press,
        Event::Mouse(mouse) => mouse.kind != MouseEventKind::Moved,
        Event::Paste(_) => true,
        Event::Resize(..) | Event::FocusGained | Event::FocusLost => false,
    }
}

/// How far a copy flash has swept over the cursor row: `reached` (0..=1)
/// of its columns, counted from the left edge, or from the right.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Sweep {
    pub(super) reached: f64,
    pub(super) from_right: bool,
}

/// Paint `sweep` over the cursor row of a list of `rows` rows just drawn
/// into `area` with `state`: the columns behind the front take
/// [`theme::yanked_row`]'s background, under the glyphs' own colours; the
/// column the front is crossing takes a background mixed toward it by how
/// far the front is into that column, so the front slides; the rest keep
/// the row's style. A cursor row scrolled out of `area` is left.
pub(super) fn paint_sweep(
    buf: &mut Buffer,
    area: Rect,
    state: &ListState,
    rows: usize,
    sweep: Sweep,
) {
    let Some(selected) = state.selected().filter(|_| rows > 0) else {
        return;
    };
    let Some(row) = selected.min(rows - 1).checked_sub(state.offset()) else {
        return;
    };
    let Ok(row) = u16::try_from(row) else {
        return;
    };
    if row >= area.height {
        return;
    }
    paint_sweep_over(buf, Rect::new(area.x, area.y + row, area.width, 1), sweep);
}

/// Paint `sweep` over every line of `rect`, the cursor row's lines, as
/// [`paint_sweep`] describes; the front crosses all of them at once.
pub(super) fn paint_sweep_over(buf: &mut Buffer, rect: Rect, sweep: Sweep) {
    let front = sweep.reached.clamp(0.0, 1.0) * f64::from(rect.width);
    let width = (front.floor() as u16).min(rect.width);
    let x = if sweep.from_right {
        rect.right() - width
    } else {
        rect.x
    };
    buf.set_style(Rect::new(x, rect.y, width, rect.height), theme::yanked_row());
    let into = front - front.floor();
    if width < rect.width && into > 0.0 {
        let front_x = if sweep.from_right {
            x - 1
        } else {
            rect.x + width
        };
        let bg = theme::mix(theme::SURFACE0, theme::SURFACE2, into);
        buf.set_style(Rect::new(front_x, rect.y, 1, rect.height), theme::yanked_row().bg(bg));
    }
}
