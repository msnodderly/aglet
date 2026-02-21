use std::collections::HashSet;

use agenda_core::model::{CategoryId, ItemId};
use crossterm::event::KeyCode;

use crate::text_buffer::TextBuffer;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum InputPanelKind {
    /// New item in the current section context.
    AddItem,
    /// Edit an existing item.
    EditItem,
    /// Single text field for naming (views, categories).
    NameInput,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum InputPanelFocus {
    Text,
    Note,
    CategoriesButton,
    SaveButton,
    CancelButton,
}

/// Action returned by InputPanel::handle_key. The caller interprets each action.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum InputPanelAction {
    /// Focus moved forward (Tab).
    FocusNext,
    /// Focus moved backward (Shift-Tab).
    FocusPrev,
    /// Open the category picker overlay.
    OpenCategoryPicker,
    /// Save / submit the panel contents.
    Save,
    /// Cancel / discard.
    Cancel,
    /// A text key was consumed internally.
    Handled,
    /// The key was not consumed.
    Unhandled,
}

/// State for the embedded category picker overlay.
#[derive(Clone, Debug)]
pub(crate) struct CategoryPickerState {
    /// Cursor row within the category list.
    pub(crate) picker_index: usize,
}

/// Unified input panel for add-item, edit-item, and name-input flows.
#[derive(Clone)]
pub(crate) struct InputPanel {
    pub(crate) kind: InputPanelKind,
    pub(crate) text: TextBuffer,
    pub(crate) note: TextBuffer,
    /// Categories currently assigned/selected in the panel (draft state).
    pub(crate) categories: HashSet<CategoryId>,
    pub(crate) focus: InputPanelFocus,
    /// `Some` when editing an existing item, `None` when adding.
    pub(crate) item_id: Option<ItemId>,
    /// Descriptive context shown below categories (section name + auto-assign preview).
    pub(crate) preview_context: String,
    /// `Some` while the embedded category picker overlay is open.
    pub(crate) category_picker: Option<CategoryPickerState>,
}

impl InputPanel {
    pub(crate) fn new_add_item(
        section_title: &str,
        on_insert_assign: &HashSet<CategoryId>,
    ) -> Self {
        Self {
            kind: InputPanelKind::AddItem,
            text: TextBuffer::empty(),
            note: TextBuffer::empty(),
            categories: HashSet::new(),
            focus: InputPanelFocus::Text,
            item_id: None,
            preview_context: format_section_context(section_title, on_insert_assign),
            category_picker: None,
        }
    }

    pub(crate) fn new_edit_item(
        item_id: ItemId,
        text: String,
        note: String,
        categories: HashSet<CategoryId>,
    ) -> Self {
        Self {
            kind: InputPanelKind::EditItem,
            text: TextBuffer::new(text),
            note: TextBuffer::new(note),
            categories,
            focus: InputPanelFocus::Text,
            item_id: Some(item_id),
            preview_context: String::new(),
            category_picker: None,
        }
    }

    pub(crate) fn new_name_input(current_name: &str, label: &str) -> Self {
        Self {
            kind: InputPanelKind::NameInput,
            text: TextBuffer::new(current_name.to_string()),
            note: TextBuffer::empty(),
            categories: HashSet::new(),
            focus: InputPanelFocus::Text,
            item_id: None,
            preview_context: label.to_string(),
            category_picker: None,
        }
    }

    /// Returns `true` if the category picker overlay is currently open.
    pub(crate) fn category_picker_open(&self) -> bool {
        self.category_picker.is_some()
    }

    /// Opens the category picker overlay with cursor at `initial_index`.
    pub(crate) fn open_category_picker(&mut self, initial_index: usize) {
        self.category_picker = Some(CategoryPickerState {
            picker_index: initial_index,
        });
    }

    /// Closes the category picker overlay.
    pub(crate) fn close_category_picker(&mut self) {
        self.category_picker = None;
    }

    /// Returns the current picker cursor index, or `None` if the picker is closed.
    pub(crate) fn picker_index(&self) -> Option<usize> {
        self.category_picker.as_ref().map(|p| p.picker_index)
    }

    /// Moves the picker cursor by `delta`, wrapping around within `list_len`.
    pub(crate) fn move_picker_cursor(&mut self, list_len: usize, delta: i32) {
        if let Some(picker) = &mut self.category_picker {
            if list_len == 0 {
                return;
            }
            let current = picker.picker_index as i64;
            let len = list_len as i64;
            let new = ((current + delta as i64).rem_euclid(len)) as usize;
            picker.picker_index = new;
        }
    }

    /// Toggles `category_id` in the panel's category set.
    pub(crate) fn toggle_category(&mut self, category_id: CategoryId) {
        if self.categories.contains(&category_id) {
            self.categories.remove(&category_id);
        } else {
            self.categories.insert(category_id);
        }
    }

    /// Handle a keypress. Called only when the category picker overlay is NOT open.
    /// Returns the action the caller should perform.
    pub(crate) fn handle_key(&mut self, code: KeyCode) -> InputPanelAction {
        if let Some(action) = self.handle_focus_navigation(code) {
            return action;
        }

        match self.focus {
            InputPanelFocus::Text | InputPanelFocus::Note => {
                let multiline = self.focus == InputPanelFocus::Note;
                let buffer = self.active_buffer_mut();
                if buffer.handle_key(code, multiline) {
                    InputPanelAction::Handled
                } else {
                    InputPanelAction::Unhandled
                }
            }
            InputPanelFocus::CategoriesButton => self.handle_categories_button(code),
            InputPanelFocus::SaveButton => self.handle_save_button(code),
            InputPanelFocus::CancelButton => self.handle_cancel_button(code),
        }
    }

    fn handle_focus_navigation(&mut self, code: KeyCode) -> Option<InputPanelAction> {
        match code {
            KeyCode::Tab => {
                self.cycle_focus_forward();
                Some(InputPanelAction::FocusNext)
            }
            KeyCode::BackTab => {
                self.cycle_focus_backward();
                Some(InputPanelAction::FocusPrev)
            }
            KeyCode::Esc => Some(InputPanelAction::Cancel),
            // Capital S saves from any focus (consistent with ViewEdit §4.7)
            KeyCode::Char('S') => Some(InputPanelAction::Save),
            _ => None,
        }
    }

    fn handle_categories_button(&self, code: KeyCode) -> InputPanelAction {
        match code {
            KeyCode::Enter | KeyCode::Char(' ') => InputPanelAction::OpenCategoryPicker,
            _ => InputPanelAction::Unhandled,
        }
    }

    fn handle_save_button(&self, code: KeyCode) -> InputPanelAction {
        match code {
            KeyCode::Enter | KeyCode::Char(' ') => InputPanelAction::Save,
            _ => InputPanelAction::Unhandled,
        }
    }

    fn handle_cancel_button(&self, code: KeyCode) -> InputPanelAction {
        match code {
            KeyCode::Enter | KeyCode::Char(' ') => InputPanelAction::Cancel,
            _ => InputPanelAction::Unhandled,
        }
    }

    fn active_buffer_mut(&mut self) -> &mut TextBuffer {
        match self.focus {
            InputPanelFocus::Note => &mut self.note,
            _ => &mut self.text,
        }
    }

    fn cycle_focus_forward(&mut self) {
        self.focus = match self.focus {
            InputPanelFocus::Text => {
                if self.kind == InputPanelKind::NameInput {
                    InputPanelFocus::SaveButton
                } else {
                    InputPanelFocus::Note
                }
            }
            InputPanelFocus::Note => InputPanelFocus::CategoriesButton,
            InputPanelFocus::CategoriesButton => InputPanelFocus::SaveButton,
            InputPanelFocus::SaveButton => InputPanelFocus::CancelButton,
            InputPanelFocus::CancelButton => InputPanelFocus::Text,
        };
    }

    fn cycle_focus_backward(&mut self) {
        self.focus = match self.focus {
            InputPanelFocus::Text => InputPanelFocus::CancelButton,
            InputPanelFocus::Note => InputPanelFocus::Text,
            InputPanelFocus::CategoriesButton => InputPanelFocus::Note,
            InputPanelFocus::SaveButton => {
                if self.kind == InputPanelKind::NameInput {
                    InputPanelFocus::Text
                } else {
                    InputPanelFocus::CategoriesButton
                }
            }
            InputPanelFocus::CancelButton => InputPanelFocus::SaveButton,
        };
    }
}

fn format_section_context(section_title: &str, on_insert_assign: &HashSet<CategoryId>) -> String {
    if on_insert_assign.is_empty() {
        format!("Adding to \"{}\"", section_title)
    } else {
        format!(
            "Adding to \"{}\" (will auto-assign {} categories)",
            section_title,
            on_insert_assign.len()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn add_panel() -> InputPanel {
        InputPanel::new_add_item("Open", &HashSet::new())
    }

    fn edit_panel() -> InputPanel {
        InputPanel::new_edit_item(
            ItemId::new_v4(),
            "Test item".to_string(),
            "Test note".to_string(),
            HashSet::new(),
        )
    }

    fn name_panel() -> InputPanel {
        InputPanel::new_name_input("", "View name")
    }

    // --- Focus cycling ---

    #[test]
    fn tab_cycles_add_panel_forward() {
        let mut p = add_panel();
        assert_eq!(p.focus, InputPanelFocus::Text);
        p.handle_key(KeyCode::Tab);
        assert_eq!(p.focus, InputPanelFocus::Note);
        p.handle_key(KeyCode::Tab);
        assert_eq!(p.focus, InputPanelFocus::CategoriesButton);
        p.handle_key(KeyCode::Tab);
        assert_eq!(p.focus, InputPanelFocus::SaveButton);
        p.handle_key(KeyCode::Tab);
        assert_eq!(p.focus, InputPanelFocus::CancelButton);
        p.handle_key(KeyCode::Tab);
        assert_eq!(p.focus, InputPanelFocus::Text);
    }

    #[test]
    fn backtab_cycles_add_panel_backward() {
        let mut p = add_panel();
        p.handle_key(KeyCode::BackTab);
        assert_eq!(p.focus, InputPanelFocus::CancelButton);
        p.handle_key(KeyCode::BackTab);
        assert_eq!(p.focus, InputPanelFocus::SaveButton);
        p.handle_key(KeyCode::BackTab);
        assert_eq!(p.focus, InputPanelFocus::CategoriesButton);
        p.handle_key(KeyCode::BackTab);
        assert_eq!(p.focus, InputPanelFocus::Note);
        p.handle_key(KeyCode::BackTab);
        assert_eq!(p.focus, InputPanelFocus::Text);
    }

    #[test]
    fn name_panel_tab_skips_note_and_categories() {
        let mut p = name_panel();
        p.handle_key(KeyCode::Tab);
        assert_eq!(p.focus, InputPanelFocus::SaveButton);
        p.handle_key(KeyCode::Tab);
        assert_eq!(p.focus, InputPanelFocus::CancelButton);
        p.handle_key(KeyCode::Tab);
        assert_eq!(p.focus, InputPanelFocus::Text);
    }

    #[test]
    fn name_panel_backtab_skips_note_and_categories() {
        let mut p = name_panel();
        p.handle_key(KeyCode::BackTab);
        assert_eq!(p.focus, InputPanelFocus::CancelButton);
        p.handle_key(KeyCode::BackTab);
        assert_eq!(p.focus, InputPanelFocus::SaveButton);
        p.handle_key(KeyCode::BackTab);
        assert_eq!(p.focus, InputPanelFocus::Text);
    }

    // --- Esc cancels from any focus ---

    #[test]
    fn esc_returns_cancel_from_any_focus() {
        let mut p = add_panel();
        for focus in [
            InputPanelFocus::Text,
            InputPanelFocus::Note,
            InputPanelFocus::CategoriesButton,
            InputPanelFocus::SaveButton,
            InputPanelFocus::CancelButton,
        ] {
            p.focus = focus;
            assert_eq!(p.handle_key(KeyCode::Esc), InputPanelAction::Cancel,
                       "expected Cancel at focus {:?}", focus);
        }
    }

    // --- Button activations ---

    #[test]
    fn enter_on_save_button_saves() {
        let mut p = add_panel();
        p.focus = InputPanelFocus::SaveButton;
        assert_eq!(p.handle_key(KeyCode::Enter), InputPanelAction::Save);
    }

    #[test]
    fn space_on_save_button_saves() {
        let mut p = add_panel();
        p.focus = InputPanelFocus::SaveButton;
        assert_eq!(p.handle_key(KeyCode::Char(' ')), InputPanelAction::Save);
    }

    #[test]
    fn enter_on_cancel_button_cancels() {
        let mut p = add_panel();
        p.focus = InputPanelFocus::CancelButton;
        assert_eq!(p.handle_key(KeyCode::Enter), InputPanelAction::Cancel);
    }

    #[test]
    fn enter_on_categories_button_opens_picker() {
        let mut p = add_panel();
        p.focus = InputPanelFocus::CategoriesButton;
        assert_eq!(p.handle_key(KeyCode::Enter), InputPanelAction::OpenCategoryPicker);
    }

    #[test]
    fn space_on_categories_button_opens_picker() {
        let mut p = add_panel();
        p.focus = InputPanelFocus::CategoriesButton;
        assert_eq!(p.handle_key(KeyCode::Char(' ')), InputPanelAction::OpenCategoryPicker);
    }

    // --- Text input routing ---

    #[test]
    fn char_consumed_in_text_focus() {
        let mut p = add_panel();
        p.focus = InputPanelFocus::Text;
        assert_eq!(p.handle_key(KeyCode::Char('x')), InputPanelAction::Handled);
        assert_eq!(p.text.text(), "x");
    }

    #[test]
    fn char_consumed_in_note_focus() {
        let mut p = add_panel();
        p.focus = InputPanelFocus::Note;
        assert_eq!(p.handle_key(KeyCode::Char('y')), InputPanelAction::Handled);
        assert_eq!(p.note.text(), "y");
    }

    #[test]
    fn char_not_consumed_on_save_button() {
        let mut p = add_panel();
        p.focus = InputPanelFocus::SaveButton;
        assert_eq!(p.handle_key(KeyCode::Char('z')), InputPanelAction::Unhandled);
        assert!(p.text.is_empty());
    }

    #[test]
    fn enter_in_text_focus_not_consumed() {
        // Enter in text field is NOT consumed by the panel (no save-on-Enter from text)
        let mut p = add_panel();
        p.focus = InputPanelFocus::Text;
        // Enter is handled as focus navigation (Tab handles it); Enter in text is Unhandled
        // (the caller decides: if text, do nothing special; Enter only does something on buttons)
        // Actually in handle_focus_navigation we only handle Tab/BackTab/Esc.
        // Enter in text focus falls through to active_buffer_mut().handle_key(Enter, false)
        // TextBuffer::handle_key for Enter with multiline=false returns false.
        let action = p.handle_key(KeyCode::Enter);
        assert_eq!(action, InputPanelAction::Unhandled);
    }

    #[test]
    fn enter_in_note_focus_inserts_newline() {
        let mut p = add_panel();
        p.focus = InputPanelFocus::Note;
        p.handle_key(KeyCode::Char('a'));
        let action = p.handle_key(KeyCode::Enter);
        assert_eq!(action, InputPanelAction::Handled);
        assert_eq!(p.note.text(), "a\n");
    }

    // --- Category picker overlay ---

    #[test]
    fn category_picker_starts_closed() {
        let p = add_panel();
        assert!(!p.category_picker_open());
        assert!(p.picker_index().is_none());
    }

    #[test]
    fn open_and_close_category_picker() {
        let mut p = add_panel();
        p.open_category_picker(3);
        assert!(p.category_picker_open());
        assert_eq!(p.picker_index(), Some(3));
        p.close_category_picker();
        assert!(!p.category_picker_open());
        assert!(p.picker_index().is_none());
    }

    #[test]
    fn move_picker_cursor_wraps() {
        let mut p = add_panel();
        p.open_category_picker(0);
        p.move_picker_cursor(5, 1);
        assert_eq!(p.picker_index(), Some(1));
        p.move_picker_cursor(5, -1);
        assert_eq!(p.picker_index(), Some(0));
        // Wrap backward from 0
        p.move_picker_cursor(5, -1);
        assert_eq!(p.picker_index(), Some(4));
    }

    #[test]
    fn move_picker_cursor_noop_on_empty_list() {
        let mut p = add_panel();
        p.open_category_picker(0);
        p.move_picker_cursor(0, 1); // no-op
        assert_eq!(p.picker_index(), Some(0));
    }

    #[test]
    fn toggle_category_adds_and_removes() {
        let mut p = add_panel();
        let cat_id = CategoryId::new_v4();
        assert!(!p.categories.contains(&cat_id));
        p.toggle_category(cat_id);
        assert!(p.categories.contains(&cat_id));
        p.toggle_category(cat_id);
        assert!(!p.categories.contains(&cat_id));
    }

    // --- Constructor checks ---

    #[test]
    fn new_add_item_has_empty_fields() {
        let p = InputPanel::new_add_item("Open", &HashSet::new());
        assert_eq!(p.kind, InputPanelKind::AddItem);
        assert!(p.text.is_empty());
        assert!(p.note.is_empty());
        assert!(p.categories.is_empty());
        assert!(p.item_id.is_none());
        assert!(p.preview_context.contains("Open"));
        assert!(!p.category_picker_open());
    }

    #[test]
    fn new_add_item_context_mentions_auto_assign() {
        let mut cats = HashSet::new();
        cats.insert(CategoryId::new_v4());
        cats.insert(CategoryId::new_v4());
        let p = InputPanel::new_add_item("Backlog", &cats);
        assert!(p.preview_context.contains("2 categories"), "got: {}", p.preview_context);
    }

    #[test]
    fn new_edit_item_prefills_fields() {
        let id = ItemId::new_v4();
        let mut cats = HashSet::new();
        cats.insert(CategoryId::new_v4());
        let p = InputPanel::new_edit_item(id, "My item".into(), "My note".into(), cats.clone());
        assert_eq!(p.kind, InputPanelKind::EditItem);
        assert_eq!(p.text.text(), "My item");
        assert_eq!(p.note.text(), "My note");
        assert_eq!(p.categories, cats);
        assert_eq!(p.item_id, Some(id));
    }

    #[test]
    fn new_name_input_prefills_text() {
        let p = InputPanel::new_name_input("Old Name", "View name");
        assert_eq!(p.kind, InputPanelKind::NameInput);
        assert_eq!(p.text.text(), "Old Name");
        assert_eq!(p.preview_context, "View name");
    }
}
