//! The delete prompt (`d`) for notes and folders, and the guards that
//! keep the todo board's store to the todo view.

use super::*;

// --- The todo board's store ---
//
// `<notez root>/_todos` holds the todo board's lists (`notez_core::todo`
// reads it). The browser lists it in the global section but leaves it to
// the todo view: `d`, `r`, `m` and `S` refuse it and every row under it,
// and it is never a move destination. `n` and `N` there work as anywhere.

/// The store's folder name under the notez root.
const TODO_STORE: &str = "_todos";

/// What the refusals below say after their verb.
pub(super) const TODO_STORE_MANAGED: &str = "the todo board's store is managed by the todo view";

/// The footer message for `d` on the store or a row under it.
const TODOS_DELETE: &str = "delete: the todo board's store is managed by the todo view";

/// The footer message for `r` on the store or a row under it.
pub(super) const TODOS_RENAME: &str = "rename: the todo board's store is managed by the todo view";

/// The footer message for `m` on the store or a row under it.
pub(super) const TODOS_MOVE: &str = "move: the todo board's store is managed by the todo view";

/// The footer message for `S` on the store or a row under it.
pub(super) const TODOS_SET_SCOPE: &str =
    "set scope: the todo board's store is managed by the todo view";

/// Whether `path` is `<notez_root>/_todos` or lies under it. The first step
/// below the root counts as the store when it is spelled `_todos` or is the
/// same directory entry the board reads as `_todos` (another spelling on a
/// case-insensitive file system). A sibling such as `_todos-archive` is not
/// the store.
pub(super) fn in_todo_store(path: &Path, notez_root: &Path) -> bool {
    let Ok(rel) = path.strip_prefix(notez_root) else {
        return false;
    };
    let Some(first) = rel.components().next() else {
        return false;
    };
    let top = notez_root.join(first);
    let store = notez_root.join(TODO_STORE);
    top == store || is_same_entry(&top, &store)
}

/// Whether `node` is the todo board's store or a row under it. Only the
/// global section's root is the notez root (`commands::tree` gives the
/// global section `notez_root` itself), so a `_todos` folder in a project's
/// store is an ordinary folder.
pub(super) fn is_todo_row(node: &TreeNode, sections: &[SectionSpec]) -> bool {
    sections
        .get(node.section)
        .is_some_and(|spec| in_section_todo_store(node, spec))
}

/// Whether `node`, a row of `spec`, is the todo board's store or a row
/// under it; see [`is_todo_row`].
pub(super) fn in_section_todo_store(node: &TreeNode, spec: &SectionSpec) -> bool {
    spec.scope == Scope::Global
        && spec.project.is_none()
        && !spec.is_doc
        && in_todo_store(&node.path, &spec.root)
}

// --- Delete ---

/// The footer message for `d` on a section row.
const SECTION_DELETE: &str = "delete: a section cannot be deleted";

/// The footer message for `d` on a folder in a docs section: those are the
/// repository's own files, which the browser does not remove wholesale.
const DOCS_FOLDER_DELETE: &str = "delete: not for folders in a docs section";

/// The footer message for `d` on a folder that holds another section.
pub(super) const FOLDER_HOLDS_SECTION: &str = "delete: this folder holds another section";

/// The open delete confirmation for one note or folder: its path, its path
/// when the session started (a rename moves `path` only), the tag root its
/// `.tags` keys live under, its path relative to its section's root, and
/// the scope it is deleted from with that scope's label. For a folder,
/// `folder` holds what the question counts, and `section_root` with
/// `section_roots` is what [`remove_folder`] checks before removing.
#[derive(Debug, Clone)]
pub(super) struct DeletePrompt {
    path: PathBuf,
    origin: PathBuf,
    tag_root: PathBuf,
    rel: String,
    scope: Scope,
    label: String,
    pub(super) folder: Option<FolderContents>,
    section_root: PathBuf,
    /// The root of every section in the view.
    section_roots: Vec<PathBuf>,
}

/// What a folder holds, for the delete question: the markdown notes the
/// tree lists (regular `.md` files, not hidden, at any depth) and whether
/// anything else would go with them (other files, hidden entries,
/// symlinks). Folders themselves are not counted.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct FolderContents {
    pub(super) notes: usize,
    pub(super) has_other_files: bool,
}

/// Count what `dir` holds (see [`FolderContents`]). Symlinks are not
/// followed, like the aggregator's walk; an unreadable directory adds
/// nothing.
pub(super) fn folder_contents(dir: &Path) -> FolderContents {
    let mut contents = FolderContents::default();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return contents;
    };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else {
            contents.has_other_files = true;
            continue;
        };
        let path = entry.path();
        let hidden = entry.file_name().to_string_lossy().starts_with('.');
        let is_note = path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("md"));
        if hidden {
            contents.has_other_files = true;
        } else if kind.is_dir() {
            let below = folder_contents(&path);
            contents.notes += below.notes;
            contents.has_other_files |= below.has_other_files;
        } else if kind.is_file() && is_note {
            contents.notes += 1;
        } else {
            contents.has_other_files = true;
        }
    }
    contents
}

/// Whether a folder delete or rename may touch `path`: it lies strictly
/// inside `section_root` through plain names only (no `.` or `..` step),
/// and it is not, and does not hold, the root of any section in
/// `section_roots`. This keeps a section root and everything outside the
/// row's section out of reach.
pub(super) fn folder_change_allowed(
    path: &Path,
    section_root: &Path,
    section_roots: &[PathBuf],
) -> bool {
    let Ok(rel) = path.strip_prefix(section_root) else {
        return false;
    };
    let mut steps = rel.components().peekable();
    if steps.peek().is_none() || !steps.all(|c| matches!(c, std::path::Component::Normal(_))) {
        return false;
    }
    !section_roots.iter().any(|root| root.starts_with(path))
}

/// Remove the folder at `path` with everything in it, after
/// [`folder_change_allowed`] passes; otherwise nothing is touched. The only
/// place the browser calls `remove_dir_all`.
fn remove_folder(
    path: &Path,
    section_root: &Path,
    section_roots: &[PathBuf],
) -> std::io::Result<()> {
    if !folder_change_allowed(path, section_root, section_roots) {
        return Err(std::io::Error::other(format!(
            "{} is not a folder inside its section",
            path.display()
        )));
    }
    std::fs::remove_dir_all(path)
}

/// What `d` does with the cursor on `row`: `Ok(Some)` opens the prompt on a
/// note or a folder, `Err` is the footer message for a section row, a row
/// in the todo board's store ([`is_todo_row`]), a docs folder, or a folder
/// holding another section, and `Ok(None)` (no row
/// under the cursor) does nothing. A folder's contents are counted from
/// the disk when the prompt opens.
pub(super) fn delete_request(
    nodes: &[TreeNode],
    sections: &[SectionSpec],
    row: Option<usize>,
    current_project: Option<&str>,
) -> std::result::Result<Option<DeletePrompt>, &'static str> {
    let Some(node) = row.and_then(|i| nodes.get(i)) else {
        return Ok(None);
    };
    if node.depth == 0 {
        return Err(SECTION_DELETE);
    }
    let Some(spec) = sections.get(node.section) else {
        return Ok(None);
    };
    if is_todo_row(node, sections) {
        return Err(TODOS_DELETE);
    }
    let section_roots: Vec<PathBuf> = sections.iter().map(|s| s.root.clone()).collect();
    let folder = if node.is_dir {
        if spec.is_doc {
            return Err(DOCS_FOLDER_DELETE);
        }
        if !folder_change_allowed(&node.path, &spec.root, &section_roots) {
            return Err(FOLDER_HOLDS_SECTION);
        }
        Some(folder_contents(&node.path))
    } else {
        None
    };
    let rel = node.path.strip_prefix(&spec.root).unwrap_or(&node.path);
    Ok(Some(DeletePrompt {
        path: node.path.clone(),
        origin: node.origin.clone(),
        tag_root: spec.tag_root.clone(),
        rel: rel.to_string_lossy().into_owned(),
        scope: spec.scope,
        label: scope_label(spec.scope, spec.project.as_deref(), current_project),
        folder,
        section_root: spec.root.clone(),
        section_roots,
    }))
}

/// The confirmation question. For a folder it names what goes with it: `delete
/// ideas/ and its 2 notes from personal? y/n` (`1 note`, `no notes`), with
/// `and other files` when it holds anything but notes. Local scratch is not
/// in any repository, so the question says a delete there cannot be undone.
fn delete_question(prompt: &DeletePrompt) -> String {
    let warning = if prompt.scope == Scope::Local {
        " (not recoverable)"
    } else {
        ""
    };
    let what = match prompt.folder {
        None => prompt.rel.clone(),
        Some(contents) => {
            let notes = match contents.notes {
                0 => " (no notes)".to_string(),
                1 => " and its 1 note".to_string(),
                n => format!(" and its {n} notes"),
            };
            let other = if contents.has_other_files {
                " and other files"
            } else {
                ""
            };
            format!("{}/{notes}{other}", prompt.rel)
        }
    };
    format!("delete {what} from {}?{warning} y/n", prompt.label)
}

/// The delete confirmation that leads the footer while it is open.
pub(super) fn delete_lead(prompt: &DeletePrompt) -> Vec<Span<'static>> {
    vec![Span::styled(
        format!(" {}", delete_question(prompt)),
        Style::default().fg(theme::PEACH),
    )]
}

/// What a confirmed delete leaves: the row for the cursor and the footer
/// message.
#[derive(Debug)]
pub(super) struct DeleteOutcome {
    pub(super) row: Option<usize>,
    pub(super) message: String,
    /// Whether `rebuild` listed the forest again (not the fallback).
    pub(super) relisted: bool,
}

/// Answer the open delete prompt with `key`. Anything but `y` cancels and
/// returns `None` without touching the disk or the forest. On `y` the note
/// is removed (or the folder, through [`remove_folder`]), the `.tags` keys
/// of what went are added to `retired` (see [`retire_folder_keys`] for a
/// folder; a note's current and original path), and the forest is rebuilt
/// from `rebuild` whether or not the removal worked, so the tree matches
/// the disk. If `rebuild` fails, the current rows minus what went are used
/// instead.
pub(super) fn answer_delete(
    key: KeyCode,
    forest: &mut Forest,
    retired: &mut Vec<(PathBuf, String)>,
    prompt: &DeletePrompt,
    search: &str,
    rebuild: &dyn Fn() -> Result<Vec<SectionSpec>>,
) -> Option<DeleteOutcome> {
    if key != KeyCode::Char('y') {
        return None;
    }
    let is_folder = prompt.folder.is_some();
    let removed = remove_and_retire(forest, retired, prompt);
    let mut message = match &removed {
        Ok(()) if is_folder => format!("deleted {}/", prompt.rel),
        Ok(()) => format!("deleted {}", prompt.rel),
        Err(e) => format!("delete failed: {e}"),
    };
    let mut relisted = true;
    let sections = rebuild().unwrap_or_else(|e| {
        relisted = false;
        message = format!("{message}, but the list could not be refreshed: {e:#}");
        if is_folder {
            let gone =
                |p: &Path| p.starts_with(&prompt.path) && std::fs::symlink_metadata(p).is_err();
            current_sections_without(forest, &gone)
        } else {
            let gone = |p: &Path| removed.is_ok() && p == prompt.path;
            current_sections_without(forest, &gone)
        }
    });
    let row = forest.rebuild_after_delete(sections, &prompt.path, search);
    Some(DeleteOutcome {
        row,
        message,
        relisted,
    })
}

/// Remove what `prompt` names, a note or (through [`remove_folder`]) a
/// folder, and add the `.tags` keys of what went to `retired`: a note's
/// current and original key once it is gone, a folder's keys through
/// [`retire_folder_keys`] whenever its removal was allowed to start. The
/// forest is not rebuilt; the caller does that once.
pub(super) fn remove_and_retire(
    forest: &Forest,
    retired: &mut Vec<(PathBuf, String)>,
    prompt: &DeletePrompt,
) -> std::io::Result<()> {
    let is_folder = prompt.folder.is_some();
    let removed = if is_folder {
        remove_folder(&prompt.path, &prompt.section_root, &prompt.section_roots)
    } else {
        std::fs::remove_file(&prompt.path)
    };
    // A refused folder delete touched nothing, so nothing is retired.
    if is_folder && folder_change_allowed(&prompt.path, &prompt.section_root, &prompt.section_roots)
    {
        retire_folder_keys(forest, prompt, retired);
    }
    if removed.is_ok() && !is_folder {
        for path in [&prompt.path, &prompt.origin] {
            if let Some(tag_key) = rel_key(&prompt.tag_root, path) {
                retire(retired, &prompt.tag_root, tag_key);
            }
        }
    }
    removed
}

/// Add `(tag_root, key)` to `retired` unless it is listed already.
pub(super) fn retire(retired: &mut Vec<(PathBuf, String)>, tag_root: &Path, key: String) {
    if !retired.iter().any(|(r, k)| r == tag_root && *k == key) {
        retired.push((tag_root.to_path_buf(), key));
    }
}

/// Retire the `.tags` keys of every note a folder delete removed, keys
/// relative to the section's tag root: for each note row under the folder
/// that is gone from the disk, its current and original key, and every key
/// of the session's loaded map under the folder whose file is gone (notes
/// tagged on disk but not listed). Called after the removal, so a delete
/// that failed midway retires exactly the notes that went.
fn retire_folder_keys(
    forest: &Forest,
    prompt: &DeletePrompt,
    retired: &mut Vec<(PathBuf, String)>,
) {
    let tag_root = &prompt.tag_root;
    let gone = |p: &Path| std::fs::symlink_metadata(p).is_err();
    for node in &forest.nodes {
        if node.is_dir || !node.path.starts_with(&prompt.path) || !gone(&node.path) {
            continue;
        }
        for path in [&node.path, &node.origin] {
            if let Some(key) = rel_key(tag_root, path) {
                retire(retired, tag_root, key);
            }
        }
    }
    let Some(folder_key) = rel_key(tag_root, &prompt.path) else {
        return;
    };
    let Some(i) = forest.tag_roots.iter().position(|r| r == tag_root) else {
        return;
    };
    let mut keys: Vec<&String> = forest.initial[i]
        .keys()
        .filter(|k| {
            Path::new(k.as_str()).starts_with(&folder_key) && gone(&tag_root.join(k.as_str()))
        })
        .collect();
    keys.sort();
    for key in keys {
        retire(retired, tag_root, key.clone());
    }
}

/// The forest's sections listing its current file rows and their listed
/// folders, minus the paths `gone` matches: the fallback listing when the
/// rebuild closure fails. Takes the sections out of `forest`; the caller
/// puts a rebuilt set back.
pub(super) fn current_sections_without(
    forest: &mut Forest,
    gone: &dyn Fn(&Path) -> bool,
) -> Vec<SectionSpec> {
    let mut sections = std::mem::take(&mut forest.sections);
    for (i, spec) in sections.iter_mut().enumerate() {
        spec.files = forest
            .nodes
            .iter()
            .filter(|n| !n.is_dir && n.section == i && !gone(&n.path))
            .map(|n| n.path.clone())
            .collect();
        spec.dirs.retain(|d| !gone(d));
    }
    sections
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use super::*;
    use notez_core::tags::{FLAG_IMPORTANT, FLAG_PRIO};

    // --- Delete ---

    #[test]
    fn d_then_y_deletes_exactly_that_note_in_every_scope() {
        for (scope, name) in SCOPES {
            let (_dir, roots) = temp_tree();
            let rebuild = || Ok(list_sections(&roots));
            let mut forest = forest_of(list_sections(&roots));
            let mut retired = Vec::new();
            let target = roots
                .iter()
                .find(|(s, _)| *s == scope)
                .unwrap()
                .1
                .join("ideas/a.md");
            let before = all_files(&roots);

            let prompt = prompt_at(&mut forest, &target);
            let outcome = answer_delete(
                KeyCode::Char('y'),
                &mut forest,
                &mut retired,
                &prompt,
                "",
                &rebuild,
            )
            .expect("y confirms");

            let expected: Vec<PathBuf> = before.into_iter().filter(|p| *p != target).collect();
            assert_eq!(all_files(&roots), expected, "{name}");
            assert!(
                !forest.nodes.iter().any(|n| n.path == target),
                "{name}: row gone"
            );
            assert_eq!(outcome.message, "deleted ideas/a.md", "{name}");
        }
    }

    #[test]
    fn any_answer_but_y_deletes_nothing() {
        let (_dir, roots) = temp_tree();
        let rebuild = || Ok(list_sections(&roots));
        let mut forest = forest_of(list_sections(&roots));
        let mut retired = Vec::new();
        let target = roots[0].1.join("ideas/a.md");
        let prompt = prompt_at(&mut forest, &target);
        let before = all_files(&roots);
        let rows = forest.nodes.len();
        for code in [
            KeyCode::Char('n'),
            KeyCode::Esc,
            KeyCode::Char('x'),
            KeyCode::Enter,
            KeyCode::Char('Y'),
            KeyCode::Char('d'),
        ] {
            assert!(
                answer_delete(code, &mut forest, &mut retired, &prompt, "", &rebuild).is_none(),
                "{code:?}"
            );
            assert_eq!(all_files(&roots), before, "{code:?}");
            assert_eq!(forest.nodes.len(), rows, "{code:?}");
        }
        assert!(retired.is_empty());
    }

    #[test]
    fn delete_prompt_names_the_file_and_the_scope() {
        let sections = project_sections();
        let (nodes, _) = build_forest(&sections);
        let question = |path: &str, current: Option<&str>| {
            let p = delete_request(&nodes, &sections, Some(row(&nodes, path)), current)
                .unwrap()
                .unwrap();
            delete_question(&p)
        };
        assert_eq!(
            question("/n/personal/proj/ideas/a.md", Some("proj")),
            "delete ideas/a.md from personal? y/n"
        );
        let public = question("/p/notez/plans/b.md", Some("proj"));
        assert_eq!(
            public,
            "delete plans/b.md from public (committed with the project)? y/n"
        );
        assert!(question("/p/notez/plans/b.md", None).contains("public"));
        assert!(question("/p/docs/design/c.md", Some("proj")).contains("public"));
        let local = question("/p/.notez/d.md", Some("proj"));
        assert!(
            local.starts_with("delete d.md from local scratch?"),
            "{local}"
        );
        assert!(local.contains("not recoverable"), "{local}");
        assert_eq!(
            question("/n/e.md", Some("proj")),
            "delete e.md from global? y/n"
        );
        assert_eq!(
            question("/n/personal/proj/top.md", None),
            "delete top.md from personal (proj)? y/n"
        );
        for path in ["/n/personal/proj/top.md", "/p/notez/plans/b.md", "/n/e.md"] {
            assert!(
                !question(path, Some("proj")).contains("not recoverable"),
                "{path}"
            );
        }
    }

    #[test]
    fn d_on_a_section_a_docs_folder_or_an_empty_tree_changes_nothing() {
        let sections = project_sections();
        let (nodes, _) = build_forest(&sections);
        for path in ["/n/personal/proj", "/p/docs", "/p/.notez", "/n"] {
            let request = delete_request(&nodes, &sections, Some(row(&nodes, path)), None);
            assert_eq!(request.err(), Some(SECTION_DELETE), "{path}");
        }
        let docs = delete_request(&nodes, &sections, Some(row(&nodes, "/p/docs/design")), None);
        assert_eq!(docs.err(), Some(DOCS_FOLDER_DELETE));
        let folder = delete_request(
            &nodes,
            &sections,
            Some(row(&nodes, "/n/personal/proj/ideas")),
            None,
        );
        assert!(
            folder.unwrap().unwrap().folder.is_some(),
            "a note folder opens the folder prompt"
        );
        assert!(matches!(
            delete_request(&nodes, &sections, None, None),
            Ok(None)
        ));
        assert!(matches!(
            delete_request(&nodes, &sections, Some(nodes.len()), None),
            Ok(None)
        ));
        assert!(matches!(delete_request(&[], &[], Some(0), None), Ok(None)));
        assert!(matches!(delete_request(&[], &[], None, None), Ok(None)));
    }

    fn delete_and_select(
        before: &[&str],
        after: &[&str],
        deleted: &str,
        expand: &[&str],
        search: &str,
    ) -> (Forest, Option<usize>) {
        let (mut nodes, roots) = build_forest(&[spec("/r", "S", before)]);
        for path in expand {
            let i = row(&nodes, path);
            nodes[i].expanded = true;
        }
        let initial = vec![HashMap::new(); roots.len()];
        let mut forest = Forest {
            sections: vec![spec("/r", "S", before)],
            nodes,
            tag_roots: roots,
            initial,
        };
        let selected =
            forest.rebuild_after_delete(vec![spec("/r", "S", after)], Path::new(deleted), search);
        (forest, selected)
    }

    #[test]
    fn delete_selects_the_next_note_then_the_previous_then_the_folder() {
        let open = ["/r", "/r/ideas"];
        let files = ["ideas/a.md", "ideas/b.md", "ideas/c.md", "z.md"];

        let (f, sel) = delete_and_select(
            &files,
            &["ideas/a.md", "ideas/c.md", "z.md"],
            "/r/ideas/b.md",
            &open,
            "",
        );
        assert_eq!(
            sel,
            Some(row(&f.nodes, "/r/ideas/c.md")),
            "the row that followed"
        );

        let (f, sel) = delete_and_select(
            &files,
            &["ideas/a.md", "ideas/b.md", "z.md"],
            "/r/ideas/c.md",
            &open,
            "",
        );
        assert_eq!(
            sel,
            Some(row(&f.nodes, "/r/ideas/b.md")),
            "last in its folder: the one before"
        );

        let (f, sel) = delete_and_select(
            &["ideas/sub/x.md", "ideas/only.md"],
            &["ideas/sub/x.md"],
            "/r/ideas/only.md",
            &open,
            "",
        );
        assert_eq!(
            sel,
            Some(row(&f.nodes, "/r/ideas/sub")),
            "the one before can be a folder"
        );

        let (f, sel) = delete_and_select(
            &["ideas/a.md", "z.md"],
            &["z.md"],
            "/r/ideas/a.md",
            &open,
            "",
        );
        assert_eq!(
            sel,
            Some(row(&f.nodes, "/r")),
            "an emptied folder leaves the tree, so its nearest listed ancestor is selected"
        );

        let (f, sel) = delete_and_select(&["ideas/a.md"], &[], "/r/ideas/a.md", &open, "");
        assert!(f.nodes.is_empty());
        assert_eq!(sel, None, "nothing left to select");
    }

    #[test]
    fn delete_keeps_expansion_unsaved_tags_and_the_filter() {
        let before = [
            "ideas/a.md",
            "ideas/b.md",
            "ideas/c-a.md",
            "plans/p.md",
            "shut/s.md",
            "z.md",
        ];
        let (mut nodes, roots) = build_forest(&[spec("/r", "S", &before)]);
        for path in ["/r", "/r/ideas", "/r/plans"] {
            let i = row(&nodes, path);
            nodes[i].expanded = true;
        }
        let p = row(&nodes, "/r/plans/p.md");
        nodes[p].flags = FLAG_PRIO;
        let z = row(&nodes, "/r/z.md");
        nodes[z].origin = PathBuf::from("/r/old-z.md");
        let initial = vec![HashMap::new()];
        let mut forest = Forest {
            sections: vec![spec("/r", "S", &before)],
            nodes,
            tag_roots: roots,
            initial,
        };

        let after = [
            "ideas/b.md",
            "ideas/c-a.md",
            "plans/p.md",
            "shut/s.md",
            "z.md",
        ];
        // The filter "a" hides b.md, the row that followed a.md, so the
        // cursor goes on to the next row it shows.
        let search = "a";
        let sel = forest.rebuild_after_delete(
            vec![spec("/r", "S", &after)],
            Path::new("/r/ideas/a.md"),
            search,
        );

        let n = &forest.nodes;
        assert!(!compute_visible(n, search).contains(&row(n, "/r/ideas/b.md")));
        assert_eq!(sel, Some(row(n, "/r/ideas/c-a.md")));
        assert!(compute_visible(n, search).contains(&sel.unwrap()));
        assert!(
            n[row(n, "/r")].expanded
                && n[row(n, "/r/ideas")].expanded
                && n[row(n, "/r/plans")].expanded
        );
        assert!(
            !n[row(n, "/r/shut")].expanded,
            "a closed folder stays closed"
        );
        assert_eq!(
            n[row(n, "/r/plans/p.md")].flags,
            FLAG_PRIO,
            "unsaved tag edits survive"
        );
        assert_eq!(n[row(n, "/r/z.md")].origin, PathBuf::from("/r/old-z.md"));
    }

    #[test]
    fn retired_keys_leave_the_tags_map_and_nothing_else_changes() {
        let s = spec("/r", "S", &["b.md"]);
        let (mut nodes, roots) = build_forest(&[s]);
        let initial = vec![HashMap::from([
            ("a.md".to_string(), FLAG_PRIO),
            ("b.md".to_string(), FLAG_IMPORTANT),
            ("elsewhere/hidden.md".to_string(), FLAG_IMPORTANT),
        ])];
        apply_tags(&mut nodes, &roots, &initial);
        let retired = vec![(PathBuf::from("/r"), "a.md".to_string())];

        let changed = changed_tag_maps_retiring(&nodes, &roots, &initial, &retired);
        assert_eq!(
            changed,
            vec![(
                PathBuf::from("/r"),
                HashMap::from([
                    ("b.md".to_string(), FLAG_IMPORTANT),
                    ("elsewhere/hidden.md".to_string(), FLAG_IMPORTANT),
                ])
            )]
        );

        let untagged = vec![(PathBuf::from("/r"), "never-tagged.md".to_string())];
        assert!(
            changed_tag_maps_retiring(&nodes, &roots, &initial, &untagged).is_empty(),
            "an untagged note's delete leaves .tags alone"
        );
        let unknown_root = vec![(PathBuf::from("/elsewhere"), "a.md".to_string())];
        assert!(changed_tag_maps_retiring(&nodes, &roots, &initial, &unknown_root).is_empty());
    }

    #[test]
    fn deleting_a_renamed_note_retires_its_original_key() {
        let (_dir, roots) = temp_tree();
        let root = roots[0].1.clone();
        std::fs::write(root.join(".tags"), "ideas/a.md:1\n").unwrap();
        let rebuild = || Ok(list_sections(&roots));
        let mut forest = forest_of(list_sections(&roots));
        let a = row(&forest.nodes, path_str(&root.join("ideas/a.md")));
        assert_ne!(forest.nodes[a].flags, 0, "the fixture's .tags loaded");
        // As a rename in this session leaves it: new path, old origin.
        std::fs::rename(root.join("ideas/a.md"), root.join("ideas/renamed.md")).unwrap();
        forest.nodes[a].path = root.join("ideas/renamed.md");
        let mut retired = Vec::new();

        let prompt = prompt_at(&mut forest, &root.join("ideas/renamed.md"));
        answer_delete(
            KeyCode::Char('y'),
            &mut forest,
            &mut retired,
            &prompt,
            "",
            &rebuild,
        )
        .unwrap();

        let changed =
            changed_tag_maps_retiring(&forest.nodes, &forest.tag_roots, &forest.initial, &retired);
        assert_eq!(changed, vec![(root.clone(), HashMap::new())]);
    }

    #[test]
    fn deleting_the_last_note_of_a_tag_root_still_retires_its_key() {
        let (_dir, roots) = temp_tree();
        let local = roots
            .iter()
            .find(|(s, _)| *s == Scope::Local)
            .unwrap()
            .1
            .clone();
        std::fs::remove_file(local.join("ideas/b.md")).unwrap();
        std::fs::remove_file(local.join("c.md")).unwrap();
        std::fs::write(local.join(".tags"), "ideas/a.md:1\nkept.md:2\n").unwrap();
        let rebuild = || Ok(list_sections(&roots));
        let mut forest = forest_of(list_sections(&roots));
        let mut retired = Vec::new();

        let prompt = prompt_at(&mut forest, &local.join("ideas/a.md"));
        answer_delete(
            KeyCode::Char('y'),
            &mut forest,
            &mut retired,
            &prompt,
            "",
            &rebuild,
        )
        .unwrap();

        assert!(
            !forest.nodes.iter().any(|n| n.path.starts_with(&local)),
            "the section is gone"
        );
        let changed =
            changed_tag_maps_retiring(&forest.nodes, &forest.tag_roots, &forest.initial, &retired);
        let initial_kept = note_tags::load_tags(&local).get("kept.md").copied();
        assert_eq!(
            changed,
            vec![(
                local.clone(),
                HashMap::from([("kept.md".to_string(), initial_kept.unwrap())])
            )]
        );
    }

    #[test]
    fn delete_of_a_note_already_gone_reports_and_rebuilds() {
        let (_dir, roots) = temp_tree();
        let rebuild = || Ok(list_sections(&roots));
        let mut forest = forest_of(list_sections(&roots));
        let mut retired = Vec::new();
        let target = roots[1].1.join("ideas/a.md");
        let prompt = prompt_at(&mut forest, &target);
        std::fs::remove_file(&target).unwrap();
        let before = all_files(&roots);

        let outcome = answer_delete(
            KeyCode::Char('y'),
            &mut forest,
            &mut retired,
            &prompt,
            "",
            &rebuild,
        )
        .unwrap();

        assert!(
            outcome.message.starts_with("delete failed: "),
            "{}",
            outcome.message
        );
        assert_eq!(all_files(&roots), before, "nothing else removed");
        assert!(
            !forest.nodes.iter().any(|n| n.path == target),
            "the display matches the disk"
        );
        assert!(retired.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn delete_without_permission_reports_and_keeps_the_note_selected() {
        use std::os::unix::fs::PermissionsExt;
        let (_dir, roots) = temp_tree();
        let ideas = roots[0].1.join("ideas");
        let rebuild = || Ok(list_sections(&roots));
        let mut forest = forest_of(list_sections(&roots));
        let mut retired = Vec::new();
        let target = ideas.join("a.md");
        let prompt = prompt_at(&mut forest, &target);
        let before = all_files(&roots);

        std::fs::set_permissions(&ideas, std::fs::Permissions::from_mode(0o555)).unwrap();
        let outcome = answer_delete(
            KeyCode::Char('y'),
            &mut forest,
            &mut retired,
            &prompt,
            "",
            &rebuild,
        );
        std::fs::set_permissions(&ideas, std::fs::Permissions::from_mode(0o755)).unwrap();
        let outcome = outcome.unwrap();

        assert!(
            outcome.message.starts_with("delete failed: "),
            "{}",
            outcome.message
        );
        assert_eq!(all_files(&roots), before);
        assert!(retired.is_empty());
        assert_eq!(outcome.row, Some(row(&forest.nodes, path_str(&target))));
    }

    #[test]
    fn a_failed_rebuild_still_drops_the_deleted_row() {
        let (_dir, roots) = temp_tree();
        let rebuild = || -> Result<Vec<SectionSpec>> { anyhow::bail!("listing broke") };
        let mut forest = forest_of(list_sections(&roots));
        let mut retired = Vec::new();
        let target = roots[0].1.join("ideas/a.md");
        let prompt = prompt_at(&mut forest, &target);

        let outcome = answer_delete(
            KeyCode::Char('y'),
            &mut forest,
            &mut retired,
            &prompt,
            "",
            &rebuild,
        )
        .unwrap();

        assert!(!target.exists());
        assert!(
            outcome.message.contains("listing broke"),
            "{}",
            outcome.message
        );
        assert!(!forest.nodes.iter().any(|n| n.path == target));
        assert_eq!(
            outcome.row,
            Some(row(&forest.nodes, path_str(&roots[0].1.join("ideas/b.md"))))
        );
    }

    #[test]
    fn delete_keys_are_in_the_table_with_their_modes() {
        let d: Vec<&KeyHint> = TREE_KEYS.iter().filter(|k| k.key == "d").collect();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].modes, BROWSE);
        assert_eq!(d[0].group, Group::Edit);
        assert_eq!(
            shown_keys(Mode::ConfirmDelete, &[], 200),
            vec!["y", "n/esc"]
        );
        let confirm: Vec<&str> = TREE_KEYS
            .iter()
            .filter(|k| k.modes.contains(&Mode::ConfirmDelete))
            .map(|k| k.key)
            .collect();
        assert_eq!(confirm, vec!["y", "n/esc"]);
        let sections = project_sections();
        let (nodes, _) = build_forest(&sections);
        let prompt = delete_request(&nodes, &sections, Some(row(&nodes, "/n/e.md")), None)
            .unwrap()
            .unwrap();
        let rendered = text_of(&lead_with_hints(
            delete_lead(&prompt),
            Mode::ConfirmDelete,
            &[],
            120,
        ));
        assert!(
            rendered.starts_with(" delete e.md from global? y/n"),
            "{rendered}"
        );
        assert!(
            rendered.contains("y confirm") && rendered.contains("n/esc cancel"),
            "{rendered}"
        );
        assert!(!rendered.contains("quit"));
    }

    /// The delete prompt `d` opens on the folder at `path`, whose parents
    /// are expanded as they are when the cursor can reach it.
    fn folder_prompt(forest: &mut Forest, path: &Path) -> DeletePrompt {
        let i = row(&forest.nodes, path_str(path));
        let mut up = forest.nodes[i].parent_idx;
        while let Some(p) = up {
            forest.nodes[p].expanded = true;
            up = forest.nodes[p].parent_idx;
        }
        delete_request(&forest.nodes, &forest.sections, Some(i), Some("proj"))
            .expect("a folder row")
            .expect("a row under the cursor")
    }

    #[test]
    fn d_then_y_on_a_folder_removes_it_and_nothing_else_and_retires_its_keys() {
        let (_dir, notez, root) = vault();
        let rebuild = || Ok(vault_sections(&notez));
        let mut forest = forest_of(vault_sections(&notez));
        let mut retired = Vec::new();
        let initial = note_tags::load_tags(&notez);
        let ideas = root.join("ideas");
        let disk: Vec<String> = disk_entries(&notez)
            .into_iter()
            .filter(|e| !Path::new(e).starts_with("personal/proj/ideas"))
            .collect();

        let prompt = folder_prompt(&mut forest, &ideas);
        assert_eq!(
            delete_question(&prompt),
            "delete ideas/ and its 2 notes from personal? y/n"
        );
        let outcome = answer_delete(
            KeyCode::Char('y'),
            &mut forest,
            &mut retired,
            &prompt,
            "",
            &rebuild,
        )
        .expect("y confirms");

        assert!(!ideas.exists());
        assert_eq!(disk_entries(&notez), disk, "only the folder went");
        assert_eq!(outcome.message, "deleted ideas/");
        assert!(!forest.nodes.iter().any(|n| n.path.starts_with(&ideas)));
        assert_eq!(
            outcome.row,
            Some(row(&forest.nodes, path_str(&root.join("plans")))),
            "the next sibling"
        );

        let changed =
            changed_tag_maps_retiring(&forest.nodes, &forest.tag_roots, &forest.initial, &retired);
        let mut expected = initial.clone();
        expected.retain(|k, _| !Path::new(k).starts_with("personal/proj/ideas"));
        assert!(expected.contains_key("personal/proj/ideas-other.md"));
        assert!(expected.contains_key("g.md") && expected.contains_key("personal/proj/plans/p.md"));
        assert_eq!(changed, vec![(notez.clone(), expected)]);
    }

    #[test]
    fn deleting_a_renamed_folder_retires_the_original_keys() {
        let (_dir, notez, root) = vault();
        let rebuild = || Ok(vault_sections(&notez));
        let mut forest = forest_of(vault_sections(&notez));
        let mut retired = Vec::new();
        rename_at(&mut forest, &root.join("ideas"), "thoughts").unwrap();

        let prompt = folder_prompt(&mut forest, &root.join("thoughts"));
        answer_delete(
            KeyCode::Char('y'),
            &mut forest,
            &mut retired,
            &prompt,
            "",
            &rebuild,
        )
        .unwrap();

        let changed =
            changed_tag_maps_retiring(&forest.nodes, &forest.tag_roots, &forest.initial, &retired);
        let (_, map) = &changed[0];
        assert!(!map.keys().any(|k| k.contains("/thoughts/")), "{map:?}");
        for old in [
            "personal/proj/ideas/a.md",
            "personal/proj/ideas/deep/deeper/n.md",
        ] {
            assert!(!map.contains_key(old), "{old}: {map:?}");
        }
        assert!(map.contains_key("personal/proj/ideas-other.md"));
    }

    #[test]
    fn the_folder_prompt_counts_notes_and_mentions_other_files() {
        let (_dir, notez, root) = vault();
        std::fs::create_dir_all(root.join("one")).unwrap();
        std::fs::write(root.join("one/x.md"), "# x\n").unwrap();
        std::fs::create_dir_all(root.join("mixed/sub")).unwrap();
        std::fs::write(root.join("mixed/sub/m.md"), "# m\n").unwrap();
        std::fs::write(root.join("mixed/sub/upper.MD"), "# m\n").unwrap();
        std::fs::write(root.join("mixed/pic.png"), "png").unwrap();
        std::fs::create_dir_all(root.join("hidden-only")).unwrap();
        std::fs::write(root.join("hidden-only/.DS_Store"), "").unwrap();
        let mut forest = forest_of(vault_sections(&notez));
        let mut question =
            |sub: &str| delete_question(&folder_prompt(&mut forest, &root.join(sub)));

        assert_eq!(
            question("ideas/empty"),
            "delete ideas/empty/ (no notes) from personal? y/n"
        );
        assert_eq!(
            question("one"),
            "delete one/ and its 1 note from personal? y/n"
        );
        assert_eq!(
            question("ideas"),
            "delete ideas/ and its 2 notes from personal? y/n"
        );
        assert_eq!(
            question("mixed"),
            "delete mixed/ and its 2 notes and other files from personal? y/n"
        );
        assert_eq!(
            question("hidden-only"),
            "delete hidden-only/ (no notes) and other files from personal? y/n"
        );
    }

    /// A symlink inside the folder goes as a link: its target, and the
    /// notes in it, stay; the prompt counts it as another file.
    #[cfg(unix)]
    #[test]
    fn a_folder_delete_removes_a_symlink_inside_it_but_not_its_target() {
        let (_dir, notez, root) = vault();
        let rebuild = || Ok(vault_sections(&notez));
        std::os::unix::fs::symlink(root.join("plans"), root.join("ideas/link")).unwrap();
        let mut forest = forest_of(vault_sections(&notez));
        let mut retired = Vec::new();

        let prompt = folder_prompt(&mut forest, &root.join("ideas"));
        assert_eq!(
            delete_question(&prompt),
            "delete ideas/ and its 2 notes and other files from personal? y/n"
        );
        answer_delete(
            KeyCode::Char('y'),
            &mut forest,
            &mut retired,
            &prompt,
            "",
            &rebuild,
        )
        .unwrap();

        assert!(!root.join("ideas").exists());
        assert!(
            root.join("plans/p.md").is_file(),
            "the link's target is untouched"
        );
        assert!(
            !retired.iter().any(|(_, k)| k.contains("plans")),
            "{retired:?}"
        );
    }

    #[test]
    fn a_local_folder_prompt_says_not_recoverable() {
        let (_dir, roots) = temp_tree();
        let mut forest = forest_of(list_sections_with_dirs(&roots));
        for (scope, _) in SCOPES {
            let root = &roots.iter().find(|(s, _)| *s == scope).unwrap().1;
            let q = delete_question(&folder_prompt(&mut forest, &root.join("ideas")));
            assert_eq!(
                q.contains("(not recoverable)"),
                scope == Scope::Local,
                "{q}"
            );
            assert!(q.starts_with("delete ideas/ and its 2 notes from "), "{q}");
        }
    }

    #[test]
    fn any_answer_but_y_on_a_folder_removes_nothing() {
        let (_dir, notez, root) = vault();
        let rebuild = || Ok(vault_sections(&notez));
        let mut forest = forest_of(vault_sections(&notez));
        let mut retired = Vec::new();
        let prompt = folder_prompt(&mut forest, &root.join("ideas"));
        let disk = disk_entries(&notez);
        for code in [
            KeyCode::Char('n'),
            KeyCode::Esc,
            KeyCode::Enter,
            KeyCode::Char('Y'),
        ] {
            assert!(
                answer_delete(code, &mut forest, &mut retired, &prompt, "", &rebuild).is_none()
            );
        }
        assert_eq!(disk_entries(&notez), disk);
        assert!(retired.is_empty());
    }

    #[test]
    fn remove_dir_all_is_unreachable_for_a_section_root_or_a_path_outside_it() {
        let (_dir, notez, root) = vault();
        let roots = vec![root.clone(), notez.clone()];
        let refused = [
            (root.clone(), root.clone(), "the section root"),
            (notez.clone(), root.clone(), "above the section root"),
            (notez.join("elsewhere"), root.clone(), "outside the section"),
            (root.join("ideas/../plans"), root.clone(), "a parent step"),
            (root.join(".."), root.clone(), "a parent step alone"),
            (
                notez.join("personal"),
                notez.clone(),
                "a folder holding another section",
            ),
            (
                notez.join("personal/proj"),
                notez.clone(),
                "another section's root",
            ),
        ];
        std::fs::create_dir_all(notez.join("elsewhere")).unwrap();
        let disk = disk_entries(&notez);
        for (path, section_root, why) in refused {
            assert!(
                !folder_change_allowed(&path, &section_root, &roots),
                "{why}"
            );
            let err = remove_folder(&path, &section_root, &roots).expect_err(why);
            assert!(
                err.to_string().contains("not a folder inside its section"),
                "{why}: {err}"
            );
        }
        assert_eq!(disk_entries(&notez), disk, "nothing removed");
        assert!(folder_change_allowed(
            &root.join("ideas/deep"),
            &root,
            &roots
        ));
    }

    #[test]
    fn a_prompt_aimed_at_a_section_root_removes_nothing() {
        let (_dir, notez, root) = vault();
        let rebuild = || Ok(vault_sections(&notez));
        let mut forest = forest_of(vault_sections(&notez));
        let mut retired = Vec::new();
        let mut prompt = folder_prompt(&mut forest, &root.join("ideas"));
        prompt.path = root.clone();
        let disk = disk_entries(&notez);

        let outcome = answer_delete(
            KeyCode::Char('y'),
            &mut forest,
            &mut retired,
            &prompt,
            "",
            &rebuild,
        )
        .unwrap();

        assert!(
            outcome.message.starts_with("delete failed: "),
            "{}",
            outcome.message
        );
        assert_eq!(disk_entries(&notez), disk);
        assert!(retired.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn a_folder_delete_that_fails_midway_reports_rebuilds_and_retires_only_what_went() {
        use std::os::unix::fs::PermissionsExt;
        let (_dir, notez, root) = vault();
        let locked = root.join("ideas/deep/deeper");
        let rebuild = || Ok(vault_sections(&notez));
        let mut forest = forest_of(vault_sections(&notez));
        let mut retired = Vec::new();
        let prompt = folder_prompt(&mut forest, &root.join("ideas"));

        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o555)).unwrap();
        let outcome = answer_delete(
            KeyCode::Char('y'),
            &mut forest,
            &mut retired,
            &prompt,
            "",
            &rebuild,
        );
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
        let outcome = outcome.unwrap();

        assert!(
            outcome.message.starts_with("delete failed: "),
            "{}",
            outcome.message
        );
        assert!(locked.join("n.md").is_file(), "the locked note is left");
        assert!(root.join("plans/p.md").is_file() && root.join("top.md").is_file());
        assert!(
            forest.nodes.iter().any(|n| n.path == locked.join("n.md")),
            "the tree shows what is left"
        );
        assert_eq!(
            outcome.row,
            Some(row(&forest.nodes, path_str(&root.join("ideas"))))
        );
        let keys: Vec<&str> = retired.iter().map(|(_, k)| k.as_str()).collect();
        assert!(
            !keys.contains(&"personal/proj/ideas/deep/deeper/n.md"),
            "{keys:?}"
        );
        assert_eq!(
            keys.contains(&"personal/proj/ideas/a.md"),
            !root.join("ideas/a.md").exists(),
            "a note's key goes exactly when the note went: {keys:?}"
        );
        assert!(
            keys.contains(&"personal/proj/ideas/gone-before.md"),
            "a key whose note is not on disk goes"
        );
    }

    #[test]
    fn a_failed_rebuild_after_a_folder_delete_drops_the_folder_rows() {
        let (_dir, notez, root) = vault();
        let rebuild = || -> Result<Vec<SectionSpec>> { anyhow::bail!("listing broke") };
        let mut forest = forest_of(vault_sections(&notez));
        let mut retired = Vec::new();
        let prompt = folder_prompt(&mut forest, &root.join("ideas"));

        let outcome = answer_delete(
            KeyCode::Char('y'),
            &mut forest,
            &mut retired,
            &prompt,
            "",
            &rebuild,
        )
        .unwrap();

        assert!(!root.join("ideas").exists());
        assert!(
            outcome.message.contains("listing broke"),
            "{}",
            outcome.message
        );
        assert!(!forest
            .nodes
            .iter()
            .any(|n| n.path.starts_with(root.join("ideas"))));
        assert!(forest
            .nodes
            .iter()
            .any(|n| n.path == root.join("plans/p.md")));
    }

    #[test]
    fn r_and_d_help_cover_folders_and_keep_their_footer_slots() {
        let hint = |k: &str| {
            TREE_KEYS
                .iter()
                .find(|h| h.key == k && h.modes == BROWSE)
                .unwrap()
        };
        assert!(
            hint("r").help.starts_with("rename note or folder"),
            "{}",
            hint("r").help
        );
        assert!(
            hint("d").help.starts_with("delete note or folder"),
            "{}",
            hint("d").help
        );
        assert_eq!(hint("r").slot, Slot::Priority(6));
        assert_eq!(hint("d").slot, Slot::Priority(7));
    }

    // --- The todo board's store (NZ-20) ---

    /// [`move_fixture`] plus the todo board's store `_todos/` (with a note
    /// and a category folder), a sibling `_todos-archive/` and a personal
    /// `_todos/` folder, neither of which is the store.
    fn todo_fixture() -> (tempfile::TempDir, PathBuf, Vec<(Scope, PathBuf)>) {
        let (dir, notez, roots) = move_fixture();
        let personal = store_of(&roots, Scope::Personal);
        for sub in ["_todos/work", "_todos-archive"] {
            std::fs::create_dir_all(notez.join(sub)).unwrap();
        }
        std::fs::create_dir_all(personal.join("_todos")).unwrap();
        for file in [
            notez.join("_todos/t.md"),
            notez.join("_todos/work/w.md"),
            notez.join("_todos-archive/a.md"),
            personal.join("_todos/p.md"),
        ] {
            std::fs::write(file, "# todo\n").unwrap();
        }
        (dir, notez, roots)
    }

    #[test]
    fn delete_rename_move_and_set_scope_refuse_the_todo_store_and_what_is_in_it() {
        let (dir, notez, roots) = todo_fixture();
        let ctx = move_ctx(&roots);
        let forest = forest_of(move_sections(&roots));
        let before = disk_entries(dir.path());
        let (nodes, sections) = (&forest.nodes, &forest.sections);

        for sub in ["_todos", "_todos/t.md", "_todos/work", "_todos/work/w.md"] {
            let at = Some(row(nodes, path_str(&notez.join(sub))));
            assert_eq!(
                delete_request(nodes, sections, at, Some("proj")).err(),
                Some(TODOS_DELETE),
                "{sub}"
            );
            assert_eq!(
                rename_request(nodes, sections, at).err(),
                Some(TODOS_RENAME),
                "{sub}"
            );
            assert_eq!(
                move_request(nodes, sections, at, &ctx).err(),
                Some(TODOS_MOVE),
                "{sub}"
            );
            assert_eq!(
                set_scope_request(nodes, sections, at, &ctx).err(),
                Some(TODOS_SET_SCOPE),
                "{sub}"
            );
        }
        assert_eq!(
            TODOS_DELETE,
            "delete: the todo board's store is managed by the todo view"
        );

        let personal = store_of(&roots, Scope::Personal);
        for path in [
            notez.join("_todos-archive"),
            notez.join("_todos-archive/a.md"),
            personal.join("_todos/p.md"),
        ] {
            let at = Some(row(nodes, path_str(&path)));
            assert!(
                matches!(
                    delete_request(nodes, sections, at, Some("proj")),
                    Ok(Some(_))
                ),
                "{}",
                path.display()
            );
            assert!(
                matches!(rename_request(nodes, sections, at), Ok(Some(_))),
                "{}",
                path.display()
            );
            assert!(
                move_request(nodes, sections, at, &ctx).is_ok(),
                "{}",
                path.display()
            );
            assert!(
                set_scope_request(nodes, sections, at, &ctx).is_ok(),
                "{}",
                path.display()
            );
        }
        assert_eq!(disk_entries(dir.path()), before);
    }

    #[test]
    fn a_bulk_action_with_a_todo_store_row_is_refused_whole() {
        let (dir, notez, roots) = todo_fixture();
        let ctx = move_ctx(&roots);
        let forest = forest_of(move_sections(&roots));
        let before = disk_entries(dir.path());
        let set = [notez.join("c.md"), notez.join("_todos/work/w.md")];
        let marks: HashSet<PathBuf> = set.iter().cloned().collect();

        let refused =
            bulk_delete_request(&forest.nodes, &forest.sections, &marks, Some("proj")).err();
        assert_eq!(refused, Some(format!("_todos/work/w.md: {TODOS_DELETE}")));
        assert_eq!(
            bulk_move_at(&forest, &ctx, &set, false).err(),
            Some(format!("_todos/work/w.md: {TODOS_MOVE}"))
        );
        assert_eq!(
            bulk_move_at(&forest, &ctx, &set, true).err(),
            Some(format!("_todos/work/w.md: {TODOS_SET_SCOPE}"))
        );
        let folder: HashSet<PathBuf> = [notez.join("_todos")].into_iter().collect();
        let refused =
            bulk_delete_request(&forest.nodes, &forest.sections, &folder, Some("proj")).err();
        assert_eq!(refused, Some(format!("_todos/: {TODOS_DELETE}")));
        assert_eq!(disk_entries(dir.path()), before);
    }

    #[test]
    fn the_todo_store_is_never_a_move_destination() {
        let (dir, notez, roots) = todo_fixture();
        let ctx = move_ctx(&roots);
        let forest = forest_of(move_sections(&roots));
        let before = disk_entries(dir.path());
        let personal = store_of(&roots, Scope::Personal);
        let refusal = format!("move: {}", TODO_STORE_MANAGED);

        assert_eq!(
            move_plan(&forest, &ctx, &notez.join("c.md"), Scope::Global, "_todos"),
            Err(refusal.clone())
        );
        assert_eq!(
            move_plan(
                &forest,
                &ctx,
                &personal.join("c.md"),
                Scope::Global,
                "_todos/work"
            ),
            Err(refusal.clone())
        );
        let set = [personal.join("c.md"), personal.join("ideas/a.md")];
        assert_eq!(
            bulk_move_plan(&forest, &ctx, &set, Scope::Global, "_todos").err(),
            Some(refusal)
        );
        assert_eq!(disk_entries(dir.path()), before);

        let plan = move_plan(
            &forest,
            &ctx,
            &personal.join("c.md"),
            Scope::Global,
            "_todos-archive",
        )
        .expect("not the store");
        assert_eq!(plan.dst, notez.join("_todos-archive/c.md"));
        let plan = move_plan(
            &forest,
            &ctx,
            &notez.join("c.md"),
            Scope::Personal,
            "_todos",
        )
        .expect("a personal folder");
        assert_eq!(plan.dst, personal.join("_todos/c.md"));
    }

    #[test]
    fn the_todo_store_is_matched_as_the_file_system_resolves_it() {
        let dir = tempfile::tempdir().unwrap();
        let notez = dir.path().join("n");
        std::fs::create_dir_all(notez.join("_TODOS")).unwrap();
        std::fs::create_dir_all(notez.join("_todos-archive")).unwrap();
        // The board reads `<notez root>/_todos`; on a case-insensitive file
        // system that is `_TODOS`, on a case-sensitive one it is not.
        let is_store = notez.join("_todos").exists();
        assert_eq!(in_todo_store(&notez.join("_TODOS/x.md"), &notez), is_store);
        assert!(in_todo_store(&notez.join("_todos"), &notez));
        assert!(in_todo_store(&notez.join("_todos/a/b.md"), &notez));
        assert!(!in_todo_store(&notez.join("_todos-archive/a.md"), &notez));
        assert!(!in_todo_store(&notez, &notez));
        assert!(!in_todo_store(&notez.join("personal/proj/_todos"), &notez));
    }
}
