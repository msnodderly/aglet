# TUI Add Item Workflow Analysis & Proposal

**Date**: 2026-02-20
**Status**: Analysis & Recommendation

## Executive Summary

The current TUI "add item" workflow is deliberately minimal (type text → Enter → done). However, the **#1 high-priority feature request** asks for a rich add panel matching the item edit overlay, enabling users to:
- Capture text + note simultaneously
- Assign categories at creation time
- Preview how the item will parse (especially `When` field)
- Set categories before insertion automatically applies rules

This analysis proposes a **unified Input Panel abstraction** that works as both add and edit, eliminating duplication and improving user mental model consistency.

---

## 1. Current State: The Add Workflow Gap

### 1.1 Fallows Article Insight

From the Lotus Agenda essay, the three core concepts are:
- **Items**: basic units of information (tasks, appointments, data)
- **Categories**: attributes (time, priority, people, themes)
- **Views**: presentations of items filtered by categories

The genius of Agenda was **automatic category assignment on input**. Rules could trigger: "Any item with 'Call Mom' goes to High Priority." The Fallows article emphasizes that the simplicity and power came from treating input as an intelligent operation, not just text capture.

### 1.2 Current TUI Add (Mode::AddInput)

```
User presses 'n' in Normal mode
    ↓
Clear input buffer, enter Mode::AddInput
    ↓
User types text (single-line text editor via TextBuffer)
    ↓
User presses Enter
    ↓
Call create_item_in_current_context() with text only
    ↓
Parse When, apply section's on_insert_assign rules
    ↓
Return to Normal mode
```

**Limitations**:
- No note capture (user must press 'm' after, edit in separate flow)
- No category assignment at creation (user must press 'a' after)
- No preview of parsed When field before commit
- Context-specific: insertion assigns categories based on section rules, but user can't see or override
- Discordance with rich item edit flow (5 fields/buttons) vs. minimal add flow (1 text field)

### 1.3 Current TUI Edit (Mode::ItemEdit)

```
User presses Enter/e on item in Normal mode
    ↓
Mode::ItemEdit, popup shows:
  - Text field (editable, multiline capable)
  - Note field (editable, multiline capable)
  - Categories button → opens picker (Mode::ItemAssignPicker)
  - Save/Cancel buttons
    ↓
User can Tab between fields, edit everything, assign categories
    ↓
User presses Enter on Save button
    ↓
Persist all changes atomically
    ↓
Return to Normal mode
```

**Consistency problem**: The edit flow is rich and discoverable; the add flow is bare-bones. Users must learn two separate workflows.

---

## 2. Feature Request Analysis

### 2.1 High-Priority Feature (#1: "Add Item Panel")

```
ID: ae49454d-e4b2-47f7-a01a-e7c4e9c252cf
Status: Open
Area: UX

Text: "The experience to add a new item should be a similar interface
       to the edit overlay/panel."

Note: "Hitting 'n' to add a new item should bring up a panel similar
       to the Edit Item panel, or exactly the same as the Edit Item panel,
       except with an empty text and note fields."
```

**This is not a bug report — it's a design request**: Users recognize the edit panel is powerful and want the same capabilities when creating items.

### 2.2 Related High-Priority Features

- **"Save prompt on view edit exit"**: Currently ViewEdit's `Enter` saves implicitly; should prompt if unsaved.
- **"S to save (not Enter)"**: Proposes capital S for explicit save, freeing Enter for other meanings.
- **"Section compatibility validation"**: When adding a section, validate it doesn't conflict with view criteria.
- **"Views without criteria"**: Allow views with no top-level criteria (show all items).
- **"CLI feature parity"**: CLI lacks TUI's view/section richness.

These are all about **making high-impact operations explicit and preventing silent saves/discards**.

---

## 3. Root Problem: Multiple Input Interfaces

The codebase has three independent text input paths:

1. **AddInput mode** (`input.rs`): Single-line text for new items
2. **ItemEdit popup** (`modes/board.rs`): Multi-field form for editing items
3. **Various name inputs** (ViewCreateName, ViewRename, etc.): Text fields for naming

Each duplicates:
- Cursor logic (via shared TextBuffer ✓ — already factored in Phase 1)
- Field cycling logic (ItemEditFocus enum, cycle methods)
- Save/cancel semantics (scattered across modes)
- Status messages (inconsistent tone and formatting)

**Better approach**: A unified **Input Panel** abstraction that:
- Wraps a form with multiple fields (text, note, picker overlays, buttons)
- Supports context-specific configuration (add vs. edit vs. name-input)
- Provides consistent save/cancel/focus behavior
- Reuses the same rendering code

---

## 4. Proposed General Solution: Input Panel Abstraction

### 4.1 Design Goals

1. **Unify add and edit workflows** into one interface
2. **Make save explicit** (following the TUI redesign proposal's principle P3: "Same key means same thing")
3. **Preserve section insert rules** but make them visible to the user
4. **Reduce mode explosion** by using sub-states within a single InputPanel mode
5. **Support configuration** so the panel can be:
   - "Add item" (empty text/note, show section insert preview)
   - "Edit item" (pre-filled text/note, show current categories)
   - "Name input" (single field, for views/categories)

### 4.2 Core Abstraction

```rust
/// Identifies what type of input panel is open.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum InputPanelKind {
    AddItem,           // new item in current section
    EditItem,          // edit existing item
    NameInput,         // single text field (for views, categories)
}

/// Which field the cursor is in (varies by kind).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum InputPanelFocus {
    Text,
    Note,
    CategoriesButton,
    SaveButton,
    CancelButton,
}

/// State of the input panel.
#[derive(Clone)]
struct InputPanel {
    kind: InputPanelKind,
    text: TextBuffer,           // reuse shared TextBuffer
    note: TextBuffer,
    categories: HashSet<CategoryId>,  // for edit mode
    focus: InputPanelFocus,
    item_id: Option<ItemId>,    // Some if editing, None if adding
    preview_when: Option<NaiveDateTime>,  // parsed When value
    preview_context: String,    // "Inserting into Slot A" or similar
}

impl InputPanel {
    fn new_add_item(section_title: String) -> Self { /* ... */ }
    fn new_edit_item(item_id: ItemId, item: Item) -> Self { /* ... */ }
    fn new_name_input(current_name: String, label: &str) -> Self { /* ... */ }

    fn render(&self, area: Rect, theme: &Theme) -> Vec<Line>;
    fn handle_key(&mut self, code: KeyCode) -> InputPanelAction;
}

#[derive(Clone, Copy, Debug)]
enum InputPanelAction {
    // Field navigation
    FocusNext,
    FocusPrev,

    // Open overlays
    OpenCategoryPicker,
    OpenBucketPicker,

    // Completion
    Save,
    Cancel,

    // No-op (text input was consumed)
    Handled,

    // Key wasn't consumed by panel
    Unhandled,
}
```

### 4.3 Concrete Workflow: Add Item with Categories

**User experience** (proposed):

```
1. Press 'n' in Normal → enter InputPanel(AddItem)
   Display:
     ┌─ Add Item: Open ────────────────────┐
     │ Text: [cursor here, empty]           │
     │ Note:                                │
     │ Categories: (none assigned)          │
     │ Preview: Will insert into "Open"     │
     │          on_insert_assign: Status→open, Priority→medium
     │                                       │
     │ [Save]  [Cancel]                     │
     └──────────────────────────────────────┘

2. Type: "Fix login timeout"
   → Text field shows typed text

3. Press Tab → focus moves to Note field
   Status: "Note (empty, press Tab to categories)"

4. Type: "User reported slow auth on mobile"
   → Note field shows multiline text

5. Press Tab → focus moves to Categories button
   Status: "Categories (none). Press Enter to assign."

6. Press Enter on Categories button → overlay opens
   Show category picker filtered to non-exclusive families
   User toggles: +Priority/High, +Area/Infrastructure
   Press Enter or Esc → dismiss overlay, focus returns to Categories button

7. Preview updates:
   ┌─ Add Item: Open ────────────────────┐
   │ Text: Fix login timeout              │
   │ Note: User reported slow auth...     │
   │ Categories: High, Priority, Infra... │
   │ Preview: Will insert into "Open"     │
   │          on_insert_assign rules: +Status/open
   │          (Priority/High already assigned)
   │                                       │
   │ [Save]  [Cancel]                     │
   └──────────────────────────────────────┘

8. Press Tab → focus moves to Save button

9. Press S → Save
   → Create item with text, note, categories
   → Apply on_insert_assign rules (Status→open)
   → Return to Normal, status: "Item added (parsed when: 2026-02-24 15:00)"
```

**vs. current workflow**:

```
1. Press 'n' → Mode::AddInput, bare input line
2. Type: "Fix login timeout"
3. Press Enter → item created, no note/categories
4. Press 'm' to edit note (enters Mode::NoteEdit)
5. Type note, press Enter
6. Press 'a' to assign categories (enters Mode::ItemAssignPicker)
7. Toggle categories, press Enter
```

**Current workflow requires 7 steps and three mode changes; proposed is 1 mode, 9 steps but all discoverable in one interface.**

---

## 5. Implementation Roadmap

### Phase 5a: InputPanel Abstraction (New)

1. Create `InputPanel` struct in new file `input_panel.rs`
2. Implement `InputPanel::new_add_item()`, `new_edit_item()`, `new_name_input()`
3. Implement key dispatch: `handle_key() → InputPanelAction`
4. Implement rendering (text/note/categories/buttons, optional preview line)
5. Add `Mode::InputPanel` variant to Mode enum
6. Wire Mode::InputPanel key dispatch to `InputPanel::handle_key()`
7. Render InputPanel in its own centered region (replaces ItemEdit popup)
8. **Test**: Add 10+ tests for field navigation, category toggling, save/cancel semantics

### Phase 5b: Migrate Add (AddInput → InputPanel)

1. Replace `Mode::AddInput` entry point with `Mode::InputPanel` (kind=AddItem)
2. Wire 'n' key to create InputPanel(AddItem)
3. On Save: call same `create_item_in_current_context()` logic
4. Delete `Mode::AddInput` and `handle_add_key()` in the same commit (no deprecated alias needed)
5. Update status messages to match InputPanel semantics
6. **Test**: Verify add flow still works, categories are assigned, When is parsed

### Phase 5c: Migrate Edit (ItemEdit → InputPanel)

1. Create InputPanel(EditItem) variant
2. Replace Mode::ItemEdit key dispatch with InputPanel dispatch
3. On Save: call `update_item()` with merged text/note/categories
4. **Test**: Edit workflow produces same results as before

### Phase 5d: Migrate Name Inputs (ViewCreateName, ViewRename, CategoryCreate, etc.)

1. Create InputPanel(NameInput) variant
2. Replace each mode's entry point with `InputPanel(NameInput { label: "View name" })`
3. Delete old modes (ViewCreateName, etc.)
4. **Test**: View/category creation/rename still work

### Phase 5e: Explicit Save Key (S, not Enter)

1. Post-5b/5c: Change InputPanel's primary save trigger from Enter-on-SaveButton to `S` from any focus
2. Enter on Text field: move focus to Note (FocusNext)
3. Enter on Note field: insert newline (multiline edit; no implicit save)
4. Enter on SaveButton: keep as secondary save binding
5. Enter on CancelButton: cancel (unchanged)
6. Update all status messages and footer hint bar text to show `S:save`
7. **Breaking change for existing users**: document in `spec/tui-ux-redesign.md §14`
8. **Test**: S saves from Text/Note focus; Enter in Text/Note does not save; Enter on SaveButton still saves

---

## 6. Secondary Improvements (Enabled by InputPanel)

### 6.1 Preview and Validation

The InputPanel can show a **preview footer** before save:

```
Preview: Will add to "Open"
         Text: Fix login timeout
         Categories: High, Priority, Infrastructure
         Parsed When: 2026-02-24 (from "login timeout" hashtags, if any)
         on_insert_assign will add: Status→open
```

### 6.2 Smart When-Parsing Feedback

In add mode, show the parsed When value in the preview so user can verify:

```
Text: "Call Mom on Tuesday at 2pm"
Preview When: 2026-02-25 14:00 (assuming Tuesday is 2026-02-25)
```

If the parse is ambiguous, highlight it and prompt to clarify.

### 6.3 Validation Gates

Before saving, validate:
- Text is not empty (current gate)
- If editing, at least one field changed (avoid silent no-ops)
- If adding to a section, the categories don't violate exclusivity
- Note field doesn't exceed size limits (if any)

---

## 7. Relationship to TUI Redesign (§4 in tui-ux-redesign.md)

The tui-ux-redesign document proposes a **unified ViewEdit** mode combining 10 old modes. The InputPanel abstraction follows the same principle:

- **Before**: Multiple modes (AddInput, ItemEdit, NoteEdit, ViewCreateName, CategoryCreate, etc.)
- **After**: Single InputPanel mode with sub-kinds and states
- **Benefit**: Consistent key vocabulary, predictable Esc behavior, reusable rendering

The two refactorings are **orthogonal** — InputPanel is about item/name input UI, ViewEdit is about view/section configuration UI.

Note: Section 7 previously referenced `Mode::ItemEditInput` (old name). This was renamed to `Mode::ItemEdit` in Phase 2d of the TUI redesign. All references below use the current name.

---

## 8. Risk Mitigation

### 8.1 Scope Creep

**Risk**: InputPanel becomes too complex, trying to handle every input type.

**Mitigation**: Strict separation of concerns:
- InputPanel handles form layout, field focus cycling, and rendering
- Business logic (create, update, parse When, apply rules) stays in the caller
- InputPanel returns clean actions (Save/Cancel with data), not side effects

### 8.2 Regression in Existing Workflows

**Risk**: Migrating add/edit/name inputs breaks something.

**Mitigation**:
- Build InputPanel alongside old modes (Phase 5a)
- Test exhaustively before deleting old modes
- Keep a shadow build of old modes for 1–2 commits during transition
- Smoke test: create/edit/rename in all relevant views

### 8.3 Discoverability Regression

**Risk**: Rich InputPanel is less discoverable than minimal Add.

**Mitigation**:
- Status bar and footer hint bar must be clear
- On entering add mode: "Add Item: type text, Tab for note, Tab for categories, S to save"
- Help text in status during common actions (Tab, Enter on buttons, etc.)

---

## 9. Open Questions

1. **Should add auto-assign categories from section rules, or just show them in preview?**
   - Current: silent auto-assign via `on_insert_assign` after item creation
   - Proposed: show in preview footer before save, keep auto-assignment semantics
   - Risk: changing the moment of assignment (pre-save vs post-save) could have subtle side effects if the user removes a category that would otherwise be auto-assigned
   - **Decision**: keep auto-assignment, show it clearly in the preview footer as "will also assign: X". User cannot veto auto-assign; they can unassign after save.

2. **Should 'S' for save break existing muscle memory?**
   - Current: Enter saves in Mode::ItemEdit
   - Proposed: S saves in InputPanel (consistent with ViewEdit target §4.7)
   - Impact: requires documentation and migration guide
   - **Decision**: defer to Phase 5e, implement after 5a–5d stabilize and get user feedback.

3. **Should InputPanel support other field types (checkboxes, pickers)?**
   - Likely future: done/not-done toggle, exclusive category selector
   - **Decision**: out of scope for Phase 5. Use picker overlays (like ViewEdit) for complex controls. Revisit when there's a concrete need.

4. **How to handle the preview When parsing?**
   - Current: When is parsed and stored on save
   - Proposed: parse on each keystroke in Text field, display parsed value in preview footer
   - Issue: if parse is wrong, user cannot override the parsed value (no editing of the structured when field)
   - **Decision**: Phase 5b — show preview only, no override. Track "edit parsed When" as a follow-on feature request if users report pain.

5. **Should NoteEdit (Mode::NoteEdit, 'm' key) be migrated to InputPanel?**
   - Currently 'm' opens a fullscreen note editor for the selected item without showing text
   - InputPanel(EditItem) shows both text and note, making 'm' redundant
   - **Decision**: after Phase 5c, deprecate 'm' in favour of 'e' (InputPanel EditItem, then Tab to Note). Remove Mode::NoteEdit in a Phase 5f cleanup. Track as a separate commit.

---

## 10. Success Criteria

✓ Adding items uses the same discoverable interface as editing items
✓ User can assign categories at creation time
✓ Preview shows parsed When and on_insert_assign context
✓ Save is explicit (one button, one key, clear feedback)
✓ Esc always cancels without side effects
✓ Mode count doesn't increase (consolidation, not expansion)
✓ All existing add/edit/name workflows still work
✓ Status messages are consistent across all input contexts
✓ Existing tests pass (100% coverage maintained)

---

## 11. Example: Day in the Life (Post-InputPanel)

**Morning**: Triaging tasks

```
1. User presses 'v' → ViewPicker → selects "Today"
2. Sees tasks in "Open" and "Done" sections
3. Presses 'n' in "Open" section → InputPanel adds to "Open"
4. Types: "Review PR #1234"
5. Tab → note field, types: "See feedback in Slack thread"
6. Tab → categories, presses Enter → picker opens
7. Toggles: +Review, +Infrastructure
8. Esc → focus back to categories button
9. Tab → focus to Save button
10. Press S → item created with all metadata, Status/open assigned automatically
11. Status bar: "Item added (parsed when: today 09:00)"
```

**Afternoon**: Editing existing task

```
1. User selects existing item, presses Enter → InputPanel(EditItem)
2. Can see and edit text, note, categories
3. Toggle categories: remove Review, add Meeting
4. Edit note to add Slack thread link
5. Press S → saves all changes atomically
6. Returns to board, item still selected, now shows "Meeting" instead of "Review"
```

---

## 12. Status & Next Steps

Implementation issues filed in `aglet-features.ag` (2026-02-20):

| Phase | FR ID | Title |
|-------|-------|-------|
| 5a-1 | `be6f0754` | Define types and implement key dispatch |
| 5a-2 | `19601dd8` | Implement rendering and wire into event loop |
| 5a-3 | `ac843fae` | Add unit tests for InputPanel |
| 5b | `cfb526a4` | Migrate Mode::AddInput to InputPanel(AddItem) |
| 5c | `0ce92977` | Migrate Mode::ItemEdit to InputPanel(EditItem) |
| 5d-1 | `fa40618c` | Migrate view name inputs to InputPanel(NameInput) |
| 5d-2 | `74c1a942` | Migrate category name inputs to InputPanel(NameInput) |
| 5e | `de4b9c32` | Standardize save key to S across InputPanel and ViewEdit |

Open questions resolved above (§9). Remaining actions:

1. **Implement 5a first** — it is the foundation; 5b–5e are all blocked on it.
2. **Get user feedback after 5b** before merging 5c. The add flow change is visible; verify it feels natural.
3. **Deprecate Mode::NoteEdit** in a Phase 5f cleanup after 5c ships (see §9.5).
4. **CLI parity**: after InputPanel is stable, evaluate exposing `--note` and `--category` flags on `agenda-cli add` to match the TUI capability (see CLI FR `6d47d7b2`).
