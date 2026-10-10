//! Drawing: rows and the tag field, the status and footer lines, pane
//! titles, and the key table shared by the footer and the help overlay.

use super::*;

/// The filter strip's contiguous 5-dot geometry: dot 0 sits at
/// `area_x + 1`, after a one-column lead, so each strip dot stands over
/// the same tag's slot in the rows' field (see [`tag_field`]).
pub(super) fn mouse_x_to_dot(mouse_col: u16, area_x: u16) -> Option<u8> {
    let dot_start = area_x.saturating_add(1);
    let dot_end = dot_start + 4;
    if mouse_col >= dot_start && mouse_col <= dot_end {
        Some((mouse_col - dot_start) as u8)
    } else {
        None
    }
}

/// The columns of every list row's tag field, one slot per tag in
/// [`FLAG_DEFS`] order, tagged or not, so the tree after it never moves.
const TAG_FIELD_WIDTH: usize = FLAG_DEFS.len();

/// Slot `slot` of a row's tag field: a `●` in the tag's colour when it is
/// set in `flags`, a dim `·` otherwise, as todoz draws its dots. Neither
/// sets a background, so the cursor row's selection shows through.
fn tag_cell(flags: u8, slot: usize) -> Span<'static> {
    if flags & FLAG_DEFS[slot].bit != 0 {
        Span::styled("●", Style::default().fg(theme::FLAG_COLORS[slot]))
    } else {
        Span::styled("·", Style::default().fg(theme::OVERLAY))
    }
}

/// A list row's tag field: the [`TAG_FIELD_WIDTH`] slots of [`tag_cell`],
/// then one space before the tree. A note row always draws its slots. A
/// folder or section row (`is_dir`) shows the tags its notes carry (see
/// [`derive_dir_flags`]) and stays blank when they carry none. Only a
/// note's slots take clicks (see [`field_click_tag`]).
fn tag_field(flags: u8, is_dir: bool) -> Vec<Span<'static>> {
    let is_blank = is_dir && flags == 0;
    let mut spans: Vec<Span<'static>> = (0..TAG_FIELD_WIDTH)
        .map(|slot| {
            if is_blank {
                Span::raw(" ")
            } else {
                tag_cell(flags, slot)
            }
        })
        .collect();
    spans.push(Span::raw(" "));
    spans
}

/// The tag field slot under `mouse_col` on a list row whose text starts at
/// `area_x`: slot n is the n-th column after the one-column gutter and
/// stands for tag n of [`FLAG_DEFS`]. `None` off the field.
pub(super) fn mouse_x_to_field_slot(mouse_col: u16, area_x: u16) -> Option<usize> {
    let first = area_x.checked_add(1)?;
    let slot = usize::from(mouse_col.checked_sub(first)?);
    (slot < TAG_FIELD_WIDTH).then_some(slot)
}

/// The tag, as an index into [`FLAG_DEFS`], that a click at `mouse_col`
/// toggles: the slot under it (see [`mouse_x_to_field_slot`]) on a note
/// row (`is_dir` false), and only while no prompt, confirm, filter, tag
/// mode or `:` line is open (`input_open`). `None` makes the click a plain
/// row click.
pub(super) fn field_click_tag(
    mouse_col: u16,
    area_x: u16,
    is_dir: bool,
    input_open: bool,
) -> Option<usize> {
    if is_dir || input_open {
        return None;
    }
    mouse_x_to_field_slot(mouse_col, area_x)
}

/// The list's rows as items, the `selected` one (clamped to the last row,
/// as the list clamps it) in [`theme::selected_row`]. The style goes on
/// the item, under the row's spans, so the tag dots keep their colours on
/// the cursor row; a list highlight style would paint over them.
pub(super) fn list_items(
    lines: Vec<Line<'static>>,
    selected: Option<usize>,
) -> Vec<ListItem<'static>> {
    let selected = selected.map(|s| s.min(lines.len().saturating_sub(1)));
    lines
        .into_iter()
        .enumerate()
        .map(|(i, line)| {
            let item = ListItem::new(line);
            if Some(i) == selected {
                item.style(theme::selected_row())
            } else {
                item
            }
        })
        .collect()
}

/// For each row, whether a later row of `visible` (rows in tree order)
/// shares its parent: the row draws `├─` and its descendants a bar at its
/// level. Rows outside `visible`, such as rows the filter hides, neither
/// get one nor count as a later sibling. Indexed like `nodes`.
fn later_siblings(nodes: &[TreeNode], visible: &[usize]) -> Vec<bool> {
    let mut later = vec![false; nodes.len()];
    let mut seen: HashSet<Option<usize>> = HashSet::new();
    for &i in visible.iter().rev() {
        later[i] = !seen.insert(nodes[i].parent_idx);
    }
    later
}

/// The tree drawing before row `idx`'s badge, from [`theme::TREE_GLYPHS`]. A
/// section row shows its expand mark. A nested row shows, for each ancestor
/// below the section, a bar if that ancestor has a later sibling and a
/// blank otherwise, then its own branch (`├─`, or `└─` when it is the last
/// child), then a folder's expand mark or a file's blank. `later` comes
/// from [`later_siblings`].
fn branch_prefix(nodes: &[TreeNode], idx: usize, later: &[bool]) -> String {
    let g = &theme::TREE_GLYPHS;
    let node = &nodes[idx];
    if node.depth == 0 {
        return if !node.is_dir {
            "  ".to_string()
        } else if node.expanded {
            g.section_open.to_string()
        } else {
            g.section_closed.to_string()
        };
    }
    let mut levels = Vec::new();
    let mut up = node.parent_idx;
    while let Some(p) = up.filter(|&p| nodes[p].depth > 0) {
        levels.push(if later[p] {
            g.ancestor_bar
        } else {
            g.ancestor_blank
        });
        up = nodes[p].parent_idx;
    }
    levels.reverse();
    let mut prefix = levels.concat();
    prefix.push_str(if later[idx] { g.branch } else { g.last_branch });
    prefix.push_str(match (node.is_dir, node.expanded) {
        (true, true) => g.folder_open,
        (true, false) => g.folder_closed,
        (false, _) => g.file,
    });
    prefix
}

/// Every row of `visible` drawn for a list `inner_width` columns wide:
/// branch lines measured over these rows, marked rows drawn as marked.
pub(super) fn list_lines(
    nodes: &[TreeNode],
    sections: &[SectionSpec],
    visible: &[usize],
    marks: &HashSet<PathBuf>,
    inner_width: usize,
) -> Vec<Line<'static>> {
    let later = later_siblings(nodes, visible);
    visible
        .iter()
        .map(|&idx| {
            let node = &nodes[idx];
            let branch = branch_prefix(nodes, idx, &later);
            let line = row_line(node, sections.get(node.section), &branch, inner_width);
            if is_marked(marks, node) {
                mark_row(line)
            } else {
                line
            }
        })
        .collect()
}

/// The 5 fixed tag-dot slots with leading space, as the preview's title
/// shows them.
pub(super) fn flags_slots(flags: u8) -> Vec<Span<'static>> {
    let mut spans: Vec<Span<'static>> = vec![Span::raw(" ")];
    for (i, def) in FLAG_DEFS.iter().enumerate() {
        if flags & def.bit != 0 {
            spans.push(Span::styled(
                "●",
                Style::default().fg(theme::FLAG_COLORS[i]),
            ));
        } else {
            spans.push(Span::styled(
                "·",
                Style::default().fg(Color::Rgb(50, 50, 65)),
            ));
        }
    }
    spans.push(Span::raw(" "));
    spans
}

/// The colour of a section's badge and scope icon. A docs
/// section is published with the repository, so it takes the public colour.
fn section_color(spec: &SectionSpec) -> Color {
    theme::scope_color(if spec.is_doc {
        Scope::Public
    } else {
        spec.scope
    })
}

/// The two-column badge directly before a nested row's name, after its
/// indentation and branch glyph: the section's icon in its colour and a
/// space, like a section header's `icon label`, on every file and folder
/// row; blanks on a section header (which shows the icon next to its
/// label). A todo row (the board's store, a row under it, or a file named
/// exactly `TODO.md`) shows [`theme::ICON_TODO`] instead, in the same
/// colour and width. Render only: the filter, preview, mouse hit testing
/// and tag keys never see it, and the tag field keeps its columns.
fn row_badge(node: &TreeNode, spec: Option<&SectionSpec>) -> Span<'static> {
    match spec {
        Some(spec) if node.depth > 0 && !spec.icon.is_empty() => {
            let is_todo_file =
                !node.is_dir && node.path.file_name().is_some_and(|name| name == "TODO.md");
            let icon = if is_todo_file || in_section_todo_store(node, spec) {
                theme::ICON_TODO
            } else {
                spec.icon
            };
            Span::styled(format!("{icon} "), Style::default().fg(section_color(spec)))
        }
        _ => Span::raw("  "),
    }
}

/// The columns a list row may fill in a list pane `pane_width` wide: the
/// pane less its two borders and its one-column padding on each side. The
/// list draws no highlight symbol, so a row starts at the padding.
pub(super) fn list_text_width(pane_width: u16) -> usize {
    pane_width.saturating_sub(4) as usize
}

/// The list pane's frame without its title and border colour: rounded
/// borders and the padding that `list_chunks` measures inside.
pub(super) fn list_block() -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .padding(Padding::new(1, 1, 1, 0))
}

/// The list pane's inside, top to bottom: the filter strip, the separator
/// and the rows. The mouse hit tests use the same rects the draw used.
pub(super) fn list_chunks(list_area: Rect) -> std::rc::Rc<[Rect]> {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
        ])
        .split(list_block().inner(list_area))
}

/// One list row: a one-column gutter (blank, or the mark of a marked row),
/// the fixed tag field and its space (see [`tag_field`]), the `branch`
/// drawing from [`branch_prefix`], the scope badge on a nested row, the
/// name, and on a directory row a dotted leader to its file count, which
/// ends at `inner_width` in display columns. A section header also shows
/// its scope icon in the scope colour before the label; the icon and colour
/// name the scope, so no scope word follows. `spec` is the row's section
/// and `inner_width` the list pane's text width.
fn row_line(
    node: &TreeNode,
    spec: Option<&SectionSpec>,
    branch: &str,
    inner_width: usize,
) -> Line<'static> {
    let header_color = spec.map_or(theme::OVERLAY, section_color);
    let mut spans = vec![Span::raw(" ")];
    spans.extend(tag_field(node.flags, node.is_dir));
    spans.push(Span::styled(
        branch.to_string(),
        Style::default().fg(theme::SURFACE),
    ));
    if node.depth > 0 {
        spans.push(row_badge(node, spec));
    }
    if !node.scope_icon.is_empty() {
        spans.push(Span::styled(
            format!("{} ", node.scope_icon),
            Style::default().fg(header_color),
        ));
    }
    if node.is_dir {
        spans.push(Span::styled(
            node.name.clone(),
            Style::default().fg(theme::SAPPHIRE),
        ));
        if node.child_count > 0 {
            let count_str = format!("{}", node.child_count);
            let prefix_len: usize = spans.iter().map(Span::width).sum();
            let avail = inner_width.saturating_sub(prefix_len + count_str.len() + 2);
            if avail > 3 {
                spans.push(Span::styled(
                    format!(" {} ", "·".repeat(avail)),
                    Style::default().fg(theme::SURFACE),
                ));
            } else {
                spans.push(Span::raw(" "));
            }
            spans.push(Span::styled(count_str, Style::default().fg(theme::OVERLAY)));
        }
    } else {
        spans.push(Span::styled(
            node.name.clone(),
            Style::default().fg(theme::TEXT),
        ));
    }
    Line::from(spans)
}

/// What the status bar shows, highest priority first.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum StatusSlot<'a> {
    Rename(&'a str),
    Message(&'a str),
    VimCommand,
    Tags,
    Warning(&'a str),
    Hints,
}

/// Pick the status bar's content. The session warning ranks below every
/// transient use of the line, so it comes back once they end, and it is kept
/// apart from the one-off `message` that each key press clears.
pub(super) fn status_slot<'a>(
    rename: Option<&'a str>,
    message: Option<&'a str>,
    vim_active: bool,
    flag_mode: bool,
    warning: Option<&'a str>,
) -> StatusSlot<'a> {
    // A message outranks the rename prompt: a refused name keeps the prompt
    // open, and its message shows until the next key brings the prompt back.
    if let Some(message) = message {
        StatusSlot::Message(message)
    } else if let Some(buffer) = rename {
        StatusSlot::Rename(buffer)
    } else if vim_active {
        StatusSlot::VimCommand
    } else if flag_mode {
        StatusSlot::Tags
    } else if let Some(warning) = warning {
        StatusSlot::Warning(warning)
    } else {
        StatusSlot::Hints
    }
}

/// The rename prompt that leads the footer in rename mode.
pub(super) fn rename_lead(buffer: &str) -> Vec<Span<'static>> {
    vec![
        Span::styled(" rename: ", Style::default().fg(theme::MAUVE)),
        Span::styled(buffer.to_string(), Style::default().fg(theme::TEXT)),
        Span::styled("_", Style::default().fg(theme::OVERLAY)),
    ]
}

/// The `:` command buffer that leads the footer in command mode.
pub(super) fn command_lead(buffer: &str) -> Vec<Span<'static>> {
    vec![Span::styled(buffer.to_string(), theme::command_line())]
}

/// The tag legend that leads the footer in tag mode; the tags set in `flags`
/// are coloured.
pub(super) fn tag_legend(flags: u8) -> Vec<Span<'static>> {
    let mut spans = vec![Span::styled(" tags: ", Style::default().fg(theme::MAUVE))];
    for (idx, def) in FLAG_DEFS.iter().enumerate() {
        let active = flags & def.bit != 0;
        let color = theme::FLAG_COLORS[idx];
        spans.push(Span::styled(
            format!("{}", idx + 1),
            Style::default().fg(color),
        ));
        spans.push(Span::styled(":", Style::default().fg(theme::OVERLAY)));
        spans.push(Span::styled(
            format!("{} ", def.label),
            Style::default().fg(if active { color } else { theme::OVERLAY }),
        ));
        spans.push(Span::raw(" "));
    }
    spans
}

/// `lead` first, then the `TREE_KEYS` hints for `mode` that fit after it.
pub(super) fn lead_with_hints(
    lead: Vec<Span<'static>>,
    mode: Mode,
    on: &[Toggle],
    width: usize,
) -> Line<'static> {
    footer::status_line(TREE_KEYS, lead, true, mode, on, Vec::new(), width)
}

/// The key of the preview toggle in `TREE_KEYS`.
const PREVIEW_TOGGLE_KEY: &str = "p";

/// `TREE_KEYS` with the preview toggle's footer hint filled in. `toggle` is
/// set while a file with a language is selected: the hint then shows the
/// view `p` switches to and drops first when space runs out. Otherwise the
/// toggle stays in the help overlay only. Same rows in the same order as
/// `TREE_KEYS`, so the footer and the help overlay still agree.
pub(super) fn tree_keys(toggle: Option<PreviewToggle>) -> Vec<KeyHint> {
    TREE_KEYS
        .iter()
        .map(|hint| match toggle {
            Some(toggle) if hint.key == PREVIEW_TOGGLE_KEY => KeyHint {
                desc: toggle.desc(),
                slot: Slot::Priority(12),
                ..*hint
            },
            _ => *hint,
        })
        .collect()
}

/// The key of the `1`/`2` focus row in `TREE_KEYS`.
const PANE_FOCUS_KEY: &str = "1/2";

/// The key of the browse-mode `tab` row in `TREE_KEYS`.
const PANE_CYCLE_KEY: &str = "tab";

/// The footer rows for the focused pane. With the list focused, `keys`
/// unchanged. With the preview focused, only the preview set: `j/k scroll`,
/// `PgDn/PgUp page`, `2 fold`, `1/tab list`, the `p` toggle when `keys`
/// shows it (a markdown note), `?` and `q`; every other row is help only.
/// Same rows in the same order as `keys`, so the footer and the help
/// overlay still agree and help lists each key once.
pub(super) fn pane_keys(keys: Vec<KeyHint>, focus: Pane) -> Vec<KeyHint> {
    if focus == Pane::List {
        return keys;
    }
    keys.into_iter()
        .map(|hint| {
            // Prompt rows keep their slots; only the browse footer changes.
            if !hint.applies_in(Mode::Normal) {
                return hint;
            }
            let preview_hint = |key, desc, priority| KeyHint {
                key,
                desc,
                slot: Slot::Priority(priority),
                ..hint
            };
            match (hint.key, hint.slot) {
                ("j/k", _) => preview_hint("j/k", "scroll", 1),
                ("PgDn/PgUp", _) => preview_hint("PgDn/PgUp", "page", 2),
                (PANE_FOCUS_KEY, _) => preview_hint("2", "fold", 3),
                (PANE_CYCLE_KEY, _) => preview_hint("1/tab", "list", 4),
                (PREVIEW_TOGGLE_KEY, Slot::Priority(_)) => KeyHint {
                    slot: Slot::Priority(5),
                    ..hint
                },
                (_, Slot::Pinned | Slot::Quit) => hint,
                _ => KeyHint {
                    slot: Slot::HelpOnly,
                    ..hint
                },
            }
        })
        .collect()
}

/// The title of a pane: its number (the key that focuses it) in the pane
/// number style, then `rest`.
pub(super) fn pane_title(pane: Pane, focus: Pane, rest: Vec<Span<'static>>) -> Line<'static> {
    let mut spans = vec![Span::styled(
        format!(" {} ", pane.number()),
        theme::pane_number(pane == focus),
    )];
    spans.extend(rest);
    Line::from(spans)
}

/// The border style of `pane`: highlighted while it has focus.
pub(super) fn pane_border(pane: Pane, focus: Pane) -> Style {
    if pane == focus {
        theme::border_focused()
    } else {
        theme::border()
    }
}

/// The browsing footer: the mark count when rows are marked, then the
/// `table` hints for `mode` that fit after it. The selected file's suffix
/// and path are on the preview's bottom border instead.
pub(super) fn browse_footer(
    table: &[KeyHint],
    marks: usize,
    mode: Mode,
    on: &[Toggle],
    width: usize,
) -> Line<'static> {
    let mut lead = Vec::new();
    if marks > 0 {
        lead.extend(marked_lead(marks));
    }
    footer::status_line(table, lead, true, mode, on, Vec::new(), width)
}

// --- Keys: one table for the footer and the help overlay ---

pub(super) const BROWSE: &[Mode] = &[Mode::Normal, Mode::Focus];
const BROWSE_AND_TAG: &[Mode] = &[Mode::Normal, Mode::Focus, Mode::Tag];
const FILTERING: &[Mode] = &[Mode::Filter];
const TAGGING: &[Mode] = &[Mode::Tag];
const RENAMING: &[Mode] = &[Mode::Rename];
const NEW_NOTE: &[Mode] = &[Mode::NewItem];
const CONFIRMING: &[Mode] = &[Mode::ConfirmDelete];
const COMMAND: &[Mode] = &[Mode::VimCommand];
const MOVING: &[Mode] = &[Mode::Move];
const SETTING_SCOPE: &[Mode] = &[Mode::SetScope];
const CONFIRMING_MOVE: &[Mode] = &[Mode::ConfirmMove];

const fn key(
    key: &'static str,
    desc: &'static str,
    help: &'static str,
    color: Color,
    group: Group,
    modes: &'static [Mode],
    slot: Slot,
    toggle: Option<Toggle>,
) -> KeyHint {
    KeyHint {
        key,
        desc,
        help,
        color,
        group,
        modes,
        slot,
        toggle,
    }
}

/// Every key, mouse action and command the tree browser handles, in footer
/// order. Kept in step with `event_loop`.
pub(super) const TREE_KEYS: &[KeyHint] = &[
    key("j/k", "move", "move down / up (also Down / Up); with the preview focused, scroll it", theme::TEXT, Group::Navigate, BROWSE_AND_TAG, Slot::HelpOnly, None),
    key("l", "expand", "expand directory (also Right)", theme::MAUVE, Group::Navigate, BROWSE_AND_TAG, Slot::HelpOnly, None),
    key("h", "collapse", "collapse directory / go to parent (also Left)", theme::MAUVE, Group::Navigate, BROWSE_AND_TAG, Slot::HelpOnly, None),
    key("wheel", "scroll", "mouse wheel scrolls the pane under the pointer: the preview, or the list's cursor", theme::TEXT, Group::Navigate, BROWSE, Slot::HelpOnly, None),
    key("click", "select", "click a pane to focus it; a row to select it and toggle a directory", theme::TEXT, Group::Navigate, BROWSE, Slot::HelpOnly, None),
    key("drag", "split", "drag the grip on the border between the panes to resize them", theme::LAVENDER, Group::View, BROWSE, Slot::HelpOnly, None),
    key("o", "open", "open file / toggle directory (also Enter)", theme::GREEN, Group::Edit, BROWSE, Slot::Priority(1), None),
    key("t", "tags", "tag mode on / off", theme::PEACH, Group::Edit, BROWSE_AND_TAG, Slot::Priority(2), Some(Toggle::Tag)),
    key("r", "rename", "rename note or folder", theme::MAUVE, Group::Edit, BROWSE, Slot::Priority(6), None),
    key("1-5", "toggle", "tag mode: toggle tag 1 to 5 on the note", theme::PEACH, Group::Edit, TAGGING, Slot::Priority(1), None),
    key("esc", "close", "tag mode: close", theme::PEACH, Group::Edit, TAGGING, Slot::Priority(3), None),
    key("click tags", "tag", "click a note's tag dot to toggle that tag",theme::PEACH, Group::Edit, BROWSE, Slot::HelpOnly, None),
    key("enter", "confirm", "rename: confirm", theme::GREEN, Group::Edit, RENAMING, Slot::Priority(1), None),
    key("esc", "cancel", "rename: cancel", theme::PEACH, Group::Edit, RENAMING, Slot::Priority(2), None),
    key("bksp", "delete", "rename: delete the last char", theme::TEXT, Group::Edit, RENAMING, Slot::Priority(3), None),
    key("n", "new", "new note in the folder under the cursor (the prompt names the scope)", theme::GREEN, Group::Edit, BROWSE, Slot::Priority(3), None),
    key("N", "folder", "new folder in the folder under the cursor (the prompt names the scope)", theme::GREEN, Group::Edit, BROWSE, Slot::Priority(8), None),
    key("m", "move", "move note or folder", theme::MAUVE, Group::Edit, BROWSE, Slot::Priority(9), None),
    key("S", "scope", "set scope", theme::MAUVE, Group::Edit, BROWSE, Slot::Priority(10), None),
    key("enter", "create", "new note or folder: create it (a note opens in the editor)", theme::GREEN, Group::Edit, NEW_NOTE, Slot::Priority(1), None),
    key("esc", "cancel", "new note or folder: cancel, nothing is created", theme::PEACH, Group::Edit, NEW_NOTE, Slot::Priority(2), None),
    key("tab", "scope", "new note or folder: next scope (personal, public, local, global), at its root", theme::SAPPHIRE, Group::Edit, NEW_NOTE, Slot::Priority(3), None),
    key("bksp", "delete", "new note or folder: delete the last char", theme::TEXT, Group::Edit, NEW_NOTE, Slot::Priority(4), None),
    key("space", "mark", "mark", theme::LAVENDER, Group::Edit, BROWSE, Slot::Priority(8), None),
    key("d", "delete", "delete note or folder under the cursor (asks first; no undo)", theme::RED, Group::Edit, BROWSE, Slot::Priority(7), None),
    key("y", "confirm", "delete: yes, delete it", theme::RED, Group::Edit, CONFIRMING, Slot::Priority(1), None),
    key("n/esc", "cancel", "delete: cancel (any other key too), nothing is deleted", theme::PEACH, Group::Edit, CONFIRMING, Slot::Priority(2), None),
    key("enter", "move", "move: move it to the typed folder (a new scope asks first)", theme::GREEN, Group::Edit, MOVING, Slot::Priority(1), None),
    key("esc", "cancel", "move: cancel, nothing moves", theme::PEACH, Group::Edit, MOVING, Slot::Priority(2), None),
    key("tab", "scope", "move: next scope (personal, public, local, global), the typed folder stays", theme::SAPPHIRE, Group::Edit, MOVING, Slot::Priority(3), None),
    key("bksp", "delete", "move: delete the last char", theme::TEXT, Group::Edit, MOVING, Slot::Priority(4), None),
    key("enter", "apply", "set scope: move it to the same folder in the shown scope (asks first)", theme::GREEN, Group::Edit, SETTING_SCOPE, Slot::Priority(1), None),
    key("esc", "cancel", "set scope: cancel, nothing moves", theme::PEACH, Group::Edit, SETTING_SCOPE, Slot::Priority(2), None),
    key("tab", "scope", "set scope: next scope (personal, public, local, global)", theme::SAPPHIRE, Group::Edit, SETTING_SCOPE, Slot::Priority(3), None),
    key("y", "confirm", "move to another scope: yes, move it", theme::RED, Group::Edit, CONFIRMING_MOVE, Slot::Priority(1), None),
    key("n/esc", "cancel", "move to another scope: cancel (any other key too), nothing moves", theme::PEACH, Group::Edit, CONFIRMING_MOVE, Slot::Priority(2), None),
    key("/", "filter", "filter: text and #tag (starts a new filter)", theme::YELLOW, Group::Filter, BROWSE_AND_TAG, Slot::Priority(4), Some(Toggle::Filter)),
    key("enter", "keep", "filter: keep the filter, back to the list", theme::GREEN, Group::Filter, FILTERING, Slot::Priority(1), None),
    key("esc", "clear", "filter: clear it and close", theme::PEACH, Group::Filter, FILTERING, Slot::Priority(2), None),
    key("\u{2190}/\u{2192}", "cursor", "filter: move the cursor", theme::TEXT, Group::Filter, FILTERING, Slot::Priority(3), None),
    key("bksp", "delete", "filter: delete the char before the cursor; at the start, clear the filter and close", theme::TEXT, Group::Filter, FILTERING, Slot::Priority(4), None),
    key("esc", "clear", "clear marks; with none, clear the filter",theme::PEACH, Group::Filter, BROWSE, Slot::HelpOnly, None),
    key("click bar", "filter", "click the filter bar to filter, a dot to filter by that tag", theme::YELLOW, Group::Filter, BROWSE, Slot::HelpOnly, None),
    key("f", "focus", "focus the current section (again to leave)", theme::GREEN, Group::View, BROWSE, Slot::Priority(3), Some(Toggle::Focus)),
    key("v", "view all", "expand all / collapse all sections", theme::SAPPHIRE, Group::View, BROWSE, Slot::Priority(5), Some(Toggle::ExpandAll)),
    key("R", "reload", "reload the tree from disk (it also reloads by itself, within 2 s of idle, when a shown folder changes)", theme::SAPPHIRE, Group::View, BROWSE, Slot::HelpOnly, None),
    // In the footer only while a markdown note is selected; see `tree_keys`.
    key(PREVIEW_TOGGLE_KEY, "raw", "toggle the preview: rendered / raw markdown, highlighted / plain code", theme::SAPPHIRE, Group::View, BROWSE, Slot::HelpOnly, None),
    key("?", "help", "this help (? or esc closes)", theme::MAUVE, Group::View, BROWSE, Slot::Pinned, Some(Toggle::Help)),
    key(":q", "quit", "vim-style quit (also :wq, :qa, :q!)", theme::MAUVE, Group::View, BROWSE, Slot::HelpOnly, None),
    key("enter", "run", ":command: run it", theme::GREEN, Group::View, COMMAND, Slot::Priority(1), None),
    key("esc", "cancel", ":command: close the command line, nothing else", theme::PEACH, Group::View, COMMAND, Slot::Priority(2), None),
    key("bksp", "delete", ":command: delete the last char; deleting the : closes it", theme::TEXT, Group::View, COMMAND, Slot::Priority(3), None),
    // Navigate keys, then the pane keys, listed last so they end the
    // footer's hints. The pane keys drop first, then "J/K preview".
    key("J/K", "preview", "scroll preview down / up (also Shift+Down/Up)", theme::TEXT, Group::Navigate, BROWSE, Slot::Priority(11), None),
    key("PgDn/PgUp", "page", "scroll preview a page", theme::TEXT, Group::Navigate, BROWSE, Slot::HelpOnly, None),
    // With the preview focused, `pane_keys` swaps in the preview footer.
    key(PANE_FOCUS_KEY, "focus", "focus the list / the preview; 2 on the focused preview folds it, 2 on a folded one unfolds and focuses it", theme::LAVENDER, Group::View, BROWSE, Slot::Priority(13), None),
    key(PANE_CYCLE_KEY, "pane", "focus the other pane (in a prompt, tab cycles the scope instead)", theme::LAVENDER, Group::View, BROWSE, Slot::Priority(14), None),
    key("</>", "split", "narrow / widen the list by 5 points (both panes keep a usable width)", theme::LAVENDER, Group::View, BROWSE, Slot::Priority(15), None),
    key("=", "reset", "reset the split to 50/50", theme::LAVENDER, Group::View, BROWSE, Slot::Priority(16), None),
    key("q", "quit", "quit (also :q)", theme::PEACH, Group::View, BROWSE, Slot::Quit, None),
];

/// The footer mode for the tree's input state, most specific first.
pub(super) fn footer_mode(
    renaming: bool,
    vim_active: bool,
    search_mode: bool,
    flag_mode: bool,
    focus_active: bool,
) -> Mode {
    if renaming {
        Mode::Rename
    } else if vim_active {
        Mode::VimCommand
    } else if search_mode {
        Mode::Filter
    } else if flag_mode {
        Mode::Tag
    } else if focus_active {
        Mode::Focus
    } else {
        Mode::Normal
    }
}

/// Toggles that are on; their keys are lit in the footer.
pub(super) fn footer_toggles(
    focus_active: bool,
    filter_on: bool,
    flag_mode: bool,
    all_expanded: bool,
    help_open: bool,
) -> Vec<Toggle> {
    [
        (focus_active, Toggle::Focus),
        (filter_on, Toggle::Filter),
        (flag_mode, Toggle::Tag),
        (all_expanded, Toggle::ExpandAll),
        (help_open, Toggle::Help),
    ]
    .into_iter()
    .filter_map(|(on, toggle)| on.then_some(toggle))
    .collect()
}

/// True when some top-level section is collapsed: `v` then expands all,
/// otherwise it collapses all, and its footer hint is lit.
pub(super) fn any_top_collapsed(nodes: &[TreeNode]) -> bool {
    nodes
        .iter()
        .any(|n| n.is_dir && n.depth == 0 && !n.expanded)
}

/// Whether `v` is lit: at least one top-level directory exists and none is
/// collapsed. The `v` key itself only checks `any_top_collapsed`.
pub(super) fn view_all_lit(nodes: &[TreeNode]) -> bool {
    nodes.iter().any(|n| n.is_dir && n.depth == 0) && !any_top_collapsed(nodes)
}

/// Columns taken by the " ! " prefix of the warning footer.
const WARNING_PREFIX_COLS: usize = 3;

/// Lays out the warning footer for `width` columns: returns the warning text,
/// truncated by chars if needed, and the padding that puts the quit hint on the
/// same column as the hints footer (`footer::QUIT_HINT_RESERVED_COLS` from the
/// right edge, the column `footer::quit_column` gives). At least one space
/// always separates text and hint.
pub(super) fn warning_layout(warning: &str, width: usize) -> (String, usize) {
    let max_chars = width.saturating_sub(WARNING_PREFIX_COLS + QUIT_HINT_RESERVED_COLS + 1);
    let text: String = warning.chars().take(max_chars).collect();
    let used = WARNING_PREFIX_COLS + text.chars().count() + QUIT_HINT_RESERVED_COLS;
    (text, width.saturating_sub(used))
}

#[cfg(test)]
mod tests {
    use super::super::test_support::*;
    use super::*;
    use notez_core::tags::{FLAG_BLOCKED, FLAG_IDEA, FLAG_IMPORTANT, FLAG_LONGTERM, FLAG_PRIO};

    #[test]
    fn warning_layout_puts_the_quit_hint_four_columns_from_the_right_edge() {
        let (text, padding) = warning_layout("pull failed", 40);
        assert_eq!(text, "pull failed");
        assert_eq!(WARNING_PREFIX_COLS + text.chars().count() + padding, 40 - 4);
        assert!(padding >= 1);
    }

    #[test]
    fn warning_footer_quit_hint_lines_up_with_the_normal_footer() {
        // Compare with the column where the real hints footer draws `q`, in
        // the modes in which the warning can share the line with hints.
        let short = "pull failed";
        let long = "x".repeat(300);
        for width in [40usize, 60, 100, 120] {
            for mode in [Mode::Normal, Mode::Focus] {
                let line = footer::line(TREE_KEYS, mode, &[], width);
                let rendered: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
                let hints_q_col = rendered.chars().position(|c| c == 'q').unwrap();
                let quit_col = footer::select(TREE_KEYS, mode, &[], width).quit_col;
                assert_eq!(quit_col, Some(hints_q_col), "width {width}, {mode:?}");
                for warning in [short, long.as_str()] {
                    let (text, padding) = warning_layout(warning, width);
                    assert_eq!(
                        WARNING_PREFIX_COLS + text.chars().count() + padding,
                        hints_q_col,
                        "width {width}, {mode:?}, warning of {} chars",
                        warning.len()
                    );
                }
            }
            let (text, padding) = warning_layout(&long, width);
            assert_eq!(text.chars().count(), width - 8);
            assert!(padding >= 1);
        }
    }

    #[test]
    fn warning_layout_truncates_on_a_char_boundary_and_keeps_the_hint() {
        let warning = "vault pull failed \u{e5}\u{e4}\u{f6} \u{1f4a5} and then some more text";
        let (text, padding) = warning_layout(warning, 24);
        assert!(warning.starts_with(&text));
        assert_eq!(
            text.chars().count(),
            24 - WARNING_PREFIX_COLS - QUIT_HINT_RESERVED_COLS - 1
        );
        assert_eq!(padding, 1);
        assert_eq!(WARNING_PREFIX_COLS + text.chars().count() + padding, 24 - 4);

        let (cut, _) = warning_layout("ab\u{1f4a5}cd", 3 + 3 + 1 + 4);
        assert_eq!(cut, "ab\u{1f4a5}");
    }

    #[test]
    fn warning_layout_survives_zero_width() {
        assert_eq!(warning_layout("anything", 0), (String::new(), 0));
    }

    fn lit_keys(mode: Mode, on: &[Toggle]) -> Vec<&'static str> {
        let sel = footer::select(TREE_KEYS, mode, on, 200);
        sel.left
            .iter()
            .chain(sel.quit.iter())
            .filter(|&&(_, lit)| lit)
            .map(|&(i, _)| TREE_KEYS[i].key)
            .collect()
    }

    #[test]
    fn footer_mode_follows_the_input_state_most_specific_first() {
        assert_eq!(footer_mode(false, false, false, false, false), Mode::Normal);
        assert_eq!(footer_mode(false, false, false, false, true), Mode::Focus);
        assert_eq!(footer_mode(false, false, false, true, true), Mode::Tag);
        assert_eq!(footer_mode(false, false, true, false, true), Mode::Filter);
        assert_eq!(
            footer_mode(false, true, true, false, false),
            Mode::VimCommand
        );
        assert_eq!(footer_mode(true, true, true, true, true), Mode::Rename);
    }

    #[test]
    fn normal_and_focus_footers_hint_the_browse_keys() {
        let expected = vec![
            "o", "t", "r", "n", "N", "m", "S", "space", "d", "/", "f", "v", "?", "J/K", "1/2",
            "tab", "</>", "=", "q",
        ];
        assert_eq!(shown_keys(Mode::Normal, &[], 200), expected);
        assert_eq!(shown_keys(Mode::Focus, &[], 200), expected);
    }

    #[test]
    fn filter_footer_hints_the_filter_input_keys_and_no_quit() {
        assert_eq!(
            shown_keys(Mode::Filter, &[], 200),
            vec!["enter", "esc", "\u{2190}/\u{2192}", "bksp"]
        );
    }

    #[test]
    fn tag_rename_and_command_footers_hint_their_own_keys() {
        assert_eq!(
            shown_keys(Mode::Tag, &[], 200),
            vec!["t", "1-5", "esc", "/"]
        );
        assert_eq!(
            shown_keys(Mode::Rename, &[], 200),
            vec!["enter", "esc", "bksp"]
        );
        assert_eq!(
            shown_keys(Mode::VimCommand, &[], 200),
            vec!["enter", "esc", "bksp"]
        );
    }

    #[test]
    fn footer_lights_the_keys_whose_state_is_on() {
        assert!(lit_keys(Mode::Normal, &[]).is_empty());
        assert_eq!(lit_keys(Mode::Focus, &[Toggle::Focus]), vec!["f"]);
        assert_eq!(lit_keys(Mode::Normal, &[Toggle::Filter]), vec!["/"]);
        assert_eq!(lit_keys(Mode::Tag, &[Toggle::Tag]), vec!["t"]);
        assert_eq!(lit_keys(Mode::Normal, &[Toggle::ExpandAll]), vec!["v"]);
        assert_eq!(lit_keys(Mode::Normal, &[Toggle::Help]), vec!["?"]);
    }

    #[test]
    fn footer_toggles_map_each_state_to_its_toggle() {
        assert!(footer_toggles(false, false, false, false, false).is_empty());
        assert_eq!(
            footer_toggles(true, true, true, true, true),
            vec![
                Toggle::Focus,
                Toggle::Filter,
                Toggle::Tag,
                Toggle::ExpandAll,
                Toggle::Help
            ]
        );
    }

    #[test]
    fn narrow_footer_drops_low_priority_hints_but_keeps_help_and_quit() {
        let all = vec!["o", "t", "r", "n", "d", "/", "f", "v", "?", "q"];
        assert_eq!(shown_keys(Mode::Normal, &[], 72), all);
        assert_eq!(
            shown_keys(Mode::Normal, &[], 71),
            vec!["o", "t", "r", "n", "/", "f", "v", "?", "q"]
        );
        assert_eq!(
            shown_keys(Mode::Normal, &[], 64),
            vec!["o", "t", "r", "n", "/", "f", "v", "?", "q"]
        );
        assert_eq!(
            shown_keys(Mode::Normal, &[], 63),
            vec!["o", "t", "n", "/", "f", "v", "?", "q"]
        );
        assert_eq!(
            shown_keys(Mode::Normal, &[], 55),
            vec!["o", "t", "n", "/", "f", "?", "q"]
        );
        assert_eq!(
            shown_keys(Mode::Normal, &[], 45),
            vec!["o", "t", "n", "f", "?", "q"]
        );
        assert_eq!(
            shown_keys(Mode::Normal, &[], 35),
            vec!["o", "t", "n", "?", "q"]
        );
        assert_eq!(shown_keys(Mode::Normal, &[], 28), vec!["o", "t", "?", "q"]);
        assert_eq!(shown_keys(Mode::Normal, &[], 18), vec!["o", "?", "q"]);
        assert_eq!(shown_keys(Mode::Normal, &[], 17), vec!["?", "q"]);
        assert_eq!(shown_keys(Mode::Normal, &[], 0), vec!["?", "q"]);
        for width in 0..120 {
            let line = footer::line(TREE_KEYS, Mode::Normal, &[], width);
            let cols: usize = line.spans.iter().map(|s| s.content.chars().count()).sum();
            // From 12 columns (" ? help" plus a space and "quit") the line
            // fills the width exactly, with `q` four columns from the edge.
            if width >= 12 {
                assert_eq!(cols, width, "width {width}");
            }
        }
    }

    #[test]
    fn help_lists_every_tree_key_exactly_once() {
        let rows = help::rows(TREE_KEYS);
        for i in 0..TREE_KEYS.len() {
            let count = rows.iter().filter(|r| **r == help::Row::Key(i)).count();
            assert_eq!(count, 1, "key {} ({})", TREE_KEYS[i].key, TREE_KEYS[i].help);
        }
        for group in Group::ALL {
            assert!(rows.contains(&help::Row::Heading(group)), "{group:?}");
        }
    }

    #[test]
    fn scrolled_clamps_at_the_top() {
        assert_eq!(scrolled(0, -1, 10), 0);
        assert_eq!(scrolled(2, -3, 10), 0);
        assert_eq!(scrolled(0, i32::MIN, 10), 0);
    }

    // --- Panes: split, focus and fold (NZ-4) ---

    #[test]
    fn the_pane_keys_are_browse_rows_in_the_view_group_listed_once() {
        for key in PANE_KEYS {
            let rows: Vec<_> = TREE_KEYS
                .iter()
                .filter(|k| k.key == key && k.modes == BROWSE)
                .collect();
            assert_eq!(rows.len(), 1, "{key}");
            assert_eq!(rows[0].group, Group::View, "{key}");
            assert!(matches!(rows[0].slot, Slot::Priority(_)), "{key}");
        }
        // The keys were free: no other browse row names `<`, `>`, `=`, `1`
        // or `2`, and `tab` is otherwise only a prompt key.
        for hint in TREE_KEYS.iter().filter(|k| !PANE_KEYS.contains(&k.key)) {
            let parts: Vec<&str> = hint.key.split('/').collect();
            let browse = hint.modes.iter().any(|m| BROWSE.contains(m));
            for key in ["<", ">", "=", "1", "2", "tab"] {
                assert!(
                    !(browse && parts.contains(&key)),
                    "{} ({}) also binds {key}",
                    hint.key,
                    hint.help
                );
            }
        }
        for prompt in [NEW_NOTE, MOVING, SETTING_SCOPE] {
            assert!(TREE_KEYS
                .iter()
                .any(|k| k.key == "tab" && k.modes == prompt));
        }
    }

    #[test]
    fn the_pane_keys_drop_first_and_in_reverse_table_order() {
        let first_width = |key: &str| {
            (0..300)
                .find(|&w| shown_keys(Mode::Normal, &[], w).contains(&key))
                .unwrap()
        };
        let widths: Vec<usize> = PANE_KEYS.iter().map(|key| first_width(key)).collect();
        assert!(widths.windows(2).all(|w| w[0] < w[1]), "{widths:?}");
        // Above the pane keys' widths the footer still fills the line.
        for width in 0..160 {
            let line = footer::line(TREE_KEYS, Mode::Normal, &[], width);
            let cols: usize = line.spans.iter().map(|s| s.content.chars().count()).sum();
            if width >= 12 {
                assert_eq!(cols, width, "width {width}");
            }
        }
    }

    fn pane_footer(focus: Pane, toggle: Option<PreviewMode>, width: usize) -> Vec<String> {
        let keys = pane_keys(tree_keys(toggle.map(PreviewToggle::markdown)), focus);
        let sel = footer::select(&keys, Mode::Normal, &[], width);
        sel.left
            .iter()
            .chain(sel.quit.iter())
            .map(|&(i, _)| format!("{} {}", keys[i].key, keys[i].desc))
            .collect()
    }

    #[test]
    fn the_list_focused_footer_is_the_browse_footer() {
        let keys = pane_keys(tree_keys(None), Pane::List);
        let shown: Vec<&str> = footer::select(&keys, Mode::Normal, &[], 200)
            .left
            .iter()
            .map(|&(i, _)| keys[i].key)
            .collect();
        assert_eq!(shown, shown_keys(Mode::Normal, &[], 200)[..shown.len()]);
    }

    #[test]
    fn the_preview_focused_footer_shows_the_preview_set() {
        assert_eq!(
            pane_footer(Pane::Preview, None, 200),
            vec![
                "j/k scroll",
                "? help",
                "PgDn/PgUp page",
                "2 fold",
                "1/tab list",
                "q quit"
            ]
        );
        assert_eq!(
            pane_footer(Pane::Preview, Some(PreviewMode::Rendered), 200),
            vec![
                "j/k scroll",
                "p raw",
                "? help",
                "PgDn/PgUp page",
                "2 fold",
                "1/tab list",
                "q quit"
            ]
        );
        assert_eq!(
            pane_footer(Pane::Preview, Some(PreviewMode::Raw), 200),
            vec![
                "j/k scroll",
                "p rendered",
                "? help",
                "PgDn/PgUp page",
                "2 fold",
                "1/tab list",
                "q quit"
            ]
        );
        // Narrow, the scroll hints are the last to go before help and quit.
        assert_eq!(
            pane_footer(Pane::Preview, Some(PreviewMode::Raw), 30),
            vec!["j/k scroll", "? help", "q quit"]
        );
    }

    #[test]
    fn the_preview_footer_keeps_the_same_rows_so_help_lists_each_key_once() {
        for toggle in [None, Some(PreviewToggle::markdown(PreviewMode::Rendered))] {
            let keys = pane_keys(tree_keys(toggle), Pane::Preview);
            assert_eq!(keys.len(), TREE_KEYS.len());
            for (shown, row) in keys.iter().zip(TREE_KEYS) {
                assert_eq!(shown.help, row.help);
                assert_eq!(shown.modes, row.modes);
            }
        }
        // The prompt `tab` rows keep their own footer slots.
        let keys = pane_keys(tree_keys(None), Pane::Preview);
        let prompt_tab = keys
            .iter()
            .find(|k| k.key == "tab" && k.modes == NEW_NOTE)
            .unwrap();
        assert_eq!(prompt_tab.slot, Slot::Priority(3));
    }

    #[test]
    fn the_focused_pane_has_the_highlighted_border_and_both_titles_carry_their_number() {
        assert_eq!(pane_border(Pane::List, Pane::List), theme::border_focused());
        assert_eq!(pane_border(Pane::Preview, Pane::List), theme::border());
        assert_eq!(
            pane_border(Pane::Preview, Pane::Preview),
            theme::border_focused()
        );
        assert_ne!(theme::border_focused(), theme::border());
        let list = pane_title(Pane::List, Pane::List, vec![Span::raw("notes ")]);
        assert_eq!(text_of(&list), " 1 notes ");
        assert_eq!(list.spans[0].style, theme::pane_number(true));
        let preview = pane_title(Pane::Preview, Pane::List, vec![Span::raw("preview ")]);
        assert_eq!(text_of(&preview), " 2 preview ");
        assert_eq!(preview.spans[0].style, theme::pane_number(false));
    }

    #[test]
    fn the_list_text_width_follows_the_split() {
        let total = Rect::new(2, 1, 120, 30);
        let mut panes = Panes::default();
        let (list, _, _) = panes.layout(total);
        assert_eq!(list_text_width(list.width), 60 - 4);
        panes.handle_key(KeyCode::Char('<'), total.width);
        let (narrower, _, _) = panes.layout(total);
        assert_eq!(list_text_width(narrower.width), 54 - 4);
        panes.handle_key(KeyCode::Char('2'), total.width);
        panes.handle_key(KeyCode::Char('2'), total.width);
        let (folded, _, preview) = panes.layout(total);
        assert!(preview.is_none());
        assert_eq!(list_text_width(folded.width), 120 - 4);
    }

    #[test]
    fn the_filter_strip_rows_and_tag_dots_take_clicks_at_a_30_and_a_70_split() {
        let total = Rect::new(2, 1, 120, 30);
        for split in [30u16, 70] {
            let mut panes = Panes {
                split,
                focus: Pane::Preview,
                ..Panes::default()
            };
            panes.clamp(total.width);
            let (list, _, _) = panes.layout(total);
            let chunks = list_chunks(list);
            let (strip, rows) = (chunks[0], chunks[2]);
            assert!(rows.height > 0 && strip.y < rows.y, "split {split}");
            // Every cell of the strip and the rows reaches the list's click
            // handling: none starts a drag, and each focuses the list.
            for (area, y) in [
                (strip, strip.y),
                (rows, rows.y),
                (rows, rows.y + rows.height - 1),
            ] {
                for x in area.x..area.x + area.width {
                    let mut clicked = panes;
                    assert_eq!(
                        clicked.press(total, x, y),
                        Press::Pane(Pane::List),
                        "split {split} ({x}, {y})"
                    );
                    assert_eq!(clicked.focus, Pane::List);
                    assert!(!clicked.dragging);
                }
            }
            // The strip's five dots map to their tags, as at 50/50, in the
            // same columns as a row's five slots right after its gutter.
            assert_eq!(strip.x, rows.x, "split {split}");
            for dot in 0..5u8 {
                let col = strip.x + 1 + u16::from(dot);
                assert_eq!(mouse_x_to_dot(col, strip.x), Some(dot), "split {split}");
                assert_eq!(
                    mouse_x_to_field_slot(col, rows.x),
                    Some(usize::from(dot)),
                    "split {split}"
                );
            }
            for col in [rows.x, rows.x + 6, rows.x + 7] {
                assert_eq!(
                    mouse_x_to_dot(col, strip.x),
                    None,
                    "split {split} column {col}"
                );
                assert_eq!(
                    mouse_x_to_field_slot(col, rows.x),
                    None,
                    "split {split} column {col}"
                );
            }
            // A row fills exactly the text width the split gives it.
            assert_eq!(
                usize::from(rows.width),
                list_text_width(list.width),
                "split {split}"
            );
        }
    }

    #[test]
    fn the_grip_is_furniture_at_rest_and_lit_while_dragging() {
        assert_eq!(theme::grip(false), theme::border());
        assert_eq!(theme::grip(true), theme::border_focused());
    }

    fn dir_node(depth: usize) -> TreeNode {
        TreeNode {
            name: "dir".into(),
            path: PathBuf::from("dir"),
            origin: PathBuf::from("dir"),
            is_dir: true,
            depth,
            expanded: false,
            child_count: 0,
            parent_idx: None,
            flags: 0,
            scope_icon: "",
            tag_root: 0,
            section: 0,
        }
    }

    #[test]
    fn any_top_collapsed_decides_the_view_all_toggle() {
        let mut nodes = vec![dir_node(0), dir_node(0)];
        nodes[0].expanded = true;
        assert!(any_top_collapsed(&nodes));
        nodes[1].expanded = true;
        assert!(!any_top_collapsed(&nodes));
    }

    #[test]
    fn view_all_is_lit_only_with_a_top_dir_and_none_collapsed() {
        assert!(!view_all_lit(&[]));
        let mut file = dir_node(0);
        file.is_dir = false;
        assert!(!view_all_lit(&[file]));
        assert!(!view_all_lit(&[dir_node(1)]));
        let mut nodes = vec![dir_node(0), dir_node(0)];
        nodes[0].expanded = true;
        assert!(!view_all_lit(&nodes));
        nodes[1].expanded = true;
        assert!(view_all_lit(&nodes));
    }

    #[test]
    fn tag_footer_draws_the_legend_then_its_hints_with_t_lit() {
        let legend = tag_legend(0b1);
        let legend_text = text_of(&Line::from(legend.clone()));
        let line = lead_with_hints(legend, Mode::Tag, &[Toggle::Tag], 100);
        let rendered = text_of(&line);
        assert!(rendered.starts_with(&legend_text), "{rendered:?}");
        let rest = &rendered[legend_text.len()..];
        for hint in ["tags", "1-5 toggle", "esc close"] {
            assert!(rest.contains(hint), "{hint}: {rendered:?}");
        }
        let lead_spans = tag_legend(0b1).len();
        let t = line.spans[lead_spans..]
            .iter()
            .find(|s| s.content == "t")
            .unwrap();
        assert_eq!(t.style.fg, Some(theme::GREEN));
        assert!(rendered.chars().count() <= 100);
    }

    #[test]
    fn rename_footer_draws_the_prompt_then_its_hints() {
        let rendered = text_of(&lead_with_hints(
            rename_lead("draft"),
            Mode::Rename,
            &[],
            100,
        ));
        assert!(rendered.starts_with(" rename: draft_ "), "{rendered:?}");
        for hint in ["enter confirm", "esc cancel", "bksp delete"] {
            assert!(rendered.contains(hint), "{hint}: {rendered:?}");
        }
        assert!(!rendered.contains("quit"));
    }

    #[test]
    fn command_footer_draws_the_buffer_then_its_hints() {
        let rendered = text_of(&lead_with_hints(
            command_lead(":wq"),
            Mode::VimCommand,
            &[],
            100,
        ));
        assert!(rendered.starts_with(":wq "), "{rendered:?}");
        for hint in ["enter run", "esc cancel", "bksp delete"] {
            assert!(rendered.contains(hint), "{hint}: {rendered:?}");
        }
    }

    #[test]
    fn narrow_lead_footers_keep_the_lead_and_drop_hints_first() {
        let cases = [
            (tag_legend(0), Mode::Tag),
            (rename_lead("a fairly long note name"), Mode::Rename),
            (command_lead(":something"), Mode::VimCommand),
        ];
        for (lead, mode) in cases {
            let lead_text = text_of(&Line::from(lead.clone()));
            let lead_cols = lead_text.chars().count();
            let full = text_of(&lead_with_hints(lead.clone(), mode, &[], 200));
            let mut previous = full.chars().count();
            for width in (0..=lead_cols + 40).rev() {
                let rendered = text_of(&lead_with_hints(lead.clone(), mode, &[], width));
                assert!(
                    rendered.starts_with(&lead_text),
                    "{mode:?} width {width}: {rendered:?}"
                );
                assert!(
                    rendered.chars().count() <= width.max(lead_cols),
                    "{mode:?} width {width}"
                );
                assert!(
                    rendered.chars().count() <= previous,
                    "{mode:?} width {width}"
                );
                previous = rendered.chars().count();
            }
            assert_eq!(
                text_of(&lead_with_hints(lead.clone(), mode, &[], lead_cols)),
                lead_text
            );
            assert!(
                full.chars().count() > lead_cols,
                "{mode:?} shows hints when wide"
            );
        }
    }

    #[test]
    fn a_pull_warning_stays_up_while_one_off_statuses_come_and_go() {
        let warning = Some("vault pull hit a conflict, rebase aborted");
        // The event loop clears the one-off message on every key press; the
        // warning lives in the context and must survive that.
        let mut message: Option<&str> = None;
        assert_eq!(
            status_slot(None, message, false, false, warning),
            StatusSlot::Warning(warning.unwrap()),
            "the warning shows as the session opens"
        );
        message = Some("rename failed: exists");
        assert_eq!(
            status_slot(None, message, false, false, warning),
            StatusSlot::Message("rename failed: exists")
        );
        message = None;
        assert_eq!(
            status_slot(None, message, false, false, warning),
            StatusSlot::Warning(warning.unwrap()),
            "a one-off status clearing must not take the warning with it"
        );
        assert_eq!(
            status_slot(Some("new"), message, false, false, warning),
            StatusSlot::Rename("new")
        );
        assert_eq!(
            status_slot(None, message, true, false, warning),
            StatusSlot::VimCommand
        );
        assert_eq!(
            status_slot(None, message, false, true, warning),
            StatusSlot::Tags
        );
        assert_eq!(
            status_slot(None, message, false, false, warning),
            StatusSlot::Warning(warning.unwrap()),
            "the warning comes back once rename, vim and tag mode end"
        );
        assert_eq!(
            status_slot(None, None, false, false, None),
            StatusSlot::Hints
        );
    }

    #[test]
    fn forest_orders_numbered_dirs_then_other_dirs_then_files() {
        let s = spec(
            "/r",
            "S",
            &[
                "zzz-other/z.md",
                "00_quick-notes/a.md",
                "top.md",
                "01_daily-logs/b.md",
            ],
        );
        let (nodes, _) = build_forest(&[s]);
        let names: Vec<&str> = nodes.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "S",
                "00_quick-notes",
                "a.md",
                "01_daily-logs",
                "b.md",
                "zzz-other",
                "z.md",
                "top.md",
            ],
        );
    }

    // --- Scope badges ---

    /// The first column of a row: blank, or the mark of a marked row.
    const GUTTER_COL: usize = 0;

    /// The column after the gutter, the five-slot tag field and its space:
    /// where the tree drawing starts on every row.
    const TREE_COL: usize = GUTTER_COL + 1 + TAG_FIELD_WIDTH + 1;

    #[test]
    fn every_file_row_has_a_badge_in_its_scope_colour_and_the_rest_unchanged() {
        let (sections, nodes) = badged_forest();
        let cases = [
            (
                "/n/personal/proj/top.md",
                Scope::Personal.icon(),
                Scope::Personal,
                "└─  ",
            ),
            (
                "/p/notez/plans/b.md",
                Scope::Public.icon(),
                Scope::Public,
                "  └─  ",
            ),
            ("/p/docs/design/c.md", "\u{f02d}", Scope::Public, "  └─  "),
            ("/p/.notez/d.md", Scope::Local.icon(), Scope::Local, "└─  "),
            ("/n/e.md", Scope::Global.icon(), Scope::Global, "└─  "),
        ];
        for (path, icon, colour_scope, branch) in cases {
            let node = &nodes[row(&nodes, path)];
            let cells = render_row(line_of(&sections, &nodes, row(&nodes, path)));
            assert_eq!(cells[GUTTER_COL].0, " ", "{path}: the gutter keeps a blank");
            let badge = TREE_COL + Span::raw(branch).width();
            assert_eq!(cells[badge].0, icon, "{path}");
            assert_eq!(
                cells[badge].1,
                Some(theme::scope_color(colour_scope)),
                "{path}"
            );
            let rest: String = cells[TREE_COL..].iter().map(|c| c.0.as_str()).collect();
            let expected = format!("{branch}{icon} {}", node.name);
            assert_eq!(rest.trim_end(), expected, "{path}");
        }
    }

    #[test]
    fn folder_rows_have_the_badge_too() {
        let (sections, nodes) = badged_forest();
        let line = line_of(&sections, &nodes, row(&nodes, "/n/personal/proj/ideas"));
        let cells = render_row(line);
        assert_eq!(cells[GUTTER_COL].0, " ", "the gutter keeps a blank");
        let badge = TREE_COL + 4;
        assert_eq!(cells[badge].0, Scope::Personal.icon());
        assert_eq!(cells[badge].1, Some(theme::scope_color(Scope::Personal)));
        let rest: String = cells[TREE_COL..].iter().map(|c| c.0.as_str()).collect();
        let expected = format!("├─▾ {} ideas ", Scope::Personal.icon());
        assert!(rest.starts_with(&expected), "{rest:?}");
        assert!(rest.trim_end().ends_with('1'), "{rest:?}");
    }

    #[test]
    fn a_section_header_shows_its_scope_by_icon_and_colour_without_the_scope_word() {
        let (sections, nodes) = badged_forest();
        for (i, spec) in sections.iter().enumerate() {
            let idx = nodes
                .iter()
                .position(|n| n.depth == 0 && n.section == i)
                .unwrap();
            let node = &nodes[idx];
            let line = line_of(&sections, &nodes, idx);
            let word = spec.scope.label();
            let colour = Some(theme::scope_color(spec.scope));
            assert!(
                line.spans.iter().all(|s| s.content != word),
                "{word}: {:?}",
                span_texts(&line)
            );
            let icon_span = line
                .spans
                .iter()
                .find(|s| s.content.starts_with(spec.icon))
                .expect("icon");
            assert_eq!(icon_span.style.fg, colour, "{word} icon");

            let cells = render_row(line);
            assert_eq!(
                cells[GUTTER_COL].0, " ",
                "{word}: no second icon on the header"
            );
            let rest: String = cells[TREE_COL..].iter().map(|c| c.0.as_str()).collect();
            let expected = format!("▼ {} {} ··", spec.icon, spec.label);
            assert!(rest.starts_with(&expected), "{rest:?}");
            assert!(
                rest.trim_end().ends_with(&node.child_count.to_string()),
                "{rest:?}"
            );
        }
    }

    #[test]
    fn a_click_on_a_set_dot_hits_its_slot_and_the_badge_does_not() {
        let (sections, mut nodes) = badged_forest();
        let idx = row(&nodes, "/p/.notez/d.md");
        nodes[idx].flags = FLAG_PRIO;
        let cells = render_row(line_of(&sections, &nodes, idx));
        let lit = cells.iter().position(|c| c.0 == "●").expect("a set dot");
        let prio = FLAG_DEFS
            .iter()
            .position(|def| def.bit == FLAG_PRIO)
            .unwrap();
        assert_eq!(mouse_x_to_field_slot(lit as u16, 0), Some(prio));
        let badge = cells
            .iter()
            .position(|c| c.0 == Scope::Local.icon())
            .expect("the badge");
        assert_eq!(
            mouse_x_to_field_slot(badge as u16, 0),
            None,
            "the badge is outside the field"
        );
    }

    // --- Todo icon (NZ-24) ---

    /// A personal section with a project `TODO.md`, a lowercase `todo.md`
    /// and an ordinary note, and the global store with the todo board's
    /// `_todos` (a note and a category folder) beside an ordinary note;
    /// icons as the real listing gives them, every row expanded.
    fn todo_icon_forest() -> (Vec<SectionSpec>, Vec<TreeNode>) {
        let mut personal = scoped(
            "/n/personal/proj",
            Scope::Personal,
            false,
            &["TODO.md", "todo.md", "a.md"],
        );
        personal.icon = Scope::Personal.icon();
        let mut global = scoped(
            "/n",
            Scope::Global,
            false,
            &["_todos/t.md", "_todos/work/w.md", "e.md"],
        );
        global.icon = Scope::Global.icon();
        let sections = vec![personal, global];
        let (mut nodes, _) = build_forest(&sections);
        for node in &mut nodes {
            node.expanded = true;
        }
        (sections, nodes)
    }

    /// The badge `path`'s row draws right before its name: text and colour.
    fn badge_of(
        sections: &[SectionSpec],
        nodes: &[TreeNode],
        path: &str,
    ) -> (String, Option<Color>) {
        let node = &nodes[row(nodes, path)];
        let line = line_of(sections, nodes, row(nodes, path));
        let badge = &line.spans[name_index(&line, node) - 1];
        (badge.content.to_string(), badge.style.fg)
    }

    #[test]
    fn the_todo_store_and_every_row_under_it_wear_the_todo_icon_in_the_scope_colour() {
        let (sections, nodes) = todo_icon_forest();
        let todo = (
            format!("{} ", theme::ICON_TODO),
            Some(theme::scope_color(Scope::Global)),
        );
        for path in [
            "/n/_todos",
            "/n/_todos/t.md",
            "/n/_todos/work",
            "/n/_todos/work/w.md",
        ] {
            assert_eq!(badge_of(&sections, &nodes, path), todo, "{path}");
        }
        let cells = render_row(line_of(&sections, &nodes, row(&nodes, "/n/_todos/t.md")));
        let badge = TREE_COL + 6;
        assert_eq!(cells[badge].0, theme::ICON_TODO);
        assert_eq!(cells[badge].1, Some(theme::scope_color(Scope::Global)));
        let rest: String = cells[TREE_COL..].iter().map(|c| c.0.as_str()).collect();
        assert_eq!(rest.trim_end(), format!("│ └─  {} t.md", theme::ICON_TODO));
    }

    #[test]
    fn a_project_todo_md_wears_the_todo_icon_and_a_lowercase_todo_md_does_not() {
        let (sections, nodes) = todo_icon_forest();
        let personal = Some(theme::scope_color(Scope::Personal));
        let todo = format!("{} ", theme::ICON_TODO);
        let scope_badge = format!("{} ", Scope::Personal.icon());
        assert_eq!(
            badge_of(&sections, &nodes, "/n/personal/proj/TODO.md"),
            (todo, personal)
        );
        assert_eq!(
            badge_of(&sections, &nodes, "/n/personal/proj/todo.md"),
            (scope_badge.clone(), personal)
        );
        assert_eq!(
            badge_of(&sections, &nodes, "/n/personal/proj/a.md"),
            (scope_badge, personal)
        );
        let global = (
            format!("{} ", Scope::Global.icon()),
            Some(theme::scope_color(Scope::Global)),
        );
        assert_eq!(badge_of(&sections, &nodes, "/n/e.md"), global);
    }

    #[test]
    fn section_headers_keep_their_scope_icon_next_to_the_todo_rows() {
        let (sections, nodes) = todo_icon_forest();
        for (idx, node) in nodes.iter().enumerate().filter(|(_, n)| n.depth == 0) {
            let line = line_of(&sections, &nodes, idx);
            let texts = span_texts(&line);
            let icon = sections[node.section].icon;
            assert!(texts.contains(&format!("{icon} ")), "{texts:?}");
            assert!(
                texts.iter().all(|t| !t.contains(theme::ICON_TODO)),
                "{texts:?}"
            );
        }
    }

    #[test]
    fn the_todo_icon_is_as_wide_as_the_scope_icons_and_todo_counts_still_align() {
        let width = Span::raw(theme::ICON_TODO).width();
        assert_eq!(width, 1);
        for scope in [Scope::Local, Scope::Personal, Scope::Public, Scope::Global] {
            assert_eq!(Span::raw(scope.icon()).width(), width, "{scope:?}");
        }
        let (sections, nodes) = todo_icon_forest();
        for path in ["/n/_todos", "/n/_todos/work"] {
            let node = &nodes[row(&nodes, path)];
            let line = line_of(&sections, &nodes, row(&nodes, path));
            assert_eq!(
                line.width(),
                LIST_TEXT_WIDTH,
                "{path}: {:?}",
                span_texts(&line)
            );
            assert_eq!(
                line.spans.last().unwrap().content,
                node.child_count.to_string(),
                "{path}"
            );
        }
    }

    // --- Row alignment ---

    /// One personal section with folders at depth 1 and 2, a file beside
    /// the depth 2 folder, and folder names with multi-byte and wide
    /// characters; badges as the real listing gives them, every row expanded.
    fn aligned_forest() -> (Vec<SectionSpec>, Vec<TreeNode>) {
        let mut spec = scoped(
            "/n/personal/proj",
            Scope::Personal,
            false,
            &["ideas/deep/x.md", "ideas/a.md", "åäö/b.md", "日本語/c.md"],
        );
        spec.icon = Scope::Personal.icon();
        let sections = vec![spec];
        let (mut nodes, _) = build_forest(&sections);
        for node in &mut nodes {
            node.expanded = true;
        }
        (sections, nodes)
    }

    fn span_texts(line: &Line<'static>) -> Vec<String> {
        line.spans.iter().map(|s| s.content.to_string()).collect()
    }

    /// The screen column where `line`'s span `index` starts.
    fn column_of(line: &Line<'static>, index: usize) -> usize {
        line.spans[..index].iter().map(Span::width).sum()
    }

    fn name_index(line: &Line<'static>, node: &TreeNode) -> usize {
        line.spans
            .iter()
            .position(|s| s.content == node.name)
            .expect("name span")
    }

    #[test]
    fn a_nested_row_draws_its_badge_right_before_the_name_and_keeps_the_gutter() {
        let (sections, nodes) = aligned_forest();
        let colour = Some(theme::scope_color(Scope::Personal));
        for (idx, node) in nodes.iter().enumerate().filter(|(_, n)| n.depth > 0) {
            let line = line_of(&sections, &nodes, idx);
            let name = name_index(&line, node);
            let path = node.path.display();
            let badge = format!("{} ", Scope::Personal.icon());
            assert_eq!(line.spans[name - 1].content, badge, "{path}");
            assert_eq!(line.spans[name - 1].style.fg, colour, "{path}");
            assert_eq!(line.spans[name - 2].content, aligned_branch(node), "{path}");
            assert_eq!(
                line.spans[0].content, " ",
                "{path}: the gutter keeps a blank"
            );
            assert_eq!(
                column_of(&line, name - 2),
                TREE_COL,
                "{path}: the branch follows the fixed tag field"
            );
        }
    }

    /// The branch drawing of each nested row of [`aligned_forest`].
    fn aligned_branch(node: &TreeNode) -> &'static str {
        let rel = node
            .path
            .strip_prefix("/n/personal/proj")
            .unwrap()
            .to_str()
            .unwrap();
        match rel {
            "ideas" => "├─▾ ",
            "ideas/deep" => "│ ├─▾ ",
            "ideas/deep/x.md" => "│ │ └─  ",
            "ideas/a.md" => "│ └─  ",
            "åäö" => "├─▾ ",
            "åäö/b.md" => "│ └─  ",
            "日本語" => "└─▾ ",
            "日本語/c.md" => "  └─  ",
            other => panic!("no row {other}"),
        }
    }

    #[test]
    fn a_nested_folder_shows_tee_or_corner_by_its_later_siblings_and_its_open_state() {
        let (sections, mut nodes) = aligned_forest();
        let branch = |nodes: &[TreeNode], path: &str| {
            let idx = row(nodes, path);
            let line = line_of(&sections, nodes, idx);
            line.spans[name_index(&line, &nodes[idx]) - 2]
                .content
                .to_string()
        };
        assert_eq!(
            branch(&nodes, "/n/personal/proj/ideas"),
            "├─▾ ",
            "a later sibling follows"
        );
        assert_eq!(
            branch(&nodes, "/n/personal/proj/日本語"),
            "└─▾ ",
            "the last child"
        );
        let last = row(&nodes, "/n/personal/proj/日本語");
        nodes[last].expanded = false;
        assert_eq!(branch(&nodes, "/n/personal/proj/日本語"), "└─▸ ", "closed");
        let ideas = row(&nodes, "/n/personal/proj/ideas");
        nodes[ideas].expanded = false;
        assert_eq!(branch(&nodes, "/n/personal/proj/ideas"), "├─▸ ", "closed");
    }

    #[test]
    fn a_nested_file_at_depth_2_as_the_last_child_draws_a_bar_for_its_parent_and_a_corner() {
        let (sections, nodes) = aligned_forest();
        let idx = row(&nodes, "/n/personal/proj/ideas/a.md");
        assert_eq!(nodes[idx].depth, 2);
        let rest: String = render_row(line_of(&sections, &nodes, idx))[TREE_COL..]
            .iter()
            .map(|c| c.0.as_str())
            .collect();
        assert_eq!(
            rest.trim_end(),
            format!("│ └─  {} a.md", Scope::Personal.icon())
        );
        let under_last = row(&nodes, "/n/personal/proj/日本語/c.md");
        let line = line_of(&sections, &nodes, under_last);
        assert_eq!(
            line.spans[name_index(&line, &nodes[under_last]) - 2].content,
            "  └─  ",
            "no bar under the last folder"
        );
    }

    #[test]
    fn later_siblings_counts_only_the_visible_rows_under_the_same_parent() {
        let (_, nodes) = aligned_forest();
        let all = compute_visible(&nodes, "");
        let later = later_siblings(&nodes, &all);
        let at = |later: &[bool], path: &str| later[row(&nodes, path)];
        assert!(at(&later, "/n/personal/proj/ideas"));
        assert!(at(&later, "/n/personal/proj/ideas/deep"));
        assert!(!at(&later, "/n/personal/proj/ideas/a.md"));
        assert!(!at(&later, "/n/personal/proj/ideas/deep/x.md"));
        assert!(at(&later, "/n/personal/proj/åäö"));
        assert!(!at(&later, "/n/personal/proj/日本語"));
        assert!(!at(&later, "/n/personal/proj"), "the only section");

        // The filter keeps x.md and its ancestors: the siblings it hides no
        // longer count, so the path down to x.md is all corners and blanks.
        let filtered = compute_visible(&nodes, "x.md");
        assert_eq!(filtered.len(), 4);
        let later = later_siblings(&nodes, &filtered);
        assert!(!at(&later, "/n/personal/proj/ideas"));
        assert!(!at(&later, "/n/personal/proj/ideas/deep"));
        assert!(
            !at(&later, "/n/personal/proj/åäö"),
            "a hidden row gets none"
        );
        let x = row(&nodes, "/n/personal/proj/ideas/deep/x.md");
        assert_eq!(branch_prefix(&nodes, x, &later), "    └─  ");
        assert_eq!(
            branch_prefix(&nodes, x, &later_siblings(&nodes, &all)),
            "│ │ └─  "
        );
    }

    #[test]
    fn every_directory_count_ends_at_the_text_width_at_every_depth() {
        let (sections, nodes) = aligned_forest();
        let dirs: Vec<usize> = (0..nodes.len()).filter(|&i| nodes[i].is_dir).collect();
        assert_eq!(dirs.iter().map(|&i| nodes[i].depth).max(), Some(2));
        for idx in dirs {
            let node = &nodes[idx];
            let line = line_of(&sections, &nodes, idx);
            let path = node.path.display();
            assert_eq!(
                line.width(),
                LIST_TEXT_WIDTH,
                "{path}: {:?}",
                span_texts(&line)
            );
            let count = node.child_count.to_string();
            assert_eq!(line.spans.last().unwrap().content, count, "{path}");
            let cells = render_row(line);
            let last = LIST_TEXT_WIDTH - 1;
            assert_eq!(cells[last].0, count, "{path}");
            assert!(cells[last + 1..].iter().all(|c| c.0 == " "), "{path}");
        }
    }

    #[test]
    fn a_folder_name_with_multi_byte_or_wide_characters_still_aligns_its_count() {
        let (sections, nodes) = aligned_forest();
        for path in ["/n/personal/proj/åäö", "/n/personal/proj/日本語"] {
            let line = line_of(&sections, &nodes, row(&nodes, path));
            assert_eq!(
                line.width(),
                LIST_TEXT_WIDTH,
                "{path}: {:?}",
                span_texts(&line)
            );
        }
    }

    /// A file's blank after the branch is as wide as a folder's expand
    /// mark, so siblings' badges share a column and a file's name starts
    /// two columns after its sibling folder's badge.
    #[test]
    fn a_file_name_starts_two_columns_after_its_sibling_folders_badge() {
        let (sections, nodes) = aligned_forest();
        let folder_idx = row(&nodes, "/n/personal/proj/ideas/deep");
        let file_idx = row(&nodes, "/n/personal/proj/ideas/a.md");
        let folder_line = line_of(&sections, &nodes, folder_idx);
        let file_line = line_of(&sections, &nodes, file_idx);
        let folder_badge = column_of(
            &folder_line,
            name_index(&folder_line, &nodes[folder_idx]) - 1,
        );
        let file_name = column_of(&file_line, name_index(&file_line, &nodes[file_idx]));
        assert_eq!(file_name, folder_badge + 2);
    }

    /// A section row: the gutter, the blank five-slot tag field of an
    /// untagged section and its space, the expand mark at [`TREE_COL`], then
    /// the scope icon, the label and the leader to the count at the text
    /// width; no scope word.
    #[test]
    fn a_section_row_keeps_its_spans() {
        let (sections, nodes) = aligned_forest();
        let line = line_of(&sections, &nodes, 0);
        let dots = |n: usize| format!(" {} ", "·".repeat(n));
        let base = [
            " ",
            " ",
            " ",
            " ",
            " ",
            " ",
            " ",
            "▼ ",
            "\u{f007} ",
            "/n/personal/proj",
        ];
        let mut expected: Vec<String> = base.iter().map(|s| s.to_string()).collect();
        expected.push(dots(32 + 2 + 7 + 9 - TAG_FIELD_WIDTH - 1));
        expected.push("4".to_string());
        assert_eq!(span_texts(&line), expected);
        assert_eq!(
            column_of(&line, 7),
            TREE_COL,
            "the expand mark starts after the tag field"
        );
    }

    #[test]
    fn a_section_count_ends_at_the_last_text_column_of_the_padded_list_pane() {
        use ratatui::buffer::Buffer;
        use ratatui::widgets::BorderType;
        let (sections, nodes) = aligned_forest();
        let pane = Rect::new(0, 0, 80, 3);
        let line = line_at_width(&sections, &nodes, 0, list_text_width(pane.width));
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .padding(Padding::new(1, 1, 0, 0));
        let mut buf = Buffer::empty(pane);
        let list = List::new(list_items(vec![line], Some(0)));
        let mut state = ListState::default();
        state.select(Some(0));
        StatefulWidget::render(list, block.inner(pane), &mut buf, &mut state);
        assert_eq!(buf[(77, 1)].symbol(), "4", "the count is not clipped");
        assert_eq!(buf[(76, 1)].symbol(), " ");
        assert_eq!(
            buf[(2, 1)].symbol(),
            " ",
            "the gutter right after the padding"
        );
        for x in 3..2 + TREE_COL as u16 {
            assert_eq!(
                buf[(x, 1)].symbol(),
                " ",
                "the blank tag field and its space, column {x}"
            );
        }
        assert_eq!(
            buf[(2 + TREE_COL as u16, 1)].symbol(),
            "▼",
            "the expand mark right after the tag field"
        );
    }

    #[test]
    fn the_selected_row_is_shown_by_its_style_across_the_whole_line_without_a_symbol() {
        let (sections, nodes) = aligned_forest();
        let plain = span_texts(&line_of(&sections, &nodes, 0)).concat();
        let cells = render_row_styled(line_of(&sections, &nodes, 0));
        let drawn: String = cells.iter().map(|c| c.0.as_str()).collect();
        assert!(
            drawn.starts_with(&plain),
            "no symbol shifts the row: {drawn:?}"
        );
        for (x, (_, style)) in cells.iter().enumerate() {
            assert_eq!(style.bg, theme::selected_row().bg, "column {x}");
            assert!(style.add_modifier.contains(Modifier::BOLD), "column {x}");
        }
    }

    // --- Compact tag field ---

    /// [`aligned_forest`] with three tags on `ideas/a.md` and one on
    /// `åäö/b.md`; the other rows have none.
    fn tagged_forest() -> (Vec<SectionSpec>, Vec<TreeNode>) {
        let (sections, mut nodes) = aligned_forest();
        let a = row(&nodes, "/n/personal/proj/ideas/a.md");
        nodes[a].flags = FLAG_IMPORTANT | FLAG_LONGTERM | FLAG_BLOCKED;
        let b = row(&nodes, "/n/personal/proj/åäö/b.md");
        nodes[b].flags = FLAG_IDEA;
        (sections, nodes)
    }

    /// The tag field of `path`'s row: the spans between the gutter and the
    /// branch drawing, text and colour.
    fn field_of(
        sections: &[SectionSpec],
        nodes: &[TreeNode],
        path: &str,
    ) -> Vec<(String, Option<Color>)> {
        let idx = row(nodes, path);
        let line = line_of(sections, nodes, idx);
        let branch = line
            .spans
            .iter()
            .position(|s| s.style.fg == Some(theme::SURFACE))
            .expect("branch");
        line.spans[1..branch]
            .iter()
            .map(|s| (s.content.to_string(), s.style.fg))
            .collect()
    }

    /// The flags with the tags at `indices` into [`FLAG_DEFS`] set.
    fn flags_of(indices: &[usize]) -> u8 {
        indices.iter().fold(0, |flags, &i| flags | FLAG_DEFS[i].bit)
    }

    #[test]
    fn the_tag_field_is_five_dot_slots_in_tag_order_and_the_tree_never_moves() {
        let (sections, mut nodes) = aligned_forest();
        let c = row(&nodes, "/n/personal/proj/日本語/c.md");
        let cases: Vec<Vec<usize>> = vec![
            vec![],
            vec![0],
            vec![1],
            vec![3],
            vec![4],
            vec![0, 1],
            vec![0, 2, 4],
            vec![0, 1, 2, 3, 4],
        ];
        let untagged_tail: Vec<String> = render_cells(line_of(&sections, &nodes, c), false)
            [TREE_COL - 1..]
            .iter()
            .map(|c| c.0.clone())
            .collect();
        for tags in cases {
            nodes[c].flags = flags_of(&tags);
            for selected in [false, true] {
                let row_bg = if selected {
                    theme::selected_row().bg.unwrap()
                } else {
                    Color::Reset
                };
                let cells = render_cells(line_of(&sections, &nodes, c), selected);
                for slot in 0..TAG_FIELD_WIDTH {
                    let (symbol, style) = &cells[GUTTER_COL + 1 + slot];
                    let label = format!("tags {tags:?}, selected {selected}, slot {slot}");
                    let (glyph, fg) = if tags.contains(&slot) {
                        ("●", theme::FLAG_COLORS[slot])
                    } else {
                        ("·", theme::OVERLAY)
                    };
                    assert_eq!(symbol, glyph, "{label}");
                    assert_eq!(style.fg, Some(fg), "{label}");
                    assert_eq!(style.bg, Some(row_bg), "{label}: the row's own background");
                }
                assert_eq!(
                    cells[GUTTER_COL].1.bg,
                    Some(row_bg),
                    "tags {tags:?}: the gutter"
                );
                let tail: Vec<String> = cells[TREE_COL - 1..].iter().map(|c| c.0.clone()).collect();
                assert_eq!(
                    tail, untagged_tail,
                    "tags {tags:?}: the space, tree and name stay put"
                );
                for (x, (_, style)) in cells.iter().enumerate().skip(TREE_COL - 1) {
                    assert_eq!(
                        style.bg,
                        Some(row_bg),
                        "tags {tags:?}, selected {selected}, column {x}"
                    );
                }
            }
        }
        // A section row and a folder row start their tree at the same column
        // and show the tags derived from their notes as dots, and the
        // folder's count still ends at the text width.
        nodes[c].flags = flags_of(&[0, 1, 2, 3, 4]);
        derive_dir_flags(&mut nodes);
        let section = render_row(line_of(&sections, &nodes, 0));
        assert_eq!(section[TREE_COL].0, "▼");
        assert_eq!(section[GUTTER_COL].0, " ", "{section:?}");
        assert_eq!(section[TREE_COL - 1].0, " ", "{section:?}");
        for slot in 0..TAG_FIELD_WIDTH {
            let cell = &section[GUTTER_COL + 1 + slot];
            assert_eq!(
                cell,
                &("●".to_string(), Some(theme::FLAG_COLORS[slot])),
                "section slot {slot}"
            );
        }
        for idx in (0..nodes.len()).filter(|&i| nodes[i].is_dir) {
            let line = line_of(&sections, &nodes, idx);
            assert_eq!(line.width(), LIST_TEXT_WIDTH, "{:?}", span_texts(&line));
        }
    }

    #[test]
    fn the_tag_field_keeps_its_five_slots_when_tagged_rows_fold_away() {
        let (sections, mut nodes) = tagged_forest();
        // Folders carry their notes' tags, as the event loop derives them,
        // and draw them as dots in the same field.
        derive_dir_flags(&mut nodes);
        let set = |i: usize| ("●".to_string(), Some(theme::FLAG_COLORS[i]));
        let unset = || ("·".to_string(), Some(theme::OVERLAY));
        let blank = || (" ".to_string(), None);
        let untagged = vec![unset(), unset(), unset(), unset(), unset(), blank()];
        assert_eq!(
            field_of(&sections, &nodes, "/n/personal/proj/ideas/a.md"),
            vec![set(0), unset(), set(2), unset(), set(4), blank()],
            "three tags, each in its own slot"
        );
        assert_eq!(
            field_of(&sections, &nodes, "/n/personal/proj/åäö/b.md"),
            vec![unset(), unset(), unset(), set(3), unset(), blank()],
            "one tag in its slot"
        );
        assert_eq!(
            field_of(&sections, &nodes, "/n/personal/proj/日本語/c.md"),
            untagged,
            "no tag"
        );
        let all_three = vec![set(0), unset(), set(2), set(3), set(4), blank()];
        assert_eq!(
            field_of(&sections, &nodes, "/n/personal/proj"),
            all_three,
            "the section row: both notes' tags"
        );
        let ideas = vec![set(0), unset(), set(2), unset(), set(4), blank()];
        assert_eq!(
            field_of(&sections, &nodes, "/n/personal/proj/ideas"),
            ideas,
            "a folder row: a.md's tags"
        );
        let only_b = vec![unset(), unset(), unset(), set(3), unset(), blank()];
        assert_eq!(
            field_of(&sections, &nodes, "/n/personal/proj/åäö"),
            only_b,
            "a folder row: b.md's tag"
        );
        let blank_field = vec![blank(), blank(), blank(), blank(), blank(), blank()];
        assert_eq!(
            field_of(&sections, &nodes, "/n/personal/proj/日本語"),
            blank_field,
            "a folder of untagged notes"
        );
        for idx in (0..nodes.len()).filter(|&i| nodes[i].is_dir) {
            let line = line_of(&sections, &nodes, idx);
            assert_eq!(line.width(), LIST_TEXT_WIDTH, "{:?}", span_texts(&line));
        }

        // Folding both tagged rows away leaves every field as it was, and
        // a folded folder still shows the tags inside it.
        for folder in ["/n/personal/proj/ideas", "/n/personal/proj/åäö"] {
            let idx = row(&nodes, folder);
            nodes[idx].expanded = false;
            assert_eq!(
                field_of(&sections, &nodes, "/n/personal/proj/日本語/c.md"),
                untagged,
                "{folder} folded"
            );
        }
        assert_eq!(
            field_of(&sections, &nodes, "/n/personal/proj/ideas"),
            ideas,
            "ideas folded"
        );
        assert_eq!(
            field_of(&sections, &nodes, "/n/personal/proj/åäö"),
            only_b,
            "åäö folded"
        );
        assert_eq!(
            field_of(&sections, &nodes, "/n/personal/proj"),
            all_three,
            "the section, both folded"
        );
        for idx in (0..nodes.len())
            .filter(|&i| nodes[i].is_dir && compute_visible(&nodes, "").contains(&i))
        {
            let line = line_of(&sections, &nodes, idx);
            assert_eq!(line.width(), LIST_TEXT_WIDTH, "{:?}", span_texts(&line));
        }
    }

    #[test]
    fn a_tagless_forest_draws_unset_dots_on_notes_a_blank_field_on_folders_and_the_section_mark_follows(
    ) {
        let (sections, mut nodes) = aligned_forest();
        derive_dir_flags(&mut nodes);
        assert!(nodes.iter().all(|n| n.flags == 0));
        for idx in 0..nodes.len() {
            let line = line_of(&sections, &nodes, idx);
            assert_eq!(line.spans[0].content, " ", "the gutter");
            let slot = if nodes[idx].is_dir { " " } else { "·" };
            let field: Vec<&str> = line.spans[1..7]
                .iter()
                .map(|s| s.content.as_ref())
                .collect();
            assert_eq!(
                field,
                [slot, slot, slot, slot, slot, " "],
                "the field and its space: {:?}",
                span_texts(&line)
            );
            // Below depth 1 a blank ancestor level may lead the tree drawing.
            if nodes[idx].depth <= 1 {
                let first = &line.spans[7];
                assert!(
                    !first.content.starts_with(' '),
                    "nothing between field and tree: {:?}",
                    span_texts(&line)
                );
            }
            let after_field: String = line.spans[7..name_index(&line, &nodes[idx])]
                .iter()
                .map(|s| s.content.as_ref())
                .collect();
            assert!(!after_field.contains(['·', '●']), "{after_field:?}");
        }
        assert_eq!(line_of(&sections, &nodes, 0).spans[7].content, "▼ ");
    }

    #[test]
    fn a_click_maps_each_field_column_to_its_slot_and_nowhere_else() {
        for area_x in [0u16, 10] {
            assert_eq!(
                mouse_x_to_field_slot(area_x, area_x),
                None,
                "the gutter at {area_x}"
            );
            for slot in 0..TAG_FIELD_WIDTH {
                let col = area_x + 1 + slot as u16;
                assert_eq!(
                    mouse_x_to_field_slot(col, area_x),
                    Some(slot),
                    "column {col} of a list at {area_x}"
                );
            }
            for col in area_x + 6..area_x + 10 {
                assert_eq!(
                    mouse_x_to_field_slot(col, area_x),
                    None,
                    "column {col}: past the field"
                );
            }
        }
        assert_eq!(mouse_x_to_field_slot(9, 10), None, "left of the list");
        assert_eq!(
            mouse_x_to_field_slot(u16::MAX, u16::MAX),
            None,
            "no overflow at the edge"
        );
    }

    #[test]
    fn a_field_click_picks_its_slots_tag_only_on_a_note_row_with_no_input_open() {
        for slot in 0..TAG_FIELD_WIDTH {
            let col = 1 + slot as u16;
            assert_eq!(
                field_click_tag(col, 0, false, false),
                Some(slot),
                "slot {slot} on a note row"
            );
            assert_eq!(
                field_click_tag(col, 0, false, true),
                None,
                "slot {slot}: an input is open"
            );
            assert_eq!(
                field_click_tag(col, 0, true, false),
                None,
                "slot {slot}: a folder row"
            );
        }
        assert_eq!(field_click_tag(0, 0, false, false), None, "the gutter");
        assert_eq!(
            field_click_tag(6, 0, false, false),
            None,
            "the space after the field"
        );
    }

    /// Clicks at a row's drawn columns, applied as the event loop applies
    /// them: each one toggles exactly the tag whose slot is under it.
    #[test]
    fn a_click_on_a_drawn_slot_toggles_exactly_that_tag_and_nothing_on_a_folder() {
        let (sections, mut nodes) = tagged_forest();
        let a = row(&nodes, "/n/personal/proj/ideas/a.md");
        let before = nodes[a].flags;
        for slot in 0..TAG_FIELD_WIDTH {
            let cells = render_row(line_of(&sections, &nodes, a));
            let col = (GUTTER_COL + 1 + slot) as u16;
            assert!(
                ["●", "·"].contains(&cells[col as usize].0.as_str()),
                "slot {slot}: {:?}",
                cells[col as usize]
            );
            let tag = field_click_tag(col, 0, nodes[a].is_dir, false).expect("a note's slot");
            nodes[a].flags ^= FLAG_DEFS[tag].bit;
            assert_eq!(
                nodes[a].flags ^ before,
                flags_of(&(0..=slot).collect::<Vec<_>>()),
                "slot {slot}"
            );
            let after = render_row(line_of(&sections, &nodes, a));
            for other in (0..TAG_FIELD_WIDTH).filter(|&k| k != slot) {
                let x = GUTTER_COL + 1 + other;
                assert_eq!(after[x], cells[x], "slot {slot} left slot {other} alone");
            }
            assert_ne!(
                after[col as usize].0, cells[col as usize].0,
                "slot {slot} flipped"
            );
        }
        // A folder's derived dots are drawn but take no click: every column
        // of its field is a plain row click and its tags stay as they were.
        derive_dir_flags(&mut nodes);
        let folder = row(&nodes, "/n/personal/proj/ideas");
        let derived = nodes[folder].flags;
        assert_ne!(derived, 0);
        let cells = render_row(line_of(&sections, &nodes, folder));
        assert!(
            cells[GUTTER_COL + 1..TREE_COL - 1]
                .iter()
                .any(|c| c.0 == "●"),
            "{cells:?}"
        );
        for col in 0..10u16 {
            assert_eq!(
                field_click_tag(col, 0, nodes[folder].is_dir, false),
                None,
                "folder column {col}"
            );
        }
        assert_eq!(nodes[folder].flags, derived);
    }

    #[test]
    fn a_marked_row_with_tags_keeps_the_mark_in_the_gutter_before_its_field() {
        let (sections, nodes) = tagged_forest();
        let a = row(&nodes, "/n/personal/proj/ideas/a.md");
        let marked = render_row(mark_row(line_of(&sections, &nodes, a)));
        let texts: Vec<&str> = marked[..8].iter().map(|c| c.0.as_str()).collect();
        assert_eq!(
            texts,
            ["▎", "●", "·", "●", "·", "●", " ", "│"],
            "the mark sits before the dots"
        );
        for (col, tag) in [(1, 0), (3, 2), (5, 4)] {
            assert_eq!(marked[col].1, Some(theme::FLAG_COLORS[tag]), "column {col}");
        }
    }
}
