# Domain + Product Gaps (Current Reality)

Date: 2026-02-16
Scope: gaps between `spec/product-spec-complete.md` and the current implementation.

## 1. Purpose

This document tracks what is still missing or mismatched after reviewing:

- core code (`agenda-core`)
- shipped CLI/TUI codepaths
- CLI/TUI demo evidence in `docs/demo-*.md` and `docs/test-*.md`

It focuses on meaningful gaps, not already-shipped features.

## 2. High-Priority Gaps

## 2.1 Spec-to-Model Mismatch (v0.6 vs shipped model)

Current implementation does not yet include several v0.6 model fields/concepts:

- item recurrence fields and recurrence engine behavior
- typed value columns (`values`, category `value_type`, precision)
- suggestion model (`rejected_suggestions`, suggestion acceptance provenance)
- aliases and condition modes on categories
- validation/date/delete condition-action variants
- `Entry When Done` reserved category trigger sequence

Why it matters:

- Product docs currently imply these are available; code does not support them.
- This causes planning and acceptance-test drift.

## 2.2 Safety Contract Gap (No-Undo Contract Hardening)

Current state:

- `crates/agenda-core/src/undo.rs` is effectively empty.
- V1 decision is no-undo + explicit recovery UX, not minimal undo.

Gap:

- Recovery affordances and user guidance are present but not yet fully codified as a coherent no-undo safety contract in all CLI/TUI surfaces.

Why it matters:

- Trust depends on fast, explicit recovery when mistakes happen, especially without `Ctrl-Z`.

## 2.3 TUI Safety/Advanced Workflow Gap (Post-Completion)

Current state:

- TUI supports navigation/add/move/remove/done/delete/filter/inspect/view switching.
- TUI supports in-app view create/rename/include-edit from view picker (`F8`).
- TUI supports direct item-to-category assignment (`a`).
- TUI supports category manager flows (create/rename/reparent/toggle/delete).
- TUI supports inline text and note edits.
- TUI supports inspect-driven unassign.

Gap:

- No in-session undo/revert path in TUI for accidental move/remove/edit operations (intentional for v1 contract).
- No suggestion review UX for assisted classification mode.
- No equivalent of v0.6 advanced workflow tooling (recurrence controls, suggestion acceptance loop).

Why it matters:

- Core daily workflow is covered in TUI, but safety and advanced intelligence loops are still incomplete.

## 2.4 Date Parsing Coverage Gap

Current state:

- Deterministic parser supports useful subset: absolute forms, `today/tomorrow/yesterday`, `this/next <weekday>`, and compound trailing time patterns.

Gap:

- Does not cover full v0.6 examples (recurrence phrases, richer relative chains, broad natural language understanding).

Why it matters:

- Spec acceptance criteria currently overstate date NLP capability.

## 3. Medium-Priority Gaps

## 3.1 View/Category Evolution APIs

Current state:

- CRUD is present; CLI provides create/list/show/delete + basic assign.

Gap:

- No first-class domain API surface for rename/reorder/reparent workflows with dedicated UX-grade errors.
- View mutation is mostly whole-object update; limited guardrails for malformed combinations.

## 3.2 Restore Fidelity Policy

Current state:

- Restore recreates item and replays existing-category assignments.
- Restored `created_at`/`modified_at` use restore-time timestamps.

Gap:

- No explicit, documented policy decision on metadata fidelity and missing-category handling mode.

## 3.3 Store Decode Strictness

Current state:

- Several row decode paths use fallbacks (`unwrap_or_default`) for malformed UUID/date/JSON values.

Gap:

- Corrupt data may be silently coerced instead of surfaced as explicit persistence errors.

## 4. Lower-Priority / Planned-Deferred Gaps

- Suggestion queue and assisted/manual classification modes.
- LLM-backed classification/date understanding.
- Advanced action/validation authoring UX.
- Column computations and typed analytical views.

## 5. Gap Closure Order (Recommended)

1. Lock spec language to shipped model (eliminate doc drift first).
2. Undo/safety decision and implementation path.
3. Domain API maturity for category/view evolution invariants.
4. Persistence/data integrity hardening (decode strictness + restore policy).
5. Advanced v0.6 deferred features (recurrence/suggestions/typed values).

## 6. Notes From Demo Evidence

CLI status is confirmed as operational by:

- `docs/demo-complete-cli-e2e-demo-log.md`
- `docs/demo-view-logic-demo-run.md`
- `docs/test-cross-domain-scenarios-run-results.md`

These logs show real execution of create/list/search/view/category/done/delete/deleted/restore paths and include/exclude query behavior across multiple domains.

TUI completion evidence is captured in:

- `docs/test-script-tui-smoke-e2e.md`
