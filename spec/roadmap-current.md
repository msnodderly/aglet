# Roadmap (Current Reality)

Date: 2026-02-16
Scope: execution roadmap derived from what is already implemented, not from pre-implementation MVP task templates.

## 1. Status Snapshot

Already implemented:

- Core domain engine and storage foundations.
- Functional CLI control plane (items/categories/views/delete log/restore).
- Usable TUI daily loop (navigate/add/move/remove/done/delete/filter/inspect/view switch).
- Broad automated coverage (`cargo test` passes workspace).
- Multiple manual/e2e scenario logs under `docs/`.

Therefore, roadmap emphasis shifts from "build from scratch" to "close targeted completion and hardening gaps".

## 2. Now (Highest Priority)

## R1. Spec/Reality Alignment

- Rewrite product docs to match shipped model and behavior.
- Mark recurrence/suggestions/typed-value-columns as deferred until implemented.
- Keep acceptance criteria tied to current parser and UI capability.

Exit criteria:

- `spec/product-current.md`, `spec/gaps.md`, `spec/roadmap-current.md` reflect current code truth.

## R2. TUI Completion For SLC

- Add category management workflows in TUI (at minimum create/delete + structural edits needed for daily use).
- Add item text and note editing from TUI.
- Improve empty-state and error-state UX in TUI.

Exit criteria:

- User can run day-to-day without dropping to CLI for basic management/editing.

## R3. Safety Contract (v1)

- Decide one of:
  - implement minimal undo, or
  - explicitly ship no-undo with strong recovery UX (deletion log visibility/restore flow + clear confirmations + inspect-driven recovery affordances).

Exit criteria:

- A documented and implemented safety model for accidental edits.

## 3. Next (Hardening)

## R4. Persistence/Data Integrity Hardening

- Replace silent decode fallbacks with explicit typed errors where feasible.
- Define and implement restore fidelity policy (timestamp provenance + missing-category behavior).
- Add targeted corruption/restore regression tests.

## R5. Domain API Maturity

- Add first-class domain operations for category/view evolution (rename/reparent/reorder/update semantics with strong invariants).
- Keep frontends consuming domain APIs rather than embedding mutation logic.

## 4. Later (Deferred v0.6 Features)

## R6. Advanced Intelligence + Model Expansion

- Recurrence model and next-instance generation.
- Suggestion queue/review workflow and rejection memory.
- Classification mode thresholds and assisted/manual review paths.
- Typed value columns and advanced computed views.

## 5. Evidence Anchors

Roadmap priorities were set using current implementation and executed demos, including:

- `docs/demo-complete-cli-e2e-demo-log.md`
- `docs/demo-view-logic-demo-run.md`
- `docs/demo-literate-cli-demo-global-priority-reuse.md`
- `docs/test-cross-domain-scenarios-run-results.md`

## 6. Recommended Sequencing

1. Finish spec/doc alignment.
2. Close TUI completeness gaps.
3. Lock and implement safety contract.
4. Harden persistence and restore integrity.
5. Resume deferred advanced feature track.
