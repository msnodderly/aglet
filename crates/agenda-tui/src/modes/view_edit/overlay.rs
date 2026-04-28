use crate::*;

impl App {
    fn toggle_criterion_mode(query: &mut Query, cat_id: CategoryId, mode: CriterionMode) {
        if query.mode_for(cat_id) == Some(mode) {
            query.remove_criterion(cat_id);
        } else {
            query.set_criterion(mode, cat_id);
        }
    }

    fn cycle_criterion_mode(query: &mut Query, cat_id: CategoryId) {
        let next = match query.mode_for(cat_id) {
            None => Some(CriterionMode::And),
            Some(CriterionMode::And) => Some(CriterionMode::Not),
            Some(CriterionMode::Not) => Some(CriterionMode::Or),
            Some(CriterionMode::Or) => None,
        };
        match next {
            Some(mode) => query.set_criterion(mode, cat_id),
            None => query.remove_criterion(cat_id),
        }
    }

    fn toggle_category_picker_selection(
        &mut self,
        target: CategoryEditTarget,
        section_index: usize,
        cat_id: CategoryId,
        mode: Option<CriterionMode>,
    ) {
        if let Some(state) = &mut self.view_edit_state {
            match target {
                CategoryEditTarget::ViewCriteria => {
                    if let Some(mode) = mode {
                        Self::toggle_criterion_mode(&mut state.draft.criteria, cat_id, mode);
                    }
                }
                CategoryEditTarget::ViewAliases => {}
                CategoryEditTarget::SectionCriteria => {
                    if let Some(section) = state.draft.sections.get_mut(section_index) {
                        if let Some(mode) = mode {
                            Self::toggle_criterion_mode(&mut section.criteria, cat_id, mode);
                        }
                    }
                }
                CategoryEditTarget::SectionColumns => {
                    if let Some(section) = state.draft.sections.get_mut(section_index) {
                        if let Some(existing_index) =
                            section.columns.iter().position(|col| col.heading == cat_id)
                        {
                            section.columns.remove(existing_index);
                        } else if let Some(cat) = self.categories.iter().find(|c| c.id == cat_id) {
                            section.columns.push(Column {
                                kind: column_kind_for_heading(cat),
                                heading: cat_id,
                                width: 12,
                                summary_fn: None,
                            });
                        }
                    }
                }
                CategoryEditTarget::SectionOnInsertAssign => {
                    if let Some(section) = state.draft.sections.get_mut(section_index) {
                        if !section.on_insert_assign.remove(&cat_id) {
                            section.on_insert_assign.insert(cat_id);
                        }
                    }
                }
                CategoryEditTarget::SectionOnRemoveUnassign => {
                    if let Some(section) = state.draft.sections.get_mut(section_index) {
                        if !section.on_remove_unassign.remove(&cat_id) {
                            section.on_remove_unassign.insert(cat_id);
                        }
                    }
                }
            }
        }
        self.set_view_edit_dirty();
        self.refresh_view_edit_preview();
    }

    pub(crate) fn handle_view_edit_overlay_key(&mut self, code: KeyCode) -> TuiResult<bool> {
        let Some(state) = &self.view_edit_state else {
            return Ok(false);
        };
        let overlay = state.overlay.clone();
        let picker_index = state.picker_index;
        let section_index = state.section_index;

        match overlay {
            Some(ViewEditOverlay::CategoryPicker { target }) => {
                let is_criteria_picker = matches!(
                    target,
                    CategoryEditTarget::ViewCriteria | CategoryEditTarget::SectionCriteria
                );
                let filtered_indices = self
                    .view_edit_state
                    .as_ref()
                    .map(|s| self.view_edit_filtered_category_row_indices(s))
                    .unwrap_or_default();
                let current_visible_pos = filtered_indices
                    .iter()
                    .position(|&actual_idx| actual_idx == picker_index)
                    .unwrap_or(0);
                match code {
                    KeyCode::Tab | KeyCode::BackTab => {
                        let forward = code == KeyCode::Tab;
                        self.close_view_edit_overlay();
                        self.cycle_view_edit_pane_focus(forward);
                    }
                    KeyCode::Char('j') | KeyCode::Down => {
                        if let Some(state) = &mut self.view_edit_state {
                            if let Some(&actual_idx) = filtered_indices.get(
                                (current_visible_pos + 1)
                                    .min(filtered_indices.len().saturating_sub(1)),
                            ) {
                                state.picker_index = actual_idx;
                            }
                        }
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        if let Some(state) = &mut self.view_edit_state {
                            if let Some(&actual_idx) =
                                filtered_indices.get(current_visible_pos.saturating_sub(1))
                            {
                                state.picker_index = actual_idx;
                            }
                        }
                    }
                    KeyCode::Enter if is_criteria_picker => {
                        self.close_view_edit_overlay();
                    }
                    KeyCode::Char('a') | KeyCode::Char('A')
                        if matches!(target, CategoryEditTarget::ViewAliases) =>
                    {
                        if let Some(&actual_idx) = filtered_indices.get(current_visible_pos) {
                            if let Some(row) = self.category_rows.get(actual_idx).cloned() {
                                self.begin_view_edit_alias_input(row.id);
                            }
                        }
                    }
                    KeyCode::Char(' ') | KeyCode::Enter => {
                        if matches!(target, CategoryEditTarget::ViewAliases) {
                            if let Some(&actual_idx) = filtered_indices.get(current_visible_pos) {
                                if let Some(row) = self.category_rows.get(actual_idx).cloned() {
                                    self.begin_view_edit_alias_input(row.id);
                                }
                            }
                        } else if is_criteria_picker {
                            if let Some(&actual_idx) = filtered_indices.get(current_visible_pos) {
                                if let Some(row) = self.category_rows.get(actual_idx).cloned() {
                                    if let Some(state) = &mut self.view_edit_state {
                                        let query = match target {
                                            CategoryEditTarget::ViewCriteria => {
                                                Some(&mut state.draft.criteria)
                                            }
                                            CategoryEditTarget::SectionCriteria => state
                                                .draft
                                                .sections
                                                .get_mut(section_index)
                                                .map(|s| &mut s.criteria),
                                            _ => None,
                                        };
                                        if let Some(query) = query {
                                            Self::cycle_criterion_mode(query, row.id);
                                        }
                                    }
                                    self.set_view_edit_dirty();
                                    self.refresh_view_edit_preview();
                                }
                            }
                        } else if let Some(&actual_idx) = filtered_indices.get(current_visible_pos)
                        {
                            if let Some(row) = self.category_rows.get(actual_idx).cloned() {
                                self.toggle_category_picker_selection(
                                    target,
                                    section_index,
                                    row.id,
                                    Some(CriterionMode::And),
                                );
                            }
                        }
                    }
                    KeyCode::Char('1') | KeyCode::Char('+') if is_criteria_picker => {
                        if let Some(&actual_idx) = filtered_indices.get(current_visible_pos) {
                            if let Some(row) = self.category_rows.get(actual_idx).cloned() {
                                self.toggle_category_picker_selection(
                                    target,
                                    section_index,
                                    row.id,
                                    Some(CriterionMode::And),
                                );
                            }
                        }
                    }
                    KeyCode::Char('2') | KeyCode::Char('-') if is_criteria_picker => {
                        if let Some(&actual_idx) = filtered_indices.get(current_visible_pos) {
                            if let Some(row) = self.category_rows.get(actual_idx).cloned() {
                                self.toggle_category_picker_selection(
                                    target,
                                    section_index,
                                    row.id,
                                    Some(CriterionMode::Not),
                                );
                            }
                        }
                    }
                    KeyCode::Char('3') if is_criteria_picker => {
                        if let Some(&actual_idx) = filtered_indices.get(current_visible_pos) {
                            if let Some(row) = self.category_rows.get(actual_idx).cloned() {
                                self.toggle_category_picker_selection(
                                    target,
                                    section_index,
                                    row.id,
                                    Some(CriterionMode::Or),
                                );
                            }
                        }
                    }
                    KeyCode::Char('0') if is_criteria_picker => {
                        if let Some(&actual_idx) = filtered_indices.get(current_visible_pos) {
                            if let Some(row) = self.category_rows.get(actual_idx).cloned() {
                                if let Some(state) = &mut self.view_edit_state {
                                    let query = match target {
                                        CategoryEditTarget::ViewCriteria => {
                                            Some(&mut state.draft.criteria)
                                        }
                                        CategoryEditTarget::SectionCriteria => state
                                            .draft
                                            .sections
                                            .get_mut(section_index)
                                            .map(|s| &mut s.criteria),
                                        _ => None,
                                    };
                                    if let Some(query) = query {
                                        query.remove_criterion(row.id);
                                    }
                                }
                                self.set_view_edit_dirty();
                                self.refresh_view_edit_preview();
                            }
                        }
                    }
                    KeyCode::Esc => {
                        self.close_view_edit_overlay();
                    }
                    _ => {}
                }
            }
            Some(ViewEditOverlay::BucketPicker { target }) => {
                let options = when_bucket_options();
                match code {
                    KeyCode::Char('j') | KeyCode::Down => {
                        if let Some(state) = &mut self.view_edit_state {
                            state.picker_index = next_index_clamped(picker_index, options.len(), 1);
                        }
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        if let Some(state) = &mut self.view_edit_state {
                            state.picker_index =
                                next_index_clamped(picker_index, options.len(), -1);
                        }
                    }
                    KeyCode::Char(' ') | KeyCode::Enter | KeyCode::Esc => {
                        if matches!(code, KeyCode::Char(' ') | KeyCode::Enter) {
                            if let Some(&bucket) = options.get(picker_index) {
                                if let Some(state) = &mut self.view_edit_state {
                                    if let Some(set) =
                                        bucket_target_set_mut(&mut state.draft, target)
                                    {
                                        if set.contains(&bucket) {
                                            set.remove(&bucket);
                                        } else {
                                            set.insert(bucket);
                                        }
                                    }
                                }
                                self.set_view_edit_dirty();
                                self.refresh_view_edit_preview();
                            }
                        }
                        if let Some(state) = &mut self.view_edit_state {
                            state.overlay = None;
                            state.picker_index = 0;
                        }
                        self.status = Self::view_edit_default_status();
                    }
                    _ => {}
                }
            }
            None => {}
        }
        Ok(true)
    }
}
