# Auto-Suggest for CategoryDirectEdit

## Overview

Add an auto-suggest feature to `CategoryDirectEdit` that shows matching categories as the user types. This provides a faster way to browse and select categories without typing the full name.

It should be similar to "ido" style fuzzy matching narrowing in Emacs.

Mode::CategoryDirectEdit (editing a column cell directly via Enter on a column). This mode does have column context - it knows the parent category (column.heading) and valid children (child_ids).

CategoryDirectEdit should get auto-suggest, filtered to only child categories of the column's parent

These are two different features:
| Mode | Context | Suggested Categories |
|------|---------|---------------------|
| ItemAssignInput | None (general assignment) | All categories |
| CategoryDirectEdit | Column-specific | Only children of column.heading |

we are discussing the column specific feature


---

## New Behavior

| User Action | Result |
|-------------|--------|
| Type character | show popup if matches exist |
| `Tab` | Autocomplete selected suggestion name into input |
| `Enter` | If popup open: assign selected category. Else: create/assign by typed name |
| `Esc` | Close popup (stay in input mode). If popup closed: return to Normal mode |
| `Up`/`Down` | Navigate suggestions when popup open |
| `Ctrl-n`/`Ctrl-p` | Navigate suggestions when popup open (vim-style) |
| Backspace to empty filter | Hide popup |

---

## Implementation Details

### 1. Data Model Changes (`lib.rs`)

Add suggestion popup state to `App` struct:

```rust
struct CategorySuggestState {
    /// Cursor index within filtered suggestions.
    suggest_index: usize,
}
```

Add field to `App`:
```rust
category_suggest: Option<CategorySuggestState>,
```

**Why `Option`?** The popup can be open or closed. `None` = closed, `Some(state)` = open.

---

### 2. Matching Logic (`ui_support.rs`)

New function:

```rust
pub(super) fn filter_child_categories(
    child_ids: &[CategoryId],
    categories: &[Category],
    query: &str,
) -> Vec<CategoryId> {
    let query_lower = query.to_ascii_lowercase();
    child_ids
        .iter()
        .filter(|id| {
            categories
                .iter()
                .find(|c| c.id == **id)
                .map(|c| {
                    // Exclude "When" from suggestions (date/time parsing category)
                    !c.name.eq_ignore_ascii_case("When")
                        && c.name.to_ascii_lowercase().contains(&query_lower)
                })
                .unwrap_or(false)
        })
        .cloned()
        .collect()
}
```

**Note**: "When" is excluded because it's a special date/time parsing category. "Done" and "Entry" CAN appear in suggestions.

**Matching behavior**: Case-insensitive substring matching (e.g., "hig" matches "High"). Fuzzy matching can be added later as an enhancement.

---

### 3. Key Handling Updates (`modes/board.rs`)

Modify `handle_category_direct_edit_key`:

```rust
pub(crate) fn handle_category_direct_edit_key(
    &mut self,
    code: KeyCode,
    agenda: &Agenda<'_>,
) -> Result<bool, String> {
    // If popup is open, handle navigation first
    if let Some(ref mut suggest) = self.category_suggest {
        match code {
            KeyCode::Up | KeyCode::Down => {
                let delta = if code == KeyCode::Up { -1 } else { 1 };
                self.move_suggest_cursor(delta);
                return Ok(false);
            }
            KeyCode::Tab => {
                self.autocomplete_from_suggestion();
                return Ok(false);
            }
            KeyCode::Enter => {
                self.assign_selected_suggestion(agenda)?;
                return Ok(false);
            }
            KeyCode::Esc => {
                self.category_suggest = None;  // Close popup only
                return Ok(false);
            }
            _ => {}  // Fall through to text input
        }
    }

    // Handle text input
    match code {
        KeyCode::Esc => {
            self.mode = Mode::Normal;
            self.status = "Cancelled".to_string();
            self.clear_input();
            self.category_suggest = None;
        }
        KeyCode::Enter => {
            let text = self.input.text().to_string();
            self.commit_category_direct_edit(&text, agenda)?;
            self.category_suggest = None;
        }
        _ => {
            self.input.handle_key(code, false);
            self.update_suggestions();  // Filter and show/hide popup
        }
    }
    Ok(false)
}
```

**New helper methods**:

```rust
fn update_suggestions(&mut self) {
    let text = self.input.text();
    if text.is_empty() {
        self.category_suggest = None;
        return;
    }

    let child_ids = self.get_current_column_child_ids();
    let matches = filter_child_categories(&child_ids, &self.categories, text);

    if matches.is_empty() {
        self.category_suggest = None;
    } else {
        self.category_suggest = Some(CategorySuggestState {
            suggest_index: 0,
        });
    }
}

fn move_suggest_cursor(&mut self, delta: i32) {
    if let Some(ref mut suggest) = self.category_suggest {
        let matches = self.get_current_matches();
        let len = matches.len();
        if len == 0 { return; }
        let new_idx = (suggest.suggest_index as i64 + delta as i64)
            .rem_euclid(len as i64) as usize;
        suggest.suggest_index = new_idx;
    }
}

fn autocomplete_from_suggestion(&mut self) {
    let matches = self.get_current_matches();
    if let Some(suggest) = &self.category_suggest {
        if let Some(&id) = matches.get(suggest.suggest_index) {
            if let Some(cat) = self.categories.iter().find(|c| c.id == id) {
                self.input.set(cat.name.clone());
                self.category_suggest = None;
            }
        }
    }
}

fn assign_selected_suggestion(&mut self, agenda: &Agenda<'_>) -> Result<(), String> {
    let matches = self.get_current_matches();
    if let Some(suggest) = &self.category_suggest {
        if let Some(&id) = matches.get(suggest.suggest_index) {
            let item_id = self.selected_item().map(|i| i.id).unwrap();
            agenda.assign_item_manual(item_id, id, Some("manual:tui.direct_edit".into()))
                .map_err(|e| e.to_string())?;
            self.mode = Mode::Normal;
            self.category_suggest = None;
            self.refresh(agenda.store())?;
        }
    }
    Ok(())
}
```

---

### 4. Rendering Implementation (`render/mod.rs`)

Add popup rendering:

```rust
fn render_category_suggest_popup(
    &self,
    frame: &mut ratatui::Frame<'_>,
    matches: &[CategoryId],
    selected_idx: usize,
) {
    // Get cell position from board layout
    let Some((cell_rect, below_space, above_space)) = self.get_edited_cell_position() else {
        return;
    };

    // Determine if popup goes below or above
    let max_items = 8;
    let needed_height = (matches.len().min(max_items) + 2) as u16;  // +2 for borders

    let popup_rect = if below_space >= needed_height {
        // Below cell
        Rect {
            x: cell_rect.x,
            y: cell_rect.y + 1,
            width: cell_rect.width.max(20),
            height: needed_height,
        }
    } else {
        // Above cell
        Rect {
            x: cell_rect.x,
            y: cell_rect.y.saturating_sub(needed_height),
            width: cell_rect.width.max(20),
            height: needed_height,
        }
    };

    frame.render_widget(Clear, popup_rect);

    let items: Vec<ListItem> = matches
        .iter()
        .enumerate()
        .map(|(i, &id)| {
            let cat = self.categories.iter().find(|c| c.id == id);
            let name = cat.map(|c| c.name.as_str()).unwrap_or("?");
            let style = if i == selected_idx {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(name).style(style)
        })
        .collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Suggestions"));

    frame.render_widget(list, popup_rect);
}
```

**Call from main render loop**:
```rust
// In render() after rendering the board
if self.mode == Mode::CategoryDirectEdit {
    if let Some(ref suggest) = self.category_suggest {
        let matches = self.get_current_matches();
        if !matches.is_empty() {
            self.render_category_suggest_popup(frame, &matches, suggest.suggest_index);
        }
    }
}
```

---

### 5. File-by-File Changes

| File | Changes |
|------|---------|
| `lib.rs` | Add `CategorySuggestState` struct, add `category_suggest: Option<CategorySuggestState>` to `App` |
| `modes/board.rs` | Update `handle_category_direct_edit_key`, add helper methods (`update_suggestions`, `move_suggest_cursor`, `autocomplete_from_suggestion`, `assign_selected_suggestion`) |
| `ui_support.rs` | Add `filter_child_categories` function |
| `render/mod.rs` | Add `render_category_suggest_popup`, call it from main render when popup is open |

---

## Testing

Manual testing scenarios:

1. **Basic filtering**: Type "hig" → see "High" in suggestions
2. **Case-insensitive**: Type "HIG" → see "High" in suggestions
3. **No matches**: Type "xyz" → popup hidden, Enter prompts create
4. **Autocomplete**: Tab copies selected name to input, closes popup
5. **Assign from suggestion**: Enter assigns selected category directly
6. **Create new**: Type non-matching name, Enter creates new category via `CategoryCreateConfirm`
7. **Empty filter**: Backspace to empty → popup hides
8. **Navigation**: Up/Down arrows move selection
9. **Wrap navigation**: Down at last item wraps to first
10. **Esc closes popup**: Esc closes popup but stays in edit mode
11. **Esc twice exits**: First Esc closes popup, second Esc exits to Normal mode
12. **"When" excluded**: "When" category should not appear in suggestions
13. **"Done" allowed**: "Done" CAN appear in suggestions if it's a child of the column's parent
14. **Multiple matches**: If column has High/Medium/Low, "h" shows only "High"
15. **Popup positioning**: Popup appears below cell if space, above if not
16. **Max 8 items**: If more than 8 matches, popup shows 8 with scrolling

---

## Future Enhancements (Out of Scope)

- Fuzzy matching (e.g., "hg" matches "High")
- Ctrl-n/Ctrl-p navigation (in addition to Up/Down)
- Persist last-used suggestion selection per column
- Show category counts or indicators in suggestions
- Keyboard shortcut to force-open suggestions (e.g., Ctrl-Space)

---

## Implementation Todo List

### Phase 1: Data Model & State

- [x] **1.1** Add `CategorySuggestState` struct to `lib.rs`
  - Define struct with `suggest_index: usize` field
  - Derive `Clone, Debug`

- [x] **1.2** Add `category_suggest: Option<CategorySuggestState>` field to `App` struct

- [x] **1.3** Initialize `category_suggest: None` in `App::default()`

### Phase 2: Filtering Logic

- [x] **2.1** Add `filter_child_categories` function to `ui_support.rs`
  - Parameters: `child_ids: &[CategoryId]`, `categories: &[Category]`, `query: &str`
  - Returns: `Vec<CategoryId>` of matching category IDs
  - Logic: case-insensitive substring match, exclude "When"

- [x] **2.2** Add unit tests for `filter_child_categories`
  - Test: basic substring match
  - Test: case-insensitive matching
  - Test: empty query returns empty
  - Test: "When" is excluded
  - Test: "Done" is included if in child_ids

### Phase 3: Key Handling Logic

- [x] **3.1** Add `get_current_column_child_ids` helper method to `App`
  - Returns `Vec<CategoryId>` for the currently edited column
  - Reuses logic from `open_category_direct_edit` and `commit_category_direct_edit`

- [x] **3.2** Add `get_current_matches` helper method to `App`
  - Combines `get_current_column_child_ids` with `filter_child_categories`
  - Returns current filtered suggestions based on input text

- [x] **3.3** Add `update_suggestions` method to `App`
  - Called after every text change
  - Sets `category_suggest` to `Some` with matches, or `None` if empty/no matches

- [x] **3.4** Add `move_suggest_cursor` method to `App`
  - Takes `delta: i32` for movement direction
  - Wraps around using `rem_euclid`

- [x] **3.5** Add `autocomplete_from_suggestion` method to `App`
  - Copies selected suggestion name to input buffer
  - Closes popup (sets `category_suggest` to `None`)

- [x] **3.6** Add `assign_selected_suggestion` method to `App`
  - Takes `agenda: &Agenda<'_>` parameter
  - Assigns selected category to current item
  - Returns `Result<(), String>`
  - Sets mode to `Normal`, clears popup, calls `refresh`

- [x] **3.7** Update `handle_category_direct_edit_key` in `modes/board.rs`
  - Check if popup is open first, handle navigation keys
  - Handle `Up`/`Down` for cursor movement when popup open
  - Handle `Tab` for autocomplete
  - Handle `Enter` for assign when popup open
  - Handle `Esc` to close popup (without exiting mode)
  - Call `update_suggestions` after text input changes

- [x] **3.8** Clear `category_suggest` when entering `CategoryDirectEdit` mode
  - Update `open_category_direct_edit` to set `self.category_suggest = None`

- [x] **3.9** Clear `category_suggest` when exiting `CategoryDirectEdit` mode
  - Ensure all exit paths (Esc, Enter, etc.) clear the state

### Phase 4: Rendering

- [x] **4.1** Add `get_edited_cell_position` helper method to `App`
  - Calculates the screen `Rect` of the currently edited cell
  - Returns `Option<(cell_rect: Rect, below_space: u16, above_space: u16)>`
  - Uses board layout info and current selection state

- [x] **4.2** Add `render_category_suggest_popup` method to `render/mod.rs`
  - Parameters: `frame`, `matches: &[CategoryId]`, `selected_idx: usize`
  - Calculate popup position (below cell if space, else above)
  - Render `Clear` widget first
  - Render `List` widget with category names
  - Highlight selected item with yellow/bold style
  - Max 8 items visible (calculate needed height)

- [x] **4.3** Add popup rendering call in main `render` method
  - Check `self.mode == Mode::CategoryDirectEdit`
  - Check `self.category_suggest.is_some()`
  - Get matches and render popup

### Phase 5: Integration & Polish

- [x] **5.1** Verify popup clears correctly on mode transitions
  - Opening other modes should clear `category_suggest`
  - Ensure no stale state between operations

- [x] **5.2** Handle edge case: column with no children
  - Should not show popup
  - Existing create flow should still work

- [x] **5.3** Handle edge case: all children filtered out
  - Popup should hide when no matches
  - Enter should proceed with create flow

- [x] **5.4** Handle edge case: single match
  - Popup shows with one item
  - Up/Down should still work (wrapping to same item)

- [x] **5.5** Update status message when popup is open
  - Consider showing hint: "Enter to select, Tab to autocomplete, Esc to close"

- [x] **5.6** Handle edge case: category name exists elsewhere
  - Check if category name already exists under different parent
  - Show helpful error message with parent location
  - Don't prompt to create duplicate

- [x] **5.7** Handle edge case: reserved category names
  - Block creation of "When", "Entry", "Done" categories

- [x] **5.8** Accept Enter in create confirmation
  - Enter key acts as 'yes' in CategoryCreateConfirm prompt

### Phase 6: Testing & Verification

- [x] **6.1** Manual test: Basic filtering ("hig" → "High")

- [x] **6.2** Manual test: Case-insensitive ("HIG" → "High")

- [x] **6.3** Manual test: No matches ("xyz" → no popup, Enter creates)

- [x] **6.4** Manual test: Autocomplete (Tab copies name to input)

- [x] **6.5** Manual test: Assign from suggestion (Enter assigns)

- [x] **6.6** Manual test: Create new category (non-matching name, Enter → confirm)

- [x] **6.7** Manual test: Empty filter (Backspace to empty → popup hides)

- [x] **6.8** Manual test: Navigation (Up/Down moves selection)

- [x] **6.9** Manual test: Wrap navigation (Down at last wraps to first)

- [x] **6.10** Manual test: Esc closes popup (stay in edit mode)

- [x] **6.11** Manual test: Esc twice exits (first closes popup, second exits to Normal)

- [x] **6.12** Manual test: "When" excluded from suggestions

- [x] **6.13** Manual test: "Done" appears if child of column parent

- [x] **6.14** Manual test: Multiple matches filter correctly

- [x] **6.15** Manual test: Popup positioning (below/above cell)

- [x] **6.16** Manual test: Max 8 items with scrolling

- [x] **6.17** Run `cargo test` to verify no regressions

- [x] **6.18** Run `cargo clippy` and fix any warnings
