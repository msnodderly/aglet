use std::collections::{HashMap, HashSet};
use std::io;
use std::path::Path;

use agenda_core::agenda::Agenda;
use agenda_core::matcher::{unknown_hashtag_tokens, SubstringClassifier};
use agenda_core::model::{
    Category, CategoryId, Column, ColumnKind, Item, ItemId, Query, Section, View, WhenBucket,
};
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
use ratatui::widgets::{
    Block, Borders, Cell, Clear, List, ListItem, ListState, Paragraph, Row, Scrollbar,
    ScrollbarOrientation, ScrollbarState, Table, TableState, Tabs, Wrap,
};
use ratatui::Terminal;

mod app;
mod input;
mod modes;
mod render;
mod text_buffer;
mod ui_support;

use ui_support::*;

type TuiTerminal = Terminal<CrosstermBackend<io::Stdout>>;

struct TerminalSession {
    terminal: TuiTerminal,
    active: bool,
}

impl TerminalSession {
    fn enter() -> Result<Self, String> {
        enable_raw_mode().map_err(|e| e.to_string())?;

        let mut stdout = io::stdout();
        if let Err(err) = execute!(stdout, EnterAlternateScreen) {
            let _ = disable_raw_mode();
            return Err(err.to_string());
        }

        let backend = CrosstermBackend::new(stdout);
        let terminal = match Terminal::new(backend) {
            Ok(terminal) => terminal,
            Err(err) => {
                let _ = execute!(io::stdout(), LeaveAlternateScreen);
                let _ = disable_raw_mode();
                return Err(err.to_string());
            }
        };

        Ok(Self {
            terminal,
            active: true,
        })
    }

    fn terminal_mut(&mut self) -> &mut TuiTerminal {
        &mut self.terminal
    }

    fn exit(&mut self) -> Result<(), String> {
        if !self.active {
            return Ok(());
        }
        disable_raw_mode().map_err(|e| e.to_string())?;
        execute!(self.terminal.backend_mut(), LeaveAlternateScreen).map_err(|e| e.to_string())?;
        self.terminal.show_cursor().map_err(|e| e.to_string())?;
        self.active = false;
        Ok(())
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}

pub fn run(db_path: &Path) -> Result<(), String> {
    let store = Store::open(db_path).map_err(|e| e.to_string())?;
    let classifier = SubstringClassifier;
    let agenda = Agenda::new(&store, &classifier);

    let mut terminal = TerminalSession::enter()?;

    let mut app = App::default();
    let result = app.run(terminal.terminal_mut(), &agenda);

    terminal.exit()?;

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
    has_note: bool,
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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum DefinitionSubTab {
    Criteria,
    Columns,
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
    note: text_buffer::TextBuffer,
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
    input: text_buffer::TextBuffer,
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
    view_manager_definition_sub_tab: DefinitionSubTab,
    view_manager_column_index: usize,
    view_manager_column_picker_target: bool,
    view_manager_column_width_input: bool,
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
    item_edit_note: text_buffer::TextBuffer,
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
            input: text_buffer::TextBuffer::empty(),
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
            view_manager_definition_sub_tab: DefinitionSubTab::Criteria,
            view_manager_column_index: 0,
            view_manager_column_picker_target: false,
            view_manager_column_width_input: false,
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
            item_edit_note: text_buffer::TextBuffer::empty(),
            preview_provenance_scroll: 0,
            preview_summary_scroll: 0,
            inspect_assignment_index: 0,
            slots: Vec::new(),
            slot_index: 0,
            item_index: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        add_capture_status_message, board_column_widths, board_item_label, bucket_target_set_mut,
        build_category_rows, build_reparent_options, category_name_map, category_target_set_mut,
        compute_board_layout, first_non_reserved_category_index, item_assignment_labels,
        item_edit_popup_area, list_scroll_for_selected_line, next_index, next_index_clamped,
        should_render_unmatched_lane, text_buffer, truncate_board_cell, when_bucket_options, App,
        BucketEditTarget, CategoryEditTarget, CategoryListRow, Mode, ViewManagerPane,
    };
    use agenda_core::agenda::Agenda;
    use agenda_core::matcher::SubstringClassifier;
    use agenda_core::model::{
        Assignment, AssignmentSource, Category, CategoryId, Column, ColumnKind, Item, Query,
        Section, View, WhenBucket,
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
        project.note = Some("roadmap details".to_string());
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
        assert!(!done_row.has_note);

        let project_row = rows
            .iter()
            .find(|row| row.id == project.id)
            .expect("project row present");
        assert!(project_row.has_note);
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
            has_note: false,
            is_exclusive: false,
            is_actionable: false,
            enable_implicit_string: false,
        };
        let user = CategoryListRow {
            id: CategoryId::new_v4(),
            name: "Work".to_string(),
            depth: 0,
            is_reserved: false,
            has_note: false,
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
            has_note: false,
            is_exclusive: false,
            is_actionable: false,
            enable_implicit_string: false,
        };
        let when = CategoryListRow {
            id: CategoryId::new_v4(),
            name: "When".to_string(),
            depth: 0,
            is_reserved: true,
            has_note: false,
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
                input: text_buffer::TextBuffer::new(input.to_string()),
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
                input: text_buffer::TextBuffer::new("abc".to_string()),
                ..App::default()
            };
            assert_eq!(app.input_cursor_position(footer), None);
        }
    }

    #[test]
    fn input_cursor_position_clamps_to_footer_inner_width() {
        let footer = Rect::new(0, 0, 8, 3);
        // cursor clamps to text length (26), which overflows the 8-wide footer → x=6
        let app = App {
            mode: Mode::AddInput,
            input: text_buffer::TextBuffer::new("abcdefghijklmnopqrstuvwxyz".to_string()),
            ..App::default()
        };

        assert_eq!(app.input_cursor_position(footer), Some((6, 1)));
    }

    #[test]
    fn input_cursor_position_tracks_edit_cursor_not_just_input_end() {
        let footer = Rect::new(0, 0, 40, 3);
        let app = App {
            mode: Mode::AddInput,
            input: text_buffer::TextBuffer::with_cursor("abcd".to_string(), 2),
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
            input: text_buffer::TextBuffer::with_cursor("abcd".to_string(), 2),
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
    fn next_index_clamped_stops_at_edges() {
        assert_eq!(next_index_clamped(0, 3, -1), 0);
        assert_eq!(next_index_clamped(0, 3, 1), 1);
        assert_eq!(next_index_clamped(2, 3, 1), 2);
        assert_eq!(next_index_clamped(2, 3, -1), 1);
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
    fn view_manager_section_detail_returns_and_applies_draft_changes() {
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
            .expect("open section detail");
        assert_eq!(app.mode, Mode::ViewSectionDetail);
        app.handle_view_section_detail_key(KeyCode::Char('h'))
            .expect("toggle show_children");
        app.handle_view_section_detail_key(KeyCode::Esc)
            .expect("return to manager");
        assert_eq!(app.mode, Mode::ViewManagerScreen);

        let selected = app.views.get(app.picker_index).expect("selected view");
        assert_eq!(selected.sections.len(), 1);
        assert!(selected.sections[0].show_children);
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
    fn inspect_unassign_picker_jk_tracks_assignment_rows() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!("agenda-tui-unassign-nav-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let work = Category::new("Work".to_string());
        let home = Category::new("Home".to_string());
        store.create_category(&work).expect("create work");
        store.create_category(&home).expect("create home");
        let item = Item::new("demo item".to_string());
        store.create_item(&item).expect("create item");
        agenda
            .assign_item_manual(item.id, work.id, Some("manual:test".to_string()))
            .expect("assign work");
        agenda
            .assign_item_manual(item.id, home.id, Some("manual:test".to_string()))
            .expect("assign home");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.mode = Mode::Normal;
        app.show_preview = true;
        app.normal_focus = super::NormalFocus::Preview;
        app.preview_mode = super::PreviewMode::Provenance;

        app.handle_normal_key(KeyCode::Char('u'), &agenda)
            .expect("open unassign picker from preview provenance");
        assert_eq!(app.mode, Mode::InspectUnassignPicker);
        assert_eq!(app.inspect_assignment_index, 0);

        app.handle_inspect_unassign_key(KeyCode::Char('j'), &agenda)
            .expect("j moves to next assignment");
        assert_eq!(app.inspect_assignment_index, 1);

        app.handle_inspect_unassign_key(KeyCode::Char('j'), &agenda)
            .expect("j wraps around");
        assert_eq!(app.inspect_assignment_index, 0);

        app.handle_inspect_unassign_key(KeyCode::Char('k'), &agenda)
            .expect("k wraps backwards");
        assert_eq!(app.inspect_assignment_index, 1);

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn normal_mode_f_toggles_focus_when_preview_is_open() {
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

        app.handle_normal_key(KeyCode::Char('f'), &agenda)
            .expect("f focuses preview");
        assert_eq!(app.normal_focus, super::NormalFocus::Preview);

        app.handle_normal_key(KeyCode::Char('f'), &agenda)
            .expect("f focuses board");
        assert_eq!(app.normal_focus, super::NormalFocus::Board);

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn normal_mode_tab_and_backtab_switch_sections_without_wrapping() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!("agenda-tui-tab-sections-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let alpha = Category::new("Alpha".to_string());
        let beta = Category::new("Beta".to_string());
        store.create_category(&alpha).expect("create alpha");
        store.create_category(&beta).expect("create beta");

        let mut view = View::new("Board".to_string());
        let mut section_alpha = Section {
            title: "Alpha".to_string(),
            criteria: Query::default(),
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
        };
        section_alpha.criteria.include.insert(alpha.id);
        let mut section_beta = Section {
            title: "Beta".to_string(),
            criteria: Query::default(),
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
        };
        section_beta.criteria.include.insert(beta.id);
        view.sections.push(section_alpha);
        view.sections.push(section_beta);
        store.create_view(&view).expect("create board view");

        let item_alpha = Item::new("a".to_string());
        let item_beta = Item::new("b".to_string());
        store.create_item(&item_alpha).expect("create item alpha");
        store.create_item(&item_beta).expect("create item beta");
        agenda
            .assign_item_manual(item_alpha.id, alpha.id, Some("manual:test".to_string()))
            .expect("assign alpha");
        agenda
            .assign_item_manual(item_beta.id, beta.id, Some("manual:test".to_string()))
            .expect("assign beta");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.set_view_selection_by_name("Board");
        app.refresh(&store).expect("refresh board");
        app.mode = Mode::Normal;
        assert_eq!(app.slot_index, 0);

        app.handle_normal_key(KeyCode::Tab, &agenda)
            .expect("tab moves to next section");
        assert_eq!(app.slot_index, 1);
        app.handle_normal_key(KeyCode::Tab, &agenda)
            .expect("tab clamps at last section");
        assert_eq!(app.slot_index, 1);

        app.handle_normal_key(KeyCode::BackTab, &agenda)
            .expect("backtab moves to previous section");
        assert_eq!(app.slot_index, 0);
        app.handle_normal_key(KeyCode::BackTab, &agenda)
            .expect("backtab clamps at first section");
        assert_eq!(app.slot_index, 0);

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
        let mut app = App {
            item_edit_note: text_buffer::TextBuffer::with_cursor(
                "first\nsecond".to_string(),
                "first\nse".chars().count(),
            ),
            ..App::default()
        };

        app.handle_item_edit_note_input_key(KeyCode::Up);
        assert_eq!(app.item_edit_note.cursor(), "fi".chars().count());

        app.handle_item_edit_note_input_key(KeyCode::Down);
        assert_eq!(app.item_edit_note.cursor(), "first\nse".chars().count());
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
    fn category_reparent_picker_preselects_current_parent() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path =
            std::env::temp_dir().join(format!("agenda-tui-category-reparent-select-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let parent = Category::new("Parent".to_string());
        store.create_category(&parent).expect("create parent");
        let mut child = Category::new("Child".to_string());
        child.parent = Some(parent.id);
        store.create_category(&child).expect("create child");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.mode = Mode::CategoryManager;
        app.category_index = app
            .category_rows
            .iter()
            .position(|row| row.id == child.id)
            .expect("child category row should exist");

        app.handle_category_manager_key(KeyCode::Char('p'), &agenda)
            .expect("open reparent picker");
        assert_eq!(app.mode, Mode::CategoryReparentPicker);
        assert!(!app.category_reparent_options.is_empty());

        let selected_parent = app
            .category_reparent_options
            .get(app.category_reparent_index)
            .and_then(|option| option.parent_id);
        assert_eq!(selected_parent, Some(parent.id));

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
        let mut app = App {
            category_rows: vec![CategoryListRow {
                id: CategoryId::new_v4(),
                name: "Work".to_string(),
                depth: 0,
                is_reserved: false,
                has_note: false,
                is_exclusive: false,
                is_actionable: true,
                enable_implicit_string: true,
            }],
            ..App::default()
        };
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
        assert_eq!(app.input.cursor(), 1);

        assert!(app.handle_text_input_key(KeyCode::Char('b')));
        assert_eq!(app.input.text(), "abc");
        assert_eq!(app.input.cursor(), 2);

        assert!(app.handle_text_input_key(KeyCode::Backspace));
        assert_eq!(app.input.text(), "ac");
        assert_eq!(app.input.cursor(), 1);

        assert!(app.handle_text_input_key(KeyCode::Delete));
        assert_eq!(app.input.text(), "a");
        assert_eq!(app.input.cursor(), 1);
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
    fn board_layout_helpers_fit_columns_within_slot_width() {
        let parent = Category::new("Parent".to_string());
        let mut child = Category::new("Child".to_string());
        child.parent = Some(parent.id);
        let categories = vec![parent.clone(), child.clone()];
        let names = category_name_map(&categories);
        let columns = vec![
            Column {
                kind: ColumnKind::When,
                heading: parent.id,
                width: 24,
            },
            Column {
                kind: ColumnKind::Standard,
                heading: parent.id,
                width: 24,
            },
        ];

        let slot_width = 64u16;
        let dynamic = compute_board_layout(&columns, &categories, &names, "Item", slot_width);
        let dynamic_used = dynamic.marker
            + dynamic.note
            + dynamic.item
            + dynamic
                .columns
                .iter()
                .map(|column| column.width)
                .sum::<usize>();
        assert!(dynamic_used <= slot_width as usize);
        assert!(dynamic.item >= 1);
        assert!(dynamic.columns.iter().all(|column| column.width >= 8));

        let legacy = board_column_widths(slot_width);
        assert!(
            legacy.marker + legacy.note + legacy.when + legacy.item + legacy.categories
                <= slot_width as usize
        );
        assert!(legacy.item >= 1);
    }

    #[test]
    fn truncate_board_cell_uses_ellipsis_for_overflow() {
        assert_eq!(truncate_board_cell("abcdef", 5), "ab...");
        assert_eq!(truncate_board_cell("abcdef", 3), "...");
        assert_eq!(truncate_board_cell("abc", 5), "abc");
    }

    #[test]
    fn board_item_label_does_not_inline_note_marker() {
        let mut item = Item::new("alignment check".to_string());
        item.note = Some("detail".to_string());
        assert_eq!(board_item_label(&item), "alignment check");
    }
}
