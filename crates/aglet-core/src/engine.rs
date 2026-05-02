use std::collections::{HashMap, HashSet};

use jiff::Timestamp;

use crate::date_rules::{item_matches_date_condition, render_date_condition, EvaluationContext};
use crate::error::{AgletError, Result};
use crate::matcher::Classifier;
use crate::model::{
    origin as origin_const, Action, Assignment, AssignmentActionKind, AssignmentEvent,
    AssignmentEventKind, AssignmentExplanation, AssignmentSource, Category, CategoryId, Condition,
    ConditionMatchMode, Item, ItemId, Query, TextMatchSource,
};
use crate::store::Store;

const MAX_PASSES: usize = 10;
const PROCESS_ITEM_SAVEPOINT: &str = "process_item_run";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeferredRemoval {
    pub target: CategoryId,
    pub triggered_by: CategoryId,
}

#[derive(Debug, Default)]
pub struct ProcessItemResult {
    pub new_assignments: HashSet<CategoryId>,
    pub removed_assignments: HashSet<CategoryId>,
    pub assignment_events: Vec<AssignmentEvent>,
    pub deferred_removals: Vec<DeferredRemoval>,
    pub semantic_candidates_seen: usize,
    pub semantic_candidates_queued_review: usize,
    pub semantic_candidates_skipped_already_assigned: usize,
    pub semantic_candidates_skipped_unavailable: usize,
    pub semantic_debug_messages: Vec<String>,
    /// When a recurring item is marked done, this holds the successor item's ID.
    pub successor_item_id: Option<ItemId>,
}

#[derive(Debug, Default)]
pub struct EvaluateAllItemsResult {
    pub processed_items: usize,
    pub affected_items: usize,
    pub total_new_assignments: usize,
    pub total_removed_assignments: usize,
    pub total_deferred_removals: usize,
}

#[derive(Debug, Default)]
struct PassResult {
    new_assignments: HashSet<CategoryId>,
    assignment_events: Vec<AssignmentEvent>,
    deferred_removals: Vec<DeferredRemoval>,
    changed: bool,
}

struct HierarchyPassInput<'a> {
    classifier: &'a dyn Classifier,
    item: &'a Item,
    item_id: ItemId,
    item_text: &'a str,
    categories: &'a [Category],
    categories_by_id: &'a HashMap<CategoryId, &'a Category>,
    original_assignments: &'a HashMap<CategoryId, Assignment>,
    options: ProcessOptions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct InsertResult {
    inserted: bool,
    new_to_store: bool,
}

#[derive(Debug, Clone)]
struct AssignmentTemplate {
    source: AssignmentSource,
    sticky: bool,
    origin: Option<String>,
    explanation: Option<AssignmentExplanation>,
}

#[derive(Debug, Clone)]
pub struct ProcessOptions {
    pub enable_implicit_string: bool,
    pub evaluation_context: EvaluationContext,
}

impl Default for ProcessOptions {
    fn default() -> Self {
        Self {
            enable_implicit_string: true,
            evaluation_context: EvaluationContext::now(),
        }
    }
}

#[derive(Debug, Clone)]
struct MatchReason {
    explanation: AssignmentExplanation,
    origin: String,
}

/// Process one item through fixed-point category evaluation.
///
/// The engine performs repeated hierarchy passes until a pass yields no new
/// assignments, or returns an error if it would require more than MAX_PASSES.
/// Remove actions are deferred during the cascade and applied once at the end.
pub fn process_item(
    store: &Store,
    classifier: &dyn Classifier,
    item_id: ItemId,
) -> Result<ProcessItemResult> {
    process_item_with_options(store, classifier, item_id, ProcessOptions::default())
}

pub fn process_item_with_options(
    store: &Store,
    classifier: &dyn Classifier,
    item_id: ItemId,
    options: ProcessOptions,
) -> Result<ProcessItemResult> {
    run_in_savepoint(store, || {
        process_item_inner(store, classifier, item_id, options)
    })
}

/// Evaluate all items in the store against the current hierarchy.
///
/// Error strategy for MVP: fail fast. If one item processing run fails,
/// return that error immediately rather than skipping it and continuing.
pub fn evaluate_all_items(
    store: &Store,
    classifier: &dyn Classifier,
    category_id: CategoryId,
) -> Result<EvaluateAllItemsResult> {
    evaluate_all_items_with_options(store, classifier, category_id, ProcessOptions::default())
}

pub fn reevaluate_all_items_with_options(
    store: &Store,
    classifier: &dyn Classifier,
    options: ProcessOptions,
) -> Result<EvaluateAllItemsResult> {
    evaluate_items_internal(store, classifier, options)
}

pub fn evaluate_all_items_with_options(
    store: &Store,
    classifier: &dyn Classifier,
    category_id: CategoryId,
    options: ProcessOptions,
) -> Result<EvaluateAllItemsResult> {
    // Validate the target category exists before beginning retroactive work.
    store.get_category(category_id)?;

    evaluate_items_internal(store, classifier, options)
}

fn evaluate_items_internal(
    store: &Store,
    classifier: &dyn Classifier,
    options: ProcessOptions,
) -> Result<EvaluateAllItemsResult> {
    let mut result = EvaluateAllItemsResult::default();
    let items = store.list_items()?;

    for item in items {
        let process_result =
            process_item_with_options(store, classifier, item.id, options.clone())?;

        result.processed_items += 1;
        result.total_new_assignments += process_result.new_assignments.len();
        result.total_removed_assignments += process_result.removed_assignments.len();
        result.total_deferred_removals += process_result.deferred_removals.len();

        if !process_result.new_assignments.is_empty()
            || !process_result.removed_assignments.is_empty()
            || !process_result.deferred_removals.is_empty()
        {
            result.affected_items += 1;
        }
    }

    Ok(result)
}

fn process_item_inner(
    store: &Store,
    classifier: &dyn Classifier,
    item_id: ItemId,
    options: ProcessOptions,
) -> Result<ProcessItemResult> {
    let item = store.get_item(item_id)?;
    let categories = store.get_hierarchy()?;
    let categories_by_id: HashMap<CategoryId, &Category> = categories
        .iter()
        .map(|category| (category.id, category))
        .collect();
    let match_text = item_match_text(&item);

    let original_assignments = item.assignments.clone();
    let mut assignments = retained_assignments(&original_assignments);
    let mut seen_pairs: HashSet<(ItemId, CategoryId)> = assignments
        .keys()
        .copied()
        .map(|category_id| (item_id, category_id))
        .collect();

    rebuild_live_subsumption_assignments(
        item_id,
        &categories_by_id,
        &original_assignments,
        &mut assignments,
        &mut seen_pairs,
    );

    let mut result = ProcessItemResult::default();
    let pass_input = HierarchyPassInput {
        classifier,
        item: &item,
        item_id,
        item_text: &match_text,
        categories: &categories,
        categories_by_id: &categories_by_id,
        original_assignments: &original_assignments,
        options,
    };

    for pass in 1..=MAX_PASSES {
        let pass_result = run_hierarchy_pass(&pass_input, &mut assignments, &mut seen_pairs)?;

        let changed = pass_result.changed;

        result.new_assignments.extend(pass_result.new_assignments);
        result
            .assignment_events
            .extend(pass_result.assignment_events);
        result
            .deferred_removals
            .extend(pass_result.deferred_removals);

        if !changed {
            apply_deferred_removals_to_assignments(&result.deferred_removals, &mut assignments);
            drop_live_subsumption_assignments(&mut assignments);
            let mut final_seen_pairs: HashSet<(ItemId, CategoryId)> = assignments
                .keys()
                .copied()
                .map(|category_id| (item_id, category_id))
                .collect();
            rebuild_live_subsumption_assignments(
                item_id,
                &categories_by_id,
                &original_assignments,
                &mut assignments,
                &mut final_seen_pairs,
            );
            sync_assignments(
                store,
                item_id,
                &original_assignments,
                &assignments,
                &mut result,
            )?;
            return Ok(result);
        }

        if pass == MAX_PASSES {
            return Err(pass_cap_error(item_id));
        }
    }

    unreachable!("fixed-point loop should always return from within MAX_PASSES");
}

fn item_match_text(item: &crate::model::Item) -> String {
    match item.note.as_deref() {
        Some(note) if !note.trim().is_empty() => format!("{} {}", item.text, note),
        _ => item.text.clone(),
    }
}

fn run_hierarchy_pass(
    input: &HierarchyPassInput<'_>,
    assignments: &mut HashMap<CategoryId, Assignment>,
    seen_pairs: &mut HashSet<(ItemId, CategoryId)>,
) -> Result<PassResult> {
    let mut pass_result = PassResult::default();

    for category in input.categories {
        let Some(reason) = evaluate_category_match(
            category,
            input.item,
            input.item_text,
            assignments,
            input.classifier,
            input.categories_by_id,
            &input.options,
        ) else {
            continue;
        };

        if !can_assign(input.item_id, category.id, assignments, seen_pairs) {
            continue;
        }
        if has_blocking_exclusive_sibling(category.id, input.categories_by_id, assignments) {
            continue;
        }

        enforce_mutual_exclusion(category.id, input.categories_by_id, assignments)?;

        let assigned = assign_if_unassigned(
            input.item_id,
            category.id,
            AssignmentTemplate {
                source: AssignmentSource::AutoMatch,
                sticky: false,
                origin: Some(reason.origin.clone()),
                explanation: Some(reason.explanation.clone()),
            },
            input.original_assignments,
            assignments,
            seen_pairs,
        );

        if !assigned.inserted {
            continue;
        }
        pass_result.changed = true;
        assign_subsumption_ancestors(
            input.item_id,
            category.id,
            input.categories_by_id,
            input.original_assignments,
            assignments,
            seen_pairs,
        );
        if assigned.new_to_store {
            pass_result.new_assignments.insert(category.id);
            pass_result.assignment_events.push(AssignmentEvent {
                kind: AssignmentEventKind::Assigned,
                category_id: category.id,
                category_name: category.name.clone(),
                summary: reason.explanation.summary(),
            });
            fire_actions(
                input.item_id,
                category,
                input.categories_by_id,
                input.original_assignments,
                assignments,
                seen_pairs,
                &mut pass_result,
            )?;
        }
    }

    Ok(pass_result)
}

fn evaluate_category_match(
    category: &Category,
    item: &Item,
    item_text: &str,
    assignments: &HashMap<CategoryId, Assignment>,
    classifier: &dyn Classifier,
    categories_by_id: &HashMap<CategoryId, &Category>,
    options: &ProcessOptions,
) -> Option<MatchReason> {
    if options.enable_implicit_string && category.enable_implicit_string {
        if let Some(matched) = classifier.classify(
            item_text,
            &category.name,
            category.match_category_name,
            &category.also_match,
        ) {
            let matched_source = match matched.source {
                crate::matcher::ImplicitMatchSource::CategoryName => TextMatchSource::CategoryName,
                crate::matcher::ImplicitMatchSource::AlsoMatch => TextMatchSource::AlsoMatch,
            };
            return Some(MatchReason {
                origin: match_origin_implicit(&category.name),
                explanation: AssignmentExplanation::ImplicitMatch {
                    matched_term: matched.matched_term,
                    matched_source,
                },
            });
        }
    }

    match category.condition_match_mode {
        ConditionMatchMode::Any => {
            for (condition_index, condition) in category.conditions.iter().enumerate() {
                match condition {
                    Condition::Profile { criteria } => {
                        if profile_matches(criteria, assignments) {
                            return Some(MatchReason {
                                origin: match_origin_profile(&category.name),
                                explanation: AssignmentExplanation::ProfileCondition {
                                    owner_category_name: category.name.clone(),
                                    condition_index,
                                    rendered_rule: render_condition_rule(
                                        condition,
                                        categories_by_id,
                                    ),
                                },
                            });
                        }
                    }
                    Condition::Date { source, matcher } => {
                        if item_matches_date_condition(
                            item,
                            *source,
                            matcher,
                            &options.evaluation_context,
                        ) {
                            return Some(MatchReason {
                                origin: format!("match:date:{}", category.name),
                                explanation: AssignmentExplanation::DateCondition {
                                    owner_category_name: category.name.clone(),
                                    condition_index,
                                    rendered_rule: render_condition_rule(
                                        condition,
                                        categories_by_id,
                                    ),
                                },
                            });
                        }
                    }
                    Condition::ImplicitString => {}
                }
            }
        }
        ConditionMatchMode::All => {
            let explicit_conditions: Vec<_> = category
                .conditions
                .iter()
                .filter(|condition| !matches!(condition, Condition::ImplicitString))
                .collect();
            if !explicit_conditions.is_empty()
                && explicit_conditions.iter().all(|condition| {
                    condition_matches(condition, item, assignments, categories_by_id, options)
                })
            {
                let rendered_rules = explicit_conditions
                    .iter()
                    .map(|condition| render_condition_rule(condition, categories_by_id))
                    .collect();
                return Some(MatchReason {
                    origin: format!("match:conditions:all:{}", category.name),
                    explanation: AssignmentExplanation::ConditionGroup {
                        owner_category_name: category.name.clone(),
                        match_mode: ConditionMatchMode::All,
                        rendered_rules,
                    },
                });
            }
        }
    }

    None
}

fn condition_matches(
    condition: &Condition,
    item: &Item,
    assignments: &HashMap<CategoryId, Assignment>,
    _categories_by_id: &HashMap<CategoryId, &Category>,
    options: &ProcessOptions,
) -> bool {
    match condition {
        Condition::Profile { criteria } => profile_matches(criteria, assignments),
        Condition::Date { source, matcher } => {
            item_matches_date_condition(item, *source, matcher, &options.evaluation_context)
        }
        Condition::ImplicitString => false,
    }
}

fn render_condition_rule(
    condition: &Condition,
    categories_by_id: &HashMap<CategoryId, &Category>,
) -> String {
    match condition {
        Condition::Profile { criteria } => criteria.format_trigger(&|category_id| {
            categories_by_id
                .get(&category_id)
                .map(|candidate| candidate.name.clone())
                .unwrap_or_else(|| category_id.to_string())
        }),
        Condition::Date { source, matcher } => render_date_condition(*source, matcher),
        Condition::ImplicitString => "ImplicitString".to_string(),
    }
}

fn profile_matches(criteria: &Query, assignments: &HashMap<CategoryId, Assignment>) -> bool {
    criteria
        .and_category_ids()
        .all(|category_id| assignments.contains_key(&category_id))
        && criteria
            .not_category_ids()
            .all(|category_id| !assignments.contains_key(&category_id))
        && {
            let or_ids: Vec<_> = criteria.or_category_ids().collect();
            or_ids.is_empty() || or_ids.iter().any(|id| assignments.contains_key(id))
        }
}

fn fire_actions(
    item_id: ItemId,
    category: &Category,
    categories_by_id: &HashMap<CategoryId, &Category>,
    original_assignments: &HashMap<CategoryId, Assignment>,
    assignments: &mut HashMap<CategoryId, Assignment>,
    seen_pairs: &mut HashSet<(ItemId, CategoryId)>,
    pass_result: &mut PassResult,
) -> Result<()> {
    for action in &category.actions {
        match action {
            Action::Assign { targets } => {
                for target_id in targets {
                    if !can_assign(item_id, *target_id, assignments, seen_pairs) {
                        continue;
                    }
                    if has_blocking_exclusive_sibling(*target_id, categories_by_id, assignments) {
                        continue;
                    }
                    enforce_mutual_exclusion(*target_id, categories_by_id, assignments)?;

                    let assigned = assign_if_unassigned(
                        item_id,
                        *target_id,
                        AssignmentTemplate {
                            source: AssignmentSource::Action,
                            sticky: true,
                            origin: Some(format!("action:{}", category.name)),
                            explanation: Some(AssignmentExplanation::Action {
                                trigger_category_name: category.name.clone(),
                                kind: AssignmentActionKind::Assign,
                            }),
                        },
                        original_assignments,
                        assignments,
                        seen_pairs,
                    );
                    if assigned.inserted {
                        pass_result.changed = true;
                        assign_subsumption_ancestors(
                            item_id,
                            *target_id,
                            categories_by_id,
                            original_assignments,
                            assignments,
                            seen_pairs,
                        );
                        if assigned.new_to_store {
                            pass_result.new_assignments.insert(*target_id);
                            let target_name = categories_by_id
                                .get(target_id)
                                .map(|category| category.name.clone())
                                .unwrap_or_else(|| target_id.to_string());
                            pass_result.assignment_events.push(AssignmentEvent {
                                kind: AssignmentEventKind::Assigned,
                                category_id: *target_id,
                                category_name: target_name,
                                summary: format!("Assigned by action on {}", category.name),
                            });
                        }
                    }
                }
            }
            Action::Remove { targets } => {
                for target_id in targets {
                    pass_result.deferred_removals.push(DeferredRemoval {
                        target: *target_id,
                        triggered_by: category.id,
                    });
                }
            }
        }
    }

    Ok(())
}

fn can_assign(
    item_id: ItemId,
    category_id: CategoryId,
    assignments: &HashMap<CategoryId, Assignment>,
    seen_pairs: &mut HashSet<(ItemId, CategoryId)>,
) -> bool {
    let pair = (item_id, category_id);

    if assignments.contains_key(&category_id) {
        seen_pairs.insert(pair);
        return false;
    }

    !seen_pairs.contains(&pair)
}

fn assign_if_unassigned(
    item_id: ItemId,
    category_id: CategoryId,
    template: AssignmentTemplate,
    original_assignments: &HashMap<CategoryId, Assignment>,
    assignments: &mut HashMap<CategoryId, Assignment>,
    seen_pairs: &mut HashSet<(ItemId, CategoryId)>,
) -> InsertResult {
    if !can_assign(item_id, category_id, assignments, seen_pairs) {
        return InsertResult {
            inserted: false,
            new_to_store: false,
        };
    }

    let pair = (item_id, category_id);
    let new_to_store = !original_assignments.contains_key(&category_id);
    let assignment = original_assignments
        .get(&category_id)
        .cloned()
        .unwrap_or_else(|| Assignment {
            source: template.source,
            assigned_at: Timestamp::now(),
            sticky: template.sticky,
            origin: template.origin,
            explanation: template.explanation,
            numeric_value: None,
        });
    assignments.insert(category_id, assignment);
    seen_pairs.insert(pair);

    InsertResult {
        inserted: true,
        new_to_store,
    }
}

fn enforce_mutual_exclusion(
    category_id: CategoryId,
    categories_by_id: &HashMap<CategoryId, &Category>,
    assignments: &mut HashMap<CategoryId, Assignment>,
) -> Result<()> {
    let Some(category) = categories_by_id.get(&category_id) else {
        return Ok(());
    };
    let Some(parent_id) = category.parent else {
        return Ok(());
    };
    let Some(parent) = categories_by_id.get(&parent_id) else {
        return Ok(());
    };
    if !parent.is_exclusive {
        return Ok(());
    }

    for sibling_id in &parent.children {
        if *sibling_id == category_id {
            continue;
        }

        assignments.remove(sibling_id);
    }

    Ok(())
}

fn has_blocking_exclusive_sibling(
    category_id: CategoryId,
    categories_by_id: &HashMap<CategoryId, &Category>,
    assignments: &HashMap<CategoryId, Assignment>,
) -> bool {
    let Some(category) = categories_by_id.get(&category_id) else {
        return false;
    };
    let Some(parent_id) = category.parent else {
        return false;
    };
    let Some(parent) = categories_by_id.get(&parent_id) else {
        return false;
    };
    if !parent.is_exclusive {
        return false;
    }

    let current_index = parent
        .children
        .iter()
        .position(|child_id| *child_id == category_id)
        .unwrap_or(parent.children.len());

    parent
        .children
        .iter()
        .enumerate()
        .any(|(sibling_index, sibling_id)| {
            if *sibling_id == category_id {
                return false;
            }
            let Some(assignment) = assignments.get(sibling_id) else {
                return false;
            };
            matches!(
                assignment.source,
                AssignmentSource::Manual | AssignmentSource::SuggestionAccepted
            ) || sibling_index < current_index
        })
}

fn assign_subsumption_ancestors(
    item_id: ItemId,
    category_id: CategoryId,
    categories_by_id: &HashMap<CategoryId, &Category>,
    original_assignments: &HashMap<CategoryId, Assignment>,
    assignments: &mut HashMap<CategoryId, Assignment>,
    seen_pairs: &mut HashSet<(ItemId, CategoryId)>,
) {
    let Some(category) = categories_by_id.get(&category_id) else {
        return;
    };

    let subsumption_origin = Some(format!("{}:{}", origin_const::SUBSUMPTION, category.name));
    let mut current_parent = category.parent;
    let mut visited = HashSet::new();

    while let Some(ancestor_id) = current_parent {
        if !visited.insert(ancestor_id) {
            break;
        }

        let pair = (item_id, ancestor_id);
        if let std::collections::hash_map::Entry::Vacant(entry) = assignments.entry(ancestor_id) {
            if !seen_pairs.contains(&pair) {
                let parent_name = categories_by_id
                    .get(&ancestor_id)
                    .map(|category| category.name.clone())
                    .unwrap_or_else(|| ancestor_id.to_string());
                let assignment = original_assignments
                    .get(&ancestor_id)
                    .cloned()
                    .unwrap_or_else(|| Assignment {
                        source: AssignmentSource::Subsumption,
                        assigned_at: Timestamp::now(),
                        sticky: false,
                        origin: subsumption_origin.clone(),
                        explanation: Some(AssignmentExplanation::Subsumption {
                            parent_category_name: parent_name,
                            via_child_category_name: category.name.clone(),
                        }),
                        numeric_value: None,
                    });
                entry.insert(assignment);
                seen_pairs.insert(pair);
            }
        } else {
            seen_pairs.insert(pair);
        }

        current_parent = categories_by_id
            .get(&ancestor_id)
            .and_then(|ancestor| ancestor.parent);
    }
}

fn apply_deferred_removals_to_assignments(
    deferred_removals: &[DeferredRemoval],
    assignments: &mut HashMap<CategoryId, Assignment>,
) {
    let mut removed_targets = HashSet::new();

    for removal in deferred_removals {
        if removed_targets.insert(removal.target) {
            assignments.remove(&removal.target);
        }
    }
}

fn drop_live_subsumption_assignments(assignments: &mut HashMap<CategoryId, Assignment>) {
    assignments.retain(|_, assignment| {
        assignment.source != AssignmentSource::Subsumption || assignment.sticky
    });
}

fn retained_assignments(
    original_assignments: &HashMap<CategoryId, Assignment>,
) -> HashMap<CategoryId, Assignment> {
    original_assignments
        .iter()
        .filter(|(_, assignment)| !is_live_derived_assignment(assignment))
        .map(|(&category_id, assignment)| (category_id, assignment.clone()))
        .collect()
}

fn is_live_derived_assignment(assignment: &Assignment) -> bool {
    !assignment.sticky
        && matches!(
            assignment.source,
            AssignmentSource::AutoMatch | AssignmentSource::Subsumption
        )
}

fn rebuild_live_subsumption_assignments(
    item_id: ItemId,
    categories_by_id: &HashMap<CategoryId, &Category>,
    original_assignments: &HashMap<CategoryId, Assignment>,
    assignments: &mut HashMap<CategoryId, Assignment>,
    seen_pairs: &mut HashSet<(ItemId, CategoryId)>,
) {
    let direct_assignment_ids: Vec<CategoryId> = assignments
        .iter()
        .filter(|(_, assignment)| assignment.source != AssignmentSource::Subsumption)
        .map(|(&category_id, _)| category_id)
        .collect();

    for category_id in direct_assignment_ids {
        assign_subsumption_ancestors(
            item_id,
            category_id,
            categories_by_id,
            original_assignments,
            assignments,
            seen_pairs,
        );
    }
}

fn sync_assignments(
    store: &Store,
    item_id: ItemId,
    original_assignments: &HashMap<CategoryId, Assignment>,
    desired_assignments: &HashMap<CategoryId, Assignment>,
    result: &mut ProcessItemResult,
) -> Result<()> {
    let original_ids: HashSet<CategoryId> = original_assignments.keys().copied().collect();
    let desired_ids: HashSet<CategoryId> = desired_assignments.keys().copied().collect();

    for category_id in original_ids.difference(&desired_ids) {
        let category_name = store
            .get_category(*category_id)
            .map(|category| category.name)
            .unwrap_or_else(|_| category_id.to_string());
        let summary = original_assignments
            .get(category_id)
            .and_then(|assignment| assignment.explanation.as_ref())
            .map(AssignmentExplanation::removal_summary)
            .unwrap_or_else(|| "Assignment removed".to_string());
        store.unassign_item(item_id, *category_id)?;
        result.removed_assignments.insert(*category_id);
        result.assignment_events.push(AssignmentEvent {
            kind: AssignmentEventKind::Removed,
            category_id: *category_id,
            category_name,
            summary,
        });
    }

    for category_id in desired_ids.difference(&original_ids) {
        if let Some(assignment) = desired_assignments.get(category_id) {
            store.assign_item(item_id, *category_id, assignment)?;
        }
    }

    Ok(())
}

fn match_origin_implicit(category_name: &str) -> String {
    format!("cat:{category_name}")
}

fn match_origin_profile(category_name: &str) -> String {
    format!("profile:{category_name}")
}

fn pass_cap_error(item_id: ItemId) -> AgletError {
    AgletError::InvalidOperation {
        message: format!(
            "rule processing exceeded {MAX_PASSES} passes for item {item_id}; possible cycle"
        ),
    }
}

fn run_in_savepoint<T, F>(store: &Store, f: F) -> Result<T>
where
    F: FnOnce() -> Result<T>,
{
    let connection = store.conn();

    connection.execute_batch(&format!("SAVEPOINT {PROCESS_ITEM_SAVEPOINT}"))?;

    match f() {
        Ok(value) => {
            connection.execute_batch(&format!("RELEASE SAVEPOINT {PROCESS_ITEM_SAVEPOINT}"))?;
            Ok(value)
        }
        Err(err) => {
            let rollback_sql = format!(
                "ROLLBACK TO SAVEPOINT {PROCESS_ITEM_SAVEPOINT}; RELEASE SAVEPOINT {PROCESS_ITEM_SAVEPOINT};"
            );
            let _ = connection.execute_batch(&rollback_sql);
            Err(err)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use jiff::{Timestamp, Zoned};

    use super::{
        evaluate_all_items, process_item, process_item_with_options, run_hierarchy_pass,
        HierarchyPassInput, ProcessOptions,
    };
    use crate::date_rules::EvaluationContext;
    use crate::error::AgletError;
    use crate::matcher::SubstringClassifier;
    use crate::model::{
        Action, Assignment, AssignmentExplanation, AssignmentSource, Category, CategoryId,
        Condition, ConditionMatchMode, CriterionMode, DateCompareOp, DateMatcher, DateSource,
        DateValueExpr, Item, ItemId, Query,
    };
    use crate::store::Store;

    fn create_category(store: &Store, category: &Category) {
        store.create_category(category).unwrap();
    }

    fn create_item(store: &Store, text: &str) -> Item {
        let item = Item::new(text.to_string());
        store.create_item(&item).unwrap();
        item
    }

    fn create_item_with_created_at(store: &Store, text: &str, created_at: Timestamp) -> Item {
        let mut item = Item::new(text.to_string());
        item.created_at = created_at;
        item.modified_at = created_at;
        store.create_item(&item).unwrap();
        item
    }

    fn set_item_text(store: &Store, item_id: ItemId, text: &str) {
        let mut item = store.get_item(item_id).unwrap();
        item.text = text.to_string();
        item.modified_at = Timestamp::now();
        store.update_item(&item).unwrap();
    }

    fn set_item_note(store: &Store, item_id: ItemId, note: Option<&str>) {
        let mut item = store.get_item(item_id).unwrap();
        item.note = note.map(str::to_string);
        item.modified_at = Timestamp::now();
        store.update_item(&item).unwrap();
    }

    fn manual_assignment() -> Assignment {
        Assignment {
            source: AssignmentSource::Manual,
            assigned_at: Timestamp::now(),
            sticky: true,
            origin: Some("manual:test".to_string()),
            explanation: None,
            numeric_value: None,
        }
    }

    fn category(name: &str, implicit: bool) -> Category {
        let mut category = Category::new(name.to_string());
        category.enable_implicit_string = implicit;
        category
    }

    fn category_with_profile(
        name: &str,
        include: &[CategoryId],
        exclude: &[CategoryId],
    ) -> Category {
        let mut category = category(name, false);
        let mut criteria = Query::default();
        for &id in include {
            criteria.set_criterion(CriterionMode::And, id);
        }
        for &id in exclude {
            criteria.set_criterion(CriterionMode::Not, id);
        }
        category.conditions.push(Condition::Profile {
            criteria: Box::new(criteria),
        });
        category
    }

    fn category_with_date(name: &str, source: DateSource, matcher: DateMatcher) -> Category {
        let mut category = category(name, false);
        category
            .conditions
            .push(Condition::Date { source, matcher });
        category
    }

    fn child_category(name: &str, parent: CategoryId, implicit: bool) -> Category {
        let mut category = category(name, implicit);
        category.parent = Some(parent);
        category
    }

    fn set_item_when(store: &Store, item_id: ItemId, when_date: Option<jiff::civil::DateTime>) {
        let mut item = store.get_item(item_id).unwrap();
        item.when_date = when_date;
        item.modified_at = Timestamp::now();
        store.update_item(&item).unwrap();
    }

    fn set_item_done(store: &Store, item_id: ItemId, done_date: Option<jiff::civil::DateTime>) {
        let mut item = store.get_item(item_id).unwrap();
        item.done_date = done_date;
        item.is_done = done_date.is_some();
        item.modified_at = Timestamp::now();
        store.update_item(&item).unwrap();
    }

    fn evaluation_context_at(zoned: &str) -> EvaluationContext {
        EvaluationContext::from_zoned(zoned.parse::<Zoned>().unwrap())
    }

    #[test]
    fn process_item_single_pass_convergence() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let sarah = Category::new("Sarah".to_string());
        create_category(&store, &sarah);

        let item = create_item(&store, "Call Sarah tomorrow");
        let result = process_item(&store, &classifier, item.id).unwrap();

        assert!(result.new_assignments.contains(&sarah.id));
        assert!(result.deferred_removals.is_empty());
    }

    #[test]
    fn process_item_implicit_match_uses_note_text() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let sarah = Category::new("Sarah".to_string());
        create_category(&store, &sarah);

        let item = create_item(&store, "Call someone tomorrow");
        set_item_note(&store, item.id, Some("Follow up with Sarah after lunch"));

        let result = process_item(&store, &classifier, item.id).unwrap();
        assert!(result.new_assignments.contains(&sarah.id));

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert_eq!(
            assignments.get(&sarah.id).unwrap().source,
            AssignmentSource::AutoMatch
        );
    }

    #[test]
    fn process_item_two_pass_cascade_assign_action() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let calendar = category("Calendar", false);
        create_category(&store, &calendar);

        let mut meetings = Category::new("Meetings".to_string());
        meetings.actions.push(Action::Assign {
            targets: HashSet::from([calendar.id]),
        });
        create_category(&store, &meetings);

        let item = create_item(&store, "Meetings with design");
        let result = process_item(&store, &classifier, item.id).unwrap();

        assert!(result.new_assignments.contains(&meetings.id));
        assert!(result.new_assignments.contains(&calendar.id));

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert_eq!(
            assignments.get(&meetings.id).unwrap().source,
            AssignmentSource::AutoMatch
        );
        assert_eq!(
            assignments.get(&calendar.id).unwrap().source,
            AssignmentSource::Action
        );
    }

    #[test]
    fn process_item_profile_cascade_across_passes() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let calendar = category("Calendar", false);
        let reminders = category_with_profile("Reminders", &[calendar.id], &[]);

        create_category(&store, &reminders);
        create_category(&store, &calendar);

        let mut meetings = Category::new("Meetings".to_string());
        meetings.actions.push(Action::Assign {
            targets: HashSet::from([calendar.id]),
        });
        create_category(&store, &meetings);

        let item = create_item(&store, "Meetings tomorrow");
        let result = process_item(&store, &classifier, item.id).unwrap();

        assert!(result.new_assignments.contains(&meetings.id));
        assert!(result.new_assignments.contains(&calendar.id));
        assert!(result.new_assignments.contains(&reminders.id));

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&meetings.id));
        assert!(assignments.contains_key(&calendar.id));
        assert!(assignments.contains_key(&reminders.id));
    }

    #[test]
    fn process_item_with_options_can_disable_implicit_matching_but_keep_profile_cascades() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let calendar = category("Calendar", false);
        create_category(&store, &calendar);
        let reminders = category_with_profile("Reminders", &[calendar.id], &[]);
        create_category(&store, &reminders);

        let item = create_item(&store, "Calendar");
        store
            .assign_item(item.id, calendar.id, &manual_assignment())
            .unwrap();

        let result = process_item_with_options(
            &store,
            &classifier,
            item.id,
            ProcessOptions {
                enable_implicit_string: false,
                evaluation_context: EvaluationContext::now(),
            },
        )
        .unwrap();

        assert!(result.new_assignments.contains(&reminders.id));
        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&calendar.id));
        assert!(assignments.contains_key(&reminders.id));
    }

    #[test]
    fn process_item_date_condition_when_before_today_matches() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let overdue = category_with_date(
            "Overdue",
            DateSource::When,
            DateMatcher::Compare {
                op: DateCompareOp::Before,
                value: DateValueExpr::Today,
            },
        );
        create_category(&store, &overdue);

        let item = create_item(&store, "Finish report");
        set_item_when(
            &store,
            item.id,
            Some(jiff::civil::DateTime::new(2026, 2, 15, 9, 0, 0, 0).unwrap()),
        );

        let result = process_item_with_options(
            &store,
            &classifier,
            item.id,
            ProcessOptions {
                enable_implicit_string: true,
                evaluation_context: EvaluationContext::for_date(
                    jiff::civil::Date::new(2026, 2, 16).unwrap(),
                ),
            },
        )
        .unwrap();

        assert!(result.new_assignments.contains(&overdue.id));
        let assignments = store.get_assignments_for_item(item.id).unwrap();
        let assignment = assignments.get(&overdue.id).unwrap();
        assert_eq!(assignment.source, AssignmentSource::AutoMatch);
        assert!(matches!(
            assignment.explanation,
            Some(AssignmentExplanation::DateCondition { .. })
        ));
    }

    #[test]
    fn process_item_date_condition_entry_uses_evaluation_timezone() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let inbox = category_with_date(
            "Inbox",
            DateSource::Entry,
            DateMatcher::Compare {
                op: DateCompareOp::On,
                value: DateValueExpr::DaysAgo(1),
            },
        );
        create_category(&store, &inbox);

        let item = create_item_with_created_at(
            &store,
            "Capture idea",
            "2026-02-16T07:30:00Z".parse::<Timestamp>().unwrap(),
        );

        let result = process_item_with_options(
            &store,
            &classifier,
            item.id,
            ProcessOptions {
                enable_implicit_string: true,
                evaluation_context: evaluation_context_at(
                    "2026-02-16T12:00:00-08:00[America/Los_Angeles]",
                ),
            },
        )
        .unwrap();

        assert!(result.new_assignments.contains(&inbox.id));
    }

    #[test]
    fn process_item_date_condition_done_recent_window_matches() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let wins = category_with_date(
            "Wins",
            DateSource::Done,
            DateMatcher::Compare {
                op: DateCompareOp::AtOrAfter,
                value: DateValueExpr::DaysAgo(2),
            },
        );
        create_category(&store, &wins);

        let item = create_item(&store, "Ship feature");
        set_item_done(
            &store,
            item.id,
            Some(jiff::civil::DateTime::new(2026, 2, 15, 14, 0, 0, 0).unwrap()),
        );

        let result = process_item_with_options(
            &store,
            &classifier,
            item.id,
            ProcessOptions {
                enable_implicit_string: true,
                evaluation_context: EvaluationContext::for_date(
                    jiff::civil::Date::new(2026, 2, 16).unwrap(),
                ),
            },
        )
        .unwrap();

        assert!(result.new_assignments.contains(&wins.id));
    }

    #[test]
    fn process_item_subsumption_assigns_ancestors() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let projects = category("Projects", false);
        create_category(&store, &projects);

        let alpha = child_category("Project Alpha", projects.id, true);
        create_category(&store, &alpha);

        let item = create_item(&store, "Project Alpha kickoff");
        let result = process_item(&store, &classifier, item.id).unwrap();

        assert!(result.new_assignments.contains(&alpha.id));
        assert!(!result.new_assignments.contains(&projects.id));

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert_eq!(
            assignments.get(&alpha.id).unwrap().source,
            AssignmentSource::AutoMatch
        );
        assert_eq!(
            assignments.get(&projects.id).unwrap().source,
            AssignmentSource::Subsumption
        );
        assert_eq!(
            assignments.get(&projects.id).unwrap().origin.as_deref(),
            Some("subsumption:Project Alpha")
        );
    }

    #[test]
    fn process_item_subsumption_walks_multi_level_parent_chain() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let work = category("Work", false);
        create_category(&store, &work);

        let projects = child_category("Projects", work.id, false);
        create_category(&store, &projects);

        let alpha = child_category("Project Alpha", projects.id, true);
        create_category(&store, &alpha);

        let item = create_item(&store, "Project Alpha sync");
        process_item(&store, &classifier, item.id).unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert_eq!(
            assignments.get(&projects.id).unwrap().source,
            AssignmentSource::Subsumption
        );
        assert_eq!(
            assignments.get(&work.id).unwrap().source,
            AssignmentSource::Subsumption
        );
    }

    #[test]
    fn process_item_subsumption_does_not_overwrite_existing_assignment() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let projects = category("Projects", false);
        create_category(&store, &projects);

        let alpha = child_category("Project Alpha", projects.id, true);
        create_category(&store, &alpha);

        let item = create_item(&store, "Project Alpha backlog");
        store
            .assign_item(item.id, projects.id, &manual_assignment())
            .unwrap();

        process_item(&store, &classifier, item.id).unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert_eq!(
            assignments.get(&projects.id).unwrap().source,
            AssignmentSource::Manual
        );
        assert_eq!(
            assignments.get(&projects.id).unwrap().origin.as_deref(),
            Some("manual:test")
        );
        assert_eq!(
            assignments.get(&alpha.id).unwrap().source,
            AssignmentSource::AutoMatch
        );
    }

    #[test]
    fn process_item_subsumption_does_not_fire_ancestor_actions() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let dashboard = category("Dashboard", false);
        create_category(&store, &dashboard);

        let mut projects = category("Projects", false);
        projects.actions.push(Action::Assign {
            targets: HashSet::from([dashboard.id]),
        });
        create_category(&store, &projects);

        let alpha = child_category("Project Alpha", projects.id, true);
        create_category(&store, &alpha);

        let item = create_item(&store, "Project Alpha review");
        let result = process_item(&store, &classifier, item.id).unwrap();

        assert!(!result.new_assignments.contains(&dashboard.id));

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&alpha.id));
        assert!(assignments.contains_key(&projects.id));
        assert!(!assignments.contains_key(&dashboard.id));
    }

    #[test]
    fn hierarchy_pass_subsumption_not_counted_as_new_assignment() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let projects = category("Projects", false);
        create_category(&store, &projects);

        let alpha = child_category("Project Alpha", projects.id, true);
        create_category(&store, &alpha);

        let item = create_item(&store, "Project Alpha planning");
        let categories = store.get_hierarchy().unwrap();
        let categories_by_id: HashMap<CategoryId, &Category> = categories
            .iter()
            .map(|category| (category.id, category))
            .collect();
        let mut assignments = HashMap::new();
        let mut seen_pairs = HashSet::new();
        let pass_input = HierarchyPassInput {
            classifier: &classifier,
            item: &item,
            item_id: item.id,
            item_text: &item.text,
            categories: &categories,
            categories_by_id: &categories_by_id,
            original_assignments: &HashMap::new(),
            options: ProcessOptions::default(),
        };

        let pass_result =
            run_hierarchy_pass(&pass_input, &mut assignments, &mut seen_pairs).unwrap();

        assert!(pass_result.new_assignments.contains(&alpha.id));
        assert!(!pass_result.new_assignments.contains(&projects.id));
        assert_eq!(
            assignments.get(&projects.id).unwrap().source,
            AssignmentSource::Subsumption
        );
    }

    #[test]
    fn process_item_action_assignment_triggers_subsumption() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let events = category("Events", false);
        create_category(&store, &events);

        let calendar = child_category("Calendar", events.id, false);
        create_category(&store, &calendar);

        let mut meetings = category("Meetings", true);
        meetings.actions.push(Action::Assign {
            targets: HashSet::from([calendar.id]),
        });
        create_category(&store, &meetings);

        let item = create_item(&store, "Meetings tomorrow");
        let result = process_item(&store, &classifier, item.id).unwrap();

        assert!(result.new_assignments.contains(&meetings.id));
        assert!(result.new_assignments.contains(&calendar.id));
        assert!(!result.new_assignments.contains(&events.id));

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert_eq!(
            assignments.get(&meetings.id).unwrap().source,
            AssignmentSource::AutoMatch
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
    fn process_item_mutual_exclusion_basic_switch_between_siblings() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let mut status = category("Status", false);
        status.is_exclusive = true;
        create_category(&store, &status);

        let todo = child_category("Todo", status.id, true);
        create_category(&store, &todo);

        let in_progress = child_category("InProgress", status.id, true);
        create_category(&store, &in_progress);

        let item = create_item(&store, "Todo");
        process_item(&store, &classifier, item.id).unwrap();

        set_item_text(&store, item.id, "InProgress");
        process_item(&store, &classifier, item.id).unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&in_progress.id));
        assert!(!assignments.contains_key(&todo.id));
    }

    #[test]
    fn process_item_mutual_exclusion_non_exclusive_parent_keeps_siblings() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let tags = category("Tags", false);
        create_category(&store, &tags);

        let urgent = child_category("Urgent", tags.id, true);
        create_category(&store, &urgent);

        let important = child_category("Important", tags.id, true);
        create_category(&store, &important);

        let item = create_item(&store, "Urgent and Important");
        process_item(&store, &classifier, item.id).unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&urgent.id));
        assert!(assignments.contains_key(&important.id));
    }

    #[test]
    fn process_item_mutual_exclusion_engine_match_leaves_one_child() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let mut priority = category("Priority", false);
        priority.is_exclusive = true;
        create_category(&store, &priority);

        let high = child_category("High", priority.id, true);
        create_category(&store, &high);

        let low = child_category("Low", priority.id, true);
        create_category(&store, &low);

        let item = create_item(&store, "High priority and Low cost");
        process_item(&store, &classifier, item.id).unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        let high_assigned = assignments.contains_key(&high.id);
        let low_assigned = assignments.contains_key(&low.id);
        assert_ne!(high_assigned, low_assigned);
    }

    #[test]
    fn process_item_mutual_exclusion_applies_to_action_assignments() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let mut status = category("Status", false);
        status.is_exclusive = true;
        create_category(&store, &status);

        let todo = child_category("Todo", status.id, true);
        create_category(&store, &todo);

        let in_progress = child_category("InProgress", status.id, false);
        create_category(&store, &in_progress);

        let mut workflow = category("Workflow", true);
        workflow.actions.push(Action::Assign {
            targets: HashSet::from([in_progress.id]),
        });
        create_category(&store, &workflow);

        let item = create_item(&store, "Todo Workflow");
        process_item(&store, &classifier, item.id).unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(
            assignments.contains_key(&todo.id),
            "earlier child should keep precedence over later action-derived sibling"
        );
        assert!(
            !assignments.contains_key(&in_progress.id),
            "later action-derived sibling should be suppressed inside the exclusive family"
        );
    }

    #[test]
    fn process_item_mutual_exclusion_keeps_manual_source_over_action_assignment() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let mut status = category("Status", false);
        status.is_exclusive = true;
        create_category(&store, &status);

        let todo = child_category("Todo", status.id, false);
        create_category(&store, &todo);

        let in_progress = child_category("InProgress", status.id, false);
        create_category(&store, &in_progress);

        let mut workflow = category("Workflow", true);
        workflow.actions.push(Action::Assign {
            targets: HashSet::from([in_progress.id]),
        });
        create_category(&store, &workflow);

        let item = create_item(&store, "Workflow item");
        store
            .assign_item(item.id, todo.id, &manual_assignment())
            .unwrap();

        process_item(&store, &classifier, item.id).unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(
            assignments.contains_key(&todo.id),
            "manual exclusive choice should survive later derived action assignments"
        );
        assert!(
            !assignments.contains_key(&in_progress.id),
            "derived action should not replace an existing manual exclusive choice"
        );
    }

    #[test]
    fn process_item_implicit_match_does_not_override_manual_exclusive_sibling() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let mut status = category("Status", false);
        status.is_exclusive = true;
        create_category(&store, &status);

        let ready = child_category("Ready", status.id, true);
        create_category(&store, &ready);

        let complete = child_category("Complete", status.id, false);
        create_category(&store, &complete);

        let item = create_item(&store, "Close docs example");
        set_item_note(
            &store,
            item.id,
            Some("The note includes aglet list --category Ready."),
        );
        store
            .assign_item(item.id, complete.id, &manual_assignment())
            .unwrap();

        process_item(&store, &classifier, item.id).unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert_eq!(
            assignments
                .get(&complete.id)
                .map(|assignment| assignment.source),
            Some(AssignmentSource::Manual),
            "manual exclusive choice should survive implicit-string matches"
        );
        assert!(
            !assignments.contains_key(&ready.id),
            "implicit-string sibling should not be re-added over manual status"
        );
    }

    #[test]
    fn process_item_mutual_exclusion_keeps_manual_choice_over_implicit_sibling() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let mut priority = category("Priority", false);
        priority.is_exclusive = true;
        create_category(&store, &priority);

        let low = child_category("Low", priority.id, false);
        create_category(&store, &low);

        let medium = child_category("Medium", priority.id, false);
        create_category(&store, &medium);

        let high = child_category("High", priority.id, true);
        create_category(&store, &high);

        let item = create_item(&store, "High impact");
        store
            .assign_item(item.id, low.id, &manual_assignment())
            .unwrap();

        process_item(&store, &classifier, item.id).unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert_eq!(
            assignments.get(&low.id).map(|assignment| assignment.source),
            Some(AssignmentSource::Manual)
        );
        assert!(!assignments.contains_key(&medium.id));
        assert!(
            !assignments.contains_key(&high.id),
            "implicit-string sibling should not override the manual choice"
        );
    }

    #[test]
    fn process_item_exclusive_family_prefers_earlier_profile_match() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let tui = category("TUI", true);
        create_category(&store, &tui);

        let mut priority = category("Priority", false);
        priority.is_exclusive = true;
        create_category(&store, &priority);

        let mut high = child_category("High", priority.id, false);
        let mut high_criteria = Query::default();
        high_criteria.set_criterion(CriterionMode::And, tui.id);
        high.conditions.push(Condition::Profile {
            criteria: Box::new(high_criteria),
        });
        create_category(&store, &high);

        let mut low = child_category("Low", priority.id, false);
        let mut low_criteria = Query::default();
        low_criteria.set_criterion(CriterionMode::And, tui.id);
        low.conditions.push(Condition::Profile {
            criteria: Box::new(low_criteria),
        });
        create_category(&store, &low);

        let item = create_item(&store, "TUI task");
        process_item(&store, &classifier, item.id).unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&tui.id));
        assert!(
            assignments.contains_key(&high.id),
            "earlier matching child should win inside the exclusive family"
        );
        assert!(
            !assignments.contains_key(&low.id),
            "later matching child should be suppressed"
        );
    }

    #[test]
    fn process_item_exclusive_family_prefers_earlier_action_target_over_later_condition() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let mut tui = category("TUI", true);
        create_category(&store, &tui);

        let mut priority = category("Priority", false);
        priority.is_exclusive = true;
        create_category(&store, &priority);

        let high = child_category("High", priority.id, false);
        create_category(&store, &high);

        let mut low = child_category("Low", priority.id, false);
        let mut low_criteria = Query::default();
        low_criteria.set_criterion(CriterionMode::And, tui.id);
        low.conditions.push(Condition::Profile {
            criteria: Box::new(low_criteria),
        });
        create_category(&store, &low);

        tui.actions.push(Action::Assign {
            targets: HashSet::from([high.id]),
        });
        store.update_category(&tui).unwrap();

        let item = create_item(&store, "TUI task");
        process_item(&store, &classifier, item.id).unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&tui.id));
        assert!(
            assignments.contains_key(&high.id),
            "earlier action-derived child should keep precedence"
        );
        assert!(
            !assignments.contains_key(&low.id),
            "later condition-derived sibling should be suppressed"
        );
    }

    #[test]
    fn evaluate_all_items_basic_retroactive_match() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let sarah = category("Sarah", true);
        create_category(&store, &sarah);

        let contains = create_item(&store, "Lunch with Sarah");
        let not_contains = create_item(&store, "Lunch with Alex");

        let result = evaluate_all_items(&store, &classifier, sarah.id).unwrap();
        assert_eq!(result.processed_items, 2);
        assert_eq!(result.affected_items, 1);

        let contains_assignments = store.get_assignments_for_item(contains.id).unwrap();
        let not_contains_assignments = store.get_assignments_for_item(not_contains.id).unwrap();

        assert!(contains_assignments.contains_key(&sarah.id));
        assert!(!not_contains_assignments.contains_key(&sarah.id));
    }

    #[test]
    fn evaluate_all_items_no_double_assignment() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let sarah = category("Sarah", true);
        create_category(&store, &sarah);

        let item = create_item(&store, "Sarah meeting");
        process_item(&store, &classifier, item.id).unwrap();

        let result = evaluate_all_items(&store, &classifier, sarah.id).unwrap();
        assert_eq!(result.processed_items, 1);
        assert_eq!(result.total_new_assignments, 0);

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert_eq!(assignments.len(), 1);
        assert!(assignments.contains_key(&sarah.id));
    }

    #[test]
    fn evaluate_all_items_with_actions() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let calendar = category("Calendar", false);
        create_category(&store, &calendar);

        let mut meetings = category("Meetings", true);
        meetings.actions.push(Action::Assign {
            targets: HashSet::from([calendar.id]),
        });
        create_category(&store, &meetings);

        let first = create_item(&store, "Meetings with team");
        let second = create_item(&store, "Plan meetings workspace");
        let third = create_item(&store, "Buy groceries");

        let result = evaluate_all_items(&store, &classifier, meetings.id).unwrap();
        assert_eq!(result.processed_items, 3);
        assert_eq!(result.affected_items, 2);

        let first_assignments = store.get_assignments_for_item(first.id).unwrap();
        let second_assignments = store.get_assignments_for_item(second.id).unwrap();
        let third_assignments = store.get_assignments_for_item(third.id).unwrap();

        assert!(first_assignments.contains_key(&meetings.id));
        assert!(first_assignments.contains_key(&calendar.id));
        assert!(second_assignments.contains_key(&meetings.id));
        assert!(second_assignments.contains_key(&calendar.id));
        assert!(!third_assignments.contains_key(&meetings.id));
        assert!(!third_assignments.contains_key(&calendar.id));
    }

    #[test]
    fn evaluate_all_items_with_subsumption() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let projects = category("Projects", false);
        create_category(&store, &projects);

        let alpha = child_category("Project Alpha", projects.id, true);
        create_category(&store, &alpha);

        let first = create_item(&store, "Project Alpha kickoff");
        let second = create_item(&store, "General note");

        let result = evaluate_all_items(&store, &classifier, alpha.id).unwrap();
        assert_eq!(result.processed_items, 2);
        assert_eq!(result.affected_items, 1);

        let first_assignments = store.get_assignments_for_item(first.id).unwrap();
        let second_assignments = store.get_assignments_for_item(second.id).unwrap();

        assert!(first_assignments.contains_key(&alpha.id));
        assert!(first_assignments.contains_key(&projects.id));
        assert!(!second_assignments.contains_key(&alpha.id));
        assert!(!second_assignments.contains_key(&projects.id));
    }

    #[test]
    fn evaluate_all_items_with_mutual_exclusion() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let mut status = category("Status", false);
        status.is_exclusive = true;
        create_category(&store, &status);

        let todo = child_category("Todo", status.id, false);
        create_category(&store, &todo);

        let in_progress = child_category("InProgress", status.id, false);
        create_category(&store, &in_progress);

        let mut workflow = category("Workflow", true);
        workflow.actions.push(Action::Assign {
            targets: HashSet::from([in_progress.id]),
        });
        create_category(&store, &workflow);

        let item = create_item(&store, "Workflow item");
        store
            .assign_item(item.id, todo.id, &manual_assignment())
            .unwrap();

        let result = evaluate_all_items(&store, &classifier, workflow.id).unwrap();
        assert_eq!(result.processed_items, 1);
        assert_eq!(result.affected_items, 1);

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(
            assignments.contains_key(&todo.id),
            "manual exclusive choice should survive during bulk evaluation too"
        );
        assert!(
            !assignments.contains_key(&in_progress.id),
            "bulk evaluation should suppress later action-derived siblings in an exclusive family"
        );
    }

    #[test]
    fn evaluate_all_items_empty_store() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let category = category("Anything", true);
        create_category(&store, &category);

        let result = evaluate_all_items(&store, &classifier, category.id).unwrap();
        assert_eq!(result.processed_items, 0);
        assert_eq!(result.affected_items, 0);
        assert_eq!(result.total_new_assignments, 0);
        assert_eq!(result.total_deferred_removals, 0);
    }

    #[test]
    fn evaluate_all_items_idempotent_re_run() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let sarah = category("Sarah", true);
        create_category(&store, &sarah);

        create_item(&store, "Sarah ping");
        create_item(&store, "No match here");

        let first = evaluate_all_items(&store, &classifier, sarah.id).unwrap();
        assert_eq!(first.processed_items, 2);
        assert_eq!(first.affected_items, 1);
        assert!(first.total_new_assignments > 0);

        let second = evaluate_all_items(&store, &classifier, sarah.id).unwrap();
        assert_eq!(second.processed_items, 2);
        assert_eq!(second.affected_items, 0);
        assert_eq!(second.total_new_assignments, 0);
        assert_eq!(second.total_deferred_removals, 0);
    }

    #[test]
    fn process_item_max_passes_cap_returns_error() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let mut stages = Vec::new();
        for index in 1..=11 {
            let stage = category(&format!("Stage{index}"), false);
            create_category(&store, &stage);
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

        let mut trigger = Category::new("Trigger".to_string());
        trigger.actions.push(Action::Assign {
            targets: HashSet::from([stages[10].id]),
        });
        create_category(&store, &trigger);

        let item = create_item(&store, "Trigger this chain");

        let err = process_item(&store, &classifier, item.id).unwrap_err();
        match err {
            AgletError::InvalidOperation { message } => {
                assert!(message.contains("exceeded 10 passes"));
            }
            other => panic!("unexpected error: {other:?}"),
        }

        // With savepoint rollback, cap-exceeded errors should not leave writes behind.
        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.is_empty());
    }

    #[test]
    fn process_item_cycle_detection_converges() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let b = category("B", false);
        create_category(&store, &b);

        let mut a = Category::new("A".to_string());
        a.actions.push(Action::Assign {
            targets: HashSet::from([b.id]),
        });
        create_category(&store, &a);

        let mut b_updated = store.get_category(b.id).unwrap();
        b_updated.actions.push(Action::Assign {
            targets: HashSet::from([a.id]),
        });
        store.update_category(&b_updated).unwrap();

        let item = create_item(&store, "A");
        let result = process_item(&store, &classifier, item.id).unwrap();

        assert!(result.new_assignments.contains(&a.id));
        assert!(result.new_assignments.contains(&b.id));

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&a.id));
        assert!(assignments.contains_key(&b.id));
    }

    #[test]
    fn process_item_deferred_removes_applied_after_loop() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let backlog = category("Backlog", false);
        create_category(&store, &backlog);

        let mut active = Category::new("Active".to_string());
        active.actions.push(Action::Remove {
            targets: HashSet::from([backlog.id]),
        });
        create_category(&store, &active);

        let item = create_item(&store, "Active work");
        store
            .assign_item(item.id, backlog.id, &manual_assignment())
            .unwrap();

        let result = process_item(&store, &classifier, item.id).unwrap();

        assert!(result.new_assignments.contains(&active.id));
        assert!(result
            .deferred_removals
            .iter()
            .any(|removal| removal.target == backlog.id));

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&active.id));
        assert!(!assignments.contains_key(&backlog.id));
    }

    #[test]
    fn process_item_remove_applies_regardless_of_source() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let projects = category("Projects", false);
        create_category(&store, &projects);

        let mut cleanup = Category::new("Cleanup".to_string());
        cleanup.actions.push(Action::Remove {
            targets: HashSet::from([projects.id]),
        });
        create_category(&store, &cleanup);

        let item = create_item(&store, "Cleanup this");
        store
            .assign_item(item.id, projects.id, &manual_assignment())
            .unwrap();

        process_item(&store, &classifier, item.id).unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(!assignments.contains_key(&projects.id));
    }

    #[test]
    fn process_item_deferred_removes_do_not_trigger_re_evaluation() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let backlog = category("Backlog", false);
        create_category(&store, &backlog);

        let cleared = category_with_profile("Cleared", &[], &[backlog.id]);
        create_category(&store, &cleared);

        let mut active = Category::new("Active".to_string());
        active.actions.push(Action::Remove {
            targets: HashSet::from([backlog.id]),
        });
        create_category(&store, &active);

        let item = create_item(&store, "Active item");
        store
            .assign_item(item.id, backlog.id, &manual_assignment())
            .unwrap();

        process_item(&store, &classifier, item.id).unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(!assignments.contains_key(&backlog.id));
        assert!(!assignments.contains_key(&cleared.id));
    }

    #[test]
    fn process_item_idempotent_re_run() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let calendar = category("Calendar", false);
        create_category(&store, &calendar);

        let mut meetings = Category::new("Meetings".to_string());
        meetings.actions.push(Action::Assign {
            targets: HashSet::from([calendar.id]),
        });
        create_category(&store, &meetings);

        let item = create_item(&store, "Meetings later today");

        let first = process_item(&store, &classifier, item.id).unwrap();
        assert!(!first.new_assignments.is_empty());

        let second = process_item(&store, &classifier, item.id).unwrap();
        assert!(second.new_assignments.is_empty());
        assert!(second.deferred_removals.is_empty());
    }

    // ── run_in_savepoint ───────────────────────────────────────────────────────

    #[test]
    fn run_in_savepoint_rolls_back_db_mutations_on_error() {
        use super::run_in_savepoint;

        let store = Store::open_memory().unwrap();
        let tag = category("Tag", false);
        create_category(&store, &tag);
        let item = create_item(&store, "some task");

        // Run a closure that assigns the item to `tag` and then returns an
        // error.  The savepoint must roll back so the assignment is NOT
        // persisted.
        let result: Result<(), AgletError> = run_in_savepoint(&store, || {
            store
                .assign_item(item.id, tag.id, &manual_assignment())
                .unwrap();
            Err(AgletError::InvalidOperation {
                message: "deliberate test error".to_string(),
            })
        });

        assert!(result.is_err(), "closure error should propagate");

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(
            !assignments.contains_key(&tag.id),
            "assignment made inside the failing closure must be rolled back"
        );
    }

    #[test]
    fn run_in_savepoint_commits_db_mutations_on_success() {
        use super::run_in_savepoint;

        let store = Store::open_memory().unwrap();
        let tag = category("Tag", false);
        create_category(&store, &tag);
        let item = create_item(&store, "some task");

        let result: Result<(), AgletError> = run_in_savepoint(&store, || {
            store
                .assign_item(item.id, tag.id, &manual_assignment())
                .unwrap();
            Ok(())
        });

        assert!(result.is_ok(), "successful closure should commit");

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(
            assignments.contains_key(&tag.id),
            "assignment made inside a successful savepoint should be persisted"
        );
    }

    // ---------------------------------------------------------------
    // Profile condition test scenarios (brainstormed from Beeswax spec)
    // ---------------------------------------------------------------

    /// Helper to create a category with profile condition including OR criteria.
    fn category_with_profile_or(
        name: &str,
        include: &[CategoryId],
        exclude: &[CategoryId],
        or: &[CategoryId],
    ) -> Category {
        let mut cat = category(name, false);
        let mut criteria = Query::default();
        for &id in include {
            criteria.set_criterion(CriterionMode::And, id);
        }
        for &id in exclude {
            criteria.set_criterion(CriterionMode::Not, id);
        }
        for &id in or {
            criteria.set_criterion(CriterionMode::Or, id);
        }
        cat.conditions.push(Condition::Profile {
            criteria: Box::new(criteria),
        });
        cat
    }

    #[test]
    fn profile_single_criterion_delegation() {
        // "If Project A → assign to Mary"
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let project_a = category("Project A", true);
        create_category(&store, &project_a);
        let mary = category_with_profile("Mary", &[project_a.id], &[]);
        create_category(&store, &mary);

        let item = create_item(&store, "Review the Project A docs");
        let result = process_item(&store, &classifier, item.id).unwrap();

        assert!(result.new_assignments.contains(&project_a.id));
        assert!(result.new_assignments.contains(&mary.id));

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert_eq!(
            assignments.get(&mary.id).unwrap().origin.as_deref(),
            Some("profile:Mary")
        );
    }

    #[test]
    fn profile_compound_and_trigger() {
        // "If Urgent AND Project Alpha → assign to Escalated"
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let urgent = category("Urgent", true);
        let project_alpha = category("Project Alpha", true);
        let escalated = category_with_profile("Escalated", &[urgent.id, project_alpha.id], &[]);
        create_category(&store, &urgent);
        create_category(&store, &project_alpha);
        create_category(&store, &escalated);

        let item = create_item(&store, "Urgent Project Alpha deploy is failing");
        let result = process_item(&store, &classifier, item.id).unwrap();

        assert!(result.new_assignments.contains(&urgent.id));
        assert!(result.new_assignments.contains(&project_alpha.id));
        assert!(result.new_assignments.contains(&escalated.id));
    }

    #[test]
    fn profile_partial_match_does_not_fire() {
        // Item in Urgent but NOT Project Alpha → Escalated should NOT fire
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let urgent = category("Urgent", true);
        let project_alpha = category("Project Alpha", true);
        let escalated = category_with_profile("Escalated", &[urgent.id, project_alpha.id], &[]);
        create_category(&store, &urgent);
        create_category(&store, &project_alpha);
        create_category(&store, &escalated);

        let item = create_item(&store, "Urgent fix the login page");
        let result = process_item(&store, &classifier, item.id).unwrap();

        assert!(result.new_assignments.contains(&urgent.id));
        assert!(!result.new_assignments.contains(&project_alpha.id));
        assert!(!result.new_assignments.contains(&escalated.id));
    }

    #[test]
    fn profile_exclude_pattern() {
        // "If Work AND NOT Delegated → assign to My-Tasks"
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let work = category("Work", true);
        let delegated = category("Delegated", false);
        let my_tasks = category_with_profile("My-Tasks", &[work.id], &[delegated.id]);
        create_category(&store, &work);
        create_category(&store, &delegated);
        create_category(&store, &my_tasks);

        // Item with "Work" but not "Delegated" → should match
        let item = create_item(&store, "Work on the API refactor");
        let result = process_item(&store, &classifier, item.id).unwrap();

        assert!(result.new_assignments.contains(&work.id));
        assert!(result.new_assignments.contains(&my_tasks.id));
    }

    #[test]
    fn profile_exclude_blocks_when_present() {
        // "If Work AND NOT Delegated → assign to My-Tasks" — but item IS Delegated
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let work = category("Work", true);
        let delegated = category("Delegated", false);
        let my_tasks = category_with_profile("My-Tasks", &[work.id], &[delegated.id]);
        create_category(&store, &work);
        create_category(&store, &delegated);
        create_category(&store, &my_tasks);

        let item = create_item(&store, "Work stuff");
        store
            .assign_item(item.id, delegated.id, &manual_assignment())
            .unwrap();
        let result = process_item(&store, &classifier, item.id).unwrap();

        assert!(!result.new_assignments.contains(&my_tasks.id));
    }

    #[test]
    fn profile_cascading_chain() {
        // Escalated → Notify:Manager (cascading two profile conditions)
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let urgent = category("Urgent", true);
        let project_alpha = category("Project Alpha", true);
        let escalated = category_with_profile("Escalated", &[urgent.id, project_alpha.id], &[]);
        let notify = category_with_profile("Notify-Manager", &[escalated.id], &[]);
        create_category(&store, &urgent);
        create_category(&store, &project_alpha);
        create_category(&store, &escalated);
        create_category(&store, &notify);

        let item = create_item(&store, "Urgent Project Alpha outage");
        let result = process_item(&store, &classifier, item.id).unwrap();

        assert!(result.new_assignments.contains(&escalated.id));
        assert!(result.new_assignments.contains(&notify.id));
    }

    #[test]
    fn profile_late_satisfaction() {
        // Assign Urgent first (no fire), then assign Project Alpha → rule fires
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let urgent = category("Urgent", false);
        let project_alpha = category("Project Alpha", false);
        let escalated = category_with_profile("Escalated", &[urgent.id, project_alpha.id], &[]);
        create_category(&store, &urgent);
        create_category(&store, &project_alpha);
        create_category(&store, &escalated);

        let item = create_item(&store, "some neutral text");

        // Manual assign Urgent only — Escalated should NOT fire
        store
            .assign_item(item.id, urgent.id, &manual_assignment())
            .unwrap();
        let result = process_item(&store, &classifier, item.id).unwrap();
        assert!(!result.new_assignments.contains(&escalated.id));

        // Now also assign Project Alpha — Escalated should fire
        store
            .assign_item(item.id, project_alpha.id, &manual_assignment())
            .unwrap();
        let result = process_item(&store, &classifier, item.id).unwrap();
        assert!(result.new_assignments.contains(&escalated.id));
    }

    #[test]
    fn profile_order_independence() {
        // Assign B then A vs A then B → same result
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let cat_a = category("CatA", false);
        let cat_b = category("CatB", false);
        let target = category_with_profile("Target", &[cat_a.id, cat_b.id], &[]);
        create_category(&store, &cat_a);
        create_category(&store, &cat_b);
        create_category(&store, &target);

        // Order 1: A then B
        let item1 = create_item(&store, "neutral text one");
        store
            .assign_item(item1.id, cat_a.id, &manual_assignment())
            .unwrap();
        store
            .assign_item(item1.id, cat_b.id, &manual_assignment())
            .unwrap();
        let result1 = process_item(&store, &classifier, item1.id).unwrap();

        // Order 2: B then A
        let item2 = create_item(&store, "neutral text two");
        store
            .assign_item(item2.id, cat_b.id, &manual_assignment())
            .unwrap();
        store
            .assign_item(item2.id, cat_a.id, &manual_assignment())
            .unwrap();
        let result2 = process_item(&store, &classifier, item2.id).unwrap();

        assert!(result1.new_assignments.contains(&target.id));
        assert!(result2.new_assignments.contains(&target.id));
    }

    #[test]
    fn profile_idempotent_when_already_assigned() {
        // Item already in target → no duplicate assignment
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let trigger = category("Trigger", false);
        let target = category_with_profile("Target", &[trigger.id], &[]);
        create_category(&store, &trigger);
        create_category(&store, &target);

        let item = create_item(&store, "neutral");
        store
            .assign_item(item.id, trigger.id, &manual_assignment())
            .unwrap();
        store
            .assign_item(item.id, target.id, &manual_assignment())
            .unwrap();

        let result = process_item(&store, &classifier, item.id).unwrap();
        // Target already assigned, so it shouldn't appear in new_assignments
        assert!(!result.new_assignments.contains(&target.id));
    }

    #[test]
    fn profile_provenance_tracking() {
        // Auto-assigned via profile gets origin="profile:CategoryName"
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let trigger = category("Trigger", false);
        let target = category_with_profile("Target", &[trigger.id], &[]);
        create_category(&store, &trigger);
        create_category(&store, &target);

        let item = create_item(&store, "neutral");
        store
            .assign_item(item.id, trigger.id, &manual_assignment())
            .unwrap();
        process_item(&store, &classifier, item.id).unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        let target_assignment = assignments.get(&target.id).unwrap();
        assert_eq!(target_assignment.source, AssignmentSource::AutoMatch);
        assert_eq!(target_assignment.origin.as_deref(), Some("profile:Target"));
    }

    #[test]
    fn profile_or_criteria() {
        // "If CatA OR CatB → assign to Target"
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let cat_a = category("CatA", false);
        let cat_b = category("CatB", false);
        let target = category_with_profile_or("Target", &[], &[], &[cat_a.id, cat_b.id]);
        create_category(&store, &cat_a);
        create_category(&store, &cat_b);
        create_category(&store, &target);

        // Only CatA assigned — should still fire (OR)
        let item = create_item(&store, "neutral");
        store
            .assign_item(item.id, cat_a.id, &manual_assignment())
            .unwrap();
        let result = process_item(&store, &classifier, item.id).unwrap();
        assert!(result.new_assignments.contains(&target.id));
    }

    #[test]
    fn profile_or_criteria_no_match() {
        // OR criteria but none present → should NOT fire
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let cat_a = category("CatA", false);
        let cat_b = category("CatB", false);
        let target = category_with_profile_or("Target", &[], &[], &[cat_a.id, cat_b.id]);
        create_category(&store, &cat_a);
        create_category(&store, &cat_b);
        create_category(&store, &target);

        let item = create_item(&store, "neutral");
        let result = process_item(&store, &classifier, item.id).unwrap();
        assert!(!result.new_assignments.contains(&target.id));
    }

    #[test]
    fn profile_multiple_conditions_or_semantics() {
        // Category with two profile conditions — either one matching should fire
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let cat_a = category("CatA", false);
        let cat_b = category("CatB", false);
        create_category(&store, &cat_a);
        create_category(&store, &cat_b);

        // Target has TWO separate profile conditions (OR'd across conditions)
        let mut target = category("Target", false);
        let mut criteria1 = Query::default();
        criteria1.set_criterion(CriterionMode::And, cat_a.id);
        target.conditions.push(Condition::Profile {
            criteria: Box::new(criteria1),
        });
        let mut criteria2 = Query::default();
        criteria2.set_criterion(CriterionMode::And, cat_b.id);
        target.conditions.push(Condition::Profile {
            criteria: Box::new(criteria2),
        });
        create_category(&store, &target);

        // Only CatB assigned — second condition matches
        let item = create_item(&store, "neutral");
        store
            .assign_item(item.id, cat_b.id, &manual_assignment())
            .unwrap();
        let result = process_item(&store, &classifier, item.id).unwrap();
        assert!(result.new_assignments.contains(&target.id));
    }

    #[test]
    fn condition_match_mode_all_requires_date_and_profile_rules() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let cost = category("Cost", false);
        create_category(&store, &cost);

        let mut target = category_with_date(
            "Moto Budget 2025",
            DateSource::When,
            DateMatcher::Range {
                from: DateValueExpr::AbsoluteDate(jiff::civil::date(2026, 1, 1)),
                through: DateValueExpr::AbsoluteDate(jiff::civil::date(2026, 12, 31)),
            },
        );
        target.condition_match_mode = ConditionMatchMode::All;
        let mut criteria = Query::default();
        criteria.set_criterion(CriterionMode::And, cost.id);
        target.conditions.push(Condition::Profile {
            criteria: Box::new(criteria),
        });
        create_category(&store, &target);

        let item = create_item(&store, "Moto registration");
        set_item_when(
            &store,
            item.id,
            Some(jiff::civil::date(2026, 4, 4).at(10, 0, 0, 0)),
        );
        store
            .assign_item(item.id, cost.id, &manual_assignment())
            .unwrap();

        let result = process_item(&store, &classifier, item.id).unwrap();
        assert!(result.new_assignments.contains(&target.id));
        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(matches!(
            assignments
                .get(&target.id)
                .and_then(|assignment| assignment.explanation.as_ref()),
            Some(AssignmentExplanation::ConditionGroup {
                match_mode: ConditionMatchMode::All,
                ..
            })
        ));

        let missing_cost = create_item(&store, "Moto insurance");
        set_item_when(
            &store,
            missing_cost.id,
            Some(jiff::civil::date(2026, 4, 5).at(10, 0, 0, 0)),
        );
        let missing_cost_result = process_item(&store, &classifier, missing_cost.id).unwrap();
        assert!(!missing_cost_result.new_assignments.contains(&target.id));

        let missing_date = create_item(&store, "Moto inspection");
        store
            .assign_item(missing_date.id, cost.id, &manual_assignment())
            .unwrap();
        let missing_date_result = process_item(&store, &classifier, missing_date.id).unwrap();
        assert!(!missing_date_result.new_assignments.contains(&target.id));
    }

    // ---------------------------------------------------------------
    // "Mom OR Dad → Family" scenarios — two ways to express OR
    // ---------------------------------------------------------------

    #[test]
    fn profile_mom_or_dad_via_or_criteria() {
        // Single condition with OR criteria: --or Mom --or Dad
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let mom = category("Mom", false);
        let dad = category("Dad", false);
        let family = category_with_profile_or("Family", &[], &[], &[mom.id, dad.id]);
        create_category(&store, &mom);
        create_category(&store, &dad);
        create_category(&store, &family);

        // Only Mom assigned → Family fires
        let item1 = create_item(&store, "neutral one");
        store
            .assign_item(item1.id, mom.id, &manual_assignment())
            .unwrap();
        let result1 = process_item(&store, &classifier, item1.id).unwrap();
        assert!(result1.new_assignments.contains(&family.id));

        // Only Dad assigned → Family fires
        let item2 = create_item(&store, "neutral two");
        store
            .assign_item(item2.id, dad.id, &manual_assignment())
            .unwrap();
        let result2 = process_item(&store, &classifier, item2.id).unwrap();
        assert!(result2.new_assignments.contains(&family.id));

        // Neither assigned → Family does NOT fire
        let item3 = create_item(&store, "neutral three");
        let result3 = process_item(&store, &classifier, item3.id).unwrap();
        assert!(!result3.new_assignments.contains(&family.id));
    }

    #[test]
    fn profile_mom_or_dad_via_multiple_conditions() {
        // Two separate profile conditions: condition1 = --and Mom, condition2 = --and Dad
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let mom = category("Mom", false);
        let dad = category("Dad", false);
        create_category(&store, &mom);
        create_category(&store, &dad);

        let mut family = category("Family", false);
        let mut c1 = Query::default();
        c1.set_criterion(CriterionMode::And, mom.id);
        family.conditions.push(Condition::Profile {
            criteria: Box::new(c1),
        });
        let mut c2 = Query::default();
        c2.set_criterion(CriterionMode::And, dad.id);
        family.conditions.push(Condition::Profile {
            criteria: Box::new(c2),
        });
        create_category(&store, &family);

        // Only Mom assigned → Family fires (first condition matches)
        let item1 = create_item(&store, "neutral one");
        store
            .assign_item(item1.id, mom.id, &manual_assignment())
            .unwrap();
        let result1 = process_item(&store, &classifier, item1.id).unwrap();
        assert!(result1.new_assignments.contains(&family.id));

        // Only Dad assigned → Family fires (second condition matches)
        let item2 = create_item(&store, "neutral two");
        store
            .assign_item(item2.id, dad.id, &manual_assignment())
            .unwrap();
        let result2 = process_item(&store, &classifier, item2.id).unwrap();
        assert!(result2.new_assignments.contains(&family.id));

        // Both assigned → Family fires (both conditions match)
        let item3 = create_item(&store, "neutral three");
        store
            .assign_item(item3.id, mom.id, &manual_assignment())
            .unwrap();
        store
            .assign_item(item3.id, dad.id, &manual_assignment())
            .unwrap();
        let result3 = process_item(&store, &classifier, item3.id).unwrap();
        assert!(result3.new_assignments.contains(&family.id));

        // Neither assigned → Family does NOT fire
        let item4 = create_item(&store, "neutral four");
        let result4 = process_item(&store, &classifier, item4.id).unwrap();
        assert!(!result4.new_assignments.contains(&family.id));
    }

    #[test]
    fn process_item_implicit_match_auto_breaks_non_sticky_assignment() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let mom = category("Mom", true);
        create_category(&store, &mom);

        let mut item = create_item(&store, "Call Mom about birthday");
        let first = process_item(&store, &classifier, item.id).unwrap();
        assert!(first.new_assignments.contains(&mom.id));

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        let mom_assignment = assignments.get(&mom.id).unwrap();
        assert_eq!(mom_assignment.source, AssignmentSource::AutoMatch);
        assert!(!mom_assignment.sticky);

        item.text = "Call Dad about birthday".to_string();
        item.modified_at = Timestamp::now();
        store.update_item(&item).unwrap();

        let second = process_item(&store, &classifier, item.id).unwrap();
        assert!(second.removed_assignments.contains(&mom.id));
        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(
            !assignments.contains_key(&mom.id),
            "implicit string assignment should auto-break when text no longer matches"
        );
    }

    #[test]
    fn process_item_profile_assignment_auto_breaks_when_prerequisite_removed() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let urgent = category("Urgent", false);
        let project_alpha = category("Project Alpha", false);
        let escalated = category_with_profile("Escalated", &[urgent.id, project_alpha.id], &[]);
        create_category(&store, &urgent);
        create_category(&store, &project_alpha);
        create_category(&store, &escalated);

        let item = create_item(&store, "neutral");
        store
            .assign_item(item.id, urgent.id, &manual_assignment())
            .unwrap();
        store
            .assign_item(item.id, project_alpha.id, &manual_assignment())
            .unwrap();

        let first = process_item(&store, &classifier, item.id).unwrap();
        assert!(first.new_assignments.contains(&escalated.id));
        let escalated_assignment = store.get_assignments_for_item(item.id).unwrap();
        assert!(!escalated_assignment[&escalated.id].sticky);

        store.unassign_item(item.id, urgent.id).unwrap();
        let second = process_item(&store, &classifier, item.id).unwrap();
        assert!(second.removed_assignments.contains(&escalated.id));
        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(
            !assignments.contains_key(&escalated.id),
            "profile assignment should auto-break when prerequisite assignment disappears"
        );
    }

    #[test]
    fn process_item_action_assignment_persists_after_live_condition_breaks() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let urgent = category("Urgent", false);
        let project_alpha = category("Project Alpha", false);
        let mut escalated = category_with_profile("Escalated", &[urgent.id, project_alpha.id], &[]);
        let notify = category("Notify", false);
        escalated.actions.push(Action::Assign {
            targets: HashSet::from([notify.id]),
        });
        create_category(&store, &urgent);
        create_category(&store, &project_alpha);
        create_category(&store, &escalated);
        create_category(&store, &notify);

        let item = create_item(&store, "neutral");
        store
            .assign_item(item.id, urgent.id, &manual_assignment())
            .unwrap();
        store
            .assign_item(item.id, project_alpha.id, &manual_assignment())
            .unwrap();

        let first = process_item(&store, &classifier, item.id).unwrap();
        assert!(first.new_assignments.contains(&escalated.id));
        assert!(first.new_assignments.contains(&notify.id));

        store.unassign_item(item.id, urgent.id).unwrap();
        let second = process_item(&store, &classifier, item.id).unwrap();
        assert!(second.removed_assignments.contains(&escalated.id));

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(!assignments.contains_key(&escalated.id));
        assert!(
            assignments.contains_key(&notify.id),
            "action-produced assignment should remain after live condition assignment breaks"
        );
        assert_eq!(assignments[&notify.id].source, AssignmentSource::Action);
        assert!(assignments[&notify.id].sticky);
    }
}
