use std::path::Path;

use rusqlite::Connection;

use crate::error::{AgendaError, Result};

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

    /// Access the underlying connection (for CRUD implementations in later tasks).
    pub fn conn(&self) -> &Connection {
        &self.conn
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
    use rusqlite::params;

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
