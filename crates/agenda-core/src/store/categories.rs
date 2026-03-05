use std::collections::HashMap;

use chrono::Utc;
use rusqlite::params;
use serde_json;

use crate::error::{AgendaError, Result};
use crate::model::{Category, CategoryId};

use super::Store;

impl Store {
    // ── Category CRUD ───────────────────────────────────────────

    pub fn create_category(&self, category: &Category) -> Result<()> {
        if Self::is_reserved_category_name(&category.name) {
            return Err(AgendaError::ReservedName {
                name: category.name.clone(),
            });
        }
        Self::validate_category_type_shape(category)?;

        if let Some(parent_id) = category.parent {
            // Ensure parent exists so callers get a deterministic NotFound error.
            let parent = self.get_category(parent_id)?;
            Self::validate_parent_accepts_children(&parent)?;
        }

        let conditions_json = serde_json::to_string(&category.conditions).map_err(|err| {
            AgendaError::StorageError {
                source: Box::new(err),
            }
        })?;
        let actions_json =
            serde_json::to_string(&category.actions).map_err(|err| AgendaError::StorageError {
                source: Box::new(err),
            })?;
        let numeric_format_json =
            serde_json::to_string(&category.numeric_format).map_err(|err| {
                AgendaError::StorageError {
                    source: Box::new(err),
                }
            })?;

        let sort_order = self.next_category_sort_order(category.parent)?;

        self.conn
            .execute(
                "INSERT INTO categories (
                    id, name, parent_id, is_exclusive, is_actionable, enable_implicit_string, note,
                    created_at, modified_at, sort_order, conditions_json, actions_json,
                    value_kind, numeric_format_json
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                params![
                    category.id.to_string(),
                    category.name,
                    category.parent.map(|id| id.to_string()),
                    category.is_exclusive as i32,
                    category.is_actionable as i32,
                    category.enable_implicit_string as i32,
                    category.note,
                    category.created_at.to_rfc3339(),
                    category.modified_at.to_rfc3339(),
                    sort_order,
                    conditions_json,
                    actions_json,
                    Self::category_value_kind_to_db(category.value_kind),
                    numeric_format_json,
                ],
            )
            .map_err(|err| Self::map_category_write_error(err, &category.name))?;

        Ok(())
    }

    pub fn get_category(&self, id: CategoryId) -> Result<Category> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, parent_id, is_exclusive, is_actionable, enable_implicit_string, note,
                    created_at, modified_at, conditions_json, actions_json, sort_order,
                    value_kind, numeric_format_json
             FROM categories WHERE id = ?1",
        )?;
        let (mut category, _) = stmt
            .query_row(params![id.to_string()], Self::row_to_category)
            .map_err(|err| match err {
                rusqlite::Error::QueryReturnedNoRows => AgendaError::NotFound {
                    entity: "Category",
                    id,
                },
                other => AgendaError::from(other),
            })?;

        category.children = self.get_child_category_ids(category.id)?;
        Ok(category)
    }

    pub fn update_category(&self, category: &Category) -> Result<()> {
        let existing = self.get_category(category.id)?;

        // Reserved categories can be updated, but they cannot be renamed or
        // impersonated by non-reserved categories.
        if Self::is_reserved_category_name(&category.name)
            && !existing.name.eq_ignore_ascii_case(&category.name)
        {
            return Err(AgendaError::ReservedName {
                name: category.name.clone(),
            });
        }
        if Self::is_reserved_category_name(&existing.name)
            && !existing.name.eq_ignore_ascii_case(&category.name)
        {
            return Err(AgendaError::ReservedName {
                name: existing.name,
            });
        }
        if category.parent == Some(category.id) {
            return Err(AgendaError::InvalidOperation {
                message: "category cannot be its own parent".to_string(),
            });
        }
        Self::validate_category_type_shape(category)?;
        self.validate_category_parent(category.id, category.parent)?;
        self.validate_category_type_transition(&existing, category)?;

        if let Some(parent_id) = category.parent {
            let parent = self.get_category(parent_id)?;
            Self::validate_parent_accepts_children(&parent)?;
        }

        let conditions_json = serde_json::to_string(&category.conditions).map_err(|err| {
            AgendaError::StorageError {
                source: Box::new(err),
            }
        })?;
        let actions_json =
            serde_json::to_string(&category.actions).map_err(|err| AgendaError::StorageError {
                source: Box::new(err),
            })?;
        let numeric_format_json =
            serde_json::to_string(&category.numeric_format).map_err(|err| {
                AgendaError::StorageError {
                    source: Box::new(err),
                }
            })?;
        let modified_at = Utc::now();

        self.conn
            .execute(
                "UPDATE categories
                 SET name = ?1,
                     parent_id = ?2,
                     is_exclusive = ?3,
                     is_actionable = ?4,
                     enable_implicit_string = ?5,
                     note = ?6,
                     modified_at = ?7,
                     conditions_json = ?8,
                     actions_json = ?9,
                     value_kind = ?10,
                     numeric_format_json = ?11
                 WHERE id = ?12",
                params![
                    category.name,
                    category.parent.map(|id| id.to_string()),
                    category.is_exclusive as i32,
                    category.is_actionable as i32,
                    category.enable_implicit_string as i32,
                    category.note,
                    modified_at.to_rfc3339(),
                    conditions_json,
                    actions_json,
                    Self::category_value_kind_to_db(category.value_kind),
                    numeric_format_json,
                    category.id.to_string(),
                ],
            )
            .map_err(|err| Self::map_category_write_error(err, &category.name))?;

        Ok(())
    }

    /// Reorder a category among its siblings by `delta` positions.
    /// Out-of-range moves are treated as no-ops.
    pub fn move_category_within_parent(&self, category_id: CategoryId, delta: i32) -> Result<()> {
        if delta == 0 {
            return Ok(());
        }

        let category = self.get_category(category_id)?;
        let parent_id = category.parent;
        let mut siblings = self.list_category_ids_for_parent(parent_id)?;
        let Some(from_index) = siblings.iter().position(|id| *id == category_id) else {
            return Err(AgendaError::NotFound {
                entity: "Category",
                id: category_id,
            });
        };

        let to_index = (from_index as i64 + delta as i64).clamp(0, siblings.len() as i64 - 1);
        let to_index = to_index as usize;
        if from_index == to_index {
            return Ok(());
        }

        let moved = siblings.remove(from_index);
        siblings.insert(to_index, moved);

        self.with_category_order_transaction(|store| {
            store.rewrite_category_sort_orders_for_parent(parent_id, &siblings)
        })
    }

    /// Reparent a category and optionally place it at a specific index among the
    /// new parent's children. `None` appends to the end.
    pub fn move_category_to_parent(
        &self,
        category_id: CategoryId,
        new_parent_id: Option<CategoryId>,
        insert_index: Option<usize>,
    ) -> Result<()> {
        let category = self.get_category(category_id)?;
        if new_parent_id == Some(category_id) {
            return Err(AgendaError::InvalidOperation {
                message: "category cannot be its own parent".to_string(),
            });
        }
        if let Some(parent_id) = new_parent_id {
            let parent = self.get_category(parent_id)?;
            Self::validate_parent_accepts_children(&parent)?;
        }
        self.validate_category_parent(category_id, new_parent_id)?;

        let old_parent_id = category.parent;
        let mut old_siblings = self.list_category_ids_for_parent(old_parent_id)?;
        let Some(old_index) = old_siblings.iter().position(|id| *id == category_id) else {
            return Err(AgendaError::NotFound {
                entity: "Category",
                id: category_id,
            });
        };
        old_siblings.remove(old_index);

        let mut new_siblings = if old_parent_id == new_parent_id {
            old_siblings.clone()
        } else {
            self.list_category_ids_for_parent(new_parent_id)?
                .into_iter()
                .filter(|id| *id != category_id)
                .collect()
        };
        let next_index = insert_index
            .unwrap_or(new_siblings.len())
            .min(new_siblings.len());
        new_siblings.insert(next_index, category_id);

        self.with_category_order_transaction(|store| {
            if old_parent_id != new_parent_id {
                store.update_category_parent_only(category_id, new_parent_id)?;
                store.rewrite_category_sort_orders_for_parent(old_parent_id, &old_siblings)?;
            }
            store.rewrite_category_sort_orders_for_parent(new_parent_id, &new_siblings)?;
            Ok(())
        })
    }

    pub fn delete_category(&self, id: CategoryId) -> Result<()> {
        let category = self.get_category(id)?;
        if Self::is_reserved_category_name(&category.name) {
            return Err(AgendaError::ReservedName {
                name: category.name,
            });
        }
        if !category.children.is_empty() {
            return Err(AgendaError::InvalidOperation {
                message: format!(
                    "cannot delete category {} while it still has children",
                    category.name
                ),
            });
        }

        let rows = self.conn.execute(
            "DELETE FROM categories WHERE id = ?1",
            params![id.to_string()],
        )?;
        if rows == 0 {
            return Err(AgendaError::NotFound {
                entity: "Category",
                id,
            });
        }
        Ok(())
    }

    pub fn get_hierarchy(&self) -> Result<Vec<Category>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, parent_id, is_exclusive, is_actionable, enable_implicit_string, note,
                    created_at, modified_at, conditions_json, actions_json, sort_order,
                    value_kind, numeric_format_json
             FROM categories
             ORDER BY sort_order ASC, name COLLATE NOCASE ASC",
        )?;

        let category_rows = stmt
            .query_map([], Self::row_to_category)?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let mut categories_by_id = HashMap::new();
        let mut child_ids_by_parent: HashMap<Option<CategoryId>, Vec<(i64, CategoryId)>> =
            HashMap::new();

        for (category, sort_order) in category_rows {
            child_ids_by_parent
                .entry(category.parent)
                .or_default()
                .push((sort_order, category.id));
            categories_by_id.insert(category.id, category);
        }

        for child_ids in child_ids_by_parent.values_mut() {
            child_ids.sort_by_key(|(sort_order, child_id)| (*sort_order, *child_id));
        }

        let category_ids: Vec<CategoryId> = categories_by_id.keys().copied().collect();
        for category_id in category_ids {
            let children = child_ids_by_parent
                .get(&Some(category_id))
                .map(|child_ids| child_ids.iter().map(|(_, child_id)| *child_id).collect())
                .unwrap_or_default();

            if let Some(category) = categories_by_id.get_mut(&category_id) {
                category.children = children;
            }
        }

        let mut ordered = Vec::new();
        if let Some(root_ids) = child_ids_by_parent.get(&None) {
            for (_, root_id) in root_ids {
                Self::flatten_hierarchy(
                    *root_id,
                    &categories_by_id,
                    &child_ids_by_parent,
                    &mut ordered,
                );
            }
        }

        Ok(ordered)
    }
}
