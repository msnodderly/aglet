---
title: TUI Implementation Notes
updated: 2026-06-12
---

# TUI Implementation Notes

These notes preserve narrow implementation gotchas that should not live in the
always-loaded `AGENTS.md`. Treat code and tests as the source of truth, then use
this file as a routing checklist when changing the named area.

## ViewEdit

- The view editor is rooted at `crates/aglet-tui/src/modes/view_edit/`.
- Organize new code by responsibility: picker, editor, inline, overlay,
  sections, details, and state.
- Keep criteria row order stable unless render/input handling carries explicit
  source indices. `state.criteria_index` edits the draft vector by index, so
  sorting only the displayed rows can make highlight and edit target diverge.
- Board/table columns are stored on `View.sections[*].columns` inside
  `views.sections_json`. `views.columns_json` is legacy and ignored by
  `Store::row_to_view`.
- CLI `aglet view show` does not display section column definitions; board
  column changes are TUI-visible today.
- View creation via `ViewPicker -> n` opens a name input, then a new unsaved
  ViewEdit draft. Persist only from ViewEdit save; cancel paths must not create
  partial views.
- Tests that edit a view should select a named mutable view. `All Items` is an
  immutable system view and may appear first in `list_views()` or `app.views`.

## Input Panels And Inline Editors

- `Esc` means cancel across editing surfaces. Dirty complex editors use the
  `y/n/Esc` discard-confirm pattern; `Esc` never saves.
- `InputPanel` note editing needs an explicit terminal cursor position from
  `input_panel_cursor_position()`.
- The blocking "Working" popup for synchronous classification should be queued
  only after local validation passes and only when semantic/Ollama
  classification is enabled.
- Category create no longer has a parent picker. `n` creates at selected level;
  `N` creates a child when allowed. Move hierarchy later with Category Manager.
- Item assign search `Enter` resolves in this order: exact existing name,
  exactly one visible row, otherwise create new category.
- In `ItemAssignPicker`, `Space` applies the toggle immediately. If the session
  is dirty, `Enter` should close without replaying the toggle.
- `NameInput` saves directly on `Enter` from the text field.
- Inline `When` validation feedback belongs inside the popup pane, not only in
  footer status. Invalid input must keep the panel open and show the attempted
  text.
- Board inline `When` editor context should use the full item text, not
  `truncate_board_cell(...)` output.
- Add-item destination/context text belongs in the fixed help/status row, not on
  the same line as `Text>`.

## Category Manager

- In the rewritten category manager (`c` / `F9`), Details pane `j/k` navigate
  fields. When Note is focused, printable keys including `j` and `k` type into
  the note; use `Up/Down` to move away.
- `H/J/K/L` structural move/reorder keys and `<<` / `>>` level shifts are
  disabled while Details is focused.
- `<<` and `>>` arm on the first key and apply on the second matching key; any
  other key clears the prefix.
- Action/filter inline text-entry states do not use footer/input-panel cursor
  logic; render must explicitly position the terminal cursor.
- Condition/action badges should use readable labels such as `[2 conditions]`
  and `[1 action]`, not `[C2]` or `[A1]`.

## Board And Normal Mode

- Board table `column_spacing` consumes width budget. If spacing is non-zero,
  subtract spacing from layout calculations, including any synthetic
  "All Categories" column.
- Marking an item done while it blocks others reuses `Mode::ConfirmDelete` with
  `App.done_blocks_confirm`; confirm UI copy and footer hints must cover both
  delete and done-blocker flows.
- Normal-mode preview `Info` includes metadata, links, and assignment
  provenance. Clamp preview scroll against the full rendered line count, not
  only assignment rows.
- `Enter` in Normal mode edits the selected item, but opens Add Item when the
  current slot has no selected item.
- Global search `g/` temporarily switches to `All Items`, preserves the return
  view context, and uses `Esc` to return.
- `p:preview` should remain discoverable in normal footer variants.
- Edit Item inspector is popup-only; the main edit panel keeps note editing and
  inline categories.

## Link Wizard And Sections

- Link wizard target matches must render with `render_stateful_widget` and
  `ListState` so selected rows stay visible while scrolling.
- Generated-section inserts must preserve backing section criteria so
  `insert_item_in_section` can apply criteria assignments.
- If `show_children=true` but the parent currently has no children, Aglet uses
  the base `SlotContext::Section`, not `GeneratedSection`.

## Runtime And Help

- TUI auto-refresh interval is DB-backed in `app_settings` under
  `tui.auto_refresh_interval`. Load on startup, persist on `Ctrl-R`, and
  tolerate unknown/missing values by falling back to off.
- The TUI run loop should redraw only after state changes, not on every idle
  poll wakeup.
- Clap help coverage depends on per-argument doc comments in parser enums; keep
  the regression test that checks user-facing args have help text.
- Key documentation is single-sourced in `crates/aglet-tui/src/keymap.rs`.
  Update keymap tables instead of duplicating footer/help copy.
