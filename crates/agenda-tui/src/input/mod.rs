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

    fn single_line_textarea(value: &str, cursor: usize) -> TextArea<'static> {
        let mut textarea = TextArea::new(vec![value.to_string()]);
        let col = cursor.min(value.chars().count()).min(u16::MAX as usize) as u16;
        textarea.move_cursor(CursorMove::Jump(0, col));
        textarea
    }

    fn multiline_textarea(value: &str, cursor: usize) -> TextArea<'static> {
        let mut textarea = TextArea::new(value.split('\n').map(str::to_string).collect());
        let (line, col) = note_cursor_line_col(value, cursor.min(value.chars().count()));
        let row = line.min(u16::MAX as usize) as u16;
        let col = col.min(u16::MAX as usize) as u16;
        textarea.move_cursor(CursorMove::Jump(row, col));
        textarea
    }

    fn char_index_from_line_col(value: &str, row: usize, col: usize) -> usize {
        let line_starts = note_line_start_chars(value);
        if line_starts.is_empty() {
            return 0;
        }

        let line_index = row.min(line_starts.len().saturating_sub(1));
        let line_start = line_starts[line_index];
        let value_len = value.chars().count();
        let line_end = if line_index + 1 < line_starts.len() {
            line_starts[line_index + 1].saturating_sub(1)
        } else {
            value_len
        };
        line_start + col.min(line_end.saturating_sub(line_start))
    }

    fn textarea_value_and_cursor(textarea: TextArea<'static>) -> (String, usize) {
        let (row, col) = textarea.cursor();
        let value = textarea.into_lines().join("\n");
        let cursor = Self::char_index_from_line_col(&value, row, col);
        (value, cursor)
    }

    fn with_input_textarea<F>(&mut self, edit: F)
    where
        F: FnOnce(&mut TextArea<'static>),
    {
        let mut textarea = Self::single_line_textarea(&self.input, self.clamped_input_cursor());
        edit(&mut textarea);
        let (value, cursor) = Self::textarea_value_and_cursor(textarea);
        self.input = value;
        self.input_cursor = cursor.min(self.input_len_chars());
    }

    fn with_item_edit_note_textarea<F>(&mut self, edit: F)
    where
        F: FnOnce(&mut TextArea<'static>),
    {
        let mut textarea =
            Self::multiline_textarea(&self.item_edit_note, self.clamped_item_edit_note_cursor());
        edit(&mut textarea);
        let (value, cursor) = Self::textarea_value_and_cursor(textarea);
        self.item_edit_note = value;
        self.item_edit_note_cursor = cursor.min(self.item_edit_note_len_chars());
    }

    fn with_category_config_note_textarea<F>(&mut self, edit: F)
    where
        F: FnOnce(&mut TextArea<'static>),
    {
        let Some((note, note_cursor)) = self
            .category_config_editor
            .as_ref()
            .map(|editor| (editor.note.clone(), editor.note_cursor))
        else {
            return;
        };
        let mut textarea = Self::multiline_textarea(&note, note_cursor);
        edit(&mut textarea);
        let (value, cursor) = Self::textarea_value_and_cursor(textarea);
        if let Some(editor) = &mut self.category_config_editor {
            editor.note = value;
            editor.note_cursor = cursor.min(editor.note.chars().count());
        }
    }

    pub(crate) fn input_len_chars(&self) -> usize {
        self.input.chars().count()
    }

    pub(crate) fn clamped_input_cursor(&self) -> usize {
        self.input_cursor.min(self.input_len_chars())
    }

    pub(crate) fn move_input_cursor_left(&mut self) {
        self.with_input_textarea(|textarea| textarea.move_cursor(CursorMove::Back));
    }

    pub(crate) fn move_input_cursor_right(&mut self) {
        self.with_input_textarea(|textarea| textarea.move_cursor(CursorMove::Forward));
    }

    pub(crate) fn move_input_cursor_home(&mut self) {
        self.with_input_textarea(|textarea| textarea.move_cursor(CursorMove::Head));
    }

    pub(crate) fn move_input_cursor_end(&mut self) {
        self.with_input_textarea(|textarea| textarea.move_cursor(CursorMove::End));
    }

    pub(crate) fn backspace_input_char(&mut self) {
        self.with_input_textarea(|textarea| {
            let _ = textarea.delete_char();
        });
    }

    pub(crate) fn delete_input_char(&mut self) {
        self.with_input_textarea(|textarea| {
            let _ = textarea.delete_next_char();
        });
    }

    pub(crate) fn insert_input_char(&mut self, c: char) {
        if c.is_control() {
            return;
        }
        self.with_input_textarea(|textarea| textarea.insert_char(c));
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

    pub(crate) fn clamped_item_edit_note_cursor(&self) -> usize {
        self.item_edit_note_cursor
            .min(self.item_edit_note_len_chars())
    }

    pub(crate) fn move_item_edit_note_cursor_left(&mut self) {
        self.with_item_edit_note_textarea(|textarea| textarea.move_cursor(CursorMove::Back));
    }

    pub(crate) fn move_item_edit_note_cursor_right(&mut self) {
        self.with_item_edit_note_textarea(|textarea| textarea.move_cursor(CursorMove::Forward));
    }

    pub(crate) fn move_item_edit_note_cursor_home(&mut self) {
        self.with_item_edit_note_textarea(|textarea| textarea.move_cursor(CursorMove::Head));
    }

    pub(crate) fn move_item_edit_note_cursor_end(&mut self) {
        self.with_item_edit_note_textarea(|textarea| textarea.move_cursor(CursorMove::End));
    }

    pub(crate) fn move_item_edit_note_cursor_up(&mut self) {
        self.move_item_edit_note_cursor_vertical(-1);
    }

    pub(crate) fn move_item_edit_note_cursor_down(&mut self) {
        self.move_item_edit_note_cursor_vertical(1);
    }

    pub(crate) fn move_item_edit_note_cursor_vertical(&mut self, delta: i32) {
        let movement = if delta < 0 {
            CursorMove::Up
        } else {
            CursorMove::Down
        };
        self.with_item_edit_note_textarea(|textarea| textarea.move_cursor(movement));
    }

    pub(crate) fn backspace_item_edit_note_char(&mut self) {
        self.with_item_edit_note_textarea(|textarea| {
            let _ = textarea.delete_char();
        });
    }

    pub(crate) fn delete_item_edit_note_char(&mut self) {
        self.with_item_edit_note_textarea(|textarea| {
            let _ = textarea.delete_next_char();
        });
    }

    pub(crate) fn insert_item_edit_note_char(&mut self, c: char) {
        if c.is_control() {
            return;
        }
        self.with_item_edit_note_textarea(|textarea| textarea.insert_char(c));
    }

    pub(crate) fn insert_item_edit_note_newline(&mut self) {
        self.with_item_edit_note_textarea(|textarea| textarea.insert_newline());
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
        self.with_category_config_note_textarea(|textarea| textarea.move_cursor(CursorMove::Back));
    }

    pub(crate) fn move_category_config_note_cursor_right(&mut self) {
        self.with_category_config_note_textarea(|textarea| {
            textarea.move_cursor(CursorMove::Forward);
        });
    }

    pub(crate) fn move_category_config_note_cursor_home(&mut self) {
        self.with_category_config_note_textarea(|textarea| textarea.move_cursor(CursorMove::Head));
    }

    pub(crate) fn move_category_config_note_cursor_end(&mut self) {
        self.with_category_config_note_textarea(|textarea| textarea.move_cursor(CursorMove::End));
    }

    pub(crate) fn move_category_config_note_cursor_vertical(&mut self, delta: i32) {
        let movement = if delta < 0 {
            CursorMove::Up
        } else {
            CursorMove::Down
        };
        self.with_category_config_note_textarea(|textarea| textarea.move_cursor(movement));
    }

    pub(crate) fn move_category_config_note_cursor_up(&mut self) {
        self.move_category_config_note_cursor_vertical(-1);
    }

    pub(crate) fn move_category_config_note_cursor_down(&mut self) {
        self.move_category_config_note_cursor_vertical(1);
    }

    pub(crate) fn backspace_category_config_note_char(&mut self) {
        self.with_category_config_note_textarea(|textarea| {
            let _ = textarea.delete_char();
        });
    }

    pub(crate) fn delete_category_config_note_char(&mut self) {
        self.with_category_config_note_textarea(|textarea| {
            let _ = textarea.delete_next_char();
        });
    }

    pub(crate) fn insert_category_config_note_char(&mut self, c: char) {
        if c.is_control() {
            return;
        }
        self.with_category_config_note_textarea(|textarea| textarea.insert_char(c));
    }

    pub(crate) fn insert_category_config_note_newline(&mut self) {
        self.with_category_config_note_textarea(|textarea| textarea.insert_newline());
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
