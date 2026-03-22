use crate::*;

const MUTED_TEXT_COLOR: Color = Color::Rgb(140, 140, 140);
const CATEGORY_MANAGER_PANE_IDLE: Color = Color::Rgb(82, 92, 112);
const CATEGORY_MANAGER_PANE_FOCUS: Color = Color::LightCyan;
const CATEGORY_MANAGER_TEXT_ENTRY: Color = Color::LightMagenta;
const CATEGORY_MANAGER_EDIT_FOCUS: Color = Color::Yellow;
const NOTE_PLACEHOLDER_TEXT: &str = "Notes, context, links, ideas, next actions...";
const ALSO_MATCH_PLACEHOLDER_TEXT: &str = "One term or phrase per line...";
const FOOTER_HEIGHT: u16 = 4;
const CATEGORY_DETAILS_INFO_HEIGHT: u16 = 5;
const CATEGORY_DETAILS_INFO_HEIGHT_NUMERIC: u16 = 6;

fn clip_text_for_row(
    text: &str,
    cursor: usize,
    width: usize,
    around_cursor: bool,
) -> (String, usize) {
    if width == 0 {
        return (String::new(), 0);
    }
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let cursor = cursor.min(len);
    if len <= width {
        return (text.to_string(), cursor);
    }
    if width == 1 {
        return ("…".to_string(), 0);
    }
    if !around_cursor {
        let keep = width.saturating_sub(1);
        let prefix: String = chars.iter().take(keep).collect();
        return (format!("{prefix}…"), cursor.min(keep.saturating_sub(1)));
    }

    let mut left = cursor
        .saturating_sub(width / 2)
        .min(len.saturating_sub(width));
    let mut right = (left + width).min(len);
    if cursor >= right {
        right = (cursor + 1).min(len);
        left = right.saturating_sub(width);
    }

    let left_ellipsis = left > 0;
    let right_ellipsis = right < len;
    let mut inner_width = width;
    if left_ellipsis {
        inner_width = inner_width.saturating_sub(1);
    }
    if right_ellipsis {
        inner_width = inner_width.saturating_sub(1);
    }

    let mut inner_left = left.min(len.saturating_sub(inner_width));
    if cursor < inner_left {
        inner_left = cursor;
    }
    if cursor >= inner_left.saturating_add(inner_width) {
        inner_left = cursor.saturating_add(1).saturating_sub(inner_width);
    }
    let inner_right = (inner_left + inner_width).min(len);

    let mut out = String::new();
    if left_ellipsis {
        out.push('…');
    }
    out.extend(chars[inner_left..inner_right].iter());
    if right_ellipsis {
        out.push('…');
    }
    let cursor_visible = (if left_ellipsis { 1 } else { 0 })
        + cursor
            .saturating_sub(inner_left)
            .min(inner_width.saturating_sub(1));
    (out, cursor_visible)
}

impl App {
    fn list_state_for(area: Rect, selected_line: Option<usize>) -> ListState {
        let mut state = ListState::default().with_selected(selected_line);
        *state.offset_mut() = list_scroll_for_selected_line(area, selected_line) as usize;
        state
    }

    fn table_state_for(area: Rect, selected_row: Option<usize>) -> TableState {
        let mut state = TableState::default().with_selected(selected_row);
        *state.offset_mut() = list_scroll_for_selected_line(area, selected_row) as usize;
        state
    }

    fn stable_table_offset(
        area: Rect,
        selected_row: Option<usize>,
        preferred_offset: usize,
        item_count: usize,
    ) -> usize {
        if item_count == 0 {
            return 0;
        }
        let clamped_preferred = preferred_offset.min(item_count.saturating_sub(1));
        let Some(selected_row) = selected_row else {
            return clamped_preferred;
        };
        let viewport_rows = area.height.saturating_sub(2) as usize;
        if viewport_rows == 0 {
            return 0;
        }
        let selected_visible = selected_row >= clamped_preferred
            && selected_row < clamped_preferred.saturating_add(viewport_rows);
        if selected_visible {
            clamped_preferred
        } else {
            list_scroll_for_selected_line(area, Some(selected_row)) as usize
        }
    }

    fn effective_board_display_mode_for_slot(&self, slot: &Slot) -> BoardDisplayMode {
        let current_view = self.current_view();
        match (&slot.context, current_view) {
            (SlotContext::Section { section_index }, Some(view))
            | (SlotContext::GeneratedSection { section_index, .. }, Some(view)) => view
                .sections
                .get(*section_index)
                .and_then(|section| section.board_display_mode_override)
                .unwrap_or(view.board_display_mode),
            _ => current_view
                .map(|view| view.board_display_mode)
                .unwrap_or(BoardDisplayMode::SingleLine),
        }
    }

    fn variable_height_list_offset(
        area: Rect,
        item_heights: &[usize],
        selected_index: Option<usize>,
    ) -> usize {
        let Some(selected_index) = selected_index else {
            return 0;
        };
        let viewport_rows = area.height.saturating_sub(2) as usize;
        if viewport_rows == 0 || item_heights.is_empty() {
            return 0;
        }

        let mut offset = selected_index.min(item_heights.len().saturating_sub(1));
        let mut used_rows = item_heights[offset].max(1);
        while offset > 0 {
            let next_rows = item_heights[offset - 1].max(1);
            if used_rows + next_rows > viewport_rows {
                break;
            }
            used_rows += next_rows;
            offset -= 1;
        }
        offset
    }

    fn selected_item_visible_with_offset(
        area: Rect,
        item_heights: &[usize],
        selected_index: usize,
        offset: usize,
    ) -> bool {
        let viewport_rows = area.height.saturating_sub(2) as usize;
        if viewport_rows == 0 || item_heights.is_empty() || selected_index < offset {
            return false;
        }

        let row_start = item_heights
            .iter()
            .skip(offset)
            .take(selected_index.saturating_sub(offset))
            .sum::<usize>();
        let row_end = row_start + item_heights[selected_index].max(1);
        row_end <= viewport_rows
    }

    fn stable_variable_height_list_offset(
        area: Rect,
        item_heights: &[usize],
        selected_index: Option<usize>,
        preferred_offset: usize,
    ) -> usize {
        let Some(selected_index) = selected_index else {
            return preferred_offset.min(item_heights.len().saturating_sub(1));
        };
        let clamped_preferred = preferred_offset.min(item_heights.len().saturating_sub(1));
        if Self::selected_item_visible_with_offset(
            area,
            item_heights,
            selected_index,
            clamped_preferred,
        ) {
            clamped_preferred
        } else {
            Self::variable_height_list_offset(area, item_heights, Some(selected_index))
        }
    }

    fn render_vertical_scrollbar(
        frame: &mut ratatui::Frame<'_>,
        area: Rect,
        content_len: usize,
        position: usize,
    ) {
        let mut scrollbar_state = ScrollbarState::new(content_len.max(1))
            .position(position.min(content_len.saturating_sub(1)));
        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight),
            area,
            &mut scrollbar_state,
        );
    }

    pub(crate) fn draw(&self, frame: &mut ratatui::Frame<'_>) {
        let show_search_bar = !(matches!(self.mode, Mode::ViewEdit | Mode::CategoryManager)
            || self.mode == Mode::InputPanel
                && self.name_input_context == Some(NameInputContext::CategoryCreate));

        let layout = if show_search_bar {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1),
                    Constraint::Length(1),
                    Constraint::Min(1),
                    Constraint::Length(FOOTER_HEIGHT),
                ])
                .split(frame.area())
        } else {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1),
                    Constraint::Min(1),
                    Constraint::Length(FOOTER_HEIGHT),
                ])
                .split(frame.area())
        };

        let header = self.render_header();
        frame.render_widget(header, layout[0]);

        let (main_area, footer_area) = if show_search_bar {
            self.render_search_bar(frame, layout[1]);
            (layout[2], layout[3])
        } else {
            (layout[1], layout[2])
        };

        self.render_main(frame, main_area);

        let footer = self.render_footer(footer_area.width);
        frame.render_widget(footer, footer_area);
        if let Some((x, y)) = self.input_cursor_position(footer_area) {
            frame.set_cursor_position((x, y));
        }
        if self.mode == Mode::InputPanel {
            if let Some(ref panel) = self.input_panel {
                let popup_area = input_panel_popup_area(frame.area(), panel.kind);
                self.render_input_panel(frame, popup_area, panel);
                if let Some((x, y)) = self.input_panel_cursor_position(popup_area, panel) {
                    frame.set_cursor_position((x, y));
                }
            }
        }
        if matches!(self.mode, Mode::ViewPicker | Mode::ViewDeleteConfirm) {
            self.render_view_picker(frame, centered_rect(60, 60, frame.area()));
        }
        if matches!(self.mode, Mode::ItemAssignPicker | Mode::ItemAssignInput) {
            self.render_item_assign_picker(frame, centered_rect(72, 72, frame.area()));
        }
        if self.mode == Mode::LinkWizard {
            let popup_area = centered_rect(72, 72, frame.area());
            self.render_link_wizard(frame, popup_area);
            if let Some((x, y)) = self.link_wizard_cursor_position(popup_area) {
                frame.set_cursor_position((x, y));
            }
        }
        if self.mode == Mode::CategoryDirectEdit {
            let popup_area = centered_rect(64, 62, frame.area());
            self.render_category_direct_edit_picker(frame, popup_area);
            if let Some((x, y)) = self.category_direct_edit_cursor_position(popup_area) {
                frame.set_cursor_position((x, y));
            }
        }
        if self.mode == Mode::CategoryColumnPicker {
            let popup_area = centered_rect(62, 58, frame.area());
            self.render_category_column_picker(frame, popup_area);
            if let Some((x, y)) = self.category_column_picker_cursor_position(popup_area) {
                frame.set_cursor_position((x, y));
            }
        }
        if self.mode == Mode::BoardAddColumnPicker {
            let popup_area = centered_rect(58, 56, frame.area());
            self.render_board_add_column_picker(frame, popup_area);
            if let Some((x, y)) = self.board_add_column_cursor_position(popup_area) {
                frame.set_cursor_position((x, y));
            }
        }
        if self.mode == Mode::HelpPanel {
            self.render_help_panel(frame, centered_rect(52, 90, frame.area()));
        }
        if self.mode == Mode::SuggestionReview {
            self.render_suggestion_review(frame, centered_rect(80, 70, frame.area()));
        }
    }

    fn is_category_direct_edit_dirty(&self) -> bool {
        self.category_direct_edit
            .as_ref()
            .map(|state| {
                let current: Vec<Option<CategoryId>> =
                    state.rows.iter().map(|r| r.category_id).collect();
                current != state.original_category_ids
            })
            .unwrap_or(false)
    }

    fn render_category_direct_edit_picker(&self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        let dirty_marker = if self.is_category_direct_edit_dirty() {
            " *"
        } else {
            ""
        };
        let title = format!("Set Category{dirty_marker}");
        frame.render_widget(Clear, area);
        frame.render_widget(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
            area,
        );

        if area.width < 4 || area.height < 6 {
            return;
        }

        let inner = Rect {
            x: area.x.saturating_add(1),
            y: area.y.saturating_add(1),
            width: area.width.saturating_sub(2),
            height: area.height.saturating_sub(2),
        };
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // context
                Constraint::Length(1), // scope/context details
                Constraint::Length(6), // row list
                Constraint::Length(3), // active input
                Constraint::Min(6),    // suggestions / create confirm
                Constraint::Length(2), // help/actions
            ])
            .split(inner);

        let direct_state = self.category_direct_edit_state();
        let context_text = direct_state
            .map(|state| {
                let focus_label = match state.focus {
                    CategoryDirectEditFocus::Entries => "Entries",
                    CategoryDirectEditFocus::Input => "Input",
                    CategoryDirectEditFocus::Suggestions => "Suggestions",
                };
                format!(
                    "Column: {}  Item: {}  Row: {}/{}  Focus: {}",
                    state.parent_name,
                    truncate_board_cell(&state.item_label, 28),
                    state.active_row.saturating_add(1),
                    state.rows.len(),
                    focus_label,
                )
            })
            .or_else(|| {
                self.current_view().and_then(|view| {
                    self.current_slot().and_then(|slot| {
                        let section = match slot.context {
                            SlotContext::Section { section_index }
                            | SlotContext::GeneratedSection { section_index, .. } => {
                                view.sections.get(section_index)
                            }
                            _ => None,
                        }?;
                        let section_column_index =
                            Self::board_column_to_section_column_index(section, self.column_index)?;
                        let column = section.columns.get(section_column_index)?;
                        let heading = view
                            .category_aliases
                            .get(&column.heading)
                            .map(|alias| alias.trim())
                            .filter(|alias| !alias.is_empty())
                            .map(|alias| alias.to_string())
                            .or_else(|| {
                                self.categories
                                    .iter()
                                    .find(|c| c.id == column.heading)
                                    .map(|c| c.name.clone())
                            })
                            .unwrap_or_else(|| "?".to_string());
                        let item_label = self
                            .selected_item()
                            .map(|item| truncate_board_cell(&board_item_label(item), 40))
                            .unwrap_or_else(|| "(no item)".to_string());
                        Some(format!("Column: {heading}  Item: {item_label}"))
                    })
                })
            })
            .unwrap_or_else(|| "Set category".to_string());
        frame.render_widget(
            Paragraph::new(context_text).style(Style::default().fg(MUTED_TEXT_COLOR)),
            chunks[0],
        );

        let scope_text = self
            .category_direct_edit_state()
            .map(|state| {
                let exclusive = self
                    .categories
                    .iter()
                    .find(|c| c.id == state.parent_id)
                    .map(|c| if c.is_exclusive { "yes" } else { "no" })
                    .unwrap_or("?");
                format!(
                    "Scope: This column only  Parent: {} (exclusive: {exclusive})",
                    state.parent_name
                )
            })
            .unwrap_or_else(|| "Scope: This column only".to_string());
        frame.render_widget(
            Paragraph::new(scope_text).style(Style::default().fg(MUTED_TEXT_COLOR)),
            chunks[1],
        );

        let focus = direct_state
            .map(|state| state.focus)
            .unwrap_or(CategoryDirectEditFocus::Input);
        let active_input = self.active_category_direct_edit_input_text().unwrap_or("");
        let active_row_index = direct_state.map(|s| s.active_row).unwrap_or(0);
        let rows: Vec<ListItem<'_>> = direct_state
            .map(|state| {
                state
                    .rows
                    .iter()
                    .enumerate()
                    .map(|(idx, row)| {
                        let label = if row.input.trimmed().is_empty() {
                            "(new row)".to_string()
                        } else {
                            row.input.text().to_string()
                        };
                        let resolved_marker = if row.category_id.is_some() { "" } else { " *" };
                        let prefix = format!("{:>2}. ", idx + 1);
                        let style = if idx == state.active_row
                            && focus != CategoryDirectEditFocus::Entries
                        {
                            Style::default().fg(Color::Cyan)
                        } else {
                            Style::default()
                        };
                        ListItem::new(format!("{prefix}{label}{resolved_marker}")).style(style)
                    })
                    .collect()
            })
            .unwrap_or_default();
        let entries_border = if focus == CategoryDirectEditFocus::Entries {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default()
        };
        let mut entries_state = Self::list_state_for(chunks[2], Some(active_row_index));
        frame.render_stateful_widget(
            List::new(rows)
                .highlight_symbol("> ")
                .highlight_style(selected_row_style())
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Assigned In This Column")
                        .border_style(entries_border),
                ),
            chunks[2],
            &mut entries_state,
        );

        let input_border = if focus == CategoryDirectEditFocus::Input {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default()
        };
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("Category> ", Style::default().fg(Color::Yellow)),
                Span::raw(active_input),
            ]))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Edit Active Row")
                    .border_style(input_border),
            ),
            chunks[3],
        );

        let inline_create_name = self
            .category_direct_edit_state()
            .and_then(|state| state.create_confirm_name.as_deref());
        if let Some(name) = inline_create_name {
            self.render_category_create_confirm_panel(
                frame,
                chunks[4],
                name,
                "as a new child category in this column?",
            );
            frame.render_widget(
                Paragraph::new("S save draft  Esc cancel draft")
                    .style(Style::default().fg(MUTED_TEXT_COLOR)),
                chunks[5],
            );
            return;
        }

        let matches = self.get_current_suggest_matches();
        let has_matches = !matches.is_empty();
        let help_text = if active_input.trim().is_empty() {
            "Empty row: Enter removes row (or keeps one blank). S saves draft. Esc cancels draft."
        } else if has_matches {
            match focus {
                CategoryDirectEditFocus::Entries => {
                    "Entries: Up/Down move rows | Tab/Shift-Tab focus | n/a add | x remove | S save"
                }
                CategoryDirectEditFocus::Input => {
                    "Input: type edits active row | Enter resolve/create | Tab focus-next | S save"
                }
                CategoryDirectEditFocus::Suggestions => {
                    "Suggestions: Up/Down move | Tab copies name | Enter resolves row | S save"
                }
            }
        } else {
            "No match: Enter opens create confirm. S saves only resolved rows. Esc cancels draft."
        };

        if matches.is_empty() {
            let empty_msg = if active_input.trim().is_empty() {
                "(no suggestions yet)"
            } else {
                "(no matches)"
            };
            frame.render_widget(
                Paragraph::new(empty_msg).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Suggested Categories")
                        .border_style(if focus == CategoryDirectEditFocus::Suggestions {
                            Style::default().fg(Color::Cyan)
                        } else {
                            Style::default()
                        }),
                ),
                chunks[4],
            );
            frame.render_widget(
                Paragraph::new(help_text)
                    .style(Style::default().fg(MUTED_TEXT_COLOR))
                    .wrap(Wrap { trim: true }),
                chunks[5],
            );
            return;
        }

        let selected_idx = self
            .category_direct_edit_state()
            .map(|s| s.suggest_index.min(matches.len() - 1))
            .unwrap_or(0);

        let items: Vec<ListItem<'_>> = matches
            .iter()
            .map(|id| {
                let name = self
                    .categories
                    .iter()
                    .find(|c| c.id == *id)
                    .map(|c| c.name.as_str())
                    .unwrap_or("?");
                ListItem::new(name)
            })
            .collect();
        let item_count = items.len();
        let mut state = Self::list_state_for(chunks[4], Some(selected_idx));
        frame.render_stateful_widget(
            List::new(items)
                .highlight_symbol("> ")
                .highlight_style(selected_row_style())
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Suggested Categories")
                        .border_style(if focus == CategoryDirectEditFocus::Suggestions {
                            Style::default().fg(Color::Cyan)
                        } else {
                            Style::default()
                        }),
                ),
            chunks[4],
            &mut state,
        );
        Self::render_vertical_scrollbar(frame, chunks[4], item_count, state.offset());
        frame.render_widget(
            Paragraph::new(help_text)
                .style(Style::default().fg(MUTED_TEXT_COLOR))
                .wrap(Wrap { trim: true }),
            chunks[5],
        );
    }

    fn render_link_wizard(&self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        frame.render_widget(Clear, area);
        frame.render_widget(
            Block::default()
                .title("Link Wizard")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
            area,
        );
        if area.width < 6 || area.height < 8 {
            return;
        }

        let Some(state) = self.link_wizard_state() else {
            return;
        };
        let action = LinkWizardAction::from_index(state.action_index);
        let anchor = self.link_wizard_anchor_item();
        let anchor_label = anchor
            .map(board_item_label)
            .unwrap_or_else(|| state.anchor_item_id.to_string());
        let source_count = self.link_wizard_source_count();
        let matches = self.link_wizard_target_matches();
        let selected_target_id = self.link_wizard_selected_target_id();

        let inner = Rect {
            x: area.x.saturating_add(1),
            y: area.y.saturating_add(1),
            width: area.width.saturating_sub(2),
            height: area.height.saturating_sub(2),
        };
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2), // anchor
                Constraint::Length(7), // actions
                Constraint::Length(3), // target query
                Constraint::Min(5),    // target matches
                Constraint::Length(4), // preview
                Constraint::Length(2), // help
            ])
            .split(inner);

        let anchor_lines = vec![
            Line::from(if source_count > 1 {
                format!("Source set ({source_count} items)")
            } else {
                "Anchor item".to_string()
            }),
            Line::from(if source_count > 1 {
                format!("  {} (focused)", truncate_board_cell(&anchor_label, 72))
            } else {
                format!("  {}", truncate_board_cell(&anchor_label, 72))
            }),
        ];
        frame.render_widget(
            Paragraph::new(anchor_lines).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray)),
            ),
            rows[0],
        );

        let mut action_items = Vec::new();
        for (idx, candidate) in LinkWizardAction::ALL.iter().enumerate() {
            let selected = idx == state.action_index;
            let marker = if selected { ">" } else { " " };
            action_items.push(ListItem::new(format!(
                "{marker} {:<18} {}",
                candidate.label(),
                candidate.description()
            )));
        }
        let action_block = Block::default()
            .title(if state.focus == LinkWizardFocus::ScopeAction {
                "Relationship *"
            } else {
                "Relationship"
            })
            .borders(Borders::ALL)
            .border_style(if state.focus == LinkWizardFocus::ScopeAction {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            });
        frame.render_widget(List::new(action_items).block(action_block), rows[1]);

        let target_title = if state.focus == LinkWizardFocus::Target {
            "Target *"
        } else {
            "Target"
        };
        let target_block = Block::default()
            .title(target_title)
            .borders(Borders::ALL)
            .border_style(if state.focus == LinkWizardFocus::Target {
                Style::default().fg(Color::Yellow)
            } else if !action.requires_target() {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default()
            });
        let target_text = if action.requires_target() {
            format!("Search> {}", state.target_filter.text())
        } else {
            "(not used for clear dependencies)".to_string()
        };
        frame.render_widget(Paragraph::new(target_text).block(target_block), rows[2]);

        let mut target_items: Vec<ListItem> = Vec::new();
        if action.requires_target() {
            for item_id in &matches {
                let label = self
                    .all_items
                    .iter()
                    .find(|item| item.id == *item_id)
                    .map(|item| {
                        let status = if item.is_done { "done" } else { "open" };
                        format!("{status} | {}", item.text)
                    })
                    .unwrap_or_else(|| format!("missing | {item_id}"));
                target_items.push(ListItem::new(truncate_board_cell(&label, 72)));
            }
            if target_items.is_empty() {
                target_items.push(ListItem::new("  (no matches)"));
            }
        } else {
            target_items.push(ListItem::new(
                "  Clear dependencies removes prereqs and blocked items",
            ));
        }
        let matches_block = Block::default()
            .title("Matches")
            .borders(Borders::ALL)
            .border_style(if state.focus == LinkWizardFocus::Target {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            });
        if action.requires_target() {
            let selected = if matches.is_empty() {
                None
            } else {
                Some(state.target_index.min(matches.len().saturating_sub(1)))
            };
            let mut list_state = Self::list_state_for(rows[3], selected);
            let item_count = target_items.len();
            frame.render_stateful_widget(
                List::new(target_items)
                    .highlight_symbol("> ")
                    .highlight_style(selected_row_style())
                    .block(matches_block),
                rows[3],
                &mut list_state,
            );
            Self::render_vertical_scrollbar(frame, rows[3], item_count, list_state.offset());
        } else {
            frame.render_widget(List::new(target_items).block(matches_block), rows[3]);
        }

        let preview_lines = {
            let mut lines = vec![Line::from("Preview")];
            match action {
                LinkWizardAction::BlockedBy => {
                    let target = selected_target_id
                        .and_then(|id| self.all_items.iter().find(|item| item.id == id));
                    if let Some(target) = target {
                        if source_count > 1 {
                            lines.push(Line::from(format!(
                                "  {source_count} selected items blocked by {}",
                                truncate_board_cell(&target.text, 28)
                            )));
                            lines.push(Line::from(
                                "  (applies one depends-on link per selected item)",
                            ));
                        } else {
                            lines.push(Line::from(format!(
                                "  {} blocked by {}",
                                truncate_board_cell(&anchor_label, 28),
                                truncate_board_cell(&target.text, 28)
                            )));
                            lines.push(Line::from(format!(
                                "  (stores: {} depends-on {})",
                                truncate_board_cell(&anchor_label, 22),
                                truncate_board_cell(&target.text, 22)
                            )));
                        }
                    } else {
                        lines.push(Line::from("  Select a target item"));
                        lines.push(Line::from(""));
                    }
                }
                LinkWizardAction::DependsOn => {
                    let target = selected_target_id
                        .and_then(|id| self.all_items.iter().find(|item| item.id == id));
                    if let Some(target) = target {
                        if source_count > 1 {
                            lines.push(Line::from(format!(
                                "  {source_count} selected items depend on {}",
                                truncate_board_cell(&target.text, 28)
                            )));
                        } else {
                            lines.push(Line::from(format!(
                                "  {} depends on {}",
                                truncate_board_cell(&anchor_label, 28),
                                truncate_board_cell(&target.text, 28)
                            )));
                        }
                        lines.push(Line::from(""));
                    } else {
                        lines.push(Line::from("  Select a target item"));
                        lines.push(Line::from(""));
                    }
                }
                LinkWizardAction::Blocks => {
                    let target = selected_target_id
                        .and_then(|id| self.all_items.iter().find(|item| item.id == id));
                    if let Some(target) = target {
                        if source_count > 1 {
                            lines.push(Line::from(format!(
                                "  {source_count} selected items block {}",
                                truncate_board_cell(&target.text, 28)
                            )));
                            lines.push(Line::from(
                                "  (adds one blocked relation per selected item)",
                            ));
                        } else {
                            lines.push(Line::from(format!(
                                "  {} blocks {}",
                                truncate_board_cell(&anchor_label, 28),
                                truncate_board_cell(&target.text, 28)
                            )));
                            lines.push(Line::from(format!(
                                "  (stores: {} depends-on {})",
                                truncate_board_cell(&target.text, 22),
                                truncate_board_cell(&anchor_label, 22)
                            )));
                        }
                    } else {
                        lines.push(Line::from("  Select a target item"));
                        lines.push(Line::from(""));
                    }
                }
                LinkWizardAction::RelatedTo => {
                    let target = selected_target_id
                        .and_then(|id| self.all_items.iter().find(|item| item.id == id));
                    if let Some(target) = target {
                        if source_count > 1 {
                            lines.push(Line::from(format!(
                                "  {source_count} selected items related to {}",
                                truncate_board_cell(&target.text, 26)
                            )));
                            lines.push(Line::from(
                                "  (adds one symmetric relation per selected item)",
                            ));
                        } else {
                            lines.push(Line::from(format!(
                                "  {} related to {}",
                                truncate_board_cell(&anchor_label, 26),
                                truncate_board_cell(&target.text, 26)
                            )));
                            lines.push(Line::from("  (symmetric)"));
                        }
                    } else {
                        lines.push(Line::from("  Select a target item"));
                        lines.push(Line::from(""));
                    }
                }
                LinkWizardAction::ClearDependencies => {
                    if source_count > 1 {
                        lines.push(Line::from(
                            "  Remove immediate prereqs and dependents for all selected items",
                        ));
                    } else {
                        lines.push(Line::from("  Remove all immediate prereqs and dependents"));
                    }
                    lines.push(Line::from("  (does not remove related links)"));
                }
            }
            while lines.len() < 4 {
                lines.push(Line::from(""));
            }
            lines
        };
        frame.render_widget(
            Paragraph::new(preview_lines).block(
                Block::default()
                    .title(if state.focus == LinkWizardFocus::Confirm {
                        "Apply *"
                    } else {
                        "Apply"
                    })
                    .borders(Borders::ALL)
                    .border_style(if state.focus == LinkWizardFocus::Confirm {
                        Style::default().fg(Color::Yellow)
                    } else {
                        Style::default()
                    }),
            ),
            rows[4],
        );

        frame.render_widget(
            Paragraph::new("j/k or arrows:move  Tab:focus  Enter:next/apply  type:search target  /:target focus  b/B:different block direction  d/r/c:action  Esc:cancel")
                .wrap(Wrap { trim: false })
                .style(Style::default().fg(MUTED_TEXT_COLOR)),
            rows[5],
        );
    }

    fn link_wizard_cursor_position(&self, area: Rect) -> Option<(u16, u16)> {
        if self.mode != Mode::LinkWizard || area.width < 6 || area.height < 8 {
            return None;
        }
        let state = self.link_wizard_state()?;
        let action = LinkWizardAction::from_index(state.action_index);
        if state.focus != LinkWizardFocus::Target || !action.requires_target() {
            return None;
        }

        let inner = Rect {
            x: area.x.saturating_add(1),
            y: area.y.saturating_add(1),
            width: area.width.saturating_sub(2),
            height: area.height.saturating_sub(2),
        };
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2), // anchor
                Constraint::Length(7), // actions
                Constraint::Length(3), // target query
                Constraint::Min(5),    // target matches
                Constraint::Length(4), // preview
                Constraint::Length(2), // help
            ])
            .split(inner);
        let target_query = rows[2];
        let input_x = target_query.x.saturating_add(1);
        let input_y = target_query.y.saturating_add(1);
        let prefix_len = "Search> ".chars().count().min(u16::MAX as usize) as u16;
        let cursor_chars = state.target_filter.cursor().min(u16::MAX as usize) as u16;
        let max_x = target_query
            .x
            .saturating_add(target_query.width.saturating_sub(2));
        let x = input_x
            .saturating_add(prefix_len)
            .saturating_add(cursor_chars)
            .min(max_x);
        Some((x, input_y))
    }

    fn category_direct_edit_cursor_position(&self, area: Rect) -> Option<(u16, u16)> {
        if self.mode != Mode::CategoryDirectEdit || area.width < 4 || area.height < 4 {
            return None;
        }
        let inner = Rect {
            x: area.x.saturating_add(1),
            y: area.y.saturating_add(1),
            width: area.width.saturating_sub(2),
            height: area.height.saturating_sub(2),
        };
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(6),
                Constraint::Length(3),
                Constraint::Min(6),
                Constraint::Length(2),
            ])
            .split(inner);
        let input_area = chunks[3];
        let input_x = input_area.x.saturating_add(1);
        let input_y = input_area.y.saturating_add(1);
        let prefix_len = "Category> ".chars().count().min(u16::MAX as usize) as u16;
        let cursor_chars = self
            .active_category_direct_edit_row()
            .map(|row| row.input.cursor())
            .unwrap_or_else(|| self.input.cursor())
            .min(u16::MAX as usize) as u16;
        let max_x = input_area
            .x
            .saturating_add(input_area.width.saturating_sub(2));
        let x = input_x
            .saturating_add(prefix_len)
            .saturating_add(cursor_chars)
            .min(max_x);
        Some((x, input_y))
    }

    fn render_category_create_confirm_panel(
        &self,
        frame: &mut ratatui::Frame<'_>,
        area: Rect,
        name: &str,
        description_suffix: &str,
    ) {
        frame.render_widget(
            Paragraph::new(format!(
                "Create \"{}\" {}\ny:confirm  Esc:cancel",
                name, description_suffix
            ))
            .style(Style::default().fg(MUTED_TEXT_COLOR))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Create Category"),
            ),
            area,
        );
    }

    fn render_category_column_picker(&self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        frame.render_widget(Clear, area);
        frame.render_widget(
            Block::default()
                .title("Set Category")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
            area,
        );
        if area.width < 4 || area.height < 6 {
            return;
        }
        let Some(state) = self.category_column_picker_state() else {
            return;
        };

        let inner = Rect {
            x: area.x.saturating_add(1),
            y: area.y.saturating_add(1),
            width: area.width.saturating_sub(2),
            height: area.height.saturating_sub(2),
        };
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2),
                Constraint::Length(5),
                Constraint::Length(3),
                Constraint::Min(4),
                Constraint::Length(2),
            ])
            .split(inner);

        let selected_names: Vec<String> = self
            .categories
            .iter()
            .filter(|c| state.selected_ids.contains(&c.id))
            .map(|c| c.name.clone())
            .collect();
        let selected_display = if selected_names.is_empty() {
            "(none)".to_string()
        } else {
            selected_names.join(", ")
        };
        frame.render_widget(
            Paragraph::new(format!(
                "Column: {}  Mode: {}\nSelected: {}",
                state.parent_name,
                if state.is_exclusive {
                    "single"
                } else {
                    "multi"
                },
                truncate_board_cell(&selected_display, 56),
            ))
            .style(Style::default().fg(MUTED_TEXT_COLOR))
            .wrap(Wrap { trim: true }),
            chunks[0],
        );

        frame.render_widget(
            Paragraph::new(state.item_label.clone())
                .style(Style::default().fg(MUTED_TEXT_COLOR))
                .block(Block::default().borders(Borders::ALL).title("Item Context"))
                .wrap(Wrap { trim: false })
                .scroll((state.item_preview_scroll, 0)),
            chunks[1],
        );

        let input_border = if state.focus == CategoryColumnPickerFocus::FilterInput {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default()
        };
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("Filter> ", Style::default().fg(Color::Yellow)),
                Span::raw(state.filter.text()),
            ]))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Filter")
                    .border_style(input_border),
            ),
            chunks[2],
        );

        if let Some(name) = state.create_confirm_name.as_deref() {
            self.render_category_create_confirm_panel(
                frame,
                chunks[3],
                name,
                "as a new child category in this column?",
            );
        } else {
            let matches = self.category_column_picker_matches();
            let items: Vec<ListItem<'_>> = if matches.is_empty() {
                let msg = if state.filter.text().trim().is_empty() {
                    "(type to filter child categories)"
                } else {
                    "(no matches)"
                };
                vec![ListItem::new(msg)]
            } else {
                matches
                    .iter()
                    .map(|id| {
                        let label = self
                            .categories
                            .iter()
                            .find(|c| c.id == *id)
                            .map(|c| c.name.as_str())
                            .unwrap_or("(missing)");
                        let mark = if state.is_exclusive {
                            if state.selected_ids.contains(id) {
                                "(*)"
                            } else {
                                "( )"
                            }
                        } else if state.selected_ids.contains(id) {
                            "[x]"
                        } else {
                            "[ ]"
                        };
                        ListItem::new(format!("{mark} {label}"))
                    })
                    .collect()
            };
            let selected = if matches.is_empty() {
                None
            } else {
                Some(state.list_index.min(matches.len() - 1))
            };
            let mut list_state = Self::list_state_for(chunks[3], selected);
            frame.render_stateful_widget(
                List::new(items)
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title("Categories")
                            .border_style(if state.focus == CategoryColumnPickerFocus::List {
                                Style::default().fg(Color::Cyan)
                            } else {
                                Style::default()
                            }),
                    )
                    .highlight_symbol("> ")
                    .highlight_style(selected_row_style()),
                chunks[3],
                &mut list_state,
            );
            Self::render_vertical_scrollbar(
                frame,
                chunks[3],
                matches.len().max(1),
                list_state.offset(),
            );
        }

        frame.render_widget(
            Paragraph::new(
                "Type filter | j/k or Up/Down move | PgUp/PgDn item | Space toggle | Enter save | Esc cancel",
            )
            .style(Style::default().fg(MUTED_TEXT_COLOR))
            .wrap(Wrap { trim: true }),
            chunks[4],
        );
    }

    fn category_column_picker_cursor_position(&self, area: Rect) -> Option<(u16, u16)> {
        if self.mode != Mode::CategoryColumnPicker || area.width < 4 || area.height < 4 {
            return None;
        }
        let state = self.category_column_picker_state()?;
        if state.focus != CategoryColumnPickerFocus::FilterInput {
            return None;
        }
        let inner = Rect {
            x: area.x.saturating_add(1),
            y: area.y.saturating_add(1),
            width: area.width.saturating_sub(2),
            height: area.height.saturating_sub(2),
        };
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2),
                Constraint::Length(5),
                Constraint::Length(3),
                Constraint::Min(4),
                Constraint::Length(2),
            ])
            .split(inner);
        let input_area = chunks[2];
        let input_x = input_area.x.saturating_add(1);
        let input_y = input_area.y.saturating_add(1);
        let prefix_len = "Filter> ".chars().count().min(u16::MAX as usize) as u16;
        let cursor_chars = state.filter.cursor().min(u16::MAX as usize) as u16;
        let max_x = input_area
            .x
            .saturating_add(input_area.width.saturating_sub(2));
        Some((
            input_x
                .saturating_add(prefix_len)
                .saturating_add(cursor_chars)
                .min(max_x),
            input_y,
        ))
    }

    fn render_board_add_column_picker(&self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        frame.render_widget(Clear, area);
        frame.render_widget(
            Block::default()
                .title("Add Column")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
            area,
        );
        if area.width < 4 || area.height < 6 {
            return;
        }

        let inner = Rect {
            x: area.x.saturating_add(1),
            y: area.y.saturating_add(1),
            width: area.width.saturating_sub(2),
            height: area.height.saturating_sub(2),
        };
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2),
                Constraint::Length(3),
                Constraint::Min(5),
                Constraint::Length(2),
            ])
            .split(inner);

        let header = self
            .board_add_column
            .as_ref()
            .map(|state| {
                let dir = match state.anchor.direction {
                    AddColumnDirection::Left => "left",
                    AddColumnDirection::Right => "right",
                };
                let section = self
                    .current_view()
                    .and_then(|v| v.sections.get(state.anchor.section_index))
                    .map(|s| s.title.as_str())
                    .unwrap_or("(missing)");
                format!(
                    "Section: {}  Insert {} of current column  Index: {}",
                    section, dir, state.anchor.insert_index
                )
            })
            .unwrap_or_else(|| "Insert a category column".to_string());
        frame.render_widget(
            Paragraph::new(header)
                .style(Style::default().fg(MUTED_TEXT_COLOR))
                .wrap(Wrap { trim: true }),
            chunks[0],
        );

        let input_text = self.board_add_column_input_text().unwrap_or("");
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("Category> ", Style::default().fg(Color::Yellow)),
                Span::raw(input_text),
            ]))
            .block(Block::default().borders(Borders::ALL).title("Typeahead")),
            chunks[1],
        );

        if let Some(name) = self.board_add_column_create_confirm_name() {
            self.render_category_create_confirm_panel(
                frame,
                chunks[2],
                name,
                "as a new top-level category and insert its column?",
            );
        } else {
            let matches = self.get_board_add_column_suggest_matches();
            let items: Vec<ListItem<'_>> = if matches.is_empty() {
                let msg = if input_text.trim().is_empty() {
                    "(type to filter categories)"
                } else {
                    "(no matches)"
                };
                vec![ListItem::new(msg)]
            } else {
                matches
                    .iter()
                    .map(|id| {
                        let label = self
                            .categories
                            .iter()
                            .find(|c| c.id == *id)
                            .map(|c| c.name.as_str())
                            .unwrap_or("(missing)");
                        ListItem::new(label)
                    })
                    .collect()
            };
            let selected = self.board_add_column.as_ref().and_then(|s| {
                (!matches.is_empty()).then(|| s.suggest_index.min(matches.len() - 1))
            });
            let mut list_state = Self::list_state_for(chunks[2], selected);
            frame.render_stateful_widget(
                List::new(items)
                    .block(Block::default().borders(Borders::ALL).title("Categories"))
                    .highlight_symbol("> ")
                    .highlight_style(selected_row_style()),
                chunks[2],
                &mut list_state,
            );
        }

        frame.render_widget(
            Paragraph::new(
                "Type filter | Up/Down select | Tab autocomplete | Enter insert | Esc cancel",
            )
            .style(Style::default().fg(MUTED_TEXT_COLOR))
            .wrap(Wrap { trim: true }),
            chunks[3],
        );
    }

    fn board_add_column_cursor_position(&self, area: Rect) -> Option<(u16, u16)> {
        if self.mode != Mode::BoardAddColumnPicker || area.width < 4 || area.height < 4 {
            return None;
        }
        let inner = Rect {
            x: area.x.saturating_add(1),
            y: area.y.saturating_add(1),
            width: area.width.saturating_sub(2),
            height: area.height.saturating_sub(2),
        };
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2),
                Constraint::Length(3),
                Constraint::Min(5),
                Constraint::Length(2),
            ])
            .split(inner);
        let input_area = chunks[1];
        let input_x = input_area.x.saturating_add(1);
        let input_y = input_area.y.saturating_add(1);
        let prefix_len = "Category> ".chars().count().min(u16::MAX as usize) as u16;
        let cursor_chars = self
            .board_add_column
            .as_ref()
            .map(|s| s.input.cursor())
            .unwrap_or(0)
            .min(u16::MAX as usize) as u16;
        let max_x = input_area
            .x
            .saturating_add(input_area.width.saturating_sub(2));
        Some((
            input_x
                .saturating_add(prefix_len)
                .saturating_add(cursor_chars)
                .min(max_x),
            input_y,
        ))
    }

    pub(crate) fn category_manager_action_cursor_position(&self, area: Rect) -> Option<(u16, u16)> {
        if self.mode != Mode::CategoryManager || area.width < 3 || area.height < 3 {
            return None;
        }
        let inner = Rect {
            x: area.x.saturating_add(1),
            y: area.y.saturating_add(1),
            width: area.width.saturating_sub(2),
            height: area.height.saturating_sub(2),
        };
        if inner.width == 0 || inner.height == 0 {
            return None;
        }
        let max_x = inner.x.saturating_add(inner.width.saturating_sub(1));

        if let Some(action) = self.category_manager_inline_action() {
            match action {
                CategoryInlineAction::Rename { buf, .. } => {
                    let prefix_len = "Rename> ".chars().count().min(u16::MAX as usize) as u16;
                    let cursor_chars = buf.cursor().min(u16::MAX as usize) as u16;
                    let cursor_x = inner
                        .x
                        .saturating_add(prefix_len)
                        .saturating_add(cursor_chars)
                        .min(max_x);
                    return Some((cursor_x, inner.y));
                }
                CategoryInlineAction::DeleteConfirm { .. } => {}
            }
            return None;
        }

        if self.category_manager_focus() == Some(CategoryManagerFocus::Filter)
            && self.category_manager_filter_editing()
        {
            let prefix_len = "Filter: ".chars().count().min(u16::MAX as usize) as u16;
            let cursor_chars = self
                .category_manager
                .as_ref()
                .map(|state| state.filter.cursor())
                .unwrap_or(0)
                .min(u16::MAX as usize) as u16;
            let cursor_x = inner
                .x
                .saturating_add(prefix_len)
                .saturating_add(cursor_chars)
                .min(max_x);
            return Some((cursor_x, inner.y));
        }

        None
    }

    pub(crate) fn category_manager_details_cursor_position(
        &self,
        area: Rect,
    ) -> Option<(u16, u16)> {
        if self.mode != Mode::CategoryManager || area.width < 3 || area.height < 3 {
            return None;
        }
        let input = self.category_manager_details_inline_input()?;
        let inner = Rect {
            x: area.x.saturating_add(1),
            y: area.y.saturating_add(1),
            width: area.width.saturating_sub(2),
            height: area.height.saturating_sub(2),
        };
        if inner.width == 0 || inner.height == 0 {
            return None;
        }

        let is_numeric_category = self
            .selected_category_row()
            .map(|row| row.value_kind == CategoryValueKind::Numeric)
            .unwrap_or(false);
        if !is_numeric_category {
            return None;
        }
        let flags_height = 7;
        let details_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(CATEGORY_DETAILS_INFO_HEIGHT_NUMERIC),
                Constraint::Length(flags_height),
                Constraint::Min(5),
                Constraint::Length(2),
            ])
            .split(inner);
        let flags_inner = Rect {
            x: details_chunks[1].x.saturating_add(1),
            y: details_chunks[1].y.saturating_add(1),
            width: details_chunks[1].width.saturating_sub(2),
            height: details_chunks[1].height.saturating_sub(2),
        };
        if flags_inner.width == 0 || flags_inner.height == 0 {
            return None;
        }
        let max_x = flags_inner
            .x
            .saturating_add(flags_inner.width.saturating_sub(1));
        let (line_offset, prefix) = match input.field {
            CategoryManagerDetailsInlineField::DecimalPlaces => (1u16, "  Decimal places: "),
            CategoryManagerDetailsInlineField::CurrencySymbol => (2u16, "  Currency symbol: "),
        };
        let prefix_len = prefix.chars().count().min(u16::MAX as usize) as u16;
        let cursor_chars = input.buffer.cursor().min(u16::MAX as usize) as u16;
        Some((
            flags_inner
                .x
                .saturating_add(prefix_len)
                .saturating_add(cursor_chars)
                .min(max_x),
            flags_inner.y.saturating_add(line_offset),
        ))
    }

    pub(crate) fn input_prompt_prefix(&self) -> Option<String> {
        match self.mode {
            Mode::SearchBarFocused => None, // cursor rendered by search bar, not footer
            Mode::ItemAssignInput => Some("Category> ".to_string()),
            Mode::Normal
            | Mode::HelpPanel
            | Mode::SuggestionReview
            | Mode::InputPanel
            | Mode::LinkWizard
            | Mode::ItemAssignPicker
            | Mode::InspectUnassign
            | Mode::ViewPicker
            | Mode::ViewEdit
            | Mode::ViewDeleteConfirm
            | Mode::ConfirmDelete
            | Mode::BoardColumnDeleteConfirm
            | Mode::CategoryManager
            | Mode::CategoryDirectEdit
            | Mode::CategoryColumnPicker
            | Mode::BoardAddColumnPicker => None,
        }
    }

    pub(crate) fn input_cursor_position(&self, footer_area: Rect) -> Option<(u16, u16)> {
        let prefix = self.input_prompt_prefix()?;
        if footer_area.width < 3 || footer_area.height < 3 {
            return None;
        }

        let inner_x = footer_area.x.saturating_add(1);
        let inner_y = footer_area.y.saturating_add(1);
        let max_inner_x = footer_area
            .x
            .saturating_add(footer_area.width.saturating_sub(2));

        let input_chars = self.clamped_input_cursor().min(u16::MAX as usize) as u16;
        let prefix_chars = prefix.chars().count().min(u16::MAX as usize) as u16;
        let raw_x = inner_x
            .saturating_add(prefix_chars)
            .saturating_add(input_chars);
        let cursor_x = raw_x.min(max_inner_x);

        Some((cursor_x, inner_y))
    }

    pub(crate) fn input_panel_cursor_position(
        &self,
        popup_area: Rect,
        panel: &input_panel::InputPanel,
    ) -> Option<(u16, u16)> {
        use input_panel::InputPanelFocus;

        if popup_area.width < 3 || popup_area.height < 3 {
            return None;
        }
        let regions = input_panel_popup_regions(popup_area, panel.kind)?;
        match panel.focus {
            InputPanelFocus::Text => {
                let prefix_str = match panel.kind {
                    input_panel::InputPanelKind::NameInput
                    | input_panel::InputPanelKind::CategoryCreate => "  Name> ",
                    input_panel::InputPanelKind::WhenDate => "  When> ",
                    input_panel::InputPanelKind::NumericValue => "  Value> ",
                    _ => "  Text> ",
                };
                let prefix_len = prefix_str.chars().count().min(u16::MAX as usize) as u16;
                let (_, visible_cursor) = clip_text_for_row(
                    panel.text.text(),
                    panel.text.cursor(),
                    (regions.text.width as usize).saturating_sub(prefix_len as usize),
                    true,
                );
                let input_chars = visible_cursor.min(u16::MAX as usize) as u16;
                let max_x = regions
                    .text
                    .x
                    .saturating_add(regions.text.width.saturating_sub(1));
                let cursor_x = regions
                    .text
                    .x
                    .saturating_add(prefix_len)
                    .saturating_add(input_chars)
                    .min(max_x);
                Some((cursor_x, regions.text.y))
            }
            InputPanelFocus::Note => {
                let note_rect = regions.note?;
                if note_rect.width < 3 || note_rect.height < 3 {
                    return None;
                }
                let note_inner = Rect {
                    x: note_rect.x.saturating_add(1),
                    y: note_rect.y.saturating_add(1),
                    width: note_rect.width.saturating_sub(2),
                    height: note_rect.height.saturating_sub(2),
                };
                if note_inner.width == 0 || note_inner.height == 0 {
                    return None;
                }
                let (line, col) = panel.note.line_col();
                let scroll = list_scroll_for_selected_line(note_rect, Some(line)) as usize;
                let visible_row = line.saturating_sub(scroll);
                let max_x = note_inner
                    .x
                    .saturating_add(note_inner.width.saturating_sub(1));
                let max_y = note_inner
                    .y
                    .saturating_add(note_inner.height.saturating_sub(1));
                let cursor_x = note_inner
                    .x
                    .saturating_add(col.min(u16::MAX as usize) as u16)
                    .min(max_x);
                let cursor_y = note_inner
                    .y
                    .saturating_add(visible_row.min(u16::MAX as usize) as u16)
                    .min(max_y);
                Some((cursor_x, cursor_y))
            }
            InputPanelFocus::Categories => {
                if panel.category_filter_editing {
                    let filter_rect = regions.categories_filter?;
                    let prefix = "> Filter> ";
                    let max_x = filter_rect
                        .x
                        .saturating_add(filter_rect.width.saturating_sub(1));
                    let cursor_x =
                        filter_rect
                            .x
                            .saturating_add(prefix.chars().count().min(u16::MAX as usize) as u16)
                            .saturating_add(
                                panel.category_filter.cursor().min(u16::MAX as usize) as u16
                            )
                            .min(max_x);
                    return Some((cursor_x, filter_rect.y));
                }

                // Show cursor in the numeric value field if on an assigned numeric category row.
                let cat_list = if panel.category_filter_editing {
                    regions.categories_list?
                } else {
                    regions.categories_inner?
                };
                let selected_row = self.input_panel_selected_category_row()?;
                let is_assigned = panel.categories.contains(&selected_row.id);
                let is_numeric =
                    selected_row.value_kind == agenda_core::model::CategoryValueKind::Numeric;
                if is_assigned && is_numeric {
                    if let Some(buf) = panel.numeric_buffers.get(&selected_row.id) {
                        // Value field is right-aligned: "[value_]"
                        // The cursor should be positioned within that field.
                        let value_text_len = buf.text().chars().count() + 2; // "[" + text + "]"
                        let buf_cursor = buf.cursor();
                        // Position: end of inner rect - value_text_len + 1 (for "[") + buf_cursor
                        let field_start_x = cat_list
                            .x
                            .saturating_add(cat_list.width)
                            .saturating_sub(value_text_len as u16);
                        let cursor_x = field_start_x
                            .saturating_add(1) // skip "["
                            .saturating_add(buf_cursor.min(u16::MAX as usize) as u16)
                            .min(cat_list.x.saturating_add(cat_list.width.saturating_sub(1)));
                        let scroll =
                            list_scroll_for_selected_line(cat_list, Some(panel.category_cursor))
                                as usize;
                        let visible_row = panel.category_cursor.saturating_sub(scroll);
                        let cursor_y = cat_list
                            .y
                            .saturating_add(visible_row.min(u16::MAX as usize) as u16)
                            .min(cat_list.y.saturating_add(cat_list.height.saturating_sub(1)));
                        return Some((cursor_x, cursor_y));
                    }
                }
                None
            }
            InputPanelFocus::When
            | InputPanelFocus::TypePicker
            | InputPanelFocus::SaveButton
            | InputPanelFocus::CancelButton => None,
        }
    }

    pub(crate) fn render_header(&self) -> Paragraph<'_> {
        let current_view = self.current_view();
        let view_name = current_view
            .map(|view| view.name.as_str())
            .unwrap_or("(none)");
        let mode = format!("{:?}", self.mode);
        let active_filters = self.section_filters.iter().filter(|f| f.is_some()).count();
        let filter = if active_filters > 0 {
            format!(" filters:{active_filters}")
        } else {
            String::new()
        };
        let view_flags = if self.effective_hide_dependent_items() {
            " dep:hidden"
        } else {
            ""
        };
        let workflow_hint = self.ready_queue_header_hint().unwrap_or_default();
        Paragraph::new(Line::from(vec![
            Span::styled(
                "Agenda Reborn",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!(
                "  view:{view_name}{view_flags}{workflow_hint}  mode:{mode}{filter}"
            )),
        ]))
    }

    fn render_search_bar(&self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        let section_name = if self.global_search_active() {
            "Global/All Items"
        } else {
            self.current_slot()
                .map(|s| s.title.as_str())
                .unwrap_or("section")
        };
        let label = format!("[{section_name}] ");
        let is_focused = self.mode == Mode::SearchBarFocused;

        let label_style = Style::default().fg(Color::Cyan);
        let (text_content, text_style) = if is_focused {
            let text = self.search_buffer.text();
            if text.is_empty() {
                (
                    "Search or create...".to_string(),
                    Style::default().fg(Color::DarkGray),
                )
            } else {
                (text.to_string(), Style::default().fg(Color::White))
            }
        } else {
            let filter = self
                .section_filters
                .get(self.slot_index)
                .and_then(|f| f.as_deref());
            if let Some(text) = filter {
                (text.to_string(), Style::default().fg(Color::Yellow))
            } else {
                (
                    "Search or create...".to_string(),
                    Style::default().fg(Color::DarkGray),
                )
            }
        };

        let line = Line::from(vec![
            Span::styled(label.clone(), label_style),
            Span::styled(text_content, text_style),
        ]);
        frame.render_widget(Paragraph::new(line), area);

        // Position cursor when focused
        if is_focused && !self.search_buffer.text().is_empty() {
            let label_len = label.chars().count() as u16;
            let cursor_offset = self.search_buffer.cursor().min(u16::MAX as usize) as u16;
            let x = area
                .x
                .saturating_add(label_len)
                .saturating_add(cursor_offset);
            let x = x.min(area.x.saturating_add(area.width.saturating_sub(1)));
            frame.set_cursor_position((x, area.y));
        } else if is_focused {
            // Empty buffer but focused: cursor at start of text area
            let label_len = label.chars().count() as u16;
            let x = area.x.saturating_add(label_len);
            frame.set_cursor_position((x, area.y));
        }
    }

    pub(crate) fn render_main(&self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        if self.mode == Mode::ViewEdit {
            self.render_view_edit_screen(frame, area);
            return;
        }
        let category_create_panel_open = self.mode == Mode::InputPanel
            && self.name_input_context == Some(NameInputContext::CategoryCreate)
            && self
                .input_panel
                .as_ref()
                .map(|panel| panel.kind == input_panel::InputPanelKind::CategoryCreate)
                .unwrap_or(false);
        if matches!(self.mode, Mode::CategoryManager) || category_create_panel_open {
            self.render_category_manager(frame, area);
            return;
        }
        if self.show_preview {
            let split = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
                .split(area);
            self.render_board_columns(frame, split[0]);
            match self.preview_mode {
                PreviewMode::Summary => {
                    frame.render_widget(self.render_preview_summary_panel(), split[1]);
                    let content_len = self
                        .selected_item()
                        .map(|item| self.item_details_lines_for_item(item).len())
                        .unwrap_or(4);
                    Self::render_vertical_scrollbar(
                        frame,
                        split[1],
                        content_len,
                        self.preview_summary_scroll,
                    );
                }
                PreviewMode::Provenance => {
                    self.render_preview_provenance_panel(frame, split[1]);
                }
            }
        } else {
            self.render_board_columns(frame, area);
        }
    }

    pub(crate) fn render_board_columns(&self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        if self.slots.is_empty() {
            frame.render_widget(
                Paragraph::new(vec![Line::from("(no sections)")]).block(
                    Block::default()
                        .title("Board")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Blue)),
                ),
                area,
            );
            return;
        }

        let slot_count = self.slots.len() as u16;
        let pct = (100 / slot_count).max(1);
        let constraints: Vec<Constraint> = (0..self.slots.len())
            .map(|_| Constraint::Percentage(pct))
            .collect();
        let slot_direction = if self.is_horizontal_section_flow() {
            Direction::Horizontal
        } else {
            Direction::Vertical
        };
        let columns = Layout::default()
            .direction(slot_direction)
            .constraints(constraints)
            .split(area);

        let current_view = self.current_view().cloned();
        let mut category_display_names = category_name_map(&self.categories);
        if let Some(view) = current_view.as_ref() {
            for (category_id, alias) in &view.category_aliases {
                let alias = alias.trim();
                if !alias.is_empty() {
                    category_display_names.insert(*category_id, alias.to_string());
                }
            }
        }
        let view_item_label = current_view
            .as_ref()
            .and_then(|v| v.item_column_label.clone())
            .filter(|label| !label.trim().is_empty())
            .unwrap_or_else(|| "Item".to_string());
        if self.is_horizontal_section_flow() {
            self.render_horizontal_board_lanes(frame, &columns, &category_display_names);
            return;
        }
        for (slot_index, slot) in self.slots.iter().enumerate() {
            let effective_display_mode = match (&slot.context, current_view.as_ref()) {
                (SlotContext::Section { section_index }, Some(view))
                | (SlotContext::GeneratedSection { section_index, .. }, Some(view)) => view
                    .sections
                    .get(*section_index)
                    .and_then(|section| section.board_display_mode_override)
                    .unwrap_or(view.board_display_mode),
                _ => current_view
                    .as_ref()
                    .map(|v| v.board_display_mode)
                    .unwrap_or(BoardDisplayMode::SingleLine),
            };
            let is_selected_slot = slot_index == self.slot_index;
            let inner_width = columns[slot_index].width.saturating_sub(2);
            let selected_row = if is_selected_slot && !slot.items.is_empty() {
                Some(self.item_index.min(slot.items.len().saturating_sub(1)))
            } else {
                None
            };
            let filter_suffix = self
                .section_filters
                .get(slot_index)
                .and_then(|f| f.as_deref())
                .map(|needle| format!("  filter:{needle}"))
                .unwrap_or_default();
            let title = format!("{} ({}){}", slot.title, slot.items.len(), filter_suffix);
            let border_color = if is_selected_slot {
                Color::Cyan
            } else {
                Color::Blue
            };
            let (slot_columns_owned, slot_item_column_index) =
                match (&slot.context, current_view.as_ref()) {
                    (SlotContext::Section { section_index }, Some(view))
                    | (SlotContext::GeneratedSection { section_index, .. }, Some(view)) => view
                        .sections
                        .get(*section_index)
                        .map(|section| {
                            (
                                section.columns.clone(),
                                Self::section_item_column_index(section),
                            )
                        })
                        .unwrap_or_default(),
                    _ => (Vec::new(), 0),
                };
            let use_dynamic = !slot_columns_owned.is_empty();
            let include_all_categories_in_dynamic = use_dynamic
                && !slot_columns_owned
                    .iter()
                    .any(|column| column.kind == ColumnKind::Standard);
            if use_dynamic {
                let layout = compute_board_layout(
                    &slot_columns_owned,
                    &self.categories,
                    &category_display_names,
                    &view_item_label,
                    inner_width,
                );
                let mut item_width = layout.item;
                let mut synthetic_categories_width = 0usize;
                if include_all_categories_in_dynamic {
                    let extra_spacing_budget = BOARD_TABLE_COLUMN_SPACING as usize;
                    let min_item_width = BOARD_ITEM_MIN_WIDTH.min(item_width);
                    let available_for_categories = item_width.saturating_sub(min_item_width);
                    if available_for_categories > extra_spacing_budget {
                        synthetic_categories_width = BOARD_CATEGORY_TARGET_WIDTH
                            .min(available_for_categories - extra_spacing_budget);
                        if synthetic_categories_width > 0 {
                            item_width = item_width
                                .saturating_sub(synthetic_categories_width + extra_spacing_budget);
                        }
                    }
                }
                let item_board_column_index = slot_item_column_index.min(layout.columns.len());
                let mut constraints = vec![
                    Constraint::Length(layout.marker.min(u16::MAX as usize) as u16),
                    Constraint::Length(layout.note.min(u16::MAX as usize) as u16),
                ];
                constraints.extend(
                    layout.columns[..item_board_column_index]
                        .iter()
                        .map(|column| {
                            Constraint::Length(column.width.min(u16::MAX as usize) as u16)
                        }),
                );
                constraints.push(Constraint::Length(item_width.min(u16::MAX as usize) as u16));
                constraints.extend(
                    layout.columns[item_board_column_index..]
                        .iter()
                        .map(|column| {
                            Constraint::Length(column.width.min(u16::MAX as usize) as u16)
                        }),
                );
                if synthetic_categories_width > 0 {
                    constraints.push(Constraint::Length(
                        synthetic_categories_width.min(u16::MAX as usize) as u16,
                    ));
                }
                let mut header_cells = vec![Cell::from(String::new()), Cell::from(String::new())];
                header_cells.extend(layout.columns[..item_board_column_index].iter().map(
                    |column| {
                        Cell::from(format_board_header_cell(
                            &column.label,
                            column.heading_value_kind,
                            column.width,
                        ))
                    },
                ));
                header_cells.push(Cell::from(layout.item_label.clone()));
                header_cells.extend(layout.columns[item_board_column_index..].iter().map(
                    |column| {
                        Cell::from(format_board_header_cell(
                            &column.label,
                            column.heading_value_kind,
                            column.width,
                        ))
                    },
                ));
                if synthetic_categories_width > 0 {
                    header_cells.push(Cell::from("All Categories"));
                }

                let rows: Vec<Row<'_>> = if slot.items.is_empty() {
                    let has_filter = self
                        .section_filters
                        .get(slot_index)
                        .map(|f| f.is_some())
                        .unwrap_or(false);
                    let all_slots_empty = self.slots.iter().all(|s| s.items.is_empty());
                    let empty_msg = if all_slots_empty {
                        "No items. n:add item  v:switch view  q:quit"
                    } else if has_filter {
                        "No matches. Esc:clear filter"
                    } else {
                        "No items in this section."
                    };
                    let mut cells = vec![Cell::from(String::new()), Cell::from(String::new())];
                    cells.extend(
                        layout.columns[..item_board_column_index]
                            .iter()
                            .map(|_| Cell::from(String::new())),
                    );
                    cells.push(Cell::from(Span::styled(
                        empty_msg,
                        Style::default().fg(MUTED_TEXT_COLOR),
                    )));
                    cells.extend(
                        layout.columns[item_board_column_index..]
                            .iter()
                            .map(|_| Cell::from(String::new())),
                    );
                    if synthetic_categories_width > 0 {
                        cells.push(Cell::from(String::new()));
                    }
                    vec![Row::new(cells)]
                } else {
                    slot.items
                        .iter()
                        .enumerate()
                        .map(|(item_index, item)| {
                            let is_focused_item = is_selected_slot && item_index == self.item_index;
                            let is_marked_selected = self.is_item_selected(item.id);
                            let marker_cell = match (is_focused_item, is_marked_selected) {
                                (true, _) => ">",
                                (false, true) => "+",
                                (false, false) => " ",
                            };
                            let note_cell = item_indicator_glyphs(
                                item.is_done,
                                self.is_item_blocked(item.id),
                                self.pending_suggestion_count_for_item(item.id) > 0,
                                has_note_text(item.note.as_deref()),
                            );
                            let item_cell_content =
                                if effective_display_mode == BoardDisplayMode::MultiLine {
                                    wrap_text_for_board_cell(&board_item_label(item), item_width)
                                        .join("\n")
                                } else {
                                    truncate_board_cell(&board_item_label(item), item_width)
                                };
                            let mut row_height = item_cell_content.lines().count().max(1);
                            let item_cell = {
                                let mut cell = Cell::from(item_cell_content);
                                if is_focused_item && self.column_index == item_board_column_index {
                                    cell = cell.style(focused_cell_style());
                                }
                                cell
                            };
                            let mut category_cells: Vec<Cell<'_>> = layout
                                .columns
                                .iter()
                                .enumerate()
                                .map(|(col_idx, column)| {
                                    let is_numeric_column =
                                        column.heading_value_kind == CategoryValueKind::Numeric;
                                    let value = match column.kind {
                                        ColumnKind::When => item
                                            .when_date
                                            .map(|dt| dt.date().to_string())
                                            .unwrap_or_else(|| "\u{2013}".to_string()),
                                        ColumnKind::Standard if is_numeric_column => {
                                            let numeric_val = item
                                                .assignments
                                                .get(&column.heading_id)
                                                .and_then(|a| a.numeric_value);
                                            format_numeric_cell(
                                                numeric_val,
                                                column.numeric_format.as_ref(),
                                            )
                                        }
                                        ColumnKind::Standard => standard_column_value(
                                            item,
                                            &column.child_ids,
                                            &category_display_names,
                                        ),
                                    };
                                    let content = if effective_display_mode
                                        == BoardDisplayMode::MultiLine
                                        && column.kind == ColumnKind::Standard
                                        && !is_numeric_column
                                    {
                                        let lines = if value == "\u{2013}" {
                                            vec!["-".to_string()]
                                        } else {
                                            let labels: Vec<String> =
                                                value.split(", ").map(str::to_string).collect();
                                            format_category_values_multi_line(
                                                &labels,
                                                BOARD_MULTI_CATEGORY_LINE_CAP,
                                            )
                                        };
                                        lines.join("\n")
                                    } else if is_numeric_column {
                                        right_pad_cell(&value, column.width)
                                    } else {
                                        truncate_board_cell(&value, column.width)
                                    };
                                    row_height = row_height.max(content.lines().count().max(1));
                                    let mut cell = Cell::from(content);
                                    let board_column_index = if col_idx < item_board_column_index {
                                        col_idx
                                    } else {
                                        col_idx + 1
                                    };
                                    if is_focused_item && self.column_index == board_column_index {
                                        cell = cell.style(focused_cell_style());
                                    }
                                    cell
                                })
                                .collect();
                            let mut cells = vec![Cell::from(marker_cell), Cell::from(note_cell)];
                            let right_cells = category_cells.split_off(item_board_column_index);
                            cells.extend(category_cells);
                            cells.push(item_cell);
                            cells.extend(right_cells);
                            if synthetic_categories_width > 0 {
                                let categories =
                                    item_assignment_labels(item, &category_display_names);
                                let categories_text = if categories.is_empty() {
                                    "-".to_string()
                                } else if effective_display_mode == BoardDisplayMode::MultiLine {
                                    format_category_values_multi_line(
                                        &categories,
                                        BOARD_MULTI_CATEGORY_LINE_CAP,
                                    )
                                    .join("\n")
                                } else {
                                    categories.join(", ")
                                };
                                let content =
                                    if effective_display_mode == BoardDisplayMode::MultiLine {
                                        categories_text
                                    } else {
                                        truncate_board_cell(
                                            &categories_text,
                                            synthetic_categories_width,
                                        )
                                    };
                                row_height = row_height.max(content.lines().count().max(1));
                                cells.push(Cell::from(content));
                            }
                            let mut row = Row::new(cells);
                            if effective_display_mode == BoardDisplayMode::MultiLine {
                                row = row.height(row_height.min(u16::MAX as usize) as u16);
                            }
                            if is_focused_item {
                                row = row.style(selected_board_row_style());
                            } else if is_marked_selected {
                                row = row.style(marked_board_row_style());
                            }
                            row
                        })
                        .collect()
                };

                // Build a fixed summary footer line for numeric columns,
                // aligned to match column positions in the table above.
                let has_summary_columns = layout.columns.iter().any(|c| {
                    c.heading_value_kind == CategoryValueKind::Numeric
                        && c.summary_fn != SummaryFn::None
                });
                let summary_spans: Option<Vec<Span>> = if has_summary_columns
                    && !slot.items.is_empty()
                {
                    let item_refs: Vec<&Item> = slot.items.iter().collect();
                    let aggregates = compute_column_aggregates(&item_refs, &layout.columns);
                    let summary_style = Style::default()
                        .fg(Color::White)
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD);
                    let pad_style = Style::default().bg(Color::DarkGray);
                    let spacing = BOARD_TABLE_COLUMN_SPACING as usize;
                    let mut spans: Vec<Span> = Vec::new();
                    // Pad for marker + note + spacing (border + left padding of block = 1)
                    let prefix_width = 1 + layout.marker + spacing + layout.note + spacing;
                    spans.push(Span::styled(" ".repeat(prefix_width), pad_style));
                    for (col_idx, spec) in layout.columns.iter().enumerate() {
                        // Item column appears at item_board_column_index; add its width
                        if col_idx == item_board_column_index {
                            let w = item_width + spacing;
                            spans.push(Span::styled(" ".repeat(w), pad_style));
                        }
                        let col_w = spec.width;
                        if spec.summary_fn != SummaryFn::None
                            && spec.heading_value_kind == CategoryValueKind::Numeric
                        {
                            let value = aggregates[col_idx]
                                .as_ref()
                                .and_then(|agg| agg.value_for(spec.summary_fn));
                            let label = if let Some(v) = value {
                                format!(
                                    "{}={}",
                                    spec.summary_fn.label(),
                                    format_numeric_cell(Some(v), spec.numeric_format.as_ref())
                                        .trim(),
                                )
                            } else {
                                format!("{}=-", spec.summary_fn.label())
                            };
                            let display = if label.len() > col_w {
                                label[..col_w].to_string()
                            } else {
                                format!("{:>width$}", label, width = col_w)
                            };
                            spans.push(Span::styled(display, summary_style));
                        } else {
                            spans.push(Span::styled(" ".repeat(col_w), pad_style));
                        }
                        // Add inter-column spacing
                        if col_idx < layout.columns.len() - 1 || synthetic_categories_width > 0 {
                            spans.push(Span::styled(" ".repeat(spacing), pad_style));
                        }
                    }
                    // Item column after all category columns
                    if item_board_column_index >= layout.columns.len() {
                        let w = item_width + spacing;
                        spans.push(Span::styled(" ".repeat(w), pad_style));
                    }
                    if synthetic_categories_width > 0 {
                        spans.push(Span::styled(
                            " ".repeat(synthetic_categories_width),
                            pad_style,
                        ));
                    }
                    // Fill remaining width with reversed background
                    spans.push(Span::styled(" ", pad_style));
                    // Only show footer if at least one column has a computed value
                    let has_any_value = aggregates.iter().enumerate().any(|(i, agg)| {
                        let spec = &layout.columns[i];
                        spec.summary_fn != SummaryFn::None
                            && agg
                                .as_ref()
                                .and_then(|a| a.value_for(spec.summary_fn))
                                .is_some()
                    });
                    if has_any_value {
                        Some(spans)
                    } else {
                        None
                    }
                } else {
                    None
                };

                // Split the slot area: table on top, optional 1-line summary footer.
                let slot_area = columns[slot_index];
                let (table_area, summary_area) = if summary_spans.is_some() && slot_area.height > 4
                {
                    let split = Layout::default()
                        .direction(ratatui::layout::Direction::Vertical)
                        .constraints([Constraint::Min(3), Constraint::Length(1)])
                        .split(slot_area);
                    (split[0], Some(split[1]))
                } else {
                    (slot_area, None)
                };

                let mut state = Self::table_state_for(table_area, selected_row);
                let remembered_index = self
                    .horizontal_slot_item_indices
                    .get(slot_index)
                    .copied()
                    .unwrap_or(0)
                    .min(slot.items.len().saturating_sub(1));
                let remembered_scroll_offset = self
                    .horizontal_slot_scroll_offsets
                    .borrow()
                    .get(slot_index)
                    .copied()
                    .unwrap_or(0)
                    .min(slot.items.len().saturating_sub(1));
                *state.offset_mut() = if is_selected_slot {
                    Self::stable_table_offset(
                        table_area,
                        selected_row,
                        remembered_scroll_offset,
                        slot.items.len(),
                    )
                } else {
                    remembered_scroll_offset.min(remembered_index)
                };
                frame.render_stateful_widget(
                    Table::new(rows, constraints)
                        .column_spacing(BOARD_TABLE_COLUMN_SPACING)
                        .header(
                            Row::new(header_cells)
                                .style(Style::default().add_modifier(Modifier::BOLD)),
                        )
                        .block(
                            Block::default()
                                .title(title)
                                .borders(Borders::ALL)
                                .border_style(Style::default().fg(border_color)),
                        ),
                    table_area,
                    &mut state,
                );
                if let Some(stored) = self
                    .horizontal_slot_scroll_offsets
                    .borrow_mut()
                    .get_mut(slot_index)
                {
                    *stored = state.offset();
                }
                if let (Some(area), Some(spans)) = (summary_area, summary_spans) {
                    frame.render_widget(Paragraph::new(Line::from(spans)), area);
                }
                Self::render_vertical_scrollbar(
                    frame,
                    table_area,
                    slot.items.len(),
                    state.offset(),
                );
            } else {
                let widths = board_column_widths(inner_width);
                let constraints = vec![
                    Constraint::Length(widths.marker.min(u16::MAX as usize) as u16),
                    Constraint::Length(widths.when.min(u16::MAX as usize) as u16),
                    Constraint::Length(widths.note.min(u16::MAX as usize) as u16),
                    Constraint::Length(widths.item.min(u16::MAX as usize) as u16),
                    Constraint::Length(widths.categories.min(u16::MAX as usize) as u16),
                ];
                let rows: Vec<Row<'_>> = if slot.items.is_empty() {
                    let has_filter = self
                        .section_filters
                        .get(slot_index)
                        .map(|f| f.is_some())
                        .unwrap_or(false);
                    let all_slots_empty = self.slots.iter().all(|s| s.items.is_empty());
                    let empty_msg = if all_slots_empty {
                        "No items. n:add item  v:switch view  q:quit"
                    } else if has_filter {
                        "No matches. Esc:clear filter"
                    } else {
                        "No items in this section."
                    };
                    vec![Row::new(vec![
                        Cell::from(String::new()),
                        Cell::from(String::new()),
                        Cell::from(String::new()),
                        Cell::from(Span::styled(
                            empty_msg,
                            Style::default().fg(MUTED_TEXT_COLOR),
                        )),
                        Cell::from(String::new()),
                    ])]
                } else {
                    slot.items
                        .iter()
                        .enumerate()
                        .map(|(item_index, item)| {
                            let is_selected = is_selected_slot && item_index == self.item_index;
                            let when = item
                                .when_date
                                .map(|dt| dt.date().to_string())
                                .unwrap_or_else(|| "-".to_string());
                            let is_marked_selected = self.is_item_selected(item.id);
                            let marker_cell = match (is_selected, is_marked_selected) {
                                (true, _) => ">",
                                (false, true) => "+",
                                (false, false) => " ",
                            };
                            let note_cell = item_indicator_glyphs(
                                item.is_done,
                                self.is_item_blocked(item.id),
                                self.pending_suggestion_count_for_item(item.id) > 0,
                                has_note_text(item.note.as_deref()),
                            );
                            let item_text = board_item_label(item);
                            let categories = item_assignment_labels(item, &category_display_names);
                            let categories_text = if categories.is_empty() {
                                "-".to_string()
                            } else if effective_display_mode == BoardDisplayMode::MultiLine {
                                format_category_values_multi_line(
                                    &categories,
                                    BOARD_MULTI_CATEGORY_LINE_CAP,
                                )
                                .join("\n")
                            } else {
                                categories.join(", ")
                            };
                            let when_text = truncate_board_cell(&when, widths.when);
                            let item_cell_text =
                                if effective_display_mode == BoardDisplayMode::MultiLine {
                                    wrap_text_for_board_cell(&item_text, widths.item).join("\n")
                                } else {
                                    truncate_board_cell(&item_text, widths.item)
                                };
                            let categories_cell_text =
                                if effective_display_mode == BoardDisplayMode::MultiLine {
                                    categories_text
                                } else {
                                    truncate_board_cell(&categories_text, widths.categories)
                                };
                            let row_height = item_cell_text
                                .lines()
                                .count()
                                .max(categories_cell_text.lines().count())
                                .max(1);
                            let mut row = Row::new(vec![
                                Cell::from(marker_cell),
                                Cell::from(when_text),
                                Cell::from(note_cell),
                                Cell::from(item_cell_text),
                                Cell::from(categories_cell_text),
                            ]);
                            if effective_display_mode == BoardDisplayMode::MultiLine {
                                row = row.height(row_height.min(u16::MAX as usize) as u16);
                            }
                            if is_selected {
                                row = row.style(selected_board_row_style());
                            } else if is_marked_selected {
                                row = row.style(marked_board_row_style());
                            }
                            row
                        })
                        .collect()
                };
                let mut state = Self::table_state_for(columns[slot_index], selected_row);
                let remembered_index = self
                    .horizontal_slot_item_indices
                    .get(slot_index)
                    .copied()
                    .unwrap_or(0)
                    .min(slot.items.len().saturating_sub(1));
                let remembered_scroll_offset = self
                    .horizontal_slot_scroll_offsets
                    .borrow()
                    .get(slot_index)
                    .copied()
                    .unwrap_or(0)
                    .min(slot.items.len().saturating_sub(1));
                *state.offset_mut() = if is_selected_slot {
                    Self::stable_table_offset(
                        columns[slot_index],
                        selected_row,
                        remembered_scroll_offset,
                        slot.items.len(),
                    )
                } else {
                    remembered_scroll_offset.min(remembered_index)
                };
                frame.render_stateful_widget(
                    Table::new(rows, constraints)
                        .column_spacing(BOARD_TABLE_COLUMN_SPACING)
                        .header(
                            Row::new(vec![
                                Cell::from(""),
                                Cell::from("When"),
                                Cell::from(""),
                                Cell::from("Item"),
                                Cell::from("All Categories"),
                            ])
                            .style(Style::default().add_modifier(Modifier::BOLD)),
                        )
                        .block(
                            Block::default()
                                .title(title)
                                .borders(Borders::ALL)
                                .border_style(Style::default().fg(border_color)),
                        ),
                    columns[slot_index],
                    &mut state,
                );
                if let Some(stored) = self
                    .horizontal_slot_scroll_offsets
                    .borrow_mut()
                    .get_mut(slot_index)
                {
                    *stored = state.offset();
                }
                Self::render_vertical_scrollbar(
                    frame,
                    columns[slot_index],
                    slot.items.len(),
                    state.offset(),
                );
            }
        }
    }

    fn render_horizontal_board_lanes(
        &self,
        frame: &mut ratatui::Frame<'_>,
        columns: &[Rect],
        category_display_names: &HashMap<CategoryId, String>,
    ) {
        let all_slots_empty = self.slots.iter().all(|slot| slot.items.is_empty());
        for (slot_index, slot) in self.slots.iter().enumerate() {
            let slot_area = columns[slot_index];
            let is_selected_slot = slot_index == self.slot_index;
            let selected_row = if is_selected_slot && !slot.items.is_empty() {
                Some(self.item_index.min(slot.items.len().saturating_sub(1)))
            } else {
                None
            };
            let filter_suffix = self
                .section_filters
                .get(slot_index)
                .and_then(|f| f.as_deref())
                .map(|needle| format!("  filter:{needle}"))
                .unwrap_or_default();
            let title = format!("{} ({}){}", slot.title, slot.items.len(), filter_suffix);
            let border_color = if is_selected_slot {
                Color::Cyan
            } else {
                Color::Blue
            };
            let effective_display_mode = self.effective_board_display_mode_for_slot(slot);
            let card_width = slot_area.width.saturating_sub(4) as usize;

            if slot.items.is_empty() {
                let has_filter = self
                    .section_filters
                    .get(slot_index)
                    .map(|f| f.is_some())
                    .unwrap_or(false);
                let empty_lines = if all_slots_empty {
                    vec![
                        Line::from(Span::styled(
                            "empty board",
                            Style::default().fg(MUTED_TEXT_COLOR),
                        )),
                        Line::from(Span::styled(
                            "n:add item  v:views  q:quit",
                            Style::default().fg(MUTED_TEXT_COLOR),
                        )),
                    ]
                } else if has_filter {
                    vec![
                        Line::from(Span::styled(
                            "no matches",
                            Style::default().fg(MUTED_TEXT_COLOR),
                        )),
                        Line::from(Span::styled(
                            "Esc:clear filter",
                            Style::default().fg(MUTED_TEXT_COLOR),
                        )),
                    ]
                } else {
                    vec![
                        Line::from(Span::styled(
                            "empty lane",
                            Style::default().fg(MUTED_TEXT_COLOR),
                        )),
                        Line::from(Span::styled(
                            "n:add item",
                            Style::default().fg(MUTED_TEXT_COLOR),
                        )),
                    ]
                };
                let block = Block::default()
                    .title(title)
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(border_color));
                let inner = block.inner(slot_area);
                frame.render_widget(block, slot_area);
                let content_height = empty_lines.len() as u16;
                let top_padding = inner.height.saturating_sub(content_height) / 2;
                let y = inner.y.saturating_add(top_padding);
                let centered_area = Rect {
                    x: inner.x,
                    y,
                    width: inner.width,
                    height: content_height.min(inner.height),
                };
                frame.render_widget(
                    Paragraph::new(empty_lines).alignment(ratatui::layout::Alignment::Center),
                    centered_area,
                );
                continue;
            }

            let title_width = card_width.saturating_sub(1).max(1);
            let meta_width = card_width.saturating_sub(3).max(1);
            let separator_width = card_width.saturating_sub(1).max(1);
            let mut cards: Vec<ListItem<'_>> = Vec::with_capacity(slot.items.len());
            let mut card_heights: Vec<usize> = Vec::with_capacity(slot.items.len());

            for (item_index, item) in slot.items.iter().enumerate() {
                let is_focused_item = is_selected_slot && item_index == self.item_index;
                let is_marked_selected = self.is_item_selected(item.id);
                let marker_prefix = if is_marked_selected && !is_focused_item {
                    "+ "
                } else {
                    ""
                };
                let item_text = board_item_label(item);
                let category_count = item_assignment_labels(item, category_display_names).len();
                let mut meta_parts = vec![format!(
                    "due:{}",
                    item.when_date
                        .map(|dt| dt.date().to_string())
                        .unwrap_or_else(|| "none".to_string())
                )];
                let glyphs = item_indicator_glyphs(
                    item.is_done,
                    self.is_item_blocked(item.id),
                    self.pending_suggestion_count_for_item(item.id) > 0,
                    has_note_text(item.note.as_deref()),
                );
                if !glyphs.is_empty() {
                    meta_parts.push(glyphs.clone());
                }
                if category_count > 0 {
                    meta_parts.push(format!(
                        "{category_count} {}",
                        if category_count == 1 {
                            "category"
                        } else {
                            "categories"
                        }
                    ));
                }
                let meta = truncate_board_cell(&meta_parts.join("  "), meta_width);

                let mut lines = Vec::new();
                match effective_display_mode {
                    BoardDisplayMode::SingleLine => {
                        let single_line_text = if glyphs.is_empty() {
                            truncate_board_cell(&format!("{marker_prefix}{item_text}"), title_width)
                        } else {
                            let glyph_prefix = format!("{glyphs} ");
                            let reserved_glyph_width = glyph_prefix.chars().count();
                            if title_width > reserved_glyph_width {
                                format!(
                                    "{}{}",
                                    glyph_prefix,
                                    truncate_board_cell(
                                        &format!("{marker_prefix}{item_text}"),
                                        title_width.saturating_sub(reserved_glyph_width),
                                    )
                                )
                            } else {
                                truncate_board_cell(&glyph_prefix, title_width)
                            }
                        };
                        lines.push(Line::from(single_line_text));
                    }
                    BoardDisplayMode::MultiLine => {
                        for line in wrap_text_for_board_cell_clamped(
                            &format!("{marker_prefix}{item_text}"),
                            title_width,
                            2,
                        ) {
                            lines.push(Line::from(format!(" {}", line)));
                        }
                        lines.push(Line::from(Span::styled(
                            format!("   {}", meta),
                            Style::default().fg(MUTED_TEXT_COLOR),
                        )));
                        if item_index + 1 < slot.items.len() {
                            lines.push(Line::from(Span::styled(
                                format!(" {}", "-".repeat(separator_width)),
                                Style::default().fg(MUTED_TEXT_COLOR),
                            )));
                        }
                    }
                }

                card_heights.push(lines.len().max(1));
                let mut card = ListItem::new(lines);
                if !is_focused_item && is_marked_selected {
                    card = card.style(marked_board_row_style());
                }
                cards.push(card);
            }

            let mut list_state = ListState::default().with_selected(selected_row);
            let remembered_index = self
                .horizontal_slot_item_indices
                .get(slot_index)
                .copied()
                .unwrap_or(0)
                .min(cards.len().saturating_sub(1));
            let remembered_scroll_offset = self
                .horizontal_slot_scroll_offsets
                .borrow()
                .get(slot_index)
                .copied()
                .unwrap_or(0)
                .min(cards.len().saturating_sub(1));
            *list_state.offset_mut() = if is_selected_slot {
                Self::stable_variable_height_list_offset(
                    slot_area,
                    &card_heights,
                    selected_row,
                    remembered_scroll_offset,
                )
            } else {
                remembered_scroll_offset.min(remembered_index)
            };
            if let Some(stored) = self
                .horizontal_slot_scroll_offsets
                .borrow_mut()
                .get_mut(slot_index)
            {
                *stored = list_state.offset();
            }
            let scroll_position = card_heights.iter().take(list_state.offset()).sum::<usize>();
            let total_card_height = card_heights.iter().sum::<usize>();

            frame.render_stateful_widget(
                List::new(cards)
                    .block(
                        Block::default()
                            .title(title)
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(border_color)),
                    )
                    .highlight_style(selected_board_row_style())
                    .highlight_symbol("> ")
                    .highlight_spacing(ratatui::widgets::HighlightSpacing::Always)
                    .repeat_highlight_symbol(false),
                slot_area,
                &mut list_state,
            );
            Self::render_vertical_scrollbar(
                frame,
                slot_area,
                total_card_height.max(1),
                scroll_position,
            );
        }
    }

    pub(crate) fn render_preview_provenance_panel(
        &self,
        frame: &mut ratatui::Frame<'_>,
        area: Rect,
    ) {
        let mut items: Vec<ListItem<'_>> = Vec::new();
        let mut selected_line = None;
        if let Some(item) = self.selected_item() {
            for line in self.item_info_header_lines_for_item(item) {
                items.push(ListItem::new(Line::from(line)));
            }
            let rows = self.inspect_assignment_rows_for_item(item);
            if rows.is_empty() {
                items.push(ListItem::new(Line::from("(no assignments)")));
            } else {
                let row_start = items.len();
                let picker_mode = self.mode == Mode::InspectUnassign;
                for (index, row) in rows.iter().enumerate() {
                    items.push(ListItem::new(Line::from(format!(
                        "{} | source={} | origin={}",
                        row.category_name, row.source_label, row.origin_label
                    ))));
                    if picker_mode && index == self.inspect_assignment_index {
                        selected_line = Some(row_start + index);
                    }
                }
            }
        } else {
            items.push(ListItem::new(Line::from("Info")));
            items.push(ListItem::new(Line::from(
                "f focus | j/k or J/K scroll | i summary",
            )));
            items.push(ListItem::new(Line::from("")));
            items.push(ListItem::new(Line::from("(no selected item)")));
        }
        let item_count = items.len();
        let mut state = Self::list_state_for(area, selected_line);
        *state.offset_mut() = self.preview_provenance_scroll;
        frame.render_stateful_widget(
            List::new(items)
                .highlight_symbol("> ")
                .highlight_style(selected_row_style())
                .block(
                    Block::default()
                        .title("Preview: Info")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(
                            if self.normal_focus == NormalFocus::Preview {
                                Color::Cyan
                            } else {
                                Color::Yellow
                            },
                        )),
                ),
            area,
            &mut state,
        );
        Self::render_vertical_scrollbar(frame, area, item_count, state.offset());
    }

    pub(crate) fn item_details_lines_for_item(&self, item: &Item) -> Vec<Line<'_>> {
        let category_names = category_name_map(&self.categories);
        let categories = item_assignment_labels(item, &category_names);
        let pending_suggestions = self.pending_suggestion_count_for_item(item.id);
        let mut lines = vec![
            Line::from("Summary"),
            Line::from("f focus | j/k or J/K scroll | i info"),
            Line::from(""),
            Line::from(format!("ID: {}", item.id)),
            Line::from(format!(
                "Suggestions: {}",
                if pending_suggestions == 0 {
                    "-".to_string()
                } else {
                    format!("{pending_suggestions} pending")
                }
            )),
            Line::from(""),
            Line::from("Note"),
        ];

        match &item.note {
            Some(note) if !note.is_empty() => {
                for line in note.lines() {
                    lines.push(Line::from(format!("  {}", line)));
                }
            }
            _ => lines.push(Line::from("  (none)")),
        }
        lines.push(Line::from(""));
        lines.push(Line::from("Categories"));
        if categories.is_empty() {
            lines.push(Line::from("  (none)"));
        } else {
            lines.push(Line::from(format!("  {}", categories.join(", "))));
        }
        lines
    }

    pub(crate) fn item_info_header_lines_for_item(&self, item: &Item) -> Vec<String> {
        let mut lines = vec![
            "Info".to_string(),
            "f focus | j/k or J/K scroll | i summary".to_string(),
            String::new(),
            format!("ID: {}", item.id),
            format!(
                "Suggestions: {}",
                match self.pending_suggestion_count_for_item(item.id) {
                    0 => "-".to_string(),
                    count => format!("{count} pending"),
                }
            ),
            String::new(),
            "Metadata".to_string(),
            format!("  Done: {}", if item.is_done { "yes" } else { "no" }),
            format!(
                "  When: {}",
                item.when_date
                    .map(|value| value.strftime("%Y-%m-%d %H:%M:%S").to_string())
                    .unwrap_or_else(|| "-".to_string())
            ),
            format!("  Created: {}", item.created_at),
            format!("  Modified: {}", item.modified_at),
        ];
        if let Some(links) = self.item_links_by_item_id.get(&item.id) {
            lines.push(String::new());
            Self::push_link_summary_section(
                &mut lines,
                "Prereqs",
                self.item_link_preview_labels(&links.depends_on),
            );
            Self::push_link_summary_section(
                &mut lines,
                "Blocks",
                self.item_link_preview_labels(&links.blocks),
            );
            Self::push_link_summary_section(
                &mut lines,
                "Related",
                self.item_link_preview_labels(&links.related),
            );
        }
        lines.push(String::new());
        lines.push("Assignment Provenance".to_string());
        lines
    }

    pub(crate) fn preview_info_line_count_for_selected_item(&self) -> usize {
        let Some(item) = self.selected_item() else {
            return 4;
        };
        let assignment_len = self.inspect_assignment_rows_for_item(item).len().max(1);
        self.item_info_header_lines_for_item(item).len() + assignment_len
    }

    fn push_link_summary_section(lines: &mut Vec<String>, label: &str, rows: Vec<String>) {
        lines.push(label.to_string());
        if rows.is_empty() {
            lines.push("  (none)".to_string());
            return;
        }
        for row in rows {
            lines.push(format!("  {row}"));
        }
    }

    fn item_link_preview_labels(&self, ids: &[ItemId]) -> Vec<String> {
        let mut rows: Vec<(String, String)> = ids
            .iter()
            .map(|id| {
                if let Some(item) = self.all_items.iter().find(|item| item.id == *id) {
                    let sort_key = item.text.to_ascii_lowercase();
                    let status = if item.is_done { "done" } else { "open" };
                    (sort_key, format!("{status} | {}", item.text))
                } else {
                    (id.to_string(), format!("missing | {}", id))
                }
            })
            .collect();
        rows.sort_by(|a, b| a.0.cmp(&b.0));
        rows.into_iter().map(|(_, label)| label).collect()
    }

    pub(crate) fn render_preview_summary_panel(&self) -> Paragraph<'_> {
        let lines = if let Some(item) = self.selected_item() {
            self.item_details_lines_for_item(item)
        } else {
            vec![
                Line::from("Summary"),
                Line::from("f focus | j/k or J/K scroll | i info"),
                Line::from(""),
                Line::from("(no selected item)"),
            ]
        };

        Paragraph::new(lines)
            .block(
                Block::default()
                    .title("Preview: Summary")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(
                        if self.normal_focus == NormalFocus::Preview {
                            Color::Cyan
                        } else {
                            Color::Yellow
                        },
                    )),
            )
            .scroll((self.preview_summary_scroll.min(u16::MAX as usize) as u16, 0))
            .wrap(Wrap { trim: false })
    }

    fn render_suggestion_review(&self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        let Some(state) = &self.suggestion_review else {
            return;
        };

        frame.render_widget(Clear, area);
        let block = Block::default()
            .title(" Review Suggestions ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Cyan));
        let inner = block.inner(area);
        frame.render_widget(block, area);

        if inner.height < 3 || inner.width < 10 {
            return;
        }

        // Two-pane layout: item list (left) | suggestion detail (right)
        let panes = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(35), // item list
                Constraint::Percentage(65), // suggestion detail
            ])
            .split(inner);

        // === Left pane: item list ===
        let items_focused = state.focus == SuggestionReviewFocus::Items;
        let items_border_style = if items_focused {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Cyan)
        };
        let items_title = if items_focused {
            format!(" > Items ({}) ", state.items.len())
        } else {
            format!(" Items ({}) ", state.items.len())
        };
        let items_block = Block::default()
            .title(items_title)
            .borders(Borders::ALL)
            .border_style(items_border_style);
        let items_inner = items_block.inner(panes[0]);
        frame.render_widget(items_block, panes[0]);

        let item_lines: Vec<Line<'_>> = state
            .items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let is_selected = i == state.item_index;
                let prefix = if is_selected { "> " } else { "  " };
                let count = item.suggestions.len();
                if is_selected && items_focused {
                    // Cursor row: selected_row_style (Cyan bg + Black fg)
                    let sel = selected_row_style();
                    Line::from(vec![
                        Span::styled(prefix, sel),
                        Span::styled(&*item.item_text, sel.add_modifier(Modifier::BOLD)),
                        Span::styled(
                            format!("  ({count})"),
                            Style::default().fg(Color::Black).bg(Color::Cyan),
                        ),
                    ])
                } else {
                    let style = if is_selected {
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::White)
                    };
                    Line::from(vec![
                        Span::styled(prefix, style),
                        Span::styled(&*item.item_text, style),
                        Span::styled(format!("  ({count})"), Style::default().fg(Color::Yellow)),
                    ])
                }
            })
            .collect();
        let item_scroll = list_scroll_for_selected_line(items_inner, Some(state.item_index));
        frame.render_widget(
            Paragraph::new(item_lines).scroll((item_scroll, 0)),
            items_inner,
        );

        // === Right pane: selected item detail + suggestions ===
        let sugg_focused = state.focus == SuggestionReviewFocus::Suggestions;
        let sugg_border_style = if sugg_focused {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Cyan)
        };

        if let Some(item) = state.items.get(state.item_index) {
            let sugg_title = if sugg_focused {
                " > Suggestions "
            } else {
                " Suggestions "
            };
            let detail_block = Block::default()
                .title(sugg_title)
                .borders(Borders::ALL)
                .border_style(sugg_border_style);
            let detail_inner = detail_block.inner(panes[1]);
            frame.render_widget(detail_block, panes[1]);

            // Vertical layout: item header + suggestion list
            let detail_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3), // item header
                    Constraint::Min(1),    // suggestion list
                ])
                .split(detail_inner);

            // Item header
            let mut header_lines = vec![Line::from(vec![
                Span::styled("Item: ", Style::default().fg(Color::Gray)),
                Span::styled(
                    &*item.item_text,
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
            ])];
            if let Some(note) = &item.note_excerpt {
                header_lines.push(Line::from(vec![
                    Span::styled("Note: ", Style::default().fg(Color::Gray)),
                    Span::styled(note.as_str(), Style::default().fg(Color::Gray)),
                ]));
            }
            if !item.current_assignments.is_empty() {
                header_lines.push(Line::from(vec![
                    Span::styled("Assigned: ", Style::default().fg(Color::Gray)),
                    Span::styled(
                        item.current_assignments.join(", "),
                        Style::default().fg(Color::White),
                    ),
                ]));
            }
            frame.render_widget(Paragraph::new(header_lines), detail_chunks[0]);

            // Suggestion list
            let cat_names = category_name_map(&self.categories);
            let sugg_lines: Vec<Line<'_>> = item
                .suggestions
                .iter()
                .enumerate()
                .map(|(i, review)| {
                    let is_cursor = sugg_focused && i == state.suggestion_cursor;
                    let marker = if review.accepted { "[x]" } else { "[ ]" };
                    let marker_color = if review.accepted {
                        Color::LightGreen
                    } else {
                        Color::LightRed
                    };
                    let category_name =
                        candidate_assignment_label(&review.suggestion.assignment, &cat_names);
                    let rationale = review
                        .suggestion
                        .rationale
                        .as_deref()
                        .unwrap_or("text match");

                    if is_cursor {
                        // Cursor row: selected_row_style (Cyan bg + Black fg)
                        let sel = selected_row_style();
                        let sel_marker = if review.accepted {
                            Style::default()
                                .fg(Color::Black)
                                .bg(Color::Cyan)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default()
                                .fg(Color::Red)
                                .bg(Color::Cyan)
                                .add_modifier(Modifier::BOLD)
                        };
                        Line::from(vec![
                            Span::styled("> ", sel),
                            Span::styled(marker, sel_marker),
                            Span::styled(" ", sel),
                            Span::styled(category_name, sel.add_modifier(Modifier::BOLD)),
                            Span::styled(
                                format!("  ({rationale})"),
                                Style::default().fg(Color::DarkGray).bg(Color::Cyan),
                            ),
                        ])
                    } else {
                        Line::from(vec![
                            Span::raw("  "),
                            Span::styled(marker, Style::default().fg(marker_color)),
                            Span::raw(" "),
                            Span::styled(
                                category_name,
                                Style::default()
                                    .fg(Color::White)
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::styled(
                                format!("  ({rationale})"),
                                Style::default().fg(Color::Gray),
                            ),
                        ])
                    }
                })
                .collect();
            let sugg_scroll =
                list_scroll_for_selected_line(detail_chunks[1], Some(state.suggestion_cursor));
            frame.render_widget(
                Paragraph::new(sugg_lines).scroll((sugg_scroll, 0)),
                detail_chunks[1],
            );
        } else {
            let detail_block = Block::default()
                .title(" Suggestions ")
                .borders(Borders::ALL)
                .border_style(sugg_border_style);
            frame.render_widget(detail_block, panes[1]);
        }
    }

    pub(crate) fn render_footer(&self, width: u16) -> Paragraph<'_> {
        let status = format!(
            "{} | Auto-refresh:{}",
            self.footer_status_text(),
            self.auto_refresh_mode_label()
        );
        let hint_pairs = self.footer_hint_pairs();
        // Build width-aware hint line with styled key:desc spans
        let available = width.saturating_sub(4) as usize; // borders + padding
        let help_suffix = "?:help";
        let help_suffix_len = help_suffix.len();
        let mut spans: Vec<Span<'_>> = Vec::new();
        let mut used = 0usize;
        for (key, desc) in &hint_pairs {
            let entry = format!("{}:{}", key, desc);
            let entry_len = entry.len() + if spans.is_empty() { 0 } else { 2 }; // "  " separator
                                                                                // Reserve space for help suffix if not already the help entry
            let reserve = if *key != "?" { help_suffix_len + 2 } else { 0 };
            if used + entry_len + reserve > available && !spans.is_empty() {
                break;
            }
            if !spans.is_empty() {
                spans.push(Span::raw("  "));
                used += 2;
            }
            spans.push(Span::styled(
                format!("{}:", key),
                Style::default().fg(Color::LightCyan),
            ));
            spans.push(Span::styled(
                desc.to_string(),
                Style::default().fg(MUTED_TEXT_COLOR),
            ));
            used += entry_len;
        }
        // Add ?:help if not already present
        if !hint_pairs.iter().any(|(k, _)| *k == "?") {
            if !spans.is_empty() {
                spans.push(Span::raw("  "));
            }
            spans.push(Span::styled("?:", Style::default().fg(Color::LightCyan)));
            spans.push(Span::styled("help", Style::default().fg(MUTED_TEXT_COLOR)));
        }
        let text = ratatui::text::Text::from(vec![
            ratatui::text::Line::from(status),
            ratatui::text::Line::from(spans),
        ]);
        Paragraph::new(text).block(Block::default().borders(Borders::ALL))
    }

    fn footer_status_text(&self) -> String {
        match self.mode {
            Mode::SearchBarFocused => {
                let section_name = self
                    .slots
                    .get(self.slot_index)
                    .map(|s| s.title.as_str())
                    .unwrap_or("section");
                let match_count = self.current_slot().map(|s| s.items.len()).unwrap_or(0);
                format!("[{section_name}] {match_count} matches")
            }
            Mode::ConfirmDelete => {
                if let Some(done_confirm) = &self.done_blocks_confirm {
                    match &done_confirm.scope {
                        DoneBlocksConfirmScope::Single {
                            blocked_item_ids, ..
                        } => {
                            let blocked_count = blocked_item_ids.len();
                            let suffix = if blocked_count == 1 { "" } else { "s" };
                            format!(
                                "This item blocks {blocked_count} other item{suffix}. Remove that link and mark done?"
                            )
                        }
                        DoneBlocksConfirmScope::Batch {
                            blocking_item_count,
                            blocked_link_count,
                            ..
                        } => {
                            let item_suffix = if *blocking_item_count == 1 { "" } else { "s" };
                            let blocked_suffix = if *blocked_link_count == 1 { "" } else { "s" };
                            format!(
                                "{blocking_item_count} selected item{item_suffix} blocks {blocked_link_count} other item{blocked_suffix}. Remove those links and mark done?"
                            )
                        }
                    }
                } else if let Some(batch_delete_item_ids) = &self.batch_delete_item_ids {
                    let selected_count = batch_delete_item_ids.len();
                    let item_suffix = if selected_count == 1 { "" } else { "s" };
                    format!(
                        "Delete {selected_count} selected item{item_suffix}? y:confirm Esc:cancel"
                    )
                } else {
                    "Delete item? y:confirm Esc:cancel".to_string()
                }
            }
            Mode::BoardColumnDeleteConfirm => {
                if let Some(name) = &self.board_pending_delete_column_label {
                    format!("Delete column '{name}'? y:confirm Esc:cancel")
                } else {
                    "Delete column? y:confirm Esc:cancel".to_string()
                }
            }
            Mode::ViewDeleteConfirm => "Delete view? y:confirm Esc:cancel".to_string(),
            Mode::ItemAssignPicker => {
                "Assign categories (Space applies; Enter/Esc close)".to_string()
            }
            Mode::ItemAssignInput => format!("Category> {}", self.input.text()),
            Mode::LinkWizard => {
                if let Some(state) = self.link_wizard_state() {
                    let action = LinkWizardAction::from_index(state.action_index);
                    if action.requires_target() {
                        format!(
                            "Link wizard ({}) target> {}",
                            action.label(),
                            state.target_filter.text()
                        )
                    } else {
                        format!("Link wizard ({})", action.label())
                    }
                } else {
                    "Link wizard".to_string()
                }
            }
            Mode::BoardAddColumnPicker => {
                format!(
                    "Add column> {}",
                    self.board_add_column_input_text().unwrap_or("")
                )
            }
            Mode::InspectUnassign => "Select assignment".to_string(),
            Mode::InputPanel => {
                if let Some(panel) = &self.input_panel {
                    use input_panel::InputPanelFocus;
                    format!(
                        "{} (focus: {})",
                        match panel.kind {
                            input_panel::InputPanelKind::AddItem => "Add item",
                            input_panel::InputPanelKind::EditItem => "Edit item",
                            input_panel::InputPanelKind::NameInput => "Name input",
                            input_panel::InputPanelKind::WhenDate => "When editor",
                            input_panel::InputPanelKind::NumericValue => "Set value",
                            input_panel::InputPanelKind::CategoryCreate => "Create category",
                        },
                        match panel.focus {
                            InputPanelFocus::Text => "Text",
                            InputPanelFocus::When => "When",
                            InputPanelFocus::Note => "Note",
                            InputPanelFocus::Categories => "Categories",
                            InputPanelFocus::TypePicker => "Type",
                            InputPanelFocus::SaveButton => "Save",
                            InputPanelFocus::CancelButton => "Cancel",
                        }
                    )
                } else {
                    self.status.clone()
                }
            }
            Mode::SuggestionReview => self.status.clone(),
            Mode::Normal => self
                .active_transient_status_text()
                .map(str::to_string)
                .unwrap_or_else(|| {
                    if let Some(suffix) = self.classification_pending_suffix() {
                        if self.status.contains("classification suggestion") {
                            self.status.clone()
                        } else {
                            format!("{} | {suffix}", self.status)
                        }
                    } else {
                        self.status.clone()
                    }
                }),
            Mode::HelpPanel
            | Mode::ViewPicker
            | Mode::ViewEdit
            | Mode::CategoryManager
            | Mode::CategoryDirectEdit
            | Mode::CategoryColumnPicker => self.status.clone(),
        }
    }

    fn footer_hint_pairs(&self) -> Vec<(&'static str, &'static str)> {
        match self.mode {
            Mode::HelpPanel => vec![("Esc", "close"), ("Enter", "close"), ("?", "close")],
            Mode::SuggestionReview => vec![
                ("Tab", "pane"),
                ("Space", "toggle"),
                ("Enter", "confirm"),
                ("s", "skip"),
                ("A", "accept all"),
                ("Esc", "close"),
            ],
            Mode::CategoryManager => {
                if self.category_manager_discard_confirm() {
                    vec![
                        ("y", "save & close"),
                        ("n", "discard"),
                        ("Esc", "keep editing"),
                    ]
                } else if self.classification_mode_picker_open {
                    vec![("j/k", "mode"), ("Enter", "apply"), ("Esc", "close")]
                } else if self.workflow_role_picker.is_some() {
                    vec![
                        ("j/k", "pick"),
                        ("Enter", "assign"),
                        ("x", "clear"),
                        ("Esc", "back"),
                    ]
                } else if self.workflow_setup_open {
                    vec![
                        ("j/k", "role"),
                        ("Enter", "choose"),
                        ("x", "clear"),
                        ("Esc", "close"),
                    ]
                } else if self.category_manager_details_note_editing() {
                    vec![("Tab", "leave note"), ("Esc", "discard")]
                } else if self.category_manager_details_also_match_editing() {
                    vec![
                        ("Type", "edit"),
                        ("Enter", "newline"),
                        ("Tab", "leave"),
                        ("Esc", "discard"),
                    ]
                } else if let Some(action) = self.category_manager_inline_action() {
                    match action {
                        CategoryInlineAction::Rename { .. } => {
                            vec![("Enter", "apply"), ("Esc", "cancel")]
                        }
                        CategoryInlineAction::DeleteConfirm { .. } => {
                            vec![("y", "confirm"), ("Esc", "cancel")]
                        }
                    }
                } else if self.category_manager_filter_editing()
                    || self.category_manager_focus() == Some(CategoryManagerFocus::Filter)
                {
                    vec![
                        ("Type", "filter"),
                        ("Esc", "clear"),
                        ("Tab", "next"),
                        ("Enter", "details"),
                    ]
                } else if self.category_manager_focus() == Some(CategoryManagerFocus::Details) {
                    vec![
                        ("j/k", "field"),
                        ("Enter/Space", "toggle"),
                        ("S", "save"),
                        ("Tab", "pane"),
                        ("Esc", "close"),
                    ]
                } else {
                    vec![
                        ("n", "new"),
                        ("r", "rename"),
                        ("x", "delete"),
                        ("S-\u{2191}/\u{2193}", "move"),
                        ("H/L", "level"),
                        ("m", "auto"),
                        ("w", "queues"),
                        ("/", "filter"),
                        ("Tab", "details"),
                        ("Esc", "close"),
                    ]
                }
            }
            Mode::ViewPicker => {
                vec![
                    ("Enter", "switch"),
                    ("N", "new"),
                    ("c", "clone"),
                    ("r", "rename"),
                    ("e", "edit"),
                    ("x", "delete"),
                    ("Esc", "cancel"),
                ]
            }
            Mode::ViewDeleteConfirm => vec![("y", "confirm"), ("Esc", "cancel")],
            Mode::ViewEdit => {
                if let Some(state) = &self.view_edit_state {
                    if state.discard_confirm {
                        vec![
                            ("y", "save & close"),
                            ("n", "discard"),
                            ("Esc", "keep editing"),
                        ]
                    } else if state.pane_focus == ViewEditPaneFocus::Sections {
                        vec![
                            ("S", "save"),
                            ("n", "new"),
                            ("x", "del"),
                            ("Enter", "details"),
                            ("Tab", "pane"),
                            ("Esc", "close"),
                        ]
                    } else if state.pane_focus == ViewEditPaneFocus::Preview {
                        vec![
                            ("S", "save"),
                            ("p", "hide"),
                            ("Tab", "pane"),
                            ("Esc", "close"),
                        ]
                    } else {
                        vec![
                            ("S", "save"),
                            ("n", "new"),
                            ("x", "del"),
                            ("Space", "toggle"),
                            ("Tab", "pane"),
                            ("Esc", "close"),
                        ]
                    }
                } else {
                    vec![("S", "save"), ("Tab", "pane"), ("Esc", "close")]
                }
            }
            Mode::ItemAssignPicker => vec![
                ("Space", "apply"),
                ("n", "new"),
                ("Enter", "close"),
                ("Esc", "cancel"),
            ],
            Mode::ItemAssignInput => vec![("Enter", "assign"), ("Esc", "cancel")],
            Mode::LinkWizard => vec![
                ("Tab", "focus"),
                ("Enter", "apply"),
                ("/", "target"),
                ("Esc", "cancel"),
            ],
            Mode::CategoryDirectEdit => vec![
                ("S", "save"),
                ("Tab", "focus"),
                ("Enter", "resolve"),
                ("x", "remove"),
                ("Esc", "cancel"),
            ],
            Mode::CategoryColumnPicker => {
                vec![("Space", "toggle"), ("Enter", "save"), ("Esc", "cancel")]
            }
            Mode::BoardAddColumnPicker => {
                vec![("Enter", "insert"), ("Tab", "complete"), ("Esc", "cancel")]
            }
            Mode::ConfirmDelete => {
                if self.done_blocks_confirm.is_some() {
                    vec![
                        ("y", "remove links + done"),
                        ("n", "done only"),
                        ("Esc", "cancel"),
                    ]
                } else {
                    vec![("y", "confirm"), ("Esc", "cancel")]
                }
            }
            Mode::BoardColumnDeleteConfirm => {
                vec![("y", "confirm"), ("Esc", "cancel")]
            }
            Mode::SearchBarFocused => {
                if self.global_search_active() {
                    vec![
                        ("Enter", "jump/create"),
                        ("\u{2193}/Tab", "browse"),
                        ("Esc", "return"),
                    ]
                } else {
                    vec![
                        ("Enter", "jump/create"),
                        ("\u{2193}/Tab", "browse"),
                        ("Esc", "clear"),
                    ]
                }
            }
            Mode::InspectUnassign => vec![("Enter", "unassign"), ("Esc", "cancel")],
            Mode::InputPanel => {
                if self.input_panel_discard_confirm_active() {
                    vec![
                        ("y", "discard"),
                        ("n", "keep editing"),
                        ("Esc", "keep editing"),
                    ]
                } else if self
                    .input_panel
                    .as_ref()
                    .map(|p| {
                        p.kind == input_panel::InputPanelKind::NumericValue
                            || p.kind == input_panel::InputPanelKind::WhenDate
                    })
                    .unwrap_or(false)
                {
                    vec![
                        ("Enter", "save"),
                        ("S", "save"),
                        ("Tab", "buttons"),
                        ("Esc", "cancel"),
                    ]
                } else if self.input_panel.as_ref().is_some_and(|p| {
                    p.focus == input_panel::InputPanelFocus::Categories && p.category_filter_editing
                }) {
                    vec![
                        ("Type", "filter"),
                        ("Enter", "keep"),
                        ("Esc", "done"),
                        ("Tab", "next"),
                    ]
                } else if self
                    .input_panel
                    .as_ref()
                    .is_some_and(|p| p.focus == input_panel::InputPanelFocus::Categories)
                {
                    vec![
                        ("S", "save"),
                        ("Tab", "next"),
                        ("/", "filter"),
                        ("Space", "toggle"),
                        ("Esc", "cancel"),
                    ]
                } else {
                    let esc_hint = if self.input_panel.as_ref().is_some_and(|panel| {
                        panel.kind == input_panel::InputPanelKind::EditItem
                            && matches!(
                                panel.focus,
                                input_panel::InputPanelFocus::Text
                                    | input_panel::InputPanelFocus::Note
                            )
                    }) {
                        "discard?"
                    } else {
                        "cancel"
                    };
                    vec![
                        ("S", "save"),
                        ("Tab", "next"),
                        ("Ctrl-G", "$EDITOR"),
                        ("Esc", esc_hint),
                    ]
                }
            }
            Mode::Normal => {
                let mut hints: Vec<(&'static str, &'static str)> = Vec::new();
                if self.selected_count() > 0 {
                    hints.extend_from_slice(&[
                        ("Space", "toggle"),
                        ("a", "assign"),
                        ("b/B", "link"),
                        ("x", "delete"),
                        ("Esc", "clear sel"),
                        ("/", "search"),
                        ("C", "classify"),
                        ("g/", "global"),
                        ("v", "views"),
                        ("p", "preview"),
                        ("q", "quit"),
                    ]);
                } else {
                    hints.extend_from_slice(&[
                        ("n", "new"),
                        ("e", "edit"),
                        ("a", "assign"),
                        ("d", "done"),
                        ("/", "search"),
                        ("v", "views"),
                        ("m", "lanes"),
                        ("s", "sort"),
                        ("f", "col fmt"),
                        ("F", "col summary"),
                        ("p", "preview"),
                        ("u", "deps"),
                        ("C", "classify"),
                        ("g/", "global"),
                        ("z", "cards"),
                    ]);
                    if self.section_filters.iter().any(|f| f.is_some()) {
                        hints.push(("Esc", "clear search"));
                    }
                    hints.extend_from_slice(&[
                        ("Ctrl-L", "reload"),
                        ("Ctrl-R", "auto-refresh"),
                        ("q", "quit"),
                    ]);
                }
                if self.undo.has_undo() {
                    // Insert undo hint near the front (after primary action keys)
                    let insert_pos = hints.len().min(4);
                    hints.insert(insert_pos, ("Ctrl-Z", "undo"));
                }
                if self.undo.has_redo() {
                    // Insert redo hint right after undo
                    let undo_pos = hints.iter().position(|h| h.0 == "Ctrl-Z");
                    let insert_pos = undo_pos
                        .map(|p| p + 1)
                        .unwrap_or_else(|| hints.len().min(5));
                    hints.insert(insert_pos, ("Ctrl-Shift-Z", "redo"));
                }
                hints
            }
        }
    }

    fn render_help_panel(&self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        if area.width < 8 || area.height < 8 {
            return;
        }

        let header = Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD);
        let key_style = Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD);

        let help_entry = |key: &str, desc: &str| -> Line<'static> {
            let pad = 12_usize.saturating_sub(key.len());
            Line::from(vec![
                Span::raw("  "),
                Span::styled(key.to_string(), key_style),
                Span::raw(" ".repeat(pad)),
                Span::raw(desc.to_string()),
            ])
        };

        let lines: Vec<Line<'static>> = vec![
            Line::from(Span::styled("CURRENT ITEM", header)),
            help_entry("n", "Add a new item to the focused section"),
            help_entry("e", "Edit the selected item (text, note, categories)"),
            help_entry("Enter", "Edit item / column cell / add (if empty)"),
            help_entry("a", "Assign categories to current item or selection"),
            help_entry("d", "Toggle done on selected item(s)"),
            help_entry("r", "Remove item from current view (keeps item)"),
            help_entry("x", "Delete selected item(s)"),
            help_entry(
                "[/] or S-\u{2191}/\u{2193}",
                "Move item to previous / next section",
            ),
            help_entry("p", "Toggle the preview sidebar"),
            help_entry("i/o", "Cycle preview mode"),
            Line::from(""),
            Line::from(Span::styled("SELECTION", header)),
            help_entry("Space", "Toggle selection on current item"),
            help_entry("b / B", "Link / unlink selected items (dependency)"),
            help_entry("x", "Delete selected items"),
            help_entry("Esc", "Clear selection"),
            Line::from(""),
            Line::from(Span::styled("NAVIGATION", header)),
            help_entry("\u{2191}/k \u{2193}/j", "Move between items"),
            help_entry("\u{2190}/h \u{2192}/l", "Move between sections (lanes)"),
            help_entry("J / K", "Scroll preview pane"),
            help_entry("Tab/S-Tab", "Next / previous section"),
            help_entry("m", "Cycle lane layout (single \u{2194} multi-column)"),
            help_entry("z", "Cycle card size (compact \u{2194} detail)"),
            Line::from(""),
            Line::from(Span::styled("SEARCH", header)),
            help_entry("/", "Search within the focused section"),
            help_entry("g/", "Search across all sections (global)"),
            help_entry("Esc", "Clear active section filter"),
            Line::from(""),
            Line::from(Span::styled("COLUMNS", header)),
            help_entry("Enter", "Edit column value (on a column cell)"),
            help_entry("+/-", "Add / remove board column"),
            help_entry("H/L", "Move board column left / right"),
            help_entry("f", "Cycle numeric column format"),
            help_entry("F", "Cycle numeric column summary (Sum/Avg/Min/Max)"),
            help_entry("s/S or </>", "Sort section by column (asc / desc)"),
            Line::from(""),
            Line::from(Span::styled("VIEWS", header)),
            help_entry("v / F8", "Open the view picker"),
            help_entry(",/.", "Previous / next view"),
            help_entry("ga", "Jump to All Items view"),
            Line::from(""),
            Line::from(Span::styled("GLOBAL", header)),
            help_entry("C", "Open classification review"),
            help_entry("ga", "Jump to All Items view"),
            help_entry("c / F9", "Open the category manager"),
            help_entry("u", "Toggle hide-dependent-items filter"),
            help_entry("Ctrl-G", "Open $EDITOR for text/note (in item editor)"),
            help_entry("Ctrl-L", "Reload data from disk"),
            help_entry("Ctrl-R", "Toggle auto-refresh interval"),
            help_entry("Ctrl-Z", "Undo"),
            help_entry("C-S-Z", "Redo"),
            help_entry("?", "Toggle this help panel"),
            help_entry("q", "Quit"),
            Line::from(""),
            Line::from(Span::styled(
                "              Esc / Enter / ? to close",
                Style::default().fg(Color::DarkGray),
            )),
        ];

        let block = Block::default()
            .title("Keyboard Shortcuts")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));
        let inner = block.inner(area);
        frame.render_widget(Clear, area);
        frame.render_widget(block, area);
        frame.render_widget(Paragraph::new(lines), inner);
    }

    pub(crate) fn render_input_panel(
        &self,
        frame: &mut ratatui::Frame<'_>,
        area: Rect,
        panel: &input_panel::InputPanel,
    ) {
        use input_panel::{InputPanelFocus, InputPanelKind};

        frame.render_widget(Clear, area);

        let dirty_marker = if panel.is_dirty() { " *" } else { "" };
        let title = match panel.kind {
            InputPanelKind::AddItem => format!("Add Item{dirty_marker}"),
            InputPanelKind::EditItem => format!("Edit Item{dirty_marker}"),
            InputPanelKind::NameInput => format!("Name{dirty_marker}"),
            InputPanelKind::WhenDate => format!("Edit When{dirty_marker}"),
            InputPanelKind::NumericValue => format!("Set Value{dirty_marker}"),
            InputPanelKind::CategoryCreate => format!("Create Category{dirty_marker}"),
        };
        let block = Block::default()
            .title(title)
            .title_style(Style::default().fg(Color::White))
            .borders(Borders::ALL)
            .border_style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            );
        frame.render_widget(block, area);

        let Some(regions) = input_panel_popup_regions(area, panel.kind) else {
            return;
        };

        // Text field
        let text_marker = if panel.focus == InputPanelFocus::Text {
            "> "
        } else {
            "  "
        };
        let text_label = match panel.kind {
            InputPanelKind::NameInput | InputPanelKind::CategoryCreate => "Name",
            InputPanelKind::WhenDate => "When",
            InputPanelKind::NumericValue => "Value",
            _ => "Text",
        };
        let text_prefix = format!("{text_marker}{text_label}> ");
        let text_width = regions.text.width as usize;
        let (visible_value, _) = clip_text_for_row(
            panel.text.text(),
            panel.text.cursor(),
            text_width.saturating_sub(text_prefix.chars().count()),
            panel.focus == InputPanelFocus::Text,
        );
        let text_prefix_style = if panel.focus == InputPanelFocus::Text {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        let text_spans = vec![
            Span::styled(text_prefix, text_prefix_style),
            Span::raw(visible_value),
        ];
        frame.render_widget(Paragraph::new(Line::from(text_spans)), regions.text);

        if panel.kind == InputPanelKind::NumericValue && !panel.preview_context.is_empty() {
            if let Some(context_rect) = regions.context {
                frame.render_widget(
                    Paragraph::new(format!("  {}", panel.preview_context))
                        .style(Style::default().fg(MUTED_TEXT_COLOR)),
                    context_rect,
                );
            }
        }

        if panel.kind == InputPanelKind::WhenDate {
            if let Some(context_rect) = regions.context {
                let context_text = format!("Item: {}", panel.preview_context);
                frame.render_widget(
                    Paragraph::new(context_text).style(Style::default().fg(MUTED_TEXT_COLOR)),
                    context_rect,
                );
            }
        }

        // When-date field (AddItem/EditItem only)
        if let Some(when_rect) = regions.when {
            let when_focused = panel.focus == InputPanelFocus::When;
            let when_marker = if when_focused { "> " } else { "  " };
            let when_prefix = format!("{when_marker}When> ");
            let when_width = when_rect.width as usize;
            let (when_visible, _) = clip_text_for_row(
                panel.when_buffer.text(),
                panel.when_buffer.cursor(),
                when_width.saturating_sub(when_prefix.chars().count()),
                when_focused,
            );
            let when_prefix_style = if when_focused {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            let when_value_style = if when_focused {
                Style::default()
            } else if panel.when_buffer.text().is_empty() {
                Style::default().fg(MUTED_TEXT_COLOR)
            } else {
                Style::default()
            };
            let when_display = if !when_focused && panel.when_buffer.text().is_empty() {
                "(none — today, tomorrow, next week, 2026-03-25, …)"
            } else {
                &when_visible
            };
            let when_spans = vec![
                Span::styled(when_prefix, when_prefix_style),
                Span::styled(when_display, when_value_style),
            ];
            frame.render_widget(Paragraph::new(Line::from(when_spans)), when_rect);
        }

        // Note (not shown for NameInput)
        if let Some(note_rect) = regions.note {
            let note_focused = panel.focus == InputPanelFocus::Note;
            let note_border_style = if note_focused {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Cyan)
            };
            let mut note_widget = panel.note.widget().clone();
            note_widget.set_placeholder_text(NOTE_PLACEHOLDER_TEXT);
            note_widget.set_placeholder_style(Style::default().fg(MUTED_TEXT_COLOR));
            note_widget.set_style(Style::default());
            if note_focused {
                note_widget.set_cursor_line_style(Style::default().bg(Color::DarkGray));
                note_widget.set_cursor_style(
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                );
            }
            note_widget.set_block(
                Block::default()
                    .title(if note_focused { "> Note" } else { "Note" })
                    .borders(Borders::ALL)
                    .border_style(note_border_style),
            );
            frame.render_widget(&note_widget, note_rect);
            let note_scroll =
                list_scroll_for_selected_line(note_rect, Some(panel.note.line_col().0));
            Self::render_vertical_scrollbar(
                frame,
                note_rect,
                panel.note.text().lines().count().max(1),
                note_scroll as usize,
            );
        }

        // Inline categories list (bordered, scrollable)
        if let Some(cat_rect) = regions.categories {
            let cat_focused = panel.focus == InputPanelFocus::Categories;
            let cat_border_style = if cat_focused {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Cyan)
            };
            let visible_indices = self.input_panel_visible_category_row_indices();
            let cat_inner = regions.categories_inner.unwrap_or(cat_rect);
            let cat_filter_rect = regions.categories_filter.unwrap_or(cat_inner);
            let cat_list_rect = if panel.category_filter_editing {
                regions.categories_list.unwrap_or(cat_inner)
            } else {
                cat_inner
            };
            let inner_width = cat_list_rect.width as usize;

            frame.render_widget(
                Block::default()
                    .title(if cat_focused {
                        "> Categories"
                    } else {
                        "Categories"
                    })
                    .borders(Borders::ALL)
                    .style(Style::default())
                    .border_style(cat_border_style),
                cat_rect,
            );
            if panel.category_filter_editing {
                frame.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        format!("> Filter> {}", panel.category_filter.text()),
                        Style::default().fg(Color::Yellow),
                    ))),
                    cat_filter_rect,
                );
            }

            let mut lines: Vec<Line<'_>> = Vec::new();

            // Pending suggestions at the top (before categories)
            let suggestion_len = panel.pending_suggestions.len();
            if suggestion_len > 0 {
                lines.push(Line::from(vec![
                    Span::styled("─── Suggested ", Style::default().fg(Color::Yellow)),
                    Span::styled("(Space: toggle) ", Style::default().fg(Color::Gray)),
                    Span::styled("───", Style::default().fg(Color::Yellow)),
                ]));
                let suggestion_cat_names = category_name_map(&self.categories);
                for (si, (suggestion, decision)) in panel.pending_suggestions.iter().enumerate() {
                    let is_cursor = cat_focused && panel.category_cursor == si;
                    let marker = decision.marker();
                    let marker_style = match decision {
                        SuggestionDecision::Pending => Style::default().fg(Color::Yellow),
                        SuggestionDecision::Accept => Style::default().fg(Color::LightGreen),
                        SuggestionDecision::Reject => Style::default().fg(Color::LightRed),
                    };
                    let marker_style = if is_cursor {
                        marker_style
                            .bg(Color::DarkGray)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        marker_style
                    };
                    let cat_name =
                        candidate_assignment_label(&suggestion.assignment, &suggestion_cat_names);
                    let rationale = suggestion.rationale.as_deref().unwrap_or("text match");
                    let base_style = if is_cursor {
                        Style::default().fg(Color::White).bg(Color::DarkGray)
                    } else {
                        Style::default().fg(Color::White)
                    };
                    let dim_style = if is_cursor {
                        Style::default().fg(Color::Gray).bg(Color::DarkGray)
                    } else {
                        Style::default().fg(Color::Gray)
                    };
                    lines.push(Line::from(vec![
                        Span::styled(format!("{marker} "), marker_style),
                        Span::styled(cat_name.clone(), base_style),
                        Span::styled(format!("  ({rationale})"), dim_style),
                    ]));
                }
                lines.push(Line::from(Span::styled(
                    "─────────────────",
                    Style::default().fg(Color::Yellow),
                )));
            }

            // Category rows
            let cat_lines: Vec<Line<'_>> = if self.category_rows.is_empty() {
                vec![Line::from(Span::styled(
                    "(no categories)",
                    Style::default().fg(MUTED_TEXT_COLOR),
                ))]
            } else if visible_indices.is_empty() {
                vec![Line::from(Span::styled(
                    "(no matching categories)",
                    Style::default().fg(MUTED_TEXT_COLOR),
                ))]
            } else {
                visible_indices
                    .iter()
                    .enumerate()
                    .map(|(i, row_index)| {
                        let row = &self.category_rows[*row_index];
                        let is_assigned = panel.categories.contains(&row.id);
                        let is_numeric =
                            row.value_kind == agenda_core::model::CategoryValueKind::Numeric;
                        let is_cursor =
                            cat_focused && (i + suggestion_len) == panel.category_cursor;

                        let check = if is_assigned && is_numeric {
                            "[#] "
                        } else if is_assigned {
                            "[x] "
                        } else {
                            "[ ] "
                        };

                        let indent = "  ".repeat(row.depth);

                        let base_style = if is_cursor {
                            Style::default().fg(Color::Black).bg(Color::Cyan)
                        } else if row.is_reserved {
                            Style::default()
                                .fg(Color::DarkGray)
                                .add_modifier(Modifier::DIM)
                        } else {
                            Style::default()
                        };

                        let suffix_style = if is_cursor {
                            // On cursor row, suffix keeps same bg but dims fg
                            Style::default().fg(Color::DarkGray).bg(Color::Cyan)
                        } else {
                            Style::default().fg(MUTED_TEXT_COLOR)
                        };

                        let type_suffix = if row.is_reserved {
                            " [reserved]"
                        } else if is_numeric {
                            " [numeric]"
                        } else {
                            ""
                        };

                        let main_prefix = format!("{check}{indent}{}", row.name);

                        // For assigned numeric categories, show value field on the right
                        if is_assigned && is_numeric {
                            if let Some(buf) = panel.numeric_buffers.get(&row.id) {
                                let value_display = buf.text();
                                let value_text = if value_display.is_empty() {
                                    "________".to_string()
                                } else {
                                    value_display.to_string()
                                };
                                let left_len =
                                    main_prefix.chars().count() + type_suffix.chars().count();
                                // value: space + value
                                let value_len = 1 + value_text.chars().count();
                                let total_needed = left_len + value_len;
                                let padding = if inner_width > total_needed {
                                    " ".repeat(inner_width - total_needed)
                                } else {
                                    " ".to_string()
                                };
                                let value_style = if is_cursor {
                                    Style::default().fg(Color::Black).bg(Color::LightCyan)
                                } else {
                                    Style::default().fg(Color::Cyan)
                                };
                                let mut spans = vec![Span::styled(main_prefix, base_style)];
                                if !type_suffix.is_empty() {
                                    spans.push(Span::styled(type_suffix.to_string(), suffix_style));
                                }
                                spans.push(Span::styled(padding, base_style));
                                spans.push(Span::styled(value_text, value_style));
                                return Line::from(spans);
                            }
                        }

                        if type_suffix.is_empty() {
                            Line::from(Span::styled(main_prefix, base_style))
                        } else {
                            Line::from(vec![
                                Span::styled(main_prefix, base_style),
                                Span::styled(type_suffix.to_string(), suffix_style),
                            ])
                        }
                    })
                    .collect()
            };
            lines.extend(cat_lines);

            // Map cursor index to visual line (accounting for separator lines)
            let visual_cursor = if suggestion_len > 0 {
                if panel.category_cursor < suggestion_len {
                    panel.category_cursor + 1 // after "─── Suggested" header
                } else {
                    panel.category_cursor + 2 // after header + closing separator
                }
            } else {
                panel.category_cursor
            };
            let cat_scroll = list_scroll_for_selected_line(cat_list_rect, Some(visual_cursor));
            let item_count = lines.len();

            if cat_list_rect.width > 0 && cat_list_rect.height > 0 {
                frame.render_widget(Paragraph::new(lines).scroll((cat_scroll, 0)), cat_list_rect);
                Self::render_vertical_scrollbar(
                    frame,
                    cat_list_rect,
                    item_count,
                    cat_scroll as usize,
                );
            }
        }

        // Type picker (CategoryCreate only)
        if let Some(type_rect) = regions.type_picker {
            let type_marker = if panel.focus == InputPanelFocus::TypePicker {
                "> "
            } else {
                "  "
            };
            let (tag_label, num_label) = match panel.value_kind {
                agenda_core::model::CategoryValueKind::Tag => ("[Tag]", " Numeric "),
                agenda_core::model::CategoryValueKind::Numeric => (" Tag ", "[Numeric]"),
            };
            let type_style = if panel.focus == InputPanelFocus::TypePicker {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default()
            };
            frame.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled(format!("{type_marker}Type:   "), type_style),
                    Span::raw(tag_label),
                    Span::raw("  "),
                    Span::raw(num_label),
                ])),
                type_rect,
            );
        }

        // Buttons row
        let save_button = if panel.focus == InputPanelFocus::SaveButton {
            "[> Save <]"
        } else {
            "[Save]"
        };
        let cancel_button = if panel.focus == InputPanelFocus::CancelButton {
            "[> Cancel <]"
        } else {
            "[Cancel]"
        };
        let save_style = if panel.focus == InputPanelFocus::SaveButton {
            selected_row_style().add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        let cancel_style = if panel.focus == InputPanelFocus::CancelButton {
            selected_row_style().add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        let actions_focused = matches!(
            panel.focus,
            InputPanelFocus::SaveButton | InputPanelFocus::CancelButton
        );
        let actions_prefix = if actions_focused { "> " } else { "  " };
        let actions_prefix_style = if actions_focused {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(actions_prefix, actions_prefix_style),
                Span::styled(save_button, save_style),
                Span::raw("  "),
                Span::styled(cancel_button, cancel_style),
            ])),
            regions.buttons,
        );

        // Help row
        let base_help = if self.input_panel_discard_confirm_active() {
            "Discard unsaved item edits?  y:discard  n/Esc:keep editing"
        } else {
            match panel.focus {
                InputPanelFocus::Text => match panel.kind {
                    InputPanelKind::NumericValue => {
                        "Type value  Enter:save  Tab:actions  Esc:cancel"
                    }
                    InputPanelKind::NameInput => "Type name  Enter:save  Tab:actions  Esc:cancel",
                    InputPanelKind::WhenDate => {
                        "Enter natural language or ISO datetime  Enter:save  Tab:actions  Esc:cancel"
                    }
                    InputPanelKind::CategoryCreate => "Type name  Tab:next  S:save  Esc:cancel",
                    InputPanelKind::EditItem => "Type title  Tab:note  S:save  Esc:discard?",
                    InputPanelKind::AddItem => "Type title  Tab:note  S:save  Esc:cancel",
                },
                InputPanelFocus::Note => {
                    if panel.kind == InputPanelKind::EditItem {
                        "Type note  Enter:new line  Tab:categories  S:save  Esc:discard?"
                    } else {
                        "Type note  Enter:new line  Tab:categories  S:save  Esc:cancel"
                    }
                }
                InputPanelFocus::Categories if panel.category_filter_editing => {
                    "Type filter  Enter:keep  Esc:done  Tab:next"
                }
                InputPanelFocus::Categories => {
                    "j/k:move  Space:toggle  /:filter  Tab:actions  S:save"
                }
                InputPanelFocus::TypePicker => "Left/Right/Space toggle type  Tab:actions",
                InputPanelFocus::SaveButton => "Enter save  Tab:cancel  Shift-Tab:categories",
                InputPanelFocus::When => {
                    if panel.kind == InputPanelKind::EditItem {
                        "When date (today, tomorrow, 2026-03-25, …)  Tab:note  S:save  Esc:discard?"
                    } else {
                        "When date (today, tomorrow, 2026-03-25, …)  Tab:note  S:save  Esc:cancel"
                    }
                }
                InputPanelFocus::CancelButton => "Enter cancel  Tab:text  Shift-Tab:save",
            }
        };
        let mut help_style = Style::default();
        let help_text = if panel.kind == InputPanelKind::WhenDate {
            let is_when_error = self.status.starts_with("Could not parse")
                || self.status.starts_with("When edit failed:");
            if is_when_error {
                help_style = Style::default().fg(Color::LightRed);
                self.status.clone()
            } else {
                base_help.to_string()
            }
        } else if panel.kind == InputPanelKind::AddItem && !panel.preview_context.is_empty() {
            format!("{} | {}", panel.preview_context, base_help)
        } else {
            base_help.to_string()
        };
        frame.render_widget(Paragraph::new(help_text).style(help_style), regions.help);

        // Second help line for WhenDate: supported format hints.
        if panel.kind == InputPanelKind::WhenDate {
            if let Some(help2_rect) = regions.help2 {
                let is_error = self.status.starts_with("Could not parse")
                    || self.status.starts_with("When edit failed:");
                let hint_style = if is_error {
                    Style::default().fg(Color::LightRed)
                } else {
                    Style::default().fg(MUTED_TEXT_COLOR)
                };
                let hint_text = if is_error {
                    // On error, repeat the error on line 2 (line 1 already has key hints)
                    String::new()
                } else {
                    "today | tomorrow | <weekday> | this/next <weekday> | next/last week/month | next year | in N days/weeks/months | end of week/month | YYYY-MM-DD".to_string()
                };
                frame.render_widget(Paragraph::new(hint_text).style(hint_style), help2_rect);
            }
        }
    }

    pub(crate) fn render_view_picker(&self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        frame.render_widget(Clear, area);

        let items: Vec<ListItem<'_>> = if self.views.is_empty() {
            vec![ListItem::new(Line::from("(no views configured)"))]
        } else {
            self.views
                .iter()
                .map(|view| ListItem::new(Line::from(view.name.clone())))
                .collect()
        };
        let mut state = Self::list_state_for(
            area,
            if self.views.is_empty() {
                None
            } else {
                Some(self.picker_index)
            },
        );
        let item_count = items.len();

        frame.render_stateful_widget(
            List::new(items)
                .highlight_symbol("> ")
                .highlight_style(selected_row_style())
                .block(
                    Block::default()
                        .title("View Palette")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Magenta)),
                ),
            area,
            &mut state,
        );
        Self::render_vertical_scrollbar(frame, area, item_count, state.offset());
    }

    pub(crate) fn render_item_assign_picker(&self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        frame.render_widget(Clear, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(1)])
            .split(area);
        frame.render_widget(
            Paragraph::new(
                "Edit selected item categories (Space toggles, n or / enters category text)",
            ),
            chunks[0],
        );

        let items: Vec<ListItem<'_>> = if self.category_rows.is_empty() {
            vec![ListItem::new(Line::from("(no categories)"))]
        } else {
            self.category_rows
                .iter()
                .map(|row| {
                    let mut flags = Vec::new();
                    if row.is_reserved {
                        flags.push("reserved");
                    }
                    if row.value_kind == agenda_core::model::CategoryValueKind::Numeric {
                        flags.push("numeric");
                    }
                    if row.is_exclusive {
                        flags.push("exclusive");
                    }
                    let suffix = if flags.is_empty() {
                        String::new()
                    } else {
                        format!(" [{}]", flags.join(","))
                    };
                    let (assigned_count, total_count) =
                        self.effective_action_assignment_counts(row.id);
                    let assigned = if total_count > 1 {
                        if assigned_count == 0 {
                            "[ ]".to_string()
                        } else if assigned_count == total_count {
                            "[x]".to_string()
                        } else {
                            "[~]".to_string()
                        }
                    } else if self.selected_item_has_assignment(row.id) {
                        "[x]".to_string()
                    } else {
                        "[ ]".to_string()
                    };
                    let category_name = with_note_marker(row.name.clone(), row.has_note);
                    let text = format!(
                        "{assigned} {}{}{}",
                        "  ".repeat(row.depth),
                        category_name,
                        suffix
                    );
                    ListItem::new(Line::from(text))
                })
                .collect()
        };

        let mut state = Self::list_state_for(
            chunks[1],
            if self.category_rows.is_empty() {
                None
            } else {
                Some(self.item_assign_category_index)
            },
        );
        let item_count = items.len();
        frame.render_stateful_widget(
            List::new(items)
                .highlight_symbol("> ")
                .highlight_style(selected_row_style())
                .block(
                    Block::default()
                        .title("Assign Item")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Cyan)),
                ),
            chunks[1],
            &mut state,
        );
        Self::render_vertical_scrollbar(frame, chunks[1], item_count, state.offset());
    }

    pub(crate) fn render_category_manager(&self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        frame.render_widget(Clear, area);

        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(1)])
            .split(area);
        let manager_focus = self
            .category_manager
            .as_ref()
            .map(|state| state.focus)
            .unwrap_or(CategoryManagerFocus::Tree);
        let filter_text = self
            .category_manager
            .as_ref()
            .map(|state| state.filter.text().to_string())
            .unwrap_or_default();
        let action_prompt = self
            .category_manager_inline_action()
            .map(|action| match action {
                CategoryInlineAction::Rename { buf, .. } => format!("Rename> {}", buf.text()),
                CategoryInlineAction::DeleteConfirm { category_name, .. } => {
                    format!("Delete '{}'? y:confirm Esc:cancel", category_name)
                }
            });
        let classification_mode = modes::classification::continuous_mode_label(
            self.classification_ui.config.continuous_mode,
        );
        let ready_name = self
            .workflow_config
            .ready_category_id
            .and_then(|id| self.categories.iter().find(|c| c.id == id))
            .map(|c| c.name.as_str())
            .unwrap_or("(unset)");
        let claim_name = self
            .workflow_config
            .claim_category_id
            .and_then(|id| self.categories.iter().find(|c| c.id == id))
            .map(|c| c.name.as_str())
            .unwrap_or("(unset)");
        let summary_line = Line::from(vec![
            Span::styled("Auto classification", Style::default().fg(MUTED_TEXT_COLOR)),
            Span::raw(": "),
            Span::styled(
                classification_mode,
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(" (m)", Style::default().fg(MUTED_TEXT_COLOR)),
            Span::styled(" | ", Style::default().fg(MUTED_TEXT_COLOR)),
            Span::styled("Ready queue", Style::default().fg(MUTED_TEXT_COLOR)),
            Span::raw(": "),
            Span::styled(ready_name, Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(" | ", Style::default().fg(MUTED_TEXT_COLOR)),
            Span::styled("Claim result", Style::default().fg(MUTED_TEXT_COLOR)),
            Span::raw(": "),
            Span::styled(claim_name, Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(" (w)", Style::default().fg(MUTED_TEXT_COLOR)),
        ]);
        frame.render_widget(
            Paragraph::new(summary_line)
                .style(Style::default().fg(CATEGORY_MANAGER_PANE_FOCUS))
                .wrap(Wrap { trim: false }),
            layout[0],
        );

        let body = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(62), Constraint::Percentage(38)])
            .split(layout[1]);
        let left = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(1)])
            .split(body[0]);

        let pane_idle = CATEGORY_MANAGER_PANE_IDLE;
        let tree_border = if manager_focus == CategoryManagerFocus::Tree {
            CATEGORY_MANAGER_PANE_FOCUS
        } else {
            pane_idle
        };
        let filter_border = if self.category_manager_filter_editing() {
            CATEGORY_MANAGER_TEXT_ENTRY
        } else if manager_focus == CategoryManagerFocus::Filter {
            CATEGORY_MANAGER_PANE_FOCUS
        } else {
            pane_idle
        };
        let details_border = if manager_focus == CategoryManagerFocus::Details {
            CATEGORY_MANAGER_PANE_FOCUS
        } else {
            pane_idle
        };
        frame.render_widget(
            Paragraph::new(if let Some(prompt) = action_prompt {
                prompt
            } else if manager_focus == CategoryManagerFocus::Filter
                && self.category_manager_filter_editing()
            {
                format!("Filter: {}", filter_text)
            } else if filter_text.trim().is_empty() {
                "Press / to filter. m: auto-classification  w: ready/claim queues.".to_string()
            } else {
                format!("Filter: {}", filter_text)
            })
            .block(
                Block::default()
                    .title(if self.category_manager_inline_action().is_some() {
                        "> Action"
                    } else if self.category_manager_filter_editing() {
                        "> Filter (editing)"
                    } else if manager_focus == CategoryManagerFocus::Filter {
                        "> Filter"
                    } else {
                        "Filter"
                    })
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(filter_border).add_modifier(
                        if manager_focus == CategoryManagerFocus::Filter {
                            Modifier::BOLD
                        } else {
                            Modifier::empty()
                        },
                    )),
            ),
            left[0],
        );
        if let Some((x, y)) = self.category_manager_action_cursor_position(left[0]) {
            frame.set_cursor_position((x, y));
        }

        let table_area = left[1];

        let title_suffix = String::new();

        let visible_row_indices: Vec<usize> = self
            .category_manager_visible_row_indices()
            .map(|rows| rows.to_vec())
            .unwrap_or_else(|| (0..self.category_rows.len()).collect());
        let rows: Vec<Row<'_>> = if visible_row_indices.is_empty() {
            vec![Row::new(vec![Cell::from("(no categories)")])]
        } else {
            visible_row_indices
                .iter()
                .filter_map(|idx| self.category_rows.get(*idx))
                .map(|row| {
                    let mut label = format!("{}{}", "  ".repeat(row.depth), row.name);
                    label = with_note_marker(label, row.has_note);
                    let mut badges = Vec::new();
                    if row.is_reserved {
                        badges.push("reserved");
                    }
                    if row.value_kind == CategoryValueKind::Numeric {
                        badges.push("numeric");
                    } else {
                        if row.is_exclusive {
                            badges.push("exclusive");
                        }
                        if self.workflow_config.ready_category_id == Some(row.id) {
                            badges.push("ready-queue");
                        }
                        if self.workflow_config.claim_category_id == Some(row.id) {
                            badges.push("claim-target");
                        }
                    }
                    if !badges.is_empty() {
                        label.push(' ');
                        label.push_str(
                            &badges
                                .into_iter()
                                .map(|badge| format!("[{badge}]"))
                                .collect::<Vec<_>>()
                                .join(" "),
                        );
                    }
                    Row::new(vec![Cell::from(label)])
                })
                .collect()
        };
        let row_count = rows.len();
        let mut state = Self::table_state_for(
            table_area,
            if visible_row_indices.is_empty() {
                None
            } else {
                Some(
                    self.category_manager_visible_tree_index()
                        .unwrap_or(0)
                        .min(visible_row_indices.len().saturating_sub(1)),
                )
            },
        );
        frame.render_stateful_widget(
            Table::new(rows, vec![Constraint::Min(20)])
                .header(
                    Row::new(vec![Cell::from("Category")])
                        .style(Style::default().add_modifier(Modifier::BOLD)),
                )
                .highlight_symbol("> ")
                .row_highlight_style(selected_row_style())
                .block(
                    Block::default()
                        .title(if manager_focus == CategoryManagerFocus::Tree {
                            format!("> Category Manager{title_suffix}")
                        } else {
                            format!("Category Manager{title_suffix}")
                        })
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(tree_border).add_modifier(
                            if manager_focus == CategoryManagerFocus::Tree {
                                Modifier::BOLD
                            } else {
                                Modifier::empty()
                            },
                        )),
                ),
            table_area,
            &mut state,
        );
        Self::render_vertical_scrollbar(frame, table_area, row_count, state.offset());
        frame.render_widget(
            Block::default()
                .title(if manager_focus == CategoryManagerFocus::Details {
                    "> Details"
                } else {
                    "Details"
                })
                .borders(Borders::ALL)
                .border_style(Style::default().fg(details_border).add_modifier(
                    if manager_focus == CategoryManagerFocus::Details {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    },
                )),
            body[1],
        );
        let details_inner = Rect {
            x: body[1].x.saturating_add(1),
            y: body[1].y.saturating_add(1),
            width: body[1].width.saturating_sub(2),
            height: body[1].height.saturating_sub(2),
        };
        if details_inner.width > 0 && details_inner.height > 0 {
            if let Some(row) = self.selected_category_row() {
                let details_focus = self
                    .category_manager_details_focus()
                    .unwrap_or(CategoryManagerDetailsFocus::Exclusive);
                let note_editing = self.category_manager_details_note_editing();
                let note_dirty = self.category_manager_details_note_dirty();
                let also_match_editing = self.category_manager_details_also_match_editing();
                let also_match_dirty = self.category_manager_details_also_match_dirty();

                let mut parent_name = "(root)".to_string();
                let mut child_count = 0usize;
                if let Some(parent_id) =
                    self.categories
                        .iter()
                        .find(|c| c.id == row.id)
                        .and_then(|c| {
                            child_count = c.children.len();
                            c.parent
                        })
                {
                    if let Some(parent) = self.categories.iter().find(|c| c.id == parent_id) {
                        parent_name = parent.name.clone();
                    }
                }

                let is_numeric_category = row.value_kind == CategoryValueKind::Numeric;
                let numeric_format = if is_numeric_category {
                    self.categories
                        .iter()
                        .find(|c| c.id == row.id)
                        .and_then(|c| c.numeric_format.clone())
                        .unwrap_or_default()
                } else {
                    NumericFormat::default()
                };
                let integer_mode = is_numeric_category && numeric_format.decimal_places == 0;
                let info_height = if is_numeric_category {
                    CATEGORY_DETAILS_INFO_HEIGHT_NUMERIC
                } else {
                    CATEGORY_DETAILS_INFO_HEIGHT
                };
                let is_ready_queue_role = self.selected_category_is_ready_queue_role();
                let is_claim_target_role = self.selected_category_is_claim_target_role();
                let workflow_role_height: u16 = if is_ready_queue_role { 2 } else { 0 }
                    + if is_claim_target_role { 2 } else { 0 };
                let flags_height = if is_numeric_category {
                    7
                } else {
                    5 + workflow_role_height
                };
                let details_chunks = if is_numeric_category {
                    Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([
                            Constraint::Length(info_height),
                            Constraint::Length(flags_height),
                            Constraint::Min(5),
                            Constraint::Length(2),
                        ])
                        .split(details_inner)
                } else {
                    Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([
                            Constraint::Length(info_height),
                            Constraint::Length(flags_height),
                            Constraint::Length(6),
                            Constraint::Min(5),
                            Constraint::Length(2),
                        ])
                        .split(details_inner)
                };
                let note_chunk_index = if is_numeric_category { 2 } else { 3 };
                let hint_chunk_index = if is_numeric_category { 3 } else { 4 };

                let assigned_item_count = self
                    .category_assignment_counts
                    .get(&row.id)
                    .copied()
                    .unwrap_or(0);
                let mut info_lines = vec![
                    Line::from(format!("Selected: {}", row.name)),
                    Line::from(format!(
                        "Depth: {}    Children: {}    Items: {}",
                        row.depth, child_count, assigned_item_count
                    )),
                    Line::from(format!("Parent: {}", parent_name)),
                    Line::from(if row.is_reserved {
                        "Reserved: yes (read-only config)".to_string()
                    } else {
                        "Reserved: no".to_string()
                    }),
                ];
                if is_numeric_category {
                    let preview_val = rust_decimal::Decimal::new(123456, 2);
                    let preview = format_numeric_cell(Some(preview_val), Some(&numeric_format));
                    let decimals_label = if numeric_format.decimal_places == 1 {
                        "1dp".to_string()
                    } else {
                        format!("{}dp", numeric_format.decimal_places)
                    };
                    info_lines.push(Line::from(format!(
                        "Format: {} ({})",
                        preview.trim(),
                        decimals_label
                    )));
                }

                frame.render_widget(
                    Paragraph::new(info_lines).wrap(Wrap { trim: false }),
                    details_chunks[0],
                );

                let focus_prefix = |active: bool| if active { "> " } else { "  " };
                let flag_line = |active: bool, label: &str, on: bool| {
                    let style = if active {
                        focused_cell_style()
                    } else {
                        Style::default()
                    };
                    Line::from(Span::styled(
                        format!(
                            "{}{} {}",
                            focus_prefix(active),
                            if on { "[x]" } else { "[ ]" },
                            label
                        ),
                        style,
                    ))
                };
                let is_numeric = row.value_kind == CategoryValueKind::Numeric;
                let flag_lines = if is_numeric {
                    let inline_input = self.category_manager_details_inline_input();
                    let integer_focused = details_focus == CategoryManagerDetailsFocus::Integer;
                    let decimal_focused =
                        details_focus == CategoryManagerDetailsFocus::DecimalPlaces;
                    let currency_focused =
                        details_focus == CategoryManagerDetailsFocus::CurrencySymbol;
                    let thousands_focused =
                        details_focus == CategoryManagerDetailsFocus::ThousandsSeparator;
                    let decimal_value = inline_input
                        .filter(|input| {
                            input.field == CategoryManagerDetailsInlineField::DecimalPlaces
                        })
                        .map(|input| input.buffer.text().to_string())
                        .unwrap_or_else(|| numeric_format.decimal_places.to_string());
                    let currency_value = inline_input
                        .filter(|input| {
                            input.field == CategoryManagerDetailsInlineField::CurrencySymbol
                        })
                        .map(|input| input.buffer.text().to_string())
                        .unwrap_or_else(|| {
                            numeric_format.currency_symbol.clone().unwrap_or_default()
                        });
                    let decimal_style = if integer_mode {
                        Style::default()
                            .fg(MUTED_TEXT_COLOR)
                            .add_modifier(Modifier::DIM)
                    } else if decimal_focused {
                        focused_cell_style()
                    } else {
                        Style::default()
                    };
                    let currency_style = if currency_focused {
                        focused_cell_style()
                    } else {
                        Style::default()
                    };
                    let thousands_style = if thousands_focused {
                        focused_cell_style()
                    } else {
                        Style::default()
                    };
                    vec![
                        flag_line(integer_focused, "Integer", integer_mode),
                        Line::from(Span::styled(
                            format!(
                                "{}Decimal places: {}",
                                if decimal_focused { "> " } else { "  " },
                                if integer_mode {
                                    "(disabled in Integer mode)".to_string()
                                } else if decimal_value.is_empty() {
                                    "_".to_string()
                                } else {
                                    decimal_value
                                }
                            ),
                            decimal_style,
                        )),
                        Line::from(Span::styled(
                            format!(
                                "{}Currency symbol: {}",
                                if currency_focused { "> " } else { "  " },
                                if currency_value.is_empty() {
                                    "(none)".to_string()
                                } else {
                                    currency_value
                                }
                            ),
                            currency_style,
                        )),
                        Line::from(Span::styled(
                            format!(
                                "{}{} Thousands separator",
                                if thousands_focused { "> " } else { "  " },
                                if numeric_format.use_thousands_separator {
                                    "[x]"
                                } else {
                                    "[ ]"
                                }
                            ),
                            thousands_style,
                        )),
                    ]
                } else {
                    let mut lines = vec![
                        flag_line(
                            details_focus == CategoryManagerDetailsFocus::Exclusive,
                            "Exclusive",
                            row.is_exclusive,
                        ),
                        flag_line(
                            details_focus == CategoryManagerDetailsFocus::MatchName,
                            "Auto-match",
                            row.enable_implicit_string,
                        ),
                        flag_line(
                            details_focus == CategoryManagerDetailsFocus::Actionable,
                            "Actionable",
                            row.is_actionable,
                        ),
                    ];
                    if is_ready_queue_role {
                        lines.push(Line::from(Span::styled(
                            "  Workflow: Ready Queue",
                            Style::default().fg(CATEGORY_MANAGER_PANE_FOCUS),
                        )));
                        lines.push(Line::from(Span::styled(
                            "  (items need this to be claimable)",
                            Style::default().fg(MUTED_TEXT_COLOR),
                        )));
                    }
                    if is_claim_target_role {
                        lines.push(Line::from(Span::styled(
                            "  Workflow: Claim Result",
                            Style::default().fg(CATEGORY_MANAGER_PANE_FOCUS),
                        )));
                        lines.push(Line::from(Span::styled(
                            "  (assigned by the CLI claim workflow)",
                            Style::default().fg(MUTED_TEXT_COLOR),
                        )));
                    }
                    lines
                };
                let flags_title = if is_numeric {
                    "Numeric Format"
                } else {
                    "Flags"
                };
                let flags_border_focused = !matches!(
                    details_focus,
                    CategoryManagerDetailsFocus::Note | CategoryManagerDetailsFocus::AlsoMatch
                );
                frame.render_widget(
                    Paragraph::new(flag_lines).block(
                        Block::default()
                            .title(flags_title)
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(if flags_border_focused {
                                CATEGORY_MANAGER_EDIT_FOCUS
                            } else {
                                pane_idle
                            })),
                    ),
                    details_chunks[1],
                );

                if !is_numeric_category {
                    let also_match_block_focus =
                        details_focus == CategoryManagerDetailsFocus::AlsoMatch;
                    let also_match_title = if also_match_editing {
                        "Also Match (editing)"
                    } else if also_match_dirty {
                        "Also Match (unsaved)"
                    } else {
                        "Also Match"
                    };
                    let also_match_rect = details_chunks[2];
                    if let Some(state) = self.category_manager.as_ref() {
                        let mut also_match_widget = state.details_also_match.widget().clone();
                        also_match_widget.set_placeholder_text(ALSO_MATCH_PLACEHOLDER_TEXT);
                        also_match_widget
                            .set_placeholder_style(Style::default().fg(MUTED_TEXT_COLOR));
                        if also_match_editing {
                            also_match_widget
                                .set_cursor_line_style(Style::default().bg(Color::DarkGray));
                            also_match_widget.set_cursor_style(
                                Style::default()
                                    .fg(Color::Black)
                                    .bg(Color::Yellow)
                                    .add_modifier(Modifier::BOLD),
                            );
                        }
                        also_match_widget.set_block(
                            Block::default()
                                .title(if also_match_block_focus {
                                    format!("> {also_match_title}")
                                } else {
                                    also_match_title.to_string()
                                })
                                .borders(Borders::ALL)
                                .border_style(Style::default().fg(if also_match_editing {
                                    CATEGORY_MANAGER_EDIT_FOCUS
                                } else if also_match_block_focus {
                                    CATEGORY_MANAGER_PANE_FOCUS
                                } else {
                                    pane_idle
                                })),
                        );
                        frame.render_widget(&also_match_widget, also_match_rect);
                        let also_match_scroll = list_scroll_for_selected_line(
                            also_match_rect,
                            Some(state.details_also_match.line_col().0),
                        );
                        Self::render_vertical_scrollbar(
                            frame,
                            also_match_rect,
                            state.details_also_match.text().lines().count().max(1),
                            also_match_scroll as usize,
                        );
                    }
                }

                let note_block_focus = details_focus == CategoryManagerDetailsFocus::Note;
                let note_title = if note_editing {
                    "Note (editing)"
                } else if note_dirty {
                    "Note (unsaved)"
                } else {
                    "Note"
                };
                let note_rect = details_chunks[note_chunk_index];
                if let Some(state) = self.category_manager.as_ref() {
                    let mut note_widget = state.details_note.widget().clone();
                    note_widget.set_placeholder_text(NOTE_PLACEHOLDER_TEXT);
                    note_widget.set_placeholder_style(Style::default().fg(MUTED_TEXT_COLOR));
                    if note_editing {
                        note_widget.set_cursor_line_style(Style::default().bg(Color::DarkGray));
                        note_widget.set_cursor_style(
                            Style::default()
                                .fg(Color::Black)
                                .bg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        );
                    }
                    note_widget.set_block(
                        Block::default()
                            .title(if note_block_focus {
                                format!("> {note_title}")
                            } else {
                                note_title.to_string()
                            })
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(if note_editing {
                                CATEGORY_MANAGER_EDIT_FOCUS
                            } else if note_block_focus {
                                CATEGORY_MANAGER_PANE_FOCUS
                            } else {
                                pane_idle
                            })),
                    );
                    frame.render_widget(&note_widget, note_rect);
                    let note_scroll = list_scroll_for_selected_line(
                        note_rect,
                        Some(state.details_note.line_col().0),
                    );
                    Self::render_vertical_scrollbar(
                        frame,
                        note_rect,
                        state.details_note.text().lines().count().max(1),
                        note_scroll as usize,
                    );
                } else {
                    frame.render_widget(
                        Paragraph::new("").block(
                            Block::default()
                                .title(note_title)
                                .borders(Borders::ALL)
                                .border_style(Style::default().fg(pane_idle)),
                        ),
                        note_rect,
                    );
                }

                let details_hint = if note_editing {
                    "Type to edit  Esc:discard  Tab:leave (warn if unsaved)"
                } else if also_match_editing {
                    "Type terms line-by-line  Enter:new line  Esc:discard  Tab:leave"
                } else {
                    match details_focus {
                        CategoryManagerDetailsFocus::Exclusive => {
                            "Only one child can be assigned to an item at a time"
                        }
                        CategoryManagerDetailsFocus::MatchName => {
                            "Auto-assign when category name appears in item text"
                        }
                        CategoryManagerDetailsFocus::Actionable => {
                            "Items need an actionable category to be marked done"
                        }
                        CategoryManagerDetailsFocus::AlsoMatch => {
                            "Enter/Space: edit also-match terms  One term or phrase per line"
                        }
                        CategoryManagerDetailsFocus::Integer => {
                            "Enter/Space: toggle Integer mode (on sets 0dp, off restores 2dp)"
                        }
                        CategoryManagerDetailsFocus::DecimalPlaces => {
                            if integer_mode {
                                "Disabled while Integer mode is enabled"
                            } else {
                                "Enter: edit decimal places  Esc: cancel edit"
                            }
                        }
                        CategoryManagerDetailsFocus::CurrencySymbol => {
                            "Enter: edit currency symbol  Empty value clears symbol"
                        }
                        CategoryManagerDetailsFocus::ThousandsSeparator => {
                            "Enter/Space: toggle thousands separator"
                        }
                        CategoryManagerDetailsFocus::Note => {
                            "j/k: focus field  Enter/Space: toggle/edit"
                        }
                    }
                };
                frame.render_widget(
                    Paragraph::new(details_hint),
                    details_chunks[hint_chunk_index],
                );
                if let Some((x, y)) = self.category_manager_details_cursor_position(body[1]) {
                    frame.set_cursor_position((x, y));
                }
            } else {
                frame.render_widget(
                    Paragraph::new(vec![
                        Line::from("No category selected"),
                        Line::from(""),
                        Line::from("Select a category to edit flags and note."),
                    ])
                    .wrap(Wrap { trim: false }),
                    details_inner,
                );
            }
        }

        if self.category_manager_discard_confirm() {
            let w = area.width.min(48);
            let h = 5;
            let x = area.x + area.width.saturating_sub(w) / 2;
            let y = area.y + area.height.saturating_sub(h) / 2;
            let overlay_area = Rect::new(x, y, w, h);
            frame.render_widget(Clear, overlay_area);
            frame.render_widget(
                Paragraph::new(vec![
                    Line::from("Save changes before closing?"),
                    Line::from(""),
                    Line::from("y: save and close   n: discard   Esc: keep editing"),
                ])
                .block(
                    Block::default()
                        .title(" Confirm ")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(CATEGORY_MANAGER_EDIT_FOCUS)),
                )
                .wrap(Wrap { trim: false }),
                overlay_area,
            );
        }

        if self.workflow_setup_open {
            let ready_name = self
                .workflow_config
                .ready_category_id
                .and_then(|id| self.categories.iter().find(|c| c.id == id))
                .map(|c| c.name.as_str())
                .unwrap_or("(unset)");
            let claim_name = self
                .workflow_config
                .claim_category_id
                .and_then(|id| self.categories.iter().find(|c| c.id == id))
                .map(|c| c.name.as_str())
                .unwrap_or("(unset)");
            let focus = self.workflow_setup_focus;
            let ready_style = if focus == 0 {
                focused_cell_style()
            } else {
                Style::default()
            };
            let claim_style = if focus == 1 {
                focused_cell_style()
            } else {
                Style::default()
            };
            let indicator = |idx: usize| if focus == idx { "> " } else { "  " };
            let w = area.width.min(58);
            let h = 16u16;
            let x = area.x + area.width.saturating_sub(w) / 2;
            let y = area.y + area.height.saturating_sub(h) / 2;
            let overlay_area = Rect::new(x, y, w, h);
            frame.render_widget(Clear, overlay_area);
            frame.render_widget(
                Paragraph::new(vec![
                    Line::from(Span::styled(
                        "This config enables the CLI claim workflow.",
                        Style::default().fg(MUTED_TEXT_COLOR),
                    )),
                    Line::from(Span::styled(
                        "Pick two categories used by agenda-cli claim:",
                        Style::default().fg(MUTED_TEXT_COLOR),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        "Ready Queue    items eligible to be claimed",
                        Style::default().fg(MUTED_TEXT_COLOR),
                    )),
                    Line::from(Span::styled(
                        format!("{}Ready Queue:   {}", indicator(0), ready_name),
                        ready_style,
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        "Claim Result   category applied after claim",
                        Style::default().fg(MUTED_TEXT_COLOR),
                    )),
                    Line::from(Span::styled(
                        format!("{}Claim Result:  {}", indicator(1), claim_name),
                        claim_style,
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        "Press Enter to choose a category for the",
                        Style::default().fg(MUTED_TEXT_COLOR),
                    )),
                    Line::from(Span::styled(
                        "highlighted role from a category picker.",
                        Style::default().fg(MUTED_TEXT_COLOR),
                    )),
                    Line::from(""),
                    Line::from("j/k:role  Enter:choose  x:clear  Esc:close"),
                ])
                .block(
                    Block::default()
                        .title(" Workflow Setup ")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(CATEGORY_MANAGER_PANE_FOCUS)),
                )
                .wrap(Wrap { trim: false }),
                overlay_area,
            );
        }

        if let Some(picker) = &self.workflow_role_picker {
            let row_indices = self.workflow_role_picker_row_indices();
            let role_label = if picker.role_index == 0 {
                "Ready Queue"
            } else {
                "Claim Result"
            };
            let current_name = if picker.role_index == 0 {
                self.workflow_config
                    .ready_category_id
                    .and_then(|id| self.categories.iter().find(|c| c.id == id))
                    .map(|c| c.name.as_str())
                    .unwrap_or("(unset)")
            } else {
                self.workflow_config
                    .claim_category_id
                    .and_then(|id| self.categories.iter().find(|c| c.id == id))
                    .map(|c| c.name.as_str())
                    .unwrap_or("(unset)")
            };
            let w = area.width.min(64);
            let h = area.height.min(22);
            let x = area.x + area.width.saturating_sub(w) / 2;
            let y = area.y + area.height.saturating_sub(h) / 2;
            let overlay_area = Rect::new(x, y, w, h);
            frame.render_widget(Clear, overlay_area);

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(5),
                    Constraint::Min(8),
                    Constraint::Length(1),
                ])
                .split(overlay_area);

            frame.render_widget(
                Paragraph::new(vec![
                    Line::from(Span::styled(
                        format!("Choose the category used as {role_label}."),
                        Style::default().fg(MUTED_TEXT_COLOR),
                    )),
                    Line::from(Span::styled(
                        format!("Current: {current_name}"),
                        Style::default().fg(MUTED_TEXT_COLOR),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        "Only normal tag categories can be used here.",
                        Style::default().fg(MUTED_TEXT_COLOR),
                    )),
                ])
                .block(
                    Block::default()
                        .title(format!(" Pick {role_label} "))
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(CATEGORY_MANAGER_PANE_FOCUS)),
                )
                .wrap(Wrap { trim: false }),
                chunks[0],
            );

            let items: Vec<ListItem<'_>> = if row_indices.is_empty() {
                vec![ListItem::new(Line::from(Span::styled(
                    "(no eligible categories)",
                    Style::default().fg(MUTED_TEXT_COLOR),
                )))]
            } else {
                row_indices
                    .iter()
                    .filter_map(|row_index| self.category_rows.get(*row_index))
                    .map(|row| {
                        let mut suffixes = Vec::new();
                        if row.is_exclusive {
                            suffixes.push("exclusive");
                        }
                        if self.workflow_config.ready_category_id == Some(row.id) {
                            suffixes.push("ready");
                        }
                        if self.workflow_config.claim_category_id == Some(row.id) {
                            suffixes.push("claim");
                        }
                        let suffix = if suffixes.is_empty() {
                            String::new()
                        } else {
                            format!(" [{}]", suffixes.join(","))
                        };
                        let text = format!(
                            "{}{}{}",
                            "  ".repeat(row.depth),
                            with_note_marker(row.name.clone(), row.has_note),
                            suffix
                        );
                        ListItem::new(Line::from(text))
                    })
                    .collect()
            };

            let mut state = Self::list_state_for(
                chunks[1],
                if row_indices.is_empty() {
                    None
                } else {
                    Some(picker.row_index.min(row_indices.len().saturating_sub(1)))
                },
            );
            frame.render_stateful_widget(
                List::new(items)
                    .highlight_symbol("> ")
                    .highlight_style(selected_row_style())
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(CATEGORY_MANAGER_EDIT_FOCUS)),
                    ),
                chunks[1],
                &mut state,
            );
            Self::render_vertical_scrollbar(frame, chunks[1], row_indices.len(), state.offset());

            frame.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    "j/k:select  Enter:assign  Esc:back",
                    Style::default().fg(MUTED_TEXT_COLOR),
                ))),
                chunks[2],
            );
        }

        if self.classification_mode_picker_open {
            let focus = self.classification_mode_picker_focus;
            let style_for = |idx: usize| {
                if focus == idx {
                    focused_cell_style()
                } else {
                    Style::default()
                }
            };
            let indicator = |idx: usize| if focus == idx { "> " } else { "  " };
            let current_mode = modes::classification::continuous_mode_label(
                self.classification_ui.config.continuous_mode,
            );
            let w = area.width.min(54);
            let h = 15u16;
            let x = area.x + area.width.saturating_sub(w) / 2;
            let y = area.y + area.height.saturating_sub(h) / 2;
            let overlay_area = Rect::new(x, y, w, h);
            frame.render_widget(Clear, overlay_area);
            frame.render_widget(
                Paragraph::new(vec![
                    Line::from(Span::styled(
                        "How should categories be assigned to",
                        Style::default().fg(MUTED_TEXT_COLOR),
                    )),
                    Line::from(Span::styled(
                        "new or edited items?",
                        Style::default().fg(MUTED_TEXT_COLOR),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        format!("Current: {current_mode}"),
                        Style::default().fg(MUTED_TEXT_COLOR),
                    )),
                    Line::from(Span::styled(
                        format!("{}Off             no auto-classification", indicator(0)),
                        style_for(0),
                    )),
                    Line::from(Span::styled(
                        format!(
                            "{}Auto-apply      assign matches instantly (default)",
                            indicator(1)
                        ),
                        style_for(1),
                    )),
                    Line::from(Span::styled(
                        format!("{}Suggest/Review  queue for manual approval", indicator(2)),
                        style_for(2),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        "Classification runs when you save an item or change categories.",
                        Style::default().fg(MUTED_TEXT_COLOR),
                    )),
                    Line::from(""),
                    Line::from(Span::styled(
                        "j/k:select  Enter:apply  Esc:close",
                        Style::default().fg(MUTED_TEXT_COLOR),
                    )),
                ])
                .block(
                    Block::default()
                        .title(" Classification Mode ")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(CATEGORY_MANAGER_PANE_FOCUS)),
                )
                .wrap(Wrap { trim: false }),
                overlay_area,
            );
        }
    }

    // -------------------------------------------------------------------------
    // ViewEdit (unified view editor)
    // -------------------------------------------------------------------------

    pub(crate) fn render_view_edit_screen(&self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        let Some(state) = &self.view_edit_state else {
            return;
        };

        let preview_three_column = state.preview_visible && area.width >= 120 && area.height >= 12;
        let preview_stacked_right = state.preview_visible && !preview_three_column;
        let panes = if preview_three_column {
            Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(36),
                    Constraint::Percentage(42),
                    Constraint::Percentage(22),
                ])
                .split(area)
        } else {
            Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
                .split(area)
        };
        let sections_area = panes[0];
        let (details_area, preview_area) = if preview_three_column && panes.len() > 2 {
            (panes[1], Some(panes[2]))
        } else if preview_stacked_right {
            let right = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(68), Constraint::Percentage(32)])
                .split(panes[1]);
            (right[0], Some(right[1]))
        } else {
            (panes[1], None)
        };

        let focused_border = Color::Cyan;
        let inactive_border = Color::Blue;

        let category_names = category_name_map(&self.categories);

        // ── Details pane (row-based, view or selected section) ──────────────
        {
            let criterion_mode_label = |mode: CriterionMode| -> &'static str {
                match mode {
                    CriterionMode::And => "Include",
                    CriterionMode::Not => "Exclude",
                    CriterionMode::Or => "Match any",
                }
            };

            let summarize_query = |query: &Query| -> Vec<String> {
                query
                    .criteria
                    .iter()
                    .map(|criterion| {
                        let name = category_names
                            .get(&criterion.category_id)
                            .cloned()
                            .unwrap_or_else(|| "(deleted)".to_string());
                        format!("{}: {}", criterion_mode_label(criterion.mode), name)
                    })
                    .collect()
            };

            let summarize_category_set = |set: &std::collections::HashSet<CategoryId>| -> String {
                if set.is_empty() {
                    return "(none)".to_string();
                }
                let mut names: Vec<String> = set
                    .iter()
                    .map(|id| {
                        category_names
                            .get(id)
                            .cloned()
                            .unwrap_or_else(|| "(deleted)".to_string())
                    })
                    .collect();
                names.sort_by_key(|s| s.to_ascii_lowercase());
                names.join(", ")
            };

            let show_view_details = state.region != ViewEditRegion::Sections
                || state.sections_view_row_selected
                || state.draft.sections.get(state.section_index).is_none();
            let details_focused = state.pane_focus == ViewEditPaneFocus::Details;
            let details_border = if details_focused {
                focused_border
            } else {
                inactive_border
            };

            let mut items: Vec<ListItem<'_>> = Vec::new();
            let mut selected_line: Option<usize> = None;
            let title = if show_view_details {
                format!(" DETAILS: View  matches:{} ", state.preview_count)
            } else {
                let section_name = state
                    .draft
                    .sections
                    .get(state.section_index)
                    .map(|s| s.title.as_str())
                    .unwrap_or("?");
                format!(" DETAILS: {} ", section_name)
            };

            if show_view_details {
                let display_mode_label = match state.draft.board_display_mode {
                    BoardDisplayMode::SingleLine => "single-line",
                    BoardDisplayMode::MultiLine => "multi-line",
                };
                let section_flow_label = match state.draft.section_flow {
                    SectionFlow::Vertical => "vertical (stacked lanes)",
                    SectionFlow::Horizontal => "horizontal (kanban lanes)",
                };

                let separator_style = Style::default().fg(Color::DarkGray);
                let pad = 26; // column alignment width

                // Helper: style + track selected_line for unmatched-region fields
                let style_for_unmatched_field =
                    |field_index: usize,
                     items: &[ListItem<'_>],
                     selected_line_ref: &mut Option<usize>| {
                        if details_focused
                            && state.region == ViewEditRegion::Unmatched
                            && state.unmatched_field_index == field_index
                        {
                            *selected_line_ref = Some(items.len());
                            Style::default().add_modifier(Modifier::REVERSED)
                        } else {
                            Style::default()
                        }
                    };

                // ── Name ──
                let editing_view_name =
                    matches!(state.inline_input, Some(ViewEditInlineInput::ViewName));
                let view_name_text = if editing_view_name {
                    format!("◀ {}", state.inline_buf.text())
                } else {
                    state.draft.name.clone()
                };
                let view_name_style = if editing_view_name {
                    selected_line = Some(0);
                    Style::default().add_modifier(Modifier::REVERSED)
                } else {
                    Style::default()
                };
                items.push(
                    ListItem::new(Line::from(format!(
                        "  {:<width$}{}",
                        "Name",
                        view_name_text,
                        width = pad
                    )))
                    .style(view_name_style),
                );

                // ── Separator ──
                items.push(ListItem::new(Line::from(Span::styled(
                    "  ─────────────────────────────────────────",
                    separator_style,
                ))));

                // ── Filter criteria ──
                items.push(ListItem::new(Line::from("  Filter criteria:")));

                let criteria_row_start = items.len();
                let criteria_lines = summarize_query(&state.draft.criteria);
                if criteria_lines.is_empty() {
                    let style = if details_focused && state.region == ViewEditRegion::Criteria {
                        selected_line = Some(criteria_row_start);
                        Style::default().add_modifier(Modifier::REVERSED)
                    } else {
                        Style::default()
                    };
                    items.push(
                        ListItem::new(Line::from("    (no criteria — matches all items)"))
                            .style(style),
                    );
                } else {
                    for (i, criterion) in criteria_lines.iter().enumerate() {
                        let style = if details_focused
                            && state.region == ViewEditRegion::Criteria
                            && i == state.criteria_index
                        {
                            selected_line = Some(criteria_row_start + i);
                            Style::default().add_modifier(Modifier::REVERSED)
                        } else {
                            Style::default()
                        };
                        items.push(
                            ListItem::new(Line::from(format!("    {criterion}"))).style(style),
                        );
                    }
                }

                // ── Separator ──
                items.push(ListItem::new(Line::from(Span::styled(
                    "  ─────────────────────────────────────────",
                    separator_style,
                ))));

                // ── Date range ──
                let when_include = if state.draft.criteria.virtual_include.is_empty() {
                    "(all)".to_string()
                } else {
                    when_bucket_options()
                        .iter()
                        .filter(|b| state.draft.criteria.virtual_include.contains(*b))
                        .map(|b| when_bucket_label(*b).to_string())
                        .collect::<Vec<String>>()
                        .join(", ")
                };
                let when_exclude = if state.draft.criteria.virtual_exclude.is_empty() {
                    "(none)".to_string()
                } else {
                    when_bucket_options()
                        .iter()
                        .filter(|b| state.draft.criteria.virtual_exclude.contains(*b))
                        .map(|b| when_bucket_label(*b).to_string())
                        .collect::<Vec<String>>()
                        .join(", ")
                };

                items.push(
                    ListItem::new(Line::from(format!(
                        "  {:<width$}{}",
                        "Date range (include)",
                        when_include,
                        width = pad
                    )))
                    .style(style_for_unmatched_field(
                        0,
                        &items,
                        &mut selected_line,
                    )),
                );
                items.push(
                    ListItem::new(Line::from(format!(
                        "  {:<width$}{}",
                        "Date range (exclude)",
                        when_exclude,
                        width = pad
                    )))
                    .style(style_for_unmatched_field(
                        1,
                        &items,
                        &mut selected_line,
                    )),
                );

                // ── Separator ──
                items.push(ListItem::new(Line::from(Span::styled(
                    "  ─────────────────────────────────────────",
                    separator_style,
                ))));

                // ── Display ──
                items.push(
                    ListItem::new(Line::from(format!(
                        "  {:<width$}{}",
                        "Display mode",
                        display_mode_label,
                        width = pad
                    )))
                    .style(style_for_unmatched_field(
                        2,
                        &items,
                        &mut selected_line,
                    )),
                );
                items.push(
                    ListItem::new(Line::from(format!(
                        "  {:<width$}{}",
                        "Section flow",
                        section_flow_label,
                        width = pad
                    )))
                    .style(style_for_unmatched_field(
                        3,
                        &items,
                        &mut selected_line,
                    )),
                );

                let configured_aliases: Vec<_> = state
                    .draft
                    .category_aliases
                    .iter()
                    .filter(|(_, alias)| !alias.trim().is_empty())
                    .map(|(cat_id, alias)| {
                        let cat_name = self
                            .categories
                            .iter()
                            .find(|c| c.id == *cat_id)
                            .map(|c| c.name.as_str())
                            .unwrap_or("?");
                        (cat_name.to_string(), alias.clone())
                    })
                    .collect();
                let alias_summary = if configured_aliases.is_empty() {
                    "(none)".to_string()
                } else {
                    format!("{} configured", configured_aliases.len())
                };
                items.push(
                    ListItem::new(Line::from(format!(
                        "  {:<width$}{}",
                        "Aliases",
                        alias_summary,
                        width = pad
                    )))
                    .style(style_for_unmatched_field(
                        7,
                        &items,
                        &mut selected_line,
                    )),
                );
                // Show configured aliases as indented sub-rows (display-only)
                for (cat_name, alias) in &configured_aliases {
                    items.push(ListItem::new(Line::from(Span::styled(
                        format!("      {} \u{2192} {}", cat_name, alias),
                        Style::default().fg(MUTED_TEXT_COLOR),
                    ))));
                }

                // ── Separator ──
                items.push(ListItem::new(Line::from(Span::styled(
                    "  ─────────────────────────────────────────",
                    separator_style,
                ))));

                // ── Unmatched ──
                let unmatched_value = if state.draft.show_unmatched {
                    format!("yes, as \"{}\"", state.draft.unmatched_label)
                } else {
                    "hidden".to_string()
                };
                items.push(
                    ListItem::new(Line::from(format!(
                        "  {:<width$}{}",
                        "Show unmatched",
                        unmatched_value,
                        width = pad
                    )))
                    .style(style_for_unmatched_field(
                        4,
                        &items,
                        &mut selected_line,
                    )),
                );

                let hide_dependent_value = if state.draft.hide_dependent_items {
                    "yes".to_string()
                } else {
                    "no".to_string()
                };
                items.push(
                    ListItem::new(Line::from(format!(
                        "  {:<width$}{}",
                        "Hide dependent",
                        hide_dependent_value,
                        width = pad
                    )))
                    .style(style_for_unmatched_field(
                        5,
                        &items,
                        &mut selected_line,
                    )),
                );

                let unmatched_label_text = if matches!(
                    state.inline_input,
                    Some(ViewEditInlineInput::UnmatchedLabel)
                ) {
                    format!("◀ {}", state.inline_buf.text())
                } else {
                    format!("\"{}\"", state.draft.unmatched_label)
                };
                items.push(
                    ListItem::new(Line::from(format!(
                        "  {:<width$}{}",
                        "Unmatched label",
                        unmatched_label_text,
                        width = pad
                    )))
                    .style(style_for_unmatched_field(
                        6,
                        &items,
                        &mut selected_line,
                    )),
                );
            } else if let Some(section) = state.draft.sections.get(state.section_index) {
                let editing_title = matches!(
                    state.inline_input,
                    Some(ViewEditInlineInput::SectionTitle { section_index })
                    if section_index == state.section_index
                );
                let title_text = if editing_title {
                    format!("◀ {}", state.inline_buf.text())
                } else {
                    section.title.clone()
                };

                let separator_style = Style::default().fg(Color::DarkGray);
                let pad = 26; // column alignment width

                // Helper: style + track selected_line for a given field index
                let style_for_section_field =
                    |field_index: usize,
                     items: &[ListItem<'_>],
                     selected_line_ref: &mut Option<usize>| {
                        if details_focused
                            && state.region == ViewEditRegion::Sections
                            && state.section_details_field_index == field_index
                        {
                            *selected_line_ref = Some(items.len());
                            Style::default().add_modifier(Modifier::REVERSED)
                        } else {
                            Style::default()
                        }
                    };

                // ── Group 1: Identity ──
                items.push(
                    ListItem::new(Line::from(format!(
                        "  {:<width$}{}",
                        "Title",
                        title_text,
                        width = pad
                    )))
                    .style(style_for_section_field(
                        0,
                        &items,
                        &mut selected_line,
                    )),
                );

                let criteria_lines = summarize_query(&section.criteria);
                let criteria_value = if criteria_lines.is_empty() {
                    "(none)".to_string()
                } else {
                    criteria_lines.join("; ")
                };
                items.push(
                    ListItem::new(Line::from(format!(
                        "  {:<width$}{}",
                        "Filter",
                        criteria_value,
                        width = pad
                    )))
                    .style(style_for_section_field(
                        1,
                        &items,
                        &mut selected_line,
                    )),
                );

                // ── Separator ──
                items.push(ListItem::new(Line::from(Span::styled(
                    "  ─────────────────────────────────────────",
                    separator_style,
                ))));

                // ── Group 2: Display ──
                let columns_summary = if section.columns.is_empty() {
                    "(none)".to_string()
                } else {
                    section
                        .columns
                        .iter()
                        .map(|column| {
                            category_names
                                .get(&column.heading)
                                .cloned()
                                .unwrap_or_else(|| "(deleted)".to_string())
                        })
                        .collect::<Vec<String>>()
                        .join(", ")
                };
                items.push(
                    ListItem::new(Line::from(format!(
                        "  {:<width$}{}",
                        "Columns",
                        columns_summary,
                        width = pad
                    )))
                    .style(style_for_section_field(
                        2,
                        &items,
                        &mut selected_line,
                    )),
                );
                let mode_label = match section.board_display_mode_override {
                    None => "(use view default)".to_string(),
                    Some(BoardDisplayMode::SingleLine) => "single-line".to_string(),
                    Some(BoardDisplayMode::MultiLine) => "multi-line".to_string(),
                };
                items.push(
                    ListItem::new(Line::from(format!(
                        "  {:<width$}{}",
                        "Display mode",
                        mode_label,
                        width = pad
                    )))
                    .style(style_for_section_field(
                        6,
                        &items,
                        &mut selected_line,
                    )),
                );

                // ── Separator ──
                items.push(ListItem::new(Line::from(Span::styled(
                    "  ─────────────────────────────────────────",
                    separator_style,
                ))));

                // ── Group 3: Automation / Behavior ──
                items.push(
                    ListItem::new(Line::from(format!(
                        "  {:<width$}{}",
                        "Auto-assign on add",
                        summarize_category_set(&section.on_insert_assign),
                        width = pad
                    )))
                    .style(style_for_section_field(
                        3,
                        &items,
                        &mut selected_line,
                    )),
                );
                items.push(
                    ListItem::new(Line::from(format!(
                        "  {:<width$}{}",
                        "Auto-unassign on remove",
                        summarize_category_set(&section.on_remove_unassign),
                        width = pad
                    )))
                    .style(style_for_section_field(
                        4,
                        &items,
                        &mut selected_line,
                    )),
                );
                items.push(
                    ListItem::new(Line::from(format!(
                        "  {:<width$}{}",
                        "Section layout",
                        self.view_edit_section_layout_value(section),
                        width = pad
                    )))
                    .style(style_for_section_field(
                        5,
                        &items,
                        &mut selected_line,
                    )),
                );
                if details_focused
                    && state.region == ViewEditRegion::Sections
                    && state.section_details_field_index == 5
                {
                    let help_style = Style::default().fg(Color::Rgb(170, 178, 198));
                    items.push(ListItem::new(Line::from(Span::styled(
                        "    Behavior:",
                        help_style,
                    ))));
                    items.push(ListItem::new(Line::from(Span::styled(
                        "    - Flat: keeps one section",
                        help_style,
                    ))));
                    items.push(ListItem::new(Line::from(Span::styled(
                        "    - Split: creates child lanes + \"(Other)\"",
                        help_style,
                    ))));
                    items.push(ListItem::new(Line::from(Span::styled(
                        "    Requirements: single Include parent, no Exclude/Any/Text/Date filters.",
                        help_style,
                    ))));
                }
            } else {
                items.push(ListItem::new(Line::from("  No selection")));
            }

            let content_len = items.len();
            let block = Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(details_border));
            let mut list_state = Self::list_state_for(details_area, selected_line);
            frame.render_stateful_widget(
                List::new(items).block(block),
                details_area,
                &mut list_state,
            );
            Self::render_vertical_scrollbar(frame, details_area, content_len, list_state.offset());
        }

        // ── Sections region ──────────────────────────────────────────────────
        {
            let inline_editing_section = state.inline_input.as_ref().and_then(|inp| {
                if let ViewEditInlineInput::SectionTitle { section_index } = inp {
                    Some(*section_index)
                } else {
                    None
                }
            });

            let filter_active = !state.sections_filter_buf.trimmed().is_empty();
            let filter_editing = matches!(
                state.inline_input,
                Some(ViewEditInlineInput::SectionsFilter)
            );
            let dirty_marker = if state.dirty { " *" } else { "" };
            let sections_title = if filter_editing {
                format!(
                    " SECTIONS{dirty_marker}  /{}◀ ",
                    state.sections_filter_buf.text()
                )
            } else if filter_active {
                format!(
                    " SECTIONS{dirty_marker}  /{} ",
                    state.sections_filter_buf.text()
                )
            } else {
                format!(" SECTIONS{dirty_marker} ")
            };
            let block = Block::default()
                .title(sections_title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(
                    if state.pane_focus == ViewEditPaneFocus::Sections {
                        focused_border
                    } else {
                        inactive_border
                    },
                ));

            let mut items: Vec<ListItem<'_>> = Vec::new();
            let mut selected_line: Option<usize> = None;
            let view_row_focused = state.region != ViewEditRegion::Sections
                || (state.region == ViewEditRegion::Sections && state.sections_view_row_selected);
            let sections_pane_focused = state.pane_focus == ViewEditPaneFocus::Sections;
            if view_row_focused {
                selected_line = Some(0);
            }
            let view_row_style = if view_row_focused && sections_pane_focused {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
            items.push(
                ListItem::new(Line::from(format!(
                    "{}  View: {}",
                    if view_row_focused { ">" } else { " " },
                    state.draft.name,
                )))
                .style(view_row_style),
            );

            let visible_section_indices: Vec<usize> = {
                let q = state.sections_filter_buf.trimmed().to_ascii_lowercase();
                if q.is_empty() {
                    (0..state.draft.sections.len()).collect()
                } else {
                    state
                        .draft
                        .sections
                        .iter()
                        .enumerate()
                        .filter_map(|(i, s)| {
                            if s.title.to_ascii_lowercase().contains(&q) {
                                Some(i)
                            } else {
                                None
                            }
                        })
                        .collect()
                }
            };

            if state.draft.sections.is_empty() {
                items.push(ListItem::new(Line::from("  (no sections — n:add)")));
            } else if visible_section_indices.is_empty() {
                items.push(ListItem::new(Line::from("  (no matching sections)")));
            } else {
                for i in visible_section_indices {
                    let section = &state.draft.sections[i];
                    if i == state.section_index
                        && !state.sections_view_row_selected
                        && sections_pane_focused
                    {
                        selected_line = Some(items.len());
                    }
                    let cursor = if i == state.section_index
                        && state.region == ViewEditRegion::Sections
                        && sections_pane_focused
                        && !state.sections_view_row_selected
                    {
                        ">"
                    } else {
                        " "
                    };
                    let title = if inline_editing_section == Some(i) {
                        format!(
                            "{}  {}. {} ◀ editing",
                            cursor,
                            i + 1,
                            state.inline_buf.text()
                        )
                    } else {
                        format!("{} {}. {}", cursor, i + 1, section.title)
                    };

                    let style = if i == state.section_index
                        && state.region == ViewEditRegion::Sections
                        && sections_pane_focused
                        && !state.sections_view_row_selected
                    {
                        Style::default().add_modifier(Modifier::REVERSED)
                    } else {
                        Style::default()
                    };
                    items.push(ListItem::new(Line::from(title)).style(style));
                }
            }

            let content_len = items.len();
            let mut list_state = Self::list_state_for(sections_area, selected_line);
            frame.render_stateful_widget(
                List::new(items).block(block),
                sections_area,
                &mut list_state,
            );
            Self::render_vertical_scrollbar(frame, sections_area, content_len, list_state.offset());
        }

        // ── Preview pane (optional) ─────────────────────────────────────────
        if let Some(preview_area) = preview_area {
            let preview_focused = state.pane_focus == ViewEditPaneFocus::Preview;
            let preview_border = if preview_focused {
                focused_border
            } else {
                inactive_border
            };

            let reference_date = jiff::Zoned::now().date();
            let resolved = resolve_view(
                &state.draft,
                &self.all_items,
                &self.categories,
                reference_date,
            );
            let mut preview_items: Vec<ListItem<'_>> = Vec::new();
            preview_items.push(ListItem::new(Line::from(format!(
                "  Matches: {}",
                state.preview_count
            ))));
            preview_items.push(ListItem::new(Line::from(format!(
                "  Sections: {} configured",
                state.draft.sections.len()
            ))));
            preview_items.push(ListItem::new(Line::from("")));

            if resolved.sections.is_empty() {
                preview_items.push(ListItem::new(Line::from("  (no section lanes)")));
            } else {
                for section in &resolved.sections {
                    let subsection_count = section.subsections.len();
                    let section_count = if subsection_count == 0 {
                        section.items.len()
                    } else {
                        section.subsections.iter().map(|s| s.items.len()).sum()
                    };
                    preview_items.push(ListItem::new(Line::from(format!(
                        "  {}: {}",
                        section.title, section_count
                    ))));
                    if subsection_count > 0 {
                        preview_items.push(ListItem::new(Line::from(format!(
                            "    generated: {}",
                            subsection_count
                        ))));
                    }
                }
            }

            let unmatched_count = resolved
                .unmatched
                .as_ref()
                .map(|items| items.len())
                .unwrap_or(0);
            preview_items.push(ListItem::new(Line::from("")));
            preview_items.push(ListItem::new(Line::from(format!(
                "  Unmatched: {} ({})",
                if state.draft.show_unmatched {
                    "shown"
                } else {
                    "hidden"
                },
                unmatched_count
            ))));
            preview_items.push(ListItem::new(Line::from(format!(
                "  Dependent items: {}",
                if state.draft.hide_dependent_items {
                    "hidden"
                } else {
                    "shown"
                }
            ))));

            let selected_preview_row = if preview_items.is_empty() {
                None
            } else {
                Some(
                    state
                        .preview_scroll
                        .min(preview_items.len().saturating_sub(1)),
                )
            };
            let content_len = preview_items.len();
            let mut list_state = Self::list_state_for(preview_area, selected_preview_row);
            frame.render_stateful_widget(
                List::new(preview_items)
                    .block(
                        Block::default()
                            .title(" PREVIEW ")
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(preview_border)),
                    )
                    .highlight_style(if preview_focused {
                        Style::default().add_modifier(Modifier::REVERSED)
                    } else {
                        Style::default()
                    }),
                preview_area,
                &mut list_state,
            );
            Self::render_vertical_scrollbar(frame, preview_area, content_len, list_state.offset());
        }

        // ── Picker overlay ───────────────────────────────────────────────────
        if let Some(overlay) = &state.overlay {
            let overlay_area = {
                let w = details_area.width.max(1);
                Rect::new(details_area.x, details_area.y, w, details_area.height)
            };
            frame.render_widget(Clear, overlay_area);
            match overlay {
                ViewEditOverlay::CategoryPicker { target } => {
                    let overlay_filter = state.overlay_filter_buf.text();
                    let filtered_indices = self.view_edit_filtered_category_row_indices(state);
                    let selected_filtered_index = filtered_indices
                        .iter()
                        .position(|&i| i == state.picker_index)
                        .unwrap_or(0);
                    let is_criteria_picker = matches!(
                        target,
                        CategoryEditTarget::ViewCriteria | CategoryEditTarget::SectionCriteria
                    );
                    let is_alias_picker = matches!(target, CategoryEditTarget::ViewAliases);
                    let toggle_hint = if is_alias_picker {
                        "A/Enter edit alias"
                    } else if is_criteria_picker {
                        "Space/Enter cycle mode"
                    } else {
                        "Space/Enter toggle"
                    };
                    let title = if overlay_filter.trim().is_empty() {
                        format!(
                            " Pick categories  {}/{}  (type filter, {toggle_hint}, Esc done) ",
                            (selected_filtered_index + 1).min(filtered_indices.len().max(1)),
                            filtered_indices.len()
                        )
                    } else {
                        format!(
                            " Pick categories /{}  {}/{} ",
                            overlay_filter,
                            (selected_filtered_index + 1).min(filtered_indices.len().max(1)),
                            filtered_indices.len()
                        )
                    };
                    let section_index = state.section_index;
                    let items: Vec<ListItem<'_>> = self
                        .category_rows
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| filtered_indices.contains(i))
                        .map(|(i, row)| {
                            let indent = "  ".repeat(row.depth);
                            let is_criteria_target = matches!(
                                target,
                                CategoryEditTarget::ViewCriteria
                                    | CategoryEditTarget::SectionCriteria
                            );
                            let criterion_mode = if is_criteria_target {
                                let query = match target {
                                    CategoryEditTarget::ViewCriteria => Some(&state.draft.criteria),
                                    CategoryEditTarget::ViewAliases => None,
                                    CategoryEditTarget::SectionCriteria => {
                                        state.draft.sections.get(section_index).map(|s| &s.criteria)
                                    }
                                    _ => None,
                                };
                                query.and_then(|q| q.mode_for(row.id))
                            } else {
                                None
                            };
                            let label = if is_criteria_target {
                                let tag = match criterion_mode {
                                    None => "   ",
                                    Some(CriterionMode::And) => "Inc",
                                    Some(CriterionMode::Not) => "Exc",
                                    Some(CriterionMode::Or) => "Any",
                                };
                                format!("{indent}[{tag}] {}", row.name)
                            } else if is_alias_picker {
                                let active_alias_edit = matches!(
                                    state.inline_input,
                                    Some(ViewEditInlineInput::CategoryAlias { category_id })
                                    if category_id == row.id
                                );
                                let current_alias = if active_alias_edit {
                                    state.inline_buf.text().to_string()
                                } else {
                                    state
                                        .draft
                                        .category_aliases
                                        .get(&row.id)
                                        .cloned()
                                        .unwrap_or_default()
                                };
                                let alias_text = if current_alias.trim().is_empty() {
                                    "(none)".to_string()
                                } else {
                                    current_alias
                                };
                                let edit_marker = if active_alias_edit { " ◀" } else { "" };
                                format!("{indent}{}  alias: {alias_text}{edit_marker}", row.name)
                            } else {
                                let checked = match target {
                                    CategoryEditTarget::ViewAliases => false,
                                    CategoryEditTarget::SectionColumns => state
                                        .draft
                                        .sections
                                        .get(section_index)
                                        .map(|section| {
                                            section.columns.iter().any(|col| col.heading == row.id)
                                        })
                                        .unwrap_or(false),
                                    CategoryEditTarget::SectionOnInsertAssign => state
                                        .draft
                                        .sections
                                        .get(section_index)
                                        .map(|section| section.on_insert_assign.contains(&row.id))
                                        .unwrap_or(false),
                                    CategoryEditTarget::SectionOnRemoveUnassign => state
                                        .draft
                                        .sections
                                        .get(section_index)
                                        .map(|section| section.on_remove_unassign.contains(&row.id))
                                        .unwrap_or(false),
                                    _ => false,
                                };
                                format!(
                                    "{indent}[{}] {}",
                                    if checked { "x" } else { " " },
                                    row.name
                                )
                            };
                            let style = if i == state.picker_index {
                                Style::default().add_modifier(Modifier::REVERSED)
                            } else {
                                Style::default()
                            };
                            ListItem::new(Line::from(label)).style(style)
                        })
                        .collect();
                    let block = Block::default()
                        .title(title)
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Yellow));
                    let mut list_state = Self::list_state_for(
                        overlay_area,
                        if filtered_indices.is_empty() {
                            None
                        } else {
                            Some(selected_filtered_index)
                        },
                    );
                    frame.render_stateful_widget(
                        List::new(items).block(block),
                        overlay_area,
                        &mut list_state,
                    );
                }
                ViewEditOverlay::BucketPicker { .. } => {
                    let options = when_bucket_options();
                    let items: Vec<ListItem<'_>> = options
                        .iter()
                        .enumerate()
                        .map(|(i, bucket)| {
                            let label = format!("  {}", when_bucket_label(*bucket));
                            let style = if i == state.picker_index {
                                Style::default().add_modifier(Modifier::REVERSED)
                            } else {
                                Style::default()
                            };
                            ListItem::new(Line::from(label)).style(style)
                        })
                        .collect();
                    let block = Block::default()
                        .title(" Pick bucket ")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Yellow));
                    let mut list_state =
                        Self::list_state_for(overlay_area, Some(state.picker_index));
                    frame.render_stateful_widget(
                        List::new(items).block(block),
                        overlay_area,
                        &mut list_state,
                    );
                }
            }
        }

        // ── Discard confirmation overlay ──────────────────────────────────────
        if state.discard_confirm {
            let w = area.width.min(48);
            let h = 5;
            let x = area.x + area.width.saturating_sub(w) / 2;
            let y = area.y + area.height.saturating_sub(h) / 2;
            let overlay_area = Rect::new(x, y, w, h);
            frame.render_widget(Clear, overlay_area);
            frame.render_widget(
                Paragraph::new(vec![
                    Line::from("Save changes before closing?"),
                    Line::from(""),
                    Line::from("y: save and close   n: discard   Esc: keep editing"),
                ])
                .block(
                    Block::default()
                        .title(" Confirm ")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Yellow)),
                )
                .wrap(Wrap { trim: false }),
                overlay_area,
            );
        }

        if let Some(section_index) = state.section_delete_confirm {
            let section_name = state
                .draft
                .sections
                .get(section_index)
                .map(|s| s.title.clone())
                .unwrap_or_else(|| "(missing)".to_string());
            let w = area.width.min(64);
            let h = 6;
            let x = area.x + area.width.saturating_sub(w) / 2;
            let y = area.y + area.height.saturating_sub(h) / 2;
            let overlay_area = Rect::new(x, y, w, h);
            frame.render_widget(Clear, overlay_area);
            frame.render_widget(
                Paragraph::new(vec![
                    Line::from("Delete section?"),
                    Line::from(format!("\"{section_name}\"")),
                    Line::from(""),
                    Line::from("y:confirm  Esc:cancel"),
                ])
                .block(
                    Block::default()
                        .title(" Confirm Delete ")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Yellow)),
                )
                .wrap(Wrap { trim: false }),
                overlay_area,
            );
        }
    }
}
