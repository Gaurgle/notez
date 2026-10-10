//! The move (`m`) and set-scope (`S`) prompts: resolving, confirming
//! and applying a move, and carrying its tags.

use super::*;

// --- Move ---

/// The footer message for `m` on a section row.
const SECTION_MOVE: &str = "move: a section cannot be moved";

/// The footer message for `m` on a row in a docs section: those are the
/// repository's own files. Docs sections are never a destination either.
pub(super) const DOCS_MOVE: &str = "move: not for a docs section";

/// The footer message for `m` on a folder that holds another section.
const FOLDER_HOLDS_SECTION_MOVE: &str = "move: this folder holds another section";

/// The footer message for `m` with no row under the cursor.
const NOTHING_TO_MOVE: &str = "move: no note under the cursor";

/// The footer message for `S` on a section row.
const SECTION_SET_SCOPE: &str = "set scope: a section has no scope to change";

/// The footer message for `S` on a row in a docs section.
pub(super) const DOCS_SET_SCOPE: &str = "set scope: not for a docs section";

/// The footer message for `S` on a folder that holds another section.
const FOLDER_HOLDS_SECTION_SET_SCOPE: &str = "set scope: this folder holds another section";

/// The footer message for `S` with no row under the cursor.
const NOTHING_TO_SET_SCOPE: &str = "set scope: no note under the cursor";

/// The open move prompt for one note or folder. `scope` is the new-note
/// prompt's machinery: its target is the root of the destination scope,
/// which `Tab` cycles as it does for `n`, and its buffer is the destination
/// folder relative to that root, prefilled with the row's current folder.
/// `src` is the note or folder, `rel` its path relative to its section root
/// and `from` its scope. `notes` is `Some` for a folder: the notes it
/// holds, counted when the prompt opens, for the question. `fixed` marks
/// the prompt `S` opens: the buffer stays the row's current folder (typing
/// does nothing) and only the scope changes.
pub(super) struct MovePrompt {
    pub(super) scope: NewNotePrompt,
    pub(super) src: PathBuf,
    pub(super) rel: String,
    pub(super) from: Scope,
    pub(super) notes: Option<usize>,
    pub(super) fixed: bool,
}

impl MovePrompt {
    /// The word footer messages about this prompt start with.
    pub(super) fn verb(&self) -> &'static str {
        if self.fixed {
            "set scope"
        } else {
            "move"
        }
    }
}

/// What `m` does with the cursor on `row`: `Ok` opens the move prompt on a
/// note or a folder, `Err` is the footer message for a section row, a row
/// in a docs section or in the todo board's store ([`is_todo_row`]), a
/// folder holding another section, or no row at all.
/// The scopes `Tab` offers are the row's project's plus global; for a
/// global row, the project the browser was opened in, if any.
pub(super) fn move_request(
    nodes: &[TreeNode],
    sections: &[SectionSpec],
    row: Option<usize>,
    ctx: &TreeContext,
) -> std::result::Result<MovePrompt, &'static str> {
    open_move(nodes, sections, row, ctx, false)
}

/// What `S` does with the cursor on `row`: the prompt `m` opens, with the
/// folder fixed to the row's current one (see [`MovePrompt`]), refused for
/// the same rows with `set scope:` messages.
pub(super) fn set_scope_request(
    nodes: &[TreeNode],
    sections: &[SectionSpec],
    row: Option<usize>,
    ctx: &TreeContext,
) -> std::result::Result<MovePrompt, &'static str> {
    open_move(nodes, sections, row, ctx, true)
}

/// The prompt behind [`move_request`] (`fixed` false) and
/// [`set_scope_request`] (`fixed` true).
pub(super) fn open_move(
    nodes: &[TreeNode],
    sections: &[SectionSpec],
    row: Option<usize>,
    ctx: &TreeContext,
    fixed: bool,
) -> std::result::Result<MovePrompt, &'static str> {
    let pick =
        |for_move: &'static str, for_scope: &'static str| if fixed { for_scope } else { for_move };
    let Some(node) = row.and_then(|i| nodes.get(i)) else {
        return Err(pick(NOTHING_TO_MOVE, NOTHING_TO_SET_SCOPE));
    };
    if node.depth == 0 {
        return Err(pick(SECTION_MOVE, SECTION_SET_SCOPE));
    }
    let Some(spec) = sections.get(node.section) else {
        return Err(pick(NOTHING_TO_MOVE, NOTHING_TO_SET_SCOPE));
    };
    if spec.is_doc {
        return Err(pick(DOCS_MOVE, DOCS_SET_SCOPE));
    }
    if is_todo_row(node, sections) {
        return Err(pick(TODOS_MOVE, TODOS_SET_SCOPE));
    }
    let notes = if node.is_dir {
        let section_roots: Vec<PathBuf> = sections.iter().map(|s| s.root.clone()).collect();
        if !folder_change_allowed(&node.path, &spec.root, &section_roots) {
            return Err(pick(
                FOLDER_HOLDS_SECTION_MOVE,
                FOLDER_HOLDS_SECTION_SET_SCOPE,
            ));
        }
        Some(folder_contents(&node.path).notes)
    } else {
        None
    };
    let current = ctx.current_project.as_deref();
    let project = spec.project.clone().or_else(|| ctx.current_project.clone());
    let root = NewNoteTarget {
        dir: spec.root.clone(),
        scope: spec.scope,
        label: scope_label(spec.scope, project.as_deref(), current),
    };
    let rel_to_root = |path: &Path| {
        path.strip_prefix(&spec.root)
            .map(|r| r.to_string_lossy().into_owned())
            .ok()
    };
    let mut scope = NewNotePrompt::open(root, project);
    scope.buffer = node.path.parent().and_then(rel_to_root).unwrap_or_default();
    Ok(MovePrompt {
        scope,
        src: node.path.clone(),
        rel: rel_to_root(&node.path).unwrap_or_else(|| node.name.clone()),
        from: spec.scope,
        notes,
        fixed,
    })
}

/// The prompt `S` leads the footer with: `set scope of <name>: <scope>
/// (Tab cycles, Enter applies)`.
pub(super) fn set_scope_lead(prompt: &MovePrompt) -> Vec<Span<'static>> {
    vec![Span::styled(
        format!(
            " set scope of {}: {} (Tab cycles, Enter applies)",
            file_name_of(&prompt.src),
            prompt.scope.target.label
        ),
        Style::default().fg(theme::MAUVE),
    )]
}

/// The move prompt that leads the footer while the folder is typed:
/// `move <name> to <scope>/<folder>_`.
pub(super) fn move_lead(prompt: &MovePrompt) -> Vec<Span<'static>> {
    vec![
        Span::styled(
            format!(
                " move {} to {}/",
                file_name_of(&prompt.src),
                prompt.scope.target.label
            ),
            Style::default().fg(theme::MAUVE),
        ),
        Span::styled(
            prompt.scope.buffer.clone(),
            Style::default().fg(theme::TEXT),
        ),
        Span::styled("_", Style::default().fg(theme::OVERLAY)),
    ]
}

/// A resolved move: the note or folder at `src` goes to `dst`. `rel` is its
/// path relative to its section root, `to_root` the destination scope's
/// root and `display` the destination as the prompt names it. `notes` is
/// `Some` for a folder (see [`MovePrompt`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct MovePlan {
    pub(super) src: PathBuf,
    pub(super) dst: PathBuf,
    pub(super) rel: String,
    pub(super) from: Scope,
    pub(super) to: Scope,
    pub(super) to_root: PathBuf,
    pub(super) display: String,
    pub(super) notes: Option<usize>,
}

impl MovePlan {
    /// The scope changes, so the move asks first.
    pub(super) fn needs_confirm(&self) -> bool {
        self.from != self.to
    }

    /// What moves, as messages name it: the note's `rel`, or a folder's
    /// `rel` with a trailing `/`.
    pub(super) fn what(&self) -> String {
        match self.notes {
            None => self.rel.clone(),
            Some(_) => format!("{}/", self.rel),
        }
    }

    /// The confirmation question (see [`move_question`]). A public store is
    /// `<repo>/notez`, so the repository is named by its directory. A
    /// folder is named `<rel>/` with its notes counted as the delete
    /// question counts them (`and its 2 notes`, `(no notes)`).
    fn question(&self) -> String {
        let repo = repo_name(&self.to_root);
        let what = match self.notes {
            None => self.rel.clone(),
            Some(0) => format!("{}/ (no notes)", self.rel),
            Some(1) => format!("{}/ and its 1 note", self.rel),
            Some(n) => format!("{}/ and its {n} notes", self.rel),
        };
        move_question(&what, &self.display, self.from, self.to, &repo)
    }
}

/// The repository a public store `<repo>/notez` belongs to, by its
/// directory name.
pub(super) fn repo_name(store_root: &Path) -> String {
    store_root
        .parent()
        .and_then(Path::file_name)
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Resolve the move prompt's `Enter`: the folder typed in the buffer under
/// the chosen scope's root. Nothing on disk is touched. `Err` is the footer
/// message: a folder that does not exist (or is hidden, a symlink, or
/// steps out with `..`), a global destination inside `personal/` (those
/// are the projects' stores) or inside the todo board's store, the row's
/// own folder, a destination inside
/// the folder being moved, or a destination name that is taken (never
/// overwritten). The messages start with the prompt's verb (`move:` or
/// `set scope:`).
pub(super) fn resolve_move(
    prompt: &MovePrompt,
    roots: &NewNoteRoots,
) -> std::result::Result<MovePlan, String> {
    let verb = prompt.verb();
    let target = &prompt.scope.target;
    let (dir, display) = resolve_folder(verb, target, &prompt.scope.buffer, roots)?;
    let name = file_name_of(&prompt.src);
    let dst = dir.join(&name);
    if dst == prompt.src {
        return Err(format!("{verb}: {name} is already in {display}"));
    }
    if prompt.notes.is_some() && dst.starts_with(&prompt.src) {
        return Err(format!("{verb}: {name}/ cannot go inside itself"));
    }
    if std::fs::symlink_metadata(&dst).is_ok() {
        return Err(format!("{verb}: {name} already exists in {display}"));
    }
    Ok(MovePlan {
        src: prompt.src.clone(),
        dst,
        rel: prompt.rel.clone(),
        from: prompt.from,
        to: target.scope,
        to_root: target.dir.clone(),
        display,
        notes: prompt.notes,
    })
}

/// The destination folder `buffer` names under `target`'s root, and the
/// destination as the prompt shows it (`personal/plans`). `Err` is the
/// footer message, starting with `verb`: a folder that does not exist
/// under exactly that spelling, is hidden, a symlink, or steps out with
/// `..`, or a global destination inside `personal/` or inside the todo
/// board's store (see [`in_todo_store`]).
pub(super) fn resolve_folder(
    verb: &str,
    target: &NewNoteTarget,
    buffer: &str,
    roots: &NewNoteRoots,
) -> std::result::Result<(PathBuf, String), String> {
    let folder: Vec<&str> = buffer.split('/').filter(|c| !c.is_empty()).collect();
    let display = if folder.is_empty() {
        target.label.clone()
    } else {
        format!("{}/{}", target.label, folder.join("/"))
    };
    let no_folder = || format!("{verb}: no folder {display}");
    if folder.iter().any(|c| c.starts_with('.')) {
        return Err(no_folder());
    }
    // Walk the typed folder one step at a time: each step must be listed
    // under exactly that spelling (a case-insensitive file system would
    // otherwise resolve `Personal` or `Plans`, and the tag keys would take
    // the typed spelling) and must be a real folder, not a symlink.
    if !std::fs::symlink_metadata(&target.dir).is_ok_and(|m| m.is_dir()) {
        return Err(no_folder());
    }
    let mut dir = target.dir.clone();
    for step in &folder {
        if !has_entry_named(&dir, step) {
            return Err(no_folder());
        }
        dir = dir.join(step);
        let Ok(meta) = std::fs::symlink_metadata(&dir) else {
            return Err(no_folder());
        };
        if meta.file_type().is_symlink() {
            return Err(format!("{verb}: {display} is not a plain folder"));
        }
        if !meta.is_dir() {
            return Err(no_folder());
        }
    }
    if target.scope == Scope::Global && dir.starts_with(roots.global.join("personal")) {
        return Err(format!(
            "{verb}: {display} holds the projects' personal notes, not global ones"
        ));
    }
    if target.scope == Scope::Global && in_todo_store(&dir, &roots.global) {
        return Err(format!("{verb}: {TODO_STORE_MANAGED}"));
    }
    Ok((dir, display))
}

/// A typing key in the move prompt: `Backspace` and characters edit the
/// destination folder; the prompt `S` opens has a fixed folder and ignores
/// them, as it ignores every other key.
pub(super) fn type_into_move(prompt: &mut MovePrompt, code: KeyCode) {
    if prompt.fixed {
        return;
    }
    match code {
        KeyCode::Backspace => {
            prompt.scope.buffer.pop();
        }
        KeyCode::Char(c) => prompt.scope.buffer.push(c),
        _ => {}
    }
}

/// `Enter` in the move or set-scope prompt: `Ok(None)` when there is
/// nothing to do (`S` on the scope the row is already in), otherwise
/// [`resolve_move`].
pub(super) fn resolve_enter(
    prompt: &MovePrompt,
    roots: &NewNoteRoots,
) -> std::result::Result<Option<MovePlan>, String> {
    if prompt.fixed && prompt.scope.target.scope == prompt.from {
        return Ok(None);
    }
    resolve_move(prompt, roots).map(Some)
}

/// The question a move across scopes asks: `move <rel> to <scope>/<folder>?
/// y/n`, with one clause in parentheses per thing the scope change does,
/// public first: into public it names the repository and says public; out
/// of public the note stays in the repository's history; into local
/// scratch it is not synced and not recoverable; out of the vault
/// (personal or global) into a repository the deletion syncs on exit.
fn move_question(rel: &str, display: &str, from: Scope, to: Scope, repo: &str) -> String {
    format!(
        "move {rel} to {display}?{} y/n",
        move_clauses(&[(from, to)], repo)
    )
}

/// The clauses of [`move_question`] for every `(from, to)` transition in
/// `transitions`, each clause once and in its fixed order (public first),
/// as ` (<clause>)` pieces; empty when no transition changes anything.
pub(super) fn move_clauses(transitions: &[(Scope, Scope)], repo: &str) -> String {
    let in_vault = |scope: Scope| matches!(scope, Scope::Personal | Scope::Global);
    let any = |applies: &dyn Fn(Scope, Scope) -> bool| {
        transitions.iter().any(|&(from, to)| applies(from, to))
    };
    let mut clauses = Vec::new();
    if any(&|from, to| to == Scope::Public && from != Scope::Public) {
        clauses.push(format!(
            "it will be in the {repo} repository, public, not yet committed"
        ));
    }
    if any(&|from, to| from == Scope::Public && to != Scope::Public) {
        clauses.push("it stays in the repository's git history".to_string());
    }
    if any(&|from, to| to == Scope::Local && from != Scope::Local) {
        clauses.push("scratch is not synced and not recoverable".to_string());
    }
    if any(&|from, to| in_vault(from) && !in_vault(to)) {
        clauses.push("it leaves the vault; the deletion syncs on exit".to_string());
    }
    clauses.iter().map(|c| format!(" ({c})")).collect()
}

/// The move confirmation that leads the footer while it is open.
pub(super) fn move_confirm_lead(plan: &MovePlan) -> Vec<Span<'static>> {
    vec![Span::styled(
        format!(" {}", plan.question()),
        Style::default().fg(theme::PEACH),
    )]
}

/// What a move leaves: the row for the cursor and the footer message.
#[derive(Debug)]
pub(super) struct MoveOutcome {
    pub(super) row: Option<usize>,
    pub(super) message: String,
    /// Whether `rebuild` listed the forest again (not the fallback).
    pub(super) relisted: bool,
}

/// A moved note the rebuilt list does not show (its destination is outside
/// the view): its destination tag root, new path and flags, so the exit
/// write still carries its tags.
pub(super) type CarriedTags = (PathBuf, PathBuf, u8);

/// Whether `key` answers the open move confirmation with yes. Only `y`
/// does; any other key cancels and nothing moves.
pub(super) fn move_confirmed(key: KeyCode) -> bool {
    key == KeyCode::Char('y')
}

/// The `.tags` root of the destination: its section's, or, for a store with
/// no section in the view, the root that section would have (the notez root
/// for personal and global, the store itself for public and local).
fn destination_tag_root(
    plan: &MovePlan,
    sections: &[SectionSpec],
    roots: &NewNoteRoots,
) -> PathBuf {
    if let Some(spec) = sections
        .iter()
        .find(|s| !s.is_doc && s.root == plan.to_root)
    {
        return spec.tag_root.clone();
    }
    match plan.to {
        Scope::Personal | Scope::Global => roots.global.clone(),
        Scope::Public | Scope::Local => plan.to_root.clone(),
    }
}

/// Move the note or folder through [`move_path::move_path`]. On failure
/// nothing is retired, and the list is read again (a copy that failed
/// midway leaves the destination on disk too). On success every moved
/// note row (the note itself, or each note at any depth under the folder)
/// has its old `.tags` keys (current and original path, in its old tag
/// root) retired and takes its new path as both `path` and `origin`, the
/// destination's tag root and section, and keeps its flags; every folder
/// row under a moved folder takes its new path and section. Tagged keys
/// under a moved folder that no row lists (notes in hidden folders) are
/// retired and go to `carried` at their new path, and `carried` entries
/// an earlier move left under the source take the new path and tag root.
/// No `.tags` file is written here. The forest is then rebuilt with the cursor on the moved
/// row, its section and folders expanded. A moved row the rebuilt list
/// does not show is reported by path, and every moved note it does not
/// list goes to `carried`.
pub(super) fn apply_move(
    forest: &mut Forest,
    retired: &mut Vec<(PathBuf, String)>,
    carried: &mut Vec<CarriedTags>,
    plan: &MovePlan,
    roots: &NewNoteRoots,
    rebuild: &dyn Fn() -> Result<Vec<SectionSpec>>,
) -> MoveOutcome {
    let moved = match move_and_repoint(forest, retired, carried, plan, roots) {
        Ok(moved) => moved,
        Err(e) => {
            let (row, relisted) = match rebuild() {
                Ok(sections) => (forest.rebuild(sections, &plan.src), true),
                Err(_) => (forest.nodes.iter().position(|n| n.path == plan.src), false),
            };
            return MoveOutcome {
                row,
                message: format!("move failed: {e}"),
                relisted,
            };
        }
    };
    let mut message = format!("moved {} to {}", plan.what(), plan.display);
    let mut refreshed = true;
    let sections = rebuild().unwrap_or_else(|e| {
        refreshed = false;
        message = format!("{message}, but the list could not be refreshed: {e:#}");
        sections_after_moves(forest, std::slice::from_ref(plan))
    });
    let row = forest.rebuild(sections, &plan.dst);
    carry_unlisted_moved(forest, carried, &moved);
    if row.is_none() && refreshed {
        message = format!("moved to {}", plan.dst.display());
    }
    MoveOutcome {
        row,
        message,
        relisted: refreshed,
    }
}

/// What [`move_and_repoint`] moved: the destination tag root and every
/// moved note row's new path with its flags.
pub(super) struct Moved {
    tag_root: PathBuf,
    notes: Vec<(PathBuf, u8)>,
}

/// The fallback listing when the rebuild after `plans` fails: the current
/// rows (already repointed) minus anything still under a source, plus every
/// folder row now under a destination.
pub(super) fn sections_after_moves(forest: &mut Forest, plans: &[MovePlan]) -> Vec<SectionSpec> {
    let mut sections = current_sections_without(forest, &|p| {
        plans.iter().any(|plan| p.starts_with(&plan.src))
    });
    for node in forest.nodes.iter().filter(|n| {
        n.is_dir && n.depth > 0 && plans.iter().any(|plan| n.path.starts_with(&plan.dst))
    }) {
        if let Some(spec) = sections.get_mut(node.section) {
            if !spec.dirs.contains(&node.path) {
                spec.dirs.push(node.path.clone());
            }
        }
    }
    sections
}

/// After the rebuild: every moved note the list does not show goes to
/// `carried`, so the exit write still carries its tags.
pub(super) fn carry_unlisted_moved(forest: &Forest, carried: &mut Vec<CarriedTags>, moved: &Moved) {
    for (path, flags) in &moved.notes {
        if !forest.nodes.iter().any(|n| !n.is_dir && n.path == *path) {
            carried.push((moved.tag_root.clone(), path.clone(), *flags));
        }
    }
}

/// The part of [`apply_move`] before its rebuild: move the note or folder
/// through [`move_path::move_path`], then retire and repoint as
/// [`apply_move`] describes. On failure nothing is retired or repointed.
pub(super) fn move_and_repoint(
    forest: &mut Forest,
    retired: &mut Vec<(PathBuf, String)>,
    carried: &mut Vec<CarriedTags>,
    plan: &MovePlan,
    roots: &NewNoteRoots,
) -> std::result::Result<Moved, move_path::MoveError> {
    move_path::move_path(&plan.src, &plan.dst)?;
    let tag_root = destination_tag_root(plan, &forest.sections, roots);
    let tag_idx = match forest.tag_roots.iter().position(|r| *r == tag_root) {
        Some(i) => i,
        None => {
            forest.initial.push(note_tags::load_tags(&tag_root));
            forest.tag_roots.push(tag_root.clone());
            forest.tag_roots.len() - 1
        }
    };
    let section = forest
        .sections
        .iter()
        .position(|s| !s.is_doc && s.root == plan.to_root);
    let src_tag_idx = forest
        .nodes
        .iter()
        .find(|n| n.depth > 0 && n.path == plan.src)
        .map(|n| n.tag_root);
    let mut moved_notes: Vec<(PathBuf, u8)> = Vec::new();
    for node in forest.nodes.iter_mut().filter(|n| n.depth > 0) {
        let Some(new_path) = moved_path(&node.path, &plan.src, &plan.dst) else {
            continue;
        };
        if !node.is_dir {
            let old_root = &forest.tag_roots[node.tag_root];
            for path in [&node.path, &node.origin] {
                if let Some(key) = rel_key(old_root, path) {
                    retire(retired, old_root, key);
                }
            }
            node.origin = new_path.clone();
            node.tag_root = tag_idx;
            moved_notes.push((new_path.clone(), node.flags));
        }
        node.path = new_path;
        if let Some(section) = section {
            node.section = section;
        }
    }
    // Tags carried by an earlier move (a hidden note, or a note out of the
    // view) follow this move too, so they stay on the file's current path.
    for (root, path, _) in carried.iter_mut() {
        if let Some(new_path) = moved_path(path, &plan.src, &plan.dst) {
            *path = new_path;
            *root = tag_root.clone();
        }
    }
    if let (Some(_), Some(old_idx)) = (plan.notes, src_tag_idx) {
        carry_unlisted_keys(forest, retired, carried, plan, old_idx, &tag_root);
    }
    Ok(Moved {
        tag_root,
        notes: moved_notes,
    })
}

/// For a moved folder: the keys of the source tag root's session map that
/// lie under the folder, belong to no row and name a file that is now under
/// the destination (a tagged note the tree does not list, such as one in a
/// hidden folder). Each is retired from the old tag root and goes to
/// `carried` at its new path with its flags, so the exit write moves it.
/// Keys a row already retired are left to that row.
fn carry_unlisted_keys(
    forest: &Forest,
    retired: &mut Vec<(PathBuf, String)>,
    carried: &mut Vec<CarriedTags>,
    plan: &MovePlan,
    old_idx: usize,
    tag_root: &Path,
) {
    let old_root = &forest.tag_roots[old_idx];
    let Some(folder_key) = rel_key(old_root, &plan.src) else {
        return;
    };
    let mut keys: Vec<(&String, u8)> = forest.initial[old_idx]
        .iter()
        .filter(|(k, _)| Path::new(k.as_str()).starts_with(&folder_key))
        .filter(|(k, _)| !retired.iter().any(|(r, key)| r == old_root && key == *k))
        .map(|(k, f)| (k, *f))
        .collect();
    keys.sort();
    for (key, flags) in keys {
        let Some(new_path) = moved_path(&old_root.join(key), &plan.src, &plan.dst) else {
            continue;
        };
        if !std::fs::symlink_metadata(&new_path).is_ok_and(|m| m.is_file()) {
            continue;
        }
        retire(retired, old_root, key.clone());
        carried.push((tag_root.to_path_buf(), new_path, flags));
    }
}

/// The tag maps to write on exit ([`changed_tag_maps_retiring`]), with the
/// `carried` tags of moved notes the list no longer shows added as rows of
/// their own. A carried note that is listed again keeps its listed row.
pub(super) fn exit_tag_maps(
    forest: &Forest,
    retired: &[(PathBuf, String)],
    carried: &[CarriedTags],
) -> Vec<(PathBuf, HashMap<String, u8>)> {
    let mut nodes = forest.nodes.clone();
    let mut tag_roots = forest.tag_roots.clone();
    let mut initial = forest.initial.clone();
    for (root, path, flags) in carried {
        if nodes.iter().any(|n| !n.is_dir && n.path == *path) {
            continue;
        }
        let tag_root = match tag_roots.iter().position(|r| r == root) {
            Some(i) => i,
            None => {
                tag_roots.push(root.clone());
                initial.push(note_tags::load_tags(root));
                tag_roots.len() - 1
            }
        };
        nodes.push(TreeNode {
            name: file_name_of(path),
            path: path.clone(),
            origin: path.clone(),
            is_dir: false,
            depth: 1,
            expanded: false,
            child_count: 0,
            parent_idx: None,
            flags: *flags,
            scope_icon: "",
            tag_root,
            section: 0,
        });
    }
    changed_tag_maps_retiring(&nodes, &tag_roots, &initial, retired)
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use super::*;

    // --- Move ---

    fn rel_entries(dir: &Path, entries: Vec<String>, from: &Path, to: &Path) -> Vec<String> {
        let from = from
            .strip_prefix(dir)
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let to = to.strip_prefix(dir).unwrap().to_string_lossy().into_owned();
        let mut out: Vec<String> = entries
            .into_iter()
            .map(|e| if e == from { to.clone() } else { e })
            .collect();
        out.sort();
        out
    }

    #[test]
    fn m_moves_a_note_to_a_folder_and_to_the_root_within_every_scope() {
        for (scope, name) in SCOPES {
            let (dir, _notez, roots) = move_fixture();
            let rebuild = || Ok(move_sections(&roots));
            let ctx = move_ctx(&roots);
            let mut forest = forest_of(move_sections(&roots));
            let (mut retired, mut carried) = (Vec::new(), Vec::new());
            let store = store_of(&roots, scope);

            let mut src = store.join("ideas/a.md");
            let i = row(&forest.nodes, path_str(&src));
            let prompt = move_request(&forest.nodes, &forest.sections, Some(i), &ctx).unwrap();
            assert_eq!(
                prompt.scope.buffer, "ideas",
                "{name}: prefilled with the note's folder"
            );

            for (folder, dst) in [
                ("plans", store.join("plans/a.md")),
                ("", store.join("a.md")),
            ] {
                let before = disk_entries(dir.path());
                let plan = move_plan(&forest, &ctx, &src, scope, folder).unwrap();
                assert!(!plan.needs_confirm(), "{name}: same scope, no question");
                assert_eq!(
                    disk_entries(dir.path()),
                    before,
                    "{name}: resolving touches nothing"
                );
                let outcome = apply_move(
                    &mut forest,
                    &mut retired,
                    &mut carried,
                    &plan,
                    &ctx.new_note_roots,
                    &rebuild,
                );

                assert_eq!(
                    disk_entries(dir.path()),
                    rel_entries(dir.path(), before, &src, &dst),
                    "{name} {folder:?}"
                );
                let row = outcome.row.expect("the moved note is listed");
                assert_eq!(
                    forest.nodes[row].path, dst,
                    "{name}: the cursor is on the moved note"
                );
                assert!(
                    get_visible_nodes(&forest.nodes).contains(&row),
                    "{name}: its folders are expanded"
                );
                let label = scope_label(
                    scope,
                    (scope != Scope::Global).then_some("proj"),
                    Some("proj"),
                );
                let shown = if folder.is_empty() {
                    label
                } else {
                    format!("{label}/{folder}")
                };
                assert_eq!(
                    outcome.message,
                    format!("moved {} to {shown}", plan.rel),
                    "{name}"
                );
                src = dst;
            }
            assert!(carried.is_empty());
        }
    }

    #[test]
    fn a_missing_folder_a_taken_name_or_the_same_folder_moves_nothing_and_says_so() {
        let (dir, _notez, roots) = move_fixture();
        let ctx = move_ctx(&roots);
        let forest = forest_of(move_sections(&roots));
        let personal = store_of(&roots, Scope::Personal);
        let src = personal.join("ideas/a.md");
        std::fs::write(personal.join("plans/a.md"), "taken").unwrap();
        let before = disk_entries(dir.path());

        let plan =
            |folder: &str| move_plan(&forest, &ctx, &src, Scope::Personal, folder).unwrap_err();
        assert_eq!(plan("nope"), "move: no folder personal/nope");
        assert_eq!(
            plan("ideas/a.md"),
            "move: no folder personal/ideas/a.md",
            "a file is no folder"
        );
        assert_eq!(
            plan("../proj/plans"),
            "move: no folder personal/../proj/plans"
        );
        assert_eq!(plan(".hidden"), "move: no folder personal/.hidden");
        assert_eq!(plan("plans"), "move: a.md already exists in personal/plans");
        assert_eq!(plan("ideas/"), "move: a.md is already in personal/ideas");
        let global = move_plan(&forest, &ctx, &src, Scope::Global, "personal/proj").unwrap_err();
        assert!(
            global.starts_with("move: global/personal/proj holds the projects' personal"),
            "{global}"
        );
        let public = move_plan(&forest, &ctx, &src, Scope::Public, "nope").unwrap_err();
        assert_eq!(
            public,
            "move: no folder public (committed with the project)/nope"
        );

        assert_eq!(disk_entries(dir.path()), before, "nothing moved");

        std::fs::remove_dir_all(store_of(&roots, Scope::Local)).unwrap();
        let local = move_plan(&forest, &ctx, &src, Scope::Local, "").unwrap_err();
        assert_eq!(
            local, "move: no folder local scratch",
            "a missing store root"
        );
        assert!(src.is_file());
    }

    #[test]
    fn a_move_that_fails_retires_nothing_and_keeps_the_note() {
        let (_dir, _notez, roots) = move_fixture();
        let rebuild = || Ok(move_sections(&roots));
        let ctx = move_ctx(&roots);
        let mut forest = forest_of(move_sections(&roots));
        let (mut retired, mut carried) = (Vec::new(), Vec::new());
        let src = store_of(&roots, Scope::Personal).join("ideas/a.md");
        let plan = move_plan(&forest, &ctx, &src, Scope::Public, "plans").unwrap();
        std::fs::write(&plan.dst, "appeared meanwhile").unwrap();

        let outcome = apply_move(
            &mut forest,
            &mut retired,
            &mut carried,
            &plan,
            &ctx.new_note_roots,
            &rebuild,
        );
        assert!(
            outcome.message.starts_with("move failed: ")
                && outcome.message.contains("already exists"),
            "{}",
            outcome.message
        );
        assert!(retired.is_empty() && carried.is_empty());
        assert_eq!(
            std::fs::read_to_string(&plan.dst).unwrap(),
            "appeared meanwhile"
        );
        assert!(src.is_file());
        assert_eq!(
            forest.nodes[outcome.row.unwrap()].path,
            src,
            "the cursor stays on the note"
        );
        assert!(final_maps(&forest, &retired, &carried, &roots)
            .into_iter()
            .all(|(root, map)| map == note_tags::load_tags(&root)));
    }

    /// Every ordered pair of different scopes: a tagged and an untagged note
    /// move; the tagged note's key leaves the source tag root and arrives in
    /// the destination one with its flags, the untagged one gets no key, and
    /// every other key stays.
    #[test]
    fn a_move_across_scopes_carries_the_tags_for_every_pair() {
        for (from, from_name) in SCOPES {
            for (to, to_name) in SCOPES {
                if from == to {
                    continue;
                }
                let pair = format!("{from_name} to {to_name}");
                let (_dir, _notez, roots) = move_fixture();
                let rebuild = || Ok(move_sections(&roots));
                let ctx = move_ctx(&roots);
                let mut forest = forest_of(move_sections(&roots));
                let (mut retired, mut carried) = (Vec::new(), Vec::new());
                let (src_store, dst_store) = (store_of(&roots, from), store_of(&roots, to));
                let (src_tags, dst_tags) = (move_tag_root(&roots, from), move_tag_root(&roots, to));
                let initial = final_maps(&forest, &[], &[], &roots);
                let flags =
                    forest.nodes[row(&forest.nodes, path_str(&src_store.join("ideas/a.md")))].flags;
                assert_ne!(flags, 0, "{pair}");

                for file in ["a.md", "b.md"] {
                    let src = src_store.join("ideas").join(file);
                    let plan = move_plan(&forest, &ctx, &src, to, "plans").unwrap();
                    assert!(plan.needs_confirm(), "{pair}");
                    let outcome = apply_move(
                        &mut forest,
                        &mut retired,
                        &mut carried,
                        &plan,
                        &ctx.new_note_roots,
                        &rebuild,
                    );
                    let dst = dst_store.join("plans").join(file);
                    assert!(dst.is_file() && !src.exists(), "{pair} {file}");
                    let moved = &forest.nodes[outcome.row.expect("listed")];
                    assert_eq!(moved.path, dst, "{pair}");
                    assert_eq!(
                        forest.sections[moved.section].root, dst_store,
                        "{pair}: section"
                    );
                    assert_eq!(
                        forest.tag_roots[moved.tag_root], dst_tags,
                        "{pair}: tag root"
                    );
                }

                let mut expected = initial.clone();
                let key = |root: &Path, store: &Path, file: &str| {
                    rel_key(root, &store.join(file)).unwrap()
                };
                expected.get_mut(&src_tags).unwrap().remove(&key(
                    &src_tags,
                    &src_store,
                    "ideas/a.md",
                ));
                expected
                    .get_mut(&dst_tags)
                    .unwrap()
                    .insert(key(&dst_tags, &dst_store, "plans/a.md"), flags);
                let finals = final_maps(&forest, &retired, &carried, &roots);
                assert_eq!(finals, expected, "{pair}");
                for map in finals.values() {
                    assert!(
                        !map.keys().any(|k| k.ends_with("b.md")),
                        "{pair}: the untagged note has no key"
                    );
                }
                assert_eq!(
                    finals[&move_tag_root(&roots, Scope::Global)].get("other.md"),
                    Some(&16),
                    "{pair}"
                );
            }
        }
    }

    /// A destination outside the view (a one-scope view) is reported by
    /// path, and the exit write still moves the tags.
    #[test]
    fn a_move_out_of_the_view_reports_the_path_and_keeps_the_tags() {
        let (_dir, notez, roots) = move_fixture();
        let personal_only = || {
            move_sections(&roots)
                .into_iter()
                .take(1)
                .collect::<Vec<_>>()
        };
        let rebuild = || Ok(personal_only());
        let ctx = move_ctx(&roots);
        let mut forest = forest_of(personal_only());
        let (mut retired, mut carried) = (Vec::new(), Vec::new());
        let personal = store_of(&roots, Scope::Personal);
        let public = store_of(&roots, Scope::Public);

        for file in ["a.md", "b.md"] {
            let plan = move_plan(
                &forest,
                &ctx,
                &personal.join("ideas").join(file),
                Scope::Public,
                "plans",
            )
            .unwrap();
            let outcome = apply_move(
                &mut forest,
                &mut retired,
                &mut carried,
                &plan,
                &ctx.new_note_roots,
                &rebuild,
            );
            assert_eq!(outcome.row, None);
            assert_eq!(
                outcome.message,
                format!("moved to {}", public.join("plans").join(file).display())
            );
        }
        let finals = final_maps(&forest, &retired, &carried, &roots);
        assert_eq!(finals[&notez].get("personal/proj/ideas/a.md"), None);
        assert_eq!(finals[&public].get("plans/a.md"), Some(&1));
        assert_eq!(finals[&public].get("plans/b.md"), None);
        assert_eq!(
            finals[&public].get("ideas/a.md"),
            Some(&3),
            "other keys stay"
        );
    }

    #[test]
    fn a_move_whose_list_refresh_fails_still_shows_the_note_at_its_new_place() {
        let (_dir, notez, roots) = move_fixture();
        let rebuild = || -> Result<Vec<SectionSpec>> { Err(anyhow::anyhow!("listing broke")) };
        let ctx = move_ctx(&roots);
        let mut forest = forest_of(move_sections(&roots));
        let (mut retired, mut carried) = (Vec::new(), Vec::new());
        let src = store_of(&roots, Scope::Personal).join("ideas/a.md");
        let plan = move_plan(&forest, &ctx, &src, Scope::Global, "plans").unwrap();

        let outcome = apply_move(
            &mut forest,
            &mut retired,
            &mut carried,
            &plan,
            &ctx.new_note_roots,
            &rebuild,
        );
        assert!(
            outcome
                .message
                .contains("could not be refreshed: listing broke"),
            "{}",
            outcome.message
        );
        let moved = &forest.nodes[outcome.row.expect("listed from the current rows")];
        assert_eq!(moved.path, notez.join("plans/a.md"));
        let finals = final_maps(&forest, &retired, &carried, &roots);
        assert_eq!(finals[&notez].get("plans/a.md"), Some(&1));
        assert_eq!(finals[&notez].get("personal/proj/ideas/a.md"), None);
    }

    #[test]
    fn the_move_question_names_each_transition_public_first() {
        use Scope::{Global, Local, Personal, Public};
        let into_public = "(it will be in the repo repository, public, not yet committed)";
        let out_of_public = "(it stays in the repository's git history)";
        let into_local = "(scratch is not synced and not recoverable)";
        let leaves_vault = "(it leaves the vault; the deletion syncs on exit)";
        let cases: [(Scope, Scope, Vec<&str>); 12] = [
            (Personal, Public, vec![into_public, leaves_vault]),
            (Personal, Local, vec![into_local, leaves_vault]),
            (Personal, Global, vec![]),
            (Public, Personal, vec![out_of_public]),
            (Public, Local, vec![out_of_public, into_local]),
            (Public, Global, vec![out_of_public]),
            (Local, Personal, vec![]),
            (Local, Public, vec![into_public]),
            (Local, Global, vec![]),
            (Global, Personal, vec![]),
            (Global, Public, vec![into_public, leaves_vault]),
            (Global, Local, vec![into_local, leaves_vault]),
        ];
        for (from, to, clauses) in cases {
            let mut expected = "move ideas/a.md to dest/plans?".to_string();
            for clause in &clauses {
                expected = format!("{expected} {clause}");
            }
            expected.push_str(" y/n");
            assert_eq!(
                move_question("ideas/a.md", "dest/plans", from, to, "repo"),
                expected,
                "{from:?} to {to:?}"
            );
            assert_eq!(
                expected.contains("public,"),
                to == Public,
                "{from:?} to {to:?}"
            );
        }
    }

    #[test]
    fn the_confirm_opens_only_for_a_new_scope_names_the_repository_and_takes_only_y() {
        let (_dir, _notez, roots) = move_fixture();
        let ctx = move_ctx(&roots);
        let forest = forest_of(move_sections(&roots));
        let src = store_of(&roots, Scope::Personal).join("ideas/a.md");
        assert!(!move_plan(&forest, &ctx, &src, Scope::Personal, "plans")
            .unwrap()
            .needs_confirm());
        let plan = move_plan(&forest, &ctx, &src, Scope::Public, "plans").unwrap();
        assert!(plan.needs_confirm());
        assert_eq!(
            plan.question(),
            "move ideas/a.md to public (committed with the project)/plans? \
             (it will be in the p repository, public, not yet committed) \
             (it leaves the vault; the deletion syncs on exit) y/n"
        );
        let rendered = text_of(&lead_with_hints(
            move_confirm_lead(&plan),
            Mode::ConfirmMove,
            &[],
            400,
        ));
        assert!(
            rendered.starts_with(&format!(" {}", plan.question())),
            "{rendered}"
        );
        assert!(
            rendered.contains("y confirm") && rendered.contains("n/esc cancel"),
            "{rendered}"
        );
        assert!(move_confirmed(KeyCode::Char('y')));
        for key in [
            KeyCode::Char('n'),
            KeyCode::Esc,
            KeyCode::Char('Y'),
            KeyCode::Enter,
        ] {
            assert!(!move_confirmed(key), "{key:?}");
        }
    }

    #[test]
    fn the_move_prompt_tabs_through_the_scopes_and_keeps_the_typed_folder() {
        let (_dir, notez, roots) = move_fixture();
        let ctx = move_ctx(&roots);
        let forest = forest_of(move_sections(&roots));
        let src = store_of(&roots, Scope::Personal).join("ideas/a.md");
        let mut prompt = move_request(
            &forest.nodes,
            &forest.sections,
            Some(row(&forest.nodes, path_str(&src))),
            &ctx,
        )
        .unwrap();
        assert_eq!(
            text_of(&Line::from(move_lead(&prompt))),
            " move a.md to personal/ideas_"
        );
        prompt.scope.buffer = "plans".to_string();
        let mut seen = Vec::new();
        for _ in 0..4 {
            let p = &prompt.scope;
            prompt.scope.target = next_scope_target(
                &p.target,
                &p.origin,
                p.project.as_deref(),
                &ctx.new_note_roots,
                Some("proj"),
            );
            seen.push((prompt.scope.target.scope, prompt.scope.target.dir.clone()));
            assert_eq!(prompt.scope.buffer, "plans");
        }
        assert_eq!(
            seen,
            vec![
                (Scope::Public, store_of(&roots, Scope::Public)),
                (Scope::Local, store_of(&roots, Scope::Local)),
                (Scope::Global, notez.clone()),
                (Scope::Personal, store_of(&roots, Scope::Personal)),
            ]
        );
        let rendered = text_of(&lead_with_hints(move_lead(&prompt), Mode::Move, &[], 200));
        assert!(
            rendered.starts_with(" move a.md to personal/plans_"),
            "{rendered}"
        );

        // A global note offers the scopes of the project the browser is in.
        let global = notez.join("c.md");
        let mut prompt = move_request(
            &forest.nodes,
            &forest.sections,
            Some(row(&forest.nodes, path_str(&global))),
            &ctx,
        )
        .unwrap();
        assert_eq!(
            prompt.scope.buffer, "",
            "a note at the root has an empty folder"
        );
        let p = &prompt.scope;
        prompt.scope.target = next_scope_target(
            &p.target,
            &p.origin,
            p.project.as_deref(),
            &ctx.new_note_roots,
            Some("proj"),
        );
        assert_eq!(prompt.scope.target.scope, Scope::Personal);
        assert!(
            !seen.iter().any(|(_, dir)| dir.ends_with("docs")),
            "docs are never offered"
        );
    }

    #[test]
    fn m_on_a_section_a_docs_row_or_an_empty_tree_is_refused() {
        let sections = project_sections();
        let (nodes, _) = build_forest(&sections);
        let ctx = ctx_with(Some("proj"), roots());
        let at = |path: &str| move_request(&nodes, &sections, Some(row(&nodes, path)), &ctx).err();
        assert_eq!(at("/n/personal/proj"), Some(SECTION_MOVE));
        assert_eq!(at("/p/docs/design/c.md"), Some(DOCS_MOVE));
        assert_eq!(at("/p/docs/design"), Some(DOCS_MOVE));
        assert!(
            at("/n/personal/proj/ideas").is_none(),
            "a folder opens the prompt"
        );
        assert!(at("/n/personal/proj/ideas/a.md").is_none());
        assert_eq!(
            move_request(&[], &[], None, &ctx).err(),
            Some(NOTHING_TO_MOVE)
        );
        assert_eq!(
            move_request(&[], &[], Some(0), &ctx).err(),
            Some(NOTHING_TO_MOVE)
        );
        assert_eq!(
            move_request(&nodes, &sections, Some(nodes.len()), &ctx).err(),
            Some(NOTHING_TO_MOVE)
        );
    }

    #[test]
    fn s_on_a_section_a_docs_row_or_an_empty_tree_is_refused() {
        let sections = project_sections();
        let (nodes, _) = build_forest(&sections);
        let ctx = ctx_with(Some("proj"), roots());
        let at =
            |path: &str| set_scope_request(&nodes, &sections, Some(row(&nodes, path)), &ctx).err();
        assert_eq!(at("/n/personal/proj"), Some(SECTION_SET_SCOPE));
        assert_eq!(at("/p/docs/design/c.md"), Some(DOCS_SET_SCOPE));
        assert_eq!(at("/p/docs/design"), Some(DOCS_SET_SCOPE));
        assert!(
            at("/n/personal/proj/ideas").is_none(),
            "a folder opens the prompt"
        );
        assert!(at("/n/personal/proj/ideas/a.md").is_none());
        assert_eq!(
            set_scope_request(&[], &[], None, &ctx).err(),
            Some(NOTHING_TO_SET_SCOPE)
        );
        assert_eq!(
            set_scope_request(&[], &[], Some(0), &ctx).err(),
            Some(NOTHING_TO_SET_SCOPE)
        );
        assert_eq!(
            set_scope_request(&nodes, &sections, Some(nodes.len()), &ctx).err(),
            Some(NOTHING_TO_SET_SCOPE)
        );
    }

    /// A folder that holds another section's root (a global folder over a
    /// project's personal store) is refused by both keys.
    #[test]
    fn m_and_s_refuse_a_folder_that_holds_another_section() {
        let (_dir, notez, roots) = move_fixture();
        let ctx = move_ctx(&roots);
        let mut sections = move_sections(&roots);
        let global = sections
            .iter_mut()
            .find(|s| s.scope == Scope::Global)
            .unwrap();
        global.dirs.push(notez.join("personal"));
        let (nodes, _) = build_forest(&sections);
        let i = Some(row(&nodes, path_str(&notez.join("personal"))));
        assert_eq!(
            move_request(&nodes, &sections, i, &ctx).err(),
            Some(FOLDER_HOLDS_SECTION_MOVE)
        );
        assert_eq!(
            set_scope_request(&nodes, &sections, i, &ctx).err(),
            Some(FOLDER_HOLDS_SECTION_SET_SCOPE)
        );
    }

    #[test]
    fn m_is_a_browse_key_after_n_with_its_prompt_and_confirm_keys() {
        let rows: Vec<(usize, &KeyHint)> = TREE_KEYS
            .iter()
            .enumerate()
            .filter(|(_, k)| k.key == "m")
            .collect();
        assert_eq!(rows.len(), 1);
        let (idx, hint) = rows[0];
        assert_eq!(hint.modes, BROWSE);
        assert_eq!(hint.group, Group::Edit);
        assert_eq!(hint.help, "move note or folder");
        assert_eq!(hint.slot, Slot::Priority(9));
        assert_eq!(
            idx,
            TREE_KEYS.iter().position(|k| k.key == "N").unwrap() + 1
        );
        assert!(help::rows(TREE_KEYS).contains(&help::Row::Key(idx)));
        assert!(shown_keys(Mode::Normal, &[], 200).contains(&"m"));
        assert!(shown_keys(Mode::Focus, &[], 200).contains(&"m"));
        for mode in [
            Mode::Tag,
            Mode::Filter,
            Mode::Rename,
            Mode::NewItem,
            Mode::ConfirmDelete,
            Mode::VimCommand,
            Mode::Move,
            Mode::ConfirmMove,
        ] {
            assert!(!shown_keys(mode, &[], 200).contains(&"m"), "{mode:?}");
        }
        let keys_in = |mode: Mode| {
            TREE_KEYS
                .iter()
                .filter(|k| k.modes.contains(&mode))
                .map(|k| k.key)
                .collect::<Vec<_>>()
        };
        assert_eq!(keys_in(Mode::Move), vec!["enter", "esc", "tab", "bksp"]);
        assert_eq!(keys_in(Mode::ConfirmMove), vec!["y", "n/esc"]);
        let with_n = (0..200)
            .find(|&w| shown_keys(Mode::Normal, &[], w).contains(&"N"))
            .unwrap();
        let with_m = (0..200)
            .find(|&w| shown_keys(Mode::Normal, &[], w).contains(&"m"))
            .unwrap();
        assert!(with_m > with_n, "m drops before N");
    }

    // --- Move: folders and set scope ---

    /// The note `ideas/deep/deeper/n.md` two levels down a moved folder, the
    /// empty folder `ideas/deep/empty` and the hidden note
    /// `ideas/.hidden/h.md` the tree does not list, added to every store of
    /// [`move_fixture`]. The deep note is tagged 5 and the hidden one 6 in
    /// every tag root (global's own keys 9 and 10).
    fn folder_fixture() -> (tempfile::TempDir, PathBuf, Vec<(Scope, PathBuf)>) {
        let (dir, notez, roots) = move_fixture();
        for (_, root) in &roots {
            std::fs::create_dir_all(root.join("ideas/deep/deeper")).unwrap();
            std::fs::create_dir_all(root.join("ideas/deep/empty")).unwrap();
            std::fs::create_dir_all(root.join("ideas/.hidden")).unwrap();
            std::fs::write(root.join("ideas/deep/deeper/n.md"), "# deep\n").unwrap();
            std::fs::write(root.join("ideas/.hidden/h.md"), "# hidden\n").unwrap();
        }
        let append = |root: &Path, lines: &str| {
            let tags = root.join(".tags");
            let old = std::fs::read_to_string(&tags).unwrap();
            std::fs::write(&tags, format!("{old}{lines}")).unwrap();
        };
        append(
            &notez,
            "personal/proj/ideas/deep/deeper/n.md:5\npersonal/proj/ideas/.hidden/h.md:6\n\
             ideas/deep/deeper/n.md:9\nideas/.hidden/h.md:10\n",
        );
        for scope in [Scope::Public, Scope::Local] {
            append(
                &store_of(&roots, scope),
                "ideas/deep/deeper/n.md:5\nideas/.hidden/h.md:6\n",
            );
        }
        (dir, notez, roots)
    }

    /// Every pair of scopes, the same one included: `ideas/` with its three
    /// listed notes (one two levels down), an empty folder and a hidden
    /// tagged note moves into `plans/`. Every note row and folder row takes
    /// its new path, section and tag root; every tagged key leaves the
    /// source tag root and arrives in the destination one with its flags;
    /// the untagged note gets no key and every other key stays.
    #[test]
    fn m_moves_a_folder_with_every_note_and_its_tags_within_and_across_scopes() {
        let files = ["a.md", "b.md", "deep/deeper/n.md", ".hidden/h.md"];
        for (from, from_name) in SCOPES {
            for (to, to_name) in SCOPES {
                let pair = format!("{from_name} to {to_name}");
                let (_dir, _notez, roots) = folder_fixture();
                let rebuild = || Ok(move_sections(&roots));
                let ctx = move_ctx(&roots);
                let mut forest = forest_of(move_sections(&roots));
                let (mut retired, mut carried) = (Vec::new(), Vec::new());
                let (src_store, dst_store) = (store_of(&roots, from), store_of(&roots, to));
                let (src_tags, dst_tags) = (move_tag_root(&roots, from), move_tag_root(&roots, to));
                let src = src_store.join("ideas");
                let dst = dst_store.join("plans/ideas");
                let initial = final_maps(&forest, &[], &[], &roots);

                let prompt = move_request(
                    &forest.nodes,
                    &forest.sections,
                    Some(row(&forest.nodes, path_str(&src))),
                    &ctx,
                )
                .unwrap();
                assert_eq!(
                    prompt.scope.buffer, "",
                    "{pair}: prefilled with the folder's parent"
                );
                let plan = move_plan(&forest, &ctx, &src, to, "plans").unwrap();
                assert_eq!(plan.dst, dst, "{pair}");
                assert_eq!(plan.needs_confirm(), from != to, "{pair}");
                assert!(
                    plan.question()
                        .starts_with("move ideas/ and its 3 notes to "),
                    "{pair}: {}",
                    plan.question()
                );

                let outcome = apply_move(
                    &mut forest,
                    &mut retired,
                    &mut carried,
                    &plan,
                    &ctx.new_note_roots,
                    &rebuild,
                );
                assert!(!src.exists(), "{pair}");
                for file in files {
                    assert!(dst.join(file).is_file(), "{pair}: {file}");
                }
                assert!(dst.join("deep/empty").is_dir(), "{pair}");
                let label = scope_label(to, (to != Scope::Global).then_some("proj"), Some("proj"));
                assert_eq!(
                    outcome.message,
                    format!("moved ideas/ to {label}/plans"),
                    "{pair}"
                );
                let at = outcome.row.expect("the moved folder is listed");
                assert_eq!(
                    forest.nodes[at].path, dst,
                    "{pair}: the cursor is on the moved folder"
                );
                assert!(forest.nodes[at].is_dir);
                assert!(
                    get_visible_nodes(&forest.nodes).contains(&at),
                    "{pair}: its section and parent are expanded"
                );
                assert!(
                    !forest.nodes.iter().any(|n| n.path.starts_with(&src)),
                    "{pair}: no row at the old place"
                );
                for path in [
                    "a.md",
                    "b.md",
                    "deep/deeper/n.md",
                    "deep",
                    "deep/deeper",
                    "deep/empty",
                ] {
                    let node = &forest.nodes[row(&forest.nodes, path_str(&dst.join(path)))];
                    assert_eq!(
                        forest.sections[node.section].root, dst_store,
                        "{pair}: {path} section"
                    );
                    if !node.is_dir {
                        assert_eq!(
                            forest.tag_roots[node.tag_root], dst_tags,
                            "{pair}: {path} tag root"
                        );
                    }
                }

                let mut expected = initial.clone();
                let key = |root: &Path, path: &Path| rel_key(root, path).unwrap();
                let mut moved = Vec::new();
                for file in ["a.md", "deep/deeper/n.md", ".hidden/h.md"] {
                    let old_key = key(&src_tags, &src.join(file));
                    let flags = expected[&src_tags]
                        .get(&old_key)
                        .copied()
                        .expect("tagged in the fixture");
                    expected.get_mut(&src_tags).unwrap().remove(&old_key);
                    moved.push((key(&dst_tags, &dst.join(file)), flags));
                }
                for (new_key, flags) in moved {
                    expected.get_mut(&dst_tags).unwrap().insert(new_key, flags);
                }
                let finals = final_maps(&forest, &retired, &carried, &roots);
                assert_eq!(finals, expected, "{pair}");
                assert!(
                    !finals[&dst_tags]
                        .keys()
                        .any(|k| k.ends_with("plans/ideas/b.md")),
                    "{pair}: untagged"
                );
            }
        }
    }

    #[test]
    fn a_folder_move_into_itself_onto_a_taken_name_or_a_missing_folder_moves_nothing() {
        let (dir, _notez, roots) = folder_fixture();
        let ctx = move_ctx(&roots);
        let forest = forest_of(move_sections(&roots));
        let personal = store_of(&roots, Scope::Personal);
        let src = personal.join("ideas");
        std::fs::create_dir(personal.join("plans/ideas")).unwrap();
        let before = disk_entries(dir.path());

        let plan =
            |scope: Scope, folder: &str| move_plan(&forest, &ctx, &src, scope, folder).unwrap_err();
        assert_eq!(
            plan(Scope::Personal, "ideas"),
            "move: ideas/ cannot go inside itself"
        );
        assert_eq!(
            plan(Scope::Personal, "ideas/deep/deeper"),
            "move: ideas/ cannot go inside itself"
        );
        assert_eq!(
            plan(Scope::Personal, ""),
            "move: ideas is already in personal"
        );
        assert_eq!(
            plan(Scope::Personal, "plans"),
            "move: ideas already exists in personal/plans"
        );
        assert_eq!(
            plan(Scope::Personal, "nope"),
            "move: no folder personal/nope"
        );
        assert_eq!(
            plan(Scope::Public, ""),
            "move: ideas already exists in public (committed with the project)"
        );
        assert_eq!(disk_entries(dir.path()), before, "nothing moved");
    }

    #[test]
    fn a_folder_move_that_fails_retires_nothing_and_keeps_every_note() {
        let (_dir, _notez, roots) = folder_fixture();
        let rebuild = || Ok(move_sections(&roots));
        let ctx = move_ctx(&roots);
        let mut forest = forest_of(move_sections(&roots));
        let (mut retired, mut carried) = (Vec::new(), Vec::new());
        let src = store_of(&roots, Scope::Personal).join("ideas");
        let plan = move_plan(&forest, &ctx, &src, Scope::Public, "plans").unwrap();
        std::fs::create_dir(&plan.dst).unwrap();

        let outcome = apply_move(
            &mut forest,
            &mut retired,
            &mut carried,
            &plan,
            &ctx.new_note_roots,
            &rebuild,
        );
        assert!(
            outcome.message.starts_with("move failed: "),
            "{}",
            outcome.message
        );
        assert!(retired.is_empty() && carried.is_empty());
        assert!(src.join("deep/deeper/n.md").is_file() && src.join(".hidden/h.md").is_file());
        assert_eq!(
            forest.nodes[outcome.row.unwrap()].path,
            src,
            "the cursor stays on the folder"
        );
        assert!(final_maps(&forest, &retired, &carried, &roots)
            .into_iter()
            .all(|(root, map)| map == note_tags::load_tags(&root)));
    }

    #[test]
    fn a_folder_moved_out_of_the_view_is_reported_and_carries_every_key() {
        let (_dir, notez, roots) = folder_fixture();
        let personal_only = || {
            move_sections(&roots)
                .into_iter()
                .take(1)
                .collect::<Vec<_>>()
        };
        let rebuild = || Ok(personal_only());
        let ctx = move_ctx(&roots);
        let mut forest = forest_of(personal_only());
        let (mut retired, mut carried) = (Vec::new(), Vec::new());
        let public = store_of(&roots, Scope::Public);
        let src = store_of(&roots, Scope::Personal).join("ideas");

        let plan = move_plan(&forest, &ctx, &src, Scope::Public, "plans").unwrap();
        let outcome = apply_move(
            &mut forest,
            &mut retired,
            &mut carried,
            &plan,
            &ctx.new_note_roots,
            &rebuild,
        );
        assert_eq!(outcome.row, None);
        assert_eq!(
            outcome.message,
            format!("moved to {}", public.join("plans/ideas").display())
        );
        let finals = final_maps(&forest, &retired, &carried, &roots);
        for key in ["ideas/a.md", "ideas/deep/deeper/n.md", "ideas/.hidden/h.md"] {
            assert_eq!(
                finals[&notez].get(&format!("personal/proj/{key}")),
                None,
                "{key}"
            );
        }
        assert_eq!(finals[&public].get("plans/ideas/a.md"), Some(&1));
        assert_eq!(
            finals[&public].get("plans/ideas/deep/deeper/n.md"),
            Some(&5)
        );
        assert_eq!(finals[&public].get("plans/ideas/.hidden/h.md"), Some(&6));
        assert_eq!(finals[&public].get("plans/ideas/b.md"), None);
        assert_eq!(
            finals[&public].get("ideas/a.md"),
            Some(&3),
            "other keys stay"
        );
        assert_eq!(
            finals[&notez].get("ideas/a.md"),
            Some(&4),
            "other keys stay"
        );
    }

    #[test]
    fn a_folder_move_whose_list_refresh_fails_lists_the_folder_at_its_new_place() {
        let (_dir, notez, roots) = folder_fixture();
        let rebuild = || -> Result<Vec<SectionSpec>> { Err(anyhow::anyhow!("listing broke")) };
        let ctx = move_ctx(&roots);
        let mut forest = forest_of(move_sections(&roots));
        let (mut retired, mut carried) = (Vec::new(), Vec::new());
        let src = store_of(&roots, Scope::Personal).join("ideas");
        let dst = notez.join("plans/ideas");
        let plan = move_plan(&forest, &ctx, &src, Scope::Global, "plans").unwrap();

        let outcome = apply_move(
            &mut forest,
            &mut retired,
            &mut carried,
            &plan,
            &ctx.new_note_roots,
            &rebuild,
        );
        assert!(
            outcome
                .message
                .contains("could not be refreshed: listing broke"),
            "{}",
            outcome.message
        );
        assert_eq!(
            forest.nodes[outcome.row.expect("listed from the current rows")].path,
            dst
        );
        for path in ["a.md", "deep/deeper/n.md", "deep/empty"] {
            let node = &forest.nodes[row(&forest.nodes, path_str(&dst.join(path)))];
            assert_eq!(forest.sections[node.section].root, notez, "{path}");
        }
        assert!(
            !forest.nodes.iter().any(|n| n.path.starts_with(&src)),
            "no row at the old place"
        );
        let finals = final_maps(&forest, &retired, &carried, &roots);
        assert_eq!(finals[&notez].get("plans/ideas/deep/deeper/n.md"), Some(&5));
        assert_eq!(
            finals[&notez].get("personal/proj/ideas/deep/deeper/n.md"),
            None
        );
    }

    /// A typed folder must exist under exactly that spelling and through
    /// real folders only: a case variant (which a case-insensitive file
    /// system would resolve, reaching the projects' `personal/` stores from
    /// global) and a symlinked folder at any step are refused, nothing moved.
    #[test]
    fn a_case_variant_or_a_symlinked_folder_in_the_typed_path_is_refused() {
        let (dir, notez, roots) = folder_fixture();
        let ctx = move_ctx(&roots);
        let personal = store_of(&roots, Scope::Personal);
        let outside = dir.path().join("outside");
        std::fs::create_dir_all(outside.join("sub")).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, personal.join("link")).unwrap();
        let forest = forest_of(move_sections(&roots));
        let before = disk_entries(dir.path());
        let tags_before = final_maps(&forest, &[], &[], &roots);

        let global_note = notez.join("c.md");
        let refused = move_plan(
            &forest,
            &ctx,
            &global_note,
            Scope::Global,
            "Personal/proj/plans",
        )
        .unwrap_err();
        assert_eq!(refused, "move: no folder global/Personal/proj/plans");
        let refused =
            move_plan(&forest, &ctx, &global_note, Scope::Global, "PERSONAL").unwrap_err();
        assert_eq!(refused, "move: no folder global/PERSONAL");
        for (scope, label) in [(Scope::Personal, "personal"), (Scope::Global, "global")] {
            let src = store_of(&roots, scope).join("c.md");
            let refused = move_plan(&forest, &ctx, &src, scope, "Plans").unwrap_err();
            assert_eq!(refused, format!("move: no folder {label}/Plans"));
            let refused = move_plan(&forest, &ctx, &src, scope, "ideas/Deep").unwrap_err();
            assert_eq!(refused, format!("move: no folder {label}/ideas/Deep"));
        }
        let folder = personal.join("ideas");
        assert_eq!(
            move_plan(&forest, &ctx, &folder, Scope::Personal, "Plans").unwrap_err(),
            "move: no folder personal/Plans"
        );
        #[cfg(unix)]
        {
            let src = personal.join("c.md");
            for typed in ["link/sub", "link"] {
                let refused = move_plan(&forest, &ctx, &src, Scope::Personal, typed).unwrap_err();
                assert_eq!(
                    refused,
                    format!("move: personal/{typed} is not a plain folder")
                );
            }
        }
        assert_eq!(disk_entries(dir.path()), before, "nothing moved");
        assert_eq!(
            final_maps(&forest, &[], &[], &roots),
            tags_before,
            "no tag changes"
        );
    }

    /// A hidden tagged note the tree does not list keeps its tag when its
    /// folder moves twice: out to `plans/` and back to the root, within a
    /// scope and through another scope.
    #[test]
    fn a_hidden_note_keeps_its_tag_when_its_folder_moves_out_and_back() {
        for (to, to_name) in [(Scope::Personal, "personal"), (Scope::Public, "public")] {
            let (_dir, notez, roots) = folder_fixture();
            let rebuild = || Ok(move_sections(&roots));
            let ctx = move_ctx(&roots);
            let mut forest = forest_of(move_sections(&roots));
            let (mut retired, mut carried) = (Vec::new(), Vec::new());
            let personal = store_of(&roots, Scope::Personal);
            let away = store_of(&roots, to).join("plans/ideas");
            let away_tags = move_tag_root(&roots, to);

            let plan = move_plan(&forest, &ctx, &personal.join("ideas"), to, "plans").unwrap();
            apply_move(
                &mut forest,
                &mut retired,
                &mut carried,
                &plan,
                &ctx.new_note_roots,
                &rebuild,
            );
            assert!(away.join(".hidden/h.md").is_file(), "{to_name}");
            let plan = move_plan(&forest, &ctx, &away, Scope::Personal, "").unwrap();
            apply_move(
                &mut forest,
                &mut retired,
                &mut carried,
                &plan,
                &ctx.new_note_roots,
                &rebuild,
            );
            assert!(personal.join("ideas/.hidden/h.md").is_file(), "{to_name}");

            let finals = final_maps(&forest, &retired, &carried, &roots);
            assert_eq!(
                finals[&notez].get("personal/proj/ideas/.hidden/h.md"),
                Some(&6),
                "{to_name}"
            );
            assert_eq!(
                finals[&notez].get("personal/proj/ideas/a.md"),
                Some(&1),
                "{to_name}"
            );
            let away_key = rel_key(&away_tags, &away.join(".hidden/h.md")).unwrap();
            assert_eq!(
                finals[&away_tags].get(&away_key),
                None,
                "{to_name}: no key at the intermediate path"
            );
            assert!(
                carried.iter().all(|(_, path, _)| path.exists()),
                "{to_name}: carried paths are current"
            );
        }
    }

    /// `S`, `Tab` to each other scope `NewNoteRoots` offers, `Enter`: a note
    /// and a folder move to the same folder there, with their tags
    /// (including an edit not yet saved). Typing does not change the folder.
    #[test]
    fn s_tab_enter_moves_a_note_and_a_folder_across_every_scope_pair_keeping_the_folder() {
        for (from, from_name) in SCOPES {
            for (to, to_name) in SCOPES {
                if from == to {
                    continue;
                }
                let pair = format!("{from_name} to {to_name}");
                let (_dir, _notez, roots) = move_fixture();
                let (src_store, dst_store) = (store_of(&roots, from), store_of(&roots, to));
                std::fs::create_dir_all(src_store.join("ideas/sub")).unwrap();
                std::fs::write(src_store.join("ideas/s.md"), "# s\n").unwrap();
                std::fs::write(src_store.join("ideas/sub/x.md"), "# x\n").unwrap();
                let rebuild = || Ok(move_sections(&roots));
                let ctx = move_ctx(&roots);
                let mut forest = forest_of(move_sections(&roots));
                let (mut retired, mut carried) = (Vec::new(), Vec::new());
                let (src_tags, dst_tags) = (move_tag_root(&roots, from), move_tag_root(&roots, to));
                for (file, flags) in [("ideas/s.md", 7), ("ideas/sub/x.md", 3)] {
                    let i = row(&forest.nodes, path_str(&src_store.join(file)));
                    forest.nodes[i].flags = flags;
                }
                let initial = final_maps(&forest, &[], &[], &roots);

                for (name, is_folder) in [("s.md", false), ("sub", true)] {
                    let src = src_store.join("ideas").join(name);
                    let i = row(&forest.nodes, path_str(&src));
                    let mut prompt =
                        set_scope_request(&forest.nodes, &forest.sections, Some(i), &ctx).unwrap();
                    assert!(prompt.fixed && prompt.scope.buffer == "ideas", "{pair}");
                    for code in [KeyCode::Char('z'), KeyCode::Backspace, KeyCode::Backspace] {
                        type_into_move(&mut prompt, code);
                    }
                    assert_eq!(prompt.scope.buffer, "ideas", "{pair}: typing does nothing");
                    assert_eq!(
                        resolve_enter(&prompt, &ctx.new_note_roots),
                        Ok(None),
                        "{pair}: same scope"
                    );
                    tab_to(&mut prompt, to, &ctx);
                    let plan = resolve_enter(&prompt, &ctx.new_note_roots)
                        .unwrap()
                        .expect("a move");
                    assert!(plan.needs_confirm(), "{pair}");
                    let dst = dst_store.join("ideas").join(name);
                    assert_eq!(plan.dst, dst, "{pair}");
                    if is_folder {
                        assert!(
                            plan.question()
                                .starts_with("move ideas/sub/ and its 1 note to "),
                            "{}",
                            plan.question()
                        );
                    }
                    let outcome = apply_move(
                        &mut forest,
                        &mut retired,
                        &mut carried,
                        &plan,
                        &ctx.new_note_roots,
                        &rebuild,
                    );
                    assert!(!src.exists() && dst.exists(), "{pair} {name}");
                    assert_eq!(
                        forest.nodes[outcome.row.expect("listed")].path,
                        dst,
                        "{pair} {name}"
                    );
                }

                let mut expected = initial.clone();
                for (file, flags) in [("ideas/s.md", 7), ("ideas/sub/x.md", 3)] {
                    expected
                        .get_mut(&src_tags)
                        .unwrap()
                        .remove(&rel_key(&src_tags, &src_store.join(file)).unwrap());
                    expected
                        .get_mut(&dst_tags)
                        .unwrap()
                        .insert(rel_key(&dst_tags, &dst_store.join(file)).unwrap(), flags);
                }
                assert_eq!(
                    final_maps(&forest, &retired, &carried, &roots),
                    expected,
                    "{pair}"
                );
            }
        }
    }

    #[test]
    fn s_on_the_original_scope_does_nothing_and_a_missing_folder_or_taken_name_refuses() {
        let (dir, _notez, roots) = move_fixture();
        let ctx = move_ctx(&roots);
        let personal = store_of(&roots, Scope::Personal);
        std::fs::create_dir_all(personal.join("only/f")).unwrap();
        std::fs::write(personal.join("only/z.md"), "# z\n").unwrap();
        let forest = forest_of(move_sections(&roots));
        let before = disk_entries(dir.path());
        let open = |path: &Path| {
            set_scope_request(
                &forest.nodes,
                &forest.sections,
                Some(row(&forest.nodes, path_str(path))),
                &ctx,
            )
            .unwrap()
        };

        let mut prompt = open(&personal.join("ideas/a.md"));
        assert_eq!(
            text_of(&Line::from(set_scope_lead(&prompt))),
            " set scope of a.md: personal (Tab cycles, Enter applies)"
        );
        assert_eq!(resolve_enter(&prompt, &ctx.new_note_roots), Ok(None));
        for _ in 0..4 {
            let p = &prompt.scope;
            prompt.scope.target = next_scope_target(
                &p.target,
                &p.origin,
                p.project.as_deref(),
                &ctx.new_note_roots,
                Some("proj"),
            );
        }
        assert_eq!(
            resolve_enter(&prompt, &ctx.new_note_roots),
            Ok(None),
            "a full cycle is back at the start"
        );

        let refused = |path: &Path, scope: Scope| {
            let mut prompt = open(path);
            tab_to(&mut prompt, scope, &ctx);
            resolve_enter(&prompt, &ctx.new_note_roots).unwrap_err()
        };
        let public = "public (committed with the project)";
        assert_eq!(
            refused(&personal.join("ideas/a.md"), Scope::Public),
            format!("set scope: a.md already exists in {public}/ideas")
        );
        assert_eq!(
            refused(&personal.join("ideas"), Scope::Local),
            "set scope: ideas already exists in local scratch"
        );
        assert_eq!(
            refused(&personal.join("only/z.md"), Scope::Public),
            format!("set scope: no folder {public}/only")
        );
        assert_eq!(
            refused(&personal.join("only/f"), Scope::Global),
            "set scope: no folder global/only"
        );
        assert_eq!(
            disk_entries(dir.path()),
            before,
            "nothing moved, no folder created"
        );

        let rendered = text_of(&lead_with_hints(
            set_scope_lead(&prompt),
            Mode::SetScope,
            &[],
            200,
        ));
        assert!(
            rendered.starts_with(" set scope of a.md: personal (Tab cycles"),
            "{rendered}"
        );
        assert!(
            rendered.contains("enter apply")
                && rendered.contains("esc cancel")
                && rendered.contains("tab scope"),
            "{rendered}"
        );
        assert!(!rendered.contains("bksp"), "{rendered}");
    }

    #[test]
    fn s_is_a_browse_key_after_m_with_its_prompt_keys() {
        let rows: Vec<(usize, &KeyHint)> = TREE_KEYS
            .iter()
            .enumerate()
            .filter(|(_, k)| k.key == "S")
            .collect();
        assert_eq!(rows.len(), 1);
        let (idx, hint) = rows[0];
        assert_eq!(hint.modes, BROWSE);
        assert_eq!(hint.group, Group::Edit);
        assert_eq!(hint.help, "set scope");
        assert_eq!(hint.slot, Slot::Priority(10));
        assert_eq!(
            idx,
            TREE_KEYS.iter().position(|k| k.key == "m").unwrap() + 1
        );
        assert!(help::rows(TREE_KEYS).contains(&help::Row::Key(idx)));
        assert!(shown_keys(Mode::Normal, &[], 200).contains(&"S"));
        assert!(shown_keys(Mode::Focus, &[], 200).contains(&"S"));
        for mode in [
            Mode::Tag,
            Mode::Filter,
            Mode::Rename,
            Mode::NewItem,
            Mode::ConfirmDelete,
            Mode::VimCommand,
            Mode::Move,
            Mode::SetScope,
            Mode::ConfirmMove,
        ] {
            assert!(!shown_keys(mode, &[], 200).contains(&"S"), "{mode:?}");
            assert!(!shown_keys(mode, &[], 200).contains(&"m"), "{mode:?}");
        }
        let keys_in = |mode: Mode| {
            TREE_KEYS
                .iter()
                .filter(|k| k.modes.contains(&mode))
                .map(|k| k.key)
                .collect::<Vec<_>>()
        };
        assert_eq!(keys_in(Mode::SetScope), vec!["enter", "esc", "tab"]);
        let with_m = (0..200)
            .find(|&w| shown_keys(Mode::Normal, &[], w).contains(&"m"))
            .unwrap();
        let with_s = (0..200)
            .find(|&w| shown_keys(Mode::Normal, &[], w).contains(&"S"))
            .unwrap();
        assert!(with_s > with_m, "S drops before m");
    }
}
