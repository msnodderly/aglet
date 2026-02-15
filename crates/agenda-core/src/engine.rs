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

    for category in categories {
        let Some(reason) = evaluate_category_match(category, item_text, assignments, classifier)
        else {
            continue;
        };

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
        pass_result.new_assignments.insert(category.id);

        fire_actions(
            store,
            item_id,
            category,
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
        .include
        .iter()
        .all(|category_id| assignments.contains_key(category_id))
        && criteria
            .exclude
            .iter()
            .all(|category_id| !assignments.contains_key(category_id))
}

fn fire_actions(
    store: &Store,
    item_id: ItemId,
    category: &Category,
    assignments: &mut HashMap<CategoryId, Assignment>,
    seen_pairs: &mut HashSet<(ItemId, CategoryId)>,
    pass_result: &mut PassResult,
) -> Result<()> {
    for action in &category.actions {
        match action {
            Action::Assign { targets } => {
                for target_id in targets {
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

fn assign_if_unassigned(
    store: &Store,
    item_id: ItemId,
    category_id: CategoryId,
    source: AssignmentSource,
    origin: Option<String>,
    assignments: &mut HashMap<CategoryId, Assignment>,
    seen_pairs: &mut HashSet<(ItemId, CategoryId)>,
) -> Result<bool> {
    let pair = (item_id, category_id);

    if assignments.contains_key(&category_id) {
        seen_pairs.insert(pair);
        return Ok(false);
    }

    if seen_pairs.contains(&pair) {
        return Ok(false);
    }

    let assignment = Assignment {
        source,
        assigned_at: Utc::now(),
        sticky: true,
        origin,
    };

    store.assign_item(item_id, category_id, &assignment)?;
    assignments.insert(category_id, assignment);
    seen_pairs.insert(pair);

    Ok(true)
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
    use std::collections::HashSet;

    use chrono::Utc;

    use super::process_item;
    use crate::error::AgendaError;
    use crate::matcher::SubstringClassifier;
    use crate::model::{
        Action, Assignment, AssignmentSource, Category, CategoryId, Condition, Item, Query,
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

    fn manual_assignment() -> Assignment {
        Assignment {
            source: AssignmentSource::Manual,
            assigned_at: Utc::now(),
            sticky: true,
            origin: Some("manual:test".to_string()),
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
        criteria.include.extend(include.iter().copied());
        criteria.exclude.extend(exclude.iter().copied());
        category.conditions.push(Condition::Profile { criteria });
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
            criteria.include.insert(stages[index + 1].id);
            stage.conditions = vec![Condition::Profile { criteria }];
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
}
