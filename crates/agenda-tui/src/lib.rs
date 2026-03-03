use std::collections::{HashMap, HashSet};
use std::io;
use std::path::Path;

use agenda_core::agenda::Agenda;
use agenda_core::matcher::{unknown_hashtag_tokens, SubstringClassifier};
use agenda_core::model::{
    BoardDisplayMode, Category, CategoryId, CategoryValueKind, Column, ColumnKind, CriterionMode,
    Item, ItemId, ItemLinksForItem, NumericFormat, Query, Section, View, WhenBucket,
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
    value_kind: CategoryValueKind,
}

#[derive(Clone)]
struct InspectAssignmentRow {
    category_id: CategoryId,
    category_name: String,
    source_label: String,
    origin_label: String,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum CategoryEditTarget {
    ViewCriteria,
    ViewAliases,
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
    LinkWizard,
    NoteEdit,
    ItemAssignPicker,
    ItemAssignInput,
    InspectUnassign,
    SearchBarFocused,
    ViewPicker,
    ViewEdit,
    ViewDeleteConfirm,
    ConfirmDelete,
    BoardColumnDeleteConfirm,
    CategoryManager,
    CategoryDirectEdit,
    CategoryColumnPicker,
    BoardAddColumnPicker,
    #[allow(dead_code)]
    CategoryCreateConfirm {
        name: String,
        parent_id: CategoryId,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum LinkWizardFocus {
    ScopeAction,
    Target,
    Confirm,
}

impl LinkWizardFocus {
    fn next(self) -> Self {
        match self {
            Self::ScopeAction => Self::Target,
            Self::Target => Self::Confirm,
            Self::Confirm => Self::ScopeAction,
        }
    }

    fn prev(self) -> Self {
        match self {
            Self::ScopeAction => Self::Confirm,
            Self::Target => Self::ScopeAction,
            Self::Confirm => Self::Target,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum LinkWizardAction {
    BlockedBy,
    DependsOn,
    Blocks,
    RelatedTo,
    ClearDependencies,
}

impl LinkWizardAction {
    const ALL: [Self; 5] = [
        Self::BlockedBy,
        Self::DependsOn,
        Self::Blocks,
        Self::RelatedTo,
        Self::ClearDependencies,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::BlockedBy => "blocked by",
            Self::DependsOn => "depends on",
            Self::Blocks => "blocks",
            Self::RelatedTo => "related to",
            Self::ClearDependencies => "clear dependencies",
        }
    }

    fn description(self) -> &'static str {
        match self {
            Self::BlockedBy => "(X blocks selected item)",
            Self::DependsOn => "(selected item depends on X)",
            Self::Blocks => "(selected item blocks X)",
            Self::RelatedTo => "(selected item related to X)",
            Self::ClearDependencies => "(remove depends-on/blocks links for selected item)",
        }
    }

    fn requires_target(self) -> bool {
        !matches!(self, Self::ClearDependencies)
    }

    fn from_index(index: usize) -> Self {
        Self::ALL.get(index).copied().unwrap_or(Self::BlockedBy)
    }

    fn index(self) -> usize {
        match self {
            Self::BlockedBy => 0,
            Self::DependsOn => 1,
            Self::Blocks => 2,
            Self::RelatedTo => 3,
            Self::ClearDependencies => 4,
        }
    }
}

#[derive(Clone, Debug)]
struct LinkWizardState {
    anchor_item_id: ItemId,
    focus: LinkWizardFocus,
    action_index: usize,
    target_filter: text_buffer::TextBuffer,
    target_index: usize,
}

/// Disambiguates which name/value operation is in flight when Mode::InputPanel
/// is open.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum NameInputContext {
    ViewCreate,
    ViewRename,
    /// Editing a numeric cell value in the board.
    NumericValueEdit,
    /// Creating a new category via InputPanel.
    CategoryCreate,
}

/// Pending state for an in-flight numeric cell edit.
#[derive(Clone, Copy, Debug)]
struct NumericEditTarget {
    item_id: ItemId,
    category_id: CategoryId,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ViewEditRegion {
    Criteria,
    Sections,
    Unmatched,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ViewEditPaneFocus {
    Sections,
    Details,
    Preview,
}

#[derive(Clone, PartialEq, Eq, Debug)]
enum ViewEditOverlay {
    CategoryPicker { target: CategoryEditTarget },
    BucketPicker { target: BucketEditTarget },
}

#[derive(Clone, PartialEq, Eq, Debug)]
enum ViewEditInlineInput {
    ViewName,
    SectionsFilter,
    CategoryAlias { category_id: CategoryId },
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
    overlay_filter_buf: text_buffer::TextBuffer,
    preview_count: usize,
    preview_visible: bool,
    preview_scroll: usize,
    sections_filter_buf: text_buffer::TextBuffer,
    dirty: bool,
    discard_confirm: bool,
    section_delete_confirm: Option<usize>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum CategoryManagerFocus {
    Tree,
    Filter,
    Details,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum CategoryManagerDetailsFocus {
    Exclusive,
    MatchName,
    Actionable,
    Note,
}

impl CategoryManagerDetailsFocus {
    /// Cycle forward. When the category is numeric, only Note is relevant.
    fn next(self, is_numeric: bool) -> Self {
        if is_numeric {
            Self::Note
        } else {
            match self {
                Self::Exclusive => Self::MatchName,
                Self::MatchName => Self::Actionable,
                Self::Actionable => Self::Note,
                Self::Note => Self::Exclusive,
            }
        }
    }

    /// Cycle backward. When the category is numeric, only Note is relevant.
    fn prev(self, is_numeric: bool) -> Self {
        if is_numeric {
            Self::Note
        } else {
            match self {
                Self::Exclusive => Self::Note,
                Self::MatchName => Self::Exclusive,
                Self::Actionable => Self::MatchName,
                Self::Note => Self::Actionable,
            }
        }
    }
}

#[derive(Clone)]
enum CategoryInlineAction {
    Rename {
        category_id: CategoryId,
        original_name: String,
        buf: text_buffer::TextBuffer,
    },
    DeleteConfirm {
        category_id: CategoryId,
        category_name: String,
    },
}

#[derive(Clone)]
struct CategoryManagerState {
    focus: CategoryManagerFocus,
    filter: text_buffer::TextBuffer,
    filter_editing: bool,
    structure_move_prefix: Option<char>,
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
    #[allow(dead_code)]
    suggest_index: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum AddColumnDirection {
    #[allow(dead_code)]
    Left,
    Right,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum NormalModePrefix {
    G,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum DoneToggleOrigin {
    NormalMode,
    ItemAssignPicker,
}

#[derive(Clone, Debug)]
struct DoneBlocksConfirmState {
    item_id: ItemId,
    blocked_item_ids: Vec<ItemId>,
    origin: DoneToggleOrigin,
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
    #[allow(dead_code)]
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
    /// Original resolved category IDs for dirty detection.
    original_category_ids: Vec<Option<CategoryId>>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum CategoryColumnPickerFocus {
    FilterInput,
    List,
}

#[derive(Clone)]
struct CategoryColumnPickerState {
    #[allow(dead_code)]
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

    #[allow(dead_code)]
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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum SlotSortDirection {
    Asc,
    Desc,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum SlotSortColumn {
    ItemText,
    SectionColumn {
        heading: CategoryId,
        kind: ColumnKind,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct SlotSortKey {
    column: SlotSortColumn,
    direction: SlotSortDirection,
}

struct App {
    mode: Mode,
    status: String,
    input: text_buffer::TextBuffer,
    section_filters: Vec<Option<String>>,
    slot_sort_keys: Vec<Vec<SlotSortKey>>,
    search_buffer: text_buffer::TextBuffer,
    show_preview: bool,
    preview_mode: PreviewMode,
    normal_focus: NormalFocus,
    all_items: Vec<Item>,
    item_links_by_item_id: HashMap<ItemId, ItemLinksForItem>,

    views: Vec<View>,
    view_index: usize,
    picker_index: usize,
    view_pending_edit_name: Option<String>,
    view_edit_state: Option<ViewEditState>,
    numeric_edit_target: Option<NumericEditTarget>,

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
    link_wizard: Option<LinkWizardState>,
    name_input_context: Option<NameInputContext>,
    preview_provenance_scroll: usize,
    preview_summary_scroll: usize,
    inspect_assignment_index: usize,
    slots: Vec<Slot>,
    slot_index: usize,
    item_index: usize,
    column_index: usize,
    normal_mode_prefix: Option<NormalModePrefix>,
    done_blocks_confirm: Option<DoneBlocksConfirmState>,
    board_pending_delete_column_label: Option<String>,
    note_edit_original: String,
    note_edit_discard_confirm: bool,
    input_panel_discard_confirm: bool,
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
            slot_sort_keys: Vec::new(),
            search_buffer: text_buffer::TextBuffer::empty(),
            show_preview: false,
            preview_mode: PreviewMode::Summary,
            normal_focus: NormalFocus::Board,
            all_items: Vec::new(),
            item_links_by_item_id: HashMap::new(),
            views: Vec::new(),
            view_index: 0,
            picker_index: 0,
            view_pending_edit_name: None,
            view_edit_state: None,
            numeric_edit_target: None,
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
            link_wizard: None,
            name_input_context: None,
            preview_provenance_scroll: 0,
            preview_summary_scroll: 0,
            inspect_assignment_index: 0,
            slots: Vec::new(),
            slot_index: 0,
            item_index: 0,
            column_index: 0,
            normal_mode_prefix: None,
            done_blocks_confirm: None,
            board_pending_delete_column_label: None,
            note_edit_original: String::new(),
            note_edit_discard_confirm: false,
            input_panel_discard_confirm: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        add_capture_status_message, board_column_widths, board_item_label,
        board_table_spacing_budget, bucket_target_set_mut, build_category_rows, category_name_map,
        compute_board_layout, first_non_reserved_category_index, input_panel,
        input_panel_popup_area, item_assignment_labels, list_scroll_for_selected_line, next_index,
        next_index_clamped, should_render_unmatched_lane, text_buffer, truncate_board_cell,
        when_bucket_options, AddColumnDirection, App, BucketEditTarget, CategoryDirectEditAnchor,
        CategoryDirectEditFocus, CategoryDirectEditRow, CategoryDirectEditState,
        CategoryInlineAction, CategoryListRow, CategoryManagerDetailsFocus, CategoryManagerFocus,
        Mode, NameInputContext, SlotSortDirection, ViewEditPaneFocus, ViewEditRegion,
    };
    use agenda_core::agenda::Agenda;
    use agenda_core::matcher::SubstringClassifier;
    use agenda_core::model::{
        Assignment, AssignmentSource, BoardDisplayMode, Category, CategoryId, CategoryValueKind,
        Column, ColumnKind, CriterionMode, Item, ItemId, Query, Section, View, WhenBucket,
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
            numeric_value: None,
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
            original_category_ids: Vec::new(),
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
            original_category_ids: vec![None, None],
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

        app.handle_category_direct_edit_key(KeyCode::Tab, &agenda)
            .expect("tab away from Input");
        app.handle_category_direct_edit_key(KeyCode::Char('S'), &agenda)
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
            original_category_ids: vec![None, None],
        };
        let mut app = App {
            category_direct_edit: Some(state.clone()),
            ..App::default()
        };
        let store = Store::open(std::env::temp_dir().join(format!(
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

        // Tab away from Input so '+' acts as add-row command instead of typing
        app.handle_category_direct_edit_key(KeyCode::Tab, &agenda)
            .expect("tab away from Input");
        app.handle_category_direct_edit_key(KeyCode::Char('+'), &agenda)
            .expect("plus adds row");

        let state = app.category_direct_edit_state().expect("direct edit state");
        assert_eq!(state.rows.len(), 2);
        assert_eq!(state.active_row, 1);
        // add_blank_row_guarded resets focus to Input
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

        // Tab away from Input so '+' acts as command instead of typing
        app.handle_category_direct_edit_key(KeyCode::Tab, &agenda)
            .expect("tab away from Input");
        app.handle_category_direct_edit_key(KeyCode::Char('+'), &agenda)
            .expect("plus handled");

        let state = app.category_direct_edit_state().expect("direct edit state");
        assert_eq!(state.rows.len(), 1);
        // Focus stays at Suggestions (Tab destination) since exclusive guard blocked add
        assert_eq!(state.focus, CategoryDirectEditFocus::Suggestions);
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

        app.handle_category_direct_edit_key(KeyCode::Char('y'), &agenda)
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

        // Tab away from Input so 'S' acts as save command instead of typing
        app.handle_category_direct_edit_key(KeyCode::Tab, &agenda)
            .expect("tab away from Input");
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

        app.handle_key(KeyCode::Char('y'), &agenda)
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
    fn board_add_column_picker_allows_nested_non_leaf_heading() {
        let store = Store::open_memory().expect("memory store");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut status = Category::new("Status".to_string());
        let mut pending = Category::new("Pending".to_string());
        pending.parent = Some(status.id);
        status.children = vec![pending.id];

        let mut project = Category::new("Project".to_string());
        let mut phase = Category::new("Phase".to_string());
        let mut phase_task = Category::new("Phase Task".to_string());
        phase.parent = Some(project.id);
        phase_task.parent = Some(phase.id);
        project.children = vec![phase.id];
        phase.children = vec![phase_task.id];

        for cat in [&status, &pending, &project, &phase, &phase_task] {
            store.create_category(cat).expect("create category");
        }

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

        app.handle_key(KeyCode::Char('+'), &agenda)
            .expect("open add-column picker");
        let suggestions = app.get_board_add_column_suggest_matches();
        assert!(
            suggestions.contains(&phase.id),
            "nested non-leaf heading should be suggested"
        );

        for ch in "Phase".chars() {
            app.handle_key(KeyCode::Char(ch), &agenda)
                .expect("type in picker");
        }
        app.handle_key(KeyCode::Enter, &agenda)
            .expect("insert nested non-leaf heading");

        assert_eq!(app.mode, Mode::Normal);
        let saved = store
            .get_view(app.current_view().expect("current view").id)
            .expect("saved view");
        let headings: Vec<CategoryId> = saved.sections[0]
            .columns
            .iter()
            .map(|c| c.heading)
            .collect();
        assert_eq!(headings, vec![status.id, phase.id]);
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
        app.handle_key(KeyCode::Char('y'), &agenda)
            .expect("y confirms delete");
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
            value_kind: CategoryValueKind::Tag,
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
            value_kind: CategoryValueKind::Tag,
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
            value_kind: CategoryValueKind::Tag,
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
            value_kind: CategoryValueKind::Tag,
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
            (Mode::ItemAssignInput, "Category> "),
        ];

        for (mode, prefix) in cases {
            let app = App {
                mode: mode.clone(),
                input: text_buffer::TextBuffer::new(input.to_string()),
                // Set note_edit_original to match input so NoteEdit isn't dirty
                note_edit_original: input.to_string(),
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
            Mode::InputPanel,       // popup mode — footer cursor hidden
            Mode::SearchBarFocused, // cursor rendered by search bar, not footer
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
            note_edit_original: "abcd".to_string(),
            ..App::default()
        };
        assert_eq!(app.input_cursor_position(footer), Some((9, 1)));
    }

    #[test]
    fn input_panel_cursor_position_uses_popup_area() {
        let screen = Rect::new(0, 0, 120, 40);
        let popup = input_panel_popup_area(screen, input_panel::InputPanelKind::EditItem);
        let panel = input_panel::InputPanel::new_edit_item(
            agenda_core::model::ItemId::new_v4(),
            "abcd".to_string(),
            String::new(),
            Default::default(),
            std::collections::HashMap::new(),
            std::collections::HashMap::new(),
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
    fn category_manager_action_cursor_position_tracks_filter_input() {
        let action_area = Rect::new(10, 4, 40, 3);
        let mut app = App {
            mode: Mode::CategoryManager,
            ..App::default()
        };
        app.ensure_category_manager_session();
        app.set_category_manager_focus(CategoryManagerFocus::Filter);
        app.set_category_manager_filter_editing(true);
        if let Some(filter) = app.category_manager_filter_mut() {
            filter.set("abc".to_string());
            let _ = filter.handle_key(KeyCode::Left, false);
        }

        let prefix_len = "Filter: ".chars().count() as u16;
        let expected = (action_area.x + 1 + prefix_len + 2, action_area.y + 1);
        assert_eq!(
            app.category_manager_action_cursor_position(action_area),
            Some(expected)
        );
    }

    #[test]
    fn category_manager_action_cursor_position_tracks_inline_rename_input() {
        let action_area = Rect::new(10, 4, 50, 3);
        let mut app = App {
            mode: Mode::CategoryManager,
            ..App::default()
        };
        app.ensure_category_manager_session();
        app.set_category_manager_inline_action(Some(CategoryInlineAction::Rename {
            category_id: CategoryId::new_v4(),
            original_name: "Old".to_string(),
            buf: text_buffer::TextBuffer::with_cursor("Office".to_string(), 3),
        }));

        let prefix_len = "Rename> ".chars().count() as u16;
        let expected = (action_area.x + 1 + prefix_len + 3, action_area.y + 1);
        assert_eq!(
            app.category_manager_action_cursor_position(action_area),
            Some(expected)
        );
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
        // Tab → Categories, Tab → SaveButton
        app.handle_input_panel_key(KeyCode::Tab, &agenda)
            .expect("focus categories");
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
        app.picker_index = app
            .views
            .iter()
            .position(|view| view.name == "Work Board")
            .expect("work board view should exist");

        app.handle_view_picker_key(KeyCode::Char('V'), &agenda)
            .expect("open view edit");

        assert_eq!(app.mode, Mode::ViewEdit);
        assert!(app.view_edit_state.is_some());

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn view_picker_blocks_edit_delete_and_rename_for_all_items() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!("agenda-tui-view-immutable-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);
        store
            .create_view(&View::new("Work Board".to_string()))
            .expect("create view");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.mode = Mode::ViewPicker;
        app.picker_index = app
            .views
            .iter()
            .position(|view| view.name == "All Items")
            .expect("All Items view should exist");

        app.handle_view_picker_key(KeyCode::Char('e'), &agenda)
            .expect("edit key");
        assert_eq!(app.mode, Mode::ViewPicker);
        assert!(app.status.contains("immutable"));

        app.handle_view_picker_key(KeyCode::Char('x'), &agenda)
            .expect("delete key");
        assert_eq!(app.mode, Mode::ViewPicker);
        assert!(app.status.contains("immutable"));

        app.handle_view_picker_key(KeyCode::Char('r'), &agenda)
            .expect("rename key");
        assert_eq!(app.mode, Mode::ViewPicker);
        assert!(app.status.contains("immutable"));

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
        // Tab from Text to SaveButton, then Enter to save (S types into text when focus is Text)
        app.handle_input_panel_key(KeyCode::Tab, &agenda)
            .expect("tab to save button");
        app.handle_input_panel_key(KeyCode::Enter, &agenda)
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
        app.picker_index = app
            .views
            .iter()
            .position(|view| view.name == "Work Board")
            .expect("work board view should exist");
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
    fn normal_mode_p_and_i_manage_preview_modes() {
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

        app.handle_normal_key(KeyCode::Char('i'), &agenda)
            .expect("switch to provenance");
        assert_eq!(app.preview_mode, super::PreviewMode::Provenance);

        app.handle_normal_key(KeyCode::Char('i'), &agenda)
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
    fn item_details_summary_prioritizes_note_and_categories() {
        let alpha = Category::new("Alpha".to_string());
        let beta = Category::new("Beta".to_string());
        let mut item = Item::new("demo".to_string());
        item.note = Some("Primary note".to_string());
        let assignment = Assignment {
            source: AssignmentSource::Manual,
            assigned_at: chrono::Utc::now(),
            sticky: false,
            origin: None,
            numeric_value: None,
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
        let note_index = plain
            .iter()
            .position(|line| line == "Note")
            .expect("summary contains Note header");
        let categories_index = plain
            .iter()
            .position(|line| line == "Categories")
            .expect("summary contains Categories header");
        assert!(
            note_index < categories_index,
            "note appears before categories"
        );
        assert!(plain.iter().any(|line| line == "  Primary note"));
        assert!(plain
            .iter()
            .any(|line| line == "  Alpha, Beta" || line == "  Beta, Alpha"));
    }

    #[test]
    fn item_info_contains_link_sections_while_summary_stays_primary() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!("agenda-tui-link-preview-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let a = Item::new("Task A".to_string());
        let b = Item::new("Task B".to_string());
        let c = Item::new("Task C".to_string());
        let d = Item::new("Task D".to_string());
        store.create_item(&a).expect("create A");
        store.create_item(&b).expect("create B");
        store.create_item(&c).expect("create C");
        store.create_item(&d).expect("create D");

        agenda
            .link_items_depends_on(a.id, b.id)
            .expect("A depends-on B");
        agenda.link_items_blocks(c.id, a.id).expect("C blocks A");
        agenda.link_items_related(a.id, d.id).expect("A related D");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        let loaded_a = store.get_item(a.id).expect("reload A");
        let lines = app.item_details_lines_for_item(&loaded_a);
        let plain: Vec<String> = lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect()
            })
            .collect();

        assert!(!plain.iter().any(|line| line == "Prereqs"));
        assert!(!plain.iter().any(|line| line == "Blocks"));
        assert!(!plain.iter().any(|line| line == "Related"));

        let info_lines = app.item_info_header_lines_for_item(&loaded_a);
        assert!(info_lines.iter().any(|line| line == "Prereqs"));
        assert!(info_lines.iter().any(|line| line == "Blocks"));
        assert!(info_lines.iter().any(|line| line == "Related"));
        assert!(info_lines.iter().any(|line| line.contains("Task B")));
        assert!(info_lines.iter().any(|line| line.contains("Task C")));
        assert!(info_lines.iter().any(|line| line.contains("Task D")));

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn input_panel_note_up_down_moves_cursor_between_lines() {
        let mut panel = input_panel::InputPanel::new_edit_item(
            agenda_core::model::ItemId::new_v4(),
            "hello".to_string(),
            String::new(),
            Default::default(),
            std::collections::HashMap::new(),
            std::collections::HashMap::new(),
        );
        // Set note buffer with multiline content and cursor mid-second-line.
        panel.note = text_buffer::TextBuffer::with_cursor(
            "first\nsecond".to_string(),
            "first\nse".chars().count(),
        );
        panel.focus = input_panel::InputPanelFocus::Note;

        panel.handle_key(KeyCode::Up, false);
        assert_eq!(panel.note.cursor(), "fi".chars().count());

        panel.handle_key(KeyCode::Down, false);
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
    fn category_create_panel_opens_and_creates_root_category() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path =
            std::env::temp_dir().join(format!("agenda-tui-category-create-panel-root-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.handle_normal_key(KeyCode::Char('c'), &agenda)
            .expect("open category manager");
        app.handle_category_manager_key(KeyCode::Char('n'), &agenda)
            .expect("open create panel");
        assert_eq!(app.mode, Mode::InputPanel);
        assert!(app.input_panel.is_some(), "panel should be open");
        assert_eq!(
            app.input_panel.as_ref().unwrap().kind,
            input_panel::InputPanelKind::CategoryCreate
        );
        assert_eq!(
            app.name_input_context,
            Some(NameInputContext::CategoryCreate)
        );

        // Type category name in the panel
        for c in "Projects".chars() {
            app.handle_input_panel_key(KeyCode::Char(c), &agenda)
                .expect("type create name");
        }
        // Save (S from text focus won't work, Tab to Save button + Enter)
        // Use capital S from a non-text focus
        app.handle_input_panel_key(KeyCode::Tab, &agenda)
            .expect("tab to parent");
        app.handle_input_panel_key(KeyCode::Char('S'), &agenda)
            .expect("save category");

        assert!(
            app.categories
                .iter()
                .any(|category| category.name == "Projects"),
            "Projects category should exist"
        );
        assert_eq!(app.mode, Mode::CategoryManager);
        assert!(app.input_panel.is_none());

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn category_create_panel_child_creates_under_selected_parent() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path =
            std::env::temp_dir().join(format!("agenda-tui-category-create-panel-child-{nanos}.ag"));
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
            .expect("open create panel");
        assert_eq!(app.mode, Mode::InputPanel);
        // Panel should have parent pre-filled
        assert_eq!(app.input_panel.as_ref().unwrap().parent_id, Some(parent.id));

        for c in "Child".chars() {
            app.handle_input_panel_key(KeyCode::Char(c), &agenda)
                .expect("type child name");
        }
        // Tab to Parent, then Tab to TypePicker, then S to save
        app.handle_input_panel_key(KeyCode::Tab, &agenda)
            .expect("tab to parent");
        app.handle_input_panel_key(KeyCode::Char('S'), &agenda)
            .expect("save category");

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
    fn category_create_panel_tab_cycles_without_parent_picker_and_keeps_default_parent() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!(
            "agenda-tui-category-create-panel-no-parent-picker-{nanos}.ag"
        ));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let alpha = Category::new("Alpha".to_string());
        store.create_category(&alpha).expect("create alpha");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.handle_normal_key(KeyCode::Char('c'), &agenda)
            .expect("open category manager");
        app.set_category_selection_by_id(alpha.id);

        app.handle_category_manager_key(KeyCode::Char('n'), &agenda)
            .expect("open create panel");
        for c in "Child".chars() {
            app.handle_input_panel_key(KeyCode::Char(c), &agenda)
                .expect("type child name");
        }
        app.handle_input_panel_key(KeyCode::Tab, &agenda)
            .expect("tab to type picker");
        assert_eq!(
            app.input_panel.as_ref().unwrap().focus,
            input_panel::InputPanelFocus::TypePicker
        );
        assert_eq!(app.input_panel.as_ref().unwrap().parent_id, Some(alpha.id));

        app.handle_input_panel_key(KeyCode::Char('S'), &agenda)
            .expect("save category create");
        assert_eq!(app.mode, Mode::CategoryManager);
        assert!(app.input_panel.is_none());

        let child = app
            .categories
            .iter()
            .find(|category| category.name == "Child")
            .expect("child should be created");
        assert_eq!(child.parent, Some(alpha.id));

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn category_create_panel_render_uses_category_manager_backdrop() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!(
            "agenda-tui-category-create-panel-backdrop-{nanos}.ag"
        ));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.handle_normal_key(KeyCode::Char('c'), &agenda)
            .expect("open category manager");
        app.handle_category_manager_key(KeyCode::Char('n'), &agenda)
            .expect("open create panel");

        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| app.draw(frame))
            .expect("render category create panel");
        let text = terminal_buffer_lines(&terminal).join("\n");
        assert!(
            text.contains("Create Category"),
            "create panel title should be rendered"
        );
        assert!(
            text.contains("Category Manager"),
            "category manager should remain visible behind the create panel"
        );

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn category_create_panel_save_updates_visible_rows_without_reopening_manager() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!(
            "agenda-tui-category-create-panel-visible-rows-{nanos}.ag"
        ));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.handle_normal_key(KeyCode::Char('c'), &agenda)
            .expect("open category manager");
        let before_visible_len = app
            .category_manager_visible_row_indices()
            .map(|rows| rows.len())
            .unwrap_or(0);

        app.handle_category_manager_key(KeyCode::Char('n'), &agenda)
            .expect("open create panel");
        for c in "ShowsImmediately".chars() {
            app.handle_input_panel_key(KeyCode::Char(c), &agenda)
                .expect("type category name");
        }
        app.handle_input_panel_key(KeyCode::Tab, &agenda)
            .expect("tab off text focus");
        app.handle_input_panel_key(KeyCode::Char('S'), &agenda)
            .expect("save category");

        assert_eq!(app.mode, Mode::CategoryManager);
        let new_row_index = app
            .category_rows
            .iter()
            .position(|row| row.name == "ShowsImmediately")
            .expect("new category row should exist");
        let visible_rows = app
            .category_manager_visible_row_indices()
            .expect("category manager visible rows");
        assert!(
            visible_rows.contains(&new_row_index),
            "new category should be visible immediately in current manager session"
        );
        assert_eq!(
            visible_rows.len(),
            app.category_rows.len(),
            "without an active filter, visible rows should include all categories"
        );
        assert!(
            visible_rows.len() > before_visible_len,
            "saving a new category should grow visible rows"
        );

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn category_create_panel_numeric_via_type_toggle() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!(
            "agenda-tui-category-create-panel-numeric-{nanos}.ag"
        ));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.handle_normal_key(KeyCode::Char('c'), &agenda)
            .expect("open category manager");
        app.handle_category_manager_key(KeyCode::Char('n'), &agenda)
            .expect("open create panel");

        for c in "Cost".chars() {
            app.handle_input_panel_key(KeyCode::Char(c), &agenda)
                .expect("type create name");
        }

        // Verify default is Tag
        assert_eq!(
            app.input_panel.as_ref().unwrap().value_kind,
            CategoryValueKind::Tag
        );

        // Tab to TypePicker, toggle to Numeric
        app.handle_input_panel_key(KeyCode::Tab, &agenda)
            .expect("tab to type picker");
        assert_eq!(
            app.input_panel.as_ref().unwrap().focus,
            input_panel::InputPanelFocus::TypePicker
        );
        app.handle_input_panel_key(KeyCode::Char(' '), &agenda)
            .expect("toggle to numeric");
        assert_eq!(
            app.input_panel.as_ref().unwrap().value_kind,
            CategoryValueKind::Numeric
        );

        // Save
        app.handle_input_panel_key(KeyCode::Char('S'), &agenda)
            .expect("save category");

        let cost = app
            .categories
            .iter()
            .find(|c| c.name == "Cost")
            .expect("Cost category created");
        assert_eq!(cost.value_kind, CategoryValueKind::Numeric);
        assert!(app.status.contains("numeric"));

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn category_create_panel_esc_cancels_without_creating() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path =
            std::env::temp_dir().join(format!("agenda-tui-category-create-panel-esc-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.handle_normal_key(KeyCode::Char('c'), &agenda)
            .expect("open category manager");
        app.handle_category_manager_key(KeyCode::Char('n'), &agenda)
            .expect("open create panel");

        for c in "Score".chars() {
            app.handle_input_panel_key(KeyCode::Char(c), &agenda)
                .expect("type create name");
        }

        // Press Esc to cancel (panel is dirty, so we need Esc twice)
        app.handle_input_panel_key(KeyCode::Esc, &agenda)
            .expect("start cancel");
        // Dirty confirmation — Esc again to discard
        app.handle_input_panel_key(KeyCode::Esc, &agenda)
            .expect("confirm discard");

        assert_eq!(app.mode, Mode::CategoryManager);
        assert!(app.input_panel.is_none());
        // Category should not have been created
        assert!(!app.categories.iter().any(|c| c.name == "Score"));

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn category_create_panel_rejects_duplicate_name() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path =
            std::env::temp_dir().join(format!("agenda-tui-category-create-panel-dup-{nanos}.ag"));
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
        app.handle_category_manager_key(KeyCode::Char('n'), &agenda)
            .expect("open create panel");
        for c in "Work".chars() {
            app.handle_input_panel_key(KeyCode::Char(c), &agenda)
                .expect("type duplicate name");
        }
        // Tab to save button and press enter to save
        app.handle_input_panel_key(KeyCode::Tab, &agenda)
            .expect("tab");
        app.handle_input_panel_key(KeyCode::Char('S'), &agenda)
            .expect("attempt save");

        assert!(app.status.contains("already exists"));
        // Panel should still be open
        assert_eq!(app.mode, Mode::InputPanel);
        assert!(app.input_panel.is_some());
        let count = app.categories.iter().filter(|c| c.name == "Work").count();
        assert_eq!(count, 1);

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn category_create_panel_rejects_reserved_name() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!(
            "agenda-tui-category-create-panel-reserved-{nanos}.ag"
        ));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.handle_normal_key(KeyCode::Char('c'), &agenda)
            .expect("open category manager");
        app.handle_category_manager_key(KeyCode::Char('n'), &agenda)
            .expect("open create panel");
        for c in "Done".chars() {
            app.handle_input_panel_key(KeyCode::Char(c), &agenda)
                .expect("type reserved name");
        }
        // Tab and S to attempt save
        app.handle_input_panel_key(KeyCode::Tab, &agenda)
            .expect("tab");
        app.handle_input_panel_key(KeyCode::Char('S'), &agenda)
            .expect("attempt save");

        assert!(app.status.contains("reserved category"));
        // Panel should still be open
        assert_eq!(app.mode, Mode::InputPanel);
        assert!(app.input_panel.is_some());

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
        app.handle_category_manager_key(KeyCode::Esc, &agenda)
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
    fn category_manager_tab_focuses_details_and_p_no_longer_opens_reparent_ui() {
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
            .expect("p should be ignored");

        let state = app.category_manager.as_ref().expect("manager state");
        assert_eq!(state.filter.text(), "");
        assert!(state.inline_action.is_none());
        assert_eq!(app.selected_category_id(), Some(child.id));

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
    fn category_manager_double_angle_right_indents_selected_category_under_previous_sibling() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!(
            "agenda-tui-category-indent-double-angle-{nanos}.ag"
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
        app.set_category_selection_by_id(beta.id);

        app.handle_category_manager_key(KeyCode::Char('>'), &agenda)
            .expect("arm double-angle indent");
        assert_eq!(
            store.get_category(beta.id).expect("beta before").parent,
            None
        );
        assert!(app.status.contains("Press > again"));

        app.handle_category_manager_key(KeyCode::Char('>'), &agenda)
            .expect("indent with >>");

        let loaded_alpha = store.get_category(alpha.id).expect("load alpha");
        let loaded_beta = store.get_category(beta.id).expect("load beta");
        assert_eq!(loaded_beta.parent, Some(alpha.id));
        assert_eq!(loaded_alpha.children, vec![beta.id]);
        assert!(app.status.contains("Indented Beta under Alpha"));
        assert_eq!(app.selected_category_id(), Some(beta.id));

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn category_manager_double_angle_left_outdents_selected_category() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!(
            "agenda-tui-category-outdent-double-angle-{nanos}.ag"
        ));
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
        app.handle_normal_key(KeyCode::Char('c'), &agenda)
            .expect("open category manager");
        app.set_category_selection_by_id(child.id);

        app.handle_category_manager_key(KeyCode::Char('<'), &agenda)
            .expect("arm double-angle outdent");
        assert_eq!(
            store.get_category(child.id).expect("child before").parent,
            Some(parent.id)
        );
        assert!(app.status.contains("Press < again"));

        app.handle_category_manager_key(KeyCode::Char('<'), &agenda)
            .expect("outdent with <<");

        let loaded_parent = store.get_category(parent.id).expect("load parent");
        let loaded_child = store.get_category(child.id).expect("load child");
        assert_eq!(loaded_child.parent, None);
        assert!(loaded_parent.children.is_empty());
        assert!(app.status.contains("Outdented Child"));
        assert_eq!(app.selected_category_id(), Some(child.id));

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
    fn category_manager_details_note_explicit_save_with_capital_s() {
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

        // Tab should NOT save — note stays dirty
        app.handle_category_manager_key(KeyCode::Tab, &agenda)
            .expect("tab away from note");
        let saved = store.get_category(category.id).expect("load category");
        assert_eq!(saved.note, None, "tab should not autosave");
        assert!(app.status.contains("unsaved changes"));

        // Go back and save explicitly
        app.set_category_selection_by_id(category.id);
        app.set_category_manager_focus(CategoryManagerFocus::Details);
        app.set_category_manager_details_focus(CategoryManagerDetailsFocus::Note);
        app.set_category_manager_details_note_editing(true);
        app.handle_category_manager_key(KeyCode::Char('S'), &agenda)
            .expect("explicit save with S");
        let saved = store.get_category(category.id).expect("load category");
        assert_eq!(saved.note.as_deref(), Some("Ship"));

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

        // Save explicitly with S
        app.handle_category_manager_key(KeyCode::Char('S'), &agenda)
            .expect("save note with S");
        let saved = store.get_category(category.id).expect("load category");
        assert_eq!(saved.note.as_deref(), Some("j"));

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn category_manager_details_note_edit_esc_discards_changes() {
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
            .expect("esc discards note edit");

        // Esc should discard, not save
        let loaded = store.get_category(category.id).expect("load category");
        assert_eq!(loaded.note.as_deref(), Some("seed"));
        assert_eq!(app.category_manager_details_note_text(), Some("seed"));
        assert!(!app.category_manager_details_note_editing());
        assert!(!app.category_manager_details_note_dirty());
        assert!(app.status.contains("discarded"));

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn category_manager_details_dirty_note_not_saved_on_selection_change() {
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
        // Selection change should NOT autosave the note
        assert_eq!(
            store.get_category(alpha.id).expect("alpha").note,
            None,
            "note should not be saved on selection change"
        );

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
        app.handle_category_manager_key(KeyCode::Char('>'), &agenda)
            .expect("first > ignored in details");
        app.handle_category_manager_key(KeyCode::Char('>'), &agenda)
            .expect("second > ignored in details");
        app.handle_category_manager_key(KeyCode::Char('<'), &agenda)
            .expect("first < ignored in details");
        app.handle_category_manager_key(KeyCode::Char('<'), &agenda)
            .expect("second < ignored in details");

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
    fn normal_mode_d_prompts_then_y_clears_blocker_links() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!("agenda-tui-d-clear-links-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let work = Category::new("Work".to_string());
        store.create_category(&work).expect("create category");
        let blocker = Item::new("Blocker".to_string());
        let blocked = Item::new("Blocked".to_string());
        store.create_item(&blocker).expect("create blocker");
        store.create_item(&blocked).expect("create blocked");
        agenda
            .assign_item_manual(blocker.id, work.id, Some("manual:test".to_string()))
            .expect("assign actionable category");
        agenda
            .link_items_blocks(blocker.id, blocked.id)
            .expect("create blocker link");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.mode = Mode::Normal;
        app.set_item_selection_by_id(blocker.id);

        app.handle_normal_key(KeyCode::Char('d'), &agenda)
            .expect("d should open done confirm");
        assert_eq!(app.mode, Mode::ConfirmDelete);
        assert!(
            !store.get_item(blocker.id).expect("load blocker").is_done,
            "item should not be marked done until confirm"
        );

        app.handle_confirm_delete_key(KeyCode::Char('y'), &agenda)
            .expect("y should confirm done + link cleanup");

        assert!(store.get_item(blocker.id).expect("load blocker").is_done);
        assert_eq!(
            agenda
                .immediate_dependent_ids(blocker.id)
                .expect("load dependents"),
            Vec::<ItemId>::new()
        );
        assert_eq!(app.mode, Mode::Normal);
        assert!(
            app.status.contains("removed 1 blocking link"),
            "status should mention blocker-link cleanup: {}",
            app.status
        );

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn normal_mode_d_prompt_n_keeps_blocker_links() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!("agenda-tui-d-keep-links-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let work = Category::new("Work".to_string());
        store.create_category(&work).expect("create category");
        let blocker = Item::new("Blocker".to_string());
        let blocked = Item::new("Blocked".to_string());
        store.create_item(&blocker).expect("create blocker");
        store.create_item(&blocked).expect("create blocked");
        agenda
            .assign_item_manual(blocker.id, work.id, Some("manual:test".to_string()))
            .expect("assign actionable category");
        agenda
            .link_items_blocks(blocker.id, blocked.id)
            .expect("create blocker link");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.mode = Mode::Normal;
        app.set_item_selection_by_id(blocker.id);

        app.handle_normal_key(KeyCode::Char('d'), &agenda)
            .expect("d should open done confirm");
        assert_eq!(app.mode, Mode::ConfirmDelete);

        app.handle_confirm_delete_key(KeyCode::Char('n'), &agenda)
            .expect("n should mark done and keep links");

        assert!(store.get_item(blocker.id).expect("load blocker").is_done);
        assert_eq!(
            agenda
                .immediate_dependent_ids(blocker.id)
                .expect("load dependents")
                .len(),
            1
        );
        assert_eq!(app.mode, Mode::Normal);

        drop(store);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn item_assign_done_prompt_esc_returns_to_picker_without_changes() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!("agenda-tui-d-picker-esc-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let work = Category::new("Work".to_string());
        store.create_category(&work).expect("create category");
        let blocker = Item::new("Blocker".to_string());
        let blocked = Item::new("Blocked".to_string());
        store.create_item(&blocker).expect("create blocker");
        store.create_item(&blocked).expect("create blocked");
        agenda
            .assign_item_manual(blocker.id, work.id, Some("manual:test".to_string()))
            .expect("assign actionable category");
        agenda
            .link_items_blocks(blocker.id, blocked.id)
            .expect("create blocker link");

        let mut app = App::default();
        app.refresh(&store).expect("refresh app");
        app.set_item_selection_by_id(blocker.id);
        app.mode = Mode::ItemAssignPicker;
        app.item_assign_category_index = app
            .category_rows
            .iter()
            .position(|row| row.name.eq_ignore_ascii_case("Done"))
            .expect("Done category row should exist");

        app.handle_item_assign_category_key(KeyCode::Char(' '), &agenda)
            .expect("space should open done confirm");
        assert_eq!(app.mode, Mode::ConfirmDelete);

        app.handle_confirm_delete_key(KeyCode::Esc, &agenda)
            .expect("Esc should cancel done prompt");
        assert_eq!(app.mode, Mode::ItemAssignPicker);
        assert!(!store.get_item(blocker.id).expect("load blocker").is_done);
        assert_eq!(
            agenda
                .immediate_dependent_ids(blocker.id)
                .expect("load dependents")
                .len(),
            1
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
                numeric_value: None,
            },
        );
        item.assignments.insert(
            category_b,
            agenda_core::model::Assignment {
                source: agenda_core::model::AssignmentSource::Manual,
                assigned_at: chrono::Utc::now(),
                sticky: true,
                origin: None,
                numeric_value: None,
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
        assert!(
            dynamic_used + board_table_spacing_budget(2 + dynamic.columns.len() + 1)
                <= slot_width as usize
        );
        assert!(dynamic.item >= 1);
        assert!(dynamic.columns.iter().all(|column| column.width >= 8));

        let legacy = board_column_widths(slot_width);
        assert!(
            legacy.marker
                + legacy.note
                + legacy.when
                + legacy.item
                + legacy.categories
                + board_table_spacing_budget(5)
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

    #[test]
    fn board_legacy_rows_keep_item_and_categories_visually_separated_when_truncated() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!(
            "agenda-tui-board-legacy-spacing-{nanos}-{}.ag",
            std::process::id()
        ));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let tag = Category::new("GapLeg".to_string());
        store.create_category(&tag).expect("create category");

        let item = Item::new("LEGACY-SEPARATOR-LONG-TEXT".to_string());
        store.create_item(&item).expect("create item");
        agenda
            .assign_item_manual(item.id, tag.id, Some("test:assign".to_string()))
            .expect("assign category");

        let mut view = View::new("Board".to_string());
        view.sections.push(Section {
            title: "Main".to_string(),
            criteria: Query::default(),
            columns: vec![],
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

        let backend = TestBackend::new(44, 14);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| app.draw(frame))
            .expect("render board");

        let lines = terminal_buffer_lines(&terminal);
        let row_line = lines
            .iter()
            .find(|line| line.contains("GapLeg"))
            .expect("row line should include category token");
        assert!(
            row_line.contains("..."),
            "expected truncated row content in narrow terminal"
        );
        let token_index = row_line
            .find("GapLeg")
            .expect("row includes category token");
        let separator = row_line[..token_index]
            .chars()
            .last()
            .expect("token should not be first character");
        assert!(
            separator.is_whitespace(),
            "expected at least one visible separator before category token: {row_line:?}"
        );

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn board_dynamic_rows_keep_adjacent_columns_separated_when_truncated() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!(
            "agenda-tui-board-dynamic-spacing-{nanos}-{}.ag",
            std::process::id()
        ));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut status = Category::new("Status".to_string());
        status.is_exclusive = true;
        store.create_category(&status).expect("create status");
        let mut gap_value = Category::new("GapDyn".to_string());
        gap_value.parent = Some(status.id);
        store
            .create_category(&gap_value)
            .expect("create status child");

        let item = Item::new("DYNAMIC-SEPARATOR-LONG-TEXT".to_string());
        store.create_item(&item).expect("create item");
        agenda
            .assign_item_manual(item.id, gap_value.id, Some("test:assign".to_string()))
            .expect("assign category");

        let mut view = View::new("Board".to_string());
        view.sections.push(Section {
            title: "Main".to_string(),
            criteria: Query::default(),
            columns: vec![Column {
                kind: ColumnKind::Standard,
                heading: status.id,
                width: 8,
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

        let backend = TestBackend::new(34, 14);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| app.draw(frame))
            .expect("render board");

        let lines = terminal_buffer_lines(&terminal);
        let row_line = lines
            .iter()
            .find(|line| line.contains("GapDyn"))
            .expect("row line should include dynamic category token");
        assert!(
            row_line.contains("..."),
            "expected truncated item text in narrow dynamic layout"
        );
        let token_index = row_line
            .find("GapDyn")
            .expect("row includes dynamic category token");
        let separator = row_line[..token_index]
            .chars()
            .last()
            .expect("token should not be first character");
        assert!(
            separator.is_whitespace(),
            "expected at least one visible separator before dynamic token: {row_line:?}"
        );

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn board_column_header_uses_view_alias_for_numeric_heading() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after epoch")
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!(
            "agenda-tui-board-alias-header-{nanos}-{}.ag",
            std::process::id()
        ));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut complexity = Category::new("Complexity".to_string());
        complexity.value_kind = CategoryValueKind::Numeric;
        store
            .create_category(&complexity)
            .expect("create numeric category");

        let item = Item::new("Alias header item".to_string());
        store.create_item(&item).expect("create item");
        agenda
            .assign_item_numeric_manual(
                item.id,
                complexity.id,
                rust_decimal::Decimal::new(5, 0),
                Some("test:assign".to_string()),
            )
            .expect("assign numeric value");

        let mut view = View::new("Board".to_string());
        view.category_aliases
            .insert(complexity.id, "Points".to_string());
        view.sections.push(Section {
            title: "Main".to_string(),
            criteria: Query::default(),
            columns: vec![Column {
                kind: ColumnKind::Standard,
                heading: complexity.id,
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

        let backend = TestBackend::new(70, 16);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| app.draw(frame))
            .expect("render board");

        let rendered = terminal_buffer_lines(&terminal).join("\n");
        assert!(
            rendered.contains("Points"),
            "board header should render aliased column name: {rendered}"
        );
        assert!(
            !rendered.contains("Complexity"),
            "board header should prefer alias over canonical heading: {rendered}"
        );

        let _ = std::fs::remove_file(&db_path);
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
            .find(|view| view.name == "TestView")
            .expect("TestView");
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
            .find(|view| view.name == "TestView")
            .expect("TestView");
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

    fn test_view_from_app(app: &App) -> View {
        app.views
            .iter()
            .find(|view| view.name == "TestView")
            .cloned()
            .expect("TestView should exist")
    }

    fn terminal_buffer_lines(terminal: &Terminal<TestBackend>) -> Vec<String> {
        let buf = terminal.backend().buffer();
        let area = buf.area;
        (0..area.height)
            .map(|y| {
                let mut line = String::new();
                for x in 0..area.width {
                    if let Some(cell) = buf.cell((x, y)) {
                        line.push_str(cell.symbol());
                    }
                }
                line
            })
            .collect()
    }

    #[test]
    fn view_picker_e_opens_view_edit() {
        let (store, db_path) = make_test_store_with_view("e-opens");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        app.mode = Mode::ViewPicker;
        app.picker_index = app
            .views
            .iter()
            .position(|view| view.name == "TestView")
            .expect("TestView should exist");

        app.handle_view_picker_key(KeyCode::Char('e'), &agenda)
            .expect("open view edit");

        assert_eq!(app.mode, Mode::ViewEdit);
        assert!(app.view_edit_state.is_some());

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn view_edit_criteria_rows_render_in_draft_order_and_space_toggles_selected_row() {
        let (store, db_path) = make_test_store_with_view("criteria-order-render");

        let critical = Category::new("Critical".to_string());
        let low = Category::new("Low".to_string());
        let medium = Category::new("Medium".to_string());
        store.create_category(&critical).expect("critical");
        store.create_category(&low).expect("low");
        store.create_category(&medium).expect("medium");

        let mut view = store
            .list_views()
            .expect("list views")
            .into_iter()
            .find(|v| v.name == "TestView")
            .expect("TestView");
        view.criteria.set_criterion(CriterionMode::And, medium.id);
        view.criteria.set_criterion(CriterionMode::Or, critical.id);
        view.criteria.set_criterion(CriterionMode::Not, low.id);
        store.update_view(&view).expect("update view");

        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);
        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        let view = app
            .views
            .iter()
            .find(|v| v.name == "TestView")
            .cloned()
            .expect("refreshed TestView");
        app.open_view_edit(view);

        let backend = TestBackend::new(140, 35);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal.draw(|frame| app.draw(frame)).expect("render");
        let text = terminal_buffer_lines(&terminal).join("\n");

        let medium_pos = text.find("Include: Medium").expect("Include row");
        let critical_pos = text.find("Match any: Critical").expect("Match any row");
        let low_pos = text.find("Exclude: Low").expect("Exclude row");
        assert!(
            medium_pos < critical_pos && critical_pos < low_pos,
            "criteria rows should preserve draft order in details pane"
        );

        app.handle_view_edit_key(KeyCode::Char(' '), &agenda)
            .expect("space toggles first criteria row");
        let state = app.view_edit_state.as_ref().expect("view edit state");
        assert_eq!(state.draft.criteria.criteria[0].category_id, medium.id);
        assert_eq!(state.draft.criteria.criteria[0].mode, CriterionMode::Not);

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn view_edit_existing_criteria_enter_opens_category_picker() {
        let (store, db_path) = make_test_store_with_view("criteria-enter-opens-picker");

        let medium = Category::new("Medium".to_string());
        store.create_category(&medium).expect("medium");

        let mut view = store
            .list_views()
            .expect("list views")
            .into_iter()
            .find(|v| v.name == "TestView")
            .expect("TestView");
        view.criteria.set_criterion(CriterionMode::And, medium.id);
        store.update_view(&view).expect("update view");

        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);
        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        let view = app
            .views
            .iter()
            .find(|v| v.name == "TestView")
            .cloned()
            .expect("refreshed TestView");
        app.open_view_edit(view);

        app.handle_view_edit_key(KeyCode::Enter, &agenda)
            .expect("enter opens category picker from criteria row");
        assert!(matches!(
            app.view_edit_state.as_ref().unwrap().overlay,
            Some(super::ViewEditOverlay::CategoryPicker {
                target: super::CategoryEditTarget::ViewCriteria
            })
        ));

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn view_edit_picker_space_cycles_criterion_mode() {
        let (store, db_path) = make_test_store_with_view("picker-mode-cycle");

        let complete = Category::new("Complete".to_string());
        store.create_category(&complete).expect("complete");

        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);
        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        let view = app
            .views
            .iter()
            .find(|v| v.name == "TestView")
            .cloned()
            .expect("refreshed TestView");
        app.open_view_edit(view);

        // No criteria yet — Space opens the picker
        app.handle_view_edit_key(KeyCode::Char(' '), &agenda)
            .expect("space opens picker when criteria empty");
        assert!(matches!(
            app.view_edit_state.as_ref().unwrap().overlay,
            Some(super::ViewEditOverlay::CategoryPicker {
                target: super::CategoryEditTarget::ViewCriteria
            })
        ));

        // Find the picker index for "Complete"
        let complete_picker_idx = app
            .category_rows
            .iter()
            .position(|r| r.id == complete.id)
            .expect("Complete in category_rows");
        app.view_edit_state.as_mut().unwrap().picker_index = complete_picker_idx;

        // First Space in picker: off → Include
        app.handle_view_edit_key(KeyCode::Char(' '), &agenda)
            .expect("picker space 1");
        let state = app.view_edit_state.as_ref().unwrap();
        assert_eq!(
            state.draft.criteria.mode_for(complete.id),
            Some(CriterionMode::And),
            "first press should set Include"
        );

        // Second Space in picker: Include → Exclude
        app.handle_view_edit_key(KeyCode::Char(' '), &agenda)
            .expect("picker space 2");
        let state = app.view_edit_state.as_ref().unwrap();
        assert_eq!(
            state.draft.criteria.mode_for(complete.id),
            Some(CriterionMode::Not),
            "second press should set Exclude"
        );

        // Third Space in picker: Exclude → Match any
        app.handle_view_edit_key(KeyCode::Char(' '), &agenda)
            .expect("picker space 3");
        let state = app.view_edit_state.as_ref().unwrap();
        assert_eq!(
            state.draft.criteria.mode_for(complete.id),
            Some(CriterionMode::Or),
            "third press should set Match any"
        );

        // Fourth Space in picker: Match any → off (removed)
        app.handle_view_edit_key(KeyCode::Char(' '), &agenda)
            .expect("picker space 4");
        let state = app.view_edit_state.as_ref().unwrap();
        assert_eq!(
            state.draft.criteria.mode_for(complete.id),
            None,
            "fourth press should remove criterion"
        );

        // Set it to Exclude and close the picker
        app.handle_view_edit_key(KeyCode::Char(' '), &agenda)
            .expect("set include");
        app.handle_view_edit_key(KeyCode::Char(' '), &agenda)
            .expect("set exclude");
        assert_eq!(
            app.view_edit_state
                .as_ref()
                .unwrap()
                .draft
                .criteria
                .mode_for(complete.id),
            Some(CriterionMode::Not)
        );

        // Close picker with Esc
        app.handle_view_edit_key(KeyCode::Esc, &agenda)
            .expect("close picker");
        assert!(app.view_edit_state.as_ref().unwrap().overlay.is_none());

        // Back in criteria region, Space should cycle the criterion mode
        app.handle_view_edit_key(KeyCode::Char(' '), &agenda)
            .expect("space cycles in criteria region");
        assert_eq!(
            app.view_edit_state
                .as_ref()
                .unwrap()
                .draft
                .criteria
                .criteria[0]
                .mode,
            CriterionMode::Or,
            "space in criteria region should cycle Not → Or"
        );

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn view_edit_tab_cycles_panes() {
        let (store, db_path) = make_test_store_with_view("tab-cycle");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        let view = test_view_from_app(&app);
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
        let view = test_view_from_app(&app);
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
        let view = test_view_from_app(&app);
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
        let view = test_view_from_app(&app);
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

        app.handle_view_edit_key(KeyCode::Esc, &agenda)
            .expect("keep editing");
        assert!(app.view_edit_state.is_some());
        assert!(!app
            .view_edit_state
            .as_ref()
            .map(|s| s.discard_confirm)
            .unwrap_or(true));

        app.handle_view_edit_key(KeyCode::Esc, &agenda)
            .expect("esc prompts again");
        app.handle_view_edit_key(KeyCode::Char('y'), &agenda)
            .expect("confirm save and close");
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
        let mut view = test_view_from_app(&app);
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
        let view = test_view_from_app(&app);
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
    fn view_edit_details_jk_moves_between_criteria_and_view_aux_rows() {
        let (store, db_path) = make_test_store_with_view("details-jk-view-rows");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        let view = test_view_from_app(&app);
        app.open_view_edit(view);

        assert_eq!(
            app.view_edit_state.as_ref().unwrap().region,
            ViewEditRegion::Criteria
        );

        app.handle_view_edit_key(KeyCode::Char('j'), &agenda)
            .expect("criteria -> when include");
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().region,
            ViewEditRegion::Unmatched
        );
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().unmatched_field_index,
            0
        );

        app.handle_view_edit_key(KeyCode::Char('j'), &agenda)
            .expect("when include -> when exclude");
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().region,
            ViewEditRegion::Unmatched
        );
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().unmatched_field_index,
            1
        );

        app.handle_view_edit_key(KeyCode::Char('j'), &agenda)
            .expect("when exclude -> display mode");
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().region,
            ViewEditRegion::Unmatched
        );
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().unmatched_field_index,
            2
        );

        app.handle_view_edit_key(KeyCode::Char('j'), &agenda)
            .expect("display mode -> unmatched visible");
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().unmatched_field_index,
            3
        );

        app.handle_view_edit_key(KeyCode::Char('j'), &agenda)
            .expect("unmatched visible -> unmatched label");
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().unmatched_field_index,
            4
        );

        app.handle_view_edit_key(KeyCode::Char('k'), &agenda)
            .expect("unmatched label -> unmatched visible");
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().unmatched_field_index,
            3
        );

        app.handle_view_edit_key(KeyCode::Char('k'), &agenda)
            .expect("unmatched visible -> display mode");
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().unmatched_field_index,
            2
        );

        app.handle_view_edit_key(KeyCode::Char('k'), &agenda)
            .expect("display mode -> when exclude");
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().unmatched_field_index,
            1
        );

        app.handle_view_edit_key(KeyCode::Char('k'), &agenda)
            .expect("when exclude -> when include");
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().unmatched_field_index,
            0
        );

        app.handle_view_edit_key(KeyCode::Char('k'), &agenda)
            .expect("when include -> criteria");
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
        let view = test_view_from_app(&app);
        app.open_view_edit(view);

        app.handle_view_edit_key(KeyCode::Char('j'), &agenda)
            .expect("to when include row");
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().region,
            ViewEditRegion::Unmatched
        );
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().unmatched_field_index,
            0
        );

        app.handle_view_edit_key(KeyCode::Char('j'), &agenda)
            .expect("to when exclude row");
        app.handle_view_edit_key(KeyCode::Char('j'), &agenda)
            .expect("to display mode row");
        app.handle_view_edit_key(KeyCode::Char('j'), &agenda)
            .expect("to unmatched visible row");
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().unmatched_field_index,
            3
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
            4
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
    fn view_edit_alias_row_enter_opens_alias_picker_and_saves_value() {
        let (store, db_path) = make_test_store_with_view("view-edit-alias-picker-save");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let category = Category::new("Project".to_string());
        store.create_category(&category).expect("create category");

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        let view = test_view_from_app(&app);
        app.open_view_edit(view);

        // Move focus to Aliases row in view details.
        for _ in 0..6 {
            app.handle_view_edit_key(KeyCode::Char('j'), &agenda)
                .expect("move details selection");
        }
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().unmatched_field_index,
            5
        );

        app.handle_view_edit_key(KeyCode::Enter, &agenda)
            .expect("open aliases picker");
        assert!(matches!(
            app.view_edit_state.as_ref().unwrap().overlay,
            Some(super::ViewEditOverlay::CategoryPicker {
                target: super::CategoryEditTarget::ViewAliases
            })
        ));

        // Select Project row in picker and edit alias.
        let project_idx = app
            .category_rows
            .iter()
            .position(|row| row.id == category.id)
            .expect("project row");
        app.view_edit_state.as_mut().unwrap().picker_index = project_idx;
        app.handle_view_edit_key(KeyCode::Char('A'), &agenda)
            .expect("start alias input");
        assert!(matches!(
            app.view_edit_state.as_ref().unwrap().inline_input,
            Some(super::ViewEditInlineInput::CategoryAlias { category_id })
                if category_id == category.id
        ));

        if let Some(state) = &mut app.view_edit_state {
            state.inline_buf = super::text_buffer::TextBuffer::new("Client".to_string());
        }
        app.handle_view_edit_key(KeyCode::Enter, &agenda)
            .expect("save alias");
        assert_eq!(
            app.view_edit_state
                .as_ref()
                .unwrap()
                .draft
                .category_aliases
                .get(&category.id)
                .map(String::as_str),
            Some("Client")
        );

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn view_edit_alias_input_empty_enter_clears_alias() {
        let (store, db_path) = make_test_store_with_view("view-edit-alias-picker-clear");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let category = Category::new("Project".to_string());
        store.create_category(&category).expect("create category");

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        let mut view = test_view_from_app(&app);
        view.category_aliases
            .insert(category.id, "Client".to_string());
        app.open_view_edit(view);

        // Enter unmatched details, then open aliases picker.
        app.handle_view_edit_key(KeyCode::Char('j'), &agenda)
            .expect("move to unmatched details");
        app.handle_view_edit_key(KeyCode::Char('A'), &agenda)
            .expect("open aliases picker");
        assert!(matches!(
            app.view_edit_state.as_ref().unwrap().overlay,
            Some(super::ViewEditOverlay::CategoryPicker {
                target: super::CategoryEditTarget::ViewAliases
            })
        ));

        let project_idx = app
            .category_rows
            .iter()
            .position(|row| row.id == category.id)
            .expect("project row");
        app.view_edit_state.as_mut().unwrap().picker_index = project_idx;
        app.handle_view_edit_key(KeyCode::Enter, &agenda)
            .expect("start alias input");
        if let Some(state) = &mut app.view_edit_state {
            state.inline_buf = super::text_buffer::TextBuffer::new(String::new());
        }
        app.handle_view_edit_key(KeyCode::Enter, &agenda)
            .expect("commit empty alias");
        assert!(
            !app.view_edit_state
                .as_ref()
                .unwrap()
                .draft
                .category_aliases
                .contains_key(&category.id),
            "empty alias input should clear alias mapping"
        );

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn view_edit_view_details_enter_opens_when_picker_and_toggles_display_mode() {
        let (store, db_path) = make_test_store_with_view("view-details-enter-actions");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        let view = test_view_from_app(&app);
        app.open_view_edit(view);

        app.handle_view_edit_key(KeyCode::Char('j'), &agenda)
            .expect("to when include row");
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().unmatched_field_index,
            0
        );
        app.handle_view_edit_key(KeyCode::Enter, &agenda)
            .expect("open when include picker");
        assert!(matches!(
            app.view_edit_state.as_ref().unwrap().overlay,
            Some(super::ViewEditOverlay::BucketPicker {
                target: super::BucketEditTarget::ViewVirtualInclude
            })
        ));
        app.handle_view_edit_key(KeyCode::Esc, &agenda)
            .expect("close bucket picker");

        app.handle_view_edit_key(KeyCode::Char('j'), &agenda)
            .expect("to when exclude row");
        app.handle_view_edit_key(KeyCode::Char('j'), &agenda)
            .expect("to display mode row");
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().unmatched_field_index,
            2
        );
        let before = app
            .view_edit_state
            .as_ref()
            .unwrap()
            .draft
            .board_display_mode;
        app.handle_view_edit_key(KeyCode::Enter, &agenda)
            .expect("toggle display mode via details row");
        assert_ne!(
            app.view_edit_state
                .as_ref()
                .unwrap()
                .draft
                .board_display_mode,
            before
        );

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn view_edit_empty_criteria_enter_opens_category_picker() {
        let (store, db_path) = make_test_store_with_view("view-empty-criteria-enter");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        let view = test_view_from_app(&app);
        app.open_view_edit(view);

        assert_eq!(
            app.view_edit_state.as_ref().unwrap().region,
            ViewEditRegion::Criteria
        );
        assert!(app
            .view_edit_state
            .as_ref()
            .unwrap()
            .draft
            .criteria
            .criteria
            .is_empty());

        app.handle_view_edit_key(KeyCode::Enter, &agenda)
            .expect("enter on empty criteria opens picker");
        assert!(matches!(
            app.view_edit_state.as_ref().unwrap().overlay,
            Some(super::ViewEditOverlay::CategoryPicker {
                target: super::CategoryEditTarget::ViewCriteria
            })
        ));

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn view_edit_section_details_enter_toggles_expanded_row() {
        let (store, db_path) = make_test_store_with_view("section-details-expand-row");
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
            .expect("to sections pane");
        app.handle_view_edit_key(KeyCode::Char('j'), &agenda)
            .expect("select section row");
        app.handle_view_edit_key(KeyCode::Tab, &agenda)
            .expect("to details pane");
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().pane_focus,
            ViewEditPaneFocus::Details
        );
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().region,
            ViewEditRegion::Sections
        );

        for _ in 0..7 {
            app.handle_view_edit_key(KeyCode::Char('j'), &agenda)
                .expect("advance details field");
        }
        assert_eq!(
            app.view_edit_state
                .as_ref()
                .unwrap()
                .section_details_field_index,
            7
        );
        assert_eq!(app.view_edit_state.as_ref().unwrap().section_expanded, None);

        app.handle_view_edit_key(KeyCode::Enter, &agenda)
            .expect("toggle expanded from details row");
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().section_expanded,
            Some(0)
        );

        app.handle_view_edit_key(KeyCode::Enter, &agenda)
            .expect("toggle expanded off from details row");
        assert_eq!(app.view_edit_state.as_ref().unwrap().section_expanded, None);

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn view_edit_section_details_enter_opens_picker_backed_rows() {
        let (store, db_path) = make_test_store_with_view("section-details-picker-rows");
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
            .expect("to sections");
        app.handle_view_edit_key(KeyCode::Char('j'), &agenda)
            .expect("select section");
        app.handle_view_edit_key(KeyCode::Tab, &agenda)
            .expect("to details");

        // Field 1: Criteria
        app.handle_view_edit_key(KeyCode::Char('j'), &agenda)
            .expect("to criteria field");
        app.handle_view_edit_key(KeyCode::Enter, &agenda)
            .expect("open section criteria picker");
        assert!(matches!(
            app.view_edit_state.as_ref().unwrap().overlay,
            Some(super::ViewEditOverlay::CategoryPicker {
                target: super::CategoryEditTarget::SectionCriteria
            })
        ));
        app.handle_view_edit_key(KeyCode::Esc, &agenda)
            .expect("close picker");

        // Field 2: Columns
        app.handle_view_edit_key(KeyCode::Char('j'), &agenda)
            .expect("to columns field");
        app.handle_view_edit_key(KeyCode::Enter, &agenda)
            .expect("open section columns picker");
        assert!(matches!(
            app.view_edit_state.as_ref().unwrap().overlay,
            Some(super::ViewEditOverlay::CategoryPicker {
                target: super::CategoryEditTarget::SectionColumns
            })
        ));
        app.handle_view_edit_key(KeyCode::Esc, &agenda)
            .expect("close picker");

        // Field 3: On insert assign
        app.handle_view_edit_key(KeyCode::Char('j'), &agenda)
            .expect("to on-insert field");
        app.handle_view_edit_key(KeyCode::Enter, &agenda)
            .expect("open on-insert picker");
        assert!(matches!(
            app.view_edit_state.as_ref().unwrap().overlay,
            Some(super::ViewEditOverlay::CategoryPicker {
                target: super::CategoryEditTarget::SectionOnInsertAssign
            })
        ));
        app.handle_view_edit_key(KeyCode::Esc, &agenda)
            .expect("close picker");

        // Field 4: On remove unassign
        app.handle_view_edit_key(KeyCode::Char('j'), &agenda)
            .expect("to on-remove field");
        app.handle_view_edit_key(KeyCode::Enter, &agenda)
            .expect("open on-remove picker");
        assert!(matches!(
            app.view_edit_state.as_ref().unwrap().overlay,
            Some(super::ViewEditOverlay::CategoryPicker {
                target: super::CategoryEditTarget::SectionOnRemoveUnassign
            })
        ));
        app.handle_view_edit_key(KeyCode::Esc, &agenda)
            .expect("close picker");

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn view_edit_section_x_prompts_before_delete_and_y_confirms() {
        let (store, db_path) = make_test_store_with_view("section-x-delete-confirm");
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
            .expect("to sections pane");
        app.handle_view_edit_key(KeyCode::Char('j'), &agenda)
            .expect("select section row");

        assert_eq!(
            app.view_edit_state.as_ref().unwrap().draft.sections.len(),
            1
        );
        app.handle_view_edit_key(KeyCode::Char('x'), &agenda)
            .expect("request delete");
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().draft.sections.len(),
            1
        );
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().section_delete_confirm,
            Some(0)
        );

        app.handle_view_edit_key(KeyCode::Esc, &agenda)
            .expect("decline delete");
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().section_delete_confirm,
            None
        );
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().draft.sections.len(),
            1
        );

        app.handle_view_edit_key(KeyCode::Char('x'), &agenda)
            .expect("request delete again");
        app.handle_view_edit_key(KeyCode::Char('y'), &agenda)
            .expect("confirm delete");
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().draft.sections.len(),
            0
        );
        assert!(app
            .view_edit_state
            .as_ref()
            .unwrap()
            .draft
            .sections
            .iter()
            .all(|s| s.title != "Alpha"));

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn view_edit_section_details_x_prompts_before_delete() {
        let (store, db_path) = make_test_store_with_view("section-details-x-delete-confirm");
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
            .expect("to sections");
        app.handle_view_edit_key(KeyCode::Char('j'), &agenda)
            .expect("select section");
        app.handle_view_edit_key(KeyCode::Tab, &agenda)
            .expect("to details");
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().pane_focus,
            ViewEditPaneFocus::Details
        );
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().region,
            ViewEditRegion::Sections
        );

        app.handle_view_edit_key(KeyCode::Char('x'), &agenda)
            .expect("details x requests delete");
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().section_delete_confirm,
            Some(0)
        );
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().draft.sections.len(),
            1
        );

        app.handle_view_edit_key(KeyCode::Esc, &agenda)
            .expect("cancel delete confirm");
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().section_delete_confirm,
            None
        );
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().draft.sections.len(),
            1
        );

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

        let home_idx = app
            .category_rows
            .iter()
            .position(|r| r.name == "Home")
            .expect("home row");
        let work_idx = app
            .category_rows
            .iter()
            .position(|r| r.name == "Work")
            .expect("work row");

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
    fn view_edit_category_picker_type_filter_updates_selected_match() {
        let (store, db_path) = make_test_store_with_view("picker-type-filter");
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
        let home_idx = app
            .category_rows
            .iter()
            .position(|r| r.name == "Home")
            .expect("home row");

        if let Some(state) = &mut app.view_edit_state {
            state.picker_index = home_idx;
        }
        app.handle_view_edit_key(KeyCode::Char('h'), &agenda)
            .expect("type picker filter");
        app.handle_view_edit_key(KeyCode::Char('o'), &agenda)
            .expect("type picker filter");
        let state = app.view_edit_state.as_ref().expect("view edit state");
        assert_eq!(state.overlay_filter_buf.text(), "ho");
        assert_eq!(
            state.picker_index, home_idx,
            "filter should select first matching row"
        );

        app.handle_view_edit_key(KeyCode::Enter, &agenda)
            .expect("toggle filtered category");
        assert!(app
            .view_edit_state
            .as_ref()
            .unwrap()
            .draft
            .criteria
            .mode_for(home.id)
            .is_some());

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
    fn view_edit_section_uppercase_jk_reorders_sections() {
        let (store, db_path) = make_test_store_with_view("section-uppercase-jk-reorder");
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
            .expect("tab to sections");
        app.handle_view_edit_key(KeyCode::Char('j'), &agenda)
            .expect("select first section");
        app.handle_view_edit_key(KeyCode::Char('j'), &agenda)
            .expect("select second section");
        assert_eq!(app.view_edit_state.as_ref().unwrap().section_index, 1);

        app.handle_view_edit_key(KeyCode::Char('K'), &agenda)
            .expect("move selected section up");
        let state = app.view_edit_state.as_ref().unwrap();
        assert_eq!(state.section_index, 0);
        assert_eq!(state.draft.sections[0].title, "Bravo");
        assert_eq!(state.draft.sections[1].title, "Alpha");

        app.handle_view_edit_key(KeyCode::Char('J'), &agenda)
            .expect("move selected section down");
        let state = app.view_edit_state.as_ref().unwrap();
        assert_eq!(state.section_index, 1);
        assert_eq!(state.draft.sections[0].title, "Alpha");
        assert_eq!(state.draft.sections[1].title, "Bravo");

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
    fn view_edit_view_row_r_starts_name_edit_and_enter_saves_draft_name() {
        let (store, db_path) = make_test_store_with_view("view-row-rename");
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
            .expect("tab to sections");
        assert!(
            app.view_edit_state
                .as_ref()
                .expect("state")
                .sections_view_row_selected
        );

        app.handle_view_edit_key(KeyCode::Char('r'), &agenda)
            .expect("start view rename");
        assert!(matches!(
            app.view_edit_state.as_ref().unwrap().inline_input,
            Some(super::ViewEditInlineInput::ViewName)
        ));

        if let Some(state) = &mut app.view_edit_state {
            state.inline_buf = super::text_buffer::TextBuffer::new("UX Board".to_string());
        }
        app.handle_view_edit_key(KeyCode::Enter, &agenda)
            .expect("commit view name");

        let state = app.view_edit_state.as_ref().expect("state");
        assert_eq!(state.draft.name, "UX Board");
        assert!(state.dirty, "renaming view should mark draft dirty");

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn view_edit_slash_opens_section_filter_and_esc_clears_filter_before_close() {
        let (store, db_path) = make_test_store_with_view("view-edit-sections-filter");
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

        app.handle_view_edit_key(KeyCode::Char('/'), &agenda)
            .expect("open section filter");
        let state = app.view_edit_state.as_ref().expect("view edit state");
        assert_eq!(state.pane_focus, ViewEditPaneFocus::Sections);
        assert!(matches!(
            state.inline_input,
            Some(super::ViewEditInlineInput::SectionsFilter)
        ));

        app.handle_view_edit_key(KeyCode::Char('b'), &agenda)
            .expect("type filter");
        assert_eq!(
            app.view_edit_state
                .as_ref()
                .unwrap()
                .sections_filter_buf
                .text(),
            "b"
        );

        app.handle_view_edit_key(KeyCode::Enter, &agenda)
            .expect("finish filter edit");
        assert!(app.view_edit_state.as_ref().unwrap().inline_input.is_none());
        assert_eq!(
            app.view_edit_state
                .as_ref()
                .unwrap()
                .sections_filter_buf
                .text(),
            "b"
        );

        app.handle_view_edit_key(KeyCode::Esc, &agenda)
            .expect("esc clears active filter before closing");
        assert_eq!(app.mode, Mode::ViewEdit, "editor should stay open");
        assert_eq!(
            app.view_edit_state
                .as_ref()
                .unwrap()
                .sections_filter_buf
                .text(),
            ""
        );

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn view_edit_p_toggles_preview_and_tab_cycles_preview_pane() {
        let (store, db_path) = make_test_store_with_view("view-edit-preview-toggle");
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

        assert!(!app.view_edit_state.as_ref().unwrap().preview_visible);
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().pane_focus,
            ViewEditPaneFocus::Details
        );

        app.handle_view_edit_key(KeyCode::Char('p'), &agenda)
            .expect("show preview");
        assert!(app.view_edit_state.as_ref().unwrap().preview_visible);

        app.handle_view_edit_key(KeyCode::Tab, &agenda)
            .expect("details -> preview");
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().pane_focus,
            ViewEditPaneFocus::Preview
        );

        app.handle_view_edit_key(KeyCode::BackTab, &agenda)
            .expect("preview -> details");
        assert_eq!(
            app.view_edit_state.as_ref().unwrap().pane_focus,
            ViewEditPaneFocus::Details
        );

        app.handle_view_edit_key(KeyCode::Tab, &agenda)
            .expect("details -> preview again");
        app.handle_view_edit_key(KeyCode::Char('p'), &agenda)
            .expect("hide preview while preview focused");
        let state = app.view_edit_state.as_ref().unwrap();
        assert!(!state.preview_visible);
        assert_eq!(state.pane_focus, ViewEditPaneFocus::Sections);

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn view_edit_preview_renders_on_narrow_terminal_without_panic() {
        let (store, db_path) = make_test_store_with_view("view-edit-preview-narrow-render");
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
        app.handle_view_edit_key(KeyCode::Char('p'), &agenda)
            .expect("show preview");
        app.handle_view_edit_key(KeyCode::Tab, &agenda)
            .expect("details -> preview focus");

        let backend = TestBackend::new(90, 28);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| app.draw(frame))
            .expect("narrow preview render should not panic");

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn view_edit_section_c_opens_columns_picker_and_toggles_column() {
        let (store, db_path) = make_test_store_with_view("section-c-columns");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let work = Category::new("Work".to_string());
        store.create_category(&work).expect("create work category");
        // Give Work a child so it qualifies as a valid column heading.
        let mut sub = Category::new("SubWork".to_string());
        sub.parent = Some(work.id);
        store.create_category(&sub).expect("create sub category");

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

        // Navigate to Work — it has children so it's a valid column heading.
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
    fn section_column_picker_excludes_leaf_tag_headings() {
        let (store, db_path) = make_test_store_with_view("col-picker-leaf-tag");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        // Leaf tag category — should be hidden from column picker.
        let leaf = Category::new("OrphanTag".to_string());
        store.create_category(&leaf).expect("create leaf");

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        let view = app
            .views
            .iter()
            .find(|v| v.name == "TestView")
            .cloned()
            .unwrap();
        app.open_view_edit(view);

        app.handle_view_edit_key(KeyCode::Tab, &agenda).unwrap();
        app.handle_view_edit_key(KeyCode::Char('n'), &agenda)
            .unwrap();
        app.handle_view_edit_key(KeyCode::Enter, &agenda).unwrap();
        app.handle_view_edit_key(KeyCode::Char('c'), &agenda)
            .unwrap();

        // Attempt to toggle the leaf category via its raw index.
        let leaf_idx = app
            .category_rows
            .iter()
            .position(|r| r.name == "OrphanTag")
            .unwrap();
        if let Some(state) = &mut app.view_edit_state {
            state.picker_index = leaf_idx;
        }
        app.handle_view_edit_key(KeyCode::Enter, &agenda).unwrap();

        // OrphanTag should NOT have been added (filtered out).
        assert!(
            !app.view_edit_state.as_ref().unwrap().draft.sections[0]
                .columns
                .iter()
                .any(|c| c.heading == leaf.id),
            "leaf tag category should be excluded from column picker"
        );

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn section_column_picker_includes_non_leaf_tag_headings() {
        let (store, db_path) = make_test_store_with_view("col-picker-nonleaf");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let parent = Category::new("Status".to_string());
        store.create_category(&parent).expect("create parent");
        let mut child = Category::new("Active".to_string());
        child.parent = Some(parent.id);
        store.create_category(&child).expect("create child");

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        let view = app
            .views
            .iter()
            .find(|v| v.name == "TestView")
            .cloned()
            .unwrap();
        app.open_view_edit(view);

        app.handle_view_edit_key(KeyCode::Tab, &agenda).unwrap();
        app.handle_view_edit_key(KeyCode::Char('n'), &agenda)
            .unwrap();
        app.handle_view_edit_key(KeyCode::Enter, &agenda).unwrap();
        app.handle_view_edit_key(KeyCode::Char('c'), &agenda)
            .unwrap();

        let status_idx = app
            .category_rows
            .iter()
            .position(|r| r.name == "Status")
            .unwrap();
        if let Some(state) = &mut app.view_edit_state {
            state.picker_index = status_idx;
        }
        app.handle_view_edit_key(KeyCode::Enter, &agenda).unwrap();

        assert!(
            app.view_edit_state.as_ref().unwrap().draft.sections[0]
                .columns
                .iter()
                .any(|c| c.heading == parent.id),
            "non-leaf tag category should be selectable as column heading"
        );

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn section_column_picker_includes_nested_non_leaf_tag_headings() {
        let (store, db_path) = make_test_store_with_view("col-picker-nested-nonleaf");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let project = Category::new("Project".to_string());
        store.create_category(&project).expect("create project");
        let mut phase = Category::new("Phase".to_string());
        phase.parent = Some(project.id);
        store.create_category(&phase).expect("create phase");
        let mut task = Category::new("Task".to_string());
        task.parent = Some(phase.id);
        store.create_category(&task).expect("create task");

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        let view = app
            .views
            .iter()
            .find(|v| v.name == "TestView")
            .cloned()
            .unwrap();
        app.open_view_edit(view);

        app.handle_view_edit_key(KeyCode::Tab, &agenda).unwrap();
        app.handle_view_edit_key(KeyCode::Char('n'), &agenda)
            .unwrap();
        app.handle_view_edit_key(KeyCode::Enter, &agenda).unwrap();
        app.handle_view_edit_key(KeyCode::Char('c'), &agenda)
            .unwrap();

        let phase_idx = app
            .category_rows
            .iter()
            .position(|r| r.name == "Phase")
            .expect("phase row");
        if let Some(state) = &mut app.view_edit_state {
            state.picker_index = phase_idx;
        }
        app.handle_view_edit_key(KeyCode::Enter, &agenda).unwrap();

        assert!(
            app.view_edit_state.as_ref().unwrap().draft.sections[0]
                .columns
                .iter()
                .any(|c| c.heading == phase.id),
            "nested non-leaf tag category should be selectable as column heading"
        );

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn section_column_picker_includes_numeric_leaf_headings() {
        let (store, db_path) = make_test_store_with_view("col-picker-numeric");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut cost = Category::new("Cost".to_string());
        cost.value_kind = CategoryValueKind::Numeric;
        store.create_category(&cost).expect("create numeric");

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        let view = app
            .views
            .iter()
            .find(|v| v.name == "TestView")
            .cloned()
            .unwrap();
        app.open_view_edit(view);

        app.handle_view_edit_key(KeyCode::Tab, &agenda).unwrap();
        app.handle_view_edit_key(KeyCode::Char('n'), &agenda)
            .unwrap();
        app.handle_view_edit_key(KeyCode::Enter, &agenda).unwrap();
        app.handle_view_edit_key(KeyCode::Char('c'), &agenda)
            .unwrap();

        let cost_idx = app
            .category_rows
            .iter()
            .position(|r| r.name == "Cost")
            .unwrap();
        if let Some(state) = &mut app.view_edit_state {
            state.picker_index = cost_idx;
        }
        app.handle_view_edit_key(KeyCode::Enter, &agenda).unwrap();

        assert!(
            app.view_edit_state.as_ref().unwrap().draft.sections[0]
                .columns
                .iter()
                .any(|c| c.heading == cost.id),
            "numeric leaf category should be selectable as column heading"
        );

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

        // Filter slot 0 for "timeout" via search bar
        app.slot_index = 0;
        app.handle_normal_key(KeyCode::Char('/'), &agenda)
            .expect("open search bar");
        assert_eq!(app.mode, Mode::SearchBarFocused);

        // Type "timeout" — live-filters as we type
        for ch in "timeout".chars() {
            app.handle_search_bar_key(KeyCode::Char(ch), &agenda)
                .expect("type char");
        }
        // Unfocus to keep filter active
        app.handle_search_bar_key(KeyCode::Down, &agenda)
            .expect("unfocus search bar");

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
    fn search_bar_esc_clears_filter_and_buffer() {
        let (store, db_path) = make_two_section_store("esc-clears");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        app.set_view_selection_by_name("TestView");
        app.refresh(&store).expect("refresh TestView");

        // Type something in search bar
        app.slot_index = 0;
        app.handle_normal_key(KeyCode::Char('/'), &agenda)
            .expect("open search bar");
        for ch in "fix".chars() {
            app.handle_search_bar_key(KeyCode::Char(ch), &agenda)
                .expect("type char");
        }
        assert_eq!(app.section_filters[0], Some("fix".to_string()));

        // Esc should clear both buffer and filter
        app.handle_search_bar_key(KeyCode::Esc, &agenda)
            .expect("esc clears");
        assert_eq!(app.mode, Mode::Normal);
        assert!(
            app.search_buffer.is_empty(),
            "search buffer should be empty"
        );
        assert_eq!(app.section_filters[0], None, "filter should be cleared");

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

    #[test]
    fn search_bar_live_filtering() {
        let (store, db_path) = make_two_section_store("live-filter");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        app.set_view_selection_by_name("TestView");
        app.refresh(&store).expect("refresh TestView");

        app.slot_index = 0;
        app.handle_normal_key(KeyCode::Char('/'), &agenda)
            .expect("open search bar");

        // Type 'f' — should filter live
        app.handle_search_bar_key(KeyCode::Char('f'), &agenda)
            .expect("type f");
        assert_eq!(
            app.section_filters[0],
            Some("f".to_string()),
            "filter updates on each keystroke"
        );
        // "Fix timeout bug" matches "f"
        assert_eq!(app.slots[0].items.len(), 1, "one item matches 'f'");

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn search_bar_enter_exact_match_jumps() {
        let (store, db_path) = make_two_section_store("exact-match");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        app.set_view_selection_by_name("TestView");
        app.refresh(&store).expect("refresh TestView");

        app.slot_index = 0;
        app.handle_normal_key(KeyCode::Char('/'), &agenda)
            .expect("open search bar");

        for ch in "fix timeout bug".chars() {
            app.handle_search_bar_key(KeyCode::Char(ch), &agenda)
                .expect("type char");
        }
        app.handle_search_bar_key(KeyCode::Enter, &agenda)
            .expect("enter");

        assert_eq!(app.mode, Mode::Normal);
        assert_eq!(app.item_index, 0, "jumped to exact match");
        assert!(app.status.contains("Jumped to"), "status confirms jump");

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn search_bar_enter_creates_item_when_no_match() {
        let (store, db_path) = make_two_section_store("create");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        app.set_view_selection_by_name("TestView");
        app.refresh(&store).expect("refresh TestView");

        app.slot_index = 0;
        app.handle_normal_key(KeyCode::Char('/'), &agenda)
            .expect("open search bar");

        for ch in "Brand new task".chars() {
            app.handle_search_bar_key(KeyCode::Char(ch), &agenda)
                .expect("type char");
        }
        app.handle_search_bar_key(KeyCode::Enter, &agenda)
            .expect("enter creates");

        assert_eq!(app.mode, Mode::InputPanel, "opens InputPanel");
        assert!(app.input_panel.is_some(), "panel exists");
        let panel = app.input_panel.as_ref().unwrap();
        assert_eq!(panel.text.text(), "Brand new task", "title pre-filled");
        assert!(app.search_buffer.is_empty(), "search buffer cleared");
        assert_eq!(app.section_filters[0], None, "filter cleared");

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn search_bar_down_unfocuses_keeps_filter() {
        let (store, db_path) = make_two_section_store("down-unfocus");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        app.set_view_selection_by_name("TestView");
        app.refresh(&store).expect("refresh TestView");

        app.slot_index = 0;
        app.handle_normal_key(KeyCode::Char('/'), &agenda)
            .expect("open search bar");

        for ch in "fix".chars() {
            app.handle_search_bar_key(KeyCode::Char(ch), &agenda)
                .expect("type char");
        }

        app.handle_search_bar_key(KeyCode::Down, &agenda)
            .expect("down unfocuses");
        assert_eq!(app.mode, Mode::Normal, "back to normal");
        assert_eq!(
            app.section_filters[0],
            Some("fix".to_string()),
            "filter stays active"
        );

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn search_bar_slash_resumes_with_text() {
        let (store, db_path) = make_two_section_store("resume");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        app.set_view_selection_by_name("TestView");
        app.refresh(&store).expect("refresh TestView");

        app.slot_index = 0;
        app.handle_normal_key(KeyCode::Char('/'), &agenda)
            .expect("open search bar");

        for ch in "fix".chars() {
            app.handle_search_bar_key(KeyCode::Char(ch), &agenda)
                .expect("type char");
        }

        // Unfocus
        app.handle_search_bar_key(KeyCode::Down, &agenda)
            .expect("down");
        assert_eq!(app.mode, Mode::Normal);

        // Re-focus with /
        app.handle_normal_key(KeyCode::Char('/'), &agenda)
            .expect("reopen search bar");
        assert_eq!(app.mode, Mode::SearchBarFocused);
        assert_eq!(app.search_buffer.text(), "fix", "buffer retains text");

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn board_sorting_new_column_becomes_primary_and_previous_becomes_secondary() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!("agenda-tui-sort-secondary-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut priority = Category::new("Priority".to_string());
        priority.is_exclusive = true;
        store.create_category(&priority).expect("create priority");

        let mut high = Category::new("High".to_string());
        high.parent = Some(priority.id);
        store.create_category(&high).expect("create high");

        let mut low = Category::new("Low".to_string());
        low.parent = Some(priority.id);
        store.create_category(&low).expect("create low");

        let bravo = Item::new("Bravo".to_string());
        let alpha = Item::new("Alpha".to_string());
        let charlie = Item::new("Charlie".to_string());
        store.create_item(&bravo).expect("create bravo");
        store.create_item(&alpha).expect("create alpha");
        store.create_item(&charlie).expect("create charlie");

        agenda
            .assign_item_manual(bravo.id, high.id, Some("test:assign".to_string()))
            .expect("assign bravo high");
        agenda
            .assign_item_manual(alpha.id, high.id, Some("test:assign".to_string()))
            .expect("assign alpha high");
        agenda
            .assign_item_manual(charlie.id, low.id, Some("test:assign".to_string()))
            .expect("assign charlie low");

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
        store.create_view(&view).expect("create board view");

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        app.set_view_selection_by_name("Board");
        app.refresh(&store).expect("refresh board");

        app.column_index = 0;
        app.handle_key(KeyCode::Char('s'), &agenda)
            .expect("sort item asc");
        app.column_index = 1;
        app.handle_key(KeyCode::Char('s'), &agenda)
            .expect("sort priority asc");

        let order: Vec<String> = app.slots[0]
            .items
            .iter()
            .map(|item| item.text.clone())
            .collect();
        assert_eq!(
            order,
            vec![
                "Alpha".to_string(),
                "Bravo".to_string(),
                "Charlie".to_string()
            ]
        );
        assert_eq!(app.slot_sort_keys[0].len(), 2);

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn board_sorting_numeric_missing_values_are_last_for_asc_and_desc() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!("agenda-tui-sort-missing-{nanos}.ag"));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut cost = Category::new("Cost".to_string());
        cost.value_kind = CategoryValueKind::Numeric;
        store.create_category(&cost).expect("create cost");

        let ten = Item::new("Ten".to_string());
        let missing = Item::new("Missing".to_string());
        let five = Item::new("Five".to_string());
        store.create_item(&ten).expect("create ten");
        store.create_item(&missing).expect("create missing");
        store.create_item(&five).expect("create five");

        agenda
            .assign_item_numeric_manual(
                ten.id,
                cost.id,
                rust_decimal::Decimal::new(10, 0),
                Some("test:assign".to_string()),
            )
            .expect("assign ten");
        agenda
            .assign_item_numeric_manual(
                five.id,
                cost.id,
                rust_decimal::Decimal::new(5, 0),
                Some("test:assign".to_string()),
            )
            .expect("assign five");

        let mut view = View::new("Board".to_string());
        view.sections.push(Section {
            title: "Main".to_string(),
            criteria: Query::default(),
            columns: vec![Column {
                kind: ColumnKind::Standard,
                heading: cost.id,
                width: 12,
            }],
            item_column_index: 0,
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        });
        store.create_view(&view).expect("create board view");

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        app.set_view_selection_by_name("Board");
        app.refresh(&store).expect("refresh board");

        app.column_index = 1;
        app.handle_key(KeyCode::Char('s'), &agenda)
            .expect("sort cost asc");
        assert_eq!(app.slot_sort_keys[0][0].direction, SlotSortDirection::Asc);
        let asc_order: Vec<String> = app.slots[0]
            .items
            .iter()
            .map(|item| item.text.clone())
            .collect();
        assert_eq!(
            asc_order,
            vec!["Five".to_string(), "Ten".to_string(), "Missing".to_string()]
        );

        app.handle_key(KeyCode::Char('s'), &agenda)
            .expect("sort cost desc");
        assert_eq!(app.slot_sort_keys[0][0].direction, SlotSortDirection::Desc);
        let desc_order: Vec<String> = app.slots[0]
            .items
            .iter()
            .map(|item| item.text.clone())
            .collect();
        assert_eq!(
            desc_order,
            vec!["Ten".to_string(), "Five".to_string(), "Missing".to_string()]
        );

        app.handle_key(KeyCode::Char('s'), &agenda)
            .expect("clear sort");
        assert!(
            app.slot_sort_keys[0].is_empty(),
            "third sort press on primary should clear that key"
        );

        let _ = std::fs::remove_file(&db_path);
    }

    // --- Phase 6: Numeric cell editing tests ---

    /// Helper: create a store with a numeric "Cost" category, a view with the Cost column,
    /// and one item. Returns (store, classifier, cost_category_id, item_id, db_path).
    fn setup_numeric_column_board(
        suffix: &str,
    ) -> (
        Store,
        SubstringClassifier,
        CategoryId,
        ItemId,
        std::path::PathBuf,
    ) {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!(
            "agenda-tui-numeric-{suffix}-{nanos}-{}.ag",
            std::process::id()
        ));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;

        let mut cost = Category::new("Cost".to_string());
        cost.value_kind = CategoryValueKind::Numeric;
        store.create_category(&cost).expect("create Cost");

        let item = Item::new("Test expense".to_string());
        store.create_item(&item).expect("create item");

        let mut view = View::new("Board".to_string());
        view.sections.push(Section {
            title: "Main".to_string(),
            criteria: Query::default(),
            columns: vec![Column {
                kind: ColumnKind::Standard,
                heading: cost.id,
                width: 12,
            }],
            item_column_index: 0,
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        });
        store.create_view(&view).expect("create view");

        (store, classifier, cost.id, item.id, db_path)
    }

    #[test]
    fn numeric_column_enter_opens_numeric_editor_not_category_picker() {
        let (store, classifier, _cost_id, _item_id, db_path) = setup_numeric_column_board("enter");
        let agenda = Agenda::new(&store, &classifier);

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        app.set_view_selection_by_name("Board");
        app.refresh(&store).expect("refresh board");
        // Column 0 = item, column 1 = Cost
        app.column_index = 1;

        app.handle_key(KeyCode::Enter, &agenda)
            .expect("press Enter on numeric column");

        assert_eq!(app.mode, Mode::InputPanel);
        assert_eq!(
            app.name_input_context,
            Some(NameInputContext::NumericValueEdit)
        );
        assert_eq!(
            app.input_panel.as_ref().map(|panel| panel.kind),
            Some(input_panel::InputPanelKind::NumericValue)
        );
        assert!(
            app.category_column_picker.is_none(),
            "should not open category picker for numeric columns"
        );

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn numeric_column_edit_saves_value_and_returns_to_normal() {
        let (store, classifier, cost_id, item_id, db_path) = setup_numeric_column_board("save");
        let agenda = Agenda::new(&store, &classifier);

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        app.set_view_selection_by_name("Board");
        app.refresh(&store).expect("refresh board");
        app.column_index = 1;

        // Open editor
        app.handle_key(KeyCode::Enter, &agenda)
            .expect("open numeric editor");
        assert_eq!(app.mode, Mode::InputPanel);

        // Type a value
        for ch in "245.96".chars() {
            app.handle_key(KeyCode::Char(ch), &agenda)
                .expect("type digit");
        }

        // Enter in value field saves directly.
        app.handle_key(KeyCode::Enter, &agenda)
            .expect("save numeric value");

        assert_eq!(app.mode, Mode::Normal);
        assert!(app.status.contains("245.96"));

        // Verify persisted
        let assignments = store
            .get_assignments_for_item(item_id)
            .expect("assignments");
        let value = assignments
            .get(&cost_id)
            .and_then(|a| a.numeric_value)
            .expect("should have numeric value");
        assert_eq!(value, rust_decimal::Decimal::new(24596, 2));

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn numeric_column_edit_invalid_input_shows_error_and_keeps_panel_open() {
        let (store, classifier, _cost_id, _item_id, db_path) =
            setup_numeric_column_board("invalid");
        let agenda = Agenda::new(&store, &classifier);

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        app.set_view_selection_by_name("Board");
        app.refresh(&store).expect("refresh board");
        app.column_index = 1;

        // Open editor
        app.handle_key(KeyCode::Enter, &agenda)
            .expect("open numeric editor");

        // Type invalid input
        for ch in "abc".chars() {
            app.handle_key(KeyCode::Char(ch), &agenda)
                .expect("type invalid");
        }

        // Enter in value field attempts save and should fail validation.
        app.handle_key(KeyCode::Enter, &agenda)
            .expect("attempt save");

        // Panel should still be open
        assert_eq!(app.mode, Mode::InputPanel);
        assert!(
            app.status.contains("Invalid number"),
            "should show validation error, got: {}",
            app.status
        );

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn numeric_column_edit_prefills_existing_value() {
        let (store, classifier, cost_id, item_id, db_path) = setup_numeric_column_board("prefill");
        let agenda = Agenda::new(&store, &classifier);

        // Set an initial value
        agenda
            .assign_item_numeric_manual(
                item_id,
                cost_id,
                rust_decimal::Decimal::new(5000, 2),
                Some("test".to_string()),
            )
            .expect("set initial value");

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        app.set_view_selection_by_name("Board");
        app.refresh(&store).expect("refresh board");
        app.column_index = 1;

        // Open editor
        app.handle_key(KeyCode::Enter, &agenda)
            .expect("open numeric editor");

        assert_eq!(app.mode, Mode::InputPanel);
        let panel_text = app
            .input_panel
            .as_ref()
            .map(|p| p.text.trimmed().to_string())
            .unwrap_or_default();
        assert_eq!(panel_text, "50.00", "should prefill existing value");

        let _ = std::fs::remove_file(&db_path);
    }

    // --- Edit panel numeric values tests ---

    /// Helper: create a store with a numeric "Cost" category, assign an item to it
    /// with a numeric value, and return the pieces needed for edit-panel tests.
    fn setup_edit_panel_numeric(
        suffix: &str,
    ) -> (
        Store,
        SubstringClassifier,
        CategoryId,
        ItemId,
        std::path::PathBuf,
    ) {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!(
            "agenda-tui-editnum-{suffix}-{nanos}-{}.ag",
            std::process::id()
        ));
        let store = Store::open(&db_path).expect("open temp db");
        let classifier = SubstringClassifier;

        let mut cost = Category::new("Cost".to_string());
        cost.value_kind = CategoryValueKind::Numeric;
        store.create_category(&cost).expect("create Cost");

        let item = Item::new("Test item".to_string());
        store.create_item(&item).expect("create item");

        // Assign item to cost with a numeric value
        let agenda = Agenda::new(&store, &classifier);
        agenda
            .assign_item_numeric_manual(
                item.id,
                cost.id,
                rust_decimal::Decimal::new(4200, 2),
                Some("test:setup".to_string()),
            )
            .expect("assign numeric");

        let mut view = View::new("Board".to_string());
        view.sections.push(Section {
            title: "Main".to_string(),
            criteria: Query::default(),
            columns: vec![Column {
                kind: ColumnKind::Standard,
                heading: cost.id,
                width: 12,
            }],
            item_column_index: 0,
            on_insert_assign: std::collections::HashSet::new(),
            on_remove_unassign: std::collections::HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        });
        store.create_view(&view).expect("create view");

        (store, classifier, cost.id, item.id, db_path)
    }

    #[test]
    fn edit_panel_shows_numeric_buffers_for_assigned_numeric_categories() {
        let (store, classifier, cost_id, _item_id, db_path) = setup_edit_panel_numeric("shows");
        let agenda = Agenda::new(&store, &classifier);

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        app.set_view_selection_by_name("Board");
        app.refresh(&store).expect("refresh board");

        // Press 'e' to open edit panel
        app.handle_key(KeyCode::Char('e'), &agenda)
            .expect("open edit panel");
        assert_eq!(app.mode, Mode::InputPanel);

        let panel = app.input_panel.as_ref().expect("panel should exist");
        assert!(panel.numeric_buffers.contains_key(&cost_id));
        assert_eq!(panel.numeric_buffers.get(&cost_id).unwrap().text(), "42");
        assert_eq!(
            panel.numeric_originals.get(&cost_id).copied().flatten(),
            Some(rust_decimal::Decimal::new(4200, 2))
        );

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn edit_panel_numeric_value_edit_and_save_persists() {
        let (store, classifier, cost_id, item_id, db_path) = setup_edit_panel_numeric("save");
        let agenda = Agenda::new(&store, &classifier);

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        app.set_view_selection_by_name("Board");
        app.refresh(&store).expect("refresh board");

        app.handle_key(KeyCode::Char('e'), &agenda)
            .expect("open edit panel");

        // Tab to Categories: Text -> Note -> Categories
        app.handle_key(KeyCode::Tab, &agenda).expect("tab");
        app.handle_key(KeyCode::Tab, &agenda).expect("tab");
        assert_eq!(
            app.input_panel.as_ref().unwrap().focus,
            input_panel::InputPanelFocus::Categories
        );

        // Navigate to the Cost category row using j/k
        // Find the index of Cost in category_rows
        let cost_idx = app
            .category_rows
            .iter()
            .position(|r| r.id == cost_id)
            .expect("Cost should be in category_rows");
        // Navigate to it
        for _ in 0..cost_idx {
            app.handle_key(KeyCode::Char('j'), &agenda).expect("j");
        }

        // Clear existing value and type new one
        // The buffer has "42", clear it
        app.handle_key(KeyCode::Backspace, &agenda).expect("bs");
        app.handle_key(KeyCode::Backspace, &agenda).expect("bs");
        for ch in "99.50".chars() {
            app.handle_key(KeyCode::Char(ch), &agenda).expect("type");
        }

        // Save with Tab to SaveButton then Enter
        app.handle_key(KeyCode::Tab, &agenda).expect("tab to save");
        assert_eq!(
            app.input_panel.as_ref().unwrap().focus,
            input_panel::InputPanelFocus::SaveButton
        );
        app.handle_key(KeyCode::Enter, &agenda).expect("save");

        assert_eq!(app.mode, Mode::Normal);
        assert!(app.status.contains("updated"), "status: {}", app.status);

        // Verify persisted
        let assignments = store
            .get_assignments_for_item(item_id)
            .expect("assignments");
        let value = assignments
            .get(&cost_id)
            .and_then(|a| a.numeric_value)
            .expect("should have numeric value");
        assert_eq!(value, rust_decimal::Decimal::new(9950, 2));

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn edit_panel_invalid_numeric_shows_error_keeps_panel_open() {
        let (store, classifier, cost_id, _item_id, db_path) = setup_edit_panel_numeric("invalid");
        let agenda = Agenda::new(&store, &classifier);

        let mut app = App::default();
        app.refresh(&store).expect("refresh");
        app.set_view_selection_by_name("Board");
        app.refresh(&store).expect("refresh board");

        app.handle_key(KeyCode::Char('e'), &agenda)
            .expect("open edit panel");

        // Tab to Categories: Text -> Note -> Categories
        app.handle_key(KeyCode::Tab, &agenda).expect("tab");
        app.handle_key(KeyCode::Tab, &agenda).expect("tab");

        // Navigate to the Cost category row
        let cost_idx = app
            .category_rows
            .iter()
            .position(|r| r.id == cost_id)
            .expect("Cost should be in category_rows");
        for _ in 0..cost_idx {
            app.handle_key(KeyCode::Char('j'), &agenda).expect("j");
        }

        // Clear and type invalid
        app.handle_key(KeyCode::Backspace, &agenda).expect("bs");
        app.handle_key(KeyCode::Backspace, &agenda).expect("bs");
        for ch in "abc".chars() {
            app.handle_key(KeyCode::Char(ch), &agenda).expect("type");
        }

        // Tab to save button and press Enter
        app.handle_key(KeyCode::Tab, &agenda).expect("tab to save");
        app.handle_key(KeyCode::Enter, &agenda)
            .expect("attempt save");

        // Panel should still be open with error
        assert_eq!(app.mode, Mode::InputPanel);
        assert!(
            app.status.contains("Invalid numeric value"),
            "should show error, got: {}",
            app.status
        );

        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn edit_panel_focus_cycle_categories_always_present() {
        let store = Store::open_memory().expect("memory store");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        // Create a non-numeric category
        let status = Category::new("Status".to_string());
        store.create_category(&status).expect("create Status");

        let item = Item::new("Test item".to_string());
        store.create_item(&item).expect("create item");

        let mut view = View::new("Board".to_string());
        view.sections.push(Section {
            title: "Main".to_string(),
            criteria: Query::default(),
            columns: Vec::new(),
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

        app.handle_key(KeyCode::Char('e'), &agenda)
            .expect("open edit panel");
        assert_eq!(app.mode, Mode::InputPanel);

        let panel = app.input_panel.as_ref().unwrap();
        assert!(panel.numeric_buffers.is_empty());

        // Tab cycle: Text -> Note -> Categories -> Save
        app.handle_key(KeyCode::Tab, &agenda).expect("tab");
        assert_eq!(
            app.input_panel.as_ref().unwrap().focus,
            input_panel::InputPanelFocus::Note
        );
        app.handle_key(KeyCode::Tab, &agenda).expect("tab");
        assert_eq!(
            app.input_panel.as_ref().unwrap().focus,
            input_panel::InputPanelFocus::Categories
        );
        app.handle_key(KeyCode::Tab, &agenda).expect("tab");
        assert_eq!(
            app.input_panel.as_ref().unwrap().focus,
            input_panel::InputPanelFocus::SaveButton
        );
    }

    #[test]
    fn is_item_blocked_returns_true_when_dependency_undone() {
        let store = Store::open_memory().expect("memory store");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut a = Item::new("Blocker".to_string());
        let b = Item::new("Blocked".to_string());
        store.create_item(&a).expect("create a");
        store.create_item(&b).expect("create b");
        agenda
            .link_items_depends_on(b.id, a.id)
            .expect("link depends_on");

        let mut app = App::default();
        app.refresh(&store).expect("refresh");

        // b depends on a (not done) → b is blocked
        assert!(app.is_item_blocked(b.id));
        // a has no deps → not blocked
        assert!(!app.is_item_blocked(a.id));

        // Mark a as done → b should no longer be blocked
        a.is_done = true;
        store.update_item(&a).expect("update a");
        app.refresh(&store).expect("refresh");
        assert!(!app.is_item_blocked(b.id));
    }
}
