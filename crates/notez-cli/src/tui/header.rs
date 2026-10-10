//! The one-line header above the panes in the tree browser and the todo
//! board. Left: the view's title and path. Right: the vault's sync state as
//! the session opened, its count of uncommitted files and the view's counts.
//!
//! Nothing here reaches the network. The sync state comes from the pull that
//! ran before the TUI opened ([`SyncState::after_open`]); the dirty count is
//! a local `git status`, run again only after an action that changed files
//! ([`DirtyCount`]), never on a plain keypress.

use std::path::Path;
use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use notez_core::sync::AutoSync;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

use super::theme;

/// The vault's sync state as the session opened.
///
/// NZ-6 (a quiet exit when offline) may report an unreachable remote from
/// the pull itself; that can become its own variant next to [`Self::Offline`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncState {
    /// The opening pull reached the remote: it brought changes, committed
    /// local work, or found nothing new.
    Synced,
    /// The vault has an upstream but the pull never reached it: offline, or
    /// the remote is unreachable.
    Offline,
    /// The vault is a repository with no upstream branch to sync with.
    NoUpstream,
    /// The opening pull stopped (a conflict, or a commit that failed). The
    /// footer carries the reason.
    Stopped,
    /// Sync is off for this run (`--no-sync`).
    Off,
    /// The vault is not a git repository: no sync or dirty segment.
    NotRepo,
}

/// What a local look at the vault's repository found. No network.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VaultRepo {
    pub is_repo: bool,
    pub has_upstream: bool,
    /// A fetch wrote `FETCH_HEAD` while the opening pull ran, so the pull
    /// reached the remote.
    pub fetched: bool,
}

impl SyncState {
    /// The state for the opening pull's result `pull` (`None` when sync is
    /// off) in a vault `repo`. `AutoSync::Idle` covers no upstream, nothing
    /// new and offline alike; `repo` tells them apart.
    pub fn classify(pull: Option<&AutoSync>, repo: VaultRepo) -> Self {
        if !repo.is_repo {
            return Self::NotRepo;
        }
        match pull {
            None => Self::Off,
            Some(AutoSync::Stopped(_)) => Self::Stopped,
            Some(AutoSync::Done) => Self::Synced,
            Some(AutoSync::Idle) if !repo.has_upstream => Self::NoUpstream,
            Some(AutoSync::Idle) if repo.fetched => Self::Synced,
            Some(AutoSync::Idle) => Self::Offline,
        }
    }

    /// The state after the opening pull in the vault at `root`, which
    /// started at `started`. Local git calls only, and only the ones the
    /// result needs.
    pub fn after_open(root: &Path, pull: Option<&AutoSync>, started: SystemTime) -> Self {
        let is_repo = git_succeeds(root, &["rev-parse", "--is-inside-work-tree"]);
        let idle = matches!(pull, Some(AutoSync::Idle));
        let has_upstream =
            is_repo && idle && git_succeeds(root, &["rev-parse", "--abbrev-ref", "@{u}"]);
        let fetched = has_upstream && fetched_since(root, started);
        Self::classify(
            pull,
            VaultRepo {
                is_repo,
                has_upstream,
                fetched,
            },
        )
    }

    /// The header segment, or `None` for a vault that is not a repository.
    fn segment(self) -> Option<Span<'static>> {
        let (text, color) = match self {
            Self::Synced => ("synced", theme::GREEN),
            Self::Offline => ("offline", theme::YELLOW),
            Self::NoUpstream => ("no upstream", theme::OVERLAY),
            Self::Stopped => ("pull stopped", theme::RED),
            Self::Off => ("sync off", theme::OVERLAY),
            Self::NotRepo => return None,
        };
        Some(Span::styled(text, Style::default().fg(color)))
    }
}

fn git_succeeds(root: &Path, args: &[&str]) -> bool {
    git_output(root, args).is_some()
}

fn git_output(root: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Whether a fetch that reached the remote wrote `FETCH_HEAD` in the
/// repository at `root` at or after `started`. Every fetch truncates the
/// file as it starts; only one that reached the remote fills it, even when
/// nothing is new. `started` is floored to the second, since some
/// filesystems keep whole-second times.
fn fetched_since(root: &Path, started: SystemTime) -> bool {
    let Some(path) = git_output(root, &["rev-parse", "--git-path", "FETCH_HEAD"]) else {
        return false;
    };
    let Ok(meta) = std::fs::metadata(root.join(path.trim())) else {
        return false;
    };
    let Ok(modified) = meta.modified() else {
        return false;
    };
    let floor = started
        .duration_since(UNIX_EPOCH)
        .map(|d| UNIX_EPOCH + Duration::from_secs(d.as_secs()))
        .unwrap_or(started);
    meta.len() > 0 && modified >= floor
}

/// The number of uncommitted files in the repository at `root`: changed,
/// staged and untracked, every untracked file counted on its own. `None`
/// when git cannot tell (not a repository, or git missing).
pub fn uncommitted_files(root: &Path) -> Option<usize> {
    let status = git_output(root, &["status", "--porcelain", "--untracked-files=all"])?;
    Some(status.lines().filter(|l| !l.is_empty()).count())
}

/// The vault's uncommitted file count, cached between actions. The event
/// loop marks it stale after an action that changes files; the next
/// [`Self::get`] runs `probe` once. Drawing a frame only reads the cache.
pub struct DirtyCount<F> {
    probe: F,
    count: Option<usize>,
    stale: bool,
}

impl<F: FnMut() -> Option<usize>> DirtyCount<F> {
    /// Stale from the start, so the first frame counts.
    pub fn new(probe: F) -> Self {
        Self {
            probe,
            count: None,
            stale: true,
        }
    }

    /// Count again on the next [`Self::get`]: files changed.
    pub fn mark_stale(&mut self) {
        self.stale = true;
    }

    pub fn get(&mut self) -> Option<usize> {
        if self.stale {
            self.count = (self.probe)();
            self.stale = false;
        }
        self.count
    }
}

/// The dirty count for the vault at `root` in sync state `sync`. A vault
/// that is not a repository never runs git and has no count.
pub fn vault_dirty_count(
    root: std::path::PathBuf,
    sync: SyncState,
) -> DirtyCount<impl FnMut() -> Option<usize>> {
    DirtyCount::new(move || {
        (sync != SyncState::NotRepo)
            .then(|| uncommitted_files(&root))
            .flatten()
    })
}

/// The dirty segment: the count of uncommitted files, or `clean`.
fn dirty_segment(count: usize) -> Span<'static> {
    match count {
        0 => Span::styled("clean", Style::default().fg(theme::OVERLAY)),
        n => Span::styled(
            format!("{n} uncommitted"),
            Style::default().fg(theme::PEACH),
        ),
    }
}

/// The tree's count segment.
pub fn note_count(count: usize) -> Vec<Span<'static>> {
    let noun = if count == 1 { "note" } else { "notes" };
    vec![Span::styled(
        format!("{count} {noun}"),
        Style::default().fg(theme::SAPPHIRE),
    )]
}

/// The todo board's count segment.
pub fn todo_counts(open: usize, done: usize) -> Vec<Span<'static>> {
    vec![
        Span::styled(format!("{open} open"), Style::default().fg(theme::SAPPHIRE)),
        separator(),
        Span::styled(format!("{done} done"), Style::default().fg(theme::GREEN)),
    ]
}

fn separator() -> Span<'static> {
    Span::styled(" · ", Style::default().fg(theme::SURFACE))
}

/// Everything the header shows.
pub struct Header<'a> {
    pub title: &'a str,
    pub path: &'a str,
    pub sync: SyncState,
    /// Uncommitted vault files; `None` hides the segment.
    pub dirty: Option<usize>,
    /// The view's counts ([`note_count`] or [`todo_counts`]).
    pub counts: Vec<Span<'static>>,
}

/// The header for a line `width` columns wide. It never wraps: when it does
/// not fit, parts drop in a fixed order, first the path, then the counts,
/// the dirty segment and the sync state, and last the title is cut with `…`.
pub fn line(header: &Header, width: usize) -> Line<'static> {
    let title = Span::styled(
        header.title.to_string(),
        Style::default()
            .fg(theme::LAVENDER)
            .add_modifier(Modifier::BOLD),
    );
    let path = [
        Span::styled(" - ", Style::default().fg(theme::SURFACE)),
        Span::styled(header.path.to_string(), Style::default().fg(theme::OVERLAY)),
    ];
    // The right-hand group in display order, which is also keep order: the
    // last one drops first.
    let mut right: Vec<Vec<Span<'static>>> = Vec::new();
    if let Some(sync) = header.sync.segment() {
        right.push(vec![sync]);
        if let Some(count) = header.dirty {
            right.push(vec![dirty_segment(count)]);
        }
    }
    if !header.counts.is_empty() {
        right.push(header.counts.clone());
    }

    let mut with_path = true;
    loop {
        let mut left = vec![Span::raw(" "), title.clone()];
        if with_path {
            left.extend(path.iter().cloned());
        }
        let right_spans = join(&right);
        // A trailing space, and at least one between the two sides when
        // there is a right side.
        let gap = usize::from(!right_spans.is_empty());
        let used = cols(&left) + gap + cols(&right_spans) + 1;
        if used <= width {
            let pad = width - used + gap;
            left.push(Span::raw(" ".repeat(pad)));
            left.extend(right_spans);
            left.push(Span::raw(" "));
            return Line::from(left);
        }
        if with_path {
            with_path = false;
        } else if right.pop().is_none() {
            break;
        }
    }
    // Not even the title fits: cut it, leaving the leading space.
    let room = width.saturating_sub(1);
    let mut spans = vec![Span::raw(" ".repeat(width.min(1)))];
    if room > 0 {
        spans.push(Span::styled(clip_right(header.title, room), title.style));
    }
    Line::from(spans)
}

/// The segments with a separator between each two.
fn join(segments: &[Vec<Span<'static>>]) -> Vec<Span<'static>> {
    let mut out = Vec::new();
    for (i, segment) in segments.iter().enumerate() {
        if i > 0 {
            out.push(separator());
        }
        out.extend(segment.iter().cloned());
    }
    out
}

fn cols(spans: &[Span]) -> usize {
    spans.iter().map(Span::width).sum()
}

/// `text` cut from the right to at most `max` columns, ending in `…` when cut.
fn clip_right(text: &str, max: usize) -> String {
    if Span::raw(text).width() <= max {
        return text.to_string();
    }
    let mut out = String::new();
    let mut used = 1;
    for ch in text.chars() {
        let w = Span::raw(ch.to_string()).width();
        if used + w > max {
            break;
        }
        used += w;
        out.push(ch);
    }
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn repo(is_repo: bool, has_upstream: bool, fetched: bool) -> VaultRepo {
        VaultRepo {
            is_repo,
            has_upstream,
            fetched,
        }
    }

    fn text(line: &Line) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    fn header(sync: SyncState, dirty: Option<usize>) -> Header<'static> {
        Header {
            title: "notez (proj)",
            path: "~/notez",
            sync,
            dirty,
            counts: note_count(42),
        }
    }

    #[test]
    fn the_pull_result_and_the_repository_pick_the_state() {
        let tracked = repo(true, true, true);
        let stopped = AutoSync::Stopped("conflict".into());
        assert_eq!(
            SyncState::classify(Some(&AutoSync::Done), tracked),
            SyncState::Synced
        );
        assert_eq!(
            SyncState::classify(Some(&stopped), tracked),
            SyncState::Stopped
        );
        assert_eq!(SyncState::classify(None, tracked), SyncState::Off);
        // Idle: nothing new, no upstream or offline, told apart locally.
        assert_eq!(
            SyncState::classify(Some(&AutoSync::Idle), tracked),
            SyncState::Synced
        );
        assert_eq!(
            SyncState::classify(Some(&AutoSync::Idle), repo(true, false, false)),
            SyncState::NoUpstream
        );
        assert_eq!(
            SyncState::classify(Some(&AutoSync::Idle), repo(true, true, false)),
            SyncState::Offline
        );
    }

    #[test]
    fn a_vault_that_is_not_a_repository_is_not_repo_whatever_the_pull_said() {
        let none = repo(false, false, false);
        for pull in [None, Some(AutoSync::Idle), Some(AutoSync::Done)] {
            assert_eq!(SyncState::classify(pull.as_ref(), none), SyncState::NotRepo);
        }
    }

    #[test]
    fn each_state_shows_its_own_segment() {
        let cases = [
            (SyncState::Synced, "synced", theme::GREEN),
            (SyncState::Offline, "offline", theme::YELLOW),
            (SyncState::NoUpstream, "no upstream", theme::OVERLAY),
            (SyncState::Stopped, "pull stopped", theme::RED),
            (SyncState::Off, "sync off", theme::OVERLAY),
        ];
        for (state, label, color) in cases {
            let line = line(&header(state, Some(3)), 100);
            let shown = text(&line);
            assert!(
                shown.ends_with(&format!("{label} · 3 uncommitted · 42 notes ")),
                "{state:?}: {shown:?}"
            );
            let span = line.spans.iter().find(|s| s.content == label).unwrap();
            assert_eq!(span.style.fg, Some(color), "{state:?}");
        }
    }

    #[test]
    fn a_vault_that_is_not_a_repository_shows_no_sync_or_dirty_segment() {
        let shown = text(&line(&header(SyncState::NotRepo, Some(3)), 100));
        assert!(shown.starts_with(" notez (proj) - ~/notez "), "{shown:?}");
        assert!(shown.ends_with(" 42 notes "), "{shown:?}");
        assert!(
            !shown.contains("uncommitted") && !shown.contains("sync"),
            "{shown:?}"
        );
    }

    #[test]
    fn a_clean_vault_says_clean_and_an_unknown_count_hides_the_segment() {
        let clean = text(&line(&header(SyncState::Synced, Some(0)), 100));
        assert!(clean.ends_with("synced · clean · 42 notes "), "{clean:?}");
        let unknown = text(&line(&header(SyncState::Synced, None), 100));
        assert!(unknown.ends_with("synced · 42 notes "), "{unknown:?}");
    }

    #[test]
    fn the_todo_board_shows_open_and_done() {
        let board = Header {
            title: "todoz (global)",
            path: "~/notez",
            sync: SyncState::Synced,
            dirty: Some(0),
            counts: todo_counts(5, 3),
        };
        let shown = text(&line(&board, 100));
        assert!(shown.starts_with(" todoz (global) - ~/notez "), "{shown:?}");
        assert!(
            shown.ends_with("synced · clean · 5 open · 3 done "),
            "{shown:?}"
        );
        assert_eq!(note_count(1)[0].content, "1 note");
    }

    #[test]
    fn the_header_fills_the_width_exactly() {
        for width in [100, 60] {
            let line = line(&header(SyncState::Synced, Some(3)), width);
            assert_eq!(line.width(), width);
        }
    }

    /// The header at each width, as text with the padding squeezed out.
    fn at(width: usize) -> String {
        let line = line(&header(SyncState::Stopped, Some(12)), width);
        assert!(line.width() <= width, "width {width}: {line:?}");
        text(&line).split_whitespace().collect::<Vec<_>>().join(" ")
    }

    #[test]
    fn narrow_widths_drop_the_path_then_counts_then_dirty_then_sync() {
        // Full: lead space, title 12, path 10, gap, right group 40, trailing
        // space: 65 columns.
        assert_eq!(
            at(65),
            "notez (proj) - ~/notez pull stopped · 12 uncommitted · 42 notes"
        );
        assert_eq!(
            at(64),
            "notez (proj) pull stopped · 12 uncommitted · 42 notes"
        );
        assert_eq!(
            at(55),
            "notez (proj) pull stopped · 12 uncommitted · 42 notes"
        );
        assert_eq!(at(54), "notez (proj) pull stopped · 12 uncommitted");
        assert_eq!(at(44), "notez (proj) pull stopped · 12 uncommitted");
        assert_eq!(at(43), "notez (proj) pull stopped");
        assert_eq!(at(27), "notez (proj) pull stopped");
        assert_eq!(at(26), "notez (proj)");
        assert_eq!(at(14), "notez (proj)");
        assert_eq!(at(13), "notez (proj)");
        assert_eq!(at(12), "notez (pro…");
        assert_eq!(at(3), "n…");
        assert_eq!(at(1), "");
        assert_eq!(at(0), "");
    }

    #[test]
    fn the_dirty_count_runs_git_once_per_change_not_per_frame() {
        let calls = Cell::new(0);
        let mut dirty = DirtyCount::new(|| {
            calls.set(calls.get() + 1);
            Some(calls.get())
        });
        // The first frame counts; plain navigation only redraws.
        for _ in 0..5 {
            assert_eq!(dirty.get(), Some(1));
        }
        assert_eq!(calls.get(), 1);
        // A changing action marks it stale: one more count, then cached.
        dirty.mark_stale();
        for _ in 0..5 {
            assert_eq!(dirty.get(), Some(2));
        }
        assert_eq!(calls.get(), 2);
    }

    fn git(dir: &Path, args: &[&str]) {
        let ok = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .unwrap();
        assert!(
            ok.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&ok.stderr)
        );
    }

    #[test]
    fn a_plain_folder_is_not_repo_and_has_no_dirty_count() {
        let dir = tempfile::tempdir().unwrap();
        let state = SyncState::after_open(dir.path(), Some(&AutoSync::Idle), SystemTime::now());
        assert_eq!(state, SyncState::NotRepo);
        assert_eq!(uncommitted_files(dir.path()), None);
        let mut dirty = vault_dirty_count(dir.path().to_path_buf(), state);
        assert_eq!(dirty.get(), None);
    }

    #[test]
    fn a_repository_without_upstream_counts_every_uncommitted_file() {
        let dir = tempfile::tempdir().unwrap();
        git(dir.path(), &["init", "-q"]);
        let started = SystemTime::now();
        assert_eq!(
            SyncState::after_open(dir.path(), Some(&AutoSync::Idle), started),
            SyncState::NoUpstream
        );
        assert_eq!(
            SyncState::after_open(dir.path(), None, started),
            SyncState::Off
        );
        assert_eq!(uncommitted_files(dir.path()), Some(0));
        std::fs::create_dir_all(dir.path().join("ideas")).unwrap();
        std::fs::write(dir.path().join("ideas/a.md"), "a").unwrap();
        std::fs::write(dir.path().join("ideas/b.md"), "b").unwrap();
        std::fs::write(dir.path().join("top.md"), "t").unwrap();
        assert_eq!(uncommitted_files(dir.path()), Some(3));
    }

    #[test]
    fn a_fetch_during_the_pull_means_synced_and_none_means_offline() {
        let remote = tempfile::tempdir().unwrap();
        let vault = tempfile::tempdir().unwrap();
        git(remote.path(), &["init", "-q", "--bare"]);
        git(vault.path(), &["init", "-q"]);
        for args in [
            &["config", "user.email", "test@example.com"][..],
            &["config", "user.name", "test"],
            &["config", "commit.gpgsign", "false"],
            &["commit", "-q", "--allow-empty", "-m", "first"],
            &["remote", "add", "origin", &remote.path().to_string_lossy()],
            &["push", "-q", "-u", "origin", "HEAD"],
        ] {
            git(vault.path(), args);
        }
        // No fetch since `started`: the pull never reached the remote.
        let later = SystemTime::now() + Duration::from_secs(5);
        assert_eq!(
            SyncState::after_open(vault.path(), Some(&AutoSync::Idle), later),
            SyncState::Offline
        );
        let started = SystemTime::now();
        git(vault.path(), &["fetch", "-q"]);
        assert_eq!(
            SyncState::after_open(vault.path(), Some(&AutoSync::Idle), started),
            SyncState::Synced
        );
        // A fetch that cannot reach the remote either leaves `FETCH_HEAD`
        // empty (git 2.39 truncates it) or does not touch it. Back-date the
        // good one first, so both outcomes read as offline whatever the
        // git version and however fast the fetch fails.
        let fetch_head = std::fs::File::options()
            .write(true)
            .open(vault.path().join(".git/FETCH_HEAD"))
            .unwrap();
        fetch_head
            .set_modified(SystemTime::now() - Duration::from_secs(60))
            .unwrap();
        drop(fetch_head);
        let gone = remote.path().join("gone");
        git(
            vault.path(),
            &["remote", "set-url", "origin", &gone.to_string_lossy()],
        );
        let started = SystemTime::now();
        let failed = Command::new("git")
            .arg("-C")
            .arg(vault.path())
            .args(["fetch", "-q"])
            .output();
        assert!(!failed.unwrap().status.success());
        assert_eq!(
            SyncState::after_open(vault.path(), Some(&AutoSync::Idle), started),
            SyncState::Offline
        );
    }
}
