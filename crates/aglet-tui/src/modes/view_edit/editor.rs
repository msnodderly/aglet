use crate::*;

impl App {
    pub(crate) const DEFAULT_VIEW_EDIT_SECTION_TITLE: &'static str = "New section";

    pub(crate) fn cycle_view_board_display_mode(mode: BoardDisplayMode) -> BoardDisplayMode {
        match mode {
            BoardDisplayMode::SingleLine => BoardDisplayMode::MultiLine,
            BoardDisplayMode::MultiLine => BoardDisplayMode::SingleLine,
        }
    }

    pub(crate) fn cycle_view_section_flow(flow: SectionFlow) -> SectionFlow {
        match flow {
            SectionFlow::Vertical => SectionFlow::Horizontal,
            SectionFlow::Horizontal => SectionFlow::Vertical,
        }
    }

    pub(crate) fn cycle_section_board_display_mode_override(
        current: Option<BoardDisplayMode>,
    ) -> Option<BoardDisplayMode> {
        match current {
            None => Some(BoardDisplayMode::SingleLine),
            Some(BoardDisplayMode::SingleLine) => Some(BoardDisplayMode::MultiLine),
            Some(BoardDisplayMode::MultiLine) => None,
        }
    }

    pub(crate) fn view_edit_default_section(title: &str) -> Section {
        Section {
            title: title.to_string(),
            criteria: Query::default(),
            columns: Vec::new(),
            item_column_index: 0,
            on_insert_assign: HashSet::new(),
            on_remove_unassign: HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        }
    }

    pub(crate) fn view_edit_default_status() -> String {
        "View editor".to_string()
    }

    pub(crate) fn view_edit_alias_picker_status() -> String {
        "Aliases: j/k select  A/Enter:edit alias  Esc:done".to_string()
    }

    pub(crate) fn set_view_edit_dirty(&mut self) {
        if let Some(state) = &mut self.view_edit_state {
            state.dirty = true;
            state.discard_confirm = false;
            state.section_delete_confirm = None;
        }
    }

    fn open_view_edit_with_mode(&mut self, view: View, is_new_view: bool) {
        let preview_count = self.preview_count_for_query(&view.criteria);
        self.view_edit_state = Some(ViewEditState {
            draft: view,
            is_new_view,
            region: ViewEditRegion::Criteria,
            pane_focus: ViewEditPaneFocus::Details,
            criteria_index: 0,
            unmatched_field_index: 0,
            section_index: 0,
            sections_view_row_selected: false,
            section_details_field_index: 0,
            overlay: None,
            inline_input: None,
            inline_buf: text_buffer::TextBuffer::empty(),
            picker_index: 0,
            overlay_filter_buf: text_buffer::TextBuffer::empty(),
            preview_count,
            preview_visible: true,
            preview_scroll: 0,
            sections_filter_buf: text_buffer::TextBuffer::empty(),
            dirty: false,
            discard_confirm: false,
            section_delete_confirm: None,
            datebook_field_index: 0,
            name_focused: false,
            view_type_focused: false,
        });
        self.mode = Mode::ViewEdit;
        self.status = Self::view_edit_default_status();
    }

    pub(crate) fn open_view_edit(&mut self, view: View) {
        self.open_view_edit_with_mode(view, false);
    }

    pub(crate) fn open_view_edit_new_view_focus_name(&mut self, view: View) {
        self.open_view_edit_with_mode(view, true);
        if let Some(state) = &mut self.view_edit_state {
            state.sections_view_row_selected = true;
            state.region = ViewEditRegion::Criteria;
            state.pane_focus = ViewEditPaneFocus::Details;
            state.inline_input = Some(ViewEditInlineInput::ViewName);
            state.inline_buf = text_buffer::TextBuffer::new(String::new());
            state.discard_confirm = false;
        }
        self.status = "New view: type name, Enter to confirm, Esc to cancel".to_string();
    }

    pub(crate) fn cycle_view_edit_pane_focus(&mut self, forward: bool) {
        if let Some(state) = &mut self.view_edit_state {
            let next = if state.pane_focus == ViewEditPaneFocus::Preview {
                if forward {
                    0
                } else {
                    2
                }
            } else {
                let current = match state.pane_focus {
                    ViewEditPaneFocus::Details if state.region == ViewEditRegion::Sections => 2,
                    ViewEditPaneFocus::Details => 0,
                    ViewEditPaneFocus::Sections => 1,
                    ViewEditPaneFocus::Preview => unreachable!(),
                };
                if forward {
                    (current + 1) % 3
                } else {
                    (current + 3 - 1) % 3
                }
            };

            state.name_focused = false;
            state.view_type_focused = false;

            match next {
                0 => {
                    state.pane_focus = ViewEditPaneFocus::Details;
                    if state.region == ViewEditRegion::Sections
                        || state.region == ViewEditRegion::Datebook
                    {
                        state.region = ViewEditRegion::Criteria;
                    }
                    state.sections_view_row_selected = false;
                }
                1 => {
                    state.pane_focus = ViewEditPaneFocus::Sections;
                    if state.draft.datebook_config.is_some() {
                        state.region = ViewEditRegion::Datebook;
                        state.datebook_field_index = state
                            .datebook_field_index
                            .min(Self::DATEBOOK_FIELD_COUNT - 1);
                    } else {
                        state.region = ViewEditRegion::Sections;
                        if state.section_index >= state.draft.sections.len() {
                            state.section_index = state.draft.sections.len().saturating_sub(1);
                        }
                    }
                    state.sections_view_row_selected = false;
                }
                2 => {
                    state.pane_focus = ViewEditPaneFocus::Details;
                    state.region = ViewEditRegion::Sections;
                    state.sections_view_row_selected = false;
                    if state.section_index >= state.draft.sections.len() {
                        state.section_index = state.draft.sections.len().saturating_sub(1);
                    }
                    state.section_details_field_index = 0;
                }
                _ => unreachable!(),
            }
        }
    }

    pub(crate) fn toggle_view_edit_preview_visible(&mut self) {
        if let Some(state) = &mut self.view_edit_state {
            state.preview_visible = !state.preview_visible;
            if !state.preview_visible && state.pane_focus == ViewEditPaneFocus::Preview {
                state.pane_focus = ViewEditPaneFocus::Sections;
            }
            state.preview_scroll = 0;
            self.status = if state.preview_visible {
                "Preview pane shown".to_string()
            } else {
                "Preview pane hidden".to_string()
            };
        }
    }

    pub(crate) fn refresh_view_edit_preview(&mut self) {
        if let Some(state) = &self.view_edit_state {
            let count = self.preview_count_for_query(&state.draft.criteria);
            if let Some(state) = &mut self.view_edit_state {
                state.preview_count = count;
            }
        }
    }

    pub(crate) fn close_view_edit_overlay(&mut self) {
        if let Some(state) = &mut self.view_edit_state {
            state.overlay = None;
            state.picker_index = 0;
            state.overlay_filter_buf.clear();
        }
        self.status = Self::view_edit_default_status();
    }

    pub(crate) fn handle_view_edit_key(
        &mut self,
        code: KeyCode,
        aglet: &Aglet<'_>,
    ) -> TuiResult<bool> {
        if self
            .view_edit_state
            .as_ref()
            .map(|s| s.inline_input.is_some())
            .unwrap_or(false)
        {
            self.handle_view_edit_inline_key(code)?;
            return Ok(false);
        }

        if self
            .view_edit_state
            .as_ref()
            .map(|s| s.overlay.is_some())
            .unwrap_or(false)
        {
            self.handle_view_edit_overlay_key(code)?;
            return Ok(false);
        }

        if self
            .view_edit_state
            .as_ref()
            .and_then(|s| s.section_delete_confirm)
            .is_some()
        {
            self.handle_view_edit_section_delete_confirm_key(code)?;
            return Ok(false);
        }

        if self
            .view_edit_state
            .as_ref()
            .map(|s| s.discard_confirm)
            .unwrap_or(false)
        {
            self.handle_view_edit_discard_confirm_key(code, aglet)?;
            return Ok(false);
        }

        self.handle_view_edit_region_key(code, aglet)?;
        Ok(false)
    }

    fn handle_view_edit_section_delete_confirm_key(&mut self, code: KeyCode) -> TuiResult<bool> {
        match code {
            KeyCode::Char('y') => {
                self.confirm_view_edit_section_delete();
            }
            KeyCode::Esc => {
                if let Some(state) = &mut self.view_edit_state {
                    state.section_delete_confirm = None;
                }
                self.status = Self::view_edit_default_status();
            }
            _ => {}
        }
        Ok(true)
    }

    fn handle_view_edit_discard_confirm_key(
        &mut self,
        code: KeyCode,
        aglet: &Aglet<'_>,
    ) -> TuiResult<bool> {
        match code {
            KeyCode::Char('y') => {
                self.handle_view_edit_save(aglet)?;
            }
            KeyCode::Char('n') => {
                let is_new_view = self
                    .view_edit_state
                    .as_ref()
                    .map(|s| s.is_new_view)
                    .unwrap_or(false);
                self.view_edit_state = None;
                self.mode = Mode::ViewPicker;
                self.status = if is_new_view {
                    "View creation canceled; unsaved draft discarded".to_string()
                } else {
                    "View edit canceled; unsaved changes discarded".to_string()
                };
            }
            KeyCode::Esc => {
                if let Some(state) = &mut self.view_edit_state {
                    state.discard_confirm = false;
                }
                self.status = Self::view_edit_default_status();
            }
            _ => {}
        }
        Ok(true)
    }

    fn handle_view_edit_region_key(&mut self, code: KeyCode, aglet: &Aglet<'_>) -> TuiResult<bool> {
        match code {
            KeyCode::Esc => {
                let filter_active = self
                    .view_edit_state
                    .as_ref()
                    .map(Self::view_edit_filter_is_active)
                    .unwrap_or(false);
                if filter_active {
                    self.clear_view_edit_section_filter();
                    return Ok(true);
                }
                let is_dirty = self
                    .view_edit_state
                    .as_ref()
                    .map(|s| s.dirty)
                    .unwrap_or(false);
                if is_dirty {
                    if let Some(state) = &mut self.view_edit_state {
                        state.discard_confirm = true;
                    }
                    self.status =
                        "Save changes? y:save and close  n:discard  Esc:keep editing".to_string();
                } else {
                    let is_new_view = self
                        .view_edit_state
                        .as_ref()
                        .map(|s| s.is_new_view)
                        .unwrap_or(false);
                    self.view_edit_state = None;
                    self.mode = Mode::ViewPicker;
                    self.status = if is_new_view {
                        "View creation canceled".to_string()
                    } else {
                        "View edit closed".to_string()
                    };
                }
                return Ok(true);
            }
            KeyCode::Tab => {
                self.cycle_view_edit_pane_focus(true);
                return Ok(true);
            }
            KeyCode::BackTab => {
                self.cycle_view_edit_pane_focus(false);
                return Ok(true);
            }
            KeyCode::Char('S') if !self.is_ctrl_s_code(code) => {
                return self.handle_view_edit_save(aglet);
            }
            _ if self.is_ctrl_s_code(code) => {
                return self.handle_view_edit_save(aglet);
            }
            KeyCode::Char('p') => {
                self.toggle_view_edit_preview_visible();
                return Ok(true);
            }
            KeyCode::Char('P') => {
                if let Some(state) = &mut self.view_edit_state {
                    if !state.preview_visible {
                        state.preview_visible = true;
                    }
                    state.pane_focus = ViewEditPaneFocus::Preview;
                }
                return Ok(true);
            }
            KeyCode::Char('/') => {
                if let Some(state) = &mut self.view_edit_state {
                    state.pane_focus = ViewEditPaneFocus::Sections;
                    state.inline_input = Some(ViewEditInlineInput::SectionsFilter);
                    state.sections_view_row_selected = state.sections_view_row_selected
                        || state.region != ViewEditRegion::Sections;
                }
                self.status = "Section filter: type to filter  Enter:done  Esc:close".to_string();
                return Ok(true);
            }
            _ => {}
        }

        let Some(state) = &self.view_edit_state else {
            return Ok(false);
        };

        match state.pane_focus {
            ViewEditPaneFocus::Sections if state.draft.datebook_config.is_some() => {
                self.handle_view_edit_datebook_key(code)
            }
            ViewEditPaneFocus::Sections => self.handle_view_edit_sections_key(code),
            ViewEditPaneFocus::Details => {
                if state.draft.datebook_config.is_some() && state.region == ViewEditRegion::Sections
                {
                    self.handle_view_edit_preview_key(code)
                } else if Self::view_edit_showing_view_details(state) {
                    self.handle_view_edit_settings_key(code)
                } else {
                    self.handle_view_edit_section_details_key(code)
                }
            }
            ViewEditPaneFocus::Preview => self.handle_view_edit_preview_key(code),
        }
    }

    fn handle_view_edit_save(&mut self, aglet: &Aglet<'_>) -> TuiResult<bool> {
        let Some((draft, is_new_view)) = self
            .view_edit_state
            .as_ref()
            .map(|s| (s.draft.clone(), s.is_new_view))
        else {
            self.status = "View edit failed: no draft".to_string();
            return Ok(false);
        };
        let view_name = draft.name.clone();
        let save_result = if is_new_view {
            aglet.store().create_view(&draft)
        } else {
            aglet.store().update_view(&draft)
        };
        match save_result {
            Ok(()) => {
                self.refresh(aglet.store())?;
                self.set_view_selection_by_name(&view_name);
                self.reset_section_filters();
                self.view_edit_state = None;
                self.mode = if is_new_view {
                    Mode::Normal
                } else {
                    Mode::ViewPicker
                };
                self.status = if is_new_view {
                    format!("Created and switched to view \"{view_name}\"")
                } else {
                    format!("Saved view \"{view_name}\"")
                };
            }
            Err(err) => {
                self.status = format!("View save failed: {err}");
            }
        }
        Ok(true)
    }
}
