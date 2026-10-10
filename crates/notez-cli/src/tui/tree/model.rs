//! The browsed model: rows ([`TreeNode`]), the [`Forest`] of sections,
//! building it from [`SectionSpec`]s, its tags, and rebuilding and reloading it.

use super::*;

/// A row in the flattened forest. Hierarchy is positional via `parent_idx`,
/// like the legacy browser.
#[derive(Debug, Clone)]
pub(super) struct TreeNode {
    pub(super) name: String,
    pub(super) path: PathBuf,
    /// Path when the browser loaded, so a rename can retire the old `.tags` key.
    pub(super) origin: PathBuf,
    pub(super) is_dir: bool,
    pub(super) depth: usize,
    pub(super) expanded: bool,
    pub(super) child_count: usize,
    pub(super) parent_idx: Option<usize>,
    pub(super) flags: u8,
    pub(super) scope_icon: &'static str,
    /// Index into the dedup'd tag-root list.
    pub(super) tag_root: usize,
    /// Index of the [`SectionSpec`] this row belongs to.
    pub(super) section: usize,
}

/// What the event loop browses, kept together so a rebuild can replace it:
/// the sections, their rows, and the tag roots with their tags as on disk
/// at the start of the session.
pub(super) struct Forest {
    pub(super) sections: Vec<SectionSpec>,
    pub(super) nodes: Vec<TreeNode>,
    /// Every tag root seen this session. A root whose last note was deleted
    /// stays listed (with no rows), so its retired keys still get written.
    pub(super) tag_roots: Vec<PathBuf>,
    pub(super) initial: Vec<HashMap<String, u8>>,
}

impl Forest {
    /// Swap in freshly listed `sections`, keeping the session state (see
    /// [`restore_state`]). Returns the row of `created`, if listed.
    pub(super) fn rebuild(&mut self, sections: Vec<SectionSpec>, created: &Path) -> Option<usize> {
        let old = self.replace(sections);
        restore_state(&old, &mut self.nodes, created)
    }

    /// Swap in `sections` listed after `deleted` was removed, keeping the
    /// session state (see [`carry_state`]). Returns the row the cursor goes
    /// to: `deleted` itself if it is still listed (the delete failed), else
    /// the next note in its folder, else the one before it, else the nearest
    /// listed ancestor, skipping rows the filter `search` hides. A folder
    /// the delete emptied stays listed (sections list their folders), so it
    /// is that nearest ancestor.
    pub(super) fn rebuild_after_delete(
        &mut self,
        sections: Vec<SectionSpec>,
        deleted: &Path,
        search: &str,
    ) -> Option<usize> {
        let candidates = cursor_candidates(&self.nodes, deleted);
        let old = self.replace(sections);
        carry_state(&old, &mut self.nodes);
        derive_dir_flags(&mut self.nodes);
        let visible = compute_visible(&self.nodes, search);
        candidates.iter().find_map(|path| {
            visible
                .iter()
                .copied()
                .find(|&i| self.nodes[i].path == *path)
        })
    }

    /// Replace the sections and rows; the new rows carry no session state
    /// yet. Returns the old rows.
    fn replace(&mut self, sections: Vec<SectionSpec>) -> Vec<TreeNode> {
        let (mut nodes, mut tag_roots) = build_forest(&sections);
        let mut initial = carry_initial_tags(&self.tag_roots, &self.initial, &tag_roots);
        apply_tags(&mut nodes, &tag_roots, &initial);
        for (root, map) in self.tag_roots.iter().zip(&self.initial) {
            if !tag_roots.contains(root) {
                tag_roots.push(root.clone());
                initial.push(map.clone());
            }
        }
        self.sections = sections;
        self.tag_roots = tag_roots;
        self.initial = initial;
        std::mem::replace(&mut self.nodes, nodes)
    }
}

/// Paths the cursor may land on once `deleted` is gone, best first:
/// `deleted` itself, its following siblings, its preceding siblings
/// nearest first, then its ancestors upward.
fn cursor_candidates(nodes: &[TreeNode], deleted: &Path) -> Vec<PathBuf> {
    let Some(idx) = nodes.iter().position(|n| n.path == deleted) else {
        return vec![deleted.to_path_buf()];
    };
    let parent = nodes[idx].parent_idx;
    let siblings: Vec<usize> = (0..nodes.len())
        .filter(|&i| nodes[i].parent_idx == parent)
        .collect();
    let pos = siblings.iter().position(|&i| i == idx).unwrap_or(0);
    let mut out = vec![deleted.to_path_buf()];
    out.extend(siblings[pos + 1..].iter().map(|&i| nodes[i].path.clone()));
    out.extend(siblings[..pos].iter().rev().map(|&i| nodes[i].path.clone()));
    let mut up = parent;
    while let Some(i) = up {
        out.push(nodes[i].path.clone());
        up = nodes[i].parent_idx;
    }
    out
}

// --- Forest construction ---

/// Intermediate per-directory grouping used to rebuild the tree shape from
/// the aggregator's flat file list.
#[derive(Default)]
struct DirTmp {
    dirs: BTreeMap<String, DirTmp>,
    files: Vec<String>,
}

impl DirTmp {
    fn insert(&mut self, comps: &[String]) {
        match comps {
            [file] => self.files.push(file.clone()),
            [dir, rest @ ..] => self.dirs.entry(dir.clone()).or_default().insert(rest),
            [] => {}
        }
    }

    /// Add the directory at `comps` (and every directory above it), with or
    /// without files in it.
    fn insert_dir(&mut self, comps: &[String]) {
        if let [dir, rest @ ..] = comps {
            self.dirs.entry(dir.clone()).or_default().insert_dir(rest);
        }
    }

    fn file_count(&self) -> usize {
        self.files.len() + self.dirs.values().map(DirTmp::file_count).sum::<usize>()
    }
}

/// Legacy sort rule: `NN_` numbered dirs sort before other dirs.
fn is_numbered(name: &str) -> bool {
    let b = name.as_bytes();
    b.len() >= 3 && b[0].is_ascii_digit() && b[1].is_ascii_digit() && b[2] == b'_'
}

/// Build the flattened forest: one depth-0 wrapper node per section, with a
/// folder row for every listed directory and every directory a file sits
/// in. Returns the nodes plus the dedup'd tag-root list they index into.
pub(super) fn build_forest(sections: &[SectionSpec]) -> (Vec<TreeNode>, Vec<PathBuf>) {
    let mut nodes: Vec<TreeNode> = Vec::new();
    let mut tag_roots: Vec<PathBuf> = Vec::new();

    for (section_idx, spec) in sections.iter().enumerate() {
        if spec.files.is_empty() && spec.dirs.is_empty() {
            continue;
        }
        let root_idx = match tag_roots.iter().position(|r| r == &spec.tag_root) {
            Some(i) => i,
            None => {
                tag_roots.push(spec.tag_root.clone());
                tag_roots.len() - 1
            }
        };

        let mut tmp = DirTmp::default();
        for file in &spec.files {
            let Ok(rel) = file.strip_prefix(&spec.root) else {
                continue;
            };
            let comps: Vec<String> = rel
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect();
            tmp.insert(&comps);
        }
        for dir in &spec.dirs {
            let Ok(rel) = dir.strip_prefix(&spec.root) else {
                continue;
            };
            let comps: Vec<String> = rel
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect();
            tmp.insert_dir(&comps);
        }

        let wrapper_idx = nodes.len();
        nodes.push(TreeNode {
            name: spec.label.clone(),
            path: spec.root.clone(),
            origin: spec.root.clone(),
            is_dir: true,
            depth: 0,
            expanded: false,
            child_count: tmp.file_count(),
            parent_idx: None,
            flags: 0,
            scope_icon: spec.icon,
            tag_root: root_idx,
            section: section_idx,
        });
        emit_children(&tmp, &spec.root, 1, wrapper_idx, root_idx, &mut nodes);
        for node in &mut nodes[wrapper_idx..] {
            node.section = section_idx;
        }
    }

    (nodes, tag_roots)
}

/// The section rows' state when the browser opens: the sections of the
/// repository it was opened in are expanded, every other section stays
/// collapsed as built. Only the first build calls this; a rebuild after a
/// create or delete carries the user's expansion instead.
pub(super) fn open_current_sections(nodes: &mut [TreeNode], sections: &[SectionSpec]) {
    for node in nodes.iter_mut().filter(|n| n.depth == 0) {
        if sections.get(node.section).is_some_and(|s| s.is_current) {
            node.expanded = true;
        }
    }
}

fn emit_children(
    tmp: &DirTmp,
    dir_path: &Path,
    depth: usize,
    parent_idx: usize,
    tag_root: usize,
    nodes: &mut Vec<TreeNode>,
) {
    let (numbered, other): (Vec<_>, Vec<_>) =
        tmp.dirs.iter().partition(|(name, _)| is_numbered(name));
    for (name, sub) in numbered.into_iter().chain(other) {
        let idx = nodes.len();
        let path = dir_path.join(name);
        nodes.push(TreeNode {
            name: name.clone(),
            path: path.clone(),
            origin: path.clone(),
            is_dir: true,
            depth,
            expanded: false,
            child_count: sub.file_count(),
            parent_idx: Some(parent_idx),
            flags: 0,
            scope_icon: "",
            tag_root,
            section: 0,
        });
        emit_children(sub, &path, depth + 1, idx, tag_root, nodes);
    }

    let mut files = tmp.files.clone();
    files.sort();
    for name in files {
        nodes.push(TreeNode {
            name: name.clone(),
            path: dir_path.join(&name),
            origin: dir_path.join(&name),
            is_dir: false,
            depth,
            expanded: false,
            child_count: 0,
            parent_idx: Some(parent_idx),
            flags: 0,
            scope_icon: "",
            tag_root,
            section: 0,
        });
    }
}

// --- Tags ---

pub(super) fn rel_key(root: &Path, path: &Path) -> Option<String> {
    path.strip_prefix(root)
        .ok()
        .map(|r| r.to_string_lossy().to_string())
        .filter(|s| !s.is_empty())
}

/// Light up file nodes from their root's loaded `.tags` map.
pub(super) fn apply_tags(
    nodes: &mut [TreeNode],
    tag_roots: &[PathBuf],
    maps: &[HashMap<String, u8>],
) {
    for node in nodes.iter_mut() {
        if node.is_dir {
            continue;
        }
        if let Some(key) = rel_key(&tag_roots[node.tag_root], &node.path) {
            if let Some(&flags) = maps[node.tag_root].get(&key) {
                node.flags = flags;
            }
        }
    }
}

/// [`changed_tag_maps_retiring`] with no deleted notes.
#[cfg(test)]
pub(super) fn changed_tag_maps(
    nodes: &[TreeNode],
    tag_roots: &[PathBuf],
    initial: &[HashMap<String, u8>],
) -> Vec<(PathBuf, HashMap<String, u8>)> {
    changed_tag_maps_retiring(nodes, tag_roots, initial, &[])
}

/// Final per-root tag maps: start from the loaded map (so keys this view
/// never showed survive untouched), drop the `retired` `(tag root, key)`
/// pairs of notes deleted this session, and overlay every file node's
/// current flags. Returns only the roots whose map differs from the loaded
/// one. Retired keys go first, so a note created again at a deleted path
/// keeps the flags its new row has.
pub(super) fn changed_tag_maps_retiring(
    nodes: &[TreeNode],
    tag_roots: &[PathBuf],
    initial: &[HashMap<String, u8>],
    retired: &[(PathBuf, String)],
) -> Vec<(PathBuf, HashMap<String, u8>)> {
    let mut finals: Vec<HashMap<String, u8>> = initial.to_vec();
    for (root, key) in retired {
        if let Some(i) = tag_roots.iter().position(|r| r == root) {
            finals[i].remove(key);
        }
    }
    for node in nodes {
        if node.is_dir {
            continue;
        }
        let Some(key) = rel_key(&tag_roots[node.tag_root], &node.path) else {
            continue;
        };
        if node.origin != node.path {
            if let Some(old_key) = rel_key(&tag_roots[node.tag_root], &node.origin) {
                finals[node.tag_root].remove(&old_key);
            }
        }
        if node.flags == 0 {
            finals[node.tag_root].remove(&key);
        } else {
            finals[node.tag_root].insert(key, node.flags);
        }
    }
    tag_roots
        .iter()
        .zip(finals)
        .zip(initial)
        .filter(|((_, fin), init)| fin != *init)
        .map(|((root, fin), _)| (root.clone(), fin))
        .collect()
}

/// Recompute directory flags as the OR of their descendants' flags. Must
/// reassign (not OR-accumulate) so bits drop when a child loses a tag.
pub(super) fn derive_dir_flags(nodes: &mut [TreeNode]) {
    let len = nodes.len();
    for i in (0..len).rev() {
        if !nodes[i].is_dir {
            continue;
        }
        let mut agg: u8 = 0;
        for j in (i + 1)..len {
            if nodes[j].depth <= nodes[i].depth {
                break;
            }
            if !nodes[j].is_dir {
                agg |= nodes[j].flags;
            }
        }
        nodes[i].flags = agg;
    }
}

// --- Rebuild ---

/// [`carry_state`], then expand the ancestors of `created` (a note or a
/// folder) so it is visible, and a created folder itself; its index is
/// returned.
pub(super) fn restore_state(
    old: &[TreeNode],
    new: &mut [TreeNode],
    created: &Path,
) -> Option<usize> {
    carry_state(old, new);
    let idx = new.iter().position(|n| n.depth > 0 && n.path == created)?;
    if new[idx].is_dir {
        new[idx].expanded = true;
    }
    let mut parent = new[idx].parent_idx;
    while let Some(p) = parent {
        new[p].expanded = true;
        parent = new[p].parent_idx;
    }
    Some(idx)
}

/// Carry the session state over to a freshly built forest: directories keep
/// their expanded state and files their tags (including edits not yet
/// saved) and original path, matched by path.
fn carry_state(old: &[TreeNode], new: &mut [TreeNode]) {
    let by_path: HashMap<&Path, &TreeNode> = old.iter().map(|n| (n.path.as_path(), n)).collect();
    for node in new.iter_mut() {
        let Some(prev) = by_path.get(node.path.as_path()) else {
            continue;
        };
        if node.is_dir {
            node.expanded = prev.expanded;
        } else {
            node.flags = prev.flags;
            node.origin = prev.origin.clone();
        }
    }
}

/// Map row indices recorded against `old` (focus mode's saved expansion) to
/// the same paths in `new`, dropping rows that no longer exist.
pub(super) fn remap_rows(
    old: &[TreeNode],
    new: &[TreeNode],
    rows: &[(usize, bool)],
) -> Vec<(usize, bool)> {
    rows.iter()
        .filter_map(|&(i, v)| {
            let path = &old.get(i)?.path;
            new.iter().position(|n| &n.path == path).map(|j| (j, v))
        })
        .collect()
}

/// Tag maps as they were on disk when the session started, for the roots
/// of a rebuilt forest: known roots keep their snapshot, new ones load.
fn carry_initial_tags(
    old_roots: &[PathBuf],
    old_initial: &[HashMap<String, u8>],
    new_roots: &[PathBuf],
) -> Vec<HashMap<String, u8>> {
    new_roots
        .iter()
        .map(|r| match old_roots.iter().position(|o| o == r) {
            Some(i) => old_initial[i].clone(),
            None => note_tags::load_tags(r),
        })
        .collect()
}

// --- Reload (`R` and the file-change probe) ---

/// How long the event loop waits for input before it probes the disk.
pub(super) const PROBE_INTERVAL: Duration = Duration::from_secs(2);

/// The modified time of each probed directory; `None` when it could not be
/// read (the path is gone or unreadable).
pub(super) type ProbeSnapshot = HashMap<PathBuf, Option<SystemTime>>;

/// List the sections again through `rebuild` and swap them in, keeping the
/// session state the delete rebuild keeps: expanded folders, unsaved tag
/// edits and original paths, matched by path (see [`carry_state`]). Unlike
/// the post-create rebuild nothing is expanded, so the folder under the
/// cursor stays as it was. Returns the row the cursor goes to: `cursor`
/// itself while it is listed, else its nearest listed neighbour as after a
/// delete, among the rows the filter `search` shows. A failed listing leaves
/// the forest untouched and returns the footer message.
fn reload_forest(
    forest: &mut Forest,
    rebuild: &dyn Fn() -> Result<Vec<SectionSpec>>,
    cursor: &Path,
    search: &str,
) -> std::result::Result<Option<usize>, String> {
    let sections = rebuild().map_err(|e| format!("reload failed: {e:#}"))?;
    Ok(forest.rebuild_after_delete(sections, cursor, search))
}

/// The directories the probe stats: every section root, listed as a row or
/// not, and every expanded folder row. A collapsed folder is not probed, so
/// a change inside it shows on the next reload (`R`, or one the probe
/// triggers elsewhere).
pub(super) fn probe_paths(sections: &[SectionSpec], nodes: &[TreeNode]) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = sections.iter().map(|s| s.root.clone()).collect();
    paths.extend(
        nodes
            .iter()
            .filter(|n| n.depth > 0 && n.is_dir && n.expanded)
            .map(|n| n.path.clone()),
    );
    paths.sort();
    paths.dedup();
    paths
}

/// The modified time of each of `paths`, from `metadata`: one `stat` per
/// path, no file is opened or read. Symlinks are followed, so a section
/// root or folder that is a link reports its target directory's time and a
/// change under the target is seen.
pub(super) fn probe_snapshot(paths: &[PathBuf]) -> ProbeSnapshot {
    paths
        .iter()
        .map(|p| {
            (
                p.clone(),
                std::fs::metadata(p).and_then(|m| m.modified()).ok(),
            )
        })
        .collect()
}

/// Whether `new` shows a change on disk against `old`: a path probed in both
/// whose modified time differs, which covers a path that appeared or
/// vanished. A path in only one of them is no change: its folder was just
/// expanded (a new baseline) or collapsed (no longer probed).
pub(super) fn probe_changed(old: &ProbeSnapshot, new: &ProbeSnapshot) -> bool {
    new.iter()
        .any(|(path, time)| old.get(path).is_some_and(|before| before != time))
}

/// Merge a fresh reading of `forest` into the probe's last one. Call it
/// only right after the forest was listed again from disk (`R`, or a
/// create, delete, rename or move whose `rebuild` succeeded), so the next
/// idle probe does not reload for that again. Never after a refused,
/// failed or in-place action: a change made elsewhere while its prompt
/// was open (the probe does not run then) would be absorbed unseen.
pub(super) fn refresh_probe(probe: &mut Option<ProbeSnapshot>, forest: &Forest) {
    let reading = probe_snapshot(&probe_paths(&forest.sections, &forest.nodes));
    probe.get_or_insert_with(HashMap::new).extend(reading);
}

/// After `Enter` in the rename prompt on row `idx`, whose path was
/// `before`. A rename renames in place without listing again, so one that
/// moved the row (its path changed) lists the forest again through
/// [`reload_view_at`], cursor on the renamed row, and on success refreshes
/// the probe. A refused, failed or unchanged rename does neither and
/// returns `None`; otherwise the reload's result.
pub(super) fn relist_after_rename(
    forest: &mut Forest,
    rebuild: &dyn Fn() -> Result<Vec<SectionSpec>>,
    state: &mut ListState,
    pre_focus_expanded: &mut Vec<(usize, bool)>,
    search: &str,
    probe: &mut Option<ProbeSnapshot>,
    idx: usize,
    before: &Path,
) -> Option<std::result::Result<Option<usize>, String>> {
    let after = forest
        .nodes
        .get(idx)
        .map(|n| n.path.clone())
        .filter(|p| p != before)?;
    let result = reload_view_at(forest, rebuild, state, pre_focus_expanded, search, after);
    if result.is_ok() {
        refresh_probe(probe, forest);
    }
    Some(result)
}

/// `R` and a probe-triggered reload in the event loop: [`reload_forest`]
/// from the cursor row, then the cursor goes to the returned row and focus
/// mode's saved expansion follows its paths. Returns the cursor's new row
/// when it stayed on the same path, so the preview keeps its scroll.
pub(super) fn reload_view(
    forest: &mut Forest,
    rebuild: &dyn Fn() -> Result<Vec<SectionSpec>>,
    state: &mut ListState,
    pre_focus_expanded: &mut Vec<(usize, bool)>,
    search: &str,
) -> std::result::Result<Option<usize>, String> {
    let visible = compute_visible(&forest.nodes, search);
    let cursor = visible
        .get(state.selected().unwrap_or(0))
        .map(|&i| forest.nodes[i].path.clone())
        .unwrap_or_default();
    reload_view_at(forest, rebuild, state, pre_focus_expanded, search, cursor)
}

/// [`reload_view`] with the cursor on `cursor` rather than the selected row.
fn reload_view_at(
    forest: &mut Forest,
    rebuild: &dyn Fn() -> Result<Vec<SectionSpec>>,
    state: &mut ListState,
    pre_focus_expanded: &mut Vec<(usize, bool)>,
    search: &str,
    cursor: PathBuf,
) -> std::result::Result<Option<usize>, String> {
    let old_nodes = forest.nodes.clone();
    let row = reload_forest(forest, rebuild, &cursor, search)?;
    *pre_focus_expanded = remap_rows(&old_nodes, &forest.nodes, pre_focus_expanded);
    let visible = compute_visible(&forest.nodes, search);
    let pos = row.and_then(|r| visible.iter().position(|&i| i == r));
    state.select(Some(pos.unwrap_or(0)));
    Ok(row.filter(|&r| forest.nodes[r].path == cursor))
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use super::*;
    use notez_core::tags::{FLAG_IMPORTANT, FLAG_PRIO};

    #[test]
    fn forest_depth_and_parents_are_positional() {
        let s = spec("/r", "S", &["dir/sub/deep.md"]);
        let (nodes, _) = build_forest(&[s]);
        // wrapper(0) > dir(1) > sub(2) > deep.md(3)
        assert_eq!(nodes[1].depth, 1);
        assert_eq!(nodes[2].depth, 2);
        assert_eq!(nodes[3].depth, 3);
        assert_eq!(nodes[3].parent_idx, Some(2));
        assert_eq!(nodes[2].parent_idx, Some(1));
        assert_eq!(nodes[1].parent_idx, Some(0));
        assert!(!nodes[3].is_dir);
        assert_eq!(nodes[0].child_count, 1);
    }

    #[test]
    fn visibility_respects_collapsed_wrappers() {
        let s = spec("/r", "S", &["dir/a.md", "b.md"]);
        let (mut nodes, _) = build_forest(&[s]);
        // Everything starts collapsed: only the wrapper shows.
        assert_eq!(get_visible_nodes(&nodes), vec![0]);
        nodes[0].expanded = true;
        let vis = get_visible_nodes(&nodes);
        // Wrapper, dir (collapsed), b.md; not dir/a.md.
        assert_eq!(vis.len(), 3);
    }

    #[test]
    fn changed_tag_maps_reports_nothing_without_edits() {
        let s = spec("/r", "S", &["a.md"]);
        let (mut nodes, roots) = build_forest(&[s]);
        let initial = vec![HashMap::from([("a.md".to_string(), FLAG_PRIO)])];
        apply_tags(&mut nodes, &roots, &initial);
        assert!(changed_tag_maps(&nodes, &roots, &initial).is_empty());
    }

    #[test]
    fn changed_tag_maps_preserves_unknown_keys() {
        let s = spec("/r", "S", &["a.md"]);
        let (mut nodes, roots) = build_forest(&[s]);
        // An entry for a file this view does not show (e.g. another scope).
        let initial = vec![HashMap::from([(
            "elsewhere/hidden.md".to_string(),
            FLAG_IMPORTANT,
        )])];
        apply_tags(&mut nodes, &roots, &initial);
        let file = nodes.iter().position(|n| n.name == "a.md").unwrap();
        nodes[file].flags = FLAG_PRIO;
        let changed = changed_tag_maps(&nodes, &roots, &initial);
        assert_eq!(changed.len(), 1);
        let map = &changed[0].1;
        assert_eq!(map.get("a.md"), Some(&FLAG_PRIO));
        assert_eq!(
            map.get("elsewhere/hidden.md"),
            Some(&FLAG_IMPORTANT),
            "keys outside the view must survive a save",
        );
    }

    #[test]
    fn sections_share_a_dedup_tag_root() {
        let a = spec("/notez", "GLOBAL", &["top.md"]);
        let mut b = spec("/notez/personal/p", "p (personal)", &["n.md"]);
        b.tag_root = PathBuf::from("/notez");
        let (nodes, roots) = build_forest(&[a, b]);
        assert_eq!(roots.len(), 1);
        assert!(nodes.iter().all(|n| n.tag_root == 0));
    }

    #[test]
    fn renamed_note_moves_its_tag_key() {
        let s = spec("/r", "S", &["2026-10-06-untitled.md"]);
        let (mut nodes, roots) = build_forest(&[s]);
        let initial = vec![HashMap::from([(
            "2026-10-06-untitled.md".to_string(),
            FLAG_PRIO,
        )])];
        apply_tags(&mut nodes, &roots, &initial);

        nodes[1].path = PathBuf::from("/r/2026-10-06-real.md");
        let changed = changed_tag_maps(&nodes, &roots, &initial);

        assert_eq!(changed.len(), 1);
        assert_eq!(
            changed[0].1,
            HashMap::from([("2026-10-06-real.md".to_string(), FLAG_PRIO)])
        );
    }

    #[test]
    fn rebuild_keeps_unsaved_tag_edits_for_the_save_on_exit() {
        let before = vec![spec("/r", "S", &["one.md"])];
        let (mut old, roots) = build_forest(&before);
        let initial = vec![HashMap::new()];
        apply_tags(&mut old, &roots, &initial);
        let one = row(&old, "/r/one.md");
        old[one].flags = FLAG_PRIO;

        let mut forest = Forest {
            sections: before,
            nodes: old,
            tag_roots: roots,
            initial,
        };
        let created = forest.rebuild(
            vec![spec("/r", "S", &["new.md", "one.md"])],
            Path::new("/r/new.md"),
        );

        assert_eq!(created, Some(row(&forest.nodes, "/r/new.md")));
        let changed = changed_tag_maps(&forest.nodes, &forest.tag_roots, &forest.initial);
        assert_eq!(
            changed,
            vec![(
                PathBuf::from("/r"),
                HashMap::from([("one.md".to_string(), FLAG_PRIO)])
            )]
        );
    }

    #[test]
    fn rebuild_without_the_new_note_selects_nothing() {
        let sections = vec![spec("/r", "S", &["one.md"])];
        let (old, _) = build_forest(&sections);
        let (mut new, _) = build_forest(&sections);
        assert_eq!(
            restore_state(&old, &mut new, Path::new("/r/missing.md")),
            None
        );
    }

    #[test]
    fn focus_rows_follow_their_paths_across_a_rebuild() {
        let (old, _) = build_forest(&[spec("/r", "S", &["b/x.md"])]);
        let (new, _) = build_forest(&[spec("/r", "S", &["a/y.md", "b/x.md"])]);
        let rows = vec![(row(&old, "/r/b"), true), (99, false)];
        assert_eq!(
            remap_rows(&old, &new, &rows),
            vec![(row(&new, "/r/b"), true)]
        );
    }

    #[test]
    fn new_note_footer_draws_the_prompt_then_its_hints() {
        assert_eq!(
            shown_keys(Mode::NewItem, &[], 200),
            vec!["enter", "esc", "tab", "bksp"]
        );
        let lead = new_note_lead("public (committed with the project)/plans", "draft");
        let rendered = text_of(&lead_with_hints(lead, Mode::NewItem, &[], 120));
        assert!(
            rendered.starts_with(" new note in public (committed with the project)/plans: draft_"),
            "{rendered}"
        );
        assert!(
            rendered.contains("create") && rendered.contains("cancel"),
            "{rendered}"
        );
    }

    // --- Initial expansion ---

    /// The section rows of `nodes` with their expanded state.
    fn section_rows(nodes: &[TreeNode]) -> Vec<(&str, bool)> {
        nodes
            .iter()
            .filter(|n| n.depth == 0)
            .map(|n| (path_str(&n.path), n.expanded))
            .collect()
    }

    #[test]
    fn the_current_repositorys_sections_open_expanded_and_the_rest_collapsed() {
        let mut sections = project_sections();
        sections[0].is_current = true;
        sections[1].is_current = true;
        let (mut nodes, _) = build_forest(&sections);
        open_current_sections(&mut nodes, &sections);
        assert_eq!(
            section_rows(&nodes),
            vec![
                ("/n/personal/proj", true),
                ("/p/notez", true),
                ("/p/docs", false),
                ("/p/.notez", false),
                ("/n", false),
            ],
        );
        assert!(
            nodes
                .iter()
                .filter(|n| n.depth > 0 && n.is_dir)
                .all(|n| !n.expanded),
            "folders inside a section stay collapsed",
        );
    }

    #[test]
    fn with_no_current_section_every_section_opens_as_before() {
        let sections = project_sections();
        let (mut nodes, _) = build_forest(&sections);
        open_current_sections(&mut nodes, &sections);
        let (built, _) = build_forest(&sections);
        assert_eq!(section_rows(&nodes), section_rows(&built));
    }

    #[test]
    fn a_rebuild_keeps_the_users_expansion_over_the_initial_one() {
        let current = || {
            let mut sections = project_sections();
            sections[0].is_current = true;
            sections
        };
        let sections = current();
        let (mut nodes, tag_roots) = build_forest(&sections);
        open_current_sections(&mut nodes, &sections);
        let initial = vec![HashMap::new(); tag_roots.len()];
        let mut forest = Forest {
            sections,
            nodes,
            tag_roots,
            initial,
        };
        let personal = row(&forest.nodes, "/n/personal/proj");
        let global = row(&forest.nodes, "/n");
        forest.nodes[personal].expanded = false;
        forest.nodes[global].expanded = true;

        forest.rebuild(current(), Path::new("/n/none.md"));

        assert!(!forest.nodes[row(&forest.nodes, "/n/personal/proj")].expanded);
        assert!(forest.nodes[row(&forest.nodes, "/n")].expanded);
    }

    // --- Folders ---

    /// [`spec`] with `dirs` (relative to `root`) listed as its directories.
    fn spec_with_dirs(root: &str, files: &[&str], dirs: &[&str]) -> SectionSpec {
        SectionSpec {
            dirs: dirs.iter().map(|d| PathBuf::from(root).join(d)).collect(),
            ..spec(root, "S", files)
        }
    }

    #[test]
    fn a_listed_directory_is_a_folder_row_with_or_without_files() {
        let s = spec_with_dirs("/r", &["full/a.md"], &["full", "empty", "outer/inner"]);
        let (nodes, _) = build_forest(&[s]);
        for (path, depth, count) in [
            ("/r/full", 1, 1),
            ("/r/empty", 1, 0),
            ("/r/outer", 1, 0),
            ("/r/outer/inner", 2, 0),
        ] {
            let node = &nodes[row(&nodes, path)];
            assert!(node.is_dir, "{path}");
            assert_eq!((node.depth, node.child_count), (depth, count), "{path}");
        }
        let inner = row(&nodes, "/r/outer/inner");
        assert_eq!(nodes[inner].parent_idx, Some(row(&nodes, "/r/outer")));
        assert_eq!(
            nodes
                .iter()
                .filter(|n| n.path == Path::new("/r/full"))
                .count(),
            1
        );
    }

    #[test]
    fn a_section_with_only_empty_folders_still_has_a_row() {
        let (nodes, roots) = build_forest(&[spec_with_dirs("/r", &[], &["empty"])]);
        assert_eq!(nodes.len(), 2);
        assert_eq!(roots, vec![PathBuf::from("/r")]);
        assert_eq!(nodes[1].path, PathBuf::from("/r/empty"));
    }

    #[test]
    fn a_directory_outside_the_section_root_is_ignored() {
        let mut s = spec("/r", "S", &["a.md"]);
        s.dirs = vec![PathBuf::from("/elsewhere/x")];
        let (nodes, _) = build_forest(&[s]);
        assert!(!nodes.iter().any(|n| n.path.starts_with("/elsewhere")));
    }

    #[test]
    fn new_folder_targets_the_folder_under_the_cursor_like_n() {
        let here = Some("proj");
        let cases = [
            (
                "/n/personal/proj/ideas",
                target("/n/personal/proj/ideas", Scope::Personal, "personal/ideas"),
            ),
            (
                "/n/personal/proj/ideas/a.md",
                target("/n/personal/proj/ideas", Scope::Personal, "personal/ideas"),
            ),
            (
                "/n/personal/proj",
                target("/n/personal/proj", Scope::Personal, "personal"),
            ),
            (
                "/p/notez",
                target(
                    "/p/notez",
                    Scope::Public,
                    "public (committed with the project)",
                ),
            ),
            (
                "/p/notez/plans/b.md",
                target(
                    "/p/notez/plans",
                    Scope::Public,
                    "public (committed with the project)/plans",
                ),
            ),
            (
                "/p/.notez",
                target("/p/.notez", Scope::Local, "local scratch"),
            ),
            (
                "/p/.notez/d.md",
                target("/p/.notez", Scope::Local, "local scratch"),
            ),
            ("/n", target("/n", Scope::Global, "global")),
            ("/n/e.md", target("/n", Scope::Global, "global")),
        ];
        for (path, expected) in cases {
            let prompt = folder_prompt_at(path, here).expect(path);
            assert!(prompt.is_folder, "{path}");
            assert_eq!(prompt.target, expected, "{path}");
            assert_eq!(prompt.origin, expected, "{path}");
        }
    }

    // --- NZ-34: reload on demand and when files change ---

    #[test]
    fn r_after_an_external_create_lists_the_note_and_keeps_cursor_expansion_filter_and_marks() {
        let (_dir, roots) = temp_tree();
        let rebuild = || Ok(list_sections(&roots));
        let mut forest = forest_of(list_sections(&roots));
        let root = roots[0].1.clone();
        let p = |rel: &str| root.join(rel).to_string_lossy().into_owned();
        for path in [p(""), p("ideas")] {
            let i = row(&forest.nodes, path.trim_end_matches('/'));
            forest.nodes[i].expanded = true;
        }
        let c = row(&forest.nodes, &p("c.md"));
        forest.nodes[c].flags = FLAG_PRIO;
        let mut marks = marks_of(&[&p("ideas/a.md"), &p("ideas/b.md")]);
        let search = "a";

        // Another shell adds a note and removes a marked one.
        std::fs::write(root.join("ideas/a2.md"), "# new\n").unwrap();
        std::fs::remove_file(root.join("ideas/b.md")).unwrap();

        let cursor = PathBuf::from(p("ideas/a.md"));
        let sel = reload_forest(&mut forest, &rebuild, &cursor, search).expect("the listing works");
        prune_marks(&mut marks, &forest.nodes);

        let n = &forest.nodes;
        let visible = compute_visible(n, search);
        assert_eq!(
            sel,
            Some(row(n, &p("ideas/a.md"))),
            "the cursor stays on its row"
        );
        assert!(
            visible.contains(&row(n, &p("ideas/a2.md"))),
            "the new note is listed and passes the filter"
        );
        assert!(!n.iter().any(|node| node.path == root.join("ideas/b.md")));
        assert!(n[row(n, root.to_str().unwrap())].expanded && n[row(n, &p("ideas"))].expanded);
        assert!(
            !n[row(n, roots[1].1.to_str().unwrap())].expanded,
            "a closed section stays closed"
        );
        assert_eq!(
            n[row(n, &p("c.md"))].flags,
            FLAG_PRIO,
            "unsaved tag edits survive"
        );
        assert_eq!(
            marks,
            marks_of(&[&p("ideas/a.md")]),
            "a mark on a vanished row is pruned"
        );
    }

    #[test]
    fn r_does_not_expand_the_folder_under_the_cursor() {
        let (_dir, roots) = temp_tree();
        let rebuild = || Ok(list_sections(&roots));
        let mut forest = forest_of(list_sections(&roots));
        let ideas = roots[0].1.join("ideas");
        let section = row(&forest.nodes, roots[0].1.to_str().unwrap());
        forest.nodes[section].expanded = true;
        let sel = reload_forest(&mut forest, &rebuild, &ideas, "").unwrap();
        let n = &forest.nodes;
        assert_eq!(sel, Some(row(n, ideas.to_str().unwrap())));
        assert!(
            !n[row(n, ideas.to_str().unwrap())].expanded,
            "the cursor folder stays collapsed"
        );
    }

    #[test]
    fn r_moves_the_cursor_to_the_next_note_when_its_note_vanished() {
        let (_dir, roots) = temp_tree();
        let rebuild = || Ok(list_sections(&roots));
        let mut forest = forest_of(list_sections(&roots));
        let root = &roots[0].1;
        for path in [root.clone(), root.join("ideas")] {
            let i = row(&forest.nodes, path.to_str().unwrap());
            forest.nodes[i].expanded = true;
        }
        std::fs::remove_file(root.join("ideas/a.md")).unwrap();
        let sel = reload_forest(&mut forest, &rebuild, &root.join("ideas/a.md"), "").unwrap();
        assert_eq!(
            sel,
            Some(row(
                &forest.nodes,
                root.join("ideas/b.md").to_str().unwrap()
            ))
        );
    }

    #[test]
    fn a_failed_reload_keeps_the_tree_and_names_the_error() {
        let (_dir, roots) = temp_tree();
        let mut forest = forest_of(list_sections(&roots));
        let before: Vec<PathBuf> = forest.nodes.iter().map(|n| n.path.clone()).collect();
        let failing = || -> Result<Vec<SectionSpec>> { Err(anyhow::anyhow!("disk gone")) };
        let err = reload_forest(&mut forest, &failing, Path::new(""), "").unwrap_err();
        assert_eq!(err, "reload failed: disk gone");
        let after: Vec<PathBuf> = forest.nodes.iter().map(|n| n.path.clone()).collect();
        assert_eq!(before, after);
    }

    fn snapshot(entries: &[(&str, Option<SystemTime>)]) -> ProbeSnapshot {
        entries
            .iter()
            .map(|(p, t)| (PathBuf::from(p), *t))
            .collect()
    }

    #[test]
    fn the_probe_sees_a_changed_root_or_expanded_folder_and_nothing_else() {
        let old = snapshot(&[("/r", at(1)), ("/r/ideas", at(2))]);
        assert!(!probe_changed(&old, &old.clone()), "no change");
        assert!(
            probe_changed(&old, &snapshot(&[("/r", at(5)), ("/r/ideas", at(2))])),
            "root mtime"
        );
        assert!(
            probe_changed(&old, &snapshot(&[("/r", at(1)), ("/r/ideas", at(5))])),
            "folder mtime"
        );
        assert!(
            probe_changed(&old, &snapshot(&[("/r", at(1)), ("/r/ideas", None)])),
            "folder vanished"
        );
        let missing = snapshot(&[("/r", None)]);
        assert!(
            probe_changed(&missing, &snapshot(&[("/r", at(1))])),
            "root appeared"
        );
    }

    #[test]
    fn a_path_probed_on_one_side_only_is_no_change() {
        let old = snapshot(&[("/r", at(1))]);
        let expanded = snapshot(&[("/r", at(1)), ("/r/ideas", at(9))]);
        assert!(
            !probe_changed(&old, &expanded),
            "a folder just expanded is a new baseline"
        );
        assert!(
            !probe_changed(&expanded, &old),
            "a folder just collapsed is not probed"
        );
    }

    #[test]
    fn the_probe_paths_are_every_section_root_and_every_expanded_folder() {
        let files = ["ideas/a.md", "ideas/deep/x.md", "plans/p.md", "z.md"];
        let sections = vec![spec("/r", "S", &files), spec("/empty", "E", &[])];
        let (mut nodes, _) = build_forest(&sections);
        let i = row(&nodes, "/r/ideas");
        nodes[i].expanded = true;
        let paths = probe_paths(&sections, &nodes);
        let mut got: Vec<&str> = paths.iter().map(|p| p.to_str().unwrap()).collect();
        got.sort();
        assert_eq!(
            got,
            ["/empty", "/r", "/r/ideas"],
            "collapsed folders and files are not probed"
        );
    }

    #[test]
    fn a_collapsed_folders_change_is_ignored_and_an_expanded_ones_is_seen() {
        let (_dir, roots) = temp_tree();
        std::fs::create_dir_all(roots[0].1.join("plans")).unwrap();
        std::fs::write(roots[0].1.join("plans/p.md"), "# p\n").unwrap();
        let sections = list_sections(&roots);
        let (mut nodes, _) = build_forest(&sections);
        let ideas = roots[0].1.join("ideas");
        let plans = roots[0].1.join("plans");
        let i = row(&nodes, ideas.to_str().unwrap());
        nodes[i].expanded = true;
        let touch = |dir: &Path, secs: u64| {
            let when = SystemTime::UNIX_EPOCH + Duration::from_secs(secs);
            std::fs::File::open(dir)
                .unwrap()
                .set_modified(when)
                .unwrap();
        };
        touch(&ideas, 1_000);
        touch(&plans, 1_000);
        let base = probe_snapshot(&probe_paths(&sections, &nodes));

        touch(&plans, 2_000);
        assert!(
            !probe_changed(&base, &probe_snapshot(&probe_paths(&sections, &nodes))),
            "plans is collapsed"
        );

        touch(&ideas, 2_000);
        assert!(
            probe_changed(&base, &probe_snapshot(&probe_paths(&sections, &nodes))),
            "ideas is expanded"
        );
    }

    #[test]
    fn a_refreshed_probe_sees_no_change_until_the_disk_changes_again() {
        let (_dir, roots) = temp_tree();
        let mut forest = forest_of(list_sections(&roots));
        let ideas = roots[0].1.join("ideas");
        let i = row(&forest.nodes, ideas.to_str().unwrap());
        forest.nodes[i].expanded = true;
        let touch = |secs: u64| {
            let when = SystemTime::UNIX_EPOCH + Duration::from_secs(secs);
            std::fs::File::open(&ideas)
                .unwrap()
                .set_modified(when)
                .unwrap();
        };
        let fresh = |forest: &Forest| probe_snapshot(&probe_paths(&forest.sections, &forest.nodes));
        touch(1_000);
        let mut probe = None;
        refresh_probe(&mut probe, &forest);

        // The browser's own change, then the refresh it makes after it.
        touch(2_000);
        assert!(
            probe_changed(probe.as_ref().unwrap(), &fresh(&forest)),
            "unrefreshed, the probe would reload"
        );
        refresh_probe(&mut probe, &forest);
        assert!(
            !probe_changed(probe.as_ref().unwrap(), &fresh(&forest)),
            "refreshed, no reload for it"
        );

        touch(3_000);
        assert!(
            probe_changed(probe.as_ref().unwrap(), &fresh(&forest)),
            "a later change is still seen"
        );
    }

    #[test]
    fn reload_view_keeps_the_filter_and_focus_expansion_and_says_whether_the_cursor_path_was_kept()
    {
        let (_dir, roots) = temp_tree();
        let rebuild = || Ok(list_sections(&roots));
        let mut forest = forest_of(list_sections(&roots));
        let root = &roots[0].1;
        let path = |rel: &str| root.join(rel);
        for dir in [root.clone(), path("ideas")] {
            let i = row(&forest.nodes, dir.to_str().unwrap());
            forest.nodes[i].expanded = true;
        }
        let search = "b";
        let b = row(&forest.nodes, path("ideas/b.md").to_str().unwrap());
        let mut state = ListState::default();
        state.select(
            compute_visible(&forest.nodes, search)
                .iter()
                .position(|&i| i == b),
        );
        // Focus mode saved the second section's row as collapsed.
        let other = row(&forest.nodes, roots[1].1.to_str().unwrap());
        let mut saved = vec![(other, false)];

        // A note listed before the cursor shifts every row index after it.
        std::fs::write(path("ideas/ab.md"), "# ab\n").unwrap();
        let kept = reload_view(&mut forest, &rebuild, &mut state, &mut saved, search).unwrap();

        let n = &forest.nodes;
        let b = row(n, path("ideas/b.md").to_str().unwrap());
        assert_eq!(
            kept,
            Some(b),
            "the cursor stayed on its path, at its new row"
        );
        assert_eq!(compute_visible(n, search)[state.selected().unwrap()], b);
        assert!(compute_visible(n, search).contains(&row(n, path("ideas/ab.md").to_str().unwrap())));
        assert_eq!(
            saved,
            vec![(row(n, roots[1].1.to_str().unwrap()), false)],
            "focus expansion follows its path"
        );

        std::fs::remove_file(path("ideas/b.md")).unwrap();
        let kept = reload_view(&mut forest, &rebuild, &mut state, &mut saved, search).unwrap();
        assert_eq!(
            kept, None,
            "the cursor's note vanished, so its path was not kept"
        );
        assert!(state.selected().is_some());
    }

    /// A temp tree with the first store's `ideas` folder expanded, its mtime
    /// set to 1000 s and recorded by the probe, then set to 2000 s: a change
    /// made elsewhere while a prompt was open, which the probe still owes.
    fn probe_with_a_pending_change() -> (
        tempfile::TempDir,
        Vec<(Scope, PathBuf)>,
        Forest,
        Option<ProbeSnapshot>,
    ) {
        let (dir, roots) = temp_tree();
        let mut forest = forest_of(list_sections(&roots));
        let ideas = roots[0].1.join("ideas");
        let i = row(&forest.nodes, ideas.to_str().unwrap());
        forest.nodes[i].expanded = true;
        let touch = |secs: u64| {
            let when = SystemTime::UNIX_EPOCH + Duration::from_secs(secs);
            std::fs::File::open(&ideas)
                .unwrap()
                .set_modified(when)
                .unwrap();
        };
        touch(1_000);
        let mut probe = None;
        refresh_probe(&mut probe, &forest);
        touch(2_000);
        (dir, roots, forest, probe)
    }

    fn still_owed(probe: &Option<ProbeSnapshot>, forest: &Forest) -> bool {
        probe_changed(
            probe.as_ref().unwrap(),
            &probe_snapshot(&probe_paths(&forest.sections, &forest.nodes)),
        )
    }

    #[test]
    fn a_no_op_rename_a_refused_folder_and_a_cancelled_or_unlisted_delete_leave_the_probe_reading()
    {
        let (_dir, roots, mut forest, mut probe) = probe_with_a_pending_change();
        let rebuild = || Ok(list_sections(&roots));
        let failing = || -> Result<Vec<SectionSpec>> { Err(anyhow::anyhow!("disk gone")) };
        let ideas = roots[0].1.join("ideas");
        let mut state = ListState::default();
        let mut saved = Vec::new();

        let i = row(&forest.nodes, ideas.to_str().unwrap());
        let before = forest.nodes[i].path.clone();
        let entered = enter_rename(&mut forest.nodes, &mut forest.sections, i, "ideas", "ideas");
        assert_eq!(entered, RenameEnter::Done(None));
        let relisted = relist_after_rename(
            &mut forest,
            &rebuild,
            &mut state,
            &mut saved,
            "",
            &mut probe,
            i,
            &before,
        );
        assert!(
            relisted.is_none(),
            "an unchanged rename lists nothing again"
        );
        assert!(still_owed(&probe, &forest), "no-op rename");

        for name in ["", "ideas"] {
            let outcome = create_folder(
                &mut forest,
                &folder_target(&roots, roots[0].0, ""),
                name,
                &rebuild,
            );
            assert!(outcome.message.is_some(), "{name:?} is refused");
            assert!(!outcome.relisted, "refused folder {name:?}");
            assert!(still_owed(&probe, &forest), "refused folder {name:?}");
        }

        let note = roots[0].1.join("c.md");
        let prompt = prompt_at(&mut forest, &note);
        let mut retired = Vec::new();
        assert!(answer_delete(
            KeyCode::Char('n'),
            &mut forest,
            &mut retired,
            &prompt,
            "",
            &rebuild
        )
        .is_none());
        assert!(still_owed(&probe, &forest), "cancelled delete");

        let outcome = answer_delete(
            KeyCode::Char('y'),
            &mut forest,
            &mut retired,
            &prompt,
            "",
            &failing,
        )
        .unwrap();
        assert!(
            !outcome.relisted,
            "a delete whose listing failed used the fallback"
        );
    }

    #[test]
    fn a_successful_rename_lists_again_keeps_the_cursor_on_it_and_refreshes_the_probe() {
        let (_dir, roots, mut forest, mut probe) = probe_with_a_pending_change();
        let rebuild = || Ok(list_sections(&roots));
        let root = &roots[0].1;
        let i = row(&forest.nodes, root.to_str().unwrap());
        forest.nodes[i].expanded = true;
        let ideas = root.join("ideas");
        let mut state = ListState::default();
        let mut saved = Vec::new();

        let i = row(&forest.nodes, ideas.to_str().unwrap());
        let entered = enter_rename(
            &mut forest.nodes,
            &mut forest.sections,
            i,
            "ideas",
            "thoughts",
        );
        assert_eq!(entered, RenameEnter::Done(None));
        let result = relist_after_rename(
            &mut forest,
            &rebuild,
            &mut state,
            &mut saved,
            "",
            &mut probe,
            i,
            &ideas,
        );

        let n = &forest.nodes;
        let thoughts = row(n, root.join("thoughts").to_str().unwrap());
        assert_eq!(
            result,
            Some(Ok(Some(thoughts))),
            "the cursor row is the renamed folder"
        );
        assert_eq!(compute_visible(n, "")[state.selected().unwrap()], thoughts);
        assert!(n[thoughts].expanded, "the folder keeps its open state");
        assert!(
            !still_owed(&probe, &forest),
            "the relist is the probe's new reading"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_symlinked_section_root_is_probed_at_its_target() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target");
        std::fs::create_dir_all(&target).unwrap();
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let paths = vec![link];
        let touch = |secs: u64| {
            let when = SystemTime::UNIX_EPOCH + Duration::from_secs(secs);
            std::fs::File::open(&target)
                .unwrap()
                .set_modified(when)
                .unwrap();
        };
        touch(1_000);
        let base = probe_snapshot(&paths);
        touch(2_000);
        assert!(
            probe_changed(&base, &probe_snapshot(&paths)),
            "a change under the link's target is seen"
        );
    }

    #[test]
    fn r_is_a_help_only_browse_key_in_the_view_group_and_free_in_every_other_mode() {
        let rows: Vec<(usize, &KeyHint)> = TREE_KEYS
            .iter()
            .enumerate()
            .filter(|(_, k)| k.key == "R")
            .collect();
        assert_eq!(rows.len(), 1);
        let (idx, hint) = rows[0];
        assert_eq!(hint.modes, BROWSE);
        assert_eq!(hint.group, Group::View);
        assert_eq!(hint.slot, Slot::HelpOnly);
        assert!(help::rows(TREE_KEYS).contains(&help::Row::Key(idx)));
        assert_eq!(
            preview_focus_key(KeyCode::Char('R'), 10),
            PreviewFocusKey::Pass
        );
        assert!(!Panes::default().handle_key(KeyCode::Char('R'), 100));
    }
}
