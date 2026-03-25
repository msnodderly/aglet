use crate::*;

pub(crate) fn literal_mode_index(mode: LiteralClassificationMode) -> usize {
    match mode {
        LiteralClassificationMode::Off => 0,
        LiteralClassificationMode::AutoApply => 1,
        LiteralClassificationMode::SuggestReview => 2,
    }
}

pub(crate) fn literal_mode_from_index(index: usize) -> LiteralClassificationMode {
    match index {
        0 => LiteralClassificationMode::Off,
        2 => LiteralClassificationMode::SuggestReview,
        _ => LiteralClassificationMode::AutoApply,
    }
}

pub(crate) fn literal_mode_label(mode: LiteralClassificationMode) -> &'static str {
    match mode {
        LiteralClassificationMode::Off => "Off",
        LiteralClassificationMode::AutoApply => "Auto-apply",
        LiteralClassificationMode::SuggestReview => "Suggest/Review",
    }
}

pub(crate) fn semantic_mode_index(mode: SemanticClassificationMode) -> usize {
    match mode {
        SemanticClassificationMode::Off => 0,
        SemanticClassificationMode::SuggestReview => 1,
    }
}

pub(crate) fn semantic_mode_from_index(index: usize) -> SemanticClassificationMode {
    match index {
        0 => SemanticClassificationMode::Off,
        _ => SemanticClassificationMode::SuggestReview,
    }
}

pub(crate) fn semantic_mode_label(mode: SemanticClassificationMode) -> &'static str {
    match mode {
        SemanticClassificationMode::Off => "Off",
        SemanticClassificationMode::SuggestReview => "Suggest/Review",
    }
}
