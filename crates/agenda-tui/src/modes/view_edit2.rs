use crate::*;

impl App {
    pub(crate) const DEFAULT_VIEW_EDIT_SECTION_TITLE: &'static str = "New section";

    fn cycle_view_board_display_mode(mode: BoardDisplayMode) -> BoardDisplayMode {
        match mode {
            BoardDisplayMode::SingleLine => BoardDisplayMode::MultiLine,
            BoardDisplayMode::MultiLine => BoardDisplayMode::SingleLine,
        }
    }

    fn cycle_section_board_display_mode_override(
        current: Option<BoardDisplayMode>,
    ) -> Option<BoardDisplayMode> {
        match current {
            None => Some(BoardDisplayMode::SingleLine),
            Some(BoardDisplayMode::SingleLine) => Some(BoardDisplayMode::MultiLine),
            Some(BoardDisplayMode::MultiLine) => None,
        }
    }

    pub(crate) fn view_edit_default_section(title: &str) -> Section {
        Section {
            title: title.to_string(),
            criteria: Query::default(),
            columns: Vec::new(),
            item_column_index: 0,
            on_insert_assign: HashSet::new(),
            on_remove_unassign: HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        }
    }

    fn view_edit_default_status() -> String {
        "View editor".to_string()
    }

    fn set_view_edit_dirty(&mut self) {
        if let Some(state) = &mut self.view_edit_state {
            state.dirty = true;
            state.discard_confirm = false;
            state.section_delete_confirm = None;
        }
    }

    fn begin_view_edit_section_title_input(&mut self, section_index: usize) {
        if let Some(state) = &mut self.view_edit_state {
            if let Some(section) = state.draft.sections.get(section_index) {
                state.region = ViewEditRegion::Sections;
                state.pane_focus = ViewEditPaneFocus::Sections;
                state.section_index = section_index;
                state.sections_view_row_selected = false;
                state.section_details_field_index = 0;
                state.section_expanded = Some(section_index);
                state.inline_input = Some(ViewEditInlineInput::SectionTitle { section_index });
                state.inline_buf = text_buffer::TextBuffer::new(section.title.clone());
                state.discard_confirm = false;
                self.status = "Section title: type text  Enter:confirm  Esc:cancel".to_string();
            }
        }
    }

    fn begin_view_edit_name_input(&mut self) {
        if let Some(state) = &mut self.view_edit_state {
            let current = state.draft.name.clone();
            state.sections_view_row_selected = true;
            if state.region == ViewEditRegion::Sections {
                state.region = ViewEditRegion::Criteria;
            }
            state.pane_focus = ViewEditPaneFocus::Details;
            state.inline_input = Some(ViewEditInlineInput::ViewName);
            state.inline_buf = text_buffer::TextBuffer::new(current);
            state.discard_confirm = false;
            state.section_delete_confirm = None;
            self.status = "View name: type text  Enter:confirm  Esc:cancel".to_string();
        }
    }

    fn insert_view_edit_section(&mut self, insert_index: usize) -> Option<usize> {
        let mut new_index = None;
        if let Some(state) = &mut self.view_edit_state {
            let idx = insert_index.min(state.draft.sections.len());
            state.draft.sections.insert(
                idx,
                Self::view_edit_default_section(Self::DEFAULT_VIEW_EDIT_SECTION_TITLE),
            );
            if let Some(expanded_index) = state.section_expanded {
                if expanded_index >= idx {
                    state.section_expanded = Some(expanded_index + 1);
                }
            }
            state.section_index = idx;
            state.sections_view_row_selected = false;
            state.section_details_field_index = 0;
            new_index = Some(idx);
        }
        if new_index.is_some() {
            self.set_view_edit_dirty();
        }
        new_index
    }

    /// Open the unified ViewEdit screen for `view`.
    pub(crate) fn open_view_edit(&mut self, view: View) {
        let preview_count = self.preview_count_for_query(&view.criteria);
        self.view_edit_state = Some(ViewEditState {
            draft: view,
            region: ViewEditRegion::Criteria,
            pane_focus: ViewEditPaneFocus::Details,
            criteria_index: 0,
            unmatched_field_index: 0,
            section_index: 0,
            sections_view_row_selected: false,
            section_details_field_index: 0,
            section_expanded: None,
            overlay: None,
            inline_input: None,
            inline_buf: text_buffer::TextBuffer::empty(),
            picker_index: 0,
            overlay_filter_buf: text_buffer::TextBuffer::empty(),
            preview_count,
            preview_visible: false,
            preview_scroll: 0,
            sections_filter_buf: text_buffer::TextBuffer::empty(),
            dirty: false,
            discard_confirm: false,
            section_delete_confirm: None,
        });
        self.mode = Mode::ViewEdit;
        self.status = Self::view_edit_default_status();
    }

    pub(crate) fn open_view_edit_new_view_focus_first_section(&mut self, view: View) {
        self.open_view_edit(view);
        self.begin_view_edit_section_title_input(0);
    }

    fn view_details_criteria_row_count(state: &ViewEditState) -> usize {
        state.draft.criteria.criteria.len().max(1)
    }

    fn view_details_aux_field_count() -> usize {
        // when include, when exclude, display mode, unmatched visible, unmatched label
        5
    }

    fn view_edit_showing_view_details(state: &ViewEditState) -> bool {
        state.region != ViewEditRegion::Sections
            || state.sections_view_row_selected
            || state.draft.sections.get(state.section_index).is_none()
    }

    fn view_edit_section_filter_query(state: &ViewEditState) -> Option<String> {
        let q = state.sections_filter_buf.trimmed();
        if q.is_empty() {
            None
        } else {
            Some(q.to_ascii_lowercase())
        }
    }

    fn view_edit_visible_section_indices(state: &ViewEditState) -> Vec<usize> {
        let Some(filter) = Self::view_edit_section_filter_query(state) else {
            return (0..state.draft.sections.len()).collect();
        };
        state
            .draft
            .sections
            .iter()
            .enumerate()
            .filter_map(|(i, section)| {
                let title = section.title.to_ascii_lowercase();
                if title.contains(&filter) {
                    Some(i)
                } else {
                    None
                }
            })
            .collect()
    }

    fn view_edit_filter_is_active(state: &ViewEditState) -> bool {
        Self::view_edit_section_filter_query(state).is_some()
    }

    fn view_edit_overlay_category_filter_query(state: &ViewEditState) -> Option<String> {
        let q = state.overlay_filter_buf.trimmed();
        if q.is_empty() {
            None
        } else {
            Some(q.to_ascii_lowercase())
        }
    }

    fn view_edit_filtered_category_row_indices(&self, state: &ViewEditState) -> Vec<usize> {
        let Some(filter) = Self::view_edit_overlay_category_filter_query(state) else {
            return (0..self.category_rows.len()).collect();
        };
        self.category_rows
            .iter()
            .enumerate()
            .filter_map(|(i, row)| {
                if row.name.to_ascii_lowercase().contains(&filter) {
                    Some(i)
                } else {
                    None
                }
            })
            .collect()
    }

    fn clear_view_edit_section_filter(&mut self) {
        if let Some(state) = &mut self.view_edit_state {
            state.sections_filter_buf.clear();
            if matches!(
                state.inline_input,
                Some(ViewEditInlineInput::SectionsFilter)
            ) {
                state.inline_input = None;
            }
        }
        self.normalize_view_edit_sections_selection_for_filter();
        self.status = "Section filter cleared".to_string();
    }

    fn normalize_view_edit_sections_selection_for_filter(&mut self) {
        let Some(state) = &mut self.view_edit_state else {
            return;
        };
        let visible = Self::view_edit_visible_section_indices(state);
        if visible.is_empty() {
            state.sections_view_row_selected = true;
            if state.region == ViewEditRegion::Sections {
                state.section_details_field_index = 0;
            }
            return;
        }

        if state.sections_view_row_selected {
            return;
        }
        if !visible.contains(&state.section_index) {
            state.section_index = visible[0];
            if state.region == ViewEditRegion::Sections {
                state.section_details_field_index = 0;
            }
        }
    }

    fn view_details_focus_index(state: &ViewEditState) -> usize {
        let criteria_rows = Self::view_details_criteria_row_count(state);
        match state.region {
            ViewEditRegion::Criteria => state.criteria_index.min(criteria_rows.saturating_sub(1)),
            ViewEditRegion::Unmatched => {
                criteria_rows
                    + state
                        .unmatched_field_index
                        .min(Self::view_details_aux_field_count() - 1)
            }
            ViewEditRegion::Sections => 0,
        }
    }

    fn set_view_details_focus_index(&mut self, new_index: usize) {
        if let Some(state) = &mut self.view_edit_state {
            let criteria_rows = Self::view_details_criteria_row_count(state);
            if new_index < criteria_rows {
                state.region = ViewEditRegion::Criteria;
                state.criteria_index = if state.draft.criteria.criteria.is_empty() {
                    0
                } else {
                    new_index.min(state.draft.criteria.criteria.len().saturating_sub(1))
                };
            } else {
                state.region = ViewEditRegion::Unmatched;
                state.unmatched_field_index =
                    (new_index - criteria_rows).min(Self::view_details_aux_field_count() - 1);
            }
        }
    }

    fn cycle_view_edit_pane_focus(&mut self, forward: bool) {
        if let Some(state) = &mut self.view_edit_state {
            let next = if state.preview_visible {
                match (state.pane_focus, forward) {
                    (ViewEditPaneFocus::Sections, true) => ViewEditPaneFocus::Details,
                    (ViewEditPaneFocus::Details, true) => ViewEditPaneFocus::Preview,
                    (ViewEditPaneFocus::Preview, true) => ViewEditPaneFocus::Sections,
                    (ViewEditPaneFocus::Sections, false) => ViewEditPaneFocus::Preview,
                    (ViewEditPaneFocus::Details, false) => ViewEditPaneFocus::Sections,
                    (ViewEditPaneFocus::Preview, false) => ViewEditPaneFocus::Details,
                }
            } else {
                match state.pane_focus {
                    ViewEditPaneFocus::Sections => ViewEditPaneFocus::Details,
                    ViewEditPaneFocus::Details => ViewEditPaneFocus::Sections,
                    ViewEditPaneFocus::Preview => ViewEditPaneFocus::Sections,
                }
            };
            state.pane_focus = next;

            if state.pane_focus == ViewEditPaneFocus::Sections {
                if state.region != ViewEditRegion::Sections {
                    state.sections_view_row_selected = true;
                }
            } else if state.pane_focus == ViewEditPaneFocus::Details
                && state.region == ViewEditRegion::Sections
            {
                if state.sections_view_row_selected {
                    state.region = ViewEditRegion::Criteria;
                }
                state.section_details_field_index = 0;
            }
        }
    }

    fn toggle_view_edit_preview_visible(&mut self) {
        if let Some(state) = &mut self.view_edit_state {
            state.preview_visible = !state.preview_visible;
            if !state.preview_visible && state.pane_focus == ViewEditPaneFocus::Preview {
                state.pane_focus = ViewEditPaneFocus::Sections;
            }
            state.preview_scroll = 0;
            self.status = if state.preview_visible {
                "Preview pane shown".to_string()
            } else {
                "Preview pane hidden".to_string()
            };
        }
    }

    /// Recompute `preview_count` in `view_edit_state` from the current draft criteria.
    fn refresh_view_edit_preview(&mut self) {
        if let Some(state) = &self.view_edit_state {
            let count = self.preview_count_for_query(&state.draft.criteria);
            if let Some(state) = &mut self.view_edit_state {
                state.preview_count = count;
            }
        }
    }

    fn close_view_edit_overlay(&mut self) {
        if let Some(state) = &mut self.view_edit_state {
            state.overlay = None;
            state.picker_index = 0;
            state.overlay_filter_buf.clear();
        }
        self.status = Self::view_edit_default_status();
    }

    fn request_view_edit_section_delete(&mut self, section_index: usize) {
        if let Some(state) = &mut self.view_edit_state {
            if section_index < state.draft.sections.len() {
                state.section_delete_confirm = Some(section_index);
                state.discard_confirm = false;
                let title = state.draft.sections[section_index].title.clone();
                self.status = format!("Delete section \"{title}\"? y/n");
            }
        }
    }

    fn confirm_view_edit_section_delete(&mut self) {
        let Some(idx) = self
            .view_edit_state
            .as_ref()
            .and_then(|s| s.section_delete_confirm)
        else {
            return;
        };

        if let Some(state) = &mut self.view_edit_state {
            state.section_delete_confirm = None;
            if idx >= state.draft.sections.len() {
                self.status = Self::view_edit_default_status();
                return;
            }

            state.draft.sections.remove(idx);
            let new_len = state.draft.sections.len();
            if state.section_index >= new_len && new_len > 0 {
                state.section_index = new_len - 1;
            }
            if new_len == 0 {
                state.sections_view_row_selected = true;
                state.section_details_field_index = 0;
            }
            if state.section_expanded == Some(idx) {
                state.section_expanded = None;
            } else if let Some(expanded) = state.section_expanded {
                if expanded > idx {
                    state.section_expanded = Some(expanded - 1);
                }
            }
            state.dirty = true;
            state.discard_confirm = false;
        }
        self.status = Self::view_edit_default_status();
    }

    fn toggle_category_picker_selection(
        &mut self,
        target: CategoryEditTarget,
        section_expanded: usize,
        cat_id: CategoryId,
    ) {
        if let Some(state) = &mut self.view_edit_state {
            match target {
                CategoryEditTarget::ViewCriteria => {
                    if state.draft.criteria.mode_for(cat_id).is_some() {
                        state.draft.criteria.remove_criterion(cat_id);
                    } else {
                        state
                            .draft
                            .criteria
                            .set_criterion(CriterionMode::And, cat_id);
                    }
                }
                CategoryEditTarget::SectionCriteria => {
                    if let Some(section) = state.draft.sections.get_mut(section_expanded) {
                        if section.criteria.mode_for(cat_id).is_some() {
                            section.criteria.remove_criterion(cat_id);
                        } else {
                            section.criteria.set_criterion(CriterionMode::And, cat_id);
                        }
                    }
                }
                CategoryEditTarget::SectionColumns => {
                    if let Some(section) = state.draft.sections.get_mut(section_expanded) {
                        if let Some(existing_index) =
                            section.columns.iter().position(|col| col.heading == cat_id)
                        {
                            section.columns.remove(existing_index);
                        } else if let Some(cat) = self.categories.iter().find(|c| c.id == cat_id) {
                            section.columns.push(Column {
                                kind: column_kind_for_heading(cat),
                                heading: cat_id,
                                width: 12,
                            });
                        }
                    }
                }
                CategoryEditTarget::SectionOnInsertAssign => {
                    if let Some(section) = state.draft.sections.get_mut(section_expanded) {
                        if !section.on_insert_assign.remove(&cat_id) {
                            section.on_insert_assign.insert(cat_id);
                        }
                    }
                }
                CategoryEditTarget::SectionOnRemoveUnassign => {
                    if let Some(section) = state.draft.sections.get_mut(section_expanded) {
                        if !section.on_remove_unassign.remove(&cat_id) {
                            section.on_remove_unassign.insert(cat_id);
                        }
                    }
                }
            }
        }
        self.set_view_edit_dirty();
        self.refresh_view_edit_preview();
    }

    pub(crate) fn handle_view_edit_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> Result<bool, String> {
        // Layer 1: inline text input intercepts all keys.
        if self
            .view_edit_state
            .as_ref()
            .map(|s| s.inline_input.is_some())
            .unwrap_or(false)
        {
            self.handle_view_edit_inline_key(code)?;
            return Ok(false);
        }

        // Layer 2: picker overlay intercepts all keys.
        if self
            .view_edit_state
            .as_ref()
            .map(|s| s.overlay.is_some())
            .unwrap_or(false)
        {
            self.handle_view_edit_overlay_key(code)?;
            return Ok(false);
        }

        // Layer 3: discard confirmation intercepts before region/global keys.
        if self
            .view_edit_state
            .as_ref()
            .map(|s| s.discard_confirm)
            .unwrap_or(false)
        {
            self.handle_view_edit_discard_confirm_key(code, agenda)?;
            return Ok(false);
        }

        // Layer 4: section delete confirmation intercepts before pane/global keys.
        if self
            .view_edit_state
            .as_ref()
            .and_then(|s| s.section_delete_confirm)
            .is_some()
        {
            self.handle_view_edit_section_delete_confirm_key(code)?;
            return Ok(false);
        }

        // Layer 5: global and region keys.
        self.handle_view_edit_region_key(code, agenda)?;
        Ok(false)
    }

    fn handle_view_edit_discard_confirm_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> Result<bool, String> {
        match code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                if let Some(state) = &mut self.view_edit_state {
                    state.discard_confirm = false;
                }
                return self.handle_view_edit_save(agenda);
            }
            KeyCode::Char('n') | KeyCode::Char('N') => {
                self.view_edit_state = None;
                self.mode = Mode::ViewPicker;
                self.status = "Discarded unsaved changes".to_string();
            }
            KeyCode::Esc => {
                if let Some(state) = &mut self.view_edit_state {
                    state.discard_confirm = false;
                }
                self.status = Self::view_edit_default_status();
            }
            _ => {}
        }
        Ok(true)
    }

    fn handle_view_edit_section_delete_confirm_key(
        &mut self,
        code: KeyCode,
    ) -> Result<bool, String> {
        match code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                self.confirm_view_edit_section_delete();
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                if let Some(state) = &mut self.view_edit_state {
                    state.section_delete_confirm = None;
                }
                self.status = Self::view_edit_default_status();
            }
            _ => {}
        }
        Ok(true)
    }

    // -------------------------------------------------------------------------
    // Layer 1: inline text input
    // -------------------------------------------------------------------------

    fn handle_view_edit_inline_key(&mut self, code: KeyCode) -> Result<bool, String> {
        let Some(state) = &self.view_edit_state else {
            return Ok(false);
        };
        let inline = state.inline_input.clone();
        match code {
            KeyCode::Esc => {
                let mut clear_buf = true;
                if let Some(state) = &mut self.view_edit_state {
                    if matches!(
                        state.inline_input,
                        Some(ViewEditInlineInput::SectionsFilter)
                    ) {
                        clear_buf = false;
                    }
                    state.inline_input = None;
                    if clear_buf {
                        state.inline_buf.clear();
                    }
                }
                self.status = Self::view_edit_default_status();
            }
            KeyCode::Enter => {
                let Some(state) = &mut self.view_edit_state else {
                    return Ok(false);
                };
                let text = state.inline_buf.trimmed().to_string();
                let mut changed = false;
                let mut filter_done_status: Option<String> = None;
                match &inline {
                    Some(ViewEditInlineInput::SectionsFilter) => {
                        state.inline_input = None;
                        let status = if Self::view_edit_filter_is_active(state) {
                            format!("Section filter: {}", state.sections_filter_buf.text())
                        } else {
                            Self::view_edit_default_status()
                        };
                        filter_done_status = Some(status);
                    }
                    Some(ViewEditInlineInput::ViewName) => {
                        changed = state.draft.name != text;
                        state.draft.name = text;
                    }
                    Some(ViewEditInlineInput::SectionTitle { section_index }) => {
                        if let Some(section) = state.draft.sections.get_mut(*section_index) {
                            changed = section.title != text;
                            section.title = text;
                        }
                    }
                    Some(ViewEditInlineInput::UnmatchedLabel) => {
                        changed = state.draft.unmatched_label != text;
                        state.draft.unmatched_label = text;
                    }
                    None => {}
                }
                if filter_done_status.is_some() {
                    state.inline_buf.clear();
                }
                state.inline_input = None;
                if filter_done_status.is_none() {
                    state.inline_buf.clear();
                }
                if changed {
                    state.dirty = true;
                    state.discard_confirm = false;
                }
                if let Some(status) = filter_done_status {
                    // mutable borrow ends before normalization/status update
                    let _ = state;
                    self.normalize_view_edit_sections_selection_for_filter();
                    self.status = status;
                    return Ok(true);
                }
                self.status = Self::view_edit_default_status();
            }
            _ => {
                let mut filter_status: Option<String> = None;
                if let Some(state) = &mut self.view_edit_state {
                    match inline {
                        Some(ViewEditInlineInput::SectionsFilter) => {
                            state.sections_filter_buf.handle_key(code, false);
                            filter_status = Some(if Self::view_edit_filter_is_active(state) {
                                format!("Section filter: {}", state.sections_filter_buf.text())
                            } else {
                                "Section filter".to_string()
                            });
                        }
                        _ => {
                            state.inline_buf.handle_key(code, false);
                        }
                    }
                }
                if let Some(status) = filter_status {
                    self.normalize_view_edit_sections_selection_for_filter();
                    self.status = status;
                }
            }
        }
        Ok(true)
    }

    // -------------------------------------------------------------------------
    // Layer 2: picker overlay
    // -------------------------------------------------------------------------

    fn handle_view_edit_overlay_key(&mut self, code: KeyCode) -> Result<bool, String> {
        let Some(state) = &self.view_edit_state else {
            return Ok(false);
        };
        let overlay = state.overlay.clone();
        let picker_index = state.picker_index;
        let section_expanded = state.section_expanded.unwrap_or(0);

        match overlay {
            Some(ViewEditOverlay::CategoryPicker { target }) => {
                let filtered_indices = self
                    .view_edit_state
                    .as_ref()
                    .map(|s| self.view_edit_filtered_category_row_indices(s))
                    .unwrap_or_default();
                let current_visible_pos = filtered_indices
                    .iter()
                    .position(|&actual_idx| actual_idx == picker_index)
                    .unwrap_or(0);
                match code {
                    KeyCode::Char('j') | KeyCode::Down => {
                        if let Some(state) = &mut self.view_edit_state {
                            if let Some(&actual_idx) = filtered_indices.get(
                                (current_visible_pos + 1)
                                    .min(filtered_indices.len().saturating_sub(1)),
                            ) {
                                state.picker_index = actual_idx;
                            }
                        }
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        if let Some(state) = &mut self.view_edit_state {
                            if let Some(&actual_idx) =
                                filtered_indices.get(current_visible_pos.saturating_sub(1))
                            {
                                state.picker_index = actual_idx;
                            }
                        }
                    }
                    KeyCode::Char(' ') | KeyCode::Enter => {
                        if let Some(&actual_idx) = filtered_indices.get(current_visible_pos) {
                            if let Some(row) = self.category_rows.get(actual_idx).cloned() {
                                self.toggle_category_picker_selection(
                                    target,
                                    section_expanded,
                                    row.id,
                                );
                            }
                        }
                    }
                    KeyCode::Esc => {
                        self.close_view_edit_overlay();
                    }
                    _ => {
                        let mut consumed = false;
                        let mut overlay_query: Option<String> = None;
                        if let Some(state) = &mut self.view_edit_state {
                            consumed = state.overlay_filter_buf.handle_key(code, false);
                            if consumed {
                                overlay_query = Some(state.overlay_filter_buf.text().to_string());
                            }
                        }
                        if consumed {
                            let filtered = if overlay_query
                                .as_deref()
                                .map(str::trim)
                                .unwrap_or("")
                                .is_empty()
                            {
                                (0..self.category_rows.len()).collect::<Vec<usize>>()
                            } else {
                                let q = overlay_query
                                    .as_deref()
                                    .unwrap_or("")
                                    .trim()
                                    .to_ascii_lowercase();
                                self.category_rows
                                    .iter()
                                    .enumerate()
                                    .filter_map(|(i, row)| {
                                        if row.name.to_ascii_lowercase().contains(&q) {
                                            Some(i)
                                        } else {
                                            None
                                        }
                                    })
                                    .collect::<Vec<usize>>()
                            };
                            if let Some(&actual_idx) = filtered.first() {
                                if let Some(state) = &mut self.view_edit_state {
                                    state.picker_index = actual_idx;
                                }
                            }
                        }
                    }
                }
            }
            Some(ViewEditOverlay::BucketPicker { target }) => {
                let options = when_bucket_options();
                match code {
                    KeyCode::Char('j') | KeyCode::Down => {
                        if let Some(state) = &mut self.view_edit_state {
                            state.picker_index = next_index_clamped(picker_index, options.len(), 1);
                        }
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        if let Some(state) = &mut self.view_edit_state {
                            state.picker_index =
                                next_index_clamped(picker_index, options.len(), -1);
                        }
                    }
                    KeyCode::Char(' ') | KeyCode::Enter | KeyCode::Esc => {
                        if matches!(code, KeyCode::Char(' ') | KeyCode::Enter) {
                            if let Some(&bucket) = options.get(picker_index) {
                                if let Some(state) = &mut self.view_edit_state {
                                    if let Some(set) =
                                        bucket_target_set_mut(&mut state.draft, target)
                                    {
                                        if set.contains(&bucket) {
                                            set.remove(&bucket);
                                        } else {
                                            set.insert(bucket);
                                        }
                                    }
                                }
                                self.set_view_edit_dirty();
                                self.refresh_view_edit_preview();
                            }
                        }
                        if let Some(state) = &mut self.view_edit_state {
                            state.overlay = None;
                            state.picker_index = 0;
                        }
                        self.status = Self::view_edit_default_status();
                    }
                    _ => {}
                }
            }
            None => {}
        }
        Ok(true)
    }

    // -------------------------------------------------------------------------
    // Layer 3: region-level keys
    // -------------------------------------------------------------------------

    fn handle_view_edit_region_key(
        &mut self,
        code: KeyCode,
        agenda: &Agenda<'_>,
    ) -> Result<bool, String> {
        // Global keys first
        match code {
            KeyCode::Esc => {
                let filter_active = self
                    .view_edit_state
                    .as_ref()
                    .map(Self::view_edit_filter_is_active)
                    .unwrap_or(false);
                if filter_active {
                    self.clear_view_edit_section_filter();
                    return Ok(true);
                }
                let is_dirty = self
                    .view_edit_state
                    .as_ref()
                    .map(|s| s.dirty)
                    .unwrap_or(false);
                if is_dirty {
                    if let Some(state) = &mut self.view_edit_state {
                        state.discard_confirm = true;
                    }
                    self.status =
                        "Unsaved changes: save before closing? y=save n=discard Esc=keep editing"
                            .to_string();
                } else {
                    self.view_edit_state = None;
                    self.mode = Mode::ViewPicker;
                    self.status = "View edit canceled".to_string();
                }
                return Ok(true);
            }
            KeyCode::Tab => {
                self.cycle_view_edit_pane_focus(true);
                return Ok(true);
            }
            KeyCode::BackTab => {
                self.cycle_view_edit_pane_focus(false);
                return Ok(true);
            }
            KeyCode::Char('S') => {
                return self.handle_view_edit_save(agenda);
            }
            KeyCode::Char('p') | KeyCode::Char('P') => {
                self.toggle_view_edit_preview_visible();
                return Ok(true);
            }
            KeyCode::Char('/') => {
                if let Some(state) = &mut self.view_edit_state {
                    state.pane_focus = ViewEditPaneFocus::Sections;
                    state.inline_input = Some(ViewEditInlineInput::SectionsFilter);
                    state.sections_view_row_selected = state.sections_view_row_selected
                        || state.region != ViewEditRegion::Sections;
                }
                self.status = "Section filter: type to filter  Enter:done  Esc:close".to_string();
                return Ok(true);
            }
            _ => {}
        }

        let Some(state) = &self.view_edit_state else {
            return Ok(false);
        };

        match state.pane_focus {
            ViewEditPaneFocus::Sections => self.handle_view_edit_sections_key(code),
            ViewEditPaneFocus::Details => {
                if Self::view_edit_showing_view_details(state) {
                    match state.region {
                        ViewEditRegion::Criteria => self.handle_view_edit_criteria_key(code),
                        ViewEditRegion::Unmatched => self.handle_view_edit_unmatched_key(code),
                        ViewEditRegion::Sections => self.handle_view_edit_criteria_key(code),
                    }
                } else {
                    self.handle_view_edit_section_details_key(code)
                }
            }
            ViewEditPaneFocus::Preview => self.handle_view_edit_preview_key(code),
        }
    }

    fn handle_view_edit_save(&mut self, agenda: &Agenda<'_>) -> Result<bool, String> {
        let Some(draft) = self.view_edit_state.as_ref().map(|s| s.draft.clone()) else {
            self.status = "View edit failed: no draft".to_string();
            return Ok(false);
        };
        let view_name = draft.name.clone();
        match agenda.store().update_view(&draft) {
            Ok(()) => {
                self.refresh(agenda.store())?;
                self.set_view_selection_by_name(&view_name);
                self.reset_section_filters();
                self.view_edit_state = None;
                self.mode = Mode::ViewPicker;
                self.status = format!("Saved view \"{view_name}\"");
            }
            Err(err) => {
                self.status = format!("View save failed: {err}");
            }
        }
        Ok(true)
    }

    // -------------------------------------------------------------------------
    // Criteria region
    // -------------------------------------------------------------------------

    fn handle_view_edit_criteria_key(&mut self, code: KeyCode) -> Result<bool, String> {
        let Some(state) = &self.view_edit_state else {
            return Ok(false);
        };
        let idx = state.criteria_index;
        let criteria_rows = Self::view_details_criteria_row_count(state);

        match code {
            KeyCode::Char('j') | KeyCode::Down => {
                let current = self
                    .view_edit_state
                    .as_ref()
                    .map(Self::view_details_focus_index)
                    .unwrap_or(0);
                let max_index = criteria_rows + 2 - 1;
                self.set_view_details_focus_index((current + 1).min(max_index));
            }
            KeyCode::Char('k') | KeyCode::Up => {
                let current = self
                    .view_edit_state
                    .as_ref()
                    .map(Self::view_details_focus_index)
                    .unwrap_or(0);
                self.set_view_details_focus_index(current.saturating_sub(1));
            }
            KeyCode::Char('n') | KeyCode::Char('N') => {
                let first = first_non_reserved_category_index(&self.category_rows);
                if let Some(state) = &mut self.view_edit_state {
                    state.overlay = Some(ViewEditOverlay::CategoryPicker {
                        target: CategoryEditTarget::ViewCriteria,
                    });
                    state.picker_index = first;
                }
                self.status = "Add criteria: j/k select  Space/Enter:toggle  Esc:done".to_string();
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                self.begin_view_edit_name_input();
            }
            KeyCode::Char('x') => {
                let mut changed = false;
                if let Some(state) = &mut self.view_edit_state {
                    if idx < state.draft.criteria.criteria.len() {
                        state.draft.criteria.criteria.remove(idx);
                        let new_len = state.draft.criteria.criteria.len();
                        if state.criteria_index >= new_len && new_len > 0 {
                            state.criteria_index = new_len - 1;
                        }
                        changed = true;
                    }
                }
                if changed {
                    self.set_view_edit_dirty();
                    self.refresh_view_edit_preview();
                }
            }
            KeyCode::Char(' ') => {
                // Cycle mode: And → Not → Or → And
                let mut changed = false;
                if let Some(state) = &mut self.view_edit_state {
                    if let Some(criterion) = state.draft.criteria.criteria.get_mut(idx) {
                        criterion.mode = match criterion.mode {
                            CriterionMode::And => CriterionMode::Not,
                            CriterionMode::Not => CriterionMode::Or,
                            CriterionMode::Or => CriterionMode::And,
                        };
                        changed = true;
                    }
                }
                if changed {
                    self.set_view_edit_dirty();
                    self.refresh_view_edit_preview();
                }
            }
            KeyCode::Enter => {
                // Match details-pane interaction semantics: Enter acts like Space on criteria rows.
                let mut changed = false;
                if let Some(state) = &mut self.view_edit_state {
                    if let Some(criterion) = state.draft.criteria.criteria.get_mut(idx) {
                        criterion.mode = match criterion.mode {
                            CriterionMode::And => CriterionMode::Not,
                            CriterionMode::Not => CriterionMode::Or,
                            CriterionMode::Or => CriterionMode::And,
                        };
                        changed = true;
                    }
                }
                if changed {
                    self.set_view_edit_dirty();
                    self.refresh_view_edit_preview();
                }
            }
            KeyCode::Char(']') => {
                if let Some(state) = &mut self.view_edit_state {
                    state.overlay = Some(ViewEditOverlay::BucketPicker {
                        target: BucketEditTarget::ViewVirtualInclude,
                    });
                    state.picker_index = 0;
                }
            }
            KeyCode::Char('[') => {
                if let Some(state) = &mut self.view_edit_state {
                    state.overlay = Some(ViewEditOverlay::BucketPicker {
                        target: BucketEditTarget::ViewVirtualExclude,
                    });
                    state.picker_index = 0;
                }
            }
            KeyCode::Char('m') | KeyCode::Char('M') => {
                if let Some(state) = &mut self.view_edit_state {
                    state.draft.board_display_mode =
                        Self::cycle_view_board_display_mode(state.draft.board_display_mode);
                    state.dirty = true;
                    state.discard_confirm = false;
                }
            }
            _ => {}
        }
        Ok(true)
    }

    // -------------------------------------------------------------------------
    // Sections region
    // -------------------------------------------------------------------------

    fn handle_view_edit_section_details_key(&mut self, code: KeyCode) -> Result<bool, String> {
        let field_count = 8usize;
        let current_index = self
            .view_edit_state
            .as_ref()
            .map(|s| {
                s.section_details_field_index
                    .min(field_count.saturating_sub(1))
            })
            .unwrap_or(0);

        match code {
            KeyCode::Char('j') | KeyCode::Down => {
                if let Some(state) = &mut self.view_edit_state {
                    state.section_details_field_index =
                        (current_index + 1).min(field_count.saturating_sub(1));
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if let Some(state) = &mut self.view_edit_state {
                    state.section_details_field_index = current_index.saturating_sub(1);
                }
            }
            KeyCode::Char(' ') | KeyCode::Enter => {
                let mapped = match current_index {
                    0 => Some(KeyCode::Char('e')),
                    1 => Some(KeyCode::Char('f')),
                    2 => Some(KeyCode::Char('c')),
                    3 => Some(KeyCode::Char('a')),
                    4 => Some(KeyCode::Char('r')),
                    5 => Some(KeyCode::Char('h')),
                    6 => Some(KeyCode::Char('m')),
                    7 => Some(KeyCode::Enter),
                    _ => None,
                };
                if let Some(mapped) = mapped {
                    return self.handle_view_edit_sections_key(mapped);
                }
            }
            _ => {
                return self.handle_view_edit_sections_key(code);
            }
        }
        Ok(true)
    }

    fn handle_view_edit_sections_key(&mut self, code: KeyCode) -> Result<bool, String> {
        let Some(state) = &self.view_edit_state else {
            return Ok(false);
        };
        let len = state.draft.sections.len();
        let idx = state.section_index;
        let selecting_view_row = state.sections_view_row_selected;
        let visible_indices = Self::view_edit_visible_section_indices(state);
        let current_visible_pos = visible_indices.iter().position(|&i| i == idx);

        match code {
            KeyCode::Char('j') | KeyCode::Down => {
                if let Some(state) = &mut self.view_edit_state {
                    if state.sections_view_row_selected {
                        if let Some(&first_visible) = visible_indices.first() {
                            state.sections_view_row_selected = false;
                            state.region = ViewEditRegion::Sections;
                            state.section_index = first_visible;
                            state.section_details_field_index = 0;
                        }
                    } else {
                        state.region = ViewEditRegion::Sections;
                        if let Some(pos) = current_visible_pos {
                            let next_pos = (pos + 1).min(visible_indices.len().saturating_sub(1));
                            state.section_index = visible_indices[next_pos];
                        } else if let Some(&first_visible) = visible_indices.first() {
                            state.section_index = first_visible;
                        } else if len > 0 {
                            state.section_index = next_index_clamped(idx, len, 1);
                        }
                        state.section_details_field_index = 0;
                    }
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if let Some(state) = &mut self.view_edit_state {
                    if !state.sections_view_row_selected {
                        let at_first_visible = current_visible_pos.map(|p| p == 0).unwrap_or(true);
                        if at_first_visible || visible_indices.is_empty() {
                            state.sections_view_row_selected = true;
                            state.region = ViewEditRegion::Sections;
                        } else {
                            state.region = ViewEditRegion::Sections;
                            if let Some(pos) = current_visible_pos {
                                state.section_index = visible_indices[pos.saturating_sub(1)];
                            } else {
                                state.section_index = visible_indices[0];
                            }
                            state.section_details_field_index = 0;
                        }
                    }
                }
            }
            KeyCode::Char('n') => {
                let filter_active = self
                    .view_edit_state
                    .as_ref()
                    .map(Self::view_edit_filter_is_active)
                    .unwrap_or(false);
                if filter_active {
                    self.clear_view_edit_section_filter();
                }
                let insert_index = if selecting_view_row || len == 0 {
                    0
                } else {
                    (idx + 1).min(len)
                };
                if let Some(new_index) = self.insert_view_edit_section(insert_index) {
                    self.begin_view_edit_section_title_input(new_index);
                }
            }
            KeyCode::Char('N') => {
                let filter_active = self
                    .view_edit_state
                    .as_ref()
                    .map(Self::view_edit_filter_is_active)
                    .unwrap_or(false);
                if filter_active {
                    self.clear_view_edit_section_filter();
                }
                let insert_index = if selecting_view_row || len == 0 {
                    0
                } else {
                    idx.min(len)
                };
                if let Some(new_index) = self.insert_view_edit_section(insert_index) {
                    self.begin_view_edit_section_title_input(new_index);
                }
            }
            KeyCode::Char('x') => {
                if selecting_view_row {
                    return Ok(true);
                }
                self.request_view_edit_section_delete(idx);
            }
            KeyCode::Char('[') | KeyCode::Char('K') => {
                if selecting_view_row {
                    return Ok(true);
                }
                if let Some(state) = &mut self.view_edit_state {
                    if idx > 0 && idx < state.draft.sections.len() {
                        state.draft.sections.swap(idx, idx - 1);
                        state.section_index = idx - 1;
                        state.dirty = true;
                        state.discard_confirm = false;
                    }
                }
            }
            KeyCode::Char(']') | KeyCode::Char('J') => {
                if selecting_view_row {
                    return Ok(true);
                }
                if let Some(state) = &mut self.view_edit_state {
                    if idx + 1 < state.draft.sections.len() {
                        state.draft.sections.swap(idx, idx + 1);
                        state.section_index = idx + 1;
                        state.dirty = true;
                        state.discard_confirm = false;
                    }
                }
            }
            KeyCode::Enter => {
                if selecting_view_row {
                    if let Some(state) = &mut self.view_edit_state {
                        state.region = ViewEditRegion::Criteria;
                        state.pane_focus = ViewEditPaneFocus::Details;
                    }
                    return Ok(true);
                }
                if let Some(state) = &mut self.view_edit_state {
                    if idx < len {
                        if state.section_expanded == Some(idx) {
                            state.section_expanded = None;
                        } else {
                            state.section_expanded = Some(idx);
                        }
                        state.section_details_field_index = 7;
                    }
                }
            }
            KeyCode::Char('t') | KeyCode::Char('e') => {
                if selecting_view_row {
                    return Ok(true);
                }
                if let Some(state) = &mut self.view_edit_state {
                    state.section_details_field_index = 0;
                }
                self.begin_view_edit_section_title_input(idx);
            }
            // Expanded section detail keys
            KeyCode::Char('f') => {
                if selecting_view_row {
                    return Ok(true);
                }
                if idx < len {
                    let first = first_non_reserved_category_index(&self.category_rows);
                    if let Some(state) = &mut self.view_edit_state {
                        state.overlay = Some(ViewEditOverlay::CategoryPicker {
                            target: CategoryEditTarget::SectionCriteria,
                        });
                        state.section_expanded = Some(idx);
                        state.picker_index = first;
                    }
                    self.status = "Edit section criteria: j/k select  Space/Enter:toggle  Esc:done"
                        .to_string();
                }
            }
            KeyCode::Char('a') => {
                if selecting_view_row {
                    return Ok(true);
                }
                if idx < len {
                    let first = first_non_reserved_category_index(&self.category_rows);
                    if let Some(state) = &mut self.view_edit_state {
                        state.overlay = Some(ViewEditOverlay::CategoryPicker {
                            target: CategoryEditTarget::SectionOnInsertAssign,
                        });
                        state.section_expanded = Some(idx);
                        state.picker_index = first;
                    }
                    self.status = "Edit on-insert assign: j/k select  Space/Enter:toggle  Esc:done"
                        .to_string();
                }
            }
            KeyCode::Char('c') | KeyCode::Char('C') => {
                if selecting_view_row {
                    return Ok(true);
                }
                if idx < len {
                    let first = first_non_reserved_category_index(&self.category_rows);
                    if let Some(state) = &mut self.view_edit_state {
                        state.overlay = Some(ViewEditOverlay::CategoryPicker {
                            target: CategoryEditTarget::SectionColumns,
                        });
                        state.section_expanded = Some(idx);
                        state.picker_index = first;
                    }
                    self.status = "Edit section columns: j/k select  Space/Enter:toggle  Esc:done"
                        .to_string();
                }
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                if selecting_view_row {
                    self.begin_view_edit_name_input();
                    return Ok(true);
                }
                if idx < len {
                    let first = first_non_reserved_category_index(&self.category_rows);
                    if let Some(state) = &mut self.view_edit_state {
                        state.overlay = Some(ViewEditOverlay::CategoryPicker {
                            target: CategoryEditTarget::SectionOnRemoveUnassign,
                        });
                        state.section_expanded = Some(idx);
                        state.picker_index = first;
                    }
                    self.status =
                        "Edit on-remove unassign: j/k select  Space/Enter:toggle  Esc:done"
                            .to_string();
                }
            }
            KeyCode::Char('h') => {
                if selecting_view_row {
                    return Ok(true);
                }
                if idx < len {
                    if let Some(state) = &mut self.view_edit_state {
                        if let Some(section) = state.draft.sections.get_mut(idx) {
                            section.show_children = !section.show_children;
                            state.section_expanded = Some(idx);
                            state.dirty = true;
                            state.discard_confirm = false;
                        }
                    }
                }
            }
            KeyCode::Char('m') | KeyCode::Char('M') => {
                if selecting_view_row {
                    return Ok(true);
                }
                if idx < len {
                    if let Some(state) = &mut self.view_edit_state {
                        if let Some(section) = state.draft.sections.get_mut(idx) {
                            section.board_display_mode_override =
                                Self::cycle_section_board_display_mode_override(
                                    section.board_display_mode_override,
                                );
                            state.section_expanded = Some(idx);
                            state.dirty = true;
                            state.discard_confirm = false;
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(true)
    }

    // -------------------------------------------------------------------------
    // Unmatched region
    // -------------------------------------------------------------------------

    fn handle_view_edit_preview_key(&mut self, code: KeyCode) -> Result<bool, String> {
        match code {
            KeyCode::Char('j') | KeyCode::Down => {
                if let Some(state) = &mut self.view_edit_state {
                    state.preview_scroll = state.preview_scroll.saturating_add(1);
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if let Some(state) = &mut self.view_edit_state {
                    state.preview_scroll = state.preview_scroll.saturating_sub(1);
                }
            }
            _ => {}
        }
        Ok(true)
    }

    fn handle_view_edit_unmatched_key(&mut self, code: KeyCode) -> Result<bool, String> {
        match code {
            KeyCode::Char('j') | KeyCode::Down => {
                let current = self
                    .view_edit_state
                    .as_ref()
                    .map(Self::view_details_focus_index)
                    .unwrap_or(0);
                if let Some(state) = &self.view_edit_state {
                    let criteria_rows = Self::view_details_criteria_row_count(state);
                    let max_index = criteria_rows + Self::view_details_aux_field_count() - 1;
                    self.set_view_details_focus_index((current + 1).min(max_index));
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                let current = self
                    .view_edit_state
                    .as_ref()
                    .map(Self::view_details_focus_index)
                    .unwrap_or(0);
                self.set_view_details_focus_index(current.saturating_sub(1));
            }
            KeyCode::Char('t') => {
                if let Some(state) = &mut self.view_edit_state {
                    state.unmatched_field_index = 3;
                    state.draft.show_unmatched = !state.draft.show_unmatched;
                    state.dirty = true;
                    state.discard_confirm = false;
                }
            }
            KeyCode::Char('l') => {
                if let Some(state) = &mut self.view_edit_state {
                    let current = state.draft.unmatched_label.clone();
                    state.unmatched_field_index = 4;
                    state.inline_input = Some(ViewEditInlineInput::UnmatchedLabel);
                    state.inline_buf = text_buffer::TextBuffer::new(current);
                }
                self.status = "Unmatched label: type text  Enter:confirm  Esc:cancel".to_string();
            }
            KeyCode::Char('r') | KeyCode::Char('R') => {
                self.begin_view_edit_name_input();
            }
            KeyCode::Char(']') => {
                if let Some(state) = &mut self.view_edit_state {
                    state.unmatched_field_index = 0;
                    state.overlay = Some(ViewEditOverlay::BucketPicker {
                        target: BucketEditTarget::ViewVirtualInclude,
                    });
                    state.picker_index = 0;
                }
            }
            KeyCode::Char('[') => {
                if let Some(state) = &mut self.view_edit_state {
                    state.unmatched_field_index = 1;
                    state.overlay = Some(ViewEditOverlay::BucketPicker {
                        target: BucketEditTarget::ViewVirtualExclude,
                    });
                    state.picker_index = 0;
                }
            }
            KeyCode::Char('m') | KeyCode::Char('M') => {
                if let Some(state) = &mut self.view_edit_state {
                    state.unmatched_field_index = 2;
                    state.draft.board_display_mode =
                        Self::cycle_view_board_display_mode(state.draft.board_display_mode);
                    state.dirty = true;
                    state.discard_confirm = false;
                }
            }
            KeyCode::Char(' ') | KeyCode::Enter => {
                let target = self
                    .view_edit_state
                    .as_ref()
                    .map(|s| s.unmatched_field_index)
                    .unwrap_or(0);
                match target {
                    0 => {
                        if let Some(state) = &mut self.view_edit_state {
                            state.overlay = Some(ViewEditOverlay::BucketPicker {
                                target: BucketEditTarget::ViewVirtualInclude,
                            });
                            state.picker_index = 0;
                        }
                    }
                    1 => {
                        if let Some(state) = &mut self.view_edit_state {
                            state.overlay = Some(ViewEditOverlay::BucketPicker {
                                target: BucketEditTarget::ViewVirtualExclude,
                            });
                            state.picker_index = 0;
                        }
                    }
                    2 => {
                        if let Some(state) = &mut self.view_edit_state {
                            state.draft.board_display_mode =
                                Self::cycle_view_board_display_mode(state.draft.board_display_mode);
                            state.dirty = true;
                            state.discard_confirm = false;
                        }
                    }
                    3 => {
                        if let Some(state) = &mut self.view_edit_state {
                            state.draft.show_unmatched = !state.draft.show_unmatched;
                            state.dirty = true;
                            state.discard_confirm = false;
                        }
                    }
                    _ => {
                        if let Some(state) = &mut self.view_edit_state {
                            let current = state.draft.unmatched_label.clone();
                            state.unmatched_field_index = 4;
                            state.inline_input = Some(ViewEditInlineInput::UnmatchedLabel);
                            state.inline_buf = text_buffer::TextBuffer::new(current);
                        }
                        self.status =
                            "Unmatched label: type text  Enter:confirm  Esc:cancel".to_string();
                    }
                }
            }
            _ => {}
        }
        Ok(true)
    }
}
