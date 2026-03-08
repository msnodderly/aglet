# R3 Plan: TUI View + Category Workflow Streamlining (T070-T076)

## Summary
- We will treat this phase as a **UI/workflow change**, not a core data-model change.
- We will keep existing view fields (`show_unmatched`, `unmatched_label`) and implement unmatched behavior in TUI rendering/editor logic.
- We will ship: section-first board layout, full in-TUI view editing (multi include/exclude + virtual filters), laptop-friendly shortcuts, and updated regression/smoke coverage.
- We will defer “always show unmatched when empty” granularity until later; this phase uses hide-empty behavior by default.

## Worktree + Process Setup
1. Create worktree before any implementation edits:
```bash
git -C /Users/mds/src/aglet worktree add /Users/mds/src/aglet-r3 -b codex/t070-r3-tui-workflow main
```
2. Keep `br` write commands on main worktree only (`/Users/mds/src/aglet`), per project rules.
3. Create R3 issues on main (`T070`..`T076`) and dependency chain in `br`, then commit `.beads` immediately after each `br` mutation batch.
4. Create implementer prompt docs for ready issues under `/Users/mds/src/aglet/docs/process-prompt-<issue-id>-*.md` as they become ready.

## Public Interface Changes
- Normal mode shortcuts in TUI:
  - `v` open view palette.
  - `c` open category manager.
  - `,` previous view.
  - `.` next view.
  - `F8`/`F9` remain aliases.
- View palette:
  - `e` opens full view editor (replaces include-only edit flow).
- View editor:
  - `+` manage include tokens.
  - `-` manage exclude tokens.
  - `s` open section editor.
  - `u` open unmatched settings.
  - `Enter` save.
  - `Esc` cancel.
- No CLI command/flag expansion in this phase (compatibility only).

## Implementation Plan by Task

### T070 Contract Adoption
- Confirm `/Users/mds/src/aglet/spec/tui-view-category-workflow.md` as active implementation contract.
- Add a small scope note in the spec/docs: unmatched “always-show-empty” pin mode is deferred; this phase ships hide-empty default and label/show toggle behavior without schema changes.
- Update `/Users/mds/src/aglet/docs/specs/product/tasks.md` statuses as tasks are completed.

### T071 Board Layout Redesign
- Replace split-pane section selector + item pane in `/Users/mds/src/aglet/crates/agenda-tui/src/lib.rs` with board-first section columns.
- Render sections as horizontal columns with title + count + items in each column.
- Keep spatial cursor model:
  - left/right => section column.
  - up/down => item row in focused section.
- Keep inspect panel, but render it as a secondary pane that does not reintroduce section-selector UI.

### T072 Full TUI View Editor
- Add view editor modes/state in `/Users/mds/src/aglet/crates/agenda-tui/src/lib.rs`.
- Replace current include-only edit flow with draft-based editor for:
  - `criteria.include` multi-select.
  - `criteria.exclude` multi-select.
  - `criteria.virtual_include` multi-select (`WhenBucket`).
  - `criteria.virtual_exclude` multi-select (`WhenBucket`).
- Support incremental toggles without resetting prior criteria.
- Show live preview count from current draft criteria before save.
- Save path persists through existing `store.update_view` and refreshes board selection safely.

### T073 Section + Unmatched Config in Editor
- Section editor inside view editor:
  - add/remove/reorder sections.
  - edit section title.
  - edit section criteria (same include/exclude/virtual fields as view criteria).
  - edit `on_insert_assign`, `on_remove_unassign`.
  - toggle `show_children`.
- Unmatched settings in editor:
  - edit unmatched label (`unmatched_label`).
  - toggle unmatched enabled (`show_unmatched`).
- TUI render behavior change:
  - if unmatched is enabled but has zero items, do not render unmatched lane (hide-empty default).
  - if unmatched has items, render it normally.
- No schema/model change.

### T074 Laptop-Friendly Shortcuts
- Add `v`, `c`, `,`, `.` handling in normal mode in `/Users/mds/src/aglet/crates/agenda-tui/src/lib.rs`.
- Preserve `F8` and `F9` behavior as aliases.
- Ensure text-input modes still treat punctuation keys as text entry when appropriate.

### T075 Help/Footer/Status Language Update
- Update default status strings, mode titles, and footer shortcut hints in `/Users/mds/src/aglet/crates/agenda-tui/src/lib.rs`.
- Use item-first language consistently.
- Replace “edit include” wording with full “edit view” wording.
- Reflect new shortcut model (`v/c/,/.` + F-key aliases).

### T076 Regression + Smoke Coverage
- Add/extend unit tests in `/Users/mds/src/aglet/crates/agenda-tui/src/lib.rs` for:
  - view cycling key behavior and alias mapping.
  - hide-empty unmatched lane behavior.
  - view editor criteria toggles and persistence path (where testable with pure helpers/state logic).
  - section add/remove/reorder helper logic.
- Update smoke script at `/Users/mds/src/aglet/docs/test-script-tui-smoke-e2e.md` to validate new workflows and shortcuts.
- Add one R3-focused manual verification script/doc if needed for section editor + criteria authoring path.

## Acceptance Criteria
- No dedicated section selector pane remains; board is section-first.
- User can edit multi include/exclude and virtual include/exclude criteria fully in TUI.
- User can configure sections (including reorder and edit-through sets) fully in TUI.
- Unmatched lane is non-intrusive by default (hidden when empty), with editable label and enabled/disabled control.
- Laptop-friendly shortcuts work, with F-key aliases preserved.
- Help/footer/status copy matches the new workflows.
- Regression tests and smoke script cover streamlined workflow paths.

## Explicit Assumptions and Defaults
- We will not change `/Users/mds/src/aglet/crates/agenda-core/src/model.rs` or `/Users/mds/src/aglet/crates/agenda-core/src/store.rs` schema for this phase.
- “Always show unmatched lane when empty” pin-mode persistence is deferred.
- CLI remains compatibility-level for view fields in this phase; no new CLI UX surface is required.
- Existing unrelated local changes in main worktree are left untouched; all implementation edits happen in `/Users/mds/src/aglet-r3`.
