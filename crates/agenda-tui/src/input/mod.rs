use crate::*;

impl App {
    pub(crate) fn handle_key_event(
        &mut self,
        key: KeyEvent,
        agenda: &Agenda<'_>,
    ) -> Result<bool, String> {
        match self.mode {
            Mode::Normal => self.handle_normal_key_event(key, agenda),
            _ => self.handle_key(key.code, agenda),
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
            Mode::ViewCreateCategory => self.handle_view_create_category_key(code, agenda),
            Mode::ViewDeleteConfirm => self.handle_view_delete_key(code, agenda),
            Mode::ConfirmDelete => self.handle_confirm_delete_key(code, agenda),
            Mode::CategoryManager => self.handle_category_manager_key(code, agenda),
            Mode::CategoryReparent => self.handle_category_reparent_key(code, agenda),
            Mode::CategoryDelete => self.handle_category_delete_key(code, agenda),
            Mode::CategoryConfig => self.handle_category_config_editor_key(code, agenda),
            Mode::CategoryDirectEdit => self.handle_category_direct_edit_key(code, agenda),
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

    pub(crate) fn handle_category_config_note_input_key(&mut self, code: KeyCode) -> bool {
        let Some(editor) = &mut self.category_config_editor else {
            return false;
        };
        editor.note.handle_key(code, true)
    }

    pub(crate) fn cycle_category_config_focus(&mut self, delta: i32) {
        let Some(editor) = &mut self.category_config_editor else {
            return;
        };
        editor.focus = match (editor.focus, delta.signum()) {
            (CategoryConfigFocus::Exclusive, d) if d >= 0 => CategoryConfigFocus::NoImplicit,
            (CategoryConfigFocus::NoImplicit, d) if d >= 0 => CategoryConfigFocus::Actionable,
            (CategoryConfigFocus::Actionable, d) if d >= 0 => CategoryConfigFocus::Note,
            (CategoryConfigFocus::Note, d) if d >= 0 => CategoryConfigFocus::SaveButton,
            (CategoryConfigFocus::SaveButton, d) if d >= 0 => CategoryConfigFocus::CancelButton,
            (CategoryConfigFocus::CancelButton, d) if d >= 0 => CategoryConfigFocus::Exclusive,
            (CategoryConfigFocus::Exclusive, _) => CategoryConfigFocus::CancelButton,
            (CategoryConfigFocus::NoImplicit, _) => CategoryConfigFocus::Exclusive,
            (CategoryConfigFocus::Actionable, _) => CategoryConfigFocus::NoImplicit,
            (CategoryConfigFocus::Note, _) => CategoryConfigFocus::Actionable,
            (CategoryConfigFocus::SaveButton, _) => CategoryConfigFocus::Note,
            (CategoryConfigFocus::CancelButton, _) => CategoryConfigFocus::SaveButton,
        };
    }

    pub(crate) fn move_category_config_checkbox_focus(&mut self, delta: i32) {
        let Some(editor) = &mut self.category_config_editor else {
            return;
        };
        editor.focus = match (editor.focus, delta.signum()) {
            (CategoryConfigFocus::Exclusive, d) if d >= 0 => CategoryConfigFocus::NoImplicit,
            (CategoryConfigFocus::NoImplicit, d) if d >= 0 => CategoryConfigFocus::Actionable,
            (CategoryConfigFocus::Actionable, d) if d >= 0 => CategoryConfigFocus::Actionable,
            (CategoryConfigFocus::Actionable, _) => CategoryConfigFocus::NoImplicit,
            (CategoryConfigFocus::NoImplicit, _) => CategoryConfigFocus::Exclusive,
            (CategoryConfigFocus::Exclusive, _) => CategoryConfigFocus::Exclusive,
            (focus, _) => focus,
        };
    }

    pub(crate) fn toggle_category_config_exclusive(&mut self) {
        if let Some(editor) = &mut self.category_config_editor {
            editor.is_exclusive = !editor.is_exclusive;
        }
    }

    pub(crate) fn toggle_category_config_no_implicit(&mut self) {
        if let Some(editor) = &mut self.category_config_editor {
            editor.enable_implicit_string = !editor.enable_implicit_string;
        }
    }

    pub(crate) fn toggle_category_config_actionable(&mut self) {
        if let Some(editor) = &mut self.category_config_editor {
            editor.is_actionable = !editor.is_actionable;
        }
    }
}
