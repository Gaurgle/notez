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

use crate::tui::tree::{NewNoteRoots, SectionSpec, TreeContext, run_tree};

/// Nerdfont book icon for docs sections (scopes use `Scope::icon`).
const ICON_DOCS: &str = "\u{f02d}";

/// Which notes the browser opens on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    /// Every scope of the current project: personal, public, docs, scratch.
    /// Falls back to [`View::Global`] outside a project.
    Project,
    /// The notez root plus every registered project.
    Global,
    /// One scope of the current project. Personal falls back to
    /// [`View::Global`] outside a project, as the personal scope itself does.
    Only(Scope),
}

impl View {
    /// The view for a browser invocation. Without a scope flag that is the
    /// project view inside a project and the global view outside one;
    /// a flag narrows it to the scope `scope` resolved to.
    pub fn for_flags(has_scope_flag: bool, scope: Scope, in_project: bool) -> Self {
        match (has_scope_flag, scope) {
            (false, _) if in_project => Self::Project,
            (false, _) | (true, Scope::Global) => Self::Global,
            (true, scope) => Self::Only(scope),
        }
    }
}

/// Open the tree browser on `view`. `warning`, if any, shows in the status
/// bar as the browser opens.
pub fn run(view: View, config: &Config, warning: Option<&str>) -> Result<()> {
    let registry = ProjectRegistry::load().unwrap_or_default();
    let (sections, mut ctx) = build_view(view, config, &registry)?;
    ctx.warning = warning.map(str::to_string);
    ctx.new_note_roots = new_note_roots(config, &registry);

    // An empty view still opens: the browser shows an empty state and `n`
    // creates the first note.
    let rebuild = || build_view(view, config, &registry).map(|(sections, _)| sections);
    let changed = run_tree(sections, &ctx, config, &rebuild)?;
    for (root, map) in &changed {
        note_tags::save_tags(root, map)?;
    }
    Ok(())
}

/// Repository root of every registered project. The project the browser
/// runs in need not be registered; its stores still live in its own
/// repository, not under the notez root.
fn repo_paths(registry: &ProjectRegistry) -> std::collections::BTreeMap<String, PathBuf> {
    let mut paths: std::collections::BTreeMap<String, PathBuf> = registry
        .iter_resolved()
        .map(|(n, p)| (n.to_string(), p))
        .collect();
    if let Some(current) = Project::try_detect() {
        paths.entry(current.name).or_insert(current.root);
    }
    paths
}

/// The scope roots the new-note prompt can target: the global store and,
/// per project with a known repository, its personal, public and scratch
/// stores (the paths `notez add` writes to inside that project).
fn new_note_roots(config: &Config, registry: &ProjectRegistry) -> NewNoteRoots {
    let notez_root = config.notez_root_path();
    let projects = repo_paths(registry)
        .into_iter()
        .map(|(name, repo)| {
            let stores = vec![
                (Scope::Personal, notez_root.join("personal").join(&name)),
                (Scope::Public, repo.join("notez")),
                (Scope::Local, repo.join(".notez")),
            ];
            (name, stores)
        })
        .collect();
    NewNoteRoots { global: notez_root, projects }
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
    let repo_paths = repo_paths(registry);

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
        let is_doc = kind == SourceKind::Doc;
        let new_note_root = if is_doc {
            notez_root.join("personal").join(&project)
        } else {
            root.clone()
        };
        out.push(SectionSpec {
            root,
            tag_root,
            label,
            icon,
            is_doc,
            files,
            scope: if bucket == 0 { Scope::Global } else { scope },
            project: (bucket != 0).then_some(project),
            new_note_root,
        });
    }
    out
}

fn build_view(
    view: View,
    config: &Config,
    registry: &ProjectRegistry,
) -> Result<(Vec<SectionSpec>, TreeContext)> {
    let notez_root = config.notez_root_path();
    let metadata = NotezMetadata::default();

    match view {
        View::Global => {
            let entries = aggregate::collect_all(config, registry, &metadata)?;
            let sections = sections_from_entries(entries, config, registry);
            Ok((
                sections,
                TreeContext {
                    title: "notez (global)".to_string(),
                    path_display: tilde::contract(&notez_root),
                    warning: None,
                    current_project: Project::try_detect().map(|p| p.name),
                    new_note_roots: NewNoteRoots::default(),
                },
            ))
        }
        View::Project => {
            let Some(project) = Project::try_detect() else {
                return build_view(View::Global, config, registry);
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
                    current_project: Some(project.name.clone()),
                    new_note_roots: NewNoteRoots::default(),
                },
            ))
        }
        View::Only(Scope::Global) => build_view(View::Global, config, registry),
        View::Only(scope) => {
            let project = Project::try_detect();
            if project.is_none() && scope == Scope::Personal {
                return build_view(View::Global, config, registry);
            }
            Ok(single_scope_view(scope, project.as_ref(), config, registry))
        }
    }
}

/// The view of one project scope. Without a project only scratch and public
/// reach here, and both are empty then, as before this view existed.
fn single_scope_view(
    scope: Scope,
    project: Option<&Project>,
    config: &Config,
    registry: &ProjectRegistry,
) -> (Vec<SectionSpec>, TreeContext) {
    let entries = aggregate::collect_in_scope(scope, config, project);
    let sections = sections_from_entries(entries, config, registry);
    let name = project
        .map(|p| p.name.clone())
        .unwrap_or_else(|| scope.to_string());
    let path_display = match (scope, project) {
        (Scope::Personal, Some(p)) => tilde::contract(
            &config.notez_root_path().join("personal").join(&p.name),
        ),
        (Scope::Public, _) => "./notez".to_string(),
        _ => "./.notez".to_string(),
    };
    (
        sections,
        TreeContext {
            title: format!("{} notez ({})", scope.icon(), name),
            path_display,
            warning: None,
            current_project: project.map(|p| p.name.clone()),
            new_note_roots: NewNoteRoots::default(),
        },
    )
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

    /// Builds `view` with the cwd in a temp git project that has one personal
    /// note, one scratch note and an empty `notez/`, and a temp notez root
    /// holding one global note. Returns the section labels with their file
    /// counts, the view title and the project name.
    fn view_in_project(view: View) -> (Vec<(String, usize)>, String, String) {
        let root = tempfile::tempdir().unwrap();
        let repo = tempfile::tempdir().unwrap();
        std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(repo.path())
            .status()
            .unwrap();
        let name = Project::try_detect_from(repo.path()).unwrap().name;
        let personal = root.path().join("personal").join(&name);
        std::fs::create_dir_all(&personal).unwrap();
        std::fs::write(personal.join("mine.md"), "# mine\n").unwrap();
        std::fs::write(root.path().join("top.md"), "# top\n").unwrap();
        std::fs::create_dir_all(repo.path().join("notez")).unwrap();
        std::fs::create_dir_all(repo.path().join(".notez")).unwrap();
        std::fs::write(repo.path().join(".notez").join("scratch.md"), "# s\n").unwrap();

        let mut config = Config::defaults();
        config.paths.notez_root = root.path().to_string_lossy().into_owned();
        let saved = std::env::current_dir().unwrap();
        std::env::set_current_dir(repo.path()).unwrap();
        let built = build_view(view, &config, &ProjectRegistry::default());
        std::env::set_current_dir(saved).unwrap();

        let (sections, ctx) = built.unwrap();
        let labels = sections
            .iter()
            .map(|s| (s.label.clone(), s.files.len()))
            .collect();
        (labels, ctx.title, name)
    }

    #[test]
    fn no_flag_picks_the_project_view_inside_a_project_and_global_outside() {
        assert_eq!(View::for_flags(false, Scope::Public, true), View::Project);
        assert_eq!(View::for_flags(false, Scope::Global, false), View::Global);
    }

    #[test]
    fn a_scope_flag_narrows_the_view_to_that_scope() {
        assert_eq!(View::for_flags(true, Scope::Global, true), View::Global);
        assert_eq!(View::for_flags(true, Scope::Global, false), View::Global);
        assert_eq!(
            View::for_flags(true, Scope::Personal, true),
            View::Only(Scope::Personal)
        );
        assert_eq!(
            View::for_flags(true, Scope::Local, true),
            View::Only(Scope::Local)
        );
    }

    #[test]
    #[serial_test::serial]
    fn project_view_shows_personal_notes_when_the_public_store_is_empty() {
        let (labels, _, name) = view_in_project(View::Project);
        assert_eq!(
            labels,
            vec![
                (format!("{name} (personal)"), 1),
                (format!("{name} (scratch)"), 1),
            ]
        );
    }

    #[test]
    #[serial_test::serial]
    fn personal_view_shows_only_the_project_personal_notes() {
        let (labels, _, name) = view_in_project(View::Only(Scope::Personal));
        assert_eq!(labels, vec![(format!("{name} (personal)"), 1)]);
    }

    #[test]
    #[serial_test::serial]
    fn scratch_view_shows_only_the_project_scratch_notes() {
        let (labels, _, name) = view_in_project(View::Only(Scope::Local));
        assert_eq!(labels, vec![(format!("{name} (scratch)"), 1)]);
    }

    #[test]
    #[serial_test::serial]
    fn global_view_is_unchanged_inside_a_project() {
        let (labels, title, _) = view_in_project(View::Global);
        assert_eq!(title, "notez (global)");
        assert_eq!(labels, vec![("NOTEZ".to_string(), 1)]);
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

    /// An unregistered project's public, scratch and docs sections live in
    /// the project, not under the notez root: a new note in the public
    /// section must land in `<project>/notez/` as `notez add` puts it.
    #[test]
    #[serial_test::serial]
    fn unregistered_project_sections_are_rooted_in_the_project() {
        let root = tempfile::tempdir().unwrap();
        let repo = tempfile::tempdir().unwrap();
        std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(repo.path())
            .status()
            .unwrap();
        let name = Project::try_detect_from(repo.path()).unwrap().name;
        for (dir, file) in [("notez/plans", "p.md"), (".notez", "s.md")] {
            std::fs::create_dir_all(repo.path().join(dir)).unwrap();
            std::fs::write(repo.path().join(dir).join(file), "# x\n").unwrap();
        }
        let mut config = Config::defaults();
        config.paths.notez_root = root.path().to_string_lossy().into_owned();

        let saved = std::env::current_dir().unwrap();
        std::env::set_current_dir(repo.path()).unwrap();
        let built = build_view(View::Project, &config, &ProjectRegistry::default());
        std::env::set_current_dir(saved).unwrap();

        let (sections, ctx) = built.unwrap();
        assert_eq!(ctx.current_project.as_deref(), Some(name.as_str()));
        let repo_root = repo.path().canonicalize().unwrap();
        for (scope, dir) in [(Scope::Public, "notez"), (Scope::Local, ".notez")] {
            let s = sections.iter().find(|s| s.scope == scope).unwrap();
            assert_eq!(s.root.canonicalize().unwrap(), repo_root.join(dir), "{scope:?}");
            assert_eq!(s.new_note_root, s.root, "{scope:?}");
            assert_eq!(s.project.as_deref(), Some(name.as_str()));
            assert_eq!(s.files.len(), 1, "{scope:?}");
        }
    }
}
