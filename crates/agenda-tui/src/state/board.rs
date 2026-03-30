use super::super::*;

#[derive(Clone)]
pub(crate) enum SlotContext {
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
pub(crate) struct Slot {
    pub(crate) title: String,
    pub(crate) items: Vec<Item>,
    pub(crate) context: SlotContext,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum LinkWizardFocus {
    ScopeAction,
    Target,
    Confirm,
}

impl LinkWizardFocus {
    pub(crate) fn next(self) -> Self {
        match self {
            Self::ScopeAction => Self::Target,
            Self::Target => Self::Confirm,
            Self::Confirm => Self::ScopeAction,
        }
    }

    pub(crate) fn prev(self) -> Self {
        match self {
            Self::ScopeAction => Self::Confirm,
            Self::Target => Self::ScopeAction,
            Self::Confirm => Self::Target,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum LinkWizardAction {
    BlockedBy,
    DependsOn,
    Blocks,
    RelatedTo,
    ClearDependencies,
}

impl LinkWizardAction {
    pub(crate) const ALL: [Self; 5] = [
        Self::BlockedBy,
        Self::DependsOn,
        Self::Blocks,
        Self::RelatedTo,
        Self::ClearDependencies,
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::BlockedBy => "blocked by",
            Self::DependsOn => "depends on",
            Self::Blocks => "blocks",
            Self::RelatedTo => "related to",
            Self::ClearDependencies => "clear dependencies",
        }
    }

    pub(crate) fn description(self) -> &'static str {
        match self {
            Self::BlockedBy => "(target blocks source item(s))",
            Self::DependsOn => "(source item(s) depend on target)",
            Self::Blocks => "(source item(s) block target)",
            Self::RelatedTo => "(source item(s) relate to target)",
            Self::ClearDependencies => "(remove depends-on/blocks links for source item(s))",
        }
    }

    pub(crate) fn requires_target(self) -> bool {
        !matches!(self, Self::ClearDependencies)
    }

    pub(crate) fn from_index(index: usize) -> Self {
        Self::ALL.get(index).copied().unwrap_or(Self::BlockedBy)
    }

    pub(crate) fn index(self) -> usize {
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
pub(crate) struct LinkWizardState {
    pub(crate) anchor_item_id: ItemId,
    pub(crate) source_item_ids: Vec<ItemId>,
    pub(crate) focus: LinkWizardFocus,
    pub(crate) action_index: usize,
    pub(crate) target_filter: text_buffer::TextBuffer,
    pub(crate) target_index: usize,
}

/// Disambiguates which name/value operation is in flight when Mode::InputPanel
/// is open.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum NameInputContext {
    ViewRename,
    ViewClone,
    NumericValueEdit,
    WhenDateEdit,
    CategoryCreate,
    OllamaBaseUrl,
    OllamaModel,
    OllamaTimeout,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct NumericEditTarget {
    pub(crate) item_id: ItemId,
    pub(crate) category_id: CategoryId,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct WhenEditTarget {
    pub(crate) item_id: ItemId,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum AddColumnDirection {
    #[allow(dead_code)]
    Left,
    Right,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum NormalModePrefix {
    G,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum DoneToggleOrigin {
    NormalMode,
    ItemAssignPicker,
}

#[derive(Clone, Debug)]
pub(crate) enum DoneBlocksConfirmScope {
    Single {
        item_id: ItemId,
        blocked_item_ids: Vec<ItemId>,
    },
    Batch {
        item_ids: Vec<ItemId>,
        blocking_item_count: usize,
        blocked_link_count: usize,
    },
}

#[derive(Clone, Debug)]
pub(crate) struct DoneBlocksConfirmState {
    pub(crate) scope: DoneBlocksConfirmScope,
    pub(crate) origin: DoneToggleOrigin,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct BoardAddColumnAnchor {
    pub(crate) slot_index: usize,
    pub(crate) section_index: usize,
    pub(crate) current_board_column_index: usize,
    pub(crate) current_section_column_index: usize,
    pub(crate) item_column_index_before: usize,
    pub(crate) insert_index: usize,
    pub(crate) direction: AddColumnDirection,
    pub(crate) is_generated_section: bool,
}

#[derive(Clone)]
pub(crate) struct BoardAddColumnState {
    pub(crate) anchor: BoardAddColumnAnchor,
    pub(crate) input: text_buffer::TextBuffer,
    pub(crate) suggest_index: usize,
    pub(crate) create_confirm_name: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum PreviewMode {
    Summary,
    Provenance,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum NormalFocus {
    Board,
    Preview,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum SlotSortDirection {
    Asc,
    Desc,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum SlotSortColumn {
    ItemText,
    SectionColumn {
        heading: CategoryId,
        kind: ColumnKind,
    },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct SlotSortKey {
    pub(crate) column: SlotSortColumn,
    pub(crate) direction: SlotSortDirection,
}

#[derive(Clone, Debug)]
pub(crate) struct GlobalSearchSession {
    pub(crate) return_view_name: Option<String>,
    pub(crate) return_slot_index: usize,
    pub(crate) return_item_index: usize,
    pub(crate) return_column_index: usize,
    pub(crate) return_section_filters: Vec<Option<String>>,
    pub(crate) return_slot_sort_keys: Vec<Vec<SlotSortKey>>,
    pub(crate) return_search_text: String,
}
