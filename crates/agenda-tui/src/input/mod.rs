use crate::*;

impl App {
    pub(crate) fn handle_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> Result<bool, String> {
        match self.mode {
            Mode::Normal => self.handle_normal_key(code, agenda),
            Mode::AddInput => self.handle_add_key(code, agenda),
            Mode::ItemEdit => self.handle_item_edit_key(code, agenda),
            Mode::NoteEdit => self.handle_note_edit_key(code, agenda),
            Mode::ItemAssignPicker => self.handle_item_assign_category_key(code, agenda),
            Mode::ItemAssignInput => {
                self.handle_item_assign_category_input_key(code, agenda)
            }
            Mode::InspectUnassign => self.handle_inspect_unassign_key(code, agenda),
            Mode::FilterInput => self.handle_filter_key(code, agenda),
            Mode::ViewPicker => self.handle_view_picker_key(code, agenda),
            Mode::ViewEdit => self.handle_view_edit_key(code, agenda),
            Mode::ViewCreateName => self.handle_view_create_name_key(code),
            Mode::ViewCreateCategory => self.handle_view_create_category_key(code, agenda),
            Mode::ViewRename => self.handle_view_rename_key(code, agenda),
            Mode::ViewDeleteConfirm => self.handle_view_delete_key(code, agenda),
            Mode::ConfirmDelete => self.handle_confirm_delete_key(code, agenda),
            Mode::CategoryManager => self.handle_category_manager_key(code, agenda),
            Mode::CategoryCreate => self.handle_category_create_key(code, agenda),
            Mode::CategoryRename => self.handle_category_rename_key(code, agenda),
            Mode::CategoryReparent => self.handle_category_reparent_key(code, agenda),
            Mode::CategoryDelete => self.handle_category_delete_key(code, agenda),
            Mode::CategoryConfig => self.handle_category_config_editor_key(code, agenda),
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

    pub(crate) fn handle_item_edit_note_input_key(&mut self, code: KeyCode) -> bool {
        self.item_edit_note.handle_key(code, true)
    }

    pub(crate) fn handle_item_edit_field_input_key(&mut self, code: KeyCode) -> bool {
        match self.item_edit_focus {
            ItemEditFocus::Text => self.handle_text_input_key(code),
            ItemEditFocus::Note => self.handle_item_edit_note_input_key(code),
            ItemEditFocus::CategoriesButton
            | ItemEditFocus::SaveButton
            | ItemEditFocus::CancelButton => false,
        }
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

    pub(crate) fn cycle_item_edit_focus(&mut self, delta: i32) {
        self.item_edit_focus = match (self.item_edit_focus, delta.signum()) {
            (ItemEditFocus::Text, d) if d >= 0 => ItemEditFocus::Note,
            (ItemEditFocus::Note, d) if d >= 0 => ItemEditFocus::CategoriesButton,
            (ItemEditFocus::CategoriesButton, d) if d >= 0 => ItemEditFocus::SaveButton,
            (ItemEditFocus::SaveButton, d) if d >= 0 => ItemEditFocus::CancelButton,
            (ItemEditFocus::CancelButton, d) if d >= 0 => ItemEditFocus::Text,
            (ItemEditFocus::Text, _) => ItemEditFocus::CancelButton,
            (ItemEditFocus::Note, _) => ItemEditFocus::Text,
            (ItemEditFocus::CategoriesButton, _) => ItemEditFocus::Note,
            (ItemEditFocus::SaveButton, _) => ItemEditFocus::CategoriesButton,
            (ItemEditFocus::CancelButton, _) => ItemEditFocus::SaveButton,
        };
    }
}
