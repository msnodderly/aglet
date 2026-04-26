use crate::*;

impl App {
    pub(crate) fn view_details_criteria_row_count(state: &ViewEditState) -> usize {
        state.draft.criteria.criteria.len().max(1)
    }

    pub(crate) fn view_details_aux_field_count() -> usize {
        9
    }

    pub(crate) fn view_edit_showing_view_details(state: &ViewEditState) -> bool {
        state.region != ViewEditRegion::Sections
            || state.sections_view_row_selected
            || state.draft.sections.get(state.section_index).is_none()
            || state.region == ViewEditRegion::Datebook
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

    /// The Name row sits at flat index 0 before the criteria rows.
    const NAME_ROW_COUNT: usize = 1;

    pub(crate) fn scope_row_next(state: &ViewEditState) -> Option<ScopeRow> {
        let n_criteria = state.draft.criteria.criteria.len();
        let has_datebook = state.draft.datebook_config.is_some();
        let after_criteria = if has_datebook {
            ScopeRow::Datebook(DatebookField::Period)
        } else {
            ScopeRow::DateInclude
        };
        Some(match state.scope_row {
            ScopeRow::Name => ScopeRow::ViewType,
            // Always include at least one Criterion row (placeholder when empty).
            ScopeRow::ViewType => ScopeRow::Criterion(0),
            ScopeRow::Criterion(i) => {
                if i + 1 < n_criteria {
                    ScopeRow::Criterion(i + 1)
                } else {
                    after_criteria
                }
            }
            ScopeRow::Datebook(DatebookField::Period) => {
                ScopeRow::Datebook(DatebookField::Interval)
            }
            ScopeRow::Datebook(DatebookField::Interval) => {
                ScopeRow::Datebook(DatebookField::Anchor)
            }
            ScopeRow::Datebook(DatebookField::Anchor) => {
                ScopeRow::Datebook(DatebookField::DateSource)
            }
            ScopeRow::Datebook(DatebookField::DateSource) => ScopeRow::DateInclude,
            ScopeRow::DateInclude => ScopeRow::DateExclude,
            ScopeRow::DateExclude => ScopeRow::HideDependent,
            ScopeRow::HideDependent => return None,
        })
    }

    pub(crate) fn scope_row_prev(state: &ViewEditState) -> Option<ScopeRow> {
        let n_criteria = state.draft.criteria.criteria.len();
        let has_datebook = state.draft.datebook_config.is_some();
        let last_criterion = ScopeRow::Criterion(n_criteria.saturating_sub(1));
        let before_date_include = if has_datebook {
            ScopeRow::Datebook(DatebookField::DateSource)
        } else {
            last_criterion
        };
        Some(match state.scope_row {
            ScopeRow::Name => return None,
            ScopeRow::ViewType => ScopeRow::Name,
            ScopeRow::Criterion(0) => ScopeRow::ViewType,
            ScopeRow::Criterion(i) => ScopeRow::Criterion(i - 1),
            ScopeRow::Datebook(DatebookField::Period) => last_criterion,
            ScopeRow::Datebook(DatebookField::Interval) => {
                ScopeRow::Datebook(DatebookField::Period)
            }
            ScopeRow::Datebook(DatebookField::Anchor) => {
                ScopeRow::Datebook(DatebookField::Interval)
            }
            ScopeRow::Datebook(DatebookField::DateSource) => {
                ScopeRow::Datebook(DatebookField::Anchor)
            }
            ScopeRow::DateInclude => before_date_include,
            ScopeRow::DateExclude => ScopeRow::DateInclude,
            ScopeRow::HideDependent => ScopeRow::DateExclude,
        })
    }

    /// Bring legacy state fields (region/criteria_index/datebook_field_index/
    /// unmatched_field_index/name_focused) in line with `state.scope_row`.
    /// Used during the migration so legacy code paths stay coherent.
    pub(crate) fn sync_legacy_from_scope_row(state: &mut ViewEditState) {
        let n_criteria = state.draft.criteria.criteria.len();
        match state.scope_row {
            ScopeRow::Name => {
                state.region = ViewEditRegion::Criteria;
                state.criteria_index = 0;
                state.name_focused = true;
            }
            ScopeRow::ViewType => {
                state.region = ViewEditRegion::Criteria;
                state.criteria_index = 0;
                state.name_focused = false;
            }
            ScopeRow::Criterion(i) => {
                state.region = ViewEditRegion::Criteria;
                state.criteria_index = i.min(n_criteria.saturating_sub(1));
                state.name_focused = false;
            }
            ScopeRow::Datebook(field) => {
                state.region = ViewEditRegion::Datebook;
                state.datebook_field_index = field.index();
                state.name_focused = false;
            }
            ScopeRow::DateInclude => {
                state.region = ViewEditRegion::Unmatched;
                state.unmatched_field_index = 0;
                state.name_focused = false;
            }
            ScopeRow::DateExclude => {
                state.region = ViewEditRegion::Unmatched;
                state.unmatched_field_index = 1;
                state.name_focused = false;
            }
            ScopeRow::HideDependent => {
                state.region = ViewEditRegion::Unmatched;
                state.unmatched_field_index = 6;
                state.name_focused = false;
            }
        }
    }

    /// Toggle View type between Board (no datebook config) and Datebook
    /// (with default DatebookConfig). Used by the Scope tab's ViewType row.
    pub(crate) fn toggle_view_edit_view_type(&mut self) {
        if let Some(state) = &mut self.view_edit_state {
            if state.draft.datebook_config.is_some() {
                state.draft.datebook_config = None;
            } else {
                state.draft.datebook_config = Some(DatebookConfig::default());
            }
            state.dirty = true;
            state.discard_confirm = false;
        }
        self.refresh_view_edit_preview();
    }

    pub(crate) fn handle_view_edit_scope_key(&mut self, code: KeyCode) -> TuiResult<bool> {
        let Some(state) = &self.view_edit_state else {
            return Ok(false);
        };
        let scope_row = state.scope_row;

        match code {
            KeyCode::Char('j') | KeyCode::Down => {
                if let Some(state) = &self.view_edit_state {
                    if let Some(next) = Self::scope_row_next(state) {
                        if let Some(state) = &mut self.view_edit_state {
                            state.scope_row = next;
                            Self::sync_legacy_from_scope_row(state);
                        }
                    }
                }
                return Ok(true);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if let Some(state) = &self.view_edit_state {
                    if let Some(prev) = Self::scope_row_prev(state) {
                        if let Some(state) = &mut self.view_edit_state {
                            state.scope_row = prev;
                            Self::sync_legacy_from_scope_row(state);
                        }
                    }
                }
                return Ok(true);
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                if let Some(state) = &mut self.view_edit_state {
                    state.scope_row = ScopeRow::Name;
                    Self::sync_legacy_from_scope_row(state);
                }
                self.begin_view_edit_name_input();
                return Ok(true);
            }
            KeyCode::Char('n') | KeyCode::Char('N') => {
                if let Some(state) = &mut self.view_edit_state {
                    if !matches!(state.scope_row, ScopeRow::Criterion(_)) {
                        state.scope_row = if state.draft.criteria.criteria.is_empty() {
                            ScopeRow::Criterion(0)
                        } else {
                            ScopeRow::Criterion(state.draft.criteria.criteria.len() - 1)
                        };
                        Self::sync_legacy_from_scope_row(state);
                    }
                }
                self.open_view_edit_view_criteria_picker();
                return Ok(true);
            }
            KeyCode::Char('x') => {
                if let ScopeRow::Criterion(i) = scope_row {
                    let mut changed = false;
                    if let Some(state) = &mut self.view_edit_state {
                        if i < state.draft.criteria.criteria.len() {
                            state.draft.criteria.criteria.remove(i);
                            let new_len = state.draft.criteria.criteria.len();
                            if new_len == 0 {
                                state.scope_row = ScopeRow::ViewType;
                            } else if i >= new_len {
                                state.scope_row = ScopeRow::Criterion(new_len - 1);
                            }
                            Self::sync_legacy_from_scope_row(state);
                            changed = true;
                        }
                    }
                    if changed {
                        self.set_view_edit_dirty();
                        self.refresh_view_edit_preview();
                    }
                }
                return Ok(true);
            }
            KeyCode::Char('d') | KeyCode::Char('D') => {
                if let Some(state) = &mut self.view_edit_state {
                    state.scope_row = ScopeRow::HideDependent;
                    Self::sync_legacy_from_scope_row(state);
                    state.draft.hide_dependent_items = !state.draft.hide_dependent_items;
                    state.dirty = true;
                    state.discard_confirm = false;
                }
                return Ok(true);
            }
            KeyCode::Char(']') => {
                if let Some(state) = &mut self.view_edit_state {
                    state.scope_row = ScopeRow::DateInclude;
                    Self::sync_legacy_from_scope_row(state);
                    state.overlay = Some(ViewEditOverlay::BucketPicker {
                        target: BucketEditTarget::ViewVirtualInclude,
                    });
                    state.picker_index = 0;
                }
                return Ok(true);
            }
            KeyCode::Char('[') => {
                if let Some(state) = &mut self.view_edit_state {
                    state.scope_row = ScopeRow::DateExclude;
                    Self::sync_legacy_from_scope_row(state);
                    state.overlay = Some(ViewEditOverlay::BucketPicker {
                        target: BucketEditTarget::ViewVirtualExclude,
                    });
                    state.picker_index = 0;
                }
                return Ok(true);
            }
            KeyCode::Char(' ') | KeyCode::Enter => {
                let is_enter = matches!(code, KeyCode::Enter);
                match scope_row {
                    ScopeRow::Name => {
                        self.begin_view_edit_name_input();
                    }
                    ScopeRow::ViewType => {
                        self.toggle_view_edit_view_type();
                    }
                    ScopeRow::Criterion(_) => {
                        // Enter always opens the picker; Space cycles the
                        // mode on an existing criterion or opens the picker
                        // when the row is the empty placeholder.
                        if is_enter {
                            self.open_view_edit_view_criteria_picker();
                        } else {
                            let mut cycled = false;
                            if let Some(state) = &mut self.view_edit_state {
                                let idx = match state.scope_row {
                                    ScopeRow::Criterion(i) => i,
                                    _ => 0,
                                };
                                if let Some(criterion) = state.draft.criteria.criteria.get_mut(idx)
                                {
                                    criterion.mode = match criterion.mode {
                                        CriterionMode::And => CriterionMode::Not,
                                        CriterionMode::Not => CriterionMode::Or,
                                        CriterionMode::Or => CriterionMode::And,
                                    };
                                    cycled = true;
                                }
                            }
                            if cycled {
                                self.set_view_edit_dirty();
                                self.refresh_view_edit_preview();
                            } else {
                                self.open_view_edit_view_criteria_picker();
                            }
                        }
                    }
                    ScopeRow::Datebook(field) => {
                        if let Some(state) = &mut self.view_edit_state {
                            if let Some(config) = &mut state.draft.datebook_config {
                                match field {
                                    DatebookField::Period => config.period = config.period.next(),
                                    DatebookField::Interval => {
                                        config.interval = config.interval.next()
                                    }
                                    DatebookField::Anchor => config.anchor = config.anchor.next(),
                                    DatebookField::DateSource => {
                                        config.date_source = config.date_source.next()
                                    }
                                }
                                while !config.is_valid() {
                                    config.interval = config.interval.next();
                                }
                                state.dirty = true;
                                state.discard_confirm = false;
                            }
                        }
                        self.refresh_view_edit_preview();
                    }
                    ScopeRow::DateInclude => {
                        if let Some(state) = &mut self.view_edit_state {
                            state.overlay = Some(ViewEditOverlay::BucketPicker {
                                target: BucketEditTarget::ViewVirtualInclude,
                            });
                            state.picker_index = 0;
                        }
                    }
                    ScopeRow::DateExclude => {
                        if let Some(state) = &mut self.view_edit_state {
                            state.overlay = Some(ViewEditOverlay::BucketPicker {
                                target: BucketEditTarget::ViewVirtualExclude,
                            });
                            state.picker_index = 0;
                        }
                    }
                    ScopeRow::HideDependent => {
                        if let Some(state) = &mut self.view_edit_state {
                            state.draft.hide_dependent_items = !state.draft.hide_dependent_items;
                            state.dirty = true;
                            state.discard_confirm = false;
                        }
                    }
                }
                return Ok(true);
            }
            _ => {}
        }
        Ok(true)
    }

    pub(crate) fn appearance_row_next(current: AppearanceRow) -> Option<AppearanceRow> {
        Some(match current {
            AppearanceRow::DisplayMode => AppearanceRow::SectionFlow,
            AppearanceRow::SectionFlow => AppearanceRow::EmptySections,
            AppearanceRow::EmptySections => AppearanceRow::Aliases,
            AppearanceRow::Aliases => return None,
        })
    }

    pub(crate) fn appearance_row_prev(current: AppearanceRow) -> Option<AppearanceRow> {
        Some(match current {
            AppearanceRow::DisplayMode => return None,
            AppearanceRow::SectionFlow => AppearanceRow::DisplayMode,
            AppearanceRow::EmptySections => AppearanceRow::SectionFlow,
            AppearanceRow::Aliases => AppearanceRow::EmptySections,
        })
    }

    /// Bring legacy `unmatched_field_index` in line with `state.appearance_row`.
    /// Used during the migration so legacy code paths stay coherent.
    pub(crate) fn sync_legacy_from_appearance_row(state: &mut ViewEditState) {
        state.region = ViewEditRegion::Unmatched;
        state.name_focused = false;
        state.unmatched_field_index = match state.appearance_row {
            AppearanceRow::DisplayMode => 2,
            AppearanceRow::SectionFlow => 3,
            AppearanceRow::EmptySections => 4,
            AppearanceRow::Aliases => 8,
        };
    }

    pub(crate) fn handle_view_edit_appearance_key(&mut self, code: KeyCode) -> TuiResult<bool> {
        let Some(state) = &self.view_edit_state else {
            return Ok(false);
        };
        let appearance_row = state.appearance_row;

        match code {
            KeyCode::Char('j') | KeyCode::Down => {
                if let Some(next) = Self::appearance_row_next(appearance_row) {
                    if let Some(state) = &mut self.view_edit_state {
                        state.appearance_row = next;
                        Self::sync_legacy_from_appearance_row(state);
                    }
                }
                return Ok(true);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if let Some(prev) = Self::appearance_row_prev(appearance_row) {
                    if let Some(state) = &mut self.view_edit_state {
                        state.appearance_row = prev;
                        Self::sync_legacy_from_appearance_row(state);
                    }
                }
                return Ok(true);
            }
            KeyCode::Char('m') | KeyCode::Char('M') => {
                if let Some(state) = &mut self.view_edit_state {
                    state.appearance_row = AppearanceRow::DisplayMode;
                    Self::sync_legacy_from_appearance_row(state);
                    state.draft.board_display_mode = match state.draft.board_display_mode {
                        BoardDisplayMode::SingleLine => BoardDisplayMode::MultiLine,
                        BoardDisplayMode::MultiLine => BoardDisplayMode::SingleLine,
                    };
                    state.dirty = true;
                    state.discard_confirm = false;
                }
                return Ok(true);
            }
            KeyCode::Char('w') | KeyCode::Char('W') => {
                if let Some(state) = &mut self.view_edit_state {
                    state.appearance_row = AppearanceRow::SectionFlow;
                    Self::sync_legacy_from_appearance_row(state);
                    state.draft.section_flow =
                        Self::cycle_view_section_flow(state.draft.section_flow);
                    state.dirty = true;
                    state.discard_confirm = false;
                }
                return Ok(true);
            }
            KeyCode::Char('e') | KeyCode::Char('E') => {
                if let Some(state) = &mut self.view_edit_state {
                    state.appearance_row = AppearanceRow::EmptySections;
                    Self::sync_legacy_from_appearance_row(state);
                    state.draft.empty_sections = state.draft.empty_sections.next();
                    state.dirty = true;
                    state.discard_confirm = false;
                }
                return Ok(true);
            }
            KeyCode::Char('a') | KeyCode::Char('A') => {
                self.open_view_edit_alias_picker();
                return Ok(true);
            }
            KeyCode::Char(' ') | KeyCode::Enter => {
                match appearance_row {
                    AppearanceRow::DisplayMode => {
                        if let Some(state) = &mut self.view_edit_state {
                            state.draft.board_display_mode = match state.draft.board_display_mode {
                                BoardDisplayMode::SingleLine => BoardDisplayMode::MultiLine,
                                BoardDisplayMode::MultiLine => BoardDisplayMode::SingleLine,
                            };
                            state.dirty = true;
                            state.discard_confirm = false;
                        }
                    }
                    AppearanceRow::SectionFlow => {
                        if let Some(state) = &mut self.view_edit_state {
                            state.draft.section_flow =
                                Self::cycle_view_section_flow(state.draft.section_flow);
                            state.dirty = true;
                            state.discard_confirm = false;
                        }
                    }
                    AppearanceRow::EmptySections => {
                        if let Some(state) = &mut self.view_edit_state {
                            state.draft.empty_sections = state.draft.empty_sections.next();
                            state.dirty = true;
                            state.discard_confirm = false;
                        }
                    }
                    AppearanceRow::Aliases => {
                        self.open_view_edit_alias_picker();
                    }
                }
                return Ok(true);
            }
            _ => {}
        }
        Ok(true)
    }

    /// Check if the current flat focus index is the Name row.
    pub(crate) fn view_details_on_name_row(state: &ViewEditState) -> bool {
        state.name_focused
    }

    fn view_details_focus_index(state: &ViewEditState) -> usize {
        if state.name_focused {
            return 0;
        }
        let name_rows = Self::NAME_ROW_COUNT;
        let criteria_rows = Self::view_details_criteria_row_count(state);
        let datebook_rows = if state.draft.datebook_config.is_some() {
            Self::DATEBOOK_FIELD_COUNT
        } else {
            0
        };
        match state.region {
            ViewEditRegion::Criteria => {
                name_rows + state.criteria_index.min(criteria_rows.saturating_sub(1))
            }
            ViewEditRegion::Datebook => {
                name_rows
                    + criteria_rows
                    + state
                        .datebook_field_index
                        .min(datebook_rows.saturating_sub(1))
            }
            ViewEditRegion::Unmatched => {
                name_rows
                    + criteria_rows
                    + datebook_rows
                    + state
                        .unmatched_field_index
                        .min(Self::view_details_aux_field_count() - 1)
            }
            ViewEditRegion::Sections => 0,
        }
    }

    fn set_view_details_focus_index(&mut self, new_index: usize) {
        if let Some(state) = &mut self.view_edit_state {
            let name_rows = Self::NAME_ROW_COUNT;
            let criteria_rows = Self::view_details_criteria_row_count(state);
            let datebook_rows = if state.draft.datebook_config.is_some() {
                Self::DATEBOOK_FIELD_COUNT
            } else {
                0
            };
            if new_index < name_rows {
                // Name row: focus it (Criteria region, index 0, name_focused flag)
                state.region = ViewEditRegion::Criteria;
                state.criteria_index = 0;
                state.name_focused = true;
                return;
            }
            state.name_focused = false;
            let adjusted = new_index - name_rows;
            if adjusted < criteria_rows {
                state.region = ViewEditRegion::Criteria;
                state.criteria_index = if state.draft.criteria.criteria.is_empty() {
                    0
                } else {
                    adjusted.min(state.draft.criteria.criteria.len().saturating_sub(1))
                };
            } else if datebook_rows > 0 && adjusted < criteria_rows + datebook_rows {
                state.region = ViewEditRegion::Datebook;
                state.datebook_field_index = adjusted - criteria_rows;
            } else {
                state.region = ViewEditRegion::Unmatched;
                state.unmatched_field_index = (adjusted - criteria_rows - datebook_rows)
                    .min(Self::view_details_aux_field_count() - 1);
            }
        }
    }

    pub(crate) fn handle_view_edit_criteria_key(&mut self, code: KeyCode) -> TuiResult<bool> {
        let Some(state) = &self.view_edit_state else {
            return Ok(false);
        };
        let name_focused = state.name_focused;
        let idx = state.criteria_index;
        let criteria_rows = Self::view_details_criteria_row_count(state);
        let datebook_rows = if state.draft.datebook_config.is_some() {
            Self::DATEBOOK_FIELD_COUNT
        } else {
            0
        };

        // When the Name row is focused, Enter/Space opens name editor,
        // j/Down navigates down, other keys are ignored.
        if name_focused {
            match code {
                KeyCode::Char('j') | KeyCode::Down => {
                    self.set_view_details_focus_index(Self::NAME_ROW_COUNT);
                }
                KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Char('r') | KeyCode::Char('R') => {
                    self.begin_view_edit_name_input();
                }
                _ => {}
            }
            return Ok(true);
        }

        match code {
            KeyCode::Char('j') | KeyCode::Down => {
                let current = self
                    .view_edit_state
                    .as_ref()
                    .map(Self::view_details_focus_index)
                    .unwrap_or(0);
                let max_index = Self::NAME_ROW_COUNT
                    + criteria_rows
                    + datebook_rows
                    + Self::view_details_aux_field_count()
                    - 1;
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

    pub(crate) fn handle_view_edit_datebook_key(&mut self, code: KeyCode) -> TuiResult<bool> {
        let Some(state) = &self.view_edit_state else {
            return Ok(false);
        };
        let datebook_rows = Self::DATEBOOK_FIELD_COUNT;
        let criteria_rows = Self::view_details_criteria_row_count(state);

        match code {
            KeyCode::Char('j') | KeyCode::Down => {
                let current = self
                    .view_edit_state
                    .as_ref()
                    .map(Self::view_details_focus_index)
                    .unwrap_or(0);
                let max_index = Self::NAME_ROW_COUNT
                    + criteria_rows
                    + datebook_rows
                    + Self::view_details_aux_field_count()
                    - 1;
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
            "Criteria: Space:cycle  +/1:require  -/2:exclude  3:or  0:clear  Esc:done".to_string();
    }

    pub(crate) fn open_view_edit_alias_picker(&mut self) {
        let first = first_non_reserved_category_index(&self.category_rows);
        if let Some(state) = &mut self.view_edit_state {
            state.active_tab = ViewEditTab::Appearance;
            state.appearance_row = AppearanceRow::Aliases;
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
                    let datebook_rows = if state.draft.datebook_config.is_some() {
                        Self::DATEBOOK_FIELD_COUNT
                    } else {
                        0
                    };
                    let max_index = Self::NAME_ROW_COUNT
                        + criteria_rows
                        + datebook_rows
                        + Self::view_details_aux_field_count()
                        - 1;
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
                    state.unmatched_field_index = 5;
                    state.draft.show_unmatched = !state.draft.show_unmatched;
                    state.dirty = true;
                    state.discard_confirm = false;
                }
            }
            KeyCode::Char('l') => {
                if let Some(state) = &mut self.view_edit_state {
                    let current = state.draft.unmatched_label.clone();
                    state.unmatched_field_index = 7;
                    state.inline_input = Some(ViewEditInlineInput::UnmatchedLabel);
                    state.inline_buf = text_buffer::TextBuffer::new(current);
                }
                self.status = "Unmatched label: type text  Enter:confirm  Esc:cancel".to_string();
            }
            KeyCode::Char('d') => {
                if let Some(state) = &mut self.view_edit_state {
                    state.unmatched_field_index = 6;
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
            KeyCode::Char('e') | KeyCode::Char('E') => {
                if let Some(state) = &mut self.view_edit_state {
                    state.unmatched_field_index = 4;
                    state.draft.empty_sections = state.draft.empty_sections.next();
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
                            state.draft.empty_sections = state.draft.empty_sections.next();
                            state.dirty = true;
                            state.discard_confirm = false;
                        }
                    }
                    5 => {
                        if let Some(state) = &mut self.view_edit_state {
                            state.draft.show_unmatched = !state.draft.show_unmatched;
                            state.dirty = true;
                            state.discard_confirm = false;
                        }
                    }
                    6 => {
                        if let Some(state) = &mut self.view_edit_state {
                            state.draft.hide_dependent_items = !state.draft.hide_dependent_items;
                            state.dirty = true;
                            state.discard_confirm = false;
                        }
                    }
                    _ => {
                        if target == 7 {
                            if let Some(state) = &mut self.view_edit_state {
                                let current = state.draft.unmatched_label.clone();
                                state.unmatched_field_index = 7;
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
