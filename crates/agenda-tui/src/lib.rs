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
    ScrollbarOrientation, ScrollbarState, Table, TableState, Wrap,
};
use ratatui::Terminal;

mod app;
mod input;
mod input_panel;
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
        section_index: usize,
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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum CategoryEditTarget {
    ViewInclude,
    SectionCriteriaInclude,
    SectionCriteriaExclude,
    SectionColumns,
    SectionOnInsertAssign,
    SectionOnRemoveUnassign,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum BucketEditTarget {
    ViewVirtualInclude,
    ViewVirtualExclude,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    Normal,
    InputPanel,  // unified add/edit/name-input (replaces AddInput + ItemEdit)
    NoteEdit,
    ItemAssignPicker,
    ItemAssignInput,
    InspectUnassign,
    FilterInput,
    ViewPicker,
    ViewEdit,
    ViewCreateCategory,
    ViewDeleteConfirm,
    ConfirmDelete,
    CategoryManager,
    CategoryReparent,
    CategoryDelete,
    CategoryConfig,
}

/// Disambiguates which name-input operation is in flight when Mode::InputPanel
/// is open with InputPanelKind::NameInput.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum NameInputContext {
    ViewCreate,
    ViewRename,
    CategoryCreate,
    CategoryRename,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ViewEditRegion {
    Criteria,
    Sections,
    Unmatched,
}

impl ViewEditRegion {
    fn next(self) -> Self {
        match self {
            Self::Criteria => Self::Sections,
            Self::Sections => Self::Unmatched,
            Self::Unmatched => Self::Criteria,
        }
    }

    fn prev(self) -> Self {
        match self {
            Self::Criteria => Self::Unmatched,
            Self::Sections => Self::Criteria,
            Self::Unmatched => Self::Sections,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
enum ViewEditOverlay {
    CategoryPicker { target: CategoryEditTarget },
    BucketPicker { target: BucketEditTarget },
}

#[derive(Clone, PartialEq, Eq, Debug)]
enum ViewEditInlineInput {
    SectionTitle { section_index: usize },
    UnmatchedLabel,
}

#[derive(Clone)]
struct ViewEditState {
    draft: View,
    region: ViewEditRegion,
    criteria_index: usize,
    section_index: usize,
    section_expanded: Option<usize>,
    overlay: Option<ViewEditOverlay>,
    inline_input: Option<ViewEditInlineInput>,
    inline_buf: text_buffer::TextBuffer,
    picker_index: usize,
    preview_count: usize,
    criteria_rows: Vec<ViewCriteriaRow>,
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
struct CategoryConfigState {
    category_id: CategoryId,
    category_name: String,
    is_exclusive: bool,
    is_actionable: bool,
    enable_implicit_string: bool,
    note: text_buffer::TextBuffer,
    focus: CategoryConfigFocus,
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
    section_filters: Vec<Option<String>>,
    filter_target_section: usize,
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
    view_create_include_selection: HashSet<CategoryId>,
    view_create_exclude_selection: HashSet<CategoryId>,
    view_edit_state: Option<ViewEditState>,

    categories: Vec<Category>,
    category_rows: Vec<CategoryListRow>,
    category_index: usize,
    category_create_parent: Option<CategoryId>,
    category_reparent_options: Vec<ReparentOptionRow>,
    category_reparent_index: usize,
    category_config_editor: Option<CategoryConfigState>,
    item_assign_category_index: usize,
    input_panel: Option<input_panel::InputPanel>,
    name_input_context: Option<NameInputContext>,
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
            section_filters: Vec::new(),
            filter_target_section: 0,
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
            view_create_include_selection: HashSet::new(),
            view_create_exclude_selection: HashSet::new(),
            view_edit_state: None,
            categories: Vec::new(),
            category_rows: Vec::new(),
            category_index: 0,
            category_create_parent: None,
            category_reparent_options: Vec::new(),
            category_reparent_index: 0,
            category_config_editor: None,
            item_assign_category_index: 0,
            input_panel: None,
            name_input_context: None,
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
        build_category_rows, build_reparent_options, category_name_map, compute_board_layout,
        first_non_reserved_category_index, input_panel, input_panel_popup_area,
        item_assignment_labels, list_scroll_for_selected_line, next_index, next_index_clamped,
        should_render_unmatched_lane, text_buffer, truncate_board_cell, when_bucket_options, App,
        BucketEditTarget, CategoryListRow, Mode, NameInputContext, ViewEditRegion,
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
        // InputPanel (add/edit) uses a popup cursor, not the footer cursor.
        // Footer cursor applies to the remaining text-in-footer modes.
        let cases = [
            (Mode::NoteEdit, "Note> "),
            (Mode::FilterInput, "Filter> "),
            (Mode::ItemAssignInput, "Category> "),
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
                Some((expected_x, footer.y + 1)),
                "mode {:?}",
                mode,
            );
        }
    }

    #[test]
    fn input_cursor_position_is_hidden_for_non_input_modes() {
        let footer = Rect::new(10, 5, 40, 3);
        for mode in [
            Mode::Normal,
            Mode::InputPanel, // popup mode — footer cursor hidden
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
            assert_eq!(app.input_cursor_position(footer), None, "mode {:?}", mode);
        }
    }

    #[test]
    fn input_cursor_position_clamps_to_footer_inner_width() {
        let footer = Rect::new(0, 0, 8, 3);
        // cursor clamps to text length (26), which overflows the 8-wide footer → x=6
        // NoteEdit still uses footer text input
        let app = App {
            mode: Mode::NoteEdit,
            input: text_buffer::TextBuffer::new("abcdefghijklmnopqrstuvwxyz".to_string()),
            ..App::default()
        };
        assert_eq!(app.input_cursor_position(footer), Some((6, 1)));
    }

    #[test]
    fn input_cursor_position_tracks_edit_cursor_not_just_input_end() {
        let footer = Rect::new(0, 0, 40, 3);
        // NoteEdit prefix = "Note> " (6 chars); inner_x=1; cursor=2 → 1+6+2=9
        let app = App {
            mode: Mode::NoteEdit,
            input: text_buffer::TextBuffer::with_cursor("abcd".to_string(), 2),
            ..App::default()
        };
        assert_eq!(app.input_cursor_position(footer), Some((9, 1)));
    }

    #[test]
    fn input_panel_cursor_position_uses_popup_area() {
        let screen = Rect::new(0, 0, 120, 40);
        let popup = input_panel_popup_area(screen);
        let panel = input_panel::InputPanel::new_edit_item(
            agenda_core::model::ItemId::new_v4(),
            "abcd".to_string(),
            String::new(),
            Default::default(),
        );
        let app = App {
            mode: Mode::InputPanel,
            input_panel: Some(panel),
            ..App::default()
        };
        // Cursor should be positioned in the text row of the popup, after the "  Text> " prefix
        // with 2 chars of cursor offset (TextBuffer::new puts cursor at end; we need with_cursor)
        // Just assert it's Some and within the popup bounds.
        let pos = if let Some(panel) = &app.input_panel {
            app.input_panel_cursor_position(popup, panel)
        } else {
            None
        };
        assert!(pos.is_some(), "expected cursor position in popup");
        let (cx, cy) = pos.unwrap();
        assert!(cx >= popup.x, "cursor x {} should be >= popup.x {}", cx, popup.x);
        assert!(cy >= popup.y, "cursor y {} should be >= popup.y {}", cy, popup.y);
        assert!(cx < popup.x + popup.width, "cursor x in bounds");
        assert!(cy < popup.y + popup.height, "cursor y in bounds");
    }

    #[test]
    fn input_panel_edit_tab_switches_to_note_and_saves() {
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

        // 'e' opens InputPanel(EditItem)
        app.handle_normal_key(KeyCode::Char('e'), &agenda)
            .expect("open item edit");
        assert_eq!(app.mode, Mode::InputPanel);
        assert_eq!(
            app.input_panel.as_ref().unwrap().focus,
            input_panel::InputPanelFocus::Text
        );

        // Tab moves focus to Note
        app.handle_input_panel_key(KeyCode::Tab, &agenda)
            .expect("switch to note");
        assert_eq!(
            app.input_panel.as_ref().unwrap().focus,
            input_panel::InputPanelFocus::Note
        );

        // Type in note field (appends to existing "old")
        for c in " updated".chars() {
            app.handle_input_panel_key(KeyCode::Char(c), &agenda)
                .expect("type note");
        }
        // Tab → CategoriesButton, Tab → SaveButton
        app.handle_input_panel_key(KeyCode::Tab, &agenda)
            .expect("focus categories button");
        app.handle_input_panel_key(KeyCode::Tab, &agenda)
            .expect("focus save button");
        assert_eq!(
            app.input_panel.as_ref().unwrap().focus,
            input_panel::InputPanelFocus::SaveButton
        );
        // Enter on SaveButton saves
        app.handle_input_panel_key(KeyCode::Enter, &agenda)
            .expect("save item edit");
        assert_eq!(app.mode, Mode::Normal);

        let saved = store.get_item(item.id).expect("load item");
        assert_eq!(saved.note.as_deref(), Some("old updated"));

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn input_panel_edit_enter_in_note_inserts_newline() {
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

        // Enter opens InputPanel(EditItem)
        app.handle_normal_key(KeyCode::Enter, &agenda)
            .expect("enter opens edit");
        assert_eq!(app.mode, Mode::InputPanel);

        // Tab to Note field
        app.handle_input_panel_key(KeyCode::Tab, &agenda)
            .expect("focus note");
        assert_eq!(
            app.input_panel.as_ref().unwrap().focus,
            input_panel::InputPanelFocus::Note
        );

        // Enter in Note inserts newline
        app.handle_input_panel_key(KeyCode::Enter, &agenda)
            .expect("enter adds newline");
        for c in "line2".chars() {
            app.handle_input_panel_key(KeyCode::Char(c), &agenda)
                .expect("type note line2");
        }
        // Tab → Categories, Tab → Save, Enter → save
        app.handle_input_panel_key(KeyCode::Tab, &agenda)
            .expect("focus categories");
        app.handle_input_panel_key(KeyCode::Tab, &agenda)
            .expect("focus save");
        app.handle_input_panel_key(KeyCode::Enter, &agenda)
            .expect("save");
        assert_eq!(app.mode, Mode::Normal);

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
    fn view_picker_v_opens_view_edit() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!("agenda-tui-view-edit-open-{nanos}.ag"));
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
            .expect("open view edit");

        assert_eq!(app.mode, Mode::ViewEdit);
        assert!(app.view_edit_state.is_some());

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn view_picker_lowercase_n_opens_view_create() {
        let (store, db_path) = make_test_store_with_view("picker-lower-n");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        app.mode = Mode::ViewPicker;

        app.handle_view_picker_key(KeyCode::Char('n'), &agenda)
            .expect("n opens create view");

        // After Phase 5d: 'n' in ViewPicker now opens InputPanel(NameInput) instead of ViewCreateName
        assert_eq!(app.mode, Mode::InputPanel);
        assert_eq!(app.name_input_context, Some(NameInputContext::ViewCreate));

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn view_edit_navigation_keys_do_not_request_app_quit() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!("agenda-tui-view-edit-nav-{nanos}.ag"));
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
            .expect("open view edit");

        let should_quit = app
            .handle_key(KeyCode::Down, &agenda)
            .expect("down in view edit");
        assert!(!should_quit, "view-edit navigation must not quit the app");
        assert_eq!(app.mode, Mode::ViewEdit);

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
        assert_eq!(app.mode, Mode::ItemAssignPicker);

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
        assert_eq!(app.mode, Mode::InspectUnassign);

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
        assert_eq!(app.mode, Mode::InspectUnassign);
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
            columns: Vec::new(),
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
        };
        section_alpha.criteria.include.insert(alpha.id);
        let mut section_beta = Section {
            title: "Beta".to_string(),
            criteria: Query::default(),
            columns: Vec::new(),
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
    fn input_panel_note_up_down_moves_cursor_between_lines() {
        let mut panel = input_panel::InputPanel::new_edit_item(
            agenda_core::model::ItemId::new_v4(),
            "hello".to_string(),
            String::new(),
            Default::default(),
        );
        // Set note buffer with multiline content and cursor mid-second-line.
        panel.note = text_buffer::TextBuffer::with_cursor(
            "first\nsecond".to_string(),
            "first\nse".chars().count(),
        );
        panel.focus = input_panel::InputPanelFocus::Note;

        panel.handle_key(KeyCode::Up);
        assert_eq!(panel.note.cursor(), "fi".chars().count());

        panel.handle_key(KeyCode::Down);
        assert_eq!(panel.note.cursor(), "first\nse".chars().count());
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

        assert_eq!(app.mode, Mode::CategoryConfig);
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
        assert_eq!(app.mode, Mode::CategoryConfig);

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
        assert_eq!(app.mode, Mode::CategoryReparent);
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
        app.mode = Mode::ViewCreateCategory;
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
    fn bucket_target_set_mut_supports_view_targets() {
        let mut view = View::new("Board".to_string());
        view.sections.push(Section {
            title: "One".to_string(),
            criteria: Query::default(),
            columns: Vec::new(),
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
        });

        let view_virtual = bucket_target_set_mut(&mut view, BucketEditTarget::ViewVirtualInclude)
            .expect("view virtual include set");
        view_virtual.insert(WhenBucket::Today);
        assert!(view.criteria.virtual_include.contains(&WhenBucket::Today));

        let view_virtual_exclude =
            bucket_target_set_mut(&mut view, BucketEditTarget::ViewVirtualExclude)
                .expect("view virtual exclude set");
        view_virtual_exclude.insert(WhenBucket::NoDate);
        assert!(view.criteria.virtual_exclude.contains(&WhenBucket::NoDate));
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

    // -------------------------------------------------------------------------
    // Phase 2a: ViewEdit tests
    // -------------------------------------------------------------------------

    #[test]
    fn store_section_roundtrip_smoke() {
        // Minimal test: can the store persist and reload a Section?
        let (store, db_path) = make_test_store_with_view("roundtrip");
        let view = store
            .list_views()
            .expect("list")
            .into_iter()
            .next()
            .expect("view");
        let mut updated = view.clone();
        updated.sections.push(Section {
            title: "Roundtrip".to_string(),
            criteria: Query::default(),
            columns: Vec::new(),
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
        });
        store.update_view(&updated).expect("update_view");
        let saved = store
            .list_views()
            .expect("list")
            .into_iter()
            .next()
            .expect("view");
        assert_eq!(
            saved.sections.len(),
            1,
            "store section roundtrip should work"
        );
        let _ = std::fs::remove_file(&db_path);
    }

    fn make_test_store_with_view(suffix: &str) -> (Store, std::path::PathBuf) {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path =
            std::env::temp_dir().join(format!("agenda-tui-view-edit-{suffix}-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        store
            .create_view(&View::new("TestView".to_string()))
            .expect("create view");
        (store, db_path)
    }

    #[test]
    fn view_picker_e_opens_view_edit() {
        let (store, db_path) = make_test_store_with_view("e-opens");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        app.mode = Mode::ViewPicker;
        app.picker_index = 0;

        app.handle_view_picker_key(KeyCode::Char('e'), &agenda)
            .expect("open view edit");

        assert_eq!(app.mode, Mode::ViewEdit);
        assert!(app.view_edit_state.is_some());

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn view_edit_tab_cycles_regions() {
        let (store, db_path) = make_test_store_with_view("tab-cycle");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        let view = app.views[0].clone();
        app.open_view_edit(view);

        assert_eq!(
            app.view_edit_state.as_ref().unwrap().region,
            ViewEditRegion::Criteria
        );

        app.handle_view_edit_key(KeyCode::Tab, &agenda)
            .expect("tab");
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().region,
            ViewEditRegion::Sections
        );

        app.handle_view_edit_key(KeyCode::Tab, &agenda)
            .expect("tab");
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().region,
            ViewEditRegion::Unmatched
        );

        app.handle_view_edit_key(KeyCode::Tab, &agenda)
            .expect("tab wraps");
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().region,
            ViewEditRegion::Criteria
        );

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn view_edit_shift_tab_cycles_regions_backwards() {
        let (store, db_path) = make_test_store_with_view("shift-tab-cycle");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        let view = app.views[0].clone();
        app.open_view_edit(view);

        assert_eq!(
            app.view_edit_state.as_ref().unwrap().region,
            ViewEditRegion::Criteria
        );
        app.handle_view_edit_key(KeyCode::BackTab, &agenda)
            .expect("shift-tab");
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().region,
            ViewEditRegion::Unmatched
        );

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn view_edit_esc_returns_to_view_picker() {
        let (store, db_path) = make_test_store_with_view("esc-returns");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        let view = app.views[0].clone();
        app.open_view_edit(view);

        app.handle_view_edit_key(KeyCode::Esc, &agenda)
            .expect("esc");

        assert_eq!(app.mode, Mode::ViewPicker);
        assert!(app.view_edit_state.is_none());

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn view_edit_inline_input_intercepts_keys_before_region() {
        let (store, db_path) = make_test_store_with_view("inline-precedence");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        let mut view = app.views[0].clone();
        view.sections.push(Section {
            title: "Old Title".to_string(),
            criteria: Query::default(),
            columns: Vec::new(),
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
        });
        app.open_view_edit(view);

        // Move to Sections region
        app.handle_view_edit_key(KeyCode::Tab, &agenda)
            .expect("tab");
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().region,
            ViewEditRegion::Sections
        );

        // Press 't' to start inline edit
        app.handle_view_edit_key(KeyCode::Char('t'), &agenda)
            .expect("t");
        assert!(app.view_edit_state.as_ref().unwrap().inline_input.is_some());

        // Tab should go into inline buf, NOT cycle regions
        app.handle_view_edit_key(KeyCode::Char('X'), &agenda)
            .expect("type");
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().region,
            ViewEditRegion::Sections
        );

        // Esc cancels inline input, stays in ViewEdit
        app.handle_view_edit_key(KeyCode::Esc, &agenda)
            .expect("esc inline");
        assert_eq!(app.mode, Mode::ViewEdit);
        assert!(app.view_edit_state.as_ref().unwrap().inline_input.is_none());

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn view_edit_overlay_intercepts_keys_before_region() {
        let (store, db_path) = make_test_store_with_view("overlay-precedence");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let category = Category::new("Work".to_string());
        store.create_category(&category).expect("create category");

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        let view = app.views[0].clone();
        app.open_view_edit(view);

        // Press 'N' to open overlay
        app.handle_view_edit_key(KeyCode::Char('N'), &agenda)
            .expect("N");
        assert!(app.view_edit_state.as_ref().unwrap().overlay.is_some());

        // Tab should not cycle regions while overlay is open
        let region_before = app.view_edit_state.as_ref().unwrap().region;
        app.handle_view_edit_key(KeyCode::Tab, &agenda)
            .expect("tab during overlay");
        // Tab in category picker is not handled, overlay stays open
        assert!(app.view_edit_state.as_ref().unwrap().overlay.is_some());
        assert_eq!(app.view_edit_state.as_ref().unwrap().region, region_before);

        // Esc closes overlay, stays in ViewEdit
        app.handle_view_edit_key(KeyCode::Esc, &agenda)
            .expect("esc overlay");
        assert_eq!(app.mode, Mode::ViewEdit);
        assert!(app.view_edit_state.as_ref().unwrap().overlay.is_none());

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn view_edit_category_picker_allows_multi_select_with_enter() {
        let (store, db_path) = make_test_store_with_view("picker-multi");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let work = Category::new("Work".to_string());
        let home = Category::new("Home".to_string());
        store.create_category(&work).expect("create work category");
        store.create_category(&home).expect("create home category");

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        let view = app
            .views
            .iter()
            .find(|v| v.name == "TestView")
            .cloned()
            .expect("TestView should exist");
        app.open_view_edit(view);

        app.handle_view_edit_key(KeyCode::Char('n'), &agenda)
            .expect("open category picker");
        assert!(app.view_edit_state.as_ref().unwrap().overlay.is_some());

        let work_idx = app
            .category_rows
            .iter()
            .position(|r| r.name == "Work")
            .expect("work row");
        let home_idx = app
            .category_rows
            .iter()
            .position(|r| r.name == "Home")
            .expect("home row");

        if let Some(state) = &mut app.view_edit_state {
            state.picker_index = work_idx;
        }
        app.handle_view_edit_key(KeyCode::Enter, &agenda)
            .expect("toggle work");
        assert!(app.view_edit_state.as_ref().unwrap().overlay.is_some());
        assert!(app
            .view_edit_state
            .as_ref()
            .unwrap()
            .draft
            .criteria
            .include
            .contains(&work.id));

        if let Some(state) = &mut app.view_edit_state {
            state.picker_index = home_idx;
        }
        app.handle_view_edit_key(KeyCode::Enter, &agenda)
            .expect("toggle home");
        assert!(app
            .view_edit_state
            .as_ref()
            .unwrap()
            .draft
            .criteria
            .include
            .contains(&home.id));
        assert!(app.view_edit_state.as_ref().unwrap().overlay.is_some());

        app.handle_view_edit_key(KeyCode::Esc, &agenda)
            .expect("close category picker");
        assert!(app.view_edit_state.as_ref().unwrap().overlay.is_none());
        assert_eq!(app.mode, Mode::ViewEdit);

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn view_edit_section_plus_opens_criteria_picker_without_pre_expand() {
        let (store, db_path) = make_test_store_with_view("section-plus");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let work = Category::new("Work".to_string());
        store.create_category(&work).expect("create work category");

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        let view = app
            .views
            .iter()
            .find(|v| v.name == "TestView")
            .cloned()
            .expect("TestView should exist");
        app.open_view_edit(view);

        app.handle_view_edit_key(KeyCode::Tab, &agenda)
            .expect("tab");
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().region,
            ViewEditRegion::Sections
        );

        app.handle_view_edit_key(KeyCode::Char('n'), &agenda)
            .expect("add section");
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().draft.sections.len(),
            1
        );
        assert_eq!(app.view_edit_state.as_ref().unwrap().section_expanded, None);

        app.handle_view_edit_key(KeyCode::Char('+'), &agenda)
            .expect("open section include picker");
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().section_expanded,
            Some(0)
        );
        assert!(matches!(
            app.view_edit_state.as_ref().unwrap().overlay,
            Some(super::ViewEditOverlay::CategoryPicker {
                target: super::CategoryEditTarget::SectionCriteriaInclude
            })
        ));

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn view_edit_section_e_starts_section_title_edit() {
        let (store, db_path) = make_test_store_with_view("section-e-rename");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        let view = app
            .views
            .iter()
            .find(|v| v.name == "TestView")
            .cloned()
            .expect("TestView should exist");
        app.open_view_edit(view);

        app.handle_view_edit_key(KeyCode::Tab, &agenda)
            .expect("tab");
        app.handle_view_edit_key(KeyCode::Char('n'), &agenda)
            .expect("add section");
        app.handle_view_edit_key(KeyCode::Char('e'), &agenda)
            .expect("start section title edit");

        assert!(matches!(
            app.view_edit_state.as_ref().unwrap().inline_input,
            Some(super::ViewEditInlineInput::SectionTitle { section_index: 0 })
        ));

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn view_edit_section_c_opens_columns_picker_and_toggles_column() {
        let (store, db_path) = make_test_store_with_view("section-c-columns");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let work = Category::new("Work".to_string());
        store.create_category(&work).expect("create work category");

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        let view = app
            .views
            .iter()
            .find(|v| v.name == "TestView")
            .cloned()
            .expect("TestView should exist");
        app.open_view_edit(view);

        app.handle_view_edit_key(KeyCode::Tab, &agenda)
            .expect("tab");
        app.handle_view_edit_key(KeyCode::Char('n'), &agenda)
            .expect("add section");

        app.handle_view_edit_key(KeyCode::Char('c'), &agenda)
            .expect("open section columns picker");
        assert!(matches!(
            app.view_edit_state.as_ref().unwrap().overlay,
            Some(super::ViewEditOverlay::CategoryPicker {
                target: super::CategoryEditTarget::SectionColumns
            })
        ));

        let work_idx = app
            .category_rows
            .iter()
            .position(|r| r.name == "Work")
            .expect("work row");
        if let Some(state) = &mut app.view_edit_state {
            state.picker_index = work_idx;
        }
        app.handle_view_edit_key(KeyCode::Enter, &agenda)
            .expect("toggle section column");
        assert!(app.view_edit_state.as_ref().unwrap().draft.sections[0]
            .columns
            .iter()
            .any(|column| column.heading == work.id));

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn view_edit_new_section_inherits_default_columns() {
        let (store, db_path) = make_test_store_with_view("section-inherit-columns");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let work = Category::new("Work".to_string());
        store.create_category(&work).expect("create work category");

        let mut view = store
            .list_views()
            .expect("list views")
            .into_iter()
            .find(|v| v.name == "TestView")
            .expect("TestView should exist");
        view.columns.push(Column {
            kind: ColumnKind::Standard,
            heading: work.id,
            width: 16,
        });
        store.update_view(&view).expect("update default columns");

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        let view = app
            .views
            .iter()
            .find(|v| v.name == "TestView")
            .cloned()
            .expect("TestView should exist");
        app.open_view_edit(view);

        app.handle_view_edit_key(KeyCode::Tab, &agenda)
            .expect("tab");
        app.handle_view_edit_key(KeyCode::Char('n'), &agenda)
            .expect("add section");

        let section_columns = &app.view_edit_state.as_ref().unwrap().draft.sections[0].columns;
        assert_eq!(section_columns.len(), 1);
        assert_eq!(section_columns[0].heading, work.id);
        assert_eq!(section_columns[0].width, 16);

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn view_edit_save_persists_view() {
        let (store, db_path) = make_test_store_with_view("save");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let category = Category::new("Work".to_string());
        store.create_category(&category).expect("create category");

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        let view = app
            .views
            .iter()
            .find(|v| v.name == "TestView")
            .cloned()
            .expect("TestView should exist");
        app.open_view_edit(view);

        // Add a section via 'N' in Sections region
        app.handle_view_edit_key(KeyCode::Tab, &agenda)
            .expect("tab");
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().region,
            ViewEditRegion::Sections
        );
        app.handle_view_edit_key(KeyCode::Char('N'), &agenda)
            .expect("N adds section");
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().draft.sections.len(),
            1
        );

        // Verify draft has section before save
        let draft_before_save = app.view_edit_state.as_ref().unwrap().draft.clone();
        assert_eq!(
            draft_before_save.sections.len(),
            1,
            "draft must have 1 section before save"
        );

        // Verify draft ID matches what's in the store
        let view_in_store = store
            .list_views()
            .expect("list before update")
            .into_iter()
            .find(|v| v.name == "TestView")
            .expect("view before update");
        assert_eq!(
            draft_before_save.id, view_in_store.id,
            "draft ID must match store ID"
        );

        // Directly verify update_view works
        agenda
            .store()
            .update_view(&draft_before_save)
            .expect("direct update_view should work");
        let after_direct = store
            .list_views()
            .expect("list")
            .into_iter()
            .find(|v| v.name == "TestView")
            .expect("view");
        assert_eq!(
            after_direct.sections.len(),
            1,
            "direct update_view should persist section"
        );

        // Save with S (save + exit)
        app.handle_view_edit_key(KeyCode::Char('S'), &agenda)
            .expect("S save");
        assert_eq!(app.mode, Mode::ViewPicker);
        assert!(app.view_edit_state.is_none());
        assert!(
            app.status.contains("Saved"),
            "save status should say Saved, got: {}",
            app.status
        );

        // Verify persisted
        let saved = store
            .list_views()
            .expect("list views")
            .into_iter()
            .find(|v| v.name == "TestView")
            .expect("saved view");
        assert_eq!(saved.sections.len(), 1);

        let _ = std::fs::remove_file(&db_path);
    }

    // ── Per-section filter tests (Phase 3) ─────────────────────────────────

    fn make_two_section_store(suffix: &str) -> (Store, std::path::PathBuf) {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let db_path = std::env::temp_dir()
            .join(format!("agenda-tui-section-filter-{suffix}-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        // Create two categories and two sections on the view
        let cat_a = Category::new("Work".to_string());
        let cat_b = Category::new("Personal".to_string());
        store.create_category(&cat_a).expect("cat_a");
        store.create_category(&cat_b).expect("cat_b");

        let mut section_work = Section {
            title: "Work Items".to_string(),
            criteria: Query::default(),
            columns: Vec::new(),
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
        };
        section_work.criteria.include.insert(cat_a.id);

        let mut section_personal = Section {
            title: "Personal Items".to_string(),
            criteria: Query::default(),
            columns: Vec::new(),
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
        };
        section_personal.criteria.include.insert(cat_b.id);

        let mut view = View::new("TestView".to_string());
        view.sections.push(section_work);
        view.sections.push(section_personal);
        store.create_view(&view).expect("create view");

        // Create items in each section
        let item_work_1 = Item::new("Fix timeout bug".to_string());
        let item_work_2 = Item::new("Write tests".to_string());
        let item_personal_1 = Item::new("Buy groceries".to_string());
        let item_personal_2 = Item::new("Fix bike".to_string());
        store.create_item(&item_work_1).expect("item w1");
        store.create_item(&item_work_2).expect("item w2");
        store.create_item(&item_personal_1).expect("item p1");
        store.create_item(&item_personal_2).expect("item p2");

        agenda
            .assign_item_manual(item_work_1.id, cat_a.id, None)
            .expect("assign w1");
        agenda
            .assign_item_manual(item_work_2.id, cat_a.id, None)
            .expect("assign w2");
        agenda
            .assign_item_manual(item_personal_1.id, cat_b.id, None)
            .expect("assign p1");
        agenda
            .assign_item_manual(item_personal_2.id, cat_b.id, None)
            .expect("assign p2");

        (store, db_path)
    }

    #[test]
    fn section_filters_are_independent() {
        let (store, db_path) = make_two_section_store("independent");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        app.set_view_selection_by_name("TestView");
        app.refresh(&store).expect("refresh TestView");

        // Two sections: Work Items (slot 0) and Personal Items (slot 1)
        assert_eq!(app.slots.len(), 2, "expected 2 sections");
        assert_eq!(app.slots[0].items.len(), 2, "Work has 2 items");
        assert_eq!(app.slots[1].items.len(), 2, "Personal has 2 items");

        // Filter slot 0 for "timeout"
        app.slot_index = 0;
        app.handle_normal_key(KeyCode::Char('/'), &agenda)
            .expect("open filter");
        assert_eq!(app.mode, Mode::FilterInput);
        assert_eq!(app.filter_target_section, 0);

        // Type "timeout"
        for ch in "timeout".chars() {
            app.handle_text_input_key(KeyCode::Char(ch));
        }
        app.handle_filter_key(KeyCode::Enter, &agenda)
            .expect("apply filter");

        // slot 0 now shows only 1 item, slot 1 is unaffected
        assert_eq!(app.slots[0].items.len(), 1, "Work filtered to 1 item");
        assert_eq!(app.slots[0].items[0].text, "Fix timeout bug");
        assert_eq!(app.slots[1].items.len(), 2, "Personal unaffected");

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn section_filter_cleared_by_esc_in_normal_mode() {
        let (store, db_path) = make_two_section_store("esc-clears");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        app.set_view_selection_by_name("TestView");
        app.refresh(&store).expect("refresh TestView");

        // Apply a filter to slot 0
        app.section_filters[0] = Some("fix".to_string());
        app.refresh(&store).expect("refresh after filter");
        assert_eq!(app.slots[0].items.len(), 1, "filtered to 1 item");

        // Esc in slot 0 clears its filter
        app.slot_index = 0;
        app.handle_normal_key(KeyCode::Esc, &agenda)
            .expect("esc clears filter");

        assert!(
            app.section_filters[0].is_none(),
            "slot 0 filter should be cleared"
        );
        assert_eq!(app.slots[0].items.len(), 2, "slot 0 shows all items again");
        assert_eq!(
            app.slots[1].items.len(),
            2,
            "slot 1 unaffected by slot 0 esc"
        );

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn filter_esc_in_filter_input_cancels_without_clearing() {
        let (store, db_path) = make_two_section_store("esc-cancel");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        app.set_view_selection_by_name("TestView");
        app.refresh(&store).expect("refresh TestView");

        // Pre-set a filter
        app.section_filters[0] = Some("fix".to_string());
        app.refresh(&store).expect("refresh after pre-filter");
        assert_eq!(app.slots[0].items.len(), 1);

        // Open FilterInput and cancel without entering anything new
        app.slot_index = 0;
        app.handle_normal_key(KeyCode::Char('/'), &agenda)
            .expect("open filter");
        assert_eq!(app.mode, Mode::FilterInput);

        app.handle_filter_key(KeyCode::Esc, &agenda)
            .expect("esc cancels");

        // Filter should be preserved (cancel doesn't clear)
        assert_eq!(app.mode, Mode::Normal);
        assert_eq!(
            app.section_filters[0],
            Some("fix".to_string()),
            "existing filter should be preserved after cancel"
        );

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn section_filters_reset_on_view_switch() {
        let (store, db_path) = make_two_section_store("view-switch");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        // Create a second view so we can switch
        store
            .create_view(&View::new("OtherView".to_string()))
            .expect("create second view");

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        app.set_view_selection_by_name("TestView");
        app.refresh(&store).expect("refresh TestView");

        // Apply filter to slot 0
        app.section_filters[0] = Some("fix".to_string());
        app.refresh(&store).expect("refresh with filter");
        assert_eq!(app.slots[0].items.len(), 1);

        // Switch view via cycle_view
        app.cycle_view(1, &agenda).expect("cycle view");

        // Filters should be reset
        assert!(
            app.section_filters.iter().all(|f| f.is_none()),
            "all filters should be cleared after view switch"
        );

        let _ = std::fs::remove_file(&db_path);
    }
}
