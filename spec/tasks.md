# Tasks: Agenda Reborn (Current Roadmap)

Date: 2026-02-16
Input: `spec/roadmap-current.md`

## Status Legend

- `[ ]` planned
- `[~]` in progress
- `[x]` complete

## Phase 0 - Alignment (R1)

Goal: lock documentation and acceptance language to current implementation reality.

- [x] T001 Update `spec/product-current.md` to implementation-grounded status.
- [x] T002 Update `spec/gaps.md` to actual spec-vs-code gaps.
- [x] T003 Update `spec/roadmap-current.md` to now/next/later execution model.
- [ ] T004 Add a scenario-to-capability matrix mapping `spec/product-spec-complete.md` scenarios to: implemented / partial / deferred.

Checkpoint:

- Core docs reflect shipped behavior and known gaps.

## Phase 1 - TUI Completion For SLC (R2)

Goal: users can stay in TUI for daily operation and basic management/editing.

- [x] T010 Add TUI category manager MVP (list/create/delete).
- [~] T011 Add TUI category structural edits (rename/reparent + exclusivity/implicit-string toggles).
- [x] T012 Add inline item text editing in TUI.
- [x] T013 Add note create/edit flow in TUI.
- [x] T014 Add unassign action from inspect panel.
- [x] T015 Harden TUI empty/error states (no views, empty sections, failed mutations).
- [ ] T016 Add TUI smoke test script for add/move/remove/done/delete/edit/category flows.

Checkpoint:

- Daily triage + basic taxonomy management is possible without dropping to CLI.

## Phase 2 - Safety Contract (R3)

Goal: define and implement v1 mistake-recovery model.

- [ ] T020 Record decision note: `minimal-undo` vs `no-undo + explicit recovery UX`.
- [ ] T021 If `minimal-undo`: add mutation journal primitives in core.
- [ ] T022 If `minimal-undo`: implement depth-1 undo for key operations (create/delete/assign/unassign/move/edit).
- [ ] T023 If `minimal-undo`: wire TUI `Ctrl-Z` with status feedback and tests.
- [ ] T024 If `no-undo`: strengthen recovery UX (confirmations + deletion-log visibility + fast restore path).
- [ ] T025 Update docs and CLI/TUI help text to match the chosen safety contract.

Checkpoint:

- Recovery behavior is explicit, test-backed, and discoverable.

## Phase 3 - Persistence/Data Integrity Hardening (R4)

Goal: eliminate silent corruption handling and lock restore semantics.

- [ ] T030 Replace store decode fallbacks (`unwrap_or_default` paths) with typed decode errors where feasible.
- [ ] T031 Define restore fidelity policy (timestamp/provenance semantics, missing-category behavior).
- [ ] T032 Implement restore policy and regression tests.
- [ ] T033 Add corruption-path tests (malformed UUID/date/JSON rows) with deterministic error expectations.

Checkpoint:

- Decode and restore paths are explicit and fail predictably.

## Phase 4 - Domain API Maturity (R5)

Goal: provide robust, frontend-agnostic evolution APIs for categories and views.

- [ ] T040 Add first-class category evolution APIs (rename/reparent/reorder/toggle semantics).
- [ ] T041 Add first-class view evolution APIs (rename/update criteria/sections/columns/remove-from-view semantics).
- [ ] T042 Define invariant-oriented error model for evolution APIs (cycle, duplicate, invalid config).
- [ ] T043 Refactor CLI/TUI mutations to consume these APIs rather than bespoke mutation logic.
- [ ] T044 Add integration tests for category/view evolution invariants.

Checkpoint:

- Frontends share one canonical mutation behavior surface.

## Phase 5 - Deferred Advanced Features (R6)

Goal: close major v0.6 deferred capability gaps.

- [ ] T050 Implement recurrence data model + next-instance generation on done.
- [ ] T051 Implement suggestion queue/review flow and rejected-suggestion memory behavior.
- [ ] T052 Implement classification thresholds and assignment modes (`automatic` / `assisted` / `manual`).
- [ ] T053 Implement typed value columns (`value_type`, item values, invariants).
- [ ] T054 Implement advanced view computations (post-MVP analytics).

Checkpoint:

- Deferred v0.6 features move from planned to shippable slices.

## Dependency Order

1. Phase 0 -> Phase 1 -> Phase 2 -> Phase 3 -> Phase 4 -> Phase 5
2. `T020` gates the branch between `T021-023` and `T024`.
3. `T030-033` should complete before major persistence-affecting advanced features (`T050-053`).
4. `T040-044` should precede broad TUI feature expansion to avoid frontend logic drift.

## Parallel Work Opportunities

- `T012`, `T013`, and `T015` can run in parallel after TUI category manager baseline (`T010`).
- `T031` (policy) can run in parallel with `T030` (technical strictness), then converge at `T032`.
- `T044` test authoring can begin as soon as API contracts for `T040-042` are drafted.
