use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::Result;
use crate::model::{
    CategoryId, CategoryValueKind, Item, ItemId, Query, Section, View, RESERVED_CATEGORY_NAMES,
};
use crate::store::Store;

pub const WORKFLOW_CONFIG_KEY: &str = "workflow.ready_queue.v1";
pub const READY_QUEUE_VIEW_NAME: &str = "Ready Queue";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowConfig {
    #[serde(default)]
    pub ready_category_id: Option<CategoryId>,
    #[serde(default)]
    pub claim_category_id: Option<CategoryId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedWorkflowConfig {
    pub ready_category_id: CategoryId,
    pub claim_category_id: CategoryId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Claimability {
    Claimable,
    Done,
    AlreadyClaimed,
    MissingReady,
    Blocked,
}

impl Claimability {
    pub fn error_message(self) -> Option<&'static str> {
        match self {
            Self::Claimable => None,
            Self::Done => Some("claim precondition failed: item is done"),
            Self::AlreadyClaimed => Some("claim precondition failed: item is already claimed"),
            Self::MissingReady => {
                Some("claim precondition failed: item is missing the configured Ready category")
            }
            Self::Blocked => Some("claim precondition failed: item is dependency-blocked"),
        }
    }
}

pub fn workflow_setup_error_message() -> &'static str {
    "workflow is not configured: set Ready Queue and Claim Target categories in TUI Global Settings"
}

pub fn resolve_workflow_config(store: &Store) -> Result<Option<ResolvedWorkflowConfig>> {
    let config = store.get_workflow_config()?;
    let Some(ready_category_id) = config.ready_category_id else {
        return Ok(None);
    };
    let Some(claim_category_id) = config.claim_category_id else {
        return Ok(None);
    };
    if ready_category_id == claim_category_id {
        return Ok(None);
    }

    let ready = match store.get_category(ready_category_id) {
        Ok(category) => category,
        Err(_) => return Ok(None),
    };
    let claim = match store.get_category(claim_category_id) {
        Ok(category) => category,
        Err(_) => return Ok(None),
    };
    if RESERVED_CATEGORY_NAMES.iter().any(|name| {
        name.eq_ignore_ascii_case(&ready.name) || name.eq_ignore_ascii_case(&claim.name)
    }) {
        return Ok(None);
    }
    if ready.value_kind == CategoryValueKind::Numeric
        || claim.value_kind == CategoryValueKind::Numeric
    {
        return Ok(None);
    }
    Ok(Some(ResolvedWorkflowConfig {
        ready_category_id,
        claim_category_id,
    }))
}

pub fn item_is_dependency_blocked(store: &Store, item_id: ItemId) -> Result<bool> {
    let dependency_ids = store.list_dependency_ids_for_item(item_id)?;
    for dependency_id in dependency_ids {
        if !store.get_item(dependency_id)?.is_done {
            return Ok(true);
        }
    }
    Ok(false)
}

pub fn blocked_item_ids(store: &Store, items: &[Item]) -> Result<HashSet<ItemId>> {
    let done_by_item_id = dependency_done_map(store, items)?;
    let mut blocked = HashSet::new();
    for item in items {
        let dependency_ids = store.list_dependency_ids_for_item(item.id)?;
        if dependency_ids_are_blocked(&dependency_ids, &done_by_item_id) {
            blocked.insert(item.id);
        }
    }
    Ok(blocked)
}

pub fn retain_items_by_dependency_state(
    store: &Store,
    items: &mut Vec<Item>,
    blocked: bool,
) -> Result<()> {
    let blocked_ids = blocked_item_ids(store, items)?;
    items.retain(|item| blocked_ids.contains(&item.id) == blocked);
    Ok(())
}

pub fn claimability_for_item(
    store: &Store,
    item: &Item,
    config: ResolvedWorkflowConfig,
) -> Result<Claimability> {
    if item.is_done {
        return Ok(Claimability::Done);
    }
    if item.assignments.contains_key(&config.claim_category_id) {
        return Ok(Claimability::AlreadyClaimed);
    }
    if !item.assignments.contains_key(&config.ready_category_id) {
        return Ok(Claimability::MissingReady);
    }
    if item_is_dependency_blocked(store, item.id)? {
        return Ok(Claimability::Blocked);
    }
    Ok(Claimability::Claimable)
}

fn dependency_done_map(store: &Store, items: &[Item]) -> Result<HashMap<ItemId, bool>> {
    let mut done_by_item_id: HashMap<ItemId, bool> =
        items.iter().map(|item| (item.id, item.is_done)).collect();

    for item in items {
        for dependency_id in store.list_dependency_ids_for_item(item.id)? {
            done_by_item_id
                .entry(dependency_id)
                .or_insert(store.get_item(dependency_id)?.is_done);
        }
    }

    Ok(done_by_item_id)
}

fn dependency_ids_are_blocked(
    dependency_ids: &[ItemId],
    done_by_item_id: &HashMap<ItemId, bool>,
) -> bool {
    dependency_ids
        .iter()
        .any(|dep_id| !done_by_item_id.get(dep_id).copied().unwrap_or(false))
}

pub fn claimable_item_ids(
    store: &Store,
    items: &[Item],
    config: ResolvedWorkflowConfig,
) -> Result<HashSet<ItemId>> {
    let mut claimable = HashSet::new();
    for item in items {
        if claimability_for_item(store, item, config)? == Claimability::Claimable {
            claimable.insert(item.id);
        }
    }
    Ok(claimable)
}

pub fn build_ready_queue_view(store: &Store, config: ResolvedWorkflowConfig) -> Result<View> {
    let ready_category = store.get_category(config.ready_category_id)?;
    let mut view = View::new(READY_QUEUE_VIEW_NAME.to_string());
    view.id = Uuid::from_u128(0x0f6e_9f74_5a9b_4b0c_b376_0cfd_0c5a_8b14);
    view.show_unmatched = false;
    view.sections.push(Section {
        title: ready_category.name,
        criteria: Query::default(),
        columns: Vec::new(),
        item_column_index: 0,
        on_insert_assign: HashSet::new(),
        on_remove_unassign: HashSet::new(),
        show_children: false,
        board_display_mode_override: None,
    });
    Ok(view)
}

#[cfg(test)]
mod tests {
    use super::{
        blocked_item_ids, item_is_dependency_blocked, retain_items_by_dependency_state,
        workflow_setup_error_message,
    };
    use crate::matcher::SubstringClassifier;
    use crate::model::Item;
    use crate::store::Store;
    use crate::workspace::Workspace;

    #[test]
    fn workflow_setup_error_message_points_to_global_settings() {
        assert_eq!(
            workflow_setup_error_message(),
            "workflow is not configured: set Ready Queue and Claim Target categories in TUI Global Settings"
        );
    }

    #[test]
    fn item_is_dependency_blocked_returns_true_for_unresolved_dependency() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let workspace = Workspace::new(&store, &classifier);

        let blocker = Item::new("Blocker".to_string());
        let blocked = Item::new("Blocked".to_string());
        store.create_item(&blocker).expect("create blocker");
        store.create_item(&blocked).expect("create blocked");
        workspace
            .link_items_depends_on(blocked.id, blocker.id)
            .expect("link depends-on");

        assert!(
            item_is_dependency_blocked(&store, blocked.id).expect("blocked check"),
            "open dependency should block the item"
        );
        assert!(
            !item_is_dependency_blocked(&store, blocker.id).expect("blocker check"),
            "item with no dependencies should not be blocked"
        );
    }

    #[test]
    fn item_is_dependency_blocked_returns_false_when_dependency_is_done() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let workspace = Workspace::new(&store, &classifier);

        let mut blocker = Item::new("Blocker".to_string());
        let blocked = Item::new("Blocked".to_string());
        store.create_item(&blocker).expect("create blocker");
        store.create_item(&blocked).expect("create blocked");
        workspace
            .link_items_depends_on(blocked.id, blocker.id)
            .expect("link depends-on");
        blocker.is_done = true;
        store.update_item(&blocker).expect("mark blocker done");

        assert!(
            !item_is_dependency_blocked(&store, blocked.id).expect("blocked check"),
            "done dependency should not block the item"
        );
    }

    #[test]
    fn blocked_item_ids_matches_single_item_checks() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let workspace = Workspace::new(&store, &classifier);

        let blocker = Item::new("Blocker".to_string());
        let blocked = Item::new("Blocked".to_string());
        let free = Item::new("Free".to_string());
        store.create_item(&blocker).expect("create blocker");
        store.create_item(&blocked).expect("create blocked");
        store.create_item(&free).expect("create free");
        workspace
            .link_items_depends_on(blocked.id, blocker.id)
            .expect("link depends-on");

        let items = store.list_items().expect("list items");
        let blocked_ids = blocked_item_ids(&store, &items).expect("blocked ids");

        for item in &items {
            assert_eq!(
                blocked_ids.contains(&item.id),
                item_is_dependency_blocked(&store, item.id).expect("single blocked check"),
                "batch blocked ids should match single-item blocked checks for {}",
                item.text
            );
        }
    }

    #[test]
    fn retain_items_by_dependency_state_filters_blocked_and_not_blocked_items() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let workspace = Workspace::new(&store, &classifier);

        let blocker = Item::new("Blocker".to_string());
        let blocked = Item::new("Blocked".to_string());
        let free = Item::new("Free".to_string());
        store.create_item(&blocker).expect("create blocker");
        store.create_item(&blocked).expect("create blocked");
        store.create_item(&free).expect("create free");
        workspace
            .link_items_depends_on(blocked.id, blocker.id)
            .expect("link depends-on");

        let mut blocked_rows = store.list_items().expect("list items");
        retain_items_by_dependency_state(&store, &mut blocked_rows, true)
            .expect("retain blocked items");
        assert_eq!(
            blocked_rows
                .into_iter()
                .map(|item| item.text)
                .collect::<Vec<_>>(),
            vec!["Blocked".to_string()]
        );

        let mut not_blocked_rows = store.list_items().expect("list items");
        retain_items_by_dependency_state(&store, &mut not_blocked_rows, false)
            .expect("retain not blocked items");
        let mut texts = not_blocked_rows
            .into_iter()
            .map(|item| item.text)
            .collect::<Vec<_>>();
        texts.sort();
        assert_eq!(texts, vec!["Blocker".to_string(), "Free".to_string()]);
    }
}
