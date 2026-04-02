use crate::*;

impl App {
    pub(crate) fn view_details_criteria_row_count(state: &ViewEditState) -> usize {
        state.draft.criteria.criteria.len().max(1)
    }

    pub(crate) fn view_details_aux_field_count() -> usize {
        8
    }

    pub(crate) fn view_edit_showing_view_details(state: &ViewEditState) -> bool {
        state.region != ViewEditRegion::Sections
            || state.sections_view_row_selected
            || state.draft.sections.get(state.section_index).is_none()
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

    fn view_details_focus_index(state: &ViewEditState) -> usize {
        let criteria_rows = Self::view_details_criteria_row_count(state);
        match state.region {
            ViewEditRegion::Criteria => state.criteria_index.min(criteria_rows.saturating_sub(1)),
            ViewEditRegion::Unmatched => {
                criteria_rows
                    + state
                        .unmatched_field_index
                        .min(Self::view_details_aux_field_count() - 1)
            }
            ViewEditRegion::Sections => 0,
        }
    }

    fn set_view_details_focus_index(&mut self, new_index: usize) {
        if let Some(state) = &mut self.view_edit_state {
            let criteria_rows = Self::view_details_criteria_row_count(state);
            if new_index < criteria_rows {
                state.region = ViewEditRegion::Criteria;
                state.criteria_index = if state.draft.criteria.criteria.is_empty() {
                    0
                } else {
                    new_index.min(state.draft.criteria.criteria.len().saturating_sub(1))
                };
            } else {
                state.region = ViewEditRegion::Unmatched;
                state.unmatched_field_index =
                    (new_index - criteria_rows).min(Self::view_details_aux_field_count() - 1);
            }
        }
    }

    pub(crate) fn handle_view_edit_criteria_key(&mut self, code: KeyCode) -> TuiResult<bool> {
        let Some(state) = &self.view_edit_state else {
            return Ok(false);
        };
        let idx = state.criteria_index;
        let criteria_rows = Self::view_details_criteria_row_count(state);

        match code {
            KeyCode::Char('j') | KeyCode::Down => {
                let current = self
                    .view_edit_state
                    .as_ref()
                    .map(Self::view_details_focus_index)
                    .unwrap_or(0);
                let max_index = criteria_rows + Self::view_details_aux_field_count() - 1;
                self.set_view_details_focus_index((current + 1).min(max_index));
            }
            KeyCode::Char('k') | KeyCode::Up => {
                let current = self
                    .view_edit_state
                    .as_ref()
                    .map(Self::view_details_focus_index)
                    .unwrap_or(0);
                self.set_view_details_focus_index(current.saturating_sub(1));
            }
            KeyCode::Char('n') | KeyCode::Char('N') => {
                self.open_view_edit_view_criteria_picker();
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                self.begin_view_edit_name_input();
            }
            KeyCode::Char('x') => {
                let mut changed = false;
                if let Some(state) = &mut self.view_edit_state {
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
            KeyCode::Char(' ') => {
                let mut changed = false;
                if let Some(state) = &mut self.view_edit_state {
                    if let Some(criterion) = state.draft.criteria.criteria.get_mut(idx) {
                        criterion.mode = match criterion.mode {
                            CriterionMode::And => CriterionMode::Not,
                            CriterionMode::Not => CriterionMode::Or,
                            CriterionMode::Or => CriterionMode::And,
                        };
                        changed = true;
                    }
                }
                if changed {
                    self.set_view_edit_dirty();
                    self.refresh_view_edit_preview();
                } else {
                    self.open_view_edit_view_criteria_picker();
                }
            }
            KeyCode::Enter => {
                self.open_view_edit_view_criteria_picker();
            }
            KeyCode::Char(']') => {
                if let Some(state) = &mut self.view_edit_state {
                    state.overlay = Some(ViewEditOverlay::BucketPicker {
                        target: BucketEditTarget::ViewVirtualInclude,
                    });
                    state.picker_index = 0;
                }
            }
            KeyCode::Char('[') => {
                if let Some(state) = &mut self.view_edit_state {
                    state.overlay = Some(ViewEditOverlay::BucketPicker {
                        target: BucketEditTarget::ViewVirtualExclude,
                    });
                    state.picker_index = 0;
                }
            }
            KeyCode::Char('m') | KeyCode::Char('M') => {
                if let Some(state) = &mut self.view_edit_state {
                    state.draft.board_display_mode =
                        Self::cycle_view_board_display_mode(state.draft.board_display_mode);
                    state.dirty = true;
                    state.discard_confirm = false;
                }
            }
            KeyCode::Char('w') | KeyCode::Char('W') => {
                if let Some(state) = &mut self.view_edit_state {
                    state.draft.section_flow =
                        Self::cycle_view_section_flow(state.draft.section_flow);
                    state.dirty = true;
                    state.discard_confirm = false;
                }
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
            "Criteria: Space:cycle  +/1:require  -/2:exclude  3:or  0:clear  Esc:done"
                .to_string();
    }

    fn open_view_edit_alias_picker(&mut self) {
        let first = first_non_reserved_category_index(&self.category_rows);
        if let Some(state) = &mut self.view_edit_state {
            state.region = ViewEditRegion::Unmatched;
            state.unmatched_field_index = 7;
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

    pub(crate) fn handle_view_edit_unmatched_key(&mut self, code: KeyCode) -> TuiResult<bool> {
        match code {
            KeyCode::Char('j') | KeyCode::Down => {
                let current = self
                    .view_edit_state
                    .as_ref()
                    .map(Self::view_details_focus_index)
                    .unwrap_or(0);
                if let Some(state) = &self.view_edit_state {
                    let criteria_rows = Self::view_details_criteria_row_count(state);
                    let max_index = criteria_rows + Self::view_details_aux_field_count() - 1;
                    self.set_view_details_focus_index((current + 1).min(max_index));
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                let current = self
                    .view_edit_state
                    .as_ref()
                    .map(Self::view_details_focus_index)
                    .unwrap_or(0);
                self.set_view_details_focus_index(current.saturating_sub(1));
            }
            KeyCode::Char('t') => {
                if let Some(state) = &mut self.view_edit_state {
                    state.unmatched_field_index = 4;
                    state.draft.show_unmatched = !state.draft.show_unmatched;
                    state.dirty = true;
                    state.discard_confirm = false;
                }
            }
            KeyCode::Char('l') => {
                if let Some(state) = &mut self.view_edit_state {
                    let current = state.draft.unmatched_label.clone();
                    state.unmatched_field_index = 6;
                    state.inline_input = Some(ViewEditInlineInput::UnmatchedLabel);
                    state.inline_buf = text_buffer::TextBuffer::new(current);
                }
                self.status = "Unmatched label: type text  Enter:confirm  Esc:cancel".to_string();
            }
            KeyCode::Char('d') => {
                if let Some(state) = &mut self.view_edit_state {
                    state.unmatched_field_index = 5;
                    state.draft.hide_dependent_items = !state.draft.hide_dependent_items;
                    state.dirty = true;
                    state.discard_confirm = false;
                }
            }
            KeyCode::Char('w') | KeyCode::Char('W') => {
                if let Some(state) = &mut self.view_edit_state {
                    state.unmatched_field_index = 3;
                    state.draft.section_flow =
                        Self::cycle_view_section_flow(state.draft.section_flow);
                    state.dirty = true;
                    state.discard_confirm = false;
                }
            }
            KeyCode::Char('a') | KeyCode::Char('A') => {
                self.open_view_edit_alias_picker();
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                self.begin_view_edit_name_input();
            }
            KeyCode::Char(']') => {
                if let Some(state) = &mut self.view_edit_state {
                    state.unmatched_field_index = 0;
                    state.overlay = Some(ViewEditOverlay::BucketPicker {
                        target: BucketEditTarget::ViewVirtualInclude,
                    });
                    state.picker_index = 0;
                }
            }
            KeyCode::Char('[') => {
                if let Some(state) = &mut self.view_edit_state {
                    state.unmatched_field_index = 1;
                    state.overlay = Some(ViewEditOverlay::BucketPicker {
                        target: BucketEditTarget::ViewVirtualExclude,
                    });
                    state.picker_index = 0;
                }
            }
            KeyCode::Char('m') | KeyCode::Char('M') => {
                if let Some(state) = &mut self.view_edit_state {
                    state.unmatched_field_index = 2;
                    state.draft.board_display_mode =
                        Self::cycle_view_board_display_mode(state.draft.board_display_mode);
                    state.dirty = true;
                    state.discard_confirm = false;
                }
            }
            KeyCode::Char(' ') | KeyCode::Enter => {
                let target = self
                    .view_edit_state
                    .as_ref()
                    .map(|s| s.unmatched_field_index)
                    .unwrap_or(0);
                match target {
                    0 => {
                        if let Some(state) = &mut self.view_edit_state {
                            state.overlay = Some(ViewEditOverlay::BucketPicker {
                                target: BucketEditTarget::ViewVirtualInclude,
                            });
                            state.picker_index = 0;
                        }
                    }
                    1 => {
                        if let Some(state) = &mut self.view_edit_state {
                            state.overlay = Some(ViewEditOverlay::BucketPicker {
                                target: BucketEditTarget::ViewVirtualExclude,
                            });
                            state.picker_index = 0;
                        }
                    }
                    2 => {
                        if let Some(state) = &mut self.view_edit_state {
                            state.draft.board_display_mode =
                                Self::cycle_view_board_display_mode(state.draft.board_display_mode);
                            state.dirty = true;
                            state.discard_confirm = false;
                        }
                    }
                    3 => {
                        if let Some(state) = &mut self.view_edit_state {
                            state.draft.section_flow =
                                Self::cycle_view_section_flow(state.draft.section_flow);
                            state.dirty = true;
                            state.discard_confirm = false;
                        }
                    }
                    4 => {
                        if let Some(state) = &mut self.view_edit_state {
                            state.draft.show_unmatched = !state.draft.show_unmatched;
                            state.dirty = true;
                            state.discard_confirm = false;
                        }
                    }
                    5 => {
                        if let Some(state) = &mut self.view_edit_state {
                            state.draft.hide_dependent_items = !state.draft.hide_dependent_items;
                            state.dirty = true;
                            state.discard_confirm = false;
                        }
                    }
                    _ => {
                        if target == 6 {
                            if let Some(state) = &mut self.view_edit_state {
                                let current = state.draft.unmatched_label.clone();
                                state.unmatched_field_index = 6;
                                state.inline_input = Some(ViewEditInlineInput::UnmatchedLabel);
                                state.inline_buf = text_buffer::TextBuffer::new(current);
                            }
                            self.status =
                                "Unmatched label: type text  Enter:confirm  Esc:cancel".to_string();
                        } else {
                            self.open_view_edit_alias_picker();
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(true)
    }
}
