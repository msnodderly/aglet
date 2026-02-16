# Agenda Reborn - SLC Spec (v0.1)

Status: Draft
Date: 2026-02-16

## 1. Why SLC Now

We are shifting from MVP framing to **SLC (Simple, Lovable, Complete)**:

- **Simple**: small enough to ship quickly and learn from real use.
- **Lovable**: the core loop feels good, fast, and trustworthy.
- **Complete**: a user can actually run their day in it without external scaffolding.

This keeps MVP speed while avoiding a "half-tool" that cannot stand on its own.

## 2. Current State (Code + Issues)

Source of truth used:

- `spec/mvp-spec.md`
- `spec/mvp-tasks.md`
- `spec/product-spec-v0.6.md`
- `spec/tasks.md`
- `br list --all`
- current code in `crates/*`

Current implementation baseline:

- `br list --all` shows closed tasks through **T032** (not just T029).
- `agenda-core` is substantially implemented and tested.
- `cargo test` passes with **158/158** tests in `agenda-core`.
- `agenda-cli` is still a placeholder.
- `agenda-tui` is still a placeholder.

Interpretation:

- The **domain engine is real** (store, matcher, rules, query, date parsing, integration layer).
- The **user-facing product is not yet real** (no usable CLI/TUI workflows).

## 3. Are We Blocked From Starting Prototype UI?

**No hard blocker.**

You can start prototype UI work immediately because core dependencies for UI are already present:

- persistent store and first-launch bootstrap
- query/view resolution
- edit-through APIs in `Agenda`
- date parsing + classification + retroactive assignment

What can still cause rework if not decided now:

1. SLC control plane choice: CLI-first configuration vs full TUI config first.
2. Spec alignment gaps between MVP and v0.6 (example: reserved categories differ).
3. SLC "complete" bar (especially inspect/undo/safety requirements for v1).

## 4. SLC v1 Product Definition

SLC v1 is complete when a single user can do all of the following without manual DB edits:

1. Capture quickly from terminal (`add` flow).
2. Open TUI and work from views as the primary interface.
3. Create and evolve categories/views in-product.
4. Re-file by moving items through sections (edit-through semantics).
5. Mark done, remove from view, and delete safely.
6. Understand why an item appears where it appears (provenance).
7. Recover from mistakes (at least one safe rollback path).

## 5. Scope Cutline

### In Scope for SLC v1

- Existing core behavior through T032.
- CLI daily commands: add/list/search/done/deleted (T033-T038).
- TUI core + view switching + input/edit-through daily loop (T039-T049).
- Category management sufficient for real usage (minimum create/rename/reparent/delete and toggles).
- Item edit + note edit from TUI.
- Inspect/provenance visibility.
- Minimal undo or equivalent safety net plus deletion-log access.
- Hardening for empty states, crash safety checks, and key engine invariants.

### Deferred from SLC v1

- Recurrence.
- LLM classifier/date parser.
- Suggestion review queue.
- Advanced category rule editor UX (if basic category management is already shipped).
- Column computations and other Phase 2 features.

## 6. Recommended Path From Here

### Phase A - SLC Alignment (short, before heavy UI coding)

Goal: prevent product/spec churn.

- Lock SLC "complete" criteria (this doc).
- Resolve MVP vs v0.6 mismatches explicitly.
- Create new `br` issue set for SLC execution order.

### Phase B - Usable Control Plane (CLI)

Goal: product is operable even before advanced TUI features.

- Implement T033-T038.
- Add missing structural commands if needed for completeness (category/view create/update flows).

Rationale: this gives us a working "admin/config" surface quickly while TUI matures.

### Phase C - Prototype UI -> Daily Driver UI

Goal: move from read-only prototype to true daily-use interface.

- Prototype slice: T039-T043 (read-only navigation + view switch).
- Make it useful: T044-T049 (input, move, remove, delete, done).
- Complete baseline editing: item text/note editing.

### Phase D - Lovable + Trustworthy

Goal: users trust automation and feel safe using it.

- Inspect/provenance (T058).
- In-view search/filter (T059).
- Safety baseline without undo: deletion confirmations + deletion-log visibility/recovery path.
- Hardening tasks T062-T067 minimum before SLC release.

## 7. SLC Release Criteria (Scenario-Oriented)

SLC v1 release should satisfy these scenario groups from `product-spec-v0.6.md`:

- First launch and immediate capture: Scenarios 01, 02.
- Retroactive organization and multifiling: Scenarios 03, 04, 20.
- Views as interface and edit-through: Scenarios 05, 12, 13, 23.
- Trust and safety: Scenarios 21, 22.
- Quick capture outside TUI: Scenario 19.

If those are working in real usage, we have a complete simple product.

## 8. Immediate Next `br` Planning Suggestion

Create a new SLC-labeled issue batch in this order:

1. CLI usable workflow (T033-T038 + any missing category/view CLI ops).
2. TUI prototype (T039-T043).
3. TUI daily-use edit-through (T044-T049).
4. Category/view management required for completeness.
5. Inspect/search + hardening gate (undo can follow as post-SLC).

## 9. Locked Decisions (2026-02-16)

1. **CLI-first configuration: YES.** Build CLI as the primary early control plane while TUI focuses on daily operation UX.
2. **Undo required for SLC v1: NO.** SLC v1 may ship without Ctrl-Z, but architecture must preserve a clean path to add undo later.
3. **Reserved categories for SLC v1: MVP set only.** Lock to `When`, `Entry`, `Done`. Defer `Entry When Done`.
4. **Advanced condition/action editor UX in SLC v1: DEFER.** Ship basic category management first.

## 10. Architectural Guardrails From Decisions

To honor decision #2 (no undo in SLC v1, but no dead-end architecture):

- Keep all user mutations routed through the `Agenda` orchestration layer, not ad hoc store writes in UI code.
- Keep operation boundaries explicit (create/update/delete/assign/unassign/move) so inverse operations can be attached later.
- Preserve provenance and deletion-log fidelity for every destructive path.
- Avoid UI-specific mutation logic that bypasses domain APIs.

---

This SLC plan keeps scope small but ships a complete product people can actually use, not just a demonstrator.
