use crate::*;

#[derive(Clone, Copy, PartialEq, Eq)]
enum ViewSettingsColumn {
    Left,
    Right,
}

impl App {
    pub(crate) fn view_edit_showing_view_details(state: &ViewEditState) -> bool {
        state.region != ViewEditRegion::Sections
            || state.sections_view_row_selected
            || (state.draft.datebook_config.is_none()
                && state.draft.sections.get(state.section_index).is_none())
    }

    fn view_edit_section_filter_query(state: &ViewEditState) -> Option<String> {
        let q = state.sections_filter_buf.trimmed();
        if q.is_empty() {
            None
        } else {
            Some(q.to_ascii_lowercase())
        }
    }

    pub(crate) fn view_edit_visible_section_indices(state: &ViewEditState) -> Vec<usize> {
        let Some(filter) = Self::view_edit_section_filter_query(state) else {
            return (0..state.draft.sections.len()).collect();
        };
        state
            .draft
            .sections
            .iter()
            .enumerate()
            .filter_map(|(i, section)| {
                let title = section.title.to_ascii_lowercase();
                if title.contains(&filter) {
                    Some(i)
                } else {
                    None
                }
            })
            .collect()
    }

    pub(crate) fn view_edit_filter_is_active(state: &ViewEditState) -> bool {
        Self::view_edit_section_filter_query(state).is_some()
    }

    fn view_edit_overlay_category_filter_query(state: &ViewEditState) -> Option<String> {
        let q = state.overlay_filter_buf.trimmed();
        if q.is_empty() {
            None
        } else {
            Some(q.to_ascii_lowercase())
        }
    }

    pub(crate) fn view_edit_filtered_category_row_indices(
        &self,
        state: &ViewEditState,
    ) -> Vec<usize> {
        let filter = Self::view_edit_overlay_category_filter_query(state);
        let is_column_picker = matches!(
            state.overlay,
            Some(ViewEditOverlay::CategoryPicker {
                target: CategoryEditTarget::SectionColumns,
            })
        );
        let cat_by_id: HashMap<CategoryId, &Category> = if is_column_picker {
            self.categories.iter().map(|c| (c.id, c)).collect()
        } else {
            HashMap::new()
        };
        self.category_rows
            .iter()
            .enumerate()
            .filter_map(|(i, row)| {
                if let Some(ref q) = filter {
                    if !row.name.to_ascii_lowercase().contains(q) {
                        return None;
                    }
                }
                if is_column_picker {
                    if let Some(cat) = cat_by_id.get(&row.id) {
                        if !is_valid_column_heading(cat) {
                            return None;
                        }
                    } else {
                        return None;
                    }
                }
                Some(i)
            })
            .collect()
    }

    pub(crate) fn clear_view_edit_section_filter(&mut self) {
        if let Some(state) = &mut self.view_edit_state {
            state.sections_filter_buf.clear();
            if matches!(
                state.inline_input,
                Some(ViewEditInlineInput::SectionsFilter)
            ) {
                state.inline_input = None;
            }
        }
        self.normalize_view_edit_sections_selection_for_filter();
        self.status = "Section filter cleared".to_string();
    }

    pub(crate) fn normalize_view_edit_sections_selection_for_filter(&mut self) {
        let Some(state) = &mut self.view_edit_state else {
            return;
        };
        let visible = Self::view_edit_visible_section_indices(state);
        if visible.is_empty() {
            state.sections_view_row_selected = true;
            if state.region == ViewEditRegion::Sections {
                state.section_details_field_index = 0;
            }
            return;
        }

        if state.sections_view_row_selected {
            return;
        }
        if !visible.contains(&state.section_index) {
            state.section_index = visible[0];
            if state.region == ViewEditRegion::Sections {
                state.section_details_field_index = 0;
            }
        }
    }

    pub(crate) const DATEBOOK_FIELD_COUNT: usize = 4; // Period, Interval, Anchor, DateSource

    /// Check if the current flat focus index is the Name row.
    pub(crate) fn view_details_on_name_row(state: &ViewEditState) -> bool {
        state.name_focused
    }

    pub(crate) fn view_details_on_view_type_row(state: &ViewEditState) -> bool {
        state.view_type_focused
    }

    fn view_settings_focus_position(state: &ViewEditState) -> (ViewSettingsColumn, usize) {
        if state.name_focused {
            return (ViewSettingsColumn::Left, 0);
        }
        if state.view_type_focused {
            return (ViewSettingsColumn::Left, 1);
        }
        match state.region {
            ViewEditRegion::Criteria => (ViewSettingsColumn::Left, 2),
            ViewEditRegion::Unmatched => match state.unmatched_field_index {
                0 => (ViewSettingsColumn::Left, 3),
                1 => (ViewSettingsColumn::Left, 4),
                2 => (ViewSettingsColumn::Right, 0),
                3 => (ViewSettingsColumn::Right, 1),
                4 => (ViewSettingsColumn::Right, 2),
                8 => (ViewSettingsColumn::Right, 3),
                5 => (ViewSettingsColumn::Right, 4),
                6 => (ViewSettingsColumn::Right, 5),
                7 => (ViewSettingsColumn::Right, 6),
                _ => (ViewSettingsColumn::Left, 2),
            },
            _ => (ViewSettingsColumn::Left, 2),
        }
    }

    fn set_view_settings_focus_position(&mut self, column: ViewSettingsColumn, row: usize) {
        if let Some(state) = &mut self.view_edit_state {
            state.name_focused = false;
            state.view_type_focused = false;
            state.sections_view_row_selected = false;
            match column {
                ViewSettingsColumn::Left => match row.min(4) {
                    0 => {
                        state.region = ViewEditRegion::Criteria;
                        state.name_focused = true;
                        state.criteria_index = 0;
                    }
                    1 => {
                        state.region = ViewEditRegion::Criteria;
                        state.view_type_focused = true;
                        state.criteria_index = 0;
                    }
                    2 => {
                        state.region = ViewEditRegion::Criteria;
                        state.criteria_index = state
                            .criteria_index
                            .min(state.draft.criteria.criteria.len().saturating_sub(1));
                    }
                    3 => {
                        state.region = ViewEditRegion::Unmatched;
                        state.unmatched_field_index = 0;
                    }
                    _ => {
                        state.region = ViewEditRegion::Unmatched;
                        state.unmatched_field_index = 1;
                    }
                },
                ViewSettingsColumn::Right => {
                    state.region = ViewEditRegion::Unmatched;
                    state.unmatched_field_index = match row.min(6) {
                        0 => 2,
                        1 => 3,
                        2 => 4,
                        3 => 8,
                        4 => 5,
                        5 => 6,
                        _ => 7,
                    };
                }
            }
        }
    }

    fn move_view_settings_focus(&mut self, code: KeyCode) -> bool {
        let Some(state) = self.view_edit_state.as_ref() else {
            return false;
        };
        let (column, row) = Self::view_settings_focus_position(state);
        match code {
            KeyCode::Char('j') | KeyCode::Down => {
                let max = match column {
                    ViewSettingsColumn::Left => 4,
                    ViewSettingsColumn::Right => 6,
                };
                self.set_view_settings_focus_position(column, (row + 1).min(max));
                true
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.set_view_settings_focus_position(column, row.saturating_sub(1));
                true
            }
            KeyCode::Char('l') | KeyCode::Right => {
                self.set_view_settings_focus_position(ViewSettingsColumn::Right, row.min(6));
                true
            }
            KeyCode::Char('h') | KeyCode::Left => {
                self.set_view_settings_focus_position(ViewSettingsColumn::Left, row.min(4));
                true
            }
            _ => false,
        }
    }

    pub(crate) fn handle_view_edit_settings_key(&mut self, code: KeyCode) -> TuiResult<bool> {
        if self.move_view_settings_focus(code) {
            return Ok(true);
        }

        let Some(state) = self.view_edit_state.as_ref() else {
            return Ok(false);
        };
        let (column, row) = Self::view_settings_focus_position(state);
        match (column, row, code) {
            (ViewSettingsColumn::Left, 0, KeyCode::Enter | KeyCode::Char(' ')) => {
                self.begin_view_edit_name_input();
            }
            (ViewSettingsColumn::Left, 1, KeyCode::Enter | KeyCode::Char(' ')) => {
                self.toggle_view_edit_view_type();
            }
            (ViewSettingsColumn::Left, 2, KeyCode::Enter | KeyCode::Char(' ')) => {
                if code == KeyCode::Char(' ') && self.cycle_selected_view_criterion() {
                    self.refresh_view_edit_preview();
                } else {
                    self.open_view_edit_view_criteria_picker();
                }
            }
            (ViewSettingsColumn::Left, 3, KeyCode::Enter | KeyCode::Char(' ')) => {
                if let Some(state) = &mut self.view_edit_state {
                    state.overlay = Some(ViewEditOverlay::BucketPicker {
                        target: BucketEditTarget::ViewVirtualInclude,
                    });
                    state.picker_index = 0;
                }
            }
            (ViewSettingsColumn::Left, 4, KeyCode::Enter | KeyCode::Char(' ')) => {
                if let Some(state) = &mut self.view_edit_state {
                    state.overlay = Some(ViewEditOverlay::BucketPicker {
                        target: BucketEditTarget::ViewVirtualExclude,
                    });
                    state.picker_index = 0;
                }
            }
            (ViewSettingsColumn::Right, 0, KeyCode::Enter | KeyCode::Char(' ')) => {
                self.toggle_view_edit_display_mode();
            }
            (ViewSettingsColumn::Right, 1, KeyCode::Enter | KeyCode::Char(' ')) => {
                self.toggle_view_edit_section_flow();
            }
            (ViewSettingsColumn::Right, 2, KeyCode::Enter | KeyCode::Char(' ')) => {
                self.toggle_view_edit_empty_sections();
            }
            (ViewSettingsColumn::Right, 3, KeyCode::Enter | KeyCode::Char(' ')) => {
                self.open_view_edit_alias_picker();
            }
            (ViewSettingsColumn::Right, 4, KeyCode::Enter | KeyCode::Char(' ')) => {
                self.toggle_view_edit_show_unmatched();
            }
            (ViewSettingsColumn::Right, 5, KeyCode::Enter | KeyCode::Char(' ')) => {
                self.toggle_view_edit_hide_dependent();
            }
            (ViewSettingsColumn::Right, 6, KeyCode::Enter | KeyCode::Char(' ')) => {
                self.begin_view_edit_unmatched_label_input();
            }
            (_, _, KeyCode::Char('r') | KeyCode::Char('R')) => self.begin_view_edit_name_input(),
            (_, _, KeyCode::Char('t') | KeyCode::Char('T')) => self.toggle_view_edit_view_type(),
            (_, _, KeyCode::Char('n') | KeyCode::Char('N')) => {
                self.open_view_edit_view_criteria_picker();
            }
            (_, _, KeyCode::Char(']')) => {
                self.set_view_settings_focus_position(ViewSettingsColumn::Left, 3);
                if let Some(state) = &mut self.view_edit_state {
                    state.overlay = Some(ViewEditOverlay::BucketPicker {
                        target: BucketEditTarget::ViewVirtualInclude,
                    });
                    state.picker_index = 0;
                }
            }
            (_, _, KeyCode::Char('[')) => {
                self.set_view_settings_focus_position(ViewSettingsColumn::Left, 4);
                if let Some(state) = &mut self.view_edit_state {
                    state.overlay = Some(ViewEditOverlay::BucketPicker {
                        target: BucketEditTarget::ViewVirtualExclude,
                    });
                    state.picker_index = 0;
                }
            }
            (_, _, KeyCode::Char('m') | KeyCode::Char('M')) => {
                self.set_view_settings_focus_position(ViewSettingsColumn::Right, 0);
                self.toggle_view_edit_display_mode();
            }
            (_, _, KeyCode::Char('w') | KeyCode::Char('W')) => {
                self.set_view_settings_focus_position(ViewSettingsColumn::Right, 1);
                self.toggle_view_edit_section_flow();
            }
            (_, _, KeyCode::Char('e') | KeyCode::Char('E')) => {
                self.set_view_settings_focus_position(ViewSettingsColumn::Right, 2);
                self.toggle_view_edit_empty_sections();
            }
            (_, _, KeyCode::Char('a') | KeyCode::Char('A')) => {
                self.set_view_settings_focus_position(ViewSettingsColumn::Right, 3);
                self.open_view_edit_alias_picker();
            }
            (_, _, KeyCode::Char('u')) => {
                self.set_view_settings_focus_position(ViewSettingsColumn::Right, 4);
                self.toggle_view_edit_show_unmatched();
            }
            (_, _, KeyCode::Char('d')) => {
                self.set_view_settings_focus_position(ViewSettingsColumn::Right, 5);
                self.toggle_view_edit_hide_dependent();
            }
            (_, _, KeyCode::Char('L')) => {
                self.set_view_settings_focus_position(ViewSettingsColumn::Right, 6);
                self.begin_view_edit_unmatched_label_input();
            }
            (_, _, KeyCode::Char('x')) if column == ViewSettingsColumn::Left && row == 2 => {
                self.remove_selected_view_criterion();
            }
            _ => {}
        }
        Ok(true)
    }

    fn toggle_view_edit_view_type(&mut self) {
        if let Some(state) = &mut self.view_edit_state {
            if !state.is_new_view {
                state.name_focused = false;
                state.view_type_focused = true;
                self.status = "View type is locked after view creation".to_string();
                return;
            }
            if state.draft.datebook_config.is_some() {
                state.draft.datebook_config = None;
                if state.draft.sections.is_empty() {
                    state.draft.sections.push(Self::view_edit_default_section(
                        Self::DEFAULT_VIEW_EDIT_SECTION_TITLE,
                    ));
                }
                self.status = "View type set to Board".to_string();
            } else {
                state.draft.datebook_config = Some(DatebookConfig::default());
                self.status = "View type set to Datebook".to_string();
            }
            state.dirty = true;
            state.discard_confirm = false;
            state.name_focused = false;
            state.view_type_focused = true;
        }
        self.refresh_view_edit_preview();
    }

    fn remove_selected_view_criterion(&mut self) {
        let mut changed = false;
        if let Some(state) = &mut self.view_edit_state {
            let idx = state.criteria_index;
            if idx < state.draft.criteria.criteria.len() {
                state.draft.criteria.criteria.remove(idx);
                let new_len = state.draft.criteria.criteria.len();
                if state.criteria_index >= new_len && new_len > 0 {
                    state.criteria_index = new_len - 1;
                }
                changed = true;
            }
        }
        if changed {
            self.set_view_edit_dirty();
            self.refresh_view_edit_preview();
        }
    }

    fn cycle_selected_view_criterion(&mut self) -> bool {
        let mut changed = false;
        if let Some(state) = &mut self.view_edit_state {
            let idx = state.criteria_index;
            if let Some(criterion) = state.draft.criteria.criteria.get_mut(idx) {
                criterion.mode = match criterion.mode {
                    CriterionMode::And => CriterionMode::Not,
                    CriterionMode::Not => CriterionMode::Or,
                    CriterionMode::Or => CriterionMode::And,
                };
                state.dirty = true;
                state.discard_confirm = false;
                changed = true;
            }
        }
        changed
    }

    fn toggle_view_edit_display_mode(&mut self) {
        if let Some(state) = &mut self.view_edit_state {
            state.draft.board_display_mode =
                Self::cycle_view_board_display_mode(state.draft.board_display_mode);
            state.dirty = true;
            state.discard_confirm = false;
        }
    }

    fn toggle_view_edit_section_flow(&mut self) {
        if let Some(state) = &mut self.view_edit_state {
            state.draft.section_flow = Self::cycle_view_section_flow(state.draft.section_flow);
            state.dirty = true;
            state.discard_confirm = false;
        }
    }

    fn toggle_view_edit_empty_sections(&mut self) {
        if let Some(state) = &mut self.view_edit_state {
            state.draft.empty_sections = state.draft.empty_sections.next();
            state.dirty = true;
            state.discard_confirm = false;
        }
    }

    fn toggle_view_edit_show_unmatched(&mut self) {
        if let Some(state) = &mut self.view_edit_state {
            state.draft.show_unmatched = !state.draft.show_unmatched;
            state.dirty = true;
            state.discard_confirm = false;
        }
        self.refresh_view_edit_preview();
    }

    fn toggle_view_edit_hide_dependent(&mut self) {
        if let Some(state) = &mut self.view_edit_state {
            state.draft.hide_dependent_items = !state.draft.hide_dependent_items;
            state.dirty = true;
            state.discard_confirm = false;
        }
        self.refresh_view_edit_preview();
    }

    fn begin_view_edit_unmatched_label_input(&mut self) {
        if let Some(state) = &mut self.view_edit_state {
            let current = state.draft.unmatched_label.clone();
            state.region = ViewEditRegion::Unmatched;
            state.unmatched_field_index = 7;
            state.inline_input = Some(ViewEditInlineInput::UnmatchedLabel);
            state.inline_buf = text_buffer::TextBuffer::new(current);
        }
        self.status = "Unmatched label: type text  Enter:confirm  Esc:cancel".to_string();
    }

    pub(crate) fn handle_view_edit_datebook_key(&mut self, code: KeyCode) -> TuiResult<bool> {
        match code {
            KeyCode::Char('j') | KeyCode::Down => {
                if let Some(state) = &mut self.view_edit_state {
                    state.datebook_field_index =
                        (state.datebook_field_index + 1).min(Self::DATEBOOK_FIELD_COUNT - 1);
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if let Some(state) = &mut self.view_edit_state {
                    state.datebook_field_index = state.datebook_field_index.saturating_sub(1);
                }
            }
            KeyCode::Char(' ') | KeyCode::Enter => {
                if let Some(state) = &mut self.view_edit_state {
                    if let Some(config) = &mut state.draft.datebook_config {
                        match state.datebook_field_index {
                            0 => config.period = config.period.next(),
                            1 => config.interval = config.interval.next(),
                            2 => config.anchor = config.anchor.next(),
                            3 => config.date_source = config.date_source.next(),
                            _ => {}
                        }
                        // Auto-fix invalid combos
                        while !config.is_valid() {
                            config.interval = config.interval.next();
                        }
                        state.dirty = true;
                        state.discard_confirm = false;
                    }
                }
                self.refresh_view_edit_preview();
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                self.begin_view_edit_name_input();
            }
            _ => {}
        }
        Ok(true)
    }

    pub(crate) fn open_view_edit_view_criteria_picker(&mut self) {
        let first = first_non_reserved_category_index(&self.category_rows);
        if let Some(state) = &mut self.view_edit_state {
            state.overlay = Some(ViewEditOverlay::CategoryPicker {
                target: CategoryEditTarget::ViewCriteria,
            });
            state.picker_index = first;
        }
        self.status =
            "Criteria: Space:cycle  +/1:require  -/2:exclude  3:or  0:clear  Enter/Tab:done"
                .to_string();
    }

    fn open_view_edit_alias_picker(&mut self) {
        let first = first_non_reserved_category_index(&self.category_rows);
        if let Some(state) = &mut self.view_edit_state {
            state.region = ViewEditRegion::Unmatched;
            state.unmatched_field_index = 8;
            state.overlay = Some(ViewEditOverlay::CategoryPicker {
                target: CategoryEditTarget::ViewAliases,
            });
            state.picker_index = first;
        }
        self.status = Self::view_edit_alias_picker_status();
    }

    pub(crate) fn handle_view_edit_preview_key(&mut self, code: KeyCode) -> TuiResult<bool> {
        match code {
            KeyCode::Char('j') | KeyCode::Down => {
                if let Some(state) = &mut self.view_edit_state {
                    state.preview_scroll = state.preview_scroll.saturating_add(1);
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if let Some(state) = &mut self.view_edit_state {
                    state.preview_scroll = state.preview_scroll.saturating_sub(1);
                }
            }
            _ => {}
        }
        Ok(true)
    }
}
