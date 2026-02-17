use std::collections::{HashMap, HashSet};
use std::io;

use agenda_core::agenda::Agenda;
use agenda_core::matcher::{unknown_hashtag_tokens, SubstringClassifier};
use agenda_core::model::{Category, CategoryId, Item, ItemId, Query, Section, View};
use agenda_core::query::resolve_view;
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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    Normal,
    AddInput,
    ItemEditInput,
    NoteEditInput,
    ItemAssignCategoryPicker,
    InspectUnassignPicker,
    FilterInput,
    ViewPicker,
    ViewCreateNameInput,
    ViewCreateCategoryPicker,
    ViewRenameInput,
    ViewEditCategoryPicker,
    ConfirmDelete,
    CategoryManager,
    CategoryCreateInput,
    CategoryRenameInput,
    CategoryReparentPicker,
    CategoryDeleteConfirm,
}

struct App {
    mode: Mode,
    status: String,
    input: String,
    input_cursor: usize,
    filter: Option<String>,
    show_inspect: bool,

    views: Vec<View>,
    view_index: usize,
    picker_index: usize,
    view_pending_name: Option<String>,
    view_pending_edit_name: Option<String>,
    view_category_index: usize,

    categories: Vec<Category>,
    category_rows: Vec<CategoryListRow>,
    category_index: usize,
    category_create_parent: Option<CategoryId>,
    category_reparent_options: Vec<ReparentOptionRow>,
    category_reparent_index: usize,
    item_assign_category_index: usize,
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
                "Press n to add, a to assign item to category, F8 to switch views, F9 for categories, q to quit"
                    .to_string(),
            input: String::new(),
            input_cursor: 0,
            filter: None,
            show_inspect: false,
            views: Vec::new(),
            view_index: 0,
            picker_index: 0,
            view_pending_name: None,
            view_pending_edit_name: None,
            view_category_index: 0,
            categories: Vec::new(),
            category_rows: Vec::new(),
            category_index: 0,
            category_create_parent: None,
            category_reparent_options: Vec::new(),
            category_reparent_index: 0,
            item_assign_category_index: 0,
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
            Mode::InspectUnassignPicker => self.handle_inspect_unassign_key(code, agenda),
            Mode::FilterInput => self.handle_filter_key(code, agenda),
            Mode::ViewPicker => self.handle_view_picker_key(code, agenda),
            Mode::ViewCreateNameInput => self.handle_view_create_name_key(code),
            Mode::ViewCreateCategoryPicker => self.handle_view_create_category_key(code, agenda),
            Mode::ViewRenameInput => self.handle_view_rename_key(code, agenda),
            Mode::ViewEditCategoryPicker => self.handle_view_edit_category_key(code, agenda),
            Mode::ConfirmDelete => self.handle_confirm_delete_key(code, agenda),
            Mode::CategoryManager => self.handle_category_manager_key(code, agenda),
            Mode::CategoryCreateInput => self.handle_category_create_key(code, agenda),
            Mode::CategoryRenameInput => self.handle_category_rename_key(code, agenda),
            Mode::CategoryReparentPicker => self.handle_category_reparent_key(code, agenda),
            Mode::CategoryDeleteConfirm => self.handle_category_delete_key(code, agenda),
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

    fn handle_normal_key(&mut self, code: KeyCode, agenda: &Agenda<'_>) -> Result<bool, String> {
        match code {
            KeyCode::Char('q') => return Ok(true),
            KeyCode::Down | KeyCode::Char('j') => self.move_item_cursor(1),
            KeyCode::Up | KeyCode::Char('k') => self.move_item_cursor(-1),
            KeyCode::Right | KeyCode::Char('l') => self.move_slot_cursor(1),
            KeyCode::Left | KeyCode::Char('h') => self.move_slot_cursor(-1),
            KeyCode::Char('n') => {
                self.mode = Mode::AddInput;
                self.clear_input();
                self.status = "Add item: type text and press Enter".to_string();
            }
            KeyCode::Char('e') => {
                if let Some(item) = self.selected_item() {
                    let existing_text = item.text.clone();
                    self.mode = Mode::ItemEditInput;
                    self.set_input(existing_text);
                    self.status = "Edit item text: Enter to save, Esc to cancel".to_string();
                } else {
                    self.status = "No selected item to edit".to_string();
                }
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
            KeyCode::F(8) => {
                self.mode = Mode::ViewPicker;
                self.picker_index = self.view_index;
                self.status =
                    "View picker: Enter switch, N create, r rename, e edit include, Esc cancel"
                        .to_string();
            }
            KeyCode::F(9) => {
                self.mode = Mode::CategoryManager;
                self.status =
                    "Category manager: n child, N root, x delete, Esc to close".to_string();
            }
            KeyCode::Char('a') => {
                if self.selected_item_id().is_none() {
                    self.status = "No selected item to assign".to_string();
                } else if self.category_rows.is_empty() {
                    self.status = "No categories available".to_string();
                } else {
                    self.mode = Mode::ItemAssignCategoryPicker;
                    self.item_assign_category_index =
                        first_non_reserved_category_index(&self.category_rows);
                    self.status =
                        "Assign item to category: j/k select category, Enter assign, Esc cancel"
                            .to_string();
                }
            }
            KeyCode::Char('i') => {
                self.show_inspect = !self.show_inspect;
            }
            KeyCode::Char('u') => {
                if !self.show_inspect {
                    self.status = "Open inspect panel (i) to unassign".to_string();
                } else if let Some(item) = self.selected_item() {
                    let rows = self.inspect_assignment_rows_for_item(item);
                    if rows.is_empty() {
                        self.status = "Selected item has no assignments".to_string();
                    } else {
                        self.mode = Mode::InspectUnassignPicker;
                        self.inspect_assignment_index = 0;
                        self.status = "Unassign: j/k select assignment, Enter confirm, Esc cancel"
                            .to_string();
                    }
                } else {
                    self.status = "No selected item to unassign".to_string();
                }
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
            KeyCode::Char('d') => {
                if let Some(item_id) = self.selected_item_id() {
                    agenda.mark_item_done(item_id).map_err(|e| e.to_string())?;
                    self.refresh(agenda.store())?;
                    self.status = "Marked item done".to_string();
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
                self.status = "Edit canceled".to_string();
            }
            KeyCode::Enter => {
                let Some(item_id) = self.selected_item_id() else {
                    self.mode = Mode::Normal;
                    self.clear_input();
                    self.status = "Edit failed: no selected item".to_string();
                    return Ok(false);
                };

                let updated_text = self.input.trim().to_string();
                if updated_text.is_empty() {
                    self.mode = Mode::Normal;
                    self.clear_input();
                    self.status = "Edit canceled: text cannot be empty".to_string();
                    return Ok(false);
                }

                let mut item = agenda
                    .store()
                    .get_item(item_id)
                    .map_err(|e| e.to_string())?;
                if item.text == updated_text {
                    self.mode = Mode::Normal;
                    self.clear_input();
                    self.status = "Edit canceled: no text change".to_string();
                    return Ok(false);
                }

                item.text = updated_text;
                item.modified_at = Utc::now();
                let reference_date = Local::now().date_naive();
                agenda
                    .update_item_with_reference_date(&item, reference_date)
                    .map_err(|e| e.to_string())?;
                self.refresh(agenda.store())?;
                self.set_item_selection_by_id(item_id);
                self.mode = Mode::Normal;
                self.clear_input();
                self.status = "Item text updated".to_string();
            }
            _ if self.handle_text_input_key(code) => {}
            _ => {}
        }
        Ok(false)
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
                self.mode = Mode::Normal;
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
            KeyCode::Enter => {
                let Some(item_id) = self.selected_item_id() else {
                    self.mode = Mode::Normal;
                    self.status = "Assign failed: no selected item".to_string();
                    return Ok(false);
                };
                let Some(row) = self
                    .category_rows
                    .get(self.item_assign_category_index)
                    .cloned()
                else {
                    self.mode = Mode::Normal;
                    self.status = "Assign failed: no category selected".to_string();
                    return Ok(false);
                };

                if row.name.eq_ignore_ascii_case("Done") {
                    agenda.mark_item_done(item_id).map_err(|e| e.to_string())?;
                    self.refresh(agenda.store())?;
                    self.set_item_selection_by_id(item_id);
                    self.mode = Mode::Normal;
                    self.status = "Assigned item to category Done (marked done)".to_string();
                    return Ok(false);
                }

                let result = agenda
                    .assign_item_manual(item_id, row.id, Some("manual:tui.assign".to_string()))
                    .map_err(|e| e.to_string())?;
                self.refresh(agenda.store())?;
                self.set_item_selection_by_id(item_id);
                self.mode = Mode::Normal;
                self.status = format!(
                    "Assigned item to category {} (new_assignments={})",
                    row.name,
                    result.new_assignments.len()
                );
            }
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
                    .store()
                    .unassign_item(item_id, row.category_id)
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
                    self.status = format!("Switched to view: {view_name}");
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
                self.status = "Create view: type name and press Enter".to_string();
            }
            KeyCode::Char('r') => {
                if let Some(view) = self.views.get(self.picker_index).cloned() {
                    self.mode = Mode::ViewRenameInput;
                    self.set_input(view.name.clone());
                    self.view_pending_edit_name = Some(view.name.clone());
                    self.status = format!("Rename view {}: type name and Enter", view.name);
                } else {
                    self.status = "No selected view to rename".to_string();
                }
            }
            KeyCode::Char('e') => {
                if let Some(view) = self.views.get(self.picker_index).cloned() {
                    self.mode = Mode::ViewEditCategoryPicker;
                    self.view_pending_edit_name = Some(view.name.clone());
                    self.view_category_index =
                        first_non_reserved_category_index(&self.category_rows);
                    self.status =
                        "Edit view include category: j/k select category, Enter save, Esc cancel"
                            .to_string();
                } else {
                    self.status = "No selected view to edit".to_string();
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

    fn handle_view_create_name_key(&mut self, code: KeyCode) -> Result<bool, String> {
        match code {
            KeyCode::Esc => {
                self.mode = Mode::ViewPicker;
                self.clear_input();
                self.view_pending_name = None;
                self.status = "View create canceled".to_string();
            }
            KeyCode::Enter => {
                let name = self.input.trim().to_string();
                if name.is_empty() {
                    self.mode = Mode::ViewPicker;
                    self.clear_input();
                    self.view_pending_name = None;
                    self.status = "View create canceled (empty name)".to_string();
                } else {
                    self.view_pending_name = Some(name.clone());
                    self.view_category_index =
                        first_non_reserved_category_index(&self.category_rows);
                    self.mode = Mode::ViewCreateCategoryPicker;
                    self.clear_input();
                    self.status =
                        format!("Create view {name}: select include category and press Enter");
                }
            }
            _ if self.handle_text_input_key(code) => {}
            _ => {}
        }
        Ok(false)
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
            KeyCode::Enter => {
                let Some(name) = self.view_pending_name.clone() else {
                    self.mode = Mode::ViewPicker;
                    self.status = "View create failed: missing name".to_string();
                    return Ok(false);
                };

                let mut view = View::new(name.clone());
                if let Some(row) = self.category_rows.get(self.view_category_index) {
                    view.criteria.include.insert(row.id);
                }

                match agenda.store().create_view(&view) {
                    Ok(()) => {
                        self.refresh(agenda.store())?;
                        self.set_view_selection_by_name(&view.name);
                        self.mode = Mode::Normal;
                        self.view_pending_name = None;
                        self.status = format!("Created view {}", view.name);
                    }
                    Err(err) => {
                        self.mode = Mode::ViewPicker;
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
        match code {
            KeyCode::Esc => {
                self.mode = Mode::ViewPicker;
                self.clear_input();
                self.view_pending_edit_name = None;
                self.status = "View rename canceled".to_string();
            }
            KeyCode::Enter => {
                let Some(view_name) = self.view_pending_edit_name.clone() else {
                    self.mode = Mode::ViewPicker;
                    self.clear_input();
                    self.status = "View rename failed: no selected view".to_string();
                    return Ok(false);
                };

                let new_name = self.input.trim().to_string();
                if new_name.is_empty() {
                    self.mode = Mode::ViewPicker;
                    self.clear_input();
                    self.view_pending_edit_name = None;
                    self.status = "View rename canceled (empty name)".to_string();
                    return Ok(false);
                }

                let Some(mut view) = self
                    .views
                    .iter()
                    .find(|view| view.name.eq_ignore_ascii_case(&view_name))
                    .cloned()
                else {
                    self.mode = Mode::ViewPicker;
                    self.clear_input();
                    self.view_pending_edit_name = None;
                    self.status = "View rename failed: selected view not found".to_string();
                    return Ok(false);
                };

                if view.name == new_name {
                    self.mode = Mode::ViewPicker;
                    self.clear_input();
                    self.view_pending_edit_name = None;
                    self.status = "View rename canceled (unchanged)".to_string();
                    return Ok(false);
                }

                view.name = new_name.clone();
                match agenda.store().update_view(&view) {
                    Ok(()) => {
                        self.refresh(agenda.store())?;
                        self.set_view_selection_by_name(&new_name);
                        self.mode = Mode::ViewPicker;
                        self.clear_input();
                        self.view_pending_edit_name = None;
                        self.status = format!("Renamed view to {}", new_name);
                    }
                    Err(err) => {
                        self.mode = Mode::ViewPicker;
                        self.clear_input();
                        self.view_pending_edit_name = None;
                        self.status = format!("View rename failed: {err}");
                    }
                }
            }
            _ if self.handle_text_input_key(code) => {}
            _ => {}
        }
        Ok(false)
    }

    fn handle_view_edit_category_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> Result<bool, String> {
        match code {
            KeyCode::Esc => {
                self.mode = Mode::ViewPicker;
                self.view_pending_edit_name = None;
                self.status = "View edit canceled".to_string();
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
            KeyCode::Enter => {
                let Some(view_name) = self.view_pending_edit_name.clone() else {
                    self.mode = Mode::ViewPicker;
                    self.status = "View edit failed: no selected view".to_string();
                    return Ok(false);
                };
                let Some(row) = self.category_rows.get(self.view_category_index).cloned() else {
                    self.mode = Mode::ViewPicker;
                    self.status = "View edit failed: no category selected".to_string();
                    return Ok(false);
                };
                let Some(mut view) = self
                    .views
                    .iter()
                    .find(|view| view.name.eq_ignore_ascii_case(&view_name))
                    .cloned()
                else {
                    self.mode = Mode::ViewPicker;
                    self.status = "View edit failed: selected view not found".to_string();
                    return Ok(false);
                };

                view.criteria.include.clear();
                view.criteria.include.insert(row.id);

                match agenda.store().update_view(&view) {
                    Ok(()) => {
                        self.refresh(agenda.store())?;
                        self.set_view_selection_by_name(&view.name);
                        self.mode = Mode::ViewPicker;
                        self.view_pending_edit_name = None;
                        self.status = format!(
                            "Updated view {} include category to {}",
                            view.name, row.name
                        );
                    }
                    Err(err) => {
                        self.mode = Mode::ViewPicker;
                        self.view_pending_edit_name = None;
                        self.status = format!("View edit failed: {err}");
                    }
                }
            }
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
                    .unwrap_or_else(|| "(root)".to_string());
                self.status = format!("Create category under {parent}: type name and Enter");
            }
            KeyCode::Char('N') => {
                self.mode = Mode::CategoryCreateInput;
                self.clear_input();
                self.category_create_parent = None;
                self.status = "Create top-level category: type name and Enter".to_string();
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
            KeyCode::Char('t') => {
                if let Some(category_id) = self.selected_category_id() {
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
                        updated.name,
                        updated.is_exclusive,
                        result.processed_items,
                        result.affected_items
                    );
                }
            }
            KeyCode::Char('i') => {
                if let Some(category_id) = self.selected_category_id() {
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
                        "{} implicit-string={} (processed_items={}, affected_items={})",
                        updated.name,
                        updated.enable_implicit_string,
                        result.processed_items,
                        result.affected_items
                    );
                }
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
                    category.parent = self.category_create_parent;
                    let create_result =
                        agenda.create_category(&category).map_err(|e| e.to_string());
                    match create_result {
                        Ok(result) => {
                            self.refresh(agenda.store())?;
                            self.set_category_selection_by_id(category.id);
                            self.mode = Mode::CategoryManager;
                            self.status = format!(
                                "Created category {} (processed_items={}, affected_items={})",
                                category.name, result.processed_items, result.affected_items
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

        let mut slots = Vec::new();
        if self.views.is_empty() {
            slots.push(Slot {
                title: "All Items (no views configured)".to_string(),
                items,
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
                slots.push(Slot {
                    title: result
                        .unmatched_label
                        .unwrap_or_else(|| "Unassigned".to_string()),
                    items: unmatched_items,
                    context: SlotContext::Unmatched,
                });
            }

            if slots.is_empty() {
                slots.push(Slot {
                    title: "All Items".to_string(),
                    items,
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
        let inspect_len = self
            .selected_item()
            .map(|item| self.inspect_assignment_rows_for_item(item).len())
            .unwrap_or(0);
        self.inspect_assignment_index = self
            .inspect_assignment_index
            .min(inspect_len.saturating_sub(1));

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

        if matches!(
            self.mode,
            Mode::ViewPicker | Mode::ViewCreateNameInput | Mode::ViewRenameInput
        ) {
            self.render_view_picker(frame, centered_rect(60, 60, frame.area()));
        }
        if self.mode == Mode::ItemAssignCategoryPicker {
            self.render_item_assign_picker(frame, centered_rect(72, 72, frame.area()));
        }
        if matches!(
            self.mode,
            Mode::ViewCreateCategoryPicker | Mode::ViewEditCategoryPicker
        ) {
            self.render_view_category_picker(frame, centered_rect(72, 72, frame.area()));
        }
        if matches!(
            self.mode,
            Mode::CategoryManager
                | Mode::CategoryCreateInput
                | Mode::CategoryRenameInput
                | Mode::CategoryReparentPicker
                | Mode::CategoryDeleteConfirm
        ) {
            self.render_category_manager(frame, centered_rect(72, 72, frame.area()));
        }
    }

    fn input_prompt_prefix(&self) -> Option<&'static str> {
        match self.mode {
            Mode::AddInput => Some("Add> "),
            Mode::ItemEditInput => Some("Edit> "),
            Mode::NoteEditInput => Some("Note> "),
            Mode::FilterInput => Some("Filter> "),
            Mode::ViewCreateNameInput => Some("View create> "),
            Mode::ViewRenameInput => Some("View rename> "),
            Mode::CategoryCreateInput => Some("Category create> "),
            Mode::CategoryRenameInput => Some("Category rename> "),
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
        let horizontal = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(area);

        let section_lines: Vec<Line<'_>> = self
            .slots
            .iter()
            .enumerate()
            .map(|(index, slot)| {
                let marker = if index == self.slot_index { "> " } else { "  " };
                Line::from(format!("{marker}{} ({})", slot.title, slot.items.len()))
            })
            .collect();

        frame.render_widget(
            Paragraph::new(section_lines).block(
                Block::default()
                    .title("Sections")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Blue)),
            ),
            horizontal[0],
        );

        if self.show_inspect {
            let right = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
                .split(horizontal[1]);
            frame.render_widget(self.render_item_panel(), right[0]);
            frame.render_widget(self.render_inspect_panel(), right[1]);
        } else {
            frame.render_widget(self.render_item_panel(), horizontal[1]);
        }
    }

    fn render_item_panel(&self) -> Paragraph<'_> {
        let lines = if let Some(slot) = self.current_slot() {
            if slot.items.is_empty() {
                vec![Line::from("(no items in section)")]
            } else {
                slot.items
                    .iter()
                    .enumerate()
                    .map(|(index, item)| {
                        let marker = if index == self.item_index { "> " } else { "  " };
                        let when = item
                            .when_date
                            .map(|dt| dt.to_string())
                            .unwrap_or_else(|| "-".to_string());
                        let done = if item.is_done { "[done] " } else { "" };
                        Line::from(format!("{marker}{done}{} | {}", when, item.text))
                    })
                    .collect()
            }
        } else {
            vec![Line::from("(no section)")]
        };

        Paragraph::new(lines)
            .block(
                Block::default()
                    .title("Items")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Blue)),
            )
            .wrap(Wrap { trim: false })
    }

    fn render_inspect_panel(&self) -> Paragraph<'_> {
        let mut lines = vec![Line::from("Assignment provenance")];
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
                    .title("Inspect (i)")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow)),
            )
            .wrap(Wrap { trim: false })
    }

    fn render_footer(&self) -> Paragraph<'_> {
        let prompt = match self.mode {
            Mode::AddInput => format!("Add> {}", self.input),
            Mode::ItemEditInput => format!("Edit> {}", self.input),
            Mode::NoteEditInput => format!("Note> {}", self.input),
            Mode::FilterInput => format!("Filter> {}", self.input),
            Mode::ConfirmDelete => "Delete selected item? y/n".to_string(),
            Mode::ViewCreateNameInput => format!("View create> {}", self.input),
            Mode::ViewRenameInput => format!("View rename> {}", self.input),
            Mode::ViewCreateCategoryPicker => "Select include category for new view".to_string(),
            Mode::ViewEditCategoryPicker => "Select include category for selected view".to_string(),
            Mode::CategoryCreateInput => format!("Category create> {}", self.input),
            Mode::CategoryRenameInput => format!("Category rename> {}", self.input),
            Mode::CategoryReparentPicker => "Select category parent".to_string(),
            Mode::CategoryDeleteConfirm => "Delete selected category? y/n".to_string(),
            Mode::ItemAssignCategoryPicker => "Select category for selected item".to_string(),
            Mode::InspectUnassignPicker => "Select assignment to unassign".to_string(),
            _ => self.status.clone(),
        };
        let footer_title = match self.mode {
            Mode::CategoryManager => {
                "j/k:select  n/N:create  r:rename  p:reparent  t:toggle-exclusive  i:toggle-implicit  x:delete  Esc/F9:close"
            }
            Mode::CategoryCreateInput => "Type category name, Enter:create, Esc:cancel",
            Mode::CategoryRenameInput => "Type new category name, Enter:rename, Esc:cancel",
            Mode::CategoryReparentPicker => "j/k:select parent  Enter:reparent  Esc:cancel",
            Mode::CategoryDeleteConfirm => "y:confirm delete  n:cancel",
            Mode::ViewPicker => "j/k:select  Enter:switch  N:create  r:rename  e:edit include  Esc:cancel",
            Mode::ViewCreateNameInput => "Type view name, Enter:next, Esc:cancel",
            Mode::ViewRenameInput => "Type new view name, Enter:rename, Esc:cancel",
            Mode::ViewCreateCategoryPicker => "j/k:select category  Enter:create view  Esc:cancel",
            Mode::ViewEditCategoryPicker => "j/k:select category  Enter:update view  Esc:cancel",
            Mode::ItemAssignCategoryPicker => "j/k:select category  Enter:assign item to category  Esc:cancel",
            Mode::ItemEditInput => "Edit selected item text, Enter:save, Esc:cancel",
            Mode::NoteEditInput => "Edit selected note, Enter:save (empty clears), Esc:cancel",
            Mode::InspectUnassignPicker => "j/k:select assignment  Enter:unassign  Esc:cancel",
            _ => {
                "n:add  a:assign-item  e:edit  m:note  u:unassign  [/]:filter  F8:views  F9:categories  []:move  r:remove  d:done  x:delete  i:inspect  q:quit"
            }
        };

        Paragraph::new(prompt).block(Block::default().title(footer_title).borders(Borders::ALL))
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
                    let marker = if index == self.picker_index {
                        "> "
                    } else {
                        "  "
                    };
                    Line::from(format!("{marker}{}", view.name))
                })
                .collect()
        };

        frame.render_widget(
            Paragraph::new(lines).block(
                Block::default()
                    .title("Select View")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Magenta)),
            ),
            area,
        );
    }

    fn render_view_category_picker(&self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        frame.render_widget(Clear, area);

        let mut lines = vec![Line::from("Choose include category for view")];
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
                lines.push(Line::from(format!(
                    "{marker}{}{}{}",
                    "  ".repeat(row.depth),
                    row.name,
                    suffix
                )));
            }
        }

        let title = match self.mode {
            Mode::ViewCreateCategoryPicker => "Create View Include",
            Mode::ViewEditCategoryPicker => "Edit View Include",
            _ => "View Include",
        };
        frame.render_widget(
            Paragraph::new(lines).block(
                Block::default()
                    .title(title)
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Magenta)),
            ),
            area,
        );
    }

    fn render_item_assign_picker(&self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        frame.render_widget(Clear, area);

        let mut lines = vec![Line::from("Assign selected item to category")];
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
                lines.push(Line::from(format!(
                    "{marker}{}{}{}",
                    "  ".repeat(row.depth),
                    row.name,
                    suffix
                )));
            }
        }

        frame.render_widget(
            Paragraph::new(lines).block(
                Block::default()
                    .title("Assign Item")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan)),
            ),
            area,
        );
    }

    fn render_category_manager(&self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        frame.render_widget(Clear, area);

        let mut lines = vec![Line::from(
            "Categories are global. n/N create, r rename, p reparent, t/i toggle, x delete.",
        )];

        if self.category_rows.is_empty() {
            lines.push(Line::from("(no categories)"));
        } else {
            for (index, row) in self.category_rows.iter().enumerate() {
                let marker = if index == self.category_index {
                    "> "
                } else {
                    "  "
                };
                let indent = "  ".repeat(row.depth);
                let mut flags = Vec::new();
                if row.is_reserved {
                    flags.push("reserved");
                }
                if row.is_exclusive {
                    flags.push("exclusive");
                }
                if !row.enable_implicit_string {
                    flags.push("no-implicit");
                }
                let suffix = if flags.is_empty() {
                    String::new()
                } else {
                    format!(" [{}]", flags.join(","))
                };
                lines.push(Line::from(format!(
                    "{marker}{indent}{}{}",
                    row.name, suffix
                )));
            }
        }

        if self.mode == Mode::CategoryCreateInput {
            let parent = self
                .create_parent_name()
                .unwrap_or_else(|| "(root)".to_string());
            lines.push(Line::from(""));
            lines.push(Line::from(format!("Create parent: {parent}")));
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
                for (index, option) in self.category_reparent_options.iter().enumerate() {
                    let marker = if index == self.category_reparent_index {
                        "> "
                    } else {
                        "  "
                    };
                    lines.push(Line::from(format!("{marker}{}", option.label)));
                }
            }
        }

        frame.render_widget(
            Paragraph::new(lines).block(
                Block::default()
                    .title("Category Manager")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Green)),
            ),
            area,
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

    use super::{
        add_capture_status_message, build_category_rows, build_reparent_options,
        first_non_reserved_category_index, App, CategoryListRow, Mode,
    };
    use agenda_core::model::{Category, CategoryId};
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
            enable_implicit_string: false,
        };
        let user = CategoryListRow {
            id: CategoryId::new_v4(),
            name: "Work".to_string(),
            depth: 0,
            is_reserved: false,
            is_exclusive: false,
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
            enable_implicit_string: false,
        };
        let when = CategoryListRow {
            id: CategoryId::new_v4(),
            name: "When".to_string(),
            depth: 0,
            is_reserved: true,
            is_exclusive: false,
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
            (Mode::ItemEditInput, "Edit> "),
            (Mode::NoteEditInput, "Note> "),
            (Mode::FilterInput, "Filter> "),
            (Mode::ViewCreateNameInput, "View create> "),
            (Mode::ViewRenameInput, "View rename> "),
            (Mode::CategoryCreateInput, "Category create> "),
            (Mode::CategoryRenameInput, "Category rename> "),
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
}
