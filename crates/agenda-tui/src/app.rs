use crate::*;
use std::cmp::Ordering;

impl App {
    pub(crate) fn run(
        &mut self,
        terminal: &mut TuiTerminal,
        agenda: &Agenda<'_>,
    ) -> Result<(), String> {
        self.refresh(agenda.store())?;

        loop {
            terminal
                .draw(|frame| self.draw(frame))
                .map_err(|e| e.to_string())?;

            if !event::poll(std::time::Duration::from_millis(200)).map_err(|e| e.to_string())? {
                continue;
            }

            let Event::Key(key) = event::read().map_err(|e| e.to_string())? else {
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
                break;
            }
        }

        Ok(())
    }

    pub(crate) fn refresh(&mut self, store: &Store) -> Result<(), String> {
        self.views = store.list_views().map_err(|e| e.to_string())?;
        self.categories = store.get_hierarchy().map_err(|e| e.to_string())?;
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
        let items = store.list_items().map_err(|e| e.to_string())?;
        self.all_items = items.clone();
        self.item_links_by_item_id.clear();
        for item in &items {
            let links = agenda_core::model::ItemLinksForItem {
                depends_on: store
                    .list_dependency_ids_for_item(item.id)
                    .map_err(|e| e.to_string())?,
                blocks: store
                    .list_dependent_ids_for_item(item.id)
                    .map_err(|e| e.to_string())?,
                related: store
                    .list_related_ids_for_item(item.id)
                    .map_err(|e| e.to_string())?,
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
            self.view_index = 0;
            self.picker_index = 0;
        } else {
            self.view_index = self.view_index.min(self.views.len().saturating_sub(1));
            let view = self
                .current_view()
                .cloned()
                .ok_or("No active view".to_string())?;
            let reference_date = Local::now().date_naive();
            let result = resolve_view(&view, &items, &self.categories, reference_date);

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

        // Apply per-slot filters and sorting.
        for (slot_index, (slot, filter)) in slots
            .iter_mut()
            .zip(self.section_filters.iter())
            .enumerate()
        {
            if let Some(needle) = filter {
                let needle = needle.to_ascii_lowercase();
                slot.items.retain(|item| item_text_matches(item, &needle));
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
        self.slot_index = self.slot_index.min(self.slots.len().saturating_sub(1));
        self.item_index = self.item_index.min(
            self.current_slot()
                .map(|slot| slot.items.len().saturating_sub(1))
                .unwrap_or(0),
        );
        let provenance_len = self
            .selected_item()
            .map(|item| self.inspect_assignment_rows_for_item(item).len())
            .unwrap_or(0);
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

        Ok(())
    }

    pub(crate) fn move_slot_cursor(&mut self, delta: i32) {
        if self.slots.is_empty() {
            return;
        }
        self.slot_index = next_index_clamped(self.slot_index, self.slots.len(), delta);
        self.item_index = self.item_index.min(
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
            return;
        }
        self.item_index = next_index_clamped(self.item_index, slot.items.len(), delta);
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
    ) -> Result<(), String> {
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
    ) -> Result<(), String> {
        match context {
            SlotContext::Section { section_index } => {
                let section = view
                    .sections
                    .get(*section_index)
                    .ok_or("Section not found".to_string())?;
                agenda
                    .remove_item_from_section(item_id, section)
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            }
            SlotContext::GeneratedSection {
                section_index: _,
                on_insert_assign: _,
                on_remove_unassign,
            } => {
                let temp = generated_section(on_remove_unassign.clone(), HashSet::new());
                agenda
                    .remove_item_from_section(item_id, &temp)
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            }
            SlotContext::Unmatched => agenda
                .remove_item_from_unmatched(item_id, view)
                .map(|_| ())
                .map_err(|e| e.to_string()),
        }
    }

    pub(crate) fn insert_into_context(
        &self,
        agenda: &Agenda<'_>,
        item_id: ItemId,
        view: &View,
        context: &SlotContext,
    ) -> Result<(), String> {
        match context {
            SlotContext::Section { section_index } => {
                let section = view
                    .sections
                    .get(*section_index)
                    .ok_or("Section not found".to_string())?;
                agenda
                    .insert_item_in_section(item_id, view, section)
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            }
            SlotContext::GeneratedSection {
                section_index: _,
                on_insert_assign,
                on_remove_unassign,
            } => {
                let temp = generated_section(on_remove_unassign.clone(), on_insert_assign.clone());
                agenda
                    .insert_item_in_section(item_id, view, &temp)
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            }
            SlotContext::Unmatched => agenda
                .insert_item_in_unmatched(item_id, view)
                .map(|_| ())
                .map_err(|e| e.to_string()),
        }
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
        self.category_manager = Some(CategoryManagerState {
            focus: CategoryManagerFocus::Tree,
            filter: text_buffer::TextBuffer::empty(),
            filter_editing: false,
            details_focus: CategoryManagerDetailsFocus::Exclusive,
            details_note_category_id: selected_category_id,
            details_note: text_buffer::TextBuffer::new(initial_note),
            details_note_dirty: false,
            details_note_editing: false,
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
            let next_note = next_category_id
                .and_then(|id| self.categories.iter().find(|c| c.id == id))
                .and_then(|c| c.note.clone())
                .unwrap_or_default();
            if let Some(state) = &mut self.category_manager {
                state.details_note_category_id = next_category_id;
                state.details_note = text_buffer::TextBuffer::new(next_note);
                state.details_note_dirty = false;
                state.details_note_editing = false;
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
        if let Some(state) = &mut self.category_manager {
            state.details_focus = match delta.signum() {
                d if d > 0 => state.details_focus.next(is_numeric),
                d if d < 0 => state.details_focus.prev(is_numeric),
                _ => state.details_focus,
            };
            state.details_note_editing = false;
        }
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
        }
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
            }
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
        let next = match (current, delta.signum()) {
            (CategoryManagerFocus::Tree, 1) => CategoryManagerFocus::Details,
            (CategoryManagerFocus::Filter, 1) => CategoryManagerFocus::Details,
            (CategoryManagerFocus::Details, 1) => CategoryManagerFocus::Tree,
            (CategoryManagerFocus::Tree, -1) => CategoryManagerFocus::Details,
            (CategoryManagerFocus::Filter, -1) => CategoryManagerFocus::Tree,
            (CategoryManagerFocus::Details, -1) => CategoryManagerFocus::Tree,
            _ => current,
        };
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
            self.view_index = index;
            self.picker_index = index;
        }
    }

    pub(crate) fn cycle_view(&mut self, delta: i32, agenda: &Agenda<'_>) -> Result<(), String> {
        if self.views.is_empty() {
            self.status = "No views available".to_string();
            return Ok(());
        }
        self.view_index = next_index(self.view_index, self.views.len(), delta);
        self.picker_index = self.view_index;
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

    pub(crate) fn jump_to_all_items_view(&mut self, agenda: &Agenda<'_>) -> Result<(), String> {
        let Some(index) = self
            .views
            .iter()
            .position(|view| view.name.eq_ignore_ascii_case("All Items"))
        else {
            self.status = "All Items view not found".to_string();
            return Ok(());
        };
        self.view_index = index;
        self.picker_index = index;
        self.slot_index = 0;
        self.item_index = 0;
        self.slot_sort_keys.clear();
        self.refresh(agenda.store())?;
        self.reset_section_filters();
        self.status = "Jumped to view: All Items".to_string();
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
        slot.items.sort_by(|left, right| {
            self.compare_items_for_sort_keys(left, right, sort_keys)
        });
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
                    let heading_category = self.categories.iter().find(|category| category.id == heading);
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
                        let left_value =
                            self.standard_sort_value_for_heading(left, category);
                        let right_value =
                            self.standard_sort_value_for_heading(right, category);
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
