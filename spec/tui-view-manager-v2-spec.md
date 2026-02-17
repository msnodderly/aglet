# TUI View Manager V2 Spec

Date: 2026-02-17
Status: Draft for next implementation slice
Scope: Full-screen TUI workflow for creating/editing views with boolean criteria and section configuration.

## 1. Objective

Replace the current popup-style view manager with a low-friction, full-screen editor that supports:

- explicit include/exclude authoring
- boolean grouping (`AND`/`OR`) for criteria
- multi-section view configuration in one place
- immediate preview of view impact before save

This is a UI/workflow contract first. Core model/store changes are deferred unless a persistence gate is met.

## 2. Design Constraints

- Keep existing model fields usable in this phase (`criteria`, `sections`, `show_unmatched`, `unmatched_label`).
- Avoid introducing a parser-driven freeform query language in V2. Start with structured expression rows.
- Preserve keyboard-first interaction.
- Make every destructive action explicit (`x` + confirm).

## 3. Primary UX Problems To Solve

- View creation/editing is currently fragmented and hard to discover.
- Include/exclude logic is underpowered and unclear.
- Section editing is not integrated into the same flow as criteria editing.
- Users cannot easily predict resulting item set before saving.

## 4. Information Architecture

View Manager becomes a dedicated full-screen mode (`Mode::ViewManagerScreen`) with 3 panes:

1. Views Pane (left)
- list existing views
- create/rename/delete/clone actions

2. Definition Pane (center)
- criteria builder (boolean groups)
- unmatched settings

3. Sections Pane (right)
- section list and per-section criteria
- section ordering and section behavior toggles

A one-line live preview/status bar appears at bottom.

## 5. Keyboard Model

Global in View Manager:

- `Tab` / `Shift+Tab`: move active pane
- `j/k`: move row in active pane
- `Enter`: open/edit focused row
- `Esc`: back/close current submode
- `s`: save draft
- `q`: cancel draft and exit manager

Views Pane:

- `N`: new view
- `r`: rename view
- `x`: delete view (confirm)
- `C`: clone view

Definition Pane (criteria):

- `N`: add rule row
- `x`: delete rule row
- `Space`: toggle include/exclude sign on row (`+`/`-`)
- `a`: set join operator to `AND`
- `o`: set join operator to `OR`
- `(` / `)`: indent/outdent row (group nesting)
- `c`: category chooser for current row
- `u`: unmatched settings

Sections Pane:

- `N`: add section
- `x`: remove section
- `[` / `]`: move section up/down
- `Enter`: open section detail

Section Detail:

- `t`: edit title
- `N`: add section rule row
- `x`: delete section rule row
- `Space`: toggle section rule sign (`+`/`-`)
- `a` / `o`: set section rule join (`AND`/`OR`)
- `i`: edit `on_insert_assign`
- `r`: edit `on_remove_unassign`
- `h`: toggle `show_children`

## 6. Criteria Builder Data Shape (UI Draft)

A criteria draft is represented as ordered rows:

- `sign`: `include` or `exclude`
- `category_id`
- `join_with_previous`: `and` or `or`
- `depth`: integer nesting level

Example (readable):

- `+ Work`
- `AND + Project Y`
- `OR  - Someday`

Nested example:

- `+ Work`
- `AND (`
- `  + Project Y`
- `  OR + Project X`
- `)`
- `AND - Done`

In this phase, nested rows are evaluated via deterministic conversion to existing query representation where possible. If expression cannot be represented losslessly, save is blocked with explicit message.

## 7. Save Contract

On `s` (save):

1. Validate criteria draft
2. Validate section definitions
3. Persist through existing store update path
4. Refresh active board and preserve selected view/item if possible

Validation errors are non-destructive and keep user in editor.

## 8. Preview Contract

Preview updates after each edit action (debounced if needed):

- matching item count for whole view
- per-section item counts
- unmatched count
- delta vs currently persisted definition (`+N/-N`)

Preview is advisory only and never mutates items.

## 9. Unmatched Behavior (Phase Default)

- `show_unmatched=true` means unmatched lane is eligible.
- empty unmatched lane is hidden by default.
- label is editable via unmatched settings.
- always-show-empty pin mode remains deferred.

## 10. Multi-Section Policy

Multiple sections remain a supported feature.

Rationale:

- Needed for board workflows where users want explicit lanes with different insertion/removal behavior.
- Keeps compatibility with Lotus-style demarcation model and current agenda-core section shape.

## 11. UI States And Empty States

- No views: show one-callout with `N create view`.
- Invalid row/category reference: row highlighted with error marker; save blocked.
- Conflicting section definitions: section row marked with reason and quick jump key (`Enter`) to fix.

## 12. Implementation Plan (Proposed)

1. Add full-screen View Manager shell with 3-pane layout and pane navigation.
2. Move existing create/rename/delete flows into Views Pane.
3. Add row-based include/exclude criteria editor (flat rows, no nesting yet).
4. Add section list/detail editor in same screen.
5. Add preview summary line and save/cancel workflow.
6. Add optional nesting (`depth`) and conversion guard rails.
7. Add regression tests for key navigation, row editing, save validation, and section reorder.

## 13. Acceptance Criteria

- View editing is discoverable from one full-screen place.
- User can create and edit include/exclude rows without mode confusion.
- User can manage multiple sections and their behavior in the same workflow.
- Save is explicit (`s`), not accidental.
- Preview gives immediate confidence before commit.
- Existing model/store compatibility is preserved for this slice.

## 14. Out Of Scope (V2 Slice)

- Arbitrary typed freeform boolean parser in persistence format.
- Per-section annotation-column overrides (tracked in column workflow stream).
- New CLI query grammar.
