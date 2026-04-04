use super::super::*;

#[derive(Clone)]
pub(crate) struct CategoryListRow {
    pub(crate) id: CategoryId,
    pub(crate) name: String,
    pub(crate) depth: usize,
    pub(crate) is_reserved: bool,
    pub(crate) has_note: bool,
    pub(crate) is_exclusive: bool,
    pub(crate) is_actionable: bool,
    pub(crate) enable_implicit_string: bool,
    pub(crate) enable_semantic_classification: bool,
    pub(crate) match_category_name: bool,
    pub(crate) value_kind: CategoryValueKind,
    pub(crate) condition_count: usize,
    pub(crate) action_count: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum CategoryManagerFocus {
    Tree,
    Filter,
    Details,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum CategoryManagerDetailsFocus {
    Exclusive,
    AutoMatch,
    SemanticMatch,
    MatchCategoryName,
    Actionable,
    AlsoMatch,
    Conditions,
    Actions,
    Integer,
    DecimalPlaces,
    CurrencySymbol,
    ThousandsSeparator,
    Note,
}

impl CategoryManagerDetailsFocus {
    pub(crate) fn next(self, is_numeric: bool, integer_mode: bool) -> Self {
        if is_numeric {
            match self {
                Self::Integer => {
                    if integer_mode {
                        Self::CurrencySymbol
                    } else {
                        Self::DecimalPlaces
                    }
                }
                Self::DecimalPlaces => Self::CurrencySymbol,
                Self::CurrencySymbol => Self::ThousandsSeparator,
                Self::ThousandsSeparator => Self::Note,
                Self::Note => Self::Integer,
                _ => Self::Integer,
            }
        } else {
            match self {
                Self::Exclusive => Self::AutoMatch,
                Self::AutoMatch => Self::SemanticMatch,
                Self::SemanticMatch => Self::MatchCategoryName,
                Self::MatchCategoryName => Self::Actionable,
                Self::Actionable => Self::AlsoMatch,
                Self::AlsoMatch => Self::Conditions,
                Self::Conditions => Self::Actions,
                Self::Actions => Self::Note,
                Self::Note => Self::Exclusive,
                _ => Self::Exclusive,
            }
        }
    }

    pub(crate) fn prev(self, is_numeric: bool, integer_mode: bool) -> Self {
        if is_numeric {
            match self {
                Self::Integer => Self::Note,
                Self::DecimalPlaces => Self::Integer,
                Self::CurrencySymbol => {
                    if integer_mode {
                        Self::Integer
                    } else {
                        Self::DecimalPlaces
                    }
                }
                Self::ThousandsSeparator => Self::CurrencySymbol,
                Self::Note => Self::ThousandsSeparator,
                _ => Self::Integer,
            }
        } else {
            match self {
                Self::Exclusive => Self::Note,
                Self::AutoMatch => Self::Exclusive,
                Self::SemanticMatch => Self::AutoMatch,
                Self::MatchCategoryName => Self::SemanticMatch,
                Self::Actionable => Self::MatchCategoryName,
                Self::AlsoMatch => Self::Actionable,
                Self::Conditions => Self::AlsoMatch,
                Self::Actions => Self::Conditions,
                Self::Note => Self::Actions,
                _ => Self::Actionable,
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum CategoryManagerDetailsInlineField {
    DecimalPlaces,
    CurrencySymbol,
}

#[derive(Clone)]
pub(crate) struct CategoryManagerDetailsInlineInput {
    pub(crate) field: CategoryManagerDetailsInlineField,
    pub(crate) buffer: text_buffer::TextBuffer,
}

#[derive(Clone)]
pub(crate) enum CategoryInlineAction {
    Rename {
        category_id: CategoryId,
        original_name: String,
        buf: text_buffer::TextBuffer,
    },
    DeleteConfirm {
        category_id: CategoryId,
        category_name: String,
    },
}

#[derive(Clone)]
pub(crate) struct ConditionEditState {
    pub(crate) condition_index: Option<usize>,
    pub(crate) draft_query: Query,
    pub(crate) list_index: usize,
    pub(crate) picker_open: bool,
    pub(crate) picker_index: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ActionEditKind {
    Assign,
    Remove,
}

impl ActionEditKind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Assign => "Assign",
            Self::Remove => "Remove",
        }
    }
}

#[derive(Clone)]
pub(crate) struct ActionEditState {
    pub(crate) action_index: Option<usize>,
    pub(crate) draft_kind: ActionEditKind,
    pub(crate) draft_targets: HashSet<CategoryId>,
    pub(crate) list_index: usize,
    pub(crate) picker_open: bool,
    pub(crate) picker_index: usize,
}

#[derive(Clone)]
pub(crate) struct CategoryManagerState {
    pub(crate) focus: CategoryManagerFocus,
    pub(crate) filter: text_buffer::TextBuffer,
    pub(crate) filter_editing: bool,
    pub(crate) structure_move_prefix: Option<char>,
    pub(crate) discard_confirm: bool,
    pub(crate) details_focus: CategoryManagerDetailsFocus,
    pub(crate) details_note_category_id: Option<CategoryId>,
    pub(crate) details_note: text_buffer::TextBuffer,
    pub(crate) details_note_dirty: bool,
    pub(crate) details_note_editing: bool,
    pub(crate) details_also_match_category_id: Option<CategoryId>,
    pub(crate) details_also_match: text_buffer::TextBuffer,
    pub(crate) details_also_match_dirty: bool,
    pub(crate) details_also_match_editing: bool,
    pub(crate) details_inline_input: Option<CategoryManagerDetailsInlineInput>,
    pub(crate) tree_index: usize,
    pub(crate) visible_row_indices: Vec<usize>,
    pub(crate) selected_category_id: Option<CategoryId>,
    pub(crate) inline_action: Option<CategoryInlineAction>,
    pub(crate) condition_edit: Option<ConditionEditState>,
    pub(crate) action_edit: Option<ActionEditState>,
}

#[derive(Clone, Debug)]
pub(crate) struct CategorySuggestState {
    #[allow(dead_code)]
    pub(crate) suggest_index: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct WorkflowRolePickerState {
    pub(crate) role_index: usize,
    pub(crate) row_index: usize,
    pub(crate) origin: WorkflowRolePickerOrigin,
    pub(crate) scroll_offset: ScrollCell<usize>,
}

#[derive(Clone, Debug)]
pub(crate) struct OllamaModelPickerState {
    pub(crate) models: Vec<String>,
    pub(crate) selected_index: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum WorkflowRolePickerOrigin {
    CategoryManager,
    GlobalSettings,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum GlobalSettingsRow {
    AutoRefresh,
    SectionBorders,
    NoteGlyphs,
    LiteralClassificationMode,
    SemanticClassificationMode,
    SemanticProvider,
    OllamaBaseUrl,
    OllamaModel,
    OllamaTimeout,
    OpenRouterModel,
    OpenRouterTimeout,
    OpenAiModel,
    OpenAiTimeout,
    WorkflowReady,
    WorkflowClaim,
}

impl GlobalSettingsRow {
    pub(crate) fn visible_rows(
        provider: agenda_core::classification::SemanticProviderKind,
    ) -> Vec<Self> {
        use agenda_core::classification::SemanticProviderKind;
        let mut rows = vec![
            Self::AutoRefresh,
            Self::SectionBorders,
            Self::NoteGlyphs,
            Self::LiteralClassificationMode,
            Self::SemanticClassificationMode,
            Self::SemanticProvider,
        ];
        match provider {
            SemanticProviderKind::Ollama => {
                rows.extend_from_slice(&[
                    Self::OllamaBaseUrl,
                    Self::OllamaModel,
                    Self::OllamaTimeout,
                ]);
            }
            SemanticProviderKind::OpenRouter => {
                rows.extend_from_slice(&[Self::OpenRouterModel, Self::OpenRouterTimeout]);
            }
            SemanticProviderKind::OpenAi => {
                rows.extend_from_slice(&[Self::OpenAiModel, Self::OpenAiTimeout]);
            }
        }
        rows.extend_from_slice(&[Self::WorkflowReady, Self::WorkflowClaim]);
        rows
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct GlobalSettingsState {
    pub(crate) selected_row: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum CategoryDirectEditFocus {
    Entries,
    Input,
    Suggestions,
}

impl CategoryDirectEditFocus {
    pub(crate) fn next(self) -> Self {
        match self {
            Self::Entries => Self::Input,
            Self::Input => Self::Suggestions,
            Self::Suggestions => Self::Entries,
        }
    }

    pub(crate) fn prev(self) -> Self {
        match self {
            Self::Entries => Self::Suggestions,
            Self::Input => Self::Entries,
            Self::Suggestions => Self::Input,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct CategoryDirectEditAnchor {
    pub(crate) slot_index: usize,
    pub(crate) section_index: usize,
    pub(crate) section_column_index: usize,
    pub(crate) board_column_index: usize,
    pub(crate) is_generated_section: bool,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub(crate) struct CategoryDirectEditColumnMeta {
    pub(crate) parent_id: CategoryId,
    pub(crate) parent_name: String,
    pub(crate) column_kind: ColumnKind,
    pub(crate) anchor: CategoryDirectEditAnchor,
    pub(crate) item_id: ItemId,
    pub(crate) item_label: String,
}

#[derive(Clone)]
pub(crate) struct CategoryDirectEditRow {
    pub(crate) input: text_buffer::TextBuffer,
    pub(crate) category_id: Option<CategoryId>,
}

#[derive(Clone)]
pub(crate) struct CategoryDirectEditState {
    #[allow(dead_code)]
    pub(crate) anchor: CategoryDirectEditAnchor,
    pub(crate) parent_id: CategoryId,
    pub(crate) parent_name: String,
    pub(crate) item_id: ItemId,
    pub(crate) item_label: String,
    pub(crate) rows: Vec<CategoryDirectEditRow>,
    pub(crate) active_row: usize,
    pub(crate) focus: CategoryDirectEditFocus,
    pub(crate) suggest_index: usize,
    pub(crate) create_confirm_name: Option<String>,
    pub(crate) original_category_ids: Vec<Option<CategoryId>>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum CategoryColumnPickerFocus {
    FilterInput,
    List,
}

#[derive(Clone)]
pub(crate) struct CategoryColumnPickerState {
    #[allow(dead_code)]
    pub(crate) anchor: CategoryDirectEditAnchor,
    pub(crate) parent_id: CategoryId,
    pub(crate) parent_name: String,
    pub(crate) item_id: ItemId,
    pub(crate) item_label: String,
    pub(crate) item_preview_scroll: u16,
    pub(crate) is_exclusive: bool,
    pub(crate) filter: text_buffer::TextBuffer,
    pub(crate) focus: CategoryColumnPickerFocus,
    pub(crate) list_index: usize,
    pub(crate) selected_ids: HashSet<CategoryId>,
    pub(crate) create_confirm_name: Option<String>,
}

impl CategoryDirectEditRow {
    pub(crate) fn blank() -> Self {
        Self {
            input: text_buffer::TextBuffer::empty(),
            category_id: None,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn resolved(category_id: CategoryId, name: String) -> Self {
        Self {
            input: text_buffer::TextBuffer::new(name),
            category_id: Some(category_id),
        }
    }
}

impl CategoryDirectEditState {
    pub(crate) fn active_row(&self) -> Option<&CategoryDirectEditRow> {
        self.rows.get(self.active_row)
    }

    pub(crate) fn active_row_mut(&mut self) -> Option<&mut CategoryDirectEditRow> {
        self.rows.get_mut(self.active_row)
    }

    pub(crate) fn clamp_active_row(&mut self) {
        if self.rows.is_empty() {
            self.active_row = 0;
            return;
        }
        self.active_row = self.active_row.min(self.rows.len() - 1);
    }

    pub(crate) fn add_blank_row(&mut self) -> usize {
        self.rows.push(CategoryDirectEditRow::blank());
        self.active_row = self.rows.len().saturating_sub(1);
        self.active_row
    }

    pub(crate) fn remove_row(&mut self, index: usize) -> Option<CategoryDirectEditRow> {
        if index >= self.rows.len() {
            return None;
        }
        let removed = self.rows.remove(index);
        if index < self.active_row {
            self.active_row = self.active_row.saturating_sub(1);
        }
        self.ensure_one_row();
        self.clamp_active_row();
        Some(removed)
    }

    pub(crate) fn ensure_one_row(&mut self) {
        if self.rows.is_empty() {
            self.rows.push(CategoryDirectEditRow::blank());
            self.active_row = 0;
        } else {
            self.clamp_active_row();
        }
    }

    pub(crate) fn row_would_duplicate_category_id(
        &self,
        row_index: usize,
        category_id: CategoryId,
    ) -> bool {
        self.rows.iter().enumerate().any(|(idx, row)| {
            idx != row_index && row.category_id.map(|id| id == category_id).unwrap_or(false)
        })
    }

    pub(crate) fn has_duplicate_resolved_category_ids(&self) -> bool {
        let mut seen = HashSet::new();
        self.rows
            .iter()
            .filter_map(|row| row.category_id)
            .any(|category_id| !seen.insert(category_id))
    }
}
