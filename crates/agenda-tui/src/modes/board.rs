use crate::*;

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
        self.category_direct_edit_create_confirm = None;
    }

    pub(crate) fn active_category_direct_edit_row(&self) -> Option<&CategoryDirectEditRow> {
        self.category_direct_edit_state()?.active_row()
    }

    fn active_category_direct_edit_row_mut(&mut self) -> Option<&mut CategoryDirectEditRow> {
        self.category_direct_edit_state_mut()?.active_row_mut()
    }

    pub(crate) fn active_category_direct_edit_input_text(&self) -> Option<&str> {
        self.active_category_direct_edit_row().map(|row| row.input.text())
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

        if self.column_index == 0 {
            return None;
        }
        let section = view.sections.get(section_index)?;
        let section_column_index = self.column_index - 1;
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
            a_name.cmp(&b_name).then_with(|| a.to_string().cmp(&b.to_string()))
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
        if active_input.trim().is_empty() {
            child_ids
                .into_iter()
                .filter(|id| {
                    self.categories
                        .iter()
                        .find(|c| c.id == *id)
                        .map(|c| !c.name.eq_ignore_ascii_case("When"))
                        .unwrap_or(false)
                })
                .collect()
        } else {
            filter_child_categories(&child_ids, &self.categories, active_input)
        }
    }

    fn update_suggestions(&mut self) {
        if self.category_direct_edit_create_confirm.is_some() {
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
        if self.category_direct_edit_create_confirm.is_some() {
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
        if self.category_direct_edit_create_confirm.is_some() {
            return;
        }
        let matches = self.get_current_suggest_matches();
        let Some(state) = self.category_direct_edit_state() else {
            return;
        };
        let Some(&id) = matches.get(state.suggest_index.min(matches.len().saturating_sub(1))) else {
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

    fn assign_selected_suggestion(&mut self, agenda: &Agenda<'_>) -> Result<(), String> {
        let matches = self.get_current_suggest_matches();
        let Some(state) = self.category_direct_edit_state() else {
            return Ok(());
        };
        let Some(&id) = matches.get(state.suggest_index.min(matches.len().saturating_sub(1))) else {
            return Ok(());
        };
        let Some(item_id) = self.selected_item_id() else {
            return Ok(());
        };
        self.replace_column_child_assignment(item_id, id, agenda)?;
        self.mode = Mode::Normal;
        self.clear_category_direct_edit_session();
        self.refresh(agenda.store())?;
        let cat_name = self
            .categories
            .iter()
            .find(|c| c.id == id)
            .map(|c| c.name.as_str())
            .unwrap_or("?");
        self.status = format!("Assigned '{}'", cat_name);
        Ok(())
    }

    fn exact_current_column_child_match_id(&self) -> Option<CategoryId> {
        let target_name = self.active_category_direct_edit_input_text()?.trim();
        if target_name.is_empty() {
            return None;
        }
        self.get_current_column_child_ids().into_iter().find(|id| {
            self.categories
                .iter()
                .find(|c| c.id == *id)
                .map(|c| c.name.eq_ignore_ascii_case(target_name))
                .unwrap_or(false)
        })
    }

    fn confirm_inline_create_category_direct_edit(
        &mut self,
        agenda: &Agenda<'_>,
    ) -> Result<(), String> {
        let Some(name) = self.category_direct_edit_create_confirm.clone() else {
            return Ok(());
        };
        let child_parent_id = self.current_view().and_then(|view| {
            self.current_slot().and_then(|slot| {
                let columns = match slot.context {
                    SlotContext::Section { section_index }
                    | SlotContext::GeneratedSection { section_index, .. } => {
                        view.sections.get(section_index).map(|s| &s.columns)
                    }
                    _ => None,
                }?;
                if self.column_index == 0 || self.column_index > columns.len() {
                    return None;
                }
                Some(columns[self.column_index - 1].heading)
            })
        });
        let Some(parent_id) = child_parent_id else {
            self.category_direct_edit_create_confirm = None;
            if let Some(state) = self.category_direct_edit_state_mut() {
                state.create_confirm_name = None;
            }
            return Ok(());
        };

        let mut category = Category::new(name.clone());
        category.parent = Some(parent_id);
        category.enable_implicit_string = true;
        let cat_id = category.id;
        agenda
            .create_category(&category)
            .map_err(|e| e.to_string())?;
        if let Some(item_id) = self.selected_item_id() {
            self.replace_column_child_assignment(item_id, cat_id, agenda)?;
        }

        self.category_direct_edit_create_confirm = None;
        if let Some(state) = self.category_direct_edit_state_mut() {
            state.create_confirm_name = None;
        }
        self.mode = Mode::Normal;
        self.clear_category_direct_edit_session();
        self.refresh(agenda.store())?;
        self.status = format!("Created and assigned '{}'", name);
        Ok(())
    }

    fn replace_column_child_assignment(
        &mut self,
        item_id: ItemId,
        target_id: CategoryId,
        agenda: &Agenda<'_>,
    ) -> Result<(), String> {
        let child_id_set: HashSet<CategoryId> =
            self.get_current_column_child_ids().into_iter().collect();
        if child_id_set.is_empty() {
            return Ok(());
        }

        if let Some(item) = self.selected_item() {
            let to_remove: Vec<CategoryId> = item
                .assignments
                .keys()
                .filter(|id| child_id_set.contains(id) && **id != target_id)
                .cloned()
                .collect();
            for id in to_remove {
                agenda
                    .unassign_item_manual(item_id, id)
                    .map_err(|e| e.to_string())?;
            }
        }

        agenda
            .assign_item_manual(item_id, target_id, Some("manual:tui.direct_edit".into()))
            .map_err(|e| e.to_string())?;
        Ok(())
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
            KeyCode::Char('n') => {
                self.open_input_panel_add_item();
            }
            KeyCode::Char('e') => {
                self.open_input_panel_edit_item();
            }
            KeyCode::Enter => {
                if self.column_index > 0 {
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
                self.jump_to_all_items_view(agenda)?;
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

    pub(crate) fn handle_category_direct_edit_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> Result<bool, String> {
        if self.category_direct_edit_create_confirm.is_some() {
            match code {
                KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                    self.confirm_inline_create_category_direct_edit(agenda)?;
                    return Ok(false);
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    self.category_direct_edit_create_confirm = None;
                    if let Some(state) = self.category_direct_edit_state_mut() {
                        state.create_confirm_name = None;
                    }
                    self.update_suggestions();
                    self.status = "Create canceled. Continue editing category.".to_string();
                    return Ok(false);
                }
                _ => {
                    self.category_direct_edit_create_confirm = None;
                    if let Some(state) = self.category_direct_edit_state_mut() {
                        state.create_confirm_name = None;
                    }
                }
            }
        }

        match code {
            KeyCode::Up => {
                self.move_suggest_cursor(-1);
                return Ok(false);
            }
            KeyCode::Down => {
                self.move_suggest_cursor(1);
                return Ok(false);
            }
            KeyCode::Tab => {
                self.autocomplete_from_suggestion();
                return Ok(false);
            }
            KeyCode::Enter => {
                let active_text = self
                    .active_category_direct_edit_input_text()
                    .unwrap_or("")
                    .to_string();
                if active_text.trim().is_empty() {
                    let text = active_text;
                    self.commit_category_direct_edit(&text, agenda)?;
                    self.category_suggest = None;
                } else if let Some(category_id) = self.exact_current_column_child_match_id() {
                    let Some(item_id) = self.selected_item_id() else {
                        return Ok(false);
                    };
                    self.replace_column_child_assignment(item_id, category_id, agenda)?;
                    self.mode = Mode::Normal;
                    self.clear_category_direct_edit_session();
                    self.refresh(agenda.store())?;
                    let cat_name = self
                        .categories
                        .iter()
                        .find(|c| c.id == category_id)
                        .map(|c| c.name.as_str())
                        .unwrap_or("?");
                    self.status = format!("Assigned '{}'", cat_name);
                } else if !self.get_current_suggest_matches().is_empty() {
                    self.assign_selected_suggestion(agenda)?;
                } else {
                    let text = active_text;
                    self.commit_category_direct_edit(&text, agenda)?;
                    self.category_suggest = None;
                }
                return Ok(false);
            }
            _ => {}
        }

        match code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.status = "Cancelled".to_string();
                self.clear_input();
                self.clear_category_direct_edit_session();
            }
            _ => {
                if let Some(row) = self.active_category_direct_edit_row_mut() {
                    row.input.handle_key(code, false);
                    row.category_id = None;
                }
                self.update_suggestions();
            }
        }
        Ok(false)
    }

    fn commit_category_direct_edit(
        &mut self,
        text: &str,
        agenda: &Agenda<'_>,
    ) -> Result<bool, String> {
        let Some(slot) = self.current_slot() else {
            return Ok(false);
        };
        let Some(item) = self.selected_item() else {
            return Ok(false);
        };
        let Some(view) = self.current_view() else {
            return Ok(false);
        };
        let columns = match slot.context {
            SlotContext::Section { section_index }
            | SlotContext::GeneratedSection { section_index, .. } => {
                view.sections.get(section_index).map(|s| &s.columns)
            }
            _ => None,
        };
        let Some(columns) = columns else {
            return Ok(false);
        };
        if self.column_index == 0 || self.column_index > columns.len() {
            return Ok(false);
        }
        let column = &columns[self.column_index - 1];
        let child_ids: Vec<CategoryId> = self
            .categories
            .iter()
            .find(|c| c.id == column.heading)
            .map(|c| c.children.clone())
            .unwrap_or_default();

        if text.trim().is_empty() {
            let child_id_set: HashSet<CategoryId> = child_ids.iter().cloned().collect();
            let to_remove: Vec<CategoryId> = item
                .assignments
                .keys()
                .filter(|id| child_id_set.contains(id))
                .cloned()
                .collect();

            let item_id = item.id;
            for id in to_remove {
                agenda
                    .unassign_item_manual(item_id, id)
                    .map_err(|e| e.to_string())?;
            }
            self.mode = Mode::Normal;
            self.category_direct_edit = None;
            self.status = "Cleared category".to_string();
            self.refresh(agenda.store())?;
            return Ok(false);
        }

        let item_id = item.id;
        let target_name = text.trim();
        let existing = child_ids.iter().find(|&id| {
            self.categories
                .iter()
                .find(|c| c.id == *id)
                .map(|c| c.name.eq_ignore_ascii_case(target_name))
                .unwrap_or(false)
        });

        if let Some(category_id) = existing {
            self.replace_column_child_assignment(item_id, *category_id, agenda)?;
            self.mode = Mode::Normal;
            self.category_direct_edit = None;
            self.status = format!("Assigned '{}'", target_name);
            self.refresh(agenda.store())?;
        } else if is_reserved_category_name(target_name) {
            self.mode = Mode::Normal;
            self.category_direct_edit = None;
            self.status = format!(
                "Cannot create reserved category '{}'. Use a different name.",
                target_name
            );
        } else {
            // Check if category already exists elsewhere in the system
            let existing_elsewhere = self
                .categories
                .iter()
                .find(|c| c.name.eq_ignore_ascii_case(target_name));
            if let Some(existing_cat) = existing_elsewhere {
                let parent_name = existing_cat
                    .parent
                    .and_then(|pid| self.categories.iter().find(|c| c.id == pid))
                    .map(|c| c.name.as_str())
                    .unwrap_or("(root)");
                self.mode = Mode::Normal;
                self.category_direct_edit = None;
                self.status = format!(
                    "Category '{}' exists under '{}'. Cannot create duplicate.",
                    target_name, parent_name
                );
            } else {
                self.category_direct_edit_create_confirm = Some(target_name.to_string());
                if let Some(state) = self.category_direct_edit_state_mut() {
                    state.create_confirm_name = Some(target_name.to_string());
                }
                self.status = format!(
                    "Create new category '{}' in this column? (Y/n)",
                    target_name
                );
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
            on_insert_assign: HashSet::new(),
            on_remove_unassign: HashSet::new(),
            show_children: false,
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
            on_insert_assign: HashSet::new(),
            on_remove_unassign: HashSet::new(),
            show_children: false,
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
            on_insert_assign: HashSet::new(),
            on_remove_unassign: HashSet::new(),
            show_children: false,
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
