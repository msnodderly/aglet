use crate::*;

impl App {
    /// Build criteria rows from a view's criteria, sorted include-then-exclude alphabetically.
    fn build_view_edit_criteria_rows(&self, view: &View) -> Vec<ViewCriteriaRow> {
        let category_names = category_name_map(&self.categories);
        let mut rows: Vec<ViewCriteriaRow> = view
            .criteria
            .include
            .iter()
            .map(|&id| ViewCriteriaRow {
                sign: ViewCriteriaSign::Include,
                category_id: id,
                join_is_or: false,
                depth: 0,
            })
            .chain(view.criteria.exclude.iter().map(|&id| ViewCriteriaRow {
                sign: ViewCriteriaSign::Exclude,
                category_id: id,
                join_is_or: false,
                depth: 0,
            }))
            .collect();
        rows.sort_by(|a, b| {
            let a_name = category_names
                .get(&a.category_id)
                .cloned()
                .unwrap_or_else(|| a.category_id.to_string());
            let b_name = category_names
                .get(&b.category_id)
                .cloned()
                .unwrap_or_else(|| b.category_id.to_string());
            let a_sign = matches!(a.sign, ViewCriteriaSign::Exclude) as u8;
            let b_sign = matches!(b.sign, ViewCriteriaSign::Exclude) as u8;
            (a_sign, a_name.to_ascii_lowercase()).cmp(&(b_sign, b_name.to_ascii_lowercase()))
        });
        rows
    }

    /// Open the unified ViewEdit screen for `view`.
    pub(crate) fn open_view_edit(&mut self, view: View) {
        let criteria_rows = self.build_view_edit_criteria_rows(&view);
        let preview_count = self.preview_count_for_query(&view.criteria);
        self.view_edit_state = Some(ViewEditState {
            draft: view,
            region: ViewEditRegion::Criteria,
            criteria_index: 0,
            column_index: 0,
            section_index: 0,
            section_expanded: None,
            overlay: None,
            inline_input: None,
            inline_buf: text_buffer::TextBuffer::empty(),
            picker_index: 0,
            preview_count,
            criteria_rows,
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
            return self.handle_view_edit_inline_key(code);
        }

        // Layer 2: picker overlay intercepts all keys.
        if self
            .view_edit_state
            .as_ref()
            .map(|s| s.overlay.is_some())
            .unwrap_or(false)
        {
            return self.handle_view_edit_overlay_key(code);
        }

        // Layer 3: global and region keys.
        self.handle_view_edit_region_key(code, agenda)
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
                    Some(ViewEditInlineInput::ColumnWidth { column_index }) => {
                        if let Ok(w) = state.inline_buf.text().trim().parse::<u16>() {
                            if let Some(col) = state.draft.columns.get_mut(*column_index) {
                                col.width = w;
                            }
                        }
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
            Some(ViewEditOverlay::CategoryPicker { target }) => {
                match code {
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
                    KeyCode::Char(' ') | KeyCode::Enter | KeyCode::Esc => {
                        let mut needs_criteria_rebuild = false;
                        if matches!(code, KeyCode::Char(' ') | KeyCode::Enter) {
                            if let Some(row) = self.category_rows.get(picker_index).cloned() {
                                let cat_id = row.id;
                                if let Some(state) = &mut self.view_edit_state {
                                    match target {
                                        CategoryEditTarget::ViewInclude => {
                                            if state.region == ViewEditRegion::Columns {
                                                let new_col = Column {
                                                    kind: ColumnKind::Standard,
                                                    heading: cat_id,
                                                    width: 12,
                                                };
                                                state.draft.columns.push(new_col);
                                                state.column_index =
                                                    state.draft.columns.len().saturating_sub(1);
                                            } else {
                                                state.draft.criteria.include.insert(cat_id);
                                                state.draft.criteria.exclude.remove(&cat_id);
                                                needs_criteria_rebuild = true;
                                            }
                                        }
                                        CategoryEditTarget::ViewExclude => {
                                            state.draft.criteria.exclude.insert(cat_id);
                                            state.draft.criteria.include.remove(&cat_id);
                                            needs_criteria_rebuild = true;
                                        }
                                        CategoryEditTarget::SectionCriteriaInclude => {
                                            if let Some(section) =
                                                state.draft.sections.get_mut(section_expanded)
                                            {
                                                section.criteria.include.insert(cat_id);
                                                section.criteria.exclude.remove(&cat_id);
                                            }
                                        }
                                        CategoryEditTarget::SectionCriteriaExclude => {
                                            if let Some(section) =
                                                state.draft.sections.get_mut(section_expanded)
                                            {
                                                section.criteria.exclude.insert(cat_id);
                                                section.criteria.include.remove(&cat_id);
                                            }
                                        }
                                        CategoryEditTarget::SectionOnInsertAssign => {
                                            if let Some(section) =
                                                state.draft.sections.get_mut(section_expanded)
                                            {
                                                section.on_insert_assign.insert(cat_id);
                                            }
                                        }
                                        CategoryEditTarget::SectionOnRemoveUnassign => {
                                            if let Some(section) =
                                                state.draft.sections.get_mut(section_expanded)
                                            {
                                                section.on_remove_unassign.insert(cat_id);
                                            }
                                        }
                                    }
                                }
                                // Rebuild criteria_rows outside the mutable borrow scope
                                if needs_criteria_rebuild {
                                    if let Some(state) = &self.view_edit_state {
                                        let draft = state.draft.clone();
                                        let rows = self.build_view_edit_criteria_rows(&draft);
                                        if let Some(state) = &mut self.view_edit_state {
                                            state.criteria_rows = rows;
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
            Some(ViewEditOverlay::BucketPicker { target }) => {
                let options = when_bucket_options();
                match code {
                    KeyCode::Char('j') | KeyCode::Down => {
                        if let Some(state) = &mut self.view_edit_state {
                            state.picker_index =
                                next_index_clamped(picker_index, options.len(), 1);
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
                                    if let Some(set) = bucket_target_set_mut(
                                        &mut state.draft,
                                        section_expanded,
                                        target,
                                    ) {
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
            ViewEditRegion::Columns => self.handle_view_edit_columns_key(code),
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
                // Reopen editor with updated view so user can continue editing
                if let Some(updated_view) =
                    self.views.iter().find(|v| v.name == view_name).cloned()
                {
                    self.open_view_edit(updated_view);
                }
                self.status = format!("Saved view \"{view_name}\" (Esc to close)");
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
        let len = state.criteria_rows.len();
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
            KeyCode::Char('N') => {
                let first = first_non_reserved_category_index(&self.category_rows);
                if let Some(state) = &mut self.view_edit_state {
                    state.overlay = Some(ViewEditOverlay::CategoryPicker {
                        target: CategoryEditTarget::ViewInclude,
                    });
                    state.picker_index = first;
                }
                self.status =
                    "Add criteria: j/k select  Space/Enter:add  Esc:back".to_string();
            }
            KeyCode::Char('x') => {
                if let Some(state) = &mut self.view_edit_state {
                    if idx < state.criteria_rows.len() {
                        let removed = state.criteria_rows.remove(idx);
                        match removed.sign {
                            ViewCriteriaSign::Include => {
                                state.draft.criteria.include.remove(&removed.category_id);
                            }
                            ViewCriteriaSign::Exclude => {
                                state.draft.criteria.exclude.remove(&removed.category_id);
                            }
                        }
                        let new_len = state.criteria_rows.len();
                        if state.criteria_index >= new_len && new_len > 0 {
                            state.criteria_index = new_len - 1;
                        }
                        self.refresh_view_edit_preview();
                    }
                }
            }
            KeyCode::Char(' ') => {
                // Toggle include/exclude on selected row
                if let Some(state) = &mut self.view_edit_state {
                    if let Some(row) = state.criteria_rows.get_mut(idx) {
                        let id = row.category_id;
                        match row.sign {
                            ViewCriteriaSign::Include => {
                                row.sign = ViewCriteriaSign::Exclude;
                                state.draft.criteria.include.remove(&id);
                                state.draft.criteria.exclude.insert(id);
                            }
                            ViewCriteriaSign::Exclude => {
                                row.sign = ViewCriteriaSign::Include;
                                state.draft.criteria.exclude.remove(&id);
                                state.draft.criteria.include.insert(id);
                            }
                        }
                    }
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
            _ => {}
        }
        Ok(true)
    }

    // -------------------------------------------------------------------------
    // Columns region
    // -------------------------------------------------------------------------

    fn handle_view_edit_columns_key(&mut self, code: KeyCode) -> Result<bool, String> {
        let Some(state) = &self.view_edit_state else {
            return Ok(false);
        };
        let len = state.draft.columns.len();
        let idx = state.column_index;

        match code {
            KeyCode::Char('j') | KeyCode::Down => {
                if let Some(state) = &mut self.view_edit_state {
                    state.column_index = next_index_clamped(idx, len, 1);
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if let Some(state) = &mut self.view_edit_state {
                    state.column_index = next_index_clamped(idx, len, -1);
                }
            }
            KeyCode::Char('N') => {
                let first = first_non_reserved_category_index(&self.category_rows);
                if let Some(state) = &mut self.view_edit_state {
                    state.overlay = Some(ViewEditOverlay::CategoryPicker {
                        target: CategoryEditTarget::ViewInclude,
                    });
                    state.picker_index = first;
                }
                self.status = "Add column: j/k select  Space/Enter:add  Esc:back".to_string();
            }
            KeyCode::Char('x') => {
                if let Some(state) = &mut self.view_edit_state {
                    if idx < state.draft.columns.len() {
                        state.draft.columns.remove(idx);
                        let new_len = state.draft.columns.len();
                        if state.column_index >= new_len && new_len > 0 {
                            state.column_index = new_len - 1;
                        }
                    }
                }
            }
            KeyCode::Char('[') => {
                if let Some(state) = &mut self.view_edit_state {
                    if idx > 0 && idx < state.draft.columns.len() {
                        state.draft.columns.swap(idx, idx - 1);
                        state.column_index = idx - 1;
                    }
                }
            }
            KeyCode::Char(']') => {
                if let Some(state) = &mut self.view_edit_state {
                    if idx + 1 < state.draft.columns.len() {
                        state.draft.columns.swap(idx, idx + 1);
                        state.column_index = idx + 1;
                    }
                }
            }
            KeyCode::Char('w') => {
                if let Some(state) = &mut self.view_edit_state {
                    if idx < state.draft.columns.len() {
                        let current_width = state.draft.columns[idx].width.to_string();
                        state.inline_input =
                            Some(ViewEditInlineInput::ColumnWidth { column_index: idx });
                        state.inline_buf = text_buffer::TextBuffer::new(current_width);
                    }
                }
                self.status = "Column width: type number  Enter:confirm  Esc:cancel".to_string();
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
        let expanded = state.section_expanded;

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
            KeyCode::Enter => {
                if let Some(state) = &mut self.view_edit_state {
                    state.section_expanded = if expanded == Some(idx) {
                        None
                    } else if idx < state.draft.sections.len() {
                        Some(idx)
                    } else {
                        None
                    };
                }
            }
            KeyCode::Char('N') => {
                if let Some(state) = &mut self.view_edit_state {
                    let new_section = Section {
                        title: "New section".to_string(),
                        criteria: Query::default(),
                        on_insert_assign: HashSet::new(),
                        on_remove_unassign: HashSet::new(),
                        show_children: false,
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
                        if state.section_expanded == Some(idx) {
                            state.section_expanded = Some(idx - 1);
                        }
                    }
                }
            }
            KeyCode::Char(']') => {
                if let Some(state) = &mut self.view_edit_state {
                    if idx + 1 < state.draft.sections.len() {
                        state.draft.sections.swap(idx, idx + 1);
                        state.section_index = idx + 1;
                        if state.section_expanded == Some(idx) {
                            state.section_expanded = Some(idx + 1);
                        }
                    }
                }
            }
            KeyCode::Char('t') => {
                if let Some(state) = &mut self.view_edit_state {
                    if idx < state.draft.sections.len() {
                        let current = state.draft.sections[idx].title.clone();
                        state.inline_input =
                            Some(ViewEditInlineInput::SectionTitle { section_index: idx });
                        state.inline_buf = text_buffer::TextBuffer::new(current);
                    }
                }
                self.status =
                    "Section title: type text  Enter:confirm  Esc:cancel".to_string();
            }
            // Expanded section detail keys
            KeyCode::Char('+') => {
                if let Some(exp) = expanded {
                    let first = first_non_reserved_category_index(&self.category_rows);
                    if let Some(state) = &mut self.view_edit_state {
                        state.overlay = Some(ViewEditOverlay::CategoryPicker {
                            target: CategoryEditTarget::SectionCriteriaInclude,
                        });
                        state.section_expanded = Some(exp);
                        state.picker_index = first;
                    }
                }
            }
            KeyCode::Char('-') => {
                if let Some(exp) = expanded {
                    let first = first_non_reserved_category_index(&self.category_rows);
                    if let Some(state) = &mut self.view_edit_state {
                        state.overlay = Some(ViewEditOverlay::CategoryPicker {
                            target: CategoryEditTarget::SectionCriteriaExclude,
                        });
                        state.section_expanded = Some(exp);
                        state.picker_index = first;
                    }
                }
            }
            KeyCode::Char('a') => {
                if let Some(exp) = expanded {
                    let first = first_non_reserved_category_index(&self.category_rows);
                    if let Some(state) = &mut self.view_edit_state {
                        state.overlay = Some(ViewEditOverlay::CategoryPicker {
                            target: CategoryEditTarget::SectionOnInsertAssign,
                        });
                        state.section_expanded = Some(exp);
                        state.picker_index = first;
                    }
                }
            }
            KeyCode::Char('r') => {
                if let Some(exp) = expanded {
                    let first = first_non_reserved_category_index(&self.category_rows);
                    if let Some(state) = &mut self.view_edit_state {
                        state.overlay = Some(ViewEditOverlay::CategoryPicker {
                            target: CategoryEditTarget::SectionOnRemoveUnassign,
                        });
                        state.section_expanded = Some(exp);
                        state.picker_index = first;
                    }
                }
            }
            KeyCode::Char('h') => {
                if let Some(exp) = expanded {
                    if let Some(state) = &mut self.view_edit_state {
                        if let Some(section) = state.draft.sections.get_mut(exp) {
                            section.show_children = !section.show_children;
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
                self.status =
                    "Unmatched label: type text  Enter:confirm  Esc:cancel".to_string();
            }
            _ => {}
        }
        Ok(true)
    }
}
