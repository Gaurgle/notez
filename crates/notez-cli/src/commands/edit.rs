//! `notez edit [term]` / `editz`: open an existing note.
//!
//! Candidates come from the scope model rather than a bespoke filesystem
//! walk, so `edit` sees exactly the notes the rest of the tool considers in
//! scope. A term that matches exactly one note skips the picker.

use std::process::Command;

use anyhow::{Context, Result, bail};

use notez_core::config::Config;
use notez_core::core::aggregate::{self, NoteEntry};
use notez_core::core::{Project, Scope};

use crate::commands::picker;

/// Notes whose filename contains `term`, case-insensitively.
pub fn filter_by_term(entries: &[NoteEntry], term: &str) -> Vec<NoteEntry> {
    let needle = term.to_lowercase();
    entries
        .iter()
        .filter(|e| e.name.to_lowercase().contains(&needle))
        .cloned()
        .collect()
}

/// Resolve `term` to a single note in `scope`, asking via the picker when
/// several match.
pub fn choose_note(term: Option<&str>, scope: Scope, config: &Config) -> Result<NoteEntry> {
    let project = Project::try_detect();
    let entries = aggregate::collect_in_scope(scope, config, project.as_ref());
    if entries.is_empty() {
        bail!("no notes found in {} scope", scope.label());
    }

    let candidates = match term {
        Some(t) => {
            let matched = filter_by_term(&entries, t);
            if matched.is_empty() {
                bail!("no notes matching \"{}\" in {} scope", t, scope.label());
            }
            matched
        }
        None => entries,
    };

    if candidates.len() == 1 {
        return Ok(candidates[0].clone());
    }
    let labels: Vec<String> = candidates.iter().map(|e| e.name.clone()).collect();
    let index = picker::pick("note> ", &labels, config.tools.fzf)?;
    Ok(candidates[index].clone())
}

pub fn run(term: Option<String>, scope: Scope, config: &Config) -> Result<()> {
    let chosen = choose_note(term.as_deref(), scope, config)?;
    Command::new(&config.editor.command)
        .arg(&chosen.path)
        .status()
        .with_context(|| format!("failed to launch {}", config.editor.command))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use notez_core::core::aggregate::SourceKind;
    use std::path::PathBuf;

    fn entry(name: &str) -> NoteEntry {
        NoteEntry {
            path: PathBuf::from("/vault").join(name),
            name: name.to_string(),
            scope: Scope::Global,
            project: None,
            kind: SourceKind::Note,
        }
    }

    #[test]
    fn filter_matches_on_substring_case_insensitively() {
        let entries = vec![
            entry("2026-04-02-API-design.md"),
            entry("2026-04-02-bug-fix.md"),
            entry("2026-04-01-meeting.md"),
        ];

        let got = filter_by_term(&entries, "api");

        assert_eq!(got.len(), 1);
        assert_eq!(got[0].name, "2026-04-02-API-design.md");
    }

    #[test]
    fn filter_can_match_several() {
        let entries = vec![
            entry("api-design.md"),
            entry("api-notes.md"),
            entry("meeting.md"),
        ];
        assert_eq!(filter_by_term(&entries, "api").len(), 2);
    }

    #[test]
    fn filter_with_no_match_is_empty() {
        let entries = vec![entry("meeting.md")];
        assert!(filter_by_term(&entries, "api").is_empty());
    }
}
