# Plan: Streamline TUI View UX (Quick Picker + Full-Screen View Editor, Auto First Section)

Date: 2026-02-24
Scope: TUI view creation and view/section management workflow (`v` / `F8`), with focus on section creation/editing ergonomics and consistency with the category manager tree editor rewrite
Primary code areas:
- `/Users/mds/src/aglet/crates/agenda-tui/src/modes/board.rs`
- `/Users/mds/src/aglet/crates/agenda-tui/src/modes/view_edit.rs`
- `/Users/mds/src/aglet/crates/agenda-tui/src/modes/view_edit2.rs`
- `/Users/mds/src/aglet/crates/agenda-tui/src/render/mod.rs`
- `/Users/mds/src/aglet/crates/agenda-tui/src/lib.rs`
- `/Users/mds/src/aglet/crates/agenda-core/src/model.rs`
- `/Users/mds/src/aglet/crates/agenda-core/src/query.rs`
Reference inspiration:
- `/Users/mds/src/aglet/reference/lotus-agenda-view-creation-workflow.md`

## 1. Objective

Make creating and managing views/sections materially faster and more intuitive while preserving a fast view-switch flow.

Primary UX goals:

- Keep `v` lightweight for quick swapping between views
- Provide a separate full-screen editor for deep view and section editing
- Auto-create the first section when creating a view in the TUI flow
- Make section editing field-oriented and discoverable (details pane), not command-memorization-heavy
- Align keyboard vocabulary with the category manager tree editor (`Tab`, `j/k`, `Enter`, `Esc`, `S`)

Target outcome:

- Quick view switching remains simple and low-cognitive-load.
- Deep editing gets a category-manager-style full-screen editor.
- Creating a new view lands the user in a ready-to-name first section.

## 2. Current Baseline (What Exists Today)

Current shipped TUI flow spans multiple modes:

1. `v` opens `Mode::ViewPicker` (`/Users/mds/src/aglet/crates/agenda-tui/src/modes/board.rs`)
2. `N` opens `InputPanel(NameInput)` for the view name (`/Users/mds/src/aglet/crates/agenda-tui/src/modes/view_edit.rs`)
3. Saving name enters `Mode::ViewCreateCategory` (legacy include/exclude picker) (`/Users/mds/src/aglet/crates/agenda-tui/src/modes/board.rs`)
4. `Enter` creates the view and opens `Mode::ViewEdit` (`/Users/mds/src/aglet/crates/agenda-tui/src/modes/view_edit.rs`)
5. `ViewEdit` has 3 regions: `Criteria`, `Sections`, `Unmatched` (`/Users/mds/src/aglet/crates/agenda-tui/src/lib.rs`)

Important current behaviors:

- `View::new()` defaults to:
  - `sections = []`
  - `show_unmatched = true`
  - `unmatched_label = "Unassigned"`
  (`/Users/mds/src/aglet/crates/agenda-core/src/model.rs`)
- Zero-section views are valid in core. `resolve_view()` returns no explicit sections and (if enabled) an unmatched group (`/Users/mds/src/aglet/crates/agenda-core/src/query.rs`)
- Empty unmatched lanes are hidden in TUI (`should_render_unmatched_lane`) (`/Users/mds/src/aglet/crates/agenda-tui/src/ui_support.rs`)

## 3. UX Problems (Observed / Confirmed In Current Code)

### 3.1 Create Flow Is Split Across Too Many Surfaces

- `ViewPicker` -> `NameInput` -> `ViewCreateCategory` -> `ViewEdit`
- The user must understand two separate criteria surfaces (`ViewCreateCategory` and `ViewEdit.Criteria`)

### 3.2 First Section Creation Is Manual And Indirect

- New view starts with no sections
- User must navigate to sections, add a section, then separately rename it
- This is a poor default for a board/section mental model

### 3.3 Section Editing Is Command-Dense, Not Field-Oriented

Current `ViewEdit.Sections` region expects memorization of action keys (`f/c/a/r/h/m`) and uses an expanded summary row rather than a proper details-pane editing workflow.

### 3.4 `v` Picker Is Good For Switching, But Not A Good Place For Heavy Editing

This is not a bug. It is a product mismatch if we overload the quick picker with a full management UI.

We want to preserve the simple picker and move complexity into a dedicated editor.

### 3.5 Create-Time Criteria Picker Has Surprising Default Behavior

In `ViewCreateCategory`, pressing `Enter` with no explicit `+`/`-` selections creates the view using the currently highlighted category as an include criterion (`/Users/mds/src/aglet/crates/agenda-tui/src/modes/view_edit.rs`).

This is efficient for experts but surprising and easy to trigger accidentally.

### 3.6 `Esc` In `ViewEdit` Drops The Draft Immediately

- `Esc` exits `ViewEdit` and discards unsaved changes
- No dirty-state prompt exists

## 4. Product Decisions (Locked For This Plan)

### 4.1 Split Architecture: Keep Quick Picker + Add Separate Full-Screen Editor

Use a split UX, inspired by Lotus Agenda’s progressive disclosure:

- **Quick View Picker** (`v` / `F8`) for fast switching and simple CRUD
- **Full-Screen View Editor** (opened from picker, e.g. `e`) for deep editing

This preserves the current strength of `ViewPicker` while giving us a better editing experience.

### 4.2 Auto-Create First Section In TUI View Create Flow

When a view is created through the TUI flow, automatically create the first section in the draft and immediately begin title editing for it.

Recommended default section:

- `title = "Main"` (preferred) or `"New section"` (lower-risk compatibility option)
- empty `criteria`
- empty `columns`
- empty `on_insert_assign` / `on_remove_unassign`
- `show_children = false`
- `board_display_mode_override = None`

### 4.3 Preserve Core Flexibility (Zero Sections Still Allowed)

Do **not** make zero-section views invalid in core/storage.

Rationale:

- Core/query/store already support zero-section views cleanly
- CLI/import/tests may intentionally use “saved search” views where unmatched acts as the only lane
- This is a TUI default/workflow improvement, not a model restriction

## 5. UX Direction (Lotus-Inspired Progressive Disclosure + Category-Manager Consistency)

### 5.1 What To Borrow From Lotus Agenda

From `/Users/mds/src/aglet/reference/lotus-agenda-view-creation-workflow.md`, the most useful patterns are:

- lightweight view manager for quick switching
- separate deeper properties/editing path
- fast section insertion relative to current section (above/below)
- category matching first, chooser/picker fallback

We are not copying Lotus literally (dialogs, function keys, exact field layout), but we should preserve these interaction principles.

### 5.2 Aglet End-State Mental Model

#### Screen A: Quick View Picker (`v`)

Purpose:

- switch views quickly
- create/rename/delete views
- open full-screen editor for selected view

Keep it visually simple.

#### Screen B: Full-Screen View Editor (`e` from picker)

Purpose:

- edit the selected view’s properties and sections on one surface
- details-pane editing for section/view fields
- explicit draft save/cancel

This editor should feel like the category manager’s full-screen tree editor, but scoped to one selected view.

## 6. Proposed UX Contract (Target)

## 6.1 Quick View Picker (`v` / `F8`)

Keep the quick picker intentionally lightweight.

Recommended keys:

- `j/k`: select view
- `Enter`: switch to selected view and close picker
- `N`: create new view (name flow)
- `r`: rename selected view
- `x`: delete selected view
- `e` (and optionally `V` alias): open full-screen View Editor for selected view
- `Esc`: close picker

Notes:

- Do not add a permanent details pane here
- Do not add section tree here
- Optional lightweight type-to-filter is OK if kept visually minimal

## 6.2 Full-Screen View Editor (Deep Path)

Default layout (recommended):

- left pane: `Sections` list (plus a synthetic `View Properties` row)
- right pane: `Details` pane
- optional `Preview` pane toggled with `p`
- lightweight filter bar toggled/focused with `/` (not always visible as a pane)

### 6.2.1 Pane / Focus Navigation

- `Tab` / `Shift+Tab`: cycle visible panes (`Sections` -> `Details` -> `Preview` if open)
- `j/k` or arrows: move selection / field focus within active pane
- `Enter`:
  - from `Sections`: focus `Details`
  - in `Details`: activate/toggle/edit field
  - in inline text action: confirm
- `Esc`:
  - cancel inline action
  - close overlay
  - clear active filter
  - prompt discard if dirty and exiting editor

### 6.2.2 Sections Pane Actions (Lotus-Inspired Quick Insertion)

When a section row is selected:

- `n`: add section **below** current row, immediately start title edit
- `N`: add section **above** current row, immediately start title edit
- `r`: rename selected section (inline)
- `x`: delete selected section (inline confirm)
- `J/K`: reorder section down/up

When `View Properties` row is selected:

- `n` / `N`: add section to current view (position policy defined by current selection)
- `r`: rename current view (inline)
- `x`: delete current view (optional in editor; can also remain picker-only)

### 6.2.3 Details Pane Fields (Section)

For a selected section:

- Title
- Criteria
- Columns
- On insert assign
- On remove unassign
- Show children
- Display mode override

### 6.2.4 Details Pane Fields (View)

For the synthetic `View Properties` row:

- Name
- Criteria
- Virtual include/exclude “When” buckets
- Board display mode
- Unmatched visible
- Unmatched label
- Remove-from-view unassign set (optional phase)

### 6.2.5 Overlays / Pickers

Overlays should reuse category manager/picker vocabulary:

- `j/k`: move
- `Space`: toggle
- `Enter`: confirm/close (or toggle + close, depending on field)
- `Esc`: close
- `/` or type-to-filter: preferred for large category hierarchies

## 7. Recommended Delivery Strategy (Phased)

Do not attempt the full editor rewrite first. Ship immediate UX improvements on the current architecture, then build the editor.

### Phase 1: Immediate UX Wins In Current `ViewEdit` (Low Risk)

Goal: reduce friction now without changing the split architecture.

Changes:

1. Auto-create first section when creating a view through TUI
2. After create, open `ViewEdit` focused on that first section title inline edit
3. In `ViewEdit.Sections`, make `N` auto-start title edit for the new section
4. Add dirty tracking + discard confirmation for `ViewEdit`
5. Update footer/status hints to match the new common path

Acceptance criteria:

- Creating a view yields a ready-to-name first section with no extra navigation
- Adding a section is `N` -> type -> `Enter`
- Unsaved view edits are not silently lost on accidental `Esc`

Notes:

- This phase can keep `ViewCreateCategory` temporarily
- No core schema changes required

### Phase 2: Remove `ViewCreateCategory` From TUI Create Path

Goal: make TUI create flow cohesive and less surprising.

Changes:

1. `ViewPicker` create flow becomes:
   - name view
   - create draft/default view
   - auto-create first section
   - open editor immediately
2. Move create-time criteria setup into the editor’s view properties/details fields
3. Remove create-time hidden default criterion behavior from TUI create path

Acceptance criteria:

- New view creation no longer requires a separate legacy category picker step
- Criteria are edited in exactly one place (the editor)

### Phase 3: Build Full-Screen View Editor (Main Refactor)

Goal: keep the quick picker and replace current `ViewEdit` UX with a calmer, details-pane-based full-screen editor for one selected view.

Changes:

1. Add a dedicated full-screen editor mode/state (or refactor `Mode::ViewEdit` into this role)
2. Scope the editor to a single selected view draft
3. Render default 2-pane layout (`Sections` + `Details`)
4. Add optional preview toggle (`p`)
5. Add compact filter bar (`/`) for section list
6. Add Lotus-inspired quick section insertion (`n` below / `N` above)
7. Keep explicit `S` save and dirty-discard prompt on `Esc`

Acceptance criteria:

- `v` picker remains simple
- `e` opens a powerful full-screen editor
- Section editing is field-based and discoverable
- Preview/filter are available but not cluttering the default screen

### Phase 4: Picker / Matching / Overlay Polish

Goal: improve speed and reduce picker fatigue, especially in large category hierarchies.

Changes:

1. Add filtering/type-to-match in category picker overlays used by view/section fields
2. Improve category matching affordances (Lotus-inspired “type first, pick if needed”)
3. Add concise field help text in details pane
4. Improve preview summary and lane estimate (when preview is open)

Acceptance criteria:

- Large category hierarchies remain usable
- Users can stay mostly on the keyboard and type-to-match for common edits

## 8. Technical Design Plan

## 8.1 Phase 1 (Current Architecture) Code Changes

### A. Auto-Create First Section In TUI Create Path

Current creation is finalized in `handle_view_create_category_key(..., Enter)` in:

- `/Users/mds/src/aglet/crates/agenda-tui/src/modes/view_edit.rs`

Implementation approach:

1. Add a helper for a default section (TUI convenience):
   - `fn default_new_section(title: &str) -> Section` (name TBD)
2. In the TUI create path, before `store.create_view(&view)`:
   - if `view.sections.is_empty()`, push a default section
3. Keep core `View::new()` unchanged

Recommended helper placement:

- `/Users/mds/src/aglet/crates/agenda-tui/src/modes/view_edit2.rs` or shared TUI helper module

Reason:

- Reused by create flow and section-add actions

### B. Open `ViewEdit` Focused On First Section Title Edit After Create

Current `open_view_edit(view)` initializes to the criteria region.

Implementation approach:

1. Add an intent-based helper (e.g. `open_view_edit_with_focus(...)`)
2. Support an open intent for “focus new section title edit”
3. On create success, use that intent path

Required state initialization:

- `region = ViewEditRegion::Sections`
- `section_index = 0` (or created section index)
- `section_expanded = Some(section_index)` (recommended)
- `inline_input = Some(ViewEditInlineInput::SectionTitle { section_index })`
- `inline_buf = TextBuffer::new(default_title)`

### C. Make `N` In `ViewEdit.Sections` Auto-Start Title Edit

Current behavior:

- `N` adds a section and selects it, but does not start title editing

Change:

- Reuse the same “new section + start title edit” helper used by create flow

### D. Dirty Tracking + Discard Confirmation For `ViewEdit`

Current `Esc` in `ViewEdit` discards the draft immediately.

Implementation approach:

1. Add dirty state to `ViewEditState` (bool or snapshot compare)
2. Mark dirty on draft mutations and inline commits
3. On `Esc`:
   - overlay/inline active -> existing local cancel behavior
   - dirty -> inline confirm (`Discard changes? y/n`)
   - clean -> close editor

## 8.2 Full-Screen View Editor State Model (Phase 3 Target)

This is **not** a global “all views” manager. It is an editor for one selected view, opened from the quick picker.

Proposed shape (illustrative, not final):

- `ViewEditorState`
  - `draft: View`
  - `focus: ViewEditorFocus` (`Sections`, `Details`, `Preview`)
  - `sections_list_index: usize`
  - `section_filter: TextBuffer`
  - `section_filter_active: bool`
  - `details_focus: ViewEditorDetailsFocus`
  - `inline_action: Option<ViewEditorInlineAction>`
  - `overlay: Option<ViewEditorOverlay>`
  - `show_preview: bool`
  - `dirty: bool`
  - `preview_cache` (optional counts/summary)

Supporting row identity (sections pane):

- `ViewEditorRow`
  - `ViewProperties`
  - `Section { section_index }`
  - `UnmatchedPseudoRow` (optional convenience row)

Supporting actions:

- `ViewEditorInlineAction`
  - `RenameView { buf }`
  - `RenameSection { section_index, buf }`
  - `CreateSection { insert_index, buf }`
  - `DeleteConfirmSection { section_index }`
  - `DiscardDraftConfirm`
- `ViewEditorOverlay`
  - `CategoryPicker { target }`
  - `BucketPicker { target }`

## 8.3 Rendering Plan (Phase 3 Target)

Add/refactor renderer in:

- `/Users/mds/src/aglet/crates/agenda-tui/src/render/mod.rs`

Default layout (recommended):

1. Left pane: `Sections` (with synthetic `View Properties` row)
2. Right pane: `Details`
3. Optional preview pane shown only when toggled (`p`)
4. Compact filter bar shown only when active/focused (`/`)

Rendering principles:

- Focused pane border cyan, inactive blue (same visual language as category manager/current `ViewEdit`)
- Low clutter by default (no always-on filter/preview panes)
- Inline editing row clearly shows active text buffer
- Footer hints are mode-aware and match actual bindings

## 8.4 Data Mutation Rules / Invariants

These rules remain true through all phases:

1. Core continues to allow zero-section views
2. TUI create path auto-adds first section by default
3. Section ordering is explicit and stable
4. Deleting the last section is allowed (advanced case) unless product later chooses to restrict it in the editor

Recommended behavior for deleting the last section:

- Allow it
- Show explicit status: `View has no sections; items will appear in unmatched if enabled`

## 9. Keybinding Compatibility / Migration Notes

Preserve where possible:

- `v` / `F8` entry point to quick picker
- `e` to open editor from picker (retain `V` alias if useful)
- `S` as explicit save in view editing contexts
- `j/k`, `Tab`, `Esc` semantics aligned with category manager
- `r` rename and `x` delete in picker/editor list contexts

Changes that need user-facing communication:

- TUI create flow no longer uses `ViewCreateCategory` (Phase 2+)
- `N`/`n` section insertion semantics become above/below-current in the full-screen editor (Lotus-inspired)
- `Esc` prompts on dirty draft instead of silently closing

## 10. Testing Plan

## 10.1 Unit Tests (TUI)

Add/extend tests for:

- TUI create view path auto-adds first section
- Create path opens editor focused on first section title edit
- `N` in sections auto-starts title edit
- `Esc` dirty-discard confirmation flow in view editing
- Save after auto-created section persists expected defaults
- Add-above / add-below section insertion behavior (Phase 3)
- Delete last section status messaging (if implemented)

## 10.2 Regression Tests

Preserve:

- Core/query behavior for zero-section views (`resolve_view` unmatched behavior)
- Existing CLI view create/edit semantics
- View save/load persistence for section fields and unmatched settings
- Quick `v` picker switch behavior (stays simple and unchanged)

## 10.3 Manual Smoke Script (Add/Update)

Create or update a docs script covering:

1. Open `v` quick picker and switch views
2. Create a new view from picker
3. Confirm first section is auto-created and title editor opens
4. Add section below (`n`) and above (`N`) in editor
5. Edit section criteria and columns in details pane
6. Toggle preview on/off (`p`)
7. Cancel with unsaved changes and verify discard prompt
8. Save and reopen to verify persistence

## 11. Rollout Recommendation

Recommended order:

1. Phase 1 (auto first section + immediate title edit + dirty confirm in current `ViewEdit`)  
2. Phase 2 (remove `ViewCreateCategory` from TUI create path)  
3. Phase 3 (build calmer full-screen View Editor; keep quick picker)  
4. Phase 4 (matching/picker polish)

This sequence preserves the fast path while improving deep editing incrementally.

## 12. Detailed TODO Checklist

This checklist is the implementation work breakdown for the plan. It is intentionally detailed so work can proceed incrementally without re-deciding scope at each step.

### Phase 0: Prep / Decision Lock (No Behavior Changes)

- [x] Lock product decisions for Phase 1/2 in this doc:
  - [x] split architecture (`v` quick picker + separate full-screen editor)
  - [x] auto-create first section in TUI create flow
  - [x] keep zero-section views valid in core
- [ ] Lock key semantics to avoid churn during implementation:
  - [ ] confirm `n`/`N` meaning in current `ViewEdit` Phase 1 (`N` remains add section, `n` alias or no-op decision)
  - [ ] confirm target default first section title (`Main` vs `New section`)
  - [ ] confirm whether delete-last-section remains allowed in TUI
- [ ] Align docs/spec references:
  - [x] ensure this plan remains source-of-truth for the split UX direction
  - [x] ensure mockup doc matches plan wording and key semantics
  - [ ] add brief reference note in any active TUI workflow spec if needed
- [x] Create implementation branch from `main` (done outside the checklist if already created)

### Phase 1: Immediate UX Wins In Current `ViewEdit`

#### 1.1 Shared Section Creation Helper

- [x] Add a TUI helper for constructing a default section (name TBD)
- [x] Use the helper in current `ViewEdit.Sections` add-section path
- [ ] Cover helper defaults in unit tests (or test through call sites)

#### 1.2 Auto-Create First Section On TUI View Create

- [x] Update `handle_view_create_category_key(... Enter)` create path to insert a default section before `store.create_view(&view)` when the view has no sections
- [x] Ensure behavior is scoped to TUI create flow only (core `View::new()` unchanged)
- [x] Verify create path still preserves any criteria selected in `ViewCreateCategory`

#### 1.3 Open `ViewEdit` Focused On First Section Title Edit After Create

- [x] Refactor `open_view_edit(view)` initialization so create flow can request a focused-start variant (e.g. `ViewEditOpenIntent`)
- [ ] Add “focus first/new section title edit” intent/state initialization:
  - [x] `region = Sections`
  - [x] select created section index
  - [x] initialize inline title input buffer
  - [x] optionally expand the section row
- [x] Route successful TUI view create to the focused-start open path
- [x] Preserve existing `e`/`V` edit-from-picker behavior (normal open path still lands where expected)

#### 1.4 Make Section Add Start Inline Title Edit Immediately

- [x] Update `handle_view_edit_sections_key` so section add enters inline title edit immediately
- [x] Reuse shared helper/state setup from create flow (avoid duplicate init logic)
- [x] Confirm repeated section adds behave correctly (selection/index/expansion remains stable)

#### 1.5 Dirty Tracking + Discard Confirmation In Current `ViewEdit`

- [x] Add dirty state to `ViewEditState`
- [ ] Decide dirty implementation approach:
  - [x] explicit dirty flag set on mutation
  - [ ] or snapshot comparison on exit (document tradeoff if chosen)
- [ ] Mark dirty on all draft mutations:
  - [x] view criteria edits
  - [x] bucket edits
  - [x] section add/remove/reorder
  - [x] section property toggles
  - [x] overlay category toggles (criteria/columns/on-insert/on-remove)
  - [x] unmatched visibility/label edits
  - [x] inline text commits (section title, unmatched label)
- [x] Add discard confirmation state/flow for top-level `Esc` when dirty
- [x] Ensure `Esc` precedence remains layered:
  - [x] inline input cancel first
  - [x] overlay close second
  - [x] discard confirm third
  - [x] editor close last
- [x] Ensure save clears dirty state and closes cleanly

#### 1.6 UX Copy / Footer Hint Updates

- [x] Update footer hints for `ViewEdit.Sections` to reflect immediate title edit after add
- [ ] Update status messages for:
  - [x] view create -> first section title edit
  - [x] add section -> type title
  - [x] discard prompt
- [x] Verify hint text matches actual key behavior

#### 1.7 Phase 1 Testing

- [ ] Add/extend TUI tests for:
  - [x] TUI create path auto-adds first section
  - [x] create path opens `ViewEdit` focused on first section title inline edit
  - [x] `N` in sections starts inline title edit
  - [x] `Esc` with dirty draft prompts instead of silent cancel
  - [x] save after auto-created section persists expected defaults
- [ ] Manual smoke pass:
  - [ ] `v -> N` create view
  - [ ] confirm first section title starts editing
  - [ ] add second section quickly
  - [ ] cancel dirty draft and verify prompt

### Phase 2: Remove `ViewCreateCategory` From TUI Create Path

#### 2.1 Create Flow Refactor (Picker -> Editor Direct)

- [x] Refactor `ViewPicker` create flow to skip `Mode::ViewCreateCategory`
- [x] Create view with default draft semantics (including auto first section)
- [x] Open editor immediately after name confirmation
- [x] Ensure editor receives correct selection/focus state (first section title edit)

#### 2.2 Criteria Editing Consolidation

- [x] Move create-time criteria setup responsibility to editor view-properties/details surface
- [x] Remove hidden default criterion behavior from TUI create path
- [x] Confirm users can still create an unfiltered view and configure criteria later without surprises

#### 2.3 Legacy Flow Cleanup

- [x] Audit whether `Mode::ViewCreateCategory` is still needed anywhere in TUI
- [ ] If still needed temporarily, narrow usage and add comments
- [x] If no longer used, remove dead routing/rendering code in a follow-up cleanup commit

#### 2.4 Phase 2 Testing

- [ ] Add/extend tests for:
  - [x] `N` in view picker no longer routes to `ViewCreateCategory`
  - [x] create flow opens editor directly
  - [x] no implicit criterion is added unless user edits criteria in editor
- [ ] Manual smoke pass:
  - [ ] create unfiltered view
  - [ ] create filtered view by editing criteria in editor
  - [ ] save and reopen

### Phase 3: Build Full-Screen View Editor (Separate Deep Editor, Keep Quick Picker)

#### 3.1 Mode / State Architecture

- [ ] Decide whether to:
  - [ ] introduce new `Mode::ViewEditor` and new state type
  - [ ] or refactor `Mode::ViewEdit` into the new shape while preserving entry semantics
- [ ] Add `ViewEditorState` (single-view draft state) to `App`
- [ ] Define supporting enums/types:
  - [ ] `ViewEditorFocus`
  - [ ] `ViewEditorRow` (`ViewProperties`, `Section{...}`, optional `UnmatchedPseudoRow`)
  - [ ] `ViewEditorInlineAction`
  - [ ] `ViewEditorOverlay`
  - [ ] `ViewEditorDetailsFocus`
- [ ] Add open/close helpers:
  - [ ] open editor from selected view in picker
  - [ ] open editor in “new view / first section title edit” intent
  - [ ] close editor and return to picker

#### 3.2 Sections Pane (Left Pane) Implementation

- [ ] Render a dedicated sections list pane with synthetic `View Properties` row
- [ ] Add selection navigation (`j/k`, arrows)
- [ ] Implement row mapping and stable selection identity across mutations
- [ ] Implement section quick actions:
  - [ ] `n` add below current row
  - [ ] `N` add above current row
  - [ ] `r` rename selected section/view
  - [ ] `x` delete selected section (and view if supported in editor)
  - [ ] `J/K` reorder selected section
- [ ] Implement inline insert/rename flows with correct insert indices
- [ ] Add inline delete confirmation flow(s)

#### 3.3 Details Pane (Right Pane) Implementation

- [ ] Render a field-based details pane for selected row
- [ ] Implement details focus movement (`j/k`)
- [ ] Implement section field editing:
  - [ ] Title (inline text)
  - [ ] Criteria (overlay/picker)
  - [ ] Columns (overlay/picker)
  - [ ] On insert assign (overlay/picker)
  - [ ] On remove unassign (overlay/picker)
  - [ ] Show children toggle
  - [ ] Display mode override cycle
- [ ] Implement view field editing on `View Properties` row:
  - [ ] Name
  - [ ] Criteria
  - [ ] When include/exclude buckets
  - [ ] Board display mode
  - [ ] Unmatched visible
  - [ ] Unmatched label
  - [ ] (optional phase in Phase 3) remove-from-view unassign set
- [ ] Ensure field labels/help are concise and status/footer stays accurate

#### 3.4 Overlay / Picker Infrastructure In Editor

- [ ] Port/reuse category picker overlay logic for editor fields
- [ ] Port/reuse when-bucket picker overlay logic for view fields
- [ ] Ensure overlay key precedence is correct (overlay intercepts before pane keys)
- [ ] Ensure overlay close returns focus to the invoking pane/field

#### 3.5 Filter + Preview (Optional/Calm By Default)

- [ ] Add compact section filter bar toggled/focused with `/`
- [ ] Implement filter state and filtered row list for sections pane
- [ ] Ensure `Esc` clears active filter before attempting editor close
- [ ] Add preview toggle `p`
- [ ] Render preview pane only when enabled
- [ ] Compute lightweight preview summary/counts (match count, lane summary)
- [ ] Integrate preview pane into `Tab` focus cycle only when visible

#### 3.6 Save / Cancel / Dirty Behavior

- [ ] Keep explicit `S` save for editor draft
- [ ] Persist draft back to store and refresh app state on save
- [ ] Return to quick picker after save (or decide to remain in editor and document behavior)
- [ ] Rebuild any board section filters/slot state impacted by saved view changes
- [ ] Implement dirty-discard confirmation at editor top level

#### 3.7 Picker Integration (Keep Quick Picker Simple)

- [ ] Preserve current `ViewPicker` rendering simplicity
- [ ] Update picker `e` action to open the new full-screen editor mode
- [ ] Retain `V` alias if desired for compatibility
- [ ] Ensure `Enter` switch and `Esc` close behaviors remain unchanged

#### 3.8 Phase 3 Testing

- [ ] Add/extend TUI tests for:
  - [ ] open editor from picker
  - [ ] synthetic `View Properties` row behavior
  - [ ] add section above/below current row
  - [ ] reorder sections with `J/K`
  - [ ] details field edits (toggles, inline text, overlays)
  - [ ] preview toggle/focus cycle behavior
  - [ ] compact filter behavior
  - [ ] save/dirty/discard flows
- [ ] Manual smoke pass:
  - [ ] switch view via picker
  - [ ] open editor, add sections above/below
  - [ ] edit view and section properties
  - [ ] toggle preview
  - [ ] save, reopen, verify persistence

### Phase 4: Matching / Overlay / UX Polish

#### 4.1 Category Matching + Picker Polish (Lotus-Inspired)

- [ ] Add filtering/type-to-match to category picker overlays used by editor fields
- [ ] Improve match feedback (count / first match / current match) where practical
- [ ] Evaluate whether inline type-to-resolve is useful for some fields before opening a picker
- [ ] Keep fallback picker browse flow available for all category-backed fields

#### 4.2 UX Copy / Help / Discoverability

- [ ] Add concise field-level help/status messages in details pane interactions
- [ ] Audit footer hints for all editor focus states and overlays
- [ ] Ensure terminology is consistent:
  - [ ] “View Picker” vs “View Editor”
  - [ ] “add above/below”
  - [ ] “Unmatched” vs “generated lane”

#### 4.3 Visual / Layout Polish

- [ ] Tune column widths and wrapping in editor details pane
- [ ] Improve selected-row and focused-field styling consistency with category manager
- [ ] Implement narrow-terminal fallback behavior (2-pane + compact footer summary)
- [ ] Verify preview pane layout behaves well at smaller terminal sizes

#### 4.4 Phase 4 Testing

- [ ] Add tests for overlay filtering behavior
- [ ] Add tests for narrow-layout rendering state selection logic (where practical)
- [ ] Manual usability pass on large category hierarchies and long section lists

### Cross-Cutting: Documentation / Scripts / Release Notes

- [ ] Update docs mockup/plan as implementation decisions finalize (if key semantics change)
- [ ] Add or update manual smoke script in `docs/`
- [ ] Update any user-facing TUI keybinding docs/reference notes
- [ ] Capture migration notes for changed view-creation workflow (`ViewCreateCategory` removal)

### Completion Gate (Definition Of Done)

- [ ] Quick `v` picker remains simple and fast (no heavy editor UI)
- [ ] TUI create flow auto-creates first section and starts title edit
- [ ] Full-screen editor supports deep view + section editing with details pane
- [ ] `n`/`N` quick section insertion (below/above) works in editor
- [ ] Dirty drafts are protected by discard confirmation
- [ ] Tests and smoke scripts cover the new core flows
- [ ] Docs reflect shipped keybindings and workflow
## 13. Open Questions To Resolve Before Phase 3

1. Should the first auto-created section default title be `"Main"` or `"New section"`?
2. Should deleting the last section be allowed without extra confirmation beyond normal delete?
3. In the editor, should `N`/`n` mean above/below current (Lotus-inspired) or top-level/new-without-position semantics?
4. Should `View Properties` be a synthetic row in the sections pane, or a dedicated toggle/key to swap details scope?
5. Should preview be off by default in all terminal sizes?

## 14. Success Criteria (User-Facing)

This plan is successful when:

- `v` remains a quick, low-friction way to swap views
- A new user can create a view and first section without discovering hidden commands
- Section editing no longer feels like a sequence of “magic letters”
- The full-screen editor feels like the same UI family as the category manager
- Expert users retain fast keyboard-only workflows with fewer keystrokes than today
