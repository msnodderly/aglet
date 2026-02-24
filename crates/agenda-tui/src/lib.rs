use std::collections::{HashMap, HashSet};
use std::io;
use std::path::Path;

use agenda_core::agenda::Agenda;
use agenda_core::matcher::{unknown_hashtag_tokens, SubstringClassifier};
use agenda_core::model::{
    BoardDisplayMode, Category, CategoryId, Column, ColumnKind, CriterionMode, Item, ItemId, Query,
    Section, View, WhenBucket,
};
use agenda_core::query::{evaluate_query, resolve_view};
use agenda_core::store::Store;
use chrono::{Local, NaiveDateTime, Utc};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
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
    ViewCriteria,
    SectionCriteria,
    SectionColumns,
    SectionOnInsertAssign,
    SectionOnRemoveUnassign,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum BucketEditTarget {
    ViewVirtualInclude,
    ViewVirtualExclude,
}

#[derive(Clone, PartialEq, Eq, Debug)]
enum Mode {
    Normal,
    InputPanel, // unified add/edit/name-input (replaces AddInput + ItemEdit)
    NoteEdit,
    ItemAssignPicker,
    ItemAssignInput,
    InspectUnassign,
    FilterInput,
    ViewPicker,
    ViewEdit,
    ViewDeleteConfirm,
    ConfirmDelete,
    BoardColumnDeleteConfirm,
    CategoryManager,
    CategoryDirectEdit,
    CategoryColumnPicker,
    BoardAddColumnPicker,
    CategoryCreateConfirm { name: String, parent_id: CategoryId },
}

/// Disambiguates which name-input operation is in flight when Mode::InputPanel
/// is open with InputPanelKind::NameInput.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum NameInputContext {
    ViewCreate,
    ViewRename,
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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ViewEditPaneFocus {
    Sections,
    Details,
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
    pane_focus: ViewEditPaneFocus,
    criteria_index: usize,
    unmatched_field_index: usize,
    section_index: usize,
    sections_view_row_selected: bool,
    section_details_field_index: usize,
    section_expanded: Option<usize>,
    overlay: Option<ViewEditOverlay>,
    inline_input: Option<ViewEditInlineInput>,
    inline_buf: text_buffer::TextBuffer,
    picker_index: usize,
    preview_count: usize,
    dirty: bool,
    discard_confirm: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum CategoryManagerFocus {
    Tree,
    Filter,
    Details,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum CategoryParentPickerFocus {
    Filter,
    List,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum CategoryManagerDetailsFocus {
    Exclusive,
    MatchName,
    Actionable,
    Note,
}

impl CategoryManagerDetailsFocus {
    fn next(self) -> Self {
        match self {
            Self::Exclusive => Self::MatchName,
            Self::MatchName => Self::Actionable,
            Self::Actionable => Self::Note,
            Self::Note => Self::Exclusive,
        }
    }

    fn prev(self) -> Self {
        match self {
            Self::Exclusive => Self::Note,
            Self::MatchName => Self::Exclusive,
            Self::Actionable => Self::MatchName,
            Self::Note => Self::Actionable,
        }
    }
}

#[derive(Clone)]
enum CategoryInlineAction {
    Create {
        parent_id: Option<CategoryId>,
        buf: text_buffer::TextBuffer,
        confirm_name: Option<String>,
    },
    Rename {
        category_id: CategoryId,
        original_name: String,
        buf: text_buffer::TextBuffer,
    },
    DeleteConfirm {
        category_id: CategoryId,
        category_name: String,
    },
    ParentPicker {
        target_category_id: CategoryId,
        target_category_name: String,
        filter: text_buffer::TextBuffer,
        filter_editing: bool,
        options: Vec<ReparentOptionRow>,
        visible_option_indices: Vec<usize>,
        list_index: usize,
        focus: CategoryParentPickerFocus,
    },
}

#[derive(Clone)]
struct CategoryManagerState {
    focus: CategoryManagerFocus,
    filter: text_buffer::TextBuffer,
    filter_editing: bool,
    details_focus: CategoryManagerDetailsFocus,
    details_note_category_id: Option<CategoryId>,
    details_note: text_buffer::TextBuffer,
    details_note_dirty: bool,
    details_note_editing: bool,
    tree_index: usize,
    visible_row_indices: Vec<usize>,
    selected_category_id: Option<CategoryId>,
    inline_action: Option<CategoryInlineAction>,
}

#[derive(Clone, Debug)]
struct CategorySuggestState {
    suggest_index: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum AddColumnDirection {
    Left,
    Right,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum NormalModePrefix {
    G,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct BoardAddColumnAnchor {
    slot_index: usize,
    section_index: usize,
    current_board_column_index: usize,
    current_section_column_index: usize,
    item_column_index_before: usize,
    insert_index: usize,
    direction: AddColumnDirection,
    is_generated_section: bool,
}

#[derive(Clone)]
struct BoardAddColumnState {
    anchor: BoardAddColumnAnchor,
    input: text_buffer::TextBuffer,
    suggest_index: usize,
    create_confirm_name: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum CategoryDirectEditFocus {
    Entries,
    Input,
    Suggestions,
}

impl CategoryDirectEditFocus {
    fn next(self) -> Self {
        match self {
            Self::Entries => Self::Input,
            Self::Input => Self::Suggestions,
            Self::Suggestions => Self::Entries,
        }
    }

    fn prev(self) -> Self {
        match self {
            Self::Entries => Self::Suggestions,
            Self::Input => Self::Entries,
            Self::Suggestions => Self::Input,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct CategoryDirectEditAnchor {
    slot_index: usize,
    section_index: usize,
    section_column_index: usize,
    board_column_index: usize,
    is_generated_section: bool,
}

#[derive(Clone, PartialEq, Eq, Debug)]
struct CategoryDirectEditColumnMeta {
    parent_id: CategoryId,
    parent_name: String,
    column_kind: ColumnKind,
    anchor: CategoryDirectEditAnchor,
    item_id: ItemId,
    item_label: String,
}

#[derive(Clone)]
struct CategoryDirectEditRow {
    input: text_buffer::TextBuffer,
    category_id: Option<CategoryId>,
}

#[derive(Clone)]
struct CategoryDirectEditState {
    anchor: CategoryDirectEditAnchor,
    parent_id: CategoryId,
    parent_name: String,
    item_id: ItemId,
    item_label: String,
    rows: Vec<CategoryDirectEditRow>,
    active_row: usize,
    focus: CategoryDirectEditFocus,
    suggest_index: usize,
    create_confirm_name: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum CategoryColumnPickerFocus {
    FilterInput,
    List,
}

#[derive(Clone)]
struct CategoryColumnPickerState {
    anchor: CategoryDirectEditAnchor,
    parent_id: CategoryId,
    parent_name: String,
    item_id: ItemId,
    item_label: String,
    is_exclusive: bool,
    filter: text_buffer::TextBuffer,
    focus: CategoryColumnPickerFocus,
    list_index: usize,
    selected_ids: HashSet<CategoryId>,
    create_confirm_name: Option<String>,
}

impl CategoryDirectEditRow {
    fn blank() -> Self {
        Self {
            input: text_buffer::TextBuffer::empty(),
            category_id: None,
        }
    }

    fn resolved(category_id: CategoryId, name: String) -> Self {
        Self {
            input: text_buffer::TextBuffer::new(name),
            category_id: Some(category_id),
        }
    }
}

impl CategoryDirectEditState {
    fn active_row(&self) -> Option<&CategoryDirectEditRow> {
        self.rows.get(self.active_row)
    }

    fn active_row_mut(&mut self) -> Option<&mut CategoryDirectEditRow> {
        self.rows.get_mut(self.active_row)
    }

    fn clamp_active_row(&mut self) {
        if self.rows.is_empty() {
            self.active_row = 0;
            return;
        }
        self.active_row = self.active_row.min(self.rows.len() - 1);
    }

    fn add_blank_row(&mut self) -> usize {
        self.rows.push(CategoryDirectEditRow::blank());
        self.active_row = self.rows.len().saturating_sub(1);
        self.active_row
    }

    fn remove_row(&mut self, index: usize) -> Option<CategoryDirectEditRow> {
        if index >= self.rows.len() {
            return None;
        }
        let removed = self.rows.remove(index);
        if index < self.active_row {
            self.active_row = self.active_row.saturating_sub(1);
        }
        self.ensure_one_row();
        self.clamp_active_row();
        Some(removed)
    }

    fn ensure_one_row(&mut self) {
        if self.rows.is_empty() {
            self.rows.push(CategoryDirectEditRow::blank());
            self.active_row = 0;
        } else {
            self.clamp_active_row();
        }
    }

    fn row_would_duplicate_category_id(&self, row_index: usize, category_id: CategoryId) -> bool {
        self.rows.iter().enumerate().any(|(idx, row)| {
            idx != row_index && row.category_id.map(|id| id == category_id).unwrap_or(false)
        })
    }

    fn has_duplicate_resolved_category_ids(&self) -> bool {
        let mut seen = HashSet::new();
        self.rows
            .iter()
            .filter_map(|row| row.category_id)
            .any(|category_id| !seen.insert(category_id))
    }
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
    view_pending_edit_name: Option<String>,
    view_edit_state: Option<ViewEditState>,

    categories: Vec<Category>,
    category_rows: Vec<CategoryListRow>,
    category_index: usize,
    category_manager: Option<CategoryManagerState>,
    category_suggest: Option<CategorySuggestState>,
    category_direct_edit: Option<CategoryDirectEditState>,
    category_direct_edit_create_confirm: Option<String>,
    category_column_picker: Option<CategoryColumnPickerState>,
    board_add_column: Option<BoardAddColumnState>,
    item_assign_category_index: usize,
    input_panel: Option<input_panel::InputPanel>,
    name_input_context: Option<NameInputContext>,
    preview_provenance_scroll: usize,
    preview_summary_scroll: usize,
    inspect_assignment_index: usize,
    slots: Vec<Slot>,
    slot_index: usize,
    item_index: usize,
    column_index: usize,
    normal_mode_prefix: Option<NormalModePrefix>,
    board_pending_delete_column_label: Option<String>,
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
            view_pending_edit_name: None,
            view_edit_state: None,
            categories: Vec::new(),
            category_rows: Vec::new(),
            category_index: 0,
            category_manager: None,
            category_suggest: None,
            category_direct_edit: None,
            category_direct_edit_create_confirm: None,
            category_column_picker: None,
            board_add_column: None,
            item_assign_category_index: 0,
            input_panel: None,
            name_input_context: None,
            preview_provenance_scroll: 0,
            preview_summary_scroll: 0,
            inspect_assignment_index: 0,
            slots: Vec::new(),
            slot_index: 0,
            item_index: 0,
            column_index: 0,
            normal_mode_prefix: None,
            board_pending_delete_column_label: None,
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
        should_render_unmatched_lane, text_buffer, truncate_board_cell, when_bucket_options,
        AddColumnDirection, App, BucketEditTarget, CategoryDirectEditAnchor,
        CategoryDirectEditFocus, CategoryDirectEditRow, CategoryDirectEditState,
        CategoryInlineAction, CategoryListRow, CategoryManagerDetailsFocus, CategoryManagerFocus,
        Mode, NameInputContext, ViewEditPaneFocus, ViewEditRegion,
    };
    use agenda_core::agenda::Agenda;
    use agenda_core::matcher::SubstringClassifier;
    use agenda_core::model::{
        Assignment, AssignmentSource, BoardDisplayMode, Category, CategoryId, Column, ColumnKind,
        CriterionMode, Item, ItemId, Query, Section, View, WhenBucket,
    };
    use agenda_core::store::Store;
    use chrono::NaiveDate;
    use crossterm::event::KeyCode;
    use ratatui::backend::TestBackend;
    use ratatui::layout::Rect;
    use ratatui::Terminal;

    fn row_depth_map(rows: &[super::CategoryListRow]) -> HashMap<CategoryId, usize> {
        rows.iter().map(|row| (row.id, row.depth)).collect()
    }

    #[test]
    fn open_category_direct_edit_initializes_rows_in_parent_order_with_alpha_fallback() {
        let mut parent = Category::new("Priority".to_string());
        let mut medium = Category::new("Medium".to_string());
        medium.parent = Some(parent.id);
        let mut high = Category::new("High".to_string());
        high.parent = Some(parent.id);
        let mut zebra = Category::new("Zebra".to_string());
        zebra.parent = Some(parent.id);
        let mut alpha = Category::new("Alpha".to_string());
        alpha.parent = Some(parent.id);
        let unrelated = Category::new("Elsewhere".to_string());

        // Intentionally non-alphabetical to verify we preserve explicit child order first.
        parent.children = vec![medium.id, high.id];

        let mut item = Item::new("Draft row ordering".to_string());
        let assignment = Assignment {
            source: AssignmentSource::Manual,
            assigned_at: chrono::Utc::now(),
            sticky: false,
            origin: None,
        };
        item.assignments.insert(high.id, assignment.clone());
        item.assignments.insert(medium.id, assignment.clone());
        // Assigned direct children missing from parent.children should fall back alphabetically.
        item.assignments.insert(zebra.id, assignment.clone());
        item.assignments.insert(alpha.id, assignment.clone());
        // Non-child assignment should be ignored for this column.
        item.assignments.insert(unrelated.id, assignment);

        let section = Section {
            title: "Main".to_string(),
            criteria: Query::default(),
            columns: vec![Column {
                kind: ColumnKind::Standard,
                heading: parent.id,
                width: 12,
            }],
            item_column_index: 0,
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        };
        let mut view = View::new("Board".to_string());
        view.sections.push(section);

        let mut app = App {
            categories: vec![
                parent.clone(),
                medium.clone(),
                high.clone(),
                zebra.clone(),
                alpha.clone(),
                unrelated.clone(),
            ],
            views: vec![view],
            slots: vec![super::Slot {
                title: "Main".to_string(),
                items: vec![item.clone()],
                context: super::SlotContext::Section { section_index: 0 },
            }],
            view_index: 0,
            slot_index: 0,
            item_index: 0,
            column_index: 1,
            ..App::default()
        };

        app.open_category_direct_edit();

        assert_eq!(app.mode, Mode::CategoryDirectEdit);
        let state = app
            .category_direct_edit_state()
            .expect("direct edit state should open");
        assert_eq!(state.parent_id, parent.id);
        assert_eq!(state.parent_name, "Priority");
        assert_eq!(state.item_id, item.id);
        assert_eq!(state.item_label, "Draft row ordering");
        assert_eq!(
            state.anchor,
            CategoryDirectEditAnchor {
                slot_index: 0,
                section_index: 0,
                section_column_index: 0,
                board_column_index: 1,
                is_generated_section: false,
            }
        );

        let row_ids: Vec<Option<CategoryId>> =
            state.rows.iter().map(|row| row.category_id).collect();
        assert_eq!(
            row_ids,
            vec![
                Some(medium.id),
                Some(high.id),
                Some(alpha.id),
                Some(zebra.id)
            ]
        );

        let row_names: Vec<String> = state
            .rows
            .iter()
            .map(|row| row.input.text().to_string())
            .collect();
        assert_eq!(row_names, vec!["Medium", "High", "Alpha", "Zebra"]);

        // Phase 1 still mirrors the active row into the shared filter buffer.
        assert_eq!(app.input.text(), "Medium");
    }

    #[test]
    fn open_category_direct_edit_adds_single_blank_row_when_no_child_assignment_exists() {
        let parent = Category::new("Status".to_string());
        let mut child = Category::new("Pending".to_string());
        child.parent = Some(parent.id);

        let item = Item::new("No status yet".to_string());
        let section = Section {
            title: "Main".to_string(),
            criteria: Query::default(),
            columns: vec![Column {
                kind: ColumnKind::Standard,
                heading: parent.id,
                width: 12,
            }],
            item_column_index: 0,
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        };
        let mut view = View::new("Board".to_string());
        view.sections.push(section);

        let mut app = App {
            categories: vec![parent, child],
            views: vec![view],
            slots: vec![super::Slot {
                title: "Main".to_string(),
                items: vec![item],
                context: super::SlotContext::Section { section_index: 0 },
            }],
            view_index: 0,
            slot_index: 0,
            item_index: 0,
            column_index: 1,
            ..App::default()
        };

        app.open_category_direct_edit();

        let state = app
            .category_direct_edit_state()
            .expect("direct edit state should open");
        assert_eq!(state.rows.len(), 1);
        assert_eq!(state.rows[0].category_id, None);
        assert!(state.rows[0].input.text().is_empty());
        assert_eq!(state.active_row, 0);
        assert_eq!(app.input.text(), "");
    }

    #[test]
    fn category_direct_edit_row_helpers_keep_state_invariants() {
        let duplicate_a = CategoryId::new_v4();
        let duplicate_b = CategoryId::new_v4();
        let mut state = CategoryDirectEditState {
            anchor: CategoryDirectEditAnchor {
                slot_index: 0,
                section_index: 0,
                section_column_index: 0,
                board_column_index: 1,
                is_generated_section: false,
            },
            parent_id: CategoryId::new_v4(),
            parent_name: "Parent".to_string(),
            item_id: agenda_core::model::ItemId::new_v4(),
            item_label: "Item".to_string(),
            rows: Vec::new(),
            active_row: 7,
            focus: CategoryDirectEditFocus::Input,
            suggest_index: 0,
            create_confirm_name: None,
        };

        assert!(state.active_row().is_none());
        state.ensure_one_row();
        assert_eq!(state.rows.len(), 1);
        assert_eq!(state.active_row, 0);
        assert!(state.active_row().is_some());
        assert_eq!(state.active_row().and_then(|row| row.category_id), None);

        state
            .active_row_mut()
            .expect("row exists")
            .input
            .set("First".to_string());
        let new_index = state.add_blank_row();
        assert_eq!(new_index, 1);
        assert_eq!(state.active_row, 1);
        assert_eq!(state.rows.len(), 2);

        state.active_row = 99;
        state.clamp_active_row();
        assert_eq!(state.active_row, 1);

        let removed = state.remove_row(0).expect("remove existing row");
        assert_eq!(removed.input.text(), "First");
        assert_eq!(state.rows.len(), 1);
        assert_eq!(state.active_row, 0);

        let removed_last = state.remove_row(0).expect("remove last row");
        assert!(removed_last.category_id.is_none());
        assert_eq!(
            state.rows.len(),
            1,
            "last-row removal should keep one blank row"
        );
        assert_eq!(state.active_row, 0);
        assert_eq!(state.rows[0].category_id, None);
        assert!(state.rows[0].input.text().is_empty());

        assert!(state.remove_row(99).is_none());
        assert_eq!(state.rows.len(), 1);

        state.rows = vec![
            CategoryDirectEditRow::resolved(duplicate_a, "A".to_string()),
            CategoryDirectEditRow::resolved(duplicate_b, "B".to_string()),
            CategoryDirectEditRow::resolved(duplicate_a, "A".to_string()),
        ];
        state.active_row = 1;
        assert!(state.has_duplicate_resolved_category_ids());
        assert!(state.row_would_duplicate_category_id(1, duplicate_a));
        assert!(!state.row_would_duplicate_category_id(1, duplicate_b));
        assert!(!state.row_would_duplicate_category_id(1, CategoryId::new_v4()));

        let _ = CategoryDirectEditRow::blank();
    }

    #[test]
    fn category_direct_edit_empty_input_shows_full_child_suggestions_excluding_when() {
        let mut parent = Category::new("Context".to_string());
        let mut alpha = Category::new("Alpha".to_string());
        alpha.parent = Some(parent.id);
        let mut when_child = Category::new("When".to_string());
        when_child.parent = Some(parent.id);
        let mut beta = Category::new("Beta".to_string());
        beta.parent = Some(parent.id);
        parent.children = vec![alpha.id, when_child.id, beta.id];

        let item = Item::new("Demo".to_string());
        let section = Section {
            title: "Main".to_string(),
            criteria: Query::default(),
            columns: vec![Column {
                kind: ColumnKind::Standard,
                heading: parent.id,
                width: 12,
            }],
            item_column_index: 0,
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        };
        let mut view = View::new("Board".to_string());
        view.sections.push(section);

        let mut app = App {
            categories: vec![parent.clone(), alpha.clone(), when_child, beta.clone()],
            views: vec![view],
            slots: vec![super::Slot {
                title: "Main".to_string(),
                items: vec![item],
                context: super::SlotContext::Section { section_index: 0 },
            }],
            view_index: 0,
            slot_index: 0,
            item_index: 0,
            column_index: 1,
            ..App::default()
        };
        app.open_category_direct_edit();
        if let Some(state) = app.category_direct_edit_state_mut() {
            state.rows[0].input.clear();
        }

        let matches = app.get_current_suggest_matches();
        assert_eq!(matches, vec![alpha.id, beta.id]);
    }

    #[test]
    fn category_direct_edit_suggestions_follow_active_row_input() {
        let mut parent = Category::new("Tags".to_string());
        let mut alpha = Category::new("Alpha".to_string());
        alpha.parent = Some(parent.id);
        let mut beta = Category::new("Beta".to_string());
        beta.parent = Some(parent.id);
        let mut gamma = Category::new("Gamma".to_string());
        gamma.parent = Some(parent.id);
        parent.children = vec![alpha.id, beta.id, gamma.id];

        let mut state = CategoryDirectEditState {
            anchor: CategoryDirectEditAnchor {
                slot_index: 0,
                section_index: 0,
                section_column_index: 0,
                board_column_index: 1,
                is_generated_section: false,
            },
            parent_id: parent.id,
            parent_name: "Tags".to_string(),
            item_id: ItemId::new_v4(),
            item_label: "Demo".to_string(),
            rows: vec![
                CategoryDirectEditRow {
                    input: text_buffer::TextBuffer::new("al".to_string()),
                    category_id: None,
                },
                CategoryDirectEditRow {
                    input: text_buffer::TextBuffer::new("be".to_string()),
                    category_id: None,
                },
            ],
            active_row: 0,
            focus: CategoryDirectEditFocus::Input,
            suggest_index: 0,
            create_confirm_name: None,
        };

        let mut app = App {
            categories: vec![parent.clone(), alpha.clone(), beta.clone(), gamma.clone()],
            views: vec![{
                let mut v = View::new("Board".to_string());
                v.sections.push(Section {
                    title: "Main".to_string(),
                    criteria: Query::default(),
                    columns: vec![Column {
                        kind: ColumnKind::Standard,
                        heading: parent.id,
                        width: 12,
                    }],
                    item_column_index: 0,
                    on_insert_assign: std::collections::HashSet::new(),
                    on_remove_unassign: std::collections::HashSet::new(),
                    show_children: false,
                    board_display_mode_override: None,
                });
                v
            }],
            slots: vec![super::Slot {
                title: "Main".to_string(),
                items: vec![Item::new("Demo".to_string())],
                context: super::SlotContext::Section { section_index: 0 },
            }],
            view_index: 0,
            slot_index: 0,
            item_index: 0,
            column_index: 1,
            category_direct_edit: Some(state.clone()),
            ..App::default()
        };

        let matches_row0 = app.get_current_suggest_matches();
        assert_eq!(matches_row0, vec![alpha.id]);

        state.active_row = 1;
        app.category_direct_edit = Some(state);
        let matches_row1 = app.get_current_suggest_matches();
        assert_eq!(matches_row1, vec![beta.id]);
    }

    #[test]
    fn category_direct_edit_enter_prefers_exact_match_over_highlighted_suggestion() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!(
            "agenda-tui-direct-edit-exact-precedence-{nanos}.ag"
        ));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut parent = Category::new("Project".to_string());
        let mut alpha_beta = Category::new("AlphaBeta".to_string());
        alpha_beta.parent = Some(parent.id);
        let mut alpha = Category::new("Alpha".to_string());
        alpha.parent = Some(parent.id);
        parent.children = vec![alpha_beta.id, alpha.id];
        store.create_category(&parent).expect("create parent");
        store
            .create_category(&alpha_beta)
            .expect("create alpha_beta");
        store.create_category(&alpha).expect("create alpha");

        let mut view = View::new("Board".to_string());
        view.sections.push(Section {
            title: "Main".to_string(),
            criteria: Query::default(),
            columns: vec![Column {
                kind: ColumnKind::Standard,
                heading: parent.id,
                width: 12,
            }],
            item_column_index: 0,
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        });
        store.create_view(&view).expect("create board view");

        let item = Item::new("Demo item".to_string());
        store.create_item(&item).expect("create item");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.set_view_selection_by_name("Board");
        app.refresh(&store).expect("refresh board view");
        app.column_index = 1;
        app.slot_index = 0;
        app.item_index = 0;
        app.open_category_direct_edit();

        let state = app
            .category_direct_edit_state_mut()
            .expect("direct edit state present");
        state.rows[0].input.set("Alpha".to_string());
        state.rows[0].category_id = None;
        state.suggest_index = 0; // Highlights AlphaBeta first, exact match should still win.

        app.handle_category_direct_edit_key(KeyCode::Enter, &agenda)
            .expect("enter handled");

        // Enter resolves the draft row only (exact match still wins over highlighted suggestion).
        let state = app
            .category_direct_edit_state()
            .expect("direct edit state still open");
        assert_eq!(state.rows[0].category_id, Some(alpha.id));
        assert_eq!(state.rows[0].input.text(), "Alpha");

        // Backend remains unchanged until explicit save.
        let saved_before = store.get_item(item.id).expect("load item before save");
        assert!(!saved_before.assignments.contains_key(&alpha.id));
        assert!(!saved_before.assignments.contains_key(&alpha_beta.id));

        app.handle_category_direct_edit_key(KeyCode::Char('s'), &agenda)
            .expect("save draft");
        let saved_after = store.get_item(item.id).expect("load item after save");
        assert!(saved_after.assignments.contains_key(&alpha.id));
        assert!(!saved_after.assignments.contains_key(&alpha_beta.id));

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn category_direct_edit_enter_on_empty_row_removes_row_or_keeps_single_blank() {
        let mut state = CategoryDirectEditState {
            anchor: CategoryDirectEditAnchor {
                slot_index: 0,
                section_index: 0,
                section_column_index: 0,
                board_column_index: 1,
                is_generated_section: false,
            },
            parent_id: CategoryId::new_v4(),
            parent_name: "Tags".to_string(),
            item_id: ItemId::new_v4(),
            item_label: "Demo".to_string(),
            rows: vec![
                CategoryDirectEditRow::blank(),
                CategoryDirectEditRow {
                    input: text_buffer::TextBuffer::new("keep".to_string()),
                    category_id: None,
                },
            ],
            active_row: 0,
            focus: CategoryDirectEditFocus::Input,
            suggest_index: 0,
            create_confirm_name: None,
        };
        let mut app = App {
            category_direct_edit: Some(state.clone()),
            ..App::default()
        };
        let store = Store::open(&std::env::temp_dir().join(format!(
            "agenda-tui-direct-edit-empty-enter-{}.ag",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        )))
        .expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        app.handle_category_direct_edit_key(KeyCode::Enter, &agenda)
            .expect("enter removes empty row");
        let state_after = app.category_direct_edit_state().expect("state");
        assert_eq!(state_after.rows.len(), 1);

        state.rows = vec![CategoryDirectEditRow::blank()];
        state.active_row = 0;
        let mut app = App {
            category_direct_edit: Some(state),
            ..App::default()
        };
        app.handle_category_direct_edit_key(KeyCode::Enter, &agenda)
            .expect("enter keeps single blank row");
        let state_after = app.category_direct_edit_state().expect("state");
        assert_eq!(state_after.rows.len(), 1);
        assert!(state_after.rows[0].input.text().is_empty());
    }

    #[test]
    fn category_direct_edit_input_focus_allows_typing_n_a_x() {
        let parent = Category::new("Tags".to_string());
        let mut child = Category::new("Alpha".to_string());
        child.parent = Some(parent.id);

        let item = Item::new("Demo".to_string());
        let section = Section {
            title: "Main".to_string(),
            criteria: Query::default(),
            columns: vec![Column {
                kind: ColumnKind::Standard,
                heading: parent.id,
                width: 12,
            }],
            item_column_index: 0,
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        };
        let mut view = View::new("Board".to_string());
        view.sections.push(section);

        let mut app = App {
            categories: vec![parent, child],
            views: vec![view],
            slots: vec![super::Slot {
                title: "Main".to_string(),
                items: vec![item],
                context: super::SlotContext::Section { section_index: 0 },
            }],
            view_index: 0,
            slot_index: 0,
            item_index: 0,
            column_index: 1,
            ..App::default()
        };
        app.open_category_direct_edit();
        let store = Store::open_memory().expect("memory store");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let rows_before = app
            .category_direct_edit_state()
            .expect("direct edit")
            .rows
            .len();
        assert_eq!(
            app.category_direct_edit_state().expect("state").focus,
            CategoryDirectEditFocus::Input
        );

        app.handle_category_direct_edit_key(KeyCode::Char('n'), &agenda)
            .expect("type n");
        app.handle_category_direct_edit_key(KeyCode::Char('a'), &agenda)
            .expect("type a");
        app.handle_category_direct_edit_key(KeyCode::Char('x'), &agenda)
            .expect("type x");

        let state = app.category_direct_edit_state().expect("direct edit state");
        assert_eq!(state.rows.len(), rows_before);
        assert_eq!(state.rows[0].input.text(), "nax");
    }

    #[test]
    fn category_direct_edit_input_focus_plus_adds_row_without_switching_focus() {
        let parent = Category::new("Tags".to_string());
        let mut child = Category::new("Alpha".to_string());
        child.parent = Some(parent.id);

        let item = Item::new("Demo".to_string());
        let section = Section {
            title: "Main".to_string(),
            criteria: Query::default(),
            columns: vec![Column {
                kind: ColumnKind::Standard,
                heading: parent.id,
                width: 12,
            }],
            item_column_index: 0,
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        };
        let mut view = View::new("Board".to_string());
        view.sections.push(section);

        let mut app = App {
            categories: vec![parent, child],
            views: vec![view],
            slots: vec![super::Slot {
                title: "Main".to_string(),
                items: vec![item],
                context: super::SlotContext::Section { section_index: 0 },
            }],
            view_index: 0,
            slot_index: 0,
            item_index: 0,
            column_index: 1,
            ..App::default()
        };
        app.open_category_direct_edit();
        let store = Store::open_memory().expect("memory store");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        assert_eq!(
            app.category_direct_edit_state().expect("state").focus,
            CategoryDirectEditFocus::Input
        );

        app.handle_category_direct_edit_key(KeyCode::Char('+'), &agenda)
            .expect("plus adds row");

        let state = app.category_direct_edit_state().expect("direct edit state");
        assert_eq!(state.rows.len(), 2);
        assert_eq!(state.active_row, 1);
        assert_eq!(state.focus, CategoryDirectEditFocus::Input);
        assert!(app.status.contains("Added row"));
    }

    #[test]
    fn category_direct_edit_input_focus_plus_is_blocked_for_exclusive_parent() {
        let mut parent = Category::new("Priority".to_string());
        parent.is_exclusive = true;
        let mut child = Category::new("High".to_string());
        child.parent = Some(parent.id);
        parent.children = vec![child.id];

        let item = Item::new("Demo".to_string());
        let section = Section {
            title: "Main".to_string(),
            criteria: Query::default(),
            columns: vec![Column {
                kind: ColumnKind::Standard,
                heading: parent.id,
                width: 12,
            }],
            item_column_index: 0,
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        };
        let mut view = View::new("Board".to_string());
        view.sections.push(section);

        let mut app = App {
            categories: vec![parent, child],
            views: vec![view],
            slots: vec![super::Slot {
                title: "Main".to_string(),
                items: vec![item],
                context: super::SlotContext::Section { section_index: 0 },
            }],
            view_index: 0,
            slot_index: 0,
            item_index: 0,
            column_index: 1,
            ..App::default()
        };
        app.open_category_direct_edit();
        let store = Store::open_memory().expect("memory store");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        app.handle_category_direct_edit_key(KeyCode::Char('+'), &agenda)
            .expect("plus handled");

        let state = app.category_direct_edit_state().expect("direct edit state");
        assert_eq!(state.rows.len(), 1);
        assert_eq!(state.focus, CategoryDirectEditFocus::Input);
        assert!(app.status.contains("exclusive"));
    }

    #[test]
    fn category_direct_edit_entries_focus_n_and_x_still_add_and_remove_rows() {
        let parent = Category::new("Tags".to_string());
        let mut child = Category::new("Alpha".to_string());
        child.parent = Some(parent.id);
        let item = Item::new("Demo".to_string());
        let section = Section {
            title: "Main".to_string(),
            criteria: Query::default(),
            columns: vec![Column {
                kind: ColumnKind::Standard,
                heading: parent.id,
                width: 12,
            }],
            item_column_index: 0,
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        };
        let mut view = View::new("Board".to_string());
        view.sections.push(section);

        let mut app = App {
            categories: vec![parent, child],
            views: vec![view],
            slots: vec![super::Slot {
                title: "Main".to_string(),
                items: vec![item],
                context: super::SlotContext::Section { section_index: 0 },
            }],
            view_index: 0,
            slot_index: 0,
            item_index: 0,
            column_index: 1,
            ..App::default()
        };
        app.open_category_direct_edit();
        let store = Store::open_memory().expect("memory store");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        app.handle_category_direct_edit_key(KeyCode::Tab, &agenda)
            .expect("tab to suggestions");
        app.handle_category_direct_edit_key(KeyCode::Tab, &agenda)
            .expect("tab to entries");
        assert_eq!(
            app.category_direct_edit_state().expect("state").focus,
            CategoryDirectEditFocus::Entries
        );

        app.handle_category_direct_edit_key(KeyCode::Char('n'), &agenda)
            .expect("entries n adds row");
        let state = app.category_direct_edit_state().expect("state");
        assert_eq!(state.rows.len(), 2);
        assert_eq!(state.focus, CategoryDirectEditFocus::Input);

        app.handle_category_direct_edit_key(KeyCode::BackTab, &agenda)
            .expect("backtab to entries");
        app.handle_category_direct_edit_key(KeyCode::Char('x'), &agenda)
            .expect("entries x removes row");
        let state = app.category_direct_edit_state().expect("state");
        assert_eq!(state.rows.len(), 1);
        assert!(app.status.contains("Removed row"));
    }

    #[test]
    fn category_direct_edit_tab_cycles_focus_instead_of_autocomplete_from_suggestions() {
        let mut parent = Category::new("Tags".to_string());
        let mut alpha = Category::new("Alpha".to_string());
        alpha.parent = Some(parent.id);
        let mut alphabet = Category::new("Alphabet".to_string());
        alphabet.parent = Some(parent.id);
        parent.children = vec![alpha.id, alphabet.id];

        let item = Item::new("Demo".to_string());
        let section = Section {
            title: "Main".to_string(),
            criteria: Query::default(),
            columns: vec![Column {
                kind: ColumnKind::Standard,
                heading: parent.id,
                width: 12,
            }],
            item_column_index: 0,
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        };
        let mut view = View::new("Board".to_string());
        view.sections.push(section);

        let mut app = App {
            categories: vec![parent, alpha, alphabet],
            views: vec![view],
            slots: vec![super::Slot {
                title: "Main".to_string(),
                items: vec![item],
                context: super::SlotContext::Section { section_index: 0 },
            }],
            view_index: 0,
            slot_index: 0,
            item_index: 0,
            column_index: 1,
            ..App::default()
        };
        app.open_category_direct_edit();
        let store = Store::open_memory().expect("memory store");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        app.handle_category_direct_edit_key(KeyCode::Char('A'), &agenda)
            .expect("type A");
        app.handle_category_direct_edit_key(KeyCode::Char('l'), &agenda)
            .expect("type l");

        app.handle_category_direct_edit_key(KeyCode::Tab, &agenda)
            .expect("tab to suggestions");
        assert_eq!(
            app.category_direct_edit_state().expect("state").focus,
            CategoryDirectEditFocus::Suggestions
        );

        app.handle_category_direct_edit_key(KeyCode::Tab, &agenda)
            .expect("tab to entries");
        let state = app.category_direct_edit_state().expect("state");
        assert_eq!(state.focus, CategoryDirectEditFocus::Entries);
        assert_eq!(state.rows[0].input.text(), "Al");
    }

    #[test]
    fn category_direct_edit_right_autocompletes_from_suggestions_focus() {
        let mut parent = Category::new("Tags".to_string());
        let mut alpha = Category::new("Alpha".to_string());
        alpha.parent = Some(parent.id);
        let mut alphabet = Category::new("Alphabet".to_string());
        alphabet.parent = Some(parent.id);
        parent.children = vec![alpha.id, alphabet.id];

        let item = Item::new("Demo".to_string());
        let section = Section {
            title: "Main".to_string(),
            criteria: Query::default(),
            columns: vec![Column {
                kind: ColumnKind::Standard,
                heading: parent.id,
                width: 12,
            }],
            item_column_index: 0,
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        };
        let mut view = View::new("Board".to_string());
        view.sections.push(section);

        let mut app = App {
            categories: vec![parent, alpha, alphabet],
            views: vec![view],
            slots: vec![super::Slot {
                title: "Main".to_string(),
                items: vec![item],
                context: super::SlotContext::Section { section_index: 0 },
            }],
            view_index: 0,
            slot_index: 0,
            item_index: 0,
            column_index: 1,
            ..App::default()
        };
        app.open_category_direct_edit();
        let store = Store::open_memory().expect("memory store");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        app.handle_category_direct_edit_key(KeyCode::Char('A'), &agenda)
            .expect("type A");
        app.handle_category_direct_edit_key(KeyCode::Char('l'), &agenda)
            .expect("type l");
        app.handle_category_direct_edit_key(KeyCode::Tab, &agenda)
            .expect("focus suggestions");
        assert_eq!(
            app.category_direct_edit_state().expect("state").focus,
            CategoryDirectEditFocus::Suggestions
        );

        app.handle_category_direct_edit_key(KeyCode::Right, &agenda)
            .expect("autocomplete");
        let state = app.category_direct_edit_state().expect("state");
        assert_eq!(state.focus, CategoryDirectEditFocus::Suggestions);
        assert_ne!(state.rows[0].input.text(), "Al");
        assert!(
            state.rows[0].input.text().starts_with("Al"),
            "unexpected autocomplete result: {}",
            state.rows[0].input.text()
        );
    }

    #[test]
    fn category_direct_edit_inline_create_confirm_resolves_row_and_stays_open() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let db_path =
            std::env::temp_dir().join(format!("agenda-tui-direct-edit-inline-create-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let parent = Category::new("Tags".to_string());
        store.create_category(&parent).expect("create parent");
        let mut view = View::new("Board".to_string());
        view.sections.push(Section {
            title: "Main".to_string(),
            criteria: Query::default(),
            columns: vec![Column {
                kind: ColumnKind::Standard,
                heading: parent.id,
                width: 12,
            }],
            item_column_index: 0,
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        });
        store.create_view(&view).expect("create view");
        let item = Item::new("Demo item".to_string());
        store.create_item(&item).expect("create item");

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        app.set_view_selection_by_name("Board");
        app.refresh(&store).expect("refresh board");
        app.column_index = 1;
        app.open_category_direct_edit();
        {
            let state = app
                .category_direct_edit_state_mut()
                .expect("direct edit state present");
            state.rows[0].input.set("NewTag".to_string());
            state.rows[0].category_id = None;
        }

        app.handle_category_direct_edit_key(KeyCode::Enter, &agenda)
            .expect("open create confirm");
        assert_eq!(
            app.category_direct_edit_state()
                .and_then(|s| s.create_confirm_name.as_deref()),
            Some("NewTag")
        );
        assert_eq!(app.mode, Mode::CategoryDirectEdit);

        app.handle_category_direct_edit_key(KeyCode::Enter, &agenda)
            .expect("confirm create");
        let state = app.category_direct_edit_state().expect("still in editor");
        assert_eq!(state.create_confirm_name, None);
        let resolved_id = state.rows[0].category_id.expect("row resolved");
        let created = store.get_category(resolved_id).expect("created category");
        assert_eq!(created.name, "NewTag");
        assert_eq!(created.parent, Some(parent.id));
        // Not assigned yet until save.
        let saved_item = store.get_item(item.id).expect("load item");
        assert!(!saved_item.assignments.contains_key(&resolved_id));

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn category_direct_edit_save_applies_mixed_diff_and_preserves_non_column_assignments() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let db_path =
            std::env::temp_dir().join(format!("agenda-tui-direct-edit-save-diff-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut parent = Category::new("Tags".to_string());
        let mut a = Category::new("A".to_string());
        a.parent = Some(parent.id);
        let mut b = Category::new("B".to_string());
        b.parent = Some(parent.id);
        let mut c = Category::new("C".to_string());
        c.parent = Some(parent.id);
        parent.children = vec![a.id, b.id, c.id];
        let outside = Category::new("Outside".to_string());
        for cat in [&parent, &a, &b, &c, &outside] {
            store.create_category(cat).expect("create category");
        }

        let mut view = View::new("Board".to_string());
        view.sections.push(Section {
            title: "Main".to_string(),
            criteria: Query::default(),
            columns: vec![Column {
                kind: ColumnKind::Standard,
                heading: parent.id,
                width: 12,
            }],
            item_column_index: 0,
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        });
        store.create_view(&view).expect("create view");

        let item = Item::new("Demo item".to_string());
        store.create_item(&item).expect("create item");
        agenda
            .assign_item_manual(item.id, a.id, None)
            .expect("assign a");
        agenda
            .assign_item_manual(item.id, b.id, None)
            .expect("assign b");
        agenda
            .assign_item_manual(item.id, outside.id, None)
            .expect("assign outside");

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        app.set_view_selection_by_name("Board");
        app.refresh(&store).expect("refresh board");
        app.column_index = 1;
        app.open_category_direct_edit();
        {
            let state = app
                .category_direct_edit_state_mut()
                .expect("direct edit state present");
            state.rows = vec![
                CategoryDirectEditRow::resolved(b.id, "B".to_string()),
                CategoryDirectEditRow::resolved(c.id, "C".to_string()),
            ];
            state.active_row = 0;
        }

        app.handle_category_direct_edit_key(KeyCode::Char('S'), &agenda)
            .expect("save draft");
        assert_eq!(app.mode, Mode::Normal);

        let saved = store.get_item(item.id).expect("load item");
        assert!(!saved.assignments.contains_key(&a.id), "A removed");
        assert!(saved.assignments.contains_key(&b.id), "B kept");
        assert!(saved.assignments.contains_key(&c.id), "C added");
        assert!(
            saved.assignments.contains_key(&outside.id),
            "non-column assignment preserved"
        );

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn board_cell_enter_opens_category_column_picker_for_non_exclusive_parent() {
        let store = Store::open_memory().expect("memory store");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut area = Category::new("Area".to_string());
        let mut cli = Category::new("CLI".to_string());
        cli.parent = Some(area.id);
        let mut ux = Category::new("UX".to_string());
        ux.parent = Some(area.id);
        area.children = vec![cli.id, ux.id];
        for cat in [&area, &cli, &ux] {
            store.create_category(cat).expect("create category");
        }

        let item = Item::new("Demo".to_string());
        store.create_item(&item).expect("create item");
        agenda
            .assign_item_manual(item.id, ux.id, None)
            .expect("assign ux");

        let mut view = View::new("Board".to_string());
        view.sections.push(Section {
            title: "Main".to_string(),
            criteria: Query::default(),
            columns: vec![Column {
                kind: ColumnKind::Standard,
                heading: area.id,
                width: 12,
            }],
            item_column_index: 0,
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        });
        store.create_view(&view).expect("create view");

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        app.set_view_selection_by_name("Board");
        app.refresh(&store).expect("refresh board");
        app.column_index = 1;

        app.handle_key(KeyCode::Enter, &agenda).expect("enter");

        assert_eq!(app.mode, Mode::CategoryColumnPicker);
        let state = app.category_column_picker.as_ref().expect("picker");
        assert_eq!(state.parent_name, "Area");
        assert!(state.selected_ids.contains(&ux.id));
        assert!(!state.selected_ids.contains(&cli.id));
    }

    #[test]
    fn board_cell_enter_opens_category_column_picker_for_exclusive_parent() {
        let store = Store::open_memory().expect("memory store");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut status = Category::new("Status".to_string());
        status.is_exclusive = true;
        let mut pending = Category::new("Pending".to_string());
        pending.parent = Some(status.id);
        status.children = vec![pending.id];
        for cat in [&status, &pending] {
            store.create_category(cat).expect("create category");
        }
        store
            .create_item(&Item::new("Demo".to_string()))
            .expect("create item");

        let mut view = View::new("Board".to_string());
        view.sections.push(Section {
            title: "Main".to_string(),
            criteria: Query::default(),
            columns: vec![Column {
                kind: ColumnKind::Standard,
                heading: status.id,
                width: 12,
            }],
            item_column_index: 0,
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        });
        store.create_view(&view).expect("create view");

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        app.set_view_selection_by_name("Board");
        app.refresh(&store).expect("refresh board");
        app.column_index = 1;

        app.handle_key(KeyCode::Enter, &agenda).expect("enter");
        assert_eq!(app.mode, Mode::CategoryColumnPicker);
        let state = app.category_column_picker.as_ref().expect("picker");
        assert!(state.is_exclusive);
    }

    #[test]
    fn category_column_picker_multi_toggle_save_applies_diff_and_preserves_other_assignments() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let db_path =
            std::env::temp_dir().join(format!("agenda-tui-column-picker-save-diff-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut area = Category::new("Area".to_string());
        let mut cli = Category::new("CLI".to_string());
        cli.parent = Some(area.id);
        let mut ux = Category::new("UX".to_string());
        ux.parent = Some(area.id);
        let mut validation = Category::new("Validation".to_string());
        validation.parent = Some(area.id);
        area.children = vec![cli.id, ux.id, validation.id];
        let outside = Category::new("Outside".to_string());
        for cat in [&area, &cli, &ux, &validation, &outside] {
            store.create_category(cat).expect("create category");
        }

        let item = Item::new("Demo".to_string());
        store.create_item(&item).expect("create item");
        agenda
            .assign_item_manual(item.id, ux.id, None)
            .expect("assign ux");
        agenda
            .assign_item_manual(item.id, outside.id, None)
            .expect("assign outside");

        let mut view = View::new("Board".to_string());
        view.sections.push(Section {
            title: "Main".to_string(),
            criteria: Query::default(),
            columns: vec![Column {
                kind: ColumnKind::Standard,
                heading: area.id,
                width: 12,
            }],
            item_column_index: 0,
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        });
        store.create_view(&view).expect("create view");

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        app.set_view_selection_by_name("Board");
        app.refresh(&store).expect("refresh board");
        app.column_index = 1;

        app.handle_key(KeyCode::Enter, &agenda)
            .expect("open picker");
        assert_eq!(app.mode, Mode::CategoryColumnPicker);

        for ch in "CLI".chars() {
            app.handle_key(KeyCode::Char(ch), &agenda)
                .expect("type cli");
        }
        app.handle_key(KeyCode::Char(' '), &agenda)
            .expect("toggle cli on");

        app.handle_key(KeyCode::Tab, &agenda).expect("focus filter");
        for _ in 0..3 {
            app.handle_key(KeyCode::Backspace, &agenda)
                .expect("clear filter");
        }
        for ch in "UX".chars() {
            app.handle_key(KeyCode::Char(ch), &agenda).expect("type ux");
        }
        app.handle_key(KeyCode::Char(' '), &agenda)
            .expect("toggle ux off");

        app.handle_key(KeyCode::Enter, &agenda)
            .expect("save picker");
        assert_eq!(app.mode, Mode::Normal);

        let saved = store.get_item(item.id).expect("load item");
        assert!(saved.assignments.contains_key(&cli.id), "CLI added");
        assert!(!saved.assignments.contains_key(&ux.id), "UX removed");
        assert!(
            saved.assignments.contains_key(&outside.id),
            "outside assignment preserved"
        );

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn category_column_picker_cancel_discards_staged_changes() {
        let store = Store::open_memory().expect("memory store");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut area = Category::new("Area".to_string());
        let mut cli = Category::new("CLI".to_string());
        cli.parent = Some(area.id);
        area.children = vec![cli.id];
        for cat in [&area, &cli] {
            store.create_category(cat).expect("create category");
        }
        let item = Item::new("Demo".to_string());
        store.create_item(&item).expect("create item");

        let mut view = View::new("Board".to_string());
        view.sections.push(Section {
            title: "Main".to_string(),
            criteria: Query::default(),
            columns: vec![Column {
                kind: ColumnKind::Standard,
                heading: area.id,
                width: 12,
            }],
            item_column_index: 0,
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        });
        store.create_view(&view).expect("create view");

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        app.set_view_selection_by_name("Board");
        app.refresh(&store).expect("refresh board");
        app.column_index = 1;

        app.handle_key(KeyCode::Enter, &agenda)
            .expect("open picker");
        for ch in "CLI".chars() {
            app.handle_key(KeyCode::Char(ch), &agenda).expect("type");
        }
        app.handle_key(KeyCode::Char(' '), &agenda)
            .expect("toggle on");
        app.handle_key(KeyCode::Esc, &agenda).expect("cancel");

        assert_eq!(app.mode, Mode::Normal);
        let saved = store.get_item(item.id).expect("load item");
        assert!(!saved.assignments.contains_key(&cli.id));
    }

    #[test]
    fn category_column_picker_exclusive_selection_replaces_previous_and_never_stages_multiple() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!(
            "agenda-tui-column-picker-exclusive-replace-{nanos}.ag"
        ));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut status = Category::new("Status".to_string());
        status.is_exclusive = true;
        let mut pending = Category::new("Pending".to_string());
        pending.parent = Some(status.id);
        let mut deferred = Category::new("Deferred".to_string());
        deferred.parent = Some(status.id);
        status.children = vec![pending.id, deferred.id];
        for cat in [&status, &pending, &deferred] {
            store.create_category(cat).expect("create category");
        }

        let item = Item::new("Demo".to_string());
        store.create_item(&item).expect("create item");
        agenda
            .assign_item_manual(item.id, pending.id, None)
            .expect("assign pending");

        let mut view = View::new("Board".to_string());
        view.sections.push(Section {
            title: "Main".to_string(),
            criteria: Query::default(),
            columns: vec![Column {
                kind: ColumnKind::Standard,
                heading: status.id,
                width: 12,
            }],
            item_column_index: 0,
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        });
        store.create_view(&view).expect("create view");

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        app.set_view_selection_by_name("Board");
        app.refresh(&store).expect("refresh board");
        app.column_index = 1;

        app.handle_key(KeyCode::Enter, &agenda)
            .expect("open picker");
        assert_eq!(app.mode, Mode::CategoryColumnPicker);
        {
            let state = app.category_column_picker.as_ref().expect("picker");
            assert!(state.selected_ids.contains(&pending.id));
            assert_eq!(state.selected_ids.len(), 1);
            assert!(state.is_exclusive);
        }

        for ch in "Deferred".chars() {
            app.handle_key(KeyCode::Char(ch), &agenda).expect("type");
        }
        app.handle_key(KeyCode::Char(' '), &agenda)
            .expect("select deferred");
        {
            let state = app.category_column_picker.as_ref().expect("picker");
            assert!(state.selected_ids.contains(&deferred.id));
            assert!(!state.selected_ids.contains(&pending.id));
            assert_eq!(state.selected_ids.len(), 1, "radio behavior");
        }

        app.handle_key(KeyCode::Enter, &agenda)
            .expect("save picker");
        let saved = store.get_item(item.id).expect("load item");
        assert!(!saved.assignments.contains_key(&pending.id));
        assert!(saved.assignments.contains_key(&deferred.id));

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn category_column_picker_create_child_confirm_selects_and_then_saves() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let db_path =
            std::env::temp_dir().join(format!("agenda-tui-column-picker-create-child-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut area = Category::new("Area".to_string());
        let mut cli = Category::new("CLI".to_string());
        cli.parent = Some(area.id);
        area.children = vec![cli.id];
        for cat in [&area, &cli] {
            store.create_category(cat).expect("create category");
        }
        let item = Item::new("Demo".to_string());
        store.create_item(&item).expect("create item");

        let mut view = View::new("Board".to_string());
        view.sections.push(Section {
            title: "Main".to_string(),
            criteria: Query::default(),
            columns: vec![Column {
                kind: ColumnKind::Standard,
                heading: area.id,
                width: 12,
            }],
            item_column_index: 0,
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        });
        store.create_view(&view).expect("create view");

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        app.set_view_selection_by_name("Board");
        app.refresh(&store).expect("refresh board");
        app.column_index = 1;

        app.handle_key(KeyCode::Enter, &agenda)
            .expect("open picker");
        for ch in "Docs".chars() {
            app.handle_key(KeyCode::Char(ch), &agenda).expect("type");
        }
        app.handle_key(KeyCode::Enter, &agenda)
            .expect("open create confirm");
        let state = app.category_column_picker.as_ref().expect("picker");
        assert_eq!(state.create_confirm_name.as_deref(), Some("Docs"));
        assert_eq!(app.mode, Mode::CategoryColumnPicker);

        app.handle_key(KeyCode::Enter, &agenda)
            .expect("confirm create");
        let created = store
            .get_hierarchy()
            .expect("hierarchy")
            .into_iter()
            .find(|c| c.name == "Docs")
            .expect("created category");
        assert_eq!(created.parent, Some(area.id));
        let state = app.category_column_picker.as_ref().expect("picker");
        assert!(state.selected_ids.contains(&created.id));
        assert_eq!(state.create_confirm_name, None);

        app.handle_key(KeyCode::Enter, &agenda)
            .expect("save picker");
        let saved = store.get_item(item.id).expect("load item");
        assert!(saved.assignments.contains_key(&created.id));

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn category_column_picker_rejects_reserved_name_create() {
        let store = Store::open_memory().expect("memory store");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut area = Category::new("Area".to_string());
        let mut cli = Category::new("CLI".to_string());
        cli.parent = Some(area.id);
        area.children = vec![cli.id];
        for cat in [&area, &cli] {
            store.create_category(cat).expect("create category");
        }
        store
            .create_item(&Item::new("Demo".to_string()))
            .expect("create item");

        let mut view = View::new("Board".to_string());
        view.sections.push(Section {
            title: "Main".to_string(),
            criteria: Query::default(),
            columns: vec![Column {
                kind: ColumnKind::Standard,
                heading: area.id,
                width: 12,
            }],
            item_column_index: 0,
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        });
        store.create_view(&view).expect("create view");

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        app.set_view_selection_by_name("Board");
        app.refresh(&store).expect("refresh board");
        app.column_index = 1;

        app.handle_key(KeyCode::Enter, &agenda)
            .expect("open picker");
        for ch in "Done".chars() {
            app.handle_key(KeyCode::Char(ch), &agenda).expect("type");
        }
        app.handle_key(KeyCode::Enter, &agenda)
            .expect("attempt create");

        let state = app.category_column_picker.as_ref().expect("picker");
        assert_eq!(state.create_confirm_name, None);
        assert!(app.status.contains("reserved"));
    }

    #[test]
    fn category_column_picker_create_cancel_preserves_filter_text() {
        let store = Store::open_memory().expect("memory store");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut area = Category::new("Area".to_string());
        let mut cli = Category::new("CLI".to_string());
        cli.parent = Some(area.id);
        area.children = vec![cli.id];
        for cat in [&area, &cli] {
            store.create_category(cat).expect("create category");
        }
        store
            .create_item(&Item::new("Demo".to_string()))
            .expect("create item");

        let mut view = View::new("Board".to_string());
        view.sections.push(Section {
            title: "Main".to_string(),
            criteria: Query::default(),
            columns: vec![Column {
                kind: ColumnKind::Standard,
                heading: area.id,
                width: 12,
            }],
            item_column_index: 0,
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        });
        store.create_view(&view).expect("create view");

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        app.set_view_selection_by_name("Board");
        app.refresh(&store).expect("refresh board");
        app.column_index = 1;

        app.handle_key(KeyCode::Enter, &agenda)
            .expect("open picker");
        for ch in "NewTag".chars() {
            app.handle_key(KeyCode::Char(ch), &agenda).expect("type");
        }
        app.handle_key(KeyCode::Enter, &agenda)
            .expect("open create confirm");
        app.handle_key(KeyCode::Esc, &agenda)
            .expect("cancel create confirm");

        let state = app.category_column_picker.as_ref().expect("picker");
        assert_eq!(state.create_confirm_name, None);
        assert_eq!(state.filter.text(), "NewTag");
        assert_eq!(app.mode, Mode::CategoryColumnPicker);
    }

    #[test]
    fn normal_mode_plus_opens_add_column_picker_to_right_of_current_column() {
        let store = Store::open_memory().expect("memory store");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let a = Category::new("A".to_string());
        let b = Category::new("B".to_string());
        for cat in [&a, &b] {
            store.create_category(cat).expect("create category");
        }

        let mut view = View::new("Board".to_string());
        view.sections.push(Section {
            title: "Main".to_string(),
            criteria: Query::default(),
            columns: vec![
                Column {
                    kind: ColumnKind::Standard,
                    heading: a.id,
                    width: 12,
                },
                Column {
                    kind: ColumnKind::Standard,
                    heading: b.id,
                    width: 12,
                },
            ],
            item_column_index: 0,
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        });
        store.create_view(&view).expect("create view");

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        app.set_view_selection_by_name("Board");
        app.refresh(&store).expect("refresh board");
        app.column_index = 2; // Board column 2 => section column index 1 (B)

        app.handle_key(KeyCode::Char('+'), &agenda)
            .expect("+ handled");
        assert_eq!(app.mode, Mode::BoardAddColumnPicker);
        let anchor = app.board_add_column.as_ref().expect("picker state").anchor;
        assert_eq!(anchor.direction, AddColumnDirection::Right);
        assert_eq!(anchor.insert_index, 2);
    }

    #[test]
    fn board_item_column_enter_still_opens_item_editor() {
        let store = Store::open_memory().expect("memory store");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let cat = Category::new("Area".to_string());
        store.create_category(&cat).expect("create category");
        store
            .create_item(&Item::new("Demo".to_string()))
            .expect("create item");

        let mut view = View::new("Board".to_string());
        view.sections.push(Section {
            title: "Main".to_string(),
            criteria: Query::default(),
            columns: vec![Column {
                kind: ColumnKind::Standard,
                heading: cat.id,
                width: 12,
            }],
            item_column_index: 0,
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        });
        store.create_view(&view).expect("create view");

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        app.set_view_selection_by_name("Board");
        app.refresh(&store).expect("refresh board");
        app.column_index = 0; // item column

        app.handle_key(KeyCode::Enter, &agenda).expect("enter");
        assert_eq!(app.mode, Mode::InputPanel);
    }

    #[test]
    fn board_when_column_enter_keeps_when_not_implemented_status() {
        let store = Store::open_memory().expect("memory store");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let when_cat = Category::new("When".to_string());
        let item = Item::new("Demo".to_string());

        let mut app = App {
            categories: vec![when_cat.clone()],
            views: vec![{
                let mut view = View::new("Board".to_string());
                view.sections.push(Section {
                    title: "Main".to_string(),
                    criteria: Query::default(),
                    columns: vec![Column {
                        kind: ColumnKind::When,
                        heading: when_cat.id,
                        width: 12,
                    }],
                    item_column_index: 0,
                    on_insert_assign: std::collections::HashSet::new(),
                    on_remove_unassign: std::collections::HashSet::new(),
                    show_children: false,
                    board_display_mode_override: None,
                });
                view
            }],
            slots: vec![super::Slot {
                title: "Main".to_string(),
                items: vec![item],
                context: super::SlotContext::Section { section_index: 0 },
            }],
            view_index: 0,
            slot_index: 0,
            item_index: 0,
            column_index: 1,
            ..App::default()
        };

        app.handle_key(KeyCode::Enter, &agenda).expect("enter");
        assert_eq!(app.mode, Mode::Normal);
        assert!(
            app.status.contains("When' date not yet implemented inline"),
            "unexpected status: {}",
            app.status
        );
    }

    #[test]
    fn normal_mode_plus_opens_add_column_picker_from_item_column_any_position() {
        let store = Store::open_memory().expect("memory store");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let a = Category::new("A".to_string());
        let b = Category::new("B".to_string());
        for cat in [&a, &b] {
            store.create_category(cat).expect("create category");
        }

        let mut view = View::new("Board".to_string());
        view.sections.push(Section {
            title: "Main".to_string(),
            criteria: Query::default(),
            columns: vec![
                Column {
                    kind: ColumnKind::Standard,
                    heading: a.id,
                    width: 12,
                },
                Column {
                    kind: ColumnKind::Standard,
                    heading: b.id,
                    width: 12,
                },
            ],
            item_column_index: 2,
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        });
        store.create_view(&view).expect("create view");

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        app.set_view_selection_by_name("Board");
        app.refresh(&store).expect("refresh board");
        app.column_index = 2; // item column is rightmost

        app.handle_key(KeyCode::Char('+'), &agenda)
            .expect("+ handled on item");
        assert_eq!(app.mode, Mode::BoardAddColumnPicker);
        let anchor = app.board_add_column.as_ref().expect("picker state").anchor;
        assert_eq!(anchor.direction, AddColumnDirection::Right);
        assert_eq!(anchor.insert_index, 2);
    }

    #[test]
    fn board_add_column_picker_insert_right_of_rightmost_item_column_preserves_item_position() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let db_path =
            std::env::temp_dir().join(format!("agenda-tui-add-column-right-of-item-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut priority = Category::new("Priority".to_string());
        let mut p1 = Category::new("P1".to_string());
        p1.parent = Some(priority.id);
        priority.children = vec![p1.id];

        let mut status = Category::new("Status".to_string());
        let mut pending = Category::new("Pending".to_string());
        pending.parent = Some(status.id);
        status.children = vec![pending.id];

        let mut owner = Category::new("Owner".to_string());
        let mut alice = Category::new("Alice".to_string());
        alice.parent = Some(owner.id);
        owner.children = vec![alice.id];

        for cat in [&priority, &p1, &status, &pending, &owner, &alice] {
            store.create_category(cat).expect("create category");
        }

        let mut view = View::new("Board".to_string());
        view.sections.push(Section {
            title: "Main".to_string(),
            criteria: Query::default(),
            columns: vec![
                Column {
                    kind: ColumnKind::Standard,
                    heading: priority.id,
                    width: 12,
                },
                Column {
                    kind: ColumnKind::Standard,
                    heading: status.id,
                    width: 12,
                },
            ],
            item_column_index: 2,
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        });
        store.create_view(&view).expect("create view");

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        app.set_view_selection_by_name("Board");
        app.refresh(&store).expect("refresh board");
        app.column_index = 2; // item column (rightmost)

        app.handle_key(KeyCode::Char('+'), &agenda)
            .expect("open picker from item column");
        for ch in "Owner".chars() {
            app.handle_key(KeyCode::Char(ch), &agenda)
                .expect("type in picker");
        }
        app.handle_key(KeyCode::Enter, &agenda)
            .expect("insert exact-match column");

        assert_eq!(app.mode, Mode::Normal);
        assert_eq!(
            app.column_index, 3,
            "new column selected to the right of item"
        );
        let saved = store
            .get_view(app.current_view().expect("current view").id)
            .expect("saved view");
        let headings: Vec<CategoryId> = saved.sections[0]
            .columns
            .iter()
            .map(|c| c.heading)
            .collect();
        assert_eq!(headings, vec![priority.id, status.id, owner.id]);
        assert_eq!(saved.sections[0].item_column_index, 2);

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn board_add_column_picker_enter_inserts_exact_match_and_persists() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!("agenda-tui-add-column-insert-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let priority = Category::new("Priority".to_string());
        let mut status = Category::new("Status".to_string());
        let mut pending = Category::new("Pending".to_string());
        pending.parent = Some(status.id);
        status.children = vec![pending.id];
        for cat in [&priority, &status, &pending] {
            store.create_category(cat).expect("create category");
        }

        let mut view = View::new("Board".to_string());
        view.sections.push(Section {
            title: "Main".to_string(),
            criteria: Query::default(),
            columns: vec![Column {
                kind: ColumnKind::Standard,
                heading: priority.id,
                width: 12,
            }],
            item_column_index: 0,
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        });
        store.create_view(&view).expect("create view");

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        app.set_view_selection_by_name("Board");
        app.refresh(&store).expect("refresh board");
        app.column_index = 1;

        app.handle_key(KeyCode::Char('+'), &agenda)
            .expect("open add-column picker");
        for ch in "Status".chars() {
            app.handle_key(KeyCode::Char(ch), &agenda)
                .expect("type in picker");
        }
        app.handle_key(KeyCode::Enter, &agenda)
            .expect("insert exact-match column");

        assert_eq!(app.mode, Mode::Normal);
        assert_eq!(app.column_index, 2);
        let saved = store
            .get_view(app.current_view().expect("current view").id)
            .expect("saved view");
        let headings: Vec<CategoryId> = saved.sections[0]
            .columns
            .iter()
            .map(|c| c.heading)
            .collect();
        assert_eq!(headings, vec![priority.id, status.id]);

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn board_add_column_picker_rejects_creating_new_leaf_heading() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!("agenda-tui-add-column-create-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let base = Category::new("Base".to_string());
        store.create_category(&base).expect("create base category");
        let mut view = View::new("Board".to_string());
        view.sections.push(Section {
            title: "Main".to_string(),
            criteria: Query::default(),
            columns: vec![Column {
                kind: ColumnKind::Standard,
                heading: base.id,
                width: 12,
            }],
            item_column_index: 0,
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        });
        store.create_view(&view).expect("create view");

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        app.set_view_selection_by_name("Board");
        app.refresh(&store).expect("refresh board");
        app.column_index = 1;

        app.handle_key(KeyCode::Char('+'), &agenda)
            .expect("open picker");
        for ch in "BrandNew".chars() {
            app.handle_key(KeyCode::Char(ch), &agenda).expect("type");
        }
        app.handle_key(KeyCode::Enter, &agenda)
            .expect("attempt create");
        assert_eq!(app.mode, Mode::BoardAddColumnPicker);
        assert_eq!(app.board_add_column_create_confirm_name(), None);
        assert!(
            app.status.contains("must already have subcategories"),
            "unexpected status: {}",
            app.status
        );

        let saved = store
            .get_view(app.current_view().expect("current view").id)
            .expect("saved view");
        assert_eq!(saved.sections[0].columns.len(), 1);
        assert!(store
            .list_views()
            .expect("views")
            .iter()
            .any(|v| v.name == "Board"));
        assert!(store
            .get_hierarchy()
            .expect("categories")
            .iter()
            .all(|c| !c.name.eq_ignore_ascii_case("BrandNew")));

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn board_add_column_picker_does_not_reuse_child_category_name_as_column_heading() {
        let store = Store::open_memory().expect("memory store");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let status = Category::new("Status".to_string());
        let mut test_child = Category::new("Test".to_string());
        test_child.parent = Some(status.id);
        let mut base = Category::new("Base".to_string());
        let mut base_child = Category::new("BaseChild".to_string());
        base_child.parent = Some(base.id);
        base.children = vec![base_child.id];
        for cat in [&status, &test_child, &base, &base_child] {
            store.create_category(cat).expect("create category");
        }

        let mut view = View::new("Board".to_string());
        view.sections.push(Section {
            title: "Main".to_string(),
            criteria: Query::default(),
            columns: vec![Column {
                kind: ColumnKind::Standard,
                heading: base.id,
                width: 12,
            }],
            item_column_index: 0,
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        });
        store.create_view(&view).expect("create view");

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        app.set_view_selection_by_name("Board");
        app.refresh(&store).expect("refresh board");
        app.column_index = 1;

        app.handle_key(KeyCode::Char('+'), &agenda)
            .expect("open add-column picker");
        for ch in "Test".chars() {
            app.handle_key(KeyCode::Char(ch), &agenda).expect("type");
        }
        app.handle_key(KeyCode::Enter, &agenda)
            .expect("attempt insert/create");

        assert_eq!(app.mode, Mode::BoardAddColumnPicker);
        assert_eq!(app.board_add_column_create_confirm_name(), None);
        assert!(
            app.status.contains("exists under 'Status'"),
            "unexpected status: {}",
            app.status
        );
        let saved = store
            .get_view(app.current_view().expect("current view").id)
            .expect("saved view");
        assert_eq!(saved.sections[0].columns.len(), 1, "no new column inserted");
    }

    #[test]
    fn board_add_column_picker_excludes_existing_section_columns() {
        let store = Store::open_memory().expect("memory store");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut base = Category::new("Base".to_string());
        let mut base_child = Category::new("BaseChild".to_string());
        base_child.parent = Some(base.id);
        base.children = vec![base_child.id];

        let mut status = Category::new("Status".to_string());
        let mut pending = Category::new("Pending".to_string());
        pending.parent = Some(status.id);
        status.children = vec![pending.id];

        let mut priority = Category::new("Priority".to_string());
        let mut high = Category::new("High".to_string());
        high.parent = Some(priority.id);
        priority.children = vec![high.id];

        for cat in [&base, &base_child, &status, &pending, &priority, &high] {
            store.create_category(cat).expect("create category");
        }

        let mut view = View::new("Board".to_string());
        view.sections.push(Section {
            title: "Main".to_string(),
            criteria: Query::default(),
            columns: vec![
                Column {
                    kind: ColumnKind::Standard,
                    heading: base.id,
                    width: 12,
                },
                Column {
                    kind: ColumnKind::Standard,
                    heading: status.id,
                    width: 12,
                },
            ],
            item_column_index: 0,
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        });
        store.create_view(&view).expect("create view");

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        app.set_view_selection_by_name("Board");
        app.refresh(&store).expect("refresh board");
        app.column_index = 1;

        app.handle_key(KeyCode::Char('+'), &agenda)
            .expect("open picker");

        let suggestions = app.get_board_add_column_suggest_matches();
        let names: Vec<String> = suggestions
            .iter()
            .map(|id| {
                app.categories
                    .iter()
                    .find(|c| c.id == *id)
                    .expect("category exists")
                    .name
                    .clone()
            })
            .collect();
        assert!(names.contains(&"Priority".to_string()));
        assert!(!names.contains(&"Base".to_string()));
        assert!(!names.contains(&"Status".to_string()));

        for ch in "Status".chars() {
            app.handle_key(KeyCode::Char(ch), &agenda).expect("type");
        }
        app.handle_key(KeyCode::Enter, &agenda)
            .expect("attempt duplicate section column");
        assert_eq!(app.mode, Mode::BoardAddColumnPicker);
        assert_eq!(app.board_add_column_create_confirm_name(), None);
        assert!(
            app.status.contains("already exists in this section"),
            "unexpected status: {}",
            app.status
        );
    }

    #[test]
    fn board_add_column_picker_render_survives_empty_matches_after_typing() {
        let store = Store::open_memory().expect("memory store");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut base = Category::new("Base".to_string());
        let mut base_child = Category::new("BaseChild".to_string());
        base_child.parent = Some(base.id);
        base.children = vec![base_child.id];
        let mut status = Category::new("Status".to_string());
        let mut pending = Category::new("Pending".to_string());
        pending.parent = Some(status.id);
        status.children = vec![pending.id];
        for cat in [&base, &base_child, &status, &pending] {
            store.create_category(cat).expect("create category");
        }

        let mut view = View::new("Board".to_string());
        view.sections.push(Section {
            title: "Main".to_string(),
            criteria: Query::default(),
            columns: vec![Column {
                kind: ColumnKind::Standard,
                heading: base.id,
                width: 12,
            }],
            item_column_index: 0,
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        });
        store.create_view(&view).expect("create view");
        store
            .create_item(&Item::new("Demo item".to_string()))
            .expect("create item");

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        app.set_view_selection_by_name("Board");
        app.refresh(&store).expect("refresh board");
        app.column_index = 1;

        app.handle_key(KeyCode::Char('+'), &agenda)
            .expect("open picker");
        app.handle_key(KeyCode::Char('z'), &agenda)
            .expect("type no-match filter");
        assert_eq!(app.mode, Mode::BoardAddColumnPicker);
        assert!(app.get_board_add_column_suggest_matches().is_empty());

        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| app.draw(frame))
            .expect("render with empty matches should not panic");
    }

    #[test]
    fn board_column_reorder_visidata_keys_move_item_column() {
        let store = Store::open_memory().expect("memory store");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let a = Category::new("A".to_string());
        let b = Category::new("B".to_string());
        for cat in [&a, &b] {
            store.create_category(cat).expect("create category");
        }

        let mut view = View::new("Board".to_string());
        view.sections.push(Section {
            title: "Main".to_string(),
            criteria: Query::default(),
            columns: vec![
                Column {
                    kind: ColumnKind::Standard,
                    heading: a.id,
                    width: 12,
                },
                Column {
                    kind: ColumnKind::Standard,
                    heading: b.id,
                    width: 12,
                },
            ],
            item_column_index: 1,
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        });
        store.create_view(&view).expect("create view");

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        app.set_view_selection_by_name("Board");
        app.refresh(&store).expect("refresh board");
        app.column_index = 1; // item column

        app.handle_key(KeyCode::Char('H'), &agenda)
            .expect("Shift+H moves item column left");
        let saved = store
            .get_view(app.current_view().expect("current view").id)
            .expect("saved view");
        assert_eq!(saved.sections[0].item_column_index, 0);
        assert_eq!(app.column_index, 0);

        app.handle_key(KeyCode::Char('g'), &agenda)
            .expect("g prefix");
        app.handle_key(KeyCode::Char('L'), &agenda)
            .expect("gL moves item column to end");
        let saved = store
            .get_view(app.current_view().expect("current view").id)
            .expect("saved view");
        assert_eq!(saved.sections[0].item_column_index, 2);
        assert_eq!(app.column_index, 2);
    }

    #[test]
    fn board_column_reorder_and_remove_visidata_keys_update_columns() {
        let store = Store::open_memory().expect("memory store");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let a = Category::new("A".to_string());
        let b = Category::new("B".to_string());
        let c = Category::new("C".to_string());
        for cat in [&a, &b, &c] {
            store.create_category(cat).expect("create category");
        }

        let mut view = View::new("Board".to_string());
        view.sections.push(Section {
            title: "Main".to_string(),
            criteria: Query::default(),
            columns: vec![
                Column {
                    kind: ColumnKind::Standard,
                    heading: a.id,
                    width: 12,
                },
                Column {
                    kind: ColumnKind::Standard,
                    heading: b.id,
                    width: 12,
                },
                Column {
                    kind: ColumnKind::Standard,
                    heading: c.id,
                    width: 12,
                },
            ],
            item_column_index: 1,
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        });
        store.create_view(&view).expect("create view");

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        app.set_view_selection_by_name("Board");
        app.refresh(&store).expect("refresh board");

        app.column_index = 3; // C in [A, Item, B, C]
        app.handle_key(KeyCode::Char('g'), &agenda)
            .expect("g prefix");
        app.handle_key(KeyCode::Char('H'), &agenda)
            .expect("gH moves category to first");

        let saved = store
            .get_view(app.current_view().expect("current view").id)
            .expect("saved view");
        let headings: Vec<CategoryId> = saved.sections[0]
            .columns
            .iter()
            .map(|c| c.heading)
            .collect();
        assert_eq!(headings, vec![c.id, a.id, b.id]);
        assert_eq!(saved.sections[0].item_column_index, 2);
        assert_eq!(app.column_index, 0);

        app.handle_key(KeyCode::Char('-'), &agenda)
            .expect("- opens delete confirmation");
        assert_eq!(app.mode, Mode::BoardColumnDeleteConfirm);
        app.handle_key(KeyCode::Enter, &agenda)
            .expect("Enter confirms delete (default yes)");
        assert_eq!(app.mode, Mode::Normal);
        let saved = store
            .get_view(app.current_view().expect("current view").id)
            .expect("saved view");
        let headings: Vec<CategoryId> = saved.sections[0]
            .columns
            .iter()
            .map(|c| c.heading)
            .collect();
        assert_eq!(headings, vec![a.id, b.id]);
        assert_eq!(saved.sections[0].item_column_index, 1);
        assert_eq!(app.column_index, 0);

        app.column_index = 1; // item column in [A, Item, B]
        app.handle_key(KeyCode::Char('-'), &agenda)
            .expect("- on item column should be blocked");
        assert_eq!(app.mode, Mode::Normal);
        assert_eq!(
            app.status,
            "Cannot delete Item column (move it with H/L or gH/gL)"
        );
    }

    #[test]
    fn board_column_reorder_preserves_slot_focus_when_item_exists_in_multiple_sections() {
        let store = Store::open_memory().expect("memory store");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let a = Category::new("A".to_string());
        let b = Category::new("B".to_string());
        for cat in [&a, &b] {
            store.create_category(cat).expect("create category");
        }

        let mut view = View::new("Board".to_string());
        for title in ["First", "Second"] {
            view.sections.push(Section {
                title: title.to_string(),
                criteria: Query::default(),
                columns: vec![
                    Column {
                        kind: ColumnKind::Standard,
                        heading: a.id,
                        width: 12,
                    },
                    Column {
                        kind: ColumnKind::Standard,
                        heading: b.id,
                        width: 12,
                    },
                ],
                item_column_index: 0,
                on_insert_assign: std::collections::HashSet::new(),
                on_remove_unassign: std::collections::HashSet::new(),
                show_children: false,
                board_display_mode_override: None,
            });
        }
        store.create_view(&view).expect("create view");

        let item = Item::new("Shared item".to_string());
        store.create_item(&item).expect("create item");

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        app.set_view_selection_by_name("Board");
        app.refresh(&store).expect("refresh board");
        assert_eq!(app.slots.len(), 2, "expected 2 sections");
        assert_eq!(app.slots[0].items.len(), 1, "slot 0 item count");
        assert_eq!(app.slots[1].items.len(), 1, "slot 1 item count");

        app.slot_index = 1;
        app.item_index = 0;
        app.column_index = 2; // B in [Item, A, B]

        app.handle_key(KeyCode::Char('H'), &agenda)
            .expect("Shift+H reorders within second section");

        let saved = store
            .get_view(app.current_view().expect("current view").id)
            .expect("saved view");
        let first_headings: Vec<CategoryId> = saved.sections[0]
            .columns
            .iter()
            .map(|c| c.heading)
            .collect();
        let second_headings: Vec<CategoryId> = saved.sections[1]
            .columns
            .iter()
            .map(|c| c.heading)
            .collect();
        assert_eq!(first_headings, vec![a.id, b.id], "first section unchanged");
        assert_eq!(
            second_headings,
            vec![b.id, a.id],
            "second section reordered"
        );
        assert_eq!(app.slot_index, 1, "focus should remain on second section");
        assert_eq!(app.item_index, 0, "shared item selected in second section");
        assert_eq!(app.column_index, 1, "moved column remains selected");
    }

    #[test]
    fn move_slot_cursor_resets_column_index() {
        let mut app = App::default();
        // Setup 2 slots
        app.slots.push(super::Slot {
            title: "A".to_string(),
            items: Vec::new(),
            context: super::SlotContext::Unmatched,
        });
        app.slots.push(super::Slot {
            title: "B".to_string(),
            items: Vec::new(),
            context: super::SlotContext::Unmatched,
        });

        // Move to slot 0, column 1 (simulate)
        app.slot_index = 0;
        app.column_index = 1;

        // Move to slot 1
        app.move_slot_cursor(1);

        assert_eq!(app.slot_index, 1);
        assert_eq!(app.column_index, 0);
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
                mode: mode.clone(),
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
                mode: mode.clone(),
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
        assert!(
            cx >= popup.x,
            "cursor x {} should be >= popup.x {}",
            cx,
            popup.x
        );
        assert!(
            cy >= popup.y,
            "cursor y {} should be >= popup.y {}",
            cy,
            popup.y
        );
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
    fn view_create_name_save_opens_view_edit_directly_with_first_section_editing() {
        let (store, db_path) = make_test_store_with_view("picker-create-direct-editor");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        app.mode = Mode::ViewPicker;

        app.handle_view_picker_key(KeyCode::Char('n'), &agenda)
            .expect("open create name input");
        assert_eq!(app.mode, Mode::InputPanel);
        assert_eq!(app.name_input_context, Some(NameInputContext::ViewCreate));

        for ch in "Mixed".chars() {
            app.handle_input_panel_key(KeyCode::Char(ch), &agenda)
                .expect("type view name");
        }
        app.handle_input_panel_key(KeyCode::Char('S'), &agenda)
            .expect("save name input");

        assert_eq!(app.mode, Mode::ViewEdit);
        let state = app.view_edit_state.as_ref().expect("view edit state");
        assert_eq!(state.region, ViewEditRegion::Sections);
        assert_eq!(state.draft.name, "Mixed");
        assert_eq!(state.draft.criteria.criteria.len(), 0);
        assert_eq!(state.draft.sections.len(), 1);
        assert!(matches!(
            state.inline_input,
            Some(super::ViewEditInlineInput::SectionTitle { section_index: 0 })
        ));

        let created = store
            .list_views()
            .expect("list views")
            .into_iter()
            .find(|view| view.name == "Mixed")
            .expect("created view");
        assert_eq!(created.criteria.criteria.len(), 0);
        assert_eq!(created.sections.len(), 1);

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

        let parent = Category::new("Parent".to_string());
        store.create_category(&parent).expect("create parent");
        let mut alpha = Category::new("Alpha".to_string());
        alpha.parent = Some(parent.id);
        let mut beta = Category::new("Beta".to_string());
        beta.parent = Some(parent.id);
        store.create_category(&alpha).expect("create alpha");
        store.create_category(&beta).expect("create beta");

        let mut view = View::new("Board".to_string());
        let mut section_alpha = Section {
            title: "Alpha".to_string(),
            criteria: Query::default(),
            columns: Vec::new(),
            item_column_index: 0,
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        };
        section_alpha
            .criteria
            .set_criterion(CriterionMode::And, alpha.id);
        let mut section_beta = Section {
            title: "Beta".to_string(),
            criteria: Query::default(),
            columns: Vec::new(),
            item_column_index: 0,
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        };
        section_beta
            .criteria
            .set_criterion(CriterionMode::And, beta.id);
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
    fn category_manager_enter_focuses_details_instead_of_opening_config_editor() {
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
            .expect("focus details");

        assert_eq!(app.mode, Mode::CategoryManager);
        assert_eq!(
            app.category_manager_focus(),
            Some(CategoryManagerFocus::Details)
        );
        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn category_manager_enter_on_reserved_category_stays_inline_in_manager() {
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
            .expect("focus details on reserved");

        assert_eq!(app.mode, Mode::CategoryManager);
        assert_eq!(
            app.category_manager_focus(),
            Some(CategoryManagerFocus::Details)
        );
        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn category_manager_parent_picker_preselects_current_parent_inline() {
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
        assert_eq!(app.mode, Mode::CategoryManager);

        let selected_parent = match app
            .category_manager
            .as_ref()
            .and_then(|s| s.inline_action.as_ref())
        {
            Some(CategoryInlineAction::ParentPicker {
                options,
                visible_option_indices,
                list_index,
                ..
            }) => visible_option_indices
                .get(*list_index)
                .and_then(|idx| options.get(*idx))
                .and_then(|option| option.parent_id),
            _ => None,
        };
        assert_eq!(selected_parent, Some(parent.id));

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn opening_and_closing_category_manager_initializes_and_clears_scaffold_state() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path =
            std::env::temp_dir().join(format!("agenda-tui-category-manager-state-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let category = Category::new("Work".to_string());
        store.create_category(&category).expect("create category");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.set_category_selection_by_id(category.id);

        app.handle_normal_key(KeyCode::Char('c'), &agenda)
            .expect("open category manager");
        assert_eq!(app.mode, Mode::CategoryManager);
        let state = app
            .category_manager
            .as_ref()
            .expect("manager state initialized");
        assert_eq!(state.focus, CategoryManagerFocus::Tree);
        assert_eq!(state.selected_category_id, Some(category.id));
        assert_eq!(state.tree_index, app.category_index);

        app.handle_category_manager_key(KeyCode::Esc, &agenda)
            .expect("close category manager");
        assert_eq!(app.mode, Mode::Normal);
        assert!(app.category_manager.is_none());

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn refresh_preserves_category_manager_selection_by_id() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path =
            std::env::temp_dir().join(format!("agenda-tui-category-manager-refresh-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");

        let parent = Category::new("Parent".to_string());
        store.create_category(&parent).expect("create parent");
        let mut alpha = Category::new("Alpha".to_string());
        alpha.parent = Some(parent.id);
        let mut beta = Category::new("Beta".to_string());
        beta.parent = Some(parent.id);
        store.create_category(&alpha).expect("create alpha");
        store.create_category(&beta).expect("create beta");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.open_category_manager_session();
        app.set_category_selection_by_id(beta.id);
        assert_eq!(
            app.category_manager
                .as_ref()
                .and_then(|state| state.selected_category_id),
            Some(beta.id)
        );

        // Insert another root category and refresh; selection should remain by ID.
        let gamma = Category::new("Gamma".to_string());
        store.create_category(&gamma).expect("create gamma");
        app.refresh(&store).expect("refresh after create");

        assert_eq!(app.selected_category_id(), Some(beta.id));
        assert_eq!(
            app.category_manager
                .as_ref()
                .and_then(|state| state.selected_category_id),
            Some(beta.id)
        );
        assert_eq!(
            app.category_manager.as_ref().map(|state| state.tree_index),
            Some(app.category_index)
        );

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn category_manager_inline_create_root_avoids_input_panel_and_creates_category() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path =
            std::env::temp_dir().join(format!("agenda-tui-category-inline-create-root-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.handle_normal_key(KeyCode::Char('c'), &agenda)
            .expect("open category manager");
        app.handle_category_manager_key(KeyCode::Char('N'), &agenda)
            .expect("start inline create");
        assert_eq!(app.mode, Mode::CategoryManager);
        assert!(
            app.input_panel.is_none(),
            "category create should stay inline"
        );
        assert!(matches!(
            app.category_manager
                .as_ref()
                .and_then(|s| s.inline_action.as_ref()),
            Some(CategoryInlineAction::Create { .. })
        ));

        for c in "Projects".chars() {
            app.handle_category_manager_key(KeyCode::Char(c), &agenda)
                .expect("type create name");
        }
        app.handle_category_manager_key(KeyCode::Enter, &agenda)
            .expect("open create confirm");
        app.handle_category_manager_key(KeyCode::Char('y'), &agenda)
            .expect("confirm create");

        assert!(app
            .categories
            .iter()
            .any(|category| category.name == "Projects"));
        assert!(app
            .category_manager
            .as_ref()
            .and_then(|s| s.inline_action.as_ref())
            .is_none());

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn category_manager_inline_create_child_creates_under_selected_parent() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!(
            "agenda-tui-category-inline-create-child-{nanos}.ag"
        ));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let parent = Category::new("Parent".to_string());
        store.create_category(&parent).expect("create parent");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.handle_normal_key(KeyCode::Char('c'), &agenda)
            .expect("open category manager");
        app.set_category_selection_by_id(parent.id);

        app.handle_category_manager_key(KeyCode::Char('n'), &agenda)
            .expect("start inline child create");
        assert!(app.input_panel.is_none());

        for c in "Child".chars() {
            app.handle_category_manager_key(KeyCode::Char(c), &agenda)
                .expect("type child name");
        }
        app.handle_category_manager_key(KeyCode::Enter, &agenda)
            .expect("open create confirm");
        app.handle_category_manager_key(KeyCode::Char('y'), &agenda)
            .expect("confirm create");

        let child = app
            .categories
            .iter()
            .find(|c| c.name == "Child")
            .expect("child created");
        assert_eq!(child.parent, Some(parent.id));
        assert_eq!(app.mode, Mode::CategoryManager);

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn category_manager_inline_create_rejects_duplicate_name_and_stays_inline() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path =
            std::env::temp_dir().join(format!("agenda-tui-category-inline-create-dup-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        store
            .create_category(&Category::new("Work".to_string()))
            .expect("create existing category");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.handle_normal_key(KeyCode::Char('c'), &agenda)
            .expect("open category manager");
        app.handle_category_manager_key(KeyCode::Char('N'), &agenda)
            .expect("start inline create");
        for c in "Work".chars() {
            app.handle_category_manager_key(KeyCode::Char(c), &agenda)
                .expect("type duplicate name");
        }
        app.handle_category_manager_key(KeyCode::Enter, &agenda)
            .expect("attempt duplicate create");

        assert!(app.status.contains("already exists"));
        assert!(matches!(
            app.category_manager
                .as_ref()
                .and_then(|s| s.inline_action.as_ref()),
            Some(CategoryInlineAction::Create {
                confirm_name: None,
                ..
            })
        ));
        let count = app.categories.iter().filter(|c| c.name == "Work").count();
        assert_eq!(count, 1);

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn category_manager_inline_create_rejects_reserved_name_and_stays_inline() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!(
            "agenda-tui-category-inline-create-reserved-{nanos}.ag"
        ));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.handle_normal_key(KeyCode::Char('c'), &agenda)
            .expect("open category manager");
        app.handle_category_manager_key(KeyCode::Char('N'), &agenda)
            .expect("start inline create");
        for c in "Done".chars() {
            app.handle_category_manager_key(KeyCode::Char(c), &agenda)
                .expect("type reserved name");
        }
        app.handle_category_manager_key(KeyCode::Enter, &agenda)
            .expect("attempt reserved create");

        assert!(app.status.contains("reserved category"));
        assert!(matches!(
            app.category_manager
                .as_ref()
                .and_then(|s| s.inline_action.as_ref()),
            Some(CategoryInlineAction::Create {
                confirm_name: None,
                ..
            })
        ));

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn category_manager_inline_rename_avoids_input_panel_and_updates_name() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path =
            std::env::temp_dir().join(format!("agenda-tui-category-inline-rename-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let category = Category::new("Work".to_string());
        store.create_category(&category).expect("create category");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.handle_normal_key(KeyCode::Char('c'), &agenda)
            .expect("open category manager");
        app.set_category_selection_by_id(category.id);
        app.handle_category_manager_key(KeyCode::Char('r'), &agenda)
            .expect("start rename");
        assert!(
            app.input_panel.is_none(),
            "category rename should stay inline"
        );

        for _ in 0.."Work".len() {
            app.handle_category_manager_key(KeyCode::Backspace, &agenda)
                .expect("clear rename buffer");
        }
        for c in "Office".chars() {
            app.handle_category_manager_key(KeyCode::Char(c), &agenda)
                .expect("type rename");
        }
        app.handle_category_manager_key(KeyCode::Enter, &agenda)
            .expect("apply rename");

        let loaded = store.get_category(category.id).expect("load renamed");
        assert_eq!(loaded.name, "Office");
        assert!(app
            .category_manager
            .as_ref()
            .and_then(|s| s.inline_action.as_ref())
            .is_none());

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn category_manager_inline_rename_unchanged_cancels_cleanly() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!(
            "agenda-tui-category-inline-rename-unchanged-{nanos}.ag"
        ));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let category = Category::new("Work".to_string());
        store.create_category(&category).expect("create category");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.handle_normal_key(KeyCode::Char('c'), &agenda)
            .expect("open category manager");
        app.set_category_selection_by_id(category.id);
        app.handle_category_manager_key(KeyCode::Char('r'), &agenda)
            .expect("start rename");
        app.handle_category_manager_key(KeyCode::Enter, &agenda)
            .expect("apply unchanged rename");

        assert!(app.status.contains("unchanged"));
        assert!(app
            .category_manager
            .as_ref()
            .and_then(|s| s.inline_action.as_ref())
            .is_none());
        assert_eq!(store.get_category(category.id).expect("load").name, "Work");

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn category_manager_inline_rename_reserved_category_is_blocked() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!(
            "agenda-tui-category-inline-rename-reserved-{nanos}.ag"
        ));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.handle_normal_key(KeyCode::Char('c'), &agenda)
            .expect("open category manager");
        let reserved_index = app
            .category_rows
            .iter()
            .position(|row| row.is_reserved)
            .expect("reserved row exists");
        app.category_index = reserved_index;
        app.sync_category_manager_state_from_selection();

        app.handle_category_manager_key(KeyCode::Char('r'), &agenda)
            .expect("attempt rename reserved");

        assert!(app.status.contains("reserved"));
        assert!(app
            .category_manager
            .as_ref()
            .and_then(|s| s.inline_action.as_ref())
            .is_none());

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn category_manager_inline_delete_confirm_stays_in_manager_mode() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path =
            std::env::temp_dir().join(format!("agenda-tui-category-inline-delete-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let category = Category::new("TempDelete".to_string());
        store.create_category(&category).expect("create category");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.handle_normal_key(KeyCode::Char('c'), &agenda)
            .expect("open category manager");
        app.set_category_selection_by_id(category.id);

        app.handle_category_manager_key(KeyCode::Char('x'), &agenda)
            .expect("open inline delete confirm");
        assert_eq!(app.mode, Mode::CategoryManager);
        assert!(matches!(
            app.category_manager
                .as_ref()
                .and_then(|s| s.inline_action.as_ref()),
            Some(CategoryInlineAction::DeleteConfirm { .. })
        ));

        app.handle_category_manager_key(KeyCode::Char('y'), &agenda)
            .expect("confirm delete");
        assert_eq!(app.mode, Mode::CategoryManager);
        assert!(matches!(
            store.get_category(category.id),
            Err(agenda_core::error::AgendaError::NotFound { .. })
        ));
        assert!(app
            .category_manager
            .as_ref()
            .and_then(|s| s.inline_action.as_ref())
            .is_none());

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn category_manager_inline_delete_cancel_keeps_category() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!(
            "agenda-tui-category-inline-delete-cancel-{nanos}.ag"
        ));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let category = Category::new("KeepMe".to_string());
        store.create_category(&category).expect("create category");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.handle_normal_key(KeyCode::Char('c'), &agenda)
            .expect("open category manager");
        app.set_category_selection_by_id(category.id);
        app.handle_category_manager_key(KeyCode::Char('x'), &agenda)
            .expect("start delete");
        app.handle_category_manager_key(KeyCode::Char('n'), &agenda)
            .expect("cancel delete");

        assert!(store.get_category(category.id).is_ok());
        assert!(app.status.contains("Delete canceled"));
        assert!(app
            .category_manager
            .as_ref()
            .and_then(|s| s.inline_action.as_ref())
            .is_none());

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn category_manager_inline_delete_non_leaf_shows_error_and_stays_in_manager() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!(
            "agenda-tui-category-inline-delete-nonleaf-{nanos}.ag"
        ));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let parent = Category::new("ParentDel".to_string());
        store.create_category(&parent).expect("create parent");
        let mut child = Category::new("ChildDel".to_string());
        child.parent = Some(parent.id);
        store.create_category(&child).expect("create child");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.handle_normal_key(KeyCode::Char('c'), &agenda)
            .expect("open category manager");
        app.set_category_selection_by_id(parent.id);
        app.handle_category_manager_key(KeyCode::Char('x'), &agenda)
            .expect("start delete");
        app.handle_category_manager_key(KeyCode::Char('y'), &agenda)
            .expect("confirm delete");

        assert_eq!(app.mode, Mode::CategoryManager);
        assert!(
            store.get_category(parent.id).is_ok(),
            "parent should remain"
        );
        assert!(app.status.contains("Delete failed"));
        assert!(app
            .category_manager
            .as_ref()
            .and_then(|s| s.inline_action.as_ref())
            .is_none());

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn category_manager_filter_focus_types_text_instead_of_triggering_commands() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path =
            std::env::temp_dir().join(format!("agenda-tui-category-filter-focus-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        store
            .create_category(&Category::new("Work".to_string()))
            .expect("create work");
        store
            .create_category(&Category::new("Home".to_string()))
            .expect("create home");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.handle_normal_key(KeyCode::Char('c'), &agenda)
            .expect("open category manager");
        app.handle_category_manager_key(KeyCode::Char('/'), &agenda)
            .expect("focus filter");
        assert_eq!(
            app.category_manager_focus(),
            Some(CategoryManagerFocus::Filter)
        );

        app.handle_category_manager_key(KeyCode::Char('n'), &agenda)
            .expect("type filter n");
        app.handle_category_manager_key(KeyCode::Char('o'), &agenda)
            .expect("type filter o");

        let state = app.category_manager.as_ref().expect("manager state");
        assert_eq!(state.filter.text(), "no");
        assert!(
            state.visible_row_indices.len() < app.category_rows.len(),
            "filter should narrow visible rows"
        );
        assert!(
            state.inline_action.is_none(),
            "typing 'n' in filter focus should not trigger create"
        );

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn category_manager_slash_arms_filter_without_inserting_slash() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path =
            std::env::temp_dir().join(format!("agenda-tui-category-filter-slash-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        store
            .create_category(&Category::new("Work".to_string()))
            .expect("create work");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.handle_normal_key(KeyCode::Char('c'), &agenda)
            .expect("open category manager");

        app.handle_category_manager_key(KeyCode::Char('/'), &agenda)
            .expect("arm filter");
        app.handle_category_manager_key(KeyCode::Char('/'), &agenda)
            .expect("slash again should not insert");

        let state = app.category_manager.as_ref().expect("manager state");
        assert_eq!(state.filter.text(), "");
        assert_eq!(state.focus, CategoryManagerFocus::Filter);
        assert!(state.filter_editing);

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn category_manager_tab_focuses_details_not_filter_and_p_still_opens_parent_picker() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path =
            std::env::temp_dir().join(format!("agenda-tui-category-filter-tab-p-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let parent = Category::new("Parent".to_string());
        let mut child = Category::new("Child".to_string());
        child.parent = Some(parent.id);
        store.create_category(&parent).expect("create parent");
        store.create_category(&child).expect("create child");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.handle_normal_key(KeyCode::Char('c'), &agenda)
            .expect("open category manager");
        app.set_category_selection_by_id(child.id);

        app.handle_category_manager_key(KeyCode::Tab, &agenda)
            .expect("focus details pane");
        assert_eq!(
            app.category_manager_focus(),
            Some(CategoryManagerFocus::Details)
        );
        assert!(!app.category_manager.as_ref().expect("state").filter_editing);

        app.handle_category_manager_key(KeyCode::Char('p'), &agenda)
            .expect("p should open parent picker, not type filter");

        let state = app.category_manager.as_ref().expect("manager state");
        assert_eq!(state.filter.text(), "");
        assert!(matches!(
            state.inline_action.as_ref(),
            Some(CategoryInlineAction::ParentPicker { .. })
        ));

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn category_manager_upper_k_reorders_selected_category_up_among_siblings() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path =
            std::env::temp_dir().join(format!("agenda-tui-category-reorder-up-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let parent = Category::new("Parent".to_string());
        store.create_category(&parent).expect("create parent");
        let mut alpha = Category::new("Alpha".to_string());
        alpha.parent = Some(parent.id);
        let mut beta = Category::new("Beta".to_string());
        beta.parent = Some(parent.id);
        store.create_category(&alpha).expect("create alpha");
        store.create_category(&beta).expect("create beta");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.handle_normal_key(KeyCode::Char('c'), &agenda)
            .expect("open category manager");
        app.set_category_selection_by_id(beta.id);

        app.handle_category_manager_key(KeyCode::Char('K'), &agenda)
            .expect("reorder up");

        let loaded_parent = store.get_category(parent.id).expect("load parent");
        assert_eq!(loaded_parent.children, vec![beta.id, alpha.id]);
        assert_eq!(app.selected_category_id(), Some(beta.id));
        assert!(app.status.contains("Moved Beta up"));

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn category_manager_upper_k_on_first_sibling_is_noop_with_status() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path =
            std::env::temp_dir().join(format!("agenda-tui-category-reorder-boundary-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let parent = Category::new("Parent".to_string());
        store.create_category(&parent).expect("create parent");
        let mut alpha = Category::new("Alpha".to_string());
        alpha.parent = Some(parent.id);
        let mut beta = Category::new("Beta".to_string());
        beta.parent = Some(parent.id);
        store.create_category(&alpha).expect("create alpha");
        store.create_category(&beta).expect("create beta");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.handle_normal_key(KeyCode::Char('c'), &agenda)
            .expect("open category manager");
        app.set_category_selection_by_id(alpha.id);

        app.handle_category_manager_key(KeyCode::Char('K'), &agenda)
            .expect("reorder boundary noop");

        let loaded_parent = store.get_category(parent.id).expect("load parent");
        assert_eq!(loaded_parent.children, vec![alpha.id, beta.id]);
        assert!(app.status.contains("already first"));
        assert_eq!(app.selected_category_id(), Some(alpha.id));

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn category_manager_upper_j_reorders_selected_category_down_among_siblings() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path =
            std::env::temp_dir().join(format!("agenda-tui-category-reorder-down-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let parent = Category::new("Parent".to_string());
        store.create_category(&parent).expect("create parent");
        let mut alpha = Category::new("Alpha".to_string());
        alpha.parent = Some(parent.id);
        let mut beta = Category::new("Beta".to_string());
        beta.parent = Some(parent.id);
        store.create_category(&alpha).expect("create alpha");
        store.create_category(&beta).expect("create beta");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.handle_normal_key(KeyCode::Char('c'), &agenda)
            .expect("open category manager");
        app.set_category_selection_by_id(alpha.id);

        app.handle_category_manager_key(KeyCode::Char('J'), &agenda)
            .expect("reorder down");

        let loaded_parent = store.get_category(parent.id).expect("load parent");
        assert_eq!(loaded_parent.children, vec![beta.id, alpha.id]);
        assert_eq!(app.selected_category_id(), Some(alpha.id));
        assert!(app.status.contains("Moved Alpha down"));

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn category_manager_upper_l_indents_selected_category_under_previous_sibling() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!("agenda-tui-category-indent-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let alpha = Category::new("Alpha".to_string());
        let beta = Category::new("Beta".to_string());
        let gamma = Category::new("Gamma".to_string());
        store.create_category(&alpha).expect("create alpha");
        store.create_category(&beta).expect("create beta");
        store.create_category(&gamma).expect("create gamma");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.handle_normal_key(KeyCode::Char('c'), &agenda)
            .expect("open category manager");
        app.set_category_selection_by_id(beta.id);

        app.handle_category_manager_key(KeyCode::Char('L'), &agenda)
            .expect("indent under previous sibling");

        let loaded_alpha = store.get_category(alpha.id).expect("load alpha");
        let loaded_beta = store.get_category(beta.id).expect("load beta");
        assert_eq!(loaded_beta.parent, Some(alpha.id));
        assert_eq!(loaded_alpha.children, vec![beta.id]);
        let root_ids: Vec<_> = app
            .categories
            .iter()
            .filter(|c| c.parent.is_none())
            .map(|c| c.id)
            .collect();
        assert!(!root_ids.contains(&beta.id));
        assert_eq!(app.selected_category_id(), Some(beta.id));
        assert!(app.status.contains("Indented Beta under Alpha"));

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn category_manager_upper_l_on_first_sibling_is_noop_with_status() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path =
            std::env::temp_dir().join(format!("agenda-tui-category-indent-boundary-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let parent = Category::new("Parent".to_string());
        store.create_category(&parent).expect("create parent");
        let mut alpha = Category::new("Alpha".to_string());
        alpha.parent = Some(parent.id);
        let mut beta = Category::new("Beta".to_string());
        beta.parent = Some(parent.id);
        store.create_category(&alpha).expect("create alpha");
        store.create_category(&beta).expect("create beta");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.handle_normal_key(KeyCode::Char('c'), &agenda)
            .expect("open category manager");
        app.set_category_selection_by_id(alpha.id);

        app.handle_category_manager_key(KeyCode::Char('L'), &agenda)
            .expect("indent boundary noop");

        assert_eq!(
            store.get_category(alpha.id).expect("alpha").parent,
            Some(parent.id)
        );
        assert_eq!(
            store.get_category(parent.id).expect("parent").children,
            vec![alpha.id, beta.id]
        );
        assert!(app.status.contains("no previous sibling"));
        assert_eq!(app.selected_category_id(), Some(alpha.id));

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn category_manager_upper_h_outdents_selected_category_after_parent() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!("agenda-tui-category-outdent-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let grandparent = Category::new("Grandparent".to_string());
        store
            .create_category(&grandparent)
            .expect("create grandparent");

        let mut parent = Category::new("Parent".to_string());
        parent.parent = Some(grandparent.id);
        store.create_category(&parent).expect("create parent");

        let mut uncle = Category::new("Uncle".to_string());
        uncle.parent = Some(grandparent.id);
        store.create_category(&uncle).expect("create uncle");

        let mut child = Category::new("Child".to_string());
        child.parent = Some(parent.id);
        store.create_category(&child).expect("create child");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.handle_normal_key(KeyCode::Char('c'), &agenda)
            .expect("open category manager");
        app.set_category_selection_by_id(child.id);

        app.handle_category_manager_key(KeyCode::Char('H'), &agenda)
            .expect("outdent");

        let loaded_gp = store
            .get_category(grandparent.id)
            .expect("load grandparent");
        let loaded_parent = store.get_category(parent.id).expect("load parent");
        let loaded_child = store.get_category(child.id).expect("load child");
        assert_eq!(loaded_child.parent, Some(grandparent.id));
        assert!(loaded_parent.children.is_empty());
        assert_eq!(loaded_gp.children, vec![parent.id, child.id, uncle.id]);
        assert_eq!(app.selected_category_id(), Some(child.id));
        assert!(app.status.contains("Outdented Child"));

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn category_manager_upper_h_outdents_child_to_root() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path =
            std::env::temp_dir().join(format!("agenda-tui-category-outdent-to-root-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let parent = Category::new("Parent".to_string());
        let sibling = Category::new("Sibling".to_string());
        store.create_category(&parent).expect("create parent");
        store.create_category(&sibling).expect("create sibling");

        let mut child = Category::new("Child".to_string());
        child.parent = Some(parent.id);
        store.create_category(&child).expect("create child");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.handle_normal_key(KeyCode::Char('c'), &agenda)
            .expect("open category manager");
        app.set_category_selection_by_id(child.id);

        app.handle_category_manager_key(KeyCode::Char('H'), &agenda)
            .expect("outdent to root");

        let loaded_child = store.get_category(child.id).expect("load child");
        let loaded_parent = store.get_category(parent.id).expect("load parent");
        assert_eq!(loaded_child.parent, None);
        assert!(loaded_parent.children.is_empty());
        let root_ids: Vec<_> = app
            .categories
            .iter()
            .filter(|c| c.parent.is_none())
            .map(|c| c.id)
            .collect();
        let parent_pos = root_ids
            .iter()
            .position(|id| *id == parent.id)
            .expect("parent in roots");
        let child_pos = root_ids
            .iter()
            .position(|id| *id == child.id)
            .expect("child in roots");
        assert_eq!(child_pos, parent_pos + 1);
        assert_eq!(app.selected_category_id(), Some(child.id));
        assert!(app.status.contains("Outdented Child to top level"));

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn category_manager_upper_h_on_root_is_noop_with_status() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path =
            std::env::temp_dir().join(format!("agenda-tui-category-outdent-root-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let root = Category::new("Root".to_string());
        store.create_category(&root).expect("create root");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.handle_normal_key(KeyCode::Char('c'), &agenda)
            .expect("open category manager");
        app.set_category_selection_by_id(root.id);

        app.handle_category_manager_key(KeyCode::Char('H'), &agenda)
            .expect("outdent noop");

        assert_eq!(store.get_category(root.id).expect("root").parent, None);
        assert!(app.status.contains("already at the top level"));
        assert_eq!(app.selected_category_id(), Some(root.id));

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn category_manager_direct_moves_are_blocked_while_filter_active() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path =
            std::env::temp_dir().join(format!("agenda-tui-category-move-filter-block-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let alpha = Category::new("Alpha".to_string());
        let beta = Category::new("Beta".to_string());
        store.create_category(&alpha).expect("create alpha");
        store.create_category(&beta).expect("create beta");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.handle_normal_key(KeyCode::Char('c'), &agenda)
            .expect("open category manager");
        app.set_category_selection_by_id(alpha.id);
        app.handle_category_manager_key(KeyCode::Char('/'), &agenda)
            .expect("focus filter");
        app.handle_category_manager_key(KeyCode::Char('a'), &agenda)
            .expect("type filter");
        app.set_category_manager_focus(CategoryManagerFocus::Tree);

        app.handle_category_manager_key(KeyCode::Char('J'), &agenda)
            .expect("move should be blocked");

        let root_names: Vec<_> = app
            .categories
            .iter()
            .filter(|c| c.parent.is_none() && (c.name == "Alpha" || c.name == "Beta"))
            .map(|c| c.name.clone())
            .collect();
        assert_eq!(root_names, vec!["Alpha".to_string(), "Beta".to_string()]);
        assert!(app
            .status
            .contains("Clear category filter before direct H/L/J/K moves"));

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn category_manager_parent_picker_filters_and_reparents_selected_category() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!(
            "agenda-tui-category-parent-picker-apply-{nanos}.ag"
        ));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let alpha = Category::new("Alpha".to_string());
        let beta = Category::new("Beta".to_string());
        store.create_category(&alpha).expect("create alpha");
        store.create_category(&beta).expect("create beta");
        let mut child = Category::new("Child".to_string());
        child.parent = Some(alpha.id);
        store.create_category(&child).expect("create child");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.handle_normal_key(KeyCode::Char('c'), &agenda)
            .expect("open category manager");
        app.set_category_selection_by_id(child.id);

        app.handle_category_manager_key(KeyCode::Char('p'), &agenda)
            .expect("open parent picker");
        app.handle_category_manager_key(KeyCode::Char('/'), &agenda)
            .expect("focus parent filter");
        for c in "Beta".chars() {
            app.handle_category_manager_key(KeyCode::Char(c), &agenda)
                .expect("type parent filter");
        }
        app.handle_category_manager_key(KeyCode::Tab, &agenda)
            .expect("focus parent list");
        app.handle_category_manager_key(KeyCode::Enter, &agenda)
            .expect("apply parent picker reparent");

        let loaded_child = store.get_category(child.id).expect("load child");
        assert_eq!(loaded_child.parent, Some(beta.id));
        assert_eq!(app.mode, Mode::CategoryManager);
        assert_eq!(app.selected_category_id(), Some(child.id));
        assert!(app
            .category_manager
            .as_ref()
            .and_then(|s| s.inline_action.as_ref())
            .is_none());
        assert!(app.status.contains("Reparented Child to Beta"));

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn category_manager_parent_picker_filter_requires_slash_and_slash_is_not_inserted() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!(
            "agenda-tui-category-parent-picker-filter-slash-{nanos}.ag"
        ));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let alpha = Category::new("Alpha".to_string());
        let beta = Category::new("Beta".to_string());
        store.create_category(&alpha).expect("create alpha");
        store.create_category(&beta).expect("create beta");
        let mut child = Category::new("Child".to_string());
        child.parent = Some(alpha.id);
        store.create_category(&child).expect("create child");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.handle_normal_key(KeyCode::Char('c'), &agenda)
            .expect("open category manager");
        app.set_category_selection_by_id(child.id);
        app.handle_category_manager_key(KeyCode::Char('p'), &agenda)
            .expect("open parent picker");

        app.handle_category_manager_key(KeyCode::Tab, &agenda)
            .expect("focus parent filter pane");
        app.handle_category_manager_key(KeyCode::Char('B'), &agenda)
            .expect("typing before slash should not edit picker filter");

        let (filter_before, editing_before) = match app
            .category_manager
            .as_ref()
            .and_then(|s| s.inline_action.as_ref())
        {
            Some(CategoryInlineAction::ParentPicker {
                filter,
                filter_editing,
                ..
            }) => (filter.text().to_string(), *filter_editing),
            _ => panic!("expected inline parent picker"),
        };
        assert_eq!(filter_before, "");
        assert!(!editing_before);

        app.handle_category_manager_key(KeyCode::Char('/'), &agenda)
            .expect("arm parent filter");
        app.handle_category_manager_key(KeyCode::Char('/'), &agenda)
            .expect("slash should not insert into picker filter");
        app.handle_category_manager_key(KeyCode::Char('B'), &agenda)
            .expect("type parent filter");

        let (filter_after, editing_after) = match app
            .category_manager
            .as_ref()
            .and_then(|s| s.inline_action.as_ref())
        {
            Some(CategoryInlineAction::ParentPicker {
                filter,
                filter_editing,
                ..
            }) => (filter.text().to_string(), *filter_editing),
            _ => panic!("expected inline parent picker"),
        };
        assert_eq!(filter_after, "B");
        assert!(editing_after);

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn category_manager_parent_picker_excludes_descendants_to_prevent_cycles() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!(
            "agenda-tui-category-parent-picker-cycles-{nanos}.ag"
        ));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let root = Category::new("Root".to_string());
        store.create_category(&root).expect("create root");
        let mut parent = Category::new("Parent".to_string());
        parent.parent = Some(root.id);
        store.create_category(&parent).expect("create parent");
        let mut child = Category::new("Child".to_string());
        child.parent = Some(parent.id);
        store.create_category(&child).expect("create child");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.handle_normal_key(KeyCode::Char('c'), &agenda)
            .expect("open category manager");
        app.set_category_selection_by_id(parent.id);

        app.handle_category_manager_key(KeyCode::Char('p'), &agenda)
            .expect("open parent picker");

        let option_parent_ids: Vec<Option<CategoryId>> = match app
            .category_manager
            .as_ref()
            .and_then(|s| s.inline_action.as_ref())
        {
            Some(CategoryInlineAction::ParentPicker { options, .. }) => {
                options.iter().map(|option| option.parent_id).collect()
            }
            _ => panic!("expected inline parent picker"),
        };

        assert!(!option_parent_ids.contains(&Some(parent.id)));
        assert!(!option_parent_ids.contains(&Some(child.id)));
        assert!(option_parent_ids.contains(&None));
        assert!(option_parent_ids.contains(&Some(root.id)));

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn category_manager_details_note_edit_autosaves_on_tab_and_allows_capital_s() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path =
            std::env::temp_dir().join(format!("agenda-tui-category-details-note-save-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let category = Category::new("Work".to_string());
        store.create_category(&category).expect("create category");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.handle_normal_key(KeyCode::Char('c'), &agenda)
            .expect("open category manager");
        app.set_category_selection_by_id(category.id);
        app.set_category_manager_focus(CategoryManagerFocus::Details);
        app.set_category_manager_details_focus(CategoryManagerDetailsFocus::Note);

        for c in "Ship".chars() {
            app.handle_category_manager_key(KeyCode::Char(c), &agenda)
                .expect("type note");
        }
        assert!(app.category_manager_details_note_editing());
        app.handle_category_manager_key(KeyCode::Tab, &agenda)
            .expect("autosave note on tab");

        let saved = store.get_category(category.id).expect("load category");
        assert_eq!(saved.note.as_deref(), Some("Ship"));
        assert_eq!(app.mode, Mode::CategoryManager);
        assert_eq!(
            app.category_manager_focus(),
            Some(CategoryManagerFocus::Tree)
        );
        assert!(!app.category_manager_details_note_editing());
        assert!(!app.category_manager_details_note_dirty());

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn category_manager_note_focus_lowercase_j_starts_note_edit() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path =
            std::env::temp_dir().join(format!("agenda-tui-category-details-note-j-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let category = Category::new("Work".to_string());
        store.create_category(&category).expect("create category");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.handle_normal_key(KeyCode::Char('c'), &agenda)
            .expect("open category manager");
        app.set_category_selection_by_id(category.id);
        app.set_category_manager_focus(CategoryManagerFocus::Details);
        app.set_category_manager_details_focus(CategoryManagerDetailsFocus::Note);

        app.handle_category_manager_key(KeyCode::Char('j'), &agenda)
            .expect("type lowercase j in note");
        assert!(app.category_manager_details_note_editing());
        assert_eq!(app.category_manager_details_note_text(), Some("j"));

        app.handle_category_manager_key(KeyCode::Esc, &agenda)
            .expect("autosave note");
        let saved = store.get_category(category.id).expect("load category");
        assert_eq!(saved.note.as_deref(), Some("j"));

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn category_manager_details_note_edit_esc_autosaves_inline_draft() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path =
            std::env::temp_dir().join(format!("agenda-tui-category-details-note-esc-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut category = Category::new("Work".to_string());
        category.note = Some("seed".to_string());
        store.create_category(&category).expect("create category");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.handle_normal_key(KeyCode::Char('c'), &agenda)
            .expect("open category manager");
        app.set_category_selection_by_id(category.id);
        app.set_category_manager_focus(CategoryManagerFocus::Details);
        app.set_category_manager_details_focus(CategoryManagerDetailsFocus::Note);

        app.handle_category_manager_key(KeyCode::Enter, &agenda)
            .expect("begin note edit");
        app.handle_category_manager_key(KeyCode::Char('!'), &agenda)
            .expect("type note");
        assert!(app.category_manager_details_note_dirty());

        app.handle_category_manager_key(KeyCode::Esc, &agenda)
            .expect("autosave note edit on esc");

        let loaded = store.get_category(category.id).expect("load category");
        assert_eq!(loaded.note.as_deref(), Some("seed!"));
        assert_eq!(app.category_manager_details_note_text(), Some("seed!"));
        assert!(!app.category_manager_details_note_editing());
        assert!(!app.category_manager_details_note_dirty());
        assert!(
            !app.status.contains("Saved note for Work"),
            "autosave should be quiet in status line"
        );

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn category_manager_details_dirty_note_autosaves_on_selection_change() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!(
            "agenda-tui-category-details-note-selection-change-{nanos}.ag"
        ));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let alpha = Category::new("Alpha".to_string());
        let beta = Category::new("Beta".to_string());
        store.create_category(&alpha).expect("create alpha");
        store.create_category(&beta).expect("create beta");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.handle_normal_key(KeyCode::Char('c'), &agenda)
            .expect("open category manager");
        app.set_category_selection_by_id(alpha.id);
        app.set_category_manager_focus(CategoryManagerFocus::Details);
        app.set_category_manager_details_focus(CategoryManagerDetailsFocus::Note);

        app.handle_category_manager_key(KeyCode::Enter, &agenda)
            .expect("begin note edit");
        app.handle_category_manager_key(KeyCode::Char('x'), &agenda)
            .expect("type note");
        assert!(app.category_manager_details_note_dirty());

        app.set_category_manager_focus(CategoryManagerFocus::Tree);
        app.handle_category_manager_key(KeyCode::Char('j'), &agenda)
            .expect("move selection");

        assert_eq!(app.selected_category_id(), Some(beta.id));
        assert!(
            !app.status.contains("Saved note for Alpha"),
            "autosave on selection change should be quiet"
        );
        assert_eq!(
            store.get_category(alpha.id).expect("alpha").note.as_deref(),
            Some("x")
        );
        assert_eq!(app.category_manager_details_note_text(), Some(""));
        assert!(!app.category_manager_details_note_dirty());

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn category_manager_details_and_quick_flag_toggles_work_inline() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!(
            "agenda-tui-category-details-flag-toggles-{nanos}.ag"
        ));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let category = Category::new("Work".to_string());
        let initial = category.clone();
        store.create_category(&category).expect("create category");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.handle_normal_key(KeyCode::Char('c'), &agenda)
            .expect("open category manager");
        app.set_category_selection_by_id(category.id);
        app.set_category_manager_focus(CategoryManagerFocus::Details);
        app.set_category_manager_details_focus(CategoryManagerDetailsFocus::Exclusive);

        app.handle_category_manager_key(KeyCode::Enter, &agenda)
            .expect("toggle exclusive from details");
        app.handle_category_manager_key(KeyCode::Char('i'), &agenda)
            .expect("quick toggle match-name");
        app.handle_category_manager_key(KeyCode::Char('a'), &agenda)
            .expect("quick toggle actionable");

        let loaded = store.get_category(category.id).expect("load category");
        assert_eq!(loaded.is_exclusive, !initial.is_exclusive);
        assert_eq!(
            loaded.enable_implicit_string,
            !initial.enable_implicit_string
        );
        assert_eq!(loaded.is_actionable, !initial.is_actionable);
        assert_eq!(app.mode, Mode::CategoryManager);

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn category_manager_details_jk_navigates_fields_without_moving_category_selection() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!(
            "agenda-tui-category-details-jk-navigation-{nanos}.ag"
        ));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let alpha = Category::new("Alpha".to_string());
        let beta = Category::new("Beta".to_string());
        store.create_category(&alpha).expect("create alpha");
        store.create_category(&beta).expect("create beta");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.handle_normal_key(KeyCode::Char('c'), &agenda)
            .expect("open category manager");
        app.set_category_selection_by_id(alpha.id);
        app.set_category_manager_focus(CategoryManagerFocus::Details);
        app.set_category_manager_details_focus(CategoryManagerDetailsFocus::Exclusive);

        app.handle_category_manager_key(KeyCode::Char('j'), &agenda)
            .expect("details next field");
        assert_eq!(
            app.category_manager_details_focus(),
            Some(CategoryManagerDetailsFocus::MatchName)
        );
        assert_eq!(app.selected_category_id(), Some(alpha.id));

        app.handle_category_manager_key(KeyCode::Char('k'), &agenda)
            .expect("details previous field");
        assert_eq!(
            app.category_manager_details_focus(),
            Some(CategoryManagerDetailsFocus::Exclusive)
        );
        assert_eq!(app.selected_category_id(), Some(alpha.id));

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn category_manager_details_blocks_shift_hjkl_structure_moves() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path =
            std::env::temp_dir().join(format!("agenda-tui-category-details-shift-hjkl-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let parent = Category::new("Parent".to_string());
        store.create_category(&parent).expect("create parent");
        let mut alpha = Category::new("Alpha".to_string());
        alpha.parent = Some(parent.id);
        let mut beta = Category::new("Beta".to_string());
        beta.parent = Some(parent.id);
        store.create_category(&alpha).expect("create alpha");
        store.create_category(&beta).expect("create beta");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.handle_normal_key(KeyCode::Char('c'), &agenda)
            .expect("open category manager");
        app.set_category_selection_by_id(beta.id);
        app.set_category_manager_focus(CategoryManagerFocus::Details);
        app.set_category_manager_details_focus(CategoryManagerDetailsFocus::Exclusive);

        app.handle_category_manager_key(KeyCode::Char('J'), &agenda)
            .expect("shift-j ignored in details");
        app.handle_category_manager_key(KeyCode::Char('H'), &agenda)
            .expect("shift-h ignored in details");
        app.handle_category_manager_key(KeyCode::Char('L'), &agenda)
            .expect("shift-l ignored in details");
        app.handle_category_manager_key(KeyCode::Char('K'), &agenda)
            .expect("shift-k ignored in details");

        let parent_loaded = store.get_category(parent.id).expect("load parent");
        assert_eq!(parent_loaded.children, vec![alpha.id, beta.id]);
        assert_eq!(app.selected_category_id(), Some(beta.id));
        assert_eq!(
            app.category_manager_focus(),
            Some(CategoryManagerFocus::Details)
        );
        assert_eq!(
            app.category_manager_details_focus(),
            Some(CategoryManagerDetailsFocus::Exclusive)
        );

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn normal_mode_ga_jumps_to_all_items_view() {
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
            .expect("g prefix should start");
        assert_eq!(
            app.current_view().map(|view| view.name.as_str()),
            Some("Work Board")
        );
        app.handle_normal_key(KeyCode::Char('a'), &agenda)
            .expect("ga should jump to all items view");
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
            item_column_index: 0,
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
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
            item_column_index: 0,
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
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
    fn view_edit_tab_cycles_panes() {
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
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().pane_focus,
            ViewEditPaneFocus::Details
        );

        app.handle_view_edit_key(KeyCode::Tab, &agenda)
            .expect("tab");
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().pane_focus,
            ViewEditPaneFocus::Sections
        );
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().region,
            ViewEditRegion::Criteria
        );

        app.handle_view_edit_key(KeyCode::Tab, &agenda)
            .expect("tab");
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().pane_focus,
            ViewEditPaneFocus::Details
        );
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().region,
            ViewEditRegion::Criteria
        );

        app.handle_view_edit_key(KeyCode::Tab, &agenda)
            .expect("tab wraps");
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().pane_focus,
            ViewEditPaneFocus::Sections
        );

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn view_edit_shift_tab_cycles_panes_backwards() {
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
            app.view_edit_state.as_ref().unwrap().pane_focus,
            ViewEditPaneFocus::Sections
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
    fn view_edit_esc_on_dirty_prompts_before_cancel() {
        let (store, db_path) = make_test_store_with_view("esc-dirty-confirm");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        let view = app.views[0].clone();
        app.open_view_edit(view);

        app.handle_view_edit_key(KeyCode::Char('m'), &agenda)
            .expect("toggle view display mode");
        app.handle_view_edit_key(KeyCode::Esc, &agenda)
            .expect("esc prompts");
        assert_eq!(app.mode, Mode::ViewEdit);
        assert!(app
            .view_edit_state
            .as_ref()
            .map(|s| s.discard_confirm)
            .unwrap_or(false));

        app.handle_view_edit_key(KeyCode::Char('n'), &agenda)
            .expect("decline discard");
        assert!(app.view_edit_state.is_some());
        assert!(!app
            .view_edit_state
            .as_ref()
            .map(|s| s.discard_confirm)
            .unwrap_or(true));

        app.handle_view_edit_key(KeyCode::Esc, &agenda)
            .expect("esc prompts again");
        app.handle_view_edit_key(KeyCode::Char('y'), &agenda)
            .expect("confirm discard");
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
            item_column_index: 0,
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        });
        app.open_view_edit(view);

        // Move to Sections pane, then select first section row
        app.handle_view_edit_key(KeyCode::Tab, &agenda)
            .expect("tab");
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().pane_focus,
            ViewEditPaneFocus::Sections
        );
        app.handle_view_edit_key(KeyCode::Char('j'), &agenda)
            .expect("move to first section");
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
    fn view_edit_details_jk_moves_between_criteria_and_unmatched_rows() {
        let (store, db_path) = make_test_store_with_view("details-jk-view-rows");
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

        app.handle_view_edit_key(KeyCode::Char('j'), &agenda)
            .expect("criteria -> unmatched visible");
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().region,
            ViewEditRegion::Unmatched
        );
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().unmatched_field_index,
            0
        );

        app.handle_view_edit_key(KeyCode::Char('j'), &agenda)
            .expect("unmatched visible -> unmatched label");
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().region,
            ViewEditRegion::Unmatched
        );
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().unmatched_field_index,
            1
        );

        app.handle_view_edit_key(KeyCode::Char('k'), &agenda)
            .expect("unmatched label -> unmatched visible");
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().region,
            ViewEditRegion::Unmatched
        );
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().unmatched_field_index,
            0
        );

        app.handle_view_edit_key(KeyCode::Char('k'), &agenda)
            .expect("unmatched visible -> criteria");
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().region,
            ViewEditRegion::Criteria
        );

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn view_edit_unmatched_enter_uses_selected_details_row() {
        let (store, db_path) = make_test_store_with_view("unmatched-enter-detail-row");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        let view = app.views[0].clone();
        app.open_view_edit(view);

        app.handle_view_edit_key(KeyCode::Char('j'), &agenda)
            .expect("to unmatched visible row");
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().region,
            ViewEditRegion::Unmatched
        );
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().unmatched_field_index,
            0
        );

        let before_visible = app.view_edit_state.as_ref().unwrap().draft.show_unmatched;
        app.handle_view_edit_key(KeyCode::Enter, &agenda)
            .expect("toggle unmatched visible via enter");
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().draft.show_unmatched,
            !before_visible
        );

        app.handle_view_edit_key(KeyCode::Char('j'), &agenda)
            .expect("move to unmatched label row");
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().unmatched_field_index,
            1
        );

        app.handle_view_edit_key(KeyCode::Enter, &agenda)
            .expect("begin unmatched label edit");
        assert!(matches!(
            app.view_edit_state.as_ref().unwrap().inline_input,
            Some(super::ViewEditInlineInput::UnmatchedLabel)
        ));

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
            .mode_for(work.id)
            .is_some());

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
            .mode_for(home.id)
            .is_some());
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
            app.view_edit_state.as_ref().unwrap().pane_focus,
            ViewEditPaneFocus::Sections
        );

        app.handle_view_edit_key(KeyCode::Char('n'), &agenda)
            .expect("add section");
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().draft.sections.len(),
            1
        );
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().section_expanded,
            Some(0)
        );
        assert!(matches!(
            app.view_edit_state.as_ref().unwrap().inline_input,
            Some(super::ViewEditInlineInput::SectionTitle { section_index: 0 })
        ));
        app.handle_view_edit_key(KeyCode::Enter, &agenda)
            .expect("confirm default section title");

        app.handle_view_edit_key(KeyCode::Char('f'), &agenda)
            .expect("open section criteria picker");
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().section_expanded,
            Some(0)
        );
        assert!(matches!(
            app.view_edit_state.as_ref().unwrap().overlay,
            Some(super::ViewEditOverlay::CategoryPicker {
                target: super::CategoryEditTarget::SectionCriteria
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
        app.handle_view_edit_key(KeyCode::Enter, &agenda)
            .expect("confirm default section title");
        app.handle_view_edit_key(KeyCode::Char('e'), &agenda)
            .expect("start section title edit");

        assert!(matches!(
            app.view_edit_state.as_ref().unwrap().inline_input,
            Some(super::ViewEditInlineInput::SectionTitle { section_index: 0 })
        ));

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn view_edit_section_uppercase_n_adds_and_starts_section_title_edit() {
        let (store, db_path) = make_test_store_with_view("section-uppercase-n-adds-edit");
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
        app.handle_view_edit_key(KeyCode::Char('N'), &agenda)
            .expect("add section and start title edit");

        let state = app.view_edit_state.as_ref().expect("view edit state");
        assert_eq!(state.draft.sections.len(), 1);
        assert_eq!(state.region, ViewEditRegion::Sections);
        assert_eq!(state.section_index, 0);
        assert!(matches!(
            state.inline_input,
            Some(super::ViewEditInlineInput::SectionTitle { section_index: 0 })
        ));

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn view_edit_section_lowercase_n_inserts_below_current_and_starts_title_edit() {
        let (store, db_path) = make_test_store_with_view("section-lowercase-n-insert-below");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        let mut view = app
            .views
            .iter()
            .find(|v| v.name == "TestView")
            .cloned()
            .expect("TestView should exist");
        view.sections.push(Section {
            title: "Alpha".to_string(),
            criteria: Query::default(),
            columns: Vec::new(),
            item_column_index: 0,
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        });
        view.sections.push(Section {
            title: "Bravo".to_string(),
            criteria: Query::default(),
            columns: Vec::new(),
            item_column_index: 0,
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        });
        app.open_view_edit(view);

        app.handle_view_edit_key(KeyCode::Tab, &agenda)
            .expect("tab");
        app.handle_view_edit_key(KeyCode::Char('j'), &agenda)
            .expect("select first section");
        if let Some(state) = &mut app.view_edit_state {
            state.section_index = 0;
        }

        app.handle_view_edit_key(KeyCode::Char('n'), &agenda)
            .expect("insert below");

        let state = app.view_edit_state.as_ref().expect("view edit state");
        assert_eq!(state.section_index, 1);
        assert_eq!(state.draft.sections.len(), 3);
        assert_eq!(state.draft.sections[0].title, "Alpha");
        assert_eq!(state.draft.sections[1].title, "New section");
        assert_eq!(state.draft.sections[2].title, "Bravo");
        assert!(matches!(
            state.inline_input,
            Some(super::ViewEditInlineInput::SectionTitle { section_index: 1 })
        ));

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn view_edit_section_uppercase_n_inserts_above_current_and_starts_title_edit() {
        let (store, db_path) = make_test_store_with_view("section-uppercase-n-insert-above");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        let mut view = app
            .views
            .iter()
            .find(|v| v.name == "TestView")
            .cloned()
            .expect("TestView should exist");
        view.sections.push(Section {
            title: "Alpha".to_string(),
            criteria: Query::default(),
            columns: Vec::new(),
            item_column_index: 0,
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        });
        view.sections.push(Section {
            title: "Bravo".to_string(),
            criteria: Query::default(),
            columns: Vec::new(),
            item_column_index: 0,
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        });
        app.open_view_edit(view);

        app.handle_view_edit_key(KeyCode::Tab, &agenda)
            .expect("tab");
        app.handle_view_edit_key(KeyCode::Char('j'), &agenda)
            .expect("select first section");
        app.handle_view_edit_key(KeyCode::Char('j'), &agenda)
            .expect("select second section");

        app.handle_view_edit_key(KeyCode::Char('N'), &agenda)
            .expect("insert above");

        let state = app.view_edit_state.as_ref().expect("view edit state");
        assert_eq!(state.section_index, 1);
        assert_eq!(state.draft.sections.len(), 3);
        assert_eq!(state.draft.sections[0].title, "Alpha");
        assert_eq!(state.draft.sections[1].title, "New section");
        assert_eq!(state.draft.sections[2].title, "Bravo");
        assert!(matches!(
            state.inline_input,
            Some(super::ViewEditInlineInput::SectionTitle { section_index: 1 })
        ));

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn view_edit_sections_can_select_view_properties_row_and_enter_opens_criteria() {
        let (store, db_path) = make_test_store_with_view("view-props-row-select");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        let mut view = app
            .views
            .iter()
            .find(|v| v.name == "TestView")
            .cloned()
            .expect("TestView should exist");
        view.sections.push(Section {
            title: "Alpha".to_string(),
            criteria: Query::default(),
            columns: Vec::new(),
            item_column_index: 0,
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        });
        app.open_view_edit(view);

        app.handle_view_edit_key(KeyCode::Tab, &agenda)
            .expect("tab to sections");
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().pane_focus,
            ViewEditPaneFocus::Sections
        );
        let state = app.view_edit_state.as_ref().unwrap();
        assert!(state.sections_view_row_selected);
        assert_eq!(state.section_index, 0, "section cursor is preserved");

        app.handle_view_edit_key(KeyCode::Enter, &agenda)
            .expect("enter should open criteria details");
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().region,
            ViewEditRegion::Criteria
        );

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
        app.handle_view_edit_key(KeyCode::Enter, &agenda)
            .expect("confirm default section title");

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
            app.view_edit_state.as_ref().unwrap().pane_focus,
            ViewEditPaneFocus::Sections
        );
        app.handle_view_edit_key(KeyCode::Char('N'), &agenda)
            .expect("N adds section");
        app.handle_view_edit_key(KeyCode::Enter, &agenda)
            .expect("confirm default section title");
        app.handle_view_edit_key(KeyCode::Char('m'), &agenda)
            .expect("toggle section display override");
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().draft.sections.len(),
            1
        );
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().draft.sections[0].board_display_mode_override,
            Some(BoardDisplayMode::SingleLine)
        );

        // Move from section details back to view criteria details
        app.handle_view_edit_key(KeyCode::BackTab, &agenda)
            .expect("backtab to details pane");
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().pane_focus,
            ViewEditPaneFocus::Details
        );
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().region,
            ViewEditRegion::Sections
        );
        app.handle_view_edit_key(KeyCode::Tab, &agenda)
            .expect("tab to sections pane");
        app.handle_view_edit_key(KeyCode::Char('k'), &agenda)
            .expect("select view properties row");
        app.handle_view_edit_key(KeyCode::Enter, &agenda)
            .expect("open view criteria details");
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().pane_focus,
            ViewEditPaneFocus::Details
        );
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().region,
            ViewEditRegion::Criteria
        );

        // Toggle view default display mode in Criteria details
        app.handle_view_edit_key(KeyCode::Char('m'), &agenda)
            .expect("toggle view display mode");
        assert_eq!(
            app.view_edit_state
                .as_ref()
                .unwrap()
                .draft
                .board_display_mode,
            BoardDisplayMode::MultiLine
        );
        app.handle_view_edit_key(KeyCode::Tab, &agenda)
            .expect("tab to sections");

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
        assert_eq!(saved.board_display_mode, BoardDisplayMode::MultiLine);
        assert_eq!(
            saved.sections[0].board_display_mode_override,
            Some(BoardDisplayMode::SingleLine)
        );

        let _ = std::fs::remove_file(&db_path);
    }

    // ── Per-section filter tests (Phase 3) ─────────────────────────────────

    fn make_two_section_store(suffix: &str) -> (Store, std::path::PathBuf) {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let db_path =
            std::env::temp_dir().join(format!("agenda-tui-section-filter-{suffix}-{nanos}.ag"));
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
            item_column_index: 0,
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        };
        section_work
            .criteria
            .set_criterion(CriterionMode::And, cat_a.id);

        let mut section_personal = Section {
            title: "Personal Items".to_string(),
            criteria: Query::default(),
            columns: Vec::new(),
            item_column_index: 0,
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        };
        section_personal
            .criteria
            .set_criterion(CriterionMode::And, cat_b.id);

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
