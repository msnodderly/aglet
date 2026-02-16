# Agenda Reborn - Product Current (Reality Snapshot)

Status: Active implementation
Date: 2026-02-16

## 1. Source Of Truth Used

This snapshot is based on current repository reality, not planned scope:

- `crates/agenda-core/*`
- `crates/agenda-cli/src/main.rs`
- `crates/agenda-tui/src/lib.rs`
- `cargo test` (workspace passes)
- CLI/TUI demo logs under `docs/`, especially:
  - `docs/demo-complete-cli-e2e-demo-log.md`
  - `docs/demo-view-logic-demo-run.md`
  - `docs/demo-literate-cli-demo-global-priority-reuse.md`
  - `docs/test-cross-domain-scenarios-run-results.md`

## 2. Implementation Baseline

## 2.1 Core (`agenda-core`)

Implemented and exercised with substantial test coverage:

- SQLite store with WAL initialization and first-launch bootstrap.
- Reserved categories: `When`, `Entry`, `Done`.
- Item/category/view CRUD.
- Assignment persistence with provenance (`source`, `origin`, timestamp, sticky).
- Rule engine with:
  - implicit string matching
  - profile conditions
  - fixed-point passes with pass cap
  - deferred remove actions
  - subsumption (ancestor assignment)
  - mutual exclusion for exclusive sibling categories
  - retroactive evaluation across existing items
- Query evaluator:
  - include/exclude category logic
  - virtual `WhenBucket` include/exclude
  - text search across item text + note
- View resolver:
  - sections
  - generated unmatched section
  - `show_children` expansion
- Date parser (`BasicDateParser`) for deterministic MVP phrases (absolute, selected relative, compound date+time patterns).
- Agenda orchestration layer wiring create/update/assign/edit-through/done/delete flows.
- Deletion log + restore support.

Not yet implemented in core:

- Recurrence model/engine behavior.
- Suggestion queue / manual review workflow.
- Classification modes/threshold policy from full v0.6 spec.
- Undo stack (`undo.rs` is effectively empty).

## 2.2 CLI (`agenda-cli`)

CLI is functional and no longer placeholder.

Current command surface:

- `add`
- `list` (`--view`, `--category`, `--include-done`)
- `search` (`text` + `note`)
- `done`
- `delete`
- `deleted`
- `restore`
- `tui`
- `category list|create|delete|assign`
- `view list|show|create|delete`

Behavior validated by e2e demos:

- End-to-end capture -> classify -> view -> done -> delete -> restore in `docs/demo-complete-cli-e2e-demo-log.md`.
- Include/exclude view logic and empty-view behavior in `docs/demo-view-logic-demo-run.md`.
- Exclusive priority behavior and global-name reuse demonstrated in `docs/demo-literate-cli-demo-global-priority-reuse.md` and `docs/demo-literate-cli-demo-exclusive-fix-validation.md`.
- Cross-domain workflows validated in `docs/test-cross-domain-scenarios-run-results.md`.

## 2.3 TUI (`agenda-tui`)

TUI is functional for daily loop basics and no longer placeholder.

Implemented:

- View-based section/item rendering.
- Keyboard navigation.
- Add item flow.
- Move item between sections via edit-through semantics.
- Remove from view.
- Mark done.
- Delete with confirmation.
- View picker (`F8`).
- In-view text filter (`/`).
- Inspect panel for assignment provenance (`i`).

Not yet implemented in TUI:

- Category manager/editor (`F9`-style full management).
- Inline text/note editing workflows.
- Undo (`Ctrl-Z`).
- Suggestion review UX.

## 3. Conformance Against `product-spec-complete.md`

High-conformance areas:

- Schema-last capture and multifiling semantics.
- Retroactive category assignment.
- Hierarchical subsumption.
- Exclusive sibling behavior.
- View include/exclude logic and unmatched grouping.
- Edit-through assignment side effects.
- Deletion log and restore path.

Partial-conformance areas:

- Date parsing: good MVP subset, not full NL coverage from spec examples.
- Inspect/provenance: present in TUI panel, but not full assign/unassign tooling from inspect workflow.
- TUI interaction model: usable, but still below full manager/editor ambitions in v0.6 text.

Non-conformance (deferred/not built):

- Recurrence generation on done.
- Suggestion queue and rejected suggestion memory model.
- Typed column-values model (`values`, `value_type`, numeric/date fields).
- Advanced condition/action types (date/delete/validation conditions).
- Undo stack behavior.
- `Entry When Done` reserved trigger category.

## 4. Updated Product Position

The current product is best described as:

- **Core engine complete enough for real usage**.
- **CLI complete enough for real control-plane operation**.
- **TUI usable for primary day-to-day navigation and triage loops, but not yet complete for full in-app configuration/editing workflows**.

This is beyond "prototype core only" and should be treated as an **implemented SLC baseline with targeted completion gaps**, not an unstarted UI phase.

## 5. Immediate Product Priorities

1. Close remaining SLC usability gaps in TUI (category management, item/note editing, stronger empty-state UX).
2. Decide and implement undo/safety stance for v1 (either true undo or explicit non-undo safety UX + restore workflows).
3. Align data model/spec language with shipped model so docs stop implying already-shipped recurrence/suggestions/value-columns.
4. Harden store decode strictness and restore fidelity policy.
