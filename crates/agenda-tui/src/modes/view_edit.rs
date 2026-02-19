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
            KeyCode::Char('N') => {
                self.mode = Mode::ViewCreateNameInput;
                self.clear_input();
                self.view_pending_name = None;
                self.view_pending_edit_name = None;
                self.view_return_to_manager = false;
                self.status = "Create view: type name and press Enter".to_string();
            }
            KeyCode::Char('r') => {
                if let Some(view) = self.views.get(self.picker_index).cloned() {
                    self.mode = Mode::ViewRenameInput;
                    self.set_input(view.name.clone());
                    self.view_pending_edit_name = Some(view.name.clone());
                    self.view_return_to_manager = false;
                    self.status = format!("Rename view {}: type name and Enter", view.name);
                } else {
                    self.status = "No selected view to rename".to_string();
                }
            }
            KeyCode::Char('e') | KeyCode::Char('E') => {
                if let Some(view) = self.views.get(self.picker_index).cloned() {
                    self.open_view_editor(view);
                    self.status =
                        "View editor: j/k select row, o/right open, Enter save, Esc cancel"
                            .to_string();
                } else {
                    self.status = "No selected view to edit".to_string();
                }
            }
            KeyCode::Char('V') => {
                if self.views.is_empty() {
                    self.status = "No views available".to_string();
                } else {
                    self.mode = Mode::ViewManagerScreen;
                    self.view_return_to_manager = false;
                    self.view_manager_pane = ViewManagerPane::Views;
                    self.view_manager_section_index = 0;
                    self.load_view_manager_rows_from_selected_view();
                    self.status =
                        "View manager: Tab pane, j/k row, Enter action, s save, q/Esc back"
                            .to_string();
                }
            }
            KeyCode::Char('x') => {
                if let Some(view) = self.views.get(self.picker_index) {
                    self.mode = Mode::ViewDeleteConfirm;
                    self.view_return_to_manager = false;
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

    pub(crate) fn handle_view_manager_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> Result<bool, String> {
        // Width input mode intercept
        if self.view_manager_column_width_input {
            match code {
                KeyCode::Esc => {
                    self.view_manager_column_width_input = false;
                    self.clear_input();
                    self.status = "Width input canceled".to_string();
                    return Ok(false);
                }
                KeyCode::Backspace => {
                    self.input.handle_key(KeyCode::Backspace, false);
                    return Ok(false);
                }
                KeyCode::Char(c) if c.is_ascii_digit() => {
                    self.input.handle_key(KeyCode::Char(c), false);
                    return Ok(false);
                }
                KeyCode::Enter => {
                    // Fall through to Enter handler below
                }
                _ => return Ok(false),
            }
        }
        match code {
            KeyCode::Esc | KeyCode::Char('q') => {
                let selected_name = self
                    .views
                    .get(self.picker_index)
                    .map(|view| view.name.clone());
                let had_unsaved = self.view_manager_dirty;
                self.refresh(agenda.store())?;
                if let Some(name) = selected_name {
                    self.set_view_selection_by_name(&name);
                    self.picker_index = self.view_index.min(self.views.len().saturating_sub(1));
                }
                self.mode = Mode::ViewPicker;
                self.status = if had_unsaved {
                    "Closed view manager (unsaved changes discarded)".to_string()
                } else {
                    "Closed view manager".to_string()
                };
            }
            KeyCode::Tab => {
                self.view_manager_pane = match self.view_manager_pane {
                    ViewManagerPane::Views => ViewManagerPane::Definition,
                    ViewManagerPane::Definition => ViewManagerPane::Sections,
                    ViewManagerPane::Sections => ViewManagerPane::Views,
                };
            }
            KeyCode::BackTab => {
                self.view_manager_pane = match self.view_manager_pane {
                    ViewManagerPane::Views => ViewManagerPane::Sections,
                    ViewManagerPane::Definition => ViewManagerPane::Views,
                    ViewManagerPane::Sections => ViewManagerPane::Definition,
                };
            }
            KeyCode::Down | KeyCode::Char('j') => match self.view_manager_pane {
                ViewManagerPane::Views => {
                    if !self.views.is_empty() {
                        let next = next_index_clamped(self.picker_index, self.views.len(), 1);
                        if self.view_manager_dirty
                            && self
                                .view_manager_loaded_view_name
                                .as_ref()
                                .map(|name| {
                                    self.views
                                        .get(next)
                                        .map(|view| !view.name.eq_ignore_ascii_case(name))
                                        .unwrap_or(false)
                                })
                                .unwrap_or(false)
                        {
                            self.status =
                                "Unsaved manager changes. Press s to save before switching view."
                                    .to_string();
                        } else {
                            self.picker_index = next;
                            self.load_view_manager_rows_from_selected_view();
                        }
                    }
                }
                ViewManagerPane::Definition => {
                    if self.view_manager_definition_sub_tab == DefinitionSubTab::Columns {
                        let count = self
                            .views
                            .get(self.picker_index)
                            .map(|v| v.columns.len())
                            .unwrap_or(0)
                            .max(1);
                        self.view_manager_column_index =
                            next_index_clamped(self.view_manager_column_index, count, 1);
                    } else {
                        let count = self.view_manager_rows.len().max(1);
                        self.view_manager_definition_index =
                            next_index_clamped(self.view_manager_definition_index, count, 1);
                    }
                }
                ViewManagerPane::Sections => {
                    let section_count = self
                        .views
                        .get(self.picker_index)
                        .map(|view| view.sections.len().max(1))
                        .unwrap_or(1);
                    self.view_manager_section_index =
                        next_index_clamped(self.view_manager_section_index, section_count, 1);
                }
            },
            KeyCode::Up | KeyCode::Char('k') => match self.view_manager_pane {
                ViewManagerPane::Views => {
                    if !self.views.is_empty() {
                        let next = next_index_clamped(self.picker_index, self.views.len(), -1);
                        if self.view_manager_dirty
                            && self
                                .view_manager_loaded_view_name
                                .as_ref()
                                .map(|name| {
                                    self.views
                                        .get(next)
                                        .map(|view| !view.name.eq_ignore_ascii_case(name))
                                        .unwrap_or(false)
                                })
                                .unwrap_or(false)
                        {
                            self.status =
                                "Unsaved manager changes. Press s to save before switching view."
                                    .to_string();
                        } else {
                            self.picker_index = next;
                            self.load_view_manager_rows_from_selected_view();
                        }
                    }
                }
                ViewManagerPane::Definition => {
                    if self.view_manager_definition_sub_tab == DefinitionSubTab::Columns {
                        let count = self
                            .views
                            .get(self.picker_index)
                            .map(|v| v.columns.len())
                            .unwrap_or(0)
                            .max(1);
                        self.view_manager_column_index =
                            next_index_clamped(self.view_manager_column_index, count, -1);
                    } else {
                        let count = self.view_manager_rows.len().max(1);
                        self.view_manager_definition_index =
                            next_index_clamped(self.view_manager_definition_index, count, -1);
                    }
                }
                ViewManagerPane::Sections => {
                    let section_count = self
                        .views
                        .get(self.picker_index)
                        .map(|view| view.sections.len().max(1))
                        .unwrap_or(1);
                    self.view_manager_section_index =
                        next_index_clamped(self.view_manager_section_index, section_count, -1);
                }
            },
            KeyCode::Enter => {
                if self.view_manager_column_width_input {
                    if let Ok(w) = self.input.trimmed().parse::<u16>() {
                        let w = w.max(4);
                        if let Some(view) = self.views.get_mut(self.picker_index) {
                            if let Some(col) = view.columns.get_mut(self.view_manager_column_index)
                            {
                                col.width = w;
                                self.view_manager_dirty = true;
                                self.status = format!("Column width set to {w}");
                            }
                        }
                    } else {
                        self.status = "Invalid width number".to_string();
                    }
                    self.view_manager_column_width_input = false;
                    self.clear_input();
                } else if self.view_manager_pane == ViewManagerPane::Views {
                    if !self.views.is_empty() {
                        self.view_index = self.picker_index.min(self.views.len() - 1);
                        self.slot_index = 0;
                        self.item_index = 0;
                        self.refresh(agenda.store())?;
                        let view_name = self
                            .current_view()
                            .map(|view| view.name.clone())
                            .unwrap_or_else(|| "(none)".to_string());
                        self.status = format!("Focused view in manager: {view_name}");
                        self.load_view_manager_rows_from_selected_view();
                    }
                } else if self.view_manager_pane == ViewManagerPane::Definition {
                    if self.view_manager_definition_sub_tab == DefinitionSubTab::Columns {
                        // Change heading of selected column via picker
                        let col_count = self
                            .views
                            .get(self.picker_index)
                            .map(|v| v.columns.len())
                            .unwrap_or(0);
                        if col_count > 0 && !self.category_rows.is_empty() {
                            self.view_manager_column_picker_target = true;
                            self.view_manager_category_row_index =
                                Some(self.view_manager_column_index);
                            self.view_category_index = 0;
                            self.mode = Mode::ViewManagerCategoryPicker;
                            self.status =
                                "Pick new heading category: j/k move, Enter choose".to_string();
                        }
                    } else if let Some(row) = self
                        .view_manager_rows
                        .get_mut(self.view_manager_definition_index)
                    {
                        row.sign = match row.sign {
                            ViewCriteriaSign::Include => ViewCriteriaSign::Exclude,
                            ViewCriteriaSign::Exclude => ViewCriteriaSign::Include,
                        };
                        self.view_manager_dirty = true;
                        self.refresh_view_manager_preview();
                    }
                } else {
                    self.open_view_manager_section_detail();
                }
            }
            KeyCode::Char('s') => {
                let Some(view) = self.views.get(self.picker_index).cloned() else {
                    self.status = "No selected view to save".to_string();
                    return Ok(false);
                };
                let validation_errors = self.view_manager_representability_errors();
                if !validation_errors.is_empty() {
                    self.status = format!("Cannot save criteria: {}", validation_errors[0]);
                    return Ok(false);
                }
                let mut updated = view.clone();
                updated.criteria = self.view_manager_query_from_rows(&view);
                match agenda.store().update_view(&updated) {
                    Ok(()) => {
                        self.refresh(agenda.store())?;
                        self.set_view_selection_by_name(&updated.name);
                        self.load_view_manager_rows_from_selected_view();
                        self.status = format!(
                            "Saved criteria for {} (matching={})",
                            updated.name, self.view_manager_preview_count
                        );
                    }
                    Err(err) => {
                        self.status = format!("View manager save failed: {err}");
                    }
                }
            }
            KeyCode::Char('N') => match self.view_manager_pane {
                ViewManagerPane::Views => {
                    self.mode = Mode::ViewCreateNameInput;
                    self.clear_input();
                    self.view_pending_name = None;
                    self.view_pending_edit_name = None;
                    self.view_return_to_manager = true;
                    self.status = "Create view: type name and press Enter".to_string();
                }
                ViewManagerPane::Definition => {
                    if self.view_manager_definition_sub_tab == DefinitionSubTab::Columns {
                        if self.category_rows.is_empty() {
                            self.status = "No categories available".to_string();
                            return Ok(false);
                        }
                        self.view_manager_column_picker_target = true;
                        self.view_manager_category_row_index = None;
                        self.view_category_index = 0;
                        self.mode = Mode::ViewManagerCategoryPicker;
                        self.status =
                            "Pick heading category for new column: j/k move, Enter choose"
                                .to_string();
                    } else {
                        let Some(category_row) = self
                            .category_rows
                            .iter()
                            .find(|row| !row.is_reserved)
                            .cloned()
                        else {
                            self.status =
                                "No user categories available for criteria rows".to_string();
                            return Ok(false);
                        };
                        self.view_manager_rows.push(ViewCriteriaRow {
                            sign: ViewCriteriaSign::Include,
                            category_id: category_row.id,
                            join_is_or: false,
                            depth: 0,
                        });
                        self.view_manager_definition_index =
                            self.view_manager_rows.len().saturating_sub(1);
                        self.view_manager_dirty = true;
                        self.refresh_view_manager_preview();
                        self.status = format!("Added criteria row for {}", category_row.name);
                    }
                }
                ViewManagerPane::Sections => {
                    let Some(view) = self.views.get_mut(self.picker_index) else {
                        self.status = "No selected view for section add".to_string();
                        return Ok(false);
                    };
                    let next = view.sections.len() + 1;
                    view.sections.push(Section {
                        title: format!("Section {next}"),
                        criteria: Query::default(),
                        on_insert_assign: HashSet::new(),
                        on_remove_unassign: HashSet::new(),
                        show_children: false,
                    });
                    self.view_manager_section_index = view.sections.len().saturating_sub(1);
                    self.view_manager_dirty = true;
                    self.status = format!("Added Section {next}");
                }
            },
            KeyCode::Char('r') => {
                if self.view_manager_pane == ViewManagerPane::Views {
                    if let Some(view) = self.views.get(self.picker_index).cloned() {
                        self.mode = Mode::ViewRenameInput;
                        self.set_input(view.name.clone());
                        self.view_pending_edit_name = Some(view.name.clone());
                        self.view_return_to_manager = true;
                        self.status = format!("Rename view {}: type name and Enter", view.name);
                    } else {
                        self.status = "No selected view to rename".to_string();
                    }
                }
            }
            KeyCode::Char('x') => match self.view_manager_pane {
                ViewManagerPane::Views => {
                    if let Some(view) = self.views.get(self.picker_index) {
                        self.mode = Mode::ViewDeleteConfirm;
                        self.view_return_to_manager = true;
                        self.status = format!("Delete view '{}' ? y/n", view.name);
                    } else {
                        self.status = "No selected view to delete".to_string();
                    }
                }
                ViewManagerPane::Definition => {
                    if self.view_manager_definition_sub_tab == DefinitionSubTab::Columns {
                        let Some(view) = self.views.get_mut(self.picker_index) else {
                            self.status = "No selected view".to_string();
                            return Ok(false);
                        };
                        if view.columns.is_empty() {
                            self.status = "No column to remove".to_string();
                            return Ok(false);
                        }
                        let idx = self
                            .view_manager_column_index
                            .min(view.columns.len().saturating_sub(1));
                        view.columns.remove(idx);
                        self.view_manager_column_index = self
                            .view_manager_column_index
                            .min(view.columns.len().saturating_sub(1));
                        self.view_manager_dirty = true;
                        self.status = "Removed column".to_string();
                    } else {
                        if self.view_manager_rows.is_empty() {
                            self.status = "No criteria row to remove".to_string();
                            return Ok(false);
                        }
                        let removed = self.view_manager_rows.remove(
                            self.view_manager_definition_index
                                .min(self.view_manager_rows.len().saturating_sub(1)),
                        );
                        self.view_manager_definition_index = self
                            .view_manager_definition_index
                            .min(self.view_manager_rows.len().saturating_sub(1));
                        self.view_manager_dirty = true;
                        self.refresh_view_manager_preview();
                        let category_name = self
                            .category_rows
                            .iter()
                            .find(|row| row.id == removed.category_id)
                            .map(|row| row.name.clone())
                            .unwrap_or_else(|| removed.category_id.to_string());
                        self.status = format!("Removed criteria row {}", category_name);
                    }
                }
                ViewManagerPane::Sections => {
                    let Some(view) = self.views.get_mut(self.picker_index) else {
                        self.status = "No selected view for section remove".to_string();
                        return Ok(false);
                    };
                    if view.sections.is_empty() {
                        self.status = "No section to remove".to_string();
                        return Ok(false);
                    }
                    let remove_index = self
                        .view_manager_section_index
                        .min(view.sections.len().saturating_sub(1));
                    let removed = view.sections.remove(remove_index);
                    self.view_manager_section_index = self
                        .view_manager_section_index
                        .min(view.sections.len().saturating_sub(1));
                    self.view_manager_dirty = true;
                    self.status = format!("Removed section {}", removed.title);
                }
            },
            KeyCode::Char('[') => {
                if self.view_manager_pane == ViewManagerPane::Definition
                    && self.view_manager_definition_sub_tab == DefinitionSubTab::Columns
                {
                    let Some(view) = self.views.get_mut(self.picker_index) else {
                        return Ok(false);
                    };
                    if view.columns.len() < 2 {
                        return Ok(false);
                    }
                    let current = self
                        .view_manager_column_index
                        .min(view.columns.len().saturating_sub(1));
                    if current == 0 {
                        return Ok(false);
                    }
                    view.columns.swap(current, current - 1);
                    self.view_manager_column_index = current - 1;
                    self.view_manager_dirty = true;
                    self.status = "Moved column up".to_string();
                } else if self.view_manager_pane == ViewManagerPane::Sections {
                    let Some(view) = self.views.get_mut(self.picker_index) else {
                        self.status = "No selected view for section reorder".to_string();
                        return Ok(false);
                    };
                    if view.sections.len() < 2 {
                        self.status = "Need at least two sections to reorder".to_string();
                        return Ok(false);
                    }
                    let current = self
                        .view_manager_section_index
                        .min(view.sections.len().saturating_sub(1));
                    if current == 0 {
                        return Ok(false);
                    }
                    let target = current - 1;
                    view.sections.swap(current, target);
                    self.view_manager_section_index = target;
                    self.view_manager_dirty = true;
                    self.status = "Moved section up".to_string();
                }
            }
            KeyCode::Char(']') => {
                if self.view_manager_pane == ViewManagerPane::Definition
                    && self.view_manager_definition_sub_tab == DefinitionSubTab::Columns
                {
                    let Some(view) = self.views.get_mut(self.picker_index) else {
                        return Ok(false);
                    };
                    if view.columns.len() < 2 {
                        return Ok(false);
                    }
                    let current = self
                        .view_manager_column_index
                        .min(view.columns.len().saturating_sub(1));
                    if current + 1 >= view.columns.len() {
                        return Ok(false);
                    }
                    view.columns.swap(current, current + 1);
                    self.view_manager_column_index = current + 1;
                    self.view_manager_dirty = true;
                    self.status = "Moved column down".to_string();
                } else if self.view_manager_pane == ViewManagerPane::Sections {
                    let Some(view) = self.views.get_mut(self.picker_index) else {
                        self.status = "No selected view for section reorder".to_string();
                        return Ok(false);
                    };
                    if view.sections.len() < 2 {
                        self.status = "Need at least two sections to reorder".to_string();
                        return Ok(false);
                    }
                    let current = self
                        .view_manager_section_index
                        .min(view.sections.len().saturating_sub(1));
                    if current + 1 >= view.sections.len() {
                        return Ok(false);
                    }
                    let target = current + 1;
                    view.sections.swap(current, target);
                    self.view_manager_section_index = target;
                    self.view_manager_dirty = true;
                    self.status = "Moved section down".to_string();
                }
            }
            KeyCode::Char(' ') => {
                if self.view_manager_pane == ViewManagerPane::Definition {
                    if let Some(row) = self
                        .view_manager_rows
                        .get_mut(self.view_manager_definition_index)
                    {
                        row.sign = match row.sign {
                            ViewCriteriaSign::Include => ViewCriteriaSign::Exclude,
                            ViewCriteriaSign::Exclude => ViewCriteriaSign::Include,
                        };
                        self.view_manager_dirty = true;
                        self.refresh_view_manager_preview();
                    }
                }
            }
            KeyCode::Char('t') => {
                if self.view_manager_pane == ViewManagerPane::Definition {
                    self.view_manager_definition_sub_tab =
                        match self.view_manager_definition_sub_tab {
                            DefinitionSubTab::Criteria => DefinitionSubTab::Columns,
                            DefinitionSubTab::Columns => DefinitionSubTab::Criteria,
                        };
                    self.status = format!(
                        "Definition sub-tab: {:?}",
                        self.view_manager_definition_sub_tab
                    );
                }
            }
            KeyCode::Char('w') => {
                if self.view_manager_pane == ViewManagerPane::Definition
                    && self.view_manager_definition_sub_tab == DefinitionSubTab::Columns
                {
                    let col_count = self
                        .views
                        .get(self.picker_index)
                        .map(|v| v.columns.len())
                        .unwrap_or(0);
                    if col_count > 0 {
                        self.view_manager_column_width_input = true;
                        let current_width = self
                            .views
                            .get(self.picker_index)
                            .and_then(|v| v.columns.get(self.view_manager_column_index))
                            .map(|c| c.width)
                            .unwrap_or(20);
                        self.set_input(current_width.to_string());
                        self.status = "Type width and press Enter".to_string();
                    }
                }
            }
            KeyCode::Char('a') => {
                if self.view_manager_pane == ViewManagerPane::Definition {
                    if let Some(row) = self
                        .view_manager_rows
                        .get_mut(self.view_manager_definition_index)
                    {
                        row.join_is_or = false;
                        self.view_manager_dirty = true;
                    }
                }
            }
            KeyCode::Char('o') => {
                if self.view_manager_pane == ViewManagerPane::Definition {
                    if let Some(row) = self
                        .view_manager_rows
                        .get_mut(self.view_manager_definition_index)
                    {
                        row.join_is_or = true;
                        self.view_manager_dirty = true;
                    }
                }
            }
            KeyCode::Char('(') => {
                if self.view_manager_pane == ViewManagerPane::Definition {
                    if let Some(row) = self
                        .view_manager_rows
                        .get_mut(self.view_manager_definition_index)
                    {
                        row.depth = row.depth.saturating_add(1).min(8);
                        self.view_manager_dirty = true;
                    }
                }
            }
            KeyCode::Char(')') => {
                if self.view_manager_pane == ViewManagerPane::Definition {
                    if let Some(row) = self
                        .view_manager_rows
                        .get_mut(self.view_manager_definition_index)
                    {
                        row.depth = row.depth.saturating_sub(1);
                        self.view_manager_dirty = true;
                    }
                }
            }
            KeyCode::Char('c') => {
                if self.view_manager_pane == ViewManagerPane::Definition {
                    let Some(row) = self
                        .view_manager_rows
                        .get(self.view_manager_definition_index)
                    else {
                        self.status = "No criteria row selected".to_string();
                        return Ok(false);
                    };
                    let Some(index) = self
                        .category_rows
                        .iter()
                        .position(|category| category.id == row.category_id)
                    else {
                        self.status = "Current row category is missing".to_string();
                        return Ok(false);
                    };
                    if self.category_rows.is_empty() {
                        self.status = "No user categories available".to_string();
                        return Ok(false);
                    }
                    self.view_manager_category_row_index = Some(self.view_manager_definition_index);
                    self.view_category_index = index;
                    self.mode = Mode::ViewManagerCategoryPicker;
                    self.status = "Pick category: j/k move, Enter choose, Esc cancel".to_string();
                }
            }
            KeyCode::Char('u') => {
                if self.view_manager_pane == ViewManagerPane::Sections {
                    self.open_view_manager_unmatched_settings();
                }
            }
            KeyCode::Char('C') => {
                if self.view_manager_pane == ViewManagerPane::Views {
                    let Some(view) = self.views.get(self.picker_index).cloned() else {
                        self.status = "No selected view to clone".to_string();
                        return Ok(false);
                    };

                    let clone_name = self.next_view_clone_name(&view.name);
                    let mut clone = View::new(clone_name.clone());
                    clone.criteria = view.criteria.clone();
                    clone.sections = view.sections.clone();
                    clone.columns = view.columns.clone();
                    clone.show_unmatched = view.show_unmatched;
                    clone.unmatched_label = view.unmatched_label.clone();
                    clone.remove_from_view_unassign = view.remove_from_view_unassign.clone();
                    clone.item_column_label = view.item_column_label.clone();
                    match agenda.store().create_view(&clone) {
                        Ok(()) => {
                            self.refresh(agenda.store())?;
                            self.set_view_selection_by_name(&clone_name);
                            self.mode = Mode::ViewManagerScreen;
                            self.load_view_manager_rows_from_selected_view();
                            self.status = format!("Cloned view as {clone_name}");
                        }
                        Err(err) => {
                            self.status = format!("View clone failed: {err}");
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(false)
    }

    pub(crate) fn next_view_clone_name(&self, base_name: &str) -> String {
        let mut candidate = format!("{base_name} Copy");
        let mut counter = 2usize;
        while self
            .views
            .iter()
            .any(|view| view.name.eq_ignore_ascii_case(&candidate))
        {
            candidate = format!("{base_name} Copy {counter}");
            counter += 1;
        }
        candidate
    }

    pub(crate) fn handle_view_manager_category_picker_key(
        &mut self,
        code: KeyCode,
    ) -> Result<bool, String> {
        match code {
            KeyCode::Esc => {
                self.mode = Mode::ViewManagerScreen;
                self.view_manager_category_row_index = None;
                self.view_manager_column_picker_target = false;
                self.status = "Category pick canceled".to_string();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if !self.category_rows.is_empty() {
                    self.view_category_index =
                        next_index_clamped(self.view_category_index, self.category_rows.len(), 1);
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if !self.category_rows.is_empty() {
                    self.view_category_index =
                        next_index_clamped(self.view_category_index, self.category_rows.len(), -1);
                }
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                let Some(selected_row) = self.category_rows.get(self.view_category_index) else {
                    self.status = "Category pick failed: no category selected".to_string();
                    return Ok(false);
                };
                let selected_category_id = selected_row.id;
                let selected_category_name = selected_row.name.clone();
                let is_reserved = selected_row.is_reserved;

                if self.view_manager_column_picker_target {
                    // Column picker: adding or editing a column heading
                    let is_when = selected_category_name.eq_ignore_ascii_case("When");
                    let kind = if is_when {
                        ColumnKind::When
                    } else {
                        ColumnKind::Standard
                    };
                    if let Some(target_index) = self.view_manager_category_row_index {
                        // Editing existing column heading
                        if let Some(view) = self.views.get_mut(self.picker_index) {
                            if let Some(col) = view.columns.get_mut(target_index) {
                                col.heading = selected_category_id;
                                col.kind = kind;
                                self.view_manager_dirty = true;
                                self.status =
                                    format!("Changed column heading to {}", selected_category_name);
                            }
                        }
                    } else {
                        // Adding new column
                        if let Some(view) = self.views.get_mut(self.picker_index) {
                            view.columns.push(Column {
                                kind,
                                heading: selected_category_id,
                                width: 20,
                            });
                            self.view_manager_column_index = view.columns.len().saturating_sub(1);
                            self.view_manager_dirty = true;
                            self.status = format!("Added column: {}", selected_category_name);
                        }
                    }
                    self.view_manager_column_picker_target = false;
                    self.view_manager_category_row_index = None;
                    self.mode = Mode::ViewManagerScreen;
                } else {
                    // Criteria row picker (original behavior)
                    let Some(target_row_index) = self.view_manager_category_row_index else {
                        self.mode = Mode::ViewManagerScreen;
                        self.status = "Category pick failed: no target row".to_string();
                        return Ok(false);
                    };
                    if is_reserved {
                        self.status =
                            "Reserved categories cannot be used in criteria rows".to_string();
                        return Ok(false);
                    }
                    if let Some(target_row) = self.view_manager_rows.get_mut(target_row_index) {
                        target_row.category_id = selected_category_id;
                        self.view_manager_dirty = true;
                        self.refresh_view_manager_preview();
                        self.status =
                            format!("Set criteria row category to {}", selected_category_name);
                    }
                    self.view_manager_category_row_index = None;
                    self.mode = Mode::ViewManagerScreen;
                }
            }
            _ => {}
        }
        Ok(false)
    }

    pub(crate) fn load_view_manager_rows_from_selected_view(&mut self) {
        let Some(view) = self.views.get(self.picker_index) else {
            self.view_manager_rows.clear();
            self.view_manager_loaded_view_name = None;
            self.view_manager_preview_count = 0;
            self.view_manager_definition_index = 0;
            self.view_manager_dirty = false;
            return;
        };

        let category_names = category_name_map(&self.categories);
        let mut rows: Vec<ViewCriteriaRow> = view
            .criteria
            .include
            .iter()
            .map(|category_id| ViewCriteriaRow {
                sign: ViewCriteriaSign::Include,
                category_id: *category_id,
                join_is_or: false,
                depth: 0,
            })
            .chain(
                view.criteria
                    .exclude
                    .iter()
                    .map(|category_id| ViewCriteriaRow {
                        sign: ViewCriteriaSign::Exclude,
                        category_id: *category_id,
                        join_is_or: false,
                        depth: 0,
                    }),
            )
            .collect();
        rows.sort_by(|a, b| {
            let a_name = category_names
                .get(&a.category_id)
                .cloned()
                .unwrap_or_else(|| a.category_id.to_string());
            let b_name = category_names
                .get(&b.category_id)
                .cloned()
                .unwrap_or_else(|| b.category_id.to_string());
            let a_sign = matches!(a.sign, ViewCriteriaSign::Exclude) as u8;
            let b_sign = matches!(b.sign, ViewCriteriaSign::Exclude) as u8;
            (a_sign, a_name.to_ascii_lowercase()).cmp(&(b_sign, b_name.to_ascii_lowercase()))
        });

        self.view_manager_rows = rows;
        self.view_manager_loaded_view_name = Some(view.name.clone());
        self.view_manager_definition_index = 0;
        self.view_manager_column_index = 0;
        self.refresh_view_manager_preview();
        self.view_manager_dirty = false;
    }

    pub(crate) fn view_manager_category_label(&self, category_id: CategoryId) -> String {
        self.category_rows
            .iter()
            .find(|row| row.id == category_id)
            .map(|row| with_note_marker(row.name.clone(), row.has_note))
            .unwrap_or_else(|| category_id.to_string())
    }

    pub(crate) fn view_manager_representability_errors(&self) -> Vec<String> {
        let mut errors = Vec::new();
        let mut seen: HashMap<CategoryId, (ViewCriteriaSign, usize)> = HashMap::new();
        for (index, row) in self.view_manager_rows.iter().enumerate() {
            let label = self.view_manager_category_label(row.category_id);
            if index > 0 && row.join_is_or {
                errors.push(format!(
                    "Row {} ({}) uses OR; current persistence only supports AND.",
                    index + 1,
                    label
                ));
            }
            if row.depth > 0 {
                errors.push(format!(
                    "Row {} ({}) uses nesting depth {}; only depth 0 is persistable.",
                    index + 1,
                    label,
                    row.depth
                ));
            }
            if let Some((prior_sign, prior_index)) = seen.get(&row.category_id).copied() {
                if prior_sign == row.sign {
                    errors.push(format!(
                        "Row {} ({}) duplicates row {}.",
                        index + 1,
                        label,
                        prior_index + 1
                    ));
                } else {
                    errors.push(format!(
                        "Row {} ({}) conflicts with row {} (+/- mismatch).",
                        index + 1,
                        label,
                        prior_index + 1
                    ));
                }
            } else {
                seen.insert(row.category_id, (row.sign, index));
            }
        }
        errors
    }

    pub(crate) fn view_manager_query_from_rows(&self, base_view: &View) -> Query {
        let mut query = base_view.criteria.clone();
        query.include.clear();
        query.exclude.clear();
        for row in &self.view_manager_rows {
            match row.sign {
                ViewCriteriaSign::Include => {
                    query.include.insert(row.category_id);
                    query.exclude.remove(&row.category_id);
                }
                ViewCriteriaSign::Exclude => {
                    query.exclude.insert(row.category_id);
                    query.include.remove(&row.category_id);
                }
            }
        }
        query
    }

    pub(crate) fn refresh_view_manager_preview(&mut self) {
        let Some(view) = self.views.get(self.picker_index) else {
            self.view_manager_preview_count = 0;
            return;
        };
        let query = self.view_manager_query_from_rows(view);
        self.view_manager_preview_count = self.preview_count_for_query(&query);
    }

    pub(crate) fn handle_view_delete_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> Result<bool, String> {
        let return_mode = if self.view_return_to_manager {
            Mode::ViewManagerScreen
        } else {
            Mode::ViewPicker
        };
        match code {
            KeyCode::Char('y') => {
                let Some(view) = self.views.get(self.picker_index).cloned() else {
                    self.mode = return_mode;
                    self.view_return_to_manager = false;
                    self.status = "Delete failed: no selected view".to_string();
                    return Ok(false);
                };
                let deleted_index = self.picker_index.min(self.views.len().saturating_sub(1));
                match agenda.store().delete_view(view.id) {
                    Ok(()) => {
                        if self.view_index > deleted_index {
                            self.view_index -= 1;
                        } else if self.view_index == deleted_index {
                            self.view_index = deleted_index.saturating_sub(1);
                        }
                        self.refresh(agenda.store())?;
                        self.mode = return_mode;
                        self.picker_index =
                            self.picker_index.min(self.views.len().saturating_sub(1));
                        self.view_return_to_manager = false;
                        if self.mode == Mode::ViewManagerScreen {
                            self.load_view_manager_rows_from_selected_view();
                        }
                        self.status = format!("Deleted view: {}", view.name);
                    }
                    Err(err) => {
                        self.mode = return_mode;
                        self.view_return_to_manager = false;
                        self.status = format!("Delete failed: {err}");
                    }
                }
            }
            KeyCode::Char('n') | KeyCode::Esc => {
                self.mode = return_mode;
                self.view_return_to_manager = false;
                if self.mode == Mode::ViewManagerScreen {
                    self.load_view_manager_rows_from_selected_view();
                }
                self.status = "Delete canceled".to_string();
            }
            _ => {}
        }
        Ok(false)
    }

    pub(crate) fn handle_view_create_name_key(&mut self, code: KeyCode) -> Result<bool, String> {
        let return_mode = if self.view_return_to_manager {
            Mode::ViewManagerScreen
        } else {
            Mode::ViewPicker
        };
        match code {
            KeyCode::Esc => {
                self.mode = return_mode;
                self.clear_input();
                self.view_pending_name = None;
                self.view_return_to_manager = false;
                if self.mode == Mode::ViewManagerScreen {
                    self.load_view_manager_rows_from_selected_view();
                }
                self.status = "View create canceled".to_string();
            }
            KeyCode::Enter => {
                let name = self.input.trimmed().to_string();
                if name.is_empty() {
                    self.mode = return_mode;
                    self.clear_input();
                    self.view_pending_name = None;
                    self.view_return_to_manager = false;
                    if self.mode == Mode::ViewManagerScreen {
                        self.load_view_manager_rows_from_selected_view();
                    }
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
                    self.mode = if self.view_return_to_manager {
                        Mode::ViewManagerScreen
                    } else {
                        Mode::ViewPicker
                    };
                    self.view_return_to_manager = false;
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
                        self.refresh(agenda.store())?;
                        self.set_view_selection_by_name(&view.name);
                        self.mode = if self.view_return_to_manager {
                            Mode::ViewManagerScreen
                        } else {
                            Mode::Normal
                        };
                        self.view_pending_name = None;
                        self.view_create_include_selection.clear();
                        self.view_create_exclude_selection.clear();
                        self.view_return_to_manager = false;
                        if self.mode == Mode::ViewManagerScreen {
                            self.load_view_manager_rows_from_selected_view();
                        }
                        self.status = format!(
                            "Created view {} (include={}, exclude={})",
                            view.name,
                            view.criteria.include.len(),
                            view.criteria.exclude.len()
                        );
                    }
                    Err(err) => {
                        self.mode = if self.view_return_to_manager {
                            Mode::ViewManagerScreen
                        } else {
                            Mode::ViewPicker
                        };
                        self.view_create_include_selection.clear();
                        self.view_create_exclude_selection.clear();
                        self.view_return_to_manager = false;
                        if self.mode == Mode::ViewManagerScreen {
                            self.load_view_manager_rows_from_selected_view();
                        }
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
        let return_mode = if self.view_return_to_manager {
            Mode::ViewManagerScreen
        } else {
            Mode::ViewPicker
        };
        match code {
            KeyCode::Esc => {
                self.mode = return_mode;
                self.clear_input();
                self.view_pending_edit_name = None;
                self.view_return_to_manager = false;
                if self.mode == Mode::ViewManagerScreen {
                    self.load_view_manager_rows_from_selected_view();
                }
                self.status = "View rename canceled".to_string();
            }
            KeyCode::Enter => {
                let Some(view_name) = self.view_pending_edit_name.clone() else {
                    self.mode = return_mode;
                    self.clear_input();
                    self.view_return_to_manager = false;
                    self.status = "View rename failed: no selected view".to_string();
                    return Ok(false);
                };

                let new_name = self.input.trimmed().to_string();
                if new_name.is_empty() {
                    self.mode = return_mode;
                    self.clear_input();
                    self.view_pending_edit_name = None;
                    self.view_return_to_manager = false;
                    self.status = "View rename canceled (empty name)".to_string();
                    return Ok(false);
                }

                let Some(mut view) = self
                    .views
                    .iter()
                    .find(|view| view.name.eq_ignore_ascii_case(&view_name))
                    .cloned()
                else {
                    self.mode = return_mode;
                    self.clear_input();
                    self.view_pending_edit_name = None;
                    self.view_return_to_manager = false;
                    self.status = "View rename failed: selected view not found".to_string();
                    return Ok(false);
                };

                if view.name == new_name {
                    self.mode = return_mode;
                    self.clear_input();
                    self.view_pending_edit_name = None;
                    self.view_return_to_manager = false;
                    self.status = "View rename canceled (unchanged)".to_string();
                    return Ok(false);
                }

                view.name = new_name.clone();
                match agenda.store().update_view(&view) {
                    Ok(()) => {
                        self.refresh(agenda.store())?;
                        self.set_view_selection_by_name(&new_name);
                        self.mode = return_mode;
                        self.clear_input();
                        self.view_pending_edit_name = None;
                        self.view_return_to_manager = false;
                        if self.mode == Mode::ViewManagerScreen {
                            self.load_view_manager_rows_from_selected_view();
                        }
                        self.status = format!("Renamed view to {}", new_name);
                    }
                    Err(err) => {
                        self.mode = return_mode;
                        self.clear_input();
                        self.view_pending_edit_name = None;
                        self.view_return_to_manager = false;
                        self.status = format!("View rename failed: {err}");
                    }
                }
            }
            _ if self.handle_text_input_key(code) => {}
            _ => {}
        }
        Ok(false)
    }

    pub(crate) fn open_view_editor(&mut self, view: View) {
        let preview_count = self.preview_count_for_query(&view.criteria);
        self.view_editor = Some(ViewEditorState {
            base_view_name: view.name.clone(),
            draft: view,
            category_index: first_non_reserved_category_index(&self.category_rows),
            bucket_index: 0,
            section_index: 0,
            action_index: 0,
            preview_count,
        });
        self.view_editor_return_to_manager = false;
        self.view_editor_category_target = None;
        self.view_editor_bucket_target = None;
        self.mode = Mode::ViewEditor;
    }

    pub(crate) fn open_view_manager_section_editor(&mut self) {
        let Some(view) = self.views.get(self.picker_index).cloned() else {
            self.status = "No selected view for section editing".to_string();
            return;
        };
        let preview_count = self.preview_count_for_query(&view.criteria);
        self.view_editor = Some(ViewEditorState {
            base_view_name: view.name.clone(),
            draft: view.clone(),
            category_index: first_non_reserved_category_index(&self.category_rows),
            bucket_index: 0,
            section_index: self
                .view_manager_section_index
                .min(view.sections.len().saturating_sub(1)),
            action_index: 4,
            preview_count,
        });
        self.view_editor_return_to_manager = true;
        self.view_editor_category_target = None;
        self.view_editor_bucket_target = None;
        self.mode = Mode::ViewSectionEditor;
        self.status = "Section editor: N/x/[/] and Enter detail, Esc return to manager".to_string();
    }

    pub(crate) fn open_view_manager_section_detail(&mut self) {
        self.open_view_manager_section_editor();
        let has_sections = self
            .view_editor
            .as_ref()
            .map(|editor| !editor.draft.sections.is_empty())
            .unwrap_or(false);
        if has_sections {
            self.mode = Mode::ViewSectionDetail;
            self.status =
                "Section detail: t title, +/- categories, [/] virtual, a insert-set, r remove-set, Esc back".to_string();
        } else {
            self.status = "No sections to edit; press N to add a section".to_string();
        }
    }

    pub(crate) fn open_view_manager_unmatched_settings(&mut self) {
        let Some(view) = self.views.get(self.picker_index).cloned() else {
            self.status = "No selected view for unmatched settings".to_string();
            return;
        };
        let preview_count = self.preview_count_for_query(&view.criteria);
        self.view_editor = Some(ViewEditorState {
            base_view_name: view.name.clone(),
            draft: view,
            category_index: first_non_reserved_category_index(&self.category_rows),
            bucket_index: 0,
            section_index: 0,
            action_index: 5,
            preview_count,
        });
        self.view_editor_return_to_manager = true;
        self.view_editor_category_target = None;
        self.view_editor_bucket_target = None;
        self.mode = Mode::ViewUnmatchedSettings;
        self.status = "Unmatched settings: t toggle, l label, Esc return to manager".to_string();
    }

    pub(crate) fn apply_view_editor_draft_to_selected_view_manager_view(&mut self) {
        if !self.view_editor_return_to_manager {
            return;
        }
        let Some(editor) = &self.view_editor else {
            return;
        };
        let Some(view) = self.views.get_mut(self.picker_index) else {
            return;
        };
        view.sections = editor.draft.sections.clone();
        view.show_unmatched = editor.draft.show_unmatched;
        view.unmatched_label = editor.draft.unmatched_label.clone();
        self.view_manager_section_index = self
            .view_manager_section_index
            .min(view.sections.len().saturating_sub(1));
        self.view_manager_dirty = true;
    }

    pub(crate) fn finish_view_editor_return_to_manager(&mut self, status: &str) {
        self.apply_view_editor_draft_to_selected_view_manager_view();
        self.view_editor = None;
        self.view_editor_return_to_manager = false;
        self.view_editor_category_target = None;
        self.view_editor_bucket_target = None;
        self.mode = Mode::ViewManagerScreen;
        self.status = status.to_string();
    }

    pub(crate) fn preview_count_for_query(&self, query: &Query) -> usize {
        let reference_date = Local::now().date_naive();
        evaluate_query(query, &self.all_items, reference_date).len()
    }

    pub(crate) fn refresh_view_editor_preview(&mut self) {
        if let Some(editor) = &mut self.view_editor {
            let reference_date = Local::now().date_naive();
            editor.preview_count =
                evaluate_query(&editor.draft.criteria, &self.all_items, reference_date).len();
        }
    }

    pub(crate) fn handle_view_editor_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> Result<bool, String> {
        const VIEW_EDITOR_ACTIONS: usize = 6;
        match code {
            KeyCode::Esc => {
                self.mode = Mode::ViewPicker;
                self.view_editor = None;
                self.view_editor_return_to_manager = false;
                self.view_editor_category_target = None;
                self.view_editor_bucket_target = None;
                self.clear_input();
                self.status = "View edit canceled".to_string();
            }
            KeyCode::Enter => {
                let Some(editor) = self.view_editor.clone() else {
                    self.mode = Mode::ViewPicker;
                    self.status = "View edit failed: no draft".to_string();
                    return Ok(false);
                };
                match agenda.store().update_view(&editor.draft) {
                    Ok(()) => {
                        self.refresh(agenda.store())?;
                        self.set_view_selection_by_name(&editor.base_view_name);
                        self.mode = Mode::ViewPicker;
                        self.view_editor = None;
                        self.view_editor_return_to_manager = false;
                        self.view_editor_category_target = None;
                        self.view_editor_bucket_target = None;
                        self.status = format!("Updated view {}", editor.base_view_name);
                    }
                    Err(err) => {
                        self.status = format!("View edit failed: {err}");
                    }
                }
            }
            KeyCode::Right | KeyCode::Char('o') => {
                if let Some(action_index) =
                    self.view_editor.as_ref().map(|editor| editor.action_index)
                {
                    self.activate_view_editor_action(action_index);
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(editor) = &mut self.view_editor {
                    editor.action_index =
                        next_index_clamped(editor.action_index, VIEW_EDITOR_ACTIONS, 1);
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(editor) = &mut self.view_editor {
                    editor.action_index =
                        next_index_clamped(editor.action_index, VIEW_EDITOR_ACTIONS, -1);
                }
            }
            KeyCode::Char('+') => {
                self.open_view_editor_category_picker(CategoryEditTarget::ViewInclude);
            }
            KeyCode::Char('-') => {
                self.open_view_editor_category_picker(CategoryEditTarget::ViewExclude);
            }
            KeyCode::Char(']') => {
                self.open_view_editor_bucket_picker(BucketEditTarget::ViewVirtualInclude);
            }
            KeyCode::Char('[') => {
                self.open_view_editor_bucket_picker(BucketEditTarget::ViewVirtualExclude);
            }
            KeyCode::Char('s') => {
                self.mode = Mode::ViewSectionEditor;
                self.status = "Section editor: j/k select, N add, x remove, [/] reorder, Enter edit, Esc back".to_string();
            }
            KeyCode::Char('u') => {
                self.mode = Mode::ViewUnmatchedSettings;
                self.status = "Unmatched: t toggle visibility, l edit label, Esc back".to_string();
            }
            _ => {}
        }
        Ok(false)
    }

    pub(crate) fn activate_view_editor_action(&mut self, action_index: usize) {
        match action_index {
            0 => self.open_view_editor_category_picker(CategoryEditTarget::ViewInclude),
            1 => self.open_view_editor_category_picker(CategoryEditTarget::ViewExclude),
            2 => self.open_view_editor_bucket_picker(BucketEditTarget::ViewVirtualInclude),
            3 => self.open_view_editor_bucket_picker(BucketEditTarget::ViewVirtualExclude),
            4 => {
                self.mode = Mode::ViewSectionEditor;
                self.status = "Section editor: j/k select, N add, x remove, [/] reorder, Enter edit, Esc back".to_string();
            }
            5 => {
                self.mode = Mode::ViewUnmatchedSettings;
                self.status = "Unmatched: t toggle visibility, l edit label, Esc back".to_string();
            }
            _ => {}
        }
    }

    pub(crate) fn open_view_editor_category_picker(&mut self, target: CategoryEditTarget) {
        if self.category_rows.is_empty() {
            self.status = "No categories available".to_string();
            return;
        }
        self.view_editor_category_target = Some(target);
        self.mode = Mode::ViewEditorCategoryPicker;
        self.status = "Category picker: j/k select, Space toggle, Enter/Esc back".to_string();
    }

    pub(crate) fn open_view_editor_bucket_picker(&mut self, target: BucketEditTarget) {
        self.view_editor_bucket_target = Some(target);
        self.mode = Mode::ViewEditorBucketPicker;
        self.status = "Bucket picker: j/k select, Space toggle, Enter/Esc back".to_string();
    }

    pub(crate) fn handle_view_editor_category_key(
        &mut self,
        code: KeyCode,
    ) -> Result<bool, String> {
        let Some(target) = self.view_editor_category_target else {
            self.mode = Mode::ViewEditor;
            return Ok(false);
        };
        match code {
            KeyCode::Esc | KeyCode::Enter => {
                self.view_editor_category_target = None;
                self.mode = if category_target_is_section(target) {
                    Mode::ViewSectionDetail
                } else {
                    Mode::ViewEditor
                };
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(editor) = &mut self.view_editor {
                    editor.category_index =
                        next_index_clamped(editor.category_index, self.category_rows.len(), 1);
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(editor) = &mut self.view_editor {
                    editor.category_index =
                        next_index_clamped(editor.category_index, self.category_rows.len(), -1);
                }
            }
            KeyCode::Char(' ') => {
                let Some(editor) = &mut self.view_editor else {
                    return Ok(false);
                };
                let row_index = editor
                    .category_index
                    .min(self.category_rows.len().saturating_sub(1));
                let Some(row) = self.category_rows.get(row_index).cloned() else {
                    return Ok(false);
                };
                if let Some(set) =
                    category_target_set_mut(&mut editor.draft, editor.section_index, target)
                {
                    if !set.insert(row.id) {
                        set.remove(&row.id);
                    }
                }
                self.refresh_view_editor_preview();
            }
            _ => {}
        }
        Ok(false)
    }

    pub(crate) fn handle_view_editor_bucket_key(&mut self, code: KeyCode) -> Result<bool, String> {
        let Some(target) = self.view_editor_bucket_target else {
            self.mode = Mode::ViewEditor;
            return Ok(false);
        };
        let buckets = when_bucket_options();
        match code {
            KeyCode::Esc | KeyCode::Enter => {
                self.view_editor_bucket_target = None;
                self.mode = if bucket_target_is_section(target) {
                    Mode::ViewSectionDetail
                } else {
                    Mode::ViewEditor
                };
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(editor) = &mut self.view_editor {
                    editor.bucket_index = next_index_clamped(editor.bucket_index, buckets.len(), 1);
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(editor) = &mut self.view_editor {
                    editor.bucket_index =
                        next_index_clamped(editor.bucket_index, buckets.len(), -1);
                }
            }
            KeyCode::Char(' ') => {
                let Some(editor) = &mut self.view_editor else {
                    return Ok(false);
                };
                let bucket = buckets[editor.bucket_index.min(buckets.len().saturating_sub(1))];
                if let Some(set) =
                    bucket_target_set_mut(&mut editor.draft, editor.section_index, target)
                {
                    if !set.insert(bucket) {
                        set.remove(&bucket);
                    }
                }
                self.refresh_view_editor_preview();
            }
            _ => {}
        }
        Ok(false)
    }

    pub(crate) fn handle_view_section_editor_key(&mut self, code: KeyCode) -> Result<bool, String> {
        if self.view_editor.is_none() {
            if self.view_editor_return_to_manager {
                self.mode = Mode::ViewManagerScreen;
                self.view_editor_return_to_manager = false;
            } else {
                self.mode = Mode::ViewPicker;
            }
            return Ok(false);
        }
        match code {
            KeyCode::Esc => {
                if self.view_editor_return_to_manager {
                    self.finish_view_editor_return_to_manager(
                        "Updated sections in manager draft (press s to persist)",
                    );
                } else {
                    self.mode = Mode::ViewEditor;
                    self.status = "View editor".to_string();
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(editor) = &mut self.view_editor {
                    if !editor.draft.sections.is_empty() {
                        editor.section_index = next_index_clamped(
                            editor.section_index,
                            editor.draft.sections.len(),
                            1,
                        );
                    }
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(editor) = &mut self.view_editor {
                    if !editor.draft.sections.is_empty() {
                        editor.section_index = next_index_clamped(
                            editor.section_index,
                            editor.draft.sections.len(),
                            -1,
                        );
                    }
                }
            }
            KeyCode::Char('N') => {
                if let Some(editor) = &mut self.view_editor {
                    let next = editor.draft.sections.len() + 1;
                    editor.draft.sections.push(Section {
                        title: format!("Section {next}"),
                        criteria: Query::default(),
                        on_insert_assign: HashSet::new(),
                        on_remove_unassign: HashSet::new(),
                        show_children: false,
                    });
                    editor.section_index = editor.draft.sections.len().saturating_sub(1);
                }
            }
            KeyCode::Char('x') => {
                if let Some(editor) = &mut self.view_editor {
                    if !editor.draft.sections.is_empty() {
                        editor.draft.sections.remove(editor.section_index);
                        editor.section_index = editor
                            .section_index
                            .min(editor.draft.sections.len().saturating_sub(1));
                    }
                }
            }
            KeyCode::Char('[') => {
                if let Some(editor) = &mut self.view_editor {
                    if editor.section_index > 0 {
                        editor
                            .draft
                            .sections
                            .swap(editor.section_index, editor.section_index - 1);
                        editor.section_index -= 1;
                    }
                }
            }
            KeyCode::Char(']') => {
                if let Some(editor) = &mut self.view_editor {
                    if editor.section_index + 1 < editor.draft.sections.len() {
                        editor
                            .draft
                            .sections
                            .swap(editor.section_index, editor.section_index + 1);
                        editor.section_index += 1;
                    }
                }
            }
            KeyCode::Enter | KeyCode::Char('e') => {
                if let Some(editor) = &self.view_editor {
                    if !editor.draft.sections.is_empty() {
                        self.mode = Mode::ViewSectionDetail;
                        self.status = "Section detail: t title, +/- categories, [/ ] virtual, a insert-set, r remove-set, h toggle children, Esc back".to_string();
                    }
                }
            }
            _ => {}
        }
        Ok(false)
    }

    pub(crate) fn handle_view_section_detail_key(&mut self, code: KeyCode) -> Result<bool, String> {
        let Some(section_index) = self.view_editor.as_ref().map(|editor| editor.section_index)
        else {
            if self.view_editor_return_to_manager {
                self.mode = Mode::ViewManagerScreen;
                self.view_editor_return_to_manager = false;
            } else {
                self.mode = Mode::ViewPicker;
            }
            return Ok(false);
        };
        let section_exists = self
            .view_editor
            .as_ref()
            .and_then(|editor| editor.draft.sections.get(section_index))
            .is_some();
        if !section_exists {
            if self.view_editor_return_to_manager {
                self.finish_view_editor_return_to_manager(
                    "Updated sections in manager draft (press s to persist)",
                );
            } else {
                self.mode = Mode::ViewSectionEditor;
            }
            return Ok(false);
        }
        match code {
            KeyCode::Esc => {
                if self.view_editor_return_to_manager {
                    self.finish_view_editor_return_to_manager(
                        "Updated sections in manager draft (press s to persist)",
                    );
                } else {
                    self.mode = Mode::ViewSectionEditor;
                }
            }
            KeyCode::Char('t') => {
                let title = self
                    .view_editor
                    .as_ref()
                    .and_then(|editor| editor.draft.sections.get(section_index))
                    .map(|section| section.title.clone())
                    .unwrap_or_default();
                self.mode = Mode::ViewSectionTitleInput;
                self.set_input(title);
            }
            KeyCode::Char('+') => {
                self.open_view_editor_category_picker(CategoryEditTarget::SectionCriteriaInclude);
            }
            KeyCode::Char('-') => {
                self.open_view_editor_category_picker(CategoryEditTarget::SectionCriteriaExclude);
            }
            KeyCode::Char(']') => {
                self.open_view_editor_bucket_picker(BucketEditTarget::SectionVirtualInclude);
            }
            KeyCode::Char('[') => {
                self.open_view_editor_bucket_picker(BucketEditTarget::SectionVirtualExclude);
            }
            KeyCode::Char('a') => {
                self.open_view_editor_category_picker(CategoryEditTarget::SectionOnInsertAssign);
            }
            KeyCode::Char('r') => {
                self.open_view_editor_category_picker(CategoryEditTarget::SectionOnRemoveUnassign);
            }
            KeyCode::Char('h') => {
                if let Some(editor) = &mut self.view_editor {
                    if let Some(section) = editor.draft.sections.get_mut(section_index) {
                        section.show_children = !section.show_children;
                    }
                }
            }
            _ => {}
        }
        Ok(false)
    }

    pub(crate) fn handle_view_section_title_key(&mut self, code: KeyCode) -> Result<bool, String> {
        match code {
            KeyCode::Esc => {
                self.mode = Mode::ViewSectionDetail;
                self.clear_input();
            }
            KeyCode::Enter => {
                if let Some(editor) = &mut self.view_editor {
                    if let Some(section) = editor.draft.sections.get_mut(editor.section_index) {
                        let title = self.input.trimmed().to_string();
                        if !title.is_empty() {
                            section.title = title;
                        }
                    }
                }
                self.mode = Mode::ViewSectionDetail;
                self.clear_input();
            }
            _ if self.handle_text_input_key(code) => {}
            _ => {}
        }
        Ok(false)
    }

    pub(crate) fn handle_view_unmatched_settings_key(
        &mut self,
        code: KeyCode,
    ) -> Result<bool, String> {
        let label = self
            .view_editor
            .as_ref()
            .map(|editor| editor.draft.unmatched_label.clone());
        if label.is_none() {
            if self.view_editor_return_to_manager {
                self.mode = Mode::ViewManagerScreen;
                self.view_editor_return_to_manager = false;
            } else {
                self.mode = Mode::ViewPicker;
            }
            return Ok(false);
        }
        match code {
            KeyCode::Esc => {
                if self.view_editor_return_to_manager {
                    self.finish_view_editor_return_to_manager(
                        "Updated unmatched settings in manager draft (press s to persist)",
                    );
                } else {
                    self.mode = Mode::ViewEditor;
                }
            }
            KeyCode::Char('t') => {
                if let Some(editor) = &mut self.view_editor {
                    editor.draft.show_unmatched = !editor.draft.show_unmatched;
                }
            }
            KeyCode::Char('l') => {
                self.mode = Mode::ViewUnmatchedLabelInput;
                self.set_input(label.unwrap_or_default());
            }
            _ => {}
        }
        Ok(false)
    }

    pub(crate) fn handle_view_unmatched_label_key(
        &mut self,
        code: KeyCode,
    ) -> Result<bool, String> {
        match code {
            KeyCode::Esc => {
                self.mode = Mode::ViewUnmatchedSettings;
                self.clear_input();
            }
            KeyCode::Enter => {
                if let Some(editor) = &mut self.view_editor {
                    let label = self.input.trimmed().to_string();
                    if !label.is_empty() {
                        editor.draft.unmatched_label = label;
                    }
                }
                self.mode = Mode::ViewUnmatchedSettings;
                self.clear_input();
            }
            _ if self.handle_text_input_key(code) => {}
            _ => {}
        }
        Ok(false)
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
