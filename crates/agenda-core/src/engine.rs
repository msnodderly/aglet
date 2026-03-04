use std::collections::{HashMap, HashSet};

use chrono::Utc;

use crate::error::{AgendaError, Result};
use crate::matcher::Classifier;
use crate::model::{
    Action, Assignment, AssignmentSource, Category, CategoryId, Condition, ItemId, Query,
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
    pub deferred_removals: Vec<DeferredRemoval>,
}

#[derive(Debug, Default)]
pub struct EvaluateAllItemsResult {
    pub processed_items: usize,
    pub affected_items: usize,
    pub total_new_assignments: usize,
    pub total_deferred_removals: usize,
}

#[derive(Debug, Default)]
struct PassResult {
    new_assignments: HashSet<CategoryId>,
    deferred_removals: Vec<DeferredRemoval>,
}

#[derive(Debug, Clone, Copy)]
enum MatchReason {
    ImplicitString,
    Profile,
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
    run_in_savepoint(store, || process_item_inner(store, classifier, item_id))
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
    // Validate the target category exists before beginning retroactive work.
    store.get_category(category_id)?;

    let mut result = EvaluateAllItemsResult::default();
    let items = store.list_items()?;

    for item in items {
        let process_result = process_item(store, classifier, item.id)?;

        result.processed_items += 1;
        result.total_new_assignments += process_result.new_assignments.len();
        result.total_deferred_removals += process_result.deferred_removals.len();

        if !process_result.new_assignments.is_empty()
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
) -> Result<ProcessItemResult> {
    let item = store.get_item(item_id)?;
    let categories = store.get_hierarchy()?;

    let mut assignments = item.assignments;
    let mut seen_pairs: HashSet<(ItemId, CategoryId)> = assignments
        .keys()
        .copied()
        .map(|category_id| (item_id, category_id))
        .collect();

    let mut result = ProcessItemResult::default();

    for pass in 1..=MAX_PASSES {
        let pass_result = run_hierarchy_pass(
            store,
            classifier,
            item_id,
            &item.text,
            &categories,
            &mut assignments,
            &mut seen_pairs,
        )?;

        let made_new_assignments = !pass_result.new_assignments.is_empty();

        result.new_assignments.extend(pass_result.new_assignments);
        result
            .deferred_removals
            .extend(pass_result.deferred_removals);

        if !made_new_assignments {
            apply_deferred_removals(store, item_id, &result.deferred_removals)?;
            return Ok(result);
        }

        if pass == MAX_PASSES {
            apply_deferred_removals(store, item_id, &result.deferred_removals)?;
            return Err(pass_cap_error(item_id));
        }
    }

    unreachable!("fixed-point loop should always return from within MAX_PASSES");
}

fn run_hierarchy_pass(
    store: &Store,
    classifier: &dyn Classifier,
    item_id: ItemId,
    item_text: &str,
    categories: &[Category],
    assignments: &mut HashMap<CategoryId, Assignment>,
    seen_pairs: &mut HashSet<(ItemId, CategoryId)>,
) -> Result<PassResult> {
    let mut pass_result = PassResult::default();
    let categories_by_id: HashMap<CategoryId, &Category> = categories
        .iter()
        .map(|category| (category.id, category))
        .collect();

    for category in categories {
        let Some(reason) = evaluate_category_match(category, item_text, assignments, classifier)
        else {
            continue;
        };

        if !can_assign(item_id, category.id, assignments, seen_pairs) {
            continue;
        }

        enforce_mutual_exclusion(store, item_id, category.id, &categories_by_id, assignments)?;

        let assigned = assign_if_unassigned(
            store,
            item_id,
            category.id,
            AssignmentSource::AutoMatch,
            Some(match_origin(reason, &category.name)),
            assignments,
            seen_pairs,
        )?;

        // Assignments are sticky: no re-assign and no action re-fire.
        if !assigned {
            continue;
        }
        assign_subsumption_ancestors(
            store,
            item_id,
            category.id,
            &categories_by_id,
            assignments,
            seen_pairs,
        )?;
        pass_result.new_assignments.insert(category.id);

        fire_actions(
            store,
            item_id,
            category,
            &categories_by_id,
            assignments,
            seen_pairs,
            &mut pass_result,
        )?;
    }

    Ok(pass_result)
}

fn evaluate_category_match(
    category: &Category,
    item_text: &str,
    assignments: &HashMap<CategoryId, Assignment>,
    classifier: &dyn Classifier,
) -> Option<MatchReason> {
    if category.enable_implicit_string && classifier.classify(item_text, &category.name).is_some() {
        return Some(MatchReason::ImplicitString);
    }

    let profile_match = category
        .conditions
        .iter()
        .filter_map(|condition| match condition {
            Condition::Profile { criteria } => Some(criteria),
            Condition::ImplicitString => None,
        })
        .any(|criteria| profile_matches(criteria, assignments));

    if profile_match {
        Some(MatchReason::Profile)
    } else {
        None
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
    store: &Store,
    item_id: ItemId,
    category: &Category,
    categories_by_id: &HashMap<CategoryId, &Category>,
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

                    enforce_mutual_exclusion(
                        store,
                        item_id,
                        *target_id,
                        categories_by_id,
                        assignments,
                    )?;

                    let assigned = assign_if_unassigned(
                        store,
                        item_id,
                        *target_id,
                        AssignmentSource::Action,
                        Some(format!("action:{}", category.name)),
                        assignments,
                        seen_pairs,
                    )?;
                    if assigned {
                        assign_subsumption_ancestors(
                            store,
                            item_id,
                            *target_id,
                            categories_by_id,
                            assignments,
                            seen_pairs,
                        )?;
                        pass_result.new_assignments.insert(*target_id);
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
    store: &Store,
    item_id: ItemId,
    category_id: CategoryId,
    source: AssignmentSource,
    origin: Option<String>,
    assignments: &mut HashMap<CategoryId, Assignment>,
    seen_pairs: &mut HashSet<(ItemId, CategoryId)>,
) -> Result<bool> {
    if !can_assign(item_id, category_id, assignments, seen_pairs) {
        return Ok(false);
    }

    let pair = (item_id, category_id);
    let assignment = Assignment {
        source,
        assigned_at: Utc::now(),
        sticky: true,
        origin,
        numeric_value: None,
    };

    store.assign_item(item_id, category_id, &assignment)?;
    assignments.insert(category_id, assignment);
    seen_pairs.insert(pair);

    Ok(true)
}

fn enforce_mutual_exclusion(
    store: &Store,
    item_id: ItemId,
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

        if assignments.remove(sibling_id).is_some() {
            store.unassign_item(item_id, *sibling_id)?;
        }
    }

    Ok(())
}

fn assign_subsumption_ancestors(
    store: &Store,
    item_id: ItemId,
    category_id: CategoryId,
    categories_by_id: &HashMap<CategoryId, &Category>,
    assignments: &mut HashMap<CategoryId, Assignment>,
    seen_pairs: &mut HashSet<(ItemId, CategoryId)>,
) -> Result<()> {
    let Some(category) = categories_by_id.get(&category_id) else {
        return Ok(());
    };

    let subsumption_origin = Some(format!("subsumption:{}", category.name));
    let mut current_parent = category.parent;
    let mut visited = HashSet::new();

    while let Some(ancestor_id) = current_parent {
        if !visited.insert(ancestor_id) {
            break;
        }

        let pair = (item_id, ancestor_id);
        if let std::collections::hash_map::Entry::Vacant(entry) = assignments.entry(ancestor_id) {
            if !seen_pairs.contains(&pair) {
                let assignment = Assignment {
                    source: AssignmentSource::Subsumption,
                    assigned_at: Utc::now(),
                    sticky: true,
                    origin: subsumption_origin.clone(),
                    numeric_value: None,
                };
                store.assign_item(item_id, ancestor_id, &assignment)?;
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

    Ok(())
}

fn apply_deferred_removals(
    store: &Store,
    item_id: ItemId,
    deferred_removals: &[DeferredRemoval],
) -> Result<()> {
    let mut removed_targets = HashSet::new();

    for removal in deferred_removals {
        if removed_targets.insert(removal.target) {
            store.unassign_item(item_id, removal.target)?;
        }
    }

    Ok(())
}

fn match_origin(reason: MatchReason, category_name: &str) -> String {
    match reason {
        MatchReason::ImplicitString => format!("cat:{category_name}"),
        MatchReason::Profile => format!("profile:{category_name}"),
    }
}

fn pass_cap_error(item_id: ItemId) -> AgendaError {
    AgendaError::InvalidOperation {
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

    use chrono::Utc;

    use super::{evaluate_all_items, process_item, run_hierarchy_pass};
    use crate::error::AgendaError;
    use crate::matcher::SubstringClassifier;
    use crate::model::{
        Action, Assignment, AssignmentSource, Category, CategoryId, Condition, CriterionMode, Item,
        ItemId, Query,
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

    fn set_item_text(store: &Store, item_id: ItemId, text: &str) {
        let mut item = store.get_item(item_id).unwrap();
        item.text = text.to_string();
        item.modified_at = Utc::now();
        store.update_item(&item).unwrap();
    }

    fn manual_assignment() -> Assignment {
        Assignment {
            source: AssignmentSource::Manual,
            assigned_at: Utc::now(),
            sticky: true,
            origin: Some("manual:test".to_string()),
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

    fn child_category(name: &str, parent: CategoryId, implicit: bool) -> Category {
        let mut category = category(name, implicit);
        category.parent = Some(parent);
        category
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
        let mut assignments = HashMap::new();
        let mut seen_pairs = HashSet::new();

        let pass_result = run_hierarchy_pass(
            &store,
            &classifier,
            item.id,
            &item.text,
            &categories,
            &mut assignments,
            &mut seen_pairs,
        )
        .unwrap();

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
        assert!(assignments.contains_key(&in_progress.id));
        assert!(!assignments.contains_key(&todo.id));
    }

    #[test]
    fn process_item_mutual_exclusion_unassigns_manual_source() {
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
        assert!(!assignments.contains_key(&todo.id));
        assert!(assignments.contains_key(&in_progress.id));
    }

    #[test]
    fn process_item_mutual_exclusion_three_children_removes_only_assigned_sibling() {
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
        assert!(!assignments.contains_key(&low.id));
        assert!(!assignments.contains_key(&medium.id));
        assert!(assignments.contains_key(&high.id));
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
        let second = create_item(&store, "Plan meetings agenda");
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
        assert!(!assignments.contains_key(&todo.id));
        assert!(assignments.contains_key(&in_progress.id));
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
            AgendaError::InvalidOperation { message } => {
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
        let result: Result<(), AgendaError> = run_in_savepoint(&store, || {
            store
                .assign_item(item.id, tag.id, &manual_assignment())
                .unwrap();
            Err(AgendaError::InvalidOperation {
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

        let result: Result<(), AgendaError> = run_in_savepoint(&store, || {
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
}
