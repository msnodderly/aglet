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
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ViewEditPaneFocus {
    Sections,
    Details,
    Preview,
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
}
