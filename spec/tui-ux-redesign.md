# TUI UX Redesign Proposal

Date: 2026-02-18
Status: Draft

## 1. Problem Statement

The current TUI works but is clunky to use. The primary pain points:

1. **Mode explosion**: 30 Mode variants with inconsistent navigation patterns
2. **Esc behavior is inconsistent**: sometimes cancels, sometimes goes back one level, sometimes clears state, sometimes does nothing useful
3. **Two parallel editing paths**: View Manager (3-pane, handles criteria+columns) and View Editor (overlay, handles criteria+sections+unmatched) duplicate concerns and have different save semantics
4. **Deep modal drilling**: creating a filtered view with sections requires navigating 7+ mode transitions
5. **Cursor/field model is ad-hoc**: three independent cursor implementations (input, item_edit_note, category_config_note) with duplicated logic; no shared abstraction for "an editable text field"
6. **`view_return_to_manager` flag**: a boolean that silently changes where Esc goes, making behavior unpredictable to the user

This proposal redesigns the interaction model without changing the data model (`View`, `Section`, `Column`, `Query`, `Category`, `Item`).

## 2. Design Principles

**P1. Esc always means "go back one level."** Every mode has exactly one parent. Esc returns to that parent. No conditional branching on flags. **Exception**: in Normal mode (the root), Esc clears the current section's text filter if one is set (see §10). This is the only mode where Esc performs a contextual action rather than navigating to a parent, because Normal has no parent to return to.

**P2. One editing surface per concept.** Views are edited in one place, not two. Categories are edited in one place.

**P3. Consistent key vocabulary.** The same key means the same thing everywhere it appears.

**P4. A shared field cursor model.** One abstraction for editable text, reused by every input context.

**P5. No new features.** Same data model, same persistence, same capabilities. Just better organized.

## 3. Esc Transition Map (Current vs. Proposed)

### 3.1 Current Esc Behavior (Inconsistencies highlighted)

| Mode | Esc goes to | Notes |
|------|-------------|-------|
| Normal | clears filter (if set), else no-op | **Inconsistent**: Esc does a filter-specific action, not "go back" |
| AddInput | Normal | OK |
| ItemEditInput | Normal | OK (discards edit) |
| NoteEditInput | Normal | OK (discards edit) |
| FilterInput | Normal + clears filter | **Inconsistent**: Esc = cancel + clear, but Enter = apply. No way to cancel *without* clearing. |
| ViewPicker | Normal | OK |
| ViewManagerScreen | ViewPicker | OK, but discards unsaved silently |
| ViewEditor | ViewPicker | OK (discards draft) |
| ViewCreateNameInput | ViewPicker *or* ViewManagerScreen | **Flag-dependent** (`view_return_to_manager`) |
| ViewCreateCategoryPicker | ViewPicker | **Inconsistent**: ignores `view_return_to_manager` flag |
| ViewRenameInput | ViewPicker *or* ViewManagerScreen | **Flag-dependent** |
| ViewDeleteConfirm | ViewPicker *or* ViewManagerScreen | **Flag-dependent** |
| ViewEditorCategoryPicker | ViewEditor *or* ViewSectionDetail | **Target-dependent** (section vs view) |
| ViewEditorBucketPicker | ViewEditor *or* ViewSectionDetail | **Target-dependent** |
| ViewManagerCategoryPicker | ViewManagerScreen | OK |
| ViewSectionEditor | ViewEditor *or* ViewManagerScreen | **Flag-dependent** |
| ViewSectionDetail | ViewSectionEditor *or* ViewManagerScreen | **Flag-dependent** |
| ViewSectionTitleInput | ViewSectionDetail | OK |
| ViewUnmatchedSettings | ViewEditor *or* ViewManagerScreen | **Flag-dependent** |
| ViewUnmatchedLabelInput | ViewUnmatchedSettings | OK |
| ConfirmDelete | Normal | OK |
| CategoryManager | Normal | OK |
| CategoryCreateInput | CategoryManager | OK |
| CategoryRenameInput | CategoryManager | OK |
| CategoryReparentPicker | CategoryManager | OK |
| CategoryDeleteConfirm | CategoryManager | OK |
| CategoryConfigEditor | CategoryManager | OK |
| ItemAssignCategoryPicker | Normal *or* ItemEditInput | **Flag-dependent** (`item_assign_return_to_item_edit`) |
| ItemAssignCategoryInput | ItemAssignCategoryPicker | OK |
| InspectUnassignPicker | Normal | OK |

### 3.2 Proposed Esc Behavior

Every mode has a **fixed** parent. No flags.

| Mode | Esc goes to | Rationale |
|------|-------------|-----------|
| Normal | clears current section's filter if set, else no-op | P1 exception: Normal is the root mode with no parent. Esc clears the focused section's filter (see §10). |
| AddInput | Normal | Unchanged |
| ItemEdit | Normal | Unchanged (discards unsaved edits) |
| NoteEdit | Normal | Unchanged |
| FilterInput | Normal (preserves existing section filter) | **Changed**: Esc = cancel input, not clear filter. Enter = apply to current section (§10). Esc from Normal clears current section's filter. |
| ViewPicker | Normal | Unchanged |
| ViewEdit | ViewPicker | **New unified mode** (replaces ViewEditor + ViewManagerScreen) |
| ViewEdit > picker overlay | ViewEdit (closes overlay) | Pickers are overlays *within* ViewEdit, not separate modes |
| ViewEdit > text input | ViewEdit (cancels input) | Title/label inputs are sub-modes of ViewEdit |
| ViewCreateName | ViewPicker | Always. No flag. |
| ViewCreateCategoryPicker | ViewPicker | Always. No flag. |
| ViewRenameInput | ViewPicker | Always. No flag. |
| ViewDeleteConfirm | ViewPicker | Always. No flag. |
| ConfirmDelete | Normal | Unchanged |
| CategoryManager | Normal | Unchanged |
| CategoryCreate | CategoryManager | Unchanged |
| CategoryRename | CategoryManager | Unchanged |
| CategoryReparent | CategoryManager | Unchanged |
| CategoryDelete | CategoryManager | Unchanged |
| CategoryConfig | CategoryManager | Unchanged |
| ItemAssignPicker | Normal | **Changed**: always returns to Normal. See section 5.3. |
| ItemAssignInput | ItemAssignPicker | Unchanged |
| InspectUnassign | Normal | Unchanged |

**Key change**: `view_return_to_manager` and `item_assign_return_to_item_edit` flags are eliminated. Each mode has one parent.

## 4. Unified View Editor

### 4.1 Rationale

Currently there are two editing surfaces for views:
- **View Manager** (Mode::ViewManagerScreen): 3-pane layout. Handles view-level criteria (as rows) and columns. Can open section/unmatched editing but does so by spawning a ViewEditorState and entering ViewSectionEditor/ViewSectionDetail modes — which also belong to the View Editor flow. Saves with `s`.
- **View Editor** (Mode::ViewEditor): overlay. Handles view-level criteria (as include/exclude sets), sections, and unmatched settings. Saves with `Enter`.

These should be one screen.

### 4.2 Proposed Layout

The unified View Editor is a **full-screen mode** (replaces the board, not an overlay). It has four vertically-stacked regions:

```
┌─ Edit View: My Status Board ─────────────────────── matches: 42 ─┐
│                                                                    │
│ CRITERIA ──────────────────────────────────────────────────────── │
│  + Work, Project                                                   │
│  - Done                                                            │
│  When: Overdue, Today, ThisWeek                                    │
│                                                                    │
│ COLUMNS ───────────────────────────────────────────────────────── │
│  1. When          w:16                                             │
│  2. Priority      w:12                                             │
│  3. Status        w:12                                             │
│                                                                    │
│ SECTIONS ──────────────────────────────────────────────────────── │
│  > 1. Open         include: Status/open                            │
│    2. Closed       include: Status/closed                          │
│                                                                    │
│ UNMATCHED ─────────────────────────────────────────────────────── │
│  Visible: yes    Label: "Unassigned"                               │
│                                                                    │
├────────────────────────────────────────────────────────────────────┤
│ Tab:region  S:save  Esc:cancel                                     │
└────────────────────────────────────────────────────────────────────┘
```

### 4.3 Region Focus

- `Tab` / `Shift-Tab` cycles focus between regions: Criteria → Columns → Sections → Unmatched → Criteria
- The focused region has a highlighted border
- Within each region, `j`/`k` navigates items

### 4.3.1 Criteria Region Rendering

Each row displays a `ViewCriteriaRow` with the same format as the current ViewManagerScreen Definition pane:

```
CRITERIA ──────────────────────────────────────────────────────────
   +Work                                    ← first row, no join prefix
 AND +Project                               ← AND join, include
 OR  -Done                                  ← OR join, exclude
   When: Overdue, Today, ThisWeek           ← virtual includes (collapsed into one line)
   When (excl): Future                      ← virtual excludes (collapsed into one line)
```

Category criteria rows (`ViewCriteriaRow`) render as: `{join} {indent}{sign}{category_name}`, where join is blank for row 0, otherwise `AND`/`OR`; sign is `+`/`-`; indent is `"  " * depth`.

Virtual (When bucket) criteria are **not** individual rows — they are a summary line at the bottom of the Criteria region showing `When: {comma-separated buckets}`. The `]`/`[` keys open a `BucketPicker` overlay to add/remove from the virtual include/exclude sets. This matches the current ViewManagerScreen behavior where virtual criteria are displayed but not individually selectable in the criteria row list.

### 4.3.2 Columns Region Rendering

```
COLUMNS ────────────────────────────────────────────────────────────
  1. When          w:16  [When]             ← ColumnKind::When tagged
  2. Priority      w:12
  3. Status        w:12
```

Each row: `{index}. {category_name}  w:{width}  {kind_tag}`, where `kind_tag` is `[When]` for `ColumnKind::When` and blank for `ColumnKind::Standard`. The category name is resolved from `col.heading` via the category name map; deleted categories show `(deleted)`.

### 4.4 Per-Region Keys

**Criteria region:**
| Key | Action |
|-----|--------|
| `j`/`k` | Navigate criteria rows |
| `N` | Add criteria row (opens category picker overlay) |
| `x` | Remove selected criteria row |
| `Space` | Toggle include/exclude on selected row |
| `c` | Change category on selected row (opens picker overlay) |
| `]`/`[` | Add/remove virtual include/exclude (opens bucket picker overlay) |

**Columns region:**
| Key | Action |
|-----|--------|
| `j`/`k` | Navigate columns |
| `N` | Add column (opens category picker overlay) |
| `x` | Remove selected column |
| `[`/`]` | Reorder column up/down |
| `w` | Edit width (inline number input) |
| `Enter` | Change heading category (opens picker overlay) |

**Sections region:**
| Key | Action |
|-----|--------|
| `j`/`k` | Navigate sections |
| `N` | Add new section |
| `x` | Remove selected section |
| `[`/`]` | Reorder section up/down |
| `Enter` | Expand/collapse section detail inline |
| `t` | Edit section title (inline text input) |

When a section is expanded:
| Key | Action |
|-----|--------|
| `+`/`-` | Add/remove include/exclude criteria (opens picker overlay) |
| `a` | Edit on-insert-assign set (opens picker overlay) |
| `r` | Edit on-remove-unassign set (opens picker overlay) |
| `h` | Toggle show_children |

**Unmatched region:**
| Key | Action |
|-----|--------|
| `t` | Toggle show_unmatched |
| `l` | Edit unmatched label (inline text input) |

**Global (any region):**
| Key | Action |
|-----|--------|
| `S` (capital) | Save entire view |
| `Esc` | Cancel (discard all changes, return to ViewPicker) |
| `Tab`/`Shift-Tab` | Cycle region focus |

### 4.5 Picker Overlays

Category and bucket pickers are overlays within ViewEdit, not separate top-level modes. They render as a right-aligned panel (40% width) over the ViewEdit screen. Keys:
- `j`/`k` navigate
- `Space` toggles selection
- `Enter` or `Esc` closes the overlay and returns to the ViewEdit region that opened it

Internally this can be tracked as a sub-state (e.g., `view_edit_overlay: Option<PickerOverlay>`) rather than as separate Mode variants. The picker overlay intercepts keys when active; when dismissed, the underlying region regains focus.

### 4.6 Inline Text Input

When editing a section title or unmatched label, the text becomes editable in-place within the region. The shared field cursor (section 6) handles this. `Enter` confirms, `Esc` cancels. This replaces the separate ViewSectionTitleInput and ViewUnmatchedLabelInput modes.

Internally this can be tracked as `view_edit_inline_input: Option<InlineInputTarget>` alongside the ViewEdit mode.

### 4.7 Save Semantics

One save action: `S` (capital S). Persists the entire view (criteria + columns + sections + unmatched settings) atomically via `store.update_view()`. This replaces the split where the View Manager saved criteria with `s` and the View Editor saved criteria+sections with `Enter`.

After saving, the editor remains open showing the saved state. The user presses `Esc` to return to the ViewPicker.

### 4.8 Entry Points

- From ViewPicker: `e` opens the selected view in ViewEdit
- From ViewPicker: `V` also opens ViewEdit (same as `e`; `V` can be removed or kept as alias)
- From ViewPicker: `N` creates a new view, then opens it in ViewEdit

## 5. Mode Reduction

### 5.1 Proposed Mode Enum

```rust
enum Mode {
    // Board
    Normal,
    AddInput,
    ItemEdit,           // was ItemEditInput
    NoteEdit,           // was NoteEditInput
    FilterInput,
    ConfirmDelete,

    // Item assignment
    ItemAssignPicker,   // was ItemAssignCategoryPicker
    ItemAssignInput,    // was ItemAssignCategoryInput
    InspectUnassign,    // was InspectUnassignPicker

    // View management
    ViewPicker,
    ViewEdit,           // NEW: unified (replaces ViewManagerScreen + ViewEditor
                        //   + ViewSectionEditor + ViewSectionDetail
                        //   + ViewSectionTitleInput + ViewEditorCategoryPicker
                        //   + ViewEditorBucketPicker + ViewManagerCategoryPicker
                        //   + ViewUnmatchedSettings + ViewUnmatchedLabelInput)
    ViewCreateName,     // was ViewCreateNameInput
    ViewCreateCategory, // was ViewCreateCategoryPicker
    ViewRename,         // was ViewRenameInput
    ViewDeleteConfirm,

    // Category management
    CategoryManager,
    CategoryCreate,     // was CategoryCreateInput
    CategoryRename,     // was CategoryRenameInput
    CategoryReparent,   // was CategoryReparentPicker
    CategoryDelete,     // was CategoryDeleteConfirm
    CategoryConfig,     // was CategoryConfigEditor
}
```

**30 modes → 21 modes.** The 10 eliminated view-editing modes (ViewManagerScreen, ViewEditor, ViewSectionEditor, ViewSectionDetail, ViewSectionTitleInput, ViewEditorCategoryPicker, ViewEditorBucketPicker, ViewManagerCategoryPicker, ViewUnmatchedSettings, ViewUnmatchedLabelInput) become sub-states of the single new ViewEdit mode.

### 5.2 ViewEdit Sub-State

The following types already exist in the codebase and are reused by ViewEdit:

```rust
// Existing — lib.rs. Used by ViewManagerScreen today; reused by ViewEdit's Criteria region.
enum ViewCriteriaSign { Include, Exclude }

struct ViewCriteriaRow {
    sign: ViewCriteriaSign,
    category_id: CategoryId,
    join_is_or: bool,    // AND vs OR between rows
    depth: usize,        // indent level for child categories
}

// Existing — lib.rs. Identifies which HashSet<CategoryId> a picker overlay is editing.
enum CategoryEditTarget {
    ViewInclude,
    ViewExclude,
    SectionCriteriaInclude,
    SectionCriteriaExclude,
    SectionOnInsertAssign,
    SectionOnRemoveUnassign,
}

// Existing — lib.rs. Identifies which HashSet<WhenBucket> a picker overlay is editing.
enum BucketEditTarget {
    ViewVirtualInclude,
    ViewVirtualExclude,
    SectionVirtualInclude,
    SectionVirtualExclude,
}
```

New types introduced for ViewEdit:

```rust
enum ViewEditRegion {
    Criteria,
    Columns,
    Sections,
    Unmatched,
}

enum ViewEditOverlay {
    CategoryPicker { target: CategoryEditTarget },
    BucketPicker { target: BucketEditTarget },
}

enum ViewEditInlineInput {
    SectionTitle { section_index: usize },
    UnmatchedLabel,
    ColumnWidth { column_index: usize },
}

struct ViewEditState {
    draft: View,
    region: ViewEditRegion,
    criteria_index: usize,
    column_index: usize,
    section_index: usize,
    section_expanded: Option<usize>,   // which section is showing detail
    overlay: Option<ViewEditOverlay>,
    inline_input: Option<ViewEditInlineInput>,
    picker_index: usize,               // cursor within overlay picker
    preview_count: usize,
    criteria_rows: Vec<ViewCriteriaRow>,
}
```

**Key dispatch precedence in ViewEdit** (innermost layer wins):
1. If `inline_input.is_some()`, handle text input keys (Enter confirms, Esc cancels inline input)
2. Else if `overlay.is_some()`, handle picker keys (j/k/Space/Enter/Esc dismisses overlay)
3. Else handle region-level keys (j/k/N/x/Tab/S/Esc returns to ViewPicker)

This precedence is normative. All key dispatch in ViewEdit must follow this order.

### 5.3 Item Edit and Category Assignment

Currently `ItemAssignCategoryPicker` uses `item_assign_return_to_item_edit` to decide whether Esc goes to Normal or ItemEditInput. This creates the inconsistency where the same mode behaves differently depending on how you entered it.

**Normative decision**: ItemAssignPicker always returns to Normal on both Esc and Enter. The `item_assign_return_to_item_edit` flag is deleted.

Workflow when the user wants to edit categories from within the ItemEdit popup:
1. User presses Enter on the "Categories" button → enters ItemAssignPicker (mode changes to ItemAssignPicker)
2. User toggles categories with Space, presses Enter or Esc → returns to Normal
3. User presses `e` to re-open ItemEdit → the same item is still selected, so the popup re-opens with current state

This means the user takes one extra keystroke (`e`) to get back into the ItemEdit popup after using the category picker. This is an acceptable trade-off for deterministic Esc behavior. The item selection is preserved across the Normal→ItemAssignPicker→Normal→ItemEdit round-trip, so no work is lost.

## 6. Shared Field Cursor Model

### 6.1 Current Problem

There are three independent cursor implementations:
- `input` + `input_cursor` (single-line, used by AddInput/FilterInput/various name inputs)
- `item_edit_note` + `item_edit_note_cursor` (multi-line, used by ItemEdit note field)
- `category_config_editor.note` + `category_config_editor.note_cursor` (multi-line, used by CategoryConfig)

Each has its own set of methods: `move_*_cursor_left`, `move_*_cursor_right`, `backspace_*_char`, etc. These are functionally identical except for the field they operate on. The `input/mod.rs` file is 487 lines, most of it duplicated logic.

### 6.2 Proposed Abstraction

```rust
/// A text buffer with a cursor position, supporting single-line and multi-line editing.
struct TextBuffer {
    text: String,
    cursor: usize,  // char offset (not byte offset)
}

impl TextBuffer {
    fn new(text: String) -> Self { /* cursor at end */ }
    fn empty() -> Self { /* empty, cursor at 0 */ }

    // Cursor movement
    fn move_left(&mut self);
    fn move_right(&mut self);
    fn move_home(&mut self);
    fn move_end(&mut self);
    fn move_up(&mut self);     // multi-line: moves to same column on previous line
    fn move_down(&mut self);   // multi-line: moves to same column on next line

    // Editing
    fn insert_char(&mut self, c: char);
    fn insert_newline(&mut self);
    fn backspace(&mut self);
    fn delete(&mut self);

    // Access
    fn text(&self) -> &str;
    fn cursor(&self) -> usize;
    fn is_empty(&self) -> bool;
    fn trimmed(&self) -> &str;

    // Key dispatch helper
    fn handle_key(&mut self, code: KeyCode, multiline: bool) -> bool;
}
```

The `handle_key` method combines the current `handle_text_input_key` / `handle_item_edit_note_input_key` / `handle_category_config_note_input_key` into one. When `multiline` is true, Up/Down move between lines and Enter inserts a newline. When false, Up/Down are not consumed and Enter is not consumed.

### 6.3 Usage

Replace the ad-hoc fields:

```rust
struct App {
    // Instead of: input: String, input_cursor: usize
    input: TextBuffer,

    // Instead of: item_edit_note: String, item_edit_note_cursor: usize
    item_edit_note: TextBuffer,

    // CategoryConfigEditorState.note + note_cursor → TextBuffer
}
```

The `handle_key` dispatch becomes:

```rust
// In AddInput handler:
KeyCode::Esc => { /* cancel */ }
KeyCode::Enter => { /* submit */ }
_ => { self.input.handle_key(code, false); }

// In ItemEdit handler, when focus is Note:
KeyCode::Esc => { /* cancel */ }
KeyCode::Tab => { /* cycle focus */ }
_ => { self.item_edit_note.handle_key(code, true); }
```

### 6.4 Cursor Position for Rendering

`TextBuffer` should also provide:

```rust
fn line_col(&self) -> (usize, usize);  // for cursor positioning in multi-line renders
fn clamped_cursor(&self) -> usize;     // cursor clamped to text length
```

This eliminates the standalone `note_cursor_line_col` and `note_line_start_chars` helper functions, folding them into the buffer.

## 7. Key Vocabulary

Consistent meanings across all contexts:

| Key | Meaning |
|-----|---------|
| `j`/`Down` | Move cursor down in a list |
| `k`/`Up` | Move cursor up in a list |
| `h`/`Left` | Move cursor left (or move to previous pane/slot) |
| `l`/`Right` | Move cursor right (or move to next pane/slot) |
| `Tab` | Cycle focus forward (fields in a form, regions in ViewEdit, slots in board) |
| `Shift-Tab` | Cycle focus backward |
| `Enter` | Confirm/activate/submit (open item for editing, submit input, expand section) |
| `Esc` | Go back / cancel / dismiss |
| `Space` | Toggle (checkbox, include/exclude, assign/unassign in picker) |
| `n` | New item (lowercase, from board) |
| `N` | New entity in current context (uppercase: new view, section, column, criteria row, category) |
| `x` | Delete selected entity (with confirmation for destructive actions) |
| `r` | Rename |
| `[`/`]` | Reorder up/down (sections, columns) *or* move item between slots (in board) |
| `+`/`-` | Add/remove include/exclude criteria (in view/section editing) |
| `S` | Save (in ViewEdit) |
| `s` | Not used in ViewEdit (avoids collision with lowercase action keys) |
| `q` | Quit (from Normal mode only) |
| `/` | Open filter for current section (§10) |
| `v` | Open view picker |
| `c` | Open category manager |
| `p` | Toggle preview |
| `d` | Toggle done |
| `a` | Open category assignment picker for selected item |

### 7.1 Conflict Resolutions

- Current ViewManagerScreen uses `s` to save criteria only (not columns/sections). ViewEdit uses `S` (capital) to save everything. This avoids conflict with `s` being used for "sections" in the View Editor.
- Current ViewEditor uses `s` to open section editor. In ViewEdit, sections are a visible region — no key needed to "open" them, just Tab to the region.
- `[`/`]` is overloaded: reorder in editing contexts, move item in board context. This is OK because they never coexist (you're either in ViewEdit or on the board).
- `h` is overloaded: cursor left globally (§7), but toggles `show_children` in ViewEdit's expanded section detail (§4.4). This is OK because ViewEdit has no left/right cursor movement — navigation is vertical within regions and Tab between them.

## 8. Status Bar Messages

The status bar should follow a consistent format:

- **Mode entry**: brief instruction. E.g., `"Add item: type text, Enter to save, Esc to cancel"`
- **Action result**: what happened. E.g., `"Item added (parsed when: 2026-02-24 15:00)"`, `"Saved view: My Board (42 matches)"`
- **Error**: prefixed with `"Error: "`. E.g., `"Error: Cannot save — text is empty"`
- **Warning**: prefixed with `"Warning: "`. E.g., `"Warning: unknown_hashtags=office,someday"`

Keep messages under 80 characters where possible. Do not embed keyboard hints in every status message — the footer hint bar is for that.

## 9. Footer Hint Bar

Currently hints are embedded in status messages, which means they disappear when the next action updates the status. Instead, render a persistent hint bar in the footer area, separate from the status message.

The hint bar content changes based on mode + focused region:

```
Normal:    n:add  e:edit  d:done  x:delete  v:views  c:categories  /:filter  q:quit
ViewPicker: Enter:switch  e:edit  N:new  r:rename  x:delete  Esc:back
ViewEdit:  Tab:region  S:save  Esc:cancel  (+ region-specific hints)
```

The status line sits above the hint bar and shows transient messages (action results, errors).

**Layout**:
```
┌─ header ───────────────────────────────────────────┐
│ main area                                           │
├─────────────────────────────────────────────────────┤
│ Status: Item added                                  │  ← transient message
│ n:add  e:edit  d:done  v:views  /:filter  q:quit   │  ← persistent hints
└─────────────────────────────────────────────────────┘
```

## 10. Per-Section Text Filters

### 10.1 Motivation

The current TUI has a single view-wide text filter (`/`). Since sections already partition items by criteria, the natural next step is scoping the text filter to the section the cursor is in. This lets users search within "Open" without hiding items in "Closed", or filter two sections differently at the same time.

### 10.2 State Representation

Replace the single `filter: Option<String>` with per-section state:

```rust
struct App {
    // Replaces: filter: Option<String>
    section_filters: Vec<Option<String>>,  // indexed by rendered section position
    filter_target_section: usize,          // which section FilterInput is editing
}
```

`section_filters` is rebuilt (reset to all-`None`) whenever the board's section structure changes. Rebuild triggers:
- View switch (different view selected from ViewPicker)
- Returning from ViewEdit after saving (sections may have been added, removed, or reordered)
- Store reload / external data change

Its length matches the number of rendered sections (including the unmatched section when visible). Filters are **not** rebuilt when merely entering/exiting ViewEdit without saving — the board layout hasn't changed.

### 10.3 Key Behavior

| Context | Key | Action |
|---------|-----|--------|
| Normal | `/` | Open FilterInput for the **current section** (the section containing the cursor) |
| Normal | `Esc` | Clear the **current section's** filter. If already clear, no-op. |
| FilterInput | `Enter` | Apply filter text to `section_filters[filter_target_section]`. Empty input clears. Return to Normal. |
| FilterInput | `Esc` | Cancel input, preserve existing filter for that section. Return to Normal. |

There is no view-wide "clear all filters" key. The user clears filters one section at a time via `Esc` while the cursor is in that section. This is consistent with P1 (Esc means "undo one thing") and avoids needing a multi-action key.

### 10.4 FilterInput Mode Change

FilterInput gains awareness of its target section:

```rust
// On entering FilterInput:
self.filter_target_section = self.current_section_index();
self.set_input(self.section_filters[self.filter_target_section].clone().unwrap_or_default());
```

The mode itself is unchanged — it's still a single text input with Enter/Esc. The only difference is where the result is stored.

### 10.5 Rendering

Each section header shows its active filter when set:

```
▸ Open (3)  filter:bug                    ← section has active filter
  Priority   Type     Item         Status
  ─────────────────────────────────────────
  high       bug      Fix login timeout   open
─────────────────────────────────────────────
▾ Closed (5)                               ← no filter, shows all items
```

The header format is: `▸ Title (count)  filter:<needle>`. The filter tag is dimmed or styled distinctly so it doesn't look like part of the title. The count reflects the post-filter item count.

### 10.6 Item Filtering Logic

Filtering is applied after section criteria evaluation and before rendering:

```
view criteria → view_items
  section criteria → section_items
    text filter → displayed_items (per section)
```

The existing `filter` text match logic (case-insensitive substring on item text) is reused unchanged; it just runs per-section instead of once for the whole view.

### 10.7 Esc Transition Map Update

The Esc behavior for Normal mode (section 3.2) is refined:

| Mode | Esc goes to | Rationale |
|------|-------------|-----------|
| Normal | Clears **current section's** filter if set, else no-op | P1 exception refined: scoped to focused section |

### 10.8 Interaction with Section Navigation

When `Tab`/`Shift-Tab` moves the cursor to a different section, the user is now in the context of that section's filter. Pressing `/` edits that section's filter; pressing `Esc` clears that section's filter. No mode change is needed — the section context is implicit from cursor position.

## 11. Implementation Sequence

### Phase 1: TextBuffer extraction — DONE (2026-02-19)
1. ~~Create `text_buffer.rs` with `TextBuffer` struct~~
2. ~~Replace `input` + `input_cursor` with `TextBuffer`~~
3. ~~Replace `item_edit_note` + `item_edit_note_cursor` with `TextBuffer`~~
4. ~~Replace `CategoryConfigEditorState.note` + `note_cursor` with `TextBuffer`~~
5. ~~Delete duplicated cursor methods from `input/mod.rs`~~
6. ~~**New tests**: TextBuffer unit tests (22+ tests covering single/multi-line ops, edge cases)~~
7. ~~All existing tests pass~~

### Phase 2: Unified ViewEdit mode

Split into sub-phases to keep each commit testable. The old modes remain functional until Phase 2c deletes them, so existing tests pass throughout.

**Phase 2a: Build ViewEdit alongside old modes — DONE (2026-02-19)**
1. ~~Create `ViewEditState` struct with region/overlay/inline_input sub-states~~
2. ~~Add `Mode::ViewEdit` variant to the Mode enum (old variants remain)~~
3. ~~Implement ViewEdit key dispatch (inline_input → overlay → region precedence, per section 5.2)~~
4. ~~Implement ViewEdit rendering (three regions: Criteria/Sections/Unmatched)~~
5. ~~Implement picker overlay rendering (right-aligned panel)~~
6. ~~Implement inline text input within regions~~
7. ~~Wire up save (`Enter`) to persist full view via `store.update_view()`~~
8. ~~Wire entry point: ViewPicker `e` enters ViewEdit instead of ViewManagerScreen/ViewEditor~~
9. Tests not yet added

**Implementation notes (deviations from spec):**
- `ViewEditRegion` has 3 variants (Criteria/Sections/Unmatched) instead of 4 — missing `Columns` region
- `ViewEditInlineInput` missing `ColumnWidth` variant — columns are edited via section-level category picker overlay (`c` key) instead of a dedicated Columns region
- Save uses `Enter` key, not `S` (capital) as spec proposed
- `ViewCriteriaRow` does not include `join_is_or` or `depth` fields (simplified)

**Phase 2b: Migrate remaining entry points — DONE (2026-02-19)**
1. ~~ViewPicker `N` (new view) opens ViewEdit after creation~~
2. ~~All ViewCreate*/ViewRename*/ViewDeleteConfirm modes return to ViewPicker unconditionally~~
3. ~~`view_return_to_manager` and `item_assign_return_to_item_edit` flags deleted~~
4. Tests not yet adapted

**Phase 2c: Delete old modes — DONE (2026-02-19)**
1. ~~Delete old modes: ViewManagerScreen, ViewEditor, ViewSectionEditor, ViewSectionDetail, ViewSectionTitleInput, ViewEditorCategoryPicker, ViewEditorBucketPicker, ViewManagerCategoryPicker, ViewUnmatchedSettings, ViewUnmatchedLabelInput~~
2. ~~Delete `view_editor_return_to_manager`, `view_editor_category_target`, `view_editor_bucket_target` fields~~
3. ~~Delete `ViewEditorState` struct~~
4. ~~Delete rendering code for the old modes~~
5. ~~All tests pass~~

**Phase 2d: Mode enum rename — DONE (2026-02-20)**
Renamed all 13 Mode variants to match spec §5.1:
- ~~`ItemEditInput` → `ItemEdit`~~
- ~~`NoteEditInput` → `NoteEdit`~~
- ~~`ItemAssignCategoryPicker` → `ItemAssignPicker`~~
- ~~`ItemAssignCategoryInput` → `ItemAssignInput`~~
- ~~`InspectUnassignPicker` → `InspectUnassign`~~
- ~~`ViewCreateNameInput` → `ViewCreateName`~~
- ~~`ViewCreateCategoryPicker` → `ViewCreateCategory`~~
- ~~`ViewRenameInput` → `ViewRename`~~
- ~~`CategoryCreateInput` → `CategoryCreate`~~
- ~~`CategoryRenameInput` → `CategoryRename`~~
- ~~`CategoryReparentPicker` → `CategoryReparent`~~
- ~~`CategoryDeleteConfirm` → `CategoryDelete`~~
- ~~`CategoryConfigEditor` → `CategoryConfig`~~

### Phase 3: Per-section text filters
1. Replace `filter: Option<String>` with `section_filters: Vec<Option<String>>` and `filter_target_section: usize`
2. Rebuild `section_filters` on view switch, returning from ViewEdit after save, store reload (length = rendered section count; see §10.2)
3. `/` in Normal sets `filter_target_section` to the current section before entering FilterInput
4. FilterInput Enter stores result in `section_filters[filter_target_section]`
5. Esc in Normal clears current section's filter (not all filters)
6. Render per-section filter indicator in section headers
7. Apply text filter per-section after section criteria evaluation, before rendering
8. **New tests**: per-section filter isolation (filter in section A doesn't affect section B), filter rebuild on view switch clears all filters, Esc-from-Normal clears only the focused section's filter, `/` targets the correct section

### Phase 4: Esc consistency (remaining fixes after Phases 2–3)
Phase 2 already deletes the flag-based Esc routing (`view_return_to_manager`, `view_editor_return_to_manager`, `item_assign_return_to_item_edit`). Phase 3 refines FilterInput to be section-scoped. This phase fixes the remaining Esc inconsistencies:

1. Fix FilterInput: Esc cancels the input and returns to Normal, preserving the existing filter for that section. Currently Esc clears the filter; this changes so that only Esc *from Normal mode* clears the current section's filter.
2. Fix ViewCreateCategoryPicker: Esc returns to ViewPicker (fixed parent, unconditional).

**Phase 5d: Name inputs → InputPanel — DONE (2026-02-21)**
- ~~Removed ViewCreateName, ViewRename, CategoryCreate, CategoryRename from Mode enum (21 → 17 modes)~~
- ~~Entry points now open InputPanel(NameInput) with NameInputContext discriminant~~
- ~~save_input_panel_name dispatches on context; cancel returns to correct parent mode~~

**Phase 5e: Change save key from Enter → S — DONE (2026-02-21)**
- ~~In InputPanel: Char('S') saves from any focus~~
- ~~In ViewEdit: Enter save → Char('S'), footer hints updated~~

**Footer hint bar — DONE (2026-02-21)**
- ~~Two-row footer: status (transient) + hint bar (persistent, per-mode, darkgray)~~
- ~~Footer height 3→4 rows; render_footer split into footer_status_text + footer_hint_text~~
- ~~All modes have explicit curated hints; Normal hints match spec §9~~
- ~~Closes FR `afe45b4e`~~

**Phase 3: Per-section text filters — DONE (2026-02-21)**
- ~~Replace filter: Option<String> with section_filters: Vec<Option<String>> + filter_target_section: usize~~
- ~~Reset on view switch, ViewEdit save; resize if slot count changes~~
- ~~/` scopes to focused section; Esc in Normal clears focused section only~~
- ~~FilterInput Esc cancels without clearing~~
- ~~Section header shows filter:needle; header shows filters:N count~~
- ~~4 new tests (isolation, esc-clears, esc-cancels, view-switch-reset)~~
- ~~Closes FR `882a75b0`~~

## 12. Current Implementation State

This section records where the shipped code **differs from the spec above**. It is the authoritative reference for "what does the code actually do today." When spec and code disagree, the code wins until a tracking issue is resolved.

Last updated: 2026-02-20. Tracking issues are in `aglet-features.ag`.

### 12.1 ViewEdit Save Key (§4.7)

| Spec says | Code does | Tracking |
|-----------|-----------|----------|
| `S` (capital S) saves the view | `S` saves the view | FR `94f2f053` — **RESOLVED** (Phase 5e) |

Phase 5e (2026-02-21) changed ViewEdit save from `Enter` to `S`. InputPanel also accepts `S` from any focus as a shortcut save.

### 12.2 ViewEdit Regions (§4, §5.2)

| Spec says | Code does | Tracking |
|-----------|-----------|----------|
| 4 regions: Criteria / Columns / Sections / Unmatched | 3 regions: Criteria / Sections / Unmatched | FR `cf6b7dd8` |

The Columns region was not implemented in Phase 2a. Columns are currently edited via a category picker overlay triggered by the `c` key from within the Sections region. `ViewEditRegion` has only 3 variants; `Tab` cycles among them.

### 12.3 ViewEditInlineInput Variants (§5.2)

| Spec says | Code does | Tracking |
|-----------|-----------|----------|
| `ColumnWidth { column_index }` variant | Variant does not exist | FR `cf6b7dd8` |

Follows from 12.2 — no Columns region means no column-width inline edit.

### 12.4 ViewCriteriaRow Fields (§5.2)

| Spec says | Code does | Notes |
|-----------|-----------|-------|
| Fields: `sign`, `category_id`, `join_is_or`, `depth` | Fields: `sign`, `category_id` only | Simplified — boolean composition not yet supported |

`join_is_or` and `depth` were omitted because the underlying query model does not yet support arbitrary boolean composition. The `ViewCriteriaRow` struct in `lib.rs` has only `sign: ViewCriteriaSign` and `category_id: CategoryId`.

### 12.5 Summary Table

| Area | Spec target | Current code | Gap FR |
|------|-------------|--------------|--------|
| ViewEdit save key | `S` | `S` ✓ | `94f2f053` resolved |
| ViewEdit regions | 4 (Criteria/Columns/Sections/Unmatched) | 3 (no Columns) | `cf6b7dd8` |
| ViewEditInlineInput | ColumnWidth variant | Missing | `cf6b7dd8` |
| ViewCriteriaRow | join_is_or + depth | Not present | — (blocked by query model) |
| Item add flow | InputPanel (Phase 5) | InputPanel(AddItem) ✓ | `cfb526a4` resolved |
| Item edit flow | InputPanel (Phase 5) | InputPanel(EditItem) ✓ | `0ce92977` resolved |
| Footer hint bar | Persistent per-mode hint bar | 2-row footer ✓ | `afe45b4e` resolved |
| Per-section filters | Vec<Option<String>> | section_filters ✓ | `882a75b0` resolved |

## 13. What This Proposal Does NOT Change

- Data model: `View`, `Section`, `Column`, `Query`, `Category`, `Item` are unchanged
- Persistence: `Store` API is unchanged
- Board rendering: slot/item layout, column rendering, preview panel — all unchanged
- Category Manager: same screen, same keys, same flow
- CLI: untouched

## 14. Migration Notes for Existing Users

Key binding changes that existing users will notice. **Note**: items marked `(spec)` describe the design target; items marked `(current)` describe what the code does today.

| Old | New | Context |
|-----|-----|---------|
| `v` then `V` to enter View Manager | `v` then `e` to enter ViewEdit | View Manager is gone |
| `s` in View Manager to save criteria | `S` in ViewEdit to save (Phase 5e) | |
| `t` in View Manager Definition pane to toggle Criteria/Columns | `Tab` in ViewEdit cycles Criteria → Sections → Unmatched (current); will include Columns when FR `cf6b7dd8` ships | No Columns region yet |
| `e` in ViewPicker to open View Editor overlay | `e` in ViewPicker to open ViewEdit (full-screen) | Same key, different presentation |
| `Enter` in View Editor to save | `S` in ViewEdit to save (Phase 5e) | |

### 14.1 Current Split Picker + Editor Workflow (Implementation Note)

Current implementation (branch-level behavior) keeps a split model:

- `v` opens a lightweight **View Picker** for quick switching and simple CRUD
- `e` from the picker opens a full-screen **View Editor** for deep editing
- creating a new view from the picker opens the editor directly, auto-creates the first section, and starts section-title inline edit

See current reference notes:

- `/Users/mds/src/aglet/docs/view-editor-keybindings-current.md`
- `/Users/mds/src/aglet/docs/view-editor-migration-notes.md`
