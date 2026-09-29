//! Tauri command surface. Each command wraps a notez-core function and
//! returns either serializable data or a rendered error string.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Mutex;

use notez_core::config::{paths, Config, NotezMetadata, ProjectRegistry};
use notez_core::core::{aggregate, note, project, Note, Project, Scope};
use notez_core::todo::{self, Task};

use crate::dto::{NoteListItem, ProjectInfo, SearchHitDto, TodoBoard};

/// The live todoz board, held in Tauri-managed state so the indices the
/// frontend sends stay valid between calls.
pub type BoardState = Mutex<Vec<Task>>;

/// Render any error as a string for the IPC boundary.
fn err<E: std::fmt::Display>(e: E) -> String {
    format!("{e:#}")
}

/// Every note across all four scopes (global + each registered project's
/// local/public/personal trees). This is the headline aggregate view and
/// needs no current-directory context.
#[tauri::command]
pub fn list_notes() -> Result<Vec<NoteListItem>, String> {
    let config = Config::load().map_err(err)?;
    let registry = ProjectRegistry::load().map_err(err)?;
    let metadata_path = paths::metadata_file(&config.notez_root_path());
    let metadata = NotezMetadata::load_from(&metadata_path).map_err(err)?;

    let entries = aggregate::collect_all(&config, &registry, &metadata).map_err(err)?;

    // Decorate each note with its importance flags from the root `.tags`.
    let root = config.notez_root_path();
    let tags = notez_core::note_tags::load_tags(&root);
    let items = entries
        .iter()
        .map(|e| {
            let mut item = NoteListItem::from(e);
            item.flags = e
                .path
                .strip_prefix(&root)
                .ok()
                .and_then(|rel| tags.get(rel.to_string_lossy().as_ref()).copied())
                .unwrap_or(0);
            item
        })
        .collect();
    Ok(items)
}

/// Case-insensitive full-text search across every note and doc source.
#[tauri::command]
pub fn search_notes(query: String) -> Result<Vec<SearchHitDto>, String> {
    let config = Config::load().map_err(err)?;
    let registry = ProjectRegistry::load().map_err(err)?;
    let hits = notez_core::search::search_notes(&query, &config, &registry).map_err(err)?;
    Ok(hits.iter().map(SearchHitDto::from).collect())
}

/// Importance flags for one note (from its root `.tags`).
#[tauri::command]
pub fn get_note_tags(path: String) -> Result<u8, String> {
    let config = Config::load().map_err(err)?;
    Ok(notez_core::note_tags::get(
        &config.notez_root_path(),
        std::path::Path::new(&path),
    ))
}

/// Set importance flags for one note (zero clears it).
#[tauri::command]
pub fn set_note_tags(path: String, flags: u8) -> Result<(), String> {
    let config = Config::load().map_err(err)?;
    notez_core::note_tags::set(
        &config.notez_root_path(),
        std::path::Path::new(&path),
        flags,
    )
    .map_err(err)
}

/// Notes for a single scope, resolved against the current directory's
/// project (if any). Global resolves without a project; the project-relative
/// scopes fall back to empty when launched outside a project.
#[tauri::command]
pub fn list_notes_in_scope(scope: Scope) -> Result<Vec<NoteListItem>, String> {
    let config = Config::load().map_err(err)?;
    let cwd_project = Project::try_detect();
    let entries = aggregate::collect_in_scope(scope, &config, cwd_project.as_ref());
    Ok(entries.iter().map(NoteListItem::from).collect())
}

/// Read a markdown note's contents. Refuses anything that isn't a `.md` file;
/// deeper path-jailing to known notez roots lands in M6.
#[tauri::command]
pub fn read_note(path: String) -> Result<String, String> {
    let p = std::path::Path::new(&path);
    if p.extension().and_then(|s| s.to_str()) != Some("md") {
        return Err("refusing to read a non-markdown file".into());
    }
    std::fs::read_to_string(p).map_err(err)
}

/// Resolve the store root for an explicit scope x binding combination.
///
/// The app has no meaningful working directory (the Tauri process cwd is
/// wherever the app was launched), so project binding always comes from the
/// registry and global binding from the config root; the process cwd is
/// never consulted. This is the app-side counterpart of the CLI's
/// cwd-derived `resolve::root`.
fn resolve_store_root(
    scope: Scope,
    project: Option<&str>,
    config: &Config,
    registry: &ProjectRegistry,
) -> Result<PathBuf, String> {
    match (scope, project) {
        (Scope::Global, Some(_)) => Err("global notes are not bound to a project".into()),
        (Scope::Global, None) => Ok(config.notez_root_path()),
        // personal+global is the valid no-project combo of the two-axis
        // model; public/scratch are meaningless without a project here.
        (Scope::Personal, None) => Ok(config.notez_root_path()),
        (scope, None) => Err(format!("a project is required for {} notes", scope.label())),
        (scope, Some(name)) => {
            let local_path = registry
                .resolve(name)
                .ok_or_else(|| format!("unknown project: {name}"))?;
            match scope {
                Scope::Personal => {
                    // Personal notes live in the notez repo, not the project,
                    // so the project dir need not exist on this machine.
                    Ok(config.notez_root_path().join("personal").join(name))
                }
                Scope::Public | Scope::Local => {
                    if !local_path.exists() {
                        return Err(format!(
                            "project {name} is not reachable on this machine ({})",
                            local_path.display()
                        ));
                    }
                    Ok(match scope {
                        Scope::Public => local_path.join("notez"),
                        _ => local_path.join(".notez"),
                    })
                }
                Scope::Global => unreachable!("handled above"),
            }
        }
    }
}

/// Create a new note. Mirrors `notez add`: empty titles become "untitled",
/// the body is optional. Returns the new file's path.
///
/// Binding is explicit: `project` names a registered project (personal /
/// public / scratch stores), no project means global. `dir` targets a folder
/// under the notez root (sidebar tree selection, global only); without it
/// the note lands in the store's quick-notes dir.
#[tauri::command]
pub fn create_note(
    scope: Scope,
    title: String,
    body: Option<String>,
    dir: Option<String>,
    project: Option<String>,
) -> Result<String, String> {
    let config = Config::load().map_err(err)?;
    let registry = ProjectRegistry::load().map_err(err)?;
    let body = body.filter(|b| !b.trim().is_empty());
    let project = project.filter(|p| !p.trim().is_empty());
    let note = Note::new(title, body);

    let target = match dir.filter(|d| !d.trim().is_empty()) {
        // A folder picked in the sidebar tree. Global scope only, and jailed
        // to the notez root: the folder must already exist (it came from the
        // tree) and canonicalize inside the root, so a crafted relative path
        // cannot escape it.
        Some(rel) => {
            if scope != Scope::Global || project.is_some() {
                return Err("folder target is only valid for global notes".into());
            }
            let root = config.notez_root_path().canonicalize().map_err(err)?;
            let joined = root.join(&rel).canonicalize().map_err(err)?;
            if !joined.starts_with(&root) {
                return Err("folder is outside the notez root".into());
            }
            joined
        }
        None => {
            let store =
                resolve_store_root(scope, project.as_deref(), &config, &registry)?;
            let d = store.join(&config.paths.quick_notes_dir);
            std::fs::create_dir_all(&d).map_err(err)?;
            if scope == Scope::Local {
                project::ensure_scratch_gitignored(&d);
            }
            d
        }
    };

    let path = target.join(note.filename());
    std::fs::write(&path, note.rendered()).map_err(err)?;
    Ok(path.to_string_lossy().into_owned())
}

/// Absolute path of the global notez root, so the frontend can relativize
/// global note paths into the sidebar folder tree.
#[tauri::command]
pub async fn notez_root() -> Result<String, String> {
    let config = Config::load().map_err(err)?;
    Ok(config.notez_root_path().to_string_lossy().into_owned())
}

/// Overwrite a note's contents. Refuses non-`.md` paths; deeper path-jailing
/// lands in M6.
#[tauri::command]
pub fn save_note(path: String, content: String) -> Result<(), String> {
    let p = std::path::Path::new(&path);
    if p.extension().and_then(|s| s.to_str()) != Some("md") {
        return Err("refusing to write a non-markdown file".into());
    }
    std::fs::write(p, content).map_err(err)
}

/// Append a timestamped entry to today's daily log. Mirrors `notez log`.
/// Returns the log file's path. Binding is explicit like `create_note`:
/// `project` targets that project's store, no project means global.
#[tauri::command]
pub fn append_log(
    scope: Scope,
    message: String,
    project: Option<String>,
) -> Result<String, String> {
    let message = message.trim().to_string();
    if message.is_empty() {
        return Err("log message cannot be empty".into());
    }
    let config = Config::load().map_err(err)?;
    let registry = ProjectRegistry::load().map_err(err)?;
    let project = project.filter(|p| !p.trim().is_empty());

    let store = resolve_store_root(scope, project.as_deref(), &config, &registry)?;
    let dir = store.join(&config.paths.daily_logs_dir);
    std::fs::create_dir_all(&dir).map_err(err)?;
    if scope == Scope::Local {
        project::ensure_scratch_gitignored(&dir);
    }

    let path = dir.join(note::todays_log_filename());
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    let updated = note::append_log_entry(&existing, &message);
    std::fs::write(&path, updated).map_err(err)?;
    Ok(path.to_string_lossy().into_owned())
}

// --- projects & sync ---

/// Every project registered on this machine.
#[tauri::command]
pub fn list_projects() -> Result<Vec<ProjectInfo>, String> {
    let reg = ProjectRegistry::load().map_err(err)?;
    Ok(reg
        .iter_resolved()
        .map(|(name, path)| ProjectInfo {
            name: name.to_string(),
            local_path: notez_core::util::tilde::contract(&path),
            reachable: path.exists(),
        })
        .collect())
}

/// Register a project name → path on this machine.
#[tauri::command]
pub fn attach_project(name: String, path: String) -> Result<ProjectInfo, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("project name is required".into());
    }
    let abs = notez_core::util::tilde::expand(path.trim());
    let mut reg = ProjectRegistry::load().map_err(err)?;
    reg.attach(&name, &abs);
    reg.save_to(&paths::registry_file()).map_err(err)?;
    // Scaffold the public store so the project can host public notes right
    // away; failure must not undo the registration.
    let _ = notez_core::core::project::ensure_public_store(&abs);
    Ok(ProjectInfo {
        name,
        local_path: notez_core::util::tilde::contract(&abs),
        reachable: abs.exists(),
    })
}

/// Unregister a project (does not touch its notes).
#[tauri::command]
pub fn detach_project(name: String) -> Result<(), String> {
    let mut reg = ProjectRegistry::load().map_err(err)?;
    reg.detach(&name);
    reg.save_to(&paths::registry_file()).map_err(err)
}

/// Commit pending changes, then `git pull --rebase` and `git push` on the
/// notez root, returning git's combined output.
#[tauri::command]
pub fn sync() -> Result<String, String> {
    let config = Config::load().map_err(err)?;
    let root = config.notez_root_path();
    if !root.join(".git").exists() {
        return Err(format!(
            "{} is not a git repository — run `git init` and add a remote first",
            root.display()
        ));
    }
    let mut log = String::new();
    if notez_core::sync::commit_pending(&root).map_err(err)? {
        log.push_str("Committed local changes.\n");
    }
    for args in [["pull", "--rebase"].as_slice(), ["push"].as_slice()] {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(&root)
            .args(args)
            .output()
            .map_err(err)?;
        log.push_str(&String::from_utf8_lossy(&out.stdout));
        log.push_str(&String::from_utf8_lossy(&out.stderr));
        if !out.status.success() {
            return Err(format!("git {} failed:\n{}", args.join(" "), log.trim()));
        }
    }
    Ok(log.trim().to_string())
}

/// The current machine's hostname, used as a per-machine identity label in
/// the sidebar avatar. Shells out to `hostname` (reliable in a GUI launch
/// where `$HOSTNAME` is often unset) and falls back to a generic label.
#[tauri::command]
pub fn machine_name() -> String {
    std::process::Command::new("hostname")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "this machine".to_string())
}

// --- legacy migration ---

/// Preview the legacy → notez migration (changes nothing).
#[tauri::command]
pub fn migrate_preview() -> Result<Vec<notez_core::migrate::PlanItem>, String> {
    let config = Config::load().map_err(err)?;
    Ok(notez_core::migrate::plan(&config))
}

/// Apply the migration: move project dirs into personal/ and attach them.
#[tauri::command]
pub fn migrate_apply() -> Result<Vec<String>, String> {
    let config = Config::load().map_err(err)?;
    notez_core::migrate::apply(&config).map_err(err)
}

// --- todoz ---

fn lock(state: &BoardState) -> Result<std::sync::MutexGuard<'_, Vec<Task>>, String> {
    state.lock().map_err(|_| "todoz board lock poisoned".to_string())
}

/// Persist only the TODO.md the mutation touched and return the refreshed
/// board. `source: None` (header/no-op mutations) writes nothing, so files
/// the user never edited are never rewritten (rewrites are lossy for
/// non-todo text).
fn persisted(guard: &[Task], source: Option<PathBuf>) -> Result<TodoBoard, String> {
    let sources: HashSet<PathBuf> = source.into_iter().collect();
    todo::save_todos_for(guard, &sources).map_err(err)?;
    Ok(TodoBoard::from_tasks(guard))
}

/// The source file a mutation of `idx` will touch. Headers and code todos
/// never persist, so mutations that no-op on them save nothing.
fn task_source(guard: &[Task], idx: usize) -> Option<PathBuf> {
    guard
        .get(idx)
        .filter(|t| !t.is_header && !t.is_code_todo)
        .map(|t| t.source.clone())
}

/// Load the global todoz board (all scopes) into managed state.
///
/// Collapse is view-only and rebuilt fresh from disk on every load, so we carry
/// the user's expand/collapse choices across the reload — otherwise a live sync
/// (CLI edit → reload) would fold the whole tree back up under the user.
#[tauri::command]
pub fn load_todo_board(state: tauri::State<BoardState>) -> Result<TodoBoard, String> {
    let config = Config::load().map_err(err)?;
    let registry = ProjectRegistry::load().map_err(err)?;
    let mut board = todo::load_board(&config, &registry);
    let mut guard = lock(&state)?;
    preserve_collapsed(&guard, &mut board);
    *guard = board;
    Ok(TodoBoard::from_tasks(&guard))
}

/// Re-apply the previous collapse state to a freshly-loaded board, matching
/// headers/parents by a stable identity (source + section + depth + text).
fn preserve_collapsed(prev: &[Task], next: &mut [Task]) {
    type Key = (std::path::PathBuf, String, u8, String);
    let key = |t: &Task| -> Key { (t.source.clone(), t.section.clone(), t.depth, t.text.clone()) };
    let map: std::collections::HashMap<Key, bool> = prev
        .iter()
        .filter(|t| t.is_header || t.has_subtasks)
        .map(|t| (key(t), t.collapsed))
        .collect();
    if map.is_empty() {
        return;
    }
    for t in next.iter_mut() {
        if let Some(&collapsed) = map.get(&key(t)) {
            t.collapsed = collapsed;
        }
    }
}

/// Load the single-scope todoz board into managed state.
#[tauri::command]
pub fn load_scope_todo_board(
    scope: Scope,
    state: tauri::State<BoardState>,
) -> Result<TodoBoard, String> {
    let config = Config::load().map_err(err)?;
    let board = todo::load_scope_board(scope, &config).map_err(err)?;
    let mut guard = lock(&state)?;
    *guard = board;
    Ok(TodoBoard::from_tasks(&guard))
}

#[tauri::command]
pub fn toggle_task(idx: usize, state: tauri::State<BoardState>) -> Result<TodoBoard, String> {
    let mut guard = lock(&state)?;
    let source = task_source(&guard, idx);
    todo::toggle_done(&mut guard, idx);
    persisted(&guard, source)
}

#[tauri::command]
pub fn set_task_flags(
    idx: usize,
    flags: u8,
    state: tauri::State<BoardState>,
) -> Result<TodoBoard, String> {
    let mut guard = lock(&state)?;
    let source = task_source(&guard, idx);
    todo::set_flags(&mut guard, idx, flags);
    persisted(&guard, source)
}

#[tauri::command]
pub fn edit_task(
    idx: usize,
    text: String,
    state: tauri::State<BoardState>,
) -> Result<TodoBoard, String> {
    let mut guard = lock(&state)?;
    let source = task_source(&guard, idx);
    todo::edit_text(&mut guard, idx, text);
    persisted(&guard, source)
}

#[tauri::command]
pub fn add_todo(
    after: usize,
    depth: u8,
    text: String,
    state: tauri::State<BoardState>,
) -> Result<TodoBoard, String> {
    let mut guard = lock(&state)?;
    // The new task inherits `after`'s source (headers included: adding under
    // a section header writes that section's file).
    let source = guard.get(after).map(|t| t.source.clone());
    todo::add_task(&mut guard, after, depth, text);
    persisted(&guard, source)
}

#[tauri::command]
pub fn remove_todo(idx: usize, state: tauri::State<BoardState>) -> Result<TodoBoard, String> {
    let mut guard = lock(&state)?;
    let source = task_source(&guard, idx);
    todo::remove_task(&mut guard, idx);
    persisted(&guard, source)
}

#[tauri::command]
pub fn reorder_task(
    start: usize,
    target: usize,
    state: tauri::State<BoardState>,
) -> Result<TodoBoard, String> {
    let mut guard = lock(&state)?;
    if todo::can_drag(&guard, start, target) {
        let source = task_source(&guard, start);
        todo::perform_drag_move(&mut guard, start, target);
        todo::derive_parent_states(&mut guard);
        persisted(&guard, source)
    } else {
        // Invalid drag: return the board unchanged.
        Ok(TodoBoard::from_tasks(&guard))
    }
}

/// Toggle a header/parent's collapsed state. View-only, never persisted.
#[tauri::command]
pub fn toggle_collapse(idx: usize, state: tauri::State<BoardState>) -> Result<TodoBoard, String> {
    let mut guard = lock(&state)?;
    if let Some(t) = guard.get_mut(idx) {
        if t.is_header || t.has_subtasks {
            t.collapsed = !t.collapsed;
        }
    }
    Ok(TodoBoard::from_tasks(&guard))
}

/// Set a task's check state explicitly ("unchecked" | "half" | "checked").
#[tauri::command]
pub fn set_task_state(
    idx: usize,
    state: String,
    board: tauri::State<BoardState>,
) -> Result<TodoBoard, String> {
    let cs = match state.as_str() {
        "unchecked" => todo::CheckState::Unchecked,
        "half" => todo::CheckState::Half,
        "checked" => todo::CheckState::Checked,
        other => return Err(format!("invalid state: {other}")),
    };
    let mut guard = lock(&board)?;
    let source = task_source(&guard, idx);
    todo::set_state(&mut guard, idx, cs);
    persisted(&guard, source)
}

/// Move a task up or down among its siblings.
#[tauri::command]
pub fn move_todo(idx: usize, up: bool, state: tauri::State<BoardState>) -> Result<TodoBoard, String> {
    let mut guard = lock(&state)?;
    let source = task_source(&guard, idx);
    let new_idx = todo::move_task(&mut guard, idx, up);
    if new_idx == idx {
        // No sibling to swap with: nothing changed, write nothing.
        return Ok(TodoBoard::from_tasks(&guard));
    }
    persisted(&guard, source)
}

/// Collapse or expand every section/parent. View-only, never persisted.
#[tauri::command]
pub fn collapse_all(
    collapsed: bool,
    state: tauri::State<BoardState>,
) -> Result<TodoBoard, String> {
    let mut guard = lock(&state)?;
    todo::set_all_collapsed(&mut guard, collapsed);
    Ok(TodoBoard::from_tasks(&guard))
}

/// Create a new global todo category at `~/notez/_todos/<name>/TODO.md`, then
/// reload the board so the new section appears.
#[tauri::command]
pub fn create_category(
    name: String,
    state: tauri::State<BoardState>,
) -> Result<TodoBoard, String> {
    let slug = notez_core::util::sanitize::name(name.trim());
    if slug.is_empty() {
        return Err("category name cannot be empty".into());
    }
    let config = Config::load().map_err(err)?;
    let registry = ProjectRegistry::load().map_err(err)?;
    let dir = config.notez_root_path().join("_todos").join(&slug);
    std::fs::create_dir_all(&dir).map_err(err)?;
    let todo_file = dir.join("TODO.md");
    if !todo_file.exists() {
        std::fs::write(&todo_file, "# TODO\n\n").map_err(err)?;
    }
    let board = todo::load_board(&config, &registry);
    let mut guard = lock(&state)?;
    *guard = board;
    Ok(TodoBoard::from_tasks(&guard))
}
