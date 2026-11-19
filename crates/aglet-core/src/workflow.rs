use std::collections::HashSet;

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

/// The subset of `items` with an unresolved `depends-on` prerequisite. One
/// query regardless of how many items are passed.
pub fn blocked_item_ids(store: &Store, items: &[Item]) -> Result<HashSet<ItemId>> {
    let blocked = store.list_blocked_item_ids()?;
    Ok(items
        .iter()
        .map(|item| item.id)
        .filter(|item_id| blocked.contains(item_id))
        .collect())
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
    claimability(item, config, || item_is_dependency_blocked(store, item.id))
}

/// The subset of `items` that `claimability_for_item` would report as
/// `Claimable`, with the dependency check done once for the whole list.
pub fn claimable_item_ids(
    store: &Store,
    items: &[Item],
    config: ResolvedWorkflowConfig,
) -> Result<HashSet<ItemId>> {
    let blocked = store.list_blocked_item_ids()?;
    let mut claimable = HashSet::new();
    for item in items {
        if claimability(item, config, || Ok(blocked.contains(&item.id)))? == Claimability::Claimable
        {
            claimable.insert(item.id);
        }
    }
    Ok(claimable)
}

/// The claim rule, shared by `aglet claim` and `aglet ready` so they cannot
/// disagree. `is_blocked` is only called once the cheaper checks pass.
fn claimability(
    item: &Item,
    config: ResolvedWorkflowConfig,
    is_blocked: impl FnOnce() -> Result<bool>,
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
    if is_blocked()? {
        return Ok(Claimability::Blocked);
    }
    Ok(Claimability::Claimable)
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
        blocked_item_ids, claimability_for_item, claimable_item_ids, item_is_dependency_blocked,
        retain_items_by_dependency_state, workflow_setup_error_message, Claimability,
        ResolvedWorkflowConfig,
    };
    use crate::aglet::Aglet;
    use crate::matcher::SubstringClassifier;
    use crate::model::{Category, Item};
    use crate::store::Store;

    /// The whole-table queries behind TUI refresh and the ready queue must
    /// agree with the per-item queries they replaced.
    #[test]
    fn batch_link_and_claim_queries_match_per_item_queries() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let aglet = Aglet::new(&store, &classifier);

        let ready = Category::new("Ready".to_string());
        let claimed = Category::new("In Progress".to_string());
        store.create_category(&ready).expect("ready");
        store.create_category(&claimed).expect("claimed");
        let config = ResolvedWorkflowConfig {
            ready_category_id: ready.id,
            claim_category_id: claimed.id,
        };

        let item = |text: &str| {
            let item = Item::new(text.to_string());
            store.create_item(&item).expect("create item");
            item.id
        };
        let free = item("free and ready");
        let waiting = item("ready, open dependency");
        let open_dep = item("open dependency");
        let unblocked = item("ready, done dependency");
        let done_dep = item("done dependency");
        let taken = item("ready and claimed");
        let finished = item("ready but done");

        for id in [done_dep, finished] {
            let mut done = store.get_item(id).expect("get");
            done.is_done = true;
            store.update_item(&done).expect("mark done");
        }

        for id in [free, waiting, unblocked, taken, finished] {
            aglet.assign_item_manual(id, ready.id, None).expect("ready");
        }
        aglet
            .assign_item_manual(taken, claimed.id, None)
            .expect("claim");
        aglet
            .link_items_depends_on(waiting, open_dep)
            .expect("link");
        aglet.link_items_depends_on(waiting, free).expect("link");
        aglet
            .link_items_depends_on(unblocked, done_dep)
            .expect("link");
        aglet.link_items_related(free, open_dep).expect("link");
        aglet.link_items_related(taken, free).expect("link");

        let items = store.list_items().expect("list items");
        let links = store.list_item_links_by_item().expect("links");
        let blocked = blocked_item_ids(&store, &items).expect("blocked");
        let claimable = claimable_item_ids(&store, &items, config).expect("claimable");

        for item in &items {
            let batch = links.get(&item.id).cloned().unwrap_or_default();
            assert_eq!(
                batch.depends_on,
                store.list_dependency_ids_for_item(item.id).expect("deps"),
                "depends_on for {}",
                item.text
            );
            assert_eq!(
                batch.blocks,
                store.list_dependent_ids_for_item(item.id).expect("blocks"),
                "blocks for {}",
                item.text
            );
            assert_eq!(
                batch.related,
                store.list_related_ids_for_item(item.id).expect("related"),
                "related for {}",
                item.text
            );
            assert_eq!(
                blocked.contains(&item.id),
                item_is_dependency_blocked(&store, item.id).expect("blocked check"),
                "blocked for {}",
                item.text
            );
            assert_eq!(
                claimable.contains(&item.id),
                claimability_for_item(&store, item, config).expect("claimability")
                    == Claimability::Claimable,
                "claimable for {}",
                item.text
            );
        }
        assert_eq!(blocked, std::collections::HashSet::from([waiting]));
        assert_eq!(
            claimable,
            std::collections::HashSet::from([free, unblocked])
        );
    }

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
        let aglet = Aglet::new(&store, &classifier);

        let blocker = Item::new("Blocker".to_string());
        let blocked = Item::new("Blocked".to_string());
        store.create_item(&blocker).expect("create blocker");
        store.create_item(&blocked).expect("create blocked");
        aglet
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
        let aglet = Aglet::new(&store, &classifier);

        let mut blocker = Item::new("Blocker".to_string());
        let blocked = Item::new("Blocked".to_string());
        store.create_item(&blocker).expect("create blocker");
        store.create_item(&blocked).expect("create blocked");
        aglet
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
        let aglet = Aglet::new(&store, &classifier);

        let blocker = Item::new("Blocker".to_string());
        let blocked = Item::new("Blocked".to_string());
        let free = Item::new("Free".to_string());
        store.create_item(&blocker).expect("create blocker");
        store.create_item(&blocked).expect("create blocked");
        store.create_item(&free).expect("create free");
        aglet
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
        let aglet = Aglet::new(&store, &classifier);

        let blocker = Item::new("Blocker".to_string());
        let blocked = Item::new("Blocked".to_string());
        let free = Item::new("Free".to_string());
        store.create_item(&blocker).expect("create blocker");
        store.create_item(&blocked).expect("create blocked");
        store.create_item(&free).expect("create free");
        aglet
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
