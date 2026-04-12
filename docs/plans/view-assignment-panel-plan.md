---
title: View/Section Assignment Pane (Implementation)
status: shipped
created: 2026-03-22
shipped: 2026-03-22
---

# Implementation Plan: View/Section Assignment Pane

See `view-assignment-panel.md` for the full proposal with mockups.

---

## Overview

The work splits into four phases:

| Phase | Scope | Risk |
|-------|-------|------|
| A | Core: pure preview computation | Low |
| B | TUI state & data model | Low |
| C | Key handling | Medium |
| D | Rendering | Medium |

Each phase is independently testable and can be merged on its own.

---

## Phase A — Core: preview computation

### A1. Expose `section_insert_targets` / `section_remove_targets` as public helpers

**File:** `crates/agenda-core/src/agenda.rs`

These two private functions already encode exactly the category arithmetic for
section transitions. Make them `pub(crate)` (or fully `pub` if useful for tests)
so the TUI layer can call them without duplicating the logic.

```rust
// before
fn section_insert_targets(view: &View, section: &Section) -> HashSet<CategoryId>
fn section_remove_targets(view: &View, section: &Section) -> HashSet<CategoryId>

// after
pub fn section_insert_targets(view: &View, section: &Section) -> HashSet<CategoryId>
pub fn section_remove_targets(view: &View, section: &Section) -> HashSet<CategoryId>
```

### A2. Add `preview_section_move` on `Agenda`

**File:** `crates/agenda-core/src/agenda.rs`

Pure read-only computation. Returns the net set of categories that would be
assigned and unassigned if the item moved from `from_section` to `to_section`
inside `view`. Mirrors the logic in `move_item_between_sections` without any
mutation.

```rust
pub struct SectionMovePreview {
    pub to_assign: HashSet<CategoryId>,
    pub to_unassign: HashSet<CategoryId>,
}

pub fn preview_section_move(
    item_id: ItemId,
    view: &View,
    from_section: Option<&Section>,   // None  → item not currently in any section
    to_section: Option<&Section>,     // None  → target is "unmatched"
) -> SectionMovePreview
```

Cases:
- `(Some(from), Some(to))` — mirrors `move_item_between_sections` arithmetic
- `(None, Some(to))` — mirrors `insert_item_in_section` / `insert_item_in_unmatched`
- `(Some(from), None)` — mirrors `remove_item_from_section`
- `(None, None)` — empty preview

This function does not touch the store. `item_id` is accepted for a potential
future extension where item's *current* assignments are used to refine the
preview, but the initial implementation only needs the section/view arguments.

### A3. Unit tests for `preview_section_move`

Cover the four cases above plus edge cases (e.g. overlapping assign/unassign
sets cancel out).

---

## Phase B — TUI state and data model

### B1. New types

**File:** `crates/agenda-tui/src/lib.rs`

```rust
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ItemAssignPane {
    Categories,
    ViewSection,
}

/// A flattened row in the View/Section pane.
#[derive(Clone)]
pub(crate) enum ViewAssignRow {
    /// Non-navigable view heading.
    ViewHeader {
        view_idx: usize,
        name: String,
    },
    /// Navigable section row (section_idx = None means "unmatched").
    SectionRow {
        view_idx: usize,
        section_idx: Option<usize>,
        label: String,
    },
}

/// Ephemeral preview computed whenever the cursor moves.
#[derive(Clone, Default)]
pub(crate) struct AssignmentPreview {
    /// Categories the action would add (shown as [+] in category pane).
    pub cat_to_add: HashSet<CategoryId>,
    /// Categories the action would remove (shown as [-] in category pane).
    pub cat_to_remove: HashSet<CategoryId>,
    /// View/section rows that would gain the item (shown as [+] in view pane).
    pub section_to_gain: HashSet<(usize, Option<usize>)>,
    /// View/section rows that would lose the item (shown as [-] in view pane).
    pub section_to_lose: HashSet<(usize, Option<usize>)>,
}
```

### B2. New fields on `App`

**File:** `crates/agenda-tui/src/lib.rs` (App struct)

```rust
item_assign_pane: ItemAssignPane,           // default: Categories
item_assign_view_row_index: usize,          // cursor in view/section list
view_assign_rows: Vec<ViewAssignRow>,       // rebuilt on open / refresh
item_assign_preview: AssignmentPreview,     // rebuilt on cursor move
```

### B3. `build_view_assign_rows()`

**File:** `crates/agenda-tui/src/ui_support.rs`

Iterates `app.views`; for each view emits one `ViewHeader` then one `SectionRow`
per section plus one `SectionRow { section_idx: None }` if `view.show_unmatched`.
Returns `Vec<ViewAssignRow>`.

### B4. `compute_assignment_preview()`

**File:** `crates/agenda-tui/src/app.rs`

Called whenever the cursor moves in either pane. Updates `self.item_assign_preview`.

**Right pane cursor moved to a `SectionRow`:**
1. Determine the item's current section in the target view (by calling
   `resolve_view` on the action items and finding where they sit).
2. Call `Agenda::preview_section_move(item_id, view, from_section, to_section)`.
3. For multi-select: union of all individual previews (an item that is already
   in the target section contributes nothing to the preview).
4. Store result in `self.item_assign_preview`.

**Left pane cursor moved to a `CategoryRow`:**
1. Simulate toggling the category: build a hypothetical `assignments` map by
   either adding or removing the hovered category from the item's current
   assignments.
2. Run `resolve_view` across all views with the hypothetical assignments.
3. Diff the current view placement against the hypothetical placement to populate
   `section_to_gain` and `section_to_lose`.
4. `cat_to_add` / `cat_to_remove` are empty in this direction (the category pane
   is the source of truth, not the preview target).

**When cursor moves to a `ViewHeader` or the pane loses focus:**
Clear `self.item_assign_preview`.

### B5. `item_in_section_counts()`

**File:** `crates/agenda-tui/src/app.rs`

Mirrors `effective_action_assignment_counts` but for view/section placement.
Runs `resolve_view` for one view and returns `(placed_count, total_count)` for
each section. Used by the right-pane renderer to produce `[x]`/`[~]`/`[ ]`.

---

## Phase C — Key handling

### C1. Open panel initialisation

**File:** `crates/agenda-tui/src/modes/board.rs`

When `a` is pressed:
- Set `self.item_assign_pane = ItemAssignPane::Categories` (existing default).
- Rebuild `self.view_assign_rows` via `build_view_assign_rows()`.
- Clear `self.item_assign_preview`.
- Rest of init is unchanged.

### C2. `Tab` key in `handle_item_assign_category_key`

Switch `self.item_assign_pane`:
- `Categories → ViewSection`: initialise `item_assign_view_row_index` to the
  first `SectionRow` that has `[x]` for the focused item (or 0 if none).
  Recompute preview.
- `ViewSection → Categories`: clear preview, restore category pane focus.

### C3. New `handle_item_assign_view_key()`

**File:** `crates/agenda-tui/src/modes/board.rs`

Handles input when `item_assign_pane == ViewSection`:

| Key | Behaviour |
|-----|-----------|
| `j` / `↓` | Advance `item_assign_view_row_index`, skip `ViewHeader` rows. Recompute preview. |
| `k` / `↑` | Reverse. Skip headers. Recompute preview. |
| `Space` | On a `SectionRow`: assign action items to the section. Uses `move_item_between_sections` if item already in a section of the same view, else `insert_item_in_section` or `insert_item_in_unmatched`. Sets `item_assign_dirty = true`. Refreshes and recomputes preview. |
| `r` | On a `SectionRow`: call `remove_item_from_view` for the view that owns the row. Sets `item_assign_dirty = true`. Refreshes. |
| `Tab` | Switch to Categories pane (via C2 logic). |
| `Enter` | Close (same as existing `Enter` handler). |
| `Esc` | Cancel (same as existing `Esc` handler). |

Multi-select: `Space` and `r` iterate `self.action_item_ids()` (existing helper),
calling the operation per item.

### C4. Route key events

In `handle_item_assign_category_key` (the existing dispatcher), add a branch at
the top:

```rust
if self.item_assign_pane == ItemAssignPane::ViewSection {
    return self.handle_item_assign_view_key(key, agenda);
}
```

`Tab` is handled before this branch so it works from either pane.

---

## Phase D — Rendering

### D1. Widen the popup

**File:** `crates/agenda-tui/src/render/mod.rs`

Change the dispatch site:

```rust
// before
self.render_item_assign_picker(frame, centered_rect(72, 72, frame.area()));
// after
self.render_item_assign_picker(frame, centered_rect(88, 72, frame.area()));
```

### D2. Two-pane layout in `render_item_assign_picker`

Split `chunks[1]` (the current list area) horizontally:

```rust
let panes = Layout::default()
    .direction(Direction::Horizontal)
    .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
    .split(chunks[1]);
// panes[0] → category list (existing, refactored)
// panes[1] → view/section list (new)
```

### D3. Category pane border style

Active (focus on Categories):
```rust
Block::default().title("Categories").borders(Borders::ALL)
    .border_style(Style::default().fg(Color::Cyan))
```
Inactive:
```rust
Block::default().title("Categories").borders(Borders::ALL)
    .border_style(Style::default().fg(Color::DarkGray))
```

### D4. Category row rendering with preview indicators

Add a 4-character prefix column before the existing `[x]`/`[ ]`/`[~]` checkbox:

```
    [x]   Backlog          ← no preview
[-] [x]   Backlog          ← would be removed
[+] [ ]   In Progress      ← would be assigned
```

The preview prefix is drawn in a distinct style (yellow for `[+]`, red for `[-]`,
dim for nothing). The existing checkbox column and category name are unchanged.

### D5. View/section pane rendering (`render_view_assign_list()`)

New private method. Row types:

**`ViewHeader`**:
- Not in the selectable list — rendered as a plain label with no checkbox or
  cursor marker.
- Style: bold or underlined to distinguish from section rows.

**`SectionRow`** — compute three components:
1. Preview prefix: `[+]` / `[-]` / `   ` based on `item_assign_preview`.
2. Placement checkbox: `[x]` / `[~]` / `[ ]` from `item_in_section_counts()`.
3. Indented label: `"    " + section.label` (4-space indent).

Cursor marker (`> `) matches the existing list highlight convention.

Border style follows the same active/inactive logic as D3.

### D6. Header line

Replace the single static header with a dynamic one based on active pane:

```rust
let header = match self.item_assign_pane {
    ItemAssignPane::Categories =>
        "Edit item assignment  (Tab switches pane · Space applies · n or / types · Enter close · Esc cancel)",
    ItemAssignPane::ViewSection =>
        "Assign view/section  (Tab switches pane · Space assigns · r removes from view · j/k navigate)",
};
```

### D7. Footer hints

**File:** `crates/agenda-tui/src/render/mod.rs` (footer hint table)

Add/update hints for `Mode::ItemAssignPicker` to reflect the active pane. Two
static arrays, selected by `self.item_assign_pane`.

---

## Testing

### Unit tests (agenda-core)

- `preview_section_move` for all four input combinations.
- Verify no store mutation occurs.

### Unit tests (agenda-tui)

- `build_view_assign_rows` emits correct row types and count for a view with
  sections and `show_unmatched = true`.
- `compute_assignment_preview` (right pane): hovering a section correctly
  populates `cat_to_add` and `cat_to_remove`.
- `compute_assignment_preview` (left pane): hovering a category correctly
  populates `section_to_gain` / `section_to_lose`.
- `handle_item_assign_view_key` Space: item moves to target section; state
  reflects the change.
- `handle_item_assign_view_key` `r`: item removed from view.
- Multi-select Space: all action items moved.
- `Tab` key switches `item_assign_pane` and clears preview.

### Regression

- Existing `handle_item_assign_category_key` tests should pass without
  modification (the new pane-routing branch only fires when pane is
  `ViewSection`, which existing tests never reach).

---

## File Change Summary

| File | Change |
|------|--------|
| `crates/agenda-core/src/agenda.rs` | Expose `section_insert_targets`, `section_remove_targets`; add `SectionMovePreview` and `preview_section_move` |
| `crates/agenda-tui/src/lib.rs` | Add `ItemAssignPane`, `ViewAssignRow`, `AssignmentPreview` types; new fields on `App` |
| `crates/agenda-tui/src/ui_support.rs` | Add `build_view_assign_rows()` |
| `crates/agenda-tui/src/app.rs` | Add `compute_assignment_preview()`, `item_in_section_counts()` |
| `crates/agenda-tui/src/modes/board.rs` | Add `handle_item_assign_view_key()`; update `handle_item_assign_category_key()` (Tab branch, init); update open-panel code |
| `crates/agenda-tui/src/render/mod.rs` | Widen popup; two-pane layout; `render_view_assign_list()`; preview indicators in category rows; dynamic header; footer hints |

No changes to `agenda-core` data model, store, or query engine. No changes to
any other TUI mode.
