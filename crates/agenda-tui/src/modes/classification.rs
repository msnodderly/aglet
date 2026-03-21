use crate::*;

impl App {
    pub(crate) fn open_classification_review(&mut self) {
        self.mode = Mode::ClassificationReview;
        self.classification_ui.focus = ClassificationFocus::Items;
        self.status = if self.classification_ui.pending_count > 0 {
            format!(
                "Classification review: {} pending suggestion{}",
                self.classification_ui.pending_count,
                if self.classification_ui.pending_count == 1 {
                    ""
                } else {
                    "s"
                }
            )
        } else {
            "Classification review: no pending suggestions".to_string()
        };
    }

    pub(crate) fn handle_classification_review_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> TuiResult<bool> {
        match code {
            KeyCode::Esc | KeyCode::Char('C') => {
                self.mode = Mode::Normal;
            }
            KeyCode::Char('?') => {
                self.mode = Mode::HelpPanel;
            }
            KeyCode::Tab => self.cycle_classification_focus(1),
            KeyCode::BackTab => self.cycle_classification_focus(-1),
            KeyCode::Up | KeyCode::Char('k') => self.move_classification_selection(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_classification_selection(1),
            KeyCode::Char('r') => {
                if self.classification_ui.focus == ClassificationFocus::Suggestions {
                    self.reject_selected_classification_suggestion(agenda)?;
                }
            }
            KeyCode::Char('A') => self.accept_all_selected_classification_item(agenda)?,
            KeyCode::Char('R') => self.reject_all_selected_classification_item(agenda)?,
            KeyCode::Enter => match self.classification_ui.focus {
                ClassificationFocus::Items => {
                    if self
                        .selected_classification_item()
                        .is_some_and(|item| !item.suggestions.is_empty())
                    {
                        self.classification_ui.focus = ClassificationFocus::Suggestions;
                    }
                }
                ClassificationFocus::Suggestions => {
                    self.accept_selected_classification_suggestion(agenda)?;
                }
            },
            _ => {}
        }
        Ok(false)
    }

    fn cycle_classification_focus(&mut self, delta: i32) {
        let order = [ClassificationFocus::Items, ClassificationFocus::Suggestions];
        let current = order
            .iter()
            .position(|focus| *focus == self.classification_ui.focus)
            .unwrap_or(0);
        let mut next = current;
        for _ in 0..order.len() {
            next = next_index_clamped(next, order.len(), delta);
            let candidate = order[next];
            if candidate != ClassificationFocus::Suggestions
                || self
                    .selected_classification_item()
                    .is_some_and(|item| !item.suggestions.is_empty())
            {
                self.classification_ui.focus = candidate;
                return;
            }
        }
    }

    fn move_classification_selection(&mut self, delta: i32) {
        match self.classification_ui.focus {
            ClassificationFocus::Items => {
                let len = self.classification_ui.review_items.len();
                self.classification_ui.selected_item_index =
                    next_index_clamped(self.classification_ui.selected_item_index, len, delta);
                let suggestion_len = self
                    .selected_classification_item()
                    .map(|item| item.suggestions.len())
                    .unwrap_or(0);
                self.classification_ui.selected_suggestion_index = self
                    .classification_ui
                    .selected_suggestion_index
                    .min(suggestion_len.saturating_sub(1));
            }
            ClassificationFocus::Suggestions => {
                let len = self
                    .selected_classification_item()
                    .map(|item| item.suggestions.len())
                    .unwrap_or(0);
                self.classification_ui.selected_suggestion_index = next_index_clamped(
                    self.classification_ui.selected_suggestion_index,
                    len,
                    delta,
                );
            }
        }
    }

    fn accept_selected_classification_suggestion(&mut self, agenda: &Agenda<'_>) -> TuiResult<()> {
        let Some(review_item) = self.selected_classification_item() else {
            self.status = "No pending suggestions".to_string();
            return Ok(());
        };
        let item_id = review_item.item_id;
        let Some(suggestion) = self.selected_classification_suggestion() else {
            self.status = "No pending suggestion selected".to_string();
            return Ok(());
        };
        let suggestion_id = suggestion.id;
        agenda.accept_classification_suggestion(suggestion_id)?;
        self.refresh(agenda.store())?;
        self.mode = Mode::ClassificationReview;
        self.set_item_selection_by_id(item_id);
        self.status = "Accepted classification suggestion".to_string();
        Ok(())
    }

    fn reject_selected_classification_suggestion(&mut self, agenda: &Agenda<'_>) -> TuiResult<()> {
        let Some(review_item) = self.selected_classification_item() else {
            self.status = "No pending suggestions".to_string();
            return Ok(());
        };
        let item_id = review_item.item_id;
        let Some(suggestion) = self.selected_classification_suggestion() else {
            self.status = "No pending suggestion selected".to_string();
            return Ok(());
        };
        let suggestion_id = suggestion.id;
        agenda.reject_classification_suggestion(suggestion_id)?;
        self.refresh(agenda.store())?;
        self.mode = Mode::ClassificationReview;
        self.set_item_selection_by_id(item_id);
        self.status = "Rejected classification suggestion".to_string();
        Ok(())
    }

    fn accept_all_selected_classification_item(&mut self, agenda: &Agenda<'_>) -> TuiResult<()> {
        let Some(review_item) = self.selected_classification_item() else {
            self.status = "No pending suggestions".to_string();
            return Ok(());
        };
        let item_id = review_item.item_id;
        let suggestion_ids: Vec<_> = review_item.suggestions.iter().map(|s| s.id).collect();
        if suggestion_ids.is_empty() {
            self.status = "No pending suggestions for selected item".to_string();
            return Ok(());
        }
        for suggestion_id in &suggestion_ids {
            agenda.accept_classification_suggestion(*suggestion_id)?;
        }
        self.refresh(agenda.store())?;
        self.mode = Mode::ClassificationReview;
        self.set_item_selection_by_id(item_id);
        self.status = format!(
            "Accepted {} suggestion{} for selected item",
            suggestion_ids.len(),
            if suggestion_ids.len() == 1 { "" } else { "s" }
        );
        Ok(())
    }

    fn reject_all_selected_classification_item(&mut self, agenda: &Agenda<'_>) -> TuiResult<()> {
        let Some(review_item) = self.selected_classification_item() else {
            self.status = "No pending suggestions".to_string();
            return Ok(());
        };
        let item_id = review_item.item_id;
        let suggestion_ids: Vec<_> = review_item.suggestions.iter().map(|s| s.id).collect();
        if suggestion_ids.is_empty() {
            self.status = "No pending suggestions for selected item".to_string();
            return Ok(());
        }
        for suggestion_id in &suggestion_ids {
            agenda.reject_classification_suggestion(*suggestion_id)?;
        }
        self.refresh(agenda.store())?;
        self.mode = Mode::ClassificationReview;
        self.set_item_selection_by_id(item_id);
        self.status = format!(
            "Rejected {} suggestion{} for selected item",
            suggestion_ids.len(),
            if suggestion_ids.len() == 1 { "" } else { "s" }
        );
        Ok(())
    }
}

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
