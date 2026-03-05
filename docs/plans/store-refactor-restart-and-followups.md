# Store Refactor Restart Plan (Post PR #48 / #49)

## Purpose

Capture exactly what PR [#48](https://github.com/msnodderly/aglet/pull/48) implemented, map those decisions onto current `main` (after merge of PR #49), and define a low-risk restart plan for the remaining refactors.

This document is intended to prevent re-litigating decisions while avoiding a risky direct merge of stale refactor code.

## Inputs Reviewed

1. PR #48 title/body/commit metadata and implementation claims.
2. PR #48 branch (`377ceac`) diff and test outcomes.
3. Current `origin/main` state at `af080a3` (Merge PR #49).
4. Existing plan in [`docs/code-quality-plan.md`](../code-quality-plan.md).

## What PR #48 Implemented (Per PR Description)

PR #48 explicitly stated it implemented items **A, C, D, E, F** from `docs/code-quality-plan.md`.

### A. Reserved category constants exported and used

- Moved reserved names from private `store.rs` constants to public exports in `model.rs`.
- Canonical constants:
  - `RESERVED_CATEGORY_NAME_WHEN`
  - `RESERVED_CATEGORY_NAME_ENTRY`
  - `RESERVED_CATEGORY_NAME_DONE`
  - `RESERVED_CATEGORY_NAMES`
- Replaced bare `"When"`, `"Entry"`, `"Done"` references in `agenda.rs` and `store.rs`.

### C. JSON serde error handling conventions

- Serialization convention: use `expect(...)` for in-memory types that should always serialize.
- Deserialization convention:
  - use `unwrap_or_default()` with comments for safe fallback paths from legacy/corrupt DB data.
  - use `map_err(...)?` for load-bearing fields.

### D. Store decomposition (`store.rs` -> `store/` submodule)

- Refactor split in PR #48 branch:
  - `store/mod.rs`
  - `store/items.rs`
  - `store/categories.rs`
  - `store/views.rs`
  - `store/assignments.rs`
  - `store/links.rs`
  - `store/tests.rs`
- Claimed invariant: no public API signature changes.

### E. Dead-code suppression documentation

- Added explanatory comments to each `#[allow(dead_code)]` usage in `agenda-tui`.

### F. Origin semantics constants

- Added documented canonical origin string constants in `model.rs` and updated write sites in engine paths.

### Verification claim in PR #48 description

- PR #48 stated: all 259 store unit tests passed and workspace builds had zero new warnings.

## Current Main Status (After PR #49)

As of `origin/main` commit `af080a3`:

1. **A is already present** on `main`.
2. **F is already present** on `main`.
3. **E is already present** on `main` (dead-code suppressions include TODO intent comments).
4. **C is partially complete** on `main`:
   - many `serde_json::to_string` paths use `map_err(...)` or `expect(...)`,
   - but some paths still use fallback forms (for example `board_display_mode` serialization fallback).
5. **D is not merged**: `main` still has monolithic `crates/agenda-core/src/store.rs`.
6. **B remains unimplemented** (ID-to-SQL conversion centralization from the original quality plan).

## Why We Should Restart Instead of Merging PR #48 Directly

Direct merge of PR #48 is high risk because:

1. The branch is stale relative to `main` and has a structural modify/delete conflict on `store.rs`.
2. `main` has newer schema/runtime behavior not represented in PR #48 split files (notably `views.hide_dependent_items` migration + persistence paths).
3. PR #48 was a broad one-shot move, which makes conflict resolution equivalent to a partial rewrite anyway.
4. A clean restart from `origin/main` preserves today’s behavior and produces a reviewable mechanical diff.

Decision: **restart the store decomposition from latest `main` and treat PR #48 as design reference, not merge candidate**.

## Required Invariants for the Restart Refactor

The restart must preserve these existing `main` behaviors exactly:

1. `SCHEMA_VERSION` and migration chain semantics.
2. `views.hide_dependent_items` column support in:
   - schema creation,
   - migrations,
   - `insert_view`,
   - `update_view`,
   - `row_to_view`,
   - migration tests.
3. Current Item persistence semantics (including `entry_date` DB handling as derived from runtime item timestamps).
4. Existing public `Store` API signatures and visibility.
5. Existing error variants and user-facing error messages for store operations.

## Execution Plan (Restart Implementation)

### Phase 0: Baseline and guardrails

1. Branch from latest `origin/main` (new branch name under `codex/` prefix).
2. Record baseline:
   - `cargo check --workspace`
   - `cargo test -p agenda-core`
3. Freeze behavioral contract:
   - capture list of `pub fn` methods on `impl Store`,
   - capture migration test list and schema version expectations.

### Phase 1: Mechanical module split only

1. Move `crates/agenda-core/src/store.rs` to `crates/agenda-core/src/store/mod.rs`.
2. Introduce child modules:
   - `items.rs`
   - `categories.rs`
   - `views.rs`
   - `assignments.rs`
   - `links.rs`
3. Move code by domain with no logic edits.
4. Keep private helper visibility unchanged (`impl Store` private helpers remain in `mod.rs` unless helper extraction is purely mechanical).
5. Compile and test after each module extraction step.

### Phase 2: Test split and parity verification

1. Move `store` unit tests into `store/tests.rs`.
2. Ensure all existing migration tests (including v8 -> v9 hide-dependent migration coverage) remain present.
3. Re-run full store tests and workspace compile.
4. Run diff audit focused on semantic changes:
   - SQL statements,
   - migration SQL,
   - default values,
   - serde fallbacks.

### Phase 3: Review hardening

1. Require reviewer checklist sign-off:
   - schema parity,
   - migration parity,
   - API parity,
   - no unexpected behavior changes.
2. Land as a dedicated “mechanical decomposition” PR.

## Additional Refactors Worth Scheduling (After Decomposition)

These should be separate PRs to keep risk isolated:

### R1. Implement Item B (typed SQL params for IDs)

- Add `ToSql` impls (or equivalent typed conversion helpers) for ID newtypes.
- Replace repetitive `.to_string()` parameterization for SQL IDs.
- Goal: less boilerplate + stronger type safety at SQL boundary.

### R2. Complete Item C consistency cleanup

- Remove remaining `to_string(...).unwrap_or_else(...)` fallback paths where hard failure or explicit typed error propagation is preferable.
- Document each intentional fallback with one-line rationale.

### R3. Migration regression harness

- Add a compact migration fixture/harness that verifies upgrade from legacy schema versions up to current version.
- Include explicit assertions for every non-legacy `views` column.

### R4. Store API surface regression test

- Add a lightweight compile-time/API guard (or generated signature snapshot) to detect accidental `Store` public API drift during future refactors.

### R5. Optional: extract store internals by concern

- After stable decomposition, consider extracting internal helper clusters:
  - row mappers,
  - migration utilities,
  - category ordering helpers.
- Keep as non-functional changes only.

## Validation Matrix

Minimum required checks for the decomposition PR:

1. `cargo check --workspace`
2. `cargo test -p agenda-core`
3. `cargo test --workspace` (preferred before merge)
4. Migration test subset:
   - v5 item_links creation path
   - v6 aliases column path
   - v7 app_settings path
   - v8 -> v9 hide_dependent_items path
5. Manual CLI smoke:
   - create/update view with hide-dependent toggle
   - reopen DB and verify persisted value.

## Deliverables

1. This plan document.
2. A restart PR for Item D only (mechanical split from latest main).
3. Follow-up PRs for R1-R5 as scoped above.

## Non-Goals

1. No schema redesign in the decomposition PR.
2. No behavioral/product changes in decomposition PR.
3. No bundling of unrelated CLI/TUI feature work with store decomposition.
