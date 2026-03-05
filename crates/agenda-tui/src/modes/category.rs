use crate::*;

enum CategoryInlineConfirmKeyAction {
    Confirm,
    Cancel,
    None,
}

fn category_inline_confirm_key_action(code: KeyCode) -> CategoryInlineConfirmKeyAction {
    match code {
        KeyCode::Char('y') => CategoryInlineConfirmKeyAction::Confirm,
        KeyCode::Esc => CategoryInlineConfirmKeyAction::Cancel,
        _ => CategoryInlineConfirmKeyAction::None,
    }
}

impl App {
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

    fn open_category_create_panel(&mut self, parent_id: Option<CategoryId>) {
        let parent_label = self.category_manager_parent_label(parent_id);
        self.input_panel = Some(input_panel::InputPanel::new_category_create(
            parent_id,
            &parent_label,
        ));
        // CategoryCreate uses InputPanel; clear any stale inline action first.
        self.set_category_manager_inline_action(None);
        self.name_input_context = Some(NameInputContext::CategoryCreate);
        self.mode = Mode::InputPanel;
        self.status = format!("Create category under {parent_label}");
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
    ) -> Result<(), String> {
        if name == original_name {
            self.set_category_manager_inline_action(None);
            self.status = "Category rename canceled (unchanged)".to_string();
            return Ok(());
        }
        let mut category = agenda
            .store()
            .get_category(category_id)
            .map_err(|e| e.to_string())?;
        if is_reserved_category_name(&category.name) {
            self.set_category_manager_inline_action(None);
            self.status = format!(
                "Category {} is reserved and cannot be renamed",
                category.name
            );
            return Ok(());
        }
        category.name = name.clone();
        match agenda.update_category(&category).map_err(|e| e.to_string()) {
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
    ) -> Result<(), String> {
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

    fn start_category_manager_details_note_edit(&mut self) {
        if self.selected_category_is_reserved() {
            self.status = "Reserved category config is read-only".to_string();
            return;
        }
        self.set_category_manager_focus(CategoryManagerFocus::Details);
        self.set_category_manager_details_focus(CategoryManagerDetailsFocus::Note);
        self.set_category_manager_details_note_editing(true);
        self.status = "Edit category note: type text, S:save  Esc:discard".to_string();
    }

    fn save_category_manager_details_note(&mut self, agenda: &Agenda<'_>) -> Result<(), String> {
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
        let mut category = agenda
            .store()
            .get_category(category_id)
            .map_err(|e| e.to_string())?;
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
        let result = agenda
            .update_category(&category)
            .map_err(|e| e.to_string())?;
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

    fn handle_category_manager_details_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> Result<bool, String> {
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
                    | CategoryManagerDetailsFocus::MatchName
                    | CategoryManagerDetailsFocus::Actionable
            )
        {
            details_focus = CategoryManagerDetailsFocus::Note;
            self.set_category_manager_details_focus(details_focus);
        }

        if details_focus == CategoryManagerDetailsFocus::Note
            && self.category_manager_details_note_editing()
        {
            match code {
                KeyCode::Char('S') => {
                    self.save_category_manager_details_note(agenda)?;
                    return Ok(true);
                }
                KeyCode::Char('s') if self.current_key_modifiers.contains(KeyModifiers::SHIFT) => {
                    self.save_category_manager_details_note(agenda)?;
                    return Ok(true);
                }
                KeyCode::Esc => {
                    if self.category_manager_details_note_dirty() {
                        self.reload_category_manager_details_note_from_selected();
                        self.mark_category_manager_details_note_dirty(false);
                        self.status = "Note changes discarded".to_string();
                    }
                    self.set_category_manager_details_note_editing(false);
                    return Ok(true);
                }
                KeyCode::Tab | KeyCode::BackTab => {
                    if self.category_manager_details_note_dirty() {
                        self.status = "Note has unsaved changes (S to save)".to_string();
                    }
                    self.set_category_manager_details_note_editing(false);
                    return Ok(false);
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

        match code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.cycle_category_manager_details_focus(-1);
                return Ok(true);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.cycle_category_manager_details_focus(1);
                return Ok(true);
            }
            KeyCode::Enter | KeyCode::Char(' ') => match details_focus {
                CategoryManagerDetailsFocus::Exclusive => {
                    self.toggle_selected_category_exclusive(agenda)?;
                    return Ok(true);
                }
                CategoryManagerDetailsFocus::MatchName => {
                    self.toggle_selected_category_implicit(agenda)?;
                    return Ok(true);
                }
                CategoryManagerDetailsFocus::Actionable => {
                    self.toggle_selected_category_actionable(agenda)?;
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

    fn outdent_selected_category(&mut self, agenda: &Agenda<'_>) -> Result<(), String> {
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
        let result = agenda
            .move_category_to_parent(category_id, new_parent_id, insert_index)
            .map_err(|e| e.to_string())?;
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
    ) -> Result<(), String> {
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
        let result = agenda
            .move_category_to_parent(category_id, new_parent_id, None)
            .map_err(|e| e.to_string())?;
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
    ) -> Result<(), String> {
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

        agenda
            .move_category_within_parent(category_id, delta.signum())
            .map_err(|e| e.to_string())?;
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
    ) -> Result<bool, String> {
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

    pub(crate) fn handle_category_manager_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> Result<bool, String> {
        self.ensure_category_manager_session();
        if self.handle_category_manager_inline_action_key(code, agenda)? {
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
        match code {
            KeyCode::Tab => {
                self.set_category_manager_filter_editing(false);
                self.cycle_category_manager_focus(1);
            }
            KeyCode::BackTab => {
                self.set_category_manager_filter_editing(false);
                self.cycle_category_manager_focus(-1);
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
                } else {
                    self.mode = Mode::Normal;
                    self.close_category_manager_session();
                    self.clear_input();
                    self.status = "Category manager closed".to_string();
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.set_category_manager_filter_editing(false);
                self.move_category_cursor(1)
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.set_category_manager_filter_editing(false);
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
                let parent_id = if self.selected_category_is_numeric()
                    || self.selected_category_is_reserved()
                {
                    None
                } else {
                    self.selected_category_id()
                };
                self.open_category_create_panel(parent_id);
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
                    self.status = "Match not applicable to numeric categories".to_string();
                } else {
                    self.toggle_selected_category_implicit(agenda)?;
                }
            }
            KeyCode::Char('a') => {
                if self.selected_category_is_numeric() {
                    self.status = "Actionable not applicable to numeric categories".to_string();
                } else {
                    self.toggle_selected_category_actionable(agenda)?;
                }
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

    pub(crate) fn toggle_selected_category_exclusive(
        &mut self,
        agenda: &Agenda<'_>,
    ) -> Result<(), String> {
        if self.selected_category_is_reserved() {
            self.status = "Reserved category config is read-only".to_string();
            return Ok(());
        }
        let Some(category_id) = self.selected_category_id() else {
            self.status = "No selected category".to_string();
            return Ok(());
        };
        let mut category = agenda
            .store()
            .get_category(category_id)
            .map_err(|e| e.to_string())?;
        category.is_exclusive = !category.is_exclusive;
        let updated = category.clone();
        let result = agenda
            .update_category(&category)
            .map_err(|e| e.to_string())?;
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
    ) -> Result<(), String> {
        if self.selected_category_is_reserved() {
            self.status = "Reserved category config is read-only".to_string();
            return Ok(());
        }
        let Some(category_id) = self.selected_category_id() else {
            self.status = "No selected category".to_string();
            return Ok(());
        };
        let mut category = agenda
            .store()
            .get_category(category_id)
            .map_err(|e| e.to_string())?;
        category.enable_implicit_string = !category.enable_implicit_string;
        let updated = category.clone();
        let result = agenda
            .update_category(&category)
            .map_err(|e| e.to_string())?;
        self.refresh(agenda.store())?;
        self.set_category_selection_by_id(updated.id);
        self.status = format!(
            "{} match-category-name={} (processed_items={}, affected_items={})",
            updated.name,
            updated.enable_implicit_string,
            result.processed_items,
            result.affected_items
        );
        Ok(())
    }

    pub(crate) fn toggle_selected_category_actionable(
        &mut self,
        agenda: &Agenda<'_>,
    ) -> Result<(), String> {
        if self.selected_category_is_reserved() {
            self.status = "Reserved category config is read-only".to_string();
            return Ok(());
        }
        let Some(category_id) = self.selected_category_id() else {
            self.status = "No selected category".to_string();
            return Ok(());
        };
        let mut category = agenda
            .store()
            .get_category(category_id)
            .map_err(|e| e.to_string())?;
        category.is_actionable = !category.is_actionable;
        let updated = category.clone();
        let result = agenda
            .update_category(&category)
            .map_err(|e| e.to_string())?;
        self.refresh(agenda.store())?;
        self.set_category_selection_by_id(updated.id);
        self.status = format!(
            "{} actionable={} (processed_items={}, affected_items={})",
            updated.name, updated.is_actionable, result.processed_items, result.affected_items
        );
        Ok(())
    }
}
