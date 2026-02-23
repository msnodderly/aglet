use crate::*;

impl App {
    fn cycle_view_board_display_mode(mode: BoardDisplayMode) -> BoardDisplayMode {
        match mode {
            BoardDisplayMode::SingleLine => BoardDisplayMode::MultiLine,
            BoardDisplayMode::MultiLine => BoardDisplayMode::SingleLine,
        }
    }

    fn cycle_section_board_display_mode_override(
        current: Option<BoardDisplayMode>,
    ) -> Option<BoardDisplayMode> {
        match current {
            None => Some(BoardDisplayMode::SingleLine),
            Some(BoardDisplayMode::SingleLine) => Some(BoardDisplayMode::MultiLine),
            Some(BoardDisplayMode::MultiLine) => None,
        }
    }

    /// Open the unified ViewEdit screen for `view`.
    pub(crate) fn open_view_edit(&mut self, view: View) {
        let preview_count = self.preview_count_for_query(&view.criteria);
        self.view_edit_state = Some(ViewEditState {
            draft: view,
            region: ViewEditRegion::Criteria,
            criteria_index: 0,
            section_index: 0,
            section_expanded: None,
            overlay: None,
            inline_input: None,
            inline_buf: text_buffer::TextBuffer::empty(),
            picker_index: 0,
            preview_count,
        });
        self.mode = Mode::ViewEdit;
        self.status = "Edit view: Tab=region  S=save  Esc=cancel".to_string();
    }

    /// Recompute `preview_count` in `view_edit_state` from the current draft criteria.
    fn refresh_view_edit_preview(&mut self) {
        if let Some(state) = &self.view_edit_state {
            let count = self.preview_count_for_query(&state.draft.criteria);
            if let Some(state) = &mut self.view_edit_state {
                state.preview_count = count;
            }
        }
    }

    fn close_view_edit_overlay(&mut self) {
        if let Some(state) = &mut self.view_edit_state {
            state.overlay = None;
            state.picker_index = 0;
        }
        self.status = "Edit view: Tab=region  S=save  Esc=cancel".to_string();
    }

    fn toggle_category_picker_selection(
        &mut self,
        target: CategoryEditTarget,
        section_expanded: usize,
        cat_id: CategoryId,
    ) {
        if let Some(state) = &mut self.view_edit_state {
            match target {
                CategoryEditTarget::ViewCriteria => {
                    if state.draft.criteria.mode_for(cat_id).is_some() {
                        state.draft.criteria.remove_criterion(cat_id);
                    } else {
                        state
                            .draft
                            .criteria
                            .set_criterion(CriterionMode::And, cat_id);
                    }
                }
                CategoryEditTarget::SectionCriteria => {
                    if let Some(section) = state.draft.sections.get_mut(section_expanded) {
                        if section.criteria.mode_for(cat_id).is_some() {
                            section.criteria.remove_criterion(cat_id);
                        } else {
                            section.criteria.set_criterion(CriterionMode::And, cat_id);
                        }
                    }
                }
                CategoryEditTarget::SectionColumns => {
                    if let Some(section) = state.draft.sections.get_mut(section_expanded) {
                        if let Some(existing_index) =
                            section.columns.iter().position(|col| col.heading == cat_id)
                        {
                            section.columns.remove(existing_index);
                        } else {
                            section.columns.push(Column {
                                kind: ColumnKind::Standard,
                                heading: cat_id,
                                width: 12,
                            });
                        }
                    }
                }
                CategoryEditTarget::SectionOnInsertAssign => {
                    if let Some(section) = state.draft.sections.get_mut(section_expanded) {
                        if !section.on_insert_assign.remove(&cat_id) {
                            section.on_insert_assign.insert(cat_id);
                        }
                    }
                }
                CategoryEditTarget::SectionOnRemoveUnassign => {
                    if let Some(section) = state.draft.sections.get_mut(section_expanded) {
                        if !section.on_remove_unassign.remove(&cat_id) {
                            section.on_remove_unassign.insert(cat_id);
                        }
                    }
                }
            }
        }
        self.refresh_view_edit_preview();
    }

    pub(crate) fn handle_view_edit_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> Result<bool, String> {
        // Layer 1: inline text input intercepts all keys.
        if self
            .view_edit_state
            .as_ref()
            .map(|s| s.inline_input.is_some())
            .unwrap_or(false)
        {
            self.handle_view_edit_inline_key(code)?;
            return Ok(false);
        }

        // Layer 2: picker overlay intercepts all keys.
        if self
            .view_edit_state
            .as_ref()
            .map(|s| s.overlay.is_some())
            .unwrap_or(false)
        {
            self.handle_view_edit_overlay_key(code)?;
            return Ok(false);
        }

        // Layer 3: global and region keys.
        self.handle_view_edit_region_key(code, agenda)?;
        Ok(false)
    }

    // -------------------------------------------------------------------------
    // Layer 1: inline text input
    // -------------------------------------------------------------------------

    fn handle_view_edit_inline_key(&mut self, code: KeyCode) -> Result<bool, String> {
        let Some(state) = &self.view_edit_state else {
            return Ok(false);
        };
        let inline = state.inline_input.clone();
        match code {
            KeyCode::Esc => {
                if let Some(state) = &mut self.view_edit_state {
                    state.inline_input = None;
                    state.inline_buf.clear();
                }
                self.status = "Edit view: Tab=region  S=save  Esc=cancel".to_string();
            }
            KeyCode::Enter => {
                let Some(state) = &mut self.view_edit_state else {
                    return Ok(false);
                };
                let text = state.inline_buf.trimmed().to_string();
                match &inline {
                    Some(ViewEditInlineInput::SectionTitle { section_index }) => {
                        if let Some(section) = state.draft.sections.get_mut(*section_index) {
                            section.title = text;
                        }
                    }
                    Some(ViewEditInlineInput::UnmatchedLabel) => {
                        state.draft.unmatched_label = text;
                    }
                    None => {}
                }
                state.inline_input = None;
                state.inline_buf.clear();
                self.status = "Edit view: Tab=region  S=save  Esc=cancel".to_string();
            }
            _ => {
                if let Some(state) = &mut self.view_edit_state {
                    state.inline_buf.handle_key(code, false);
                }
            }
        }
        Ok(true)
    }

    // -------------------------------------------------------------------------
    // Layer 2: picker overlay
    // -------------------------------------------------------------------------

    fn handle_view_edit_overlay_key(&mut self, code: KeyCode) -> Result<bool, String> {
        let Some(state) = &self.view_edit_state else {
            return Ok(false);
        };
        let overlay = state.overlay.clone();
        let picker_index = state.picker_index;
        let section_expanded = state.section_expanded.unwrap_or(0);

        match overlay {
            Some(ViewEditOverlay::CategoryPicker { target }) => match code {
                KeyCode::Char('j') | KeyCode::Down => {
                    if let Some(state) = &mut self.view_edit_state {
                        state.picker_index =
                            next_index_clamped(picker_index, self.category_rows.len(), 1);
                    }
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    if let Some(state) = &mut self.view_edit_state {
                        state.picker_index =
                            next_index_clamped(picker_index, self.category_rows.len(), -1);
                    }
                }
                KeyCode::Char(' ') | KeyCode::Enter => {
                    if let Some(row) = self.category_rows.get(picker_index).cloned() {
                        self.toggle_category_picker_selection(target, section_expanded, row.id);
                    }
                }
                KeyCode::Esc => {
                    self.close_view_edit_overlay();
                }
                _ => {}
            },
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
                                self.refresh_view_edit_preview();
                            }
                        }
                        if let Some(state) = &mut self.view_edit_state {
                            state.overlay = None;
                            state.picker_index = 0;
                        }
                        self.status = "Edit view: Tab=region  S=save  Esc=cancel".to_string();
                    }
                    _ => {}
                }
            }
            None => {}
        }
        Ok(true)
    }

    // -------------------------------------------------------------------------
    // Layer 3: region-level keys
    // -------------------------------------------------------------------------

    fn handle_view_edit_region_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> Result<bool, String> {
        // Global keys first
        match code {
            KeyCode::Esc => {
                self.view_edit_state = None;
                self.mode = Mode::ViewPicker;
                self.status = "View edit canceled".to_string();
                return Ok(true);
            }
            KeyCode::Tab => {
                if let Some(state) = &mut self.view_edit_state {
                    state.region = state.region.next();
                }
                return Ok(true);
            }
            KeyCode::BackTab => {
                if let Some(state) = &mut self.view_edit_state {
                    state.region = state.region.prev();
                }
                return Ok(true);
            }
            KeyCode::Char('S') => {
                return self.handle_view_edit_save(agenda);
            }
            _ => {}
        }

        // Region-specific keys
        let region = self
            .view_edit_state
            .as_ref()
            .map(|s| s.region)
            .unwrap_or(ViewEditRegion::Criteria);

        match region {
            ViewEditRegion::Criteria => self.handle_view_edit_criteria_key(code),
            ViewEditRegion::Sections => self.handle_view_edit_sections_key(code),
            ViewEditRegion::Unmatched => self.handle_view_edit_unmatched_key(code),
        }
    }

    fn handle_view_edit_save(&mut self, agenda: &Agenda<'_>) -> Result<bool, String> {
        let Some(draft) = self.view_edit_state.as_ref().map(|s| s.draft.clone()) else {
            self.status = "View edit failed: no draft".to_string();
            return Ok(false);
        };
        let view_name = draft.name.clone();
        match agenda.store().update_view(&draft) {
            Ok(()) => {
                self.refresh(agenda.store())?;
                self.set_view_selection_by_name(&view_name);
                self.reset_section_filters();
                self.view_edit_state = None;
                self.mode = Mode::ViewPicker;
                self.status = format!("Saved view \"{view_name}\"");
            }
            Err(err) => {
                self.status = format!("View save failed: {err}");
            }
        }
        Ok(true)
    }

    // -------------------------------------------------------------------------
    // Criteria region
    // -------------------------------------------------------------------------

    fn handle_view_edit_criteria_key(&mut self, code: KeyCode) -> Result<bool, String> {
        let Some(state) = &self.view_edit_state else {
            return Ok(false);
        };
        let len = state.draft.criteria.criteria.len();
        let idx = state.criteria_index;

        match code {
            KeyCode::Char('j') | KeyCode::Down => {
                if let Some(state) = &mut self.view_edit_state {
                    state.criteria_index = next_index_clamped(idx, len, 1);
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if let Some(state) = &mut self.view_edit_state {
                    state.criteria_index = next_index_clamped(idx, len, -1);
                }
            }
            KeyCode::Char('n') | KeyCode::Char('N') => {
                let first = first_non_reserved_category_index(&self.category_rows);
                if let Some(state) = &mut self.view_edit_state {
                    state.overlay = Some(ViewEditOverlay::CategoryPicker {
                        target: CategoryEditTarget::ViewCriteria,
                    });
                    state.picker_index = first;
                }
                self.status = "Add criteria: j/k select  Space/Enter:toggle  Esc:done".to_string();
            }
            KeyCode::Char('x') => {
                if let Some(state) = &mut self.view_edit_state {
                    if idx < state.draft.criteria.criteria.len() {
                        state.draft.criteria.criteria.remove(idx);
                        let new_len = state.draft.criteria.criteria.len();
                        if state.criteria_index >= new_len && new_len > 0 {
                            state.criteria_index = new_len - 1;
                        }
                        self.refresh_view_edit_preview();
                    }
                }
            }
            KeyCode::Char(' ') => {
                // Cycle mode: And → Not → Or → And
                if let Some(state) = &mut self.view_edit_state {
                    if let Some(criterion) = state.draft.criteria.criteria.get_mut(idx) {
                        criterion.mode = match criterion.mode {
                            CriterionMode::And => CriterionMode::Not,
                            CriterionMode::Not => CriterionMode::Or,
                            CriterionMode::Or => CriterionMode::And,
                        };
                    }
                    self.refresh_view_edit_preview();
                }
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
                }
            }
            _ => {}
        }
        Ok(true)
    }

    // -------------------------------------------------------------------------
    // Sections region
    // -------------------------------------------------------------------------

    fn handle_view_edit_sections_key(&mut self, code: KeyCode) -> Result<bool, String> {
        let Some(state) = &self.view_edit_state else {
            return Ok(false);
        };
        let len = state.draft.sections.len();
        let idx = state.section_index;

        match code {
            KeyCode::Char('j') | KeyCode::Down => {
                if let Some(state) = &mut self.view_edit_state {
                    state.section_index = next_index_clamped(idx, len, 1);
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if let Some(state) = &mut self.view_edit_state {
                    state.section_index = next_index_clamped(idx, len, -1);
                }
            }
            KeyCode::Char('n') | KeyCode::Char('N') => {
                if let Some(state) = &mut self.view_edit_state {
                    let new_section = Section {
                        title: "New section".to_string(),
                        criteria: Query::default(),
                        columns: Vec::new(),
                        item_column_index: 0,
                        on_insert_assign: HashSet::new(),
                        on_remove_unassign: HashSet::new(),
                        show_children: false,
                        board_display_mode_override: None,
                    };
                    state.draft.sections.push(new_section);
                    state.section_index = state.draft.sections.len() - 1;
                }
            }
            KeyCode::Char('x') => {
                if let Some(state) = &mut self.view_edit_state {
                    if idx < state.draft.sections.len() {
                        state.draft.sections.remove(idx);
                        let new_len = state.draft.sections.len();
                        if state.section_index >= new_len && new_len > 0 {
                            state.section_index = new_len - 1;
                        }
                        if state.section_expanded == Some(idx) {
                            state.section_expanded = None;
                        }
                    }
                }
            }
            KeyCode::Char('[') => {
                if let Some(state) = &mut self.view_edit_state {
                    if idx > 0 && idx < state.draft.sections.len() {
                        state.draft.sections.swap(idx, idx - 1);
                        state.section_index = idx - 1;
                    }
                }
            }
            KeyCode::Char(']') => {
                if let Some(state) = &mut self.view_edit_state {
                    if idx + 1 < state.draft.sections.len() {
                        state.draft.sections.swap(idx, idx + 1);
                        state.section_index = idx + 1;
                    }
                }
            }
            KeyCode::Enter => {
                if let Some(state) = &mut self.view_edit_state {
                    if idx < len {
                        if state.section_expanded == Some(idx) {
                            state.section_expanded = None;
                        } else {
                            state.section_expanded = Some(idx);
                        }
                    }
                }
            }
            KeyCode::Char('t') | KeyCode::Char('e') => {
                if let Some(state) = &mut self.view_edit_state {
                    if idx < state.draft.sections.len() {
                        let current = state.draft.sections[idx].title.clone();
                        state.inline_input =
                            Some(ViewEditInlineInput::SectionTitle { section_index: idx });
                        state.inline_buf = text_buffer::TextBuffer::new(current);
                        state.section_expanded = Some(idx);
                    }
                }
                self.status = "Section title: type text  Enter:confirm  Esc:cancel".to_string();
            }
            // Expanded section detail keys
            KeyCode::Char('f') => {
                if idx < len {
                    let first = first_non_reserved_category_index(&self.category_rows);
                    if let Some(state) = &mut self.view_edit_state {
                        state.overlay = Some(ViewEditOverlay::CategoryPicker {
                            target: CategoryEditTarget::SectionCriteria,
                        });
                        state.section_expanded = Some(idx);
                        state.picker_index = first;
                    }
                }
            }
            KeyCode::Char('a') => {
                if idx < len {
                    let first = first_non_reserved_category_index(&self.category_rows);
                    if let Some(state) = &mut self.view_edit_state {
                        state.overlay = Some(ViewEditOverlay::CategoryPicker {
                            target: CategoryEditTarget::SectionOnInsertAssign,
                        });
                        state.section_expanded = Some(idx);
                        state.picker_index = first;
                    }
                }
            }
            KeyCode::Char('c') | KeyCode::Char('C') => {
                if idx < len {
                    let first = first_non_reserved_category_index(&self.category_rows);
                    if let Some(state) = &mut self.view_edit_state {
                        state.overlay = Some(ViewEditOverlay::CategoryPicker {
                            target: CategoryEditTarget::SectionColumns,
                        });
                        state.section_expanded = Some(idx);
                        state.picker_index = first;
                    }
                }
            }
            KeyCode::Char('r') => {
                if idx < len {
                    let first = first_non_reserved_category_index(&self.category_rows);
                    if let Some(state) = &mut self.view_edit_state {
                        state.overlay = Some(ViewEditOverlay::CategoryPicker {
                            target: CategoryEditTarget::SectionOnRemoveUnassign,
                        });
                        state.section_expanded = Some(idx);
                        state.picker_index = first;
                    }
                }
            }
            KeyCode::Char('h') => {
                if idx < len {
                    if let Some(state) = &mut self.view_edit_state {
                        if let Some(section) = state.draft.sections.get_mut(idx) {
                            section.show_children = !section.show_children;
                            state.section_expanded = Some(idx);
                        }
                    }
                }
            }
            KeyCode::Char('m') | KeyCode::Char('M') => {
                if idx < len {
                    if let Some(state) = &mut self.view_edit_state {
                        if let Some(section) = state.draft.sections.get_mut(idx) {
                            section.board_display_mode_override =
                                Self::cycle_section_board_display_mode_override(
                                    section.board_display_mode_override,
                                );
                            state.section_expanded = Some(idx);
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(true)
    }

    // -------------------------------------------------------------------------
    // Unmatched region
    // -------------------------------------------------------------------------

    fn handle_view_edit_unmatched_key(&mut self, code: KeyCode) -> Result<bool, String> {
        match code {
            KeyCode::Char('t') => {
                if let Some(state) = &mut self.view_edit_state {
                    state.draft.show_unmatched = !state.draft.show_unmatched;
                }
            }
            KeyCode::Char('l') => {
                if let Some(state) = &mut self.view_edit_state {
                    let current = state.draft.unmatched_label.clone();
                    state.inline_input = Some(ViewEditInlineInput::UnmatchedLabel);
                    state.inline_buf = text_buffer::TextBuffer::new(current);
                }
                self.status = "Unmatched label: type text  Enter:confirm  Esc:cancel".to_string();
            }
            _ => {}
        }
        Ok(true)
    }
}
