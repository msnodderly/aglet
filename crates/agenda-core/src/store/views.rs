use rusqlite::params;
use serde_json;
use uuid::Uuid;

use crate::error::{AgendaError, Result};
use crate::model::View;

use super::Store;

impl Store {
    // ── View CRUD ───────────────────────────────────────────────

    pub fn create_view(&self, view: &View) -> Result<()> {
        if Self::is_system_view_name(&view.name) {
            return Err(AgendaError::InvalidOperation {
                message: format!("cannot create system view: {}", view.name),
            });
        }

        self.insert_view(view)
    }

    pub fn clone_view(&self, source_id: Uuid, new_name: String) -> Result<View> {
        let mut cloned = self.get_view(source_id)?;
        cloned.id = Uuid::new_v4();
        cloned.name = new_name;
        self.create_view(&cloned)?;
        Ok(cloned)
    }

    pub fn get_view(&self, id: Uuid) -> Result<View> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, criteria_json, sections_json, columns_json,
                    show_unmatched, unmatched_label, remove_from_view_unassign_json,
                    category_aliases_json, item_column_label, board_display_mode
             FROM views WHERE id = ?1",
        )?;
        stmt.query_row(params![id.to_string()], Self::row_to_view)
            .map_err(|err| match err {
                rusqlite::Error::QueryReturnedNoRows => {
                    AgendaError::NotFound { entity: "View", id }
                }
                other => AgendaError::from(other),
            })
    }

    pub fn update_view(&self, view: &View) -> Result<()> {
        let existing = self.get_view(view.id)?;
        if Self::is_system_view_name(&existing.name) {
            return Err(AgendaError::InvalidOperation {
                message: format!("cannot modify system view: {}", existing.name),
            });
        }
        if Self::is_system_view_name(&view.name) {
            return Err(AgendaError::InvalidOperation {
                message: format!(
                    "cannot rename view {} to reserved system view name: {}",
                    existing.name, view.name
                ),
            });
        }

        let criteria_json =
            serde_json::to_string(&view.criteria).map_err(|err| AgendaError::StorageError {
                source: Box::new(err),
            })?;
        let sections_json =
            serde_json::to_string(&view.sections).map_err(|err| AgendaError::StorageError {
                source: Box::new(err),
            })?;
        let remove_from_view_unassign_json = serde_json::to_string(&view.remove_from_view_unassign)
            .map_err(|err| AgendaError::StorageError {
                source: Box::new(err),
            })?;
        let category_aliases_json =
            serde_json::to_string(&view.category_aliases).map_err(|err| {
                AgendaError::StorageError {
                    source: Box::new(err),
                }
            })?;

        let rows = self
            .conn
            .execute(
                "UPDATE views
                 SET name = ?1,
                     criteria_json = ?2,
                     sections_json = ?3,
                     columns_json = ?4,
                     show_unmatched = ?5,
                     unmatched_label = ?6,
                     remove_from_view_unassign_json = ?7,
                     category_aliases_json = ?8,
                     item_column_label = ?9,
                     board_display_mode = ?10
                 WHERE id = ?11",
                params![
                    view.name,
                    criteria_json,
                    sections_json,
                    "[]",
                    view.show_unmatched as i32,
                    view.unmatched_label,
                    remove_from_view_unassign_json,
                    category_aliases_json,
                    view.item_column_label,
                    serde_json::to_string(&view.board_display_mode)
                        .unwrap_or_else(|_| "\"SingleLine\"".to_string()),
                    view.id.to_string(),
                ],
            )
            .map_err(|err| Self::map_view_write_error(err, &view.name))?;
        if rows == 0 {
            return Err(AgendaError::NotFound {
                entity: "View",
                id: view.id,
            });
        }
        Ok(())
    }

    pub fn list_views(&self) -> Result<Vec<View>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, criteria_json, sections_json, columns_json,
                    show_unmatched, unmatched_label, remove_from_view_unassign_json,
                    category_aliases_json, item_column_label, board_display_mode
             FROM views
             ORDER BY name COLLATE NOCASE ASC",
        )?;
        let rows = stmt
            .query_map([], Self::row_to_view)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn delete_view(&self, id: Uuid) -> Result<()> {
        let existing = self.get_view(id)?;
        if Self::is_system_view_name(&existing.name) {
            return Err(AgendaError::InvalidOperation {
                message: format!("cannot modify system view: {}", existing.name),
            });
        }

        let rows = self
            .conn
            .execute("DELETE FROM views WHERE id = ?1", params![id.to_string()])?;
        if rows == 0 {
            return Err(AgendaError::NotFound { entity: "View", id });
        }
        Ok(())
    }
}
