---
title: Code Quality Improvement
status: shipped
created: 2026-02-10
shipped: 2026-03-21
---

# Code Quality Improvement Plan

Status of implemented quick wins is in git log (commit `8ceb7a2`).
This document covers the remaining items, ordered by priority.

---

## Item A — Export and use `RESERVED_CATEGORY_NAMES` everywhere

**Priority:** Medium
**Effort:** Small (30–60 min)
**Risk:** Low

### Problem

`RESERVED_CATEGORY_NAMES` is defined in `store.rs:19` but is `const` (private). The strings `"When"`, `"Entry"`, and `"Done"` appear as bare literals in at least 5 other places:

| File | Line | Literal |
|---|---|---|
| `agenda.rs` | 616 | `"When"` (via `category_id_by_name`) |
| `agenda.rs` | 629 | `"Done"` (via `category_id_by_name`) |
| `agenda.rs` | 734 | `.eq_ignore_ascii_case("When")` |
| `agenda.rs` | 1919 | `.eq_ignore_ascii_case("Done")` |
| `agenda.rs` | 1979 | `.eq_ignore_ascii_case("Done")` |
| `store.rs` | 1705 | `.eq_ignore_ascii_case("When")` |
| `store.rs` | 1860 | `get_category_id_by_name("When")` |

### Plan

1. Move the constant to `model.rs` or a new `constants.rs` file (both crates can see it):

   ```rust
   // crates/aglet-core/src/model.rs (or constants.rs)
   pub const RESERVED_CATEGORY_NAME_WHEN: &str = "When";
   pub const RESERVED_CATEGORY_NAME_ENTRY: &str = "Entry";
   pub const RESERVED_CATEGORY_NAME_DONE: &str = "Done";
   pub const RESERVED_CATEGORY_NAMES: [&str; 3] = [
       RESERVED_CATEGORY_NAME_WHEN,
       RESERVED_CATEGORY_NAME_ENTRY,
       RESERVED_CATEGORY_NAME_DONE,
   ];
   ```

2. Remove the existing `const RESERVED_CATEGORY_NAMES` from `store.rs`.

3. Replace every bare `"When"` / `"Done"` / `"Entry"` occurrence in `agenda.rs` and `store.rs` with the named constant. Use search-and-replace carefully — only strings that refer to the reserved category, not unrelated string literals.

4. `cargo build` and `cargo test` to confirm no regressions.

### Verification

`grep -r '"When"\|"Entry"\|"Done"' crates/aglet-core/src/` should return zero results except in tests using realistic fixture data and in the constant definition itself.

---

## Item B — Centralise UUID → SQL param conversion

**Priority:** Medium
**Effort:** Medium (2–4 h)
**Risk:** Low-Medium (touches 150+ call sites)

### Problem

`store.rs` contains 153 `.to_string()` calls (per `grep`). The majority are `some_id.to_string()` immediately before passing to `params![...]`. This is repetitive and bypasses the type system — a `CategoryId` and `ItemId` can be swapped with no compile error.

### Plan

**Option 1 (recommended): implement `rusqlite::ToSql` for newtype IDs**

`CategoryId`, `ItemId`, etc. are `pub struct CategoryId(pub Uuid)` newtypes in `model.rs`. Add:

```rust
// crates/aglet-core/src/model.rs
use rusqlite::types::{ToSql, ToSqlOutput, ValueRef};

impl ToSql for ItemId {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        self.0.to_string().to_sql()
    }
}

impl ToSql for CategoryId {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        self.0.to_string().to_sql()
    }
}
// repeat for ViewId, etc.
```

Then call sites change from:
```rust
params![item.id.to_string(), ...]
```
to:
```rust
params![item.id, ...]
```

**Steps:**
1. Identify all newtype ID types in `model.rs` that are passed to `params![]`.
2. Implement `ToSql` for each.
3. Do a crate-wide find-and-replace: `(\w+\.id)\.to_string\(\)` → `$1` inside `params![]` macros. Manual review is needed since some `.to_string()` calls are for non-SQL uses (error messages, etc.).
4. `cargo build` — compiler errors guide the remaining cases.
5. `cargo test`.

**Option 2 (simpler, less complete):** Add a `fn id_param(id: impl ToString) -> String` helper to avoid the cognitive overhead, but this doesn't add type safety. Not recommended.

---

## Item C — Standardise JSON serialisation error handling

**Priority:** Medium
**Effort:** Small (1–2 h)
**Risk:** Low

### Problem

Three patterns coexist in `store.rs` for JSON serde:

| Pattern | Example line | Semantics |
|---|---|---|
| `.unwrap_or_else(\|_\| "{}".to_string())` | 275 | Silently produce empty JSON object on ser failure |
| `.unwrap_or_default()` | 367, 694, 1025 | Silently produce `Default` on de failure |
| `.map_err(\|err\| AgletError::StorageError { ... })?` | 394–408, 738–750 | Propagate as hard error |

The inconsistency makes it hard to reason about data loss scenarios.

### Plan

Establish two explicit conventions and apply them consistently:

**Serialisation (`to_string`)** — our own types serialise without failure; unwrap is acceptable. Prefer:
```rust
// Before
serde_json::to_string(&item.assignments).unwrap_or_else(|_| "{}".to_string())
// After
serde_json::to_string(&item.assignments).expect("Item assignments are always serialisable")
```

**Deserialisation (`from_str`)** of data from the DB — data may be corrupt/legacy; prefer `unwrap_or_default()` for non-critical fields and `map_err(...)? ` for fields that are required for correct behaviour. Add a code comment for every `unwrap_or_default()` case explaining the fallback intent:

```rust
// Legacy rows may have null/malformed JSON; default to empty assignments.
let assignments = serde_json::from_str(&assignments_json).unwrap_or_default();
```

**Steps:**
1. Audit all `serde_json::to_string(...)` calls — change `unwrap_or_else(|_| ...)` to `expect(...)` with a reason string.
2. Audit all `serde_json::from_str(...)` calls — add a comment on every `unwrap_or_default()` explaining why silent fallback is safe.
3. Ensure struct fields that are load-bearing (e.g. `view.criteria`) use `map_err(...)?` rather than silently defaulting.

---

## Item D — Decompose the `Store` god struct

**Priority:** Medium
**Effort:** Large (1–2 days)
**Risk:** Medium

### Problem

`store.rs` is 3,797 lines. The `Store` struct handles item CRUD, category CRUD, view persistence, assignments, deletion log, item links, and schema migrations in a single file. This makes navigation and isolated unit testing difficult.

### Plan

Convert `store.rs` → `store/` module directory using file-level decomposition without changing the public API surface:

```
crates/aglet-core/src/store/
    mod.rs         ← Store struct definition, constructor, conn(), app settings, transactions
    migrations.rs  ← SCHEMA_SQL, SCHEMA_VERSION, run_migrations()
    items.rs       ← CRUD for Item (create, get, update, delete, list, prefix)
    categories.rs  ← CRUD for Category + flatten_hierarchy, map_write_error
    views.rs       ← CRUD for View
    assignments.rs ← Assignment reads/writes, deletion log
    links.rs       ← ItemLink CRUD
    row_mappers.rs ← row_to_item(), row_to_category(), row_to_view() private helpers
```

**Steps:**

1. Create `crates/aglet-core/src/store/` directory.
2. Copy `store.rs` → `store/mod.rs`.
3. Extract one module at a time, starting with the lowest-dependency code:
   - `row_mappers.rs` first (pure functions, no dependencies on other Store methods)
   - `migrations.rs` second (just constants and a fn)
   - `links.rs` / `assignments.rs` next
   - `views.rs`, `categories.rs`, `items.rs` last
4. In each new file, use `use super::Store;` to access shared fields and helpers.
5. In `mod.rs`, add `mod items; mod categories; ...` and re-export nothing new (internal reorganisation only).
6. `cargo build` after each extraction step.

**Note:** Do NOT change method signatures or visibility. This is a pure file-split refactor.

---

## Item E — Document or remove dead-code suppressions

**Priority:** Low
**Effort:** Small (30 min)
**Risk:** Very low

### Problem

Six `#[allow(dead_code)]` attributes exist without explanatory comments:

| File | Line | Target |
|---|---|---|
| `lib.rs` | 177 | `Mode::NoteEdit` |
| `lib.rs` | 192 | `Mode::CategoryCreateConfirm` |
| `board.rs` | 720 | unnamed function |
| `board.rs` | 1331 | unnamed function |
| `ui_support.rs` | 135 | `format_category_values_single_line` |

### Plan

For each suppression, decide:
- **If planned but not yet implemented:** replace `#[allow(dead_code)]` with `// TODO(feature): <description>` comment and keep the code.
- **If no longer planned:** remove the code entirely.
- **If intentionally kept for debugging/testing:** add `#[allow(dead_code)] // used in tests / debugging helper`.

Check each target:
1. `Mode::NoteEdit` — search call sites; if no handler exists, remove the variant or add a TODO.
2. `Mode::CategoryCreateConfirm` — same.
3. The two `board.rs` functions — read their bodies; determine if they're debug/util or stale.
4. `format_category_values_single_line` — check if it's a planned alternate display path; if so, document it.

---

## Item F — Document `origin` field semantics

**Priority:** Low
**Effort:** Small (1 h)
**Risk:** Very low

### Problem

`Assignment.origin` and `ItemLink.origin` are `Option<String>` with no documentation of valid values. In practice the codebase writes values like `"nlp:date"`, `"manual:link"`, `"cat:CategoryName"` but there's no canonical list.

### Plan

**Option 1 (recommended): Define an enum**

```rust
// crates/aglet-core/src/model.rs
/// How an assignment or link was created.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    /// Created by the NLP date parser.
    NlpDate,
    /// Created by the NLP text matcher for a given category.
    NlpCategory(String),
    /// Created manually by the user.
    Manual,
    /// Created by a rule/action in the engine.
    Rule,
}
```

Update `Assignment.origin` and `ItemLink.origin` from `Option<String>` to `Option<Origin>`. Adjust serialisation tests.

**Option 2 (lower risk):** Keep `Option<String>` but add a doc comment enumerating the canonical values and add a `const`/`enum` of the string prefixes. Less refactoring, still eliminates the knowledge gap.

---

## Sequencing Recommendation

| Order | Item | Depends on |
|---|---|---|
| 1 | A — Reserved category constants | nothing |
| 2 | C — JSON error conventions | nothing |
| 3 | E — Dead-code suppressions | nothing |
| 4 | F — Origin field docs | nothing |
| 5 | B — UUID ToSql impls | nothing (but cleanest after A) |
| 6 | D — Store decomposition | B ideally done first (fewer distractions during split) |

Items A–F can each be done independently on their own branch. Item D is the largest and highest-risk; do it last when the other cleanups have reduced noise in `store.rs`.
