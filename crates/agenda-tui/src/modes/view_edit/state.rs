use crate::{text_buffer, CategoryId, View};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum CategoryEditTarget {
    ViewCriteria,
    ViewAliases,
    SectionCriteria,
    SectionColumns,
    SectionOnInsertAssign,
    SectionOnRemoveUnassign,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum BucketEditTarget {
    ViewVirtualInclude,
    ViewVirtualExclude,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ViewEditRegion {
    Criteria,
    Sections,
    Unmatched,
    Datebook,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ViewEditPaneFocus {
    Sections,
    Details,
    Preview,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ViewEditTab {
    Scope,
    Sections,
    Appearance,
}

impl ViewEditTab {
    pub(crate) fn previous(self) -> Self {
        match self {
            Self::Scope => Self::Appearance,
            Self::Sections => Self::Scope,
            Self::Appearance => Self::Sections,
        }
    }

    pub(crate) fn next(self) -> Self {
        match self {
            Self::Scope => Self::Sections,
            Self::Sections => Self::Appearance,
            Self::Appearance => Self::Scope,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Scope => "Scope",
            Self::Sections => "Sections",
            Self::Appearance => "Appearance",
        }
    }

    pub(crate) fn number(self) -> char {
        match self {
            Self::Scope => '1',
            Self::Sections => '2',
            Self::Appearance => '3',
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum DatebookField {
    Period,
    Interval,
    Anchor,
    DateSource,
}

impl DatebookField {
    pub(crate) fn index(self) -> usize {
        match self {
            Self::Period => 0,
            Self::Interval => 1,
            Self::Anchor => 2,
            Self::DateSource => 3,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ScopeRow {
    Name,
    ViewType,
    Criterion(usize),
    Datebook(DatebookField),
    DateInclude,
    DateExclude,
    HideDependent,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum AppearanceRow {
    DisplayMode,
    SectionFlow,
    EmptySections,
    Aliases,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum SectionDetailsRow {
    Title,
    Filter,
    Columns,
    DisplayMode,
    OnInsertAssign,
    OnRemoveUnassign,
    ShowChildren,
}

impl SectionDetailsRow {
    pub(crate) fn from_index(idx: usize) -> Self {
        match idx {
            0 => Self::Title,
            1 => Self::Filter,
            2 => Self::Columns,
            3 => Self::DisplayMode,
            4 => Self::OnInsertAssign,
            5 => Self::OnRemoveUnassign,
            _ => Self::ShowChildren,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) enum ViewEditOverlay {
    CategoryPicker { target: CategoryEditTarget },
    BucketPicker { target: BucketEditTarget },
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) enum ViewEditInlineInput {
    ViewName,
    SectionsFilter,
    CategoryAlias { category_id: CategoryId },
    SectionTitle { section_index: usize, is_new: bool },
    UnmatchedLabel,
}

#[derive(Clone)]
pub(crate) struct ViewEditState {
    pub(crate) draft: View,
    pub(crate) is_new_view: bool,
    pub(crate) active_tab: ViewEditTab,
    pub(crate) scope_row: ScopeRow,
    pub(crate) appearance_row: AppearanceRow,
    pub(crate) region: ViewEditRegion,
    pub(crate) pane_focus: ViewEditPaneFocus,
    pub(crate) criteria_index: usize,
    pub(crate) unmatched_field_index: usize,
    pub(crate) section_index: usize,
    pub(crate) sections_view_row_selected: bool,
    pub(crate) section_details_field_index: usize,
    pub(crate) overlay: Option<ViewEditOverlay>,
    pub(crate) inline_input: Option<ViewEditInlineInput>,
    pub(crate) inline_buf: text_buffer::TextBuffer,
    pub(crate) picker_index: usize,
    pub(crate) overlay_filter_buf: text_buffer::TextBuffer,
    pub(crate) preview_count: usize,
    pub(crate) preview_visible: bool,
    pub(crate) preview_scroll: usize,
    pub(crate) sections_filter_buf: text_buffer::TextBuffer,
    pub(crate) dirty: bool,
    pub(crate) discard_confirm: bool,
    pub(crate) section_delete_confirm: Option<usize>,
    pub(crate) datebook_field_index: usize,
    pub(crate) name_focused: bool,
}
