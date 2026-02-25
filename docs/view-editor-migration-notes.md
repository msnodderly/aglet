# View Editor Migration Notes (Create Flow + Split Picker/Editor)

This note documents the shipped workflow changes for the TUI view manager/editor.

## What Changed

- `v` remains a lightweight **View Picker** (quick switch / quick actions).
- Deep editing happens in a separate full-screen **View Editor** (open with `e` from picker).
- Creating a view no longer opens a create-time criteria/category picker.
- Creating a view now:
  1. prompts for the view name
  2. creates the view directly
  3. auto-creates the first section
  4. opens the editor with first-section title inline edit active

## Behavior Differences From Older Flow

- No hidden implicit include criterion is added during create.
- Criteria are configured in the editor details pane after create.
- Section add now supports relative insertion:
  - `n` = add below current
  - `N` = add above current
- Section delete (`x`) now asks for confirmation.
- `Esc` in the editor prompts before discarding dirty changes.

## Compatibility Notes

- Quick picker remains simple (`j/k`, `Enter`, `N`, `r`, `x`, `e`, `Esc`).
- Existing views/data model remain compatible (core still permits zero sections).
- The TUI now defaults to creating a first section for usability.
