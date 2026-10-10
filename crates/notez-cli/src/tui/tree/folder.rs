//! The new-folder prompt (`N`).

use super::*;

// --- New folder ---

/// The footer message for `N` in a docs section: those are the repository's
/// own files, and `n` there writes to the personal store instead.
const FOLDER_IN_DOCS: &str = "new folder: not in a docs section";

/// The footer message for a folder name that sanitizes to nothing.
const FOLDER_NAME_EMPTY: &str = "new folder: the name is empty";

/// The prompt `N` opens for the row at `row`: the new-note prompt on the
/// same target (see [`open_new_note_prompt`]), creating a folder instead.
/// `Err` is the footer message for a row in a docs section.
pub(super) fn open_new_folder_prompt(
    nodes: &[TreeNode],
    sections: &[SectionSpec],
    row: Option<usize>,
    ctx: &TreeContext,
) -> std::result::Result<NewNotePrompt, &'static str> {
    let spec = row
        .and_then(|i| nodes.get(i))
        .and_then(|n| sections.get(n.section));
    if spec.is_some_and(|s| s.is_doc) {
        return Err(FOLDER_IN_DOCS);
    }
    let mut prompt = open_new_note_prompt(nodes, sections, row, ctx);
    prompt.is_folder = true;
    Ok(prompt)
}

/// The new-folder prompt that leads the footer while a name is typed.
pub(super) fn new_folder_lead(label: &str, buffer: &str) -> Vec<Span<'static>> {
    vec![
        Span::styled(
            format!(" new folder in {label}: "),
            Style::default().fg(theme::MAUVE),
        ),
        Span::styled(buffer.to_string(), Style::default().fg(theme::TEXT)),
        Span::styled("_", Style::default().fg(theme::OVERLAY)),
    ]
}

/// What a confirmed new folder leaves: the row for the cursor, if the
/// folder is listed, and the footer message, if any.
#[derive(Debug)]
pub(super) struct FolderOutcome {
    pub(super) row: Option<usize>,
    pub(super) message: Option<String>,
    /// Whether the forest was listed again from disk; only then may the
    /// file-change probe take a fresh reading ([`refresh_probe`]).
    pub(super) relisted: bool,
}

/// Create the folder `name` in `target` through [`mkdir::create_in_dir`],
/// the path `notez mkdir` takes. A name that sanitizes to nothing, or to
/// the name of anything already in the target (file or folder; a
/// case-insensitive file system matches regardless of case), is refused
/// before anything is created, so nothing is merged or overwritten. On
/// success the forest is rebuilt from `rebuild` and the new folder's row,
/// expanded with its ancestors, is returned.
pub(super) fn create_folder(
    forest: &mut Forest,
    target: &NewNoteTarget,
    name: &str,
    rebuild: &dyn Fn() -> Result<Vec<SectionSpec>>,
) -> FolderOutcome {
    let refuse = |message: String| FolderOutcome {
        row: None,
        message: Some(message),
        relisted: false,
    };
    let cleaned = sanitize::name(name);
    if cleaned.is_empty() {
        return refuse(FOLDER_NAME_EMPTY.to_string());
    }
    if std::fs::symlink_metadata(target.dir.join(&cleaned)).is_ok() {
        return refuse(format!(
            "new folder: {cleaned} already exists in {}",
            target.label
        ));
    }
    let path = match mkdir::create_in_dir(&target.dir, name, target.scope) {
        Ok(path) => path,
        Err(e) => return refuse(format!("new folder failed: {e:#}")),
    };
    let sections = match rebuild() {
        Ok(sections) => sections,
        Err(e) => {
            return refuse(format!(
                "created {}, but the list could not be refreshed: {e:#}",
                path.display()
            ));
        }
    };
    match forest.rebuild(sections, &path) {
        Some(row) => FolderOutcome {
            row: Some(row),
            message: None,
            relisted: true,
        },
        None => FolderOutcome {
            row: None,
            message: Some(format!("created {}", path.display())),
            relisted: true,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use super::*;

    #[test]
    fn new_folder_tab_cycles_the_scopes_as_n_does() {
        let sections = project_sections();
        let (nodes, _) = build_forest(&sections);
        let ctx = ctx_with(Some("proj"), roots());
        let at = Some(row(&nodes, "/n/personal/proj/ideas/a.md"));
        let note = open_new_note_prompt(&nodes, &sections, at, &ctx);
        let folder = open_new_folder_prompt(&nodes, &sections, at, &ctx).unwrap();
        assert_eq!(folder.project, note.project);
        let (mut a, mut b) = (folder.target.clone(), note.target.clone());
        for _ in 0..4 {
            a = next_scope_target(
                &a,
                &folder.origin,
                folder.project.as_deref(),
                &ctx.new_note_roots,
                Some("proj"),
            );
            b = next_scope_target(
                &b,
                &note.origin,
                note.project.as_deref(),
                &ctx.new_note_roots,
                Some("proj"),
            );
            assert_eq!(a, b);
        }
    }

    #[test]
    fn new_folder_in_a_docs_section_is_refused() {
        for path in ["/p/docs", "/p/docs/design", "/p/docs/design/c.md"] {
            assert_eq!(
                folder_prompt_at(path, Some("proj")).err(),
                Some(FOLDER_IN_DOCS),
                "{path}"
            );
        }
    }

    #[test]
    fn new_folder_on_an_empty_tree_targets_what_n_targets() {
        let ctx = ctx_with(Some("proj"), roots());
        let folder = open_new_folder_prompt(&[], &[], None, &ctx).unwrap();
        let note = open_new_note_prompt(&[], &[], None, &ctx);
        assert_eq!(folder.target, note.target);
        let past_end = open_new_folder_prompt(&[], &[], Some(0), &ctx).unwrap();
        assert_eq!(past_end.target, note.target);
    }

    #[test]
    fn new_folder_lead_names_the_scope_and_folder() {
        let lead = new_folder_lead("personal/ideas", "drafts");
        let rendered = text_of(&lead_with_hints(lead, Mode::NewItem, &[], 120));
        assert!(
            rendered.starts_with(" new folder in personal/ideas: drafts_"),
            "{rendered}"
        );
        assert!(
            rendered.contains("create") && rendered.contains("cancel"),
            "{rendered}"
        );
    }

    #[test]
    fn enter_creates_exactly_that_folder_selected_and_expanded_in_every_scope() {
        for (scope, name) in SCOPES {
            for sub in ["", "ideas"] {
                let (_dir, roots) = temp_tree();
                let rebuild = || Ok(list_sections_with_dirs(&roots));
                let mut forest = forest_of(list_sections_with_dirs(&roots));
                let target = folder_target(&roots, scope, sub);
                let before = all_files(&roots);

                let outcome = create_folder(&mut forest, &target, "New Stuff", &rebuild);

                let made = target.dir.join("new-stuff");
                assert!(made.is_dir(), "{name}/{sub}");
                assert_eq!(std::fs::read_dir(&made).unwrap().count(), 0, "{name}/{sub}");
                assert_eq!(all_files(&roots), before, "{name}/{sub}");
                assert_eq!(outcome.message, None, "{name}/{sub}");
                let row_idx = outcome.row.expect("the new folder is listed");
                let node = &forest.nodes[row_idx];
                assert_eq!(node.path, made, "{name}/{sub}");
                assert!(node.is_dir && node.expanded, "{name}/{sub}");
                assert!(
                    get_visible_nodes(&forest.nodes).contains(&row_idx),
                    "{name}/{sub}"
                );
            }
        }
    }

    #[test]
    fn a_new_local_folder_gets_the_scratch_gitignore_step() {
        let repo = tempfile::tempdir().unwrap();
        std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(repo.path())
            .status()
            .unwrap();
        let store = repo.path().join(".notez");
        std::fs::create_dir_all(&store).unwrap();
        std::fs::write(store.join("s.md"), "# s\n").unwrap();
        let roots = vec![(Scope::Local, store.clone())];
        let rebuild = || Ok(list_sections_with_dirs(&roots));
        let mut forest = forest_of(list_sections_with_dirs(&roots));
        let target = folder_target(&roots, Scope::Local, "");

        create_folder(&mut forest, &target, "scratchpad", &rebuild);

        assert!(store.join("scratchpad").is_dir());
        let ignore = std::fs::read_to_string(repo.path().join(".gitignore")).unwrap();
        assert!(ignore.lines().any(|l| l == ".notez"), "{ignore:?}");
    }

    #[test]
    fn an_empty_name_creates_nothing_and_says_so() {
        let (_dir, roots) = temp_tree();
        let rebuild = || -> Result<Vec<SectionSpec>> { panic!("nothing to rebuild") };
        let mut forest = forest_of(list_sections_with_dirs(&roots));
        let rows_before = forest.nodes.len();
        let target = folder_target(&roots, Scope::Personal, "");
        let dirs_before = all_dirs(&target.dir);

        for name in ["", "   ", "?!"] {
            let outcome = create_folder(&mut forest, &target, name, &rebuild);
            assert_eq!(
                outcome.message.as_deref(),
                Some(FOLDER_NAME_EMPTY),
                "{name:?}"
            );
            assert_eq!(outcome.row, None);
        }
        assert_eq!(all_dirs(&target.dir), dirs_before);
        assert_eq!(forest.nodes.len(), rows_before);
    }

    #[test]
    fn an_existing_name_is_refused_and_nothing_is_merged_or_overwritten() {
        let (_dir, roots) = temp_tree();
        let rebuild = || -> Result<Vec<SectionSpec>> { panic!("nothing to rebuild") };
        let mut forest = forest_of(list_sections_with_dirs(&roots));
        let target = folder_target(&roots, Scope::Personal, "");
        std::fs::write(target.dir.join("plain"), "not a note").unwrap();
        let files_before = all_files(&roots);
        let dirs_before = all_dirs(&target.dir);

        for name in ["ideas", "Ideas", "plain"] {
            let outcome = create_folder(&mut forest, &target, name, &rebuild);
            let message = outcome.message.expect("a refusal");
            assert!(message.contains("already exists"), "{name}: {message}");
            assert_eq!(outcome.row, None);
        }
        assert_eq!(all_files(&roots), files_before);
        assert_eq!(all_dirs(&target.dir), dirs_before);
        assert_eq!(
            std::fs::read_to_string(target.dir.join("plain")).unwrap(),
            "not a note"
        );
    }

    #[test]
    fn a_folder_the_rebuild_does_not_list_is_reported_by_path() {
        let (_dir, roots) = temp_tree();
        let rebuild = || Ok(list_sections(&roots));
        let mut forest = forest_of(list_sections(&roots));
        let target = folder_target(&roots, Scope::Global, "");

        let outcome = create_folder(&mut forest, &target, "unlisted", &rebuild);

        assert!(target.dir.join("unlisted").is_dir());
        assert_eq!(outcome.row, None);
        assert!(outcome.message.unwrap().contains("unlisted"));
    }

    #[test]
    fn new_folder_key_is_a_browse_key_in_the_table_and_the_help() {
        let rows: Vec<(usize, &KeyHint)> = TREE_KEYS
            .iter()
            .enumerate()
            .filter(|(_, k)| k.key == "N")
            .collect();
        assert_eq!(rows.len(), 1);
        let (idx, hint) = rows[0];
        assert_eq!(hint.modes, BROWSE);
        assert_eq!(hint.group, Group::Edit);
        assert!(hint.help.starts_with("new folder"), "{}", hint.help);
        assert!(help::rows(TREE_KEYS).contains(&help::Row::Key(idx)));
        let n = TREE_KEYS.iter().position(|k| k.key == "n").unwrap();
        assert_eq!(idx, n + 1, "listed right after n");
        assert!(shown_keys(Mode::Normal, &[], 200).contains(&"N"));
        assert!(shown_keys(Mode::Focus, &[], 200).contains(&"N"));
        for mode in [
            Mode::Tag,
            Mode::Filter,
            Mode::Rename,
            Mode::NewItem,
            Mode::ConfirmDelete,
            Mode::VimCommand,
        ] {
            assert!(!shown_keys(mode, &[], 200).contains(&"N"), "{mode:?}");
        }
    }

    /// The listing's directories survive a rebuild after a note delete: a
    /// folder emptied by the delete keeps its row and takes the cursor.
    #[test]
    fn deleting_the_last_note_in_a_listed_folder_keeps_the_folder() {
        let (_dir, roots) = temp_tree();
        let personal = roots[0].1.clone();
        std::fs::create_dir_all(personal.join("solo")).unwrap();
        std::fs::write(personal.join("solo/only.md"), "# o\n").unwrap();
        let rebuild = || Ok(list_sections_with_dirs(&roots));
        let mut forest = forest_of(list_sections_with_dirs(&roots));
        let mut retired = Vec::new();
        let prompt = prompt_at(&mut forest, &personal.join("solo/only.md"));

        let outcome = answer_delete(
            KeyCode::Char('y'),
            &mut forest,
            &mut retired,
            &prompt,
            "",
            &rebuild,
        )
        .unwrap();

        let solo = row(&forest.nodes, path_str(&personal.join("solo")));
        assert_eq!(outcome.row, Some(solo));
    }
}
