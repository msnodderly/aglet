# Implementation Plan: `4d156d1e` Multi-Select in TUI

## Status

Branch: `codex/4d156d1e-phase0-selection`
Worktree: `/Users/mds/src/aglet-4d156d1e-phase0`
Current base merge: `81e13e5` (`origin/main` merged on 2026-03-06)

## Progress

### Completed

- Phase 0 foundation completed in commit `ab272d9`
- Fresh `origin/main` merged cleanly in commit `81e13e5`

### Phase 0 Delivered

- Added transient selection state to TUI Normal mode
- `Space` toggles selection on the focused item
- `Esc` clears selection before search/global-search behavior
- Selection clears on view switch
- Selection is pruned on refresh when selected items disappear
- Board render and footer hints now expose active selection state
- Focused `agenda-tui` tests added for selection lifecycle and visibility

### Active Work

- Apply marker UX correction:
  - keep `>` for any focused row
  - use `+` only for selected but unfocused rows
  - remove redundant `sel:N` header count
- Implement Phase A1 fast-path batch assign:
  - with active selection, `a` should enter typed category assign
  - `Enter` should resolve/create category once and assign it to all selected items
  - preserve current single-item picker flow when no selection exists

## Next Checkpoints

### Checkpoint 2: Batch Assign Demo

Expected demo:

1. Select 2-3 items with `Space`
2. Press `a`
3. Type category name
4. Press `Enter`
5. Category is assigned to all selected items

### After A1

- Phase X: batch delete
- Phase B1: fast-path batch link via `b`
- Phase A2: full tri-state category picker
- Phase B2: full batch link wizard parity

## Notes

- Keep single-item flows unchanged when no selection exists
- Prefer explicit batch helpers over scattered selection-condition branching
- Clear selection after successful batch mutations unless a later phase proves a
  better UX
