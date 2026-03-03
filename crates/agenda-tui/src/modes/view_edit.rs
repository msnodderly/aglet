use crate::*;

fn is_immutable_view(view: &View) -> bool {
    view.name.eq_ignore_ascii_case("All Items")
}

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
                    self.slot_sort_keys.clear();
                    self.refresh(agenda.store())?;
                    self.reset_section_filters();
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
                self.view_pending_edit_name = None;
                self.input_panel =
                    Some(input_panel::InputPanel::new_name_input("", "New view name"));
                self.name_input_context = Some(NameInputContext::ViewCreate);
                self.mode = Mode::InputPanel;
                self.status =
                    "Create view: type name, Tab/Save to confirm, Esc to cancel".to_string();
            }
            KeyCode::Char('r') => {
                if let Some(view) = self.views.get(self.picker_index).cloned() {
                    if is_immutable_view(&view) {
                        self.status = "All Items view is immutable".to_string();
                        return Ok(false);
                    }
                    self.view_pending_edit_name = Some(view.name.clone());
                    self.input_panel = Some(input_panel::InputPanel::new_name_input(
                        &view.name,
                        "Rename view",
                    ));
                    self.name_input_context = Some(NameInputContext::ViewRename);
                    self.mode = Mode::InputPanel;
                    self.status = format!(
                        "Rename view {}: edit name, Save to confirm, Esc to cancel",
                        view.name
                    );
                } else {
                    self.status = "No selected view to rename".to_string();
                }
            }
            KeyCode::Char('e') | KeyCode::Char('E') => {
                if let Some(view) = self.views.get(self.picker_index).cloned() {
                    if is_immutable_view(&view) {
                        self.status = "All Items view is immutable".to_string();
                        return Ok(false);
                    }
                    self.open_view_edit(view);
                } else {
                    self.status = "No selected view to edit".to_string();
                }
            }
            KeyCode::Char('V') => {
                if let Some(view) = self.views.get(self.picker_index).cloned() {
                    if is_immutable_view(&view) {
                        self.status = "All Items view is immutable".to_string();
                        return Ok(false);
                    }
                    self.open_view_edit(view);
                } else {
                    self.status = "No views available".to_string();
                }
            }
            KeyCode::Char('x') => {
                if let Some(view) = self.views.get(self.picker_index) {
                    if is_immutable_view(view) {
                        self.status = "All Items view is immutable".to_string();
                        return Ok(false);
                    }
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
                if is_immutable_view(&view) {
                    self.mode = Mode::ViewPicker;
                    self.status = "Delete failed: All Items view is immutable".to_string();
                    return Ok(false);
                }
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
            KeyCode::Esc => {
                self.mode = Mode::ViewPicker;
                self.status = "Delete canceled".to_string();
            }
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
        if let Some(done_confirm) = self.done_blocks_confirm.clone() {
            match code {
                KeyCode::Char('y') => {
                    self.done_blocks_confirm = None;
                    self.apply_done_toggle_action(
                        agenda,
                        done_confirm.item_id,
                        false,
                        done_confirm.origin,
                        &done_confirm.blocked_item_ids,
                    )?;
                }
                KeyCode::Char('n') => {
                    self.done_blocks_confirm = None;
                    self.apply_done_toggle_action(
                        agenda,
                        done_confirm.item_id,
                        false,
                        done_confirm.origin,
                        &[],
                    )?;
                }
                KeyCode::Esc => {
                    self.done_blocks_confirm = None;
                    self.mode = Self::done_toggle_return_mode(done_confirm.origin);
                    self.status = "Done update canceled".to_string();
                }
                _ => {}
            }
            return Ok(false);
        }

        match code {
            KeyCode::Char('y') => {
                if let Some(item_id) = self.selected_item_id() {
                    agenda
                        .delete_item(item_id, "user:tui")
                        .map_err(|e| e.to_string())?;
                    self.refresh(agenda.store())?;
                    self.status = "Item deleted".to_string();
                }
                self.done_blocks_confirm = None;
                self.mode = Mode::Normal;
            }
            KeyCode::Esc => {
                self.done_blocks_confirm = None;
                self.mode = Mode::Normal;
                self.status = "Delete canceled".to_string();
            }
            _ => {}
        }
        Ok(false)
    }
}
