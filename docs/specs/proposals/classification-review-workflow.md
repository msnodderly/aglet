---
title: Classification Review Workflow
status: draft
created: 2026-03-15
---

# Classification Review Workflow Design

## Problem

The current `Mode::ClassificationReview` is a standalone modal with a two-pane
browser layout. It works but doesn't feel right:

- Requires full mode switch (`Shift+C`) away from the board
- Two-pane layout (items list + suggestions list) forces context-switching
- Binary accept/reject per suggestion, not toggle-based
- No natural "advance to next item" flow
- No `?` indicator on individual items in the board view

The original Lotus Agenda review workflow was optimized for rapid sequential triage:
see item → scan suggestions → toggle off wrong ones → confirm → next item. We want
that feel.

## Design

### Part 1: `?` indicator in the board gutter

Add a `?` glyph to the existing indicator column (alongside `✓` done, `&` blocked,
`♪` note) for items with pending classification suggestions.

```text
┌─ Backlog (5) ──────────────────────────────────────────────────┐
      Item                          When         Categories
>  ✓  Review Q4 budget              2026-03-15   Finance
   ?♪ Reimburse Sam for hotel       2026-03-20   Expense
      Fix login timeout             -            Bugs
   ?  Call Mom about birthday       -            -
      Grocery run                   -            -
```

The `?` appears in the same column as `✓&♪`, in position order: `✓` `&` `?` `♪`.

**Implementation:**
- Add `has_pending_suggestions: bool` parameter to `item_indicator_glyphs()`
- Look up `pending_suggestion_count_for_item(item.id) > 0` at each render call site
- Add `const SUGGESTION_MARKER_SYMBOL: &str = "?";`
- Insert between blocked and note in the glyph string

**Footer indicator:**
When pending suggestions exist globally, show in the footer status line:

```text
? 3 pending suggestions
```

This replaces the current verbose `"3 classification suggestions pending"` text.

### Part 2: Review overlay panel (bulk triage via `g?`)

When the user presses `g?` (g-prefix → `?`) in Normal mode, a review overlay
appears and enters **bulk triage mode**: the system finds the first item in the
current view with pending suggestions and presents it. After each confirmation
(Enter), it automatically advances to the next `?` item. The user works through
the entire queue sequentially — they don't pick items individually. This mirrors
Lotus Agenda's Utilities > Questions flow. The overlay is a modal panel (like
InputPanel) centered over the board, not a full mode switch.

```text
┌─ Review Suggestions ─────────────────────────────────────────┐
│                                                               │
│ Reimburse Sam for hotel                                       │
│ Note: paid on card ending 1142                                │
│ Current: Expense, Work                                        │
│                                                               │
│ ┌─ Suggested categories ───────────────────────────────────┐  │
│ │                                                          │  │
│ │  [x] Travel          matched "hotel" in text             │  │
│ │ >[x] Reimbursement   LLM: reimbursement language         │  │
│ │  [x] Budget          assigned to Travel AND Expense      │  │
│ │                                                          │  │
│ └──────────────────────────────────────────────────────────┘  │
│                                                               │
│ Space:toggle  Enter:confirm  A:accept-all  Esc:cancel         │
│ (2 more items have pending suggestions)                       │
└───────────────────────────────────────────────────────────────┘
```

**Key UX properties (matching Lotus Agenda):**

1. **Item-sequential:** The panel shows ONE item at a time. After confirming, it
   automatically advances to the next item with pending suggestions (if any).

2. **Toggle-based:** Each suggestion starts as `[x]` (proposed/on). The user
   deselects the ones they don't want. This is the opposite of the current
   accept/reject model — the default is acceptance, and the user opts out of
   wrong suggestions.

3. **Non-destructive default:** If the user just presses Enter without changing
   anything, all suggestions are accepted. This makes the common case (system
   is right) fast.

4. **Batch confirmation:** Enter confirms the entire set — toggled-on suggestions
   are accepted, toggled-off suggestions are rejected. One keypress resolves all
   suggestions for the item.

5. **Count indicator:** Footer shows how many more items have pending suggestions,
   so the user knows how much triage remains.

**Keybindings:**

| Key | Action |
|-----|--------|
| `j` / `k` / `↑` / `↓` | Navigate suggestion list |
| `Space` | Toggle focused suggestion on/off |
| `Enter` | Confirm: accept [x] suggestions, reject [ ] ones, advance to next item |
| `A` | Set all suggestions to [x] (accept all) |
| `Esc` | Close panel without making any changes to this item |
| `?` | Help |

**After Enter:**
- All `[x]` suggestions → `agenda.accept_classification_suggestion(id)`
- All `[ ]` suggestions → `agenda.reject_classification_suggestion(id)`
- If more items have pending suggestions: advance cursor to next `?` item and
  reopen the panel for that item
- If no more items: close panel, return to Normal mode, show status
  "Resolved N suggestions across M items"

**After Esc:**
- Close panel, return to Normal mode
- No changes made to this item's suggestions
- `?` remains on the item

### Part 3: Inline suggestions in edit-item panel

When the user presses `e` to edit an item that has pending suggestions, the
InputPanel's category list shows those suggestions inline, below a separator,
with a distinct `[?]` marker.

```text
┌─ Edit Item ──────────────────────────────────────────────┐
│ Text: Reimburse Sam for hotel                            │
│ Note: paid on card ending 1142                           │
│                                                          │
│ Categories                                               │
│  [x] Expense                                             │
│  [x] Work                                                │
│  [ ] Budget                                              │
│  [ ] Family                                              │
│  ─── Suggested ──────────────────────────────────        │
│  [?] Travel          matched "hotel" in text             │
│  [?] Reimbursement   LLM: reimbursement language         │
│                                                          │
│  [Save]  [Cancel]                                        │
└──────────────────────────────────────────────────────────┘
```

**Behavior:**
- `[?]` suggestions appear after a `─── Suggested ───` separator
- Navigation with `j/k` moves through both manual categories and suggestions
- `Space` on a `[?]` suggestion toggles it to `[x]` (will accept on Save) or
  `[ ]` (will reject on Save)
- **Untouched `[?]` suggestions remain pending** — no decision is made. This
  differs from the `g?` triage flow where the default is accept. The edit panel
  is a side channel for managing an individual item; the user may be focused on
  text/note edits and not ready to triage suggestions.
- On Save:
  - `[x]` suggestions → `agenda.accept_classification_suggestion(id)`
  - `[ ]` suggestions → `agenda.reject_classification_suggestion(id)`
  - `[?]` suggestions → no action (remain pending)
- On Cancel: no suggestion decisions applied, all remain pending

**Three-state toggle cycle:** `[?]` → `[x]` → `[ ]` → `[?]` (Space cycles).
This lets the user return to "no decision" if they change their mind.

**Implementation:**
- Extend `InputPanel` (or the category list rendering) to include a
  `pending_suggestions: Vec<ReviewSuggestion>` section
- Add `SuggestionDecision` enum: `Pending` / `Accept` / `Reject`
- Track decisions as draft state on the panel, applied on Save
- The separator row is non-selectable (cursor skips it)

### Part 4: Entry points

**Primary — `g?` in Normal mode (bulk triage):**
- Opens review overlay starting at the first `?` item in the current view
- Auto-advances through all `?` items sequentially after each confirmation
- If no items have pending suggestions → show status "No pending suggestions"
- Suggestions default to `[x]` (accept). Optimized for burning through the queue.

**Secondary — `e` edit-item panel (individual item):**
- Suggestions appear inline in the category list as `[?]`
- User can address them while editing, or ignore them
- Suggestions default to `[?]` (no decision). Editing is the primary task.

**Future — `R` in Category Manager (category-centric):**
- Open review overlay filtered to suggestions for the selected category
- Not in initial implementation

### Part 5: What changes from current implementation

**Remove:**
- `Mode::ClassificationReview` (the standalone two-pane modal)
- `Shift+C` keybinding to open it
- `ClassificationFocus::Items` / `ClassificationFocus::Suggestions` enum
- The split-pane rendering in `render/mod.rs`

**Keep:**
- `ClassificationUiState` (rename fields as needed)
- `ClassificationReviewItem` struct
- `rebuild_classification_ui()` — still needed to track pending items
- `pending_suggestion_count_for_item()` — needed for `?` indicator
- `accept_selected_classification_suggestion()` / `reject_...()` — core logic
- All store/agenda layer code unchanged

**Add:**
- `SuggestionReviewState` — new struct for the `g?` overlay panel:
  ```
  struct SuggestionReviewState {
      item_id: ItemId,
      item_text: String,
      note_excerpt: Option<String>,
      current_assignments: Vec<String>,
      suggestions: Vec<ReviewSuggestion>,
      cursor: usize,
      resolved_count: usize,   // running total for status message
      resolved_items: usize,
  }

  struct ReviewSuggestion {
      suggestion: ClassificationSuggestion,
      accepted: bool,  // starts true in g? flow; Space toggles
  }

  enum SuggestionDecision {
      Pending,   // [?] — no decision (edit panel only)
      Accept,    // [x]
      Reject,    // [ ]
  }
  ```
- `Mode::SuggestionReview` — lightweight overlay mode for `g?`
- `g?` handler in Normal mode (via `NormalModePrefix::G`)
- `item_indicator_glyphs()` updated with `has_pending` parameter
- `SUGGESTION_MARKER_SYMBOL`
- Rendering for the overlay panel
- Auto-advance logic after confirmation
- Edit-item panel: `pending_suggestions` section with `SuggestionDecision` state
- Three-state toggle rendering: `[?]` / `[x]` / `[ ]`

### Part 6: Interaction flow walkthrough

**Scenario A: Bulk triage via `g?`**

User has 3 items with pending suggestions.

1. User is browsing the board. They see `?` indicators on 3 items.
   Footer shows: `? 3 items have pending suggestions`

2. User navigates to first `?` item: "Reimburse Sam for hotel"

3. User presses `g?`. Review overlay appears:
   ```
   ┌─ Review Suggestions ─────────────────────────────┐
   │ Reimburse Sam for hotel                           │
   │ Current: Expense, Work                            │
   │                                                   │
   │  >[x] Travel        matched "hotel" in text       │
   │   [x] Reimbursement LLM: reimbursement language   │
   │                                                   │
   │ Space:toggle  Enter:confirm  Esc:cancel            │
   │ (2 more items)                                     │
   └───────────────────────────────────────────────────┘
   ```

4. User sees Travel is correct but Reimbursement is wrong. Presses `j` to move
   to Reimbursement, then `Space` to toggle it off:
   ```
   │   [x] Travel        matched "hotel" in text       │
   │  >[ ] Reimbursement LLM: reimbursement language   │
   ```

5. User presses `Enter`. Travel is accepted, Reimbursement is rejected. The
   overlay automatically opens for the next `?` item: "Call Mom about birthday"
   ```
   ┌─ Review Suggestions ─────────────────────────────┐
   │ Call Mom about birthday                           │
   │ Current: (none)                                   │
   │                                                   │
   │  >[x] Phone Calls   matched "call" in text        │
   │   [x] Family        LLM: family relationship      │
   │                                                   │
   │ Space:toggle  Enter:confirm  Esc:cancel            │
   │ (1 more item)                                      │
   └───────────────────────────────────────────────────┘
   ```

6. Both suggestions look right. User presses `Enter` (or `A` then `Enter`).
   Advances to third item.

7. After confirming the third item, no more pending suggestions exist. Overlay
   closes, user returns to Normal mode. Status: "Resolved 5 suggestions across
   3 items." All `?` indicators are gone.

**Scenario B: Inline review via edit panel**

1. User presses `e` to edit "Reimburse Sam for hotel" (which has `?` indicator).

2. Edit panel opens. Below the normal category toggles, a separator and
   suggested categories appear:
   ```
   │ Categories                                        │
   │  [x] Expense                                      │
   │  [x] Work                                         │
   │  [ ] Budget                                       │
   │  ─── Suggested ───────────────────────────        │
   │  [?] Travel          matched "hotel" in text      │
   │  [?] Reimbursement   LLM: reimbursement language  │
   ```

3. User is primarily editing the note. While in the category list, they press
   Space on Travel to toggle `[?]` → `[x]`, and Space twice on Reimbursement
   to toggle `[?]` → `[x]` → `[ ]`.

4. User presses `S` (Save). The item text/note are saved. Travel is accepted.
   Reimbursement is rejected. The `?` indicator disappears from this item.

5. If the user had pressed Cancel instead, no suggestion decisions would be
   applied — both would remain `[?]` (pending).

### Part 7: Edge cases

- **Item has only 1 suggestion:** Panel still opens; user confirms or cancels.
  Single Enter resolves it.

- **All suggestions toggled off:** Enter rejects all. Item loses `?`. Advance
  to next.

- **User presses Esc mid-triage:** Panel closes, no changes to current item.
  Other items' `?` indicators remain. User can re-enter triage later.

- **No pending suggestions when `g?` pressed:** Status message "No pending
  suggestions." No panel opens.

- **Suggestion accepted triggers cascade:** The cascade may produce NEW
  suggestions for other items. After confirming, the panel should re-check
  pending count and potentially show newly-created suggestions on the next item.

- **Item was modified between opening panel and confirming:** The suggestion's
  `item_revision_hash` may no longer match. The accept call will still work
  (the engine handles this), but if the suggestion was superseded, accept is
  a no-op. The panel should refresh after confirm and skip items with no
  remaining pending suggestions.

## Implementation Checklist

### Step 1: `?` glyph in board view

**`crates/aglet-tui/src/ui_support.rs`:**
- [ ] Add `pub(super) const SUGGESTION_MARKER_SYMBOL: &str = "?";`
- [ ] Update `item_indicator_glyphs(is_done, is_blocked, has_note)` signature
      to `item_indicator_glyphs(is_done, is_blocked, has_pending, has_note)` —
      `?` goes between `&` (blocked) and `♪` (note)
- [ ] Update existing test `item_indicator_glyphs_supports_all_three_indicators`
      → rename to `..._all_four_indicators`, assert `"✓&?♪"`

**`crates/aglet-tui/src/render/mod.rs`:**
- [ ] Update call site ~line 2052 (table/column board mode): pass
      `self.pending_suggestion_count_for_item(item.id) > 0` as `has_pending`
- [ ] Update call site ~line 2386 (list board mode): same
- [ ] Update call site ~line 2618 (card board mode): same
- [ ] Update Normal mode footer status (~line 3345): change
      `classification_pending_suffix()` format to `"? N pending suggestions"`

**`crates/aglet-tui/src/app.rs`:**
- [ ] Update `classification_pending_suffix()` (~line 873) to return
      `"? N pending suggestions"` instead of
      `"N classification suggestion(s) pending"`

### Step 2: New types and `Mode::SuggestionReview`

**`crates/aglet-tui/src/lib.rs`:**
- [ ] Add `Mode::SuggestionReview` variant to `enum Mode`
- [ ] Add `SuggestionReviewState` struct:
      ```rust
      struct SuggestionReviewState {
          item_id: ItemId,
          item_text: String,
          note_excerpt: Option<String>,
          current_assignments: Vec<String>,
          suggestions: Vec<ReviewSuggestion>,
          cursor: usize,
          resolved_count: usize,
          resolved_items: usize,
      }
      ```
- [ ] Add `ReviewSuggestion` struct:
      ```rust
      struct ReviewSuggestion {
          suggestion: ClassificationSuggestion,
          accepted: bool, // starts true in g? flow
      }
      ```
- [ ] Add `SuggestionDecision` enum (for edit panel, Step 5):
      ```rust
      enum SuggestionDecision { Pending, Accept, Reject }
      ```
- [ ] Add `suggestion_review: Option<SuggestionReviewState>` field to `App`
- [ ] Add `Mode::SuggestionReview` to all `match self.mode` arms that need it
      (key dispatch in `handle_key`, render dispatch, footer hints, status text)

### Step 3: Wire `g?` entry point

**`crates/aglet-tui/src/modes/board.rs`:**
- [ ] In `handle_normal_key`, extend the `NormalModePrefix::G` match arm:
      add `(NormalModePrefix::G, KeyCode::Char('?')) => { self.open_suggestion_review(agenda)?; }`
- [ ] Update the g-prefix status text (~line 2258) to:
      `"g-prefix: ga=All Items, g/=Global search, g?=Review suggestions"`

**`crates/aglet-tui/src/modes/` — new file `suggestion_review.rs`:**
- [ ] Create `crates/aglet-tui/src/modes/suggestion_review.rs`
- [ ] Add `mod suggestion_review;` to `crates/aglet-tui/src/modes/mod.rs`
- [ ] Implement `App::open_suggestion_review(&mut self, agenda)`:
      - Call `rebuild_classification_ui()` to ensure fresh data
      - Find first item in `classification_ui.review_items` that has suggestions
      - If none: set status `"No pending suggestions"`, return
      - Build `SuggestionReviewState` from that item (all suggestions `accepted: true`)
      - Set `self.suggestion_review = Some(state)`
      - Set `self.mode = Mode::SuggestionReview`
- [ ] Implement `App::advance_suggestion_review(&mut self, agenda)`:
      - Refresh classification UI
      - Find next item with pending suggestions
      - If found: build new `SuggestionReviewState`, update `self.suggestion_review`
      - If none: close overlay, set `self.mode = Mode::Normal`,
        set status `"Resolved N suggestions across M items"`

### Step 4: Key handling for `g?` review mode

**`crates/aglet-tui/src/modes/suggestion_review.rs`:**
- [ ] Implement `App::handle_suggestion_review_key(code, agenda) -> TuiResult<bool>`:
      - `Esc`: set `self.suggestion_review = None`, `self.mode = Mode::Normal`
      - `j`/`Down`: increment cursor (clamped to suggestions.len() - 1)
      - `k`/`Up`: decrement cursor (clamped to 0)
      - `Space`: toggle `suggestions[cursor].accepted`
      - `A`: set all suggestions to `accepted = true`
      - `Enter`: confirm batch —
        for each suggestion: if `accepted` call `agenda.accept_classification_suggestion(id)`,
        else call `agenda.reject_classification_suggestion(id)`;
        increment `resolved_count` and `resolved_items`;
        call `self.refresh(agenda.store())?`;
        call `self.advance_suggestion_review(agenda)?`
      - `?`: open help panel
- [ ] Wire `Mode::SuggestionReview` in the main key dispatch (`handle_key` in
      `lib.rs` or `modes/mod.rs`)

### Step 5: Render the `g?` review overlay

**`crates/aglet-tui/src/render/mod.rs`:**
- [ ] Add `fn render_suggestion_review(&self, frame, area)`:
      - Calculate centered overlay rect (e.g., 60 wide × suggestion count + 10 tall,
        capped at 80% of terminal)
      - Render `Clear` + `Block` with border and title `"Review Suggestions"`
      - Render item header: text (bold), note excerpt (dim), current assignments
      - Render suggestion list: for each suggestion, render
        `[x]` or `[ ]` + category name + rationale (dimmed);
        prefix focused row with `>`
      - Render footer: keybinding hints + remaining-items count
        `"(N more items have pending suggestions)"`
- [ ] In the main `render` function, add: if `Mode::SuggestionReview`,
      render board first (underneath), then call `render_suggestion_review`
      as an overlay on top
- [ ] Add `Mode::SuggestionReview` footer hints:
      `[("Space", "toggle"), ("Enter", "confirm"), ("A", "accept all"), ("Esc", "cancel")]`
- [ ] Add `Mode::SuggestionReview` status text (return current status string)

### Step 6: Edit-item panel — inline suggestions

**`crates/aglet-tui/src/input_panel.rs`:**
- [ ] Add field `pub(crate) pending_suggestions: Vec<(ClassificationSuggestion, SuggestionDecision)>`
      (empty by default; populated when opening edit panel for item with `?`)
- [ ] In `InputPanel::new()` or builder, initialize `pending_suggestions` to empty vec
- [ ] Add method to compute total navigable row count: existing categories
      + (1 separator if suggestions non-empty) + suggestion count
- [ ] Update `handle_key` for `InputPanelFocus::Categories`:
      - Adjust `MoveCategoryCursor` bounds to include suggestion rows
      - When cursor is on a suggestion row and Space is pressed, return new action
        `ToggleSuggestion` (or handle inline with three-state cycle)
      - Separator row: cursor skips (delta jumps over it)

**`crates/aglet-tui/src/app.rs`:**
- [ ] In `open_input_panel_edit_item()`: look up pending suggestions for the item
      via `pending_suggestion_count_for_item()` / `classification_ui.review_items`;
      populate `panel.pending_suggestions` with `(suggestion, SuggestionDecision::Pending)`
- [ ] In the save handler for edit-item: after saving text/note/categories,
      iterate `panel.pending_suggestions`:
      - `Accept` → `agenda.accept_classification_suggestion(id)`
      - `Reject` → `agenda.reject_classification_suggestion(id)`
      - `Pending` → skip (no action)

**`crates/aglet-tui/src/render/mod.rs`:**
- [ ] In the InputPanel category list rendering (AddItem/EditItem):
      after the normal category rows, if `pending_suggestions` is non-empty:
      - Render a `"─── Suggested ───"` separator line (dimmed, non-selectable)
      - Render each suggestion as `[?]`/`[x]`/`[ ]` + category name + rationale
      - Highlight focused row if cursor is in suggestion range
- [ ] Update category list height calculation to account for separator + suggestion rows

### Step 7: Remove old ClassificationReview mode

**`crates/aglet-tui/src/lib.rs`:**
- [ ] Remove `Mode::ClassificationReview` variant from `enum Mode`
- [ ] Remove `ClassificationFocus` enum
- [ ] Remove `focus` field from `ClassificationUiState`
      (keep `pending_count`, `review_items`, `config`, `selected_item_index`,
      `selected_suggestion_index` — these are still used by `pending_suggestion_count_for_item`)

**`crates/aglet-tui/src/modes/classification.rs`:**
- [ ] Remove `open_classification_review()` method
- [ ] Remove `handle_classification_review_key()` method
- [ ] Remove `cycle_classification_focus()`, `move_classification_selection()`
- [ ] Remove `accept_selected_classification_suggestion()`,
      `reject_selected_classification_suggestion()`,
      `accept_all_selected_classification_item()`,
      `reject_all_selected_classification_item()`
- [ ] Keep `continuous_mode_index()`, `continuous_mode_from_index()`,
      `continuous_mode_label()` — still used by Category Manager mode picker
- [ ] (Or delete the file entirely and move the 3 helpers elsewhere)

**`crates/aglet-tui/src/modes/board.rs`:**
- [ ] Remove `KeyCode::Char('C') => { self.open_classification_review(); }` (~line 2162)
- [ ] Free the `C` key (leave unbound or repurpose later)

**`crates/aglet-tui/src/render/mod.rs`:**
- [ ] Remove `fn render_classification_review()` (~line 2960, ~220 lines)
- [ ] Remove `Mode::ClassificationReview` arms from footer hints (~line 3367)
      and status text (~line 3340)

### Step 8: Tests

**`crates/aglet-tui/src/lib.rs` (test module) and/or new test file:**

`?` glyph tests:
- [ ] `item_indicator_glyphs_shows_question_mark_for_pending` — assert `"?"` when
      only `has_pending` is true
- [ ] `item_indicator_glyphs_combines_all_four` — assert `"✓&?♪"` when all true
- [ ] `board_renders_question_mark_on_item_with_pending_suggestion` — create item,
      create pending suggestion via SuggestReview mode, refresh, verify `?` appears
      in rendered output (or verify via `pending_suggestion_count_for_item > 0`)

`g?` overlay tests:
- [ ] `g_question_opens_suggestion_review_for_first_pending_item` — create 2 items
      with pending suggestions, press `g` then `?`, assert `mode == SuggestionReview`,
      assert overlay shows first item
- [ ] `g_question_shows_status_when_no_pending` — no pending suggestions,
      press `g?`, assert mode stays Normal, status contains "No pending"
- [ ] `suggestion_review_space_toggles_accepted` — open overlay, verify first
      suggestion `accepted == true`, press Space, verify `accepted == false`,
      press Space again, verify `accepted == true`
- [ ] `suggestion_review_enter_confirms_and_advances` — 2 items with suggestions,
      open overlay on first, press Enter (accept all defaults), verify first item's
      suggestions accepted, overlay advances to second item
- [ ] `suggestion_review_enter_on_last_item_closes_overlay` — 1 item with suggestion,
      open overlay, press Enter, verify mode returns to Normal, status shows resolved count
- [ ] `suggestion_review_esc_cancels_without_changes` — open overlay, toggle a
      suggestion off, press Esc, verify suggestion status still Pending in store
- [ ] `suggestion_review_reject_toggled_off` — open overlay, toggle off a suggestion,
      press Enter, verify that suggestion is Rejected in store
- [ ] `suggestion_review_a_sets_all_accepted` — open overlay, toggle some off,
      press A, verify all `accepted == true`

Edit panel tests:
- [ ] `edit_panel_shows_pending_suggestions_below_separator` — create item with
      pending suggestion, open edit panel, verify `pending_suggestions` is populated
- [ ] `edit_panel_three_state_toggle_cycle` — cursor on suggestion row,
      Space cycles `Pending → Accept → Reject → Pending`
- [ ] `edit_panel_save_accepts_and_rejects_suggestions` — toggle one to Accept,
      one to Reject, save, verify store has Accepted and Rejected statuses
- [ ] `edit_panel_save_skips_pending_suggestions` — leave suggestion as Pending,
      save, verify suggestion still Pending in store
- [ ] `edit_panel_cancel_leaves_all_pending` — toggle suggestions, cancel,
      verify all still Pending in store

Old mode removal tests:
- [ ] Remove `uppercase_c_opens_classification_review_from_normal_mode` (~line 7426)
- [ ] Remove `classification_review_enter_accepts_selected_suggestion` (~line 7460)
- [ ] Remove `classification_review_with_no_pending_suggestions_starts_on_items_pane` (~line 7499)
- [ ] Keep `category_manager_m_opens_picker_and_enter_applies_classification_mode` (~line 7513)
