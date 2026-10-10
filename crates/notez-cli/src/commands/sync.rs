//! `notez sync`: commit, pull and push the global notez root via git.
//!
//! Commits any pending changes (see `notez_core::sync::commit_pending`), then
//! runs `git -C <notez_root> pull --rebase` followed by
//! `git -C <notez_root> push`. Surfaces git's own output on conflict so the
//! user can resolve manually.

use std::process::Command;

use anyhow::{Context, Result, bail};

use notez_core::config::Config;

pub fn run(config: &Config) -> Result<()> {
    let root = config.notez_root_path();
    if !root.join(".git").exists() {
        bail!(
            "notez root at {} is not a git repository. Run `git init` there and add a remote first.",
            root.display()
        );
    }

    if notez_core::sync::commit_pending(&root)? {
        println!("Committed local changes.");
    }

    println!("Pulling latest from remote...");
    let status = Command::new("git")
        .args(["-C", root.to_str().unwrap_or(""), "pull", "--rebase"])
        .status()
        .context("failed to invoke git pull")?;
    if !status.success() {
        bail!("git pull --rebase failed; resolve conflicts manually and rerun");
    }

    println!("Pushing local commits...");
    let status = Command::new("git")
        .args(["-C", root.to_str().unwrap_or(""), "push"])
        .status()
        .context("failed to invoke git push")?;
    if !status.success() {
        bail!("git push failed");
    }

    println!("Sync complete.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn git(dir: &Path, args: &[&str]) {
        let out = Command::new("git").arg("-C").arg(dir).args(args).output().unwrap();
        assert!(out.status.success(), "git {args:?}: {out:?}");
    }

    #[test]
    fn explicit_sync_reports_an_unreachable_remote() {
        let tmp = tempfile::tempdir().unwrap();
        let remote = tmp.path().join("remote.git");
        let vault = tmp.path().join("vault");
        std::fs::create_dir(&remote).unwrap();
        std::fs::create_dir(&vault).unwrap();
        git(&remote, &["init", "-q", "--bare", "-b", "main"]);
        for args in [
            &["init", "-q", "-b", "main"][..],
            &["config", "user.email", "t@example.com"],
            &["config", "user.name", "t"],
            &["config", "commit.gpgsign", "false"],
        ] {
            git(&vault, args);
        }
        std::fs::write(vault.join("a.md"), "base\n").unwrap();
        git(&vault, &["add", "-A"]);
        git(&vault, &["commit", "-q", "-m", "seed"]);
        git(&vault, &["remote", "add", "origin", remote.to_str().unwrap()]);
        git(&vault, &["push", "-q", "-u", "origin", "main"]);
        std::fs::rename(&remote, tmp.path().join("gone.git")).unwrap();
        std::fs::write(vault.join("new.md"), "pending\n").unwrap();
        let mut config = Config::defaults();
        config.paths.notez_root = vault.to_str().unwrap().to_string();

        let err = run(&config).unwrap_err();

        assert!(err.to_string().contains("git pull --rebase failed"), "{err:#}");
    }
}
