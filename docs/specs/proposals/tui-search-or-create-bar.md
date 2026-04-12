---
title: TUI Search-or-Create Bar
status: draft
created: 2026-03-15
---

# TUI Search-or-Create Bar

## Context

Inspired by NeoNV / Notational Velocity, this adds a persistent "Search or create" bar to the TUI item view. The core insight: searching and creating are the same gesture — you type what you want, and the system either finds it or makes it. This eliminates the cognitive overhead of separate `n` (new) and `/` (filter) commands.

The bar is **section-scoped**: it filters within the focused section and uses that section's category assignment rules when creating. Left/Right arrows move section focus while preserving the search text, letting you scan across sections.

## UI Layout

Current (3-region):
```
┌──────────────────────────────────────────────────────┐
│ Agenda Reborn  view:MyView  mode:Normal              │  header (1 line)
├──────────────────────────────────────────────────────┤
│ Board columns / list content                         │  main (fills)
├──────────────────────────────────────────────────────┤
│ status                                               │  footer (4 lines)
│ n:new  e:edit  /:filter  v:views  q:quit             │
└──────────────────────────────────────────────────────┘
```

Proposed (4-region, search bar inserted between header and main):
```
┌──────────────────────────────────────────────────────┐
│ Agenda Reborn  view:MyView                           │  header (1 line)
├──────────────────────────────────────────────────────┤
│ [High] Search or create...                           │  search bar (1 line)
├──────────────────────────────────────────────────────┤
│ Board columns / list content (filtered live)         │  main (fills)
├──────────────────────────────────────────────────────┤
│ 2 matches · Enter:jump/create  Esc:clear             │  footer (4 lines)
│ e:edit  v:views  q:quit                              │
└──────────────────────────────────────────────────────┘
```

Search bar states:
- **Empty + unfocused**: `[SectionName] Search or create...` (dimmed gray)
- **Focused with text**: `[SectionName] buy gro█` (cyan label, white text, cursor visible)
- **Unfocused with active filter**: `[SectionName] timeout` (cyan label, yellow text)

The search bar only appears on board/list screens — not during ViewEdit, CategoryManager, or CategoryCreate.

## UX Flow

### Activation
- `/` from Normal mode → focus search bar (loads existing filter text if any)
- `n` is preserved as explicit "new blank item" (muscle memory)

### While Search Bar Focused
| Key | Action |
|-----|--------|
| Printable chars | Insert into search buffer; **live-filter** focused section |
| Backspace/Delete | Edit search text; re-filter live |
| Enter | **Exact title match** → jump to item. **No match** → open InputPanel with title pre-filled |
| Left / Right | Move section focus; clear old section filter, apply search text to new section |
| Down / Tab | Unfocus search bar → Normal mode, filter stays active |
| Esc | Clear search text + filter, return to Normal |

### While in Normal Mode with Active Filter
| Key | Action |
|-----|--------|
| `/` | Re-focus search bar with existing text |
| Esc | Clear focused section's filter + search buffer |
| All other keys | Normal behavior (e, d, a, v, q, etc.) |

### Enter-to-Create Flow
1. User types "Buy groceries" in search bar (focused on "High" section)
2. No exact match → InputPanel opens as AddItem
3. Title field pre-filled with "Buy groceries"
4. Section's `on_insert_assign` categories pre-applied (same as `n`)
5. Search buffer + section filter cleared on InputPanel open

### Section Scanning
1. User types "meeting" in search bar → filters "High" section
2. Presses Right → "High" filter cleared, "Medium" section now filtered by "meeting"
3. Presses Right again → "Medium" cleared, "Low" filtered by "meeting"
4. Found it in "Low" → presses Enter to jump to exact match, or Down to browse

## Implementation

### 1. Data Model Changes

**File: `crates/agenda-tui/src/lib.rs`**

Add to `App` struct:
```rust
search_buffer: text_buffer::TextBuffer,  // dedicated search bar text buffer
```

Remove:
```rust
filter_target_section: usize,  // no longer needed; search always targets slot_index
```

Remove `filter_target_section` from `Default` impl (line 749). Add `search_buffer: text_buffer::TextBuffer::empty()`.

### 2. Mode Rename

**File: `crates/agenda-tui/src/lib.rs` (line 185)**

Rename `Mode::FilterInput` → `Mode::SearchBarFocused`.

Note: `CategoryColumnPickerFocus::FilterInput` is a separate enum — unchanged.

Update all references:
- `lib.rs:185` — enum definition
- `lib.rs:562` — if there's a second reference in a sub-enum
- `lib.rs:3551` — test `input_prompt_prefix` case
- `lib.rs:8789, 8862` — test assertions
- `input/mod.rs:31` — key dispatch
- `render/mod.rs:1073, 2016, 2130` — footer/prompt rendering
- `modes/board.rs:2030` — activation

### 3. Key Handling

**File: `crates/agenda-tui/src/modes/board.rs`**

**3a. Replace `handle_filter_key` (line 4173) with `handle_search_bar_key`:**

```rust
pub(crate) fn handle_search_bar_key(
    &mut self, code: KeyCode, agenda: &Agenda<'_>,
) -> Result<bool, String> {
    match code {
        KeyCode::Esc => {
            self.search_buffer.clear();
            if self.slot_index < self.section_filters.len() {
                self.section_filters[self.slot_index] = None;
            }
            self.mode = Mode::Normal;
            self.refresh(agenda.store())?;
        }
        KeyCode::Enter => {
            let query = self.search_buffer.trimmed().to_string();
            if query.is_empty() {
                self.mode = Mode::Normal;
            } else if let Some(idx) = self.find_exact_match_in_slot(&query) {
                self.item_index = idx;
                self.mode = Mode::Normal;
                self.status = format!("Jumped to '{}'", query);
            } else {
                self.open_add_item_with_title(query, agenda)?;
            }
        }
        KeyCode::Down | KeyCode::Tab => {
            self.mode = Mode::Normal;  // keep filter active
        }
        KeyCode::Left => {
            let old = self.slot_index;
            self.move_slot_cursor(-1);
            self.transfer_search_filter(old);
            self.refresh(agenda.store())?;
        }
        KeyCode::Right => {
            let old = self.slot_index;
            self.move_slot_cursor(1);
            self.transfer_search_filter(old);
            self.refresh(agenda.store())?;
        }
        _ => {
            if self.search_buffer.handle_key(code, false) {
                self.apply_search_filter();
                self.refresh(agenda.store())?;
            }
        }
    }
    Ok(false)
}
```

**3b. New helper methods:**

```rust
fn apply_search_filter(&mut self) {
    let slot = self.slot_index;
    if slot < self.section_filters.len() {
        let text = self.search_buffer.trimmed().to_string();
        self.section_filters[slot] = if text.is_empty() { None } else { Some(text) };
    }
}

fn transfer_search_filter(&mut self, old_slot: usize) {
    if old_slot < self.section_filters.len() {
        self.section_filters[old_slot] = None;
    }
    self.apply_search_filter();
}

fn find_exact_match_in_slot(&self, query: &str) -> Option<usize> {
    let needle = query.to_ascii_lowercase();
    self.current_slot()?.items.iter()
        .position(|item| item.text.to_ascii_lowercase() == needle)
}

fn open_add_item_with_title(&mut self, title: String, agenda: &Agenda<'_>) -> Result<(), String> {
    // Same section context extraction as open_input_panel_add_item
    let (section_title, on_insert_assign) = /* extract from current_slot().context */;
    let mut panel = input_panel::InputPanel::new_add_item(&section_title, &on_insert_assign);
    panel.text.set(title);  // pre-fill title (pub(crate) field)
    self.input_panel = Some(panel);
    self.mode = Mode::InputPanel;
    // Clear search state
    self.search_buffer.clear();
    if self.slot_index < self.section_filters.len() {
        self.section_filters[self.slot_index] = None;
    }
    Ok(())
}
```

**3c. Update Normal mode `/` handler (line 2027):**

```rust
KeyCode::Char('/') => {
    self.mode = Mode::SearchBarFocused;
    // Load existing filter text if search_buffer is empty
    if self.search_buffer.is_empty() {
        if let Some(existing) = self.section_filters
            .get(self.slot_index)
            .and_then(|f| f.clone())
        {
            self.search_buffer.set(existing);
        }
    }
}
```

**3d. Update Normal mode Esc handler (line 2040):**

Add `self.search_buffer.clear()` before the existing filter-clear logic.

### 4. Input Dispatch

**File: `crates/agenda-tui/src/input/mod.rs` (line 31)**

```rust
Mode::SearchBarFocused => self.handle_search_bar_key(code, agenda),
```

### 5. Rendering

**File: `crates/agenda-tui/src/render/mod.rs`**

**5a. Layout change in `draw()` (line 33):**

Add conditional 4th region:
```rust
let show_search_bar = !matches!(self.mode,
    Mode::ViewEdit | Mode::CategoryManager
) && !(self.mode == Mode::InputPanel
    && self.name_input_context == Some(NameInputContext::CategoryCreate));

if show_search_bar {
    // 4-region: header, search bar, main, footer
    constraints = [Length(1), Length(1), Min(1), Length(4)]
} else {
    // 3-region: header, main, footer (unchanged)
    constraints = [Length(1), Min(1), Length(4)]
}
```

Adjust index references for `render_main`, footer, and overlay rendering accordingly.

**5b. New `render_search_bar` method:**

Renders a single-line `Paragraph`:
- Section label in cyan: `[SectionName] `
- Text content: white when focused, yellow when unfocused+active, gray placeholder when empty
- Cursor positioned at `area.x + prefix_len + cursor_offset` when focused

**5c. Remove `Mode::FilterInput` from `input_prompt_prefix` (line 1073):**

The search bar cursor is now rendered directly by `render_search_bar`, not via the footer's `input_cursor_position`.

**5d. Footer text updates:**

- `footer_status_text`: replace `Mode::FilterInput` arm (line 2016) with `Mode::SearchBarFocused` showing section name and match count
- `footer_hint_text`: replace `Mode::FilterInput` hint (line 2130) with `"Enter:jump/create  ←/→:section  ↓/Tab:browse  Esc:clear"`
- Normal mode hint (line 2157): change `/:filter` to `/:search`

### 6. Refresh Integration

**File: `crates/agenda-tui/src/app.rs`**

In `refresh()` (line 156), when section count changes and filters reset, also clear search buffer:
```rust
if self.section_filters.len() != slots.len() {
    self.section_filters = vec![None; slots.len()];
    self.search_buffer.clear();
}
```

In `reset_section_filters()` (line 895), also clear search buffer:
```rust
pub(crate) fn reset_section_filters(&mut self) {
    self.section_filters = vec![None; self.slots.len()];
    self.search_buffer.clear();
}
```

### 7. Existing Behavior: Esc Semantics Change

**Important difference from old FilterInput:**

Old behavior: Esc in FilterInput *cancels* without clearing existing filter (preserves `section_filters[target]`).

New behavior: Esc in SearchBarFocused *always clears* the filter and search text. Rationale: the search bar is persistent and visible — if text is showing, Esc should dismiss it. There's no "cancel vs. clear" distinction needed because the user can always see the search state.

This changes test `filter_esc_in_filter_input_cancels_without_clearing` (line 8843). Update or replace this test.

## Files to Modify

| File | Changes |
|------|---------|
| `crates/agenda-tui/src/lib.rs` | Add `search_buffer` field, remove `filter_target_section`, rename `Mode::FilterInput` → `SearchBarFocused`, update tests |
| `crates/agenda-tui/src/modes/board.rs` | Replace `handle_filter_key` with `handle_search_bar_key`, add helpers, update `/` and `Esc` handlers |
| `crates/agenda-tui/src/render/mod.rs` | 4-region layout, `render_search_bar`, remove FilterInput from `input_prompt_prefix`, update footer |
| `crates/agenda-tui/src/input/mod.rs` | Rename dispatch (1 line) |
| `crates/agenda-tui/src/app.rs` | Clear `search_buffer` in `refresh()` and `reset_section_filters()`, remove `filter_target_section` usage |

## Tests

Update 3 existing filter tests to use new API (`handle_search_bar_key` instead of `handle_filter_key`, `Mode::SearchBarFocused` instead of `Mode::FilterInput`).

Add new tests:
1. **Live filtering**: type chars in SearchBarFocused → `section_filters[slot_index]` updates on each keystroke
2. **Enter exact match**: pre-populated slot, type exact title → `item_index` jumps to match
3. **Enter create**: type non-matching text → mode becomes `InputPanel`, panel title pre-filled
4. **Section scanning**: Left/Right in SearchBarFocused → old filter cleared, new filter applied, search text preserved
5. **Down unfocuses**: Down → Normal mode, filter stays active
6. **`/` resumes**: after Down, press `/` → SearchBarFocused, buffer retains text
7. **Esc clears all**: Esc → search buffer empty, section filter None, mode Normal

## Verification

1. `cargo clippy --all-targets` — no warnings
2. `cargo test -p agenda-tui` — all tests pass
3. Manual TUI test:
   - Launch TUI with a populated database
   - Press `/` → search bar activates, section label shows
   - Type text → list filters live
   - Press Left/Right → section focus moves, filter re-applies
   - Press Enter on exact match → cursor jumps to item
   - Type new text, press Enter → InputPanel opens with title pre-filled
   - Press Esc → search clears, full list restored
   - Press Down → focus moves to list, filter stays
   - Press `/` again → search bar re-focused with same text
