use crate::*;
use agenda_core::model::AssignmentSource;

enum CategoryInlineConfirmKeyAction {
    Confirm,
    Cancel,
    None,
}

struct WorkflowRolePrepResult {
    auto_match_disabled: bool,
    warn_other_derived_sources: bool,
}

fn category_inline_confirm_key_action(code: KeyCode) -> CategoryInlineConfirmKeyAction {
    match code {
        KeyCode::Char('y') => CategoryInlineConfirmKeyAction::Confirm,
        KeyCode::Esc => CategoryInlineConfirmKeyAction::Cancel,
        _ => CategoryInlineConfirmKeyAction::None,
    }
}

fn parse_also_match_entries(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

impl App {
    fn prepare_category_for_workflow_role(
        &mut self,
        category_id: CategoryId,
        agenda: &Agenda<'_>,
    ) -> TuiResult<WorkflowRolePrepResult> {
        let mut category = agenda.store().get_category(category_id)?;
        let warn_other_derived_sources =
            !category.conditions.is_empty() || !category.actions.is_empty();
        let auto_match_disabled = category.enable_implicit_string;
        if auto_match_disabled {
            category.enable_implicit_string = false;
            agenda.update_category(&category)?;
            let implicit_origin = format!("cat:{}", category.name);
            let implicit_assigned_item_ids: Vec<_> = agenda
                .store()
                .list_items()?
                .into_iter()
                .filter_map(|item| {
                    item.assignments.get(&category_id).and_then(|assignment| {
                        ((assignment.source == AssignmentSource::AutoMatch
                            || assignment.source == AssignmentSource::AutoClassified)
                            && assignment.origin.as_deref() == Some(implicit_origin.as_str()))
                        .then_some(item.id)
                    })
                })
                .collect();
            if !implicit_assigned_item_ids.is_empty() {
                for item_id in implicit_assigned_item_ids {
                    agenda.store().unassign_item(item_id, category_id)?;
                }
                let refreshed_category = agenda.store().get_category(category_id)?;
                agenda.update_category(&refreshed_category)?;
            }
        }
        Ok(WorkflowRolePrepResult {
            auto_match_disabled,
            warn_other_derived_sources,
        })
    }

    fn workflow_role_status_message(
        role_label: &str,
        category_name: &str,
        previous_name: Option<&str>,
        prep: Option<&WorkflowRolePrepResult>,
    ) -> String {
        let mut message = if let Some(previous_name) = previous_name {
            format!("{category_name} is now the {role_label} category (replaced {previous_name})")
        } else {
            format!("{category_name} is now the {role_label} category")
        };
        if let Some(prep) = prep {
            if prep.auto_match_disabled {
                message.push_str("; Auto-match disabled for workflow role");
            }
            if prep.warn_other_derived_sources {
                message.push_str("; warning: profile rules/actions can still assign it");
            }
        }
        message
    }

    fn workflow_setup_cross_role_conflict_status(
        &self,
        agenda: &Agenda<'_>,
        role_index: usize,
        selected_category_id: CategoryId,
    ) -> TuiResult<Option<String>> {
        let (role_label, current_role_id, other_role_label, other_role_id) = if role_index == 0 {
            (
                "Ready Queue",
                self.workflow_config.ready_category_id,
                "Claim Result",
                self.workflow_config.claim_category_id,
            )
        } else {
            (
                "Claim Result",
                self.workflow_config.claim_category_id,
                "Ready Queue",
                self.workflow_config.ready_category_id,
            )
        };

        if other_role_id != Some(selected_category_id)
            || current_role_id == Some(selected_category_id)
        {
            return Ok(None);
        }

        let selected_name = agenda.store().get_category(selected_category_id)?.name;
        let current_name = current_role_id
            .and_then(|category_id| agenda.store().get_category(category_id).ok())
            .map(|category| category.name)
            .unwrap_or_else(|| "(unset)".to_string());

        Ok(Some(format!(
            "{selected_name} is already the {other_role_label} category. Select {current_name} to unset {role_label}, or another category to replace it"
        )))
    }

    fn category_manager_save_key_pressed(&self, code: KeyCode) -> bool {
        matches!(code, KeyCode::Char('S'))
            || (matches!(code, KeyCode::Char('s'))
                && self.current_key_modifiers.contains(KeyModifiers::SHIFT))
    }

    fn close_category_manager_with_status(&mut self, status: &str) {
        self.mode = Mode::Normal;
        self.close_category_manager_session();
        self.workflow_setup_open = false;
        self.workflow_role_picker = None;
        self.clear_input();
        self.status = status.to_string();
    }

    fn handle_category_manager_discard_confirm_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> TuiResult<bool> {
        match code {
            KeyCode::Char('y') => {
                self.save_category_manager_dirty_details(agenda)?;
                self.close_category_manager_with_status("Category manager closed (saved)");
            }
            KeyCode::Char('n') => {
                self.close_category_manager_with_status(
                    "Category manager closed; unsaved detail changes discarded",
                );
            }
            KeyCode::Esc => {
                self.set_category_manager_discard_confirm(false);
                self.status =
                    "Kept category manager open; unsaved detail drafts retained".to_string();
            }
            _ => {}
        }
        Ok(true)
    }

    pub(crate) fn category_manager_parent_label(&self, parent_id: Option<CategoryId>) -> String {
        parent_id
            .and_then(|id| {
                self.category_rows
                    .iter()
                    .find(|row| row.id == id)
                    .map(|row| row.name.clone())
            })
            .unwrap_or_else(|| "top level".to_string())
    }

    pub(crate) fn category_name_exists_elsewhere(
        &self,
        candidate: &str,
        excluding_id: Option<CategoryId>,
    ) -> bool {
        self.categories.iter().any(|category| {
            Some(category.id) != excluding_id && category.name.eq_ignore_ascii_case(candidate)
        })
    }

    fn selected_category_parent_id(&self) -> Option<CategoryId> {
        let selected_id = self.selected_category_id()?;
        self.categories
            .iter()
            .find(|category| category.id == selected_id)
            .and_then(|category| category.parent)
    }

    fn open_category_create_panel(&mut self, parent_id: Option<CategoryId>, status: String) {
        let parent_label = self.category_manager_parent_label(parent_id);
        self.input_panel = Some(input_panel::InputPanel::new_category_create(
            parent_id,
            &parent_label,
        ));
        self.input_panel_discard_confirm = false;
        // CategoryCreate uses InputPanel; clear any stale inline action first.
        self.set_category_manager_inline_action(None);
        self.name_input_context = Some(NameInputContext::CategoryCreate);
        self.mode = Mode::InputPanel;
        self.status = status;
    }

    fn start_category_inline_rename(&mut self) {
        let Some((row_id, row_name, is_reserved)) = self
            .selected_category_row()
            .map(|row| (row.id, row.name.clone(), row.is_reserved))
        else {
            self.status = "No selected category".to_string();
            return;
        };
        if is_reserved {
            self.status = format!("Category {} is reserved and cannot be renamed", row_name);
            return;
        }
        self.set_category_manager_inline_action(Some(CategoryInlineAction::Rename {
            category_id: row_id,
            original_name: row_name.clone(),
            buf: text_buffer::TextBuffer::new(row_name.clone()),
        }));
        self.status = format!("Rename {}: edit name, Enter apply, Esc cancel", row_name);
    }

    fn start_category_inline_delete_confirm(&mut self) {
        let Some((row_id, row_name)) = self
            .selected_category_row()
            .map(|row| (row.id, row.name.clone()))
        else {
            self.status = "No selected category".to_string();
            return;
        };
        self.set_category_manager_inline_action(Some(CategoryInlineAction::DeleteConfirm {
            category_id: row_id,
            category_name: row_name.clone(),
        }));
        self.status = format!("Delete category \"{}\"? y/n", row_name);
    }

    fn apply_category_inline_rename(
        &mut self,
        agenda: &Agenda<'_>,
        category_id: CategoryId,
        original_name: String,
        name: String,
    ) -> TuiResult<()> {
        if name == original_name {
            self.set_category_manager_inline_action(None);
            self.status = "Category rename canceled (unchanged)".to_string();
            return Ok(());
        }
        let mut category = agenda.store().get_category(category_id)?;
        if is_reserved_category_name(&category.name) {
            self.set_category_manager_inline_action(None);
            self.status = format!(
                "Category {} is reserved and cannot be renamed",
                category.name
            );
            return Ok(());
        }
        category.name = name.clone();
        match agenda.update_category(&category) {
            Ok(result) => {
                self.refresh(agenda.store())?;
                self.set_category_selection_by_id(category_id);
                self.set_category_manager_inline_action(None);
                self.status = format!(
                    "Renamed category to {name} (processed_items={}, affected_items={})",
                    result.processed_items, result.affected_items
                );
            }
            Err(err) => {
                self.status = format!("Rename failed: {err}");
            }
        }
        Ok(())
    }

    fn apply_category_inline_delete_confirm(
        &mut self,
        agenda: &Agenda<'_>,
        category_id: CategoryId,
        category_name: String,
    ) -> TuiResult<()> {
        let old_visible_index = self.category_manager_visible_tree_index().unwrap_or(0);
        match agenda.store().delete_category(category_id) {
            Ok(()) => {
                self.refresh(agenda.store())?;
                if let Some(visible) = self.category_manager_visible_row_indices() {
                    if !visible.is_empty() {
                        let next = old_visible_index.min(visible.len().saturating_sub(1));
                        self.set_category_manager_visible_selection(next);
                    }
                }
                self.status = format!("Deleted category {}", category_name);
            }
            Err(err) => {
                self.status = format!("Delete failed: {err}");
            }
        }
        self.set_category_manager_inline_action(None);
        Ok(())
    }

    fn category_manager_has_active_filter(&self) -> bool {
        self.category_manager_filter_text()
            .map(|t| !t.trim().is_empty())
            .unwrap_or(false)
    }

    fn block_direct_structure_move_while_filtered(&mut self) -> bool {
        if self.category_manager_has_active_filter() {
            self.status =
                "Clear category filter before direct H/L/J/K moves or << / >> shifts".to_string();
            true
        } else {
            false
        }
    }

    fn recompute_category_manager_details_note_dirty(&mut self) {
        let selected_id = self.selected_category_id();
        let saved_note = selected_id
            .and_then(|id| self.categories.iter().find(|c| c.id == id))
            .and_then(|c| c.note.clone())
            .unwrap_or_default();
        let current_note = self
            .category_manager_details_note_text()
            .unwrap_or_default()
            .to_string();
        self.mark_category_manager_details_note_dirty(current_note != saved_note);
    }

    fn recompute_category_manager_details_also_match_dirty(&mut self) {
        let selected_id = self.selected_category_id();
        let saved_also_match = selected_id
            .and_then(|id| self.categories.iter().find(|c| c.id == id))
            .map(|c| c.also_match.clone())
            .unwrap_or_default();
        let current_also_match = parse_also_match_entries(
            self.category_manager_details_also_match_text()
                .unwrap_or_default(),
        );
        self.mark_category_manager_details_also_match_dirty(current_also_match != saved_also_match);
    }

    fn start_category_manager_details_note_edit(&mut self) {
        if self.selected_category_is_reserved() {
            self.status = "Reserved category config is read-only".to_string();
            return;
        }
        self.set_category_manager_focus(CategoryManagerFocus::Details);
        self.set_category_manager_details_focus(CategoryManagerDetailsFocus::Note);
        self.set_category_manager_details_note_editing(true);
        self.status = "Edit category note: type text, Esc:discard, Tab:leave".to_string();
    }

    fn start_category_manager_details_also_match_edit(&mut self) {
        if self.selected_category_is_reserved() {
            self.status = "Reserved category config is read-only".to_string();
            return;
        }
        self.set_category_manager_focus(CategoryManagerFocus::Details);
        self.set_category_manager_details_focus(CategoryManagerDetailsFocus::AlsoMatch);
        self.set_category_manager_details_also_match_editing(true);
        self.status =
            "Edit also-match terms: one entry per line, Esc:discard, Tab:leave".to_string();
    }

    fn save_category_manager_details_note(&mut self, agenda: &Agenda<'_>) -> TuiResult<()> {
        if self.selected_category_is_reserved() {
            self.status = "Reserved category config is read-only".to_string();
            self.set_category_manager_details_note_editing(false);
            self.reload_category_manager_details_note_from_selected();
            return Ok(());
        }
        let Some(category_id) = self.selected_category_id() else {
            self.status = "No selected category".to_string();
            return Ok(());
        };
        let mut category = agenda.store().get_category(category_id)?;
        let next_note = self
            .category_manager_details_note_text()
            .map(|t| t.to_string())
            .unwrap_or_default();
        let next_note = if next_note.trim().is_empty() {
            None
        } else {
            Some(next_note)
        };
        if category.note == next_note {
            self.mark_category_manager_details_note_dirty(false);
            self.set_category_manager_details_note_editing(false);
            self.status = "Category note unchanged".to_string();
            return Ok(());
        }

        category.note = next_note;
        let saved_name = category.name.clone();
        let result = agenda.update_category(&category)?;
        self.refresh(agenda.store())?;
        self.set_category_selection_by_id(category_id);
        self.reload_category_manager_details_note_from_selected();
        self.set_category_manager_focus(CategoryManagerFocus::Details);
        self.set_category_manager_details_focus(CategoryManagerDetailsFocus::Note);
        self.status = format!(
            "Saved note for {} (processed_items={}, affected_items={})",
            saved_name, result.processed_items, result.affected_items
        );
        Ok(())
    }

    fn save_category_manager_details_also_match(&mut self, agenda: &Agenda<'_>) -> TuiResult<()> {
        if self.selected_category_is_reserved() {
            self.status = "Reserved category config is read-only".to_string();
            self.set_category_manager_details_also_match_editing(false);
            self.reload_category_manager_details_also_match_from_selected();
            return Ok(());
        }
        let Some(category_id) = self.selected_category_id() else {
            self.status = "No selected category".to_string();
            return Ok(());
        };
        let mut category = agenda.store().get_category(category_id)?;
        let next_also_match = parse_also_match_entries(
            self.category_manager_details_also_match_text()
                .unwrap_or_default(),
        );
        if category.also_match == next_also_match {
            self.mark_category_manager_details_also_match_dirty(false);
            self.set_category_manager_details_also_match_editing(false);
            self.status = "Also-match terms unchanged".to_string();
            return Ok(());
        }

        category.also_match = next_also_match;
        let saved_name = category.name.clone();
        let result = agenda.update_category(&category)?;
        self.refresh(agenda.store())?;
        self.set_category_selection_by_id(category_id);
        self.reload_category_manager_details_also_match_from_selected();
        self.set_category_manager_focus(CategoryManagerFocus::Details);
        self.set_category_manager_details_focus(CategoryManagerDetailsFocus::AlsoMatch);
        self.status = format!(
            "Saved also-match terms for {} (processed_items={}, affected_items={})",
            saved_name, result.processed_items, result.affected_items
        );
        Ok(())
    }

    fn save_category_manager_dirty_details(&mut self, agenda: &Agenda<'_>) -> TuiResult<()> {
        if self.category_manager_details_note_dirty()
            && !self.category_manager_details_note_editing()
        {
            self.save_category_manager_details_note(agenda)?;
        }
        if self.category_manager_details_also_match_dirty()
            && !self.category_manager_details_also_match_editing()
        {
            self.save_category_manager_details_also_match(agenda)?;
        }
        Ok(())
    }

    fn category_manager_details_context(&self) -> (bool, bool) {
        let is_numeric = self
            .selected_category_row()
            .map(|row| row.value_kind == CategoryValueKind::Numeric)
            .unwrap_or(false);
        let integer_mode = self
            .selected_category_id()
            .and_then(|id| self.categories.iter().find(|c| c.id == id))
            .map(|c| c.numeric_format.clone().unwrap_or_default().decimal_places == 0)
            .unwrap_or(false);
        (is_numeric, integer_mode)
    }

    fn cycle_category_manager_details_section(&mut self, delta: i32) {
        let Some(details_focus) = self.category_manager_details_focus() else {
            return;
        };
        let (is_numeric, _integer_mode) = self.category_manager_details_context();
        let target = match delta.signum() {
            d if d > 0 => {
                if is_numeric {
                    match details_focus {
                        CategoryManagerDetailsFocus::Integer
                        | CategoryManagerDetailsFocus::DecimalPlaces
                        | CategoryManagerDetailsFocus::CurrencySymbol
                        | CategoryManagerDetailsFocus::ThousandsSeparator => {
                            CategoryManagerDetailsFocus::Note
                        }
                        CategoryManagerDetailsFocus::Note => {
                            self.set_category_manager_focus(CategoryManagerFocus::Filter);
                            return;
                        }
                        _ => CategoryManagerDetailsFocus::Integer,
                    }
                } else {
                    match details_focus {
                        CategoryManagerDetailsFocus::Exclusive
                        | CategoryManagerDetailsFocus::AutoMatch
                        | CategoryManagerDetailsFocus::SemanticMatch
                        | CategoryManagerDetailsFocus::MatchCategoryName
                        | CategoryManagerDetailsFocus::Actionable => {
                            CategoryManagerDetailsFocus::AlsoMatch
                        }
                        CategoryManagerDetailsFocus::AlsoMatch => CategoryManagerDetailsFocus::Note,
                        CategoryManagerDetailsFocus::Note => {
                            self.set_category_manager_focus(CategoryManagerFocus::Filter);
                            return;
                        }
                        _ => CategoryManagerDetailsFocus::Exclusive,
                    }
                }
            }
            d if d < 0 => {
                if is_numeric {
                    match details_focus {
                        CategoryManagerDetailsFocus::Note => {
                            CategoryManagerDetailsFocus::ThousandsSeparator
                        }
                        CategoryManagerDetailsFocus::Integer
                        | CategoryManagerDetailsFocus::DecimalPlaces
                        | CategoryManagerDetailsFocus::CurrencySymbol
                        | CategoryManagerDetailsFocus::ThousandsSeparator => {
                            self.set_category_manager_focus(CategoryManagerFocus::Tree);
                            return;
                        }
                        _ => {
                            self.set_category_manager_focus(CategoryManagerFocus::Tree);
                            return;
                        }
                    }
                } else {
                    match details_focus {
                        CategoryManagerDetailsFocus::Note => CategoryManagerDetailsFocus::AlsoMatch,
                        CategoryManagerDetailsFocus::AlsoMatch => {
                            CategoryManagerDetailsFocus::Actionable
                        }
                        CategoryManagerDetailsFocus::Exclusive
                        | CategoryManagerDetailsFocus::AutoMatch
                        | CategoryManagerDetailsFocus::SemanticMatch
                        | CategoryManagerDetailsFocus::MatchCategoryName
                        | CategoryManagerDetailsFocus::Actionable => {
                            self.set_category_manager_focus(CategoryManagerFocus::Tree);
                            return;
                        }
                        _ => {
                            self.set_category_manager_focus(CategoryManagerFocus::Tree);
                            return;
                        }
                    }
                }
            }
            _ => details_focus,
        };
        self.set_category_manager_focus(CategoryManagerFocus::Details);
        self.set_category_manager_details_focus(target);
    }

    fn handle_category_manager_tab_navigation(&mut self, reverse: bool) {
        match self.category_manager_focus() {
            Some(CategoryManagerFocus::Filter) => {
                self.set_category_manager_filter_editing(false);
                if reverse {
                    self.set_category_manager_focus(CategoryManagerFocus::Details);
                } else {
                    self.set_category_manager_focus(CategoryManagerFocus::Tree);
                }
            }
            Some(CategoryManagerFocus::Tree) => {
                self.set_category_manager_focus(if reverse {
                    CategoryManagerFocus::Filter
                } else {
                    CategoryManagerFocus::Details
                });
            }
            Some(CategoryManagerFocus::Details) => {
                self.cycle_category_manager_details_section(if reverse { -1 } else { 1 });
            }
            None => {}
        }
    }

    fn handle_category_manager_details_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> TuiResult<bool> {
        if self.category_manager_focus() != Some(CategoryManagerFocus::Details) {
            return Ok(false);
        }
        let Some(mut details_focus) = self.category_manager_details_focus() else {
            return Ok(false);
        };

        // Snap focus to Note when viewing a numeric category (flags don't apply)
        let is_numeric = self
            .selected_category_row()
            .map(|row| row.value_kind == CategoryValueKind::Numeric)
            .unwrap_or(false);
        if is_numeric
            && matches!(
                details_focus,
                CategoryManagerDetailsFocus::Exclusive
                    | CategoryManagerDetailsFocus::AutoMatch
                    | CategoryManagerDetailsFocus::SemanticMatch
                    | CategoryManagerDetailsFocus::MatchCategoryName
                    | CategoryManagerDetailsFocus::Actionable
                    | CategoryManagerDetailsFocus::AlsoMatch
            )
        {
            details_focus = CategoryManagerDetailsFocus::Note;
            self.set_category_manager_details_focus(details_focus);
        }

        if self.category_manager_details_inline_input().is_some() {
            match code {
                KeyCode::Esc => {
                    self.set_category_manager_details_inline_input(None);
                    self.status = "Numeric format edit canceled".to_string();
                    return Ok(true);
                }
                KeyCode::Enter => {
                    self.save_category_manager_numeric_inline_edit(agenda)?;
                    return Ok(true);
                }
                _ => {
                    let text_key = self.text_key_event(code);
                    if let Some(input) = self.category_manager_details_inline_input_mut() {
                        if input.buffer.handle_key_event(text_key, false) {
                            return Ok(true);
                        }
                    }
                }
            }
            return Ok(false);
        }

        if self.category_manager_save_key_pressed(code)
            && (self.category_manager_details_note_dirty()
                || self.category_manager_details_also_match_dirty())
            && !self.category_manager_details_note_editing()
            && !self.category_manager_details_also_match_editing()
        {
            // Let category-manager level save handling persist any dirty detail drafts
            // before text-entry auto-start consumes the key.
            return Ok(false);
        }

        if details_focus == CategoryManagerDetailsFocus::Note
            && self.category_manager_details_note_editing()
        {
            match code {
                KeyCode::Esc | KeyCode::Tab | KeyCode::BackTab => {
                    self.set_category_manager_details_note_editing(false);
                    if self.category_manager_details_note_dirty() {
                        self.save_category_manager_details_note(agenda)?;
                    }
                    // Esc is consumed (stays on Note); Tab/BackTab fall through to navigation.
                    return Ok(code == KeyCode::Esc);
                }
                _ => {
                    let text_key = self.text_key_event(code);
                    if let Some(buf) = self.category_manager_details_note_edit_mut() {
                        if buf.handle_key_event(text_key, true) {
                            self.recompute_category_manager_details_note_dirty();
                            return Ok(true);
                        }
                    }
                }
            }
            return Ok(false);
        }

        if details_focus == CategoryManagerDetailsFocus::AlsoMatch
            && self.category_manager_details_also_match_editing()
        {
            match code {
                KeyCode::Esc | KeyCode::Tab | KeyCode::BackTab => {
                    self.set_category_manager_details_also_match_editing(false);
                    if self.category_manager_details_also_match_dirty() {
                        self.save_category_manager_details_also_match(agenda)?;
                    }
                    return Ok(code == KeyCode::Esc);
                }
                _ => {
                    let text_key = self.text_key_event(code);
                    if let Some(buf) = self.category_manager_details_also_match_edit_mut() {
                        if buf.handle_key_event(text_key, true) {
                            self.recompute_category_manager_details_also_match_dirty();
                            return Ok(true);
                        }
                    }
                }
            }
            return Ok(false);
        }

        if details_focus == CategoryManagerDetailsFocus::Note
            && (matches!(code, KeyCode::Char(c) if c != ' ')
                || matches!(code, KeyCode::Backspace | KeyCode::Delete))
        {
            self.start_category_manager_details_note_edit();
            if self.category_manager_details_note_editing() {
                let text_key = self.text_key_event(code);
                if let Some(buf) = self.category_manager_details_note_edit_mut() {
                    if buf.handle_key_event(text_key, true) {
                        self.recompute_category_manager_details_note_dirty();
                    }
                }
                return Ok(true);
            }
        }

        if details_focus == CategoryManagerDetailsFocus::AlsoMatch
            && (matches!(code, KeyCode::Char(_))
                || matches!(code, KeyCode::Backspace | KeyCode::Delete))
        {
            self.start_category_manager_details_also_match_edit();
            if self.category_manager_details_also_match_editing() {
                let text_key = self.text_key_event(code);
                if let Some(buf) = self.category_manager_details_also_match_edit_mut() {
                    if buf.handle_key_event(text_key, true) {
                        self.recompute_category_manager_details_also_match_dirty();
                    }
                }
                return Ok(true);
            }
        }

        match code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.cycle_category_manager_details_focus(-1);
                return Ok(true);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.cycle_category_manager_details_focus(1);
                return Ok(true);
            }
            KeyCode::Enter => match details_focus {
                CategoryManagerDetailsFocus::Exclusive => {
                    self.toggle_selected_category_exclusive(agenda)?;
                    self.set_category_manager_focus(CategoryManagerFocus::Tree);
                    return Ok(true);
                }
                CategoryManagerDetailsFocus::AutoMatch => {
                    self.toggle_selected_category_implicit(agenda)?;
                    self.set_category_manager_focus(CategoryManagerFocus::Tree);
                    return Ok(true);
                }
                CategoryManagerDetailsFocus::SemanticMatch => {
                    self.toggle_selected_category_semantic(agenda)?;
                    self.set_category_manager_focus(CategoryManagerFocus::Tree);
                    return Ok(true);
                }
                CategoryManagerDetailsFocus::MatchCategoryName => {
                    self.toggle_selected_category_match_category_name(agenda)?;
                    self.set_category_manager_focus(CategoryManagerFocus::Tree);
                    return Ok(true);
                }
                CategoryManagerDetailsFocus::Actionable => {
                    self.toggle_selected_category_actionable(agenda)?;
                    self.set_category_manager_focus(CategoryManagerFocus::Tree);
                    return Ok(true);
                }
                CategoryManagerDetailsFocus::AlsoMatch => {
                    self.start_category_manager_details_also_match_edit();
                    return Ok(true);
                }
                CategoryManagerDetailsFocus::Integer => {
                    self.toggle_selected_category_integer_mode(agenda)?;
                    self.set_category_manager_focus(CategoryManagerFocus::Tree);
                    return Ok(true);
                }
                CategoryManagerDetailsFocus::DecimalPlaces => {
                    self.start_category_manager_numeric_inline_edit(
                        CategoryManagerDetailsInlineField::DecimalPlaces,
                    )?;
                    return Ok(true);
                }
                CategoryManagerDetailsFocus::CurrencySymbol => {
                    self.start_category_manager_numeric_inline_edit(
                        CategoryManagerDetailsInlineField::CurrencySymbol,
                    )?;
                    return Ok(true);
                }
                CategoryManagerDetailsFocus::ThousandsSeparator => {
                    self.toggle_selected_category_thousands_separator(agenda)?;
                    self.set_category_manager_focus(CategoryManagerFocus::Tree);
                    return Ok(true);
                }
                CategoryManagerDetailsFocus::Note => {
                    self.start_category_manager_details_note_edit();
                    return Ok(true);
                }
            },
            KeyCode::Char(' ') => match details_focus {
                CategoryManagerDetailsFocus::Exclusive => {
                    self.toggle_selected_category_exclusive(agenda)?;
                    return Ok(true);
                }
                CategoryManagerDetailsFocus::AutoMatch => {
                    self.toggle_selected_category_implicit(agenda)?;
                    return Ok(true);
                }
                CategoryManagerDetailsFocus::SemanticMatch => {
                    self.toggle_selected_category_semantic(agenda)?;
                    return Ok(true);
                }
                CategoryManagerDetailsFocus::MatchCategoryName => {
                    self.toggle_selected_category_match_category_name(agenda)?;
                    return Ok(true);
                }
                CategoryManagerDetailsFocus::Actionable => {
                    self.toggle_selected_category_actionable(agenda)?;
                    return Ok(true);
                }
                CategoryManagerDetailsFocus::AlsoMatch => {
                    self.start_category_manager_details_also_match_edit();
                    return Ok(true);
                }
                CategoryManagerDetailsFocus::Integer => {
                    self.toggle_selected_category_integer_mode(agenda)?;
                    return Ok(true);
                }
                CategoryManagerDetailsFocus::DecimalPlaces
                | CategoryManagerDetailsFocus::CurrencySymbol => {
                    return Ok(true);
                }
                CategoryManagerDetailsFocus::ThousandsSeparator => {
                    self.toggle_selected_category_thousands_separator(agenda)?;
                    return Ok(true);
                }
                CategoryManagerDetailsFocus::Note => {
                    self.start_category_manager_details_note_edit();
                    return Ok(true);
                }
            },
            _ => {}
        }

        Ok(false)
    }

    fn selected_category_mut(&self) -> Option<Category> {
        let row = self.selected_category_row()?;
        self.categories.iter().find(|c| c.id == row.id).cloned()
    }

    fn persist_selected_category_numeric_format(
        &mut self,
        agenda: &Agenda<'_>,
        next: NumericFormat,
        status: String,
    ) -> TuiResult<()> {
        let mut cat = self.selected_category_mut().ok_or("No category")?;
        cat.numeric_format = Some(next);
        agenda.store().update_category(&cat)?;
        let category_id = cat.id;
        self.refresh(agenda.store())?;
        self.set_category_selection_by_id(category_id);
        self.set_category_manager_focus(CategoryManagerFocus::Details);
        self.normalize_category_manager_details_focus();
        self.status = status;
        Ok(())
    }

    fn toggle_selected_category_integer_mode(&mut self, agenda: &Agenda<'_>) -> TuiResult<()> {
        let current = self
            .selected_category_numeric_format()
            .ok_or("No category")?;
        let mut next = current.clone();
        next.decimal_places = if current.decimal_places == 0 { 2 } else { 0 };
        self.persist_selected_category_numeric_format(
            agenda,
            next,
            format!(
                "Format: {}",
                if current.decimal_places == 0 {
                    "decimal mode"
                } else {
                    "integer mode"
                }
            ),
        )
    }

    fn toggle_selected_category_thousands_separator(
        &mut self,
        agenda: &Agenda<'_>,
    ) -> TuiResult<()> {
        let current = self
            .selected_category_numeric_format()
            .ok_or("No category")?;
        let mut next = current.clone();
        next.use_thousands_separator = !next.use_thousands_separator;
        self.persist_selected_category_numeric_format(
            agenda,
            next,
            if current.use_thousands_separator {
                "Thousands separator disabled".to_string()
            } else {
                "Thousands separator enabled".to_string()
            },
        )
    }

    fn start_category_manager_numeric_inline_edit(
        &mut self,
        field: CategoryManagerDetailsInlineField,
    ) -> TuiResult<()> {
        let current = self
            .selected_category_numeric_format()
            .ok_or("No category")?;
        if field == CategoryManagerDetailsInlineField::DecimalPlaces && current.decimal_places == 0
        {
            self.status = "Decimal places is disabled while Integer is enabled".to_string();
            self.set_category_manager_details_focus(CategoryManagerDetailsFocus::Integer);
            return Ok(());
        }
        let buffer = match field {
            CategoryManagerDetailsInlineField::DecimalPlaces => {
                text_buffer::TextBuffer::new(current.decimal_places.to_string())
            }
            CategoryManagerDetailsInlineField::CurrencySymbol => {
                text_buffer::TextBuffer::new(current.currency_symbol.unwrap_or_default())
            }
        };
        self.set_category_manager_focus(CategoryManagerFocus::Details);
        self.set_category_manager_details_focus(match field {
            CategoryManagerDetailsInlineField::DecimalPlaces => {
                CategoryManagerDetailsFocus::DecimalPlaces
            }
            CategoryManagerDetailsInlineField::CurrencySymbol => {
                CategoryManagerDetailsFocus::CurrencySymbol
            }
        });
        self.set_category_manager_details_inline_input(Some(CategoryManagerDetailsInlineInput {
            field,
            buffer,
        }));
        self.status = match field {
            CategoryManagerDetailsInlineField::DecimalPlaces => {
                "Editing decimal places: Enter save, Esc cancel".to_string()
            }
            CategoryManagerDetailsInlineField::CurrencySymbol => {
                "Editing currency symbol: Enter save, Esc cancel".to_string()
            }
        };
        Ok(())
    }

    fn save_category_manager_numeric_inline_edit(&mut self, agenda: &Agenda<'_>) -> TuiResult<()> {
        let Some(input) = self.category_manager_details_inline_input().cloned() else {
            return Ok(());
        };
        let mut next = self
            .selected_category_numeric_format()
            .ok_or("No category")?;
        match input.field {
            CategoryManagerDetailsInlineField::DecimalPlaces => {
                let raw = input.buffer.trimmed();
                let Ok(parsed) = raw.parse::<u8>() else {
                    self.status = "Decimal places must be a non-negative integer".to_string();
                    return Ok(());
                };
                next.decimal_places = parsed;
                self.set_category_manager_details_inline_input(None);
                self.persist_selected_category_numeric_format(
                    agenda,
                    next,
                    format!("Decimal places set to {parsed}"),
                )?;
            }
            CategoryManagerDetailsInlineField::CurrencySymbol => {
                let trimmed = input.buffer.trimmed();
                next.currency_symbol = if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                };
                self.set_category_manager_details_inline_input(None);
                self.persist_selected_category_numeric_format(
                    agenda,
                    next,
                    "Updated currency symbol".to_string(),
                )?;
            }
        }
        Ok(())
    }

    fn outdent_selected_category(&mut self, agenda: &Agenda<'_>) -> TuiResult<()> {
        if self.block_direct_structure_move_while_filtered() {
            return Ok(());
        }
        if self.selected_category_is_reserved() {
            self.status = "Reserved category structure is read-only".to_string();
            return Ok(());
        }
        let Some(category_id) = self.selected_category_id() else {
            self.status = "No selected category".to_string();
            return Ok(());
        };
        let Some(category) = self.categories.iter().find(|c| c.id == category_id) else {
            self.status = "Selected category missing".to_string();
            return Ok(());
        };
        let category_name = category.name.clone();
        let Some(parent_id) = category.parent else {
            self.status = format!("{category_name} is already at the top level");
            return Ok(());
        };
        let Some(parent) = self.categories.iter().find(|c| c.id == parent_id) else {
            self.status = "Outdent failed: parent category missing".to_string();
            return Ok(());
        };
        let new_parent_id = parent.parent;
        let target_siblings: Vec<CategoryId> = if let Some(grandparent_id) = new_parent_id {
            self.categories
                .iter()
                .find(|c| c.id == grandparent_id)
                .map(|grandparent| grandparent.children.clone())
                .unwrap_or_default()
        } else {
            self.categories
                .iter()
                .filter(|c| c.parent.is_none())
                .map(|c| c.id)
                .collect()
        };
        let insert_index = Some(
            target_siblings
                .iter()
                .position(|id| *id == parent_id)
                .map(|idx| idx + 1)
                .unwrap_or(target_siblings.len()),
        );
        let new_parent_label = self.category_manager_parent_label(new_parent_id);
        let result = agenda.move_category_to_parent(category_id, new_parent_id, insert_index)?;
        self.refresh(agenda.store())?;
        self.set_category_selection_by_id(category_id);
        self.status = format!(
            "Outdented {} to {} (processed_items={}, affected_items={})",
            category_name, new_parent_label, result.processed_items, result.affected_items
        );
        Ok(())
    }

    fn indent_selected_category_under_previous_sibling(
        &mut self,
        agenda: &Agenda<'_>,
    ) -> TuiResult<()> {
        if self.block_direct_structure_move_while_filtered() {
            return Ok(());
        }
        if self.selected_category_is_reserved() {
            self.status = "Reserved category structure is read-only".to_string();
            return Ok(());
        }
        let Some(category_id) = self.selected_category_id() else {
            self.status = "No selected category".to_string();
            return Ok(());
        };
        let Some(category) = self.categories.iter().find(|c| c.id == category_id) else {
            self.status = "Selected category missing".to_string();
            return Ok(());
        };
        let category_name = category.name.clone();
        let sibling_ids: Vec<CategoryId> = if let Some(parent_id) = category.parent {
            self.categories
                .iter()
                .find(|c| c.id == parent_id)
                .map(|parent| parent.children.clone())
                .unwrap_or_default()
        } else {
            self.categories
                .iter()
                .filter(|c| c.parent.is_none())
                .map(|c| c.id)
                .collect()
        };
        let Some(idx) = sibling_ids.iter().position(|id| *id == category_id) else {
            self.status = "Indent failed: category not found among siblings".to_string();
            return Ok(());
        };
        if idx == 0 {
            self.status = format!("{category_name} has no previous sibling to indent under");
            return Ok(());
        }
        let new_parent_id = Some(sibling_ids[idx - 1]);
        let new_parent_label = self.category_manager_parent_label(new_parent_id);
        let result = agenda.move_category_to_parent(category_id, new_parent_id, None)?;
        self.refresh(agenda.store())?;
        self.set_category_selection_by_id(category_id);
        self.status = format!(
            "Indented {} under {} (processed_items={}, affected_items={})",
            category_name, new_parent_label, result.processed_items, result.affected_items
        );
        Ok(())
    }

    fn reorder_selected_category_sibling(
        &mut self,
        delta: i32,
        agenda: &Agenda<'_>,
    ) -> TuiResult<()> {
        if self.block_direct_structure_move_while_filtered() {
            return Ok(());
        }
        if self.selected_category_is_reserved() {
            self.status = "Reserved category order is read-only".to_string();
            return Ok(());
        }
        let Some(category_id) = self.selected_category_id() else {
            self.status = "No selected category".to_string();
            return Ok(());
        };
        let Some(category) = self.categories.iter().find(|c| c.id == category_id) else {
            self.status = "Selected category missing".to_string();
            return Ok(());
        };
        let category_name = category.name.clone();
        let parent_id = category.parent;
        let sibling_ids: Vec<CategoryId> = if let Some(parent_id) = parent_id {
            self.categories
                .iter()
                .find(|c| c.id == parent_id)
                .map(|parent| parent.children.clone())
                .unwrap_or_default()
        } else {
            self.categories
                .iter()
                .filter(|c| c.parent.is_none())
                .map(|c| c.id)
                .collect()
        };
        let Some(idx) = sibling_ids.iter().position(|id| *id == category_id) else {
            self.status = "Reorder failed: category not found among siblings".to_string();
            return Ok(());
        };

        if (delta < 0 && idx == 0) || (delta > 0 && idx + 1 >= sibling_ids.len()) {
            self.status = if delta < 0 {
                format!("{category_name} is already first among siblings")
            } else {
                format!("{category_name} is already last among siblings")
            };
            return Ok(());
        }

        agenda.move_category_within_parent(category_id, delta.signum())?;
        self.refresh(agenda.store())?;
        self.set_category_selection_by_id(category_id);
        self.status = if delta < 0 {
            format!("Moved {category_name} up among siblings")
        } else {
            format!("Moved {category_name} down among siblings")
        };
        Ok(())
    }

    pub(crate) fn handle_category_manager_inline_action_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> TuiResult<bool> {
        let Some(action) = self.category_manager_inline_action().cloned() else {
            return Ok(false);
        };

        match action {
            CategoryInlineAction::Rename {
                category_id,
                original_name,
                mut buf,
            } => {
                match code {
                    KeyCode::Esc => {
                        self.set_category_manager_inline_action(None);
                        self.status = "Rename canceled".to_string();
                    }
                    KeyCode::Enter => {
                        let name = buf.trimmed().to_string();
                        if name.is_empty() {
                            self.status = "Name cannot be empty".to_string();
                        } else if is_reserved_category_name(&name)
                            && !original_name.eq_ignore_ascii_case(&name)
                        {
                            self.status = format!(
                                "Cannot rename to reserved category '{}'. Use a different name.",
                                name
                            );
                        } else if self.category_name_exists_elsewhere(&name, Some(category_id)) {
                            self.status = format!(
                                "Category '{}' already exists. Cannot rename duplicate.",
                                name
                            );
                        } else {
                            self.apply_category_inline_rename(
                                agenda,
                                category_id,
                                original_name,
                                name,
                            )?;
                        }
                    }
                    _ => {
                        if buf.handle_key_event(self.text_key_event(code), false) {
                            self.set_category_manager_inline_action(Some(
                                CategoryInlineAction::Rename {
                                    category_id,
                                    original_name,
                                    buf,
                                },
                            ));
                        }
                    }
                }
                Ok(true)
            }
            CategoryInlineAction::DeleteConfirm {
                category_id,
                category_name,
            } => {
                match category_inline_confirm_key_action(code) {
                    CategoryInlineConfirmKeyAction::Confirm => {
                        self.apply_category_inline_delete_confirm(
                            agenda,
                            category_id,
                            category_name,
                        )?;
                    }
                    CategoryInlineConfirmKeyAction::Cancel => {
                        self.set_category_manager_inline_action(None);
                        self.status = "Delete canceled".to_string();
                    }
                    CategoryInlineConfirmKeyAction::None => {}
                }
                Ok(true)
            }
        }
    }

    fn handle_workflow_setup_key(&mut self, code: KeyCode, agenda: &Agenda<'_>) -> TuiResult<bool> {
        match code {
            KeyCode::Esc | KeyCode::Char('w') => {
                self.workflow_setup_open = false;
                self.workflow_role_picker = None;
                self.status = "Workflow setup closed".to_string();
                return Ok(true);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.workflow_setup_focus = 1;
                return Ok(true);
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.workflow_setup_focus = 0;
                return Ok(true);
            }
            KeyCode::Char('x') => {
                self.clear_workflow_role(agenda, self.workflow_setup_focus)?;
                return Ok(true);
            }
            KeyCode::Enter => {
                self.open_workflow_role_picker_with_origin(
                    self.workflow_setup_focus,
                    WorkflowRolePickerOrigin::CategoryManager,
                );
                return Ok(true);
            }
            _ => {}
        }
        Ok(true)
    }

    pub(crate) fn workflow_role_picker_row_indices(&self) -> Vec<usize> {
        self.category_rows
            .iter()
            .enumerate()
            .filter_map(|(idx, row)| {
                if row.is_reserved || row.value_kind == CategoryValueKind::Numeric {
                    None
                } else {
                    Some(idx)
                }
            })
            .collect()
    }

    pub(crate) fn open_workflow_role_picker_with_origin(
        &mut self,
        role_index: usize,
        origin: WorkflowRolePickerOrigin,
    ) {
        let row_indices = self.workflow_role_picker_row_indices();
        if row_indices.is_empty() {
            self.status = "No eligible categories available for workflow roles".to_string();
            return;
        }
        let current_role_id = if role_index == 0 {
            self.workflow_config.ready_category_id
        } else {
            self.workflow_config.claim_category_id
        };
        let row_index = current_role_id
            .and_then(|category_id| {
                row_indices.iter().position(|idx| {
                    self.category_rows
                        .get(*idx)
                        .map(|row| row.id == category_id)
                        .unwrap_or(false)
                })
            })
            .unwrap_or(0);
        self.workflow_role_picker = Some(WorkflowRolePickerState {
            role_index,
            row_index,
            origin,
            scroll_offset: ScrollCell::new(0),
        });
        let role_label = if role_index == 0 {
            "Ready Queue"
        } else {
            "Claim Result"
        };
        self.status = format!("{role_label} picker: j/k select category, Enter assign, Esc back");
    }

    pub(crate) fn handle_workflow_role_picker_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> TuiResult<bool> {
        let visible_row_indices = self.workflow_role_picker_row_indices();
        let Some(picker) = self.workflow_role_picker.clone() else {
            return Ok(true);
        };
        match code {
            KeyCode::Esc | KeyCode::Char('w') => {
                self.workflow_role_picker = None;
                self.status = "Workflow category picker closed".to_string();
                return Ok(true);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(active_picker) = self.workflow_role_picker.as_mut() {
                    active_picker.row_index =
                        next_index_clamped(picker.row_index, visible_row_indices.len(), 1);
                }
                return Ok(true);
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(active_picker) = self.workflow_role_picker.as_mut() {
                    active_picker.row_index =
                        next_index_clamped(picker.row_index, visible_row_indices.len(), -1);
                }
                return Ok(true);
            }
            KeyCode::Char('x') => {
                self.clear_workflow_role(agenda, picker.role_index)?;
                self.workflow_role_picker = None;
                return Ok(true);
            }
            KeyCode::Enter => {
                let Some(row_idx) = visible_row_indices.get(picker.row_index).copied() else {
                    self.status = "No category selected".to_string();
                    return Ok(true);
                };
                let Some(row) = self.category_rows.get(row_idx).cloned() else {
                    self.status = "No category selected".to_string();
                    return Ok(true);
                };
                let preserved_selection = self.selected_category_id();
                if let Some(status) = self.workflow_setup_cross_role_conflict_status(
                    agenda,
                    picker.role_index,
                    row.id,
                )? {
                    self.status = status;
                    return Ok(true);
                }
                if picker.role_index == 0 {
                    self.assign_ready_queue_role(agenda, row.id, preserved_selection)?;
                } else {
                    self.assign_claim_result_role(agenda, row.id, preserved_selection)?;
                }
                self.workflow_role_picker = None;
                return Ok(true);
            }
            _ => {}
        }
        Ok(true)
    }

    fn clear_workflow_role(&mut self, agenda: &Agenda<'_>, role_index: usize) -> TuiResult<()> {
        let selected_category_id = self.selected_category_id();
        let mut workflow = self.workflow_config.clone();
        let (role_label, cleared_id) = if role_index == 0 {
            ("Ready Queue", workflow.ready_category_id.take())
        } else {
            ("Claim Result", workflow.claim_category_id.take())
        };
        let Some(cleared_id) = cleared_id else {
            self.status = format!("{role_label} is already unset");
            return Ok(());
        };
        let cleared_name = agenda.store().get_category(cleared_id)?.name;
        agenda.store().set_workflow_config(&workflow)?;
        self.refresh(agenda.store())?;
        if let Some(category_id) = selected_category_id {
            self.set_category_selection_by_id(category_id);
        }
        self.status = format!("Cleared {role_label} category ({cleared_name})");
        Ok(())
    }

    fn handle_classification_mode_picker_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> TuiResult<bool> {
        match code {
            KeyCode::Esc | KeyCode::Char('m') => {
                self.classification_mode_picker_open = false;
                self.status = "Classification mode picker closed".to_string();
                return Ok(true);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.classification_mode_picker_focus =
                    next_index_clamped(self.classification_mode_picker_focus, 3, 1);
                return Ok(true);
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.classification_mode_picker_focus =
                    next_index_clamped(self.classification_mode_picker_focus, 3, -1);
                return Ok(true);
            }
            KeyCode::Enter => {
                let mode = modes::classification::literal_mode_from_index(
                    self.classification_mode_picker_focus,
                );
                self.apply_category_manager_classification_mode(agenda, mode)?;
                self.classification_mode_picker_open = false;
                return Ok(true);
            }
            _ => {}
        }
        Ok(true)
    }

    pub(crate) fn handle_category_manager_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> TuiResult<bool> {
        self.ensure_category_manager_session();
        if self.handle_category_manager_inline_action_key(code, agenda)? {
            return Ok(false);
        }
        if self.category_manager_discard_confirm() {
            self.handle_category_manager_discard_confirm_key(code, agenda)?;
            return Ok(false);
        }
        if self.classification_mode_picker_open {
            self.handle_classification_mode_picker_key(code, agenda)?;
            return Ok(false);
        }
        if self.workflow_role_picker.is_some() {
            self.handle_workflow_role_picker_key(code, agenda)?;
            return Ok(false);
        }
        if self.workflow_setup_open {
            self.handle_workflow_setup_key(code, agenda)?;
            return Ok(false);
        }
        if self.handle_category_manager_details_key(code, agenda)? {
            return Ok(false);
        }
        if self.category_manager_filter_editing() {
            match code {
                KeyCode::Char('/') => {
                    self.set_category_manager_focus(CategoryManagerFocus::Filter);
                    return Ok(false);
                }
                KeyCode::Esc
                | KeyCode::F(9)
                | KeyCode::Tab
                | KeyCode::BackTab
                | KeyCode::Down
                | KeyCode::Up => {
                    self.set_category_manager_filter_editing(false);
                }
                _ => {
                    let text_key = self.text_key_event(code);
                    if let Some(filter) = self.category_manager_filter_mut() {
                        if filter.handle_key_event(text_key, false) {
                            self.rebuild_category_manager_visible_rows();
                            let count = self
                                .category_manager_visible_row_indices()
                                .map(|rows| rows.len())
                                .unwrap_or(0);
                            self.status = if count == 0 {
                                "No categories match filter".to_string()
                            } else {
                                format!("Category filter active: {} matches", count)
                            };
                            return Ok(false);
                        }
                    }
                }
            }
        }
        if !matches!(code, KeyCode::Char('<') | KeyCode::Char('>')) {
            self.set_category_manager_structure_move_prefix(None);
        }
        if self.category_manager_save_key_pressed(code)
            && (self.category_manager_details_note_dirty()
                || self.category_manager_details_also_match_dirty())
            && !self.category_manager_details_note_editing()
            && !self.category_manager_details_also_match_editing()
        {
            self.save_category_manager_dirty_details(agenda)?;
            return Ok(false);
        }
        match code {
            KeyCode::Tab => {
                self.handle_category_manager_tab_navigation(false);
            }
            KeyCode::BackTab => {
                self.handle_category_manager_tab_navigation(true);
            }
            KeyCode::Char('/') => {
                self.set_category_manager_focus(CategoryManagerFocus::Filter);
                self.set_category_manager_filter_editing(true);
                self.status = "Category filter: type to narrow list, Esc clears filter".to_string();
            }
            KeyCode::Esc | KeyCode::F(9) => {
                self.set_category_manager_filter_editing(false);
                if self
                    .category_manager_filter_text()
                    .is_some_and(|t| !t.trim().is_empty())
                {
                    if let Some(filter) = self.category_manager_filter_mut() {
                        filter.clear();
                    }
                    self.rebuild_category_manager_visible_rows();
                    self.set_category_manager_focus(CategoryManagerFocus::Tree);
                    self.status = "Category filter cleared".to_string();
                } else if self.category_manager_details_note_dirty()
                    || self.category_manager_details_also_match_dirty()
                {
                    self.set_category_manager_discard_confirm(true);
                    self.status =
                        "Save changes? y:save and close  n:discard  Esc:keep editing".to_string();
                } else {
                    self.close_category_manager_with_status("Category manager closed");
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.set_category_manager_filter_editing(false);
                self.save_category_manager_dirty_details(agenda)?;
                self.move_category_cursor(1)
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.set_category_manager_filter_editing(false);
                self.save_category_manager_dirty_details(agenda)?;
                self.move_category_cursor(-1)
            }
            KeyCode::Char('K') => {
                if self.category_manager_focus() == Some(CategoryManagerFocus::Details) {
                    return Ok(false);
                }
                self.reorder_selected_category_sibling(-1, agenda)?;
            }
            KeyCode::Char('J') => {
                if self.category_manager_focus() == Some(CategoryManagerFocus::Details) {
                    return Ok(false);
                }
                self.reorder_selected_category_sibling(1, agenda)?;
            }
            KeyCode::Char('<') => {
                if self.category_manager_focus() == Some(CategoryManagerFocus::Details) {
                    self.set_category_manager_structure_move_prefix(None);
                    return Ok(false);
                }
                if self.category_manager_structure_move_prefix() == Some('<') {
                    self.set_category_manager_structure_move_prefix(None);
                    self.outdent_selected_category(agenda)?;
                } else {
                    self.set_category_manager_structure_move_prefix(Some('<'));
                    self.status = "Press < again to outdent selected category (<<)".to_string();
                }
            }
            KeyCode::Char('>') => {
                if self.category_manager_focus() == Some(CategoryManagerFocus::Details) {
                    self.set_category_manager_structure_move_prefix(None);
                    return Ok(false);
                }
                if self.category_manager_structure_move_prefix() == Some('>') {
                    self.set_category_manager_structure_move_prefix(None);
                    self.indent_selected_category_under_previous_sibling(agenda)?;
                } else {
                    self.set_category_manager_structure_move_prefix(Some('>'));
                    self.status = "Press > again to indent selected category (>>)".to_string();
                }
            }
            KeyCode::Char('H') => {
                if self.category_manager_focus() == Some(CategoryManagerFocus::Details) {
                    return Ok(false);
                }
                self.outdent_selected_category(agenda)?;
            }
            KeyCode::Char('L') => {
                if self.category_manager_focus() == Some(CategoryManagerFocus::Details) {
                    return Ok(false);
                }
                self.indent_selected_category_under_previous_sibling(agenda)?;
            }
            KeyCode::Char('n') => {
                let selected_name = self.selected_category_row().map(|row| row.name.clone());
                let parent_id = self.selected_category_parent_id();
                let status = match selected_name {
                    Some(name) if parent_id.is_some() => {
                        let parent_label = self.category_manager_parent_label(parent_id);
                        format!("Create category at same level as {name} under {parent_label}")
                    }
                    Some(name) => format!("Create top-level category at same level as {name}"),
                    None => "Create top-level category".to_string(),
                };
                self.open_category_create_panel(parent_id, status);
            }
            KeyCode::Char('N') => {
                let selected_name = self.selected_category_row().map(|row| row.name.clone());
                let parent_id = if self.selected_category_is_numeric()
                    || self.selected_category_is_reserved()
                {
                    None
                } else {
                    self.selected_category_id()
                };
                let status = match selected_name {
                    Some(name) if parent_id.is_some() => format!("Create child category under {name}"),
                    Some(name) => {
                        format!("{name} cannot have child categories here; creating top-level category")
                    }
                    None => "Create top-level category".to_string(),
                };
                self.open_category_create_panel(parent_id, status);
            }
            KeyCode::Char('r') => {
                self.start_category_inline_rename();
            }
            KeyCode::Char('e') => {
                if self.selected_category_is_numeric() {
                    self.status = "Exclusive not applicable to numeric categories".to_string();
                } else {
                    self.toggle_selected_category_exclusive(agenda)?;
                }
            }
            KeyCode::Char('i') => {
                if self.selected_category_is_numeric() {
                    self.status = "Auto-match not applicable to numeric categories".to_string();
                } else {
                    self.toggle_selected_category_implicit(agenda)?;
                }
            }
            KeyCode::Char('g') => {
                if self.selected_category_is_numeric() {
                    self.status =
                        "Match-category-name not applicable to numeric categories".to_string();
                } else {
                    self.toggle_selected_category_match_category_name(agenda)?;
                }
            }
            KeyCode::Char('a') => {
                if self.selected_category_is_numeric() {
                    self.status = "Actionable not applicable to numeric categories".to_string();
                } else {
                    self.toggle_selected_category_actionable(agenda)?;
                }
            }
            KeyCode::Char('w') => {
                self.status =
                    "Workflow roles moved to Global Settings (return to Normal and use g s or F10)"
                        .to_string();
            }
            KeyCode::Char('m') => {
                self.status =
                    "Classification mode moved to Global Settings (return to Normal and use g s or F10)"
                        .to_string();
            }
            KeyCode::Enter => {
                self.set_category_manager_focus(CategoryManagerFocus::Details);
                self.status =
                    "Details pane focused: use j/k (or arrows) to select field, Enter/Space to edit/toggle"
                        .to_string();
            }
            KeyCode::Char('x') => {
                self.start_category_inline_delete_confirm();
            }
            _ => {}
        }
        Ok(false)
    }

    fn apply_category_manager_classification_mode(
        &mut self,
        agenda: &Agenda<'_>,
        mode: LiteralClassificationMode,
    ) -> TuiResult<()> {
        let mut config = self.classification_ui.config.clone();
        config.literal_mode = mode;
        config.sync_enabled_flag();
        let selected_category_id = self.selected_category_id();
        let manager_focus = self.category_manager_focus();
        let details_focus = self.category_manager_details_focus();
        let mode_label = modes::classification::literal_mode_label(config.literal_mode);

        agenda.store().set_classification_config(&config)?;
        self.refresh(agenda.store())?;
        self.mode = Mode::CategoryManager;
        if let Some(category_id) = selected_category_id {
            self.set_category_selection_by_id(category_id);
        }
        if let Some(focus) = manager_focus {
            self.set_category_manager_focus(focus);
        }
        if let Some(focus) = details_focus {
            self.set_category_manager_details_focus(focus);
        }
        self.status = format!("Literal classification: {mode_label}");
        Ok(())
    }

    pub(crate) fn toggle_selected_category_exclusive(
        &mut self,
        agenda: &Agenda<'_>,
    ) -> TuiResult<()> {
        if self.selected_category_is_reserved() {
            self.status = "Reserved category config is read-only".to_string();
            return Ok(());
        }
        let Some(category_id) = self.selected_category_id() else {
            self.status = "No selected category".to_string();
            return Ok(());
        };
        let mut category = agenda.store().get_category(category_id)?;
        category.is_exclusive = !category.is_exclusive;
        let updated = category.clone();
        let result = agenda.update_category(&category)?;
        self.refresh(agenda.store())?;
        self.set_category_selection_by_id(updated.id);
        self.status = format!(
            "{} exclusive={} (processed_items={}, affected_items={})",
            updated.name, updated.is_exclusive, result.processed_items, result.affected_items
        );
        Ok(())
    }

    pub(crate) fn toggle_selected_category_implicit(
        &mut self,
        agenda: &Agenda<'_>,
    ) -> TuiResult<()> {
        if self.selected_category_is_reserved() {
            self.status = "Reserved category config is read-only".to_string();
            return Ok(());
        }
        let Some(category_id) = self.selected_category_id() else {
            self.status = "No selected category".to_string();
            return Ok(());
        };
        let mut category = agenda.store().get_category(category_id)?;
        category.enable_implicit_string = !category.enable_implicit_string;
        let updated = category.clone();
        let result = agenda.update_category(&category)?;
        self.refresh(agenda.store())?;
        self.set_category_selection_by_id(updated.id);
        self.status = format!(
            "{} auto-match={} (processed_items={}, affected_items={})",
            updated.name,
            updated.enable_implicit_string,
            result.processed_items,
            result.affected_items
        );
        Ok(())
    }

    pub(crate) fn toggle_selected_category_semantic(
        &mut self,
        agenda: &Agenda<'_>,
    ) -> TuiResult<()> {
        if self.selected_category_is_reserved() {
            self.status = "Reserved category config is read-only".to_string();
            return Ok(());
        }
        let Some(category_id) = self.selected_category_id() else {
            self.status = "No selected category".to_string();
            return Ok(());
        };
        let mut category = agenda.store().get_category(category_id)?;
        category.enable_semantic_classification = !category.enable_semantic_classification;
        let updated = category.clone();
        let result = agenda.update_category(&category)?;
        self.refresh(agenda.store())?;
        self.set_category_selection_by_id(updated.id);
        self.status = format!(
            "{} semantic-match={} (processed_items={}, affected_items={})",
            updated.name,
            updated.enable_semantic_classification,
            result.processed_items,
            result.affected_items
        );
        Ok(())
    }

    pub(crate) fn toggle_selected_category_match_category_name(
        &mut self,
        agenda: &Agenda<'_>,
    ) -> TuiResult<()> {
        if self.selected_category_is_reserved() {
            self.status = "Reserved category config is read-only".to_string();
            return Ok(());
        }
        let Some(category_id) = self.selected_category_id() else {
            self.status = "No selected category".to_string();
            return Ok(());
        };
        let mut category = agenda.store().get_category(category_id)?;
        category.match_category_name = !category.match_category_name;
        let updated = category.clone();
        let result = agenda.update_category(&category)?;
        self.refresh(agenda.store())?;
        self.set_category_selection_by_id(updated.id);
        self.status = format!(
            "{} match-category-name={} (processed_items={}, affected_items={})",
            updated.name,
            updated.match_category_name,
            result.processed_items,
            result.affected_items
        );
        Ok(())
    }

    pub(crate) fn toggle_selected_category_actionable(
        &mut self,
        agenda: &Agenda<'_>,
    ) -> TuiResult<()> {
        if self.selected_category_is_reserved() {
            self.status = "Reserved category config is read-only".to_string();
            return Ok(());
        }
        let Some(category_id) = self.selected_category_id() else {
            self.status = "No selected category".to_string();
            return Ok(());
        };
        let mut category = agenda.store().get_category(category_id)?;
        category.is_actionable = !category.is_actionable;
        let updated = category.clone();
        let result = agenda.update_category(&category)?;
        self.refresh(agenda.store())?;
        self.set_category_selection_by_id(updated.id);
        self.status = format!(
            "{} actionable={} (processed_items={}, affected_items={})",
            updated.name, updated.is_actionable, result.processed_items, result.affected_items
        );
        Ok(())
    }

    fn assign_ready_queue_role(
        &mut self,
        agenda: &Agenda<'_>,
        category_id: CategoryId,
        selection_after: Option<CategoryId>,
    ) -> TuiResult<()> {
        let Some(row) = self.category_rows.iter().find(|row| row.id == category_id) else {
            self.status = "No selected category".to_string();
            return Ok(());
        };
        if row.is_reserved {
            self.status = "Reserved categories cannot be workflow roles".to_string();
            return Ok(());
        }
        if row.value_kind == CategoryValueKind::Numeric {
            self.status = "Workflow roles are not applicable to numeric categories".to_string();
            return Ok(());
        }

        let category = agenda.store().get_category(category_id)?;
        let mut workflow = self.workflow_config.clone();
        if workflow.ready_category_id == Some(category_id) {
            self.status = format!("{} is already the Ready Queue category", category.name);
            return Ok(());
        }
        let previous_ready_category_name = workflow
            .ready_category_id
            .and_then(|existing_id| agenda.store().get_category(existing_id).ok())
            .map(|existing| existing.name);
        if workflow.claim_category_id == Some(category_id) {
            self.status = format!(
                "{} is already the Claim Result category and cannot also be Ready Queue",
                category.name
            );
            return Ok(());
        }

        let prep = Some(self.prepare_category_for_workflow_role(category_id, agenda)?);
        workflow.ready_category_id = Some(category_id);
        agenda.store().set_workflow_config(&workflow)?;
        self.refresh(agenda.store())?;
        self.set_category_selection_by_id(selection_after.unwrap_or(category_id));
        self.status = Self::workflow_role_status_message(
            "Ready Queue",
            &category.name,
            previous_ready_category_name.as_deref(),
            prep.as_ref(),
        );
        Ok(())
    }

    fn assign_claim_result_role(
        &mut self,
        agenda: &Agenda<'_>,
        category_id: CategoryId,
        selection_after: Option<CategoryId>,
    ) -> TuiResult<()> {
        let Some(row) = self.category_rows.iter().find(|row| row.id == category_id) else {
            self.status = "No selected category".to_string();
            return Ok(());
        };
        if row.is_reserved {
            self.status = "Reserved categories cannot be workflow roles".to_string();
            return Ok(());
        }
        if row.value_kind == CategoryValueKind::Numeric {
            self.status = "Workflow roles are not applicable to numeric categories".to_string();
            return Ok(());
        }

        let category = agenda.store().get_category(category_id)?;
        let mut workflow = self.workflow_config.clone();
        if workflow.claim_category_id == Some(category_id) {
            self.status = format!("{} is already the Claim Result category", category.name);
            return Ok(());
        }
        let previous_claim_category_name = workflow
            .claim_category_id
            .and_then(|existing_id| agenda.store().get_category(existing_id).ok())
            .map(|existing| existing.name);
        if workflow.ready_category_id == Some(category_id) {
            self.status = format!(
                "{} is already the Ready Queue category and cannot also be Claim Result",
                category.name
            );
            return Ok(());
        }

        let prep = Some(self.prepare_category_for_workflow_role(category_id, agenda)?);
        workflow.claim_category_id = Some(category_id);
        agenda.store().set_workflow_config(&workflow)?;
        self.refresh(agenda.store())?;
        self.set_category_selection_by_id(selection_after.unwrap_or(category_id));
        self.status = Self::workflow_role_status_message(
            "Claim Result",
            &category.name,
            previous_claim_category_name.as_deref(),
            prep.as_ref(),
        );
        Ok(())
    }
}
