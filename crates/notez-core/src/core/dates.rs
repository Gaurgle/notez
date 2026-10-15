//! Dates on notes and todos: the two accepted shapes and their type.
//!
//! A date is `YYYY-MM-DD` or `YYYY-MM-DDTHH:MM`, local wall-clock time with
//! no zone. Anything else is not a date: [`NoteDate::parse`] returns `None`
//! and callers keep the raw text verbatim, so a malformed value is never an
//! error. Design: `docs/design-nz47-dates.md`.

use std::fmt;
use std::path::Path;

use chrono::{Datelike, NaiveDate, NaiveTime, Timelike};
use serde::{Deserialize, Serialize};

use crate::core::frontmatter;

/// How much of a file [`NoteDates::read`] reads. A frontmatter block or
/// `Date:` line beyond this is not seen.
const HEAD_BYTES: u64 = 4096;

/// How many body lines (after any frontmatter) may hold the `Date:` line.
/// `notez add` writes it on the third.
const DATE_LINE_WINDOW: usize = 5;

/// The three optional dates of a note. Read on demand (an inspector, an
/// agenda), never while listing notes, so `collect_all` stays a pure walk.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteDates {
    /// When the note was made: the `created:` frontmatter key, else a
    /// `Date: YYYY-MM-DD` line near the top, else a `YYYY-MM-DD-` or
    /// `YYYY-MM-DD-HHMMSS-` filename prefix. Never the file's mtime.
    #[serde(default)]
    pub created: Option<NoteDate>,
    /// The `due:` frontmatter key, a deadline.
    #[serde(default)]
    pub due: Option<NoteDate>,
    /// The `event:` frontmatter key, when the thing the note is about happens.
    #[serde(default)]
    pub event: Option<NoteDate>,
}

impl NoteDates {
    /// The dates of a note with contents `text` and file name `file_name`.
    /// Pure; a malformed value counts as absent, and a malformed `created`
    /// falls through to the next source.
    pub fn from_text(text: &str, file_name: &str) -> NoteDates {
        let (fields, body) = frontmatter::split(text);
        let field = |key| frontmatter::get(&fields, key).and_then(NoteDate::parse);
        NoteDates {
            created: field("created")
                .or_else(|| date_line(body))
                .or_else(|| filename_date(file_name)),
            due: field("due"),
            event: field("event"),
        }
    }

    /// The dates of the note at `path`, from at most its first 4 KiB. A read
    /// error gives no dates at all. Read-only.
    pub fn read(path: &Path) -> NoteDates {
        let Ok(head) = read_head(path) else {
            return NoteDates::default();
        };
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        NoteDates::from_text(&head, name)
    }
}

fn read_head(path: &Path) -> std::io::Result<String> {
    use std::io::Read;
    let mut buf = Vec::with_capacity(HEAD_BYTES as usize);
    std::fs::File::open(path)?
        .take(HEAD_BYTES)
        .read_to_end(&mut buf)?;
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

/// The `Date: YYYY-MM-DD` line among the first lines of the body.
fn date_line(body: &str) -> Option<NoteDate> {
    body.lines().take(DATE_LINE_WINDOW).find_map(|line| {
        let value = line.trim().strip_prefix("Date:")?;
        NoteDate::parse(value.trim())
    })
}

/// A `YYYY-MM-DD-` filename prefix, or Pinz's `YYYY-MM-DD-HHMMSS-` (seconds
/// dropped; an impossible time keeps just the date).
fn filename_date(name: &str) -> Option<NoteDate> {
    let mut date = NoteDate::parse(name.get(..10)?)?;
    if name.as_bytes().get(10) != Some(&b'-') {
        return None;
    }
    let stamp = name
        .get(11..17)
        .filter(|s| s.bytes().all(|b| b.is_ascii_digit()));
    if let (Some(stamp), Some(b'-')) = (stamp, name.as_bytes().get(17)) {
        let hour = stamp[0..2].parse().unwrap_or(99);
        let minute = stamp[2..4].parse().unwrap_or(99);
        let second = stamp[4..6].parse().unwrap_or(99);
        if NaiveTime::from_hms_opt(hour, minute, second).is_some() {
            date.time = NaiveTime::from_hms_opt(hour, minute, 0);
        }
    }
    Some(date)
}

/// A calendar date with an optional time of day.
///
/// Serializes as its text form (`"2026-10-20"` or `"2026-10-20T14:00"`), the
/// same string that is written in a note or on a todo line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(into = "String", try_from = "String")]
pub struct NoteDate {
    pub date: NaiveDate,
    pub time: Option<NaiveTime>,
}

impl NoteDate {
    /// Parse exactly `YYYY-MM-DD` or `YYYY-MM-DDTHH:MM`. No surrounding
    /// whitespace, no seconds, no zone; an impossible date such as
    /// `2026-13-45` is `None`.
    pub fn parse(text: &str) -> Option<Self> {
        let bytes = text.as_bytes();
        let date = match bytes.len() {
            10 | 16 => parse_date(&bytes[..10])?,
            _ => return None,
        };
        let time = if bytes.len() == 16 {
            if bytes[10] != b'T' || bytes[13] != b':' {
                return None;
            }
            let hour = digits(&bytes[11..13])?;
            let minute = digits(&bytes[14..16])?;
            Some(NaiveTime::from_hms_opt(hour, minute, 0)?)
        } else {
            None
        };
        Some(NoteDate { date, time })
    }
}

/// `YYYY-MM-DD` from exactly ten bytes.
fn parse_date(bytes: &[u8]) -> Option<NaiveDate> {
    if bytes[4] != b'-' || bytes[7] != b'-' {
        return None;
    }
    let year = digits(&bytes[0..4])?;
    let month = digits(&bytes[5..7])?;
    let day = digits(&bytes[8..10])?;
    NaiveDate::from_ymd_opt(year as i32, month, day)
}

/// The value of an all-ASCII-digit slice, or `None` if any byte is not a digit.
fn digits(bytes: &[u8]) -> Option<u32> {
    bytes.iter().try_fold(0u32, |acc, b| {
        b.is_ascii_digit().then(|| acc * 10 + u32::from(b - b'0'))
    })
}

impl fmt::Display for NoteDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let d = self.date;
        write!(f, "{:04}-{:02}-{:02}", d.year(), d.month(), d.day())?;
        if let Some(t) = self.time {
            write!(f, "T{:02}:{:02}", t.hour(), t.minute())?;
        }
        Ok(())
    }
}

impl From<NoteDate> for String {
    fn from(date: NoteDate) -> String {
        date.to_string()
    }
}

impl TryFrom<String> for NoteDate {
    type Error = String;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        NoteDate::parse(&text).ok_or_else(|| format!("not a date: {text:?}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_date_only() {
        let d = NoteDate::parse("2026-10-20").unwrap();
        assert_eq!(d.date, NaiveDate::from_ymd_opt(2026, 10, 20).unwrap());
        assert_eq!(d.time, None);
        assert_eq!(d.to_string(), "2026-10-20");
    }

    #[test]
    fn parses_date_and_time() {
        let d = NoteDate::parse("2026-11-02T14:00").unwrap();
        assert_eq!(d.time, NaiveTime::from_hms_opt(14, 0, 0));
        assert_eq!(d.to_string(), "2026-11-02T14:00");
    }

    #[test]
    fn rejects_everything_else() {
        for bad in [
            "",
            "2026",
            "2026-1-02",
            "2026-13-45",
            "2026-02-30",
            "2026-10-20T25:00",
            "2026-10-20T14:60",
            "2026-10-20T14:00:00",
            "2026-10-20 14:00",
            "2026-10-20Z",
            " 2026-10-20",
            "2026-10-20 ",
            "next friday",
            "+026-10-20",
            "２０２６-10-20",
        ] {
            assert_eq!(NoteDate::parse(bad), None, "{bad:?} should not parse");
        }
    }

    #[test]
    fn serde_uses_the_text_form() {
        let d = NoteDate::parse("2026-11-02T09:05").unwrap();
        let toml = toml::to_string(&Wrap { d }).unwrap();
        assert_eq!(toml.trim(), "d = \"2026-11-02T09:05\"");
        let back: Wrap = toml::from_str(&toml).unwrap();
        assert_eq!(back.d, d);
        assert!(toml::from_str::<Wrap>("d = \"tomorrow\"").is_err());
    }

    fn date(text: &str) -> Option<NoteDate> {
        Some(NoteDate::parse(text).unwrap())
    }

    fn triple(d: NoteDates) -> (Option<NoteDate>, Option<NoteDate>, Option<NoteDate>) {
        (d.created, d.due, d.event)
    }

    #[test]
    fn note_dates_come_from_frontmatter() {
        let text = "---\ncreated: 2026-10-15\ndue: 2026-10-20\nevent: 2026-11-02T14:00\n---\n# Book\n\nDate: 2026-01-01\n";
        assert_eq!(
            triple(NoteDates::from_text(text, "2025-05-05-book.md")),
            (
                date("2026-10-15"),
                date("2026-10-20"),
                date("2026-11-02T14:00")
            )
        );
    }

    #[test]
    fn note_dates_from_a_pinz_pin() {
        let text =
            "---\nx: -359\ny: -749\nz: 131\ncolor: peach\ndue: 2026-10-20\n---\n# Title\n\nbody";
        assert_eq!(
            triple(NoteDates::from_text(text, "2026-10-14-093000-title.md")),
            (date("2026-10-14T09:30"), date("2026-10-20"), None)
        );
    }

    #[test]
    fn malformed_values_are_absent_and_created_falls_back() {
        let text = "---\ncreated: yesterday\ndue: next friday\nevent: 2026-13-45\n---\n# T\n\nDate: 2026-10-15\n";
        assert_eq!(
            triple(NoteDates::from_text(text, "t.md")),
            (date("2026-10-15"), None, None)
        );
    }

    #[test]
    fn created_falls_back_to_date_line_then_filename() {
        let created = |text: &str, name: &str| NoteDates::from_text(text, name).created;
        assert_eq!(
            created("# Title\n\nDate: 2026-10-15\n\nbody\n", "2026-01-01-x.md"),
            date("2026-10-15")
        );
        assert_eq!(
            created("# Title\n\nbody\n", "2026-01-01-x.md"),
            date("2026-01-01")
        );
        assert_eq!(
            created("# x\n", "2026-01-01-251500-x.md"),
            date("2026-01-01"),
            "an impossible HHMMSS keeps just the date"
        );
        assert_eq!(created("# x\n", "2026-01-01.md"), None);
        assert_eq!(created("# x\n", "notes.md"), None);
        assert_eq!(created("", "ü-2026-01-01-x.md"), None);
    }

    #[test]
    fn date_line_deep_in_the_body_is_not_created() {
        let text = format!("# Title\n\n{}Date: 2026-10-15\n", "line\n".repeat(10));
        assert_eq!(NoteDates::from_text(&text, "x.md").created, None);
    }

    #[test]
    fn read_uses_the_file_name_and_contents() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("2026-01-01-x.md");
        std::fs::write(&path, "---\ndue: 2026-10-20\n---\n# x\n").unwrap();
        assert_eq!(
            triple(NoteDates::read(&path)),
            (date("2026-01-01"), date("2026-10-20"), None)
        );
    }

    #[test]
    fn read_sees_only_the_first_4_kib() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.md");
        let text = format!("---\n{}due: 2026-10-20\n---\n# T\n", "pad: x\n".repeat(700));
        std::fs::write(&path, &text).unwrap();
        assert_eq!(NoteDates::read(&path).due, None);
        assert_eq!(NoteDates::from_text(&text, "x.md").due, date("2026-10-20"));
    }

    #[test]
    fn read_error_gives_no_dates() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("2026-01-01-gone.md");
        assert_eq!(NoteDates::read(&missing), NoteDates::default());
    }

    #[derive(Serialize, Deserialize)]
    struct Wrap {
        d: NoteDate,
    }
}
