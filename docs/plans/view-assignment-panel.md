---
title: View/Section Assignment Pane (Proposal)
status: shipped
created: 2026-03-22
shipped: 2026-03-22
---

# Proposal: View/Section Assignment Pane in the `a` Panel

## Summary

Extend the existing "assign item" panel (opened with `a`) from a single category
list into a two-pane layout. The left pane retains the current category picker.
The right pane is new: it shows every view and its sections, indicating where the
selected item(s) currently land, and allows direct reassignment. Both panes show
live previews of the changes the other pane would cause before any key is pressed.

---

## Motivation

The `r` key and `[`/`]` shortcuts already let users move items between sections
within the *current* view. There is no way to see or change an item's placement
across *all* views in one place, nor to understand which categories drive that
placement. Fusing both into a single panel removes the need to switch views and
makes the category ↔ section relationship visible rather than implicit.

---

## UI Mockups

All mockups use a popup sized `centered_rect(88, 72, frame.area())` (wider than
the current 72 % to give both panes comfortable room). The active pane gets the
existing cyan border style; the inactive pane uses the dimmed border style already
used elsewhere in the app.

### Left pane active — single item, no hover preview

```
Edit item assignment  (Tab switches pane · Space applies · n or / types · Enter close · Esc cancel)
┌─ Categories ──────────────────────────────────┐ ┌─ View/Section ─────────────────────────────┐
│     [x]   Work [reserved]                     │ │  Work Board                                │
│     [ ]     Engineering                       │ │     [x]   Backlog                          │
│ >   [x]     Design                            │ │     [ ]   In Progress                      │
│     [ ]   Personal [reserved]                 │ │     [ ]   Review                           │
│     [x]   Status [exclusive]                  │ │     [ ]   Done                             │
│     [x]     Backlog                           │ │     [ ]   (unmatched)                      │
│     [ ]     In Progress                       │ │                                            │
│     [ ]     Done [reserved]                   │ │  Personal                                  │
│                                               │ │     [ ]   Today                            │
│                                               │ │     [ ]   This Week                        │
│                                               │ │     [ ]   Someday                          │
└───────────────────────────────────────────────┘ └────────────────────────────────────────────┘
```

### Right pane active — cursor on "In Progress", live category preview in left pane

Moving to In Progress would assign `Status > In Progress` (currently Backlog) and
unassign `Status > Backlog`. The left pane shows this as `[+]`/`[-]` indicators
without committing any change.

```
Assign view/section  (Tab switches pane · Space assigns · r removes from view · j/k navigate)
┌─ Categories ──────────────────────────────────┐ ┌─ View/Section ─────────────────────────────┐
│     [x]   Work [reserved]                     │ │  Work Board                                │
│     [ ]     Engineering                       │ │     [x]   Backlog                          │
│     [x]     Design                            │ │ >   [ ]   In Progress       ← cursor       │
│     [ ]   Personal [reserved]                 │ │     [ ]   Review                           │
│     [x]   Status [exclusive]                  │ │     [ ]   Done                             │
│ [-]   [x]     Backlog                         │ │     [ ]   (unmatched)                      │
│ [+]   [ ]     In Progress                     │ │                                            │
│     [ ]     Done [reserved]                   │ │  Personal                                  │
│                                               │ │     [ ]   Today                            │
│                                               │ │     [ ]   This Week                        │
│                                               │ │     [ ]   Someday                          │
└───────────────────────────────────────────────┘ └────────────────────────────────────────────┘
```

`[+]` = would be assigned if Space pressed.  `[-]` = would be unassigned.
Indicators are shown in addition to the current `[x]`/`[ ]` state so the user
can see both where things are and where they are going.

### Left pane active — cursor hovering a category, view preview in right pane

When the cursor is on a category in the left pane, the right pane previews how
the view placement would shift if that category were toggled. Here the cursor is
on `Backlog` (currently assigned); toggling it would remove the item from the
Work Board > Backlog section.

```
Edit item assignment  (Tab switches pane · Space applies · n or / types · Enter close · Esc cancel)
┌─ Categories ──────────────────────────────────┐ ┌─ View/Section ─────────────────────────────┐
│     [x]   Work [reserved]                     │ │  Work Board                                │
│     [ ]     Engineering                       │ [-]  [x]   Backlog                           │
│     [x]     Design                            │ │     [ ]   In Progress                      │
│     [ ]   Personal [reserved]                 │ │     [ ]   Review                           │
│     [x]   Status [exclusive]                  │ │     [ ]   Done                             │
│ >   [x]     Backlog    ← cursor               │ │[+?] [ ]   (unmatched)                      │
│     [ ]     In Progress                       │ │                                            │
│     [ ]     Done [reserved]                   │ │  Personal                                  │
│                                               │ │     [ ]   Today                            │
│                                               │ │     [ ]   This Week                        │
│                                               │ │     [ ]   Someday                          │
└───────────────────────────────────────────────┘ └────────────────────────────────────────────┘
```

`[+?]` denotes a conditional gain: the item would appear in unmatched *if* the
view's base criteria are still satisfied. The `?` marks uncertainty introduced by
engine cascade (exclusive siblings, conditions) that cannot be fully resolved
without running the engine.

### Multi-select (3 items: 2 in Work>Backlog, 1 in Work>In Progress)

```
3 items selected — Tab switches pane · Space assigns all · r removes all from view
┌─ Categories ──────────────────────────────────┐ ┌─ View/Section ─────────────────────────────┐
│     [~]   Work [reserved]                     │ │  Work Board                                │
│     [ ]     Engineering                       │ │     [~]   Backlog      ← 2 of 3            │
│     [x]     Design                            │ │     [~]   In Progress  ← 1 of 3            │
│     [ ]   Personal [reserved]                 │ │ >   [ ]   Review                           │
│     [~]   Status [exclusive]                  │ │     [ ]   Done                             │
│     [~]     Backlog                           │ │     [ ]   (unmatched)                      │
│     [~]     In Progress                       │ │                                            │
│     [ ]     Done [reserved]                   │ │  Personal                                  │
│                                               │ │     [ ]   Today                            │
└───────────────────────────────────────────────┘ └────────────────────────────────────────────┘
```

`[~]` semantics are the same as the existing category picker: some but not all
selected items match. Space on a `[~]` or `[ ]` section assigns *all* selected
items to that section (using move or insert as appropriate per item).

---

## Key Bindings

| Key | Active pane | Action |
|-----|-------------|--------|
| `Tab` | Either | Switch focus between left and right pane |
| `j` / `↓` | Either | Next row (view headers skipped in right pane) |
| `k` / `↑` | Either | Previous row |
| `Space` | Left | Toggle category on selected item(s) (existing) |
| `Space` | Right | Assign selected item(s) to this section |
| `r` | Right | Remove selected item(s) from this section's view |
| `n` / `/` | Left | Enter category text input (existing) |
| `Enter` | Either | Confirm and close |
| `Esc` | Either | Cancel and close |

Footer hints update to reflect the active pane.

---

## Mutual Exclusivity

Views can be implicitly exclusive: assigning to section B of the same view may
remove the item from section A because the underlying engine unassigns conflicting
exclusive-sibling categories. This is handled identically to how the category
picker already handles `[exclusive]` siblings — the panel re-reads item state
after each committed operation and redraws. The live preview (`[+]`/`[-]`)
surfaces this *before* the user commits, making the consequence visible.

---

## Compatibility

- Multi-select: both panes read `effective_action_assignment_counts` (existing
  helper) to produce the `[x]`/`[~]`/`[ ]` tri-state. All operations apply to
  the full action set.
- Undo: view-section operations already don't participate in the undo stack
  (consistent with `[` / `]` today); this remains unchanged.
- No core data model changes: all required operations
  (`insert_item_in_section`, `remove_item_from_section`,
  `move_item_between_sections`, `remove_item_from_view`,
  `insert_item_in_unmatched`) already exist.
