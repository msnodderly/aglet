# Profile Conditions: CLI + TUI Implementation Plan

## Context

Profile conditions are fully implemented in the engine (`engine.rs`) and data model (`model.rs`).
Users currently cannot create or manage profile conditions through CLI or TUI — only through
direct database manipulation. This plan adds the missing user-facing layer.

The Beeswax/Agenda article states profile conditions are "identical in form to the view criteria."
The view criteria picker in `crates/agenda-tui/src/modes/view_edit/details.rs` already edits
`Query` objects with AND/NOT/OR logic.
We reuse this same pattern for category profile condition editing.

## Design Decisions

1. **Include + Exclude from day 1** — AND/NOT criteria in the editor
2. **Multiple conditions per category, OR'd** — if ANY condition matches, the item is assigned
3. **Dialog/modal layout** — criteria editor opens as a modal in the TUI
4. **Reuse view criteria picker pattern** from `modes/view_edit/details.rs`

## What Exists (Backend)

- `Condition::Profile { criteria: Box<Query> }` in `model.rs`
- `profile_matches()` in `engine.rs` with AND/NOT/OR semantics
- Fixed-point cascade (max 10 passes) in `engine.rs`
- `category show` CLI displays conditions (read-only)
- Persistence via `conditions_json` in SQLite `categories` table
- Test helpers: `category_with_profile()` in engine tests

## What's Missing (This Work)

### CLI Commands
- `category add-condition <name> --and <cat>... [--not <cat>...] [--or <cat>...]`
- `category remove-condition <name> <index>`
- `category list-conditions <name>` (or fold into `category show`)

### TUI: Profile Condition Editing in Category Manager
- Add condition management to category.rs mode
- Reuse criteria picker pattern from `modes/view_edit/details.rs`
- Two-level UI:
  - Level 1: List conditions on a category (summary view)
  - Level 2: Edit a condition's criteria (AND/NOT/OR category picker)

## UI Mockups

### Level 1: Condition List

```
+-- Rules: Escalated ------------------------------------+
|                                                        |
|  Auto-assign to "Escalated" when ANY rule matches:     |
|                                                        |
|  1. AND: Urgent, Project Alpha                  [x]    |
|  2. AND: P0 / NOT: Resolved                     [x]    |
|                                                        |
|  [a] Add rule                                          |
|                                                        |
+--------------------------------------------------------+
```

### Level 2: Criteria Editor (reuse view criteria picker)

```
+-- Edit Condition ---------------------------------------+
|                                                         |
|  AND (must have all):                                   |
|    Urgent                                       [x]     |
|    Project Alpha                                [x]     |
|    > add...                                             |
|                                                         |
|  NOT (must not have):                                   |
|    > add...                                             |
|                                                         |
|                           [Esc] Cancel  [Enter] Save    |
+---------------------------------------------------------+
```

## Test Cases

### Brainstormed scenarios for integration tests
1. "If Project A -> assign to Mary" — single AND criterion
2. "If Urgent AND Project Alpha -> assign to Escalated" — compound AND
3. "If Mom -> assign to High Priority" — priority boost
4. "If Bug AND Backend -> assign to Team:Infrastructure" — routing
5. "If Escalated -> assign to Notify:Manager" — cascading chain
6. "If Work AND NOT Delegated -> assign to My-Tasks" — exclude pattern
7. "If Personal AND NOT Urgent -> assign to Weekend" — deprioritize
8. Idempotency — item already in target -> no duplicate
9. Order independence — assign A then B, or B then A -> same result
10. Cascading convergence — stable in 2 passes
11. Cycle detection — A->B and B->A -> halts
12. Late satisfaction — second assignment triggers rule
13. Provenance — origin="profile:CategoryName"

## TODO

- [x] 1. Add CLI `category add-condition` command
- [x] 2. Add CLI `category remove-condition` command
- [x] 3. Add profile condition display improvements to CLI `category show`
- [x] 4. Add TUI condition list view in category manager
- [x] 5. Add TUI criteria editor (reusing `modes/view_edit/details.rs` pattern)
- [x] 6. Write engine tests for brainstormed scenarios
- [x] 7. Write CLI integration tests
- [x] 8. Write TUI mode tests

## Key Files

- `crates/agenda-core/src/model.rs:167-171` — Condition enum
- `crates/agenda-core/src/engine.rs:273-321` — evaluation logic
- `crates/agenda-core/src/store.rs` — persistence
- `crates/agenda-cli/src/main.rs:288-413` — CategoryCommand enum (add variants here)
- `crates/agenda-cli/src/main.rs:1950-1979` — category show display
- `crates/agenda-tui/src/modes/category.rs` — category manager (add condition UI here)
- `crates/agenda-tui/src/modes/view_edit/details.rs` — criteria picker to reuse
