---
title: Category Manager Workflow Redesign
status: shipped
created: 2026-03-15
shipped: 2026-03-21
---

## Category Manager Workflow Redesign

### Goal

Clarify the difference between:

- per-category flags (Exclusive, Auto-match, Actionable)
- database-wide workflow role assignments (Ready Queue, Claim Target)

and remove the now-awkward checkbox columns from the category list.

### Problem Summary

The current Category Manager mixes two scopes in one visual language:

- `Exclusive`, `Auto-match`, and `Actionable` are properties of the selected category.
- `Ready Queue` and `Claim Target` are singleton workflow roles for the whole database.

Showing both as checkboxes in the Details pane causes two UX problems:

1. The details pane implies all rows are the same kind of toggle.
2. The left-side `Excl / Match / Todo` checkbox columns are noisy — most categories have Match and Todo on, so the columns are mostly `[x] [x]` with low signal.

### Design Decisions

1. **Replace checkbox columns with exception badges** — only badge what's notable, leave normal rows clean.
2. **Keep 2-pane layout** — Tree + Category details. Workflow setup is rare (configure-once), so it doesn't deserve permanent screen space.
3. **Workflow setup is a popup overlay** — triggered by `w`, same pattern as the `a` assign picker (`ItemAssignPicker`). Appears centered, dismisses on `Esc`.
4. **Workflow roles are visible without the popup** — badges on tree rows (`[ready-queue]`, `[claim-target]`) and a line in Category details when that category holds a role.
5. **Note stays in Category details pane** — it's a property of the selected category, not a separate pane.

### Mockup: Normal View (2 panes)

This is the default view 99% of the time.

```text
Agenda Reborn  view:Aglet  mode:CategoryManager
Categories are shared across all views. Press n to add, H/J/K/L to reorder.

Filter
  Press / to filter categories by name.

Category Manager                                    Category
┌─────────────────────────────────────────────────┐  ┌──────────────────────────┐
│ When [reserved]                                 │  │ Selected: Ready          │
│ Entry [reserved]                                │  │ Parent: Status           │
│ Done [reserved]                                 │  │ Depth: 1  Children: 0    │
│ Software Projects                               │  │ Reserved: no             │
│   Aglet                                         │  │                          │
│   NeoNV                                         │  │ Flags                    │
│ Area                                            │  │ [ ] Exclusive            │
│   CLI                                           │  │ [x] Auto-match           │
│   TUI                                           │  │ [x] Actionable           │
│   Views                                         │  │                          │
│   Core                                          │  │ Workflow Role             │
│ Issue type                                      │  │ Ready Queue              │
│ Status [exclusive]                              │  │                          │
│   Needs Refinement                              │  │ Note                     │
│   Waiting/Blocked                               │  │ Marks the manual         │
│ > Ready [ready-queue]                           │  │ category required for    │
│   In Progress [claim-target]                    │  │ claim eligibility.       │
│   Complete                                      │  │                          │
│ Priority [exclusive] [no-todo]                  │  │                          │
│   Critical                                      │  │                          │
│   High                                          │  │                          │
│   Normal                                        │  │                          │
│   Low                                           │  │                          │
│ TODO                                            │  │                          │
│ Complexity ♪                                    │  │                          │
└─────────────────────────────────────────────────┘  └──────────────────────────┘
Ready Queue = Ready  |  Claim Target = In Progress
S:save  n:new  r:rename  x:delete  Tab:pane  /:filter  w:workflow  Esc:close
```

### Mockup: Workflow Setup Popup (press w)

Same pattern as the `a` assign picker — a centered popup overlay on top of the normal view.

```text
                    ┌─ Workflow Setup ──────────────────────┐
                    │ Database-wide workflow role config     │
                    │                                       │
                    │ > Ready Queue:    Ready               │
                    │   Claim Target:   In Progress         │
                    │                                       │
                    │ Navigate tree to select a category,   │
                    │ then press Enter to assign it to the  │
                    │ focused slot. Press x to clear.       │
                    │                                       │
                    │ j/k: select slot                      │
                    │ Enter: assign selected category       │
                    │ x: clear slot                         │
                    │ Esc: close                            │
                    └───────────────────────────────────────┘
```

When the popup is open, the tree behind it is still navigable (like the assign picker). The user navigates to a category in the tree, then presses Enter in the popup to assign it to the focused workflow slot.

### Badge Rules

Replace the `Excl / Match / Todo` checkbox columns with inline exception badges on tree rows.

| Badge | Meaning | When shown |
|---|---|---|
| `[reserved]` | Reserved system category | Always (structural) |
| `[exclusive]` | Exclusive category | When exclusive is on (rare, important) |
| `[no-match]` | Auto-match is off | When auto-match is off (exception to default) |
| `[no-todo]` | Actionable is off | When actionable is off (exception to default) |
| `[ready-queue]` | Assigned as Ready Queue role | When this category is the Ready Queue |
| `[claim-target]` | Assigned as Claim Target role | When this category is the Claim Target |
| _(no badge)_ | Normal defaults | Match on, todo on, not exclusive |

Most rows end up clean. Badges only appear where something is unusual — which is exactly when you want to notice it.

### Category Details: Workflow Role Line

When the selected category holds a workflow role, the Category details pane shows it:

- If the category is the Ready Queue: `Workflow Role: Ready Queue`
- If the category is the Claim Target: `Workflow Role: Claim Target`
- If neither: section omitted

This provides context without needing the popup open.

### Interaction Model

#### Tree pane (left)

- `j`/`k` navigate categories
- `H`/`J`/`K`/`L` reorder/indent
- `n` add category
- `r` rename
- `x` delete
- `/` filter

#### Category details pane (right)

- `Tab` to reach from tree
- `j`/`k` navigate flags
- `Space`/`Enter` toggle flags (Exclusive, Auto-match, Actionable)
- Note is visible and editable within this pane

#### Workflow setup popup (press `w`)

- `j`/`k` select workflow slot (Ready Queue / Claim Target)
- `Enter` assigns the currently selected tree category to the focused slot
- `x` clears the focused slot
- `Esc` closes the popup

The tree remains navigable while the popup is open — same interaction model as `ItemAssignPicker`.

### Status Messages

On assignment:
- `Ready Queue assigned to Ready`
- `Claim Target assigned to In Progress`
- `Ready Queue assigned to Ready (replaced Next Action)`

On clear:
- `Claim Target cleared`

### Implementation Plan

#### Phase 1: Replace checkbox columns with badges

- Remove the `Excl / Match / Todo` checkbox columns from the tree.
- Render inline badges after the category label using the badge rules above.
- Include `[ready-queue]` and `[claim-target]` badges for workflow role visibility.

Files likely affected:
- `crates/agenda-tui/src/render/mod.rs`

#### Phase 2: Add workflow role line to Category details

- When the selected category holds a workflow role, show `Workflow Role: Ready Queue` or `Workflow Role: Claim Target` in the details pane.
- Remove the current `Targets` section (checkbox-style Ready Queue / Claim Target toggles) from details.

Files likely affected:
- `crates/agenda-tui/src/render/mod.rs`

#### Phase 3: Workflow setup popup

- Add `Mode::WorkflowSetup` (or similar).
- `w` key in CategoryManager opens the popup.
- Render as a centered overlay using `centered_rect`, same pattern as `render_item_assign_picker`.
- Popup shows two rows: Ready Queue and Claim Target, each displaying the currently assigned category name or "(unset)".
- `j`/`k` selects slot, `Enter` assigns from tree selection, `x` clears, `Esc` closes.

Files likely affected:
- `crates/agenda-tui/src/lib.rs` (mode enum, state)
- `crates/agenda-tui/src/modes/category.rs` (input handling)
- `crates/agenda-tui/src/render/mod.rs` (popup rendering)

#### Phase 4: Status and footer

- Show `Ready Queue = X | Claim Target = Y` in the status bar when both are assigned.
- Show `Workflow incomplete: assign Ready Queue and Claim Target to enable ready/claim` when one or both are missing.
- Footer hints include `w:workflow` in CategoryManager mode.

Files likely affected:
- `crates/agenda-tui/src/render/mod.rs`

### Test Plan

- Tree renders badges instead of checkbox columns
- `[exclusive]`, `[no-match]`, `[no-todo]` badges appear only for exceptions
- `[ready-queue]` and `[claim-target]` badges appear on the correct rows
- Category details shows "Workflow Role: Ready Queue" for the assigned category
- `w` opens the workflow popup, `Esc` closes it
- `Enter` in popup assigns the tree-selected category to the focused slot
- `x` in popup clears the focused slot
- Assigning a slot replaces the previous owner (old category loses its badge)
- Category flag toggles still work independently in the details pane
- Narrow-width render does not clip badge text
- Footer/help text includes `w:workflow`
- Status bar shows workflow role summary
