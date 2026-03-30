use super::super::*;

#[derive(Clone, Debug, Default)]
pub(crate) struct ClassificationReviewItem {
    pub(crate) item_id: ItemId,
    pub(crate) item_text: String,
    pub(crate) note_excerpt: Option<String>,
    pub(crate) current_assignments: Vec<String>,
    pub(crate) suggestions: Vec<ClassificationSuggestion>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct ClassificationUiState {
    pub(crate) pending_count: usize,
    pub(crate) config: ClassificationConfig,
    pub(crate) review_items: Vec<ClassificationReviewItem>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum SuggestionReviewFocus {
    Items,
    Suggestions,
}

#[derive(Clone, Debug)]
#[allow(dead_code)] // item_id kept for future use (e.g. refresh after confirm)
pub(crate) struct SuggestionReviewItem {
    pub(crate) item_id: ItemId,
    pub(crate) item_text: String,
    pub(crate) note_excerpt: Option<String>,
    pub(crate) current_assignments: Vec<String>,
    pub(crate) suggestions: Vec<ReviewSuggestion>,
}

#[derive(Clone, Debug)]
pub(crate) struct SuggestionReviewState {
    pub(crate) items: Vec<SuggestionReviewItem>,
    pub(crate) item_index: usize,
    pub(crate) suggestion_cursor: usize,
    pub(crate) focus: SuggestionReviewFocus,
    pub(crate) resolved_count: usize,
    pub(crate) resolved_items: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct ReviewSuggestion {
    pub(crate) suggestion: ClassificationSuggestion,
    pub(crate) accepted: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum SuggestionDecision {
    Pending,
    Accept,
    Reject,
}

impl SuggestionDecision {
    pub(crate) fn next(self) -> Self {
        match self {
            Self::Pending => Self::Accept,
            Self::Accept => Self::Reject,
            Self::Reject => Self::Pending,
        }
    }

    pub(crate) fn marker(self) -> &'static str {
        match self {
            Self::Pending => "[?]",
            Self::Accept => "[x]",
            Self::Reject => "[ ]",
        }
    }
}
