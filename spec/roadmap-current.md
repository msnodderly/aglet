# Roadmap (Current Reality)

Date: 2026-02-17
Scope: execution roadmap derived from what is already implemented, not from pre-implementation MVP task templates.

## 1. Status Snapshot

Already implemented:

- Core domain engine and storage foundations.
- Functional CLI control plane (items/categories/views/delete log/restore).
- Usable TUI daily loop and management/editing workflows (navigate/add/move/remove/done/delete/filter/inspect/view switch/category manager/edit/unassign).
- Broad automated coverage (`cargo test` passes workspace).
- Multiple manual/e2e scenario logs under `docs/`.
- Scenario-to-capability matrix for `product-spec-complete` scenarios (`spec/scenario-capability-matrix.md`).

Therefore, roadmap emphasis shifts from "build from scratch" to "close targeted completion and hardening gaps".

## 2. Completed Milestones

## R1. Spec/Reality Alignment

- Rewrite product docs to match shipped model and behavior.
- Mark recurrence/suggestions/typed-value-columns as deferred until implemented.
- Keep acceptance criteria tied to current parser and UI capability.
- Add scenario-to-capability mapping for `product-spec-complete` NLSpec scenarios.

Exit criteria:

- `spec/product-current.md`, `spec/gaps.md`, `spec/roadmap-current.md` reflect current code truth.
- `spec/scenario-capability-matrix.md` exists and is kept current as conformance reference.

## R2. TUI Completion For SLC

- Add category management workflows in TUI (at minimum create/delete + structural edits needed for daily use).
- Add item text and note editing from TUI.
- Improve empty-state and error-state UX in TUI.

Exit criteria:

- User can run day-to-day without dropping to CLI for basic management/editing.

## 3. Now (Highest Priority)

## R3. TUI View + Category Workflow Streamlining (Completed, Maintain)

- Adopt the workflow spec in `spec/tui-view-category-workflow.md`.
- Replace split-pane section navigation with section-first horizontal board layout.
- Implement full in-TUI view criteria editing (multi include/exclude + virtual include/exclude).
- Reduce shortcut friction with laptop-friendly bindings and retain F-key aliases.
- Make unmatched section behavior configurable and less intrusive by default.

Exit criteria:

- View and category workflows in TUI match the formal interaction spec and no longer require CLI for basic view criteria shaping.

## R3.5. View + Column Workflow Design and Experiments

- Lock view/column workflow contract using Lotus-style selection/demarcation/annotation framing.
- Keep current baseline (`When | Item | All Categories`) explicit and stable for all views.
- Prototype category-family column headings (for example `Priority`, `People`, `Department`) with UI-first experiments before model changes.
- Add an explicit model/persistence gate so schema changes happen only if experimentation proves necessity.

Exit criteria:

- A detailed design spec exists (`spec/tui-view-column-workflow-design.md`).
- A sequenced experiment plan exists in `spec/tasks.md` (`T077-T083`).
- Team has a clear decision path for "UI-only vs model extension" for column workflows.

## R4. Safety Contract (v1)

- Chosen contract: ship **no-undo** for v1 with strong explicit recovery UX.
- Prioritize:
  - destructive-action confirmation and clear status messaging
  - deletion-log visibility and fast restore workflows
  - CLI/TUI help text and docs that make the no-undo contract explicit

Exit criteria:

- A documented and implemented no-undo recovery model for accidental edits.

## 4. Next (Hardening)

## R5. Persistence/Data Integrity Hardening

- Replace silent decode fallbacks with explicit typed errors where feasible.
- Define and implement restore fidelity policy (timestamp provenance + missing-category behavior).
- Add targeted corruption/restore regression tests.

## R6. Domain API Maturity

- Add first-class domain operations for category/view evolution (rename/reparent/reorder/update semantics with strong invariants).
- Keep frontends consuming domain APIs rather than embedding mutation logic.

## 5. Later (Deferred v0.6 Features)

## R7. Advanced Intelligence + Model Expansion

- Recurrence model and next-instance generation.
- Suggestion queue/review workflow and rejection memory.
- Classification mode thresholds and assisted/manual review paths.
- Typed value columns and advanced computed views.

## 6. Evidence Anchors

Roadmap priorities were set using current implementation and executed demos, including:

- `docs/demo-complete-cli-e2e-demo-log.md`
- `docs/demo-view-logic-demo-run.md`
- `docs/demo-literate-cli-demo-global-priority-reuse.md`
- `docs/test-cross-domain-scenarios-run-results.md`
- `docs/test-script-tui-smoke-e2e.md`
- `spec/scenario-capability-matrix.md`

## 7. Recommended Sequencing

1. Implement TUI view/category workflow streamlining.
2. Lock and run view/column workflow design experiments.
3. Lock and implement safety contract.
4. Harden persistence and restore integrity.
5. Grow domain API maturity for robust frontend evolution.
6. Resume deferred advanced feature track.
