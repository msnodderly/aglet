use std::collections::{HashMap, HashSet};

use chrono::Utc;

use crate::error::Result;
use crate::matcher::Classifier;
use crate::model::{
    Action, Assignment, AssignmentSource, Category, CategoryId, Condition, ItemId, Query,
};
use crate::store::Store;

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

#[derive(Debug, Clone, Copy)]
enum MatchReason {
    ImplicitString,
    Profile,
}

/// Process one item through a single hierarchy walk.
///
/// This intentionally performs a single pass only. Fixed-point re-processing
/// is implemented separately.
pub fn process_item(
    store: &Store,
    classifier: &dyn Classifier,
    item_id: ItemId,
) -> Result<ProcessItemResult> {
    let item = store.get_item(item_id)?;
    let categories = store.get_hierarchy()?;

    let mut result = ProcessItemResult::default();
    let mut assignments = item.assignments;

    for category in categories {
        let Some(reason) = evaluate_category_match(&category, &item.text, &assignments, classifier)
        else {
            continue;
        };

        let assigned = assign_if_unassigned(
            store,
            item_id,
            category.id,
            AssignmentSource::AutoMatch,
            Some(match_origin(reason, &category.name)),
            &mut assignments,
            &mut result.new_assignments,
        )?;

        // Assignments are sticky: no re-assign and no action re-fire.
        if !assigned {
            continue;
        }

        fire_actions(store, item_id, &category, &mut assignments, &mut result)?;
    }

    Ok(result)
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
    result: &mut ProcessItemResult,
) -> Result<()> {
    for action in &category.actions {
        match action {
            Action::Assign { targets } => {
                for target_id in targets {
                    assign_if_unassigned(
                        store,
                        item_id,
                        *target_id,
                        AssignmentSource::Action,
                        Some(format!("action:{}", category.name)),
                        assignments,
                        &mut result.new_assignments,
                    )?;
                }
            }
            Action::Remove { targets } => {
                for target_id in targets {
                    result.deferred_removals.push(DeferredRemoval {
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
    new_assignments: &mut HashSet<CategoryId>,
) -> Result<bool> {
    if assignments.contains_key(&category_id) {
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
    new_assignments.insert(category_id);

    Ok(true)
}

fn match_origin(reason: MatchReason, category_name: &str) -> String {
    match reason {
        MatchReason::ImplicitString => format!("cat:{category_name}"),
        MatchReason::Profile => format!("profile:{category_name}"),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use chrono::Utc;

    use super::process_item;
    use crate::matcher::SubstringClassifier;
    use crate::model::{Action, Assignment, AssignmentSource, Category, Condition, Item, Query};
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

    fn category_id_by_name(store: &Store, name: &str) -> crate::model::CategoryId {
        store
            .get_hierarchy()
            .unwrap()
            .into_iter()
            .find(|category| category.name == name)
            .map(|category| category.id)
            .unwrap()
    }

    #[test]
    fn process_item_implicit_string_match_assigns_category() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let sarah = Category::new("Sarah".to_string());
        create_category(&store, &sarah);

        let item = create_item(&store, "Call Sarah tomorrow");
        let result = process_item(&store, &classifier, item.id).unwrap();

        assert!(result.new_assignments.contains(&sarah.id));

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        let assignment = assignments.get(&sarah.id).unwrap();
        assert_eq!(assignment.source, AssignmentSource::AutoMatch);
    }

    #[test]
    fn process_item_implicit_string_disabled_does_not_assign_reserved_done() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let done_id = category_id_by_name(&store, "Done");
        let item = create_item(&store, "Get it done");

        process_item(&store, &classifier, item.id).unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(!assignments.contains_key(&done_id));
    }

    #[test]
    fn process_item_profile_match_assigns_category() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let urgent = Category::new("Urgent".to_string());
        create_category(&store, &urgent);

        let mut escalated = Category::new("Escalated".to_string());
        let mut criteria = Query::default();
        criteria.include.insert(urgent.id);
        escalated.conditions.push(Condition::Profile { criteria });
        create_category(&store, &escalated);

        let item = create_item(&store, "Call vendor");
        store
            .assign_item(item.id, urgent.id, &manual_assignment())
            .unwrap();

        let result = process_item(&store, &classifier, item.id).unwrap();
        assert!(result.new_assignments.contains(&escalated.id));

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        let assignment = assignments.get(&escalated.id).unwrap();
        assert_eq!(assignment.source, AssignmentSource::AutoMatch);
        assert_eq!(assignment.origin.as_deref(), Some("profile:Escalated"));
    }

    #[test]
    fn process_item_profile_no_match_does_not_assign_category() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let urgent = Category::new("Urgent".to_string());
        create_category(&store, &urgent);

        let mut escalated = Category::new("Escalated".to_string());
        let mut criteria = Query::default();
        criteria.include.insert(urgent.id);
        escalated.conditions.push(Condition::Profile { criteria });
        create_category(&store, &escalated);

        let item = create_item(&store, "Call vendor");
        process_item(&store, &classifier, item.id).unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(!assignments.contains_key(&escalated.id));
    }

    #[test]
    fn process_item_assign_action_fires_for_new_match() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let calendar = Category::new("Calendar".to_string());
        create_category(&store, &calendar);

        let mut meetings = Category::new("Meetings".to_string());
        meetings.actions.push(Action::Assign {
            targets: HashSet::from([calendar.id]),
        });
        create_category(&store, &meetings);

        let item = create_item(&store, "Team Meetings tomorrow");

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
    fn process_item_remove_action_is_deferred() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let projects = Category::new("Projects".to_string());
        create_category(&store, &projects);

        let mut cleanup = Category::new("Cleanup".to_string());
        cleanup.actions.push(Action::Remove {
            targets: HashSet::from([projects.id]),
        });
        create_category(&store, &cleanup);

        let item = create_item(&store, "Cleanup inbox");
        store
            .assign_item(item.id, projects.id, &manual_assignment())
            .unwrap();

        let result = process_item(&store, &classifier, item.id).unwrap();

        assert!(result.deferred_removals.iter().any(|removal| {
            removal.target == projects.id && removal.triggered_by == cleanup.id
        }));

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&projects.id));
    }

    #[test]
    fn process_item_already_assigned_skips_refire() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let calendar = Category::new("Calendar".to_string());
        create_category(&store, &calendar);

        let mut meetings = Category::new("Meetings".to_string());
        meetings.actions.push(Action::Assign {
            targets: HashSet::from([calendar.id]),
        });
        create_category(&store, &meetings);

        let item = create_item(&store, "Meetings today");
        store
            .assign_item(item.id, meetings.id, &manual_assignment())
            .unwrap();

        let result = process_item(&store, &classifier, item.id).unwrap();

        assert!(!result.new_assignments.contains(&meetings.id));

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&meetings.id));
        assert!(!assignments.contains_key(&calendar.id));
    }

    #[test]
    fn process_item_no_match_does_not_assign() {
        let store = Store::open_memory().unwrap();
        let classifier = SubstringClassifier;

        let sarah = Category::new("Sarah".to_string());
        create_category(&store, &sarah);

        let item = create_item(&store, "Buy groceries");
        let result = process_item(&store, &classifier, item.id).unwrap();

        assert!(result.new_assignments.is_empty());

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(!assignments.contains_key(&sarah.id));
    }
}
