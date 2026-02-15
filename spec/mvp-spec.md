# Agenda Reborn — MVP Spec

> A modern clone of Lotus Agenda (1988), the free-form personal information manager.
> Type information in natural language. The system figures out how to organize it.
>
> This spec defines the minimum viable product: the smallest thing that delivers
> the core interaction loop of items, categories, views, and edit-through semantics.

---

## 1. Principles

1. **Schema-last**: Type first. Structure is inferred or added after the fact.
2. **One item, many homes**: Multifiling is the killer feature.
3. **Views are the interface**: Users interact with filtered, sectioned projections — and edit through them.
4. **The hierarchy is a program**: The category tree with conditions and actions is a declarative rule engine.
5. **Start small, stay small**: Resist scope creep. This is how Chandler died.
6. **Frontend-agnostic core**: The engine is a library with zero UI dependencies.

---

## 2. Data Model

### 2.1 Item

```rust
Item {
    id:           Uuid,
    text:         String,           // max ~350 chars, the headline
    note:         Option<String>,   // unlimited length, the body
    created_at:   DateTime<Utc>,
    modified_at:  DateTime<Utc>,
    entry_date:   NaiveDate,        // auto-set on creation, never null
    when_date:    Option<NaiveDateTime>, // parsed from text or manually set
    done_date:    Option<NaiveDateTime>,
    is_done:      bool,             // default false
    assignments:  HashMap<CategoryId, Assignment>,
}

Assignment {
    source:      AssignmentSource,  // manual | auto_match | action | subsumption
    assigned_at: DateTime<Utc>,
    sticky:      bool,             // default true; sticky = never auto-removed
    origin:      Option<String>,   // human-debuggable provenance string
}

// Assignment.origin format:
// Short <namespace>:<detail> string. Null if no meaningful source.
//
//   "cat:Urgent"              — implicit string match on category name
//   "cond:Escalated.profile"  — profile condition on Escalated
//   "action:Done.remove"      — RemoveAction fired by Done category
//   "nlp:date"                — date parser assigned to When
//   "manual"                  — explicit user action (or just use source field)

enum AssignmentSource {
    Manual,           // user explicitly assigned
    AutoMatch,        // string matcher assigned
    Action,           // fired by a category action
    Subsumption,      // inherited from child category
}
```

**Key behaviors:**

- `text` is never empty. `note` is optional extended content (headline + body model).
- `entry_date` is always set on creation, never null.
- `when_date` is populated by the date parser or set manually.
- Notes are searchable — text search covers both `text` and `note`.
- Notes are indicated in the grid by a `+` marker so the user sees which items have them.
- Items exist independently in the database. Views and categories reference them.
- All assignments are **sticky** — the rule engine never revokes existing assignments.
  Re-evaluation only adds new assignments. Only the user can remove them.

### 2.2 Category

```rust
Category {
    id:                     Uuid,
    name:                   String,     // globally unique, case-insensitive
    parent:                 Option<CategoryId>,
    children:               Vec<CategoryId>,  // ordered
    is_exclusive:           bool,       // default false
    enable_implicit_string: bool,       // default true
    note:                   Option<String>,
    created_at:             DateTime<Utc>,
    modified_at:            DateTime<Utc>,  // updated on rename, reparent, condition/action changes
    conditions:             Vec<Condition>,
    actions:                Vec<Action>,
}
```

**Key behaviors:**

- **Subsumption**: Assigned to child → implicitly assigned to all ancestors.
- **Mutual exclusion**: If `is_exclusive`, an item can be in at most one child.
  Assigning to a new child auto-unassigns from siblings.
- **Implicit string matching**: By default, every category matches item text against
  its name (case-insensitive word match). Set `enable_implicit_string = false` to
  disable (e.g., "Done" shouldn't match text containing "done").
- Names are globally unique (case-insensitive). Reserved names cannot be reused.

### 2.3 Reserved Categories

Three built-in categories always exist and cannot be deleted:

- **`When`** — date-type. Items with parsed dates are assigned to virtual
  subcategories (computed at query time, not stored):

  ```
  Overdue | Today | Tomorrow | This Week | Next Week | This Month | Future | No Date
  ```

  Bucket boundaries use the system's local timezone and current date.

- **`Entry`** — tracks when items were created.

- **`Done`** — boolean. When assigned:
  - `is_done` is set to `true`, `done_date` is recorded.
  - Actions attached to `Done` fire normally (e.g., RemoveAction to unassign
    from active project categories).
  - Views can exclude `Done` items via query criteria.

Reserved categories can have user-defined conditions and actions like any other category.

### 2.4 Conditions and Actions

```rust
enum Condition {
    // Implicit from category name when enable_implicit_string = true.
    // Case-insensitive word boundary match.
    ImplicitString,

    // Query evaluated against the item's current assignments.
    // "If assigned to both Urgent AND Project Alpha → assign to Escalated"
    Profile { criteria: Query },
}

enum Action {
    // Push item onto additional categories
    Assign { targets: HashSet<CategoryId> },

    // Unassign from categories (non-destructive; item stays in database)
    Remove { targets: HashSet<CategoryId> },
}
```

**Processing model:**

1. Item created or modified → enters processing queue.
2. Engine walks category hierarchy depth-first.
3. For each category, evaluates conditions (ORed).
4. String matches above threshold → auto-assign.
5. For each new assignment, fires that category's actions.
6. If actions modify the item, it re-enters the queue.
7. Fixed-point: stops when a pass produces no new changes.
8. **Termination**: max 10 passes. Cycle detector tracks `(ItemId, CategoryId)` pairs —
   duplicate assignments are skipped. RemoveAction results are deferred until cascade completes.

### 2.5 The Classifier Trait

String matching is the MVP implementation. The architecture supports swapping in
LLM-based classification in future phases.

```rust
pub trait Classifier: Send + Sync {
    /// Returns None = no match, Some(confidence) = match.
    /// MVP: SubstringClassifier returns Some(1.0) or None.
    /// Future: LlmClassifier returns graded confidence scores.
    fn classify(&self, text: &str, category_name: &str) -> Option<f32>;
}
```

For MVP, classification is fully automatic: match = assign. No suggestion queue,
no thresholds, no review UI. The trait's confidence return type preserves the
hook for future threshold-based routing (auto-assign vs suggest vs ignore).

### 2.6 View

```rust
View {
    id:                       Uuid,
    name:                     String,
    criteria:                 Query,
    sections:                 Vec<Section>,  // ordered
    columns:                  Vec<Column>,
    show_unmatched:           bool,          // default true
    unmatched_label:          String,        // default "Unassigned"
    remove_from_view_unassign: HashSet<CategoryId>,
}

Section {
    title:              String,
    criteria:           Query,
    on_insert_assign:   HashSet<CategoryId>,
    on_remove_unassign: HashSet<CategoryId>,
    show_children:      bool,  // default false
}

Column {
    heading: CategoryId,
    width:   u16,
}

Query {
    include:         HashSet<CategoryId>,     // must be assigned to ALL
    exclude:         HashSet<CategoryId>,     // must NOT be assigned to ANY
    virtual_include: HashSet<WhenBucket>,     // must match ALL
    virtual_exclude: HashSet<WhenBucket>,     // must NOT match ANY
    text_search:     Option<String>,          // full-text over item text + notes
}

enum WhenBucket {
    Overdue, Today, Tomorrow, ThisWeek, NextWeek, ThisMonth, Future, NoDate,
}
```

**Key behaviors:**

- Views are live. Changes to items or categories update views immediately.
- **Edit-through semantics:**
  - Insert item in section → assigns `section.on_insert_assign` + `view.criteria.include`
  - Remove item from section → unassigns `section.on_remove_unassign`
  - Remove item from view → unassigns `view.remove_from_view_unassign`
  - Delete item → permanently removes from database
- **Section membership**: An item appears in every matching section (default).
  If no section matches and `show_unmatched = true`, appears under `unmatched_label`.
  Items appear in explicit sections OR unmatched, never both.
- **Unmatched section** behaves as a generated section with edit-through:
  - `on_insert_assign`: `view.criteria.include`
  - `on_remove_unassign`: `view.remove_from_view_unassign`
- **show_children**: When a section's criteria is a single category include,
  auto-generates subsections for each direct child. One level only. Uses the
  category's stored child order. Items matching parent but no child go to
  unmatched. Inherits parent section's on_insert_assign + child category.

### 2.7 Date Parser

MVP date parsing handles common cases. Architecture supports plugging in
LLM-based parsing for advanced expressions in future phases.

```rust
pub trait DateParser: Send + Sync {
    /// Extract a date/time from item text. Returns None if no date found.
    fn parse(&self, text: &str, reference_date: NaiveDate) -> Option<ParsedDate>;
}

pub struct ParsedDate {
    pub datetime: NaiveDateTime,
    pub span: (usize, usize),  // character range in source text
}
```

**MVP parser handles:**

- Absolute: "May 25, 2026", "2026-05-25", "12/5/26", "December 5"
- Relative: "today", "tomorrow", "yesterday", "next Tuesday", "this Friday"
- Times: "at 3pm", "at 15:00", "at noon"
- Compound: "next Tuesday at 3pm"

**Deferred to future phases (LLM-assisted):**

- "a week from Thursday", "the last week in June"
- Recurring expressions ("every Tuesday", "the 1st of every month")
- Disambiguation ("March forward" is not a date, "May I ask" is not a date)

### 2.8 Undo

Single-level, in-memory undo. Low priority for MVP but the data model supports it.

Undoable operations:
- Item creation (undo = delete)
- Item deletion (undo = restore with assignments)
- Item text/note edit (undo = restore previous text)
- Category assignment/unassignment (undo = reverse)
- Item move between sections (undo = reverse implied assignment changes)

Stack depth: 1. Resets on application restart.

### 2.9 Deletion Log

A separate table persists deleted items for recovery. Every delete (user or action)
writes here before removing from the items table. This is the safety net — data
destruction is always recoverable.

```rust
DeletionLogEntry {
    id:              Uuid,          // new ID for the log entry
    item_id:         Uuid,          // original item ID
    text:            String,
    note:            Option<String>,
    entry_date:      NaiveDate,
    when_date:       Option<NaiveDateTime>,
    done_date:       Option<NaiveDateTime>,
    is_done:         bool,
    assignments_json: String,       // serialized assignments snapshot
    deleted_at:      DateTime<Utc>,
    deleted_by:      String,        // "user" | "action:CategoryName" — what triggered deletion
}
```

**Behaviors:**
- Written to on every item deletion, no exceptions.
- Queryable: `agenda deleted` CLI command, or a TUI view in future phases.
- Restorable: restoring an entry recreates the item with original assignments.
- Never automatically pruned. The user can manually clear old entries.

---

## 3. Architecture

```
┌──────────────────────────────┐
│  TUI (ratatui)  │  CLI (clap) │   Frontends. Send commands. Render state.
├──────────────────────────────┤
│  agenda-core                  │   Library crate. Zero UI dependencies.
│  ┌────────────┐ ┌──────────┐ │
│  │ Engine     │ │ Query    │ │   Rule processor, query evaluator,
│  │ Classifier │ │ DateParser│ │   trait-based matching and parsing.
│  ├────────────┴─┴──────────┤ │
│  │ Store (rusqlite + WAL)   │ │   Single-file SQLite. Crash-safe.
│  └──────────────────────────┘ │
└──────────────────────────────┘
```

### 3.1 Crate Layout

```
agenda/
├── crates/
│   ├── agenda-core/       # Engine, models, store. No UI.
│   ├── agenda-tui/        # ratatui frontend
│   └── agenda-cli/        # CLI commands (add, list, search)
├── Cargo.toml             # Workspace
```

### 3.2 Storage

- Single SQLite file. Default: `~/.agenda/default.ag`
- WAL mode for crash safety.
- Path configurable via `--db` flag or `AGENDA_DB` env var.
- No server. No network. No sync. Local-only.

### 3.3 Performance Targets

- View switching: <100ms for <10,000 items.
- Rule engine processing per item: <50ms for <500 categories.
- Retroactive assignment on category creation: runs asynchronously,
  does not block the UI. Progress indicator shown.

---

## 4. TUI

### 4.1 Layout

```
┌─ View Name ──────────────────────────────────── [F8:Views  F9:Categories] ─┐
│                                                                             │
│  ▸ Section Name                                              When           │
│    Item text here                                            Feb 14         │
│    Another item with a note                              +   Feb 13         │
│                                                                             │
│  ▸ Another Section                                                          │
│    More items...                                             Feb 18         │
│                                                                             │
│  ▸ Unassigned                                                               │
│    Items matching view but no section                        —              │
│                                                                             │
├─────────────────────────────────────────────────────────────────────────────┤
│  [→ Section Name] > _                                            Ctrl-Z Undo│
└─────────────────────────────────────────────────────────────────────────────┘
```

### 4.2 Key Bindings

| Key | Action |
|-----|--------|
| `Enter` | Edit item text inline |
| `Ctrl+G` | Open item in `$EDITOR` (text + note) |
| `n` | Open/edit note for selected item |
| `a` | Quick-assign category to selected item |
| `d` | Toggle Done on selected item |
| `x` | Delete item (with confirmation) |
| `r` | Remove item from current view (unassign, non-destructive) |
| `i` | Inspect assignments (show provenance) |
| `/` | Filter current view (incremental text search) |
| `Esc` | Clear filter / cancel operation |
| `F8` | View switcher |
| `F9` | Category manager |
| `Tab` | Move between columns |
| `Arrow keys` | Navigate items and sections |
| `Ctrl+Z` | Undo |

### 4.3 Input Bar

The bottom input bar is context-sensitive:

- When cursor is in a section: shows `[→ Section Name]`. New items get that
  section's `on_insert_assign` categories + view criteria `include` categories.
- When no section is focused: shows `[no section]`. New items enter the database
  with no section assignment — only the classification engine assigns categories.

### 4.4 $EDITOR Integration

`Ctrl+G` suspends the TUI and opens the selected item in `$EDITOR` (or `vi` as
fallback). File format:

```
Item text goes on the first line
---
Note content goes below the separator.
Can be multiple lines.
```

On save and exit, the TUI resumes and applies changes. If the text changed,
the classification engine re-evaluates.

### 4.5 View Switcher (F8)

Popup list of all views. Arrow keys + Enter to select. Typing filters the list.

### 4.6 Category Manager (F9)

Tree view of the category hierarchy. Operations:

- Create category (at root or as child of selected)
- Rename category
- Reparent (move in hierarchy)
- Toggle `is_exclusive`
- Toggle `enable_implicit_string`
- Add/edit conditions and actions
- Delete category (prompt if non-empty: remove category only vs delete items)
- Item count displayed next to each category

### 4.7 Search

- `/` — View filter. Narrows current view. `Esc` clears.
- Searches match against item `text` and `note` content.

### 4.8 First Launch

1. Creates database with three reserved categories: When, Entry, Done.
2. Creates default "All Items" view with a When column.
3. Empty grid with input bar ready. No wizard, no onboarding.
4. User can type their first item immediately.

---

## 5. CLI

Single binary, subcommand interface:

```bash
# Quick-add (the primary use case)
agenda add "Call Sarah this Friday about the proposal"

# List items (optionally filtered)
agenda list
agenda list --view "Sarah 1:1"
agenda list --category "Urgent"

# Search
agenda search "deploy"

# Mark done
agenda done <item-id>

# Launch TUI (default when no subcommand)
agenda
```

CLI uses the same `agenda-core` engine. Date parsing and classification run
on every `add`. The item is fully processed before the command returns.

---

## 6. Build Order

Each step produces testable, demonstrable functionality.

### Phase A: Foundation (agenda-core)

1. **Data model + SQLite schema** — Item, Category, View, Assignment structs.
   CRUD operations. Schema migrations. This is the foundation — get it right.
2. **String matcher + rule engine** — Create a category, watch existing items
   get auto-assigned. The "aha" in a unit test.
3. **Query evaluator** — Given a View's Query, return matching items grouped by
   sections. Virtual When buckets resolve against current date.
4. **Date parser** — Extract dates from item text. Populate when_date.

### Phase B: CLI

5. **`agenda add`** — Create item from command line. Date parsed, categories
   assigned, stored in SQLite. Confirm it works by reading back.
6. **`agenda list`, `agenda search`** — Read-only queries. Validates the query
   evaluator works end-to-end.

### Phase C: TUI

7. **Read-only grid** — Render a view with sections, items, columns. View
   switching (F8). This is the first time you *see* it work.
8. **Input bar + item creation** — Type items into the TUI. Context-sensitive
   section assignment. Edit-through on insert.
9. **Edit-through: move and remove** — Move items between sections, remove from
   view. Assignment changes happen as side effects.
10. **Category manager (F9)** — Create, edit, reparent categories. Retroactive
    assignment runs and the grid updates live.
11. **Inline editing + $EDITOR** — Edit item text inline (Enter) and in $EDITOR
    (Ctrl+G). Note editing. Re-classification on text change.
12. **Inspect, Done, Delete, Undo** — Polish operations. Undo is last.

### Phase D: Hardening

13. **Deletion log** — Persist deleted items for recovery.
14. **Retroactive assignment progress** — Async processing with UI indicator.
15. **Edge cases** — Exclusive sibling enforcement, subsumption correctness,
    cycle detection in rule engine, empty state handling.

---

## 7. What's Deferred

Explicitly out of scope for MVP. Listed as a guard against scope creep.

### Phase 2

- Recurrence (recurring items, RecurrenceRule, series tracking)
- `Entry When Done` reserved category (two-phase completion sequencing)
- LLM-based classification (swap in via Classifier trait)
- Advanced date parsing (LLM-assisted, via DateParser trait)
- Classification settings (confidence_threshold, review_threshold, assignment_mode)
- Suggestion review queue
- Column value types (numeric, text, date — boolean membership only for MVP)
- Column computations (sum, average, max, min, count)
- ValidationCondition (preconditions on assignment)
- Category aliases
- Condition AND mode (condition_mode — OR only for MVP)
- Date range filters on Query
- Sort specification on views
- Multi-level undo/redo
- Import from file
- Global search (cross-view)
- Per-category confidence thresholds

### Never

- Contacts, email, calendar UI, collaboration, sync, cloud, mobile, printing

---

## 8. Glossary

| Term | Definition |
|------|------------|
| **Item** | Atomic unit of information: a sentence, task, fact, or thought with optional note |
| **Category** | Named concept in a hierarchy used to organize items via assignment |
| **Assignment** | Relationship between item and category, with source provenance |
| **View** | Saved, dynamic, editable projection of items filtered by Query |
| **Section** | Grouping within a view, defined by Query criteria, with edit-through semantics |
| **Edit-through** | Inserting/moving/removing items in a view implicitly changes category assignments |
| **Subsumption** | Assigned to child → implicitly assigned to all ancestors |
| **Mutual exclusion** | Exclusive category → item in at most one child |
| **Multifiling** | One item assigned to many categories simultaneously |
| **Classifier** | Trait for matching items to categories. MVP: substring. Future: LLM |
| **DateParser** | Trait for extracting dates from text. MVP: common patterns. Future: LLM |
| **When bucket** | Virtual date subcategory (Today, This Week, etc.) computed at query time |
| **Sticky** | Assignment that is never auto-removed by the rule engine |
| **Deletion log** | Persistent record of deleted items, browsable and restorable |

---

*This spec is the MVP. Build it, use it, then decide what's next.*
