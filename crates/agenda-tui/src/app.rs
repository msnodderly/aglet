use crate::*;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::time::{Duration, Instant};

use agenda_core::store::DEFAULT_VIEW_NAME;
use agenda_core::workflow::{
    build_ready_queue_view, claimable_item_ids, resolve_workflow_config, READY_QUEUE_VIEW_NAME,
};

impl App {
    const AUTO_REFRESH_STATUS_TTL: Duration = Duration::from_millis(2_000);
    const AUTO_REFRESH_SETTING_KEY: &'static str = "tui.auto_refresh_interval";
    const LAST_VIEW_NAME_SETTING_KEY: &'static str = "tui.last_view_name";

    fn is_auto_refresh_cycle_key(key: KeyEvent) -> bool {
        key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('r') | KeyCode::Char('R'))
    }

    pub(crate) fn active_transient_status_text(&self) -> Option<&str> {
        self.transient_status.as_ref().and_then(|transient| {
            if Instant::now() < transient.expires_at {
                Some(transient.message.as_str())
            } else {
                None
            }
        })
    }

    pub(crate) fn clear_expired_transient_status(&mut self) {
        if self
            .transient_status
            .as_ref()
            .is_some_and(|transient| Instant::now() >= transient.expires_at)
        {
            self.transient_status = None;
        }
    }

    pub(crate) fn clear_transient_status_on_key(&mut self, key: KeyEvent) {
        if self.transient_status.is_some() && !Self::is_auto_refresh_cycle_key(key) {
            self.transient_status = None;
        }
    }

    fn should_auto_refresh_now(&self) -> bool {
        let Some(interval) = self.auto_refresh_interval.as_duration() else {
            return false;
        };
        self.mode == Mode::Normal && self.auto_refresh_last_tick.elapsed() >= interval
    }

    pub(crate) fn auto_refresh_mode_label(&self) -> &'static str {
        self.auto_refresh_interval.label()
    }

    pub(crate) fn cycle_auto_refresh_interval(&mut self) {
        self.auto_refresh_interval = self.auto_refresh_interval.next();
        self.auto_refresh_last_tick = Instant::now();
        self.transient_status = Some(TransientStatus {
            message: format!("Auto-refresh interval: {}", self.auto_refresh_mode_label()),
            expires_at: Instant::now() + Self::AUTO_REFRESH_STATUS_TTL,
        });
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

    pub(crate) fn persist_auto_refresh_interval(&self, store: &Store) -> TuiResult<()> {
        store.set_app_setting(
            Self::AUTO_REFRESH_SETTING_KEY,
            self.auto_refresh_interval.persisted_value(),
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

    pub(crate) fn maybe_run_auto_refresh(&mut self, store: &Store) -> TuiResult<()> {
        if self.should_auto_refresh_now() {
            self.refresh(store)?;
            self.auto_refresh_last_tick = Instant::now();
        }
        Ok(())
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
        self.refresh(agenda.store())?;
        self.load_last_view_name(agenda.store())?;
        self.refresh(agenda.store())?; // re-resolve slots for the restored view
        self.load_auto_refresh_interval(agenda.store())?;
        self.auto_refresh_last_tick = Instant::now();

        loop {
            self.clear_expired_transient_status();
            terminal.draw(|frame| self.draw(frame))?;

            if !event::poll(std::time::Duration::from_millis(200))? {
                self.maybe_run_auto_refresh(agenda.store())?;
                continue;
            }

            let Event::Key(key) = event::read()? else {
                continue;
            };
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
            if should_quit {
                let _ = self.persist_last_view_name(agenda.store());
                break;
            }

            self.maybe_run_auto_refresh(agenda.store())?;
        }

        Ok(())
    }

    pub(crate) fn refresh(&mut self, store: &Store) -> TuiResult<()> {
        self.views = store.list_views()?;
        self.workflow_config = store.get_workflow_config()?;
        if let Some(workflow) = resolve_workflow_config(store)? {
            let ready_queue_view = build_ready_queue_view(store, workflow)?;
            let insert_at = self
                .views
                .iter()
                .position(|view| view.name.eq_ignore_ascii_case(DEFAULT_VIEW_NAME))
                .map(|index| index + 1)
                .unwrap_or(0)
                .min(self.views.len());
            self.views.insert(insert_at, ready_queue_view);
        }
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

        let mut slots = Vec::new();
        if self.views.is_empty() {
            slots.push(Slot {
                title: "All Items (no views configured)".to_string(),
                items: items.clone(),
                context: SlotContext::Unmatched,
            });
            if self.mode == Mode::Normal {
                self.status = "No views configured; showing fallback item list".to_string();
            }
            self.set_active_view_index(0);
        } else {
            self.set_active_view_index(self.view_index);
            let view = self
                .current_view()
                .cloned()
                .ok_or("No active view".to_string())?;
            let reference_date = jiff::Zoned::now().date();
            let view_items = if view.name.eq_ignore_ascii_case(READY_QUEUE_VIEW_NAME) {
                if let Some(workflow) = resolve_workflow_config(store)? {
                    let claimable_ids = claimable_item_ids(store, &items, workflow)?;
                    items
                        .iter()
                        .filter(|item| claimable_ids.contains(&item.id))
                        .cloned()
                        .collect()
                } else {
                    Vec::new()
                }
            } else {
                items.clone()
            };
            let mut result = resolve_view(&view, &view_items, &self.categories, reference_date);
            if self.effective_hide_dependent_items() {
                for section in &mut result.sections {
                    section.items.retain(|item| !self.is_item_blocked(item.id));
                    for subsection in &mut section.subsections {
                        subsection
                            .items
                            .retain(|item| !self.is_item_blocked(item.id));
                    }
                }
                if let Some(unmatched_items) = &mut result.unmatched {
                    unmatched_items.retain(|item| !self.is_item_blocked(item.id));
                }
            }

            for section in result.sections {
                if section.subsections.is_empty() {
                    slots.push(Slot {
                        title: section.title,
                        items: section.items,
                        context: SlotContext::Section {
                            section_index: section.section_index,
                        },
                    });
                    continue;
                }

                for subsection in section.subsections {
                    slots.push(Slot {
                        title: format!("{} / {}", section.title, subsection.title),
                        items: subsection.items,
                        context: SlotContext::GeneratedSection {
                            section_index: section.section_index,
                            on_insert_assign: subsection.on_insert_assign,
                            on_remove_unassign: subsection.on_remove_unassign,
                        },
                    });
                }
            }

            if let Some(unmatched_items) = result.unmatched {
                if should_render_unmatched_lane(&unmatched_items) {
                    slots.push(Slot {
                        title: result
                            .unmatched_label
                            .unwrap_or_else(|| "Unassigned".to_string()),
                        items: unmatched_items,
                        context: SlotContext::Unmatched,
                    });
                }
            }

            if slots.is_empty() {
                slots.push(Slot {
                    title: "No visible sections".to_string(),
                    items: Vec::new(),
                    context: SlotContext::Unmatched,
                });
            }
        }

        // Resize per-slot state to match slot count (resets if structure changed)
        if self.section_filters.len() != slots.len() {
            self.section_filters = vec![None; slots.len()];
            self.search_buffer.clear();
        }
        if self.slot_sort_keys.len() != slots.len() {
            self.slot_sort_keys = vec![Vec::new(); slots.len()];
        }

        let active_view = self.current_view().cloned();
        let category_names_lower_ascii: HashMap<CategoryId, String> = self
            .categories
            .iter()
            .map(|category| (category.id, category.name.to_ascii_lowercase()))
            .collect();

        // Apply per-slot filters and sorting.
        for (slot_index, (slot, filter)) in slots
            .iter_mut()
            .zip(self.section_filters.iter())
            .enumerate()
        {
            if let Some(needle) = filter {
                let needle = needle.to_ascii_lowercase();
                slot.items
                    .retain(|item| item_text_matches(item, &needle, &category_names_lower_ascii));
            }

            let mut sort_keys = self.slot_sort_keys[slot_index].clone();
            sort_keys.retain(|key| {
                self.slot_sort_key_is_valid_for_slot(active_view.as_ref(), slot, key)
            });
            if sort_keys != self.slot_sort_keys[slot_index] {
                self.slot_sort_keys[slot_index] = sort_keys.clone();
            }
            if !sort_keys.is_empty() {
                self.sort_slot_items(slot, &sort_keys);
            }
        }

        self.slots = slots;
        self.prune_selected_items_to_visible_slots();
        self.clamp_horizontal_slot_item_indices();
        self.clamp_horizontal_slot_scroll_offsets();
        self.slot_index = self.slot_index.min(self.slots.len().saturating_sub(1));
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
        let previous_item_id = self
            .classification_ui
            .review_items
            .get(self.classification_ui.selected_item_index)
            .map(|item| item.item_id);
        let previous_suggestion_id = self
            .selected_classification_suggestion()
            .map(|suggestion| suggestion.id);

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

        self.classification_ui.pending_count =
            review_items.iter().map(|item| item.suggestions.len()).sum();
        self.classification_ui.config = config;
        self.classification_ui.review_items = review_items;

        if let Some(item_id) = previous_item_id {
            if let Some(index) = self
                .classification_ui
                .review_items
                .iter()
                .position(|item| item.item_id == item_id)
            {
                self.classification_ui.selected_item_index = index;
            }
        }
        self.classification_ui.selected_item_index = self
            .classification_ui
            .selected_item_index
            .min(self.classification_ui.review_items.len().saturating_sub(1));

        let suggestion_len = self
            .selected_classification_item()
            .map(|item| item.suggestions.len())
            .unwrap_or(0);
        if let Some(suggestion_id) = previous_suggestion_id {
            if let Some(index) = self
                .selected_classification_item()
                .and_then(|item| item.suggestions.iter().position(|s| s.id == suggestion_id))
            {
                self.classification_ui.selected_suggestion_index = index;
            }
        }
        self.classification_ui.selected_suggestion_index = self
            .classification_ui
            .selected_suggestion_index
            .min(suggestion_len.saturating_sub(1));
        if self.classification_ui.pending_count == 0
            && self.classification_ui.focus == ClassificationFocus::Suggestions
        {
            self.classification_ui.focus = ClassificationFocus::Items;
        }

        Ok(())
    }

    pub(crate) fn move_slot_cursor(&mut self, delta: i32) {
        if self.slots.is_empty() {
            return;
        }
        if self.is_horizontal_section_flow() {
            if let Some(stored) = self.horizontal_slot_item_indices.get_mut(self.slot_index) {
                *stored = self.item_index;
            }
            self.slot_index = next_index_clamped(self.slot_index, self.slots.len(), delta);
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
        let view = self
            .current_view()
            .cloned()
            .ok_or("No active view".to_string())?;

        self.remove_from_context(agenda, item_id, &view, &from_context)?;
        self.insert_into_context(agenda, item_id, &view, &to_context)?;

        self.slot_index = to_index;
        self.item_index = 0;
        self.refresh(agenda.store())?;
        self.status = "Moved item to new section".to_string();
        Ok(())
    }

    pub(crate) fn remove_from_context(
        &self,
        agenda: &Agenda<'_>,
        item_id: ItemId,
        view: &View,
        context: &SlotContext,
    ) -> TuiResult<()> {
        match context {
            SlotContext::Section { section_index } => {
                let section = view
                    .sections
                    .get(*section_index)
                    .ok_or("Section not found".to_string())?;
                agenda.remove_item_from_section(item_id, section)?;
            }
            SlotContext::GeneratedSection {
                section_index,
                on_insert_assign: _,
                on_remove_unassign,
            } => {
                let mut temp = view
                    .sections
                    .get(*section_index)
                    .cloned()
                    .ok_or("Section not found".to_string())?;
                temp.on_remove_unassign = on_remove_unassign.clone();
                agenda.remove_item_from_section(item_id, &temp)?;
            }
            SlotContext::Unmatched => {
                agenda.remove_item_from_unmatched(item_id, view)?;
            }
        }
        Ok(())
    }

    pub(crate) fn insert_into_context(
        &self,
        agenda: &Agenda<'_>,
        item_id: ItemId,
        view: &View,
        context: &SlotContext,
    ) -> TuiResult<()> {
        match context {
            SlotContext::Section { section_index } => {
                let section = view
                    .sections
                    .get(*section_index)
                    .ok_or("Section not found".to_string())?;
                agenda.insert_item_in_section(item_id, view, section)?;
            }
            SlotContext::GeneratedSection {
                section_index,
                on_insert_assign,
                on_remove_unassign,
            } => {
                let mut temp = view
                    .sections
                    .get(*section_index)
                    .cloned()
                    .ok_or("Section not found".to_string())?;
                temp.on_insert_assign = on_insert_assign.clone();
                temp.on_remove_unassign = on_remove_unassign.clone();
                agenda.insert_item_in_section(item_id, view, &temp)?;
            }
            SlotContext::Unmatched => {
                agenda.insert_item_in_unmatched(item_id, view)?;
            }
        }
        Ok(())
    }

    pub(crate) fn current_slot(&self) -> Option<&Slot> {
        self.slots.get(self.slot_index)
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

    pub(crate) fn classification_pending_count(&self) -> usize {
        self.classification_ui.pending_count
    }

    pub(crate) fn pending_suggestion_count_for_item(&self, item_id: ItemId) -> usize {
        self.classification_ui
            .review_items
            .iter()
            .find(|item| item.item_id == item_id)
            .map(|item| item.suggestions.len())
            .unwrap_or(0)
    }

    pub(crate) fn classification_pending_suffix(&self) -> Option<String> {
        if self.classification_ui.pending_count == 0 {
            None
        } else {
            Some(format!(
                "{} classification suggestion{} pending",
                self.classification_ui.pending_count,
                if self.classification_ui.pending_count == 1 {
                    ""
                } else {
                    "s"
                }
            ))
        }
    }

    pub(crate) fn selected_classification_item(&self) -> Option<&ClassificationReviewItem> {
        self.classification_ui
            .review_items
            .get(self.classification_ui.selected_item_index)
    }

    pub(crate) fn selected_classification_suggestion(&self) -> Option<&ClassificationSuggestion> {
        self.selected_classification_item().and_then(|item| {
            item.suggestions
                .get(self.classification_ui.selected_suggestion_index)
        })
    }

    pub(crate) fn selected_item_has_assignment(&self, category_id: CategoryId) -> bool {
        self.selected_item()
            .map(|item| item.assignments.contains_key(&category_id))
            .unwrap_or(false)
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

    pub(crate) fn effective_action_item_ids(&self) -> Vec<ItemId> {
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
        self.item_links_by_item_id
            .get(&item_id)
            .map(|links| {
                links
                    .depends_on
                    .iter()
                    .any(|dep_id| !self.all_items.iter().any(|i| i.id == *dep_id && i.is_done))
            })
            .unwrap_or(false)
    }

    pub(crate) fn current_view(&self) -> Option<&View> {
        self.views.get(self.view_index)
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
        let initial_note = selected_category_id
            .and_then(|id| self.categories.iter().find(|c| c.id == id))
            .and_then(|c| c.note.clone())
            .unwrap_or_default();
        let is_numeric = selected_category_id
            .and_then(|id| self.categories.iter().find(|c| c.id == id))
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
            details_inline_input: None,
            tree_index: self.category_index,
            visible_row_indices: Vec::new(),
            selected_category_id,
            inline_action: None,
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

            if state.details_note_category_id != state.selected_category_id {
                dropped_dirty_note = state.details_note_dirty;
                reload_details_for = Some(state.selected_category_id);
            }
        }

        if let Some(next_category_id) = reload_details_for {
            let next_cat =
                next_category_id.and_then(|id| self.categories.iter().find(|c| c.id == id));
            let next_note = next_cat.and_then(|c| c.note.clone()).unwrap_or_default();
            let is_numeric = next_cat
                .map(|c| c.value_kind == CategoryValueKind::Numeric)
                .unwrap_or(false);
            if let Some(state) = &mut self.category_manager {
                state.details_note_category_id = next_category_id;
                state.details_note = text_buffer::TextBuffer::new(next_note);
                state.details_note_dirty = false;
                state.details_note_editing = false;
                state.details_inline_input = None;
                state.details_focus = if is_numeric {
                    CategoryManagerDetailsFocus::Integer
                } else {
                    CategoryManagerDetailsFocus::Exclusive
                };
            }
            if dropped_dirty_note {
                self.status =
                    "Discarded unsaved category note draft after selection changed".to_string();
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

    pub(crate) fn cycle_category_manager_focus(&mut self, delta: i32) {
        let Some(current) = self.category_manager_focus() else {
            return;
        };
        let order = [
            CategoryManagerFocus::Filter,
            CategoryManagerFocus::Tree,
            CategoryManagerFocus::Details,
        ];
        let current_index = order
            .iter()
            .position(|focus| *focus == current)
            .unwrap_or(1);
        let next = order[next_index(current_index, order.len(), delta)];
        self.set_category_manager_focus(next);
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
        self.status = "Global search: All Items (Esc returns to previous view)".to_string();
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

    fn sort_slot_items(&self, slot: &mut Slot, sort_keys: &[SlotSortKey]) {
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
}
