use crate::*;

impl App {
    pub(crate) fn handle_key_event(
        &mut self,
        key: KeyEvent,
        agenda: &Agenda<'_>,
    ) -> TuiResult<bool> {
        self.clear_expired_transient_status();
        self.clear_transient_status_on_key(key);
        self.current_key_modifiers = key.modifiers;
        let handled = match self.mode {
            Mode::Normal => self.handle_normal_key_event(key, agenda),
            Mode::GlobalSettings
            | Mode::HelpPanel
            | Mode::SuggestionReview
            | Mode::InputPanel
            | Mode::LinkWizard
            | Mode::ItemAssignPicker
            | Mode::ItemAssignInput
            | Mode::InspectUnassign
            | Mode::SearchBarFocused
            | Mode::ViewPicker
            | Mode::ViewEdit
            | Mode::ViewDeleteConfirm
            | Mode::ConfirmDelete
            | Mode::BoardColumnDeleteConfirm
            | Mode::CategoryManager
            | Mode::CategoryDirectEdit
            | Mode::CategoryColumnPicker
            | Mode::BoardAddColumnPicker => {
                self.normal_mode_prefix = None;
                self.handle_key(key.code, agenda)
            }
        };
        self.current_key_modifiers = KeyModifiers::NONE;
        handled
    }

    pub(crate) fn handle_key(&mut self, code: KeyCode, agenda: &Agenda<'_>) -> TuiResult<bool> {
        match self.mode {
            Mode::Normal => self.handle_normal_key(code, agenda),
            Mode::GlobalSettings => self.handle_global_settings_key(code, agenda),
            Mode::HelpPanel => self.handle_help_panel_key(code),
            Mode::SuggestionReview => self.handle_suggestion_review_key(code, agenda),
            Mode::InputPanel => self.handle_input_panel_key(code, agenda),
            Mode::LinkWizard => self.handle_link_wizard_key(code, agenda),
            Mode::ItemAssignPicker => self.handle_item_assign_category_key(code, agenda),
            Mode::ItemAssignInput => self.handle_item_assign_category_input_key(code, agenda),
            Mode::InspectUnassign => self.handle_inspect_unassign_key(code, agenda),
            Mode::SearchBarFocused => self.handle_search_bar_key(code, agenda),
            Mode::ViewPicker => self.handle_view_picker_key(code, agenda),
            Mode::ViewEdit => self.handle_view_edit_key(code, agenda),
            Mode::ViewDeleteConfirm => self.handle_view_delete_key(code, agenda),
            Mode::ConfirmDelete => self.handle_confirm_delete_key(code, agenda),
            Mode::BoardColumnDeleteConfirm => {
                self.handle_board_column_delete_confirm_key(code, agenda)
            }
            Mode::CategoryManager => self.handle_category_manager_key(code, agenda),
            Mode::CategoryDirectEdit => self.handle_category_direct_edit_key(code, agenda),
            Mode::CategoryColumnPicker => self.handle_category_column_picker_key(code, agenda),
            Mode::BoardAddColumnPicker => self.handle_board_add_column_key(code, agenda),
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

    pub(crate) fn handle_text_input_key(&mut self, code: KeyCode) -> bool {
        self.input
            .handle_key_event(self.text_key_event(code), false)
    }

    pub(crate) fn text_key_event(&self, code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, self.current_key_modifiers)
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
