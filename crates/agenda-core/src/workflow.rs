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
    use super::workflow_setup_error_message;

    #[test]
    fn workflow_setup_error_message_points_to_global_settings() {
        assert_eq!(
            workflow_setup_error_message(),
            "workflow is not configured: set Ready Queue and Claim Target categories in TUI Global Settings"
        );
    }
}
