---
title: Configurable View Columns
status: shipped
created: 2026-02-18
shipped: 2026-03-21
---

# Configurable View Columns

## Context

The TUI board currently hardcodes three columns: `When | Item | All Categories`. The "All Categories" column dumps every assigned category name, which isn't useful. We need Lotus Agenda-style **Standard columns** where the user picks a parent category as the column header and sees which *children* of that parent each item is assigned to. The Item column header should be configurable (or omitted, since the section border already names the section).

## Files to Modify

- `crates/agenda-core/src/model.rs` — data model changes
- `crates/agenda-core/src/store.rs` — schema migration, persistence
- `crates/agenda-tui/src/lib.rs` — rendering pipeline + view manager columns tab

---

## Step 1: Data Model (`model.rs`)

### 1a. Add `ColumnKind` enum

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColumnKind {
    When,      // Virtual — renders item.when_date
    Standard,  // Shows children of heading category assigned to item
}

impl Default for ColumnKind {
    fn default() -> Self { ColumnKind::Standard }
}
```

### 1b. Update `Column` struct

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Column {
    #[serde(default)]
    pub kind: ColumnKind,
    pub heading: CategoryId,
    pub width: u16,
}
```

`#[serde(default)]` on `kind` provides backward compat — old JSON without `kind` deserializes as `Standard`. The v2→v3 migration (Step 2) fixes existing When columns.

### 1c. Add `item_column_label` to `View`

```rust
pub struct View {
    // ...existing fields...
    #[serde(default)]
    pub item_column_label: Option<String>,
}
```

Update `View::new()` to include `item_column_label: None`.

**Note**: Since `item_column_label` needs its own SQL column (it's on View, not in columns_json), we add it via migration.

---

## Step 2: Persistence & Migration (`store.rs`)

### 2a. Bump schema version

`SCHEMA_VERSION: i32 = 3`

### 2b. Update `SCHEMA_SQL`

Add to the views CREATE TABLE:
```sql
item_column_label TEXT
```

### 2c. Add v3 migration in `apply_migrations`

```rust
if from_version < 3 {
    // 1. Add item_column_label column
    if !self.column_exists("views", "item_column_label")? {
        self.conn.execute_batch(
            "ALTER TABLE views ADD COLUMN item_column_label TEXT;"
        )?;
    }

    // 2. Fix existing columns_json: add kind field
    // Find When category ID, then iterate all views and inject
    // kind:"When" for columns whose heading matches When cat ID,
    // kind:"Standard" for all others. Write back.
}
```

### 2d. Update SQL queries

- `create_view`: Add `item_column_label` to INSERT (9th param)
- `update_view`: Add `item_column_label` to UPDATE SET clause
- `get_view`, `list_views`: Add `item_column_label` to SELECT
- `row_to_view`: Read `item_column_label` from row index 8

### 2e. Update `ensure_default_view`

```rust
view.columns.push(Column {
    kind: ColumnKind::When,
    heading: when_category_id,
    width: 16,
});
```

---

## Step 3: TUI Board Rendering (`lib.rs`)

This is the core change. Replace the hardcoded 3-column renderer with a dynamic column system.

### 3a. New types

```rust
#[derive(Clone, Debug)]
struct BoardColumnLayout {
    marker: usize,
    item: usize,                          // flex width
    item_label: String,                   // from view.item_column_label or section title
    columns: Vec<BoardColumnSpec>,        // configured columns in order
}

#[derive(Clone, Debug)]
struct BoardColumnSpec {
    label: String,                        // display name (category name or "When")
    width: usize,
    kind: ColumnKind,
    heading_id: CategoryId,
    child_ids: Vec<CategoryId>,           // pre-computed for Standard columns
}
```

### 3b. New function: `compute_board_layout`

Replaces `board_column_widths`. Takes `view.columns`, categories, slot_width.

Algorithm:
1. Marker = 2 (constant)
2. Separators = `" | ".len() * view.columns.len()` (between item col and each configured col)
3. Sum configured widths (each column.width, min 8)
4. Item col = remaining space (min 12)
5. If item col < min, proportionally shrink configured columns
6. For each Standard column: lookup `category.children` to pre-fill `child_ids`

### 3c. Backward compatibility

When `view.columns` is empty: fall back to current hardcoded rendering. Rename existing functions with `legacy_` prefix and keep them as-is. This way existing views with no columns configured keep working identically.

### 3d. New rendering functions

- `board_dynamic_header(layout) -> String` — renders column headers, item column uses `layout.item_label` (empty string = no header, just spaces)
- `board_dynamic_row(is_selected, item, layout, category_names) -> String` — renders one item row
- `standard_column_value(item, child_ids, category_names) -> String` — for Standard columns: filters item's assignments to only children of heading, returns comma-separated names sorted alpha, or `"–"` if none

### 3e. Update `render_board_columns`

In the main rendering loop, branch:
```rust
let view_columns = current_view.map(|v| &v.columns[..]).unwrap_or(&[]);
if view_columns.is_empty() {
    // Legacy path (existing code, renamed)
} else {
    // Dynamic path using compute_board_layout + board_dynamic_header/row
}
```

The item column label logic:
- If `view.item_column_label` is `Some(label)`: use that
- Else: use empty string (section border already shows the name)

---

## Step 4: View Manager — Columns Tab (`lib.rs`)

### 4a. New state

```rust
view_manager_definition_sub_tab: DefinitionSubTab, // Criteria or Columns
view_manager_column_index: usize,                  // cursor in columns list
view_manager_column_picker_target: bool,            // flag: picker is for column, not criteria
```

```rust
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum DefinitionSubTab { Criteria, Columns }
```

### 4b. Toggle with `t` key

In `handle_view_manager_key`, when pane is Definition:
- `t` toggles `view_manager_definition_sub_tab` between Criteria and Columns
- `j`/`k` navigate the active sub-tab's list

### 4c. Columns sub-tab key bindings

- `N` — add column: opens category picker with `view_manager_column_picker_target = true`
- `x` — delete selected column, set dirty
- `[`/`]` — reorder selected column up/down, set dirty
- `w` — enter width input mode (reuse existing input pattern)
- `Enter` — change heading: opens category picker for selected column

On category picker return (when `column_picker_target`):
- If adding: push new `Column { kind, heading, width: 20 }` — auto-detect kind (When cat → `ColumnKind::When`, else `Standard`)
- If editing: replace heading on selected column

### 4d. Columns sub-tab rendering

In `render_view_manager_screen`, when Definition pane renders and sub-tab is Columns:

```
┌─ Definition ────────────────────────────────────┐
│  [Criteria]  [Columns]          t:toggle        │
│                                                 │
│  View: Smoke Board                              │
│  Columns: 3                     *unsaved*       │
│                                                 │
│  > When                         w: 16           │
│    People                       w: 20           │
│    Project                      w: 20           │
│                                                 │
│  N:add  x:del  [/]:move  w:width  Enter:heading │
└─────────────────────────────────────────────────┘
```

### 4e. Save integration

The `s` key save handler already persists the full View including `columns`. Column edits should modify `self.views[self.picker_index].columns` directly and set `view_manager_dirty = true`. On save, the existing `store.update_view()` call serializes everything.

---

## Step 5: Edge Cases

- **Empty columns**: Legacy fallback (When | Item | All Categories)
- **Deleted heading category**: Show "(deleted)" label, empty child_ids, column renders "–" for all items
- **No matching children**: Standard column renders "–"
- **Multiple values**: "Mike, Dave" comma-separated, truncated to column width
- **Very narrow terminal**: Item column shrinks to min 12; if still not enough, configured columns shrink proportionally

---

## Verification

1. **Build**: `cargo build` — all three crates compile
2. **Tests**: `cargo test` — existing tests pass, add new unit tests:
   - `standard_column_value` with assigned/unassigned/missing children
   - `compute_board_layout` with various column configs and widths
   - `board_dynamic_header` + `board_dynamic_row` alignment (pipe positions match)
   - Serialization roundtrip: Column with kind field
   - Migration: v2 DB opens correctly, When column gets `kind: When`
3. **Manual TUI testing**:
   - Open existing database — views with empty columns render as before
   - Open view manager → Definition pane → press `t` to switch to Columns tab
   - Add a Standard column (e.g., People category with children), save
   - Switch to board view — new column appears with child category values
   - Add a When column, verify dates appear
   - Reorder columns, change widths, verify rendering updates
   - Delete all columns — falls back to legacy rendering
