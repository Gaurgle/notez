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
}
