//! `notez tree` / `treez`: interactive tree browser TUI.
//!
//! Assembles [`SectionSpec`]s out of the aggregator (registry + scopes +
//! project docs, no symlink walking) and hands them to `tui::tree`. On
//! exit, only `.tags` roots whose tag maps actually changed are written.

use std::path::{Path, PathBuf};

use anyhow::Result;

use notez_core::config::{Config, NotezMetadata, ProjectRegistry};
use notez_core::core::aggregate::{self, NoteEntry, SourceKind};
use notez_core::core::project::has_note_stores;
use notez_core::core::{Project, Scope};
use notez_core::note_tags;
use notez_core::util::tilde;

use crate::tui::header::SyncState;
use crate::tui::tree::{NewNoteRoots, SectionSpec, TreeContext, run_tree};

/// Nerdfont book icon for docs sections (scopes use `Scope::icon`).
const ICON_DOCS: &str = "\u{f02d}";

/// Which notes the browser opens on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    /// Everything: the current repository's sections first (when there is
    /// one), then the vault's global notes, then every other project,
    /// registered or only present as a `personal/<name>/` folder.
    All,
    /// One scope: the current project's personal notes or scratch, or the
    /// vault's global notes (the vault root minus `personal/`). Personal
    /// falls back to [`View::All`] outside a project.
    Only(Scope),
}

impl View {
    /// The view for a browser invocation. Without a scope flag that is
    /// [`View::All`], inside a project or not; a flag narrows it to the
    /// scope `scope` resolved to.
    pub fn for_flags(has_scope_flag: bool, scope: Scope, in_project: bool) -> Self {
        match (has_scope_flag, scope) {
            (false, _) => Self::All,
            (true, Scope::Personal) if !in_project => Self::All,
            (true, scope) => Self::Only(scope),
        }
    }
}

/// Open the tree browser on `view`. `warning`, if any, shows in the status
/// bar as the browser opens; `sync` is the header's sync state.
pub fn run(view: View, config: &Config, warning: Option<&str>, sync: SyncState) -> Result<()> {
    let registry = ProjectRegistry::load().unwrap_or_default();
    let (sections, mut ctx) = build_view(view, config, &registry)?;
    ctx.warning = warning.map(str::to_string);
    ctx.sync = sync;
    ctx.new_note_roots = new_note_roots(config, &registry, Project::try_detect().as_ref());

    // An empty view still opens: the browser shows an empty state and `n`
    // creates the first note.
    let rebuild = || build_view(view, config, &registry).map(|(sections, _)| sections);
    let changed = run_tree(sections, &ctx, config, &rebuild)?;
    for (root, map) in &changed {
        note_tags::save_tags(root, map)?;
    }
    Ok(())
}

/// Repository root of every registered project. The current project
/// (`current`, the one the browser runs in) need not be registered; its
/// stores still live in its own repository, not under the notez root.
fn repo_paths(
    registry: &ProjectRegistry,
    current: Option<&Project>,
) -> std::collections::BTreeMap<String, PathBuf> {
    let mut paths: std::collections::BTreeMap<String, PathBuf> = registry
        .iter_resolved()
        .map(|(n, p)| (n.to_string(), p))
        .collect();
    if let Some(current) = current {
        paths
            .entry(current.name.clone())
            .or_insert_with(|| current.root.clone());
    }
    paths
}

/// Names of the `personal/<name>/` folders in the vault: projects whose
/// personal notes exist here even when this machine has no repository for
/// them. Hidden names and anything that is not a directory are skipped, as
/// `aggregate::collect_all` skips them.
fn personal_folders(notez_root: &std::path::Path) -> Vec<String> {
    let Ok(read) = std::fs::read_dir(notez_root.join("personal")) else {
        return Vec::new();
    };
    read.filter_map(Result::ok)
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().to_str().map(str::to_string))
        .filter(|name| !name.starts_with('.'))
        .collect()
}

/// The scope roots the new-note prompt can target: the global store and,
/// per project with a known repository, its personal, public and scratch
/// stores (the paths `notez add` writes to inside that project). A project
/// known only by its `personal/<name>/` folder lists its personal store
/// alone, so nothing can be aimed at a repository that is not here.
fn new_note_roots(
    config: &Config,
    registry: &ProjectRegistry,
    current: Option<&Project>,
) -> NewNoteRoots {
    let notez_root = config.notez_root_path();
    let personal = |name: &str| (Scope::Personal, notez_root.join("personal").join(name));
    let mut projects: std::collections::HashMap<String, Vec<(Scope, PathBuf)>> =
        repo_paths(registry, current)
            .into_iter()
            .map(|(name, repo)| {
                let stores = vec![
                    personal(&name),
                    (Scope::Public, repo.join("notez")),
                    (Scope::Local, repo.join(".notez")),
                ];
                (name, stores)
            })
            .collect();
    for name in personal_folders(&notez_root) {
        let store = personal(&name);
        projects.entry(name).or_insert_with(|| vec![store]);
    }
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

/// Section bucket of the current repository: its sections come first.
const BUCKET_CURRENT: u8 = 0;
/// Section bucket of the vault's global notes: after the current repository.
const BUCKET_GLOBAL: u8 = 1;
/// Section bucket of every other project, alphabetically by name.
const BUCKET_OTHER: u8 = 2;

/// A project's own note stores, in section order.
const PROJECT_STORES: [Scope; 3] = [Scope::Personal, Scope::Public, Scope::Local];

/// Which stores get a section while they hold no notes.
#[derive(Debug, Clone, Copy)]
enum EmptyStores {
    /// None: a store shows only once it has a note.
    Hidden,
    /// This store of the current project (a narrowed view).
    Current(Scope),
    /// Personal, public and scratch of every project with a repository here,
    /// registered or current (the everything view).
    AllProjects,
}

/// Group aggregator entries into ordered sections: the current project's
/// sections first (when `current` is set), then NOTEZ (global), then each
/// other project alphabetically. Within a project the order is personal,
/// public, docs, local ([`scope_rank`]).
///
/// A project with no known repository (one only present as a
/// `personal/<name>/` folder) has a personal section only, rooted at that
/// folder; its tag root is the notez root like every personal section.
///
/// Stores named by `empty_stores` get a section even with no notes in them,
/// as long as their folder exists: an empty `notez/` or `.notez/` is still a
/// place to create notes and folders in.
fn sections_from_entries(
    entries: Vec<NoteEntry>,
    config: &Config,
    registry: &ProjectRegistry,
    current: Option<&Project>,
    empty_stores: EmptyStores,
) -> Vec<SectionSpec> {
    let notez_root = config.notez_root_path();
    let repo_paths = repo_paths(registry, current);
    let current_name = current.map(|p| p.name.as_str());

    let mut grouped: std::collections::BTreeMap<(u8, String, u8), Vec<PathBuf>> =
        std::collections::BTreeMap::new();
    let listed_empty: Vec<(&str, Scope)> = match empty_stores {
        EmptyStores::Hidden => Vec::new(),
        EmptyStores::Current(scope) => current_name.map(|n| (n, scope)).into_iter().collect(),
        EmptyStores::AllProjects => repo_paths
            .keys()
            .flat_map(|name| PROJECT_STORES.map(|scope| (name.as_str(), scope)))
            .collect(),
    };
    for (name, scope) in listed_empty {
        // `repo_paths` holds the current project too, registered or not.
        let Some(repo) = repo_paths.get(name) else {
            continue;
        };
        let (root, ..) = section_meta(scope, SourceKind::Note, name, repo, &notez_root);
        if root.is_dir() {
            let bucket = if Some(name) == current_name {
                BUCKET_CURRENT
            } else {
                BUCKET_OTHER
            };
            let rank = scope_rank(scope, SourceKind::Note);
            grouped.entry((bucket, name.to_string(), rank)).or_default();
        }
    }
    for entry in entries {
        let (bucket, project) = match &entry.project {
            None => (BUCKET_GLOBAL, String::new()),
            Some(p) if Some(p.as_str()) == current_name => (BUCKET_CURRENT, p.clone()),
            Some(p) => (BUCKET_OTHER, p.clone()),
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
        let is_global = bucket == BUCKET_GLOBAL;
        let (root, tag_root, label, icon) = if is_global {
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
        // The global walk leaves `personal/` to the projects, as the
        // aggregator does for the notes in it.
        let personal_root = notez_root.join("personal");
        let skip = is_global.then_some(personal_root.as_path());
        let dirs = section_dirs(&root, skip);
        out.push(SectionSpec {
            root,
            tag_root,
            label,
            icon,
            is_doc,
            files,
            dirs,
            scope: if is_global { Scope::Global } else { scope },
            project: (!is_global).then_some(project),
            new_note_root,
            is_current: bucket == BUCKET_CURRENT,
        });
    }
    out
}

/// Every directory under `root`, depth first, as absolute paths: the
/// section's folders, including ones with no notes in them. Mirrors the
/// aggregator's note walk: hidden names (starting with `.`) are skipped with
/// everything under them, symlinks are not followed, and an unreadable
/// directory just ends that branch. `skip`, if set, is left out with its
/// subtree. A missing `root` lists nothing.
fn section_dirs(root: &Path, skip: Option<&Path>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(read) = std::fs::read_dir(root) else {
        return out;
    };
    let mut children: Vec<PathBuf> = read
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
        .map(|e| e.path())
        .filter(|p| Some(p.as_path()) != skip)
        .collect();
    children.sort();
    for child in children {
        let below = section_dirs(&child, skip);
        out.push(child);
        out.extend(below);
    }
    out
}

fn build_view(
    view: View,
    config: &Config,
    registry: &ProjectRegistry,
) -> Result<(Vec<SectionSpec>, TreeContext)> {
    let project = Project::try_detect();
    match view {
        View::All => all_view(project.as_ref(), config, registry),
        View::Only(Scope::Personal) if project.is_none() => all_view(None, config, registry),
        View::Only(scope) => Ok(single_scope_view(scope, project.as_ref(), config, registry)),
    }
}

/// The everything view, with `current` (the project the browser runs in, if
/// any) first. A current project that is not registered is not in the
/// aggregate, so its stores are collected from its repository here.
fn all_view(
    current: Option<&Project>,
    config: &Config,
    registry: &ProjectRegistry,
) -> Result<(Vec<SectionSpec>, TreeContext)> {
    let mut entries = aggregate::collect_all(config, registry, &NotezMetadata::default())?;
    if let Some(project) = current {
        let attached = registry
            .iter_resolved()
            .any(|(name, _)| name == project.name);
        if !attached {
            for s in [Scope::Personal, Scope::Public, Scope::Local] {
                entries.extend(aggregate::collect_in_scope(s, config, Some(project)));
            }
            // The unregistered project's personal folder is in the aggregate
            // already, and a repository inside the vault overlaps its global
            // walk: keep the first occurrence of each path.
            let mut seen = std::collections::HashSet::new();
            entries.retain(|e| seen.insert(e.path.clone()));
        }
    }
    let sections =
        sections_from_entries(entries, config, registry, current, EmptyStores::AllProjects);
    let title = match current {
        Some(p) => format!("notez ({})", p.name),
        None => "notez".to_string(),
    };
    Ok((
        sections,
        TreeContext {
            title,
            path_display: tilde::contract(&config.notez_root_path()),
            warning: None,
            sync: SyncState::Off,
            current_project: current.map(|p| p.name.clone()),
            expand_current: current.is_some_and(|p| has_note_stores(&p.root)),
            new_note_roots: NewNoteRoots::default(),
        },
    ))
}

/// The view of one scope: a project's personal notes, public notes or
/// scratch, or the vault's global notes. Without a project only scratch,
/// public and global reach here; scratch and public are empty then.
fn single_scope_view(
    scope: Scope,
    project: Option<&Project>,
    config: &Config,
    registry: &ProjectRegistry,
) -> (Vec<SectionSpec>, TreeContext) {
    // Global notes belong to no project: collected without one, they group
    // into the NOTEZ section rather than under the current project.
    let owner = project.filter(|_| scope != Scope::Global);
    let entries = aggregate::collect_in_scope(scope, config, owner);
    let empty_stores = if scope == Scope::Global {
        EmptyStores::Hidden
    } else {
        EmptyStores::Current(scope)
    };
    let sections = sections_from_entries(entries, config, registry, project, empty_stores);
    let name = match (scope, project) {
        (Scope::Global, _) | (_, None) => scope.to_string(),
        (_, Some(p)) => p.name.clone(),
    };
    let path_display = match (scope, project) {
        (Scope::Global, _) => tilde::contract(&config.notez_root_path()),
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
            sync: SyncState::Off,
            current_project: project.map(|p| p.name.clone()),
            expand_current: project.is_some_and(|p| has_note_stores(&p.root)),
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
        let sections =
            sections_from_entries(entries, &config, &registry, None, EmptyStores::Hidden);
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
    fn no_flag_picks_the_all_view_inside_and_outside_a_project() {
        assert_eq!(View::for_flags(false, Scope::Public, true), View::All);
        assert_eq!(View::for_flags(false, Scope::Global, false), View::All);
    }

    #[test]
    fn a_scope_flag_narrows_the_view_to_that_scope() {
        assert_eq!(
            View::for_flags(true, Scope::Global, true),
            View::Only(Scope::Global)
        );
        assert_eq!(
            View::for_flags(true, Scope::Global, false),
            View::Only(Scope::Global)
        );
        // Personal notes need a project; outside one `-p` shows everything.
        assert_eq!(View::for_flags(true, Scope::Personal, false), View::All);
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
    fn all_view_lists_the_current_projects_empty_public_store() {
        let (labels, title, name) = view_in_project(View::All);
        assert_eq!(
            labels,
            vec![
                (format!("{name} (personal)"), 1),
                (format!("{name} (public)"), 0),
                (format!("{name} (scratch)"), 1),
                ("NOTEZ".to_string(), 1),
            ]
        );
        assert_eq!(title, format!("notez ({name})"));
    }

    /// Whether `view` opens the current repository's sections expanded, with
    /// the cwd in a temp git repository holding `stores` (and a personal
    /// note, so the repository has a section either way).
    fn expands_current(view: View, stores: &[&str]) -> bool {
        let root = tempfile::tempdir().unwrap();
        let repo = tempfile::tempdir().unwrap();
        std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(repo.path())
            .status()
            .unwrap();
        for store in stores {
            std::fs::create_dir_all(repo.path().join(store)).unwrap();
        }
        let name = Project::try_detect_from(repo.path()).unwrap().name;
        let personal = root.path().join("personal").join(&name);
        std::fs::create_dir_all(&personal).unwrap();
        std::fs::write(personal.join("mine.md"), "# mine\n").unwrap();

        let mut config = Config::defaults();
        config.paths.notez_root = root.path().to_string_lossy().into_owned();
        let saved = std::env::current_dir().unwrap();
        std::env::set_current_dir(repo.path()).unwrap();
        let built = build_view(view, &config, &ProjectRegistry::default());
        std::env::set_current_dir(saved).unwrap();
        built.unwrap().1.expand_current
    }

    #[test]
    #[serial_test::serial]
    fn a_repo_with_notes_folders_opens_its_sections_expanded() {
        assert!(expands_current(View::All, &["notez"]));
        assert!(expands_current(View::All, &[".notez"]));
        assert!(expands_current(View::Only(Scope::Personal), &["notez"]));
    }

    #[test]
    #[serial_test::serial]
    fn a_repo_without_notes_folders_opens_everything_collapsed() {
        assert!(!expands_current(View::All, &[]));
        assert!(!expands_current(View::Only(Scope::Personal), &[]));
    }

    #[test]
    #[serial_test::serial]
    fn public_view_lists_the_empty_public_store() {
        let (labels, _, name) = view_in_project(View::Only(Scope::Public));
        assert_eq!(labels, vec![(format!("{name} (public)"), 0)]);
    }

    /// A current project whose `notez/` and `.notez/` hold folders but no
    /// notes, with no personal folder in the vault.
    fn empty_project() -> (tempfile::TempDir, Config, Project) {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        std::fs::create_dir_all(repo.join("notez")).unwrap();
        std::fs::create_dir_all(repo.join(".notez/drafts")).unwrap();
        let mut config = Config::defaults();
        config.paths.notez_root = dir.path().join("vault").to_string_lossy().into_owned();
        let project = Project {
            name: "proj".to_string(),
            root: repo,
        };
        (dir, config, project)
    }

    #[test]
    fn empty_current_stores_get_sections_when_their_folder_exists() {
        let (_dir, config, project) = empty_project();
        let sections = sections_from_entries(
            Vec::new(),
            &config,
            &ProjectRegistry::default(),
            Some(&project),
            EmptyStores::AllProjects,
        );
        let listed: Vec<(&str, usize, bool)> = sections
            .iter()
            .map(|s| (s.label.as_str(), s.dirs.len(), s.is_current))
            .collect();
        // No personal section: `personal/proj/` does not exist.
        assert_eq!(
            listed,
            vec![("proj (public)", 0, true), ("proj (scratch)", 1, true)]
        );
    }

    #[test]
    fn a_narrowed_view_lists_only_its_own_empty_store() {
        let (_dir, config, project) = empty_project();
        let sections = sections_from_entries(
            Vec::new(),
            &config,
            &ProjectRegistry::default(),
            Some(&project),
            EmptyStores::Current(Scope::Local),
        );
        let labels: Vec<&str> = sections.iter().map(|s| s.label.as_str()).collect();
        assert_eq!(labels, vec!["proj (scratch)"]);
    }

    #[test]
    fn other_registered_projects_list_their_empty_stores_too() {
        let (dir, config, project) = empty_project();
        let other = dir.path().join("other");
        std::fs::create_dir_all(other.join("notez")).unwrap();
        let missing = dir.path().join("not-on-this-machine");
        let mut registry = ProjectRegistry::default();
        registry.attach("other", &other);
        registry.attach("gone", &missing);
        let sections = sections_from_entries(
            Vec::new(),
            &config,
            &registry,
            Some(&project),
            EmptyStores::AllProjects,
        );
        let listed: Vec<(&str, bool)> = sections
            .iter()
            .map(|s| (s.label.as_str(), s.is_current))
            .collect();
        assert_eq!(
            listed,
            vec![
                ("proj (public)", true),
                ("proj (scratch)", true),
                ("other (public)", false),
            ]
        );
    }

    #[test]
    fn hidden_lists_no_empty_store() {
        let (_dir, config, project) = empty_project();
        let sections = sections_from_entries(
            Vec::new(),
            &config,
            &ProjectRegistry::default(),
            Some(&project),
            EmptyStores::Hidden,
        );
        assert!(sections.is_empty());
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
    fn global_view_shows_only_the_vault_global_notes_inside_a_project() {
        let (labels, title, _) = view_in_project(View::Only(Scope::Global));
        assert_eq!(title, format!("{} notez (global)", Scope::Global.icon()));
        // The project's personal note under personal/ is not global.
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
        let sections =
            sections_from_entries(entries, &config, &registry, None, EmptyStores::Hidden);
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
        let built = build_view(View::All, &config, &ProjectRegistry::default());
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

    /// Entries for the current project `beta`, two other projects and the
    /// vault's global notes, in no particular order.
    fn mixed_entries() -> Vec<NoteEntry> {
        let note = |path, scope, project| entry(path, scope, project, SourceKind::Note);
        vec![
            note("/nr/personal/zeta/z.md", Scope::Personal, Some("zeta")),
            note("/repo-b/notez/b.md", Scope::Public, Some("beta")),
            note("/nr/top.md", Scope::Global, None),
            note("/repo-a/notez/a.md", Scope::Public, Some("alpha")),
            note("/nr/personal/beta/p.md", Scope::Personal, Some("beta")),
            note("/repo-b/.notez/s.md", Scope::Local, Some("beta")),
        ]
    }

    fn nr_config() -> Config {
        let mut config = Config::defaults();
        config.paths.notez_root = "/nr".to_string();
        config
    }

    #[test]
    fn current_project_sections_come_first_then_global_then_others() {
        let current = Project {
            name: "beta".to_string(),
            root: PathBuf::from("/repo-b"),
        };
        let sections = sections_from_entries(
            mixed_entries(),
            &nr_config(),
            &ProjectRegistry::default(),
            Some(&current),
            EmptyStores::Hidden,
        );
        let labels: Vec<&str> = sections.iter().map(|s| s.label.as_str()).collect();
        assert_eq!(
            labels,
            vec![
                "beta (personal)",
                "beta (public)",
                "beta (scratch)",
                "NOTEZ",
                "alpha (public)",
                "zeta (personal)",
            ]
        );
        let current_flags: Vec<bool> = sections.iter().map(|s| s.is_current).collect();
        assert_eq!(current_flags, vec![true, true, true, false, false, false]);
    }

    #[test]
    fn outside_a_project_global_comes_first_and_nothing_is_current() {
        let sections = sections_from_entries(
            mixed_entries(),
            &nr_config(),
            &ProjectRegistry::default(),
            None,
            EmptyStores::Hidden,
        );
        let labels: Vec<&str> = sections.iter().map(|s| s.label.as_str()).collect();
        assert_eq!(
            labels,
            vec![
                "NOTEZ",
                "alpha (public)",
                "beta (personal)",
                "beta (public)",
                "beta (scratch)",
                "zeta (personal)",
            ]
        );
        assert!(sections.iter().all(|s| !s.is_current));
    }

    /// A project only present as a `personal/<name>/` folder: its one
    /// section is rooted at that folder and new notes land there. The tag
    /// root stays the notez root, where every personal section keeps its
    /// tags.
    #[test]
    fn unregistered_personal_only_project_is_rooted_at_its_folder() {
        let entries = vec![entry(
            "/nr/personal/socials/post.md",
            Scope::Personal,
            Some("socials"),
            SourceKind::Note,
        )];
        let sections = sections_from_entries(
            entries,
            &nr_config(),
            &ProjectRegistry::default(),
            None,
            EmptyStores::Hidden,
        );
        assert_eq!(sections.len(), 1);
        let s = &sections[0];
        assert_eq!(s.label, "socials (personal)");
        assert_eq!(s.root, PathBuf::from("/nr/personal/socials"));
        assert_eq!(s.new_note_root, PathBuf::from("/nr/personal/socials"));
        assert_eq!(s.tag_root, PathBuf::from("/nr"));
        assert_eq!(s.project.as_deref(), Some("socials"));
    }

    #[test]
    fn new_note_roots_list_a_personal_only_project_with_its_personal_store_alone() {
        let vault = tempfile::tempdir().unwrap();
        let repo = tempfile::tempdir().unwrap();
        let personal = vault.path().join("personal");
        for name in ["socials", "alpha", ".hidden"] {
            std::fs::create_dir_all(personal.join(name)).unwrap();
        }
        std::fs::write(personal.join("loose.md"), "# x\n").unwrap();
        let mut config = Config::defaults();
        config.paths.notez_root = vault.path().to_string_lossy().into_owned();
        let mut registry = ProjectRegistry::default();
        registry.attach("alpha", repo.path());
        let current = Project {
            name: "cur".to_string(),
            root: PathBuf::from("/repo-cur"),
        };

        let roots = new_note_roots(&config, &registry, Some(&current));

        let mut names: Vec<&str> = roots.projects.keys().map(String::as_str).collect();
        names.sort();
        assert_eq!(names, vec!["alpha", "cur", "socials"]);
        assert_eq!(
            roots.projects["socials"],
            vec![(Scope::Personal, personal.join("socials"))]
        );
        let scopes =
            |name: &str| -> Vec<Scope> { roots.projects[name].iter().map(|(s, _)| *s).collect() };
        let all = vec![Scope::Personal, Scope::Public, Scope::Local];
        assert_eq!(scopes("alpha"), all);
        assert_eq!(scopes("cur"), all);
        assert_eq!(
            roots.projects["cur"][1],
            (Scope::Public, PathBuf::from("/repo-cur/notez"))
        );
    }

    /// A temp vault with a global note, a registered project `alpha` with a
    /// public note, an unregistered `personal/socials/` folder and a
    /// current-project personal note; `build_view(view)` with the cwd in
    /// `cwd_in_repo` (a fresh git repository) or in a plain directory.
    /// Returns section labels, their `is_current` flags, the title and the
    /// current project name, if any.
    fn view_of_vault(
        view: View,
        cwd_in_repo: bool,
    ) -> (Vec<String>, Vec<bool>, String, Option<String>) {
        let vault = tempfile::tempdir().unwrap();
        let alpha = tempfile::tempdir().unwrap();
        let cwd = tempfile::tempdir().unwrap();
        if cwd_in_repo {
            std::process::Command::new("git")
                .args(["init", "-q"])
                .current_dir(cwd.path())
                .status()
                .unwrap();
        }
        let current = Project::try_detect_from(cwd.path()).map(|p| p.name);
        let write = |path: PathBuf| {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, "# x\n").unwrap();
        };
        write(vault.path().join("top.md"));
        let personal = vault.path().join("personal");
        write(personal.join("socials").join("post.md"));
        write(alpha.path().join("notez").join("a.md"));
        if let Some(name) = &current {
            write(personal.join(name).join("mine.md"));
        }
        let mut config = Config::defaults();
        config.paths.notez_root = vault.path().to_string_lossy().into_owned();
        let mut registry = ProjectRegistry::default();
        registry.attach("alpha", alpha.path());

        let saved = std::env::current_dir().unwrap();
        std::env::set_current_dir(cwd.path()).unwrap();
        let built = build_view(view, &config, &registry);
        std::env::set_current_dir(saved).unwrap();

        let (sections, ctx) = built.unwrap();
        assert_eq!(ctx.current_project, current);
        let labels = sections.iter().map(|s| s.label.clone()).collect();
        let flags = sections.iter().map(|s| s.is_current).collect();
        (labels, flags, ctx.title, current)
    }

    #[test]
    #[serial_test::serial]
    fn all_view_inside_a_project_lists_it_first_then_global_then_every_other_project() {
        let (labels, flags, title, current) = view_of_vault(View::All, true);
        let name = current.unwrap();
        assert_eq!(
            labels,
            vec![
                format!("{name} (personal)"),
                "NOTEZ".to_string(),
                "alpha (public)".to_string(),
                "socials (personal)".to_string(),
            ]
        );
        assert_eq!(flags, vec![true, false, false, false]);
        assert_eq!(title, format!("notez ({name})"));
    }

    /// [`view_of_vault`]'s sections outside a project.
    const OUTSIDE_LABELS: [&str; 3] = ["NOTEZ", "alpha (public)", "socials (personal)"];

    #[test]
    #[serial_test::serial]
    fn all_view_outside_a_project_lists_global_then_every_project() {
        let (labels, flags, title, current) = view_of_vault(View::All, false);
        assert_eq!(current, None);
        assert_eq!(labels, OUTSIDE_LABELS);
        assert!(flags.iter().all(|f| !f));
        assert_eq!(title, "notez");
    }

    #[test]
    #[serial_test::serial]
    fn personal_view_outside_a_project_falls_back_to_the_all_view() {
        let (labels, _, title, _) = view_of_vault(View::Only(Scope::Personal), false);
        assert_eq!(labels, OUTSIDE_LABELS);
        assert_eq!(title, "notez");
    }

    /// A vault whose global store holds a note, an empty folder, a nested
    /// empty folder, a hidden folder and a symlinked folder, next to a
    /// project's personal folder with an empty subfolder of its own.
    #[test]
    fn sections_list_empty_folders_but_not_hidden_or_linked_ones() {
        let vault = tempfile::tempdir().unwrap();
        let root = vault.path();
        for dir in ["empty", "outer/inner", ".hidden/sub", "personal/proj/drafts"] {
            std::fs::create_dir_all(root.join(dir)).unwrap();
        }
        std::fs::write(root.join("top.md"), "# x\n").unwrap();
        std::fs::write(root.join("personal/proj/n.md"), "# x\n").unwrap();
        std::os::unix::fs::symlink(root.join("outer"), root.join("linked")).unwrap();
        let mut config = Config::defaults();
        config.paths.notez_root = root.to_string_lossy().into_owned();
        let note = |path: PathBuf, scope, project: Option<&str>| NoteEntry {
            name: path.file_name().unwrap().to_string_lossy().into_owned(),
            path,
            scope,
            project: project.map(str::to_string),
            kind: SourceKind::Note,
        };
        let entries = vec![
            note(root.join("top.md"), Scope::Global, None),
            note(root.join("personal/proj/n.md"), Scope::Personal, Some("proj")),
        ];

        let sections =
            sections_from_entries(entries, &config, &ProjectRegistry::default(), None, EmptyStores::Hidden);

        let global = sections.iter().find(|s| s.scope == Scope::Global).unwrap();
        let expected: Vec<PathBuf> =
            ["empty", "outer", "outer/inner"].iter().map(|d| root.join(d)).collect();
        assert_eq!(global.dirs, expected, "no hidden, linked or personal folders");
        let personal = sections.iter().find(|s| s.scope == Scope::Personal).unwrap();
        assert_eq!(personal.dirs, vec![root.join("personal/proj/drafts")]);
    }

    #[test]
    fn a_missing_section_root_lists_no_folders() {
        assert!(section_dirs(Path::new("/no/such/notez/root"), None).is_empty());
    }

    #[test]
    #[serial_test::serial]
    fn global_view_excludes_every_personal_folder() {
        for in_repo in [true, false] {
            let (labels, flags, _, _) = view_of_vault(View::Only(Scope::Global), in_repo);
            assert_eq!(labels, vec!["NOTEZ"], "in_repo={in_repo}");
            assert_eq!(flags, vec![false], "in_repo={in_repo}");
        }
    }
}
