//! The footer's one-off status message, shared by the tree browser and the
//! todo board. A success (`vault synced`, `copied 7 B`, `reloaded`) clears
//! by itself after [`EXPIRY`]; anything else (an error, a refusal, a held
//! file) stays until the next key, since the user has to read it. The
//! event loop does the clearing: [`Notice::wait`] shortens its poll while a
//! success shows and [`Notice::expire`] drops it once the time is up.

use std::time::{Duration, Instant};

use super::sync::Finished;

/// How long a success message shows.
pub const EXPIRY: Duration = Duration::from_secs(2);

/// The footer message and, for a success, when it goes.
#[derive(Debug, Default)]
pub struct Notice {
    text: Option<String>,
    until: Option<Instant>,
}

impl Notice {
    /// Show `text` until the next key (`None` clears the footer).
    pub fn set(&mut self, text: Option<String>) {
        self.text = text;
        self.until = None;
    }

    /// Show the success `text` for [`EXPIRY`] from `now`; a new message
    /// starts the time again.
    pub fn flash(&mut self, text: String, now: Instant) {
        self.text = Some(text);
        self.until = Some(now + EXPIRY);
    }

    /// Show a finished sync's result: a success flashes, the rest stays.
    pub fn finished(&mut self, done: &Finished, now: Instant) {
        if done.succeeded() {
            self.flash(done.message(), now);
        } else {
            self.set(Some(done.message()));
        }
    }

    pub fn as_deref(&self) -> Option<&str> {
        self.text.as_deref()
    }

    pub fn is_some(&self) -> bool {
        self.text.is_some()
    }

    pub fn is_none(&self) -> bool {
        self.text.is_none()
    }

    /// Clear a success whose time is up at `now`. True when it cleared, so
    /// the view redraws.
    pub fn expire(&mut self, now: Instant) -> bool {
        match self.until {
            Some(until) if now >= until => {
                self.set(None);
                true
            }
            _ => false,
        }
    }

    /// How long the event loop may wait for input: `wait`, or less while a
    /// success shows, so it clears on time.
    pub fn wait(&self, wait: Duration, now: Instant) -> Duration {
        match self.until {
            Some(until) => wait.min(until.saturating_duration_since(now)),
            None => wait,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_success_clears_after_the_expiry_and_an_error_stays() {
        let t0 = Instant::now();
        let mut notice = Notice::default();
        notice.flash("vault synced".to_string(), t0);
        assert!(!notice.expire(t0 + EXPIRY - Duration::from_millis(1)));
        assert_eq!(notice.as_deref(), Some("vault synced"));
        assert!(notice.expire(t0 + EXPIRY));
        assert!(notice.is_none());

        notice.set(Some("not synced: the vault has no upstream".to_string()));
        assert!(!notice.expire(t0 + EXPIRY * 100));
        assert_eq!(notice.as_deref(), Some("not synced: the vault has no upstream"));
    }

    #[test]
    fn a_new_message_resets_the_timer() {
        let t0 = Instant::now();
        let mut notice = Notice::default();
        notice.flash("reloaded".to_string(), t0);
        let later = t0 + Duration::from_millis(1500);
        notice.flash("copied path".to_string(), later);
        assert!(!notice.expire(t0 + EXPIRY));
        assert_eq!(notice.as_deref(), Some("copied path"));
        assert!(notice.expire(later + EXPIRY));

        // An error over a live success keeps the error.
        notice.flash("reloaded".to_string(), t0);
        notice.set(Some("copy failed: no clipboard".to_string()));
        assert!(!notice.expire(t0 + EXPIRY));
        assert_eq!(notice.as_deref(), Some("copy failed: no clipboard"));
    }

    #[test]
    fn the_wait_shortens_only_while_a_success_shows() {
        let t0 = Instant::now();
        let idle = Duration::from_secs(5);
        let mut notice = Notice::default();
        assert_eq!(notice.wait(idle, t0), idle);
        notice.flash("reloaded".to_string(), t0);
        let at = t0 + Duration::from_millis(500);
        assert_eq!(notice.wait(idle, at), EXPIRY - Duration::from_millis(500));
        assert_eq!(notice.wait(Duration::from_millis(100), at), Duration::from_millis(100));
        assert_eq!(notice.wait(idle, t0 + EXPIRY * 2), Duration::ZERO);
        notice.set(Some("save failed".to_string()));
        assert_eq!(notice.wait(idle, at), idle);
    }
}
