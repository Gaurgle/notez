//! Test helpers shared by the tests of more than one tree module.

use super::*;

pub(super) fn shown_keys(mode: Mode, on: &[Toggle], width: usize) -> Vec<&'static str> {
    let sel = footer::select(TREE_KEYS, mode, on, width);
    sel.left
        .iter()
        .chain(sel.quit.iter())
        .map(|&(i, _)| TREE_KEYS[i].key)
        .collect()
}

/// The footer keys of the pane rows, in table order.
pub(super) const PANE_KEYS: [&str; 4] = ["1/2", "tab", "</>", "="];

pub(super) fn plain(lines: &[Line]) -> Vec<String> {
    lines.iter().map(text_of).collect()
}

pub(super) fn text_of(line: &Line) -> String {
    line.spans.iter().map(|s| s.content.as_ref()).collect()
}

pub(super) fn spec(root: &str, label: &str, files: &[&str]) -> SectionSpec {
    SectionSpec {
        root: PathBuf::from(root),
        tag_root: PathBuf::from(root),
        label: label.to_string(),
        icon: "",
        is_doc: false,
        files: files.iter().map(|f| PathBuf::from(root).join(f)).collect(),
        dirs: Vec::new(),
        scope: Scope::Global,
        project: None,
        new_note_root: PathBuf::from(root),
        is_current: false,
    }
}

pub(super) fn scoped(root: &str, scope: Scope, is_doc: bool, files: &[&str]) -> SectionSpec {
    let project = (scope != Scope::Global).then(|| "proj".to_string());
    SectionSpec {
        scope,
        project,
        is_doc,
        new_note_root: if is_doc {
            PathBuf::from("/n/personal/proj")
        } else {
            PathBuf::from(root)
        },
        ..spec(root, root, files)
    }
}

/// A project view plus the global store: personal, public, docs,
/// scratch and global sections.
pub(super) fn project_sections() -> Vec<SectionSpec> {
    vec![
        scoped(
            "/n/personal/proj",
            Scope::Personal,
            false,
            &["ideas/a.md", "top.md"],
        ),
        scoped("/p/notez", Scope::Public, false, &["plans/b.md"]),
        scoped("/p/docs", Scope::Public, true, &["design/c.md"]),
        scoped("/p/.notez", Scope::Local, false, &["d.md"]),
        scoped("/n", Scope::Global, false, &["e.md"]),
    ]
}

pub(super) fn row(nodes: &[TreeNode], path: &str) -> usize {
    nodes
        .iter()
        .position(|n| n.path == Path::new(path))
        .unwrap_or_else(|| panic!("no row {path}"))
}

pub(super) fn target(dir: &str, scope: Scope, label: &str) -> NewNoteTarget {
    NewNoteTarget {
        dir: PathBuf::from(dir),
        scope,
        label: label.to_string(),
    }
}

pub(super) fn roots() -> NewNoteRoots {
    NewNoteRoots {
        global: PathBuf::from("/n"),
        projects: HashMap::from([(
            "proj".to_string(),
            vec![
                (Scope::Personal, PathBuf::from("/n/personal/proj")),
                (Scope::Public, PathBuf::from("/p/notez")),
                (Scope::Local, PathBuf::from("/p/.notez")),
            ],
        )]),
    }
}

pub(super) fn ctx_with(current: Option<&str>, new_note_roots: NewNoteRoots) -> TreeContext {
    TreeContext {
        title: String::new(),
        path_display: String::new(),
        warning: None,
        sync: SyncState::Off,
        current_project: current.map(str::to_string),
        new_note_roots,
    }
}

/// A fixed list text width for row tests. It fits the 80-column
/// [`render_row`] list; the event loop uses [`list_text_width`].
pub(super) const LIST_TEXT_WIDTH: usize = 80 - 6;

/// [`project_sections`] with the icons the real listing gives them, and
/// every row expanded.
pub(super) fn badged_forest() -> (Vec<SectionSpec>, Vec<TreeNode>) {
    let mut sections = project_sections();
    for spec in &mut sections {
        spec.icon = if spec.is_doc {
            "\u{f02d}"
        } else {
            spec.scope.icon()
        };
    }
    let (mut nodes, _) = build_forest(&sections);
    for node in &mut nodes {
        node.expanded = true;
    }
    (sections, nodes)
}

/// Row `idx` as the event loop draws it in a list `width` columns wide,
/// with the rows `compute_visible` gives for no filter and no marks.
pub(super) fn line_at_width(
    sections: &[SectionSpec],
    nodes: &[TreeNode],
    idx: usize,
    width: usize,
) -> Line<'static> {
    let visible = compute_visible(nodes, "");
    let pos = visible
        .iter()
        .position(|&i| i == idx)
        .expect("a visible row");
    list_lines(nodes, sections, &visible, &HashSet::new(), width).swap_remove(pos)
}

/// [`line_at_width`] at [`LIST_TEXT_WIDTH`].
pub(super) fn line_of(sections: &[SectionSpec], nodes: &[TreeNode], idx: usize) -> Line<'static> {
    line_at_width(sections, nodes, idx, LIST_TEXT_WIDTH)
}

/// `line` drawn as the selected row of an 80-column list, the way the
/// event loop draws it: the cells of that row.
pub(super) fn render_row(line: Line<'static>) -> Vec<(String, Option<Color>)> {
    render_row_styled(line)
        .into_iter()
        .map(|(symbol, style)| (symbol, style.fg))
        .collect()
}

/// [`render_row`] with each cell's whole style.
pub(super) fn render_row_styled(line: Line<'static>) -> Vec<(String, Style)> {
    render_cells(line, true)
}

/// `line` drawn as a row of an 80-column list, the cursor row when
/// `selected`, the way the event loop draws it: each cell's symbol and
/// whole style.
pub(super) fn render_cells(line: Line<'static>, selected: bool) -> Vec<(String, Style)> {
    use ratatui::buffer::Buffer;
    let area = Rect::new(0, 0, 80, 1);
    let mut buf = Buffer::empty(area);
    let selected = selected.then_some(0);
    let list = List::new(list_items(vec![line], selected));
    let mut state = ListState::default();
    state.select(selected);
    StatefulWidget::render(list, area, &mut buf, &mut state);
    (0..80)
        .map(|x| {
            let cell = &buf[(x, 0)];
            (cell.symbol().to_string(), cell.style())
        })
        .collect()
}

pub(super) const SCOPES: [(Scope, &str); 4] = [
    (Scope::Personal, "personal"),
    (Scope::Public, "public"),
    (Scope::Local, "local"),
    (Scope::Global, "global"),
];

/// Every `.md` file under `dir`, skipping dot entries like `.tags`: what
/// the aggregator lists for a store.
pub(super) fn md_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        if path.is_dir() {
            out.extend(md_files(&path));
        } else if path.extension().is_some_and(|e| e == "md") {
            out.push(path);
        }
    }
    out.sort();
    out
}

/// A temp tree with one store per scope, each holding `ideas/a.md`,
/// `ideas/b.md` and `c.md`.
pub(super) fn temp_tree() -> (tempfile::TempDir, Vec<(Scope, PathBuf)>) {
    let dir = tempfile::tempdir().unwrap();
    let mut roots = Vec::new();
    for (scope, name) in SCOPES {
        let root = dir.path().join(name);
        std::fs::create_dir_all(root.join("ideas")).unwrap();
        for file in ["ideas/a.md", "ideas/b.md", "c.md"] {
            std::fs::write(root.join(file), "# note\n").unwrap();
        }
        roots.push((scope, root));
    }
    (dir, roots)
}

/// The rebuild closure's work: list the stores again from disk.
pub(super) fn list_sections(roots: &[(Scope, PathBuf)]) -> Vec<SectionSpec> {
    roots
        .iter()
        .map(|(scope, root)| SectionSpec {
            root: root.clone(),
            tag_root: root.clone(),
            label: format!("{scope:?}"),
            icon: "",
            is_doc: false,
            files: md_files(root),
            dirs: Vec::new(),
            scope: *scope,
            project: (*scope != Scope::Global).then(|| "proj".to_string()),
            new_note_root: root.clone(),
            is_current: false,
        })
        .collect()
}

pub(super) fn all_files(roots: &[(Scope, PathBuf)]) -> Vec<PathBuf> {
    roots.iter().flat_map(|(_, root)| md_files(root)).collect()
}

pub(super) fn forest_of(sections: Vec<SectionSpec>) -> Forest {
    let (mut nodes, tag_roots) = build_forest(&sections);
    let initial: Vec<HashMap<String, u8>> =
        tag_roots.iter().map(|r| note_tags::load_tags(r)).collect();
    apply_tags(&mut nodes, &tag_roots, &initial);
    Forest {
        sections,
        nodes,
        tag_roots,
        initial,
    }
}

pub(super) fn path_str(path: &Path) -> &str {
    path.to_str().unwrap()
}

/// The prompt `d` opens with the cursor on `path`, whose folders are
/// expanded as they are when the cursor can reach it.
pub(super) fn prompt_at(forest: &mut Forest, path: &Path) -> DeletePrompt {
    let i = row(&forest.nodes, path_str(path));
    let mut up = forest.nodes[i].parent_idx;
    while let Some(p) = up {
        forest.nodes[p].expanded = true;
        up = forest.nodes[p].parent_idx;
    }
    delete_request(&forest.nodes, &forest.sections, Some(i), Some("proj"))
        .expect("a note row")
        .expect("a row under the cursor")
}

/// The prompt `N` opens at `path` in [`project_sections`].
pub(super) fn folder_prompt_at(
    path: &str,
    current: Option<&str>,
) -> Result<NewNotePrompt, &'static str> {
    let sections = project_sections();
    let (nodes, _) = build_forest(&sections);
    open_new_folder_prompt(
        &nodes,
        &sections,
        Some(row(&nodes, path)),
        &ctx_with(current, roots()),
    )
}

/// A temp tree ([`temp_tree`]) listed with its directories, as the real
/// listing does.
pub(super) fn list_sections_with_dirs(roots: &[(Scope, PathBuf)]) -> Vec<SectionSpec> {
    let mut sections = list_sections(roots);
    for spec in &mut sections {
        spec.dirs = all_dirs(&spec.root);
    }
    sections
}

pub(super) fn all_dirs(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        if entry.file_type().unwrap().is_dir()
            && !entry.file_name().to_string_lossy().starts_with('.')
        {
            out.push(entry.path());
            out.extend(all_dirs(&entry.path()));
        }
    }
    out
}

pub(super) fn folder_target(roots: &[(Scope, PathBuf)], scope: Scope, sub: &str) -> NewNoteTarget {
    let root = &roots.iter().find(|(s, _)| *s == scope).unwrap().1;
    NewNoteTarget {
        dir: root.join(sub),
        scope,
        label: format!("{scope:?}"),
    }
}

/// A vault laid out like the real one: the notez root `n` is the tag
/// root of the personal section `n/personal/proj`, so its `.tags` keys
/// read `personal/proj/...`. The section holds `ideas/a.md`,
/// `ideas/deep/deeper/n.md` (two levels under `ideas`), the empty
/// `ideas/empty/`, `plans/p.md` and `top.md`; the global section holds
/// `g.md`. Returns the temp dir, the notez root and the section root.
pub(super) fn vault() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let notez = dir.path().join("n");
    let root = notez.join("personal/proj");
    for sub in ["ideas/deep/deeper", "ideas/empty", "plans"] {
        std::fs::create_dir_all(root.join(sub)).unwrap();
    }
    for file in [
        "ideas/a.md",
        "ideas/deep/deeper/n.md",
        "plans/p.md",
        "top.md",
    ] {
        std::fs::write(root.join(file), "# note\n").unwrap();
    }
    std::fs::write(notez.join("g.md"), "# g\n").unwrap();
    std::fs::write(
        notez.join(".tags"),
        "personal/proj/ideas/a.md:1\n\
         personal/proj/ideas/deep/deeper/n.md:2\n\
         personal/proj/ideas/gone-before.md:1\n\
         personal/proj/plans/p.md:4\n\
         personal/proj/ideas-other.md:1\n\
         g.md:1\n",
    )
    .unwrap();
    (dir, notez, root)
}

/// The rebuild closure's work for [`vault`]: the personal section with
/// its folders, then the global section (which leaves `personal/` out).
pub(super) fn vault_sections(notez: &Path) -> Vec<SectionSpec> {
    let root = notez.join("personal/proj");
    let personal = SectionSpec {
        root: root.clone(),
        tag_root: notez.to_path_buf(),
        label: "proj (personal)".to_string(),
        icon: "",
        is_doc: false,
        files: md_files(&root),
        dirs: all_dirs(&root),
        scope: Scope::Personal,
        project: Some("proj".to_string()),
        new_note_root: root.clone(),
        is_current: true,
    };
    let global = SectionSpec {
        root: notez.to_path_buf(),
        tag_root: notez.to_path_buf(),
        label: "NOTEZ".to_string(),
        icon: "",
        is_doc: false,
        files: vec![notez.join("g.md")],
        dirs: Vec::new(),
        scope: Scope::Global,
        project: None,
        new_note_root: notez.to_path_buf(),
        is_current: false,
    };
    vec![personal, global]
}

/// Every entry under `dir`, files and folders, relative to it.
pub(super) fn disk_entries(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(at) = stack.pop() {
        for entry in std::fs::read_dir(&at).unwrap().flatten() {
            let path = entry.path();
            out.push(
                path.strip_prefix(dir)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
            );
            if entry.file_type().unwrap().is_dir() {
                stack.push(path);
            }
        }
    }
    out.sort();
    out
}

/// `rename_request` then `rename_folder` with the cursor on `path`, as
/// `r`, a typed name and `Enter` do.
pub(super) fn rename_at(
    forest: &mut Forest,
    path: &Path,
    name: &str,
) -> std::result::Result<(), String> {
    let i = row(&forest.nodes, path_str(path));
    let shown = rename_request(&forest.nodes, &forest.sections, Some(i))
        .expect("a folder row opens the prompt")
        .expect("a row under the cursor");
    assert_eq!(
        shown, forest.nodes[i].name,
        "the prompt shows the current name"
    );
    rename_folder(&mut forest.nodes, &mut forest.sections, i, name)
}

/// A notez root `n` and the repository `p` of the project `proj`, with
/// one store per scope: personal `n/personal/proj` and global `n` (both
/// keyed in `n/.tags`), public `p/notez` and local `p/.notez` (each with
/// its own `.tags`), plus `p/docs/d.md`. Every store holds `ideas/a.md`
/// (tagged), `ideas/b.md` (untagged), `c.md` (tagged) and the empty
/// folder `plans/`. Returns the temp dir, the notez root and the stores.
pub(super) fn move_fixture() -> (tempfile::TempDir, PathBuf, Vec<(Scope, PathBuf)>) {
    let dir = tempfile::tempdir().unwrap();
    let notez = dir.path().join("n");
    let repo = dir.path().join("p");
    let roots = vec![
        (Scope::Personal, notez.join("personal/proj")),
        (Scope::Public, repo.join("notez")),
        (Scope::Local, repo.join(".notez")),
        (Scope::Global, notez.clone()),
    ];
    for (_, root) in &roots {
        std::fs::create_dir_all(root.join("ideas")).unwrap();
        std::fs::create_dir_all(root.join("plans")).unwrap();
        for file in ["ideas/a.md", "ideas/b.md", "c.md"] {
            std::fs::write(root.join(file), "# note\n").unwrap();
        }
    }
    std::fs::create_dir_all(repo.join("docs")).unwrap();
    std::fs::write(repo.join("docs/d.md"), "# doc\n").unwrap();
    std::fs::write(
        notez.join(".tags"),
        "personal/proj/ideas/a.md:1\npersonal/proj/c.md:2\nideas/a.md:4\nc.md:8\nother.md:16\n",
    )
    .unwrap();
    for store in ["notez", ".notez"] {
        std::fs::write(repo.join(store).join(".tags"), "ideas/a.md:3\nc.md:2\n").unwrap();
    }
    (dir, notez, roots)
}

/// The tag root a store of `scope` keys its notes from, as
/// `commands::tree` sets it.
pub(super) fn move_tag_root(roots: &[(Scope, PathBuf)], scope: Scope) -> PathBuf {
    let store = |s: Scope| roots.iter().find(|(x, _)| *x == s).unwrap().1.clone();
    match scope {
        Scope::Personal | Scope::Global => store(Scope::Global),
        other => store(other),
    }
}

/// The rebuild closure's work for [`move_fixture`]: personal, public,
/// docs, local and global sections, listed from the disk.
pub(super) fn move_sections(roots: &[(Scope, PathBuf)]) -> Vec<SectionSpec> {
    let notez = move_tag_root(roots, Scope::Global);
    let personal_dir = notez.join("personal");
    let docs = move_tag_root(roots, Scope::Public)
        .parent()
        .unwrap()
        .join("docs");
    let section = |scope: Scope, root: &Path, is_doc: bool| {
        let outside = |p: &PathBuf| scope == Scope::Global && p.starts_with(&personal_dir);
        SectionSpec {
            root: root.to_path_buf(),
            tag_root: if is_doc {
                root.to_path_buf()
            } else {
                move_tag_root(roots, scope)
            },
            label: format!("{scope:?}"),
            icon: "",
            is_doc,
            files: md_files(root).into_iter().filter(|p| !outside(p)).collect(),
            dirs: all_dirs(root).into_iter().filter(|p| !outside(p)).collect(),
            scope,
            project: (scope != Scope::Global).then(|| "proj".to_string()),
            new_note_root: if is_doc {
                roots[0].1.clone()
            } else {
                root.to_path_buf()
            },
            is_current: scope != Scope::Global,
        }
    };
    let mut out: Vec<SectionSpec> = roots
        .iter()
        .filter(|(s, _)| *s != Scope::Global)
        .map(|(s, root)| section(*s, root, false))
        .collect();
    out.insert(2, section(Scope::Public, &docs, true));
    out.push(section(Scope::Global, &notez, false));
    out
}

pub(super) fn move_ctx(roots: &[(Scope, PathBuf)]) -> TreeContext {
    let stores = roots
        .iter()
        .filter(|(s, _)| *s != Scope::Global)
        .cloned()
        .collect();
    let new_note_roots = NewNoteRoots {
        global: move_tag_root(roots, Scope::Global),
        projects: HashMap::from([("proj".to_string(), stores)]),
    };
    ctx_with(Some("proj"), new_note_roots)
}

pub(super) fn store_of(roots: &[(Scope, PathBuf)], scope: Scope) -> PathBuf {
    roots.iter().find(|(s, _)| *s == scope).unwrap().1.clone()
}

/// `m` on `src`, `Tab` until the prompt targets `scope`, the buffer set
/// to `folder`, then `Enter`: the resolved move or the footer message.
pub(super) fn move_plan(
    forest: &Forest,
    ctx: &TreeContext,
    src: &Path,
    scope: Scope,
    folder: &str,
) -> std::result::Result<MovePlan, String> {
    let i = row(&forest.nodes, path_str(src));
    let mut prompt =
        move_request(&forest.nodes, &forest.sections, Some(i), ctx).expect("a note row");
    for _ in 0..4 {
        if prompt.scope.target.scope == scope {
            break;
        }
        let p = &prompt.scope;
        prompt.scope.target = next_scope_target(
            &p.target,
            &p.origin,
            p.project.as_deref(),
            &ctx.new_note_roots,
            ctx.current_project.as_deref(),
        );
    }
    assert_eq!(prompt.scope.target.scope, scope, "Tab reaches {scope:?}");
    prompt.scope.buffer = folder.to_string();
    resolve_move(&prompt, &ctx.new_note_roots)
}

/// Every final tag map after exit, by tag root: the changed ones as the
/// exit write reports them, the rest as loaded from the disk.
pub(super) fn final_maps(
    forest: &Forest,
    retired: &[(PathBuf, String)],
    carried: &[CarriedTags],
    roots: &[(Scope, PathBuf)],
) -> HashMap<PathBuf, HashMap<String, u8>> {
    let changed: HashMap<PathBuf, HashMap<String, u8>> = exit_tag_maps(forest, retired, carried)
        .into_iter()
        .collect();
    [Scope::Global, Scope::Public, Scope::Local]
        .into_iter()
        .map(|s| move_tag_root(roots, s))
        .map(|root| {
            let map = changed
                .get(&root)
                .cloned()
                .unwrap_or_else(|| note_tags::load_tags(&root));
            (root, map)
        })
        .collect()
}

/// `Tab` in `prompt` until it targets `scope`.
pub(super) fn tab_to(prompt: &mut MovePrompt, scope: Scope, ctx: &TreeContext) {
    for _ in 0..4 {
        if prompt.scope.target.scope == scope {
            return;
        }
        let p = &prompt.scope;
        prompt.scope.target = next_scope_target(
            &p.target,
            &p.origin,
            p.project.as_deref(),
            &ctx.new_note_roots,
            ctx.current_project.as_deref(),
        );
    }
    assert_eq!(prompt.scope.target.scope, scope, "Tab reaches {scope:?}");
}

pub(super) fn marks_of(paths: &[&str]) -> HashSet<PathBuf> {
    paths.iter().map(PathBuf::from).collect()
}

/// Mark `paths` and press `m` (`fixed` false) or `S` (`fixed` true).
pub(super) fn bulk_move_at(
    forest: &Forest,
    ctx: &TreeContext,
    paths: &[PathBuf],
    fixed: bool,
) -> std::result::Result<(MovePrompt, Vec<MovePrompt>), String> {
    let marks: HashSet<PathBuf> = paths.iter().cloned().collect();
    bulk_move_request(&forest.nodes, &forest.sections, &marks, ctx, fixed)
}

/// `m` on the marked `paths`, `Tab` to `scope`, `folder` typed, `Enter`.
pub(super) fn bulk_move_plan(
    forest: &Forest,
    ctx: &TreeContext,
    paths: &[PathBuf],
    scope: Scope,
    folder: &str,
) -> std::result::Result<Option<BulkMovePlan>, String> {
    let (mut prompt, items) = bulk_move_at(forest, ctx, paths, false)?;
    tab_to(&mut prompt, scope, ctx);
    prompt.scope.buffer = folder.to_string();
    resolve_bulk_move(&prompt, &items, &ctx.new_note_roots)
}

/// `r` with the cursor on `path`, the buffer set to `typed`, then
/// `Enter`, as the event loop runs it.
pub(super) fn rename_enter_at(forest: &mut Forest, path: &Path, typed: &str) -> RenameEnter {
    let i = row(&forest.nodes, path_str(path));
    let shown = rename_request(&forest.nodes, &forest.sections, Some(i))
        .expect("the row opens the prompt")
        .expect("a row under the cursor");
    enter_rename(&mut forest.nodes, &mut forest.sections, i, &shown, typed)
}

pub(super) fn at(secs: u64) -> Option<SystemTime> {
    Some(SystemTime::UNIX_EPOCH + Duration::from_secs(secs))
}
