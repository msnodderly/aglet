use crate::*;

enum CategoryInlineConfirmKeyAction {
    Confirm,
    Cancel,
    None,
}

fn category_inline_confirm_key_action(code: KeyCode) -> CategoryInlineConfirmKeyAction {
    match code {
        KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y') => {
            CategoryInlineConfirmKeyAction::Confirm
        }
        KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
            CategoryInlineConfirmKeyAction::Cancel
        }
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

    fn category_name_exists_elsewhere(
        &self,
        candidate: &str,
        excluding_id: Option<CategoryId>,
    ) -> bool {
        self.categories.iter().any(|category| {
            Some(category.id) != excluding_id && category.name.eq_ignore_ascii_case(candidate)
        })
    }

    fn start_category_inline_create(&mut self, parent_id: Option<CategoryId>) {
        self.set_category_manager_inline_action(Some(CategoryInlineAction::Create {
            parent_id,
            buf: text_buffer::TextBuffer::empty(),
            confirm_name: None,
        }));
        let parent = self.category_manager_parent_label(parent_id);
        self.status =
            format!("Create category under {parent}: type name, Enter confirm, Esc cancel");
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

    fn apply_category_inline_create_confirm(
        &mut self,
        agenda: &Agenda<'_>,
        parent_id: Option<CategoryId>,
        name: String,
    ) -> Result<(), String> {
        let mut category = Category::new(name.clone());
        category.enable_implicit_string = true;
        category.parent = parent_id;
        let parent_label = self.category_manager_parent_label(parent_id);
        match agenda.create_category(&category).map_err(|e| e.to_string()) {
            Ok(result) => {
                self.refresh(agenda.store())?;
                self.set_category_selection_by_id(category.id);
                self.set_category_manager_inline_action(None);
                self.status = format!(
                    "Created category {name} under {parent_label} (processed_items={}, affected_items={})",
                    result.processed_items, result.affected_items
                );
            }
            Err(err) => {
                self.status = format!("Create failed: {err}");
            }
        }
        Ok(())
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

    fn reorder_selected_category_sibling(
        &mut self,
        delta: i32,
        agenda: &Agenda<'_>,
    ) -> Result<(), String> {
        if self
            .category_manager_filter_text()
            .map(|t| !t.trim().is_empty())
            .unwrap_or(false)
        {
            self.status =
                "Clear category filter before reordering (Phase 4 movement + filter behavior pending)"
                    .to_string();
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

    fn handle_category_manager_inline_action_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> Result<bool, String> {
        let Some(action) = self.category_manager_inline_action().cloned() else {
            return Ok(false);
        };

        match action {
            CategoryInlineAction::Create {
                parent_id,
                mut buf,
                confirm_name,
            } => {
                if let Some(confirm_name) = confirm_name {
                    match category_inline_confirm_key_action(code) {
                        CategoryInlineConfirmKeyAction::Confirm => {
                            self.apply_category_inline_create_confirm(
                                agenda,
                                parent_id,
                                confirm_name,
                            )?;
                        }
                        CategoryInlineConfirmKeyAction::Cancel => {
                            self.set_category_manager_inline_action(Some(
                                CategoryInlineAction::Create {
                                    parent_id,
                                    buf,
                                    confirm_name: None,
                                },
                            ));
                            self.status = "Create canceled. Continue editing name.".to_string();
                        }
                        CategoryInlineConfirmKeyAction::None => {}
                    }
                    return Ok(true);
                }

                match code {
                    KeyCode::Esc => {
                        self.set_category_manager_inline_action(None);
                        self.status = "Create canceled".to_string();
                    }
                    KeyCode::Enter => {
                        let name = buf.trimmed().to_string();
                        if name.is_empty() {
                            self.status = "Name cannot be empty".to_string();
                        } else if is_reserved_category_name(&name) {
                            self.status = format!(
                                "Cannot create reserved category '{}'. Use a different name.",
                                name
                            );
                        } else if self.category_name_exists_elsewhere(&name, None) {
                            self.status = format!(
                                "Category '{}' already exists. Cannot create duplicate.",
                                name
                            );
                        } else {
                            let parent_label = self.category_manager_parent_label(parent_id);
                            self.set_category_manager_inline_action(Some(
                                CategoryInlineAction::Create {
                                    parent_id,
                                    buf,
                                    confirm_name: Some(name.clone()),
                                },
                            ));
                            self.status =
                                format!("Create category '{}' under {}? (Y/n)", name, parent_label);
                        }
                    }
                    _ => {
                        if buf.handle_key(code, false) {
                            self.set_category_manager_inline_action(Some(
                                CategoryInlineAction::Create {
                                    parent_id,
                                    buf,
                                    confirm_name: None,
                                },
                            ));
                        }
                    }
                }
                Ok(true)
            }
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
                        if buf.handle_key(code, false) {
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
        if matches!(
            self.category_manager_focus(),
            Some(CategoryManagerFocus::Filter)
        ) {
            match code {
                KeyCode::Esc
                | KeyCode::F(9)
                | KeyCode::Tab
                | KeyCode::BackTab
                | KeyCode::Down
                | KeyCode::Up
                | KeyCode::Char('j')
                | KeyCode::Char('k') => {}
                _ => {
                    if let Some(filter) = self.category_manager_filter_mut() {
                        if filter.handle_key(code, false) {
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
        match code {
            KeyCode::Tab => {
                self.cycle_category_manager_focus(1);
            }
            KeyCode::BackTab => {
                self.cycle_category_manager_focus(-1);
            }
            KeyCode::Char('/') => {
                self.set_category_manager_focus(CategoryManagerFocus::Filter);
                self.status = "Category filter: type to narrow list, Esc clears filter".to_string();
            }
            KeyCode::Esc | KeyCode::F(9) => {
                if self
                    .category_manager_filter_text()
                    .map_or(false, |t| !t.trim().is_empty())
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
                    self.category_create_parent = None;
                    self.category_reparent_options.clear();
                    self.category_reparent_index = 0;
                    self.category_config_editor = None;
                    self.status = "Category manager closed".to_string();
                }
            }
            KeyCode::Down | KeyCode::Char('j') => self.move_category_cursor(1),
            KeyCode::Up | KeyCode::Char('k') => self.move_category_cursor(-1),
            KeyCode::Char('K') => {
                self.reorder_selected_category_sibling(-1, agenda)?;
            }
            KeyCode::Char('J') => {
                self.reorder_selected_category_sibling(1, agenda)?;
            }
            KeyCode::Char('n') => {
                self.start_category_inline_create(self.selected_category_id());
            }
            KeyCode::Char('N') => {
                self.start_category_inline_create(None);
            }
            KeyCode::Char('r') => {
                self.start_category_inline_rename();
            }
            KeyCode::Char('p') => {
                if let Some(category_id) = self.selected_category_id() {
                    self.category_reparent_options =
                        build_reparent_options(&self.category_rows, &self.categories, category_id);
                    self.category_reparent_index = self
                        .selected_category_parent_index(category_id)
                        .unwrap_or(0)
                        .min(self.category_reparent_options.len().saturating_sub(1));
                    self.mode = Mode::CategoryReparent;
                    self.status = "Reparent category: j/k select parent, Enter apply".to_string();
                }
            }
            KeyCode::Char('e') => {
                self.toggle_selected_category_exclusive(agenda)?;
            }
            KeyCode::Char('i') => {
                self.toggle_selected_category_implicit(agenda)?;
            }
            KeyCode::Char('a') => {
                self.toggle_selected_category_actionable(agenda)?;
            }
            KeyCode::Enter => {
                self.open_category_config_editor(agenda)?;
            }
            KeyCode::Char('x') => {
                self.start_category_inline_delete_confirm();
            }
            _ => {}
        }
        Ok(false)
    }

    pub(crate) fn open_category_config_editor(
        &mut self,
        agenda: &Agenda<'_>,
    ) -> Result<(), String> {
        let Some(row) = self.selected_category_row() else {
            self.status = "No selected category".to_string();
            return Ok(());
        };
        if row.is_reserved {
            self.status = format!("Category {} is reserved and cannot be edited", row.name);
            return Ok(());
        }

        let category = agenda
            .store()
            .get_category(row.id)
            .map_err(|e| e.to_string())?;
        let note = category.note.clone().unwrap_or_default();
        self.category_config_editor = Some(CategoryConfigState {
            category_id: category.id,
            category_name: category.name.clone(),
            is_exclusive: category.is_exclusive,
            is_actionable: category.is_actionable,
            enable_implicit_string: category.enable_implicit_string,
            note: crate::text_buffer::TextBuffer::new(note),
            focus: CategoryConfigFocus::Exclusive,
        });
        self.mode = Mode::CategoryConfig;
        self.status = format!(
            "Edit category config for {}: Space toggles, Enter saves (except note field)",
            category.name
        );
        Ok(())
    }

    pub(crate) fn save_category_config_editor(
        &mut self,
        agenda: &Agenda<'_>,
    ) -> Result<(), String> {
        let Some(editor) = self.category_config_editor.clone() else {
            self.mode = Mode::CategoryManager;
            self.status = "Category config editor closed".to_string();
            return Ok(());
        };

        let mut category = agenda
            .store()
            .get_category(editor.category_id)
            .map_err(|e| e.to_string())?;
        if is_reserved_category_name(&category.name) {
            self.mode = Mode::CategoryManager;
            self.category_config_editor = None;
            self.status = format!(
                "Category {} is reserved and cannot be edited",
                category.name
            );
            return Ok(());
        }

        let next_note = if editor.note.trimmed().is_empty() {
            None
        } else {
            Some(editor.note.text().to_string())
        };
        if category.is_exclusive == editor.is_exclusive
            && category.is_actionable == editor.is_actionable
            && category.enable_implicit_string == editor.enable_implicit_string
            && category.note == next_note
        {
            self.mode = Mode::CategoryManager;
            self.category_config_editor = None;
            self.status = "Category config canceled: no changes".to_string();
            return Ok(());
        }

        category.is_exclusive = editor.is_exclusive;
        category.is_actionable = editor.is_actionable;
        category.enable_implicit_string = editor.enable_implicit_string;
        category.note = next_note;
        let result = agenda
            .update_category(&category)
            .map_err(|e| e.to_string())?;
        self.refresh(agenda.store())?;
        self.set_category_selection_by_id(category.id);
        self.mode = Mode::CategoryManager;
        self.category_config_editor = None;
        self.status = format!(
            "Updated {} (processed_items={}, affected_items={})",
            category.name, result.processed_items, result.affected_items
        );
        Ok(())
    }

    pub(crate) fn handle_category_config_editor_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> Result<bool, String> {
        if self.category_config_editor.is_none() {
            self.mode = Mode::CategoryManager;
            self.status = "Category config editor closed".to_string();
            return Ok(false);
        }
        let focus = self
            .category_config_editor
            .as_ref()
            .map(|editor| editor.focus)
            .unwrap_or(CategoryConfigFocus::Exclusive);
        let category_name = self
            .category_config_editor
            .as_ref()
            .map(|editor| editor.category_name.clone())
            .unwrap_or_else(|| "(unknown)".to_string());

        match code {
            KeyCode::Esc => {
                self.mode = Mode::CategoryManager;
                self.category_config_editor = None;
                self.status = format!("Canceled config changes for {}", category_name);
            }
            KeyCode::Tab | KeyCode::BackTab => {
                self.cycle_category_config_focus(if matches!(code, KeyCode::BackTab) {
                    -1
                } else {
                    1
                });
            }
            KeyCode::Left => {
                self.move_category_config_checkbox_focus(-1);
            }
            KeyCode::Right => {
                self.move_category_config_checkbox_focus(1);
            }
            KeyCode::Char('h') => {
                if !matches!(focus, CategoryConfigFocus::Note) {
                    self.move_category_config_checkbox_focus(-1);
                } else {
                    self.handle_category_config_note_input_key(KeyCode::Char('h'));
                }
            }
            KeyCode::Char('l') => {
                if !matches!(focus, CategoryConfigFocus::Note) {
                    self.move_category_config_checkbox_focus(1);
                } else {
                    self.handle_category_config_note_input_key(KeyCode::Char('l'));
                }
            }
            KeyCode::Char('e') => {
                if !matches!(focus, CategoryConfigFocus::Note) {
                    self.toggle_category_config_exclusive();
                } else {
                    self.handle_category_config_note_input_key(KeyCode::Char('e'));
                }
            }
            KeyCode::Char('i') => {
                if !matches!(focus, CategoryConfigFocus::Note) {
                    self.toggle_category_config_no_implicit();
                } else {
                    self.handle_category_config_note_input_key(KeyCode::Char('i'));
                }
            }
            KeyCode::Char('a') => {
                if !matches!(focus, CategoryConfigFocus::Note) {
                    self.toggle_category_config_actionable();
                } else {
                    self.handle_category_config_note_input_key(KeyCode::Char('a'));
                }
            }
            KeyCode::Char(' ') => match focus {
                CategoryConfigFocus::Exclusive => self.toggle_category_config_exclusive(),
                CategoryConfigFocus::NoImplicit => self.toggle_category_config_no_implicit(),
                CategoryConfigFocus::Actionable => self.toggle_category_config_actionable(),
                CategoryConfigFocus::Note => {
                    self.handle_category_config_note_input_key(KeyCode::Char(' '));
                }
                CategoryConfigFocus::SaveButton | CategoryConfigFocus::CancelButton => {}
            },
            KeyCode::Enter => match focus {
                CategoryConfigFocus::Exclusive
                | CategoryConfigFocus::NoImplicit
                | CategoryConfigFocus::Actionable => self.save_category_config_editor(agenda)?,
                CategoryConfigFocus::Note => {
                    self.handle_category_config_note_input_key(KeyCode::Enter);
                }
                CategoryConfigFocus::SaveButton => self.save_category_config_editor(agenda)?,
                CategoryConfigFocus::CancelButton => {
                    self.mode = Mode::CategoryManager;
                    self.category_config_editor = None;
                    self.status = "Category config canceled".to_string();
                }
            },
            _ => {
                if matches!(focus, CategoryConfigFocus::Note) {
                    let _ = self.handle_category_config_note_input_key(code);
                }
            }
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

    pub(crate) fn handle_category_reparent_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> Result<bool, String> {
        match code {
            KeyCode::Esc => {
                self.mode = Mode::CategoryManager;
                self.category_reparent_options.clear();
                self.category_reparent_index = 0;
                self.status = "Category reparent canceled".to_string();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if !self.category_reparent_options.is_empty() {
                    self.category_reparent_index = next_index(
                        self.category_reparent_index,
                        self.category_reparent_options.len(),
                        1,
                    );
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if !self.category_reparent_options.is_empty() {
                    self.category_reparent_index = next_index(
                        self.category_reparent_index,
                        self.category_reparent_options.len(),
                        -1,
                    );
                }
            }
            KeyCode::Enter => {
                let Some(category_id) = self.selected_category_id() else {
                    self.mode = Mode::CategoryManager;
                    self.status = "Category reparent failed: no selection".to_string();
                    self.category_reparent_options.clear();
                    self.category_reparent_index = 0;
                    return Ok(false);
                };

                let Some(option) = self
                    .category_reparent_options
                    .get(self.category_reparent_index)
                    .cloned()
                else {
                    self.mode = Mode::CategoryManager;
                    self.status = "Category reparent failed: no parent selected".to_string();
                    self.category_reparent_options.clear();
                    self.category_reparent_index = 0;
                    return Ok(false);
                };

                let mut category = agenda
                    .store()
                    .get_category(category_id)
                    .map_err(|e| e.to_string())?;
                if category.parent == option.parent_id {
                    self.mode = Mode::CategoryManager;
                    self.status = "Category reparent canceled (unchanged)".to_string();
                    self.category_reparent_options.clear();
                    self.category_reparent_index = 0;
                    return Ok(false);
                }

                category.parent = option.parent_id;
                let result = agenda
                    .update_category(&category)
                    .map_err(|e| e.to_string())?;
                self.refresh(agenda.store())?;
                self.set_category_selection_by_id(category_id);
                self.mode = Mode::CategoryManager;
                self.status = format!(
                    "Reparented {} (processed_items={}, affected_items={})",
                    category.name, result.processed_items, result.affected_items
                );
                self.category_reparent_options.clear();
                self.category_reparent_index = 0;
            }
            _ => {}
        }

        Ok(false)
    }

    pub(crate) fn handle_category_delete_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> Result<bool, String> {
        match code {
            KeyCode::Char('y') => {
                if let Some(row) = self.selected_category_row().cloned() {
                    match agenda.store().delete_category(row.id) {
                        Ok(()) => {
                            self.refresh(agenda.store())?;
                            self.status = format!("Deleted category {}", row.name);
                        }
                        Err(err) => {
                            self.status = format!("Delete failed: {err}");
                        }
                    }
                }
                self.mode = Mode::CategoryManager;
            }
            KeyCode::Char('n') | KeyCode::Esc => {
                self.mode = Mode::CategoryManager;
                self.status = "Delete canceled".to_string();
            }
            _ => {}
        }
        Ok(false)
    }
}
