use crate::*;

impl App {
    pub(crate) fn handle_key_event(
        &mut self,
        key: KeyEvent,
        agenda: &Agenda<'_>,
    ) -> Result<bool, String> {
        match self.mode {
            Mode::Normal => self.handle_normal_key_event(key, agenda),
            _ => {
                self.normal_mode_prefix = None;
                self.handle_key(key.code, agenda)
            }
        }
    }

    pub(crate) fn handle_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> Result<bool, String> {
        match self.mode {
            Mode::Normal => self.handle_normal_key(code, agenda),
            Mode::InputPanel => self.handle_input_panel_key(code, agenda),
            Mode::NoteEdit => self.handle_note_edit_key(code, agenda),
            Mode::ItemAssignPicker => self.handle_item_assign_category_key(code, agenda),
            Mode::ItemAssignInput => self.handle_item_assign_category_input_key(code, agenda),
            Mode::InspectUnassign => self.handle_inspect_unassign_key(code, agenda),
            Mode::FilterInput => self.handle_filter_key(code, agenda),
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
            Mode::CategoryCreateConfirm { .. } => {
                self.handle_category_create_confirm_key(code, agenda)
            }
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
        self.input.handle_key(code, false)
    }

    pub(crate) fn selected_category_is_reserved(&self) -> bool {
        self.selected_category_row()
            .map(|row| row.is_reserved)
            .unwrap_or(false)
    }
}
