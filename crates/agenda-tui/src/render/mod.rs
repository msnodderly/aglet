use crate::*;

const MUTED_TEXT_COLOR: Color = Color::Rgb(140, 140, 140);

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
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(1),
                Constraint::Length(4),
            ])
            .split(frame.area());

        let header = self.render_header();
        frame.render_widget(header, layout[0]);

        self.render_main(frame, layout[1]);

        let footer_area = layout[2];
        let footer = self.render_footer(footer_area.width);
        frame.render_widget(footer, footer_area);
        if let Some((x, y)) = self.input_cursor_position(footer_area) {
            frame.set_cursor_position((x, y));
        }
        if self.mode == Mode::InputPanel {
            if let Some(ref panel) = self.input_panel {
                let popup_area = input_panel_popup_area(frame.area());
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
            self.render_link_wizard(frame, centered_rect(72, 72, frame.area()));
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
                        let heading = self
                            .categories
                            .iter()
                            .find(|c| c.id == column.heading)
                            .map(|c| c.name.as_str())
                            .unwrap_or("?");
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
                Constraint::Length(2), // target query
                Constraint::Min(5),    // target matches
                Constraint::Length(4), // preview
                Constraint::Length(2), // help
            ])
            .split(inner);

        let anchor_lines = vec![
            Line::from("Anchor item"),
            Line::from(format!("  {}", truncate_board_cell(&anchor_label, 72))),
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
        frame.render_widget(
            List::new(action_items).block(action_block),
            rows[1],
        );

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
            for (idx, item_id) in matches.iter().enumerate() {
                let label = self
                    .all_items
                    .iter()
                    .find(|item| item.id == *item_id)
                    .map(|item| {
                        let status = if item.is_done { "done" } else { "open" };
                        format!("{status} | {}", item.text)
                    })
                    .unwrap_or_else(|| format!("missing | {item_id}"));
                let marker = if idx == state.target_index { ">" } else { " " };
                target_items.push(ListItem::new(format!("{marker} {}", truncate_board_cell(&label, 72))));
            }
            if target_items.is_empty() {
                target_items.push(ListItem::new("  (no matches)"));
            }
        } else {
            target_items.push(ListItem::new("  Clear dependencies removes prereqs and blocked items"));
        }
        frame.render_widget(
            List::new(target_items).block(
                Block::default()
                    .title("Matches")
                    .borders(Borders::ALL)
                    .border_style(if state.focus == LinkWizardFocus::Target {
                        Style::default().fg(Color::Yellow)
                    } else {
                        Style::default()
                    }),
            ),
            rows[3],
        );

        let preview_lines = {
            let mut lines = vec![Line::from("Preview")];
            match action {
                LinkWizardAction::BlockedBy => {
                    let target = selected_target_id.and_then(|id| self.all_items.iter().find(|item| item.id == id));
                    if let Some(target) = target {
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
                    } else {
                        lines.push(Line::from("  Select a target item"));
                        lines.push(Line::from(""));
                    }
                }
                LinkWizardAction::DependsOn => {
                    let target = selected_target_id.and_then(|id| self.all_items.iter().find(|item| item.id == id));
                    if let Some(target) = target {
                        lines.push(Line::from(format!(
                            "  {} depends on {}",
                            truncate_board_cell(&anchor_label, 28),
                            truncate_board_cell(&target.text, 28)
                        )));
                        lines.push(Line::from(""));
                    } else {
                        lines.push(Line::from("  Select a target item"));
                        lines.push(Line::from(""));
                    }
                }
                LinkWizardAction::Blocks => {
                    let target = selected_target_id.and_then(|id| self.all_items.iter().find(|item| item.id == id));
                    if let Some(target) = target {
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
                    } else {
                        lines.push(Line::from("  Select a target item"));
                        lines.push(Line::from(""));
                    }
                }
                LinkWizardAction::RelatedTo => {
                    let target = selected_target_id.and_then(|id| self.all_items.iter().find(|item| item.id == id));
                    if let Some(target) = target {
                        lines.push(Line::from(format!(
                            "  {} related to {}",
                            truncate_board_cell(&anchor_label, 26),
                            truncate_board_cell(&target.text, 26)
                        )));
                        lines.push(Line::from("  (symmetric)"));
                    } else {
                        lines.push(Line::from("  Select a target item"));
                        lines.push(Line::from(""));
                    }
                }
                LinkWizardAction::ClearDependencies => {
                    lines.push(Line::from("  Remove all immediate prereqs and dependents"));
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
            Paragraph::new("j/k:move  Tab:focus  Enter:next/apply  b/B:different block direction  d/r/c:action  /:target  Esc:cancel")
                .wrap(Wrap { trim: false })
                .style(Style::default().fg(MUTED_TEXT_COLOR)),
            rows[5],
        );
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
                Constraint::Length(3),
                Constraint::Min(6),
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
                "Column: {}  Item: {}\nSelected: {}  Mode: {}",
                state.parent_name,
                truncate_board_cell(&state.item_label, 28),
                truncate_board_cell(&selected_display, 28),
                if state.is_exclusive {
                    "single"
                } else {
                    "multi"
                }
            ))
            .style(Style::default().fg(MUTED_TEXT_COLOR))
            .wrap(Wrap { trim: true }),
            chunks[0],
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
            chunks[1],
        );

        if let Some(name) = state.create_confirm_name.as_deref() {
            self.render_category_create_confirm_panel(
                frame,
                chunks[2],
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
            let mut list_state = Self::list_state_for(chunks[2], selected);
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
                chunks[2],
                &mut list_state,
            );
            Self::render_vertical_scrollbar(
                frame,
                chunks[2],
                matches.len().max(1),
                list_state.offset(),
            );
        }

        frame.render_widget(
            Paragraph::new(
                "Type filter | j/k or Up/Down move | Space toggle | Enter save | Esc cancel",
            )
            .style(Style::default().fg(MUTED_TEXT_COLOR))
            .wrap(Wrap { trim: true }),
            chunks[3],
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
                Constraint::Length(3),
                Constraint::Min(6),
                Constraint::Length(2),
            ])
            .split(inner);
        let input_area = chunks[1];
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

    pub(crate) fn input_prompt_prefix(&self) -> Option<String> {
        match self.mode {
            Mode::NoteEdit => {
                let dirty = self.input.text() != self.note_edit_original;
                let marker = if dirty { " *" } else { "" };
                Some(format!("Note{marker}> "))
            }
            Mode::FilterInput => Some("Filter> ".to_string()),
            Mode::ItemAssignInput => Some("Category> ".to_string()),
            _ => None,
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
                let prefix_len = "  Text> ".chars().count().min(u16::MAX as usize) as u16;
                let input_chars = panel.text.cursor().min(u16::MAX as usize) as u16;
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
                let note_inner = regions.note_inner?;
                if note_inner.width == 0 || note_inner.height == 0 {
                    return None;
                }
                let note_rect = regions.note?;
                let (line, col) = panel.note.line_col();
                let scroll = list_scroll_for_selected_line(note_rect, Some(line)) as usize;
                let visible_line = line.saturating_sub(scroll);
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
                    .saturating_add(visible_line.min(u16::MAX as usize) as u16)
                    .min(max_y);
                Some((cursor_x, cursor_y))
            }
            InputPanelFocus::Categories => {
                // Show cursor in the numeric value field if on an assigned numeric category row.
                let cat_rect = regions.categories?;
                let cat_inner = regions.categories_inner?;
                if cat_inner.width == 0 || cat_inner.height == 0 {
                    return None;
                }
                let row = self.category_rows.get(panel.category_cursor)?;
                let is_assigned = panel.categories.contains(&row.id);
                let is_numeric =
                    row.value_kind == agenda_core::model::CategoryValueKind::Numeric;
                if is_assigned && is_numeric {
                    if let Some(buf) = panel.numeric_buffers.get(&row.id) {
                        // Value field is right-aligned: "[value_]"
                        // The cursor should be positioned within that field.
                        let value_text_len = buf.text().chars().count() + 2; // "[" + text + "]"
                        let buf_cursor = buf.cursor();
                        // Position: end of inner rect - value_text_len + 1 (for "[") + buf_cursor
                        let field_start_x = cat_inner
                            .x
                            .saturating_add(cat_inner.width)
                            .saturating_sub(value_text_len as u16);
                        let cursor_x = field_start_x
                            .saturating_add(1) // skip "["
                            .saturating_add(buf_cursor.min(u16::MAX as usize) as u16)
                            .min(cat_inner.x.saturating_add(cat_inner.width.saturating_sub(1)));
                        let scroll =
                            list_scroll_for_selected_line(cat_rect, Some(panel.category_cursor))
                                as usize;
                        let visible_row = panel.category_cursor.saturating_sub(scroll);
                        let cursor_y = cat_inner
                            .y
                            .saturating_add(visible_row.min(u16::MAX as usize) as u16)
                            .min(cat_inner.y.saturating_add(cat_inner.height.saturating_sub(1)));
                        return Some((cursor_x, cursor_y));
                    }
                }
                None
            }
            InputPanelFocus::SaveButton | InputPanelFocus::CancelButton => None,
        }
    }

    pub(crate) fn render_header(&self) -> Paragraph<'_> {
        let view_name = self
            .current_view()
            .map(|view| view.name.as_str())
            .unwrap_or("(none)");
        let mode = format!("{:?}", self.mode);
        let active_filters = self.section_filters.iter().filter(|f| f.is_some()).count();
        let filter = if active_filters > 0 {
            format!(" filters:{active_filters}")
        } else {
            String::new()
        };

        Paragraph::new(Line::from(vec![
            Span::styled(
                "Agenda Reborn",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!("  view:{view_name}  mode:{mode}{filter}")),
        ]))
    }

    pub(crate) fn render_main(&self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        if self.mode == Mode::ViewEdit {
            self.render_view_edit_screen(frame, area);
            return;
        }
        if matches!(self.mode, Mode::CategoryManager) {
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
        let columns = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(area);

        let category_names = category_name_map(&self.categories);
        let current_view = self.current_view().cloned();
        let view_item_label = current_view
            .as_ref()
            .and_then(|v| v.item_column_label.clone())
            .filter(|label| !label.trim().is_empty())
            .unwrap_or_else(|| "Item".to_string());
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
                    &category_names,
                    &view_item_label,
                    inner_width,
                );
                let mut item_width = layout.item;
                let mut synthetic_categories_width = 0usize;
                if include_all_categories_in_dynamic {
                    let min_item_width = BOARD_ITEM_MIN_WIDTH.min(item_width);
                    let available_for_categories = item_width.saturating_sub(min_item_width);
                    if available_for_categories > 0 {
                        synthetic_categories_width =
                            BOARD_CATEGORY_TARGET_WIDTH.min(available_for_categories);
                        item_width = item_width.saturating_sub(synthetic_categories_width);
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
                header_cells.extend(
                    layout.columns[..item_board_column_index]
                        .iter()
                        .map(|column| Cell::from(column.label.clone())),
                );
                header_cells.push(Cell::from(layout.item_label.clone()));
                header_cells.extend(
                    layout.columns[item_board_column_index..]
                        .iter()
                        .map(|column| Cell::from(column.label.clone())),
                );
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
                            let is_selected = is_selected_slot && item_index == self.item_index;
                            let marker_cell = if is_selected { ">" } else { " " };
                            let note_cell = item_indicator_glyphs(
                                item.is_done,
                                self.is_item_blocked(item.id),
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
                                if is_selected && self.column_index == item_board_column_index {
                                    cell = cell.style(focused_cell_style());
                                }
                                cell
                            };
                            let mut category_cells: Vec<Cell<'_>> = layout
                                .columns
                                .iter()
                                .enumerate()
                                .map(|(col_idx, column)| {
                                    let is_numeric_column = column.heading_value_kind
                                        == CategoryValueKind::Numeric;
                                    let value = match column.kind {
                                        ColumnKind::When => item
                                            .when_date
                                            .map(|dt| dt.to_string())
                                            .unwrap_or_else(|| "\u{2013}".to_string()),
                                        ColumnKind::Standard if is_numeric_column => {
                                            let numeric_val = item
                                                .assignments
                                                .get(&column.heading_id)
                                                .and_then(|a| a.numeric_value);
                                            format_numeric_cell(numeric_val, None)
                                        }
                                        ColumnKind::Standard => standard_column_value(
                                            item,
                                            &column.child_ids,
                                            &category_names,
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
                                    if is_selected && self.column_index == board_column_index {
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
                                let categories = item_assignment_labels(item, &category_names);
                                let categories_text = if categories.is_empty() {
                                    "-".to_string()
                                } else {
                                    if effective_display_mode == BoardDisplayMode::MultiLine {
                                        format_category_values_multi_line(
                                            &categories,
                                            BOARD_MULTI_CATEGORY_LINE_CAP,
                                        )
                                        .join("\n")
                                    } else {
                                        categories.join(", ")
                                    }
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
                            if is_selected {
                                row = row.style(selected_board_row_style());
                            }
                            row
                        })
                        .collect()
                };

                // Append SUM/AVG footer rows for sections with numeric columns.
                let has_numeric_columns = layout
                    .columns
                    .iter()
                    .any(|c| c.heading_value_kind == CategoryValueKind::Numeric);
                let mut rows = rows;
                if has_numeric_columns && !slot.items.is_empty() {
                    let item_refs: Vec<&Item> = slot.items.iter().collect();
                    let aggregates = compute_column_aggregates(&item_refs, &layout.columns);
                    let footer_style = Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD);

                    for (label, extract_value) in [
                        ("SUM", Box::new(|agg: &NumericAggregate| Some(agg.sum)) as Box<dyn Fn(&NumericAggregate) -> Option<rust_decimal::Decimal>>),
                        ("AVG", Box::new(|agg: &NumericAggregate| agg.avg()) as Box<dyn Fn(&NumericAggregate) -> Option<rust_decimal::Decimal>>),
                    ] {
                        let mut footer_cells = vec![
                            Cell::from(String::new()),
                            Cell::from(String::new()),
                        ];
                        let category_footer_cells: Vec<Cell<'_>> = aggregates
                            .iter()
                            .enumerate()
                            .map(|(col_idx, agg_opt)| {
                                let text = agg_opt
                                    .as_ref()
                                    .and_then(|agg| {
                                        if agg.count == 0 {
                                            None
                                        } else {
                                            extract_value(agg)
                                        }
                                    })
                                    .map(|v| {
                                        right_pad_cell(
                                            &format_numeric_cell(Some(v), None),
                                            layout.columns[col_idx].width,
                                        )
                                    })
                                    .unwrap_or_default();
                                Cell::from(text).style(footer_style)
                            })
                            .collect();
                        let mut left_cats: Vec<Cell<'_>> =
                            category_footer_cells[..item_board_column_index].to_vec();
                        let right_cats: Vec<Cell<'_>> =
                            category_footer_cells[item_board_column_index..].to_vec();
                        footer_cells.append(&mut left_cats);
                        footer_cells
                            .push(Cell::from(format!("  {label}")).style(footer_style));
                        footer_cells.extend(right_cats);
                        if synthetic_categories_width > 0 {
                            footer_cells.push(Cell::from(String::new()));
                        }
                        rows.push(Row::new(footer_cells));
                    }
                }

                let mut state = Self::table_state_for(columns[slot_index], selected_row);
                frame.render_stateful_widget(
                    Table::new(rows, constraints)
                        .column_spacing(0)
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
                    columns[slot_index],
                    &mut state,
                );
                Self::render_vertical_scrollbar(
                    frame,
                    columns[slot_index],
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
                                .map(|dt| dt.to_string())
                                .unwrap_or_else(|| "-".to_string());
                            let marker_cell = if is_selected { ">" } else { " " };
                            let note_cell = item_indicator_glyphs(
                                item.is_done,
                                self.is_item_blocked(item.id),
                                has_note_text(item.note.as_deref()),
                            );
                            let item_text = board_item_label(item);
                            let categories = item_assignment_labels(item, &category_names);
                            let categories_text = if categories.is_empty() {
                                "-".to_string()
                            } else {
                                if effective_display_mode == BoardDisplayMode::MultiLine {
                                    format_category_values_multi_line(
                                        &categories,
                                        BOARD_MULTI_CATEGORY_LINE_CAP,
                                    )
                                    .join("\n")
                                } else {
                                    categories.join(", ")
                                }
                            };
                            let when_text = if effective_display_mode == BoardDisplayMode::MultiLine
                            {
                                truncate_board_cell(&when, widths.when)
                            } else {
                                truncate_board_cell(&when, widths.when)
                            };
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
                            }
                            row
                        })
                        .collect()
                };
                let mut state = Self::table_state_for(columns[slot_index], selected_row);
                frame.render_stateful_widget(
                    Table::new(rows, constraints)
                        .column_spacing(0)
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
                Self::render_vertical_scrollbar(
                    frame,
                    columns[slot_index],
                    slot.items.len(),
                    state.offset(),
                );
            }
        }
    }

    pub(crate) fn render_preview_provenance_panel(
        &self,
        frame: &mut ratatui::Frame<'_>,
        area: Rect,
    ) {
        let mut items = vec![
            ListItem::new(Line::from("Provenance")),
            ListItem::new(Line::from(
                "f focus | j/k or J/K scroll | o summary | u unassign",
            )),
        ];
        let mut selected_line = None;
        if let Some(item) = self.selected_item() {
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
                        .title("Preview: Provenance")
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
        let mut lines = vec![
            Line::from("Summary"),
            Line::from("f focus | j/k or J/K scroll | o provenance"),
            Line::from(""),
            Line::from("Categories"),
        ];

        if categories.is_empty() {
            lines.push(Line::from("  (none)"));
        } else {
            lines.push(Line::from(format!("  {}", categories.join(", "))));
        }

        if let Some(links) = self.item_links_by_item_id.get(&item.id) {
            lines.push(Line::from(""));
            Self::push_link_summary_section(&mut lines, "Prereqs", self.item_link_preview_labels(&links.depends_on));
            Self::push_link_summary_section(&mut lines, "Blocks", self.item_link_preview_labels(&links.blocks));
            Self::push_link_summary_section(&mut lines, "Related", self.item_link_preview_labels(&links.related));
        }

        lines.push(Line::from(""));
        lines.push(Line::from("Note"));
        match &item.note {
            Some(note) if !note.is_empty() => {
                for line in note.lines() {
                    lines.push(Line::from(format!("  {}", line)));
                }
            }
            _ => lines.push(Line::from("  (none)")),
        }
        lines
    }

    fn push_link_summary_section(lines: &mut Vec<Line<'_>>, label: &str, rows: Vec<String>) {
        lines.push(Line::from(label.to_string()));
        if rows.is_empty() {
            lines.push(Line::from("  (none)"));
            return;
        }
        for row in rows {
            lines.push(Line::from(format!("  {row}")));
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
                    (
                        id.to_string(),
                        format!("missing | {}", id),
                    )
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
                Line::from("f focus | j/k or J/K scroll | o provenance"),
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

    pub(crate) fn render_footer(&self, _width: u16) -> Paragraph<'_> {
        let status = self.footer_status_text();
        let hints = self.footer_hint_text();
        let text = ratatui::text::Text::from(vec![
            ratatui::text::Line::from(status),
            ratatui::text::Line::from(ratatui::text::Span::styled(
                hints,
                Style::default().fg(MUTED_TEXT_COLOR),
            )),
        ]);
        Paragraph::new(text).block(Block::default().borders(Borders::ALL))
    }

    fn footer_status_text(&self) -> String {
        match self.mode {
            Mode::NoteEdit => {
                let dirty = self.input.text() != self.note_edit_original;
                let marker = if dirty { " *" } else { "" };
                format!("Note{marker}> {}", self.input.text())
            }
            Mode::FilterInput => {
                let target = self.filter_target_section;
                let section_name = self
                    .slots
                    .get(target)
                    .map(|s| s.title.as_str())
                    .unwrap_or("section");
                format!("Filter [{section_name}]> {}", self.input.text())
            }
            Mode::ConfirmDelete => "Delete item? y:confirm Esc:cancel".to_string(),
            Mode::BoardColumnDeleteConfirm => {
                if let Some(name) = &self.board_pending_delete_column_label {
                    format!("Delete column '{name}'? y:confirm Esc:cancel")
                } else {
                    "Delete column? y:confirm Esc:cancel".to_string()
                }
            }
            Mode::ViewDeleteConfirm => "Delete view? y:confirm Esc:cancel".to_string(),
            Mode::ItemAssignPicker => "Assign categories (changes apply immediately)".to_string(),
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
                        },
                        match panel.focus {
                            InputPanelFocus::Text => "Text",
                            InputPanelFocus::Note => "Note",
                            InputPanelFocus::Categories => "Categories",
                            InputPanelFocus::SaveButton => "Save",
                            InputPanelFocus::CancelButton => "Cancel",
                        }
                    )
                } else {
                    self.status.clone()
                }
            }
            _ => self.status.clone(),
        }
    }

    fn footer_hint_text(&self) -> &'static str {
        match self.mode {
            Mode::CategoryManager => {
                if self.category_manager_details_note_editing() {
                    "S:save  Esc:discard"
                } else {
                    "n:new  r:rename  x:delete  Tab:pane  /:filter  Esc:close"
                }
            }
            Mode::ViewPicker => "Enter:switch  N:new  r:rename  e:edit  x:delete  Esc:cancel",
            Mode::ViewDeleteConfirm => "y:confirm  Esc:cancel",
            Mode::ViewEdit => {
                if let Some(state) = &self.view_edit_state {
                    if state.pane_focus == ViewEditPaneFocus::Sections {
                        "S:save  n:new  x:delete  Enter:details  Tab:pane  Esc:cancel"
                    } else if state.pane_focus == ViewEditPaneFocus::Preview {
                        "S:save  p:hide  Tab:pane  Esc:cancel"
                    } else {
                        "S:save  n:new  x:delete  Space:toggle  Tab:pane  Esc:cancel"
                    }
                } else {
                    "S:save  Tab:pane  Esc:cancel"
                }
            }
            Mode::ItemAssignPicker => "Space:toggle  n:new  Enter:done  Esc:cancel",
            Mode::ItemAssignInput => "Enter:assign  Esc:cancel",
            Mode::LinkWizard => "Tab:focus  Enter:apply  Esc:cancel",
            Mode::CategoryDirectEdit => {
                "S:save  Tab:focus  Enter:resolve  x:remove  Esc:cancel"
            }
            Mode::CategoryColumnPicker => "Space:toggle  Enter:save  Esc:cancel",
            Mode::BoardAddColumnPicker => "Enter:insert  Tab:complete  Esc:cancel",
            Mode::ConfirmDelete
            | Mode::BoardColumnDeleteConfirm
            | Mode::CategoryCreateConfirm { .. } => "y:confirm  Esc:cancel",
            Mode::FilterInput => "Enter:apply  Esc:cancel",
            Mode::NoteEdit => "Enter:save  Esc:cancel",
            Mode::InspectUnassign => "Enter:unassign  Esc:cancel",
            Mode::InputPanel => {
                if self
                    .input_panel
                    .as_ref()
                    .map_or(false, |p| p.focus == input_panel::InputPanelFocus::Categories)
                {
                    "S:save  Tab:next  Space:toggle  Esc:cancel"
                } else {
                    "S:save  Tab:next  Esc:cancel"
                }
            }
            _ => "n:new  e:edit  d:done  a:assign  /:filter  v:views  q:quit",
        }
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
        };
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));
        frame.render_widget(block, area);

        let Some(regions) = input_panel_popup_regions(area, panel.kind) else {
            return;
        };

        // Text field (with inline preview context for AddItem)
        let text_marker = if panel.focus == InputPanelFocus::Text {
            "> "
        } else {
            "  "
        };
        let text_label = if panel.kind == InputPanelKind::NameInput {
            "Name"
        } else {
            "Text"
        };
        let mut text_spans = vec![Span::raw(format!(
            "{text_marker}{text_label}> {}",
            panel.text.text()
        ))];
        if !panel.preview_context.is_empty() {
            text_spans.push(Span::styled(
                format!("  {}", panel.preview_context),
                Style::default().fg(MUTED_TEXT_COLOR),
            ));
        }
        frame.render_widget(Paragraph::new(Line::from(text_spans)), regions.text);

        // Note (not shown for NameInput)
        if let Some(note_rect) = regions.note {
            let note_lines: Vec<Line<'_>> = if panel.note.is_empty() {
                vec![Line::from("")]
            } else {
                panel.note.text().lines().map(Line::from).collect()
            };
            let note_focused = panel.focus == InputPanelFocus::Note;
            let note_border_color = if note_focused {
                Color::Cyan
            } else {
                Color::Blue
            };
            let note_title = "Note";
            let note_cursor_line = panel.note.line_col().0;
            let note_scroll = list_scroll_for_selected_line(note_rect, Some(note_cursor_line));
            frame.render_widget(
                Paragraph::new(note_lines)
                    .scroll((note_scroll, 0))
                    .block(
                        Block::default()
                            .title(note_title)
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(note_border_color)),
                    )
                    .wrap(Wrap { trim: false }),
                note_rect,
            );
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
            let cat_border_color = if cat_focused {
                Color::Cyan
            } else {
                Color::Blue
            };
            let cat_title = "Categories";

            let cat_inner = regions.categories_inner.unwrap_or(cat_rect);
            let inner_width = cat_inner.width as usize;

            let lines: Vec<Line<'_>> = if self.category_rows.is_empty() {
                vec![Line::from(Span::styled(
                    "(no categories)",
                    Style::default().fg(MUTED_TEXT_COLOR),
                ))]
            } else {
                self.category_rows
                    .iter()
                    .enumerate()
                    .map(|(i, row)| {
                        let is_assigned = panel.categories.contains(&row.id);
                        let is_numeric = row.value_kind
                            == agenda_core::model::CategoryValueKind::Numeric;
                        let is_cursor = cat_focused && i == panel.category_cursor;

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
                            Style::default().fg(MUTED_TEXT_COLOR)
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
                                let left_len = main_prefix.chars().count()
                                    + type_suffix.chars().count();
                                // value: space + value
                                let value_len = 1 + value_text.chars().count();
                                let total_needed = left_len + value_len;
                                let padding = if inner_width > total_needed {
                                    " ".repeat(inner_width - total_needed)
                                } else {
                                    " ".to_string()
                                };
                                let value_style = if is_cursor {
                                    Style::default().fg(Color::Black).bg(Color::Yellow)
                                } else {
                                    Style::default().fg(Color::Yellow)
                                };
                                let mut spans = vec![
                                    Span::styled(main_prefix, base_style),
                                ];
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

            let cat_scroll =
                list_scroll_for_selected_line(cat_rect, Some(panel.category_cursor));
            let item_count = lines.len();

            frame.render_widget(
                Paragraph::new(lines)
                    .scroll((cat_scroll, 0))
                    .block(
                        Block::default()
                            .title(cat_title)
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(cat_border_color)),
                    ),
                cat_rect,
            );
            Self::render_vertical_scrollbar(
                frame,
                cat_rect,
                item_count,
                cat_scroll as usize,
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
        frame.render_widget(
            Paragraph::new(format!("  {save_button}  {cancel_button}")),
            regions.buttons,
        );

        // Help row
        frame.render_widget(
            Paragraph::new("S:save  Tab:cycle  Space:toggle  j/k:move  Esc:cancel"),
            regions.help,
        );
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
                    if row.is_exclusive {
                        flags.push("exclusive");
                    }
                    let suffix = if flags.is_empty() {
                        String::new()
                    } else {
                        format!(" [{}]", flags.join(","))
                    };
                    let assigned = if self.selected_item_has_assignment(row.id) {
                        if row.value_kind == agenda_core::model::CategoryValueKind::Numeric {
                            // Show numeric value for assigned numeric categories
                            let val = self
                                .selected_item()
                                .and_then(|item| item.assignments.get(&row.id))
                                .and_then(|a| a.numeric_value);
                            match val {
                                Some(v) => format!("[{}]", v),
                                None => "[x]".to_string(),
                            }
                        } else {
                            "[x]".to_string()
                        }
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
                CategoryInlineAction::Create {
                    parent_id,
                    buf,
                    confirm_name,
                } => {
                    if let Some(name) = confirm_name {
                        format!(
                            "Create '{}' under {}? y:confirm Esc:cancel",
                            name,
                            self.category_manager_parent_label(*parent_id)
                        )
                    } else {
                        format!("Create> {}", buf.text())
                    }
                }
                CategoryInlineAction::Rename { buf, .. } => format!("Rename> {}", buf.text()),
                CategoryInlineAction::DeleteConfirm { category_name, .. } => {
                    format!("Delete '{}'? y:confirm Esc:cancel", category_name)
                }
                CategoryInlineAction::ParentPicker {
                    target_category_name,
                    filter,
                    visible_option_indices,
                    focus,
                    ..
                } => {
                    let focus_label = match focus {
                        CategoryParentPickerFocus::Filter => "filter",
                        CategoryParentPickerFocus::List => "list",
                    };
                    if filter.text().trim().is_empty() {
                        format!(
                            "Reparent {} | parent filter ({}) [{}]",
                            target_category_name,
                            visible_option_indices.len(),
                            focus_label
                        )
                    } else {
                        format!(
                            "Reparent {} | parent filter> {} ({}) [{}]",
                            target_category_name,
                            filter.text(),
                            visible_option_indices.len(),
                            focus_label
                        )
                    }
                }
            });
        frame.render_widget(
            Paragraph::new(
                "Categories are global. Tree editor: inline create/move plus details-pane note editing.",
            ),
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

        let pane_idle = Color::DarkGray;
        let tree_border = if manager_focus == CategoryManagerFocus::Tree {
            Color::Yellow
        } else {
            pane_idle
        };
        let filter_border = if manager_focus == CategoryManagerFocus::Filter {
            Color::LightMagenta
        } else {
            pane_idle
        };
        let details_border = if manager_focus == CategoryManagerFocus::Details {
            Color::White
        } else {
            pane_idle
        };
        frame.render_widget(
            Paragraph::new(if let Some(prompt) = action_prompt {
                prompt
            } else if filter_text.trim().is_empty() {
                "Filter: (type / then text to narrow list)".to_string()
            } else {
                format!("Filter: {}", filter_text)
            })
            .block(
                Block::default()
                    .title(if self.category_manager_inline_action().is_some() {
                        "> Action"
                    } else {
                        if manager_focus == CategoryManagerFocus::Filter {
                            "> Filter"
                        } else {
                            "Filter"
                        }
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

        let show_inline_parent_picker = matches!(
            self.category_manager_inline_action(),
            Some(CategoryInlineAction::ParentPicker { .. })
        );
        let table_area = if show_inline_parent_picker {
            let left_body = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(60), Constraint::Min(4)])
                .split(left[1]);
            left_body[0]
        } else {
            left[1]
        };

        let title_suffix = String::new();

        let visible_row_indices: Vec<usize> = self
            .category_manager_visible_row_indices()
            .map(|rows| rows.to_vec())
            .unwrap_or_else(|| (0..self.category_rows.len()).collect());
        let rows: Vec<Row<'_>> = if visible_row_indices.is_empty() {
            vec![Row::new(vec![
                Cell::from("(no categories)"),
                Cell::from(String::new()),
                Cell::from(String::new()),
                Cell::from(String::new()),
            ])]
        } else {
            visible_row_indices
                .iter()
                .filter_map(|idx| self.category_rows.get(*idx))
                .map(|row| {
                    let mut label = format!("{}{}", "  ".repeat(row.depth), row.name);
                    label = with_note_marker(label, row.has_note);
                    if row.is_reserved {
                        label.push_str(" [reserved]");
                    }
                    Row::new(vec![
                        Cell::from(label),
                        Cell::from(if row.is_exclusive { "[x]" } else { "[ ]" }),
                        Cell::from(if row.enable_implicit_string {
                            "[x]"
                        } else {
                            "[ ]"
                        }),
                        Cell::from(if row.is_actionable { "[x]" } else { "[ ]" }),
                    ])
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
            Table::new(
                rows,
                vec![
                    Constraint::Min(20),
                    Constraint::Length(6),
                    Constraint::Length(7),
                    Constraint::Length(6),
                ],
            )
            .header(
                Row::new(vec![
                    Cell::from("Category"),
                    Cell::from("Excl"),
                    Cell::from("Match"),
                    Cell::from("Todo"),
                ])
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
                let note_text = self
                    .category_manager_details_note_text()
                    .unwrap_or_default();

                let mut parent_name = "(root)".to_string();
                let mut child_count = 0usize;
                if let Some(parent_id) = self
                    .categories
                    .iter()
                    .find(|c| c.id == row.id)
                    .map(|c| {
                        child_count = c.children.len();
                        c.parent
                    })
                    .flatten()
                {
                    if let Some(parent) = self.categories.iter().find(|c| c.id == parent_id) {
                        parent_name = parent.name.clone();
                    }
                }

                let details_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(5),
                        Constraint::Length(6),
                        Constraint::Min(5),
                        Constraint::Length(2),
                    ])
                    .split(details_inner);

                let type_label = match row.value_kind {
                    CategoryValueKind::Tag => "Tag",
                    CategoryValueKind::Numeric => "Numeric",
                };
                frame.render_widget(
                    Paragraph::new(vec![
                        Line::from(format!("Selected: {}", row.name)),
                        Line::from(format!(
                            "Type: {}    Depth: {}    Children: {}",
                            type_label, row.depth, child_count
                        )),
                        Line::from(format!("Parent: {}", parent_name)),
                        Line::from(if row.is_reserved {
                            "Reserved: yes (read-only config)".to_string()
                        } else {
                            "Reserved: no".to_string()
                        }),
                    ])
                    .wrap(Wrap { trim: false }),
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
                frame.render_widget(
                    Paragraph::new(vec![
                        flag_line(
                            details_focus == CategoryManagerDetailsFocus::Exclusive,
                            "Exclusive Children",
                            row.is_exclusive,
                        ),
                        flag_line(
                            details_focus == CategoryManagerDetailsFocus::MatchName,
                            "Match category name",
                            row.enable_implicit_string,
                        ),
                        flag_line(
                            details_focus == CategoryManagerDetailsFocus::Actionable,
                            "Actionable",
                            row.is_actionable,
                        ),
                    ])
                    .block(
                        Block::default()
                            .title("Flags")
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(
                                if details_focus == CategoryManagerDetailsFocus::Note {
                                    pane_idle
                                } else {
                                    Color::LightCyan
                                },
                            )),
                    ),
                    details_chunks[1],
                );

                let note_block_focus = details_focus == CategoryManagerDetailsFocus::Note;
                let note_title = if note_editing {
                    "Note (editing)"
                } else if note_dirty {
                    "Note (unsaved)"
                } else {
                    "Note"
                };
                let note_rect = details_chunks[2];
                let note_inner = Rect {
                    x: note_rect.x.saturating_add(1),
                    y: note_rect.y.saturating_add(1),
                    width: note_rect.width.saturating_sub(2),
                    height: note_rect.height.saturating_sub(2),
                };
                let note_lines: Vec<Line<'_>> = if note_text.is_empty() {
                    vec![Line::from("")]
                } else {
                    note_text.lines().map(Line::from).collect()
                };
                let note_cursor_line = self
                    .category_manager
                    .as_ref()
                    .map(|state| state.details_note.line_col().0)
                    .unwrap_or(0);
                let note_scroll = list_scroll_for_selected_line(note_rect, Some(note_cursor_line));
                frame.render_widget(
                    Paragraph::new(note_lines)
                        .scroll((note_scroll, 0))
                        .wrap(Wrap { trim: false })
                        .block(
                            Block::default()
                                .title(if note_block_focus {
                                    format!("> {note_title}")
                                } else {
                                    note_title.to_string()
                                })
                                .borders(Borders::ALL)
                                .border_style(Style::default().fg(if note_editing {
                                    Color::Yellow
                                } else if note_block_focus {
                                    Color::LightCyan
                                } else {
                                    pane_idle
                                })),
                        ),
                    note_rect,
                );
                Self::render_vertical_scrollbar(
                    frame,
                    note_rect,
                    note_text.lines().count().max(1),
                    note_scroll as usize,
                );

                frame.render_widget(
                    Paragraph::new(if note_editing {
                        "Type to edit  Tab/Esc: save and leave note"
                    } else {
                        "j/k or arrows: focus field  Enter/Space: toggle/edit"
                    }),
                    details_chunks[3],
                );

                if note_editing && note_inner.width > 0 && note_inner.height > 0 {
                    let (line, col) = self
                        .category_manager
                        .as_ref()
                        .map(|state| state.details_note.line_col())
                        .unwrap_or((0, 0));
                    let visible_line = line.saturating_sub(note_scroll as usize);
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
                        .saturating_add(visible_line.min(u16::MAX as usize) as u16)
                        .min(max_y);
                    frame.set_cursor_position((cursor_x, cursor_y));
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

        if show_inline_parent_picker {
            let left_body = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(60), Constraint::Min(4)])
                .split(left[1]);
            if let Some(CategoryInlineAction::ParentPicker {
                options,
                visible_option_indices,
                list_index,
                focus,
                ..
            }) = self.category_manager_inline_action()
            {
                let parent_items: Vec<ListItem<'_>> = if visible_option_indices.is_empty() {
                    vec![ListItem::new(Line::from("(no matching parent options)"))]
                } else {
                    visible_option_indices
                        .iter()
                        .filter_map(|idx| options.get(*idx))
                        .map(|option| ListItem::new(Line::from(option.label.clone())))
                        .collect()
                };
                let mut parent_state = Self::list_state_for(
                    left_body[1],
                    if visible_option_indices.is_empty() {
                        None
                    } else {
                        Some((*list_index).min(visible_option_indices.len().saturating_sub(1)))
                    },
                );
                let item_count = parent_items.len();
                let border_color = if *focus == CategoryParentPickerFocus::List {
                    Color::Cyan
                } else {
                    Color::Blue
                };
                frame.render_stateful_widget(
                    List::new(parent_items)
                        .highlight_symbol("> ")
                        .highlight_style(selected_row_style())
                        .block(
                            Block::default()
                                .title("Parent Picker")
                                .borders(Borders::ALL)
                                .border_style(Style::default().fg(border_color)),
                        ),
                    left_body[1],
                    &mut parent_state,
                );
                Self::render_vertical_scrollbar(
                    frame,
                    left_body[1],
                    item_count,
                    parent_state.offset(),
                );
            }
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
                format!(
                    " DETAILS  View Properties  matches:{} ",
                    state.preview_count
                )
            } else {
                format!(" DETAILS  Section {} ", state.section_index + 1)
            };

            if show_view_details {
                let display_mode_label = match state.draft.board_display_mode {
                    BoardDisplayMode::SingleLine => "single-line",
                    BoardDisplayMode::MultiLine => "multi-line",
                };

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
                    ListItem::new(Line::from(format!("  Name (r): {view_name_text}")))
                        .style(view_name_style),
                );
                items.push(ListItem::new(Line::from("  Criteria:")));

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

                let when_include = if state.draft.criteria.virtual_include.is_empty() {
                    "(none)".to_string()
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

                let when_include_row = items.len();
                let when_include_style = if details_focused
                    && state.region == ViewEditRegion::Unmatched
                    && state.unmatched_field_index == 0
                {
                    selected_line = Some(when_include_row);
                    Style::default().add_modifier(Modifier::REVERSED)
                } else {
                    Style::default()
                };
                items.push(
                    ListItem::new(Line::from(format!("  When include: {when_include}")))
                        .style(when_include_style),
                );

                let when_exclude_row = items.len();
                let when_exclude_style = if details_focused
                    && state.region == ViewEditRegion::Unmatched
                    && state.unmatched_field_index == 1
                {
                    selected_line = Some(when_exclude_row);
                    Style::default().add_modifier(Modifier::REVERSED)
                } else {
                    Style::default()
                };
                items.push(
                    ListItem::new(Line::from(format!("  When exclude: {when_exclude}")))
                        .style(when_exclude_style),
                );

                let display_mode_row = items.len();
                let display_mode_style = if details_focused
                    && state.region == ViewEditRegion::Unmatched
                    && state.unmatched_field_index == 2
                {
                    selected_line = Some(display_mode_row);
                    Style::default().add_modifier(Modifier::REVERSED)
                } else {
                    Style::default()
                };
                items.push(
                    ListItem::new(Line::from(format!("  Display mode: {display_mode_label}")))
                        .style(display_mode_style),
                );

                let unmatched_visible_row = items.len();
                let unmatched_visible_style = if details_focused
                    && state.region == ViewEditRegion::Unmatched
                    && state.unmatched_field_index == 3
                {
                    selected_line = Some(unmatched_visible_row);
                    Style::default().add_modifier(Modifier::REVERSED)
                } else {
                    Style::default()
                };
                items.push(
                    ListItem::new(Line::from(format!(
                        "  Unmatched visible: {}",
                        if state.draft.show_unmatched {
                            "yes"
                        } else {
                            "no"
                        }
                    )))
                    .style(unmatched_visible_style),
                );

                let unmatched_label_row = items.len();
                let unmatched_label_text = if matches!(
                    state.inline_input,
                    Some(ViewEditInlineInput::UnmatchedLabel)
                ) {
                    format!("◀ {}", state.inline_buf.text())
                } else {
                    format!("\"{}\"", state.draft.unmatched_label)
                };
                let unmatched_label_style = if details_focused
                    && state.region == ViewEditRegion::Unmatched
                    && state.unmatched_field_index == 4
                {
                    selected_line = Some(unmatched_label_row);
                    Style::default().add_modifier(Modifier::REVERSED)
                } else {
                    Style::default()
                };
                items.push(
                    ListItem::new(Line::from(format!(
                        "  Unmatched label: {unmatched_label_text}"
                    )))
                    .style(unmatched_label_style),
                );

                items.push(ListItem::new(Line::from(
                    "  View keys: n:add  x:remove  Enter/Space:cycle criterion mode  ]/[:when  m:display  t/l:unmatched",
                )));
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
                let title_style = if details_focused && state.region == ViewEditRegion::Sections {
                    selected_line = Some(state.section_details_field_index.min(7));
                    Style::default().add_modifier(Modifier::REVERSED)
                } else {
                    Style::default()
                };
                items.push(
                    ListItem::new(Line::from(format!("  Title: {title_text}"))).style(title_style),
                );

                let criteria_lines = summarize_query(&section.criteria);
                let criteria_style = if details_focused
                    && state.region == ViewEditRegion::Sections
                    && state.section_details_field_index == 1
                {
                    selected_line = Some(1);
                    Style::default().add_modifier(Modifier::REVERSED)
                } else {
                    Style::default()
                };
                items.push(
                    ListItem::new(Line::from(format!(
                        "  Criteria: {}",
                        if criteria_lines.is_empty() {
                            "(none)".to_string()
                        } else {
                            criteria_lines.join("; ")
                        }
                    )))
                    .style(criteria_style),
                );

                let columns_summary = if section.columns.is_empty() {
                    "(none)".to_string()
                } else {
                    section
                        .columns
                        .iter()
                        .map(|column| {
                            let name = category_names
                                .get(&column.heading)
                                .cloned()
                                .unwrap_or_else(|| "(deleted)".to_string());
                            format!("{name}[w:{}]", column.width)
                        })
                        .collect::<Vec<String>>()
                        .join(", ")
                };
                let style_for_section_field =
                    |field_index: usize, selected_line_ref: &mut Option<usize>| {
                        if details_focused
                            && state.region == ViewEditRegion::Sections
                            && state.section_details_field_index == field_index
                        {
                            *selected_line_ref = Some(field_index);
                            Style::default().add_modifier(Modifier::REVERSED)
                        } else {
                            Style::default()
                        }
                    };
                items.push(
                    ListItem::new(Line::from(format!("  Columns: {columns_summary}")))
                        .style(style_for_section_field(2, &mut selected_line)),
                );
                items.push(
                    ListItem::new(Line::from(format!(
                        "  On insert assign: {}",
                        summarize_category_set(&section.on_insert_assign)
                    )))
                    .style(style_for_section_field(3, &mut selected_line)),
                );
                items.push(
                    ListItem::new(Line::from(format!(
                        "  On remove unassign: {}",
                        summarize_category_set(&section.on_remove_unassign)
                    )))
                    .style(style_for_section_field(4, &mut selected_line)),
                );
                items.push(
                    ListItem::new(Line::from(format!(
                        "  Show children: {}",
                        if section.show_children { "yes" } else { "no" }
                    )))
                    .style(style_for_section_field(5, &mut selected_line)),
                );
                let mode_label = match section.board_display_mode_override {
                    None => "inherit".to_string(),
                    Some(BoardDisplayMode::SingleLine) => "single-line".to_string(),
                    Some(BoardDisplayMode::MultiLine) => "multi-line".to_string(),
                };
                items.push(
                    ListItem::new(Line::from(format!("  Display override: {mode_label}")))
                        .style(style_for_section_field(6, &mut selected_line)),
                );
                items.push(
                    ListItem::new(Line::from(format!(
                        "  Expand in Sections list: {}",
                        if state.section_expanded == Some(state.section_index) {
                            "yes"
                        } else {
                            "no"
                        }
                    )))
                    .style(style_for_section_field(7, &mut selected_line)),
                );
                items.push(ListItem::new(Line::from(
                    "  Tip: Enter/Space edits selected field (J/K or [/] reorder; shortcuts optional)",
                )));
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
                format!(" SECTIONS{dirty_marker}  /{}◀ ", state.sections_filter_buf.text())
            } else if filter_active {
                format!(" SECTIONS{dirty_marker}  /{} ", state.sections_filter_buf.text())
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
                    let is_expanded = state.section_expanded == Some(i);
                    let expand_icon = if is_expanded { "▾" } else { "▸" };

                    let title = if inline_editing_section == Some(i) {
                        format!(
                            "{}  {}. {} ◀ editing",
                            cursor,
                            i + 1,
                            state.inline_buf.text()
                        )
                    } else {
                        format!("{} {} {}. {}", cursor, expand_icon, i + 1, section.title)
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

                    if is_expanded {
                        let criteria_count = section.criteria.criteria.len();
                        let columns_count = section.columns.len();
                        let mode_label = match section.board_display_mode_override {
                            None => "inherit".to_string(),
                            Some(BoardDisplayMode::SingleLine) => "single-line".to_string(),
                            Some(BoardDisplayMode::MultiLine) => "multi-line".to_string(),
                        };
                        items.push(ListItem::new(Line::from(format!(
                            "     criteria:{}  columns:{}  children:{}  display:{}",
                            if criteria_count == 0 {
                                "none".to_string()
                            } else {
                                criteria_count.to_string()
                            },
                            if columns_count == 0 {
                                "none".to_string()
                            } else {
                                columns_count.to_string()
                            },
                            if section.show_children { "yes" } else { "no" },
                            mode_label,
                        ))));
                    }
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

            let reference_date = Local::now().date_naive();
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
                    let filtered_indices: Vec<usize> = if overlay_filter.trim().is_empty() {
                        (0..self.category_rows.len()).collect()
                    } else {
                        let q = overlay_filter.trim().to_ascii_lowercase();
                        self.category_rows
                            .iter()
                            .enumerate()
                            .filter_map(|(i, row)| {
                                if row.name.to_ascii_lowercase().contains(&q) {
                                    Some(i)
                                } else {
                                    None
                                }
                            })
                            .collect()
                    };
                    let selected_filtered_index = filtered_indices
                        .iter()
                        .position(|&i| i == state.picker_index)
                        .unwrap_or(0);
                    let title = if overlay_filter.trim().is_empty() {
                        format!(
                            " Pick categories  {}/{}  (type filter, Space/Enter toggle, Esc done) ",
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
                    let section_expanded = state.section_expanded.unwrap_or(0);
                    let items: Vec<ListItem<'_>> = self
                        .category_rows
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| filtered_indices.contains(i))
                        .map(|(i, row)| {
                            let indent = "  ".repeat(row.depth);
                            let checked = match target {
                                CategoryEditTarget::ViewCriteria => {
                                    state.draft.criteria.mode_for(row.id).is_some()
                                }
                                CategoryEditTarget::SectionCriteria => state
                                    .draft
                                    .sections
                                    .get(section_expanded)
                                    .map(|section| section.criteria.mode_for(row.id).is_some())
                                    .unwrap_or(false),
                                CategoryEditTarget::SectionColumns => state
                                    .draft
                                    .sections
                                    .get(section_expanded)
                                    .map(|section| {
                                        section.columns.iter().any(|col| col.heading == row.id)
                                    })
                                    .unwrap_or(false),
                                CategoryEditTarget::SectionOnInsertAssign => state
                                    .draft
                                    .sections
                                    .get(section_expanded)
                                    .map(|section| section.on_insert_assign.contains(&row.id))
                                    .unwrap_or(false),
                                CategoryEditTarget::SectionOnRemoveUnassign => state
                                    .draft
                                    .sections
                                    .get(section_expanded)
                                    .map(|section| section.on_remove_unassign.contains(&row.id))
                                    .unwrap_or(false),
                            };
                            let label = format!(
                                "{indent}[{}] {}",
                                if checked { "x" } else { " " },
                                row.name
                            );
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
