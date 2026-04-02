use crate::*;

impl App {
    pub(crate) fn toggle_view_edit_section_show_children(&mut self, section_index: usize) {
        if let Some(state) = &mut self.view_edit_state {
            if let Some(section) = state.draft.sections.get_mut(section_index) {
                section.show_children = !section.show_children;
                state.dirty = true;
                state.discard_confirm = false;
            }
        }
    }

    pub(crate) fn view_edit_section_layout_value(&self, section: &Section) -> String {
        if section.show_children {
            if self
                .view_edit_section_split_unavailable_reason(section)
                .is_some()
            {
                "Split by direct child category (inactive)".to_string()
            } else {
                "Split by direct child category".to_string()
            }
        } else {
            "Flat (single section)".to_string()
        }
    }

    pub(crate) fn view_edit_section_split_unavailable_reason(
        &self,
        section: &Section,
    ) -> Option<String> {
        if !section.criteria.virtual_include.is_empty()
            || !section.criteria.virtual_exclude.is_empty()
            || section.criteria.text_search.is_some()
        {
            return Some("Split unavailable: remove date/text filters.".to_string());
        }

        let and_ids: Vec<CategoryId> = section.criteria.and_category_ids().collect();
        if and_ids.len() != 1 {
            return Some("Split unavailable: requires exactly one Include category.".to_string());
        }

        if section.criteria.not_category_ids().count() > 0
            || section.criteria.or_category_ids().count() > 0
        {
            return Some("Split unavailable: remove Exclude/Match-any criteria.".to_string());
        }

        let parent_id = and_ids[0];
        let Some(parent) = self
            .categories
            .iter()
            .find(|category| category.id == parent_id)
        else {
            return Some("Split unavailable: selected category no longer exists.".to_string());
        };

        if parent.children.is_empty() {
            return Some(format!(
                "Split unavailable: \"{}\" has no child categories.",
                parent.name
            ));
        }

        None
    }

    fn begin_view_edit_section_title_input(&mut self, section_index: usize) {
        self.begin_view_edit_section_title_input_inner(section_index, false);
    }

    fn begin_view_edit_new_section_title_input(&mut self, section_index: usize) {
        self.begin_view_edit_section_title_input_inner(section_index, true);
    }

    fn begin_view_edit_section_title_input_inner(&mut self, section_index: usize, is_new: bool) {
        if let Some(state) = &mut self.view_edit_state {
            if let Some(section) = state.draft.sections.get(section_index) {
                state.region = ViewEditRegion::Sections;
                state.pane_focus = ViewEditPaneFocus::Sections;
                state.section_index = section_index;
                state.sections_view_row_selected = false;
                state.section_details_field_index = 0;
                state.inline_input =
                    Some(ViewEditInlineInput::SectionTitle { section_index, is_new });
                state.inline_buf = text_buffer::TextBuffer::new(section.title.clone());
                state.discard_confirm = false;
                self.status = "Section title: type text  Enter:confirm  Esc:cancel".to_string();
            }
        }
    }

    fn insert_view_edit_section(&mut self, insert_index: usize) -> Option<usize> {
        let mut new_index = None;
        if let Some(state) = &mut self.view_edit_state {
            let idx = insert_index.min(state.draft.sections.len());
            state.draft.sections.insert(
                idx,
                Self::view_edit_default_section(Self::DEFAULT_VIEW_EDIT_SECTION_TITLE),
            );
            state.section_index = idx;
            state.sections_view_row_selected = false;
            state.section_details_field_index = 0;
            new_index = Some(idx);
        }
        if new_index.is_some() {
            self.set_view_edit_dirty();
        }
        new_index
    }

    pub(crate) fn request_view_edit_section_delete(&mut self, section_index: usize) {
        if let Some(state) = &mut self.view_edit_state {
            if section_index < state.draft.sections.len() {
                state.section_delete_confirm = Some(section_index);
                state.discard_confirm = false;
                let title = state.draft.sections[section_index].title.clone();
                self.status = format!("Delete section \"{title}\"? y/n");
            }
        }
    }

    pub(crate) fn confirm_view_edit_section_delete(&mut self) {
        let Some(idx) = self
            .view_edit_state
            .as_ref()
            .and_then(|s| s.section_delete_confirm)
        else {
            return;
        };

        if let Some(state) = &mut self.view_edit_state {
            state.section_delete_confirm = None;
            if idx >= state.draft.sections.len() {
                self.status = Self::view_edit_default_status();
                return;
            }

            state.draft.sections.remove(idx);
            let new_len = state.draft.sections.len();
            if state.section_index >= new_len && new_len > 0 {
                state.section_index = new_len - 1;
            }
            if new_len == 0 {
                state.sections_view_row_selected = true;
                state.section_details_field_index = 0;
            }
            state.dirty = true;
            state.discard_confirm = false;
        }
        self.status = Self::view_edit_default_status();
    }

    pub(crate) fn handle_view_edit_section_details_key(
        &mut self,
        code: KeyCode,
    ) -> TuiResult<bool> {
        let field_count = 7usize;
        let section_index = self
            .view_edit_state
            .as_ref()
            .map(|s| s.section_index)
            .unwrap_or(0);
        let current_index = self
            .view_edit_state
            .as_ref()
            .map(|s| {
                s.section_details_field_index
                    .min(field_count.saturating_sub(1))
            })
            .unwrap_or(0);

        match code {
            KeyCode::Char('j') | KeyCode::Down => {
                if let Some(state) = &mut self.view_edit_state {
                    state.section_details_field_index =
                        (current_index + 1).min(field_count.saturating_sub(1));
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if let Some(state) = &mut self.view_edit_state {
                    state.section_details_field_index = current_index.saturating_sub(1);
                }
            }
            KeyCode::Char(' ') | KeyCode::Enter => {
                let mapped = match current_index {
                    0 => Some(KeyCode::Char('e')),
                    1 => Some(KeyCode::Char('f')),
                    2 => Some(KeyCode::Char('c')),
                    3 => Some(KeyCode::Char('m')),
                    4 => Some(KeyCode::Char('a')),
                    5 => Some(KeyCode::Char('r')),
                    6 => None,
                    _ => None,
                };
                if let Some(mapped) = mapped {
                    return self.handle_view_edit_sections_key(mapped);
                }
                if current_index == 6 {
                    self.toggle_view_edit_section_show_children(section_index);
                    return Ok(true);
                }
            }
            _ => {
                return self.handle_view_edit_sections_key(code);
            }
        }
        Ok(true)
    }

    pub(crate) fn handle_view_edit_sections_key(&mut self, code: KeyCode) -> TuiResult<bool> {
        let Some(state) = &self.view_edit_state else {
            return Ok(false);
        };
        let len = state.draft.sections.len();
        let idx = state.section_index;
        let selecting_view_row = state.sections_view_row_selected;
        let visible_indices = Self::view_edit_visible_section_indices(state);
        let current_visible_pos = visible_indices.iter().position(|&i| i == idx);

        match code {
            KeyCode::Char('j') | KeyCode::Down => {
                if let Some(state) = &mut self.view_edit_state {
                    if state.sections_view_row_selected {
                        if let Some(&first_visible) = visible_indices.first() {
                            state.sections_view_row_selected = false;
                            state.region = ViewEditRegion::Sections;
                            state.section_index = first_visible;
                            state.section_details_field_index = 0;
                        }
                    } else {
                        state.region = ViewEditRegion::Sections;
                        if let Some(pos) = current_visible_pos {
                            let next_pos = (pos + 1).min(visible_indices.len().saturating_sub(1));
                            state.section_index = visible_indices[next_pos];
                        } else if let Some(&first_visible) = visible_indices.first() {
                            state.section_index = first_visible;
                        } else if len > 0 {
                            state.section_index = next_index_clamped(idx, len, 1);
                        }
                        state.section_details_field_index = 0;
                    }
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if let Some(state) = &mut self.view_edit_state {
                    if !state.sections_view_row_selected {
                        let at_first_visible = current_visible_pos.map(|p| p == 0).unwrap_or(true);
                        if at_first_visible || visible_indices.is_empty() {
                            state.sections_view_row_selected = true;
                            state.region = ViewEditRegion::Sections;
                        } else {
                            state.region = ViewEditRegion::Sections;
                            if let Some(pos) = current_visible_pos {
                                state.section_index = visible_indices[pos.saturating_sub(1)];
                            } else {
                                state.section_index = visible_indices[0];
                            }
                            state.section_details_field_index = 0;
                        }
                    }
                }
            }
            KeyCode::Char('n') => {
                let filter_active = self
                    .view_edit_state
                    .as_ref()
                    .map(Self::view_edit_filter_is_active)
                    .unwrap_or(false);
                if filter_active {
                    self.clear_view_edit_section_filter();
                }
                let insert_index = if selecting_view_row || len == 0 {
                    0
                } else {
                    (idx + 1).min(len)
                };
                if let Some(new_index) = self.insert_view_edit_section(insert_index) {
                    self.begin_view_edit_new_section_title_input(new_index);
                }
            }
            KeyCode::Char('N') => {
                let filter_active = self
                    .view_edit_state
                    .as_ref()
                    .map(Self::view_edit_filter_is_active)
                    .unwrap_or(false);
                if filter_active {
                    self.clear_view_edit_section_filter();
                }
                let insert_index = if selecting_view_row || len == 0 {
                    0
                } else {
                    idx.min(len)
                };
                if let Some(new_index) = self.insert_view_edit_section(insert_index) {
                    self.begin_view_edit_new_section_title_input(new_index);
                }
            }
            KeyCode::Char('x') => {
                if selecting_view_row {
                    return Ok(true);
                }
                self.request_view_edit_section_delete(idx);
            }
            KeyCode::Char('[') | KeyCode::Char('K') => {
                if selecting_view_row {
                    return Ok(true);
                }
                if let Some(state) = &mut self.view_edit_state {
                    if idx > 0 && idx < state.draft.sections.len() {
                        state.draft.sections.swap(idx, idx - 1);
                        state.section_index = idx - 1;
                        state.dirty = true;
                        state.discard_confirm = false;
                    }
                }
            }
            KeyCode::Char(']') | KeyCode::Char('J') => {
                if selecting_view_row {
                    return Ok(true);
                }
                if let Some(state) = &mut self.view_edit_state {
                    if idx + 1 < state.draft.sections.len() {
                        state.draft.sections.swap(idx, idx + 1);
                        state.section_index = idx + 1;
                        state.dirty = true;
                        state.discard_confirm = false;
                    }
                }
            }
            KeyCode::Enter => {
                if selecting_view_row {
                    if let Some(state) = &mut self.view_edit_state {
                        state.region = ViewEditRegion::Criteria;
                        state.pane_focus = ViewEditPaneFocus::Details;
                    }
                    return Ok(true);
                }
                if let Some(state) = &mut self.view_edit_state {
                    if idx < len {
                        state.pane_focus = ViewEditPaneFocus::Details;
                        state.section_details_field_index = 0;
                    }
                }
            }
            KeyCode::Char('t') | KeyCode::Char('e') => {
                if selecting_view_row {
                    return Ok(true);
                }
                if let Some(state) = &mut self.view_edit_state {
                    state.section_details_field_index = 0;
                }
                self.begin_view_edit_section_title_input(idx);
            }
            KeyCode::Char('f') => {
                if selecting_view_row {
                    return Ok(true);
                }
                if idx < len {
                    let first = first_non_reserved_category_index(&self.category_rows);
                    if let Some(state) = &mut self.view_edit_state {
                        state.overlay = Some(ViewEditOverlay::CategoryPicker {
                            target: CategoryEditTarget::SectionCriteria,
                        });
                        state.picker_index = first;
                    }
                    self.status = "Section criteria: Space:cycle  +/1:require  -/2:exclude  3:or  0:clear  Esc:done"
                        .to_string();
                }
            }
            KeyCode::Char('a') => {
                if selecting_view_row {
                    return Ok(true);
                }
                if idx < len {
                    let first = first_non_reserved_category_index(&self.category_rows);
                    if let Some(state) = &mut self.view_edit_state {
                        state.overlay = Some(ViewEditOverlay::CategoryPicker {
                            target: CategoryEditTarget::SectionOnInsertAssign,
                        });
                        state.picker_index = first;
                    }
                    self.status = "Edit on-insert assign: j/k select  Space/Enter:toggle  Esc:done"
                        .to_string();
                }
            }
            KeyCode::Char('c') | KeyCode::Char('C') => {
                if selecting_view_row {
                    return Ok(true);
                }
                if idx < len {
                    let first = first_non_reserved_category_index(&self.category_rows);
                    if let Some(state) = &mut self.view_edit_state {
                        state.overlay = Some(ViewEditOverlay::CategoryPicker {
                            target: CategoryEditTarget::SectionColumns,
                        });
                        state.picker_index = first;
                    }
                    self.status = "Edit section columns: j/k select  Space/Enter:toggle  Esc:done  (leaf tags hidden)"
                        .to_string();
                }
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                if selecting_view_row {
                    self.begin_view_edit_name_input();
                    return Ok(true);
                }
                if idx < len {
                    let first = first_non_reserved_category_index(&self.category_rows);
                    if let Some(state) = &mut self.view_edit_state {
                        state.overlay = Some(ViewEditOverlay::CategoryPicker {
                            target: CategoryEditTarget::SectionOnRemoveUnassign,
                        });
                        state.picker_index = first;
                    }
                    self.status =
                        "Edit on-remove unassign: j/k select  Space/Enter:toggle  Esc:done"
                            .to_string();
                }
            }
            KeyCode::Char('m') | KeyCode::Char('M') => {
                if selecting_view_row {
                    return Ok(true);
                }
                if idx < len {
                    if let Some(state) = &mut self.view_edit_state {
                        if let Some(section) = state.draft.sections.get_mut(idx) {
                            section.board_display_mode_override =
                                Self::cycle_section_board_display_mode_override(
                                    section.board_display_mode_override,
                                );
                            state.dirty = true;
                            state.discard_confirm = false;
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(true)
    }
}
