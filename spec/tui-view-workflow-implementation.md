# TUI View Workflow Implementation Spec

Date: 2026-02-17
Status: Active implementation contract
Scope: Unified spec for TUI board/view/category workflows, view-manager UX, and column evolution.
Primary reference: `/Users/mds/src/aglet/reference/lotus-agenda-paper.txt`

## 1. Purpose

This document is the single implementation source of truth for TUI view workflows.

It consolidates and supersedes prior split docs for:

- view + category workflow
- view manager V2 design
- column workflow design
- wireframe/mockup behavior

## 2. Product Intent

The TUI should let users capture, organize, and reshape work *through views* without dropping to CLI.

A view has three concerns:

- Selection: which items appear.
- Demarcation: how items are partitioned into sections.
- Annotation: which category/context columns appear beside item text.

## 3. Scope And Non-Goals

In scope:

- TUI layout, mode model, and keyboard behavior
- view create/edit/delete/switch workflows
- category manager + item assignment flows
- section and unmatched behavior
- view-manager redesign path (full-screen)
- column evolution path and persistence gate

Out of scope (for this slice):

- freeform parser-backed boolean persistence model
- CLI grammar expansion for complex view expressions
- schema changes unless the persistence gate is met
- unmatched always-show-empty pin persistence

## 4. UX Principles

- Item-first: board and item actions are primary.
- Explicit save for high-impact editors.
- Full-line highlight in grid/list modes.
- `x` means delete everywhere destructive.
- Laptop-first shortcuts (`v/c/,/.`) with explicit board/preview focus.
- Smallest logically consistent mutation when editing through views.

## 5. Current Shipped Contract (Baseline)

### 5.1 Board Layout

- Sections render as stacked lanes (top-to-bottom).
- Each lane shows title, count, and rows.
- Rows use fixed annotation columns:
  - `When`
  - `Item`
  - `All Categories`
- Header and rows share width layout for separator alignment.
- Rows are compact single-line density.
- Unified preview pane is optional and scrollable; it is not a section selector.
- Preview defaults to `Summary` (categories + note) and can switch to
  `Provenance` details in-pane.

### 5.2 Normal Mode Keys

- `n`: add item in focused lane context
- `Enter` / `e`: open item edit popup
- `m`: note-only edit flow
- `a`: item category assignment picker
- `u`: category picker by default; in preview `Provenance` mode with preview
  focus, opens provenance unassign picker
- `[` / `]`: move item between lanes
- `r`: remove item from current view context
- `d` / `D`: done toggle
- `x`: delete item (confirm)
- `v` / `F8`: view palette
- `c` / `F9`: category manager
- `,` / `.`: view cycling
- `g`: hop to All Items view
- `/`: filter
- `p`: preview pane toggle (combined item summary + inspect/provenance)
- `o`: preview mode toggle (`Summary` / `Provenance`)
- `Tab` / `Shift+Tab`: focus board vs preview pane (when preview is open)
- `q`: quit

### 5.3 Done Toggle Rule

- Marking done is only allowed if the item has at least one assigned actionable category.
- If none are actionable:
  - do not mutate item
  - show status/toast indicating done is unavailable

### 5.4 Item Edit Popup (Current Target UX)

- Centered popup editor
- editable `Text` and multiline `Note` in one flow
- tab-focus navigates controls
- explicit controls include:
  - `Categories` button (opens item category picker)
  - `Save` button
  - `Cancel` button
- Enter in note field inserts newline, not implicit save

### 5.5 Item Category Picker

- checkbox assign/unassign on one surface (`Space`)
- freeform category text entry (`n` or `/`) for assign/create
- parent-removal guard:
  - cannot remove ancestor category if a descendant assignment remains
  - picker stays open; error/status shown
- picker supports scrolling for large category sets

### 5.6 Category Manager

- global category hierarchy management
- full-screen manager surface (replaces prior popup-style manager)
- hierarchy list is always expanded (no collapse/expand state)
- `Enter` opens a centered category-config popup for the selected category
- config popup includes:
  - exclusive checkbox
  - match-category-name checkbox (`enable_implicit_string`)
  - actionable/todo checkbox
  - multiline category note editor
- quick toggles remain available in manager context:
  - `e`: exclusive
  - `i`: match category name
  - `a`: actionable
- reserved categories are read-only for config fields and note
- creation copy must be explicit:
  - `n`: subcategory under selected parent
  - `N`: top-level category (root parent)

### 5.7 View Palette And Editor (Current)

Palette:

- `Enter`: switch view
- `N`: create
- `r`: rename
- `x`: delete
- `e`: edit selected view

Create:

- name input, then category picker
- picker supports include (`+`/`Space`) and exclude (`-`)

Editor:

- include/exclude + virtual include/exclude editing
- section editor
- unmatched settings
- preview count before save

### 5.8 Unmatched Lane Behavior

- `show_unmatched=true` allows unmatched lane
- empty unmatched lane is hidden by default
- unmatched label editable
- always-show-empty pin behavior is deferred

## 6. Update-Through-View Semantics

The TUI should apply the minimal logically consistent mutation implied by context.

- Insert in section/lane:
  - create item and apply lane/view insert assignment semantics
- Remove from view/lane:
  - apply configured unassign semantics only
  - do not delete item from database
- Move between lanes:
  - source remove semantics then destination insert semantics
  - enforce exclusivity constraints
- Delete (`x`): explicit database removal with confirmation

## 7. View Manager V2 (Target Refactor)

## 7.1 Goal

Replace popup-centric view editing with one full-screen manager that handles view list, criteria, sections, and preview.

## 7.2 Information Architecture

Three-pane full-screen surface:

1. Views pane (left)
- list views
- new/rename/delete/clone

2. Definition pane (center)
- boolean criteria builder
- unmatched settings

3. Sections pane (right)
- section list and section detail authoring

Bottom line:

- live preview summary and delta (`+N/-N`)
- explicit save/cancel controls

## 7.3 Keyboard Model (V2)

Global:

- `Tab` / `Shift+Tab`: focus pane
- `j/k`: row navigation in active pane
- `Enter`: open/edit/activate focused row/control
- `Esc`: back out of submode
- `s`: save draft
- `q`: cancel draft and exit manager

Views pane:

- `N`, `r`, `x`, `C`

Definition pane:

- `N`: add row
- `x`: delete row
- `Space`: include/exclude sign toggle
- `a` / `o`: `AND`/`OR`
- `(` / `)`: nesting depth
- `c`: category picker
- `u`: unmatched settings

Sections pane:

- `N`: add section
- `x`: remove section
- `[` / `]`: reorder
- `Enter`: section detail

Section detail:

- `t`: title
- row-level criteria editing same model as definition pane
- `i`: edit `on_insert_assign`
- `r`: edit `on_remove_unassign`
- `h`: toggle `show_children`

## 8. Boolean Criteria Authoring Model

Draft row shape:

- `sign`: include/exclude
- `category_id`
- `join_with_previous`: and/or
- `depth`: nesting

Example:

- `+ Work`
- `AND + Project Y`
- `OR - Someday`

Representability rule:

- If draft can be converted to existing query/section fields, allow save.
- If not losslessly representable, block save with explicit reason.

## 9. Annotation Columns: Baseline And Evolution

## 9.1 Baseline (Shipped)

- fixed columns: `When | Item | All Categories`
- `All Categories` sorted by display name
- comma-separated values
- truncation for overflow

## 9.2 Target Evolution

Support configurable column headings, typically category-family roots:

- `Priority` -> `High, Medium, Low`
- `People` -> `Mike, Dave, Sally`
- `Department` -> `Sales, Marketing, Engineering`

Possible section-specific column overrides are supported as a design direction.

## 9.3 Column Setup UX (Experimental)

- list mode: add/remove/reorder/select column
- detail mode: title, kind, family root, scope

## 10. Persistence Gate For Model Changes

No schema changes by default.

Schema/model changes are justified only if at least one is true:

- custom column sets must persist per view
- section-specific column overrides are required in shipped behavior
- column-interactive edits need metadata unavailable in current fields

Candidate shape if gate is triggered:

- view-level `annotation_columns`
- optional section-level `annotation_columns_override`
- column-kind enum with `category_family` reference

## 11. Validation, Errors, And Empty States

- no views: clear empty state with create action
- invalid criteria row: row-level error marker and blocked save
- section config conflict: mark section row and jump-to-fix
- non-actionable done toggle: no mutation + visible status
- blocked unassign due to descendants: remain in picker + visible status

## 12. Wireframe Mockups

### 12.1 Full-Screen View Manager

```text
Agenda Reborn  screen:View Manager  draft:*unsaved*  view:Project Y Board
┌──────────────────────┬──────────────────────────────────────────────┬──────────────────────────────┐
│ Views                │ Definition                                   │ Sections                     │
│ > All Items          │ Criteria rows                                │ > Slot A                     │
│   Smoke Board        │  1. + Work                                   │   Slot B                     │
│   Project Y Board    │  2. AND + Project Y                          │   Unassigned                 │
│                      │  3. AND - Done                               │                              │
│ [N] New [r] Rename   │ [N] row [x] del [Space] +/- [a/o] AND/OR    │ [N] add [x] del [[/]] move   │
│ [C] Clone [x] Delete │ [()/()] nest [c] category [u] unmatched     │ Enter section detail         │
├──────────────────────┴──────────────────────────────────────────────┴──────────────────────────────┤
│ Preview: 43 matching | Slot A:12 Slot B:19 Unmatched:12 | delta:+3/-1 | [s] Save [q] Cancel         │
└──────────────────────────────────────────────────────────────────────────────────────────────────────┘
```

### 12.2 Criteria Category Picker

```text
┌──────────────────────────────────────────────────────────────┐
│ Pick Category (row 2: AND + ?)                              │
│ filter: proj                                                 │
│ > [ ] Project X                                              │
│   [x] Project Y                                              │
│   [ ] Projects/Archive                                       │
│ Space toggle  Enter choose  / filter  Esc back              │
└──────────────────────────────────────────────────────────────┘
```

### 12.3 Section Detail

```text
┌──────────────────────────────────────────────────────────────┐
│ Section Detail: Slot B                                       │
│ Title: Slot B      show_children: [ ]                        │
│ Rules:                                                       │
│  1. + SlotB                                                  │
│  2. AND - Someday                                            │
│ on_insert_assign   : SlotB                                   │
│ on_remove_unassign : SlotB                                   │
│ N row  x del  Space +/-  a/o AND/OR  i insert-set  r remove-set │
└──────────────────────────────────────────────────────────────┘
```

## 13. Implementation Sequence

1. Keep baseline stable (`When|Item|All Categories`, stacked lanes, item/category flows).
2. Build full-screen View Manager shell with 3-pane navigation.
3. Move existing view create/rename/delete/edit flows into the shell.
4. Implement row-based boolean criteria authoring + validation.
5. Integrate section authoring in same screen.
6. Add preview summary + explicit save/cancel.
7. Add column setup experimental mode (non-persistent first).
8. Evaluate persistence gate and decide model-extension path.

## 14. Acceptance Criteria

- One discoverable in-app surface for full view authoring.
- Include/exclude and boolean composition are low-friction.
- Multi-section view configuration is fully editable in TUI.
- Save is explicit and resilient to accidental keypresses.
- Preview provides confidence before commit.
- Baseline board and item workflows remain stable.
- Tests and smoke scripts cover key paths and edge cases.

## 15. Traceability

Roadmap/task anchors:

- R3: T070-T076
- R3.5: T077-T085
- R3.6: T087-T092

Design decisions:

- Decision 35: aligned grid columns
- Decision 36: stacked sections
- Decision 37: include/exclude picker + tab cycling
- Decision 38: subsumption-safe unassign
- Decision 39: full-screen view manager direction
