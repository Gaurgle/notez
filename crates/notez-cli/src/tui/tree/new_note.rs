//! The new-note prompt (`n`): its target, `Tab` scope cycling, the empty
//! state and the typed-name checks shared with the other prompts.

use super::*;

// --- New note ---

/// Where a note created from the browser goes: the directory, the scope the
/// note gets (it decides the scratch gitignore step) and the label the
/// prompt shows, so the target is named before anything is created.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct NewNoteTarget {
    pub(super) dir: PathBuf,
    pub(super) scope: Scope,
    pub(super) label: String,
}

/// The open new-note prompt. It owns its target, so switching the scope
/// replaces `target` and leaves the typed title alone. `origin` is the
/// target the prompt opened on, which `Tab` returns to after a full cycle,
/// and `project` the project whose scopes `Tab` cycles through.
/// `is_folder` marks the same prompt opened by `N`, which creates a folder
/// named by the buffer instead of a note.
pub(super) struct NewNotePrompt {
    pub(super) target: NewNoteTarget,
    pub(super) origin: NewNoteTarget,
    pub(super) project: Option<String>,
    pub(super) buffer: String,
    pub(super) is_folder: bool,
}

impl NewNotePrompt {
    pub(super) fn open(target: NewNoteTarget, project: Option<String>) -> Self {
        Self {
            origin: target.clone(),
            target,
            project,
            buffer: String::new(),
            is_folder: false,
        }
    }
}

/// The target of `n` when the tree has no rows: the current project's
/// personal root inside a project, the global root outside one. Personal,
/// not public, so nothing is published by accident.
fn empty_tree_target(roots: &NewNoteRoots, current_project: Option<&str>) -> NewNoteTarget {
    let Some(project) = current_project else {
        return NewNoteTarget {
            dir: roots.global.clone(),
            scope: Scope::Global,
            label: scope_label(Scope::Global, None, None),
        };
    };
    let dir = roots
        .projects
        .get(project)
        .and_then(|stores| stores.iter().find(|(s, _)| *s == Scope::Personal))
        .map(|(_, dir)| dir.clone())
        .unwrap_or_else(|| roots.global.join("personal").join(project));
    NewNoteTarget {
        dir,
        scope: Scope::Personal,
        label: scope_label(Scope::Personal, Some(project), current_project),
    }
}

/// The list pane's text when the view has no notes at all.
const EMPTY_STATE: &str = "no notes here yet: n creates one";

/// The empty-state line, shown in place of the list when there are no rows.
pub(super) fn empty_state_line(nodes: &[TreeNode]) -> Option<&'static str> {
    nodes.is_empty().then_some(EMPTY_STATE)
}

/// The empty-state text for the view titled `title`. A narrowed view
/// (`-p`, `-l`, `-g`) is titled `<scope icon> notez (...)` by
/// `commands::tree`, so its scope is read from that icon and named:
/// `no scratch notes here yet: n creates one`. The all view's title starts
/// with `notez` and keeps [`EMPTY_STATE`].
pub(super) fn empty_state_text(title: &str) -> String {
    let narrowed = [Scope::Personal, Scope::Public, Scope::Local, Scope::Global]
        .into_iter()
        .find(|scope| title.starts_with(scope.icon()));
    // `Scope::label()` calls the global scope `notez`; "global" reads better here.
    let word = match narrowed {
        Some(Scope::Global) => "global",
        Some(scope) => scope.label(),
        None => return EMPTY_STATE.to_string(),
    };
    format!("no {word} notes here yet: n creates one")
}

/// The prompt `n` opens for the row at `row`: the folder under the cursor
/// in that row's section, or [`empty_tree_target`] when no row is under
/// the cursor (an empty tree, or a filter that hides every row).
pub(super) fn open_new_note_prompt(
    nodes: &[TreeNode],
    sections: &[SectionSpec],
    row: Option<usize>,
    ctx: &TreeContext,
) -> NewNotePrompt {
    let current = ctx.current_project.as_deref();
    let at_row = row.and_then(|i| {
        let target = new_note_target(nodes, sections, i, current)?;
        Some((target, sections[nodes[i].section].project.clone()))
    });
    match at_row {
        Some((target, project)) => NewNotePrompt::open(target, project),
        None => NewNotePrompt::open(
            empty_tree_target(&ctx.new_note_roots, current),
            ctx.current_project.clone(),
        ),
    }
}

/// The target after one `Tab` in the prompt. The scopes cycle in the order
/// personal, public, local, global, limited to the ones that apply: the
/// project's known stores plus global, or global alone for a row outside
/// any project. Each step targets the scope's root; arriving back at the
/// origin's scope restores the origin itself, folder included.
pub(super) fn next_scope_target(
    current: &NewNoteTarget,
    origin: &NewNoteTarget,
    project: Option<&str>,
    roots: &NewNoteRoots,
    current_project: Option<&str>,
) -> NewNoteTarget {
    let mut cycle: Vec<(Scope, PathBuf)> = project
        .and_then(|p| roots.projects.get(p))
        .cloned()
        .unwrap_or_default();
    cycle.push((Scope::Global, roots.global.clone()));
    if !cycle.iter().any(|(s, _)| *s == origin.scope) {
        cycle.insert(0, (origin.scope, origin.dir.clone()));
    }
    let pos = cycle
        .iter()
        .position(|(s, _)| *s == current.scope)
        .unwrap_or(0);
    let (scope, dir) = cycle[(pos + 1) % cycle.len()].clone();
    if scope == origin.scope {
        return origin.clone();
    }
    NewNoteTarget {
        label: scope_label(scope, project, current_project),
        dir,
        scope,
    }
}

/// Human name of a scope in the prompt. Public says what it means: those
/// notes are committed with the project repository. A project other than
/// `current_project` is named, so the global view is unambiguous.
pub(super) fn scope_label(
    scope: Scope,
    project: Option<&str>,
    current_project: Option<&str>,
) -> String {
    let other = project.filter(|p| Some(*p) != current_project);
    match (scope, other) {
        (Scope::Public, None) => "public (committed with the project)".to_string(),
        (Scope::Public, Some(p)) => format!("public (committed with {p})"),
        (Scope::Personal, None) => "personal".to_string(),
        (Scope::Personal, Some(p)) => format!("personal ({p})"),
        (Scope::Local, None) => "local scratch".to_string(),
        (Scope::Local, Some(p)) => format!("local scratch ({p})"),
        (Scope::Global, _) => "global".to_string(),
    }
}

/// Resolve the row at `idx` to the new note's target: a directory row is the
/// target itself, a file row its parent directory, a section row its root.
/// In a docs section the target is the project's personal root instead.
/// `None` when there is no such row.
fn new_note_target(
    nodes: &[TreeNode],
    sections: &[SectionSpec],
    idx: usize,
    current_project: Option<&str>,
) -> Option<NewNoteTarget> {
    let node = nodes.get(idx)?;
    let spec = sections.get(node.section)?;
    let project = spec.project.as_deref();
    if spec.is_doc {
        return Some(NewNoteTarget {
            dir: spec.new_note_root.clone(),
            scope: Scope::Personal,
            label: scope_label(Scope::Personal, project, current_project),
        });
    }
    let dir = if node.is_dir {
        node.path.clone()
    } else {
        node.path.parent().unwrap_or(&spec.root).to_path_buf()
    };
    let mut label = scope_label(spec.scope, project, current_project);
    if let Ok(rel) = dir.strip_prefix(&spec.root) {
        let rel = rel.to_string_lossy();
        if !rel.is_empty() {
            label = format!("{label}/{rel}");
        }
    }
    Some(NewNoteTarget {
        dir,
        scope: spec.scope,
        label,
    })
}

// --- Typed names ---
//
// The prompts `n`, `N` and `r` refuse a typed name that `sanitize::name`
// would drop characters from (`_`, `.`), with what it would have become,
// instead of changing it without a word. Lowercasing and blanks to `-` stay
// silent (NZ-26). The CLI commands keep sanitizing.

/// The soft form of a typed name: `sanitize::name`'s steps before its
/// character filter (trim, lowercase, whitespace runs to `-`), in its order,
/// so filtering this form gives exactly `sanitize::name(input)`.
fn soft_name(input: &str) -> String {
    input
        .trim()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-")
}

/// What `sanitize::name` makes of `input` when that drops characters from
/// its [`soft_name`] form; `None` when the name is taken (lowercasing and
/// blanks to `-` are silent). A name that sanitizes to nothing is `None`
/// too: the prompts' empty-name paths answer it.
pub(super) fn name_would_change(input: &str) -> Option<String> {
    let cleaned = sanitize::name(input);
    (!cleaned.is_empty() && cleaned != soft_name(input)).then_some(cleaned)
}

/// Whether `dir` is a quick-notes folder, where new notes get a date prefix.
pub(super) fn is_quick_notes_dir(dir: &Path, config: &Config) -> bool {
    dir.file_name()
        .is_some_and(|name| name.to_string_lossy() == config.paths.quick_notes_dir)
}

/// The footer message for a typed name [`name_would_change`] refuses.
pub(super) fn altered_name_message(cleaned: &str) -> String {
    format!("name would become {cleaned}; use letters, digits and -")
}

/// The footer message for a new-note title that sanitizes to nothing. An
/// empty title still makes an `untitled` note.
const NOTE_NAME_EMPTY: &str = "new note: the name is empty";

/// Why `Enter` in the `n` or `N` prompt creates nothing and leaves the
/// prompt open with the typed name: an altered name (see
/// [`name_would_change`]), or a note title that is not empty but sanitizes
/// to nothing. `None` goes on to create.
pub(super) fn new_item_refusal(prompt: &NewNotePrompt) -> Option<String> {
    let buffer = prompt.buffer.as_str();
    if !prompt.is_folder && !buffer.trim().is_empty() && sanitize::name(buffer).is_empty() {
        return Some(NOTE_NAME_EMPTY.to_string());
    }
    name_would_change(buffer).map(|cleaned| altered_name_message(&cleaned))
}

/// The new-note prompt that leads the footer while a title is typed.
pub(super) fn new_note_lead(label: &str, buffer: &str) -> Vec<Span<'static>> {
    vec![
        Span::styled(
            format!(" new note in {label}: "),
            Style::default().fg(theme::MAUVE),
        ),
        Span::styled(buffer.to_string(), Style::default().fg(theme::TEXT)),
        Span::styled("_", Style::default().fg(theme::OVERLAY)),
    ]
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use super::*;
    use notez_core::tags::FLAG_IMPORTANT;

    // --- New note ---

    fn target_at(path: &str, current: Option<&str>) -> NewNoteTarget {
        let sections = project_sections();
        let (nodes, _) = build_forest(&sections);
        new_note_target(&nodes, &sections, row(&nodes, path), current).unwrap()
    }

    #[test]
    fn new_note_targets_the_folder_under_the_cursor_in_its_section() {
        let here = Some("proj");
        let ideas = target("/n/personal/proj/ideas", Scope::Personal, "personal/ideas");
        assert_eq!(target_at("/n/personal/proj/ideas", here), ideas);
        assert_eq!(target_at("/n/personal/proj/ideas/a.md", here), ideas);
        let personal_root = target("/n/personal/proj", Scope::Personal, "personal");
        assert_eq!(target_at("/n/personal/proj", here), personal_root);
        assert_eq!(target_at("/n/personal/proj/top.md", here), personal_root);
        assert_eq!(
            target_at("/p/notez/plans/b.md", here),
            target(
                "/p/notez/plans",
                Scope::Public,
                "public (committed with the project)/plans"
            )
        );
        assert_eq!(
            target_at("/p/notez", here),
            target(
                "/p/notez",
                Scope::Public,
                "public (committed with the project)"
            )
        );
        assert_eq!(
            target_at("/p/.notez/d.md", here),
            target("/p/.notez", Scope::Local, "local scratch")
        );
        assert_eq!(
            target_at("/n/e.md", here),
            target("/n", Scope::Global, "global")
        );
        assert_eq!(target_at("/n", here), target("/n", Scope::Global, "global"));
    }

    #[test]
    fn new_note_from_a_docs_section_goes_to_the_personal_root() {
        for path in ["/p/docs", "/p/docs/design", "/p/docs/design/c.md"] {
            assert_eq!(
                target_at(path, Some("proj")),
                target("/n/personal/proj", Scope::Personal, "personal"),
                "{path}"
            );
        }
    }

    #[test]
    fn new_note_label_names_a_project_other_than_the_current_one() {
        for current in [None, Some("other")] {
            assert_eq!(
                target_at("/n/personal/proj/ideas", current).label,
                "personal (proj)/ideas"
            );
            assert_eq!(
                target_at("/p/notez/plans", current).label,
                "public (committed with proj)/plans"
            );
            assert_eq!(
                target_at("/p/.notez", current).label,
                "local scratch (proj)"
            );
            assert_eq!(target_at("/n", current).label, "global");
        }
    }

    #[test]
    fn new_note_label_says_public_for_every_public_row() {
        let sections = project_sections();
        let (nodes, _) = build_forest(&sections);
        for (i, node) in nodes.iter().enumerate() {
            for current in [None, Some("proj"), Some("other")] {
                let t = new_note_target(&nodes, &sections, i, current).unwrap();
                if t.scope == Scope::Public {
                    assert!(
                        t.label.starts_with("public (committed with"),
                        "{:?}",
                        node.path
                    );
                    assert!(t.dir.starts_with("/p/notez"), "{:?}", node.path);
                }
            }
        }
    }

    #[test]
    fn new_note_target_is_none_past_the_last_row() {
        let sections = project_sections();
        let (nodes, _) = build_forest(&sections);
        assert_eq!(new_note_target(&nodes, &sections, nodes.len(), None), None);
        assert_eq!(new_note_target(&[], &[], 0, None), None);
    }

    #[test]
    fn rebuild_keeps_expansion_and_tags_and_selects_the_new_note() {
        let before = vec![spec("/r", "S", &["a/one.md", "b/two.md", "c/three.md"])];
        let (mut old, _) = build_forest(&before);
        let wrapper = row(&old, "/r");
        old[wrapper].expanded = true;
        let b = row(&old, "/r/b");
        old[b].expanded = true;
        let two = row(&old, "/r/b/two.md");
        old[two].flags = FLAG_IMPORTANT;
        let three = row(&old, "/r/c/three.md");
        old[three].origin = PathBuf::from("/r/c/old-three.md");

        let after = vec![spec(
            "/r",
            "S",
            &["a/new.md", "a/one.md", "b/two.md", "c/three.md"],
        )];
        let (mut new, _) = build_forest(&after);
        let selected = restore_state(&old, &mut new, Path::new("/r/a/new.md"));

        assert_eq!(selected, Some(row(&new, "/r/a/new.md")));
        assert!(new[row(&new, "/r")].expanded);
        assert!(
            new[row(&new, "/r/a")].expanded,
            "the new note's folder opens"
        );
        assert!(new[row(&new, "/r/b")].expanded, "an open folder stays open");
        assert!(
            !new[row(&new, "/r/c")].expanded,
            "a closed folder stays closed"
        );
        assert_eq!(new[row(&new, "/r/b/two.md")].flags, FLAG_IMPORTANT);
        assert_eq!(
            new[row(&new, "/r/c/three.md")].origin,
            PathBuf::from("/r/c/old-three.md")
        );
        assert!(get_visible_nodes(&new).contains(&selected.unwrap()));
    }

    #[test]
    fn n_is_a_browse_key_listed_in_help() {
        let rows: Vec<&KeyHint> = TREE_KEYS.iter().filter(|k| k.key == "n").collect();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].modes, BROWSE);
        assert_eq!(rows[0].group, Group::Edit);
        let prompt_keys: Vec<&str> = TREE_KEYS
            .iter()
            .filter(|k| k.modes.contains(&Mode::NewItem))
            .map(|k| k.key)
            .collect();
        assert_eq!(prompt_keys, vec!["enter", "esc", "tab", "bksp"]);
    }

    /// `n` is a primary action: the normal footer shows it, and on a narrow
    /// terminal it drops after the other keys but before open and tags.
    #[test]
    fn n_shows_in_the_footer_and_drops_before_open_and_tags() {
        assert!(shown_keys(Mode::Normal, &[], 200).contains(&"n"));
        assert!(shown_keys(Mode::Focus, &[], 200).contains(&"n"));
        assert!(!shown_keys(Mode::Tag, &[], 200).contains(&"n"));
        let mut saw_n_without_r = false;
        for width in 20..200 {
            let keys = shown_keys(Mode::Normal, &[], width);
            if keys.contains(&"n") {
                assert!(
                    keys.contains(&"o") && keys.contains(&"t"),
                    "width {width}: {keys:?}"
                );
                saw_n_without_r |= !keys.contains(&"r");
            }
        }
        assert!(saw_n_without_r, "rename drops before new");
    }

    // --- Tab scope cycling and the empty tree ---

    /// Press `Tab` `presses` times on the prompt `n` opens at `path`; returns
    /// the target after each press.
    fn tab_through(path: &str, current: Option<&str>, presses: usize) -> Vec<NewNoteTarget> {
        let sections = project_sections();
        let (nodes, _) = build_forest(&sections);
        let ctx = ctx_with(current, roots());
        let mut prompt = open_new_note_prompt(&nodes, &sections, Some(row(&nodes, path)), &ctx);
        (0..presses)
            .map(|_| {
                prompt.target = next_scope_target(
                    &prompt.target,
                    &prompt.origin,
                    prompt.project.as_deref(),
                    &ctx.new_note_roots,
                    current,
                );
                prompt.target.clone()
            })
            .collect()
    }

    #[test]
    fn tab_cycles_scope_roots_and_returns_to_the_original_folder() {
        let public = "public (committed with the project)";
        assert_eq!(
            tab_through("/n/personal/proj/ideas/a.md", Some("proj"), 4),
            vec![
                target("/p/notez", Scope::Public, public),
                target("/p/.notez", Scope::Local, "local scratch"),
                target("/n", Scope::Global, "global"),
                target("/n/personal/proj/ideas", Scope::Personal, "personal/ideas"),
            ]
        );
        assert_eq!(
            tab_through("/p/notez/plans", Some("proj"), 4),
            vec![
                target("/p/.notez", Scope::Local, "local scratch"),
                target("/n", Scope::Global, "global"),
                target("/n/personal/proj", Scope::Personal, "personal"),
                target("/p/notez/plans", Scope::Public, &format!("{public}/plans")),
            ]
        );
    }

    #[test]
    fn tab_from_a_docs_section_starts_at_the_personal_root() {
        let cycle = tab_through("/p/docs/design/c.md", Some("proj"), 4);
        assert_eq!(cycle[0].scope, Scope::Public);
        assert_eq!(
            cycle[3],
            target("/n/personal/proj", Scope::Personal, "personal")
        );
    }

    #[test]
    fn tab_names_the_project_in_another_projects_cycle() {
        let cycle = tab_through("/p/.notez/d.md", None, 2);
        assert_eq!(cycle[0], target("/n", Scope::Global, "global"));
        assert_eq!(
            cycle[1],
            target("/n/personal/proj", Scope::Personal, "personal (proj)")
        );
    }

    #[test]
    fn tab_outside_any_project_stays_global() {
        assert_eq!(
            tab_through("/n/e.md", Some("proj"), 2),
            vec![target("/n", Scope::Global, "global"); 2]
        );
    }

    /// A project the browser knows no repository for offers its personal
    /// store and global only: no public or scratch path is guessed.
    #[test]
    fn tab_without_a_known_repository_offers_personal_and_global() {
        let sections = project_sections();
        let (nodes, _) = build_forest(&sections);
        let ctx = ctx_with(
            None,
            NewNoteRoots {
                global: PathBuf::from("/n"),
                ..NewNoteRoots::default()
            },
        );
        let prompt = open_new_note_prompt(
            &nodes,
            &sections,
            Some(row(&nodes, "/n/personal/proj")),
            &ctx,
        );
        let next = next_scope_target(
            &prompt.target,
            &prompt.origin,
            Some("proj"),
            &ctx.new_note_roots,
            None,
        );
        assert_eq!(next, target("/n", Scope::Global, "global"));
        let back = next_scope_target(
            &next,
            &prompt.origin,
            Some("proj"),
            &ctx.new_note_roots,
            None,
        );
        assert_eq!(back, prompt.origin);
    }

    #[test]
    fn n_on_an_empty_tree_targets_the_personal_root_in_a_project() {
        let (nodes, _) = build_forest(&[]);
        assert!(nodes.is_empty());
        let in_project = open_new_note_prompt(&nodes, &[], None, &ctx_with(Some("proj"), roots()));
        assert_eq!(
            in_project.target,
            target("/n/personal/proj", Scope::Personal, "personal")
        );
        assert_eq!(in_project.project.as_deref(), Some("proj"));
        let outside = open_new_note_prompt(&nodes, &[], None, &ctx_with(None, roots()));
        assert_eq!(outside.target, target("/n", Scope::Global, "global"));
        let unknown = open_new_note_prompt(&nodes, &[], Some(0), &ctx_with(Some("x"), roots()));
        assert_eq!(
            unknown.target,
            target("/n/personal/x", Scope::Personal, "personal")
        );
    }

    /// The helpers the event loop calls on every key and frame accept an
    /// empty forest without indexing into it.
    #[test]
    fn empty_tree_helpers_do_not_panic() {
        let (mut nodes, roots) = build_forest(&[]);
        assert!(roots.is_empty());
        assert!(get_visible_nodes(&nodes).is_empty());
        assert!(compute_visible(&nodes, "").is_empty());
        assert!(compute_visible(&nodes, "#1 x").is_empty());
        assert_eq!(find_top_dir(&nodes, 0), None);
        assert!(!any_top_collapsed(&nodes));
        assert!(!view_all_lit(&nodes));
        assert_eq!(restore_state(&[], &mut nodes, Path::new("/n/a.md")), None);
        assert!(remap_rows(&[], &nodes, &[(0, true)]).is_empty());
        assert_eq!(empty_state_line(&nodes), Some(EMPTY_STATE));
        let (some, _) = build_forest(&project_sections());
        assert_eq!(empty_state_line(&some), None);
    }

    #[test]
    fn a_narrowed_empty_view_names_its_scope() {
        let cases = [
            (Scope::Personal, "no personal notes here yet: n creates one"),
            (Scope::Public, "no public notes here yet: n creates one"),
            (Scope::Local, "no scratch notes here yet: n creates one"),
            (Scope::Global, "no global notes here yet: n creates one"),
        ];
        for (scope, expected) in cases {
            let title = format!("{} notez (proj)", scope.icon());
            assert_eq!(empty_state_text(&title), expected);
        }
    }

    #[test]
    fn the_all_view_keeps_the_plain_empty_state() {
        assert_eq!(empty_state_text("notez (proj)"), EMPTY_STATE);
        assert_eq!(empty_state_text("notez"), EMPTY_STATE);
        assert_eq!(empty_state_text(""), EMPTY_STATE);
    }

    // --- Refused names (NZ-20) ---

    /// Typed names sanitizing would drop characters from, each with what it
    /// becomes.
    const ALTERED: [(&str, &str); 2] = [("00_quick", "00quick"), ("a.b", "ab")];

    /// Typed names the prompts take as they are.
    const ACCEPTED: [&str; 3] = ["00-quick", "my-note", "ideas"];

    /// Typed names the prompts accept although sanitizing lowercases them or
    /// turns their blanks into `-` (NZ-26), each with what it becomes.
    const SOFTENED: [(&str, &str); 2] = [("My Note", "my-note"), ("Ideas", "ideas")];

    #[test]
    fn soft_name_lowercases_and_hyphenates_without_dropping_anything() {
        assert_eq!(soft_name("My Note"), "my-note");
        assert_eq!(soft_name("  My \t  Big\nNote  "), "my-big-note");
        assert_eq!(soft_name("00_quick"), "00_quick");
        assert_eq!(soft_name("a.b"), "a.b");
        assert_eq!(soft_name("\u{c4}"), "\u{e4}");
        assert_eq!(soft_name("   "), "");
        for typed in [
            "00_quick", "a.b", "My Note", "Ideas", "!!!", "\u{c4}", "a\u{308}", "  x  y ",
        ] {
            let soft = soft_name(typed);
            let filtered: String = soft
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == '-')
                .collect();
            assert_eq!(
                filtered,
                sanitize::name(typed),
                "soft form then filter is sanitize::name: {typed}"
            );
        }
    }

    #[test]
    fn name_would_change_names_what_sanitizing_makes_of_an_altered_name() {
        for (typed, cleaned) in ALTERED {
            assert_eq!(
                name_would_change(typed).as_deref(),
                Some(cleaned),
                "{typed}"
            );
        }
        for (typed, _) in SOFTENED {
            assert_eq!(
                name_would_change(typed),
                None,
                "{typed}: lowercasing and hyphens are silent"
            );
        }
        assert_eq!(
            name_would_change("\u{c4}"),
            None,
            "capital A umlaut lowercases silently"
        );
        // A decomposed umlaut loses its combining mark, as before NZ-26.
        assert_eq!(
            name_would_change("a\u{308}").as_deref(),
            Some("a"),
            "decomposed a umlaut"
        );
        for typed in ACCEPTED {
            assert_eq!(name_would_change(typed), None, "{typed}");
        }
        assert_eq!(
            name_would_change("  ideas  "),
            None,
            "surrounding blanks are trimmed"
        );
        assert_eq!(
            name_would_change("\u{e5}\u{e4}\u{f6}"),
            None,
            "lowercase letters beyond ASCII"
        );
        assert_eq!(
            name_would_change(""),
            None,
            "an empty name is the empty-name path's"
        );
        assert_eq!(
            name_would_change("!!!"),
            None,
            "so is one that sanitizes to nothing"
        );
        assert_eq!(
            altered_name_message("00quick"),
            "name would become 00quick; use letters, digits and -"
        );
    }

    #[test]
    fn new_note_and_new_folder_enter_refuses_altered_names_and_keeps_the_rest() {
        let (_dir, notez, root) = vault();
        let target = NewNoteTarget {
            dir: root.clone(),
            scope: Scope::Personal,
            label: "personal".to_string(),
        };
        for is_folder in [false, true] {
            let mut prompt = NewNotePrompt::open(target.clone(), Some("proj".to_string()));
            prompt.is_folder = is_folder;
            for (typed, cleaned) in ALTERED {
                prompt.buffer = typed.to_string();
                assert_eq!(
                    new_item_refusal(&prompt),
                    Some(altered_name_message(cleaned)),
                    "{typed}"
                );
            }
            let softened = SOFTENED.map(|(typed, _)| typed);
            for typed in ACCEPTED
                .into_iter()
                .chain(softened)
                .chain(["", "  ideas  "])
            {
                prompt.buffer = typed.to_string();
                assert_eq!(
                    new_item_refusal(&prompt),
                    None,
                    "{typed}, folder {is_folder}"
                );
            }
        }
        let mut note = NewNotePrompt::open(target.clone(), None);
        note.buffer = "!!!".to_string();
        assert_eq!(new_item_refusal(&note).as_deref(), Some(NOTE_NAME_EMPTY));
        let mut folder = NewNotePrompt::open(target.clone(), None);
        folder.is_folder = true;
        folder.buffer = "!!!".to_string();
        assert_eq!(
            new_item_refusal(&folder),
            None,
            "create_folder answers FOLDER_NAME_EMPTY"
        );

        // An accepted folder name is created under exactly that name.
        let mut forest = forest_of(vault_sections(&notez));
        let rebuild = || Ok(vault_sections(&notez));
        let plans = NewNoteTarget {
            dir: root.join("plans"),
            ..target
        };
        for name in ACCEPTED {
            let outcome = create_folder(&mut forest, &plans, name, &rebuild);
            assert_eq!(outcome.message, None, "{name}");
            assert!(has_entry_named(&plans.dir, name), "{name}");
        }
    }

    /// New notes are dated only in the quick-notes folder, found by its
    /// configured name rather than a hard-coded one.
    #[test]
    fn only_the_quick_notes_folder_gets_dated_notes() {
        let mut config = Config::defaults();
        assert!(is_quick_notes_dir(Path::new("/v/00_quick-notes"), &config));
        assert!(!is_quick_notes_dir(Path::new("/v/personal/notez"), &config));
        assert!(!is_quick_notes_dir(Path::new("/"), &config));

        config.paths.quick_notes_dir = "jots".to_string();
        assert!(is_quick_notes_dir(Path::new("/v/jots"), &config));
        assert!(!is_quick_notes_dir(Path::new("/v/00_quick-notes"), &config));
    }

    /// NZ-26: `n` takes `My Note` as `notez add` does (file `my-note`,
    /// heading as typed), `N` takes `Big Plans` as the folder `big-plans`;
    /// `00_quick` stays refused in both.
    #[test]
    fn new_note_and_new_folder_enter_accept_a_title_with_capitals_and_blanks() {
        let (_dir, notez, root) = vault();
        let target = NewNoteTarget {
            dir: root.clone(),
            scope: Scope::Personal,
            label: "personal".to_string(),
        };
        let mut note = NewNotePrompt::open(target.clone(), None);
        note.buffer = "00_quick".to_string();
        assert_eq!(
            new_item_refusal(&note),
            Some(altered_name_message("00quick"))
        );
        note.buffer = "My Note".to_string();
        assert_eq!(new_item_refusal(&note), None);
        let words = note.buffer.split_whitespace().map(String::from).collect();
        let created = add::create_in_dir(words, &target.dir, target.scope, false).unwrap();
        assert_eq!(file_name_of(&created.path), "my-note.md");
        let content = std::fs::read_to_string(&created.path).unwrap();
        assert!(content.starts_with("# My Note\n"), "{content}");

        let mut forest = forest_of(vault_sections(&notez));
        let rebuild = || Ok(vault_sections(&notez));
        let mut folder = NewNotePrompt::open(target.clone(), None);
        folder.is_folder = true;
        folder.buffer = "00_quick".to_string();
        assert_eq!(
            new_item_refusal(&folder),
            Some(altered_name_message("00quick"))
        );
        folder.buffer = "Big Plans".to_string();
        assert_eq!(new_item_refusal(&folder), None);
        let outcome = create_folder(&mut forest, &target, &folder.buffer, &rebuild);
        assert_eq!(outcome.message, None);
        assert!(root.join("big-plans").is_dir());
        assert!(!has_entry_named(&root, "00quick") && !has_entry_named(&root, "00_quick"));
    }

    #[test]
    fn rename_enter_refuses_altered_names_for_notes_and_folders_and_keeps_the_prompt() {
        let (_dir, notez, root) = vault();
        let mut forest = forest_of(vault_sections(&notez));
        let before = disk_entries(&notez);
        for path in [root.join("ideas"), root.join("top.md")] {
            for (typed, cleaned) in ALTERED {
                assert_eq!(
                    rename_enter_at(&mut forest, &path, typed),
                    RenameEnter::Keep(altered_name_message(cleaned)),
                    "{typed} on {}",
                    path.display()
                );
            }
        }
        assert_eq!(disk_entries(&notez), before, "nothing renamed");

        assert_eq!(
            rename_enter_at(&mut forest, &root.join("ideas"), "my-ideas"),
            RenameEnter::Done(None)
        );
        assert!(root.join("my-ideas/a.md").is_file());
        assert_eq!(
            rename_enter_at(&mut forest, &root.join("top.md"), "00-quick"),
            RenameEnter::Done(None)
        );
        assert!(root.join("00-quick.md").is_file() && !root.join("top.md").exists());
        let renamed = row(&forest.nodes, path_str(&root.join("00-quick.md")));
        assert_eq!(forest.nodes[renamed].name, "00-quick.md");
    }
}
