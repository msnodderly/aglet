# Tasks: Agenda Reborn (Current Roadmap)

Date: 2026-02-17
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
- [x] T004 Add a scenario-to-capability matrix mapping `spec/product-spec-complete.md` scenarios to: implemented / partial / deferred.

Checkpoint:

- Core docs reflect shipped behavior and known gaps.

## Phase 1 - TUI Completion For SLC (R2)

Goal: users can stay in TUI for daily operation and basic management/editing.

- [x] T010 Add TUI category manager MVP (list/create/delete).
- [x] T011 Add TUI category structural edits (rename/reparent + exclusivity/implicit-string toggles).
- [x] T012 Add inline item text editing in TUI.
- [x] T013 Add note create/edit flow in TUI.
- [x] T014 Add unassign action from inspect panel.
- [x] T015 Harden TUI empty/error states (no views, empty sections, failed mutations).
- [x] T016 Add TUI smoke test script for add/move/remove/done/delete/edit/category flows.

Checkpoint:

- Daily triage + basic taxonomy management is possible without dropping to CLI.

## Phase 2 - TUI View + Category Workflow Streamlining (R3)

Goal: ship a low-friction TUI workflow for board layout, view criteria editing, and category interactions.

- [x] T070 Adopt and maintain `spec/tui-view-workflow-implementation.md` as the implementation contract.
- [x] T071 Redesign TUI board rendering to section-first horizontal layout (remove dedicated section selector pane).
- [x] T072 Add full TUI view editor for multi include/exclude and virtual include/exclude criteria.
- [x] T073 Add section/unmatched configuration in TUI view editor (hide-empty default, label/pin settings).
- [x] T074 Replace function-key dependence with laptop-friendly shortcuts (`v`/`c`/`,`/`.`) and retain F-key aliases.
- [x] T075 Update TUI help/footer/status text to match new workflows and shortcut model.
- [x] T076 Add regression/smoke coverage for streamlined view/category workflows.

Checkpoint:

- View/category management workflows are fast, in-app, and aligned with `spec/tui-view-workflow-implementation.md`.

## Phase 2b - View + Column Workflow Design and Experiments (R3.5)

Goal: lock and validate a Lotus-style annotation-column workflow before committing to persistence/model changes.

- [x] T077 Publish detailed view/column workflow design spec (consolidated): `spec/tui-view-workflow-implementation.md`.
- [x] T078 Add explicit in-lane column headers in TUI for `When | Item | All Categories`.
- [x] T079 Add rendering policy + tests for `All Categories` cell formatting (sorted, comma-separated, truncated safely).
- [x] T084 Switch board section arrangement to top-to-bottom stacked lanes and tighten row density.
- [x] T085 Add view-create include/exclude picks (`+`/`-`) and `Tab`/`Shift+Tab` view cycling.
- [ ] T080 Add view-editor "column setup" experimental UX entry point (no persistence changes).
- [ ] T081 Prototype category-family column rendering (for examples like `Priority`, `People`, `Department`) with non-persistent config.
- [ ] T082 Record model/persistence decision using the gate in `spec/tui-view-workflow-implementation.md`.
- [ ] T083 Extend smoke/manual script coverage for annotation-column workflows and category-family prototype scenarios.

Checkpoint:

- View/column workflow is specified, testable, and sequenced for low-risk experimentation.

## Phase 2c - View Manager UX Refactor (R3.6)

Goal: replace the clunky popup view flow with a full-screen manager for boolean criteria and multi-section authoring.

- [x] T087 Publish detailed view manager workflow spec (consolidated): `spec/tui-view-workflow-implementation.md`.
- [x] T088 Publish terminal mockup/wireframes (consolidated): `spec/tui-view-workflow-implementation.md`.
- [x] T089 Implement full-screen view manager shell with 3-pane navigation and explicit save/cancel.
- [x] T090 Implement row-based boolean criteria builder (`+`/`-`, `AND`/`OR`, optional nesting) with validation + preview summary.
- [x] T091 Integrate section authoring into the same screen (add/remove/reorder + section criteria + insert/remove assignment sets).
- [x] T092 Add regression/smoke coverage for view-manager flows and boolean-criteria edge cases.

Checkpoint:

- A single in-app view manager supports end-to-end authoring of criteria and sections with explicit save and preview feedback.

## Phase 3 - Safety Contract (R4)

Goal: define and implement v1 mistake-recovery model.

- [x] T020 Record decision note: `minimal-undo` vs `no-undo + explicit recovery UX`.
- [ ] T021 If `minimal-undo`: add mutation journal primitives in core. (inactive path for v1)
- [ ] T022 If `minimal-undo`: implement depth-1 undo for key operations (create/delete/assign/unassign/move/edit). (inactive path for v1)
- [ ] T023 If `minimal-undo`: wire TUI `Ctrl-Z` with status feedback and tests. (inactive path for v1)
- [ ] T024 If `no-undo`: strengthen recovery UX (confirmations + deletion-log visibility + fast restore path).
- [ ] T025 Update docs and CLI/TUI help text to match the chosen safety contract.

Checkpoint:

- Recovery behavior is explicit, test-backed, and discoverable.

## Phase 4 - Persistence/Data Integrity Hardening (R5)

Goal: eliminate silent corruption handling and lock restore semantics.

- [ ] T030 Replace store decode fallbacks (`unwrap_or_default` paths) with typed decode errors where feasible.
- [ ] T031 Define restore fidelity policy (timestamp/provenance semantics, missing-category behavior).
- [ ] T032 Implement restore policy and regression tests.
- [ ] T033 Add corruption-path tests (malformed UUID/date/JSON rows) with deterministic error expectations.

Checkpoint:

- Decode and restore paths are explicit and fail predictably.

## Phase 5 - Domain API Maturity (R6)

Goal: provide robust, frontend-agnostic evolution APIs for categories and views.

- [ ] T040 Add first-class category evolution APIs (rename/reparent/reorder/toggle semantics).
- [ ] T041 Add first-class view evolution APIs (rename/update criteria/sections/columns/remove-from-view semantics).
- [ ] T042 Define invariant-oriented error model for evolution APIs (cycle, duplicate, invalid config).
- [ ] T043 Refactor CLI/TUI mutations to consume these APIs rather than bespoke mutation logic.
- [ ] T044 Add integration tests for category/view evolution invariants.

Checkpoint:

- Frontends share one canonical mutation behavior surface.

## Phase 6 - Deferred Advanced Features (R7)

Goal: close major v0.6 deferred capability gaps.

- [ ] T050 Implement recurrence data model + next-instance generation on done.
- [ ] T051 Implement suggestion queue/review flow and rejected-suggestion memory behavior.
- [ ] T052 Implement classification thresholds and assignment modes (`automatic` / `assisted` / `manual`).
- [ ] T053 Implement typed value columns (`value_type`, item values, invariants).
- [ ] T054 Implement advanced view computations (post-MVP analytics).

Checkpoint:

- Deferred v0.6 features move from planned to shippable slices.

## Dependency Order

1. Phase 0 -> Phase 1 -> Phase 2 -> Phase 2b -> Phase 2c -> Phase 3 -> Phase 4 -> Phase 5 -> Phase 6
2. `T020` gates the branch between `T021-023` and `T024`.
3. `T030-033` should complete before major persistence-affecting advanced features (`T050-053`).
4. `T040-044` should precede broad TUI feature expansion to avoid frontend logic drift.

## Parallel Work Opportunities

- `T012`, `T013`, and `T015` can run in parallel after TUI category manager baseline (`T010`).
- `T071`, `T072`, and `T074` can run in parallel once `T070` is approved.
- `T078` and `T079` can run in parallel after `T077`.
- `T080` and `T081` can run in parallel once `T078` is merged.
- `T089` can begin in parallel with `T090` if shell layout + state boundaries are pre-agreed.
- `T031` (policy) can run in parallel with `T030` (technical strictness), then converge at `T032`.
- `T044` test authoring can begin as soon as API contracts for `T040-042` are drafted.
