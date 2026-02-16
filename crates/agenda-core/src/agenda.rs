use chrono::Utc;

use crate::engine::{evaluate_all_items, process_item, EvaluateAllItemsResult, ProcessItemResult};
use crate::error::Result;
use crate::matcher::Classifier;
use crate::model::{Assignment, AssignmentSource, Category, CategoryId, Item, ItemId};
use crate::store::Store;

/// Synchronous integration layer that wires Store mutations to engine execution.
pub struct Agenda<'a> {
    store: &'a Store,
    classifier: &'a dyn Classifier,
}

impl<'a> Agenda<'a> {
    pub fn new(store: &'a Store, classifier: &'a dyn Classifier) -> Self {
        Self { store, classifier }
    }

    pub fn store(&self) -> &Store {
        self.store
    }

    pub fn create_item(&self, item: &Item) -> Result<ProcessItemResult> {
        self.store.create_item(item)?;
        process_item(self.store, self.classifier, item.id)
    }

    pub fn update_item(&self, item: &Item) -> Result<ProcessItemResult> {
        self.store.update_item(item)?;
        process_item(self.store, self.classifier, item.id)
    }

    pub fn create_category(&self, category: &Category) -> Result<EvaluateAllItemsResult> {
        self.store.create_category(category)?;
        evaluate_all_items(self.store, self.classifier, category.id)
    }

    pub fn update_category(&self, category: &Category) -> Result<EvaluateAllItemsResult> {
        self.store.update_category(category)?;
        evaluate_all_items(self.store, self.classifier, category.id)
    }

    pub fn assign_item_manual(
        &self,
        item_id: ItemId,
        category_id: CategoryId,
        origin: Option<String>,
    ) -> Result<ProcessItemResult> {
        let assignment = Assignment {
            source: AssignmentSource::Manual,
            assigned_at: Utc::now(),
            sticky: true,
            origin: origin.or_else(|| Some("manual".to_string())),
        };

        self.store.assign_item(item_id, category_id, &assignment)?;
        process_item(self.store, self.classifier, item_id)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use chrono::Utc;

    use super::Agenda;
    use crate::error::AgendaError;
    use crate::matcher::SubstringClassifier;
    use crate::model::{Action, AssignmentSource, Category, CategoryId, Condition, Item, Query};
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
        updated.modified_at = Utc::now();

        let result = agenda.update_item(&updated).unwrap();
        assert!(result.new_assignments.contains(&urgent.id));
        assert!(store
            .get_assignments_for_item(item.id)
            .unwrap()
            .contains_key(&urgent.id));
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
        criteria.include.insert(urgent.id);
        escalated.conditions.push(Condition::Profile { criteria });
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
            criteria.include.insert(stages[index + 1].id);
            stage.conditions = vec![Condition::Profile { criteria }];
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
}
