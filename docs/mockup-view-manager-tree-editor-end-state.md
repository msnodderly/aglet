# Mockup: TUI View Picker + Full-Screen View Editor (End-State UX)

Date: 2026-02-24
Status: Design sketch / implementation mockup (revised split architecture)
Related plan:
- `/Users/mds/src/aglet/docs/plan-view-manager-tree-editor-ux-streamlining.md`
Reference inspiration:
- `/Users/mds/src/aglet/reference/lotus-agenda-view-creation-workflow.md`

## 1. Purpose

This document sketches the intended end-state UX using a split model:

- `v` opens a lightweight **View Picker** for fast switching and simple CRUD
- `e` (from picker) opens a separate full-screen **View Editor** for deep view/section editing

This keeps the quick path simple while still giving us a category-manager-style editing surface.

Design goals:

- Preserve a very fast view-switch flow (Lotus `F8` spirit)
- Harmonize the deep editor with the category manager tree editor (`c` / `F9`)
- Make section creation/editing direct and field-based
- Keep keyboard model consistent (`Tab`, `j/k`, `Enter`, `Esc`, `S`)

## 2. Architecture (Split UX)

### 2.1 Screen A: Quick View Picker (`v` / `F8`)

Primary use cases:

- switch views quickly
- create a view
- rename/delete a view
- open full-screen editor for selected view

This screen should stay intentionally minimal and non-busy.

### 2.2 Screen B: Full-Screen View Editor (opened from picker)

Primary use cases:

- edit view criteria / unmatched settings
- create/reorder/remove sections
- edit section criteria/columns/edit-through rules
- save/cancel a draft explicitly

Default editor layout should be **2 panes** (Sections + Details), with optional/toggled Preview and Filter surfaces.

## 3. Lotus Agenda Patterns To Borrow (Without Copying Literally)

From `/Users/mds/src/aglet/reference/lotus-agenda-view-creation-workflow.md`:

- Lightweight view manager for quick switching, with a separate deeper properties/edit path
- Progressive disclosure: quick command path and full properties path
- Fast section insertion shortcuts relative to current section (above/below)
- Category matching first, chooser/picker fallback

Aglet adaptation:

- Keep `v` quick and lightweight
- Use a modern full-screen editor instead of modal property boxes
- Support quick section add above/below current row in the editor tree
- Prefer typing + matching/suggestions, with picker overlays as fallback

## 4. Screen A: Quick View Picker (Default `v` Screen)

Notes:

- This is not the full editor
- It should remain easy to parse at a glance
- Focused list border shown with `*...*` title below

```text
+-----------------------------------------------------------------------------------------------+
| View Picker                                                                                   |
+-----------------------------------------------------------------------------------------------+
| *VIEWS*                                                                                       |
|-----------------------------------------------------------------------------------------------|
| > Backlog                                                                                     |
|   Personal                                                                                    |
|   Someday/Maybe                                                                               |
|   All Items                                                                                   |
|                                                                                               |
| 4 views                                                                                       |
+-----------------------------------------------------------------------------------------------+
| Enter:switch  N:new  r:rename  x:delete  e:edit(full-screen)  Esc:back                       |
+-----------------------------------------------------------------------------------------------+
```

### 4.1 Optional Picker Enhancements (Still Keep It Simple)

Allowed if they do not add visual clutter:

- type-to-filter (single-line filter at bottom)
- small metadata on selected view (match count / section count) in status line only
- `V` alias for edit (existing compatibility)

Not recommended for this screen:

- section tree
- details pane
- preview pane
- heavy draft state UI

## 5. Screen B: Full-Screen View Editor (Default 2-Pane Layout)

This is the category-manager-style editing surface.

Notes:

- Focused pane border = cyan (represented with `*...*` title)
- Inactive pane border = blue (represented with normal title)
- Overlay border = yellow (represented with `! ... !`)
- Preview is hidden by default; can be toggled with `p`
- Filter is lightweight and can be toggled/focused (not always a full pane)

```text
+-----------------------------------------------------------------------------------------------+
| View Editor: Backlog (draft)                                                  dirty: yes      |
+--------------------------------------+--------------------------------------------------------+
| *SECTIONS*                           | Details                                                |
|--------------------------------------|--------------------------------------------------------|
| > [S] Triage                         | Selected: Section "Triage"                            |
|   [S] In Progress                    |--------------------------------------------------------|
|   [S] Blocked                        | > Title                Triage                          |
|   [S] Done Review                    |   Criteria             +Pending +High                  |
|                                      |   Columns              Priority, Area, Owner           |
|   [U] Unmatched (generated)          |   On Insert Assign     In Progress                     |
|                                      |   On Remove Unassign   In Progress                     |
|                                      |   Show Children        no                              |
|                                      |   Display Override     inherit                         |
|                                      |                                                        |
|                                      | View-level (when a view row is selected):             |
|                                      | Name / Criteria / When buckets / Unmatched / Display  |
+--------------------------------------+--------------------------------------------------------+
| Status: n:add below  N:add above  r:rename  J/K:reorder  Enter:edit field  p:preview  /:filter |
+-----------------------------------------------------------------------------------------------+
| Tab:pane  j/k:navigate  S:save draft  Esc:back (prompts if dirty)                             |
+-----------------------------------------------------------------------------------------------+
```

### 5.1 Why This Is Less Busy Than The Previous Mockup

- No always-visible tree of all views in the editor
- No always-visible filter pane
- No always-visible preview pane
- Focus is on current view’s sections + details, which is the primary editing task

## 6. View Editor With Preview Toggled On (`p`)

Preview is optional and should appear only when requested.

Recommended behavior:

- `p` toggles preview pane on/off
- `Tab` cycles panes (`Sections` -> `Details` -> `Preview`)
- Preview is read-only summary/counts and lane preview (no heavy editing)

```text
+-----------------------------------------------------------------------------------------------+
| View Editor: Backlog (draft)                                                  dirty: yes      |
+------------------------------+----------------------------------+-----------------------------+
| SECTIONS                     | *Details*                        | Preview                      |
|------------------------------|----------------------------------|-----------------------------|
| > [S] Triage                 | Selected: Section "Triage"      | Summary                      |
|   [S] In Progress            | > Criteria   +Pending +High     | - matches now: 42            |
|   [S] Blocked                |   Columns    Priority, Area     | - explicit sections: 4       |
|   [S] Done Review            |   Show Chld  no                 | - unmatched visible: yes     |
|   [U] Unmatched (generated)  |   Disp Ovr   inherit            |                              |
|                              |                                  | Lane preview                 |
|                              |                                  | 1. Triage (12)               |
|                              |                                  | 2. In Progress (8)           |
|                              |                                  | 3. Blocked (1)               |
|                              |                                  | 4. Done Review (3)           |
|                              |                                  | 5. Unassigned (18)           |
+------------------------------+----------------------------------+-----------------------------+
| Tab:pane  j/k:move  Enter:edit/toggle  S:save  p:hide preview  Esc:back                        |
+-----------------------------------------------------------------------------------------------+
```

## 7. View Editor Filter (Lightweight, Not A Permanent Pane)

The editor should support filtering section rows (and later details fields if useful), but not require a dedicated permanent filter pane.

Recommended UI:

- `/` focuses a compact filter bar below the sections pane (or status line mode)
- `Esc` clears filter if active; otherwise backs out of current layer

### 7.1 Filter Active (Compact Bar)

```text
+--------------------------------------+--------------------------------------------------------+
| *SECTIONS*                           | Details                                                |
|--------------------------------------|--------------------------------------------------------|
| > [S] In Progress                    | Selected: Section "In Progress"                       |
|   [S] Blocked                        | ...                                                    |
|                                                                                               |
+--------------------------------------+--------------------------------------------------------+
| Section filter: prog   (2 rows shown)   Enter:keep  Esc:clear                                 |
+-----------------------------------------------------------------------------------------------+
```

## 8. New View Flow (Auto-Create First Section)

This flow starts in the **Quick View Picker**, then transitions to the **full-screen View Editor**.

Recommended flow:

1. `v` opens View Picker
2. `N` starts inline create/rename for view name (picker context)
3. `Enter` confirms view name
4. System creates the view and auto-creates first section (`Main`)
5. Full-screen View Editor opens on that view
6. First section title is immediately in inline edit

### 8.1 Picker: Create View Name (Inline)

```text
+-----------------------------------------------------------------------------------------------+
| View Picker                                                                                   |
+-----------------------------------------------------------------------------------------------+
| *VIEWS*                                                                                       |
|-----------------------------------------------------------------------------------------------|
|   Backlog                                                                                     |
|   Personal                                                                                    |
| > New View_|                                                                                  |
|                                                                                               |
+-----------------------------------------------------------------------------------------------+
| Create view: type name, Enter confirm, Esc cancel                                             |
+-----------------------------------------------------------------------------------------------+
```

### 8.2 Editor Opens With First Section Title Editing

```text
+-----------------------------------------------------------------------------------------------+
| View Editor: Sprint Board (draft)                                              dirty: yes      |
+--------------------------------------+--------------------------------------------------------+
| *SECTIONS*                           | Details                                                |
|--------------------------------------|--------------------------------------------------------|
| > [S] Main_|                         | Selected: Section (new)                                |
|   [U] Unmatched (generated)          |--------------------------------------------------------|
|                                      | > Title                Main                            |
|                                      |   Criteria             (none; matches all in view)     |
|                                      |   Columns              (none)                          |
|                                      |   On Insert Assign     (none)                          |
|                                      |   On Remove Unassign   (none)                          |
|                                      |   Show Children        no                              |
|                                      |   Display Override     inherit                         |
+--------------------------------------+--------------------------------------------------------+
| Name first section: type title, Enter confirm, Esc cancel                                     |
+-----------------------------------------------------------------------------------------------+
```

## 9. Section List Quick Actions (Lotus-Inspired Above/Below Add)

Borrowing the Lotus idea of fast insertion relative to the current section, adapted to Aglet keys.

Recommended defaults (editor `Sections` pane):

- `n`: add section **below** current row, then start title edit
- `N`: add section **above** current row, then start title edit
- `J/K`: reorder selected section down/up
- `r`: rename selected section
- `x`: delete selected section (inline confirm)

### 9.1 Add Section Below (`n`)

```text
+--------------------------------------+--------------------------------------------------------+
| *SECTIONS*                           | Details                                                |
|--------------------------------------|--------------------------------------------------------|
|   [S] Triage                         | Selected: Section (new)                                |
| > [S] New section_|                  | > Title                New section                     |
|   [S] In Progress                    | ...                                                    |
|   [S] Blocked                        |                                                        |
+--------------------------------------+--------------------------------------------------------+
| Add section below: type title, Enter confirm, Esc cancel                                      |
+-----------------------------------------------------------------------------------------------+
```

### 9.2 Add Section Above (`N`)

```text
+--------------------------------------+--------------------------------------------------------+
| *SECTIONS*                           | Details                                                |
|--------------------------------------|--------------------------------------------------------|
|   [S] Triage                         | Selected: Section (new)                                |
| > [S] New section_|                  | > Title                New section                     |
|   [S] In Progress                    | Inserted above previous current row                     |
|   [S] Blocked                        |                                                        |
+--------------------------------------+--------------------------------------------------------+
| Add section above: type title, Enter confirm, Esc cancel                                      |
+-----------------------------------------------------------------------------------------------+
```

## 10. Editing View-Level Properties (Same Editor, Different Selection)

To keep the editor focused but powerful, allow selecting a synthetic view row/header in the sections pane.

Recommended row types in `SECTIONS` pane:

- `[V] View Properties` (synthetic row for the current view)
- `[S]` explicit section rows
- `[U] Unmatched (generated)` informational row / shortcut to unmatched settings (optional)

### 10.1 View Properties Row Selected

```text
+--------------------------------------+--------------------------------------------------------+
| SECTIONS                             | *Details*                                              |
|--------------------------------------|--------------------------------------------------------|
| > [V] View Properties                | Selected: View "Backlog"                              |
|   [S] Triage                         |--------------------------------------------------------|
|   [S] In Progress                    | > Name                 Backlog                         |
|   [S] Blocked                        |   Criteria             +Work -Done                     |
|   [S] Done Review                    |   When Include         Today, ThisWeek                 |
|   [U] Unmatched (generated)          |   When Exclude         (none)                          |
|                                      |   Display Mode         single-line                     |
|                                      |   Unmatched Visible    yes                             |
|                                      |   Unmatched Label      Unassigned                      |
+--------------------------------------+--------------------------------------------------------+
| View properties: Enter edits field, j/k moves field                                           |
+-----------------------------------------------------------------------------------------------+
```

## 11. Category Picker Overlay (Criteria / Columns / Assign Sets)

Overlay should preserve editor context and be shown only when needed.

```text
+--------------------------------------+------------------------! Pick Categories !-------------+
| SECTIONS                             | Details                | Filter: pri                  |
| ...                                  | ...                    |------------------------------|
| > [S] Triage                         | > Criteria ...         | > [x] Priority               |
|                                      |                        |   [x] High                   |
|                                      |                        |   [ ] Medium                 |
|                                      |                        |   [ ] Low                    |
|                                      |                        |   [ ] Projects               |
|                                      |                        |   [ ] Personal               |
|                                      |                        |                              |
|                                      |                        | Space:toggle  Enter:done     |
|                                      |                        | / or type:filter  Esc:done   |
+-----------------------------------------------------------------------------------------------+
```

Notes:

- Prefer category matching / filtering first, picker traversal second (Lotus-inspired)
- Reuse category manager picker vocabulary where possible
- For criteria modes (`+`, `-`, `|`), exact UI can be decided separately (field-level mode editor vs overlay mode toggle)

## 12. Delete Confirmation (Inline, Editor-Scoped)

Delete should not open a separate screen. Keep it inline and local to the selected row.

### 12.1 Delete Section

```text
+--------------------------------------+--------------------------------------------------------+
| *SECTIONS*                           | Details                                                |
|--------------------------------------|--------------------------------------------------------|
|   [S] Triage                         | Selected: Section "Blocked"                            |
|   [S] In Progress                    |                                                        |
| > [S] Blocked                        |                                                        |
|   [S] Done Review                    |                                                        |
+--------------------------------------+--------------------------------------------------------+
| Delete section "Blocked"? y/n                                                              |
+-----------------------------------------------------------------------------------------------+
```

### 12.2 Delete Last Section (Allowed, Explicit Consequence)

```text
+-----------------------------------------------------------------------------------------------+
| Deleted section "Main". View has no sections; items will appear in unmatched if enabled.      |
+-----------------------------------------------------------------------------------------------+
```

## 13. Dirty Draft Discard Prompt (`Esc`)

`Esc` should back out one layer first. At editor top level with a dirty draft, prompt before discard.

```text
+--------------------------------------+--------------------------------------------------------+
| SECTIONS                             | Details                                                |
| ...                                  | ...                                                    |
+--------------------------------------+--------------------------------------------------------+
| ! Discard Draft Changes? !                                                                  |
| Press y to discard and close, n/Esc to continue editing                                      |
+-----------------------------------------------------------------------------------------------+
```

## 14. Empty / Edge States

### 14.1 Picker: No Views Yet

```text
+-----------------------------------------------------------------------------------------------+
| View Picker                                                                                   |
+-----------------------------------------------------------------------------------------------+
| *VIEWS*                                                                                       |
|-----------------------------------------------------------------------------------------------|
| (no views)                                                                                    |
|                                                                                               |
| Press N to create your first view                                                             |
+-----------------------------------------------------------------------------------------------+
| N:new view  Esc:back                                                                          |
+-----------------------------------------------------------------------------------------------+
```

### 14.2 Editor: Filter Active, No Sections Match

```text
+--------------------------------------+--------------------------------------------------------+
| *SECTIONS*                           | Details                                                |
|--------------------------------------|--------------------------------------------------------|
| (no matching sections)               | No section selected                                    |
|                                      | Select [V] View Properties or clear filter             |
+--------------------------------------+--------------------------------------------------------+
| Section filter: xyz   (0 rows shown)   Esc:clear                                             |
+-----------------------------------------------------------------------------------------------+
```

## 15. Narrow Terminal Fallback (Editor)

If width is too narrow, keep the split architecture but simplify the editor:

- 2-pane remains preferred (`Sections` + `Details`)
- Preview collapses into footer summary (`match:42 lanes:5`)
- Filter stays compact inline bar

Example:

```text
+--------------------------------------+--------------------------------------+
| *SECTIONS*                           | Details                              |
| ...                                  | ...                                  |
+--------------------------------------+--------------------------------------+
| Section filter: pending (4 rows)                                            |
+------------------------------------------------------------------------------+
| match:42 lanes:5 dirty:yes  Tab:pane  j/k:move  Enter:edit  S:save  Esc      |
+------------------------------------------------------------------------------+
```

## 16. Implementation Notes (Mapping To Current Code)

This mockup assumes a split end-state:

- lightweight `ViewPicker` remains (current `v` flow)
- full-screen `ViewEditor` is a separate mode/surface

Immediate changes that can ship first (without full rewrite):

- auto-create first section on TUI view create
- open editor focused on first section title edit
- `N` in sections starts inline title edit immediately
- dirty discard confirm on `Esc`

These move the UX toward this design while preserving the simple quick-switch picker.
