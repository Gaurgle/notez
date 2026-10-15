//! Sets `NOTEZ_VERSION`, the string `notez --version` prints after the name.
//!
//! In a git checkout it is `<CARGO_PKG_VERSION> (<short sha>)`, with
//! `-dirty` appended to the sha when tracked files have uncommitted changes
//! (untracked files do not count, matching `git describe --dirty`).
//!
//! Fallback: outside a git checkout (a source tarball, a vendored copy), when
//! `git` is not on `PATH`, or when `git rev-parse` fails for any other reason,
//! it is the plain `CARGO_PKG_VERSION` and the build still succeeds. No git
//! failure is ever an error here.
//!
//! Rerun triggers: the file holding HEAD, the ref HEAD points at (or
//! `packed-refs` when that ref is packed), and the workspace `crates/`
//! directory. Paths come from `git rev-parse --git-path`, so a linked
//! worktree, whose `.git` is a file pointing into the main repo, resolves
//! the same as a plain `.git` directory. Watching sources means a source edit
//! refreshes the `-dirty` mark; an edit outside `crates/` alone does not.

use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let package_version = env!("CARGO_PKG_VERSION");
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    let version = match git(&manifest_dir, &["rev-parse", "--short", "HEAD"]) {
        Some(sha) => {
            emit_rerun_triggers(&manifest_dir);
            let dirty = git(
                &manifest_dir,
                &["status", "--porcelain", "--untracked-files=no"],
            )
            .is_some_and(|status| !status.is_empty());
            let suffix = if dirty { "-dirty" } else { "" };
            format!("{package_version} ({sha}{suffix})")
        }
        None => package_version.to_string(),
    };

    println!("cargo:rustc-env=NOTEZ_VERSION={version}");
}

/// Runs `git` in `dir` and returns its trimmed stdout, or `None` when git is
/// missing, exits non-zero or prints non-UTF-8.
fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    Some(text.trim().to_string())
}

/// Resolves a path inside the git directory, absolute even when git prints it
/// relative to `dir`.
fn git_path(dir: &Path, name: &str) -> Option<PathBuf> {
    let path = PathBuf::from(git(dir, &["rev-parse", "--git-path", name])?);
    Some(if path.is_absolute() {
        path
    } else {
        dir.join(path)
    })
}

fn emit_rerun_triggers(manifest_dir: &Path) {
    if let Some(head) = git_path(manifest_dir, "HEAD") {
        println!("cargo:rerun-if-changed={}", head.display());
    }

    // Detached HEAD has no symbolic ref; the HEAD file itself then changes
    // on every checkout.
    if let Some(reference) = git(manifest_dir, &["symbolic-ref", "-q", "HEAD"]) {
        // A missing watched file makes cargo rerun on every build, so watch
        // `packed-refs` instead when the ref has no loose file.
        let loose = git_path(manifest_dir, &reference).filter(|path| path.exists());
        let watched =
            loose.or_else(|| git_path(manifest_dir, "packed-refs").filter(|p| p.exists()));
        if let Some(path) = watched {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }

    if let Some(crates_dir) = manifest_dir.parent() {
        println!("cargo:rerun-if-changed={}", crates_dir.display());
    }
}
