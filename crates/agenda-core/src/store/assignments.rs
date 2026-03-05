use std::collections::HashMap;

use rusqlite::params;

use crate::error::Result;
use crate::model::{Assignment, AssignmentSource, CategoryId, Item, ItemId};

use super::Store;

impl Store {
    // ── Assignment persistence ──────────────────────────────────

    /// Assign an item to a category. If the assignment already exists, it is
    /// replaced (upsert).
    pub fn assign_item(
        &self,
        item_id: ItemId,
        category_id: CategoryId,
        assignment: &Assignment,
    ) -> Result<()> {
        let source_str = match assignment.source {
            AssignmentSource::Manual => "Manual",
            AssignmentSource::AutoMatch => "AutoMatch",
            AssignmentSource::Action => "Action",
            AssignmentSource::Subsumption => "Subsumption",
        };
        self.conn.execute(
            "INSERT OR REPLACE INTO assignments (item_id, category_id, source, assigned_at, sticky, origin, numeric_value)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                item_id.to_string(),
                category_id.to_string(),
                source_str,
                assignment.assigned_at.to_rfc3339(),
                assignment.sticky as i32,
                assignment.origin,
                assignment.numeric_value.map(|v| v.to_string()),
            ],
        )?;
        Ok(())
    }

    /// Remove an assignment. Returns Ok even if the assignment didn't exist.
    pub fn unassign_item(&self, item_id: ItemId, category_id: CategoryId) -> Result<()> {
        self.conn.execute(
            "DELETE FROM assignments WHERE item_id = ?1 AND category_id = ?2",
            params![item_id.to_string(), category_id.to_string()],
        )?;
        Ok(())
    }

    /// Get all assignments for an item as a HashMap.
    pub fn get_assignments_for_item(
        &self,
        item_id: ItemId,
    ) -> Result<HashMap<CategoryId, Assignment>> {
        let mut item = Item::new(String::new());
        item.id = item_id;
        let item = self.load_assignments(item)?;
        Ok(item.assignments)
    }
}
