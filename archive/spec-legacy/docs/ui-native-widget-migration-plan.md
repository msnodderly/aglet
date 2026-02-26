# UI Native Widget Migration Plan (Ratatui)

## Goal

Replace ad hoc, string-rendered UI controls with native `ratatui` widgets (and minimal ecosystem crates where ratatui has no built-in equivalent), while preserving existing keyboard behavior and test coverage.

## Findings: Ad Hoc UI Surfaces

### 1) Text input and editor behavior is hand-rolled

- Single-line input editing (cursor movement, insert/delete/backspace) is custom in:
  - `crates/agenda-tui/src/input/mod.rs:45`
  - `crates/agenda-tui/src/input/mod.rs:124`
- Item note multiline editor is custom in:
  - `crates/agenda-tui/src/input/mod.rs:138`
  - `crates/agenda-tui/src/input/mod.rs:250`
- Category config note multiline editor duplicates similar logic:
  - `crates/agenda-tui/src/input/mod.rs:283`
  - `crates/agenda-tui/src/input/mod.rs:403`
- Cursor positioning is manually calculated in render:
  - `crates/agenda-tui/src/render/mod.rs:103`
  - `crates/agenda-tui/src/render/mod.rs:125`
  - `crates/agenda-tui/src/render/mod.rs:185`

Better fit:
- `tui-textarea` for both multiline and single-line inputs (`set_max_histories`, cursor movement, editing semantics handled by widget).

### 2) Buttons and checkboxes are rendered as plain text affordances

- Item edit popup buttons are string labels (`[Save]`, `[Cancel]`, `[Categories]`) plus focus enums:
  - `crates/agenda-tui/src/render/mod.rs:866`
  - `crates/agenda-tui/src/input/mod.rs:473`
  - `crates/agenda-tui/src/modes/board.rs:330`
- Category config popup toggles and buttons are also string-driven:
  - `crates/agenda-tui/src/render/mod.rs:1606`
  - `crates/agenda-tui/src/modes/category.rs:175`
  - `crates/agenda-tui/src/input/mod.rs:420`

Better fit:
- Convert action rows to a native `List`/`Tabs` selection pattern in ratatui (focus handled by `ListState`).
- Optional ecosystem crate for explicit checkbox semantics: `tui-checkbox`.

### 3) Most pickers/lists are simulated with `Paragraph` + markers

- `> ` selection markers and `[x]/[ ]` state are manually composed in:
  - `crates/agenda-tui/src/render/mod.rs:948` (view picker)
  - `crates/agenda-tui/src/render/mod.rs:989` (view create category picker)
  - `crates/agenda-tui/src/render/mod.rs:1132` (view editor category picker)
  - `crates/agenda-tui/src/render/mod.rs:1191` (view manager category picker)
  - `crates/agenda-tui/src/render/mod.rs:1252` (bucket picker)
  - `crates/agenda-tui/src/render/mod.rs:1427` (item assign picker)
  - `crates/agenda-tui/src/render/mod.rs:1493` (category manager list area)
- Scroll position for these lists is manually computed:
  - `crates/agenda-tui/src/ui_support.rs:206`

Better fit:
- `ratatui::widgets::List` with `ListState`.
- `ratatui::widgets::Scrollbar` for long lists.

### 4) Tab/pane navigation is custom strings and mode flags

- View manager sub-tabs (`[Criteria] [Columns]`) are rendered as text, with key handling for toggles:
  - `crates/agenda-tui/src/render/mod.rs:328`
  - `crates/agenda-tui/src/modes/view_edit.rs:98`

Better fit:
- `ratatui::widgets::Tabs` for pane/sub-tab affordances.

### 5) Table-like screens are manually formatted strings

- Board rows and headers are rendered by custom width math/string truncation:
  - `crates/agenda-tui/src/render/mod.rs:531`
  - `crates/agenda-tui/src/ui_support.rs:261`
  - `crates/agenda-tui/src/ui_support.rs:298`
  - `crates/agenda-tui/src/ui_support.rs:524`
- Category manager grid-like rows are similarly manual:
  - `crates/agenda-tui/src/render/mod.rs:1493`

Better fit:
- `ratatui::widgets::Table` (+ `Row`, `Cell`) with `TableState`.

## Recommended Target Widget Stack

- Keep: `ratatui` core for layout, blocks, text.
- Add:
  - `tui-textarea` for all text entry/edit fields.
  - Optional `tui-checkbox` only if explicit checkbox widgets are preferred over list-based toggles.
- Adopt ratatui built-ins where currently missing:
  - `List`, `ListState`
  - `Table`, `TableState`
  - `Tabs`
  - `Scrollbar`

## Implementation Path

### Phase 0: Foundations (no behavior change)

1. Add `crates/agenda-tui/src/widgets/` module with thin wrappers:
   - `SelectableListView`
   - `TableView`
   - `TextEditorField` (backed by `tui-textarea`)
2. Introduce state carriers in `App`:
   - list/table selection state objects
   - shared editor state type to replace duplicated note/input cursor structs
3. Keep old modes and keybindings intact while swapping rendering internals behind wrappers.

Exit criteria:
- No command key changes.
- Existing tests still pass.

### Phase 1: Migrate low-risk pickers to `List`

Migrate these renderers first:
- `render_view_picker`
- `render_view_category_picker`
- `render_view_editor_category_picker`
- `render_view_manager_category_picker`
- `render_view_editor_bucket_picker`
- `render_item_assign_picker`

Then simplify corresponding handlers in:
- `crates/agenda-tui/src/modes/view_edit.rs`
- `crates/agenda-tui/src/modes/board.rs`

Exit criteria:
- All picker screens use `List` + `ListState`.
- No manual `> ` prefix composition remains in migrated screens.

### Phase 2: Migrate text editors to `tui-textarea`

Migrate:
- Footer single-line prompts (add/filter/rename/title inputs).
- Item edit text + note fields.
- Category config note field.

Primary files:
- `crates/agenda-tui/src/input/mod.rs`
- `crates/agenda-tui/src/render/mod.rs`
- `crates/agenda-tui/src/modes/board.rs`
- `crates/agenda-tui/src/modes/category.rs`
- `crates/agenda-tui/src/modes/view_edit.rs`

Exit criteria:
- Remove duplicate byte-index/cursor editing utilities.
- Remove duplicated note cursor movement implementations.

### Phase 3: Replace pseudo buttons/toggles with standard controls

Migrate popups:
- Item edit popup action row
- Category config popup toggles and action row

Approach:
- Preferred: list-based action controls (`List` selection + Enter).
- Optional: `tui-checkbox` for category flags if richer semantics are desired.

Exit criteria:
- No string-literal button states like `[> Save <]` or manual focus cycling enums for migrated popups.

### Phase 4: Migrate table-like views to `Table`

Migrate:
- Board columns (legacy + dynamic modes)
- Category manager grid

Primary files:
- `crates/agenda-tui/src/render/mod.rs`
- `crates/agenda-tui/src/ui_support.rs`

Exit criteria:
- Replace custom width/truncation and pseudo-row marker composition with `Table` rendering logic.
- Keep note marker and done state styling behavior.

### Phase 5: Tabs and scrollbars

Migrate:
- View manager definition sub-tabs to `Tabs`.
- Add `Scrollbar` to long lists/panels (view manager panes, pickers, previews).

Exit criteria:
- No bracketed tab strings (`[Criteria]`) in rendering code.
- Scroll offsets managed via widget state where available.

### Phase 6: Cleanup and test hardening

1. Remove dead helpers from `ui_support.rs`:
   - string-based row/cell formatters
   - duplicated cursor helpers no longer needed
2. Expand tests around:
   - text editing behavior parity
   - selection movement and persistence
   - save/cancel semantics in popups and manager flows

Relevant existing tests to preserve/extend:
- `crates/agenda-tui/src/lib.rs:596`
- `crates/agenda-tui/src/lib.rs:735`
- `crates/agenda-tui/src/lib.rs:887`
- `crates/agenda-tui/src/lib.rs:1965`
- `crates/agenda-tui/src/lib.rs:2048`
- `crates/agenda-tui/src/lib.rs:2305`
- `crates/agenda-tui/src/lib.rs:2456`

## Risk Notes

- Highest regression risk: text editing parity (cursor movement, newline behavior, save/cancel).
- Medium risk: view manager workflows due many mode transitions.
- Lowest risk: picker rendering migration to `List`.

Mitigation:
- Migrate in phases with behavior-preserving tests after each phase.
- Keep keybindings stable until migration is complete; only then consider UX-level key simplification.
