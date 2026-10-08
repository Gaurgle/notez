//! `notez mkdir`: create a new subdirectory under the scope's root.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use notez_core::config::Config;
use notez_core::core::{Scope, project, resolve};
use notez_core::util::sanitize;

/// Create the directory. Returns its absolute path.
pub fn run(name_words: Vec<String>, scope: Scope, config: &Config) -> Result<PathBuf> {
    let raw = name_words.join(" ");
    // Checked before the root resolves, so an empty name reports as such
    // even where the scope has no root (local outside a project).
    cleaned_name(&raw)?;
    let root = resolve::root(scope, config)?;
    create_in_dir(&root, &raw, scope)
}

/// Create the folder `name` in `dir`: the creation path shared by `notez
/// mkdir` and the tree browser's new-folder prompt. The name is sanitized
/// (`sanitize::name`); an empty result is an error. Creates missing parents
/// with `create_dir_all`, so an existing folder is not an error here: a
/// caller that must refuse one checks first. For [`Scope::Local`] the
/// scratch store is gitignored. Returns the folder's path.
pub fn create_in_dir(dir: &Path, name: &str, scope: Scope) -> Result<PathBuf> {
    let path = dir.join(cleaned_name(name)?);
    std::fs::create_dir_all(&path)
        .with_context(|| format!("failed to create directory {}", path.display()))?;
    if scope == Scope::Local {
        project::ensure_scratch_gitignored(&path);
    }
    Ok(path)
}

/// `raw` sanitized for a folder name; an error when nothing is left.
fn cleaned_name(raw: &str) -> Result<String> {
    let cleaned = sanitize::name(raw);
    if cleaned.is_empty() {
        bail!("directory name cannot be empty");
    }
    Ok(cleaned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn config_in(root: &std::path::Path) -> Config {
        let mut c = Config::defaults();
        c.paths.notez_root = root.to_string_lossy().into_owned();
        c
    }

    #[test]
    fn creates_global_subdir() {
        let dir = tempdir().unwrap();
        let config = config_in(dir.path());

        let p = run(vec!["my".into(), "ideas".into()], Scope::Global, &config).unwrap();
        assert!(p.is_dir());
        assert!(p.ends_with("my-ideas"));
    }

    #[test]
    fn empty_name_errors() {
        let dir = tempdir().unwrap();
        let config = config_in(dir.path());
        let err = run(vec!["   ".into()], Scope::Global, &config).unwrap_err();
        assert!(err.to_string().contains("empty"));
    }

    #[test]
    fn sanitizes_specials() {
        let dir = tempdir().unwrap();
        let config = config_in(dir.path());
        let p = run(vec!["Hello, World!".into()], Scope::Global, &config).unwrap();
        assert!(p.ends_with("hello-world"));
    }

    /// For every scope, `create_in_dir` on the scope's root makes exactly the
    /// folder `run` makes, and the local scope gets its `.gitignore` entry
    /// either way.
    #[test]
    #[serial_test::serial]
    fn create_in_dir_matches_run_in_every_scope() {
        let vault = tempdir().unwrap();
        let repo = tempdir().unwrap();
        std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(repo.path())
            .status()
            .unwrap();
        let config = config_in(vault.path());
        let saved = std::env::current_dir().unwrap();
        std::env::set_current_dir(repo.path()).unwrap();

        let mut results = Vec::new();
        for scope in [Scope::Personal, Scope::Public, Scope::Local, Scope::Global] {
            let root = resolve::root(scope, &config).unwrap();
            let carved = create_in_dir(&root, "My Ideas", scope).unwrap();
            let carved_ignored = std::fs::read_to_string(repo.path().join(".gitignore"));
            std::fs::remove_dir(&carved).unwrap();
            let _ = std::fs::remove_file(repo.path().join(".gitignore"));
            let ran = run(vec!["My".into(), "Ideas".into()], scope, &config).unwrap();
            let ran_ignored = std::fs::read_to_string(repo.path().join(".gitignore"));
            let _ = std::fs::remove_file(repo.path().join(".gitignore"));
            results.push((scope, root, carved, ran, carved_ignored.ok(), ran_ignored.ok()));
        }
        std::env::set_current_dir(saved).unwrap();

        for (scope, root, carved, ran, carved_ignored, ran_ignored) in results {
            assert_eq!(carved, ran, "{scope:?}");
            assert_eq!(ran, root.join("my-ideas"), "{scope:?}");
            assert!(ran.is_dir(), "{scope:?}");
            assert_eq!(carved_ignored, ran_ignored, "{scope:?}");
            let ignored = carved_ignored.is_some_and(|g| g.lines().any(|l| l == ".notez"));
            assert_eq!(ignored, scope == Scope::Local, "{scope:?}");
        }
    }

    #[test]
    fn create_in_dir_refuses_an_empty_name_and_creates_nothing() {
        let dir = tempdir().unwrap();
        for name in ["", "   ", "!!!"] {
            let err = create_in_dir(dir.path(), name, Scope::Global).unwrap_err();
            assert!(err.to_string().contains("empty"), "{name:?}");
        }
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }
}
