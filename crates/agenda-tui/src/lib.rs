use std::collections::{HashMap, HashSet};
use std::io;

use agenda_core::agenda::Agenda;
use agenda_core::matcher::SubstringClassifier;
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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    Normal,
    AddInput,
    ItemEditInput,
    NoteEditInput,
    FilterInput,
    ViewPicker,
    ConfirmDelete,
    CategoryManager,
    CategoryCreateInput,
    CategoryDeleteConfirm,
}

struct App {
    mode: Mode,
    status: String,
    input: String,
    filter: Option<String>,
    show_inspect: bool,

    views: Vec<View>,
    view_index: usize,
    picker_index: usize,

    categories: Vec<Category>,
    category_rows: Vec<CategoryListRow>,
    category_index: usize,
    category_create_parent: Option<CategoryId>,
    slots: Vec<Slot>,
    slot_index: usize,
    item_index: usize,
}

impl Default for App {
    fn default() -> Self {
        Self {
            mode: Mode::Normal,
            status: "Press n to add, F8 to switch views, F9 for categories, q to quit".to_string(),
            input: String::new(),
            filter: None,
            show_inspect: false,
            views: Vec::new(),
            view_index: 0,
            picker_index: 0,
            categories: Vec::new(),
            category_rows: Vec::new(),
            category_index: 0,
            category_create_parent: None,
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

            if self.handle_key(key.code, agenda)? {
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
            Mode::FilterInput => self.handle_filter_key(code, agenda),
            Mode::ViewPicker => self.handle_view_picker_key(code, agenda),
            Mode::ConfirmDelete => self.handle_confirm_delete_key(code, agenda),
            Mode::CategoryManager => self.handle_category_manager_key(code),
            Mode::CategoryCreateInput => self.handle_category_create_key(code, agenda),
            Mode::CategoryDeleteConfirm => self.handle_category_delete_key(code, agenda),
        }
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
                self.input.clear();
                self.status = "Add item: type text and press Enter".to_string();
            }
            KeyCode::Char('e') => {
                if let Some(item) = self.selected_item() {
                    let existing_text = item.text.clone();
                    self.mode = Mode::ItemEditInput;
                    self.input = existing_text;
                    self.status = "Edit item text: Enter to save, Esc to cancel".to_string();
                } else {
                    self.status = "No selected item to edit".to_string();
                }
            }
            KeyCode::Char('m') => {
                if let Some(item) = self.selected_item() {
                    let existing_note = item.note.clone().unwrap_or_default();
                    self.mode = Mode::NoteEditInput;
                    self.input = existing_note;
                    self.status =
                        "Edit note: Enter to save (empty clears), Esc to cancel".to_string();
                } else {
                    self.status = "No selected item to add/edit note".to_string();
                }
            }
            KeyCode::Char('/') => {
                self.mode = Mode::FilterInput;
                self.input = self.filter.clone().unwrap_or_default();
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
                self.status = "View picker: Enter to switch, Esc to cancel".to_string();
            }
            KeyCode::F(9) => {
                self.mode = Mode::CategoryManager;
                self.status =
                    "Category manager: n child, N root, x delete, Esc to close".to_string();
            }
            KeyCode::Char('i') => {
                self.show_inspect = !self.show_inspect;
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
                self.input.clear();
                self.status = "Add canceled".to_string();
            }
            KeyCode::Enter => {
                let text = self.input.trim();
                if !text.is_empty() {
                    let parsed_when =
                        self.create_item_in_current_context(agenda, text.to_string())?;
                    self.status = add_capture_status_message(parsed_when);
                }
                self.mode = Mode::Normal;
                self.input.clear();
            }
            KeyCode::Backspace => {
                self.input.pop();
            }
            KeyCode::Char(c) => self.input.push(c),
            _ => {}
        }
        Ok(false)
    }

    fn handle_item_edit_key(&mut self, code: KeyCode, agenda: &Agenda<'_>) -> Result<bool, String> {
        match code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.input.clear();
                self.status = "Edit canceled".to_string();
            }
            KeyCode::Enter => {
                let Some(item_id) = self.selected_item_id() else {
                    self.mode = Mode::Normal;
                    self.input.clear();
                    self.status = "Edit failed: no selected item".to_string();
                    return Ok(false);
                };

                let updated_text = self.input.trim().to_string();
                if updated_text.is_empty() {
                    self.mode = Mode::Normal;
                    self.input.clear();
                    self.status = "Edit canceled: text cannot be empty".to_string();
                    return Ok(false);
                }

                let mut item = agenda
                    .store()
                    .get_item(item_id)
                    .map_err(|e| e.to_string())?;
                if item.text == updated_text {
                    self.mode = Mode::Normal;
                    self.input.clear();
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
                self.input.clear();
                self.status = "Item text updated".to_string();
            }
            KeyCode::Backspace => {
                self.input.pop();
            }
            KeyCode::Char(c) => self.input.push(c),
            _ => {}
        }
        Ok(false)
    }

    fn handle_note_edit_key(&mut self, code: KeyCode, agenda: &Agenda<'_>) -> Result<bool, String> {
        match code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.input.clear();
                self.status = "Note edit canceled".to_string();
            }
            KeyCode::Enter => {
                let Some(item_id) = self.selected_item_id() else {
                    self.mode = Mode::Normal;
                    self.input.clear();
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
                    self.input.clear();
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
                self.input.clear();
                self.status = if item.note.is_some() {
                    "Note updated".to_string()
                } else {
                    "Note cleared".to_string()
                };
            }
            KeyCode::Backspace => {
                self.input.pop();
            }
            KeyCode::Char(c) => self.input.push(c),
            _ => {}
        }
        Ok(false)
    }

    fn handle_filter_key(&mut self, code: KeyCode, agenda: &Agenda<'_>) -> Result<bool, String> {
        match code {
            KeyCode::Esc => {
                self.mode = Mode::Normal;
                self.filter = None;
                self.input.clear();
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
                self.input.clear();
            }
            KeyCode::Backspace => {
                self.input.pop();
            }
            KeyCode::Char(c) => self.input.push(c),
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
                }
                self.mode = Mode::Normal;
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

    fn handle_category_manager_key(&mut self, code: KeyCode) -> Result<bool, String> {
        match code {
            KeyCode::Esc | KeyCode::F(9) => {
                self.mode = Mode::Normal;
                self.input.clear();
                self.category_create_parent = None;
                self.status = "Category manager closed".to_string();
            }
            KeyCode::Down | KeyCode::Char('j') => self.move_category_cursor(1),
            KeyCode::Up | KeyCode::Char('k') => self.move_category_cursor(-1),
            KeyCode::Char('n') => {
                self.mode = Mode::CategoryCreateInput;
                self.input.clear();
                self.category_create_parent = self.selected_category_id();
                let parent = self
                    .create_parent_name()
                    .unwrap_or_else(|| "(root)".to_string());
                self.status = format!("Create category under {parent}: type name and Enter");
            }
            KeyCode::Char('N') => {
                self.mode = Mode::CategoryCreateInput;
                self.input.clear();
                self.category_create_parent = None;
                self.status = "Create top-level category: type name and Enter".to_string();
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
                self.input.clear();
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
                self.input.clear();
                self.category_create_parent = None;
            }
            KeyCode::Backspace => {
                self.input.pop();
            }
            KeyCode::Char(c) => self.input.push(c),
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
        if self.views.is_empty() {
            return Err("No views found".to_string());
        }

        self.view_index = self.view_index.min(self.views.len().saturating_sub(1));
        self.categories = store.get_hierarchy().map_err(|e| e.to_string())?;
        self.category_rows = build_category_rows(&self.categories);
        self.category_index = self
            .category_index
            .min(self.category_rows.len().saturating_sub(1));
        let items = store.list_items().map_err(|e| e.to_string())?;

        let view = self
            .current_view()
            .cloned()
            .ok_or("No active view".to_string())?;
        let reference_date = Local::now().date_naive();
        let result = resolve_view(&view, &items, &self.categories, reference_date);

        let mut slots = Vec::new();
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
        frame.render_widget(footer, layout[2]);

        if self.mode == Mode::ViewPicker {
            self.render_view_picker(frame, centered_rect(60, 60, frame.area()));
        }
        if matches!(
            self.mode,
            Mode::CategoryManager | Mode::CategoryCreateInput | Mode::CategoryDeleteConfirm
        ) {
            self.render_category_manager(frame, centered_rect(72, 72, frame.area()));
        }
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

        let category_names = category_name_map(&self.categories);
        if let Some(item) = self.selected_item() {
            if item.assignments.is_empty() {
                lines.push(Line::from("(no assignments)"));
            } else {
                let mut entries: Vec<String> = item
                    .assignments
                    .iter()
                    .map(|(category_id, assignment)| {
                        let category_name = category_names
                            .get(category_id)
                            .cloned()
                            .unwrap_or_else(|| category_id.to_string());
                        let origin = assignment.origin.clone().unwrap_or_else(|| "-".to_string());
                        format!(
                            "{} | source={:?} | origin={}",
                            category_name, assignment.source, origin
                        )
                    })
                    .collect();
                entries.sort_by_key(|entry| entry.to_ascii_lowercase());
                lines.extend(entries.into_iter().map(Line::from));
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
            Mode::CategoryCreateInput => format!("Category create> {}", self.input),
            Mode::CategoryDeleteConfirm => "Delete selected category? y/n".to_string(),
            _ => self.status.clone(),
        };
        let footer_title = match self.mode {
            Mode::CategoryManager => {
                "j/k:select  n:new-child  N:new-root  x:delete  Esc/F9:close"
            }
            Mode::CategoryCreateInput => "Type category name, Enter:create, Esc:cancel",
            Mode::CategoryDeleteConfirm => "y:confirm delete  n:cancel",
            Mode::ViewPicker => "j/k:select  Enter:switch  Esc:cancel",
            Mode::ItemEditInput => "Edit selected item text, Enter:save, Esc:cancel",
            Mode::NoteEditInput => "Edit selected note, Enter:save (empty clears), Esc:cancel",
            _ => {
                "n:add  e:edit  m:note  [/]:filter  F8:views  F9:categories  []:move  r:remove  d:done  x:delete  i:inspect  q:quit"
            }
        };

        Paragraph::new(prompt).block(Block::default().title(footer_title).borders(Borders::ALL))
    }

    fn render_view_picker(&self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        frame.render_widget(Clear, area);

        let lines: Vec<Line<'_>> = self
            .views
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
            .collect();

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

    fn render_category_manager(&self, frame: &mut ratatui::Frame<'_>, area: Rect) {
        frame.render_widget(Clear, area);

        let mut lines = vec![Line::from(
            "Categories are global; create under selection with n or at root with N.",
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

fn add_capture_status_message(parsed_when: Option<NaiveDateTime>) -> String {
    match parsed_when {
        Some(when) => format!("Item added (parsed when: {when})"),
        None => "Item added".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{add_capture_status_message, build_category_rows};
    use agenda_core::model::{Category, CategoryId};
    use chrono::NaiveDate;

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
            add_capture_status_message(Some(when)),
            "Item added (parsed when: 2026-02-24 15:00:00)"
        );
    }

    #[test]
    fn add_capture_status_message_defaults_when_no_datetime() {
        assert_eq!(add_capture_status_message(None), "Item added");
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
}
