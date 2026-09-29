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
