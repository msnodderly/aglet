use crate::*;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::time::{Duration, Instant};

use agenda_core::classification::PROVIDER_ID_IMPLICIT_STRING;
use agenda_core::workflow::{blocked_item_ids, READY_QUEUE_VIEW_NAME};

pub(crate) fn parse_external_editor_command(editor: &str) -> Result<(String, Vec<String>), String> {
    let Some(parts) = shlex::split(editor) else {
        return Err(format!("Could not parse $EDITOR value: {editor}"));
    };
    let Some((command, args)) = parts.split_first() else {
        return Err("External editor command is empty".to_string());
    };
    Ok((command.clone(), args.to_vec()))
}

impl App {
    const AUTO_REFRESH_STATUS_TTL: Duration = Duration::from_millis(2_000);
    const AUTO_REFRESH_SETTING_KEY: &'static str = "tui.auto_refresh_interval";
    const SECTION_BORDER_MODE_SETTING_KEY: &'static str = "tui.section_border_mode";
    const SHOW_NOTE_GLYPHS_SETTING_KEY: &'static str = "tui.show_note_glyphs";
    const LAST_VIEW_NAME_SETTING_KEY: &'static str = "tui.last_view_name";

    pub(crate) fn active_transient_status_text(&self) -> Option<&str> {
        self.transient.status.as_ref().and_then(|transient| {
            if Instant::now() < transient.expires_at {
                Some(transient.message.as_str())
            } else {
                None
            }
        })
    }

    pub(crate) fn clear_expired_transient_status(&mut self) -> bool {
        let expired = self
            .transient
            .status
            .as_ref()
            .is_some_and(|transient| Instant::now() >= transient.expires_at);
        if expired {
            self.transient.status = None;
        }
        expired
    }

    pub(crate) fn clear_transient_status_on_key(&mut self, _key: KeyEvent) {
        if self.transient.status.is_some() {
            self.transient.status = None;
        }
    }

    fn should_auto_refresh_now(&self) -> bool {
        let Some(interval) = self.auto_refresh_interval.as_duration() else {
            return false;
        };
        self.mode == Mode::Normal && self.auto_refresh_last_tick.elapsed() >= interval
    }

    fn current_minute_key() -> (i16, i8, i8, i8, i8) {
        let now = jiff::Zoned::now().datetime();
        (now.year(), now.month(), now.day(), now.hour(), now.minute())
    }

    fn mark_temporal_refresh_now(&mut self) {
        self.last_temporal_refresh_minute = Some(Self::current_minute_key());
    }

    fn maybe_run_temporal_reevaluation(&mut self, agenda: &Agenda<'_>) -> TuiResult<()> {
        if !agenda.has_date_conditions()? {
            return Ok(());
        }
        let current_minute = Self::current_minute_key();
        if self.last_temporal_refresh_minute == Some(current_minute) {
            return Ok(());
        }
        let _ = agenda.reevaluate_temporal_conditions()?;
        self.last_temporal_refresh_minute = Some(current_minute);
        Ok(())
    }

    pub(crate) fn auto_refresh_mode_label(&self) -> &'static str {
        self.auto_refresh_interval.label()
    }

    pub(crate) fn set_auto_refresh_interval(&mut self, interval: AutoRefreshInterval) {
        self.auto_refresh_interval = interval;
        self.auto_refresh_last_tick = Instant::now();
        self.transient.status = Some(TransientStatus {
            message: format!("Auto-refresh interval: {}", self.auto_refresh_mode_label()),
            expires_at: Instant::now() + Self::AUTO_REFRESH_STATUS_TTL,
        });
    }

    pub(crate) fn section_border_mode_label(&self) -> &'static str {
        self.section_border_mode.label()
    }

    pub(crate) fn set_section_border_mode(&mut self, mode: SectionBorderMode) {
        self.section_border_mode = mode;
    }

    pub(crate) fn load_auto_refresh_interval(&mut self, store: &Store) -> TuiResult<()> {
        let persisted = store.get_app_setting(Self::AUTO_REFRESH_SETTING_KEY)?;
        self.auto_refresh_interval = persisted
            .as_deref()
            .and_then(AutoRefreshInterval::from_persisted_value)
            .unwrap_or(AutoRefreshInterval::Off);
        self.auto_refresh_last_tick = Instant::now();
        Ok(())
    }

    pub(crate) fn load_section_border_mode(&mut self, store: &Store) -> TuiResult<()> {
        let persisted = store.get_app_setting(Self::SECTION_BORDER_MODE_SETTING_KEY)?;
        self.section_border_mode = persisted
            .as_deref()
            .and_then(SectionBorderMode::from_persisted_value)
            .unwrap_or(SectionBorderMode::Full);
        Ok(())
    }

    pub(crate) fn persist_auto_refresh_interval(&self, store: &Store) -> TuiResult<()> {
        store.set_app_setting(
            Self::AUTO_REFRESH_SETTING_KEY,
            self.auto_refresh_interval.persisted_value(),
        )?;
        Ok(())
    }

    pub(crate) fn persist_section_border_mode(&self, store: &Store) -> TuiResult<()> {
        store.set_app_setting(
            Self::SECTION_BORDER_MODE_SETTING_KEY,
            self.section_border_mode.persisted_value(),
        )?;
        Ok(())
    }

    pub(crate) fn show_note_glyphs_label(&self) -> &'static str {
        if self.show_note_glyphs {
            "on"
        } else {
            "off"
        }
    }

    pub(crate) fn load_show_note_glyphs(&mut self, store: &Store) -> TuiResult<()> {
        let persisted = store.get_app_setting(Self::SHOW_NOTE_GLYPHS_SETTING_KEY)?;
        self.show_note_glyphs = persisted.as_deref() == Some("on");
        Ok(())
    }

    pub(crate) fn persist_show_note_glyphs(&self, store: &Store) -> TuiResult<()> {
        store.set_app_setting(
            Self::SHOW_NOTE_GLYPHS_SETTING_KEY,
            if self.show_note_glyphs { "on" } else { "off" },
        )?;
        Ok(())
    }

    pub(crate) fn load_last_view_name(&mut self, store: &Store) -> TuiResult<()> {
        let persisted = store.get_app_setting(Self::LAST_VIEW_NAME_SETTING_KEY)?;
        if let Some(view_name) = persisted {
            if let Some(index) = self
                .views
                .iter()
                .position(|view| view.name.eq_ignore_ascii_case(&view_name))
            {
                self.set_active_view_index(index);
            }
        }
        Ok(())
    }

    pub(crate) fn persist_last_view_name(&self, store: &Store) -> TuiResult<()> {
        if let Some(view_name) = &self.active_view_name {
            store.set_app_setting(Self::LAST_VIEW_NAME_SETTING_KEY, view_name)?;
        }
        Ok(())
    }

    pub(crate) fn open_global_settings(&mut self, store: &Store) -> TuiResult<()> {
        self.refresh(store)?;
        self.load_auto_refresh_interval(store)?;
        self.load_section_border_mode(store)?;
        self.load_show_note_glyphs(store)?;
        self.settings.global_settings = Some(GlobalSettingsState::default());
        self.mode = Mode::GlobalSettings;
        self.status =
            "Global settings: j/k move, Space or ←/→ cycle, Enter pick, Esc close".to_string();
        Ok(())
    }

    pub(crate) fn maybe_run_auto_refresh(&mut self, agenda: &Agenda<'_>) -> TuiResult<bool> {
        if self.should_auto_refresh_now() {
            self.maybe_run_temporal_reevaluation(agenda)?;
            self.refresh(agenda.store())?;
            self.auto_refresh_last_tick = Instant::now();
            return Ok(true);
        }
        Ok(false)
    }

    pub(crate) fn effective_section_flow(&self) -> SectionFlow {
        self.current_view()
            .map(|view| view.section_flow)
            .unwrap_or(SectionFlow::Vertical)
    }

    pub(crate) fn is_horizontal_section_flow(&self) -> bool {
        self.effective_section_flow() == SectionFlow::Horizontal
    }

    fn clamp_horizontal_slot_item_indices(&mut self) {
        if self.horizontal_slot_item_indices.len() != self.slots.len() {
            self.horizontal_slot_item_indices = vec![0; self.slots.len()];
        }
        for (slot_index, slot) in self.slots.iter().enumerate() {
            let max_index = slot.items.len().saturating_sub(1);
            if let Some(stored) = self.horizontal_slot_item_indices.get_mut(slot_index) {
                *stored = (*stored).min(max_index);
            }
        }
    }

    fn clamp_horizontal_slot_scroll_offsets(&self) {
        let mut offsets = self.horizontal_slot_scroll_offsets.borrow_mut();
        if offsets.len() != self.slots.len() {
            offsets.resize(self.slots.len(), 0);
        }
        for (slot_index, slot) in self.slots.iter().enumerate() {
            let max_index = slot.items.len().saturating_sub(1);
            if let Some(stored) = offsets.get_mut(slot_index) {
                *stored = (*stored).min(max_index);
            }
        }
    }

    pub(crate) fn run(&mut self, terminal: &mut TuiTerminal, agenda: &Agenda<'_>) -> TuiResult<()> {
        self.maybe_run_temporal_reevaluation(agenda)?;
        self.refresh(agenda.store())?;
        self.load_last_view_name(agenda.store())?;
        self.refresh(agenda.store())?; // re-resolve slots for the restored view
        self.load_auto_refresh_interval(agenda.store())?;
        self.load_section_border_mode(agenda.store())?;
        self.load_show_note_glyphs(agenda.store())?;
        self.auto_refresh_last_tick = Instant::now();
        self.mark_temporal_refresh_now();
        let mut needs_redraw = true;

        loop {
            needs_redraw |= self.clear_expired_transient_status();
            needs_redraw |= self.process_classification_results(agenda)?;
            if needs_redraw {
                terminal.draw(|frame| self.draw(frame))?;
                needs_redraw = false;
            }

            if !event::poll(std::time::Duration::from_millis(200))? {
                needs_redraw |= self.maybe_run_auto_refresh(agenda)?;
                continue;
            }

            match event::read()? {
                Event::Resize(_, _) => {
                    needs_redraw = true;
                    continue;
                }
                Event::Key(key) => {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }

                    let should_quit = match self.handle_key_event(key, agenda) {
                        Ok(value) => value,
                        Err(err) => {
                            self.mode = Mode::Normal;
                            self.clear_input();
                            self.status = format!("Error: {err}");
                            false
                        }
                    };
                    needs_redraw = true;
                    if should_quit {
                        let _ = self.persist_last_view_name(agenda.store());
                        break;
                    }

                    if let Some(target) = self.transient.pending_external_edit.take() {
                        self.run_external_editor(terminal, target)?;
                        needs_redraw = true;
                    }

                    needs_redraw |= self.maybe_run_auto_refresh(agenda)?;
                }
                _ => continue,
            };
        }

        Ok(())
    }

    pub(crate) fn refresh(&mut self, store: &Store) -> TuiResult<()> {
        self.views = projection::load_views_with_ready_queue(store)?;
        self.workflow_config = store.get_workflow_config()?;
        if let Some(active_view_name) = self.active_view_name.clone() {
            if let Some(index) = self
                .views
                .iter()
                .position(|view| view.name.eq_ignore_ascii_case(&active_view_name))
            {
                self.view_index = index;
            }
        }
        self.categories = store.get_hierarchy()?;
        self.category_rows = build_category_rows(&self.categories);
        if let Some(category_id) = self
            .category_manager
            .as_ref()
            .and_then(|state| state.selected_category_id)
        {
            if let Some(index) = self
                .category_rows
                .iter()
                .position(|row| row.id == category_id)
            {
                self.category_index = index;
            }
        }
        self.category_index = self
            .category_index
            .min(self.category_rows.len().saturating_sub(1));
        if self.category_manager.is_some() {
            // Keep visible-row projection in sync with rebuilt category_rows.
            self.rebuild_category_manager_visible_rows();
        }
        self.sync_category_manager_state_from_selection();
        let items = store.list_items()?;
        self.all_items = items.clone();
        self.blocked_item_ids = blocked_item_ids(store, &items)?;
        self.category_assignment_counts.clear();
        for item in &items {
            for cat_id in item.assignments.keys() {
                *self.category_assignment_counts.entry(*cat_id).or_insert(0) += 1;
            }
        }
        self.item_links_by_item_id.clear();
        for item in &items {
            let links = agenda_core::model::ItemLinksForItem {
                depends_on: store.list_dependency_ids_for_item(item.id)?,
                blocks: store.list_dependent_ids_for_item(item.id)?,
                related: store.list_related_ids_for_item(item.id)?,
            };
            self.item_links_by_item_id.insert(item.id, links);
        }
        let slots = projection::project_slots(self, store, &items)?;

        self.slots = slots;
        self.prune_selected_items_to_visible_slots();
        self.clamp_horizontal_slot_item_indices();
        self.clamp_horizontal_slot_scroll_offsets();
        self.slot_index = self.slot_index.min(self.slots.len().saturating_sub(1));
        self.board_scroll_offset = self
            .board_scroll_offset
            .min(self.slots.len().saturating_sub(1));
        // If Hide mode landed us on an empty slot, advance to the nearest non-empty.
        if self.effective_empty_sections() == EmptySections::Hide {
            self.skip_hidden_slots(1);
        }
        if self.is_horizontal_section_flow() {
            self.item_index = self
                .horizontal_slot_item_indices
                .get(self.slot_index)
                .copied()
                .unwrap_or(0)
                .min(
                    self.current_slot()
                        .map(|slot| slot.items.len().saturating_sub(1))
                        .unwrap_or(0),
                );
        } else {
            self.item_index = self.item_index.min(
                self.current_slot()
                    .map(|slot| slot.items.len().saturating_sub(1))
                    .unwrap_or(0),
            );
            if let Some(stored) = self.horizontal_slot_item_indices.get_mut(self.slot_index) {
                *stored = self.item_index;
            }
        }
        if self.is_horizontal_section_flow() {
            self.column_index = self.current_slot_item_column_index();
        } else {
            self.column_index = self.column_index.min(self.current_slot_column_count());
        }
        let provenance_len = self.preview_info_line_count_for_selected_item();
        let summary_len = self
            .selected_item()
            .map(|item| self.item_details_lines_for_item(item).len())
            .unwrap_or(0);
        self.inspect_assignment_index = self
            .inspect_assignment_index
            .min(provenance_len.saturating_sub(1));
        self.preview_provenance_scroll = self
            .preview_provenance_scroll
            .min(provenance_len.saturating_sub(1));
        self.preview_summary_scroll = self
            .preview_summary_scroll
            .min(summary_len.saturating_sub(1));
        self.rebuild_classification_ui(store)?;

        Ok(())
    }

    fn rebuild_classification_ui(&mut self, store: &Store) -> TuiResult<()> {
        let config = store.get_classification_config()?;
        let mut pending = store.list_pending_suggestions()?;
        pending.sort_by(|left, right| {
            left.item_id
                .cmp(&right.item_id)
                .then_with(|| left.created_at.cmp(&right.created_at))
        });

        let category_names = category_name_map(&self.categories);
        let mut grouped: HashMap<ItemId, Vec<ClassificationSuggestion>> = HashMap::new();
        for suggestion in pending {
            grouped
                .entry(suggestion.item_id)
                .or_default()
                .push(suggestion);
        }

        let mut review_items: Vec<ClassificationReviewItem> = grouped
            .into_iter()
            .map(|(item_id, mut suggestions)| {
                suggestions.sort_by(|left, right| left.created_at.cmp(&right.created_at));
                if let Some(item) = self.all_items.iter().find(|item| item.id == item_id) {
                    ClassificationReviewItem {
                        item_id,
                        item_text: item.text.clone(),
                        note_excerpt: item
                            .note
                            .as_deref()
                            .map(str::trim)
                            .filter(|note| !note.is_empty())
                            .map(|note| note.lines().next().unwrap_or(note).to_string()),
                        current_assignments: item_assignment_labels(item, &category_names),
                        suggestions,
                    }
                } else {
                    ClassificationReviewItem {
                        item_id,
                        item_text: format!("Missing item {item_id}"),
                        note_excerpt: None,
                        current_assignments: Vec::new(),
                        suggestions,
                    }
                }
            })
            .collect();
        review_items.sort_by(|left, right| {
            right
                .suggestions
                .first()
                .map(|suggestion| suggestion.created_at)
                .cmp(
                    &left
                        .suggestions
                        .first()
                        .map(|suggestion| suggestion.created_at),
                )
                .then_with(|| {
                    left.item_text
                        .to_ascii_lowercase()
                        .cmp(&right.item_text.to_ascii_lowercase())
                })
        });

        self.classification.ui.pending_count =
            review_items.iter().map(|item| item.suggestions.len()).sum();
        self.classification.ui.config = config;
        self.classification.ui.review_items = review_items;

        Ok(())
    }

    /// Drain completed background classification results and apply them.
    pub(crate) fn process_classification_results(
        &mut self,
        agenda: &Agenda<'_>,
    ) -> TuiResult<bool> {
        let mut any_applied = false;
        let mut state_changed = false;
        while let Some(result) = self.classification.worker.try_recv() {
            state_changed = true;
            self.classification
                .in_flight_classifications
                .remove(&result.item_id);
            self.debug_log_classification(
                agenda,
                &format!(
                    "background_classification: finished item_id={} candidates={} debug_summaries={:?} error={:?}",
                    result.item_id,
                    result.candidates.len(),
                    result.debug_summaries,
                    result.error
                ),
            );

            if let Some(error) = result.error {
                self.status = format!("Classification error: {error}");
                continue;
            }

            // Staleness check: compare revision hash against current item state.
            let current_item = match agenda.store().get_item(result.item_id) {
                Ok(item) => item,
                Err(_) => continue, // item was deleted
            };
            let current_hash = agenda_core::classification::item_revision_hash(&current_item);
            if current_hash != result.item_revision_hash {
                self.status = format!(
                    "Classification skipped for '{}' (item was modified)",
                    truncate_str(&current_item.text, 40)
                );
                continue;
            }

            let queued = agenda.apply_classification_results(
                result.item_id,
                &result.item_revision_hash,
                &result.candidates,
            )?;

            any_applied = true;
            let remaining = self.classification.in_flight_classifications.len();
            if remaining > 0 {
                let sug_part = if queued > 0 {
                    format!(
                        "{queued} new {} — ",
                        if queued == 1 {
                            "suggestion"
                        } else {
                            "suggestions"
                        }
                    )
                } else {
                    String::new()
                };
                self.status = format!(
                    "Classified '{}': {sug_part}{remaining} still pending…",
                    truncate_str(&current_item.text, 30)
                );
            } else if queued > 0 {
                let sug_word = if queued == 1 {
                    "suggestion"
                } else {
                    "suggestions"
                };
                self.status =
                    format!("Classification complete: {queued} new {sug_word} (Shift+C to review)");
            } else if !result.debug_summaries.is_empty() {
                self.status = format!(
                    "Classification complete: no new suggestions ({})",
                    result.debug_summaries.join("; ")
                );
            } else {
                self.status = "Classification complete: no new suggestions".to_string();
            }
        }

        if any_applied {
            self.refresh(agenda.store())?;
        }
        Ok(state_changed)
    }

    /// Submit background classification for a single item.
    /// Returns true if a job was submitted, false if skipped.
    pub(crate) fn submit_background_classification(
        &mut self,
        agenda: &Agenda<'_>,
        item_id: ItemId,
    ) -> TuiResult<BackgroundClassificationSubmitResult> {
        if self
            .classification
            .in_flight_classifications
            .contains(&item_id)
        {
            self.debug_log_classification(
                agenda,
                &format!(
                    "background_classification: skip item_id={item_id} reason=already_in_flight"
                ),
            );
            return Ok(BackgroundClassificationSubmitResult::AlreadyInFlight);
        }
        let reference_date = jiff::Zoned::now().date();
        if let Some(job) = agenda.prepare_background_classification(item_id, reference_date)? {
            self.classification
                .in_flight_classifications
                .insert(item_id);
            self.debug_log_classification(
                agenda,
                &format!(
                    "background_classification: submit item_id={item_id} semantic_provider={:?}",
                    job.config.semantic_provider
                ),
            );
            if self.classification.worker.submit(job) {
                Ok(BackgroundClassificationSubmitResult::Submitted)
            } else {
                self.classification
                    .in_flight_classifications
                    .remove(&item_id);
                self.debug_log_classification(
                    agenda,
                    &format!(
                        "background_classification: submit_failed item_id={item_id} reason=worker_disconnected"
                    ),
                );
                Err(std::io::Error::other("background classification worker unavailable").into())
            }
        } else {
            self.debug_log_classification(
                agenda,
                &format!("background_classification: skip item_id={item_id} reason=no_providers"),
            );
            Ok(BackgroundClassificationSubmitResult::NoProvidersEnabled)
        }
    }

    fn debug_log_classification(&self, agenda: &Agenda<'_>, message: &str) {
        if !agenda.debug_enabled() {
            return;
        }
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(agenda_core::classification::CLASSIFICATION_DEBUG_LOG_PATH)
        {
            let _ = writeln!(file, "[{}] {message}", jiff::Zoned::now());
        }
    }

    pub(crate) fn move_slot_cursor(&mut self, delta: i32) {
        if self.slots.is_empty() {
            return;
        }
        let hide_empty = self.effective_empty_sections() == EmptySections::Hide;
        if self.is_horizontal_section_flow() {
            if let Some(stored) = self.horizontal_slot_item_indices.get_mut(self.slot_index) {
                *stored = self.item_index;
            }
            self.slot_index = next_index_clamped(self.slot_index, self.slots.len(), delta);
            if hide_empty {
                self.skip_hidden_slots(delta);
            }
            self.item_index = self
                .horizontal_slot_item_indices
                .get(self.slot_index)
                .copied()
                .unwrap_or(0)
                .min(
                    self.current_slot()
                        .map(|slot| slot.items.len().saturating_sub(1))
                        .unwrap_or(0),
                );
            self.column_index = self.current_slot_item_column_index();
            return;
        }
        if let Some(stored) = self.horizontal_slot_item_indices.get_mut(self.slot_index) {
            *stored = self.item_index;
        }
        self.slot_index = next_index_clamped(self.slot_index, self.slots.len(), delta);
        if hide_empty {
            self.skip_hidden_slots(delta);
        }
        self.item_index = self
            .horizontal_slot_item_indices
            .get(self.slot_index)
            .copied()
            .unwrap_or(0)
            .min(
                self.current_slot()
                    .map(|slot| slot.items.len().saturating_sub(1))
                    .unwrap_or(0),
            );
        self.column_index = self.current_slot_item_column_index();
    }

    /// When EmptySections::Hide is active, advance slot_index past empty slots
    /// in the direction of `delta`. If no non-empty slot is reachable, stay put.
    fn skip_hidden_slots(&mut self, delta: i32) {
        let step = if delta > 0 { 1i32 } else { -1i32 };
        let start = self.slot_index;
        for _ in 0..self.slots.len() {
            if self
                .slots
                .get(self.slot_index)
                .is_none_or(|s| !s.items.is_empty())
            {
                return; // landed on a non-empty slot (or out of range)
            }
            let next = next_index_clamped(self.slot_index, self.slots.len(), step);
            if next == self.slot_index {
                break; // hit edge, can't advance further
            }
            self.slot_index = next;
        }
        // If we couldn't find a non-empty slot, revert.
        if self
            .slots
            .get(self.slot_index)
            .is_none_or(|s| s.items.is_empty())
        {
            self.slot_index = start;
        }
    }

    pub(crate) fn move_item_cursor(&mut self, delta: i32) {
        let Some(slot) = self.current_slot() else {
            return;
        };
        if slot.items.is_empty() {
            self.item_index = 0;
            if let Some(stored) = self.horizontal_slot_item_indices.get_mut(self.slot_index) {
                *stored = 0;
            }
            return;
        }
        self.item_index = next_index_clamped(self.item_index, slot.items.len(), delta);
        if let Some(stored) = self.horizontal_slot_item_indices.get_mut(self.slot_index) {
            *stored = self.item_index;
        }
    }

    pub(crate) fn move_category_cursor(&mut self, delta: i32) {
        if let Some(state) = &self.category_manager {
            if !state.visible_row_indices.is_empty() {
                let next_visible =
                    next_index_clamped(state.tree_index, state.visible_row_indices.len(), delta);
                self.set_category_manager_visible_selection(next_visible);
                return;
            }
        }
        if self.category_rows.is_empty() {
            self.category_index = 0;
            self.sync_category_manager_state_from_selection();
            return;
        }
        self.category_index =
            next_index_clamped(self.category_index, self.category_rows.len(), delta);
        self.sync_category_manager_state_from_selection();
    }

    pub(crate) fn move_selected_item_between_slots(
        &mut self,
        delta: i32,
        agenda: &Agenda<'_>,
    ) -> TuiResult<()> {
        // Datebook sections are time-based; moving items between them would
        // require changing the when_date, not category assignments.
        if self
            .current_view()
            .is_some_and(|v| v.datebook_config.is_some())
        {
            self.status = "Cannot move items between datebook sections".to_string();
            return Ok(());
        }
        if self.slots.len() < 2 {
            return Ok(());
        }
        let Some(item_id) = self.selected_item_id() else {
            return Ok(());
        };

        let from_index = self.slot_index;
        let to_index = next_index_clamped(self.slot_index, self.slots.len(), delta);
        if from_index == to_index {
            return Ok(());
        }

        let from_context = self
            .slots
            .get(from_index)
            .map(|slot| slot.context.clone())
            .ok_or("Invalid source slot".to_string())?;
        let to_context = self
            .slots
            .get(to_index)
            .map(|slot| slot.context.clone())
            .ok_or("Invalid target slot".to_string())?;
        let to_title = self
            .slots
            .get(to_index)
            .map(|slot| slot.title.clone())
            .unwrap_or_else(|| "new section".to_string());
        let view = self
            .current_view()
            .cloned()
            .ok_or("No active view".to_string())?;

        let from_section = Self::section_for_context(&view, &from_context)?;
        let to_section = Self::section_for_context(&view, &to_context)?;
        let preview =
            Agenda::preview_section_move(&view, from_section.as_ref(), to_section.as_ref());

        match (from_section, to_section) {
            (Some(from_section), Some(to_section)) => {
                agenda.move_item_between_sections(item_id, &view, &from_section, &to_section)?;
            }
            (Some(from_section), None) => {
                agenda.remove_item_from_section(item_id, &view, &from_section)?;
            }
            (None, Some(to_section)) => {
                agenda.insert_item_in_section(item_id, &view, &to_section)?;
            }
            (None, None) => {}
        }

        self.slot_index = to_index;
        self.refresh(agenda.store())?;
        self.item_index = self
            .slots
            .get(to_index)
            .and_then(|slot| slot.items.iter().position(|i| i.id == item_id))
            .unwrap_or(0);
        self.status = self.section_move_status(&to_title, &preview);
        Ok(())
    }

    fn section_move_status(
        &self,
        target_title: &str,
        preview: &agenda_core::agenda::SectionMovePreview,
    ) -> String {
        let mut removed = self.category_names_for_status(&preview.to_unassign);
        let mut added = self.category_names_for_status(&preview.to_assign);
        if removed.is_empty() && added.is_empty() {
            return format!("Moved to {target_title}");
        }

        removed.iter_mut().for_each(|name| name.insert(0, '-'));
        added.iter_mut().for_each(|name| name.insert(0, '+'));
        let mut changes = removed;
        changes.extend(added);
        format!("Moved to {target_title} ({})", changes.join(" "))
    }

    fn category_names_for_status(&self, category_ids: &HashSet<CategoryId>) -> Vec<String> {
        let mut names: Vec<String> = category_ids
            .iter()
            .map(|id| {
                self.categories
                    .iter()
                    .find(|category| category.id == *id)
                    .map(|category| category.name.clone())
                    .unwrap_or_else(|| "(deleted)".to_string())
            })
            .collect();
        names.sort_by_key(|name| name.to_ascii_lowercase());
        names
    }

    pub(crate) fn insert_into_context(
        &self,
        agenda: &Agenda<'_>,
        item_id: ItemId,
        view: &View,
        context: &SlotContext,
    ) -> TuiResult<()> {
        match Self::section_for_context(view, context)? {
            Some(section) => {
                agenda.insert_item_in_section(item_id, view, &section)?;
            }
            None => {
                agenda.insert_item_in_unmatched(item_id, view)?;
            }
        }
        Ok(())
    }

    fn section_for_context(view: &View, context: &SlotContext) -> TuiResult<Option<Section>> {
        match context {
            SlotContext::Section { section_index } => view
                .sections
                .get(*section_index)
                .cloned()
                .map(Some)
                .ok_or_else(|| "Section not found".into()),
            SlotContext::GeneratedSection {
                section_index,
                on_insert_assign,
                on_remove_unassign,
            } => {
                let mut section = view
                    .sections
                    .get(*section_index)
                    .cloned()
                    .ok_or("Section not found".to_string())?;
                section.on_insert_assign = on_insert_assign.clone();
                section.on_remove_unassign = on_remove_unassign.clone();
                Ok(Some(section))
            }
            SlotContext::Unmatched => Ok(None),
        }
    }

    pub(crate) fn current_slot(&self) -> Option<&Slot> {
        self.slots.get(self.slot_index)
    }

    /// For datebook views, returns the slot index whose date range contains today.
    pub(crate) fn datebook_today_slot_index(&self) -> Option<usize> {
        let view = self.current_view()?;
        let config = view.datebook_config.as_ref()?;
        let today = jiff::Zoned::now().date();
        let today_dt = today.at(0, 0, 0, 0);
        let sections = generate_datebook_sections(config, today);
        sections
            .iter()
            .position(|s| today_dt >= s.range_start && today_dt < s.range_end)
    }

    /// For datebook views, returns the start date of the current slot as a
    /// "YYYY-MM-DD" string, suitable for pre-filling the when_buffer.
    pub(crate) fn datebook_slot_date_string(&self) -> Option<String> {
        let view = self.current_view()?;
        let config = view.datebook_config.as_ref()?;
        let slot = self.current_slot()?;
        // Only Section slots map 1:1 with datebook sections (not Unmatched)
        let section_index = match &slot.context {
            SlotContext::Section { section_index } => *section_index,
            _ => return None,
        };
        let today = jiff::Zoned::now().date();
        let sections = generate_datebook_sections(config, today);
        let ds = sections.get(section_index)?;
        Some(format!(
            "{:04}-{:02}-{:02}",
            ds.range_start.date().year(),
            ds.range_start.date().month(),
            ds.range_start.date().day()
        ))
    }

    pub(crate) fn current_slot_sort_column(&self) -> Option<SlotSortColumn> {
        let slot = self.current_slot()?;
        self.slot_sort_column_for_board_index(slot, self.column_index)
    }

    pub(crate) fn slot_sort_column_for_board_index(
        &self,
        slot: &Slot,
        board_column_index: usize,
    ) -> Option<SlotSortColumn> {
        let item_column_index = self.slot_item_column_index(slot);
        if board_column_index == item_column_index {
            return Some(SlotSortColumn::ItemText);
        }

        let view = self.current_view()?;
        let section_index = match slot.context {
            SlotContext::Section { section_index }
            | SlotContext::GeneratedSection { section_index, .. } => section_index,
            SlotContext::Unmatched => return None,
        };
        let section = view.sections.get(section_index)?;
        let section_column_index =
            Self::board_column_to_section_column_index(section, board_column_index)?;
        let column = section.columns.get(section_column_index)?;
        Some(SlotSortColumn::SectionColumn {
            heading: column.heading,
            kind: column.kind,
        })
    }

    pub(crate) fn section_item_column_index(section: &Section) -> usize {
        section.item_column_index.min(section.columns.len())
    }

    pub(crate) fn slot_item_column_index(&self, slot: &Slot) -> usize {
        let Some(view) = self.current_view() else {
            return 0;
        };
        match slot.context {
            SlotContext::Section { section_index }
            | SlotContext::GeneratedSection { section_index, .. } => view
                .sections
                .get(section_index)
                .map(Self::section_item_column_index)
                .unwrap_or(0),
            SlotContext::Unmatched => 0,
        }
    }

    pub(crate) fn current_slot_item_column_index(&self) -> usize {
        self.current_slot()
            .map(|slot| self.slot_item_column_index(slot))
            .unwrap_or(0)
    }

    pub(crate) fn board_column_to_section_column_index(
        section: &Section,
        board_column_index: usize,
    ) -> Option<usize> {
        let item_column_index = Self::section_item_column_index(section);
        if board_column_index > section.columns.len() || board_column_index == item_column_index {
            return None;
        }
        Some(if board_column_index < item_column_index {
            board_column_index
        } else {
            board_column_index - 1
        })
    }

    pub(crate) fn section_column_to_board_column_index(
        section: &Section,
        section_column_index: usize,
    ) -> usize {
        let item_column_index = Self::section_item_column_index(section);
        if section_column_index < item_column_index {
            section_column_index
        } else {
            section_column_index + 1
        }
    }

    pub(crate) fn current_slot_column_count(&self) -> usize {
        let Some(slot) = self.current_slot() else {
            return 0;
        };
        let Some(view) = self.current_view() else {
            return 0;
        };
        match slot.context {
            SlotContext::Section { section_index }
            | SlotContext::GeneratedSection {
                section_index,
                on_insert_assign: _,
                on_remove_unassign: _,
            } => view
                .sections
                .get(section_index)
                .map(|s| s.columns.len())
                .unwrap_or(0),
            SlotContext::Unmatched => 0,
        }
    }

    pub(crate) fn selected_item(&self) -> Option<&Item> {
        self.current_slot()
            .and_then(|slot| slot.items.get(self.item_index))
    }

    #[cfg(test)]
    pub(crate) fn classification_pending_count(&self) -> usize {
        self.classification.ui.pending_count
    }

    pub(crate) fn pending_suggestion_count_for_item(&self, item_id: ItemId) -> usize {
        self.classification
            .ui
            .review_items
            .iter()
            .find(|item| item.item_id == item_id)
            .map(|item| item.suggestions.len())
            .unwrap_or(0)
    }

    pub(crate) fn classification_pending_suffix(&self) -> Option<String> {
        if self.classification.ui.pending_count == 0 {
            None
        } else {
            Some(format!(
                "? {} pending suggestion{}",
                self.classification.ui.pending_count,
                if self.classification.ui.pending_count == 1 {
                    ""
                } else {
                    "s"
                }
            ))
        }
    }

    pub(crate) fn assignment_event_status_summary(
        &self,
        result: &agenda_core::engine::ProcessItemResult,
    ) -> Option<String> {
        if result.assignment_events.is_empty() {
            return None;
        }

        let mut unique = Vec::new();
        for event in &result.assignment_events {
            let summary = event.concise_summary();
            if !unique.contains(&summary) {
                unique.push(summary);
            }
        }
        if unique.is_empty() {
            return None;
        }

        let mut message = unique.into_iter().take(3).collect::<Vec<_>>().join("; ");
        let extra = result.assignment_events.len().saturating_sub(3);
        if extra > 0 {
            message.push_str(&format!("; +{extra} more"));
        }
        Some(message)
    }

    pub(crate) fn classification_feedback_for_saved_item(
        &self,
        item_id: ItemId,
        result: &agenda_core::engine::ProcessItemResult,
    ) -> Option<(String, bool)> {
        let semantic_debug = if result.semantic_debug_messages.is_empty() {
            None
        } else {
            let mut unique = Vec::new();
            for msg in &result.semantic_debug_messages {
                if !unique.contains(msg) {
                    unique.push(msg.clone());
                }
            }
            Some(unique.join("; "))
        };
        let pending_for_item = self.pending_suggestion_count_for_item(item_id);
        if pending_for_item > 0 {
            return Some((
                match semantic_debug {
                    Some(debug) => format!(
                        "? {pending_for_item} pending suggestion{} for this item | {debug}",
                        if pending_for_item == 1 { "" } else { "s" }
                    ),
                    None => format!(
                        "? {pending_for_item} pending suggestion{} for this item",
                        if pending_for_item == 1 { "" } else { "s" }
                    ),
                },
                true,
            ));
        }

        if let Some(message) = self.assignment_event_status_summary(result) {
            return Some((message, false));
        }

        if result.semantic_candidates_seen > 0 {
            if result.semantic_candidates_skipped_already_assigned
                + result.semantic_candidates_skipped_unavailable
                == result.semantic_candidates_seen
            {
                let reason = match (
                    result.semantic_candidates_skipped_already_assigned > 0,
                    result.semantic_candidates_skipped_unavailable > 0,
                ) {
                    (true, true) => "all already assigned or unavailable",
                    (true, false) => "all already assigned",
                    (false, true) => "all unavailable under current rules",
                    (false, false) => "no reviewable candidates",
                };
                return Some((
                    match semantic_debug {
                        Some(debug) => {
                            format!("semantic ran; no new review suggestions ({reason}) | {debug}")
                        }
                        None => format!("semantic ran; no new review suggestions ({reason})"),
                    },
                    false,
                ));
            }
            if result.semantic_candidates_queued_review == 0 {
                return Some((
                    match semantic_debug {
                        Some(debug) => {
                            format!("semantic ran; no new review suggestions | {debug}")
                        }
                        None => "semantic ran; no new review suggestions".to_string(),
                    },
                    false,
                ));
            }
        }

        if let Some(debug) = semantic_debug {
            return Some((debug, false));
        }

        None
    }

    pub(crate) fn selected_item_has_assignment(&self, category_id: CategoryId) -> bool {
        self.item_assign_anchor_id()
            .and_then(|item_id| self.all_items.iter().find(|item| item.id == item_id))
            .or_else(|| self.selected_item())
            .map(|item| item.assignments.contains_key(&category_id))
            .unwrap_or(false)
    }

    pub(crate) fn selected_item_assignment(&self, category_id: CategoryId) -> Option<&Assignment> {
        self.item_assign_anchor_id()
            .and_then(|item_id| self.all_items.iter().find(|item| item.id == item_id))
            .or_else(|| self.selected_item())
            .and_then(|item| item.assignments.get(&category_id))
    }

    fn summarize_implicit_string_rationale(rationale: &str) -> Option<String> {
        rationale
            .strip_prefix("matched category name '")
            .and_then(|tail| tail.strip_suffix('\''))
            .map(|term| format!("Matched category name \"{term}\""))
            .or_else(|| {
                rationale
                    .strip_prefix("matched also-match term '")
                    .and_then(|tail| tail.strip_suffix('\''))
                    .map(|term| format!("Matched alias \"{term}\""))
            })
    }

    pub(crate) fn assignment_badge_from_assignment(
        assignment: &Assignment,
    ) -> Option<&'static str> {
        match assignment.explanation.as_ref() {
            Some(AssignmentExplanation::ImplicitMatch { .. }) => Some("auto-match"),
            Some(AssignmentExplanation::ProfileCondition { .. }) => Some("profile"),
            Some(AssignmentExplanation::DateCondition { .. }) => Some("date"),
            Some(AssignmentExplanation::ConditionGroup { .. }) => Some("rules"),
            Some(AssignmentExplanation::Action { .. }) => Some("action"),
            Some(AssignmentExplanation::Subsumption { .. }) => Some("inherited"),
            Some(AssignmentExplanation::SuggestionAccepted { .. }) => Some("suggested"),
            Some(AssignmentExplanation::AutoClassified { provider_id, .. })
                if provider_id == PROVIDER_ID_IMPLICIT_STRING =>
            {
                Some("auto-match")
            }
            Some(AssignmentExplanation::AutoClassified { .. }) => Some("auto"),
            Some(AssignmentExplanation::Manual { .. }) | None => None,
        }
    }

    pub(crate) fn assignment_status_summary(assignment: &Assignment) -> String {
        match assignment.explanation.as_ref() {
            Some(AssignmentExplanation::AutoClassified {
                provider_id,
                rationale: Some(rationale),
                ..
            }) if provider_id == PROVIDER_ID_IMPLICIT_STRING => {
                Self::summarize_implicit_string_rationale(rationale)
                    .unwrap_or_else(|| assignment.explanation.as_ref().unwrap().summary())
            }
            Some(explanation) => explanation.summary(),
            None => format!("{:?}", assignment.source),
        }
    }

    pub(crate) fn selected_item_assignment_badge(
        &self,
        category_id: CategoryId,
    ) -> Option<&'static str> {
        let assignment = self.selected_item_assignment(category_id)?;
        Self::assignment_badge_from_assignment(assignment)
    }

    pub(crate) fn selected_item_has_actionable_assignment(&self) -> bool {
        let Some(item) = self.selected_item() else {
            return false;
        };
        item.assignments.keys().any(|category_id| {
            self.categories
                .iter()
                .find(|category| category.id == *category_id)
                .map(|category| category.is_actionable)
                .unwrap_or(false)
        })
    }

    pub(crate) fn inspect_assignment_rows_for_item(
        &self,
        item: &Item,
    ) -> Vec<InspectAssignmentRow> {
        let category_names = category_name_map(&self.categories);
        let mut rows: Vec<InspectAssignmentRow> = item
            .assignments
            .iter()
            .map(|(category_id, assignment)| InspectAssignmentRow {
                category_id: *category_id,
                category_name: category_names
                    .get(category_id)
                    .cloned()
                    .unwrap_or_else(|| category_id.to_string()),
                source_label: format!("{:?}", assignment.source),
                origin_label: assignment.origin.clone().unwrap_or_else(|| "-".to_string()),
                explanation_label: assignment
                    .explanation
                    .as_ref()
                    .map(|explanation| explanation.summary()),
            })
            .collect();
        rows.sort_by_key(|row| row.category_name.to_ascii_lowercase());
        rows
    }

    pub(crate) fn selected_item_id(&self) -> Option<ItemId> {
        self.selected_item().map(|item| item.id)
    }

    pub(crate) fn has_selected_items(&self) -> bool {
        !self.selected_item_ids.is_empty()
    }

    pub(crate) fn selected_count(&self) -> usize {
        self.selected_item_ids.len()
    }

    pub(crate) fn is_item_selected(&self, item_id: ItemId) -> bool {
        self.selected_item_ids.contains(&item_id)
    }

    pub(crate) fn toggle_selected_item(&mut self, item_id: ItemId) -> bool {
        if !self.selected_item_ids.insert(item_id) {
            self.selected_item_ids.remove(&item_id);
            false
        } else {
            true
        }
    }

    pub(crate) fn clear_selected_items(&mut self) -> usize {
        let count = self.selected_item_ids.len();
        self.selected_item_ids.clear();
        count
    }

    pub(crate) fn selected_item_ids_in_view_order(&self) -> Vec<ItemId> {
        let mut ordered = Vec::new();
        let mut seen = HashSet::new();
        for slot in &self.slots {
            for item in &slot.items {
                if self.selected_item_ids.contains(&item.id) && seen.insert(item.id) {
                    ordered.push(item.id);
                }
            }
        }
        ordered
    }

    pub(crate) fn item_assign_anchor_id(&self) -> Option<ItemId> {
        if matches!(self.mode, Mode::ItemAssignPicker | Mode::ItemAssignInput)
            && self.item_assign_anchor_item_id.is_some()
        {
            self.item_assign_anchor_item_id
        } else {
            None
        }
    }

    pub(crate) fn start_item_assign_session(&mut self) {
        let anchor_item_id = self.selected_item_id();
        let selected_item_ids = self.selected_item_ids_in_view_order();
        self.item_assign_anchor_item_id = anchor_item_id;
        self.item_assign_target_item_ids = if !selected_item_ids.is_empty() {
            selected_item_ids
        } else {
            anchor_item_id.into_iter().collect()
        };
    }

    pub(crate) fn clear_item_assign_session(&mut self) {
        self.item_assign_dirty = false;
        self.item_assign_anchor_item_id = None;
        self.item_assign_target_item_ids.clear();
        self.item_assign_preview = AssignmentPreview::default();
        self.clear_input();
    }

    pub(crate) fn close_item_assign_session(&mut self, status: impl Into<String>) {
        let status = status.into();
        let clear_selection = self.item_assign_dirty && self.has_selected_items();
        let return_target = self.item_assign_return_target.take();
        self.clear_item_assign_session();

        if clear_selection {
            self.clear_selected_items();
        }

        match return_target {
            Some(ItemAssignReturnTarget::EditPanel(panel)) => {
                if let Some(item_id) = panel.item_id {
                    self.set_item_selection_by_id(item_id);
                }
                self.input_panel = Some(panel);
                self.mode = Mode::InputPanel;
            }
            None => {
                self.mode = Mode::Normal;
            }
        }

        self.status = status;
    }

    pub(crate) fn effective_action_item_ids(&self) -> Vec<ItemId> {
        if matches!(self.mode, Mode::ItemAssignPicker | Mode::ItemAssignInput)
            && !self.item_assign_target_item_ids.is_empty()
        {
            return self.item_assign_target_item_ids.clone();
        }
        let selected = self.selected_item_ids_in_view_order();
        if !selected.is_empty() {
            selected
        } else {
            self.selected_item_id().into_iter().collect()
        }
    }

    pub(crate) fn effective_action_assignment_counts(
        &self,
        category_id: CategoryId,
    ) -> (usize, usize) {
        let action_item_ids = self.effective_action_item_ids();
        let assigned = action_item_ids
            .iter()
            .filter(|item_id| {
                self.all_items
                    .iter()
                    .find(|item| item.id == **item_id)
                    .is_some_and(|item| item.assignments.contains_key(&category_id))
            })
            .count();
        (assigned, action_item_ids.len())
    }

    pub(crate) fn item_assign_visible_category_row_indices(&self) -> Vec<usize> {
        let query = self.input.trimmed().to_ascii_lowercase();
        self.category_rows
            .iter()
            .enumerate()
            .filter(|(_, row)| {
                !row.is_reserved
                    && (query.is_empty() || row.name.to_ascii_lowercase().contains(&query))
            })
            .map(|(idx, _)| idx)
            .collect()
    }

    pub(crate) fn item_assign_selected_category_row_index(&self) -> Option<usize> {
        let visible_indices = self.item_assign_visible_category_row_indices();
        visible_indices
            .iter()
            .position(|row_index| *row_index == self.item_assign_category_index)
    }

    pub(crate) fn item_assign_selected_category_row(&self) -> Option<&CategoryListRow> {
        let row_index = self
            .item_assign_visible_category_row_indices()
            .get(self.item_assign_selected_category_row_index()?)
            .copied()?;
        self.category_rows.get(row_index)
    }

    pub(crate) fn clamp_item_assign_category_index(&mut self) {
        let visible_indices = self.item_assign_visible_category_row_indices();
        if visible_indices.is_empty() {
            self.item_assign_category_index = 0;
            return;
        }

        if let Some(visible_index) = visible_indices
            .iter()
            .position(|row_index| *row_index == self.item_assign_category_index)
        {
            self.item_assign_category_index = visible_indices[visible_index];
        } else {
            self.item_assign_category_index = visible_indices[0];
        }
    }

    pub(crate) fn set_item_assign_category_visible_selection(&mut self, visible_index: usize) {
        let visible_indices = self.item_assign_visible_category_row_indices();
        if visible_indices.is_empty() {
            self.item_assign_category_index = 0;
            return;
        }
        let next_visible = visible_index.min(visible_indices.len() - 1);
        self.item_assign_category_index = visible_indices[next_visible];
    }

    pub(crate) fn set_item_assign_category_selection_by_id(&mut self, category_id: CategoryId) {
        if let Some(row_index) = self
            .item_assign_visible_category_row_indices()
            .into_iter()
            .find(|row_index| {
                self.category_rows
                    .get(*row_index)
                    .map(|row| row.id == category_id)
                    .unwrap_or(false)
            })
        {
            self.item_assign_category_index = row_index;
        } else {
            self.clamp_item_assign_category_index();
        }
    }

    pub(crate) fn focused_numeric_board_column(&self) -> bool {
        match self.current_slot_sort_column() {
            Some(SlotSortColumn::SectionColumn {
                heading,
                kind: ColumnKind::Standard,
            }) => self
                .categories
                .iter()
                .find(|category| category.id == heading)
                .map(|category| category.value_kind == CategoryValueKind::Numeric)
                .unwrap_or(false),
            _ => false,
        }
    }

    /// Returns `(present_count, total_action_count)` — how many of the current
    /// action items appear in the specified view section (or the unmatched slot
    /// when `section_idx` is `None`).
    pub(crate) fn item_in_section_counts(
        &self,
        view_idx: usize,
        section_idx: Option<usize>,
    ) -> (usize, usize) {
        let action_ids = self.effective_action_item_ids();
        if action_ids.is_empty() {
            return (0, 0);
        }
        let Some(view) = self.views.get(view_idx) else {
            return (0, action_ids.len());
        };
        let reference_date = jiff::Zoned::now().date();
        let result = resolve_view(view, &self.all_items, &self.categories, reference_date);
        let present_ids: HashSet<ItemId> = match section_idx {
            Some(si) => result
                .sections
                .iter()
                .find(|s| s.section_index == si)
                .map(|s| {
                    let mut ids: HashSet<ItemId> = s.items.iter().map(|i| i.id).collect();
                    for sub in &s.subsections {
                        ids.extend(sub.items.iter().map(|i| i.id));
                    }
                    ids
                })
                .unwrap_or_default(),
            None => result
                .unmatched
                .as_deref()
                .unwrap_or_default()
                .iter()
                .map(|i| i.id)
                .collect(),
        };
        let present = action_ids
            .iter()
            .filter(|id| present_ids.contains(id))
            .count();
        (present, action_ids.len())
    }

    /// Returns the item's placement within a resolved view result.
    ///
    /// `None` means the item is absent from the view entirely.
    /// `Some(None)` means the item is present in the unmatched slot.
    /// `Some(Some(section_idx))` means the item is present in a concrete section.
    pub(crate) fn item_membership_in_view_result(
        item_id: ItemId,
        result: &agenda_core::query::ViewResult,
    ) -> Option<Option<usize>> {
        if let Some(section_idx) = result.sections.iter().find_map(|section| {
            let in_top = section.items.iter().any(|item| item.id == item_id);
            let in_sub = section
                .subsections
                .iter()
                .any(|subsection| subsection.items.iter().any(|item| item.id == item_id));
            if in_top || in_sub {
                Some(section.section_index)
            } else {
                None
            }
        }) {
            return Some(Some(section_idx));
        }

        if result
            .unmatched
            .as_ref()
            .is_some_and(|items| items.iter().any(|item| item.id == item_id))
        {
            return Some(None);
        }

        None
    }

    /// Recompute `item_assign_preview` based on the current pane focus and
    /// cursor position.  This is a pure read — nothing is mutated except the
    /// preview field itself.
    pub(crate) fn compute_assignment_preview(&mut self, agenda: &Agenda<'_>) {
        self.item_assign_preview = AssignmentPreview::default();

        match self.item_assign_pane {
            // ── Right pane hovered: show which categories would change ────────
            ItemAssignPane::ViewSection => {
                let Some(row) = self
                    .view_assign_rows
                    .get(self.item_assign_view_row_index)
                    .cloned()
                else {
                    return;
                };
                let ViewAssignRow::SectionRow {
                    view_idx,
                    section_idx,
                    ..
                } = row
                else {
                    return; // ViewHeader — no preview
                };
                let Some(view) = self.views.get(view_idx).cloned() else {
                    return;
                };
                let to_section = section_idx.and_then(|si| view.sections.get(si)).cloned();
                let reference_date = jiff::Zoned::now().date();
                let result = resolve_view(&view, &self.all_items, &self.categories, reference_date);

                // Union preview across all action items.
                let action_ids = self.effective_action_item_ids();
                for item_id in &action_ids {
                    let current_placement = Self::item_membership_in_view_result(*item_id, &result);

                    // Skip items already in the target slot — no change.
                    if current_placement == Some(section_idx) {
                        continue;
                    }

                    if current_placement.is_none() && section_idx.is_none() {
                        self.item_assign_preview
                            .cat_to_add
                            .extend(view.criteria.and_category_ids());
                        continue;
                    }

                    let from_section = current_placement
                        .and_then(|current_section_idx| current_section_idx)
                        .and_then(|current_section_idx| view.sections.get(current_section_idx));

                    let preview = agenda_core::agenda::Agenda::preview_section_move(
                        &view,
                        from_section,
                        to_section.as_ref(),
                    );
                    self.item_assign_preview
                        .cat_to_add
                        .extend(preview.to_assign);
                    self.item_assign_preview
                        .cat_to_remove
                        .extend(preview.to_unassign);
                }
            }

            // ── Left pane hovered: show which view slots would change ─────────
            ItemAssignPane::Categories => {
                let Some(row) = self.item_assign_selected_category_row().cloned() else {
                    return;
                };
                let action_ids = self.effective_action_item_ids();
                let reference_date = jiff::Zoned::now().date();

                for item_id in &action_ids {
                    let Some(item) = self.all_items.iter().find(|i| i.id == *item_id).cloned()
                    else {
                        continue;
                    };
                    let Ok(hypothetical) = agenda.preview_manual_category_toggle(*item_id, row.id)
                    else {
                        continue;
                    };
                    let current_assignments: HashSet<_> =
                        item.assignments.keys().copied().collect();
                    let hypothetical_assignments: HashSet<_> =
                        hypothetical.assignments.keys().copied().collect();

                    self.item_assign_preview.cat_to_add.extend(
                        hypothetical_assignments
                            .difference(&current_assignments)
                            .copied(),
                    );
                    self.item_assign_preview.cat_to_remove.extend(
                        current_assignments
                            .difference(&hypothetical_assignments)
                            .copied(),
                    );

                    // Build item lists: current and hypothetical.
                    let other_items: Vec<_> = self
                        .all_items
                        .iter()
                        .filter(|i| i.id != *item_id)
                        .cloned()
                        .collect();
                    let current_items: Vec<_> = other_items
                        .iter()
                        .cloned()
                        .chain(std::iter::once(item.clone()))
                        .collect();
                    let hyp_items: Vec<_> = other_items
                        .iter()
                        .cloned()
                        .chain(std::iter::once(hypothetical))
                        .collect();

                    for (view_idx, view) in self.views.iter().enumerate() {
                        let cur =
                            resolve_view(view, &current_items, &self.categories, reference_date);
                        let hyp = resolve_view(view, &hyp_items, &self.categories, reference_date);
                        let cur_section = Self::item_membership_in_view_result(*item_id, &cur);
                        let hyp_section = Self::item_membership_in_view_result(*item_id, &hyp);

                        if cur_section != hyp_section {
                            if let Some(slot) = cur_section {
                                self.item_assign_preview
                                    .section_to_lose
                                    .insert((view_idx, slot));
                            }
                            if let Some(slot) = hyp_section {
                                self.item_assign_preview
                                    .section_to_gain
                                    .insert((view_idx, slot));
                            }
                        }
                    }
                }
            }
        }
    }

    pub(crate) fn prune_selected_items_to_visible_slots(&mut self) {
        if self.selected_item_ids.is_empty() {
            return;
        }

        let visible_ids: HashSet<ItemId> = self
            .slots
            .iter()
            .flat_map(|slot| slot.items.iter().map(|item| item.id))
            .collect();
        self.selected_item_ids
            .retain(|item_id| visible_ids.contains(item_id));
    }

    pub(crate) fn is_item_blocked(&self, item_id: ItemId) -> bool {
        self.blocked_item_ids.contains(&item_id)
    }

    pub(crate) fn current_view(&self) -> Option<&View> {
        self.views.get(self.view_index)
    }

    /// Resolve the effective empty-sections display mode for the current view.
    pub(crate) fn effective_empty_sections(&self) -> EmptySections {
        self.current_view()
            .map(|view| view.empty_sections)
            .unwrap_or(EmptySections::Show)
    }

    pub(crate) fn selected_category_is_ready_queue_role(&self) -> bool {
        self.selected_category_row()
            .is_some_and(|row| self.workflow_config.ready_category_id == Some(row.id))
    }

    pub(crate) fn selected_category_is_claim_target_role(&self) -> bool {
        self.selected_category_row()
            .is_some_and(|row| self.workflow_config.claim_category_id == Some(row.id))
    }

    fn workflow_role_category_name(&self, category_id: Option<CategoryId>) -> Option<&str> {
        let category_id = category_id?;
        self.categories
            .iter()
            .find(|category| category.id == category_id)
            .map(|category| category.name.as_str())
    }

    pub(crate) fn ready_queue_header_hint(&self) -> Option<String> {
        let current_view = self.current_view()?;
        if !current_view
            .name
            .eq_ignore_ascii_case(READY_QUEUE_VIEW_NAME)
        {
            return None;
        }
        let ready_name =
            self.workflow_role_category_name(self.workflow_config.ready_category_id)?;
        let claim_name =
            self.workflow_role_category_name(self.workflow_config.claim_category_id)?;
        Some(format!("  workflow:{ready_name}->{claim_name}"))
    }

    pub(crate) fn effective_hide_dependent_items(&self) -> bool {
        self.session_hide_dependent_items_override
            .unwrap_or_else(|| {
                self.current_view()
                    .map(|view| view.hide_dependent_items)
                    .unwrap_or(false)
            })
    }

    pub(crate) fn set_active_view_index(&mut self, index: usize) {
        if self.views.is_empty() {
            self.view_index = 0;
            self.picker_index = 0;
            self.active_view_name = None;
            self.session_hide_dependent_items_override = None;
            self.selected_item_ids.clear();
            return;
        }

        let next_index = index.min(self.views.len().saturating_sub(1));
        let next_view_name = self.views.get(next_index).map(|view| view.name.clone());
        if self.active_view_name != next_view_name {
            self.session_hide_dependent_items_override = None;
            self.selected_item_ids.clear();
            self.board_scroll_offset = 0;
        }
        self.view_index = next_index;
        self.picker_index = next_index;
        self.active_view_name = next_view_name;
    }

    pub(crate) fn selected_category_row(&self) -> Option<&CategoryListRow> {
        self.category_rows.get(self.category_index)
    }

    pub(crate) fn selected_category_id(&self) -> Option<CategoryId> {
        self.selected_category_row().map(|row| row.id)
    }

    pub(crate) fn set_category_selection_by_id(&mut self, category_id: CategoryId) {
        if let Some(index) = self
            .category_rows
            .iter()
            .position(|row| row.id == category_id)
        {
            self.category_index = index;
        }
        self.sync_category_manager_state_from_selection();
    }

    pub(crate) fn open_category_manager_session(&mut self) {
        let selected_category_id = self.selected_category_id();
        let selected_category =
            selected_category_id.and_then(|id| self.categories.iter().find(|c| c.id == id));
        let initial_note = selected_category
            .and_then(|c| c.note.clone())
            .unwrap_or_default();
        let initial_also_match = selected_category
            .map(|c| c.also_match.join("\n"))
            .unwrap_or_default();
        let is_numeric = selected_category
            .map(|c| c.value_kind == CategoryValueKind::Numeric)
            .unwrap_or(false);
        let initial_details_focus = if is_numeric {
            CategoryManagerDetailsFocus::Integer
        } else {
            CategoryManagerDetailsFocus::Exclusive
        };
        self.category_manager = Some(CategoryManagerState {
            focus: CategoryManagerFocus::Tree,
            filter: text_buffer::TextBuffer::empty(),
            filter_editing: false,
            structure_move_prefix: None,
            discard_confirm: false,
            details_focus: initial_details_focus,
            details_note_category_id: selected_category_id,
            details_note: text_buffer::TextBuffer::new(initial_note),
            details_note_dirty: false,
            details_note_editing: false,
            details_also_match_category_id: selected_category_id,
            details_also_match: text_buffer::TextBuffer::new(initial_also_match),
            details_also_match_dirty: false,
            details_also_match_editing: false,
            details_inline_input: None,
            tree_index: self.category_index,
            visible_row_indices: Vec::new(),
            selected_category_id,
            inline_action: None,
            condition_edit: None,
            action_edit: None,
        });
        self.rebuild_category_manager_visible_rows();
    }

    pub(crate) fn close_category_manager_session(&mut self) {
        self.category_manager = None;
    }

    pub(crate) fn ensure_category_manager_session(&mut self) {
        if self.category_manager.is_none() {
            self.open_category_manager_session();
        } else {
            self.sync_category_manager_state_from_selection();
        }
    }

    pub(crate) fn sync_category_manager_state_from_selection(&mut self) {
        let selected_category_id = self.selected_category_id();
        let mut reload_details_for: Option<Option<CategoryId>> = None;
        let mut dropped_dirty_note = false;
        let mut dropped_dirty_also_match = false;
        if let Some(state) = &mut self.category_manager {
            state.selected_category_id = selected_category_id;
            if let Some(pos) = state.visible_row_indices.iter().position(|row_index| {
                self.category_rows.get(*row_index).map(|r| r.id) == selected_category_id
            }) {
                state.tree_index = pos;
            } else if state.visible_row_indices.is_empty() {
                state.tree_index = 0;
            } else {
                state.tree_index = state.tree_index.min(state.visible_row_indices.len() - 1);
            }
            if state.visible_row_indices.is_empty() {
                state.tree_index = 0;
            }

            if state.details_note_category_id != state.selected_category_id
                || state.details_also_match_category_id != state.selected_category_id
            {
                dropped_dirty_note = state.details_note_dirty;
                dropped_dirty_also_match = state.details_also_match_dirty;
                reload_details_for = Some(state.selected_category_id);
            }
        }

        if let Some(next_category_id) = reload_details_for {
            let next_cat =
                next_category_id.and_then(|id| self.categories.iter().find(|c| c.id == id));
            let next_note = next_cat.and_then(|c| c.note.clone()).unwrap_or_default();
            let next_also_match = next_cat
                .map(|c| c.also_match.join("\n"))
                .unwrap_or_default();
            let is_numeric = next_cat
                .map(|c| c.value_kind == CategoryValueKind::Numeric)
                .unwrap_or(false);
            if let Some(state) = &mut self.category_manager {
                state.details_note_category_id = next_category_id;
                state.details_note = text_buffer::TextBuffer::new(next_note);
                state.details_note_dirty = false;
                state.details_note_editing = false;
                state.details_also_match_category_id = next_category_id;
                state.details_also_match = text_buffer::TextBuffer::new(next_also_match);
                state.details_also_match_dirty = false;
                state.details_also_match_editing = false;
                state.details_inline_input = None;
                state.details_focus = if is_numeric {
                    CategoryManagerDetailsFocus::Integer
                } else {
                    CategoryManagerDetailsFocus::Exclusive
                };
            }
            if dropped_dirty_note || dropped_dirty_also_match {
                self.status =
                    "Discarded unsaved category detail draft after selection changed".to_string();
            }
        }
    }

    pub(crate) fn category_manager_visible_row_indices(&self) -> Option<&[usize]> {
        self.category_manager
            .as_ref()
            .map(|state| state.visible_row_indices.as_slice())
    }

    pub(crate) fn category_manager_visible_tree_index(&self) -> Option<usize> {
        self.category_manager.as_ref().map(|state| state.tree_index)
    }

    pub(crate) fn category_manager_inline_action(&self) -> Option<&CategoryInlineAction> {
        self.category_manager
            .as_ref()
            .and_then(|state| state.inline_action.as_ref())
    }

    pub(crate) fn set_category_manager_inline_action(
        &mut self,
        action: Option<CategoryInlineAction>,
    ) {
        if let Some(state) = &mut self.category_manager {
            state.inline_action = action;
        }
    }

    pub(crate) fn category_manager_structure_move_prefix(&self) -> Option<char> {
        self.category_manager
            .as_ref()
            .and_then(|state| state.structure_move_prefix)
    }

    pub(crate) fn set_category_manager_structure_move_prefix(&mut self, prefix: Option<char>) {
        if let Some(state) = &mut self.category_manager {
            state.structure_move_prefix = prefix;
        }
    }

    pub(crate) fn category_manager_filter_text(&self) -> Option<&str> {
        self.category_manager
            .as_ref()
            .map(|state| state.filter.text())
    }

    pub(crate) fn category_manager_filter_mut(&mut self) -> Option<&mut text_buffer::TextBuffer> {
        self.category_manager
            .as_mut()
            .map(|state| &mut state.filter)
    }

    pub(crate) fn category_manager_filter_editing(&self) -> bool {
        self.category_manager
            .as_ref()
            .map(|state| state.filter_editing)
            .unwrap_or(false)
    }

    pub(crate) fn set_category_manager_filter_editing(&mut self, editing: bool) {
        if let Some(state) = &mut self.category_manager {
            state.filter_editing = editing;
        }
    }

    pub(crate) fn category_manager_focus(&self) -> Option<CategoryManagerFocus> {
        self.category_manager.as_ref().map(|state| state.focus)
    }

    pub(crate) fn category_manager_discard_confirm(&self) -> bool {
        self.category_manager
            .as_ref()
            .map(|state| state.discard_confirm)
            .unwrap_or(false)
    }

    pub(crate) fn set_category_manager_discard_confirm(&mut self, discard_confirm: bool) {
        if let Some(state) = &mut self.category_manager {
            state.discard_confirm = discard_confirm;
        }
    }

    pub(crate) fn set_category_manager_focus(&mut self, focus: CategoryManagerFocus) {
        if let Some(state) = &mut self.category_manager {
            state.focus = focus;
            if focus != CategoryManagerFocus::Filter {
                state.filter_editing = false;
            }
            if focus != CategoryManagerFocus::Details {
                state.details_note_editing = false;
                state.details_also_match_editing = false;
            }
        }
    }

    pub(crate) fn category_manager_details_focus(&self) -> Option<CategoryManagerDetailsFocus> {
        self.category_manager
            .as_ref()
            .map(|state| state.details_focus)
    }

    pub(crate) fn cycle_category_manager_details_focus(&mut self, delta: i32) {
        let is_numeric = self
            .selected_category_row()
            .map(|row| row.value_kind == CategoryValueKind::Numeric)
            .unwrap_or(false);
        let integer_mode = self
            .selected_category_id()
            .and_then(|id| self.categories.iter().find(|c| c.id == id))
            .map(|c| c.numeric_format.clone().unwrap_or_default().decimal_places == 0)
            .unwrap_or(false);
        if let Some(state) = &mut self.category_manager {
            state.details_focus = match delta.signum() {
                d if d > 0 => state.details_focus.next(is_numeric, integer_mode),
                d if d < 0 => state.details_focus.prev(is_numeric, integer_mode),
                _ => state.details_focus,
            };
            state.details_note_editing = false;
            state.details_also_match_editing = false;
            state.details_inline_input = None;
        }
        self.normalize_category_manager_details_focus();
    }

    pub(crate) fn set_category_manager_details_focus(
        &mut self,
        focus: CategoryManagerDetailsFocus,
    ) {
        if let Some(state) = &mut self.category_manager {
            state.details_focus = focus;
            if focus != CategoryManagerDetailsFocus::Note {
                state.details_note_editing = false;
            }
            if focus != CategoryManagerDetailsFocus::AlsoMatch {
                state.details_also_match_editing = false;
            }
            let keep_inline = matches!(
                (&state.details_inline_input, focus),
                (
                    Some(CategoryManagerDetailsInlineInput {
                        field: CategoryManagerDetailsInlineField::DecimalPlaces,
                        ..
                    }),
                    CategoryManagerDetailsFocus::DecimalPlaces
                ) | (
                    Some(CategoryManagerDetailsInlineInput {
                        field: CategoryManagerDetailsInlineField::CurrencySymbol,
                        ..
                    }),
                    CategoryManagerDetailsFocus::CurrencySymbol
                )
            );
            if !keep_inline {
                state.details_inline_input = None;
            }
        }
        self.normalize_category_manager_details_focus();
    }

    pub(crate) fn category_manager_details_note_editing(&self) -> bool {
        self.category_manager
            .as_ref()
            .map(|state| state.details_note_editing)
            .unwrap_or(false)
    }

    pub(crate) fn set_category_manager_details_note_editing(&mut self, editing: bool) {
        if let Some(state) = &mut self.category_manager {
            state.details_note_editing = editing;
            if editing {
                state.details_focus = CategoryManagerDetailsFocus::Note;
                state.details_also_match_editing = false;
                state.details_inline_input = None;
            }
        }
    }

    pub(crate) fn category_manager_details_also_match_editing(&self) -> bool {
        self.category_manager
            .as_ref()
            .map(|state| state.details_also_match_editing)
            .unwrap_or(false)
    }

    pub(crate) fn set_category_manager_details_also_match_editing(&mut self, editing: bool) {
        if let Some(state) = &mut self.category_manager {
            state.details_also_match_editing = editing;
            if editing {
                state.details_focus = CategoryManagerDetailsFocus::AlsoMatch;
                state.details_note_editing = false;
                state.details_inline_input = None;
            }
        }
    }

    pub(crate) fn category_manager_details_inline_input(
        &self,
    ) -> Option<&CategoryManagerDetailsInlineInput> {
        self.category_manager
            .as_ref()
            .and_then(|state| state.details_inline_input.as_ref())
    }

    pub(crate) fn category_manager_details_inline_input_mut(
        &mut self,
    ) -> Option<&mut CategoryManagerDetailsInlineInput> {
        self.category_manager
            .as_mut()
            .and_then(|state| state.details_inline_input.as_mut())
    }

    pub(crate) fn set_category_manager_details_inline_input(
        &mut self,
        input: Option<CategoryManagerDetailsInlineInput>,
    ) {
        if let Some(state) = &mut self.category_manager {
            state.details_inline_input = input;
        }
    }

    pub(crate) fn category_manager_condition_edit(&self) -> Option<&ConditionEditState> {
        self.category_manager
            .as_ref()
            .and_then(|state| state.condition_edit.as_ref())
    }

    pub(crate) fn category_manager_condition_edit_mut(
        &mut self,
    ) -> Option<&mut ConditionEditState> {
        self.category_manager
            .as_mut()
            .and_then(|state| state.condition_edit.as_mut())
    }

    pub(crate) fn category_manager_action_edit(&self) -> Option<&ActionEditState> {
        self.category_manager
            .as_ref()
            .and_then(|state| state.action_edit.as_ref())
    }

    pub(crate) fn category_manager_action_edit_mut(&mut self) -> Option<&mut ActionEditState> {
        self.category_manager
            .as_mut()
            .and_then(|state| state.action_edit.as_mut())
    }

    pub(crate) fn selected_category_numeric_format(&self) -> Option<NumericFormat> {
        self.selected_category_id()
            .and_then(|id| self.categories.iter().find(|c| c.id == id))
            .map(|category| category.numeric_format.clone().unwrap_or_default())
    }

    pub(crate) fn normalize_category_manager_details_focus(&mut self) {
        let is_numeric = self
            .selected_category_row()
            .map(|row| row.value_kind == CategoryValueKind::Numeric)
            .unwrap_or(false);
        if !is_numeric {
            return;
        }
        let integer_mode = self
            .selected_category_numeric_format()
            .map(|fmt| fmt.decimal_places == 0)
            .unwrap_or(false);
        if integer_mode
            && self.category_manager_details_focus()
                == Some(CategoryManagerDetailsFocus::DecimalPlaces)
        {
            self.set_category_manager_details_focus(CategoryManagerDetailsFocus::Integer);
        }
    }

    pub(crate) fn category_manager_details_note_text(&self) -> Option<&str> {
        self.category_manager
            .as_ref()
            .map(|state| state.details_note.text())
    }

    pub(crate) fn category_manager_details_note_dirty(&self) -> bool {
        self.category_manager
            .as_ref()
            .map(|state| state.details_note_dirty)
            .unwrap_or(false)
    }

    pub(crate) fn category_manager_details_note_edit_mut(
        &mut self,
    ) -> Option<&mut text_buffer::TextBuffer> {
        self.category_manager
            .as_mut()
            .map(|state| &mut state.details_note)
    }

    pub(crate) fn mark_category_manager_details_note_dirty(&mut self, dirty: bool) {
        if let Some(state) = &mut self.category_manager {
            state.details_note_dirty = dirty;
        }
    }

    pub(crate) fn category_manager_details_also_match_text(&self) -> Option<&str> {
        self.category_manager
            .as_ref()
            .map(|state| state.details_also_match.text())
    }

    pub(crate) fn category_manager_details_also_match_dirty(&self) -> bool {
        self.category_manager
            .as_ref()
            .map(|state| state.details_also_match_dirty)
            .unwrap_or(false)
    }

    pub(crate) fn category_manager_details_also_match_edit_mut(
        &mut self,
    ) -> Option<&mut text_buffer::TextBuffer> {
        self.category_manager
            .as_mut()
            .map(|state| &mut state.details_also_match)
    }

    pub(crate) fn mark_category_manager_details_also_match_dirty(&mut self, dirty: bool) {
        if let Some(state) = &mut self.category_manager {
            state.details_also_match_dirty = dirty;
        }
    }

    pub(crate) fn reload_category_manager_details_note_from_selected(&mut self) {
        let selected_id = self.selected_category_id();
        let note = selected_id
            .and_then(|id| self.categories.iter().find(|c| c.id == id))
            .and_then(|c| c.note.clone())
            .unwrap_or_default();
        if let Some(state) = &mut self.category_manager {
            state.details_note_category_id = selected_id;
            state.details_note = text_buffer::TextBuffer::new(note);
            state.details_note_dirty = false;
            state.details_note_editing = false;
        }
    }

    pub(crate) fn reload_category_manager_details_also_match_from_selected(&mut self) {
        let selected_id = self.selected_category_id();
        let also_match = selected_id
            .and_then(|id| self.categories.iter().find(|c| c.id == id))
            .map(|c| c.also_match.join("\n"))
            .unwrap_or_default();
        if let Some(state) = &mut self.category_manager {
            state.details_also_match_category_id = selected_id;
            state.details_also_match = text_buffer::TextBuffer::new(also_match);
            state.details_also_match_dirty = false;
            state.details_also_match_editing = false;
        }
    }

    pub(crate) fn rebuild_category_manager_visible_rows(&mut self) {
        let Some(state) = &mut self.category_manager else {
            return;
        };
        let query = state.filter.trimmed().to_ascii_lowercase();
        let mut visible: Vec<usize> = if query.is_empty() {
            (0..self.category_rows.len()).collect()
        } else {
            self.category_rows
                .iter()
                .enumerate()
                .filter(|(_, row)| row.name.to_ascii_lowercase().contains(&query))
                .map(|(idx, _)| idx)
                .collect()
        };
        // Keep deterministic fallback and avoid stale indices after refresh.
        visible.retain(|idx| *idx < self.category_rows.len());
        state.visible_row_indices = visible;

        if state.visible_row_indices.is_empty() {
            state.tree_index = 0;
            return;
        }

        if let Some(selected_id) = state.selected_category_id {
            if let Some(pos) = state.visible_row_indices.iter().position(|row_index| {
                self.category_rows
                    .get(*row_index)
                    .map(|row| row.id == selected_id)
                    .unwrap_or(false)
            }) {
                state.tree_index = pos;
                self.category_index = state.visible_row_indices[pos];
                return;
            }
        }

        state.tree_index = state.tree_index.min(state.visible_row_indices.len() - 1);
        self.category_index = state.visible_row_indices[state.tree_index];
        let selected = self
            .category_rows
            .get(self.category_index)
            .map(|row| row.id);
        state.selected_category_id = selected;
    }

    pub(crate) fn set_category_manager_visible_selection(&mut self, visible_index: usize) {
        {
            let Some(state) = &mut self.category_manager else {
                return;
            };
            if state.visible_row_indices.is_empty() {
                state.tree_index = 0;
                return;
            }
            let next_visible = visible_index.min(state.visible_row_indices.len() - 1);
            state.tree_index = next_visible;
            self.category_index = state.visible_row_indices[next_visible];
            let selected = self
                .category_rows
                .get(self.category_index)
                .map(|row| row.id);
            state.selected_category_id = selected;
        }
        self.sync_category_manager_state_from_selection();
    }

    pub(crate) fn set_item_selection_by_id(&mut self, item_id: ItemId) {
        for (slot_index, slot) in self.slots.iter().enumerate() {
            if let Some(item_index) = slot.items.iter().position(|item| item.id == item_id) {
                self.slot_index = slot_index;
                self.item_index = item_index;
                if let Some(stored) = self.horizontal_slot_item_indices.get_mut(slot_index) {
                    *stored = item_index;
                }
                return;
            }
        }
    }

    pub(crate) fn reset_section_filters(&mut self) {
        self.section_filters = vec![None; self.slots.len()];
        self.search_buffer.clear();
    }

    pub(crate) fn set_view_selection_by_name(&mut self, view_name: &str) {
        if let Some(index) = self
            .views
            .iter()
            .position(|view| view.name.eq_ignore_ascii_case(view_name))
        {
            self.set_active_view_index(index);
        }
    }

    pub(crate) fn cycle_view(&mut self, delta: i32, agenda: &Agenda<'_>) -> TuiResult<()> {
        if self.views.is_empty() {
            self.status = "No views available".to_string();
            return Ok(());
        }
        let next_view_index = next_index(self.view_index, self.views.len(), delta);
        self.set_active_view_index(next_view_index);
        self.slot_index = 0;
        self.item_index = 0;
        self.slot_sort_keys.clear();
        self.refresh(agenda.store())?;
        self.reset_section_filters();
        let view_name = self
            .current_view()
            .map(|view| view.name.clone())
            .unwrap_or_else(|| "(none)".to_string());
        self.status = format!("Switched to view: {view_name} (press v then e to edit view)");
        Ok(())
    }

    pub(crate) fn jump_to_all_items_view(&mut self, agenda: &Agenda<'_>) -> TuiResult<()> {
        let Some(index) = self
            .views
            .iter()
            .position(|view| view.name.eq_ignore_ascii_case("All Items"))
        else {
            self.status = "All Items view not found".to_string();
            return Ok(());
        };
        self.set_active_view_index(index);
        self.slot_index = 0;
        self.item_index = 0;
        self.slot_sort_keys.clear();
        self.refresh(agenda.store())?;
        self.reset_section_filters();
        self.status = "Jumped to view: All Items".to_string();
        Ok(())
    }

    pub(crate) fn global_search_active(&self) -> bool {
        self.global_search_session.is_some()
    }

    pub(crate) fn begin_global_search_session(&mut self, agenda: &Agenda<'_>) -> TuiResult<()> {
        let Some(index) = self
            .views
            .iter()
            .position(|view| view.name.eq_ignore_ascii_case("All Items"))
        else {
            self.status = "All Items view not found".to_string();
            return Ok(());
        };

        if self.global_search_session.is_none() {
            self.global_search_session = Some(GlobalSearchSession {
                return_view_name: self.current_view().map(|view| view.name.clone()),
                return_slot_index: self.slot_index,
                return_item_index: self.item_index,
                return_column_index: self.column_index,
                return_section_filters: self.section_filters.clone(),
                return_slot_sort_keys: self.slot_sort_keys.clone(),
                return_search_text: self.search_buffer.text().to_string(),
            });
        }

        self.set_active_view_index(index);
        self.slot_index = 0;
        self.item_index = 0;
        self.column_index = 0;
        self.slot_sort_keys.clear();
        self.refresh(agenda.store())?;
        self.reset_section_filters();
        self.search_buffer.clear();
        self.mode = Mode::SearchBarFocused;
        let return_view_name = self
            .global_search_session
            .as_ref()
            .and_then(|session| session.return_view_name.as_deref())
            .unwrap_or("previous view");
        self.status = format!("Global search from {return_view_name}: All Items (Esc returns)");
        Ok(())
    }

    pub(crate) fn restore_global_search_session(&mut self, agenda: &Agenda<'_>) -> TuiResult<()> {
        let Some(session) = self.global_search_session.take() else {
            return Ok(());
        };

        let return_slot_index = session.return_slot_index;
        let return_item_index = session.return_item_index;
        let return_column_index = session.return_column_index;
        let return_search_text = session.return_search_text;
        let return_filters = session.return_section_filters;
        let return_sort_keys = session.return_slot_sort_keys;

        let restored_view = session
            .return_view_name
            .as_deref()
            .and_then(|name| {
                self.views
                    .iter()
                    .position(|view| view.name.eq_ignore_ascii_case(name))
            })
            .map(|index| {
                self.set_active_view_index(index);
                true
            })
            .unwrap_or(false);

        self.slot_index = return_slot_index;
        self.item_index = return_item_index;
        self.column_index = return_column_index;
        self.refresh(agenda.store())?;

        let mut restored_sort_keys = vec![Vec::new(); self.slots.len()];
        for (index, keys) in return_sort_keys.into_iter().enumerate() {
            if let Some(slot_keys) = restored_sort_keys.get_mut(index) {
                *slot_keys = keys;
            }
        }
        self.slot_sort_keys = restored_sort_keys;

        let mut restored_filters = vec![None; self.slots.len()];
        for (index, filter) in return_filters.into_iter().enumerate() {
            if let Some(slot_filter) = restored_filters.get_mut(index) {
                *slot_filter = filter;
            }
        }
        self.section_filters = restored_filters;
        self.search_buffer.set(return_search_text);
        self.refresh(agenda.store())?;

        self.slot_index = return_slot_index.min(self.slots.len().saturating_sub(1));
        self.item_index = return_item_index.min(
            self.current_slot()
                .map(|slot| slot.items.len().saturating_sub(1))
                .unwrap_or(0),
        );
        self.column_index = return_column_index.min(self.current_slot_column_count());
        self.mode = Mode::Normal;
        self.status = if restored_view {
            "Returned to previous view".to_string()
        } else {
            "Previous view not found; staying on current view".to_string()
        };
        Ok(())
    }

    pub(crate) fn slot_sort_key_is_valid_for_slot(
        &self,
        view: Option<&View>,
        slot: &Slot,
        key: &SlotSortKey,
    ) -> bool {
        match key.column {
            SlotSortColumn::ItemText => true,
            SlotSortColumn::SectionColumn { heading, kind } => {
                let Some(view) = view else {
                    return false;
                };
                let section_index = match slot.context {
                    SlotContext::Section { section_index }
                    | SlotContext::GeneratedSection { section_index, .. } => section_index,
                    SlotContext::Unmatched => return false,
                };
                view.sections
                    .get(section_index)
                    .map(|section| {
                        section
                            .columns
                            .iter()
                            .any(|column| column.heading == heading && column.kind == kind)
                    })
                    .unwrap_or(false)
            }
        }
    }

    pub(crate) fn sort_slot_items(&self, slot: &mut Slot, sort_keys: &[SlotSortKey]) {
        if sort_keys.is_empty() || slot.items.len() < 2 {
            return;
        }
        slot.items
            .sort_by(|left, right| self.compare_items_for_sort_keys(left, right, sort_keys));
    }

    fn compare_items_for_sort_keys(
        &self,
        left: &Item,
        right: &Item,
        sort_keys: &[SlotSortKey],
    ) -> Ordering {
        for key in sort_keys {
            let ord = self.compare_items_for_sort_key(left, right, key);
            if ord != Ordering::Equal {
                return ord;
            }
        }
        Ordering::Equal
    }

    fn compare_items_for_sort_key(&self, left: &Item, right: &Item, key: &SlotSortKey) -> Ordering {
        match key.column {
            SlotSortColumn::ItemText => {
                let left_text = left.text.to_ascii_lowercase();
                let right_text = right.text.to_ascii_lowercase();
                self.compare_some_values(left_text, right_text, key.direction)
            }
            SlotSortColumn::SectionColumn { heading, kind } => match kind {
                ColumnKind::When => {
                    self.compare_optional_values(left.when_date, right.when_date, key.direction)
                }
                ColumnKind::Standard => {
                    let heading_category = self
                        .categories
                        .iter()
                        .find(|category| category.id == heading);
                    if heading_category
                        .map(|category| category.value_kind == CategoryValueKind::Numeric)
                        .unwrap_or(false)
                    {
                        let left_value = left
                            .assignments
                            .get(&heading)
                            .and_then(|assignment| assignment.numeric_value);
                        let right_value = right
                            .assignments
                            .get(&heading)
                            .and_then(|assignment| assignment.numeric_value);
                        self.compare_optional_values(left_value, right_value, key.direction)
                    } else if let Some(category) = heading_category {
                        let left_value = self.standard_sort_value_for_heading(left, category);
                        let right_value = self.standard_sort_value_for_heading(right, category);
                        self.compare_optional_values(left_value, right_value, key.direction)
                    } else {
                        Ordering::Equal
                    }
                }
            },
        }
    }

    fn standard_sort_value_for_heading(&self, item: &Item, heading: &Category) -> Option<String> {
        let mut values: Vec<String> = heading
            .children
            .iter()
            .filter(|child_id| item.assignments.contains_key(child_id))
            .map(|child_id| {
                self.categories
                    .iter()
                    .find(|category| category.id == *child_id)
                    .map(|category| category.name.clone())
                    .unwrap_or_else(|| child_id.to_string())
            })
            .collect();
        if values.is_empty() {
            return None;
        }
        values.sort_by_key(|value| value.to_ascii_lowercase());
        Some(values.join(", ").to_ascii_lowercase())
    }

    fn compare_some_values<T: Ord>(
        &self,
        left: T,
        right: T,
        direction: SlotSortDirection,
    ) -> Ordering {
        match direction {
            SlotSortDirection::Asc => left.cmp(&right),
            SlotSortDirection::Desc => right.cmp(&left),
        }
    }

    fn compare_optional_values<T: Ord>(
        &self,
        left: Option<T>,
        right: Option<T>,
        direction: SlotSortDirection,
    ) -> Ordering {
        match (left, right) {
            (None, None) => Ordering::Equal,
            (None, Some(_)) => Ordering::Greater,
            (Some(_), None) => Ordering::Less,
            (Some(left), Some(right)) => self.compare_some_values(left, right, direction),
        }
    }

    /// Suspend the TUI, open an external editor for the given InputPanel buffer,
    /// and resume the TUI with the edited content.
    fn run_external_editor(
        &mut self,
        terminal: &mut TuiTerminal,
        target: ExternalEditorTarget,
    ) -> TuiResult<()> {
        let Some(panel) = &self.input_panel else {
            return Ok(());
        };

        let content = match target {
            ExternalEditorTarget::Text => panel.text.text().to_string(),
            ExternalEditorTarget::Note => panel.note.text().to_string(),
        };

        let editor = std::env::var("VISUAL")
            .or_else(|_| std::env::var("EDITOR"))
            .unwrap_or_else(|_| "vi".to_string());
        let (command, args) = match parse_external_editor_command(&editor) {
            Ok(parts) => parts,
            Err(err) => {
                self.status = err;
                return Ok(());
            }
        };

        // Write content to a temporary file.
        let suffix = match target {
            ExternalEditorTarget::Text => ".txt",
            ExternalEditorTarget::Note => ".md",
        };
        let mut tmp = tempfile::Builder::new()
            .prefix("aglet-")
            .suffix(suffix)
            .tempfile()
            .map_err(|e| format!("Failed to create temp file: {e}"))?;
        tmp.write_all(content.as_bytes())
            .map_err(|e| format!("Failed to write temp file: {e}"))?;
        tmp.flush()
            .map_err(|e| format!("Failed to flush temp file: {e}"))?;
        let tmp_path = tmp.path().to_path_buf();

        // Suspend the TUI.
        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;

        // Spawn the editor using shell-style parsing so quoted paths/args work.
        let status = std::process::Command::new(&command)
            .args(&args)
            .arg(&tmp_path)
            .status();

        // Resume the TUI regardless of editor outcome.
        enable_raw_mode()?;
        execute!(terminal.backend_mut(), EnterAlternateScreen)?;
        TerminalSession::try_apply_preferred_cursor_style(terminal.backend_mut());
        terminal.clear()?;

        match status {
            Ok(exit_status) if exit_status.success() => {
                let new_content = std::fs::read_to_string(&tmp_path)
                    .map_err(|e| format!("Failed to read back temp file: {e}"))?;
                // Strip a single trailing newline that editors typically add.
                let new_content = new_content.strip_suffix('\n').unwrap_or(&new_content);
                if let Some(panel) = &mut self.input_panel {
                    match target {
                        ExternalEditorTarget::Text => panel.text.set(new_content.to_string()),
                        ExternalEditorTarget::Note => panel.note.set(new_content.to_string()),
                    }
                }
                let field = match target {
                    ExternalEditorTarget::Text => "text",
                    ExternalEditorTarget::Note => "note",
                };
                self.status = format!("Updated {field} from $EDITOR");
            }
            Ok(_) => {
                self.status = format!("Editor ({editor}) exited with error; content unchanged");
            }
            Err(e) => {
                self.status = format!("Failed to launch editor '{editor}': {e}");
            }
        }

        Ok(())
    }
}
