# Standard Views Plan: Sections, Columns, and View Manager

Date: 2026-02-18
Scope: Standard views only (Datebook and Show views are explicitly out of scope)
Primary input: external Lotus Agenda documentation (not included in repo)

## 1. Objective

Implement Standard-view behavior so Aglet can:

- display sections as first-class, user-configurable groups
- display configurable columns per section (with a mandatory item column)
- provide a view manager that supports the Standard-view operations described in the Lotus summary

## 2. Current Baseline (What Exists Today)

- View CRUD exists (CLI/TUI).
- Sections exist and render as stacked lanes.
- Board annotation is fixed to `When | Item | All Categories`.
- `View.columns` persists in storage, but board rendering does not use it.
- View manager supports: switch focus, create, rename, delete, clone, edit criteria/sections, unmatched settings.

## 3. Gaps vs. Standard View Requirements

### 3.1 Section Display Gaps

- No section sorting modes (none/category/alphabetic/numeric + direction).
- No per-section item sort override.
- No view-level display toggles (hide empty sections, hide inherited items, hide done items, hide column heads, section separators, item numbering).
- No per-section filter surface (only criteria-based section query editing).

### 3.2 Column Display Gaps

- Board ignores persisted `view.columns`.
- No per-section column layout.
- No column manager UX (add/remove/move/width/type).
- No column types beyond current fixed text rendering.

### 3.3 View Manager Issues (Against Summary Behavior)

- Missing **sort view names** action.
- Missing **reposition view** action with persistent manual order.
- No dedicated **view properties** editor for Standard display settings.
- No section/column property surface in manager.
- View list ordering is currently DB name-order, not insertion order with optional sort toggle.

## 4. Implementation Strategy (Phased)

### Phase 0: Contract + Data Shape Decisions

- Lock Standard-only contract in spec:
  - define exactly which Lotus settings are in this slice
  - defer Datebook/Show-specific settings
- Decide persistence approach:
  - if we need per-section columns and ordering, trigger schema/model extension now
- Produce migration plan with backward-compatible defaults.

### Phase 1: View Manager Foundation Fixes

- Add persistent view ordering (`sort_order`) for views.
- Add manager actions:
  - `Sort names` (one-shot alphabetical rewrite of view order)
  - `Reposition` (`[`/`]` on views pane)
- Keep current clone/create/rename/delete flows; integrate into ordered list semantics.
- Add save/dirty behavior coverage for view-order edits.

Acceptance for Phase 1:
- Manual order persists across restart.
- Sort names works and persists.
- Reposition works and persists.

### Phase 2: Standard View Properties Surface

- Add Standard-view properties model and manager UI:
  - hide empty sections
  - hide done items
  - hide inherited items
  - hide column heads (after first section)
  - section separators
  - number items
  - section sorting + direction
  - default item sorting
- Add section-level override fields:
  - section item sorting override
- Wire properties into resolve + render pipeline.

Acceptance for Phase 2:
- Properties are editable in manager and persist.
- Render output changes according to toggles/sort settings.

### Phase 3: Column Model MVP (Standard Columns)

- Replace fixed board annotation with schema-driven columns.
- Introduce column definition for Standard scope:
  - mandatory item column (cannot remove)
  - optional category columns (initially “Standard” style values)
  - width + order
- Use existing `view.columns` as migration base where possible.

Acceptance for Phase 3:
- Board renders from configured columns.
- Item column always present.
- Column width/order persist and render correctly.

### Phase 4: Per-Section Columns

- Add per-section column overrides (or full per-section column sets).
- Manager section detail gains “Columns” entry.
- Rendering uses section-specific columns if defined, else view default columns.

Acceptance for Phase 4:
- Two sections in same view can display different columns.
- Persistence + edit-through behavior covered by tests.

### Phase 5: View Manager Column UX

- Add column editor flow in manager:
  - add/remove column
  - reorder
  - set width
  - choose target category for Standard columns
- Show live preview deltas after column edits.

Acceptance for Phase 5:
- Full column authoring is available without leaving manager.
- Preview matches final saved board output.

## 5. Technical Design Notes

### 5.1 Core/Storage

Likely touches:

- `crates/agenda-core/src/model.rs`
- `crates/agenda-core/src/store.rs`
- `crates/agenda-core/src/query.rs`

Expected changes:

- Add view ordering field.
- Extend `View`/`Section` settings for Standard properties.
- Extend column model to represent Standard columns and per-section overrides.

### 5.2 TUI

Likely touches:

- `crates/agenda-tui/src/lib.rs`

Expected changes:

- View manager views-pane actions for sort/reposition.
- Properties editor mode(s) for Standard settings.
- Section detail additions for sort/filter/columns.
- Board rendering pipeline refactor from fixed columns to configured columns.

## 6. Testing Plan

- Core unit tests:
  - migration/defaulting
  - view order persistence
  - section sorting + filtering behavior
  - per-section column override resolution
- TUI unit tests:
  - manager key flows (sort/reposition/properties/columns)
  - board rendering alignment with dynamic columns
- Smoke script updates:
  - end-to-end: create view, reorder views, configure sections/columns, restart, verify persistence

## 7. Proposed Delivery Order

1. Phase 1 (manager ordering/sort)  
2. Phase 2 (Standard properties)  
3. Phase 3 (column model MVP + render from view columns)  
4. Phase 4 (per-section columns)  
5. Phase 5 (column UX polish)

This order gives early user-visible value and de-risks data-model changes before full column UX complexity.
