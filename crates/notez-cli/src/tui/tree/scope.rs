//! The view scope `s` cycles: which sections the tree shows. Rows keep
//! their order and their state; a section outside the scope and every row
//! under it are only left out of the visible rows (see
//! [`get_visible_nodes`]).

use super::*;

/// The sections the tree shows, in `s` order. `f` (focus one section) is
/// separate and works inside any scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum ViewScope {
    /// Every section.
    #[default]
    All,
    /// The current repository's `.notez/`.
    Scratch,
    /// The current repository's personal, public and docs sections.
    Project,
    /// The vault's global notes.
    Global,
}

impl ViewScope {
    /// The scope after this one: all, scratch, project, global, all.
    pub(super) fn next(self) -> Self {
        match self {
            Self::All => Self::Scratch,
            Self::Scratch => Self::Project,
            Self::Project => Self::Global,
            Self::Global => Self::All,
        }
    }

    /// The name the header shows.
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Scratch => "scratch",
            Self::Project => "project",
            Self::Global => "global",
        }
    }

    /// Whether `spec` is shown in this scope. Other projects' sections show
    /// only in [`Self::All`].
    pub(super) fn includes(self, spec: &SectionSpec) -> bool {
        match self {
            Self::All => true,
            Self::Scratch => spec.is_current && !spec.is_doc && spec.scope == Scope::Local,
            Self::Project => {
                spec.is_current
                    && (spec.is_doc || matches!(spec.scope, Scope::Personal | Scope::Public))
            }
            Self::Global => spec.scope == Scope::Global && spec.project.is_none(),
        }
    }
}

/// Mark each section row hidden or not for `scope`. Only section rows
/// (depth 0) carry the mark; [`get_visible_nodes`] reads it for the rows
/// under them. [`Forest::set_scope`] calls it when the scope changes and
/// every rebuild calls it on the new rows, so a cursor row chosen after
/// either already sees the marks.
pub(super) fn apply_scope(nodes: &mut [TreeNode], sections: &[SectionSpec], scope: ViewScope) {
    for node in nodes.iter_mut().filter(|n| n.depth == 0) {
        node.hidden = !sections
            .get(node.section)
            .is_some_and(|spec| scope.includes(spec));
    }
}

/// Whether `row` belongs to a section the view scope shows.
fn in_shown_section(nodes: &[TreeNode], row: usize) -> bool {
    find_top_dir(nodes, row).is_none_or(|top| !nodes[top].hidden)
}

/// The notes in the sections the scope shows, for the header's count.
pub(super) fn notes_in_scope(nodes: &[TreeNode]) -> usize {
    nodes
        .iter()
        .enumerate()
        .filter(|&(row, node)| !node.is_dir && in_shown_section(nodes, row))
        .count()
}

/// Drop the marks on rows in sections the view scope hides, so a bulk
/// `d`, `m` or `S` only ever acts on rows that can be seen.
pub(super) fn drop_hidden_marks(marks: &mut HashSet<PathBuf>, nodes: &[TreeNode]) {
    if marks.is_empty() {
        return;
    }
    let hidden: HashSet<&Path> = nodes
        .iter()
        .enumerate()
        .filter(|&(row, _)| !in_shown_section(nodes, row))
        .map(|(_, node)| node.path.as_path())
        .collect();
    marks.retain(|path| !hidden.contains(path.as_path()));
}

/// What [`reveal_row`] changed to bring a row into view.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) struct Revealed {
    /// The row's section was hidden: the view moved to a scope showing it.
    pub(super) scope_changed: bool,
    /// The filter hid the row, so it was cleared.
    pub(super) filter_cleared: bool,
}

impl Revealed {
    /// The status line for a new `what` ("note", "folder") brought into
    /// view this way, `None` when nothing changed. `scope` is the scope now
    /// shown.
    pub(super) fn message(self, what: &str, scope: ViewScope) -> Option<String> {
        let scope = scope.label();
        match (self.scope_changed, self.filter_cleared) {
            (false, false) => None,
            (true, false) => Some(format!("showing the {scope} scope for the new {what}")),
            (false, true) => Some(format!("filter cleared to show the new {what}")),
            (true, true) => Some(format!(
                "filter cleared and showing the {scope} scope for the new {what}"
            )),
        }
    }
}

/// Bring `row` (just created or moved) into view. A row in a section the
/// view scope hides first switches the scope to the narrowest one showing
/// that section (scratch, project or global), else to all. Only then is
/// the filter `search_buffer` checked, and it is cleared only if it is not
/// empty and still hides the row.
pub(super) fn reveal_row(forest: &mut Forest, search_buffer: &mut String, row: usize) -> Revealed {
    let mut revealed = Revealed::default();
    if !in_shown_section(&forest.nodes, row) {
        let spec = find_top_dir(&forest.nodes, row)
            .and_then(|top| forest.sections.get(forest.nodes[top].section));
        let scope = [ViewScope::Scratch, ViewScope::Project, ViewScope::Global]
            .into_iter()
            .find(|scope| spec.is_some_and(|spec| scope.includes(spec)))
            .unwrap_or(ViewScope::All);
        forest.set_scope(scope);
        revealed.scope_changed = true;
    }
    if !search_buffer.is_empty() && !compute_visible(&forest.nodes, search_buffer).contains(&row) {
        search_buffer.clear();
        revealed.filter_cleared = true;
    }
    revealed
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use super::*;

    /// [`project_sections`] opened in that project: the global section is
    /// not the project's.
    fn current_sections() -> Vec<SectionSpec> {
        let mut sections = project_sections();
        for spec in &mut sections {
            spec.is_current = spec.project.is_some();
        }
        // Another project's scratch shows only in the all scope.
        let mut other = scoped("/o/.notez", Scope::Local, false, &["x.md"]);
        other.project = Some("other".to_string());
        sections.push(other);
        sections
    }

    fn shown_sections(scope: ViewScope) -> Vec<String> {
        let sections = current_sections();
        let (mut nodes, _) = build_forest(&sections);
        apply_scope(&mut nodes, &sections, scope);
        get_visible_nodes(&nodes)
            .into_iter()
            .map(|i| nodes[i].name.clone())
            .collect()
    }

    #[test]
    fn s_cycles_all_scratch_project_global_and_back() {
        let mut scope = ViewScope::default();
        let mut seen = vec![scope.label()];
        for _ in 0..4 {
            scope = scope.next();
            seen.push(scope.label());
        }
        assert_eq!(seen, ["all", "scratch", "project", "global", "all"]);
    }

    #[test]
    fn each_scope_shows_its_own_sections_in_tree_order() {
        assert_eq!(
            shown_sections(ViewScope::All),
            [
                "/n/personal/proj",
                "/p/notez",
                "/p/docs",
                "/p/.notez",
                "/n",
                "/o/.notez"
            ]
        );
        assert_eq!(shown_sections(ViewScope::Scratch), ["/p/.notez"]);
        assert_eq!(
            shown_sections(ViewScope::Project),
            ["/n/personal/proj", "/p/notez", "/p/docs"]
        );
        assert_eq!(shown_sections(ViewScope::Global), ["/n"]);
    }

    #[test]
    fn rows_under_a_hidden_section_stay_hidden_even_when_expanded() {
        let sections = current_sections();
        let (mut nodes, _) = build_forest(&sections);
        for node in &mut nodes {
            node.expanded = true;
        }
        apply_scope(&mut nodes, &sections, ViewScope::Scratch);
        let shown: Vec<&str> = get_visible_nodes(&nodes)
            .into_iter()
            .map(|i| nodes[i].name.as_str())
            .collect();
        assert_eq!(shown, ["/p/.notez", "d.md"]);
        assert_eq!(notes_in_scope(&nodes), 1);
        apply_scope(&mut nodes, &sections, ViewScope::All);
        assert_eq!(notes_in_scope(&nodes), 7);
    }

    #[test]
    fn an_empty_store_section_still_shows_in_its_scope() {
        let mut sections = current_sections();
        sections[3].files.clear();
        let (mut nodes, _) = build_forest(&sections);
        apply_scope(&mut nodes, &sections, ViewScope::Scratch);
        let shown: Vec<&str> = get_visible_nodes(&nodes)
            .into_iter()
            .map(|i| nodes[i].name.as_str())
            .collect();
        assert_eq!(shown, ["/p/.notez"]);
        assert_eq!(notes_in_scope(&nodes), 0);
    }

    fn shown_names(forest: &Forest) -> Vec<&str> {
        get_visible_nodes(&forest.nodes)
            .into_iter()
            .map(|i| forest.nodes[i].name.as_str())
            .collect()
    }

    #[test]
    fn a_rebuild_keeps_the_scope_of_each_section() {
        let mut forest = forest_of(current_sections());
        forest.set_scope(ViewScope::Global);
        forest.rebuild(current_sections(), Path::new("/n/new.md"));
        assert_eq!(shown_names(&forest), ["/n"]);
    }

    #[test]
    fn a_section_that_first_appears_in_a_rebuild_is_marked_before_any_cursor_row() {
        let mut sections = current_sections();
        sections.remove(3); // no scratch store yet
        let mut forest = forest_of(sections);
        forest.set_scope(ViewScope::Global);
        let row = forest.rebuild(current_sections(), Path::new("/p/.notez/d.md"));
        let row = row.expect("the new scratch note is listed");
        assert!(!compute_visible(&forest.nodes, "").contains(&row));
        assert_eq!(shown_names(&forest), ["/n"]);
        // The delete path chooses its cursor row among shown rows only.
        let at = forest.rebuild_after_delete(current_sections(), Path::new("/p/.notez/d.md"), "");
        assert!(at.is_none_or(|r| compute_visible(&forest.nodes, "").contains(&r)));
    }

    /// The sections after a new note `/n/new.md` in the global store.
    fn with_new_global_note() -> Vec<SectionSpec> {
        let mut sections = current_sections();
        sections[4].files.push(PathBuf::from("/n/new.md"));
        sections
    }

    #[test]
    fn a_note_created_in_a_hidden_section_switches_to_a_scope_that_shows_it() {
        let mut forest = forest_of(current_sections());
        forest.set_scope(ViewScope::Scratch);
        let row = forest
            .rebuild(with_new_global_note(), Path::new("/n/new.md"))
            .expect("listed");
        assert!(!compute_visible(&forest.nodes, "").contains(&row));

        let mut search = String::new();
        let revealed = reveal_row(&mut forest, &mut search, row);
        assert_eq!(
            revealed,
            Revealed {
                scope_changed: true,
                filter_cleared: false
            }
        );
        assert_eq!(forest.scope, ViewScope::Global);
        assert!(compute_visible(&forest.nodes, "").contains(&row));
        assert_eq!(
            revealed.message("note", forest.scope).as_deref(),
            Some("showing the global scope for the new note")
        );
    }

    #[test]
    fn revealing_keeps_a_filter_that_shows_the_row_and_clears_one_that_hides_it() {
        for (filter, cleared) in [("nw", false), ("zzz", true)] {
            let mut forest = forest_of(current_sections());
            forest.set_scope(ViewScope::Project);
            let row = forest
                .rebuild(with_new_global_note(), Path::new("/n/new.md"))
                .expect("listed");
            let mut search = filter.to_string();
            let revealed = reveal_row(&mut forest, &mut search, row);
            assert!(revealed.scope_changed, "{filter}");
            assert_eq!(revealed.filter_cleared, cleared, "{filter}");
            assert_eq!(search.is_empty(), cleared, "{filter}");
            assert!(
                compute_visible(&forest.nodes, &search).contains(&row),
                "{filter}"
            );
        }
        // Shown section, hiding filter: only the filter goes.
        let mut forest = forest_of(current_sections());
        let row = forest
            .rebuild(with_new_global_note(), Path::new("/n/new.md"))
            .expect("listed");
        let mut search = "zzz".to_string();
        let revealed = reveal_row(&mut forest, &mut search, row);
        assert_eq!(forest.scope, ViewScope::All);
        assert_eq!(
            revealed.message("folder", forest.scope).as_deref(),
            Some("filter cleared to show the new folder")
        );
    }

    #[test]
    fn marks_in_a_hidden_section_are_dropped() {
        let mut forest = forest_of(current_sections());
        let mut marks: HashSet<PathBuf> = ["/p/.notez/d.md", "/n/e.md", "/n/personal/proj/top.md"]
            .into_iter()
            .map(PathBuf::from)
            .collect();
        drop_hidden_marks(&mut marks, &forest.nodes);
        assert_eq!(marks.len(), 3, "all scope hides nothing");
        forest.set_scope(ViewScope::Global);
        drop_hidden_marks(&mut marks, &forest.nodes);
        assert_eq!(marks, HashSet::from([PathBuf::from("/n/e.md")]));
    }
}
