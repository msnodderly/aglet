use crate::*;

impl App {
    pub(crate) fn handle_view_picker_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> Result<bool, String> {
        match code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.status = "View switch canceled".to_string();
            }
            KeyCode::Enter => {
                if !self.views.is_empty() {
                    self.view_index = self.picker_index.min(self.views.len() - 1);
                    self.slot_index = 0;
                    self.item_index = 0;
                    self.refresh(agenda.store())?;
                    let view_name = self
                        .current_view()
                        .map(|view| view.name.clone())
                        .unwrap_or_else(|| "(none)".to_string());
                    self.status =
                        format!("Switched to view: {view_name} (press v then e to edit view)");
                } else {
                    self.status = "No views available".to_string();
                }
                self.mode = Mode::Normal;
            }
            KeyCode::Char('n') | KeyCode::Char('N') => {
                self.mode = Mode::ViewCreateNameInput;
                self.clear_input();
                self.view_pending_name = None;
                self.view_pending_edit_name = None;
                self.status = "Create view: type name and press Enter".to_string();
            }
            KeyCode::Char('r') => {
                if let Some(view) = self.views.get(self.picker_index).cloned() {
                    self.mode = Mode::ViewRenameInput;
                    self.set_input(view.name.clone());
                    self.view_pending_edit_name = Some(view.name.clone());
                    self.status = format!("Rename view {}: type name and Enter", view.name);
                } else {
                    self.status = "No selected view to rename".to_string();
                }
            }
            KeyCode::Char('e') | KeyCode::Char('E') => {
                if let Some(view) = self.views.get(self.picker_index).cloned() {
                    self.open_view_edit(view);
                } else {
                    self.status = "No selected view to edit".to_string();
                }
            }
            KeyCode::Char('V') => {
                if let Some(view) = self.views.get(self.picker_index).cloned() {
                    self.open_view_edit(view);
                } else {
                    self.status = "No views available".to_string();
                }
            }
            KeyCode::Char('x') => {
                if let Some(view) = self.views.get(self.picker_index) {
                    self.mode = Mode::ViewDeleteConfirm;
                    self.status = format!("Delete view '{}' ? y/n", view.name);
                } else {
                    self.status = "No selected view to delete".to_string();
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if !self.views.is_empty() {
                    self.picker_index = next_index_clamped(self.picker_index, self.views.len(), 1);
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if !self.views.is_empty() {
                    self.picker_index = next_index_clamped(self.picker_index, self.views.len(), -1);
                }
            }
            _ => {}
        }
        Ok(false)
    }


    pub(crate) fn handle_view_delete_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> Result<bool, String> {
        match code {
            KeyCode::Char('y') => {
                let Some(view) = self.views.get(self.picker_index).cloned() else {
                    self.mode = Mode::ViewPicker;
                    self.status = "Delete failed: no selected view".to_string();
                    return Ok(false);
                };
                let deleted_index = self.picker_index.min(self.views.len().saturating_sub(1));
                match agenda.store().delete_view(view.id) {
                    Ok(()) => {
                        match self.view_index.cmp(&deleted_index) {
                            std::cmp::Ordering::Greater => {
                                self.view_index -= 1;
                            }
                            std::cmp::Ordering::Equal => {
                                self.view_index = deleted_index.saturating_sub(1);
                            }
                            std::cmp::Ordering::Less => {}
                        }
                        self.refresh(agenda.store())?;
                        self.mode = Mode::ViewPicker;
                        self.picker_index =
                            self.picker_index.min(self.views.len().saturating_sub(1));
                        self.status = format!("Deleted view: {}", view.name);
                    }
                    Err(err) => {
                        self.mode = Mode::ViewPicker;
                        self.status = format!("Delete failed: {err}");
                    }
                }
            }
            KeyCode::Char('n') | KeyCode::Esc => {
                self.mode = Mode::ViewPicker;
                self.status = "Delete canceled".to_string();
            }
            _ => {}
        }
        Ok(false)
    }

    pub(crate) fn handle_view_create_name_key(&mut self, code: KeyCode) -> Result<bool, String> {
        match code {
            KeyCode::Esc => {
                self.mode = Mode::ViewPicker;
                self.clear_input();
                self.view_pending_name = None;
                self.status = "View create canceled".to_string();
            }
            KeyCode::Enter => {
                let name = self.input.trimmed().to_string();
                if name.is_empty() {
                    self.mode = Mode::ViewPicker;
                    self.clear_input();
                    self.view_pending_name = None;
                    self.status = "View create canceled (empty name)".to_string();
                } else {
                    self.view_pending_name = Some(name.clone());
                    self.view_category_index =
                        first_non_reserved_category_index(&self.category_rows);
                    self.view_create_include_selection.clear();
                    self.view_create_exclude_selection.clear();
                    self.mode = Mode::ViewCreateCategoryPicker;
                    self.clear_input();
                    self.status =
                        format!("Create view {name}: + include, - exclude, Enter creates");
                }
            }
            _ if self.handle_text_input_key(code) => {}
            _ => {}
        }
        Ok(false)
    }

    pub(crate) fn toggle_view_create_include(&mut self, category_id: CategoryId) {
        if !self.view_create_include_selection.insert(category_id) {
            self.view_create_include_selection.remove(&category_id);
        }
        self.view_create_exclude_selection.remove(&category_id);
    }

    pub(crate) fn toggle_view_create_exclude(&mut self, category_id: CategoryId) {
        if !self.view_create_exclude_selection.insert(category_id) {
            self.view_create_exclude_selection.remove(&category_id);
        }
        self.view_create_include_selection.remove(&category_id);
    }

    pub(crate) fn handle_view_create_category_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> Result<bool, String> {
        match code {
            KeyCode::Esc => {
                self.mode = Mode::ViewPicker;
                self.view_pending_name = None;
                self.view_create_include_selection.clear();
                self.view_create_exclude_selection.clear();
                self.status = "View create canceled".to_string();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if !self.category_rows.is_empty() {
                    self.view_category_index =
                        next_index(self.view_category_index, self.category_rows.len(), 1);
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if !self.category_rows.is_empty() {
                    self.view_category_index =
                        next_index(self.view_category_index, self.category_rows.len(), -1);
                }
            }
            KeyCode::Char(' ') | KeyCode::Char('+') => {
                if let Some(row) = self.category_rows.get(self.view_category_index) {
                    self.toggle_view_create_include(row.id);
                }
            }
            KeyCode::Char('-') => {
                if let Some(row) = self.category_rows.get(self.view_category_index) {
                    self.toggle_view_create_exclude(row.id);
                }
            }
            KeyCode::Enter => {
                let Some(name) = self.view_pending_name.clone() else {
                    self.mode = Mode::ViewPicker;
                    self.status = "View create failed: missing name".to_string();
                    return Ok(false);
                };

                let mut view = View::new(name.clone());
                if self.view_create_include_selection.is_empty()
                    && self.view_create_exclude_selection.is_empty()
                {
                    if let Some(row) = self.category_rows.get(self.view_category_index) {
                        view.criteria.include.insert(row.id);
                    }
                } else {
                    view.criteria
                        .include
                        .extend(self.view_create_include_selection.iter().copied());
                    view.criteria
                        .exclude
                        .extend(self.view_create_exclude_selection.iter().copied());
                }

                match agenda.store().create_view(&view) {
                    Ok(()) => {
                        let include_count = view.criteria.include.len();
                        let exclude_count = view.criteria.exclude.len();
                        let view_name = view.name.clone();
                        self.refresh(agenda.store())?;
                        self.view_pending_name = None;
                        self.view_create_include_selection.clear();
                        self.view_create_exclude_selection.clear();
                        if let Some(new_view) =
                            self.views.iter().find(|v| v.name == view_name).cloned()
                        {
                            self.open_view_edit(new_view);
                        } else {
                            self.mode = Mode::ViewPicker;
                        }
                        self.status = format!(
                            "Created view {} (include={}, exclude={})",
                            view_name, include_count, exclude_count
                        );
                    }
                    Err(err) => {
                        self.mode = Mode::ViewPicker;
                        self.view_create_include_selection.clear();
                        self.view_create_exclude_selection.clear();
                        self.status = format!("View create failed: {err}");
                    }
                }
            }
            _ => {}
        }
        Ok(false)
    }

    pub(crate) fn handle_view_rename_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> Result<bool, String> {
        match code {
            KeyCode::Esc => {
                self.mode = Mode::ViewPicker;
                self.clear_input();
                self.view_pending_edit_name = None;
                self.status = "View rename canceled".to_string();
            }
            KeyCode::Enter => {
                let Some(view_name) = self.view_pending_edit_name.clone() else {
                    self.mode = Mode::ViewPicker;
                    self.clear_input();
                    self.status = "View rename failed: no selected view".to_string();
                    return Ok(false);
                };

                let new_name = self.input.trimmed().to_string();
                if new_name.is_empty() {
                    self.mode = Mode::ViewPicker;
                    self.clear_input();
                    self.view_pending_edit_name = None;
                    self.status = "View rename canceled (empty name)".to_string();
                    return Ok(false);
                }

                let Some(mut view) = self
                    .views
                    .iter()
                    .find(|view| view.name.eq_ignore_ascii_case(&view_name))
                    .cloned()
                else {
                    self.mode = Mode::ViewPicker;
                    self.clear_input();
                    self.view_pending_edit_name = None;
                    self.status = "View rename failed: selected view not found".to_string();
                    return Ok(false);
                };

                if view.name == new_name {
                    self.mode = Mode::ViewPicker;
                    self.clear_input();
                    self.view_pending_edit_name = None;
                    self.status = "View rename canceled (unchanged)".to_string();
                    return Ok(false);
                }

                view.name = new_name.clone();
                match agenda.store().update_view(&view) {
                    Ok(()) => {
                        self.refresh(agenda.store())?;
                        self.set_view_selection_by_name(&new_name);
                        self.mode = Mode::ViewPicker;
                        self.clear_input();
                        self.view_pending_edit_name = None;
                        self.status = format!("Renamed view to {}", new_name);
                    }
                    Err(err) => {
                        self.mode = Mode::ViewPicker;
                        self.clear_input();
                        self.view_pending_edit_name = None;
                        self.status = format!("View rename failed: {err}");
                    }
                }
            }
            _ if self.handle_text_input_key(code) => {}
            _ => {}
        }
        Ok(false)
    }

    pub(crate) fn preview_count_for_query(&self, query: &Query) -> usize {
        let reference_date = Local::now().date_naive();
        evaluate_query(query, &self.all_items, reference_date).len()
    }

    pub(crate) fn handle_confirm_delete_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> Result<bool, String> {
        match code {
            KeyCode::Char('y') => {
                if let Some(item_id) = self.selected_item_id() {
                    agenda
                        .delete_item(item_id, "user:tui")
                        .map_err(|e| e.to_string())?;
                    self.refresh(agenda.store())?;
                    self.status = "Item deleted".to_string();
                }
                self.mode = Mode::Normal;
            }
            KeyCode::Char('n') | KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.status = "Delete canceled".to_string();
            }
            _ => {}
        }
        Ok(false)
    }
}
