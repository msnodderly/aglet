use std::collections::{HashMap, HashSet};
use std::io;

use agenda_core::agenda::Agenda;
use agenda_core::matcher::{unknown_hashtag_tokens, SubstringClassifier};
use agenda_core::model::{Category, CategoryId, Item, ItemId, Query, Section, View, WhenBucket};
use agenda_core::query::{evaluate_query, resolve_view};
use agenda_core::store::Store;
use chrono::{Local, NaiveDateTime, Utc};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Terminal;

pub fn run(db_path: &std::path::Path) -> Result<(), String> {
    let store = Store::open(db_path).map_err(|e| e.to_string())?;
    let classifier = SubstringClassifier;
    let agenda = Agenda::new(&store, &classifier);

    enable_raw_mode().map_err(|e| e.to_string())?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).map_err(|e| e.to_string())?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).map_err(|e| e.to_string())?;

    let mut app = App::default();
    let result = app.run(&mut terminal, &agenda);

    disable_raw_mode().map_err(|e| e.to_string())?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen).map_err(|e| e.to_string())?;
    terminal.show_cursor().map_err(|e| e.to_string())?;

    result
}

#[derive(Clone)]
enum SlotContext {
    Section {
        section_index: usize,
    },
    GeneratedSection {
        on_insert_assign: HashSet<CategoryId>,
        on_remove_unassign: HashSet<CategoryId>,
    },
    Unmatched,
}

#[derive(Clone)]
struct Slot {
    title: String,
    items: Vec<Item>,
    context: SlotContext,
}

#[derive(Clone)]
struct CategoryListRow {
    id: CategoryId,
    name: String,
    depth: usize,
    is_reserved: bool,
    is_exclusive: bool,
    is_actionable: bool,
    enable_implicit_string: bool,
}

#[derive(Clone)]
struct InspectAssignmentRow {
    category_id: CategoryId,
    category_name: String,
    source_label: String,
    origin_label: String,
}

#[derive(Clone)]
struct ReparentOptionRow {
    parent_id: Option<CategoryId>,
    label: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CategoryEditTarget {
    ViewInclude,
    ViewExclude,
    SectionCriteriaInclude,
    SectionCriteriaExclude,
    SectionOnInsertAssign,
    SectionOnRemoveUnassign,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BucketEditTarget {
    ViewVirtualInclude,
    ViewVirtualExclude,
    SectionVirtualInclude,
    SectionVirtualExclude,
}

#[derive(Clone)]
struct ViewEditorState {
    base_view_name: String,
    draft: View,
    category_index: usize,
    bucket_index: usize,
    section_index: usize,
    action_index: usize,
    preview_count: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    Normal,
    AddInput,
    ItemEditInput,
    NoteEditInput,
    ItemAssignCategoryPicker,
    ItemAssignCategoryInput,
    InspectUnassignPicker,
    FilterInput,
    ViewPicker,
    ViewManagerScreen,
    ViewCreateNameInput,
    ViewCreateCategoryPicker,
    ViewRenameInput,
    ViewDeleteConfirm,
    ViewEditor,
    ViewEditorCategoryPicker,
    ViewEditorBucketPicker,
    ViewManagerCategoryPicker,
    ViewSectionEditor,
    ViewSectionDetail,
    ViewSectionTitleInput,
    ViewUnmatchedSettings,
    ViewUnmatchedLabelInput,
    ConfirmDelete,
    CategoryManager,
    CategoryCreateInput,
    CategoryRenameInput,
    CategoryReparentPicker,
    CategoryDeleteConfirm,
    CategoryConfigEditor,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ViewManagerPane {
    Views,
    Definition,
    Sections,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ViewCriteriaSign {
    Include,
    Exclude,
}

#[derive(Clone)]
struct ViewCriteriaRow {
    sign: ViewCriteriaSign,
    category_id: CategoryId,
    join_is_or: bool,
    depth: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum CategoryConfigFocus {
    Exclusive,
    NoImplicit,
    Actionable,
    Note,
    SaveButton,
    CancelButton,
}

#[derive(Clone)]
struct CategoryConfigEditorState {
    category_id: CategoryId,
    category_name: String,
    is_exclusive: bool,
    is_actionable: bool,
    enable_implicit_string: bool,
    note: String,
    note_cursor: usize,
    focus: CategoryConfigFocus,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ItemEditFocus {
    Text,
    Note,
    CategoriesButton,
    SaveButton,
    CancelButton,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum PreviewMode {
    Summary,
    Provenance,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum NormalFocus {
    Board,
    Preview,
}

struct App {
    mode: Mode,
    status: String,
    input: String,
    input_cursor: usize,
    filter: Option<String>,
    show_preview: bool,
    preview_mode: PreviewMode,
    normal_focus: NormalFocus,
    all_items: Vec<Item>,

    views: Vec<View>,
    view_index: usize,
    picker_index: usize,
    view_pending_name: Option<String>,
    view_pending_edit_name: Option<String>,
    view_category_index: usize,
    view_return_to_manager: bool,
    view_manager_pane: ViewManagerPane,
    view_manager_definition_index: usize,
    view_manager_section_index: usize,
    view_manager_rows: Vec<ViewCriteriaRow>,
    view_manager_loaded_view_name: Option<String>,
    view_manager_preview_count: usize,
    view_manager_dirty: bool,
    view_manager_category_row_index: Option<usize>,
    view_create_include_selection: HashSet<CategoryId>,
    view_create_exclude_selection: HashSet<CategoryId>,
    view_editor: Option<ViewEditorState>,
    view_editor_return_to_manager: bool,
    view_editor_category_target: Option<CategoryEditTarget>,
    view_editor_bucket_target: Option<BucketEditTarget>,

    categories: Vec<Category>,
    category_rows: Vec<CategoryListRow>,
    category_index: usize,
    category_create_parent: Option<CategoryId>,
    category_reparent_options: Vec<ReparentOptionRow>,
    category_reparent_index: usize,
    category_config_editor: Option<CategoryConfigEditorState>,
    item_assign_category_index: usize,
    item_assign_return_to_item_edit: bool,
    item_edit_focus: ItemEditFocus,
    item_edit_note: String,
    item_edit_note_cursor: usize,
    preview_provenance_scroll: usize,
    preview_summary_scroll: usize,
    inspect_assignment_index: usize,
    slots: Vec<Slot>,
    slot_index: usize,
    item_index: usize,
}

impl Default for App {
    fn default() -> Self {
        Self {
            mode: Mode::Normal,
            status:
                "Press n to add, v for view palette, c for category manager, p for preview, q to quit"
                    .to_string(),
            input: String::new(),
            input_cursor: 0,
            filter: None,
            show_preview: false,
            preview_mode: PreviewMode::Summary,
            normal_focus: NormalFocus::Board,
            all_items: Vec::new(),
            views: Vec::new(),
            view_index: 0,
            picker_index: 0,
            view_pending_name: None,
            view_pending_edit_name: None,
            view_category_index: 0,
            view_return_to_manager: false,
            view_manager_pane: ViewManagerPane::Views,
            view_manager_definition_index: 0,
            view_manager_section_index: 0,
            view_manager_rows: Vec::new(),
            view_manager_loaded_view_name: None,
            view_manager_preview_count: 0,
            view_manager_dirty: false,
            view_manager_category_row_index: None,
            view_create_include_selection: HashSet::new(),
            view_create_exclude_selection: HashSet::new(),
            view_editor: None,
            view_editor_return_to_manager: false,
            view_editor_category_target: None,
            view_editor_bucket_target: None,
            categories: Vec::new(),
            category_rows: Vec::new(),
            category_index: 0,
            category_create_parent: None,
            category_reparent_options: Vec::new(),
            category_reparent_index: 0,
            category_config_editor: None,
            item_assign_category_index: 0,
            item_assign_return_to_item_edit: false,
            item_edit_focus: ItemEditFocus::Text,
            item_edit_note: String::new(),
            item_edit_note_cursor: 0,
            preview_provenance_scroll: 0,
            preview_summary_scroll: 0,
            inspect_assignment_index: 0,
            slots: Vec::new(),
            slot_index: 0,
            item_index: 0,
        }
    }
}

impl App {
    fn run(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        agenda: &Agenda<'_>,
    ) -> Result<(), String> {
        self.refresh(agenda.store())?;

        loop {
            terminal
                .draw(|frame| self.draw(frame))
                .map_err(|e| e.to_string())?;

            if !event::poll(std::time::Duration::from_millis(200)).map_err(|e| e.to_string())? {
                continue;
            }

            let Event::Key(key) = event::read().map_err(|e| e.to_string())? else {
                continue;
            };
            if key.kind != KeyEventKind::Press {
                continue;
            }

            let should_quit = match self.handle_key(key.code, agenda) {
                Ok(value) => value,
                Err(err) => {
                    self.mode = Mode::Normal;
                    self.clear_input();
                    self.status = format!("Error: {err}");
                    false
                }
            };
            if should_quit {
                break;
            }
        }

        Ok(())
    }

    fn handle_key(&mut self, code: KeyCode, agenda: &Agenda<'_>) -> Result<bool, String> {
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

    fn set_input(&mut self, value: String) {
        self.input = value;
        self.input_cursor = self.input.chars().count();
    }

    fn clear_input(&mut self) {
        self.input.clear();
        self.input_cursor = 0;
    }

    fn input_len_chars(&self) -> usize {
        self.input.chars().count()
    }

    fn clamped_input_cursor(&self) -> usize {
        self.input_cursor.min(self.input_len_chars())
    }

    fn input_byte_index(&self, char_index: usize) -> usize {
        if char_index == 0 {
            return 0;
        }
        self.input
            .char_indices()
            .nth(char_index)
            .map(|(byte_index, _)| byte_index)
            .unwrap_or(self.input.len())
    }

    fn move_input_cursor_left(&mut self) {
        let cursor = self.clamped_input_cursor();
        self.input_cursor = cursor.saturating_sub(1);
    }

    fn move_input_cursor_right(&mut self) {
        let cursor = self.clamped_input_cursor();
        self.input_cursor = (cursor + 1).min(self.input_len_chars());
    }

    fn move_input_cursor_home(&mut self) {
        self.input_cursor = 0;
    }

    fn move_input_cursor_end(&mut self) {
        self.input_cursor = self.input_len_chars();
    }

    fn backspace_input_char(&mut self) {
        let cursor = self.clamped_input_cursor();
        if cursor == 0 {
            return;
        }
        let start = self.input_byte_index(cursor - 1);
        let end = self.input_byte_index(cursor);
        self.input.replace_range(start..end, "");
        self.input_cursor = cursor - 1;
    }

    fn delete_input_char(&mut self) {
        let cursor = self.clamped_input_cursor();
        if cursor >= self.input_len_chars() {
            return;
        }
        let start = self.input_byte_index(cursor);
        let end = self.input_byte_index(cursor + 1);
        self.input.replace_range(start..end, "");
        self.input_cursor = cursor;
    }

    fn insert_input_char(&mut self, c: char) {
        if c.is_control() {
            return;
        }
        let cursor = self.clamped_input_cursor();
        let byte_index = self.input_byte_index(cursor);
        self.input.insert(byte_index, c);
        self.input_cursor = cursor + 1;
    }

    fn handle_text_input_key(&mut self, code: KeyCode) -> bool {
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

    fn item_edit_note_len_chars(&self) -> usize {
        self.item_edit_note.chars().count()
    }

    fn item_edit_note_byte_index(&self, char_index: usize) -> usize {
        if char_index == 0 {
            return 0;
        }
        self.item_edit_note
            .char_indices()
            .nth(char_index)
            .map(|(byte_index, _)| byte_index)
            .unwrap_or(self.item_edit_note.len())
    }

    fn clamped_item_edit_note_cursor(&self) -> usize {
        self.item_edit_note_cursor
            .min(self.item_edit_note_len_chars())
    }

    fn move_item_edit_note_cursor_left(&mut self) {
        let cursor = self.clamped_item_edit_note_cursor();
        self.item_edit_note_cursor = cursor.saturating_sub(1);
    }

    fn move_item_edit_note_cursor_right(&mut self) {
        let cursor = self.clamped_item_edit_note_cursor();
        self.item_edit_note_cursor = (cursor + 1).min(self.item_edit_note_len_chars());
    }

    fn move_item_edit_note_cursor_home(&mut self) {
        self.item_edit_note_cursor = 0;
    }

    fn move_item_edit_note_cursor_end(&mut self) {
        self.item_edit_note_cursor = self.item_edit_note_len_chars();
    }

    fn move_item_edit_note_cursor_up(&mut self) {
        self.move_item_edit_note_cursor_vertical(-1);
    }

    fn move_item_edit_note_cursor_down(&mut self) {
        self.move_item_edit_note_cursor_vertical(1);
    }

    fn move_item_edit_note_cursor_vertical(&mut self, delta: i32) {
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

    fn backspace_item_edit_note_char(&mut self) {
        let cursor = self.clamped_item_edit_note_cursor();
        if cursor == 0 {
            return;
        }
        let start = self.item_edit_note_byte_index(cursor - 1);
        let end = self.item_edit_note_byte_index(cursor);
        self.item_edit_note.replace_range(start..end, "");
        self.item_edit_note_cursor = cursor - 1;
    }

    fn delete_item_edit_note_char(&mut self) {
        let cursor = self.clamped_item_edit_note_cursor();
        if cursor >= self.item_edit_note_len_chars() {
            return;
        }
        let start = self.item_edit_note_byte_index(cursor);
        let end = self.item_edit_note_byte_index(cursor + 1);
        self.item_edit_note.replace_range(start..end, "");
        self.item_edit_note_cursor = cursor;
    }

    fn insert_item_edit_note_char(&mut self, c: char) {
        if c.is_control() {
            return;
        }
        let cursor = self.clamped_item_edit_note_cursor();
        let byte_index = self.item_edit_note_byte_index(cursor);
        self.item_edit_note.insert(byte_index, c);
        self.item_edit_note_cursor = cursor + 1;
    }

    fn insert_item_edit_note_newline(&mut self) {
        let cursor = self.clamped_item_edit_note_cursor();
        let byte_index = self.item_edit_note_byte_index(cursor);
        self.item_edit_note.insert(byte_index, '\n');
        self.item_edit_note_cursor = cursor + 1;
    }

    fn handle_item_edit_note_input_key(&mut self, code: KeyCode) -> bool {
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

    fn handle_item_edit_field_input_key(&mut self, code: KeyCode) -> bool {
        match self.item_edit_focus {
            ItemEditFocus::Text => self.handle_text_input_key(code),
            ItemEditFocus::Note => self.handle_item_edit_note_input_key(code),
            ItemEditFocus::CategoriesButton
            | ItemEditFocus::SaveButton
            | ItemEditFocus::CancelButton => false,
        }
    }

    fn selected_category_is_reserved(&self) -> bool {
        self.selected_category_row()
            .map(|row| row.is_reserved)
            .unwrap_or(false)
    }

    fn category_config_note_cursor(&self) -> Option<usize> {
        self.category_config_editor
            .as_ref()
            .map(|editor| editor.note_cursor.min(editor.note.chars().count()))
    }

    fn move_category_config_note_cursor_left(&mut self) {
        if let Some(editor) = &mut self.category_config_editor {
            editor.note_cursor = editor.note_cursor.saturating_sub(1);
        }
    }

    fn move_category_config_note_cursor_right(&mut self) {
        if let Some(editor) = &mut self.category_config_editor {
            let max = editor.note.chars().count();
            editor.note_cursor = (editor.note_cursor + 1).min(max);
        }
    }

    fn move_category_config_note_cursor_home(&mut self) {
        if let Some(editor) = &mut self.category_config_editor {
            editor.note_cursor = 0;
        }
    }

    fn move_category_config_note_cursor_end(&mut self) {
        if let Some(editor) = &mut self.category_config_editor {
            editor.note_cursor = editor.note.chars().count();
        }
    }

    fn move_category_config_note_cursor_vertical(&mut self, delta: i32) {
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

    fn move_category_config_note_cursor_up(&mut self) {
        self.move_category_config_note_cursor_vertical(-1);
    }

    fn move_category_config_note_cursor_down(&mut self) {
        self.move_category_config_note_cursor_vertical(1);
    }

    fn backspace_category_config_note_char(&mut self) {
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

    fn delete_category_config_note_char(&mut self) {
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

    fn insert_category_config_note_char(&mut self, c: char) {
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

    fn insert_category_config_note_newline(&mut self) {
        let Some(editor) = &mut self.category_config_editor else {
            return;
        };
        let cursor = editor.note_cursor.min(editor.note.chars().count());
        let byte_index = string_byte_index(&editor.note, cursor);
        editor.note.insert(byte_index, '\n');
        editor.note_cursor = cursor + 1;
    }

    fn handle_category_config_note_input_key(&mut self, code: KeyCode) -> bool {
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

    fn cycle_category_config_focus(&mut self, delta: i32) {
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

    fn move_category_config_checkbox_focus(&mut self, delta: i32) {
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

    fn toggle_category_config_exclusive(&mut self) {
        if let Some(editor) = &mut self.category_config_editor {
            editor.is_exclusive = !editor.is_exclusive;
        }
    }

    fn toggle_category_config_no_implicit(&mut self) {
        if let Some(editor) = &mut self.category_config_editor {
            editor.enable_implicit_string = !editor.enable_implicit_string;
        }
    }

    fn toggle_category_config_actionable(&mut self) {
        if let Some(editor) = &mut self.category_config_editor {
            editor.is_actionable = !editor.is_actionable;
        }
    }

    fn cycle_item_edit_focus(&mut self, delta: i32) {
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

    fn toggle_preview(&mut self) {
        self.show_preview = !self.show_preview;
        if self.show_preview {
            self.preview_mode = PreviewMode::Summary;
            self.normal_focus = NormalFocus::Board;
            self.preview_summary_scroll = 0;
            self.status =
                "Preview opened (Summary). Tab to focus pane, o for provenance".to_string();
        } else {
            self.normal_focus = NormalFocus::Board;
            self.status = "Preview closed".to_string();
        }
    }

    fn toggle_preview_mode(&mut self) {
        if !self.show_preview {
            self.status = "Preview is closed (press p to open)".to_string();
            return;
        }
        self.preview_mode = match self.preview_mode {
            PreviewMode::Summary => PreviewMode::Provenance,
            PreviewMode::Provenance => PreviewMode::Summary,
        };
        self.status = match self.preview_mode {
            PreviewMode::Summary => "Preview mode: Summary".to_string(),
            PreviewMode::Provenance => "Preview mode: Provenance".to_string(),
        };
    }

    fn toggle_normal_focus(&mut self) {
        if !self.show_preview {
            self.status = "Preview is closed (press p to open)".to_string();
            return;
        }
        self.normal_focus = match self.normal_focus {
            NormalFocus::Board => NormalFocus::Preview,
            NormalFocus::Preview => NormalFocus::Board,
        };
        self.status = match self.normal_focus {
            NormalFocus::Board => "Focus: Board".to_string(),
            NormalFocus::Preview => "Focus: Preview".to_string(),
        };
    }

    fn scroll_preview(&mut self, delta: i32) {
        if !self.show_preview {
            return;
        }
        match self.preview_mode {
            PreviewMode::Summary => {
                if delta > 0 {
                    self.preview_summary_scroll = self.preview_summary_scroll.saturating_add(1);
                } else {
                    self.preview_summary_scroll = self.preview_summary_scroll.saturating_sub(1);
                }
            }
            PreviewMode::Provenance => {
                if delta > 0 {
                    self.preview_provenance_scroll =
                        self.preview_provenance_scroll.saturating_add(1);
                } else {
                    self.preview_provenance_scroll =
                        self.preview_provenance_scroll.saturating_sub(1);
                }
            }
        }
    }

    fn open_provenance_unassign_picker(&mut self) {
        if !self.show_preview {
            self.status = "Preview is closed (press p to open)".to_string();
            return;
        }
        if self.normal_focus != NormalFocus::Preview {
            self.status = "Focus preview pane to unassign from provenance (Tab)".to_string();
            return;
        }
        if self.preview_mode != PreviewMode::Provenance {
            self.status = "Switch preview to Provenance mode (o) to unassign".to_string();
            return;
        }
        let Some(item) = self.selected_item() else {
            self.status = "No selected item to unassign".to_string();
            return;
        };
        let rows = self.inspect_assignment_rows_for_item(item);
        if rows.is_empty() {
            self.status = "No assignments available to unassign".to_string();
            return;
        }
        self.mode = Mode::InspectUnassignPicker;
        self.inspect_assignment_index = self.inspect_assignment_index.min(rows.len() - 1);
        self.status = "Select assignment to unassign (j/k, Enter, Esc)".to_string();
    }

    fn handle_normal_key(&mut self, code: KeyCode, agenda: &Agenda<'_>) -> Result<bool, String> {
        match code {
            KeyCode::Char('q') => return Ok(true),
            KeyCode::Down | KeyCode::Char('j') => {
                if self.show_preview && self.normal_focus == NormalFocus::Preview {
                    self.scroll_preview(1);
                } else {
                    self.move_item_cursor(1);
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.show_preview && self.normal_focus == NormalFocus::Preview {
                    self.scroll_preview(-1);
                } else {
                    self.move_item_cursor(-1);
                }
            }
            KeyCode::Right | KeyCode::Char('l') => self.move_slot_cursor(1),
            KeyCode::Left | KeyCode::Char('h') => self.move_slot_cursor(-1),
            KeyCode::Char('n') => {
                self.mode = Mode::AddInput;
                self.clear_input();
                self.status = "Add item: type text and press Enter".to_string();
            }
            KeyCode::Char('e') => {
                self.open_item_edit_for_selected_item();
            }
            KeyCode::Enter => {
                self.open_item_edit_for_selected_item();
            }
            KeyCode::Char('m') => {
                if let Some(item) = self.selected_item() {
                    let existing_note = item.note.clone().unwrap_or_default();
                    self.mode = Mode::NoteEditInput;
                    self.set_input(existing_note);
                    self.status =
                        "Edit note: Enter to save (empty clears), Esc to cancel".to_string();
                } else {
                    self.status = "No selected item to add/edit note".to_string();
                }
            }
            KeyCode::Char('/') => {
                self.mode = Mode::FilterInput;
                self.set_input(self.filter.clone().unwrap_or_default());
                self.status = "Filter: type query and press Enter (Esc clears)".to_string();
            }
            KeyCode::Esc => {
                if self.filter.take().is_some() {
                    self.refresh(agenda.store())?;
                    self.status = "Filter cleared".to_string();
                }
            }
            KeyCode::F(8) | KeyCode::Char('v') => {
                self.mode = Mode::ViewPicker;
                self.picker_index = self.view_index;
                self.status =
                    "View palette: Enter switch, N create, r rename, x delete, e edit view, Esc cancel"
                        .to_string();
            }
            KeyCode::F(9) | KeyCode::Char('c') => {
                self.mode = Mode::CategoryManager;
                self.category_config_editor = None;
                self.status =
                    "Category manager: Enter config popup, e/i/a quick toggles (exclusive/match-name/actionable), n/N create, r rename, p reparent, x delete".to_string();
            }
            KeyCode::Char(',') => {
                self.cycle_view(-1, agenda)?;
            }
            KeyCode::Char('.') => {
                self.cycle_view(1, agenda)?;
            }
            KeyCode::Tab | KeyCode::BackTab => self.toggle_normal_focus(),
            KeyCode::Char('g') => {
                self.jump_to_all_items_view(agenda)?;
            }
            KeyCode::Char('a') => {
                if self.selected_item_id().is_none() {
                    self.status = "No selected item to edit categories".to_string();
                } else if self.category_rows.is_empty() {
                    self.status = "No categories available".to_string();
                } else {
                    self.mode = Mode::ItemAssignCategoryPicker;
                    self.item_assign_return_to_item_edit = false;
                    self.item_assign_category_index =
                        first_non_reserved_category_index(&self.category_rows);
                    self.clear_input();
                    self.status =
                        "Item categories: j/k select, Space toggle, n type category, Enter done, Esc cancel"
                            .to_string();
                }
            }
            KeyCode::Char('u') => {
                if self.show_preview
                    && self.normal_focus == NormalFocus::Preview
                    && self.preview_mode == PreviewMode::Provenance
                {
                    self.open_provenance_unassign_picker();
                } else {
                    if self.selected_item_id().is_none() {
                        self.status = "No selected item to edit categories".to_string();
                    } else if self.category_rows.is_empty() {
                        self.status = "No categories available".to_string();
                    } else {
                        self.mode = Mode::ItemAssignCategoryPicker;
                        self.item_assign_return_to_item_edit = false;
                        self.item_assign_category_index =
                            first_non_reserved_category_index(&self.category_rows);
                        self.clear_input();
                        self.status =
                            "Item categories: j/k select, Space toggle, n type category, Enter done, Esc cancel"
                                .to_string();
                    }
                }
            }
            KeyCode::Char('p') => self.toggle_preview(),
            KeyCode::Char('o') => self.toggle_preview_mode(),
            KeyCode::Char('J') => {
                self.scroll_preview(1);
            }
            KeyCode::Char('K') => {
                self.scroll_preview(-1);
            }
            KeyCode::Char('r') => {
                if let Some(item_id) = self.selected_item_id() {
                    if let Some(view) = self.current_view().cloned() {
                        agenda
                            .remove_item_from_view(item_id, &view)
                            .map_err(|e| e.to_string())?;
                        self.refresh(agenda.store())?;
                        self.status = "Removed item from current view".to_string();
                    }
                }
            }
            KeyCode::Char('d') | KeyCode::Char('D') => {
                if let Some(item_id) = self.selected_item_id() {
                    let was_done = self
                        .selected_item()
                        .map(|item| item.is_done)
                        .unwrap_or(false);
                    if !was_done && !self.selected_item_has_actionable_assignment() {
                        self.status =
                            "Done unavailable: item has no actionable category assignments"
                                .to_string();
                        return Ok(false);
                    }
                    agenda
                        .toggle_item_done(item_id)
                        .map_err(|e| e.to_string())?;
                    self.refresh(agenda.store())?;
                    self.status = if was_done {
                        "Marked item not-done".to_string()
                    } else {
                        "Marked item done".to_string()
                    };
                }
            }
            KeyCode::Char('x') => {
                if self.selected_item_id().is_some() {
                    self.mode = Mode::ConfirmDelete;
                    self.status = "Delete item? y/n".to_string();
                }
            }
            KeyCode::Char(']') => {
                self.move_selected_item_between_slots(1, agenda)?;
            }
            KeyCode::Char('[') => {
                self.move_selected_item_between_slots(-1, agenda)?;
            }
            _ => {}
        }

        Ok(false)
    }

    fn open_item_edit_for_selected_item(&mut self) {
        if let Some(item) = self.selected_item() {
            let existing_text = item.text.clone();
            let existing_note = item.note.clone().unwrap_or_default();
            self.mode = Mode::ItemEditInput;
            self.set_input(existing_text);
            self.item_edit_focus = ItemEditFocus::Text;
            self.item_edit_note = existing_note;
            self.item_edit_note_cursor = self.item_edit_note.chars().count();
            self.status =
                "Edit item: Tab cycles fields/buttons, Enter activates focused control, Up/Down in note"
                    .to_string();
        } else {
            self.status = "No selected item to edit".to_string();
        }
    }

    fn handle_add_key(&mut self, code: KeyCode, agenda: &Agenda<'_>) -> Result<bool, String> {
        match code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.clear_input();
                self.status = "Add canceled".to_string();
            }
            KeyCode::Enter => {
                let text = self.input.trim();
                if !text.is_empty() {
                    let text_value = text.to_string();
                    let category_names: Vec<String> = agenda
                        .store()
                        .get_hierarchy()
                        .map_err(|e| e.to_string())?
                        .into_iter()
                        .map(|category| category.name)
                        .collect();
                    let unknown_hashtags = unknown_hashtag_tokens(&text_value, &category_names);
                    let parsed_when = self.create_item_in_current_context(agenda, text_value)?;
                    self.status = add_capture_status_message(parsed_when, &unknown_hashtags);
                }
                self.mode = Mode::Normal;
                self.clear_input();
            }
            _ if self.handle_text_input_key(code) => {}
            _ => {}
        }
        Ok(false)
    }

    fn handle_item_edit_key(&mut self, code: KeyCode, agenda: &Agenda<'_>) -> Result<bool, String> {
        match code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.clear_input();
                self.item_edit_note.clear();
                self.item_edit_note_cursor = 0;
                self.item_edit_focus = ItemEditFocus::Text;
                self.status = "Edit canceled".to_string();
            }
            KeyCode::Tab | KeyCode::BackTab => {
                self.cycle_item_edit_focus(if matches!(code, KeyCode::BackTab) {
                    -1
                } else {
                    1
                });
            }
            KeyCode::F(3) => {
                self.item_edit_focus = ItemEditFocus::CategoriesButton;
                self.open_item_assign_picker_from_item_edit();
            }
            KeyCode::Enter => match self.item_edit_focus {
                ItemEditFocus::Text => {
                    self.cycle_item_edit_focus(1);
                }
                ItemEditFocus::Note => {
                    self.insert_item_edit_note_newline();
                }
                ItemEditFocus::CategoriesButton => {
                    self.open_item_assign_picker_from_item_edit();
                }
                ItemEditFocus::SaveButton => {
                    self.save_item_edit(agenda)?;
                }
                ItemEditFocus::CancelButton => {
                    self.mode = Mode::Normal;
                    self.clear_input();
                    self.item_edit_note.clear();
                    self.item_edit_note_cursor = 0;
                    self.item_edit_focus = ItemEditFocus::Text;
                    self.status = "Edit canceled".to_string();
                }
            },
            _ if self.handle_item_edit_field_input_key(code) => {}
            _ => {}
        }
        Ok(false)
    }

    fn open_item_assign_picker_from_item_edit(&mut self) {
        if self.selected_item_id().is_none() {
            self.status = "No selected item to edit categories".to_string();
            return;
        }
        if self.category_rows.is_empty() {
            self.status = "No categories available".to_string();
            return;
        }
        self.mode = Mode::ItemAssignCategoryPicker;
        self.item_assign_return_to_item_edit = true;
        self.item_assign_category_index = first_non_reserved_category_index(&self.category_rows);
        self.status =
            "Item categories: j/k select, Space toggle, n type category, Enter done, Esc cancel"
                .to_string();
    }

    fn save_item_edit(&mut self, agenda: &Agenda<'_>) -> Result<(), String> {
        let Some(item_id) = self.selected_item_id() else {
            self.mode = Mode::Normal;
            self.clear_input();
            self.item_edit_note.clear();
            self.item_edit_note_cursor = 0;
            self.item_edit_focus = ItemEditFocus::Text;
            self.status = "Edit failed: no selected item".to_string();
            return Ok(());
        };

        let updated_text = self.input.trim().to_string();
        if updated_text.is_empty() {
            self.status = "Cannot save: text cannot be empty".to_string();
            return Ok(());
        }
        let updated_note = if self.item_edit_note.trim().is_empty() {
            None
        } else {
            Some(self.item_edit_note.clone())
        };

        let mut item = agenda
            .store()
            .get_item(item_id)
            .map_err(|e| e.to_string())?;
        if item.text == updated_text && item.note == updated_note {
            self.mode = Mode::Normal;
            self.clear_input();
            self.item_edit_note.clear();
            self.item_edit_note_cursor = 0;
            self.item_edit_focus = ItemEditFocus::Text;
            self.status = "Edit canceled: no changes".to_string();
            return Ok(());
        }

        item.text = updated_text;
        item.note = updated_note;
        item.modified_at = Utc::now();
        let reference_date = Local::now().date_naive();
        agenda
            .update_item_with_reference_date(&item, reference_date)
            .map_err(|e| e.to_string())?;
        self.refresh(agenda.store())?;
        self.set_item_selection_by_id(item_id);
        self.mode = Mode::Normal;
        self.clear_input();
        self.item_edit_note.clear();
        self.item_edit_note_cursor = 0;
        self.item_edit_focus = ItemEditFocus::Text;
        self.status = "Item updated".to_string();
        Ok(())
    }

    fn handle_note_edit_key(&mut self, code: KeyCode, agenda: &Agenda<'_>) -> Result<bool, String> {
        match code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.clear_input();
                self.status = "Note edit canceled".to_string();
            }
            KeyCode::Enter => {
                let Some(item_id) = self.selected_item_id() else {
                    self.mode = Mode::Normal;
                    self.clear_input();
                    self.status = "Note edit failed: no selected item".to_string();
                    return Ok(false);
                };

                let new_note = if self.input.trim().is_empty() {
                    None
                } else {
                    Some(self.input.clone())
                };

                let mut item = agenda
                    .store()
                    .get_item(item_id)
                    .map_err(|e| e.to_string())?;
                if item.note == new_note {
                    self.mode = Mode::Normal;
                    self.clear_input();
                    self.status = "Note edit canceled: no note change".to_string();
                    return Ok(false);
                }

                item.note = new_note;
                item.modified_at = Utc::now();
                let reference_date = Local::now().date_naive();
                agenda
                    .update_item_with_reference_date(&item, reference_date)
                    .map_err(|e| e.to_string())?;
                self.refresh(agenda.store())?;
                self.set_item_selection_by_id(item_id);
                self.mode = Mode::Normal;
                self.clear_input();
                self.status = if item.note.is_some() {
                    "Note updated".to_string()
                } else {
                    "Note cleared".to_string()
                };
            }
            _ if self.handle_text_input_key(code) => {}
            _ => {}
        }
        Ok(false)
    }

    fn handle_item_assign_category_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> Result<bool, String> {
        match code {
            KeyCode::Esc => {
                self.mode = if self.item_assign_return_to_item_edit {
                    Mode::ItemEditInput
                } else {
                    Mode::Normal
                };
                self.item_assign_return_to_item_edit = false;
                self.clear_input();
                self.status = "Assign canceled".to_string();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if !self.category_rows.is_empty() {
                    self.item_assign_category_index =
                        next_index(self.item_assign_category_index, self.category_rows.len(), 1);
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if !self.category_rows.is_empty() {
                    self.item_assign_category_index = next_index(
                        self.item_assign_category_index,
                        self.category_rows.len(),
                        -1,
                    );
                }
            }
            KeyCode::Char('n') | KeyCode::Char('/') => {
                self.mode = Mode::ItemAssignCategoryInput;
                self.clear_input();
                self.status = "Type category name: Enter assign/create, Esc back".to_string();
            }
            KeyCode::Char(' ') => {
                let Some(item_id) = self.selected_item_id() else {
                    self.mode = if self.item_assign_return_to_item_edit {
                        Mode::ItemEditInput
                    } else {
                        Mode::Normal
                    };
                    self.item_assign_return_to_item_edit = false;
                    self.status = "Assign failed: no selected item".to_string();
                    return Ok(false);
                };
                let Some(row) = self
                    .category_rows
                    .get(self.item_assign_category_index)
                    .cloned()
                else {
                    self.status = "Assign failed: no category selected".to_string();
                    return Ok(false);
                };

                if row.name.eq_ignore_ascii_case("Done") {
                    let was_done = self
                        .selected_item()
                        .map(|item| item.is_done)
                        .unwrap_or(false);
                    if !was_done && !self.selected_item_has_actionable_assignment() {
                        self.status =
                            "Done unavailable: item has no actionable category assignments"
                                .to_string();
                        return Ok(false);
                    }
                    match agenda.toggle_item_done(item_id) {
                        Ok(_) => {
                            self.refresh(agenda.store())?;
                            self.set_item_selection_by_id(item_id);
                            self.status = if was_done {
                                "Removed category Done (marked not-done)".to_string()
                            } else {
                                "Assigned item to category Done (marked done)".to_string()
                            };
                        }
                        Err(err) => {
                            self.status = format!("Done toggle failed: {}", err);
                        }
                    }
                    return Ok(false);
                }

                if self.selected_item_has_assignment(row.id) {
                    match agenda.unassign_item_manual(item_id, row.id) {
                        Ok(()) => {
                            self.refresh(agenda.store())?;
                            self.set_item_selection_by_id(item_id);
                            self.status = format!("Removed category {}", row.name);
                        }
                        Err(err) => {
                            self.status = format!("Cannot remove {}: {}", row.name, err);
                        }
                    }
                } else {
                    let result = agenda
                        .assign_item_manual(item_id, row.id, Some("manual:tui.assign".to_string()))
                        .map_err(|e| e.to_string())?;
                    self.refresh(agenda.store())?;
                    self.set_item_selection_by_id(item_id);
                    self.status = format!(
                        "Added category {} (new_assignments={})",
                        row.name,
                        result.new_assignments.len()
                    );
                }
            }
            KeyCode::Enter => {
                self.mode = if self.item_assign_return_to_item_edit {
                    Mode::ItemEditInput
                } else {
                    Mode::Normal
                };
                self.item_assign_return_to_item_edit = false;
                self.clear_input();
                self.status = "Category edit saved".to_string();
            }
            _ => {}
        }

        Ok(false)
    }

    fn handle_item_assign_category_input_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> Result<bool, String> {
        match code {
            KeyCode::Esc => {
                self.mode = Mode::ItemAssignCategoryPicker;
                self.clear_input();
                self.status = "Category name entry canceled".to_string();
            }
            KeyCode::Enter => {
                let Some(item_id) = self.selected_item_id() else {
                    self.mode = Mode::Normal;
                    self.clear_input();
                    self.status = "Assign failed: no selected item".to_string();
                    return Ok(false);
                };
                let name = self.input.trim().to_string();
                if name.is_empty() {
                    self.mode = Mode::ItemAssignCategoryPicker;
                    self.clear_input();
                    self.status = "Category name entry canceled (empty)".to_string();
                    return Ok(false);
                }

                let category_id = if let Some(existing) = self
                    .categories
                    .iter()
                    .find(|category| category.name.eq_ignore_ascii_case(&name))
                {
                    existing.id
                } else {
                    let mut category = Category::new(name.clone());
                    category.enable_implicit_string = true;
                    agenda
                        .store()
                        .create_category(&category)
                        .map_err(|e| e.to_string())?;
                    category.id
                };

                let result = agenda
                    .assign_item_manual(item_id, category_id, Some("manual:tui.assign".to_string()))
                    .map_err(|e| e.to_string())?;
                self.refresh(agenda.store())?;
                self.set_item_selection_by_id(item_id);
                if let Some(index) = self
                    .category_rows
                    .iter()
                    .position(|row| row.id == category_id)
                {
                    self.item_assign_category_index = index;
                }
                self.mode = Mode::ItemAssignCategoryPicker;
                self.clear_input();
                self.status = format!(
                    "Assigned category {} (new_assignments={})",
                    name,
                    result.new_assignments.len()
                );
            }
            _ if self.handle_text_input_key(code) => {}
            _ => {}
        }

        Ok(false)
    }

    fn handle_inspect_unassign_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> Result<bool, String> {
        let rows = self
            .selected_item()
            .map(|item| self.inspect_assignment_rows_for_item(item))
            .unwrap_or_default();

        match code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.status = "Unassign canceled".to_string();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if !rows.is_empty() {
                    self.inspect_assignment_index =
                        next_index(self.inspect_assignment_index, rows.len(), 1);
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if !rows.is_empty() {
                    self.inspect_assignment_index =
                        next_index(self.inspect_assignment_index, rows.len(), -1);
                }
            }
            KeyCode::Enter => {
                let Some(item_id) = self.selected_item_id() else {
                    self.mode = Mode::Normal;
                    self.status = "Unassign failed: no selected item".to_string();
                    return Ok(false);
                };
                let Some(row) = rows.get(self.inspect_assignment_index).cloned() else {
                    self.mode = Mode::Normal;
                    self.status = "Unassign failed: no assignment selected".to_string();
                    return Ok(false);
                };

                agenda
                    .unassign_item_manual(item_id, row.category_id)
                    .map_err(|e| e.to_string())?;
                self.refresh(agenda.store())?;
                self.set_item_selection_by_id(item_id);
                self.mode = Mode::Normal;
                self.status = format!("Unassigned {}", row.category_name);
            }
            _ => {}
        }

        Ok(false)
    }

    fn handle_filter_key(&mut self, code: KeyCode, agenda: &Agenda<'_>) -> Result<bool, String> {
        match code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.filter = None;
                self.clear_input();
                self.refresh(agenda.store())?;
                self.status = "Filter cleared".to_string();
            }
            KeyCode::Enter => {
                self.mode = Mode::Normal;
                let value = self.input.trim().to_string();
                self.filter = if value.is_empty() { None } else { Some(value) };
                self.refresh(agenda.store())?;
                self.status = if self.filter.is_some() {
                    "Filter applied".to_string()
                } else {
                    "Filter cleared".to_string()
                };
                self.clear_input();
            }
            _ if self.handle_text_input_key(code) => {}
            _ => {}
        }
        Ok(false)
    }

    fn handle_view_picker_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> Result<bool, String> {
        match code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.status = "View switch canceled".to_string();
            }
            KeyCode::Enter => {
                if !self.views.is_empty() {
                    self.view_index = self.picker_index.min(self.views.len() - 1);
                    self.slot_index = 0;
                    self.item_index = 0;
                    self.refresh(agenda.store())?;
                    let view_name = self
                        .current_view()
                        .map(|view| view.name.clone())
                        .unwrap_or_else(|| "(none)".to_string());
                    self.status =
                        format!("Switched to view: {view_name} (press v then e to edit view)");
                } else {
                    self.status = "No views available".to_string();
                }
                self.mode = Mode::Normal;
            }
            KeyCode::Char('N') => {
                self.mode = Mode::ViewCreateNameInput;
                self.clear_input();
                self.view_pending_name = None;
                self.view_pending_edit_name = None;
                self.view_return_to_manager = false;
                self.status = "Create view: type name and press Enter".to_string();
            }
            KeyCode::Char('r') => {
                if let Some(view) = self.views.get(self.picker_index).cloned() {
                    self.mode = Mode::ViewRenameInput;
                    self.set_input(view.name.clone());
                    self.view_pending_edit_name = Some(view.name.clone());
                    self.view_return_to_manager = false;
                    self.status = format!("Rename view {}: type name and Enter", view.name);
                } else {
                    self.status = "No selected view to rename".to_string();
                }
            }
            KeyCode::Char('e') | KeyCode::Char('E') => {
                if let Some(view) = self.views.get(self.picker_index).cloned() {
                    self.open_view_editor(view);
                    self.status =
                        "View editor: j/k select row, o/right open, Enter save, Esc cancel"
                            .to_string();
                } else {
                    self.status = "No selected view to edit".to_string();
                }
            }
            KeyCode::Char('V') => {
                if self.views.is_empty() {
                    self.status = "No views available".to_string();
                } else {
                    self.mode = Mode::ViewManagerScreen;
                    self.view_return_to_manager = false;
                    self.view_manager_pane = ViewManagerPane::Views;
                    self.view_manager_section_index = 0;
                    self.load_view_manager_rows_from_selected_view();
                    self.status =
                        "View manager: Tab pane, j/k row, Enter action, s save, q/Esc back"
                            .to_string();
                }
            }
            KeyCode::Char('x') => {
                if let Some(view) = self.views.get(self.picker_index) {
                    self.mode = Mode::ViewDeleteConfirm;
                    self.view_return_to_manager = false;
                    self.status = format!("Delete view '{}' ? y/n", view.name);
                } else {
                    self.status = "No selected view to delete".to_string();
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if !self.views.is_empty() {
                    self.picker_index = (self.picker_index + 1) % self.views.len();
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if !self.views.is_empty() {
                    self.picker_index = if self.picker_index == 0 {
                        self.views.len() - 1
                    } else {
                        self.picker_index - 1
                    };
                }
            }
            _ => {}
        }
        Ok(false)
    }

    fn handle_view_manager_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> Result<bool, String> {
        match code {
            KeyCode::Esc | KeyCode::Char('q') => {
                let selected_name = self
                    .views
                    .get(self.picker_index)
                    .map(|view| view.name.clone());
                let had_unsaved = self.view_manager_dirty;
                self.refresh(agenda.store())?;
                if let Some(name) = selected_name {
                    self.set_view_selection_by_name(&name);
                    self.picker_index = self.view_index.min(self.views.len().saturating_sub(1));
                }
                self.mode = Mode::ViewPicker;
                self.status = if had_unsaved {
                    "Closed view manager (unsaved changes discarded)".to_string()
                } else {
                    "Closed view manager".to_string()
                };
            }
            KeyCode::Tab => {
                self.view_manager_pane = match self.view_manager_pane {
                    ViewManagerPane::Views => ViewManagerPane::Definition,
                    ViewManagerPane::Definition => ViewManagerPane::Sections,
                    ViewManagerPane::Sections => ViewManagerPane::Views,
                };
            }
            KeyCode::BackTab => {
                self.view_manager_pane = match self.view_manager_pane {
                    ViewManagerPane::Views => ViewManagerPane::Sections,
                    ViewManagerPane::Definition => ViewManagerPane::Views,
                    ViewManagerPane::Sections => ViewManagerPane::Definition,
                };
            }
            KeyCode::Down | KeyCode::Char('j') => match self.view_manager_pane {
                ViewManagerPane::Views => {
                    if !self.views.is_empty() {
                        let next = next_index(self.picker_index, self.views.len(), 1);
                        if self.view_manager_dirty
                            && self
                                .view_manager_loaded_view_name
                                .as_ref()
                                .map(|name| {
                                    self.views
                                        .get(next)
                                        .map(|view| !view.name.eq_ignore_ascii_case(name))
                                        .unwrap_or(false)
                                })
                                .unwrap_or(false)
                        {
                            self.status =
                                "Unsaved manager changes. Press s to save before switching view."
                                    .to_string();
                        } else {
                            self.picker_index = next;
                            self.load_view_manager_rows_from_selected_view();
                        }
                    }
                }
                ViewManagerPane::Definition => {
                    let count = self.view_manager_rows.len().max(1);
                    self.view_manager_definition_index =
                        next_index(self.view_manager_definition_index, count, 1);
                }
                ViewManagerPane::Sections => {
                    let section_count = self
                        .views
                        .get(self.picker_index)
                        .map(|view| view.sections.len().max(1))
                        .unwrap_or(1);
                    self.view_manager_section_index =
                        next_index(self.view_manager_section_index, section_count, 1);
                }
            },
            KeyCode::Up | KeyCode::Char('k') => match self.view_manager_pane {
                ViewManagerPane::Views => {
                    if !self.views.is_empty() {
                        let next = next_index(self.picker_index, self.views.len(), -1);
                        if self.view_manager_dirty
                            && self
                                .view_manager_loaded_view_name
                                .as_ref()
                                .map(|name| {
                                    self.views
                                        .get(next)
                                        .map(|view| !view.name.eq_ignore_ascii_case(name))
                                        .unwrap_or(false)
                                })
                                .unwrap_or(false)
                        {
                            self.status =
                                "Unsaved manager changes. Press s to save before switching view."
                                    .to_string();
                        } else {
                            self.picker_index = next;
                            self.load_view_manager_rows_from_selected_view();
                        }
                    }
                }
                ViewManagerPane::Definition => {
                    let count = self.view_manager_rows.len().max(1);
                    self.view_manager_definition_index =
                        next_index(self.view_manager_definition_index, count, -1);
                }
                ViewManagerPane::Sections => {
                    let section_count = self
                        .views
                        .get(self.picker_index)
                        .map(|view| view.sections.len().max(1))
                        .unwrap_or(1);
                    self.view_manager_section_index =
                        next_index(self.view_manager_section_index, section_count, -1);
                }
            },
            KeyCode::Enter => {
                if self.view_manager_pane == ViewManagerPane::Views {
                    if !self.views.is_empty() {
                        self.view_index = self.picker_index.min(self.views.len() - 1);
                        self.slot_index = 0;
                        self.item_index = 0;
                        self.refresh(agenda.store())?;
                        let view_name = self
                            .current_view()
                            .map(|view| view.name.clone())
                            .unwrap_or_else(|| "(none)".to_string());
                        self.status = format!("Focused view in manager: {view_name}");
                        self.load_view_manager_rows_from_selected_view();
                    }
                } else if self.view_manager_pane == ViewManagerPane::Definition {
                    if let Some(row) = self
                        .view_manager_rows
                        .get_mut(self.view_manager_definition_index)
                    {
                        row.sign = match row.sign {
                            ViewCriteriaSign::Include => ViewCriteriaSign::Exclude,
                            ViewCriteriaSign::Exclude => ViewCriteriaSign::Include,
                        };
                        self.view_manager_dirty = true;
                        self.refresh_view_manager_preview();
                    }
                } else {
                    self.open_view_manager_section_editor();
                }
            }
            KeyCode::Char('s') => {
                let Some(view) = self.views.get(self.picker_index).cloned() else {
                    self.status = "No selected view to save".to_string();
                    return Ok(false);
                };
                let validation_errors = self.view_manager_representability_errors();
                if !validation_errors.is_empty() {
                    self.status = format!("Cannot save criteria: {}", validation_errors[0]);
                    return Ok(false);
                }
                let mut updated = view.clone();
                updated.criteria = self.view_manager_query_from_rows(&view);
                match agenda.store().update_view(&updated) {
                    Ok(()) => {
                        self.refresh(agenda.store())?;
                        self.set_view_selection_by_name(&updated.name);
                        self.load_view_manager_rows_from_selected_view();
                        self.status = format!(
                            "Saved criteria for {} (matching={})",
                            updated.name, self.view_manager_preview_count
                        );
                    }
                    Err(err) => {
                        self.status = format!("View manager save failed: {err}");
                    }
                }
            }
            KeyCode::Char('N') => match self.view_manager_pane {
                ViewManagerPane::Views => {
                    self.mode = Mode::ViewCreateNameInput;
                    self.clear_input();
                    self.view_pending_name = None;
                    self.view_pending_edit_name = None;
                    self.view_return_to_manager = true;
                    self.status = "Create view: type name and press Enter".to_string();
                }
                ViewManagerPane::Definition => {
                    let Some(category_row) = self
                        .category_rows
                        .iter()
                        .find(|row| !row.is_reserved)
                        .cloned()
                    else {
                        self.status = "No user categories available for criteria rows".to_string();
                        return Ok(false);
                    };
                    self.view_manager_rows.push(ViewCriteriaRow {
                        sign: ViewCriteriaSign::Include,
                        category_id: category_row.id,
                        join_is_or: false,
                        depth: 0,
                    });
                    self.view_manager_definition_index =
                        self.view_manager_rows.len().saturating_sub(1);
                    self.view_manager_dirty = true;
                    self.refresh_view_manager_preview();
                    self.status = format!("Added criteria row for {}", category_row.name);
                }
                ViewManagerPane::Sections => {
                    let Some(view) = self.views.get_mut(self.picker_index) else {
                        self.status = "No selected view for section add".to_string();
                        return Ok(false);
                    };
                    let next = view.sections.len() + 1;
                    view.sections.push(Section {
                        title: format!("Section {next}"),
                        criteria: Query::default(),
                        on_insert_assign: HashSet::new(),
                        on_remove_unassign: HashSet::new(),
                        show_children: false,
                    });
                    self.view_manager_section_index = view.sections.len().saturating_sub(1);
                    self.view_manager_dirty = true;
                    self.status = format!("Added Section {next}");
                }
            },
            KeyCode::Char('r') => {
                if self.view_manager_pane == ViewManagerPane::Views {
                    if let Some(view) = self.views.get(self.picker_index).cloned() {
                        self.mode = Mode::ViewRenameInput;
                        self.set_input(view.name.clone());
                        self.view_pending_edit_name = Some(view.name.clone());
                        self.view_return_to_manager = true;
                        self.status = format!("Rename view {}: type name and Enter", view.name);
                    } else {
                        self.status = "No selected view to rename".to_string();
                    }
                }
            }
            KeyCode::Char('x') => match self.view_manager_pane {
                ViewManagerPane::Views => {
                    if let Some(view) = self.views.get(self.picker_index) {
                        self.mode = Mode::ViewDeleteConfirm;
                        self.view_return_to_manager = true;
                        self.status = format!("Delete view '{}' ? y/n", view.name);
                    } else {
                        self.status = "No selected view to delete".to_string();
                    }
                }
                ViewManagerPane::Definition => {
                    if self.view_manager_rows.is_empty() {
                        self.status = "No criteria row to remove".to_string();
                        return Ok(false);
                    }
                    let removed = self.view_manager_rows.remove(
                        self.view_manager_definition_index
                            .min(self.view_manager_rows.len().saturating_sub(1)),
                    );
                    self.view_manager_definition_index = self
                        .view_manager_definition_index
                        .min(self.view_manager_rows.len().saturating_sub(1));
                    self.view_manager_dirty = true;
                    self.refresh_view_manager_preview();
                    let category_name = self
                        .category_rows
                        .iter()
                        .find(|row| row.id == removed.category_id)
                        .map(|row| row.name.clone())
                        .unwrap_or_else(|| removed.category_id.to_string());
                    self.status = format!("Removed criteria row {}", category_name);
                }
                ViewManagerPane::Sections => {
                    let Some(view) = self.views.get_mut(self.picker_index) else {
                        self.status = "No selected view for section remove".to_string();
                        return Ok(false);
                    };
                    if view.sections.is_empty() {
                        self.status = "No section to remove".to_string();
                        return Ok(false);
                    }
                    let remove_index = self
                        .view_manager_section_index
                        .min(view.sections.len().saturating_sub(1));
                    let removed = view.sections.remove(remove_index);
                    self.view_manager_section_index = self
                        .view_manager_section_index
                        .min(view.sections.len().saturating_sub(1));
                    self.view_manager_dirty = true;
                    self.status = format!("Removed section {}", removed.title);
                }
            },
            KeyCode::Char('[') => {
                if self.view_manager_pane == ViewManagerPane::Sections {
                    let Some(view) = self.views.get_mut(self.picker_index) else {
                        self.status = "No selected view for section reorder".to_string();
                        return Ok(false);
                    };
                    if view.sections.len() < 2 {
                        self.status = "Need at least two sections to reorder".to_string();
                        return Ok(false);
                    }
                    let current = self
                        .view_manager_section_index
                        .min(view.sections.len().saturating_sub(1));
                    if current == 0 {
                        return Ok(false);
                    }
                    let target = current - 1;
                    view.sections.swap(current, target);
                    self.view_manager_section_index = target;
                    self.view_manager_dirty = true;
                    self.status = "Moved section up".to_string();
                }
            }
            KeyCode::Char(']') => {
                if self.view_manager_pane == ViewManagerPane::Sections {
                    let Some(view) = self.views.get_mut(self.picker_index) else {
                        self.status = "No selected view for section reorder".to_string();
                        return Ok(false);
                    };
                    if view.sections.len() < 2 {
                        self.status = "Need at least two sections to reorder".to_string();
                        return Ok(false);
                    }
                    let current = self
                        .view_manager_section_index
                        .min(view.sections.len().saturating_sub(1));
                    if current + 1 >= view.sections.len() {
                        return Ok(false);
                    }
                    let target = current + 1;
                    view.sections.swap(current, target);
                    self.view_manager_section_index = target;
                    self.view_manager_dirty = true;
                    self.status = "Moved section down".to_string();
                }
            }
            KeyCode::Char(' ') => {
                if self.view_manager_pane == ViewManagerPane::Definition {
                    if let Some(row) = self
                        .view_manager_rows
                        .get_mut(self.view_manager_definition_index)
                    {
                        row.sign = match row.sign {
                            ViewCriteriaSign::Include => ViewCriteriaSign::Exclude,
                            ViewCriteriaSign::Exclude => ViewCriteriaSign::Include,
                        };
                        self.view_manager_dirty = true;
                        self.refresh_view_manager_preview();
                    }
                }
            }
            KeyCode::Char('a') => {
                if self.view_manager_pane == ViewManagerPane::Definition {
                    if let Some(row) = self
                        .view_manager_rows
                        .get_mut(self.view_manager_definition_index)
                    {
                        row.join_is_or = false;
                        self.view_manager_dirty = true;
                    }
                }
            }
            KeyCode::Char('o') => {
                if self.view_manager_pane == ViewManagerPane::Definition {
                    if let Some(row) = self
                        .view_manager_rows
                        .get_mut(self.view_manager_definition_index)
                    {
                        row.join_is_or = true;
                        self.view_manager_dirty = true;
                    }
                }
            }
            KeyCode::Char('(') => {
                if self.view_manager_pane == ViewManagerPane::Definition {
                    if let Some(row) = self
                        .view_manager_rows
                        .get_mut(self.view_manager_definition_index)
                    {
                        row.depth = row.depth.saturating_add(1).min(8);
                        self.view_manager_dirty = true;
                    }
                }
            }
            KeyCode::Char(')') => {
                if self.view_manager_pane == ViewManagerPane::Definition {
                    if let Some(row) = self
                        .view_manager_rows
                        .get_mut(self.view_manager_definition_index)
                    {
                        row.depth = row.depth.saturating_sub(1);
                        self.view_manager_dirty = true;
                    }
                }
            }
            KeyCode::Char('c') => {
                if self.view_manager_pane == ViewManagerPane::Definition {
                    let Some(row) = self
                        .view_manager_rows
                        .get(self.view_manager_definition_index)
                    else {
                        self.status = "No criteria row selected".to_string();
                        return Ok(false);
                    };
                    let Some(index) = self
                        .category_rows
                        .iter()
                        .position(|category| category.id == row.category_id)
                    else {
                        self.status = "Current row category is missing".to_string();
                        return Ok(false);
                    };
                    if self.category_rows.is_empty() {
                        self.status = "No user categories available".to_string();
                        return Ok(false);
                    }
                    self.view_manager_category_row_index = Some(self.view_manager_definition_index);
                    self.view_category_index = index;
                    self.mode = Mode::ViewManagerCategoryPicker;
                    self.status = "Pick category: j/k move, Enter choose, Esc cancel".to_string();
                }
            }
            KeyCode::Char('u') => {
                if self.view_manager_pane == ViewManagerPane::Sections {
                    self.open_view_manager_unmatched_settings();
                }
            }
            KeyCode::Char('C') => {
                if self.view_manager_pane == ViewManagerPane::Views {
                    let Some(view) = self.views.get(self.picker_index).cloned() else {
                        self.status = "No selected view to clone".to_string();
                        return Ok(false);
                    };

                    let clone_name = self.next_view_clone_name(&view.name);
                    let mut clone = View::new(clone_name.clone());
                    clone.criteria = view.criteria.clone();
                    clone.sections = view.sections.clone();
                    clone.columns = view.columns.clone();
                    clone.show_unmatched = view.show_unmatched;
                    clone.unmatched_label = view.unmatched_label.clone();
                    clone.remove_from_view_unassign = view.remove_from_view_unassign.clone();
                    match agenda.store().create_view(&clone) {
                        Ok(()) => {
                            self.refresh(agenda.store())?;
                            self.set_view_selection_by_name(&clone_name);
                            self.mode = Mode::ViewManagerScreen;
                            self.load_view_manager_rows_from_selected_view();
                            self.status = format!("Cloned view as {clone_name}");
                        }
                        Err(err) => {
                            self.status = format!("View clone failed: {err}");
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(false)
    }

    fn next_view_clone_name(&self, base_name: &str) -> String {
        let mut candidate = format!("{base_name} Copy");
        let mut counter = 2usize;
        while self
            .views
            .iter()
            .any(|view| view.name.eq_ignore_ascii_case(&candidate))
        {
            candidate = format!("{base_name} Copy {counter}");
            counter += 1;
        }
        candidate
    }

    fn handle_view_manager_category_picker_key(&mut self, code: KeyCode) -> Result<bool, String> {
        match code {
            KeyCode::Esc => {
                self.mode = Mode::ViewManagerScreen;
                self.view_manager_category_row_index = None;
                self.status = "Category pick canceled".to_string();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if !self.category_rows.is_empty() {
                    self.view_category_index =
                        next_index(self.view_category_index, self.category_rows.len(), 1);
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if !self.category_rows.is_empty() {
                    self.view_category_index =
                        next_index(self.view_category_index, self.category_rows.len(), -1);
                }
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                let Some(target_row_index) = self.view_manager_category_row_index else {
                    self.mode = Mode::ViewManagerScreen;
                    self.status = "Category pick failed: no target row".to_string();
                    return Ok(false);
                };
                let Some(selected_row) = self.category_rows.get(self.view_category_index) else {
                    self.status = "Category pick failed: no category selected".to_string();
                    return Ok(false);
                };
                let selected_category_id = selected_row.id;
                let selected_category_name = selected_row.name.clone();
                if selected_row.is_reserved {
                    self.status = "Reserved categories cannot be used in criteria rows".to_string();
                    return Ok(false);
                }
                if let Some(target_row) = self.view_manager_rows.get_mut(target_row_index) {
                    target_row.category_id = selected_category_id;
                    self.view_manager_dirty = true;
                    self.refresh_view_manager_preview();
                    self.status =
                        format!("Set criteria row category to {}", selected_category_name);
                }
                self.view_manager_category_row_index = None;
                self.mode = Mode::ViewManagerScreen;
            }
            _ => {}
        }
        Ok(false)
    }

    fn load_view_manager_rows_from_selected_view(&mut self) {
        let Some(view) = self.views.get(self.picker_index) else {
            self.view_manager_rows.clear();
            self.view_manager_loaded_view_name = None;
            self.view_manager_preview_count = 0;
            self.view_manager_definition_index = 0;
            self.view_manager_dirty = false;
            return;
        };

        let category_names = category_name_map(&self.categories);
        let mut rows: Vec<ViewCriteriaRow> = view
            .criteria
            .include
            .iter()
            .map(|category_id| ViewCriteriaRow {
                sign: ViewCriteriaSign::Include,
                category_id: *category_id,
                join_is_or: false,
                depth: 0,
            })
            .chain(
                view.criteria
                    .exclude
                    .iter()
                    .map(|category_id| ViewCriteriaRow {
                        sign: ViewCriteriaSign::Exclude,
                        category_id: *category_id,
                        join_is_or: false,
                        depth: 0,
                    }),
            )
            .collect();
        rows.sort_by(|a, b| {
            let a_name = category_names
                .get(&a.category_id)
                .cloned()
                .unwrap_or_else(|| a.category_id.to_string());
            let b_name = category_names
                .get(&b.category_id)
                .cloned()
                .unwrap_or_else(|| b.category_id.to_string());
            let a_sign = matches!(a.sign, ViewCriteriaSign::Exclude) as u8;
            let b_sign = matches!(b.sign, ViewCriteriaSign::Exclude) as u8;
            (a_sign, a_name.to_ascii_lowercase()).cmp(&(b_sign, b_name.to_ascii_lowercase()))
        });

        self.view_manager_rows = rows;
        self.view_manager_loaded_view_name = Some(view.name.clone());
        self.view_manager_definition_index = 0;
        self.refresh_view_manager_preview();
        self.view_manager_dirty = false;
    }

    fn view_manager_category_label(&self, category_id: CategoryId) -> String {
        self.category_rows
            .iter()
            .find(|row| row.id == category_id)
            .map(|row| row.name.clone())
            .unwrap_or_else(|| category_id.to_string())
    }

    fn view_manager_representability_errors(&self) -> Vec<String> {
        let mut errors = Vec::new();
        let mut seen: HashMap<CategoryId, (ViewCriteriaSign, usize)> = HashMap::new();
        for (index, row) in self.view_manager_rows.iter().enumerate() {
            let label = self.view_manager_category_label(row.category_id);
            if index > 0 && row.join_is_or {
                errors.push(format!(
                    "Row {} ({}) uses OR; current persistence only supports AND.",
                    index + 1,
                    label
                ));
            }
            if row.depth > 0 {
                errors.push(format!(
                    "Row {} ({}) uses nesting depth {}; only depth 0 is persistable.",
                    index + 1,
                    label,
                    row.depth
                ));
            }
            if let Some((prior_sign, prior_index)) = seen.get(&row.category_id).copied() {
                if prior_sign == row.sign {
                    errors.push(format!(
                        "Row {} ({}) duplicates row {}.",
                        index + 1,
                        label,
                        prior_index + 1
                    ));
                } else {
                    errors.push(format!(
                        "Row {} ({}) conflicts with row {} (+/- mismatch).",
                        index + 1,
                        label,
                        prior_index + 1
                    ));
                }
            } else {
                seen.insert(row.category_id, (row.sign, index));
            }
        }
        errors
    }

    fn view_manager_query_from_rows(&self, base_view: &View) -> Query {
        let mut query = base_view.criteria.clone();
        query.include.clear();
        query.exclude.clear();
        for row in &self.view_manager_rows {
            match row.sign {
                ViewCriteriaSign::Include => {
                    query.include.insert(row.category_id);
                    query.exclude.remove(&row.category_id);
                }
                ViewCriteriaSign::Exclude => {
                    query.exclude.insert(row.category_id);
                    query.include.remove(&row.category_id);
                }
            }
        }
        query
    }

    fn refresh_view_manager_preview(&mut self) {
        let Some(view) = self.views.get(self.picker_index) else {
            self.view_manager_preview_count = 0;
            return;
        };
        let query = self.view_manager_query_from_rows(view);
        self.view_manager_preview_count = self.preview_count_for_query(&query);
    }

    fn handle_view_delete_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> Result<bool, String> {
        let return_mode = if self.view_return_to_manager {
            Mode::ViewManagerScreen
        } else {
            Mode::ViewPicker
        };
        match code {
            KeyCode::Char('y') => {
                let Some(view) = self.views.get(self.picker_index).cloned() else {
                    self.mode = return_mode;
                    self.view_return_to_manager = false;
                    self.status = "Delete failed: no selected view".to_string();
                    return Ok(false);
                };
                let deleted_index = self.picker_index.min(self.views.len().saturating_sub(1));
                match agenda.store().delete_view(view.id) {
                    Ok(()) => {
                        if self.view_index > deleted_index {
                            self.view_index -= 1;
                        } else if self.view_index == deleted_index {
                            self.view_index = deleted_index.saturating_sub(1);
                        }
                        self.refresh(agenda.store())?;
                        self.mode = return_mode;
                        self.picker_index =
                            self.picker_index.min(self.views.len().saturating_sub(1));
                        self.view_return_to_manager = false;
                        if self.mode == Mode::ViewManagerScreen {
                            self.load_view_manager_rows_from_selected_view();
                        }
                        self.status = format!("Deleted view: {}", view.name);
                    }
                    Err(err) => {
                        self.mode = return_mode;
                        self.view_return_to_manager = false;
                        self.status = format!("Delete failed: {err}");
                    }
                }
            }
            KeyCode::Char('n') | KeyCode::Esc => {
                self.mode = return_mode;
                self.view_return_to_manager = false;
                if self.mode == Mode::ViewManagerScreen {
                    self.load_view_manager_rows_from_selected_view();
                }
                self.status = "Delete canceled".to_string();
            }
            _ => {}
        }
        Ok(false)
    }

    fn handle_view_create_name_key(&mut self, code: KeyCode) -> Result<bool, String> {
        let return_mode = if self.view_return_to_manager {
            Mode::ViewManagerScreen
        } else {
            Mode::ViewPicker
        };
        match code {
            KeyCode::Esc => {
                self.mode = return_mode;
                self.clear_input();
                self.view_pending_name = None;
                self.view_return_to_manager = false;
                if self.mode == Mode::ViewManagerScreen {
                    self.load_view_manager_rows_from_selected_view();
                }
                self.status = "View create canceled".to_string();
            }
            KeyCode::Enter => {
                let name = self.input.trim().to_string();
                if name.is_empty() {
                    self.mode = return_mode;
                    self.clear_input();
                    self.view_pending_name = None;
                    self.view_return_to_manager = false;
                    if self.mode == Mode::ViewManagerScreen {
                        self.load_view_manager_rows_from_selected_view();
                    }
                    self.status = "View create canceled (empty name)".to_string();
                } else {
                    self.view_pending_name = Some(name.clone());
                    self.view_category_index =
                        first_non_reserved_category_index(&self.category_rows);
                    self.view_create_include_selection.clear();
                    self.view_create_exclude_selection.clear();
                    self.mode = Mode::ViewCreateCategoryPicker;
                    self.clear_input();
                    self.status =
                        format!("Create view {name}: + include, - exclude, Enter creates");
                }
            }
            _ if self.handle_text_input_key(code) => {}
            _ => {}
        }
        Ok(false)
    }

    fn toggle_view_create_include(&mut self, category_id: CategoryId) {
        if !self.view_create_include_selection.insert(category_id) {
            self.view_create_include_selection.remove(&category_id);
        }
        self.view_create_exclude_selection.remove(&category_id);
    }

    fn toggle_view_create_exclude(&mut self, category_id: CategoryId) {
        if !self.view_create_exclude_selection.insert(category_id) {
            self.view_create_exclude_selection.remove(&category_id);
        }
        self.view_create_include_selection.remove(&category_id);
    }

    fn handle_view_create_category_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> Result<bool, String> {
        match code {
            KeyCode::Esc => {
                self.mode = Mode::ViewPicker;
                self.view_pending_name = None;
                self.view_create_include_selection.clear();
                self.view_create_exclude_selection.clear();
                self.status = "View create canceled".to_string();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if !self.category_rows.is_empty() {
                    self.view_category_index =
                        next_index(self.view_category_index, self.category_rows.len(), 1);
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if !self.category_rows.is_empty() {
                    self.view_category_index =
                        next_index(self.view_category_index, self.category_rows.len(), -1);
                }
            }
            KeyCode::Char(' ') | KeyCode::Char('+') => {
                if let Some(row) = self.category_rows.get(self.view_category_index) {
                    self.toggle_view_create_include(row.id);
                }
            }
            KeyCode::Char('-') => {
                if let Some(row) = self.category_rows.get(self.view_category_index) {
                    self.toggle_view_create_exclude(row.id);
                }
            }
            KeyCode::Enter => {
                let Some(name) = self.view_pending_name.clone() else {
                    self.mode = if self.view_return_to_manager {
                        Mode::ViewManagerScreen
                    } else {
                        Mode::ViewPicker
                    };
                    self.view_return_to_manager = false;
                    self.status = "View create failed: missing name".to_string();
                    return Ok(false);
                };

                let mut view = View::new(name.clone());
                if self.view_create_include_selection.is_empty()
                    && self.view_create_exclude_selection.is_empty()
                {
                    if let Some(row) = self.category_rows.get(self.view_category_index) {
                        view.criteria.include.insert(row.id);
                    }
                } else {
                    view.criteria
                        .include
                        .extend(self.view_create_include_selection.iter().copied());
                    view.criteria
                        .exclude
                        .extend(self.view_create_exclude_selection.iter().copied());
                }

                match agenda.store().create_view(&view) {
                    Ok(()) => {
                        self.refresh(agenda.store())?;
                        self.set_view_selection_by_name(&view.name);
                        self.mode = if self.view_return_to_manager {
                            Mode::ViewManagerScreen
                        } else {
                            Mode::Normal
                        };
                        self.view_pending_name = None;
                        self.view_create_include_selection.clear();
                        self.view_create_exclude_selection.clear();
                        self.view_return_to_manager = false;
                        if self.mode == Mode::ViewManagerScreen {
                            self.load_view_manager_rows_from_selected_view();
                        }
                        self.status = format!(
                            "Created view {} (include={}, exclude={})",
                            view.name,
                            view.criteria.include.len(),
                            view.criteria.exclude.len()
                        );
                    }
                    Err(err) => {
                        self.mode = if self.view_return_to_manager {
                            Mode::ViewManagerScreen
                        } else {
                            Mode::ViewPicker
                        };
                        self.view_create_include_selection.clear();
                        self.view_create_exclude_selection.clear();
                        self.view_return_to_manager = false;
                        if self.mode == Mode::ViewManagerScreen {
                            self.load_view_manager_rows_from_selected_view();
                        }
                        self.status = format!("View create failed: {err}");
                    }
                }
            }
            _ => {}
        }
        Ok(false)
    }

    fn handle_view_rename_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> Result<bool, String> {
        let return_mode = if self.view_return_to_manager {
            Mode::ViewManagerScreen
        } else {
            Mode::ViewPicker
        };
        match code {
            KeyCode::Esc => {
                self.mode = return_mode;
                self.clear_input();
                self.view_pending_edit_name = None;
                self.view_return_to_manager = false;
                if self.mode == Mode::ViewManagerScreen {
                    self.load_view_manager_rows_from_selected_view();
                }
                self.status = "View rename canceled".to_string();
            }
            KeyCode::Enter => {
                let Some(view_name) = self.view_pending_edit_name.clone() else {
                    self.mode = return_mode;
                    self.clear_input();
                    self.view_return_to_manager = false;
                    self.status = "View rename failed: no selected view".to_string();
                    return Ok(false);
                };

                let new_name = self.input.trim().to_string();
                if new_name.is_empty() {
                    self.mode = return_mode;
                    self.clear_input();
                    self.view_pending_edit_name = None;
                    self.view_return_to_manager = false;
                    self.status = "View rename canceled (empty name)".to_string();
                    return Ok(false);
                }

                let Some(mut view) = self
                    .views
                    .iter()
                    .find(|view| view.name.eq_ignore_ascii_case(&view_name))
                    .cloned()
                else {
                    self.mode = return_mode;
                    self.clear_input();
                    self.view_pending_edit_name = None;
                    self.view_return_to_manager = false;
                    self.status = "View rename failed: selected view not found".to_string();
                    return Ok(false);
                };

                if view.name == new_name {
                    self.mode = return_mode;
                    self.clear_input();
                    self.view_pending_edit_name = None;
                    self.view_return_to_manager = false;
                    self.status = "View rename canceled (unchanged)".to_string();
                    return Ok(false);
                }

                view.name = new_name.clone();
                match agenda.store().update_view(&view) {
                    Ok(()) => {
                        self.refresh(agenda.store())?;
                        self.set_view_selection_by_name(&new_name);
                        self.mode = return_mode;
                        self.clear_input();
                        self.view_pending_edit_name = None;
                        self.view_return_to_manager = false;
                        if self.mode == Mode::ViewManagerScreen {
                            self.load_view_manager_rows_from_selected_view();
                        }
                        self.status = format!("Renamed view to {}", new_name);
                    }
                    Err(err) => {
                        self.mode = return_mode;
                        self.clear_input();
                        self.view_pending_edit_name = None;
                        self.view_return_to_manager = false;
                        self.status = format!("View rename failed: {err}");
                    }
                }
            }
            _ if self.handle_text_input_key(code) => {}
            _ => {}
        }
        Ok(false)
    }

    fn open_view_editor(&mut self, view: View) {
        let preview_count = self.preview_count_for_query(&view.criteria);
        self.view_editor = Some(ViewEditorState {
            base_view_name: view.name.clone(),
            draft: view,
            category_index: first_non_reserved_category_index(&self.category_rows),
            bucket_index: 0,
            section_index: 0,
            action_index: 0,
            preview_count,
        });
        self.view_editor_return_to_manager = false;
        self.view_editor_category_target = None;
        self.view_editor_bucket_target = None;
        self.mode = Mode::ViewEditor;
    }

    fn open_view_manager_section_editor(&mut self) {
        let Some(view) = self.views.get(self.picker_index).cloned() else {
            self.status = "No selected view for section editing".to_string();
            return;
        };
        let preview_count = self.preview_count_for_query(&view.criteria);
        self.view_editor = Some(ViewEditorState {
            base_view_name: view.name.clone(),
            draft: view.clone(),
            category_index: first_non_reserved_category_index(&self.category_rows),
            bucket_index: 0,
            section_index: self
                .view_manager_section_index
                .min(view.sections.len().saturating_sub(1)),
            action_index: 4,
            preview_count,
        });
        self.view_editor_return_to_manager = true;
        self.view_editor_category_target = None;
        self.view_editor_bucket_target = None;
        self.mode = Mode::ViewSectionEditor;
        self.status = "Section editor: N/x/[/] and Enter detail, Esc return to manager".to_string();
    }

    fn open_view_manager_unmatched_settings(&mut self) {
        let Some(view) = self.views.get(self.picker_index).cloned() else {
            self.status = "No selected view for unmatched settings".to_string();
            return;
        };
        let preview_count = self.preview_count_for_query(&view.criteria);
        self.view_editor = Some(ViewEditorState {
            base_view_name: view.name.clone(),
            draft: view,
            category_index: first_non_reserved_category_index(&self.category_rows),
            bucket_index: 0,
            section_index: 0,
            action_index: 5,
            preview_count,
        });
        self.view_editor_return_to_manager = true;
        self.view_editor_category_target = None;
        self.view_editor_bucket_target = None;
        self.mode = Mode::ViewUnmatchedSettings;
        self.status = "Unmatched settings: t toggle, l label, Esc return to manager".to_string();
    }

    fn apply_view_editor_draft_to_selected_view_manager_view(&mut self) {
        if !self.view_editor_return_to_manager {
            return;
        }
        let Some(editor) = &self.view_editor else {
            return;
        };
        let Some(view) = self.views.get_mut(self.picker_index) else {
            return;
        };
        view.sections = editor.draft.sections.clone();
        view.show_unmatched = editor.draft.show_unmatched;
        view.unmatched_label = editor.draft.unmatched_label.clone();
        self.view_manager_section_index = self
            .view_manager_section_index
            .min(view.sections.len().saturating_sub(1));
        self.view_manager_dirty = true;
    }

    fn finish_view_editor_return_to_manager(&mut self, status: &str) {
        self.apply_view_editor_draft_to_selected_view_manager_view();
        self.view_editor = None;
        self.view_editor_return_to_manager = false;
        self.view_editor_category_target = None;
        self.view_editor_bucket_target = None;
        self.mode = Mode::ViewManagerScreen;
        self.status = status.to_string();
    }

    fn preview_count_for_query(&self, query: &Query) -> usize {
        let reference_date = Local::now().date_naive();
        evaluate_query(query, &self.all_items, reference_date).len()
    }

    fn refresh_view_editor_preview(&mut self) {
        if let Some(editor) = &mut self.view_editor {
            let reference_date = Local::now().date_naive();
            editor.preview_count =
                evaluate_query(&editor.draft.criteria, &self.all_items, reference_date).len();
        }
    }

    fn handle_view_editor_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> Result<bool, String> {
        const VIEW_EDITOR_ACTIONS: usize = 6;
        match code {
            KeyCode::Esc => {
                self.mode = Mode::ViewPicker;
                self.view_editor = None;
                self.view_editor_return_to_manager = false;
                self.view_editor_category_target = None;
                self.view_editor_bucket_target = None;
                self.clear_input();
                self.status = "View edit canceled".to_string();
            }
            KeyCode::Enter => {
                let Some(editor) = self.view_editor.clone() else {
                    self.mode = Mode::ViewPicker;
                    self.status = "View edit failed: no draft".to_string();
                    return Ok(false);
                };
                match agenda.store().update_view(&editor.draft) {
                    Ok(()) => {
                        self.refresh(agenda.store())?;
                        self.set_view_selection_by_name(&editor.base_view_name);
                        self.mode = Mode::ViewPicker;
                        self.view_editor = None;
                        self.view_editor_return_to_manager = false;
                        self.view_editor_category_target = None;
                        self.view_editor_bucket_target = None;
                        self.status = format!("Updated view {}", editor.base_view_name);
                    }
                    Err(err) => {
                        self.status = format!("View edit failed: {err}");
                    }
                }
            }
            KeyCode::Right | KeyCode::Char('o') => {
                if let Some(action_index) =
                    self.view_editor.as_ref().map(|editor| editor.action_index)
                {
                    self.activate_view_editor_action(action_index);
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(editor) = &mut self.view_editor {
                    editor.action_index = next_index(editor.action_index, VIEW_EDITOR_ACTIONS, 1);
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(editor) = &mut self.view_editor {
                    editor.action_index = next_index(editor.action_index, VIEW_EDITOR_ACTIONS, -1);
                }
            }
            KeyCode::Char('+') => {
                self.open_view_editor_category_picker(CategoryEditTarget::ViewInclude);
            }
            KeyCode::Char('-') => {
                self.open_view_editor_category_picker(CategoryEditTarget::ViewExclude);
            }
            KeyCode::Char(']') => {
                self.open_view_editor_bucket_picker(BucketEditTarget::ViewVirtualInclude);
            }
            KeyCode::Char('[') => {
                self.open_view_editor_bucket_picker(BucketEditTarget::ViewVirtualExclude);
            }
            KeyCode::Char('s') => {
                self.mode = Mode::ViewSectionEditor;
                self.status = "Section editor: j/k select, N add, x remove, [/] reorder, Enter edit, Esc back".to_string();
            }
            KeyCode::Char('u') => {
                self.mode = Mode::ViewUnmatchedSettings;
                self.status = "Unmatched: t toggle visibility, l edit label, Esc back".to_string();
            }
            _ => {}
        }
        Ok(false)
    }

    fn activate_view_editor_action(&mut self, action_index: usize) {
        match action_index {
            0 => self.open_view_editor_category_picker(CategoryEditTarget::ViewInclude),
            1 => self.open_view_editor_category_picker(CategoryEditTarget::ViewExclude),
            2 => self.open_view_editor_bucket_picker(BucketEditTarget::ViewVirtualInclude),
            3 => self.open_view_editor_bucket_picker(BucketEditTarget::ViewVirtualExclude),
            4 => {
                self.mode = Mode::ViewSectionEditor;
                self.status = "Section editor: j/k select, N add, x remove, [/] reorder, Enter edit, Esc back".to_string();
            }
            5 => {
                self.mode = Mode::ViewUnmatchedSettings;
                self.status = "Unmatched: t toggle visibility, l edit label, Esc back".to_string();
            }
            _ => {}
        }
    }

    fn open_view_editor_category_picker(&mut self, target: CategoryEditTarget) {
        if self.category_rows.is_empty() {
            self.status = "No categories available".to_string();
            return;
        }
        self.view_editor_category_target = Some(target);
        self.mode = Mode::ViewEditorCategoryPicker;
        self.status = "Category picker: j/k select, Space toggle, Enter/Esc back".to_string();
    }

    fn open_view_editor_bucket_picker(&mut self, target: BucketEditTarget) {
        self.view_editor_bucket_target = Some(target);
        self.mode = Mode::ViewEditorBucketPicker;
        self.status = "Bucket picker: j/k select, Space toggle, Enter/Esc back".to_string();
    }

    fn handle_view_editor_category_key(&mut self, code: KeyCode) -> Result<bool, String> {
        let Some(target) = self.view_editor_category_target else {
            self.mode = Mode::ViewEditor;
            return Ok(false);
        };
        match code {
            KeyCode::Esc | KeyCode::Enter => {
                self.view_editor_category_target = None;
                self.mode = if category_target_is_section(target) {
                    Mode::ViewSectionDetail
                } else {
                    Mode::ViewEditor
                };
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(editor) = &mut self.view_editor {
                    editor.category_index =
                        next_index(editor.category_index, self.category_rows.len(), 1);
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(editor) = &mut self.view_editor {
                    editor.category_index =
                        next_index(editor.category_index, self.category_rows.len(), -1);
                }
            }
            KeyCode::Char(' ') => {
                let Some(editor) = &mut self.view_editor else {
                    return Ok(false);
                };
                let row_index = editor
                    .category_index
                    .min(self.category_rows.len().saturating_sub(1));
                let Some(row) = self.category_rows.get(row_index).cloned() else {
                    return Ok(false);
                };
                if let Some(set) =
                    category_target_set_mut(&mut editor.draft, editor.section_index, target)
                {
                    if !set.insert(row.id) {
                        set.remove(&row.id);
                    }
                }
                self.refresh_view_editor_preview();
            }
            _ => {}
        }
        Ok(false)
    }

    fn handle_view_editor_bucket_key(&mut self, code: KeyCode) -> Result<bool, String> {
        let Some(target) = self.view_editor_bucket_target else {
            self.mode = Mode::ViewEditor;
            return Ok(false);
        };
        let buckets = when_bucket_options();
        match code {
            KeyCode::Esc | KeyCode::Enter => {
                self.view_editor_bucket_target = None;
                self.mode = if bucket_target_is_section(target) {
                    Mode::ViewSectionDetail
                } else {
                    Mode::ViewEditor
                };
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(editor) = &mut self.view_editor {
                    editor.bucket_index = next_index(editor.bucket_index, buckets.len(), 1);
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(editor) = &mut self.view_editor {
                    editor.bucket_index = next_index(editor.bucket_index, buckets.len(), -1);
                }
            }
            KeyCode::Char(' ') => {
                let Some(editor) = &mut self.view_editor else {
                    return Ok(false);
                };
                let bucket = buckets[editor.bucket_index.min(buckets.len().saturating_sub(1))];
                if let Some(set) =
                    bucket_target_set_mut(&mut editor.draft, editor.section_index, target)
                {
                    if !set.insert(bucket) {
                        set.remove(&bucket);
                    }
                }
                self.refresh_view_editor_preview();
            }
            _ => {}
        }
        Ok(false)
    }

    fn handle_view_section_editor_key(&mut self, code: KeyCode) -> Result<bool, String> {
        if self.view_editor.is_none() {
            if self.view_editor_return_to_manager {
                self.mode = Mode::ViewManagerScreen;
                self.view_editor_return_to_manager = false;
            } else {
                self.mode = Mode::ViewPicker;
            }
            return Ok(false);
        }
        match code {
            KeyCode::Esc => {
                if self.view_editor_return_to_manager {
                    self.finish_view_editor_return_to_manager(
                        "Updated sections in manager draft (press s to persist)",
                    );
                } else {
                    self.mode = Mode::ViewEditor;
                    self.status = "View editor".to_string();
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(editor) = &mut self.view_editor {
                    if !editor.draft.sections.is_empty() {
                        editor.section_index =
                            next_index(editor.section_index, editor.draft.sections.len(), 1);
                    }
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(editor) = &mut self.view_editor {
                    if !editor.draft.sections.is_empty() {
                        editor.section_index =
                            next_index(editor.section_index, editor.draft.sections.len(), -1);
                    }
                }
            }
            KeyCode::Char('N') => {
                if let Some(editor) = &mut self.view_editor {
                    let next = editor.draft.sections.len() + 1;
                    editor.draft.sections.push(Section {
                        title: format!("Section {next}"),
                        criteria: Query::default(),
                        on_insert_assign: HashSet::new(),
                        on_remove_unassign: HashSet::new(),
                        show_children: false,
                    });
                    editor.section_index = editor.draft.sections.len().saturating_sub(1);
                }
            }
            KeyCode::Char('x') => {
                if let Some(editor) = &mut self.view_editor {
                    if !editor.draft.sections.is_empty() {
                        editor.draft.sections.remove(editor.section_index);
                        editor.section_index = editor
                            .section_index
                            .min(editor.draft.sections.len().saturating_sub(1));
                    }
                }
            }
            KeyCode::Char('[') => {
                if let Some(editor) = &mut self.view_editor {
                    if editor.section_index > 0 {
                        editor
                            .draft
                            .sections
                            .swap(editor.section_index, editor.section_index - 1);
                        editor.section_index -= 1;
                    }
                }
            }
            KeyCode::Char(']') => {
                if let Some(editor) = &mut self.view_editor {
                    if editor.section_index + 1 < editor.draft.sections.len() {
                        editor
                            .draft
                            .sections
                            .swap(editor.section_index, editor.section_index + 1);
                        editor.section_index += 1;
                    }
                }
            }
            KeyCode::Enter | KeyCode::Char('e') => {
                if let Some(editor) = &self.view_editor {
                    if !editor.draft.sections.is_empty() {
                        self.mode = Mode::ViewSectionDetail;
                        self.status = "Section detail: t title, +/- categories, [/ ] virtual, a insert-set, r remove-set, h toggle children, Esc back".to_string();
                    }
                }
            }
            _ => {}
        }
        Ok(false)
    }

    fn handle_view_section_detail_key(&mut self, code: KeyCode) -> Result<bool, String> {
        let Some(section_index) = self.view_editor.as_ref().map(|editor| editor.section_index)
        else {
            self.mode = Mode::ViewPicker;
            return Ok(false);
        };
        let section_exists = self
            .view_editor
            .as_ref()
            .and_then(|editor| editor.draft.sections.get(section_index))
            .is_some();
        if !section_exists {
            self.mode = Mode::ViewSectionEditor;
            return Ok(false);
        }
        match code {
            KeyCode::Esc => {
                self.mode = Mode::ViewSectionEditor;
            }
            KeyCode::Char('t') => {
                let title = self
                    .view_editor
                    .as_ref()
                    .and_then(|editor| editor.draft.sections.get(section_index))
                    .map(|section| section.title.clone())
                    .unwrap_or_default();
                self.mode = Mode::ViewSectionTitleInput;
                self.set_input(title);
            }
            KeyCode::Char('+') => {
                self.open_view_editor_category_picker(CategoryEditTarget::SectionCriteriaInclude);
            }
            KeyCode::Char('-') => {
                self.open_view_editor_category_picker(CategoryEditTarget::SectionCriteriaExclude);
            }
            KeyCode::Char(']') => {
                self.open_view_editor_bucket_picker(BucketEditTarget::SectionVirtualInclude);
            }
            KeyCode::Char('[') => {
                self.open_view_editor_bucket_picker(BucketEditTarget::SectionVirtualExclude);
            }
            KeyCode::Char('a') => {
                self.open_view_editor_category_picker(CategoryEditTarget::SectionOnInsertAssign);
            }
            KeyCode::Char('r') => {
                self.open_view_editor_category_picker(CategoryEditTarget::SectionOnRemoveUnassign);
            }
            KeyCode::Char('h') => {
                if let Some(editor) = &mut self.view_editor {
                    if let Some(section) = editor.draft.sections.get_mut(section_index) {
                        section.show_children = !section.show_children;
                    }
                }
            }
            _ => {}
        }
        Ok(false)
    }

    fn handle_view_section_title_key(&mut self, code: KeyCode) -> Result<bool, String> {
        match code {
            KeyCode::Esc => {
                self.mode = Mode::ViewSectionDetail;
                self.clear_input();
            }
            KeyCode::Enter => {
                if let Some(editor) = &mut self.view_editor {
                    if let Some(section) = editor.draft.sections.get_mut(editor.section_index) {
                        let title = self.input.trim().to_string();
                        if !title.is_empty() {
                            section.title = title;
                        }
                    }
                }
                self.mode = Mode::ViewSectionDetail;
                self.clear_input();
            }
            _ if self.handle_text_input_key(code) => {}
            _ => {}
        }
        Ok(false)
    }

    fn handle_view_unmatched_settings_key(&mut self, code: KeyCode) -> Result<bool, String> {
        let label = self
            .view_editor
            .as_ref()
            .map(|editor| editor.draft.unmatched_label.clone());
        if label.is_none() {
            if self.view_editor_return_to_manager {
                self.mode = Mode::ViewManagerScreen;
                self.view_editor_return_to_manager = false;
            } else {
                self.mode = Mode::ViewPicker;
            }
            return Ok(false);
        }
        match code {
            KeyCode::Esc => {
                if self.view_editor_return_to_manager {
                    self.finish_view_editor_return_to_manager(
                        "Updated unmatched settings in manager draft (press s to persist)",
                    );
                } else {
                    self.mode = Mode::ViewEditor;
                }
            }
            KeyCode::Char('t') => {
                if let Some(editor) = &mut self.view_editor {
                    editor.draft.show_unmatched = !editor.draft.show_unmatched;
                }
            }
            KeyCode::Char('l') => {
                self.mode = Mode::ViewUnmatchedLabelInput;
                self.set_input(label.unwrap_or_default());
            }
            _ => {}
        }
        Ok(false)
    }

    fn handle_view_unmatched_label_key(&mut self, code: KeyCode) -> Result<bool, String> {
        match code {
            KeyCode::Esc => {
                self.mode = Mode::ViewUnmatchedSettings;
                self.clear_input();
            }
            KeyCode::Enter => {
                if let Some(editor) = &mut self.view_editor {
                    let label = self.input.trim().to_string();
                    if !label.is_empty() {
                        editor.draft.unmatched_label = label;
                    }
                }
                self.mode = Mode::ViewUnmatchedSettings;
                self.clear_input();
            }
            _ if self.handle_text_input_key(code) => {}
            _ => {}
        }
        Ok(false)
    }

    fn handle_confirm_delete_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> Result<bool, String> {
        match code {
            KeyCode::Char('y') => {
                if let Some(item_id) = self.selected_item_id() {
                    agenda
                        .delete_item(item_id, "user:tui")
                        .map_err(|e| e.to_string())?;
                    self.refresh(agenda.store())?;
                    self.status = "Item deleted".to_string();
                }
                self.mode = Mode::Normal;
            }
            KeyCode::Char('n') | KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.status = "Delete canceled".to_string();
            }
            _ => {}
        }
        Ok(false)
    }

    fn handle_category_manager_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> Result<bool, String> {
        match code {
            KeyCode::Esc | KeyCode::F(9) => {
                self.mode = Mode::Normal;
                self.clear_input();
                self.category_create_parent = None;
                self.category_reparent_options.clear();
                self.category_reparent_index = 0;
                self.category_config_editor = None;
                self.status = "Category manager closed".to_string();
            }
            KeyCode::Down | KeyCode::Char('j') => self.move_category_cursor(1),
            KeyCode::Up | KeyCode::Char('k') => self.move_category_cursor(-1),
            KeyCode::Char('n') => {
                self.mode = Mode::CategoryCreateInput;
                self.clear_input();
                self.category_create_parent = self.selected_category_id();
                let parent = self
                    .create_parent_name()
                    .unwrap_or_else(|| "top level".to_string());
                self.status = format!("Create subcategory under {parent}: type name and Enter");
            }
            KeyCode::Char('N') => {
                self.mode = Mode::CategoryCreateInput;
                self.clear_input();
                self.category_create_parent = None;
                self.status =
                    "Create top-level category (no parent): type name and Enter".to_string();
            }
            KeyCode::Char('r') => {
                if let Some(row) = self.selected_category_row() {
                    let row_name = row.name.clone();
                    self.mode = Mode::CategoryRenameInput;
                    self.set_input(row_name.clone());
                    self.status = format!("Rename category {}: type name and Enter", row_name);
                }
            }
            KeyCode::Char('p') => {
                if let Some(category_id) = self.selected_category_id() {
                    self.category_reparent_options =
                        build_reparent_options(&self.category_rows, &self.categories, category_id);
                    self.category_reparent_index = self
                        .selected_category_parent_index(category_id)
                        .unwrap_or(0)
                        .min(self.category_reparent_options.len().saturating_sub(1));
                    self.mode = Mode::CategoryReparentPicker;
                    self.status = "Reparent category: j/k select parent, Enter apply".to_string();
                }
            }
            KeyCode::Char('e') => {
                self.toggle_selected_category_exclusive(agenda)?;
            }
            KeyCode::Char('i') => {
                self.toggle_selected_category_implicit(agenda)?;
            }
            KeyCode::Char('a') => {
                self.toggle_selected_category_actionable(agenda)?;
            }
            KeyCode::Enter => {
                self.open_category_config_editor(agenda)?;
            }
            KeyCode::Char('x') => {
                if let Some(row) = self.selected_category_row() {
                    let row_name = row.name.clone();
                    self.mode = Mode::CategoryDeleteConfirm;
                    self.status = format!("Delete category \"{}\"? y/n", row_name);
                }
            }
            _ => {}
        }
        Ok(false)
    }

    fn open_category_config_editor(&mut self, agenda: &Agenda<'_>) -> Result<(), String> {
        let Some(row) = self.selected_category_row() else {
            self.status = "No selected category".to_string();
            return Ok(());
        };
        if row.is_reserved {
            self.status = format!("Category {} is reserved and cannot be edited", row.name);
            return Ok(());
        }

        let category = agenda
            .store()
            .get_category(row.id)
            .map_err(|e| e.to_string())?;
        let note = category.note.clone().unwrap_or_default();
        self.category_config_editor = Some(CategoryConfigEditorState {
            category_id: category.id,
            category_name: category.name.clone(),
            is_exclusive: category.is_exclusive,
            is_actionable: category.is_actionable,
            enable_implicit_string: category.enable_implicit_string,
            note_cursor: note.chars().count(),
            note,
            focus: CategoryConfigFocus::Exclusive,
        });
        self.mode = Mode::CategoryConfigEditor;
        self.status = format!(
            "Edit category config for {}: Space toggles, Enter saves (except note field)",
            category.name
        );
        Ok(())
    }

    fn save_category_config_editor(&mut self, agenda: &Agenda<'_>) -> Result<(), String> {
        let Some(editor) = self.category_config_editor.clone() else {
            self.mode = Mode::CategoryManager;
            self.status = "Category config editor closed".to_string();
            return Ok(());
        };

        let mut category = agenda
            .store()
            .get_category(editor.category_id)
            .map_err(|e| e.to_string())?;
        if is_reserved_category_name(&category.name) {
            self.mode = Mode::CategoryManager;
            self.category_config_editor = None;
            self.status = format!(
                "Category {} is reserved and cannot be edited",
                category.name
            );
            return Ok(());
        }

        let next_note = if editor.note.trim().is_empty() {
            None
        } else {
            Some(editor.note.clone())
        };
        if category.is_exclusive == editor.is_exclusive
            && category.is_actionable == editor.is_actionable
            && category.enable_implicit_string == editor.enable_implicit_string
            && category.note == next_note
        {
            self.mode = Mode::CategoryManager;
            self.category_config_editor = None;
            self.status = "Category config canceled: no changes".to_string();
            return Ok(());
        }

        category.is_exclusive = editor.is_exclusive;
        category.is_actionable = editor.is_actionable;
        category.enable_implicit_string = editor.enable_implicit_string;
        category.note = next_note;
        let result = agenda
            .update_category(&category)
            .map_err(|e| e.to_string())?;
        self.refresh(agenda.store())?;
        self.set_category_selection_by_id(category.id);
        self.mode = Mode::CategoryManager;
        self.category_config_editor = None;
        self.status = format!(
            "Updated {} (processed_items={}, affected_items={})",
            category.name, result.processed_items, result.affected_items
        );
        Ok(())
    }

    fn handle_category_config_editor_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> Result<bool, String> {
        if self.category_config_editor.is_none() {
            self.mode = Mode::CategoryManager;
            self.status = "Category config editor closed".to_string();
            return Ok(false);
        }
        let focus = self
            .category_config_editor
            .as_ref()
            .map(|editor| editor.focus)
            .unwrap_or(CategoryConfigFocus::Exclusive);
        let category_name = self
            .category_config_editor
            .as_ref()
            .map(|editor| editor.category_name.clone())
            .unwrap_or_else(|| "(unknown)".to_string());

        match code {
            KeyCode::Esc => {
                self.mode = Mode::CategoryManager;
                self.category_config_editor = None;
                self.status = format!("Canceled config changes for {}", category_name);
            }
            KeyCode::Tab | KeyCode::BackTab => {
                self.cycle_category_config_focus(if matches!(code, KeyCode::BackTab) {
                    -1
                } else {
                    1
                });
            }
            KeyCode::Left => {
                self.move_category_config_checkbox_focus(-1);
            }
            KeyCode::Right => {
                self.move_category_config_checkbox_focus(1);
            }
            KeyCode::Char('h') => {
                if !matches!(focus, CategoryConfigFocus::Note) {
                    self.move_category_config_checkbox_focus(-1);
                } else {
                    self.insert_category_config_note_char('h');
                }
            }
            KeyCode::Char('l') => {
                if !matches!(focus, CategoryConfigFocus::Note) {
                    self.move_category_config_checkbox_focus(1);
                } else {
                    self.insert_category_config_note_char('l');
                }
            }
            KeyCode::Char('e') => {
                if !matches!(focus, CategoryConfigFocus::Note) {
                    self.toggle_category_config_exclusive();
                } else {
                    self.insert_category_config_note_char('e');
                }
            }
            KeyCode::Char('i') => {
                if !matches!(focus, CategoryConfigFocus::Note) {
                    self.toggle_category_config_no_implicit();
                } else {
                    self.insert_category_config_note_char('i');
                }
            }
            KeyCode::Char('a') => {
                if !matches!(focus, CategoryConfigFocus::Note) {
                    self.toggle_category_config_actionable();
                } else {
                    self.insert_category_config_note_char('a');
                }
            }
            KeyCode::Char(' ') => match focus {
                CategoryConfigFocus::Exclusive => self.toggle_category_config_exclusive(),
                CategoryConfigFocus::NoImplicit => self.toggle_category_config_no_implicit(),
                CategoryConfigFocus::Actionable => self.toggle_category_config_actionable(),
                CategoryConfigFocus::Note => self.insert_category_config_note_char(' '),
                CategoryConfigFocus::SaveButton | CategoryConfigFocus::CancelButton => {}
            },
            KeyCode::Enter => match focus {
                CategoryConfigFocus::Exclusive
                | CategoryConfigFocus::NoImplicit
                | CategoryConfigFocus::Actionable => self.save_category_config_editor(agenda)?,
                CategoryConfigFocus::Note => self.insert_category_config_note_newline(),
                CategoryConfigFocus::SaveButton => self.save_category_config_editor(agenda)?,
                CategoryConfigFocus::CancelButton => {
                    self.mode = Mode::CategoryManager;
                    self.category_config_editor = None;
                    self.status = "Category config canceled".to_string();
                }
            },
            _ => {
                if matches!(focus, CategoryConfigFocus::Note) {
                    let _ = self.handle_category_config_note_input_key(code);
                }
            }
        }
        Ok(false)
    }

    fn toggle_selected_category_exclusive(&mut self, agenda: &Agenda<'_>) -> Result<(), String> {
        if self.selected_category_is_reserved() {
            self.status = "Reserved category config is read-only".to_string();
            return Ok(());
        }
        let Some(category_id) = self.selected_category_id() else {
            self.status = "No selected category".to_string();
            return Ok(());
        };
        let mut category = agenda
            .store()
            .get_category(category_id)
            .map_err(|e| e.to_string())?;
        category.is_exclusive = !category.is_exclusive;
        let updated = category.clone();
        let result = agenda
            .update_category(&category)
            .map_err(|e| e.to_string())?;
        self.refresh(agenda.store())?;
        self.set_category_selection_by_id(updated.id);
        self.status = format!(
            "{} exclusive={} (processed_items={}, affected_items={})",
            updated.name, updated.is_exclusive, result.processed_items, result.affected_items
        );
        Ok(())
    }

    fn toggle_selected_category_implicit(&mut self, agenda: &Agenda<'_>) -> Result<(), String> {
        if self.selected_category_is_reserved() {
            self.status = "Reserved category config is read-only".to_string();
            return Ok(());
        }
        let Some(category_id) = self.selected_category_id() else {
            self.status = "No selected category".to_string();
            return Ok(());
        };
        let mut category = agenda
            .store()
            .get_category(category_id)
            .map_err(|e| e.to_string())?;
        category.enable_implicit_string = !category.enable_implicit_string;
        let updated = category.clone();
        let result = agenda
            .update_category(&category)
            .map_err(|e| e.to_string())?;
        self.refresh(agenda.store())?;
        self.set_category_selection_by_id(updated.id);
        self.status = format!(
            "{} match-category-name={} (processed_items={}, affected_items={})",
            updated.name,
            updated.enable_implicit_string,
            result.processed_items,
            result.affected_items
        );
        Ok(())
    }

    fn toggle_selected_category_actionable(&mut self, agenda: &Agenda<'_>) -> Result<(), String> {
        if self.selected_category_is_reserved() {
            self.status = "Reserved category config is read-only".to_string();
            return Ok(());
        }
        let Some(category_id) = self.selected_category_id() else {
            self.status = "No selected category".to_string();
            return Ok(());
        };
        let mut category = agenda
            .store()
            .get_category(category_id)
            .map_err(|e| e.to_string())?;
        category.is_actionable = !category.is_actionable;
        let updated = category.clone();
        let result = agenda
            .update_category(&category)
            .map_err(|e| e.to_string())?;
        self.refresh(agenda.store())?;
        self.set_category_selection_by_id(updated.id);
        self.status = format!(
            "{} actionable={} (processed_items={}, affected_items={})",
            updated.name, updated.is_actionable, result.processed_items, result.affected_items
        );
        Ok(())
    }

    fn handle_category_create_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> Result<bool, String> {
        match code {
            KeyCode::Esc => {
                self.mode = Mode::CategoryManager;
                self.clear_input();
                self.category_create_parent = None;
                self.status = "Category create canceled".to_string();
            }
            KeyCode::Enter => {
                let name = self.input.trim().to_string();
                if !name.is_empty() {
                    let mut category = Category::new(name.clone());
                    category.enable_implicit_string = true;
                    category.parent = self.category_create_parent;
                    let parent_label = self
                        .create_parent_name()
                        .unwrap_or_else(|| "top level".to_string());
                    let create_result =
                        agenda.create_category(&category).map_err(|e| e.to_string());
                    match create_result {
                        Ok(result) => {
                            self.refresh(agenda.store())?;
                            self.set_category_selection_by_id(category.id);
                            self.mode = Mode::CategoryManager;
                            self.status = format!(
                                "Created category {} under {} (processed_items={}, affected_items={})",
                                category.name,
                                parent_label,
                                result.processed_items,
                                result.affected_items
                            );
                        }
                        Err(err) => {
                            self.mode = Mode::CategoryManager;
                            self.status = format!("Create failed: {err}");
                        }
                    }
                } else {
                    self.mode = Mode::CategoryManager;
                    self.status = "Category create canceled (empty name)".to_string();
                }
                self.clear_input();
                self.category_create_parent = None;
            }
            _ if self.handle_text_input_key(code) => {}
            _ => {}
        }
        Ok(false)
    }

    fn handle_category_rename_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> Result<bool, String> {
        match code {
            KeyCode::Esc => {
                self.mode = Mode::CategoryManager;
                self.clear_input();
                self.status = "Category rename canceled".to_string();
            }
            KeyCode::Enter => {
                let Some(category_id) = self.selected_category_id() else {
                    self.mode = Mode::CategoryManager;
                    self.clear_input();
                    self.status = "Category rename failed: no selection".to_string();
                    return Ok(false);
                };

                let new_name = self.input.trim().to_string();
                if new_name.is_empty() {
                    self.mode = Mode::CategoryManager;
                    self.clear_input();
                    self.status = "Category rename canceled (empty name)".to_string();
                    return Ok(false);
                }

                let mut category = agenda
                    .store()
                    .get_category(category_id)
                    .map_err(|e| e.to_string())?;
                if category.name == new_name {
                    self.mode = Mode::CategoryManager;
                    self.clear_input();
                    self.status = "Category rename canceled (unchanged)".to_string();
                    return Ok(false);
                }

                category.name = new_name.clone();
                let result = agenda
                    .update_category(&category)
                    .map_err(|e| e.to_string())?;
                self.refresh(agenda.store())?;
                self.set_category_selection_by_id(category_id);
                self.mode = Mode::CategoryManager;
                self.clear_input();
                self.status = format!(
                    "Renamed category to {} (processed_items={}, affected_items={})",
                    new_name, result.processed_items, result.affected_items
                );
            }
            _ if self.handle_text_input_key(code) => {}
            _ => {}
        }
        Ok(false)
    }

    fn handle_category_reparent_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> Result<bool, String> {
        match code {
            KeyCode::Esc => {
                self.mode = Mode::CategoryManager;
                self.category_reparent_options.clear();
                self.category_reparent_index = 0;
                self.status = "Category reparent canceled".to_string();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if !self.category_reparent_options.is_empty() {
                    self.category_reparent_index = next_index(
                        self.category_reparent_index,
                        self.category_reparent_options.len(),
                        1,
                    );
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if !self.category_reparent_options.is_empty() {
                    self.category_reparent_index = next_index(
                        self.category_reparent_index,
                        self.category_reparent_options.len(),
                        -1,
                    );
                }
            }
            KeyCode::Enter => {
                let Some(category_id) = self.selected_category_id() else {
                    self.mode = Mode::CategoryManager;
                    self.status = "Category reparent failed: no selection".to_string();
                    self.category_reparent_options.clear();
                    self.category_reparent_index = 0;
                    return Ok(false);
                };

                let Some(option) = self
                    .category_reparent_options
                    .get(self.category_reparent_index)
                    .cloned()
                else {
                    self.mode = Mode::CategoryManager;
                    self.status = "Category reparent failed: no parent selected".to_string();
                    self.category_reparent_options.clear();
                    self.category_reparent_index = 0;
                    return Ok(false);
                };

                let mut category = agenda
                    .store()
                    .get_category(category_id)
                    .map_err(|e| e.to_string())?;
                if category.parent == option.parent_id {
                    self.mode = Mode::CategoryManager;
                    self.status = "Category reparent canceled (unchanged)".to_string();
                    self.category_reparent_options.clear();
                    self.category_reparent_index = 0;
                    return Ok(false);
                }

                category.parent = option.parent_id;
                let result = agenda
                    .update_category(&category)
                    .map_err(|e| e.to_string())?;
                self.refresh(agenda.store())?;
                self.set_category_selection_by_id(category_id);
                self.mode = Mode::CategoryManager;
                self.status = format!(
                    "Reparented {} (processed_items={}, affected_items={})",
                    category.name, result.processed_items, result.affected_items
                );
                self.category_reparent_options.clear();
                self.category_reparent_index = 0;
            }
            _ => {}
        }

        Ok(false)
    }

    fn handle_category_delete_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> Result<bool, String> {
        match code {
            KeyCode::Char('y') => {
                if let Some(row) = self.selected_category_row().cloned() {
                    match agenda.store().delete_category(row.id) {
                        Ok(()) => {
                            self.refresh(agenda.store())?;
                            self.status = format!("Deleted category {}", row.name);
                        }
                        Err(err) => {
                            self.status = format!("Delete failed: {err}");
                        }
                    }
                }
                self.mode = Mode::CategoryManager;
            }
            KeyCode::Char('n') | KeyCode::Esc => {
                self.mode = Mode::CategoryManager;
                self.status = "Delete canceled".to_string();
            }
            _ => {}
        }
        Ok(false)
    }

    fn refresh(&mut self, store: &Store) -> Result<(), String> {
        self.views = store.list_views().map_err(|e| e.to_string())?;
        self.categories = store.get_hierarchy().map_err(|e| e.to_string())?;
        self.category_rows = build_category_rows(&self.categories);
        self.category_index = self
            .category_index
            .min(self.category_rows.len().saturating_sub(1));
        let items = store.list_items().map_err(|e| e.to_string())?;
        self.all_items = items.clone();

        let mut slots = Vec::new();
        if self.views.is_empty() {
            slots.push(Slot {
                title: "All Items (no views configured)".to_string(),
                items: items.clone(),
                context: SlotContext::Unmatched,
            });
            if self.mode == Mode::Normal {
                self.status = "No views configured; showing fallback item list".to_string();
            }
            self.view_index = 0;
            self.picker_index = 0;
        } else {
            self.view_index = self.view_index.min(self.views.len().saturating_sub(1));
            let view = self
                .current_view()
                .cloned()
                .ok_or("No active view".to_string())?;
            let reference_date = Local::now().date_naive();
            let result = resolve_view(&view, &items, &self.categories, reference_date);

            for section in result.sections {
                if section.subsections.is_empty() {
                    slots.push(Slot {
                        title: section.title,
                        items: section.items,
                        context: SlotContext::Section {
                            section_index: section.section_index,
                        },
                    });
                    continue;
                }

                for subsection in section.subsections {
                    slots.push(Slot {
                        title: format!("{} / {}", section.title, subsection.title),
                        items: subsection.items,
                        context: SlotContext::GeneratedSection {
                            on_insert_assign: subsection.on_insert_assign,
                            on_remove_unassign: subsection.on_remove_unassign,
                        },
                    });
                }
            }

            if let Some(unmatched_items) = result.unmatched {
                if should_render_unmatched_lane(&unmatched_items) {
                    slots.push(Slot {
                        title: result
                            .unmatched_label
                            .unwrap_or_else(|| "Unassigned".to_string()),
                        items: unmatched_items,
                        context: SlotContext::Unmatched,
                    });
                }
            }

            if slots.is_empty() {
                slots.push(Slot {
                    title: "No visible sections".to_string(),
                    items: Vec::new(),
                    context: SlotContext::Unmatched,
                });
            }
        }

        if let Some(filter) = &self.filter {
            let needle = filter.to_ascii_lowercase();
            for slot in &mut slots {
                slot.items.retain(|item| item_text_matches(item, &needle));
            }
        }

        self.slots = slots;
        self.slot_index = self.slot_index.min(self.slots.len().saturating_sub(1));
        self.item_index = self.item_index.min(
            self.current_slot()
                .map(|slot| slot.items.len().saturating_sub(1))
                .unwrap_or(0),
        );
        let provenance_len = self
            .selected_item()
            .map(|item| self.inspect_assignment_rows_for_item(item).len())
            .unwrap_or(0);
        let summary_len = self
            .selected_item()
            .map(|item| self.item_details_lines_for_item(item).len())
            .unwrap_or(0);
        self.inspect_assignment_index = self
            .inspect_assignment_index
            .min(provenance_len.saturating_sub(1));
        self.preview_provenance_scroll = self
            .preview_provenance_scroll
            .min(provenance_len.saturating_sub(1));
        self.preview_summary_scroll = self
            .preview_summary_scroll
            .min(summary_len.saturating_sub(1));

        Ok(())
    }

    fn draw(&self, frame: &mut ratatui::Frame<'_>) {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(1),
                Constraint::Length(3),
            ])
            .split(frame.area());

        let header = self.render_header();
        frame.render_widget(header, layout[0]);

        self.render_main(frame, layout[1]);

        let footer = self.render_footer();
        let footer_area = layout[2];
        frame.render_widget(footer, footer_area);
        if let Some((x, y)) = self.input_cursor_position(footer_area) {
            frame.set_cursor_position((x, y));
        }
        if self.mode == Mode::ItemEditInput {
            let popup_area = item_edit_popup_area(frame.area());
            self.render_item_edit_popup(frame, popup_area);
            if let Some((x, y)) = self.item_edit_cursor_position(popup_area) {
                frame.set_cursor_position((x, y));
            }
        }
        if self.mode == Mode::CategoryConfigEditor {
            let popup_area = category_config_popup_area(frame.area());
            self.render_category_config_editor(frame, popup_area);
            if let Some((x, y)) = self.category_config_cursor_position(popup_area) {
                frame.set_cursor_position((x, y));
            }
        }

        if matches!(
            self.mode,
            Mode::ViewPicker
                | Mode::ViewCreateNameInput
                | Mode::ViewRenameInput
                | Mode::ViewDeleteConfirm
        ) {
            self.render_view_picker(frame, centered_rect(60, 60, frame.area()));
        }
        if matches!(
            self.mode,
            Mode::ItemAssignCategoryPicker | Mode::ItemAssignCategoryInput
        ) {
            self.render_item_assign_picker(frame, centered_rect(72, 72, frame.area()));
        }
        if matches!(self.mode, Mode::ViewCreateCategoryPicker) {
            self.render_view_category_picker(frame, centered_rect(72, 72, frame.area()));
        }
        if self.mode == Mode::ViewEditor {
            self.render_view_editor(frame, centered_rect(72, 72, frame.area()));
        }
        if self.mode == Mode::ViewEditorCategoryPicker {
            self.render_view_editor_category_picker(frame, centered_rect(72, 72, frame.area()));
        }
        if self.mode == Mode::ViewManagerCategoryPicker {
            self.render_view_manager_category_picker(frame, centered_rect(72, 72, frame.area()));
        }
        if self.mode == Mode::ViewEditorBucketPicker {
            self.render_view_editor_bucket_picker(frame, centered_rect(60, 60, frame.area()));
        }
        if self.mode == Mode::ViewSectionEditor {
            self.render_view_section_editor(frame, centered_rect(72, 72, frame.area()));
        }
        if matches!(
            self.mode,
            Mode::ViewSectionDetail | Mode::ViewSectionTitleInput
        ) {
            self.render_view_section_detail(frame, centered_rect(72, 72, frame.area()));
        }
        if matches!(
            self.mode,
            Mode::ViewUnmatchedSettings | Mode::ViewUnmatchedLabelInput
        ) {
            self.render_view_unmatched_settings(frame, centered_rect(60, 40, frame.area()));
        }
    }

    fn input_prompt_prefix(&self) -> Option<&'static str> {
        match self.mode {
            Mode::AddInput => Some("Add> "),
            Mode::NoteEditInput => Some("Note> "),
            Mode::FilterInput => Some("Filter> "),
            Mode::ViewCreateNameInput => Some("View create> "),
            Mode::ViewRenameInput => Some("View rename> "),
            Mode::ViewSectionTitleInput => Some("Section title> "),
            Mode::ViewUnmatchedLabelInput => Some("Unmatched label> "),
            Mode::CategoryCreateInput => Some("Category create> "),
            Mode::CategoryRenameInput => Some("Category rename> "),
            Mode::ItemAssignCategoryInput => Some("Category> "),
            _ => None,
        }
    }

    fn input_cursor_position(&self, footer_area: Rect) -> Option<(u16, u16)> {
        let prefix = self.input_prompt_prefix()?;
        if footer_area.width < 3 || footer_area.height < 3 {
            return None;
        }

        let inner_x = footer_area.x.saturating_add(1);
        let inner_y = footer_area.y.saturating_add(1);
        let max_inner_x = footer_area
            .x
            .saturating_add(footer_area.width.saturating_sub(2));

        let input_chars = self.clamped_input_cursor().min(u16::MAX as usize) as u16;
        let prefix_chars = prefix.chars().count().min(u16::MAX as usize) as u16;
        let raw_x = inner_x
            .saturating_add(prefix_chars)
            .saturating_add(input_chars);
        let cursor_x = raw_x.min(max_inner_x);

        Some((cursor_x, inner_y))
    }

    fn item_edit_cursor_position(&self, popup_area: Rect) -> Option<(u16, u16)> {
        if self.mode != Mode::ItemEditInput {
            return None;
        }
        if popup_area.width < 3 || popup_area.height < 3 {
            return None;
        }
        let regions = item_edit_popup_regions(popup_area)?;
        match self.item_edit_focus {
            ItemEditFocus::Text => {
                let prefix_len = "  Text> ".chars().count().min(u16::MAX as usize) as u16;
                let input_chars = self.clamped_input_cursor().min(u16::MAX as usize) as u16;
                let max_x = regions
                    .text
                    .x
                    .saturating_add(regions.text.width.saturating_sub(1));
                let cursor_x = regions
                    .text
                    .x
                    .saturating_add(prefix_len)
                    .saturating_add(input_chars)
                    .min(max_x);
                Some((cursor_x, regions.text.y))
            }
            ItemEditFocus::Note => {
                if regions.note_inner.width == 0 || regions.note_inner.height == 0 {
                    return None;
                }
                let (line, col) = note_cursor_line_col(
                    &self.item_edit_note,
                    self.clamped_item_edit_note_cursor(),
                );
                let scroll = list_scroll_for_selected_line(regions.note, Some(line)) as usize;
                let visible_line = line.saturating_sub(scroll);
                let max_x = regions
                    .note_inner
                    .x
                    .saturating_add(regions.note_inner.width.saturating_sub(1));
                let max_y = regions
                    .note_inner
                    .y
                    .saturating_add(regions.note_inner.height.saturating_sub(1));
                let cursor_x = regions
                    .note_inner
                    .x
                    .saturating_add(col.min(u16::MAX as usize) as u16)
                    .min(max_x);
                let cursor_y = regions
                    .note_inner
                    .y
                    .saturating_add(visible_line.min(u16::MAX as usize) as u16)
                    .min(max_y);
                Some((cursor_x, cursor_y))
            }
            ItemEditFocus::CategoriesButton
            | ItemEditFocus::SaveButton
            | ItemEditFocus::CancelButton => None,
        }
    }

    fn category_config_cursor_position(&self, popup_area: Rect) -> Option<(u16, u16)> {
        if self.mode != Mode::CategoryConfigEditor {
            return None;
        }
        let Some(editor) = &self.category_config_editor else {
            return None;
        };
        if popup_area.width < 3 || popup_area.height < 3 {
            return None;
        }
        let regions = category_config_popup_regions(popup_area)?;
        if editor.focus != CategoryConfigFocus::Note {
            return None;
        }
        if regions.note_inner.width == 0 || regions.note_inner.height == 0 {
            return None;
        }

        let cursor = self.category_config_note_cursor().unwrap_or(0);
        let (line, col) = note_cursor_line_col(&editor.note, cursor);
        let scroll = list_scroll_for_selected_line(regions.note, Some(line)) as usize;
        let visible_line = line.saturating_sub(scroll);
        let max_x = regions
            .note_inner
            .x
            .saturating_add(regions.note_inner.width.saturating_sub(1));
        let max_y = regions
            .note_inner
            .y
            .saturating_add(regions.note_inner.height.saturating_sub(1));
        let cursor_x = regions
            .note_inner
            .x
            .saturating_add(col.min(u16::MAX as usize) as u16)
            .min(max_x);
        let cursor_y = regions
            .note_inner
            .y
            .saturating_add(visible_line.min(u16::MAX as usize) as u16)
            .min(max_y);
        Some((cursor_x, cursor_y))
    }

    fn render_header(&self) -> Paragraph<'_> {
        let view_name = self
            .current_view()
            .map(|view| view.name.as_str())
            .unwrap_or("(none)");
        let mode = format!("{:?}", self.mode);
        let filter = self
            .filter
            .as_ref()
            .map(|value| format!(" filter:{value}"))
            .unwrap_or_default();

        Paragraph::new(Line::from(vec![
            Span::styled(
                "Agenda Reborn",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!("  view:{view_name}  mode:{mode}{filter}")),
        ]))
    }

    fn render_main(&self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        if self.mode == Mode::ViewManagerScreen {
            self.render_view_manager_screen(frame, area);
            return;
        }
        if matches!(
            self.mode,
            Mode::CategoryManager
                | Mode::CategoryCreateInput
                | Mode::CategoryRenameInput
                | Mode::CategoryReparentPicker
                | Mode::CategoryDeleteConfirm
                | Mode::CategoryConfigEditor
        ) {
            self.render_category_manager(frame, area);
            return;
        }
        if self.show_preview {
            let split = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
                .split(area);
            self.render_board_columns(frame, split[0]);
            frame.render_widget(self.render_preview_panel(), split[1]);
        } else {
            self.render_board_columns(frame, area);
        }
    }

    fn render_view_manager_screen(&self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        let panes = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(30),
                Constraint::Percentage(40),
                Constraint::Percentage(30),
            ])
            .split(area);

        let selected_view = self.views.get(self.picker_index);
        let category_names = category_name_map(&self.categories);
        let views_lines: Vec<Line<'_>> = if self.views.is_empty() {
            vec![Line::from("(no views)")]
        } else {
            self.views
                .iter()
                .enumerate()
                .map(|(index, view)| {
                    let text = format!(
                        "{}{}",
                        if index == self.picker_index {
                            "> "
                        } else {
                            "  "
                        },
                        view.name
                    );
                    if index == self.picker_index {
                        Line::styled(text, selected_row_style())
                    } else {
                        Line::from(text)
                    }
                })
                .collect()
        };
        let views_border = if self.view_manager_pane == ViewManagerPane::Views {
            Color::Cyan
        } else {
            Color::Blue
        };
        frame.render_widget(
            Paragraph::new(views_lines).block(
                Block::default()
                    .title("Views")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(views_border)),
            ),
            panes[0],
        );

        let mut definition_lines = vec![Line::from("Criteria (shell)"), Line::from("")];
        if let Some(view) = selected_view {
            let validation_errors = self.view_manager_representability_errors();
            definition_lines.push(Line::from(format!("View: {}", view.name)));
            definition_lines.push(Line::from(format!(
                "Rows: {}{}",
                self.view_manager_rows.len(),
                if self.view_manager_dirty {
                    "  *unsaved*"
                } else {
                    ""
                }
            )));
            definition_lines.push(Line::from(format!(
                "Preview matching: {}",
                self.view_manager_preview_count
            )));
            if validation_errors.is_empty() {
                definition_lines.push(Line::from("Validation: ok"));
            } else {
                definition_lines.push(Line::styled(
                    format!("Validation errors: {}", validation_errors.len()),
                    Style::default().fg(Color::Red),
                ));
                definition_lines.push(Line::styled(
                    format!("  - {}", validation_errors[0]),
                    Style::default().fg(Color::Red),
                ));
            }
            definition_lines.push(Line::from(""));
            if self.view_manager_rows.is_empty() {
                definition_lines.push(Line::from("(no criteria rows)"));
            } else {
                definition_lines.extend(self.view_manager_rows.iter().enumerate().map(
                    |(index, row)| {
                        let marker = if index == self.view_manager_definition_index {
                            "> "
                        } else {
                            "  "
                        };
                        let join = if index == 0 {
                            "  "
                        } else if row.join_is_or {
                            "OR"
                        } else {
                            "AND"
                        };
                        let sign = match row.sign {
                            ViewCriteriaSign::Include => '+',
                            ViewCriteriaSign::Exclude => '-',
                        };
                        let category_name = category_names
                            .get(&row.category_id)
                            .cloned()
                            .unwrap_or_else(|| row.category_id.to_string());
                        let text = format!(
                            "{marker}{join} {}{} {}",
                            "  ".repeat(row.depth),
                            sign,
                            category_name
                        );
                        if index == self.view_manager_definition_index {
                            Line::styled(text, selected_row_style())
                        } else {
                            Line::from(text)
                        }
                    },
                ));
            }
        } else {
            definition_lines.push(Line::from("(no selected view)"));
        }
        let definition_border = if self.view_manager_pane == ViewManagerPane::Definition {
            Color::Cyan
        } else {
            Color::Blue
        };
        frame.render_widget(
            Paragraph::new(definition_lines).block(
                Block::default()
                    .title("Definition")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(definition_border)),
            ),
            panes[1],
        );

        let mut section_lines = vec![Line::from("Sections"), Line::from("")];
        if let Some(view) = selected_view {
            if view.sections.is_empty() {
                section_lines.push(Line::from("(no sections configured)"));
            } else {
                section_lines.extend(view.sections.iter().enumerate().map(|(index, section)| {
                    let text = format!(
                        "{}{}",
                        if index == self.view_manager_section_index {
                            "> "
                        } else {
                            "  "
                        },
                        section.title
                    );
                    if index == self.view_manager_section_index {
                        Line::styled(text, selected_row_style())
                    } else {
                        Line::from(text)
                    }
                }));
            }
            section_lines.push(Line::from(""));
            section_lines.push(Line::from(format!(
                "Unmatched: {}",
                if view.show_unmatched { "on" } else { "off" }
            )));
            section_lines.push(Line::from(format!(
                "Label: {}",
                if view.unmatched_label.trim().is_empty() {
                    "Unassigned".to_string()
                } else {
                    view.unmatched_label.clone()
                }
            )));
        } else {
            section_lines.push(Line::from("(no selected view)"));
        }
        let section_border = if self.view_manager_pane == ViewManagerPane::Sections {
            Color::Cyan
        } else {
            Color::Blue
        };
        frame.render_widget(
            Paragraph::new(section_lines).block(
                Block::default()
                    .title("Sections")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(section_border)),
            ),
            panes[2],
        );
    }

    fn render_board_columns(&self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        if self.slots.is_empty() {
            frame.render_widget(
                Paragraph::new(vec![Line::from("(no sections)")]).block(
                    Block::default()
                        .title("Board")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Blue)),
                ),
                area,
            );
            return;
        }

        let slot_count = self.slots.len() as u16;
        let pct = (100 / slot_count).max(1);
        let constraints: Vec<Constraint> = (0..self.slots.len())
            .map(|_| Constraint::Percentage(pct))
            .collect();
        let columns = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(area);

        let category_names = category_name_map(&self.categories);
        for (slot_index, slot) in self.slots.iter().enumerate() {
            let is_selected_slot = slot_index == self.slot_index;
            let inner_width = columns[slot_index].width.saturating_sub(2);
            let widths = board_column_widths(inner_width);
            let mut lines: Vec<Line<'_>> = vec![Line::from(board_annotation_header(widths))];
            if slot.items.is_empty() {
                lines.push(Line::from("(no items)"));
            } else {
                lines.extend(slot.items.iter().enumerate().map(|(item_index, item)| {
                    let when = item
                        .when_date
                        .map(|dt| dt.to_string())
                        .unwrap_or_else(|| "-".to_string());
                    let item_text = if item.is_done {
                        format!("[done] {}", item.text)
                    } else {
                        item.text.clone()
                    };
                    let categories = item_assignment_labels(item, &category_names);
                    let categories_text = if categories.is_empty() {
                        "-".to_string()
                    } else {
                        categories.join(", ")
                    };
                    let is_selected = is_selected_slot && item_index == self.item_index;
                    let row_text =
                        board_item_row(is_selected, &when, &item_text, &categories_text, widths);
                    if is_selected {
                        Line::styled(row_text, selected_row_style())
                    } else {
                        Line::from(row_text)
                    }
                }));
            }
            let title = format!("{} ({})", slot.title, slot.items.len());
            let border_color = if is_selected_slot {
                Color::Cyan
            } else {
                Color::Blue
            };
            frame.render_widget(
                Paragraph::new(lines).block(
                    Block::default()
                        .title(title)
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(border_color)),
                ),
                columns[slot_index],
            );
        }
    }

    fn render_preview_provenance_panel(&self) -> Paragraph<'_> {
        let mut lines = vec![
            Line::from("Provenance"),
            Line::from("Tab focus | j/k or J/K scroll | o summary | u unassign"),
        ];
        if let Some(item) = self.selected_item() {
            let rows = self.inspect_assignment_rows_for_item(item);
            if rows.is_empty() {
                lines.push(Line::from("(no assignments)"));
            } else {
                let is_picker_mode = self.mode == Mode::InspectUnassignPicker;
                for (index, row) in rows.iter().enumerate() {
                    let marker = if is_picker_mode && index == self.inspect_assignment_index {
                        "> "
                    } else {
                        "  "
                    };
                    lines.push(Line::from(format!(
                        "{marker}{} | source={} | origin={}",
                        row.category_name, row.source_label, row.origin_label
                    )));
                }
            }
        } else {
            lines.push(Line::from("(no selected item)"));
        }

        Paragraph::new(lines)
            .block(
                Block::default()
                    .title("Preview: Provenance")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(
                        if self.normal_focus == NormalFocus::Preview {
                            Color::Cyan
                        } else {
                            Color::Yellow
                        },
                    )),
            )
            .scroll((
                self.preview_provenance_scroll.min(u16::MAX as usize) as u16,
                0,
            ))
            .wrap(Wrap { trim: false })
    }

    fn item_details_lines_for_item(&self, item: &Item) -> Vec<Line<'_>> {
        let category_names = category_name_map(&self.categories);
        let categories = item_assignment_labels(item, &category_names);
        let mut lines = vec![
            Line::from("Summary"),
            Line::from("Tab focus | j/k or J/K scroll | o provenance"),
            Line::from(""),
            Line::from("Categories"),
        ];

        if categories.is_empty() {
            lines.push(Line::from("  (none)"));
        } else {
            lines.push(Line::from(format!("  {}", categories.join(", "))));
        }

        lines.push(Line::from(""));
        lines.push(Line::from("Note"));
        match &item.note {
            Some(note) if !note.is_empty() => {
                for line in note.lines() {
                    lines.push(Line::from(format!("  {}", line)));
                }
            }
            _ => lines.push(Line::from("  (none)")),
        }
        lines
    }

    fn render_preview_summary_panel(&self) -> Paragraph<'_> {
        let lines = if let Some(item) = self.selected_item() {
            self.item_details_lines_for_item(item)
        } else {
            vec![
                Line::from("Summary"),
                Line::from("Tab focus | j/k or J/K scroll | o provenance"),
                Line::from(""),
                Line::from("(no selected item)"),
            ]
        };

        Paragraph::new(lines)
            .block(
                Block::default()
                    .title("Preview: Summary")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(
                        if self.normal_focus == NormalFocus::Preview {
                            Color::Cyan
                        } else {
                            Color::Yellow
                        },
                    )),
            )
            .scroll((self.preview_summary_scroll.min(u16::MAX as usize) as u16, 0))
            .wrap(Wrap { trim: false })
    }

    fn render_preview_panel(&self) -> Paragraph<'_> {
        match self.preview_mode {
            PreviewMode::Summary => self.render_preview_summary_panel(),
            PreviewMode::Provenance => self.render_preview_provenance_panel(),
        }
    }

    fn render_footer(&self) -> Paragraph<'_> {
        let prompt = match self.mode {
            Mode::AddInput => format!("Add> {}", self.input),
            Mode::NoteEditInput => format!("Note> {}", self.input),
            Mode::FilterInput => format!("Filter> {}", self.input),
            Mode::ConfirmDelete => "Delete selected item? y/n".to_string(),
            Mode::ViewCreateNameInput => format!("View create> {}", self.input),
            Mode::ViewRenameInput => format!("View rename> {}", self.input),
            Mode::ViewDeleteConfirm => "Delete selected view? y/n".to_string(),
            Mode::ViewCreateCategoryPicker => {
                "Set include/exclude categories for new view".to_string()
            }
            Mode::ViewManagerCategoryPicker => "Pick category for criteria row".to_string(),
            Mode::ViewManagerScreen => format!(
                "View manager pane:{:?} preview:{}{}",
                self.view_manager_pane,
                self.view_manager_preview_count,
                if self.view_manager_dirty {
                    "  *unsaved*"
                } else {
                    ""
                }
            ),
            Mode::ViewSectionTitleInput => format!("Section title> {}", self.input),
            Mode::ViewUnmatchedLabelInput => format!("Unmatched label> {}", self.input),
            Mode::CategoryCreateInput => format!("Category create> {}", self.input),
            Mode::CategoryRenameInput => format!("Category rename> {}", self.input),
            Mode::CategoryReparentPicker => "Select category parent".to_string(),
            Mode::CategoryDeleteConfirm => "Delete selected category? y/n".to_string(),
            Mode::CategoryConfigEditor => {
                if let Some(editor) = &self.category_config_editor {
                    format!("Edit category config (focus: {:?})", editor.focus)
                } else {
                    "Edit category config".to_string()
                }
            }
            Mode::ItemAssignCategoryPicker => "Select category for selected item".to_string(),
            Mode::ItemAssignCategoryInput => format!("Category> {}", self.input),
            Mode::InspectUnassignPicker => "Select assignment".to_string(),
            Mode::ItemEditInput => format!(
                "Edit item fields in popup (focus: {})",
                match self.item_edit_focus {
                    ItemEditFocus::Text => "Text",
                    ItemEditFocus::Note => "Note",
                    ItemEditFocus::CategoriesButton => "Categories",
                    ItemEditFocus::SaveButton => "Save",
                    ItemEditFocus::CancelButton => "Cancel",
                }
            ),
            _ => self.status.clone(),
        };
        let footer_title = match self.mode {
            Mode::CategoryManager => {
                "j/k:row  Enter:config popup  e:exclusive  i:match-name  a:actionable  n/N:create  r:rename  p:reparent  x:delete  Esc/F9:close"
            }
            Mode::CategoryCreateInput => "Type category name, Enter:create, Esc:cancel",
            Mode::CategoryRenameInput => "Type new category name, Enter:rename, Esc:cancel",
            Mode::CategoryReparentPicker => "j/k:select parent  Enter:reparent  Esc:cancel",
            Mode::CategoryDeleteConfirm => "y:confirm delete  n:cancel",
            Mode::CategoryConfigEditor => {
                "Tab/Shift+Tab:focus  h/l:checkbox focus  Space:toggle  Enter:save (except note)  e/i/a:quick toggle  Esc:cancel"
            }
            Mode::ViewPicker => {
                "j/k:select  Enter:switch  N:create  r:rename  x:delete  e:edit view  V:view manager  Esc:cancel"
            }
            Mode::ViewManagerScreen => {
                "Tab/Shift+Tab:pane  j/k:row  Enter:activate  N:add  x:remove  [/] reorder  a/o:join  (/):depth  c:pick-category  u:unmatched  s:save  q/Esc:back"
            }
            Mode::ViewManagerCategoryPicker => "j/k:select  Enter/Space:choose  Esc:cancel",
            Mode::ViewCreateNameInput => "Type view name, Enter:next, Esc:cancel",
            Mode::ViewRenameInput => "Type new view name, Enter:rename, Esc:cancel",
            Mode::ViewDeleteConfirm => "y:confirm delete  n/Esc:cancel",
            Mode::ViewCreateCategoryPicker => {
                "j/k:select category  +:include  -:exclude  Space:+include  Enter:create view  Esc:cancel"
            }
            Mode::ViewEditor => "j/k:select  o/right:open  +|-|[|]:quick open  s/u:sections/unmatched  Enter:save  Esc:cancel",
            Mode::ViewEditorCategoryPicker => "j/k:select category  Space:toggle  Enter/Esc:back",
            Mode::ViewEditorBucketPicker => "j/k:select bucket  Space:toggle  Enter/Esc:back",
            Mode::ViewSectionEditor => "j/k:select  N:add  x:remove  [/] reorder  Enter:edit  Esc:back",
            Mode::ViewSectionDetail => "t:title  +/-:criteria  [/] virtual  a:on-insert  r:on-remove  h:children  Esc:back",
            Mode::ViewSectionTitleInput => "Type section title, Enter:save, Esc:cancel",
            Mode::ViewUnmatchedSettings => "t:toggle unmatched  l:label  Esc:back",
            Mode::ViewUnmatchedLabelInput => "Type unmatched label, Enter:save, Esc:cancel",
            Mode::ItemAssignCategoryPicker => "j/k:select category  Space:toggle add/remove  n or /:type name assign/create  Enter:done  Esc:cancel",
            Mode::ItemAssignCategoryInput => "Type category name, Enter:assign/create, Esc:back",
            Mode::ItemEditInput => {
                "Edit popup: Tab/Shift+Tab navigate  Enter activate  Up/Down note  Esc cancel  F3 categories"
            }
            Mode::NoteEditInput => "Edit selected note, Enter:save (empty clears), Esc:cancel",
            Mode::InspectUnassignPicker => "j/k:select assignment  Enter:apply  Esc:cancel",
            _ => {
                "n:add  Enter/e:edit-item  a/u:item-categories  m:note  [/]:filter  v/F8:views  c/F9:categories  g:all-items  ,/.:view  p:preview  o:preview-mode  Tab:board/preview focus  []:move  r:remove  d/D:done-toggle  x:delete  J/K:preview-scroll  q:quit"
            }
        };

        Paragraph::new(prompt).block(Block::default().title(footer_title).borders(Borders::ALL))
    }

    fn render_item_edit_popup(&self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        frame.render_widget(Clear, area);
        let block = Block::default()
            .title("Edit Item")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));
        frame.render_widget(block, area);
        let Some(regions) = item_edit_popup_regions(area) else {
            return;
        };
        let text_marker = if self.item_edit_focus == ItemEditFocus::Text {
            ">"
        } else {
            " "
        };
        let categories_button = if self.item_edit_focus == ItemEditFocus::CategoriesButton {
            "[> Categories <]"
        } else {
            "[Categories]"
        };
        let save_button = if self.item_edit_focus == ItemEditFocus::SaveButton {
            "[> Save <]"
        } else {
            "[Save]"
        };
        let cancel_button = if self.item_edit_focus == ItemEditFocus::CancelButton {
            "[> Cancel <]"
        } else {
            "[Cancel]"
        };

        frame.render_widget(Paragraph::new("Edit selected item"), regions.heading);
        frame.render_widget(
            Paragraph::new(format!("{text_marker} Text> {}", self.input)),
            regions.text,
        );

        let note_lines: Vec<Line<'_>> = if self.item_edit_note.is_empty() {
            vec![Line::from("")]
        } else {
            self.item_edit_note.lines().map(Line::from).collect()
        };
        let note_border_color = if self.item_edit_focus == ItemEditFocus::Note {
            Color::Cyan
        } else {
            Color::Blue
        };
        let note_title = if self.item_edit_focus == ItemEditFocus::Note {
            "Note (> editable)"
        } else {
            "Note (editable)"
        };
        let note_cursor_line =
            note_cursor_line_col(&self.item_edit_note, self.clamped_item_edit_note_cursor()).0;
        let note_scroll = list_scroll_for_selected_line(regions.note, Some(note_cursor_line));
        frame.render_widget(
            Paragraph::new(note_lines)
                .scroll((note_scroll, 0))
                .block(
                    Block::default()
                        .title(note_title)
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(note_border_color)),
                )
                .wrap(Wrap { trim: false }),
            regions.note,
        );
        frame.render_widget(
            Paragraph::new(format!(
                "  {}  {}  {}",
                categories_button, save_button, cancel_button
            )),
            regions.buttons,
        );
        frame.render_widget(
            Paragraph::new(
                "Tab/Shift+Tab navigate  Enter activate  Up/Down note  Esc cancel  F3 categories",
            ),
            regions.help,
        );
    }

    fn render_view_picker(&self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        frame.render_widget(Clear, area);

        let lines: Vec<Line<'_>> = if self.views.is_empty() {
            vec![Line::from("(no views configured)")]
        } else {
            self.views
                .iter()
                .enumerate()
                .map(|(index, view)| {
                    let is_selected = index == self.picker_index;
                    let marker = if is_selected { "> " } else { "  " };
                    let text = format!("{marker}{}", view.name);
                    if is_selected {
                        Line::styled(text, selected_row_style())
                    } else {
                        Line::from(text)
                    }
                })
                .collect()
        };
        let scroll = list_scroll_for_selected_line(
            area,
            if self.views.is_empty() {
                None
            } else {
                Some(self.picker_index)
            },
        );

        frame.render_widget(
            Paragraph::new(lines).scroll((scroll, 0)).block(
                Block::default()
                    .title("View Palette")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Magenta)),
            ),
            area,
        );
    }

    fn render_view_category_picker(&self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        frame.render_widget(Clear, area);

        let mut lines = vec![Line::from(
            "Choose criteria for new view (+ include, - exclude, Enter create)",
        )];
        if self.category_rows.is_empty() {
            lines.push(Line::from("(no categories available)"));
        } else {
            for (index, row) in self.category_rows.iter().enumerate() {
                let marker = if index == self.view_category_index {
                    "> "
                } else {
                    "  "
                };
                let mut flags = Vec::new();
                if row.is_reserved {
                    flags.push("reserved");
                }
                if row.is_exclusive {
                    flags.push("exclusive");
                }
                let suffix = if flags.is_empty() {
                    String::new()
                } else {
                    format!(" [{}]", flags.join(","))
                };
                let check = if self.view_create_include_selection.contains(&row.id) {
                    "[+]"
                } else if self.view_create_exclude_selection.contains(&row.id) {
                    "[-]"
                } else {
                    "[ ]"
                };
                let text = format!(
                    "{marker}{check} {}{}{}",
                    "  ".repeat(row.depth),
                    row.name,
                    suffix
                );
                if index == self.view_category_index {
                    lines.push(Line::styled(text, selected_row_style()));
                } else {
                    lines.push(Line::from(text));
                }
            }
        }

        let title = match self.mode {
            Mode::ViewCreateCategoryPicker => "Create View Criteria",
            _ => "View Criteria",
        };
        let scroll = list_scroll_for_selected_line(
            area,
            if self.category_rows.is_empty() {
                None
            } else {
                Some(1 + self.view_category_index)
            },
        );
        frame.render_widget(
            Paragraph::new(lines).scroll((scroll, 0)).block(
                Block::default()
                    .title(title)
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Magenta)),
            ),
            area,
        );
    }

    fn render_view_editor(&self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        frame.render_widget(Clear, area);
        let Some(editor) = &self.view_editor else {
            return;
        };
        let mut lines = vec![
            Line::from(format!("Editing view: {}", editor.base_view_name)),
            Line::from(format!("Preview matches: {}", editor.preview_count)),
            Line::from(""),
        ];
        let actions = [
            format!(
                "Include categories ({})",
                editor.draft.criteria.include.len()
            ),
            format!(
                "Exclude categories ({})",
                editor.draft.criteria.exclude.len()
            ),
            format!(
                "Virtual include buckets ({})",
                editor.draft.criteria.virtual_include.len()
            ),
            format!(
                "Virtual exclude buckets ({})",
                editor.draft.criteria.virtual_exclude.len()
            ),
            format!("Sections ({})", editor.draft.sections.len()),
            format!(
                "Unmatched settings (enabled={} label={})",
                editor.draft.show_unmatched, editor.draft.unmatched_label
            ),
        ];
        for (index, action) in actions.into_iter().enumerate() {
            let marker = if index == editor.action_index {
                "> "
            } else {
                "  "
            };
            let text = format!("{marker}{action}");
            if index == editor.action_index {
                lines.push(Line::styled(text, selected_row_style()));
            } else {
                lines.push(Line::from(text));
            }
        }
        lines.extend([
            Line::from(""),
            Line::from("Use j/k then o/right to open selected editor."),
            Line::from("Quick keys: + include  - exclude  ] v-include  [ v-exclude"),
            Line::from("            s sections  u unmatched  Enter save  Esc cancel"),
        ]);
        if editor.draft.sections.is_empty() {
            lines.push(Line::from("No sections configured yet."));
        }

        let scroll = list_scroll_for_selected_line(area, Some(3 + editor.action_index));
        frame.render_widget(
            Paragraph::new(lines)
                .scroll((scroll, 0))
                .block(
                    Block::default()
                        .title("View Editor")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Magenta)),
                )
                .wrap(Wrap { trim: false }),
            area,
        );
    }

    fn render_view_editor_category_picker(&self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        frame.render_widget(Clear, area);
        let Some(editor) = &self.view_editor else {
            return;
        };
        let Some(target) = self.view_editor_category_target else {
            return;
        };
        let mut lines = vec![Line::from(format!(
            "Target: {}",
            category_target_label(target)
        ))];
        for (index, row) in self.category_rows.iter().enumerate() {
            let marker = if index == editor.category_index {
                "> "
            } else {
                "  "
            };
            let selected =
                category_target_contains(&editor.draft, editor.section_index, target, row.id);
            let check = if selected { "[x]" } else { "[ ]" };
            let text = format!("{marker}{check} {}{}", "  ".repeat(row.depth), row.name);
            if index == editor.category_index {
                lines.push(Line::styled(text, selected_row_style()));
            } else {
                lines.push(Line::from(text));
            }
        }
        let scroll = list_scroll_for_selected_line(
            area,
            if self.category_rows.is_empty() {
                None
            } else {
                Some(1 + editor.category_index)
            },
        );
        frame.render_widget(
            Paragraph::new(lines)
                .scroll((scroll, 0))
                .block(
                    Block::default()
                        .title("Category Picker")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Magenta)),
                )
                .wrap(Wrap { trim: false }),
            area,
        );
    }

    fn render_view_manager_category_picker(&self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        frame.render_widget(Clear, area);
        let lines: Vec<Line<'_>> = if self.category_rows.is_empty() {
            vec![Line::from("(no categories)")]
        } else {
            self.category_rows
                .iter()
                .enumerate()
                .map(|(index, row)| {
                    let marker = if index == self.view_category_index {
                        "> "
                    } else {
                        "  "
                    };
                    let selected_flag = self
                        .view_manager_category_row_index
                        .and_then(|row_index| self.view_manager_rows.get(row_index))
                        .map(|criteria_row| criteria_row.category_id == row.id)
                        .unwrap_or(false);
                    let check = if selected_flag { "[x]" } else { "[ ]" };
                    let reserved = if row.is_reserved { " [reserved]" } else { "" };
                    let text = format!(
                        "{marker}{}{} {}{}",
                        "  ".repeat(row.depth),
                        check,
                        row.name,
                        reserved
                    );
                    if index == self.view_category_index {
                        Line::styled(text, selected_row_style())
                    } else {
                        Line::from(text)
                    }
                })
                .collect()
        };
        let scroll = list_scroll_for_selected_line(
            area,
            if self.category_rows.is_empty() {
                None
            } else {
                Some(self.view_category_index)
            },
        );

        frame.render_widget(
            Paragraph::new(lines).scroll((scroll, 0)).block(
                Block::default()
                    .title("View Manager Category Picker")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Magenta)),
            ),
            area,
        );
    }

    fn render_view_editor_bucket_picker(&self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        frame.render_widget(Clear, area);
        let Some(editor) = &self.view_editor else {
            return;
        };
        let Some(target) = self.view_editor_bucket_target else {
            return;
        };
        let mut lines = vec![Line::from(format!(
            "Target: {}",
            bucket_target_label(target)
        ))];
        for (index, bucket) in when_bucket_options().iter().enumerate() {
            let marker = if index == editor.bucket_index {
                "> "
            } else {
                "  "
            };
            let selected =
                bucket_target_contains(&editor.draft, editor.section_index, target, *bucket);
            let check = if selected { "[x]" } else { "[ ]" };
            let text = format!("{marker}{check} {}", when_bucket_label(*bucket));
            if index == editor.bucket_index {
                lines.push(Line::styled(text, selected_row_style()));
            } else {
                lines.push(Line::from(text));
            }
        }
        let scroll = list_scroll_for_selected_line(area, Some(1 + editor.bucket_index));
        frame.render_widget(
            Paragraph::new(lines)
                .scroll((scroll, 0))
                .block(
                    Block::default()
                        .title("Bucket Picker")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Magenta)),
                )
                .wrap(Wrap { trim: false }),
            area,
        );
    }

    fn render_view_section_editor(&self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        frame.render_widget(Clear, area);
        let Some(editor) = &self.view_editor else {
            return;
        };
        let mut lines = vec![Line::from("Sections in current view draft:")];
        if editor.draft.sections.is_empty() {
            lines.push(Line::from("(no sections)"));
        } else {
            for (index, section) in editor.draft.sections.iter().enumerate() {
                let marker = if index == editor.section_index {
                    "> "
                } else {
                    "  "
                };
                let text = format!(
                    "{marker}{} (include={}, exclude={}, v+={}, v-={}, show_children={})",
                    section.title,
                    section.criteria.include.len(),
                    section.criteria.exclude.len(),
                    section.criteria.virtual_include.len(),
                    section.criteria.virtual_exclude.len(),
                    section.show_children
                );
                if index == editor.section_index {
                    lines.push(Line::styled(text, selected_row_style()));
                } else {
                    lines.push(Line::from(text));
                }
            }
        }
        lines.push(Line::from(""));
        lines.push(Line::from(
            "N add  x remove  [/] reorder  Enter edit  Esc back",
        ));

        let scroll = list_scroll_for_selected_line(
            area,
            if editor.draft.sections.is_empty() {
                None
            } else {
                Some(1 + editor.section_index)
            },
        );
        frame.render_widget(
            Paragraph::new(lines)
                .scroll((scroll, 0))
                .block(
                    Block::default()
                        .title("Section Editor")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Magenta)),
                )
                .wrap(Wrap { trim: false }),
            area,
        );
    }

    fn render_view_section_detail(&self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        frame.render_widget(Clear, area);
        let Some(editor) = &self.view_editor else {
            return;
        };
        let Some(section) = editor.draft.sections.get(editor.section_index) else {
            return;
        };

        let lines = vec![
            Line::from(format!("Section: {}", section.title)),
            Line::from(format!(
                "criteria include={} exclude={} v_include={} v_exclude={}",
                section.criteria.include.len(),
                section.criteria.exclude.len(),
                section.criteria.virtual_include.len(),
                section.criteria.virtual_exclude.len()
            )),
            Line::from(format!(
                "on_insert_assign={} on_remove_unassign={}",
                section.on_insert_assign.len(),
                section.on_remove_unassign.len()
            )),
            Line::from(format!("show_children={}", section.show_children)),
            Line::from(""),
            Line::from("t title  + include  - exclude  ] v-include  [ v-exclude"),
            Line::from("a on-insert  r on-remove  h toggle show_children  Esc back"),
        ];

        frame.render_widget(
            Paragraph::new(lines)
                .block(
                    Block::default()
                        .title("Section Detail")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Magenta)),
                )
                .wrap(Wrap { trim: false }),
            area,
        );
    }

    fn render_view_unmatched_settings(&self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        frame.render_widget(Clear, area);
        let Some(editor) = &self.view_editor else {
            return;
        };
        let lines = vec![
            Line::from(format!("show_unmatched: {}", editor.draft.show_unmatched)),
            Line::from(format!("unmatched_label: {}", editor.draft.unmatched_label)),
            Line::from(""),
            Line::from("t toggle visibility  l edit label  Esc back"),
        ];
        frame.render_widget(
            Paragraph::new(lines)
                .block(
                    Block::default()
                        .title("Unmatched Settings")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Magenta)),
                )
                .wrap(Wrap { trim: false }),
            area,
        );
    }

    fn render_item_assign_picker(&self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        frame.render_widget(Clear, area);

        let mut lines = vec![Line::from(
            "Edit selected item categories (Space toggles, n or / enters category text)",
        )];
        if self.category_rows.is_empty() {
            lines.push(Line::from("(no categories)"));
        } else {
            for (index, row) in self.category_rows.iter().enumerate() {
                let marker = if index == self.item_assign_category_index {
                    "> "
                } else {
                    "  "
                };
                let mut flags = Vec::new();
                if row.is_reserved {
                    flags.push("reserved");
                }
                if row.is_exclusive {
                    flags.push("exclusive");
                }
                let suffix = if flags.is_empty() {
                    String::new()
                } else {
                    format!(" [{}]", flags.join(","))
                };
                let assigned = if self.selected_item_has_assignment(row.id) {
                    "[x]"
                } else {
                    "[ ]"
                };
                let text = format!(
                    "{marker}{assigned} {}{}{}",
                    "  ".repeat(row.depth),
                    row.name,
                    suffix
                );
                if index == self.item_assign_category_index {
                    lines.push(Line::styled(text, selected_row_style()));
                } else {
                    lines.push(Line::from(text));
                }
            }
        }

        let scroll = list_scroll_for_selected_line(
            area,
            if self.category_rows.is_empty() {
                None
            } else {
                Some(1 + self.item_assign_category_index)
            },
        );
        frame.render_widget(
            Paragraph::new(lines).scroll((scroll, 0)).block(
                Block::default()
                    .title("Assign Item")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan)),
            ),
            area,
        );
    }

    fn render_category_manager(&self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        let mut lines = vec![Line::from(
            "Categories are global. Enter opens config popup (checkboxes + note).",
        )];
        let inner_width = area.width.saturating_sub(2) as usize;
        let marker_width = 2usize;
        let excl_width = 4usize;
        let noimpl_width = 7usize;
        let todo_width = 6usize;
        let separator_width = BOARD_COLUMN_SEPARATOR.len() * 3;
        let name_width = inner_width.saturating_sub(
            marker_width + excl_width + noimpl_width + todo_width + separator_width,
        );
        lines.push(Line::from(format!(
            "{}{}{}{}{}{}{}{}",
            " ".repeat(marker_width),
            fit_board_cell("Category", name_width),
            BOARD_COLUMN_SEPARATOR,
            fit_board_cell("Excl", excl_width),
            BOARD_COLUMN_SEPARATOR,
            fit_board_cell("Match", noimpl_width),
            BOARD_COLUMN_SEPARATOR,
            fit_board_cell("Todo", todo_width),
        )));
        let mut selected_line = None;
        if self.category_rows.is_empty() {
            lines.push(Line::from("(no categories)"));
        } else {
            for (index, row) in self.category_rows.iter().enumerate() {
                let is_selected = index == self.category_index;
                let marker = if is_selected { "> " } else { "  " };
                if is_selected {
                    selected_line = Some(2 + index);
                }
                let indent = "  ".repeat(row.depth);
                let mut label = format!("{indent}{}", row.name);
                if row.is_reserved {
                    label.push_str(" [reserved]");
                }
                let excl = if row.is_exclusive { "[x]" } else { "[ ]" };
                let noimp = if row.enable_implicit_string {
                    "[x]"
                } else {
                    "[ ]"
                };
                let todo = if row.is_actionable { "[x]" } else { "[ ]" };
                let text = format!(
                    "{marker}{}{}{}{}{}{}{}",
                    fit_board_cell(&label, name_width),
                    BOARD_COLUMN_SEPARATOR,
                    fit_board_cell(excl, excl_width),
                    BOARD_COLUMN_SEPARATOR,
                    fit_board_cell(noimp, noimpl_width),
                    BOARD_COLUMN_SEPARATOR,
                    fit_board_cell(todo, todo_width),
                );
                if is_selected {
                    lines.push(Line::styled(text, selected_row_style()));
                } else {
                    lines.push(Line::from(text));
                }
            }
        }

        if self.mode == Mode::CategoryCreateInput {
            let parent = self
                .create_parent_name()
                .unwrap_or_else(|| "(top level / no parent)".to_string());
            lines.push(Line::from(""));
            lines.push(Line::from(format!("New category location: under {parent}")));
        }
        if self.mode == Mode::CategoryRenameInput {
            let target = self
                .selected_category_row()
                .map(|row| row.name.clone())
                .unwrap_or_else(|| "(none)".to_string());
            lines.push(Line::from(""));
            lines.push(Line::from(format!("Rename target: {target}")));
        }
        if self.mode == Mode::CategoryReparentPicker {
            lines.push(Line::from(""));
            lines.push(Line::from("Select new parent:"));
            if self.category_reparent_options.is_empty() {
                lines.push(Line::from("(no valid parent options)"));
            } else {
                let options_start = lines.len();
                for (index, option) in self.category_reparent_options.iter().enumerate() {
                    let marker = if index == self.category_reparent_index {
                        "> "
                    } else {
                        "  "
                    };
                    if index == self.category_reparent_index {
                        selected_line = Some(options_start + index);
                    }
                    lines.push(Line::from(format!("{marker}{}", option.label)));
                }
            }
        }
        let scroll = list_scroll_for_selected_line(area, selected_line);

        frame.render_widget(
            Paragraph::new(lines).scroll((scroll, 0)).block(
                Block::default()
                    .title("Category Manager")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Green)),
            ),
            area,
        );
    }

    fn render_category_config_editor(&self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        frame.render_widget(Clear, area);
        let block = Block::default()
            .title("Category Config")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Green));
        frame.render_widget(block, area);

        let Some(editor) = &self.category_config_editor else {
            return;
        };
        let Some(regions) = category_config_popup_regions(area) else {
            return;
        };
        frame.render_widget(
            Paragraph::new(format!("Editing: {}", editor.category_name)),
            regions.heading,
        );

        let excl_text = if editor.is_exclusive {
            "[x] Exclusive Children"
        } else {
            "[ ] Exclusive Children"
        };
        let noimp_text = if editor.enable_implicit_string {
            "[x] Match category name"
        } else {
            "[ ] Match category name"
        };
        let actionable_text = if editor.is_actionable {
            "[x] Actionable"
        } else {
            "[ ] Actionable"
        };
        let excl_style = if editor.focus == CategoryConfigFocus::Exclusive {
            focused_cell_style()
        } else {
            Style::default()
        };
        let noimp_style = if editor.focus == CategoryConfigFocus::NoImplicit {
            focused_cell_style()
        } else {
            Style::default()
        };
        let actionable_style = if editor.focus == CategoryConfigFocus::Actionable {
            focused_cell_style()
        } else {
            Style::default()
        };
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(format!(" {} ", excl_text), excl_style),
                Span::raw("  "),
                Span::styled(format!(" {} ", noimp_text), noimp_style),
                Span::raw("  "),
                Span::styled(format!(" {} ", actionable_text), actionable_style),
            ])),
            regions.toggles,
        );

        let note_lines: Vec<Line<'_>> = if editor.note.is_empty() {
            vec![Line::from("")]
        } else {
            editor.note.lines().map(Line::from).collect()
        };
        let note_border_color = if editor.focus == CategoryConfigFocus::Note {
            Color::Cyan
        } else {
            Color::Blue
        };
        let note_title = if editor.focus == CategoryConfigFocus::Note {
            "Note (> editable)"
        } else {
            "Note (editable)"
        };
        let note_cursor = self.category_config_note_cursor().unwrap_or(0);
        let note_cursor_line = note_cursor_line_col(&editor.note, note_cursor).0;
        let note_scroll = list_scroll_for_selected_line(regions.note, Some(note_cursor_line));
        frame.render_widget(
            Paragraph::new(note_lines)
                .scroll((note_scroll, 0))
                .block(
                    Block::default()
                        .title(note_title)
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(note_border_color)),
                )
                .wrap(Wrap { trim: false }),
            regions.note,
        );

        let save_button = if editor.focus == CategoryConfigFocus::SaveButton {
            "[> Save <]"
        } else {
            "[Save]"
        };
        let cancel_button = if editor.focus == CategoryConfigFocus::CancelButton {
            "[> Cancel <]"
        } else {
            "[Cancel]"
        };
        frame.render_widget(
            Paragraph::new(format!("  {}  {}", save_button, cancel_button)),
            regions.buttons,
        );
        frame.render_widget(
            Paragraph::new(
                "Tab focus  h/l checkbox focus  Space toggle  Enter saves (except note)  e/i/a quick toggle  Esc cancel",
            ),
            regions.help,
        );
    }

    fn move_slot_cursor(&mut self, delta: i32) {
        if self.slots.is_empty() {
            return;
        }
        self.slot_index = next_index(self.slot_index, self.slots.len(), delta);
        self.item_index = self.item_index.min(
            self.current_slot()
                .map(|slot| slot.items.len().saturating_sub(1))
                .unwrap_or(0),
        );
    }

    fn move_item_cursor(&mut self, delta: i32) {
        let Some(slot) = self.current_slot() else {
            return;
        };
        if slot.items.is_empty() {
            self.item_index = 0;
            return;
        }
        self.item_index = next_index(self.item_index, slot.items.len(), delta);
    }

    fn move_category_cursor(&mut self, delta: i32) {
        if self.category_rows.is_empty() {
            self.category_index = 0;
            return;
        }
        self.category_index = next_index(self.category_index, self.category_rows.len(), delta);
    }

    fn move_selected_item_between_slots(
        &mut self,
        delta: i32,
        agenda: &Agenda<'_>,
    ) -> Result<(), String> {
        if self.slots.len() < 2 {
            return Ok(());
        }
        let Some(item_id) = self.selected_item_id() else {
            return Ok(());
        };

        let from_index = self.slot_index;
        let to_index = next_index(self.slot_index, self.slots.len(), delta);
        if from_index == to_index {
            return Ok(());
        }

        let from_context = self
            .slots
            .get(from_index)
            .map(|slot| slot.context.clone())
            .ok_or("Invalid source slot".to_string())?;
        let to_context = self
            .slots
            .get(to_index)
            .map(|slot| slot.context.clone())
            .ok_or("Invalid target slot".to_string())?;
        let view = self
            .current_view()
            .cloned()
            .ok_or("No active view".to_string())?;

        self.remove_from_context(agenda, item_id, &view, &from_context)?;
        self.insert_into_context(agenda, item_id, &view, &to_context)?;

        self.slot_index = to_index;
        self.item_index = 0;
        self.refresh(agenda.store())?;
        self.status = "Moved item to new section".to_string();
        Ok(())
    }

    fn create_item_in_current_context(
        &mut self,
        agenda: &Agenda<'_>,
        text: String,
    ) -> Result<Option<NaiveDateTime>, String> {
        let item = Item::new(text);
        let reference_date = Local::now().date_naive();
        agenda
            .create_item_with_reference_date(&item, reference_date)
            .map_err(|e| e.to_string())?;

        if let Some(view) = self.current_view().cloned() {
            if let Some(context) = self.current_slot().map(|slot| slot.context.clone()) {
                self.insert_into_context(agenda, item.id, &view, &context)?;
            }
        }

        let created = agenda
            .store()
            .get_item(item.id)
            .map_err(|e| e.to_string())?;
        self.refresh(agenda.store())?;
        Ok(created.when_date)
    }

    fn remove_from_context(
        &self,
        agenda: &Agenda<'_>,
        item_id: ItemId,
        view: &View,
        context: &SlotContext,
    ) -> Result<(), String> {
        match context {
            SlotContext::Section { section_index } => {
                let section = view
                    .sections
                    .get(*section_index)
                    .ok_or("Section not found".to_string())?;
                agenda
                    .remove_item_from_section(item_id, section)
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            }
            SlotContext::GeneratedSection {
                on_insert_assign: _,
                on_remove_unassign,
            } => {
                let temp = generated_section(on_remove_unassign.clone(), HashSet::new());
                agenda
                    .remove_item_from_section(item_id, &temp)
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            }
            SlotContext::Unmatched => agenda
                .remove_item_from_unmatched(item_id, view)
                .map(|_| ())
                .map_err(|e| e.to_string()),
        }
    }

    fn insert_into_context(
        &self,
        agenda: &Agenda<'_>,
        item_id: ItemId,
        view: &View,
        context: &SlotContext,
    ) -> Result<(), String> {
        match context {
            SlotContext::Section { section_index } => {
                let section = view
                    .sections
                    .get(*section_index)
                    .ok_or("Section not found".to_string())?;
                agenda
                    .insert_item_in_section(item_id, view, section)
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            }
            SlotContext::GeneratedSection {
                on_insert_assign,
                on_remove_unassign,
            } => {
                let temp = generated_section(on_remove_unassign.clone(), on_insert_assign.clone());
                agenda
                    .insert_item_in_section(item_id, view, &temp)
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            }
            SlotContext::Unmatched => agenda
                .insert_item_in_unmatched(item_id, view)
                .map(|_| ())
                .map_err(|e| e.to_string()),
        }
    }

    fn current_slot(&self) -> Option<&Slot> {
        self.slots.get(self.slot_index)
    }

    fn selected_item(&self) -> Option<&Item> {
        self.current_slot()
            .and_then(|slot| slot.items.get(self.item_index))
    }

    fn selected_item_has_assignment(&self, category_id: CategoryId) -> bool {
        self.selected_item()
            .map(|item| item.assignments.contains_key(&category_id))
            .unwrap_or(false)
    }

    fn selected_item_has_actionable_assignment(&self) -> bool {
        let Some(item) = self.selected_item() else {
            return false;
        };
        item.assignments.keys().any(|category_id| {
            self.categories
                .iter()
                .find(|category| category.id == *category_id)
                .map(|category| category.is_actionable)
                .unwrap_or(false)
        })
    }

    fn inspect_assignment_rows_for_item(&self, item: &Item) -> Vec<InspectAssignmentRow> {
        let category_names = category_name_map(&self.categories);
        let mut rows: Vec<InspectAssignmentRow> = item
            .assignments
            .iter()
            .map(|(category_id, assignment)| InspectAssignmentRow {
                category_id: *category_id,
                category_name: category_names
                    .get(category_id)
                    .cloned()
                    .unwrap_or_else(|| category_id.to_string()),
                source_label: format!("{:?}", assignment.source),
                origin_label: assignment.origin.clone().unwrap_or_else(|| "-".to_string()),
            })
            .collect();
        rows.sort_by_key(|row| row.category_name.to_ascii_lowercase());
        rows
    }

    fn selected_item_id(&self) -> Option<ItemId> {
        self.selected_item().map(|item| item.id)
    }

    fn current_view(&self) -> Option<&View> {
        self.views.get(self.view_index)
    }

    fn selected_category_row(&self) -> Option<&CategoryListRow> {
        self.category_rows.get(self.category_index)
    }

    fn selected_category_id(&self) -> Option<CategoryId> {
        self.selected_category_row().map(|row| row.id)
    }

    fn create_parent_name(&self) -> Option<String> {
        let parent_id = self.category_create_parent?;
        self.category_rows
            .iter()
            .find(|row| row.id == parent_id)
            .map(|row| row.name.clone())
    }

    fn selected_category_parent_index(&self, category_id: CategoryId) -> Option<usize> {
        let parent_id = self
            .categories
            .iter()
            .find(|category| category.id == category_id)
            .and_then(|category| category.parent);
        self.category_reparent_options
            .iter()
            .position(|option| option.parent_id == parent_id)
    }

    fn set_category_selection_by_id(&mut self, category_id: CategoryId) {
        if let Some(index) = self
            .category_rows
            .iter()
            .position(|row| row.id == category_id)
        {
            self.category_index = index;
        }
    }

    fn set_item_selection_by_id(&mut self, item_id: ItemId) {
        for (slot_index, slot) in self.slots.iter().enumerate() {
            if let Some(item_index) = slot.items.iter().position(|item| item.id == item_id) {
                self.slot_index = slot_index;
                self.item_index = item_index;
                return;
            }
        }
    }

    fn set_view_selection_by_name(&mut self, view_name: &str) {
        if let Some(index) = self
            .views
            .iter()
            .position(|view| view.name.eq_ignore_ascii_case(view_name))
        {
            self.view_index = index;
            self.picker_index = index;
        }
    }

    fn cycle_view(&mut self, delta: i32, agenda: &Agenda<'_>) -> Result<(), String> {
        if self.views.is_empty() {
            self.status = "No views available".to_string();
            return Ok(());
        }
        self.view_index = next_index(self.view_index, self.views.len(), delta);
        self.picker_index = self.view_index;
        self.slot_index = 0;
        self.item_index = 0;
        self.refresh(agenda.store())?;
        let view_name = self
            .current_view()
            .map(|view| view.name.clone())
            .unwrap_or_else(|| "(none)".to_string());
        self.status = format!("Switched to view: {view_name} (press v then e to edit view)");
        Ok(())
    }

    fn jump_to_all_items_view(&mut self, agenda: &Agenda<'_>) -> Result<(), String> {
        let Some(index) = self
            .views
            .iter()
            .position(|view| view.name.eq_ignore_ascii_case("All Items"))
        else {
            self.status = "All Items view not found".to_string();
            return Ok(());
        };
        self.view_index = index;
        self.picker_index = index;
        self.slot_index = 0;
        self.item_index = 0;
        self.refresh(agenda.store())?;
        self.status = "Jumped to view: All Items".to_string();
        Ok(())
    }
}

fn generated_section(
    on_remove_unassign: HashSet<CategoryId>,
    on_insert_assign: HashSet<CategoryId>,
) -> Section {
    Section {
        title: "generated".to_string(),
        criteria: Query::default(),
        on_insert_assign,
        on_remove_unassign,
        show_children: false,
    }
}

fn next_index(current: usize, len: usize, delta: i32) -> usize {
    if len == 0 {
        return 0;
    }
    if delta > 0 {
        (current + delta as usize) % len
    } else {
        let amount = (-delta) as usize % len;
        (current + len - amount) % len
    }
}

fn when_bucket_options() -> &'static [WhenBucket] {
    &[
        WhenBucket::Overdue,
        WhenBucket::Today,
        WhenBucket::Tomorrow,
        WhenBucket::ThisWeek,
        WhenBucket::NextWeek,
        WhenBucket::ThisMonth,
        WhenBucket::Future,
        WhenBucket::NoDate,
    ]
}

fn when_bucket_label(bucket: WhenBucket) -> &'static str {
    match bucket {
        WhenBucket::Overdue => "Overdue",
        WhenBucket::Today => "Today",
        WhenBucket::Tomorrow => "Tomorrow",
        WhenBucket::ThisWeek => "ThisWeek",
        WhenBucket::NextWeek => "NextWeek",
        WhenBucket::ThisMonth => "ThisMonth",
        WhenBucket::Future => "Future",
        WhenBucket::NoDate => "NoDate",
    }
}

fn category_target_is_section(target: CategoryEditTarget) -> bool {
    matches!(
        target,
        CategoryEditTarget::SectionCriteriaInclude
            | CategoryEditTarget::SectionCriteriaExclude
            | CategoryEditTarget::SectionOnInsertAssign
            | CategoryEditTarget::SectionOnRemoveUnassign
    )
}

fn bucket_target_is_section(target: BucketEditTarget) -> bool {
    matches!(
        target,
        BucketEditTarget::SectionVirtualInclude | BucketEditTarget::SectionVirtualExclude
    )
}

fn category_target_label(target: CategoryEditTarget) -> &'static str {
    match target {
        CategoryEditTarget::ViewInclude => "View include categories",
        CategoryEditTarget::ViewExclude => "View exclude categories",
        CategoryEditTarget::SectionCriteriaInclude => "Section include criteria",
        CategoryEditTarget::SectionCriteriaExclude => "Section exclude criteria",
        CategoryEditTarget::SectionOnInsertAssign => "Section on-insert assign",
        CategoryEditTarget::SectionOnRemoveUnassign => "Section on-remove unassign",
    }
}

fn bucket_target_label(target: BucketEditTarget) -> &'static str {
    match target {
        BucketEditTarget::ViewVirtualInclude => "View virtual include buckets",
        BucketEditTarget::ViewVirtualExclude => "View virtual exclude buckets",
        BucketEditTarget::SectionVirtualInclude => "Section virtual include buckets",
        BucketEditTarget::SectionVirtualExclude => "Section virtual exclude buckets",
    }
}

fn category_target_set_mut<'a>(
    view: &'a mut View,
    section_index: usize,
    target: CategoryEditTarget,
) -> Option<&'a mut HashSet<CategoryId>> {
    match target {
        CategoryEditTarget::ViewInclude => Some(&mut view.criteria.include),
        CategoryEditTarget::ViewExclude => Some(&mut view.criteria.exclude),
        CategoryEditTarget::SectionCriteriaInclude => view
            .sections
            .get_mut(section_index)
            .map(|section| &mut section.criteria.include),
        CategoryEditTarget::SectionCriteriaExclude => view
            .sections
            .get_mut(section_index)
            .map(|section| &mut section.criteria.exclude),
        CategoryEditTarget::SectionOnInsertAssign => view
            .sections
            .get_mut(section_index)
            .map(|section| &mut section.on_insert_assign),
        CategoryEditTarget::SectionOnRemoveUnassign => view
            .sections
            .get_mut(section_index)
            .map(|section| &mut section.on_remove_unassign),
    }
}

fn bucket_target_set_mut<'a>(
    view: &'a mut View,
    section_index: usize,
    target: BucketEditTarget,
) -> Option<&'a mut HashSet<WhenBucket>> {
    match target {
        BucketEditTarget::ViewVirtualInclude => Some(&mut view.criteria.virtual_include),
        BucketEditTarget::ViewVirtualExclude => Some(&mut view.criteria.virtual_exclude),
        BucketEditTarget::SectionVirtualInclude => view
            .sections
            .get_mut(section_index)
            .map(|section| &mut section.criteria.virtual_include),
        BucketEditTarget::SectionVirtualExclude => view
            .sections
            .get_mut(section_index)
            .map(|section| &mut section.criteria.virtual_exclude),
    }
}

fn category_target_contains(
    view: &View,
    section_index: usize,
    target: CategoryEditTarget,
    category_id: CategoryId,
) -> bool {
    match target {
        CategoryEditTarget::ViewInclude => view.criteria.include.contains(&category_id),
        CategoryEditTarget::ViewExclude => view.criteria.exclude.contains(&category_id),
        CategoryEditTarget::SectionCriteriaInclude => view
            .sections
            .get(section_index)
            .map(|section| section.criteria.include.contains(&category_id))
            .unwrap_or(false),
        CategoryEditTarget::SectionCriteriaExclude => view
            .sections
            .get(section_index)
            .map(|section| section.criteria.exclude.contains(&category_id))
            .unwrap_or(false),
        CategoryEditTarget::SectionOnInsertAssign => view
            .sections
            .get(section_index)
            .map(|section| section.on_insert_assign.contains(&category_id))
            .unwrap_or(false),
        CategoryEditTarget::SectionOnRemoveUnassign => view
            .sections
            .get(section_index)
            .map(|section| section.on_remove_unassign.contains(&category_id))
            .unwrap_or(false),
    }
}

fn bucket_target_contains(
    view: &View,
    section_index: usize,
    target: BucketEditTarget,
    bucket: WhenBucket,
) -> bool {
    match target {
        BucketEditTarget::ViewVirtualInclude => view.criteria.virtual_include.contains(&bucket),
        BucketEditTarget::ViewVirtualExclude => view.criteria.virtual_exclude.contains(&bucket),
        BucketEditTarget::SectionVirtualInclude => view
            .sections
            .get(section_index)
            .map(|section| section.criteria.virtual_include.contains(&bucket))
            .unwrap_or(false),
        BucketEditTarget::SectionVirtualExclude => view
            .sections
            .get(section_index)
            .map(|section| section.criteria.virtual_exclude.contains(&bucket))
            .unwrap_or(false),
    }
}

fn list_scroll_for_selected_line(area: Rect, selected_line: Option<usize>) -> u16 {
    let Some(selected_line) = selected_line else {
        return 0;
    };
    let viewport_rows = area.height.saturating_sub(2) as usize;
    if viewport_rows == 0 {
        return 0;
    }
    selected_line
        .saturating_add(1)
        .saturating_sub(viewport_rows)
        .min(u16::MAX as usize) as u16
}

fn should_render_unmatched_lane(unmatched_items: &[Item]) -> bool {
    !unmatched_items.is_empty()
}

fn item_text_matches(item: &Item, needle_lower_ascii: &str) -> bool {
    if item.text.to_ascii_lowercase().contains(needle_lower_ascii) {
        return true;
    }

    item.note
        .as_ref()
        .map(|note| note.to_ascii_lowercase().contains(needle_lower_ascii))
        .unwrap_or(false)
}

fn category_name_map(categories: &[Category]) -> HashMap<CategoryId, String> {
    categories
        .iter()
        .map(|category| (category.id, category.name.clone()))
        .collect()
}

fn item_assignment_labels(
    item: &Item,
    category_names: &HashMap<CategoryId, String>,
) -> Vec<String> {
    let mut labels: Vec<String> = item
        .assignments
        .keys()
        .map(|category_id| {
            category_names
                .get(category_id)
                .cloned()
                .unwrap_or_else(|| category_id.to_string())
        })
        .collect();
    labels.sort_by_key(|name| name.to_ascii_lowercase());
    labels
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BoardColumnWidths {
    marker: usize,
    when: usize,
    item: usize,
    categories: usize,
}

const BOARD_ROW_MARKER_WIDTH: usize = 2;
const BOARD_COLUMN_SEPARATOR: &str = " | ";
const BOARD_WHEN_TARGET_WIDTH: usize = 19;
const BOARD_WHEN_MIN_WIDTH: usize = 10;
const BOARD_ITEM_MIN_WIDTH: usize = 12;
const BOARD_CATEGORY_TARGET_WIDTH: usize = 34;
const BOARD_CATEGORY_MIN_WIDTH: usize = 14;
const BOARD_TRUNCATION_SUFFIX: &str = "...";

fn board_column_widths(slot_width: u16) -> BoardColumnWidths {
    let total = slot_width as usize;
    let marker = BOARD_ROW_MARKER_WIDTH.min(total);
    let separator_total = BOARD_COLUMN_SEPARATOR.len() * 2;
    let available = total.saturating_sub(marker + separator_total);

    if available == 0 {
        return BoardColumnWidths {
            marker,
            when: 0,
            item: 0,
            categories: 0,
        };
    }

    let mut when = BOARD_WHEN_TARGET_WIDTH.min(available);
    let mut categories = BOARD_CATEGORY_TARGET_WIDTH.min(available.saturating_sub(when));
    let mut item = available.saturating_sub(when + categories);

    let min_item = BOARD_ITEM_MIN_WIDTH.min(available);
    if item < min_item {
        let needed = min_item - item;
        let min_categories = BOARD_CATEGORY_MIN_WIDTH.min(categories);
        let category_shift = needed.min(categories.saturating_sub(min_categories));
        categories -= category_shift;
        item += category_shift;

        let needed = min_item.saturating_sub(item);
        let min_when = BOARD_WHEN_MIN_WIDTH.min(when);
        let when_shift = needed.min(when.saturating_sub(min_when));
        when -= when_shift;
        item += when_shift;
    }

    if item == 0 && available > 0 {
        if categories > 0 {
            categories -= 1;
            item += 1;
        } else if when > 0 {
            when -= 1;
            item += 1;
        }
    }

    let used = when + item + categories;
    if used < available {
        item += available - used;
    }

    BoardColumnWidths {
        marker,
        when,
        item,
        categories,
    }
}

fn fit_board_cell(text: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let count = text.chars().count();
    if count <= width {
        return format!("{text:<width$}");
    }
    if width <= BOARD_TRUNCATION_SUFFIX.len() {
        return ".".repeat(width);
    }
    let keep = width - BOARD_TRUNCATION_SUFFIX.len();
    let prefix: String = text.chars().take(keep).collect();
    format!("{prefix}{BOARD_TRUNCATION_SUFFIX}")
}

fn board_row_marker(is_selected: bool, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    if is_selected {
        let mut marker = ">".to_string();
        marker.push_str(&" ".repeat(width.saturating_sub(1)));
        marker
    } else {
        " ".repeat(width)
    }
}

fn board_annotation_header(widths: BoardColumnWidths) -> String {
    format!(
        "{}{}{}{}{}{}",
        " ".repeat(widths.marker),
        fit_board_cell("When", widths.when),
        BOARD_COLUMN_SEPARATOR,
        fit_board_cell("Item", widths.item),
        BOARD_COLUMN_SEPARATOR,
        fit_board_cell("All Categories", widths.categories),
    )
}

fn board_item_row(
    is_selected: bool,
    when: &str,
    item: &str,
    categories: &str,
    widths: BoardColumnWidths,
) -> String {
    format!(
        "{}{}{}{}{}{}",
        board_row_marker(is_selected, widths.marker),
        fit_board_cell(when, widths.when),
        BOARD_COLUMN_SEPARATOR,
        fit_board_cell(item, widths.item),
        BOARD_COLUMN_SEPARATOR,
        fit_board_cell(categories, widths.categories),
    )
}

fn selected_row_style() -> Style {
    Style::default().fg(Color::Black).bg(Color::Cyan)
}

fn focused_cell_style() -> Style {
    Style::default()
        .fg(Color::Black)
        .bg(Color::Yellow)
        .add_modifier(Modifier::BOLD)
}

fn build_category_rows(categories: &[Category]) -> Vec<CategoryListRow> {
    let parent_by_id: HashMap<CategoryId, Option<CategoryId>> = categories
        .iter()
        .map(|category| (category.id, category.parent))
        .collect();

    categories
        .iter()
        .map(|category| CategoryListRow {
            id: category.id,
            name: category.name.clone(),
            depth: category_depth(category.id, &parent_by_id, categories.len()),
            is_reserved: is_reserved_category_name(&category.name),
            is_exclusive: category.is_exclusive,
            is_actionable: category.is_actionable,
            enable_implicit_string: category.enable_implicit_string,
        })
        .collect()
}

fn build_reparent_options(
    category_rows: &[CategoryListRow],
    categories: &[Category],
    selected_category_id: CategoryId,
) -> Vec<ReparentOptionRow> {
    let descendants = descendant_category_ids(categories, selected_category_id);
    let mut options = vec![ReparentOptionRow {
        parent_id: None,
        label: "(root)".to_string(),
    }];

    for row in category_rows {
        if row.id == selected_category_id {
            continue;
        }
        if descendants.contains(&row.id) {
            continue;
        }
        options.push(ReparentOptionRow {
            parent_id: Some(row.id),
            label: format!("{}{}", "  ".repeat(row.depth), row.name),
        });
    }

    options
}

fn descendant_category_ids(categories: &[Category], root_id: CategoryId) -> HashSet<CategoryId> {
    let children_by_parent: HashMap<CategoryId, Vec<CategoryId>> = categories
        .iter()
        .filter_map(|category| category.parent.map(|parent| (parent, category.id)))
        .fold(HashMap::new(), |mut acc, (parent, child)| {
            acc.entry(parent).or_default().push(child);
            acc
        });

    let mut seen = HashSet::new();
    let mut stack = vec![root_id];
    while let Some(current) = stack.pop() {
        let Some(children) = children_by_parent.get(&current) else {
            continue;
        };
        for child in children {
            if seen.insert(*child) {
                stack.push(*child);
            }
        }
    }

    seen
}

fn category_depth(
    category_id: CategoryId,
    parent_by_id: &HashMap<CategoryId, Option<CategoryId>>,
    max_depth: usize,
) -> usize {
    let mut depth = 0usize;
    let mut cursor = parent_by_id.get(&category_id).copied().flatten();

    while let Some(parent_id) = cursor {
        depth += 1;
        if depth > max_depth {
            break;
        }
        cursor = parent_by_id.get(&parent_id).copied().flatten();
    }

    depth
}

fn is_reserved_category_name(name: &str) -> bool {
    name.eq_ignore_ascii_case("When")
        || name.eq_ignore_ascii_case("Entry")
        || name.eq_ignore_ascii_case("Done")
}

fn first_non_reserved_category_index(category_rows: &[CategoryListRow]) -> usize {
    category_rows
        .iter()
        .position(|row| !row.is_reserved)
        .unwrap_or(0)
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1]);

    horizontal[1]
}

fn item_edit_popup_area(area: Rect) -> Rect {
    centered_rect(84, 70, area)
}

fn category_config_popup_area(area: Rect) -> Rect {
    centered_rect(84, 76, area)
}

struct ItemEditPopupRegions {
    heading: Rect,
    text: Rect,
    note: Rect,
    note_inner: Rect,
    buttons: Rect,
    help: Rect,
}

struct CategoryConfigPopupRegions {
    heading: Rect,
    toggles: Rect,
    note: Rect,
    note_inner: Rect,
    buttons: Rect,
    help: Rect,
}

fn item_edit_popup_regions(area: Rect) -> Option<ItemEditPopupRegions> {
    if area.width < 3 || area.height < 3 {
        return None;
    }
    let inner = Rect {
        x: area.x.saturating_add(1),
        y: area.y.saturating_add(1),
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };
    if inner.width == 0 || inner.height < 5 {
        return None;
    }
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(3),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);
    let note = chunks[2];
    let note_inner = Rect {
        x: note.x.saturating_add(1),
        y: note.y.saturating_add(1),
        width: note.width.saturating_sub(2),
        height: note.height.saturating_sub(2),
    };
    Some(ItemEditPopupRegions {
        heading: chunks[0],
        text: chunks[1],
        note,
        note_inner,
        buttons: chunks[3],
        help: chunks[4],
    })
}

fn category_config_popup_regions(area: Rect) -> Option<CategoryConfigPopupRegions> {
    if area.width < 3 || area.height < 3 {
        return None;
    }
    let inner = Rect {
        x: area.x.saturating_add(1),
        y: area.y.saturating_add(1),
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    };
    if inner.width == 0 || inner.height < 5 {
        return None;
    }
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(4),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);
    let note = chunks[2];
    let note_inner = Rect {
        x: note.x.saturating_add(1),
        y: note.y.saturating_add(1),
        width: note.width.saturating_sub(2),
        height: note.height.saturating_sub(2),
    };
    Some(CategoryConfigPopupRegions {
        heading: chunks[0],
        toggles: chunks[1],
        note,
        note_inner,
        buttons: chunks[3],
        help: chunks[4],
    })
}

fn string_byte_index(value: &str, char_index: usize) -> usize {
    if char_index == 0 {
        return 0;
    }
    value
        .char_indices()
        .nth(char_index)
        .map(|(byte_index, _)| byte_index)
        .unwrap_or(value.len())
}

fn note_cursor_line_col(note: &str, cursor_chars: usize) -> (usize, usize) {
    let mut line = 0usize;
    let mut col = 0usize;
    for c in note.chars().take(cursor_chars) {
        if c == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    (line, col)
}

fn note_line_start_chars(note: &str) -> Vec<usize> {
    let mut starts = vec![0usize];
    let mut char_index = 0usize;
    for c in note.chars() {
        char_index += 1;
        if c == '\n' {
            starts.push(char_index);
        }
    }
    starts
}

fn add_capture_status_message(
    parsed_when: Option<NaiveDateTime>,
    unknown_hashtags: &[String],
) -> String {
    let warning = if unknown_hashtags.is_empty() {
        String::new()
    } else {
        format!(" | warning unknown_hashtags={}", unknown_hashtags.join(","))
    };
    match parsed_when {
        Some(when) => format!("Item added (parsed when: {when}{warning})"),
        None => format!("Item added{warning}"),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        add_capture_status_message, board_annotation_header, board_column_widths, board_item_row,
        bucket_target_set_mut, build_category_rows, build_reparent_options,
        category_target_set_mut, first_non_reserved_category_index, item_assignment_labels,
        item_edit_popup_area, list_scroll_for_selected_line, next_index,
        should_render_unmatched_lane, when_bucket_options, App, BucketEditTarget,
        CategoryEditTarget, CategoryListRow, Mode, ViewManagerPane,
    };
    use agenda_core::agenda::Agenda;
    use agenda_core::matcher::SubstringClassifier;
    use agenda_core::model::{
        Assignment, AssignmentSource, Category, CategoryId, Item, Query, Section, View, WhenBucket,
    };
    use agenda_core::store::Store;
    use chrono::NaiveDate;
    use crossterm::event::KeyCode;
    use ratatui::layout::Rect;

    fn row_depth_map(rows: &[super::CategoryListRow]) -> HashMap<CategoryId, usize> {
        rows.iter().map(|row| (row.id, row.depth)).collect()
    }

    #[test]
    fn add_capture_status_message_includes_parsed_datetime_when_present() {
        let when = NaiveDate::from_ymd_opt(2026, 2, 24)
            .expect("valid date")
            .and_hms_opt(15, 0, 0)
            .expect("valid time");

        assert_eq!(
            add_capture_status_message(Some(when), &[]),
            "Item added (parsed when: 2026-02-24 15:00:00)"
        );
    }

    #[test]
    fn add_capture_status_message_defaults_when_no_datetime() {
        assert_eq!(add_capture_status_message(None, &[]), "Item added");
    }

    #[test]
    fn add_capture_status_message_includes_unknown_hashtag_warning() {
        assert_eq!(
            add_capture_status_message(None, &["office".to_string(), "someday".to_string()]),
            "Item added | warning unknown_hashtags=office,someday"
        );
    }

    #[test]
    fn build_category_rows_marks_reserved_and_tracks_depth() {
        let mut work = Category::new("Work".to_string());
        let mut project = Category::new("Project Y".to_string());
        project.parent = Some(work.id);
        let mut frabulator = Category::new("Frabulator".to_string());
        frabulator.parent = Some(project.id);
        let done = Category::new("Done".to_string());

        work.enable_implicit_string = true;

        let categories = vec![
            done.clone(),
            work.clone(),
            project.clone(),
            frabulator.clone(),
        ];
        let rows = build_category_rows(&categories);
        let depth_by_id = row_depth_map(&rows);

        assert_eq!(depth_by_id.get(&work.id), Some(&0));
        assert_eq!(depth_by_id.get(&project.id), Some(&1));
        assert_eq!(depth_by_id.get(&frabulator.id), Some(&2));

        let done_row = rows
            .iter()
            .find(|row| row.id == done.id)
            .expect("done row present");
        assert!(done_row.is_reserved);
    }

    #[test]
    fn build_category_rows_handles_missing_parent_without_panic() {
        let mut orphan = Category::new("Orphan".to_string());
        orphan.parent = Some(CategoryId::new_v4());

        let rows = build_category_rows(&[orphan.clone()]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].depth, 1);
        assert!(!rows[0].is_reserved);
    }

    #[test]
    fn build_reparent_options_excludes_self_and_descendants() {
        let work = Category::new("Work".to_string());
        let mut project = Category::new("Project Y".to_string());
        project.parent = Some(work.id);
        let mut subproject = Category::new("Frabulator".to_string());
        subproject.parent = Some(project.id);
        let personal = Category::new("Personal".to_string());

        let categories = vec![
            work.clone(),
            project.clone(),
            subproject.clone(),
            personal.clone(),
        ];
        let rows = build_category_rows(&categories);

        let options = build_reparent_options(&rows, &categories, project.id);
        assert!(options.iter().any(|option| option.parent_id.is_none()));
        assert!(options
            .iter()
            .any(|option| option.parent_id == Some(work.id)));
        assert!(options
            .iter()
            .any(|option| option.parent_id == Some(personal.id)));
        assert!(!options
            .iter()
            .any(|option| option.parent_id == Some(project.id)));
        assert!(!options
            .iter()
            .any(|option| option.parent_id == Some(subproject.id)));
    }

    #[test]
    fn first_non_reserved_category_index_prefers_non_reserved_row() {
        let reserved = CategoryListRow {
            id: CategoryId::new_v4(),
            name: "Done".to_string(),
            depth: 0,
            is_reserved: true,
            is_exclusive: false,
            is_actionable: false,
            enable_implicit_string: false,
        };
        let user = CategoryListRow {
            id: CategoryId::new_v4(),
            name: "Work".to_string(),
            depth: 0,
            is_reserved: false,
            is_exclusive: false,
            is_actionable: true,
            enable_implicit_string: true,
        };

        assert_eq!(
            first_non_reserved_category_index(&[reserved.clone(), user.clone()]),
            1
        );
    }

    #[test]
    fn first_non_reserved_category_index_defaults_to_zero_when_all_reserved() {
        let done = CategoryListRow {
            id: CategoryId::new_v4(),
            name: "Done".to_string(),
            depth: 0,
            is_reserved: true,
            is_exclusive: false,
            is_actionable: false,
            enable_implicit_string: false,
        };
        let when = CategoryListRow {
            id: CategoryId::new_v4(),
            name: "When".to_string(),
            depth: 0,
            is_reserved: true,
            is_exclusive: false,
            is_actionable: false,
            enable_implicit_string: false,
        };

        assert_eq!(first_non_reserved_category_index(&[done, when]), 0);
    }

    #[test]
    fn input_cursor_position_is_set_for_text_input_modes() {
        let footer = Rect::new(10, 5, 40, 3);
        let input = "abc";
        let cases = [
            (Mode::AddInput, "Add> "),
            (Mode::NoteEditInput, "Note> "),
            (Mode::FilterInput, "Filter> "),
            (Mode::ViewCreateNameInput, "View create> "),
            (Mode::ViewRenameInput, "View rename> "),
            (Mode::ViewSectionTitleInput, "Section title> "),
            (Mode::ViewUnmatchedLabelInput, "Unmatched label> "),
            (Mode::CategoryCreateInput, "Category create> "),
            (Mode::CategoryRenameInput, "Category rename> "),
            (Mode::ItemAssignCategoryInput, "Category> "),
        ];

        for (mode, prefix) in cases {
            let app = App {
                mode,
                input: input.to_string(),
                input_cursor: input.len(),
                ..App::default()
            };
            let expected_x = footer.x + 1 + prefix.len() as u16 + input.len() as u16;
            assert_eq!(
                app.input_cursor_position(footer),
                Some((expected_x, footer.y + 1))
            );
        }
    }

    #[test]
    fn input_cursor_position_is_hidden_for_non_input_modes() {
        let footer = Rect::new(10, 5, 40, 3);
        for mode in [
            Mode::Normal,
            Mode::ConfirmDelete,
            Mode::ViewPicker,
            Mode::ViewDeleteConfirm,
            Mode::CategoryManager,
        ] {
            let app = App {
                mode,
                input: "abc".to_string(),
                ..App::default()
            };
            assert_eq!(app.input_cursor_position(footer), None);
        }
    }

    #[test]
    fn input_cursor_position_clamps_to_footer_inner_width() {
        let footer = Rect::new(0, 0, 8, 3);
        let app = App {
            mode: Mode::AddInput,
            input: "abcdefghijklmnopqrstuvwxyz".to_string(),
            input_cursor: usize::MAX,
            ..App::default()
        };

        assert_eq!(app.input_cursor_position(footer), Some((6, 1)));
    }

    #[test]
    fn input_cursor_position_tracks_edit_cursor_not_just_input_end() {
        let footer = Rect::new(0, 0, 40, 3);
        let app = App {
            mode: Mode::AddInput,
            input: "abcd".to_string(),
            input_cursor: 2,
            ..App::default()
        };

        assert_eq!(app.input_cursor_position(footer), Some((8, 1)));
    }

    #[test]
    fn item_edit_cursor_position_uses_popup_area() {
        let screen = Rect::new(0, 0, 120, 40);
        let popup = item_edit_popup_area(screen);
        let app = App {
            mode: Mode::ItemEditInput,
            input: "abcd".to_string(),
            input_cursor: 2,
            ..App::default()
        };
        assert_eq!(
            app.item_edit_cursor_position(popup),
            Some((popup.x + 1 + "  Text> ".len() as u16 + 2, popup.y + 2))
        );
    }

    #[test]
    fn item_edit_tab_switches_to_note_and_saves_note_inline() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!("agenda-tui-item-edit-note-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut item = Item::new("demo item".to_string());
        item.note = Some("old".to_string());
        store.create_item(&item).expect("create item");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.mode = Mode::Normal;
        app.set_item_selection_by_id(item.id);

        app.handle_normal_key(KeyCode::Char('e'), &agenda)
            .expect("open item edit");
        assert_eq!(app.mode, Mode::ItemEditInput);

        app.handle_item_edit_key(KeyCode::Tab, &agenda)
            .expect("switch to note");
        assert_eq!(app.item_edit_focus, super::ItemEditFocus::Note);

        for c in " updated".chars() {
            app.handle_item_edit_key(KeyCode::Char(c), &agenda)
                .expect("type note");
        }
        app.handle_item_edit_key(KeyCode::Tab, &agenda)
            .expect("focus categories button");
        app.handle_item_edit_key(KeyCode::Tab, &agenda)
            .expect("focus save button");
        app.handle_item_edit_key(KeyCode::Enter, &agenda)
            .expect("save item edit");

        let saved = store.get_item(item.id).expect("load item");
        assert_eq!(saved.note.as_deref(), Some("old updated"));

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn item_edit_enter_in_note_inserts_newline_until_save_button() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path =
            std::env::temp_dir().join(format!("agenda-tui-item-edit-multiline-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut item = Item::new("demo item".to_string());
        item.note = Some("line1".to_string());
        store.create_item(&item).expect("create item");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.mode = Mode::Normal;
        app.set_item_selection_by_id(item.id);
        app.handle_normal_key(KeyCode::Enter, &agenda)
            .expect("enter opens edit");
        app.handle_item_edit_key(KeyCode::Tab, &agenda)
            .expect("focus note");
        app.handle_item_edit_key(KeyCode::Enter, &agenda)
            .expect("enter adds newline");
        for c in "line2".chars() {
            app.handle_item_edit_key(KeyCode::Char(c), &agenda)
                .expect("type note line2");
        }
        app.handle_item_edit_key(KeyCode::Tab, &agenda)
            .expect("focus categories");
        app.handle_item_edit_key(KeyCode::Tab, &agenda)
            .expect("focus save");
        app.handle_item_edit_key(KeyCode::Enter, &agenda)
            .expect("save");

        let saved = store.get_item(item.id).expect("load item");
        assert_eq!(saved.note.as_deref(), Some("line1\nline2"));

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn list_scroll_keeps_selected_line_visible() {
        let area = Rect::new(0, 0, 50, 10);
        assert_eq!(list_scroll_for_selected_line(area, None), 0);
        assert_eq!(list_scroll_for_selected_line(area, Some(0)), 0);
        assert_eq!(list_scroll_for_selected_line(area, Some(7)), 0);
        assert_eq!(list_scroll_for_selected_line(area, Some(8)), 1);
    }

    #[test]
    fn view_picker_delete_uses_x_and_removes_selected_view() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!("agenda-tui-view-delete-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let keep = View::new("Keep Me".to_string());
        let remove = View::new("Remove Me".to_string());
        store.create_view(&keep).expect("create keep view");
        store.create_view(&remove).expect("create remove view");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.mode = Mode::ViewPicker;
        app.picker_index = app
            .views
            .iter()
            .position(|view| view.name == "Remove Me")
            .expect("remove view should exist");

        app.handle_view_picker_key(KeyCode::Char('x'), &agenda)
            .expect("open delete confirm");
        assert_eq!(app.mode, Mode::ViewDeleteConfirm);

        app.handle_view_delete_key(KeyCode::Char('y'), &agenda)
            .expect("confirm delete");
        assert_eq!(app.mode, Mode::ViewPicker);
        assert!(!store
            .list_views()
            .expect("list views")
            .iter()
            .any(|view| view.name == "Remove Me"));
        assert!(store
            .list_views()
            .expect("list views")
            .iter()
            .any(|view| view.name == "Keep Me"));

        app.mode = Mode::ViewPicker;
        app.picker_index = app
            .views
            .iter()
            .position(|view| view.name == "Keep Me")
            .expect("keep view should exist");
        app.handle_view_picker_key(KeyCode::Char('d'), &agenda)
            .expect("d key should be ignored");
        assert_eq!(app.mode, Mode::ViewPicker);
        assert!(store
            .list_views()
            .expect("list views")
            .iter()
            .any(|view| view.name == "Keep Me"));

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn view_picker_v_opens_view_manager_screen() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!("agenda-tui-view-manager-open-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);
        store
            .create_view(&View::new("Work Board".to_string()))
            .expect("create view");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.mode = Mode::ViewPicker;
        app.picker_index = 0;

        app.handle_view_picker_key(KeyCode::Char('V'), &agenda)
            .expect("open view manager shell");

        assert_eq!(app.mode, Mode::ViewManagerScreen);
        assert_eq!(app.view_manager_pane, ViewManagerPane::Views);

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn view_manager_tab_cycles_panes() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!("agenda-tui-view-manager-tabs-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.mode = Mode::ViewManagerScreen;
        app.view_manager_pane = ViewManagerPane::Views;

        app.handle_view_manager_key(KeyCode::Tab, &agenda)
            .expect("tab");
        assert_eq!(app.view_manager_pane, ViewManagerPane::Definition);

        app.handle_view_manager_key(KeyCode::Tab, &agenda)
            .expect("tab");
        assert_eq!(app.view_manager_pane, ViewManagerPane::Sections);

        app.handle_view_manager_key(KeyCode::BackTab, &agenda)
            .expect("backtab");
        assert_eq!(app.view_manager_pane, ViewManagerPane::Definition);

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn view_manager_sections_support_add_remove_and_reorder() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path =
            std::env::temp_dir().join(format!("agenda-tui-view-manager-sections-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut view = View::new("Board".to_string());
        view.sections.push(Section {
            title: "A".to_string(),
            criteria: Query::default(),
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
        });
        view.sections.push(Section {
            title: "B".to_string(),
            criteria: Query::default(),
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
        });
        store.create_view(&view).expect("create view");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.mode = Mode::ViewPicker;
        app.picker_index = app
            .views
            .iter()
            .position(|v| v.name == "Board")
            .expect("board view exists");
        app.handle_view_picker_key(KeyCode::Char('V'), &agenda)
            .expect("open manager");
        app.view_manager_pane = ViewManagerPane::Sections;

        app.handle_view_manager_key(KeyCode::Char(']'), &agenda)
            .expect("reorder down");
        let selected = app.views.get(app.picker_index).expect("selected view");
        assert_eq!(selected.sections[0].title, "B");
        assert_eq!(selected.sections[1].title, "A");

        app.handle_view_manager_key(KeyCode::Char('['), &agenda)
            .expect("reorder up");
        let selected = app.views.get(app.picker_index).expect("selected view");
        assert_eq!(selected.sections[0].title, "A");
        assert_eq!(selected.sections[1].title, "B");

        app.handle_view_manager_key(KeyCode::Char('N'), &agenda)
            .expect("add section");
        let selected = app.views.get(app.picker_index).expect("selected view");
        assert_eq!(selected.sections.len(), 3);

        app.handle_view_manager_key(KeyCode::Char('x'), &agenda)
            .expect("remove section");
        let selected = app.views.get(app.picker_index).expect("selected view");
        assert_eq!(selected.sections.len(), 2);
        assert!(app.view_manager_dirty);

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn view_manager_section_editor_returns_and_applies_draft_changes() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path =
            std::env::temp_dir().join(format!("agenda-tui-view-manager-section-editor-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut view = View::new("Board".to_string());
        view.sections.push(Section {
            title: "A".to_string(),
            criteria: Query::default(),
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
        });
        store.create_view(&view).expect("create view");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.mode = Mode::ViewPicker;
        app.picker_index = app
            .views
            .iter()
            .position(|v| v.name == "Board")
            .expect("board view exists");
        app.handle_view_picker_key(KeyCode::Char('V'), &agenda)
            .expect("open manager");
        app.view_manager_pane = ViewManagerPane::Sections;

        app.handle_view_manager_key(KeyCode::Enter, &agenda)
            .expect("open section editor");
        assert_eq!(app.mode, Mode::ViewSectionEditor);

        app.handle_view_section_editor_key(KeyCode::Char('N'))
            .expect("add section in editor");
        app.handle_view_section_editor_key(KeyCode::Esc)
            .expect("return to manager");
        assert_eq!(app.mode, Mode::ViewManagerScreen);

        let selected = app.views.get(app.picker_index).expect("selected view");
        assert_eq!(selected.sections.len(), 2);
        assert!(app.view_manager_dirty);

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn view_manager_unmatched_settings_apply_and_persist_on_save() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path =
            std::env::temp_dir().join(format!("agenda-tui-view-manager-unmatched-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let view = View::new("Board".to_string());
        store.create_view(&view).expect("create view");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.mode = Mode::ViewPicker;
        app.picker_index = app
            .views
            .iter()
            .position(|v| v.name == "Board")
            .expect("board view exists");
        app.handle_view_picker_key(KeyCode::Char('V'), &agenda)
            .expect("open manager");
        app.view_manager_pane = ViewManagerPane::Sections;

        app.handle_view_manager_key(KeyCode::Char('N'), &agenda)
            .expect("add section");
        app.handle_view_manager_key(KeyCode::Char('u'), &agenda)
            .expect("open unmatched settings");
        assert_eq!(app.mode, Mode::ViewUnmatchedSettings);
        app.handle_view_unmatched_settings_key(KeyCode::Char('t'))
            .expect("toggle unmatched");
        app.handle_view_unmatched_settings_key(KeyCode::Esc)
            .expect("return to manager");
        assert_eq!(app.mode, Mode::ViewManagerScreen);

        app.handle_view_manager_key(KeyCode::Char('s'), &agenda)
            .expect("save manager changes");

        let saved = store
            .list_views()
            .expect("list views")
            .into_iter()
            .find(|v| v.name == "Board")
            .expect("saved board");
        assert_eq!(saved.sections.len(), 1);
        assert!(!saved.show_unmatched);

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn view_manager_cancel_discards_unsaved_section_changes() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path =
            std::env::temp_dir().join(format!("agenda-tui-view-manager-cancel-discard-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let view = View::new("Board".to_string());
        store.create_view(&view).expect("create view");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.mode = Mode::ViewPicker;
        app.picker_index = app
            .views
            .iter()
            .position(|v| v.name == "Board")
            .expect("board view exists");
        app.handle_view_picker_key(KeyCode::Char('V'), &agenda)
            .expect("open manager");
        app.view_manager_pane = ViewManagerPane::Sections;

        app.handle_view_manager_key(KeyCode::Char('N'), &agenda)
            .expect("add unsaved section");
        let local_sections = app.views[app.picker_index].sections.len();
        assert_eq!(local_sections, 1);
        assert!(app.view_manager_dirty);

        app.handle_view_manager_key(KeyCode::Esc, &agenda)
            .expect("close manager and discard");
        assert_eq!(app.mode, Mode::ViewPicker);
        assert!(app.status.contains("discarded"));

        app.handle_view_picker_key(KeyCode::Char('V'), &agenda)
            .expect("reopen manager");
        assert_eq!(app.views[app.picker_index].sections.len(), 0);

        let saved = store
            .list_views()
            .expect("list views")
            .into_iter()
            .find(|v| v.name == "Board")
            .expect("saved board");
        assert_eq!(saved.sections.len(), 0);

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn view_manager_section_detail_edits_children_and_assignment_sets() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path =
            std::env::temp_dir().join(format!("agenda-tui-view-manager-section-detail-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let category = Category::new("Work".to_string());
        store.create_category(&category).expect("create category");

        let mut view = View::new("Board".to_string());
        view.sections.push(Section {
            title: "Section A".to_string(),
            criteria: Query::default(),
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
        });
        store.create_view(&view).expect("create view");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.mode = Mode::ViewPicker;
        app.picker_index = app
            .views
            .iter()
            .position(|v| v.name == "Board")
            .expect("board view exists");
        app.handle_view_picker_key(KeyCode::Char('V'), &agenda)
            .expect("open manager");
        app.view_manager_pane = ViewManagerPane::Sections;

        app.handle_view_manager_key(KeyCode::Enter, &agenda)
            .expect("open section editor");
        assert_eq!(app.mode, Mode::ViewSectionEditor);
        app.handle_view_section_editor_key(KeyCode::Enter)
            .expect("open section detail");
        assert_eq!(app.mode, Mode::ViewSectionDetail);

        app.handle_view_section_detail_key(KeyCode::Char('h'))
            .expect("toggle show_children");

        app.handle_view_section_detail_key(KeyCode::Char('a'))
            .expect("open on-insert picker");
        assert_eq!(app.mode, Mode::ViewEditorCategoryPicker);
        app.handle_view_editor_category_key(KeyCode::Char(' '))
            .expect("toggle on-insert category");
        app.handle_view_editor_category_key(KeyCode::Enter)
            .expect("close on-insert picker");
        assert_eq!(app.mode, Mode::ViewSectionDetail);

        app.handle_view_section_detail_key(KeyCode::Char('r'))
            .expect("open on-remove picker");
        assert_eq!(app.mode, Mode::ViewEditorCategoryPicker);
        app.handle_view_editor_category_key(KeyCode::Char(' '))
            .expect("toggle on-remove category");
        app.handle_view_editor_category_key(KeyCode::Enter)
            .expect("close on-remove picker");
        assert_eq!(app.mode, Mode::ViewSectionDetail);

        app.handle_view_section_detail_key(KeyCode::Esc)
            .expect("back to section editor");
        app.handle_view_section_editor_key(KeyCode::Esc)
            .expect("back to manager");
        assert_eq!(app.mode, Mode::ViewManagerScreen);
        app.handle_view_manager_key(KeyCode::Char('s'), &agenda)
            .expect("save manager");

        let saved = store
            .list_views()
            .expect("list views")
            .into_iter()
            .find(|v| v.name == "Board")
            .expect("saved board");
        let section = saved.sections.first().expect("saved section");
        assert!(section.show_children);
        assert_eq!(section.on_insert_assign.len(), 1);
        assert_eq!(section.on_remove_unassign.len(), 1);

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn view_manager_escape_returns_to_view_picker() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!("agenda-tui-view-manager-esc-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.mode = Mode::ViewManagerScreen;

        app.handle_view_manager_key(KeyCode::Esc, &agenda)
            .expect("escape");
        assert_eq!(app.mode, Mode::ViewPicker);

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn view_manager_create_cancel_returns_to_manager() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path =
            std::env::temp_dir().join(format!("agenda-tui-view-manager-create-cancel-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.mode = Mode::ViewManagerScreen;
        app.view_manager_pane = ViewManagerPane::Views;

        app.handle_view_manager_key(KeyCode::Char('N'), &agenda)
            .expect("open create");
        assert_eq!(app.mode, Mode::ViewCreateNameInput);
        app.handle_view_create_name_key(KeyCode::Esc)
            .expect("cancel create");
        assert_eq!(app.mode, Mode::ViewManagerScreen);

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn view_manager_delete_cancel_returns_to_manager() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path =
            std::env::temp_dir().join(format!("agenda-tui-view-manager-delete-cancel-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.mode = Mode::ViewManagerScreen;
        app.view_manager_pane = ViewManagerPane::Views;
        app.picker_index = app
            .views
            .iter()
            .position(|view| view.name == "All Items")
            .unwrap_or(0);

        app.handle_view_manager_key(KeyCode::Char('x'), &agenda)
            .expect("open delete");
        assert_eq!(app.mode, Mode::ViewDeleteConfirm);
        app.handle_view_delete_key(KeyCode::Esc, &agenda)
            .expect("cancel delete");
        assert_eq!(app.mode, Mode::ViewManagerScreen);

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn view_manager_clone_creates_copy_view() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path =
            std::env::temp_dir().join(format!("agenda-tui-view-manager-clone-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut view = View::new("Board".to_string());
        view.show_unmatched = false;
        store.create_view(&view).expect("create base view");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.mode = Mode::ViewManagerScreen;
        app.view_manager_pane = ViewManagerPane::Views;
        app.picker_index = app
            .views
            .iter()
            .position(|v| v.name == "Board")
            .expect("base view present");

        app.handle_view_manager_key(KeyCode::Char('C'), &agenda)
            .expect("clone view");

        let names: Vec<String> = store
            .list_views()
            .expect("list views")
            .into_iter()
            .map(|v| v.name)
            .collect();
        assert!(names.iter().any(|n| n == "Board"));
        assert!(names.iter().any(|n| n == "Board Copy"));

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn view_manager_definition_space_and_save_persists_criteria() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path =
            std::env::temp_dir().join(format!("agenda-tui-view-manager-def-save-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let category = Category::new("Work".to_string());
        store.create_category(&category).expect("create category");

        let mut view = View::new("WorkView".to_string());
        view.criteria.include.insert(category.id);
        store.create_view(&view).expect("create view");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.mode = Mode::ViewPicker;
        app.picker_index = app
            .views
            .iter()
            .position(|v| v.name == "WorkView")
            .expect("view exists");
        app.handle_view_picker_key(KeyCode::Char('V'), &agenda)
            .expect("open manager");

        app.handle_view_manager_key(KeyCode::Tab, &agenda)
            .expect("move to definition pane");
        assert_eq!(app.view_manager_pane, ViewManagerPane::Definition);
        app.handle_view_manager_key(KeyCode::Char(' '), &agenda)
            .expect("toggle sign");
        app.handle_view_manager_key(KeyCode::Char('s'), &agenda)
            .expect("save criteria");

        let saved = store
            .list_views()
            .expect("list views")
            .into_iter()
            .find(|v| v.name == "WorkView")
            .expect("saved view");
        assert!(!saved.criteria.include.contains(&category.id));
        assert!(saved.criteria.exclude.contains(&category.id));

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn view_manager_definition_add_remove_rows_and_save() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path =
            std::env::temp_dir().join(format!("agenda-tui-view-manager-def-rows-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let category = Category::new("Focus".to_string());
        store.create_category(&category).expect("create category");
        let view = View::new("FocusView".to_string());
        store.create_view(&view).expect("create view");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.mode = Mode::ViewPicker;
        app.picker_index = app
            .views
            .iter()
            .position(|v| v.name == "FocusView")
            .expect("view exists");
        app.handle_view_picker_key(KeyCode::Char('V'), &agenda)
            .expect("open manager");
        app.handle_view_manager_key(KeyCode::Tab, &agenda)
            .expect("move to definition pane");

        app.handle_view_manager_key(KeyCode::Char('N'), &agenda)
            .expect("add criteria row");
        app.handle_view_manager_key(KeyCode::Char('s'), &agenda)
            .expect("save include row");

        let saved_with_row = store
            .list_views()
            .expect("list views")
            .into_iter()
            .find(|v| v.name == "FocusView")
            .expect("saved view");
        assert!(saved_with_row.criteria.include.contains(&category.id));

        app.handle_view_manager_key(KeyCode::Char('x'), &agenda)
            .expect("remove criteria row");
        app.handle_view_manager_key(KeyCode::Char('s'), &agenda)
            .expect("save without rows");

        let saved_without_row = store
            .list_views()
            .expect("list views")
            .into_iter()
            .find(|v| v.name == "FocusView")
            .expect("saved view");
        assert!(!saved_without_row.criteria.include.contains(&category.id));

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn view_manager_save_rejects_or_rows_as_not_representable() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path =
            std::env::temp_dir().join(format!("agenda-tui-view-manager-or-invalid-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let alpha = Category::new("Alpha".to_string());
        let beta = Category::new("Beta".to_string());
        store.create_category(&alpha).expect("create alpha");
        store.create_category(&beta).expect("create beta");

        let mut view = View::new("InvalidOr".to_string());
        view.criteria.include.insert(alpha.id);
        store.create_view(&view).expect("create view");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.mode = Mode::ViewPicker;
        app.picker_index = app
            .views
            .iter()
            .position(|v| v.name == "InvalidOr")
            .expect("view exists");
        app.handle_view_picker_key(KeyCode::Char('V'), &agenda)
            .expect("open manager");
        app.view_manager_pane = ViewManagerPane::Definition;
        app.view_manager_rows = vec![
            super::ViewCriteriaRow {
                sign: super::ViewCriteriaSign::Include,
                category_id: alpha.id,
                join_is_or: false,
                depth: 0,
            },
            super::ViewCriteriaRow {
                sign: super::ViewCriteriaSign::Include,
                category_id: beta.id,
                join_is_or: true,
                depth: 0,
            },
        ];
        app.view_manager_dirty = true;

        app.handle_view_manager_key(KeyCode::Char('s'), &agenda)
            .expect("attempt save");
        assert!(app.status.starts_with("Cannot save criteria: "));

        let saved = store
            .list_views()
            .expect("list views")
            .into_iter()
            .find(|v| v.name == "InvalidOr")
            .expect("saved view");
        assert!(saved.criteria.include.contains(&alpha.id));
        assert!(!saved.criteria.include.contains(&beta.id));

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn view_manager_save_rejects_nested_rows_as_not_representable() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path =
            std::env::temp_dir().join(format!("agenda-tui-view-manager-depth-invalid-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let alpha = Category::new("Alpha".to_string());
        store.create_category(&alpha).expect("create alpha");

        let mut view = View::new("InvalidDepth".to_string());
        view.criteria.include.insert(alpha.id);
        store.create_view(&view).expect("create view");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.mode = Mode::ViewPicker;
        app.picker_index = app
            .views
            .iter()
            .position(|v| v.name == "InvalidDepth")
            .expect("view exists");
        app.handle_view_picker_key(KeyCode::Char('V'), &agenda)
            .expect("open manager");
        app.view_manager_pane = ViewManagerPane::Definition;
        app.view_manager_rows = vec![super::ViewCriteriaRow {
            sign: super::ViewCriteriaSign::Include,
            category_id: alpha.id,
            join_is_or: false,
            depth: 1,
        }];
        app.view_manager_dirty = true;

        app.handle_view_manager_key(KeyCode::Char('s'), &agenda)
            .expect("attempt save");
        assert!(app.status.starts_with("Cannot save criteria: "));

        let saved = store
            .list_views()
            .expect("list views")
            .into_iter()
            .find(|v| v.name == "InvalidDepth")
            .expect("saved view");
        assert!(saved.criteria.include.contains(&alpha.id));
        assert!(saved.criteria.exclude.is_empty());

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn view_manager_definition_c_opens_picker_and_applies_category() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path =
            std::env::temp_dir().join(format!("agenda-tui-view-manager-def-picker-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let alpha = Category::new("Alpha".to_string());
        let beta = Category::new("Beta".to_string());
        store.create_category(&alpha).expect("create alpha");
        store.create_category(&beta).expect("create beta");

        let mut view = View::new("PickerView".to_string());
        view.criteria.include.insert(alpha.id);
        store.create_view(&view).expect("create view");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.mode = Mode::ViewPicker;
        app.picker_index = app
            .views
            .iter()
            .position(|v| v.name == "PickerView")
            .expect("view exists");
        app.handle_view_picker_key(KeyCode::Char('V'), &agenda)
            .expect("open manager");
        app.handle_view_manager_key(KeyCode::Tab, &agenda)
            .expect("move to definition");
        assert_eq!(app.view_manager_pane, ViewManagerPane::Definition);

        app.handle_view_manager_key(KeyCode::Char('c'), &agenda)
            .expect("open picker");
        assert_eq!(app.mode, Mode::ViewManagerCategoryPicker);

        app.view_category_index = app
            .category_rows
            .iter()
            .position(|row| row.id == beta.id)
            .expect("beta row");
        app.handle_view_manager_category_picker_key(KeyCode::Enter)
            .expect("apply picker selection");
        assert_eq!(app.mode, Mode::ViewManagerScreen);
        assert_eq!(app.view_manager_rows[0].category_id, beta.id);

        app.handle_view_manager_key(KeyCode::Char('s'), &agenda)
            .expect("save criteria");
        let saved = store
            .list_views()
            .expect("list views")
            .into_iter()
            .find(|v| v.name == "PickerView")
            .expect("saved view");
        assert!(saved.criteria.include.contains(&beta.id));
        assert!(!saved.criteria.include.contains(&alpha.id));

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn normal_mode_comma_and_dot_cycle_views() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!("agenda-tui-symbol-cycle-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        store
            .create_view(&View::new("AAA".to_string()))
            .expect("create first view");
        store
            .create_view(&View::new("BBB".to_string()))
            .expect("create second view");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.set_view_selection_by_name("AAA");
        app.mode = Mode::Normal;
        let start_index = app.view_index;
        let expected_next = app.views[next_index(start_index, app.views.len(), 1)]
            .name
            .clone();

        app.handle_normal_key(KeyCode::Char('.'), &agenda)
            .expect("dot should cycle view");
        assert_eq!(
            app.current_view().map(|view| view.name.as_str()),
            Some(expected_next.as_str())
        );

        app.handle_normal_key(KeyCode::Char(','), &agenda)
            .expect("comma should cycle backwards");
        assert_eq!(
            app.current_view().map(|view| view.name.as_str()),
            Some("AAA")
        );

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn normal_mode_u_opens_item_category_picker_alias() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!("agenda-tui-u-alias-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        store
            .create_category(&Category::new("Work".to_string()))
            .expect("create category");
        store
            .create_item(&Item::new("demo item".to_string()))
            .expect("create item");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.mode = Mode::Normal;

        app.handle_normal_key(KeyCode::Char('u'), &agenda)
            .expect("u alias should open item category picker");
        assert_eq!(app.mode, Mode::ItemAssignCategoryPicker);

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn normal_mode_u_in_preview_provenance_opens_unassign_picker() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!("agenda-tui-u-provenance-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let category = Category::new("Work".to_string());
        store.create_category(&category).expect("create category");
        let item = Item::new("demo item".to_string());
        store.create_item(&item).expect("create item");
        agenda
            .assign_item_manual(item.id, category.id, Some("manual:test".to_string()))
            .expect("assign category");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.mode = Mode::Normal;
        app.show_preview = true;
        app.normal_focus = super::NormalFocus::Preview;
        app.preview_mode = super::PreviewMode::Provenance;

        app.handle_normal_key(KeyCode::Char('u'), &agenda)
            .expect("open unassign picker from preview provenance");
        assert_eq!(app.mode, Mode::InspectUnassignPicker);

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn normal_mode_tab_toggles_focus_when_preview_is_open() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!("agenda-tui-preview-focus-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.mode = Mode::Normal;

        app.handle_normal_key(KeyCode::Char('p'), &agenda)
            .expect("open preview");
        assert_eq!(app.normal_focus, super::NormalFocus::Board);
        assert!(app.show_preview);

        app.handle_normal_key(KeyCode::Tab, &agenda)
            .expect("tab focuses preview");
        assert_eq!(app.normal_focus, super::NormalFocus::Preview);

        app.handle_normal_key(KeyCode::BackTab, &agenda)
            .expect("backtab focuses board");
        assert_eq!(app.normal_focus, super::NormalFocus::Board);

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn normal_mode_p_and_o_manage_preview_modes() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!("agenda-tui-preview-toggle-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.mode = Mode::Normal;
        assert!(!app.show_preview);

        app.handle_normal_key(KeyCode::Char('p'), &agenda)
            .expect("open preview");
        assert!(app.show_preview);
        assert_eq!(app.preview_mode, super::PreviewMode::Summary);

        app.handle_normal_key(KeyCode::Char('o'), &agenda)
            .expect("switch to provenance");
        assert_eq!(app.preview_mode, super::PreviewMode::Provenance);

        app.handle_normal_key(KeyCode::Char('o'), &agenda)
            .expect("switch to summary");
        assert_eq!(app.preview_mode, super::PreviewMode::Summary);

        app.handle_normal_key(KeyCode::Char('p'), &agenda)
            .expect("close preview");
        assert!(!app.show_preview);

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn normal_mode_jk_scrolls_preview_when_preview_is_focused() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!("agenda-tui-preview-scroll-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.mode = Mode::Normal;
        app.show_preview = true;
        app.normal_focus = super::NormalFocus::Preview;
        app.preview_mode = super::PreviewMode::Summary;

        app.handle_normal_key(KeyCode::Char('j'), &agenda)
            .expect("scroll summary down");
        assert_eq!(app.preview_summary_scroll, 1);

        app.handle_normal_key(KeyCode::Char('k'), &agenda)
            .expect("scroll summary up");
        assert_eq!(app.preview_summary_scroll, 0);

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn item_details_categories_are_single_comma_separated_line() {
        let alpha = Category::new("Alpha".to_string());
        let beta = Category::new("Beta".to_string());
        let mut item = Item::new("demo".to_string());
        let assignment = Assignment {
            source: AssignmentSource::Manual,
            assigned_at: chrono::Utc::now(),
            sticky: false,
            origin: None,
        };
        item.assignments.insert(alpha.id, assignment.clone());
        item.assignments.insert(beta.id, assignment);

        let app = App {
            categories: vec![alpha, beta],
            ..App::default()
        };
        let lines = app.item_details_lines_for_item(&item);
        let plain: Vec<String> = lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect()
            })
            .collect();
        assert!(plain
            .iter()
            .any(|line| line == "  Alpha, Beta" || line == "  Beta, Alpha"));
    }

    #[test]
    fn item_edit_note_up_down_moves_cursor_between_lines() {
        let mut app = App::default();
        app.item_edit_note = "first\nsecond".to_string();
        app.item_edit_note_cursor = "first\nse".chars().count();

        app.move_item_edit_note_cursor_up();
        assert_eq!(app.item_edit_note_cursor, "fi".chars().count());

        app.move_item_edit_note_cursor_down();
        assert_eq!(app.item_edit_note_cursor, "first\nse".chars().count());
    }

    #[test]
    fn category_manager_enter_opens_config_editor_for_non_reserved_category() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path =
            std::env::temp_dir().join(format!("agenda-tui-category-config-open-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let category = Category::new("Work".to_string());
        store.create_category(&category).expect("create category");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.mode = Mode::CategoryManager;
        app.category_index = app
            .category_rows
            .iter()
            .position(|row| row.id == category.id)
            .expect("work category row should exist");

        app.handle_category_manager_key(KeyCode::Enter, &agenda)
            .expect("open config editor");

        assert_eq!(app.mode, Mode::CategoryConfigEditor);
        assert_eq!(
            app.category_config_editor
                .as_ref()
                .map(|editor| editor.category_name.as_str()),
            Some("Work")
        );

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn category_manager_enter_refuses_reserved_category_config_edit() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path =
            std::env::temp_dir().join(format!("agenda-tui-category-config-reserved-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.mode = Mode::CategoryManager;
        app.category_index = app
            .category_rows
            .iter()
            .position(|row| row.is_reserved)
            .expect("reserved row should exist");

        app.handle_category_manager_key(KeyCode::Enter, &agenda)
            .expect("attempt open reserved editor");

        assert_eq!(app.mode, Mode::CategoryManager);
        assert!(app.status.contains("reserved"));

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn category_config_editor_save_updates_category_flags_and_note() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path =
            std::env::temp_dir().join(format!("agenda-tui-category-config-save-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let category = Category::new("Work".to_string());
        store.create_category(&category).expect("create category");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.mode = Mode::CategoryManager;
        app.category_index = app
            .category_rows
            .iter()
            .position(|row| row.id == category.id)
            .expect("work category row should exist");
        app.handle_category_manager_key(KeyCode::Enter, &agenda)
            .expect("open category config");
        assert_eq!(app.mode, Mode::CategoryConfigEditor);

        app.handle_category_config_editor_key(KeyCode::Char('e'), &agenda)
            .expect("toggle exclusive");
        app.handle_category_config_editor_key(KeyCode::Tab, &agenda)
            .expect("focus no implicit");
        app.handle_category_config_editor_key(KeyCode::Tab, &agenda)
            .expect("focus actionable");
        app.handle_category_config_editor_key(KeyCode::Tab, &agenda)
            .expect("focus note");
        for c in "line1".chars() {
            app.handle_category_config_editor_key(KeyCode::Char(c), &agenda)
                .expect("type note line1");
        }
        app.handle_category_config_editor_key(KeyCode::Enter, &agenda)
            .expect("insert newline");
        for c in "line2".chars() {
            app.handle_category_config_editor_key(KeyCode::Char(c), &agenda)
                .expect("type note line2");
        }
        app.handle_category_config_editor_key(KeyCode::Tab, &agenda)
            .expect("focus save");
        app.handle_category_config_editor_key(KeyCode::Enter, &agenda)
            .expect("save config");

        assert_eq!(app.mode, Mode::CategoryManager);
        let saved = store.get_category(category.id).expect("load category");
        assert!(saved.is_exclusive);
        assert_eq!(saved.note.as_deref(), Some("line1\nline2"));

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn normal_mode_g_jumps_to_all_items_view() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!("agenda-tui-g-all-items-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        store
            .create_view(&View::new("Work Board".to_string()))
            .expect("create second view");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.set_view_selection_by_name("Work Board");
        app.mode = Mode::Normal;

        app.handle_normal_key(KeyCode::Char('g'), &agenda)
            .expect("g should jump to all items view");
        assert_eq!(
            app.current_view().map(|view| view.name.as_str()),
            Some("All Items")
        );

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn normal_mode_d_toggles_done_state() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!("agenda-tui-d-toggle-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let work = Category::new("Work".to_string());
        store.create_category(&work).expect("create category");
        let item = Item::new("demo item".to_string());
        store.create_item(&item).expect("create item");
        agenda
            .assign_item_manual(item.id, work.id, Some("manual:test".to_string()))
            .expect("assign actionable category");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.mode = Mode::Normal;
        app.set_item_selection_by_id(item.id);

        app.handle_normal_key(KeyCode::Char('d'), &agenda)
            .expect("d should mark done");
        assert!(store.get_item(item.id).expect("load item").is_done);

        app.handle_normal_key(KeyCode::Char('D'), &agenda)
            .expect("D should clear done");
        assert!(!store.get_item(item.id).expect("load item").is_done);

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn normal_mode_d_refuses_done_for_non_actionable_item() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!("agenda-tui-d-non-actionable-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut reference = Category::new("Reference".to_string());
        reference.is_actionable = false;
        store
            .create_category(&reference)
            .expect("create non-actionable category");
        let item = Item::new("demo item".to_string());
        store.create_item(&item).expect("create item");
        agenda
            .assign_item_manual(item.id, reference.id, Some("manual:test".to_string()))
            .expect("assign non-actionable category");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.mode = Mode::Normal;
        app.set_item_selection_by_id(item.id);

        app.handle_normal_key(KeyCode::Char('d'), &agenda)
            .expect("d should be handled");
        assert!(!store.get_item(item.id).expect("load item").is_done);
        assert_eq!(
            app.status,
            "Done unavailable: item has no actionable category assignments"
        );

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn view_create_category_picker_supports_include_and_exclude() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!("agenda-tui-view-create-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let include_cat = Category::new("ProjectY".to_string());
        let exclude_cat = Category::new("Someday".to_string());
        store
            .create_category(&include_cat)
            .expect("create include category");
        store
            .create_category(&exclude_cat)
            .expect("create exclude category");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.mode = Mode::ViewCreateCategoryPicker;
        app.view_pending_name = Some("Mixed".to_string());
        app.view_category_index = app
            .category_rows
            .iter()
            .position(|row| row.id == include_cat.id)
            .expect("include row should exist");
        app.handle_view_create_category_key(KeyCode::Char('+'), &agenda)
            .expect("include toggle should work");

        app.view_category_index = app
            .category_rows
            .iter()
            .position(|row| row.id == exclude_cat.id)
            .expect("exclude row should exist");
        app.handle_view_create_category_key(KeyCode::Char('-'), &agenda)
            .expect("exclude toggle should work");

        app.handle_view_create_category_key(KeyCode::Enter, &agenda)
            .expect("view create should succeed");

        let created = store
            .list_views()
            .expect("list views")
            .into_iter()
            .find(|view| view.name == "Mixed")
            .expect("created view exists");
        assert!(created.criteria.include.contains(&include_cat.id));
        assert!(created.criteria.exclude.contains(&exclude_cat.id));

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn view_editor_action_selection_opens_expected_picker() {
        let mut app = App::default();
        app.category_rows = vec![CategoryListRow {
            id: CategoryId::new_v4(),
            name: "Work".to_string(),
            depth: 0,
            is_reserved: false,
            is_exclusive: false,
            is_actionable: true,
            enable_implicit_string: true,
        }];
        app.open_view_editor(View::new("Board".to_string()));
        if let Some(editor) = &mut app.view_editor {
            editor.action_index = 0;
        }

        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path =
            std::env::temp_dir().join(format!("agenda-tui-view-editor-action-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        app.handle_view_editor_key(KeyCode::Char('o'), &agenda)
            .expect("open selected action");
        assert_eq!(app.mode, Mode::ViewEditorCategoryPicker);

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn text_input_editing_supports_navigation_insert_backspace_and_delete() {
        let mut app = App::default();
        app.set_input("ac".to_string());

        assert!(app.handle_text_input_key(KeyCode::Left));
        assert_eq!(app.input_cursor, 1);

        assert!(app.handle_text_input_key(KeyCode::Char('b')));
        assert_eq!(app.input, "abc");
        assert_eq!(app.input_cursor, 2);

        assert!(app.handle_text_input_key(KeyCode::Backspace));
        assert_eq!(app.input, "ac");
        assert_eq!(app.input_cursor, 1);

        assert!(app.handle_text_input_key(KeyCode::Delete));
        assert_eq!(app.input, "a");
        assert_eq!(app.input_cursor, 1);
    }

    #[test]
    fn should_render_unmatched_lane_hides_empty_and_shows_non_empty() {
        assert!(!should_render_unmatched_lane(&[]));
        let item = Item::new("one".to_string());
        assert!(should_render_unmatched_lane(&[item]));
    }

    #[test]
    fn category_target_set_mut_supports_view_and_section_targets() {
        let mut view = View::new("Board".to_string());
        view.sections.push(Section {
            title: "One".to_string(),
            criteria: Query::default(),
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
        });
        let category_id = CategoryId::new_v4();

        let view_include = category_target_set_mut(&mut view, 0, CategoryEditTarget::ViewInclude)
            .expect("view include set");
        view_include.insert(category_id);
        assert!(view.criteria.include.contains(&category_id));

        let section_insert =
            category_target_set_mut(&mut view, 0, CategoryEditTarget::SectionOnInsertAssign)
                .expect("section on_insert_assign set");
        section_insert.insert(category_id);
        assert!(view.sections[0].on_insert_assign.contains(&category_id));
    }

    #[test]
    fn bucket_target_set_mut_supports_view_and_section_targets() {
        let mut view = View::new("Board".to_string());
        view.sections.push(Section {
            title: "One".to_string(),
            criteria: Query::default(),
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
        });

        let view_virtual =
            bucket_target_set_mut(&mut view, 0, BucketEditTarget::ViewVirtualInclude)
                .expect("view virtual include set");
        view_virtual.insert(WhenBucket::Today);
        assert!(view.criteria.virtual_include.contains(&WhenBucket::Today));

        let section_virtual =
            bucket_target_set_mut(&mut view, 0, BucketEditTarget::SectionVirtualExclude)
                .expect("section virtual exclude set");
        section_virtual.insert(WhenBucket::NoDate);
        assert!(view.sections[0]
            .criteria
            .virtual_exclude
            .contains(&WhenBucket::NoDate));
    }

    #[test]
    fn when_bucket_options_exposes_expected_bucket_set() {
        let options = when_bucket_options();
        assert!(options.contains(&WhenBucket::Today));
        assert!(options.contains(&WhenBucket::NoDate));
        assert_eq!(options.len(), 8);
    }

    #[test]
    fn item_assignment_labels_are_sorted_and_human_readable() {
        let category_a = CategoryId::new_v4();
        let category_b = CategoryId::new_v4();
        let mut item = Item::new("demo".to_string());
        item.assignments.insert(
            category_a,
            agenda_core::model::Assignment {
                source: agenda_core::model::AssignmentSource::Manual,
                assigned_at: chrono::Utc::now(),
                sticky: true,
                origin: None,
            },
        );
        item.assignments.insert(
            category_b,
            agenda_core::model::Assignment {
                source: agenda_core::model::AssignmentSource::Manual,
                assigned_at: chrono::Utc::now(),
                sticky: true,
                origin: None,
            },
        );
        let names = HashMap::from([
            (category_a, "slotB".to_string()),
            (category_b, "garage".to_string()),
        ]);
        let labels = item_assignment_labels(&item, &names);
        assert_eq!(labels, vec!["garage".to_string(), "slotB".to_string()]);
    }

    #[test]
    fn board_annotation_header_and_rows_share_grid_boundaries() {
        let widths = board_column_widths(72);
        let header = board_annotation_header(widths);
        let row = board_item_row(
            true,
            "2026-02-17",
            "alignment check",
            "Home, SlotA, SlotB",
            widths,
        );

        let header_pipes: Vec<usize> = header.match_indices('|').map(|(idx, _)| idx).collect();
        let row_pipes: Vec<usize> = row.match_indices('|').map(|(idx, _)| idx).collect();
        assert_eq!(header_pipes, row_pipes);
    }

    #[test]
    fn board_item_row_truncates_to_slot_width() {
        let widths = board_column_widths(44);
        let row = board_item_row(
            false,
            "2026-02-17 14:00:00",
            "very long item text that should truncate cleanly",
            "one, two, three, four, five, six",
            widths,
        );

        assert!(row.len() <= 44);
        assert!(row.contains("..."));
    }
}
