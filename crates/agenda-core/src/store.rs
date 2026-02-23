use std::collections::{HashMap, HashSet};
use std::path::Path;

use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde_json;
use uuid::Uuid;

use crate::error::{AgendaError, Result};
use crate::model::{
    Action, Assignment, AssignmentSource, BoardDisplayMode, Category, CategoryId, Condition,
    DeletionLogEntry, Item, ItemId, Query, Section, View,
};

const SCHEMA_VERSION: i32 = 4;
const RESERVED_CATEGORY_NAMES: [&str; 3] = ["When", "Entry", "Done"];
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
    remove_from_view_unassign_json TEXT NOT NULL DEFAULT '[]',
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

    // ── Category CRUD ───────────────────────────────────────────

    pub fn create_category(&self, category: &Category) -> Result<()> {
        if Self::is_reserved_category_name(&category.name) {
            return Err(AgendaError::ReservedName {
                name: category.name.clone(),
            });
        }

        if let Some(parent_id) = category.parent {
            // Ensure parent exists so callers get a deterministic NotFound error.
            self.get_category(parent_id)?;
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

        let sort_order = self.next_category_sort_order(category.parent)?;

        self.conn
            .execute(
                "INSERT INTO categories (
                    id, name, parent_id, is_exclusive, is_actionable, enable_implicit_string, note,
                    created_at, modified_at, sort_order, conditions_json, actions_json
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
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
                ],
            )
            .map_err(|err| Self::map_category_write_error(err, &category.name))?;

        Ok(())
    }

    pub fn get_category(&self, id: CategoryId) -> Result<Category> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, parent_id, is_exclusive, is_actionable, enable_implicit_string, note,
                    created_at, modified_at, conditions_json, actions_json, sort_order
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
        self.validate_category_parent(category.id, category.parent)?;

        let conditions_json = serde_json::to_string(&category.conditions).map_err(|err| {
            AgendaError::StorageError {
                source: Box::new(err),
            }
        })?;
        let actions_json =
            serde_json::to_string(&category.actions).map_err(|err| AgendaError::StorageError {
                source: Box::new(err),
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
                     actions_json = ?9
                 WHERE id = ?10",
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
                    category.id.to_string(),
                ],
            )
            .map_err(|err| Self::map_category_write_error(err, &category.name))?;

        Ok(())
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
                    created_at, modified_at, conditions_json, actions_json, sort_order
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

    // ── View CRUD ───────────────────────────────────────────────

    pub fn create_view(&self, view: &View) -> Result<()> {
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

        self.conn
            .execute(
                "INSERT INTO views (
                    id, name, criteria_json, sections_json, columns_json,
                    show_unmatched, unmatched_label, remove_from_view_unassign_json,
                    item_column_label, board_display_mode
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    view.id.to_string(),
                    view.name,
                    criteria_json,
                    sections_json,
                    "[]",
                    view.show_unmatched as i32,
                    view.unmatched_label,
                    remove_from_view_unassign_json,
                    view.item_column_label,
                    serde_json::to_string(&view.board_display_mode)
                        .unwrap_or_else(|_| "\"SingleLine\"".to_string()),
                ],
            )
            .map_err(|err| Self::map_view_write_error(err, &view.name))?;

        Ok(())
    }

    pub fn get_view(&self, id: Uuid) -> Result<View> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, criteria_json, sections_json, columns_json,
                    show_unmatched, unmatched_label, remove_from_view_unassign_json,
                    item_column_label, board_display_mode
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
                     item_column_label = ?8,
                     board_display_mode = ?9
                 WHERE id = ?10",
                params![
                    view.name,
                    criteria_json,
                    sections_json,
                    "[]",
                    view.show_unmatched as i32,
                    view.unmatched_label,
                    remove_from_view_unassign_json,
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
                    item_column_label, board_display_mode
             FROM views
             ORDER BY name COLLATE NOCASE ASC",
        )?;
        let rows = stmt
            .query_map([], Self::row_to_view)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn delete_view(&self, id: Uuid) -> Result<()> {
        let rows = self
            .conn
            .execute("DELETE FROM views WHERE id = ?1", params![id.to_string()])?;
        if rows == 0 {
            return Err(AgendaError::NotFound { entity: "View", id });
        }
        Ok(())
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

    fn row_to_view(row: &Row<'_>) -> rusqlite::Result<View> {
        let id_str: String = row.get(0)?;
        let criteria_json: String = row.get(2)?;
        let sections_json: String = row.get(3)?;
        let _columns_json: String = row.get(4)?; // legacy column, no longer used
        let show_unmatched: i32 = row.get(5)?;
        let remove_from_view_unassign_json: String = row.get(7)?;
        let item_column_label: Option<String> = row.get(8)?;
        let board_display_mode_json: Option<String> = row.get(9)?;

        let criteria: Query = serde_json::from_str(&criteria_json).unwrap_or_default();
        let sections: Vec<Section> = serde_json::from_str(&sections_json).unwrap_or_default();
        let remove_from_view_unassign: HashSet<CategoryId> =
            serde_json::from_str(&remove_from_view_unassign_json).unwrap_or_default();
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
            item_column_label,
            board_display_mode,
        })
    }

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
            "INSERT OR REPLACE INTO assignments (item_id, category_id, source, assigned_at, sticky, origin)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                item_id.to_string(),
                category_id.to_string(),
                source_str,
                assignment.assigned_at.to_rfc3339(),
                assignment.sticky as i32,
                assignment.origin,
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

        let conditions: Vec<Condition> = serde_json::from_str(&conditions_json).unwrap_or_default();
        let actions: Vec<Action> = serde_json::from_str(&actions_json).unwrap_or_default();

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

    fn map_category_write_error(err: rusqlite::Error, category_name: &str) -> AgendaError {
        match err {
            rusqlite::Error::SqliteFailure(sqlite_err, _)
                if sqlite_err.code == rusqlite::ErrorCode::ConstraintViolation
                    && sqlite_err.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE =>
            {
                AgendaError::DuplicateName {
                    name: category_name.to_string(),
                }
            }
            rusqlite::Error::SqliteFailure(sqlite_err, Some(message))
                if sqlite_err.code == rusqlite::ErrorCode::ConstraintViolation
                    && message.contains("categories.name") =>
            {
                AgendaError::DuplicateName {
                    name: category_name.to_string(),
                }
            }
            other => AgendaError::from(other),
        }
    }

    fn map_view_write_error(err: rusqlite::Error, view_name: &str) -> AgendaError {
        match err {
            rusqlite::Error::SqliteFailure(sqlite_err, _)
                if sqlite_err.code == rusqlite::ErrorCode::ConstraintViolation
                    && sqlite_err.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE =>
            {
                AgendaError::DuplicateName {
                    name: view_name.to_string(),
                }
            }
            rusqlite::Error::SqliteFailure(sqlite_err, Some(message))
                if sqlite_err.code == rusqlite::ErrorCode::ConstraintViolation
                    && message.contains("views.name") =>
            {
                AgendaError::DuplicateName {
                    name: view_name.to_string(),
                }
            }
            other => AgendaError::from(other),
        }
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
                    created_at, modified_at, sort_order, conditions_json, actions_json
                 ) VALUES (?1, ?2, NULL, 0, 0, 0, NULL, ?3, ?3, ?4, '[]', '[]')",
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
            if reserved_name.eq_ignore_ascii_case("When") {
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
        self.create_view(&view)?;
        Ok(())
    }

    fn is_reserved_category_name(name: &str) -> bool {
        RESERVED_CATEGORY_NAMES
            .iter()
            .any(|reserved| reserved.eq_ignore_ascii_case(name))
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

        if from_version < 3 {
            // Inject kind field into existing columns_json.
            // Find the When category ID, then tag columns whose heading matches it
            // as When, all others as Standard.
            let when_cat_id = self.get_category_id_by_name("When")?;
            let mut stmt = self.conn.prepare("SELECT id, columns_json FROM views")?;
            let rows: Vec<(String, String)> = stmt
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;

            for (view_id, columns_json) in rows {
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
                    let new_json = serde_json::to_string(&columns).unwrap_or_default();
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        Assignment, AssignmentSource, BoardDisplayMode, Category, Column, ColumnKind,
        CriterionMode, Item, Query, Section, View,
    };
    use chrono::{Duration, Utc};
    use rusqlite::params;
    use std::collections::HashSet;
    use uuid::Uuid;

    fn new_category(name: &str) -> Category {
        Category::new(name.to_string())
    }

    fn new_view(name: &str) -> View {
        View::new(name.to_string())
    }

    fn category_id_by_name(store: &Store, name: &str) -> Uuid {
        let id: String = store
            .conn
            .query_row(
                "SELECT id FROM categories WHERE name = ?1 COLLATE NOCASE",
                params![name],
                |row| row.get(0),
            )
            .unwrap();
        Uuid::parse_str(&id).unwrap()
    }

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
        let reserved_before: i64 = store
            .conn
            .query_row(
                "SELECT COUNT(*) FROM categories WHERE name IN ('When', 'Entry', 'Done')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let default_view_before: i64 = store
            .conn
            .query_row(
                "SELECT COUNT(*) FROM views WHERE name = 'All Items'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        // Calling init again should be idempotent.
        store.init().expect("second init should be idempotent");

        let reserved_after: i64 = store
            .conn
            .query_row(
                "SELECT COUNT(*) FROM categories WHERE name IN ('When', 'Entry', 'Done')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let default_view_after: i64 = store
            .conn
            .query_row(
                "SELECT COUNT(*) FROM views WHERE name = 'All Items'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(reserved_before, 3);
        assert_eq!(reserved_after, 3);
        assert_eq!(default_view_before, 1);
        assert_eq!(default_view_after, 1);
    }

    #[test]
    fn test_first_launch_creates_reserved_categories_and_default_view() {
        let store = Store::open_memory().expect("failed to open in-memory store");

        let categories: Vec<String> = store
            .conn
            .prepare("SELECT name FROM categories ORDER BY sort_order ASC")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<std::result::Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(categories, vec!["When", "Entry", "Done"]);

        // Reserved categories should have implicit string matching disabled
        // so common words like "done" or "when" don't trigger auto-assignment.
        for name in ["When", "Entry", "Done"] {
            let cat = store
                .get_category(category_id_by_name(&store, name))
                .unwrap();
            assert!(
                !cat.enable_implicit_string,
                "{name} should have enable_implicit_string = false"
            );
            assert!(
                !cat.is_actionable,
                "{name} should have is_actionable = false"
            );
        }

        let _when_id = category_id_by_name(&store, "When");
        let all_items_view: String = store
            .conn
            .query_row("SELECT id FROM views WHERE name = 'All Items'", [], |row| {
                row.get(0)
            })
            .unwrap();
        let view = store
            .get_view(Uuid::parse_str(&all_items_view).unwrap())
            .unwrap();

        assert_eq!(view.name, "All Items");
        assert!(view.criteria.criteria.is_empty());
        assert!(view.sections.is_empty());
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
        assert!(matches!(
            store.get_item(id),
            Err(AgendaError::NotFound { .. })
        ));

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
    fn test_list_deleted_items_returns_latest_first() {
        let store = Store::open_memory().unwrap();

        let first = Item::new("First deleted".to_string());
        let second = Item::new("Second deleted".to_string());
        store.create_item(&first).unwrap();
        store.create_item(&second).unwrap();

        store.delete_item(first.id, "user").unwrap();
        store.delete_item(second.id, "user").unwrap();

        let deleted = store.list_deleted_items().unwrap();
        assert_eq!(deleted.len(), 2);
        assert_eq!(deleted[0].item_id, second.id);
        assert_eq!(deleted[1].item_id, first.id);
    }

    #[test]
    fn test_restore_deleted_item_recreates_item_and_assignments() {
        let store = Store::open_memory().unwrap();
        let category_id = make_category(&store, "RestoreTarget");

        let item = Item::new("Restore me".to_string());
        store.create_item(&item).unwrap();
        let assignment = Assignment {
            source: AssignmentSource::Manual,
            assigned_at: Utc::now(),
            sticky: true,
            origin: Some("manual:test".to_string()),
        };
        store
            .assign_item(item.id, category_id, &assignment)
            .unwrap();
        store.delete_item(item.id, "user").unwrap();

        let log_entry_id: Uuid = store
            .conn
            .query_row(
                "SELECT id FROM deletion_log WHERE item_id = ?1 ORDER BY deleted_at DESC LIMIT 1",
                params![item.id.to_string()],
                |row| {
                    let id_str: String = row.get(0)?;
                    Ok(Uuid::parse_str(&id_str).unwrap())
                },
            )
            .unwrap();

        let restored_item_id = store.restore_deleted_item(log_entry_id).unwrap();
        assert_eq!(restored_item_id, item.id);

        let restored = store.get_item(restored_item_id).unwrap();
        assert_eq!(restored.text, "Restore me");
        let assignments = store.get_assignments_for_item(restored_item_id).unwrap();
        assert!(assignments.contains_key(&category_id));
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
    fn test_create_items_allows_duplicate_text_with_distinct_ids() {
        let store = Store::open_memory().unwrap();
        let first = Item::new("Buy milk".to_string());
        let second = Item::new("Buy milk".to_string());
        assert_ne!(first.id, second.id);

        store.create_item(&first).unwrap();
        store.create_item(&second).unwrap();

        let items = store.list_items().unwrap();
        let duplicates: Vec<&Item> = items
            .iter()
            .filter(|item| item.text == "Buy milk")
            .collect();
        assert_eq!(duplicates.len(), 2);
        assert_ne!(duplicates[0].id, duplicates[1].id);
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

    fn make_category(store: &Store, name: &str) -> Uuid {
        let id = Uuid::new_v4();
        store
            .conn
            .execute(
                "INSERT INTO categories (id, name, created_at, modified_at) VALUES (?1, ?2, ?3, ?3)",
                params![id.to_string(), name, Utc::now().to_rfc3339()],
            )
            .unwrap();
        id
    }

    #[test]
    fn test_assign_and_get_assignments() {
        let store = Store::open_memory().unwrap();
        let item = Item::new("Test item".to_string());
        let item_id = item.id;
        store.create_item(&item).unwrap();

        let cat_id = make_category(&store, "Project");
        let assignment = Assignment {
            source: AssignmentSource::Manual,
            assigned_at: Utc::now(),
            sticky: true,
            origin: Some("manual".to_string()),
        };
        store.assign_item(item_id, cat_id, &assignment).unwrap();

        let assignments = store.get_assignments_for_item(item_id).unwrap();
        assert_eq!(assignments.len(), 1);
        assert!(assignments.contains_key(&cat_id));
        assert_eq!(assignments[&cat_id].source, AssignmentSource::Manual);
        assert_eq!(assignments[&cat_id].origin.as_deref(), Some("manual"));
    }

    #[test]
    fn test_assign_upsert_replaces() {
        let store = Store::open_memory().unwrap();
        let item = Item::new("Test item".to_string());
        let item_id = item.id;
        store.create_item(&item).unwrap();

        let cat_id = make_category(&store, "Status");
        let a1 = Assignment {
            source: AssignmentSource::AutoMatch,
            assigned_at: Utc::now(),
            sticky: true,
            origin: Some("cat:Status".to_string()),
        };
        store.assign_item(item_id, cat_id, &a1).unwrap();

        // Re-assign with different source — should replace.
        let a2 = Assignment {
            source: AssignmentSource::Manual,
            assigned_at: Utc::now(),
            sticky: false,
            origin: Some("manual".to_string()),
        };
        store.assign_item(item_id, cat_id, &a2).unwrap();

        let assignments = store.get_assignments_for_item(item_id).unwrap();
        assert_eq!(assignments.len(), 1);
        assert_eq!(assignments[&cat_id].source, AssignmentSource::Manual);
        assert!(!assignments[&cat_id].sticky);
    }

    #[test]
    fn test_unassign_item() {
        let store = Store::open_memory().unwrap();
        let item = Item::new("Test item".to_string());
        let item_id = item.id;
        store.create_item(&item).unwrap();

        let cat_id = make_category(&store, "Remove");
        let assignment = Assignment {
            source: AssignmentSource::Manual,
            assigned_at: Utc::now(),
            sticky: true,
            origin: None,
        };
        store.assign_item(item_id, cat_id, &assignment).unwrap();
        assert_eq!(store.get_assignments_for_item(item_id).unwrap().len(), 1);

        store.unassign_item(item_id, cat_id).unwrap();
        assert_eq!(store.get_assignments_for_item(item_id).unwrap().len(), 0);
    }

    #[test]
    fn test_unassign_nonexistent_is_ok() {
        let store = Store::open_memory().unwrap();
        // Unassigning something that doesn't exist should not error.
        store.unassign_item(Uuid::new_v4(), Uuid::new_v4()).unwrap();
    }

    #[test]
    fn test_multiple_assignments() {
        let store = Store::open_memory().unwrap();
        let item = Item::new("Multi-assign".to_string());
        let item_id = item.id;
        store.create_item(&item).unwrap();

        let cat1 = make_category(&store, "Cat1");
        let cat2 = make_category(&store, "Cat2");
        let cat3 = make_category(&store, "Cat3");

        for (cat_id, src) in [
            (cat1, AssignmentSource::Manual),
            (cat2, AssignmentSource::AutoMatch),
            (cat3, AssignmentSource::Subsumption),
        ] {
            let a = Assignment {
                source: src,
                assigned_at: Utc::now(),
                sticky: true,
                origin: None,
            };
            store.assign_item(item_id, cat_id, &a).unwrap();
        }

        let assignments = store.get_assignments_for_item(item_id).unwrap();
        assert_eq!(assignments.len(), 3);
        assert_eq!(assignments[&cat1].source, AssignmentSource::Manual);
        assert_eq!(assignments[&cat2].source, AssignmentSource::AutoMatch);
        assert_eq!(assignments[&cat3].source, AssignmentSource::Subsumption);
    }

    #[test]
    fn test_category_name_unique_case_insensitive() {
        let store = Store::open_memory().expect("failed to open in-memory store");
        let category = new_category("TestCat");
        store.create_category(&category).unwrap();

        let duplicate = new_category("testcat");
        let result = store.create_category(&duplicate);
        assert!(matches!(
            result,
            Err(AgendaError::DuplicateName { name }) if name == "testcat"
        ));
    }

    #[test]
    fn test_create_and_get_category() {
        let store = Store::open_memory().unwrap();
        let mut root = new_category("Projects");
        root.is_exclusive = true;
        root.note = Some("top-level".to_string());
        store.create_category(&root).unwrap();

        let mut child = new_category("Aglet");
        child.parent = Some(root.id);
        store.create_category(&child).unwrap();

        let loaded_root = store.get_category(root.id).unwrap();
        assert_eq!(loaded_root.name, "Projects");
        assert!(loaded_root.children.contains(&child.id));
        assert!(loaded_root.is_exclusive);
        assert_eq!(loaded_root.note.as_deref(), Some("top-level"));

        let loaded_child = store.get_category(child.id).unwrap();
        assert_eq!(loaded_child.parent, Some(root.id));
    }

    #[test]
    fn test_create_category_rejects_reserved_names() {
        let store = Store::open_memory().unwrap();
        let reserved = new_category("wHeN");
        let result = store.create_category(&reserved);
        assert!(matches!(
            result,
            Err(AgendaError::ReservedName { name }) if name == "wHeN"
        ));
    }

    #[test]
    fn test_create_category_with_invalid_parent_rejected() {
        let store = Store::open_memory().unwrap();
        let mut category = new_category("Orphan");
        category.parent = Some(Uuid::new_v4());

        let result = store.create_category(&category);
        assert!(matches!(
            result,
            Err(AgendaError::NotFound {
                entity: "Category",
                ..
            })
        ));
    }

    #[test]
    fn test_update_category_touches_modified_at() {
        let store = Store::open_memory().unwrap();
        let mut category = new_category("Draft");
        category.modified_at = Utc::now() - Duration::minutes(10);
        store.create_category(&category).unwrap();

        let original_modified_at = category.modified_at;
        category.name = "Published".to_string();
        category.enable_implicit_string = false;
        category.note = Some("updated".to_string());
        store.update_category(&category).unwrap();

        let loaded = store.get_category(category.id).unwrap();
        assert_eq!(loaded.name, "Published");
        assert!(!loaded.enable_implicit_string);
        assert_eq!(loaded.note.as_deref(), Some("updated"));
        assert!(loaded.modified_at > original_modified_at);
    }

    #[test]
    fn test_update_category_not_found() {
        let store = Store::open_memory().unwrap();
        let missing = new_category("Missing");
        let result = store.update_category(&missing);
        assert!(matches!(result, Err(AgendaError::NotFound { .. })));
    }

    #[test]
    fn test_update_category_rename_to_duplicate_rejected() {
        let store = Store::open_memory().unwrap();
        let mut one = new_category("One");
        let two = new_category("Two");
        store.create_category(&one).unwrap();
        store.create_category(&two).unwrap();

        one.name = "Two".to_string();
        let result = store.update_category(&one);
        assert!(matches!(
            result,
            Err(AgendaError::DuplicateName { name }) if name == "Two"
        ));
    }

    #[test]
    fn test_update_category_reparent_cycle_rejected() {
        let store = Store::open_memory().unwrap();
        let root = new_category("Root");
        store.create_category(&root).unwrap();

        let mut child = new_category("Child");
        child.parent = Some(root.id);
        store.create_category(&child).unwrap();

        let mut updated_root = store.get_category(root.id).unwrap();
        updated_root.parent = Some(child.id);

        let result = store.update_category(&updated_root);
        assert!(matches!(result, Err(AgendaError::InvalidOperation { .. })));
    }

    #[test]
    fn test_delete_category() {
        let store = Store::open_memory().unwrap();
        let category = new_category("Temp");
        let id = category.id;
        store.create_category(&category).unwrap();

        store.delete_category(id).unwrap();
        assert!(matches!(
            store.get_category(id),
            Err(AgendaError::NotFound { .. })
        ));
    }

    #[test]
    fn test_delete_category_with_children_rejected() {
        let store = Store::open_memory().unwrap();
        let parent = new_category("Parent");
        store.create_category(&parent).unwrap();

        let mut child = new_category("Child");
        child.parent = Some(parent.id);
        store.create_category(&child).unwrap();

        let result = store.delete_category(parent.id);
        assert!(matches!(result, Err(AgendaError::InvalidOperation { .. })));
    }

    #[test]
    fn test_delete_reserved_category_rejected() {
        let store = Store::open_memory().unwrap();
        let reserved_id = category_id_by_name(&store, "Done");

        let result = store.delete_category(reserved_id);
        assert!(matches!(
            result,
            Err(AgendaError::ReservedName { name }) if name == "Done"
        ));
    }

    #[test]
    fn test_update_reserved_category_allowed_without_rename() {
        let store = Store::open_memory().unwrap();
        let reserved_id = category_id_by_name(&store, "When");

        let mut category = store.get_category(reserved_id).unwrap();
        category.note = Some("allowed".to_string());
        category.enable_implicit_string = false;
        store.update_category(&category).unwrap();

        let loaded = store.get_category(reserved_id).unwrap();
        assert_eq!(loaded.name, "When");
        assert_eq!(loaded.note.as_deref(), Some("allowed"));
        assert!(!loaded.enable_implicit_string);
    }

    #[test]
    fn test_create_and_get_view() {
        let store = Store::open_memory().unwrap();
        let when_category = make_category(&store, "WhenColumn");

        let mut view = new_view("Inbox");
        view.criteria
            .set_criterion(CriterionMode::And, when_category);

        let mut section_criteria = Query::default();
        section_criteria.set_criterion(CriterionMode::And, when_category);
        view.sections.push(Section {
            title: "Due Soon".to_string(),
            criteria: section_criteria,
            columns: vec![Column {
                kind: ColumnKind::Standard,
                heading: when_category,
                width: 18,
            }],
            item_column_index: 0,
            on_insert_assign: HashSet::from([when_category]),
            on_remove_unassign: HashSet::new(),
            show_children: true,
            board_display_mode_override: None,
        });
        view.show_unmatched = false;
        view.unmatched_label = "Other".to_string();
        view.remove_from_view_unassign.insert(when_category);

        store.create_view(&view).unwrap();

        let loaded = store.get_view(view.id).unwrap();
        assert_eq!(loaded.id, view.id);
        assert_eq!(loaded.name, "Inbox");
        assert_eq!(loaded.criteria.criteria, view.criteria.criteria);
        assert_eq!(loaded.sections.len(), 1);
        assert_eq!(loaded.sections[0].title, "Due Soon");
        assert!(loaded.sections[0].show_children);
        assert_eq!(loaded.sections[0].columns.len(), 1);
        assert_eq!(loaded.sections[0].columns[0].heading, when_category);
        assert_eq!(loaded.sections[0].columns[0].width, 18);
        assert!(!loaded.show_unmatched);
        assert_eq!(loaded.unmatched_label, "Other");
        assert_eq!(
            loaded.remove_from_view_unassign,
            view.remove_from_view_unassign
        );
    }

    #[test]
    fn test_get_view_not_found() {
        let store = Store::open_memory().unwrap();
        let result = store.get_view(Uuid::new_v4());
        assert!(matches!(
            result,
            Err(AgendaError::NotFound { entity: "View", .. })
        ));
    }

    #[test]
    fn test_create_view_duplicate_name_rejected() {
        let store = Store::open_memory().unwrap();
        let one = new_view("Planning");
        let two = new_view("Planning");
        store.create_view(&one).unwrap();

        let result = store.create_view(&two);
        assert!(matches!(
            result,
            Err(AgendaError::DuplicateName { name }) if name == "Planning"
        ));
    }

    #[test]
    fn test_update_view() {
        let store = Store::open_memory().unwrap();
        let mut view = new_view("Daily");
        store.create_view(&view).unwrap();

        let category_id = make_category(&store, "Schedule");
        view.name = "Daily Agenda".to_string();
        view.criteria.set_criterion(CriterionMode::And, category_id);
        view.sections.push(Section {
            title: "Today".to_string(),
            criteria: Query::default(),
            columns: Vec::new(),
            item_column_index: 0,
            on_insert_assign: HashSet::from([category_id]),
            on_remove_unassign: HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        });
        view.show_unmatched = false;
        view.unmatched_label = "Unsectioned".to_string();
        view.remove_from_view_unassign.insert(category_id);

        store.update_view(&view).unwrap();

        let loaded = store.get_view(view.id).unwrap();
        assert_eq!(loaded.name, "Daily Agenda");
        assert!(loaded
            .criteria
            .and_category_ids()
            .any(|id| id == category_id));
        assert_eq!(loaded.sections.len(), 1);
        assert!(!loaded.show_unmatched);
        assert_eq!(loaded.unmatched_label, "Unsectioned");
        assert_eq!(
            loaded.remove_from_view_unassign,
            HashSet::from([category_id])
        );
    }

    #[test]
    fn test_update_view_not_found() {
        let store = Store::open_memory().unwrap();
        let missing = new_view("Missing");
        let result = store.update_view(&missing);
        assert!(matches!(
            result,
            Err(AgendaError::NotFound {
                entity: "View",
                id
            }) if id == missing.id
        ));
    }

    #[test]
    fn test_view_board_display_mode_roundtrip_and_section_override() {
        let store = Store::open_memory().unwrap();
        let mut view = new_view("Display");
        view.board_display_mode = BoardDisplayMode::MultiLine;
        view.sections.push(Section {
            title: "One".to_string(),
            criteria: Query::default(),
            columns: Vec::new(),
            item_column_index: 1,
            on_insert_assign: HashSet::new(),
            on_remove_unassign: HashSet::new(),
            show_children: false,
            board_display_mode_override: Some(BoardDisplayMode::SingleLine),
        });
        store.create_view(&view).unwrap();

        let loaded = store.get_view(view.id).unwrap();
        assert_eq!(loaded.board_display_mode, BoardDisplayMode::MultiLine);
        assert_eq!(
            loaded.sections[0].board_display_mode_override,
            Some(BoardDisplayMode::SingleLine)
        );
        assert_eq!(
            loaded.sections[0].item_column_index, 1,
            "roundtrips section field"
        );
    }

    #[test]
    fn test_sections_json_without_display_override_defaults_to_none() {
        let legacy_json = r#"[{"title":"Legacy","criteria":{},"columns":[],"on_insert_assign":[],"on_remove_unassign":[],"show_children":false}]"#;
        let sections: Vec<Section> = serde_json::from_str(legacy_json).expect("legacy json parses");
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].item_column_index, 0);
        assert_eq!(sections[0].board_display_mode_override, None);
    }

    #[test]
    fn test_list_views_ordered_by_name_case_insensitive() {
        let store = Store::open_memory().unwrap();
        store.create_view(&new_view("zeta")).unwrap();
        store.create_view(&new_view("Alpha")).unwrap();
        store.create_view(&new_view("beta")).unwrap();

        let views = store.list_views().unwrap();
        let names: Vec<String> = views.into_iter().map(|v| v.name).collect();
        assert_eq!(names, vec!["All Items", "Alpha", "beta", "zeta"]);
    }

    #[test]
    fn test_delete_view() {
        let store = Store::open_memory().unwrap();
        let view = new_view("Temp");
        let id = view.id;
        store.create_view(&view).unwrap();

        store.delete_view(id).unwrap();
        assert!(matches!(
            store.get_view(id),
            Err(AgendaError::NotFound { entity: "View", .. })
        ));
    }

    #[test]
    fn test_delete_view_not_found() {
        let store = Store::open_memory().unwrap();
        let result = store.delete_view(Uuid::new_v4());
        assert!(matches!(
            result,
            Err(AgendaError::NotFound { entity: "View", .. })
        ));
    }

    #[test]
    fn test_get_hierarchy_returns_depth_first_with_children() {
        let store = Store::open_memory().unwrap();
        let root_a = new_category("RootA");
        let root_b = new_category("RootB");
        store.create_category(&root_a).unwrap();
        store.create_category(&root_b).unwrap();

        let mut child_a = new_category("ChildA");
        child_a.parent = Some(root_a.id);
        store.create_category(&child_a).unwrap();

        let mut grandchild = new_category("Grandchild");
        grandchild.parent = Some(child_a.id);
        store.create_category(&grandchild).unwrap();

        let hierarchy = store.get_hierarchy().unwrap();
        let root_a_pos = hierarchy.iter().position(|c| c.id == root_a.id).unwrap();
        let child_a_pos = hierarchy.iter().position(|c| c.id == child_a.id).unwrap();
        let grandchild_pos = hierarchy
            .iter()
            .position(|c| c.id == grandchild.id)
            .unwrap();
        let root_b_pos = hierarchy.iter().position(|c| c.id == root_b.id).unwrap();

        assert!(root_a_pos < child_a_pos);
        assert!(child_a_pos < grandchild_pos);
        assert!(grandchild_pos < root_b_pos);

        let loaded_root_a = hierarchy.iter().find(|c| c.id == root_a.id).unwrap();
        assert_eq!(loaded_root_a.children, vec![child_a.id]);

        let loaded_child_a = hierarchy.iter().find(|c| c.id == child_a.id).unwrap();
        assert_eq!(loaded_child_a.children, vec![grandchild.id]);
    }
}
