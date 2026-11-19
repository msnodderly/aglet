use crate::*;

/// Ctrl+S, the save chord used by every editor.
pub(crate) fn is_ctrl_s(key: KeyEvent) -> bool {
    matches!(key.code, KeyCode::Char('s') | KeyCode::Char('S'))
        && key.modifiers.contains(KeyModifiers::CONTROL)
}

impl App {
    pub(crate) fn handle_key_event(&mut self, key: KeyEvent, aglet: &Aglet<'_>) -> TuiResult<bool> {
        self.clear_expired_transient_status();
        self.clear_transient_status_on_key(key);
        if self.mode == Mode::Normal {
            return self.handle_normal_key_event(key, aglet);
        }
        self.normal_mode_prefix = None;
        self.handle_key(key, aglet)
    }

    /// Route a key to the current mode's handler.
    pub(crate) fn handle_key(&mut self, key: KeyEvent, aglet: &Aglet<'_>) -> TuiResult<bool> {
        match self.mode {
            Mode::Normal => self.handle_normal_key(key, aglet),
            Mode::GlobalSettings => self.handle_global_settings_key(key, aglet),
            Mode::HelpPanel { ref mut scroll } => {
                if modes::help_panel::handle_key(scroll, key) {
                    self.mode = Mode::Normal;
                }
                Ok(false)
            }
            Mode::SuggestionReview => self.handle_suggestion_review_key(key, aglet),
            Mode::InputPanel => self.handle_input_panel_key(key, aglet),
            Mode::LinkWizard => self.handle_link_wizard_key(key, aglet),
            Mode::ItemAssignPicker => self.handle_item_assign_category_key(key, aglet),
            Mode::ItemAssignInput => self.handle_item_assign_category_input_key(key, aglet),
            Mode::InspectUnassign => self.handle_inspect_unassign_key(key, aglet),
            Mode::SearchBarFocused => self.handle_search_bar_key(key, aglet),
            Mode::ViewPicker { index } => self.handle_view_picker_key(index, key, aglet),
            Mode::ViewEdit => self.handle_view_edit_key(key, aglet),
            Mode::ViewDeleteConfirm { index } => self.handle_view_delete_key(index, key, aglet),
            Mode::ConfirmDelete => self.handle_confirm_delete_key(key, aglet),
            Mode::BoardColumnDeleteConfirm => {
                self.handle_board_column_delete_confirm_key(key, aglet)
            }
            Mode::CategoryManager => self.handle_category_manager_key(key, aglet),
            Mode::CategoryDirectEdit => self.handle_category_direct_edit_key(key, aglet),
            Mode::CategoryColumnPicker => self.handle_category_column_picker_key(key, aglet),
            Mode::BoardAddColumnPicker => self.handle_board_add_column_key(key, aglet),
        }
    }

    pub(crate) fn set_input(&mut self, value: String) {
        self.input.set(value);
    }

    pub(crate) fn clear_input(&mut self) {
        self.input.clear();
    }

    pub(crate) fn clamped_input_cursor(&self) -> usize {
        self.input.cursor()
    }

    pub(crate) fn handle_text_input_key(&mut self, key: KeyEvent) -> bool {
        self.input.handle_key_event(key, false)
    }

    pub(crate) fn selected_category_is_reserved(&self) -> bool {
        self.selected_category_row()
            .map(|row| row.is_reserved)
            .unwrap_or(false)
    }

    pub(crate) fn selected_category_is_numeric(&self) -> bool {
        self.selected_category_row()
            .map(|row| row.value_kind == CategoryValueKind::Numeric)
            .unwrap_or(false)
    }
}
