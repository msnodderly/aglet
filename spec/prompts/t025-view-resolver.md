# Task: T025 — View Resolver

## Context

The Query evaluator (T024) filters items against a single Query. But a View
is more than one filter — it has a top-level Query (which items are "in" the
View) and multiple Sections, each with their own Query (how to group the
matching items).

The View resolver ties these together: evaluate the View's criteria to get
matching items, then evaluate each Section's criteria to group those items,
then handle the unmatched bucket for items that don't fit any section.

This is what the TUI will call to render its grid. The CLI's `agenda list
--view` will also use it.

## What to read

1. `spec/mvp-spec.md` §2.6 — View, Section, Query structs. Key behaviors:
   section membership, unmatched section, show_children (T026, not this task).
2. `spec/phase4-overview.md` — Overall phase context and the "My Week" example.
3. `crates/agenda-core/src/query.rs` — `evaluate_query` (T024) and
   `resolve_when_bucket` (T023). Build on these.
4. `crates/agenda-core/src/model.rs` — `View`, `Section`, `Query`,
   `Item`.
5. `crates/agenda-core/src/store.rs` — `list_items()`, `get_hierarchy()`.
   The resolver may need to fetch items and categories from the store,
   or the caller may provide them — design choice is yours.

## What to build

**File**: `crates/agenda-core/src/query.rs` (extend existing code)

### The resolver function

A public function that takes a View (and whatever other inputs it needs —
items, reference date, possibly a store reference) and returns a structured
result grouping items into sections.

### Result type

Design a `ViewResult` (or similar) struct that represents the resolved view.
It needs to contain:

- An ordered list of section groups, each with:
  - The section title (or a reference to the section)
  - The items in that section (references or owned)
- Optionally, the unmatched group (if `show_unmatched = true`)

The TUI and CLI will consume this struct to render output, so make it
ergonomic for iteration and display.

### Resolution logic

1. **Filter by View criteria**: Run `evaluate_query` with the View's
   `criteria` against all items. This gives the set of items "in" the View.

2. **Group by Sections**: For each Section (in order), run `evaluate_query`
   with the Section's `criteria` against the View-filtered items. Items can
   appear in multiple sections — an item matching two sections appears in
   both.

3. **Unmatched bucket**: If `view.show_unmatched = true`, collect items that
   matched the View criteria but didn't match ANY section's criteria. These
   go in the unmatched group, labeled with `view.unmatched_label`.

4. **Mutual exclusivity of explicit vs unmatched**: An item appears in
   explicit sections OR in the unmatched group, never both. If an item
   matches at least one explicit section, it does NOT appear in unmatched.

### Data flow

The resolver needs all items from the store to filter against the View
criteria. Options:
- Accept a `&[Item]` slice (caller fetches items)
- Accept a `&Store` reference (resolver fetches items)

Either is fine. If you accept a slice, the caller has more control (useful
for testing). If you accept a Store, it's more convenient for callers.
Consider offering both, or pick whichever fits better.

## Tests to write

1. **Basic view with sections**: View with criteria `include: {Work}`, two
   sections ("Urgent" and "Normal"). Items assigned to Work+Urgent go in
   first section, Work+Normal in second. Items only assigned to Work go in
   unmatched.

2. **Empty view criteria matches all**: View with empty criteria, one section.
   All items in store considered, section filters them.

3. **Item in multiple sections**: Item matches criteria for two sections.
   Verify it appears in both.

4. **Unmatched section**: View with `show_unmatched = true`. Items matching
   view but no section appear in unmatched group with correct label.

5. **No unmatched when disabled**: `show_unmatched = false`. Items matching
   view but no section are simply absent from results.

6. **Item in section not in unmatched**: Item matches both the view and
   one section. Verify it does NOT also appear in unmatched.

7. **Section order preserved**: Sections appear in results in the same
   order as defined in the View.

8. **Empty view**: View criteria matches no items. All sections empty,
   unmatched empty.

9. **View with text_search**: View criteria has text_search. Only items
   matching the text appear, then grouped by sections.

10. **View with virtual_include**: View criteria has virtual_include
    {Today}. Only items due today appear, grouped by sections.

## What NOT to do

- **Don't implement show_children** — that's T026. If a section has
  `show_children = true`, ignore it for now (treat as a normal section).
- **Don't implement edit-through** — that's T027.
- **Don't sort items within sections** — return them in the order they
  come from the query evaluator (which preserves input order).

## Workflow

Follow `AGENTS.md`. Issue ID: `bd-3d1`.

```bash
git checkout -b task/t025-view-resolver
# Extend crates/agenda-core/src/query.rs
# cargo test -p agenda-core && cargo clippy -p agenda-core
```

## Definition of done

- [ ] Public function resolves a View into grouped section results
- [ ] View criteria filters items, section criteria groups them
- [ ] Unmatched bucket collects items in view but no section (when enabled)
- [ ] Items appear in explicit sections OR unmatched, never both
- [ ] Section order preserved
- [ ] Result type is ergonomic for TUI/CLI consumption
- [ ] All tests pass — `cargo test -p agenda-core`
- [ ] `cargo clippy -p agenda-core` clean
- [ ] Changes limited to `crates/agenda-core/src/query.rs`
