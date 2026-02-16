# Task: T021 — Retroactive Assignment

## Context

When a user creates a new category named "Sarah", every item containing "Sarah"
should instantly be assigned to it. This is the "aha moment" — the user types a
category name and existing items reorganize themselves.

The current engine has `process_item`, which evaluates one item against the full
category hierarchy. Retroactive assignment is the inverse: evaluate one category
against all items. You'll implement `evaluate_all_items(category_id)` which
iterates over every item in the store and runs `process_item` for each.

This function is the enabler for T022, where creating a category or changing its
conditions triggers retroactive classification.

## What to read

1. `spec/mvp-spec.md` §2.4, steps 1-2 — "Item created or modified → enters
   processing queue." Retroactive assignment is the category-side equivalent.
2. `crates/agenda-core/src/engine.rs` — the current engine. Understand
   `process_item` and its return type `ProcessItemResult`.
3. `crates/agenda-core/src/store.rs` — `list_items()` returns all items.
   `get_hierarchy()` returns all categories.
4. `crates/agenda-core/src/model.rs` — `Item`, `Category`, `CategoryId`

## What to build

**File**: `crates/agenda-core/src/engine.rs` (extend existing code)

### `evaluate_all_items`

A public function that takes a store, a classifier, and a category ID. It
iterates over all items and runs the engine for each one.

**Key behaviors:**

- Fetch all items from the store (via `list_items`).
- For each item, call `process_item`. The engine's existing logic handles
  condition evaluation, action firing, subsumption, mutual exclusion, and the
  fixed-point loop. You don't need to reimplement any of that.
- Collect results — the caller needs to know what happened (how many items were
  affected, any errors). Design a return type that makes sense.
- If `process_item` returns an error for one item (e.g., pass cap exceeded),
  decide how to handle it. Options: stop and propagate the error, or continue
  with remaining items and collect errors. Either approach is acceptable for
  MVP — pick one and document it in a comment.
- The function does NOT need to filter items by whether they'd match the target
  category. Running `process_item` on all items is correct because:
  - The category might have Profile conditions that depend on other categories.
  - Actions on the category might assign to other categories.
  - It's simpler and the MVP dataset is small enough that full iteration is fine.

**Performance note:** This is O(items × categories) per call. That's acceptable
for MVP. If performance becomes an issue later, the optimization is to filter
items to only those that could possibly match (e.g., text search for implicit
string categories). Don't optimize now.

### How your code will be used

T022 will wire this function into store operations:
- `store.create_category()` → calls `evaluate_all_items(new_category_id)`
- `store.update_category()` → calls `evaluate_all_items(updated_category_id)`
  (if conditions or name changed)

The function is called from outside the engine, so it needs to be `pub`.

## Tests to write

1. **Basic retroactive match**: Create several items (some containing "Sarah",
   some not). Then create category "Sarah" with `enable_implicit_string = true`.
   Call `evaluate_all_items`. Verify items containing "Sarah" are assigned,
   others are not.

2. **No double-assignment**: Create item "Sarah's meeting". Create category
   "Sarah". Run `process_item` on the item (simulating initial processing).
   Then call `evaluate_all_items`. Verify the item is assigned exactly once —
   no duplicate, no error.

3. **Retroactive with actions**: Create category "Meetings" with Assign action
   targeting "Calendar". Create items containing "Meetings". Call
   `evaluate_all_items`. Verify items are assigned to both "Meetings" and
   "Calendar" (action fired).

4. **Retroactive with subsumption**: "Projects" → child "Project Alpha". Create
   items containing "Project Alpha". Call `evaluate_all_items`. Verify items
   are assigned to "Project Alpha" AND "Projects" (subsumption).

5. **Retroactive with mutual exclusion**: Exclusive parent "Status" with
   children "To Do" and "In Progress". Item is assigned to "To Do". Create
   condition/action that would assign to "In Progress". Call
   `evaluate_all_items`. Verify exclusion: item ends up in "In Progress",
   not "To Do".

6. **Empty store**: Call `evaluate_all_items` with no items in the store.
   Verify it returns successfully with no assignments.

7. **Idempotent re-run**: Call `evaluate_all_items` twice for the same
   category. Second call should produce no new assignments.

## What NOT to do

- **Don't wire into store operations** — that's T022. This function is called
  explicitly, not triggered automatically.
- **Don't reimplement engine logic** — use `process_item` as-is. The whole
  point is that the existing engine handles the complexity.
- **Don't filter items before processing** — run all items through the engine.
  Premature filtering would miss Profile conditions and action cascades.
- **Don't add parallelism or batching** — sequential iteration is fine for MVP.

## Workflow

Follow `AGENTS.md`. Issue ID: `bd-288`.

```bash
git checkout -b task/t021-retroactive-assignment
# Extend crates/agenda-core/src/engine.rs
# cargo test -p agenda-core && cargo clippy -p agenda-core
```

## Definition of done

- [ ] `evaluate_all_items` is a public function in `engine.rs`
- [ ] It runs `process_item` for every item in the store
- [ ] Results are collected and returned to the caller
- [ ] Error handling strategy is documented
- [ ] Idempotent — safe to call multiple times
- [ ] All tests pass — `cargo test -p agenda-core`
- [ ] `cargo clippy -p agenda-core` clean
- [ ] Changes limited to `crates/agenda-core/src/engine.rs`
