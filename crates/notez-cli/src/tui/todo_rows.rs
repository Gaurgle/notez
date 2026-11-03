//! The todoz row drawing, shared by the board (`tui::todo`) and the tree
//! browser's preview of a todo file: tag dots, tree drawing, checkbox and
//! text, wrapped to the list width. Pure functions of the items; the board
//! adds its hover and filter inputs, the preview passes none.

use std::collections::HashSet;

use ratatui::prelude::*;

use notez_core::filter;
use notez_core::tags::FLAG_DEFS;
use notez_core::todo::{CheckState, Task};
use notez_core::util::tilde;

use super::{text, theme};

/// Render the 5 fixed tag-dot slots: the row's one-column gutter, the dots,
/// and one space before the tree drawing. `hover_dot` previews an unset slot in its tag color while the mouse
/// hovers over it.
pub(super) fn flags_slots_with_hover(flags: u8, hover_dot: Option<u8>) -> Vec<Span<'static>> {
    let mut spans: Vec<Span<'static>> = vec![Span::raw(" ")];
    for (i, def) in FLAG_DEFS.iter().enumerate() {
        let is_set = flags & def.bit != 0;
        let is_hovered = hover_dot == Some(i as u8);
        if is_set {
            spans.push(Span::styled(
                "●",
                Style::default().fg(theme::FLAG_COLORS[i]),
            ));
        } else if is_hovered {
            spans.push(Span::styled(
                "●",
                Style::default()
                    .fg(theme::FLAG_COLORS[i])
                    .add_modifier(Modifier::DIM),
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

pub(super) fn flags_slots(flags: u8) -> Vec<Span<'static>> {
    flags_slots_with_hover(flags, None)
}

/// The row a todo hangs under: the nearest earlier todo one level up, or
/// for a top-level todo its section header. `None` for a header, or a todo
/// with nothing above it.
pub(super) fn parent_of(items: &[Task], idx: usize) -> Option<usize> {
    if items[idx].is_header {
        return None;
    }
    let depth = items[idx].depth;
    (0..idx)
        .rev()
        .find(|&j| items[j].is_header || items[j].depth < depth)
}

/// For each row, whether a later row of `visible` shares its parent: the
/// row draws `├─` and its descendants a bar at its level. Rows outside
/// `visible` (collapsed or filtered away) neither get one nor count as a
/// later sibling. Indexed like `items`. Mirrors the tree browser's.
pub(super) fn later_siblings(items: &[Task], visible: &[usize]) -> Vec<bool> {
    let mut later = vec![false; items.len()];
    let mut seen: HashSet<Option<usize>> = HashSet::new();
    for &i in visible.iter().rev() {
        if !items[i].is_header {
            later[i] = !seen.insert(parent_of(items, i));
        }
    }
    later
}

/// The ancestor columns of todo `idx`: for each todo above it, a bar if
/// that ancestor has a later sibling and a blank otherwise, outermost first.
fn ancestor_levels(items: &[Task], idx: usize, later: &[bool]) -> String {
    let g = &theme::TREE_GLYPHS;
    let mut levels = Vec::new();
    let mut up = parent_of(items, idx);
    while let Some(p) = up.filter(|&p| !items[p].is_header) {
        levels.push(if later[p] { g.ancestor_bar } else { g.ancestor_blank });
        up = parent_of(items, p);
    }
    levels.reverse();
    levels.concat()
}

/// The tree drawing before row `idx`'s checkbox, from
/// [`theme::TREE_GLYPHS`], as the tree browser draws it: a section header
/// shows its expand mark; a todo shows its ancestor columns, its own branch
/// (`├─`, or `└─` when it is the last child), then a parent's fold mark or
/// a blank. `later` comes from [`later_siblings`].
pub(super) fn branch_prefix(items: &[Task], idx: usize, later: &[bool]) -> String {
    let g = &theme::TREE_GLYPHS;
    let item = &items[idx];
    if item.is_header {
        let mark = if item.collapsed { g.section_closed } else { g.section_open };
        return mark.to_string();
    }
    let mut prefix = ancestor_levels(items, idx, later);
    prefix.push_str(if later[idx] { g.branch } else { g.last_branch });
    prefix.push_str(match (item.has_subtasks, item.collapsed) {
        (true, true) => g.folder_closed,
        (true, false) => g.folder_open,
        (false, _) => g.file,
    });
    prefix
}

/// The tree drawing on a wrapped todo's continuation lines: the branch
/// lines that pass the row keep going, its own included when it has a
/// later sibling, and a bar under the fold mark when its first child
/// follows it (`has_visible_child`).
fn continuation_prefix(items: &[Task], idx: usize, later: &[bool], has_visible_child: bool) -> String {
    let g = &theme::TREE_GLYPHS;
    let mut prefix = ancestor_levels(items, idx, later);
    prefix.push_str(if later[idx] { g.ancestor_bar } else { g.ancestor_blank });
    prefix.push_str(if has_visible_child { g.ancestor_bar } else { g.ancestor_blank });
    prefix
}

/// The extra columns a wrapped todo's continuation lines are indented past
/// its text, so a wrap reads as a continuation.
const WRAP_INDENT: usize = 2;

/// The display columns of every row's dot field: gutter, dots and space.
fn dot_field_cols() -> usize {
    flags_slots(0).iter().map(Span::width).sum()
}

/// Every row of `visible` drawn for a list `list_width` columns wide:
/// the dot field (with the hover preview of `hover_flag`), the tree
/// drawing in the tree's dim colour, then the header label, or the
/// checkbox and text of a todo, wrapping long todos [`WRAP_INDENT`]
/// columns past their text.
/// The characters of each todo's text that the text tokens of the filter
/// `search_buffer` matched are in [`theme::filter_match`], on every line of
/// a wrapped todo; section headers are never lit. With no hover and an
/// empty filter, nothing is highlighted.
pub(super) fn rows(
    items: &[Task],
    visible: &[usize],
    hover_flag: Option<(usize, u8)>,
    list_width: u16,
    search_buffer: &str,
) -> Vec<Text<'static>> {
    let later = later_siblings(items, visible);
    let branch_style = Style::default().fg(theme::SURFACE);
    let f = filter::parse(search_buffer);
    visible
        .iter()
        .enumerate()
        .map(|(pos, &idx)| {
            let item = &items[idx];
            let branch = branch_prefix(items, idx, &later);
            let matched = if item.is_header {
                Vec::new()
            } else {
                text::filter_match_positions(&f, &item.text)
            };
            let lit = |piece: &str, start: usize, style: Style| {
                text::highlight_spans(piece, start, &matched, style, theme::filter_match())
            };
            if item.is_header {
                let path_display = item
                    .source
                    .canonicalize()
                    .unwrap_or_else(|_| item.source.clone())
                    .parent()
                    .map(tilde::contract)
                    .unwrap_or_default();
                let header_color = if item.is_code_todo { theme::YELLOW } else { theme::MAUVE };
                let mut spans = flags_slots(item.flags);
                spans.push(Span::styled(branch, branch_style));
                spans.push(Span::styled(
                    format!("{} ", item.text),
                    Style::default().fg(header_color).add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled(path_display, Style::default().fg(theme::OVERLAY)));
                return Text::from(Line::from(spans));
            }
            if item.is_code_todo {
                let mut spans = flags_slots(0);
                spans.push(Span::styled(branch, branch_style));
                spans.extend(lit(&item.text, 0, Style::default().fg(theme::OVERLAY)));
                return Text::from(Line::from(spans));
            }
            let (mark, mark_color, bracket_color, style) = match item.state {
                CheckState::Checked => (
                    "x",
                    theme::SAPPHIRE,
                    theme::OVERLAY,
                    Style::default().fg(theme::OVERLAY),
                ),
                CheckState::Half => (
                    "/",
                    theme::YELLOW,
                    theme::OVERLAY,
                    Style::default().fg(theme::SUBTEXT),
                ),
                CheckState::Unchecked => (
                    " ",
                    theme::SURFACE,
                    match item.depth {
                        0 => theme::SAPPHIRE,
                        1 => Color::Rgb(86, 169, 206),
                        _ => Color::Rgb(56, 139, 176),
                    },
                    Style::default().fg(theme::TEXT),
                ),
            };
            const CHECKBOX_COLS: usize = 4;
            let prefix_len = dot_field_cols() + Span::raw(branch.as_str()).width() + CHECKBOX_COLS;
            let text_width = (list_width as usize).saturating_sub(prefix_len + 4);
            let hover_dot = hover_flag.and_then(|(hi, d)| (hi == idx).then_some(d));
            let mut first_line = flags_slots_with_hover(item.flags, hover_dot);
            first_line.push(Span::styled(branch, branch_style));
            first_line.push(Span::styled("[", Style::default().fg(bracket_color)));
            first_line.push(Span::styled(mark, Style::default().fg(mark_color)));
            first_line.push(Span::styled("] ", Style::default().fg(bracket_color)));

            if text_width == 0 || item.text.chars().count() <= text_width {
                first_line.extend(lit(&item.text, 0, style));
                return Text::from(Line::from(first_line));
            }
            let has_visible_child = visible
                .get(pos + 1)
                .is_some_and(|&next| parent_of(items, next) == Some(idx));
            let cont = continuation_prefix(items, idx, &later, has_visible_child);
            let field_blank = " ".repeat(dot_field_cols());
            let checkbox_blank = " ".repeat(CHECKBOX_COLS + WRAP_INDENT);
            // Wrap on char boundaries; slicing mid-char (å, ö, icons) would
            // panic on tiny widths.
            let mut lines = vec![];
            let mut remaining = item.text.as_str();
            let mut first = Some(first_line);
            while !remaining.is_empty() {
                let chunk_width = if first.is_some() {
                    text_width
                } else {
                    text_width.saturating_sub(WRAP_INDENT).max(1)
                };
                let split_at = remaining
                    .char_indices()
                    .nth(chunk_width)
                    .map(|(i, _)| i)
                    .unwrap_or(remaining.len());
                let split_at = if split_at < remaining.len() {
                    remaining[..split_at].rfind(' ').map(|i| i + 1).unwrap_or(split_at)
                } else {
                    split_at
                };
                let (chunk, rest) = remaining.split_at(split_at);
                // `remaining` is always a suffix of the text.
                let chunk = lit(chunk, item.text.len() - remaining.len(), style);
                match first.take() {
                    Some(mut spans) => {
                        spans.extend(chunk);
                        lines.push(Line::from(spans));
                    }
                    None => {
                        let mut spans = vec![
                            Span::raw(field_blank.clone()),
                            Span::styled(cont.clone(), branch_style),
                            Span::raw(checkbox_blank.clone()),
                        ];
                        spans.extend(chunk);
                        lines.push(Line::from(spans));
                    }
                }
                remaining = rest.trim_start();
            }
            Text::from(lines)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use notez_core::tags::{FLAG_BLOCKED, FLAG_IMPORTANT, FLAG_PRIO};
    use std::path::PathBuf;

    fn task(text: &str, depth: u8, flags: u8, state: CheckState) -> Task {
        Task {
            text: text.to_string(),
            state,
            source: PathBuf::from("/nz51/none/TODO.md"),
            section: "s".to_string(),
            is_header: false,
            depth,
            has_subtasks: false,
            collapsed: false,
            is_code_todo: false,
            flags,
            due: None,
        }
    }

    /// A board with every kind of row: a section header with a tag, a
    /// tagged parent, a half-done child, done and open grandchildren, a
    /// long todo with a child that wraps, a collapsed parent hiding a child,
    /// a done top-level todo, and a code section with a code todo.
    fn every_kind_of_row() -> (Vec<Task>, Vec<usize>) {
        let mut items = vec![
            Task { is_header: true, ..task("WORK", 0, FLAG_PRIO, CheckState::Unchecked) },
            task("parent flag", 0, FLAG_IMPORTANT | FLAG_BLOCKED, CheckState::Unchecked),
            task("half child", 1, 0, CheckState::Half),
            task("done grandchild", 2, FLAG_PRIO, CheckState::Checked),
            task("open grandchild", 2, 0, CheckState::Unchecked),
            task(
                "a long todo that wraps over several lines of the fleet with åäö ünïcode and more words",
                1,
                0,
                CheckState::Unchecked,
            ),
            task("child of the long one", 2, 0, CheckState::Unchecked),
            task("collapsed parent", 0, 0, CheckState::Unchecked),
            task("hidden kid", 1, 0, CheckState::Unchecked),
            task("last top flz", 0, 0, CheckState::Checked),
            Task { is_header: true, is_code_todo: true, ..task("CODE", 0, 0, CheckState::Unchecked) },
            Task { is_code_todo: true, ..task("src/x.rs:3 fix the fleet", 0, 0, CheckState::Unchecked) },
        ];
        for parent in [1, 2, 5, 7] {
            items[parent].has_subtasks = true;
        }
        items[7].collapsed = true;
        let visible = vec![0, 1, 2, 3, 4, 5, 6, 7, 9, 10, 11];
        (items, visible)
    }

    /// One text line per drawn line, prefixed by its row: each span as its
    /// quoted text, then its foreground, background and modifiers when set.
    fn serialize(rows: &[Text<'static>]) -> String {
        let colour = |c: Option<Color>| match c {
            None => String::new(),
            Some(Color::Rgb(r, g, b)) => format!("#{r:02x}{g:02x}{b:02x}"),
            Some(other) => format!("{other:?}"),
        };
        let mut out = String::new();
        for (i, row) in rows.iter().enumerate() {
            for line in &row.lines {
                out.push_str(&format!("{i}:"));
                for s in &line.spans {
                    out.push_str(&format!(" {:?}{}", s.content, colour(s.style.fg)));
                    if s.style.bg.is_some() {
                        out.push_str(&format!(" bg{}", colour(s.style.bg)));
                    }
                    if !s.style.add_modifier.is_empty() {
                        out.push_str(&format!("+{:?}", s.style.add_modifier));
                    }
                }
                out.push('\n');
            }
        }
        out
    }

    fn plain(row: &Text<'static>) -> Vec<String> {
        row.lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect()
    }

    fn span<'a>(row: &'a Text<'static>, content: &str) -> &'a Span<'static> {
        row.lines[0]
            .spans
            .iter()
            .find(|s| s.content == content)
            .unwrap_or_else(|| panic!("no span {content:?} in {row:?}"))
    }

    #[test]
    fn the_board_rows_are_the_same_as_before_the_move() {
        let (items, visible) = every_kind_of_row();
        let mut out = serialize(&rows(&items, &visible, None, 60, ""));
        out.push_str(&serialize(&rows(&items, &visible, Some((1, 3)), 34, "flt")));
        out.push_str(&serialize(&rows(&items, &visible, Some((4, 0)), 3, "")));
        assert_eq!(out, BOARD_ROWS_BEFORE_THE_MOVE);
    }

    /// The board's rows for [`every_kind_of_row`] as `todo.rs` drew them
    /// before the row drawing moved here (NZ-51): at width 60 with no hover
    /// or filter, at 34 with a hover dot and the filter `flt`, and at 3,
    /// too narrow to wrap, with a hover dot on an open grandchild. Since
    /// NZ-58 the continuation lines of wrapped todos are indented
    /// [`WRAP_INDENT`] more and wrapped 2 columns narrower; every other line
    /// is as before.
    const BOARD_ROWS_BEFORE_THE_MOVE: &str = r##"0: " " "·"#323241 "●"#fab387 "·"#323241 "·"#323241 "·"#323241 " " "▼ "#45475a "WORK "#cba6f7+BOLD "/nz51/none"#6c7086
1: " " "●"#f38ba8 "·"#323241 "·"#323241 "·"#323241 "●"#cba6f7 " " "├─▾ "#45475a "["#74c7ec " "#45475a "] "#74c7ec "parent flag"#cdd6f4
2: " " "·"#323241 "·"#323241 "·"#323241 "·"#323241 "·"#323241 " " "│ ├─▾ "#45475a "["#6c7086 "/"#f9e2af "] "#6c7086 "half child"#a6adc8
3: " " "·"#323241 "●"#fab387 "·"#323241 "·"#323241 "·"#323241 " " "│ │ ├─  "#45475a "["#6c7086 "x"#74c7ec "] "#6c7086 "done grandchild"#6c7086
4: " " "·"#323241 "·"#323241 "·"#323241 "·"#323241 "·"#323241 " " "│ │ └─  "#45475a "["#388bb0 " "#45475a "] "#388bb0 "open grandchild"#cdd6f4
5: " " "·"#323241 "·"#323241 "·"#323241 "·"#323241 "·"#323241 " " "│ └─▾ "#45475a "["#56a9ce " "#45475a "] "#56a9ce "a long todo that wraps over several "#cdd6f4
5: "       " "│   │ "#45475a "      " "lines of the fleet with åäö ünïcode "#cdd6f4
5: "       " "│   │ "#45475a "      " "and more words"#cdd6f4
6: " " "·"#323241 "·"#323241 "·"#323241 "·"#323241 "·"#323241 " " "│   └─  "#45475a "["#388bb0 " "#45475a "] "#388bb0 "child of the long one"#cdd6f4
7: " " "·"#323241 "·"#323241 "·"#323241 "·"#323241 "·"#323241 " " "├─▸ "#45475a "["#74c7ec " "#45475a "] "#74c7ec "collapsed parent"#cdd6f4
8: " " "·"#323241 "·"#323241 "·"#323241 "·"#323241 "·"#323241 " " "└─  "#45475a "["#6c7086 "x"#74c7ec "] "#6c7086 "last top flz"#6c7086
9: " " "·"#323241 "·"#323241 "·"#323241 "·"#323241 "·"#323241 " " "▼ "#45475a "CODE "#f9e2af+BOLD "/nz51/none"#6c7086
10: " " "·"#323241 "·"#323241 "·"#323241 "·"#323241 "·"#323241 " " "└─  "#45475a "src/x.rs:3 fix the fleet"#6c7086
0: " " "·"#323241 "●"#fab387 "·"#323241 "·"#323241 "·"#323241 " " "▼ "#45475a "WORK "#cba6f7+BOLD "/nz51/none"#6c7086
1: " " "●"#f38ba8 "·"#323241 "·"#323241 "●"#74c7ec+DIM "●"#cba6f7 " " "├─▾ "#45475a "["#74c7ec " "#45475a "] "#74c7ec "parent flag"#cdd6f4
2: " " "·"#323241 "·"#323241 "·"#323241 "·"#323241 "·"#323241 " " "│ ├─▾ "#45475a "["#6c7086 "/"#f9e2af "] "#6c7086 "half child"#a6adc8
3: " " "·"#323241 "●"#fab387 "·"#323241 "·"#323241 "·"#323241 " " "│ │ ├─  "#45475a "["#6c7086 "x"#74c7ec "] "#6c7086 "done "#6c7086
3: "       " "│ │ │   "#45475a "      " "grandchil"#6c7086
3: "       " "│ │ │   "#45475a "      " "d"#6c7086
4: " " "·"#323241 "·"#323241 "·"#323241 "·"#323241 "·"#323241 " " "│ │ └─  "#45475a "["#388bb0 " "#45475a "] "#388bb0 "open "#cdd6f4
4: "       " "│ │     "#45475a "      " "grandchil"#cdd6f4
4: "       " "│ │     "#45475a "      " "d"#cdd6f4
5: " " "·"#323241 "·"#323241 "·"#323241 "·"#323241 "·"#323241 " " "│ └─▾ "#45475a "["#56a9ce " "#45475a "] "#56a9ce "a long todo "#cdd6f4
5: "       " "│   │ "#45475a "      " "that wraps "#cdd6f4
5: "       " "│   │ "#45475a "      " "over "#cdd6f4
5: "       " "│   │ "#45475a "      " "several "#cdd6f4
5: "       " "│   │ "#45475a "      " "lines o"#cdd6f4 "f"#f9e2af+BOLD " "#cdd6f4
5: "       " "│   │ "#45475a "      " "the f"#cdd6f4 "l"#f9e2af+BOLD "ee"#cdd6f4 "t"#f9e2af+BOLD " "#cdd6f4
5: "       " "│   │ "#45475a "      " "with åäö "#cdd6f4
5: "       " "│   │ "#45475a "      " "ünïcode "#cdd6f4
5: "       " "│   │ "#45475a "      " "and more "#cdd6f4
5: "       " "│   │ "#45475a "      " "words"#cdd6f4
6: " " "·"#323241 "·"#323241 "·"#323241 "·"#323241 "·"#323241 " " "│   └─  "#45475a "["#388bb0 " "#45475a "] "#388bb0 "child of "#cdd6f4
6: "       " "│       "#45475a "      " "the long "#cdd6f4
6: "       " "│       "#45475a "      " "one"#cdd6f4
7: " " "·"#323241 "·"#323241 "·"#323241 "·"#323241 "·"#323241 " " "├─▸ "#45475a "["#74c7ec " "#45475a "] "#74c7ec "collapsed "#cdd6f4
7: "       " "│   "#45475a "      " "parent"#cdd6f4
8: " " "·"#323241 "·"#323241 "·"#323241 "·"#323241 "·"#323241 " " "└─  "#45475a "["#6c7086 "x"#74c7ec "] "#6c7086 "last top flz"#6c7086
9: " " "·"#323241 "·"#323241 "·"#323241 "·"#323241 "·"#323241 " " "▼ "#45475a "CODE "#f9e2af+BOLD "/nz51/none"#6c7086
10: " " "·"#323241 "·"#323241 "·"#323241 "·"#323241 "·"#323241 " " "└─  "#45475a "src/x.rs:3 "#6c7086 "f"#f9e2af+BOLD "ix the f"#6c7086 "l"#f9e2af+BOLD "ee"#6c7086 "t"#f9e2af+BOLD
0: " " "·"#323241 "●"#fab387 "·"#323241 "·"#323241 "·"#323241 " " "▼ "#45475a "WORK "#cba6f7+BOLD "/nz51/none"#6c7086
1: " " "●"#f38ba8 "·"#323241 "·"#323241 "·"#323241 "●"#cba6f7 " " "├─▾ "#45475a "["#74c7ec " "#45475a "] "#74c7ec "parent flag"#cdd6f4
2: " " "·"#323241 "·"#323241 "·"#323241 "·"#323241 "·"#323241 " " "│ ├─▾ "#45475a "["#6c7086 "/"#f9e2af "] "#6c7086 "half child"#a6adc8
3: " " "·"#323241 "●"#fab387 "·"#323241 "·"#323241 "·"#323241 " " "│ │ ├─  "#45475a "["#6c7086 "x"#74c7ec "] "#6c7086 "done grandchild"#6c7086
4: " " "●"#f38ba8+DIM "·"#323241 "·"#323241 "·"#323241 "·"#323241 " " "│ │ └─  "#45475a "["#388bb0 " "#45475a "] "#388bb0 "open grandchild"#cdd6f4
5: " " "·"#323241 "·"#323241 "·"#323241 "·"#323241 "·"#323241 " " "│ └─▾ "#45475a "["#56a9ce " "#45475a "] "#56a9ce "a long todo that wraps over several lines of the fleet with åäö ünïcode and more words"#cdd6f4
6: " " "·"#323241 "·"#323241 "·"#323241 "·"#323241 "·"#323241 " " "│   └─  "#45475a "["#388bb0 " "#45475a "] "#388bb0 "child of the long one"#cdd6f4
7: " " "·"#323241 "·"#323241 "·"#323241 "·"#323241 "·"#323241 " " "├─▸ "#45475a "["#74c7ec " "#45475a "] "#74c7ec "collapsed parent"#cdd6f4
8: " " "·"#323241 "·"#323241 "·"#323241 "·"#323241 "·"#323241 " " "└─  "#45475a "["#6c7086 "x"#74c7ec "] "#6c7086 "last top flz"#6c7086
9: " " "·"#323241 "·"#323241 "·"#323241 "·"#323241 "·"#323241 " " "▼ "#45475a "CODE "#f9e2af+BOLD "/nz51/none"#6c7086
10: " " "·"#323241 "·"#323241 "·"#323241 "·"#323241 "·"#323241 " " "└─  "#45475a "src/x.rs:3 fix the fleet"#6c7086
"##;

    #[test]
    fn each_state_draws_its_mark_and_bracket_colours_by_depth() {
        let items = vec![
            task("open 0", 0, 0, CheckState::Unchecked),
            task("open 1", 1, 0, CheckState::Unchecked),
            task("open 2", 2, 0, CheckState::Unchecked),
            task("half", 0, 0, CheckState::Half),
            task("done", 0, 0, CheckState::Checked),
        ];
        let drawn = rows(&items, &[0, 1, 2, 3, 4], None, 80, "");
        let brackets = |row: &Text<'static>| (span(row, "[").style.fg, span(row, "] ").style.fg);
        let sapphire = Some(theme::SAPPHIRE);
        assert_eq!(brackets(&drawn[0]), (sapphire, sapphire));
        assert_eq!(brackets(&drawn[1]).0, Some(Color::Rgb(86, 169, 206)));
        assert_eq!(brackets(&drawn[2]).0, Some(Color::Rgb(56, 139, 176)));
        assert_eq!(span(&drawn[0], " ").style.fg, None, "the gutter");
        assert_eq!(span(&drawn[0], "open 0").style.fg, Some(theme::TEXT));

        assert_eq!(span(&drawn[3], "/").style.fg, Some(theme::YELLOW));
        assert_eq!(brackets(&drawn[3]).0, Some(theme::OVERLAY));
        assert_eq!(span(&drawn[3], "half").style.fg, Some(theme::SUBTEXT));

        assert_eq!(span(&drawn[4], "x").style.fg, Some(theme::SAPPHIRE));
        assert_eq!(brackets(&drawn[4]).0, Some(theme::OVERLAY));
        assert_eq!(span(&drawn[4], "done").style.fg, Some(theme::OVERLAY), "done is dimmed");
    }

    #[test]
    fn a_header_row_shows_its_label_bold_and_its_folder() {
        let items = vec![Task { is_header: true, ..task("WORK", 0, 0, CheckState::Unchecked) }];
        let drawn = rows(&items, &[0], None, 80, "");
        let label = span(&drawn[0], "WORK ");
        assert_eq!(label.style.fg, Some(theme::MAUVE));
        assert!(label.style.add_modifier.contains(Modifier::BOLD));
        assert_eq!(span(&drawn[0], "/nz51/none").style.fg, Some(theme::OVERLAY));
        assert_eq!(span(&drawn[0], theme::TREE_GLYPHS.section_open).style.fg, Some(theme::SURFACE));
    }

    #[test]
    fn a_long_todo_wraps_two_columns_past_its_text() {
        let items = vec![task("one two three four five six seven", 0, 0, CheckState::Unchecked)];
        let drawn = rows(&items, &[0], None, 30, "");
        let lines = plain(&drawn[0]);
        assert!(lines.len() > 2, "{lines:?}");
        let text_col = |s: &str| s.chars().position(char::is_alphabetic).unwrap();
        for line in &lines[1..] {
            assert_eq!(
                text_col(line),
                text_col(&lines[0]) + WRAP_INDENT,
                "{lines:?}"
            );
        }
        assert_eq!(lines.concat().split_whitespace().filter(|w| w.chars().all(char::is_alphabetic)).count(), 7);
    }

    #[test]
    fn no_wrapped_row_is_wider_than_the_list() {
        let text = "a long todo that wraps over several lines of the fleet with åäö ünïcode and \
                    a_word_longer_than_any_line_can_hold";
        let items = vec![
            task("parent", 0, 0, CheckState::Unchecked),
            task(text, 1, 0, CheckState::Unchecked),
            task(text, 2, 0, CheckState::Unchecked),
            task(text, 0, 0, CheckState::Unchecked),
        ];
        for width in 24..=40 {
            let drawn = rows(&items, &[0, 1, 2, 3], None, width, "");
            for row in &drawn {
                for line in &row.lines {
                    assert!(line.width() <= usize::from(width), "{width}: {line:?}");
                }
            }
            assert!(drawn[2].lines.len() > 1, "{width}: the deepest todo wraps");
        }
    }

    #[test]
    fn set_tags_draw_coloured_dots_and_unset_ones_dim_dots() {
        let spans = flags_slots(FLAG_IMPORTANT | FLAG_BLOCKED);
        let dots: Vec<&str> = spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(dots, vec![" ", "●", "·", "·", "·", "●", " "]);
        assert_eq!(spans[1].style.fg, Some(theme::FLAG_COLORS[0]));
        assert_eq!(spans[5].style.fg, Some(theme::FLAG_COLORS[4]));
        assert_eq!(spans[2].style.fg, Some(Color::Rgb(50, 50, 65)));
        let hovered = flags_slots_with_hover(0, Some(2));
        assert_eq!(hovered[3].content, "●");
        assert!(hovered[3].style.add_modifier.contains(Modifier::DIM));
    }
}
