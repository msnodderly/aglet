use crate::*;

impl App {
    /// Opens the suggestion review overlay (Shift-C bulk triage).
    /// Builds the full queue of items with pending suggestions.
    pub(crate) fn open_suggestion_review(&mut self, agenda: &Agenda<'_>) -> TuiResult<()> {
        self.refresh(agenda.store())?;

        let items: Vec<SuggestionReviewItem> = self
            .classification
            .ui
            .review_items
            .iter()
            .filter(|item| !item.suggestions.is_empty())
            .map(|item| SuggestionReviewItem {
                item_id: item.item_id,
                item_text: item.item_text.clone(),
                note_excerpt: item.note_excerpt.clone(),
                current_assignments: item.current_assignments.clone(),
                suggestions: item
                    .suggestions
                    .iter()
                    .map(|s| ReviewSuggestion {
                        suggestion: s.clone(),
                        accepted: true, // default to accept in bulk triage
                    })
                    .collect(),
            })
            .collect();

        if items.is_empty() {
            self.status = "No pending classification suggestions".to_string();
            return Ok(());
        }

        self.classification.suggestion_review = Some(SuggestionReviewState {
            items,
            item_index: 0,
            suggestion_cursor: 0,
            focus: SuggestionReviewFocus::Suggestions,
            resolved_count: 0,
            resolved_items: 0,
        });
        self.mode = Mode::SuggestionReview;
        self.status =
            "Review suggestions: Tab switch pane, Space toggle, Enter confirm item, s skip, Esc close"
                .to_string();
        Ok(())
    }

    pub(crate) fn handle_suggestion_review_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> TuiResult<bool> {
        match code {
            KeyCode::Esc => {
                self.classification.suggestion_review = None;
                self.mode = Mode::Normal;
                self.status = "Suggestion review closed".to_string();
            }
            KeyCode::Char('?') => {
                self.mode = Mode::HelpPanel;
            }
            KeyCode::Tab | KeyCode::BackTab => {
                if let Some(state) = &mut self.classification.suggestion_review {
                    state.focus = match state.focus {
                        SuggestionReviewFocus::Items => SuggestionReviewFocus::Suggestions,
                        SuggestionReviewFocus::Suggestions => SuggestionReviewFocus::Items,
                    };
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(state) = &mut self.classification.suggestion_review {
                    match state.focus {
                        SuggestionReviewFocus::Items => {
                            let len = state.items.len();
                            if len > 0 {
                                state.item_index = (state.item_index + 1) % len;
                                state.suggestion_cursor = 0;
                            }
                        }
                        SuggestionReviewFocus::Suggestions => {
                            if let Some(item) = state.items.get(state.item_index) {
                                let len = item.suggestions.len();
                                if len > 0 {
                                    state.suggestion_cursor = (state.suggestion_cursor + 1) % len;
                                }
                            }
                        }
                    }
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(state) = &mut self.classification.suggestion_review {
                    match state.focus {
                        SuggestionReviewFocus::Items => {
                            let len = state.items.len();
                            if len > 0 {
                                state.item_index = (state.item_index + len - 1) % len;
                                state.suggestion_cursor = 0;
                            }
                        }
                        SuggestionReviewFocus::Suggestions => {
                            if let Some(item) = state.items.get(state.item_index) {
                                let len = item.suggestions.len();
                                if len > 0 {
                                    state.suggestion_cursor =
                                        (state.suggestion_cursor + len - 1) % len;
                                }
                            }
                        }
                    }
                }
            }
            KeyCode::Char(' ') => {
                if let Some(state) = &mut self.classification.suggestion_review {
                    if state.focus == SuggestionReviewFocus::Suggestions {
                        if let Some(item) = state.items.get_mut(state.item_index) {
                            if let Some(s) = item.suggestions.get_mut(state.suggestion_cursor) {
                                s.accepted = !s.accepted;
                            }
                        }
                    }
                }
            }
            KeyCode::Char('A') => {
                if let Some(state) = &mut self.classification.suggestion_review {
                    if let Some(item) = state.items.get_mut(state.item_index) {
                        for s in &mut item.suggestions {
                            s.accepted = true;
                        }
                        self.status = "All suggestions marked as accepted".to_string();
                    }
                }
            }
            KeyCode::Char('s') => {
                // Skip: advance to next item without confirming
                if let Some(state) = &mut self.classification.suggestion_review {
                    let len = state.items.len();
                    if len > 1 {
                        state.item_index = (state.item_index + 1) % len;
                        state.suggestion_cursor = 0;
                        state.focus = SuggestionReviewFocus::Suggestions;
                        self.status = "Skipped to next item".to_string();
                    } else {
                        self.status = "No other items to skip to".to_string();
                    }
                }
            }
            KeyCode::Enter => {
                self.confirm_suggestion_review_item(agenda)?;
            }
            _ => {}
        }
        Ok(false)
    }

    fn confirm_suggestion_review_item(&mut self, agenda: &Agenda<'_>) -> TuiResult<()> {
        // Collect decisions and mutate state in a scoped borrow, then refresh after.
        let outcome = {
            let state = match &mut self.classification.suggestion_review {
                Some(s) => s,
                None => return Ok(()),
            };

            let item_index = state.item_index;
            if item_index >= state.items.len() {
                return Ok(());
            }

            let item = &state.items[item_index];
            let mut accepted_count = 0usize;
            let mut rejected_count = 0usize;

            for review in &item.suggestions {
                if review.accepted {
                    agenda.accept_classification_suggestion(review.suggestion.id)?;
                    accepted_count += 1;
                } else {
                    agenda.reject_classification_suggestion(review.suggestion.id)?;
                    rejected_count += 1;
                }
            }

            state.resolved_count += accepted_count + rejected_count;
            state.resolved_items += 1;
            state.items.remove(item_index);

            if state.items.is_empty() {
                let resolved_count = state.resolved_count;
                let resolved_items = state.resolved_items;
                Some((
                    resolved_count,
                    resolved_items,
                    accepted_count,
                    rejected_count,
                    true,
                ))
            } else {
                state.item_index = state.item_index.min(state.items.len() - 1);
                state.suggestion_cursor = 0;
                state.focus = SuggestionReviewFocus::Suggestions;
                let remaining = state.items.len();
                Some((remaining, 0, accepted_count, rejected_count, false))
            }
        };

        if let Some((count, resolved_items, accepted, rejected, is_done)) = outcome {
            self.refresh(agenda.store())?;
            if is_done {
                self.classification.suggestion_review = None;
                self.mode = Mode::Normal;
                self.status = format!(
                    "Review complete: {} suggestion{} resolved across {} item{}",
                    count,
                    if count == 1 { "" } else { "s" },
                    resolved_items,
                    if resolved_items == 1 { "" } else { "s" },
                );
            } else {
                self.status = format!(
                    "Confirmed ({accepted} accepted, {rejected} rejected). {count} item{} remaining.",
                    if count == 1 { "" } else { "s" },
                );
            }
        }
        Ok(())
    }
}
