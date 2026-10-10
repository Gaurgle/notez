//! Git helpers for syncing the notez root.
//!
//! Shared by `notez sync` and the desktop app so both commit the same way
//! before pulling and pushing.

use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};
use chrono::Local;

/// Stage and commit every pending change under `root`, including untracked
/// notes. Returns `true` if a commit was made, `false` if the tree was clean.
///
/// This has to run before `git pull --rebase`, which refuses to start on a
/// dirty working tree.
pub fn commit_pending(root: &Path) -> Result<bool> {
    let status = git(root, &["status", "--porcelain"])?;
    if status.trim().is_empty() {
        return Ok(false);
    }
    git(root, &["add", "-A"])?;
    let message = format!("notes: sync {}", Local::now().format("%Y-%m-%d %H:%M"));
    git(root, &["commit", "-m", &message])?;
    Ok(true)
}

/// What an automatic sync did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutoSync {
    /// Not a repo, no upstream, or nothing to send. Not an error: offline and
    /// unconfigured vaults carry on silently.
    Idle,
    /// Changes were committed and/or pushed.
    Done,
    /// A step failed. The rebase, if one started, was aborted, so the vault is
    /// exactly as it was before the pull and `notez sync` shows git's own words.
    Stopped(String),
}

/// Commit, pull and push `root` without a terminal, for use when a session
/// ends. Follows "stop rather than guess": a failed pull aborts its rebase and
/// leaves the work as local commits instead of resolving anything.
///
/// A fetch that fails is `Idle`, after the commit, so ending a session offline
/// is quiet and the work waits as local commits for the next sync that reaches
/// the remote. The fetch runs on its own so git's exit status, not its error
/// text, tells "could not reach the remote" (offline, missing remote, expired
/// credential) apart from a conflict or a rejected push, which still stop.
/// `notez sync` does not use this and still reports an unreachable remote.
pub fn auto_sync(root: &Path) -> AutoSync {
    if git(root, &["rev-parse", "--is-inside-work-tree"]).is_err()
        || git(root, &["rev-parse", "--abbrev-ref", "@{u}"]).is_err()
    {
        return AutoSync::Idle;
    }
    let committed = match commit_pending(root) {
        Ok(c) => c,
        Err(e) => return AutoSync::Stopped(format!("{e:#}")),
    };
    if git(root, &["fetch", "--quiet"]).is_err() {
        return AutoSync::Idle;
    }
    if let Err(e) = git(root, &["pull", "--rebase"]) {
        let _ = git(root, &["rebase", "--abort"]);
        return AutoSync::Stopped(format!("{e:#}"));
    }
    let ahead = git(root, &["rev-list", "--count", "@{u}..HEAD"])
        .ok()
        .and_then(|n| n.trim().parse::<usize>().ok())
        .unwrap_or(0);
    if ahead == 0 {
        return if committed { AutoSync::Done } else { AutoSync::Idle };
    }
    match git(root, &["push"]) {
        Ok(_) => AutoSync::Done,
        Err(e) => AutoSync::Stopped(format!("{e:#}")),
    }
}

/// Commit pending work in `root`, then `git pull --rebase`, for use before an
/// interactive session opens so it shows the merged vault. Never pushes: that
/// is left to [`auto_sync`] when the session ends.
///
/// Not a repo, no upstream, or an unreachable remote is `Idle`, and the
/// session opens on the local files. A pull that stops on a conflict aborts
/// its rebase, so the vault is exactly as the commit left it, and returns
/// `Stopped`; if the abort itself fails, the message says so and names the
/// manual fix instead. Callers should then skip the exit sync so its push does not go
/// over a pull that needs a human.
pub fn pull_on_open(root: &Path) -> AutoSync {
    if git(root, &["rev-parse", "--is-inside-work-tree"]).is_err()
        || git(root, &["rev-parse", "--abbrev-ref", "@{u}"]).is_err()
    {
        return AutoSync::Idle;
    }
    let committed = match commit_pending(root) {
        Ok(c) => c,
        Err(e) => return AutoSync::Stopped(format!("{e:#}")),
    };
    let before = git(root, &["rev-parse", "HEAD"]).ok();
    if git(root, &["pull", "--rebase"]).is_err() {
        // Without a rebase in progress the pull never got past the fetch:
        // offline or the remote is unreachable, which is not worth a warning.
        if !rebase_in_progress(root) {
            return AutoSync::Idle;
        }
        if git(root, &["rebase", "--abort"]).is_err() {
            return AutoSync::Stopped(format!(
                "vault pull hit a conflict and the rebase could not be aborted; \
                 the vault needs manual attention: run `git rebase --abort` in {}",
                crate::util::tilde::contract(root)
            ));
        }
        return AutoSync::Stopped("vault pull hit a conflict, rebase aborted".into());
    }
    let after = git(root, &["rev-parse", "HEAD"]).ok();
    if committed || before != after {
        AutoSync::Done
    } else {
        AutoSync::Idle
    }
}

/// Whether a rebase stopped part way in `root`, in either backend's state dir.
fn rebase_in_progress(root: &Path) -> bool {
    ["rebase-merge", "rebase-apply"].iter().any(|dir| {
        git(root, &["rev-parse", "--git-path", dir])
            .map(|p| root.join(p.trim()).exists())
            .unwrap_or(false)
    })
}

fn git(root: &Path, args: &[&str]) -> Result<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .with_context(|| format!("failed to invoke git {}", args.join(" ")))?;
    if !out.status.success() {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn repo() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        for args in [
            &["init", "-q"][..],
            &["config", "user.email", "test@example.com"],
            &["config", "user.name", "test"],
            &["config", "commit.gpgsign", "false"],
        ] {
            git(tmp.path(), args).unwrap();
        }
        tmp
    }

    fn commit_count(root: &Path) -> usize {
        git(root, &["rev-list", "--count", "HEAD"])
            .map(|s| s.trim().parse().unwrap())
            .unwrap_or(0)
    }

    #[test]
    fn should_not_commit_when_tree_is_clean() {
        let tmp = repo();
        assert!(!commit_pending(tmp.path()).unwrap());
        assert_eq!(commit_count(tmp.path()), 0);
    }

    #[test]
    fn should_commit_untracked_note() {
        let tmp = repo();
        fs::create_dir_all(tmp.path().join("personal/p/01_daily-logs")).unwrap();
        fs::write(tmp.path().join("personal/p/01_daily-logs/log.md"), "hi").unwrap();

        assert!(commit_pending(tmp.path()).unwrap());
        assert_eq!(commit_count(tmp.path()), 1);
        assert!(git(tmp.path(), &["status", "--porcelain"]).unwrap().is_empty());
        let subject = git(tmp.path(), &["log", "-1", "--format=%s"]).unwrap();
        assert!(subject.starts_with("notes: sync "));
    }

    #[test]
    fn should_commit_modified_tracked_note() {
        let tmp = repo();
        fs::write(tmp.path().join("a.md"), "one").unwrap();
        commit_pending(tmp.path()).unwrap();
        fs::write(tmp.path().join("a.md"), "two").unwrap();

        assert!(commit_pending(tmp.path()).unwrap());
        assert_eq!(commit_count(tmp.path()), 2);
    }

    #[test]
    fn should_fail_outside_a_git_repo() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(commit_pending(tmp.path()).is_err());
    }

    fn git_in(dir: &Path, args: &[&str]) -> String {
        git(dir, args).unwrap()
    }

    /// A bare remote plus two clones sharing it.
    fn remote_with_clones() -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let remote = tmp.path().join("remote.git");
        fs::create_dir(&remote).unwrap();
        git_in(&remote, &["init", "-q", "--bare", "-b", "main"]);
        let seed = tmp.path().join("seed");
        fs::create_dir(&seed).unwrap();
        for args in [
            &["init", "-q", "-b", "main"][..],
            &["config", "user.email", "t@example.com"],
            &["config", "user.name", "t"],
            &["config", "commit.gpgsign", "false"],
        ] {
            git_in(&seed, args);
        }
        fs::write(seed.join("a.md"), "base\n").unwrap();
        git_in(&seed, &["add", "-A"]);
        git_in(&seed, &["commit", "-q", "-m", "seed"]);
        git_in(&seed, &["remote", "add", "origin", remote.to_str().unwrap()]);
        git_in(&seed, &["push", "-q", "-u", "origin", "main"]);

        let mut clones = Vec::new();
        for name in ["one", "two"] {
            let dir = tmp.path().join(name);
            let out = Command::new("git")
                .args(["clone", "-q", remote.to_str().unwrap(), dir.to_str().unwrap()])
                .output()
                .unwrap();
            assert!(out.status.success());
            for args in [
                &["config", "user.email", "t@example.com"][..],
                &["config", "user.name", "t"],
                &["config", "commit.gpgsign", "false"],
            ] {
                git_in(&dir, args);
            }
            clones.push(dir);
        }
        let two = clones.pop().unwrap();
        let one = clones.pop().unwrap();
        (tmp, one, two)
    }

    #[test]
    fn auto_sync_is_idle_outside_a_repo_or_without_upstream() {
        let plain = tempfile::tempdir().unwrap();
        assert_eq!(auto_sync(plain.path()), AutoSync::Idle);

        let local_only = repo();
        fs::write(local_only.path().join("a.md"), "x").unwrap();
        assert_eq!(auto_sync(local_only.path()), AutoSync::Idle);
        assert!(!git_in(local_only.path(), &["status", "--porcelain"]).is_empty());
    }

    #[test]
    fn auto_sync_commits_and_pushes_a_new_note() {
        let (_tmp, one, two) = remote_with_clones();
        fs::write(one.join("new.md"), "hello\n").unwrap();

        assert_eq!(auto_sync(&one), AutoSync::Done);

        git_in(&two, &["pull", "-q"]);
        assert!(two.join("new.md").exists());
    }

    #[test]
    fn auto_sync_rebases_onto_remote_changes() {
        let (_tmp, one, two) = remote_with_clones();
        fs::write(two.join("theirs.md"), "t\n").unwrap();
        assert_eq!(auto_sync(&two), AutoSync::Done);
        fs::write(one.join("mine.md"), "m\n").unwrap();

        assert_eq!(auto_sync(&one), AutoSync::Done);

        assert!(one.join("theirs.md").exists());
        git_in(&two, &["pull", "-q"]);
        assert!(two.join("mine.md").exists());
    }

    #[test]
    fn auto_sync_stops_on_conflict_and_leaves_no_rebase_behind() {
        let (_tmp, one, two) = remote_with_clones();
        fs::write(two.join("a.md"), "theirs\n").unwrap();
        assert_eq!(auto_sync(&two), AutoSync::Done);
        fs::write(one.join("a.md"), "mine\n").unwrap();

        let outcome = auto_sync(&one);

        assert!(matches!(outcome, AutoSync::Stopped(_)));
        assert!(!one.join(".git/rebase-merge").exists());
        assert!(!one.join(".git/rebase-apply").exists());
        assert_eq!(fs::read_to_string(one.join("a.md")).unwrap(), "mine\n");
    }

    #[test]
    fn auto_sync_is_idle_and_commits_locally_when_the_remote_is_unreachable() {
        let (tmp, one, _two) = remote_with_clones();
        fs::rename(tmp.path().join("remote.git"), tmp.path().join("gone.git")).unwrap();
        fs::write(one.join("mine.md"), "m\n").unwrap();

        assert_eq!(auto_sync(&one), AutoSync::Idle);

        assert!(git_in(&one, &["status", "--porcelain"]).is_empty());
        assert_eq!(
            git_in(&one, &["rev-list", "--count", "@{u}..HEAD"]).trim(),
            "1"
        );
        assert!(!one.join(".git/rebase-merge").exists());
        assert!(!one.join(".git/rebase-apply").exists());
    }

    #[test]
    fn auto_sync_reports_a_conflict_with_a_reachable_remote_in_gits_words() {
        let (_tmp, one, two) = remote_with_clones();
        fs::write(two.join("a.md"), "theirs\n").unwrap();
        assert_eq!(auto_sync(&two), AutoSync::Done);
        fs::write(one.join("a.md"), "mine\n").unwrap();

        let outcome = auto_sync(&one);

        let AutoSync::Stopped(why) = outcome else {
            panic!("expected Stopped, got {outcome:?}");
        };
        assert!(why.starts_with("git pull --rebase failed: "), "{why}");
    }

    #[test]
    fn auto_sync_pushes_offline_commits_once_the_remote_is_reachable() {
        let (tmp, one, two) = remote_with_clones();
        let remote = tmp.path().join("remote.git");
        let gone = tmp.path().join("gone.git");
        fs::rename(&remote, &gone).unwrap();
        fs::write(one.join("mine.md"), "m\n").unwrap();
        assert_eq!(auto_sync(&one), AutoSync::Idle);
        fs::rename(&gone, &remote).unwrap();

        assert_eq!(auto_sync(&one), AutoSync::Done);

        git_in(&two, &["pull", "-q"]);
        assert!(two.join("mine.md").exists());
    }

    /// Point `dir` at its own hooks directory, so a global core.hooksPath
    /// cannot make git skip the hook, and install an executable `name` hook.
    #[cfg(unix)]
    fn install_hook(dir: &Path, hooks_dir: &Path, name: &str, body: &str) {
        use std::os::unix::fs::PermissionsExt;
        fs::create_dir_all(hooks_dir).unwrap();
        git_in(dir, &["config", "core.hooksPath", hooks_dir.to_str().unwrap()]);
        let hook = hooks_dir.join(name);
        fs::write(&hook, body).unwrap();
        fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn auto_sync_reports_a_failed_commit_even_when_the_remote_is_unreachable() {
        let (tmp, one, _two) = remote_with_clones();
        fs::rename(tmp.path().join("remote.git"), tmp.path().join("gone.git")).unwrap();
        install_hook(&one, &one.join(".git/hooks"), "pre-commit", "#!/bin/sh\nexit 1\n");
        fs::write(one.join("mine.md"), "m\n").unwrap();

        let outcome = auto_sync(&one);

        let AutoSync::Stopped(why) = outcome else {
            panic!("expected Stopped, got {outcome:?}");
        };
        assert!(why.starts_with("git commit -m notes: sync "), "{why}");
    }

    #[cfg(unix)]
    #[test]
    fn auto_sync_reports_a_push_rejected_by_a_reachable_remote() {
        let (tmp, one, _two) = remote_with_clones();
        let remote = tmp.path().join("remote.git");
        install_hook(&remote, &remote.join("hooks"), "pre-receive", "#!/bin/sh\nexit 1\n");
        fs::write(one.join("mine.md"), "m\n").unwrap();

        let outcome = auto_sync(&one);

        let AutoSync::Stopped(why) = outcome else {
            panic!("expected Stopped, got {outcome:?}");
        };
        assert!(why.starts_with("git push failed: "), "{why}");
    }

    #[test]
    fn pull_on_open_picks_up_a_remote_change() {
        let (_tmp, one, two) = remote_with_clones();
        fs::write(two.join("theirs.md"), "t\n").unwrap();
        assert_eq!(auto_sync(&two), AutoSync::Done);

        assert_eq!(pull_on_open(&one), AutoSync::Done);

        assert!(one.join("theirs.md").exists());
    }

    #[test]
    fn pull_on_open_commits_pending_work_but_never_pushes() {
        let (_tmp, one, two) = remote_with_clones();
        fs::write(two.join("theirs.md"), "t\n").unwrap();
        assert_eq!(auto_sync(&two), AutoSync::Done);
        fs::write(one.join("mine.md"), "m\n").unwrap();

        assert_eq!(pull_on_open(&one), AutoSync::Done);

        assert!(one.join("theirs.md").exists());
        assert!(git_in(&one, &["status", "--porcelain"]).is_empty());
        assert_eq!(
            git_in(&one, &["rev-list", "--count", "@{u}..HEAD"]).trim(),
            "1"
        );
        git_in(&two, &["pull", "-q"]);
        assert!(!two.join("mine.md").exists());
    }

    #[test]
    fn pull_on_open_is_idle_when_already_up_to_date() {
        let (_tmp, one, _two) = remote_with_clones();
        assert_eq!(pull_on_open(&one), AutoSync::Idle);
    }

    #[test]
    fn pull_on_open_stops_on_conflict_and_leaves_no_rebase_behind() {
        let (_tmp, one, two) = remote_with_clones();
        fs::write(two.join("a.md"), "theirs\n").unwrap();
        assert_eq!(auto_sync(&two), AutoSync::Done);
        fs::write(one.join("a.md"), "mine\n").unwrap();

        let outcome = pull_on_open(&one);

        assert!(matches!(outcome, AutoSync::Stopped(_)));
        assert!(!one.join(".git/rebase-merge").exists());
        assert!(!one.join(".git/rebase-apply").exists());
        assert_eq!(fs::read_to_string(one.join("a.md")).unwrap(), "mine\n");
        assert!(git_in(&one, &["status", "--porcelain"]).is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn pull_on_open_says_so_when_the_rebase_cannot_be_aborted() {
        use std::os::unix::fs::PermissionsExt;

        let (_tmp, one, two) = remote_with_clones();
        fs::write(two.join("a.md"), "theirs\n").unwrap();
        assert_eq!(auto_sync(&two), AutoSync::Done);
        fs::write(one.join("a.md"), "mine\n").unwrap();
        // The rebase checks out the upstream first; holding the index lock
        // from then on stops the rebase part way and makes its abort fail.
        // A global core.hooksPath would make git skip the hook below.
        let hooks_dir = one.join(".git/hooks");
        git_in(&one, &["config", "core.hooksPath", hooks_dir.to_str().unwrap()]);
        let hook = hooks_dir.join("post-checkout");
        fs::write(&hook, "#!/bin/sh\ntouch \"$(git rev-parse --git-dir)/index.lock\"\n").unwrap();
        fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();

        let outcome = pull_on_open(&one);

        let AutoSync::Stopped(why) = outcome else {
            panic!("expected Stopped, got {outcome:?}");
        };
        assert!(why.contains("could not be aborted"), "{why}");
        assert!(why.contains("git rebase --abort"), "{why}");
        assert!(!why.contains("rebase aborted"), "{why}");
    }

    #[test]
    fn pull_on_open_is_idle_outside_a_repo_or_without_upstream() {
        let plain = tempfile::tempdir().unwrap();
        assert_eq!(pull_on_open(plain.path()), AutoSync::Idle);

        let local_only = repo();
        fs::write(local_only.path().join("a.md"), "x").unwrap();
        assert_eq!(pull_on_open(local_only.path()), AutoSync::Idle);
        assert!(!git_in(local_only.path(), &["status", "--porcelain"]).is_empty());
    }

    #[test]
    fn pull_on_open_is_idle_when_the_remote_is_unreachable() {
        let (tmp, one, _two) = remote_with_clones();
        fs::rename(tmp.path().join("remote.git"), tmp.path().join("gone.git")).unwrap();
        fs::write(one.join("mine.md"), "m\n").unwrap();

        assert_eq!(pull_on_open(&one), AutoSync::Idle);

        assert!(one.join("mine.md").exists());
        assert!(!one.join(".git/rebase-merge").exists());
        assert!(!one.join(".git/rebase-apply").exists());
    }
}
