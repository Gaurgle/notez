//! `notez cp [term]` / `notez cpwd [term]`: put a note's content or its
//! absolute path on the clipboard. The note is found as `edit` finds it, so
//! a bare call opens the same picker. Prints nothing on success.

use std::path::Path;

use anyhow::{Context, Result};

use notez_core::config::Config;
use notez_core::core::Scope;

use crate::commands::edit;
use crate::tui::clipboard;

/// What `cp` and `cpwd` copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Copy {
    Content,
    Path,
}

pub fn run(term: Option<String>, what: Copy, scope: Scope, config: &Config) -> Result<()> {
    let chosen = edit::choose_note(term.as_deref(), scope, config)?;
    copy_note(&chosen.path, what, &mut clipboard::copy)
}

/// Copy the note at `path` with `write`, the clipboard in `run`.
pub fn copy_note(
    path: &Path,
    what: Copy,
    write: &mut dyn FnMut(&str) -> Result<()>,
) -> Result<()> {
    match what {
        Copy::Content => {
            let text = std::fs::read_to_string(path)
                .with_context(|| format!("failed to read {}", path.display()))?;
            write(&text)
        }
        Copy::Path => {
            let absolute = std::path::absolute(path)
                .with_context(|| format!("failed to resolve {}", path.display()))?;
            write(&absolute.to_string_lossy())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn copied(path: &Path, what: Copy) -> Result<Vec<String>> {
        let mut written = Vec::new();
        copy_note(path, what, &mut |text: &str| {
            written.push(text.to_string());
            Ok(())
        })?;
        Ok(written)
    }

    #[test]
    fn cp_copies_the_note_content() {
        let dir = tempfile::tempdir().unwrap();
        let note = dir.path().join("idea.md");
        std::fs::write(&note, "# idea\nbody\n").unwrap();
        assert_eq!(copied(&note, Copy::Content).unwrap(), ["# idea\nbody\n"]);
    }

    #[test]
    fn cpwd_copies_the_note_absolute_path() {
        let dir = tempfile::tempdir().unwrap();
        let note = dir.path().join("idea.md");
        std::fs::write(&note, "# idea\n").unwrap();
        assert_eq!(
            copied(&note, Copy::Path).unwrap(),
            [note.to_string_lossy().into_owned()]
        );
    }

    #[test]
    fn cpwd_makes_a_relative_path_absolute() {
        let got = copied(Path::new("idea.md"), Copy::Path).unwrap();
        let want = std::env::current_dir().unwrap().join("idea.md");
        assert_eq!(got, [want.to_string_lossy().into_owned()]);
    }

    #[test]
    fn cp_of_a_missing_note_is_an_error_and_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let err = copied(&dir.path().join("gone.md"), Copy::Content).unwrap_err();
        assert!(err.to_string().starts_with("failed to read"), "{err:#}");
    }

    #[test]
    fn a_writer_error_is_returned() {
        let dir = tempfile::tempdir().unwrap();
        let note = dir.path().join("idea.md");
        std::fs::write(&note, "x").unwrap();
        for what in [Copy::Content, Copy::Path] {
            let err = copy_note(&note, what, &mut |_: &str| {
                anyhow::bail!("no clipboard tool found: install pbcopy")
            })
            .unwrap_err();
            assert_eq!(err.to_string(), "no clipboard tool found: install pbcopy");
        }
    }
}
