# Plan: Centralize Dependency-Blocked Evaluation

## Summary

Unify dependency-blocked evaluation in `agenda-core` so CLI and TUI both use
the same rules for:

- single-item blocked checks
- batch blocked-id computation
- blocked/not-blocked filtering
- workflow claimability

This keeps dependency graph semantics separate from workflow claim semantics:
dependency links model prerequisite structure, while workflow claimability reads
that derived blocked state as one input.

## Implementation Checklist

- [x] Confirm and document the current duplication points in core, CLI, and TUI
- [x] Add shared batch dependency-state helpers in `crates/agenda-core/src/workflow.rs`
- [x] Keep `claimability_for_item(...)` layered on top of the shared dependency-state helpers
- [x] Add/expand `agenda-core` tests for:
  - [x] unresolved dependency means blocked
  - [x] done dependency means not blocked
  - [x] batch blocked-id computation matches single-item checks
  - [x] blocked/not-blocked filtering preserves expected rows
- [x] Update CLI list/search/view paths to use the shared core helpers
- [x] Remove CLI-local duplicate dependency-state helpers that are superseded
- [x] Update CLI tests to target the shared behavior instead of removed local internals
- [x] Align TUI blocked checks with the same shared core semantics
- [x] Keep or add TUI regression coverage for:
  - [x] ready queue excludes blocked items
  - [x] hide-dependent-items excludes blocked items
  - [x] single-item blocked detection flips when dependency becomes done
- [x] Run targeted test suites for `agenda-core`, `agenda-cli`, and `agenda-tui`
- [x] Update this checklist to fully complete with any implementation notes

## Detailed Implementation Notes

### Core

Introduce shared helpers in `workflow.rs` with a shape close to:

- `item_is_dependency_blocked(store, item_id) -> Result<bool>`
- `blocked_item_ids(store, items) -> Result<HashSet<ItemId>>`
- `retain_items_by_dependency_state(store, items, blocked: bool) -> Result<()>`

The batch helper should avoid recomputing done state from the store for every
edge when that information is already present in the provided item slice.

`claimability_for_item(...)` should continue to read as policy logic and should
call the shared blocked helper rather than open-coding dependency traversal.

### CLI

Replace local dependency-state implementations in
`crates/agenda-cli/src/main.rs` with calls into `agenda_core::workflow`.

Impacted surfaces:

- `list --blocked/--not-blocked`
- `search --blocked/--not-blocked`
- `view show "Ready Queue"`
- general blocked-id computation used when rendering views with
  `hide_dependent_items`

The CLI should continue enforcing the special-case Ready Queue UX:

- `view show "Ready Queue" --blocked` is invalid
- `view show "Ready Queue" --not-blocked` is redundant

### TUI

The TUI already uses core claimability for the ready queue, but normal blocked
checks still live in `App::is_item_blocked()`. Update that method to delegate to
the shared core helper so projection/rendering uses the same semantics as the
CLI and workflow code.

### Verification

At minimum, run:

```bash
cargo test -p agenda-core workflow
cargo test -p agenda-cli dependency_state_filter
cargo test -p agenda-tui is_item_blocked_returns_true_when_dependency_undone
```

If targeted test names drift during refactor, run the nearest equivalent focused
tests and record the exact commands used below.

Executed verification commands:

```bash
cargo test -p agenda-core workflow
cargo test -p agenda-cli dependency_state_filter
cargo test -p agenda-cli blocked_item_ids_marks_open_dependency_as_blocked
cargo test -p agenda-tui is_item_blocked_returns_true_when_dependency_undone
cargo test -p agenda-tui view_picker_enter_switches_to_ready_queue_and_filters_blocked_items
cargo test -p agenda-tui hide_dependent_items_view_setting_filters_blocked_items_from_slots
```

## Progress Notes

- Initial draft created on 2026-03-31.
- Core now owns batch blocked-id computation and dependency-state filtering.
- CLI list/search/view paths were switched from local helpers to `agenda_core::workflow`.
- TUI now caches core-computed blocked item ids during `App::refresh()`.
- Focused verification passed in all three crates after one test adjustment to
  avoid `toggle_item_done()`'s actionable-category precondition in a core-only
  dependency-state test.
