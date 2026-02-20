# TUI UX Redesign — Implementation Progress

## Worktree

All changes: `/Users/mds/src/aglet-tui-redesign-phase1`
Branch: `tui-redesign-phase1`
Spec: `/Users/mds/src/aglet/spec/tui-ux-redesign.md`

---

## Phase 1: TextBuffer Extraction — ✅ COMPLETE

**All 5 steps done. 84 tests pass, zero warnings.**

### What was done

Extracted a `TextBuffer` struct that encapsulates text + cursor, replacing three
independent `String + usize` cursor pairs that had ~30 duplicated cursor methods.

| Commit | Description |
|--------|-------------|
| `012b5d9` | Created `crates/agenda-tui/src/text_buffer.rs` — TextBuffer struct + 22 unit tests |
| `2c4ac16` | Replaced `App.input + input_cursor` with `TextBuffer` |
| `49b9ffe` | Replaced `App.item_edit_note + item_edit_note_cursor` with `TextBuffer` |
| `d32becc` | Replaced `CategoryConfigEditorState.note + note_cursor` with `TextBuffer` |
| `93538ce` | Deleted all dead helpers now living inside TextBuffer (zero warnings) |

### Key outcomes

- `text_buffer.rs`: new, 22 unit tests cover single-line, multi-line, edge cases
- `input/mod.rs`: 441 lines → ~160 lines of thin wrappers
- Deleted: `single_line_textarea`, `multiline_textarea`, `char_index_from_line_col`,
  `textarea_value_and_cursor` (from `input/mod.rs`) and `note_cursor_line_col`,
  `note_line_start_chars` (from `ui_support.rs`)
- `TextBuffer::with_cursor` is `#[cfg(test)]` only (used in test fixtures)
- Test count: 62 original + 22 new = **84 total**

### TextBuffer API (as shipped)

```rust
pub(crate) struct TextBuffer { text: String, cursor: usize }

impl TextBuffer {
    pub(crate) fn new(text: String) -> Self       // cursor at end
    pub(crate) fn empty() -> Self                 // text="", cursor=0
    #[cfg(test)] pub(crate) fn with_cursor(text: String, cursor: usize) -> Self

    pub(crate) fn set(&mut self, text: String)    // cursor moves to end
    pub(crate) fn clear(&mut self)                // text="", cursor=0

    pub(crate) fn text(&self) -> &str
    pub(crate) fn cursor(&self) -> usize          // clamped to len_chars()
    pub(crate) fn len_chars(&self) -> usize
    pub(crate) fn is_empty(&self) -> bool
    pub(crate) fn trimmed(&self) -> &str
    pub(crate) fn line_col(&self) -> (usize, usize)

    pub(crate) fn handle_key(&mut self, code: KeyCode, multiline: bool) -> bool
}
```

---

## Phase 2: Unified ViewEdit Mode — 🔄 IN PROGRESS

Replaces 10 separate view-editing modes with one unified `Mode::ViewEdit`
backed by `ViewEditState`. Old modes remain until Phase 2c deletes them,
so existing tests keep passing throughout.

### Phase 2a: Build ViewEdit alongside old modes — ✅ COMPLETE

**Commit**: `163b1cf` — `tui: Phase 2a – add Mode::ViewEdit alongside old modes`

**92 tests pass, 1 warning (dead_code: open_view_editor, expected — deleted in 2c).**

#### What was done

- `lib.rs`: Added `Mode::ViewEdit`; new types `ViewEditRegion`, `ViewEditOverlay`,
  `ViewEditInlineInput`, `ViewEditState`; `view_edit_state: Option<ViewEditState>` on App.
- `modes/view_edit2.rs` (new): `open_view_edit()`, `handle_view_edit_key()` with
  3-layer dispatch (inline → overlay → region), per-region handlers for all 4 regions,
  `handle_view_edit_save()` (persists via `update_view`, refreshes, reopens editor).
- `render/mod.rs`: `render_view_edit_screen()` — 4-region vertical layout with focus
  highlight, right-aligned picker overlay panel.
- `input/mod.rs`: `Mode::ViewEdit` routed to `handle_view_edit_key`.
- `view_edit.rs`: ViewPicker `e` and `V` now call `open_view_edit()`.
- 8 new tests (region cycling, key precedence, save round-trip, Esc).

### Phase 2b: Migrate remaining entry points

1. `ViewPicker N` (new view) opens ViewEdit after creation
2. All ViewCreate*/ViewRename*/ViewDeleteConfirm return to ViewPicker unconditionally
   → delete `view_return_to_manager` bool field from `App`
3. `ItemAssignPicker` always returns to Normal
   → delete `item_assign_return_to_item_edit` bool field from `App`
4. Update affected tests for new Esc targets

Commit: `tui: migrate all entry points to ViewEdit, drop flag-based routing`

### Phase 2c: Delete old modes

Delete 10 mode variants and their associated state/render/handler code:
`ViewManagerScreen`, `ViewEditor`, `ViewSectionEditor`, `ViewSectionDetail`,
`ViewSectionTitleInput`, `ViewEditorCategoryPicker`, `ViewEditorBucketPicker`,
`ViewManagerCategoryPicker`, `ViewUnmatchedSettings`, `ViewUnmatchedLabelInput`

Also delete: `view_editor_return_to_manager`, `view_editor_category_target`,
`view_editor_bucket_target` fields; `ViewEditorState` struct.

All tests must pass.

Commit: `tui: delete 10 old view-editing modes replaced by ViewEdit`

---

## Phase 3: Per-Section Text Filters — 🔲 TODO

Replace `filter: Option<String>` with `section_filters: Vec<Option<String>>`.

- `/` targets the current section's filter slot
- Esc in Normal clears only the focused section's filter
- Filters rebuild on view switch / returning from ViewEdit after save
- Render per-section filter indicator in section headers

Commit: `tui: per-section text filters replacing global filter`

---

## Phase 4: Esc Consistency — 🔲 TODO

(Phase 2 removes flag-based routing; Phase 3 scopes FilterInput. This phase
finishes the cleanup.)

1. FilterInput: Esc cancels input and returns to Normal, preserving existing filter
2. ViewCreateCategoryPicker: Esc returns to ViewPicker unconditionally

Commit: `tui: fix remaining Esc inconsistencies`

---

## Phase 5: Footer Hint Bar — 🔲 TODO

1. Add persistent hint bar to footer layout
2. Compute hint text from current mode + sub-state
3. Separate status messages from hints

Commit: `tui: add footer hint bar with mode-aware hints`

---

## Verification (run after each phase)

```bash
cd /Users/mds/src/aglet-tui-redesign-phase1
cargo test -p agenda-tui        # all tests must pass
cargo clippy -p agenda-tui      # no new warnings
cargo build -p agenda-tui       # must compile
```
