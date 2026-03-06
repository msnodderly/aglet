# Implementation Plan: `4d156d1e` Multi-Select in TUI

## Goal

Implement transient multi-select in TUI Normal mode so users can select several
items and run batch actions without repeating the same workflow item-by-item.

Primary actions in scope:

- `Space`: toggle selected state on focused item
- `a`: batch category assign/edit
- `b` / `B`: batch link actions
- `x`: batch delete
- `Esc`: clear selection

## Status

- Branch: `codex/4d156d1e-phase0-selection`
- Worktree: `/Users/mds/src/aglet-4d156d1e-phase0`
- Current merged base includes local `main` updates through `4f5a7a4`

## Progress

### Completed

- Phase 0 foundation completed in commit `ab272d9`
- Fresh `origin/main` merged cleanly in commit `81e13e5`
- Local `main` merged cleanly in commit `4e961bd`
- Marker/header cleanup completed on top of merged branch:
  - focused rows keep `>`
  - selected but unfocused rows show `+`
  - header no longer repeats `sel:N`
- Phase A1 fast-path batch assign completed in commit `c87052d`

### Phase 0 Delivered

- Added transient selection state to TUI Normal mode
- `Space` toggles selection on the focused item
- `Esc` clears selection before search/global-search behavior
- Selection clears on view switch
- Selection is pruned on refresh when selected items disappear
- Board render and footer hints now expose active selection state
- Focused `agenda-tui` tests added for selection lifecycle and visibility

### Phase A1 Delivered

- With active selection, `a` can batch-apply one resolved category to the full
  selected set
- Typed category entry resolves or creates the category once, then applies it
  to every selected item
- Existing single-item picker flow remains intact when no selection exists

### Active Work

- Reconcile Phase A1 with the intended `a` UX:
  - with active selection, users must still be able to navigate the category
    picker
  - picker `Space` must apply to the selected set rather than only the focused
    item
- Finish Phase X batch delete:
  - with active selection, `x` should open delete confirm for the selected set
  - `y` should delete the selected set in one pass
  - `Esc` should cancel without clearing selection

## Delivery Strategy

Get to a visible demo as quickly as possible, then complete the harder modal
integration work.

Fastest useful demo:

1. Add transient selection state.
2. Make `Space` toggle selection in Normal mode.
3. Render selection clearly and show selection-specific footer hints.
4. Make `a` support the simplest batch path first.

That slice proves the core architecture before touching delete confirmation and
link wizard semantics.

## Constraints and Decisions

- Selection is transient and session-scoped.
- Selection is keyed by `ItemId`, not by slot index or row position.
- Existing single-item behavior must remain unchanged when no items are selected.
- View switch clears selection.
- Refresh prunes deleted or no-longer-visible selected item IDs.
- Successful batch actions clear selection only when that clearly improves the
  flow.
- Out of scope: persistent saved selections, pairwise linking selected items to
  each other, batch done toggle, batch lane moves.

## Key Files

- `/Users/mds/src/aglet-4d156d1e-phase0/crates/agenda-tui/src/app.rs`
- `/Users/mds/src/aglet-4d156d1e-phase0/crates/agenda-tui/src/lib.rs`
- `/Users/mds/src/aglet-4d156d1e-phase0/crates/agenda-tui/src/modes/board.rs`
- `/Users/mds/src/aglet-4d156d1e-phase0/crates/agenda-tui/src/modes/view_edit.rs`
- `/Users/mds/src/aglet-4d156d1e-phase0/crates/agenda-tui/src/render/mod.rs`

## Phase 0: Selection Foundation

Purpose: establish the selection model once, so all action-specific phases can
reuse it.

### Exit Criteria

- User can select multiple items and see which ones are selected.
- Selection survives movement and disappears on `Esc` or view switch.

## Phase A1: Fast Demo for Batch Assign

Purpose: reach a working demo as early as possible with the least new modal
complexity.

### Scope

Support the quickest batch-assign path first:

- If one or more items are selected, `a` can batch-apply one resolved category
  to all selected items.
- Existing exact-match / single-visible-match / create-new logic stays intact.

### Demo Outcome

1. Select 2-3 items with `Space`.
2. Press `a`.
3. Type a category name and press `Enter`.
4. All selected items receive that category.

## Phase A2: Full Batch Category Picker

Purpose: complete the intended category UX for `a`.

### Scope

Bring the existing picker to batch parity with tri-state rows:

- `all assigned`
- `none assigned`
- `mixed`

`Space` behavior:

- `none assigned` -> assign to all selected items
- `all assigned` -> remove from all selected items
- `mixed` -> assign to all selected items

### Exit Criteria

- With active selection, `a` opens a navigable picker rather than forcing
  typed-input-only mode.
- Batch assign UI is implementation-complete relative to the issue scope.

## Phase X: Batch Delete

Purpose: make `x` useful for rapid cleanup after selection is in place.

### Demo Outcome

1. Select 2-3 items with `Space`.
2. Press `x`.
3. Press `Esc` to verify cancel keeps selection.
4. Press `x` again.
5. Press `y`.
6. All selected items are deleted in one confirm step.

## Phase B1: Fast Demo for Batch Link

Purpose: prove that batch actions can drive linking without implementing full
pairwise or wizard-complete semantics immediately.

### Demo Outcome

1. Select 2 items.
2. Press `b` or `B`.
3. Choose one target item.
4. Confirm.
5. Both selected items receive the chosen link relation.

## Phase B2: Full Batch Link Wizard

Purpose: complete the link workflow promised by the issue.

## Recommended Order From Here

1. Keep the branch green after the `main` merge.
2. Fix `a` so batch mode can still use the picker.
3. Commit Phase X batch delete.
4. Implement minimal batch linking for the next demo.
5. Return to full tri-state assign picker behavior.

## Notes

- Keep single-item flows unchanged when no selection exists.
- Prefer explicit batch helpers over scattered selection-condition branching.
- Preserve the ability to demonstrate the simplest working batch flow at each
  phase boundary.
