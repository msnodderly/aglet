use crate::*;

impl App {
    pub(crate) fn begin_view_edit_name_input(&mut self) {
        if let Some(state) = &mut self.view_edit_state {
            let current = if state.is_new_view && state.draft.name == "Untitled View" {
                String::new()
            } else {
                state.draft.name.clone()
            };
            state.sections_view_row_selected = true;
            state.active_tab = ViewEditTab::Criteria;
            if state.region == ViewEditRegion::Sections {
                state.region = ViewEditRegion::Criteria;
            }
            state.pane_focus = ViewEditPaneFocus::Details;
            state.inline_input = Some(ViewEditInlineInput::ViewName);
            state.inline_buf = text_buffer::TextBuffer::new(current);
            state.discard_confirm = false;
            state.section_delete_confirm = None;
            self.status = "View name: type text  Enter:confirm  Esc:cancel".to_string();
        }
    }

    pub(crate) fn begin_view_edit_alias_input(&mut self, category_id: CategoryId) {
        let Some(row) = self.category_rows.iter().find(|row| row.id == category_id) else {
            return;
        };
        if let Some(state) = &mut self.view_edit_state {
            let current = state
                .draft
                .category_aliases
                .get(&category_id)
                .cloned()
                .unwrap_or_default();
            state.inline_input = Some(ViewEditInlineInput::CategoryAlias { category_id });
            state.inline_buf = text_buffer::TextBuffer::new(current);
            state.discard_confirm = false;
            state.section_delete_confirm = None;
        }
        self.status = format!("Alias for {}: type text  Enter:save  Esc:cancel", row.name);
    }

    pub(crate) fn handle_view_edit_inline_key(&mut self, code: KeyCode) -> TuiResult<bool> {
        let Some(state) = &self.view_edit_state else {
            return Ok(false);
        };
        let inline = state.inline_input.clone();
        match code {
            KeyCode::Esc => {
                let mut clear_buf = true;
                if let Some(state) = &mut self.view_edit_state {
                    if matches!(
                        state.inline_input,
                        Some(ViewEditInlineInput::SectionsFilter)
                    ) {
                        clear_buf = false;
                    }
                    state.inline_input = None;
                    if clear_buf {
                        state.inline_buf.clear();
                    }
                }
                self.status = if matches!(inline, Some(ViewEditInlineInput::CategoryAlias { .. })) {
                    Self::view_edit_alias_picker_status()
                } else {
                    Self::view_edit_default_status()
                };
            }
            KeyCode::Enter => {
                let Some(state) = &mut self.view_edit_state else {
                    return Ok(false);
                };
                let text = state.inline_buf.trimmed().to_string();
                let mut changed = false;
                let mut filter_done_status: Option<String> = None;
                let mut alias_status: Option<String> = None;
                match &inline {
                    Some(ViewEditInlineInput::SectionsFilter) => {
                        state.inline_input = None;
                        let status = if Self::view_edit_filter_is_active(state) {
                            format!("Section filter: {}", state.sections_filter_buf.text())
                        } else {
                            Self::view_edit_default_status()
                        };
                        filter_done_status = Some(status);
                    }
                    Some(ViewEditInlineInput::ViewName) => {
                        changed = state.draft.name != text;
                        state.draft.name = text;
                    }
                    Some(ViewEditInlineInput::SectionTitle {
                        section_index,
                        is_new,
                    }) => {
                        if let Some(section) = state.draft.sections.get_mut(*section_index) {
                            changed = section.title != text;
                            section.title = text;
                        }
                        if *is_new {
                            state.pane_focus = ViewEditPaneFocus::Details;
                            state.region = ViewEditRegion::Sections;
                            state.section_details_field_index = 1;
                        }
                    }
                    Some(ViewEditInlineInput::UnmatchedLabel) => {
                        changed = state.draft.unmatched_label != text;
                        state.draft.unmatched_label = text;
                    }
                    Some(ViewEditInlineInput::CategoryAlias { category_id }) => {
                        if text.is_empty() {
                            changed = state.draft.category_aliases.remove(category_id).is_some();
                            alias_status = Some("Alias cleared".to_string());
                        } else {
                            let next_alias = text.clone();
                            let previous = state.draft.category_aliases.get(category_id).cloned();
                            changed = previous.as_deref() != Some(next_alias.as_str());
                            state
                                .draft
                                .category_aliases
                                .insert(*category_id, next_alias);
                            alias_status = Some("Alias saved".to_string());
                        }
                    }
                    None => {}
                }
                if filter_done_status.is_some() {
                    state.inline_buf.clear();
                }
                state.inline_input = None;
                if filter_done_status.is_none() {
                    state.inline_buf.clear();
                }
                if changed {
                    state.dirty = true;
                    state.discard_confirm = false;
                }
                if let Some(status) = filter_done_status {
                    let _ = state;
                    self.normalize_view_edit_sections_selection_for_filter();
                    self.status = status;
                    return Ok(true);
                }
                let new_section_created = matches!(
                    inline,
                    Some(ViewEditInlineInput::SectionTitle { is_new: true, .. })
                );
                self.status = if let Some(status) = alias_status {
                    status
                } else if matches!(inline, Some(ViewEditInlineInput::CategoryAlias { .. })) {
                    Self::view_edit_alias_picker_status()
                } else if new_section_created {
                    "Section created — configure filter below, or Tab to go back".to_string()
                } else {
                    Self::view_edit_default_status()
                };
            }
            _ => {
                let mut filter_status: Option<String> = None;
                let text_key = self.text_key_event(code);
                if let Some(state) = &mut self.view_edit_state {
                    match inline {
                        Some(ViewEditInlineInput::SectionsFilter) => {
                            state.sections_filter_buf.handle_key_event(text_key, false);
                            filter_status = Some(if Self::view_edit_filter_is_active(state) {
                                format!("Section filter: {}", state.sections_filter_buf.text())
                            } else {
                                "Section filter".to_string()
                            });
                        }
                        _ => {
                            state.inline_buf.handle_key_event(text_key, false);
                        }
                    }
                }
                if let Some(status) = filter_status {
                    self.normalize_view_edit_sections_selection_for_filter();
                    self.status = status;
                }
            }
        }
        Ok(true)
    }
}
