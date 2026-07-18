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
    ReqwestOpenRouterTransport, SemanticClassificationMode, SemanticProviderKind, SuggestionStatus,
    WhenParserProvider, CLASSIFICATION_DEBUG_LOG_PATH, PROVIDER_ID_IMPLICIT_STRING,
    PROVIDER_ID_OLLAMA_OPENAI_COMPAT, PROVIDER_ID_OPENAI, PROVIDER_ID_OPENROUTER,
    PROVIDER_ID_WHEN_PARSER,
};
use crate::date_rules::{category_uses_date_conditions, EvaluationContext};
use crate::dates::BasicDateParser;
use crate::engine::{
    evaluate_all_items_with_options, process_item_with_options, reevaluate_all_items_with_options,
    AssignmentIntent, EvaluateAllItemsResult, ProcessItemResult, ProcessOptions,
};
use crate::error::{AgletError, Result};
use crate::matcher::Classifier;
use crate::model::{
    origin as origin_const, Action, Assignment, AssignmentActionKind, AssignmentEvent,
    AssignmentEventKind, AssignmentExplanation, AssignmentSource, Category, CategoryId,
    CategoryValueKind, Condition, Item, ItemId, ItemLink, ItemLinkKind, ItemLinksForItem,
    RecurrenceRule, Section, View, RESERVED_CATEGORY_NAME_DONE, RESERVED_CATEGORY_NAME_ENTRY,
    RESERVED_CATEGORY_NAME_WHEN,
};
use crate::store::Store;
use crate::workflow::{
    claimability_for_item, resolve_workflow_config, workflow_setup_error_message, Claimability,
    ResolvedWorkflowConfig,
};

/// Synchronous integration layer that wires Store mutations to engine execution.
pub struct Aglet<'a> {
    store: &'a Store,
    classifier: &'a dyn Classifier,
    date_parser: BasicDateParser,
    ollama_transport: Arc<dyn OllamaTransport>,
    openrouter_transport: Arc<dyn OpenRouterTransport>,
    openai_transport: Arc<dyn OpenAiTransport>,
    debug: bool,
    /// Recursion depth for applying special action effects (SetWhen/MarkDone/
    /// Delete): each application reprocesses the item, which may produce more
    /// specials. Capped so cascades of specials terminate.
    specials_depth: std::cell::Cell<u8>,
}

/// The net category changes that would result from a section-move operation,
/// as computed by [`Aglet::preview_section_move`].
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

enum SemanticCandidateAvailability {
    AlreadyAssigned,
    Unavailable,
    Available,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct ReviewQueueResult {
    queued: usize,
    semantic_queued: usize,
    semantic_skipped_already_assigned: usize,
    semantic_skipped_unavailable: usize,
}

struct ReviewQueueChoice {
    suggestion: ClassificationSuggestion,
    exclusive_parent: Option<CategoryId>,
    exclusive_child_index: usize,
    is_semantic: bool,
}

impl<'a> Aglet<'a> {
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
        let mut aglet = Self::new(store, classifier);
        aglet.debug = debug;
        aglet
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
            specials_depth: std::cell::Cell::new(0),
        }
    }

    pub fn store(&self) -> &Store {
        self.store
    }

    pub fn has_date_conditions(&self) -> Result<bool> {
        Ok(self
            .store
            .get_hierarchy()?
            .iter()
            .any(|category| category_uses_date_conditions(&category.conditions)))
    }

    pub fn reevaluate_temporal_conditions(&self) -> Result<EvaluateAllItemsResult> {
        self.reevaluate_temporal_conditions_with_context(EvaluationContext::now())
    }

    pub fn reevaluate_temporal_conditions_with_context(
        &self,
        evaluation_context: EvaluationContext,
    ) -> Result<EvaluateAllItemsResult> {
        let mut result = reevaluate_all_items_with_options(
            self.store,
            self.classifier,
            ProcessOptions {
                enable_implicit_string: false,
                evaluation_context: evaluation_context.clone(),
                ..ProcessOptions::default()
            },
        )?;
        self.apply_bulk_deferred_specials(&mut result, &evaluation_context)?;
        Ok(result)
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

    pub fn set_item_recurrence_rule(
        &self,
        item_id: ItemId,
        rule: Option<RecurrenceRule>,
    ) -> Result<()> {
        let mut item = self.store.get_item(item_id)?;
        item.recurrence_rule = rule;
        item.modified_at = Timestamp::now();
        self.store.update_item(&item)
    }

    pub fn create_category(&self, category: &Category) -> Result<EvaluateAllItemsResult> {
        self.store.create_category(category)?;
        self.process_category_change(category.id)
    }

    pub fn update_category(&self, category: &Category) -> Result<EvaluateAllItemsResult> {
        self.store.update_category(category)?;
        self.process_category_change(category.id)
    }

    pub fn add_category_action(
        &self,
        category_id: CategoryId,
        action: Action,
    ) -> Result<(usize, EvaluateAllItemsResult)> {
        let mut category = self.store.get_category(category_id)?;
        self.validate_category_action(&category, &action)?;
        category.actions.push(action);
        let action_index = category.actions.len() - 1;
        let result = self.update_category(&category)?;
        Ok((action_index, result))
    }

    pub fn update_category_action(
        &self,
        category_id: CategoryId,
        action_index: usize,
        action: Action,
    ) -> Result<EvaluateAllItemsResult> {
        let mut category = self.store.get_category(category_id)?;
        if action_index >= category.actions.len() {
            return Err(AgletError::InvalidOperation {
                message: format!(
                    "action index {} out of range for category '{}' (has {} action(s))",
                    action_index + 1,
                    category.name,
                    category.actions.len()
                ),
            });
        }
        self.validate_category_action(&category, &action)?;
        category.actions[action_index] = action;
        self.update_category(&category)
    }

    pub fn remove_category_action(
        &self,
        category_id: CategoryId,
        action_index: usize,
    ) -> Result<(Action, EvaluateAllItemsResult)> {
        let mut category = self.store.get_category(category_id)?;
        if action_index >= category.actions.len() {
            return Err(AgletError::InvalidOperation {
                message: format!(
                    "action index {} out of range for category '{}' (has {} action(s))",
                    action_index + 1,
                    category.name,
                    category.actions.len()
                ),
            });
        }
        let removed = category.actions.remove(action_index);
        let result = self.update_category(&category)?;
        Ok((removed, result))
    }

    fn validate_category_action(&self, category: &Category, action: &Action) -> Result<()> {
        if let Some(targets) = action.category_targets() {
            if targets.is_empty() {
                return Err(AgletError::InvalidOperation {
                    message: "category actions must target at least one category".to_string(),
                });
            }
            if targets.contains(&category.id) {
                return Err(AgletError::InvalidOperation {
                    message: format!(
                        "category '{}' cannot target itself in an action",
                        category.name
                    ),
                });
            }
        }
        match action {
            Action::AssignNumeric { target, .. } => {
                if *target == category.id {
                    return Err(AgletError::InvalidOperation {
                        message: format!(
                            "category '{}' cannot target itself in an action",
                            category.name
                        ),
                    });
                }
                let target_category = self.store.get_category(*target)?;
                if target_category.value_kind != CategoryValueKind::Numeric {
                    return Err(AgletError::InvalidOperation {
                        message: format!(
                            "numeric action target '{}' is not a Numeric category",
                            target_category.name
                        ),
                    });
                }
            }
            Action::Delete => {
                if !category.allow_delete_action {
                    return Err(AgletError::InvalidOperation {
                        message: format!(
                            "category '{}' must have allow_delete_action enabled before a Delete action can be attached",
                            category.name
                        ),
                    });
                }
            }
            Action::Assign { .. }
            | Action::Remove { .. }
            | Action::SetWhen { .. }
            | Action::MarkDone => {}
        }
        Ok(())
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

    /// Build the intent for a sticky Manual assignment; the engine executes
    /// it (veto clear, exclusivity, subsumption, action firing) as the single
    /// assignment write path.
    ///
    /// `upgrade_existing` selects the two manual-write semantics: direct
    /// assignment upserts (re-assigning an auto-matched category upgrades it
    /// to sticky Manual, numeric assignment updates the value); edit-through
    /// paths skip categories that are already assigned so a section insert
    /// never rewrites existing provenance.
    fn manual_intent(
        category_id: CategoryId,
        origin: Option<String>,
        default_origin: &str,
        numeric_value: Option<Decimal>,
        upgrade_existing: bool,
    ) -> AssignmentIntent {
        AssignmentIntent {
            category_id,
            source: AssignmentSource::Manual,
            origin: Some(
                origin
                    .clone()
                    .unwrap_or_else(|| default_origin.to_string()),
            ),
            explanation: Some(AssignmentExplanation::Manual { origin }),
            numeric_value,
            upgrade_existing,
            clears_veto: true,
            override_exclusive: true,
        }
    }

    pub fn assign_item_manual(
        &self,
        item_id: ItemId,
        category_id: CategoryId,
        origin: Option<String>,
    ) -> Result<ProcessItemResult> {
        let intent =
            Self::manual_intent(category_id, origin, origin_const::MANUAL, None, true);
        let result = self.reprocess_existing_item_with_intents(item_id, vec![intent])?;
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
                    return Err(AgletError::InvalidOperation {
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
            return Err(AgletError::InvalidOperation {
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
                outcome => Err(AgletError::InvalidOperation {
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
            return Err(AgletError::InvalidOperation {
                message: workflow_setup_error_message().to_string(),
            });
        };

        let item = self.store.get_item(item_id)?;
        if !item.assignments.contains_key(&workflow.claim_category_id) {
            return Err(AgletError::InvalidOperation {
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
            return Err(AgletError::InvalidOperation {
                message: format!("category '{}' is not Numeric", category.name),
            });
        }

        let intent = Self::manual_intent(
            category_id,
            origin,
            origin_const::MANUAL_NUMERIC,
            Some(numeric_value),
            true,
        );
        let result = self.reprocess_existing_item_with_intents(item_id, vec![intent])?;
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
            return Err(AgletError::InvalidOperation {
                message: format!(
                    "cannot remove category '{ancestor_name}' while descendant '{descendant_name}' is assigned; remove descendant first"
                ),
            });
        }
        // Manually removing a machine-made assignment is Agenda's negative
        // assignment (`-`): record a veto so the engine never re-assigns it.
        // Removing one's own manual assignment stays a plain removal.
        let removed_source = self
            .store
            .get_assignments_for_item(item_id)?
            .get(&category_id)
            .map(|assignment| assignment.source);
        if removed_source.is_some_and(|source| source != AssignmentSource::Manual) {
            self.store
                .add_assignment_veto(item_id, category_id, Some("manual:unassign"))?;
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

        let intents = Self::manual_category_intents(&targets, "edit:section.insert");
        self.reprocess_existing_item_with_intents(item_id, intents)
    }

    pub fn insert_item_in_unmatched(
        &self,
        item_id: ItemId,
        view: &View,
    ) -> Result<ProcessItemResult> {
        let view_include: HashSet<CategoryId> = view.criteria.and_category_ids().collect();
        let intents = Self::manual_category_intents(&view_include, "edit:view.insert");
        self.reprocess_existing_item_with_intents(item_id, intents)
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
        let intents = Self::manual_category_intents(&to_assign, "edit:section.move");
        self.reprocess_existing_item_with_intents(item_id, intents)
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

        let preview_aglet = Aglet::new(&preview_store, self.classifier);
        let preview_category_id =
            Self::mapped_category_id(category_id, &id_map, "preview toggle category")?;

        if source_item.assignments.contains_key(&category_id) {
            preview_aglet.unassign_item_manual(item_id, preview_category_id)?;
        } else {
            preview_aglet.assign_item_manual(
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
            return Err(AgletError::InvalidOperation {
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
        let intent = Self::manual_intent(
            done_category_id,
            Some(origin_const::MANUAL_DONE.to_string()),
            origin_const::MANUAL_DONE,
            None,
            true,
        );
        self.clear_claim_assignment_if_configured(item_id)?;
        let mut result = self.reprocess_existing_item_with_intents(item_id, vec![intent])?;

        // Succession: generate next instance for recurring items
        if let Some(ref rule) = item.recurrence_rule {
            let successor_id = self.generate_recurrence_successor(&item, rule, done_at)?;
            result.successor_item_id = Some(successor_id);
        }

        Ok(result)
    }

    /// Generate the next recurrence instance from a completed item.
    ///
    /// The successor gets the same text, note, recurrence rule, and sticky non-reserved
    /// assignments. Its `when_date` is advanced per the rule. The completed and successor
    /// items share the same `recurrence_series_id`.
    fn generate_recurrence_successor(
        &self,
        completed: &Item,
        rule: &RecurrenceRule,
        done_at: jiff::civil::DateTime,
    ) -> Result<ItemId> {
        // Double-succession guard: skip if a successor already exists
        if self.store.has_recurrence_successor(completed.id)? {
            return Err(AgletError::InvalidOperation {
                message: "recurrence successor already exists".to_string(),
            });
        }

        let now = Timestamp::now();
        let anchor = completed.when_date.unwrap_or(done_at);
        let next_when = rule.next_date(anchor);

        // Ensure series_id exists; create and backfill if needed
        let series_id = match completed.recurrence_series_id {
            Some(id) => id,
            None => {
                let new_series_id = Uuid::new_v4();
                // Backfill series_id on the completed item
                let mut updated = completed.clone();
                updated.recurrence_series_id = Some(new_series_id);
                updated.modified_at = now;
                self.store.update_item(&updated)?;
                new_series_id
            }
        };

        let successor = Item {
            id: Uuid::new_v4(),
            text: completed.text.clone(),
            note: completed.note.clone(),
            created_at: now,
            modified_at: now,
            when_date: Some(next_when),
            done_date: None,
            is_done: false,
            assignments: HashMap::new(),
            recurrence_rule: Some(rule.clone()),
            recurrence_series_id: Some(series_id),
            recurrence_parent_item_id: Some(completed.id),
        };

        self.store.create_item(&successor)?;
        self.sync_when_assignment(successor.id, successor.when_date, None)?;

        // Copy sticky non-reserved assignments from the completed item
        let done_cat = self.done_category_id()?;
        let when_cat = self.category_id_by_name(RESERVED_CATEGORY_NAME_WHEN)?;
        let entry_cat = self.category_id_by_name(RESERVED_CATEGORY_NAME_ENTRY)?;
        let reserved = [done_cat, when_cat, entry_cat];

        for (cat_id, assignment) in &completed.assignments {
            if reserved.contains(cat_id) {
                continue;
            }
            // Only carry sticky assignments (Manual, SuggestionAccepted, Action)
            if !assignment.sticky {
                continue;
            }
            let carry = Assignment {
                source: assignment.source,
                assigned_at: now,
                sticky: true,
                origin: Some(origin_const::RECURRENCE_CARRY.to_string()),
                explanation: assignment.explanation.clone(),
                numeric_value: assignment.numeric_value,
            };
            self.store.assign_item(successor.id, *cat_id, &carry)?;
        }

        // Trigger engine reprocessing on the successor
        let reference_date = jiff::Zoned::now().date();
        self.process_item_save(successor.id, reference_date, true)?;

        Ok(successor.id)
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
            .ok_or(AgletError::NotFound {
                entity: "ClassificationSuggestion",
                id: suggestion_id,
            })?;

        let (mut result, intent) = self.apply_suggestion_assignment(&suggestion)?;
        self.store
            .set_suggestion_status(suggestion_id, SuggestionStatus::Accepted)?;
        let triggers = result.new_assignments.clone();
        merge_process_results(
            &mut result,
            self.reprocess_with_options(
                suggestion.item_id,
                false,
                EvaluationContext::now(),
                triggers,
                intent.into_iter().collect(),
            )?,
        );
        self.debug_log_process_result("suggestion.accept", suggestion.item_id, &result);
        Ok(result)
    }

    pub fn reject_classification_suggestion(&self, suggestion_id: Uuid) -> Result<()> {
        // Rejection is the user saying "no" to this category for this item;
        // record the veto so no machine path (including literal auto-apply)
        // re-assigns it. Accepting a suggestion or assigning manually clears
        // the veto again (product decision #46).
        if let Some(suggestion) = self.store.get_classification_suggestion(suggestion_id)? {
            if let crate::classification::CandidateAssignment::Category(category_id) =
                suggestion.assignment
            {
                self.store.add_assignment_veto(
                    suggestion.item_id,
                    category_id,
                    Some("suggestion:rejected"),
                )?;
            }
        }
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
        let mut intents: Vec<AssignmentIntent> = Vec::new();
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
                let (_, intent) = self.apply_auto_classification_candidate(item_id, candidate)?;
                intents.extend(intent);
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
                    let (_, intent) =
                        self.apply_auto_classification_candidate(item_id, candidate)?;
                    intents.extend(intent);
                }
                CandidateDisposition::QueueReview => {}
            }
        }
        if !intents.is_empty() {
            // On-demand/background classification previously wrote rows with
            // no follow-up run, so auto-applied assignments never fired their
            // actions here; routing intents through the engine closes that.
            self.reprocess_existing_item_with_intents(item_id, intents)?;
        }

        queued += self
            .queue_review_candidates(item_id, item_revision_hash, candidates, &cfg)?
            .queued;

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
        let evaluation_context = EvaluationContext::for_date(reference_date);
        let mut result = ProcessItemResult::default();
        let cfg = self.store.get_classification_config()?;
        if !cfg.should_run_continuously() || !cfg.run_on_item_save {
            let result = self.reprocess_with_options(
                item_id,
                false,
                evaluation_context,
                HashSet::new(),
                Vec::new(),
            )?;
            self.debug_log_process_result("item.process", item_id, &result);
            return Ok(result);
        }

        let service = if include_semantic {
            self.classification_service(reference_date, &cfg, text_changed)
        } else {
            self.classification_service_cheap(reference_date, &cfg, text_changed)
        };
        if !service.has_providers() {
            let result = self.reprocess_with_options(
                item_id,
                false,
                evaluation_context,
                HashSet::new(),
                Vec::new(),
            )?;
            self.debug_log_process_result("item.process", item_id, &result);
            return Ok(result);
        }

        let (_item, item_revision_hash, candidates, debug_summaries) =
            service.collect_candidates(item_id)?;
        result.semantic_debug_messages.extend(debug_summaries);
        self.store
            .supersede_suggestions_for_item_revision(item_id, &item_revision_hash)?;

        let mut intents: Vec<AssignmentIntent> = Vec::new();
        for candidate in &candidates {
            let is_semantic = Self::is_semantic_provider(&candidate.provider);
            if is_semantic {
                result.semantic_candidates_seen += 1;
            }
            if matches!(
                candidate.assignment,
                crate::classification::CandidateAssignment::When(_)
            ) {
                let suggestion = ClassificationSuggestion::from_candidate(
                    candidate,
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
                let (when_result, intent) =
                    self.apply_auto_classification_candidate(item_id, candidate)?;
                merge_process_results(&mut result, when_result);
                intents.extend(intent);
                continue;
            }

            match self.candidate_status_for_config(&cfg, candidate) {
                CandidateDisposition::Skip => {}
                CandidateDisposition::AutoApply => {
                    let suggestion = ClassificationSuggestion::from_candidate(
                        candidate,
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
                    let (when_result, intent) =
                        self.apply_auto_classification_candidate(item_id, candidate)?;
                    merge_process_results(&mut result, when_result);
                    intents.extend(intent);
                }
                CandidateDisposition::QueueReview => {}
            }
        }

        // Apply the collected intents (and fire When triggers) before queueing
        // review candidates, so the already-assigned check sees the final
        // assignment state.
        let triggers = result.new_assignments.clone();
        merge_process_results(
            &mut result,
            self.reprocess_with_options(
                item_id,
                false,
                evaluation_context,
                triggers,
                intents,
            )?,
        );

        let review_result =
            self.queue_review_candidates(item_id, &item_revision_hash, &candidates, &cfg)?;
        result.semantic_candidates_queued_review += review_result.semantic_queued;
        result.semantic_candidates_skipped_already_assigned +=
            review_result.semantic_skipped_already_assigned;
        result.semantic_candidates_skipped_unavailable +=
            review_result.semantic_skipped_unavailable;
        self.debug_log_process_result("item.process", item_id, &result);
        Ok(result)
    }

    fn queue_review_candidates(
        &self,
        item_id: ItemId,
        item_revision_hash: &str,
        candidates: &[ClassificationCandidate],
        cfg: &ClassificationConfig,
    ) -> Result<ReviewQueueResult> {
        let mut result = ReviewQueueResult::default();
        let mut choices = Vec::new();
        let mut winning_choice_by_exclusive_parent: HashMap<CategoryId, usize> = HashMap::new();
        let vetoes = self.store.get_vetoes_for_item(item_id)?;

        for candidate in candidates {
            if matches!(
                candidate.assignment,
                crate::classification::CandidateAssignment::When(_)
            ) || self.candidate_status_for_config(cfg, candidate)
                != CandidateDisposition::QueueReview
            {
                continue;
            }
            if let crate::classification::CandidateAssignment::Category(category_id) =
                candidate.assignment
            {
                if vetoes.contains(&category_id) {
                    continue;
                }
            }

            let is_semantic = Self::is_semantic_provider(&candidate.provider);
            match self.semantic_candidate_availability(item_id, candidate)? {
                SemanticCandidateAvailability::AlreadyAssigned => {
                    if is_semantic {
                        result.semantic_skipped_already_assigned += 1;
                    }
                    continue;
                }
                SemanticCandidateAvailability::Unavailable => {
                    if is_semantic {
                        result.semantic_skipped_unavailable += 1;
                    }
                    continue;
                }
                SemanticCandidateAvailability::Available => {}
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

            let (exclusive_parent, exclusive_child_index) = match candidate.assignment {
                crate::classification::CandidateAssignment::Category(category_id) => self
                    .exclusive_parent_precedence(category_id)?
                    .map(|(parent_id, index)| (Some(parent_id), index))
                    .unwrap_or((None, usize::MAX)),
                crate::classification::CandidateAssignment::When(_) => (None, usize::MAX),
            };

            let choice_index = choices.len();
            choices.push(ReviewQueueChoice {
                suggestion,
                exclusive_parent,
                exclusive_child_index,
                is_semantic,
            });

            if let Some(parent_id) = exclusive_parent {
                let should_replace = winning_choice_by_exclusive_parent
                    .get(&parent_id)
                    .copied()
                    .is_none_or(|winner_index| {
                        Self::review_choice_wins(&choices[choice_index], &choices[winner_index])
                    });
                if should_replace {
                    winning_choice_by_exclusive_parent.insert(parent_id, choice_index);
                }
            }
        }

        for (choice_index, choice) in choices.into_iter().enumerate() {
            if let Some(parent_id) = choice.exclusive_parent {
                let should_queue = winning_choice_by_exclusive_parent
                    .get(&parent_id)
                    .is_some_and(|winner_index| *winner_index == choice_index);
                if !should_queue {
                    if choice.is_semantic {
                        result.semantic_skipped_unavailable += 1;
                    }
                    continue;
                }
            }

            self.store.upsert_suggestion(&choice.suggestion)?;
            result.queued += 1;
            if choice.is_semantic {
                result.semantic_queued += 1;
            }
        }

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

    fn semantic_candidate_availability(
        &self,
        item_id: ItemId,
        candidate: &ClassificationCandidate,
    ) -> Result<SemanticCandidateAvailability> {
        match candidate.assignment {
            crate::classification::CandidateAssignment::Category(category_id) => {
                if self.candidate_assignment_already_present(item_id, candidate)? {
                    return Ok(SemanticCandidateAvailability::AlreadyAssigned);
                }
                if self.has_any_exclusive_sibling_assigned(item_id, category_id)? {
                    return Ok(SemanticCandidateAvailability::Unavailable);
                }
                if !self.preview_suggestion_assignment_would_change_effective_state(
                    item_id,
                    category_id,
                )? {
                    return Ok(SemanticCandidateAvailability::Unavailable);
                }
                Ok(SemanticCandidateAvailability::Available)
            }
            crate::classification::CandidateAssignment::When(_) => {
                if self.candidate_assignment_already_present(item_id, candidate)? {
                    Ok(SemanticCandidateAvailability::AlreadyAssigned)
                } else {
                    Ok(SemanticCandidateAvailability::Available)
                }
            }
        }
    }

    fn exclusive_parent_precedence(
        &self,
        category_id: CategoryId,
    ) -> Result<Option<(CategoryId, usize)>> {
        let category = self.store.get_category(category_id)?;
        let Some(parent_id) = category.parent else {
            return Ok(None);
        };
        let parent = self.store.get_category(parent_id)?;
        if !parent.is_exclusive {
            return Ok(None);
        }
        let child_index = parent
            .children
            .iter()
            .position(|child_id| *child_id == category_id)
            .unwrap_or(parent.children.len());
        Ok(Some((parent_id, child_index)))
    }

    fn review_choice_wins(candidate: &ReviewQueueChoice, incumbent: &ReviewQueueChoice) -> bool {
        let candidate_confidence = Self::review_confidence_score(candidate.suggestion.confidence);
        let incumbent_confidence = Self::review_confidence_score(incumbent.suggestion.confidence);
        if candidate_confidence > incumbent_confidence {
            return true;
        }
        if candidate_confidence < incumbent_confidence {
            return false;
        }
        candidate.exclusive_child_index < incumbent.exclusive_child_index
    }

    fn review_confidence_score(confidence: Option<f32>) -> f32 {
        match confidence {
            Some(value) if value.is_finite() => value,
            _ => 0.0,
        }
    }

    fn is_semantic_provider(provider_id: &str) -> bool {
        provider_id == PROVIDER_ID_OLLAMA_OPENAI_COMPAT
            || provider_id == PROVIDER_ID_OPENROUTER
            || provider_id == PROVIDER_ID_OPENAI
    }

    fn has_any_exclusive_sibling_assigned(
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
        Ok(parent
            .children
            .into_iter()
            .any(|sibling_id| sibling_id != category_id && assignments.contains_key(&sibling_id)))
    }

    fn preview_suggestion_assignment_would_change_effective_state(
        &self,
        item_id: ItemId,
        category_id: CategoryId,
    ) -> Result<bool> {
        let source_item = self.store.get_item(item_id)?;
        let current_assignments: HashSet<CategoryId> =
            source_item.assignments.keys().copied().collect();
        let preview_item = self.preview_suggestion_assignment(item_id, category_id)?;
        let preview_assignments: HashSet<CategoryId> =
            preview_item.assignments.keys().copied().collect();
        Ok(preview_assignments != current_assignments)
    }

    fn preview_suggestion_assignment(
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

        let preview_aglet = Aglet::new(&preview_store, self.classifier);
        let preview_category_id =
            Self::mapped_category_id(category_id, &id_map, "preview suggestion category")?;
        let preview_suggestion = ClassificationSuggestion {
            id: Uuid::new_v4(),
            item_id,
            assignment: crate::classification::CandidateAssignment::Category(preview_category_id),
            provider_id: "preview".to_string(),
            model: None,
            confidence: None,
            rationale: None,
            context_hash: "preview".to_string(),
            item_revision_hash: "preview".to_string(),
            status: SuggestionStatus::Pending,
            created_at: Timestamp::now(),
            decided_at: None,
        };
        let (_, preview_intent) = preview_aglet.apply_suggestion_assignment(&preview_suggestion)?;
        preview_aglet
            .reprocess_existing_item_with_intents(item_id, preview_intent.into_iter().collect())?;

        let mut preview_result = preview_store.get_item(item_id)?;
        preview_result.assignments = preview_result
            .assignments
            .into_iter()
            .map(|(preview_id, assignment)| {
                let source_id =
                    Self::source_category_id(preview_id, &id_map, "preview suggestion result")
                        .expect("preview category mapping should round-trip");
                (source_id, assignment)
            })
            .collect();
        Ok(preview_result)
    }

    fn process_category_change(&self, category_id: CategoryId) -> Result<EvaluateAllItemsResult> {
        let cfg = self.store.get_classification_config()?;
        let enable_implicit_string = cfg.should_run_continuously()
            && cfg.run_on_category_change
            && cfg.literal_mode == LiteralClassificationMode::AutoApply;
        let evaluation_context = EvaluationContext::now();
        let mut result = evaluate_all_items_with_options(
            self.store,
            self.classifier,
            category_id,
            ProcessOptions {
                enable_implicit_string,
                evaluation_context: evaluation_context.clone(),
                ..ProcessOptions::default()
            },
        )?;
        self.apply_bulk_deferred_specials(&mut result, &evaluation_context)?;
        Ok(result)
    }

    /// Apply the special-action effects a bulk evaluation produced, item by
    /// item — the bulk counterpart of the specials application that
    /// single-item reprocessing performs. Items may have been deleted by an
    /// earlier item's cascade; those are skipped.
    fn apply_bulk_deferred_specials(
        &self,
        result: &mut EvaluateAllItemsResult,
        evaluation_context: &EvaluationContext,
    ) -> Result<()> {
        let per_item = std::mem::take(&mut result.deferred_specials);
        if per_item.is_empty() {
            return Ok(());
        }
        let mut grouped: Vec<(ItemId, Vec<crate::engine::DeferredSpecial>)> = Vec::new();
        for (item_id, special) in per_item {
            match grouped.iter_mut().find(|(id, _)| *id == item_id) {
                Some((_, specials)) => specials.push(special),
                None => grouped.push((item_id, vec![special])),
            }
        }
        for (item_id, specials) in grouped {
            if !self.item_exists(item_id)? {
                continue;
            }
            let mut item_result = ProcessItemResult::default();
            self.apply_deferred_specials(item_id, &specials, evaluation_context, &mut item_result)?;
        }
        Ok(())
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
        self.reprocess_existing_item_with_triggers(item_id, HashSet::new())
    }

    /// Reprocess after an external assignment write. `action_triggers` names the
    /// categories whose assignment the caller just created; the engine fires
    /// their actions as assignment events.
    fn reprocess_existing_item_with_triggers(
        &self,
        item_id: ItemId,
        action_triggers: HashSet<CategoryId>,
    ) -> Result<ProcessItemResult> {
        let cfg = self.store.get_classification_config()?;
        self.reprocess_with_options(
            item_id,
            self.should_reprocess_with_implicit(&cfg),
            EvaluationContext::now(),
            action_triggers,
            Vec::new(),
        )
    }

    /// Reprocess while submitting assignment intents for the engine to
    /// execute — the single write path for assignments.
    fn reprocess_existing_item_with_intents(
        &self,
        item_id: ItemId,
        intents: Vec<AssignmentIntent>,
    ) -> Result<ProcessItemResult> {
        let cfg = self.store.get_classification_config()?;
        self.reprocess_with_options(
            item_id,
            self.should_reprocess_with_implicit(&cfg),
            EvaluationContext::now(),
            HashSet::new(),
            intents,
        )
    }

    fn reprocess_with_options(
        &self,
        item_id: ItemId,
        enable_implicit_string: bool,
        evaluation_context: EvaluationContext,
        pending_action_triggers: HashSet<CategoryId>,
        pending_intents: Vec<AssignmentIntent>,
    ) -> Result<ProcessItemResult> {
        let mut result = process_item_with_options(
            self.store,
            self.classifier,
            item_id,
            ProcessOptions {
                enable_implicit_string,
                evaluation_context: evaluation_context.clone(),
                pending_action_triggers,
                pending_intents,
            },
        )?;
        if !result.deferred_specials.is_empty() {
            let specials = std::mem::take(&mut result.deferred_specials);
            self.apply_deferred_specials(item_id, &specials, &evaluation_context, &mut result)?;
        }
        Ok(result)
    }

    /// Apply the item-mutating effects of special actions (SetWhen, MarkDone,
    /// Delete) after an engine run. Order: dates first, done second, delete
    /// last — a deletion ends processing. Each application reprocesses the
    /// item, which can produce further specials; `specials_depth` caps that
    /// recursion so cascades terminate.
    fn apply_deferred_specials(
        &self,
        item_id: ItemId,
        specials: &[crate::engine::DeferredSpecial],
        evaluation_context: &EvaluationContext,
        result: &mut ProcessItemResult,
    ) -> Result<()> {
        const MAX_SPECIALS_DEPTH: u8 = 3;
        let depth = self.specials_depth.get();
        if depth >= MAX_SPECIALS_DEPTH {
            let warning = format!(
                "rule cascade cut short: {} special effect(s) dropped at depth {} for item {item_id}",
                specials.len(),
                depth
            );
            self.debug_log(&format!("specials: {warning}"));
            result.warnings.push(warning);
            return Ok(());
        }
        self.specials_depth.set(depth + 1);
        let applied = self.apply_deferred_specials_inner(item_id, specials, evaluation_context, result);
        self.specials_depth.set(depth);
        applied
    }

    fn item_exists(&self, item_id: ItemId) -> Result<bool> {
        match self.store.get_item(item_id) {
            Ok(_) => Ok(true),
            Err(AgletError::NotFound { .. }) => Ok(false),
            Err(err) => Err(err),
        }
    }

    fn apply_deferred_specials_inner(
        &self,
        item_id: ItemId,
        specials: &[crate::engine::DeferredSpecial],
        evaluation_context: &EvaluationContext,
        result: &mut ProcessItemResult,
    ) -> Result<()> {
        use crate::engine::SpecialActionKind;

        let trigger_name = |category_id: CategoryId| {
            self.store
                .get_category(category_id)
                .map(|category| category.name)
                .unwrap_or_else(|_| category_id.to_string())
        };

        let mut mark_done_by: Option<CategoryId> = None;
        let mut delete_by: Option<CategoryId> = None;
        let mut when_changed = false;

        for special in specials {
            match &special.kind {
                SpecialActionKind::SetWhen(expr) => {
                    let when = crate::date_rules::resolve_date_value_expr(expr, evaluation_context);
                    let name = trigger_name(special.triggered_by);
                    if self.apply_when_assignment(
                        item_id,
                        when,
                        AssignmentSource::Action,
                        Some(format!("action:{name}")),
                        Some(AssignmentExplanation::Action {
                            trigger_category_name: name,
                            kind: AssignmentActionKind::Assign,
                        }),
                    )? {
                        when_changed = true;
                    }
                }
                SpecialActionKind::MarkDone => {
                    mark_done_by.get_or_insert(special.triggered_by);
                }
                SpecialActionKind::Delete => {
                    delete_by.get_or_insert(special.triggered_by);
                }
            }
        }

        if when_changed {
            // The item changed outside the engine run; re-enter processing so
            // date conditions see the new When value (target.md §2.5 step 7).
            let cascade = self.reprocess_with_options(
                item_id,
                false,
                evaluation_context.clone(),
                HashSet::new(),
                Vec::new(),
            )?;
            merge_process_results(result, cascade);
            // That cascade runs nested specials at depth+1 and may have
            // deleted the item (e.g. a date condition now matches a category
            // carrying a Delete action). Nothing left to apply if so.
            if !self.item_exists(item_id)? {
                return Ok(());
            }
        }

        if let Some(trigger) = mark_done_by {
            let item = self.store.get_item(item_id)?;
            if !item.is_done {
                self.debug_log(&format!(
                    "specials: MarkDone on item {item_id} triggered by {}",
                    trigger_name(trigger)
                ));
                let done = self.mark_item_done(item_id)?;
                merge_process_results(result, done);
                // mark_item_done reprocesses; a nested Delete may have fired.
                if !self.item_exists(item_id)? {
                    return Ok(());
                }
            }
        }

        if let Some(trigger) = delete_by {
            // allow_delete_action was checked at fire time; the deletion log
            // records the triggering category via deleted_by.
            let name = trigger_name(trigger);
            self.store.delete_item(item_id, &format!("action:{name}"))?;
            self.debug_log(&format!(
                "specials: Delete on item {item_id} triggered by {name} (logged to deletion log)"
            ));
        }

        Ok(())
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
                    .ok_or_else(|| AgletError::InvalidOperation {
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
            Condition::Date { source, matcher } => Ok(Condition::Date {
                source: *source,
                matcher: matcher.clone(),
            }),
            Condition::Numeric {
                category_id,
                min,
                max,
                outside,
            } => Ok(Condition::Numeric {
                category_id: Self::mapped_category_id(
                    *category_id,
                    id_map,
                    "preview numeric condition",
                )?,
                min: *min,
                max: *max,
                outside: *outside,
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
            Action::AssignNumeric { target, value } => Ok(Action::AssignNumeric {
                target: Self::mapped_category_id(*target, id_map, "preview numeric action")?,
                value: *value,
            }),
            Action::SetWhen { value } => Ok(Action::SetWhen {
                value: value.clone(),
            }),
            Action::MarkDone => Ok(Action::MarkDone),
            Action::Delete => Ok(Action::Delete),
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
            .ok_or_else(|| AgletError::InvalidOperation {
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
            .ok_or_else(|| AgletError::InvalidOperation {
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
            PROVIDER_ID_OLLAMA_OPENAI_COMPAT | PROVIDER_ID_OPENROUTER | PROVIDER_ID_OPENAI => {
                match cfg.semantic_mode {
                    SemanticClassificationMode::Off => CandidateDisposition::Skip,
                    SemanticClassificationMode::SuggestReview => CandidateDisposition::QueueReview,
                }
            }
            _ => CandidateDisposition::Skip,
        }
    }

    /// Convert an auto-classified candidate into work: Category candidates
    /// become assignment intents the engine executes (single write path,
    /// with veto and exclusivity handling); When candidates are applied
    /// immediately because they sync the typed `when_date` field.
    fn apply_auto_classification_candidate(
        &self,
        item_id: ItemId,
        candidate: &ClassificationCandidate,
    ) -> Result<(ProcessItemResult, Option<AssignmentIntent>)> {
        match &candidate.assignment {
            crate::classification::CandidateAssignment::Category(category_id) => {
                let category = self.store.get_category(*category_id)?;
                let intent = AssignmentIntent {
                    category_id: *category_id,
                    source: AssignmentSource::AutoClassified,
                    origin: Some(format!("cat:{}", category.name)),
                    explanation: Some(AssignmentExplanation::AutoClassified {
                        provider_id: candidate.provider.clone(),
                        model: candidate.model.clone(),
                        rationale: candidate.rationale.clone(),
                    }),
                    numeric_value: None,
                    upgrade_existing: false,
                    clears_veto: false,
                    override_exclusive: false,
                };
                Ok((ProcessItemResult::default(), Some(intent)))
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
                Ok((result, None))
            }
        }
    }


    /// Convert an accepted suggestion into work: Category suggestions become
    /// assignment intents (explicit user intent: clears the veto and may kick
    /// exclusive siblings); When suggestions are applied immediately because
    /// they sync the typed `when_date` field.
    fn apply_suggestion_assignment(
        &self,
        suggestion: &ClassificationSuggestion,
    ) -> Result<(ProcessItemResult, Option<AssignmentIntent>)> {
        let origin = Some(format!("suggestion:accepted:{}", suggestion.provider_id));
        match suggestion.assignment {
            crate::classification::CandidateAssignment::Category(category_id) => {
                let intent = AssignmentIntent {
                    category_id,
                    source: AssignmentSource::SuggestionAccepted,
                    origin,
                    explanation: Some(AssignmentExplanation::SuggestionAccepted {
                        provider_id: suggestion.provider_id.clone(),
                        model: suggestion.model.clone(),
                        rationale: suggestion.rationale.clone(),
                    }),
                    numeric_value: None,
                    upgrade_existing: false,
                    clears_veto: true,
                    override_exclusive: true,
                };
                Ok((ProcessItemResult::default(), Some(intent)))
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
                Ok((result, None))
            }
        }
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

    /// Intents for edit-through assignment (section insert/move): idempotent
    /// for categories that are already assigned.
    fn manual_category_intents(
        targets: &HashSet<CategoryId>,
        origin: &str,
    ) -> Vec<AssignmentIntent> {
        targets
            .iter()
            .map(|category_id| {
                Self::manual_intent(
                    *category_id,
                    Some(origin.to_string()),
                    origin,
                    None,
                    false,
                )
            })
            .collect()
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

    fn debug_log_process_result(&self, context: &str, item_id: ItemId, result: &ProcessItemResult) {
        if !self.debug {
            return;
        }
        for event in &result.assignment_events {
            eprintln!(
                "[aglet.debug] context={context} item={item_id} event={:?} category={} summary={}",
                event.kind, event.category_name, event.summary
            );
        }
        for message in &result.semantic_debug_messages {
            eprintln!("[aglet.debug] context={context} item={item_id} semantic={message}");
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
            "[aglet.debug] event={family} kind={kind:?} item={item_id} other={other_item_id}"
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
            return Err(AgletError::InvalidOperation {
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
                return Err(AgletError::InvalidOperation {
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
            .ok_or_else(|| AgletError::StorageError {
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
    target.semantic_candidates_skipped_unavailable +=
        incoming.semantic_candidates_skipped_unavailable;
    target
        .semantic_debug_messages
        .extend(incoming.semantic_debug_messages);
    target.warnings.extend(incoming.warnings);
}

#[cfg(test)]
#[path = "aglet_tests.rs"]
mod tests;
