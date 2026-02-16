# SLC Implementation Log

Status: Active
Branch: `codex/slc-v1`
Worktree: `/Users/mds/src/aglet-slc`
Started: 2026-02-16

## Locked Product Decisions

- CLI-first configuration: yes.
- Undo required in SLC v1: no.
- Reserved categories for SLC v1: `When`, `Entry`, `Done`.
- Advanced condition/action editor UX: deferred.

## Architecture Guardrails

- Route user mutations through `agenda_core::agenda::Agenda` where possible.
- Keep mutation operations explicit so undo can attach inverse operations later.
- Preserve deletion-log and provenance paths.
- Avoid UI-layer direct SQL writes.

## Current Status Snapshot

Completed:

- SLC spec drafted and locked in `spec/slc-spec.md`.
- New branch/worktree created for SLC implementation.
- Began CLI control plane implementation (in progress).
- Added `Store::list_deleted_items()` API for deletion-log visibility.
- Added `Agenda::mark_item_done()` and `Agenda::delete_item()` operations to keep mutation boundaries in domain layer.
- Implemented CLI commands:
  - `add`
  - `list` (with `--view`, `--category`, `--include-done`)
  - `search`
  - `done`
  - `delete`
  - `deleted`
  - `restore`
  - `category list/create/delete`
  - `view list/create/delete`
- Added core tests for new APIs:
  - `agenda::mark_item_done_sets_done_fields_and_assigns_done_category`
  - `store::test_list_deleted_items_returns_latest_first`
  - `store::test_restore_deleted_item_recreates_item_and_assignments`
- Full test suite passing (`cargo test`).
- Manual CLI smoke-tested against a temp DB (`/tmp/aglet-slc-test.ag`).
- Manual delete/restore recovery flow smoke-tested (`/tmp/aglet-slc-restore.ag`).
- Implemented first usable TUI in `agenda-tui`:
  - view-based sections/items display
  - keyboard navigation (sections + items)
  - add item flow
  - move item between sections (`[`/`]`) using edit-through semantics
  - remove from view (`r`)
  - mark done (`d`)
  - delete with confirmation (`x`, `y/n`)
  - view picker (`F8`)
  - in-view filter (`/`)
  - inspect panel with assignment provenance (`i`)
- Added `agenda-tui` executable entrypoint (`crates/agenda-tui/src/main.rs`) with `--db` and `AGENDA_DB` support.
- Manual TUI startup/exit smoke test performed in PTY (`cargo run -p agenda-tui -- --db /tmp/aglet-slc-test.ag`).

In progress:

- SLC hardening and completeness gap-closing.

Remaining (high-level):

1. Stabilize and test CLI milestone.
2. Build TUI prototype (read-only + navigation + view switch).
3. Build TUI daily-use edit-through loop (create/move/remove/delete/done).
4. Add enough category/view management to be complete for SLC.
5. Implement inspect/search safety baseline and hardening pass.

## Design Decisions Taken During Implementation

1. CLI default behavior is `list` when no subcommand is supplied.
   - Reason: useful immediately for CLI-first operation while TUI is still under development.
2. Added basic CLI category and view management (not only item commands).
   - Reason: enables practical configuration without waiting for full TUI manager UX.
3. Mark-done logic placed in `Agenda::mark_item_done`.
   - Reason: keep done semantics centralized and reusable by both CLI and future TUI.

## Open Questions / Follow-ups

1. Whether SLC v1 should treat default command (`agenda`) as list or immediately launch TUI once TUI is available.
   - Current implementation: defaults to list for reliability during transition.
2. Whether to add restore-from-deletion-log command in CLI for SLC v1.
   - Current implementation: read-only deletion-log listing.
3. How much category management UX must be in TUI for SLC v1 given CLI-first configuration.
   - Current approach: keep TUI focused on daily flow first; category CRUD is already available from CLI.
4. Whether `agenda` (CLI without subcommand) should eventually launch TUI by default after one more stabilization pass.
   - Current behavior: defaults to list.
5. Whether to add a lightweight restore command for deletion log in SLC v1.
   - Resolved: implemented CLI restore (`restore <log-id>`) plus core restore API.

## Next Immediate Steps

1. Commit safety/recovery milestone with updated log.
2. Add targeted UX polish and stabilization:
   - section headers and empty states
   - safer item move semantics across generated sections
   - keybinding/help consistency across CLI/TUI docs
3. Evaluate final SLC gaps (category manager UX, default launcher behavior, undo-ready boundaries) and implement highest-value remaining pieces.
