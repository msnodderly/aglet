---
title: Codebase Walkthrough
updated: 2026-04-01
---

# Aglet Codebase Walkthrough

*2026-03-01 — updated to reflect current codebase*

Aglet is a personal agenda and task-management system built in Rust. It is a workspace of three crates:

- **`aglet-core`** — the domain library: data model, SQLite storage, a rule engine that auto-categorises items, natural-language date parsing, and view resolution.
- **`aglet-cli`** — a Clap-powered command-line interface.
- **`aglet-tui`** — an interactive terminal UI built with ratatui / crossterm.

The database is a single SQLite file (`.ag` extension). Items flow through a pipeline: text is parsed for dates, matched against category names for auto-assignment, and rules cascade through a fixed-point engine. Views then slice and group items for display.

We will walk through the code bottom-up, starting with the data model and working our way up to the frontends.

## 1. Workspace Layout

The workspace root `Cargo.toml` declares three member crates:

```bash
cat Cargo.toml
```

```output
[workspace]
resolver = "2"
members = [
    "crates/aglet-core",
    "crates/aglet-tui",
    "crates/aglet-cli",
]```
```

Each crate has a focused purpose. `aglet-core` depends on rusqlite, uuid, chrono, serde, and rust_decimal. The CLI adds clap. The TUI adds ratatui and crossterm. Both frontends depend on `aglet-core`.

## 2. The Data Model (`aglet-core/src/model.rs`)

Everything starts with four core entities: **Items**, **Categories**, **Views**, and **Assignments**. Let us look at each.

### Item Links

Before the core entities, the model defines **item-to-item links**. Links come in two kinds: `DependsOn` (directed dependency) and `Related` (bidirectional). The `ItemLinksForItem` struct provides a convenience view of all links for a single item.

```bash
sed -n "10,35p" crates/aglet-core/src/model.rs
```

```output
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ItemLinkKind {
    #[serde(rename = "depends-on")]
    DependsOn,
    #[serde(rename = "related")]
    Related,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemLink {
    /// Endpoint semantics depend on `kind`:
    /// - DependsOn: item_id = dependent, other_item_id = dependency
    /// - Related: normalized unordered pair (item_id < other_item_id)
    pub item_id: ItemId,
    pub other_item_id: ItemId,
    pub kind: ItemLinkKind,
    pub created_at: DateTime<Utc>,
    pub origin: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ItemLinksForItem {
    pub depends_on: Vec<ItemId>,
    pub blocks: Vec<ItemId>,
    pub related: Vec<ItemId>,
}
```

### Items

An Item is a task or note. It has text, an optional note, timestamps, an optional `when_date` (parsed from natural language), and a `done` state. Its `assignments` map holds all the categories it belongs to.

```bash
sed -n "37,49p" crates/aglet-core/src/model.rs
```

```output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    pub id: ItemId,
    pub text: String,
    pub note: Option<String>,
    pub created_at: DateTime<Utc>,
    pub modified_at: DateTime<Utc>,
    pub entry_date: NaiveDate,
    pub when_date: Option<NaiveDateTime>,
    pub done_date: Option<NaiveDateTime>,
    pub is_done: bool,
    pub assignments: HashMap<CategoryId, Assignment>,
}
```

### Assignments

Each assignment records *how* an item came to be in a category. The `AssignmentSource` enum distinguishes manual assignments from engine-driven ones: `Manual` (user did it), `AutoMatch` (implicit string matching), `Action` (a rule fired), or `Subsumption` (inherited from a child category up to its parent). Numeric categories can carry a `numeric_value` on their assignment.

```bash
sed -n "51,67p" crates/aglet-core/src/model.rs
```

```output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Assignment {
    pub source: AssignmentSource,
    pub assigned_at: DateTime<Utc>,
    pub sticky: bool,
    pub origin: Option<String>,
    #[serde(default)]
    pub numeric_value: Option<Decimal>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssignmentSource {
    Manual,
    AutoMatch,
    Action,
    Subsumption,
}
```

### Categories

Categories form a tree. A parent can be marked `is_exclusive`, meaning only one of its children can be assigned to an item at a time (e.g., a "Priority" parent with children "High" / "Medium" / "Low"). Categories can carry **conditions** (rules that auto-match items) and **actions** (side-effects that fire when the category is assigned). Categories also have a `value_kind` (Tag or Numeric) and an optional `numeric_format` for display formatting.

```bash
sed -n "69,130p" crates/aglet-core/src/model.rs
```

```output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Category {
    pub id: CategoryId,
    pub name: String,
    pub parent: Option<CategoryId>,
    pub children: Vec<CategoryId>,
    pub is_exclusive: bool,
    pub is_actionable: bool,
    pub enable_implicit_string: bool,
    pub note: Option<String>,
    pub created_at: DateTime<Utc>,
    pub modified_at: DateTime<Utc>,
    pub conditions: Vec<Condition>,
    pub actions: Vec<Action>,
    #[serde(default)]
    pub value_kind: CategoryValueKind,
    #[serde(default)]
    pub numeric_format: Option<NumericFormat>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum CategoryValueKind {
    #[default]
    Tag,
    Numeric,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NumericFormat {
    #[serde(default = "default_numeric_decimal_places")]
    pub decimal_places: u8,
    #[serde(default)]
    pub currency_symbol: Option<String>,
    #[serde(default)]
    pub use_thousands_separator: bool,
}
...
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Condition {
    ImplicitString,
    Profile { criteria: Box<Query> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Action {
    Assign { targets: HashSet<CategoryId> },
    Remove { targets: HashSet<CategoryId> },
}
```

Key flags on `Category`:

- **`is_exclusive`** — on a parent, enforces mutual exclusion among its children.
- **`is_actionable`** — items must have at least one actionable category to be marked "done".
- **`enable_implicit_string`** — controls whether the substring classifier auto-matches the category name in item text. Reserved categories (When, Entry, Done) have this disabled so words like "done" in normal text do not trigger assignment.
- **`value_kind`** — `Tag` (default, boolean presence) or `Numeric` (carries a decimal value on its assignment).

The two `Condition` variants:
- `ImplicitString` — matches if the category name appears as a whole word in item text.
- `Profile` — matches if the item's current assignments satisfy a `Query` (AND/NOT/OR criteria).

The two `Action` variants:
- `Assign` — when this category matches, also assign additional target categories.
- `Remove` — when this category matches, remove (unassign) target categories.

### Views, Sections, and Queries

A **View** is a saved lens over the item collection. It has top-level criteria (which items appear at all), an ordered list of **Sections** (sub-groups), and an optional "unmatched" bucket for items that pass the view criteria but do not land in any section.

A **Section** has its own criteria, optional columns (for board-style display), and edit-through sets (`on_insert_assign`, `on_remove_unassign`) that define what category changes happen when a user drags items in/out.

Both views and sections use **Query**, which combines:
- **Criteria**: a list of `(mode, category_id)` pairs where mode is And, Not, or Or.
- **Virtual include/exclude**: temporal WhenBucket filters (Overdue, Today, Tomorrow, etc.).
- **Text search**: optional free-text filter.

```bash
sed -n "132,167p" crates/aglet-core/src/model.rs
```

```output
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct View {
    pub id: Uuid,
    pub name: String,
    pub criteria: Query,
    pub sections: Vec<Section>,
    pub show_unmatched: bool,
    pub unmatched_label: String,
    pub remove_from_view_unassign: HashSet<CategoryId>,
    #[serde(default)]
    pub item_column_label: Option<String>,
    #[serde(default)]
    pub board_display_mode: BoardDisplayMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum BoardDisplayMode {
    #[default]
    SingleLine,
    MultiLine,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Section {
    pub title: String,
    pub criteria: Query,
    #[serde(default)]
    pub columns: Vec<Column>,
    #[serde(default)]
    pub item_column_index: usize,
    pub on_insert_assign: HashSet<CategoryId>,
    pub on_remove_unassign: HashSet<CategoryId>,
    pub show_children: bool,
    #[serde(default)]
    pub board_display_mode_override: Option<BoardDisplayMode>,
}
```

## 3. Error Handling (`aglet-core/src/error.rs`)

The crate defines a single `AgletError` enum with five variants that cover all failure modes. Every public function returns `Result<T, AgletError>`. SQLite errors are wrapped via a `From<rusqlite::Error>` impl.

```bash
sed -n "6,23p" crates/aglet-core/src/error.rs
```

```output
pub enum AgletError {
    /// Referenced entity not found.
    NotFound { entity: &'static str, id: Uuid },

    /// Category name already exists (case-insensitive).
    DuplicateName { name: String },

    /// Attempted to modify or delete a reserved category (When, Entry, Done).
    ReservedName { name: String },

    /// Operation not valid in current state (e.g., assigning to deleted item).
    InvalidOperation { message: String },

    /// SQLite or other storage failure.
    StorageError {
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}
```

## 4. The Storage Layer (`aglet-core/src/store.rs`)

`Store` wraps a `rusqlite::Connection` and owns the SQLite schema. On first open it creates six tables and their indices:

```bash
sed -n "21,109p" crates/aglet-core/src/store.rs
```

```output
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
...
";
```

Key design points in the schema:

- **UUIDs as TEXT** primary keys (generated client-side via `uuid::Uuid::new_v4()`).
- **Category names are UNIQUE COLLATE NOCASE** — no two categories can share the same name regardless of casing.
- **Assignments** use a composite primary key `(item_id, category_id)` and CASCADE deletes. Numeric categories store their value in the `numeric_value` column.
- **Views** store their criteria and sections as JSON blobs (`criteria_json`, `sections_json`).
- **Deletion log** — deleted items are not lost. They are moved to `deletion_log` with a snapshot of their assignments, enabling restore.
- **Item links** — directed dependency (`depends-on`) and bidirectional (`related`) links between items, with CHECK constraints ensuring self-links are forbidden and related pairs are normalized.

On first launch, the store also creates three **reserved categories** (`When`, `Entry`, `Done`) and a default "All Items" view. Reserved categories have `enable_implicit_string = false` and `is_actionable = false` so they do not interfere with normal text matching.

The `Store` exposes CRUD methods for each entity. Category hierarchy is assembled by `get_hierarchy()`, which queries all categories, sorts by `sort_order`, and builds the parent-child tree via a depth-first flattening pass.

## 5. The Text Classifier (`aglet-core/src/matcher.rs`)

The `Classifier` trait is the extension point for text-to-category matching. The MVP implementation, `SubstringClassifier`, does **case-insensitive word-boundary substring matching**. It finds the category name in the item text, but only if surrounded by non-alphanumeric boundaries—preventing "Condone" from matching "Done" or "Sarahville" from matching "Sarah".

```bash
sed -n "1,38p" crates/aglet-core/src/matcher.rs
```

```output
use std::collections::HashSet;

/// Classifier interface for category matching.
///
/// `None` means no match; `Some(confidence)` means match.
pub trait Classifier: Send + Sync {
    fn classify(&self, text: &str, category_name: &str) -> Option<f32>;
}

/// MVP classifier that performs case-insensitive word-boundary substring matches.
#[derive(Debug, Default, Clone, Copy)]
pub struct SubstringClassifier;

impl Classifier for SubstringClassifier {
    fn classify(&self, text: &str, category_name: &str) -> Option<f32> {
        let needle = category_name.trim();
        if needle.is_empty() {
            return None;
        }

        let haystack_lower = text.to_ascii_lowercase();
        let needle_lower = needle.to_ascii_lowercase();

        let mut offset = 0usize;
        while let Some(relative_idx) = haystack_lower[offset..].find(&needle_lower) {
            let start = offset + relative_idx;
            let end = start + needle_lower.len();

            if has_word_boundaries(&haystack_lower, start, end) {
                return Some(1.0);
            }

            offset = start + 1;
        }

        None
    }
}
```

The module also provides `extract_hashtag_tokens()` and `unknown_hashtag_tokens()` — these parse `#hashtag` tokens from item text and compare them against known category names. The CLI uses this to warn users about unknown hashtags when adding items.

## 6. Date Parsing (`aglet-core/src/dates.rs`)

The `DateParser` trait and its `BasicDateParser` implementation extract dates from natural language in item text. It supports:

- **Relative words**: "today", "tomorrow", "yesterday"
- **Relative weekdays**: "this Tuesday", "next Friday"  
- **Month-day formats**: "March 15", "May 25, 2026"
- **ISO dates**: "2026-02-24", "20260224"
- **Numeric M/D/Y**: "2/24/2026"
- **Compound time**: "tomorrow at 3pm", "next Tuesday at noon", "May 25 at 15:00"

The parser is deterministic — no AI, no ambiguity. A `WeekdayDisambiguationPolicy` controls whether "next Tuesday" means the following calendar week (StrictNextWeek, the default) or the next occurrence (InclusiveNext).

```bash
sed -n "60,73p" crates/aglet-core/src/dates.rs
```

```output
impl DateParser for BasicDateParser {
    fn parse(&self, text: &str, reference_date: NaiveDate) -> Option<ParsedDate> {
        let bytes = text.as_bytes();
        let mut best = None;

        scan_relative_dates(bytes, reference_date, self.weekday_policy, &mut best);
        scan_month_name_dates(bytes, reference_date, &mut best);
        scan_iso_dashed_dates(bytes, &mut best);
        scan_iso_compact_dates(bytes, &mut best);
        scan_numeric_mdy_dates(bytes, &mut best);

        best.map(|parsed| attach_trailing_time(bytes, parsed))
    }
}
```

The parser runs all scanners and picks the best match. It then checks for a trailing "at <time>" suffix and merges it into the datetime. The `ParsedDate` struct carries byte-offset spans so callers know which part of the text was consumed.

## 7. The Rule Engine (`aglet-core/src/engine.rs`)

This is the heart of aglet's automation. When an item is created or updated, `process_item()` runs a **fixed-point loop** over the full category hierarchy. Each pass:

1. Evaluates every category against the item (implicit string match + profile conditions).
2. For matches, assigns the category (respecting mutual exclusion on exclusive parents).
3. Fires any **actions** attached to newly matched categories (Assign → add more categories, Remove → defer removals).
4. Walks up the tree to assign **subsumption ancestors** (if "High" is assigned and its parent is "Priority", then "Priority" is also assigned).
5. If any new assignments were made, runs another pass (actions can trigger further matches).

The loop converges when a pass produces no new assignments, or errors if it exceeds 10 passes (indicating a cycle). Remove actions are **deferred** until after the loop finishes to avoid interfering with in-progress matching.

All engine writes happen inside a SQLite **savepoint**. If the engine errors (e.g. cycle cap), the savepoint is rolled back and no partial assignments are left behind.

```bash
sed -n "47,139p" crates/aglet-core/src/engine.rs
```

```output
/// Process one item through fixed-point category evaluation.
///
/// The engine performs repeated hierarchy passes until a pass yields no new
/// assignments, or returns an error if it would require more than MAX_PASSES.
/// Remove actions are deferred during the cascade and applied once at the end.
pub fn process_item(
    store: &Store,
    classifier: &dyn Classifier,
    item_id: ItemId,
) -> Result<ProcessItemResult> {
    run_in_savepoint(store, || process_item_inner(store, classifier, item_id))
}

/// Evaluate all items in the store against the current hierarchy.
///
/// Error strategy for MVP: fail fast. If one item processing run fails,
/// return that error immediately rather than skipping it and continuing.
pub fn evaluate_all_items(
    store: &Store,
    classifier: &dyn Classifier,
    category_id: CategoryId,
) -> Result<EvaluateAllItemsResult> {
    // Validate the target category exists before beginning retroactive work.
    store.get_category(category_id)?;

    let mut result = EvaluateAllItemsResult::default();
    let items = store.list_items()?;

    for item in items {
        let process_result = process_item(store, classifier, item.id)?;

        result.processed_items += 1;
        result.total_new_assignments += process_result.new_assignments.len();
        result.total_deferred_removals += process_result.deferred_removals.len();

        if !process_result.new_assignments.is_empty()
            || !process_result.deferred_removals.is_empty()
        {
            result.affected_items += 1;
        }
    }

    Ok(result)
}

fn process_item_inner(
    store: &Store,
    classifier: &dyn Classifier,
    item_id: ItemId,
) -> Result<ProcessItemResult> {
    let item = store.get_item(item_id)?;
    let categories = store.get_hierarchy()?;

    let mut assignments = item.assignments;
    let mut seen_pairs: HashSet<(ItemId, CategoryId)> = assignments
        .keys()
        .copied()
        .map(|category_id| (item_id, category_id))
        .collect();

    let mut result = ProcessItemResult::default();

    for pass in 1..=MAX_PASSES {
        let pass_result = run_hierarchy_pass(
            store,
            classifier,
            item_id,
            &item.text,
            &categories,
            &mut assignments,
            &mut seen_pairs,
        )?;

        let made_new_assignments = !pass_result.new_assignments.is_empty();

        result.new_assignments.extend(pass_result.new_assignments);
        result
            .deferred_removals
            .extend(pass_result.deferred_removals);

        if !made_new_assignments {
            apply_deferred_removals(store, item_id, &result.deferred_removals)?;
            return Ok(result);
        }

        if pass == MAX_PASSES {
            apply_deferred_removals(store, item_id, &result.deferred_removals)?;
            return Err(pass_cap_error(item_id));
        }
    }

    unreachable!("fixed-point loop should always return from within MAX_PASSES");
}
```

The engine also handles **mutual exclusion** during the cascade. When assigning a category whose parent is exclusive, `enforce_mutual_exclusion()` removes any siblings that are already assigned:

```bash
sed -n "351,382p" crates/aglet-core/src/engine.rs
```

```output
fn enforce_mutual_exclusion(
    store: &Store,
    item_id: ItemId,
    category_id: CategoryId,
    categories_by_id: &HashMap<CategoryId, &Category>,
    assignments: &mut HashMap<CategoryId, Assignment>,
) -> Result<()> {
    let Some(category) = categories_by_id.get(&category_id) else {
        return Ok(());
    };
    let Some(parent_id) = category.parent else {
        return Ok(());
    };
    let Some(parent) = categories_by_id.get(&parent_id) else {
        return Ok(());
    };
    if !parent.is_exclusive {
        return Ok(());
    }

    for sibling_id in &parent.children {
        if *sibling_id == category_id {
            continue;
        }

        if assignments.remove(sibling_id).is_some() {
            store.unassign_item(item_id, *sibling_id)?;
        }
    }

    Ok(())
}
```

## 8. The Integration Layer (`aglet-core/src/aglet.rs`)

`Aglet` is the synchronous API surface that wires together the Store, Classifier, and Engine. Every mutating operation goes through `Aglet` — it is the single entry point that ensures the engine runs after each change. Here is how item creation flows:

```bash
sed -n "16,60p" crates/aglet-core/src/aglet.rs
```

```output
/// Synchronous integration layer that wires Store mutations to engine execution.
pub struct Aglet<'a> {
    store: &'a Store,
    classifier: &'a dyn Classifier,
    date_parser: BasicDateParser,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct LinkItemsResult {
    pub created: bool,
}

impl<'a> Aglet<'a> {
    pub fn new(store: &'a Store, classifier: &'a dyn Classifier) -> Self {
        Self {
            store,
            classifier,
            date_parser: BasicDateParser::default(),
        }
    }

    pub fn store(&self) -> &Store {
        self.store
    }

    pub fn create_item(&self, item: &Item) -> Result<ProcessItemResult> {
        self.create_item_with_reference_date(item, Utc::now().date_naive())
    }

    pub fn create_item_with_reference_date(
        &self,
        item: &Item,
        reference_date: NaiveDate,
    ) -> Result<ProcessItemResult> {
        let mut item_to_create = item.clone();
        let parsed_datetime = self.parse_datetime_from_text(&item_to_create.text, reference_date);
        if let Some(datetime) = parsed_datetime {
            item_to_create.when_date = Some(datetime);
        }

        self.store.create_item(&item_to_create)?;

        if parsed_datetime.is_some() {
            self.assign_when_provenance(item_to_create.id)?;
        }

        process_item(self.store, self.classifier, item_to_create.id)
    }
```

The flow for `create_item` is:

1. Parse the item text for a date expression → set `when_date` if found.
2. Persist the item to SQLite via `store.create_item()`.
3. If a date was parsed, assign the reserved "When" category (provenance tracking).
4. Run the rule engine via `process_item()` → auto-assigns categories, fires actions, builds subsumption chain.

The Aglet layer also handles **manual assignment** with exclusive sibling enforcement, **mark done/not-done** (toggles the "Done" reserved category), **view/section insert/remove** (translates drag-and-drop semantics into category mutations), **category CRUD** (creating a category triggers `evaluate_all_items` to retroactively match existing items), and **item linking** (creating/removing dependency and related links between items).

## 9. Query & View Resolution (`aglet-core/src/query.rs`)

When it is time to display data, `resolve_view()` evaluates a View against the full item set:

1. Applies the view's top-level query to get the pool of visible items.
2. For each section, applies the section's query against the pool.
3. If a section has `show_children = true` and a single And-criterion pointing to a parent category, it auto-expands into subsections — one per child category.
4. Items that pass the view query but no section query go into the "unmatched" bucket.

The query evaluator checks AND criteria (item must have all), NOT criteria (item must lack all), OR criteria (item must have at least one), virtual WhenBucket filters, and text search.

```bash
sed -n "101,162p" crates/aglet-core/src/query.rs
```

```output
/// Resolve a view into ordered section groups and an optional unmatched group.
pub fn resolve_view(
    view: &View,
    items: &[Item],
    categories: &[Category],
    reference_date: NaiveDate,
) -> ViewResult {
    let categories_by_id: HashMap<CategoryId, &Category> = categories
        .iter()
        .map(|category| (category.id, category))
        .collect();
    let view_items: Vec<Item> = evaluate_query(&view.criteria, items, reference_date)
        .into_iter()
        .cloned()
        .collect();

    let mut matched_in_sections = HashSet::new();
    let sections = view
        .sections
        .iter()
        .enumerate()
        .map(|(section_index, section)| {
            let section_items = evaluate_query(&section.criteria, &view_items, reference_date);
            matched_in_sections.extend(section_items.iter().map(|item| item.id));

            if let Some(subsections) =
                expand_show_children_subsections(section, &section_items, &categories_by_id)
            {
                return ViewSectionResult {
                    section_index,
                    title: section.title.clone(),
                    items: Vec::new(),
                    subsections,
                };
            }

            ViewSectionResult {
                section_index,
                title: section.title.clone(),
                items: section_items.into_iter().cloned().collect(),
                subsections: Vec::new(),
            }
        })
        .collect();

    let (unmatched, unmatched_label) = if view.show_unmatched {
        let unmatched_items = view_items
            .iter()
            .filter(|item| !matched_in_sections.contains(&item.id))
            .cloned()
            .collect();
        (Some(unmatched_items), Some(view.unmatched_label.clone()))
    } else {
        (None, None)
    };

    ViewResult {
        sections,
        unmatched,
        unmatched_label,
    }
}
```

The `WhenBucket` system provides temporal grouping. `resolve_when_bucket()` maps a `when_date` to one of: Overdue, Today, Tomorrow, ThisWeek, NextWeek, ThisMonth, Future, or NoDate. Views can include or exclude items based on these buckets without needing actual date-range criteria.

```bash
sed -n "6,56p" crates/aglet-core/src/query.rs
```

```output
/// Resolve a `when_date` into its virtual `WhenBucket` for a given reference date.
pub fn resolve_when_bucket(
    when_date: Option<NaiveDateTime>,
    reference_date: NaiveDate,
) -> WhenBucket {
    let Some(when_datetime) = when_date else {
        return WhenBucket::NoDate;
    };

    let when_day = when_datetime.date();

    if when_day < reference_date {
        return WhenBucket::Overdue;
    }

    if when_day == reference_date {
        return WhenBucket::Today;
    }

    if let Some(tomorrow) = reference_date.succ_opt() {
        if when_day == tomorrow {
            return WhenBucket::Tomorrow;
        }
    }

    let this_week_start = start_of_iso_week(reference_date);
    let this_week_end = this_week_start
        .checked_add_signed(Duration::days(6))
        .expect("valid week range");

    if when_day > reference_date && when_day >= this_week_start && when_day <= this_week_end {
        return WhenBucket::ThisWeek;
    }

    let next_week_start = this_week_start
        .checked_add_signed(Duration::days(7))
        .expect("valid next week start");
    let next_week_end = next_week_start
        .checked_add_signed(Duration::days(6))
        .expect("valid next week range");

    if when_day >= next_week_start && when_day <= next_week_end {
        return WhenBucket::NextWeek;
    }

    if when_day.year() == reference_date.year() && when_day.month() == reference_date.month() {
        return WhenBucket::ThisMonth;
    }

    WhenBucket::Future
}
```

## 10. The CLI (`aglet-cli/src/main.rs`)

The CLI is a single-file binary using Clap's derive API. It parses a `--db` path (or `AGLET_DB` env var, defaulting to `~/.aglet/default.ag`), opens a Store, creates an Aglet, and dispatches commands.

```bash
sed -n "47,123p" crates/aglet-cli/src/main.rs
```

```output
#[derive(Subcommand, Debug)]
enum Command {
    /// Add a new item
    Add {
        text: String,
        #[arg(long)]
        note: Option<String>,
    },

    /// Edit an existing item's text, note, and/or done state
    Edit {
        item_id: String,
        /// New text (positional shorthand; also available as --text)
        text: Option<String>,
        #[arg(long)]
        note: Option<String>,
        #[arg(long = "clear-note")]
        clear_note: bool,
        #[arg(long)]
        done: Option<bool>,
    },

    /// Show a single item with its assignments
    Show { item_id: String },

    /// List items (optionally filtered)
    List {
        #[arg(long)]
        view: Option<String>,
        /// Category filter (repeat for AND). Item must have ALL specified categories.
        #[arg(long)]
        category: Vec<String>,
        /// Sort key(s): item, when, or category name. Repeat for multi-key sorting.
        /// Optional suffix `:asc` or `:desc` (default: asc).
        #[arg(long = "sort", value_name = "COLUMN[:asc|desc]")]
        sort: Vec<String>,
        #[arg(long)]
        include_done: bool,
    },

    /// Search item text and note
    Search {
        query: String,
        #[arg(long)]
        include_done: bool,
    },

    /// Delete an item (writes deletion log)
    Delete { item_id: String },

    /// List deletion log entries
    Deleted,

    /// Restore an item from deletion log by log entry id
    Restore { log_id: String },

    /// Launch the interactive TUI
    Tui,

    /// Category commands
    Category {
        #[command(subcommand)]
        command: CategoryCommand,
    },

    /// View commands
    View {
        #[command(subcommand)]
        command: ViewCommand,
    },

    /// Item-to-item link commands
    Link {
        #[command(subcommand)]
        command: LinkCommand,
    },
}
```

The CLI provides:

- **`add`** — creates an item, runs the engine, reports parsed dates and new assignments, warns about unknown hashtags.
- **`edit`** — updates text/note/done state, re-runs the engine.
- **`show`** — detailed single-item view with assignment provenance.
- **`list`** — shows items through a named view (or the default view), optionally filtered by category. Supports repeatable `--category` for AND filtering and `--sort COLUMN[:asc|desc]` for multi-key sorting.
- **`search`** — free-text search across item text and notes.
- **`delete`** / **`deleted`** / **`restore`** — soft-delete lifecycle.
- **`category`** subcommands — list, show, create, delete, rename, reparent, update, assign, set-value, unassign. Categories can be created with `--type numeric` for numeric value support.
- **`view`** subcommands — list, create, rename, delete, show (with `--sort`).
- **`link`** subcommands — depends-on, blocks, related.
- **`unlink`** subcommands — depends-on, blocks, related.
- **`tui`** — launches the interactive terminal UI.

A key detail: running `aglet` with no subcommand opens the TUI. Use `aglet list` for the scriptable list command. The `tui` command is still available explicitly and delegates to `aglet_tui::run_with_options()` before the normal Store/Aglet setup, since the TUI manages its own lifecycle.

Let us see the CLI in action against the project's own dogfooding database (`aglet-features.ag`):

```bash
cargo run --bin aglet -- --db aglet-features.ag category list 2>/dev/null
```

```output
- Done [no-implicit-string] [non-actionable]
- Entry [no-implicit-string] [non-actionable]
- Expenses
  - Cost [numeric]
  - DRZ [numeric]
- Issue type
  - Bug
  - Idea
  - Feature request
- Priority [exclusive] [no-implicit-string] [non-actionable]
  - Critical [exclusive]
  - High [exclusive]
  - Normal [exclusive]
  - Low [exclusive]
- Software Project
  - Aglet
  - NeoNV
- Status [exclusive]
  - Complete
  - In Progress
  - Next Action
  - Ready
  - Waiting/Blocked
- When [no-implicit-string] [non-actionable]
```

```bash
cargo run --bin aglet -- --db aglet-features.ag view list 2>/dev/null
```

```output
Aglet (sections=2, and=1, not=0, or=0)
All Items (sections=0, and=0, not=0, or=0)
Expenses (sections=1, and=0, not=0, or=0)
Software Projects (sections=2, and=1, not=0, or=0)
hint: use `aglet view show "<name>"` to see view contents
```

## 11. The TUI (`aglet-tui/`)

The TUI is a full ratatui application with a rich modal interface. Its structure:

```bash
find crates/aglet-tui/src -type f -name "*.rs" | sort
```

```output
crates/aglet-tui/src/app.rs
crates/aglet-tui/src/input/mod.rs
crates/aglet-tui/src/input_panel.rs
crates/aglet-tui/src/lib.rs
crates/aglet-tui/src/modes/board.rs
crates/aglet-tui/src/modes/category.rs
crates/aglet-tui/src/modes/mod.rs
crates/aglet-tui/src/modes/view_edit/details.rs
crates/aglet-tui/src/modes/view_edit/editor.rs
crates/aglet-tui/src/modes/view_edit/inline.rs
crates/aglet-tui/src/modes/view_edit/mod.rs
crates/aglet-tui/src/modes/view_edit/overlay.rs
crates/aglet-tui/src/modes/view_edit/picker.rs
crates/aglet-tui/src/modes/view_edit/sections.rs
crates/aglet-tui/src/modes/view_edit/state.rs
crates/aglet-tui/src/render/mod.rs
crates/aglet-tui/src/text_buffer.rs
crates/aglet-tui/src/ui_support.rs
```

The TUI is organized as:

- **`lib.rs`** — the main `App` struct (≈1600 lines), all state, the `Mode` enum, and `pub fn run()`.
- **`app.rs`** — the event loop (`App::run`), `refresh()` to rebuild slots from views, cursor movement, and view cycling.
- **`input/mod.rs`** — key dispatch: routes key events to the right handler based on current `Mode`.
- **`input_panel.rs`** — unified text input widget for add/edit/rename operations.
- **`modes/`** — mode-specific handlers: `board.rs` (board/column operations), `category.rs` (category manager with tree + details panes), `view_edit/` (the view editor feature module with picker/editor/inline/overlay/sections/details/state responsibilities).
- **`render/mod.rs`** — all drawing code: layout, tables, overlays, board grids.
- **`text_buffer.rs`** — a simple text editing buffer with cursor support.
- **`ui_support.rs`** — helper functions shared across modules.

### The Mode System

The TUI uses a modal architecture. The `Mode` enum has 17+ variants:

```bash
sed -n "177,200p" crates/aglet-tui/src/lib.rs
```

```output
enum Mode {
    Normal,
    InputPanel, // unified add/edit/name-input (replaces AddInput + ItemEdit)
    LinkWizard,
    NoteEdit,
    ItemAssignPicker,
    ItemAssignInput,
    InspectUnassign,
    SearchBarFocused,
    ViewPicker,
    ViewEdit,
    ViewDeleteConfirm,
    ConfirmDelete,
    BoardColumnDeleteConfirm,
    CategoryManager,
    CategoryDirectEdit,
    CategoryColumnPicker,
    BoardAddColumnPicker,
    #[allow(dead_code)]
    CategoryCreateConfirm {
        name: String,
        parent_id: CategoryId,
    },
}
```

### The Event Loop

The `App::run()` method in `app.rs` is straightforward: draw, poll for key events (200ms timeout), dispatch via `handle_key_event()`. Errors during key handling are caught and displayed in the status bar rather than crashing:

```bash
sed -n "4,43p" crates/aglet-tui/src/app.rs
```

```output
impl App {
    pub(crate) fn run(
        &mut self,
        terminal: &mut TuiTerminal,
        agenda: &Aglet<'_>,
    ) -> Result<(), String> {
        self.refresh(agenda.store())?;

        loop {
            terminal
                .draw(|frame| self.draw(frame))
                .map_err(|e| e.to_string())?;

            if !event::poll(std::time::Duration::from_millis(200)).map_err(|e| e.to_string())? {
                continue;
            }

            let Event::Key(key) = event::read().map_err(|e| e.to_string())? else {
                continue;
            };
            if key.kind != KeyEventKind::Press {
                continue;
            }

            let should_quit = match self.handle_key_event(key, agenda) {
                Ok(value) => value,
                Err(err) => {
                    self.mode = Mode::Normal;
                    self.clear_input();
                    self.status = format!("Error: {err}");
                    false
                }
            };
            if should_quit {
                break;
            }
        }

        Ok(())
    }
```

### Refresh & Slots

`App::refresh()` reloads all data from the Store and resolves the current view into **slots**. A slot is a displayable section with a title and a list of items. Sections with `show_children = true` expand into multiple slots (one per child category). The unmatched bucket becomes a slot too. Per-section text filters are applied after resolution.

## 12. How It All Connects

Let us trace what happens end-to-end when a user types:

```
aglet --db my.ag add "Call Sarah about Project Atlas tomorrow at 3pm"
```

1. **CLI** parses args, opens `my.ag`, creates `Aglet`.
2. **`cmd_add()`** checks for unknown hashtags, creates an `Item::new()`.
3. **`Aglet::create_item_with_reference_date()`**:
   - Runs `BasicDateParser::parse()` → finds "tomorrow at 3pm" → sets `when_date` to tomorrow 15:00.
   - Calls `store.create_item()` → INSERT into SQLite.
   - Assigns the "When" reserved category (provenance: `nlp:date`).
   - Calls `engine::process_item()`.
4. **Engine fixed-point loop** (pass 1):
   - Iterates all categories. `SubstringClassifier` checks each:
     - "Sarah" → word-boundary match → assigns "Sarah" (if such a category exists).
     - "Project Atlas" → word-boundary match → assigns "Project Atlas".
   - For each match, `assign_subsumption_ancestors()` walks up: if "Sarah" is under "People", then "People" is also assigned.
   - Actions fire: if "Project Atlas" has an `Assign { targets: [Work] }` action, "Work" gets assigned too.
   - Mutual exclusion: if assigning "High" and parent "Priority" is exclusive, any sibling (Medium, Low) is removed.
5. **Pass 2**: re-evaluates with new assignments. Profile conditions may now match. Loop continues until stable.
6. **Deferred removals** are applied (any `Remove` actions).
7. **CLI** prints `created <uuid>`, `parsed_when=...`, `new_assignments=N`.

When the user later runs `aglet list --view "My View"`:

1. `store.list_items()` loads all items with their assignments.
2. `store.list_views()` finds the view by name.
3. `query::resolve_view()` evaluates the view's criteria (AND/NOT/OR on categories, WhenBucket filters, text search), groups into sections, expands show_children subsections, and collects unmatched.
4. `print_items_for_view()` formats and prints the results.

## 13. Test Coverage

The codebase is heavily tested. Each core module has inline `#[cfg(test)]` modules. Let us run the test suite to see:

```bash
cargo test --workspace 2>&1 | tail -20
```

```output
test result: ok. 278 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.37s

     Running unittests src/main.rs (target/debug/deps/aglet_tui-...)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests aglet_core

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests aglet_tui

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

```

278 tests across the workspace, all passing. The test suite covers:

- **`store.rs`** — CRUD for all entities, schema migrations, reserved category creation, deletion log, restore, hierarchy building.
- **`engine.rs`** — fixed-point convergence, cycle detection (cap at 10 passes with savepoint rollback), action cascading, mutual exclusion, deferred removals, idempotent re-runs.
- **`aglet.rs`** — end-to-end integration: item creation with auto-categorization, manual assignment with exclusive enforcement, done/not-done toggling, section insert/remove, view resolution with filters.
- **`matcher.rs`** — word-boundary matching, case insensitivity, hashtag extraction.
- **`dates.rs`** — all date formats, weekday disambiguation policies, compound time parsing, boundary conditions.
- **`query.rs`** — query evaluation with AND/NOT/OR, WhenBucket resolution, view resolution with sections and show_children expansion.
- **`lib.rs` (TUI)** — board operations, category direct edit, column picker.

## Summary

Aglet is a carefully layered system:

| Layer | File(s) | Responsibility |
|-------|---------|---------------|
| **Data model** | `model.rs` | Items, Categories, Assignments, Views, Queries, Item Links |
| **Storage** | `store.rs` | SQLite CRUD, schema, hierarchy, reserved categories, item links |
| **Classifier** | `matcher.rs` | Word-boundary substring matching, hashtag extraction |
| **Date parser** | `dates.rs` | Natural language → NaiveDateTime |
| **Rule engine** | `engine.rs` | Fixed-point auto-assignment, actions, mutual exclusion |
| **Integration** | `aglet.rs` | Wires Store + Engine + Classifier, transaction boundary, linking |
| **Query resolution** | `query.rs` | WhenBuckets, view/section evaluation, show_children |
| **CLI** | `main.rs` (cli) | Clap commands, text output, link management |
| **TUI** | `lib.rs` + modules | Ratatui modal interface, board views, category manager, link wizard |

The architecture makes the right tradeoffs for a personal productivity tool: single-user SQLite for simplicity, deterministic rules instead of ML, and a clean separation between the engine (which is purely functional over the store) and the UI frontends that consume it.
