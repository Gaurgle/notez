//! Aggregate notes from the four scopes into a unified collection.
//!
//! This module owns the logic that replaces notez-cli's filesystem symlink
//! walking. The TUI tree browser and the todoz aggregator both consume the
//! same `NoteEntry` stream from here, so adding a new scope only takes
//! changes in one place.

use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::config::{Config, NotezMetadata, ProjectRegistry};
use crate::core::{Project, Scope};

/// What kind of source a [`NoteEntry`] came from. `Doc` entries are markdown
/// found in a project's `docs/` directory: repo documentation surfaced next
/// to notes, not notes themselves.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceKind {
    #[default]
    Note,
    Doc,
}

/// A single discoverable note or content directory.
///
/// Entries carry their scope so the TUI can render the lock/user/globe/home
/// icon next to each row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteEntry {
    /// Absolute path on disk.
    pub path: PathBuf,
    /// Filename only (e.g. `2026-05-13-my-idea.md`).
    pub name: String,
    /// Which scope this entry came from.
    pub scope: Scope,
    /// Owning project name. `None` for global-only notes that do not belong
    /// to any project.
    pub project: Option<String>,
    /// Note vs project doc. Defaults to `Note` for wire compatibility.
    #[serde(default)]
    pub kind: SourceKind,
}

/// Walk a directory recursively and collect every `.md` file under it.
///
/// Hidden directories (those whose name starts with `.`) are skipped at
/// the top level except for the root itself.
fn walk_markdown(root: &Path) -> Vec<PathBuf> {
    if !root.exists() {
        return Vec::new();
    }
    WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| {
            // Always descend into the root.
            if e.depth() == 0 {
                return true;
            }
            !e.file_name().to_string_lossy().starts_with('.')
        })
        .filter_map(Result::ok)
        .filter(|e| {
            e.file_type().is_file()
                && e.path()
                    .extension()
                    .and_then(|s| s.to_str())
                    .map_or(false, |ext| ext.eq_ignore_ascii_case("md"))
        })
        .map(|e| e.into_path())
        .collect()
}

/// Collect notes for a specific scope and project context.
///
/// `cwd_project`: the project the user is currently inside (None if not in
/// a git repo). Used by Local, Personal and Public scopes.
pub fn collect_in_scope(
    scope: Scope,
    config: &Config,
    cwd_project: Option<&Project>,
) -> Vec<NoteEntry> {
    let project_name = cwd_project.map(|p| p.name.clone());
    let root = match scope {
        Scope::Local => cwd_project.map(|p| p.root.join(".notez")),
        Scope::Public => cwd_project.map(|p| p.root.join("notez")),
        Scope::Personal => match cwd_project {
            Some(p) => Some(config.notez_root_path().join("personal").join(&p.name)),
            None => Some(config.notez_root_path()),
        },
        Scope::Global => Some(config.notez_root_path()),
    };

    let Some(root) = root else {
        return Vec::new();
    };

    let personal_root = config.notez_root_path().join("personal");
    let mut entries = Vec::new();
    for path in walk_markdown(&root) {
        // For the global scope, the personal/ subtree belongs to projects.
        if scope == Scope::Global && path.starts_with(&personal_root) {
            continue;
        }
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        entries.push(NoteEntry {
            path: path.clone(),
            name: name.to_string(),
            scope,
            project: project_name.clone(),
            kind: SourceKind::Note,
        });
    }
    entries
}

/// Aggregate every visible note for the global TUI view.
///
/// Walks:
/// - Every project in `registry`: collects its Local (`.notez/`) and Public
///   (`notez/`) trees plus its Personal subtree at `<notez_root>/personal/<name>/`
/// - Every other `<notez_root>/personal/<name>/` folder, as Personal notes of
///   project `<name>`, so a project not registered on this machine is still
///   listed
/// - The global notez root for cross-project notes (excluding the `personal/`
///   subtree, which is already attributed to projects above)
///
/// Projects whose `local_path` does not exist on this machine are skipped
/// silently. The TUI surfaces them separately so the user knows which
/// projects' notes are unreachable on this machine.
pub fn collect_all(
    config: &Config,
    registry: &ProjectRegistry,
    _metadata: &NotezMetadata,
) -> Result<Vec<NoteEntry>> {
    let mut out = Vec::new();
    let notez_root = config.notez_root_path();

    for (name, local_path) in registry.iter_resolved() {
        // Skip projects that aren't on this machine.
        if !local_path.exists() {
            continue;
        }
        let project = Project {
            name: name.to_string(),
            root: local_path.clone(),
        };

        for path in walk_markdown(&local_path.join(".notez")) {
            push_entry(&mut out, path, Scope::Local, Some(&project.name));
        }
        for path in walk_markdown(&local_path.join("notez")) {
            push_entry(&mut out, path, Scope::Public, Some(&project.name));
        }
        let personal_dir = notez_root.join("personal").join(&project.name);
        for path in walk_markdown(&personal_dir) {
            push_entry(&mut out, path, Scope::Personal, Some(&project.name));
        }
        // Repo documentation: surfaced alongside notes so everything about a
        // project is reachable from one place, but tagged Doc so UIs can
        // label it. Committed with the repo, hence Public scope.
        if config.paths.project_docs {
            for path in walk_markdown(&local_path.join("docs")) {
                push_doc(&mut out, path, Some(&project.name));
            }
        }
    }

    let global_root = config.notez_root_path();
    let personal_root = global_root.join("personal");
    for (name, dir) in unregistered_personal_dirs(&personal_root, registry) {
        for path in walk_markdown(&dir) {
            push_entry(&mut out, path, Scope::Personal, Some(&name));
        }
    }
    for path in walk_markdown(&global_root) {
        // The personal/ subtree is attributed to its owning project above;
        // surfacing those files as global notes too would double-count them.
        if path.starts_with(&personal_root) {
            continue;
        }
        push_entry(&mut out, path, Scope::Global, None);
    }

    // Safety net: a single physical file can be reachable under more than one
    // root (e.g. symlinked project dirs). Keep the first — i.e. most specific
    // scope — occurrence of each path so the combined view never duplicates.
    let mut seen = std::collections::HashSet::new();
    out.retain(|e| seen.insert(e.path.clone()));

    Ok(out)
}

/// The `personal/<name>/` folders whose project is not in `registry`, sorted
/// by name. Their notes are personal notes of a project this machine has no
/// repository for, so they are listed under that project's name. Anything
/// under `personal/` that is not a readable directory (a loose file, a
/// dangling symlink) and hidden names are skipped, as the walk skips them.
fn unregistered_personal_dirs(
    personal_root: &Path,
    registry: &ProjectRegistry,
) -> Vec<(String, PathBuf)> {
    let Ok(read) = std::fs::read_dir(personal_root) else {
        return Vec::new();
    };
    let mut dirs: Vec<(String, PathBuf)> = read
        .filter_map(Result::ok)
        .filter_map(|e| {
            let name = e.file_name().to_str()?.to_string();
            let path = e.path();
            let listed =
                !name.starts_with('.') && !registry.projects.contains_key(&name) && path.is_dir();
            listed.then_some((name, path))
        })
        .collect();
    dirs.sort();
    dirs
}

fn push_entry(out: &mut Vec<NoteEntry>, path: PathBuf, scope: Scope, project: Option<&str>) {
    push_with_kind(out, path, scope, project, SourceKind::Note);
}

fn push_doc(out: &mut Vec<NoteEntry>, path: PathBuf, project: Option<&str>) {
    push_with_kind(out, path, Scope::Public, project, SourceKind::Doc);
}

fn push_with_kind(
    out: &mut Vec<NoteEntry>,
    path: PathBuf,
    scope: Scope,
    project: Option<&str>,
    kind: SourceKind,
) {
    let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
        return;
    };
    out.push(NoteEntry {
        path: path.clone(),
        name: name.to_string(),
        scope,
        project: project.map(|s| s.to_string()),
        kind,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn touch(path: &Path) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, "# placeholder\n").unwrap();
    }

    fn config_with_root(root: &Path) -> Config {
        let mut c = Config::defaults();
        c.paths.notez_root = root.to_string_lossy().into_owned();
        c
    }

    #[test]
    #[serial_test::serial]
    fn collect_all_surfaces_project_docs_as_doc_kind() {
        let tmp = tempdir().unwrap();
        unsafe { std::env::set_var("XDG_CONFIG_HOME", tmp.path().join("xdg")) };
        let notez_root = tmp.path().join("root");
        let repo = tmp.path().join("repo");
        touch(&repo.join("docs").join("hardware.md"));
        touch(&repo.join("notez").join("pub.md"));

        let config = config_with_root(&notez_root);
        let mut registry = crate::config::ProjectRegistry::default();
        registry.attach("proj", &repo);

        let entries = collect_all(&config, &registry, &Default::default()).unwrap();
        let doc = entries.iter().find(|e| e.name == "hardware.md").unwrap();
        assert_eq!(doc.kind, SourceKind::Doc);
        assert_eq!(doc.scope, Scope::Public);
        assert_eq!(doc.project.as_deref(), Some("proj"));
        let note = entries.iter().find(|e| e.name == "pub.md").unwrap();
        assert_eq!(note.kind, SourceKind::Note);
    }

    #[test]
    fn collect_in_scope_skips_missing_root() {
        let notez_root = tempdir().unwrap();
        let config = config_with_root(notez_root.path());

        // Global root has nothing in it.
        let entries = collect_in_scope(Scope::Global, &config, None);
        assert!(entries.is_empty());
    }

    #[test]
    fn collect_global_returns_md_files() {
        let notez_root = tempdir().unwrap();
        let config = config_with_root(notez_root.path());

        touch(&notez_root.path().join("00_quick-notes").join("a.md"));
        touch(&notez_root.path().join("01_daily-logs").join("2026.md"));
        // Non-md file should be skipped.
        std::fs::write(notez_root.path().join("readme.txt"), "ignore").unwrap();

        let entries = collect_in_scope(Scope::Global, &config, None);
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(entries.len(), 2);
        assert!(names.contains(&"a.md"));
        assert!(names.contains(&"2026.md"));
    }

    #[test]
    fn collect_personal_uses_project_subdir() {
        let notez_root = tempdir().unwrap();
        let config = config_with_root(notez_root.path());
        let project = Project {
            name: "myproj".to_string(),
            root: PathBuf::from("/tmp/myproj"),
        };

        touch(
            &notez_root
                .path()
                .join("personal")
                .join("myproj")
                .join("note-a.md"),
        );

        let entries = collect_in_scope(Scope::Personal, &config, Some(&project));
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "note-a.md");
        assert_eq!(entries[0].scope, Scope::Personal);
        assert_eq!(entries[0].project.as_deref(), Some("myproj"));
    }

    #[test]
    fn collect_local_uses_project_dot_notez() {
        let project_dir = tempdir().unwrap();
        let project = Project {
            name: "p".to_string(),
            root: project_dir.path().to_path_buf(),
        };
        let config = Config::defaults();

        touch(&project_dir.path().join(".notez").join("a.md"));

        let entries = collect_in_scope(Scope::Local, &config, Some(&project));
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "a.md");
        assert_eq!(entries[0].scope, Scope::Local);
    }

    #[test]
    fn collect_public_uses_project_notez() {
        let project_dir = tempdir().unwrap();
        let project = Project {
            name: "p".to_string(),
            root: project_dir.path().to_path_buf(),
        };
        let config = Config::defaults();

        touch(&project_dir.path().join("notez").join("readme.md"));

        let entries = collect_in_scope(Scope::Public, &config, Some(&project));
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "readme.md");
        assert_eq!(entries[0].scope, Scope::Public);
    }

    #[test]
    fn collect_local_without_project_returns_empty() {
        let config = Config::defaults();
        let entries = collect_in_scope(Scope::Local, &config, None);
        assert!(entries.is_empty());
    }

    #[test]
    fn collect_personal_without_project_uses_global_root() {
        let notez_root = tempdir().unwrap();
        let config = config_with_root(notez_root.path());

        touch(&notez_root.path().join("scratch.md"));

        let entries = collect_in_scope(Scope::Personal, &config, None);
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"scratch.md"));
    }

    #[test]
    fn collect_all_aggregates_three_scopes_per_project() {
        let notez_root = tempdir().unwrap();
        let config = config_with_root(notez_root.path());

        let project_dir = tempdir().unwrap();
        touch(&project_dir.path().join(".notez").join("local-note.md"));
        touch(&project_dir.path().join("notez").join("public-note.md"));
        touch(
            &notez_root
                .path()
                .join("personal")
                .join("p")
                .join("personal-note.md"),
        );

        let mut registry = ProjectRegistry::default();
        registry.attach("p", project_dir.path());
        let metadata = NotezMetadata::default();

        let entries = collect_all(&config, &registry, &metadata).unwrap();
        let by_scope: std::collections::HashMap<Scope, Vec<&str>> = entries
            .iter()
            .fold(std::collections::HashMap::new(), |mut acc, e| {
                acc.entry(e.scope).or_default().push(e.name.as_str());
                acc
            });

        assert_eq!(by_scope[&Scope::Local], vec!["local-note.md"]);
        assert_eq!(by_scope[&Scope::Public], vec!["public-note.md"]);
        assert_eq!(by_scope[&Scope::Personal], vec!["personal-note.md"]);
    }

    #[test]
    fn collect_all_skips_unreachable_projects() {
        let notez_root = tempdir().unwrap();
        let config = config_with_root(notez_root.path());

        // Register a project pointing at a path that does not exist.
        let mut registry = ProjectRegistry::default();
        registry.projects.insert(
            "ghost".to_string(),
            crate::config::registry::ProjectEntry {
                local_path: "~/no-such-dir-xyz/ghost-project".to_string(),
            },
        );
        let metadata = NotezMetadata::default();

        let entries = collect_all(&config, &registry, &metadata).unwrap();
        // Ghost project contributes nothing.
        for e in &entries {
            assert_ne!(e.project.as_deref(), Some("ghost"));
        }
    }

    #[test]
    fn collect_all_picks_up_global_notes_too() {
        let notez_root = tempdir().unwrap();
        let config = config_with_root(notez_root.path());

        touch(&notez_root.path().join("00_quick-notes").join("scratch.md"));

        let registry = ProjectRegistry::default();
        let metadata = NotezMetadata::default();

        let entries = collect_all(&config, &registry, &metadata).unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"scratch.md"));
    }

    /// A `personal/<name>/` folder whose project is not registered on this
    /// machine is still listed, as that project's personal notes.
    #[test]
    fn collect_all_lists_unregistered_personal_folders() {
        let notez_root = tempdir().unwrap();
        let config = config_with_root(notez_root.path());
        let personal = notez_root.path().join("personal");

        let project_dir = tempdir().unwrap();
        touch(&project_dir.path().join(".notez").join("scratch.md"));
        touch(&project_dir.path().join("notez").join("pub.md"));
        touch(&personal.join("a").join("x.md"));
        touch(&personal.join("b").join("y.md"));
        touch(&notez_root.path().join("z.md"));
        // A plain file where a project folder would be, and a loose file
        // directly under personal/: neither is a project.
        std::fs::write(personal.join("c"), "not a folder").unwrap();
        touch(&personal.join("loose.md"));

        let mut registry = ProjectRegistry::default();
        registry.attach("a", project_dir.path());

        let entries = collect_all(&config, &registry, &NotezMetadata::default()).unwrap();
        let mut found: Vec<(&str, Scope, Option<&str>)> = entries
            .iter()
            .map(|e| (e.name.as_str(), e.scope, e.project.as_deref()))
            .collect();
        found.sort_by_key(|(name, _, _)| *name);
        let expected = vec![
            ("pub.md", Scope::Public, Some("a")),
            ("scratch.md", Scope::Local, Some("a")),
            ("x.md", Scope::Personal, Some("a")),
            ("y.md", Scope::Personal, Some("b")),
            ("z.md", Scope::Global, None),
        ];
        assert_eq!(found, expected);
    }

    #[test]
    fn collect_all_does_not_duplicate_personal_into_global() {
        let notez_root = tempdir().unwrap();
        let config = config_with_root(notez_root.path());

        let project_dir = tempdir().unwrap();
        // A personal note for project "p" lives under <root>/personal/p/.
        touch(&notez_root.path().join("personal").join("p").join("secret.md"));
        // A genuine global note elsewhere in the root.
        touch(&notez_root.path().join("00_quick").join("global.md"));

        let mut registry = ProjectRegistry::default();
        registry.attach("p", project_dir.path());
        let metadata = NotezMetadata::default();

        let entries = collect_all(&config, &registry, &metadata).unwrap();

        // No path may appear more than once.
        let total = entries.len();
        let mut paths: Vec<_> = entries.iter().map(|e| e.path.clone()).collect();
        paths.sort();
        paths.dedup();
        assert_eq!(paths.len(), total, "collect_all returned duplicate paths");

        // The personal note is attributed to Personal only — never Global.
        let secret_scopes: Vec<Scope> = entries
            .iter()
            .filter(|e| e.name == "secret.md")
            .map(|e| e.scope)
            .collect();
        assert_eq!(secret_scopes, vec![Scope::Personal]);

        // The genuine global note is still present and global.
        assert!(entries
            .iter()
            .any(|e| e.name == "global.md" && e.scope == Scope::Global));
    }
}
