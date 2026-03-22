use crate::*;

pub(crate) fn continuous_mode_index(mode: ContinuousMode) -> usize {
    match mode {
        ContinuousMode::Off => 0,
        ContinuousMode::AutoApply => 1,
        ContinuousMode::SuggestReview => 2,
    }
}

pub(crate) fn continuous_mode_from_index(index: usize) -> ContinuousMode {
    match index {
        0 => ContinuousMode::Off,
        2 => ContinuousMode::SuggestReview,
        _ => ContinuousMode::AutoApply,
    }
}

pub(crate) fn continuous_mode_label(mode: ContinuousMode) -> &'static str {
    match mode {
        ContinuousMode::Off => "Off",
        ContinuousMode::AutoApply => "Auto-apply",
        ContinuousMode::SuggestReview => "Suggest/Review",
    }
}
