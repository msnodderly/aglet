# Plan: Numeric Categories and Section Aggregates

## Goal

Add a new category type for numeric values (for costs, counts, miles, etc.) so
aglet can:

- store a numeric value per item for that category
- display numeric columns in views/board sections
- compute section-level aggregates (MVP: `sum`, `avg`)

This extends aglet's category/view model instead of introducing a spreadsheet
subsystem.

## Status Update (2026-02-25)

### Current implementation status

- Phase 1 (`agenda-core` model/schema/migrations): completed
- Phase 2 (`agenda-core` Agenda API + behavior rules): completed (MVP subset)
- Phase 3 (`agenda-cli` numeric category/value commands): completed (MVP subset)
- Phase 4+ (`agenda-tui` rendering/editing): not started in this worktree

### Decisions locked during implementation

- Numeric values are stored on assignments (`item_id` + `category_id`) as an
  optional decimal payload.
- `rust_decimal::Decimal` is used for numeric value storage/logic.
- Numeric categories are leaf-only in MVP:
  - numeric categories cannot have children
  - you cannot create/reparent a child under a numeric category
- Category type transitions:
  - `Tag -> Numeric` allowed only when the category has no children and no
    existing assignments
  - `Numeric -> Tag` is rejected
- CLI surface:
  - `agenda category set-value <item-id> <category> <value>`
  - `agenda category assign` rejects numeric categories and points users to
    `set-value`
- Footer scope remains pending implementation (TUI board only is still the MVP
  target)

### Validation completed

- `cargo fmt` passing
- `cargo test -p agenda-core` passing
- `cargo test -p agenda-cli` passing
- manual CLI smoke test completed for:
  - `category create --type numeric`
  - `category show`
  - `add`
  - `category set-value`
  - numeric-category rejection in `category assign`

## Context (Current aglet Architecture)

Current behavior is category-membership-centric:

- `Assignment` stores provenance only (manual/auto/action/subsumption), but no
  value payload.
- `Category` has flags/rules/actions, but no type metadata (`Tag` vs `Numeric`).
- `Section.columns` are view board columns, but `Standard` columns currently
  render child-category labels under a heading category.
- `resolve_view()` filters and groups items only; it does not compute column
  values or aggregates.

This means numeric support touches:

- `agenda-core` model + storage + migrations
- assignment APIs (`Store` / `Agenda`)
- TUI board rendering and editing
- CLI category/value commands

## Lotus Agenda Review (Reference)

Reviewed:

- external Lotus Agenda documentation on columns and numeric columns (not included in repo)

Relevant Lotus Agenda behaviors (paraphrased):

- Columns are typed: `Standard`, `Numeric`, `Date`, `Unindexed`.
- Numeric columns store numbers per item and support simple calculations.
- Numeric category type becomes effectively immutable once set (no later type
  conversion).
- Numeric columns support per-column formatting (currency symbol, decimals,
  thousands separator, negative style).
- Numeric columns can show a `% of total` companion column.
- Section-level numeric calculations can be shown (count/total/average/min/max).
- Numeric columns are right-aligned.

## Recommended Product Direction for aglet

### MVP (what to build first)

- Category type: `Tag` (existing behavior) + `Numeric`
- Per-item numeric value storage for numeric categories
- Numeric column rendering in TUI board
- Section footer aggregates for numeric columns:
  - `sum`
  - `avg`
- Basic numeric formatting:
  - decimal places
  - optional currency symbol (start with `$` / none)

### Explicitly out of MVP

- formulas / cross-column calculations
- `% of total` companion column
- min/max/count footer options (easy follow-on)
- numeric comparisons in queries (`>`, `<`, ranges)
- automatic parsing of `$12.34` from item text into numeric categories

## Key Design Decision

Use **typed categories + typed assignment payloads**, not a separate spreadsheet
table.

Why:

- preserves aglet's category-centric mental model
- integrates naturally with existing views/sections/columns
- keeps multi-category columns and numeric columns side-by-side
- avoids duplicating item-to-column storage outside assignments

## Data Model Changes

### 1) Category type metadata

Add category type info so the same category can serve as a numeric column head
and value container.

```rust
// crates/agenda-core/src/model.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CategoryValueKind {
    #[default]
    Tag,      // current behavior (categorization by membership)
    Numeric,  // one numeric value per item/category assignment
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NumericFormat {
    #[serde(default)]
    pub decimal_places: u8,            // e.g. 2
    #[serde(default)]
    pub currency_symbol: Option<String>, // e.g. "$"
    #[serde(default)]
    pub use_thousands_separator: bool,
}
```

Then add to `Category`:

```rust
pub struct Category {
    // existing fields...
    #[serde(default)]
    pub value_kind: CategoryValueKind,
    #[serde(default)]
    pub numeric_format: Option<NumericFormat>,
}
```

Notes:

- `Tag` must remain the default for backward compatibility.
- `numeric_format` is only meaningful when `value_kind == Numeric`.

### 2) Assignment payload for numeric values

Extend assignments to carry an optional numeric payload.

```rust
// crates/agenda-core/src/model.rs
use rust_decimal::Decimal;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Assignment {
    pub source: AssignmentSource,
    pub assigned_at: DateTime<Utc>,
    pub sticky: bool,
    pub origin: Option<String>,
    #[serde(default)]
    pub numeric_value: Option<Decimal>,
}
```

Rationale:

- Numeric values belong on the item/category assignment edge.
- Preserves existing one-row-per-(item, category) uniqueness.
- Allows one item to have many numeric values under different numeric categories.

Precision:

- Use `rust_decimal::Decimal` (recommended) for expense tracking accuracy.
- Persist as string in SQLite `TEXT` column to preserve precision.

## Storage and Migration Plan (`agenda-core::store`)

### Schema changes (v5)

Bump schema version from `4` to `5`, then add:

- `categories.value_kind TEXT NOT NULL DEFAULT 'Tag'`
- `categories.numeric_format_json TEXT NOT NULL DEFAULT '{}'`
- `assignments.numeric_value TEXT NULL`

Example migration SQL (idempotent style):

```rust
// crates/agenda-core/src/store.rs::apply_migrations
if !self.column_exists("categories", "value_kind")? {
    self.conn.execute_batch(
        "ALTER TABLE categories ADD COLUMN value_kind TEXT NOT NULL DEFAULT 'Tag';",
    )?;
}
if !self.column_exists("categories", "numeric_format_json")? {
    self.conn.execute_batch(
        "ALTER TABLE categories ADD COLUMN numeric_format_json TEXT NOT NULL DEFAULT '{}';",
    )?;
}
if !self.column_exists("assignments", "numeric_value")? {
    self.conn.execute_batch(
        "ALTER TABLE assignments ADD COLUMN numeric_value TEXT;",
    )?;
}
```

### Row mapping updates

Update:

- `row_to_category()`
- `create_category()`
- `update_category()`
- `load_assignments()`
- `assign_item()`

Snippet:

```rust
let numeric_value_str: Option<String> = row.get(col_idx_numeric_value)?;
let numeric_value = numeric_value_str
    .as_deref()
    .and_then(|s| s.parse::<Decimal>().ok());
```

### Backward compatibility

- Existing DBs load with `Tag` categories and `None` numeric values.
- Existing assignment JSON snapshots in `deletion_log` deserialize due to
  `#[serde(default)]` on the new `numeric_value` field.

## Core API Changes (`agenda-core::agenda` and `store`)

### New assignment methods

Keep existing boolean-style APIs, add typed helpers:

```rust
// Store
pub fn assign_item_with_numeric_value(
    &self,
    item_id: ItemId,
    category_id: CategoryId,
    assignment: &Assignment,
    numeric_value: Decimal,
) -> Result<()>;

pub fn set_assignment_numeric_value(
    &self,
    item_id: ItemId,
    category_id: CategoryId,
    numeric_value: Option<Decimal>,
) -> Result<()>;
```

```rust
// Agenda
pub fn assign_item_numeric_manual(
    &self,
    item_id: ItemId,
    category_id: CategoryId,
    numeric_value: Decimal,
    origin: Option<String>,
) -> Result<ProcessItemResult>;
```

### Behavioral rules

- Assigning a numeric value should still create/replace the assignment.
- Manual exclusive sibling enforcement still applies.
- Subsumption ancestors must be assigned **without** copying numeric values.
- Engine auto/action assignments remain value-less unless explicitly extended in
  a future phase.

Validation:

- Reject numeric value assignment to non-numeric categories.
- Reject non-numeric assignment payloads on numeric categories only if payload is
  required by the calling path (CLI/TUI may allow blank/unset values as a valid
  state initially).

## View / Query Semantics (No Query Changes in MVP)

Do **not** change query evaluation semantics in MVP.

- `AND/NOT/OR` remain category-presence checks only.
- Numeric categories can be used in queries as "has a value/assignment for this
  category".
- No `>`, `<`, range, or aggregate predicates yet.

This avoids destabilizing:

- profile conditions
- rule engine matching
- view resolver logic

## TUI Plan (`agenda-tui`)

## A. Board column eligibility (requested change)

Columns should be creatable from **any non-leaf category**, not just top-level
categories, with `When` remaining special.

Current board add-column validation is stricter than desired. Align it to:

```rust
fn is_valid_board_column_heading_category(category: &Category) -> bool {
    if category.name.eq_ignore_ascii_case("Entry") {
        return false;
    }
    if category.name.eq_ignore_ascii_case("When") {
        return category.parent.is_none();
    }
    !category.children.is_empty() || category.value_kind == CategoryValueKind::Numeric
}
```

Note:

- Once numeric categories exist, numeric leaf categories must also be valid
  column heads, even though standard leaf categories are not.

## B. Column rendering: standard vs numeric

Extend board column layout metadata so render code can inspect the heading
category type (and optional numeric format).

```rust
pub(super) struct BoardColumnSpec {
    pub(super) label: String,
    pub(super) width: usize,
    pub(super) kind: ColumnKind,
    pub(super) child_ids: Vec<CategoryId>,
    pub(super) heading_id: CategoryId,
    pub(super) heading_value_kind: CategoryValueKind,
    pub(super) numeric_format: Option<NumericFormat>,
}
```

Then render cells as:

- `ColumnKind::When` -> existing `when_date` formatting
- `Standard + Tag heading` -> existing child-category label rendering
- `Standard + Numeric heading` -> numeric assignment value rendering (numeric
  column in aglet's current board system)

Cell formatter sketch:

```rust
fn board_column_value(
    item: &Item,
    col: &BoardColumnSpec,
    category_names: &HashMap<CategoryId, String>,
) -> String {
    match col.kind {
        ColumnKind::When => item.when_date.map(|d| d.to_string()).unwrap_or("-".into()),
        ColumnKind::Standard if col.heading_value_kind == CategoryValueKind::Numeric => {
            let value = item.assignments
                .get(&col.heading_id)
                .and_then(|a| a.numeric_value);
            format_numeric_cell(value, col.numeric_format.as_ref())
        }
        ColumnKind::Standard => standard_column_value(item, &col.child_ids, category_names),
    }
}
```

## C. Section footer aggregates (MVP: Sum, Avg)

Compute aggregates from `slot.items` in `render_board_columns()` so totals respect:

- resolved view sections
- generated child subsections (`show_children`)
- per-slot text filtering

Aggregate helper:

```rust
#[derive(Default, Clone)]
struct NumericAggregate {
    count: usize,
    sum: Decimal,
}

impl NumericAggregate {
    fn push(&mut self, v: Decimal) {
        self.count += 1;
        self.sum += v;
    }

    fn avg(&self) -> Option<Decimal> {
        (self.count > 0).then(|| self.sum / Decimal::from(self.count as u32))
    }
}
```

Footer rendering approach:

- Add 2 rows after data rows (for dynamic-column board tables):
  - `SUM`
  - `AVG`
- Only populate cells for numeric columns; leave standard/date columns blank.
- Right-align numeric footer cells.

## D. Numeric cell editing UX

Current board column edit flows assume category-selection under a parent.

Add a numeric value editor path:

- If selected column heading category is numeric:
  - open inline numeric input (reusing existing `InputPanel` if possible)
  - parse decimal
  - call `Agenda::assign_item_numeric_manual(...)`
- If parse fails:
  - show non-destructive validation error

Example parser:

```rust
fn parse_numeric_input(raw: &str) -> Result<Decimal, String> {
    let trimmed = raw.trim().replace(',', "");
    trimmed.parse::<Decimal>()
        .map_err(|_| format!("invalid number: {raw}"))
}
```

## E. View Editor column picker alignment

The board add-column picker and view editor section-column picker should apply
the same heading eligibility rules.

Create a shared helper in `agenda-tui` (e.g. `ui_support.rs`) to prevent drift:

```rust
fn is_valid_section_column_heading(category: &Category) -> bool { /* shared */ }
```

## CLI Plan (`agenda-cli`)

### Category type management

Extend category create/update/show:

- `agenda category create <name> --type tag|numeric`
- `agenda category update <name> --type ...` (or `--numeric`)
- show category type in `category show` and `category list` (at least in details)

MVP restriction (recommended):

- Allow `Tag -> Numeric` only if category has no children and no existing
  assignments (or make type immutable after creation to mirror Lotus behavior).

### Assigning numeric values

Options:

1. Extend `category assign`
- `agenda category assign <item-id> <category> --number 245.96`

2. Add explicit command (clearer)
- `agenda category set-value <item-id> <category> 245.96`

Recommendation: option 2 for clarity and less overload.

## Formatting Plan (MVP vs Future)

### MVP formatting

Store minimal numeric formatting on category:

- `decimal_places`
- `currency_symbol` (optional)
- `use_thousands_separator`

### Future formatting (Lotus-inspired)

- negative style (`-123` vs `(123)`)
- `% of total` column
- per-column overrides vs category defaults
- locale-aware symbols/separators

## Testing Plan

### `agenda-core` tests

- migration: existing DB opens and gets new columns
- create/load numeric category roundtrip
- assign/load numeric value roundtrip
- delete/restore item preserves numeric values in assignment snapshots
- manual numeric assignment respects exclusivity and subsumption
- subsumption ancestors do not copy numeric payload

### `agenda-tui` tests

- board add-column allows nested non-leaf headings
- numeric leaf heading allowed once category type is `Numeric`
- numeric cells render formatted values
- footer `sum` and `avg` render for sections/subsections
- filtered slot recomputes aggregates correctly
- invalid numeric input does not corrupt assignment

### `agenda-cli` tests

- category create/update/show type fields
- set numeric value command validates type + value format

## Implementation Phases

### Phase 1: Core model + storage (Completed)

- add `CategoryValueKind`, `NumericFormat`, `Assignment.numeric_value`
- migrations + row mapping + persistence
- add core tests

### Phase 2: CLI support (Completed)

- category type create/show/update
- numeric value assignment command

### Phase 3: TUI rendering (Not started)

- typed column rendering
- numeric formatting
- section footer `sum`/`avg`
- right alignment for numeric columns

### Phase 4: TUI editing

- numeric cell input/editor
- validation and status messages
- unify heading eligibility logic across board/view editor

### Phase 5: Hardening

- more tests
- docs/walkthrough updates
- sample/demo data scenarios (expenses, counts)

## Detailed TODO Checklist

This is the execution checklist for implementing the plan. It is intentionally
granular so work can be done in small, reviewable patches. Do not implement all
of this in one change.

### Phase 0: Design Decisions and Scope Lock

- [x] Decide category type mutability policy for MVP:
- [x] Chosen: controlled `Tag -> Numeric` migration with validation
- [ ] Option A: immutable after creation (Lotus-like)
- [x] Option B: allow controlled `Tag -> Numeric` migration with validation
- [x] Decide CLI surface for numeric assignment:
- [x] Option A: `agenda category set-value`
- [ ] Option B: `agenda category assign --number`
- [x] Decide whether numeric categories may be leaf headings in board/view columns (recommended: yes)
- [x] Decide MVP footer scope:
- [x] TUI board only
- [ ] TUI board + CLI `view show`
- [x] Confirm Decimal dependency choice (`rust_decimal`) and serialization strategy (`TEXT`)
- [x] Document final MVP decisions in this plan before coding starts

### Phase 1: Core Model and Schema (agenda-core)

#### 1.1 Dependencies and compilation prep

- [x] Add `rust_decimal` dependency to `/Users/mds/src/aglet-numeric-categories-plan/crates/agenda-core/Cargo.toml`
- [x] Add serde support feature for `rust_decimal` if needed
- [x] Ensure downstream crates compile against updated `agenda-core` public types (later phases may still fail until adapted)

#### 1.2 Model types in `model.rs`

- [x] Add `CategoryValueKind` enum with `Tag` default and `Numeric`
- [x] Add `NumericFormat` struct with MVP fields (`decimal_places`, `currency_symbol`, `use_thousands_separator`)
- [x] Add `value_kind` field to `Category` with `#[serde(default)]`
- [x] Add `numeric_format` field to `Category` with `#[serde(default)]`
- [x] Add `numeric_value: Option<Decimal>` to `Assignment` with `#[serde(default)]`
- [x] Update `Category::new(...)` defaults to initialize new fields sensibly
- [x] Review `Item::new(...)` / `View::new(...)` for any compile fallout (should be none)
- [x] Update any derives/imports required by new types

#### 1.3 Schema updates in `store.rs`

- [x] Bump `SCHEMA_VERSION` from `4` to `5`
- [x] Update `SCHEMA_SQL` `categories` table definition to include:
- [x] `value_kind`
- [x] `numeric_format_json`
- [x] Update `SCHEMA_SQL` `assignments` table definition to include `numeric_value`
- [x] Verify default values keep new DB bootstraps backward-compatible
- [x] Verify reserved-category bootstrap inserts populate `value_kind='Tag'` and default format JSON

#### 1.4 Migrations in `apply_migrations(...)`

- [x] Add idempotent `ALTER TABLE` for `categories.value_kind`
- [x] Add idempotent `ALTER TABLE` for `categories.numeric_format_json`
- [x] Add idempotent `ALTER TABLE` for `assignments.numeric_value`
- [x] Ensure migration order is safe for DBs from versions 0-4
- [x] Keep existing migrations unchanged and append new logic
- [x] Verify migration does not overwrite existing category/assignment data

#### 1.5 Category persistence read/write paths

- [x] Update `create_category(...)` to serialize `numeric_format_json`
- [x] Update `update_category(...)` to serialize `numeric_format_json`
- [x] Update `get_category(...)` select list for new columns
- [x] Update `get_hierarchy(...)` select list for new columns
- [x] Update `row_to_category(...)` to parse `value_kind` and `numeric_format_json`
- [x] Decide fallback behavior for invalid DB values:
- [x] invalid `value_kind` -> default `Tag`
- [x] invalid `numeric_format_json` -> default/`None`

#### 1.6 Assignment persistence read/write paths

- [x] Update `assign_item(...)` SQL to write `numeric_value`
- [x] Update `load_assignments(...)` select list to read `numeric_value`
- [x] Parse numeric string to `Decimal` with graceful fallback (`None` on parse error)
- [x] Update `get_assignments_for_item(...)` indirectly via `load_assignments(...)`
- [x] Review `delete_item(...)` snapshot serialization (`assignments_json`) for compatibility
- [x] Review `restore_deleted_item(...)` assignment replay path for numeric payload preservation

#### 1.7 New store helper APIs (if chosen)

- [ ] Add `set_assignment_numeric_value(...)` helper OR equivalent upsert path
- [x] Add typed assignment helper(s) only if they reduce duplication
- [x] Keep existing `assign_item(...)` behavior stable for non-numeric callers

#### 1.8 Core model/storage tests (`agenda-core`)

- [x] Add unit test: numeric category create/get roundtrip
- [ ] Add unit test: numeric category appears correctly in `get_hierarchy()`
- [x] Add unit test: assignment numeric value persists + reloads
- [ ] Add unit test: `deletion_log` snapshot restore preserves numeric assignment payload
- [ ] Add migration test: DB created at old schema upgrades to v5 and loads correctly
- [ ] Add serde test: old JSON `Assignment` deserializes with `numeric_value=None`

### Phase 2: Core Agenda APIs and Behavior Rules (agenda-core)

#### 2.1 Agenda API additions

- [x] Add `Agenda::assign_item_numeric_manual(...)` (or equivalent)
- [ ] Add optional `Agenda::set_item_numeric_value(...)` if command/TUI UX needs edit semantics distinct from assign
- [ ] Decide and implement “clear numeric value” semantics:
- [ ] clear payload but keep assignment
- [ ] or unassign category entirely when value is cleared

#### 2.2 Validation rules

- [x] Validate target category exists and is `Numeric`
- [x] Reject numeric assignment to non-numeric categories with actionable error
- [x] Validate parsing/normalization happens before mutation (CLI/TUI layer may parse first)
- [x] Decide whether `Numeric` categories may exist with `children` in MVP (recommended: no for simplicity; if allowed, document semantics)

#### 2.3 Exclusivity and subsumption behavior

- [x] Reuse manual exclusive sibling enforcement path
- [x] Ensure numeric assignment still triggers subsumption ancestors
- [x] Ensure ancestor subsumption assignments do not inherit `numeric_value`
- [ ] Add tests covering exclusive parent numeric categories if supported

#### 2.4 Engine compatibility review

- [x] Audit `engine.rs` assignment creation sites for compile updates (`numeric_value: None`)
- [x] Audit `agenda.rs` assignment creation sites for compile updates (`numeric_value: None`)
- [x] Audit `query.rs` tests/helpers for assignment struct initialization changes
- [x] Audit `store.rs` tests/helpers for assignment struct initialization changes
- [x] Audit `agenda-tui/src/lib.rs` tests/helpers for assignment struct initialization changes (compile-only at this phase if needed)

#### 2.5 Agenda behavior tests

- [x] Add test: numeric manual assign creates assignment + payload
- [ ] Add test: numeric assign updates existing assignment payload (if supported)
- [ ] Add test: numeric assign under exclusive parent removes sibling assignment as expected
- [x] Add test: subsumption ancestor payload remains `None`
- [x] Add test: reject numeric value on non-numeric category

### Phase 3: CLI Surface for Numeric Categories (agenda-cli)

#### 3.1 CLI command design finalization

- [x] Finalize command syntax (`set-value` vs `assign --number`)
- [x] Update CLI help text and examples in code comments/docs
- [x] Preserve existing `category assign` behavior for non-numeric categories

#### 3.2 Category create/update/show/list enhancements

- [x] Add `--type tag|numeric` to `category create`
- [x] Add type update option to `category update` (if mutability allowed)
- [ ] Add numeric format options (MVP subset), likely:
- [ ] `--decimals <n>`
- [ ] `--currency <symbol|none>`
- [ ] `--thousands-sep <bool>` or simpler toggles
- [x] Show category type in `category show`
- [x] Consider surfacing type marker in `category list` output (optional MVP)
- [x] Add validation errors for illegal type transitions

#### 3.3 Numeric value assignment command implementation

- [x] Add command variant and parser wiring in `Command` / `CategoryCommand`
- [x] Parse item id and category name as usual
- [x] Parse decimal input (strip commas, optional leading `$` if desired)
- [x] Call new `Agenda` numeric assignment API
- [x] Print clear success message including normalized stored value
- [ ] Add clear-value option if supported (e.g. `--clear`)

#### 3.4 CLI tests

- [x] Unit tests for parsing/validation helper(s)
- [ ] End-to-end style tests (if present pattern exists) for:
- [ ] create numeric category
- [ ] set numeric value
- [ ] reject invalid decimal
- [ ] reject non-numeric target category

### Phase 4: TUI Category and Column Metadata Support (agenda-tui foundations)

#### 4.1 Shared heading eligibility helper

- [ ] Create shared helper for section column heading validity in `ui_support.rs` (or equivalent)
- [ ] Apply it to board add-column picker
- [ ] Apply it to view editor section-column picker
- [ ] Ensure behavior is consistent for:
- [ ] standard non-leaf categories
- [ ] `When`
- [ ] numeric leaf categories (once type exists)
- [ ] `Entry` exclusion

#### 4.2 Category manager display for type/format (read-only first)

- [ ] Add category type display in Category Manager details pane
- [ ] Add category type display in `category show` parity notes if needed
- [ ] Ensure reserved categories still behave as read-only where appropriate
- [ ] Decide whether TUI category manager can edit numeric type in MVP or defer to CLI

#### 4.3 Compile/test fallout from `Assignment` and `Category` struct changes

- [ ] Update all TUI tests constructing `Assignment` to include `numeric_value: None`
- [ ] Update all TUI tests constructing `Category` (if explicit literals are used) for new fields/defaults
- [ ] Run focused TUI test subsets to catch type-level regressions early

### Phase 5: TUI Board Rendering (numeric columns + footer aggregates)

#### 5.1 Board layout metadata changes

- [ ] Extend `BoardColumnSpec` with heading category id and category value kind
- [ ] Include numeric format metadata in `BoardColumnSpec` (or resolve from category map at render time)
- [ ] Update `compute_board_layout(...)` to populate new metadata
- [ ] Ensure existing `When` and standard-tag columns still work unchanged

#### 5.2 Numeric cell rendering

- [ ] Add numeric cell formatting helper (`format_numeric_cell`)
- [ ] Handle empty/unset numeric values with a clear placeholder (`-` or `–`)
- [ ] Respect decimal places and currency symbol
- [ ] Respect thousands separator option
- [ ] Ensure truncation/wrapping behavior is sane for numeric cells in single-line mode
- [ ] Decide multi-line mode behavior for numeric cells (likely single-line only)

#### 5.3 Right alignment behavior

- [ ] Determine ratatui table cell/right-align implementation strategy for numeric cells
- [ ] Apply right alignment to numeric column entries
- [ ] Apply right alignment to numeric aggregate footer cells
- [ ] Confirm non-numeric columns keep existing alignment

#### 5.4 Aggregate computation

- [ ] Add helper to collect numeric values for a given column from `slot.items`
- [ ] Compute `sum` and `avg` using `Decimal`
- [ ] Exclude empty/unset values from `avg` denominator (document this)
- [ ] Decide whether to include zero values (recommended: yes)
- [ ] Keep aggregate scope limited to current slot (section/subsection after filters)

#### 5.5 Footer row rendering

- [ ] Add `SUM` footer row for sections with at least one numeric column
- [ ] Add `AVG` footer row for sections with at least one numeric column
- [ ] Leave non-numeric columns blank in footer rows
- [ ] Ensure footer rows render in both dynamic and fallback board table modes (or document fallback limitation)
- [ ] Verify selection/highlight logic ignores footer rows (no accidental selection indexing bugs)
- [ ] Verify scroll behavior remains tied to item rows only

#### 5.6 TUI rendering tests

- [ ] Add render test: numeric column shows formatted value
- [ ] Add render test: mixed standard + numeric columns render correctly
- [ ] Add render test: footer sum/avg values for populated section
- [ ] Add render test: footer with empty numeric values shows blanks/placeholders safely
- [ ] Add render test: aggregates respect per-slot text filter
- [ ] Add render test: generated subsections (`show_children`) compute independent aggregates

### Phase 6: TUI Numeric Editing UX

#### 6.1 UX path selection

- [ ] Decide entry point for numeric editing in board mode:
- [ ] existing edit key on selected numeric column cell
- [ ] dedicated keybinding
- [ ] Decide whether editing opens `InputPanel` vs a lightweight inline editor state
- [ ] Define clear/cancel semantics for numeric values

#### 6.2 Board mode edit dispatch

- [ ] Detect selected column heading category type in `modes/board.rs`
- [ ] Branch to numeric editor path for numeric columns
- [ ] Preserve existing category direct-edit and column-picker flows for standard columns
- [ ] Keep `When` column behavior unchanged (still not implemented inline unless explicitly added)

#### 6.3 Numeric parser and validation

- [ ] Add parsing helper for decimal text input
- [ ] Decide whether to accept:
- [ ] commas (`1,234.56`)
- [ ] currency symbols (`$123.45`)
- [ ] parentheses negatives (`(123.45)`) (probably future)
- [ ] Normalize and display parse errors without mutating data

#### 6.4 Mutation wiring

- [ ] Call `Agenda::assign_item_numeric_manual(...)` (or edit-specific API)
- [ ] Refresh app state after save
- [ ] Preserve cursor slot/item/column position after refresh
- [ ] Update status messages on success/failure

#### 6.5 TUI edit tests

- [ ] Add test: edit numeric cell saves value and persists through refresh
- [ ] Add test: invalid input shows error and preserves previous value
- [ ] Add test: clearing value follows chosen semantics
- [ ] Add test: editing numeric column does not invoke category picker flow

### Phase 7: View Editor and Column Workflow Consistency

#### 7.1 View editor section-column picker filtering

- [ ] Restrict section-column picker candidate list using shared eligibility helper
- [ ] Ensure leaf standard categories cannot be selected as headings
- [ ] Ensure numeric leaf categories can be selected once typed categories are available
- [ ] Update picker status/help text to mention category-type behavior if needed

#### 7.2 View editor tests

- [ ] Add test: section-column picker excludes invalid leaf headings
- [ ] Add test: section-column picker includes nested non-leaf headings
- [ ] Add test: section-column picker includes numeric leaf headings (after type support)

### Phase 8: Documentation and Developer Guidance

#### 8.1 Project docs

- [ ] Update `/Users/mds/src/aglet-numeric-categories-plan/walkthrough.md` with typed categories and assignment payloads
- [ ] Document numeric column/aggregate behavior in user-facing docs (CLI/TUI usage)
- [ ] Add at least one example workflow (expenses and counts)

#### 8.2 AGENTS/implementation notes

- [ ] Add AGENTS note if new surprises emerge (e.g. footer row indexing vs selection/scroll assumptions)
- [ ] Document any semantic decisions that differ from Lotus Agenda (immutability, formatting scope, aggregate options)

### Phase 9: Integration Validation and Polish

#### 9.1 End-to-end manual scenarios

- [ ] Expense tracking scenario:
- [ ] create numeric `Cost` category
- [ ] assign values to multiple items
- [ ] add `Cost` column to board section
- [ ] verify sum/avg footer
- [ ] Count tracking scenario:
- [ ] create numeric `Qty` category
- [ ] assign integer values
- [ ] verify average handles integer inputs correctly
- [ ] Mixed section scenario:
- [ ] standard categories + numeric column + `When` column together

#### 9.2 Regression checks

- [ ] Existing standard category assignment workflows unchanged
- [ ] Existing view resolution semantics unchanged
- [ ] Existing board add-column standard behavior unchanged except heading eligibility broadening
- [ ] Existing reserved category protections unchanged

#### 9.3 Quality gates

- [x] `cargo test -p agenda-core`
- [x] `cargo test -p agenda-cli`
- [ ] `cargo test -p agenda-tui`
- [ ] Run targeted TUI board/view editor test subsets if full suite is slow
- [x] Run formatting/lint commands used by the project (if applicable)

### Optional Phase 10 (Post-MVP, Lotus-inspired)

- [ ] Add footer `count`, `min`, `max`
- [ ] Add `% of total` companion display column
- [ ] Add per-column (view-level) formatting overrides distinct from category defaults
- [ ] Add negative formatting styles (`-123` vs `(123)`)
- [ ] Add numeric query predicates and profile-condition numeric comparisons
- [ ] Add date/unindexed typed categories using the same category-type architecture

## Open Questions (Decide Before Coding)

1. Type mutability (Resolved for current implementation)
- Implemented: controlled `Tag -> Numeric` migration with validation (no
  children and no existing assignments); `Numeric -> Tag` is rejected.

2. Numeric leaf headings (Resolved for planned TUI implementation)
- Yes: numeric leaf categories should be valid column headings once TUI typed
  columns are implemented. (Core/CLI groundwork is done; TUI support pending.)

3. CLI surface (Resolved)
- Implemented dedicated `agenda category set-value` command. `category assign`
  now rejects numeric categories and points to `set-value`.

4. Footer scope (Pending implementation)
- MVP target remains TUI board only. CLI `view show` aggregates are deferred.

## Suggested First Patch (High Confidence)

Completed in this session:

- model + migration + storage roundtrip
- `agenda category create ... --type numeric`
- `agenda category set-value ...`

This provides an end-to-end data path before touching TUI rendering/editor
complexity. Next recommended work starts at Phase 4/5 (TUI rendering +
aggregates), then Phase 6 (TUI numeric editing).

## Session Handoff (2026-02-25)

This worktree contains completed MVP groundwork for numeric categories in
`agenda-core` and `agenda-cli` (Phases 1-3). TUI support is not started yet in
this branch/worktree.

Recommended restart point for the next session:

1. Phase 4.1 shared heading eligibility helper (reuse for board + view editor)
2. Phase 5 numeric board rendering and footer aggregates (`sum`, `avg`)
3. Phase 6 numeric board cell editing (`set-value` flow)
