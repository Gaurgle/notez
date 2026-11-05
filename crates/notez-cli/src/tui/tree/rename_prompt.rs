//! The rename prompt (`r`) for notes and folders.

use super::*;
use crate::commands::agenda;

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
        return Ok(Some(note_stem(&node.name)));
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

/// The refusal for a note name typed with an extension.
const NOTE_EXTENSION: &str = "rename: name only, the extension is kept";

/// The part of a note's file name the rename prompt edits: the stem,
/// without its date prefix. The extension, in whatever case, is never
/// shown (`D.MD` is `D`).
fn note_stem(file_name: &str) -> String {
    rename::editable_title(&Path::new(file_name).with_extension("md").to_string_lossy())
}

/// Whether a typed note name ends in an extension (`b.md`, `b.txt`) or is
/// one (`.md`): a dot, then two or more ASCII letters and digits starting
/// with a letter. A note's extension is never typed. A shorter tail
/// (`a.b`) is no extension; the altered-name refusal answers it.
fn has_extension(typed: &str) -> bool {
    typed.rsplit_once('.').is_some_and(|(_, ext)| {
        ext.len() >= 2
            && ext.starts_with(|c: char| c.is_ascii_alphabetic())
            && ext.chars().all(|c| c.is_ascii_alphanumeric())
    })
}

/// The rename prompt's title for `node`, by the first kind that holds:
/// `rename directory` for a folder, `rename todo` for a todo note
/// ([`is_todo_note`]), `rename log` for a note in a daily-logs folder
/// ([`agenda::is_daily_log`], within its section), `rename quick note` for
/// a note directly in a quick-notes folder ([`is_quick_notes_dir`]),
/// `rename doc` for a note in a docs section, else `rename note`.
pub(super) fn rename_title(node: &TreeNode, sections: &[SectionSpec], config: &Config) -> &'static str {
    let spec = sections.get(node.section);
    let in_section = spec
        .and_then(|s| node.path.strip_prefix(&s.root).ok())
        .unwrap_or(&node.path);
    if node.is_dir {
        "rename directory"
    } else if is_todo_note(node) {
        "rename todo"
    } else if agenda::is_daily_log(in_section, &config.paths.daily_logs_dir) {
        "rename log"
    } else if node.path.parent().is_some_and(|dir| is_quick_notes_dir(dir, config)) {
        "rename quick note"
    } else if spec.is_some_and(|s| s.is_doc) {
        "rename doc"
    } else {
        "rename note"
    }
}

/// The file name the note at `path` would have on disk after `Enter` with
/// `typed` in a prompt that showed `shown`: computed by [`note_target`],
/// as the rename does, so the two cannot disagree. `typed` unchanged is the
/// current name. `None` for a name `Enter` refuses before any disk work
/// (empty, an extension, an altered name); a clash is only found on Enter.
pub(super) fn rename_preview(path: &Path, shown: &str, typed: &str) -> Option<String> {
    let typed = typed.trim();
    if typed == shown {
        return Some(file_name_of(path));
    }
    if has_extension(typed) || name_would_change(typed).is_some() {
        return None;
    }
    note_target(path, typed).ok().map(|target| file_name_of(&target))
}

/// Where retitling the note at `path` to the stem `title` puts it: the
/// date-prefixed name [`rename::renamed_filename`] gives, with `path`'s own
/// extension put back, so `D.MD` stays `.MD`.
fn note_target(path: &Path, title: &str) -> anyhow::Result<PathBuf> {
    let mut target = path.with_file_name(rename::renamed_filename(&file_name_of(path), title)?);
    target.set_extension(path.extension().unwrap_or_default());
    Ok(target)
}

/// Retitle the note at `path` to the stem `title` the way `notez rename`
/// does ([`rename::rename_note`]), keeping the note's own extension: the
/// target is [`note_target`]. A note not ending in `.md` moves twice,
/// through the `.md` name `rename_note` gives it, and a taken target is
/// refused first.
fn rename_note_stem(path: &Path, title: &str) -> std::result::Result<PathBuf, String> {
    let failed = |e: anyhow::Error| format!("rename failed: {e}");
    let target = note_target(path, title).map_err(failed)?;
    let extension = path.extension().unwrap_or_default();
    if target == path {
        return Ok(target);
    }
    if extension == "md" {
        return rename::rename_note(path, title).map_err(failed);
    }
    if std::fs::symlink_metadata(&target).is_ok() {
        return Err(format!("rename failed: {} already exists", target.display()));
    }
    let as_md = rename::rename_note(path, title).map_err(failed)?;
    std::fs::rename(&as_md, &target).map_err(|e| format!("rename failed: {e}"))?;
    Ok(target)
}

/// `Enter` in the rename prompt opened on the row at `idx` showing `shown`
/// (the text the prompt was prefilled with), with `typed` in the buffer.
/// `typed` trimmed equal to `shown` changes nothing: no file, heading or
/// `.tags` key moves, even for a name sanitizing would alter (`My_Note`).
/// A note's name is its stem: one typed with an extension is refused
/// ([`has_extension`]). Otherwise a name [`name_would_change`] alters is
/// refused before any disk work, and the rest renames the folder
/// ([`rename_folder`]) or the note, keeping its extension
/// ([`rename_note_stem`]).
///
/// Every refusal that left the row where it was on disk (an empty name, a
/// clash, an extension) is `Keep`, so the prompt stays open with it; a
/// failure after the entry moved is `Done`, so the relist picks up the move.
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
    let Some(node) = nodes.get(idx) else {
        return RenameEnter::Done(None);
    };
    let typed = if node.is_dir { typed } else { typed.trim() };
    if !node.is_dir && has_extension(typed) {
        return RenameEnter::Keep(NOTE_EXTENSION.to_string());
    }
    if let Some(cleaned) = name_would_change(typed) {
        return RenameEnter::Keep(altered_name_message(&cleaned));
    }
    let before = node.path.clone();
    let failed = if node.is_dir {
        rename_folder(nodes, sections, idx, typed).err()
    } else {
        match rename_note_stem(&before, typed) {
            Ok(new_path) => {
                nodes[idx].name = file_name_of(&new_path);
                nodes[idx].path = new_path;
                None
            }
            Err(message) => Some(message),
        }
    };
    match failed {
        None => RenameEnter::Done(None),
        Some(message)
            if nodes[idx].path == before && std::fs::symlink_metadata(&before).is_ok() =>
        {
            RenameEnter::Keep(message)
        }
        Some(message) => RenameEnter::Done(Some(message)),
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

    /// NZ-76: the prompt shows a note's stem (no date, no extension) and a
    /// folder's full name.
    #[test]
    fn rename_prefill_is_a_notes_stem_and_a_folders_name() {
        let (_dir, notez, root) = vault();
        for name in ["D.MD", "README.md", "2026-10-06-dated.md"] {
            std::fs::write(root.join(name), "# n\n").unwrap();
        }
        let mut sections = vault_sections(&notez);
        sections[0].files.push(root.join("D.MD"));
        let forest = forest_of(sections);
        for (path, shown) in [
            ("top.md", "top"),
            ("D.MD", "D"),
            ("README.md", "README"),
            ("2026-10-06-dated.md", "dated"),
            ("ideas", "ideas"),
            ("ideas/deep", "deep"),
        ] {
            let i = row(&forest.nodes, path_str(&root.join(path)));
            assert_eq!(
                rename_request(&forest.nodes, &forest.sections, Some(i)),
                Ok(Some(shown.to_string())),
                "{path}"
            );
        }
    }

    /// NZ-76: a note's name is its stem only. Any typed extension, `.md`
    /// included, keeps the prompt open with the refusal; the note keeps
    /// the extension it has, in its case.
    #[test]
    fn note_rename_edits_the_stem_and_refuses_any_extension() {
        let (_dir, notez, root) = vault();
        std::fs::write(root.join("D.MD"), "# D\n").unwrap();
        let mut sections = vault_sections(&notez);
        sections[0].files.push(root.join("D.MD"));
        let mut forest = forest_of(sections);
        let before = disk_entries(&notez);
        for typed in [
            "b.md", "b.MD", "top.md", "b.txt", "b.TXT", "b.markdown", "b.md.txt", ".md", " b.md ",
        ] {
            assert_eq!(
                rename_enter_at(&mut forest, &root.join("top.md"), typed),
                RenameEnter::Keep(NOTE_EXTENSION.to_string()),
                "{typed}"
            );
        }
        assert_eq!(
            rename_enter_at(&mut forest, &root.join("D.MD"), "e.MD"),
            RenameEnter::Keep(NOTE_EXTENSION.to_string())
        );
        assert_eq!(disk_entries(&notez), before, "nothing renamed");

        assert_eq!(rename_enter_at(&mut forest, &root.join("top.md"), "b"), RenameEnter::Done(None));
        assert!(root.join("b.md").is_file() && !root.join("top.md").exists());
        assert_eq!(std::fs::read_to_string(root.join("b.md")).unwrap().lines().next(), Some("# b"));
        assert_eq!(forest.nodes[row(&forest.nodes, path_str(&root.join("b.md")))].name, "b.md");

        assert_eq!(rename_enter_at(&mut forest, &root.join("D.MD"), "e"), RenameEnter::Done(None));
        assert!(has_entry_named(&root, "e.MD") && !has_entry_named(&root, "e.md"));
        assert!(!has_entry_named(&root, "D.MD"));
        assert_eq!(std::fs::read_to_string(root.join("e.MD")).unwrap(), "# e\n");
        let e = row(&forest.nodes, path_str(&root.join("e.MD")));
        assert_eq!(forest.nodes[e].name, "e.MD");

        // `E` sanitizes to the stem it has: the note stays `e.MD`.
        let disk = disk_entries(&notez);
        assert_eq!(rename_enter_at(&mut forest, &root.join("e.MD"), "E"), RenameEnter::Done(None));
        assert_eq!(disk_entries(&notez), disk);
        assert!(has_entry_named(&root, "e.MD"));
    }

    /// An empty or clashing name, for a note or a folder, keeps the prompt
    /// open with its message: `enter_rename` decides that, not its caller.
    #[test]
    fn rename_enter_keeps_an_empty_or_clashing_name() {
        let (_dir, notez, root) = vault();
        std::fs::write(root.join("clash.md"), "# clash\n").unwrap();
        std::fs::write(root.join("Up.MD"), "# up\n").unwrap();
        std::fs::write(root.join("taken.MD"), "# taken\n").unwrap();
        // `taken.md` is free in any case: only the kept extension clashes.
        std::fs::write(root.join("Long.markdown"), "# long\n").unwrap();
        std::fs::write(root.join("held.markdown"), "# held\n").unwrap();
        let mut sections = vault_sections(&notez);
        sections[0]
            .files
            .extend([root.join("Up.MD"), root.join("Long.markdown")]);
        let mut forest = forest_of(sections);
        let before = disk_entries(&notez);
        for (path, typed, want) in [
            (root.join("top.md"), "", "no usable characters"),
            (root.join("top.md"), "  ", "no usable characters"),
            (root.join("top.md"), "clash", "already exists"),
            (root.join("top.md"), "Clash", "already exists"),
            (root.join("Up.MD"), "taken", "already exists"),
            (root.join("Long.markdown"), "held", "already exists"),
            (root.join("Up.MD"), "", "no usable characters"),
            (root.join("ideas"), "", "the name is empty"),
            (root.join("ideas"), "plans", "already exists"),
        ] {
            let outcome = rename_enter_at(&mut forest, &path, typed);
            let RenameEnter::Keep(message) = outcome else {
                panic!("{typed:?} on {}: {outcome:?}", path.display())
            };
            assert!(message.contains(want), "{typed:?}: {message}");
        }
        assert_eq!(disk_entries(&notez), before, "nothing moved");
    }

    /// NZ-76: the preview is the complete file name, date prefix and
    /// extension included, and it is the name the rename then gives on disk.
    /// A daily log keeps its date.
    #[test]
    fn rename_preview_is_the_name_the_rename_gives_on_disk() {
        let (_dir, notez, root) = vault();
        std::fs::create_dir_all(root.join("01_daily-logs")).unwrap();
        for name in ["D.MD", "2026-10-06-dated.md", "01_daily-logs/2026-10-06-daily-log.md"] {
            std::fs::write(root.join(name), "# n\n").unwrap();
        }
        let mut sections = vault_sections(&notez);
        sections[0].files.push(root.join("D.MD"));
        let mut forest = forest_of(sections);
        for (name, typed, want) in [
            ("top.md", "b", "b.md"),
            ("2026-10-06-dated.md", "x", "2026-10-06-x.md"),
            ("D.MD", " e ", "e.MD"),
            ("01_daily-logs/2026-10-06-daily-log.md", "standup", "2026-10-06-standup.md"),
        ] {
            let path = root.join(name);
            let shown = rename_request(&forest.nodes, &forest.sections, Some(row(&forest.nodes, path_str(&path))))
                .unwrap()
                .unwrap();
            assert_eq!(rename_preview(&path, &shown, typed).as_deref(), Some(want), "{name}");
            assert_eq!(rename_enter_at(&mut forest, &path, typed), RenameEnter::Done(None), "{name}");
            assert!(has_entry_named(path.parent().unwrap(), want), "{name}: {want} on disk");
            assert!(!path.exists(), "{name} moved");
        }
    }

    /// An empty or refused name has no preview, so the plain hint shows; the
    /// unchanged name previews the current one.
    #[test]
    fn rename_preview_is_none_for_an_empty_or_refused_name() {
        let path = Path::new("/v/2026-10-06-dated.md");
        for typed in ["", "  ", "b.md", "b.txt", "Bad_Name", "?!"] {
            assert_eq!(rename_preview(path, "dated", typed), None, "{typed:?}");
        }
        assert_eq!(rename_preview(path, "dated", " dated ").as_deref(), Some("2026-10-06-dated.md"));
        let kept = Path::new("/v/2026-10-06-My_Note.md");
        assert_eq!(rename_preview(kept, "My_Note", "My_Note").as_deref(), Some("2026-10-06-My_Note.md"));
        assert_eq!(rename_preview(path, "dated", "A b").as_deref(), Some("2026-10-06-a-b.md"));
    }

    /// NZ-76: the prompt's title names the row's kind.
    #[test]
    fn rename_title_names_the_rows_kind() {
        let (_dir, notez, root) = vault();
        for sub in ["01_daily-logs", "00_quick-notes", "todo"] {
            std::fs::create_dir_all(root.join(sub)).unwrap();
        }
        for file in [
            "01_daily-logs/2026-10-06-daily-log.md",
            "00_quick-notes/2026-10-06-q.md",
            "todo/TODO.md",
        ] {
            std::fs::write(root.join(file), "# n\n").unwrap();
        }
        let mut forest = forest_of(vault_sections(&notez));
        let config = Config::defaults();
        let title = |forest: &Forest, path: &str| {
            let node = &forest.nodes[row(&forest.nodes, path_str(&root.join(path)))];
            rename_title(node, &forest.sections, &config)
        };
        for (path, want) in [
            ("ideas", "rename directory"),
            ("ideas/deep", "rename directory"),
            ("01_daily-logs", "rename directory"),
            ("top.md", "rename note"),
            ("ideas/a.md", "rename note"),
            ("todo/TODO.md", "rename todo"),
            ("01_daily-logs/2026-10-06-daily-log.md", "rename log"),
            ("00_quick-notes/2026-10-06-q.md", "rename quick note"),
        ] {
            assert_eq!(title(&forest, path), want, "{path}");
        }
        forest.sections[0].is_doc = true;
        assert_eq!(title(&forest, "top.md"), "rename doc");
        assert_eq!(title(&forest, "todo/TODO.md"), "rename todo");
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

        for (name, shown) in [("2026-10-06-My_Note.md", "My_Note"), ("x.MD", "x")] {
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
