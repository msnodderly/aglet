# Task: T019 — Subsumption

## Context

Subsumption is the rule that being assigned to a child category implies
assignment to all its ancestors. If "Project Alpha" is a child of "Projects",
assigning an item to "Project Alpha" should also create assignments to
"Projects" (and any higher ancestors).

This is bookkeeping, not classification. Subsumption assignments exist so
that a View filtering by "Projects" will show items assigned to any of its
children. Without subsumption, a View for "Projects" would only show items
assigned directly to "Projects" — not items in "Project Alpha" or "Project
Beta."

Subsumption assignments are **not** condition matches. They should not
trigger action firing or count as "new assignments" for fixed-point loop
purposes. They are silent, additive records that maintain the ancestor
invariant.

## What to read

1. `spec/mvp-spec.md` §2.2 — "Subsumption: Assigned to child → implicitly
   assigned to all ancestors."
2. `crates/agenda-core/src/engine.rs` — the current engine. Find where
   assignments are made (`assign_if_unassigned`) and understand when
   subsumption should trigger.
3. `crates/agenda-core/src/model.rs` — `Category.parent`, `Category.children`,
   `AssignmentSource::Subsumption`
4. `spec/design-decisions.md` §3 (Sticky assignments) — subsumption
   assignments are also sticky.

## What to build

**File**: `crates/agenda-core/src/engine.rs` (extend existing code)

### The subsumption hook

After any successful assignment (AutoMatch, Action, or even Manual if
the engine processes it), walk up the parent chain from the assigned
category and create an assignment for each ancestor with
`source: Subsumption`.

**Key behaviors:**

- Walk up via `category.parent` until `None` (root reached).
- For each ancestor, create an assignment with `source: Subsumption` and
  an appropriate `origin` (e.g., `"subsumption:<child-category-name>"`).
- Skip ancestors the item is already assigned to (sticky — don't overwrite
  an existing assignment with a Subsumption one).
- Subsumption assignments do **not** count as "new assignments" for the
  fixed-point loop. They should not trigger another pass. The pass's
  `new_assignments` set should not include subsumption-created assignments.
- Subsumption assignments do **not** fire the ancestor category's actions.
  If "Projects" has an Assign action, it should not fire when a subsumption
  assignment is created for "Projects" — only when an item actually matches
  "Projects" by condition.

**Finding ancestors:** The `categories` slice from `get_hierarchy()` has
`parent: Option<CategoryId>` on each category. You'll need to look up
categories by ID to walk the chain. Consider building a
`HashMap<CategoryId, &Category>` from the categories slice at the start
of the pass (or at `process_item` level) if one doesn't already exist.

### Where to hook in

Subsumption should run after every successful assignment, regardless of
source (AutoMatch or Action). Look at the call sites of
`assign_if_unassigned` — after each one returns `Ok(true)`, walk the
parent chain. Alternatively, integrate subsumption into
`assign_if_unassigned` itself, though this couples the assignment gate
to hierarchy awareness.

The choice is yours — both approaches work. The key constraint is that
subsumption assignments must not trigger actions or count as new
assignments for the loop.

## Tests to write

1. **Basic subsumption**: Create "Projects" → child "Project Alpha". Assign
   item to "Project Alpha" (via text match). Verify item is also assigned to
   "Projects" with `source: Subsumption`.

2. **Multi-level subsumption**: Create "Work" → "Projects" → "Project Alpha".
   Item matches "Project Alpha". Verify assignments to both "Projects" AND
   "Work" with `source: Subsumption`.

3. **No duplicate subsumption**: Item is already assigned to "Projects"
   (manually). Item matches "Project Alpha". Verify "Projects" assignment
   is NOT overwritten — it keeps its original source (Manual), not
   Subsumption.

4. **Subsumption doesn't fire actions**: "Projects" has an Assign action
   targeting "Dashboard". Item matches "Project Alpha". Subsumption assigns
   to "Projects". Verify item is NOT assigned to "Dashboard" — subsumption
   didn't fire "Projects" actions.

5. **Subsumption doesn't trigger extra passes**: Verify that subsumption
   assignments alone don't cause the fixed-point loop to run additional
   passes. A single-pass item should still converge in one pass even with
   subsumption creating ancestor assignments.

6. **Action-triggered subsumption**: Category "Meetings" matches text, has
   Assign action → "Calendar" (child of "Events"). Verify: item assigned to
   "Meetings", "Calendar" (via action), and "Events" (via subsumption of
   "Calendar").

## What NOT to do

- **Don't implement mutual exclusion** — T020. Don't check `is_exclusive`.
- **Don't fire actions for subsumption assignments** — subsumption is
  bookkeeping, not classification.
- **Don't count subsumption as new assignments** — the loop should not
  re-run because of subsumption.
- **Don't overwrite existing assignments** — if an item is already assigned
  to an ancestor by a different source, leave it.

## Workflow

Follow `AGENTS.md`. Issue ID: `bd-1e2`.

```bash
git checkout -b task/t019-subsumption
# Extend crates/agenda-core/src/engine.rs
# cargo test -p agenda-core && cargo clippy -p agenda-core
```

## Definition of done

- [ ] Assigning to a child creates Subsumption assignments for all ancestors
- [ ] Subsumption works for both AutoMatch and Action assignments
- [ ] Existing assignments are not overwritten by subsumption
- [ ] Subsumption assignments don't fire actions
- [ ] Subsumption assignments don't count as new for fixed-point loop
- [ ] All tests pass — `cargo test -p agenda-core`
- [ ] `cargo clippy -p agenda-core` clean
- [ ] Changes limited to `crates/agenda-core/src/engine.rs`
