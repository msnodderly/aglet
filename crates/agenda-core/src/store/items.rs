use std::collections::HashMap;

use chrono::Utc;
use rusqlite::params;
use serde_json;
use uuid::Uuid;

use crate::error::{AgendaError, Result};
use crate::model::{Assignment, CategoryId, DeletionLogEntry, Item, ItemId};

use super::Store;

impl Store {
    // ── Item CRUD ──────────────────────────────────────────────

    pub fn create_item(&self, item: &Item) -> Result<()> {
        self.conn.execute(
            "INSERT INTO items (id, text, note, created_at, modified_at, entry_date, when_date, done_date, is_done)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                item.id.to_string(),
                item.text,
                item.note,
                item.created_at.to_rfc3339(),
                item.modified_at.to_rfc3339(),
                item.entry_date.to_string(),
                item.when_date.map(|d| d.to_string()),
                item.done_date.map(|d| d.to_string()),
                item.is_done as i32,
            ],
        )?;
        Ok(())
    }

    /// Resolve a short UUID prefix to a full ItemId.
    ///
    /// The prefix is matched case-insensitively against the start of stored item
    /// UUIDs (hyphen-normalized). Returns an error if zero or multiple items match.
    pub fn resolve_item_prefix(&self, prefix: &str) -> Result<ItemId> {
        let normalized = prefix.to_lowercase().replace('-', "");
        if normalized.is_empty() {
            return Err(AgendaError::InvalidOperation {
                message: "empty item id prefix".to_string(),
            });
        }
        // Only allow valid hex characters
        if !normalized.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(AgendaError::InvalidOperation {
                message: format!("invalid item id prefix: {prefix}"),
            });
        }
        let pattern = format!("{normalized}%");
        let mut stmt = self
            .conn
            .prepare("SELECT id FROM items WHERE REPLACE(LOWER(id), '-', '') LIKE ?1")?;
        let matches: Vec<String> = stmt
            .query_map(params![pattern], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        match matches.len() {
            0 => Err(AgendaError::InvalidOperation {
                message: format!("no item found matching prefix: {prefix}"),
            }),
            1 => {
                let id = Uuid::parse_str(&matches[0]).map_err(|e| AgendaError::StorageError {
                    source: Box::new(e),
                })?;
                Ok(id)
            }
            _ => Err(AgendaError::AmbiguousId {
                prefix: prefix.to_string(),
                matches,
            }),
        }
    }

    pub fn get_item(&self, id: ItemId) -> Result<Item> {
        let mut stmt = self.conn.prepare(
            "SELECT id, text, note, created_at, modified_at, entry_date, when_date, done_date, is_done
             FROM items WHERE id = ?1",
        )?;
        let item = stmt
            .query_row(params![id.to_string()], Self::row_to_item)
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    AgendaError::NotFound { entity: "Item", id }
                }
                other => AgendaError::from(other),
            })?;
        self.load_assignments(item)
    }

    pub fn update_item(&self, item: &Item) -> Result<()> {
        let rows = self.conn.execute(
            "UPDATE items SET text = ?1, note = ?2, modified_at = ?3, when_date = ?4, done_date = ?5, is_done = ?6
             WHERE id = ?7",
            params![
                item.text,
                item.note,
                item.modified_at.to_rfc3339(),
                item.when_date.map(|d| d.to_string()),
                item.done_date.map(|d| d.to_string()),
                item.is_done as i32,
                item.id.to_string(),
            ],
        )?;
        if rows == 0 {
            return Err(AgendaError::NotFound {
                entity: "Item",
                id: item.id,
            });
        }
        Ok(())
    }

    /// Delete an item. Writes to deletion_log first, then removes from items table.
    pub fn delete_item(&self, id: ItemId, deleted_by: &str) -> Result<()> {
        let item = self.get_item(id)?;
        let assignments_json =
            serde_json::to_string(&item.assignments)
                .expect("BTreeMap<CategoryId, Assignment> is always serialisable");

        self.conn.execute(
            "INSERT INTO deletion_log (id, item_id, text, note, entry_date, when_date, done_date, is_done, assignments_json, deleted_at, deleted_by)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                Uuid::new_v4().to_string(),
                item.id.to_string(),
                item.text,
                item.note,
                item.entry_date.to_string(),
                item.when_date.map(|d| d.to_string()),
                item.done_date.map(|d| d.to_string()),
                item.is_done as i32,
                assignments_json,
                Utc::now().to_rfc3339(),
                deleted_by,
            ],
        )?;

        // CASCADE deletes assignments automatically.
        self.conn
            .execute("DELETE FROM items WHERE id = ?1", params![id.to_string()])?;
        Ok(())
    }

    pub fn list_items(&self) -> Result<Vec<Item>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, text, note, created_at, modified_at, entry_date, when_date, done_date, is_done
             FROM items ORDER BY created_at DESC",
        )?;
        let rows = stmt
            .query_map([], Self::row_to_item)?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        rows.into_iter()
            .map(|item| self.load_assignments(item))
            .collect()
    }

    pub fn list_deleted_items(&self) -> Result<Vec<DeletionLogEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, item_id, text, note, entry_date, when_date, done_date, is_done,
                    assignments_json, deleted_at, deleted_by
             FROM deletion_log
             ORDER BY deleted_at DESC",
        )?;
        let rows = stmt
            .query_map([], Self::row_to_deleted_item)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn restore_deleted_item(&self, log_entry_id: Uuid) -> Result<ItemId> {
        let mut stmt = self.conn.prepare(
            "SELECT id, item_id, text, note, entry_date, when_date, done_date, is_done,
                    assignments_json, deleted_at, deleted_by
             FROM deletion_log
             WHERE id = ?1",
        )?;
        let entry = stmt
            .query_row(params![log_entry_id.to_string()], Self::row_to_deleted_item)
            .map_err(|err| match err {
                rusqlite::Error::QueryReturnedNoRows => AgendaError::NotFound {
                    entity: "DeletionLogEntry",
                    id: log_entry_id,
                },
                other => AgendaError::from(other),
            })?;

        if self.get_item(entry.item_id).is_ok() {
            return Err(AgendaError::InvalidOperation {
                message: format!("item {} already exists", entry.item_id),
            });
        }

        let now = Utc::now();
        let item = Item {
            id: entry.item_id,
            text: entry.text,
            note: entry.note,
            created_at: now,
            modified_at: now,
            entry_date: entry.entry_date,
            when_date: entry.when_date,
            done_date: entry.done_date,
            is_done: entry.is_done,
            assignments: HashMap::new(),
        };
        self.create_item(&item)?;

        // Corrupt or legacy deletion-log row: restore item without assignments.
        let assignments: HashMap<CategoryId, Assignment> =
            serde_json::from_str(&entry.assignments_json).unwrap_or_default();
        for (category_id, assignment) in assignments {
            if self.get_category(category_id).is_err() {
                continue;
            }
            self.assign_item(item.id, category_id, &assignment)?;
        }

        Ok(item.id)
    }
}
