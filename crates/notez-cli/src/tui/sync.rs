//! The in-session vault sync behind `g` in the tree browser and the todo
//! board: the same [`auto_sync`] the session runs on exit (commit pending
//! work, fetch, `pull --rebase`, push), on a background thread so the UI
//! never waits on the network.
//!
//! [`VaultSync`] owns at most one running sync. The view's `run` function
//! owns it and, after leaving the TUI, calls [`VaultSync::finish`], so a
//! view that quits mid-sync never lets the exit sync's git race this one on
//! `index.lock`, and a sync hung on the network waits on a normal terminal
//! (Ctrl-C works) rather than in raw mode. Dropping it finishes it too, as
//! a backstop for paths that unwind; those run after the TUI has left (the
//! owner leaves first, and the panic hook leaves before unwinding).

use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime};

use notez_core::sync::{auto_sync, AutoSync};

use super::header::SyncState;

/// How long the event loop waits for input while a sync runs, so its
/// result shows promptly.
const RUNNING_POLL: Duration = Duration::from_millis(100);

/// Why `g` does nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// The opening pull stopped: a sync would push over a pull that needs
    /// a human, so the session stays local-only.
    PullStopped,
    /// Sync is off for this run (`--no-sync`).
    Off,
    /// The vault is not a git repository.
    NotRepo,
    /// The vault has no upstream branch to sync with.
    NoUpstream,
    /// A sync is already running.
    Running,
}

impl Refusal {
    /// The one-line footer reason.
    pub fn message(self) -> &'static str {
        match self {
            Self::PullStopped => "not synced: the opening pull stopped, resolve it and run `notez sync`",
            Self::Off => "not synced: sync is off this run (--no-sync)",
            Self::NotRepo => "not synced: the vault is not a git repository",
            Self::NoUpstream => "not synced: the vault has no upstream",
            Self::Running => "sync already running",
        }
    }
}

/// Whether `g` may start a sync, for a session that opened in state
/// `opened` and now shows `current`. Only a stopped *opening* pull refuses
/// for good; a sync that stopped in the session aborted its rebase, so
/// trying again is safe.
pub fn refusal(opened: SyncState, current: SyncState) -> Option<Refusal> {
    if current == SyncState::Syncing {
        return Some(Refusal::Running);
    }
    if opened == SyncState::Stopped {
        return Some(Refusal::PullStopped);
    }
    match current {
        SyncState::Off => Some(Refusal::Off),
        SyncState::NotRepo => Some(Refusal::NotRepo),
        SyncState::NoUpstream => Some(Refusal::NoUpstream),
        _ => None,
    }
}

/// What a finished sync did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finished {
    pub result: AutoSync,
    /// The header state it leaves: Synced, Offline, No upstream or Stopped.
    pub state: SyncState,
    /// `HEAD` moved: the sync committed or pulled, so files may differ from
    /// what the view loaded.
    pub moved: bool,
}

impl Finished {
    /// The footer message: the outcome, or git's first line when it stopped.
    pub fn message(&self) -> String {
        if let AutoSync::Stopped(why) = &self.result {
            let first = why.lines().next().unwrap_or("vault sync stopped");
            return format!("sync stopped: {first}");
        }
        match (self.state, &self.result) {
            (SyncState::Synced, AutoSync::Done) => "vault synced".to_string(),
            (SyncState::Synced, _) => "vault synced, nothing to send".to_string(),
            (SyncState::Offline, _) => "not synced: the remote is unreachable".to_string(),
            (SyncState::NoUpstream, _) => "not synced: the vault has no upstream".to_string(),
            _ => "not synced".to_string(),
        }
    }
}

/// Sync the vault at `root` now, on the calling thread: [`auto_sync`], then
/// the header state from its result as after the opening pull.
pub fn run(root: &Path) -> Finished {
    let started = SystemTime::now();
    let before = head(root);
    let result = auto_sync(root);
    let state = SyncState::after_open(root, Some(&result), started);
    let moved = head(root) != before;
    Finished {
        result,
        state,
        moved,
    }
}

fn head(root: &Path) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// The session's sync: the header state and at most one running sync.
pub struct VaultSync {
    root: PathBuf,
    opened: SyncState,
    state: SyncState,
    job: Option<JoinHandle<Finished>>,
}

impl VaultSync {
    /// For the vault at `root`, in the state the session opened with.
    pub fn new(root: PathBuf, opened: SyncState) -> Self {
        Self {
            root,
            opened,
            state: opened,
            job: None,
        }
    }

    /// The header state: [`SyncState::Syncing`] while a sync runs.
    pub fn state(&self) -> SyncState {
        self.state
    }

    pub fn running(&self) -> bool {
        self.job.is_some()
    }

    /// Why `g` would do nothing now, if it would.
    pub fn refusal(&self) -> Option<Refusal> {
        refusal(self.opened, self.state)
    }

    /// Start a sync of the vault on a background thread, unless refused.
    pub fn start(&mut self) -> Result<(), Refusal> {
        let root = self.root.clone();
        self.start_with(move || run(&root))
    }

    /// [`Self::start`] with `job` as the sync.
    fn start_with(&mut self, job: impl FnOnce() -> Finished + Send + 'static) -> Result<(), Refusal> {
        if let Some(why) = self.refusal() {
            return Err(why);
        }
        self.job = Some(std::thread::spawn(job));
        self.state = SyncState::Syncing;
        Ok(())
    }

    /// The running sync's result once it has ended; never blocks.
    pub fn take_finished(&mut self) -> Option<Finished> {
        if !self.job.as_ref()?.is_finished() {
            return None;
        }
        self.wait()
    }

    /// Wait for the running sync, if any, and return its result.
    pub fn wait(&mut self) -> Option<Finished> {
        let done = self.job.take()?.join().unwrap_or_else(|_| Finished {
            result: AutoSync::Stopped("the sync thread panicked".to_string()),
            state: SyncState::Stopped,
            moved: true,
        });
        self.state = done.state;
        Some(done)
    }

    /// [`Self::wait`] for a view that has left the TUI: a sync still
    /// running is announced on stderr first, so a slow network does not
    /// look like a hang. Never call it while raw mode is on.
    pub fn finish(&mut self) -> Option<Finished> {
        if self.job.as_ref().is_some_and(|job| !job.is_finished()) {
            eprintln!("notez: waiting for the vault sync");
        }
        self.wait()
    }

    /// How long the event loop should wait for input: `idle`, or a short
    /// wait while a sync runs.
    pub fn poll_interval(&self, idle: Duration) -> Duration {
        if self.running() {
            RUNNING_POLL.min(idle)
        } else {
            idle
        }
    }
}

impl Drop for VaultSync {
    fn drop(&mut self) {
        self.finish();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn g_refuses_with_a_reason_and_otherwise_may_run() {
        use SyncState::*;
        let cases = [
            (Stopped, Stopped, Some(Refusal::PullStopped)),
            (Off, Off, Some(Refusal::Off)),
            (NotRepo, NotRepo, Some(Refusal::NotRepo)),
            (NoUpstream, NoUpstream, Some(Refusal::NoUpstream)),
            (Synced, Syncing, Some(Refusal::Running)),
            (Synced, Synced, None),
            // Offline at open: the remote may be back now.
            (Offline, Offline, None),
            // A sync that stopped in the session aborted its rebase: retry.
            (Synced, Stopped, None),
        ];
        for (opened, current, want) in cases {
            assert_eq!(refusal(opened, current), want, "{opened:?} -> {current:?}");
        }
        assert_eq!(Refusal::Running.message(), "sync already running");
        for why in [Refusal::PullStopped, Refusal::Off, Refusal::NotRepo, Refusal::NoUpstream] {
            assert!(!why.message().contains('\n'), "{why:?}");
        }
    }

    #[test]
    fn a_refused_start_runs_nothing_and_keeps_the_state() {
        for opened in [SyncState::Stopped, SyncState::Off, SyncState::NotRepo, SyncState::NoUpstream] {
            let mut sync = VaultSync::new(PathBuf::from("/nowhere"), opened);
            let refused = sync.start_with(|| panic!("a refused sync must not run"));
            assert!(refused.is_err(), "{opened:?}");
            assert!(!sync.running());
            assert_eq!(sync.state(), opened);
        }
    }

    fn finished(result: AutoSync, state: SyncState) -> Finished {
        Finished {
            result,
            state,
            moved: false,
        }
    }

    #[test]
    fn the_header_goes_idle_then_syncing_then_the_result() {
        let results = [
            finished(AutoSync::Done, SyncState::Synced),
            finished(AutoSync::Idle, SyncState::Offline),
            finished(AutoSync::Idle, SyncState::NoUpstream),
            finished(AutoSync::Stopped("CONFLICT (content)\nmore".into()), SyncState::Stopped),
        ];
        for want in results {
            let mut sync = VaultSync::new(PathBuf::from("/vault"), SyncState::Synced);
            assert_eq!(sync.state(), SyncState::Synced, "idle shows the opening state");
            assert_eq!(sync.poll_interval(Duration::from_secs(2)), Duration::from_secs(2));

            let (go, gate) = mpsc::channel::<()>();
            let result = want.clone();
            sync.start_with(move || {
                gate.recv().unwrap();
                result
            })
            .unwrap();
            assert_eq!(sync.state(), SyncState::Syncing);
            assert!(sync.running());
            assert_eq!(sync.poll_interval(Duration::from_secs(2)), RUNNING_POLL);
            assert_eq!(sync.take_finished(), None, "still running");
            assert_eq!(sync.refusal(), Some(Refusal::Running));
            assert_eq!(sync.start_with(|| unreachable!()), Err(Refusal::Running));

            go.send(()).unwrap();
            let done = sync.wait().unwrap();
            assert_eq!(done, want);
            assert_eq!(sync.state(), want.state);
            assert!(!sync.running());
            assert_eq!(sync.take_finished(), None, "a result is reported once");
        }
    }

    #[test]
    fn each_result_has_its_footer_message() {
        let cases = [
            (finished(AutoSync::Done, SyncState::Synced), "vault synced"),
            (finished(AutoSync::Idle, SyncState::Synced), "vault synced, nothing to send"),
            (finished(AutoSync::Idle, SyncState::Offline), "not synced: the remote is unreachable"),
            (finished(AutoSync::Idle, SyncState::NoUpstream), "not synced: the vault has no upstream"),
            (
                finished(AutoSync::Stopped("CONFLICT (content)\nhint".into()), SyncState::Stopped),
                "sync stopped: CONFLICT (content)",
            ),
        ];
        for (done, want) in cases {
            assert_eq!(done.message(), want);
        }
    }

    #[test]
    fn dropping_waits_for_the_running_sync() {
        let (tx, rx) = mpsc::channel();
        let mut sync = VaultSync::new(PathBuf::from("/vault"), SyncState::Synced);
        sync.start_with(move || {
            std::thread::sleep(Duration::from_millis(200));
            tx.send(()).unwrap();
            finished(AutoSync::Idle, SyncState::Synced)
        })
        .unwrap();
        drop(sync);
        assert!(rx.try_recv().is_ok(), "the sync ended before the drop returned");
    }

    fn git(dir: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    fn clone_of(remote: &Path, dir: &Path) {
        let out = Command::new("git")
            .args(["clone", "-q"])
            .arg(remote)
            .arg(dir)
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        for args in [
            &["config", "user.email", "test@example.com"][..],
            &["config", "user.name", "test"],
            &["config", "commit.gpgsign", "false"],
            &["config", "pull.rebase", "true"],
        ] {
            git(dir, args);
        }
    }

    #[test]
    fn the_runner_commits_pushes_and_pulls_a_second_clones_change() {
        let tmp = tempfile::tempdir().unwrap();
        let remote = tmp.path().join("remote.git");
        let here = tmp.path().join("here");
        let there = tmp.path().join("there");
        git(tmp.path(), &["init", "-q", "--bare", "-b", "main", remote.to_str().unwrap()]);
        clone_of(&remote, &here);
        std::fs::write(here.join("seed.md"), "seed\n").unwrap();
        git(&here, &["add", "-A"]);
        git(&here, &["commit", "-q", "-m", "seed"]);
        git(&here, &["push", "-q", "-u", "origin", "HEAD:main"]);
        git(&here, &["branch", "-q", "--set-upstream-to=origin/main"]);
        clone_of(&remote, &there);

        // The second clone pushes a note; this one has an uncommitted one.
        std::fs::write(there.join("from-there.md"), "there\n").unwrap();
        git(&there, &["add", "-A"]);
        git(&there, &["commit", "-q", "-m", "there"]);
        git(&there, &["push", "-q"]);
        std::fs::write(here.join("from-here.md"), "here\n").unwrap();

        let done = run(&here);

        assert_eq!(done.result, AutoSync::Done);
        assert_eq!(done.state, SyncState::Synced);
        assert!(done.moved);
        assert_eq!(done.message(), "vault synced");
        assert!(git(&here, &["status", "--porcelain"]).is_empty(), "committed");
        assert_eq!(
            std::fs::read_to_string(here.join("from-there.md")).unwrap(),
            "there\n",
            "pulled the second clone's note"
        );
        assert_eq!(
            git(&remote, &["rev-parse", "main"]),
            git(&here, &["rev-parse", "HEAD"]),
            "pushed"
        );
        git(&there, &["pull", "-q"]);
        assert!(there.join("from-here.md").exists(), "the second clone sees it");

        // Nothing new on either side: HEAD stays.
        let again = run(&here);
        assert_eq!(again.result, AutoSync::Idle);
        assert_eq!(again.state, SyncState::Synced);
        assert!(!again.moved);
    }

    #[test]
    fn the_runner_on_the_background_thread_reports_through_take_finished() {
        let dir = tempfile::tempdir().unwrap();
        git(dir.path(), &["init", "-q"]);
        let mut sync = VaultSync::new(dir.path().to_path_buf(), SyncState::Synced);
        sync.start().unwrap();
        let done = loop {
            if let Some(done) = sync.take_finished() {
                break done;
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        assert_eq!(done.result, AutoSync::Idle);
        assert_eq!(done.state, SyncState::NoUpstream);
        assert_eq!(sync.state(), SyncState::NoUpstream);
        assert_eq!(sync.refusal(), Some(Refusal::NoUpstream));
    }
}
