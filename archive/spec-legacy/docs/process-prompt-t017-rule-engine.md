# Task: T017 — Rule Engine (process_item)

## Context

You are building the **rule engine** for Agenda Reborn — the component that
makes auto-assignment work. When an item is created or modified, the engine
walks the category hierarchy and decides which categories the item belongs to.
This is the heart of the system: without it, categories are just labels that
users assign manually.

The engine evaluates two kinds of conditions, each serving a different
level of automation:

- **ImplicitString** (name-based, zero-config): Does the item's text contain
  the category's name? Already implemented — you'll call the `Classifier`
  trait from `matcher.rs`. This is the entry-level "aha moment" — create a
  category named "Sarah" and items containing "Sarah" auto-assign.

- **Profile** (assignment-based, user-configured): Is the item already
  assigned to a specific combination of categories? This lets users build
  cascading rules like "if assigned to both Urgent AND Project Alpha, also
  assign to Escalated." The input is the item's current assignment set, not
  its text. Profile conditions are what make the category tree a declarative
  rule engine — users express organizational logic without writing code.

When a condition matches, the engine assigns the item to that category and
fires the category's actions, which may assign to (or remove from) other
categories. Actions can trigger further condition matches, creating cascades
that the fixed-point loop (T018) resolves.

See `spec/design-decisions.md` §1 for the full design rationale.

## What to read

Before writing code, read these files:

1. `spec/mvp-spec.md` §2.4 (Conditions and Actions) — the processing model
   you're implementing. Pay close attention to steps 1-8.
2. `spec/mvp-spec.md` §2.2 (Category) — subsumption, mutual exclusion,
   `enable_implicit_string`. You won't implement subsumption or mutual
   exclusion (those are T019/T020), but you need to understand what the engine
   will eventually do so you design the right extension points.
3. `crates/agenda-core/src/matcher.rs` — the `Classifier` trait and
   `SubstringClassifier`. Your engine takes a `&dyn Classifier`.
4. `crates/agenda-core/src/model.rs` — `Category`, `Condition`, `Action`,
   `Assignment`, `AssignmentSource`, `Query`. Understand the data shapes.
5. `crates/agenda-core/src/store.rs` — the store API you'll call:
   `get_hierarchy()`, `get_item()`, `list_items()`, `assign_item()`,
   `unassign_item()`, `get_assignments_for_item()`.
6. `AGENTS.md` — branching workflow and issue comment protocol.

## What to build

**File**: `crates/agenda-core/src/engine.rs`

### Core: `process_item`

Given an item, walk the full category hierarchy (depth-first) and evaluate
each category's conditions against the item. When a condition matches, assign
the item to that category and fire the category's actions.

**Condition evaluation rules:**

- Conditions are **ORed** — if any condition matches, the category matches.
- **ImplicitString**: Check `category.enable_implicit_string` (the bool flag
  on the Category struct). If true, call the classifier. If the classifier
  returns `Some(_)`, the category matches. Do NOT look at the `conditions` vec
  for `Condition::ImplicitString` — the bool flag is the source of truth.
- **Profile**: Check the `conditions` vec for `Condition::Profile { criteria }`
  entries. Evaluate `criteria` as a simple set-membership check against the
  item's current assignments: the item must be assigned to ALL categories in
  `criteria.include` AND NOT assigned to ANY in `criteria.exclude`. Ignore
  `virtual_include`, `virtual_exclude`, and `text_search` — those are for
  View queries (Phase 4).
- Reserved categories (When, Entry, Done) have `enable_implicit_string = false`
  so they won't match by name. But they can still have Profile conditions.

**On match:**
- Assign the item to the category via `store.assign_item()` with
  `source: AutoMatch` and an appropriate `origin` string (e.g., `"cat:Sarah"`
  for implicit string, `"profile:<category-name>"` for profile conditions).
- Fire the category's actions:
  - `Action::Assign { targets }` → assign the item to each target category
    (source: `Action`, origin referencing the triggering category).
  - `Action::Remove { targets }` → **collect but defer**. Remove actions are
    not applied during the cascade. They accumulate and are applied after the
    fixed-point loop completes (T018 will handle this, but your design should
    accommodate it — e.g., return the deferred removals).
- Assignments are **sticky** — never revoke an existing assignment. If the
  item is already assigned to a category, skip it (don't re-assign, don't
  re-fire actions). This is critical for termination.

**Depth-first walk:**
- Use `store.get_hierarchy()` which returns categories in depth-first order
  (parents before children, sorted by `sort_order`).
- Evaluate every category on every pass. The hierarchy walk is the outer loop;
  the fixed-point loop (T018) is the outer-outer loop that re-runs this walk
  when new assignments are made.

### Design for extensibility

T018 (fixed-point loop), T019 (subsumption), and T020 (mutual exclusion)
will extend this engine. Your `process_item` should be structured so that:

- T018 can wrap it in a loop that re-runs until no new assignments are made.
- T019 can hook into "after assignment" to walk up the parent chain.
- T020 can hook into "before assignment" to check exclusive parent and unassign
  siblings.

You don't need to build these hooks, but be aware they're coming. A clean
separation between "evaluate conditions" and "perform assignment" makes the
extensions straightforward.

## Tests to write

Use `Store::open_memory()` for an in-memory database. Create test categories
and items via the store, then run the engine.

1. **ImplicitString match**: Create category "Sarah" (default
   `enable_implicit_string = true`). Create item "Call Sarah tomorrow". Run
   engine. Item should be assigned to "Sarah" with `source: AutoMatch`.

2. **ImplicitString disabled**: Create category "Done" with
   `enable_implicit_string = false`. Create item "Get it done". Run engine.
   Item should NOT be assigned to "Done".

3. **Profile condition match**: Create categories "Urgent" and "Escalated".
   Set "Escalated" conditions to include a Profile that requires assignment to
   "Urgent". Manually assign item to "Urgent". Run engine. Item should be
   assigned to "Escalated".

4. **Profile condition no match**: Same setup but item is NOT assigned to
   "Urgent". Run engine. Item should NOT be assigned to "Escalated".

5. **Assign action fires**: Create category "Meetings" with an Assign action
   targeting "Calendar". Create item "Team meeting". Run engine. Item should be
   assigned to both "Meetings" (via implicit string) and "Calendar" (via action).

6. **Remove action is deferred**: Create category with a Remove action. Verify
   the remove is collected/returned but not applied during the walk. (The exact
   mechanism depends on your design — the point is that removes don't happen
   mid-cascade.)

7. **Already assigned — skip**: Manually assign item to category. Run engine.
   Verify no duplicate assignment is created, no actions re-fire.

8. **No match**: Create category "Sarah", create item "Buy groceries". Run
   engine. No assignment.

## What NOT to do

- **Don't implement the fixed-point loop** — that's T018. Your `process_item`
  does a single pass over the hierarchy. T018 wraps it in a loop.
- **Don't implement subsumption** — that's T019. When you assign to a child,
  don't walk up the parent chain yet.
- **Don't implement mutual exclusion** — that's T020. When you assign to a
  child of an exclusive parent, don't unassign siblings yet.
- **Don't implement `evaluate_all_items`** — that's T021. You're building the
  single-item processor.
- **Don't wire into store create/update** — that's T022. The engine is called
  explicitly in tests, not triggered automatically.
- **Don't evaluate `virtual_include`/`virtual_exclude`/`text_search`** on
  Profile conditions — those Query fields are for Phase 4 (View queries).

## Workflow

Follow `AGENTS.md`. Issue ID: `bd-1c5`.

```bash
# Claim on main:
#   br update bd-1c5 --status in_progress
#   br comments add bd-1c5 "Claimed <date>. Plan: <your approach>"
#   # (legacy beads reference removed) -m "br sync: Claim bd-1c5"

git checkout -b task/t017-rule-engine

# Implement in crates/agenda-core/src/engine.rs
# Run: cargo test -p agenda-core
# Run: cargo clippy -p agenda-core
# Commit on branch

# Merge and close happen on main per AGENTS.md
```

## Definition of done

- [ ] Engine can process a single item against the full category hierarchy
- [ ] ImplicitString and Profile conditions both evaluate correctly
- [ ] Assign actions fire and create new assignments
- [ ] Remove actions are deferred (not applied mid-cascade)
- [ ] Existing assignments are skipped (idempotent)
- [ ] All tests pass — `cargo test -p agenda-core`
- [ ] `cargo clippy -p agenda-core` clean
- [ ] Changes limited to `crates/agenda-core/src/engine.rs` (and `lib.rs`
  only if module re-exports are needed)
