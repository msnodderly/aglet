use crate::*;

impl App {
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
        self.mode = Mode::InspectUnassignPicker;
        self.inspect_assignment_index = self.inspect_assignment_index.min(rows.len() - 1);
        self.status = "Select assignment to unassign (j/k, Enter, Esc)".to_string();
    }

    pub(crate) fn handle_normal_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> Result<bool, String> {
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
            KeyCode::Right | KeyCode::Char('l') => self.move_slot_cursor(1),
            KeyCode::Left | KeyCode::Char('h') => self.move_slot_cursor(-1),
            KeyCode::Char('n') => {
                self.mode = Mode::AddInput;
                self.clear_input();
                self.status = "Add item: type text and press Enter".to_string();
            }
            KeyCode::Char('e') => {
                self.open_item_edit_for_selected_item();
            }
            KeyCode::Enter => {
                self.open_item_edit_for_selected_item();
            }
            KeyCode::Char('m') => {
                if let Some(item) = self.selected_item() {
                    let existing_note = item.note.clone().unwrap_or_default();
                    self.mode = Mode::NoteEditInput;
                    self.set_input(existing_note);
                    self.status =
                        "Edit note: Enter to save (empty clears), Esc to cancel".to_string();
                } else {
                    self.status = "No selected item to add/edit note".to_string();
                }
            }
            KeyCode::Char('/') => {
                self.mode = Mode::FilterInput;
                self.set_input(self.filter.clone().unwrap_or_default());
                self.status = "Filter: type query and press Enter (Esc clears)".to_string();
            }
            KeyCode::Esc => {
                if self.filter.take().is_some() {
                    self.refresh(agenda.store())?;
                    self.status = "Filter cleared".to_string();
                }
            }
            KeyCode::F(8) | KeyCode::Char('v') => {
                self.mode = Mode::ViewPicker;
                self.picker_index = self.view_index;
                self.status =
                    "View palette: Enter switch, N create, r rename, x delete, e edit view, Esc cancel"
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
                self.jump_to_all_items_view(agenda)?;
            }
            KeyCode::Char('a') => {
                if self.selected_item_id().is_none() {
                    self.status = "No selected item to edit categories".to_string();
                } else if self.category_rows.is_empty() {
                    self.status = "No categories available".to_string();
                } else {
                    self.mode = Mode::ItemAssignCategoryPicker;
                    self.item_assign_return_to_item_edit = false;
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
                } else {
                    if self.selected_item_id().is_none() {
                        self.status = "No selected item to edit categories".to_string();
                    } else if self.category_rows.is_empty() {
                        self.status = "No categories available".to_string();
                    } else {
                        self.mode = Mode::ItemAssignCategoryPicker;
                        self.item_assign_return_to_item_edit = false;
                        self.item_assign_category_index =
                            first_non_reserved_category_index(&self.category_rows);
                        self.clear_input();
                        self.status =
                            "Item categories: j/k select, Space toggle, n type category, Enter done, Esc cancel"
                                .to_string();
                    }
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

    pub(crate) fn open_item_edit_for_selected_item(&mut self) {
        if let Some(item) = self.selected_item() {
            let existing_text = item.text.clone();
            let existing_note = item.note.clone().unwrap_or_default();
            self.mode = Mode::ItemEditInput;
            self.set_input(existing_text);
            self.item_edit_focus = ItemEditFocus::Text;
            self.item_edit_note = existing_note;
            self.item_edit_note_cursor = self.item_edit_note.chars().count();
            self.status =
                "Edit item: Tab cycles fields/buttons, Enter activates focused control, Up/Down in note"
                    .to_string();
        } else {
            self.status = "No selected item to edit".to_string();
        }
    }

    pub(crate) fn handle_add_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> Result<bool, String> {
        match code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.clear_input();
                self.status = "Add canceled".to_string();
            }
            KeyCode::Enter => {
                let text = self.input.trimmed();
                if !text.is_empty() {
                    let text_value = text.to_string();
                    let category_names: Vec<String> = agenda
                        .store()
                        .get_hierarchy()
                        .map_err(|e| e.to_string())?
                        .into_iter()
                        .map(|category| category.name)
                        .collect();
                    let unknown_hashtags = unknown_hashtag_tokens(&text_value, &category_names);
                    let parsed_when = self.create_item_in_current_context(agenda, text_value)?;
                    self.status = add_capture_status_message(parsed_when, &unknown_hashtags);
                }
                self.mode = Mode::Normal;
                self.clear_input();
            }
            _ if self.handle_text_input_key(code) => {}
            _ => {}
        }
        Ok(false)
    }

    pub(crate) fn handle_item_edit_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> Result<bool, String> {
        match code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.clear_input();
                self.item_edit_note.clear();
                self.item_edit_note_cursor = 0;
                self.item_edit_focus = ItemEditFocus::Text;
                self.status = "Edit canceled".to_string();
            }
            KeyCode::Tab | KeyCode::BackTab => {
                self.cycle_item_edit_focus(if matches!(code, KeyCode::BackTab) {
                    -1
                } else {
                    1
                });
            }
            KeyCode::F(3) => {
                self.item_edit_focus = ItemEditFocus::CategoriesButton;
                self.open_item_assign_picker_from_item_edit();
            }
            KeyCode::Enter => match self.item_edit_focus {
                ItemEditFocus::Text => {
                    self.cycle_item_edit_focus(1);
                }
                ItemEditFocus::Note => {
                    self.insert_item_edit_note_newline();
                }
                ItemEditFocus::CategoriesButton => {
                    self.open_item_assign_picker_from_item_edit();
                }
                ItemEditFocus::SaveButton => {
                    self.save_item_edit(agenda)?;
                }
                ItemEditFocus::CancelButton => {
                    self.mode = Mode::Normal;
                    self.clear_input();
                    self.item_edit_note.clear();
                    self.item_edit_note_cursor = 0;
                    self.item_edit_focus = ItemEditFocus::Text;
                    self.status = "Edit canceled".to_string();
                }
            },
            _ if self.handle_item_edit_field_input_key(code) => {}
            _ => {}
        }
        Ok(false)
    }

    pub(crate) fn open_item_assign_picker_from_item_edit(&mut self) {
        if self.selected_item_id().is_none() {
            self.status = "No selected item to edit categories".to_string();
            return;
        }
        if self.category_rows.is_empty() {
            self.status = "No categories available".to_string();
            return;
        }
        self.mode = Mode::ItemAssignCategoryPicker;
        self.item_assign_return_to_item_edit = true;
        self.item_assign_category_index = first_non_reserved_category_index(&self.category_rows);
        self.status =
            "Item categories: j/k select, Space toggle, n type category, Enter done, Esc cancel"
                .to_string();
    }

    pub(crate) fn save_item_edit(&mut self, agenda: &Agenda<'_>) -> Result<(), String> {
        let Some(item_id) = self.selected_item_id() else {
            self.mode = Mode::Normal;
            self.clear_input();
            self.item_edit_note.clear();
            self.item_edit_note_cursor = 0;
            self.item_edit_focus = ItemEditFocus::Text;
            self.status = "Edit failed: no selected item".to_string();
            return Ok(());
        };

        let updated_text = self.input.trimmed().to_string();
        if updated_text.is_empty() {
            self.status = "Cannot save: text cannot be empty".to_string();
            return Ok(());
        }
        let updated_note = if self.item_edit_note.trim().is_empty() {
            None
        } else {
            Some(self.item_edit_note.clone())
        };

        let mut item = agenda
            .store()
            .get_item(item_id)
            .map_err(|e| e.to_string())?;
        if item.text == updated_text && item.note == updated_note {
            self.mode = Mode::Normal;
            self.clear_input();
            self.item_edit_note.clear();
            self.item_edit_note_cursor = 0;
            self.item_edit_focus = ItemEditFocus::Text;
            self.status = "Edit canceled: no changes".to_string();
            return Ok(());
        }

        item.text = updated_text;
        item.note = updated_note;
        item.modified_at = Utc::now();
        let reference_date = Local::now().date_naive();
        agenda
            .update_item_with_reference_date(&item, reference_date)
            .map_err(|e| e.to_string())?;
        self.refresh(agenda.store())?;
        self.set_item_selection_by_id(item_id);
        self.mode = Mode::Normal;
        self.clear_input();
        self.item_edit_note.clear();
        self.item_edit_note_cursor = 0;
        self.item_edit_focus = ItemEditFocus::Text;
        self.status = "Item updated".to_string();
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
            KeyCode::Enter => {
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
                self.mode = if self.item_assign_return_to_item_edit {
                    Mode::ItemEditInput
                } else {
                    Mode::Normal
                };
                self.item_assign_return_to_item_edit = false;
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
                self.mode = Mode::ItemAssignCategoryInput;
                self.clear_input();
                self.status = "Type category name: Enter assign/create, Esc back".to_string();
            }
            KeyCode::Char(' ') => {
                let Some(item_id) = self.selected_item_id() else {
                    self.mode = if self.item_assign_return_to_item_edit {
                        Mode::ItemEditInput
                    } else {
                        Mode::Normal
                    };
                    self.item_assign_return_to_item_edit = false;
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
                self.mode = if self.item_assign_return_to_item_edit {
                    Mode::ItemEditInput
                } else {
                    Mode::Normal
                };
                self.item_assign_return_to_item_edit = false;
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
                self.mode = Mode::ItemAssignCategoryPicker;
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
                    self.mode = Mode::ItemAssignCategoryPicker;
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
                self.mode = Mode::ItemAssignCategoryPicker;
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
                self.mode = Mode::Normal;
                self.filter = None;
                self.clear_input();
                self.refresh(agenda.store())?;
                self.status = "Filter cleared".to_string();
            }
            KeyCode::Enter => {
                self.mode = Mode::Normal;
                let value = self.input.trimmed().to_string();
                self.filter = if value.is_empty() { None } else { Some(value) };
                self.refresh(agenda.store())?;
                self.status = if self.filter.is_some() {
                    "Filter applied".to_string()
                } else {
                    "Filter cleared".to_string()
                };
                self.clear_input();
            }
            _ if self.handle_text_input_key(code) => {}
            _ => {}
        }
        Ok(false)
    }
}
