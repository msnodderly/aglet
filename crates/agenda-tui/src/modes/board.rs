use crate::*;

enum InlineCreateConfirmKeyAction {
    Confirm,
    Cancel,
    DismissAndContinue,
    None,
}

fn inline_create_confirm_key_action(code: KeyCode) -> InlineCreateConfirmKeyAction {
    match code {
        KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
            InlineCreateConfirmKeyAction::Confirm
        }
        KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
            InlineCreateConfirmKeyAction::Cancel
        }
        KeyCode::Char(_)
        | KeyCode::Backspace
        | KeyCode::Delete
        | KeyCode::Left
        | KeyCode::Right => InlineCreateConfirmKeyAction::DismissAndContinue,
        _ => InlineCreateConfirmKeyAction::None,
    }
}

impl App {
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

    fn refresh_category_cache(&mut self, store: &Store) -> Result<(), String> {
        self.categories = store.get_hierarchy().map_err(|e| e.to_string())?;
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
    ) -> Result<bool, String> {
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

    fn resolve_active_row_from_highlighted_suggestion(&mut self) -> Result<bool, String> {
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
        self.status = format!("Create new category '{}' in this column? (Y/n)", typed);
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
    ) -> Result<BoardAddColumnAnchor, String> {
        let slot = self
            .current_slot()
            .ok_or("No active board slot".to_string())?;
        let (section_index, is_generated_section) = match slot.context {
            SlotContext::Section { section_index } => (section_index, false),
            SlotContext::GeneratedSection { section_index, .. } => (section_index, true),
            SlotContext::Unmatched => {
                return Err("Cannot add columns from unmatched lane".to_string());
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
            return Err("Current column is out of range".to_string());
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
        if category.name.eq_ignore_ascii_case("Entry") {
            return false;
        }
        if category.name.eq_ignore_ascii_case("When") {
            return category.parent.is_none();
        }
        category.parent.is_none() && !category.children.is_empty()
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
                "Add column: type to filter top-level categories with children (or When)"
                    .to_string()
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
            self.status = "Preview opened (Summary). f to focus pane, o for provenance".to_string();
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
            PreviewMode::Provenance => "Preview mode: Provenance".to_string(),
        };
    }

    pub(crate) fn toggle_normal_focus(&mut self) {
        if !self.show_preview {
            self.status = "Preview is closed (press p to open)".to_string();
            return;
        }
        self.normal_focus = match self.normal_focus {
            NormalFocus::Board => NormalFocus::Preview,
            NormalFocus::Preview => NormalFocus::Board,
        };
        self.status = match self.normal_focus {
            NormalFocus::Board => "Focus: Board".to_string(),
            NormalFocus::Preview => "Focus: Preview".to_string(),
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

    pub(crate) fn open_provenance_unassign_picker(&mut self) {
        if !self.show_preview {
            self.status = "Preview is closed (press p to open)".to_string();
            return;
        }
        if self.normal_focus != NormalFocus::Preview {
            self.status = "Focus preview pane to unassign from provenance (f)".to_string();
            return;
        }
        if self.preview_mode != PreviewMode::Provenance {
            self.status = "Switch preview to Provenance mode (o) to unassign".to_string();
            return;
        }
        let Some(item) = self.selected_item() else {
            self.status = "No selected item to unassign".to_string();
            return;
        };
        let rows = self.inspect_assignment_rows_for_item(item);
        if rows.is_empty() {
            self.status = "No assignments available to unassign".to_string();
            return;
        }
        self.mode = Mode::InspectUnassign;
        self.inspect_assignment_index = self.inspect_assignment_index.min(rows.len() - 1);
        self.status = "Select assignment to unassign (j/k, Enter, Esc)".to_string();
    }

    pub(crate) fn open_board_add_column_picker(
        &mut self,
        direction: AddColumnDirection,
    ) -> Result<(), String> {
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
            let parent_label = existing_cat
                .parent
                .and_then(|pid| self.categories.iter().find(|c| c.id == pid))
                .map(|c| c.name.as_str())
                .unwrap_or("(top level)");
            if existing_cat.parent.is_some() {
                self.status = format!(
                    "Category '{}' exists under '{}'. Column headings must be top-level categories with children (or When).",
                    existing_cat.name, parent_label
                );
            } else if existing_cat.children.is_empty()
                && !existing_cat.name.eq_ignore_ascii_case("When")
            {
                self.status = format!(
                    "Category '{}' is top-level but has no subcategories, so it cannot be a column heading yet.",
                    existing_cat.name
                );
            } else {
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
    ) -> Result<(), String> {
        let Some(add_state) = self.board_add_column_state().cloned() else {
            return Ok(());
        };
        let Some(mut view) = self.current_view().cloned() else {
            return Err("No active view".to_string());
        };
        let Some(section) = view.sections.get_mut(add_state.anchor.section_index) else {
            return Err("Current section not found".to_string());
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
            self.status = if heading_category.parent.is_some() {
                "Invalid column heading: choose a top-level category with subcategories (or When)"
                    .to_string()
            } else {
                format!(
                    "Invalid column heading '{}': add subcategories first",
                    heading_category.name
                )
            };
            return Ok(());
        }

        let kind = if heading_category.name.eq_ignore_ascii_case("When") {
            ColumnKind::When
        } else {
            ColumnKind::Standard
        };

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
        agenda
            .store()
            .update_view(&view)
            .map_err(|e| e.to_string())?;
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
    ) -> Result<(), String> {
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
            return Err("No active view".to_string());
        };
        let Some(section) = view.sections.get_mut(section_index) else {
            return Err("Current section not found".to_string());
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
        let selected_item_id = self.selected_item_id();
        agenda
            .store()
            .update_view(&view)
            .map_err(|e| e.to_string())?;
        self.refresh(agenda.store())?;
        self.set_view_selection_by_name(&view_name);
        if let Some(item_id) = selected_item_id {
            self.set_item_selection_by_id(item_id);
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
    ) -> Result<(), String> {
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

    fn move_current_board_column_to_edge(
        &mut self,
        rightmost: bool,
        agenda: &Agenda<'_>,
    ) -> Result<(), String> {
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
        self.move_current_board_column_to_index(if rightmost { max_board_index } else { 0 }, agenda)
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
            self.status = "Cannot delete Item column (move it with H/L or gH/gL)".to_string();
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

    fn remove_current_board_column(&mut self, agenda: &Agenda<'_>) -> Result<(), String> {
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
            return Err("No active view".to_string());
        };
        let Some(section) = view.sections.get_mut(section_index) else {
            return Err("Current section not found".to_string());
        };
        let item_board_index = Self::section_item_column_index(section);
        if self.column_index == item_board_index {
            self.status = "Cannot delete Item column (move it with H/L or gH/gL)".to_string();
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
        agenda
            .store()
            .update_view(&view)
            .map_err(|e| e.to_string())?;
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
    ) -> Result<bool, String> {
        match code {
            KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y') => {
                self.board_pending_delete_column_label = None;
                self.mode = Mode::Normal;
                self.remove_current_board_column(agenda)?;
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
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

    fn confirm_inline_create_board_add_column(
        &mut self,
        agenda: &Agenda<'_>,
    ) -> Result<(), String> {
        let Some(name) = self
            .board_add_column_create_confirm_name()
            .map(str::to_string)
        else {
            return Ok(());
        };
        let mut category = Category::new(name.clone());
        category.enable_implicit_string = true;
        let cat_id = category.id;
        agenda
            .create_category(&category)
            .map_err(|e| e.to_string())?;
        self.refresh_category_cache(agenda.store())?;
        self.set_board_add_column_create_confirm_name(None);
        self.insert_board_column_for_category(agenda, cat_id)?;
        if self.mode == Mode::Normal {
            self.status = format!("Created category '{}' and inserted column", name);
        }
        Ok(())
    }

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
        });
        self.set_input(input_value);
        self.category_suggest = None;
        self.category_direct_edit_create_confirm = None;
        self.status = "Set category: type to filter, Enter assign/create, Esc cancel".to_string();
        self.update_suggestions();
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

    fn assign_selected_suggestion(&mut self, _agenda: &Agenda<'_>) -> Result<(), String> {
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

    fn confirm_inline_create_category_direct_edit(
        &mut self,
        agenda: &Agenda<'_>,
    ) -> Result<(), String> {
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
        agenda
            .create_category(&category)
            .map_err(|e| e.to_string())?;
        self.refresh_category_cache(agenda.store())?;
        self.set_direct_edit_create_confirm_name(None);
        let _ = self.resolve_active_category_direct_edit_row(cat_id)?;
        self.status = format!("Created category '{}' and resolved current row", name);
        Ok(())
    }

    fn apply_category_direct_edit_draft(&mut self, agenda: &Agenda<'_>) -> Result<(), String> {
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
            agenda
                .unassign_item_manual(item_id, id)
                .map_err(|e| e.to_string())?;
        }
        for id in to_add {
            agenda
                .assign_item_manual(
                    item_id,
                    id,
                    Some("manual:tui.direct_edit.multi".to_string()),
                )
                .map_err(|e| e.to_string())?;
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
    ) -> Result<bool, String> {
        if let Some(prefix) = self.normal_mode_prefix.take() {
            match (prefix, code) {
                (NormalModePrefix::G, KeyCode::Char('a')) => {
                    self.jump_to_all_items_view(agenda)?;
                    self.status = "Jumped to All Items view".to_string();
                    return Ok(false);
                }
                (NormalModePrefix::G, KeyCode::Char('H')) => {
                    self.move_current_board_column_to_edge(false, agenda)?;
                    return Ok(false);
                }
                (NormalModePrefix::G, KeyCode::Char('L')) => {
                    self.move_current_board_column_to_edge(true, agenda)?;
                    return Ok(false);
                }
                (NormalModePrefix::G, KeyCode::Esc) => {
                    self.status = "Cancelled g-prefix command".to_string();
                    return Ok(false);
                }
                (NormalModePrefix::G, _) => {
                    self.status = "Unknown g command (use ga, gH, or gL)".to_string();
                    return Ok(false);
                }
            }
        }
        match code {
            KeyCode::Char('q') => return Ok(true),
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
                let max_cols = self.current_slot_column_count();
                if self.column_index < max_cols {
                    self.column_index += 1;
                } else {
                    self.move_slot_cursor(1);
                }
            }
            KeyCode::Left | KeyCode::Char('h') => {
                if self.column_index > 0 {
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
                    self.status = err;
                }
            }
            KeyCode::Char('-') => {
                self.open_remove_current_board_column_confirm();
            }
            KeyCode::Char('n') => {
                self.open_input_panel_add_item();
            }
            KeyCode::Char('e') => {
                self.open_input_panel_edit_item();
            }
            KeyCode::Enter => {
                if self.column_index != self.current_slot_item_column_index() {
                    self.open_category_direct_edit();
                } else {
                    self.open_input_panel_edit_item();
                }
            }
            KeyCode::Char('m') => {
                if let Some(item) = self.selected_item() {
                    let existing_note = item.note.clone().unwrap_or_default();
                    self.mode = Mode::NoteEdit;
                    self.set_input(existing_note);
                    self.status =
                        "Edit note: Enter to save (empty clears), Esc to cancel".to_string();
                } else {
                    self.status = "No selected item to add/edit note".to_string();
                }
            }
            KeyCode::Char('/') => {
                let target = self.slot_index;
                self.filter_target_section = target;
                self.mode = Mode::FilterInput;
                let existing = self
                    .section_filters
                    .get(target)
                    .and_then(|f| f.clone())
                    .unwrap_or_default();
                self.set_input(existing);
                self.status =
                    "Filter section: type query and Enter to apply, Esc to cancel".to_string();
            }
            KeyCode::Esc => {
                let target = self.slot_index;
                if target < self.section_filters.len()
                    && self.section_filters[target].take().is_some()
                {
                    self.refresh(agenda.store())?;
                    self.status = "Filter cleared".to_string();
                }
            }
            KeyCode::F(8) | KeyCode::Char('v') => {
                self.mode = Mode::ViewPicker;
                self.picker_index = self.view_index;
                self.status =
                    "View palette: Enter switch, n create, r rename, x delete, e edit view, Esc cancel"
                        .to_string();
            }
            KeyCode::F(9) | KeyCode::Char('c') => {
                self.mode = Mode::CategoryManager;
                self.category_config_editor = None;
                self.status =
                    "Category manager: Enter config popup, e/i/a quick toggles (exclusive/match-name/actionable), n/N create, r rename, p reparent, x delete".to_string();
            }
            KeyCode::Char(',') => {
                self.cycle_view(-1, agenda)?;
            }
            KeyCode::Char('.') => {
                self.cycle_view(1, agenda)?;
            }
            KeyCode::Tab => self.move_slot_cursor(1),
            KeyCode::BackTab => self.move_slot_cursor(-1),
            KeyCode::Char('f') => self.toggle_normal_focus(),
            KeyCode::Char('g') => {
                self.normal_mode_prefix = Some(NormalModePrefix::G);
                self.status =
                    "g-prefix: ga=All Items  gH=move column first  gL=move column last".to_string();
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
                    self.clear_input();
                    self.status =
                        "Item categories: j/k select, Space toggle, n type category, Enter done, Esc cancel"
                            .to_string();
                }
            }
            KeyCode::Char('u') => {
                if self.show_preview
                    && self.normal_focus == NormalFocus::Preview
                    && self.preview_mode == PreviewMode::Provenance
                {
                    self.open_provenance_unassign_picker();
                } else if self.selected_item_id().is_none() {
                    self.status = "No selected item to edit categories".to_string();
                } else if self.category_rows.is_empty() {
                    self.status = "No categories available".to_string();
                } else {
                    self.mode = Mode::ItemAssignPicker;
                    self.item_assign_category_index =
                        first_non_reserved_category_index(&self.category_rows);
                    self.clear_input();
                    self.status =
                        "Item categories: j/k select, Space toggle, n type category, Enter done, Esc cancel"
                            .to_string();
                }
            }
            KeyCode::Char('p') => self.toggle_preview(),
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
                        agenda
                            .remove_item_from_view(item_id, &view)
                            .map_err(|e| e.to_string())?;
                        self.refresh(agenda.store())?;
                        self.status = "Removed item from current view".to_string();
                    }
                }
            }
            KeyCode::Char('d') | KeyCode::Char('D') => {
                if let Some(item_id) = self.selected_item_id() {
                    let was_done = self
                        .selected_item()
                        .map(|item| item.is_done)
                        .unwrap_or(false);
                    if !was_done && !self.selected_item_has_actionable_assignment() {
                        self.status =
                            "Done unavailable: item has no actionable category assignments"
                                .to_string();
                        return Ok(false);
                    }
                    agenda
                        .toggle_item_done(item_id)
                        .map_err(|e| e.to_string())?;
                    self.refresh(agenda.store())?;
                    self.status = if was_done {
                        "Marked item not-done".to_string()
                    } else {
                        "Marked item done".to_string()
                    };
                }
            }
            KeyCode::Char('x') => {
                if self.selected_item_id().is_some() {
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

    pub(crate) fn handle_normal_key_event(
        &mut self,
        key: KeyEvent,
        agenda: &Agenda<'_>,
    ) -> Result<bool, String> {
        let ctrl_only = key.modifiers.contains(KeyModifiers::CONTROL)
            && !key
                .modifiers
                .intersects(KeyModifiers::ALT | KeyModifiers::SUPER);
        if ctrl_only {
            match key.code {
                KeyCode::Char('l') | KeyCode::Char('L') => {
                    self.status = "Use + to add a column and H/L/gH/gL to move it".to_string();
                    return Ok(false);
                }
                KeyCode::Char('r') | KeyCode::Char('R') => {
                    self.status = "Use + to add a column and H/L/gH/gL to move it".to_string();
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
        self.mode = Mode::InputPanel;
        self.status =
            "Add item: type text, S to save, Tab for note/categories, Esc to cancel".to_string();
    }

    pub(crate) fn handle_board_add_column_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> Result<bool, String> {
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
            KeyCode::Char('j') | KeyCode::Down => {
                self.move_board_add_column_suggest_cursor(1);
                return Ok(false);
            }
            KeyCode::Char('k') | KeyCode::Up => {
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

        if let Some(state) = self.board_add_column_state_mut() {
            if state.input.handle_key(code, false) {
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
    ) -> Result<bool, String> {
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
            KeyCode::Char('S') | KeyCode::Char('s') => {
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
            KeyCode::Char('+') => {
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
                    if let Some(row) = self.active_category_direct_edit_row_mut() {
                        row.input.handle_key(code, false);
                        row.category_id = None;
                    }
                    self.sync_category_direct_edit_input_mirror();
                    self.update_suggestions();
                }
            }
        }
        Ok(false)
    }

    pub(crate) fn handle_category_create_confirm_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> Result<bool, String> {
        match code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                if let Mode::CategoryCreateConfirm { name, parent_id } = self.mode.clone() {
                    let mut category = Category::new(name.clone());
                    category.parent = Some(parent_id);
                    category.enable_implicit_string = true;
                    let cat_id = category.id;

                    agenda
                        .create_category(&category)
                        .map_err(|e| e.to_string())?;

                    if let Some(item_id) = self.selected_item_id() {
                        agenda
                            .assign_item_manual(
                                item_id,
                                cat_id,
                                Some("manual:tui.direct_edit".to_string()),
                            )
                            .map_err(|e| e.to_string())?;
                    }

                    self.mode = Mode::Normal;
                    self.status = format!("Created and assigned '{}'", name);
                    self.refresh(agenda.store())?;
                }
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.status = "Cancelled creation".to_string();
            }
            _ => {}
        }
        Ok(false)
    }

    /// Open an InputPanel for editing the currently selected item.
    pub(crate) fn open_input_panel_edit_item(&mut self) {
        if let Some(item) = self.selected_item() {
            let text = item.text.clone();
            let note = item.note.clone().unwrap_or_default();
            // Collect manually-assigned category IDs for the draft.
            let categories: HashSet<agenda_core::model::CategoryId> = item
                .assignments
                .iter()
                .filter(|(_, a)| {
                    matches!(
                        a.source,
                        agenda_core::model::AssignmentSource::Manual
                            | agenda_core::model::AssignmentSource::Action
                    )
                })
                .map(|(id, _)| *id)
                .collect();
            let item_id = item.id;
            self.input_panel = Some(input_panel::InputPanel::new_edit_item(
                item_id, text, note, categories,
            ));
            self.mode = Mode::InputPanel;
            self.status = "Edit item: S to save, Tab cycles fields, Esc to cancel".to_string();
        } else {
            self.status = "No selected item to edit".to_string();
        }
    }

    /// Handle a key event while in Mode::InputPanel.
    pub(crate) fn handle_input_panel_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> Result<bool, String> {
        // If the category picker overlay is open, route keys to it.
        if self
            .input_panel
            .as_ref()
            .map_or(false, |p| p.category_picker_open())
        {
            self.handle_input_panel_picker_key(code);
            return Ok(false);
        }

        let Some(panel) = &mut self.input_panel else {
            self.mode = Mode::Normal;
            self.status = "InputPanel error: no panel state".to_string();
            return Ok(false);
        };

        let action = panel.handle_key(code);

        use input_panel::InputPanelAction;
        match action {
            InputPanelAction::Cancel => {
                let kind = self.input_panel.as_ref().map(|p| p.kind);
                self.input_panel = None;
                match kind {
                    Some(input_panel::InputPanelKind::NameInput) => {
                        self.mode = self.name_input_return_mode();
                        self.name_input_context = None;
                        self.status = "Canceled".to_string();
                    }
                    _ => {
                        self.mode = Mode::Normal;
                        self.status = "Canceled".to_string();
                    }
                }
            }
            InputPanelAction::Save => {
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
                }
            }
            InputPanelAction::OpenCategoryPicker => {
                if self.category_rows.is_empty() {
                    self.status = "No categories available".to_string();
                } else {
                    let initial = first_non_reserved_category_index(&self.category_rows);
                    if let Some(panel) = &mut self.input_panel {
                        panel.open_category_picker(initial);
                    }
                    self.status =
                        "Categories: j/k navigate, Space toggle, Enter/Esc close".to_string();
                }
            }
            InputPanelAction::FocusNext
            | InputPanelAction::FocusPrev
            | InputPanelAction::Handled
            | InputPanelAction::Unhandled => {}
        }
        Ok(false)
    }

    /// Handle key events while the InputPanel's category picker overlay is open.
    fn handle_input_panel_picker_key(&mut self, code: KeyCode) {
        let list_len = self.category_rows.len();
        match code {
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(panel) = &mut self.input_panel {
                    panel.move_picker_cursor(list_len, 1);
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(panel) = &mut self.input_panel {
                    panel.move_picker_cursor(list_len, -1);
                }
            }
            KeyCode::Char(' ') => {
                // Read index and row first, then mutate panel.
                let idx = self
                    .input_panel
                    .as_ref()
                    .and_then(|p| p.picker_index())
                    .unwrap_or(0);
                let row = self.category_rows.get(idx).cloned();
                if let Some(row) = row {
                    if !row.is_reserved {
                        // Determine if this is an add (not currently selected).
                        let is_adding = self
                            .input_panel
                            .as_ref()
                            .map(|p| !p.categories.contains(&row.id))
                            .unwrap_or(false);
                        // If adding into an exclusive parent group, clear siblings first.
                        if is_adding {
                            let to_clear = exclusive_siblings_to_clear(&self.category_rows, idx);
                            if let Some(panel) = &mut self.input_panel {
                                for sibling_id in to_clear {
                                    panel.categories.remove(&sibling_id);
                                }
                            }
                        }
                        if let Some(panel) = &mut self.input_panel {
                            panel.toggle_category(row.id);
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
            KeyCode::Enter | KeyCode::Esc => {
                if let Some(panel) = &mut self.input_panel {
                    panel.close_category_picker();
                }
                let count = self
                    .input_panel
                    .as_ref()
                    .map(|p| p.categories.len())
                    .unwrap_or(0);
                self.status = format!("{} categories selected", count);
            }
            _ => {}
        }
    }

    /// Save an InputPanel(AddItem) to the store.
    fn save_input_panel_add(&mut self, agenda: &Agenda<'_>) -> Result<(), String> {
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
        let reference_date = Local::now().date_naive();
        agenda
            .create_item_with_reference_date(&item, reference_date)
            .map_err(|e| e.to_string())?;

        // Set note if provided.
        if note.is_some() {
            let mut loaded = agenda
                .store()
                .get_item(item.id)
                .map_err(|e| e.to_string())?;
            loaded.note = note;
            loaded.modified_at = Utc::now();
            agenda
                .update_item_with_reference_date(&loaded, reference_date)
                .map_err(|e| e.to_string())?;
        }

        // Assign explicitly selected categories.
        for cat_id in &categories_to_assign {
            let _ = agenda.assign_item_manual(
                item.id,
                *cat_id,
                Some("manual:input_panel.add".to_string()),
            );
        }

        // Insert into section context (applies on_insert_assign rules).
        if let Some(view) = self.current_view().cloned() {
            if let Some(context) = self.current_slot().map(|slot| slot.context.clone()) {
                self.insert_into_context(agenda, item.id, &view, &context)?;
            }
        }

        let category_names: Vec<String> = agenda
            .store()
            .get_hierarchy()
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|c| c.name)
            .collect();
        let unknown_hashtags = unknown_hashtag_tokens(&text, &category_names);
        let created = agenda
            .store()
            .get_item(item.id)
            .map_err(|e| e.to_string())?;

        self.refresh(agenda.store())?;
        self.set_item_selection_by_id(item.id);
        self.input_panel = None;
        self.mode = Mode::Normal;
        self.status = add_capture_status_message(created.when_date, &unknown_hashtags);
        Ok(())
    }

    /// Save an InputPanel(EditItem) to the store (text, note, and category diff).
    fn save_input_panel_edit(&mut self, agenda: &Agenda<'_>) -> Result<(), String> {
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

        let mut item = agenda
            .store()
            .get_item(item_id)
            .map_err(|e| e.to_string())?;

        // Compute category diff: which to add, which to remove.
        let existing_categories: HashSet<_> = item
            .assignments
            .iter()
            .filter(|(_, a)| {
                matches!(
                    a.source,
                    agenda_core::model::AssignmentSource::Manual
                        | agenda_core::model::AssignmentSource::Action
                )
            })
            .map(|(id, _)| *id)
            .collect();

        let no_text_change = item.text == updated_text;
        let no_note_change = item.note == updated_note;
        let no_cat_change = existing_categories == new_categories;

        if no_text_change && no_note_change && no_cat_change {
            self.input_panel = None;
            self.mode = Mode::Normal;
            self.status = "Edit canceled: no changes".to_string();
            return Ok(());
        }

        // Update text and note.
        item.text = updated_text;
        item.note = updated_note;
        item.modified_at = Utc::now();
        let reference_date = Local::now().date_naive();
        agenda
            .update_item_with_reference_date(&item, reference_date)
            .map_err(|e| e.to_string())?;

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

        self.refresh(agenda.store())?;
        self.set_item_selection_by_id(item_id);
        self.input_panel = None;
        self.mode = Mode::Normal;
        self.status = "Item updated".to_string();
        Ok(())
    }

    /// Returns the mode to return to when a NameInput panel is canceled or completed.
    fn name_input_return_mode(&self) -> Mode {
        match self.name_input_context {
            Some(NameInputContext::ViewCreate) | Some(NameInputContext::ViewRename) => {
                Mode::ViewPicker
            }
            Some(NameInputContext::CategoryCreate) | Some(NameInputContext::CategoryRename) => {
                Mode::CategoryManager
            }
            None => Mode::Normal,
        }
    }

    /// Save an InputPanel(NameInput) — dispatches on name_input_context.
    fn save_input_panel_name(&mut self, agenda: &Agenda<'_>) -> Result<(), String> {
        let name = self
            .input_panel
            .as_ref()
            .map(|p| p.text.trimmed().to_string())
            .unwrap_or_default();

        if name.is_empty() {
            self.status = "Name cannot be empty".to_string();
            return Ok(());
        }

        match self.name_input_context {
            Some(NameInputContext::ViewCreate) => {
                self.view_pending_name = Some(name.clone());
                self.view_category_index = first_non_reserved_category_index(&self.category_rows);
                self.view_create_include_selection.clear();
                self.view_create_exclude_selection.clear();
                self.input_panel = None;
                self.name_input_context = None;
                self.mode = Mode::ViewCreateCategory;
                self.status = format!("Create view {name}: + include, - exclude, Enter creates");
            }
            Some(NameInputContext::ViewRename) => {
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
            Some(NameInputContext::CategoryCreate) => {
                let mut category = Category::new(name.clone());
                category.enable_implicit_string = true;
                category.parent = self.category_create_parent;
                let parent_label = self
                    .create_parent_name()
                    .unwrap_or_else(|| "top level".to_string());
                match agenda.create_category(&category).map_err(|e| e.to_string()) {
                    Ok(result) => {
                        self.refresh(agenda.store())?;
                        self.set_category_selection_by_id(category.id);
                        self.input_panel = None;
                        self.name_input_context = None;
                        self.category_create_parent = None;
                        self.mode = Mode::CategoryManager;
                        self.status = format!(
                            "Created category {name} under {parent_label} (processed_items={}, affected_items={})",
                            result.processed_items, result.affected_items
                        );
                    }
                    Err(err) => {
                        self.input_panel = None;
                        self.name_input_context = None;
                        self.category_create_parent = None;
                        self.mode = Mode::CategoryManager;
                        self.status = format!("Create failed: {err}");
                    }
                }
            }
            Some(NameInputContext::CategoryRename) => {
                let Some(category_id) = self.selected_category_id() else {
                    self.input_panel = None;
                    self.name_input_context = None;
                    self.mode = Mode::CategoryManager;
                    self.status = "Category rename failed: no selection".to_string();
                    return Ok(());
                };
                let mut category = agenda
                    .store()
                    .get_category(category_id)
                    .map_err(|e| e.to_string())?;
                if category.name == name {
                    self.input_panel = None;
                    self.name_input_context = None;
                    self.mode = Mode::CategoryManager;
                    self.status = "Category rename canceled (unchanged)".to_string();
                    return Ok(());
                }
                category.name = name.clone();
                let result = agenda
                    .update_category(&category)
                    .map_err(|e| e.to_string())?;
                self.refresh(agenda.store())?;
                self.set_category_selection_by_id(category_id);
                self.input_panel = None;
                self.name_input_context = None;
                self.mode = Mode::CategoryManager;
                self.status = format!(
                    "Renamed category to {name} (processed_items={}, affected_items={})",
                    result.processed_items, result.affected_items
                );
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

    pub(crate) fn handle_note_edit_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> Result<bool, String> {
        match code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.clear_input();
                self.status = "Note edit canceled".to_string();
            }
            KeyCode::Char('S') | KeyCode::Enter => {
                let Some(item_id) = self.selected_item_id() else {
                    self.mode = Mode::Normal;
                    self.clear_input();
                    self.status = "Note edit failed: no selected item".to_string();
                    return Ok(false);
                };

                let new_note = if self.input.trimmed().is_empty() {
                    None
                } else {
                    Some(self.input.text().to_string())
                };

                let mut item = agenda
                    .store()
                    .get_item(item_id)
                    .map_err(|e| e.to_string())?;
                if item.note == new_note {
                    self.mode = Mode::Normal;
                    self.clear_input();
                    self.status = "Note edit canceled: no note change".to_string();
                    return Ok(false);
                }

                item.note = new_note;
                item.modified_at = Utc::now();
                let reference_date = Local::now().date_naive();
                agenda
                    .update_item_with_reference_date(&item, reference_date)
                    .map_err(|e| e.to_string())?;
                self.refresh(agenda.store())?;
                self.set_item_selection_by_id(item_id);
                self.mode = Mode::Normal;
                self.clear_input();
                self.status = if item.note.is_some() {
                    "Note updated".to_string()
                } else {
                    "Note cleared".to_string()
                };
            }
            _ if self.handle_text_input_key(code) => {}
            _ => {}
        }
        Ok(false)
    }

    pub(crate) fn handle_item_assign_category_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> Result<bool, String> {
        match code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.clear_input();
                self.status = "Assign canceled".to_string();
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
                self.status = "Type category name: Enter assign/create, Esc back".to_string();
            }
            KeyCode::Char(' ') => {
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
                    let was_done = self
                        .selected_item()
                        .map(|item| item.is_done)
                        .unwrap_or(false);
                    if !was_done && !self.selected_item_has_actionable_assignment() {
                        self.status =
                            "Done unavailable: item has no actionable category assignments"
                                .to_string();
                        return Ok(false);
                    }
                    match agenda.toggle_item_done(item_id) {
                        Ok(_) => {
                            self.refresh(agenda.store())?;
                            self.set_item_selection_by_id(item_id);
                            self.status = if was_done {
                                "Removed category Done (marked not-done)".to_string()
                            } else {
                                "Assigned item to category Done (marked done)".to_string()
                            };
                        }
                        Err(err) => {
                            self.status = format!("Done toggle failed: {}", err);
                        }
                    }
                    return Ok(false);
                }

                if self.selected_item_has_assignment(row.id) {
                    match agenda.unassign_item_manual(item_id, row.id) {
                        Ok(()) => {
                            self.refresh(agenda.store())?;
                            self.set_item_selection_by_id(item_id);
                            self.status = format!("Removed category {}", row.name);
                        }
                        Err(err) => {
                            self.status = format!("Cannot remove {}: {}", row.name, err);
                        }
                    }
                } else {
                    let result = agenda
                        .assign_item_manual(item_id, row.id, Some("manual:tui.assign".to_string()))
                        .map_err(|e| e.to_string())?;
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
                self.mode = Mode::Normal;
                self.clear_input();
                self.status = "Category edit saved".to_string();
            }
            _ => {}
        }

        Ok(false)
    }

    pub(crate) fn handle_item_assign_category_input_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> Result<bool, String> {
        match code {
            KeyCode::Esc => {
                self.mode = Mode::ItemAssignPicker;
                self.clear_input();
                self.status = "Category name entry canceled".to_string();
            }
            KeyCode::Enter => {
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

                let category_id = if let Some(existing) = self
                    .categories
                    .iter()
                    .find(|category| category.name.eq_ignore_ascii_case(&name))
                {
                    existing.id
                } else {
                    let mut category = Category::new(name.clone());
                    category.enable_implicit_string = true;
                    agenda
                        .store()
                        .create_category(&category)
                        .map_err(|e| e.to_string())?;
                    category.id
                };

                let result = agenda
                    .assign_item_manual(item_id, category_id, Some("manual:tui.assign".to_string()))
                    .map_err(|e| e.to_string())?;
                self.refresh(agenda.store())?;
                self.set_item_selection_by_id(item_id);
                if let Some(index) = self
                    .category_rows
                    .iter()
                    .position(|row| row.id == category_id)
                {
                    self.item_assign_category_index = index;
                }
                self.mode = Mode::ItemAssignPicker;
                self.clear_input();
                self.status = format!(
                    "Assigned category {} (new_assignments={})",
                    name,
                    result.new_assignments.len()
                );
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
    ) -> Result<bool, String> {
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

                agenda
                    .unassign_item_manual(item_id, row.category_id)
                    .map_err(|e| e.to_string())?;
                self.refresh(agenda.store())?;
                self.set_item_selection_by_id(item_id);
                self.mode = Mode::Normal;
                self.status = format!("Unassigned {}", row.category_name);
            }
            _ => {}
        }

        Ok(false)
    }

    pub(crate) fn handle_filter_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> Result<bool, String> {
        match code {
            KeyCode::Esc => {
                // Cancel — preserve existing filter for this section
                self.mode = Mode::Normal;
                self.clear_input();
                self.status = "Filter cancelled".to_string();
            }
            KeyCode::Enter => {
                self.mode = Mode::Normal;
                let value = self.input.trimmed().to_string();
                let target = self.filter_target_section;
                if target < self.section_filters.len() {
                    self.section_filters[target] =
                        if value.is_empty() { None } else { Some(value) };
                }
                self.clear_input();
                self.refresh(agenda.store())?;
                self.status = if self
                    .section_filters
                    .get(target)
                    .is_some_and(|f| f.is_some())
                {
                    "Filter applied".to_string()
                } else {
                    "Filter cleared".to_string()
                };
            }
            _ if self.handle_text_input_key(code) => {}
            _ => {}
        }
        Ok(false)
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
    for i in (parent_idx + 1)..rows.len() {
        if rows[i].depth <= parent_depth {
            break; // Exited the parent's subtree
        }
        if rows[i].depth == target_depth && i != row_idx {
            siblings.push(rows[i].id);
        }
    }
    siblings
}

#[cfg(test)]
mod tests {
    use super::*;
    use agenda_core::model::CategoryId;

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
}
