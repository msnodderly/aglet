use crate::*;

fn is_immutable_view(view: &View) -> bool {
    agenda_core::store::is_system_view_name(&view.name)
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
                    self.set_active_view_index(self.picker_index.min(self.views.len() - 1));
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
                        self.status = format!("{} view is immutable", view.name);
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
                        self.status = format!("{} view is immutable", view.name);
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
                        self.status = format!("{} view is immutable", view.name);
                        return Ok(false);
                    }
                    self.open_view_edit(view);
                } else {
                    self.status = "No views available".to_string();
                }
            }
            KeyCode::Char('c') => {
                if let Some(view) = self.views.get(self.picker_index).cloned() {
                    if is_immutable_view(&view) {
                        self.status = format!("{} view is immutable", view.name);
                        return Ok(false);
                    }
                    self.view_pending_clone_id = Some(view.id);
                    self.input_panel = Some(input_panel::InputPanel::new_name_input(
                        "",
                        &format!("Clone view '{}'", view.name),
                    ));
                    self.name_input_context = Some(NameInputContext::ViewClone);
                    self.mode = Mode::InputPanel;
                    self.status = format!(
                        "Clone view '{}': type new name, Enter to confirm, Esc to cancel",
                        view.name
                    );
                } else {
                    self.status = "No selected view to clone".to_string();
                }
            }
            KeyCode::Char('x') => {
                if let Some(view) = self.views.get(self.picker_index) {
                    if is_immutable_view(view) {
                        self.status = format!("{} view is immutable", view.name);
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
                    self.status = format!("Delete failed: {} view is immutable", view.name);
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
        let reference_date = jiff::Zoned::now().date();
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
                    match done_confirm.scope {
                        DoneBlocksConfirmScope::Single {
                            item_id,
                            blocked_item_ids,
                        } => {
                            self.apply_done_toggle_action(
                                agenda,
                                item_id,
                                false,
                                done_confirm.origin,
                                &blocked_item_ids,
                            )?;
                        }
                        DoneBlocksConfirmScope::Batch { item_ids, .. } => {
                            self.apply_batch_done_action(
                                agenda,
                                &item_ids,
                                true,
                                done_confirm.origin,
                            )?;
                        }
                    }
                }
                KeyCode::Char('n') => {
                    self.done_blocks_confirm = None;
                    match done_confirm.scope {
                        DoneBlocksConfirmScope::Single { item_id, .. } => {
                            self.apply_done_toggle_action(
                                agenda,
                                item_id,
                                false,
                                done_confirm.origin,
                                &[],
                            )?;
                        }
                        DoneBlocksConfirmScope::Batch { item_ids, .. } => {
                            self.apply_batch_done_action(
                                agenda,
                                &item_ids,
                                false,
                                done_confirm.origin,
                            )?;
                        }
                    }
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

        if let Some(batch_delete_item_ids) = self.batch_delete_item_ids.clone() {
            match code {
                KeyCode::Char('y') => {
                    let mut deleted = 0usize;
                    let mut failed = 0usize;
                    let mut first_error = None;
                    for item_id in &batch_delete_item_ids {
                        match agenda.delete_item(*item_id, "user:tui") {
                            Ok(()) => deleted += 1,
                            Err(err) => {
                                failed += 1;
                                if first_error.is_none() {
                                    first_error = Some(err.to_string());
                                }
                            }
                        }
                    }
                    self.batch_delete_item_ids = None;
                    self.clear_selected_items();
                    self.refresh(agenda.store())?;
                    self.mode = Mode::Normal;
                    self.status = if failed == 0 {
                        format!("Deleted {deleted} selected items")
                    } else {
                        let mut summary =
                            format!("Deleted {deleted} selected items (failed={failed})");
                        if let Some(err) = first_error {
                            summary.push_str(&format!(" first_error={err}"));
                        }
                        summary
                    };
                }
                KeyCode::Esc => {
                    self.batch_delete_item_ids = None;
                    self.mode = Mode::Normal;
                    self.status = "Batch delete canceled".to_string();
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
                self.batch_delete_item_ids = None;
                self.mode = Mode::Normal;
            }
            KeyCode::Esc => {
                self.done_blocks_confirm = None;
                self.batch_delete_item_ids = None;
                self.mode = Mode::Normal;
                self.status = "Delete canceled".to_string();
            }
            _ => {}
        }
        Ok(false)
    }
}
