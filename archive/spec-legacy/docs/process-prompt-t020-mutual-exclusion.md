# Task: T020 — Mutual Exclusion

## Context

Some categories are **exclusive** — an item can belong to at most one of
their children. Think of status categories: an item is either "To Do",
"In Progress", or "Done" — never two at once.

When a parent category has `is_exclusive = true`, assigning an item to one
of its children must first unassign the item from all sibling children. This
is "last writer wins" — the most recent assignment replaces any previous one.

This applies to all assignment paths: AutoMatch, Action, and eventually
Manual (T022). The enforcement happens at assignment time, not at evaluation
time.

## What to read

1. `spec/mvp-spec.md` §2.2 — "Mutual exclusion: If `is_exclusive`, an item
   can be in at most one child. Assigning to a new child auto-unassigns from
   siblings."
2. `crates/agenda-core/src/engine.rs` — the current engine. Find where
   assignments happen and understand the flow.
3. `crates/agenda-core/src/model.rs` — `Category.is_exclusive`,
   `Category.parent`, `Category.children`
4. `spec/design-decisions.md` §4 (Remove regardless of source) — mutual
   exclusion unassignment follows the same principle.

## What to build

**File**: `crates/agenda-core/src/engine.rs` (extend existing code)

### The exclusion check

Before assigning an item to a category, check if the category's parent
has `is_exclusive = true`. If so, unassign the item from all other children
of that parent before proceeding with the assignment.

**Key behaviors:**

- Check is on the **parent**, not the category itself. A category being
  exclusive means its *children* are mutually exclusive with each other.
- Unassign from **sibling children only** — not from the parent itself,
  not from cousins in other branches.
- The unassignment happens **immediately** (not deferred like Remove
  actions). Mutual exclusion is a structural invariant, not an action
  side-effect. Deferring it would allow an item to be in two exclusive
  siblings simultaneously during the cascade, which violates the invariant.
- Unassign regardless of assignment source — if the sibling assignment
  was Manual, AutoMatch, Action, or Subsumption, it still gets removed.
- When unassigning a sibling, also unassign its subsumption ancestors
  **if they have no other reason to exist**. This is subtle — if "Project
  Alpha" and "Project Beta" are siblings under exclusive parent "Status",
  and the item was in "Project Alpha" (with subsumption to "Status"),
  unassigning "Project Alpha" should clean up that subsumption path. But
  if "Status" also has a direct match, its assignment stays. For MVP,
  a simpler approach is acceptable: just unassign the sibling child and
  leave stale subsumption assignments. They're harmless bookkeeping and
  can be cleaned up in hardening (Phase 11).

**Finding siblings:** You need the parent category to get its `children`
list. The `categories` slice from `get_hierarchy()` has both `parent` and
`children` populated. Build a `HashMap<CategoryId, &Category>` lookup if
one doesn't already exist (T019 may have already added one).

### Where to hook in

The check should happen **before** the assignment is persisted — inside or
just before `assign_if_unassigned`. When the check finds siblings to
unassign, it calls `store.unassign_item()` for each and removes them from
the in-memory `assignments` map.

This is different from deferred Remove actions. Mutual exclusion is
immediate because it's a structural constraint, not a workflow action.

### Interaction with subsumption (T019)

If T019 is already merged, mutual exclusion unassignment should happen
before subsumption. The flow for a single assignment becomes:

1. Check exclusive parent → unassign siblings (if any)
2. Persist the assignment
3. Walk parent chain → create subsumption assignments (T019)

This ordering ensures subsumption sees the correct assignment state.

## Tests to write

1. **Basic exclusion**: Create exclusive parent "Status" with children
   "To Do" and "In Progress". Assign item to "To Do". Then assign to
   "In Progress". Verify item is assigned to "In Progress", NOT "To Do".

2. **Non-exclusive parent — no unassignment**: Create non-exclusive parent
   "Tags" with children "Urgent" and "Important". Assign item to both.
   Verify both assignments coexist.

3. **Exclusion via engine match**: Create exclusive "Priority" with children
   "High" and "Low" (both with `enable_implicit_string = true`). Create
   item "High priority and Low cost". Engine matches "High" first (depth-first).
   Then if "Low" also matches, the previous "High" assignment should be
   removed. (Note: matching order depends on hierarchy order — test should
   verify that only one child is assigned at the end.)

4. **Exclusion applies to action assignments**: Category "Workflow" matches
   text, has Assign action → "In Progress" (child of exclusive "Status").
   Item is already assigned to "To Do" (another child of "Status"). After
   engine: item is in "In Progress", NOT "To Do".

5. **Exclusion applies regardless of source**: Item is manually assigned to
   "To Do" (child of exclusive "Status"). Engine assigns to "In Progress".
   Verify "To Do" (manual) is removed.

6. **Three children — correct sibling removed**: Exclusive "Priority" has
   three children: "Low", "Medium", "High". Item is in "Low". Assign to
   "High". Verify "Low" removed, "Medium" untouched (was never assigned),
   "High" assigned.

## What NOT to do

- **Don't defer exclusion unassignment** — it's immediate, unlike
  `Action::Remove`. The structural invariant must hold at all times during
  the cascade.
- **Don't clean up stale subsumption** — for MVP, if unassigning a sibling
  leaves orphaned subsumption assignments on ancestors, that's acceptable.
  Defer cleanup to hardening.
- **Don't check `is_exclusive` on the category being assigned to** — check
  it on the **parent**. The parent being exclusive constrains its children.

## Workflow

Follow `AGENTS.md`. Issue ID: `bd-8zh`.

```bash
git checkout -b task/t020-mutual-exclusion
# Extend crates/agenda-core/src/engine.rs
# cargo test -p agenda-core && cargo clippy -p agenda-core
```

## Definition of done

- [ ] Assigning to child of exclusive parent unassigns from sibling children
- [ ] Exclusion is immediate (not deferred)
- [ ] Exclusion applies regardless of assignment source
- [ ] Non-exclusive parents don't trigger unassignment
- [ ] All tests pass — `cargo test -p agenda-core`
- [ ] `cargo clippy -p agenda-core` clean
- [ ] Changes limited to `crates/agenda-core/src/engine.rs`
