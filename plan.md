# Plan: Global `Ctrl-S` Save Shortcut in the Aglet TUI

## Problem

Saving in the TUI is currently inconsistent and hard to discover:

- `n -> type description -> Enter` saves a simple item.
- If focus moves to `When`, `Enter` parses/normalizes the date instead of saving.
- If focus moves to multiline `Note`, `Enter` correctly inserts a newline and `S` is typed into the note.
- The user must know to leave text-like fields and press capital `S` from a non-text focus such as Categories.
- Footer/help text often advertises `Enter:save` or `S:save` in ways that are only true for some focused fields.

The Note behavior is intentional: **`Enter` in a multiline note must continue inserting a newline**.

## Goal

Add a consistent global save shortcut:

> `Ctrl-S` saves/commits the current savable editor everywhere it is safe to save.

This should work from text fields, date fields, multiline notes, pickers, and explicit save/cancel rows without inserting `s` into any buffer.

## Current code findings

### Input event flow

- `crates/aglet-tui/src/input/mod.rs`
  - `App::handle_key_event(KeyEvent, ...)` records modifiers in `self.transient.key_modifiers`, then calls existing `handle_key(KeyCode, ...)` for most modes.
  - This means many mode handlers only receive `KeyCode`; modifier-aware checks must either:
    - use `self.transient.key_modifiers`, or
    - be refactored to accept `KeyEvent`.

### Unified input panel

- `crates/aglet-tui/src/input_panel.rs`
  - `InputPanel::handle_key_event(...)` already receives a full `KeyEvent`.
  - `handle_focus_navigation(...)` currently receives only `KeyCode`.
  - Capital `S` saves only when focus is not `Text`, `Note`, or `When`, and not on an assigned numeric category row.
  - `Enter` saves from `Text` focus.
  - `Enter` in `When` is handled as text input / date recalculation by the caller.
  - `Enter` in `Note` is routed to `TextBuffer` with multiline enabled.

### Save dispatch for input panel

- `crates/aglet-tui/src/modes/board.rs`
  - `handle_input_panel_key(...)` dispatches `InputPanelAction::Save` to:
    - `save_input_panel_add`
    - `save_input_panel_edit`
    - `save_input_panel_name`
    - `save_input_panel_category_create`
  - Add/Edit save paths already do validation and parse `When` as needed, including direct-save fallback comments for capital `S`.

### Other savable TUI contexts found by search

- `crates/aglet-tui/src/modes/view_edit/editor.rs`
  - `KeyCode::Char('S')` saves ViewEdit via `handle_view_edit_save`.
- `crates/aglet-tui/src/modes/category.rs`
  - `category_manager_save_key_pressed` currently matches `KeyCode::Char('S')`.
  - Condition/action editors mostly save with `Enter`.
  - Numeric inline edits save with `Enter`.
- `crates/aglet-tui/src/modes/board.rs`
  - Board column/category picker paths use `s/S`, `Enter`, and explicit save rows depending on mode.
  - Inline `WhenDate` and `NumericValue` panels are routed through InputPanel save dispatch.
- `crates/aglet-tui/src/modes/global_settings.rs`
  - Text settings save with `Enter`.
- `crates/aglet-tui/src/modes/view_edit/inline.rs`
  - Inline alias/name editing saves with `Enter`.
- `crates/aglet-tui/src/render/mod.rs`
  - Many footer/help hints mention `S:save` or `Enter:save`; these must be audited after behavior changes.

## UX rules

1. `Ctrl-S` means save/commit in every savable editor.
2. `Ctrl-S` must be handled before text buffers, so it never inserts `s`.
3. Preserve local `Enter` behavior:
   - Description/Text in Add/Edit: `Enter` may continue quick-saving.
   - Multiline Note: `Enter` inserts a newline and does not save.
   - When in Add/Edit: `Enter` parses/normalizes the date and does not save.
   - Single-field panels: `Enter` may continue saving.
   - Pickers/editors that currently use `Enter` to select/save can keep doing so.
4. Keep capital `S` where it is already safe and useful, but do not make users tab out of text fields to save.
5. Footer/help text should advertise `Ctrl-S:save` in all savable contexts, with field-specific `Enter` behavior shown where relevant.

## Implementation plan

### 1. Add a shared helper for `Ctrl-S`

Create a small helper, likely in `crates/aglet-tui/src/input/mod.rs` or `crates/aglet-tui/src/ui_support.rs`:

```rust
pub(crate) fn is_ctrl_s_event(key: KeyEvent) -> bool {
    matches!(key.code, KeyCode::Char('s') | KeyCode::Char('S'))
        && key.modifiers.contains(KeyModifiers::CONTROL)
}
```

Also add an `App` helper for handlers that only receive `KeyCode`:

```rust
pub(crate) fn is_ctrl_s_code(&self, code: KeyCode) -> bool {
    matches!(code, KeyCode::Char('s') | KeyCode::Char('S'))
        && self.transient.key_modifiers.contains(KeyModifiers::CONTROL)
}
```

This avoids a broad refactor from `KeyCode` to `KeyEvent` in the first pass.

### 2. Implement `Ctrl-S` in `InputPanel`

In `InputPanel::handle_key_event(...)`, check `Ctrl-S` before `handle_focus_navigation` and before routing to `TextBuffer`:

```rust
if is_ctrl_s_event(key) {
    return InputPanelAction::Save;
}
```

Important details:

- This should apply from `Text`, `When`, `Note`, `Categories`, `Actions`, `Suggestions`, and `TypePicker`.
- Unlike capital `S`, it should save from an assigned numeric category row too, because `Ctrl-S` is an explicit command rather than typed numeric input.
- Existing save validation remains in `save_input_panel_*`.

### 3. Add `Ctrl-S` to major mode save handlers

Update mode handlers that currently save on capital `S` or `Enter` only:

- `ViewEdit`: in `handle_view_edit_key`, treat `self.is_ctrl_s_code(code)` like capital `S` and call `handle_view_edit_save`.
- `CategoryManager`: update `category_manager_save_key_pressed` or its call sites to recognize `Ctrl-S` using modifiers. Since the function currently only receives `KeyCode`, either:
  - make it an `App` method using `self.transient.key_modifiers`, or
  - pass `KeyModifiers` to it.
- `CategoryDirectEdit`: add `Ctrl-S` wherever capital `S` currently saves the draft.
- Board category/column picker modes: where `s/S` saves a picker or `Enter` saves an explicit row, add `Ctrl-S` as equivalent save.
- Inline `WhenDate`, `NumericValue`, `NameInput`, and `CategoryCreate` are probably covered by InputPanel once step 2 lands.
- Global settings and ViewEdit inline alias/name edits should get `Ctrl-S` if they use text buffers and save on `Enter`.

Suggested first-pass priority:

1. InputPanel Add/Edit/Name/WhenDate/Numeric/CategoryCreate.
2. ViewEdit.
3. CategoryManager and CategoryDirectEdit.
4. Board column/category pickers.
5. GlobalSettings and small inline editors.

### 4. Update footer/help copy

Audit `crates/aglet-tui/src/render/mod.rs` and status strings in mode files.

For Add/Edit input panel:

- Text focus: `Type title  Enter/Ctrl-S:save  Tab:when  Esc:cancel`
- When focus: `Type date  Enter:parse  Ctrl-S:save  Tab:note  Esc:cancel`
- Note focus: `Type note  Enter:newline  Ctrl-S:save  Tab:categories  Esc:cancel`
- Categories/Actions/Suggestions: include `S/Ctrl-S:save` if capital `S` remains valid.

For single-field panels:

- `Type value  Enter/Ctrl-S:save  Esc:cancel`
- `Type name  Enter/Ctrl-S:save  Esc:cancel`
- `Enter natural language or ISO datetime  Enter/Ctrl-S:save  Esc:cancel`

For ViewEdit and CategoryManager:

- Replace or augment `S:save` with `S/Ctrl-S:save`.

Avoid saying `Enter:save` on Add/Edit note or when focus if `Enter` does not save there.

### 5. Tests

Add unit and integration-style tests near existing tests.

#### InputPanel unit tests (`crates/aglet-tui/src/input_panel.rs`)

- `ctrl_s_saves_from_text_focus`
- `ctrl_s_saves_from_when_focus`
- `ctrl_s_saves_from_note_focus`
- `ctrl_s_saves_from_categories_numeric_row`
- `ctrl_s_does_not_mutate_text_buffer`
- Preserve existing behavior:
  - `enter_in_note_focus_inserts_newline_or_is_handled_not_save`
  - `enter_in_when_focus_does_not_save`
  - capital `S` still does not save from text/note/when.

#### App/TUI tests (`crates/aglet-tui/src/tests.rs`)

- Add item: type title, tab to Note, type multiline note, press `Ctrl-S`; item is saved with note intact.
- Add item: type title, tab to When, type `tomorrow`, press `Ctrl-S`; item is saved and parsed when is persisted.
- Edit item: focus Note, modify note, press `Ctrl-S`; item is saved.
- ViewEdit: make draft dirty, press `Ctrl-S`; view persists.
- CategoryDirectEdit: edit search/input field if applicable, press `Ctrl-S`; draft applies without needing to tab out.
- CategoryManager details note: pressing literal capital `S` while editing note still inserts `S`/does not save; pressing `Ctrl-S` saves.
- Rendering tests:
  - Add/Edit note help includes `Ctrl-S:save` and `Enter:newline`.
  - Add/Edit when help includes `Ctrl-S:save` and `Enter:parse`.
  - Existing tests that assert note focus hides `S:save` should be updated to assert it hides capital `S:save` but shows `Ctrl-S:save`.

### 6. Manual smoke tests

Use a temp DB:

```bash
cargo run --bin aglet -- --db /tmp/aglet-ctrl-s-smoke.ag
```

Scenarios:

1. Simple add:
   - `n`, type `simple ctrl s`, press `Ctrl-S`.
   - Expected: item added.
2. Add with When:
   - `n`, type title, `Tab`, type `tomorrow`, press `Ctrl-S`.
   - Expected: item added with parsed date.
3. Add with Note:
   - `n`, type title, `Tab`, `Tab`, type line one, `Enter`, type line two, press `Ctrl-S`.
   - Expected: item added with two-line note; no literal `s` inserted.
4. Edit note:
   - Select item, `e`, tab to note, edit note, press `Ctrl-S`.
   - Expected: edit saved.
5. ViewEdit:
   - Open view editor, make a small change, press `Ctrl-S`.
   - Expected: view saved and editor closes/returns per existing `S` behavior.
6. Category manager detail note:
   - Edit details note, press literal `S` and confirm it types `S`.
   - Press `Ctrl-S` and confirm it saves.

Verify with CLI as needed:

```bash
cargo run --bin aglet -- --db /tmp/aglet-ctrl-s-smoke.ag view show "All Items"
```

## Risks and edge cases

- Terminals can intercept `Ctrl-S` for XON/XOFF flow control. Many modern terminal setups pass it through, but some may freeze output. If this is a concern, document `stty -ixon` or consider an alternate chord such as `Ctrl-Enter`/`Alt-S` as a fallback.
- Some crossterm backends may report `Ctrl-S` as `Char('s')` with `CONTROL`, while others may use uppercase depending on Shift/Caps; match both `s` and `S`.
- Existing code often receives only `KeyCode`; relying on `self.transient.key_modifiers` is the least invasive approach, but future cleanup could pass `KeyEvent` through all mode handlers.
- Be careful not to turn `Ctrl-S` into save in non-savable modes such as Normal search, Help, or destructive confirmation prompts.
- Validation and blocking overlays should remain exactly where they are: `Ctrl-S` should call existing save paths, not bypass preflight validation.

## Acceptance criteria

- `Ctrl-S` saves from Add/Edit Description, When, Note, Categories, Actions, and Suggestions.
- `Enter` in multiline Note still inserts a newline and does not save.
- `Enter` in Add/Edit When still parses/normalizes and does not save.
- `Ctrl-S` does not insert text into any field.
- Footer/help copy accurately exposes `Ctrl-S:save` and does not misrepresent `Enter` behavior.
- Existing capital `S` save behavior remains available where it is safe.
- Tests cover the above behavior and pass with `cargo test -p aglet-tui` or the relevant focused test command.
