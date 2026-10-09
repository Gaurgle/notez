//! `notez rename [term] [title]`: retitle an existing note.
//!
//! The filename keeps its `YYYY-MM-DD-` prefix, if it has one, and gets a freshly sanitized
//! slug, and a leading `# ` heading in the body is rewritten to match. An
//! existing file is never overwritten.

use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use notez_core::config::Config;
use notez_core::core::Scope;
use notez_core::util::sanitize;

use crate::commands::edit;

/// The `YYYY-MM-DD-` prefix of a note filename, or "" when it has none.
fn date_prefix(file_name: &str) -> &str {
    let bytes = file_name.as_bytes();
    let looks_dated = bytes.len() > 11
        && bytes[..11].iter().enumerate().all(|(i, b)| match i {
            4 | 7 | 10 => *b == b'-',
            _ => b.is_ascii_digit(),
        });
    if looks_dated { &file_name[..11] } else { "" }
}

/// The editable part of a note filename: no date prefix, no `.md`.
pub fn editable_title(file_name: &str) -> String {
    let rest = &file_name[date_prefix(file_name).len()..];
    rest.strip_suffix(".md").unwrap_or(rest).to_string()
}

/// New filename for `old_name` retitled to `title`.
pub fn renamed_filename(old_name: &str, title: &str) -> Result<String> {
    let slug = sanitize::name(title);
    if slug.is_empty() {
        bail!("title \"{}\" has no usable characters", title);
    }
    Ok(format!("{}{}.md", date_prefix(old_name), slug))
}

/// Replace a leading `# ` heading with the new title; other content is kept.
fn retitle_heading(content: &str, title: &str) -> String {
    match content.split_once('\n') {
        Some((first, rest)) if first.starts_with("# ") => format!("# {title}\n{rest}"),
        None if content.starts_with("# ") => format!("# {title}"),
        _ => content.to_string(),
    }
}

/// Rename the note at `path` to `title`, returning the new path.
pub fn rename_note(path: &Path, title: &str) -> Result<PathBuf> {
    let old_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .with_context(|| format!("{} has no usable file name", path.display()))?;
    let target = path.with_file_name(renamed_filename(old_name, title)?);
    if target == path {
        return Ok(target);
    }
    if target.exists() {
        bail!("{} already exists", target.display());
    }

    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    std::fs::rename(path, &target)
        .with_context(|| format!("failed to rename {}", path.display()))?;
    let retitled = retitle_heading(&content, title.trim());
    if retitled != content {
        std::fs::write(&target, retitled)
            .with_context(|| format!("failed to update heading in {}", target.display()))?;
    }
    Ok(target)
}

fn prompt_title(current: &str) -> Result<String> {
    print!("new title for {current}: ");
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;
    Ok(line.trim().to_string())
}

pub fn run(
    term: Option<String>,
    title_words: Vec<String>,
    scope: Scope,
    config: &Config,
) -> Result<PathBuf> {
    let chosen = edit::choose_note(term.as_deref(), scope, config)?;
    let title = if title_words.is_empty() {
        prompt_title(&chosen.name)?
    } else {
        title_words.join(" ")
    };
    rename_note(&chosen.path, &title)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_date_prefix_and_slugifies_title() {
        let got = renamed_filename("2026-10-06-untitled.md", "Fragrance Layering!").unwrap();
        assert_eq!(got, "2026-10-06-fragrance-layering.md");
    }

    #[test]
    fn editable_title_drops_prefix_and_extension() {
        assert_eq!(editable_title("2026-10-06-untitled.md"), "untitled");
        assert_eq!(editable_title("scratch.md"), "scratch");
    }

    #[test]
    fn undated_name_gets_no_prefix() {
        assert_eq!(renamed_filename("scratch.md", "Ideas").unwrap(), "ideas.md");
    }

    #[test]
    fn rejects_title_without_usable_characters() {
        assert!(renamed_filename("2026-10-06-untitled.md", "?!").is_err());
    }

    #[test]
    fn rename_moves_file_and_rewrites_heading() {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("2026-10-06-untitled.md");
        std::fs::write(&old, "# untitled\n\nDate: 2026-10-06\n\nbody\n").unwrap();

        let new = rename_note(&old, "Real Name").unwrap();

        assert_eq!(new, dir.path().join("2026-10-06-real-name.md"));
        assert!(!old.exists());
        assert_eq!(
            std::fs::read_to_string(&new).unwrap(),
            "# Real Name\n\nDate: 2026-10-06\n\nbody\n"
        );
    }

    #[test]
    fn rename_leaves_body_without_heading_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("2026-10-06-untitled.md");
        std::fs::write(&old, "just text\n").unwrap();

        let new = rename_note(&old, "named").unwrap();

        assert_eq!(std::fs::read_to_string(new).unwrap(), "just text\n");
    }

    #[test]
    fn rename_refuses_to_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("2026-10-06-untitled.md");
        let taken = dir.path().join("2026-10-06-taken.md");
        std::fs::write(&old, "a").unwrap();
        std::fs::write(&taken, "b").unwrap();

        assert!(rename_note(&old, "taken").is_err());
        assert_eq!(std::fs::read_to_string(&taken).unwrap(), "b");
        assert!(old.exists());
    }
}
