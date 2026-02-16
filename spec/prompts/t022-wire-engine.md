# Task: T022 — Wire Engine into Store Operations

## Context

Right now the engine exists as standalone functions — `process_item` and
`evaluate_all_items` must be called explicitly by test code. In the real
application, they need to fire automatically:

- Create or update an **item** → run `process_item` to classify it.
- Create a **category** → run `evaluate_all_items` to retroactively classify
  existing items.
- Update a **category** (name or conditions changed) → run `evaluate_all_items`
  to reclassify.

This is the integration point where the engine becomes automatic. After this
task, the core loop is complete: type an item, it gets organized; create a
category, existing items get organized.

## What to read

1. `spec/mvp-spec.md` §2.4 — "Item created or modified → enters processing
   queue." This task implements that queue (synchronously for MVP).
2. `crates/agenda-core/src/engine.rs` — `process_item` and
   `evaluate_all_items` (T021).
3. `crates/agenda-core/src/store.rs` — `create_item`, `update_item`,
   `create_category`, `update_category`. These are the operations that should
   trigger the engine.
4. `crates/agenda-core/src/matcher.rs` — the `Classifier` trait. The engine
   needs a classifier instance to do its work.

## What to build

**Files**: Likely a new file or module, plus modifications to existing code.
The exact architecture is up to you — read the options below and pick one.

### The integration problem

The Store doesn't know about the Classifier or the engine. The engine takes
`&Store` and `&dyn Classifier` as parameters. You need to create a layer that
holds both and orchestrates them.

**Option A — Wrapper struct**: Create an `Agenda` (or `Engine`, or `Core`)
struct that owns or borrows a `Store` and a `Box<dyn Classifier>`. It exposes
methods like `create_item`, `update_item`, `create_category` that delegate to
the store and then call the engine. Callers use this struct instead of the
store directly for operations that need engine processing.

**Option B — Callbacks on Store**: Add an optional callback/hook mechanism to
Store that fires after mutations. The store calls the hook, and the hook runs
the engine. This is more decoupled but adds complexity.

**Option C — Free functions**: Keep it simple — expose `create_item_with_engine`
and similar free functions that combine the store call and engine call. Less
elegant but minimal plumbing.

Pick whichever approach fits the codebase best. Option A is the most natural
for Rust and scales well, but the others are acceptable for MVP.

### Trigger points

Regardless of architecture, these are the trigger points:

1. **Item created** → `process_item(item_id)` to classify the new item.
2. **Item updated** (text changed) → `process_item(item_id)` to reclassify.
   Note: the engine is additive (sticky assignments), so reclassification only
   adds new matches — it won't remove assignments that no longer match.
3. **Category created** → `evaluate_all_items(category_id)` to retroactively
   classify existing items against the new category.
4. **Category updated** (name or conditions changed) →
   `evaluate_all_items(category_id)` to reclassify against updated rules.

**What about category deletion?** When a category is deleted, its assignments
are cleaned up by the store's `ON DELETE CASCADE`. The engine doesn't need to
run — there's nothing to classify against a deleted category.

**What about manual assignment (T022)?** Manual assignment (`source: Manual`)
should also trigger `process_item` on the assigned item, because the new
assignment might satisfy a Profile condition on another category, triggering a
cascade. This is the "mark as Urgent → triggers Escalated rule" flow from
the design decisions.

### Error propagation

If the engine returns an error (e.g., pass cap exceeded), the integration
layer should propagate it to the caller. The store mutation has already
happened (the item/category was created/updated), but the caller should know
that classification failed so they can surface the issue.

For MVP, don't roll back the store mutation on engine failure — the item or
category should still exist, even if classification didn't complete. The
engine's savepoint mechanism (already in `process_item`) handles atomicity
of the engine's own work.

## Tests to write

1. **Create item triggers classification**: Create category "Sarah". Then
   create item "Sarah's meeting" via the integration layer. Verify the item
   is automatically assigned to "Sarah" — no explicit engine call needed.

2. **Update item triggers reclassification**: Create category "Urgent". Create
   item "normal task". Verify not assigned to "Urgent". Update item text to
   "Urgent task". Verify now assigned to "Urgent".

3. **Create category triggers retroactive**: Create items "Sarah's meeting"
   and "Bob's lunch". Then create category "Sarah". Verify "Sarah's meeting"
   is assigned, "Bob's lunch" is not.

4. **Update category triggers reclassification**: Create category "Foo" with
   `enable_implicit_string = true`. Create item "meeting with Foo". Verify
   assigned. Rename category to "Bar". Run `evaluate_all_items`. Create item
   "meeting with Bar". Verify new item matches. (Note: existing assignment to
   old name stays — sticky.)

5. **Manual assignment triggers cascade**: Create "Urgent" and "Escalated"
   categories. "Escalated" has Profile condition: include "Urgent". Manually
   assign item to "Urgent" via the integration layer. Verify item is
   automatically assigned to "Escalated" via the cascade.

6. **Engine error doesn't prevent store mutation**: Create a pathological
   rule configuration that exceeds 10 passes. Create an item that triggers it.
   Verify the item exists in the store (mutation succeeded) even though the
   engine returned an error.

7. **End-to-end workflow**: Create category "Meetings" with Assign action →
   "Calendar". Create item "Team meetings tomorrow". Verify: item assigned to
   "Meetings" (text match), "Calendar" (action), and any parent categories
   (subsumption) — all automatically, no manual engine calls.

## What NOT to do

- **Don't modify the engine functions** — `process_item` and
  `evaluate_all_items` should remain as-is. This task is about wiring, not
  engine changes.
- **Don't add async/queue infrastructure** — synchronous execution is correct
  for MVP. The engine runs inline with the store operation.
- **Don't optimize `evaluate_all_items` calls** — if creating a category
  triggers a full item scan, that's fine. Don't add change-detection or
  dirty-tracking yet.
- **Don't implement View integration** — that's Phase 4. Views are not
  involved in this wiring.

## Workflow

Follow `AGENTS.md`. Issue ID: `bd-3tj`.

```bash
git checkout -b task/t022-wire-engine
# Implement integration layer
# cargo test -p agenda-core && cargo clippy -p agenda-core
```

## Definition of done

- [ ] Creating an item automatically runs classification
- [ ] Updating an item's text automatically runs reclassification
- [ ] Creating a category automatically runs retroactive classification
- [ ] Updating a category (name/conditions) runs retroactive classification
- [ ] Manual assignment triggers `process_item` for cascading
- [ ] Engine errors are propagated (store mutation still succeeds)
- [ ] All tests pass — `cargo test -p agenda-core`
- [ ] `cargo clippy -p agenda-core` clean
