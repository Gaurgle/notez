//! todoz core: parse, serialize, aggregate and mutate `TODO.md` boards.
//!
//! Ported from notez-cli's TUI, with the terminal layer stripped out and the
//! symlink-based file discovery replaced by notez's scope + registry model.
//! The board is a flat `Vec<Task>`; see [`model`] for the hierarchy encoding.

pub mod model;

use std::path::{Path, PathBuf};

use crate::config::{Config, ProjectRegistry};
use crate::core::NoteDate;
use crate::core::{resolve, Scope};
use crate::tags::{parse_flags, serialize_flags};

pub use model::{CheckState, Task, TodoLine};

// --- Parsing & serialization ---

/// Split one checkbox line into `(depth, state, text after the checkbox)`.
/// `None` for any other line.
fn split_checkbox_line(line: &str) -> Option<(u8, CheckState, &str)> {
    let depth = if line.starts_with("    - ") || line.starts_with("    -\t") {
        2
    } else if line.starts_with("  - ") || line.starts_with("  -\t") {
        1
    } else {
        0
    };
    let trimmed = line.trim();
    let state = if trimmed.starts_with("- [ ] ") {
        CheckState::Unchecked
    } else if trimmed.starts_with("- [/] ") {
        CheckState::Half
    } else if trimmed.starts_with("- [x] ") || trimmed.starts_with("- [X] ") {
        CheckState::Checked
    } else {
        return None;
    };
    Some((depth, state, &trimmed[6..]))
}

/// Strip the due-date token from `text`: a whitespace-delimited word that is
/// exactly `@YYYY-MM-DD` or `@YYYY-MM-DDTHH:MM`. If several words qualify,
/// the last one is the due date and the others stay in the text; since the
/// serializer writes the due date after the text, this keeps a rewritten
/// line stable. Without a token the text is returned unchanged.
fn strip_due(text: &str) -> (String, Option<NoteDate>) {
    let words: Vec<&str> = text.split_whitespace().collect();
    let found = words.iter().enumerate().rev().find_map(|(i, w)| {
        let date = NoteDate::parse(w.strip_prefix('@')?)?;
        Some((i, date))
    });
    let Some((index, date)) = found else {
        return (text.to_string(), None);
    };
    let kept: Vec<&str> = words
        .iter()
        .enumerate()
        .filter_map(|(i, w)| (i != index).then_some(*w))
        .collect();
    (kept.join(" "), Some(date))
}

/// Parse `TODO.md` content into one [`TodoLine`] per checkbox line, with the
/// tags and the `@date` due token stripped from the text. Non-checkbox lines
/// (headers, blanks, prose) are skipped.
pub fn parse_todo_lines(content: &str) -> Vec<TodoLine> {
    content
        .lines()
        .filter_map(split_checkbox_line)
        .map(|(depth, state, raw_text)| {
            let (without_due, due) = strip_due(raw_text);
            let (text, flags) = parse_flags(&without_due);
            TodoLine {
                text,
                state,
                depth,
                flags,
                due,
            }
        })
        .collect()
}

/// Parse `TODO.md` content into `(text, state, depth, flags)` tuples, one per
/// checkbox line. Non-checkbox lines (headers, blanks) are skipped.
///
/// Kept for existing callers with its pre-0.2.0 output: the `@date` token
/// stays in the text. New code should use [`parse_todo_lines`].
pub fn parse_todos_from_content(content: &str) -> Vec<(String, CheckState, u8, u8)> {
    content
        .lines()
        .filter_map(split_checkbox_line)
        .map(|(depth, state, raw_text)| {
            let (text, flags) = parse_flags(raw_text);
            (text, state, depth, flags)
        })
        .collect()
}

/// Serialize every task belonging to `source` back into `TODO.md` text.
/// Headers, code-todos and tasks from other files are skipped. A task's due
/// date is written after its text and before its tags.
pub fn serialize_tasks_for_file(items: &[Task], source: &Path) -> String {
    let mut out = String::from("# TODO\n\n");
    for item in items {
        if item.is_header || item.is_code_todo || item.source != source {
            continue;
        }
        let indent = "  ".repeat(item.depth as usize);
        let text = match (item.due, item.text.trim_end()) {
            (Some(due), "") => format!("@{due}"),
            (Some(due), text) => format!("{text} @{due}"),
            (None, _) => item.text.clone(),
        };
        let line = serialize_flags(&text, item.flags);
        out.push_str(&format!("{}- {} {}\n", indent, item.state.marker(), line));
    }
    out
}

/// Load a single `TODO.md` into tasks, deriving parent states from children.
pub fn load_single_todo(path: &Path, section: &str) -> Vec<Task> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let parsed = parse_todo_lines(&content);
    let mut items: Vec<Task> = Vec::with_capacity(parsed.len());

    for TodoLine {
        text,
        state,
        depth,
        flags,
        due,
    } in parsed
    {
        if depth > 0 {
            if let Some(parent) = items
                .iter_mut()
                .rev()
                .find(|i| !i.is_header && i.depth < depth)
            {
                parent.has_subtasks = true;
            }
        }
        items.push(Task {
            text,
            state,
            source: path.to_path_buf(),
            section: section.to_string(),
            is_header: false,
            depth,
            has_subtasks: false,
            collapsed: false,
            is_code_todo: false,
            flags,
            due,
        });
    }

    derive_parent_states(&mut items);
    items
}

// --- Hierarchy & reorder ---

/// Exclusive end index of the block at `idx`: the item plus all consecutive
/// strictly-deeper descendants. Stops at headers or same/shallower depth.
pub fn block_end(items: &[Task], idx: usize) -> usize {
    let depth = items[idx].depth;
    let mut end = idx + 1;
    while end < items.len() && !items[end].is_header && items[end].depth > depth {
        end += 1;
    }
    end
}

/// True iff `start` and `target` are valid drag endpoints: same depth, same
/// section (no header between), same parent (no shallower item between).
pub fn can_drag(items: &[Task], start: usize, target: usize) -> bool {
    if start >= items.len() || target >= items.len() {
        return false;
    }
    if items[start].is_header || items[target].is_header {
        return false;
    }
    if items[start].is_code_todo || items[target].is_code_todo {
        return false;
    }
    if items[start].depth != items[target].depth {
        return false;
    }
    let depth = items[start].depth;
    let (lo, hi) = if start <= target {
        (start, target)
    } else {
        (target, start)
    };
    for item in &items[lo..=hi] {
        if item.is_header || item.depth < depth {
            return false;
        }
    }
    true
}

/// Move the block at `start` to land at `target`'s position. Assumes
/// `can_drag` holds. Returns the moved block's new start index.
pub fn perform_drag_move(items: &mut [Task], start: usize, target: usize) -> usize {
    let start_end = block_end(items, start);
    let start_len = start_end - start;
    if start < target {
        let target_end = block_end(items, target);
        items[start..target_end].rotate_left(start_len);
        start + (target_end - start_end)
    } else if target < start {
        items[target..start_end].rotate_right(start_len);
        target
    } else {
        start
    }
}

// --- Derivation ---

/// Recompute `has_subtasks` and parent check states from direct children.
pub fn derive_parent_states(items: &mut [Task]) {
    let len = items.len();
    for i in (0..len).rev() {
        if items[i].is_header {
            continue;
        }
        let parent_depth = items[i].depth;
        let mut total = 0;
        let mut checked = 0;
        for j in (i + 1)..len {
            if items[j].depth <= parent_depth {
                break;
            }
            if items[j].depth == parent_depth + 1 {
                total += 1;
                if items[j].state == CheckState::Checked {
                    checked += 1;
                }
            }
        }
        items[i].has_subtasks = total > 0;
        if total > 0 {
            items[i].state = if checked == 0 {
                CheckState::Unchecked
            } else if checked == total {
                CheckState::Checked
            } else {
                CheckState::Half
            };
        }
    }
}

/// Aggregate child flags onto each section header.
pub fn derive_header_flags(items: &mut [Task]) {
    let len = items.len();
    for i in 0..len {
        if !items[i].is_header {
            continue;
        }
        let mut agg: u8 = 0;
        for j in (i + 1)..len {
            if items[j].is_header {
                break;
            }
            agg |= items[j].flags;
        }
        items[i].flags = agg;
    }
}

/// Collapse-aware list of indices that should be visible, honoring collapsed
/// headers and collapsed parents.
pub fn get_visible_indices(items: &[Task]) -> Vec<usize> {
    let mut visible = Vec::new();
    let mut skip_section = false;
    let mut collapse_depth: Option<u8> = None;

    for (i, item) in items.iter().enumerate() {
        if item.is_header {
            visible.push(i);
            skip_section = item.collapsed;
            collapse_depth = None;
            continue;
        }
        if skip_section {
            continue;
        }
        if let Some(cd) = collapse_depth {
            if item.depth > cd {
                continue;
            }
            collapse_depth = None;
        }
        visible.push(i);
        if item.has_subtasks && item.collapsed {
            collapse_depth = Some(item.depth);
        }
    }
    visible
}

// --- Board loading ---

fn header_task(path: &Path, section: &str, label: &str) -> Task {
    Task {
        text: label.to_string(),
        state: CheckState::Unchecked,
        source: path.to_path_buf(),
        section: section.to_string(),
        is_header: true,
        depth: 0,
        has_subtasks: false,
        collapsed: true,
        is_code_todo: false,
        flags: 0,
        due: None,
    }
}

/// Append a section's header + tasks to `out`, deduping by canonical path.
/// `always` forces the header to show even when the file is missing/empty
/// (used for `_todos` categories so the user knows they exist).
fn push_section(
    out: &mut Vec<Task>,
    seen: &mut Vec<PathBuf>,
    path: &Path,
    section: &str,
    label: &str,
    always: bool,
) {
    if !path.exists() && !always {
        return;
    }
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    if seen.contains(&canonical) {
        return;
    }
    let items = load_single_todo(path, section);
    if items.is_empty() && !always {
        return;
    }
    seen.push(canonical);
    out.push(header_task(path, section, label));
    out.extend(items);
}

/// Build the global todoz board across every scope, driven by the project
/// registry (no symlinks). Order: GLOBAL, then `_todos/<category>`, then each
/// registered project's personal / public / local `TODO.md`.
pub fn load_board(config: &Config, registry: &ProjectRegistry) -> Vec<Task> {
    let notez_root = config.notez_root_path();
    let mut seen: Vec<PathBuf> = Vec::new();
    let mut out: Vec<Task> = Vec::new();

    // Global section.
    push_section(
        &mut out,
        &mut seen,
        &notez_root.join("TODO.md"),
        "global",
        "NOTEZ",
        true,
    );

    // _todos/<category>/TODO.md - always shown.
    if let Ok(entries) = std::fs::read_dir(notez_root.join("_todos")) {
        let mut dirs: Vec<_> = entries.flatten().filter(|e| e.path().is_dir()).collect();
        dirs.sort_by_key(|e| e.file_name());
        for entry in dirs {
            let cat = entry.file_name().to_string_lossy().to_string();
            push_section(
                &mut out,
                &mut seen,
                &entry.path().join("TODO.md"),
                &cat,
                &cat.to_uppercase(),
                true,
            );
        }
    }

    // Each reachable registered project: personal, public, local.
    for (name, local_path) in registry.iter_resolved() {
        if !local_path.exists() {
            continue;
        }
        push_section(
            &mut out,
            &mut seen,
            &notez_root.join("personal").join(name).join("TODO.md"),
            name,
            &format!("{name} (personal)"),
            false,
        );
        push_section(
            &mut out,
            &mut seen,
            &local_path.join("notez").join("TODO.md"),
            name,
            &format!("{name} (public)"),
            false,
        );
        push_section(
            &mut out,
            &mut seen,
            &local_path.join(".notez").join("TODO.md"),
            name,
            &format!("{name} (scratch)"),
            false,
        );
    }

    derive_header_flags(&mut out);
    out
}

/// Load the single-scope board: the `TODO.md` at the given scope's root,
/// resolved against the current directory's project.
pub fn load_scope_board(scope: Scope, config: &Config) -> anyhow::Result<Vec<Task>> {
    let root = resolve::root(scope, config)?;
    Ok(load_single_todo(&root.join("TODO.md"), &scope.to_string()))
}

// --- Saving ---

/// Write every distinct source file represented in `items`, deduping by
/// canonical path so a file reached two ways isn't written twice.
pub fn save_all_todos(items: &[Task]) -> std::io::Result<()> {
    let all: std::collections::HashSet<PathBuf> = items
        .iter()
        .filter(|i| !i.is_code_todo)
        .map(|i| i.source.clone())
        .collect();
    save_todos_for(items, &all)
}

/// Write only the source files in `sources`; everything else on the board is
/// left untouched on disk. An empty set writes nothing. This is the save
/// path for editors that track dirty files: rewriting a `TODO.md` drops any
/// non-todo text in it (see [`has_non_todo_content`]), so only files the
/// user actually changed should be rewritten.
pub fn save_todos_for(
    items: &[Task],
    sources: &std::collections::HashSet<PathBuf>,
) -> std::io::Result<()> {
    let mut targets: Vec<PathBuf> = Vec::new();
    let mut seen_canonical: Vec<PathBuf> = Vec::new();
    for item in items {
        if item.is_code_todo
            || !sources.contains(&item.source)
            || targets.contains(&item.source)
        {
            continue;
        }
        let canonical = item
            .source
            .canonicalize()
            .unwrap_or_else(|_| item.source.clone());
        if seen_canonical.contains(&canonical) {
            continue;
        }
        seen_canonical.push(canonical);
        targets.push(item.source.clone());
    }
    for source in &targets {
        let content = serialize_tasks_for_file(items, source);
        if let Some(parent) = source.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(source, content)?;
    }
    Ok(())
}

/// True when `content` holds lines the todo parser does not model: prose,
/// extra markdown headers, anything that is not the `# TODO` title, a blank
/// line, or a checkbox line. Serializing a board over such a file drops
/// those lines, so callers should warn before rewriting it.
pub fn has_non_todo_content(content: &str) -> bool {
    content.lines().any(|line| {
        let trimmed = line.trim();
        !(trimmed.is_empty()
            || trimmed == "# TODO"
            || trimmed.starts_with("- [ ] ")
            || trimmed.starts_with("- [/] ")
            || trimmed.starts_with("- [x] ")
            || trimmed.starts_with("- [X] "))
    })
}

// --- Mutators (operate on the in-memory board; caller persists) ---

/// Toggle a task done/undone. Toggling a parent applies to all its descendants;
/// parent states are then re-derived.
pub fn toggle_done(items: &mut [Task], idx: usize) {
    if idx >= items.len() || items[idx].is_header {
        return;
    }
    let new_state = if items[idx].state == CheckState::Checked {
        CheckState::Unchecked
    } else {
        CheckState::Checked
    };
    let depth = items[idx].depth;
    items[idx].state = new_state;
    let mut j = idx + 1;
    while j < items.len() && !items[j].is_header && items[j].depth > depth {
        items[j].state = new_state;
        j += 1;
    }
    derive_parent_states(items);
}

/// Replace a task's tag flags, then re-aggregate header flags.
pub fn set_flags(items: &mut [Task], idx: usize, flags: u8) {
    if idx >= items.len() || items[idx].is_header {
        return;
    }
    items[idx].flags = flags;
    derive_header_flags(items);
}

/// Replace a task's text (tags stay in `flags`, not the text).
pub fn edit_text(items: &mut [Task], idx: usize, text: String) {
    if idx >= items.len() || items[idx].is_header {
        return;
    }
    items[idx].text = text;
}

/// Insert a new task immediately after `after`, inheriting its source/section.
/// Returns the new task's index.
pub fn add_task(items: &mut Vec<Task>, after: usize, depth: u8, text: String) -> usize {
    let (source, section) = items
        .get(after)
        .map(|t| (t.source.clone(), t.section.clone()))
        .unwrap_or_default();
    let task = Task {
        text,
        state: CheckState::Unchecked,
        source,
        section,
        is_header: false,
        depth,
        has_subtasks: false,
        collapsed: false,
        is_code_todo: false,
        flags: 0,
        due: None,
    };
    let at = (after + 1).min(items.len());
    items.insert(at, task);
    derive_parent_states(items);
    at
}

/// Remove a task and all its descendants.
pub fn remove_task(items: &mut Vec<Task>, idx: usize) {
    if idx >= items.len() || items[idx].is_header {
        return;
    }
    let end = block_end(items, idx);
    items.drain(idx..end);
    derive_parent_states(items);
}

/// Set a task's check state directly (used for the `[/]` half / "almost done"
/// mark). Parent states are re-derived afterwards.
pub fn set_state(items: &mut [Task], idx: usize, state: CheckState) {
    if idx >= items.len() || items[idx].is_header {
        return;
    }
    items[idx].state = state;
    derive_parent_states(items);
}

/// Move a task block to swap with its previous (`up`) or next sibling. Returns
/// the block's new start index, or the original index if it can't move.
pub fn move_task(items: &mut [Task], idx: usize, up: bool) -> usize {
    if idx >= items.len() || items[idx].is_header {
        return idx;
    }
    let depth = items[idx].depth;
    if up {
        // Walk back to the previous same-depth sibling's start.
        let mut j = idx;
        while j > 0 {
            j -= 1;
            if items[j].is_header || items[j].depth < depth {
                return idx; // no previous sibling
            }
            if items[j].depth == depth {
                let new = perform_drag_move(items, idx, j);
                derive_parent_states(items);
                return new;
            }
        }
        idx
    } else {
        let end = block_end(items, idx);
        if end >= items.len() || items[end].is_header || items[end].depth != depth {
            return idx; // no next sibling
        }
        let new = perform_drag_move(items, idx, end);
        derive_parent_states(items);
        new
    }
}

/// Collapse or expand every header and parent at once.
pub fn set_all_collapsed(items: &mut [Task], collapsed: bool) {
    for t in items.iter_mut() {
        if t.is_header || t.has_subtasks {
            t.collapsed = collapsed;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tags::{FLAG_BLOCKED, FLAG_IMPORTANT, FLAG_PRIO};
    use tempfile::tempdir;

    fn write_todo(content: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempdir().unwrap();
        let path = dir.path().join("TODO.md");
        std::fs::write(&path, content).unwrap();
        (dir, path)
    }

    #[test]
    fn round_trip_preserves_canonical_content() {
        // Parent state already matches its children, so derivation is a no-op
        // and the round-trip is byte-identical.
        let content = "# TODO\n\n- [x] alpha #prio\n  - [x] beta\n- [ ] gamma #important #blocked\n";
        let (_d, path) = write_todo(content);

        let items = load_single_todo(&path, "t");
        let out = serialize_tasks_for_file(&items, &path);
        assert_eq!(out, content);
    }

    #[test]
    fn parse_extracts_depth_state_and_flags() {
        let parsed = parse_todos_from_content("- [ ] top\n  - [x] child #prio\n");
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0], ("top".to_string(), CheckState::Unchecked, 0, 0));
        assert_eq!(parsed[1], ("child".to_string(), CheckState::Checked, 1, FLAG_PRIO));
    }

    #[test]
    fn derive_promotes_parent_when_all_children_checked() {
        let (_d, path) = write_todo("- [ ] parent\n  - [x] a\n  - [x] b\n");
        let items = load_single_todo(&path, "t");
        assert_eq!(items[0].state, CheckState::Checked);
        assert!(items[0].has_subtasks);
    }

    #[test]
    fn derive_sets_half_when_children_partial() {
        let (_d, path) = write_todo("- [ ] parent\n  - [x] a\n  - [ ] b\n");
        let items = load_single_todo(&path, "t");
        assert_eq!(items[0].state, CheckState::Half);
    }

    #[test]
    fn reorder_moves_block_down() {
        let (_d, path) = write_todo("- [ ] a\n- [ ] b\n- [ ] c\n");
        let mut items = load_single_todo(&path, "t");
        assert!(can_drag(&items, 0, 2));
        let new_idx = perform_drag_move(&mut items, 0, 2);
        assert_eq!(new_idx, 2);
        let texts: Vec<_> = items.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, vec!["b", "c", "a"]);
    }

    #[test]
    fn parent_drags_with_its_children() {
        let (_d, path) = write_todo("- [ ] a\n  - [ ] a1\n- [ ] b\n");
        let mut items = load_single_todo(&path, "t");
        // Move the "a" block (a + a1) past "b".
        let new_idx = perform_drag_move(&mut items, 0, 2);
        let texts: Vec<_> = items.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, vec!["b", "a", "a1"]);
        assert_eq!(items[new_idx].text, "a");
    }

    #[test]
    fn cannot_drag_across_depths() {
        let (_d, path) = write_todo("- [ ] a\n  - [ ] a1\n");
        let items = load_single_todo(&path, "t");
        assert!(!can_drag(&items, 0, 1));
    }

    #[test]
    fn toggle_parent_toggles_children() {
        let (_d, path) = write_todo("- [ ] p\n  - [ ] a\n  - [ ] b\n");
        let mut items = load_single_todo(&path, "t");
        toggle_done(&mut items, 0);
        assert_eq!(items[0].state, CheckState::Checked);
        assert_eq!(items[1].state, CheckState::Checked);
        assert_eq!(items[2].state, CheckState::Checked);
    }

    #[test]
    fn set_flags_aggregates_to_header() {
        let mut items = vec![
            header_task(Path::new("/tmp/TODO.md"), "s", "S"),
            Task {
                text: "a".into(),
                state: CheckState::Unchecked,
                source: PathBuf::from("/tmp/TODO.md"),
                section: "s".into(),
                is_header: false,
                depth: 0,
                has_subtasks: false,
                collapsed: false,
                is_code_todo: false,
                flags: 0,
                due: None,
            },
        ];
        set_flags(&mut items, 1, FLAG_IMPORTANT | FLAG_BLOCKED);
        assert_eq!(items[0].flags, FLAG_IMPORTANT | FLAG_BLOCKED);
    }

    #[test]
    fn save_round_trips_through_disk() {
        let content = "# TODO\n\n- [ ] write tests #important\n";
        let (_d, path) = write_todo(content);
        let mut items = load_single_todo(&path, "t");
        edit_text(&mut items, 0, "write more tests".into());
        save_all_todos(&items).unwrap();
        let reloaded = std::fs::read_to_string(&path).unwrap();
        assert_eq!(reloaded, "# TODO\n\n- [ ] write more tests #important\n");
    }

    #[test]
    fn save_todos_for_empty_set_writes_nothing() {
        // A file with prose would lose it on any rewrite; an empty dirty set
        // must therefore leave it byte-identical.
        let content = "# TODO\n\n## Ideas\n\nsome prose here\n\n- [ ] a\n";
        let (_d, path) = write_todo(content);
        let items = load_single_todo(&path, "t");

        save_todos_for(&items, &std::collections::HashSet::new()).unwrap();

        assert_eq!(std::fs::read_to_string(&path).unwrap(), content);
    }

    #[test]
    fn save_todos_for_touches_only_given_sources() {
        let dir = tempdir().unwrap();
        let a = dir.path().join("a/TODO.md");
        let b = dir.path().join("b/TODO.md");
        std::fs::create_dir_all(a.parent().unwrap()).unwrap();
        std::fs::create_dir_all(b.parent().unwrap()).unwrap();
        std::fs::write(&a, "# TODO\n\n- [ ] task a\n").unwrap();
        let b_content = "# TODO\n\n## prose section\n\n- [ ] task b\n";
        std::fs::write(&b, b_content).unwrap();

        let mut items = load_single_todo(&a, "a");
        items.extend(load_single_todo(&b, "b"));
        edit_text(&mut items, 0, "task a edited".into());

        let dirty: std::collections::HashSet<PathBuf> = [a.clone()].into();
        save_todos_for(&items, &dirty).unwrap();

        assert_eq!(
            std::fs::read_to_string(&a).unwrap(),
            "# TODO\n\n- [ ] task a edited\n"
        );
        // The untouched file keeps its prose, byte for byte.
        assert_eq!(std::fs::read_to_string(&b).unwrap(), b_content);
    }

    fn due(text: &str) -> Option<NoteDate> {
        Some(NoteDate::parse(text).unwrap())
    }

    #[test]
    fn parse_todo_lines_reads_the_due_token() {
        let lines = parse_todo_lines(
            "- [ ] send the contract @2026-10-20 #prio\n  - [/] call @2026-11-02T14:00 back\n",
        );
        assert_eq!(
            lines,
            vec![
                TodoLine {
                    text: "send the contract".into(),
                    state: CheckState::Unchecked,
                    depth: 0,
                    flags: FLAG_PRIO,
                    due: due("2026-10-20"),
                },
                TodoLine {
                    text: "call back".into(),
                    state: CheckState::Half,
                    depth: 1,
                    flags: 0,
                    due: due("2026-11-02T14:00"),
                },
            ]
        );
    }

    #[test]
    fn date_lookalikes_stay_in_the_text() {
        for text in [
            "mail name@example.com",
            "plans for @2026",
            "bad @2026-13-45",
            "glued x@2026-10-20",
            "punctuated @2026-10-20,",
            "bracketed (@2026-10-20)",
            "seconds @2026-10-20T14:00:00",
            "bare 2026-10-20",
        ] {
            let lines = parse_todo_lines(&format!("- [ ] {text}\n"));
            assert_eq!(lines[0].due, None, "{text:?}");
            assert_eq!(lines[0].text, text);
        }
    }

    #[test]
    fn only_one_date_token_is_stripped() {
        // The last token is the due date, so a line the serializer wrote
        // (`text @due #tags`) reads back to the same task even when the text
        // itself holds a date token.
        let lines = parse_todo_lines("- [ ] move @2026-10-01 to @2026-10-02 #idea\n");
        assert_eq!(lines[0].text, "move @2026-10-01 to");
        assert_eq!(lines[0].due, due("2026-10-02"));
    }

    #[test]
    fn tuple_parser_keeps_its_old_output() {
        let parsed = parse_todos_from_content("- [ ] a @2026-10-20 b #prio\n");
        assert_eq!(
            parsed,
            vec![(
                "a @2026-10-20 b".to_string(),
                CheckState::Unchecked,
                0,
                FLAG_PRIO
            )]
        );
    }

    #[test]
    fn serializer_writes_due_after_text_before_tags() {
        let (_d, path) = write_todo("- [ ] @2026-10-20 send #prio the contract\n");
        let items = load_single_todo(&path, "t");
        assert_eq!(items[0].due, due("2026-10-20"));
        assert_eq!(
            serialize_tasks_for_file(&items, &path),
            "# TODO\n\n- [ ] send the contract @2026-10-20 #prio\n"
        );
    }

    #[test]
    fn task_that_is_only_a_date_round_trips() {
        let content = "# TODO\n\n- [ ] @2026-10-20 #prio\n";
        let (_d, path) = write_todo(content);
        let items = load_single_todo(&path, "t");
        assert_eq!(items[0].text, "");
        assert_eq!(serialize_tasks_for_file(&items, &path), content);
    }

    #[test]
    fn added_task_has_no_due() {
        let (_d, path) = write_todo("- [ ] a @2026-10-20\n");
        let mut items = load_single_todo(&path, "t");
        let at = add_task(&mut items, 0, 0, "b".into());
        assert_eq!(items[at].due, None);
    }

    #[test]
    fn task_without_due_still_deserializes() {
        let old = "text = \"a\"\nstate = \"unchecked\"\nsource = \"/v/TODO.md\"\nsection = \"s\"\nis_header = false\ndepth = 0\nhas_subtasks = false\ncollapsed = false\nis_code_todo = false\nflags = 0\n";
        let task: Task = toml::from_str(old).unwrap();
        assert_eq!(task.due, None);
    }

    /// A `TODO.md` written in canonical form, with dates, tags, nesting, a
    /// `[/]` state and an email address, is rewritten byte for byte by the
    /// dirty-source save path; a second file with prose between dated tasks
    /// is on the same board but not dirty, so it is not rewritten at all.
    #[test]
    fn todo_md_with_dates_round_trips_byte_identically() {
        let dir = tempdir().unwrap();
        let canonical = dir.path().join("a/TODO.md");
        let prose = dir.path().join("b/TODO.md");
        std::fs::create_dir_all(canonical.parent().unwrap()).unwrap();
        std::fs::create_dir_all(prose.parent().unwrap()).unwrap();
        let canonical_text = "# TODO\n\n\
            - [ ] send the contract @2026-10-20 #prio\n\
            - [/] almost there @2026-10-21T09:30\n\
            - [/] release\n  \
              - [x] mail name@example.com about the @2026 plan #important\n  \
              - [ ] check @2026-13-45 stays text #idea\n    \
                - [ ] move @2026-10-01 to @2026-10-02 #important #blocked\n\
            - [x] plain done\n";
        let prose_text = "# TODO\n\n## This week\n\n- [ ] book venue @2026-11-02 #prio\n\n\
            Notes about the venue, ask name@example.com.\n\n  \
            - [ ] nested after prose @2026-11-03\n";
        std::fs::write(&canonical, canonical_text).unwrap();
        std::fs::write(&prose, prose_text).unwrap();

        let mut items = load_single_todo(&canonical, "a");
        items.extend(load_single_todo(&prose, "b"));
        assert_eq!(items[0].due, due("2026-10-20"));
        assert_eq!(items[1].state, CheckState::Half);
        assert_eq!(items[3].text, "mail name@example.com about the @2026 plan");
        assert_eq!(items[3].due, None);
        assert_eq!(items[7].due, due("2026-11-02"));

        let dirty: std::collections::HashSet<PathBuf> = [canonical.clone()].into();
        save_todos_for(&items, &dirty).unwrap();
        assert_eq!(std::fs::read_to_string(&canonical).unwrap(), canonical_text);
        assert_eq!(std::fs::read_to_string(&prose).unwrap(), prose_text);

        // And once more through the save-everything path for the canonical file.
        let reloaded = load_single_todo(&canonical, "a");
        save_all_todos(&reloaded).unwrap();
        assert_eq!(std::fs::read_to_string(&canonical).unwrap(), canonical_text);
    }

    #[test]
    fn detects_non_todo_content() {
        assert!(!has_non_todo_content("# TODO\n\n- [ ] a\n  - [x] b\n- [/] c\n"));
        assert!(!has_non_todo_content(""));
        assert!(has_non_todo_content("# TODO\n\n## Feature ideas\n"));
        assert!(has_non_todo_content("# TODO\n\nplain paragraph text\n"));
        assert!(has_non_todo_content("# other title\n- [ ] a\n"));
    }
}
