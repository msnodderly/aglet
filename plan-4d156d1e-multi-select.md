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

- With active selection, `a` opens typed category input directly
- `Enter` resolves or creates the category once, then applies it to the full
  selected set
- Successful batch assign clears selection and returns to Normal mode
- Existing single-item picker flow remains intact when no selection exists

### Active Work

- Merge latest `main` updates into the worktree branch and keep the plan in sync
- Reconcile Phase A1 with the intended `a` UX:
  - with active selection, users must still be able to use the category picker
  - batch mode needs a navigable picker rather than typed-input only
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
- Successful batch actions clear selection unless preserving it is clearly
  needed for follow-on flows.
- Out of scope: persistent saved selections, pairwise linking selected items to
  each other, batch done toggle, batch lane moves.

## Key Files

- `/Users/mds/src/aglet-4d156d1e-phase0/crates/agenda-tui/src/lib.rs`
- `/Users/mds/src/aglet-4d156d1e-phase0/crates/agenda-tui/src/modes/board.rs`
- `/Users/mds/src/aglet-4d156d1e-phase0/crates/agenda-tui/src/modes/view_edit.rs`
- `/Users/mds/src/aglet-4d156d1e-phase0/crates/agenda-tui/src/render/mod.rs`

## Phase 0: Selection Foundation

Purpose: establish the selection model once, so all action-specific phases can
reuse it.

### Work

- Add `selected_item_ids: HashSet<ItemId>` to `App`.
- Add small helpers:
  - `selected_count()`
  - `is_item_selected(item_id)`
  - `toggle_selected_item(item_id)`
  - `clear_selected_items()`
  - `selected_item_ids_in_view_order()`
  - `prune_selected_items_to_visible_slots()`
- Wire `Space` in Normal mode to toggle the focused item.
- Keep the focused cursor where it is after toggling.
- Clear selection on view switch paths.
- Prune selection after `refresh()`.
- Make `Esc` clear selection first when any selected items exist; otherwise
  preserve current search/global-search behavior.

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

Basic demo script:

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

### Work

- Add batch-aware row-state computation over the selected item set.
- Update row rendering to show tri-state markers such as `[x]`, `[ ]`, and
  `[-]`.
- Ensure exclusive-category parents still behave correctly when applying one
  child to all selected items.
- Keep `n` / `/` path available from the picker for typed assign/create.
- Update picker status text so behavior is explicit.

### Exit Criteria

- With active selection, `a` opens a navigable picker rather than forcing
  typed-input-only mode.
- Batch assign UI is implementation-complete relative to the issue scope.

## Phase X: Batch Delete

Purpose: make `x` useful for rapid cleanup after selection is in place.

### Work

- Extend `ConfirmDelete` flow to support batch delete context.
- Add transient batch-delete state:
  - selected IDs snapshot
  - item count
- Open confirmation with `Delete N selected items? y/n`.
- On confirm:
  - delete all selected items
  - refresh
  - clear selection
  - show summary counts
- On cancel:
  - restore Normal mode
  - preserve selection

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

### Scope

- Reuse the existing Link Wizard entry point from a selected set.
- Keep one anchor-target action model for the demo:
  - selected items are the source set
  - chosen target is the single destination
- Start with the most useful action first:
  - `b` batch `depends-on` / `blocked-by`, or
  - `B` batch `blocks`

### Demo Outcome

1. Select 2 items.
2. Press `b` or `B`.
3. Choose one target item.
4. Confirm.
5. Both selected items receive the chosen link relation.

## Phase B2: Full Batch Link Wizard

Purpose: complete the link workflow promised by the issue.

### Work

- Define batch semantics per action explicitly in the wizard copy.
- Prevent illegal self-links when the target is inside the selected set.
- Keep target filtering and scrolling behavior consistent with current wizard.
- Report counts for applied / skipped / failed links.

## Recommended Order From Here

1. Finalize merge state and keep the branch green.
2. Fix `a` so batch mode can still use the picker.
3. Commit Phase X batch delete.
4. Implement minimal batch linking for the next demo.
5. Return to full tri-state assign picker behavior.

## Notes

- Keep single-item flows unchanged when no selection exists.
- Prefer explicit batch helpers over scattered selection-condition branching.
- Clear selection after successful batch mutations unless a later phase proves a
  better UX.
