# Domain Gaps For Main-Branch Iterations

Date: 2026-02-16
Scope: underlying/core work not solvable by CLI/TUI phrasing or command wiring alone.

## Purpose

This document captures remaining domain-layer gaps after SLC UI/CLI fixes so we can plan main-branch iterations with clear boundaries.

## Summary

The current branch demonstrates a usable SLC workflow. Remaining gaps are mostly about deeper category/view evolution semantics, restore fidelity choices, and domain APIs for advanced editing flows.

## Gaps Requiring Domain Work

## 1. Full Category Evolution API Surface

Current state:

- Category create/delete/assign flows are available.
- Reparent and toggle semantics exist indirectly via low-level update pathways but are not exposed as robust domain operations with explicit invariants and UX-ready errors.

Needed domain work:

- First-class domain operations for category rename (I think rename is just delete+recreate?), reparent, reorder children, toggle exclusivity, toggle implicit matching.
- Stronger invariant-oriented error model for these operations (cycle, duplicate name, reserved behavior, and hierarchy integrity).

Why this is domain work:

- Correctness depends on hierarchy invariants, not command parsing.
- Multiple frontends should share identical behavior.

## 2. Full View Evolution API Surface

Current state:

- View create/list/delete works.
- View display works via resolver.

Needed domain work:

- First-class domain operations to evolve views safely: rename, update criteria, section edits, column edits, and remove-from-view semantics updates.
- Validation rules for malformed/contradictory view definitions.

Why this is domain work:

- View correctness and edit-through side-effects are core semantics and must be frontend-agnostic.

## 3. Restore Fidelity Policy and Implementation

Current state:

- Restore works and reconstructs assignments where categories still exist.
- Restored items currently get new `created_at`/`modified_at` values (restore-time timestamps).

Needed domain work:

- Explicit policy decision and implementation for restored metadata fidelity:
  - preserve original timestamps from deletion log, or
  - preserve restore-time timestamps but add explicit provenance metadata.
- Optional strict/lenient restore mode for missing categories.

Why this is domain work:

- This affects data model truth and long-term audit semantics.

## 4. Explicit Unassign API and Symmetric Manual Edit Semantics

Current state:

- Manual assign is first-class.
- Manual assign now enforces exclusive sibling cleanup for exclusive parents.
- Unassignment exists in lower-level store/edit-through paths but not as a clear top-level manual domain operation mirrored to assign.

Needed domain work:

- First-class domain API for manual unassign that preserves invariants and predictable cascades.
- Define/lock behavior for ancestor cleanup under subsumption when child assignments are removed.

Why this is domain work:

- Subsumption and exclusivity interactions require engine/store correctness, not just CLI wiring.

## 5. Undo-Ready Mutation Ledger (Without Shipping Undo Yet)

Current state:

- Architecture is intentionally undo-ready, but no formal operation journal is emitted by domain methods.

Needed domain work:

- Standardized mutation events/inverse metadata from domain mutations to enable robust undo later.
- Operation identity and replay/rollback boundaries defined in core.

Why this is domain work:

- Undo correctness depends on canonical mutation representation across all frontends.

## 6. Store Decode Error Strictness

Current state:

- Some row decode paths still use fallback decoding (default UUID/date/JSON) instead of failing hard.

Needed domain work:

- Replace silent fallback decode with typed decode errors and optional quarantine/report strategy.

Why this is domain work:

- This is persistence correctness and data integrity behavior in core storage.

## Suggested Iteration Order (Main Branch)

1. Category/view evolution domain operations (largest product leverage).
2. Unassign + subsumption cleanup semantics.
3. Restore fidelity policy and implementation.
4. Mutation ledger for undo readiness.
5. Decode strictness hardening.

## Notes

- CLI/UI ergonomics can continue in parallel, but should consume these domain operations rather than reimplement behavior in frontends.
- This list aligns with SLC goals in `spec/slc-spec.md`: complete daily use now, deeper semantic hardening in planned main-branch iterations.
