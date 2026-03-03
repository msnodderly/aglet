use std::collections::{HashMap, HashSet};

use chrono::{NaiveDate, NaiveDateTime, Timelike, Utc};
use rust_decimal::Decimal;

use crate::dates::{BasicDateParser, DateParser};
use crate::engine::{evaluate_all_items, process_item, EvaluateAllItemsResult, ProcessItemResult};
use crate::error::{AgendaError, Result};
use crate::matcher::Classifier;
use crate::model::{
    Assignment, AssignmentSource, Category, CategoryId, CategoryValueKind, Item, ItemId, ItemLink,
    ItemLinkKind, ItemLinksForItem, Section, View,
};
use crate::store::Store;

/// Synchronous integration layer that wires Store mutations to engine execution.
pub struct Agenda<'a> {
    store: &'a Store,
    classifier: &'a dyn Classifier,
    date_parser: BasicDateParser,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct LinkItemsResult {
    pub created: bool,
}

impl<'a> Agenda<'a> {
    pub fn new(store: &'a Store, classifier: &'a dyn Classifier) -> Self {
        Self {
            store,
            classifier,
            date_parser: BasicDateParser::default(),
        }
    }

    pub fn store(&self) -> &Store {
        self.store
    }

    pub fn create_item(&self, item: &Item) -> Result<ProcessItemResult> {
        self.create_item_with_reference_date(item, Utc::now().date_naive())
    }

    pub fn create_item_with_reference_date(
        &self,
        item: &Item,
        reference_date: NaiveDate,
    ) -> Result<ProcessItemResult> {
        let mut item_to_create = item.clone();
        let parsed_datetime = self.parse_datetime_from_text(&item_to_create.text, reference_date);
        if let Some(datetime) = parsed_datetime {
            item_to_create.when_date = Some(datetime);
        }

        self.store.create_item(&item_to_create)?;

        if parsed_datetime.is_some() {
            self.assign_when_provenance(item_to_create.id)?;
        }

        process_item(self.store, self.classifier, item_to_create.id)
    }

    pub fn update_item(&self, item: &Item) -> Result<ProcessItemResult> {
        self.update_item_with_reference_date(item, Utc::now().date_naive())
    }

    pub fn update_item_with_reference_date(
        &self,
        item: &Item,
        reference_date: NaiveDate,
    ) -> Result<ProcessItemResult> {
        let mut item_to_update = item.clone();
        let parsed_datetime = self.parse_datetime_from_text(&item_to_update.text, reference_date);
        if let Some(datetime) = parsed_datetime {
            item_to_update.when_date = Some(datetime);
        }

        self.store.update_item(&item_to_update)?;

        if parsed_datetime.is_some() {
            self.assign_when_provenance(item_to_update.id)?;
        }

        process_item(self.store, self.classifier, item_to_update.id)
    }

    pub fn create_category(&self, category: &Category) -> Result<EvaluateAllItemsResult> {
        self.store.create_category(category)?;
        evaluate_all_items(self.store, self.classifier, category.id)
    }

    pub fn update_category(&self, category: &Category) -> Result<EvaluateAllItemsResult> {
        self.store.update_category(category)?;
        evaluate_all_items(self.store, self.classifier, category.id)
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
        evaluate_all_items(self.store, self.classifier, category_id)
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
            assigned_at: Utc::now(),
            sticky: true,
            origin: origin.or_else(|| Some("manual".to_string())),
            numeric_value: None,
        };

        self.store.assign_item(item_id, category_id, &assignment)?;
        self.assign_subsumption_for_category(item_id, category_id)?;
        process_item(self.store, self.classifier, item_id)
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
            assigned_at: Utc::now(),
            sticky: true,
            origin: origin.or_else(|| Some("manual:numeric".to_string())),
            numeric_value: Some(numeric_value),
        };

        self.store.assign_item(item_id, category_id, &assignment)?;
        self.assign_subsumption_for_category(item_id, category_id)?;
        process_item(self.store, self.classifier, item_id)
    }

    pub fn unassign_item_manual(&self, item_id: ItemId, category_id: CategoryId) -> Result<()> {
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
        self.store.unassign_item(item_id, category_id)
    }

    pub fn insert_item_in_section(
        &self,
        item_id: ItemId,
        view: &View,
        section: &Section,
    ) -> Result<ProcessItemResult> {
        let mut targets = section.on_insert_assign.clone();
        targets.extend(section.criteria.and_category_ids());
        targets.extend(view.criteria.and_category_ids());

        self.assign_manual_categories(item_id, &targets, "edit:section.insert")?;
        process_item(self.store, self.classifier, item_id)
    }

    pub fn insert_item_in_unmatched(
        &self,
        item_id: ItemId,
        view: &View,
    ) -> Result<ProcessItemResult> {
        let view_include: HashSet<CategoryId> = view.criteria.and_category_ids().collect();
        self.assign_manual_categories(item_id, &view_include, "edit:view.insert")?;
        process_item(self.store, self.classifier, item_id)
    }

    pub fn remove_item_from_section(
        &self,
        item_id: ItemId,
        section: &Section,
    ) -> Result<ProcessItemResult> {
        self.unassign_categories(item_id, &section.on_remove_unassign)?;
        process_item(self.store, self.classifier, item_id)
    }

    pub fn remove_item_from_view(&self, item_id: ItemId, view: &View) -> Result<ProcessItemResult> {
        self.unassign_categories(item_id, &view.remove_from_view_unassign)?;
        process_item(self.store, self.classifier, item_id)
    }

    pub fn remove_item_from_unmatched(
        &self,
        item_id: ItemId,
        view: &View,
    ) -> Result<ProcessItemResult> {
        self.unassign_categories(item_id, &view.remove_from_view_unassign)?;
        process_item(self.store, self.classifier, item_id)
    }

    pub fn mark_item_done(&self, item_id: ItemId) -> Result<ProcessItemResult> {
        if !self.item_is_actionable(item_id)? {
            return Err(AgendaError::InvalidOperation {
                message: "selected item has no actionable categories".to_string(),
            });
        }
        let mut item = self.store.get_item(item_id)?;
        let now = Utc::now();
        let done_at = now
            .naive_utc()
            .with_nanosecond(0)
            .unwrap_or(now.naive_utc());
        item.is_done = true;
        item.done_date = Some(done_at);
        item.modified_at = now;
        self.store.update_item(&item)?;

        let done_category_id = self.done_category_id()?;
        let assignment = Assignment {
            source: AssignmentSource::Manual,
            assigned_at: now,
            sticky: true,
            origin: Some("manual:done".to_string()),
            numeric_value: None,
        };
        self.store
            .assign_item(item_id, done_category_id, &assignment)?;
        process_item(self.store, self.classifier, item_id)
    }

    pub fn mark_item_not_done(&self, item_id: ItemId) -> Result<ProcessItemResult> {
        let mut item = self.store.get_item(item_id)?;
        item.is_done = false;
        item.done_date = None;
        item.modified_at = Utc::now();
        self.store.update_item(&item)?;
        let done_category_id = self.done_category_id()?;
        self.store.unassign_item(item_id, done_category_id)?;
        process_item(self.store, self.classifier, item_id)
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
        Ok(LinkItemsResult { created: true })
    }

    pub fn unlink_items_depends_on(
        &self,
        dependent_id: ItemId,
        dependency_id: ItemId,
    ) -> Result<()> {
        self.store
            .delete_item_link(dependent_id, dependency_id, ItemLinkKind::DependsOn)
    }

    pub fn unlink_items_blocks(&self, blocker_id: ItemId, blocked_id: ItemId) -> Result<()> {
        self.unlink_items_depends_on(blocked_id, blocker_id)
    }

    pub fn unlink_items_related(&self, a: ItemId, b: ItemId) -> Result<()> {
        let (item_id, other_item_id) = Self::normalize_related_pair(a, b);
        self.store
            .delete_item_link(item_id, other_item_id, ItemLinkKind::Related)
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
            assigned_at: Utc::now(),
            sticky: true,
            origin: Some(origin.to_string()),
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
                    assigned_at: Utc::now(),
                    sticky: true,
                    origin: Some(format!("subsumption:{parent_name}")),
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
            created_at: Utc::now(),
            origin: Some("manual:link".to_string()),
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
                        "adding dependency would create a cycle: {} depends-on ... depends-on {}",
                        dependency_id, dependent_id
                    ),
                });
            }
            stack.extend(self.store.list_dependency_ids_for_item(current)?);
        }

        Ok(())
    }

    fn parse_datetime_from_text(
        &self,
        text: &str,
        reference_date: NaiveDate,
    ) -> Option<NaiveDateTime> {
        self.date_parser
            .parse(text, reference_date)
            .map(|parsed| parsed.datetime)
    }

    fn assign_when_provenance(&self, item_id: ItemId) -> Result<()> {
        let when_category_id = self.category_id_by_name("When")?;
        let assignment = Assignment {
            source: AssignmentSource::AutoMatch,
            assigned_at: Utc::now(),
            sticky: true,
            origin: Some("nlp:date".to_string()),
            numeric_value: None,
        };
        self.store
            .assign_item(item_id, when_category_id, &assignment)
    }

    fn done_category_id(&self) -> Result<CategoryId> {
        self.category_id_by_name("Done")
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

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::sync::{Arc, Barrier};
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    use chrono::{NaiveDate, NaiveDateTime, Utc};
    use rust_decimal::Decimal;

    use super::Agenda;
    use crate::error::AgendaError;
    use crate::matcher::SubstringClassifier;
    use crate::model::{
        Action, Assignment, AssignmentSource, Category, CategoryId, CategoryValueKind, Condition,
        CriterionMode, Item, ItemId, ItemLinkKind, Query, Section, View, WhenBucket,
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
            assigned_at: Utc::now(),
            sticky: true,
            origin: Some(origin.to_string()),
            numeric_value: None,
        }
    }

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).expect("valid date")
    }

    fn datetime(y: i32, m: u32, d: u32, h: u32, min: u32) -> NaiveDateTime {
        date(y, m, d).and_hms_opt(h, min, 0).expect("valid time")
    }

    fn when_category_id(store: &Store) -> CategoryId {
        store
            .get_hierarchy()
            .expect("hierarchy available")
            .into_iter()
            .find(|category| category.name.eq_ignore_ascii_case("When"))
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
        updated.modified_at = Utc::now();

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
        assert_eq!(when_assignment.source, AssignmentSource::AutoMatch);
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
        updated.modified_at = Utc::now();

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
        updated.modified_at = Utc::now();

        agenda
            .update_item_with_reference_date(&updated, date(2026, 2, 16))
            .unwrap();

        let loaded = store.get_item(item.id).unwrap();
        assert_eq!(loaded.when_date, Some(datetime(2026, 2, 17, 0, 0)));
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
    fn remove_from_section_unassigns_targets_and_preserves_others() {
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

        let mut current_section = section("Urgent");
        current_section.on_remove_unassign.insert(urgent.id);

        agenda
            .remove_item_from_section(item.id, &current_section)
            .unwrap();

        let assignments = store.get_assignments_for_item(item.id).unwrap();
        assert!(assignments.contains_key(&work.id));
        assert!(!assignments.contains_key(&urgent.id));
        assert!(assignments.contains_key(&personal.id));
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
            .find(|category| category.name.eq_ignore_ascii_case("Done"))
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
            .find(|category| category.name.eq_ignore_ascii_case("Done"))
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
}
