# TUI View Manager V2 Mockup

Date: 2026-02-17
Status: Draft wireframe mockup (terminal-first)

## 1. Full-Screen Manager Layout

```text
Agenda Reborn  screen:View Manager  draft:*unsaved*  view:Project Y Board
┌──────────────────────┬──────────────────────────────────────────────┬──────────────────────────────┐
│ Views                │ Definition                                   │ Sections                     │
│                      │                                              │                              │
│ > All Items          │ Criteria rows                                │ > Slot A                     │
│   Smoke Board        │  1. + Work                                   │   Slot B                     │
│   Project Y Board    │  2. AND + Project Y                          │   Unassigned                 │
│   Home Focus         │  3. AND - Done                               │                              │
│                      │                                              │ Section detail (Enter):      │
│ [N] New  [r] Rename  │ [N] add row  [x] delete row                 │ Title: Slot A                │
│ [C] Clone [x] Delete │ [Space] +/-  [a]/[o] AND/OR                 │ Rules: + SlotA               │
│                      │ [(]/[)] nest  [c] pick category             │ on_insert: SlotA             │
│                      │ [u] unmatched settings                       │ on_remove: SlotA             │
├──────────────────────┴──────────────────────────────────────────────┴──────────────────────────────┤
│ Preview: 43 matching  |  Slot A:12  Slot B:19  Unmatched:12  |  delta:+3/-1  |  [s] Save  [q] Cancel │
└──────────────────────────────────────────────────────────────────────────────────────────────────────┘
```

## 2. Category Picker (For Criteria Row)

```text
┌──────────────────────────────────────────────────────────────┐
│ Pick Category (row 2: AND + ?)                              │
│                                                              │
│ filter: proj                                                 │
│ > [ ] Project X                                              │
│   [x] Project Y                                              │
│   [ ] Projects/Archive                                       │
│                                                              │
│ Space toggle  Enter choose  / filter  Esc back              │
└──────────────────────────────────────────────────────────────┘
```

## 3. Unmatched Settings Subview

```text
┌──────────────────────────────────────────────────────────────┐
│ Unmatched Settings                                           │
│                                                              │
│ [x] Enabled                                                  │
│ Label: Unassigned                                            │
│ Hide when empty: ON (phase default)                          │
│ Always-show-empty pin: Deferred                              │
│                                                              │
│ t toggle enabled  l edit label  Esc back                     │
└──────────────────────────────────────────────────────────────┘
```

## 4. Section Detail Subview

```text
┌──────────────────────────────────────────────────────────────┐
│ Section Detail: Slot B                                       │
│                                                              │
│ Title: Slot B                                                │
│ show_children: [ ]                                           │
│                                                              │
│ Rules                                                        │
│  1. + SlotB                                                  │
│  2. AND - Someday                                            │
│                                                              │
│ on_insert_assign      : SlotB                                │
│ on_remove_unassign    : SlotB                                │
│                                                              │
│ N add-rule  x del-rule  Space +/-  a/o AND/OR               │
│ i edit on_insert  r edit on_remove  h toggle children        │
│ Enter open picker  Esc back                                  │
└──────────────────────────────────────────────────────────────┘
```

## 5. Interaction Notes

- The focused pane title is highlighted.
- The focused row is full-line highlighted.
- `Tab` rotates pane focus left -> center -> right.
- Save is only explicit via `s`.
- Enter in note/content fields should never implicit-save the full editor.
