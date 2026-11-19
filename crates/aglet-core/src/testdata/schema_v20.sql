-- Schema v20, as written by aglet v0.1.0. Fixture for the v21 upgrade tests.
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
