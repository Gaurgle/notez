//! Note creation: filenames, headers, body templates.

use chrono::Local;
use serde::{Deserialize, Serialize};

/// A new note to be written.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    pub title: String,
    pub body: Option<String>,
}

impl Note {
    /// Construct a new note. Empty title is replaced with `"untitled"`.
    pub fn new(title: impl Into<String>, body: Option<String>) -> Self {
        let title = title.into();
        let title = if title.trim().is_empty() {
            "untitled".to_string()
        } else {
            title
        };
        Self { title, body }
    }

    /// Filename for this note: `sanitized-title.md`, or
    /// `YYYY-MM-DD-sanitized-title.md` when `dated`. Only quick notes are
    /// dated; the creation date of any other note is in its header.
    pub fn filename(&self, dated: bool) -> String {
        let slug = crate::util::sanitize::name(&self.title);
        if dated {
            format!("{}-{}.md", Local::now().format("%Y-%m-%d"), slug)
        } else {
            format!("{slug}.md")
        }
    }

    /// Full file contents to write. Includes a header with title and date,
    /// then the body if provided.
    pub fn rendered(&self) -> String {
        let date = Local::now().format("%Y-%m-%d");
        let mut out = format!("# {}\n\nDate: {}\n\n", self.title, date);
        if let Some(body) = &self.body {
            out.push_str(body.trim_end());
            out.push('\n');
        }
        out
    }
}

/// Append a timestamped log entry to today's daily log content.
///
/// Returns the new full file contents. If `existing` is empty, prepends a
/// `# Daily Log - YYYY-MM-DD` header.
pub fn append_log_entry(existing: &str, message: &str) -> String {
    let now = Local::now();
    let date = now.format("%Y-%m-%d");
    let time = now.format("%H:%M");

    let mut out = if existing.is_empty() {
        format!("# Daily Log - {}\n\n", date)
    } else {
        let mut s = existing.to_string();
        if !s.ends_with('\n') {
            s.push('\n');
        }
        s
    };
    out.push_str(&format!("{} - {}\n", time, message));
    out
}

/// Today's daily log filename: `YYYY-MM-DD-daily-log.md`.
pub fn todays_log_filename() -> String {
    let date = Local::now().format("%Y-%m-%d");
    format!("{}-daily-log.md", date)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_title_becomes_untitled() {
        let n = Note::new("", None);
        assert_eq!(n.title, "untitled");
    }

    #[test]
    fn undated_filename_is_only_the_sanitized_title() {
        let n = Note::new("My Idea!", None);
        assert_eq!(n.filename(false), "my-idea.md");
    }

    #[test]
    fn dated_filename_has_date_and_sanitized_title() {
        let n = Note::new("My Idea!", None);
        let fname = n.filename(true);
        assert!(fname.ends_with("-my-idea.md"), "got {}", fname);
        // Starts with YYYY-MM-DD
        let prefix = &fname[..10];
        assert!(prefix.chars().nth(4) == Some('-'));
        assert!(prefix.chars().nth(7) == Some('-'));
    }

    #[test]
    fn rendered_contains_title_and_date() {
        let n = Note::new("Hello", Some("some body".to_string()));
        let s = n.rendered();
        assert!(s.starts_with("# Hello\n"));
        assert!(s.contains("Date: "));
        assert!(s.contains("some body"));
    }

    #[test]
    fn rendered_without_body_still_has_header() {
        let n = Note::new("hi", None);
        let s = n.rendered();
        assert!(s.starts_with("# hi\n"));
        assert!(s.contains("Date: "));
    }

    #[test]
    fn append_log_to_empty_creates_header() {
        let out = append_log_entry("", "first message");
        assert!(out.starts_with("# Daily Log - "));
        assert!(out.contains("first message"));
    }

    #[test]
    fn append_log_to_existing_appends() {
        let existing = "# Daily Log - 2026-05-13\n\n09:00 - first\n";
        let out = append_log_entry(existing, "second");
        assert!(out.starts_with("# Daily Log - 2026-05-13\n"));
        assert!(out.contains("09:00 - first"));
        assert!(out.contains(" - second"));
    }

    #[test]
    fn append_log_handles_missing_trailing_newline() {
        let existing = "# Daily Log - 2026-05-13\n\n09:00 - first";
        let out = append_log_entry(existing, "second");
        // Should not run lines together.
        assert!(out.contains("09:00 - first\n"));
    }

    #[test]
    fn todays_log_filename_format() {
        let f = todays_log_filename();
        assert!(f.ends_with("-daily-log.md"));
        assert_eq!(f.len(), "YYYY-MM-DD".len() + "-daily-log.md".len());
    }
}
