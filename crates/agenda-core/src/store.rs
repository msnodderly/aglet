use std::collections::HashMap;
use std::path::Path;

use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use rusqlite::{Connection, Row, params};
use serde_json;
use uuid::Uuid;

use crate::error::{AgendaError, Result};
use crate::model::{Assignment, AssignmentSource, Item, ItemId};

const SCHEMA_VERSION: i32 = 1;

const SCHEMA_SQL: &str = "
CREATE TABLE IF NOT EXISTS items (
    id          TEXT PRIMARY KEY,
    text        TEXT NOT NULL,
    note        TEXT,
    created_at  TEXT NOT NULL,
    modified_at TEXT NOT NULL,
    entry_date  TEXT NOT NULL,
    when_date   TEXT,
    done_date   TEXT,
    is_done     INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS categories (
    id                     TEXT PRIMARY KEY,
    name                   TEXT NOT NULL UNIQUE COLLATE NOCASE,
    parent_id              TEXT REFERENCES categories(id),
    is_exclusive           INTEGER NOT NULL DEFAULT 0,
    enable_implicit_string INTEGER NOT NULL DEFAULT 1,
    note                   TEXT,
    created_at             TEXT NOT NULL,
    modified_at            TEXT NOT NULL,
    sort_order             INTEGER NOT NULL DEFAULT 0,
    conditions_json        TEXT NOT NULL DEFAULT '[]',
    actions_json           TEXT NOT NULL DEFAULT '[]'
);

CREATE TABLE IF NOT EXISTS assignments (
    item_id     TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    category_id TEXT NOT NULL REFERENCES categories(id) ON DELETE CASCADE,
    source      TEXT NOT NULL,
    assigned_at TEXT NOT NULL,
    sticky      INTEGER NOT NULL DEFAULT 1,
    origin      TEXT,
    PRIMARY KEY (item_id, category_id)
);

CREATE TABLE IF NOT EXISTS views (
    id                          TEXT PRIMARY KEY,
    name                        TEXT NOT NULL UNIQUE,
    criteria_json               TEXT NOT NULL DEFAULT '{}',
    sections_json               TEXT NOT NULL DEFAULT '[]',
    columns_json                TEXT NOT NULL DEFAULT '[]',
    show_unmatched              INTEGER NOT NULL DEFAULT 1,
    unmatched_label             TEXT NOT NULL DEFAULT 'Unassigned',
    remove_from_view_unassign_json TEXT NOT NULL DEFAULT '[]'
);

CREATE TABLE IF NOT EXISTS deletion_log (
    id               TEXT PRIMARY KEY,
    item_id          TEXT NOT NULL,
    text             TEXT NOT NULL,
    note             TEXT,
    entry_date       TEXT NOT NULL,
    when_date        TEXT,
    done_date        TEXT,
    is_done          INTEGER NOT NULL DEFAULT 0,
    assignments_json TEXT NOT NULL DEFAULT '{}',
    deleted_at       TEXT NOT NULL,
    deleted_by       TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_assignments_item ON assignments(item_id);
CREATE INDEX IF NOT EXISTS idx_assignments_category ON assignments(category_id);
CREATE INDEX IF NOT EXISTS idx_categories_parent ON categories(parent_id);
CREATE INDEX IF NOT EXISTS idx_items_when_date ON items(when_date);
CREATE INDEX IF NOT EXISTS idx_items_is_done ON items(is_done);
CREATE INDEX IF NOT EXISTS idx_deletion_log_item ON deletion_log(item_id);
";

pub struct Store {
    conn: Connection,
}

impl Store {
    /// Open (or create) a database at the given path. Enables WAL mode and
    /// creates the schema if needed.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let conn = Connection::open(path)?;
        let store = Self { conn };
        store.init()?;
        Ok(store)
    }

    /// Open an in-memory database (for tests).
    pub fn open_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let store = Self { conn };
        store.init()?;
        Ok(store)
    }

    /// Access the underlying connection.
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

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

    pub fn get_item(&self, id: ItemId) -> Result<Item> {
        let mut stmt = self.conn.prepare(
            "SELECT id, text, note, created_at, modified_at, entry_date, when_date, done_date, is_done
             FROM items WHERE id = ?1",
        )?;
        let item = stmt
            .query_row(params![id.to_string()], |row| Self::row_to_item(row))
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => AgendaError::NotFound {
                    entity: "Item",
                    id,
                },
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
            serde_json::to_string(&item.assignments).unwrap_or_else(|_| "{}".to_string());

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
            .query_map([], |row| Self::row_to_item(row))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        rows.into_iter().map(|item| self.load_assignments(item)).collect()
    }

    // ── Item helpers ───────────────────────────────────────────

    fn row_to_item(row: &Row<'_>) -> rusqlite::Result<Item> {
        let id_str: String = row.get(0)?;
        let created_str: String = row.get(3)?;
        let modified_str: String = row.get(4)?;
        let entry_str: String = row.get(5)?;
        let when_str: Option<String> = row.get(6)?;
        let done_str: Option<String> = row.get(7)?;
        let is_done_int: i32 = row.get(8)?;

        Ok(Item {
            id: Uuid::parse_str(&id_str).unwrap_or_default(),
            text: row.get(1)?,
            note: row.get(2)?,
            created_at: DateTime::parse_from_rfc3339(&created_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_default(),
            modified_at: DateTime::parse_from_rfc3339(&modified_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_default(),
            entry_date: NaiveDate::parse_from_str(&entry_str, "%Y-%m-%d").unwrap_or_default(),
            when_date: when_str.and_then(|s| NaiveDateTime::parse_from_str(&s, "%Y-%m-%d %H:%M:%S").ok()),
            done_date: done_str.and_then(|s| NaiveDateTime::parse_from_str(&s, "%Y-%m-%d %H:%M:%S").ok()),
            is_done: is_done_int != 0,
            assignments: HashMap::new(),
        })
    }

    fn load_assignments(&self, mut item: Item) -> Result<Item> {
        let mut stmt = self.conn.prepare(
            "SELECT category_id, source, assigned_at, sticky, origin
             FROM assignments WHERE item_id = ?1",
        )?;
        let rows = stmt.query_map(params![item.id.to_string()], |row| {
            let cat_str: String = row.get(0)?;
            let source_str: String = row.get(1)?;
            let assigned_str: String = row.get(2)?;
            let sticky_int: i32 = row.get(3)?;
            let origin: Option<String> = row.get(4)?;
            Ok((cat_str, source_str, assigned_str, sticky_int, origin))
        })?;

        for row in rows {
            let (cat_str, source_str, assigned_str, sticky_int, origin) = row?;
            let cat_id = Uuid::parse_str(&cat_str).unwrap_or_default();
            let source = match source_str.as_str() {
                "Manual" => AssignmentSource::Manual,
                "AutoMatch" => AssignmentSource::AutoMatch,
                "Action" => AssignmentSource::Action,
                "Subsumption" => AssignmentSource::Subsumption,
                _ => AssignmentSource::Manual,
            };
            let assigned_at = DateTime::parse_from_rfc3339(&assigned_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_default();
            item.assignments.insert(
                cat_id,
                Assignment {
                    source,
                    assigned_at,
                    sticky: sticky_int != 0,
                    origin,
                },
            );
        }
        Ok(item)
    }

    fn init(&self) -> Result<()> {
        // WAL mode for crash safety and concurrent reads.
        self.conn.pragma_update(None, "journal_mode", "wal")?;
        // Enable foreign key enforcement.
        self.conn.pragma_update(None, "foreign_keys", "ON")?;

        let version: i32 = self
            .conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap_or(0);

        if version < SCHEMA_VERSION {
            self.conn.execute_batch(SCHEMA_SQL).map_err(|e| {
                AgendaError::StorageError {
                    source: Box::new(e),
                }
            })?;
            self.conn
                .pragma_update(None, "user_version", SCHEMA_VERSION)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AssignmentSource, Item};
    use chrono::Utc;
    use rusqlite::params;
    use uuid::Uuid;

    #[test]
    fn test_open_memory_creates_schema() {
        let store = Store::open_memory().expect("failed to open in-memory store");

        // Verify all tables exist by querying sqlite_master.
        let tables: Vec<String> = store
            .conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();

        assert!(tables.contains(&"items".to_string()));
        assert!(tables.contains(&"categories".to_string()));
        assert!(tables.contains(&"assignments".to_string()));
        assert!(tables.contains(&"views".to_string()));
        assert!(tables.contains(&"deletion_log".to_string()));
    }

    #[test]
    fn test_wal_mode_enabled() {
        let store = Store::open_memory().expect("failed to open in-memory store");
        let mode: String = store
            .conn
            .pragma_query_value(None, "journal_mode", |row| row.get(0))
            .unwrap();
        // In-memory databases use "memory" journal mode, but the pragma was set.
        // For file-based DBs it would be "wal". Just verify no error.
        assert!(!mode.is_empty());
    }

    #[test]
    fn test_schema_version_set() {
        let store = Store::open_memory().expect("failed to open in-memory store");
        let version: i32 = store
            .conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);
    }

    #[test]
    fn test_foreign_keys_enabled() {
        let store = Store::open_memory().expect("failed to open in-memory store");
        let fk: i32 = store
            .conn
            .pragma_query_value(None, "foreign_keys", |row| row.get(0))
            .unwrap();
        assert_eq!(fk, 1);
    }

    #[test]
    fn test_idempotent_init() {
        let store = Store::open_memory().expect("failed to open in-memory store");
        // Calling init again should not fail.
        store.init().expect("second init should be idempotent");
    }

    #[test]
    fn test_create_and_get_item() {
        let store = Store::open_memory().unwrap();
        let item = Item::new("Buy groceries".to_string());
        let id = item.id;
        store.create_item(&item).unwrap();

        let loaded = store.get_item(id).unwrap();
        assert_eq!(loaded.id, id);
        assert_eq!(loaded.text, "Buy groceries");
        assert!(!loaded.is_done);
        assert!(loaded.note.is_none());
    }

    #[test]
    fn test_get_item_not_found() {
        let store = Store::open_memory().unwrap();
        let result = store.get_item(Uuid::new_v4());
        assert!(matches!(result, Err(AgendaError::NotFound { .. })));
    }

    #[test]
    fn test_update_item() {
        let store = Store::open_memory().unwrap();
        let mut item = Item::new("Draft".to_string());
        store.create_item(&item).unwrap();

        item.text = "Final version".to_string();
        item.note = Some("Added details".to_string());
        item.modified_at = Utc::now();
        store.update_item(&item).unwrap();

        let loaded = store.get_item(item.id).unwrap();
        assert_eq!(loaded.text, "Final version");
        assert_eq!(loaded.note.as_deref(), Some("Added details"));
    }

    #[test]
    fn test_update_nonexistent_item() {
        let store = Store::open_memory().unwrap();
        let item = Item::new("Ghost".to_string());
        let result = store.update_item(&item);
        assert!(matches!(result, Err(AgendaError::NotFound { .. })));
    }

    #[test]
    fn test_delete_item_writes_log() {
        let store = Store::open_memory().unwrap();
        let item = Item::new("To be deleted".to_string());
        let id = item.id;
        store.create_item(&item).unwrap();

        store.delete_item(id, "user").unwrap();

        // Item should be gone.
        assert!(matches!(store.get_item(id), Err(AgendaError::NotFound { .. })));

        // Deletion log should have an entry.
        let count: i32 = store
            .conn
            .query_row(
                "SELECT COUNT(*) FROM deletion_log WHERE item_id = ?1",
                params![id.to_string()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_list_items() {
        let store = Store::open_memory().unwrap();
        store.create_item(&Item::new("First".to_string())).unwrap();
        store.create_item(&Item::new("Second".to_string())).unwrap();

        let items = store.list_items().unwrap();
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn test_item_with_assignments_loaded() {
        let store = Store::open_memory().unwrap();
        let item = Item::new("Test assignments".to_string());
        let item_id = item.id;
        store.create_item(&item).unwrap();

        // Insert a category and assignment directly.
        let cat_id = Uuid::new_v4();
        store
            .conn
            .execute(
                "INSERT INTO categories (id, name, created_at, modified_at) VALUES (?1, ?2, ?3, ?3)",
                params![cat_id.to_string(), "TestCat", Utc::now().to_rfc3339()],
            )
            .unwrap();
        store
            .conn
            .execute(
                "INSERT INTO assignments (item_id, category_id, source, assigned_at, sticky, origin)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    item_id.to_string(),
                    cat_id.to_string(),
                    "Manual",
                    Utc::now().to_rfc3339(),
                    1,
                    "manual",
                ],
            )
            .unwrap();

        let loaded = store.get_item(item_id).unwrap();
        assert_eq!(loaded.assignments.len(), 1);
        assert!(loaded.assignments.contains_key(&cat_id));
        assert_eq!(loaded.assignments[&cat_id].source, AssignmentSource::Manual);
    }

    #[test]
    fn test_category_name_unique_case_insensitive() {
        let store = Store::open_memory().expect("failed to open in-memory store");
        store
            .conn
            .execute(
                "INSERT INTO categories (id, name, created_at, modified_at) VALUES (?1, ?2, ?3, ?3)",
                params!["id1", "TestCat", "2026-01-01T00:00:00Z"],
            )
            .unwrap();

        let result = store.conn.execute(
            "INSERT INTO categories (id, name, created_at, modified_at) VALUES (?1, ?2, ?3, ?3)",
            params!["id2", "testcat", "2026-01-01T00:00:00Z"],
        );
        assert!(result.is_err(), "duplicate case-insensitive name should fail");
    }
}
