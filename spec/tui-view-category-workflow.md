# TUI View + Category Workflow Spec (Detailed)

Date: 2026-02-17  
Status: Active implementation contract  
Scope: TUI interaction model for daily board use, category management, and view authoring.

## 1. Goal

Define a clear, low-friction workflow for:

- navigating work as sections on a board
- creating and managing categories
- creating, editing, switching, renaming, and deleting views
- keeping advanced configuration in-app (no CLI detour required for common work)

## 2. Scope and Non-Goals

In scope:

- TUI layout and interaction behavior
- keyboard mapping and mode semantics
- view criteria/sections/unmatched editing in TUI
- phrasing and status-copy expectations

Out of scope for this phase:

- core schema changes in `/Users/mds/src/aglet/crates/agenda-core/src/model.rs`
- new store schema behavior in `/Users/mds/src/aglet/crates/agenda-core/src/store.rs`
- CLI UX expansion for new editor workflows
- persisted "always show unmatched when empty" pin-mode semantics

## 3. UX Principles

- Item-first: the board and item actions are primary.
- Mode clarity: each mode has explicit, short, action-oriented hints.
- Destructive consistency: `x` is delete across TUI surfaces.
- Laptop-first entry points: `v`, `c`, `,`, `.` are first-class; `F8`/`F9` remain aliases.
- Local context: add/edit operations act on current section/view focus.

## 4. Main Board Behavior

### 4.1 Layout

- Board renders sections as stacked lanes top-to-bottom.
- Each lane shows:
  - lane title
  - item count
  - item rows with annotation columns (`When | Item | All Categories`)
- Inspect panel is optional secondary pane and does not act as a section selector.

### 4.2 Annotation Contract (V1)

- For this phase, board annotation columns are fixed as:
  - `When`
  - `Item`
  - `All Categories`
- This applies to all views, including the default `All Items` view.
- `All Categories` displays every assigned category for the item:
  - values sorted by category display name
  - rendered as a comma-separated list
  - empty when the item has no category assignments
- Header and item rows use one shared width layout so `When`, `Item`, and
  `All Categories` stay visually aligned as a grid.
- When content exceeds available column width, truncate in-cell rather than
  shifting separator positions.
- Row density is compact (single-line rows, no wrap-induced blank spacer lines).
- This fixed annotation contract is intentionally model-free in this phase (UI behavior only).

### 4.3 Cursor Model

- `left/right` (`h/l`): move focused lane.
- `up/down` (`j/k`): move focused item in current lane.
- Selection is spatial and preserved across refresh where possible.

### 4.4 Normal Mode Keys

- `n`: add item in current lane context.
- `e`: edit selected item text.
- `m`: edit selected item note.
- `a`: assign selected item to category.
- `u`: unassign through inspect panel picker.
- `[` / `]`: move selected item between lanes.
- `r`: remove selected item from current view.
- `d`: mark selected item done.
- `x`: delete selected item (confirm).
- `v` / `F8`: open view palette.
- `c` / `F9`: open category manager.
- `,` / `.`: previous/next view.
- `i`: toggle inspect panel.
- `/`: filter input.
- `q`: quit.

## 5. Category Workflow

### 5.1 Category Manager Keys

- `n`: create subcategory under selected category.
- `N`: create top-level category (parent = root).
- `r`: rename selected category.
- `p`: reparent selected category.
- `t`: toggle exclusive.
- `i`: toggle implicit-string matching.
- `x`: delete selected category (confirm).
- `Esc` / `F9`: close manager.

### 5.2 Required Copy Semantics

- Creating with `n` must communicate parent explicitly:
  - `Create subcategory under '<parent>'`
- Creating with `N` must communicate root explicitly:
  - `Create top-level category (root parent)`
- Avoid ambiguous phrases like `create parent <name>`.

## 6. View Workflow

### 6.1 View Palette Keys

- `Enter`: switch active view.
- `N`: create view.
- `r`: rename selected view.
- `x`: delete selected view (confirm).
- `e`: open full view editor for selected view.
- `Esc`: close palette.

### 6.2 View Creation Flow

Step 1: name input

- Enter `View create` mode from palette with `N`.
- `Enter` proceeds to include-category picker.
- `Esc` cancels.

Step 2: include-category picker

- `j/k`: move category cursor.
- `Space`: toggle category include selection.
- `Enter`: create view from selected includes.
  - If no toggles were selected, fallback is current highlighted category.
- `Esc`: cancel and return to palette.

### 6.3 View Rename Flow

- Open with `r` on selected view.
- `Enter` saves new name.
- `Esc` cancels.

### 6.4 View Delete Flow

- Open with `x` on selected view.
- Confirm mode behavior:
  - `y`: delete selected view.
  - `n` or `Esc`: cancel.
- On successful delete:
  - remain in view palette for cleanup/continued management
  - keep selection on nearest valid row
  - if active view was deleted, active index shifts to nearest surviving view
- `d` is not used for view deletion (reserved for item done in normal mode).

### 6.5 View Editor Keys

Entry: `v` -> select view -> `e`.

- `+`: manage include category set.
- `-`: manage exclude category set.
- `]`: manage virtual include buckets.
- `[`: manage virtual exclude buckets.
- `s`: section editor.
- `u`: unmatched settings.
- `Enter`: save draft to store.
- `Esc`: cancel draft.

Editor picker behavior:

- Category picker: `j/k` + `Space` toggle + `Enter/Esc` back.
- Bucket picker: `j/k` + `Space` toggle + `Enter/Esc` back.
- Preview count updates from draft criteria before save.

### 6.6 Section Editor and Detail

Section list mode:

- `N`: add section.
- `x`: remove section.
- `[` / `]`: reorder section.
- `Enter` or `e`: open section detail.
- `Esc`: return to view editor.

Section detail mode:

- `t`: edit title.
- `+` / `-`: section include/exclude categories.
- `]` / `[`: section virtual include/exclude buckets.
- `a`: edit `on_insert_assign`.
- `r`: edit `on_remove_unassign`.
- `h`: toggle `show_children`.
- `Esc`: back to section list.

### 6.7 Unmatched Settings

- `t`: toggle `show_unmatched`.
- `l`: edit `unmatched_label`.
- `Esc`: back to view editor.

Render policy for this phase:

- If unmatched lane has zero items, do not render lane.
- If unmatched lane has items, render lane.
- No schema/model changes for "always show when empty" pin persistence.

### 6.8 Update-Through-View Interpretation (V1)

The TUI should favor the minimal logically consistent mutation for actions taken in a view:

- Insert item (`n`) in a lane:
  - creates item
  - applies assignment semantics implied by the lane context (`on_insert_assign` or generated lane rules)
  - if view/section criteria constrain assignment, apply the smallest assignment set that satisfies visible context
- Remove from view (`r`) in a lane:
  - unassign categories implied by `on_remove_unassign` / remove-from-view rules for the current view context
  - does not delete the item from the database
- Delete item (`x`) is explicit destructive removal from the database and must always require confirmation.
- Move between lanes (`[` / `]`) is interpreted as:
  - remove semantics from source lane, then insert semantics into destination lane
  - with exclusivity rules enforced by category constraints.

## 7. Mode-Specific `e` Semantics

- In normal mode, `e` edits selected item text.
- In view palette, `e` edits selected view definition.
- Mode/footer copy must make this distinction explicit.

## 8. Data Contract Constraints

- Keep existing view fields and persistence paths:
  - `show_unmatched`
  - `unmatched_label`
  - `criteria.*`
  - `sections.*`
- Persist via existing `store.update_view` / `store.create_view` / `store.delete_view`.
- No new core schema fields in this phase.

## 9. Deferred Next Slice: Column Headings Inside Sections

The next design slice is specified in:

- `/Users/mds/src/aglet-experiments/spec/tui-view-column-workflow-design.md`

Summary of target direction:

- column headings should usually map to category-family roots (for example `Priority`, `People`, `Department`)
- column values should come from assigned categories in the selected heading subtree
- column sets may eventually vary by section after experimentation and persistence-gate review

## 10. Acceptance Criteria

- No dedicated section-selector pane remains.
- Board rows use `When | Item | All Categories` annotation columns.
- `All Items` view uses the same annotation contract by default.
- View palette supports switch/create/rename/delete/edit.
- View creation supports multi-category include toggles.
- View editor supports include/exclude + virtual include/exclude edits without reset.
- Section add/remove/reorder/detail editing is fully in TUI.
- Unmatched lane is hidden when empty and configurable for label + enabled toggle.
- `v`/`c`/`,`/`.` shortcuts work, with `F8`/`F9` aliases preserved.
- Delete key consistency: `x` is delete action in view/category/item contexts.
- Help/footer/status copy reflects mode-specific behavior.
