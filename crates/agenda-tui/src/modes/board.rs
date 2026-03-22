use crate::*;
use agenda_core::error::AgendaError;

/// Cycle: integer → 1dp → 2dp → 2dp+thousands → currency (2dp+thousands+$) → integer
pub(super) fn cycle_numeric_format_preset(current: &NumericFormat) -> NumericFormat {
    let has_currency = current.currency_symbol.is_some();
    match (
        current.decimal_places,
        current.use_thousands_separator,
        has_currency,
    ) {
        (0, false, false) => NumericFormat {
            decimal_places: 1,
            currency_symbol: None,
            use_thousands_separator: false,
        },
        (1, false, false) => NumericFormat {
            decimal_places: 2,
            currency_symbol: None,
            use_thousands_separator: false,
        },
        (2, false, false) => NumericFormat {
            decimal_places: 2,
            currency_symbol: None,
            use_thousands_separator: true,
        },
        (2, true, false) => NumericFormat {
            decimal_places: 2,
            currency_symbol: Some("$".to_string()),
            use_thousands_separator: true,
        },
        // Currency or any custom state → back to integer
        _ => NumericFormat {
            decimal_places: 0,
            currency_symbol: None,
            use_thousands_separator: false,
        },
    }
}

pub(crate) fn describe_numeric_format(fmt: &NumericFormat) -> &'static str {
    let has_currency = fmt.currency_symbol.is_some();
    match (
        fmt.decimal_places,
        fmt.use_thousands_separator,
        has_currency,
    ) {
        (0, false, false) => "Integer",
        (1, false, false) => "1 decimal place",
        (2, false, false) => "2 decimal places",
        (2, true, false) => "2dp, thousands",
        (_, _, true) => "Currency",
        _ => "Custom",
    }
}

enum InlineCreateConfirmKeyAction {
    Confirm,
    Cancel,
    DismissAndContinue,
    None,
}

fn inline_create_confirm_key_action(code: KeyCode) -> InlineCreateConfirmKeyAction {
    match code {
        KeyCode::Char('y') => InlineCreateConfirmKeyAction::Confirm,
        KeyCode::Esc => InlineCreateConfirmKeyAction::Cancel,
        KeyCode::Char(_)
        | KeyCode::Backspace
        | KeyCode::Delete
        | KeyCode::Left
        | KeyCode::Right => InlineCreateConfirmKeyAction::DismissAndContinue,
        _ => InlineCreateConfirmKeyAction::None,
    }
}

impl App {
    pub(crate) fn input_panel_discard_confirm_active(&self) -> bool {
        self.input_panel_discard_confirm
            && self
                .input_panel
                .as_ref()
                .is_some_and(|panel| panel.kind == input_panel::InputPanelKind::EditItem)
    }

    fn begin_input_panel_discard_confirm(&mut self) {
        self.input_panel_discard_confirm = true;
        self.status = "Discard unsaved item edits? y:discard n:keep editing".to_string();
    }

    fn cancel_input_panel_with_status(
        &mut self,
        kind: input_panel::InputPanelKind,
        was_dirty: bool,
    ) {
        self.input_panel_discard_confirm = false;
        self.input_panel = None;
        match kind {
            input_panel::InputPanelKind::NameInput
            | input_panel::InputPanelKind::WhenDate
            | input_panel::InputPanelKind::NumericValue
            | input_panel::InputPanelKind::CategoryCreate => {
                self.mode = self.name_input_return_mode();
                self.name_input_context = None;
                self.numeric_edit_target = None;
                self.when_edit_target = None;
            }
            input_panel::InputPanelKind::AddItem | input_panel::InputPanelKind::EditItem => {
                self.mode = Mode::Normal;
            }
        }
        self.status = if was_dirty {
            "Changes discarded".to_string()
        } else {
            "Canceled".to_string()
        };
    }

    pub(crate) fn category_column_picker_state(&self) -> Option<&CategoryColumnPickerState> {
        self.category_column_picker.as_ref()
    }

    fn category_column_picker_state_mut(&mut self) -> Option<&mut CategoryColumnPickerState> {
        self.category_column_picker.as_mut()
    }

    fn clear_category_column_picker_session(&mut self) {
        self.category_column_picker = None;
    }

    pub(crate) fn category_direct_edit_state(&self) -> Option<&CategoryDirectEditState> {
        self.category_direct_edit.as_ref()
    }

    pub(crate) fn category_direct_edit_state_mut(
        &mut self,
    ) -> Option<&mut CategoryDirectEditState> {
        self.category_direct_edit.as_mut()
    }

    fn clear_category_direct_edit_session(&mut self) {
        self.category_direct_edit = None;
        self.category_suggest = None;
        self.category_direct_edit_create_confirm = None; // legacy field; direct-edit now uses state.create_confirm_name
    }

    fn board_add_column_state(&self) -> Option<&BoardAddColumnState> {
        self.board_add_column.as_ref()
    }

    fn board_add_column_state_mut(&mut self) -> Option<&mut BoardAddColumnState> {
        self.board_add_column.as_mut()
    }

    fn clear_board_add_column_session(&mut self) {
        self.board_add_column = None;
        self.category_suggest = None;
    }

    pub(crate) fn board_add_column_input_text(&self) -> Option<&str> {
        self.board_add_column_state().map(|s| s.input.text())
    }

    pub(crate) fn board_add_column_create_confirm_name(&self) -> Option<&str> {
        self.board_add_column_state()?
            .create_confirm_name
            .as_deref()
    }

    fn board_add_column_create_confirm_open(&self) -> bool {
        self.board_add_column_create_confirm_name().is_some()
    }

    fn set_board_add_column_create_confirm_name(&mut self, name: Option<String>) {
        if let Some(state) = self.board_add_column_state_mut() {
            state.create_confirm_name = name;
        }
    }

    fn category_column_picker_filter_text(&self) -> Option<&str> {
        self.category_column_picker_state().map(|s| s.filter.text())
    }

    fn category_column_picker_create_confirm_name(&self) -> Option<&str> {
        self.category_column_picker_state()?
            .create_confirm_name
            .as_deref()
    }

    fn category_column_picker_create_confirm_open(&self) -> bool {
        self.category_column_picker_create_confirm_name().is_some()
    }

    fn set_category_column_picker_create_confirm_name(&mut self, name: Option<String>) {
        if let Some(state) = self.category_column_picker_state_mut() {
            state.create_confirm_name = name;
        }
    }

    pub(crate) fn category_column_picker_matches(&self) -> Vec<CategoryId> {
        let child_ids = self.get_current_column_child_ids();
        let query = self.category_column_picker_filter_text().unwrap_or("");
        filter_category_ids_by_query(&child_ids, &self.categories, query, true, true)
    }

    fn clamp_category_column_picker_list_index(&mut self) {
        let len = self.category_column_picker_matches().len();
        if let Some(state) = self.category_column_picker_state_mut() {
            state.list_index = if len == 0 {
                0
            } else {
                state.list_index.min(len - 1)
            };
        }
    }

    fn input_panel_category_filter_text(&self) -> Option<&str> {
        self.input_panel.as_ref().and_then(|panel| {
            if matches!(
                panel.kind,
                input_panel::InputPanelKind::AddItem | input_panel::InputPanelKind::EditItem
            ) {
                Some(panel.category_filter.text())
            } else {
                None
            }
        })
    }

    pub(crate) fn input_panel_visible_category_row_indices(&self) -> Vec<usize> {
        let query = self
            .input_panel_category_filter_text()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        // Hide reserved categories in AddItem/EditItem panels
        let hide_reserved = self
            .input_panel
            .as_ref()
            .map(|p| {
                matches!(
                    p.kind,
                    input_panel::InputPanelKind::AddItem | input_panel::InputPanelKind::EditItem
                )
            })
            .unwrap_or(false);
        self.category_rows
            .iter()
            .enumerate()
            .filter(|(_, row)| {
                if hide_reserved && row.is_reserved {
                    return false;
                }
                query.is_empty() || row.name.to_ascii_lowercase().contains(&query)
            })
            .map(|(idx, _)| idx)
            .collect()
    }

    pub(crate) fn input_panel_selected_category_row_index(&self) -> Option<usize> {
        let visible_indices = self.input_panel_visible_category_row_indices();
        let suggestion_len = self
            .input_panel
            .as_ref()
            .map(|p| p.pending_suggestions.len())
            .unwrap_or(0);
        let cursor = self
            .input_panel
            .as_ref()
            .map(|panel| panel.category_cursor)?;
        // Suggestions occupy indices 0..suggestion_len; categories start at suggestion_len
        if cursor < suggestion_len {
            return None; // cursor is on a suggestion row
        }
        visible_indices.get(cursor - suggestion_len).copied()
    }

    pub(crate) fn input_panel_selected_category_row(&self) -> Option<&CategoryListRow> {
        let row_index = self.input_panel_selected_category_row_index()?;
        self.category_rows.get(row_index)
    }

    fn clamp_input_panel_category_cursor(&mut self) {
        let visible_len = self.input_panel_visible_category_row_indices().len();
        let suggestion_len = self
            .input_panel
            .as_ref()
            .map(|p| p.pending_suggestions.len())
            .unwrap_or(0);
        let total_len = suggestion_len + visible_len;
        if let Some(panel) = &mut self.input_panel {
            panel.category_cursor = if total_len == 0 {
                0
            } else {
                panel.category_cursor.min(total_len - 1)
            };
        }
    }

    fn handle_input_panel_category_filter_key(&mut self, code: KeyCode) -> bool {
        let mut status: Option<String> = None;
        let mut should_clamp = false;
        let mut show_matches = false;
        let text_key = self.text_key_event(code);
        let consumed = {
            let Some(panel) = self.input_panel.as_mut() else {
                return false;
            };
            if !matches!(
                panel.kind,
                input_panel::InputPanelKind::AddItem | input_panel::InputPanelKind::EditItem
            ) || panel.focus != input_panel::InputPanelFocus::Categories
            {
                return false;
            }

            if code == KeyCode::Char('/') {
                panel.category_filter_editing = true;
                status =
                    Some("Type to filter categories, Enter to keep filter, Esc done".to_string());
                true
            } else if !panel.category_filter_editing {
                false
            } else {
                match code {
                    KeyCode::Enter => {
                        panel.category_filter_editing = false;
                        status = Some("Category filter applied".to_string());
                        true
                    }
                    KeyCode::Esc => {
                        panel.category_filter_editing = false;
                        status = Some("Category filter exited".to_string());
                        true
                    }
                    KeyCode::Tab | KeyCode::BackTab => {
                        panel.category_filter_editing = false;
                        false
                    }
                    _ => {
                        if panel.category_filter.handle_key_event(text_key, false) {
                            should_clamp = true;
                            show_matches = true;
                        }
                        true
                    }
                }
            }
        };

        if should_clamp {
            self.clamp_input_panel_category_cursor();
        }
        if show_matches {
            let matches = self.input_panel_visible_category_row_indices().len();
            self.status = format!("Category filter matches: {matches}");
        } else if let Some(status) = status {
            self.status = status;
        }
        consumed
    }

    pub(crate) fn active_category_direct_edit_row(&self) -> Option<&CategoryDirectEditRow> {
        self.category_direct_edit_state()?.active_row()
    }

    fn active_category_direct_edit_focus(&self) -> Option<CategoryDirectEditFocus> {
        self.category_direct_edit_state().map(|state| state.focus)
    }

    fn active_category_direct_edit_row_mut(&mut self) -> Option<&mut CategoryDirectEditRow> {
        self.category_direct_edit_state_mut()?.active_row_mut()
    }

    pub(crate) fn active_category_direct_edit_input_text(&self) -> Option<&str> {
        self.active_category_direct_edit_row()
            .map(|row| row.input.text())
    }

    fn direct_edit_create_confirm_name(&self) -> Option<&str> {
        self.category_direct_edit_state()?
            .create_confirm_name
            .as_deref()
    }

    fn direct_edit_create_confirm_open(&self) -> bool {
        self.direct_edit_create_confirm_name().is_some()
    }

    fn set_direct_edit_create_confirm_name(&mut self, name: Option<String>) {
        if let Some(state) = self.category_direct_edit_state_mut() {
            state.create_confirm_name = name;
        }
    }

    fn sync_category_direct_edit_input_mirror(&mut self) {
        if let Some(text) = self.active_category_direct_edit_input_text() {
            self.set_input(text.to_string());
        }
    }

    fn refresh_category_cache(&mut self, store: &Store) -> TuiResult<()> {
        self.categories = store.get_hierarchy()?;
        self.category_rows = build_category_rows(&self.categories);
        self.category_index = self
            .category_index
            .min(self.category_rows.len().saturating_sub(1));
        Ok(())
    }

    fn category_direct_edit_focus_label(focus: CategoryDirectEditFocus) -> &'static str {
        match focus {
            CategoryDirectEditFocus::Entries => "Entries",
            CategoryDirectEditFocus::Input => "Input",
            CategoryDirectEditFocus::Suggestions => "Suggestions",
        }
    }

    fn cycle_category_direct_edit_focus(&mut self, forward: bool) {
        let mut new_focus = None;
        if let Some(state) = self.category_direct_edit_state_mut() {
            state.focus = if forward {
                state.focus.next()
            } else {
                state.focus.prev()
            };
            new_focus = Some(state.focus);
        }
        self.sync_category_direct_edit_input_mirror();
        self.update_suggestions();
        if let Some(focus) = new_focus {
            let label = Self::category_direct_edit_focus_label(focus);
            self.status = format!("Direct edit focus: {label}");
        }
    }

    fn move_category_direct_edit_active_row(&mut self, delta: i32) {
        let Some(state) = self.category_direct_edit_state_mut() else {
            return;
        };
        if state.rows.is_empty() {
            state.ensure_one_row();
        } else {
            state.active_row = next_index_clamped(state.active_row, state.rows.len(), delta);
        }
        state.clamp_active_row();
        self.sync_category_direct_edit_input_mirror();
        self.update_suggestions();
    }

    fn remove_active_category_direct_edit_row(&mut self) {
        let Some(state) = self.category_direct_edit_state_mut() else {
            return;
        };
        let before_len = state.rows.len();
        let active = state.active_row;
        let _ = state.remove_row(active);
        let after_len = state.rows.len();
        let kept_single_blank = before_len == 1 && after_len == 1;
        self.sync_category_direct_edit_input_mirror();
        self.update_suggestions();
        self.status = if kept_single_blank {
            "Kept one blank row (cannot remove the last row)".to_string()
        } else {
            "Removed row".to_string()
        };
    }

    fn resolve_active_category_direct_edit_row(
        &mut self,
        category_id: CategoryId,
    ) -> TuiResult<bool> {
        let Some(cat_name) = self
            .categories
            .iter()
            .find(|c| c.id == category_id)
            .map(|c| c.name.clone())
        else {
            return Ok(false);
        };
        let duplicate = match self.category_direct_edit_state() {
            Some(state) => state.row_would_duplicate_category_id(state.active_row, category_id),
            None => return Ok(false),
        };
        if duplicate {
            self.status = "Category already selected in another row".to_string();
            return Ok(false);
        }
        if let Some(row) = self.active_category_direct_edit_row_mut() {
            row.category_id = Some(category_id);
            row.input.set(cat_name.clone());
        }
        self.sync_category_direct_edit_input_mirror();
        self.update_suggestions();
        if self.current_column_parent_is_exclusive() {
            self.status = format!("Resolved row to '{cat_name}'. Press s/S to save");
        } else {
            self.status =
                format!("Resolved row to '{cat_name}'. Press + to add another row or s/S to save");
        }
        Ok(true)
    }

    fn resolve_active_row_from_highlighted_suggestion(&mut self) -> TuiResult<bool> {
        let matches = self.get_current_suggest_matches();
        let Some(state) = self.category_direct_edit_state() else {
            return Ok(false);
        };
        let Some(&id) = matches.get(state.suggest_index.min(matches.len().saturating_sub(1)))
        else {
            return Ok(false);
        };
        self.resolve_active_category_direct_edit_row(id)
    }

    fn open_direct_edit_create_confirm_for_active_row(&mut self) {
        let typed = self
            .active_category_direct_edit_input_text()
            .unwrap_or("")
            .trim()
            .to_string();
        if typed.is_empty() {
            self.status = "Empty row: nothing to create".to_string();
            return;
        }
        if is_reserved_category_name(&typed) {
            self.status = format!(
                "Cannot create reserved category '{}'. Use a different name.",
                typed
            );
            return;
        }
        if let Some(existing_cat) = self
            .categories
            .iter()
            .find(|c| c.name.eq_ignore_ascii_case(&typed))
        {
            let parent_name = existing_cat
                .parent
                .and_then(|pid| self.categories.iter().find(|c| c.id == pid))
                .map(|c| c.name.as_str())
                .unwrap_or("(root)");
            self.status = format!(
                "Category '{}' exists under '{}'. Cannot create duplicate.",
                typed, parent_name
            );
            return;
        }
        self.set_direct_edit_create_confirm_name(Some(typed.clone()));
        self.status = format!(
            "Create new category '{}' in this column? y:confirm Esc:cancel",
            typed
        );
    }

    fn desired_child_ids_from_category_direct_edit_draft(&self) -> Vec<CategoryId> {
        let Some(state) = self.category_direct_edit_state() else {
            return Vec::new();
        };
        let mut seen = HashSet::new();
        state
            .rows
            .iter()
            .filter_map(|row| row.category_id)
            .filter(|id| seen.insert(*id))
            .collect()
    }

    fn category_direct_edit_has_unresolved_nonempty_rows(&self) -> bool {
        self.category_direct_edit_state()
            .map(|state| {
                state
                    .rows
                    .iter()
                    .any(|row| row.category_id.is_none() && !row.input.trimmed().is_empty())
            })
            .unwrap_or(false)
    }

    fn current_category_direct_edit_column_meta(&self) -> Option<CategoryDirectEditColumnMeta> {
        let slot = self.current_slot()?;
        let item = self.selected_item()?;
        let view = self.current_view()?;

        let (section_index, is_generated_section) = match slot.context {
            SlotContext::Section { section_index } => (section_index, false),
            SlotContext::GeneratedSection { section_index, .. } => (section_index, true),
            SlotContext::Unmatched => return None,
        };

        let section = view.sections.get(section_index)?;
        let section_column_index =
            Self::board_column_to_section_column_index(section, self.column_index)?;
        let column = section.columns.get(section_column_index)?;
        let parent = self.categories.iter().find(|c| c.id == column.heading)?;

        Some(CategoryDirectEditColumnMeta {
            parent_id: parent.id,
            parent_name: parent.name.clone(),
            column_kind: column.kind,
            anchor: CategoryDirectEditAnchor {
                slot_index: self.slot_index,
                section_index,
                section_column_index,
                board_column_index: self.column_index,
                is_generated_section,
            },
            item_id: item.id,
            item_label: board_item_label(item),
        })
    }

    fn current_board_add_column_anchor(
        &self,
        direction: AddColumnDirection,
    ) -> TuiResult<BoardAddColumnAnchor> {
        let slot = self
            .current_slot()
            .ok_or("No active board slot".to_string())?;
        let (section_index, is_generated_section) = match slot.context {
            SlotContext::Section { section_index } => (section_index, false),
            SlotContext::GeneratedSection { section_index, .. } => (section_index, true),
            SlotContext::Unmatched => {
                return Err("Cannot add columns from unmatched lane".into());
            }
        };
        let view = self
            .current_view()
            .ok_or("No active view available".to_string())?;
        let section = view
            .sections
            .get(section_index)
            .ok_or("Current section not found".to_string())?;
        if self.column_index > section.columns.len() {
            return Err("Current column is out of range".into());
        }
        let item_column_index = Self::section_item_column_index(section);
        let current_section_column_index = if self.column_index == item_column_index {
            item_column_index
        } else {
            Self::board_column_to_section_column_index(section, self.column_index)
                .ok_or("Current column is out of range".to_string())?
        };
        let insert_index = if self.column_index == item_column_index {
            item_column_index
        } else {
            match direction {
                AddColumnDirection::Left => current_section_column_index,
                AddColumnDirection::Right => current_section_column_index + 1,
            }
        };

        Ok(BoardAddColumnAnchor {
            slot_index: self.slot_index,
            section_index,
            current_board_column_index: self.column_index,
            current_section_column_index,
            item_column_index_before: item_column_index,
            insert_index,
            direction,
            is_generated_section,
        })
    }

    fn is_valid_board_column_heading_category(category: &Category) -> bool {
        is_valid_column_heading(category)
    }

    fn board_add_column_scope_ids(&self) -> Vec<CategoryId> {
        let existing_in_section: HashSet<CategoryId> = self
            .board_add_column_state()
            .and_then(|state| {
                self.current_view()
                    .and_then(|v| v.sections.get(state.anchor.section_index))
                    .map(|section| section.columns.iter().map(|c| c.heading).collect())
            })
            .unwrap_or_default();
        self.categories
            .iter()
            .filter(|c| Self::is_valid_board_column_heading_category(c))
            .filter(|c| !existing_in_section.contains(&c.id))
            .map(|c| c.id)
            .collect()
    }

    fn current_board_add_column_section_has_heading(&self, category_id: CategoryId) -> bool {
        self.board_add_column_state()
            .and_then(|state| {
                self.current_view()
                    .and_then(|v| v.sections.get(state.anchor.section_index))
            })
            .map(|section| section.columns.iter().any(|c| c.heading == category_id))
            .unwrap_or(false)
    }

    pub(crate) fn get_board_add_column_suggest_matches(&self) -> Vec<CategoryId> {
        let scope_ids = self.board_add_column_scope_ids();
        let query = self.board_add_column_input_text().unwrap_or("");
        filter_category_ids_by_query(&scope_ids, &self.categories, query, true, false)
    }

    fn exact_board_add_column_match_id(&self) -> Option<CategoryId> {
        let scope_ids = self.board_add_column_scope_ids();
        exact_category_name_match_in_scope(
            &scope_ids,
            &self.categories,
            self.board_add_column_input_text()?,
        )
    }

    fn update_board_add_column_suggestions(&mut self) {
        if self.board_add_column_create_confirm_open() {
            return;
        }
        let matches = self.get_board_add_column_suggest_matches();
        if matches.is_empty() {
            let typed = self
                .board_add_column_state()
                .map(|s| s.input.trimmed().to_string())
                .unwrap_or_default();
            if let Some(state) = self.board_add_column_state_mut() {
                state.suggest_index = 0;
            }
            self.category_suggest = None;
            self.status = if typed.is_empty() {
                "Add column: type to filter categories with children (or When)".to_string()
            } else {
                "No valid column heading match. Enter explains why (leaf headings are invalid)."
                    .to_string()
            };
        } else {
            if let Some(state) = self.board_add_column_state_mut() {
                state.suggest_index = state.suggest_index.min(matches.len() - 1);
            }
            self.category_suggest = Some(CategorySuggestState { suggest_index: 0 });
            self.status =
                "Add column: type to filter, ↑↓ select, Tab autocomplete, Enter insert".to_string();
        }
    }

    fn move_board_add_column_suggest_cursor(&mut self, delta: i32) {
        if self.board_add_column_create_confirm_open() {
            return;
        }
        let matches = self.get_board_add_column_suggest_matches();
        let len = matches.len();
        if len == 0 {
            return;
        }
        if let Some(state) = self.board_add_column_state_mut() {
            let current = state.suggest_index.min(len - 1);
            state.suggest_index = (current as i64 + delta as i64).rem_euclid(len as i64) as usize;
        }
    }

    fn autocomplete_board_add_column_from_suggestion(&mut self) {
        if self.board_add_column_create_confirm_open() {
            return;
        }
        let matches = self.get_board_add_column_suggest_matches();
        let Some(state) = self.board_add_column_state() else {
            return;
        };
        let Some(&id) = matches.get(state.suggest_index.min(matches.len().saturating_sub(1)))
        else {
            return;
        };
        let Some(name) = self
            .categories
            .iter()
            .find(|c| c.id == id)
            .map(|c| c.name.clone())
        else {
            return;
        };
        if let Some(state) = self.board_add_column_state_mut() {
            state.input.set(name);
        }
        self.update_board_add_column_suggestions();
    }

    fn collect_assigned_child_categories_for_parent(
        &self,
        item: &Item,
        parent_id: CategoryId,
    ) -> Vec<CategoryId> {
        let mut assigned_child_ids: HashSet<CategoryId> = item
            .assignments
            .keys()
            .filter(|id| {
                self.categories
                    .iter()
                    .find(|c| c.id == **id)
                    .and_then(|c| c.parent)
                    .map(|pid| pid == parent_id)
                    .unwrap_or(false)
            })
            .copied()
            .collect();

        if assigned_child_ids.is_empty() {
            return Vec::new();
        }

        let mut ordered = Vec::new();
        if let Some(parent) = self.categories.iter().find(|c| c.id == parent_id) {
            for child_id in &parent.children {
                if assigned_child_ids.remove(child_id) {
                    ordered.push(*child_id);
                }
            }
        }

        let mut extras: Vec<CategoryId> = assigned_child_ids.into_iter().collect();
        extras.sort_by(|a, b| {
            let a_name = self
                .categories
                .iter()
                .find(|c| c.id == *a)
                .map(|c| c.name.to_ascii_lowercase())
                .unwrap_or_else(|| a.to_string());
            let b_name = self
                .categories
                .iter()
                .find(|c| c.id == *b)
                .map(|c| c.name.to_ascii_lowercase())
                .unwrap_or_else(|| b.to_string());
            a_name
                .cmp(&b_name)
                .then_with(|| a.to_string().cmp(&b.to_string()))
        });
        ordered.extend(extras);
        ordered
    }

    fn current_column_assigned_child_ids(&self) -> Vec<CategoryId> {
        let Some(meta) = self.current_category_direct_edit_column_meta() else {
            return Vec::new();
        };
        let Some(item) = self.selected_item() else {
            return Vec::new();
        };
        self.collect_assigned_child_categories_for_parent(item, meta.parent_id)
    }

    // TODO(feature): inline column direct-edit not yet triggered; see open_category_direct_edit
    #[allow(dead_code)]
    fn build_current_column_direct_edit_rows(&self) -> Vec<CategoryDirectEditRow> {
        let assigned_child_ids = self.current_column_assigned_child_ids();
        let mut rows: Vec<CategoryDirectEditRow> = assigned_child_ids
            .into_iter()
            .map(|category_id| {
                let name = self
                    .categories
                    .iter()
                    .find(|c| c.id == category_id)
                    .map(|c| c.name.clone())
                    .unwrap_or_else(|| category_id.to_string());
                CategoryDirectEditRow::resolved(category_id, name)
            })
            .collect();
        if rows.is_empty() {
            rows.push(CategoryDirectEditRow::blank());
        }
        rows
    }

    fn current_column_parent_is_exclusive(&self) -> bool {
        let Some(meta) = self.current_category_direct_edit_column_meta() else {
            return false;
        };
        self.categories
            .iter()
            .find(|c| c.id == meta.parent_id)
            .map(|c| c.is_exclusive)
            .unwrap_or(false)
    }

    fn category_direct_edit_add_blank_row_guarded(&mut self) -> bool {
        let is_exclusive = self.current_column_parent_is_exclusive();
        let Some(state) = self.category_direct_edit_state_mut() else {
            return false;
        };
        if is_exclusive && !state.rows.is_empty() {
            self.status = format!("'{}' is exclusive; only one row allowed", state.parent_name);
            return false;
        }
        state.add_blank_row();
        state.focus = CategoryDirectEditFocus::Input;
        state.suggest_index = 0;
        // End mutable borrow before calling other `self` methods.
        let _ = state;
        self.sync_category_direct_edit_input_mirror();
        self.update_suggestions();
        self.status = "Added row".to_string();
        true
    }

    pub(crate) fn toggle_preview(&mut self) {
        self.show_preview = !self.show_preview;
        if self.show_preview {
            self.preview_mode = PreviewMode::Summary;
            self.normal_focus = NormalFocus::Board;
            self.preview_summary_scroll = 0;
            self.status = "Preview opened (Summary). f to focus pane, i for info".to_string();
        } else {
            self.normal_focus = NormalFocus::Board;
            self.status = "Preview closed".to_string();
        }
    }

    pub(crate) fn toggle_preview_mode(&mut self) {
        if !self.show_preview {
            self.status = "Preview is closed (press p to open)".to_string();
            return;
        }
        self.preview_mode = match self.preview_mode {
            PreviewMode::Summary => PreviewMode::Provenance,
            PreviewMode::Provenance => PreviewMode::Summary,
        };
        self.status = match self.preview_mode {
            PreviewMode::Summary => "Preview mode: Summary".to_string(),
            PreviewMode::Provenance => "Preview mode: Info".to_string(),
        };
    }

    pub(crate) fn scroll_preview(&mut self, delta: i32) {
        if !self.show_preview {
            return;
        }
        match self.preview_mode {
            PreviewMode::Summary => {
                if delta > 0 {
                    self.preview_summary_scroll = self.preview_summary_scroll.saturating_add(1);
                } else {
                    self.preview_summary_scroll = self.preview_summary_scroll.saturating_sub(1);
                }
            }
            PreviewMode::Provenance => {
                if delta > 0 {
                    self.preview_provenance_scroll =
                        self.preview_provenance_scroll.saturating_add(1);
                } else {
                    self.preview_provenance_scroll =
                        self.preview_provenance_scroll.saturating_sub(1);
                }
            }
        }
    }

    pub(crate) fn open_board_add_column_picker(
        &mut self,
        direction: AddColumnDirection,
    ) -> TuiResult<()> {
        let anchor = self.current_board_add_column_anchor(direction)?;
        self.mode = Mode::BoardAddColumnPicker;
        self.board_add_column = Some(BoardAddColumnState {
            anchor,
            input: text_buffer::TextBuffer::empty(),
            suggest_index: 0,
            create_confirm_name: None,
        });
        self.category_suggest = None;
        self.status = match direction {
            AddColumnDirection::Left => "Add column (left): type a category name".to_string(),
            AddColumnDirection::Right => "Add column (right): type a category name".to_string(),
        };
        self.update_board_add_column_suggestions();
        Ok(())
    }

    fn open_board_add_column_create_confirm(&mut self) {
        let typed = self
            .board_add_column_input_text()
            .unwrap_or("")
            .trim()
            .to_string();
        if typed.is_empty() {
            self.status = "Type a category name first".to_string();
            return;
        }
        if is_reserved_category_name(&typed) {
            if typed.eq_ignore_ascii_case("When") {
                self.status = "Press Enter to insert the existing 'When' column".to_string();
            } else {
                self.status = format!(
                    "Cannot create reserved category '{}'. Use a different name.",
                    typed
                );
            }
            return;
        }
        if let Some(existing_cat) = self
            .categories
            .iter()
            .find(|c| c.name.eq_ignore_ascii_case(&typed))
        {
            if self.current_board_add_column_section_has_heading(existing_cat.id) {
                self.status = format!(
                    "Column '{}' already exists in this section.",
                    existing_cat.name
                );
                return;
            }
            if !is_valid_column_heading(existing_cat) {
                let parent_label = existing_cat
                    .parent
                    .and_then(|pid| self.categories.iter().find(|c| c.id == pid))
                    .map(|c| c.name.as_str());
                self.status = if let Some(parent_name) = parent_label {
                    format!(
                        "Category '{}' exists under '{}' and is not a valid column heading.",
                        existing_cat.name, parent_name
                    )
                } else {
                    format!(
                        "Category '{}' is not a valid column heading: needs subcategories or numeric type.",
                        existing_cat.name
                    )
                };
            } else {
                let parent_label = existing_cat
                    .parent
                    .and_then(|pid| self.categories.iter().find(|c| c.id == pid))
                    .map(|c| c.name.as_str())
                    .unwrap_or("(top level)");
                self.status = format!(
                    "Category '{}' already exists under '{}'; use Enter to insert it.",
                    existing_cat.name, parent_label
                );
            }
            return;
        }
        self.status = format!(
            "Cannot create '{}' here: column headings must already have subcategories. Create the category and at least one child first.",
            typed
        );
    }

    fn insert_board_column_for_category(
        &mut self,
        agenda: &Agenda<'_>,
        category_id: CategoryId,
    ) -> TuiResult<()> {
        let Some(add_state) = self.board_add_column_state().cloned() else {
            return Ok(());
        };
        let Some(mut view) = self.current_view().cloned() else {
            return Err("No active view".into());
        };
        let Some(section) = view.sections.get_mut(add_state.anchor.section_index) else {
            return Err("Current section not found".into());
        };

        if section.columns.iter().any(|col| col.heading == category_id) {
            let existing_idx = section
                .columns
                .iter()
                .position(|col| col.heading == category_id)
                .unwrap_or(0);
            self.status = "Column already exists in this section".to_string();
            self.column_index = Self::section_column_to_board_column_index(section, existing_idx);
            return Ok(());
        }

        let Some(heading_category) = self.categories.iter().find(|c| c.id == category_id) else {
            self.status = "Selected category no longer exists".to_string();
            return Ok(());
        };
        if !Self::is_valid_board_column_heading_category(heading_category) {
            self.status = format!(
                "Invalid column heading '{}': needs subcategories, numeric type, or be When",
                heading_category.name
            );
            return Ok(());
        }

        let kind = column_kind_for_heading(heading_category);

        let insert_index = add_state.anchor.insert_index.min(section.columns.len());
        let item_column_index_before = add_state
            .anchor
            .item_column_index_before
            .min(section.columns.len());
        section.columns.insert(
            insert_index,
            Column {
                kind,
                heading: category_id,
                width: 12,
                summary_fn: None,
            },
        );
        let inserted_board_index_before = match add_state.anchor.direction {
            AddColumnDirection::Left => add_state.anchor.current_board_column_index,
            AddColumnDirection::Right => add_state.anchor.current_board_column_index + 1,
        };
        let should_shift_item_right = inserted_board_index_before <= item_column_index_before;
        let mut new_item_column_index = item_column_index_before;
        if should_shift_item_right {
            new_item_column_index += 1;
        }
        section.item_column_index = new_item_column_index.min(section.columns.len());
        let inserted_board_column_index =
            Self::section_column_to_board_column_index(section, insert_index);

        let view_name = view.name.clone();
        let selected_item_id = self.selected_item_id();
        agenda.store().update_view(&view)?;
        self.clear_board_add_column_session();
        self.mode = Mode::Normal;
        self.refresh(agenda.store())?;
        self.set_view_selection_by_name(&view_name);
        if let Some(item_id) = selected_item_id {
            self.set_item_selection_by_id(item_id);
        }
        self.slot_index = add_state
            .anchor
            .slot_index
            .min(self.slots.len().saturating_sub(1));
        self.column_index = inserted_board_column_index.min(self.current_slot_column_count());
        let category_name = self
            .categories
            .iter()
            .find(|c| c.id == category_id)
            .map(|c| c.name.clone())
            .unwrap_or_else(|| "(unknown)".to_string());
        self.status = format!(
            "Inserted column '{}' {}",
            category_name,
            match add_state.anchor.direction {
                AddColumnDirection::Left => "to the left",
                AddColumnDirection::Right => "to the right",
            }
        );
        Ok(())
    }

    fn move_current_board_column_to_index(
        &mut self,
        target_board_index: usize,
        agenda: &Agenda<'_>,
    ) -> TuiResult<()> {
        let Some(slot) = self.current_slot() else {
            self.status = "No active board slot".to_string();
            return Ok(());
        };
        let section_index = match slot.context {
            SlotContext::Section { section_index }
            | SlotContext::GeneratedSection { section_index, .. } => section_index,
            SlotContext::Unmatched => {
                self.status = "Cannot reorder columns in unmatched lane".to_string();
                return Ok(());
            }
        };
        let Some(mut view) = self.current_view().cloned() else {
            return Err("No active view".into());
        };
        let Some(section) = view.sections.get_mut(section_index) else {
            return Err("Current section not found".into());
        };
        if self.column_index > section.columns.len() {
            self.status = "Current column is out of range".to_string();
            return Ok(());
        }

        let max_board_index = section.columns.len();
        let current_board_index = self.column_index.min(max_board_index);
        let target_board_index = target_board_index.min(max_board_index);
        if current_board_index == target_board_index {
            self.status = "Column is already in that position".to_string();
            return Ok(());
        }

        let item_board_index = Self::section_item_column_index(section);
        let item_label = view
            .item_column_label
            .clone()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "Item".to_string());
        let moved_label = if current_board_index == item_board_index {
            item_label
        } else {
            let section_col_index =
                Self::board_column_to_section_column_index(section, current_board_index)
                    .ok_or("Current column is out of range".to_string())?;
            let heading_id = section
                .columns
                .get(section_col_index)
                .map(|c| c.heading)
                .ok_or("Current column is out of range".to_string())?;
            self.categories
                .iter()
                .find(|c| c.id == heading_id)
                .map(|c| c.name.clone())
                .unwrap_or_else(|| "(deleted)".to_string())
        };

        let mut board_tokens: Vec<Option<Column>> = (0..=section.columns.len())
            .map(|board_index| {
                if board_index == item_board_index {
                    None
                } else {
                    let section_col_index =
                        Self::board_column_to_section_column_index(section, board_index)
                            .expect("valid board index");
                    Some(section.columns[section_col_index].clone())
                }
            })
            .collect();
        let moved = board_tokens.remove(current_board_index);
        board_tokens.insert(target_board_index, moved);

        section.columns.clear();
        let mut new_item_column_index = 0usize;
        for (idx, token) in board_tokens.into_iter().enumerate() {
            match token {
                None => new_item_column_index = idx,
                Some(col) => section.columns.push(col),
            }
        }
        section.item_column_index = new_item_column_index.min(section.columns.len());

        let view_name = view.name.clone();
        let selected_slot_index = self.slot_index;
        let selected_item_id = self.selected_item_id();
        agenda.store().update_view(&view)?;
        self.refresh(agenda.store())?;
        self.set_view_selection_by_name(&view_name);
        self.slot_index = selected_slot_index.min(self.slots.len().saturating_sub(1));
        if let Some(item_id) = selected_item_id {
            let restored_in_slot = self
                .slots
                .get(self.slot_index)
                .and_then(|slot| slot.items.iter().position(|item| item.id == item_id));
            if let Some(item_index) = restored_in_slot {
                self.item_index = item_index;
            } else {
                self.set_item_selection_by_id(item_id);
            }
        }
        self.column_index = target_board_index.min(self.current_slot_column_count());
        self.status = format!(
            "Moved column '{}' {}",
            moved_label,
            if target_board_index < current_board_index {
                "left"
            } else {
                "right"
            }
        );
        Ok(())
    }

    fn move_current_board_column_relative(
        &mut self,
        delta: i32,
        agenda: &Agenda<'_>,
    ) -> TuiResult<()> {
        let Some(slot) = self.current_slot() else {
            self.status = "No active board slot".to_string();
            return Ok(());
        };
        let max_board_index = match slot.context {
            SlotContext::Section { section_index }
            | SlotContext::GeneratedSection { section_index, .. } => self
                .current_view()
                .and_then(|v| v.sections.get(section_index))
                .map(|s| s.columns.len())
                .unwrap_or(0),
            SlotContext::Unmatched => {
                self.status = "Cannot reorder columns in unmatched lane".to_string();
                return Ok(());
            }
        };
        let target =
            next_index_clamped(self.column_index, max_board_index.saturating_add(1), delta);
        self.move_current_board_column_to_index(target, agenda)
    }

    fn open_remove_current_board_column_confirm(&mut self) {
        let Some(slot) = self.current_slot() else {
            self.status = "No active board slot".to_string();
            return;
        };
        let section_index = match slot.context {
            SlotContext::Section { section_index }
            | SlotContext::GeneratedSection { section_index, .. } => section_index,
            SlotContext::Unmatched => {
                self.status = "Cannot remove columns from unmatched lane".to_string();
                return;
            }
        };
        let Some(view) = self.current_view() else {
            self.status = "No active view".to_string();
            return;
        };
        let Some(section) = view.sections.get(section_index) else {
            self.status = "Current section not found".to_string();
            return;
        };
        let item_board_index = Self::section_item_column_index(section);
        if self.column_index == item_board_index {
            self.status = "Cannot delete Item column (move it with H/L)".to_string();
            return;
        }
        let Some(section_column_index) =
            Self::board_column_to_section_column_index(section, self.column_index)
        else {
            self.status = "Current column is out of range".to_string();
            return;
        };
        let Some(column) = section.columns.get(section_column_index) else {
            self.status = "Current column is out of range".to_string();
            return;
        };
        let label = self
            .categories
            .iter()
            .find(|c| c.id == column.heading)
            .map(|c| c.name.clone())
            .unwrap_or_else(|| "(deleted)".to_string());
        self.board_pending_delete_column_label = Some(label.clone());
        self.mode = Mode::BoardColumnDeleteConfirm;
        self.status = format!("WARNING: Delete column '{label}' from this section? [Y/n]");
    }

    fn remove_current_board_column(&mut self, agenda: &Agenda<'_>) -> TuiResult<()> {
        let Some(slot) = self.current_slot() else {
            self.status = "No active board slot".to_string();
            return Ok(());
        };
        let section_index = match slot.context {
            SlotContext::Section { section_index }
            | SlotContext::GeneratedSection { section_index, .. } => section_index,
            SlotContext::Unmatched => {
                self.status = "Cannot remove columns from unmatched lane".to_string();
                return Ok(());
            }
        };
        let Some(mut view) = self.current_view().cloned() else {
            return Err("No active view".into());
        };
        let Some(section) = view.sections.get_mut(section_index) else {
            return Err("Current section not found".into());
        };
        let item_board_index = Self::section_item_column_index(section);
        if self.column_index == item_board_index {
            self.status = "Cannot delete Item column (move it with H/L)".to_string();
            return Ok(());
        }
        let Some(section_column_index) =
            Self::board_column_to_section_column_index(section, self.column_index)
        else {
            self.status = "Current column is out of range".to_string();
            return Ok(());
        };

        let removed_column = section.columns.remove(section_column_index);
        if section_column_index < item_board_index {
            section.item_column_index = item_board_index.saturating_sub(1);
        } else {
            section.item_column_index = item_board_index.min(section.columns.len());
        }
        let removed_label = self
            .categories
            .iter()
            .find(|c| c.id == removed_column.heading)
            .map(|c| c.name.clone())
            .unwrap_or_else(|| "(deleted)".to_string());

        let view_name = view.name.clone();
        let selected_item_id = self.selected_item_id();
        let old_column_index = self.column_index;
        agenda.store().update_view(&view)?;
        self.refresh(agenda.store())?;
        self.set_view_selection_by_name(&view_name);
        if let Some(item_id) = selected_item_id {
            self.set_item_selection_by_id(item_id);
        }
        self.column_index = old_column_index.min(self.current_slot_column_count());
        self.status = format!("Removed column '{}'", removed_label);
        Ok(())
    }

    pub(crate) fn handle_board_column_delete_confirm_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> TuiResult<bool> {
        match code {
            KeyCode::Char('y') => {
                self.board_pending_delete_column_label = None;
                self.mode = Mode::Normal;
                self.remove_current_board_column(agenda)?;
            }
            KeyCode::Esc => {
                let label = self.board_pending_delete_column_label.take();
                self.mode = Mode::Normal;
                self.status = match label {
                    Some(name) => format!("Delete column '{}' canceled", name),
                    None => "Delete column canceled".to_string(),
                };
            }
            _ => {}
        }
        Ok(false)
    }

    fn confirm_inline_create_board_add_column(&mut self, agenda: &Agenda<'_>) -> TuiResult<()> {
        let Some(name) = self
            .board_add_column_create_confirm_name()
            .map(str::to_string)
        else {
            return Ok(());
        };
        let mut category = Category::new(name.clone());
        category.enable_implicit_string = true;
        let cat_id = category.id;
        agenda.create_category(&category)?;
        self.refresh_category_cache(agenda.store())?;
        self.set_board_add_column_create_confirm_name(None);
        self.insert_board_column_for_category(agenda, cat_id)?;
        if self.mode == Mode::Normal {
            self.status = format!("Created category '{}' and inserted column", name);
        }
        Ok(())
    }

    // TODO(feature): inline direct-edit for board columns not yet invoked from key handler
    #[allow(dead_code)]
    pub(crate) fn open_category_direct_edit(&mut self) {
        let Some(meta) = self.current_category_direct_edit_column_meta() else {
            return;
        };

        if meta.column_kind == ColumnKind::When {
            self.status = "Editing 'When' date not yet implemented inline".to_string();
            return;
        }
        let rows = self.build_current_column_direct_edit_rows();
        let input_value = rows
            .first()
            .map(|row| row.input.text().to_string())
            .unwrap_or_default();

        let original_category_ids: Vec<Option<CategoryId>> =
            rows.iter().map(|r| r.category_id).collect();
        self.mode = Mode::CategoryDirectEdit;
        self.category_direct_edit = Some(CategoryDirectEditState {
            anchor: meta.anchor,
            parent_id: meta.parent_id,
            parent_name: meta.parent_name,
            item_id: meta.item_id,
            item_label: meta.item_label,
            rows,
            active_row: 0,
            focus: CategoryDirectEditFocus::Input,
            suggest_index: 0,
            create_confirm_name: None,
            original_category_ids,
        });
        self.set_input(input_value);
        self.category_suggest = None;
        self.category_direct_edit_create_confirm = None;
        self.status = "Set category: type to filter, Enter assign/create, Esc cancel".to_string();
        self.update_suggestions();
    }

    pub(crate) fn open_category_column_editor(&mut self) {
        let Some(meta) = self.current_category_direct_edit_column_meta() else {
            return;
        };
        if meta.column_kind == ColumnKind::When {
            self.open_when_column_editor(&meta);
            return;
        }

        // Numeric column → open numeric value editor instead of category picker.
        let is_numeric = self
            .categories
            .iter()
            .find(|c| c.id == meta.parent_id)
            .map(|c| c.value_kind == CategoryValueKind::Numeric)
            .unwrap_or(false);
        if is_numeric {
            self.open_numeric_column_editor(&meta);
            return;
        }

        let is_exclusive = self
            .categories
            .iter()
            .find(|c| c.id == meta.parent_id)
            .map(|c| c.is_exclusive)
            .unwrap_or(false);
        let selected_ids: HashSet<CategoryId> = self
            .current_column_assigned_child_ids()
            .into_iter()
            .collect();
        self.mode = Mode::CategoryColumnPicker;
        self.category_column_picker = Some(CategoryColumnPickerState {
            anchor: meta.anchor,
            parent_id: meta.parent_id,
            parent_name: meta.parent_name.clone(),
            item_id: meta.item_id,
            item_label: meta.item_label,
            item_preview_scroll: 0,
            is_exclusive,
            filter: text_buffer::TextBuffer::empty(),
            focus: CategoryColumnPickerFocus::FilterInput,
            list_index: 0,
            selected_ids,
            create_confirm_name: None,
        });
        self.category_suggest = None;
        self.clear_input();
        self.clamp_category_column_picker_list_index();
        self.status = format!(
            "Set {}: type to filter, Space toggle, Enter save, Esc cancel",
            meta.parent_name
        );
    }

    fn open_when_column_editor(&mut self, meta: &CategoryDirectEditColumnMeta) {
        let current_value = self
            .selected_item()
            .and_then(|item| item.when_date)
            .map(|value| value.strftime("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_default();

        self.when_edit_target = Some(WhenEditTarget {
            item_id: meta.item_id,
        });
        self.input_panel = Some(input_panel::InputPanel::new_when_date_input(
            &current_value,
            &meta.item_label,
        ));
        self.input_panel_discard_confirm = false;
        self.name_input_context = Some(NameInputContext::WhenDateEdit);
        self.mode = Mode::InputPanel;
        self.status = String::new();
    }

    fn open_numeric_column_editor(&mut self, meta: &CategoryDirectEditColumnMeta) {
        // Pre-fill with the current numeric value if one exists.
        let current_value = self
            .selected_item()
            .and_then(|item| {
                item.assignments
                    .get(&meta.parent_id)
                    .and_then(|a| a.numeric_value)
            })
            .map(|v| v.to_string())
            .unwrap_or_default();

        self.numeric_edit_target = Some(NumericEditTarget {
            item_id: meta.item_id,
            category_id: meta.parent_id,
        });
        self.input_panel = Some(input_panel::InputPanel::new_numeric_value_input(
            &current_value,
            &format!(
                "Category: {}    Item: {}",
                meta.parent_name,
                truncate_board_cell(&meta.item_label, 32)
            ),
        ));
        self.input_panel_discard_confirm = false;
        self.name_input_context = Some(NameInputContext::NumericValueEdit);
        self.mode = Mode::InputPanel;
        self.status = format!(
            "Set {} value: type a number, Enter saves, Esc cancels",
            meta.parent_name
        );
    }

    fn move_category_column_picker_list(&mut self, delta: i32) {
        let matches = self.category_column_picker_matches();
        let len = matches.len();
        if len == 0 {
            return;
        }
        if let Some(state) = self.category_column_picker_state_mut() {
            let cur = state.list_index.min(len - 1);
            state.list_index = (cur as i64 + delta as i64).rem_euclid(len as i64) as usize;
            state.focus = CategoryColumnPickerFocus::List;
        }
    }

    fn scroll_category_column_picker_item_preview(&mut self, delta: i32) {
        let Some(state) = self.category_column_picker_state_mut() else {
            return;
        };
        match delta.cmp(&0) {
            std::cmp::Ordering::Greater => {
                state.item_preview_scroll = state
                    .item_preview_scroll
                    .saturating_add(delta.unsigned_abs() as u16);
            }
            std::cmp::Ordering::Less => {
                state.item_preview_scroll = state
                    .item_preview_scroll
                    .saturating_sub(delta.unsigned_abs() as u16);
            }
            std::cmp::Ordering::Equal => {}
        }
    }

    fn toggle_category_column_picker_selected(&mut self) {
        let matches = self.category_column_picker_matches();
        let Some(state_ro) = self.category_column_picker_state() else {
            return;
        };
        let Some(&id) = matches.get(state_ro.list_index.min(matches.len().saturating_sub(1)))
        else {
            self.status = "No category to toggle".to_string();
            return;
        };
        let is_exclusive = state_ro.is_exclusive;
        let _ = state_ro;
        if let Some(state) = self.category_column_picker_state_mut() {
            if is_exclusive {
                state.selected_ids.clear();
                state.selected_ids.insert(id);
            } else if !state.selected_ids.insert(id) {
                state.selected_ids.remove(&id);
            }
            state.focus = CategoryColumnPickerFocus::List;
        }
        let label = self
            .categories
            .iter()
            .find(|c| c.id == id)
            .map(|c| c.name.as_str())
            .unwrap_or("(missing)");
        self.status = if is_exclusive {
            format!("Selected '{label}'. Enter saves, Esc cancels")
        } else {
            format!("Toggled '{label}'. Enter saves, Esc cancels")
        };
    }

    fn open_category_column_picker_create_confirm(&mut self) {
        let typed = self
            .category_column_picker_filter_text()
            .unwrap_or("")
            .trim()
            .to_string();
        if typed.is_empty() {
            self.status = "Type a category name first".to_string();
            return;
        }
        if is_reserved_category_name(&typed) {
            self.status = format!(
                "Cannot create reserved category '{}'. Use a different name.",
                typed
            );
            return;
        }
        if let Some(existing_cat) = self
            .categories
            .iter()
            .find(|c| c.name.eq_ignore_ascii_case(&typed))
        {
            let parent_name = existing_cat
                .parent
                .and_then(|pid| self.categories.iter().find(|c| c.id == pid))
                .map(|c| c.name.as_str())
                .unwrap_or("(root)");
            self.status = format!(
                "Category '{}' exists under '{}'. Cannot create duplicate.",
                typed, parent_name
            );
            return;
        }
        self.set_category_column_picker_create_confirm_name(Some(typed.clone()));
        self.status = format!(
            "Create new category '{}' in this column? y:confirm Esc:cancel",
            typed
        );
    }

    fn confirm_inline_create_category_column_picker(
        &mut self,
        agenda: &Agenda<'_>,
    ) -> TuiResult<()> {
        let Some(name) = self
            .category_column_picker_create_confirm_name()
            .map(str::to_string)
        else {
            return Ok(());
        };
        let Some(parent_id) = self.category_column_picker_state().map(|s| s.parent_id) else {
            self.set_category_column_picker_create_confirm_name(None);
            return Ok(());
        };

        let mut category = Category::new(name.clone());
        category.parent = Some(parent_id);
        category.enable_implicit_string = true;
        let cat_id = category.id;
        agenda.create_category(&category)?;
        self.refresh_category_cache(agenda.store())?;
        if let Some(state) = self.category_column_picker_state_mut() {
            if state.is_exclusive {
                state.selected_ids.clear();
            }
            state.selected_ids.insert(cat_id);
            state.create_confirm_name = None;
            state.focus = CategoryColumnPickerFocus::FilterInput;
        }
        self.clamp_category_column_picker_list_index();
        self.status = format!("Created category '{}' and selected it (Enter saves)", name);
        Ok(())
    }

    fn apply_category_column_picker_selection(&mut self, agenda: &Agenda<'_>) -> TuiResult<()> {
        let Some(state) = self.category_column_picker_state().cloned() else {
            return Ok(());
        };
        if state.is_exclusive && state.selected_ids.len() > 1 {
            self.status = "Cannot save: parent category is exclusive".to_string();
            return Ok(());
        }
        let desired_set: HashSet<CategoryId> = state.selected_ids.clone();
        let current_ids = self.current_column_assigned_child_ids();
        let current_set: HashSet<CategoryId> = current_ids.iter().copied().collect();
        let to_remove: Vec<CategoryId> = current_ids
            .iter()
            .copied()
            .filter(|id| !desired_set.contains(id))
            .collect();
        let to_add: Vec<CategoryId> = desired_set
            .iter()
            .copied()
            .filter(|id| !current_set.contains(id))
            .collect();

        let item_id = state.item_id;
        let item_label = state.item_label.clone();
        let view_name = self.current_view().map(|v| v.name.clone());
        let column_index = self.column_index;

        for id in to_remove {
            agenda.unassign_item_manual(item_id, id)?;
        }
        for id in to_add {
            agenda.assign_item_manual(
                item_id,
                id,
                Some("manual:tui.column_picker.multi".to_string()),
            )?;
        }

        self.mode = Mode::Normal;
        self.clear_category_column_picker_session();
        self.refresh(agenda.store())?;
        if let Some(name) = view_name {
            self.set_view_selection_by_name(&name);
        }
        self.set_item_selection_by_id(item_id);
        self.column_index = column_index.min(self.current_slot_column_count());
        self.status = format!(
            "Saved column edits for '{}'",
            truncate_board_cell(&item_label, 40)
        );
        Ok(())
    }

    pub(crate) fn handle_category_column_picker_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> TuiResult<bool> {
        if self.category_column_picker_create_confirm_open() {
            match inline_create_confirm_key_action(code) {
                InlineCreateConfirmKeyAction::Confirm => {
                    self.confirm_inline_create_category_column_picker(agenda)?;
                    return Ok(false);
                }
                InlineCreateConfirmKeyAction::Cancel => {
                    self.set_category_column_picker_create_confirm_name(None);
                    self.status = "Create canceled. Continue editing category.".to_string();
                    return Ok(false);
                }
                InlineCreateConfirmKeyAction::DismissAndContinue => {
                    self.set_category_column_picker_create_confirm_name(None);
                    self.status = "Create canceled. Continue editing category.".to_string();
                }
                InlineCreateConfirmKeyAction::None => {}
            }
        }

        match code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.clear_category_column_picker_session();
                self.status = "Cancelled column edits".to_string();
                return Ok(false);
            }
            KeyCode::Enter => {
                let typed = self
                    .category_column_picker_filter_text()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if self.category_column_picker_matches().is_empty() && !typed.is_empty() {
                    self.open_category_column_picker_create_confirm();
                } else {
                    self.apply_category_column_picker_selection(agenda)?;
                }
                return Ok(false);
            }
            KeyCode::Tab | KeyCode::BackTab => {
                if let Some(state) = self.category_column_picker_state_mut() {
                    state.focus = match state.focus {
                        CategoryColumnPickerFocus::FilterInput => CategoryColumnPickerFocus::List,
                        CategoryColumnPickerFocus::List => CategoryColumnPickerFocus::FilterInput,
                    };
                }
                return Ok(false);
            }
            KeyCode::Up => {
                self.move_category_column_picker_list(-1);
                return Ok(false);
            }
            KeyCode::Down => {
                self.move_category_column_picker_list(1);
                return Ok(false);
            }
            KeyCode::PageUp => {
                self.scroll_category_column_picker_item_preview(-1);
                return Ok(false);
            }
            KeyCode::PageDown => {
                self.scroll_category_column_picker_item_preview(1);
                return Ok(false);
            }
            KeyCode::Char('k')
                if !matches!(
                    self.category_column_picker_state().map(|s| &s.focus),
                    Some(CategoryColumnPickerFocus::FilterInput)
                ) =>
            {
                self.move_category_column_picker_list(-1);
                return Ok(false);
            }
            KeyCode::Char('j')
                if !matches!(
                    self.category_column_picker_state().map(|s| &s.focus),
                    Some(CategoryColumnPickerFocus::FilterInput)
                ) =>
            {
                self.move_category_column_picker_list(1);
                return Ok(false);
            }
            KeyCode::Char(' ') => {
                self.toggle_category_column_picker_selected();
                return Ok(false);
            }
            _ => {}
        }

        let mut edited = false;
        let text_key = self.text_key_event(code);
        if let Some(state) = self.category_column_picker_state_mut() {
            if matches!(state.focus, CategoryColumnPickerFocus::FilterInput) {
                edited = state.filter.handle_key_event(text_key, false);
                if edited {
                    state.create_confirm_name = None;
                }
            }
        }
        if edited {
            self.clamp_category_column_picker_list_index();
            let typed = self
                .category_column_picker_filter_text()
                .unwrap_or("")
                .trim()
                .to_string();
            let no_matches = self.category_column_picker_matches().is_empty();
            self.status = if typed.is_empty() {
                "Type to filter categories. Space toggles highlighted row, Enter saves".to_string()
            } else if no_matches {
                "No categories found. Enter creates a new child category.".to_string()
            } else {
                "Space toggles highlighted category. Enter saves, Esc cancels".to_string()
            };
        }
        Ok(false)
    }

    fn get_current_column_child_ids(&self) -> Vec<CategoryId> {
        let Some(meta) = self.current_category_direct_edit_column_meta() else {
            return Vec::new();
        };
        self.categories
            .iter()
            .find(|c| c.id == meta.parent_id)
            .map(|c| c.children.clone())
            .unwrap_or_default()
    }

    pub(crate) fn get_current_suggest_matches(&self) -> Vec<CategoryId> {
        let child_ids = self.get_current_column_child_ids();
        let active_input = self.active_category_direct_edit_input_text().unwrap_or("");
        filter_category_ids_by_query(&child_ids, &self.categories, active_input, true, true)
    }

    fn update_suggestions(&mut self) {
        if self.direct_edit_create_confirm_open() {
            return;
        }
        let text = self
            .active_category_direct_edit_input_text()
            .unwrap_or("")
            .to_string();
        let matches = self.get_current_suggest_matches();
        if matches.is_empty() {
            self.category_suggest = None;
            if let Some(state) = self.category_direct_edit_state_mut() {
                state.suggest_index = 0;
            }
            self.status = if text.trim().is_empty() {
                "No categories in this column yet. Enter clears current value.".to_string()
            } else {
                "No categories found. Enter creates a new child category.".to_string()
            };
        } else {
            if let Some(state) = self.category_direct_edit_state_mut() {
                state.suggest_index = state.suggest_index.min(matches.len() - 1);
            }
            self.category_suggest = Some(CategorySuggestState { suggest_index: 0 });
            self.status =
                "Choose category: type to narrow, ↑↓ select, Enter apply, Esc cancel".to_string();
        }
    }

    fn move_suggest_cursor(&mut self, delta: i32) {
        if self.direct_edit_create_confirm_open() {
            return;
        }
        let matches = self.get_current_suggest_matches();
        let len = matches.len();
        if len == 0 {
            return;
        }
        let Some(state) = self.category_direct_edit_state() else {
            return;
        };
        let current_idx = state.suggest_index.min(len - 1);
        let new_idx = (current_idx as i64 + delta as i64).rem_euclid(len as i64) as usize;
        if let Some(state) = self.category_direct_edit_state_mut() {
            state.suggest_index = new_idx;
        }
    }

    fn autocomplete_from_suggestion(&mut self) {
        if self.direct_edit_create_confirm_open() {
            return;
        }
        let matches = self.get_current_suggest_matches();
        let Some(state) = self.category_direct_edit_state() else {
            return;
        };
        let Some(&id) = matches.get(state.suggest_index.min(matches.len().saturating_sub(1)))
        else {
            return;
        };
        let Some(cat_name) = self
            .categories
            .iter()
            .find(|c| c.id == id)
            .map(|c| c.name.clone())
        else {
            return;
        };
        if let Some(row) = self.active_category_direct_edit_row_mut() {
            row.input.set(cat_name);
        }
        self.update_suggestions();
    }

    fn assign_selected_suggestion(&mut self, _agenda: &Agenda<'_>) -> TuiResult<()> {
        self.resolve_active_row_from_highlighted_suggestion()?;
        Ok(())
    }

    fn exact_current_column_child_match_id(&self) -> Option<CategoryId> {
        let child_ids = self.get_current_column_child_ids();
        exact_category_name_match_in_scope(
            &child_ids,
            &self.categories,
            self.active_category_direct_edit_input_text()?,
        )
    }

    fn confirm_inline_create_category_direct_edit(&mut self, agenda: &Agenda<'_>) -> TuiResult<()> {
        let Some(name) = self.direct_edit_create_confirm_name().map(str::to_string) else {
            return Ok(());
        };
        let child_parent_id = self.current_view().and_then(|view| {
            self.current_slot().and_then(|slot| {
                let section = match slot.context {
                    SlotContext::Section { section_index }
                    | SlotContext::GeneratedSection { section_index, .. } => {
                        view.sections.get(section_index)
                    }
                    _ => None,
                }?;
                let section_column_index =
                    Self::board_column_to_section_column_index(section, self.column_index)?;
                Some(section.columns.get(section_column_index)?.heading)
            })
        });
        let Some(parent_id) = child_parent_id else {
            self.set_direct_edit_create_confirm_name(None);
            return Ok(());
        };

        let mut category = Category::new(name.clone());
        category.parent = Some(parent_id);
        category.enable_implicit_string = true;
        let cat_id = category.id;
        agenda.create_category(&category)?;
        self.refresh_category_cache(agenda.store())?;
        self.set_direct_edit_create_confirm_name(None);
        let _ = self.resolve_active_category_direct_edit_row(cat_id)?;
        self.status = format!("Created category '{}' and resolved current row", name);
        Ok(())
    }

    fn apply_category_direct_edit_draft(&mut self, agenda: &Agenda<'_>) -> TuiResult<()> {
        if self.direct_edit_create_confirm_open() {
            self.status = "Confirm or cancel category creation before saving".to_string();
            return Ok(());
        }
        if self.category_direct_edit_has_unresolved_nonempty_rows() {
            self.status = "Resolve or clear all non-empty rows before saving".to_string();
            return Ok(());
        }
        if self
            .category_direct_edit_state()
            .map(|s| s.has_duplicate_resolved_category_ids())
            .unwrap_or(false)
        {
            self.status =
                "Duplicate categories in draft; remove duplicates before saving".to_string();
            return Ok(());
        }

        let desired_ids = self.desired_child_ids_from_category_direct_edit_draft();
        if self.current_column_parent_is_exclusive() && desired_ids.len() > 1 {
            self.status =
                "Cannot save: parent category is exclusive (only one row may be resolved)"
                    .to_string();
            return Ok(());
        }

        let current_ids = self.current_column_assigned_child_ids();
        let current_set: HashSet<CategoryId> = current_ids.iter().copied().collect();
        let desired_set: HashSet<CategoryId> = desired_ids.iter().copied().collect();
        let to_remove: Vec<CategoryId> = current_ids
            .iter()
            .copied()
            .filter(|id| !desired_set.contains(id))
            .collect();
        let to_add: Vec<CategoryId> = desired_ids
            .iter()
            .copied()
            .filter(|id| !current_set.contains(id))
            .collect();

        let Some(state) = self.category_direct_edit_state() else {
            return Ok(());
        };
        let item_id = state.item_id;
        let item_label = state.item_label.clone();
        let view_name = self.current_view().map(|v| v.name.clone());
        let column_index = self.column_index;

        for id in to_remove {
            agenda.unassign_item_manual(item_id, id)?;
        }
        for id in to_add {
            agenda.assign_item_manual(
                item_id,
                id,
                Some("manual:tui.direct_edit.multi".to_string()),
            )?;
        }

        self.mode = Mode::Normal;
        self.clear_input();
        self.clear_category_direct_edit_session();
        self.refresh(agenda.store())?;
        if let Some(name) = view_name {
            self.set_view_selection_by_name(&name);
        }
        self.set_item_selection_by_id(item_id);
        self.column_index = column_index.min(self.current_slot_column_count());
        self.status = format!(
            "Saved column edits for '{}'",
            truncate_board_cell(&item_label, 40)
        );
        Ok(())
    }

    pub(crate) fn handle_normal_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> TuiResult<bool> {
        if let Some(prefix) = self.normal_mode_prefix.take() {
            match (prefix, code) {
                (NormalModePrefix::G, KeyCode::Char('a')) => {
                    self.jump_to_all_items_view(agenda)?;
                    self.status = "Jumped to All Items view".to_string();
                    return Ok(false);
                }
                (NormalModePrefix::G, KeyCode::Char('/')) => {
                    self.begin_global_search_session(agenda)?;
                    return Ok(false);
                }
                (NormalModePrefix::G, KeyCode::Esc) => {
                    self.status = "Cancelled g-prefix command".to_string();
                    return Ok(false);
                }
                (NormalModePrefix::G, _) => {
                    self.status = "Unknown g command (use ga or g/)".to_string();
                    return Ok(false);
                }
            }
        }
        match code {
            KeyCode::Char('q') => return Ok(true),
            KeyCode::Char(' ') => {
                if let Some(item_id) = self.selected_item_id() {
                    let is_selected = self.toggle_selected_item(item_id);
                    let selected_count = self.selected_count();
                    let item_suffix = if selected_count == 1 { "" } else { "s" };
                    self.status = if is_selected {
                        format!(
                            "Selected {selected_count} item{item_suffix} (Space toggles, Esc clears selection)"
                        )
                    } else if selected_count == 0 {
                        "Selection cleared".to_string()
                    } else {
                        format!(
                            "Selected {selected_count} item{item_suffix} (Space toggles, Esc clears selection)"
                        )
                    };
                } else {
                    self.status = "No selected item to toggle".to_string();
                }
            }
            KeyCode::Down if self.current_key_modifiers.contains(KeyModifiers::SHIFT) => {
                self.move_selected_item_between_slots(1, agenda)?;
            }
            KeyCode::Up if self.current_key_modifiers.contains(KeyModifiers::SHIFT) => {
                self.move_selected_item_between_slots(-1, agenda)?;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.show_preview && self.normal_focus == NormalFocus::Preview {
                    self.scroll_preview(1);
                } else {
                    self.move_item_cursor(1);
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.show_preview && self.normal_focus == NormalFocus::Preview {
                    self.scroll_preview(-1);
                } else {
                    self.move_item_cursor(-1);
                }
            }
            KeyCode::Right | KeyCode::Char('l') => {
                if self.is_horizontal_section_flow() {
                    self.move_slot_cursor(1);
                } else {
                    let max_cols = self.current_slot_column_count();
                    if self.column_index < max_cols {
                        self.column_index += 1;
                    } else {
                        self.move_slot_cursor(1);
                    }
                }
            }
            KeyCode::Left | KeyCode::Char('h') => {
                if self.is_horizontal_section_flow() {
                    self.move_slot_cursor(-1);
                } else if self.column_index > 0 {
                    self.column_index -= 1;
                } else {
                    self.move_slot_cursor(-1);
                }
            }
            KeyCode::Char('H') => {
                self.move_current_board_column_relative(-1, agenda)?;
            }
            KeyCode::Char('L') => {
                self.move_current_board_column_relative(1, agenda)?;
            }
            KeyCode::Char('+') => {
                if let Err(err) = self.open_board_add_column_picker(AddColumnDirection::Right) {
                    self.status = err.to_string();
                }
            }
            KeyCode::Char('-') => {
                self.open_remove_current_board_column_confirm();
            }
            KeyCode::Char('n') => {
                self.open_input_panel_add_item();
            }
            KeyCode::Char('?') => {
                self.mode = Mode::HelpPanel;
            }
            KeyCode::Char('m') => {
                self.toggle_current_view_section_flow(agenda)?;
            }
            KeyCode::Char('z') => {
                self.cycle_current_board_display_mode(agenda)?;
            }
            KeyCode::Char('s') | KeyCode::Char('<') => {
                self.sort_current_slot_by_active_column(None, agenda)?;
            }
            KeyCode::Char('S') | KeyCode::Char('>') => {
                self.sort_current_slot_by_active_column(Some(SlotSortDirection::Desc), agenda)?;
            }
            KeyCode::Char('F') => {
                self.cycle_column_summary_fn(agenda)?;
            }
            KeyCode::Char('b') => {
                self.open_link_wizard(LinkWizardAction::BlockedBy);
            }
            KeyCode::Char('B') => {
                self.open_link_wizard(LinkWizardAction::Blocks);
            }
            KeyCode::Char('e') => {
                self.open_input_panel_edit_item();
            }
            KeyCode::Char('C') => {
                self.open_suggestion_review(agenda)?;
            }
            KeyCode::Enter => {
                if self.column_index != self.current_slot_item_column_index() {
                    self.open_category_column_editor();
                } else if self.selected_item_id().is_none() {
                    self.open_input_panel_add_item();
                } else {
                    self.open_input_panel_edit_item();
                }
            }
            KeyCode::Char('/') => {
                self.mode = Mode::SearchBarFocused;
                // Load existing filter text if search_buffer is empty
                if self.search_buffer.is_empty() {
                    if let Some(existing) = self
                        .section_filters
                        .get(self.slot_index)
                        .and_then(|f| f.clone())
                    {
                        self.search_buffer.set(existing);
                    }
                }
            }
            KeyCode::Esc => {
                if self.has_selected_items() {
                    let cleared_count = self.clear_selected_items();
                    let item_suffix = if cleared_count == 1 { "" } else { "s" };
                    self.status = format!("Cleared selection ({cleared_count} item{item_suffix})");
                } else if self.global_search_active() {
                    self.restore_global_search_session(agenda)?;
                } else {
                    self.search_buffer.clear();
                    let target = self.slot_index;
                    if target < self.section_filters.len()
                        && self.section_filters[target].take().is_some()
                    {
                        self.refresh(agenda.store())?;
                        self.status = "Filter cleared".to_string();
                    }
                }
            }
            KeyCode::F(8) | KeyCode::Char('v') | KeyCode::Char('V') => {
                self.mode = Mode::ViewPicker;
                self.picker_index = self.view_index;
                self.status =
                    "View palette: Enter switch, n create, r rename, x delete, e edit view, Esc cancel"
                        .to_string();
            }
            KeyCode::F(9) | KeyCode::Char('c') => {
                self.mode = Mode::CategoryManager;
                self.open_category_manager_session();
                self.status =
                    "Category manager: Enter focuses details pane, e/i/a quick toggles, n/N create, r rename, x delete, H/J/K/L move, << / >> shift level".to_string();
            }
            KeyCode::Char(',') => {
                self.cycle_view(-1, agenda)?;
            }
            KeyCode::Char('.') => {
                self.cycle_view(1, agenda)?;
            }
            KeyCode::Tab => self.move_slot_cursor(1),
            KeyCode::BackTab => self.move_slot_cursor(-1),
            KeyCode::Char('f') => {
                self.cycle_column_numeric_format(agenda)?;
            }
            KeyCode::Char('g') => {
                self.normal_mode_prefix = Some(NormalModePrefix::G);
                self.status = "g-prefix: ga=All Items, g/=Global search".to_string();
            }
            KeyCode::Char('a') => {
                if self.selected_item_id().is_none() {
                    self.status = "No selected item to edit categories".to_string();
                } else if self.category_rows.is_empty() {
                    self.status = "No categories available".to_string();
                } else {
                    self.mode = Mode::ItemAssignPicker;
                    self.item_assign_category_index =
                        first_non_reserved_category_index(&self.category_rows);
                    self.item_assign_dirty = false;
                    self.clear_input();
                    self.status = if self.has_selected_items() {
                        let selected_count = self.selected_count();
                        let item_suffix = if selected_count == 1 { "" } else { "s" };
                        format!(
                            "Batch categories for {selected_count} selected item{item_suffix}: j/k select, Space apply, n or / type category, Enter close, Esc cancel"
                        )
                    } else {
                        "Item categories: j/k select, Space apply, n or / type category, Enter close, Esc cancel"
                            .to_string()
                    };
                }
            }
            KeyCode::Char('u') => {
                let persisted = self
                    .current_view()
                    .map(|view| view.hide_dependent_items)
                    .unwrap_or(false);
                let next_hide_dependent = !self.effective_hide_dependent_items();
                self.session_hide_dependent_items_override = if next_hide_dependent == persisted {
                    None
                } else {
                    Some(next_hide_dependent)
                };
                self.refresh(agenda.store())?;
                self.status = format!(
                    "Hide dependent items: {} (session-only)",
                    if next_hide_dependent { "ON" } else { "OFF" }
                );
            }
            KeyCode::Char('p') => self.toggle_preview(),
            KeyCode::Char('i') => self.toggle_preview_mode(),
            KeyCode::Char('o') => self.toggle_preview_mode(),
            KeyCode::Char('J') => {
                self.scroll_preview(1);
            }
            KeyCode::Char('K') => {
                self.scroll_preview(-1);
            }
            KeyCode::Char('r') => {
                if let Some(item_id) = self.selected_item_id() {
                    if let Some(view) = self.current_view().cloned() {
                        agenda.remove_item_from_view(item_id, &view)?;
                        self.refresh(agenda.store())?;
                        self.status = "Removed item from current view".to_string();
                    }
                }
            }
            KeyCode::Char('d') | KeyCode::Char('D') => {
                if self.selected_count() > 1 {
                    self.batch_toggle_selected_items_done(agenda, DoneToggleOrigin::NormalMode)?;
                } else if let Some(item_id) = self.selected_item_id() {
                    self.begin_done_toggle_or_confirm(
                        agenda,
                        item_id,
                        DoneToggleOrigin::NormalMode,
                    )?;
                }
            }
            KeyCode::Char('x') => {
                if self.has_selected_items() {
                    self.done_blocks_confirm = None;
                    self.batch_delete_item_ids = Some(self.selected_item_ids_in_view_order());
                    let selected_count = self.selected_count();
                    let item_suffix = if selected_count == 1 { "" } else { "s" };
                    self.mode = Mode::ConfirmDelete;
                    self.status =
                        format!("Delete {selected_count} selected item{item_suffix}? y/n");
                } else if self.selected_item_id().is_some() {
                    self.done_blocks_confirm = None;
                    self.batch_delete_item_ids = None;
                    self.mode = Mode::ConfirmDelete;
                    self.status = "Delete item? y/n".to_string();
                }
            }
            KeyCode::Char(']') => {
                self.move_selected_item_between_slots(1, agenda)?;
            }
            KeyCode::Char('[') => {
                self.move_selected_item_between_slots(-1, agenda)?;
            }
            _ => {}
        }

        Ok(false)
    }

    pub(crate) fn done_toggle_return_mode(origin: DoneToggleOrigin) -> Mode {
        match origin {
            DoneToggleOrigin::NormalMode => Mode::Normal,
            DoneToggleOrigin::ItemAssignPicker => Mode::ItemAssignPicker,
        }
    }

    fn done_toggle_status_message(
        origin: DoneToggleOrigin,
        was_done: bool,
        removed_blocker_links: usize,
    ) -> String {
        if was_done {
            return match origin {
                DoneToggleOrigin::NormalMode => "Marked item not-done".to_string(),
                DoneToggleOrigin::ItemAssignPicker => {
                    "Removed category Done (marked not-done)".to_string()
                }
            };
        }

        if removed_blocker_links > 0 {
            let suffix = if removed_blocker_links == 1 { "" } else { "s" };
            return match origin {
                DoneToggleOrigin::NormalMode => format!(
                    "Marked item done and removed {removed_blocker_links} blocking link{suffix}"
                ),
                DoneToggleOrigin::ItemAssignPicker => format!(
                    "Assigned item to category Done (marked done; removed {removed_blocker_links} blocking link{suffix})"
                ),
            };
        }

        match origin {
            DoneToggleOrigin::NormalMode => "Marked item done".to_string(),
            DoneToggleOrigin::ItemAssignPicker => {
                "Assigned item to category Done (marked done)".to_string()
            }
        }
    }

    pub(crate) fn apply_done_toggle_action(
        &mut self,
        agenda: &Agenda<'_>,
        item_id: ItemId,
        was_done: bool,
        origin: DoneToggleOrigin,
        clear_blocked_item_ids: &[ItemId],
    ) -> TuiResult<()> {
        self.push_undo(UndoEntry::ItemDoneToggled { item_id, was_done });
        agenda.toggle_item_done(item_id)?;

        let mut removed_blocker_links = 0usize;
        if !was_done {
            for blocked_id in clear_blocked_item_ids {
                agenda.unlink_items_blocks(item_id, *blocked_id)?;
                removed_blocker_links += 1;
            }
        }

        self.refresh(agenda.store())?;
        self.set_item_selection_by_id(item_id);
        self.mode = Self::done_toggle_return_mode(origin);
        self.status = Self::done_toggle_status_message(origin, was_done, removed_blocker_links);
        Ok(())
    }

    fn begin_done_toggle_or_confirm(
        &mut self,
        agenda: &Agenda<'_>,
        item_id: ItemId,
        origin: DoneToggleOrigin,
    ) -> TuiResult<()> {
        let was_done = self
            .selected_item()
            .map(|item| item.is_done)
            .unwrap_or(false);
        if !was_done && !self.selected_item_has_actionable_assignment() {
            self.status =
                "Done unavailable: item has no actionable category assignments".to_string();
            return Ok(());
        }

        if !was_done {
            let blocked_item_ids = self
                .item_links_by_item_id
                .get(&item_id)
                .map(|links| links.blocks.clone())
                .unwrap_or_default();
            if !blocked_item_ids.is_empty() {
                let blocked_count = blocked_item_ids.len();
                let suffix = if blocked_count == 1 { "" } else { "s" };
                self.done_blocks_confirm = Some(DoneBlocksConfirmState {
                    scope: DoneBlocksConfirmScope::Single {
                        item_id,
                        blocked_item_ids,
                    },
                    origin,
                });
                self.mode = Mode::ConfirmDelete;
                self.status = format!(
                    "This item blocks {blocked_count} other item{suffix}. Remove that link and mark done?"
                );
                return Ok(());
            }
        }

        self.apply_done_toggle_action(agenda, item_id, was_done, origin, &[])
    }

    fn batch_toggle_selected_items_done(
        &mut self,
        agenda: &Agenda<'_>,
        origin: DoneToggleOrigin,
    ) -> TuiResult<()> {
        let action_item_ids = self.selected_item_ids_in_view_order();
        if action_item_ids.is_empty() {
            self.status = "No selected items to update".to_string();
            return Ok(());
        }

        let mark_done = !action_item_ids.iter().all(|item_id| {
            self.all_items
                .iter()
                .find(|item| item.id == *item_id)
                .is_some_and(|item| item.is_done)
        });
        if mark_done {
            let mut blocking_item_count = 0usize;
            let mut blocked_link_count = 0usize;
            for item_id in &action_item_ids {
                let Some(item) = self
                    .all_items
                    .iter()
                    .find(|candidate| candidate.id == *item_id)
                else {
                    continue;
                };
                if item.is_done {
                    continue;
                }
                let has_actionable_assignment = item.assignments.keys().any(|category_id| {
                    self.categories
                        .iter()
                        .find(|category| category.id == *category_id)
                        .is_some_and(|category| category.is_actionable)
                });
                if !has_actionable_assignment {
                    continue;
                }
                let blocked_count = self
                    .item_links_by_item_id
                    .get(item_id)
                    .map(|links| links.blocks.len())
                    .unwrap_or(0);
                if blocked_count > 0 {
                    blocking_item_count += 1;
                    blocked_link_count += blocked_count;
                }
            }

            if blocked_link_count > 0 {
                let item_suffix = if blocking_item_count == 1 { "" } else { "s" };
                let blocked_suffix = if blocked_link_count == 1 { "" } else { "s" };
                self.done_blocks_confirm = Some(DoneBlocksConfirmState {
                    scope: DoneBlocksConfirmScope::Batch {
                        item_ids: action_item_ids,
                        blocking_item_count,
                        blocked_link_count,
                    },
                    origin,
                });
                self.mode = Mode::ConfirmDelete;
                self.status = format!(
                    "{blocking_item_count} selected item{item_suffix} blocks {blocked_link_count} other item{blocked_suffix}. Remove those links and mark done?"
                );
                return Ok(());
            }
        }

        self.apply_batch_done_action(agenda, &action_item_ids, false, origin)
    }

    pub(crate) fn apply_batch_done_action(
        &mut self,
        agenda: &Agenda<'_>,
        item_ids: &[ItemId],
        remove_blocking_links: bool,
        origin: DoneToggleOrigin,
    ) -> TuiResult<()> {
        if item_ids.is_empty() {
            self.status = "No selected items to update".to_string();
            self.mode = Self::done_toggle_return_mode(origin);
            return Ok(());
        }

        let mark_done = !item_ids.iter().all(|item_id| {
            self.all_items
                .iter()
                .find(|item| item.id == *item_id)
                .is_some_and(|item| item.is_done)
        });
        let anchor_id = self.selected_item_id().unwrap_or(item_ids[0]);
        let mut changed = 0usize;
        let mut skipped = 0usize;
        let mut failed = 0usize;
        let mut removed_links = 0usize;
        let mut first_error = None;
        let mut toggled_ids: Vec<ItemId> = Vec::new();

        for item_id in item_ids {
            let Some(item) = self
                .all_items
                .iter()
                .find(|candidate| candidate.id == *item_id)
            else {
                failed += 1;
                if first_error.is_none() {
                    first_error = Some(format!("item {} is no longer available", item_id));
                }
                continue;
            };

            if mark_done {
                if item.is_done {
                    skipped += 1;
                    continue;
                }
                let has_actionable_assignment = item.assignments.keys().any(|category_id| {
                    self.categories
                        .iter()
                        .find(|category| category.id == *category_id)
                        .is_some_and(|category| category.is_actionable)
                });
                if !has_actionable_assignment {
                    failed += 1;
                    if first_error.is_none() {
                        first_error = Some(format!(
                            "item '{}' has no actionable category assignments",
                            truncate_board_cell(&item.text, 24)
                        ));
                    }
                    continue;
                }
            } else if !item.is_done {
                skipped += 1;
                continue;
            }

            match agenda.toggle_item_done(*item_id) {
                Ok(_) => {
                    changed += 1;
                    toggled_ids.push(*item_id);
                    if mark_done && remove_blocking_links {
                        let blocked_ids = self
                            .item_links_by_item_id
                            .get(item_id)
                            .map(|links| links.blocks.clone())
                            .unwrap_or_default();
                        for blocked_id in &blocked_ids {
                            agenda.unlink_items_blocks(*item_id, *blocked_id)?;
                            removed_links += 1;
                        }
                    }
                }
                Err(err) => {
                    failed += 1;
                    if first_error.is_none() {
                        first_error = Some(err.to_string());
                    }
                }
            }
        }

        if !toggled_ids.is_empty() {
            self.push_undo(UndoEntry::BatchDone {
                item_ids: toggled_ids,
            });
        }

        self.refresh(agenda.store())?;
        self.set_item_selection_by_id(anchor_id);
        let clear_selection = failed == 0 && changed > 0;
        if clear_selection {
            self.clear_selected_items();
        }

        let mut summary = format!(
            "{} {} selected items {} (changed={changed}, skipped={skipped}, failed={failed}",
            if mark_done { "Marked" } else { "Unmarked" },
            item_ids.len(),
            if mark_done { "done" } else { "not-done" },
        );
        if remove_blocking_links {
            summary.push_str(&format!(", removed_links={removed_links}"));
        }
        summary.push(')');
        if let Some(err) = first_error {
            summary.push_str(&format!(" first_error={err}"));
        }
        self.status = summary;
        self.mode = if clear_selection {
            Mode::Normal
        } else {
            Self::done_toggle_return_mode(origin)
        };
        Ok(())
    }

    fn sort_current_slot_by_active_column(
        &mut self,
        preferred_direction: Option<SlotSortDirection>,
        agenda: &Agenda<'_>,
    ) -> TuiResult<()> {
        let Some(column) = self.current_slot_sort_column() else {
            self.status = "Cannot sort current lane by this column".to_string();
            return Ok(());
        };
        let selected_item_id = self.selected_item_id();
        if self.slot_sort_keys.len() != self.slots.len() {
            self.slot_sort_keys = vec![Vec::new(); self.slots.len()];
        }
        let slot_index = self
            .slot_index
            .min(self.slot_sort_keys.len().saturating_sub(1));
        {
            let sort_keys = self
                .slot_sort_keys
                .get_mut(slot_index)
                .ok_or("Sort state unavailable".to_string())?;

            if sort_keys.is_empty() {
                sort_keys.push(SlotSortKey {
                    column,
                    direction: preferred_direction.unwrap_or(SlotSortDirection::Asc),
                });
            } else if sort_keys[0].column == column {
                let current_direction = sort_keys[0].direction;
                if let Some(direction) = preferred_direction {
                    if current_direction == direction {
                        sort_keys.remove(0);
                    } else {
                        sort_keys[0].direction = direction;
                    }
                } else {
                    match current_direction {
                        SlotSortDirection::Asc => sort_keys[0].direction = SlotSortDirection::Desc,
                        SlotSortDirection::Desc => {
                            sort_keys.remove(0);
                        }
                    }
                }
            } else if let Some(existing_index) =
                sort_keys.iter().position(|key| key.column == column)
            {
                let mut key = sort_keys.remove(existing_index);
                key.direction = preferred_direction.unwrap_or(SlotSortDirection::Asc);
                sort_keys.insert(0, key);
            } else {
                sort_keys.insert(
                    0,
                    SlotSortKey {
                        column,
                        direction: preferred_direction.unwrap_or(SlotSortDirection::Asc),
                    },
                );
            }
        }

        self.refresh(agenda.store())?;
        if let Some(item_id) = selected_item_id {
            self.set_item_selection_by_id(item_id);
        }
        self.status = self.describe_current_slot_sort("Sorted current lane");
        Ok(())
    }

    fn cycle_column_summary_fn(&mut self, agenda: &Agenda<'_>) -> TuiResult<()> {
        let slot = match self.current_slot() {
            Some(s) => s,
            None => return Ok(()),
        };
        let section_index = match slot.context {
            SlotContext::Section { section_index }
            | SlotContext::GeneratedSection { section_index, .. } => section_index,
            SlotContext::Unmatched => {
                self.status = "No columns on unmatched lane".to_string();
                return Ok(());
            }
        };
        let mut view = match self.current_view().cloned() {
            Some(v) => v,
            None => return Ok(()),
        };
        let section = match view.sections.get(section_index) {
            Some(s) => s,
            None => return Ok(()),
        };
        let section_column_index =
            match Self::board_column_to_section_column_index(section, self.column_index) {
                Some(i) => i,
                None => {
                    self.status = "No column selected".to_string();
                    return Ok(());
                }
            };
        let column = match section.columns.get(section_column_index) {
            Some(c) => c,
            None => return Ok(()),
        };
        let is_numeric = self
            .categories
            .iter()
            .find(|c| c.id == column.heading)
            .map(|c| c.value_kind == CategoryValueKind::Numeric)
            .unwrap_or(false);
        if !is_numeric {
            self.status = "Summary functions only apply to numeric columns".to_string();
            return Ok(());
        }
        let current = column.summary_fn.unwrap_or(SummaryFn::None);
        let next = current.next();
        let section_mut = &mut view.sections[section_index];
        section_mut.columns[section_column_index].summary_fn = Some(next);
        agenda.store().update_view(&view)?;
        self.refresh(agenda.store())?;
        self.status = format!("Column summary: {}", next.label());
        Ok(())
    }

    fn cycle_column_numeric_format(&mut self, agenda: &Agenda<'_>) -> TuiResult<()> {
        let slot = match self.current_slot() {
            Some(s) => s,
            None => return Ok(()),
        };
        let section_index = match slot.context {
            SlotContext::Section { section_index }
            | SlotContext::GeneratedSection { section_index, .. } => section_index,
            SlotContext::Unmatched => {
                self.status = "No columns on unmatched lane".to_string();
                return Ok(());
            }
        };
        let view = match self.current_view() {
            Some(v) => v,
            None => return Ok(()),
        };
        let section = match view.sections.get(section_index) {
            Some(s) => s,
            None => return Ok(()),
        };
        let section_column_index =
            match Self::board_column_to_section_column_index(section, self.column_index) {
                Some(i) => i,
                None => {
                    self.status = "No column selected".to_string();
                    return Ok(());
                }
            };
        let column = match section.columns.get(section_column_index) {
            Some(c) => c,
            None => return Ok(()),
        };
        let category = match self.categories.iter().find(|c| c.id == column.heading) {
            Some(c) => c,
            None => return Ok(()),
        };
        if category.value_kind != CategoryValueKind::Numeric {
            self.status = "Format cycling only applies to numeric columns".to_string();
            return Ok(());
        }
        let mut updated_cat = category.clone();
        let current = updated_cat.numeric_format.clone().unwrap_or_default();
        let next = cycle_numeric_format_preset(&current);
        updated_cat.numeric_format = Some(next.clone());
        // Use store directly — format-only change needs no reclassification.
        agenda.store().update_category(&updated_cat)?;
        self.refresh(agenda.store())?;
        self.status = format!("Column format: {}", describe_numeric_format(&next));
        Ok(())
    }

    fn cycle_current_board_display_mode(&mut self, agenda: &Agenda<'_>) -> TuiResult<()> {
        let Some(mut view) = self.current_view().cloned() else {
            return Ok(());
        };

        let selected_slot_index = self.slot_index;
        let selected_item_id = self.selected_item_id();
        let mut status_prefix = "View card display";
        let next_mode = match self.current_slot().map(|slot| slot.context.clone()) {
            Some(SlotContext::Section { section_index })
            | Some(SlotContext::GeneratedSection { section_index, .. }) => {
                if let Some(section) = view.sections.get_mut(section_index) {
                    if let Some(current) = section.board_display_mode_override {
                        let next = toggle_board_display_mode(current);
                        section.board_display_mode_override = Some(next);
                        status_prefix = "Lane card display";
                        next
                    } else {
                        let next = toggle_board_display_mode(view.board_display_mode);
                        view.board_display_mode = next;
                        next
                    }
                } else {
                    let next = toggle_board_display_mode(view.board_display_mode);
                    view.board_display_mode = next;
                    next
                }
            }
            None | Some(SlotContext::Unmatched) => {
                let next = toggle_board_display_mode(view.board_display_mode);
                view.board_display_mode = next;
                next
            }
        };

        let view_name = view.name.clone();
        agenda.store().update_view(&view)?;
        self.refresh(agenda.store())?;
        self.set_view_selection_by_name(&view_name);
        self.slot_index = selected_slot_index.min(self.slots.len().saturating_sub(1));
        if let Some(item_id) = selected_item_id {
            self.set_item_selection_by_id(item_id);
        }
        self.status = format!(
            "{status_prefix}: {}",
            match next_mode {
                BoardDisplayMode::SingleLine => "single-line",
                BoardDisplayMode::MultiLine => "multi-line",
            }
        );
        Ok(())
    }

    fn toggle_current_view_section_flow(&mut self, agenda: &Agenda<'_>) -> TuiResult<()> {
        let Some(mut view) = self.current_view().cloned() else {
            return Ok(());
        };

        let selected_slot_index = self.slot_index;
        let selected_item_id = self.selected_item_id();
        view.section_flow = match view.section_flow {
            SectionFlow::Vertical => SectionFlow::Horizontal,
            SectionFlow::Horizontal => SectionFlow::Vertical,
        };
        let next_flow = view.section_flow;
        let view_name = view.name.clone();

        agenda.store().update_view(&view)?;
        self.refresh(agenda.store())?;
        self.set_view_selection_by_name(&view_name);
        self.slot_index = selected_slot_index.min(self.slots.len().saturating_sub(1));
        if let Some(item_id) = selected_item_id {
            self.set_item_selection_by_id(item_id);
        }
        self.status = format!(
            "Board layout: {}",
            match next_flow {
                SectionFlow::Vertical => "vertical",
                SectionFlow::Horizontal => "horizontal",
            }
        );
        Ok(())
    }

    fn describe_current_slot_sort(&self, prefix: &str) -> String {
        let Some(sort_keys) = self.slot_sort_keys.get(self.slot_index) else {
            return prefix.to_string();
        };
        if sort_keys.is_empty() {
            return format!("{prefix}: none");
        }
        let mut labels = Vec::with_capacity(sort_keys.len());
        for key in sort_keys {
            labels.push(self.sort_key_label(key));
        }
        format!("{prefix}: {}", labels.join(" then "))
    }

    fn sort_key_label(&self, key: &SlotSortKey) -> String {
        let direction_label = match key.direction {
            SlotSortDirection::Asc => "asc",
            SlotSortDirection::Desc => "desc",
        };
        let column_label = match key.column {
            SlotSortColumn::ItemText => "Item".to_string(),
            SlotSortColumn::SectionColumn { heading, .. } => self
                .categories
                .iter()
                .find(|category| category.id == heading)
                .map(|category| category.name.clone())
                .unwrap_or_else(|| heading.to_string()),
        };
        format!("{column_label} {direction_label}")
    }

    fn open_link_wizard(&mut self, default_action: LinkWizardAction) {
        let Some(anchor_item_id) = self.selected_item_id() else {
            self.status = "No selected item to link".to_string();
            return;
        };
        let source_item_ids = self.effective_action_item_ids();
        self.link_wizard = Some(LinkWizardState {
            anchor_item_id,
            source_item_ids: source_item_ids.clone(),
            focus: LinkWizardFocus::ScopeAction,
            action_index: default_action.index(),
            target_filter: text_buffer::TextBuffer::empty(),
            target_index: 0,
        });
        self.mode = Mode::LinkWizard;
        let anchor_label = self
            .selected_item()
            .map(board_item_label)
            .unwrap_or_else(|| anchor_item_id.to_string());
        self.status = if source_item_ids.len() > 1 {
            format!(
                "Link wizard for {} selected items: choose relation, target, then Enter to apply",
                source_item_ids.len()
            )
        } else {
            format!(
                "Link wizard for '{}': choose relation, target, then Enter to apply",
                truncate_board_cell(&anchor_label, 40)
            )
        };
    }

    pub(crate) fn link_wizard_state(&self) -> Option<&LinkWizardState> {
        self.link_wizard.as_ref()
    }

    fn link_wizard_state_mut(&mut self) -> Option<&mut LinkWizardState> {
        self.link_wizard.as_mut()
    }

    pub(crate) fn link_wizard_selected_action(&self) -> Option<LinkWizardAction> {
        self.link_wizard_state()
            .map(|state| LinkWizardAction::from_index(state.action_index))
    }

    pub(crate) fn link_wizard_anchor_item(&self) -> Option<&Item> {
        let anchor_id = self.link_wizard_state()?.anchor_item_id;
        self.all_items.iter().find(|item| item.id == anchor_id)
    }

    pub(crate) fn link_wizard_source_count(&self) -> usize {
        self.link_wizard_state()
            .map(|state| state.source_item_ids.len())
            .unwrap_or(0)
    }

    pub(crate) fn link_wizard_target_matches(&self) -> Vec<ItemId> {
        let Some(state) = self.link_wizard_state() else {
            return Vec::new();
        };
        let query = state.target_filter.trimmed().to_ascii_lowercase();
        let closed_category_ids: HashSet<CategoryId> = self
            .categories
            .iter()
            .filter(|category| {
                category.name.eq_ignore_ascii_case("Done")
                    || category.name.eq_ignore_ascii_case("Complete")
            })
            .map(|category| category.id)
            .collect();
        let mut rows: Vec<(String, ItemId)> = self
            .all_items
            .iter()
            .filter(|item| !state.source_item_ids.contains(&item.id))
            .filter(|item| !item.is_done)
            .filter(|item| {
                !item
                    .assignments
                    .keys()
                    .any(|category_id| closed_category_ids.contains(category_id))
            })
            .filter(|item| {
                if query.is_empty() {
                    return true;
                }
                item.text.to_ascii_lowercase().contains(&query)
                    || item.id.to_string().contains(&query)
            })
            .map(|item| (item.text.to_ascii_lowercase(), item.id))
            .collect();
        rows.sort_by(|a, b| a.0.cmp(&b.0));
        rows.into_iter().map(|(_, id)| id).collect()
    }

    fn clamp_link_wizard_target_index(&mut self) {
        let len = self.link_wizard_target_matches().len();
        if let Some(state) = self.link_wizard_state_mut() {
            state.target_index = if len == 0 {
                0
            } else {
                state.target_index.min(len - 1)
            };
            if !LinkWizardAction::from_index(state.action_index).requires_target()
                && state.focus == LinkWizardFocus::Target
            {
                state.focus = LinkWizardFocus::Confirm;
            }
        }
    }

    fn set_link_wizard_action(&mut self, action: LinkWizardAction) {
        if let Some(state) = self.link_wizard_state_mut() {
            state.action_index = action.index();
            if !action.requires_target() && state.focus == LinkWizardFocus::Target {
                state.focus = LinkWizardFocus::Confirm;
            }
        }
        self.clamp_link_wizard_target_index();
    }

    fn move_link_wizard_action_cursor(&mut self, delta: i32) {
        if let Some(state) = self.link_wizard_state_mut() {
            state.action_index = next_index(state.action_index, LinkWizardAction::ALL.len(), delta);
        }
        self.clamp_link_wizard_target_index();
    }

    fn move_link_wizard_target_cursor(&mut self, delta: i32) {
        let len = self.link_wizard_target_matches().len();
        if len == 0 {
            if let Some(state) = self.link_wizard_state_mut() {
                state.target_index = 0;
            }
            return;
        }
        if let Some(state) = self.link_wizard_state_mut() {
            state.target_index = next_index_clamped(state.target_index, len, delta);
        }
    }

    pub(crate) fn link_wizard_selected_target_id(&self) -> Option<ItemId> {
        let state = self.link_wizard_state()?;
        let matches = self.link_wizard_target_matches();
        matches
            .get(state.target_index.min(matches.len().saturating_sub(1)))
            .copied()
    }

    fn close_link_wizard(&mut self, status: &str) {
        self.mode = Mode::Normal;
        self.link_wizard = None;
        self.status = status.to_string();
    }

    fn apply_link_action_to_sources<F>(
        &self,
        source_item_ids: &[ItemId],
        mut apply: F,
    ) -> (usize, usize, usize, Option<String>)
    where
        F: FnMut(ItemId) -> Result<bool, AgendaError>,
    {
        let mut created = 0usize;
        let mut skipped = 0usize;
        let mut failed = 0usize;
        let mut first_error = None;

        for source_item_id in source_item_ids {
            match apply(*source_item_id) {
                Ok(true) => created += 1,
                Ok(false) => skipped += 1,
                Err(err) => {
                    failed += 1;
                    if first_error.is_none() {
                        first_error = Some(err.to_string());
                    }
                }
            }
        }

        (created, skipped, failed, first_error)
    }

    fn apply_link_wizard(&mut self, agenda: &Agenda<'_>) -> TuiResult<()> {
        let Some(state) = self.link_wizard_state().cloned() else {
            self.mode = Mode::Normal;
            return Ok(());
        };

        let action = LinkWizardAction::from_index(state.action_index);
        let anchor_id = state.anchor_item_id;
        let source_item_ids = state.source_item_ids.clone();
        let anchor_label = self
            .all_items
            .iter()
            .find(|item| item.id == anchor_id)
            .map(board_item_label)
            .unwrap_or_else(|| anchor_id.to_string());
        let source_label = if source_item_ids.len() > 1 {
            format!("{} selected items", source_item_ids.len())
        } else {
            anchor_label.clone()
        };
        let batch_mode = source_item_ids.len() > 1;

        let (status, clear_selection) = match action {
            LinkWizardAction::BlockedBy => {
                let target_id = self
                    .link_wizard_selected_target_id()
                    .ok_or("No target selected".to_string())?;
                let target_label = self
                    .all_items
                    .iter()
                    .find(|item| item.id == target_id)
                    .map(board_item_label)
                    .unwrap_or_else(|| target_id.to_string());
                let (created, skipped, failed, first_error) =
                    self.apply_link_action_to_sources(&source_item_ids, |source_item_id| {
                        agenda
                            .link_items_depends_on(source_item_id, target_id)
                            .map(|result| result.created)
                    });
                if batch_mode {
                    let mut summary = format!(
                        "Linked {} blocked by '{}' (created={created}, skipped={skipped}, failed={failed})",
                        source_label,
                        truncate_board_cell(&target_label, 30)
                    );
                    if let Some(err) = first_error {
                        summary.push_str(&format!(" first_error={err}"));
                    }
                    (summary, failed == 0)
                } else if failed > 0 {
                    (
                        format!(
                            "Link failed: {}",
                            first_error.unwrap_or_else(|| "unknown error".to_string())
                        ),
                        false,
                    )
                } else if created > 0 {
                    (
                        format!(
                            "Linked '{}' blocked by '{}'",
                            truncate_board_cell(&anchor_label, 30),
                            truncate_board_cell(&target_label, 30)
                        ),
                        true,
                    )
                } else {
                    ("Link already exists".to_string(), true)
                }
            }
            LinkWizardAction::DependsOn => {
                let target_id = self
                    .link_wizard_selected_target_id()
                    .ok_or("No target selected".to_string())?;
                let target_label = self
                    .all_items
                    .iter()
                    .find(|item| item.id == target_id)
                    .map(board_item_label)
                    .unwrap_or_else(|| target_id.to_string());
                let (created, skipped, failed, first_error) =
                    self.apply_link_action_to_sources(&source_item_ids, |source_item_id| {
                        agenda
                            .link_items_depends_on(source_item_id, target_id)
                            .map(|result| result.created)
                    });
                if batch_mode {
                    let mut summary = format!(
                        "Linked {} depends on '{}' (created={created}, skipped={skipped}, failed={failed})",
                        source_label,
                        truncate_board_cell(&target_label, 30)
                    );
                    if let Some(err) = first_error {
                        summary.push_str(&format!(" first_error={err}"));
                    }
                    (summary, failed == 0)
                } else if failed > 0 {
                    (
                        format!(
                            "Link failed: {}",
                            first_error.unwrap_or_else(|| "unknown error".to_string())
                        ),
                        false,
                    )
                } else if created > 0 {
                    ("Linked depends-on".to_string(), true)
                } else {
                    ("Link already exists".to_string(), true)
                }
            }
            LinkWizardAction::Blocks => {
                let target_id = self
                    .link_wizard_selected_target_id()
                    .ok_or("No target selected".to_string())?;
                let target_label = self
                    .all_items
                    .iter()
                    .find(|item| item.id == target_id)
                    .map(board_item_label)
                    .unwrap_or_else(|| target_id.to_string());
                let (created, skipped, failed, first_error) =
                    self.apply_link_action_to_sources(&source_item_ids, |source_item_id| {
                        agenda
                            .link_items_blocks(source_item_id, target_id)
                            .map(|result| result.created)
                    });
                if batch_mode {
                    let mut summary = format!(
                        "Linked {} blocks '{}' (created={created}, skipped={skipped}, failed={failed})",
                        source_label,
                        truncate_board_cell(&target_label, 30)
                    );
                    if let Some(err) = first_error {
                        summary.push_str(&format!(" first_error={err}"));
                    }
                    (summary, failed == 0)
                } else if failed > 0 {
                    (
                        format!(
                            "Link failed: {}",
                            first_error.unwrap_or_else(|| "unknown error".to_string())
                        ),
                        false,
                    )
                } else if created > 0 {
                    (
                        format!(
                            "Linked '{}' blocks '{}'",
                            truncate_board_cell(&anchor_label, 30),
                            truncate_board_cell(&target_label, 30)
                        ),
                        true,
                    )
                } else {
                    ("Link already exists".to_string(), true)
                }
            }
            LinkWizardAction::RelatedTo => {
                let target_id = self
                    .link_wizard_selected_target_id()
                    .ok_or("No target selected".to_string())?;
                let target_label = self
                    .all_items
                    .iter()
                    .find(|item| item.id == target_id)
                    .map(board_item_label)
                    .unwrap_or_else(|| target_id.to_string());
                let (created, skipped, failed, first_error) =
                    self.apply_link_action_to_sources(&source_item_ids, |source_item_id| {
                        agenda
                            .link_items_related(source_item_id, target_id)
                            .map(|result| result.created)
                    });
                if batch_mode {
                    let mut summary = format!(
                        "Linked {} related to '{}' (created={created}, skipped={skipped}, failed={failed})",
                        source_label,
                        truncate_board_cell(&target_label, 30)
                    );
                    if let Some(err) = first_error {
                        summary.push_str(&format!(" first_error={err}"));
                    }
                    (summary, failed == 0)
                } else if failed > 0 {
                    (
                        format!(
                            "Link failed: {}",
                            first_error.unwrap_or_else(|| "unknown error".to_string())
                        ),
                        false,
                    )
                } else if created > 0 {
                    ("Linked related items".to_string(), true)
                } else {
                    ("Link already exists".to_string(), true)
                }
            }
            LinkWizardAction::ClearDependencies => {
                let mut total_prereqs = 0usize;
                let mut total_dependents = 0usize;
                for source_item_id in &source_item_ids {
                    let prereqs = agenda.immediate_prereq_ids(*source_item_id)?;
                    let dependents = agenda.immediate_dependent_ids(*source_item_id)?;
                    total_prereqs += prereqs.len();
                    total_dependents += dependents.len();
                    for dependency_id in &prereqs {
                        agenda.unlink_items_depends_on(*source_item_id, *dependency_id)?;
                    }
                    for blocked_id in &dependents {
                        agenda.unlink_items_blocks(*source_item_id, *blocked_id)?;
                    }
                }
                if source_item_ids.len() > 1 {
                    (
                        format!(
                            "Cleared dependencies for {} (prereqs={}, blocks={})",
                            source_label, total_prereqs, total_dependents
                        ),
                        true,
                    )
                } else {
                    (
                        format!(
                            "Cleared dependencies for '{}' (prereqs={}, blocks={})",
                            truncate_board_cell(&anchor_label, 30),
                            total_prereqs,
                            total_dependents
                        ),
                        true,
                    )
                }
            }
        };

        self.refresh(agenda.store())?;
        self.set_item_selection_by_id(anchor_id);
        if batch_mode && clear_selection {
            self.clear_selected_items();
        }
        self.close_link_wizard(&status);
        Ok(())
    }

    pub(crate) fn handle_link_wizard_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> TuiResult<bool> {
        let Some(state) = self.link_wizard_state().cloned() else {
            self.mode = Mode::Normal;
            return Ok(false);
        };
        let in_target_focus = state.focus == LinkWizardFocus::Target;

        match code {
            KeyCode::Esc => {
                self.close_link_wizard("Link wizard canceled");
            }
            KeyCode::Tab => {
                if let Some(wizard) = self.link_wizard_state_mut() {
                    wizard.focus = wizard.focus.next();
                }
                if self
                    .link_wizard_selected_action()
                    .is_some_and(|action| !action.requires_target())
                    && self
                        .link_wizard_state()
                        .is_some_and(|wizard| wizard.focus == LinkWizardFocus::Target)
                {
                    if let Some(wizard) = self.link_wizard_state_mut() {
                        wizard.focus = LinkWizardFocus::Confirm;
                    }
                }
            }
            KeyCode::BackTab => {
                if let Some(wizard) = self.link_wizard_state_mut() {
                    wizard.focus = wizard.focus.prev();
                }
                if self
                    .link_wizard_selected_action()
                    .is_some_and(|action| !action.requires_target())
                    && self
                        .link_wizard_state()
                        .is_some_and(|wizard| wizard.focus == LinkWizardFocus::Target)
                {
                    if let Some(wizard) = self.link_wizard_state_mut() {
                        wizard.focus = LinkWizardFocus::ScopeAction;
                    }
                }
            }
            KeyCode::Char('b') if !in_target_focus => {
                self.set_link_wizard_action(LinkWizardAction::BlockedBy)
            }
            KeyCode::Char('B') if !in_target_focus => {
                self.set_link_wizard_action(LinkWizardAction::Blocks)
            }
            KeyCode::Char('d') | KeyCode::Char('D') if !in_target_focus => {
                self.set_link_wizard_action(LinkWizardAction::DependsOn)
            }
            KeyCode::Char('r') | KeyCode::Char('R') if !in_target_focus => {
                self.set_link_wizard_action(LinkWizardAction::RelatedTo)
            }
            KeyCode::Char('c') | KeyCode::Char('C') if !in_target_focus => {
                self.set_link_wizard_action(LinkWizardAction::ClearDependencies)
            }
            KeyCode::Down | KeyCode::Char('j') => match state.focus {
                LinkWizardFocus::ScopeAction => self.move_link_wizard_action_cursor(1),
                LinkWizardFocus::Target => self.move_link_wizard_target_cursor(1),
                LinkWizardFocus::Confirm => {}
            },
            KeyCode::Up | KeyCode::Char('k') => match state.focus {
                LinkWizardFocus::ScopeAction => self.move_link_wizard_action_cursor(-1),
                LinkWizardFocus::Target => self.move_link_wizard_target_cursor(-1),
                LinkWizardFocus::Confirm => {}
            },
            KeyCode::Char('/') => {
                if self
                    .link_wizard_selected_action()
                    .is_some_and(|action| action.requires_target())
                {
                    if let Some(wizard) = self.link_wizard_state_mut() {
                        wizard.focus = LinkWizardFocus::Target;
                    }
                }
            }
            KeyCode::Enter => match state.focus {
                LinkWizardFocus::ScopeAction => {
                    if self
                        .link_wizard_selected_action()
                        .is_some_and(|action| action.requires_target())
                    {
                        if let Some(wizard) = self.link_wizard_state_mut() {
                            wizard.focus = LinkWizardFocus::Target;
                        }
                    } else if let Some(wizard) = self.link_wizard_state_mut() {
                        wizard.focus = LinkWizardFocus::Confirm;
                    }
                }
                LinkWizardFocus::Target => {
                    if self
                        .link_wizard_selected_action()
                        .is_some_and(|action| action.requires_target())
                    {
                        if self.link_wizard_selected_target_id().is_some() {
                            if let Some(wizard) = self.link_wizard_state_mut() {
                                wizard.focus = LinkWizardFocus::Confirm;
                            }
                        } else {
                            self.status = "No target selected".to_string();
                        }
                    } else if let Some(wizard) = self.link_wizard_state_mut() {
                        wizard.focus = LinkWizardFocus::Confirm;
                    }
                }
                LinkWizardFocus::Confirm => {
                    self.apply_link_wizard(agenda)?;
                }
            },
            _ => {
                let requires_target = self
                    .link_wizard_selected_action()
                    .is_some_and(|action| action.requires_target());
                if requires_target
                    && self
                        .link_wizard_state()
                        .is_some_and(|wizard| wizard.focus == LinkWizardFocus::Target)
                {
                    let text_key = self.text_key_event(code);
                    let consumed = if let Some(wizard) = self.link_wizard_state_mut() {
                        wizard.target_filter.handle_key_event(text_key, false)
                    } else {
                        false
                    };
                    if consumed {
                        self.clamp_link_wizard_target_index();
                    }
                }
            }
        }
        Ok(false)
    }

    pub(crate) fn handle_help_panel_key(&mut self, code: KeyCode) -> TuiResult<bool> {
        match code {
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('?') | KeyCode::Char('q') => {
                self.mode = Mode::Normal;
            }
            _ => {}
        }
        Ok(false)
    }

    pub(crate) fn handle_normal_key_event(
        &mut self,
        key: KeyEvent,
        agenda: &Agenda<'_>,
    ) -> TuiResult<bool> {
        let ctrl_only = key.modifiers.contains(KeyModifiers::CONTROL)
            && !key
                .modifiers
                .intersects(KeyModifiers::ALT | KeyModifiers::SUPER);
        if ctrl_only {
            match key.code {
                KeyCode::Char('l') | KeyCode::Char('L') => {
                    self.refresh(agenda.store())?;
                    self.status = "Reloaded view from store".to_string();
                    return Ok(false);
                }
                KeyCode::Char('r') | KeyCode::Char('R') => {
                    self.cycle_auto_refresh_interval();
                    self.persist_auto_refresh_interval(agenda.store())?;
                    return Ok(false);
                }
                KeyCode::Char('z') => {
                    self.apply_undo(agenda)?;
                    return Ok(false);
                }
                KeyCode::Char('Z') => {
                    self.apply_redo(agenda)?;
                    return Ok(false);
                }
                _ => {}
            }
        }
        self.handle_normal_key(key.code, agenda)
    }

    /// Open an InputPanel for adding a new item in the current section.
    pub(crate) fn open_input_panel_add_item(&mut self) {
        let (section_title, on_insert_assign) = self
            .current_slot()
            .map(|slot| {
                let title = slot.title.clone();
                let on_insert = match &slot.context {
                    SlotContext::GeneratedSection {
                        on_insert_assign, ..
                    } => on_insert_assign.clone(),
                    SlotContext::Section { section_index } => {
                        let idx = *section_index;
                        self.current_view()
                            .and_then(|v| v.sections.get(idx))
                            .map(|s| s.on_insert_assign.clone())
                            .unwrap_or_default()
                    }
                    SlotContext::Unmatched => HashSet::new(),
                };
                (title, on_insert)
            })
            .unwrap_or_else(|| ("Items".to_string(), HashSet::new()));

        self.input_panel = Some(input_panel::InputPanel::new_add_item(
            &section_title,
            &on_insert_assign,
        ));
        self.input_panel_discard_confirm = false;
        self.mode = Mode::InputPanel;
        self.status =
            "Add item: type text, S to save, Tab for note/categories, Esc to cancel".to_string();
    }

    pub(crate) fn handle_board_add_column_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> TuiResult<bool> {
        if self.board_add_column_create_confirm_open() {
            match inline_create_confirm_key_action(code) {
                InlineCreateConfirmKeyAction::Confirm => {
                    self.confirm_inline_create_board_add_column(agenda)?;
                    return Ok(false);
                }
                InlineCreateConfirmKeyAction::Cancel => {
                    self.set_board_add_column_create_confirm_name(None);
                    self.update_board_add_column_suggestions();
                    self.status = "Create canceled. Continue picking column.".to_string();
                    return Ok(false);
                }
                InlineCreateConfirmKeyAction::DismissAndContinue => {
                    self.set_board_add_column_create_confirm_name(None);
                    self.update_board_add_column_suggestions();
                    self.status = "Create canceled. Continue picking column.".to_string();
                }
                InlineCreateConfirmKeyAction::None => {}
            }
        }

        match code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.clear_board_add_column_session();
                self.status = "Add column canceled".to_string();
                return Ok(false);
            }
            KeyCode::Down => {
                self.move_board_add_column_suggest_cursor(1);
                return Ok(false);
            }
            KeyCode::Up => {
                self.move_board_add_column_suggest_cursor(-1);
                return Ok(false);
            }
            KeyCode::Tab => {
                self.autocomplete_board_add_column_from_suggestion();
                return Ok(false);
            }
            KeyCode::Enter => {
                if let Some(category_id) = self.exact_board_add_column_match_id() {
                    self.insert_board_column_for_category(agenda, category_id)?;
                } else {
                    let matches = self.get_board_add_column_suggest_matches();
                    if let Some(state) = self.board_add_column_state() {
                        if let Some(&category_id) =
                            matches.get(state.suggest_index.min(matches.len().saturating_sub(1)))
                        {
                            self.insert_board_column_for_category(agenda, category_id)?;
                        } else {
                            self.open_board_add_column_create_confirm();
                        }
                    }
                }
                return Ok(false);
            }
            _ => {}
        }

        let text_key = self.text_key_event(code);
        if let Some(state) = self.board_add_column_state_mut() {
            if state.input.handle_key_event(text_key, false) {
                state.create_confirm_name = None;
                self.update_board_add_column_suggestions();
            }
        }
        Ok(false)
    }

    pub(crate) fn handle_category_direct_edit_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> TuiResult<bool> {
        if self.direct_edit_create_confirm_open() {
            match inline_create_confirm_key_action(code) {
                InlineCreateConfirmKeyAction::Confirm => {
                    self.confirm_inline_create_category_direct_edit(agenda)?;
                    return Ok(false);
                }
                InlineCreateConfirmKeyAction::Cancel => {
                    self.set_direct_edit_create_confirm_name(None);
                    self.update_suggestions();
                    self.status = "Create canceled. Continue editing category.".to_string();
                    return Ok(false);
                }
                InlineCreateConfirmKeyAction::DismissAndContinue => {
                    self.set_direct_edit_create_confirm_name(None);
                    self.status = "Create canceled. Continue editing category.".to_string();
                    self.update_suggestions();
                }
                InlineCreateConfirmKeyAction::None => {}
            }
        }

        match code {
            KeyCode::Char('S')
                if !matches!(
                    self.active_category_direct_edit_focus(),
                    Some(CategoryDirectEditFocus::Input)
                ) =>
            {
                self.apply_category_direct_edit_draft(agenda)?;
                return Ok(false);
            }
            KeyCode::BackTab => {
                self.cycle_category_direct_edit_focus(false);
                return Ok(false);
            }
            KeyCode::Tab => {
                self.cycle_category_direct_edit_focus(true);
                return Ok(false);
            }
            KeyCode::Right
                if matches!(
                    self.active_category_direct_edit_focus(),
                    Some(CategoryDirectEditFocus::Suggestions)
                ) =>
            {
                self.autocomplete_from_suggestion();
                return Ok(false);
            }
            KeyCode::Char('+')
                if !matches!(
                    self.active_category_direct_edit_focus(),
                    Some(CategoryDirectEditFocus::Input)
                ) =>
            {
                self.category_direct_edit_add_blank_row_guarded();
                return Ok(false);
            }
            KeyCode::Char('n') | KeyCode::Char('a')
                if matches!(
                    self.active_category_direct_edit_focus(),
                    Some(CategoryDirectEditFocus::Entries)
                ) =>
            {
                self.category_direct_edit_add_blank_row_guarded();
                return Ok(false);
            }
            KeyCode::Char('x')
                if matches!(
                    self.active_category_direct_edit_focus(),
                    Some(CategoryDirectEditFocus::Entries)
                ) =>
            {
                self.remove_active_category_direct_edit_row();
                return Ok(false);
            }
            KeyCode::Up | KeyCode::Down => {
                let delta = if matches!(code, KeyCode::Up) { -1 } else { 1 };
                match self
                    .active_category_direct_edit_focus()
                    .unwrap_or(CategoryDirectEditFocus::Input)
                {
                    CategoryDirectEditFocus::Entries => {
                        self.move_category_direct_edit_active_row(delta)
                    }
                    CategoryDirectEditFocus::Suggestions => self.move_suggest_cursor(delta),
                    CategoryDirectEditFocus::Input => {}
                }
                return Ok(false);
            }
            KeyCode::Char('j') | KeyCode::Char('k')
                if !matches!(
                    self.active_category_direct_edit_focus(),
                    Some(CategoryDirectEditFocus::Input)
                ) =>
            {
                let delta = if matches!(code, KeyCode::Char('k')) {
                    -1
                } else {
                    1
                };
                match self
                    .active_category_direct_edit_focus()
                    .unwrap_or(CategoryDirectEditFocus::Input)
                {
                    CategoryDirectEditFocus::Entries => {
                        self.move_category_direct_edit_active_row(delta)
                    }
                    CategoryDirectEditFocus::Suggestions => self.move_suggest_cursor(delta),
                    CategoryDirectEditFocus::Input => {}
                }
                return Ok(false);
            }
            KeyCode::Enter => {
                let active_text = self
                    .active_category_direct_edit_input_text()
                    .unwrap_or("")
                    .to_string();
                if active_text.trim().is_empty() {
                    let row_count = self
                        .category_direct_edit_state()
                        .map(|s| s.rows.len())
                        .unwrap_or(0);
                    self.remove_active_category_direct_edit_row();
                    self.status = if row_count <= 1 {
                        "Empty row kept (must keep one row). Press s/S to save cleared column"
                            .to_string()
                    } else {
                        "Removed empty row".to_string()
                    };
                } else if let Some(category_id) = self.exact_current_column_child_match_id() {
                    let _ = self.resolve_active_category_direct_edit_row(category_id)?;
                } else if !self.get_current_suggest_matches().is_empty() {
                    self.assign_selected_suggestion(agenda)?;
                } else {
                    self.open_direct_edit_create_confirm_for_active_row();
                }
                return Ok(false);
            }
            _ => {}
        }

        match code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.status = "Cancelled column edits".to_string();
                self.clear_input();
                self.clear_category_direct_edit_session();
            }
            _ => {
                if matches!(
                    self.active_category_direct_edit_focus(),
                    Some(CategoryDirectEditFocus::Input)
                ) {
                    let text_key = self.text_key_event(code);
                    if let Some(row) = self.active_category_direct_edit_row_mut() {
                        row.input.handle_key_event(text_key, false);
                        row.category_id = None;
                    }
                    self.sync_category_direct_edit_input_mirror();
                    self.update_suggestions();
                }
            }
        }
        Ok(false)
    }

    /// Open an InputPanel for editing the currently selected item.
    pub(crate) fn open_input_panel_edit_item(&mut self) {
        if let Some(item) = self.selected_item() {
            let text = item.text.clone();
            let note = item.note.clone().unwrap_or_default();
            // Collect all assigned category IDs for the draft so Edit view
            // mirrors Assign/Column picker check states (manual + derived).
            let categories: HashSet<agenda_core::model::CategoryId> =
                item.assignments.keys().copied().collect();
            // Collect numeric buffers and originals for assigned numeric categories.
            let mut numeric_buffers = std::collections::HashMap::new();
            let mut numeric_originals = std::collections::HashMap::new();
            for (cat_id, assignment) in &item.assignments {
                let cat = self.categories.iter().find(|c| c.id == *cat_id);
                if let Some(cat) = cat {
                    if cat.value_kind == agenda_core::model::CategoryValueKind::Numeric {
                        numeric_buffers.insert(
                            *cat_id,
                            crate::text_buffer::TextBuffer::new(
                                assignment
                                    .numeric_value
                                    .map(|v| v.normalize().to_string())
                                    .unwrap_or_default(),
                            ),
                        );
                        numeric_originals.insert(*cat_id, assignment.numeric_value);
                    }
                }
            }
            let item_id = item.id;
            let when_value = item
                .when_date
                .map(|value| value.strftime("%Y-%m-%d %H:%M").to_string())
                .unwrap_or_default();
            let mut panel = input_panel::InputPanel::new_edit_item(
                item_id,
                text,
                note,
                when_value,
                categories,
                numeric_buffers,
                numeric_originals,
            );
            // Populate pending classification suggestions for inline review.
            if let Some(review_item) = self
                .classification_ui
                .review_items
                .iter()
                .find(|ri| ri.item_id == item_id)
            {
                panel.pending_suggestions = review_item
                    .suggestions
                    .iter()
                    .map(|s| (s.clone(), SuggestionDecision::Pending))
                    .collect();
            }
            self.input_panel = Some(panel);
            self.input_panel_discard_confirm = false;
            self.mode = Mode::InputPanel;
            self.status =
                "Edit item: S to save, Tab cycles fields, Esc asks before discard".to_string();
        } else {
            self.status = "No selected item to edit".to_string();
        }
    }

    /// Handle a key event while in Mode::InputPanel.
    pub(crate) fn handle_input_panel_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> TuiResult<bool> {
        let Some(_) = &self.input_panel else {
            self.mode = Mode::Normal;
            self.status = "InputPanel error: no panel state".to_string();
            return Ok(false);
        };

        if self.input_panel_discard_confirm_active() {
            match code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    self.cancel_input_panel_with_status(
                        input_panel::InputPanelKind::EditItem,
                        true,
                    );
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    self.input_panel_discard_confirm = false;
                    self.status = "Continue editing item".to_string();
                }
                _ => {}
            }
            return Ok(false);
        }

        // Ctrl+G: open the focused text buffer in $EDITOR.
        if self.current_key_modifiers.contains(KeyModifiers::CONTROL)
            && matches!(code, KeyCode::Char('g') | KeyCode::Char('G'))
        {
            if let Some(panel) = &self.input_panel {
                let target = match panel.focus {
                    input_panel::InputPanelFocus::Note => ExternalEditorTarget::Note,
                    _ => ExternalEditorTarget::Text,
                };
                let allowed = matches!(
                    panel.kind,
                    input_panel::InputPanelKind::AddItem | input_panel::InputPanelKind::EditItem
                ) || matches!(panel.focus, input_panel::InputPanelFocus::Text);
                if allowed {
                    self.pending_external_edit = Some(target);
                    return Ok(false);
                }
            }
        }

        if self.handle_input_panel_category_filter_key(code) {
            return Ok(false);
        }

        // Determine if the current category row is an assigned numeric category
        // (needed for key routing decisions in handle_key).
        let current_row_is_assigned_numeric = self
            .input_panel
            .as_ref()
            .and_then(|panel| {
                if panel.focus != input_panel::InputPanelFocus::Categories {
                    return Some(false);
                }
                self.input_panel_selected_category_row().map(|row| {
                    panel.categories.contains(&row.id)
                        && row.value_kind == agenda_core::model::CategoryValueKind::Numeric
                })
            })
            .unwrap_or(false);

        let input_key = self.text_key_event(code);
        let action = {
            let panel = self
                .input_panel
                .as_mut()
                .expect("input panel checked above");
            panel.handle_key_event(input_key, current_row_is_assigned_numeric)
        };

        use input_panel::InputPanelAction;
        match action {
            InputPanelAction::Cancel => {
                let was_dirty = self
                    .input_panel
                    .as_ref()
                    .map(|p| p.is_dirty())
                    .unwrap_or(false);
                let kind = self
                    .input_panel
                    .as_ref()
                    .map(|p| p.kind)
                    .unwrap_or(input_panel::InputPanelKind::AddItem);
                if kind == input_panel::InputPanelKind::EditItem && was_dirty {
                    self.begin_input_panel_discard_confirm();
                    return Ok(false);
                }
                self.cancel_input_panel_with_status(kind, was_dirty);
            }
            InputPanelAction::Save => {
                self.input_panel_discard_confirm = false;
                let kind = self
                    .input_panel
                    .as_ref()
                    .map(|p| p.kind)
                    .unwrap_or(input_panel::InputPanelKind::AddItem);
                match kind {
                    input_panel::InputPanelKind::AddItem => {
                        self.save_input_panel_add(agenda)?;
                    }
                    input_panel::InputPanelKind::EditItem => {
                        self.save_input_panel_edit(agenda)?;
                    }
                    input_panel::InputPanelKind::NameInput => {
                        self.save_input_panel_name(agenda)?;
                    }
                    input_panel::InputPanelKind::WhenDate => {
                        self.save_input_panel_name(agenda)?;
                    }
                    input_panel::InputPanelKind::NumericValue => {
                        self.save_input_panel_name(agenda)?;
                    }
                    input_panel::InputPanelKind::CategoryCreate => {
                        self.save_input_panel_category_create(agenda)?;
                    }
                }
            }
            InputPanelAction::ToggleCategory => {
                // Suggestions occupy indices 0..suggestion_len; categories start after
                let suggestion_len = self
                    .input_panel
                    .as_ref()
                    .map(|p| p.pending_suggestions.len())
                    .unwrap_or(0);
                let cursor = self
                    .input_panel
                    .as_ref()
                    .map(|p| p.category_cursor)
                    .unwrap_or(0);
                let suggestion_index = if cursor < suggestion_len {
                    Some(cursor)
                } else {
                    None
                };

                if let Some(si) = suggestion_index {
                    // Toggle suggestion decision: Pending → Accept → Reject → Pending
                    let new_status = if let Some(panel) = &mut self.input_panel {
                        if let Some(entry) = panel.pending_suggestions.get_mut(si) {
                            entry.1 = entry.1.next();
                            let cat_names = category_name_map(&self.categories);
                            let cat_name =
                                candidate_assignment_label(&entry.0.assignment, &cat_names);
                            Some(format!(
                                "Suggestion '{}': {}",
                                cat_name,
                                match entry.1 {
                                    SuggestionDecision::Pending => "pending",
                                    SuggestionDecision::Accept => "accept",
                                    SuggestionDecision::Reject => "reject",
                                }
                            ))
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    if let Some(status) = new_status {
                        self.status = status;
                    }
                } else {
                    let idx = self.input_panel_selected_category_row_index();
                    let row = self.input_panel_selected_category_row().cloned();
                    if let Some(row) = row {
                        if !row.is_reserved {
                            let is_adding = self
                                .input_panel
                                .as_ref()
                                .map(|p| !p.categories.contains(&row.id))
                                .unwrap_or(false);
                            let is_numeric =
                                row.value_kind == agenda_core::model::CategoryValueKind::Numeric;
                            // If adding into an exclusive parent group, clear siblings first.
                            if is_adding {
                                if let Some(idx) = idx {
                                    let to_clear =
                                        exclusive_siblings_to_clear(&self.category_rows, idx);
                                    if let Some(panel) = &mut self.input_panel {
                                        for sibling_id in &to_clear {
                                            panel.categories.remove(sibling_id);
                                            panel.numeric_buffers.remove(sibling_id);
                                        }
                                    }
                                }
                            }
                            if let Some(panel) = &mut self.input_panel {
                                panel.toggle_category(row.id);
                                // Manage numeric buffer — keep buffer on toggle-off
                                // so the value is preserved if user toggles back on.
                                if is_numeric && is_adding {
                                    panel
                                        .numeric_buffers
                                        .entry(row.id)
                                        .or_insert_with(crate::text_buffer::TextBuffer::empty);
                                }
                            }
                            let selected = self
                                .input_panel
                                .as_ref()
                                .map(|p| p.categories.len())
                                .unwrap_or(0);
                            self.status =
                                format!("Category '{}' toggled — {} selected", row.name, selected);
                        } else {
                            self.status = format!("'{}' cannot be assigned here", row.name);
                        }
                    }
                }
            }
            InputPanelAction::MoveCategoryCursor(delta) => {
                let cat_len = self.input_panel_visible_category_row_indices().len();
                let suggestion_len = self
                    .input_panel
                    .as_ref()
                    .map(|p| p.pending_suggestions.len())
                    .unwrap_or(0);
                let total_len = cat_len + suggestion_len;
                if let Some(panel) = &mut self.input_panel {
                    if total_len > 0 {
                        let current = panel.category_cursor as i64;
                        let len = total_len as i64;
                        let new = ((current + delta as i64).rem_euclid(len)) as usize;
                        panel.category_cursor = new;
                    }
                }
            }
            InputPanelAction::Handled => {
                // If we're on Categories focus and the row is an assigned numeric,
                // route the key to the numeric buffer.
                if current_row_is_assigned_numeric {
                    let cat_id = self.input_panel_selected_category_row().map(|r| r.id);
                    if let Some(cat_id) = cat_id {
                        let input_key = self.text_key_event(code);
                        if let Some(panel) = &mut self.input_panel {
                            if let Some(buf) = panel.numeric_buffers.get_mut(&cat_id) {
                                buf.handle_key_event(input_key, false);
                            }
                        }
                    }
                } else if matches!(
                    self.name_input_context,
                    Some(NameInputContext::WhenDateEdit)
                ) && self.status.starts_with("Could not parse")
                {
                    self.status.clear();
                }
            }
            InputPanelAction::ToggleType => {
                if let Some(panel) = &mut self.input_panel {
                    panel.value_kind = match panel.value_kind {
                        CategoryValueKind::Tag => CategoryValueKind::Numeric,
                        CategoryValueKind::Numeric => CategoryValueKind::Tag,
                    };
                    let label = match panel.value_kind {
                        CategoryValueKind::Tag => "Tag",
                        CategoryValueKind::Numeric => "Numeric",
                    };
                    self.status = format!("Type set to {label}");
                }
            }
            InputPanelAction::FocusNext
            | InputPanelAction::FocusPrev
            | InputPanelAction::Unhandled => {}
        }
        if let Some(panel) = &mut self.input_panel {
            if panel.focus != input_panel::InputPanelFocus::Categories {
                panel.category_filter_editing = false;
            }
        }
        Ok(false)
    }

    /// Save an InputPanel(AddItem) to the store.
    fn save_input_panel_add(&mut self, agenda: &Agenda<'_>) -> TuiResult<()> {
        let Some(panel) = &self.input_panel else {
            self.mode = Mode::Normal;
            return Ok(());
        };
        let text = panel.text.trimmed().to_string();
        if text.is_empty() {
            self.status = "Cannot save: text cannot be empty".to_string();
            return Ok(());
        }
        let note_raw = panel.note.trimmed().to_string();
        let note = if note_raw.is_empty() {
            None
        } else {
            Some(note_raw)
        };
        let categories_to_assign: Vec<_> = panel.categories.iter().copied().collect();

        // Create item (parses When, applies on_insert_assign via insert_into_context).
        let item = Item::new(text.clone());
        let reference_date = jiff::Zoned::now().date();
        agenda.create_item_with_reference_date(&item, reference_date)?;

        // Set note if provided.
        if note.is_some() {
            let mut loaded = agenda.store().get_item(item.id)?;
            loaded.note = note;
            loaded.modified_at = Timestamp::now();
            agenda.update_item_with_reference_date(&loaded, reference_date)?;
        }

        // Assign explicitly selected categories.
        for cat_id in &categories_to_assign {
            let _ = agenda.assign_item_manual(
                item.id,
                *cat_id,
                Some("manual:input_panel.add".to_string()),
            );
        }

        // Apply numeric values for assigned numeric categories.
        for (cat_id, buf) in &panel.numeric_buffers {
            let trimmed = buf.trimmed();
            if trimmed.is_empty() {
                continue;
            }
            if let Ok(val) = trimmed.replace(',', "").parse::<rust_decimal::Decimal>() {
                let _ = agenda.assign_item_numeric_manual(
                    item.id,
                    *cat_id,
                    val,
                    Some("manual:input_panel.add".to_string()),
                );
            }
        }

        // Insert into section context (applies on_insert_assign rules).
        if let Some(view) = self.current_view().cloned() {
            if let Some(context) = self.current_slot().map(|slot| slot.context.clone()) {
                self.insert_into_context(agenda, item.id, &view, &context)?;
            }
        }

        let category_names: Vec<String> = agenda
            .store()
            .get_hierarchy()?
            .into_iter()
            .map(|c| c.name)
            .collect();
        let unknown_hashtags = unknown_hashtag_tokens(&text, &category_names);
        let created = agenda.store().get_item(item.id)?;

        self.push_undo(UndoEntry::ItemCreated { item_id: item.id });
        self.refresh(agenda.store())?;
        self.set_item_selection_by_id(item.id);
        self.input_panel = None;
        self.mode = Mode::Normal;
        self.status = add_capture_status_message(created.when_date, &unknown_hashtags);
        if let Some(suffix) = self.classification_pending_suffix() {
            self.status = format!("{} | {suffix}. Press C to review.", self.status);
        }
        Ok(())
    }

    /// Save an InputPanel(EditItem) to the store (text, note, and category diff).
    fn save_input_panel_edit(&mut self, agenda: &Agenda<'_>) -> TuiResult<()> {
        let Some(panel) = &self.input_panel else {
            self.mode = Mode::Normal;
            return Ok(());
        };
        let Some(item_id) = panel.item_id else {
            self.input_panel = None;
            self.mode = Mode::Normal;
            self.status = "Edit failed: no item ID".to_string();
            return Ok(());
        };
        let updated_text = panel.text.trimmed().to_string();
        if updated_text.is_empty() {
            self.status = "Cannot save: text cannot be empty".to_string();
            return Ok(());
        }
        let updated_note = if panel.note.trimmed().is_empty() {
            None
        } else {
            Some(panel.note.text().to_string())
        };
        let new_categories: HashSet<agenda_core::model::CategoryId> = panel.categories.clone();
        let numeric_buffers = panel.numeric_buffers.clone();
        let numeric_originals = panel.numeric_originals.clone();

        let mut item = agenda.store().get_item(item_id)?;
        let undo_old_text = item.text.clone();
        let undo_old_note = item.note.clone();

        // Compute category diff: which to add, which to remove.
        let existing_categories: HashSet<_> = item.assignments.keys().copied().collect();

        // Check for numeric value changes.
        let has_numeric_changes = numeric_buffers.iter().any(|(cat_id, buf)| {
            let trimmed = buf.trimmed();
            if trimmed.is_empty() {
                return false; // empty = keep existing, no change
            }
            let original = numeric_originals.get(cat_id).copied().flatten();
            match trimmed.replace(',', "").parse::<rust_decimal::Decimal>() {
                Ok(new_val) => original != Some(new_val),
                Err(_) => true, // invalid input counts as a "change" (will error on save)
            }
        });

        let no_text_change = item.text == updated_text;
        let no_note_change = item.note == updated_note;
        let no_cat_change = existing_categories == new_categories;

        let when_text = panel.when_buffer.trimmed().to_string();
        let no_when_change = when_text == panel.original_when;

        let has_suggestion_decisions = panel
            .pending_suggestions
            .iter()
            .any(|(_, d)| *d != SuggestionDecision::Pending);
        let pending_suggestions = panel.pending_suggestions.clone();

        if no_text_change
            && no_note_change
            && no_cat_change
            && no_when_change
            && !has_numeric_changes
            && !has_suggestion_decisions
        {
            self.input_panel = None;
            self.mode = Mode::Normal;
            self.status = "Edit canceled: no changes".to_string();
            return Ok(());
        }

        // Validate when-date input before making any changes.
        let parsed_when = if !no_when_change {
            if when_text.is_empty() {
                Some(None) // clear the when date
            } else {
                match Self::parse_when_datetime_input(&when_text) {
                    Ok(dt) => Some(dt),
                    Err(e) => {
                        self.status = format!("Could not parse when date: {e}");
                        return Ok(());
                    }
                }
            }
        } else {
            None // no change
        };

        // Validate numeric values before making any changes.
        for (cat_id, buf) in &numeric_buffers {
            let trimmed = buf.trimmed();
            if trimmed.is_empty() {
                continue;
            }
            if trimmed
                .replace(',', "")
                .parse::<rust_decimal::Decimal>()
                .is_err()
            {
                let cat_name = self
                    .categories
                    .iter()
                    .find(|c| c.id == *cat_id)
                    .map(|c| c.name.as_str())
                    .unwrap_or("?");
                self.status = format!("Invalid numeric value for '{}': '{}'", cat_name, trimmed);
                return Ok(());
            }
        }

        // Update text and note.
        item.text = updated_text;
        item.note = updated_note;
        item.modified_at = Timestamp::now();
        let reference_date = jiff::Zoned::now().date();
        agenda.update_item_with_reference_date(&item, reference_date)?;

        // Apply when-date change.
        if let Some(new_when) = parsed_when {
            agenda.set_item_when_date(
                item_id,
                new_when,
                Some("manual:input_panel.edit".to_string()),
            )?;
        }

        // Apply category changes.
        for cat_id in new_categories.difference(&existing_categories) {
            let _ = agenda.assign_item_manual(
                item_id,
                *cat_id,
                Some("manual:input_panel.edit".to_string()),
            );
        }
        for cat_id in existing_categories.difference(&new_categories) {
            let _ = agenda.unassign_item_manual(item_id, *cat_id);
        }

        // Apply numeric value changes.
        for (cat_id, buf) in &numeric_buffers {
            let trimmed = buf.trimmed();
            if trimmed.is_empty() {
                continue; // keep existing value
            }
            let new_val: rust_decimal::Decimal = trimmed.replace(',', "").parse().unwrap();
            let original = numeric_originals.get(cat_id).copied().flatten();
            if original == Some(new_val) {
                continue; // no change
            }
            let _ = agenda.assign_item_numeric_manual(
                item_id,
                *cat_id,
                new_val,
                Some("manual:input_panel.edit".to_string()),
            );
        }

        // Apply suggestion decisions.
        let mut suggestion_accepted = 0usize;
        let mut suggestion_rejected = 0usize;
        for (suggestion, decision) in &pending_suggestions {
            match decision {
                SuggestionDecision::Accept => {
                    agenda.accept_classification_suggestion(suggestion.id)?;
                    suggestion_accepted += 1;
                }
                SuggestionDecision::Reject => {
                    agenda.reject_classification_suggestion(suggestion.id)?;
                    suggestion_rejected += 1;
                }
                SuggestionDecision::Pending => {}
            }
        }

        self.push_undo(UndoEntry::ItemEdited {
            item_id,
            old_text: undo_old_text,
            old_note: undo_old_note,
        });
        self.refresh(agenda.store())?;
        self.set_item_selection_by_id(item_id);
        self.input_panel = None;
        self.mode = Mode::Normal;
        let mut status = "Item updated".to_string();
        if suggestion_accepted + suggestion_rejected > 0 {
            status = format!(
                "{status} ({suggestion_accepted} accepted, {suggestion_rejected} rejected)"
            );
        }
        if let Some(suffix) = self.classification_pending_suffix() {
            status = format!("{status} | {suffix}. Press C to review.");
        }
        self.status = status;
        Ok(())
    }

    /// Returns the mode to return to when a NameInput panel is canceled or completed.
    fn name_input_return_mode(&self) -> Mode {
        match self.name_input_context {
            Some(NameInputContext::ViewCreate)
            | Some(NameInputContext::ViewRename)
            | Some(NameInputContext::ViewClone) => Mode::ViewPicker,
            Some(NameInputContext::NumericValueEdit) => Mode::Normal,
            Some(NameInputContext::WhenDateEdit) => Mode::Normal,
            Some(NameInputContext::CategoryCreate) => Mode::CategoryManager,
            None => Mode::Normal,
        }
    }

    fn parse_when_datetime_input_with_reference_date(
        input: &str,
        reference_date: Date,
    ) -> TuiResult<Option<DateTime>> {
        use agenda_core::dates::DateParser;

        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }

        if let Ok(value) = trimmed.replace(' ', "T").parse::<DateTime>() {
            return Ok(Some(value));
        }
        if let Ok(date_only) = trimmed.parse::<Date>() {
            return Ok(Some(date_only.at(0, 0, 0, 0)));
        }

        let parser = agenda_core::dates::BasicDateParser::default();
        if let Some(parsed) = parser.parse(trimmed, reference_date) {
            return Ok(Some(parsed.datetime));
        }

        Err(format!(
            "Could not parse '{}'. Try: today, tomorrow, next week, in 3 days, end of month, YYYY-MM-DD, M/D/YY",
            trimmed,
        ).into())
    }

    fn parse_when_datetime_input(input: &str) -> TuiResult<Option<DateTime>> {
        Self::parse_when_datetime_input_with_reference_date(input, jiff::Zoned::now().date())
    }

    /// Save an InputPanel(NameInput) — dispatches on name_input_context.
    fn save_input_panel_name(&mut self, agenda: &Agenda<'_>) -> TuiResult<()> {
        let input_text = self
            .input_panel
            .as_ref()
            .map(|p| p.text.trimmed().to_string())
            .unwrap_or_default();

        match self.name_input_context {
            Some(NameInputContext::ViewCreate) => {
                if input_text.is_empty() {
                    self.status = "Name cannot be empty".to_string();
                    return Ok(());
                }
                let name = input_text.clone();
                if self
                    .views
                    .iter()
                    .any(|view| view.name.eq_ignore_ascii_case(&name))
                {
                    self.status = format!("View \"{name}\" already exists");
                    return Ok(());
                }
                let mut view = View::new(name.clone());
                if view.sections.is_empty() {
                    view.sections.push(Self::view_edit_default_section(
                        Self::DEFAULT_VIEW_EDIT_SECTION_TITLE,
                    ));
                }
                self.input_panel = None;
                self.name_input_context = None;
                self.open_view_edit_new_view_focus_first_section(view);
            }
            Some(NameInputContext::ViewClone) => {
                if input_text.is_empty() {
                    self.status = "Name cannot be empty".to_string();
                    return Ok(());
                }
                let name = input_text.clone();
                if self
                    .views
                    .iter()
                    .any(|view| view.name.eq_ignore_ascii_case(&name))
                {
                    self.status = format!("View \"{name}\" already exists");
                    return Ok(());
                }
                let Some(source_id) = self.view_pending_clone_id.take() else {
                    self.input_panel = None;
                    self.name_input_context = None;
                    self.mode = Mode::ViewPicker;
                    self.status = "Clone failed: no source view".to_string();
                    return Ok(());
                };
                match agenda.store().clone_view(source_id, name.clone()) {
                    Ok(_cloned) => {
                        self.refresh(agenda.store())?;
                        self.set_view_selection_by_name(&name);
                        self.input_panel = None;
                        self.name_input_context = None;
                        self.mode = Mode::ViewPicker;
                        self.status = format!("Cloned view as '{name}'");
                    }
                    Err(err) => {
                        self.input_panel = None;
                        self.name_input_context = None;
                        self.mode = Mode::ViewPicker;
                        self.status = format!("Clone failed: {err}");
                    }
                }
            }
            Some(NameInputContext::ViewRename) => {
                if input_text.is_empty() {
                    self.status = "Name cannot be empty".to_string();
                    return Ok(());
                }
                let name = input_text.clone();
                let Some(old_name) = self.view_pending_edit_name.clone() else {
                    self.input_panel = None;
                    self.name_input_context = None;
                    self.mode = Mode::ViewPicker;
                    self.status = "View rename failed: no selected view".to_string();
                    return Ok(());
                };
                let Some(mut view) = self
                    .views
                    .iter()
                    .find(|v| v.name.eq_ignore_ascii_case(&old_name))
                    .cloned()
                else {
                    self.input_panel = None;
                    self.name_input_context = None;
                    self.view_pending_edit_name = None;
                    self.mode = Mode::ViewPicker;
                    self.status = "View rename failed: selected view not found".to_string();
                    return Ok(());
                };
                if agenda_core::store::is_system_view_name(&view.name) {
                    self.input_panel = None;
                    self.name_input_context = None;
                    self.view_pending_edit_name = None;
                    self.mode = Mode::ViewPicker;
                    self.status = format!("View rename failed: {} view is immutable", view.name);
                    return Ok(());
                }
                if view.name == name {
                    self.input_panel = None;
                    self.name_input_context = None;
                    self.view_pending_edit_name = None;
                    self.mode = Mode::ViewPicker;
                    self.status = "View rename canceled (unchanged)".to_string();
                    return Ok(());
                }
                view.name = name.clone();
                match agenda.store().update_view(&view) {
                    Ok(()) => {
                        self.refresh(agenda.store())?;
                        self.set_view_selection_by_name(&name);
                        self.input_panel = None;
                        self.name_input_context = None;
                        self.view_pending_edit_name = None;
                        self.mode = Mode::ViewPicker;
                        self.status = format!("Renamed view to {name}");
                    }
                    Err(err) => {
                        self.input_panel = None;
                        self.name_input_context = None;
                        self.view_pending_edit_name = None;
                        self.mode = Mode::ViewPicker;
                        self.status = format!("View rename failed: {err}");
                    }
                }
            }
            Some(NameInputContext::NumericValueEdit) => {
                let name = input_text;
                let Some(target) = self.numeric_edit_target.take() else {
                    self.input_panel = None;
                    self.name_input_context = None;
                    self.mode = Mode::Normal;
                    self.status = "Numeric edit: no target".to_string();
                    return Ok(());
                };

                if name.is_empty() {
                    self.status =
                        "Value cannot be empty. Enter a number (for example 12.50).".to_string();
                    return Ok(());
                }

                let normalized = name.replace(',', "");
                // Capture old numeric value for undo
                let old_numeric_value = self
                    .all_items
                    .iter()
                    .find(|i| i.id == target.item_id)
                    .and_then(|i| i.assignments.get(&target.category_id))
                    .and_then(|a| a.numeric_value);
                match normalized.parse::<rust_decimal::Decimal>() {
                    Ok(decimal_value) => {
                        match agenda.assign_item_numeric_manual(
                            target.item_id,
                            target.category_id,
                            decimal_value,
                            Some("manual:tui.numeric-edit".to_string()),
                        ) {
                            Ok(_result) => {
                                self.push_undo(UndoEntry::NumericValueSet {
                                    item_id: target.item_id,
                                    category_id: target.category_id,
                                    old_value: old_numeric_value,
                                });
                                self.refresh(agenda.store())?;
                                self.input_panel = None;
                                self.name_input_context = None;
                                self.mode = Mode::Normal;
                                self.status = format!("Set value to {}", decimal_value.normalize());
                            }
                            Err(err) => {
                                self.input_panel = None;
                                self.name_input_context = None;
                                self.mode = Mode::Normal;
                                self.status = format!("Numeric save failed: {err}");
                            }
                        }
                    }
                    Err(_) => {
                        // Keep the panel open so the user can fix the input.
                        self.numeric_edit_target = Some(target);
                        self.status = format!(
                            "Invalid number: '{}'. Edit the value or Esc to cancel.",
                            name
                        );
                        return Ok(());
                    }
                }
            }
            Some(NameInputContext::WhenDateEdit) => {
                let Some(target) = self.when_edit_target.take() else {
                    self.input_panel = None;
                    self.name_input_context = None;
                    self.mode = Mode::Normal;
                    self.status = "When edit: no target".to_string();
                    return Ok(());
                };

                let parsed_when = match Self::parse_when_datetime_input(&input_text) {
                    Ok(value) => value,
                    Err(err) => {
                        self.when_edit_target = Some(target);
                        self.status = err.to_string();
                        return Ok(());
                    }
                };

                match agenda.set_item_when_date(
                    target.item_id,
                    parsed_when,
                    Some("manual:tui.when-edit".to_string()),
                ) {
                    Ok(_result) => {
                        self.refresh(agenda.store())?;
                        self.set_item_selection_by_id(target.item_id);
                        self.input_panel = None;
                        self.name_input_context = None;
                        self.mode = Mode::Normal;
                        self.status = match parsed_when {
                            Some(value) => {
                                format!("When set to {}", value.strftime("%Y-%m-%d %H:%M"))
                            }
                            None => "When cleared".to_string(),
                        };
                    }
                    Err(err) => {
                        self.input_panel = None;
                        self.name_input_context = None;
                        self.mode = Mode::Normal;
                        self.status = format!("When save failed: {err}");
                    }
                }
            }
            Some(NameInputContext::CategoryCreate) => {
                // Should not be reached — CategoryCreate has its own save function
                self.input_panel = None;
                self.name_input_context = None;
                self.mode = Mode::CategoryManager;
                self.status = "Unexpected save dispatch for CategoryCreate".to_string();
            }
            None => {
                self.input_panel = None;
                self.name_input_context = None;
                self.mode = Mode::Normal;
                self.status = "NameInput save: no context".to_string();
            }
        }
        Ok(())
    }

    /// Save an InputPanel(CategoryCreate) — creates a new category.
    fn save_input_panel_category_create(&mut self, agenda: &Agenda<'_>) -> TuiResult<()> {
        let Some(panel) = &self.input_panel else {
            self.mode = self.name_input_return_mode();
            self.name_input_context = None;
            return Ok(());
        };
        let name = panel.text.trimmed().to_string();
        if name.is_empty() {
            self.status = "Name cannot be empty".to_string();
            return Ok(());
        }
        if is_reserved_category_name(&name) {
            self.status = format!(
                "Cannot create reserved category '{}'. Use a different name.",
                name
            );
            return Ok(());
        }
        if self.category_name_exists_elsewhere(&name, None) {
            self.status = format!(
                "Category '{}' already exists. Cannot create duplicate.",
                name
            );
            return Ok(());
        }

        let parent_id = panel.parent_id;
        let value_kind = panel.value_kind;
        let parent_label = panel.parent_label.clone();
        let kind_label = match value_kind {
            CategoryValueKind::Tag => "tag",
            CategoryValueKind::Numeric => "numeric",
        };

        let mut category = Category::new(name.clone());
        category.enable_implicit_string = true;
        category.parent = parent_id;
        category.value_kind = value_kind;

        match agenda.create_category(&category) {
            Ok(result) => {
                self.refresh(agenda.store())?;
                self.set_category_selection_by_id(category.id);
                self.input_panel = None;
                self.set_category_manager_inline_action(None);
                self.name_input_context = None;
                self.mode = Mode::CategoryManager;
                self.status = format!(
                    "Created {kind_label} category {name} under {parent_label} (processed_items={}, affected_items={})",
                    result.processed_items, result.affected_items
                );
            }
            Err(err) => {
                self.status = format!("Create failed: {err}");
            }
        }
        Ok(())
    }

    pub(crate) fn handle_item_assign_category_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> TuiResult<bool> {
        match code {
            KeyCode::Esc => {
                let clear_selection = self.item_assign_dirty && self.has_selected_items();
                self.mode = Mode::Normal;
                if clear_selection {
                    self.clear_selected_items();
                }
                self.item_assign_dirty = false;
                self.clear_input();
                if !clear_selection {
                    self.status = "Assign canceled".to_string();
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if !self.category_rows.is_empty() {
                    self.item_assign_category_index =
                        next_index(self.item_assign_category_index, self.category_rows.len(), 1);
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if !self.category_rows.is_empty() {
                    self.item_assign_category_index = next_index(
                        self.item_assign_category_index,
                        self.category_rows.len(),
                        -1,
                    );
                }
            }
            KeyCode::Char('n') | KeyCode::Char('/') => {
                self.mode = Mode::ItemAssignInput;
                self.clear_input();
                self.status = "Type category name: Enter assign/create, Tab autocomplete, Esc back"
                    .to_string();
            }
            KeyCode::Char(' ') => {
                let batch_mode = self.has_selected_items();
                let action_item_ids = self.effective_action_item_ids();
                let Some(item_id) = self.selected_item_id() else {
                    self.mode = Mode::Normal;
                    self.status = "Assign failed: no selected item".to_string();
                    return Ok(false);
                };
                let Some(row) = self
                    .category_rows
                    .get(self.item_assign_category_index)
                    .cloned()
                else {
                    self.status = "Assign failed: no category selected".to_string();
                    return Ok(false);
                };

                if row.name.eq_ignore_ascii_case("Done") {
                    if batch_mode && action_item_ids.len() > 1 {
                        if let Err(err) = self.batch_toggle_selected_items_done(
                            agenda,
                            DoneToggleOrigin::ItemAssignPicker,
                        ) {
                            self.status = format!("Done toggle failed: {}", err);
                        }
                        return Ok(false);
                    }
                    if let Err(err) = self.begin_done_toggle_or_confirm(
                        agenda,
                        item_id,
                        DoneToggleOrigin::ItemAssignPicker,
                    ) {
                        self.status = format!("Done toggle failed: {}", err);
                    }
                    return Ok(false);
                }

                if batch_mode && action_item_ids.len() > 1 {
                    let (assigned_count, total_count) =
                        self.effective_action_assignment_counts(row.id);
                    let should_unassign = assigned_count == total_count;
                    let mut changed = 0usize;
                    let mut failed = 0usize;
                    let mut first_error = None;

                    for action_item_id in &action_item_ids {
                        let result = if should_unassign {
                            agenda.unassign_item_manual(*action_item_id, row.id)
                        } else {
                            agenda
                                .assign_item_manual(
                                    *action_item_id,
                                    row.id,
                                    Some("manual:tui.assign".to_string()),
                                )
                                .map(|_| ())
                        };
                        match result {
                            Ok(()) => changed += 1,
                            Err(err) => {
                                failed += 1;
                                if first_error.is_none() {
                                    first_error = Some(err.to_string());
                                }
                            }
                        }
                    }

                    self.refresh(agenda.store())?;
                    self.set_item_selection_by_id(item_id);
                    if changed > 0 && failed == 0 {
                        self.item_assign_dirty = true;
                    }
                    self.status = if failed == 0 {
                        if should_unassign {
                            format!("Removed category {} from {} items", row.name, changed)
                        } else {
                            format!("Applied category {} to {} items", row.name, changed)
                        }
                    } else {
                        let mut summary = if should_unassign {
                            format!(
                                "Removed category {} from {} items (failed={failed})",
                                row.name, changed
                            )
                        } else {
                            format!(
                                "Applied category {} to {} items (failed={failed})",
                                row.name, changed
                            )
                        };
                        if let Some(err) = first_error {
                            summary.push_str(&format!(" first_error={err}"));
                        }
                        summary
                    };
                } else if self.selected_item_has_assignment(row.id) {
                    // Capture old assignment for undo
                    let old_assignment = self
                        .all_items
                        .iter()
                        .find(|i| i.id == item_id)
                        .and_then(|i| i.assignments.get(&row.id))
                        .cloned();
                    match agenda.unassign_item_manual(item_id, row.id) {
                        Ok(()) => {
                            if let Some(assignment) = old_assignment {
                                self.push_undo(UndoEntry::CategoryUnassigned {
                                    item_id,
                                    category_id: row.id,
                                    old_assignment: assignment,
                                });
                            }
                            self.refresh(agenda.store())?;
                            self.set_item_selection_by_id(item_id);
                            self.status = format!("Removed category {}", row.name);
                        }
                        Err(err) => {
                            self.status = format!("Cannot remove {}: {}", row.name, err);
                        }
                    }
                } else {
                    let result = agenda.assign_item_manual(
                        item_id,
                        row.id,
                        Some("manual:tui.assign".to_string()),
                    )?;
                    self.push_undo(UndoEntry::CategoryAssigned {
                        item_id,
                        category_id: row.id,
                    });
                    self.refresh(agenda.store())?;
                    self.set_item_selection_by_id(item_id);
                    self.status = format!(
                        "Added category {} (new_assignments={})",
                        row.name,
                        result.new_assignments.len()
                    );
                }
            }
            KeyCode::Enter => {
                let clear_selection = self.item_assign_dirty && self.has_selected_items();
                self.mode = Mode::Normal;
                if clear_selection {
                    self.clear_selected_items();
                }
                self.item_assign_dirty = false;
                self.clear_input();
                if !clear_selection {
                    self.status = "Category edit saved".to_string();
                }
            }
            _ => {}
        }

        Ok(false)
    }

    pub(crate) fn handle_item_assign_category_input_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> TuiResult<bool> {
        match code {
            KeyCode::Esc => {
                self.mode = Mode::ItemAssignPicker;
                self.clear_input();
                self.status = "Category name entry canceled".to_string();
            }
            KeyCode::Enter => {
                let action_item_ids = self.effective_action_item_ids();
                let Some(item_id) = self.selected_item_id() else {
                    self.mode = Mode::Normal;
                    self.clear_input();
                    self.status = "Assign failed: no selected item".to_string();
                    return Ok(false);
                };
                let name = self.input.trimmed().to_string();
                if name.is_empty() {
                    self.mode = Mode::ItemAssignPicker;
                    self.clear_input();
                    self.status = "Category name entry canceled (empty)".to_string();
                    return Ok(false);
                }

                let exact_match = self
                    .categories
                    .iter()
                    .find(|category| category.name.eq_ignore_ascii_case(&name))
                    .map(|category| (category.id, category.name.clone()));
                let single_visible_match = if exact_match.is_none() {
                    let query = name.to_ascii_lowercase();
                    let mut matching_rows = self
                        .category_rows
                        .iter()
                        .filter(|row| row.name.to_ascii_lowercase().contains(&query));
                    match (matching_rows.next(), matching_rows.next()) {
                        (Some(row), None) => Some((row.id, row.name.clone())),
                        _ => None,
                    }
                } else {
                    None
                };

                let mut created_new_category = false;
                let (category_id, category_name) =
                    if let Some((category_id, category_name)) = exact_match {
                        (category_id, category_name)
                    } else if let Some((category_id, category_name)) = single_visible_match {
                        (category_id, category_name)
                    } else {
                        let mut category = Category::new(name.clone());
                        category.enable_implicit_string = true;
                        let category_id = category.id;
                        agenda.store().create_category(&category)?;
                        created_new_category = true;
                        (category_id, category.name)
                    };
                let mut assigned = 0usize;
                let mut already_had = 0usize;
                let mut failed = 0usize;
                let mut first_error = None;

                for action_item_id in &action_item_ids {
                    let item = agenda.store().get_item(*action_item_id)?;
                    if item.assignments.contains_key(&category_id) {
                        already_had += 1;
                        continue;
                    }

                    match agenda.assign_item_manual(
                        *action_item_id,
                        category_id,
                        Some("manual:tui.assign".to_string()),
                    ) {
                        Ok(_) => assigned += 1,
                        Err(err) => {
                            failed += 1;
                            if first_error.is_none() {
                                first_error = Some(err.to_string());
                            }
                        }
                    }
                }

                self.refresh(agenda.store())?;
                self.set_item_selection_by_id(item_id);
                if let Some(index) = self
                    .category_rows
                    .iter()
                    .position(|row| row.id == category_id)
                {
                    self.item_assign_category_index = index;
                }
                self.set_item_selection_by_id(item_id);
                self.mode = Mode::ItemAssignPicker;
                if assigned > 0 && failed == 0 {
                    self.item_assign_dirty = true;
                }
                self.clear_input();
                self.status = if action_item_ids.len() > 1 {
                    let mut summary = format!(
                        "{} category {} to {} items (assigned={}, already={}, failed={})",
                        if created_new_category {
                            "Created and applied"
                        } else {
                            "Applied"
                        },
                        category_name,
                        action_item_ids.len(),
                        assigned,
                        already_had,
                        failed
                    );
                    if let Some(err) = first_error {
                        summary.push_str(&format!(" first_error={err}"));
                    }
                    summary
                } else if failed > 0 {
                    if created_new_category {
                        format!(
                            "Created category {} but could not assign: {}",
                            category_name,
                            first_error.unwrap_or_else(|| "unknown error".to_string())
                        )
                    } else {
                        format!(
                            "Could not assign category {}: {}",
                            category_name,
                            first_error.unwrap_or_else(|| "unknown error".to_string())
                        )
                    }
                } else if created_new_category {
                    format!("Created and assigned category {}", category_name)
                } else if already_had > 0 {
                    format!("Category {} already assigned", category_name)
                } else {
                    format!("Assigned category {}", category_name)
                };
            }
            _ if self.handle_text_input_key(code) => {}
            _ => {}
        }

        Ok(false)
    }

    pub(crate) fn handle_inspect_unassign_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> TuiResult<bool> {
        let rows = self
            .selected_item()
            .map(|item| self.inspect_assignment_rows_for_item(item))
            .unwrap_or_default();

        match code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.status = "Unassign canceled".to_string();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if !rows.is_empty() {
                    self.inspect_assignment_index =
                        next_index(self.inspect_assignment_index, rows.len(), 1);
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if !rows.is_empty() {
                    self.inspect_assignment_index =
                        next_index(self.inspect_assignment_index, rows.len(), -1);
                }
            }
            KeyCode::Enter => {
                let Some(item_id) = self.selected_item_id() else {
                    self.mode = Mode::Normal;
                    self.status = "Unassign failed: no selected item".to_string();
                    return Ok(false);
                };
                let Some(row) = rows.get(self.inspect_assignment_index).cloned() else {
                    self.mode = Mode::Normal;
                    self.status = "Unassign failed: no assignment selected".to_string();
                    return Ok(false);
                };

                agenda.unassign_item_manual(item_id, row.category_id)?;
                self.refresh(agenda.store())?;
                self.set_item_selection_by_id(item_id);
                self.mode = Mode::Normal;
                self.status = format!("Unassigned {}", row.category_name);
            }
            _ => {}
        }

        Ok(false)
    }

    pub(crate) fn handle_search_bar_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> TuiResult<bool> {
        match code {
            KeyCode::Esc => {
                if self.global_search_active() {
                    self.restore_global_search_session(agenda)?;
                } else {
                    self.search_buffer.clear();
                    if self.slot_index < self.section_filters.len() {
                        self.section_filters[self.slot_index] = None;
                    }
                    self.mode = Mode::Normal;
                    self.refresh(agenda.store())?;
                    self.status = "Search cleared".to_string();
                }
            }
            KeyCode::Enter => {
                let query = self.search_buffer.trimmed().to_string();
                if query.is_empty() {
                    self.mode = Mode::Normal;
                } else if let Some((slot_idx, item_idx)) = self.find_first_visible_search_result() {
                    self.slot_index = slot_idx;
                    self.item_index = item_idx;
                    self.open_input_panel_edit_item();
                } else {
                    self.status = format!("No items match '{}'", query);
                }
            }
            KeyCode::Down | KeyCode::Tab => {
                self.mode = Mode::Normal; // keep filter active
            }
            _ => {
                if self
                    .search_buffer
                    .handle_key_event(self.text_key_event(code), false)
                {
                    self.apply_search_filter();
                    self.refresh(agenda.store())?;
                }
            }
        }
        Ok(false)
    }

    fn apply_search_filter(&mut self) {
        let text = self.search_buffer.trimmed().to_string();
        let filter = if text.is_empty() { None } else { Some(text) };
        if self.global_search_active() {
            for slot_filter in &mut self.section_filters {
                *slot_filter = filter.clone();
            }
        } else {
            let slot = self.slot_index;
            if slot < self.section_filters.len() {
                self.section_filters[slot] = filter;
            }
        }
    }

    fn find_first_visible_search_result(&self) -> Option<(usize, usize)> {
        if self.global_search_active() {
            self.slots
                .iter()
                .enumerate()
                .find_map(|(slot_index, slot)| (!slot.items.is_empty()).then_some((slot_index, 0)))
        } else {
            self.current_slot()
                .and_then(|slot| (!slot.items.is_empty()).then_some((self.slot_index, 0)))
        }
    }
}

fn toggle_board_display_mode(mode: BoardDisplayMode) -> BoardDisplayMode {
    match mode {
        BoardDisplayMode::SingleLine => BoardDisplayMode::MultiLine,
        BoardDisplayMode::MultiLine => BoardDisplayMode::SingleLine,
    }
}

/// Returns the CategoryIds of all siblings that should be deselected when `row_idx`
/// is selected in an exclusive-parent group.
///
/// Walks backward from `row_idx` to find the parent row (depth = row.depth - 1).
/// If the parent has `is_exclusive = true`, returns all direct children of that parent
/// except the one at `row_idx`.  Returns an empty vec if no exclusivity applies.
fn exclusive_siblings_to_clear(rows: &[CategoryListRow], row_idx: usize) -> Vec<CategoryId> {
    let Some(row) = rows.get(row_idx) else {
        return vec![];
    };
    if row.depth == 0 {
        return vec![];
    }
    let target_depth = row.depth;
    let parent_depth = target_depth - 1;

    // Find the nearest ancestor at parent_depth (depth-first layout, so walk backward).
    let parent_idx = match (0..row_idx).rev().find(|&i| rows[i].depth == parent_depth) {
        Some(i) => i,
        None => return vec![],
    };

    if !rows[parent_idx].is_exclusive {
        return vec![];
    }

    // Collect all direct children of the parent (depth == target_depth) within its subtree.
    let mut siblings = vec![];
    for (i, row) in rows.iter().enumerate().skip(parent_idx + 1) {
        if row.depth <= parent_depth {
            break; // Exited the parent's subtree
        }
        if row.depth == target_depth && i != row_idx {
            siblings.push(row.id);
        }
    }
    siblings
}

#[cfg(test)]
mod tests {
    use super::*;
    use agenda_core::dates::{BasicDateParser, DateParser};
    use agenda_core::model::CategoryId;
    use jiff::civil::Date;

    fn make_row(id: CategoryId, depth: usize, is_exclusive: bool) -> CategoryListRow {
        CategoryListRow {
            id,
            name: id.to_string(),
            depth,
            is_reserved: false,
            has_note: false,
            is_exclusive,
            is_actionable: false,
            enable_implicit_string: false,
            value_kind: CategoryValueKind::Tag,
        }
    }

    fn direct_edit_state_with_rows(count: usize) -> CategoryDirectEditState {
        CategoryDirectEditState {
            anchor: CategoryDirectEditAnchor {
                slot_index: 0,
                section_index: 0,
                section_column_index: 0,
                board_column_index: 1,
                is_generated_section: false,
            },
            parent_id: CategoryId::new_v4(),
            parent_name: "Parent".to_string(),
            item_id: ItemId::new_v4(),
            item_label: "Demo".to_string(),
            rows: (0..count)
                .map(|idx| CategoryDirectEditRow {
                    input: text_buffer::TextBuffer::new(format!("row{idx}")),
                    category_id: None,
                })
                .collect(),
            active_row: 0,
            focus: CategoryDirectEditFocus::Input,
            suggest_index: 0,
            create_confirm_name: None,
            original_category_ids: vec![None; count],
        }
    }

    #[test]
    fn exclusive_siblings_cleared_when_parent_is_exclusive() {
        // Priority (depth=0, exclusive=true)
        //   High   (depth=1)
        //   Medium (depth=1)
        //   Low    (depth=1)
        let p = CategoryId::new_v4();
        let high = CategoryId::new_v4();
        let medium = CategoryId::new_v4();
        let low = CategoryId::new_v4();
        let rows = vec![
            make_row(p, 0, true),
            make_row(high, 1, false),
            make_row(medium, 1, false),
            make_row(low, 1, false),
        ];
        // Selecting "Medium" (idx=2) should return High and Low as siblings to clear.
        let to_clear = exclusive_siblings_to_clear(&rows, 2);
        assert!(to_clear.contains(&high));
        assert!(to_clear.contains(&low));
        assert!(!to_clear.contains(&medium));
    }

    #[test]
    fn exclusive_siblings_empty_when_parent_is_not_exclusive() {
        let p = CategoryId::new_v4();
        let a = CategoryId::new_v4();
        let b = CategoryId::new_v4();
        let rows = vec![
            make_row(p, 0, false), // NOT exclusive
            make_row(a, 1, false),
            make_row(b, 1, false),
        ];
        assert!(exclusive_siblings_to_clear(&rows, 1).is_empty());
    }

    #[test]
    fn exclusive_siblings_empty_for_top_level_row() {
        let a = CategoryId::new_v4();
        let rows = vec![make_row(a, 0, true)];
        assert!(exclusive_siblings_to_clear(&rows, 0).is_empty());
    }

    #[test]
    fn exclusive_siblings_respects_subtree_boundary() {
        // Priority (depth=0, exclusive)
        //   High   (depth=1)
        //   Low    (depth=1)
        // Status  (depth=0, exclusive)
        //   Open   (depth=1)
        let priority = CategoryId::new_v4();
        let high = CategoryId::new_v4();
        let low = CategoryId::new_v4();
        let status = CategoryId::new_v4();
        let open = CategoryId::new_v4();
        let rows = vec![
            make_row(priority, 0, true),
            make_row(high, 1, false),
            make_row(low, 1, false),
            make_row(status, 0, true),
            make_row(open, 1, false),
        ];
        // Selecting "High" (idx=1) should only return "Low", not "Open".
        let to_clear = exclusive_siblings_to_clear(&rows, 1);
        assert_eq!(to_clear, vec![low]);
        assert!(!to_clear.contains(&open));
    }

    #[test]
    fn direct_edit_focus_cycles_forward_and_back() {
        let mut app = App {
            category_direct_edit: Some(direct_edit_state_with_rows(1)),
            ..App::default()
        };

        assert_eq!(
            app.active_category_direct_edit_focus(),
            Some(CategoryDirectEditFocus::Input)
        );

        app.cycle_category_direct_edit_focus(true);
        assert_eq!(
            app.active_category_direct_edit_focus(),
            Some(CategoryDirectEditFocus::Suggestions)
        );

        app.cycle_category_direct_edit_focus(true);
        assert_eq!(
            app.active_category_direct_edit_focus(),
            Some(CategoryDirectEditFocus::Entries)
        );

        app.cycle_category_direct_edit_focus(false);
        assert_eq!(
            app.active_category_direct_edit_focus(),
            Some(CategoryDirectEditFocus::Suggestions)
        );
    }

    #[test]
    fn direct_edit_row_navigation_clamps_and_syncs_active_input() {
        let mut state = direct_edit_state_with_rows(3);
        state.active_row = 1;
        let mut app = App {
            category_direct_edit: Some(state),
            ..App::default()
        };

        app.move_category_direct_edit_active_row(1);
        assert_eq!(
            app.category_direct_edit_state().map(|s| s.active_row),
            Some(2)
        );
        assert_eq!(app.active_category_direct_edit_input_text(), Some("row2"));

        app.move_category_direct_edit_active_row(1);
        assert_eq!(
            app.category_direct_edit_state().map(|s| s.active_row),
            Some(2)
        );

        app.move_category_direct_edit_active_row(-99);
        assert_eq!(
            app.category_direct_edit_state().map(|s| s.active_row),
            Some(0)
        );
        assert_eq!(app.active_category_direct_edit_input_text(), Some("row0"));
    }

    #[test]
    fn direct_edit_remove_row_keeps_single_blank_row() {
        let mut app = App {
            category_direct_edit: Some(direct_edit_state_with_rows(1)),
            ..App::default()
        };

        app.remove_active_category_direct_edit_row();

        let state = app.category_direct_edit_state().expect("state");
        assert_eq!(state.rows.len(), 1);
        assert_eq!(state.active_row, 0);
        assert!(state.rows[0].input.text().is_empty());
        assert!(app.status.contains("last row"));
    }

    #[test]
    fn current_column_parent_is_exclusive_detects_parent_flag() {
        let mut parent = Category::new("Status".to_string());
        parent.is_exclusive = true;
        let mut child = Category::new("Pending".to_string());
        child.parent = Some(parent.id);
        parent.children = vec![child.id];
        let item = Item::new("Demo".to_string());

        let mut view = View::new("Board".to_string());
        view.sections.push(Section {
            title: "Main".to_string(),
            criteria: Query::default(),
            columns: vec![Column {
                kind: ColumnKind::Standard,
                heading: parent.id,
                width: 12,
                summary_fn: None,
            }],
            item_column_index: 0,
            on_insert_assign: HashSet::new(),
            on_remove_unassign: HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        });

        let app = App {
            categories: vec![parent.clone(), child],
            views: vec![view],
            slots: vec![Slot {
                title: "Main".to_string(),
                items: vec![item],
                context: SlotContext::Section { section_index: 0 },
            }],
            view_index: 0,
            slot_index: 0,
            item_index: 0,
            column_index: 1,
            ..App::default()
        };

        assert!(app.current_column_parent_is_exclusive());
    }

    #[test]
    fn add_blank_row_guard_blocks_second_row_for_exclusive_parent() {
        let parent_id = CategoryId::new_v4();
        let item_id = ItemId::new_v4();

        let mut app = App {
            categories: vec![{
                let mut c = Category::new("Priority".to_string());
                c.id = parent_id;
                c.is_exclusive = true;
                c
            }],
            category_direct_edit: Some(CategoryDirectEditState {
                anchor: CategoryDirectEditAnchor {
                    slot_index: 0,
                    section_index: 0,
                    section_column_index: 0,
                    board_column_index: 1,
                    is_generated_section: false,
                },
                parent_id,
                parent_name: "Priority".to_string(),
                item_id,
                item_label: "Demo".to_string(),
                rows: vec![CategoryDirectEditRow::blank()],
                active_row: 0,
                focus: CategoryDirectEditFocus::Input,
                suggest_index: 0,
                create_confirm_name: None,
                original_category_ids: vec![None],
            }),
            ..App::default()
        };

        // Build enough board context for the current-column helper path.
        let mut view = View::new("Board".to_string());
        view.sections.push(Section {
            title: "Main".to_string(),
            criteria: Query::default(),
            columns: vec![Column {
                kind: ColumnKind::Standard,
                heading: parent_id,
                width: 12,
                summary_fn: None,
            }],
            item_column_index: 0,
            on_insert_assign: HashSet::new(),
            on_remove_unassign: HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        });
        app.views = vec![view];
        app.slots = vec![Slot {
            title: "Main".to_string(),
            items: vec![Item {
                id: item_id,
                ..Item::new("Demo".to_string())
            }],
            context: SlotContext::Section { section_index: 0 },
        }];
        app.view_index = 0;
        app.slot_index = 0;
        app.item_index = 0;
        app.column_index = 1;

        assert!(!app.category_direct_edit_add_blank_row_guarded());
        let state = app.category_direct_edit_state().expect("state");
        assert_eq!(state.rows.len(), 1);
        assert!(app.status.contains("exclusive"));
    }

    #[test]
    fn add_blank_row_guard_allows_non_exclusive_parent() {
        let parent_id = CategoryId::new_v4();
        let item_id = ItemId::new_v4();

        let mut app = App {
            categories: vec![{
                let mut c = Category::new("Tags".to_string());
                c.id = parent_id;
                c.is_exclusive = false;
                c
            }],
            category_direct_edit: Some(CategoryDirectEditState {
                anchor: CategoryDirectEditAnchor {
                    slot_index: 0,
                    section_index: 0,
                    section_column_index: 0,
                    board_column_index: 1,
                    is_generated_section: false,
                },
                parent_id,
                parent_name: "Tags".to_string(),
                item_id,
                item_label: "Demo".to_string(),
                rows: vec![CategoryDirectEditRow::blank()],
                active_row: 0,
                focus: CategoryDirectEditFocus::Input,
                suggest_index: 0,
                create_confirm_name: None,
                original_category_ids: vec![None],
            }),
            ..App::default()
        };

        let mut view = View::new("Board".to_string());
        view.sections.push(Section {
            title: "Main".to_string(),
            criteria: Query::default(),
            columns: vec![Column {
                kind: ColumnKind::Standard,
                heading: parent_id,
                width: 12,
                summary_fn: None,
            }],
            item_column_index: 0,
            on_insert_assign: HashSet::new(),
            on_remove_unassign: HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        });
        app.views = vec![view];
        app.slots = vec![Slot {
            title: "Main".to_string(),
            items: vec![Item {
                id: item_id,
                ..Item::new("Demo".to_string())
            }],
            context: SlotContext::Section { section_index: 0 },
        }];
        app.view_index = 0;
        app.slot_index = 0;
        app.item_index = 0;
        app.column_index = 1;

        assert!(app.category_direct_edit_add_blank_row_guarded());
        let state = app.category_direct_edit_state().expect("state");
        assert_eq!(state.rows.len(), 2);
        assert_eq!(state.active_row, 1);
    }

    #[test]
    fn input_panel_category_filter_narrows_rows_and_toggles_match() {
        let store = Store::open_memory().expect("open store");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let alpha = Category::new("Alpha".to_string());
        let beta = Category::new("Beta".to_string());
        let gamma = Category::new("Gamma".to_string());

        let mut app = App {
            categories: vec![alpha.clone(), beta.clone(), gamma.clone()],
            ..App::default()
        };
        app.category_rows = build_category_rows(&app.categories);
        app.input_panel = Some(input_panel::InputPanel::new_add_item(
            "Main",
            &HashSet::new(),
        ));
        app.mode = Mode::InputPanel;
        if let Some(panel) = &mut app.input_panel {
            panel.focus = input_panel::InputPanelFocus::Categories;
        }

        app.handle_input_panel_key(KeyCode::Char('/'), &agenda)
            .expect("open filter");
        app.handle_input_panel_key(KeyCode::Char('b'), &agenda)
            .expect("type filter");
        assert_eq!(
            app.input_panel_visible_category_row_indices().len(),
            1,
            "filter should narrow rows to a single match"
        );
        assert_eq!(
            app.input_panel_selected_category_row()
                .map(|row| row.name.as_str()),
            Some("Beta")
        );

        app.handle_input_panel_key(KeyCode::Enter, &agenda)
            .expect("finish filter editing");
        app.handle_input_panel_key(KeyCode::Char(' '), &agenda)
            .expect("toggle selected category");

        let panel = app.input_panel.as_ref().expect("panel");
        assert!(panel.categories.contains(&beta.id));
        assert!(!panel.categories.contains(&alpha.id));
        assert!(!panel.categories.contains(&gamma.id));
    }

    #[test]
    fn input_panel_filter_esc_exits_filter_editing_in_one_step() {
        let store = Store::open_memory().expect("open store");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let alpha = Category::new("Alpha".to_string());
        let beta = Category::new("Beta".to_string());
        let mut app = App {
            categories: vec![alpha, beta],
            ..App::default()
        };
        app.category_rows = build_category_rows(&app.categories);
        app.input_panel = Some(input_panel::InputPanel::new_add_item(
            "Main",
            &HashSet::new(),
        ));
        app.mode = Mode::InputPanel;
        if let Some(panel) = &mut app.input_panel {
            panel.focus = input_panel::InputPanelFocus::Categories;
        }

        app.handle_input_panel_key(KeyCode::Char('/'), &agenda)
            .expect("open filter");
        app.handle_input_panel_key(KeyCode::Char('b'), &agenda)
            .expect("type filter");

        app.handle_input_panel_key(KeyCode::Esc, &agenda)
            .expect("exit filter editing");
        let panel = app.input_panel.as_ref().expect("panel");
        assert!(!panel.category_filter_editing);
        assert_eq!(panel.category_filter.text(), "b");
        assert_eq!(app.mode, Mode::InputPanel);

        app.handle_input_panel_key(KeyCode::Esc, &agenda)
            .expect("cancel panel");
        assert_eq!(app.mode, Mode::Normal);
    }

    #[test]
    fn ctrl_g_sets_pending_external_edit_for_text() {
        let mut app = App {
            input_panel: Some(input_panel::InputPanel::new_add_item(
                "Main",
                &HashSet::new(),
            )),
            mode: Mode::InputPanel,
            current_key_modifiers: KeyModifiers::CONTROL,
            ..App::default()
        };

        let store = Store::open_memory().expect("open store");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);
        app.handle_input_panel_key(KeyCode::Char('g'), &agenda)
            .expect("ctrl+g");
        assert_eq!(app.pending_external_edit, Some(ExternalEditorTarget::Text));
    }

    #[test]
    fn ctrl_g_sets_pending_external_edit_for_note() {
        let mut panel = input_panel::InputPanel::new_add_item("Main", &HashSet::new());
        panel.focus = input_panel::InputPanelFocus::Note;
        let mut app = App {
            input_panel: Some(panel),
            mode: Mode::InputPanel,
            current_key_modifiers: KeyModifiers::CONTROL,
            ..App::default()
        };

        let store = Store::open_memory().expect("open store");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);
        app.handle_input_panel_key(KeyCode::Char('g'), &agenda)
            .expect("ctrl+g");
        assert_eq!(app.pending_external_edit, Some(ExternalEditorTarget::Note));
    }

    #[test]
    fn ctrl_g_not_allowed_from_non_text_focus_in_name_input() {
        let mut panel = input_panel::InputPanel::new_name_input("test", "label");
        panel.focus = input_panel::InputPanelFocus::SaveButton;
        let mut app = App {
            input_panel: Some(panel),
            mode: Mode::InputPanel,
            current_key_modifiers: KeyModifiers::CONTROL,
            ..App::default()
        };

        let store = Store::open_memory().expect("open store");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);
        // NameInput kind with SaveButton focus — not allowed (only AddItem/EditItem
        // allow Ctrl+G from any focus; other kinds require Text focus).
        app.handle_input_panel_key(KeyCode::Char('g'), &agenda)
            .expect("ctrl+g");
        assert_eq!(app.pending_external_edit, None);
    }

    #[test]
    fn ctrl_g_allowed_from_text_focus_in_name_input() {
        let mut app = App {
            input_panel: Some(input_panel::InputPanel::new_name_input("test", "label")),
            mode: Mode::InputPanel,
            current_key_modifiers: KeyModifiers::CONTROL,
            ..App::default()
        };

        let store = Store::open_memory().expect("open store");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);
        app.handle_input_panel_key(KeyCode::Char('g'), &agenda)
            .expect("ctrl+g");
        assert_eq!(app.pending_external_edit, Some(ExternalEditorTarget::Text));
    }

    #[test]
    fn edit_item_esc_from_note_focus_warns_about_discarded_text_edits() {
        let store = Store::open_memory().expect("open store");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut panel = input_panel::InputPanel::new_edit_item(
            ItemId::new_v4(),
            "Original".to_string(),
            "Existing note".to_string(),
            String::new(),
            HashSet::new(),
            HashMap::new(),
            HashMap::new(),
        );
        panel.focus = input_panel::InputPanelFocus::Note;
        panel.note.set("Updated note".to_string());

        let mut app = App {
            input_panel: Some(panel),
            mode: Mode::InputPanel,
            ..App::default()
        };

        app.handle_input_panel_key(KeyCode::Esc, &agenda)
            .expect("open discard confirm");

        assert_eq!(app.mode, Mode::InputPanel);
        assert!(app.input_panel_discard_confirm);
        assert_eq!(
            app.status,
            "Discard unsaved item edits? y:discard n:keep editing"
        );
    }

    #[test]
    fn edit_item_discard_confirm_n_keeps_panel_open() {
        let store = Store::open_memory().expect("open store");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut panel = input_panel::InputPanel::new_edit_item(
            ItemId::new_v4(),
            "Original".to_string(),
            "Existing note".to_string(),
            String::new(),
            HashSet::new(),
            HashMap::new(),
            HashMap::new(),
        );
        panel.note.set("Updated note".to_string());

        let mut app = App {
            input_panel: Some(panel),
            mode: Mode::InputPanel,
            input_panel_discard_confirm: true,
            ..App::default()
        };

        app.handle_input_panel_key(KeyCode::Char('n'), &agenda)
            .expect("keep editing");

        assert_eq!(app.mode, Mode::InputPanel);
        assert!(!app.input_panel_discard_confirm);
        assert!(app.input_panel.is_some());
        assert_eq!(app.status, "Continue editing item");
    }

    #[test]
    fn edit_item_discard_confirm_y_discards_changes() {
        let store = Store::open_memory().expect("open store");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut panel = input_panel::InputPanel::new_edit_item(
            ItemId::new_v4(),
            "Original".to_string(),
            "Existing note".to_string(),
            String::new(),
            HashSet::new(),
            HashMap::new(),
            HashMap::new(),
        );
        panel.note.set("Updated note".to_string());

        let mut app = App {
            input_panel: Some(panel),
            mode: Mode::InputPanel,
            input_panel_discard_confirm: true,
            ..App::default()
        };

        app.handle_input_panel_key(KeyCode::Char('y'), &agenda)
            .expect("discard changes");

        assert_eq!(app.mode, Mode::Normal);
        assert!(!app.input_panel_discard_confirm);
        assert!(app.input_panel.is_none());
        assert_eq!(app.status, "Changes discarded");
    }

    #[test]
    fn add_item_in_ready_section_with_no_children_assigns_ready_category() {
        let store = Store::open_memory().expect("open store");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let aglet = Category::new("Aglet".to_string());
        let mut status = Category::new("Status".to_string());
        status.is_exclusive = true;
        let mut ready = Category::new("Ready".to_string());
        ready.parent = Some(status.id);

        store.create_category(&aglet).expect("create Aglet");
        store.create_category(&status).expect("create Status");
        store.create_category(&ready).expect("create Ready");

        let mut view = View::new("Aglet Board".to_string());
        view.criteria.set_criterion(CriterionMode::And, aglet.id);
        let mut ready_criteria = Query::default();
        ready_criteria.set_criterion(CriterionMode::And, ready.id);
        view.sections.push(Section {
            title: "Ready".to_string(),
            criteria: ready_criteria,
            columns: Vec::new(),
            item_column_index: 0,
            on_insert_assign: HashSet::new(),
            on_remove_unassign: HashSet::new(),
            show_children: true,
            board_display_mode_override: None,
        });
        store.create_view(&view).expect("create view");

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        app.set_view_selection_by_name("Aglet Board");
        app.refresh(&store).expect("refresh aglet board");
        assert!(matches!(
            app.slots.first().map(|slot| &slot.context),
            Some(SlotContext::Section { .. })
        ));

        app.open_input_panel_add_item();
        if let Some(panel) = &mut app.input_panel {
            panel.text.set("Ready task".to_string());
            panel.focus = input_panel::InputPanelFocus::SaveButton;
        }
        app.handle_input_panel_key(KeyCode::Enter, &agenda)
            .expect("save add item");

        let created = store
            .list_items()
            .expect("list items")
            .into_iter()
            .find(|item| item.text == "Ready task")
            .expect("created item");
        let assignments = store
            .get_assignments_for_item(created.id)
            .expect("load assignments");
        assert!(
            assignments.contains_key(&ready.id),
            "Ready should be assigned"
        );
        assert!(
            assignments.contains_key(&aglet.id),
            "view include should be assigned"
        );
    }

    #[test]
    fn parse_when_datetime_input_supports_core_relative_phrase_forms() {
        let reference = Date::new(2026, 2, 16).expect("valid date");
        let input = "next Tuesday at 3pm";
        let expected = BasicDateParser::default()
            .parse(input, reference)
            .expect("core parser should parse")
            .datetime;

        let parsed = App::parse_when_datetime_input_with_reference_date(input, reference)
            .expect("input should parse")
            .expect("should return datetime");

        assert_eq!(parsed, expected);
    }

    #[test]
    fn parse_when_datetime_input_reports_unparsable_input() {
        let reference = Date::new(2026, 2, 16).expect("valid date");
        let err = App::parse_when_datetime_input_with_reference_date("next weem", reference)
            .expect_err("invalid phrase should return error");
        let err_msg = err.to_string();
        assert!(
            err_msg.contains("Could not parse"),
            "unexpected parse error: {err_msg}"
        );
        assert!(
            err_msg.contains("next weem"),
            "error should echo invalid input for debugging: {err_msg}"
        );
    }
}
