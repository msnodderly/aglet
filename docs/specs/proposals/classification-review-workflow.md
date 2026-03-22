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

## Implementation Sequence

### Step 1: Add `?` glyph to board view
- Update `item_indicator_glyphs()` signature to add `has_pending_suggestions`
- Add `SUGGESTION_MARKER_SYMBOL` constant
- Update all three render call sites (table, list, card modes)
- Update footer status text: `"? N pending suggestions"`

### Step 2: Add `SuggestionReviewState` and `Mode::SuggestionReview`
- Define `SuggestionReviewState`, `ReviewSuggestion`, `SuggestionDecision`
- Add `Mode::SuggestionReview` variant
- Wire `g?` in Normal mode (extend `NormalModePrefix::G` handler)

### Step 3: Render the `g?` review overlay
- Centered overlay panel (similar to InputPanel rendering)
- Item header (text, note excerpt, current assignments)
- Suggestion list with `[x]`/`[ ]` toggles and `>` cursor
- Footer with keybinding hints and remaining-items count

### Step 4: Key handling for `g?` review mode
- `j/k/↑/↓` navigation
- `Space` toggle
- `Enter` confirm (accept `[x]`, reject `[ ]`) and auto-advance
- `A` set all to `[x]`
- `Esc` cancel (no changes, close overlay)
- Auto-advance to next `?` item; close when queue empty

### Step 5: Edit-item panel — inline suggestions
- Add `pending_suggestions` with `SuggestionDecision` to InputPanel state
- Render `─── Suggested ───` separator and `[?]`/`[x]`/`[ ]` rows
- Three-state toggle cycle on Space: `[?]` → `[x]` → `[ ]` → `[?]`
- On Save: accept `[x]`, reject `[ ]`, skip `[?]`
- Separator row is non-selectable (cursor skips)

### Step 6: Remove old ClassificationReview mode
- Remove `Mode::ClassificationReview`
- Remove `Shift+C` keybinding
- Remove two-pane rendering
- Clean up `ClassificationFocus` enum
- Update g-prefix status text to include `g?`

### Step 7: Tests
- `?` glyph appears on items with pending suggestions
- `?` glyph disappears after all suggestions resolved
- `g?` opens review overlay for focused `?` item
- `g?` jumps to next `?` item if focused item has none
- `g?` shows status when no pending suggestions exist
- Space toggles suggestion state in overlay
- Enter confirms and auto-advances to next item
- Esc cancels without changes
- Panel closes when last item is resolved
- Edit panel shows `[?]` suggestions below separator
- Three-state toggle cycle works in edit panel
- Save in edit panel accepts `[x]`, rejects `[ ]`, skips `[?]`
- Cancel in edit panel leaves all suggestions pending
