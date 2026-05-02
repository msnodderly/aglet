---
title: Linked Items MVP
status: shipped
created: 2026-02-15
shipped: 2026-03-21
---

# Linked Items MVP Plan (`depends-on` / `blocks` / `related`)

## Goal

Implement item-to-item links in aglet with:

- Hard dependency links (`depends-on`)
- Inverse vocabulary (`blocks`) in CLI/TUI output and commands
- Soft, bidirectional links (`related`)

Out of scope for this MVP:

- `parent-child` links
- readiness/blocked status integration into query/view engine
- TUI multi-item marking (but design APIs to support it next)
- delete/restore link snapshotting in `deletion_log` (follow-up)

This plan is designed for the current aglet architecture:

- storage in `crates/aglet-core/src/store.rs`
- behavior validation in `crates/aglet-core/src/aglet.rs`
- CLI surfaces in `crates/aglet-cli/src/main.rs`
- TUI read-only display first, editing later

## Codebase Sync Notes (2026-02-26)

This plan was originally drafted before several unrelated core changes landed.
The walkthrough (`docs/reference/codebase-walkthrough.md`) is now the best onboarding reference for the
current architecture.

Important updates for implementation on the current branch:

- Workspace paths should use `crates/...` (not `aglet-core/...` at repo root).
- `crates/aglet-core/src/store.rs` already has `SCHEMA_VERSION = 5`.
  Linked-items schema work must target **v6** (or later if more migrations land first).
- `ItemLinkKind`, `ItemLink`, and `ItemLinksForItem` are already present in
  `crates/aglet-core/src/model.rs` with serde-name tests.
- Linked-items storage, Aglet semantics, CLI commands, CLI `show` rendering,
  and TUI read-only preview display are implemented on this branch.
- TUI link editing, view-level mark/batch linking UX, dependency row markers,
  and dependency-tree browsing are not implemented yet.

## Gaps To Fix Before Coding

- **Migration version drift**: all references to "bump to v5" are stale; use the
  next available schema version at implementation time and re-check `main`.
- **Migration validation target**: test upgrade from an existing **v5** DB
  (not v4) because current code already stamps `user_version = 5`.
- **Store kind parsing behavior**: avoid silently defaulting unknown DB values to
  `Related`; prefer returning a storage/parse error so DB corruption is visible.
- **Transaction boundary for cycle checks**: document whether cycle check + insert
  run in one transaction (recommended) to avoid future race issues if write
  concurrency grows.
- **Delete/restore UX messaging**: MVP intentionally drops links on restore;
  CLI/TUI should surface that behavior clearly to avoid surprise.
- **Display ordering policy**: the plan recommends UI sorting by item text, but
  Store ordering should still be deterministic and documented (e.g., `created_at`,
  then ID tie-breaker).
- **TUI integration touchpoints**: the current TUI refresh pipeline is centered on
  `App::refresh(...)`; fetch/caching decisions should align with that flow rather
  than ad hoc render-time DB reads.

## Semantics (MVP)

### Canonical semantics

- `A depends-on B` means `B blocks A`
- Store only `depends-on` as the canonical hard dependency direction
- Expose `blocks` as an alias/inverse in user-facing commands and output

### `related` semantics

- `A related B` is non-blocking and bidirectional
- Persist as a single normalized row (not two mirrored rows)
- Reads for an item return neighbors from either endpoint column

### Invariants

- No self-links for any kind
- `depends-on` must be acyclic
- `related` does not participate in cycle detection
- `related` and `depends-on` may coexist between the same pair

## Terminology Mapping

| User Phrase | Internal Meaning | Stored Kind | Stored Direction |
|---|---|---|---|
| `A depends on B` | hard prerequisite | `depends-on` | `A -> B` |
| `A blocks B` | inverse hard prerequisite | `depends-on` | store as `B -> A` |
| `A related B` | soft association | `related` | normalized pair only |

## Data Model Additions (`crates/aglet-core/src/model.rs`)

Add explicit link types to the domain model. Do not embed links into `Item` yet for MVP; keep link loading/querying explicit through `Store` to minimize churn.

```rust
use chrono::{DateTime, Utc};

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
```

Optional (nice-to-have for CLI/TUI rendering):

```rust
#[derive(Debug, Clone)]
pub struct ItemLinksForItem {
    pub depends_on: Vec<ItemId>,  // immediate prerequisites
    pub blocks: Vec<ItemId>,      // immediate dependents (inverse view)
    pub related: Vec<ItemId>,     // soft links
}
```

## Exact MVP SQLite Schema (`crates/aglet-core/src/store.rs`)

### `SCHEMA_VERSION`

Bump:

```rust
const SCHEMA_VERSION: i32 = 6;
```

### Add to `SCHEMA_SQL`

```sql
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
    -- For related links, store a single canonical row by UUID string order.
    CHECK (kind <> 'related' OR item_id < other_item_id)
);

CREATE INDEX IF NOT EXISTS idx_item_links_item_kind
    ON item_links(item_id, kind);
CREATE INDEX IF NOT EXISTS idx_item_links_other_kind
    ON item_links(other_item_id, kind);
CREATE INDEX IF NOT EXISTS idx_item_links_kind
    ON item_links(kind);
```

Notes:

- `metadata_json` is optional for MVP behavior, but cheap to reserve now for future link annotations.
- UUID lexical comparison is safe here because IDs are fixed-length UUID strings.

### Migration plan (`apply_migrations`)

Use idempotent `CREATE TABLE IF NOT EXISTS` / `CREATE INDEX IF NOT EXISTS` in `apply_migrations` so existing databases upgrade safely.

```rust
fn apply_migrations(&self, from_version: i32) -> Result<()> {
    // existing migrations...

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

    Ok(())
}
```

## Store Layer Design (`crates/aglet-core/src/store.rs`)

Keep `Store` focused on persistence and retrieval; do not put cycle detection here.

### Exact MVP Store API signatures

```rust
use crate::model::{ItemLink, ItemLinkKind, ItemId};
use crate::error::Result;

impl Store {
    pub fn create_item_link(&self, link: &ItemLink) -> Result<()>;

    pub fn delete_item_link(
        &self,
        item_id: ItemId,
        other_item_id: ItemId,
        kind: ItemLinkKind,
    ) -> Result<()>;

    pub fn item_link_exists(
        &self,
        item_id: ItemId,
        other_item_id: ItemId,
        kind: ItemLinkKind,
    ) -> Result<bool>;

    /// Immediate prerequisites for a dependent item (outbound depends-on edges).
    pub fn list_dependency_ids_for_item(&self, item_id: ItemId) -> Result<Vec<ItemId>>;

    /// Immediate dependents of an item (inbound depends-on edges; inverse "blocks" view).
    pub fn list_dependent_ids_for_item(&self, item_id: ItemId) -> Result<Vec<ItemId>>;

    /// Immediate related items (symmetric query over normalized `related` rows).
    pub fn list_related_ids_for_item(&self, item_id: ItemId) -> Result<Vec<ItemId>>;

    /// Optional convenience for `agenda show` / TUI panels.
    pub fn list_item_links_for_item(&self, item_id: ItemId) -> Result<Vec<ItemLink>>;
}
```

### Store implementation notes

- `create_item_link` should fail with FK error if item IDs do not exist (Aglet will pre-validate for nicer errors).
- `delete_item_link` should be idempotent (`DELETE ...` and return `Ok(())` even if absent), matching assignment removal style.
- `item_link_exists` is useful to avoid duplicate insert errors and produce clean status messages in batch linking.

### Row parser + kind encoding helpers

```rust
fn item_link_kind_to_str(kind: ItemLinkKind) -> &'static str {
    match kind {
        ItemLinkKind::DependsOn => "depends-on",
        ItemLinkKind::Related => "related",
    }
}

fn item_link_kind_from_str(s: &str) -> ItemLinkKind {
    match s {
        "depends-on" => ItemLinkKind::DependsOn,
        "related" => ItemLinkKind::Related,
        _ => ItemLinkKind::Related, // defensive fallback; consider hard error
    }
}
```

### `related` query pattern

```sql
SELECT item_id, other_item_id, kind, created_at, origin
FROM item_links
WHERE kind = 'related'
  AND (item_id = ?1 OR other_item_id = ?1)
ORDER BY created_at ASC;
```

Then map neighbor as:

```rust
let neighbor_id = if row.item_id == item_id {
    row.other_item_id
} else {
    row.item_id
};
```

## Aglet Layer Design (`crates/aglet-core/src/aglet.rs`)

Put all semantic rules here:

- self-link rejection
- canonicalization for `related`
- `blocks` alias inversion
- `depends-on` cycle detection
- batch linking support for future TUI multi-marking

### Exact MVP Aglet API signatures (as proposed, now concretized)

```rust
use crate::error::Result;
use crate::model::{ItemId, ItemLink, ItemLinkKind};

#[derive(Debug, Default, Clone)]
pub struct LinkItemsResult {
    pub created: usize,
    pub skipped_existing: usize,
}

impl<'a> Aglet<'a> {
    pub fn link_items_depends_on(
        &self,
        dependent_id: ItemId,
        dependency_id: ItemId,
        origin: Option<String>,
    ) -> Result<()>;

    pub fn link_items_blocks(
        &self,
        blocker_id: ItemId,
        blocked_id: ItemId,
        origin: Option<String>,
    ) -> Result<()>;

    pub fn link_items_related(
        &self,
        a: ItemId,
        b: ItemId,
        origin: Option<String>,
    ) -> Result<()>;

    pub fn unlink_items_depends_on(
        &self,
        dependent_id: ItemId,
        dependency_id: ItemId,
    ) -> Result<()>;

    pub fn unlink_items_blocks(
        &self,
        blocker_id: ItemId,
        blocked_id: ItemId,
    ) -> Result<()>;

    pub fn unlink_items_related(&self, a: ItemId, b: ItemId) -> Result<()>;

    /// Batch-friendly API for future TUI multi-marking ("make current item dependent on marked items").
    pub fn link_items_depends_on_many(
        &self,
        dependent_id: ItemId,
        dependency_ids: &[ItemId],
        origin: Option<String>,
    ) -> Result<LinkItemsResult>;
}
```

### Optional read APIs (Lotus-style utilities support)

```rust
impl<'a> Aglet<'a> {
    pub fn immediate_prereq_ids(&self, item_id: ItemId) -> Result<Vec<ItemId>>;
    pub fn immediate_dependent_ids(&self, item_id: ItemId) -> Result<Vec<ItemId>>;
    pub fn immediate_related_ids(&self, item_id: ItemId) -> Result<Vec<ItemId>>;

    pub fn all_prereq_ids(&self, item_id: ItemId) -> Result<Vec<ItemId>>;
    pub fn all_dependent_ids(&self, item_id: ItemId) -> Result<Vec<ItemId>>;

    pub fn list_items_with_prereqs(&self) -> Result<Vec<ItemId>>;
    pub fn list_items_with_dependents(&self) -> Result<Vec<ItemId>>;
}
```

### Canonicalization helpers (Aglet private)

```rust
fn normalize_related_pair(a: ItemId, b: ItemId) -> (ItemId, ItemId) {
    let a_str = a.to_string();
    let b_str = b.to_string();
    if a_str <= b_str { (a, b) } else { (b, a) }
}

fn build_link(
    &self,
    item_id: ItemId,
    other_item_id: ItemId,
    kind: ItemLinkKind,
    origin: Option<String>,
) -> ItemLink {
    ItemLink {
        item_id,
        other_item_id,
        kind,
        created_at: Utc::now(),
        origin,
    }
}
```

### Cycle detection (depends-on only)

When adding `dependent -> dependency`, reject if `dependency` already reaches `dependent` through existing `depends-on` edges.

```rust
fn ensure_depends_on_no_cycle(
    &self,
    dependent_id: ItemId,
    dependency_id: ItemId,
) -> Result<()> {
    use std::collections::HashSet;

    if dependent_id == dependency_id {
        return Err(AgletError::InvalidOperation {
            message: "item cannot depend on itself".to_string(),
        });
    }

    let mut seen = HashSet::new();
    let mut stack = vec![dependency_id];

    while let Some(current) = stack.pop() {
        if !seen.insert(current) {
            continue;
        }
        if current == dependent_id {
            return Err(AgletError::InvalidOperation {
                message: format!(
                    "adding dependency would create a cycle: {} depends-on ... depends-on {}",
                    dependency_id, dependent_id
                ),
            });
        }
        stack.extend(self.store.list_dependency_ids_for_item(current)?);
    }

    Ok(())
}
```

## CLI Implementation Plan (`crates/aglet-cli/src/main.rs`)

### Phase 1 (MVP CLI)

Add a new top-level subcommand:

```rust
enum Command {
    // ...
    Link {
        #[command(subcommand)]
        command: LinkCommand,
    },
}

#[derive(Subcommand, Debug)]
enum LinkCommand {
    DependsOn { dependent_id: String, dependency_id: String },
    Blocks { blocker_id: String, blocked_id: String },
    Related { item_a_id: String, item_b_id: String },

    UnlinkDependsOn { dependent_id: String, dependency_id: String },
    UnlinkBlocks { blocker_id: String, blocked_id: String },
    UnlinkRelated { item_a_id: String, item_b_id: String },
}
```

Command behavior:

- `depends-on` calls `agenda.link_items_depends_on(...)`
- `blocks` calls `agenda.link_items_blocks(...)` (inverts args internally)
- `related` calls `agenda.link_items_related(...)`
- unlink variants mirror the same semantics

### Extend `agenda show`

Add link sections after assignments:

```text
prereqs (depends_on):
  <id> | open | <text>
  ...

dependents (blocks):
  <id> | open | <text>
  ...

related:
  <id> | done | <text>
```

Implementation approach:

- use `Store` link queries to get neighbor IDs
- resolve each neighbor via `get_item`
- print immediate one-level links only in MVP

### Future CLI (Lotus-inspired utilities)

Add traversal commands after MVP:

- `agenda link prereqs <ITEM_ID> --all-levels`
- `agenda link depends <ITEM_ID> --all-levels`
- `agenda link prereqs --every-item`
- `agenda link depends --every-item`

These map directly to Lotus Agenda “Show Prereqs / Show Depends” menus.

## Lotus Agenda Reference Findings (2026-02-26)

Relevant Lotus Agenda behavior (from external material not included in this
repo) that informs Aglet's design:

- Dependency creation is a **view-level command** (`ALT-O`), not a note/item-edit subfeature.
- Lotus workflow is **mark items first**, then run dependency command against the
  current highlighted item, then confirm in a modal **"Make Item Dependent Box"**.
- Lotus distinguishes dependency browsing from editing via **Utilities Show**
  menus (`Prereqs`, `Depends`) with `One Level`, `All Levels`, and `Every Item`.
- Lotus displays a dependency marker (`&`) directly in the item row for scanability.
- Lotus includes a separate **"Clear Dependencies Box"** to remove all
  prerequisites from the current item.

Design implication for Aglet:

- keep the **view-level, mark-aware workflow shape**
- modernize the UX with a richer wizard/picker + explicit preview
- keep dependency browsing (tree/chain viewing) separate from link editing

## TUI Plan (`crates/aglet-tui`)

### Phase 1 (read-only)

Show link info in Preview Summary panel (`render/mod.rs`) beneath Categories:

- `Prereqs:` immediate dependencies
- `Blocks:` immediate dependents
- `Related:` immediate related items

Current status (merged to `main`):

- Implemented in preview summary (`Preview: Summary`)
- Cached/populated via `App::refresh(...)`

No new modes in Phase 1.

### Phase 2 (editing)

#### UX Direction (Resolved 2026-02-26)

Primary linking workflow should be a **view-level Link Wizard** opened directly
from the view with `b` / `B` (on the selected item), not a subfunction that requires
opening the item edit panel first.

Binding direction (resolved):

- `b` opens Link Wizard with `blocked by` preselected
- `B` opens Link Wizard with `blocks` preselected
- keep `L` reserved for board-column reordering
- mark/batch mode is deferred to a follow-up phase (wizard design remains batch-ready)

Item edit panel still gets convenience features:

- read-only link summary (`Prereqs`, `Blocks`, `Related`)
- single-item `Clear dependencies` action
- hint/action to open Link Wizard

#### Exact Workflow (Recommended)

When the user presses `b` or `B` in a view:

1. Open **Link Wizard** anchored to the current selected item.
   - `b` defaults to `blocked by`
   - `B` defaults to `blocks`
2. Determine scope:
   - current phase: scope = current item (single-item mode)
   - later phase: if items are marked, scope = marked items (batch mode)
3. Choose relationship/action:
   - `blocked by`
   - `depends on`
   - `blocks`
   - `related to`
   - `clear dependencies`
4. If action requires a target item, open/activate target search + picker.
5. Show a plain-language preview of the operations to apply.
6. Confirm/apply (or cancel/back).

Notes:

- `blocks` should show a preview line explaining stored semantics (`depends-on` inverse)
- `clear dependencies` skips target selection and previews removals
- the same wizard should support future batch linking and batch clear
- current implementation can ship single-item first; batch mode should not block progress

#### UI Mockup Options (Discussed)

##### Option A (Alternative): Side Panel "Link Composer"

Stays in the view while opening a right-side panel for existing links + add-link composer.
Good balance, but more layout complexity.

```text
┌ View ───────────────────────────────────────────────┬ Link Composer ───────────────────┐
│ > Track day                                         │ Anchor: Track day (open)          │
│   Task B                                            │                                   │
│   Task C                                            │ Existing                          │
│                                                     │  Depends On: Task B, Task C       │
│                                                     │  Blocks:     (none)               │
│                                                     │  Related:    Task D               │
│                                                     │                                   │
│                                                     │ Add Link                          │
│                                                     │  Relation: [blocks v]             │
│                                                     │  Target: issue _                  │
│                                                     │  Matches: > Issue 123 ...         │
│                                                     │  Preview: Track day blocks ...    │
│                                                     │                                   │
│                                                     │ Enter:add  x:remove  Tab:pane     │
└─────────────────────────────────────────────────────┴───────────────────────────────────┘
```

##### Option B (Preferred): Link Wizard (Batch-Capable)

Dedicated modal/wizard that works for one selected item now and marked-item batch
workflows later (Lotus-inspired shape, modernized UI). This is the preferred
design and matches the current single-item TUI implementation direction.

```text
┌ Link Wizard ───────────────────────────────────────────────────────────┐
│ Scope                                                                 │
│   Selected items: 1  (or N marked items)                              │
│   > Track day                                                         │
│                                                                       │
│ Relationship                                                          │
│   > blocked by                                                        │
│     depends on                                                        │
│     blocks                                                            │
│     related to                                                        │
│     clear dependencies                                                │
│                                                                       │
│ Target item (hidden for "clear dependencies")                         │
│   Search: issue _                                                     │
│   > Issue 123 Retry bug                                               │
│     Issue 201 Billing export                                          │
│                                                                       │
│ Preview (N operation(s))                                              │
│   Track day depends-on Issue 123 Retry bug                            │
│   (or batch preview lines for each marked item)                       │
│                                                                       │
│ [Apply] [Cancel]   Tab:section  j/k:move  Enter:select/apply          │
└───────────────────────────────────────────────────────────────────────┘
```

#### Dependency Markers In View (Lotus-inspired)

Add row-level dependency markers (similar in spirit to the note marker) so users
can scan for dependency state without opening preview.

Direction:

- mark items that have prerequisites (`depends-on` outbound edges; blocked items)
- mark items that block others (dependents / inbound edges)
- exact glyphs are TBD, but should support distinguishing "blocked" vs "blocking"

#### Phase 2 Scope (Implementation)

Add view-level link editing workflow and item-edit-panel convenience actions:

- `b` / `B` open Link Wizard from the view on selected item (preselecting block direction)
- add/remove `depends-on` / `blocks` / `related`
- include `clear dependencies` action (single-item at minimum)
- reuse item picker/search patterns where possible
- keep item edit panel as convenience summary + clear/open-wizard entry point

### Phase 3 (Lotus-style multi-item marking)

This is implied by “make current item dependent on marked item(s)” and should be
built on top of the batch API already in `Aglet`, reusing the same Link Wizard.

This phase is intentionally deferred until after single-item wizard stabilization
and relationship-aware filtering/readiness work.

Proposed `App` state addition:

```rust
use std::collections::HashSet;

struct App {
    // ...
    marked_item_ids: HashSet<ItemId>,
}
```

Why mark by `ItemId` (not row index):

- survives refresh/view re-resolution
- works across slots/sections
- matches Lotus semantics of marked items “in the file”

Proposed TUI behavior (follow-up):

- mark/unmark current item
- clear all marks
- `b` / `B` open Link Wizard in batch mode when marks exist (preselecting relation)
- “blocked by” / “depends on” / `clear dependencies` actions apply to all marked items
- skip self-link operations automatically if current item is also in marked set
- show preview count and per-item action list before apply

#### Future Dependency Tree / Chain Viewer (separate from wizard)

Dependency-tree browsing should be a separate viewer/command (not folded into
the Link Wizard), using the traversal APIs below:

- one-level and all-level prereq/depends views
- eventual hierarchy/tree display for "items blocking items blocking items"
- maps conceptually to Lotus "Utilities Show Prereqs / Depends"

## Traversal Implementation (Lotus “One Level” / “All Levels”)

### Immediate (One Level)

- `Prereqs`: `Store::list_dependency_ids_for_item(current)`
- `Depends`: `Store::list_dependent_ids_for_item(current)`

### Transitive (All Levels)

Use BFS/DFS with visited set over the appropriate adjacency query.

```rust
fn collect_transitive(
    &self,
    start: ItemId,
    next_ids: impl Fn(ItemId) -> Result<Vec<ItemId>>,
) -> Result<Vec<ItemId>> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    let mut stack = next_ids(start)?;

    while let Some(id) = stack.pop() {
        if !seen.insert(id) {
            continue;
        }
        out.push(id);
        stack.extend(next_ids(id)?);
    }

    Ok(out)
}
```

## Delete / Restore Behavior (MVP Decision)

MVP decision: **do not snapshot links in `deletion_log` yet**.

Behavior:

- deleting an item cascades and removes its links via FK
- restoring an item does not restore prior links

Rationale:

- keeps MVP focused on link semantics + UI
- avoids expanding `deletion_log` schema and restore logic immediately

Follow-up (recommended):

- add `item_links_json` to `deletion_log`
- restore links best-effort when counterpart items still exist

## Testing Plan

### `crates/aglet-core/src/store.rs`

- schema init creates `item_links`
- migration from v4 adds `item_links`
- create/delete `depends-on`
- create/delete `related`
- `related` normalization check enforced (via Aglet + DB CHECK)
- cascade delete removes links when item deleted
- inbound/outbound/symmetric query helpers return correct IDs

### `crates/aglet-core/src/aglet.rs`

- rejects self `depends-on`
- rejects self `related`
- rejects `depends-on` cycle (`A->B`, `B->A`, longer cycle)
- allows `related` cycles/triangles (`A~B~C~A`)
- `blocks` alias creates `depends-on` inverse correctly
- `related` insert is idempotent via normalized pair + exists check
- `link_items_depends_on_many` reports created/skipped counts correctly

### `crates/aglet-cli/src/main.rs`

- parse/dispatch `link` subcommands
- `agenda show` prints prereqs/dependents/related sections
- `blocks` and `depends-on` produce same stored edge semantics

### `crates/aglet-tui` (Phase 1)

- preview summary renders link lines for selected item
- no regressions to existing category/provenance panels

## Implementation Order (Recommended)

1. `aglet-core/model.rs`
   - add `ItemLinkKind` + `ItemLink`
2. `aglet-core/store.rs`
   - schema v5 + migration
   - persistence/query helpers
3. `aglet-core/agenda.rs`
   - link/unlink APIs
   - normalization + cycle detection
   - batch depends-on helper
4. `aglet-core` tests
5. `aglet-cli`
   - `link` subcommands
   - `show` output enhancements
6. `aglet-tui` Phase 1
   - read-only preview display
7. Follow-up issues
   - transitive CLI commands (Lotus utilities)
   - TUI multi-marking + batch link action
   - delete/restore link snapshotting

## Notes on Future Readiness Integration

If aglet later grows a “ready” command/view semantics based on dependencies:

- only `depends-on` should block readiness
- `related` remains informational
- keep per-kind behavior centralized (e.g., helper methods on `ItemLinkKind`)

Example helper:

```rust
impl ItemLinkKind {
    pub fn affects_readiness(self) -> bool {
        matches!(self, ItemLinkKind::DependsOn)
    }

    pub fn is_symmetric(self) -> bool {
        matches!(self, ItemLinkKind::Related)
    }
}
```

## Open Questions (Updated 2026-02-26)

Resolved so far:

- `agenda show` should display **one-level** links only in MVP (no transitive expansion)
- display layers should sort rendered neighbors by **item text** for readability
- CLI batch syntax can be deferred; batch linking should be handled by future TUI
  multi-marking + Link Wizard batch mode
- TUI linking should be a **view-level `b` / `B` workflow** (Link Wizard), not item-edit-panel-first
- keep a **Clear dependencies** convenience action, inspired by Lotus
- dependency tree browsing should be a **separate workflow** from link editing
- relationship-aware filtering is a core goal; first target is a **Ready** ("not blocked") view/filter

Remaining decisions:

- exact dependency marker glyph set for row scanability (`blocked`, `blocking`, `both`)
- whether to ship an item-edit-panel "Open Link Wizard" button and keybinding in
  the same release as the view-level `b` / `B` wizard, or immediately after

## Detailed TODO Checklist (Do Not Implement Yet)

This checklist is the execution plan broken into concrete tasks. Phases are ordered, but some sub-tasks can run in parallel once prerequisites land.

### Phase 0: Pre-Implementation Decisions and Task Breakdown

- [x] Confirm MVP command naming in CLI:
  - `agenda link depends-on`
  - `agenda link blocks`
  - `agenda link related`
- [x] Confirm whether unlink commands should be nested under `agenda link` (recommended) or split top-level.
- [x] Confirm `agenda show` scope is immediate links only (one-level) for MVP.
- [x] Confirm `related` rendering sort order (recommended: by item text in display layer).
- [ ] Decide whether `metadata_json` ships in MVP schema now (recommended yes, unused initially).
- [ ] Decide whether `Store::list_item_links_for_item` is required in MVP or deferred in favor of dedicated per-kind query methods.
- [x] Decide TUI linking entry point and workflow shape:
  - view-level `b` / `B` open Link Wizard (preselecting block direction)
  - keep `L` for column reorder
  - mark-aware batch mode in follow-up
  - item edit panel gets summary + clear/open-wizard convenience
- [x] Defer TUI mark/batch linking mode to a later phase after single-item wizard.
- [x] Prioritize relationship-aware filtering follow-up (`Ready` / not blocked) over batch mode polish.
- [ ] Convert this plan into tracked implementation tasks (feature requests / issues) with explicit dependencies:
  - core schema/store
  - agenda validation + traversal
  - CLI commands
  - CLI show output
  - TUI read-only display
  - tests + docs follow-up

### Phase 1: Domain Model Additions (`crates/aglet-core/src/model.rs`)

- [x] Add `ItemLinkKind` enum with serde names:
  - `depends-on`
  - `related`
- [x] Add `ItemLink` struct with fields:
  - `item_id`
  - `other_item_id`
  - `kind`
  - `created_at`
  - `origin`
- [ ] Decide whether to include `metadata_json` in `ItemLink` model immediately:
  - if yes, add `metadata_json: String` or typed metadata wrapper
  - if no, keep DB-only for now and document conversion behavior
- [x] Add `ItemLinksForItem` convenience struct (optional, recommended for CLI/TUI ergonomics).
- [x] Export model additions via existing module usage (no `lib.rs` change needed unless reexports are added later).
- [x] Add/adjust model-level tests if there are serde round-trip tests for enums/structs.

Phase 1 exit criteria:

- [x] New link types compile in `aglet-core`.
- [x] Serde names for `depends-on` / `related` are explicit and stable.

### Phase 2: SQLite Schema and Migration (`crates/aglet-core/src/store.rs`)

- [x] Bump `SCHEMA_VERSION` from `5` to `6` (or next available version if drifted again).
- [x] Add `item_links` table to `SCHEMA_SQL`.
- [x] Add `CHECK` constraints:
  - no self-link
  - allowed kinds only (`depends-on`, `related`)
  - normalized ordering for `related`
- [x] Add indices for:
  - `(item_id, kind)`
  - `(other_item_id, kind)`
  - `(kind)`
- [x] Add migration block in `apply_migrations(from_version)` for v6.
- [x] Ensure migration SQL is idempotent (`IF NOT EXISTS`).
- [x] Verify `init()` behavior for:
  - fresh DBs (schema contains `item_links`)
  - upgraded DBs (`user_version` set to 6)
- [x] Add/extend schema tests:
  - table exists
  - schema version bumped
  - idempotent `init()` still passes

Phase 2 exit criteria:

- [x] Fresh and upgraded DBs have `item_links`.
- [x] Existing tests still pass around init/migration behavior.

### Phase 3: Store Persistence + Query APIs (`crates/aglet-core/src/store.rs`)

#### 3A. Helpers and Row Parsing

- [x] Add kind string encoder (`ItemLinkKind -> &str`).
- [x] Add kind parser (`&str -> ItemLinkKind`) with explicit handling for unknown values.
- [x] Add row-to-link parser helper for `item_links` rows.
- [x] Choose parse strategy for invalid DB values:
  - hard error (preferred)
  - defensive fallback (only if necessary)

#### 3B. Write APIs

- [x] Implement `create_item_link(&self, link: &ItemLink) -> Result<()>`.
- [x] Implement `delete_item_link(...) -> Result<()>` as idempotent delete.
- [x] Implement `item_link_exists(...) -> Result<bool>`.
- [x] Confirm FK behavior is acceptable for non-existent items (Aglet will pre-validate but Store can still return storage error).

#### 3C. Read APIs (Immediate Neighbors)

- [x] Implement `list_dependency_ids_for_item(item_id)` (outbound `depends-on`).
- [x] Implement `list_dependent_ids_for_item(item_id)` (inbound inverse / `blocks` view).
- [x] Implement `list_related_ids_for_item(item_id)` (symmetric query over normalized rows).
- [x] Implement optional `list_item_links_for_item(item_id)` convenience method.
- [x] Define deterministic ordering at Store level (e.g., by `created_at`) and document that display layers may re-sort.

#### 3D. Store Tests

- [x] Add tests for `create_item_link` / `delete_item_link`.
- [x] Add test for `item_link_exists`.
- [x] Add test for `depends-on` outbound lookup.
- [x] Add test for `depends-on` inbound lookup (`blocks` inverse).
- [x] Add test for `related` symmetric lookup from both endpoints.
- [x] Add test for DB self-link constraint rejection.
- [x] Add test for DB normalized `related` check rejecting unnormalized row (if inserted directly).
- [x] Add test that deleting an item cascades and removes `item_links`.
- [x] Add test that two different kinds may coexist for same pair (`depends-on` and `related`).

Phase 3 exit criteria:

- [x] Store APIs persist and retrieve both link kinds correctly.
- [x] Symmetric `related` behavior works via single normalized row.

### Phase 4: Aglet Semantic APIs + Validation (`crates/aglet-core/src/aglet.rs`)

#### 4A. Public APIs

- [x] Add `LinkItemsResult` struct.
- [x] Implement `link_items_depends_on`.
- [x] Implement `link_items_blocks` (argument inversion alias).
- [x] Implement `link_items_related`.
- [x] Implement `unlink_items_depends_on`.
- [x] Implement `unlink_items_blocks`.
- [x] Implement `unlink_items_related`.
- [ ] Implement `link_items_depends_on_many` batch API.

#### 4B. Private Helpers

- [x] Add `normalize_related_pair(a, b)` helper.
- [x] Add `build_link(...)` helper for `ItemLink`.
- [x] Add item existence validation helper (e.g., `ensure_item_exists(item_id)` or direct `get_item` checks).
- [x] Add self-link validation helper shared across kinds.
- [x] Add duplicate-short-circuit checks using `Store::item_link_exists`.

#### 4C. Cycle Detection (`depends-on` only)

- [x] Implement `ensure_depends_on_no_cycle(dependent, dependency)`.
- [x] Ensure cycle check runs before insert.
- [x] Confirm cycle detection traverses only `depends-on` edges.
- [x] Confirm `related` links skip cycle logic entirely.
- [x] Decide and document error messages for:
  - self-link
  - duplicate
  - cycle

#### 4D. Optional Read/Traversal APIs (Lotus groundwork)

- [x] Implement immediate read APIs:
  - `immediate_prereq_ids`
  - `immediate_dependent_ids`
  - `immediate_related_ids`
- [ ] Implement internal generic traversal helper (BFS/DFS).
- [ ] Implement transitive read APIs:
  - `all_prereq_ids`
  - `all_dependent_ids`
- [ ] Implement "Every Item" helper APIs:
  - `list_items_with_prereqs`
  - `list_items_with_dependents`
- [ ] Decide whether these ship in MVP CLI/TUI or remain internal-only until follow-up commands.

#### 4E. Aglet Tests

- [x] Add test: `depends-on` rejects self-link.
- [x] Add test: `related` rejects self-link.
- [x] Add test: `depends-on` cycle rejection (`A->B`, `B->A`).
- [x] Add test: longer cycle rejection (`A->B->C`, add `C->A`).
- [x] Add test: `related` triangle allowed (`A~B`, `B~C`, `C~A`).
- [x] Add test: `link_items_blocks` stores inverse `depends-on` edge correctly.
- [x] Add test: `link_items_related` normalizes pair and is idempotent.
- [ ] Add test: `link_items_depends_on_many` skips duplicates and self.
- [ ] Add test: `link_items_depends_on_many` returns accurate counts.
- [ ] Add tests for transitive traversal order/contents (if traversal APIs implemented in this phase).

Phase 4 exit criteria:

- [x] Aglet enforces all semantic invariants.
- [x] `blocks` and `depends-on` are equivalent user vocabularies over one stored representation.

### Phase 5: CLI Link Commands (`crates/aglet-cli/src/main.rs`)

#### 5A. Command Definitions and Dispatch

- [x] Add `Command::Link` top-level variant.
- [x] Add `LinkCommand` enum with link and unlink variants.
- [x] Wire command dispatch in `run()`.
- [x] Add `cmd_link(...)` handler function.

#### 5B. Parsing and Execution

- [x] Reuse `parse_item_id` for all link commands (full UUID only, current behavior).
- [x] Implement handler branches:
  - `depends-on`
  - `blocks`
  - `related`
  - unlink variants
- [x] Choose success output format for each command (consistent with existing CLI style).
- [x] Choose idempotency messaging:
  - silent success on existing link
  - explicit "already exists"
  - count-based output
- [x] Map Aglet errors to user-friendly CLI messages (especially cycle/self-link).

#### 5C. CLI Tests

- [x] Add parser/dispatch unit tests if coverage exists for command parsing patterns.
- [x] Add output-focused tests for helper text (if command handler helpers are testable).
- [x] Add manual verification script examples in plan/docs for:
  - create `depends-on`
  - create `blocks`
  - create `related`
  - unlink each
  - cycle rejection

Phase 5 exit criteria:

- [x] CLI can create and remove all MVP link types with both vocabularies.

### Phase 6: CLI `show` Enhancements (`crates/aglet-cli/src/main.rs`)

#### 6A. Read and Render Immediate Links

- [x] Extend `cmd_show` to query immediate links for selected item.
- [x] Resolve neighbor IDs to items (text/status) for display.
- [x] Render separate sections:
  - prereqs (`depends-on`)
  - dependents (`blocks`)
  - related
- [x] Define behavior when linked item cannot be loaded (should be impossible with FKs, but guard and label if needed).
- [x] Sort rendered rows by item text (current recommendation).

#### 6B. Output Format Consistency

- [x] Match existing `cmd_show` style (labels, indentation, `(none)` markers).
- [x] Ensure output remains readable for items with no links.
- [x] Confirm categories/assignments output remains unchanged in ordering.

#### 6C. CLI Tests / Manual Checks

- [x] Add tests (if practical) or documented manual checks for `agenda show` link sections.
- [x] Manual check one-level semantics:
  - only immediate neighbors shown
  - no transitive chain expansion yet

Phase 6 exit criteria:

- [x] `agenda show` presents link information clearly without regressions.

### Phase 7: TUI Phase 1 (Read-Only Link Display)

#### 7A. Data Access Strategy

- [x] Decide where link data is fetched for preview rendering:
  - direct Store calls in render path (avoid if possible)
  - precomputed in `App::refresh` (preferred if performance acceptable)
  - on-demand helper methods using `Store` from event/refresh path
- [x] Choose minimal implementation for MVP (read-only, immediate neighbors only).

#### 7B. App/Render Changes

- [x] Add helper(s) to compute immediate link labels for selected item.
- [x] Extend `item_details_lines_for_item` in `render/mod.rs` to include:
  - `Prereqs`
  - `Blocks`
  - `Related`
- [x] Preserve existing preview scroll behavior and line counts.
- [x] Ensure no layout overflow regressions in preview pane.

#### 7C. TUI Tests

- [x] Add/extend unit tests for preview summary text lines (if feasible with existing helpers).
- [x] Manual TUI smoke test:
  - selected item with all three categories of link output
  - item with no links
  - switching selection updates preview correctly

Phase 7 exit criteria:

- [x] TUI preview shows immediate link context read-only.

### Phase 8: Integration Validation and QA (MVP Cut)

- [x] Run `cargo test --workspace`.
- [x] Run targeted manual CLI scenarios on `aglet-features.ag` or a temp `.ag`:
  - create items
  - add `depends-on`
  - add `blocks` (verify inversion)
  - add `related`
  - reject cycle
  - reject self-link
  - delete item and verify cascade removes links
- [x] Manual TUI check (if Phase 7 included in MVP cut).
- [x] Verify DB migration on an existing v5 database file (copy/scratch DB).
- [x] Verify idempotent startup after migration (`Store::init`).
- [x] Confirm no impact on existing engine/category/view flows.

Phase 8 exit criteria:

- [x] MVP functionality works end-to-end.
- [x] No regressions in existing workflows/tests.

### Phase 9: Post-MVP Follow-Ups (Planned, Not in MVP)

#### 9A. Lotus-Style CLI Traversal Utilities

- [ ] Add `agenda link prereqs <ITEM_ID>` (One Level default).
- [ ] Add `agenda link prereqs <ITEM_ID> --all-levels`.
- [ ] Add `agenda link depends <ITEM_ID>` (One Level default).
- [ ] Add `agenda link depends <ITEM_ID> --all-levels`.
- [ ] Add `agenda link prereqs --every-item`.
- [ ] Add `agenda link depends --every-item`.
- [ ] Add output formatting for chain/tree display.

#### 9B. TUI Editing for Links

- [x] Add view-level `b` / `B` Link Wizard for selected item (single-item mode).
- [x] `b` preselects `blocked by`; `B` preselects `blocks`.
- [x] Add relationship choices:
  - `blocked by`
  - `depends on`
  - `blocks`
  - `related to`
  - `clear dependencies`
- [x] Add target item search/picker for relationship modes that require a target.
- [x] Add explicit preview panel/list before apply (plain-language operations).
- [ ] Add add/remove commands for `depends-on` / `blocks` / `related`.
- [x] Add `clear dependencies` action (single-item at minimum).
- [x] Reuse picker/search patterns for item selection.
- [x] Add status messages for successful/failed link operations.
- [ ] Add item edit panel convenience features:
  - read-only link summary
  - `Clear dependencies` action
  - hint/action to open Link Wizard

#### 9C. TUI Multi-Item Marking (Lotus ALT-O analog)

Deferred follow-up phase (do not block current single-item linking/readiness work).

- [ ] Add `marked_item_ids: HashSet<ItemId>` to `App`.
- [ ] Add mark/unmark current item keybinding(s).
- [ ] Add clear-marks command.
- [ ] Make `b` / `B` open Link Wizard in batch mode when marks exist.
- [ ] Add “blocked by” / “depends on” batch actions using `Aglet::link_items_depends_on_many` (or equivalent batch APIs).
- [ ] Add batch `clear dependencies` action.
- [ ] Add UI affordance showing marked count and/or mark indicators.
- [ ] Add tests for marks surviving refresh/view changes.

#### 9D. TUI Dependency Markers In Views

- [ ] Add row-level dependency markers (Lotus-inspired) in the item list/board row marker area.
- [ ] Distinguish at least:
  - item has prerequisites (blocked)
  - item has dependents (blocking)
  - both states (combined marker or dual marker)
- [ ] Ensure marker rendering coexists with note/done/alarm indicators.
- [ ] Add tests for marker rendering state combinations.

#### 9E. Deletion/Restore Link Snapshotting

- [ ] Extend `deletion_log` schema with `item_links_json`.
- [ ] Snapshot links on delete.
- [ ] Restore links best-effort on restore if counterpart items exist.
- [ ] Add tests for partial restore behavior when some linked items are missing.

#### 9F. Dependency Tree / Chain Viewer (Future)

- [ ] Add TUI dependency browser/viewer separate from Link Wizard.
- [ ] Support one-level and all-level traversal for prereqs and dependents.
- [ ] Add hierarchy/tree rendering for "items blocking items blocking items".
- [ ] Add navigation from selected item into dependency tree viewer and back.

#### 9G. Readiness/Blocked Views (Future)

- [ ] Define readiness semantics for aglet ("Ready" = not blocked) and document edge cases.
- [ ] Ensure only dependency/blocking relationships (`depends-on`) affect readiness (`related` does not).
- [ ] Decide whether done prerequisites still block readiness or are treated as satisfied.
- [ ] Add query/filter support for dependency state (at minimum `ready` / `blocked`).
- [ ] Add a user-facing "Ready" view/filter (CLI and/or TUI).
- [ ] Add tests for readiness filtering with mixed `depends-on`, `related`, and done items.

### Cross-Cutting Documentation Tasks

- [ ] Update `docs/reference/codebase-walkthrough.md` after implementation to mention `item_links` schema and link APIs.
- [ ] Update `AGENTS.md` if any operational surprises are discovered during implementation (e.g., migration caveats, CLI direction confusion).
- [ ] Add CLI usage examples for links to docs (if project has a suitable CLI reference file for aglet).
- [ ] Document exact semantics (`A depends-on B` means `B blocks A`) in user-facing text to prevent direction mistakes.

### Suggested Execution Tracking Format

When implementation starts, track each phase as:

- `pending`
- `in_progress`
- `completed`
- `blocked` (with reason)

Recommended milestone checkpoints:

- [x] Milestone A: Core schema + Store APIs + tests
- [x] Milestone B: Aglet semantics + cycle detection + tests
- [x] Milestone C: CLI commands + `show` output
- [x] Milestone D: TUI read-only preview (optional for MVP cut if time-boxed)
