//! The event loop and its key dispatch.

use super::*;

/// What a browse key does while the preview is focused, before the list's
/// own handling: scroll the preview by a number of lines, nothing, or the
/// same as with the list focused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PreviewFocusKey {
    Scroll(i32),
    Inert,
    Pass,
}

/// The preview-focused meaning of `code`; `page` is the PgDn/PgUp step.
/// `j`/`k`/Down/Up (Shift or not) scroll a line and PgDn/PgUp a page. The
/// keys that act on the list's cursor row or tree shape are inert: `h`,
/// `l`, Left, Right, Enter, `o` and Space. Everything else passes through
/// to the list's handling, and a key that opens a prompt moves focus there.
pub(super) fn preview_focus_key(code: KeyCode, page: i32) -> PreviewFocusKey {
    match code {
        KeyCode::Char('j') | KeyCode::Down => PreviewFocusKey::Scroll(1),
        KeyCode::Char('k') | KeyCode::Up => PreviewFocusKey::Scroll(-1),
        KeyCode::PageDown => PreviewFocusKey::Scroll(page),
        KeyCode::PageUp => PreviewFocusKey::Scroll(-page),
        KeyCode::Char('h' | 'l' | 'o' | ' ') | KeyCode::Left | KeyCode::Right | KeyCode::Enter => {
            PreviewFocusKey::Inert
        }
        _ => PreviewFocusKey::Pass,
    }
}

// --- Event loop ---

#[allow(clippy::too_many_lines)]
pub(super) fn event_loop(
    terminal: &mut super::TuiTerminal,
    forest: &mut Forest,
    retired: &mut Vec<(PathBuf, String)>,
    carried: &mut Vec<CarriedTags>,
    ctx: &TreeContext,
    config: &Config,
    rebuild: &dyn Fn() -> Result<Vec<SectionSpec>>,
) -> Result<()> {
    let mut new_note: Option<NewNotePrompt> = None;
    let mut confirm_delete: Option<DeletePrompt> = None;
    let mut confirm_bulk_delete: Option<Vec<BulkItem>> = None;
    let mut marks: HashSet<PathBuf> = HashSet::new();
    let mut move_prompt: Option<MovePrompt> = None;
    // With rows marked, `move_prompt` is the shared prompt and this holds
    // one prompt per item of the set.
    let mut move_set: Option<Vec<MovePrompt>> = None;
    let mut confirm_move: Option<MovePlan> = None;
    let mut confirm_bulk_move: Option<BulkMovePlan> = None;
    let mut state = ListState::default();
    state.select(Some(0));
    let mut vim = VimCommandMode::new();
    let mut search_mode = false;
    let mut search_buffer = String::new();
    let mut cursor_pos: usize = 0;
    let mut focus_active = false;
    let mut pre_focus_expanded: Vec<(usize, bool)> = Vec::new();
    let mut help = HelpState::default();
    let mut flag_mode = false;
    let mut rename_buffer: Option<String> = None;
    // The text the open rename prompt was prefilled with: Enter on it
    // unchanged renames nothing.
    let mut rename_shown = String::new();
    let mut status_message: Option<String> = None;
    let mut preview_scroll: u16 = 0;
    // Set on every draw: the last scroll offset that still fills the preview
    // pane, and the pane's inner height, for the scroll keys.
    let mut preview_max: u16 = 0;
    let mut preview_height: u16 = 0;
    let mut last_preview_idx: usize = usize::MAX;
    let mut preview = Preview::default();
    // The split, focus, fold and drag; session only. `body_area` is the rect
    // the two panes shared at the last draw, for the split keys' clamp and
    // the mouse hit tests.
    let mut panes = Panes::default();
    let mut body_area: Rect = Rect::default();
    let mut filter_strip_area: Rect = Rect::default();
    let mut list_inner_area: Rect = Rect::default();
    let mut visible_for_mouse: Vec<usize> = Vec::new();
    let mut prev_filter_buffer = String::new();
    // The file-change probe's last reading; `None` until the first probe,
    // which only records it. Paths no longer probed keep their entry, so a
    // folder expanded again compares against its last reading.
    let mut probe: Option<ProbeSnapshot> = None;
    // The header's count of uncommitted vault files. Every action below
    // that changes files marks it stale; plain navigation never runs git.
    let mut dirty = header::vault_dirty_count(config.notez_root_path(), ctx.sync);

    loop {
        let nodes = &mut forest.nodes;
        let sections = &forest.sections;
        derive_dir_flags(nodes);
        prune_marks(&mut marks, nodes);

        // Auto-expand directories that contain filter matches whenever the
        // filter changes; matches inside collapsed dirs would stay hidden.
        let f = filter::parse(&search_buffer);
        if !f.is_empty() && search_buffer != prev_filter_buffer {
            let keep = compute_filter_keep(nodes, &f);
            for (i, k) in keep.iter().enumerate() {
                if *k && nodes[i].is_dir {
                    nodes[i].expanded = true;
                }
            }
        }
        prev_filter_buffer = search_buffer.clone();

        let visible = compute_visible(nodes, &search_buffer);
        let sel = state.selected().unwrap_or(0);
        let real_idx = visible.get(sel).copied().unwrap_or(0);

        // Prompts, confirms, the filter, tag mode and the `:` line belong to
        // the list: opening one with the preview focused moves focus there.
        let input_open = new_note.is_some()
            || rename_buffer.is_some()
            || move_prompt.is_some()
            || confirm_delete.is_some()
            || confirm_bulk_delete.is_some()
            || confirm_move.is_some()
            || confirm_bulk_move.is_some()
            || search_mode
            || flag_mode
            || vim.active;
        if input_open {
            panes.focus(Pane::List);
        }
        // Cached between actions that change files; see `DirtyCount`.
        let dirty_files = dirty.get();
        let note_count = nodes.iter().filter(|n| !n.is_dir).count();

        terminal
            .draw(|frame| {
                let full = frame.area();
                let area = Rect::new(
                    full.x + 2,
                    full.y + 1,
                    full.width.saturating_sub(4),
                    full.height.saturating_sub(2),
                );
                // The header line above the panes carries the view's title.
                let [header_area, area] =
                    Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(area);
                let header_line = header::line(
                    &header::Header {
                        title: &ctx.title,
                        path: &ctx.path_display,
                        sync: ctx.sync,
                        dirty: dirty_files,
                        counts: header::note_count(note_count),
                    },
                    usize::from(header_area.width),
                );
                frame.render_widget(Paragraph::new(header_line), header_area);
                let rows = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Min(1), Constraint::Length(1)])
                    .split(area);
                // Too narrow for both panes: the preview folds on its own.
                panes.fit(rows[0].width);
                // The border strip between the panes carries only the grip.
                let (list_area, _, preview_area) = panes.layout(rows[0]);
                body_area = rows[0];
                let inner_width = list_text_width(list_area.width);

                let items = list_items(
                    list_lines(nodes, sections, &visible, &marks, inner_width),
                    state.selected(),
                );

                let header = pane_title(Pane::List, panes.focus, Vec::new());

                // Filter strip (mirrors the todoz board).
                let active_tags = filter::parse(&search_buffer)
                    .tag_sets
                    .iter()
                    .fold(0u8, |a, s| a | s);
                // One column of lead, as the rows' gutter: see `mouse_x_to_dot`.
                let mut filter_spans: Vec<Span> = vec![Span::raw(" ")];
                for (i, def) in FLAG_DEFS.iter().enumerate() {
                    let style = if active_tags & def.bit != 0 {
                        Style::default()
                            .fg(theme::FLAG_COLORS[i])
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(theme::dim_color(theme::FLAG_COLORS[i]))
                    };
                    filter_spans.push(Span::styled("●", style));
                }
                filter_spans.push(Span::raw("  "));
                if search_mode {
                    let (before, after) =
                        search_buffer.split_at(cursor_pos.min(search_buffer.len()));
                    let cursor_char = after
                        .chars()
                        .next()
                        .map(|c| c.to_string())
                        .unwrap_or_else(|| " ".to_string());
                    let rest = if after.len() > cursor_char.len() {
                        &after[cursor_char.len()..]
                    } else {
                        ""
                    };
                    filter_spans.push(Span::styled("/", Style::default().fg(theme::YELLOW)));
                    filter_spans.push(Span::styled(
                        before.to_string(),
                        Style::default().fg(theme::TEXT),
                    ));
                    filter_spans.push(Span::styled(
                        cursor_char,
                        Style::default().fg(theme::BASE).bg(theme::SAPPHIRE),
                    ));
                    filter_spans.push(Span::styled(
                        rest.to_string(),
                        Style::default().fg(theme::TEXT),
                    ));
                    if search_buffer.is_empty() {
                        filter_spans.push(Span::styled(
                            "  text + #tag or click a dot",
                            Style::default().fg(Color::Rgb(80, 80, 95)),
                        ));
                    }
                } else if !search_buffer.is_empty() {
                    filter_spans.push(Span::styled("/", Style::default().fg(theme::YELLOW)));
                    for word in search_buffer.split(' ') {
                        if word.is_empty() {
                            continue;
                        }
                        let mut tag_color: Option<Color> = None;
                        if let Some(name) = word.strip_prefix('#') {
                            for (idx, def) in FLAG_DEFS.iter().enumerate() {
                                if def.key.eq_ignore_ascii_case(name) {
                                    tag_color = Some(theme::FLAG_COLORS[idx]);
                                    break;
                                }
                            }
                        }
                        let style = match tag_color {
                            Some(c) => Style::default().fg(c).add_modifier(Modifier::BOLD),
                            None => Style::default().fg(theme::YELLOW),
                        };
                        filter_spans.push(Span::styled(format!("{} ", word), style));
                    }
                    filter_spans.push(Span::styled(
                        " esc to clear ",
                        Style::default().fg(theme::OVERLAY),
                    ));
                } else {
                    filter_spans.push(Span::styled("/", Style::default().fg(theme::YELLOW)));
                    filter_spans.push(Span::styled("filter", Style::default().fg(theme::OVERLAY)));
                }

                let block = list_block()
                    .title(header)
                    .border_style(pane_border(Pane::List, panes.focus));
                let inner_chunks = list_chunks(list_area);
                filter_strip_area = inner_chunks[0];
                list_inner_area = inner_chunks[2];
                visible_for_mouse = visible.clone();

                frame.render_widget(block, list_area);
                frame.render_widget(Paragraph::new(Line::from(filter_spans)), inner_chunks[0]);
                frame.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        "─".repeat(inner_chunks[1].width as usize),
                        Style::default().fg(Color::Rgb(50, 50, 65)),
                    ))),
                    inner_chunks[1],
                );

                if empty_state_line(nodes).is_some() {
                    frame.render_widget(
                        Paragraph::new(Line::from(Span::styled(
                            format!("  {}", empty_state_text(&ctx.title)),
                            Style::default().fg(theme::OVERLAY),
                        ))),
                        inner_chunks[2],
                    );
                } else {
                    frame.render_stateful_widget(List::new(items), inner_chunks[2], &mut state);
                }

                if real_idx != last_preview_idx {
                    preview_scroll = 0;
                    last_preview_idx = real_idx;
                }

                // A folded preview is neither read nor drawn; its cache entry
                // stays for the unfold. Nothing is left to scroll meanwhile.
                if let Some(preview_area) = preview_area {
                    // The pane number replaces the dots' leading space.
                    let flags = if real_idx < nodes.len() {
                        nodes[real_idx].flags
                    } else {
                        0
                    };
                    let mut preview_title_spans: Vec<Span<'static>> =
                        flags_slots(flags).into_iter().skip(1).collect();
                    preview_title_spans.push(Span::styled(
                        if real_idx < nodes.len() {
                            format!("{} ", nodes[real_idx].name)
                        } else {
                            String::new()
                        },
                        Style::default().fg(theme::OVERLAY),
                    ));
                    // A file row adds its section root to the title and its
                    // suffix and section-relative path to the bottom border;
                    // folder and section rows keep the title alone.
                    let border_width = usize::from(preview_area.width.saturating_sub(2));
                    let preview_file = nodes.get(real_idx).filter(|n| !n.is_dir);
                    let mut bottom_titles = Vec::new();
                    if let Some(node) = preview_file {
                        let root = &sections[node.section].root;
                        let title_width = border_width
                            .saturating_sub(text_width(&format!(" {} ", Pane::Preview.number())));
                        preview_title_spans =
                            with_section_root(preview_title_spans, &tilde_path(root), title_width);
                        if let Some(suffix) = file_suffix(&node.path, false) {
                            let (left, right) = preview_bottom_titles(
                                suffix_spans(&suffix, preview.mode),
                                &section_relative_path(&node.path, root),
                                border_width,
                            );
                            bottom_titles.push(left);
                            bottom_titles.extend(right);
                        }
                    }
                    let mut preview_block = Block::default().title(pane_title(
                        Pane::Preview,
                        panes.focus,
                        preview_title_spans,
                    ));
                    for title in bottom_titles {
                        preview_block = preview_block.title_bottom(title);
                    }
                    let preview_block = preview_block
                        .borders(Borders::ALL)
                        .border_style(pane_border(Pane::Preview, panes.focus))
                        .border_type(ratatui::widgets::BorderType::Rounded)
                        .padding(Padding::new(1, 1, 0, 0));
                    let preview_width = preview_block.inner(preview_area).width;

                    // Preview pane: file content (cached), or a directory listing.
                    let dir_lines: Vec<Line>;
                    let preview_lines: &[Line] =
                        if real_idx < nodes.len() && !nodes[real_idx].is_dir {
                            preview.file_lines(&nodes[real_idx].path, preview_width)
                        } else {
                            dir_lines = if real_idx < nodes.len() {
                                match std::fs::read_dir(&nodes[real_idx].path) {
                                    Ok(entries) => {
                                        // Match the tree rows: infrastructure dotfiles
                                        // (.git, .tags, .notez-config.toml) are not notes.
                                        let mut names: Vec<String> = entries
                                            .flatten()
                                            .map(|e| e.file_name().to_string_lossy().to_string())
                                            .filter(|n| !n.starts_with('.'))
                                            .collect();
                                        names.sort();
                                        names
                                            .iter()
                                            .map(|n| {
                                                let color = if n.ends_with(".md") {
                                                    theme::TEXT
                                                } else {
                                                    theme::SAPPHIRE
                                                };
                                                Line::from(Span::styled(
                                                    format!("  {}", n),
                                                    Style::default().fg(color),
                                                ))
                                            })
                                            .collect()
                                    }
                                    Err(_) => vec![],
                                }
                            } else {
                                vec![]
                            };
                            &dir_lines
                        };

                    // Past u16::MAX lines the scroll offset cannot reach anyway.
                    let total_lines = u16::try_from(preview_lines.len()).unwrap_or(u16::MAX);
                    preview_height = preview_area.height.saturating_sub(2);
                    preview_max = total_lines.saturating_sub(preview_height);
                    preview_scroll = scrolled(preview_scroll, 0, preview_max);

                    // Only the visible lines are handed over, so a long cached
                    // preview is not copied whole on every frame. Same picture
                    // as scrolling the whole text: nothing wraps here.
                    let first = usize::from(preview_scroll).min(preview_lines.len());
                    let last = (first + usize::from(preview_height)).min(preview_lines.len());
                    frame.render_widget(
                        Paragraph::new(preview_lines[first..last].to_vec()).block(preview_block),
                        preview_area,
                    );
                } else {
                    preview_height = 0;
                    preview_max = 0;
                }

                // The grip on the border strip, from the same layout the
                // drag hit test uses; lit while it is being dragged.
                if let Some(grip) = panes.grip(rows[0]) {
                    for y in grip.y..grip.y + grip.height {
                        if let Some(cell) = frame.buffer_mut().cell_mut((grip.x, y)) {
                            cell.set_symbol(panes::GRIP);
                            cell.set_style(theme::grip(panes.dragging));
                        }
                    }
                }

                // Status bar.
                let slot = status_slot(
                    rename_buffer.as_deref(),
                    status_message.as_deref(),
                    vim.active,
                    flag_mode,
                    ctx.warning.as_deref(),
                );
                let width = area.width as usize;
                let toggles = footer_toggles(
                    focus_active,
                    search_mode || !search_buffer.is_empty(),
                    flag_mode,
                    view_all_lit(nodes),
                    help.open,
                );
                // For a file with a language, the preview toggle joins the
                // browsing hints.
                let selected_file = nodes
                    .get(real_idx)
                    .filter(|n| !n.is_dir)
                    .map(|n| n.path.as_path());
                let keys = pane_keys(
                    tree_keys(
                        selected_file.and_then(|path| PreviewToggle::for_file(path, preview.mode)),
                    ),
                    panes.focus,
                );
                let status = match slot {
                    _ if confirm_delete.is_some() => {
                        let prompt = confirm_delete.as_ref().expect("checked by the guard");
                        lead_with_hints(delete_lead(prompt), Mode::ConfirmDelete, &toggles, width)
                    }
                    _ if confirm_bulk_delete.is_some() => {
                        let items = confirm_bulk_delete
                            .as_deref()
                            .expect("checked by the guard");
                        lead_with_hints(
                            bulk_delete_lead(items),
                            Mode::ConfirmDelete,
                            &toggles,
                            width,
                        )
                    }
                    _ if confirm_move.is_some() => {
                        let plan = confirm_move.as_ref().expect("checked by the guard");
                        lead_with_hints(move_confirm_lead(plan), Mode::ConfirmMove, &toggles, width)
                    }
                    _ if confirm_bulk_move.is_some() => {
                        let plan = confirm_bulk_move.as_ref().expect("checked by the guard");
                        lead_with_hints(
                            bulk_move_confirm_lead(plan),
                            Mode::ConfirmMove,
                            &toggles,
                            width,
                        )
                    }
                    _ if move_prompt.is_some() => {
                        let prompt = move_prompt.as_ref().expect("checked by the guard");
                        let mode = if prompt.fixed {
                            Mode::SetScope
                        } else {
                            Mode::Move
                        };
                        if let Some(items) = &move_set {
                            lead_with_hints(
                                bulk_move_lead(prompt, items.len()),
                                mode,
                                &toggles,
                                width,
                            )
                        } else if prompt.fixed {
                            lead_with_hints(set_scope_lead(prompt), Mode::SetScope, &toggles, width)
                        } else {
                            lead_with_hints(move_lead(prompt), Mode::Move, &toggles, width)
                        }
                    }
                    // A refused name's message shows in place of the prompt.
                    _ if new_note.is_some() && status_message.is_none() => {
                        let prompt = new_note.as_ref().expect("checked by the guard");
                        let lead = if prompt.is_folder {
                            new_folder_lead(&prompt.target.label, &prompt.buffer)
                        } else {
                            new_note_lead(&prompt.target.label, &prompt.buffer)
                        };
                        lead_with_hints(lead, Mode::NewItem, &toggles, width)
                    }
                    StatusSlot::Rename(buffer) => {
                        lead_with_hints(rename_lead(buffer), Mode::Rename, &toggles, width)
                    }
                    StatusSlot::Message(message) => Line::from(Span::styled(
                        format!(" {message}"),
                        Style::default().fg(theme::PEACH),
                    )),
                    StatusSlot::VimCommand => lead_with_hints(
                        command_lead(&vim.buffer),
                        Mode::VimCommand,
                        &toggles,
                        width,
                    ),
                    StatusSlot::Tags => {
                        let cur_flags = if real_idx < nodes.len() {
                            nodes[real_idx].flags
                        } else {
                            0
                        };
                        lead_with_hints(tag_legend(cur_flags), Mode::Tag, &toggles, width)
                    }
                    // While rows are marked the count leads the hints, over
                    // the session warning, which comes back once they clear.
                    StatusSlot::Warning(_) | StatusSlot::Hints if !marks.is_empty() => {
                        let mode = footer_mode(
                            rename_buffer.is_some(),
                            vim.active,
                            search_mode,
                            flag_mode,
                            focus_active,
                        );
                        browse_footer(&keys, marks.len(), mode, &toggles, width)
                    }
                    StatusSlot::Warning(warning) => {
                        let (text, padding) = warning_layout(warning, area.width as usize);
                        Line::from(vec![
                            Span::styled(
                                " ! ",
                                Style::default().fg(theme::RED).add_modifier(Modifier::BOLD),
                            ),
                            Span::styled(text, Style::default().fg(theme::YELLOW)),
                            Span::raw(" ".repeat(padding)),
                            Span::styled(
                                "q",
                                Style::default()
                                    .fg(theme::PEACH)
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::styled("uit ", Style::default().fg(theme::OVERLAY)),
                        ])
                    }
                    StatusSlot::Hints => {
                        let mode = footer_mode(
                            rename_buffer.is_some(),
                            vim.active,
                            search_mode,
                            flag_mode,
                            focus_active,
                        );
                        browse_footer(&keys, 0, mode, &toggles, width)
                    }
                };
                frame.render_widget(Paragraph::new(status), rows[1]);

                if help.open {
                    help::render(frame, full, TREE_KEYS, &mut help);
                }
            })
            .context("failed to draw")?;

        // Wait for input. Each `PROBE_INTERVAL` without any, probe the disk
        // unless a prompt, mode, the help overlay or a drag is open; only a
        // change ends the wait without an event, to reload and redraw.
        let can_probe = !(input_open || help.open || panes.dragging);
        let ev = loop {
            if event::poll(PROBE_INTERVAL).context("failed to poll for events")? {
                break Some(event::read().context("failed to read event")?);
            }
            if !can_probe {
                continue;
            }
            let reading = probe_snapshot(&probe_paths(sections, nodes));
            let Some(last) = probe.as_mut() else {
                probe = Some(reading);
                continue;
            };
            let changed = probe_changed(last, &reading);
            last.extend(reading);
            if changed {
                break None;
            }
        };
        let Some(ev) = ev else {
            dirty.mark_stale();
            match reload_view(
                forest,
                rebuild,
                &mut state,
                &mut pre_focus_expanded,
                &search_buffer,
            ) {
                Ok(kept) => {
                    if let Some(row) = kept {
                        last_preview_idx = row;
                    }
                    // A message already showing (an action's outcome) stays.
                    if status_message.is_none() {
                        status_message = Some("reloaded (files changed)".to_string());
                    }
                }
                Err(message) => status_message = Some(message),
            }
            continue;
        };

        if let Event::Mouse(mouse) = ev {
            // A drag owns every mouse event until the button comes up,
            // wherever the pointer wanders. Anything but a drag ends it: a
            // release outside the terminal can swallow the Up event.
            if panes.dragging {
                match mouse.kind {
                    MouseEventKind::Drag(MouseButton::Left) => {
                        panes.drag_to(body_area, mouse.column)
                    }
                    _ => panes.dragging = false,
                }
                continue;
            }
            match mouse.kind {
                // The wheel scrolls the pane under the pointer: the preview
                // by `WHEEL_STEP` lines, the list by moving the cursor a row,
                // as `j`/`k` do (not while a prompt or mode owns the list).
                MouseEventKind::ScrollDown | MouseEventKind::ScrollUp => {
                    let down = mouse.kind == MouseEventKind::ScrollDown;
                    match panes.pane_at(body_area, mouse.column, mouse.row) {
                        Some(Pane::Preview) => {
                            let delta = if down { WHEEL_STEP } else { -WHEEL_STEP };
                            preview_scroll = scrolled(preview_scroll, delta, preview_max);
                        }
                        Some(Pane::List) if !input_open => {
                            if down && sel + 1 < visible.len() {
                                navigate(
                                    nodes,
                                    &mut state,
                                    &visible,
                                    sel,
                                    real_idx,
                                    focus_active,
                                    1,
                                );
                            } else if !down && sel > 0 && sel < visible.len() {
                                navigate(
                                    nodes,
                                    &mut state,
                                    &visible,
                                    sel,
                                    real_idx,
                                    focus_active,
                                    -1,
                                );
                            }
                        }
                        _ => {}
                    }
                }
                // Grabbing the border starts a drag before any other click
                // handling. Otherwise the click focuses the pane under it; a
                // preview click does nothing else, a list click goes on to
                // the filter strip, the rows and the rows' tag field.
                MouseEventKind::Down(MouseButton::Left) => {
                    if panes.press(body_area, mouse.column, mouse.row) != Press::Pane(Pane::List) {
                        continue;
                    }
                    if mouse.row == filter_strip_area.y {
                        if let Some(d) = mouse_x_to_dot(mouse.column, filter_strip_area.x) {
                            search_buffer =
                                filter::toggle_tag_in_buffer(&search_buffer, d as usize);
                            cursor_pos = search_buffer.len();
                            continue;
                        }
                        search_mode = true;
                        cursor_pos = search_buffer.len();
                        continue;
                    }
                    if mouse.row >= list_inner_area.y
                        && mouse.row < list_inner_area.y.saturating_add(list_inner_area.height)
                        && mouse.column >= list_inner_area.x
                        && mouse.column < list_inner_area.x.saturating_add(list_inner_area.width)
                    {
                        let list_row = (mouse.row - list_inner_area.y) as usize;
                        let vis_idx = state.offset() + list_row;
                        if let Some(&real) = visible_for_mouse.get(vis_idx) {
                            state.select(Some(vis_idx));
                            if let Some(tag) = field_click_tag(
                                mouse.column,
                                list_inner_area.x,
                                nodes[real].is_dir,
                                input_open,
                            ) {
                                nodes[real].flags ^= FLAG_DEFS[tag].bit;
                                continue;
                            }
                            if nodes[real].is_dir {
                                nodes[real].expanded = !nodes[real].expanded;
                            }
                        }
                    }
                }
                _ => {}
            }
            continue;
        }

        let Event::Key(key) = ev else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }

        if key.code == KeyCode::Char('c')
            && key
                .modifiers
                .contains(crossterm::event::KeyModifiers::CONTROL)
        {
            break;
        }

        if help.handle_key(key) {
            continue;
        }

        status_message = None;

        if let Some(prompt) = confirm_delete.take() {
            let old_nodes = forest.nodes.clone();
            if let Some(outcome) =
                answer_delete(key.code, forest, retired, &prompt, &search_buffer, rebuild)
            {
                dirty.mark_stale();
                if outcome.relisted {
                    refresh_probe(&mut probe, forest);
                }
                pre_focus_expanded = remap_rows(&old_nodes, &forest.nodes, &pre_focus_expanded);
                status_message = Some(outcome.message);
                let visible = compute_visible(&forest.nodes, &search_buffer);
                let pos = outcome
                    .row
                    .and_then(|r| visible.iter().position(|&i| i == r));
                state.select(Some(pos.unwrap_or(0)));
            }
            continue;
        }

        if let Some(items) = confirm_bulk_delete.take() {
            let old_nodes = forest.nodes.clone();
            if let Some(outcome) =
                answer_bulk_delete(key.code, forest, retired, &items, &search_buffer, rebuild)
            {
                dirty.mark_stale();
                if outcome.relisted {
                    refresh_probe(&mut probe, forest);
                }
                marks.clear();
                pre_focus_expanded = remap_rows(&old_nodes, &forest.nodes, &pre_focus_expanded);
                status_message = Some(outcome.message);
                let visible = compute_visible(&forest.nodes, &search_buffer);
                let pos = outcome
                    .row
                    .and_then(|r| visible.iter().position(|&i| i == r));
                state.select(Some(pos.unwrap_or(0)));
            }
            continue;
        }

        // The move runs below, once, whether it was confirmed or needed no
        // question; every other path through the prompt ends here.
        let mut run_move: Option<MovePlan> = None;
        let mut run_bulk_move: Option<BulkMovePlan> = None;
        if let Some(plan) = confirm_move.take() {
            if !move_confirmed(key.code) {
                continue;
            }
            run_move = Some(plan);
        } else if let Some(plan) = confirm_bulk_move.take() {
            // Cancelling keeps the marks.
            if !move_confirmed(key.code) {
                continue;
            }
            run_bulk_move = Some(plan);
        } else if let Some(prompt) = move_prompt.as_mut() {
            match key.code {
                KeyCode::Esc => {
                    move_prompt = None;
                    move_set = None;
                }
                KeyCode::Tab => {
                    let scope = &mut prompt.scope;
                    scope.target = next_scope_target(
                        &scope.target,
                        &scope.origin,
                        scope.project.as_deref(),
                        &ctx.new_note_roots,
                        ctx.current_project.as_deref(),
                    );
                }
                KeyCode::Enter => {
                    let prompt = move_prompt.take().expect("the prompt is open");
                    if let Some(items) = move_set.take() {
                        match resolve_bulk_move(&prompt, &items, &ctx.new_note_roots) {
                            Err(message) => status_message = Some(message),
                            Ok(None) => {}
                            Ok(Some(plan)) if plan.needs_confirm() => {
                                confirm_bulk_move = Some(plan)
                            }
                            Ok(Some(plan)) => run_bulk_move = Some(plan),
                        }
                    } else {
                        match resolve_enter(&prompt, &ctx.new_note_roots) {
                            Err(message) => status_message = Some(message),
                            Ok(None) => {}
                            Ok(Some(plan)) if plan.needs_confirm() => confirm_move = Some(plan),
                            Ok(Some(plan)) => run_move = Some(plan),
                        }
                    }
                }
                code => type_into_move(prompt, code),
            }
            if run_move.is_none() && run_bulk_move.is_none() {
                continue;
            }
        }
        if let Some(plan) = run_bulk_move {
            // The cursor goes to the first moved item when the rebuilt list
            // shows it, else it stays on the row it was on.
            let old_nodes = forest.nodes.clone();
            let was_on = compute_visible(&old_nodes, &search_buffer)
                .get(state.selected().unwrap_or(0))
                .map(|&i| old_nodes[i].path.clone());
            let outcome = apply_bulk_move(
                forest,
                retired,
                carried,
                &plan.plans,
                &ctx.new_note_roots,
                rebuild,
            );
            dirty.mark_stale();
            if outcome.relisted {
                refresh_probe(&mut probe, forest);
            }
            marks.clear();
            pre_focus_expanded = remap_rows(&old_nodes, &forest.nodes, &pre_focus_expanded);
            status_message = Some(outcome.message);
            let mut visible = compute_visible(&forest.nodes, &search_buffer);
            let pos = match outcome.row {
                Some(row) => {
                    if !visible.contains(&row) {
                        search_buffer.clear();
                        cursor_pos = 0;
                        search_mode = false;
                        visible = compute_visible(&forest.nodes, &search_buffer);
                    }
                    visible.iter().position(|&i| i == row)
                }
                None => was_on
                    .and_then(|path| visible.iter().position(|&i| forest.nodes[i].path == path)),
            };
            let last = visible.len().saturating_sub(1);
            state.select(Some(
                pos.unwrap_or_else(|| state.selected().unwrap_or(0).min(last)),
            ));
            continue;
        }
        if let Some(plan) = run_move {
            let old_nodes = forest.nodes.clone();
            let outcome = apply_move(
                forest,
                retired,
                carried,
                &plan,
                &ctx.new_note_roots,
                rebuild,
            );
            dirty.mark_stale();
            if outcome.relisted {
                refresh_probe(&mut probe, forest);
            }
            pre_focus_expanded = remap_rows(&old_nodes, &forest.nodes, &pre_focus_expanded);
            status_message = Some(outcome.message);
            let Some(row) = outcome.row else {
                continue;
            };
            let mut visible = compute_visible(&forest.nodes, &search_buffer);
            if !visible.contains(&row) {
                search_buffer.clear();
                cursor_pos = 0;
                search_mode = false;
                visible = compute_visible(&forest.nodes, &search_buffer);
            }
            state.select(visible.iter().position(|&i| i == row));
            continue;
        }

        if let Some(prompt) = new_note.as_mut() {
            match key.code {
                KeyCode::Esc => new_note = None,
                KeyCode::Tab => {
                    prompt.target = next_scope_target(
                        &prompt.target,
                        &prompt.origin,
                        prompt.project.as_deref(),
                        &ctx.new_note_roots,
                        ctx.current_project.as_deref(),
                    );
                }
                KeyCode::Enter if new_item_refusal(prompt).is_some() => {
                    status_message = new_item_refusal(prompt);
                }
                KeyCode::Enter if prompt.is_folder => {
                    let NewNotePrompt { target, buffer, .. } =
                        new_note.take().expect("the prompt is open");
                    let old_nodes = forest.nodes.clone();
                    let outcome = create_folder(forest, &target, &buffer, rebuild);
                    dirty.mark_stale();
                    if outcome.relisted {
                        refresh_probe(&mut probe, forest);
                    }
                    pre_focus_expanded = remap_rows(&old_nodes, &forest.nodes, &pre_focus_expanded);
                    status_message = outcome.message;
                    let Some(row) = outcome.row else {
                        continue;
                    };
                    let mut visible = compute_visible(&forest.nodes, &search_buffer);
                    if !visible.contains(&row) {
                        search_buffer.clear();
                        cursor_pos = 0;
                        search_mode = false;
                        visible = compute_visible(&forest.nodes, &search_buffer);
                        status_message = Some("filter cleared to show the new folder".to_string());
                    }
                    state.select(visible.iter().position(|&i| i == row));
                }
                KeyCode::Enter => {
                    let NewNotePrompt { target, buffer, .. } =
                        new_note.take().expect("the prompt is open");
                    let words = buffer.split_whitespace().map(String::from).collect();
                    let dated = is_quick_notes_dir(&target.dir, config);
                    let created = match add::create_in_dir(words, &target.dir, target.scope, dated)
                    {
                        Ok(created) => created,
                        Err(e) => {
                            status_message = Some(format!("new note failed: {e:#}"));
                            continue;
                        }
                    };
                    super::leave().context("failed to leave TUI")?;
                    add::open_created(&created.path, config);
                    dirty.mark_stale();
                    *terminal = super::enter().context("failed to re-enter TUI")?;

                    let sections = match rebuild() {
                        Ok(sections) => sections,
                        Err(e) => {
                            status_message = Some(format!(
                                "created {}, but the list could not be refreshed: {e:#}",
                                created.path.display()
                            ));
                            continue;
                        }
                    };
                    let old_nodes = forest.nodes.clone();
                    let row = forest.rebuild(sections, &created.path);
                    refresh_probe(&mut probe, forest);
                    pre_focus_expanded = remap_rows(&old_nodes, &forest.nodes, &pre_focus_expanded);
                    let Some(row) = row else {
                        status_message = Some(format!("created {}", created.path.display()));
                        continue;
                    };
                    let mut visible = compute_visible(&forest.nodes, &search_buffer);
                    if !visible.contains(&row) {
                        search_buffer.clear();
                        cursor_pos = 0;
                        search_mode = false;
                        visible = compute_visible(&forest.nodes, &search_buffer);
                        status_message = Some("filter cleared to show the new note".to_string());
                    }
                    state.select(visible.iter().position(|&i| i == row));
                }
                KeyCode::Backspace => {
                    prompt.buffer.pop();
                }
                KeyCode::Char(c) => prompt.buffer.push(c),
                _ => {}
            }
            continue;
        }

        if let Some(buffer) = rename_buffer.as_mut() {
            match key.code {
                KeyCode::Esc => rename_buffer = None,
                KeyCode::Enter => {
                    let visible = compute_visible(nodes, &search_buffer);
                    let vs = state.selected().unwrap_or(0);
                    let Some(&ri) = visible.get(vs) else {
                        rename_buffer = None;
                        continue;
                    };
                    let before = nodes[ri].path.clone();
                    match enter_rename(nodes, &mut forest.sections, ri, &rename_shown, buffer) {
                        RenameEnter::Keep(message) => status_message = Some(message),
                        RenameEnter::Done(message) => {
                            dirty.mark_stale();
                            rename_buffer = None;
                            if message.is_none() && nodes[ri].is_dir {
                                let visible = compute_visible(nodes, &search_buffer);
                                if let Some(pos) = visible.iter().position(|&i| i == ri) {
                                    state.select(Some(pos));
                                }
                            }
                            status_message = message;
                            match relist_after_rename(
                                forest,
                                rebuild,
                                &mut state,
                                &mut pre_focus_expanded,
                                &search_buffer,
                                &mut probe,
                                ri,
                                &before,
                            ) {
                                Some(Ok(Some(row))) => last_preview_idx = row,
                                Some(Err(failure)) => status_message = Some(failure),
                                Some(Ok(None)) | None => {}
                            }
                        }
                    }
                }
                KeyCode::Backspace => {
                    buffer.pop();
                }
                KeyCode::Char(c) => buffer.push(c),
                _ => {}
            }
            continue;
        }

        if flag_mode {
            let mut consumed = true;
            match key.code {
                KeyCode::Char(c @ '1'..='5') => {
                    let idx = (c as u8 - b'1') as usize;
                    let visible = compute_visible(nodes, &search_buffer);
                    let vs = state.selected().unwrap_or(0);
                    let ri = visible.get(vs).copied().unwrap_or(0);
                    if ri < nodes.len() && !nodes[ri].is_dir {
                        nodes[ri].flags ^= FLAG_DEFS[idx].bit;
                    }
                }
                KeyCode::Char('t') | KeyCode::Esc => {
                    flag_mode = false;
                }
                // `/` exits flag mode AND falls through to the filter handler.
                KeyCode::Char('/') => {
                    flag_mode = false;
                    consumed = false;
                }
                KeyCode::Char('j' | 'k' | 'h' | 'l')
                | KeyCode::Down
                | KeyCode::Up
                | KeyCode::Left
                | KeyCode::Right => {
                    consumed = false;
                }
                _ => {}
            }
            if consumed {
                continue;
            }
        }

        if search_mode {
            match key.code {
                KeyCode::Enter | KeyCode::Esc => {
                    search_mode = false;
                    cursor_pos = 0;
                    if key.code == KeyCode::Esc {
                        search_buffer.clear();
                    }
                }
                KeyCode::Left => {
                    cursor_pos = super::text::prev_char_boundary(&search_buffer, cursor_pos);
                }
                KeyCode::Right => {
                    cursor_pos = super::text::next_char_boundary(&search_buffer, cursor_pos);
                }
                KeyCode::Backspace => {
                    if cursor_pos > 0 {
                        let prev = super::text::prev_char_boundary(&search_buffer, cursor_pos);
                        search_buffer.remove(prev);
                        cursor_pos = prev;
                    } else {
                        search_buffer.clear();
                        search_mode = false;
                    }
                }
                KeyCode::Char(c) => {
                    search_buffer.insert(cursor_pos, c);
                    cursor_pos += c.len_utf8();
                }
                _ => {}
            }
            state.select(Some(0));
            continue;
        }

        match vim.handle_key(key) {
            VimKey::Command(cmd) if VimCommandMode::is_quit(&cmd) => break,
            VimKey::Command(_) | VimKey::Consumed => continue,
            VimKey::NotConsumed => {}
        }

        let visible = compute_visible(nodes, &search_buffer);
        let selected = state.selected().unwrap_or(0);
        let real_idx = visible.get(selected).copied().unwrap_or(0);

        if panes.focus == Pane::Preview {
            match preview_focus_key(key.code, preview_page(preview_height)) {
                PreviewFocusKey::Scroll(delta) => {
                    preview_scroll = scrolled(preview_scroll, delta, preview_max);
                    continue;
                }
                PreviewFocusKey::Inert => continue,
                PreviewFocusKey::Pass => {}
            }
        }
        if panes.handle_key(key.code, body_area.width) {
            continue;
        }

        match key.code {
            KeyCode::Char('q') => break,
            KeyCode::Esc => browse_escape(&mut marks, &mut search_buffer),
            KeyCode::Char(' ') => {
                if let Some(message) =
                    press_space(&mut marks, nodes, &mut state, &visible, focus_active)
                {
                    status_message = Some(message.to_string());
                }
            }
            // Shift+Down/Up scroll the preview; matched before the plain
            // arrows, which move the cursor.
            KeyCode::Down if key.modifiers.contains(KeyModifiers::SHIFT) => {
                preview_scroll = scrolled(preview_scroll, 1, preview_max);
            }
            KeyCode::Up if key.modifiers.contains(KeyModifiers::SHIFT) => {
                preview_scroll = scrolled(preview_scroll, -1, preview_max);
            }
            KeyCode::PageDown => {
                preview_scroll =
                    scrolled(preview_scroll, preview_page(preview_height), preview_max);
            }
            KeyCode::PageUp => {
                preview_scroll =
                    scrolled(preview_scroll, -preview_page(preview_height), preview_max);
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if selected + 1 < visible.len() {
                    navigate(
                        nodes,
                        &mut state,
                        &visible,
                        selected,
                        real_idx,
                        focus_active,
                        1,
                    );
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if selected > 0 {
                    navigate(
                        nodes,
                        &mut state,
                        &visible,
                        selected,
                        real_idx,
                        focus_active,
                        -1,
                    );
                }
            }
            KeyCode::Char('l') | KeyCode::Right => {
                if selected < visible.len() {
                    let idx = visible[selected];
                    if nodes[idx].is_dir && !nodes[idx].expanded {
                        nodes[idx].expanded = true;
                        focus_active = false;
                    }
                }
            }
            KeyCode::Char('h') | KeyCode::Left => {
                if selected < visible.len() {
                    let idx = visible[selected];
                    if nodes[idx].is_dir && nodes[idx].expanded {
                        nodes[idx].expanded = false;
                        focus_active = false;
                    } else if let Some(parent) = nodes[idx].parent_idx {
                        let vis = get_visible_nodes(nodes);
                        if let Some(pos) = vis.iter().position(|&i| i == parent) {
                            state.select(Some(pos));
                        }
                    }
                }
            }
            KeyCode::Enter | KeyCode::Char('o') => {
                if selected < visible.len() {
                    let idx = visible[selected];
                    if nodes[idx].is_dir {
                        nodes[idx].expanded = !nodes[idx].expanded;
                    } else {
                        let path = nodes[idx].path.clone();
                        super::open_in_editor(&config.editor.command, &path).ok();
                        dirty.mark_stale();
                        *terminal = super::enter().context("failed to re-enter TUI")?;
                    }
                }
            }
            KeyCode::Char('f') => {
                if real_idx < nodes.len() {
                    if focus_active {
                        let current_top = find_top_dir(nodes, real_idx);
                        for &(idx, was_expanded) in &pre_focus_expanded {
                            if idx < nodes.len() {
                                nodes[idx].expanded = was_expanded;
                            }
                        }
                        let new_vis = get_visible_nodes(nodes);
                        if let Some(pos) =
                            new_vis.iter().position(|&i| i == current_top.unwrap_or(0))
                        {
                            *state.offset_mut() = 0;
                            state.select(Some(pos));
                        }
                        focus_active = false;
                    } else {
                        pre_focus_expanded = nodes
                            .iter()
                            .enumerate()
                            .filter(|(_, n)| n.is_dir && n.depth == 0)
                            .map(|(i, n)| (i, n.expanded))
                            .collect();
                        let focused_top = find_top_dir(nodes, real_idx);
                        for (i, node) in nodes.iter_mut().enumerate() {
                            if node.is_dir && node.depth == 0 {
                                node.expanded = Some(i) == focused_top;
                            }
                        }
                        focus_active = true;
                    }
                }
            }
            KeyCode::Char('v') => {
                let current_top = find_top_dir(nodes, real_idx).unwrap_or(0);
                let any_collapsed = any_top_collapsed(nodes);
                for node in nodes.iter_mut() {
                    if node.is_dir && node.depth == 0 {
                        node.expanded = any_collapsed;
                    }
                }
                let new_vis = get_visible_nodes(nodes);
                if let Some(pos) = new_vis.iter().position(|&i| i == current_top) {
                    *state.offset_mut() = 0;
                    state.select(Some(pos));
                }
                focus_active = false;
            }
            KeyCode::Char('R') => {
                dirty.mark_stale();
                match reload_view(
                    forest,
                    rebuild,
                    &mut state,
                    &mut pre_focus_expanded,
                    &search_buffer,
                ) {
                    Ok(kept) => {
                        if let Some(row) = kept {
                            last_preview_idx = row;
                        }
                        status_message = Some("reloaded".to_string());
                        refresh_probe(&mut probe, forest);
                    }
                    Err(message) => status_message = Some(message),
                }
            }
            KeyCode::Char('/') => {
                search_mode = true;
                search_buffer.clear();
                cursor_pos = 0;
            }
            KeyCode::Char('t') => {
                flag_mode = true;
            }
            KeyCode::Char('r') => {
                match rename_request(nodes, &forest.sections, visible.get(selected).copied()) {
                    Ok(text) => {
                        rename_shown = text.clone().unwrap_or_default();
                        rename_buffer = text;
                    }
                    Err(message) => status_message = Some(message.to_string()),
                }
            }
            KeyCode::Char('n') => {
                new_note = Some(open_new_note_prompt(
                    nodes,
                    &forest.sections,
                    visible.get(selected).copied(),
                    ctx,
                ));
            }
            KeyCode::Char('N') => {
                match open_new_folder_prompt(
                    nodes,
                    &forest.sections,
                    visible.get(selected).copied(),
                    ctx,
                ) {
                    Ok(prompt) => new_note = Some(prompt),
                    Err(message) => status_message = Some(message.to_string()),
                }
            }
            KeyCode::Char(code @ ('m' | 'S')) if !marks.is_empty() => {
                match bulk_move_request(nodes, &forest.sections, &marks, ctx, code == 'S') {
                    Ok((prompt, items)) => {
                        move_prompt = Some(prompt);
                        move_set = Some(items);
                    }
                    Err(message) => status_message = Some(message),
                }
            }
            KeyCode::Char('d') if !marks.is_empty() => {
                match bulk_delete_request(
                    nodes,
                    &forest.sections,
                    &marks,
                    ctx.current_project.as_deref(),
                ) {
                    Ok(items) if !items.is_empty() => confirm_bulk_delete = Some(items),
                    Ok(_) => {}
                    Err(message) => status_message = Some(message),
                }
            }
            KeyCode::Char('m') => {
                match move_request(nodes, &forest.sections, visible.get(selected).copied(), ctx) {
                    Ok(prompt) => move_prompt = Some(prompt),
                    Err(message) => status_message = Some(message.to_string()),
                }
            }
            KeyCode::Char('S') => {
                match set_scope_request(
                    nodes,
                    &forest.sections,
                    visible.get(selected).copied(),
                    ctx,
                ) {
                    Ok(prompt) => move_prompt = Some(prompt),
                    Err(message) => status_message = Some(message.to_string()),
                }
            }
            KeyCode::Char('d') => {
                match delete_request(
                    nodes,
                    &forest.sections,
                    visible.get(selected).copied(),
                    ctx.current_project.as_deref(),
                ) {
                    Ok(prompt) => confirm_delete = prompt,
                    Err(message) => status_message = Some(message.to_string()),
                }
            }
            KeyCode::Char('J') => {
                preview_scroll = scrolled(preview_scroll, 1, preview_max);
            }
            KeyCode::Char('K') => {
                preview_scroll = scrolled(preview_scroll, -1, preview_max);
            }
            KeyCode::Char('p') => {
                let path = nodes
                    .get(real_idx)
                    .filter(|n| !n.is_dir)
                    .map(|n| n.path.as_path());
                preview.toggle(path);
            }
            KeyCode::Char('?') => {
                help.open();
            }
            _ => {}
        }
    }

    Ok(())
}

pub(super) fn file_name_of(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// j/k step that, in focus mode, hops the exclusive expansion from one
/// top-level section to the next as the cursor crosses section boundaries.
pub(super) fn navigate(
    nodes: &mut [TreeNode],
    state: &mut ListState,
    visible: &[usize],
    selected: usize,
    real_idx: usize,
    focus_active: bool,
    step: isize,
) {
    let next_sel = selected.saturating_add_signed(step);
    let target = visible[next_sel];
    if focus_active {
        let old_top = find_top_dir(nodes, real_idx);
        let new_top = find_top_dir(nodes, target);
        if old_top != new_top {
            if let Some(ot) = old_top {
                nodes[ot].expanded = false;
            }
            if let Some(nt) = new_top {
                nodes[nt].expanded = true;
            }
            let new_vis = get_visible_nodes(nodes);
            if let Some(pos) = new_vis.iter().position(|&i| i == target) {
                state.select(Some(pos));
            }
            return;
        }
    }
    state.select(Some(next_sel));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_the_preview_focused_j_k_arrows_and_pages_scroll_it() {
        assert_eq!(
            preview_focus_key(KeyCode::Char('j'), 9),
            PreviewFocusKey::Scroll(1)
        );
        assert_eq!(
            preview_focus_key(KeyCode::Down, 9),
            PreviewFocusKey::Scroll(1)
        );
        assert_eq!(
            preview_focus_key(KeyCode::Char('k'), 9),
            PreviewFocusKey::Scroll(-1)
        );
        assert_eq!(
            preview_focus_key(KeyCode::Up, 9),
            PreviewFocusKey::Scroll(-1)
        );
        assert_eq!(
            preview_focus_key(KeyCode::PageDown, 9),
            PreviewFocusKey::Scroll(9)
        );
        assert_eq!(
            preview_focus_key(KeyCode::PageUp, 9),
            PreviewFocusKey::Scroll(-9)
        );
    }

    #[test]
    fn with_the_preview_focused_the_list_cursor_keys_are_inert() {
        for code in [
            KeyCode::Char('h'),
            KeyCode::Char('l'),
            KeyCode::Left,
            KeyCode::Right,
            KeyCode::Enter,
            KeyCode::Char('o'),
            KeyCode::Char(' '),
        ] {
            assert_eq!(
                preview_focus_key(code, 9),
                PreviewFocusKey::Inert,
                "{code:?}"
            );
        }
    }

    #[test]
    fn with_the_preview_focused_every_other_key_passes_to_the_list() {
        for c in [
            'J', 'K', 'p', 'q', '?', '/', 't', 'n', 'N', 'r', 'm', 'S', 'd', 'f', 'v', ':', '1',
            '2', '<', '>', '=',
        ] {
            assert_eq!(
                preview_focus_key(KeyCode::Char(c), 9),
                PreviewFocusKey::Pass,
                "{c}"
            );
        }
        for code in [KeyCode::Tab, KeyCode::Esc] {
            assert_eq!(
                preview_focus_key(code, 9),
                PreviewFocusKey::Pass,
                "{code:?}"
            );
        }
    }
}
