use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::sync::Arc;

use jiff::Timestamp;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::classification::{
    BackgroundClassificationJob, ClassificationCandidate, ClassificationConfig,
    ClassificationService, ClassificationSuggestion, ImplicitStringProvider,
    LiteralClassificationMode, OllamaProvider, OllamaTransport, OpenAiProvider, OpenAiTransport,
    OpenRouterProvider, OpenRouterTransport, ReqwestOllamaTransport, ReqwestOpenAiTransport,
    ReqwestOpenRouterTransport, SemanticClassificationMode, SemanticProviderKind,
    SuggestionStatus, WhenParserProvider, CLASSIFICATION_DEBUG_LOG_PATH,
    PROVIDER_ID_IMPLICIT_STRING, PROVIDER_ID_OLLAMA_OPENAI_COMPAT, PROVIDER_ID_OPENAI,
    PROVIDER_ID_OPENROUTER, PROVIDER_ID_WHEN_PARSER,
};
use crate::dates::BasicDateParser;
use crate::engine::{
    evaluate_all_items_with_options, process_item_with_options, EvaluateAllItemsResult,
    ProcessItemResult, ProcessOptions,
};
use crate::error::{AgendaError, Result};
use crate::matcher::Classifier;
use crate::model::{
    origin as origin_const, Action, Assignment, AssignmentActionKind, AssignmentEvent,
    AssignmentEventKind, AssignmentExplanation, AssignmentSource, Category, CategoryId,
    CategoryValueKind, Condition, Item, ItemId, ItemLink, ItemLinkKind, ItemLinksForItem, Section,
    View, RESERVED_CATEGORY_NAME_DONE, RESERVED_CATEGORY_NAME_ENTRY, RESERVED_CATEGORY_NAME_WHEN,
};
use crate::store::Store;
use crate::workflow::{
    claimability_for_item, resolve_workflow_config, workflow_setup_error_message, Claimability,
    ResolvedWorkflowConfig,
};

/// Synchronous integration layer that wires Store mutations to engine execution.
pub struct Agenda<'a> {
    store: &'a Store,
    classifier: &'a dyn Classifier,
    date_parser: BasicDateParser,
    ollama_transport: Arc<dyn OllamaTransport>,
    openrouter_transport: Arc<dyn OpenRouterTransport>,
    openai_transport: Arc<dyn OpenAiTransport>,
    debug: bool,
}

/// The net category changes that would result from a section-move operation,
/// as computed by [`Agenda::preview_section_move`].
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct SectionMovePreview {
    /// Categories that would be assigned.
    pub to_assign: HashSet<CategoryId>,
    /// Categories that would be unassigned.
    pub to_unassign: HashSet<CategoryId>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct LinkItemsResult {
    pub created: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CandidateDisposition {
    Skip,
    AutoApply,
    QueueReview,
}

impl<'a> Agenda<'a> {
    pub fn new(store: &'a Store, classifier: &'a dyn Classifier) -> Self {
        Self::with_transports(
            store,
            classifier,
            Arc::new(ReqwestOllamaTransport),
            Arc::new(ReqwestOpenRouterTransport),
            Arc::new(ReqwestOpenAiTransport),
        )
    }

    pub fn with_debug(store: &'a Store, classifier: &'a dyn Classifier, debug: bool) -> Self {
        let mut agenda = Self::new(store, classifier);
        agenda.debug = debug;
        agenda
    }

    pub fn with_ollama_transport(
        store: &'a Store,
        classifier: &'a dyn Classifier,
        ollama_transport: Arc<dyn OllamaTransport>,
    ) -> Self {
        Self::with_transports(
            store,
            classifier,
            ollama_transport,
            Arc::new(ReqwestOpenRouterTransport),
            Arc::new(ReqwestOpenAiTransport),
        )
    }

    pub fn with_transports(
        store: &'a Store,
        classifier: &'a dyn Classifier,
        ollama_transport: Arc<dyn OllamaTransport>,
        openrouter_transport: Arc<dyn OpenRouterTransport>,
        openai_transport: Arc<dyn OpenAiTransport>,
    ) -> Self {
        Self {
            store,
            classifier,
            date_parser: BasicDateParser::default(),
            ollama_transport,
            openrouter_transport,
            openai_transport,
            debug: false,
        }
    }

    pub fn store(&self) -> &Store {
        self.store
    }

    pub fn debug_enabled(&self) -> bool {
        self.debug
    }

    pub fn create_item(&self, item: &Item) -> Result<ProcessItemResult> {
        self.create_item_with_reference_date(item, jiff::Zoned::now().date())
    }

    pub fn create_item_with_reference_date(
        &self,
        item: &Item,
        reference_date: jiff::civil::Date,
    ) -> Result<ProcessItemResult> {
        let item_to_create = item.clone();
        self.store.create_item(&item_to_create)?;
        self.sync_when_assignment(item_to_create.id, item_to_create.when_date, None)?;
        self.process_item_save(item_to_create.id, reference_date, true)
    }

    /// Like `create_item_with_reference_date` but skips expensive providers
    /// (Ollama). Callers are expected to submit background classification
    /// separately for the semantic/LLM path.
    pub fn create_item_cheap(
        &self,
        item: &Item,
        reference_date: jiff::civil::Date,
    ) -> Result<ProcessItemResult> {
        let item_to_create = item.clone();
        self.store.create_item(&item_to_create)?;
        self.sync_when_assignment(item_to_create.id, item_to_create.when_date, None)?;
        self.process_item_save_cheap(item_to_create.id, reference_date, true)
    }

    pub fn update_item(&self, item: &Item) -> Result<ProcessItemResult> {
        self.update_item_with_reference_date(item, jiff::Zoned::now().date())
    }

    pub fn update_item_with_reference_date(
        &self,
        item: &Item,
        reference_date: jiff::civil::Date,
    ) -> Result<ProcessItemResult> {
        self.update_item_inner(item, reference_date, true)
    }

    /// Like `update_item_with_reference_date` but skips expensive providers
    /// (Ollama). Callers are expected to submit background classification
    /// separately for the semantic/LLM path.
    pub fn update_item_cheap(
        &self,
        item: &Item,
        reference_date: jiff::civil::Date,
    ) -> Result<ProcessItemResult> {
        self.update_item_inner(item, reference_date, false)
    }

    fn update_item_inner(
        &self,
        item: &Item,
        reference_date: jiff::civil::Date,
        include_semantic: bool,
    ) -> Result<ProcessItemResult> {
        let item_to_update = item.clone();
        let existing = self.store.get_item(item.id)?;
        let text_changed = item_to_update.text != existing.text;
        self.store.update_item(&item_to_update)?;
        self.sync_when_assignment(item_to_update.id, item_to_update.when_date, None)?;
        if include_semantic {
            self.process_item_save(item_to_update.id, reference_date, text_changed)
        } else {
            self.process_item_save_cheap(item_to_update.id, reference_date, text_changed)
        }
    }

    pub fn set_item_when_date(
        &self,
        item_id: ItemId,
        when_date: Option<jiff::civil::DateTime>,
        origin: Option<String>,
    ) -> Result<ProcessItemResult> {
        let mut item = self.store.get_item(item_id)?;
        item.when_date = when_date;
        item.modified_at = Timestamp::now();
        self.store.update_item(&item)?;

        let when_assignment = when_date.map(|_| Assignment {
            source: AssignmentSource::Manual,
            assigned_at: Timestamp::now(),
            sticky: true,
            origin: origin
                .clone()
                .or_else(|| Some(origin_const::MANUAL_WHEN.to_string())),
            explanation: Some(AssignmentExplanation::Manual { origin }),
            numeric_value: None,
        });
        self.sync_when_assignment(item_id, when_date, when_assignment)?;

        self.reprocess_existing_item(item_id)
    }

    pub fn create_category(&self, category: &Category) -> Result<EvaluateAllItemsResult> {
        self.store.create_category(category)?;
        self.process_category_change(category.id)
    }

    pub fn update_category(&self, category: &Category) -> Result<EvaluateAllItemsResult> {
        self.store.update_category(category)?;
        self.process_category_change(category.id)
    }

    pub fn move_category_within_parent(&self, category_id: CategoryId, delta: i32) -> Result<()> {
        self.store.move_category_within_parent(category_id, delta)
    }

    pub fn move_category_to_parent(
        &self,
        category_id: CategoryId,
        new_parent_id: Option<CategoryId>,
        insert_index: Option<usize>,
    ) -> Result<EvaluateAllItemsResult> {
        self.store
            .move_category_to_parent(category_id, new_parent_id, insert_index)?;
        self.process_category_change(category_id)
    }

    pub fn assign_item_manual(
        &self,
        item_id: ItemId,
        category_id: CategoryId,
        origin: Option<String>,
    ) -> Result<ProcessItemResult> {
        self.enforce_manual_exclusive_siblings(item_id, category_id)?;

        let assignment = Assignment {
            source: AssignmentSource::Manual,
            assigned_at: Timestamp::now(),
            sticky: true,
            origin: origin
                .clone()
                .or_else(|| Some(origin_const::MANUAL.to_string())),
            explanation: Some(AssignmentExplanation::Manual { origin }),
            numeric_value: None,
        };

        self.store.assign_item(item_id, category_id, &assignment)?;
        self.assign_subsumption_for_category(item_id, category_id)?;
        let mut result = self.reprocess_existing_item(item_id)?;
        self.prepend_assignment_event(
            &mut result,
            AssignmentEventKind::Assigned,
            category_id,
            "Assigned manually",
        );
        self.debug_log_process_result("assign.manual", item_id, &result);
        Ok(result)
    }

    pub fn claim_item_manual(
        &self,
        item_id: ItemId,
        category_id: CategoryId,
        must_not_have_category_ids: &[CategoryId],
        origin: Option<String>,
    ) -> Result<ProcessItemResult> {
        self.store.with_immediate_transaction(|store| {
            let _ = store.get_item(item_id)?;
            let assignments = store.get_assignments_for_item(item_id)?;
            for blocked_category_id in must_not_have_category_ids {
                if assignments.contains_key(blocked_category_id) {
                    let blocked_category_name = store
                        .get_category(*blocked_category_id)
                        .map(|category| category.name)
                        .unwrap_or_else(|_| blocked_category_id.to_string());
                    return Err(AgendaError::InvalidOperation {
                        message: format!(
                            "claim precondition failed: item {item_id} already has category '{blocked_category_name}'"
                        ),
                    });
                }
            }
            self.assign_item_manual(item_id, category_id, origin.clone())
        })
    }

    pub fn claim_item_workflow(&self, item_id: ItemId) -> Result<ProcessItemResult> {
        let Some(workflow) = resolve_workflow_config(self.store)? else {
            return Err(AgendaError::InvalidOperation {
                message: workflow_setup_error_message().to_string(),
            });
        };

        self.store.with_immediate_transaction(|store| {
            let item = store.get_item(item_id)?;
            match claimability_for_item(store, &item, workflow)? {
                Claimability::Claimable => self.assign_item_manual(
                    item_id,
                    workflow.claim_category_id,
                    Some("manual:cli.claim".to_string()),
                ),
                outcome => Err(AgendaError::InvalidOperation {
                    message: outcome
                        .error_message()
                        .unwrap_or("claim precondition failed")
                        .to_string(),
                }),
            }
        })
    }

    pub fn release_item_claim(&self, item_id: ItemId) -> Result<ProcessItemResult> {
        let Some(workflow) = resolve_workflow_config(self.store)? else {
            return Err(AgendaError::InvalidOperation {
                message: workflow_setup_error_message().to_string(),
            });
        };

        let item = self.store.get_item(item_id)?;
        if !item.assignments.contains_key(&workflow.claim_category_id) {
            return Err(AgendaError::InvalidOperation {
                message: "release precondition failed: item is not currently claimed".to_string(),
            });
        }

        self.store
            .unassign_item(item_id, workflow.claim_category_id)?;
        self.reprocess_existing_item(item_id)
    }

    pub fn assign_item_numeric_manual(
        &self,
        item_id: ItemId,
        category_id: CategoryId,
        numeric_value: Decimal,
        origin: Option<String>,
    ) -> Result<ProcessItemResult> {
        let category = self.store.get_category(category_id)?;
        if category.value_kind != CategoryValueKind::Numeric {
            return Err(AgendaError::InvalidOperation {
                message: format!("category '{}' is not Numeric", category.name),
            });
        }

        self.enforce_manual_exclusive_siblings(item_id, category_id)?;

        let assignment = Assignment {
            source: AssignmentSource::Manual,
            assigned_at: Timestamp::now(),
            sticky: true,
            origin: origin
                .clone()
                .or_else(|| Some(origin_const::MANUAL_NUMERIC.to_string())),
            explanation: Some(AssignmentExplanation::Manual { origin }),
            numeric_value: Some(numeric_value),
        };

        self.store.assign_item(item_id, category_id, &assignment)?;
        self.assign_subsumption_for_category(item_id, category_id)?;
        let mut result = self.reprocess_existing_item(item_id)?;
        self.prepend_assignment_event(
            &mut result,
            AssignmentEventKind::Assigned,
            category_id,
            "Assigned manually",
        );
        self.debug_log_process_result("assign.manual.numeric", item_id, &result);
        Ok(result)
    }

    pub fn unassign_item_manual(
        &self,
        item_id: ItemId,
        category_id: CategoryId,
    ) -> Result<ProcessItemResult> {
        if let Some(blocking_descendant_id) =
            self.first_assigned_descendant(item_id, category_id)?
        {
            let hierarchy = self.store.get_hierarchy()?;
            let names: HashMap<CategoryId, String> = hierarchy
                .into_iter()
                .map(|category| (category.id, category.name))
                .collect();
            let ancestor_name = names
                .get(&category_id)
                .cloned()
                .unwrap_or_else(|| category_id.to_string());
            let descendant_name = names
                .get(&blocking_descendant_id)
                .cloned()
                .unwrap_or_else(|| blocking_descendant_id.to_string());
            return Err(AgendaError::InvalidOperation {
                message: format!(
                    "cannot remove category '{ancestor_name}' while descendant '{descendant_name}' is assigned; remove descendant first"
                ),
            });
        }
        self.store.unassign_item(item_id, category_id)?;
        let mut result = self.reprocess_existing_item(item_id)?;
        let category_name = self
            .store
            .get_category(category_id)
            .map(|category| category.name)
            .unwrap_or_else(|_| category_id.to_string());
        result.assignment_events.insert(
            0,
            AssignmentEvent {
                kind: AssignmentEventKind::Removed,
                category_id,
                category_name,
                summary: "Removed manually".to_string(),
            },
        );
        self.debug_log_process_result("remove.manual", item_id, &result);
        Ok(result)
    }

    pub fn insert_item_in_section(
        &self,
        item_id: ItemId,
        view: &View,
        section: &Section,
    ) -> Result<ProcessItemResult> {
        let targets = Self::section_insert_targets(view, section);

        self.assign_manual_categories(item_id, &targets, "edit:section.insert")?;
        self.reprocess_existing_item(item_id)
    }

    pub fn insert_item_in_unmatched(
        &self,
        item_id: ItemId,
        view: &View,
    ) -> Result<ProcessItemResult> {
        let view_include: HashSet<CategoryId> = view.criteria.and_category_ids().collect();
        self.assign_manual_categories(item_id, &view_include, "edit:view.insert")?;
        self.reprocess_existing_item(item_id)
    }

    pub fn remove_item_from_section(
        &self,
        item_id: ItemId,
        view: &View,
        section: &Section,
    ) -> Result<ProcessItemResult> {
        let targets = Self::section_remove_targets(view, section);
        self.unassign_categories(item_id, &targets)?;
        self.reprocess_existing_item(item_id)
    }

    pub fn move_item_between_sections(
        &self,
        item_id: ItemId,
        view: &View,
        from_section: &Section,
        to_section: &Section,
    ) -> Result<ProcessItemResult> {
        let mut to_unassign = Self::section_structural_targets(from_section);
        let preserve = Self::section_insert_targets(view, to_section);
        to_unassign.retain(|category_id| !preserve.contains(category_id));
        to_unassign.extend(from_section.on_remove_unassign.iter().copied());

        let to_assign = Self::section_insert_targets(view, to_section);

        self.unassign_categories(item_id, &to_unassign)?;
        self.assign_manual_categories(item_id, &to_assign, "edit:section.move")?;
        self.reprocess_existing_item(item_id)
    }

    /// Compute the net category changes that would result from moving an item
    /// to a different view placement, without touching the store.
    ///
    /// * `from_section` — the section the item currently occupies in `view`,
    ///   or `None` if the item is unmatched / not present in the view.
    /// * `to_section`   — the target section, or `None` for the unmatched slot.
    ///
    /// Returns a [`SectionMovePreview`] describing which categories would be
    /// assigned and which would be unassigned. Categories that appear in both
    /// sets cancel out and are excluded from the result.
    pub fn preview_section_move(
        view: &View,
        from_section: Option<&Section>,
        to_section: Option<&Section>,
    ) -> SectionMovePreview {
        let (mut to_assign, mut to_unassign) = match (from_section, to_section) {
            (Some(from), Some(to)) => {
                let mut unassign = Self::section_structural_targets(from);
                let preserve = Self::section_insert_targets(view, to);
                unassign.retain(|id| !preserve.contains(id));
                unassign.extend(from.on_remove_unassign.iter().copied());
                let assign = Self::section_insert_targets(view, to);
                (assign, unassign)
            }
            (None, Some(to)) => {
                let assign = Self::section_insert_targets(view, to);
                (assign, HashSet::new())
            }
            (Some(from), None) => {
                let unassign = Self::section_remove_targets(view, from);
                (HashSet::new(), unassign)
            }
            (None, None) => (HashSet::new(), HashSet::new()),
        };

        // Categories that are both assigned and unassigned cancel out.
        let overlap: HashSet<_> = to_assign.intersection(&to_unassign).copied().collect();
        to_assign.retain(|id| !overlap.contains(id));
        to_unassign.retain(|id| !overlap.contains(id));

        SectionMovePreview {
            to_assign,
            to_unassign,
        }
    }

    pub fn remove_item_from_view(&self, item_id: ItemId, view: &View) -> Result<ProcessItemResult> {
        self.unassign_categories(item_id, &view.remove_from_view_unassign)?;
        self.reprocess_existing_item(item_id)
    }

    pub fn preview_manual_category_toggle(
        &self,
        item_id: ItemId,
        category_id: CategoryId,
    ) -> Result<Item> {
        let source_item = self.store.get_item(item_id)?;
        let preview_store = Store::open_memory()?;
        preview_store.set_classification_config(&self.store.get_classification_config()?)?;

        let source_categories = self.store.get_hierarchy()?;
        let id_map = Self::copy_preview_categories(&preview_store, &source_categories)?;

        let mut preview_item = source_item.clone();
        preview_item.assignments.clear();
        preview_store.create_item(&preview_item)?;
        for (assigned_category_id, assignment) in &source_item.assignments {
            if let Some(preview_category_id) = id_map.get(assigned_category_id) {
                preview_store.assign_item(item_id, *preview_category_id, assignment)?;
            }
        }

        let preview_agenda = Agenda::new(&preview_store, self.classifier);
        let preview_category_id =
            Self::mapped_category_id(category_id, &id_map, "preview toggle category")?;

        if source_item.assignments.contains_key(&category_id) {
            preview_agenda.unassign_item_manual(item_id, preview_category_id)?;
        } else {
            preview_agenda.assign_item_manual(
                item_id,
                preview_category_id,
                Some("preview:tui.assign".to_string()),
            )?;
        }

        let mut preview_result = preview_store.get_item(item_id)?;
        preview_result.assignments = preview_result
            .assignments
            .into_iter()
            .map(|(preview_id, assignment)| {
                let source_id =
                    Self::source_category_id(preview_id, &id_map, "preview result assignment")
                        .expect("preview category mapping should round-trip");
                (source_id, assignment)
            })
            .collect();
        Ok(preview_result)
    }

    pub fn remove_item_from_unmatched(
        &self,
        item_id: ItemId,
        view: &View,
    ) -> Result<ProcessItemResult> {
        self.unassign_categories(item_id, &view.remove_from_view_unassign)?;
        self.reprocess_existing_item(item_id)
    }

    pub fn mark_item_done(&self, item_id: ItemId) -> Result<ProcessItemResult> {
        if !self.item_is_actionable(item_id)? {
            return Err(AgendaError::InvalidOperation {
                message: "selected item has no actionable categories".to_string(),
            });
        }
        let mut item = self.store.get_item(item_id)?;
        let now = Timestamp::now();
        let done_at = now
            .to_zoned(jiff::tz::TimeZone::UTC)
            .datetime()
            .round(jiff::Unit::Second)
            .unwrap_or_else(|_| now.to_zoned(jiff::tz::TimeZone::UTC).datetime());
        item.is_done = true;
        item.done_date = Some(done_at);
        item.modified_at = now;
        self.store.update_item(&item)?;

        let done_category_id = self.done_category_id()?;
        let assignment = Assignment {
            source: AssignmentSource::Manual,
            assigned_at: now,
            sticky: true,
            origin: Some(origin_const::MANUAL_DONE.to_string()),
            explanation: Some(AssignmentExplanation::Manual {
                origin: Some(origin_const::MANUAL_DONE.to_string()),
            }),
            numeric_value: None,
        };
        self.store
            .assign_item(item_id, done_category_id, &assignment)?;
        self.clear_claim_assignment_if_configured(item_id)?;
        self.reprocess_existing_item(item_id)
    }

    pub fn mark_item_not_done(&self, item_id: ItemId) -> Result<ProcessItemResult> {
        let mut item = self.store.get_item(item_id)?;
        item.is_done = false;
        item.done_date = None;
        item.modified_at = Timestamp::now();
        self.store.update_item(&item)?;
        let done_category_id = self.done_category_id()?;
        self.store.unassign_item(item_id, done_category_id)?;
        self.reprocess_existing_item(item_id)
    }

    pub fn toggle_item_done(&self, item_id: ItemId) -> Result<ProcessItemResult> {
        let item = self.store.get_item(item_id)?;
        if item.is_done {
            return self.mark_item_not_done(item_id);
        }
        self.mark_item_done(item_id)
    }

    pub fn delete_item(&self, item_id: ItemId, deleted_by: &str) -> Result<()> {
        self.store.delete_item(item_id, deleted_by)
    }

    pub fn link_items_depends_on(
        &self,
        dependent_id: ItemId,
        dependency_id: ItemId,
    ) -> Result<LinkItemsResult> {
        self.ensure_not_self_link(dependent_id, dependency_id, "depends-on")?;
        self.ensure_item_exists(dependent_id)?;
        self.ensure_item_exists(dependency_id)?;

        if self
            .store
            .item_link_exists(dependent_id, dependency_id, ItemLinkKind::DependsOn)?
        {
            return Ok(LinkItemsResult { created: false });
        }

        self.ensure_depends_on_no_cycle(dependent_id, dependency_id)?;
        let link = self.build_link(dependent_id, dependency_id, ItemLinkKind::DependsOn);
        self.store.create_item_link(&link)?;
        self.debug_log_link_event(
            "link.created",
            dependent_id,
            dependency_id,
            ItemLinkKind::DependsOn,
        );
        Ok(LinkItemsResult { created: true })
    }

    pub fn link_items_blocks(
        &self,
        blocker_id: ItemId,
        blocked_id: ItemId,
    ) -> Result<LinkItemsResult> {
        self.link_items_depends_on(blocked_id, blocker_id)
    }

    pub fn link_items_related(&self, a: ItemId, b: ItemId) -> Result<LinkItemsResult> {
        self.ensure_not_self_link(a, b, "related")?;
        self.ensure_item_exists(a)?;
        self.ensure_item_exists(b)?;

        let (item_id, other_item_id) = Self::normalize_related_pair(a, b);
        if self
            .store
            .item_link_exists(item_id, other_item_id, ItemLinkKind::Related)?
        {
            return Ok(LinkItemsResult { created: false });
        }

        let link = self.build_link(item_id, other_item_id, ItemLinkKind::Related);
        self.store.create_item_link(&link)?;
        self.debug_log_link_event(
            "link.created",
            item_id,
            other_item_id,
            ItemLinkKind::Related,
        );
        Ok(LinkItemsResult { created: true })
    }

    pub fn unlink_items_depends_on(
        &self,
        dependent_id: ItemId,
        dependency_id: ItemId,
    ) -> Result<()> {
        self.store
            .delete_item_link(dependent_id, dependency_id, ItemLinkKind::DependsOn)?;
        self.debug_log_link_event(
            "link.removed",
            dependent_id,
            dependency_id,
            ItemLinkKind::DependsOn,
        );
        Ok(())
    }

    pub fn unlink_items_blocks(&self, blocker_id: ItemId, blocked_id: ItemId) -> Result<()> {
        self.unlink_items_depends_on(blocked_id, blocker_id)
    }

    pub fn unlink_items_related(&self, a: ItemId, b: ItemId) -> Result<()> {
        let (item_id, other_item_id) = Self::normalize_related_pair(a, b);
        self.store
            .delete_item_link(item_id, other_item_id, ItemLinkKind::Related)?;
        self.debug_log_link_event(
            "link.removed",
            item_id,
            other_item_id,
            ItemLinkKind::Related,
        );
        Ok(())
    }

    pub fn immediate_prereq_ids(&self, item_id: ItemId) -> Result<Vec<ItemId>> {
        self.store.list_dependency_ids_for_item(item_id)
    }

    pub fn immediate_dependent_ids(&self, item_id: ItemId) -> Result<Vec<ItemId>> {
        self.store.list_dependent_ids_for_item(item_id)
    }

    pub fn immediate_related_ids(&self, item_id: ItemId) -> Result<Vec<ItemId>> {
        self.store.list_related_ids_for_item(item_id)
    }

    pub fn immediate_links_for_item(&self, item_id: ItemId) -> Result<ItemLinksForItem> {
        Ok(ItemLinksForItem {
            depends_on: self.immediate_prereq_ids(item_id)?,
            blocks: self.immediate_dependent_ids(item_id)?,
            related: self.immediate_related_ids(item_id)?,
        })
    }

    pub fn list_pending_classification_suggestions(&self) -> Result<Vec<ClassificationSuggestion>> {
        self.store.list_pending_suggestions()
    }

    pub fn list_pending_classification_suggestions_for_item(
        &self,
        item_id: ItemId,
    ) -> Result<Vec<ClassificationSuggestion>> {
        self.store.list_pending_suggestions_for_item(item_id)
    }

    pub fn accept_classification_suggestion(
        &self,
        suggestion_id: Uuid,
    ) -> Result<ProcessItemResult> {
        let suggestion = self
            .store
            .get_classification_suggestion(suggestion_id)?
            .ok_or(AgendaError::NotFound {
                entity: "ClassificationSuggestion",
                id: suggestion_id,
            })?;

        let mut result = self.apply_suggestion_assignment(&suggestion)?;
        self.store
            .set_suggestion_status(suggestion_id, SuggestionStatus::Accepted)?;
        merge_process_results(&mut result, self.process_cascades(suggestion.item_id)?);
        self.debug_log_process_result("suggestion.accept", suggestion.item_id, &result);
        Ok(result)
    }

    pub fn reject_classification_suggestion(&self, suggestion_id: Uuid) -> Result<()> {
        self.store
            .set_suggestion_status(suggestion_id, SuggestionStatus::Rejected)
    }

    /// Run classification on demand for a single item, regardless of
    /// `run_on_item_save` / `should_run_continuously` config flags.
    /// Returns the number of new pending suggestions created.
    pub fn classify_item_on_demand(
        &self,
        item_id: ItemId,
        reference_date: jiff::civil::Date,
    ) -> Result<usize> {
        let cfg = self.store.get_classification_config()?;
        let service = self.classification_service(reference_date, &cfg, true);
        if !service.has_providers() {
            return Ok(0);
        }

        let (_item, item_revision_hash, candidates, _debug) =
            service.collect_candidates(item_id)?;
        self.apply_classification_results(item_id, &item_revision_hash, &candidates)
    }

    /// Apply pre-computed classification candidates for an item.
    /// Supersedes old suggestions for this revision, then upserts new ones
    /// based on the current classification config disposition.
    /// Returns the number of new pending suggestions created.
    pub fn apply_classification_results(
        &self,
        item_id: ItemId,
        item_revision_hash: &str,
        candidates: &[ClassificationCandidate],
    ) -> Result<usize> {
        let cfg = self.store.get_classification_config()?;
        self.store
            .supersede_suggestions_for_item_revision(item_id, item_revision_hash)?;

        let mut queued = 0usize;
        for candidate in candidates {
            if matches!(
                candidate.assignment,
                crate::classification::CandidateAssignment::When(_)
            ) {
                let suggestion = ClassificationSuggestion::from_candidate(
                    candidate,
                    item_revision_hash.to_string(),
                    SuggestionStatus::Accepted,
                );
                if self
                    .store
                    .get_classification_suggestion(suggestion.id)?
                    .is_some_and(|existing| existing.status == SuggestionStatus::Rejected)
                {
                    continue;
                }
                self.store.upsert_suggestion(&suggestion)?;
                self.apply_auto_classification_candidate(item_id, candidate)?;
                continue;
            }

            match self.candidate_status_for_config(&cfg, candidate) {
                CandidateDisposition::Skip => {}
                CandidateDisposition::AutoApply => {
                    let suggestion = ClassificationSuggestion::from_candidate(
                        candidate,
                        item_revision_hash.to_string(),
                        SuggestionStatus::Accepted,
                    );
                    if self
                        .store
                        .get_classification_suggestion(suggestion.id)?
                        .is_some_and(|existing| existing.status == SuggestionStatus::Rejected)
                    {
                        continue;
                    }
                    self.store.upsert_suggestion(&suggestion)?;
                    self.apply_auto_classification_candidate(item_id, candidate)?;
                }
                CandidateDisposition::QueueReview => {
                    if self.candidate_assignment_already_present(item_id, candidate)? {
                        continue;
                    }
                    let suggestion = ClassificationSuggestion::from_candidate(
                        candidate,
                        item_revision_hash.to_string(),
                        SuggestionStatus::Pending,
                    );
                    if self
                        .store
                        .get_classification_suggestion(suggestion.id)?
                        .is_some_and(|existing| {
                            existing.status == SuggestionStatus::Rejected
                                || existing.status == SuggestionStatus::Accepted
                        })
                    {
                        continue;
                    }
                    self.store.upsert_suggestion(&suggestion)?;
                    queued += 1;
                }
            }
        }

        Ok(queued)
    }

    /// Prepare a background classification job for an item. Returns `None`
    /// if no expensive (semantic) providers are enabled.
    /// The returned job contains everything needed to run classification
    /// on a background thread without Store access.
    pub fn prepare_background_classification(
        &self,
        item_id: ItemId,
        reference_date: jiff::civil::Date,
    ) -> Result<Option<BackgroundClassificationJob>> {
        let cfg = self.store.get_classification_config()?;
        self.debug_log(&format!(
            "prepare_background_classification: item_id={item_id} semantic_mode={:?} semantic_provider={:?}",
            cfg.semantic_mode, cfg.semantic_provider
        ));
        if cfg.semantic_mode == SemanticClassificationMode::Off {
            self.debug_log(&format!(
                "prepare_background_classification: skip item_id={item_id} reason=semantic_mode_off"
            ));
            return Ok(None);
        }

        let (request, revision_hash) = {
            let service = self.classification_service(reference_date, &cfg, true);
            if !service.has_providers() {
                self.debug_log(&format!(
                    "prepare_background_classification: skip item_id={item_id} reason=no_providers"
                ));
                return Ok(None);
            }
            let (_item, request, revision_hash) = service.prepare_request(item_id)?;
            (request, revision_hash)
        };
        self.debug_log(&format!(
            "prepare_background_classification: queued item_id={item_id} literal_candidates={} semantic_candidates={}",
            request.literal_candidate_categories.len(),
            request.semantic_candidate_categories.len()
        ));
        Ok(Some(BackgroundClassificationJob {
            item_id,
            item_revision_hash: revision_hash,
            request,
            config: cfg,
            ollama_transport: Arc::clone(&self.ollama_transport),
            openrouter_transport: Arc::clone(&self.openrouter_transport),
            openai_transport: Arc::clone(&self.openai_transport),
            reference_date,
            debug: self.debug,
        }))
    }

    fn process_item_save(
        &self,
        item_id: ItemId,
        reference_date: jiff::civil::Date,
        text_changed: bool,
    ) -> Result<ProcessItemResult> {
        self.process_item_save_inner(item_id, reference_date, text_changed, true)
    }

    /// Like `process_item_save` but excludes expensive (semantic/Ollama) providers.
    fn process_item_save_cheap(
        &self,
        item_id: ItemId,
        reference_date: jiff::civil::Date,
        text_changed: bool,
    ) -> Result<ProcessItemResult> {
        self.process_item_save_inner(item_id, reference_date, text_changed, false)
    }

    fn process_item_save_inner(
        &self,
        item_id: ItemId,
        reference_date: jiff::civil::Date,
        text_changed: bool,
        include_semantic: bool,
    ) -> Result<ProcessItemResult> {
        let mut result = ProcessItemResult::default();
        let cfg = self.store.get_classification_config()?;
        if !cfg.should_run_continuously() || !cfg.run_on_item_save {
            let result = self.process_cascades(item_id)?;
            self.debug_log_process_result("item.process", item_id, &result);
            return Ok(result);
        }

        let service = if include_semantic {
            self.classification_service(reference_date, &cfg, text_changed)
        } else {
            self.classification_service_cheap(reference_date, &cfg, text_changed)
        };
        if !service.has_providers() {
            let result = self.process_cascades(item_id)?;
            self.debug_log_process_result("item.process", item_id, &result);
            return Ok(result);
        }

        let (_item, item_revision_hash, candidates, debug_summaries) =
            service.collect_candidates(item_id)?;
        result.semantic_debug_messages.extend(debug_summaries);
        self.store
            .supersede_suggestions_for_item_revision(item_id, &item_revision_hash)?;

        for candidate in candidates {
            let is_semantic = candidate.provider == PROVIDER_ID_OLLAMA_OPENAI_COMPAT
                || candidate.provider == PROVIDER_ID_OPENROUTER
                || candidate.provider == PROVIDER_ID_OPENAI;
            if is_semantic {
                result.semantic_candidates_seen += 1;
            }
            if matches!(
                candidate.assignment,
                crate::classification::CandidateAssignment::When(_)
            ) {
                let suggestion = ClassificationSuggestion::from_candidate(
                    &candidate,
                    item_revision_hash.clone(),
                    SuggestionStatus::Accepted,
                );
                if self
                    .store
                    .get_classification_suggestion(suggestion.id)?
                    .is_some_and(|existing| existing.status == SuggestionStatus::Rejected)
                {
                    continue;
                }
                self.store.upsert_suggestion(&suggestion)?;
                merge_process_results(
                    &mut result,
                    self.apply_auto_classification_candidate(item_id, &candidate)?,
                );
                continue;
            }

            match self.candidate_status_for_config(&cfg, &candidate) {
                CandidateDisposition::Skip => {}
                CandidateDisposition::AutoApply => {
                    let suggestion = ClassificationSuggestion::from_candidate(
                        &candidate,
                        item_revision_hash.clone(),
                        SuggestionStatus::Accepted,
                    );
                    if self
                        .store
                        .get_classification_suggestion(suggestion.id)?
                        .is_some_and(|existing| existing.status == SuggestionStatus::Rejected)
                    {
                        continue;
                    }
                    self.store.upsert_suggestion(&suggestion)?;
                    merge_process_results(
                        &mut result,
                        self.apply_auto_classification_candidate(item_id, &candidate)?,
                    );
                }
                CandidateDisposition::QueueReview => {
                    if self.candidate_assignment_already_present(item_id, &candidate)? {
                        if is_semantic {
                            result.semantic_candidates_skipped_already_assigned += 1;
                        }
                        continue;
                    }
                    let suggestion = ClassificationSuggestion::from_candidate(
                        &candidate,
                        item_revision_hash.clone(),
                        SuggestionStatus::Pending,
                    );
                    if self
                        .store
                        .get_classification_suggestion(suggestion.id)?
                        .is_some_and(|existing| {
                            existing.status == SuggestionStatus::Rejected
                                || existing.status == SuggestionStatus::Accepted
                        })
                    {
                        continue;
                    }
                    self.store.upsert_suggestion(&suggestion)?;
                    if is_semantic {
                        result.semantic_candidates_queued_review += 1;
                    }
                }
            }
        }

        merge_process_results(&mut result, self.process_cascades(item_id)?);
        self.debug_log_process_result("item.process", item_id, &result);
        Ok(result)
    }

    fn candidate_assignment_already_present(
        &self,
        item_id: ItemId,
        candidate: &ClassificationCandidate,
    ) -> Result<bool> {
        match candidate.assignment {
            crate::classification::CandidateAssignment::Category(category_id) => Ok(self
                .store
                .get_assignments_for_item(item_id)?
                .contains_key(&category_id)),
            crate::classification::CandidateAssignment::When(when_date) => Ok(self
                .store
                .get_item(item_id)?
                .when_date
                .is_some_and(|current| current == when_date)),
        }
    }

    fn process_category_change(&self, category_id: CategoryId) -> Result<EvaluateAllItemsResult> {
        let cfg = self.store.get_classification_config()?;
        let enable_implicit_string = cfg.should_run_continuously()
            && cfg.run_on_category_change
            && cfg.literal_mode == LiteralClassificationMode::AutoApply;
        evaluate_all_items_with_options(
            self.store,
            self.classifier,
            category_id,
            ProcessOptions {
                enable_implicit_string,
            },
        )
    }

    fn classification_service<'b>(
        &'b self,
        reference_date: jiff::civil::Date,
        cfg: &'b ClassificationConfig,
        allow_when_parser: bool,
    ) -> ClassificationService<'b> {
        self.classification_service_inner(reference_date, cfg, allow_when_parser, true)
    }

    fn classification_service_cheap<'b>(
        &'b self,
        reference_date: jiff::civil::Date,
        cfg: &'b ClassificationConfig,
        allow_when_parser: bool,
    ) -> ClassificationService<'b> {
        self.classification_service_inner(reference_date, cfg, allow_when_parser, false)
    }

    fn classification_service_inner<'b>(
        &'b self,
        reference_date: jiff::civil::Date,
        cfg: &'b ClassificationConfig,
        allow_when_parser: bool,
        include_semantic: bool,
    ) -> ClassificationService<'b> {
        let mut providers = Vec::new();
        if cfg.literal_mode != LiteralClassificationMode::Off
            && cfg.provider_enabled(PROVIDER_ID_IMPLICIT_STRING)
        {
            providers.push(Box::new(ImplicitStringProvider {
                classifier: self.classifier,
            }) as _);
        }
        if allow_when_parser
            && cfg.literal_mode != LiteralClassificationMode::Off
            && cfg.provider_enabled(PROVIDER_ID_WHEN_PARSER)
        {
            providers.push(Box::new(WhenParserProvider {
                parser: self.date_parser,
                reference_date,
            }) as _);
        }
        if include_semantic && cfg.semantic_mode == SemanticClassificationMode::SuggestReview {
            match cfg.semantic_provider {
                SemanticProviderKind::Ollama => {
                    providers.push(Box::new(OllamaProvider {
                        settings: &cfg.ollama,
                        transport: self.ollama_transport.as_ref(),
                        debug: self.debug,
                    }) as _);
                }
                SemanticProviderKind::OpenRouter => {
                    providers.push(Box::new(OpenRouterProvider {
                        settings: &cfg.openrouter,
                        transport: self.openrouter_transport.as_ref(),
                        debug: self.debug,
                    }) as _);
                }
                SemanticProviderKind::OpenAi => {
                    providers.push(Box::new(OpenAiProvider {
                        settings: &cfg.openai,
                        transport: self.openai_transport.as_ref(),
                        debug: self.debug,
                    }) as _);
                }
            }
        }
        ClassificationService::new(self.store, providers)
    }

    fn debug_log(&self, message: &str) {
        if !self.debug {
            return;
        }
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(CLASSIFICATION_DEBUG_LOG_PATH)
        {
            let _ = writeln!(file, "[{}] {message}", jiff::Zoned::now());
        }
    }

    fn reprocess_existing_item(&self, item_id: ItemId) -> Result<ProcessItemResult> {
        let cfg = self.store.get_classification_config()?;
        self.reprocess_with_implicit(item_id, self.should_reprocess_with_implicit(&cfg))
    }

    fn process_cascades(&self, item_id: ItemId) -> Result<ProcessItemResult> {
        self.reprocess_with_implicit(item_id, false)
    }

    fn copy_preview_categories(
        preview_store: &Store,
        source_categories: &[Category],
    ) -> Result<HashMap<CategoryId, CategoryId>> {
        let preview_reserved: HashMap<String, CategoryId> = preview_store
            .get_hierarchy()?
            .into_iter()
            .filter(|category| Self::is_reserved_category_name(&category.name))
            .map(|category| (category.name.to_ascii_lowercase(), category.id))
            .collect();

        let mut id_map = HashMap::new();
        for category in source_categories {
            let preview_id = if Self::is_reserved_category_name(&category.name) {
                *preview_reserved
                    .get(&category.name.to_ascii_lowercase())
                    .ok_or_else(|| AgendaError::InvalidOperation {
                        message: format!("missing preview reserved category '{}'", category.name),
                    })?
            } else {
                category.id
            };
            id_map.insert(category.id, preview_id);
        }

        for category in source_categories {
            let preview_category = Self::remap_category_for_preview(category, &id_map)?;
            if Self::is_reserved_category_name(&category.name) {
                preview_store.update_category(&preview_category)?;
            } else {
                preview_store.create_category(&preview_category)?;
            }
        }

        Ok(id_map)
    }

    fn remap_category_for_preview(
        category: &Category,
        id_map: &HashMap<CategoryId, CategoryId>,
    ) -> Result<Category> {
        let mut preview_category = category.clone();
        preview_category.id = Self::mapped_category_id(category.id, id_map, "preview category id")?;
        preview_category.parent = category
            .parent
            .map(|parent_id| Self::mapped_category_id(parent_id, id_map, "preview parent id"))
            .transpose()?;
        preview_category.children.clear();
        preview_category.conditions = category
            .conditions
            .iter()
            .map(|condition| Self::remap_condition_for_preview(condition, id_map))
            .collect::<Result<Vec<_>>>()?;
        preview_category.actions = category
            .actions
            .iter()
            .map(|action| Self::remap_action_for_preview(action, id_map))
            .collect::<Result<Vec<_>>>()?;
        Ok(preview_category)
    }

    fn remap_condition_for_preview(
        condition: &Condition,
        id_map: &HashMap<CategoryId, CategoryId>,
    ) -> Result<Condition> {
        match condition {
            Condition::ImplicitString => Ok(Condition::ImplicitString),
            Condition::Profile { criteria } => Ok(Condition::Profile {
                criteria: Box::new(Self::remap_query_for_preview(criteria, id_map)?),
            }),
        }
    }

    fn remap_action_for_preview(
        action: &Action,
        id_map: &HashMap<CategoryId, CategoryId>,
    ) -> Result<Action> {
        let remap_targets = |targets: &HashSet<CategoryId>| -> Result<HashSet<CategoryId>> {
            targets
                .iter()
                .map(|target_id| Self::mapped_category_id(*target_id, id_map, "preview action"))
                .collect()
        };

        match action {
            Action::Assign { targets } => Ok(Action::Assign {
                targets: remap_targets(targets)?,
            }),
            Action::Remove { targets } => Ok(Action::Remove {
                targets: remap_targets(targets)?,
            }),
        }
    }

    fn remap_query_for_preview(
        query: &crate::model::Query,
        id_map: &HashMap<CategoryId, CategoryId>,
    ) -> Result<crate::model::Query> {
        Ok(crate::model::Query {
            criteria: query
                .criteria
                .iter()
                .map(|criterion| {
                    Ok(crate::model::Criterion {
                        mode: criterion.mode,
                        category_id: Self::mapped_category_id(
                            criterion.category_id,
                            id_map,
                            "preview query criterion",
                        )?,
                    })
                })
                .collect::<Result<Vec<_>>>()?,
            virtual_include: query.virtual_include.clone(),
            virtual_exclude: query.virtual_exclude.clone(),
            text_search: query.text_search.clone(),
        })
    }

    fn mapped_category_id(
        source_id: CategoryId,
        id_map: &HashMap<CategoryId, CategoryId>,
        context: &str,
    ) -> Result<CategoryId> {
        id_map
            .get(&source_id)
            .copied()
            .ok_or_else(|| AgendaError::InvalidOperation {
                message: format!("missing category mapping for {context}: {source_id}"),
            })
    }

    fn source_category_id(
        preview_id: CategoryId,
        id_map: &HashMap<CategoryId, CategoryId>,
        context: &str,
    ) -> Result<CategoryId> {
        id_map
            .iter()
            .find_map(|(source_id, mapped_id)| (*mapped_id == preview_id).then_some(*source_id))
            .ok_or_else(|| AgendaError::InvalidOperation {
                message: format!("missing reverse category mapping for {context}: {preview_id}"),
            })
    }

    fn is_reserved_category_name(name: &str) -> bool {
        [
            RESERVED_CATEGORY_NAME_WHEN,
            RESERVED_CATEGORY_NAME_ENTRY,
            RESERVED_CATEGORY_NAME_DONE,
        ]
        .iter()
        .any(|reserved_name| reserved_name.eq_ignore_ascii_case(name))
    }

    fn reprocess_with_implicit(
        &self,
        item_id: ItemId,
        enable_implicit_string: bool,
    ) -> Result<ProcessItemResult> {
        process_item_with_options(
            self.store,
            self.classifier,
            item_id,
            ProcessOptions {
                enable_implicit_string,
            },
        )
    }

    fn should_reprocess_with_implicit(&self, cfg: &ClassificationConfig) -> bool {
        cfg.should_run_continuously() && cfg.literal_mode == LiteralClassificationMode::AutoApply
    }

    fn candidate_status_for_config(
        &self,
        cfg: &ClassificationConfig,
        candidate: &ClassificationCandidate,
    ) -> CandidateDisposition {
        match candidate.provider.as_str() {
            PROVIDER_ID_IMPLICIT_STRING => match cfg.literal_mode {
                LiteralClassificationMode::Off => CandidateDisposition::Skip,
                LiteralClassificationMode::AutoApply => CandidateDisposition::AutoApply,
                LiteralClassificationMode::SuggestReview => CandidateDisposition::QueueReview,
            },
            PROVIDER_ID_OLLAMA_OPENAI_COMPAT | PROVIDER_ID_OPENROUTER | PROVIDER_ID_OPENAI => match cfg.semantic_mode {
                SemanticClassificationMode::Off => CandidateDisposition::Skip,
                SemanticClassificationMode::SuggestReview => CandidateDisposition::QueueReview,
            },
            _ => CandidateDisposition::Skip,
        }
    }

    fn apply_auto_classification_candidate(
        &self,
        item_id: ItemId,
        candidate: &ClassificationCandidate,
    ) -> Result<ProcessItemResult> {
        match &candidate.assignment {
            crate::classification::CandidateAssignment::Category(category_id) => {
                let category = self.store.get_category(*category_id)?;
                let mut result = ProcessItemResult::default();
                if self.apply_category_assignment(
                    item_id,
                    *category_id,
                    AssignmentSource::AutoClassified,
                    Some(format!("cat:{}", category.name)),
                    Some(AssignmentExplanation::AutoClassified {
                        provider_id: candidate.provider.clone(),
                        model: candidate.model.clone(),
                        rationale: candidate.rationale.clone(),
                    }),
                    false,
                )? {
                    result.new_assignments.insert(*category_id);
                    result.assignment_events.push(AssignmentEvent {
                        kind: AssignmentEventKind::Assigned,
                        category_id: *category_id,
                        category_name: category.name.clone(),
                        summary: AssignmentExplanation::AutoClassified {
                            provider_id: candidate.provider.clone(),
                            model: candidate.model.clone(),
                            rationale: candidate.rationale.clone(),
                        }
                        .summary(),
                    });
                    merge_process_results(
                        &mut result,
                        self.apply_actions_for_category(item_id, *category_id)?,
                    );
                }
                Ok(result)
            }
            crate::classification::CandidateAssignment::When(when_date) => {
                let origin = if candidate.provider == PROVIDER_ID_WHEN_PARSER {
                    Some(origin_const::NLP_DATE.to_string())
                } else {
                    Some(format!("classification:auto:{}", candidate.provider))
                };
                let mut result = ProcessItemResult::default();
                if self.apply_when_assignment(
                    item_id,
                    *when_date,
                    AssignmentSource::AutoClassified,
                    origin,
                    Some(AssignmentExplanation::AutoClassified {
                        provider_id: candidate.provider.clone(),
                        model: candidate.model.clone(),
                        rationale: candidate.rationale.clone(),
                    }),
                )? {
                    let when_category_id = self.category_id_by_name(RESERVED_CATEGORY_NAME_WHEN)?;
                    result.new_assignments.insert(when_category_id);
                    result.assignment_events.push(AssignmentEvent {
                        kind: AssignmentEventKind::Assigned,
                        category_id: when_category_id,
                        category_name: RESERVED_CATEGORY_NAME_WHEN.to_string(),
                        summary: AssignmentExplanation::AutoClassified {
                            provider_id: candidate.provider.clone(),
                            model: candidate.model.clone(),
                            rationale: candidate.rationale.clone(),
                        }
                        .summary(),
                    });
                }
                Ok(result)
            }
        }
    }

    fn apply_suggestion_assignment(
        &self,
        suggestion: &ClassificationSuggestion,
    ) -> Result<ProcessItemResult> {
        let origin = Some(format!("suggestion:accepted:{}", suggestion.provider_id));
        match suggestion.assignment {
            crate::classification::CandidateAssignment::Category(category_id) => {
                let mut result = ProcessItemResult::default();
                if self.apply_category_assignment(
                    suggestion.item_id,
                    category_id,
                    AssignmentSource::SuggestionAccepted,
                    origin,
                    Some(AssignmentExplanation::SuggestionAccepted {
                        provider_id: suggestion.provider_id.clone(),
                        model: suggestion.model.clone(),
                        rationale: suggestion.rationale.clone(),
                    }),
                    true,
                )? {
                    result.new_assignments.insert(category_id);
                    result.assignment_events.push(AssignmentEvent {
                        kind: AssignmentEventKind::Assigned,
                        category_id,
                        category_name: self.category_name_or_id(category_id),
                        summary: AssignmentExplanation::SuggestionAccepted {
                            provider_id: suggestion.provider_id.clone(),
                            model: suggestion.model.clone(),
                            rationale: suggestion.rationale.clone(),
                        }
                        .summary(),
                    });
                    merge_process_results(
                        &mut result,
                        self.apply_actions_for_category(suggestion.item_id, category_id)?,
                    );
                }
                Ok(result)
            }
            crate::classification::CandidateAssignment::When(when_date) => {
                let mut result = ProcessItemResult::default();
                if self.apply_when_assignment(
                    suggestion.item_id,
                    when_date,
                    AssignmentSource::SuggestionAccepted,
                    origin,
                    Some(AssignmentExplanation::SuggestionAccepted {
                        provider_id: suggestion.provider_id.clone(),
                        model: suggestion.model.clone(),
                        rationale: suggestion.rationale.clone(),
                    }),
                )? {
                    let when_category_id = self.category_id_by_name(RESERVED_CATEGORY_NAME_WHEN)?;
                    result.new_assignments.insert(when_category_id);
                    result.assignment_events.push(AssignmentEvent {
                        kind: AssignmentEventKind::Assigned,
                        category_id: when_category_id,
                        category_name: RESERVED_CATEGORY_NAME_WHEN.to_string(),
                        summary: AssignmentExplanation::SuggestionAccepted {
                            provider_id: suggestion.provider_id.clone(),
                            model: suggestion.model.clone(),
                            rationale: suggestion.rationale.clone(),
                        }
                        .summary(),
                    });
                }
                Ok(result)
            }
        }
    }

    fn apply_category_assignment(
        &self,
        item_id: ItemId,
        category_id: CategoryId,
        source: AssignmentSource,
        origin: Option<String>,
        explanation: Option<AssignmentExplanation>,
        allow_manual_exclusive_override: bool,
    ) -> Result<bool> {
        let assignments = self.store.get_assignments_for_item(item_id)?;
        if assignments.contains_key(&category_id) {
            return Ok(false);
        }
        if !allow_manual_exclusive_override
            && self.has_manual_exclusive_sibling(item_id, category_id)?
        {
            return Ok(false);
        }

        self.enforce_manual_exclusive_siblings(item_id, category_id)?;
        let assignment = Assignment {
            source,
            assigned_at: Timestamp::now(),
            sticky: true,
            origin,
            explanation,
            numeric_value: None,
        };
        self.store.assign_item(item_id, category_id, &assignment)?;
        self.assign_subsumption_for_category(item_id, category_id)?;
        Ok(true)
    }

    fn apply_when_assignment(
        &self,
        item_id: ItemId,
        when_date: jiff::civil::DateTime,
        source: AssignmentSource,
        origin: Option<String>,
        explanation: Option<AssignmentExplanation>,
    ) -> Result<bool> {
        let mut item = self.store.get_item(item_id)?;
        if item.when_date == Some(when_date) {
            return Ok(false);
        }

        item.when_date = Some(when_date);
        item.modified_at = Timestamp::now();
        self.store.update_item(&item)?;
        let assignment = Assignment {
            source,
            assigned_at: Timestamp::now(),
            sticky: true,
            origin,
            explanation,
            numeric_value: None,
        };
        self.sync_when_assignment(item_id, Some(when_date), Some(assignment))?;
        Ok(true)
    }

    fn apply_actions_for_category(
        &self,
        item_id: ItemId,
        category_id: CategoryId,
    ) -> Result<ProcessItemResult> {
        let category = self.store.get_category(category_id)?;
        let mut result = ProcessItemResult::default();

        for action in &category.actions {
            match action {
                Action::Assign { targets } => {
                    for target_id in targets {
                        if self.apply_category_assignment(
                            item_id,
                            *target_id,
                            AssignmentSource::Action,
                            Some(format!("action:{}", category.name)),
                            Some(AssignmentExplanation::Action {
                                trigger_category_name: category.name.clone(),
                                kind: AssignmentActionKind::Assign,
                            }),
                            true,
                        )? {
                            result.new_assignments.insert(*target_id);
                            result.assignment_events.push(AssignmentEvent {
                                kind: AssignmentEventKind::Assigned,
                                category_id: *target_id,
                                category_name: self.category_name_or_id(*target_id),
                                summary: format!("Assigned by action on {}", category.name),
                            });
                        }
                    }
                }
                Action::Remove { targets } => {
                    for target_id in targets {
                        let existing_assignments = self.store.get_assignments_for_item(item_id)?;
                        if let Some(existing) = existing_assignments.get(target_id) {
                            self.store.unassign_item(item_id, *target_id)?;
                            result
                                .deferred_removals
                                .push(crate::engine::DeferredRemoval {
                                    target: *target_id,
                                    triggered_by: category_id,
                                });
                            result.removed_assignments.insert(*target_id);
                            result.assignment_events.push(AssignmentEvent {
                                kind: AssignmentEventKind::Removed,
                                category_id: *target_id,
                                category_name: self.store.get_category(*target_id)?.name,
                                summary: existing
                                    .explanation
                                    .as_ref()
                                    .map(|explanation| explanation.removal_summary())
                                    .unwrap_or_else(|| {
                                        format!("Removed by action on {}", category.name)
                                    }),
                            });
                        }
                    }
                }
            }
        }

        Ok(result)
    }

    fn has_manual_exclusive_sibling(
        &self,
        item_id: ItemId,
        category_id: CategoryId,
    ) -> Result<bool> {
        let category = self.store.get_category(category_id)?;
        let Some(parent_id) = category.parent else {
            return Ok(false);
        };
        let parent = self.store.get_category(parent_id)?;
        if !parent.is_exclusive {
            return Ok(false);
        }
        let assignments = self.store.get_assignments_for_item(item_id)?;
        Ok(parent.children.into_iter().any(|sibling_id| {
            sibling_id != category_id
                && assignments.get(&sibling_id).is_some_and(|assignment| {
                    assignment.source == AssignmentSource::Manual
                        || assignment.source == AssignmentSource::SuggestionAccepted
                })
        }))
    }

    fn assign_manual_categories(
        &self,
        item_id: ItemId,
        targets: &HashSet<CategoryId>,
        origin: &str,
    ) -> Result<()> {
        if targets.is_empty() {
            return Ok(());
        }

        let mut existing = self.store.get_assignments_for_item(item_id)?;
        let assignment = Assignment {
            source: AssignmentSource::Manual,
            assigned_at: Timestamp::now(),
            sticky: true,
            origin: Some(origin.to_string()),
            explanation: Some(AssignmentExplanation::Manual {
                origin: Some(origin.to_string()),
            }),
            numeric_value: None,
        };

        for category_id in targets {
            if existing.contains_key(category_id) {
                continue;
            }
            self.enforce_manual_exclusive_siblings(item_id, *category_id)?;
            self.store.assign_item(item_id, *category_id, &assignment)?;
            self.assign_subsumption_for_category(item_id, *category_id)?;
            existing = self.store.get_assignments_for_item(item_id)?;
        }

        Ok(())
    }

    fn enforce_manual_exclusive_siblings(
        &self,
        item_id: ItemId,
        category_id: CategoryId,
    ) -> Result<()> {
        let category = self.store.get_category(category_id)?;
        let Some(parent_id) = category.parent else {
            return Ok(());
        };

        let parent = self.store.get_category(parent_id)?;
        if !parent.is_exclusive {
            return Ok(());
        }

        let assignments = self.store.get_assignments_for_item(item_id)?;
        for sibling_id in parent.children {
            if sibling_id == category_id {
                continue;
            }
            if assignments.contains_key(&sibling_id) {
                self.store.unassign_item(item_id, sibling_id)?;
            }
        }

        Ok(())
    }

    fn assign_subsumption_for_category(
        &self,
        item_id: ItemId,
        category_id: CategoryId,
    ) -> Result<()> {
        let categories = self.store.get_hierarchy()?;
        let categories_by_id: HashMap<CategoryId, &Category> = categories
            .iter()
            .map(|category| (category.id, category))
            .collect();
        let mut existing = self.store.get_assignments_for_item(item_id)?;

        let mut cursor = categories_by_id
            .get(&category_id)
            .and_then(|category| category.parent);
        while let Some(parent_id) = cursor {
            if let std::collections::hash_map::Entry::Vacant(entry) = existing.entry(parent_id) {
                let parent_name = categories_by_id
                    .get(&parent_id)
                    .map(|category| category.name.clone())
                    .unwrap_or_else(|| parent_id.to_string());
                let assignment = Assignment {
                    source: AssignmentSource::Subsumption,
                    assigned_at: Timestamp::now(),
                    sticky: false,
                    origin: Some(format!("{}:{parent_name}", origin_const::SUBSUMPTION)),
                    explanation: Some(AssignmentExplanation::Subsumption {
                        parent_category_name: parent_name.clone(),
                        via_child_category_name: categories_by_id
                            .get(&category_id)
                            .map(|category| category.name.clone())
                            .unwrap_or_else(|| category_id.to_string()),
                    }),
                    numeric_value: None,
                };
                self.store.assign_item(item_id, parent_id, &assignment)?;
                entry.insert(assignment);
            }

            cursor = categories_by_id
                .get(&parent_id)
                .and_then(|category| category.parent);
        }

        Ok(())
    }

    fn unassign_categories(&self, item_id: ItemId, targets: &HashSet<CategoryId>) -> Result<()> {
        for category_id in targets {
            self.store.unassign_item(item_id, *category_id)?;
        }
        Ok(())
    }

    pub fn section_insert_targets(view: &View, section: &Section) -> HashSet<CategoryId> {
        let mut targets = section.on_insert_assign.clone();
        targets.extend(section.criteria.and_category_ids());
        targets.extend(view.criteria.and_category_ids());
        targets
    }

    pub fn section_remove_targets(view: &View, section: &Section) -> HashSet<CategoryId> {
        let mut targets = Self::section_structural_targets(section);
        let preserve: HashSet<CategoryId> = view.criteria.and_category_ids().collect();
        targets.retain(|category_id| !preserve.contains(category_id));
        targets.extend(section.on_remove_unassign.iter().copied());
        targets
    }

    fn section_structural_targets(section: &Section) -> HashSet<CategoryId> {
        if section.criteria.or_category_ids().next().is_some() {
            return HashSet::new();
        }

        let mut targets = section.on_insert_assign.clone();
        targets.extend(section.criteria.and_category_ids());
        targets
    }

    fn prepend_assignment_event(
        &self,
        result: &mut ProcessItemResult,
        kind: AssignmentEventKind,
        category_id: CategoryId,
        summary: &str,
    ) {
        result.assignment_events.insert(
            0,
            AssignmentEvent {
                kind,
                category_id,
                category_name: self.category_name_or_id(category_id),
                summary: summary.to_string(),
            },
        );
    }

    fn category_name_or_id(&self, category_id: CategoryId) -> String {
        self.store
            .get_category(category_id)
            .map(|category| category.name)
            .unwrap_or_else(|_| category_id.to_string())
    }

    fn debug_log_process_result(&self, context: &str, item_id: ItemId, result: &ProcessItemResult) {
        if !self.debug {
            return;
        }
        for event in &result.assignment_events {
            eprintln!(
                "[agenda.debug] context={context} item={item_id} event={:?} category={} summary={}",
                event.kind, event.category_name, event.summary
            );
        }
        for message in &result.semantic_debug_messages {
            eprintln!("[agenda.debug] context={context} item={item_id} semantic={message}");
        }
    }

    fn debug_log_link_event(
        &self,
        family: &str,
        item_id: ItemId,
        other_item_id: ItemId,
        kind: ItemLinkKind,
    ) {
        if !self.debug {
            return;
        }
        eprintln!(
            "[agenda.debug] event={family} kind={kind:?} item={item_id} other={other_item_id}"
        );
    }

    fn first_assigned_descendant(
        &self,
        item_id: ItemId,
        category_id: CategoryId,
    ) -> Result<Option<CategoryId>> {
        let assignments = self.store.get_assignments_for_item(item_id)?;
        if !assignments.contains_key(&category_id) {
            return Ok(None);
        }

        let hierarchy = self.store.get_hierarchy()?;
        let categories_by_id: HashMap<CategoryId, &Category> = hierarchy
            .iter()
            .map(|category| (category.id, category))
            .collect();
        let mut stack: Vec<CategoryId> = categories_by_id
            .get(&category_id)
            .map(|category| category.children.clone())
            .unwrap_or_default();
        let mut visited = HashSet::new();

        while let Some(current_id) = stack.pop() {
            if !visited.insert(current_id) {
                continue;
            }
            if assignments.contains_key(&current_id) {
                return Ok(Some(current_id));
            }
            if let Some(category) = categories_by_id.get(&current_id) {
                stack.extend(category.children.iter().copied());
            }
        }

        Ok(None)
    }

    fn normalize_related_pair(a: ItemId, b: ItemId) -> (ItemId, ItemId) {
        if a.to_string() < b.to_string() {
            (a, b)
        } else {
            (b, a)
        }
    }

    fn build_link(&self, item_id: ItemId, other_item_id: ItemId, kind: ItemLinkKind) -> ItemLink {
        ItemLink {
            item_id,
            other_item_id,
            kind,
            created_at: Timestamp::now(),
            origin: Some(origin_const::MANUAL_LINK.to_string()),
        }
    }

    fn ensure_item_exists(&self, item_id: ItemId) -> Result<()> {
        let _ = self.store.get_item(item_id)?;
        Ok(())
    }

    fn ensure_not_self_link(&self, a: ItemId, b: ItemId, relation: &str) -> Result<()> {
        if a == b {
            return Err(AgendaError::InvalidOperation {
                message: format!("cannot create self-link for {relation}"),
            });
        }
        Ok(())
    }

    fn ensure_depends_on_no_cycle(
        &self,
        dependent_id: ItemId,
        dependency_id: ItemId,
    ) -> Result<()> {
        let mut stack = vec![dependency_id];
        let mut visited = HashSet::new();

        while let Some(current) = stack.pop() {
            if !visited.insert(current) {
                continue;
            }
            if current == dependent_id {
                return Err(AgendaError::InvalidOperation {
                    message: format!(
                        "cannot create depends-on link {dependent_id} -> {dependency_id}: cycle detected"
                    ),
                });
            }
            stack.extend(self.store.list_dependency_ids_for_item(current)?);
        }

        Ok(())
    }

    fn sync_when_assignment(
        &self,
        item_id: ItemId,
        when_date: Option<jiff::civil::DateTime>,
        assignment_override: Option<Assignment>,
    ) -> Result<()> {
        let when_category_id = self.category_id_by_name(RESERVED_CATEGORY_NAME_WHEN)?;
        let assignments = self.store.get_assignments_for_item(item_id)?;

        match when_date {
            Some(_) => {
                if let Some(assignment) = assignment_override {
                    self.store
                        .assign_item(item_id, when_category_id, &assignment)?;
                } else if !assignments.contains_key(&when_category_id) {
                    self.store.assign_item(
                        item_id,
                        when_category_id,
                        &Self::default_when_assignment(),
                    )?;
                }
            }
            None => {
                if assignments.contains_key(&when_category_id) {
                    self.store.unassign_item(item_id, when_category_id)?;
                }
            }
        }

        Ok(())
    }

    fn default_when_assignment() -> Assignment {
        Assignment {
            source: AssignmentSource::AutoClassified,
            assigned_at: Timestamp::now(),
            sticky: true,
            origin: Some(origin_const::NLP_DATE.to_string()),
            explanation: Some(AssignmentExplanation::AutoClassified {
                provider_id: PROVIDER_ID_WHEN_PARSER.to_string(),
                model: None,
                rationale: None,
            }),
            numeric_value: None,
        }
    }

    fn done_category_id(&self) -> Result<CategoryId> {
        self.category_id_by_name(RESERVED_CATEGORY_NAME_DONE)
    }

    fn clear_claim_assignment_if_configured(&self, item_id: ItemId) -> Result<()> {
        if let Some(ResolvedWorkflowConfig {
            claim_category_id, ..
        }) = resolve_workflow_config(self.store)?
        {
            self.store.unassign_item(item_id, claim_category_id)?;
        }
        Ok(())
    }

    fn item_is_actionable(&self, item_id: ItemId) -> Result<bool> {
        let categories_by_id: HashMap<CategoryId, Category> = self
            .store
            .get_hierarchy()?
            .into_iter()
            .map(|category| (category.id, category))
            .collect();
        let assignments = self.store.get_assignments_for_item(item_id)?;
        Ok(assignments.keys().any(|category_id| {
            categories_by_id
                .get(category_id)
                .map(|c| c.is_actionable)
                .unwrap_or(false)
        }))
    }

    fn category_id_by_name(&self, category_name: &str) -> Result<CategoryId> {
        self.store
            .get_hierarchy()?
            .into_iter()
            .find(|category| category.name.eq_ignore_ascii_case(category_name))
            .map(|category| category.id)
            .ok_or_else(|| AgendaError::StorageError {
                source: Box::new(std::io::Error::other(format!(
                    "missing category: {category_name}"
                ))),
            })
    }
}

fn merge_process_results(target: &mut ProcessItemResult, incoming: ProcessItemResult) {
    target.new_assignments.extend(incoming.new_assignments);
    target
        .removed_assignments
        .extend(incoming.removed_assignments);
    target.assignment_events.extend(incoming.assignment_events);
    target.deferred_removals.extend(incoming.deferred_removals);
    target.semantic_candidates_seen += incoming.semantic_candidates_seen;
    target.semantic_candidates_queued_review += incoming.semantic_candidates_queued_review;
    target.semantic_candidates_skipped_already_assigned +=
        incoming.semantic_candidates_skipped_already_assigned;
    target
        .semantic_debug_messages
        .extend(incoming.semantic_debug_messages);
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::sync::{Arc, Barrier};
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    use jiff::civil::{Date, DateTime};
    use jiff::Timestamp;
    use rust_decimal::Decimal;

    use super::Agenda;
    use crate::classification::{
        ClassificationConfig, LiteralClassificationMode, OllamaProviderSettings, OllamaTransport,
        SemanticClassificationMode, SuggestionStatus, PROVIDER_ID_IMPLICIT_STRING,
        PROVIDER_ID_OLLAMA_OPENAI_COMPAT,
    };
    use crate::error::AgendaError;
    use crate::matcher::SubstringClassifier;
    use crate::model::{
        Action, Assignment, AssignmentSource, Category, CategoryId, CategoryValueKind, Condition,
        CriterionMode, Item, ItemId, ItemLinkKind, Query, Section, View, WhenBucket,
        RESERVED_CATEGORY_NAME_DONE, RESERVED_CATEGORY_NAME_WHEN,
    };
    use crate::query::{resolve_view, resolve_when_bucket};
    use crate::store::Store;

    fn category(name: &str, implicit: bool) -> Category {
        let mut category = Category::new(name.to_string());
        category.enable_implicit_string = implicit;
        category
    }

    fn child_category(name: &str, parent: CategoryId, implicit: bool) -> Category {
        let mut category = category(name, implicit);
        category.parent = Some(parent);
        category
    }

    fn section(title: &str) -> Section {
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

    fn view(name: &str) -> View {
        View::new(name.to_string())
    }

    fn manual_assignment(origin: &str) -> Assignment {
        Assignment {
            source: AssignmentSource::Manual,
            assigned_at: Timestamp::now(),
            sticky: true,
            origin: Some(origin.to_string()),
            explanation: None,
            numeric_value: None,
        }
    }

    fn date(y: i16, m: i8, d: i8) -> Date {
        Date::new(y, m, d).expect("valid date")
    }

    fn datetime(y: i16, m: i8, d: i8, h: i8, min: i8) -> DateTime {
        date(y, m, d).at(h, min, 0, 0)
    }

    fn when_category_id(store: &Store) -> CategoryId {
        store
            .get_hierarchy()
            .expect("hierarchy available")
            .into_iter()
            .find(|category| {
                category
                    .name
                    .eq_ignore_ascii_case(RESERVED_CATEGORY_NAME_WHEN)
            })
            .expect("reserved When category exists")
            .id
    }

    fn category_id_by_name(store: &Store, name: &str) -> Option<CategoryId> {
        store
            .get_hierarchy()
            .expect("hierarchy available")
            .into_iter()
            .find(|category| category.name.eq_ignore_ascii_case(name))
            .map(|category| category.id)
    }

    fn make_item(store: &Store, text: &str) -> ItemId {
        let item = Item::new(text.to_string());
        let id = item.id;
        store.create_item(&item).unwrap();
        id
    }

    #[derive(Default)]
    struct FakeOllamaTransport {
        response: Option<String>,
    }

    impl OllamaTransport for FakeOllamaTransport {
        fn complete(
            &self,
            _settings: &OllamaProviderSettings,
            _system_prompt: &str,
            _user_prompt: &str,
        ) -> crate::error::Result<Option<String>> {
            Ok(self.response.clone())
        }
    }

    #[test]
    fn create_item_triggers_classification() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let sarah = category("Sarah", true);
        store.create_category(&sarah).unwrap();

        let item = Item::new("Sarah's meeting".to_string());
        let result = agenda.create_item(&item).unwrap();
        assert!(result.new_assignments.contains(&sarah.id));

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&sarah.id));
    }

    #[test]
    fn create_item_triggers_classification_from_also_match_term() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut phone_calls = category("Phone Calls", true);
        phone_calls.match_category_name = false;
        phone_calls.also_match = vec!["dial".to_string(), "ring".to_string()];
        store.create_category(&phone_calls).unwrap();

        let item = Item::new("Dial Sarah tomorrow".to_string());
        let result = agenda.create_item(&item).unwrap();
        assert!(result.new_assignments.contains(&phone_calls.id));

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&phone_calls.id));
    }

    #[test]
    fn create_item_does_not_match_literal_category_name_when_disabled() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut person = category("Person", true);
        person.match_category_name = false;
        person.also_match = vec!["bob".to_string(), "sally".to_string()];
        store.create_category(&person).unwrap();

        let person_item = Item::new("Person".to_string());
        let person_result = agenda.create_item(&person_item).unwrap();
        assert!(!person_result.new_assignments.contains(&person.id));

        let bob_item = Item::new("Call Bob tomorrow".to_string());
        let bob_result = agenda.create_item(&bob_item).unwrap();
        assert!(bob_result.new_assignments.contains(&person.id));
    }

    #[test]
    fn create_item_triggers_classification_from_suffix_normalized_match() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let calls = category("Call", true);
        store.create_category(&calls).unwrap();

        let item = Item::new("Calling vendors".to_string());
        let result = agenda.create_item(&item).unwrap();
        assert!(result.new_assignments.contains(&calls.id));

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&calls.id));
    }

    #[test]
    fn create_item_in_suggest_review_mode_queues_pending_suggestions_without_assigning() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let cfg = ClassificationConfig {
            literal_mode: LiteralClassificationMode::SuggestReview,
            semantic_mode: SemanticClassificationMode::Off,
            ..ClassificationConfig::default()
        };
        store
            .set_classification_config(&cfg)
            .expect("persist config");

        let travel = category("Travel", true);
        store.create_category(&travel).unwrap();

        let item = Item::new("Book travel next Tuesday".to_string());
        let result = agenda
            .create_item_with_reference_date(&item, date(2026, 3, 20))
            .unwrap();
        assert_eq!(result.new_assignments.len(), 1);
        let when_id = store
            .get_hierarchy()
            .expect("load hierarchy")
            .into_iter()
            .find(|category| category.name == RESERVED_CATEGORY_NAME_WHEN)
            .expect("reserved When category present")
            .id;
        assert!(result.new_assignments.contains(&when_id));

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(!assignments.contains_key(&travel.id));
        assert!(assignments.contains_key(&when_id));

        let pending = agenda
            .list_pending_classification_suggestions_for_item(item.id)
            .expect("list pending suggestions");
        assert_eq!(pending.len(), 1);
        assert!(matches!(
            pending[0].assignment,
            crate::classification::CandidateAssignment::Category(category_id)
                if category_id == travel.id
        ));
    }

    #[test]
    fn create_item_with_classification_disabled_skips_implicit_matching() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let cfg = ClassificationConfig {
            enabled: false,
            literal_mode: LiteralClassificationMode::Off,
            semantic_mode: SemanticClassificationMode::Off,
            ..ClassificationConfig::default()
        };
        store
            .set_classification_config(&cfg)
            .expect("persist config");

        let travel = category("Travel", true);
        store.create_category(&travel).unwrap();

        let item = Item::new("Travel to Seattle".to_string());
        agenda.create_item(&item).unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(!assignments.contains_key(&travel.id));
    }

    #[test]
    fn literal_auto_apply_and_semantic_off_keeps_current_deterministic_behavior() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let cfg = ClassificationConfig {
            literal_mode: LiteralClassificationMode::AutoApply,
            semantic_mode: SemanticClassificationMode::Off,
            ..ClassificationConfig::default()
        };
        store.set_classification_config(&cfg).unwrap();

        let work = category("Work", true);
        store.create_category(&work).unwrap();

        let item = Item::new("Work trip planning".to_string());
        agenda.create_item(&item).unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&work.id));
        let pending = agenda
            .list_pending_classification_suggestions_for_item(item.id)
            .unwrap();
        assert!(pending.is_empty());
    }

    #[test]
    fn literal_auto_apply_and_semantic_review_can_run_together() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let transport = Arc::new(FakeOllamaTransport {
            response: Some(
                r#"{"suggestions":[{"category":"Travel","confidence":0.9,"rationale":"trip planning"}]}"#
                    .to_string(),
            ),
        });
        let agenda = Agenda::with_ollama_transport(&store, &classifier, transport);

        let mut cfg = ClassificationConfig {
            literal_mode: LiteralClassificationMode::AutoApply,
            semantic_mode: SemanticClassificationMode::SuggestReview,
            ..ClassificationConfig::default()
        };
        cfg.ollama.enabled = true;
        cfg.set_provider_enabled(PROVIDER_ID_OLLAMA_OPENAI_COMPAT, true);
        store.set_classification_config(&cfg).unwrap();

        let work = category("Work", true);
        let travel = category("Travel", false);
        store.create_category(&work).unwrap();
        store.create_category(&travel).unwrap();

        let item = Item::new("Work trip planning".to_string());
        agenda.create_item(&item).unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&work.id));
        assert!(!assignments.contains_key(&travel.id));

        let pending = agenda
            .list_pending_classification_suggestions_for_item(item.id)
            .unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].provider_id, PROVIDER_ID_OLLAMA_OPENAI_COMPAT);
        assert!(matches!(
            pending[0].assignment,
            crate::classification::CandidateAssignment::Category(category_id)
                if category_id == travel.id
        ));
    }

    #[test]
    fn semantic_review_does_not_queue_duplicate_for_already_assigned_category() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let transport = Arc::new(FakeOllamaTransport {
            response: Some(
                r#"{"suggestions":[{"category":"Travel","confidence":0.95,"rationale":"travel intent"}]}"#
                    .to_string(),
            ),
        });
        let agenda = Agenda::with_ollama_transport(&store, &classifier, transport);

        let mut cfg = ClassificationConfig {
            literal_mode: LiteralClassificationMode::AutoApply,
            semantic_mode: SemanticClassificationMode::SuggestReview,
            ..ClassificationConfig::default()
        };
        cfg.ollama.enabled = true;
        cfg.set_provider_enabled(PROVIDER_ID_OLLAMA_OPENAI_COMPAT, true);
        store.set_classification_config(&cfg).unwrap();

        let travel = category("Travel", true);
        store.create_category(&travel).unwrap();

        let item = Item::new("Conference travel planning".to_string());
        let result = agenda.create_item(&item).unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&travel.id));
        assert_eq!(result.semantic_candidates_seen, 1);
        assert_eq!(result.semantic_candidates_queued_review, 0);
        assert_eq!(result.semantic_candidates_skipped_already_assigned, 1);

        let pending = agenda
            .list_pending_classification_suggestions_for_item(item.id)
            .unwrap();
        assert!(
            pending.is_empty(),
            "should not queue semantic duplicate for already-assigned category"
        );
    }

    #[test]
    fn semantic_mode_can_run_without_literal_matching() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let transport = Arc::new(FakeOllamaTransport {
            response: Some(
                r#"{"suggestions":[{"category":"Travel","confidence":0.7,"rationale":"travel intent"}]}"#
                    .to_string(),
            ),
        });
        let agenda = Agenda::with_ollama_transport(&store, &classifier, transport);

        let mut cfg = ClassificationConfig {
            literal_mode: LiteralClassificationMode::Off,
            semantic_mode: SemanticClassificationMode::SuggestReview,
            ..ClassificationConfig::default()
        };
        cfg.ollama.enabled = true;
        cfg.set_provider_enabled(PROVIDER_ID_OLLAMA_OPENAI_COMPAT, true);
        store.set_classification_config(&cfg).unwrap();

        let travel = category("Travel", true);
        store.create_category(&travel).unwrap();

        let item = Item::new("Plan a trip soon".to_string());
        agenda.create_item(&item).unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(!assignments.contains_key(&travel.id));
        let pending = agenda
            .list_pending_classification_suggestions_for_item(item.id)
            .unwrap();
        assert_eq!(pending.len(), 1);
    }

    #[test]
    fn literal_and_semantic_suggest_review_queue_both_category_suggestions() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let transport = Arc::new(FakeOllamaTransport {
            response: Some(
                r#"{"suggestions":[{"category":"Travel","confidence":0.88,"rationale":"trip planning"}]}"#
                    .to_string(),
            ),
        });
        let agenda = Agenda::with_ollama_transport(&store, &classifier, transport);

        let mut cfg = ClassificationConfig {
            literal_mode: LiteralClassificationMode::SuggestReview,
            semantic_mode: SemanticClassificationMode::SuggestReview,
            ..ClassificationConfig::default()
        };
        cfg.ollama.enabled = true;
        cfg.set_provider_enabled(PROVIDER_ID_OLLAMA_OPENAI_COMPAT, true);
        store.set_classification_config(&cfg).unwrap();

        let work = category("Work", true);
        let travel = category("Travel", false);
        store.create_category(&work).unwrap();
        store.create_category(&travel).unwrap();

        let item = Item::new("Work trip planning".to_string());
        agenda.create_item(&item).unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(!assignments.contains_key(&work.id));
        assert!(!assignments.contains_key(&travel.id));

        let pending = agenda
            .list_pending_classification_suggestions_for_item(item.id)
            .unwrap();
        assert_eq!(pending.len(), 2);
        assert!(pending.iter().any(|suggestion| {
            suggestion.provider_id == PROVIDER_ID_IMPLICIT_STRING
                && matches!(
                    suggestion.assignment,
                    crate::classification::CandidateAssignment::Category(category_id)
                        if category_id == work.id
                )
        }));
        assert!(pending.iter().any(|suggestion| {
            suggestion.provider_id == PROVIDER_ID_OLLAMA_OPENAI_COMPAT
                && matches!(
                    suggestion.assignment,
                    crate::classification::CandidateAssignment::Category(category_id)
                        if category_id == travel.id
                )
        }));
    }

    #[test]
    fn semantic_matching_is_independent_from_implicit_matching() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let transport = Arc::new(FakeOllamaTransport {
            response: Some(
                r#"{"suggestions":[{"category":"Travel","confidence":0.85,"rationale":"trip intent"}]}"#
                    .to_string(),
            ),
        });
        let agenda = Agenda::with_ollama_transport(&store, &classifier, transport);

        let mut cfg = ClassificationConfig {
            literal_mode: LiteralClassificationMode::Off,
            semantic_mode: SemanticClassificationMode::SuggestReview,
            ..ClassificationConfig::default()
        };
        cfg.ollama.enabled = true;
        cfg.set_provider_enabled(PROVIDER_ID_OLLAMA_OPENAI_COMPAT, true);
        store.set_classification_config(&cfg).unwrap();

        let mut travel = category("Travel", false);
        travel.enable_semantic_classification = true;
        store.create_category(&travel).unwrap();

        let item = Item::new("Need flights and a hotel".to_string());
        agenda.create_item(&item).unwrap();

        let pending = agenda
            .list_pending_classification_suggestions_for_item(item.id)
            .unwrap();
        assert_eq!(pending.len(), 1);
        assert!(matches!(
            pending[0].assignment,
            crate::classification::CandidateAssignment::Category(category_id)
                if category_id == travel.id
        ));
    }

    #[test]
    fn when_parser_follows_literal_policy_and_is_skipped_when_literal_mode_is_off() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let cfg = ClassificationConfig {
            literal_mode: LiteralClassificationMode::Off,
            semantic_mode: SemanticClassificationMode::Off,
            ..ClassificationConfig::default()
        };
        store.set_classification_config(&cfg).unwrap();

        let item = Item::new("Book travel next Tuesday".to_string());
        let result = agenda
            .create_item_with_reference_date(&item, date(2026, 3, 20))
            .unwrap();
        assert!(result.new_assignments.is_empty());

        let when_id = store
            .get_hierarchy()
            .expect("load hierarchy")
            .into_iter()
            .find(|category| category.name == RESERVED_CATEGORY_NAME_WHEN)
            .expect("reserved When category present")
            .id;
        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(!assignments.contains_key(&when_id));
    }

    #[test]
    #[ignore = "requires local Ollama with a mistral-compatible model running"]
    fn local_ollama_smoke_test_current_item_review_flow() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut cfg = ClassificationConfig {
            literal_mode: LiteralClassificationMode::AutoApply,
            semantic_mode: SemanticClassificationMode::SuggestReview,
            ..ClassificationConfig::default()
        };
        cfg.ollama.enabled = true;
        cfg.ollama.base_url = "http://127.0.0.1:11434/v1".to_string();
        cfg.ollama.model = "mistral".to_string();
        cfg.ollama.timeout_secs = 30;
        cfg.set_provider_enabled(PROVIDER_ID_OLLAMA_OPENAI_COMPAT, true);
        store.set_classification_config(&cfg).unwrap();

        let mut work = category("Work", true);
        work.enable_semantic_classification = false;
        let mut travel = category("Travel", false);
        travel.enable_semantic_classification = true;
        store.create_category(&work).unwrap();
        store.create_category(&travel).unwrap();

        let mut item = Item::new("Work trip planning for conference travel".to_string());
        item.note = Some("Book flights, hotel, and local transport".to_string());
        agenda.create_item(&item).unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(
            assignments.contains_key(&work.id),
            "expected literal Work assignment to auto-apply"
        );
        assert!(
            !assignments.contains_key(&travel.id),
            "semantic Travel should queue for review before acceptance"
        );

        let pending = agenda
            .list_pending_classification_suggestions_for_item(item.id)
            .unwrap();
        assert!(
            pending.iter().any(|suggestion| {
                suggestion.provider_id == PROVIDER_ID_OLLAMA_OPENAI_COMPAT
                    && matches!(
                        suggestion.assignment,
                        crate::classification::CandidateAssignment::Category(category_id)
                            if category_id == travel.id
                    )
            }),
            "expected a pending Ollama travel suggestion, got: {pending:?}"
        );

        let travel_suggestion = pending
            .iter()
            .find(|suggestion| {
                matches!(
                    suggestion.assignment,
                    crate::classification::CandidateAssignment::Category(category_id)
                        if category_id == travel.id
                )
            })
            .expect("travel suggestion present");

        agenda
            .accept_classification_suggestion(travel_suggestion.id)
            .unwrap();

        let reloaded_assignments = store.get_assignments_for_item(item.id).unwrap();
        let travel_assignment = reloaded_assignments
            .get(&travel.id)
            .expect("travel assignment should exist after acceptance");
        assert_eq!(
            travel_assignment.source,
            AssignmentSource::SuggestionAccepted
        );

        let reloaded_suggestion = store
            .get_classification_suggestion(travel_suggestion.id)
            .unwrap()
            .expect("accepted suggestion exists");
        assert_eq!(reloaded_suggestion.status, SuggestionStatus::Accepted);
    }

    #[test]
    fn create_item_hashtag_matches_existing_categories_without_creating_hash_category() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut priority = category("Priority", false);
        priority.is_exclusive = true;
        store.create_category(&priority).unwrap();
        let high = child_category("High", priority.id, true);
        store.create_category(&high).unwrap();
        let follow_up = category("Follow-up", true);
        store.create_category(&follow_up).unwrap();

        let item = Item::new("Hashtag parsing test #high #FOLLOW-UP".to_string());
        agenda.create_item(&item).unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&high.id));
        assert!(assignments.contains_key(&priority.id));
        assert!(assignments.contains_key(&follow_up.id));

        assert!(category_id_by_name(&store, "#high").is_none());
        assert!(category_id_by_name(&store, "#follow-up").is_none());
    }

    #[test]
    fn create_item_unknown_hashtag_does_not_auto_create_category() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let item = Item::new("Unknown hashtag behavior test #office".to_string());
        let _ = agenda.create_item(&item).unwrap();

        assert!(category_id_by_name(&store, "Office").is_none());
        assert!(category_id_by_name(&store, "#office").is_none());

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.is_empty());
    }

    #[test]
    fn update_item_triggers_reclassification() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let urgent = category("Urgent", true);
        store.create_category(&urgent).unwrap();

        let item = Item::new("normal task".to_string());
        agenda.create_item(&item).unwrap();
        assert!(!store
            .get_assignments_for_item(item.id)
            .unwrap()
            .contains_key(&urgent.id));

        let mut updated = store.get_item(item.id).unwrap();
        updated.text = "Urgent task".to_string();
        updated.modified_at = Timestamp::now();

        let result = agenda.update_item(&updated).unwrap();
        assert!(result.new_assignments.contains(&urgent.id));
        assert!(store
            .get_assignments_for_item(item.id)
            .unwrap()
            .contains_key(&urgent.id));
    }

    #[test]
    fn create_item_parses_date_and_sets_when_provenance() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);
        let when_id = when_category_id(&store);

        let item = Item::new("next Tuesday at 3pm".to_string());
        agenda
            .create_item_with_reference_date(&item, date(2026, 2, 18))
            .unwrap();

        let loaded = store.get_item(item.id).unwrap();
        assert_eq!(loaded.when_date, Some(datetime(2026, 2, 24, 15, 0)));

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        let when_assignment = assignments.get(&when_id).expect("when assignment exists");
        assert_eq!(when_assignment.source, AssignmentSource::AutoClassified);
        assert_eq!(when_assignment.origin.as_deref(), Some("nlp:date"));
    }

    #[test]
    fn update_item_parses_new_date_text_and_sets_when_date() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let item = Item::new("plain task".to_string());
        agenda
            .create_item_with_reference_date(&item, date(2026, 2, 16))
            .unwrap();

        let mut updated = store.get_item(item.id).unwrap();
        updated.text = "today at noon".to_string();
        updated.modified_at = Timestamp::now();

        agenda
            .update_item_with_reference_date(&updated, date(2026, 2, 16))
            .unwrap();

        let loaded = store.get_item(item.id).unwrap();
        assert_eq!(loaded.when_date, Some(datetime(2026, 2, 16, 12, 0)));
    }

    #[test]
    fn update_item_without_parse_does_not_auto_clear_when_date() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let item = Item::new("tomorrow".to_string());
        agenda
            .create_item_with_reference_date(&item, date(2026, 2, 16))
            .unwrap();

        let mut updated = store.get_item(item.id).unwrap();
        updated.text = "just notes now".to_string();
        updated.modified_at = Timestamp::now();

        agenda
            .update_item_with_reference_date(&updated, date(2026, 2, 16))
            .unwrap();

        let loaded = store.get_item(item.id).unwrap();
        assert_eq!(loaded.when_date, Some(datetime(2026, 2, 17, 0, 0)));
    }

    #[test]
    fn update_item_note_only_does_not_reparse_relative_when_date() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let item = Item::new("tomorrow".to_string());
        agenda
            .create_item_with_reference_date(&item, date(2026, 2, 16))
            .unwrap();

        let mut updated = store.get_item(item.id).unwrap();
        updated.note = Some("added note text".to_string());
        updated.modified_at = Timestamp::now();

        agenda
            .update_item_with_reference_date(&updated, date(2026, 2, 20))
            .unwrap();

        let loaded = store.get_item(item.id).unwrap();
        assert_eq!(
            loaded.when_date,
            Some(datetime(2026, 2, 17, 0, 0)),
            "note-only edits should not reparse relative date text"
        );
    }

    #[test]
    fn set_item_when_date_assigns_reserved_when_with_manual_provenance() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);
        let when_id = when_category_id(&store);

        let item = Item::new("plain item".to_string());
        store.create_item(&item).unwrap();

        let target_when = datetime(2026, 2, 20, 9, 30);
        agenda
            .set_item_when_date(
                item.id,
                Some(target_when),
                Some("manual:test.when-edit".to_string()),
            )
            .unwrap();

        let loaded = store.get_item(item.id).unwrap();
        assert_eq!(loaded.when_date, Some(target_when));

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        let when_assignment = assignments.get(&when_id).expect("when assignment exists");
        assert_eq!(when_assignment.source, AssignmentSource::Manual);
        assert_eq!(
            when_assignment.origin.as_deref(),
            Some("manual:test.when-edit")
        );
        assert!(when_assignment.sticky);
    }

    #[test]
    fn set_item_when_date_uses_default_manual_origin_when_none_is_provided() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);
        let when_id = when_category_id(&store);

        let item = Item::new("plain item".to_string());
        store.create_item(&item).unwrap();

        agenda
            .set_item_when_date(item.id, Some(datetime(2026, 2, 20, 9, 30)), None)
            .unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        let when_assignment = assignments.get(&when_id).expect("when assignment exists");
        assert_eq!(when_assignment.source, AssignmentSource::Manual);
        assert_eq!(when_assignment.origin.as_deref(), Some("manual:when"));
    }

    #[test]
    fn set_item_when_date_none_clears_datetime_and_reserved_when_assignment() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);
        let when_id = when_category_id(&store);

        let item = Item::new("tomorrow".to_string());
        agenda
            .create_item_with_reference_date(&item, date(2026, 2, 16))
            .unwrap();
        assert!(store
            .get_assignments_for_item(item.id)
            .unwrap()
            .contains_key(&when_id));

        agenda
            .set_item_when_date(item.id, None, Some("manual:test.when-clear".to_string()))
            .unwrap();

        let loaded = store.get_item(item.id).unwrap();
        assert_eq!(loaded.when_date, None);
        assert!(
            !store
                .get_assignments_for_item(item.id)
                .unwrap()
                .contains_key(&when_id),
            "clearing when_date should unassign reserved When"
        );
    }

    #[test]
    fn parsed_when_date_places_item_in_expected_bucket() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);
        let reference_date = date(2026, 2, 16);

        let item = Item::new("today at noon".to_string());
        agenda
            .create_item_with_reference_date(&item, reference_date)
            .unwrap();

        let loaded = store.get_item(item.id).unwrap();
        let bucket = resolve_when_bucket(loaded.when_date, reference_date);
        assert_eq!(bucket, WhenBucket::Today);
    }

    #[test]
    fn create_category_triggers_retroactive_classification() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let sarah_item = Item::new("Sarah's meeting".to_string());
        let bob_item = Item::new("Bob's lunch".to_string());
        store.create_item(&sarah_item).unwrap();
        store.create_item(&bob_item).unwrap();

        let sarah = category("Sarah", true);
        let result = agenda.create_category(&sarah).unwrap();
        assert_eq!(result.processed_items, 2);
        assert_eq!(result.affected_items, 1);

        let sarah_assignments = store.get_assignments_for_item(sarah_item.id).unwrap();
        let bob_assignments = store.get_assignments_for_item(bob_item.id).unwrap();
        assert!(sarah_assignments.contains_key(&sarah.id));
        assert!(!bob_assignments.contains_key(&sarah.id));
    }

    #[test]
    fn update_category_triggers_reclassification() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let foo = category("Foo", true);
        agenda.create_category(&foo).unwrap();

        let existing = Item::new("meeting with Foo".to_string());
        agenda.create_item(&existing).unwrap();
        assert!(store
            .get_assignments_for_item(existing.id)
            .unwrap()
            .contains_key(&foo.id));

        let mut renamed = store.get_category(foo.id).unwrap();
        renamed.name = "Bar".to_string();
        let update_result = agenda.update_category(&renamed).unwrap();
        assert_eq!(update_result.processed_items, 1);

        let existing_after = store.get_assignments_for_item(existing.id).unwrap();
        assert!(existing_after.contains_key(&foo.id));

        let new_item = Item::new("meeting with Bar".to_string());
        agenda.create_item(&new_item).unwrap();
        let new_assignments = store.get_assignments_for_item(new_item.id).unwrap();
        assert!(new_assignments.contains_key(&foo.id));
    }

    #[test]
    fn manual_assignment_triggers_cascade() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let urgent = category("Urgent", false);
        store.create_category(&urgent).unwrap();

        let mut escalated = category("Escalated", false);
        let mut criteria = Query::default();
        criteria.set_criterion(CriterionMode::And, urgent.id);
        escalated.conditions.push(Condition::Profile {
            criteria: Box::new(criteria),
        });
        store.create_category(&escalated).unwrap();

        let item = Item::new("Task".to_string());
        store.create_item(&item).unwrap();

        let result = agenda
            .assign_item_manual(item.id, urgent.id, Some("manual:user".to_string()))
            .unwrap();
        assert!(result.new_assignments.contains(&escalated.id));

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert_eq!(
            assignments.get(&urgent.id).unwrap().source,
            AssignmentSource::Manual
        );
        assert!(assignments.contains_key(&escalated.id));
    }

    #[test]
    fn manual_assignment_applies_subsumption_to_all_ancestors() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let work = category("Work", false);
        store.create_category(&work).unwrap();
        let project_y = child_category("Project Y", work.id, false);
        store.create_category(&project_y).unwrap();
        let frabulator = child_category("Frabulator", project_y.id, false);
        store.create_category(&frabulator).unwrap();

        let item = Item::new("Talk to Sarah".to_string());
        store.create_item(&item).unwrap();

        agenda
            .assign_item_manual(item.id, frabulator.id, Some("manual:user".to_string()))
            .unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert_eq!(
            assignments
                .get(&frabulator.id)
                .map(|assignment| assignment.source),
            Some(AssignmentSource::Manual)
        );
        assert_eq!(
            assignments
                .get(&project_y.id)
                .map(|assignment| assignment.source),
            Some(AssignmentSource::Subsumption)
        );
        assert_eq!(
            assignments
                .get(&work.id)
                .map(|assignment| assignment.source),
            Some(AssignmentSource::Subsumption)
        );
    }

    #[test]
    fn preview_manual_category_toggle_uses_reprocessed_assignments() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let work = category("Work", false);
        store.create_category(&work).unwrap();
        let project_y = child_category("Project Y", work.id, false);
        store.create_category(&project_y).unwrap();

        let item = Item::new("Talk to Sarah".to_string());
        store.create_item(&item).unwrap();

        let preview = agenda
            .preview_manual_category_toggle(item.id, project_y.id)
            .unwrap();

        assert_eq!(
            preview
                .assignments
                .get(&project_y.id)
                .map(|assignment| assignment.source),
            Some(AssignmentSource::Manual)
        );
        assert_eq!(
            preview
                .assignments
                .get(&work.id)
                .map(|assignment| assignment.source),
            Some(AssignmentSource::Subsumption),
            "preview should include the same subsumption ancestor the real assign path creates"
        );
        assert!(
            !store
                .get_assignments_for_item(item.id)
                .unwrap()
                .contains_key(&work.id),
            "preview should not mutate the real store"
        );
    }

    #[test]
    fn manual_unassign_blocks_removing_ancestor_while_descendant_assigned() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let work = category("Work", false);
        store.create_category(&work).unwrap();
        let project_y = child_category("Project Y", work.id, false);
        store.create_category(&project_y).unwrap();

        let item = Item::new("Kickoff".to_string());
        store.create_item(&item).unwrap();
        agenda
            .assign_item_manual(item.id, project_y.id, Some("manual:user".to_string()))
            .unwrap();

        let err = agenda.unassign_item_manual(item.id, work.id).unwrap_err();
        assert!(matches!(err, AgendaError::InvalidOperation { .. }));
        let message = err.to_string();
        assert!(message.contains("cannot remove category"));
        assert!(message.contains("Project Y"));

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&project_y.id));
        assert!(assignments.contains_key(&work.id));
    }

    #[test]
    fn manual_unassign_removes_live_subsumption_ancestor() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let work = category("Work", false);
        store.create_category(&work).unwrap();
        let project_y = child_category("Project Y", work.id, false);
        store.create_category(&project_y).unwrap();

        let item = Item::new("Kickoff".to_string());
        store.create_item(&item).unwrap();
        agenda
            .assign_item_manual(item.id, project_y.id, Some("manual:user".to_string()))
            .unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        let ancestor = assignments.get(&work.id).unwrap();
        assert_eq!(ancestor.source, AssignmentSource::Subsumption);
        assert!(!ancestor.sticky);

        agenda.unassign_item_manual(item.id, project_y.id).unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(!assignments.contains_key(&project_y.id));
        assert!(
            !assignments.contains_key(&work.id),
            "subsumption ancestor should auto-break once the supporting descendant is removed"
        );
    }

    #[test]
    fn manual_unassign_allows_removing_leaf_then_parent() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let work = category("Work", false);
        store.create_category(&work).unwrap();
        let project_y = child_category("Project Y", work.id, false);
        store.create_category(&project_y).unwrap();

        let item = Item::new("Kickoff".to_string());
        store.create_item(&item).unwrap();
        agenda
            .assign_item_manual(item.id, project_y.id, Some("manual:user".to_string()))
            .unwrap();

        agenda.unassign_item_manual(item.id, project_y.id).unwrap();
        agenda.unassign_item_manual(item.id, work.id).unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(!assignments.contains_key(&project_y.id));
        assert!(!assignments.contains_key(&work.id));
    }

    #[test]
    fn manual_assignment_enforces_exclusive_siblings() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut priority = category("Priority", false);
        priority.is_exclusive = true;
        store.create_category(&priority).unwrap();

        let high = child_category("High", priority.id, false);
        let medium = child_category("Medium", priority.id, false);
        store.create_category(&high).unwrap();
        store.create_category(&medium).unwrap();

        let item = Item::new("Finish report".to_string());
        store.create_item(&item).unwrap();

        agenda
            .assign_item_manual(item.id, high.id, Some("manual:user".to_string()))
            .unwrap();
        agenda
            .assign_item_manual(item.id, medium.id, Some("manual:user".to_string()))
            .unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(!assignments.contains_key(&high.id));
        assert!(assignments.contains_key(&medium.id));
    }

    #[test]
    fn claim_item_manual_rejects_when_precondition_category_is_already_assigned() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut status = category("Status", false);
        status.is_exclusive = true;
        store.create_category(&status).unwrap();
        let in_progress = child_category("In Progress", status.id, false);
        let complete = child_category("Complete", status.id, false);
        store.create_category(&in_progress).unwrap();
        store.create_category(&complete).unwrap();

        let item = Item::new("Task".to_string());
        store.create_item(&item).unwrap();
        agenda
            .assign_item_manual(item.id, complete.id, Some("manual:test".to_string()))
            .unwrap();

        let err = agenda
            .claim_item_manual(
                item.id,
                in_progress.id,
                &[in_progress.id, complete.id],
                Some("manual:test.claim".to_string()),
            )
            .expect_err("claim should fail when complete is assigned");
        assert!(matches!(err, AgendaError::InvalidOperation { .. }));
        let msg = err.to_string();
        assert!(msg.contains("claim precondition failed"));
        assert!(msg.contains("Complete"));

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&complete.id));
        assert!(!assignments.contains_key(&in_progress.id));
    }

    #[test]
    fn claim_item_manual_race_allows_only_one_winner() {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let db_path = std::env::temp_dir().join(format!("agenda-claim-race-{nanos}.ag"));

        let (item_id, ready_id, in_progress_id, complete_id) = {
            let store = Store::open(&db_path).expect("open temp db");
            let classifier = SubstringClassifier;
            let agenda = Agenda::new(&store, &classifier);

            let mut status = category("Status", false);
            status.is_exclusive = true;
            store.create_category(&status).expect("create status");
            let ready = child_category("Ready", status.id, false);
            let in_progress = child_category("In Progress", status.id, false);
            let complete = child_category("Complete", status.id, false);
            store.create_category(&ready).expect("create ready");
            store
                .create_category(&in_progress)
                .expect("create in progress");
            store.create_category(&complete).expect("create complete");

            let item = Item::new("Concurrent claim target".to_string());
            store.create_item(&item).expect("create item");
            agenda
                .assign_item_manual(item.id, ready.id, Some("manual:test".to_string()))
                .expect("assign ready");
            (item.id, ready.id, in_progress.id, complete.id)
        };

        let barrier = Arc::new(Barrier::new(2));
        let mut handles = Vec::new();
        for _ in 0..2 {
            let db_path = db_path.clone();
            let barrier = Arc::clone(&barrier);
            handles.push(thread::spawn(move || {
                let store = Store::open(&db_path).expect("open raced store");
                let classifier = SubstringClassifier;
                let agenda = Agenda::new(&store, &classifier);
                barrier.wait();
                agenda
                    .claim_item_manual(
                        item_id,
                        in_progress_id,
                        &[in_progress_id, complete_id],
                        Some("manual:test.claim".to_string()),
                    )
                    .map(|_| ())
                    .map_err(|err| err.to_string())
            }));
        }

        let outcomes: Vec<Result<(), String>> = handles
            .into_iter()
            .map(|handle| handle.join().expect("thread result"))
            .collect();

        let success_count = outcomes.iter().filter(|result| result.is_ok()).count();
        assert_eq!(success_count, 1, "exactly one claim should succeed");
        let failure_messages: Vec<&str> = outcomes
            .iter()
            .filter_map(|result| result.as_ref().err().map(String::as_str))
            .collect();
        assert_eq!(failure_messages.len(), 1);
        assert!(
            failure_messages[0].contains("claim precondition failed"),
            "expected precondition failure, got: {}",
            failure_messages[0]
        );

        let verify_store = Store::open(&db_path).expect("open verify store");
        let assignments = verify_store
            .get_assignments_for_item(item_id)
            .expect("load assignments");
        assert!(assignments.contains_key(&in_progress_id));
        assert!(!assignments.contains_key(&ready_id));
        assert!(!assignments.contains_key(&complete_id));

        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_file(format!("{}-wal", db_path.display()));
        let _ = std::fs::remove_file(format!("{}-shm", db_path.display()));
    }

    #[test]
    fn assign_item_numeric_manual_sets_payload_and_subsumption_ancestor_has_none() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let project = category("Project", false);
        store.create_category(&project).unwrap();
        let mut cost = child_category("Cost", project.id, false);
        cost.value_kind = CategoryValueKind::Numeric;
        store.create_category(&cost).unwrap();

        let item = Item::new("Vendor invoice".to_string());
        store.create_item(&item).unwrap();

        agenda
            .assign_item_numeric_manual(
                item.id,
                cost.id,
                Decimal::new(24596, 2),
                Some("manual:test".to_string()),
            )
            .unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert_eq!(
            assignments.get(&cost.id).and_then(|a| a.numeric_value),
            Some(Decimal::new(24596, 2))
        );
        assert_eq!(
            assignments.get(&project.id).and_then(|a| a.numeric_value),
            None
        );
        assert_eq!(
            assignments.get(&project.id).map(|a| a.source),
            Some(AssignmentSource::Subsumption)
        );
    }

    #[test]
    fn assign_item_numeric_manual_rejects_non_numeric_category() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let tag = category("TagOnly", false);
        store.create_category(&tag).unwrap();

        let item = Item::new("Test".to_string());
        store.create_item(&item).unwrap();

        let err = agenda
            .assign_item_numeric_manual(item.id, tag.id, Decimal::new(10, 0), None)
            .unwrap_err();
        assert!(matches!(err, AgendaError::InvalidOperation { .. }));
        assert!(err.to_string().contains("not Numeric"));
    }

    #[test]
    fn manual_assignment_rejects_duplicate_category_names() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let work = category("Work", false);
        store.create_category(&work).unwrap();
        let project_x = child_category("Project X", work.id, false);
        store.create_category(&project_x).unwrap();

        let mut work_priority = child_category("Priority", work.id, false);
        work_priority.is_exclusive = true;
        agenda.create_category(&work_priority).unwrap();

        let mut project_priority = child_category("Priority", project_x.id, false);
        project_priority.is_exclusive = true;
        let err = agenda.create_category(&project_priority).unwrap_err();
        assert!(matches!(err, AgendaError::DuplicateName { .. }));
    }

    #[test]
    fn manual_assignment_enforces_exclusivity_per_priority_branch() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let work = category("Work", false);
        store.create_category(&work).unwrap();
        let project_x = child_category("Project X", work.id, false);
        store.create_category(&project_x).unwrap();

        let mut work_priority = child_category("Priority", work.id, false);
        work_priority.is_exclusive = true;
        store.create_category(&work_priority).unwrap();
        let work_high = child_category("High", work_priority.id, false);
        let work_medium = child_category("Medium", work_priority.id, false);
        store.create_category(&work_high).unwrap();
        store.create_category(&work_medium).unwrap();

        let mut project_priority = child_category("Project X Priority", project_x.id, false);
        project_priority.is_exclusive = true;
        store.create_category(&project_priority).unwrap();
        let project_high = child_category("Project X High", project_priority.id, false);
        let project_medium = child_category("Project X Medium", project_priority.id, false);
        store.create_category(&project_high).unwrap();
        store.create_category(&project_medium).unwrap();

        let item = Item::new("Prepare sprint plan".to_string());
        store.create_item(&item).unwrap();

        agenda
            .assign_item_manual(item.id, work_high.id, Some("manual:user".to_string()))
            .unwrap();
        agenda
            .assign_item_manual(item.id, project_high.id, Some("manual:user".to_string()))
            .unwrap();
        agenda
            .assign_item_manual(item.id, work_medium.id, Some("manual:user".to_string()))
            .unwrap();
        agenda
            .assign_item_manual(item.id, project_medium.id, Some("manual:user".to_string()))
            .unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(!assignments.contains_key(&work_high.id));
        assert!(assignments.contains_key(&work_medium.id));
        assert!(!assignments.contains_key(&project_high.id));
        assert!(assignments.contains_key(&project_medium.id));
    }

    #[test]
    fn engine_error_does_not_prevent_store_mutation() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut stages = Vec::new();
        for index in 1..=11 {
            let stage = category(&format!("Stage{index}"), false);
            store.create_category(&stage).unwrap();
            stages.push(stage);
        }

        for index in 0..10 {
            let mut stage = store.get_category(stages[index].id).unwrap();
            let mut criteria = Query::default();
            criteria.set_criterion(CriterionMode::And, stages[index + 1].id);
            stage.conditions = vec![Condition::Profile {
                criteria: Box::new(criteria),
            }];
            store.update_category(&stage).unwrap();
        }

        let mut trigger = category("Trigger", true);
        trigger.actions.push(Action::Assign {
            targets: HashSet::from([stages[10].id]),
        });
        store.create_category(&trigger).unwrap();

        let item = Item::new("Trigger this chain".to_string());
        let err = agenda.create_item(&item).unwrap_err();
        match err {
            AgendaError::InvalidOperation { message } => {
                assert!(message.contains("exceeded 10 passes"));
            }
            other => panic!("unexpected error: {other:?}"),
        }

        let loaded = store.get_item(item.id).unwrap();
        assert_eq!(loaded.text, "Trigger this chain");
    }

    #[test]
    fn end_to_end_workflow_runs_automatically() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let events = category("Events", false);
        agenda.create_category(&events).unwrap();

        let calendar = child_category("Calendar", events.id, false);
        agenda.create_category(&calendar).unwrap();

        let mut meetings = category("Meetings", true);
        meetings.actions.push(Action::Assign {
            targets: HashSet::from([calendar.id]),
        });
        agenda.create_category(&meetings).unwrap();

        let item = Item::new("Team meetings tomorrow".to_string());
        agenda.create_item(&item).unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert_eq!(
            assignments.get(&meetings.id).unwrap().source,
            AssignmentSource::AutoClassified
        );
        assert_eq!(
            assignments.get(&calendar.id).unwrap().source,
            AssignmentSource::Action
        );
        assert_eq!(
            assignments.get(&events.id).unwrap().source,
            AssignmentSource::Subsumption
        );
    }

    #[test]
    fn insert_item_in_section_assigns_section_and_view_categories() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let work = category("Work", false);
        let urgent = category("Urgent", false);
        store.create_category(&work).unwrap();
        store.create_category(&urgent).unwrap();

        let item = Item::new("task".to_string());
        store.create_item(&item).unwrap();

        let mut current_view = view("My Work");
        current_view
            .criteria
            .set_criterion(CriterionMode::And, work.id);
        let mut current_section = section("Urgent");
        current_section.on_insert_assign.insert(urgent.id);

        agenda
            .insert_item_in_section(item.id, &current_view, &current_section)
            .unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&work.id));
        assert!(assignments.contains_key(&urgent.id));
        assert_eq!(
            assignments.get(&work.id).and_then(|a| a.origin.as_deref()),
            Some("edit:section.insert")
        );
        assert_eq!(
            assignments
                .get(&urgent.id)
                .and_then(|a| a.origin.as_deref()),
            Some("edit:section.insert")
        );
    }

    #[test]
    fn insert_item_in_section_assigns_section_criteria_include_categories() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let p0 = category("P0", false);
        store.create_category(&p0).unwrap();

        let item = Item::new("task".to_string());
        store.create_item(&item).unwrap();

        let current_view = view("Board");
        let mut current_section = section("P0");
        current_section
            .criteria
            .set_criterion(CriterionMode::And, p0.id);

        agenda
            .insert_item_in_section(item.id, &current_view, &current_section)
            .unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&p0.id));
        assert_eq!(
            assignments.get(&p0.id).and_then(|a| a.origin.as_deref()),
            Some("edit:section.insert")
        );
    }

    #[test]
    fn insert_item_in_section_triggers_engine_cascade() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let work = category("Work", false);
        let urgent = category("Urgent", false);
        store.create_category(&work).unwrap();
        store.create_category(&urgent).unwrap();

        let mut escalated = category("Escalated", false);
        let mut criteria = Query::default();
        criteria.set_criterion(CriterionMode::And, work.id);
        criteria.set_criterion(CriterionMode::And, urgent.id);
        escalated.conditions.push(Condition::Profile {
            criteria: Box::new(criteria),
        });
        store.create_category(&escalated).unwrap();

        let item = Item::new("task".to_string());
        store.create_item(&item).unwrap();

        let mut current_view = view("My Work");
        current_view
            .criteria
            .set_criterion(CriterionMode::And, work.id);
        let mut current_section = section("Urgent");
        current_section.on_insert_assign.insert(urgent.id);

        let result = agenda
            .insert_item_in_section(item.id, &current_view, &current_section)
            .unwrap();

        assert!(result.new_assignments.contains(&escalated.id));
        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&escalated.id));
    }

    #[test]
    fn insert_item_in_section_applies_subsumption_for_manual_section_assignments() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let work = category("Work", false);
        store.create_category(&work).unwrap();
        let project_y = child_category("Project Y", work.id, false);
        store.create_category(&project_y).unwrap();

        let item = Item::new("task".to_string());
        store.create_item(&item).unwrap();

        let mut current_view = view("Project Y Board");
        current_view
            .criteria
            .set_criterion(CriterionMode::And, project_y.id);
        let mut current_section = section("Project Y");
        current_section.on_insert_assign.insert(project_y.id);

        agenda
            .insert_item_in_section(item.id, &current_view, &current_section)
            .unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&project_y.id));
        assert!(assignments.contains_key(&work.id));
        assert_eq!(
            assignments
                .get(&work.id)
                .map(|assignment| assignment.source),
            Some(AssignmentSource::Subsumption)
        );
    }

    #[test]
    fn move_between_sections_uses_structural_diff_without_manual_remove_targets() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let work = category("Work", false);
        let ready = category("Ready", false);
        let in_progress = category("In Progress", false);
        let personal = category("Personal", false);
        for category in [&work, &ready, &in_progress, &personal] {
            store.create_category(category).unwrap();
        }

        let item = Item::new("task".to_string());
        store.create_item(&item).unwrap();
        for category_id in [work.id, ready.id, personal.id] {
            store
                .assign_item(item.id, category_id, &manual_assignment("manual:user"))
                .unwrap();
        }

        let mut current_view = view("Work Board");
        current_view
            .criteria
            .set_criterion(CriterionMode::And, work.id);
        let mut source_section = section("Ready");
        source_section
            .criteria
            .set_criterion(CriterionMode::And, ready.id);
        let mut target_section = section("In Progress");
        target_section
            .criteria
            .set_criterion(CriterionMode::And, in_progress.id);

        agenda
            .move_item_between_sections(item.id, &current_view, &source_section, &target_section)
            .unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&work.id));
        assert!(!assignments.contains_key(&ready.id));
        assert!(assignments.contains_key(&in_progress.id));
        assert!(assignments.contains_key(&personal.id));
    }

    #[test]
    fn move_between_sections_honors_on_remove_side_effects() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let ready = category("Ready", false);
        let in_progress = category("In Progress", false);
        let needs_review = category("Needs Review", false);
        for category in [&ready, &in_progress, &needs_review] {
            store.create_category(category).unwrap();
        }

        let item = Item::new("task".to_string());
        store.create_item(&item).unwrap();
        for category_id in [ready.id, needs_review.id] {
            store
                .assign_item(item.id, category_id, &manual_assignment("manual:user"))
                .unwrap();
        }

        let mut source_section = section("Ready");
        source_section
            .criteria
            .set_criterion(CriterionMode::And, ready.id);
        source_section.on_remove_unassign.insert(needs_review.id);

        let mut target_section = section("In Progress");
        target_section
            .criteria
            .set_criterion(CriterionMode::And, in_progress.id);

        agenda
            .move_item_between_sections(item.id, &view("Board"), &source_section, &target_section)
            .unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(!assignments.contains_key(&ready.id));
        assert!(assignments.contains_key(&in_progress.id));
        assert!(!assignments.contains_key(&needs_review.id));
    }

    #[test]
    fn move_between_sections_preserves_overlapping_categories() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let shared = category("Shared", false);
        let urgent = category("Urgent", false);
        let next = category("Next", false);
        for category in [&shared, &urgent, &next] {
            store.create_category(category).unwrap();
        }

        let item = Item::new("task".to_string());
        store.create_item(&item).unwrap();
        for category_id in [shared.id, urgent.id] {
            store
                .assign_item(item.id, category_id, &manual_assignment("manual:user"))
                .unwrap();
        }

        let mut source_section = section("Urgent");
        source_section
            .criteria
            .set_criterion(CriterionMode::And, shared.id);
        source_section
            .criteria
            .set_criterion(CriterionMode::And, urgent.id);

        let mut target_section = section("Next");
        target_section
            .criteria
            .set_criterion(CriterionMode::And, shared.id);
        target_section
            .criteria
            .set_criterion(CriterionMode::And, next.id);

        agenda
            .move_item_between_sections(item.id, &view("Board"), &source_section, &target_section)
            .unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&shared.id));
        assert!(!assignments.contains_key(&urgent.id));
        assert!(assignments.contains_key(&next.id));
    }

    #[test]
    fn move_between_generated_subsections_swaps_child_assignment() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let parent = category("Project", false);
        store.create_category(&parent).unwrap();
        let alpha = child_category("Alpha", parent.id, false);
        let beta = child_category("Beta", parent.id, false);
        store.create_category(&alpha).unwrap();
        store.create_category(&beta).unwrap();

        let item = Item::new("task".to_string());
        store.create_item(&item).unwrap();
        for category_id in [parent.id, alpha.id] {
            store
                .assign_item(item.id, category_id, &manual_assignment("manual:user"))
                .unwrap();
        }

        let mut source_section = section("Project");
        source_section
            .criteria
            .set_criterion(CriterionMode::And, parent.id);
        source_section.on_insert_assign.insert(alpha.id);

        let mut target_section = section("Project");
        target_section
            .criteria
            .set_criterion(CriterionMode::And, parent.id);
        target_section.on_insert_assign.insert(beta.id);

        agenda
            .move_item_between_sections(item.id, &view("Board"), &source_section, &target_section)
            .unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&parent.id));
        assert!(!assignments.contains_key(&alpha.id));
        assert!(assignments.contains_key(&beta.id));
    }

    #[test]
    fn remove_from_section_preserves_view_criteria_and_strips_structural_targets() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let work = category("Work", false);
        let urgent = category("Urgent", false);
        let personal = category("Personal", false);
        store.create_category(&work).unwrap();
        store.create_category(&urgent).unwrap();
        store.create_category(&personal).unwrap();

        let item = Item::new("task".to_string());
        store.create_item(&item).unwrap();
        store
            .assign_item(item.id, work.id, &manual_assignment("manual:user"))
            .unwrap();
        store
            .assign_item(item.id, urgent.id, &manual_assignment("manual:user"))
            .unwrap();
        store
            .assign_item(item.id, personal.id, &manual_assignment("manual:user"))
            .unwrap();

        let mut current_view = view("Work");
        current_view
            .criteria
            .set_criterion(CriterionMode::And, work.id);
        let mut current_section = section("Urgent");
        current_section
            .criteria
            .set_criterion(CriterionMode::And, urgent.id);

        agenda
            .remove_item_from_section(item.id, &current_view, &current_section)
            .unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&work.id));
        assert!(!assignments.contains_key(&urgent.id));
        assert!(assignments.contains_key(&personal.id));
    }

    #[test]
    fn remove_from_section_honors_on_remove_side_effects() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let work = category("Work", false);
        let urgent = category("Urgent", false);
        let lane_marker = category("Lane Marker", false);
        let review_flag = category("Needs Review", false);
        for category in [&work, &urgent, &lane_marker, &review_flag] {
            store.create_category(category).unwrap();
        }

        let item = Item::new("task".to_string());
        store.create_item(&item).unwrap();
        for category_id in [work.id, urgent.id, lane_marker.id, review_flag.id] {
            store
                .assign_item(item.id, category_id, &manual_assignment("manual:user"))
                .unwrap();
        }

        let mut current_view = view("Work");
        current_view
            .criteria
            .set_criterion(CriterionMode::And, work.id);
        let mut current_section = section("Urgent");
        current_section
            .criteria
            .set_criterion(CriterionMode::And, urgent.id);
        current_section.on_insert_assign.insert(lane_marker.id);
        current_section.on_remove_unassign.insert(review_flag.id);

        agenda
            .remove_item_from_section(item.id, &current_view, &current_section)
            .unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&work.id));
        assert!(!assignments.contains_key(&urgent.id));
        assert!(!assignments.contains_key(&lane_marker.id));
        assert!(!assignments.contains_key(&review_flag.id));
    }

    #[test]
    fn remove_from_view_unassigns_view_targets() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let work = category("Work", false);
        let personal = category("Personal", false);
        store.create_category(&work).unwrap();
        store.create_category(&personal).unwrap();

        let item = Item::new("task".to_string());
        store.create_item(&item).unwrap();
        store
            .assign_item(item.id, work.id, &manual_assignment("manual:user"))
            .unwrap();
        store
            .assign_item(item.id, personal.id, &manual_assignment("manual:user"))
            .unwrap();

        let mut current_view = view("My Work");
        current_view.remove_from_view_unassign.insert(work.id);

        agenda
            .remove_item_from_view(item.id, &current_view)
            .unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(!assignments.contains_key(&work.id));
        assert!(assignments.contains_key(&personal.id));
    }

    #[test]
    fn unmatched_insert_uses_view_criteria_include() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let work = category("Work", false);
        store.create_category(&work).unwrap();

        let item = Item::new("task".to_string());
        store.create_item(&item).unwrap();

        let mut current_view = view("My Work");
        current_view
            .criteria
            .set_criterion(CriterionMode::And, work.id);

        agenda
            .insert_item_in_unmatched(item.id, &current_view)
            .unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&work.id));
        assert_eq!(
            assignments.get(&work.id).and_then(|a| a.origin.as_deref()),
            Some("edit:view.insert")
        );
    }

    #[test]
    fn unmatched_insert_applies_subsumption_for_view_include_assignments() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let work = category("Work", false);
        store.create_category(&work).unwrap();
        let project_y = child_category("Project Y", work.id, false);
        store.create_category(&project_y).unwrap();

        let item = Item::new("task".to_string());
        store.create_item(&item).unwrap();

        let mut current_view = view("Project Y Board");
        current_view
            .criteria
            .set_criterion(CriterionMode::And, project_y.id);

        agenda
            .insert_item_in_unmatched(item.id, &current_view)
            .unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&project_y.id));
        assert!(assignments.contains_key(&work.id));
        assert_eq!(
            assignments
                .get(&work.id)
                .map(|assignment| assignment.source),
            Some(AssignmentSource::Subsumption)
        );
    }

    #[test]
    fn unmatched_remove_uses_view_remove_targets() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let work = category("Work", false);
        let personal = category("Personal", false);
        store.create_category(&work).unwrap();
        store.create_category(&personal).unwrap();

        let item = Item::new("task".to_string());
        store.create_item(&item).unwrap();
        store
            .assign_item(item.id, work.id, &manual_assignment("manual:user"))
            .unwrap();
        store
            .assign_item(item.id, personal.id, &manual_assignment("manual:user"))
            .unwrap();

        let mut current_view = view("My Work");
        current_view.remove_from_view_unassign.insert(work.id);

        agenda
            .remove_item_from_unmatched(item.id, &current_view)
            .unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(!assignments.contains_key(&work.id));
        assert!(assignments.contains_key(&personal.id));
    }

    #[test]
    fn insert_item_in_section_is_idempotent_for_existing_assignments() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let work = category("Work", false);
        store.create_category(&work).unwrap();

        let item = Item::new("task".to_string());
        store.create_item(&item).unwrap();
        agenda
            .assign_item_manual(item.id, work.id, Some("manual:user".to_string()))
            .unwrap();

        let mut current_view = view("My Work");
        current_view
            .criteria
            .set_criterion(CriterionMode::And, work.id);
        let mut current_section = section("Work");
        current_section.on_insert_assign.insert(work.id);

        agenda
            .insert_item_in_section(item.id, &current_view, &current_section)
            .unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert_eq!(assignments.len(), 1);
        assert_eq!(
            assignments.get(&work.id).and_then(|a| a.origin.as_deref()),
            Some("manual:user")
        );
    }

    #[test]
    fn remove_from_view_triggers_engine_even_with_no_unassign_targets() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let trigger = category("Trigger", true);
        store.create_category(&trigger).unwrap();

        let item = Item::new("trigger task".to_string());
        store.create_item(&item).unwrap();

        let current_view = view("Any");
        agenda
            .remove_item_from_view(item.id, &current_view)
            .unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&trigger.id));
    }

    #[test]
    fn db_backed_setup_with_items_categories_views_and_assignments_resolves_filters() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let work = category("Work", false);
        agenda.create_category(&work).unwrap();

        let mut project_atlas = child_category("Project Atlas", work.id, true);
        project_atlas.enable_implicit_string = true;
        agenda.create_category(&project_atlas).unwrap();

        let mut miguel = child_category("Miguel", work.id, true);
        miguel.enable_implicit_string = true;
        agenda.create_category(&miguel).unwrap();

        let mut alice = child_category("Alice", work.id, true);
        alice.enable_implicit_string = true;
        agenda.create_category(&alice).unwrap();

        let mut priority = category("Priority", false);
        priority.is_exclusive = true;
        agenda.create_category(&priority).unwrap();
        let high = child_category("High", priority.id, false);
        agenda.create_category(&high).unwrap();

        let collaborative = Item::new(
            "Project Atlas: Miguel and Alice triage defects tomorrow at noon".to_string(),
        );
        agenda.create_item(&collaborative).unwrap();
        agenda
            .assign_item_manual(collaborative.id, high.id, Some("manual:test".to_string()))
            .unwrap();

        let solo = Item::new("Project Atlas: Miguel draft rollout checklist".to_string());
        agenda.create_item(&solo).unwrap();
        agenda
            .assign_item_manual(solo.id, high.id, Some("manual:test".to_string()))
            .unwrap();

        let collaborative_assignments = store.get_assignments_for_item(collaborative.id).unwrap();
        assert!(collaborative_assignments.contains_key(&project_atlas.id));
        assert!(collaborative_assignments.contains_key(&work.id));
        assert!(collaborative_assignments.contains_key(&miguel.id));
        assert!(collaborative_assignments.contains_key(&alice.id));
        assert!(collaborative_assignments.contains_key(&high.id));

        let mut view = view("Miguel Without Alice");
        view.criteria.set_criterion(CriterionMode::And, work.id);
        view.criteria.set_criterion(CriterionMode::And, miguel.id);
        view.criteria.set_criterion(CriterionMode::Not, alice.id);
        store.create_view(&view).unwrap();

        let persisted_view = store.get_view(view.id).unwrap();
        let items = store.list_items().unwrap();
        let categories = store.get_hierarchy().unwrap();
        let result = resolve_view(&persisted_view, &items, &categories, date(2026, 2, 16));

        assert!(result.sections.is_empty());
        let unmatched = result.unmatched.expect("unmatched group is enabled");
        assert_eq!(unmatched.len(), 1);
        assert_eq!(unmatched[0].id, solo.id);
    }

    #[test]
    fn mark_item_done_sets_done_fields_and_assigns_done_category() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let work = category("Work", false);
        agenda.create_category(&work).unwrap();
        let item = Item::new("Ship SLC".to_string());
        agenda.create_item(&item).unwrap();
        agenda
            .assign_item_manual(item.id, work.id, Some("manual:test".to_string()))
            .unwrap();

        let _result = agenda.mark_item_done(item.id).unwrap();
        let loaded = store.get_item(item.id).unwrap();
        assert!(loaded.is_done);
        assert!(loaded.done_date.is_some());

        let done_category_id = store
            .get_hierarchy()
            .unwrap()
            .into_iter()
            .find(|category| {
                category
                    .name
                    .eq_ignore_ascii_case(RESERVED_CATEGORY_NAME_DONE)
            })
            .expect("Done category exists")
            .id;
        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&done_category_id));
        assert_eq!(
            assignments
                .get(&done_category_id)
                .and_then(|assignment| assignment.origin.as_deref()),
            Some("manual:done")
        );
    }

    #[test]
    fn claim_item_workflow_assigns_claim_target_for_ready_item() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let ready = category("Ready", false);
        store.create_category(&ready).unwrap();
        let in_progress = category("In Progress", false);
        store.create_category(&in_progress).unwrap();
        store
            .set_workflow_config(&crate::workflow::WorkflowConfig {
                ready_category_id: Some(ready.id),
                claim_category_id: Some(in_progress.id),
            })
            .unwrap();

        let item = Item::new("Claim me".to_string());
        store.create_item(&item).unwrap();
        agenda
            .assign_item_manual(item.id, ready.id, Some("manual:test".to_string()))
            .unwrap();

        agenda.claim_item_workflow(item.id).unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&in_progress.id));
    }

    #[test]
    fn claim_item_workflow_honors_exclusive_status_parent() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut status = category("Status", false);
        status.is_exclusive = true;
        store.create_category(&status).unwrap();

        let ready = child_category("Ready", status.id, false);
        store.create_category(&ready).unwrap();
        let in_progress = child_category("In Progress", status.id, false);
        store.create_category(&in_progress).unwrap();
        store
            .set_workflow_config(&crate::workflow::WorkflowConfig {
                ready_category_id: Some(ready.id),
                claim_category_id: Some(in_progress.id),
            })
            .unwrap();

        let item = Item::new("Claim me exclusively".to_string());
        store.create_item(&item).unwrap();
        agenda
            .assign_item_manual(item.id, ready.id, Some("manual:test".to_string()))
            .unwrap();

        agenda.claim_item_workflow(item.id).unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&in_progress.id));
        assert!(!assignments.contains_key(&ready.id));
    }

    #[test]
    fn mark_item_done_clears_workflow_claim_assignment() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let ready = category("Ready", false);
        store.create_category(&ready).unwrap();
        let in_progress = category("In Progress", false);
        store.create_category(&in_progress).unwrap();
        let work = category("Work", false);
        store.create_category(&work).unwrap();
        store
            .set_workflow_config(&crate::workflow::WorkflowConfig {
                ready_category_id: Some(ready.id),
                claim_category_id: Some(in_progress.id),
            })
            .unwrap();

        let item = Item::new("Finish me".to_string());
        store.create_item(&item).unwrap();
        agenda
            .assign_item_manual(item.id, ready.id, Some("manual:test".to_string()))
            .unwrap();
        agenda
            .assign_item_manual(item.id, work.id, Some("manual:test".to_string()))
            .unwrap();
        agenda.claim_item_workflow(item.id).unwrap();

        agenda.mark_item_done(item.id).unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(!assignments.contains_key(&in_progress.id));
    }

    #[test]
    fn mark_item_done_rejects_non_actionable_only_items() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut reference = category("Reference", false);
        reference.is_actionable = false;
        agenda.create_category(&reference).unwrap();

        let item = Item::new("Read policy document".to_string());
        agenda.create_item(&item).unwrap();
        agenda
            .assign_item_manual(item.id, reference.id, Some("manual:test".to_string()))
            .unwrap();

        let err = agenda.mark_item_done(item.id).unwrap_err();
        assert!(matches!(err, AgendaError::InvalidOperation { .. }));
    }

    #[test]
    fn toggle_item_done_unsets_done_state_and_done_assignment() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let work = category("Work", false);
        agenda.create_category(&work).unwrap();

        let item = Item::new("Ship SLC".to_string());
        agenda.create_item(&item).unwrap();
        agenda
            .assign_item_manual(item.id, work.id, Some("manual:test".to_string()))
            .unwrap();

        agenda.toggle_item_done(item.id).unwrap();
        assert!(store.get_item(item.id).unwrap().is_done);

        agenda.toggle_item_done(item.id).unwrap();
        let loaded = store.get_item(item.id).unwrap();
        assert!(!loaded.is_done);
        assert!(loaded.done_date.is_none());

        let done_category_id = store
            .get_hierarchy()
            .unwrap()
            .into_iter()
            .find(|category| {
                category
                    .name
                    .eq_ignore_ascii_case(RESERVED_CATEGORY_NAME_DONE)
            })
            .expect("Done category exists")
            .id;
        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(!assignments.contains_key(&done_category_id));
    }

    #[test]
    fn move_category_to_parent_reparents_category() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let left = category("Left", false);
        let right = category("Right", false);
        agenda.create_category(&left).unwrap();
        agenda.create_category(&right).unwrap();

        let child = child_category("Child", left.id, false);
        agenda.create_category(&child).unwrap();

        let result = agenda
            .move_category_to_parent(child.id, Some(right.id), None)
            .unwrap();
        assert!(result.processed_items >= result.affected_items);
        assert_eq!(store.get_category(child.id).unwrap().parent, Some(right.id));
    }

    #[test]
    fn move_category_within_parent_reorders_children() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let parent = category("Parent", false);
        agenda.create_category(&parent).unwrap();
        let alpha = child_category("Alpha", parent.id, false);
        let beta = child_category("Beta", parent.id, false);
        agenda.create_category(&alpha).unwrap();
        agenda.create_category(&beta).unwrap();

        agenda.move_category_within_parent(beta.id, -1).unwrap();

        let loaded_parent = store.get_category(parent.id).unwrap();
        assert_eq!(loaded_parent.children, vec![beta.id, alpha.id]);
    }

    #[test]
    fn link_items_depends_on_rejects_self_link() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);
        let item_id = make_item(&store, "A");

        let err = agenda.link_items_depends_on(item_id, item_id).unwrap_err();
        assert!(matches!(err, AgendaError::InvalidOperation { .. }));
    }

    #[test]
    fn link_items_related_rejects_self_link() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);
        let item_id = make_item(&store, "A");

        let err = agenda.link_items_related(item_id, item_id).unwrap_err();
        assert!(matches!(err, AgendaError::InvalidOperation { .. }));
    }

    #[test]
    fn link_items_blocks_stores_inverse_depends_on_edge() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);
        let blocker = make_item(&store, "Blocker");
        let blocked = make_item(&store, "Blocked");

        let result = agenda.link_items_blocks(blocker, blocked).unwrap();
        assert!(result.created);
        assert!(store
            .item_link_exists(blocked, blocker, ItemLinkKind::DependsOn)
            .unwrap());
        assert_eq!(
            agenda.immediate_dependent_ids(blocker).unwrap(),
            vec![blocked]
        );
        assert_eq!(agenda.immediate_prereq_ids(blocked).unwrap(), vec![blocker]);
    }

    #[test]
    fn link_items_related_normalizes_pair_and_is_idempotent() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);
        let a = make_item(&store, "A");
        let b = make_item(&store, "B");
        let (low, high) = if a.to_string() < b.to_string() {
            (a, b)
        } else {
            (b, a)
        };

        let first = agenda.link_items_related(high, low).unwrap();
        let second = agenda.link_items_related(low, high).unwrap();

        assert!(first.created);
        assert!(!second.created);
        assert!(store
            .item_link_exists(low, high, ItemLinkKind::Related)
            .unwrap());

        let count: i64 = store
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM item_links WHERE kind = 'related'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn link_items_depends_on_rejects_direct_cycle() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);
        let a = make_item(&store, "A");
        let b = make_item(&store, "B");

        agenda.link_items_depends_on(a, b).unwrap();
        let err = agenda.link_items_depends_on(b, a).unwrap_err();
        assert!(matches!(err, AgendaError::InvalidOperation { .. }));
    }

    #[test]
    fn link_items_depends_on_rejects_longer_cycle() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);
        let a = make_item(&store, "A");
        let b = make_item(&store, "B");
        let c = make_item(&store, "C");

        agenda.link_items_depends_on(a, b).unwrap();
        agenda.link_items_depends_on(b, c).unwrap();
        let err = agenda.link_items_depends_on(c, a).unwrap_err();
        assert!(matches!(err, AgendaError::InvalidOperation { .. }));
    }

    #[test]
    fn link_items_blocks_rejects_direct_cycle() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);
        let a = make_item(&store, "A");
        let b = make_item(&store, "B");

        agenda.link_items_blocks(a, b).unwrap();
        let err = agenda.link_items_blocks(b, a).unwrap_err();
        assert!(matches!(err, AgendaError::InvalidOperation { .. }));
    }

    #[test]
    fn link_items_blocks_rejects_longer_cycle() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);
        let a = make_item(&store, "A");
        let b = make_item(&store, "B");
        let c = make_item(&store, "C");

        agenda.link_items_blocks(a, b).unwrap();
        agenda.link_items_blocks(b, c).unwrap();
        let err = agenda.link_items_blocks(c, a).unwrap_err();
        assert!(matches!(err, AgendaError::InvalidOperation { .. }));
    }

    #[test]
    fn link_items_related_allows_triangle() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);
        let a = make_item(&store, "A");
        let b = make_item(&store, "B");
        let c = make_item(&store, "C");

        assert!(agenda.link_items_related(a, b).unwrap().created);
        assert!(agenda.link_items_related(b, c).unwrap().created);
        assert!(agenda.link_items_related(c, a).unwrap().created);

        let links_a = agenda.immediate_related_ids(a).unwrap();
        let links_b = agenda.immediate_related_ids(b).unwrap();
        let links_c = agenda.immediate_related_ids(c).unwrap();
        assert_eq!(links_a.len(), 2);
        assert_eq!(links_b.len(), 2);
        assert_eq!(links_c.len(), 2);
    }

    #[test]
    fn unlink_items_blocks_and_related_are_idempotent() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);
        let a = make_item(&store, "A");
        let b = make_item(&store, "B");

        agenda.link_items_blocks(a, b).unwrap();
        agenda.link_items_related(a, b).unwrap();

        agenda.unlink_items_blocks(a, b).unwrap();
        agenda.unlink_items_related(a, b).unwrap();
        // idempotent delete behavior delegated to Store
        agenda.unlink_items_blocks(a, b).unwrap();
        agenda.unlink_items_related(a, b).unwrap();

        assert!(agenda.immediate_dependent_ids(a).unwrap().is_empty());
        assert!(agenda.immediate_prereq_ids(b).unwrap().is_empty());
        assert!(agenda.immediate_related_ids(a).unwrap().is_empty());
        assert!(agenda.immediate_related_ids(b).unwrap().is_empty());
    }

    #[test]
    fn immediate_links_for_item_groups_prereqs_blocks_and_related() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);
        let a = make_item(&store, "A");
        let b = make_item(&store, "B");
        let c = make_item(&store, "C");
        let d = make_item(&store, "D");

        agenda.link_items_depends_on(a, b).unwrap();
        agenda.link_items_blocks(a, c).unwrap(); // c depends-on a
        agenda.link_items_related(a, d).unwrap();

        let links = agenda.immediate_links_for_item(a).unwrap();
        assert_eq!(links.depends_on, vec![b]);
        assert_eq!(links.blocks, vec![c]);
        assert_eq!(links.related, vec![d]);
    }

    // ── normalize_related_pair ─────────────────────────────────────────────────

    #[test]
    fn normalize_related_pair_returns_lexicographic_order() {
        use uuid::Uuid;
        let a: ItemId = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
        let b: ItemId = Uuid::parse_str("ffffffff-ffff-ffff-ffff-ffffffffffff").unwrap();

        // a < b lexicographically, so (a, b) should be unchanged.
        let (lo, hi) = Agenda::normalize_related_pair(a, b);
        assert_eq!(lo, a);
        assert_eq!(hi, b);

        // Reversed input should also produce (a, b).
        let (lo2, hi2) = Agenda::normalize_related_pair(b, a);
        assert_eq!(lo2, a);
        assert_eq!(hi2, b);
    }

    #[test]
    fn normalize_related_pair_is_idempotent() {
        use uuid::Uuid;
        let a: ItemId = Uuid::parse_str("aaaaaaaa-0000-0000-0000-000000000000").unwrap();
        let b: ItemId = Uuid::parse_str("bbbbbbbb-0000-0000-0000-000000000000").unwrap();

        let first = Agenda::normalize_related_pair(a, b);
        let second = Agenda::normalize_related_pair(first.0, first.1);
        assert_eq!(first, second);
    }

    // ── ensure_not_self_link ───────────────────────────────────────────────────

    #[test]
    fn ensure_not_self_link_rejects_identical_ids() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);
        let id = make_item(&store, "Task");

        let result = agenda.ensure_not_self_link(id, id, "depends-on");
        assert!(result.is_err(), "self-link should be rejected");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("self-link"),
            "error message should mention self-link, got: {msg}"
        );
    }

    #[test]
    fn ensure_not_self_link_accepts_distinct_ids() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);
        let a = make_item(&store, "Task A");
        let b = make_item(&store, "Task B");

        assert!(
            agenda.ensure_not_self_link(a, b, "depends-on").is_ok(),
            "distinct ids should be accepted"
        );
    }

    // ── ensure_depends_on_no_cycle ─────────────────────────────────────────────

    #[test]
    fn ensure_depends_on_no_cycle_detects_direct_cycle() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);
        let a = make_item(&store, "A");
        let b = make_item(&store, "B");

        // A depends-on B
        agenda.link_items_depends_on(a, b).unwrap();

        // Trying to make B depend-on A would create A→B→A cycle.
        let result = agenda.ensure_depends_on_no_cycle(b, a);
        assert!(result.is_err(), "direct cycle A→B→A should be detected");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("cycle"),
            "error should mention cycle, got: {msg}"
        );
    }

    #[test]
    fn ensure_depends_on_no_cycle_detects_transitive_cycle() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);
        let a = make_item(&store, "A");
        let b = make_item(&store, "B");
        let c = make_item(&store, "C");

        // A→B, B→C
        agenda.link_items_depends_on(a, b).unwrap();
        agenda.link_items_depends_on(b, c).unwrap();

        // Trying to make C depend-on A would create A→B→C→A cycle.
        let result = agenda.ensure_depends_on_no_cycle(c, a);
        assert!(
            result.is_err(),
            "transitive cycle A→B→C→A should be detected"
        );
    }

    #[test]
    fn ensure_depends_on_no_cycle_allows_non_cyclic_dependency() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);
        let a = make_item(&store, "A");
        let b = make_item(&store, "B");
        let c = make_item(&store, "C");

        agenda.link_items_depends_on(a, b).unwrap();

        // C→A is fine; there's no path from A back to C.
        assert!(
            agenda.ensure_depends_on_no_cycle(c, a).is_ok(),
            "non-cyclic dependency should be accepted"
        );
    }

    // ── preview_section_move ──────────────────────────────────────────────────

    /// Build a simple two-section view used across preview tests.
    ///
    /// View criteria: requires `view_cat`.
    /// Section A criteria: requires `cat_a`.
    /// Section B criteria: requires `cat_b`, on_insert_assign: `extra`.
    fn preview_test_setup() -> (
        View,
        Section,
        Section,
        CategoryId,
        CategoryId,
        CategoryId,
        CategoryId,
    ) {
        let view_cat_id = CategoryId::new_v4();
        let cat_a_id = CategoryId::new_v4();
        let cat_b_id = CategoryId::new_v4();
        let extra_id = CategoryId::new_v4();

        let mut v = view("Board");
        v.criteria.set_criterion(CriterionMode::And, view_cat_id);

        let mut sec_a = section("A");
        sec_a.criteria.set_criterion(CriterionMode::And, cat_a_id);

        let mut sec_b = section("B");
        sec_b.criteria.set_criterion(CriterionMode::And, cat_b_id);
        sec_b.on_insert_assign.insert(extra_id);

        (v, sec_a, sec_b, view_cat_id, cat_a_id, cat_b_id, extra_id)
    }

    #[test]
    fn preview_section_move_none_to_none_is_empty() {
        let v = view("Empty");
        let preview = Agenda::preview_section_move(&v, None, None);
        assert!(preview.to_assign.is_empty());
        assert!(preview.to_unassign.is_empty());
    }

    #[test]
    fn preview_section_move_none_to_section_assigns_insert_targets() {
        let (v, sec_a, _, view_cat_id, cat_a_id, _, _) = preview_test_setup();

        let preview = Agenda::preview_section_move(&v, None, Some(&sec_a));

        // Moving from unmatched → section A should assign section A's criteria
        // (cat_a) plus the view's criteria (view_cat).
        assert!(preview.to_assign.contains(&cat_a_id), "should assign cat_a");
        assert!(
            preview.to_assign.contains(&view_cat_id),
            "should assign view_cat"
        );
        assert!(preview.to_unassign.is_empty(), "nothing to unassign");
    }

    #[test]
    fn preview_section_move_section_to_none_unassigns_remove_targets() {
        let (v, sec_a, _, view_cat_id, cat_a_id, _, _) = preview_test_setup();

        let preview = Agenda::preview_section_move(&v, Some(&sec_a), None);

        // section_remove_targets for sec_a: structural targets (cat_a) minus
        // view criteria (view_cat) — so only cat_a is unassigned.
        assert!(
            preview.to_unassign.contains(&cat_a_id),
            "should unassign cat_a"
        );
        // view_cat is preserved because it belongs to the view criteria.
        assert!(
            !preview.to_unassign.contains(&view_cat_id),
            "view_cat should be preserved"
        );
        assert!(preview.to_assign.is_empty(), "nothing to assign");
    }

    #[test]
    fn preview_section_move_between_sections_net_change() {
        let (v, sec_a, sec_b, view_cat_id, cat_a_id, cat_b_id, extra_id) = preview_test_setup();

        let preview = Agenda::preview_section_move(&v, Some(&sec_a), Some(&sec_b));

        // Moving A → B:
        //   to_assign  = section_insert_targets(view, sec_b) = {cat_b, extra, view_cat}
        //   to_unassign = section_structural_targets(sec_a)  = {cat_a}
        //                 minus preserve (insert targets of B, which doesn't include cat_a)
        //                 → {cat_a}
        // view_cat is NOT in to_unassign (structural targets only covers the source
        // section's own categories, not the view's), so there is no cancellation.
        assert!(preview.to_assign.contains(&cat_b_id), "should assign cat_b");
        assert!(
            preview.to_assign.contains(&extra_id),
            "should assign on_insert_assign extra"
        );
        assert!(
            preview.to_assign.contains(&view_cat_id),
            "view_cat included in insert targets"
        );
        assert!(
            preview.to_unassign.contains(&cat_a_id),
            "should unassign cat_a"
        );
        assert!(
            !preview.to_unassign.contains(&view_cat_id),
            "view_cat should not be in to_unassign"
        );
    }

    #[test]
    fn preview_section_move_same_section_to_unassign_is_empty() {
        let (v, sec_a, _, view_cat_id, cat_a_id, _, _) = preview_test_setup();

        // Moving to the same section: the preserve set (= insert targets of the
        // target) covers all of the structural targets of the source, so nothing
        // is unassigned.  The insert targets are still returned in to_assign
        // (they would be re-applied, which is a no-op when already assigned).
        let preview = Agenda::preview_section_move(&v, Some(&sec_a), Some(&sec_a));
        assert!(
            preview.to_unassign.is_empty(),
            "to_unassign should be empty when target is same section"
        );
        assert!(
            preview.to_assign.contains(&cat_a_id),
            "cat_a appears in to_assign (re-apply is safe)"
        );
        assert!(
            preview.to_assign.contains(&view_cat_id),
            "view_cat appears in to_assign"
        );
    }

    #[test]
    fn preview_section_move_on_remove_unassign_included() {
        let v = view("Board");
        let extra_remove_id = CategoryId::new_v4();

        let mut sec = section("WithExtra");
        sec.on_remove_unassign.insert(extra_remove_id);

        let preview = Agenda::preview_section_move(&v, Some(&sec), None);

        assert!(
            preview.to_unassign.contains(&extra_remove_id),
            "on_remove_unassign should appear in to_unassign"
        );
    }
}
