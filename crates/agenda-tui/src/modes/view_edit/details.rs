use crate::*;

impl App {
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

    pub(crate) fn view_scope_rows(state: &ViewEditState) -> Vec<ViewScopeRow> {
        let mut rows = vec![ViewScopeRow::ViewType];
        if state.draft.datebook_config.is_some() {
            rows.extend([
                ViewScopeRow::DatebookPeriod,
                ViewScopeRow::DatebookInterval,
                ViewScopeRow::DatebookAnchor,
                ViewScopeRow::DatebookDateSource,
            ]);
        }
        rows.push(ViewScopeRow::Name);
        let criteria_len = state.draft.criteria.criteria.len().max(1);
        rows.extend((0..criteria_len).map(ViewScopeRow::Criterion));
        rows.extend([
            ViewScopeRow::DateInclude,
            ViewScopeRow::DateExclude,
            ViewScopeRow::HideDependent,
        ]);
        rows
    }

    pub(crate) fn normalized_scope_row(state: &ViewEditState) -> ViewScopeRow {
        let rows = Self::view_scope_rows(state);
        if rows.contains(&state.scope_row) {
            return state.scope_row;
        }

        match state.scope_row {
            ViewScopeRow::Criterion(index) => {
                let adjusted = ViewScopeRow::Criterion(
                    index.min(state.draft.criteria.criteria.len().saturating_sub(1)),
                );
                if rows.contains(&adjusted) {
                    adjusted
                } else {
                    ViewScopeRow::Criterion(0)
                }
            }
            ViewScopeRow::DatebookPeriod
            | ViewScopeRow::DatebookInterval
            | ViewScopeRow::DatebookAnchor
            | ViewScopeRow::DatebookDateSource => ViewScopeRow::ViewType,
            _ => rows.first().copied().unwrap_or(ViewScopeRow::ViewType),
        }
    }

    pub(crate) fn sync_scope_row_to_legacy(state: &mut ViewEditState) {
        state.name_focused = false;
        match Self::normalized_scope_row(state) {
            ViewScopeRow::ViewType => {
                state.region = ViewEditRegion::Unmatched;
                state.unmatched_field_index = 0;
            }
            ViewScopeRow::DatebookPeriod => {
                state.region = ViewEditRegion::Datebook;
                state.datebook_field_index = 0;
            }
            ViewScopeRow::DatebookInterval => {
                state.region = ViewEditRegion::Datebook;
                state.datebook_field_index = 1;
            }
            ViewScopeRow::DatebookAnchor => {
                state.region = ViewEditRegion::Datebook;
                state.datebook_field_index = 2;
            }
            ViewScopeRow::DatebookDateSource => {
                state.region = ViewEditRegion::Datebook;
                state.datebook_field_index = 3;
            }
            ViewScopeRow::Name => {
                state.region = ViewEditRegion::Criteria;
                state.name_focused = true;
            }
            ViewScopeRow::Criterion(index) => {
                state.region = ViewEditRegion::Criteria;
                state.criteria_index = if state.draft.criteria.criteria.is_empty() {
                    0
                } else {
                    index.min(state.draft.criteria.criteria.len().saturating_sub(1))
                };
            }
            ViewScopeRow::DateInclude => {
                state.region = ViewEditRegion::Unmatched;
                state.unmatched_field_index = 0;
            }
            ViewScopeRow::DateExclude => {
                state.region = ViewEditRegion::Unmatched;
                state.unmatched_field_index = 1;
            }
            ViewScopeRow::HideDependent => {
                state.region = ViewEditRegion::Unmatched;
                state.unmatched_field_index = 6;
            }
        }
    }

    pub(crate) fn set_view_edit_scope_row(&mut self, row: ViewScopeRow) {
        if let Some(state) = &mut self.view_edit_state {
            state.scope_row = row;
            Self::sync_scope_row_to_legacy(state);
        }
    }

    pub(crate) fn normalized_appearance_row(state: &ViewEditState) -> ViewAppearanceRow {
        state.appearance_row
    }

    pub(crate) fn sync_appearance_row_to_legacy(state: &mut ViewEditState) {
        state.name_focused = false;
        state.region = ViewEditRegion::Unmatched;
        state.unmatched_field_index = match Self::normalized_appearance_row(state) {
            ViewAppearanceRow::DisplayMode => 2,
            ViewAppearanceRow::SectionFlow => 3,
            ViewAppearanceRow::EmptySections => 4,
            ViewAppearanceRow::Aliases => 8,
        };
    }

    pub(crate) fn set_view_edit_appearance_row(&mut self, row: ViewAppearanceRow) {
        if let Some(state) = &mut self.view_edit_state {
            state.appearance_row = row;
            Self::sync_appearance_row_to_legacy(state);
        }
    }

    pub(crate) fn sync_sections_settings_row_to_legacy(state: &mut ViewEditState) {
        state.name_focused = false;
        state.region = ViewEditRegion::Sections;
        if state.draft.datebook_config.is_none() {
            state.unmatched_field_index = match state.sections_settings_row {
                ViewSectionsSettingsRow::ShowUnmatched => 5,
                ViewSectionsSettingsRow::UnmatchedLabel => 7,
                ViewSectionsSettingsRow::DatebookPreview => 5,
            };
        }
    }

    pub(crate) fn set_view_edit_sections_settings_row(&mut self, row: ViewSectionsSettingsRow) {
        if let Some(state) = &mut self.view_edit_state {
            state.sections_settings_row = row;
            Self::sync_sections_settings_row_to_legacy(state);
        }
    }

    fn move_view_scope_row(&mut self, forward: bool) {
        let next = self.view_edit_state.as_ref().map(|state| {
            let rows = Self::view_scope_rows(state);
            let current = Self::normalized_scope_row(state);
            let current_index = rows.iter().position(|row| *row == current).unwrap_or(0);
            let next_index = if forward {
                (current_index + 1).min(rows.len().saturating_sub(1))
            } else {
                current_index.saturating_sub(1)
            };
            rows[next_index]
        });
        if let Some(row) = next {
            self.set_view_edit_scope_row(row);
        }
    }

    fn cycle_view_edit_datebook_config_row(&mut self, row: ViewScopeRow) {
        if let Some(state) = &mut self.view_edit_state {
            if let Some(config) = &mut state.draft.datebook_config {
                match row {
                    ViewScopeRow::DatebookPeriod => config.period = config.period.next(),
                    ViewScopeRow::DatebookInterval => config.interval = config.interval.next(),
                    ViewScopeRow::DatebookAnchor => config.anchor = config.anchor.next(),
                    ViewScopeRow::DatebookDateSource => {
                        config.date_source = config.date_source.next();
                    }
                    _ => {}
                }
                while !config.is_valid() {
                    config.interval = config.interval.next();
                }
                state.dirty = true;
                state.discard_confirm = false;
                state.section_delete_confirm = None;
            }
        }
        self.refresh_view_edit_preview();
    }

    fn open_view_edit_bucket_picker(&mut self, target: BucketEditTarget) {
        if let Some(state) = &mut self.view_edit_state {
            state.overlay = Some(ViewEditOverlay::BucketPicker { target });
            state.picker_index = 0;
        }
    }

    fn toggle_view_edit_view_type(&mut self) {
        if let Some(state) = &mut self.view_edit_state {
            if state.draft.datebook_config.is_some() {
                state.draft.datebook_config = None;
                if state.draft.sections.is_empty() {
                    state.draft.sections.push(Self::view_edit_default_section(
                        Self::DEFAULT_VIEW_EDIT_SECTION_TITLE,
                    ));
                }
                self.status = "View type: Board".to_string();
            } else {
                state.draft.datebook_config = Some(DatebookConfig::default());
                self.status = "View type: Datebook".to_string();
            }
            state.scope_row = ViewScopeRow::ViewType;
            Self::sync_scope_row_to_legacy(state);
            state.dirty = true;
            state.discard_confirm = false;
            state.section_delete_confirm = None;
            state.pane_focus = ViewEditPaneFocus::Details;
        }
        self.refresh_view_edit_preview();
    }

    pub(crate) fn handle_view_edit_scope_key(&mut self, code: KeyCode) -> TuiResult<bool> {
        let Some(row) = self
            .view_edit_state
            .as_ref()
            .map(Self::normalized_scope_row)
        else {
            return Ok(false);
        };

        match code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.move_view_scope_row(true);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.move_view_scope_row(false);
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                self.set_view_edit_scope_row(ViewScopeRow::Name);
                self.begin_view_edit_name_input();
            }
            KeyCode::Char('n') | KeyCode::Char('N') => {
                self.set_view_edit_scope_row(ViewScopeRow::Criterion(0));
                self.open_view_edit_view_criteria_picker();
            }
            KeyCode::Char('x') => {
                if let ViewScopeRow::Criterion(index) = row {
                    let mut changed = false;
                    let mut next_row = row;
                    if let Some(state) = &mut self.view_edit_state {
                        if index < state.draft.criteria.criteria.len() {
                            state.draft.criteria.criteria.remove(index);
                            let new_len = state.draft.criteria.criteria.len();
                            next_row = ViewScopeRow::Criterion(if new_len == 0 {
                                0
                            } else {
                                index.min(new_len.saturating_sub(1))
                            });
                            changed = true;
                        }
                    }
                    if changed {
                        self.set_view_edit_scope_row(next_row);
                        self.set_view_edit_dirty();
                        self.refresh_view_edit_preview();
                    }
                }
            }
            KeyCode::Char(']') => {
                self.set_view_edit_scope_row(ViewScopeRow::DateInclude);
                self.open_view_edit_bucket_picker(BucketEditTarget::ViewVirtualInclude);
            }
            KeyCode::Char('[') => {
                self.set_view_edit_scope_row(ViewScopeRow::DateExclude);
                self.open_view_edit_bucket_picker(BucketEditTarget::ViewVirtualExclude);
            }
            KeyCode::Char('d') | KeyCode::Char('D') => {
                self.set_view_edit_scope_row(ViewScopeRow::HideDependent);
                if let Some(state) = &mut self.view_edit_state {
                    state.draft.hide_dependent_items = !state.draft.hide_dependent_items;
                    state.dirty = true;
                    state.discard_confirm = false;
                }
            }
            KeyCode::Enter => match row {
                ViewScopeRow::Criterion(_) => self.open_view_edit_view_criteria_picker(),
                _ => self.activate_view_scope_row(row, false),
            },
            KeyCode::Char(' ') => {
                self.activate_view_scope_row(row, true);
            }
            _ => {}
        }
        Ok(true)
    }

    fn activate_view_scope_row(&mut self, row: ViewScopeRow, cycle_criterion: bool) {
        match row {
            ViewScopeRow::ViewType => self.toggle_view_edit_view_type(),
            ViewScopeRow::DatebookPeriod
            | ViewScopeRow::DatebookInterval
            | ViewScopeRow::DatebookAnchor
            | ViewScopeRow::DatebookDateSource => {
                self.cycle_view_edit_datebook_config_row(row);
            }
            ViewScopeRow::Name => {
                self.begin_view_edit_name_input();
            }
            ViewScopeRow::Criterion(index) => {
                if !cycle_criterion {
                    self.open_view_edit_view_criteria_picker();
                    return;
                }
                let mut changed = false;
                if let Some(state) = &mut self.view_edit_state {
                    if let Some(criterion) = state.draft.criteria.criteria.get_mut(index) {
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
            ViewScopeRow::DateInclude => {
                self.open_view_edit_bucket_picker(BucketEditTarget::ViewVirtualInclude);
            }
            ViewScopeRow::DateExclude => {
                self.open_view_edit_bucket_picker(BucketEditTarget::ViewVirtualExclude);
            }
            ViewScopeRow::HideDependent => {
                if let Some(state) = &mut self.view_edit_state {
                    state.draft.hide_dependent_items = !state.draft.hide_dependent_items;
                    state.dirty = true;
                    state.discard_confirm = false;
                }
            }
        }
    }

    pub(crate) fn open_view_edit_view_criteria_picker(&mut self) {
        let first = first_non_reserved_category_index(&self.category_rows);
        if let Some(state) = &mut self.view_edit_state {
            state.active_tab = ViewEditTab::Scope;
            state.overlay = Some(ViewEditOverlay::CategoryPicker {
                target: CategoryEditTarget::ViewCriteria,
            });
            state.picker_index = first;
        }
        self.status =
            "Criteria: Space:cycle  +/1:require  -/2:exclude  3:or  0:clear  Esc:done".to_string();
    }

    fn open_view_edit_alias_picker(&mut self) {
        let first = first_non_reserved_category_index(&self.category_rows);
        if let Some(state) = &mut self.view_edit_state {
            state.active_tab = ViewEditTab::Appearance;
            state.appearance_row = ViewAppearanceRow::Aliases;
            Self::sync_appearance_row_to_legacy(state);
            state.overlay = Some(ViewEditOverlay::CategoryPicker {
                target: CategoryEditTarget::ViewAliases,
            });
            state.picker_index = first;
        }
        self.status = Self::view_edit_alias_picker_status();
    }

    fn move_view_appearance_row(&mut self, forward: bool) {
        const ROWS: &[ViewAppearanceRow] = &[
            ViewAppearanceRow::DisplayMode,
            ViewAppearanceRow::SectionFlow,
            ViewAppearanceRow::EmptySections,
            ViewAppearanceRow::Aliases,
        ];
        let next = self.view_edit_state.as_ref().map(|state| {
            let current = Self::normalized_appearance_row(state);
            let current_index = ROWS.iter().position(|row| *row == current).unwrap_or(0);
            let next_index = if forward {
                (current_index + 1).min(ROWS.len().saturating_sub(1))
            } else {
                current_index.saturating_sub(1)
            };
            ROWS[next_index]
        });
        if let Some(row) = next {
            self.set_view_edit_appearance_row(row);
        }
    }

    fn activate_view_appearance_row(&mut self, row: ViewAppearanceRow) {
        match row {
            ViewAppearanceRow::DisplayMode => {
                if let Some(state) = &mut self.view_edit_state {
                    state.draft.board_display_mode =
                        Self::cycle_view_board_display_mode(state.draft.board_display_mode);
                    state.dirty = true;
                    state.discard_confirm = false;
                }
            }
            ViewAppearanceRow::SectionFlow => {
                if let Some(state) = &mut self.view_edit_state {
                    state.draft.section_flow =
                        Self::cycle_view_section_flow(state.draft.section_flow);
                    state.dirty = true;
                    state.discard_confirm = false;
                }
            }
            ViewAppearanceRow::EmptySections => {
                if let Some(state) = &mut self.view_edit_state {
                    state.draft.empty_sections = state.draft.empty_sections.next();
                    state.dirty = true;
                    state.discard_confirm = false;
                }
            }
            ViewAppearanceRow::Aliases => {
                self.open_view_edit_alias_picker();
            }
        }
    }

    pub(crate) fn handle_view_edit_appearance_key(&mut self, code: KeyCode) -> TuiResult<bool> {
        let Some(row) = self
            .view_edit_state
            .as_ref()
            .map(Self::normalized_appearance_row)
        else {
            return Ok(false);
        };

        match code {
            KeyCode::Char('j') | KeyCode::Down => self.move_view_appearance_row(true),
            KeyCode::Char('k') | KeyCode::Up => self.move_view_appearance_row(false),
            KeyCode::Char('m') | KeyCode::Char('M') => {
                self.set_view_edit_appearance_row(ViewAppearanceRow::DisplayMode);
                self.activate_view_appearance_row(ViewAppearanceRow::DisplayMode);
            }
            KeyCode::Char('w') | KeyCode::Char('W') => {
                self.set_view_edit_appearance_row(ViewAppearanceRow::SectionFlow);
                self.activate_view_appearance_row(ViewAppearanceRow::SectionFlow);
            }
            KeyCode::Char('e') | KeyCode::Char('E') => {
                self.set_view_edit_appearance_row(ViewAppearanceRow::EmptySections);
                self.activate_view_appearance_row(ViewAppearanceRow::EmptySections);
            }
            KeyCode::Char('a') | KeyCode::Char('A') => {
                self.set_view_edit_appearance_row(ViewAppearanceRow::Aliases);
                self.activate_view_appearance_row(ViewAppearanceRow::Aliases);
            }
            KeyCode::Char(' ') | KeyCode::Enter => {
                self.activate_view_appearance_row(row);
            }
            _ => {}
        }
        Ok(true)
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
