use crate::*;

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

        let footer = self.render_footer();
        let footer_area = layout[2];
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
        if self.mode == Mode::CategoryConfig {
            let popup_area = category_config_popup_area(frame.area());
            self.render_category_config_editor(frame, popup_area);
            if let Some((x, y)) = self.category_config_cursor_position(popup_area) {
                frame.set_cursor_position((x, y));
            }
        }

        if matches!(self.mode, Mode::ViewPicker | Mode::ViewDeleteConfirm) {
            self.render_view_picker(frame, centered_rect(60, 60, frame.area()));
        }
        if matches!(
            self.mode,
            Mode::ItemAssignPicker | Mode::ItemAssignInput
        ) {
            self.render_item_assign_picker(frame, centered_rect(72, 72, frame.area()));
        }
        if matches!(self.mode, Mode::ViewCreateCategory) {
            self.render_view_category_picker(frame, centered_rect(72, 72, frame.area()));
        }
    }

    pub(crate) fn input_prompt_prefix(&self) -> Option<&'static str> {
        match self.mode {
            Mode::NoteEdit => Some("Note> "),
            Mode::FilterInput => Some("Filter> "),
            Mode::ItemAssignInput => Some("Category> "),
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

        // No cursor while category picker overlay is open (it shows a list, not text)
        if panel.category_picker_open() {
            return None;
        }
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
            InputPanelFocus::CategoriesButton
            | InputPanelFocus::SaveButton
            | InputPanelFocus::CancelButton => None,
        }
    }

    pub(crate) fn category_config_cursor_position(&self, popup_area: Rect) -> Option<(u16, u16)> {
        if self.mode != Mode::CategoryConfig {
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

        let (line, col) = editor.note.line_col();
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
        let active_filters = self
            .section_filters
            .iter()
            .filter(|f| f.is_some())
            .count();
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
        if matches!(
            self.mode,
            Mode::CategoryManager
                | Mode::CategoryReparent
                | Mode::CategoryDelete
                | Mode::CategoryConfig
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
            let slot_columns_owned = match (&slot.context, current_view.as_ref()) {
                (SlotContext::Section { section_index }, Some(view))
                | (SlotContext::GeneratedSection { section_index, .. }, Some(view)) => view
                    .sections
                    .get(*section_index)
                    .map(|section| section.columns.clone())
                    .unwrap_or_default(),
                _ => Vec::new(),
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
                let mut constraints = vec![
                    Constraint::Length(layout.marker.min(u16::MAX as usize) as u16),
                    Constraint::Length(layout.note.min(u16::MAX as usize) as u16),
                    Constraint::Length(item_width.min(u16::MAX as usize) as u16),
                ];
                constraints.extend(
                    layout.columns.iter().map(|column| {
                        Constraint::Length(column.width.min(u16::MAX as usize) as u16)
                    }),
                );
                if synthetic_categories_width > 0 {
                    constraints.push(Constraint::Length(
                        synthetic_categories_width.min(u16::MAX as usize) as u16,
                    ));
                }
                let mut header_cells = vec![
                    Cell::from(String::new()),
                    Cell::from(String::new()),
                    Cell::from(layout.item_label.clone()),
                ];
                header_cells.extend(
                    layout
                        .columns
                        .iter()
                        .map(|column| Cell::from(column.label.clone())),
                );
                if synthetic_categories_width > 0 {
                    header_cells.push(Cell::from("All Categories"));
                }

                let rows: Vec<Row<'_>> = if slot.items.is_empty() {
                    let mut cells = vec![
                        Cell::from(String::new()),
                        Cell::from(String::new()),
                        Cell::from("(no items)"),
                    ];
                    cells.extend(layout.columns.iter().map(|_| Cell::from(String::new())));
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
                            let note_cell = if has_note_text(item.note.as_deref()) {
                                NOTE_MARKER_SYMBOL
                            } else {
                                " "
                            };
                            let mut cells = vec![
                                Cell::from(marker_cell),
                                Cell::from(note_cell),
                                {
                                    let content = truncate_board_cell(
                                        &board_item_label(item),
                                        item_width,
                                    );
                                    let mut cell = Cell::from(content);
                                    if is_selected && self.column_index == 0 {
                                        cell = cell.style(focused_cell_style());
                                    }
                                    cell
                                }
                            ];
                            cells.extend(layout.columns.iter().enumerate().map(|(col_idx, column)| {
                                let value = if self.mode == Mode::CategoryDirectEdit
                                    && is_selected
                                    && self.column_index == col_idx + 1
                                {
                                    self.input.text().to_string()
                                } else {
                                    match column.kind {
                                        ColumnKind::When => item
                                            .when_date
                                            .map(|dt| dt.to_string())
                                            .unwrap_or_else(|| "\u{2013}".to_string()),
                                        ColumnKind::Standard => standard_column_value(
                                            item,
                                            &column.child_ids,
                                            &category_names,
                                        ),
                                    }
                                };
                                let content = truncate_board_cell(&value, column.width);
                                let mut cell = Cell::from(content);
                                if is_selected && self.column_index == col_idx + 1 {
                                    cell = cell.style(focused_cell_style());
                                }
                                cell
                            }));
                            if synthetic_categories_width > 0 {
                                let categories = item_assignment_labels(item, &category_names);
                                let categories_text = if categories.is_empty() {
                                    "-".to_string()
                                } else {
                                    categories.join(", ")
                                };
                                cells.push(Cell::from(truncate_board_cell(
                                    &categories_text,
                                    synthetic_categories_width,
                                )));
                            }
                            Row::new(cells)
                        })
                        .collect()
                };

                let mut state = Self::table_state_for(columns[slot_index], selected_row);
                frame.render_stateful_widget(
                    Table::new(rows, constraints)
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
                    vec![Row::new(vec![
                        Cell::from(String::new()),
                        Cell::from(String::new()),
                        Cell::from(String::new()),
                        Cell::from("(no items)"),
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
                            let note_cell = if has_note_text(item.note.as_deref()) {
                                NOTE_MARKER_SYMBOL
                            } else {
                                " "
                            };
                            let item_text = board_item_label(item);
                            let categories = item_assignment_labels(item, &category_names);
                            let categories_text = if categories.is_empty() {
                                "-".to_string()
                            } else {
                                categories.join(", ")
                            };
                            Row::new(vec![
                                Cell::from(marker_cell),
                                Cell::from(truncate_board_cell(&when, widths.when)),
                                Cell::from(note_cell),
                                Cell::from(truncate_board_cell(&item_text, widths.item)),
                                Cell::from(truncate_board_cell(
                                    &categories_text,
                                    widths.categories,
                                )),
                            ])
                        })
                        .collect()
                };
                let mut state = Self::table_state_for(columns[slot_index], selected_row);
                frame.render_stateful_widget(
                    Table::new(rows, constraints)
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
                        .row_highlight_style(selected_row_style())
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

    pub(crate) fn render_footer(&self) -> Paragraph<'_> {
        let status = self.footer_status_text();
        let hints = self.footer_hint_text();
        let text = ratatui::text::Text::from(vec![
            ratatui::text::Line::from(status),
            ratatui::text::Line::from(ratatui::text::Span::styled(
                hints,
                Style::default().fg(Color::DarkGray),
            )),
        ]);
        Paragraph::new(text).block(Block::default().borders(Borders::ALL))
    }

    fn footer_status_text(&self) -> String {
        match self.mode {
            Mode::NoteEdit => format!("Note> {}", self.input.text()),
            Mode::FilterInput => {
                let target = self.filter_target_section;
                let section_name = self
                    .slots
                    .get(target)
                    .map(|s| s.title.as_str())
                    .unwrap_or("section");
                format!("Filter [{section_name}]> {}", self.input.text())
            }
            Mode::ConfirmDelete => "Delete selected item? y/n".to_string(),
            Mode::ViewDeleteConfirm => "Delete selected view? y/n".to_string(),
            Mode::ViewCreateCategory => {
                "Set include/exclude categories for new view".to_string()
            }
            Mode::CategoryReparent => "Select category parent".to_string(),
            Mode::CategoryDelete => "Delete selected category? y/n".to_string(),
            Mode::CategoryConfig => {
                if let Some(editor) = &self.category_config_editor {
                    format!("Edit category config (focus: {:?})", editor.focus)
                } else {
                    "Edit category config".to_string()
                }
            }
            Mode::ItemAssignPicker => "Select category for selected item".to_string(),
            Mode::ItemAssignInput => format!("Category> {}", self.input.text()),
            Mode::InspectUnassign => "Select assignment".to_string(),
            Mode::InputPanel => {
                if let Some(panel) = &self.input_panel {
                    use input_panel::InputPanelFocus;
                    if panel.category_picker_open() {
                        "Category picker open".to_string()
                    } else {
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
                                InputPanelFocus::CategoriesButton => "Categories",
                                InputPanelFocus::SaveButton => "Save",
                                InputPanelFocus::CancelButton => "Cancel",
                            }
                        )
                    }
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
                "j/k:row  Enter:config  e:exclusive  i:match-name  a:actionable  n/N:create  r:rename  p:reparent  x:delete  Esc:close"
            }
            Mode::CategoryReparent => "j/k:select parent  Enter:reparent  Esc:cancel",
            Mode::CategoryDelete => "y:confirm delete  n:cancel",
            Mode::CategoryConfig => {
                "Tab/Shift+Tab:focus  Space:toggle  S:save  e/i/a:quick toggle  Esc:cancel"
            }
            Mode::ViewPicker => {
                "j/k:select  Enter:switch  N:new  r:rename  x:delete  e:edit  Esc:back"
            }
            Mode::ViewDeleteConfirm => "y:confirm delete  n/Esc:cancel",
            Mode::ViewCreateCategory => {
                "j/k:select  +:include  -:exclude  Space:toggle  Enter:create  Esc:cancel"
            }
            Mode::ViewEdit => {
                if let Some(state) = &self.view_edit_state {
                    match state.region {
                        ViewEditRegion::Criteria => "n:add  x:remove  Space:toggle+/-  ]/[:when-buckets  Tab:region  S:save  Esc:cancel",
                        ViewEditRegion::Sections => "Enter:expand  n:add  e/t:rename  +/-:criteria  c:columns  a:on-insert  r:on-remove  h:children  x:remove  [/]:reorder  Tab:region  S:save  Esc:cancel",
                        ViewEditRegion::Unmatched => "t:toggle-visible  l:label  Tab:region  S:save  Esc:cancel",
                    }
                } else {
                    "Tab:region  S:save  Esc:cancel"
                }
            }
            Mode::ItemAssignPicker => "j/k:select  Space:toggle  n:new  Enter:done  Esc:cancel",
            Mode::ItemAssignInput => "Enter:assign/create  Esc:back",
            Mode::ConfirmDelete => "y:confirm delete  n:cancel",
            Mode::FilterInput => "Enter:apply  Esc:cancel",
            Mode::NoteEdit => "S:save (empty=clear)  Esc:cancel",
            Mode::InspectUnassign => "j/k:select  Enter:apply  Esc:cancel",
            Mode::InputPanel => {
                if self.input_panel.as_ref().map_or(false, |p| p.category_picker_open()) {
                    "j/k:navigate  Space:toggle  Enter/Esc:close picker"
                } else {
                    "S:save  Tab/Shift+Tab:cycle fields  Enter:activate button  Up/Down in note  Esc:cancel"
                }
            }
            _ => "n:add  e:edit  d:done  x:delete  v:views  c:categories  /:filter  q:quit",
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

        let title = match panel.kind {
            InputPanelKind::AddItem => "Add Item",
            InputPanelKind::EditItem => "Edit Item",
            InputPanelKind::NameInput => "Name",
        };
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));
        frame.render_widget(block, area);

        // If category picker overlay is open, render it over the popup.
        if panel.category_picker_open() {
            self.render_input_panel_category_picker(frame, area, panel);
            return;
        }

        let Some(regions) = input_panel_popup_regions(area, panel.kind) else {
            return;
        };

        // Heading
        let heading_text = match panel.kind {
            InputPanelKind::AddItem => "Create new item",
            InputPanelKind::EditItem => "Edit selected item",
            InputPanelKind::NameInput => "Enter name",
        };
        frame.render_widget(Paragraph::new(heading_text), regions.heading);

        // Text field
        let text_marker = if panel.focus == InputPanelFocus::Text { ">" } else { " " };
        let text_label = if panel.kind == InputPanelKind::NameInput {
            "Name"
        } else {
            "Text"
        };
        frame.render_widget(
            Paragraph::new(format!("{text_marker} {text_label}> {}", panel.text.text())),
            regions.text,
        );

        // Note and Categories (not shown for NameInput)
        if let Some(note_rect) = regions.note {
            let note_lines: Vec<Line<'_>> = if panel.note.is_empty() {
                vec![Line::from("")]
            } else {
                panel.note.text().lines().map(Line::from).collect()
            };
            let note_focused = panel.focus == InputPanelFocus::Note;
            let note_border_color = if note_focused { Color::Cyan } else { Color::Blue };
            let note_title = if note_focused { "Note (> editable)" } else { "Note (editable)" };
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

        if let Some(categories_rect) = regions.categories {
            let cat_focused = panel.focus == InputPanelFocus::CategoriesButton;
            let cat_marker = if cat_focused { "[> Categories <]" } else { "[Categories]" };
            let cat_names = self.category_names_for_ids(&panel.categories);
            let cat_display = if cat_names.is_empty() {
                format!("{cat_marker}  (none)")
            } else {
                format!("{cat_marker}  {}", cat_names.join(", "))
            };
            frame.render_widget(Paragraph::new(cat_display), categories_rect);
        }

        if let Some(preview_rect) = regions.preview {
            if !panel.preview_context.is_empty() {
                frame.render_widget(
                    Paragraph::new(panel.preview_context.as_str())
                        .style(Style::default().fg(Color::DarkGray)),
                    preview_rect,
                );
            }
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
            Paragraph::new("S:save  Tab/Shift+Tab:cycle  Enter:activate  Up/Down:note  Esc:cancel"),
            regions.help,
        );
    }

    /// Render the category picker overlay within the InputPanel popup area.
    fn render_input_panel_category_picker(
        &self,
        frame: &mut ratatui::Frame<'_>,
        area: Rect,
        panel: &input_panel::InputPanel,
    ) {
        let picker_index = panel.picker_index().unwrap_or(0);
        let category_names = self.category_names_for_ids(&panel.categories);
        let selected_label = if category_names.is_empty() {
            "(none selected)".to_string()
        } else {
            category_names.join(", ")
        };

        // Use inner area of the popup for the list
        let inner = Rect {
            x: area.x.saturating_add(1),
            y: area.y.saturating_add(1),
            width: area.width.saturating_sub(2),
            height: area.height.saturating_sub(2),
        };
        if inner.height < 2 {
            return;
        }
        let chunks = ratatui::layout::Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(1)])
            .split(inner);

        frame.render_widget(
            Paragraph::new(format!("Selected: {selected_label}"))
                .style(Style::default().fg(Color::Yellow)),
            chunks[0],
        );

        let items: Vec<ListItem<'_>> = if self.category_rows.is_empty() {
            vec![ListItem::new("(no categories available)")]
        } else {
            self.category_rows
                .iter()
                .map(|row| {
                    let check = if panel.categories.contains(&row.id) {
                        "■ "
                    } else {
                        "□ "
                    };
                    let indent = "  ".repeat(row.depth);
                    let reserved = if row.is_reserved { " [reserved]" } else { "" };
                    ListItem::new(format!("{check}{indent}{}{reserved}", row.name))
                })
                .collect()
        };
        let item_count = items.len();
        let mut state = Self::list_state_for(chunks[1], Some(picker_index));
        frame.render_stateful_widget(
            List::new(items)
                .highlight_symbol("> ")
                .highlight_style(selected_row_style()),
            chunks[1],
            &mut state,
        );
        Self::render_vertical_scrollbar(frame, chunks[1], item_count, state.offset());
    }

    /// Returns the display names for a set of category IDs.
    fn category_names_for_ids(
        &self,
        ids: &std::collections::HashSet<agenda_core::model::CategoryId>,
    ) -> Vec<String> {
        ids.iter()
            .filter_map(|id| {
                self.category_rows
                    .iter()
                    .find(|row| row.id == *id)
                    .map(|row| row.name.clone())
            })
            .collect()
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

    pub(crate) fn render_view_category_picker(&self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        frame.render_widget(Clear, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(1), Constraint::Min(1)])
            .split(area);
        frame.render_widget(
            Paragraph::new("Choose criteria for new view (+ include, - exclude, Enter create)"),
            chunks[0],
        );

        let items: Vec<ListItem<'_>> = if self.category_rows.is_empty() {
            vec![ListItem::new(Line::from("(no categories available)"))]
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
                    let check = if self.view_create_include_selection.contains(&row.id) {
                        "[+]"
                    } else if self.view_create_exclude_selection.contains(&row.id) {
                        "[-]"
                    } else {
                        "[ ]"
                    };
                    let category_name = with_note_marker(row.name.clone(), row.has_note);
                    let text = format!(
                        "{check} {}{}{}",
                        "  ".repeat(row.depth),
                        category_name,
                        suffix
                    );
                    ListItem::new(Line::from(text))
                })
                .collect()
        };

        let title = match self.mode {
            Mode::ViewCreateCategory => "Create View Criteria",
            _ => "View Criteria",
        };
        let mut state = Self::list_state_for(
            chunks[1],
            if self.category_rows.is_empty() {
                None
            } else {
                Some(self.view_category_index)
            },
        );
        let item_count = items.len();
        frame.render_stateful_widget(
            List::new(items)
                .highlight_symbol("> ")
                .highlight_style(selected_row_style())
                .block(
                    Block::default()
                        .title(title)
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Magenta)),
                ),
            chunks[1],
            &mut state,
        );
        Self::render_vertical_scrollbar(frame, chunks[1], item_count, state.offset());
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
                        "[x]"
                    } else {
                        "[ ]"
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
        frame.render_widget(
            Paragraph::new("Categories are global. Enter opens config popup (checkboxes + note)."),
            layout[0],
        );

        let table_area = if self.mode == Mode::CategoryReparent {
            let body = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(60), Constraint::Min(4)])
                .split(layout[1]);
            body[0]
        } else {
            layout[1]
        };

        let title_suffix = String::new();

        let rows: Vec<Row<'_>> = if self.category_rows.is_empty() {
            vec![Row::new(vec![
                Cell::from("(no categories)"),
                Cell::from(String::new()),
                Cell::from(String::new()),
                Cell::from(String::new()),
            ])]
        } else {
            self.category_rows
                .iter()
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
        let mut state = Self::table_state_for(
            table_area,
            if self.category_rows.is_empty() {
                None
            } else {
                Some(self.category_index)
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
                    .title(format!("Category Manager{title_suffix}"))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Green)),
            ),
            table_area,
            &mut state,
        );
        Self::render_vertical_scrollbar(
            frame,
            table_area,
            self.category_rows.len(),
            state.offset(),
        );

        if self.mode == Mode::CategoryReparent {
            let body = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(60), Constraint::Min(4)])
                .split(layout[1]);
            let reparent_items: Vec<ListItem<'_>> = if self.category_reparent_options.is_empty() {
                vec![ListItem::new(Line::from("(no valid parent options)"))]
            } else {
                self.category_reparent_options
                    .iter()
                    .map(|option| ListItem::new(Line::from(option.label.clone())))
                    .collect()
            };
            let mut reparent_state = Self::list_state_for(
                body[1],
                if self.category_reparent_options.is_empty() {
                    None
                } else {
                    Some(
                        self.category_reparent_index
                            .min(self.category_reparent_options.len().saturating_sub(1)),
                    )
                },
            );
            let item_count = reparent_items.len();
            frame.render_stateful_widget(
                List::new(reparent_items)
                    .highlight_symbol("> ")
                    .highlight_style(selected_row_style())
                    .block(
                        Block::default()
                            .title("Select new parent")
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(Color::Green)),
                    ),
                body[1],
                &mut reparent_state,
            );
            Self::render_vertical_scrollbar(frame, body[1], item_count, reparent_state.offset());
        }
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
            editor.note.text().lines().map(Line::from).collect()
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
        let note_cursor_line = editor.note.line_col().0;
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
        Self::render_vertical_scrollbar(
            frame,
            regions.note,
            editor.note.text().lines().count().max(1),
            note_scroll as usize,
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

    // -------------------------------------------------------------------------
    // ViewEdit (unified view editor)
    // -------------------------------------------------------------------------

    pub(crate) fn render_view_edit_screen(&self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        let Some(state) = &self.view_edit_state else {
            return;
        };

        // Split into 3 vertical regions: Criteria / Sections / Unmatched
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(25),
                Constraint::Percentage(50),
                Constraint::Percentage(25),
            ])
            .split(area);

        let focused_border = Color::Cyan;
        let inactive_border = Color::Blue;

        let border_for = |region: ViewEditRegion| -> Color {
            if state.region == region {
                focused_border
            } else {
                inactive_border
            }
        };

        let category_names = category_name_map(&self.categories);

        // ── Criteria region ──────────────────────────────────────────────────
        {
            let block = Block::default()
                .title(format!(
                    " VIEW CRITERIA  matches:{} ",
                    state.preview_count
                ))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_for(ViewEditRegion::Criteria)));

            let mut items: Vec<ListItem<'_>> = state
                .criteria_rows
                .iter()
                .enumerate()
                .map(|(i, row)| {
                    let name = category_names
                        .get(&row.category_id)
                        .cloned()
                        .unwrap_or_else(|| "(deleted)".to_string());
                    let sign = match row.sign {
                        ViewCriteriaSign::Include => "+",
                        ViewCriteriaSign::Exclude => "-",
                    };
                    let label = format!("  {sign}{name}");
                    let style =
                        if i == state.criteria_index && state.region == ViewEditRegion::Criteria {
                            Style::default().add_modifier(Modifier::REVERSED)
                        } else {
                            Style::default()
                        };
                    ListItem::new(Line::from(label)).style(style)
                })
                .collect();

            // Virtual include/exclude summary lines
            if !state.draft.criteria.virtual_include.is_empty() {
                let buckets: Vec<&str> = when_bucket_options()
                    .iter()
                    .filter(|b| state.draft.criteria.virtual_include.contains(*b))
                    .map(|b| when_bucket_label(*b))
                    .collect();
                items.push(ListItem::new(Line::from(format!(
                    "  When: {}",
                    buckets.join(", ")
                ))));
            }
            if !state.draft.criteria.virtual_exclude.is_empty() {
                let buckets: Vec<&str> = when_bucket_options()
                    .iter()
                    .filter(|b| state.draft.criteria.virtual_exclude.contains(*b))
                    .map(|b| when_bucket_label(*b))
                    .collect();
                items.push(ListItem::new(Line::from(format!(
                    "  When (excl): {}",
                    buckets.join(", ")
                ))));
            }

            if items.is_empty() {
                items.push(ListItem::new(Line::from(
                    "  (no criteria — matches all items)",
                )));
            }

            let list = List::new(items).block(block);
            frame.render_widget(list, chunks[0]);
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

            let block = Block::default()
                .title(" SECTIONS ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_for(ViewEditRegion::Sections)));

            let mut items: Vec<ListItem<'_>> = Vec::new();
            let mut selected_line: Option<usize> = None;
            if state.draft.sections.is_empty() {
                items.push(ListItem::new(Line::from("  (no sections — n:add)")));
            } else {
                for (i, section) in state.draft.sections.iter().enumerate() {
                    if i == state.section_index {
                        selected_line = Some(items.len());
                    }
                    let cursor =
                        if i == state.section_index && state.region == ViewEditRegion::Sections {
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

                    let style =
                        if i == state.section_index && state.region == ViewEditRegion::Sections {
                            Style::default().add_modifier(Modifier::REVERSED)
                        } else {
                            Style::default()
                        };
                    items.push(ListItem::new(Line::from(title)).style(style));

                    // Only render detail lines for the expanded section
                    if is_expanded {
                        let mut inc: Vec<String> = section
                            .criteria
                            .include
                            .iter()
                            .map(|id| {
                                category_names
                                    .get(id)
                                    .cloned()
                                    .unwrap_or_else(|| id.to_string())
                            })
                            .collect();
                        inc.sort_by_key(|name| name.to_ascii_lowercase());
                        let mut exc: Vec<String> = section
                            .criteria
                            .exclude
                            .iter()
                            .map(|id| {
                                category_names
                                    .get(id)
                                    .cloned()
                                    .unwrap_or_else(|| id.to_string())
                            })
                            .collect();
                        exc.sort_by_key(|name| name.to_ascii_lowercase());
                        if !inc.is_empty() {
                            items.push(ListItem::new(Line::from(format!(
                                "     include: {}",
                                inc.join(", ")
                            ))));
                        }
                        if !exc.is_empty() {
                            items.push(ListItem::new(Line::from(format!(
                                "     exclude: {}",
                                exc.join(", ")
                            ))));
                        }
                        let section_columns: Vec<String> = if section.columns.is_empty() {
                            vec!["(none)".to_string()]
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
                                .collect()
                        };
                        items.push(ListItem::new(Line::from(format!(
                            "     columns: {}",
                            section_columns.join(", ")
                        ))));
                        items.push(ListItem::new(Line::from(format!(
                            "     children:{}  (e/t:title  +/-:criteria  c:columns  a:on-insert  r:on-remove  h:children)",
                            if section.show_children { "yes" } else { "no" }
                        ))));
                    }
                }
            }

            let content_len = items.len();
            let mut list_state = Self::list_state_for(chunks[1], selected_line);
            frame.render_stateful_widget(List::new(items).block(block), chunks[1], &mut list_state);
            Self::render_vertical_scrollbar(frame, chunks[1], content_len, list_state.offset());
        }

        // ── Unmatched region ─────────────────────────────────────────────────
        {
            let editing_label = matches!(
                state.inline_input,
                Some(ViewEditInlineInput::UnmatchedLabel)
            );

            let block = Block::default()
                .title(" UNMATCHED ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_for(ViewEditRegion::Unmatched)));

            let label_text = if editing_label {
                format!("  ◀ {}", state.inline_buf.text())
            } else {
                format!("  \"{}\"", state.draft.unmatched_label)
            };
            let text = format!(
                "  Visible: {}    Label: {}",
                if state.draft.show_unmatched {
                    "yes"
                } else {
                    "no"
                },
                label_text
            );
            let style = if state.region == ViewEditRegion::Unmatched {
                Style::default().add_modifier(Modifier::REVERSED)
            } else {
                Style::default()
            };
            let para = Paragraph::new(Line::from(text).style(style)).block(block);
            frame.render_widget(para, chunks[2]);
        }

        // ── Picker overlay ───────────────────────────────────────────────────
        if let Some(overlay) = &state.overlay {
            let overlay_area = {
                let x = area.x + area.width * 6 / 10;
                let w = area.width * 4 / 10;
                Rect::new(x, area.y, w, area.height)
            };
            frame.render_widget(Clear, overlay_area);
            match overlay {
                ViewEditOverlay::CategoryPicker { target } => {
                    let title = " Pick categories (Space/Enter toggle, Esc done) ";
                    let section_expanded = state.section_expanded.unwrap_or(0);
                    let items: Vec<ListItem<'_>> = self
                        .category_rows
                        .iter()
                        .enumerate()
                        .map(|(i, row)| {
                            let indent = "  ".repeat(row.depth);
                            let checked = match target {
                                CategoryEditTarget::ViewInclude => {
                                    state.draft.criteria.include.contains(&row.id)
                                }
                                CategoryEditTarget::SectionCriteriaInclude => state
                                    .draft
                                    .sections
                                    .get(section_expanded)
                                    .map(|section| section.criteria.include.contains(&row.id))
                                    .unwrap_or(false),
                                CategoryEditTarget::SectionCriteriaExclude => state
                                    .draft
                                    .sections
                                    .get(section_expanded)
                                    .map(|section| section.criteria.exclude.contains(&row.id))
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
                    let mut list_state =
                        Self::list_state_for(overlay_area, Some(state.picker_index));
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
    }
}
