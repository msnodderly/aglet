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
            Mode::ItemEditInput => self.handle_item_edit_key(code, agenda),
            Mode::NoteEditInput => self.handle_note_edit_key(code, agenda),
            Mode::ItemAssignCategoryPicker => self.handle_item_assign_category_key(code, agenda),
            Mode::ItemAssignCategoryInput => {
                self.handle_item_assign_category_input_key(code, agenda)
            }
            Mode::InspectUnassignPicker => self.handle_inspect_unassign_key(code, agenda),
            Mode::FilterInput => self.handle_filter_key(code, agenda),
            Mode::ViewPicker => self.handle_view_picker_key(code, agenda),
            Mode::ViewManagerScreen => self.handle_view_manager_key(code, agenda),
            Mode::ViewCreateNameInput => self.handle_view_create_name_key(code),
            Mode::ViewCreateCategoryPicker => self.handle_view_create_category_key(code, agenda),
            Mode::ViewRenameInput => self.handle_view_rename_key(code, agenda),
            Mode::ViewDeleteConfirm => self.handle_view_delete_key(code, agenda),
            Mode::ViewEditor => self.handle_view_editor_key(code, agenda),
            Mode::ViewEditorCategoryPicker => self.handle_view_editor_category_key(code),
            Mode::ViewEditorBucketPicker => self.handle_view_editor_bucket_key(code),
            Mode::ViewManagerCategoryPicker => self.handle_view_manager_category_picker_key(code),
            Mode::ViewSectionEditor => self.handle_view_section_editor_key(code),
            Mode::ViewSectionDetail => self.handle_view_section_detail_key(code),
            Mode::ViewSectionTitleInput => self.handle_view_section_title_key(code),
            Mode::ViewUnmatchedSettings => self.handle_view_unmatched_settings_key(code),
            Mode::ViewUnmatchedLabelInput => self.handle_view_unmatched_label_key(code),
            Mode::ConfirmDelete => self.handle_confirm_delete_key(code, agenda),
            Mode::CategoryManager => self.handle_category_manager_key(code, agenda),
            Mode::CategoryCreateInput => self.handle_category_create_key(code, agenda),
            Mode::CategoryRenameInput => self.handle_category_rename_key(code, agenda),
            Mode::CategoryReparentPicker => self.handle_category_reparent_key(code, agenda),
            Mode::CategoryDeleteConfirm => self.handle_category_delete_key(code, agenda),
            Mode::CategoryConfigEditor => self.handle_category_config_editor_key(code, agenda),
        }
    }

    pub(crate) fn set_input(&mut self, value: String) {
        self.input = value;
        self.input_cursor = self.input.chars().count();
    }

    pub(crate) fn clear_input(&mut self) {
        self.input.clear();
        self.input_cursor = 0;
    }

    pub(crate) fn input_len_chars(&self) -> usize {
        self.input.chars().count()
    }

    pub(crate) fn clamped_input_cursor(&self) -> usize {
        self.input_cursor.min(self.input_len_chars())
    }

    pub(crate) fn input_byte_index(&self, char_index: usize) -> usize {
        if char_index == 0 {
            return 0;
        }
        self.input
            .char_indices()
            .nth(char_index)
            .map(|(byte_index, _)| byte_index)
            .unwrap_or(self.input.len())
    }

    pub(crate) fn move_input_cursor_left(&mut self) {
        let cursor = self.clamped_input_cursor();
        self.input_cursor = cursor.saturating_sub(1);
    }

    pub(crate) fn move_input_cursor_right(&mut self) {
        let cursor = self.clamped_input_cursor();
        self.input_cursor = (cursor + 1).min(self.input_len_chars());
    }

    pub(crate) fn move_input_cursor_home(&mut self) {
        self.input_cursor = 0;
    }

    pub(crate) fn move_input_cursor_end(&mut self) {
        self.input_cursor = self.input_len_chars();
    }

    pub(crate) fn backspace_input_char(&mut self) {
        let cursor = self.clamped_input_cursor();
        if cursor == 0 {
            return;
        }
        let start = self.input_byte_index(cursor - 1);
        let end = self.input_byte_index(cursor);
        self.input.replace_range(start..end, "");
        self.input_cursor = cursor - 1;
    }

    pub(crate) fn delete_input_char(&mut self) {
        let cursor = self.clamped_input_cursor();
        if cursor >= self.input_len_chars() {
            return;
        }
        let start = self.input_byte_index(cursor);
        let end = self.input_byte_index(cursor + 1);
        self.input.replace_range(start..end, "");
        self.input_cursor = cursor;
    }

    pub(crate) fn insert_input_char(&mut self, c: char) {
        if c.is_control() {
            return;
        }
        let cursor = self.clamped_input_cursor();
        let byte_index = self.input_byte_index(cursor);
        self.input.insert(byte_index, c);
        self.input_cursor = cursor + 1;
    }

    pub(crate) fn handle_text_input_key(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Left => self.move_input_cursor_left(),
            KeyCode::Right => self.move_input_cursor_right(),
            KeyCode::Home => self.move_input_cursor_home(),
            KeyCode::End => self.move_input_cursor_end(),
            KeyCode::Backspace => self.backspace_input_char(),
            KeyCode::Delete => self.delete_input_char(),
            KeyCode::Char(c) => self.insert_input_char(c),
            _ => return false,
        }
        true
    }

    pub(crate) fn item_edit_note_len_chars(&self) -> usize {
        self.item_edit_note.chars().count()
    }

    pub(crate) fn item_edit_note_byte_index(&self, char_index: usize) -> usize {
        if char_index == 0 {
            return 0;
        }
        self.item_edit_note
            .char_indices()
            .nth(char_index)
            .map(|(byte_index, _)| byte_index)
            .unwrap_or(self.item_edit_note.len())
    }

    pub(crate) fn clamped_item_edit_note_cursor(&self) -> usize {
        self.item_edit_note_cursor
            .min(self.item_edit_note_len_chars())
    }

    pub(crate) fn move_item_edit_note_cursor_left(&mut self) {
        let cursor = self.clamped_item_edit_note_cursor();
        self.item_edit_note_cursor = cursor.saturating_sub(1);
    }

    pub(crate) fn move_item_edit_note_cursor_right(&mut self) {
        let cursor = self.clamped_item_edit_note_cursor();
        self.item_edit_note_cursor = (cursor + 1).min(self.item_edit_note_len_chars());
    }

    pub(crate) fn move_item_edit_note_cursor_home(&mut self) {
        self.item_edit_note_cursor = 0;
    }

    pub(crate) fn move_item_edit_note_cursor_end(&mut self) {
        self.item_edit_note_cursor = self.item_edit_note_len_chars();
    }

    pub(crate) fn move_item_edit_note_cursor_up(&mut self) {
        self.move_item_edit_note_cursor_vertical(-1);
    }

    pub(crate) fn move_item_edit_note_cursor_down(&mut self) {
        self.move_item_edit_note_cursor_vertical(1);
    }

    pub(crate) fn move_item_edit_note_cursor_vertical(&mut self, delta: i32) {
        let cursor = self.clamped_item_edit_note_cursor();
        let (line, col) = note_cursor_line_col(&self.item_edit_note, cursor);
        let line_starts = note_line_start_chars(&self.item_edit_note);
        if line_starts.is_empty() {
            self.item_edit_note_cursor = 0;
            return;
        }
        let target_line = if delta < 0 {
            line.saturating_sub(1)
        } else {
            (line + 1).min(line_starts.len().saturating_sub(1))
        };
        if target_line == line {
            return;
        }
        let target_start = line_starts[target_line];
        let note_len = self.item_edit_note_len_chars();
        let target_end = if target_line + 1 < line_starts.len() {
            line_starts[target_line + 1].saturating_sub(1)
        } else {
            note_len
        };
        let target_len = target_end.saturating_sub(target_start);
        self.item_edit_note_cursor = target_start + col.min(target_len);
    }

    pub(crate) fn backspace_item_edit_note_char(&mut self) {
        let cursor = self.clamped_item_edit_note_cursor();
        if cursor == 0 {
            return;
        }
        let start = self.item_edit_note_byte_index(cursor - 1);
        let end = self.item_edit_note_byte_index(cursor);
        self.item_edit_note.replace_range(start..end, "");
        self.item_edit_note_cursor = cursor - 1;
    }

    pub(crate) fn delete_item_edit_note_char(&mut self) {
        let cursor = self.clamped_item_edit_note_cursor();
        if cursor >= self.item_edit_note_len_chars() {
            return;
        }
        let start = self.item_edit_note_byte_index(cursor);
        let end = self.item_edit_note_byte_index(cursor + 1);
        self.item_edit_note.replace_range(start..end, "");
        self.item_edit_note_cursor = cursor;
    }

    pub(crate) fn insert_item_edit_note_char(&mut self, c: char) {
        if c.is_control() {
            return;
        }
        let cursor = self.clamped_item_edit_note_cursor();
        let byte_index = self.item_edit_note_byte_index(cursor);
        self.item_edit_note.insert(byte_index, c);
        self.item_edit_note_cursor = cursor + 1;
    }

    pub(crate) fn insert_item_edit_note_newline(&mut self) {
        let cursor = self.clamped_item_edit_note_cursor();
        let byte_index = self.item_edit_note_byte_index(cursor);
        self.item_edit_note.insert(byte_index, '\n');
        self.item_edit_note_cursor = cursor + 1;
    }

    pub(crate) fn handle_item_edit_note_input_key(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Left => self.move_item_edit_note_cursor_left(),
            KeyCode::Right => self.move_item_edit_note_cursor_right(),
            KeyCode::Up => self.move_item_edit_note_cursor_up(),
            KeyCode::Down => self.move_item_edit_note_cursor_down(),
            KeyCode::Home => self.move_item_edit_note_cursor_home(),
            KeyCode::End => self.move_item_edit_note_cursor_end(),
            KeyCode::Backspace => self.backspace_item_edit_note_char(),
            KeyCode::Delete => self.delete_item_edit_note_char(),
            KeyCode::Enter => self.insert_item_edit_note_newline(),
            KeyCode::Char(c) => self.insert_item_edit_note_char(c),
            _ => return false,
        }
        true
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

    pub(crate) fn category_config_note_cursor(&self) -> Option<usize> {
        self.category_config_editor
            .as_ref()
            .map(|editor| editor.note_cursor.min(editor.note.chars().count()))
    }

    pub(crate) fn move_category_config_note_cursor_left(&mut self) {
        if let Some(editor) = &mut self.category_config_editor {
            editor.note_cursor = editor.note_cursor.saturating_sub(1);
        }
    }

    pub(crate) fn move_category_config_note_cursor_right(&mut self) {
        if let Some(editor) = &mut self.category_config_editor {
            let max = editor.note.chars().count();
            editor.note_cursor = (editor.note_cursor + 1).min(max);
        }
    }

    pub(crate) fn move_category_config_note_cursor_home(&mut self) {
        if let Some(editor) = &mut self.category_config_editor {
            editor.note_cursor = 0;
        }
    }

    pub(crate) fn move_category_config_note_cursor_end(&mut self) {
        if let Some(editor) = &mut self.category_config_editor {
            editor.note_cursor = editor.note.chars().count();
        }
    }

    pub(crate) fn move_category_config_note_cursor_vertical(&mut self, delta: i32) {
        let Some(editor) = &mut self.category_config_editor else {
            return;
        };
        let cursor = editor.note_cursor.min(editor.note.chars().count());
        let (line, col) = note_cursor_line_col(&editor.note, cursor);
        let line_starts = note_line_start_chars(&editor.note);
        if line_starts.is_empty() {
            editor.note_cursor = 0;
            return;
        }
        let target_line = if delta < 0 {
            line.saturating_sub(1)
        } else {
            (line + 1).min(line_starts.len().saturating_sub(1))
        };
        if target_line == line {
            return;
        }
        let target_start = line_starts[target_line];
        let note_len = editor.note.chars().count();
        let target_end = if target_line + 1 < line_starts.len() {
            line_starts[target_line + 1].saturating_sub(1)
        } else {
            note_len
        };
        let target_len = target_end.saturating_sub(target_start);
        editor.note_cursor = target_start + col.min(target_len);
    }

    pub(crate) fn move_category_config_note_cursor_up(&mut self) {
        self.move_category_config_note_cursor_vertical(-1);
    }

    pub(crate) fn move_category_config_note_cursor_down(&mut self) {
        self.move_category_config_note_cursor_vertical(1);
    }

    pub(crate) fn backspace_category_config_note_char(&mut self) {
        let Some(editor) = &mut self.category_config_editor else {
            return;
        };
        let cursor = editor.note_cursor.min(editor.note.chars().count());
        if cursor == 0 {
            return;
        }
        let start = string_byte_index(&editor.note, cursor - 1);
        let end = string_byte_index(&editor.note, cursor);
        editor.note.replace_range(start..end, "");
        editor.note_cursor = cursor - 1;
    }

    pub(crate) fn delete_category_config_note_char(&mut self) {
        let Some(editor) = &mut self.category_config_editor else {
            return;
        };
        let cursor = editor.note_cursor.min(editor.note.chars().count());
        if cursor >= editor.note.chars().count() {
            return;
        }
        let start = string_byte_index(&editor.note, cursor);
        let end = string_byte_index(&editor.note, cursor + 1);
        editor.note.replace_range(start..end, "");
        editor.note_cursor = cursor;
    }

    pub(crate) fn insert_category_config_note_char(&mut self, c: char) {
        if c.is_control() {
            return;
        }
        let Some(editor) = &mut self.category_config_editor else {
            return;
        };
        let cursor = editor.note_cursor.min(editor.note.chars().count());
        let byte_index = string_byte_index(&editor.note, cursor);
        editor.note.insert(byte_index, c);
        editor.note_cursor = cursor + 1;
    }

    pub(crate) fn insert_category_config_note_newline(&mut self) {
        let Some(editor) = &mut self.category_config_editor else {
            return;
        };
        let cursor = editor.note_cursor.min(editor.note.chars().count());
        let byte_index = string_byte_index(&editor.note, cursor);
        editor.note.insert(byte_index, '\n');
        editor.note_cursor = cursor + 1;
    }

    pub(crate) fn handle_category_config_note_input_key(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Left => self.move_category_config_note_cursor_left(),
            KeyCode::Right => self.move_category_config_note_cursor_right(),
            KeyCode::Up => self.move_category_config_note_cursor_up(),
            KeyCode::Down => self.move_category_config_note_cursor_down(),
            KeyCode::Home => self.move_category_config_note_cursor_home(),
            KeyCode::End => self.move_category_config_note_cursor_end(),
            KeyCode::Backspace => self.backspace_category_config_note_char(),
            KeyCode::Delete => self.delete_category_config_note_char(),
            KeyCode::Enter => self.insert_category_config_note_newline(),
            KeyCode::Char(c) => self.insert_category_config_note_char(c),
            _ => return false,
        }
        true
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
