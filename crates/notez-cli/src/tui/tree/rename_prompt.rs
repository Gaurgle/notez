//! The rename prompt (`r`) for notes and folders.

use super::*;

// --- Rename ---

/// The footer message for `r` on a section row.
const SECTION_RENAME: &str = "rename: a section cannot be renamed";

/// The footer message for a folder name that sanitizes to nothing.
const FOLDER_RENAME_EMPTY: &str = "rename: the name is empty";

/// What `r` does with the cursor on `row`: `Ok(Some)` opens the rename
/// prompt with that text (a note's editable title, a folder's name), `Err`
/// is the footer message for a section row or a row in the todo board's
/// store ([`is_todo_row`]), and `Ok(None)` does nothing:
/// no row under the cursor, or a folder in a docs section.
pub(super) fn rename_request(
    nodes: &[TreeNode],
    sections: &[SectionSpec],
    row: Option<usize>,
) -> std::result::Result<Option<String>, &'static str> {
    let Some(node) = row.and_then(|i| nodes.get(i)) else {
        return Ok(None);
    };
    if node.depth == 0 {
        return Err(SECTION_RENAME);
    }
    if is_todo_row(node, sections) {
        return Err(TODOS_RENAME);
    }
    if !node.is_dir {
        return Ok(Some(rename::editable_title(&node.name)));
    }
    let is_doc = sections.get(node.section).map_or(true, |s| s.is_doc);
    Ok((!is_doc).then(|| node.name.clone()))
}

/// Rename the folder row at `idx` to `name` within its parent, sanitized
/// like `notez mkdir` names. Refused, with nothing changed, for an empty
/// name, a row [`folder_change_allowed`] rejects, or a target that already
/// exists (file or folder). On a case-insensitive file system a case-only
/// change finds the folder itself at the target; that is renamed, and
/// refused if the file system keeps the old spelling. On success every row
/// under the folder, and the stored sections' listings, take the new path;
/// rows keep their `origin`, so the exit write moves their `.tags` keys
/// and no `.tags` file is touched here. The rows stay where they are, so
/// the cursor stays on the folder.
pub(super) fn rename_folder(
    nodes: &mut [TreeNode],
    sections: &mut [SectionSpec],
    idx: usize,
    name: &str,
) -> std::result::Result<(), String> {
    let Some(node) = nodes.get(idx).filter(|n| n.is_dir && n.depth > 0) else {
        return Ok(());
    };
    let Some(spec) = sections.get(node.section) else {
        return Ok(());
    };
    // Enter on the untouched prompt keeps the name as it is, even one
    // sanitizing would change (`00_quick-notes`, `_todos`, `IDEAS`).
    if name.trim() == node.name {
        return Ok(());
    }
    let cleaned = sanitize::name(name);
    if cleaned.is_empty() {
        return Err(FOLDER_RENAME_EMPTY.to_string());
    }
    let old = node.path.clone();
    let section_roots: Vec<PathBuf> = sections.iter().map(|s| s.root.clone()).collect();
    if !folder_change_allowed(&old, &spec.root, &section_roots) {
        return Err(format!(
            "rename: {} is not a folder inside its section",
            old.display()
        ));
    }
    let new = old.with_file_name(&cleaned);
    if new == old {
        return Ok(());
    }
    let case_only = std::fs::symlink_metadata(&new).is_ok();
    if case_only && !is_same_entry(&old, &new) {
        return Err(format!("rename: {cleaned} already exists"));
    }
    std::fs::rename(&old, &new).map_err(|e| format!("rename failed: {e}"))?;
    if case_only && !has_entry_named(new.parent().unwrap_or(&new), &cleaned) {
        return Err(format!(
            "rename: the file system kept the name {}",
            node.name
        ));
    }
    for node in nodes.iter_mut() {
        if let Some(moved) = moved_path(&node.path, &old, &new) {
            node.path = moved;
        }
    }
    nodes[idx].name = cleaned;
    for spec in sections.iter_mut() {
        for path in spec.files.iter_mut().chain(spec.dirs.iter_mut()) {
            if let Some(moved) = moved_path(path, &old, &new) {
                *path = moved;
            }
        }
    }
    Ok(())
}

/// What `Enter` in the rename prompt did.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum RenameEnter {
    /// The typed name is refused: the prompt stays open with it and the
    /// footer shows the message.
    Keep(String),
    /// The prompt closes, with the footer message if there is one.
    Done(Option<String>),
}

/// `Enter` in the rename prompt opened on the row at `idx` showing `shown`
/// (the text the prompt was prefilled with), with `typed` in the buffer.
/// `typed` trimmed equal to `shown` changes nothing: no file, heading or
/// `.tags` key moves, even for a name sanitizing would alter (`My_Note`,
/// `x.MD`). Otherwise a name [`name_would_change`] alters is refused before
/// any disk work, and the rest renames the folder ([`rename_folder`]) or
/// the note (`notez rename`'s [`rename::rename_note`]).
pub(super) fn enter_rename(
    nodes: &mut [TreeNode],
    sections: &mut [SectionSpec],
    idx: usize,
    shown: &str,
    typed: &str,
) -> RenameEnter {
    if typed.trim() == shown {
        return RenameEnter::Done(None);
    }
    if let Some(cleaned) = name_would_change(typed) {
        return RenameEnter::Keep(altered_name_message(&cleaned));
    }
    let Some(node) = nodes.get(idx) else {
        return RenameEnter::Done(None);
    };
    if node.is_dir {
        return RenameEnter::Done(rename_folder(nodes, sections, idx, typed).err());
    }
    match rename::rename_note(&node.path, typed) {
        Ok(new_path) => {
            nodes[idx].name = file_name_of(&new_path);
            nodes[idx].path = new_path;
            RenameEnter::Done(None)
        }
        Err(e) => RenameEnter::Done(Some(format!("rename failed: {e}"))),
    }
}

/// `path` with its `old` prefix replaced by `new`, if it lies under `old`.
pub(super) fn moved_path(path: &Path, old: &Path, new: &Path) -> Option<PathBuf> {
    let rest = path.strip_prefix(old).ok()?;
    Some(if rest.as_os_str().is_empty() {
        new.to_path_buf()
    } else {
        new.join(rest)
    })
}

/// Whether `a` and `b` name the same directory entry (the case-insensitive
/// match of a case-only rename).
#[cfg(unix)]
pub(super) fn is_same_entry(a: &Path, b: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    match (std::fs::symlink_metadata(a), std::fs::symlink_metadata(b)) {
        (Ok(x), Ok(y)) => x.dev() == y.dev() && x.ino() == y.ino(),
        _ => false,
    }
}

/// Without inode numbers a match cannot be told from another entry, so an
/// existing target is always refused.
#[cfg(not(unix))]
pub(super) fn is_same_entry(_a: &Path, _b: &Path) -> bool {
    false
}

/// Whether `dir` lists an entry spelled exactly `name`.
pub(super) fn has_entry_named(dir: &Path, name: &str) -> bool {
    std::fs::read_dir(dir)
        .map(|entries| entries.flatten().any(|e| e.file_name() == name))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use super::*;

    // --- Folder rename and delete ---

    /// Enter on the untouched prompt keeps a name `sanitize::name` would
    /// change (underscores, capitals): nothing on disk, in the rows or in
    /// the tags moves.
    #[test]
    fn folder_rename_with_the_shown_name_unchanged_changes_nothing() {
        let (_dir, notez, root) = vault();
        for sub in ["00_quick-notes", "_todos/IDEAS"] {
            std::fs::create_dir_all(root.join(sub)).unwrap();
        }
        for file in ["00_quick-notes/q.md", "_todos/t.md", "_todos/IDEAS/i.md"] {
            std::fs::write(root.join(file), "# note\n").unwrap();
        }
        let mut forest = forest_of(vault_sections(&notez));
        let before = disk_entries(&notez);
        let paths = |f: &Forest| f.nodes.iter().map(|n| n.path.clone()).collect::<Vec<_>>();
        let rows = paths(&forest);

        for sub in ["00_quick-notes", "_todos", "_todos/IDEAS"] {
            let path = root.join(sub);
            let shown = forest.nodes[row(&forest.nodes, path_str(&path))]
                .name
                .clone();
            assert_eq!(rename_at(&mut forest, &path, &shown), Ok(()), "{sub}");
            assert_eq!(disk_entries(&notez), before, "{sub}");
            assert_eq!(paths(&forest), rows, "{sub}");
        }
        assert!(changed_tag_maps(&forest.nodes, &forest.tag_roots, &forest.initial).is_empty());
    }

    #[test]
    fn folder_rename_moves_every_note_and_the_tags_follow_on_exit() {
        let (_dir, notez, root) = vault();
        let mut forest = forest_of(vault_sections(&notez));
        let initial = note_tags::load_tags(&notez);
        let ideas = row(&forest.nodes, path_str(&root.join("ideas")));

        rename_at(&mut forest, &root.join("ideas"), "Thoughts").expect("renamed");

        let thoughts = root.join("thoughts");
        assert!(!root.join("ideas").exists());
        for file in ["a.md", "deep/deeper/n.md"] {
            assert!(thoughts.join(file).is_file(), "{file}");
        }
        assert!(thoughts.join("empty").is_dir());
        assert_eq!(
            forest.nodes[ideas].path, thoughts,
            "the cursor row is the renamed folder"
        );
        assert_eq!(forest.nodes[ideas].name, "thoughts");
        let deep = forest.nodes.iter().find(|n| n.name == "n.md").unwrap();
        assert_eq!(deep.path, thoughts.join("deep/deeper/n.md"));
        assert_eq!(
            deep.origin,
            root.join("ideas/deep/deeper/n.md"),
            "origin is kept"
        );
        assert!(forest
            .nodes
            .iter()
            .all(|n| !n.path.starts_with(root.join("ideas"))));
        assert!(forest.sections[0]
            .dirs
            .contains(&thoughts.join("deep/deeper")));
        assert!(forest.sections[0].files.contains(&thoughts.join("a.md")));

        let mut expected = initial.clone();
        for (old, new) in [
            ("ideas/a.md", "thoughts/a.md"),
            ("ideas/deep/deeper/n.md", "thoughts/deep/deeper/n.md"),
        ] {
            let flags = expected.remove(&format!("personal/proj/{old}")).unwrap();
            expected.insert(format!("personal/proj/{new}"), flags);
        }
        let changed = changed_tag_maps(&forest.nodes, &forest.tag_roots, &forest.initial);
        assert_eq!(changed, vec![(notez.clone(), expected.clone())]);
        assert!(
            expected.contains_key("personal/proj/ideas/gone-before.md"),
            "keys without a row stay"
        );
        assert!(
            expected.contains_key("personal/proj/ideas-other.md"),
            "a look-alike prefix stays"
        );

        // A later rebuild (a new note, say) lists the new paths and keeps
        // the origins, so the exit write is the same.
        forest.rebuild(vault_sections(&notez), Path::new("/nothing/created"));
        let after = changed_tag_maps(&forest.nodes, &forest.tag_roots, &forest.initial);
        assert_eq!(after, vec![(notez.clone(), expected)]);
    }

    #[test]
    fn folder_rename_onto_an_existing_name_changes_nothing() {
        let (_dir, notez, root) = vault();
        std::fs::write(root.join("plain"), "not a note").unwrap();
        let mut forest = forest_of(vault_sections(&notez));
        let disk = disk_entries(&notez);
        let rows: Vec<PathBuf> = forest.nodes.iter().map(|n| n.path.clone()).collect();

        // A sibling folder (any case), a sibling file, and a nested folder
        // onto its own sibling.
        for (folder, name) in [
            ("ideas", "plans"),
            ("ideas", "Plans"),
            ("ideas", "plain"),
            ("ideas/deep", "empty"),
        ] {
            let message = rename_at(&mut forest, &root.join(folder), name).expect_err(name);
            assert!(message.contains("already exists"), "{name}: {message}");
        }
        assert_eq!(disk_entries(&notez), disk, "nothing moved");
        let now: Vec<PathBuf> = forest.nodes.iter().map(|n| n.path.clone()).collect();
        assert_eq!(now, rows, "no row path changed");
        assert!(changed_tag_maps(&forest.nodes, &forest.tag_roots, &forest.initial).is_empty());
    }

    #[test]
    fn folder_rename_refuses_an_empty_name_and_keeps_the_same_name() {
        let (_dir, notez, root) = vault();
        let mut forest = forest_of(vault_sections(&notez));
        let disk = disk_entries(&notez);
        for name in ["", "  ", "?!"] {
            assert_eq!(
                rename_at(&mut forest, &root.join("ideas"), name),
                Err(FOLDER_RENAME_EMPTY.to_string())
            );
        }
        assert_eq!(
            rename_at(&mut forest, &root.join("ideas"), "Ideas"),
            Ok(()),
            "same name: nothing to do"
        );
        assert_eq!(disk_entries(&notez), disk);
        assert!(changed_tag_maps(&forest.nodes, &forest.tag_roots, &forest.initial).is_empty());
    }

    /// `Ideas` to `ideas`: on a case-insensitive file system the target
    /// "exists" as the folder itself, which is no reason to refuse.
    #[test]
    fn folder_rename_can_change_only_the_case() {
        let (_dir, notez, root) = vault();
        std::fs::create_dir_all(root.join("Caps")).unwrap();
        std::fs::write(root.join("Caps/c.md"), "# c\n").unwrap();
        let mut forest = forest_of(vault_sections(&notez));

        rename_at(&mut forest, &root.join("Caps"), "caps").expect("a case-only rename");

        let names: Vec<String> = std::fs::read_dir(&root)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert!(
            names.contains(&"caps".to_string()) && !names.contains(&"Caps".to_string()),
            "{names:?}"
        );
        assert!(forest
            .nodes
            .iter()
            .any(|n| n.path == root.join("caps/c.md")));
    }

    #[test]
    fn r_on_a_section_a_docs_folder_or_an_empty_tree_changes_nothing() {
        let sections = project_sections();
        let (nodes, _) = build_forest(&sections);
        for path in ["/n/personal/proj", "/p/docs", "/n"] {
            assert_eq!(
                rename_request(&nodes, &sections, Some(row(&nodes, path))),
                Err(SECTION_RENAME),
                "{path}"
            );
        }
        assert_eq!(
            rename_request(&nodes, &sections, Some(row(&nodes, "/p/docs/design"))),
            Ok(None)
        );
        assert_eq!(
            rename_request(&nodes, &sections, Some(row(&nodes, "/p/docs/design/c.md"))),
            Ok(Some("c".to_string())),
            "a docs note renames as before"
        );
        assert_eq!(
            rename_request(
                &nodes,
                &sections,
                Some(row(&nodes, "/n/personal/proj/ideas"))
            ),
            Ok(Some("ideas".to_string()))
        );
        assert_eq!(rename_request(&nodes, &sections, None), Ok(None));
        assert_eq!(
            rename_request(&nodes, &sections, Some(nodes.len())),
            Ok(None)
        );
        assert_eq!(rename_request(&[], &[], Some(0)), Ok(None));
        assert_eq!(rename_request(&[], &[], None), Ok(None));
    }

    /// NZ-26: `r` takes `My Note` on a note (file `my-note.md`, heading as
    /// typed) and `My Ideas` on a folder (`my-ideas`); `Ideas` on the folder
    /// `ideas` sanitizes to its own name and changes nothing, silently.
    #[test]
    fn rename_enter_accepts_capitals_and_blanks_for_notes_and_folders() {
        let (_dir, notez, root) = vault();
        let mut forest = forest_of(vault_sections(&notez));
        let before = disk_entries(&notez);
        assert_eq!(
            rename_enter_at(&mut forest, &root.join("ideas"), "Ideas"),
            RenameEnter::Done(None)
        );
        assert_eq!(
            disk_entries(&notez),
            before,
            "Ideas on ideas: nothing to do"
        );
        assert!(has_entry_named(&root, "ideas"));
        assert!(changed_tag_maps(&forest.nodes, &forest.tag_roots, &forest.initial).is_empty());

        assert_eq!(
            rename_enter_at(&mut forest, &root.join("top.md"), "00_quick"),
            RenameEnter::Keep(altered_name_message("00quick"))
        );
        assert_eq!(
            rename_enter_at(&mut forest, &root.join("ideas"), "00_quick"),
            RenameEnter::Keep(altered_name_message("00quick"))
        );
        assert_eq!(disk_entries(&notez), before, "nothing renamed");

        assert_eq!(
            rename_enter_at(&mut forest, &root.join("top.md"), "My Note"),
            RenameEnter::Done(None)
        );
        let note = root.join("my-note.md");
        assert!(note.is_file() && !root.join("top.md").exists());
        assert_eq!(std::fs::read_to_string(&note).unwrap(), "# My Note\n");
        assert_eq!(
            forest.nodes[row(&forest.nodes, path_str(&note))].name,
            "my-note.md"
        );

        assert_eq!(
            rename_enter_at(&mut forest, &root.join("ideas"), "My Ideas"),
            RenameEnter::Done(None)
        );
        assert!(root.join("my-ideas/a.md").is_file() && !root.join("ideas").exists());
    }

    #[test]
    fn note_rename_with_the_shown_title_unchanged_changes_nothing() {
        let (_dir, notez, root) = vault();
        let content = "# My_Note\n\nbody\n";
        for name in ["2026-10-06-My_Note.md", "x.MD"] {
            std::fs::write(root.join(name), content).unwrap();
        }
        std::fs::write(
            notez.join(".tags"),
            "personal/proj/2026-10-06-My_Note.md:3\npersonal/proj/x.MD:1\n",
        )
        .unwrap();
        let mut sections = vault_sections(&notez);
        sections[0]
            .files
            .extend([root.join("2026-10-06-My_Note.md"), root.join("x.MD")]);
        let mut forest = forest_of(sections);
        let before = disk_entries(&notez);
        let tags = std::fs::read_to_string(notez.join(".tags")).unwrap();

        for (name, shown) in [("2026-10-06-My_Note.md", "My_Note"), ("x.MD", "x.MD")] {
            let path = root.join(name);
            for typed in [shown.to_string(), format!("  {shown} ")] {
                assert_eq!(
                    rename_enter_at(&mut forest, &path, &typed),
                    RenameEnter::Done(None),
                    "{name}"
                );
            }
            assert_eq!(
                std::fs::read_to_string(&path).unwrap(),
                content,
                "{name}: the heading stays"
            );
            assert_eq!(forest.nodes[row(&forest.nodes, path_str(&path))].name, name);
        }
        assert_eq!(disk_entries(&notez), before);
        assert!(changed_tag_maps(&forest.nodes, &forest.tag_roots, &forest.initial).is_empty());
        assert_eq!(std::fs::read_to_string(notez.join(".tags")).unwrap(), tags);
    }
}
