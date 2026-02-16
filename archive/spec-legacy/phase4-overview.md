# Phase 4: Query Evaluator & Views

## The big picture

Phase 3 built the engine — items get classified into categories automatically.
But categories alone are just labels. Users can manually assign items to
categories at any time — and often will — but the primary *surface* for
seeing and organizing items is **Views**.

A View is a saved, dynamic, editable window into the data. It answers
questions like "show me all items assigned to Project Alpha, grouped by
status" or "show me everything due this week." Views are how Agenda
transforms a flat pile of items into structured, actionable lists.

Phase 4 makes Views work.

## What the user experiences

### Before Phase 4

The user can create items and categories. The engine assigns items to
categories. But there's no way to *see* those assignments in context.
`store.list_items()` returns a flat list. The data model has Views, Sections,
and Queries defined and persisted — but nothing evaluates them.

### After Phase 4

The user defines a View like:

```
View: "My Week"
  Criteria: include {Work}
  Sections:
    - "Overdue"   → virtual_include {Overdue}
    - "Today"     → virtual_include {Today}
    - "This Week" → virtual_include {ThisWeek}
    - "Unassigned" (show_unmatched = true)
```

And gets back a structured result:

```
My Week
  ▸ Overdue
    Call Sarah about the proposal          Feb 12
    Follow up on Project Alpha review      Feb 10
  ▸ Today
    Team standup                           Feb 15
    Review PR #42                          Feb 15
  ▸ This Week
    Submit quarterly report                Feb 18
  ▸ Unassigned
    Research new deployment options        —
```

Items appear in the right sections based on their category assignments and
date buckets. The grouping is live — as items are created, classified, or
updated, the View reflects the current state.

### Edit-through: the Agenda magic

The original Lotus Agenda's breakthrough was that Views aren't read-only
projections. They're *editable surfaces* where interacting with items in
context implicitly changes their category assignments.

The key insight is bidirectionality. Views are *derived from* assignments
(an item appears in a section because of its categories), but the
relationship works in reverse too: inserting an item into a section assigns
it to that section's categories. Removing it from a section unassigns.
The user doesn't need to think about category assignments at all — they
just organize items where they make sense, and the data model updates
to match.

This works alongside direct manual assignment (the `a` key, or any API
call). Edit-through is one workflow for changing assignments — the one
that feels most natural when you're already looking at a View. But users
can always assign categories directly, and the engine cascades either way.

Phase 4 builds the core library functions for this. The TUI (Phase 7-8)
will provide the visual surface, but the edit-through *logic* lives here
in agenda-core.

## What gets built

### T023 — WhenBucket resolution

Pure date math. Given an item's `when_date` and today's date, determine
which bucket it belongs to: Overdue, Today, Tomorrow, ThisWeek, NextWeek,
ThisMonth, Future, or NoDate.

These buckets are virtual — computed at query time, not stored. An item
that was "Tomorrow" yesterday is "Today" today. This is what makes the When
category feel alive.

### T024 — Query evaluator

The core filter. Given a `Query` (include categories, exclude categories,
When buckets, text search) and a list of items, return the items that match
**all** criteria.

This is the foundation that Views and Sections build on. Every View has a
top-level Query that determines which items are "in" the View. Every Section
has its own Query that further filters within the View.

### T025 — View resolver

The grouping layer. Takes a View definition and produces a structured result
with items grouped into their sections. Handles the "unmatched" bucket for
items that match the View but don't fit any explicit section.

This is what the TUI will call to render its grid. The resolver evaluates
the View's criteria, then evaluates each Section's criteria against the
matching items, and returns an ordered list of groups.

### T026 — show_children expansion

A convenience feature. When a section's criteria is a single category, and
`show_children = true`, the resolver auto-generates subsections for each
direct child category. One level deep.

Example: A section with criteria `include: {Projects}` and
`show_children = true` auto-generates:

```
▸ Projects
    ▸ Project Alpha
      Item 1...
    ▸ Project Beta
      Item 2...
    ▸ Unmatched
      Item 3 (assigned to Projects but no specific child)
```

This saves users from manually defining a section for every project.

### T027 — Edit-through logic

The assignment side-effects that make Views editable:

- **Insert in section**: Item gets assigned to the section's
  `on_insert_assign` categories plus the view's `criteria.include`
  categories.
- **Remove from section**: Item gets unassigned from the section's
  `on_remove_unassign` categories.
- **Remove from view**: Item gets unassigned from the view's
  `remove_from_view_unassign` categories.

Each operation triggers `process_item` so the engine can cascade — a
section insert might satisfy a Profile condition, triggering further
assignments.

## Dependency chain

```
T023 (WhenBucket)
  └→ T024 (Query evaluator)
       └→ T025 (View resolver)
            ├→ T026 (show_children)    ← parallel
            └→ T027 (edit-through)     ← parallel
```

T026 and T027 are independent and can be implemented simultaneously.

## Phase checkpoint

When Phase 4 is complete:

- Views correctly filter items by category assignments, When buckets, and
  text search
- Items are grouped into sections with proper ordering
- show_children auto-generates subsections from the category hierarchy
- Unmatched items appear in a catch-all section when enabled
- Edit-through operations (insert, remove from section, remove from view)
  change category assignments as side effects
- When buckets resolve dynamically based on the current date
- All of this is library code in agenda-core — no UI yet

## What's next

Phase 5 (Date Parsing) and Phase 4 are independent — they can run in
parallel. Phase 5 gives items their `when_date` values; Phase 4 gives
Views the ability to filter by them.

Phase 6 (CLI) needs both Phase 4 and Phase 5 to be complete — `agenda list`
uses the View resolver, and `agenda add` uses the date parser.

Phase 7 (TUI Core) needs Phase 4 — it renders Views using the resolver
built here.
