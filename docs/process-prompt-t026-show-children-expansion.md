# Task: T026 — show_children Section Expansion

## Context

When a section has `show_children = true` and its criteria is a single
category include, the View resolver should auto-generate subsections for
each direct child of that category. This saves users from manually defining
a section per project, per status, etc.

Example: A "Projects" section with `show_children = true` and criteria
`include: {Projects}` auto-expands into:

```
▸ Projects
    ▸ Project Alpha
      Item 1...
    ▸ Project Beta
      Item 2...
    ▸ Unmatched
      Item 3 (assigned to Projects but no specific child)
```

This is a convenience feature that makes the category hierarchy visible
in Views without manual section configuration.

## What to read

1. `spec/mvp-spec.md` §2.6 — "show_children: When a section's criteria is
   a single category include, auto-generates subsections for each direct
   child. One level only. Uses the category's stored child order. Items
   matching parent but no child go to unmatched. Inherits parent section's
   on_insert_assign + child category."
2. `spec/design-decisions.md` §15 — Non-exclusive section membership.
3. `crates/agenda-core/src/query.rs` — `resolve_view`, `ViewResult`,
   `ViewSectionResult`. The current resolver ignores `show_children`. You
   need to extend it.
4. `crates/agenda-core/src/model.rs` — `Section.show_children`,
   `Category.children`, `Category.id`.
5. `crates/agenda-core/src/store.rs` — `get_hierarchy()` returns categories
   with `children` populated.

## What to build

**File**: `crates/agenda-core/src/query.rs` (extend existing code)

### Expansion logic

When processing sections in `resolve_view`, check if a section has
`show_children = true`. If so, and if the section's criteria has exactly
one entry in `include` (and nothing else — empty exclude, empty virtual
fields, no text_search), expand it into subsections.

**Expansion steps:**

1. Identify the parent category from the section's single `include` ID.
2. Look up the parent category to get its `children` list (ordered).
3. For each child, create a subsection with:
   - Title: the child category's name
   - Criteria: `include: {child_category_id}` (items assigned to that child)
   - Items: items from the parent section that are also assigned to this child
4. Create an unmatched subsection for items assigned to the parent but
   none of its children.

**One level only.** Even if a child also has children, don't recurse.
Subsections are always leaf groups.

**Category lookup:** The resolver needs access to the category hierarchy
to look up children. This means `resolve_view` needs a way to get
categories — either accept `&[Category]` as an additional parameter, or
accept a `&Store`. Extending the function signature is acceptable.

### Result type changes

The current `ViewSectionResult` is flat. You need to represent subsections.
Options:
- Add a `subsections: Option<Vec<ViewSectionResult>>` field
- Use a separate `ViewSubsectionResult` type
- Nest `ViewSectionResult` recursively (one level only in practice)

Pick whichever is cleanest. The TUI needs to know whether a section has
subsections so it can render the nested structure.

### on_insert_assign inheritance

The spec says expanded subsections inherit the parent section's
`on_insert_assign` plus the child category. This is for edit-through
(T027) — when inserting an item into a child subsection, it gets assigned
to both the parent section's categories and the specific child. You don't
need to implement the edit-through behavior, but make sure the subsection
data includes enough information for T027 to do so (e.g., store the
effective `on_insert_assign` set on each subsection).

## Tests to write

1. **Basic expansion**: Section with `show_children = true`, single include
   for category with 2 children. Verify 2 subsections + unmatched created.

2. **Child order preserved**: Children appear in subsections in the same
   order as `category.children`.

3. **Unmatched subsection**: Item assigned to parent but no child appears
   in the unmatched subsection, not in any child subsection.

4. **No expansion when show_children false**: Section with
   `show_children = false` behaves as a normal flat section.

5. **No expansion when criteria isn't single include**: Section with
   `show_children = true` but criteria has two includes, or has an exclude,
   or has text_search — treated as a normal section (no expansion).

6. **One level only**: Parent has children, children have grandchildren.
   Only children become subsections, not grandchildren.

7. **Empty children**: Category has no children. Section expands to just
   an unmatched subsection containing all items.

8. **Items in multiple child subsections**: Item assigned to two children
   of the parent appears in both subsections (non-exclusive, consistent
   with §15).

## What NOT to do

- **Don't implement edit-through** — that's T027. Just make sure the
  data is there for T027 to use.
- **Don't recurse** — one level of expansion only.
- **Don't expand when criteria is complex** — only single-include,
  otherwise-empty criteria triggers expansion.

## Workflow

Follow `AGENTS.md`. Issue ID: `bd-yo5`.

```bash
git checkout -b task/t026-show-children
# Extend crates/agenda-core/src/query.rs
# cargo test -p agenda-core && cargo clippy -p agenda-core
```

## Definition of done

- [ ] Sections with `show_children = true` and single-include criteria expand
- [ ] Subsections created for each direct child in order
- [ ] Unmatched subsection for items in parent but no child
- [ ] No expansion when criteria is complex or show_children is false
- [ ] One level only — no recursion into grandchildren
- [ ] Subsection data includes enough for T027 edit-through
- [ ] All tests pass — `cargo test -p agenda-core`
- [ ] `cargo clippy -p agenda-core` clean
