use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;
use std::time::Duration;

use jiff::Timestamp;
use rusqlite::{params, Connection, OptionalExtension, Row};
use rust_decimal::Decimal;
use serde_json;
use uuid::Uuid;

use crate::classification::{
    CandidateAssignment, ClassificationConfig, ClassificationSuggestion, SuggestionStatus,
    CLASSIFICATION_CONFIG_KEY,
};
use crate::error::{AgletError, Result};
use crate::model::{
    Action, Assignment, AssignmentExplanation, AssignmentSource, BoardDisplayMode, Category,
    CategoryId, CategoryValueKind, Condition, ConditionMatchMode, DatebookConfig, DeletionLogEntry,
    EmptySections, Item, ItemId, ItemLink, ItemLinkKind, NumericFormat, Query, RecurrenceRule,
    Section, SectionFlow, View, RESERVED_CATEGORY_NAMES, RESERVED_CATEGORY_NAME_WHEN,
};
use crate::workflow::{WorkflowConfig, READY_QUEUE_VIEW_NAME, WORKFLOW_CONFIG_KEY};

const SCHEMA_VERSION: i32 = 20;
pub const DEFAULT_VIEW_NAME: &str = "All Items";

pub fn canonical_system_view_name(name: &str) -> Option<&'static str> {
    if name.eq_ignore_ascii_case(DEFAULT_VIEW_NAME) {
        Some(DEFAULT_VIEW_NAME)
    } else if name.eq_ignore_ascii_case(READY_QUEUE_VIEW_NAME) {
        Some(READY_QUEUE_VIEW_NAME)
    } else {
        None
    }
}

pub fn is_system_view_name(name: &str) -> bool {
    canonical_system_view_name(name).is_some()
}

const SCHEMA_SQL: &str = "
CREATE TABLE IF NOT EXISTS items (
    id                        TEXT PRIMARY KEY,
    text                      TEXT NOT NULL,
    note                      TEXT,
    created_at                TEXT NOT NULL,
    modified_at               TEXT NOT NULL,
    entry_date                TEXT NOT NULL,
    when_date                 TEXT,
    done_date                 TEXT,
    is_done                   INTEGER NOT NULL DEFAULT 0,
    recurrence_rule_json      TEXT,
    recurrence_series_id      TEXT,
    recurrence_parent_item_id TEXT
);

CREATE TABLE IF NOT EXISTS categories (
    id                     TEXT PRIMARY KEY,
    name                   TEXT NOT NULL UNIQUE COLLATE NOCASE,
    parent_id              TEXT REFERENCES categories(id),
    is_exclusive           INTEGER NOT NULL DEFAULT 0,
    is_actionable          INTEGER NOT NULL DEFAULT 1,
    enable_implicit_string INTEGER NOT NULL DEFAULT 1,
    enable_semantic_classification INTEGER NOT NULL DEFAULT 1,
    match_category_name    INTEGER NOT NULL DEFAULT 1,
    also_match_json        TEXT NOT NULL DEFAULT '[]',
    note                   TEXT,
    created_at             TEXT NOT NULL,
    modified_at            TEXT NOT NULL,
    condition_match_mode   TEXT NOT NULL DEFAULT 'Any',
    sort_order             INTEGER NOT NULL DEFAULT 0,
    conditions_json        TEXT NOT NULL DEFAULT '[]',
    actions_json           TEXT NOT NULL DEFAULT '[]',
    value_kind             TEXT NOT NULL DEFAULT 'Tag',
    numeric_format_json    TEXT NOT NULL DEFAULT 'null',
    allow_delete_action    INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS assignments (
    item_id     TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    category_id TEXT NOT NULL REFERENCES categories(id) ON DELETE CASCADE,
    source      TEXT NOT NULL,
    assigned_at TEXT NOT NULL,
    sticky      INTEGER NOT NULL DEFAULT 1,
    origin      TEXT,
    explanation_json TEXT NOT NULL DEFAULT 'null',
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
    board_display_mode          TEXT NOT NULL DEFAULT 'SingleLine',
    section_flow                TEXT NOT NULL DEFAULT 'Vertical',
    empty_sections              TEXT NOT NULL DEFAULT 'Show',
    hide_dependent_items        INTEGER NOT NULL DEFAULT 0,
    datebook_config_json        TEXT
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

CREATE TABLE IF NOT EXISTS assignment_vetoes (
    item_id     TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    category_id TEXT NOT NULL REFERENCES categories(id) ON DELETE CASCADE,
    created_at  TEXT NOT NULL,
    origin      TEXT,
    PRIMARY KEY (item_id, category_id)
);

CREATE INDEX IF NOT EXISTS idx_assignments_item ON assignments(item_id);
CREATE INDEX IF NOT EXISTS idx_assignment_vetoes_item ON assignment_vetoes(item_id);
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

CREATE TABLE IF NOT EXISTS classification_suggestions (
    id                 TEXT PRIMARY KEY,
    item_id            TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    kind               TEXT NOT NULL,
    category_id        TEXT,
    when_value         TEXT,
    provider_id        TEXT NOT NULL,
    model              TEXT,
    confidence         REAL,
    rationale          TEXT,
    status             TEXT NOT NULL,
    context_hash       TEXT NOT NULL,
    item_revision_hash TEXT NOT NULL,
    created_at         TEXT NOT NULL,
    decided_at         TEXT
);

CREATE INDEX IF NOT EXISTS idx_classification_suggestions_item_id
    ON classification_suggestions (item_id);
CREATE INDEX IF NOT EXISTS idx_classification_suggestions_status
    ON classification_suggestions (status);
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
            .map_err(AgletError::from)
    }

    pub fn set_workflow_config(&self, config: &WorkflowConfig) -> Result<()> {
        let body = serde_json::to_string(config).map_err(|err| AgletError::StorageError {
            source: Box::new(err),
        })?;
        self.set_app_setting(WORKFLOW_CONFIG_KEY, &body)
    }

    pub fn get_workflow_config(&self) -> Result<WorkflowConfig> {
        let Some(raw) = self.get_app_setting(WORKFLOW_CONFIG_KEY)? else {
            return Ok(WorkflowConfig::default());
        };
        Ok(serde_json::from_str(&raw).unwrap_or_default())
    }

    pub fn set_classification_config(&self, config: &ClassificationConfig) -> Result<()> {
        let body = serde_json::to_string(config).map_err(|err| AgletError::StorageError {
            source: Box::new(err),
        })?;
        self.set_app_setting(CLASSIFICATION_CONFIG_KEY, &body)
    }

    pub fn get_classification_config(&self) -> Result<ClassificationConfig> {
        let Some(raw) = self.get_app_setting(CLASSIFICATION_CONFIG_KEY)? else {
            return Ok(ClassificationConfig::default());
        };
        Ok(serde_json::from_str(&raw).unwrap_or_default())
    }

    pub fn get_classification_suggestion(
        &self,
        suggestion_id: Uuid,
    ) -> Result<Option<ClassificationSuggestion>> {
        self.conn
            .query_row(
                "SELECT id, item_id, kind, category_id, when_value, provider_id, model,
                        confidence, rationale, status, context_hash, item_revision_hash,
                        created_at, decided_at
                 FROM classification_suggestions
                 WHERE id = ?1",
                params![suggestion_id.to_string()],
                Self::row_to_classification_suggestion,
            )
            .optional()
            .map_err(AgletError::from)
    }

    pub fn list_pending_suggestions(&self) -> Result<Vec<ClassificationSuggestion>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, item_id, kind, category_id, when_value, provider_id, model,
                    confidence, rationale, status, context_hash, item_revision_hash,
                    created_at, decided_at
             FROM classification_suggestions
             WHERE status = 'pending'
             ORDER BY created_at ASC, id ASC",
        )?;
        let suggestions = stmt
            .query_map([], Self::row_to_classification_suggestion)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(AgletError::from)?;
        Ok(suggestions)
    }

    pub fn list_pending_suggestions_for_item(
        &self,
        item_id: ItemId,
    ) -> Result<Vec<ClassificationSuggestion>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, item_id, kind, category_id, when_value, provider_id, model,
                    confidence, rationale, status, context_hash, item_revision_hash,
                    created_at, decided_at
             FROM classification_suggestions
             WHERE item_id = ?1 AND status = 'pending'
             ORDER BY created_at ASC, id ASC",
        )?;
        let suggestions = stmt
            .query_map(
                params![item_id.to_string()],
                Self::row_to_classification_suggestion,
            )?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(AgletError::from)?;
        Ok(suggestions)
    }

    pub fn upsert_suggestion(&self, suggestion: &ClassificationSuggestion) -> Result<()> {
        self.conn.execute(
            "INSERT INTO classification_suggestions (
                id, item_id, kind, category_id, when_value, provider_id, model, confidence,
                rationale, status, context_hash, item_revision_hash, created_at, decided_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
             ON CONFLICT(id) DO UPDATE SET
                item_id = excluded.item_id,
                kind = excluded.kind,
                category_id = excluded.category_id,
                when_value = excluded.when_value,
                provider_id = excluded.provider_id,
                model = excluded.model,
                confidence = excluded.confidence,
                rationale = excluded.rationale,
                status = excluded.status,
                context_hash = excluded.context_hash,
                item_revision_hash = excluded.item_revision_hash,
                created_at = excluded.created_at,
                decided_at = excluded.decided_at",
            params![
                suggestion.id.to_string(),
                suggestion.item_id.to_string(),
                suggestion.assignment.kind(),
                suggestion.assignment.category_id().map(|id| id.to_string()),
                suggestion
                    .assignment
                    .when_value()
                    .map(|value| value.to_string()),
                suggestion.provider_id,
                suggestion.model,
                suggestion.confidence,
                suggestion.rationale,
                suggestion_status_label(suggestion.status),
                suggestion.context_hash,
                suggestion.item_revision_hash,
                suggestion.created_at.to_string(),
                suggestion.decided_at.map(|value| value.to_string()),
            ],
        )?;
        Ok(())
    }

    pub fn set_suggestion_status(
        &self,
        suggestion_id: Uuid,
        status: SuggestionStatus,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE classification_suggestions
             SET status = ?2,
                 decided_at = CASE
                     WHEN ?2 IN ('accepted', 'rejected', 'superseded') THEN ?3
                     ELSE decided_at
                 END
             WHERE id = ?1",
            params![
                suggestion_id.to_string(),
                suggestion_status_label(status),
                Timestamp::now().to_string(),
            ],
        )?;
        Ok(())
    }

    pub fn supersede_suggestions_for_item_revision(
        &self,
        item_id: ItemId,
        new_revision_hash: &str,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE classification_suggestions
             SET status = 'superseded',
                 decided_at = ?3
             WHERE item_id = ?1
               AND status = 'pending'
               AND item_revision_hash <> ?2",
            params![
                item_id.to_string(),
                new_revision_hash,
                Timestamp::now().to_string()
            ],
        )?;
        Ok(())
    }

    // ── Item CRUD ──────────────────────────────────────────────

    pub fn create_item(&self, item: &Item) -> Result<()> {
        let recurrence_json = item
            .recurrence_rule
            .as_ref()
            .map(|r| serde_json::to_string(r).expect("RecurrenceRule is always serialisable"));
        self.conn.execute(
            "INSERT INTO items (id, text, note, created_at, modified_at, entry_date, when_date, done_date, is_done, recurrence_rule_json, recurrence_series_id, recurrence_parent_item_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                item.id.to_string(),
                item.text,
                item.note,
                item.created_at.to_string(),
                item.modified_at.to_string(),
                item.created_at.to_zoned(jiff::tz::TimeZone::UTC).date().to_string(),
                item.when_date.map(|d| d.to_string()),
                item.done_date.map(|d| d.to_string()),
                item.is_done as i32,
                recurrence_json,
                item.recurrence_series_id.map(|id| id.to_string()),
                item.recurrence_parent_item_id.map(|id| id.to_string()),
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
            return Err(AgletError::InvalidOperation {
                message: "empty item id prefix".to_string(),
            });
        }
        // Only allow valid hex characters
        if !normalized.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(AgletError::InvalidOperation {
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
            0 => Err(AgletError::InvalidOperation {
                message: format!("no item found matching prefix: {prefix}"),
            }),
            1 => {
                let id = Uuid::parse_str(&matches[0]).map_err(|e| AgletError::StorageError {
                    source: Box::new(e),
                })?;
                Ok(id)
            }
            _ => Err(AgletError::AmbiguousId {
                prefix: prefix.to_string(),
                matches,
            }),
        }
    }

    pub fn get_item(&self, id: ItemId) -> Result<Item> {
        let mut stmt = self.conn.prepare(
            "SELECT id, text, note, created_at, modified_at, entry_date, when_date, done_date, is_done, recurrence_rule_json, recurrence_series_id, recurrence_parent_item_id
             FROM items WHERE id = ?1",
        )?;
        let item = stmt
            .query_row(params![id.to_string()], Self::row_to_item)
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => AgletError::NotFound { entity: "Item", id },
                other => AgletError::from(other),
            })?;
        self.load_assignments(item)
    }

    pub fn update_item(&self, item: &Item) -> Result<()> {
        let recurrence_json = item
            .recurrence_rule
            .as_ref()
            .map(|r| serde_json::to_string(r).expect("RecurrenceRule is always serialisable"));
        let rows = self.conn.execute(
            "UPDATE items SET text = ?1, note = ?2, modified_at = ?3, when_date = ?4, done_date = ?5, is_done = ?6, recurrence_rule_json = ?7, recurrence_series_id = ?8, recurrence_parent_item_id = ?9
             WHERE id = ?10",
            params![
                item.text,
                item.note,
                item.modified_at.to_string(),
                item.when_date.map(|d| d.to_string()),
                item.done_date.map(|d| d.to_string()),
                item.is_done as i32,
                recurrence_json,
                item.recurrence_series_id.map(|id| id.to_string()),
                item.recurrence_parent_item_id.map(|id| id.to_string()),
                item.id.to_string(),
            ],
        )?;
        if rows == 0 {
            return Err(AgletError::NotFound {
                entity: "Item",
                id: item.id,
            });
        }
        Ok(())
    }

    /// Delete an item. Writes to deletion_log first, then removes from items table.
    pub fn delete_item(&self, id: ItemId, deleted_by: &str) -> Result<()> {
        let item = self.get_item(id)?;
        let assignments_json = serde_json::to_string(&item.assignments)
            .expect("BTreeMap<CategoryId, Assignment> is always serialisable");

        self.conn.execute(
            "INSERT INTO deletion_log (id, item_id, text, note, entry_date, when_date, done_date, is_done, assignments_json, deleted_at, deleted_by)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                Uuid::new_v4().to_string(),
                item.id.to_string(),
                item.text,
                item.note,
                item.created_at.to_zoned(jiff::tz::TimeZone::UTC).date().to_string(),
                item.when_date.map(|d| d.to_string()),
                item.done_date.map(|d| d.to_string()),
                item.is_done as i32,
                assignments_json,
                Timestamp::now().to_string(),
                deleted_by,
            ],
        )?;

        // CASCADE deletes assignments automatically.
        self.conn
            .execute("DELETE FROM items WHERE id = ?1", params![id.to_string()])?;
        Ok(())
    }

    /// Check if a successor item already exists for the given parent item.
    pub fn has_recurrence_successor(&self, parent_item_id: ItemId) -> Result<bool> {
        let count: i32 = self.conn.query_row(
            "SELECT COUNT(*) FROM items WHERE recurrence_parent_item_id = ?1",
            params![parent_item_id.to_string()],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    pub fn list_items(&self) -> Result<Vec<Item>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, text, note, created_at, modified_at, entry_date, when_date, done_date, is_done, recurrence_rule_json, recurrence_series_id, recurrence_parent_item_id
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
                rusqlite::Error::QueryReturnedNoRows => AgletError::NotFound {
                    entity: "DeletionLogEntry",
                    id: log_entry_id,
                },
                other => AgletError::from(other),
            })?;

        if self.get_item(entry.item_id).is_ok() {
            return Err(AgletError::InvalidOperation {
                message: format!("item {} already exists", entry.item_id),
            });
        }

        let now = Timestamp::now();
        let item = Item {
            id: entry.item_id,
            text: entry.text,
            note: entry.note,
            created_at: now,
            modified_at: now,
            when_date: entry.when_date,
            done_date: entry.done_date,
            is_done: entry.is_done,
            assignments: HashMap::new(),
            recurrence_rule: None,
            recurrence_series_id: None,
            recurrence_parent_item_id: None,
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

    // ── Category CRUD ───────────────────────────────────────────

    pub fn create_category(&self, category: &Category) -> Result<()> {
        if Self::is_reserved_category_name(&category.name) {
            return Err(AgletError::ReservedName {
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
            AgletError::StorageError {
                source: Box::new(err),
            }
        })?;
        let actions_json =
            serde_json::to_string(&category.actions).map_err(|err| AgletError::StorageError {
                source: Box::new(err),
            })?;
        let also_match_json = serde_json::to_string(&category.also_match).map_err(|err| {
            AgletError::StorageError {
                source: Box::new(err),
            }
        })?;
        let numeric_format_json =
            serde_json::to_string(&category.numeric_format).map_err(|err| {
                AgletError::StorageError {
                    source: Box::new(err),
                }
            })?;

        let sort_order = self.next_category_sort_order(category.parent)?;

        self.conn
            .execute(
                "INSERT INTO categories (
                    id, name, parent_id, is_exclusive, is_actionable, enable_implicit_string,
                    enable_semantic_classification, match_category_name, also_match_json, note,
                    created_at, modified_at, condition_match_mode, sort_order, conditions_json,
                    actions_json, value_kind, numeric_format_json, allow_delete_action
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
                params![
                    category.id.to_string(),
                    category.name,
                    category.parent.map(|id| id.to_string()),
                    category.is_exclusive as i32,
                    category.is_actionable as i32,
                    category.enable_implicit_string as i32,
                    category.enable_semantic_classification as i32,
                    category.match_category_name as i32,
                    also_match_json,
                    category.note,
                    category.created_at.to_string(),
                    category.modified_at.to_string(),
                    Self::condition_match_mode_to_db(category.condition_match_mode),
                    sort_order,
                    conditions_json,
                    actions_json,
                    Self::category_value_kind_to_db(category.value_kind),
                    numeric_format_json,
                    category.allow_delete_action as i32,
                ],
            )
            .map_err(|err| Self::map_category_write_error(err, &category.name))?;

        Ok(())
    }

    pub fn get_category(&self, id: CategoryId) -> Result<Category> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, parent_id, is_exclusive, is_actionable, enable_implicit_string,
                    enable_semantic_classification, match_category_name, also_match_json, note,
                    created_at, modified_at, condition_match_mode, conditions_json, actions_json,
                    sort_order, value_kind, numeric_format_json, allow_delete_action
             FROM categories WHERE id = ?1",
        )?;
        let (mut category, _) = stmt
            .query_row(params![id.to_string()], Self::row_to_category)
            .map_err(|err| match err {
                rusqlite::Error::QueryReturnedNoRows => AgletError::NotFound {
                    entity: "Category",
                    id,
                },
                other => AgletError::from(other),
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
            return Err(AgletError::ReservedName {
                name: category.name.clone(),
            });
        }
        if Self::is_reserved_category_name(&existing.name)
            && !existing.name.eq_ignore_ascii_case(&category.name)
        {
            return Err(AgletError::ReservedName {
                name: existing.name,
            });
        }
        if category.parent == Some(category.id) {
            return Err(AgletError::InvalidOperation {
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
            AgletError::StorageError {
                source: Box::new(err),
            }
        })?;
        let actions_json =
            serde_json::to_string(&category.actions).map_err(|err| AgletError::StorageError {
                source: Box::new(err),
            })?;
        let also_match_json = serde_json::to_string(&category.also_match).map_err(|err| {
            AgletError::StorageError {
                source: Box::new(err),
            }
        })?;
        let numeric_format_json =
            serde_json::to_string(&category.numeric_format).map_err(|err| {
                AgletError::StorageError {
                    source: Box::new(err),
                }
            })?;
        let modified_at = Timestamp::now();

        self.conn
            .execute(
                "UPDATE categories
                 SET name = ?1,
                     parent_id = ?2,
                     is_exclusive = ?3,
                     is_actionable = ?4,
                     enable_implicit_string = ?5,
                     enable_semantic_classification = ?6,
                     match_category_name = ?7,
                     also_match_json = ?8,
                     note = ?9,
                     modified_at = ?10,
                     condition_match_mode = ?11,
                     conditions_json = ?12,
                     actions_json = ?13,
                     value_kind = ?14,
                     numeric_format_json = ?15,
                     allow_delete_action = ?16
                 WHERE id = ?17",
                params![
                    category.name,
                    category.parent.map(|id| id.to_string()),
                    category.is_exclusive as i32,
                    category.is_actionable as i32,
                    category.enable_implicit_string as i32,
                    category.enable_semantic_classification as i32,
                    category.match_category_name as i32,
                    also_match_json,
                    category.note,
                    modified_at.to_string(),
                    Self::condition_match_mode_to_db(category.condition_match_mode),
                    conditions_json,
                    actions_json,
                    Self::category_value_kind_to_db(category.value_kind),
                    numeric_format_json,
                    category.allow_delete_action as i32,
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
            return Err(AgletError::NotFound {
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
            return Err(AgletError::InvalidOperation {
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
            return Err(AgletError::NotFound {
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
            return Err(AgletError::ReservedName {
                name: category.name,
            });
        }
        if !category.children.is_empty() {
            return Err(AgletError::InvalidOperation {
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
            return Err(AgletError::NotFound {
                entity: "Category",
                id,
            });
        }
        Ok(())
    }

    pub fn get_hierarchy(&self) -> Result<Vec<Category>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, parent_id, is_exclusive, is_actionable, enable_implicit_string,
                    enable_semantic_classification, match_category_name, also_match_json, note,
                    created_at, modified_at, condition_match_mode, conditions_json, actions_json,
                    sort_order, value_kind, numeric_format_json, allow_delete_action
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
        if let Some(system_name) = canonical_system_view_name(&view.name) {
            return Err(AgletError::InvalidOperation {
                message: format!("cannot create system view: {system_name}"),
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

    fn insert_view(&self, view: &View) -> Result<()> {
        let criteria_json =
            serde_json::to_string(&view.criteria).map_err(|err| AgletError::StorageError {
                source: Box::new(err),
            })?;
        let sections_json =
            serde_json::to_string(&view.sections).map_err(|err| AgletError::StorageError {
                source: Box::new(err),
            })?;
        let remove_from_view_unassign_json = serde_json::to_string(&view.remove_from_view_unassign)
            .map_err(|err| AgletError::StorageError {
                source: Box::new(err),
            })?;
        let category_aliases_json =
            serde_json::to_string(&view.category_aliases).map_err(|err| {
                AgletError::StorageError {
                    source: Box::new(err),
                }
            })?;

        let datebook_config_json: Option<String> = view
            .datebook_config
            .as_ref()
            .map(|c| serde_json::to_string(c).expect("DatebookConfig serializable"));

        self.conn
            .execute(
                "INSERT INTO views (
                    id, name, criteria_json, sections_json, columns_json,
                    show_unmatched, unmatched_label, remove_from_view_unassign_json,
                    category_aliases_json, item_column_label, board_display_mode,
                    section_flow, empty_sections, hide_dependent_items, datebook_config_json
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
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
                    serde_json::to_string(&view.section_flow)
                        .unwrap_or_else(|_| "\"Vertical\"".to_string()),
                    serde_json::to_string(&view.empty_sections)
                        .unwrap_or_else(|_| "\"Show\"".to_string()),
                    view.hide_dependent_items as i32,
                    datebook_config_json,
                ],
            )
            .map_err(|err| Self::map_view_write_error(err, &view.name))?;

        Ok(())
    }

    pub fn get_view(&self, id: Uuid) -> Result<View> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, criteria_json, sections_json, columns_json,
                    show_unmatched, unmatched_label, remove_from_view_unassign_json,
                    category_aliases_json, item_column_label, board_display_mode,
                    section_flow, empty_sections, hide_dependent_items, datebook_config_json
             FROM views WHERE id = ?1",
        )?;
        stmt.query_row(params![id.to_string()], Self::row_to_view)
            .map_err(|err| match err {
                rusqlite::Error::QueryReturnedNoRows => AgletError::NotFound { entity: "View", id },
                other => AgletError::from(other),
            })
    }

    pub fn update_view(&self, view: &View) -> Result<()> {
        let existing = self.get_view(view.id)?;
        if is_system_view_name(&existing.name) {
            return Err(AgletError::InvalidOperation {
                message: format!("cannot modify system view: {}", existing.name),
            });
        }
        if let Some(system_name) = canonical_system_view_name(&view.name) {
            return Err(AgletError::InvalidOperation {
                message: format!(
                    "cannot rename view {} to reserved system view name: {system_name}",
                    existing.name,
                ),
            });
        }

        let criteria_json =
            serde_json::to_string(&view.criteria).map_err(|err| AgletError::StorageError {
                source: Box::new(err),
            })?;
        let sections_json =
            serde_json::to_string(&view.sections).map_err(|err| AgletError::StorageError {
                source: Box::new(err),
            })?;
        let remove_from_view_unassign_json = serde_json::to_string(&view.remove_from_view_unassign)
            .map_err(|err| AgletError::StorageError {
                source: Box::new(err),
            })?;
        let category_aliases_json =
            serde_json::to_string(&view.category_aliases).map_err(|err| {
                AgletError::StorageError {
                    source: Box::new(err),
                }
            })?;

        let datebook_config_json: Option<String> = view
            .datebook_config
            .as_ref()
            .map(|c| serde_json::to_string(c).expect("DatebookConfig serializable"));

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
                     board_display_mode = ?10,
                     section_flow = ?11,
                     empty_sections = ?12,
                     hide_dependent_items = ?13,
                     datebook_config_json = ?14
                 WHERE id = ?15",
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
                    serde_json::to_string(&view.section_flow)
                        .unwrap_or_else(|_| "\"Vertical\"".to_string()),
                    serde_json::to_string(&view.empty_sections)
                        .unwrap_or_else(|_| "\"Show\"".to_string()),
                    view.hide_dependent_items as i32,
                    datebook_config_json,
                    view.id.to_string(),
                ],
            )
            .map_err(|err| Self::map_view_write_error(err, &view.name))?;
        if rows == 0 {
            return Err(AgletError::NotFound {
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
                    category_aliases_json, item_column_label, board_display_mode,
                    section_flow, empty_sections, hide_dependent_items, datebook_config_json
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
        if is_system_view_name(&existing.name) {
            return Err(AgletError::InvalidOperation {
                message: format!("cannot modify system view: {}", existing.name),
            });
        }

        let rows = self
            .conn
            .execute("DELETE FROM views WHERE id = ?1", params![id.to_string()])?;
        if rows == 0 {
            return Err(AgletError::NotFound { entity: "View", id });
        }
        Ok(())
    }

    // ── Item helpers ───────────────────────────────────────────

    fn row_to_item(row: &Row<'_>) -> rusqlite::Result<Item> {
        let id_str: String = row.get(0)?;
        let created_str: String = row.get(3)?;
        let modified_str: String = row.get(4)?;
        let _entry_str: String = row.get(5)?;
        let when_str: Option<String> = row.get(6)?;
        let done_str: Option<String> = row.get(7)?;
        let is_done_int: i32 = row.get(8)?;
        let recurrence_json: Option<String> = row.get(9)?;
        let series_id_str: Option<String> = row.get(10)?;
        let parent_id_str: Option<String> = row.get(11)?;

        Ok(Item {
            id: Uuid::parse_str(&id_str).unwrap_or_default(),
            text: row.get(1)?,
            note: row.get(2)?,
            created_at: created_str.parse::<Timestamp>().unwrap_or_default(),
            modified_at: modified_str.parse::<Timestamp>().unwrap_or_default(),
            when_date: when_str.and_then(|s| s.parse::<jiff::civil::DateTime>().ok()),
            done_date: done_str.and_then(|s| s.parse::<jiff::civil::DateTime>().ok()),
            is_done: is_done_int != 0,
            assignments: HashMap::new(),
            recurrence_rule: recurrence_json
                .and_then(|s| serde_json::from_str::<RecurrenceRule>(&s).ok()),
            recurrence_series_id: series_id_str.and_then(|s| Uuid::parse_str(&s).ok()),
            recurrence_parent_item_id: parent_id_str.and_then(|s| Uuid::parse_str(&s).ok()),
        })
    }

    fn row_to_deleted_item(row: &Row<'_>) -> rusqlite::Result<DeletionLogEntry> {
        let id_str: String = row.get(0)?;
        let item_id_str: String = row.get(1)?;
        let _entry_str: String = row.get(4)?;
        let when_str: Option<String> = row.get(5)?;
        let done_str: Option<String> = row.get(6)?;
        let is_done_int: i32 = row.get(7)?;
        let deleted_at_str: String = row.get(9)?;

        Ok(DeletionLogEntry {
            id: Uuid::parse_str(&id_str).unwrap_or_default(),
            item_id: Uuid::parse_str(&item_id_str).unwrap_or_default(),
            text: row.get(2)?,
            note: row.get(3)?,
            when_date: when_str.and_then(|s| s.parse::<jiff::civil::DateTime>().ok()),
            done_date: done_str.and_then(|s| s.parse::<jiff::civil::DateTime>().ok()),
            is_done: is_done_int != 0,
            assignments_json: row.get(8)?,
            deleted_at: deleted_at_str.parse::<Timestamp>().unwrap_or_default(),
            deleted_by: row.get(10)?,
        })
    }

    fn load_assignments(&self, mut item: Item) -> Result<Item> {
        let mut stmt = self.conn.prepare(
            "SELECT category_id, source, assigned_at, sticky, origin, explanation_json, numeric_value
             FROM assignments WHERE item_id = ?1",
        )?;
        let rows = stmt.query_map(params![item.id.to_string()], |row| {
            let cat_str: String = row.get(0)?;
            let source_str: String = row.get(1)?;
            let assigned_str: String = row.get(2)?;
            let sticky_int: i32 = row.get(3)?;
            let origin: Option<String> = row.get(4)?;
            let explanation_json: String = row.get(5)?;
            let numeric_value: Option<String> = row.get(6)?;
            Ok((
                cat_str,
                source_str,
                assigned_str,
                sticky_int,
                origin,
                explanation_json,
                numeric_value,
            ))
        })?;

        for row in rows {
            let (
                cat_str,
                source_str,
                assigned_str,
                sticky_int,
                origin,
                explanation_json,
                numeric_value_str,
            ) = row?;
            let cat_id = Uuid::parse_str(&cat_str).unwrap_or_default();
            let source = match source_str.as_str() {
                "Manual" => AssignmentSource::Manual,
                "AutoMatch" => AssignmentSource::AutoMatch,
                "AutoClassified" => AssignmentSource::AutoClassified,
                "SuggestionAccepted" => AssignmentSource::SuggestionAccepted,
                "Action" => AssignmentSource::Action,
                "Subsumption" => AssignmentSource::Subsumption,
                _ => AssignmentSource::Manual,
            };
            let assigned_at = assigned_str.parse::<Timestamp>().unwrap_or_default();
            let explanation =
                serde_json::from_str::<Option<AssignmentExplanation>>(&explanation_json)
                    .unwrap_or_default();
            let numeric_value = numeric_value_str.and_then(|s| s.parse::<Decimal>().ok());
            item.assignments.insert(
                cat_id,
                Assignment {
                    source,
                    assigned_at,
                    sticky: sticky_int != 0,
                    origin,
                    explanation,
                    numeric_value,
                },
            );
        }
        Ok(item)
    }

    fn row_to_classification_suggestion(
        row: &Row<'_>,
    ) -> rusqlite::Result<ClassificationSuggestion> {
        let id: String = row.get(0)?;
        let item_id: String = row.get(1)?;
        let kind: String = row.get(2)?;
        let category_id: Option<String> = row.get(3)?;
        let when_value: Option<String> = row.get(4)?;
        let status: String = row.get(9)?;
        let created_at: String = row.get(12)?;
        let decided_at: Option<String> = row.get(13)?;

        let assignment = match kind.as_str() {
            "category" => CandidateAssignment::Category(
                category_id
                    .and_then(|value| Uuid::parse_str(&value).ok())
                    .unwrap_or_default(),
            ),
            "when" => CandidateAssignment::When(
                when_value
                    .and_then(|value| value.parse::<jiff::civil::DateTime>().ok())
                    .unwrap_or_else(|| {
                        jiff::civil::DateTime::new(1970, 1, 1, 0, 0, 0, 0)
                            .expect("fallback datetime is valid")
                    }),
            ),
            _ => CandidateAssignment::When(
                jiff::civil::DateTime::new(1970, 1, 1, 0, 0, 0, 0)
                    .expect("fallback datetime is valid"),
            ),
        };

        Ok(ClassificationSuggestion {
            id: Uuid::parse_str(&id).unwrap_or_default(),
            item_id: Uuid::parse_str(&item_id).unwrap_or_default(),
            assignment,
            provider_id: row.get(5)?,
            model: row.get(6)?,
            confidence: row.get(7)?,
            rationale: row.get(8)?,
            status: suggestion_status_from_db(&status),
            context_hash: row.get(10)?,
            item_revision_hash: row.get(11)?,
            created_at: created_at.parse::<Timestamp>().unwrap_or_default(),
            decided_at: decided_at.and_then(|value| value.parse::<Timestamp>().ok()),
        })
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
        let section_flow_json: Option<String> = row.get(11)?;
        let empty_sections_json: Option<String> = row.get(12)?;
        let hide_dependent_items: Option<i32> = row.get(13)?;
        let datebook_config_json: Option<String> = row.get(14)?;

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
        let section_flow = section_flow_json
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or(SectionFlow::Vertical);
        let empty_sections = empty_sections_json
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or(EmptySections::Show);
        let datebook_config =
            datebook_config_json.and_then(|json| serde_json::from_str(&json).ok());

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
            section_flow,
            empty_sections,
            hide_dependent_items: hide_dependent_items.unwrap_or(0) != 0,
            datebook_config,
        })
    }

    // ── Item link persistence ──────────────────────────────────

    pub fn create_item_link(&self, link: &ItemLink) -> Result<()> {
        self.conn.execute(
            "INSERT INTO item_links (item_id, other_item_id, kind, created_at, origin)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                link.item_id.to_string(),
                link.other_item_id.to_string(),
                Self::item_link_kind_to_db(link.kind),
                link.created_at.to_string(),
                link.origin,
            ],
        )?;
        Ok(())
    }

    pub fn delete_item_link(
        &self,
        item_id: ItemId,
        other_item_id: ItemId,
        kind: ItemLinkKind,
    ) -> Result<()> {
        self.conn.execute(
            "DELETE FROM item_links
             WHERE item_id = ?1 AND other_item_id = ?2 AND kind = ?3",
            params![
                item_id.to_string(),
                other_item_id.to_string(),
                Self::item_link_kind_to_db(kind),
            ],
        )?;
        Ok(())
    }

    pub fn item_link_exists(
        &self,
        item_id: ItemId,
        other_item_id: ItemId,
        kind: ItemLinkKind,
    ) -> Result<bool> {
        let exists: Option<i32> = self
            .conn
            .query_row(
                "SELECT 1 FROM item_links
                 WHERE item_id = ?1 AND other_item_id = ?2 AND kind = ?3
                 LIMIT 1",
                params![
                    item_id.to_string(),
                    other_item_id.to_string(),
                    Self::item_link_kind_to_db(kind),
                ],
                |row| row.get(0),
            )
            .optional()?;
        Ok(exists.is_some())
    }

    /// Immediate prerequisites for a dependent item (outbound depends-on edges).
    pub fn list_dependency_ids_for_item(&self, item_id: ItemId) -> Result<Vec<ItemId>> {
        let mut stmt = self.conn.prepare(
            "SELECT other_item_id
             FROM item_links
             WHERE item_id = ?1 AND kind = 'depends-on'
             ORDER BY created_at ASC, other_item_id ASC",
        )?;
        let rows = stmt
            .query_map(params![item_id.to_string()], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        rows.into_iter()
            .map(|raw| Self::parse_uuid_from_db_text(&raw, "item_links.other_item_id"))
            .collect()
    }

    /// Immediate dependents of an item (inbound depends-on edges; inverse "blocks" view).
    pub fn list_dependent_ids_for_item(&self, item_id: ItemId) -> Result<Vec<ItemId>> {
        let mut stmt = self.conn.prepare(
            "SELECT item_id
             FROM item_links
             WHERE other_item_id = ?1 AND kind = 'depends-on'
             ORDER BY created_at ASC, item_id ASC",
        )?;
        let rows = stmt
            .query_map(params![item_id.to_string()], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        rows.into_iter()
            .map(|raw| Self::parse_uuid_from_db_text(&raw, "item_links.item_id"))
            .collect()
    }

    /// Immediate related items (symmetric query over normalized `related` rows).
    pub fn list_related_ids_for_item(&self, item_id: ItemId) -> Result<Vec<ItemId>> {
        let mut stmt = self.conn.prepare(
            "SELECT CASE WHEN item_id = ?1 THEN other_item_id ELSE item_id END AS neighbor_id
             FROM item_links
             WHERE kind = 'related' AND (item_id = ?1 OR other_item_id = ?1)
             ORDER BY created_at ASC, neighbor_id ASC",
        )?;
        let rows = stmt
            .query_map(params![item_id.to_string()], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        rows.into_iter()
            .map(|raw| Self::parse_uuid_from_db_text(&raw, "item_links.related_neighbor_id"))
            .collect()
    }

    /// Optional convenience for `aglet show` / TUI panels.
    pub fn list_item_links_for_item(&self, item_id: ItemId) -> Result<Vec<ItemLink>> {
        let mut stmt = self.conn.prepare(
            "SELECT item_id, other_item_id, kind, created_at, origin
             FROM item_links
             WHERE item_id = ?1 OR other_item_id = ?1
             ORDER BY created_at ASC, item_id ASC, other_item_id ASC, kind ASC",
        )?;
        let rows = stmt
            .query_map(params![item_id.to_string()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        rows.into_iter()
            .map(
                |(item_id_str, other_item_id_str, kind_str, created_at_str, origin)| {
                    Self::item_link_from_db_row(
                        &item_id_str,
                        &other_item_id_str,
                        &kind_str,
                        &created_at_str,
                        origin,
                    )
                },
            )
            .collect()
    }

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
        let created_at = created_at_str.parse::<Timestamp>().map_err(|e| {
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

    fn storage_data_error(message: String) -> AgletError {
        AgletError::StorageError {
            source: Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                message,
            )),
        }
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
        self.conn.execute(
            "INSERT OR REPLACE INTO assignments (item_id, category_id, source, assigned_at, sticky, origin, explanation_json, numeric_value)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                item_id.to_string(),
                category_id.to_string(),
                assignment_source_label(assignment.source),
                assignment.assigned_at.to_string(),
                assignment.sticky as i32,
                assignment.origin,
                serde_json::to_string(&assignment.explanation).map_err(|err| {
                    AgletError::StorageError {
                        source: Box::new(err),
                    }
                })?,
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

    /// Record a negative assignment (Agenda's `-`): the engine must never
    /// auto-assign this item to this category again. Idempotent.
    pub fn add_assignment_veto(
        &self,
        item_id: ItemId,
        category_id: CategoryId,
        origin: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO assignment_vetoes (item_id, category_id, created_at, origin)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                item_id.to_string(),
                category_id.to_string(),
                Timestamp::now().to_string(),
                origin,
            ],
        )?;
        Ok(())
    }

    /// Clear a negative assignment. Returns Ok even if no veto existed.
    pub fn remove_assignment_veto(&self, item_id: ItemId, category_id: CategoryId) -> Result<()> {
        self.conn.execute(
            "DELETE FROM assignment_vetoes WHERE item_id = ?1 AND category_id = ?2",
            params![item_id.to_string(), category_id.to_string()],
        )?;
        Ok(())
    }

    /// All vetoes in the database, grouped by item — one query for UI caches.
    pub fn list_assignment_vetoes(&self) -> Result<HashMap<ItemId, HashSet<CategoryId>>> {
        let mut stmt = self
            .conn
            .prepare("SELECT item_id, category_id FROM assignment_vetoes")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut vetoes: HashMap<ItemId, HashSet<CategoryId>> = HashMap::new();
        for row in rows {
            let (item_str, category_str) = row?;
            if let (Ok(item_id), Ok(category_id)) =
                (Uuid::parse_str(&item_str), Uuid::parse_str(&category_str))
            {
                vetoes.entry(item_id).or_default().insert(category_id);
            }
        }
        Ok(vetoes)
    }

    /// All categories the user has vetoed for this item.
    pub fn get_vetoes_for_item(&self, item_id: ItemId) -> Result<HashSet<CategoryId>> {
        let mut stmt = self
            .conn
            .prepare("SELECT category_id FROM assignment_vetoes WHERE item_id = ?1")?;
        let rows = stmt.query_map(params![item_id.to_string()], |row| {
            row.get::<_, String>(0)
        })?;
        let mut vetoes = HashSet::new();
        for row in rows {
            let id_str = row?;
            if let Ok(id) = Uuid::parse_str(&id_str) {
                vetoes.insert(id);
            }
        }
        Ok(vetoes)
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
        let enable_semantic_classification: i32 = row.get(6)?;
        let match_category_name: i32 = row.get(7)?;
        let also_match_json: String = row.get(8)?;
        let created_str: String = row.get(10)?;
        let modified_str: String = row.get(11)?;
        let condition_match_mode_str: String = row.get(12)?;
        let conditions_json: String = row.get(13)?;
        let actions_json: String = row.get(14)?;
        let sort_order: i64 = row.get(15)?;
        let value_kind_str: String = row.get(16)?;
        let numeric_format_json: String = row.get(17)?;
        let allow_delete_action: i32 = row.get(18)?;

        // Corrupt or legacy category row: fall back to no conditions/actions
        // so the category still loads without its rules rather than failing.
        let mut conditions: Vec<Condition> =
            serde_json::from_str(&conditions_json).unwrap_or_default();
        // Legacy no-op marker: implicit text matching is governed by the
        // enable_implicit_string flag, never by a stored condition. Strip on
        // load; nothing writes it anymore, so it ages out of stored rows on
        // the next category save.
        conditions.retain(|condition| !matches!(condition, Condition::ImplicitString));
        let actions: Vec<Action> = serde_json::from_str(&actions_json).unwrap_or_default();
        let also_match: Vec<String> = serde_json::from_str(&also_match_json).unwrap_or_default();
        let value_kind = Self::category_value_kind_from_db(&value_kind_str);
        let condition_match_mode = Self::condition_match_mode_from_db(&condition_match_mode_str);
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
                enable_semantic_classification: enable_semantic_classification != 0,
                match_category_name: match_category_name != 0,
                also_match,
                note: row.get(9)?,
                created_at: created_str.parse::<Timestamp>().unwrap_or_default(),
                modified_at: modified_str.parse::<Timestamp>().unwrap_or_default(),
                condition_match_mode,
                conditions,
                actions,
                value_kind,
                numeric_format,
                allow_delete_action: allow_delete_action != 0,
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
                return Err(AgletError::NotFound {
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
        let modified_at = Timestamp::now();
        let rows = self.conn.execute(
            "UPDATE categories
             SET parent_id = ?1, modified_at = ?2
             WHERE id = ?3",
            params![
                new_parent_id.map(|id| id.to_string()),
                modified_at.to_string(),
                category_id.to_string()
            ],
        )?;
        if rows == 0 {
            return Err(AgletError::NotFound {
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
                return Err(AgletError::InvalidOperation {
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
    fn map_write_error(err: rusqlite::Error, name: &str, table_column: &str) -> AgletError {
        match err {
            rusqlite::Error::SqliteFailure(sqlite_err, _)
                if sqlite_err.code == rusqlite::ErrorCode::ConstraintViolation
                    && sqlite_err.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE =>
            {
                AgletError::DuplicateName {
                    name: name.to_string(),
                }
            }
            rusqlite::Error::SqliteFailure(sqlite_err, Some(ref message))
                if sqlite_err.code == rusqlite::ErrorCode::ConstraintViolation
                    && message.contains(table_column) =>
            {
                AgletError::DuplicateName {
                    name: name.to_string(),
                }
            }
            other => AgletError::from(other),
        }
    }

    fn map_category_write_error(err: rusqlite::Error, category_name: &str) -> AgletError {
        Self::map_write_error(err, category_name, "categories.name")
    }

    fn map_view_write_error(err: rusqlite::Error, view_name: &str) -> AgletError {
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

    fn condition_match_mode_to_db(mode: ConditionMatchMode) -> &'static str {
        match mode {
            ConditionMatchMode::Any => "Any",
            ConditionMatchMode::All => "All",
        }
    }

    fn condition_match_mode_from_db(raw: &str) -> ConditionMatchMode {
        if raw.eq_ignore_ascii_case("all") {
            ConditionMatchMode::All
        } else {
            ConditionMatchMode::Any
        }
    }

    fn validate_parent_accepts_children(parent: &Category) -> Result<()> {
        if parent.value_kind == CategoryValueKind::Numeric {
            return Err(AgletError::InvalidOperation {
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
            return Err(AgletError::InvalidOperation {
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
                    return Err(AgletError::InvalidOperation {
                        message: format!(
                            "cannot convert category '{}' to Numeric while it has children",
                            existing.name
                        ),
                    });
                }
                if self.category_assignment_count(existing.id)? > 0 {
                    return Err(AgletError::InvalidOperation {
                        message: format!(
                            "cannot convert category '{}' to Numeric after assignments already exist",
                            existing.name
                        ),
                    });
                }
                Ok(())
            }
            (CategoryValueKind::Numeric, CategoryValueKind::Tag) => {
                Err(AgletError::InvalidOperation {
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
        let now = Timestamp::now().to_string();
        let sort_order = self.next_category_sort_order(None)?;

        // Reserved categories have implicit string matching disabled by default.
        // "Done", "When", "Entry" should not auto-match item text containing
        // those common words.
        self.conn
            .execute(
                "INSERT INTO categories (
                    id, name, parent_id, is_exclusive, is_actionable, enable_implicit_string, note,
                    created_at, modified_at, condition_match_mode, sort_order, conditions_json,
                    actions_json, value_kind, numeric_format_json
                 ) VALUES (?1, ?2, NULL, 0, 0, 0, NULL, ?3, ?3, 'Any', ?4, '[]', '[]', 'Tag', 'null')",
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

        when_category_id.ok_or_else(|| AgletError::StorageError {
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
                .map_err(|e| AgletError::StorageError {
                    source: Box::new(e),
                })?;
        }
        // Some local DBs have been stamped with the current schema version by
        // partial development builds while still missing columns. The migration
        // body is intentionally idempotent, so run it on every open to repair
        // those drifted schemas before any SELECT references the new columns.
        self.apply_migrations(version)?;
        if version < SCHEMA_VERSION {
            self.conn
                .pragma_update(None, "user_version", SCHEMA_VERSION)?;
        }

        let when_category_id = self.ensure_reserved_categories()?;
        self.ensure_default_view(when_category_id)?;

        Ok(())
    }

    /// Versioned migrations in ascending order, followed by an idempotent
    /// column-repair pass for DBs stamped with a current version by partial
    /// development builds. Add new migrations to the END of the table with
    /// the next schema version; keep bodies idempotent as defense in depth.
    fn apply_migrations(&self, from_version: i32) -> Result<()> {
        type Migration = (i32, fn(&Store) -> Result<()>);
        const MIGRATIONS: &[Migration] = &[
            (2, Store::migrate_v2_actionable_flag),
            (3, Store::migrate_v3_column_kinds),
            (6, Store::migrate_v6_item_links),
            (8, Store::migrate_v8_app_settings),
            (11, Store::migrate_v11_suggestions_and_datetime_format),
            (16, Store::migrate_v16_recurrence_columns),
            (20, Store::migrate_v20_assignment_vetoes),
        ];
        for (version, migrate) in MIGRATIONS {
            if from_version < *version {
                migrate(self)?;
            }
        }
        self.repair_missing_columns()
    }

    fn migrate_v2_actionable_flag(&self) -> Result<()> {
        if self.column_exists("categories", "is_actionable")? {
            return Ok(());
        }
        self.conn.execute_batch(
            "ALTER TABLE categories ADD COLUMN is_actionable INTEGER NOT NULL DEFAULT 1;",
        )?;
        self.conn.execute(
            "UPDATE categories
             SET is_actionable = 0
             WHERE name IN ('When', 'Entry', 'Done') COLLATE NOCASE",
            [],
        )?;
        Ok(())
    }

    fn migrate_v3_column_kinds(&self) -> Result<()> {
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
        Ok(())
    }

    fn migrate_v6_item_links(&self) -> Result<()> {
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
        Ok(())
    }

    fn migrate_v8_app_settings(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS app_settings (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            "#,
        )?;
        Ok(())
    }

    fn migrate_v11_suggestions_and_datetime_format(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS classification_suggestions (
                id                 TEXT PRIMARY KEY,
                item_id            TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,
                kind               TEXT NOT NULL,
                category_id        TEXT,
                when_value         TEXT,
                provider_id        TEXT NOT NULL,
                model              TEXT,
                confidence         REAL,
                rationale          TEXT,
                status             TEXT NOT NULL,
                context_hash       TEXT NOT NULL,
                item_revision_hash TEXT NOT NULL,
                created_at         TEXT NOT NULL,
                decided_at         TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_classification_suggestions_item_id
                ON classification_suggestions (item_id);
            CREATE INDEX IF NOT EXISTS idx_classification_suggestions_status
                ON classification_suggestions (status);
            "#,
        )?;
        // Migrate when_date / done_date from "YYYY-MM-DD HH:MM:SS" to "YYYY-MM-DDTHH:MM:SS"
        self.conn.execute_batch(
            "UPDATE items SET when_date = REPLACE(when_date, ' ', 'T') WHERE when_date IS NOT NULL;
             UPDATE items SET done_date = REPLACE(done_date, ' ', 'T') WHERE done_date IS NOT NULL;
             UPDATE deletion_log SET when_date = REPLACE(when_date, ' ', 'T') WHERE when_date IS NOT NULL;
             UPDATE deletion_log SET done_date = REPLACE(done_date, ' ', 'T') WHERE done_date IS NOT NULL;",
        )?;
        Ok(())
    }

    fn migrate_v16_recurrence_columns(&self) -> Result<()> {
        if !self.column_exists("items", "recurrence_rule_json")? {
            self.conn
                .execute_batch("ALTER TABLE items ADD COLUMN recurrence_rule_json TEXT;")?;
        }
        if !self.column_exists("items", "recurrence_series_id")? {
            self.conn
                .execute_batch("ALTER TABLE items ADD COLUMN recurrence_series_id TEXT;")?;
        }
        if !self.column_exists("items", "recurrence_parent_item_id")? {
            self.conn
                .execute_batch("ALTER TABLE items ADD COLUMN recurrence_parent_item_id TEXT;")?;
        }
        Ok(())
    }

    fn migrate_v20_assignment_vetoes(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS assignment_vetoes (
                item_id     TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,
                category_id TEXT NOT NULL REFERENCES categories(id) ON DELETE CASCADE,
                created_at  TEXT NOT NULL,
                origin      TEXT,
                PRIMARY KEY (item_id, category_id)
            );
            CREATE INDEX IF NOT EXISTS idx_assignment_vetoes_item
                ON assignment_vetoes(item_id);
            "#,
        )?;
        Ok(())
    }

    /// Idempotent column adds for DBs that were stamped with a current
    /// schema version by earlier partial development builds while still
    /// missing columns. Runs on every open, after versioned migrations.
    fn repair_missing_columns(&self) -> Result<()> {
        if !self.column_exists("views", "item_column_label")? {
            self.conn
                .execute_batch("ALTER TABLE views ADD COLUMN item_column_label TEXT;")?;
        }
        if !self.column_exists("views", "board_display_mode")? {
            self.conn.execute_batch(
                "ALTER TABLE views ADD COLUMN board_display_mode TEXT NOT NULL DEFAULT 'SingleLine';",
            )?;
        }
        if !self.column_exists("views", "section_flow")? {
            self.conn.execute_batch(
                "ALTER TABLE views ADD COLUMN section_flow TEXT NOT NULL DEFAULT 'Vertical';",
            )?;
        }
        let added_empty_sections_column = if !self.column_exists("views", "empty_sections")? {
            self.conn.execute_batch(
                "ALTER TABLE views ADD COLUMN empty_sections TEXT NOT NULL DEFAULT 'Show';",
            )?;
            true
        } else {
            false
        };
        if added_empty_sections_column && self.column_exists("views", "datebook_config_json")? {
            self.migrate_datebook_empty_sections_to_view()?;
        }
        if !self.column_exists("views", "category_aliases_json")? {
            self.conn.execute_batch(
                "ALTER TABLE views ADD COLUMN category_aliases_json TEXT NOT NULL DEFAULT '{}';",
            )?;
        }
        if !self.column_exists("views", "hide_dependent_items")? {
            self.conn.execute_batch(
                "ALTER TABLE views ADD COLUMN hide_dependent_items INTEGER NOT NULL DEFAULT 0;",
            )?;
        }
        if !self.column_exists("categories", "value_kind")? {
            self.conn.execute_batch(
                "ALTER TABLE categories ADD COLUMN value_kind TEXT NOT NULL DEFAULT 'Tag';",
            )?;
        }
        if !self.column_exists("categories", "allow_delete_action")? {
            self.conn.execute_batch(
                "ALTER TABLE categories ADD COLUMN allow_delete_action INTEGER NOT NULL DEFAULT 0;",
            )?;
        }
        if !self.column_exists("categories", "numeric_format_json")? {
            self.conn.execute_batch(
                "ALTER TABLE categories ADD COLUMN numeric_format_json TEXT NOT NULL DEFAULT 'null';",
            )?;
        }
        if !self.column_exists("categories", "also_match_json")? {
            self.conn.execute_batch(
                "ALTER TABLE categories ADD COLUMN also_match_json TEXT NOT NULL DEFAULT '[]';",
            )?;
        }
        if !self.column_exists("categories", "match_category_name")? {
            self.conn.execute_batch(
                "ALTER TABLE categories ADD COLUMN match_category_name INTEGER NOT NULL DEFAULT 1;",
            )?;
        }
        if !self.column_exists("categories", "enable_semantic_classification")? {
            self.conn.execute_batch(
                "ALTER TABLE categories ADD COLUMN enable_semantic_classification INTEGER NOT NULL DEFAULT 1;",
            )?;
        }
        if !self.column_exists("categories", "condition_match_mode")? {
            self.conn.execute_batch(
                "ALTER TABLE categories ADD COLUMN condition_match_mode TEXT NOT NULL DEFAULT 'Any';",
            )?;
        }
        if !self.column_exists("assignments", "explanation_json")? {
            self.conn.execute_batch(
                "ALTER TABLE assignments ADD COLUMN explanation_json TEXT NOT NULL DEFAULT 'null';",
            )?;
        }
        if !self.column_exists("assignments", "numeric_value")? {
            self.conn
                .execute_batch("ALTER TABLE assignments ADD COLUMN numeric_value TEXT;")?;
        }
        if !self.column_exists("views", "datebook_config_json")? {
            self.conn
                .execute_batch("ALTER TABLE views ADD COLUMN datebook_config_json TEXT;")?;
        }
        Ok(())
    }

    fn migrate_datebook_empty_sections_to_view(&self) -> Result<()> {
        let mut stmt = self.conn.prepare(
            "SELECT id, datebook_config_json FROM views WHERE datebook_config_json IS NOT NULL",
        )?;
        let rows = stmt
            .query_map([], |row| {
                let id: String = row.get(0)?;
                let config_json: String = row.get(1)?;
                Ok((id, config_json))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        drop(stmt);

        for (id, config_json) in rows {
            let Ok(config) = serde_json::from_str::<DatebookConfig>(&config_json) else {
                continue;
            };
            let empty_sections_json = serde_json::to_string(&config.empty_sections)
                .unwrap_or_else(|_| "\"Show\"".to_string());
            self.conn.execute(
                "UPDATE views SET empty_sections = ?1 WHERE id = ?2",
                params![empty_sections_json, id],
            )?;
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

fn assignment_source_label(source: AssignmentSource) -> &'static str {
    match source {
        AssignmentSource::Manual => "Manual",
        AssignmentSource::AutoMatch => "AutoMatch",
        AssignmentSource::AutoClassified => "AutoClassified",
        AssignmentSource::SuggestionAccepted => "SuggestionAccepted",
        AssignmentSource::Action => "Action",
        AssignmentSource::Subsumption => "Subsumption",
    }
}

fn suggestion_status_from_db(status: &str) -> SuggestionStatus {
    match status {
        "pending" => SuggestionStatus::Pending,
        "accepted" => SuggestionStatus::Accepted,
        "rejected" => SuggestionStatus::Rejected,
        "superseded" => SuggestionStatus::Superseded,
        _ => SuggestionStatus::Pending,
    }
}

fn suggestion_status_label(status: SuggestionStatus) -> &'static str {
    match status {
        SuggestionStatus::Pending => "pending",
        SuggestionStatus::Accepted => "accepted",
        SuggestionStatus::Rejected => "rejected",
        SuggestionStatus::Superseded => "superseded",
    }
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
