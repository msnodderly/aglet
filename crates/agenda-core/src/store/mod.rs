use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;
use std::time::Duration;

use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension, Row};
use rust_decimal::Decimal;
use serde_json;
use uuid::Uuid;

use crate::error::{AgendaError, Result};
use crate::model::{
    Action, Assignment, AssignmentSource, BoardDisplayMode, Category, CategoryId,
    CategoryValueKind, Condition, DeletionLogEntry, Item, ItemLink, ItemLinkKind,
    NumericFormat, Query, Section, View, RESERVED_CATEGORY_NAMES, RESERVED_CATEGORY_NAME_WHEN,
};

mod assignments;
mod categories;
mod items;
mod links;
mod views;

#[cfg(test)]
mod tests;

const SCHEMA_VERSION: i32 = 8;
const DEFAULT_VIEW_NAME: &str = "All Items";

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
    is_actionable          INTEGER NOT NULL DEFAULT 1,
    enable_implicit_string INTEGER NOT NULL DEFAULT 1,
    note                   TEXT,
    created_at             TEXT NOT NULL,
    modified_at            TEXT NOT NULL,
    sort_order             INTEGER NOT NULL DEFAULT 0,
    conditions_json        TEXT NOT NULL DEFAULT '[]',
    actions_json           TEXT NOT NULL DEFAULT '[]',
    value_kind             TEXT NOT NULL DEFAULT 'Tag',
    numeric_format_json    TEXT NOT NULL DEFAULT 'null'
);

CREATE TABLE IF NOT EXISTS assignments (
    item_id     TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    category_id TEXT NOT NULL REFERENCES categories(id) ON DELETE CASCADE,
    source      TEXT NOT NULL,
    assigned_at TEXT NOT NULL,
    sticky      INTEGER NOT NULL DEFAULT 1,
    origin      TEXT,
    numeric_value TEXT,
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
    remove_from_view_unassign_json TEXT NOT NULL DEFAULT '[]',
    category_aliases_json       TEXT NOT NULL DEFAULT '{}',
    item_column_label           TEXT,
    board_display_mode          TEXT NOT NULL DEFAULT 'SingleLine'
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

CREATE TABLE IF NOT EXISTS item_links (
    item_id       TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    other_item_id TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    kind          TEXT NOT NULL,
    created_at    TEXT NOT NULL,
    origin        TEXT,
    metadata_json TEXT NOT NULL DEFAULT '{}',
    PRIMARY KEY (item_id, other_item_id, kind),
    CHECK (item_id <> other_item_id),
    CHECK (kind IN ('depends-on', 'related')),
    CHECK (kind <> 'related' OR item_id < other_item_id)
);

CREATE INDEX IF NOT EXISTS idx_assignments_item ON assignments(item_id);
CREATE INDEX IF NOT EXISTS idx_assignments_category ON assignments(category_id);
CREATE INDEX IF NOT EXISTS idx_categories_parent ON categories(parent_id);
CREATE INDEX IF NOT EXISTS idx_items_when_date ON items(when_date);
CREATE INDEX IF NOT EXISTS idx_items_is_done ON items(is_done);
CREATE INDEX IF NOT EXISTS idx_deletion_log_item ON deletion_log(item_id);
CREATE INDEX IF NOT EXISTS idx_item_links_item_kind ON item_links(item_id, kind);
CREATE INDEX IF NOT EXISTS idx_item_links_other_kind ON item_links(other_item_id, kind);
CREATE INDEX IF NOT EXISTS idx_item_links_kind ON item_links(kind);

CREATE TABLE IF NOT EXISTS app_settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
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
    pub(crate) fn conn(&self) -> &Connection {
        &self.conn
    }

    /// Store an application-level key/value setting.
    pub fn set_app_setting(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO app_settings (key, value)
             VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    /// Retrieve an application-level key/value setting.
    pub fn get_app_setting(&self, key: &str) -> Result<Option<String>> {
        self.conn
            .query_row(
                "SELECT value FROM app_settings WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()
            .map_err(AgendaError::from)
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
            when_date: when_str
                .and_then(|s| NaiveDateTime::parse_from_str(&s, "%Y-%m-%d %H:%M:%S").ok()),
            done_date: done_str
                .and_then(|s| NaiveDateTime::parse_from_str(&s, "%Y-%m-%d %H:%M:%S").ok()),
            is_done: is_done_int != 0,
            assignments: HashMap::new(),
        })
    }

    fn row_to_deleted_item(row: &Row<'_>) -> rusqlite::Result<DeletionLogEntry> {
        let id_str: String = row.get(0)?;
        let item_id_str: String = row.get(1)?;
        let entry_str: String = row.get(4)?;
        let when_str: Option<String> = row.get(5)?;
        let done_str: Option<String> = row.get(6)?;
        let is_done_int: i32 = row.get(7)?;
        let deleted_at_str: String = row.get(9)?;

        Ok(DeletionLogEntry {
            id: Uuid::parse_str(&id_str).unwrap_or_default(),
            item_id: Uuid::parse_str(&item_id_str).unwrap_or_default(),
            text: row.get(2)?,
            note: row.get(3)?,
            entry_date: NaiveDate::parse_from_str(&entry_str, "%Y-%m-%d").unwrap_or_default(),
            when_date: when_str
                .and_then(|s| NaiveDateTime::parse_from_str(&s, "%Y-%m-%d %H:%M:%S").ok()),
            done_date: done_str
                .and_then(|s| NaiveDateTime::parse_from_str(&s, "%Y-%m-%d %H:%M:%S").ok()),
            is_done: is_done_int != 0,
            assignments_json: row.get(8)?,
            deleted_at: DateTime::parse_from_rfc3339(&deleted_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_default(),
            deleted_by: row.get(10)?,
        })
    }

    fn load_assignments(&self, mut item: Item) -> Result<Item> {
        let mut stmt = self.conn.prepare(
            "SELECT category_id, source, assigned_at, sticky, origin, numeric_value
             FROM assignments WHERE item_id = ?1",
        )?;
        let rows = stmt.query_map(params![item.id.to_string()], |row| {
            let cat_str: String = row.get(0)?;
            let source_str: String = row.get(1)?;
            let assigned_str: String = row.get(2)?;
            let sticky_int: i32 = row.get(3)?;
            let origin: Option<String> = row.get(4)?;
            let numeric_value: Option<String> = row.get(5)?;
            Ok((
                cat_str,
                source_str,
                assigned_str,
                sticky_int,
                origin,
                numeric_value,
            ))
        })?;

        for row in rows {
            let (cat_str, source_str, assigned_str, sticky_int, origin, numeric_value_str) = row?;
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
            let numeric_value = numeric_value_str.and_then(|s| s.parse::<Decimal>().ok());
            item.assignments.insert(
                cat_id,
                Assignment {
                    source,
                    assigned_at,
                    sticky: sticky_int != 0,
                    origin,
                    numeric_value,
                },
            );
        }
        Ok(item)
    }

    fn row_to_view(row: &Row<'_>) -> rusqlite::Result<View> {
        let id_str: String = row.get(0)?;
        let criteria_json: String = row.get(2)?;
        let sections_json: String = row.get(3)?;
        let _columns_json: String = row.get(4)?; // legacy column, no longer used
        let show_unmatched: i32 = row.get(5)?;
        let remove_from_view_unassign_json: String = row.get(7)?;
        let category_aliases_json: Option<String> = row.get(8)?;
        let item_column_label: Option<String> = row.get(9)?;
        let board_display_mode_json: Option<String> = row.get(10)?;

        // Corrupt or legacy view row: fall back to empty defaults so the view
        // still loads rather than failing the entire hierarchy read.
        let criteria: Query = serde_json::from_str(&criteria_json).unwrap_or_default();
        let sections: Vec<Section> = serde_json::from_str(&sections_json).unwrap_or_default();
        let remove_from_view_unassign: HashSet<CategoryId> =
            serde_json::from_str(&remove_from_view_unassign_json).unwrap_or_default();
        let category_aliases: BTreeMap<CategoryId, String> = category_aliases_json
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default();
        let board_display_mode = board_display_mode_json
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or(BoardDisplayMode::SingleLine);

        Ok(View {
            id: Uuid::parse_str(&id_str).unwrap_or_default(),
            name: row.get(1)?,
            criteria,
            sections,
            show_unmatched: show_unmatched != 0,
            unmatched_label: row.get(6)?,
            remove_from_view_unassign,
            category_aliases,
            item_column_label,
            board_display_mode,
        })
    }

    fn insert_view(&self, view: &View) -> Result<()> {
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

        self.conn
            .execute(
                "INSERT INTO views (
                    id, name, criteria_json, sections_json, columns_json,
                    show_unmatched, unmatched_label, remove_from_view_unassign_json,
                    category_aliases_json, item_column_label, board_display_mode
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    view.id.to_string(),
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
                ],
            )
            .map_err(|err| Self::map_view_write_error(err, &view.name))?;

        Ok(())
    }

    // ── Item link helpers ──────────────────────────────────────

    fn item_link_kind_to_db(kind: ItemLinkKind) -> &'static str {
        match kind {
            ItemLinkKind::DependsOn => "depends-on",
            ItemLinkKind::Related => "related",
        }
    }

    fn item_link_kind_from_db(kind: &str) -> Result<ItemLinkKind> {
        match kind {
            "depends-on" => Ok(ItemLinkKind::DependsOn),
            "related" => Ok(ItemLinkKind::Related),
            other => Err(Self::storage_data_error(format!(
                "invalid item link kind in database: {other}"
            ))),
        }
    }

    fn item_link_from_db_row(
        item_id_str: &str,
        other_item_id_str: &str,
        kind_str: &str,
        created_at_str: &str,
        origin: Option<String>,
    ) -> Result<ItemLink> {
        let item_id = Self::parse_uuid_from_db_text(item_id_str, "item_links.item_id")?;
        let other_item_id =
            Self::parse_uuid_from_db_text(other_item_id_str, "item_links.other_item_id")?;
        let kind = Self::item_link_kind_from_db(kind_str)?;
        let created_at = DateTime::parse_from_rfc3339(created_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|e| {
                Self::storage_data_error(format!(
                    "invalid item_links.created_at '{created_at_str}': {e}"
                ))
            })?;

        Ok(ItemLink {
            item_id,
            other_item_id,
            kind,
            created_at,
            origin,
        })
    }

    fn parse_uuid_from_db_text(raw: &str, field: &'static str) -> Result<Uuid> {
        Uuid::parse_str(raw)
            .map_err(|e| Self::storage_data_error(format!("invalid UUID in {field}: {raw} ({e})")))
    }

    fn storage_data_error(message: String) -> AgendaError {
        AgendaError::StorageError {
            source: Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                message,
            )),
        }
    }

    // ── Category helpers ───────────────────────────────────────

    fn row_to_category(row: &Row<'_>) -> rusqlite::Result<(Category, i64)> {
        let id_str: String = row.get(0)?;
        let parent_id_str: Option<String> = row.get(2)?;
        let is_exclusive: i32 = row.get(3)?;
        let is_actionable: i32 = row.get(4)?;
        let enable_implicit_string: i32 = row.get(5)?;
        let created_str: String = row.get(7)?;
        let modified_str: String = row.get(8)?;
        let conditions_json: String = row.get(9)?;
        let actions_json: String = row.get(10)?;
        let sort_order: i64 = row.get(11)?;
        let value_kind_str: String = row.get(12)?;
        let numeric_format_json: String = row.get(13)?;

        // Corrupt or legacy category row: fall back to no conditions/actions
        // so the category still loads without its rules rather than failing.
        let conditions: Vec<Condition> = serde_json::from_str(&conditions_json).unwrap_or_default();
        let actions: Vec<Action> = serde_json::from_str(&actions_json).unwrap_or_default();
        let value_kind = Self::category_value_kind_from_db(&value_kind_str);
        let numeric_format: Option<NumericFormat> =
            serde_json::from_str(&numeric_format_json).unwrap_or(None);

        Ok((
            Category {
                id: Uuid::parse_str(&id_str).unwrap_or_default(),
                name: row.get(1)?,
                parent: parent_id_str.and_then(|s| Uuid::parse_str(&s).ok()),
                children: Vec::new(),
                is_exclusive: is_exclusive != 0,
                is_actionable: is_actionable != 0,
                enable_implicit_string: enable_implicit_string != 0,
                note: row.get(6)?,
                created_at: DateTime::parse_from_rfc3339(&created_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_default(),
                modified_at: DateTime::parse_from_rfc3339(&modified_str)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_default(),
                conditions,
                actions,
                value_kind,
                numeric_format,
            },
            sort_order,
        ))
    }

    fn get_child_category_ids(&self, parent_id: CategoryId) -> Result<Vec<CategoryId>> {
        let mut stmt = self.conn.prepare(
            "SELECT id
             FROM categories
             WHERE parent_id = ?1
             ORDER BY sort_order ASC, name COLLATE NOCASE ASC",
        )?;
        let rows = stmt
            .query_map(params![parent_id.to_string()], |row| {
                let id_str: String = row.get(0)?;
                Ok(Uuid::parse_str(&id_str).unwrap_or_default())
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    fn list_category_ids_for_parent(
        &self,
        parent_id: Option<CategoryId>,
    ) -> Result<Vec<CategoryId>> {
        let sql_root = "SELECT id
             FROM categories
             WHERE parent_id IS NULL
             ORDER BY sort_order ASC, name COLLATE NOCASE ASC";
        let sql_child = "SELECT id
             FROM categories
             WHERE parent_id = ?1
             ORDER BY sort_order ASC, name COLLATE NOCASE ASC";

        let mut stmt = if parent_id.is_some() {
            self.conn.prepare(sql_child)?
        } else {
            self.conn.prepare(sql_root)?
        };
        let rows = if let Some(parent_id) = parent_id {
            stmt.query_map(params![parent_id.to_string()], |row| {
                let id_str: String = row.get(0)?;
                Ok(Uuid::parse_str(&id_str).unwrap_or_default())
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?
        } else {
            stmt.query_map([], |row| {
                let id_str: String = row.get(0)?;
                Ok(Uuid::parse_str(&id_str).unwrap_or_default())
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?
        };
        Ok(rows)
    }

    fn rewrite_category_sort_orders_for_parent(
        &self,
        parent_id: Option<CategoryId>,
        ordered_ids: &[CategoryId],
    ) -> Result<()> {
        let sql_root = "UPDATE categories
             SET sort_order = ?1
             WHERE id = ?2 AND parent_id IS NULL";
        let sql_child = "UPDATE categories
             SET sort_order = ?1
             WHERE id = ?2 AND parent_id = ?3";

        for (idx, category_id) in ordered_ids.iter().enumerate() {
            let rows = if let Some(parent_id) = parent_id {
                self.conn.execute(
                    sql_child,
                    params![idx as i64, category_id.to_string(), parent_id.to_string()],
                )?
            } else {
                self.conn
                    .execute(sql_root, params![idx as i64, category_id.to_string()])?
            };
            if rows == 0 {
                return Err(AgendaError::NotFound {
                    entity: "Category",
                    id: *category_id,
                });
            }
        }
        Ok(())
    }

    fn update_category_parent_only(
        &self,
        category_id: CategoryId,
        new_parent_id: Option<CategoryId>,
    ) -> Result<()> {
        let modified_at = Utc::now();
        let rows = self.conn.execute(
            "UPDATE categories
             SET parent_id = ?1, modified_at = ?2
             WHERE id = ?3",
            params![
                new_parent_id.map(|id| id.to_string()),
                modified_at.to_rfc3339(),
                category_id.to_string()
            ],
        )?;
        if rows == 0 {
            return Err(AgendaError::NotFound {
                entity: "Category",
                id: category_id,
            });
        }
        Ok(())
    }

    pub(crate) fn with_immediate_transaction<T>(
        &self,
        f: impl FnOnce(&Store) -> Result<T>,
    ) -> Result<T> {
        self.conn.execute_batch("BEGIN IMMEDIATE TRANSACTION")?;
        let result = f(self);
        match result {
            Ok(value) => {
                self.conn.execute_batch("COMMIT")?;
                Ok(value)
            }
            Err(err) => {
                let _ = self.conn.execute_batch("ROLLBACK");
                Err(err)
            }
        }
    }

    fn with_category_order_transaction<T>(&self, f: impl FnOnce(&Store) -> Result<T>) -> Result<T> {
        self.with_immediate_transaction(f)
    }

    fn next_category_sort_order(&self, parent_id: Option<CategoryId>) -> Result<i64> {
        let sql_for_root =
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM categories WHERE parent_id IS NULL";
        let sql_for_child =
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM categories WHERE parent_id = ?1";

        if let Some(parent_id) = parent_id {
            let next =
                self.conn
                    .query_row(sql_for_child, params![parent_id.to_string()], |row| {
                        row.get(0)
                    })?;
            Ok(next)
        } else {
            let next = self.conn.query_row(sql_for_root, [], |row| row.get(0))?;
            Ok(next)
        }
    }

    fn validate_category_parent(
        &self,
        category_id: CategoryId,
        parent_id: Option<CategoryId>,
    ) -> Result<()> {
        let mut cursor = parent_id;
        while let Some(current_parent_id) = cursor {
            if current_parent_id == category_id {
                return Err(AgendaError::InvalidOperation {
                    message: "category reparent would create a cycle".to_string(),
                });
            }
            let parent = self.get_category(current_parent_id)?;
            cursor = parent.parent;
        }
        Ok(())
    }

    fn flatten_hierarchy(
        category_id: CategoryId,
        categories_by_id: &HashMap<CategoryId, Category>,
        child_ids_by_parent: &HashMap<Option<CategoryId>, Vec<(i64, CategoryId)>>,
        output: &mut Vec<Category>,
    ) {
        if let Some(category) = categories_by_id.get(&category_id) {
            output.push(category.clone());
        }

        if let Some(child_ids) = child_ids_by_parent.get(&Some(category_id)) {
            for (_, child_id) in child_ids {
                Self::flatten_hierarchy(*child_id, categories_by_id, child_ids_by_parent, output);
            }
        }
    }

    /// Map a SQLite write error to a domain error, detecting unique-name violations.
    /// `table_column` is e.g. `"categories.name"` or `"views.name"` for the fallback
    /// message-based detection path.
    fn map_write_error(err: rusqlite::Error, name: &str, table_column: &str) -> AgendaError {
        match err {
            rusqlite::Error::SqliteFailure(sqlite_err, _)
                if sqlite_err.code == rusqlite::ErrorCode::ConstraintViolation
                    && sqlite_err.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE =>
            {
                AgendaError::DuplicateName {
                    name: name.to_string(),
                }
            }
            rusqlite::Error::SqliteFailure(sqlite_err, Some(ref message))
                if sqlite_err.code == rusqlite::ErrorCode::ConstraintViolation
                    && message.contains(table_column) =>
            {
                AgendaError::DuplicateName {
                    name: name.to_string(),
                }
            }
            other => AgendaError::from(other),
        }
    }

    fn map_category_write_error(err: rusqlite::Error, category_name: &str) -> AgendaError {
        Self::map_write_error(err, category_name, "categories.name")
    }

    fn map_view_write_error(err: rusqlite::Error, view_name: &str) -> AgendaError {
        Self::map_write_error(err, view_name, "views.name")
    }

    fn get_category_id_by_name(&self, name: &str) -> Result<Option<CategoryId>> {
        let id_str: Option<String> = self
            .conn
            .query_row(
                "SELECT id FROM categories WHERE name = ?1 COLLATE NOCASE LIMIT 1",
                params![name],
                |row| row.get(0),
            )
            .optional()?;
        Ok(id_str.and_then(|s| Uuid::parse_str(&s).ok()))
    }

    fn category_assignment_count(&self, category_id: CategoryId) -> Result<i64> {
        let count = self.conn.query_row(
            "SELECT COUNT(*) FROM assignments WHERE category_id = ?1",
            params![category_id.to_string()],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    fn category_value_kind_to_db(kind: CategoryValueKind) -> &'static str {
        match kind {
            CategoryValueKind::Tag => "Tag",
            CategoryValueKind::Numeric => "Numeric",
        }
    }

    fn category_value_kind_from_db(raw: &str) -> CategoryValueKind {
        if raw.eq_ignore_ascii_case("numeric") {
            CategoryValueKind::Numeric
        } else {
            CategoryValueKind::Tag
        }
    }

    fn validate_parent_accepts_children(parent: &Category) -> Result<()> {
        if parent.value_kind == CategoryValueKind::Numeric {
            return Err(AgendaError::InvalidOperation {
                message: format!(
                    "cannot add child under numeric category '{}'; numeric categories must be leaves",
                    parent.name
                ),
            });
        }
        Ok(())
    }

    fn validate_category_type_shape(category: &Category) -> Result<()> {
        if category.value_kind == CategoryValueKind::Numeric && !category.children.is_empty() {
            return Err(AgendaError::InvalidOperation {
                message: format!("numeric category '{}' cannot have children", category.name),
            });
        }
        Ok(())
    }

    fn validate_category_type_transition(
        &self,
        existing: &Category,
        updated: &Category,
    ) -> Result<()> {
        if existing.value_kind == updated.value_kind {
            return Ok(());
        }

        match (existing.value_kind, updated.value_kind) {
            (CategoryValueKind::Tag, CategoryValueKind::Numeric) => {
                if !existing.children.is_empty() {
                    return Err(AgendaError::InvalidOperation {
                        message: format!(
                            "cannot convert category '{}' to Numeric while it has children",
                            existing.name
                        ),
                    });
                }
                if self.category_assignment_count(existing.id)? > 0 {
                    return Err(AgendaError::InvalidOperation {
                        message: format!(
                            "cannot convert category '{}' to Numeric after assignments already exist",
                            existing.name
                        ),
                    });
                }
                Ok(())
            }
            (CategoryValueKind::Numeric, CategoryValueKind::Tag) => {
                Err(AgendaError::InvalidOperation {
                    message: format!(
                        "cannot convert numeric category '{}' back to Tag",
                        existing.name
                    ),
                })
            }
            _ => Ok(()),
        }
    }

    fn insert_reserved_category(&self, name: &str) -> Result<CategoryId> {
        let id = Uuid::new_v4();
        let now = Utc::now().to_rfc3339();
        let sort_order = self.next_category_sort_order(None)?;

        // Reserved categories have implicit string matching disabled by default.
        // "Done", "When", "Entry" should not auto-match item text containing
        // those common words.
        self.conn
            .execute(
                "INSERT INTO categories (
                    id, name, parent_id, is_exclusive, is_actionable, enable_implicit_string, note,
                    created_at, modified_at, sort_order, conditions_json, actions_json,
                    value_kind, numeric_format_json
                 ) VALUES (?1, ?2, NULL, 0, 0, 0, NULL, ?3, ?3, ?4, '[]', '[]', 'Tag', 'null')",
                params![id.to_string(), name, now, sort_order],
            )
            .map_err(|err| Self::map_category_write_error(err, name))?;

        Ok(id)
    }

    fn ensure_reserved_categories(&self) -> Result<CategoryId> {
        let mut when_category_id = None;

        for reserved_name in RESERVED_CATEGORY_NAMES {
            let category_id = match self.get_category_id_by_name(reserved_name)? {
                Some(existing_id) => existing_id,
                None => self.insert_reserved_category(reserved_name)?,
            };
            if reserved_name == RESERVED_CATEGORY_NAME_WHEN {
                when_category_id = Some(category_id);
            }
        }

        when_category_id.ok_or_else(|| AgendaError::StorageError {
            source: Box::new(std::io::Error::other("missing reserved When category")),
        })
    }

    fn has_view_named(&self, name: &str) -> Result<bool> {
        let exists: Option<i64> = self
            .conn
            .query_row(
                "SELECT 1 FROM views WHERE name = ?1 COLLATE NOCASE LIMIT 1",
                params![name],
                |row| row.get(0),
            )
            .optional()?;
        Ok(exists.is_some())
    }

    fn ensure_default_view(&self, _when_category_id: CategoryId) -> Result<()> {
        if self.has_view_named(DEFAULT_VIEW_NAME)? {
            return Ok(());
        }

        let view = View::new(DEFAULT_VIEW_NAME.to_string());
        self.insert_view(&view)?;
        Ok(())
    }

    fn is_reserved_category_name(name: &str) -> bool {
        RESERVED_CATEGORY_NAMES
            .iter()
            .any(|reserved| reserved.eq_ignore_ascii_case(name))
    }

    fn is_system_view_name(name: &str) -> bool {
        name.eq_ignore_ascii_case(DEFAULT_VIEW_NAME)
    }

    // ── Schema & migrations ────────────────────────────────────

    fn init(&self) -> Result<()> {
        // WAL mode for crash safety and concurrent reads.
        self.conn.pragma_update(None, "journal_mode", "wal")?;
        // Enable foreign key enforcement.
        self.conn.pragma_update(None, "foreign_keys", "ON")?;
        // Allow short waits on write contention so transactional claim paths can
        // resolve to precondition failures instead of immediate lock errors.
        self.conn.busy_timeout(Duration::from_secs(2))?;

        let version: i32 = self
            .conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap_or(0);

        if version < SCHEMA_VERSION {
            self.conn
                .execute_batch(SCHEMA_SQL)
                .map_err(|e| AgendaError::StorageError {
                    source: Box::new(e),
                })?;
            self.apply_migrations(version)?;
            self.conn
                .pragma_update(None, "user_version", SCHEMA_VERSION)?;
        }

        let when_category_id = self.ensure_reserved_categories()?;
        self.ensure_default_view(when_category_id)?;

        Ok(())
    }

    fn apply_migrations(&self, from_version: i32) -> Result<()> {
        if from_version < 2 && !self.column_exists("categories", "is_actionable")? {
            self.conn.execute_batch(
                "ALTER TABLE categories ADD COLUMN is_actionable INTEGER NOT NULL DEFAULT 1;",
            )?;
            self.conn.execute(
                "UPDATE categories
                 SET is_actionable = 0
                 WHERE name IN ('When', 'Entry', 'Done') COLLATE NOCASE",
                [],
            )?;
        }
        // Always ensure item_column_label exists — idempotent column add
        // guards against DBs that were stamped at version 3 by an earlier
        // partial implementation before this column was added to the schema.
        if !self.column_exists("views", "item_column_label")? {
            self.conn
                .execute_batch("ALTER TABLE views ADD COLUMN item_column_label TEXT;")?;
        }
        if !self.column_exists("views", "board_display_mode")? {
            self.conn.execute_batch(
                "ALTER TABLE views ADD COLUMN board_display_mode TEXT NOT NULL DEFAULT 'SingleLine';",
            )?;
        }
        if !self.column_exists("views", "category_aliases_json")? {
            self.conn.execute_batch(
                "ALTER TABLE views ADD COLUMN category_aliases_json TEXT NOT NULL DEFAULT '{}';",
            )?;
        }
        if !self.column_exists("categories", "value_kind")? {
            self.conn.execute_batch(
                "ALTER TABLE categories ADD COLUMN value_kind TEXT NOT NULL DEFAULT 'Tag';",
            )?;
        }
        if !self.column_exists("categories", "numeric_format_json")? {
            self.conn.execute_batch(
                "ALTER TABLE categories ADD COLUMN numeric_format_json TEXT NOT NULL DEFAULT 'null';",
            )?;
        }
        if !self.column_exists("assignments", "numeric_value")? {
            self.conn
                .execute_batch("ALTER TABLE assignments ADD COLUMN numeric_value TEXT;")?;
        }
        if from_version < 6 {
            self.conn.execute_batch(
                r#"
                CREATE TABLE IF NOT EXISTS item_links (
                    item_id       TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,
                    other_item_id TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,
                    kind          TEXT NOT NULL,
                    created_at    TEXT NOT NULL,
                    origin        TEXT,
                    metadata_json TEXT NOT NULL DEFAULT '{}',
                    PRIMARY KEY (item_id, other_item_id, kind),
                    CHECK (item_id <> other_item_id),
                    CHECK (kind IN ('depends-on', 'related')),
                    CHECK (kind <> 'related' OR item_id < other_item_id)
                );
                CREATE INDEX IF NOT EXISTS idx_item_links_item_kind
                    ON item_links(item_id, kind);
                CREATE INDEX IF NOT EXISTS idx_item_links_other_kind
                    ON item_links(other_item_id, kind);
                CREATE INDEX IF NOT EXISTS idx_item_links_kind
                    ON item_links(kind);
                "#,
            )?;
        }
        if from_version < 8 {
            self.conn.execute_batch(
                r#"
                CREATE TABLE IF NOT EXISTS app_settings (
                    key   TEXT PRIMARY KEY,
                    value TEXT NOT NULL
                );
                "#,
            )?;
        }

        if from_version < 3 {
            // Inject kind field into existing columns_json.
            // Find the When category ID, then tag columns whose heading matches it
            // as When, all others as Standard.
            let when_cat_id = self.get_category_id_by_name(RESERVED_CATEGORY_NAME_WHEN)?;
            let mut stmt = self.conn.prepare("SELECT id, columns_json FROM views")?;
            let rows: Vec<(String, String)> = stmt
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;

            for (view_id, columns_json) in rows {
                // Corrupt legacy row: treat as having no columns and skip the migration.
                let mut columns: Vec<serde_json::Value> =
                    serde_json::from_str(&columns_json).unwrap_or_default();
                let mut changed = false;
                for col in columns.iter_mut() {
                    if col.get("kind").is_none() {
                        let heading = col.get("heading").and_then(|h| h.as_str()).unwrap_or("");
                        let is_when = when_cat_id
                            .map(|wid| heading == wid.to_string())
                            .unwrap_or(false);
                        col.as_object_mut().unwrap().insert(
                            "kind".to_string(),
                            serde_json::Value::String(
                                if is_when { "When" } else { "Standard" }.to_string(),
                            ),
                        );
                        changed = true;
                    }
                }
                if changed {
                    let new_json = serde_json::to_string(&columns)
                        .expect("Vec<serde_json::Value> is always serialisable");
                    self.conn.execute(
                        "UPDATE views SET columns_json = ?1 WHERE id = ?2",
                        params![new_json, view_id],
                    )?;
                }
            }
        }
        Ok(())
    }

    fn column_exists(&self, table: &str, column: &str) -> Result<bool> {
        let pragma = format!("PRAGMA table_info({table})");
        let mut stmt = self.conn.prepare(&pragma)?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let name: String = row.get(1)?;
            if name.eq_ignore_ascii_case(column) {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
