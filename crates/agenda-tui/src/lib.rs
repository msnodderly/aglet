use std::cell::{Cell as ScrollCell, RefCell};
use std::collections::{HashMap, HashSet};
use std::io;
use std::path::Path;
use std::time::{Duration, Instant};

use agenda_core::agenda::Agenda;
use agenda_core::classification::{
    CandidateAssignment, ClassificationConfig, ClassificationSuggestion, LiteralClassificationMode,
    SemanticClassificationMode,
};
use agenda_core::matcher::{unknown_hashtag_tokens, SubstringClassifier};
use agenda_core::model::{
    Action, Assignment, AssignmentExplanation, BoardDisplayMode, Category, CategoryId,
    CategoryValueKind, Column, ColumnKind, Condition, CriterionMode, Item, ItemId,
    ItemLinksForItem, NumericFormat, Query, Section, SectionFlow, SummaryFn, View, WhenBucket,
};
use agenda_core::query::{evaluate_query, resolve_view};
use agenda_core::store::Store;
use agenda_core::workflow::WorkflowConfig;
use crossterm::cursor::SetCursorStyle;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use jiff::civil::{Date, DateTime};
use jiff::Timestamp;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Cell, Clear, List, ListItem, ListState, Padding, Paragraph, Row,
    Scrollbar, ScrollbarOrientation, ScrollbarState, Table, TableState, Wrap,
};
use ratatui::Terminal;
use uuid::Uuid;

mod app;
mod async_classify;
mod error;
mod input;
mod input_panel;
mod modes;
mod projection;
mod render;
mod state;
mod text_buffer;
mod ui_support;
mod undo;

pub use error::{TuiError, TuiResult};

use modes::view_edit::{
    BucketEditTarget, CategoryEditTarget, ViewEditInlineInput, ViewEditOverlay, ViewEditPaneFocus,
    ViewEditRegion, ViewEditState,
};
use state::assign::{
    AssignmentPreview, InspectAssignmentRow, ItemAssignPane, ItemAssignReturnTarget, ViewAssignRow,
};
use state::board::{
    AddColumnDirection, BoardAddColumnAnchor, BoardAddColumnState, DoneBlocksConfirmScope,
    DoneBlocksConfirmState, DoneToggleOrigin, GlobalSearchSession, LinkWizardAction,
    LinkWizardFocus, LinkWizardState, NameInputContext, NormalFocus, NormalModePrefix,
    NumericEditTarget, PreviewMode, Slot, SlotContext, SlotSortColumn, SlotSortDirection,
    SlotSortKey, WhenEditTarget,
};
use state::category::{
    ActionEditKind, ActionEditState, CategoryColumnPickerFocus, CategoryColumnPickerState,
    CategoryDirectEditAnchor, CategoryDirectEditColumnMeta, CategoryDirectEditFocus,
    CategoryDirectEditRow, CategoryDirectEditState, CategoryInlineAction, CategoryListRow,
    CategoryManagerDetailsFocus, CategoryManagerDetailsInlineField,
    CategoryManagerDetailsInlineInput, CategoryManagerFocus, CategoryManagerState,
    CategorySuggestState, ConditionEditState, GlobalSettingsRow, GlobalSettingsState,
    OllamaModelPickerState, WorkflowRolePickerOrigin, WorkflowRolePickerState,
};
use state::classification::{
    ClassificationReviewItem, ClassificationUiState, ReviewSuggestion, SuggestionDecision,
    SuggestionReviewFocus, SuggestionReviewItem, SuggestionReviewState,
};
use ui_support::*;
use undo::{UndoEntry, UndoState};

type TuiTerminal = Terminal<CrosstermBackend<io::Stdout>>;

struct TerminalSession {
    terminal: TuiTerminal,
    active: bool,
}

impl TerminalSession {
    fn try_apply_preferred_cursor_style<W: io::Write>(writer: &mut W) {
        // Prefer a tall blinking bar; fall back to steady bar when blink is unsupported.
        if execute!(writer, SetCursorStyle::BlinkingBar).is_err() {
            let _ = execute!(writer, SetCursorStyle::SteadyBar);
        }
    }

    fn enter() -> TuiResult<Self> {
        enable_raw_mode()?;

        let mut stdout = io::stdout();
        if let Err(err) = execute!(stdout, EnterAlternateScreen) {
            let _ = disable_raw_mode();
            return Err(err.into());
        }
        Self::try_apply_preferred_cursor_style(&mut stdout);

        let backend = CrosstermBackend::new(stdout);
        let terminal = match Terminal::new(backend) {
            Ok(terminal) => terminal,
            Err(err) => {
                let _ = execute!(io::stdout(), LeaveAlternateScreen);
                let _ = disable_raw_mode();
                return Err(err.into());
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

    fn exit(&mut self) -> TuiResult<()> {
        if !self.active {
            return Ok(());
        }
        let _ = execute!(
            self.terminal.backend_mut(),
            SetCursorStyle::DefaultUserShape
        );
        disable_raw_mode()?;
        execute!(self.terminal.backend_mut(), LeaveAlternateScreen)?;
        self.terminal.show_cursor()?;
        self.active = false;
        Ok(())
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        let _ = execute!(
            self.terminal.backend_mut(),
            SetCursorStyle::DefaultUserShape
        );
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}

pub fn run(db_path: &Path) -> TuiResult<()> {
    run_with_options(db_path, false)
}

pub fn run_with_options(db_path: &Path, debug: bool) -> TuiResult<()> {
    let store = Store::open(db_path)?;
    let classifier = SubstringClassifier;
    let agenda = Agenda::with_debug(&store, &classifier, debug);

    let mut terminal = TerminalSession::enter()?;

    let mut app = App::default();
    let result = app.run(terminal.terminal_mut(), &agenda);

    terminal.exit()?;

    result
}

#[derive(Clone, PartialEq, Eq, Debug)]
enum Mode {
    Normal,
    GlobalSettings,
    HelpPanel,
    SuggestionReview,
    InputPanel, // unified add/edit/name-input (replaces AddInput + ItemEdit)
    LinkWizard,
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
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum AutoRefreshInterval {
    Off,
    OneSecond,
    FiveSeconds,
}

impl AutoRefreshInterval {
    fn next(self) -> Self {
        match self {
            Self::Off => Self::OneSecond,
            Self::OneSecond => Self::FiveSeconds,
            Self::FiveSeconds => Self::Off,
        }
    }

    fn prev(self) -> Self {
        match self {
            Self::Off => Self::FiveSeconds,
            Self::OneSecond => Self::Off,
            Self::FiveSeconds => Self::OneSecond,
        }
    }

    fn as_duration(self) -> Option<Duration> {
        match self {
            Self::Off => None,
            Self::OneSecond => Some(Duration::from_secs(1)),
            Self::FiveSeconds => Some(Duration::from_secs(5)),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::OneSecond => "1s",
            Self::FiveSeconds => "5s",
        }
    }

    fn persisted_value(self) -> &'static str {
        self.label()
    }

    fn from_persisted_value(value: &str) -> Option<Self> {
        match value {
            "off" => Some(Self::Off),
            "1s" => Some(Self::OneSecond),
            "5s" => Some(Self::FiveSeconds),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum SectionBorderMode {
    Full,
    Compact,
}

impl SectionBorderMode {
    fn next(self) -> Self {
        match self {
            Self::Full => Self::Compact,
            Self::Compact => Self::Full,
        }
    }

    fn prev(self) -> Self {
        self.next()
    }

    fn label(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Compact => "compact",
        }
    }

    fn persisted_value(self) -> &'static str {
        self.label()
    }

    fn from_persisted_value(value: &str) -> Option<Self> {
        match value {
            "full" => Some(Self::Full),
            "compact" => Some(Self::Compact),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
struct TransientStatus {
    message: String,
    expires_at: Instant,
}

/// Which InputPanel buffer to open in the external editor.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ExternalEditorTarget {
    Text,
    Note,
}

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

struct SettingsUiState {
    workflow_role_picker: Option<WorkflowRolePickerState>,
    ollama_model_picker: Option<OllamaModelPickerState>,
    classification_mode_picker_open: bool,
    classification_mode_picker_focus: usize,
    global_settings: Option<GlobalSettingsState>,
}

impl Default for SettingsUiState {
    fn default() -> Self {
        Self {
            workflow_role_picker: None,
            ollama_model_picker: None,
            classification_mode_picker_open: false,
            classification_mode_picker_focus: 1,
            global_settings: None,
        }
    }
}

struct ClassificationAppState {
    ui: ClassificationUiState,
    suggestion_review: Option<SuggestionReviewState>,
    worker: async_classify::ClassificationWorker,
    in_flight_classifications: HashSet<ItemId>,
}

impl Default for ClassificationAppState {
    fn default() -> Self {
        Self {
            ui: ClassificationUiState::default(),
            suggestion_review: None,
            worker: async_classify::ClassificationWorker::spawn(),
            in_flight_classifications: HashSet::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BackgroundClassificationSubmitResult {
    Submitted,
    AlreadyInFlight,
    NoProvidersEnabled,
}

struct TransientUiState {
    status: Option<TransientStatus>,
    key_modifiers: KeyModifiers,
    pending_external_edit: Option<ExternalEditorTarget>,
}

impl Default for TransientUiState {
    fn default() -> Self {
        Self {
            status: None,
            key_modifiers: KeyModifiers::NONE,
            pending_external_edit: None,
        }
    }
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
    blocked_item_ids: HashSet<ItemId>,

    views: Vec<View>,
    view_index: usize,
    active_view_name: Option<String>,
    session_hide_dependent_items_override: Option<bool>,
    picker_index: usize,
    view_pending_edit_name: Option<String>,
    view_pending_clone_id: Option<Uuid>,
    view_edit_state: Option<ViewEditState>,
    numeric_edit_target: Option<NumericEditTarget>,
    when_edit_target: Option<WhenEditTarget>,

    categories: Vec<Category>,
    category_rows: Vec<CategoryListRow>,
    category_index: usize,
    workflow_config: WorkflowConfig,
    workflow_setup_open: bool,
    workflow_setup_focus: usize,
    settings: SettingsUiState,
    category_manager: Option<CategoryManagerState>,
    category_suggest: Option<CategorySuggestState>,
    category_direct_edit: Option<CategoryDirectEditState>,
    category_direct_edit_create_confirm: Option<String>,
    category_column_picker: Option<CategoryColumnPickerState>,
    board_add_column: Option<BoardAddColumnState>,
    item_assign_category_index: usize,
    item_assign_dirty: bool,
    item_assign_anchor_item_id: Option<ItemId>,
    item_assign_target_item_ids: Vec<ItemId>,
    item_assign_pane: ItemAssignPane,
    item_assign_view_row_index: usize,
    view_assign_rows: Vec<ViewAssignRow>,
    item_assign_preview: AssignmentPreview,
    item_assign_return_target: Option<ItemAssignReturnTarget>,
    input_panel: Option<input_panel::InputPanel>,
    link_wizard: Option<LinkWizardState>,
    name_input_context: Option<NameInputContext>,
    preview_provenance_scroll: usize,
    preview_summary_scroll: usize,
    inspect_assignment_index: usize,
    slots: Vec<Slot>,
    selected_item_ids: HashSet<ItemId>,
    horizontal_slot_item_indices: Vec<usize>,
    horizontal_slot_scroll_offsets: RefCell<Vec<usize>>,
    slot_index: usize,
    item_index: usize,
    column_index: usize,
    normal_mode_prefix: Option<NormalModePrefix>,
    global_search_session: Option<GlobalSearchSession>,
    done_blocks_confirm: Option<DoneBlocksConfirmState>,
    batch_delete_item_ids: Option<Vec<ItemId>>,
    board_pending_delete_column_label: Option<String>,
    auto_refresh_interval: AutoRefreshInterval,
    section_border_mode: SectionBorderMode,
    auto_refresh_last_tick: Instant,
    transient: TransientUiState,
    category_assignment_counts: HashMap<CategoryId, usize>,
    classification: ClassificationAppState,
    undo: UndoState,
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
            blocked_item_ids: HashSet::new(),
            views: Vec::new(),
            view_index: 0,
            active_view_name: None,
            session_hide_dependent_items_override: None,
            picker_index: 0,
            view_pending_edit_name: None,
            view_pending_clone_id: None,
            view_edit_state: None,
            numeric_edit_target: None,
            when_edit_target: None,
            categories: Vec::new(),
            category_rows: Vec::new(),
            category_index: 0,
            workflow_config: WorkflowConfig::default(),
            workflow_setup_open: false,
            workflow_setup_focus: 0,
            settings: SettingsUiState::default(),
            category_manager: None,
            category_suggest: None,
            category_direct_edit: None,
            category_direct_edit_create_confirm: None,
            category_column_picker: None,
            board_add_column: None,
            item_assign_category_index: 0,
            item_assign_dirty: false,
            item_assign_anchor_item_id: None,
            item_assign_target_item_ids: Vec::new(),
            item_assign_pane: ItemAssignPane::Categories,
            item_assign_view_row_index: 0,
            view_assign_rows: Vec::new(),
            item_assign_preview: AssignmentPreview::default(),
            item_assign_return_target: None,
            input_panel: None,
            link_wizard: None,
            name_input_context: None,
            preview_provenance_scroll: 0,
            preview_summary_scroll: 0,
            inspect_assignment_index: 0,
            slots: Vec::new(),
            selected_item_ids: HashSet::new(),
            horizontal_slot_item_indices: Vec::new(),
            horizontal_slot_scroll_offsets: RefCell::new(Vec::new()),
            slot_index: 0,
            item_index: 0,
            column_index: 0,
            normal_mode_prefix: None,
            global_search_session: None,
            done_blocks_confirm: None,
            batch_delete_item_ids: None,
            board_pending_delete_column_label: None,
            auto_refresh_interval: AutoRefreshInterval::Off,
            section_border_mode: SectionBorderMode::Full,
            auto_refresh_last_tick: Instant::now(),
            transient: TransientUiState::default(),
            category_assignment_counts: HashMap::new(),
            classification: ClassificationAppState::default(),
            undo: UndoState::default(),
        }
    }
}

#[cfg(test)]
mod tests;
