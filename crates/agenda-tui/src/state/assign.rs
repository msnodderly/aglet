use super::super::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) enum ItemAssignPane {
    #[default]
    Categories,
    ViewSection,
}

/// A flattened, navigable row in the View/Section pane of the assign panel.
#[derive(Clone)]
pub(crate) enum ViewAssignRow {
    /// Non-navigable view heading — skipped during j/k navigation.
    #[allow(dead_code)]
    ViewHeader { view_idx: usize, name: String },
    /// Navigable section row; `section_idx` is `None` for the "unmatched" slot.
    SectionRow {
        view_idx: usize,
        section_idx: Option<usize>,
        label: String,
    },
}

/// Ephemeral preview computed whenever the assign-panel cursor moves.
/// Cleared whenever the cursor leaves a navigable row or the pane loses focus.
#[derive(Clone, Default)]
pub(crate) struct AssignmentPreview {
    /// Categories that would be assigned (shown as `[+]` in the category pane).
    pub(crate) cat_to_add: HashSet<CategoryId>,
    /// Categories that would be removed (shown as `[-]` in the category pane).
    pub(crate) cat_to_remove: HashSet<CategoryId>,
    /// View/section slots that the item would gain (shown as `[+]` in the view pane).
    /// Encoded as `(view_idx, section_idx)` where `None` means the unmatched slot.
    pub(crate) section_to_gain: HashSet<(usize, Option<usize>)>,
    /// View/section slots that the item would lose (shown as `[-]` in the view pane).
    pub(crate) section_to_lose: HashSet<(usize, Option<usize>)>,
}

#[derive(Clone)]
pub(crate) struct InspectAssignmentRow {
    pub(crate) category_id: CategoryId,
    pub(crate) category_name: String,
    pub(crate) source_label: String,
    pub(crate) origin_label: String,
    pub(crate) explanation_label: Option<String>,
}

#[derive(Clone)]
pub(crate) enum ItemAssignReturnTarget {
    EditPanel(input_panel::InputPanel),
}
