//! Scope -> filesystem root resolution.
//!
//! Separated from [`super::scope`] so [`Scope`] can stay a pure data type
//! while the resolution logic gets to depend on [`Config`] and
//! [`Project`].

use std::path::PathBuf;

use anyhow::Result;

use crate::config::Config;
use crate::core::{Project, Scope};

/// Root of the project the cwd belongs to.
///
/// The git toplevel when inside a repo, otherwise the cwd itself. Scratch
/// and public stores hang off this rather than off the cwd, so a note taken
/// three directories deep lands in the project's one store instead of
/// spawning a new store beside it.
fn project_root() -> Result<PathBuf> {
    match Project::try_detect() {
        Some(p) => Ok(p.root),
        None => Ok(std::env::current_dir()?),
    }
}

/// Resolve the root directory for the given scope.
///
/// - `Local`: `<project root>/.notez/` (git toplevel, else cwd)
/// - `Public`: `<project root>/notez/` (git toplevel, else cwd)
/// - `Personal`: `<notez_root>/personal/<project>/` if cwd is inside a git
///   repo; otherwise falls back to `<notez_root>/` (same as Global).
/// - `Global`: `<notez_root>/`
pub fn root(scope: Scope, config: &Config) -> Result<PathBuf> {
    let root = match scope {
        Scope::Local => project_root()?.join(".notez"),
        Scope::Public => project_root()?.join("notez"),
        Scope::Personal => match Project::try_detect() {
            Some(p) => config.notez_root_path().join("personal").join(&p.name),
            None => config.notez_root_path(),
        },
        Scope::Global => config.notez_root_path(),
    };
    Ok(root)
}

/// Resolve the quick-notes directory for the given scope.
pub fn quick_notes(scope: Scope, config: &Config) -> Result<PathBuf> {
    Ok(root(scope, config)?.join(&config.paths.quick_notes_dir))
}

/// Resolve the daily-logs directory for the given scope.
pub fn daily_logs(scope: Scope, config: &Config) -> Result<PathBuf> {
    Ok(root(scope, config)?.join(&config.paths.daily_logs_dir))
}

/// The todo board's store under the global notez root: one folder per
/// category, `<notez_root>/_todos/<category>/TODO.md`.
pub const TODOS_DIR: &str = "_todos";

/// The file name of every todo list: a scope root's own list and each
/// `_todos` category's.
pub const TODO_FILE: &str = "TODO.md";

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn config_with_root(root: &std::path::Path) -> Config {
        let mut c = Config::defaults();
        c.paths.notez_root = root.to_string_lossy().into_owned();
        c
    }

    #[test]
    fn global_uses_notez_root() {
        let dir = tempdir().unwrap();
        let config = config_with_root(dir.path());
        let r = root(Scope::Global, &config).unwrap();
        assert_eq!(r, dir.path());
    }

    #[test]
    #[serial_test::serial]
    fn personal_falls_back_to_global_outside_git() {
        let dir = tempdir().unwrap();
        let config = config_with_root(dir.path());

        // Move into a non-git tempdir for the duration of this test.
        let cwd_holder = tempdir().unwrap();
        let saved = std::env::current_dir().unwrap();
        std::env::set_current_dir(cwd_holder.path()).unwrap();

        let r = root(Scope::Personal, &config);
        std::env::set_current_dir(saved).unwrap();

        // Personal falls back to the global notez_root when not inside git.
        assert_eq!(r.unwrap(), dir.path());
    }

    #[test]
    #[serial_test::serial]
    fn personal_inside_git_uses_personal_subdir() {
        let project_dir = tempdir().unwrap();
        std::process::Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(project_dir.path())
            .status()
            .unwrap();

        let notez_root = tempdir().unwrap();
        let config = config_with_root(notez_root.path());

        let saved = std::env::current_dir().unwrap();
        std::env::set_current_dir(project_dir.path()).unwrap();
        let r = root(Scope::Personal, &config);
        std::env::set_current_dir(saved).unwrap();

        let p = r.unwrap();
        // Path ends with `personal/<sanitized-tempdir-name>/`.
        assert!(
            p.starts_with(notez_root.path().join("personal")),
            "got {:?}",
            p,
        );
    }

    #[test]
    #[serial_test::serial]
    fn local_uses_dot_notez_in_cwd() {
        let dir = tempdir().unwrap();
        let canonical = dir.path().canonicalize().unwrap();
        let config = Config::defaults();

        let saved = std::env::current_dir().unwrap();
        std::env::set_current_dir(&canonical).unwrap();
        let r = root(Scope::Local, &config);
        std::env::set_current_dir(saved).unwrap();

        // root() returns cwd.join(".notez"); cwd was already canonical.
        assert_eq!(r.unwrap(), canonical.join(".notez"));
    }

    #[test]
    #[serial_test::serial]
    fn public_uses_notez_in_cwd() {
        let dir = tempdir().unwrap();
        let canonical = dir.path().canonicalize().unwrap();
        let config = Config::defaults();

        let saved = std::env::current_dir().unwrap();
        std::env::set_current_dir(&canonical).unwrap();
        let r = root(Scope::Public, &config);
        std::env::set_current_dir(saved).unwrap();

        assert_eq!(r.unwrap(), canonical.join("notez"));
    }

    #[test]
    fn quick_notes_joins_subdir() {
        let dir = tempdir().unwrap();
        let mut config = config_with_root(dir.path());
        config.paths.quick_notes_dir = "00_qn".to_string();

        let r = quick_notes(Scope::Global, &config).unwrap();
        assert_eq!(r, dir.path().join("00_qn"));
    }

    /// The todos store names are on-disk layout shared with existing vaults
    /// and epoz; renaming either would orphan every todo list.
    #[test]
    fn todos_store_names_match_the_vault_layout() {
        assert_eq!(TODOS_DIR, "_todos");
        assert_eq!(TODO_FILE, "TODO.md");
    }

    /// Create a git repo with a nested subdirectory. Returns (repo root, subdir),
    /// both canonicalized so they compare equal to what git reports.
    fn git_repo_with_subdir() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let dir = tempdir().unwrap();
        let toplevel = dir.path().canonicalize().unwrap();
        std::process::Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(&toplevel)
            .status()
            .unwrap();
        let sub = toplevel.join("crates").join("deep");
        std::fs::create_dir_all(&sub).unwrap();
        (dir, toplevel, sub)
    }

    #[test]
    #[serial_test::serial]
    fn local_resolves_to_project_root_from_a_subdirectory() {
        let (_guard, toplevel, sub) = git_repo_with_subdir();
        let config = Config::defaults();

        let saved = std::env::current_dir().unwrap();
        std::env::set_current_dir(&sub).unwrap();
        let r = root(Scope::Local, &config);
        std::env::set_current_dir(saved).unwrap();

        assert_eq!(r.unwrap(), toplevel.join(".notez"));
    }

    #[test]
    #[serial_test::serial]
    fn public_resolves_to_project_root_from_a_subdirectory() {
        let (_guard, toplevel, sub) = git_repo_with_subdir();
        let config = Config::defaults();

        let saved = std::env::current_dir().unwrap();
        std::env::set_current_dir(&sub).unwrap();
        let r = root(Scope::Public, &config);
        std::env::set_current_dir(saved).unwrap();

        assert_eq!(r.unwrap(), toplevel.join("notez"));
    }

    #[test]
    #[serial_test::serial]
    fn daily_logs_follow_the_project_root_from_a_subdirectory() {
        let (_guard, toplevel, sub) = git_repo_with_subdir();
        let mut config = Config::defaults();
        config.paths.daily_logs_dir = "01_daily-logs".to_string();

        let saved = std::env::current_dir().unwrap();
        std::env::set_current_dir(&sub).unwrap();
        let r = daily_logs(Scope::Local, &config);
        std::env::set_current_dir(saved).unwrap();

        assert_eq!(r.unwrap(), toplevel.join(".notez").join("01_daily-logs"));
    }
}
