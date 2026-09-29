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
}
