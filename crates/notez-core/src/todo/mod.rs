//! todoz core: parse, serialize, aggregate and mutate `TODO.md` boards.
//!
//! Ported from notez-cli's TUI, with the terminal layer stripped out and the
//! symlink-based file discovery replaced by notez's scope + registry model.
//! The board is a flat `Vec<Task>`; see [`model`] for the hierarchy encoding.

pub mod model;

use std::path::{Path, PathBuf};

use crate::config::{Config, ProjectRegistry};
use crate::core::{resolve, Scope};
use crate::tags::{parse_flags, serialize_flags};

pub use model::{CheckState, Task};

// --- Parsing & serialization ---

/// Parse `TODO.md` content into `(text, state, depth, flags)` tuples, one per
/// checkbox line. Non-checkbox lines (headers, blanks) are skipped.
pub fn parse_todos_from_content(content: &str) -> Vec<(String, CheckState, u8, u8)> {
    content
        .lines()
        .filter_map(|line| {
            let depth = if line.starts_with("    - ") || line.starts_with("    -\t") {
                2
            } else if line.starts_with("  - ") || line.starts_with("  -\t") {
                1
            } else {
                0
            };
            let trimmed = line.trim();
            let (raw_text, state) = if trimmed.starts_with("- [ ] ") {
                (&trimmed[6..], CheckState::Unchecked)
            } else if trimmed.starts_with("- [/] ") {
                (&trimmed[6..], CheckState::Half)
            } else if trimmed.starts_with("- [x] ") || trimmed.starts_with("- [X] ") {
                (&trimmed[6..], CheckState::Checked)
            } else {
                return None;
            };
            let (text, flags) = parse_flags(raw_text);
            Some((text, state, depth, flags))
        })
        .collect()
}

/// Serialize every task belonging to `source` back into `TODO.md` text.
/// Headers, code-todos and tasks from other files are skipped.
pub fn serialize_tasks_for_file(items: &[Task], source: &Path) -> String {
    let mut out = String::from("# TODO\n\n");
    for item in items {
        if item.is_header || item.is_code_todo || item.source != source {
            continue;
        }
        let indent = "  ".repeat(item.depth as usize);
        let line = serialize_flags(&item.text, item.flags);
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
    let parsed = parse_todos_from_content(&content);
    let mut items: Vec<Task> = Vec::with_capacity(parsed.len());

    for (text, state, depth, flags) in parsed {
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

    #[test]
    fn detects_non_todo_content() {
        assert!(!has_non_todo_content("# TODO\n\n- [ ] a\n  - [x] b\n- [/] c\n"));
        assert!(!has_non_todo_content(""));
        assert!(has_non_todo_content("# TODO\n\n## Feature ideas\n"));
        assert!(has_non_todo_content("# TODO\n\nplain paragraph text\n"));
        assert!(has_non_todo_content("# other title\n- [ ] a\n"));
    }
}
