# Task: T027 — Edit-Through Logic

## Context

Edit-through is the mechanism that makes Views editable. When a user inserts
an item into a section, removes it from a section, or removes it from a
view, category assignments change as a side effect. The user organizes items
visually; the system updates the data model to match.

This task implements the core library functions for these operations. The
TUI (Phase 7-8) will call them in response to user actions.

## What to read

1. `spec/mvp-spec.md` §2.6 — Edit-through semantics:
   - Insert item in section → assigns `section.on_insert_assign` +
     `view.criteria.include`
   - Remove item from section → unassigns `section.on_remove_unassign`
   - Remove item from view → unassigns `view.remove_from_view_unassign`
   - Unmatched section: `on_insert_assign` = `view.criteria.include`,
     `on_remove_unassign` = `view.remove_from_view_unassign`
2. `spec/phase4-overview.md` — Edit-through as one workflow among many.
3. `spec/design-decisions.md` §11 — Manual assignment triggers the engine.
4. `crates/agenda-core/src/query.rs` — `resolve_view`, `ViewResult`,
   `ViewSectionResult`. The resolved view tells you which sections exist.
5. `crates/agenda-core/src/agenda.rs` — `Agenda` struct with
   `assign_item_manual` and `store()`. Edit-through operations will
   likely live here or call through here.
6. `crates/agenda-core/src/store.rs` — `assign_item()`, `unassign_item()`.
7. `crates/agenda-core/src/model.rs` — `Section.on_insert_assign`,
   `Section.on_remove_unassign`, `View.remove_from_view_unassign`,
   `View.criteria.include`.

## What to build

**Files**: `crates/agenda-core/src/agenda.rs` or a new module — wherever
fits the architecture best. These operations combine store mutations with
engine processing, similar to how `Agenda` wraps item/category CRUD.

### Three operations

**1. Insert in section**

When an item is inserted into a specific section of a view:
- Assign the item to every category in `section.on_insert_assign`
- Assign the item to every category in `view.criteria.include`
- After all assignments, run `process_item` to trigger cascading

The assignments are Manual source — the user chose to put the item there.

**2. Remove from section**

When an item is removed from a specific section:
- Unassign the item from every category in `section.on_remove_unassign`
- After unassignments, run `process_item` — the changed assignment set
  might trigger new Profile matches

**3. Remove from view**

When an item is removed from the view entirely:
- Unassign the item from every category in `view.remove_from_view_unassign`
- After unassignments, run `process_item`

### Unmatched section behavior

When the user inserts into or removes from the unmatched section, use the
view-level defaults:
- Insert: `on_insert_assign` = `view.criteria.include`
- Remove: `on_remove_unassign` = `view.remove_from_view_unassign`

The resolver (T025) doesn't store unmatched as a Section struct, so the
caller (TUI) will need to know to use the view-level fields. Your API
should make this clear — either handle unmatched as a special case or
document that the caller constructs the equivalent parameters.

### Engine cascading

Every edit-through operation should trigger `process_item` after the
assignments/unassignments are applied. This is critical because:
- Inserting into a section might satisfy a Profile condition
- Removing from a section might make a previously-blocked Profile no
  longer match (though sticky assignments mean nothing gets auto-removed)

The Agenda struct already has `assign_item_manual` which calls
`process_item`. You may be able to reuse it for the insert case. For
remove, you'll need to call `store.unassign_item()` followed by
`process_item`.

## Tests to write

1. **Insert assigns section + view categories**: View with
   `criteria.include: {Work}`, section with `on_insert_assign: {Urgent}`.
   Insert item into section. Verify item assigned to both Work and Urgent.

2. **Insert triggers engine cascade**: View section inserts assign to
   category that satisfies a Profile condition on another category.
   Verify the cascade fires.

3. **Remove from section unassigns**: Section with
   `on_remove_unassign: {Urgent}`. Item is assigned to Urgent. Remove
   from section. Verify Urgent unassigned.

4. **Remove from view unassigns**: View with
   `remove_from_view_unassign: {Work}`. Remove item from view. Verify
   Work unassigned.

5. **Unmatched insert uses view criteria**: Insert into unmatched section.
   Verify item gets `view.criteria.include` categories.

6. **Unmatched remove uses view-level unassign**: Remove from unmatched.
   Verify `view.remove_from_view_unassign` categories unassigned.

7. **Remove doesn't affect other assignments**: Item assigned to Work,
   Urgent, and Personal. Remove from section unassigns Urgent. Verify
   Work and Personal still assigned.

8. **Insert is idempotent for existing assignments**: Item already assigned
   to Work. Insert into section with `on_insert_assign: {Work}`. No error,
   assignment unchanged.

## What NOT to do

- **Don't modify the View resolver** — `resolve_view` is read-only.
  Edit-through is a separate write path.
- **Don't implement delete** — item deletion (the `x` key) is a store
  operation, not edit-through. Edit-through only changes assignments.
- **Don't implement move between sections** — that's a TUI-level
  composition of remove-from-section + insert-in-section. The library
  provides the primitives.

## Workflow

Follow `AGENTS.md`. Issue ID: `bd-211`.

```bash
git checkout -b task/t027-edit-through
# Implement edit-through operations
# cargo test -p agenda-core && cargo clippy -p agenda-core
```

## Definition of done

- [ ] Insert-in-section assigns on_insert_assign + view criteria include
- [ ] Remove-from-section unassigns on_remove_unassign
- [ ] Remove-from-view unassigns remove_from_view_unassign
- [ ] All operations trigger process_item for cascading
- [ ] Unmatched section uses view-level defaults
- [ ] All tests pass — `cargo test -p agenda-core`
- [ ] `cargo clippy -p agenda-core` clean
