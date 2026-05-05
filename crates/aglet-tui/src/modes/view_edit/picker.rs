use crate::*;

fn is_immutable_view(view: &View) -> bool {
    aglet_core::store::is_system_view_name(&view.name)
}

impl App {
    fn selected_view_from_picker(&mut self) -> Option<View> {
        if self.views.is_empty() {
            return None;
        }
        let clamped_index = self.picker_index.min(self.views.len().saturating_sub(1));
        self.picker_index = clamped_index;
        self.views.get(clamped_index).cloned()
    }

    pub(crate) fn handle_view_picker_key(
        &mut self,
        code: KeyCode,
        aglet: &Aglet<'_>,
    ) -> TuiResult<bool> {
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
                    self.refresh(aglet.store())?;
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
                let mut view = View::new("Untitled View".to_string());
                view.show_unmatched = false;
                if view.sections.is_empty() {
                    view.sections.push(Self::view_edit_default_section(
                        Self::DEFAULT_VIEW_EDIT_SECTION_TITLE,
                    ));
                }
                self.open_view_edit_new_view_focus_name(view);
            }
            KeyCode::Char('d') | KeyCode::Char('D') => {
                self.status =
                    "Use n:new, then set View type to Datebook in View Settings".to_string();
            }
            KeyCode::Char('r') => {
                if let Some(view) = self.selected_view_from_picker() {
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
                if let Some(view) = self.selected_view_from_picker() {
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
                if let Some(view) = self.selected_view_from_picker() {
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
                if let Some(view) = self.selected_view_from_picker() {
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
                if let Some(view) = self.selected_view_from_picker() {
                    if is_immutable_view(&view) {
                        self.status = format!("{} view is immutable", view.name);
                        return Ok(false);
                    }
                    self.mode = Mode::ViewDeleteConfirm;
                    self.status = format!("Delete view '{}' ? y/n", view.name);
                } else {
                    self.status = "No selected view to delete".to_string();
                }
            }
            KeyCode::Down | KeyCode::Char('j') if !self.views.is_empty() => {
                self.picker_index = next_index_clamped(self.picker_index, self.views.len(), 1);
            }
            KeyCode::Up | KeyCode::Char('k') if !self.views.is_empty() => {
                self.picker_index = next_index_clamped(self.picker_index, self.views.len(), -1);
            }
            _ => {}
        }
        Ok(false)
    }

    pub(crate) fn handle_view_delete_key(
        &mut self,
        code: KeyCode,
        aglet: &Aglet<'_>,
    ) -> TuiResult<bool> {
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
                match aglet.store().delete_view(view.id) {
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
                        self.refresh(aglet.store())?;
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
        aglet: &Aglet<'_>,
    ) -> TuiResult<bool> {
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
                                aglet,
                                item_id,
                                false,
                                done_confirm.origin,
                                &blocked_item_ids,
                            )?;
                        }
                        DoneBlocksConfirmScope::Batch { item_ids, .. } => {
                            self.apply_batch_done_action(
                                aglet,
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
                                aglet,
                                item_id,
                                false,
                                done_confirm.origin,
                                &[],
                            )?;
                        }
                        DoneBlocksConfirmScope::Batch { item_ids, .. } => {
                            self.apply_batch_done_action(
                                aglet,
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
                    // Capture items for undo before deletion
                    let mut captured_items = Vec::new();
                    for item_id in &batch_delete_item_ids {
                        if let Ok(item) = aglet.store().get_item(*item_id) {
                            captured_items.push(item);
                        }
                    }
                    let mut deleted = 0usize;
                    let mut failed = 0usize;
                    let mut first_error = None;
                    for item_id in &batch_delete_item_ids {
                        match aglet.delete_item(*item_id, "user:tui") {
                            Ok(()) => deleted += 1,
                            Err(err) => {
                                failed += 1;
                                if first_error.is_none() {
                                    first_error = Some(err.to_string());
                                }
                            }
                        }
                    }
                    // Push undo entries for successfully deleted items (reverse
                    // order so undoing pops them back in original order)
                    for item in captured_items.into_iter().rev() {
                        self.push_undo(UndoEntry::ItemDeleted {
                            item: Box::new(item),
                        });
                    }
                    self.batch_delete_item_ids = None;
                    self.clear_selected_items();
                    self.refresh(aglet.store())?;
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
                    // Capture item state for undo before deletion
                    if let Ok(item) = aglet.store().get_item(item_id) {
                        self.push_undo(UndoEntry::ItemDeleted {
                            item: Box::new(item),
                        });
                    }
                    aglet.delete_item(item_id, "user:tui")?;
                    self.refresh(aglet.store())?;
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
