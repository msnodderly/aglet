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

## Delivery Strategy

Get to a visible demo as quickly as possible, then complete the harder modal
integration work.

Fastest useful demo:

1. Add transient selection state.
2. Make `Space` toggle selection in Normal mode.
3. Render selection clearly and show a selected-count status/footer hint.
4. Make `a` support the simplest batch path first: assign one resolved category
   to all selected items.

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

- `/Users/mds/src/aglet/crates/agenda-tui/src/lib.rs`
- `/Users/mds/src/aglet/crates/agenda-tui/src/modes/board.rs`
- `/Users/mds/src/aglet/crates/agenda-tui/src/render/mod.rs`
- `/Users/mds/src/aglet/crates/agenda-tui/src/input/mod.rs`

## Phase 0: Selection Foundation

Purpose: establish the selection model once, so all action-specific phases can
reuse it.

### Work

- Add `selected_item_ids: HashSet<ItemId>` to `App`.
- Add small helpers:
  - `has_multi_selection()`
  - `selected_count()`
  - `is_item_selected(item_id)`
  - `toggle_selected_item(item_id)`
  - `clear_selected_items()`
  - `selected_item_ids_in_view_order()`
  - `prune_selected_items_to_visible_slots()`
- Wire `Space` in Normal mode to toggle the focused item instead of being a
  no-op.
- Keep the focused cursor where it is after toggling.
- Clear selection on view switch paths.
- Prune selection after `refresh()`.
- Make `Esc` clear selection first when any selected items exist; otherwise
  preserve current search/global-search behavior.

### Rendering / UX

- Add a visible selected-state treatment distinct from focused state.
- Show selected count in footer or status text.
- Update footer hints to advertise `Space:select` and `Esc:clear sel` when
  applicable.

### Tests

- Toggling selection on one item.
- Selecting multiple items across cursor moves.
- `Esc` clears selection without clearing filters when selection exists.
- View switch clears selection.
- Refresh prunes stale selected IDs.

### Exit Criteria

- User can select multiple items and see which ones are selected.
- Selection survives movement and disappears on `Esc` or view switch.

## Phase A1: Fast Demo for Batch Assign

Purpose: reach a working demo as early as possible with the least new modal
complexity.

### Scope

Support the quickest batch-assign path first:

- If one or more items are selected, `a` opens category-name entry directly or
  reuses the existing input path in a way that assigns the resolved category to
  every selected item.
- Existing exact-match / single-visible-match / create-new logic stays intact.

This is a demo slice, not the final UX. It proves that batch actions can flow
from the shared selection state into store updates.

### Work

- Add batch context helpers:
  - `effective_action_item_ids()` returning selected IDs if non-empty, else the
    focused item ID.
  - status-summary helper for batch actions.
- Extend category-name resolution path so one resolved category can be assigned
  across `N` items.
- Report counts:
  - assigned
  - already had category
  - failed

### Tests

- Batch assign an existing category to multiple selected items.
- Batch assign a newly created category to multiple selected items.
- Empty selection still uses single-item path.

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

### Tests

- Mixed-state row renders correctly.
- Mixed-state `Space` assigns to all, not toggles per item.
- All-assigned `Space` removes from all.
- Exclusive category assignment replaces conflicting siblings per item.

### Exit Criteria

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
  - preserve selection unless there is a strong UX reason to clear it

### Design Notes

- Batch delete should not depend on the focused item after the confirmation
  opens; use a snapshot.
- Reuse existing `Mode::ConfirmDelete` if practical, but do not overload it so
  heavily that the code becomes opaque.

### Tests

- Confirm deletes all selected items.
- Cancel leaves items untouched.
- Empty selection preserves current single-item delete behavior.

### Exit Criteria

- User can select several items and delete them in one confirmation flow.

## Phase B1: Fast Demo for Batch Linking

Purpose: prove the selected-set can drive a repeated per-item linking action
without implementing every relation variant immediately.

### Scope

Start with the smallest high-value batch linking path:

- `b` only
- one shared target
- apply `BlockedBy` / depends-on semantics across all selected items

This gives a working demo for “make all of these blocked by X.”

### Work

- Add batch context to link wizard state:
  - anchor item IDs instead of only one anchor ID when in batch mode
- Exclude every selected item from target matches.
- Apply the chosen target once per selected item.
- Summarize:
  - links created
  - already existed
  - failed

### Tests

- Selected items are excluded from target list.
- Batch `b` creates depends-on links for every selected item.
- Existing links are counted as already present, not treated as hard errors.

### Demo Outcome

Basic demo script:

1. Select 3 items.
2. Press `b`.
3. Choose one blocker target.
4. Confirm.
5. All 3 items are now blocked by that target.

## Phase B2: Full Batch Link Wizard

Purpose: complete linking parity for the issue scope.

### Scope

Support the intended batch behavior for:

- `b` -> blocked by target
- `B` -> blocks target
- optionally wizard-selected actions already present in UI:
  - `DependsOn`
  - `RelatedTo`
  - `ClearDependencies`

### Work

- Generalize wizard state so single-item and batch-item flows share one code
  path where possible.
- Keep targetless action support for `ClearDependencies`.
- Ensure status summaries are relation-specific and counted.
- Keep target filtering and focus behavior stable in batch mode.

### Tests

- Batch `B` creates `blocks` links from every selected item to the chosen
  target.
- Batch `ClearDependencies` removes links across all selected items.
- Wizard cancel returns to Normal mode without mutating links.

### Exit Criteria

- Batch linking covers all link actions promised by the issue note.

## Phase R: Render and Discoverability Polish

Purpose: make the feature understandable without reading code or memorizing
hidden states.

### Work

- Distinguish:
  - focused item
  - selected item
  - focused + selected item
- Verify both single-line and multi-line board displays.
- Update footer hints for:
  - no selection
  - active selection
  - batch modal entry points
- Add status messages that include selected count and action summary.

### Tests

- Footer hint text changes when selection exists.
- Rendering keeps selected items visible and distinct in both board display
  modes.

## Phase T: Hardening and Regression Coverage

Purpose: finish the story cleanly and avoid modal regressions.

### Work

- Review edge cases:
  - selected item deleted externally before refresh
  - selection across slots and horizontal flow
  - selection while preview is open
  - empty slot behavior
  - interaction with global search session
- Normalize action result reporting so partial failures are understandable.
- Check whether AGENTS.md needs a new gotcha only if implementation reveals one.

### Verification

- `cargo fmt`
- `cargo clippy --all-targets --all-features`
- `cargo test -p agenda-tui`
- `cargo test`

## Recommended Implementation Order

If optimizing for fastest end-to-end demo:

1. Phase 0
2. Phase A1
3. Demo checkpoint
4. Phase X
5. Phase B1
6. Phase A2
7. Phase B2
8. Phase R
9. Phase T

## Demo Checkpoints

### Checkpoint 1: Selection Visible

- `Space` selects multiple items
- selected count is visible
- `Esc` clears selection

### Checkpoint 2: Basic Batch Assign

- `a` assigns one category to all selected items

### Checkpoint 3: Basic Batch Delete

- `x` deletes all selected items after one confirmation

### Checkpoint 4: Basic Batch Link

- `b` links all selected items to one chosen target

## Risks

- Reusing single-item modal state too aggressively may create hidden branching
  and brittle conditionals.
- `ConfirmDelete` already multiplexes delete and done-toggle confirmation; batch
  delete should avoid making that mode unreadable.
- Link Wizard currently assumes a single anchor item in several helper paths.
- Category picker semantics for mixed state can become confusing if row
  indicators and status copy are weak.

## Recommended Code Shape

- Keep selection state generic and action-agnostic.
- Add narrow batch helpers rather than scattering `if selected_count > 0`
  checks through unrelated code.
- Prefer explicit batch context structs when reusing modal flows:
  - `BatchDeleteState`
  - `BatchAssignContext`
  - `BatchLinkContext`

That keeps the code easier to reason about than overloading single-item fields.
