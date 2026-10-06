//! `notez tree` / `treez`: interactive tree browser TUI.
//!
//! Assembles [`SectionSpec`]s out of the aggregator (registry + scopes +
//! project docs, no symlink walking) and hands them to `tui::tree`. On
//! exit, only `.tags` roots whose tag maps actually changed are written.

use std::path::PathBuf;

use anyhow::Result;

use notez_core::config::{Config, NotezMetadata, ProjectRegistry};
use notez_core::core::aggregate::{self, NoteEntry, SourceKind};
use notez_core::core::{Project, Scope};
use notez_core::note_tags;
use notez_core::util::tilde;

use crate::tui::tree::{SectionSpec, TreeContext, run_tree};

/// Nerdfont book icon for docs sections (scopes use `Scope::icon`).
const ICON_DOCS: &str = "\u{f02d}";

/// Open the tree browser on `scope`. `warning`, if any, shows in the status
/// bar as the browser opens.
pub fn run(scope: Scope, config: &Config, warning: Option<&str>) -> Result<()> {
    let registry = ProjectRegistry::load().unwrap_or_default();
    let (sections, mut ctx) = build_view(scope, config, &registry)?;
    ctx.warning = warning.map(str::to_string);

    if sections.iter().all(|s| s.files.is_empty()) {
        println!("\n  - No notes here.\n");
        return Ok(());
    }

    let changed = run_tree(sections, &ctx, config)?;
    for (root, map) in &changed {
        note_tags::save_tags(root, map)?;
    }
    Ok(())
}

/// Section order within a project: personal, public, docs, local.
fn scope_rank(scope: Scope, kind: SourceKind) -> u8 {
    match (scope, kind) {
        (Scope::Personal, _) => 0,
        (Scope::Public, SourceKind::Note) => 1,
        (Scope::Public, SourceKind::Doc) => 2,
        (Scope::Local, _) => 3,
        (Scope::Global, _) => 4,
    }
}

fn section_meta(
    scope: Scope,
    kind: SourceKind,
    project: &str,
    repo: &PathBuf,
    notez_root: &PathBuf,
) -> (PathBuf, PathBuf, String, &'static str) {
    match (scope, kind) {
        (Scope::Personal, _) => (
            notez_root.join("personal").join(project),
            notez_root.clone(),
            format!("{project} (personal)"),
            Scope::Personal.icon(),
        ),
        (Scope::Public, SourceKind::Note) => (
            repo.join("notez"),
            repo.join("notez"),
            format!("{project} (public)"),
            Scope::Public.icon(),
        ),
        (Scope::Public, SourceKind::Doc) => (
            repo.join("docs"),
            repo.join("docs"),
            format!("{project} (docs)"),
            ICON_DOCS,
        ),
        (Scope::Local, _) => (
            repo.join(".notez"),
            repo.join(".notez"),
            format!("{project} (scratch)"),
            Scope::Local.icon(),
        ),
        (Scope::Global, _) => (
            notez_root.clone(),
            notez_root.clone(),
            "NOTEZ".to_string(),
            Scope::Global.icon(),
        ),
    }
}

/// Group aggregator entries into ordered sections: NOTEZ (global) first, then each
/// project alphabetically with personal / public / docs / local sections.
fn sections_from_entries(
    entries: Vec<NoteEntry>,
    config: &Config,
    registry: &ProjectRegistry,
) -> Vec<SectionSpec> {
    let notez_root = config.notez_root_path();
    let repo_paths: std::collections::BTreeMap<String, PathBuf> = registry
        .iter_resolved()
        .map(|(n, p)| (n.to_string(), p))
        .collect();

    // Grouping key sorts NOTEZ (bucket 0) ahead of the projects (bucket 1).
    let mut grouped: std::collections::BTreeMap<(u8, String, u8), Vec<PathBuf>> =
        std::collections::BTreeMap::new();
    for entry in entries {
        let (bucket, project) = match &entry.project {
            None => (0u8, String::new()),
            Some(p) => (1u8, p.clone()),
        };
        let rank = scope_rank(entry.scope, entry.kind);
        grouped
            .entry((bucket, project, rank))
            .or_default()
            .push(entry.path);
    }

    let mut out = Vec::new();
    for ((bucket, project, rank), files) in grouped {
        let (scope, kind) = match rank {
            0 => (Scope::Personal, SourceKind::Note),
            1 => (Scope::Public, SourceKind::Note),
            2 => (Scope::Public, SourceKind::Doc),
            3 => (Scope::Local, SourceKind::Note),
            _ => (Scope::Global, SourceKind::Note),
        };
        let repo = repo_paths
            .get(&project)
            .cloned()
            .unwrap_or_else(|| notez_root.clone());
        let (root, tag_root, label, icon) = if bucket == 0 {
            section_meta(Scope::Global, SourceKind::Note, "", &repo, &notez_root)
        } else {
            section_meta(scope, kind, &project, &repo, &notez_root)
        };
        out.push(SectionSpec {
            root,
            tag_root,
            label,
            icon,
            is_doc: kind == SourceKind::Doc,
            files,
        });
    }
    out
}

fn build_view(
    scope: Scope,
    config: &Config,
    registry: &ProjectRegistry,
) -> Result<(Vec<SectionSpec>, TreeContext)> {
    let notez_root = config.notez_root_path();
    let metadata = NotezMetadata::default();

    match scope {
        Scope::Global => {
            let entries = aggregate::collect_all(config, registry, &metadata)?;
            let sections = sections_from_entries(entries, config, registry);
            Ok((
                sections,
                TreeContext {
                    title: "notez (global)".to_string(),
                    path_display: tilde::contract(&notez_root),
                    warning: None,
                },
            ))
        }
        Scope::Personal => {
            // Default view: every scope of the current project. Falls back
            // to the global view outside a project (mirroring how the
            // personal scope itself falls back).
            let Some(project) = Project::try_detect() else {
                return build_view(Scope::Global, config, registry);
            };
            let attached = registry
                .iter_resolved()
                .any(|(name, _)| name == project.name);
            let entries: Vec<NoteEntry> = if attached {
                aggregate::collect_all(config, registry, &metadata)?
                    .into_iter()
                    .filter(|e| e.project.as_deref() == Some(project.name.as_str()))
                    .collect()
            } else {
                let mut v = Vec::new();
                for s in [Scope::Personal, Scope::Public, Scope::Local] {
                    v.extend(aggregate::collect_in_scope(s, config, Some(&project)));
                }
                v
            };
            let sections = sections_from_entries(entries, config, registry);
            Ok((
                sections,
                TreeContext {
                    title: format!("notez ({})", project.name),
                    path_display: tilde::contract(&project.root),
                    warning: None,
                },
            ))
        }
        Scope::Public | Scope::Local => {
            let project = Project::try_detect();
            let entries = aggregate::collect_in_scope(scope, config, project.as_ref());
            let sections = sections_from_entries(entries, config, registry);
            let name = project
                .as_ref()
                .map(|p| p.name.clone())
                .unwrap_or_else(|| scope.to_string());
            Ok((
                sections,
                TreeContext {
                    title: format!("{} notez ({})", scope.icon(), name),
                    path_display: if scope == Scope::Public {
                        "./notez".to_string()
                    } else {
                        "./.notez".to_string()
                    },
                    warning: None,
                },
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str, scope: Scope, project: Option<&str>, kind: SourceKind) -> NoteEntry {
        NoteEntry {
            path: PathBuf::from(path),
            name: PathBuf::from(path)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string(),
            scope,
            project: project.map(|s| s.to_string()),
            kind,
        }
    }

    #[test]
    fn global_section_comes_first_then_projects_with_docs() {
        let mut config = Config::defaults();
        config.paths.notez_root = "/nr".to_string();
        let registry = ProjectRegistry::default();

        let entries = vec![
            entry(
                "/repo/docs/hw.md",
                Scope::Public,
                Some("proj"),
                SourceKind::Doc,
            ),
            entry("/nr/top.md", Scope::Global, None, SourceKind::Note),
            entry(
                "/nr/personal/proj/n.md",
                Scope::Personal,
                Some("proj"),
                SourceKind::Note,
            ),
        ];
        let sections = sections_from_entries(entries, &config, &registry);
        let labels: Vec<&str> = sections.iter().map(|s| s.label.as_str()).collect();
        assert_eq!(labels, vec!["NOTEZ", "proj (personal)", "proj (docs)"]);
        assert!(sections[2].is_doc);
    }

    #[test]
    fn personal_sections_share_the_notez_root_tag_root() {
        let mut config = Config::defaults();
        config.paths.notez_root = "/nr".to_string();
        let registry = ProjectRegistry::default();
        let entries = vec![entry(
            "/nr/personal/proj/n.md",
            Scope::Personal,
            Some("proj"),
            SourceKind::Note,
        )];
        let sections = sections_from_entries(entries, &config, &registry);
        assert_eq!(sections[0].root, PathBuf::from("/nr/personal/proj"));
        assert_eq!(sections[0].tag_root, PathBuf::from("/nr"));
    }
}
