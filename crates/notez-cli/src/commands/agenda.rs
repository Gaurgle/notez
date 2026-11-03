//! `notez agenda`: dated notes and due todos across every scope, overdue
//! first, then one group per day. Design: `docs/design-nz47-dates.md`.

use std::path::{Path, PathBuf};

use anyhow::Result;
use chrono::{Datelike, Days, NaiveDate, Timelike};
use console::Style;
use serde_json::{json, Value};

use notez_core::config::{Config, NotezMetadata, ProjectRegistry};
use notez_core::core::aggregate::{collect_all, NoteEntry, SourceKind};
use notez_core::core::dates::{NoteDate, NoteDates};
use notez_core::core::{frontmatter, Scope};
use notez_core::todo::{load_board, CheckState, Task};

/// How much of a note is read for its dates and title, the same limit as
/// [`NoteDates::read`].
const HEAD_BYTES: u64 = 4096;

/// What puts an entry on the agenda.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AgendaKind {
    /// A note's `due:` date.
    Due,
    /// A note's `event:` date.
    Event,
    /// A todo task's `@date`.
    Todo,
}

impl AgendaKind {
    fn word(self) -> &'static str {
        match self {
            AgendaKind::Due => "due",
            AgendaKind::Event => "event",
            AgendaKind::Todo => "todo",
        }
    }
}

/// One dated line of the agenda.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgendaEntry {
    pub when: NoteDate,
    pub kind: AgendaKind,
    /// The note title or the task text.
    pub title: String,
    /// Scope and project, the way `notez search` labels a hit; a task's
    /// section.
    pub label: String,
    /// The note file, or the `TODO.md` the task lives in.
    pub path: PathBuf,
    /// `local`, `personal`, `public` or `global`; `docs` for a project doc.
    pub scope: String,
    /// The owning project; `None` for a global note or todo.
    pub project: Option<String>,
}

/// The agenda for a window: entries before today, ascending, then the days
/// from today on that have entries, each sorted by time.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Agenda {
    pub overdue: Vec<AgendaEntry>,
    pub days: Vec<(NaiveDate, Vec<AgendaEntry>)>,
}

impl Agenda {
    pub fn is_empty(&self) -> bool {
        self.overdue.is_empty() && self.days.is_empty()
    }
}

/// Sort `entries` into the agenda for `today` through `today + days`
/// inclusive. Later entries are dropped. Within a day an untimed entry
/// comes first, then by time, kind and title.
pub fn agenda(today: NaiveDate, days: u32, mut entries: Vec<AgendaEntry>) -> Agenda {
    let last = today
        .checked_add_days(Days::new(u64::from(days)))
        .unwrap_or(NaiveDate::MAX);
    entries.retain(|e| e.when.date <= last);
    entries.sort_by(|a, b| (a.when, a.kind, &a.title).cmp(&(b.when, b.kind, &b.title)));

    let mut out = Agenda::default();
    for entry in entries {
        if entry.when.date < today {
            out.overdue.push(entry);
            continue;
        }
        match out.days.last_mut() {
            Some((day, group)) if *day == entry.when.date => group.push(entry),
            _ => out.days.push((entry.when.date, vec![entry])),
        }
    }
    out
}

/// Whether `path` lies in a daily-logs folder (`dir`, `01_daily-logs` by
/// default) in any scope. Daily logs never appear on the agenda.
pub fn is_daily_log(path: &Path, dir: &str) -> bool {
    path.components().any(|c| c.as_os_str() == dir)
}

/// The agenda entries of the board's tasks: every task with a due date
/// that is not checked. Header rows are skipped; a half-done task counts.
/// `notez_root` places each task's `TODO.md` in its scope.
pub fn todo_entries(tasks: &[Task], notez_root: &Path) -> Vec<AgendaEntry> {
    tasks
        .iter()
        .filter(|t| !t.is_header && t.state != CheckState::Checked)
        .filter_map(|t| {
            let (scope, project) = todo_scope(t, notez_root);
            Some(AgendaEntry {
                when: t.due?,
                kind: AgendaKind::Todo,
                title: t.text.clone(),
                label: t.section.clone(),
                path: t.source.clone(),
                scope: scope.to_string(),
                project,
            })
        })
        .collect()
}

/// The scope and project of a board task, from where `load_board` found
/// its `TODO.md`: `<root>/personal/<project>/` is personal, anything else
/// under the notez root (its own `TODO.md`, `_todos/<category>/`) is global
/// with no project, `<repo>/.notez/` is local and `<repo>/notez/` public.
fn todo_scope(task: &Task, notez_root: &Path) -> (Scope, Option<String>) {
    let project = Some(task.section.clone());
    match task.source.strip_prefix(notez_root) {
        Ok(rest) if rest.starts_with("personal") => (Scope::Personal, project),
        Ok(_) => (Scope::Global, None),
        Err(_) if task.source.parent().and_then(Path::file_name) == Some(".notez".as_ref()) => {
            (Scope::Local, project)
        }
        Err(_) => (Scope::Public, project),
    }
}

/// The agenda entries of one note: its `due` and its `event`, if set. A
/// note with neither (only `created`, or nothing) gives none.
pub fn note_entries(dates: &NoteDates, title: &str, note: &NoteEntry) -> Vec<AgendaEntry> {
    [(dates.due, AgendaKind::Due), (dates.event, AgendaKind::Event)]
        .into_iter()
        .filter_map(|(when, kind)| {
            Some(AgendaEntry {
                when: when?,
                kind,
                title: title.to_string(),
                label: note_label(note),
                path: note.path.clone(),
                scope: note_source(note),
                project: note.project.clone(),
            })
        })
        .collect()
}

/// The agenda entries of `notes`, each read through `read_head` (its first
/// bytes, `None` when unreadable). Notes in a daily-logs folder named
/// `daily_logs_dir` are skipped without being read.
pub fn notes_entries(
    notes: &[NoteEntry],
    daily_logs_dir: &str,
    read_head: impl Fn(&Path) -> Option<String>,
) -> Vec<AgendaEntry> {
    let mut entries = Vec::new();
    for note in notes {
        if is_daily_log(&note.path, daily_logs_dir) {
            continue;
        }
        let Some(head) = read_head(&note.path) else {
            continue;
        };
        let dates = NoteDates::from_text(&head, &note.name);
        if dates.due.is_none() && dates.event.is_none() {
            continue;
        }
        let title = note_title(&head, &note.path);
        entries.extend(note_entries(&dates, &title, note));
    }
    entries
}

/// A note's title: the first `# ` heading of its body (after any
/// frontmatter), else the file stem.
pub fn note_title(text: &str, path: &Path) -> String {
    let (_, body) = frontmatter::split(text);
    body.lines()
        .find_map(|line| line.strip_prefix("# ").map(str::trim))
        .filter(|title| !title.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| {
            path.file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default()
        })
}

/// The scope label of a note, as `notez search` prints it.
fn note_label(entry: &NoteEntry) -> String {
    let source = note_source(entry);
    match &entry.project {
        Some(p) => format!("{p} ({source})"),
        None => source,
    }
}

/// A note's scope word, `docs` for a project doc.
fn note_source(entry: &NoteEntry) -> String {
    match entry.kind {
        SourceKind::Doc => "docs".to_string(),
        SourceKind::Note => entry.scope.to_string(),
    }
}

fn read_head(path: &Path) -> Option<String> {
    use std::io::Read;
    let mut buf = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take(HEAD_BYTES)
        .read_to_end(&mut buf)
        .ok()?;
    Some(String::from_utf8_lossy(&buf).into_owned())
}

/// `2026-10-20`, or `2026-10-20 14:00` with a time.
fn when_text(when: NoteDate) -> String {
    let d = when.date;
    let mut out = format!("{:04}-{:02}-{:02}", d.year(), d.month(), d.day());
    if let Some(t) = when.time {
        out.push_str(&format!(" {:02}:{:02}", t.hour(), t.minute()));
    }
    out
}

/// The agenda as the `--json` document: `today`, `days`, `overdue` and
/// `days_list` (each `{date, entries}`). An entry has `date`, `time`
/// (`HH:MM` or null), `kind`, `title`, `path`, `scope` and `project` (or
/// null). These names are a contract for scripts; keep them stable.
pub fn agenda_json(today: NaiveDate, days: u32, agenda: &Agenda) -> Value {
    let entries = |group: &[AgendaEntry]| group.iter().map(entry_json).collect::<Vec<_>>();
    json!({
        "today": today.format("%Y-%m-%d").to_string(),
        "days": days,
        "overdue": entries(&agenda.overdue),
        "days_list": agenda
            .days
            .iter()
            .map(|(day, group)| json!({
                "date": day.format("%Y-%m-%d").to_string(),
                "entries": entries(group),
            }))
            .collect::<Vec<_>>(),
    })
}

fn entry_json(entry: &AgendaEntry) -> Value {
    json!({
        "date": entry.when.date.format("%Y-%m-%d").to_string(),
        "time": entry.when.time.map(|t| t.format("%H:%M").to_string()),
        "kind": entry.kind.word(),
        "title": entry.title,
        "path": entry.path.to_string_lossy(),
        "scope": entry.scope,
        "project": entry.project,
    })
}

pub fn run(days: u32, json: bool, config: &Config) -> Result<()> {
    let registry = ProjectRegistry::load().unwrap_or_default();
    let notes = collect_all(config, &registry, &NotezMetadata::default())?;
    let mut entries = notes_entries(&notes, &config.paths.daily_logs_dir, read_head);
    entries.extend(todo_entries(
        &load_board(config, &registry),
        &config.notez_root_path(),
    ));

    let today = chrono::Local::now().date_naive();
    let agenda = agenda(today, days, entries);
    if json {
        println!("{}", serde_json::to_string_pretty(&agenda_json(today, days, &agenda))?);
        return Ok(());
    }
    if agenda.is_empty() {
        let unit = if days == 1 { "day" } else { "days" };
        println!("nothing due or scheduled in the next {days} {unit}");
        return Ok(());
    }

    let heading = Style::new().color256(183).bold();
    let overdue_heading = Style::new().color256(204).bold();
    let date_style = Style::new().color256(110);
    let kind_style = Style::new().color256(244);
    let scope_style = Style::new().color256(140);

    let print_entry = |e: &AgendaEntry| {
        println!(
            "  {:<16}  {:<5}  {}  {}",
            date_style.apply_to(when_text(e.when)),
            kind_style.apply_to(e.kind.word()),
            e.title,
            scope_style.apply_to(&e.label),
        );
    };

    if !agenda.overdue.is_empty() {
        println!("{}", overdue_heading.apply_to("overdue"));
        agenda.overdue.iter().for_each(print_entry);
    }
    for (day, group) in &agenda.days {
        let name = match (*day - today).num_days() {
            0 => format!("today, {}", day.format("%a %Y-%m-%d")),
            1 => format!("tomorrow, {}", day.format("%a %Y-%m-%d")),
            _ => day.format("%a %Y-%m-%d").to_string(),
        };
        println!("{}", heading.apply_to(name));
        group.iter().for_each(print_entry);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn d(s: &str) -> NoteDate {
        NoteDate::parse(s).unwrap()
    }

    fn day(s: &str) -> NaiveDate {
        d(s).date
    }

    fn entry(when: &str, kind: AgendaKind, title: &str) -> AgendaEntry {
        AgendaEntry {
            when: d(when),
            kind,
            title: title.to_string(),
            label: "global".to_string(),
            path: PathBuf::from(format!("/h/notez/{title}.md")),
            scope: "global".to_string(),
            project: None,
        }
    }

    fn titles(entries: &[AgendaEntry]) -> Vec<&str> {
        entries.iter().map(|e| e.title.as_str()).collect()
    }

    const TODAY: &str = "2026-10-26";

    #[test]
    fn overdue_entries_come_first_in_ascending_order() {
        let a = agenda(
            day(TODAY),
            7,
            vec![
                entry("2026-10-25", AgendaKind::Due, "yesterday"),
                entry("2026-10-26", AgendaKind::Due, "today"),
                entry("2026-09-01", AgendaKind::Todo, "long ago"),
                entry("2026-10-20T09:00", AgendaKind::Event, "last week"),
            ],
        );
        assert_eq!(titles(&a.overdue), vec!["long ago", "last week", "yesterday"]);
        assert_eq!(a.days.len(), 1);
        assert_eq!(a.days[0].0, day(TODAY));
        assert_eq!(titles(&a.days[0].1), vec!["today"]);
    }

    #[test]
    fn the_window_includes_today_plus_n_and_not_the_day_after() {
        let a = agenda(
            day(TODAY),
            3,
            vec![
                entry("2026-10-29", AgendaKind::Due, "edge"),
                entry("2026-10-29T23:59", AgendaKind::Event, "edge, late"),
                entry("2026-10-30", AgendaKind::Due, "outside"),
                entry("2026-10-27", AgendaKind::Todo, "tomorrow"),
            ],
        );
        let days: Vec<NaiveDate> = a.days.iter().map(|(d, _)| *d).collect();
        assert_eq!(days, vec![day("2026-10-27"), day("2026-10-29")]);
        assert_eq!(titles(&a.days[1].1), vec!["edge", "edge, late"]);
        assert!(a.overdue.is_empty());
    }

    #[test]
    fn days_zero_is_today_only() {
        let a = agenda(
            day(TODAY),
            0,
            vec![
                entry("2026-10-26", AgendaKind::Due, "today"),
                entry("2026-10-27", AgendaKind::Due, "tomorrow"),
            ],
        );
        assert_eq!(a.days.len(), 1);
        assert_eq!(titles(&a.days[0].1), vec!["today"]);
    }

    #[test]
    fn a_huge_window_saturates_instead_of_overflowing() {
        let a = agenda(
            day(TODAY),
            u32::MAX,
            vec![
                entry("9999-12-31", AgendaKind::Event, "far future"),
                entry("2026-10-25", AgendaKind::Due, "yesterday"),
            ],
        );
        assert_eq!(titles(&a.overdue), vec!["yesterday"]);
        assert_eq!(a.days.len(), 1);
        assert_eq!(titles(&a.days[0].1), vec!["far future"]);
    }

    #[test]
    fn entries_within_a_day_sort_untimed_first_then_by_time() {
        let a = agenda(
            day(TODAY),
            7,
            vec![
                entry("2026-10-28T14:00", AgendaKind::Event, "afternoon"),
                entry("2026-10-28T09:30", AgendaKind::Todo, "morning"),
                entry("2026-10-28", AgendaKind::Due, "all day"),
            ],
        );
        assert_eq!(titles(&a.days[0].1), vec!["all day", "morning", "afternoon"]);
    }

    #[test]
    fn nothing_dated_is_an_empty_agenda() {
        assert!(agenda(day(TODAY), 7, Vec::new()).is_empty());
    }

    fn task(text: &str, state: CheckState, due: Option<&str>) -> Task {
        Task {
            text: text.to_string(),
            state,
            source: PathBuf::from("/v/TODO.md"),
            section: "NOTEZ".to_string(),
            is_header: false,
            depth: 0,
            has_subtasks: false,
            collapsed: false,
            is_code_todo: false,
            flags: 0,
            due: due.map(d),
        }
    }

    #[test]
    fn todo_entries_skip_checked_undated_and_header_rows() {
        let mut header = task("NOTEZ", CheckState::Unchecked, Some("2026-10-27"));
        header.is_header = true;
        let tasks = vec![
            header,
            task("open", CheckState::Unchecked, Some("2026-10-27")),
            task("half", CheckState::Half, Some("2026-10-28T10:00")),
            task("done", CheckState::Checked, Some("2026-10-20")),
            task("undated", CheckState::Unchecked, None),
        ];
        let entries = todo_entries(&tasks, Path::new("/v"));
        assert_eq!(titles(&entries), vec!["open", "half"]);
        assert!(entries.iter().all(|e| e.kind == AgendaKind::Todo));
        assert_eq!(entries[0].label, "NOTEZ");
        assert_eq!(entries[1].when, d("2026-10-28T10:00"));
        assert_eq!(entries[0].path, PathBuf::from("/v/TODO.md"));
    }

    #[test]
    fn daily_logs_are_recognised_in_any_scope() {
        let dir = "01_daily-logs";
        assert!(is_daily_log(Path::new("/h/notez/01_daily-logs/2026-10-26.md"), dir));
        assert!(is_daily_log(
            Path::new("/h/notez/personal/p/01_daily-logs/log.md"),
            dir
        ));
        assert!(is_daily_log(Path::new("/r/.notez/01_daily-logs/x.md"), dir));
        assert!(!is_daily_log(Path::new("/h/notez/ideas/01_daily-logs.md"), dir));
        assert!(!is_daily_log(Path::new("/h/notez/ideas/plan.md"), dir));
    }

    #[test]
    fn a_note_gives_its_due_and_event_and_nothing_for_created_alone() {
        let dates = NoteDates::from_text(
            "---\ncreated: 2026-10-01\ndue: 2026-10-20\nevent: 2026-11-02T14:00\n---\n# Venue\n",
            "venue.md",
        );
        let entries = note_entries(&dates, "Venue", &note_at("/h/notez/venue.md", None));
        assert_eq!(entries.len(), 2);
        assert_eq!((entries[0].kind, entries[0].when), (AgendaKind::Due, d("2026-10-20")));
        assert_eq!(
            (entries[1].kind, entries[1].when),
            (AgendaKind::Event, d("2026-11-02T14:00"))
        );

        let created_only = NoteDates::from_text("# Old\n\nDate: 2026-10-01\n", "2026-10-01-old.md");
        assert!(created_only.created.is_some());
        assert!(note_entries(&created_only, "Old", &note_at("/h/notez/old.md", None)).is_empty());

        let malformed = NoteDates::from_text("---\ndue: next friday\n---\n# X\n", "x.md");
        assert!(note_entries(&malformed, "X", &note_at("/h/notez/x.md", None)).is_empty());
    }

    fn note_at(path: &str, project: Option<&str>) -> NoteEntry {
        let path = PathBuf::from(path);
        NoteEntry {
            name: path.file_name().unwrap().to_string_lossy().into_owned(),
            path,
            scope: notez_core::core::Scope::Personal,
            project: project.map(str::to_string),
            kind: SourceKind::Note,
        }
    }

    #[test]
    fn notes_entries_skip_daily_logs_unreadable_and_undated_notes() {
        let dated = "---\ndue: 2026-10-27\n---\n# Dated\n";
        let notes = vec![
            note_at("/h/notez/personal/p/01_daily-logs/2026-10-26.md", Some("p")),
            note_at("/h/notez/01_daily-logs/2026-10-26.md", None),
            note_at("/h/notez/personal/p/plan.md", Some("p")),
            note_at("/h/notez/personal/p/gone.md", Some("p")),
            note_at("/h/notez/personal/p/plain.md", Some("p")),
        ];
        let read = |path: &Path| match path.file_name()?.to_str()? {
            "gone.md" => None,
            "plain.md" => Some("# Plain\n\nDate: 2026-10-01\n".to_string()),
            _ => Some(dated.to_string()),
        };
        let entries = notes_entries(&notes, "01_daily-logs", read);
        assert_eq!(titles(&entries), vec!["Dated"]);
        assert_eq!(entries[0].label, "p (personal)");
        assert_eq!(entries[0].kind, AgendaKind::Due);
        assert_eq!(entries[0].path, PathBuf::from("/h/notez/personal/p/plan.md"));
        assert_eq!(entries[0].scope, "personal");
        assert_eq!(entries[0].project.as_deref(), Some("p"));
    }

    #[test]
    fn todo_scope_follows_where_the_board_found_the_todo_file() {
        let root = Path::new("/h/notez");
        let at = |source: &str, section: &str| {
            let mut t = task("t", CheckState::Unchecked, Some("2026-10-27"));
            t.source = PathBuf::from(source);
            t.section = section.to_string();
            let e = todo_entries(&[t], root).remove(0);
            (e.scope, e.project)
        };
        let some = |p: &str| Some(p.to_string());
        assert_eq!(at("/h/notez/TODO.md", "global"), ("global".into(), None));
        assert_eq!(at("/h/notez/_todos/home/TODO.md", "home"), ("global".into(), None));
        assert_eq!(at("/h/notez/personal/p/TODO.md", "p"), ("personal".into(), some("p")));
        assert_eq!(at("/r/p/notez/TODO.md", "p"), ("public".into(), some("p")));
        assert_eq!(at("/r/p/.notez/TODO.md", "p"), ("local".into(), some("p")));
    }

    #[test]
    fn agenda_json_has_today_days_overdue_and_one_group_per_day() {
        let mut todo = entry("2026-10-27T09:30", AgendaKind::Todo, "call");
        todo.path = PathBuf::from("/r/p/notez/TODO.md");
        todo.scope = "public".to_string();
        todo.project = Some("p".to_string());
        let a = agenda(
            day(TODAY),
            3,
            vec![entry("2026-10-24", AgendaKind::Due, "late"), todo],
        );
        let text = serde_json::to_string(&agenda_json(day(TODAY), 3, &a)).unwrap();
        let value: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(
            value,
            json!({
                "today": "2026-10-26",
                "days": 3,
                "overdue": [{
                    "date": "2026-10-24", "time": null, "kind": "due", "title": "late",
                    "path": "/h/notez/late.md", "scope": "global", "project": null,
                }],
                "days_list": [{
                    "date": "2026-10-27",
                    "entries": [{
                        "date": "2026-10-27", "time": "09:30", "kind": "todo", "title": "call",
                        "path": "/r/p/notez/TODO.md", "scope": "public", "project": "p",
                    }],
                }],
            })
        );
    }

    #[test]
    fn an_empty_agenda_is_still_a_full_json_document() {
        let value = agenda_json(day(TODAY), 7, &Agenda::default());
        assert_eq!(
            value,
            json!({ "today": "2026-10-26", "days": 7, "overdue": [], "days_list": [] })
        );
    }

    #[test]
    fn note_title_is_the_first_heading_or_the_file_stem() {
        let path = Path::new("/n/2026-10-15-book-venue.md");
        assert_eq!(
            note_title("---\ndue: 2026-10-20\n---\n# Book the venue\n\nbody\n", path),
            "Book the venue"
        );
        assert_eq!(note_title("intro\n## sub\n# Main\n", path), "Main");
        assert_eq!(note_title("no heading\n", path), "2026-10-15-book-venue");
        assert_eq!(note_title("#tag only\n", path), "2026-10-15-book-venue");
    }

    #[test]
    fn when_text_shows_the_time_with_a_space() {
        assert_eq!(when_text(d("2026-10-20")), "2026-10-20");
        assert_eq!(when_text(d("2026-10-20T14:00")), "2026-10-20 14:00");
    }
}
