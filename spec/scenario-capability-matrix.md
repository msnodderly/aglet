# Scenario-To-Capability Matrix (Current Reality)

Date: 2026-02-16  
Scope: map `spec/product-spec-complete.md` scenarios (01-23) to current implementation status.

## Status Legend

- `implemented`: behavior is shipped and evidenced in code/tests/demo flows.
- `partial`: core behavior exists, but notable scenario expectations are missing.
- `deferred`: scenario is not implemented for current product scope.

## Summary

- Implemented: 10/23
- Partial: 9/23
- Deferred: 4/23

## Matrix

| Scenario | Status | Current capability assessment | Evidence anchors |
|---|---|---|---|
| S01 First Launch - Empty State | partial | DB bootstrap and default view exist; reserved category set is `When`, `Entry`, `Done`, but `Entry When Done` trigger category is not implemented. | `crates/agenda-core/src/store.rs`, `spec/product-current.md` |
| S02 Free-Form Item Entry with Date Extraction | partial | Free-form capture and deterministic date parsing work for supported phrases; full natural-language date coverage target is not met. | `crates/agenda-core/src/dates.rs`, `docs/demo-complete-cli-e2e-demo-log.md` |
| S03 Creating a Category and Retroactive Assignment | partial | Retroactive evaluation is implemented when creating categories; threshold/mode policy from v0.6 (`automatic`/`assisted`/`manual`) is not implemented. | `crates/agenda-core/src/agenda.rs`, `spec/product-current.md` |
| S04 Multifiling - One Item, Many Categories | implemented | One item can hold multiple simultaneous assignments and appears across matching views with single-record storage semantics. | `docs/demo-complete-cli-e2e-demo-log.md`, `crates/agenda-core/src/query.rs` |
| S05 View Creation and Edit-Through Semantics | implemented | Include/exclude views, explicit sections, unmatched section, insert-through assign, move/remove semantics, and delete-vs-remove behavior are shipped. | `docs/demo-view-logic-demo-run.md`, `docs/test-script-tui-smoke-e2e.md` |
| S06 Hierarchical Categories with Inheritance | implemented | Parent/child hierarchy, subsumption visibility, and show-children section expansion are implemented. | `crates/agenda-core/src/engine.rs`, `crates/agenda-core/src/query.rs` |
| S07 Automatic Assignment via Profile Conditions | partial | Profile conditions and action cascades exist in engine, but user-facing authoring UX for richer policy management is limited. | `crates/agenda-core/src/engine.rs`, `spec/gaps.md` |
| S08 Mutual Exclusion | implemented | Exclusive sibling constraint is enforced automatically on assignment changes. | `crates/agenda-core/src/engine.rs`, `docs/demo-literate-cli-demo-exclusive-fix-validation.md` |
| S09 Dynamic Date Categories | implemented | Virtual `WhenBucket` filtering and automatic bucket resolution over time are implemented without persisted bucket categories. | `crates/agenda-core/src/query.rs`, `spec/product-current.md` |
| S10 Text Matching with Classification Control | partial | Implicit string matching exists; confidence thresholds, suggestion queue behavior, and assignment modes are deferred. | `crates/agenda-core/src/matcher.rs`, `spec/gaps.md` |
| S11 Recurring Items | deferred | Recurrence model and next-instance generation are not implemented. | `spec/tasks.md` (T050), `spec/gaps.md` |
| S12 View Switching as Primary Navigation | partial | TUI view picker (`F8`) is implemented and usable, including in-app view create/rename/include-edit; sequential next/previous navigation and explicit performance SLO validation are not yet documented. | `crates/agenda-tui/src/lib.rs`, `docs/test-script-tui-smoke-e2e.md` |
| S13 Editing an Item Updates All Views | implemented | TUI inline text edit and note edit are implemented; edits re-evaluate classification and propagate across all views of the same item. | `crates/agenda-tui/src/lib.rs`, `docs/test-script-tui-smoke-e2e.md` |
| S14 Category Reparenting Without Data Loss | implemented | Reparenting and structural edits are available in TUI manager and preserve assignment semantics. | `crates/agenda-tui/src/lib.rs`, `docs/test-script-tui-smoke-e2e.md` |
| S15 Importing Free-Form Text | deferred | Paragraph import workflow is not implemented in CLI/TUI. | `spec/tasks.md`, `spec/gaps.md` |
| S16 Suggested Assignments Review | deferred | Suggestion queue/review (`?`) behavior is not implemented. | `spec/tasks.md` (T051/T052), `spec/gaps.md` |
| S17 Database Resilience - Crash Recovery | partial | SQLite WAL and integrity-oriented store behavior are present; explicit unclean-shutdown reporting/recovery UX is limited. | `crates/agenda-core/src/store.rs`, `spec/gaps.md` |
| S18 Empty Category Cleanup | partial | Category deletion exists, but count-driven empty-category workflows and richer safe-delete options are not fully implemented. | `crates/agenda-cli/src/main.rs`, `spec/gaps.md` |
| S19 Quick-Add from Outside the Application | implemented | CLI quick-add capture path is implemented and feeds the same parse/classification pipeline used by TUI sessions. | `crates/agenda-cli/src/main.rs`, `docs/demo-complete-cli-e2e-demo-log.md` |
| S20 The "Agenda Moment" - Emergence of Structure | partial | Retroactive assignment on new category creation is implemented; explicit background progress UX for large corpus reclassification is not implemented. | `crates/agenda-core/src/agenda.rs`, `docs/demo-complete-cli-e2e-demo-log.md` |
| S21 Understanding Why an Item Is Here | implemented | TUI inspect panel shows assignment provenance and supports in-panel unassign. | `crates/agenda-tui/src/lib.rs`, `docs/test-script-tui-smoke-e2e.md` |
| S22 Undo After Accidental Move | deferred | Undo stack and `Ctrl-Z` recovery flow are not implemented; v1 safety contract is explicit no-undo + recovery UX. | `crates/agenda-core/src/undo.rs`, `spec/decisions.md` |
| S23 Section Gaps and the Unmatched Section | implemented | Generated unmatched section behavior and non-disappearing matching items are implemented. | `crates/agenda-core/src/query.rs`, `docs/demo-view-logic-demo-run.md` |
