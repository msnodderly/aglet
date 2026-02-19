use crate::*;

impl App {
    pub(crate) fn draw(&self, frame: &mut ratatui::Frame<'_>) {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(1),
                Constraint::Length(3),
            ])
            .split(frame.area());

        let header = self.render_header();
        frame.render_widget(header, layout[0]);

        self.render_main(frame, layout[1]);

        let footer = self.render_footer();
        let footer_area = layout[2];
        frame.render_widget(footer, footer_area);
        if let Some((x, y)) = self.input_cursor_position(footer_area) {
            frame.set_cursor_position((x, y));
        }
        if self.mode == Mode::ItemEditInput {
            let popup_area = item_edit_popup_area(frame.area());
            self.render_item_edit_popup(frame, popup_area);
            if let Some((x, y)) = self.item_edit_cursor_position(popup_area) {
                frame.set_cursor_position((x, y));
            }
        }
        if self.mode == Mode::CategoryConfigEditor {
            let popup_area = category_config_popup_area(frame.area());
            self.render_category_config_editor(frame, popup_area);
            if let Some((x, y)) = self.category_config_cursor_position(popup_area) {
                frame.set_cursor_position((x, y));
            }
        }

        if matches!(
            self.mode,
            Mode::ViewPicker
                | Mode::ViewCreateNameInput
                | Mode::ViewRenameInput
                | Mode::ViewDeleteConfirm
        ) {
            self.render_view_picker(frame, centered_rect(60, 60, frame.area()));
        }
        if matches!(
            self.mode,
            Mode::ItemAssignCategoryPicker | Mode::ItemAssignCategoryInput
        ) {
            self.render_item_assign_picker(frame, centered_rect(72, 72, frame.area()));
        }
        if matches!(self.mode, Mode::ViewCreateCategoryPicker) {
            self.render_view_category_picker(frame, centered_rect(72, 72, frame.area()));
        }
        if self.mode == Mode::ViewEditor {
            self.render_view_editor(frame, centered_rect(72, 72, frame.area()));
        }
        if self.mode == Mode::ViewEditorCategoryPicker {
            self.render_view_editor_category_picker(frame, centered_rect(72, 72, frame.area()));
        }
        if self.mode == Mode::ViewManagerCategoryPicker {
            self.render_view_manager_category_picker(frame, centered_rect(72, 72, frame.area()));
        }
        if self.mode == Mode::ViewEditorBucketPicker {
            self.render_view_editor_bucket_picker(frame, centered_rect(60, 60, frame.area()));
        }
        if self.mode == Mode::ViewSectionEditor {
            self.render_view_section_editor(frame, centered_rect(72, 72, frame.area()));
        }
        if matches!(
            self.mode,
            Mode::ViewSectionDetail | Mode::ViewSectionTitleInput
        ) {
            self.render_view_section_detail(frame, centered_rect(72, 72, frame.area()));
        }
        if matches!(
            self.mode,
            Mode::ViewUnmatchedSettings | Mode::ViewUnmatchedLabelInput
        ) {
            self.render_view_unmatched_settings(frame, centered_rect(60, 40, frame.area()));
        }
    }

    pub(crate) fn input_prompt_prefix(&self) -> Option<&'static str> {
        match self.mode {
            Mode::AddInput => Some("Add> "),
            Mode::NoteEditInput => Some("Note> "),
            Mode::FilterInput => Some("Filter> "),
            Mode::ViewCreateNameInput => Some("View create> "),
            Mode::ViewRenameInput => Some("View rename> "),
            Mode::ViewSectionTitleInput => Some("Section title> "),
            Mode::ViewUnmatchedLabelInput => Some("Unmatched label> "),
            Mode::CategoryCreateInput => Some("Category create> "),
            Mode::CategoryRenameInput => Some("Category rename> "),
            Mode::ItemAssignCategoryInput => Some("Category> "),
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

    pub(crate) fn item_edit_cursor_position(&self, popup_area: Rect) -> Option<(u16, u16)> {
        if self.mode != Mode::ItemEditInput {
            return None;
        }
        if popup_area.width < 3 || popup_area.height < 3 {
            return None;
        }
        let regions = item_edit_popup_regions(popup_area)?;
        match self.item_edit_focus {
            ItemEditFocus::Text => {
                let prefix_len = "  Text> ".chars().count().min(u16::MAX as usize) as u16;
                let input_chars = self.clamped_input_cursor().min(u16::MAX as usize) as u16;
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
            ItemEditFocus::Note => {
                if regions.note_inner.width == 0 || regions.note_inner.height == 0 {
                    return None;
                }
                let (line, col) = note_cursor_line_col(
                    &self.item_edit_note,
                    self.clamped_item_edit_note_cursor(),
                );
                let scroll = list_scroll_for_selected_line(regions.note, Some(line)) as usize;
                let visible_line = line.saturating_sub(scroll);
                let max_x = regions
                    .note_inner
                    .x
                    .saturating_add(regions.note_inner.width.saturating_sub(1));
                let max_y = regions
                    .note_inner
                    .y
                    .saturating_add(regions.note_inner.height.saturating_sub(1));
                let cursor_x = regions
                    .note_inner
                    .x
                    .saturating_add(col.min(u16::MAX as usize) as u16)
                    .min(max_x);
                let cursor_y = regions
                    .note_inner
                    .y
                    .saturating_add(visible_line.min(u16::MAX as usize) as u16)
                    .min(max_y);
                Some((cursor_x, cursor_y))
            }
            ItemEditFocus::CategoriesButton
            | ItemEditFocus::SaveButton
            | ItemEditFocus::CancelButton => None,
        }
    }

    pub(crate) fn category_config_cursor_position(&self, popup_area: Rect) -> Option<(u16, u16)> {
        if self.mode != Mode::CategoryConfigEditor {
            return None;
        }
        let Some(editor) = &self.category_config_editor else {
            return None;
        };
        if popup_area.width < 3 || popup_area.height < 3 {
            return None;
        }
        let regions = category_config_popup_regions(popup_area)?;
        if editor.focus != CategoryConfigFocus::Note {
            return None;
        }
        if regions.note_inner.width == 0 || regions.note_inner.height == 0 {
            return None;
        }

        let cursor = self.category_config_note_cursor().unwrap_or(0);
        let (line, col) = note_cursor_line_col(&editor.note, cursor);
        let scroll = list_scroll_for_selected_line(regions.note, Some(line)) as usize;
        let visible_line = line.saturating_sub(scroll);
        let max_x = regions
            .note_inner
            .x
            .saturating_add(regions.note_inner.width.saturating_sub(1));
        let max_y = regions
            .note_inner
            .y
            .saturating_add(regions.note_inner.height.saturating_sub(1));
        let cursor_x = regions
            .note_inner
            .x
            .saturating_add(col.min(u16::MAX as usize) as u16)
            .min(max_x);
        let cursor_y = regions
            .note_inner
            .y
            .saturating_add(visible_line.min(u16::MAX as usize) as u16)
            .min(max_y);
        Some((cursor_x, cursor_y))
    }

    pub(crate) fn render_header(&self) -> Paragraph<'_> {
        let view_name = self
            .current_view()
            .map(|view| view.name.as_str())
            .unwrap_or("(none)");
        let mode = format!("{:?}", self.mode);
        let filter = self
            .filter
            .as_ref()
            .map(|value| format!(" filter:{value}"))
            .unwrap_or_default();

        Paragraph::new(Line::from(vec![
            Span::styled(
                "Agenda Reborn",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!("  view:{view_name}  mode:{mode}{filter}")),
        ]))
    }

    pub(crate) fn render_main(&self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        if self.mode == Mode::ViewManagerScreen {
            self.render_view_manager_screen(frame, area);
            return;
        }
        if matches!(
            self.mode,
            Mode::CategoryManager
                | Mode::CategoryCreateInput
                | Mode::CategoryRenameInput
                | Mode::CategoryReparentPicker
                | Mode::CategoryDeleteConfirm
                | Mode::CategoryConfigEditor
        ) {
            self.render_category_manager(frame, area);
            return;
        }
        if self.show_preview {
            let split = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
                .split(area);
            self.render_board_columns(frame, split[0]);
            frame.render_widget(self.render_preview_panel(), split[1]);
        } else {
            self.render_board_columns(frame, area);
        }
    }

    pub(crate) fn render_view_manager_screen(&self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        let panes = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(30),
                Constraint::Percentage(40),
                Constraint::Percentage(30),
            ])
            .split(area);

        let selected_view = self.views.get(self.picker_index);
        let views_lines: Vec<Line<'_>> = if self.views.is_empty() {
            vec![Line::from("(no views)")]
        } else {
            self.views
                .iter()
                .enumerate()
                .map(|(index, view)| {
                    let text = format!(
                        "{}{}",
                        if index == self.picker_index {
                            "> "
                        } else {
                            "  "
                        },
                        view.name
                    );
                    if index == self.picker_index {
                        Line::styled(text, selected_row_style())
                    } else {
                        Line::from(text)
                    }
                })
                .collect()
        };
        let views_border = if self.view_manager_pane == ViewManagerPane::Views {
            Color::Cyan
        } else {
            Color::Blue
        };
        frame.render_widget(
            Paragraph::new(views_lines).block(
                Block::default()
                    .title("Views")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(views_border)),
            ),
            panes[0],
        );

        let criteria_tab_label =
            if self.view_manager_definition_sub_tab == DefinitionSubTab::Criteria {
                "[Criteria]"
            } else {
                " Criteria "
            };
        let columns_tab_label = if self.view_manager_definition_sub_tab == DefinitionSubTab::Columns
        {
            "[Columns]"
        } else {
            " Columns "
        };
        let mut definition_lines = vec![
            Line::from(format!(
                "  {criteria_tab_label}  {columns_tab_label}    t:toggle"
            )),
            Line::from(""),
        ];
        if let Some(view) = selected_view {
            if self.view_manager_definition_sub_tab == DefinitionSubTab::Criteria {
                let validation_errors = self.view_manager_representability_errors();
                definition_lines.push(Line::from(format!("View: {}", view.name)));
                definition_lines.push(Line::from(format!(
                    "Rows: {}{}",
                    self.view_manager_rows.len(),
                    if self.view_manager_dirty {
                        "  *unsaved*"
                    } else {
                        ""
                    }
                )));
                definition_lines.push(Line::from(format!(
                    "Preview matching: {}",
                    self.view_manager_preview_count
                )));
                if validation_errors.is_empty() {
                    definition_lines.push(Line::from("Validation: ok"));
                } else {
                    definition_lines.push(Line::styled(
                        format!("Validation errors: {}", validation_errors.len()),
                        Style::default().fg(Color::Red),
                    ));
                    definition_lines.push(Line::styled(
                        format!("  - {}", validation_errors[0]),
                        Style::default().fg(Color::Red),
                    ));
                }
                definition_lines.push(Line::from(""));
                if self.view_manager_rows.is_empty() {
                    definition_lines.push(Line::from("(no criteria rows)"));
                } else {
                    definition_lines.extend(self.view_manager_rows.iter().enumerate().map(
                        |(index, row)| {
                            let marker = if index == self.view_manager_definition_index {
                                "> "
                            } else {
                                "  "
                            };
                            let join = if index == 0 {
                                "  "
                            } else if row.join_is_or {
                                "OR"
                            } else {
                                "AND"
                            };
                            let sign = match row.sign {
                                ViewCriteriaSign::Include => '+',
                                ViewCriteriaSign::Exclude => '-',
                            };
                            let category_name = self.view_manager_category_label(row.category_id);
                            let text = format!(
                                "{marker}{join} {}{} {}",
                                "  ".repeat(row.depth),
                                sign,
                                category_name
                            );
                            if index == self.view_manager_definition_index {
                                Line::styled(text, selected_row_style())
                            } else {
                                Line::from(text)
                            }
                        },
                    ));
                }
            } else {
                // Columns sub-tab
                let category_names = category_name_map(&self.categories);
                definition_lines.push(Line::from(format!("View: {}", view.name)));
                definition_lines.push(Line::from(format!(
                    "Columns: {}{}",
                    view.columns.len(),
                    if self.view_manager_dirty {
                        "  *unsaved*"
                    } else {
                        ""
                    }
                )));
                definition_lines.push(Line::from(""));
                if view.columns.is_empty() {
                    definition_lines.push(Line::from("(no columns — legacy rendering)"));
                } else {
                    for (index, col) in view.columns.iter().enumerate() {
                        let marker = if index == self.view_manager_column_index {
                            "> "
                        } else {
                            "  "
                        };
                        let label = category_names
                            .get(&col.heading)
                            .cloned()
                            .unwrap_or_else(|| "(deleted)".to_string());
                        let kind_tag = match col.kind {
                            ColumnKind::When => " [When]",
                            ColumnKind::Standard => "",
                        };
                        let text = format!("{marker}{label}{kind_tag}  w:{}", col.width);
                        if index == self.view_manager_column_index {
                            definition_lines.push(Line::styled(text, selected_row_style()));
                        } else {
                            definition_lines.push(Line::from(text));
                        }
                    }
                }
                definition_lines.push(Line::from(""));
                if self.view_manager_column_width_input {
                    definition_lines.push(Line::from(format!("Width> {}", self.input)));
                } else {
                    definition_lines
                        .push(Line::from("N:add  x:del  [/]:move  w:width  Enter:heading"));
                }
            }
        } else {
            definition_lines.push(Line::from("(no selected view)"));
        }
        let definition_border = if self.view_manager_pane == ViewManagerPane::Definition {
            Color::Cyan
        } else {
            Color::Blue
        };
        frame.render_widget(
            Paragraph::new(definition_lines).block(
                Block::default()
                    .title("Definition")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(definition_border)),
            ),
            panes[1],
        );

        let mut section_lines = vec![Line::from("Sections"), Line::from("")];
        if let Some(view) = selected_view {
            if view.sections.is_empty() {
                section_lines.push(Line::from("(no sections configured)"));
            } else {
                section_lines.extend(view.sections.iter().enumerate().map(|(index, section)| {
                    let text = format!(
                        "{}{}",
                        if index == self.view_manager_section_index {
                            "> "
                        } else {
                            "  "
                        },
                        section.title
                    );
                    if index == self.view_manager_section_index {
                        Line::styled(text, selected_row_style())
                    } else {
                        Line::from(text)
                    }
                }));
            }
            section_lines.push(Line::from(""));
            section_lines.push(Line::from(format!(
                "Unmatched: {}",
                if view.show_unmatched { "on" } else { "off" }
            )));
            section_lines.push(Line::from(format!(
                "Label: {}",
                if view.unmatched_label.trim().is_empty() {
                    "Unassigned".to_string()
                } else {
                    view.unmatched_label.clone()
                }
            )));
        } else {
            section_lines.push(Line::from("(no selected view)"));
        }
        let section_border = if self.view_manager_pane == ViewManagerPane::Sections {
            Color::Cyan
        } else {
            Color::Blue
        };
        frame.render_widget(
            Paragraph::new(section_lines).block(
                Block::default()
                    .title("Sections")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(section_border)),
            ),
            panes[2],
        );
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
        let view_columns = self
            .current_view()
            .map(|v| v.columns.as_slice())
            .unwrap_or(&[]);
        let use_dynamic = !view_columns.is_empty();
        let view_item_label = self
            .current_view()
            .and_then(|v| v.item_column_label.clone())
            .unwrap_or_default();
        // Clone view_columns so we don't hold a borrow on self
        let view_columns_owned: Vec<Column> = view_columns.to_vec();
        for (slot_index, slot) in self.slots.iter().enumerate() {
            let is_selected_slot = slot_index == self.slot_index;
            let inner_width = columns[slot_index].width.saturating_sub(2);
            let mut lines: Vec<Line<'_>>;
            if use_dynamic {
                let layout = compute_board_layout(
                    &view_columns_owned,
                    &self.categories,
                    &category_names,
                    &view_item_label,
                    inner_width,
                );
                lines = vec![Line::from(board_dynamic_header(&layout))];
                if slot.items.is_empty() {
                    lines.push(Line::from("(no items)"));
                } else {
                    lines.extend(slot.items.iter().enumerate().map(|(item_index, item)| {
                        let is_selected = is_selected_slot && item_index == self.item_index;
                        let row_text =
                            board_dynamic_row(is_selected, item, &layout, &category_names);
                        if is_selected {
                            Line::styled(row_text, selected_row_style())
                        } else {
                            Line::from(row_text)
                        }
                    }));
                }
            } else {
                let widths = board_column_widths(inner_width);
                lines = vec![Line::from(board_annotation_header(widths))];
                if slot.items.is_empty() {
                    lines.push(Line::from("(no items)"));
                } else {
                    lines.extend(slot.items.iter().enumerate().map(|(item_index, item)| {
                        let when = item
                            .when_date
                            .map(|dt| dt.to_string())
                            .unwrap_or_else(|| "-".to_string());
                        let item_text = board_item_label(item);
                        let categories = item_assignment_labels(item, &category_names);
                        let categories_text = if categories.is_empty() {
                            "-".to_string()
                        } else {
                            categories.join(", ")
                        };
                        let has_note = has_note_text(item.note.as_deref());
                        let is_selected = is_selected_slot && item_index == self.item_index;
                        let row_text = board_item_row(
                            is_selected,
                            &when,
                            &item_text,
                            &categories_text,
                            has_note,
                            widths,
                        );
                        if is_selected {
                            Line::styled(row_text, selected_row_style())
                        } else {
                            Line::from(row_text)
                        }
                    }));
                }
            }
            let title = format!("{} ({})", slot.title, slot.items.len());
            let border_color = if is_selected_slot {
                Color::Cyan
            } else {
                Color::Blue
            };
            let selected_line = if is_selected_slot && !slot.items.is_empty() {
                Some(1 + self.item_index.min(slot.items.len().saturating_sub(1)))
            } else {
                None
            };
            let scroll = list_scroll_for_selected_line(columns[slot_index], selected_line);
            frame.render_widget(
                Paragraph::new(lines)
                    .block(
                        Block::default()
                            .title(title)
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(border_color)),
                    )
                    .scroll((scroll, 0)),
                columns[slot_index],
            );
        }
    }

    pub(crate) fn render_preview_provenance_panel(&self) -> Paragraph<'_> {
        let mut lines = vec![
            Line::from("Provenance"),
            Line::from("f focus | j/k or J/K scroll | o summary | u unassign"),
        ];
        if let Some(item) = self.selected_item() {
            let rows = self.inspect_assignment_rows_for_item(item);
            if rows.is_empty() {
                lines.push(Line::from("(no assignments)"));
            } else {
                let is_picker_mode = self.mode == Mode::InspectUnassignPicker;
                for (index, row) in rows.iter().enumerate() {
                    let marker = if is_picker_mode && index == self.inspect_assignment_index {
                        "> "
                    } else {
                        "  "
                    };
                    lines.push(Line::from(format!(
                        "{marker}{} | source={} | origin={}",
                        row.category_name, row.source_label, row.origin_label
                    )));
                }
            }
        } else {
            lines.push(Line::from("(no selected item)"));
        }

        Paragraph::new(lines)
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
            )
            .scroll((
                self.preview_provenance_scroll.min(u16::MAX as usize) as u16,
                0,
            ))
            .wrap(Wrap { trim: false })
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

    pub(crate) fn render_preview_panel(&self) -> Paragraph<'_> {
        match self.preview_mode {
            PreviewMode::Summary => self.render_preview_summary_panel(),
            PreviewMode::Provenance => self.render_preview_provenance_panel(),
        }
    }

    pub(crate) fn render_footer(&self) -> Paragraph<'_> {
        let prompt = match self.mode {
            Mode::AddInput => format!("Add> {}", self.input),
            Mode::NoteEditInput => format!("Note> {}", self.input),
            Mode::FilterInput => format!("Filter> {}", self.input),
            Mode::ConfirmDelete => "Delete selected item? y/n".to_string(),
            Mode::ViewCreateNameInput => format!("View create> {}", self.input),
            Mode::ViewRenameInput => format!("View rename> {}", self.input),
            Mode::ViewDeleteConfirm => "Delete selected view? y/n".to_string(),
            Mode::ViewCreateCategoryPicker => {
                "Set include/exclude categories for new view".to_string()
            }
            Mode::ViewManagerCategoryPicker => "Pick category for criteria row".to_string(),
            Mode::ViewManagerScreen => format!(
                "View manager pane:{:?} preview:{}{}",
                self.view_manager_pane,
                self.view_manager_preview_count,
                if self.view_manager_dirty {
                    "  *unsaved*"
                } else {
                    ""
                }
            ),
            Mode::ViewSectionTitleInput => format!("Section title> {}", self.input),
            Mode::ViewUnmatchedLabelInput => format!("Unmatched label> {}", self.input),
            Mode::CategoryCreateInput => format!("Category create> {}", self.input),
            Mode::CategoryRenameInput => format!("Category rename> {}", self.input),
            Mode::CategoryReparentPicker => "Select category parent".to_string(),
            Mode::CategoryDeleteConfirm => "Delete selected category? y/n".to_string(),
            Mode::CategoryConfigEditor => {
                if let Some(editor) = &self.category_config_editor {
                    format!("Edit category config (focus: {:?})", editor.focus)
                } else {
                    "Edit category config".to_string()
                }
            }
            Mode::ItemAssignCategoryPicker => "Select category for selected item".to_string(),
            Mode::ItemAssignCategoryInput => format!("Category> {}", self.input),
            Mode::InspectUnassignPicker => "Select assignment".to_string(),
            Mode::ItemEditInput => format!(
                "Edit item fields in popup (focus: {})",
                match self.item_edit_focus {
                    ItemEditFocus::Text => "Text",
                    ItemEditFocus::Note => "Note",
                    ItemEditFocus::CategoriesButton => "Categories",
                    ItemEditFocus::SaveButton => "Save",
                    ItemEditFocus::CancelButton => "Cancel",
                }
            ),
            _ => self.status.clone(),
        };
        let footer_title = match self.mode {
            Mode::CategoryManager => {
                "j/k:row  Enter:config popup  e:exclusive  i:match-name  a:actionable  n/N:create  r:rename  p:reparent  x:delete  Esc/F9:close"
            }
            Mode::CategoryCreateInput => "Type category name, Enter:create, Esc:cancel",
            Mode::CategoryRenameInput => "Type new category name, Enter:rename, Esc:cancel",
            Mode::CategoryReparentPicker => "j/k:select parent  Enter:reparent  Esc:cancel",
            Mode::CategoryDeleteConfirm => "y:confirm delete  n:cancel",
            Mode::CategoryConfigEditor => {
                "Tab/Shift+Tab:focus  h/l:checkbox focus  Space:toggle  Enter:save (except note)  e/i/a:quick toggle  Esc:cancel"
            }
            Mode::ViewPicker => {
                "j/k:select  Enter:switch  N:create  r:rename  x:delete  e:edit view  V:view manager  Esc:cancel"
            }
            Mode::ViewManagerScreen => {
                "Tab/Shift+Tab:pane  j/k:row  Enter:activate  N:add  x:remove  [/] reorder  a/o:join  (/):depth  c:pick-category  u:unmatched  s:save  q/Esc:back"
            }
            Mode::ViewManagerCategoryPicker => "j/k:select  Enter/Space:choose  Esc:cancel",
            Mode::ViewCreateNameInput => "Type view name, Enter:next, Esc:cancel",
            Mode::ViewRenameInput => "Type new view name, Enter:rename, Esc:cancel",
            Mode::ViewDeleteConfirm => "y:confirm delete  n/Esc:cancel",
            Mode::ViewCreateCategoryPicker => {
                "j/k:select category  +:include  -:exclude  Space:+include  Enter:create view  Esc:cancel"
            }
            Mode::ViewEditor => "j/k:select  o/right:open  +|-|[|]:quick open  s/u:sections/unmatched  Enter:save  Esc:cancel",
            Mode::ViewEditorCategoryPicker => "j/k:select category  Space:toggle  Enter/Esc:back",
            Mode::ViewEditorBucketPicker => "j/k:select bucket  Space:toggle  Enter/Esc:back",
            Mode::ViewSectionEditor => "j/k:select  N:add  x:remove  [/] reorder  Enter:edit  Esc:back",
            Mode::ViewSectionDetail => "t:title  +/-:criteria  [/] virtual  a:on-insert  r:on-remove  h:children  Esc:back",
            Mode::ViewSectionTitleInput => "Type section title, Enter:save, Esc:cancel",
            Mode::ViewUnmatchedSettings => "t:toggle unmatched  l:label  Esc:back",
            Mode::ViewUnmatchedLabelInput => "Type unmatched label, Enter:save, Esc:cancel",
            Mode::ItemAssignCategoryPicker => "j/k:select category  Space:toggle add/remove  n or /:type name assign/create  Enter:done  Esc:cancel",
            Mode::ItemAssignCategoryInput => "Type category name, Enter:assign/create, Esc:back",
            Mode::ItemEditInput => {
                "Edit popup: Tab/Shift+Tab navigate  Enter activate  Up/Down note  Esc cancel  F3 categories"
            }
            Mode::NoteEditInput => "Edit selected note, Enter:save (empty clears), Esc:cancel",
            Mode::InspectUnassignPicker => "j/k:select assignment  Enter:apply  Esc:cancel",
            _ => {
                "n:add  Enter/e:edit-item  a/u:item-categories  m:note  [/]:filter  v/F8:views  c/F9:categories  g:all-items  ,/.:view  p:preview  o:preview-mode  Tab/Shift+Tab:section  f:board/preview focus  []:move  r:remove  d/D:done-toggle  x:delete  J/K:preview-scroll  q:quit"
            }
        };

        Paragraph::new(prompt).block(Block::default().title(footer_title).borders(Borders::ALL))
    }

    pub(crate) fn render_item_edit_popup(&self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        frame.render_widget(Clear, area);
        let block = Block::default()
            .title("Edit Item")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));
        frame.render_widget(block, area);
        let Some(regions) = item_edit_popup_regions(area) else {
            return;
        };
        let text_marker = if self.item_edit_focus == ItemEditFocus::Text {
            ">"
        } else {
            " "
        };
        let categories_button = if self.item_edit_focus == ItemEditFocus::CategoriesButton {
            "[> Categories <]"
        } else {
            "[Categories]"
        };
        let save_button = if self.item_edit_focus == ItemEditFocus::SaveButton {
            "[> Save <]"
        } else {
            "[Save]"
        };
        let cancel_button = if self.item_edit_focus == ItemEditFocus::CancelButton {
            "[> Cancel <]"
        } else {
            "[Cancel]"
        };

        frame.render_widget(Paragraph::new("Edit selected item"), regions.heading);
        frame.render_widget(
            Paragraph::new(format!("{text_marker} Text> {}", self.input)),
            regions.text,
        );

        let note_lines: Vec<Line<'_>> = if self.item_edit_note.is_empty() {
            vec![Line::from("")]
        } else {
            self.item_edit_note.lines().map(Line::from).collect()
        };
        let note_border_color = if self.item_edit_focus == ItemEditFocus::Note {
            Color::Cyan
        } else {
            Color::Blue
        };
        let note_title = if self.item_edit_focus == ItemEditFocus::Note {
            "Note (> editable)"
        } else {
            "Note (editable)"
        };
        let note_cursor_line =
            note_cursor_line_col(&self.item_edit_note, self.clamped_item_edit_note_cursor()).0;
        let note_scroll = list_scroll_for_selected_line(regions.note, Some(note_cursor_line));
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
            regions.note,
        );
        frame.render_widget(
            Paragraph::new(format!(
                "  {}  {}  {}",
                categories_button, save_button, cancel_button
            )),
            regions.buttons,
        );
        frame.render_widget(
            Paragraph::new(
                "Tab/Shift+Tab navigate  Enter activate  Up/Down note  Esc cancel  F3 categories",
            ),
            regions.help,
        );
    }

    pub(crate) fn render_view_picker(&self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        frame.render_widget(Clear, area);

        let lines: Vec<Line<'_>> = if self.views.is_empty() {
            vec![Line::from("(no views configured)")]
        } else {
            self.views
                .iter()
                .enumerate()
                .map(|(index, view)| {
                    let is_selected = index == self.picker_index;
                    let marker = if is_selected { "> " } else { "  " };
                    let text = format!("{marker}{}", view.name);
                    if is_selected {
                        Line::styled(text, selected_row_style())
                    } else {
                        Line::from(text)
                    }
                })
                .collect()
        };
        let scroll = list_scroll_for_selected_line(
            area,
            if self.views.is_empty() {
                None
            } else {
                Some(self.picker_index)
            },
        );

        frame.render_widget(
            Paragraph::new(lines).scroll((scroll, 0)).block(
                Block::default()
                    .title("View Palette")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Magenta)),
            ),
            area,
        );
    }

    pub(crate) fn render_view_category_picker(&self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        frame.render_widget(Clear, area);

        let mut lines = vec![Line::from(
            "Choose criteria for new view (+ include, - exclude, Enter create)",
        )];
        if self.category_rows.is_empty() {
            lines.push(Line::from("(no categories available)"));
        } else {
            for (index, row) in self.category_rows.iter().enumerate() {
                let marker = if index == self.view_category_index {
                    "> "
                } else {
                    "  "
                };
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
                let check = if self.view_create_include_selection.contains(&row.id) {
                    "[+]"
                } else if self.view_create_exclude_selection.contains(&row.id) {
                    "[-]"
                } else {
                    "[ ]"
                };
                let category_name = with_note_marker(row.name.clone(), row.has_note);
                let text = format!(
                    "{marker}{check} {}{}{}",
                    "  ".repeat(row.depth),
                    category_name,
                    suffix
                );
                if index == self.view_category_index {
                    lines.push(Line::styled(text, selected_row_style()));
                } else {
                    lines.push(Line::from(text));
                }
            }
        }

        let title = match self.mode {
            Mode::ViewCreateCategoryPicker => "Create View Criteria",
            _ => "View Criteria",
        };
        let scroll = list_scroll_for_selected_line(
            area,
            if self.category_rows.is_empty() {
                None
            } else {
                Some(1 + self.view_category_index)
            },
        );
        frame.render_widget(
            Paragraph::new(lines).scroll((scroll, 0)).block(
                Block::default()
                    .title(title)
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Magenta)),
            ),
            area,
        );
    }

    pub(crate) fn render_view_editor(&self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        frame.render_widget(Clear, area);
        let Some(editor) = &self.view_editor else {
            return;
        };
        let mut lines = vec![
            Line::from(format!("Editing view: {}", editor.base_view_name)),
            Line::from(format!("Preview matches: {}", editor.preview_count)),
            Line::from(""),
        ];
        let actions = [
            format!(
                "Include categories ({})",
                editor.draft.criteria.include.len()
            ),
            format!(
                "Exclude categories ({})",
                editor.draft.criteria.exclude.len()
            ),
            format!(
                "Virtual include buckets ({})",
                editor.draft.criteria.virtual_include.len()
            ),
            format!(
                "Virtual exclude buckets ({})",
                editor.draft.criteria.virtual_exclude.len()
            ),
            format!("Sections ({})", editor.draft.sections.len()),
            format!(
                "Unmatched settings (enabled={} label={})",
                editor.draft.show_unmatched, editor.draft.unmatched_label
            ),
        ];
        for (index, action) in actions.into_iter().enumerate() {
            let marker = if index == editor.action_index {
                "> "
            } else {
                "  "
            };
            let text = format!("{marker}{action}");
            if index == editor.action_index {
                lines.push(Line::styled(text, selected_row_style()));
            } else {
                lines.push(Line::from(text));
            }
        }
        lines.extend([
            Line::from(""),
            Line::from("Use j/k then o/right to open selected editor."),
            Line::from("Quick keys: + include  - exclude  ] v-include  [ v-exclude"),
            Line::from("            s sections  u unmatched  Enter save  Esc cancel"),
        ]);
        if editor.draft.sections.is_empty() {
            lines.push(Line::from("No sections configured yet."));
        }

        let scroll = list_scroll_for_selected_line(area, Some(3 + editor.action_index));
        frame.render_widget(
            Paragraph::new(lines)
                .scroll((scroll, 0))
                .block(
                    Block::default()
                        .title("View Editor")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Magenta)),
                )
                .wrap(Wrap { trim: false }),
            area,
        );
    }

    pub(crate) fn render_view_editor_category_picker(
        &self,
        frame: &mut ratatui::Frame<'_>,
        area: Rect,
    ) {
        frame.render_widget(Clear, area);
        let Some(editor) = &self.view_editor else {
            return;
        };
        let Some(target) = self.view_editor_category_target else {
            return;
        };
        let mut lines = vec![Line::from(format!(
            "Target: {}",
            category_target_label(target)
        ))];
        for (index, row) in self.category_rows.iter().enumerate() {
            let marker = if index == editor.category_index {
                "> "
            } else {
                "  "
            };
            let selected =
                category_target_contains(&editor.draft, editor.section_index, target, row.id);
            let check = if selected { "[x]" } else { "[ ]" };
            let category_name = with_note_marker(row.name.clone(), row.has_note);
            let text = format!(
                "{marker}{check} {}{}",
                "  ".repeat(row.depth),
                category_name
            );
            if index == editor.category_index {
                lines.push(Line::styled(text, selected_row_style()));
            } else {
                lines.push(Line::from(text));
            }
        }
        let scroll = list_scroll_for_selected_line(
            area,
            if self.category_rows.is_empty() {
                None
            } else {
                Some(1 + editor.category_index)
            },
        );
        frame.render_widget(
            Paragraph::new(lines)
                .scroll((scroll, 0))
                .block(
                    Block::default()
                        .title("Category Picker")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Magenta)),
                )
                .wrap(Wrap { trim: false }),
            area,
        );
    }

    pub(crate) fn render_view_manager_category_picker(
        &self,
        frame: &mut ratatui::Frame<'_>,
        area: Rect,
    ) {
        frame.render_widget(Clear, area);
        let lines: Vec<Line<'_>> = if self.category_rows.is_empty() {
            vec![Line::from("(no categories)")]
        } else {
            self.category_rows
                .iter()
                .enumerate()
                .map(|(index, row)| {
                    let marker = if index == self.view_category_index {
                        "> "
                    } else {
                        "  "
                    };
                    let selected_flag = self
                        .view_manager_category_row_index
                        .and_then(|row_index| self.view_manager_rows.get(row_index))
                        .map(|criteria_row| criteria_row.category_id == row.id)
                        .unwrap_or(false);
                    let check = if selected_flag { "[x]" } else { "[ ]" };
                    let reserved = if row.is_reserved { " [reserved]" } else { "" };
                    let category_name = with_note_marker(row.name.clone(), row.has_note);
                    let text = format!(
                        "{marker}{}{} {}{}",
                        "  ".repeat(row.depth),
                        check,
                        category_name,
                        reserved
                    );
                    if index == self.view_category_index {
                        Line::styled(text, selected_row_style())
                    } else {
                        Line::from(text)
                    }
                })
                .collect()
        };
        let scroll = list_scroll_for_selected_line(
            area,
            if self.category_rows.is_empty() {
                None
            } else {
                Some(self.view_category_index)
            },
        );

        frame.render_widget(
            Paragraph::new(lines).scroll((scroll, 0)).block(
                Block::default()
                    .title("View Manager Category Picker")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Magenta)),
            ),
            area,
        );
    }

    pub(crate) fn render_view_editor_bucket_picker(
        &self,
        frame: &mut ratatui::Frame<'_>,
        area: Rect,
    ) {
        frame.render_widget(Clear, area);
        let Some(editor) = &self.view_editor else {
            return;
        };
        let Some(target) = self.view_editor_bucket_target else {
            return;
        };
        let mut lines = vec![Line::from(format!(
            "Target: {}",
            bucket_target_label(target)
        ))];
        for (index, bucket) in when_bucket_options().iter().enumerate() {
            let marker = if index == editor.bucket_index {
                "> "
            } else {
                "  "
            };
            let selected =
                bucket_target_contains(&editor.draft, editor.section_index, target, *bucket);
            let check = if selected { "[x]" } else { "[ ]" };
            let text = format!("{marker}{check} {}", when_bucket_label(*bucket));
            if index == editor.bucket_index {
                lines.push(Line::styled(text, selected_row_style()));
            } else {
                lines.push(Line::from(text));
            }
        }
        let scroll = list_scroll_for_selected_line(area, Some(1 + editor.bucket_index));
        frame.render_widget(
            Paragraph::new(lines)
                .scroll((scroll, 0))
                .block(
                    Block::default()
                        .title("Bucket Picker")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Magenta)),
                )
                .wrap(Wrap { trim: false }),
            area,
        );
    }

    pub(crate) fn render_view_section_editor(&self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        frame.render_widget(Clear, area);
        let Some(editor) = &self.view_editor else {
            return;
        };
        let mut lines = vec![Line::from("Sections in current view draft:")];
        if editor.draft.sections.is_empty() {
            lines.push(Line::from("(no sections)"));
        } else {
            for (index, section) in editor.draft.sections.iter().enumerate() {
                let marker = if index == editor.section_index {
                    "> "
                } else {
                    "  "
                };
                let text = format!(
                    "{marker}{} (include={}, exclude={}, v+={}, v-={}, show_children={})",
                    section.title,
                    section.criteria.include.len(),
                    section.criteria.exclude.len(),
                    section.criteria.virtual_include.len(),
                    section.criteria.virtual_exclude.len(),
                    section.show_children
                );
                if index == editor.section_index {
                    lines.push(Line::styled(text, selected_row_style()));
                } else {
                    lines.push(Line::from(text));
                }
            }
        }
        lines.push(Line::from(""));
        lines.push(Line::from(
            "N add  x remove  [/] reorder  Enter edit  Esc back",
        ));

        let scroll = list_scroll_for_selected_line(
            area,
            if editor.draft.sections.is_empty() {
                None
            } else {
                Some(1 + editor.section_index)
            },
        );
        frame.render_widget(
            Paragraph::new(lines)
                .scroll((scroll, 0))
                .block(
                    Block::default()
                        .title("Section Editor")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Magenta)),
                )
                .wrap(Wrap { trim: false }),
            area,
        );
    }

    pub(crate) fn render_view_section_detail(&self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        frame.render_widget(Clear, area);
        let Some(editor) = &self.view_editor else {
            return;
        };
        let Some(section) = editor.draft.sections.get(editor.section_index) else {
            return;
        };

        let lines = vec![
            Line::from(format!("Section: {}", section.title)),
            Line::from(format!(
                "criteria include={} exclude={} v_include={} v_exclude={}",
                section.criteria.include.len(),
                section.criteria.exclude.len(),
                section.criteria.virtual_include.len(),
                section.criteria.virtual_exclude.len()
            )),
            Line::from(format!(
                "on_insert_assign={} on_remove_unassign={}",
                section.on_insert_assign.len(),
                section.on_remove_unassign.len()
            )),
            Line::from(format!("show_children={}", section.show_children)),
            Line::from(""),
            Line::from("t title  + include  - exclude  ] v-include  [ v-exclude"),
            Line::from("a on-insert  r on-remove  h toggle show_children  Esc back"),
        ];

        frame.render_widget(
            Paragraph::new(lines)
                .block(
                    Block::default()
                        .title("Section Detail")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Magenta)),
                )
                .wrap(Wrap { trim: false }),
            area,
        );
    }

    pub(crate) fn render_view_unmatched_settings(
        &self,
        frame: &mut ratatui::Frame<'_>,
        area: Rect,
    ) {
        frame.render_widget(Clear, area);
        let Some(editor) = &self.view_editor else {
            return;
        };
        let lines = vec![
            Line::from(format!("show_unmatched: {}", editor.draft.show_unmatched)),
            Line::from(format!("unmatched_label: {}", editor.draft.unmatched_label)),
            Line::from(""),
            Line::from("t toggle visibility  l edit label  Esc back"),
        ];
        frame.render_widget(
            Paragraph::new(lines)
                .block(
                    Block::default()
                        .title("Unmatched Settings")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Magenta)),
                )
                .wrap(Wrap { trim: false }),
            area,
        );
    }

    pub(crate) fn render_item_assign_picker(&self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        frame.render_widget(Clear, area);

        let mut lines = vec![Line::from(
            "Edit selected item categories (Space toggles, n or / enters category text)",
        )];
        if self.category_rows.is_empty() {
            lines.push(Line::from("(no categories)"));
        } else {
            for (index, row) in self.category_rows.iter().enumerate() {
                let marker = if index == self.item_assign_category_index {
                    "> "
                } else {
                    "  "
                };
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
                    "[x]"
                } else {
                    "[ ]"
                };
                let category_name = with_note_marker(row.name.clone(), row.has_note);
                let text = format!(
                    "{marker}{assigned} {}{}{}",
                    "  ".repeat(row.depth),
                    category_name,
                    suffix
                );
                if index == self.item_assign_category_index {
                    lines.push(Line::styled(text, selected_row_style()));
                } else {
                    lines.push(Line::from(text));
                }
            }
        }

        let scroll = list_scroll_for_selected_line(
            area,
            if self.category_rows.is_empty() {
                None
            } else {
                Some(1 + self.item_assign_category_index)
            },
        );
        frame.render_widget(
            Paragraph::new(lines).scroll((scroll, 0)).block(
                Block::default()
                    .title("Assign Item")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan)),
            ),
            area,
        );
    }

    pub(crate) fn render_category_manager(&self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        let mut lines = vec![Line::from(
            "Categories are global. Enter opens config popup (checkboxes + note).",
        )];
        let inner_width = area.width.saturating_sub(2) as usize;
        let marker_width = 2usize;
        let excl_width = 4usize;
        let noimpl_width = 7usize;
        let todo_width = 6usize;
        let separator_width = BOARD_COLUMN_SEPARATOR.len() * 3;
        let name_width = inner_width.saturating_sub(
            marker_width + excl_width + noimpl_width + todo_width + separator_width,
        );
        lines.push(Line::from(format!(
            "{}{}{}{}{}{}{}{}",
            " ".repeat(marker_width),
            fit_board_cell("Category", name_width),
            BOARD_COLUMN_SEPARATOR,
            fit_board_cell("Excl", excl_width),
            BOARD_COLUMN_SEPARATOR,
            fit_board_cell("Match", noimpl_width),
            BOARD_COLUMN_SEPARATOR,
            fit_board_cell("Todo", todo_width),
        )));
        let mut selected_line = None;
        if self.category_rows.is_empty() {
            lines.push(Line::from("(no categories)"));
        } else {
            for (index, row) in self.category_rows.iter().enumerate() {
                let is_selected = index == self.category_index;
                let marker = if is_selected { "> " } else { "  " };
                if is_selected {
                    selected_line = Some(2 + index);
                }
                let indent = "  ".repeat(row.depth);
                let mut label = format!("{indent}{}", row.name);
                label = with_note_marker(label, row.has_note);
                if row.is_reserved {
                    label.push_str(" [reserved]");
                }
                let excl = if row.is_exclusive { "[x]" } else { "[ ]" };
                let noimp = if row.enable_implicit_string {
                    "[x]"
                } else {
                    "[ ]"
                };
                let todo = if row.is_actionable { "[x]" } else { "[ ]" };
                let text = format!(
                    "{marker}{}{}{}{}{}{}{}",
                    fit_board_cell(&label, name_width),
                    BOARD_COLUMN_SEPARATOR,
                    fit_board_cell(excl, excl_width),
                    BOARD_COLUMN_SEPARATOR,
                    fit_board_cell(noimp, noimpl_width),
                    BOARD_COLUMN_SEPARATOR,
                    fit_board_cell(todo, todo_width),
                );
                if is_selected {
                    lines.push(Line::styled(text, selected_row_style()));
                } else {
                    lines.push(Line::from(text));
                }
            }
        }

        if self.mode == Mode::CategoryCreateInput {
            let parent = self
                .create_parent_name()
                .unwrap_or_else(|| "(top level / no parent)".to_string());
            lines.push(Line::from(""));
            lines.push(Line::from(format!("New category location: under {parent}")));
        }
        if self.mode == Mode::CategoryRenameInput {
            let target = self
                .selected_category_row()
                .map(|row| row.name.clone())
                .unwrap_or_else(|| "(none)".to_string());
            lines.push(Line::from(""));
            lines.push(Line::from(format!("Rename target: {target}")));
        }
        if self.mode == Mode::CategoryReparentPicker {
            lines.push(Line::from(""));
            lines.push(Line::from("Select new parent:"));
            if self.category_reparent_options.is_empty() {
                lines.push(Line::from("(no valid parent options)"));
            } else {
                let options_start = lines.len();
                for (index, option) in self.category_reparent_options.iter().enumerate() {
                    let marker = if index == self.category_reparent_index {
                        "> "
                    } else {
                        "  "
                    };
                    if index == self.category_reparent_index {
                        selected_line = Some(options_start + index);
                    }
                    lines.push(Line::from(format!("{marker}{}", option.label)));
                }
            }
        }
        let scroll = list_scroll_for_selected_line(area, selected_line);

        frame.render_widget(
            Paragraph::new(lines).scroll((scroll, 0)).block(
                Block::default()
                    .title("Category Manager")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Green)),
            ),
            area,
        );
    }

    pub(crate) fn render_category_config_editor(&self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        frame.render_widget(Clear, area);
        let block = Block::default()
            .title("Category Config")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Green));
        frame.render_widget(block, area);

        let Some(editor) = &self.category_config_editor else {
            return;
        };
        let Some(regions) = category_config_popup_regions(area) else {
            return;
        };
        frame.render_widget(
            Paragraph::new(format!("Editing: {}", editor.category_name)),
            regions.heading,
        );

        let excl_text = if editor.is_exclusive {
            "[x] Exclusive Children"
        } else {
            "[ ] Exclusive Children"
        };
        let noimp_text = if editor.enable_implicit_string {
            "[x] Match category name"
        } else {
            "[ ] Match category name"
        };
        let actionable_text = if editor.is_actionable {
            "[x] Actionable"
        } else {
            "[ ] Actionable"
        };
        let excl_style = if editor.focus == CategoryConfigFocus::Exclusive {
            focused_cell_style()
        } else {
            Style::default()
        };
        let noimp_style = if editor.focus == CategoryConfigFocus::NoImplicit {
            focused_cell_style()
        } else {
            Style::default()
        };
        let actionable_style = if editor.focus == CategoryConfigFocus::Actionable {
            focused_cell_style()
        } else {
            Style::default()
        };
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(format!(" {} ", excl_text), excl_style),
                Span::raw("  "),
                Span::styled(format!(" {} ", noimp_text), noimp_style),
                Span::raw("  "),
                Span::styled(format!(" {} ", actionable_text), actionable_style),
            ])),
            regions.toggles,
        );

        let note_lines: Vec<Line<'_>> = if editor.note.is_empty() {
            vec![Line::from("")]
        } else {
            editor.note.lines().map(Line::from).collect()
        };
        let note_border_color = if editor.focus == CategoryConfigFocus::Note {
            Color::Cyan
        } else {
            Color::Blue
        };
        let note_title = if editor.focus == CategoryConfigFocus::Note {
            "Note (> editable)"
        } else {
            "Note (editable)"
        };
        let note_cursor = self.category_config_note_cursor().unwrap_or(0);
        let note_cursor_line = note_cursor_line_col(&editor.note, note_cursor).0;
        let note_scroll = list_scroll_for_selected_line(regions.note, Some(note_cursor_line));
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
            regions.note,
        );

        let save_button = if editor.focus == CategoryConfigFocus::SaveButton {
            "[> Save <]"
        } else {
            "[Save]"
        };
        let cancel_button = if editor.focus == CategoryConfigFocus::CancelButton {
            "[> Cancel <]"
        } else {
            "[Cancel]"
        };
        frame.render_widget(
            Paragraph::new(format!("  {}  {}", save_button, cancel_button)),
            regions.buttons,
        );
        frame.render_widget(
            Paragraph::new(
                "Tab focus  h/l checkbox focus  Space toggle  Enter saves (except note)  e/i/a quick toggle  Esc cancel",
            ),
            regions.help,
        );
    }
}
