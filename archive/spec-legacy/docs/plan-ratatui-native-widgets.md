# Plan: Replace Hand-Rolled UI Controls with Native Ratatui Widgets

## Summary

The `agenda-tui` crate (ratatui 0.29) has significant hand-rolled UI patterns that
duplicate functionality already provided by ratatui's built-in `List`, `Table`,
`Tabs`, and `Scrollbar` widgets. This plan catalogs each instance and proposes a
migration path.

## Status Update (February 19, 2026)

Implemented on branch `codex/native-widgets-textarea`:

- Completed: Phase 1 list/widget migration (`List` + `ListState`) across view picker,
  view/category pickers, view manager panes, section editor, assignment picker,
  category reparent picker, and preview provenance picker.
- Completed: Phase 2 text editing migration by adopting `tui-textarea`
  (not the original extract-internal-struct option).
- Completed: Phase 3 table migration (`Table` + `TableState`) for board columns
  (legacy and dynamic) and category manager rows.
- Completed: Phase 4 scrollbar overlays for list/table/pop-up note editors and preview.
- Completed: Phase 5 tab migration (`Tabs`) for view manager Criteria/Columns sub-tabs.
- Added regression coverage for widgetized controls:
  provenance picker selection navigation, category reparent preselection behavior,
  and board layout width invariants.

Remaining work:

- Manual QA pass in live TUI for ergonomics (scroll feel, highlight visibility,
  narrow terminal behavior).
- Optional follow-up cleanup to remove `list_scroll_for_selected_line()` if we
  decide to rely entirely on widget-native viewport management.

---

## 1. Hand-Rolled Selectable Lists → `List` + `ListState`

**Current state:** Every picker/list in the app is rendered by building a
`Vec<Line>` manually, prepending `"> "` / `"  "` markers, applying
`selected_row_style()` conditionally, computing scroll offset via
`list_scroll_for_selected_line()`, and passing the result to `Paragraph::scroll()`.
The selection index is tracked as a raw `usize` field on `App`.

**Occurrences (8+):**

| Render function | Location | Selection field |
|---|---|---|
| `render_view_picker` | render/mod.rs ~948 | `picker_index` |
| `render_item_assign_picker` | render/mod.rs ~1427 | `item_assign_category_index` |
| `render_view_category_picker` | render/mod.rs ~989 | `view_category_index` |
| `render_view_editor_category_picker` | render/mod.rs ~1132 | `view_editor.category_index` |
| `render_view_manager_category_picker` | render/mod.rs ~1191 | `view_category_index` |
| `render_view_editor_bucket_picker` | render/mod.rs ~1252 | `view_editor.bucket_index` |
| `render_view_section_editor` | render/mod.rs ~1299 | `view_editor.section_index` |
| `render_category_manager` | render/mod.rs ~1493 | `category_index` |
| `render_view_manager_screen` (views list) | render/mod.rs ~278 | `picker_index` |
| `render_view_manager_screen` (definition rows) | render/mod.rs ~376 | `view_manager_definition_index` |
| `render_view_manager_screen` (sections list) | render/mod.rs ~482 | `view_manager_section_index` |
| `render_view_manager_screen` (columns list) | render/mod.rs ~429 | `view_manager_column_index` |
| `render_view_editor` (action list) | render/mod.rs ~1094 | `view_editor.action_index` |
| `render_preview_provenance_panel` | render/mod.rs ~656 | `inspect_assignment_index` |
| `render_category_reparent` (embedded in category_manager) | render/mod.rs ~1573 | `category_reparent_index` |

**Proposed replacement:** Use `ratatui::widgets::List` with `ListItem` entries and
`ListState` for selection tracking. This eliminates:
- Manual `"> "` marker injection
- Manual `selected_row_style()` application (use `List::highlight_style()` +
  `List::highlight_symbol()`)
- Manual scroll computation (`list_scroll_for_selected_line`) — `ListState`
  handles viewport scrolling automatically via `frame.render_stateful_widget()`
- The entire `list_scroll_for_selected_line()` helper function

**Migration steps:**
1. Add `ListState` fields to `App` (or co-locate with existing index fields).
   Each existing `*_index` field maps to `ListState::with_selected(Some(index))`.
2. Convert each render function: build `Vec<ListItem>` instead of `Vec<Line>`,
   construct `List::new(items).highlight_symbol("> ").highlight_style(selected_row_style())`,
   and call `frame.render_stateful_widget(list, area, &mut state)`.
3. Update cursor-movement helpers (`move_*_cursor`) to mutate `ListState`
   instead of raw `usize`. Or keep the index and sync into `ListState` before
   each render.
4. Delete `list_scroll_for_selected_line()` once all callers are migrated.

**Risk:** Low. `List` is the most mature ratatui widget. The only subtlety is
that some lists have a header row (e.g., category manager table header) that
shouldn't be selectable — those can be rendered as a separate `Paragraph` above
the `List`, or use `Table` instead (see §2).

---

## 2. Hand-Rolled Tabular Data → `Table` + `TableState`

**Current state:** The category manager and the board columns render tabular data
by manually computing column widths (`board_column_widths`, `fit_board_cell`),
padding strings, and concatenating with `BOARD_COLUMN_SEPARATOR`.

**Occurrences (3):**

| Render function | Location |
|---|---|
| `render_category_manager` | render/mod.rs ~1493 (Name / Excl / Match / Todo columns) |
| `render_board_columns` (legacy path) | render/mod.rs ~594 (When / Item / Categories columns) |
| `render_board_columns` (dynamic path) | render/mod.rs ~571 (`board_dynamic_header`/`board_dynamic_row`) |

**Proposed replacement:** Use `ratatui::widgets::Table` with `Row` and `Cell`.
- Define `Constraint`-based column widths (replaces `board_column_widths` / `fit_board_cell`)
- Use `Table::highlight_style()` + `Table::highlight_symbol()` for selection
- Use `TableState` for scroll/selection tracking
- The header row maps to `Table::header()`

**Migration steps:**
1. Convert `render_category_manager` first (simplest case, fixed columns).
2. Convert the legacy board path.
3. Convert the dynamic board path (column definitions come from `View::columns`).
4. Delete `board_column_widths`, `fit_board_cell`, `board_item_row`,
   `board_annotation_header`, `board_dynamic_header`, `board_dynamic_row`, and
   `BOARD_COLUMN_SEPARATOR`.

**Risk:** Medium. The dynamic board path has variable columns whose widths come
from model data. `Table` supports this via `Constraint::Length(col.width)`.
The board uses `Paragraph::scroll()` for vertical scrolling; `TableState`
replaces this.

---

## 3. Hand-Rolled Tab Bar → `Tabs`

**Current state:** The view manager definition sub-tab UI (Criteria vs Columns)
is rendered by manually formatting `[Criteria]` / ` Criteria ` strings with
bracket toggling.

**Location:** `render_view_manager_screen` ~328-339

**Proposed replacement:** Use `ratatui::widgets::Tabs::new(["Criteria", "Columns"])`
with `.select(tab_index)` and `.highlight_style()`.

**Migration steps:**
1. Replace the manual `criteria_tab_label`/`columns_tab_label` formatting with
   `Tabs::new(...)` and render it into the first row of the definition pane.
2. Map `view_manager_definition_sub_tab` enum to a numeric index for `Tabs::select()`.

**Risk:** Very low.

---

## 4. Missing Scrollbar → `Scrollbar` + `ScrollbarState`

**Current state:** All scrollable panels (board columns, preview panels, pickers,
note editors) scroll via `Paragraph::scroll()` with no visual scroll indicator.
Users have no idea how much content is off-screen.

**Proposed enhancement:** Add `ratatui::widgets::Scrollbar` alongside any
scrollable content.

**Priority locations:**
- Board columns (items can scroll off-screen)
- Preview provenance / summary panels
- Note editor areas (item edit popup, category config popup)
- All picker popups (category pickers, view picker, etc.)

**Migration steps:**
1. For each scrollable area, compute `ScrollbarState::new(total).position(current)`.
2. Render `Scrollbar::new(ScrollbarOrientation::VerticalRight)` in the same area.
3. This is additive — no existing code needs to change, just overlay the scrollbar.

**Risk:** Very low. Purely additive.

---

## 5. Hand-Rolled Text Input → Extract Reusable Widget (or adopt `tui-textarea`)

**Current state:** There are **three independent text-editing implementations:**

| Text editor | Fields | Cursor tracking | Lines |
|---|---|---|---|
| Single-line input | `App::input`, `App::input_cursor` | char-index arithmetic | ~100 lines in input/mod.rs |
| Item edit note (multi-line) | `App::item_edit_note`, `App::item_edit_note_cursor` | `note_cursor_line_col`, `note_line_start_chars` | ~120 lines in input/mod.rs |
| Category config note (multi-line) | `CategoryConfigEditorState::note`, `note_cursor` | Same helpers, duplicated logic | ~110 lines in input/mod.rs |

Each has its own `move_*_cursor_left/right/up/down/home/end`,
`backspace_*_char`, `delete_*_char`, `insert_*_char`, `insert_*_newline`
methods — nearly identical code repeated three times.

**Option A — Internal refactor:** Extract a `TextInput` struct with `value: String`,
`cursor: usize`, and methods for cursor movement/editing. Instantiate it for each
use. No new dependency.

**Option B — Adopt `tui-textarea`:** The `tui-textarea` crate provides a full
multi-line text editor widget with cursor rendering, scrolling, and clipboard
support. It would replace all three implementations and the manual cursor
positioning logic in `input_cursor_position` / `item_edit_cursor_position` /
`category_config_cursor_position`.

**Recommendation (updated):** Option B (`tui-textarea`) is now implemented.
This reduced duplicated note-editing logic while preserving existing keyboard
affordances in the app flow.

**Migration steps (Option A):**
1. Create a `TextInput` struct with `value: String`, `cursor: usize`, and all
   editing methods (extracted from the existing single-line implementation).
2. Create a `TextArea` struct extending `TextInput` with multi-line support
   (up/down movement, newline insertion).
3. Replace `App::input` / `App::input_cursor` with `TextInput`.
4. Replace `App::item_edit_note` / `App::item_edit_note_cursor` with `TextArea`.
5. Replace `CategoryConfigEditorState::note` / `note_cursor` with `TextArea`.
6. Delete all the duplicated `move_*`, `backspace_*`, `insert_*` methods.
7. Delete `note_cursor_line_col`, `note_line_start_chars`, `string_byte_index`
   helpers (move into `TextArea` struct).

**Risk:** Medium. Touch points are broad, but the refactor is mechanical. Tests
already exist for the editing behavior.

---

## 6. Hand-Rolled Buttons → Styled Spans (keep as-is)

**Current state:** Buttons like `[Save]` / `[> Save <]` and `[Categories]` /
`[> Categories <]` are rendered as formatted strings.

**Assessment:** Ratatui has no built-in button widget. The current approach is
standard practice in TUI apps. **No change recommended.**

---

## 7. Hand-Rolled Checkboxes → Styled Spans (keep as-is)

**Current state:** Checkboxes are rendered as `[x]`/`[ ]` strings in the category
config editor and pickers.

**Assessment:** Ratatui has no built-in checkbox widget. The `[x]`/`[ ]` convention
is idiomatic for TUI apps. **No change recommended.**

---

## 8. `centered_rect` → Keep (no ratatui equivalent)

**Current state:** `centered_rect()` creates a centered `Rect` using nested
`Layout` splits.

**Assessment:** This is a standard ratatui recipe. No built-in centering widget
exists. **No change recommended.**

---

## Implementation Order

| Phase | Scope | Effort | Impact |
|---|---|---|---|
| **Phase 1** | Lists → `List`/`ListState` | Completed | High — duplicated pattern removed |
| **Phase 2** | Text editing migration (`tui-textarea`) | Completed | High — duplicated editor logic reduced |
| **Phase 3** | Category manager + board → `Table`/`TableState` | Completed | Medium — native table layout/selection |
| **Phase 4** | Add `Scrollbar` overlays to scrollable areas | Completed | Medium — improved off-screen awareness |
| **Phase 5** | Tab bar → `Tabs` widget | Completed | Low — clearer native tab affordance |

**Actual effort:** Completed in this branch over multiple implementation/test passes.

## Lines of Code Impact

| Category | Lines removed (est.) | Lines added (est.) |
|---|---|---|
| Manual list rendering | ~400 | ~200 (List widget setup) |
| Scroll computation helpers | ~30 | 0 |
| Duplicated text input code | ~330 | ~120 (shared struct) |
| Board column formatting | ~100 | ~70 (Table setup) |
| **Net reduction** | **~860** | **~390 → ~470 lines saved** |
