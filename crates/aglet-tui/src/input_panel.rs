use std::collections::{HashMap, HashSet};

use aglet_core::classification::ClassificationSuggestion;
use aglet_core::model::{CategoryId, CategoryValueKind, ItemId, RecurrenceRule};
#[cfg(test)]
use crossterm::event::KeyModifiers;
use crossterm::event::{KeyCode, KeyEvent};
use rust_decimal::Decimal;

use crate::text_buffer::TextBuffer;
use crate::SuggestionDecision;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum InputPanelKind {
    /// New item in the current section context.
    AddItem,
    /// Edit an existing item.
    EditItem,
    /// Single text field for naming (views, categories).
    NameInput,
    /// Single text field for editing When datetime text.
    WhenDate,
    /// Single text field for editing a numeric value.
    NumericValue,
    /// Category creation: Name + Type picker.
    CategoryCreate,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum InputPanelFocus {
    Text,
    When,
    Note,
    Categories,
    Actions,
    Suggestions,
    /// Tag/Numeric toggle (CategoryCreate only).
    TypePicker,
}

/// Action returned by InputPanel::handle_key. The caller interprets each action.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum InputPanelAction {
    /// Focus moved forward (Tab).
    FocusNext,
    /// Focus moved backward (Shift-Tab).
    FocusPrev,
    /// Toggle the category at the current cursor position.
    ToggleCategory,
    /// Move the category cursor by a delta.
    MoveCategoryCursor(i32),
    /// Save / submit the panel contents.
    Save,
    /// Cancel / close the panel without saving.
    Cancel,
    /// Toggle the value kind between Tag/Numeric (CategoryCreate).
    ToggleType,
    /// A text key was consumed internally.
    Handled,
    /// The key was not consumed.
    Unhandled,
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
    /// Descriptive context shown in a static panel row (section + auto-assign preview).
    pub(crate) preview_context: String,
    /// Cursor position within the category list.
    pub(crate) category_cursor: usize,
    /// Cursor position within the edit-panel actions list.
    pub(crate) action_cursor: usize,
    /// Inline filter text for narrowing the categories list in add/edit panels.
    pub(crate) category_filter: TextBuffer,
    /// Whether category filter input is actively focused for typing.
    pub(crate) category_filter_editing: bool,
    /// Editing buffers for assigned numeric categories.
    /// Created when a numeric category is toggled on; removed when toggled off.
    pub(crate) numeric_buffers: HashMap<CategoryId, TextBuffer>,
    /// Original numeric values for change detection (populated when opening EditItem).
    pub(crate) numeric_originals: HashMap<CategoryId, Option<Decimal>>,
    /// Parent category for CategoryCreate (None = root).
    pub(crate) parent_id: Option<CategoryId>,
    /// Display name for the parent (cached for rendering).
    pub(crate) parent_label: String,
    /// Value kind selection for CategoryCreate.
    pub(crate) value_kind: CategoryValueKind,
    /// Pending classification suggestions for the item (edit panel only).
    /// Each entry pairs a suggestion with a three-state decision.
    pub(crate) pending_suggestions: Vec<(ClassificationSuggestion, SuggestionDecision)>,
    /// When-date editor buffer (AddItem/EditItem only).
    pub(crate) when_buffer: TextBuffer,
    /// Parsed recurrence rule from the When field (set by recalculate_input_panel_when).
    pub(crate) parsed_recurrence_rule: Option<RecurrenceRule>,
    /// Scroll offset for the EditItem details popup.
    pub(crate) details_scroll: usize,
    /// Whether the EditItem details popup is currently open.
    pub(crate) details_popup_open: bool,
    /// Whether a discard-confirm prompt is active (AddItem/EditItem only).
    pub(crate) discard_confirm: bool,
    // --- Original values for dirty tracking ---
    original_text: String,
    original_note: String,
    original_categories: HashSet<CategoryId>,
    pub(crate) original_when: String,
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
            categories: on_insert_assign.clone(),
            focus: InputPanelFocus::Text,
            item_id: None,
            preview_context: format_section_context(section_title, on_insert_assign),
            category_cursor: 0,
            action_cursor: 0,
            category_filter: TextBuffer::empty(),
            category_filter_editing: false,
            numeric_buffers: HashMap::new(),
            numeric_originals: HashMap::new(),
            parent_id: None,
            parent_label: String::new(),
            value_kind: CategoryValueKind::Tag,
            pending_suggestions: Vec::new(),
            when_buffer: TextBuffer::empty(),
            parsed_recurrence_rule: None,
            details_scroll: 0,
            details_popup_open: false,
            discard_confirm: false,
            original_text: String::new(),
            original_note: String::new(),
            original_categories: on_insert_assign.clone(),
            original_when: String::new(),
        }
    }

    pub(crate) fn new_edit_item(
        item_id: ItemId,
        text: String,
        note: String,
        when_value: String,
        categories: HashSet<CategoryId>,
        numeric_buffers: HashMap<CategoryId, TextBuffer>,
        numeric_originals: HashMap<CategoryId, Option<Decimal>>,
    ) -> Self {
        let original_text = text.clone();
        let original_note = note.clone();
        let original_categories = categories.clone();
        let original_when = when_value.clone();
        Self {
            kind: InputPanelKind::EditItem,
            text: TextBuffer::new(text),
            note: TextBuffer::new(note),
            categories,
            focus: InputPanelFocus::Text,
            item_id: Some(item_id),
            preview_context: String::new(),
            category_cursor: 0,
            action_cursor: 0,
            category_filter: TextBuffer::empty(),
            category_filter_editing: false,
            numeric_buffers,
            numeric_originals,
            parent_id: None,
            parent_label: String::new(),
            value_kind: CategoryValueKind::Tag,
            pending_suggestions: Vec::new(),
            when_buffer: TextBuffer::new(when_value),
            parsed_recurrence_rule: None,
            details_scroll: 0,
            details_popup_open: false,
            discard_confirm: false,
            original_text,
            original_note,
            original_categories,
            original_when,
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
            category_cursor: 0,
            action_cursor: 0,
            category_filter: TextBuffer::empty(),
            category_filter_editing: false,
            numeric_buffers: HashMap::new(),
            numeric_originals: HashMap::new(),
            parent_id: None,
            parent_label: String::new(),
            value_kind: CategoryValueKind::Tag,
            pending_suggestions: Vec::new(),
            when_buffer: TextBuffer::empty(),
            parsed_recurrence_rule: None,
            details_scroll: 0,
            details_popup_open: false,
            discard_confirm: false,
            original_text: current_name.to_string(),
            original_note: String::new(),
            original_categories: HashSet::new(),
            original_when: String::new(),
        }
    }

    pub(crate) fn new_numeric_value_input(current_value: &str, label: &str) -> Self {
        Self {
            kind: InputPanelKind::NumericValue,
            text: TextBuffer::new(current_value.to_string()),
            note: TextBuffer::empty(),
            categories: HashSet::new(),
            focus: InputPanelFocus::Text,
            item_id: None,
            preview_context: label.to_string(),
            category_cursor: 0,
            action_cursor: 0,
            category_filter: TextBuffer::empty(),
            category_filter_editing: false,
            numeric_buffers: HashMap::new(),
            numeric_originals: HashMap::new(),
            parent_id: None,
            parent_label: String::new(),
            value_kind: CategoryValueKind::Tag,
            pending_suggestions: Vec::new(),
            when_buffer: TextBuffer::empty(),
            parsed_recurrence_rule: None,
            details_scroll: 0,
            details_popup_open: false,
            discard_confirm: false,
            original_text: current_value.to_string(),
            original_note: String::new(),
            original_categories: HashSet::new(),
            original_when: String::new(),
        }
    }

    pub(crate) fn new_when_date_input(current_value: &str, label: &str) -> Self {
        Self {
            kind: InputPanelKind::WhenDate,
            text: TextBuffer::new(current_value.to_string()),
            note: TextBuffer::empty(),
            categories: HashSet::new(),
            focus: InputPanelFocus::Text,
            item_id: None,
            preview_context: label.to_string(),
            category_cursor: 0,
            action_cursor: 0,
            category_filter: TextBuffer::empty(),
            category_filter_editing: false,
            numeric_buffers: HashMap::new(),
            numeric_originals: HashMap::new(),
            parent_id: None,
            parent_label: String::new(),
            value_kind: CategoryValueKind::Tag,
            pending_suggestions: Vec::new(),
            when_buffer: TextBuffer::empty(),
            parsed_recurrence_rule: None,
            details_scroll: 0,
            details_popup_open: false,
            discard_confirm: false,
            original_text: current_value.to_string(),
            original_note: String::new(),
            original_categories: HashSet::new(),
            original_when: String::new(),
        }
    }

    pub(crate) fn new_category_create(parent_id: Option<CategoryId>, parent_label: &str) -> Self {
        Self {
            kind: InputPanelKind::CategoryCreate,
            text: TextBuffer::empty(),
            note: TextBuffer::empty(),
            categories: HashSet::new(),
            focus: InputPanelFocus::Text,
            item_id: None,
            preview_context: String::new(),
            category_cursor: 0,
            action_cursor: 0,
            category_filter: TextBuffer::empty(),
            category_filter_editing: false,
            numeric_buffers: HashMap::new(),
            numeric_originals: HashMap::new(),
            parent_id,
            parent_label: parent_label.to_string(),
            value_kind: CategoryValueKind::Tag,
            pending_suggestions: Vec::new(),
            when_buffer: TextBuffer::empty(),
            parsed_recurrence_rule: None,
            details_scroll: 0,
            details_popup_open: false,
            discard_confirm: false,
            original_text: String::new(),
            original_note: String::new(),
            original_categories: HashSet::new(),
            original_when: String::new(),
        }
    }

    /// Returns true if any field differs from its original value.
    pub(crate) fn is_dirty(&self) -> bool {
        if self.text.text() != self.original_text {
            return true;
        }
        if self.note.text() != self.original_note {
            return true;
        }
        if self.categories != self.original_categories {
            return true;
        }
        // Check numeric buffers against originals
        for (cat_id, buf) in &self.numeric_buffers {
            let current: Option<Decimal> = buf.text().trim().parse().ok();
            let original = self.numeric_originals.get(cat_id).copied().flatten();
            if current != original {
                return true;
            }
        }
        false
    }

    /// Toggles `category_id` in the panel's category set.
    pub(crate) fn toggle_category(&mut self, category_id: CategoryId) {
        if self.categories.contains(&category_id) {
            self.categories.remove(&category_id);
        } else {
            self.categories.insert(category_id);
        }
    }

    /// Handle a key event. Returns the action the caller should perform.
    /// `current_row_is_assigned_numeric` tells whether the
    /// category row at the cursor is an assigned numeric category (for key routing).
    pub(crate) fn handle_key_event(
        &mut self,
        key: KeyEvent,
        current_row_is_assigned_numeric: bool,
    ) -> InputPanelAction {
        let code = key.code;
        if let Some(action) = self.handle_focus_navigation(code, current_row_is_assigned_numeric) {
            return action;
        }

        match self.focus {
            InputPanelFocus::Text | InputPanelFocus::Note | InputPanelFocus::When => {
                let multiline = self.focus == InputPanelFocus::Note;
                let buffer = self.active_buffer_mut();
                if buffer.handle_key_event(key, multiline) {
                    InputPanelAction::Handled
                } else {
                    InputPanelAction::Unhandled
                }
            }
            InputPanelFocus::Categories => {
                self.handle_categories_focus(code, current_row_is_assigned_numeric)
            }
            InputPanelFocus::Actions | InputPanelFocus::Suggestions => {
                self.handle_edit_sidebar_focus(code)
            }
            InputPanelFocus::TypePicker => self.handle_type_picker_focus(code),
        }
    }

    fn handle_focus_navigation(
        &mut self,
        code: KeyCode,
        current_row_is_assigned_numeric: bool,
    ) -> Option<InputPanelAction> {
        match code {
            KeyCode::Tab => {
                self.cycle_focus_forward();
                Some(InputPanelAction::FocusNext)
            }
            KeyCode::BackTab => {
                self.cycle_focus_backward();
                Some(InputPanelAction::FocusPrev)
            }
            // Esc cancels (caller decides whether to prompt dirty-confirm)
            KeyCode::Esc => Some(InputPanelAction::Cancel),
            // Capital S saves only when not editing text fields or numeric value buffers
            KeyCode::Char('S')
                if !(matches!(
                    self.focus,
                    InputPanelFocus::Text | InputPanelFocus::Note | InputPanelFocus::When
                ) || matches!(
                    self.focus,
                    InputPanelFocus::Categories
                        | InputPanelFocus::Actions
                        | InputPanelFocus::Suggestions
                ) && current_row_is_assigned_numeric) =>
            {
                Some(InputPanelAction::Save)
            }
            // Single-value editors: Enter from text field saves directly.
            // The When field is handled separately so Enter can recalculate in place.
            KeyCode::Enter if matches!(self.focus, InputPanelFocus::Text) => {
                Some(InputPanelAction::Save)
            }
            _ => None,
        }
    }

    fn handle_categories_focus(
        &mut self,
        code: KeyCode,
        current_row_is_assigned_numeric: bool,
    ) -> InputPanelAction {
        match code {
            KeyCode::Down | KeyCode::Char('j') => InputPanelAction::MoveCategoryCursor(1),
            KeyCode::Up | KeyCode::Char('k') => InputPanelAction::MoveCategoryCursor(-1),
            KeyCode::Char(' ') => InputPanelAction::ToggleCategory,
            KeyCode::Enter => {
                if current_row_is_assigned_numeric {
                    // No-op on numeric row to avoid accidental save
                    InputPanelAction::Handled
                } else {
                    // On tag row, Enter acts as toggle
                    InputPanelAction::ToggleCategory
                }
            }
            _ => {
                // Route printable chars and editing keys to numeric buffer if on assigned numeric row
                if current_row_is_assigned_numeric {
                    match code {
                        KeyCode::Char(_)
                        | KeyCode::Backspace
                        | KeyCode::Delete
                        | KeyCode::Left
                        | KeyCode::Right
                        | KeyCode::Home
                        | KeyCode::End => {
                            // The caller will route this to the appropriate TextBuffer
                            InputPanelAction::Handled
                        }
                        _ => InputPanelAction::Unhandled,
                    }
                } else {
                    InputPanelAction::Unhandled
                }
            }
        }
    }

    fn handle_edit_sidebar_focus(&mut self, code: KeyCode) -> InputPanelAction {
        match code {
            KeyCode::Down | KeyCode::Char('j') => InputPanelAction::MoveCategoryCursor(1),
            KeyCode::Up | KeyCode::Char('k') => InputPanelAction::MoveCategoryCursor(-1),
            KeyCode::Char(' ') | KeyCode::Enter => InputPanelAction::ToggleCategory,
            _ => InputPanelAction::Unhandled,
        }
    }

    fn handle_type_picker_focus(&mut self, code: KeyCode) -> InputPanelAction {
        match code {
            KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Left | KeyCode::Right => {
                InputPanelAction::ToggleType
            }
            _ => InputPanelAction::Unhandled,
        }
    }

    fn active_buffer_mut(&mut self) -> &mut TextBuffer {
        match self.focus {
            InputPanelFocus::Note => &mut self.note,
            InputPanelFocus::When => &mut self.when_buffer,
            _ => &mut self.text,
        }
    }

    #[cfg(test)]
    pub(crate) fn handle_key(
        &mut self,
        code: KeyCode,
        current_row_is_assigned_numeric: bool,
    ) -> InputPanelAction {
        self.handle_key_event(
            KeyEvent::new(code, KeyModifiers::NONE),
            current_row_is_assigned_numeric,
        )
    }

    pub(crate) fn cycle_focus_forward(&mut self) {
        self.focus = match self.focus {
            InputPanelFocus::Text => match self.kind {
                InputPanelKind::NameInput
                | InputPanelKind::WhenDate
                | InputPanelKind::NumericValue => InputPanelFocus::Text, // single field, no-op
                InputPanelKind::CategoryCreate => InputPanelFocus::TypePicker,
                InputPanelKind::AddItem | InputPanelKind::EditItem => InputPanelFocus::When,
            },
            InputPanelFocus::When => InputPanelFocus::Note,
            InputPanelFocus::Note => match self.kind {
                InputPanelKind::EditItem => InputPanelFocus::Actions,
                _ => InputPanelFocus::Categories,
            },
            InputPanelFocus::Actions => {
                if self.kind == InputPanelKind::EditItem && !self.pending_suggestions.is_empty() {
                    InputPanelFocus::Suggestions
                } else {
                    InputPanelFocus::Text
                }
            }
            InputPanelFocus::Suggestions => InputPanelFocus::Text,
            InputPanelFocus::Categories => InputPanelFocus::Text,
            InputPanelFocus::TypePicker => InputPanelFocus::Text,
        };
    }

    pub(crate) fn cycle_focus_backward(&mut self) {
        self.focus = match self.focus {
            InputPanelFocus::Text => match self.kind {
                InputPanelKind::NameInput
                | InputPanelKind::WhenDate
                | InputPanelKind::NumericValue => InputPanelFocus::Text, // single field, no-op
                InputPanelKind::CategoryCreate => InputPanelFocus::TypePicker,
                InputPanelKind::EditItem if !self.pending_suggestions.is_empty() => {
                    InputPanelFocus::Suggestions
                }
                InputPanelKind::EditItem => InputPanelFocus::Actions,
                _ => InputPanelFocus::Categories,
            },
            InputPanelFocus::When => InputPanelFocus::Text,
            InputPanelFocus::Note => InputPanelFocus::When,
            InputPanelFocus::Categories => InputPanelFocus::Note,
            InputPanelFocus::Actions => InputPanelFocus::Note,
            InputPanelFocus::Suggestions => InputPanelFocus::Actions,
            InputPanelFocus::TypePicker => InputPanelFocus::Text,
        };
    }
}

fn format_section_context(section_title: &str, on_insert_assign: &HashSet<CategoryId>) -> String {
    format!(
        "Adding to \"{}\" (auto-assign {} categories)",
        section_title,
        on_insert_assign.len()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn add_panel() -> InputPanel {
        InputPanel::new_add_item("Open", &HashSet::new())
    }

    fn name_panel() -> InputPanel {
        InputPanel::new_name_input("", "View name")
    }

    fn numeric_value_panel() -> InputPanel {
        InputPanel::new_numeric_value_input("12.50", "Cost")
    }

    // --- Focus cycling ---

    #[test]
    fn tab_cycles_add_panel_forward() {
        let mut p = add_panel();
        assert_eq!(p.focus, InputPanelFocus::Text);
        p.handle_key(KeyCode::Tab, false);
        assert_eq!(p.focus, InputPanelFocus::When);
        p.handle_key(KeyCode::Tab, false);
        assert_eq!(p.focus, InputPanelFocus::Note);
        p.handle_key(KeyCode::Tab, false);
        assert_eq!(p.focus, InputPanelFocus::Categories);
        // Categories wraps back to Text (no Save/Cancel buttons)
        p.handle_key(KeyCode::Tab, false);
        assert_eq!(p.focus, InputPanelFocus::Text);
    }

    #[test]
    fn backtab_cycles_add_panel_backward() {
        let mut p = add_panel();
        // Text wraps to Categories (no Save/Cancel buttons)
        p.handle_key(KeyCode::BackTab, false);
        assert_eq!(p.focus, InputPanelFocus::Categories);
        p.handle_key(KeyCode::BackTab, false);
        assert_eq!(p.focus, InputPanelFocus::Note);
        p.handle_key(KeyCode::BackTab, false);
        assert_eq!(p.focus, InputPanelFocus::When);
        p.handle_key(KeyCode::BackTab, false);
        assert_eq!(p.focus, InputPanelFocus::Text);
    }

    #[test]
    fn name_panel_tab_stays_on_text() {
        let mut p = name_panel();
        // Single-field panel: Tab is a no-op
        p.handle_key(KeyCode::Tab, false);
        assert_eq!(p.focus, InputPanelFocus::Text);
    }

    #[test]
    fn name_panel_backtab_stays_on_text() {
        let mut p = name_panel();
        p.handle_key(KeyCode::BackTab, false);
        assert_eq!(p.focus, InputPanelFocus::Text);
    }

    #[test]
    fn numeric_value_panel_tab_stays_on_text() {
        let mut p = numeric_value_panel();
        p.handle_key(KeyCode::Tab, false);
        assert_eq!(p.focus, InputPanelFocus::Text);
    }

    // --- Esc cancels from any focus ---

    #[test]
    fn esc_returns_cancel_from_any_focus() {
        let mut p = add_panel();
        for focus in [
            InputPanelFocus::Text,
            InputPanelFocus::Note,
            InputPanelFocus::Categories,
        ] {
            p.focus = focus;
            assert_eq!(
                p.handle_key(KeyCode::Esc, false),
                InputPanelAction::Cancel,
                "expected Cancel at focus {:?}",
                focus
            );
        }
    }

    // --- Categories focus ---

    #[test]
    fn space_on_categories_returns_toggle() {
        let mut p = add_panel();
        p.focus = InputPanelFocus::Categories;
        assert_eq!(
            p.handle_key(KeyCode::Char(' '), false),
            InputPanelAction::ToggleCategory
        );
    }

    #[test]
    fn j_k_on_categories_returns_cursor_move() {
        let mut p = add_panel();
        p.focus = InputPanelFocus::Categories;
        assert_eq!(
            p.handle_key(KeyCode::Char('j'), false),
            InputPanelAction::MoveCategoryCursor(1)
        );
        assert_eq!(
            p.handle_key(KeyCode::Char('k'), false),
            InputPanelAction::MoveCategoryCursor(-1)
        );
    }

    #[test]
    fn enter_on_tag_row_toggles() {
        let mut p = add_panel();
        p.focus = InputPanelFocus::Categories;
        assert_eq!(
            p.handle_key(KeyCode::Enter, false),
            InputPanelAction::ToggleCategory
        );
    }

    #[test]
    fn enter_on_numeric_row_is_handled_noop() {
        let mut p = add_panel();
        p.focus = InputPanelFocus::Categories;
        assert_eq!(
            p.handle_key(KeyCode::Enter, true),
            InputPanelAction::Handled
        );
    }

    #[test]
    fn typing_on_numeric_row_is_handled() {
        let mut p = add_panel();
        p.focus = InputPanelFocus::Categories;
        assert_eq!(
            p.handle_key(KeyCode::Char('5'), true),
            InputPanelAction::Handled
        );
    }

    #[test]
    fn typing_on_tag_row_is_unhandled() {
        let mut p = add_panel();
        p.focus = InputPanelFocus::Categories;
        assert_eq!(
            p.handle_key(KeyCode::Char('5'), false),
            InputPanelAction::Unhandled
        );
    }

    #[test]
    fn capital_s_saves_from_categories_when_not_numeric() {
        let mut p = add_panel();
        p.focus = InputPanelFocus::Categories;
        assert_eq!(
            p.handle_key(KeyCode::Char('S'), false),
            InputPanelAction::Save
        );
    }

    #[test]
    fn capital_s_does_not_save_from_numeric_row() {
        let mut p = add_panel();
        p.focus = InputPanelFocus::Categories;
        // When on assigned numeric row, S should be routed as text input (Handled)
        assert_eq!(
            p.handle_key(KeyCode::Char('S'), true),
            InputPanelAction::Handled
        );
    }

    // --- Text input routing ---

    #[test]
    fn char_consumed_in_text_focus() {
        let mut p = add_panel();
        p.focus = InputPanelFocus::Text;
        assert_eq!(
            p.handle_key(KeyCode::Char('x'), false),
            InputPanelAction::Handled
        );
        assert_eq!(p.text.text(), "x");
    }

    #[test]
    fn char_consumed_in_note_focus() {
        let mut p = add_panel();
        p.focus = InputPanelFocus::Note;
        assert_eq!(
            p.handle_key(KeyCode::Char('y'), false),
            InputPanelAction::Handled
        );
        assert_eq!(p.note.text(), "y");
    }

    #[test]
    fn enter_in_text_focus_saves() {
        let mut p = add_panel();
        p.focus = InputPanelFocus::Text;
        let action = p.handle_key(KeyCode::Enter, false);
        assert_eq!(action, InputPanelAction::Save);
    }

    #[test]
    fn enter_in_numeric_value_text_focus_saves() {
        let mut p = numeric_value_panel();
        p.focus = InputPanelFocus::Text;
        assert_eq!(p.handle_key(KeyCode::Enter, false), InputPanelAction::Save);
    }

    #[test]
    fn enter_in_name_input_text_focus_saves() {
        let mut p = name_panel();
        p.focus = InputPanelFocus::Text;
        assert_eq!(p.handle_key(KeyCode::Enter, false), InputPanelAction::Save);
    }

    #[test]
    fn enter_in_note_focus_inserts_newline() {
        let mut p = add_panel();
        p.focus = InputPanelFocus::Note;
        p.handle_key(KeyCode::Char('a'), false);
        let action = p.handle_key(KeyCode::Enter, false);
        assert_eq!(action, InputPanelAction::Handled);
        assert_eq!(p.note.text(), "a\n");
    }

    // --- Category toggle ---

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

    // --- Numeric buffer management ---

    #[test]
    fn numeric_buffer_toggle_on_creates_buffer() {
        let mut p = add_panel();
        let cat_id = CategoryId::new_v4();
        p.categories.insert(cat_id);
        p.numeric_buffers.insert(cat_id, TextBuffer::empty());
        assert!(p.numeric_buffers.contains_key(&cat_id));
    }

    #[test]
    fn numeric_buffer_toggle_off_removes_buffer() {
        let mut p = add_panel();
        let cat_id = CategoryId::new_v4();
        p.categories.insert(cat_id);
        p.numeric_buffers.insert(cat_id, TextBuffer::empty());
        p.categories.remove(&cat_id);
        p.numeric_buffers.remove(&cat_id);
        assert!(!p.numeric_buffers.contains_key(&cat_id));
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
        assert!(p.preview_context.contains("auto-assign 0 categories"));
    }

    #[test]
    fn new_add_item_context_mentions_auto_assign() {
        let mut cats = HashSet::new();
        cats.insert(CategoryId::new_v4());
        cats.insert(CategoryId::new_v4());
        let p = InputPanel::new_add_item("Backlog", &cats);
        assert!(
            p.preview_context.contains("2 categories"),
            "got: {}",
            p.preview_context
        );
    }

    #[test]
    fn new_edit_item_prefills_fields() {
        let id = ItemId::new_v4();
        let mut cats = HashSet::new();
        cats.insert(CategoryId::new_v4());
        let p = InputPanel::new_edit_item(
            id,
            "My item".into(),
            "My note".into(),
            String::new(),
            cats.clone(),
            HashMap::new(),
            HashMap::new(),
        );
        assert_eq!(p.kind, InputPanelKind::EditItem);
        assert_eq!(p.text.text(), "My item");
        assert_eq!(p.note.text(), "My note");
        assert_eq!(p.categories, cats);
        assert_eq!(p.item_id, Some(id));
        assert_eq!(p.details_scroll, 0);
        assert!(!p.details_popup_open);
    }

    #[test]
    fn edit_item_tab_cycles_directly_to_actions() {
        let mut p = InputPanel::new_edit_item(
            ItemId::new_v4(),
            "My item".into(),
            "My note".into(),
            String::new(),
            HashSet::new(),
            HashMap::new(),
            HashMap::new(),
        );

        assert_eq!(p.focus, InputPanelFocus::Text);
        p.handle_key(KeyCode::Tab, false);
        assert_eq!(p.focus, InputPanelFocus::When);
        p.handle_key(KeyCode::Tab, false);
        assert_eq!(p.focus, InputPanelFocus::Note);
        p.handle_key(KeyCode::Tab, false);
        assert_eq!(p.focus, InputPanelFocus::Actions);
    }

    #[test]
    fn edit_item_backtab_returns_from_actions_to_note() {
        let mut p = InputPanel::new_edit_item(
            ItemId::new_v4(),
            "My item".into(),
            "My note".into(),
            String::new(),
            HashSet::new(),
            HashMap::new(),
            HashMap::new(),
        );
        p.focus = InputPanelFocus::Actions;
        p.handle_key(KeyCode::BackTab, false);
        assert_eq!(p.focus, InputPanelFocus::Note);
    }

    #[test]
    fn edit_item_tab_and_backtab_include_suggestions_when_present() {
        let mut p = InputPanel::new_edit_item(
            ItemId::new_v4(),
            "My item".into(),
            "My note".into(),
            String::new(),
            HashSet::new(),
            HashMap::new(),
            HashMap::new(),
        );
        p.pending_suggestions.push((
            ClassificationSuggestion {
                id: uuid::Uuid::new_v4(),
                item_id: ItemId::new_v4(),
                assignment: aglet_core::classification::CandidateAssignment::Category(
                    CategoryId::new_v4(),
                ),
                provider_id: "test".into(),
                model: None,
                confidence: None,
                rationale: None,
                status: aglet_core::classification::SuggestionStatus::Pending,
                context_hash: "ctx".into(),
                item_revision_hash: "rev".into(),
                created_at: crate::Timestamp::now(),
                decided_at: None,
            },
            SuggestionDecision::Pending,
        ));

        p.focus = InputPanelFocus::Note;
        p.handle_key(KeyCode::Tab, false);
        assert_eq!(p.focus, InputPanelFocus::Actions);
        p.handle_key(KeyCode::Tab, false);
        assert_eq!(p.focus, InputPanelFocus::Suggestions);
        p.handle_key(KeyCode::BackTab, false);
        assert_eq!(p.focus, InputPanelFocus::Actions);
    }

    #[test]
    fn new_name_input_prefills_text() {
        let p = InputPanel::new_name_input("Old Name", "View name");
        assert_eq!(p.kind, InputPanelKind::NameInput);
        assert_eq!(p.text.text(), "Old Name");
        assert_eq!(p.preview_context, "View name");
    }

    #[test]
    fn new_numeric_value_input_prefills_text() {
        let p = InputPanel::new_numeric_value_input("12.5", "Cost");
        assert_eq!(p.kind, InputPanelKind::NumericValue);
        assert_eq!(p.text.text(), "12.5");
        assert_eq!(p.preview_context, "Cost");
    }

    #[test]
    fn new_when_date_input_prefills_text() {
        let p = InputPanel::new_when_date_input("tomorrow 3pm", "When date for: Demo");
        assert_eq!(p.kind, InputPanelKind::WhenDate);
        assert_eq!(p.text.text(), "tomorrow 3pm");
        assert_eq!(p.preview_context, "When date for: Demo");
    }

    #[test]
    fn is_dirty_detects_text_change() {
        let mut p = InputPanel::new_name_input("Hello", "label");
        assert!(!p.is_dirty());
        p.text.set("Hello!".to_string());
        assert!(p.is_dirty());
    }

    #[test]
    fn is_dirty_detects_category_change() {
        let cat_id = CategoryId::new_v4();
        let mut cats = std::collections::HashSet::new();
        cats.insert(cat_id);
        let p = InputPanel::new_edit_item(
            ItemId::new_v4(),
            "text".to_string(),
            String::new(),
            String::new(),
            cats,
            std::collections::HashMap::new(),
            std::collections::HashMap::new(),
        );
        assert!(!p.is_dirty());
        let mut p2 = p.clone();
        p2.categories.remove(&cat_id);
        assert!(p2.is_dirty());
    }

    #[test]
    fn add_item_dirty_on_any_input() {
        let p = InputPanel::new_add_item("Section", &std::collections::HashSet::new());
        assert!(!p.is_dirty());
        let mut p2 = p.clone();
        p2.text.set("something".to_string());
        assert!(p2.is_dirty());
    }

    // --- CategoryCreate focus cycling ---

    fn cat_create_panel() -> InputPanel {
        InputPanel::new_category_create(None, "top level")
    }

    #[test]
    fn category_create_tab_cycles_forward() {
        let mut p = cat_create_panel();
        assert_eq!(p.focus, InputPanelFocus::Text);
        p.handle_key(KeyCode::Tab, false);
        assert_eq!(p.focus, InputPanelFocus::TypePicker);
        // TypePicker wraps back to Text (no buttons)
        p.handle_key(KeyCode::Tab, false);
        assert_eq!(p.focus, InputPanelFocus::Text);
    }

    #[test]
    fn category_create_backtab_cycles_backward() {
        let mut p = cat_create_panel();
        // Text wraps to TypePicker (no buttons)
        p.handle_key(KeyCode::BackTab, false);
        assert_eq!(p.focus, InputPanelFocus::TypePicker);
        p.handle_key(KeyCode::BackTab, false);
        assert_eq!(p.focus, InputPanelFocus::Text);
    }

    #[test]
    fn category_create_space_on_type_picker_toggles() {
        let mut p = cat_create_panel();
        p.focus = InputPanelFocus::TypePicker;
        assert_eq!(
            p.handle_key(KeyCode::Char(' '), false),
            InputPanelAction::ToggleType
        );
    }

    #[test]
    fn category_create_arrows_on_type_picker_toggle() {
        let mut p = cat_create_panel();
        p.focus = InputPanelFocus::TypePicker;
        assert_eq!(
            p.handle_key(KeyCode::Left, false),
            InputPanelAction::ToggleType
        );
        assert_eq!(
            p.handle_key(KeyCode::Right, false),
            InputPanelAction::ToggleType
        );
    }

    #[test]
    fn new_category_create_defaults() {
        let p = InputPanel::new_category_create(Some(CategoryId::new_v4()), "Parent");
        assert_eq!(p.kind, InputPanelKind::CategoryCreate);
        assert!(p.text.is_empty());
        assert_eq!(p.parent_label, "Parent");
        assert_eq!(p.value_kind, CategoryValueKind::Tag);
    }
}
