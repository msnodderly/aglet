---
title: Agenda-TUI Practical Refactors
status: shipped
created: 2026-03-25
shipped: 2026-03-29
---

# Agenda-TUI Practical Refactors

## Summary

Refactor `agenda-tui` in small, behavior-preserving phases that reduce the size
and responsibility of `src/lib.rs`, improve local reasoning, and create clearer
boundaries for future UI work. This plan is intentionally scoped to internal
structure only; it does not change user-visible behavior, schema, or CLI/store
contracts.

## Refactor Phases

### 1. Extract Tests From `lib.rs`

- Move the large `#[cfg(test)]` module out of `crates/agenda-tui/src/lib.rs`
  into dedicated `src/tests*.rs` modules.
- Keep helper functions and assertions unchanged.
- Keep tests compiling as crate-internal unit tests.

### 2. Extract Undo / History

- Move `UndoState`, `UndoEntry`, and undo/redo application logic into a
  dedicated module such as `crates/agenda-tui/src/undo.rs`.
- Preserve refresh timing, status text, and inverse-operation behavior.

### 3. Extract Feature State Types

- Move feature-local enums and structs out of `lib.rs` into grouped internal
  modules:
  - `state/board.rs`
  - `state/category.rs`
  - `state/view_edit.rs`
  - `state/classification.rs`
  - `state/assign.rs`
- Re-export only the types needed by sibling modules.

### 4. Group `App` Fields

- Replace the flat `App` field list with cohesive sub-state structs covering:
  - board/session navigation
  - category management
  - view editing
  - classification/review
  - transient UI/session state
- Preserve existing method behavior and defaults.

### 5. Extract Projection / Refresh Logic

- Pull the derived screen-state work out of `crates/agenda-tui/src/app.rs`
  `refresh()` into a dedicated projection module.
- Centralize view resolution, synthetic ready-queue insertion, slot building,
  lane filtering/sorting, and related derived-state rebuilding.

### 6. Add A Backend Facade

- Introduce a small internal adapter around `Agenda` plus `Store` reads so TUI
- feature modules stop reaching into `agenda.store()` directly for common
  shared operations.
- Limit the first pass to operations used across board, category, view, and
  settings flows.

## Validation

- Run `agenda-tui` tests after each phase.
- After phases 2, 4, and 5, run the full workspace tests if the crate tests are
  green.
- Keep each phase independently mergeable; do not rely on later phases to make
  earlier ones correct.

## Defaults

- No user-visible behavior changes are intended.
- No schema, DB format, or CLI changes are included.
- If a phase causes disproportionate churn or fallout, stop at the last green
  phase and leave the remaining work for follow-up rather than forcing a single
  oversized patch.
