use crate::*;

impl App {
    pub(crate) fn handle_category_manager_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> Result<bool, String> {
        match code {
            KeyCode::Esc | KeyCode::F(9) => {
                self.mode = Mode::Normal;
                self.clear_input();
                self.category_create_parent = None;
                self.category_reparent_options.clear();
                self.category_reparent_index = 0;
                self.category_config_editor = None;
                self.status = "Category manager closed".to_string();
            }
            KeyCode::Down | KeyCode::Char('j') => self.move_category_cursor(1),
            KeyCode::Up | KeyCode::Char('k') => self.move_category_cursor(-1),
            KeyCode::Char('n') => {
                self.mode = Mode::CategoryCreate;
                self.clear_input();
                self.category_create_parent = self.selected_category_id();
                let parent = self
                    .create_parent_name()
                    .unwrap_or_else(|| "top level".to_string());
                self.status = format!("Create subcategory under {parent}: type name and Enter");
            }
            KeyCode::Char('N') => {
                self.mode = Mode::CategoryCreate;
                self.clear_input();
                self.category_create_parent = None;
                self.status =
                    "Create top-level category (no parent): type name and Enter".to_string();
            }
            KeyCode::Char('r') => {
                if let Some(row) = self.selected_category_row() {
                    let row_name = row.name.clone();
                    self.mode = Mode::CategoryRename;
                    self.set_input(row_name.clone());
                    self.status = format!("Rename category {}: type name and Enter", row_name);
                }
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
                if let Some(row) = self.selected_category_row() {
                    let row_name = row.name.clone();
                    self.mode = Mode::CategoryDelete;
                    self.status = format!("Delete category \"{}\"? y/n", row_name);
                }
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

    pub(crate) fn handle_category_create_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> Result<bool, String> {
        match code {
            KeyCode::Esc => {
                self.mode = Mode::CategoryManager;
                self.clear_input();
                self.category_create_parent = None;
                self.status = "Category create canceled".to_string();
            }
            KeyCode::Enter => {
                let name = self.input.trimmed().to_string();
                if !name.is_empty() {
                    let mut category = Category::new(name.clone());
                    category.enable_implicit_string = true;
                    category.parent = self.category_create_parent;
                    let parent_label = self
                        .create_parent_name()
                        .unwrap_or_else(|| "top level".to_string());
                    let create_result =
                        agenda.create_category(&category).map_err(|e| e.to_string());
                    match create_result {
                        Ok(result) => {
                            self.refresh(agenda.store())?;
                            self.set_category_selection_by_id(category.id);
                            self.mode = Mode::CategoryManager;
                            self.status = format!(
                                "Created category {} under {} (processed_items={}, affected_items={})",
                                category.name,
                                parent_label,
                                result.processed_items,
                                result.affected_items
                            );
                        }
                        Err(err) => {
                            self.mode = Mode::CategoryManager;
                            self.status = format!("Create failed: {err}");
                        }
                    }
                } else {
                    self.mode = Mode::CategoryManager;
                    self.status = "Category create canceled (empty name)".to_string();
                }
                self.clear_input();
                self.category_create_parent = None;
            }
            _ if self.handle_text_input_key(code) => {}
            _ => {}
        }
        Ok(false)
    }

    pub(crate) fn handle_category_rename_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> Result<bool, String> {
        match code {
            KeyCode::Esc => {
                self.mode = Mode::CategoryManager;
                self.clear_input();
                self.status = "Category rename canceled".to_string();
            }
            KeyCode::Enter => {
                let Some(category_id) = self.selected_category_id() else {
                    self.mode = Mode::CategoryManager;
                    self.clear_input();
                    self.status = "Category rename failed: no selection".to_string();
                    return Ok(false);
                };

                let new_name = self.input.trimmed().to_string();
                if new_name.is_empty() {
                    self.mode = Mode::CategoryManager;
                    self.clear_input();
                    self.status = "Category rename canceled (empty name)".to_string();
                    return Ok(false);
                }

                let mut category = agenda
                    .store()
                    .get_category(category_id)
                    .map_err(|e| e.to_string())?;
                if category.name == new_name {
                    self.mode = Mode::CategoryManager;
                    self.clear_input();
                    self.status = "Category rename canceled (unchanged)".to_string();
                    return Ok(false);
                }

                category.name = new_name.clone();
                let result = agenda
                    .update_category(&category)
                    .map_err(|e| e.to_string())?;
                self.refresh(agenda.store())?;
                self.set_category_selection_by_id(category_id);
                self.mode = Mode::CategoryManager;
                self.clear_input();
                self.status = format!(
                    "Renamed category to {} (processed_items={}, affected_items={})",
                    new_name, result.processed_items, result.affected_items
                );
            }
            _ if self.handle_text_input_key(code) => {}
            _ => {}
        }
        Ok(false)
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
