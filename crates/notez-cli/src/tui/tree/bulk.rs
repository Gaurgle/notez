//! Marks (`Space`) and the delete, move and set-scope actions on the
//! marked set.

use super::*;

// --- Marks ---
//
// `Space` marks note and folder rows by path for the session; nothing is
// persisted. Marks are matched by path, so they survive navigation, the
// filter and a rebuild, and a row whose path changes (rename, move) or that
// is no longer listed loses its mark (see [`prune_marks`]). With marks
// present, `d`, `m` and `S` act on the [`action_set`] instead of the cursor
// row.

/// The footer message for `Space` on a section row.
const SECTION_MARK: &str = "mark: a section cannot be marked";

/// Drawn in the gutter of a marked row, the column before its tag field.
/// A thin bar, so it reads as a mark rather than as one of the tag dots.
const MARK_GLYPH: &str = "▎";

/// Toggle the mark on `row`: `Ok(true)` when a note or folder row was
/// marked or unmarked, `Ok(false)` when there is no row, `Err` with the
/// footer message on a section row.
fn toggle_mark(
    marks: &mut HashSet<PathBuf>,
    nodes: &[TreeNode],
    row: Option<usize>,
) -> std::result::Result<bool, &'static str> {
    let Some(node) = row.and_then(|i| nodes.get(i)) else {
        return Ok(false);
    };
    if node.depth == 0 {
        return Err(SECTION_MARK);
    }
    if !marks.remove(&node.path) {
        marks.insert(node.path.clone());
    }
    Ok(true)
}

/// `Space` in browse mode: toggle the mark on the cursor row, then move the
/// cursor down one row (as `j` does) unless it is on the last one. Returns
/// the footer message of a refusal; a refused row keeps the cursor.
pub(super) fn press_space(
    marks: &mut HashSet<PathBuf>,
    nodes: &mut [TreeNode],
    state: &mut ListState,
    visible: &[usize],
    focus_active: bool,
) -> Option<&'static str> {
    let selected = state.selected().unwrap_or(0);
    match toggle_mark(marks, nodes, visible.get(selected).copied()) {
        Ok(true) if selected + 1 < visible.len() => {
            navigate(
                nodes,
                state,
                visible,
                selected,
                visible[selected],
                focus_active,
                1,
            );
            None
        }
        Ok(_) => None,
        Err(message) => Some(message),
    }
}

/// `Esc` in browse mode: clear the marks if there are any and do nothing
/// else; otherwise clear the filter if there is one; otherwise do nothing.
/// `Esc` never quits, so a stray press after a prompt closes is harmless;
/// `q` and `:q` are the ways out.
pub(super) fn browse_escape(marks: &mut HashSet<PathBuf>, search_buffer: &mut String) {
    if !marks.is_empty() {
        marks.clear();
    } else {
        search_buffer.clear();
    }
}

/// Drop every mark whose path no note or folder row lists any more.
pub(super) fn prune_marks(marks: &mut HashSet<PathBuf>, nodes: &[TreeNode]) {
    if marks.is_empty() {
        return;
    }
    let listed: HashSet<&Path> = nodes
        .iter()
        .filter(|n| n.depth > 0)
        .map(|n| n.path.as_path())
        .collect();
    marks.retain(|p| listed.contains(p.as_path()));
}

/// Whether `node` is drawn and acted on as marked. Section rows never are,
/// even when a folder row elsewhere shares their path.
pub(super) fn is_marked(marks: &HashSet<PathBuf>, node: &TreeNode) -> bool {
    node.depth > 0 && marks.contains(&node.path)
}

/// The rows a bulk action works on, in tree order: every marked note or
/// folder row minus each row with a marked folder above it in its section,
/// so a folder and one of its own notes act once, on the folder. A marked
/// path no row lists is left out.
fn action_set(marks: &HashSet<PathBuf>, nodes: &[TreeNode]) -> Vec<usize> {
    (0..nodes.len())
        .filter(|&i| is_marked(marks, &nodes[i]))
        .filter(|&i| {
            let mut up = nodes[i].parent_idx;
            while let Some(p) = up {
                if is_marked(marks, &nodes[p]) {
                    return false;
                }
                up = nodes[p].parent_idx;
            }
            true
        })
        .collect()
}

/// A row's `line` drawn as marked: the gutter before the tag field shows
/// [`MARK_GLYPH`] in the header colour and the whole row is bold. Every
/// other column stays where it was, so the tag field keeps its click
/// position.
pub(super) fn mark_row(mut line: Line<'static>) -> Line<'static> {
    for span in &mut line.spans {
        span.style = span.style.add_modifier(Modifier::BOLD);
    }
    if let Some(gutter) = line.spans.first_mut() {
        *gutter = Span::styled(MARK_GLYPH, theme::header());
    }
    line
}

/// The mark count that leads the footer while any row is marked.
pub(super) fn marked_lead(count: usize) -> Vec<Span<'static>> {
    vec![Span::styled(format!(" {count} marked "), theme::header())]
}

/// One row of a bulk delete: what the question counts and the single
/// delete's prompt for it, or the footer message `d` gives on that row
/// alone (the guard that refuses it), which the delete reports as its
/// failure.
#[derive(Debug, Clone)]
pub(super) struct BulkItem {
    path: PathBuf,
    /// Path relative to its section's root.
    rel: String,
    is_dir: bool,
    scope: Scope,
    /// The scope's label, as the single delete question names it.
    label: String,
    /// The notes a folder holds; 0 for a note.
    notes_inside: usize,
    /// A folder holds something besides notes (see [`FolderContents`]).
    has_other_files: bool,
    prompt: std::result::Result<DeletePrompt, &'static str>,
}

impl BulkItem {
    /// The name a failure message uses: the relative path, a folder's with
    /// a trailing `/`.
    fn display_name(&self) -> String {
        if self.is_dir {
            format!("{}/", self.rel)
        } else {
            self.rel.clone()
        }
    }
}

/// What `d` opens with rows marked: the [`bulk_items`] of the marks, or,
/// when the guard refuses any of them, the footer message for the first
/// one, its name in front (`personal/: delete: this folder holds another
/// section`). A refused set is refused whole, before anything is asked, so
/// the confirm only counts items that will be attempted.
pub(super) fn bulk_delete_request(
    nodes: &[TreeNode],
    sections: &[SectionSpec],
    marks: &HashSet<PathBuf>,
    current_project: Option<&str>,
) -> std::result::Result<Vec<BulkItem>, String> {
    let items = bulk_items(nodes, sections, marks, current_project);
    if let Some(item) = items.iter().find(|i| i.prompt.is_err()) {
        let refusal = item.prompt.as_ref().err().copied().unwrap_or_default();
        return Err(format!("{}: {refusal}", item.display_name()));
    }
    Ok(items)
}

/// One item per row of the [`action_set`], each checked by
/// [`delete_request`] as if the cursor were on it. A folder's contents are
/// counted from the disk now.
fn bulk_items(
    nodes: &[TreeNode],
    sections: &[SectionSpec],
    marks: &HashSet<PathBuf>,
    current_project: Option<&str>,
) -> Vec<BulkItem> {
    let mut items = Vec::new();
    for i in action_set(marks, nodes) {
        let node = &nodes[i];
        let Some(spec) = sections.get(node.section) else {
            continue;
        };
        let prompt = match delete_request(nodes, sections, Some(i), current_project) {
            Ok(Some(prompt)) => Ok(prompt),
            Ok(None) => continue,
            Err(message) => Err(message),
        };
        let contents = match &prompt {
            _ if !node.is_dir => FolderContents::default(),
            Ok(DeletePrompt {
                folder: Some(contents),
                ..
            }) => *contents,
            _ => folder_contents(&node.path),
        };
        let rel = node.path.strip_prefix(&spec.root).unwrap_or(&node.path);
        items.push(BulkItem {
            path: node.path.clone(),
            rel: rel.to_string_lossy().into_owned(),
            is_dir: node.is_dir,
            scope: spec.scope,
            label: scope_label(spec.scope, spec.project.as_deref(), current_project),
            notes_inside: contents.notes,
            has_other_files: contents.has_other_files,
            prompt,
        });
    }
    items
}

/// `1 note`, `2 notes`.
fn count_of(count: usize, noun: &str) -> String {
    if count == 1 {
        format!("1 {noun}")
    } else {
        format!("{count} {noun}s")
    }
}

/// The bulk delete question: `delete 2 notes and 1 folder (3 notes inside)
/// from personal, global? y/n`. The note clause is left out when no note is
/// in the set and the folder clause when no folder is; each scope label is
/// named once, in tree order. As in the single delete, the folder clause
/// adds `and other files` when a folder holds anything but notes, and a set
/// with anything in local scratch says it cannot be undone.
fn bulk_delete_question(items: &[BulkItem]) -> String {
    let notes = items.iter().filter(|i| !i.is_dir).count();
    let folders = items.iter().filter(|i| i.is_dir).count();
    let mut what = Vec::new();
    if notes > 0 {
        what.push(count_of(notes, "note"));
    }
    if folders > 0 {
        let inside: usize = items.iter().map(|i| i.notes_inside).sum();
        let inside = if inside == 0 {
            "no notes".to_string()
        } else {
            count_of(inside, "note")
        };
        let other = if items.iter().any(|i| i.has_other_files) {
            " and other files"
        } else {
            ""
        };
        what.push(format!(
            "{} ({inside} inside){other}",
            count_of(folders, "folder")
        ));
    }
    let mut labels: Vec<&str> = Vec::new();
    for item in items {
        if !labels.contains(&item.label.as_str()) {
            labels.push(&item.label);
        }
    }
    let warning = if items.iter().any(|i| i.scope == Scope::Local) {
        " (not recoverable)"
    } else {
        ""
    };
    format!(
        "delete {} from {}?{warning} y/n",
        what.join(" and "),
        labels.join(", ")
    )
}

/// The bulk delete confirmation that leads the footer while it is open.
pub(super) fn bulk_delete_lead(items: &[BulkItem]) -> Vec<Span<'static>> {
    vec![Span::styled(
        format!(" {}", bulk_delete_question(items)),
        Style::default().fg(theme::PEACH),
    )]
}

/// Answer the open bulk delete with `key`. Anything but `y` cancels and
/// returns `None` without touching the disk or the forest. On `y` every
/// item is deleted in order exactly as a single delete would
/// ([`remove_and_retire`]); an item its guard refuses, or whose removal
/// fails, is counted as failed and the rest go on. Then the forest is
/// rebuilt once (falling back to the current rows minus what went, as
/// [`answer_delete`] does), and the cursor row follows the last deleted
/// item by [`Forest::rebuild_after_delete`]'s rule.
pub(super) fn answer_bulk_delete(
    key: KeyCode,
    forest: &mut Forest,
    retired: &mut Vec<(PathBuf, String)>,
    items: &[BulkItem],
    search: &str,
    rebuild: &dyn Fn() -> Result<Vec<SectionSpec>>,
) -> Option<DeleteOutcome> {
    if key != KeyCode::Char('y') {
        return None;
    }
    let mut deleted = 0;
    let mut failed = 0;
    let mut first_failure: Option<String> = None;
    let mut last_deleted: Option<&Path> = None;
    for item in items {
        let result = match &item.prompt {
            Ok(prompt) => remove_and_retire(forest, retired, prompt).map_err(|e| e.to_string()),
            Err(message) => Err((*message).to_string()),
        };
        match result {
            Ok(()) => {
                deleted += 1;
                last_deleted = Some(&item.path);
            }
            Err(e) => {
                failed += 1;
                first_failure.get_or_insert_with(|| format!("{} ({e})", item.display_name()));
            }
        }
    }
    let mut message = match first_failure {
        None => format!("deleted {deleted}"),
        Some(first) => format!("deleted {deleted}, failed {failed}: {first}"),
    };
    let mut relisted = true;
    let sections = rebuild().unwrap_or_else(|e| {
        relisted = false;
        message = format!("{message}, but the list could not be refreshed: {e:#}");
        let gone = |p: &Path| {
            items.iter().any(|i| p.starts_with(&i.path)) && std::fs::symlink_metadata(p).is_err()
        };
        current_sections_without(forest, &gone)
    });
    let anchor = last_deleted.or_else(|| items.last().map(|i| i.path.as_path()));
    let row = forest.rebuild_after_delete(sections, anchor.unwrap_or(Path::new("")), search);
    Some(DeleteOutcome {
        row,
        message,
        relisted,
    })
}

// --- Bulk move and set scope ---
//
// With rows marked, `m` and `S` open their prompt once for the
// [`action_set`]. The prompt is the first item's, so `Tab` cycles that
// item's project's scopes; one prompt per item, as `m` or `S` would open it
// on that row, keeps what each item needs. The whole set is resolved before
// anything moves and refused as a whole on the first problem; the moves
// then run one by one, a failure leaving its item in place, followed by one
// rebuild.

/// A marked row's name in footer messages: its path relative to its
/// section root, a folder's with a trailing `/`.
fn marked_name(node: &TreeNode, sections: &[SectionSpec]) -> String {
    let rel = sections
        .get(node.section)
        .and_then(|spec| node.path.strip_prefix(&spec.root).ok())
        .map_or_else(
            || node.name.clone(),
            |rel| rel.to_string_lossy().into_owned(),
        );
    if node.is_dir {
        format!("{rel}/")
    } else {
        rel
    }
}

/// What `m` (`fixed` false) or `S` (`fixed` true) opens with rows marked:
/// the shared prompt and one prompt per item of the [`action_set`], in tree
/// order. `Err` is the footer message: the guard refusal of the first item
/// [`open_move`] refuses (a docs row, a folder holding a section), its name
/// in front, or `<verb>: marked items span projects` when the items do not
/// all offer the same project's scopes.
pub(super) fn bulk_move_request(
    nodes: &[TreeNode],
    sections: &[SectionSpec],
    marks: &HashSet<PathBuf>,
    ctx: &TreeContext,
    fixed: bool,
) -> std::result::Result<(MovePrompt, Vec<MovePrompt>), String> {
    let rows = action_set(marks, nodes);
    let mut items = Vec::new();
    for &i in &rows {
        match open_move(nodes, sections, Some(i), ctx, fixed) {
            Ok(item) => items.push(item),
            Err(refusal) => return Err(format!("{}: {refusal}", marked_name(&nodes[i], sections))),
        }
    }
    let prompt =
        open_move(nodes, sections, rows.first().copied(), ctx, fixed).map_err(str::to_string)?;
    if items
        .iter()
        .any(|item| item.scope.project != prompt.scope.project)
    {
        return Err(format!("{}: marked items span projects", prompt.verb()));
    }
    Ok((prompt, items))
}

/// The prompt that leads the footer for a marked set: `move <n> items to
/// <scope>/<folder>_`, or `set scope of <n> items: <scope> (Tab cycles,
/// Enter applies)`.
pub(super) fn bulk_move_lead(prompt: &MovePrompt, count: usize) -> Vec<Span<'static>> {
    let items = count_of(count, "item");
    let label = &prompt.scope.target.label;
    if prompt.fixed {
        return vec![Span::styled(
            format!(" set scope of {items}: {label} (Tab cycles, Enter applies)"),
            Style::default().fg(theme::MAUVE),
        )];
    }
    vec![
        Span::styled(
            format!(" move {items} to {label}/"),
            Style::default().fg(theme::MAUVE),
        ),
        Span::styled(
            prompt.scope.buffer.clone(),
            Style::default().fg(theme::TEXT),
        ),
        Span::styled("_", Style::default().fg(theme::OVERLAY)),
    ]
}

/// A resolved bulk move: one plan per item that moves, in tree order, and
/// the destination as the question names it (the scope and typed folder for
/// `m`, the scope alone for `S`, where each item keeps its own folder).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct BulkMovePlan {
    pub(super) plans: Vec<MovePlan>,
    display: String,
}

impl BulkMovePlan {
    /// Some item changes scope, so the set asks first.
    pub(super) fn needs_confirm(&self) -> bool {
        self.plans.iter().any(MovePlan::needs_confirm)
    }

    /// `move <n> items to <destination>? y/n` with the clauses of every
    /// scope change in the set, each once, public first (see
    /// [`move_clauses`]).
    fn question(&self) -> String {
        let transitions: Vec<(Scope, Scope)> = self.plans.iter().map(|p| (p.from, p.to)).collect();
        let repo = self
            .plans
            .first()
            .map(|p| repo_name(&p.to_root))
            .unwrap_or_default();
        let clauses = move_clauses(&transitions, &repo);
        format!(
            "move {} to {}?{clauses} y/n",
            count_of(self.plans.len(), "item"),
            self.display
        )
    }
}

/// Resolve `Enter` in the bulk prompt for `items` (see
/// [`bulk_move_request`]). Nothing on disk is touched. Each item goes to
/// `<destination folder>/<its own name>`: the typed folder for `m`, its own
/// folder under the chosen scope for `S`. An item already at its
/// destination is left out; `Ok(None)` when that leaves nothing for `S`
/// (`Enter` on the scope every item is in). `Err` is the footer message for
/// the first problem in tree order, and refuses the whole set: a folder
/// [`resolve_folder`] refuses, a folder going inside itself or inside
/// another marked folder, two items with the same destination name, an
/// existing destination, or (`m` only) nothing left to move.
pub(super) fn resolve_bulk_move(
    prompt: &MovePrompt,
    items: &[MovePrompt],
    roots: &NewNoteRoots,
) -> std::result::Result<Option<BulkMovePlan>, String> {
    let verb = prompt.verb();
    let target = &prompt.scope.target;
    let display = if prompt.fixed {
        target.label.clone()
    } else {
        resolve_folder(verb, target, &prompt.scope.buffer, roots)?.1
    };
    let mut plans: Vec<MovePlan> = Vec::new();
    for item in items {
        let buffer = if prompt.fixed {
            &item.scope.buffer
        } else {
            &prompt.scope.buffer
        };
        let (dir, shown) = resolve_folder(verb, target, buffer, roots)?;
        let name = file_name_of(&item.src);
        let dst = dir.join(&name);
        if dst == item.src {
            continue;
        }
        if item.notes.is_some() && dst.starts_with(&item.src) {
            return Err(format!("{verb}: {name}/ cannot go inside itself"));
        }
        if let Some(holder) = items
            .iter()
            .find(|o| o.notes.is_some() && dst.starts_with(&o.src))
        {
            return Err(format!(
                "{verb}: {shown}/{name} is inside the marked folder {}/",
                holder.rel
            ));
        }
        if plans.iter().any(|p| p.dst == dst) {
            return Err(format!("{verb}: two marked items are named {name}"));
        }
        if std::fs::symlink_metadata(&dst).is_ok() {
            return Err(format!("{verb}: {shown}/{name} already exists"));
        }
        plans.push(MovePlan {
            src: item.src.clone(),
            dst,
            rel: item.rel.clone(),
            from: item.from,
            to: target.scope,
            to_root: target.dir.clone(),
            display: shown,
            notes: item.notes,
        });
    }
    if !plans.is_empty() {
        Ok(Some(BulkMovePlan { plans, display }))
    } else if prompt.fixed {
        Ok(None)
    } else {
        Err(format!("{verb}: the marked items are already in {display}"))
    }
}

/// The bulk move confirmation that leads the footer while it is open.
pub(super) fn bulk_move_confirm_lead(plan: &BulkMovePlan) -> Vec<Span<'static>> {
    vec![Span::styled(
        format!(" {}", plan.question()),
        Style::default().fg(theme::PEACH),
    )]
}

/// Run the resolved bulk move: each plan in order through the single
/// move's [`move_and_repoint`], so every item retires, repoints and carries
/// its tags exactly as [`apply_move`] does; a failing item stays in place,
/// is counted and the rest go on. Then the forest is rebuilt once (falling
/// back to the current rows as [`apply_move`] does) with the first moved
/// item revealed; its row is the outcome's, `None` when nothing moved or it
/// is not listed. The message is `moved <ok>` or `moved <ok>, failed <bad>:
/// <first failing name> (<error>)`.
pub(super) fn apply_bulk_move(
    forest: &mut Forest,
    retired: &mut Vec<(PathBuf, String)>,
    carried: &mut Vec<CarriedTags>,
    plans: &[MovePlan],
    roots: &NewNoteRoots,
    rebuild: &dyn Fn() -> Result<Vec<SectionSpec>>,
) -> MoveOutcome {
    let mut done: Vec<MovePlan> = Vec::new();
    let mut moved: Vec<Moved> = Vec::new();
    let mut failed = 0;
    let mut first_failure: Option<String> = None;
    for plan in plans {
        match move_and_repoint(forest, retired, carried, plan, roots) {
            Ok(m) => {
                done.push(plan.clone());
                moved.push(m);
            }
            Err(e) => {
                failed += 1;
                first_failure.get_or_insert_with(|| format!("{} ({e})", plan.what()));
            }
        }
    }
    let mut message = match first_failure {
        None => format!("moved {}", done.len()),
        Some(first) => format!("moved {}, failed {failed}: {first}", done.len()),
    };
    let mut relisted = true;
    let sections = rebuild().unwrap_or_else(|e| {
        relisted = false;
        message = format!("{message}, but the list could not be refreshed: {e:#}");
        sections_after_moves(forest, &done)
    });
    let anchor = done.first().map_or(Path::new(""), |p| p.dst.as_path());
    let row = forest.rebuild(sections, anchor);
    for m in &moved {
        carry_unlisted_moved(forest, carried, m);
    }
    MoveOutcome {
        row,
        message,
        relisted,
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use super::*;

    // --- Marks ---

    fn action_paths(marks: &HashSet<PathBuf>, nodes: &[TreeNode]) -> Vec<String> {
        action_set(marks, nodes)
            .iter()
            .map(|&i| nodes[i].path.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn space_marks_and_unmarks_notes_and_folders_and_refuses_a_section() {
        let sections = project_sections();
        let (nodes, _) = build_forest(&sections);
        let mut marks = HashSet::new();
        let note = row(&nodes, "/n/personal/proj/top.md");
        let folder = row(&nodes, "/n/personal/proj/ideas");
        assert_eq!(toggle_mark(&mut marks, &nodes, Some(note)), Ok(true));
        assert_eq!(toggle_mark(&mut marks, &nodes, Some(folder)), Ok(true));
        assert_eq!(
            marks,
            marks_of(&["/n/personal/proj/top.md", "/n/personal/proj/ideas"])
        );
        assert_eq!(toggle_mark(&mut marks, &nodes, Some(note)), Ok(true));
        assert_eq!(
            marks,
            marks_of(&["/n/personal/proj/ideas"]),
            "a second Space unmarks"
        );
        for section in ["/n/personal/proj", "/p/docs", "/p/.notez", "/n"] {
            let refused = toggle_mark(&mut marks, &nodes, Some(row(&nodes, section)));
            assert_eq!(refused, Err(SECTION_MARK), "{section}");
        }
        assert_eq!(toggle_mark(&mut marks, &nodes, None), Ok(false));
        assert_eq!(
            toggle_mark(&mut marks, &nodes, Some(nodes.len())),
            Ok(false)
        );
        assert_eq!(toggle_mark(&mut HashSet::new(), &[], Some(0)), Ok(false));
        assert_eq!(
            marks,
            marks_of(&["/n/personal/proj/ideas"]),
            "refusals change nothing"
        );
    }

    #[test]
    fn space_moves_the_cursor_down_one_row_and_stays_on_a_refusal_or_the_last_row() {
        let sections = vec![spec("/r", "S", &["ideas/a.md", "z.md"])];
        let (mut nodes, _) = build_forest(&sections);
        for node in &mut nodes {
            node.expanded = true;
        }
        let visible = compute_visible(&nodes, "");
        let mut marks = HashSet::new();
        let mut state = ListState::default();
        let at =
            |nodes: &[TreeNode], path: &str| visible.iter().position(|&i| i == row(nodes, path));

        state.select(at(&nodes, "/r"));
        assert_eq!(
            press_space(&mut marks, &mut nodes, &mut state, &visible, false),
            Some(SECTION_MARK)
        );
        assert_eq!(
            state.selected(),
            at(&nodes, "/r"),
            "a refusal does not move the cursor"
        );

        state.select(at(&nodes, "/r/ideas"));
        assert_eq!(
            press_space(&mut marks, &mut nodes, &mut state, &visible, false),
            None
        );
        assert_eq!(state.selected(), at(&nodes, "/r/ideas/a.md"));
        assert_eq!(
            press_space(&mut marks, &mut nodes, &mut state, &visible, false),
            None
        );
        assert_eq!(state.selected(), at(&nodes, "/r/z.md"));
        assert_eq!(
            press_space(&mut marks, &mut nodes, &mut state, &visible, false),
            None
        );
        assert_eq!(
            state.selected(),
            at(&nodes, "/r/z.md"),
            "the last row keeps the cursor"
        );
        assert_eq!(marks, marks_of(&["/r/ideas", "/r/ideas/a.md", "/r/z.md"]));
    }

    #[test]
    fn esc_clears_the_marks_first_then_the_filter_then_does_nothing() {
        let mut marks = marks_of(&["/r/a.md", "/r/b.md"]);
        let mut search = "a".to_string();
        browse_escape(&mut marks, &mut search);
        assert!(marks.is_empty());
        assert_eq!(search, "a", "with marks, Esc clears only the marks");
        browse_escape(&mut marks, &mut search);
        assert!(search.is_empty(), "then the filter");
        browse_escape(&mut marks, &mut search);
        assert!(
            marks.is_empty() && search.is_empty(),
            "then Esc does nothing and never quits"
        );
    }

    #[test]
    fn marks_survive_filter_collapse_and_a_rebuild_and_go_with_their_row() {
        let files = ["ideas/a.md", "ideas/b.md", "z.md"];
        let (nodes, tag_roots) = build_forest(&[spec("/r", "S", &files)]);
        let initial = vec![HashMap::new(); tag_roots.len()];
        let mut forest = Forest {
            sections: vec![spec("/r", "S", &files)],
            nodes,
            tag_roots,
            initial,
        };
        let mut marks = marks_of(&["/r/ideas", "/r/ideas/a.md", "/r/ideas/b.md", "/r/z.md"]);

        let ideas = row(&forest.nodes, "/r/ideas");
        forest.nodes[ideas].expanded = false;
        assert!(!compute_visible(&forest.nodes, "z").contains(&row(&forest.nodes, "/r/ideas/a.md")));
        prune_marks(&mut marks, &forest.nodes);
        assert_eq!(
            marks.len(),
            4,
            "collapsed and filtered rows keep their marks"
        );

        forest.rebuild(
            vec![spec("/r", "S", &["ideas/a.md", "ideas/b.md"])],
            Path::new(""),
        );
        prune_marks(&mut marks, &forest.nodes);
        assert_eq!(
            marks,
            marks_of(&["/r/ideas", "/r/ideas/a.md", "/r/ideas/b.md"]),
            "a row the rebuild drops loses its mark"
        );

        let b = row(&forest.nodes, "/r/ideas/b.md");
        forest.nodes[b].path = PathBuf::from("/r/ideas/renamed.md");
        prune_marks(&mut marks, &forest.nodes);
        assert_eq!(
            marks,
            marks_of(&["/r/ideas", "/r/ideas/a.md"]),
            "a renamed row loses its mark"
        );

        marks.insert(PathBuf::from("/r"));
        prune_marks(&mut marks, &forest.nodes);
        assert!(
            !marks.contains(Path::new("/r")),
            "a section row is never marked"
        );
    }

    #[test]
    fn the_action_set_drops_every_row_under_a_marked_folder() {
        let files = [
            "ideas/a.md",
            "ideas/deep/m.md",
            "ideas/deep/n.md",
            "plans/p.md",
            "z.md",
        ];
        let (nodes, _) = build_forest(&[spec("/r", "S", &files), spec("/q", "Q", &["ideas/a.md"])]);
        assert!(
            nodes.iter().all(|n| !n.expanded),
            "every folder is collapsed"
        );

        let folder_and_note = marks_of(&["/r/ideas", "/r/ideas/a.md"]);
        assert_eq!(action_paths(&folder_and_note, &nodes), vec!["/r/ideas"]);

        let nested = marks_of(&["/r/ideas/deep", "/r/ideas", "/r/ideas/deep/n.md"]);
        assert_eq!(action_paths(&nested, &nodes), vec!["/r/ideas"]);

        let disjoint = marks_of(&["/q/ideas/a.md", "/r/z.md", "/r/plans", "/r/ideas/deep/n.md"]);
        assert_eq!(
            action_paths(&disjoint, &nodes),
            vec!["/r/ideas/deep/n.md", "/r/plans", "/r/z.md", "/q/ideas/a.md"],
            "tree order"
        );

        let unlisted = marks_of(&["/r/gone.md", "/r/z.md", "/r"]);
        assert_eq!(
            action_paths(&unlisted, &nodes),
            vec!["/r/z.md"],
            "unlisted paths and sections drop"
        );
        assert!(action_set(&HashSet::new(), &nodes).is_empty());
    }

    fn bulk_item(rel: &str, folder_notes: Option<usize>, scope: Scope, label: &str) -> BulkItem {
        BulkItem {
            path: PathBuf::from("/x").join(rel),
            rel: rel.to_string(),
            is_dir: folder_notes.is_some(),
            scope,
            label: label.to_string(),
            notes_inside: folder_notes.unwrap_or(0),
            has_other_files: false,
            prompt: Err("unused"),
        }
    }

    #[test]
    fn the_bulk_question_counts_notes_and_folders_and_names_each_scope_once() {
        let note = |rel, scope, label| bulk_item(rel, None, scope, label);
        let folder = |rel, notes, scope, label| bulk_item(rel, Some(notes), scope, label);
        let public = "public (committed with the project)";

        let notes_only = [
            note("a.md", Scope::Personal, "personal"),
            note("b.md", Scope::Personal, "personal"),
        ];
        assert_eq!(
            bulk_delete_question(&notes_only),
            "delete 2 notes from personal? y/n"
        );
        assert_eq!(
            bulk_delete_question(&[note("e.md", Scope::Global, "global")]),
            "delete 1 note from global? y/n"
        );

        let folders_only = [
            folder("ideas", 2, Scope::Personal, "personal"),
            folder("plans", 0, Scope::Global, "global"),
        ];
        assert_eq!(
            bulk_delete_question(&folders_only),
            "delete 2 folders (2 notes inside) from personal, global? y/n"
        );
        assert_eq!(
            bulk_delete_question(&[folder("empty", 0, Scope::Personal, "personal")]),
            "delete 1 folder (no notes inside) from personal? y/n"
        );
        assert_eq!(
            bulk_delete_question(&[folder("one", 1, Scope::Public, public)]),
            format!("delete 1 folder (1 note inside) from {public}? y/n")
        );

        let mixed = [
            note("c.md", Scope::Public, public),
            folder("ideas", 3, Scope::Personal, "personal"),
            note("d.md", Scope::Personal, "personal"),
        ];
        assert_eq!(
            bulk_delete_question(&mixed),
            format!("delete 2 notes and 1 folder (3 notes inside) from {public}, personal? y/n")
        );

        let local = [
            note("a.md", Scope::Personal, "personal"),
            folder("x", 1, Scope::Local, "local scratch"),
        ];
        assert_eq!(
            bulk_delete_question(&local),
            "delete 1 note and 1 folder (1 note inside) from personal, local scratch? (not recoverable) y/n"
        );
        for set in [&notes_only[..], &folders_only[..], &mixed[..]] {
            assert!(!bulk_delete_question(set).contains("not recoverable"));
            assert!(!bulk_delete_question(set).contains("other files"));
        }
    }

    #[test]
    fn the_bulk_question_says_other_files_when_a_folder_holds_more_than_notes() {
        let mut odd = bulk_item("assets", Some(1), Scope::Personal, "personal");
        odd.has_other_files = true;
        let set = [
            bulk_item("a.md", None, Scope::Personal, "personal"),
            bulk_item("ideas", Some(2), Scope::Personal, "personal"),
            odd,
        ];
        assert_eq!(
            bulk_delete_question(&set),
            "delete 1 note and 2 folders (3 notes inside) and other files from personal? y/n"
        );

        let (_dir, roots) = temp_tree();
        let personal = roots[0].1.clone();
        std::fs::write(personal.join("ideas/picture.png"), "png").unwrap();
        let forest = forest_of(list_sections_with_dirs(&roots));
        let items = bulk_at(&forest, &[personal.join("ideas"), personal.join("c.md")]);
        assert!(
            bulk_delete_question(&items).contains("(2 notes inside) and other files from"),
            "{}",
            bulk_delete_question(&items)
        );
    }

    /// Mark `paths` and open the confirm as `d` does with marks present.
    fn bulk_at(forest: &Forest, paths: &[PathBuf]) -> Vec<BulkItem> {
        let marks: HashSet<PathBuf> = paths.iter().cloned().collect();
        bulk_delete_request(&forest.nodes, &forest.sections, &marks, Some("proj"))
            .expect("no guard refuses the set")
    }

    #[test]
    fn bulk_delete_across_two_scopes_removes_exactly_the_set_and_retires_its_keys() {
        let (_dir, roots) = temp_tree();
        let personal = roots[0].1.clone();
        let public = roots[1].1.clone();
        std::fs::write(
            personal.join(".tags"),
            "ideas/a.md:1\nideas/b.md:2\nc.md:4\n",
        )
        .unwrap();
        std::fs::write(public.join(".tags"), "c.md:1\nideas/a.md:2\n").unwrap();
        let rebuild = || Ok(list_sections_with_dirs(&roots));
        let mut forest = forest_of(list_sections_with_dirs(&roots));
        for node in &mut forest.nodes {
            node.expanded = true;
        }
        let mut retired = Vec::new();
        let before = all_files(&roots);
        let gone = [
            personal.join("ideas/a.md"),
            personal.join("ideas/b.md"),
            public.join("c.md"),
        ];

        let items = bulk_at(
            &forest,
            &[
                personal.join("ideas"),
                personal.join("ideas/a.md"),
                public.join("c.md"),
            ],
        );
        assert_eq!(
            bulk_delete_question(&items),
            "delete 1 note and 1 folder (2 notes inside) from personal, public (committed with the project)? y/n"
        );
        let outcome = answer_bulk_delete(
            KeyCode::Char('y'),
            &mut forest,
            &mut retired,
            &items,
            "",
            &rebuild,
        )
        .expect("y confirms");

        let expected: Vec<PathBuf> = before.into_iter().filter(|p| !gone.contains(p)).collect();
        assert_eq!(all_files(&roots), expected, "exactly the set went");
        assert!(!personal.join("ideas").exists());
        assert_eq!(outcome.message, "deleted 2");
        assert!(!forest
            .nodes
            .iter()
            .any(|n| n.path.starts_with(personal.join("ideas")) || n.path == public.join("c.md")));
        assert_eq!(
            outcome.row,
            Some(row(&forest.nodes, path_str(&public.join("ideas")))),
            "the last deleted row had no next sibling, so the one before it"
        );

        let changed =
            changed_tag_maps_retiring(&forest.nodes, &forest.tag_roots, &forest.initial, &retired);
        let mut personal_map = note_tags::load_tags(&personal);
        personal_map.retain(|k, _| k == "c.md");
        let mut public_map = note_tags::load_tags(&public);
        public_map.retain(|k, _| k == "ideas/a.md");
        assert_eq!(changed.len(), 2, "{changed:?}");
        assert!(
            changed.contains(&(personal.clone(), personal_map)),
            "{changed:?}"
        );
        assert!(
            changed.contains(&(public.clone(), public_map)),
            "{changed:?}"
        );
    }

    #[test]
    fn any_answer_but_y_to_the_bulk_question_deletes_nothing() {
        let (_dir, roots) = temp_tree();
        let rebuild = || Ok(list_sections_with_dirs(&roots));
        let mut forest = forest_of(list_sections_with_dirs(&roots));
        let mut retired = Vec::new();
        let items = bulk_at(
            &forest,
            &[roots[0].1.join("ideas"), roots[1].1.join("c.md")],
        );
        let before = all_files(&roots);
        for code in [
            KeyCode::Char('n'),
            KeyCode::Esc,
            KeyCode::Char(' '),
            KeyCode::Enter,
            KeyCode::Char('Y'),
        ] {
            assert!(
                answer_bulk_delete(code, &mut forest, &mut retired, &items, "", &rebuild).is_none(),
                "{code:?}"
            );
        }
        assert_eq!(all_files(&roots), before);
        assert!(retired.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn a_failing_item_is_reported_and_the_rest_of_the_set_is_deleted() {
        use std::os::unix::fs::PermissionsExt;
        let (_dir, roots) = temp_tree();
        let personal = roots[0].1.clone();
        let global = roots[3].1.clone();
        std::fs::write(personal.join(".tags"), "ideas/a.md:1\nc.md:2\n").unwrap();
        let rebuild = || Ok(list_sections_with_dirs(&roots));
        let mut forest = forest_of(list_sections_with_dirs(&roots));
        let mut retired = Vec::new();
        let locked = personal.join("ideas");
        let items = bulk_at(
            &forest,
            &[global.join("c.md"), locked.clone(), personal.join("c.md")],
        );

        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o555)).unwrap();
        let outcome = answer_bulk_delete(
            KeyCode::Char('y'),
            &mut forest,
            &mut retired,
            &items,
            "",
            &rebuild,
        );
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o755)).unwrap();
        let outcome = outcome.unwrap();

        assert!(
            outcome.message.starts_with("deleted 2, failed 1: ideas/ ("),
            "{}",
            outcome.message
        );
        assert!(
            locked.join("a.md").is_file() && locked.join("b.md").is_file(),
            "the locked folder is left whole"
        );
        assert!(
            !personal.join("c.md").exists() && !global.join("c.md").exists(),
            "the rest went"
        );
        assert!(
            forest.nodes.iter().any(|n| n.path == locked.join("a.md")),
            "the tree shows what is left"
        );
        assert!(
            !retired.iter().any(|(_, k)| k.starts_with("ideas/")),
            "{retired:?}"
        );
        assert!(
            retired.contains(&(personal.clone(), "c.md".to_string())),
            "{retired:?}"
        );
        assert!(
            retired.contains(&(global.clone(), "c.md".to_string())),
            "{retired:?}"
        );
    }

    #[test]
    fn the_guards_hold_per_item_in_a_bulk_delete() {
        let (_dir, notez, root) = vault();
        let sections = || {
            let mut sections = vault_sections(&notez);
            sections[1].dirs = vec![notez.join("personal")];
            sections
        };
        let rebuild = || Ok(sections());
        let mut forest = forest_of(sections());
        let mut retired = Vec::new();
        let holder = notez.join("personal");
        assert!(
            forest.nodes.iter().any(|n| n.path == holder && n.depth > 0),
            "the global section lists the folder"
        );
        let untouched = disk_entries(&notez);
        let disk: Vec<String> = disk_entries(&notez)
            .into_iter()
            .filter(|e| e != "personal/proj/top.md")
            .collect();

        // The guard refuses the whole set before anything is asked.
        let marks: HashSet<PathBuf> = [root.join("top.md"), holder.clone()].into_iter().collect();
        assert_eq!(
            bulk_delete_request(&forest.nodes, &forest.sections, &marks, Some("proj")).unwrap_err(),
            format!("personal/: {FOLDER_HOLDS_SECTION}")
        );
        assert_eq!(
            disk_entries(&notez),
            untouched,
            "a refused set deletes nothing"
        );

        // The guard runs again per item when the set is deleted.
        let items = bulk_items(&forest.nodes, &forest.sections, &marks, Some("proj"));
        let outcome = answer_bulk_delete(
            KeyCode::Char('y'),
            &mut forest,
            &mut retired,
            &items,
            "",
            &rebuild,
        )
        .unwrap();

        assert_eq!(
            outcome.message,
            format!("deleted 1, failed 1: personal/ ({FOLDER_HOLDS_SECTION})")
        );
        assert_eq!(disk_entries(&notez), disk, "only the note went");
    }

    #[test]
    fn space_is_a_browse_key_whose_footer_hint_drops_after_m_and_before_n() {
        let rows: Vec<&KeyHint> = TREE_KEYS.iter().filter(|k| k.key == "space").collect();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].modes, BROWSE);
        assert_eq!(rows[0].group, Group::Edit);
        assert_eq!(rows[0].help, "mark");
        let first_width = |key: &str| {
            (0..200)
                .find(|&w| shown_keys(Mode::Normal, &[], w).contains(&key))
                .unwrap()
        };
        assert!(
            first_width("m") > first_width("space"),
            "m drops before space"
        );
        assert!(
            first_width("space") > first_width("N"),
            "space drops before N"
        );
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
            assert!(
                !TREE_KEYS
                    .iter()
                    .any(|k| k.key == "space" && k.modes.contains(&mode)),
                "{mode:?}"
            );
        }
        let esc = TREE_KEYS
            .iter()
            .find(|k| k.key == "esc" && k.modes == BROWSE)
            .unwrap();
        assert!(esc.help.starts_with("clear marks"), "{}", esc.help);
    }

    #[test]
    fn the_footer_leads_with_the_mark_count_then_the_hints() {
        let rendered = text_of(&lead_with_hints(marked_lead(3), Mode::Normal, &[], 200));
        assert!(rendered.starts_with(" 3 marked  open  tags"), "{rendered}");
        assert!(
            rendered.contains("space mark") && rendered.trim_end().ends_with("quit"),
            "{rendered}"
        );
        let filtering = text_of(&lead_with_hints(marked_lead(1), Mode::Filter, &[], 200));
        assert!(
            filtering.starts_with(" 1 marked  enter keep"),
            "{filtering}"
        );
    }

    /// The gutter: the row's first column, before the tag field.
    const MARK_COL: usize = 0;

    #[test]
    fn a_marked_row_shows_the_mark_in_the_gutter_and_keeps_every_other_column() {
        let (sections, nodes) = badged_forest();
        for path in ["/p/.notez/d.md", "/n/personal/proj/ideas"] {
            let plain = render_row(line_of(&sections, &nodes, row(&nodes, path)));
            let line = mark_row(line_of(&sections, &nodes, row(&nodes, path)));
            let marked = render_row(line.clone());
            assert_eq!(plain[MARK_COL].0, " ", "{path}");
            assert_eq!(marked[MARK_COL].0, MARK_GLYPH, "{path}");
            for x in (0..80).filter(|&x| x != MARK_COL) {
                assert_eq!(marked[x].0, plain[x].0, "{path} column {x}");
            }
            assert!(
                line.spans
                    .iter()
                    .all(|s| s.style.add_modifier.contains(Modifier::BOLD)),
                "{path}"
            );
        }
        assert_eq!(
            mouse_x_to_field_slot(MARK_COL as u16, 0),
            None,
            "the mark is outside the tag field"
        );
    }

    // --- Marks: bulk move and set scope ---

    #[test]
    fn bulk_move_across_scopes_lands_each_item_under_its_own_name_and_the_tags_follow() {
        let (_dir, notez, roots) = move_fixture();
        let rebuild = || Ok(move_sections(&roots));
        let ctx = move_ctx(&roots);
        let mut forest = forest_of(move_sections(&roots));
        let (mut retired, mut carried) = (Vec::new(), Vec::new());
        let (personal, public, local) = (
            store_of(&roots, Scope::Personal),
            store_of(&roots, Scope::Public),
            store_of(&roots, Scope::Local),
        );
        let marked = [
            local.join("ideas"),
            notez.join("c.md"),
            personal.join("ideas/a.md"),
        ];
        let initial = final_maps(&forest, &[], &[], &roots);

        let (prompt, items) = bulk_move_at(&forest, &ctx, &marked, false).unwrap();
        assert_eq!(
            items.iter().map(|i| i.src.clone()).collect::<Vec<_>>(),
            vec![marked[2].clone(), marked[0].clone(), marked[1].clone()],
            "tree order"
        );
        assert_eq!(
            text_of(&Line::from(bulk_move_lead(&prompt, items.len()))),
            " move 3 items to personal/ideas_"
        );
        let rendered = text_of(&lead_with_hints(
            bulk_move_lead(&prompt, items.len()),
            Mode::Move,
            &[],
            200,
        ));
        assert!(
            rendered.contains("enter move") && rendered.contains("tab scope"),
            "{rendered}"
        );

        let plan = bulk_move_plan(&forest, &ctx, &marked, Scope::Public, "plans")
            .unwrap()
            .expect("a move");
        let dst = public.join("plans");
        assert_eq!(
            plan.plans.iter().map(|p| p.dst.clone()).collect::<Vec<_>>(),
            vec![dst.join("a.md"), dst.join("ideas"), dst.join("c.md")]
        );
        assert!(plan.needs_confirm());
        assert_eq!(
            plan.question(),
            "move 3 items to public (committed with the project)/plans? \
             (it will be in the p repository, public, not yet committed) \
             (it leaves the vault; the deletion syncs on exit) y/n",
            "each clause once"
        );
        let rendered = text_of(&lead_with_hints(
            bulk_move_confirm_lead(&plan),
            Mode::ConfirmMove,
            &[],
            400,
        ));
        assert!(
            rendered.starts_with(&format!(" {}", plan.question()))
                && rendered.contains("n/esc cancel"),
            "{rendered}"
        );

        let outcome = apply_bulk_move(
            &mut forest,
            &mut retired,
            &mut carried,
            &plan.plans,
            &ctx.new_note_roots,
            &rebuild,
        );
        assert_eq!(outcome.message, "moved 3");
        for file in ["a.md", "c.md", "ideas/a.md", "ideas/b.md"] {
            assert!(dst.join(file).is_file(), "{file}");
        }
        for src in &marked {
            assert!(!src.exists(), "{}", src.display());
        }
        assert_eq!(
            forest.nodes[outcome.row.expect("listed")].path,
            dst.join("a.md"),
            "the cursor is on the first moved item"
        );
        assert!(carried.is_empty());

        let notez_tags = move_tag_root(&roots, Scope::Global);
        let mut expected = initial.clone();
        for key in ["personal/proj/ideas/a.md", "c.md"] {
            expected.get_mut(&notez_tags).unwrap().remove(key);
        }
        expected.get_mut(&local).unwrap().remove("ideas/a.md");
        let public_map = expected.get_mut(&public).unwrap();
        public_map.insert("plans/a.md".to_string(), 1);
        public_map.insert("plans/c.md".to_string(), 8);
        public_map.insert("plans/ideas/a.md".to_string(), 3);
        assert_eq!(final_maps(&forest, &retired, &carried, &roots), expected);
    }

    #[test]
    fn a_bulk_move_is_refused_whole_before_anything_moves() {
        let (dir, notez, roots) = move_fixture();
        let ctx = move_ctx(&roots);
        let forest = forest_of(move_sections(&roots));
        let (personal, public, local) = (
            store_of(&roots, Scope::Personal),
            store_of(&roots, Scope::Public),
            store_of(&roots, Scope::Local),
        );
        let before = disk_entries(dir.path());
        let refused = |paths: &[PathBuf], scope: Scope, folder: &str| {
            bulk_move_plan(&forest, &ctx, paths, scope, folder).unwrap_err()
        };

        assert_eq!(
            refused(
                &[public.join("c.md"), notez.join("c.md")],
                Scope::Personal,
                "plans"
            ),
            "move: two marked items are named c.md"
        );
        assert_eq!(
            refused(
                &[personal.join("c.md"), public.join("ideas/a.md")],
                Scope::Personal,
                "ideas"
            ),
            "move: personal/ideas/a.md already exists",
            "the first collision in tree order is named"
        );
        assert_eq!(
            refused(
                &[public.join("c.md"), local.join("ideas")],
                Scope::Local,
                "ideas"
            ),
            "move: local scratch/ideas/c.md is inside the marked folder ideas/"
        );
        assert_eq!(
            refused(&[local.join("ideas")], Scope::Local, "ideas/"),
            "move: ideas/ cannot go inside itself"
        );
        assert_eq!(
            refused(&[personal.join("c.md")], Scope::Personal, "nope"),
            "move: no folder personal/nope"
        );
        assert_eq!(
            refused(
                &[personal.join("ideas/a.md"), personal.join("ideas/b.md")],
                Scope::Personal,
                "ideas"
            ),
            "move: the marked items are already in personal/ideas"
        );

        let docs = public.parent().unwrap().join("docs/d.md");
        let opened =
            bulk_move_at(&forest, &ctx, &[personal.join("c.md"), docs.clone()], false).err();
        assert_eq!(
            opened.as_deref(),
            Some(format!("d.md: {DOCS_MOVE}").as_str())
        );
        let opened = bulk_move_at(&forest, &ctx, &[docs], true).err();
        assert_eq!(
            opened.as_deref(),
            Some(format!("d.md: {DOCS_SET_SCOPE}").as_str())
        );

        let mut sections = move_sections(&roots);
        sections[3].project = Some("other".to_string());
        let other = forest_of(sections);
        let opened = bulk_move_at(
            &other,
            &ctx,
            &[personal.join("c.md"), local.join("c.md")],
            false,
        )
        .err();
        assert_eq!(opened.as_deref(), Some("move: marked items span projects"));
        let opened = bulk_move_at(
            &other,
            &ctx,
            &[personal.join("c.md"), local.join("c.md")],
            true,
        )
        .err();
        assert_eq!(
            opened.as_deref(),
            Some("set scope: marked items span projects")
        );

        assert_eq!(disk_entries(dir.path()), before, "nothing moved");
    }

    #[test]
    fn a_failing_item_in_a_bulk_move_is_reported_and_the_rest_move() {
        let (_dir, notez, roots) = move_fixture();
        let rebuild = || Ok(move_sections(&roots));
        let ctx = move_ctx(&roots);
        let mut forest = forest_of(move_sections(&roots));
        let (mut retired, mut carried) = (Vec::new(), Vec::new());
        let (personal, public) = (
            store_of(&roots, Scope::Personal),
            store_of(&roots, Scope::Public),
        );
        let marked = [personal.join("ideas/a.md"), public.join("c.md")];
        let plan = bulk_move_plan(&forest, &ctx, &marked, Scope::Personal, "plans")
            .unwrap()
            .unwrap();
        std::fs::write(personal.join("plans/a.md"), "appeared meanwhile").unwrap();

        let outcome = apply_bulk_move(
            &mut forest,
            &mut retired,
            &mut carried,
            &plan.plans,
            &ctx.new_note_roots,
            &rebuild,
        );
        assert!(
            outcome
                .message
                .starts_with("moved 1, failed 1: ideas/a.md ("),
            "{}",
            outcome.message
        );
        assert!(
            outcome.message.contains("already exists"),
            "{}",
            outcome.message
        );
        assert_eq!(
            std::fs::read_to_string(personal.join("plans/a.md")).unwrap(),
            "appeared meanwhile"
        );
        assert!(marked[0].is_file(), "the failing item stays");
        assert!(
            !marked[1].exists() && personal.join("plans/c.md").is_file(),
            "the rest moved"
        );
        assert_eq!(
            forest.nodes[outcome.row.unwrap()].path,
            personal.join("plans/c.md"),
            "the first moved item"
        );
        assert!(
            forest.nodes.iter().any(|n| n.path == marked[0]),
            "the failing item is still listed"
        );

        let maps = final_maps(&forest, &retired, &carried, &roots);
        assert_eq!(
            maps[&notez].get("personal/proj/ideas/a.md"),
            Some(&1),
            "the failing item keeps its tags"
        );
        assert_eq!(
            maps[&notez].get("personal/proj/plans/c.md"),
            Some(&2),
            "the moved item's tags follow"
        );
        assert_eq!(maps[&public].get("c.md"), None);
    }

    #[test]
    fn bulk_set_scope_keeps_each_items_folder_and_lists_each_transition_once() {
        let (dir, _notez, roots) = move_fixture();
        let (personal, public, local) = (
            store_of(&roots, Scope::Personal),
            store_of(&roots, Scope::Public),
            store_of(&roots, Scope::Local),
        );
        std::fs::write(personal.join("ideas/s.md"), "# s\n").unwrap();
        std::fs::write(public.join("plans/p.md"), "# p\n").unwrap();
        std::fs::write(local.join("ideas/l.md"), "# l\n").unwrap();
        std::fs::create_dir_all(personal.join("only")).unwrap();
        std::fs::write(personal.join("only/z.md"), "# z\n").unwrap();
        let rebuild = || Ok(move_sections(&roots));
        let ctx = move_ctx(&roots);
        let mut forest = forest_of(move_sections(&roots));
        let (mut retired, mut carried) = (Vec::new(), Vec::new());
        for (path, flags) in [
            (personal.join("ideas/s.md"), 7),
            (public.join("plans/p.md"), 3),
        ] {
            let i = row(&forest.nodes, path_str(&path));
            forest.nodes[i].flags = flags;
        }
        let initial = final_maps(&forest, &[], &[], &roots);
        let enter = |paths: &[PathBuf], scope: Scope| {
            let (mut prompt, items) = bulk_move_at(&forest, &ctx, paths, true).unwrap();
            for code in [KeyCode::Char('z'), KeyCode::Backspace] {
                type_into_move(&mut prompt, code);
            }
            tab_to(&mut prompt, scope, &ctx);
            resolve_bulk_move(&prompt, &items, &ctx.new_note_roots)
        };

        let before = disk_entries(dir.path());
        let all_personal = [personal.join("ideas/a.md"), personal.join("c.md")];
        assert_eq!(
            enter(&all_personal, Scope::Personal),
            Ok(None),
            "Enter on the scope every item is in"
        );
        assert_eq!(
            enter(
                &[personal.join("only/z.md"), personal.join("c.md")],
                Scope::Public
            )
            .unwrap_err(),
            "set scope: no folder public (committed with the project)/only"
        );
        assert_eq!(
            enter(
                &[personal.join("ideas/s.md"), personal.join("ideas/a.md")],
                Scope::Local
            )
            .unwrap_err(),
            "set scope: local scratch/ideas/a.md already exists"
        );
        assert_eq!(disk_entries(dir.path()), before, "nothing moved");

        let marked = [
            personal.join("ideas/s.md"),
            public.join("plans/p.md"),
            local.join("ideas/l.md"),
        ];
        let (prompt, items) = bulk_move_at(&forest, &ctx, &marked, true).unwrap();
        assert_eq!(
            text_of(&Line::from(bulk_move_lead(&prompt, items.len()))),
            " set scope of 3 items: personal (Tab cycles, Enter applies)"
        );
        let plan = enter(&marked, Scope::Local).unwrap().expect("a move");
        assert_eq!(
            plan.plans.iter().map(|p| p.dst.clone()).collect::<Vec<_>>(),
            vec![local.join("ideas/s.md"), local.join("plans/p.md")],
            "each keeps its folder; the item already in local scratch stays"
        );
        assert_eq!(
            plan.question(),
            "move 2 items to local scratch? (it stays in the repository's git history) \
             (scratch is not synced and not recoverable) \
             (it leaves the vault; the deletion syncs on exit) y/n"
        );

        let outcome = apply_bulk_move(
            &mut forest,
            &mut retired,
            &mut carried,
            &plan.plans,
            &ctx.new_note_roots,
            &rebuild,
        );
        assert_eq!(outcome.message, "moved 2");
        assert!(
            local.join("ideas/s.md").is_file()
                && local.join("plans/p.md").is_file()
                && local.join("ideas/l.md").is_file()
        );
        assert!(!marked[0].exists() && !marked[1].exists());
        assert_eq!(
            forest.nodes[outcome.row.unwrap()].path,
            local.join("ideas/s.md")
        );

        let notez_tags = move_tag_root(&roots, Scope::Global);
        let mut expected = initial.clone();
        expected
            .get_mut(&notez_tags)
            .unwrap()
            .remove("personal/proj/ideas/s.md");
        expected.get_mut(&public).unwrap().remove("plans/p.md");
        expected
            .get_mut(&local)
            .unwrap()
            .insert("ideas/s.md".to_string(), 7);
        expected
            .get_mut(&local)
            .unwrap()
            .insert("plans/p.md".to_string(), 3);
        assert_eq!(final_maps(&forest, &retired, &carried, &roots), expected);
    }
}
