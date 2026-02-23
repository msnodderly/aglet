# Plan: Full Rewrite of TUI Category Manager UX (Tree Editor)

Date: 2026-02-23
Scope: TUI category manager (`c` / `F9`) for category hierarchy creation, subcategory management, reparenting, sibling reordering, and category config editing
Primary code areas:
- `/Users/mds/src/aglet/crates/agenda-tui/src/modes/category.rs`
- `/Users/mds/src/aglet/crates/agenda-tui/src/render/mod.rs`
- `/Users/mds/src/aglet/crates/agenda-tui/src/lib.rs`
- `/Users/mds/src/aglet/crates/agenda-tui/src/ui_support.rs`
- `/Users/mds/src/aglet/crates/agenda-core/src/agenda.rs`
- `/Users/mds/src/aglet/crates/agenda-core/src/store.rs`

## 1. Objective

Replace the current category manager’s modal/popup workflow with a single, direct-manipulation tree editor that makes these tasks fast and obvious:

- create top-level categories
- create subcategories under the selected category
- rename categories
- move categories around (reparent + reorder among siblings)
- edit category flags and note
- delete categories safely

Target outcome:

- Common category operations happen in one screen with minimal mode switching.
- The interaction model matches the newer VisiData-style column UX patterns (type to filter, inline create confirm, direct list interaction).
- Structural edits (especially subcategories and moving) become first-class operations, not secondary flows hidden behind popups.

## 2. Current Baseline (What Exists Today)

Current category manager behavior (high level):

- `c` opens `Mode::CategoryManager`
- `n` opens Name Input popup to create child under selected row
- `N` opens Name Input popup to create top-level category
- `p` opens separate `Mode::CategoryReparent` picker
- `Enter` opens separate `Mode::CategoryConfig` popup
- `x` opens separate `Mode::CategoryDelete` confirm

Problems:

- Too many mode hops for simple tasks.
- Create/rename uses `InputPanel(NameInput)` semantics (`S` save), which are not obvious from the tree context.
- Reparenting has no filter and no direct tree movement shortcuts.
- No direct sibling reorder action in category manager even though storage already has category `sort_order`.
- “Manage sub-categories and move things around” feels like a set of disconnected commands rather than one editing surface.

## 3. Full Rewrite UX Direction

### 3.1 New Mental Model: Category Tree Editor

Keep `c` as the entry point, but make the category manager a dedicated tree editor with:

- left pane: category tree (filterable)
- right pane: details/config for selected category
- inline action states (rename/create/delete confirm) embedded in the same screen
- direct structural movement keys (reparent + reorder)


### 3.2 Guiding Principles

- One primary mode, fewer transient modes
- Type-to-filter everywhere (borrowed from column picker UX)
- Inline create when no match exists (borrowed from column picker UX)
- Keep vim/terminal-friendly keys (`j/k`, `Esc`, `Enter`)
- Preserve discoverable footer hints and status messages
- Reserved category rules must remain enforced

### 3.3 Proposed User Workflow (Example)

Goal: create `Projects` (top-level), create `Aglet` under it, then move `Aglet` under `Work`, and mark `Projects` as exclusive.

Example flow (target UX):

- `c` open category tree editor
- `N` start inline create (root), type `Projects`, `Enter`, `y`
- `n` start inline create (child of selected `Projects`), type `Aglet`, `Enter`, `y`
- `k/j` navigate to `Aglet` if needed
- `H` or `p` move/reparent `Aglet` (see keymap below)
- `j/k` select target parent (if picker fallback path), `Enter`
- `k/j` select `Projects`
- `e` toggle Exclusive Children (or toggle in details pane)

No modal `InputPanel`, no separate reparent screen for the common path, no popup config screen for simple flag toggles.

## 4. UX Contract (Proposed Keybindings)

The exact keys can be adjusted, but the rewrite should lock a coherent contract. Recommended default:

### 4.1 Navigation / Selection

- `j` / `Down`: move selection down visible rows
- `k` / `Up`: move selection up visible rows
- `g` / `G` (optional): jump top/bottom
- `Tab` / `Shift+Tab`: cycle focus (`Tree` -> `Filter` -> `Details`)
- `Esc`: cancel inline action / clear filter / close manager (in that priority order)

### 4.2 Filtering

- Typing in `Tree` focus appends to filter (auto-focus filter behavior)
- `Backspace`: edit filter
- `Enter` in filter context:
  - if matches exist -> selects highlighted match (and keeps editor open)
  - if no matches and filter is non-empty -> starts inline create confirm under current create context
- `/` (optional): move focus to filter explicitly
- `Ctrl-U` or `Alt-Backspace` (optional): clear filter

### 4.3 Create / Rename / Delete (Inline)

- `n`: create child of selected category
- `N`: create top-level category
- `a` (optional): create sibling of selected category
- `r`: rename selected category inline
- `x`: delete selected category (inline confirm `y/n`)
- `Enter`: confirm inline text action when editing a name

### 4.4 Move / Reparent / Reorder (Primary UX Win)

- `H`: outdent (reparent to current parent’s parent)
- `L`: indent (reparent under previous visible sibling, if valid)
- `J`: move down among siblings (swap/reorder)
- `K`: move up among siblings (swap/reorder)
- `p`: open filterable parent picker (fallback for exact parent selection / non-adjacent reparent)

Notes:

- `H/L/J/K` mirror the VisiData-style “move things around” feel introduced for board columns.
- `p` remains as an explicit fallback for complex reparenting, but it should be filterable and inline.

### 4.5 Category Config / Details Pane

- `e`: toggle Exclusive Children
- `i`: toggle Match category name
- `t` or `a`: toggle Actionable (pick one and preserve compatibility if possible)
- `Enter` on details note field: edit note inline in details pane
- `S`: save details draft (only needed if note/details are draft-based)

Recommendation:

- Flags (`e/i/a`) should apply immediately.
- Note editing can remain draft-based with explicit `S`.

## 5. Scope and Non-Goals

### In Scope

- Replace current category manager modal flow with a tree editor interaction model.
- Inline create/rename/delete within category manager.
- Filterable tree and filterable parent picker.
- Direct reparent and sibling reorder actions.
- Category details/config pane in category manager (or staged integration).
- Footer/status/help rewrite for the new model.
- Test coverage for all category hierarchy editing flows.

### Out of Scope (for this rewrite)

- Category schema redesign
- Bulk multi-category edits across multiple selected categories
- Rich mouse interactions / drag-and-drop
- CLI category UX changes

## 6. Referencing the Column UX (Partial Example to Reuse)

The category manager rewrite should reuse proven patterns from the VisiData-style column UX work (commit `2b4e1e4` and its descendants), especially:

- `CategoryColumnPicker` filter + list interaction model
- inline create confirmation when no matches exist
- status message style that teaches the current next action
- list cursor + filter state handling

Concrete reusable patterns:

- `inline_create_confirm_key_action(...)` pattern in `/Users/mds/src/aglet/crates/agenda-tui/src/modes/board.rs`
- `filter_category_ids_by_query(...)` in `/Users/mds/src/aglet/crates/agenda-tui/src/ui_support.rs`
- staged selection/list navigation state style from `CategoryColumnPickerState`

Important distinction:

- Column UX edits category assignments for an item under one parent.
- Category manager rewrite edits the category hierarchy itself.

So this is a pattern reuse, not a direct copy.

## 7. Technical Design Plan

## 7.1 Mode and State Architecture (Rewrite Target)

### Current State to Retire (Category Manager Flows)

The current category manager spreads functionality across:

- `Mode::CategoryManager`
- `Mode::CategoryReparent`
- `Mode::CategoryDelete`
- `Mode::CategoryConfig`
- `Mode::InputPanel` + `NameInputContext::{CategoryCreate,CategoryRename}`

### Proposed Replacement

Keep `Mode::CategoryManager`, but make it own a richer state object, for example:

- `CategoryManagerState` (new)

Proposed fields (shape, not final names):

- `focus: CategoryManagerFocus` (`Tree`, `Filter`, `DetailsFlags`, `DetailsNote`, `ParentPicker`)
- `tree_index: usize`
- `filter: text_buffer::TextBuffer`
- `filtered_row_ids: Vec<CategoryId>` or `filtered_row_indices: Vec<usize>`
- `parent_picker: Option<CategoryParentPickerState>`
- `inline_action: Option<CategoryInlineAction>`
- `details_draft: Option<CategoryDetailsDraft>`
- `dirty_details: bool`
- `last_selected_category_id: Option<CategoryId>`

New helper enums (suggested):

- `CategoryManagerFocus`
- `CategoryInlineAction`
  - `CreateRoot { buf, confirm_name }`
  - `CreateChild { parent_id, buf, confirm_name }`
  - `CreateSibling { parent_id, buf, confirm_name }`
  - `Rename { category_id, original_name, buf }`
  - `DeleteConfirm { category_id }`
- `CategoryParentPickerState`
  - `target_category_id`
  - `filter`
  - `list_index`
  - `visible_parent_options`
  - `create_confirm` (usually none; parent picker is select-only)
- `CategoryDetailsDraft`
  - `category_id`
  - `note: TextBuffer`
  - `focus` (if details pane is form-like)

Design goal:

- Most transient state lives under `CategoryManagerState`, not in top-level `App` fields.
- Reduce scattered category manager globals (`category_create_parent`, `category_reparent_options`, `category_reparent_index`, `category_config_editor`) after cutover.

## 7.2 Rendering (New Category Tree Editor Layout)

Update `/Users/mds/src/aglet/crates/agenda-tui/src/render/mod.rs` to render a persistent two-pane layout:

- Header: `Category Manager` + context (filter summary / active inline action)
- Left pane: tree table/list
  - category name with indentation
  - optional flag columns (`Excl`, `Match`, `Todo`) preserved
  - row markers for selection and filtered matches
- Right pane: details
  - selected category name + parent path
  - flags (exclusive / match-name / actionable)
  - note editor
  - constraints/info (`reserved`, child count, item impact preview optional)
- Footer help: mode-aware hints matching actual keys

Optional (Phase 2+):

- Show filtered match count and whether filter is narrowing all rows or only current subtree.
- Highlight pending inline action row in a distinct style.

## 7.3 Input Handling (Single-State Machine)

Rewrite `/Users/mds/src/aglet/crates/agenda-tui/src/modes/category.rs` around a single dispatcher:

- first handle inline action state (rename/create/delete confirm)
- then handle parent picker if open
- then handle global keys (`Esc`, `Tab`, filter typing, navigation)
- then handle structural ops / config toggles / note editing

Recommended precedence:

1. Inline confirm/edit consumes input
2. Parent picker consumes input
3. Details note editor consumes input (when focused)
4. Tree/filter shortcuts

This avoids the current “which mode am I in?” confusion while still using clear sub-states.

## 7.4 Category Create/Rename Inline Flow

### Create Flow

For `n` / `N` / `a`:

- open inline name buffer in the same screen (tree or header row)
- `Enter`:
  - if empty -> error status
  - if reserved name -> error status
  - if duplicate under target parent -> error status
  - else open inline create confirm (`Create '<name>' under <parent>? [Y/n]`)
- `y` / `Enter` confirms
- `n` / `Esc` cancels confirmation and returns to inline edit or tree

Reuse pattern from column UX:

- same confirm key behavior (`y/Enter` confirm, `n/Esc` cancel)
- same “dismiss and continue typing” behavior for navigation keys if useful

### Rename Flow

- `r` opens inline rename for selected row
- `Enter` applies via `agenda.update_category(...)`
- Preserve selection on renamed category
- Reserved category rename remains blocked

## 7.5 Reparenting and Reordering (Core Functional Difference vs MVP)

This is the main reason for a full rewrite.

### Reparent via Direct Tree Keys (`H` / `L`)

#### `L` (Indent)

Attempt to make selected category a child of the previous visible sibling:

- Validate there is a valid previous visible row at same depth (or compatible target)
- Reject if target would create a cycle
- Reject if selected category is reserved (if reserved categories stay structurally read-only)
- Reparent selected category to that target parent
- Insert at end of new parent’s children (or directly after the target’s existing children, design choice)

#### `H` (Outdent)

Move selected category to the parent of its current parent:

- If already root: no-op with status
- New parent becomes current parent’s parent (or root)
- Insert after current parent in the target sibling list (preferred for intuitive spatial movement)

### Reorder among Siblings (`J` / `K`)

Move selected category within the same parent’s child order:

- `K` moves up one sibling
- `J` moves down one sibling
- Preserve selection after reorder
- No semantic item re-evaluation needed for pure reorder

### `p` Fallback Parent Picker

For precise/non-adjacent reparent:

- open filterable parent picker inline (same screen)
- options include `(root)` and all valid non-descendant categories
- `Enter` applies reparent
- `Esc` cancels

This replaces the current non-filterable `CategoryReparent` picker.

## 7.6 Core/Store API Changes (Needed for Full Rewrite)

Current storage already has category `sort_order`, but TUI lacks explicit reorder/move APIs.

Add store APIs (names illustrative):

- `move_category_to_parent(category_id, new_parent_id, insert_pos)` (transactional)
- `move_category_before(category_id, sibling_id)` / `move_category_after(...)`
- `move_category_within_parent(category_id, delta)`
- `normalize_category_sort_orders(parent_id)` (internal helper)

Alternative API design:

- one generic transaction method that rewrites sibling `sort_order` for affected parent(s)

Requirements:

- Transactionally update all impacted siblings
- Preserve contiguous, deterministic sibling order
- Validate no cycles / self-parenting
- Work for root (`parent_id IS NULL`) and child lists

Agenda-level wrappers:

- structural moves that change `parent` should go through `Agenda` and re-run evaluation (same as `update_category`)
- pure reorder may be store-only (no semantics change), but prefer an `Agenda` wrapper for UI consistency if practical

## 7.7 Details Pane Integration (Flags + Note)

Two implementation options:

### Option A (Preferred for user UX)

- Flags apply immediately (`e/i/a`)
- Note is edited in place in details pane with `TextBuffer`
- `S` saves note changes
- `Esc` in note focus cancels note edits only (not manager)

### Option B (Transitional)

- Keep existing `CategoryConfig` popup behind `Enter` initially
- Ship tree/reparent/reorder rewrite first
- Replace popup with details pane in a follow-up phase

Recommendation:

- Implement Option B only as a short-lived milestone if delivery risk is high.
- Final rewrite should converge to Option A and remove `Mode::CategoryConfig`.

## 7.8 Selection, Filtering, and Stability Rules

The rewrite should explicitly preserve user position across operations.

Rules:

- After create: select new category
- After rename: keep same category selected
- After reparent/reorder: keep moved category selected
- After delete:
  - prefer next visible sibling
  - else previous visible row
  - else nearest parent
- Filter edits should not lose the selected category if it still matches
- Clearing filter should restore selection to the same category ID

## 8. Implementation Strategy (Phased Delivery)

This is a full rewrite plan, but it should still ship incrementally to reduce breakage risk.

### Phase 0: UX Contract + State Cutover Plan

Deliverables:

- Lock keybindings and precedence rules
- Decide details-pane strategy (Option A vs transitional B)
- Define `CategoryManagerState` shapes and cleanup targets
- Document reserved-category behavior for move/toggle/edit actions

Acceptance:

- Written contract reviewed and stable enough for test writing

### Phase 1: Core Category Move/Reorder APIs (No TUI Rewrite Yet)

Deliverables:

- Add store/agenda APIs for sibling reorder and reparent-with-order
- Add unit tests for root and nested category ordering
- Ensure `get_hierarchy()` reflects updated order correctly

Acceptance:

- Can reorder siblings and reparent categories while preserving deterministic order
- Cycle/self-parent validation remains intact

### Phase 2: CategoryManagerState Scaffold + New Renderer Shell

Deliverables:

- Introduce `CategoryManagerState`
- Add new render layout (tree + details placeholder + filter)
- Route `Mode::CategoryManager` through new state while preserving current read-only navigation

Acceptance:

- `c` opens new layout
- `j/k` navigation works
- footer hints correspond to new layout

### Phase 3: Filter + Inline Create/Rename/Delete (Single-Screen)

Deliverables:

- Type-to-filter behavior
- inline create flows (`n`, `N`, optional `a`)
- inline create confirm (reusing column UX behavior pattern)
- inline rename
- inline delete confirm
- remove category create/rename dependency on `InputPanel(NameInput)` for category manager path

Acceptance:

- Create top-level and child categories without entering `Mode::InputPanel`
- Rename without popup
- Delete confirm is inline and preserves selection behavior

### Phase 4: Reparent + Reorder (Primary Structural Editing UX)

Deliverables:

- `H/L` direct reparent
- `J/K` sibling reorder
- filterable parent picker fallback on `p`
- status messages for invalid moves and successful moves

Acceptance:

- Common “move this under that” flows require no separate full-screen mode
- Non-adjacent reparent is possible via filtered picker
- Selection stability rules hold after moves

### Phase 5: Details Pane Editing (Flags + Note)

Deliverables:

- inline/details-pane note editing
- immediate flag toggles
- remove or deprecate `Mode::CategoryConfig`
- update footer/status help for details focus

Acceptance:

- `Enter` no longer needs to open a config popup for normal category editing
- full category editing is possible within category manager

### Phase 6: Cleanup, Deletion of Old Modes, and Documentation

Deliverables:

- remove obsolete mode-specific fields/state from `App`
- retire `Mode::CategoryReparent`, `Mode::CategoryDelete`, `Mode::CategoryConfig` if fully superseded
- clean up `NameInputContext::{CategoryCreate, CategoryRename}` usage in category manager path
- update docs and smoke scripts

Acceptance:

- Category manager code paths are centralized and simpler to reason about
- footer hints/docs reflect actual final UX

## 9. Testing Plan

## 9.1 Core Tests (`agenda-core`)

Add tests for:

- reordering root siblings (`K/J` semantics via API)
- reordering nested siblings
- reparenting to root and nested parent preserves valid ordering
- reparent cycle prevention (cannot parent under descendant)
- self-parent rejection
- reserved-category move restrictions (if enforced at core layer)

## 9.2 TUI Unit Tests (`agenda-tui`)

Add/extend tests for:

- `c` opens new category editor layout/state
- inline create root (`N`) success
- inline create child (`n`) success
- reserved-name rejection (`Done`, `When`, `Entry`)
- duplicate-name rejection under same parent
- inline rename success + reserved rename rejection
- inline delete confirm/cancel
- `H/L` reparent success/failure cases
- `J/K` sibling reorder success/failure cases
- `p` parent picker filter + apply
- selection preservation after create/move/reorder/delete
- details flag toggles and note save/cancel (if Phase 5 included)

Regression tests:

- `c` still opens category manager from board mode
- unrelated board column UX remains unchanged
- item edit `InputPanel` flows still use existing `S` semantics

## 9.3 Manual Smoke Script

Add/update a `docs/` smoke script covering:

- create root category
- create nested subcategory
- rename category
- reorder sibling up/down
- reparent with `H/L`
- reparent with `p` + filter
- toggle exclusive/match/actionable
- edit note
- delete leaf category
- cancel each inline action path (`Esc`, `n`)

## 10. Risks and Mitigations

### Risk: Rewrite introduces keybinding confusion

Mitigation:

- Lock UX contract before implementation
- Keep `j/k`, `Esc`, `Enter`, `n/N`, `r`, `x`, `p` where possible
- Footer hints must be generated from the same state machine rules

### Risk: Tree movement logic becomes brittle

Mitigation:

- Implement core move/reorder APIs first with strong tests
- Keep TUI movement commands thin wrappers over tested core/store operations

### Risk: Selection and filter interactions regress

Mitigation:

- Add explicit selection-stability tests
- Make all post-op selection logic ID-based, not row-index-based

### Risk: Over-scoping details-pane editing delays structural UX improvements

Mitigation:

- Phase structural editing (create/reparent/reorder) before note/details polish
- Allow temporary details fallback only if needed, with a clear removal phase

## 11. Proposed Delivery Order (Practical)

1. Phase 0 (contract)
2. Phase 1 (core move/reorder APIs)
3. Phase 2 (new layout/state scaffold)
4. Phase 3 (inline create/rename/delete)
5. Phase 4 (reparent/reorder UX)
6. Phase 5 (details pane finalize)
7. Phase 6 (cleanup + docs)

This order delivers the biggest user-visible improvements early (create + moving subcategories around) while reducing rewrite risk via a tested core movement API.

## 12. Definition of Done (Final Rewrite)

The rewrite is done when:

- category creation, subcategory creation, rename, delete, reparent, and reorder can all be performed from one category editor screen
- common flows no longer enter `InputPanel` or separate category reparent/config screens
- moving categories around is fast (`H/L/J/K`) and precise (`p` picker fallback)
- footer/status hints match the final interaction contract
- tests cover structural operations and selection stability

## 13. Detailed TODO List (Implementation Checklist)

This checklist is intentionally detailed and task-oriented so work can be executed incrementally without re-planning each phase.

### Phase 0: UX Contract + State Cutover Plan

- [ ] Confirm final keybinding contract for category tree editor
  - [ ] Decide whether `a` is `create sibling` or remains `toggle actionable`
  - [ ] Confirm `t` vs `a` for actionable toggle (compatibility decision)
  - [ ] Confirm whether `Enter` on tree opens note/details focus or is reserved for inline actions only
  - [ ] Confirm `g/G` jump behavior inclusion
  - [ ] Confirm `/` and clear-filter shortcut support (`Ctrl-U` and/or `Alt-Backspace`)
- [ ] Define final focus model and key precedence rules
  - [ ] Document exact `Esc` priority behavior (inline action -> parent picker -> clear filter -> close manager)
  - [ ] Document when typing edits the filter vs note vs inline rename/create buffer
  - [ ] Document `Tab` / `Shift+Tab` cycle order
- [ ] Choose details-pane rollout strategy
  - [ ] Decide Option A (inline details pane now) vs Option B (temporary config popup fallback)
  - [ ] If Option B, define explicit removal criteria and target phase
- [ ] Define reserved-category behavior contract
  - [ ] Can reserved categories be reordered among roots?
  - [ ] Can reserved categories be reparented? (recommended: no)
  - [ ] Can reserved categories toggle flags? (current behavior is limited)
  - [ ] Can reserved categories note-edit? (decide and document)
- [ ] Define movement semantics precisely
  - [ ] `L` indent target selection rule (previous visible row vs previous sibling only)
  - [ ] `H` outdent insertion position rule (after parent vs end of target level)
  - [ ] `J/K` sibling reorder behavior at boundaries
  - [ ] Behavior when filter is active (move in filtered view vs underlying full tree)
- [ ] Document selection stability rules in a short contract appendix
- [ ] Add a “Rewrite Invariants” subsection to this plan
  - [ ] Selection preserved by category ID across refreshes
  - [ ] Tree render order always reflects `get_hierarchy()`
  - [ ] No hidden modal transitions for category create/rename in final UX

### Phase 1: Core Category Move/Reorder APIs (`agenda-core`)

- [x] Audit current category ordering behavior in `Store::get_hierarchy()` and category writes
  - [x] Confirm `sort_order` semantics for roots and child lists
  - [x] Confirm duplicate/negative gaps are tolerated and normalized only on writes
- [x] Design API signatures for structural edits (store layer)
  - [x] Choose final function names and argument shapes
  - [x] Decide on explicit `insert_pos` enum/type vs `before/after` helpers
  - [x] Decide if pure reorder APIs belong in `Store` only or also `Agenda`
  - Note: implemented `Store::move_category_within_parent(category_id, delta)` and `Store::move_category_to_parent(category_id, new_parent_id, insert_index)`.
- [x] Implement helper(s) in `crates/agenda-core/src/store.rs`
  - [x] Fetch siblings for a parent ordered by `sort_order`
  - [x] Rewrite sibling `sort_order` sequence transactionally
  - [ ] Normalize sibling sort orders helper (internal)
  - [x] Utility to insert/move an ID in a sibling vector
- [x] Implement store API: move category within same parent (reorder)
  - [x] Validate category exists
  - [x] Validate target movement is legal (same parent only for this API)
  - [x] Persist reordered sibling sort orders in one transaction
- [x] Implement store API: move category to new parent with ordering
  - [x] Validate category exists
  - [x] Validate parent exists when non-root
  - [x] Validate not self-parent
  - [x] Validate no cycle (cannot reparent under descendant)
  - [x] Update `parent_id`
  - [x] Reassign sort orders in old and new parent sibling lists transactionally
- [x] Implement agenda-level wrapper(s) in `crates/agenda-core/src/agenda.rs`
  - [x] Reparent wrapper that triggers re-evaluation and returns `EvaluateAllItemsResult`
  - [x] Reorder wrapper (decide whether to re-evaluate; document choice)
  - Note: reorder wrapper is store-only (`Result<()>`) because sibling order does not affect category assignment semantics.
- [x] Add unit tests for store structural APIs
  - [x] reorder root siblings up/down
  - [x] reorder child siblings up/down
  - [ ] reparent root -> child
  - [x] reparent child -> root
  - [x] reparent child -> different parent
  - [x] cycle prevention
  - [x] self-parent rejection
  - [x] invalid parent not found
  - [ ] stable hierarchy ordering after multiple moves
- [x] Add agenda tests for reparent side effects
  - [x] Ensure evaluation path still runs
  - [x] Ensure returned result is surfaced correctly

### Phase 2: CategoryManagerState Scaffold + New Renderer Shell (`agenda-tui`)

- [ ] Introduce `CategoryManagerState` and related enums/structs in `crates/agenda-tui/src/lib.rs`
  - [ ] `CategoryManagerFocus`
  - [ ] `CategoryInlineAction`
  - [ ] `CategoryParentPickerState`
  - [ ] `CategoryDetailsDraft` (or placeholder type)
- [ ] Add `category_manager: Option<CategoryManagerState>` (or equivalent) to `App`
- [ ] Initialize/reset new category manager state in `App::default()`
- [ ] Add helper methods in `crates/agenda-tui/src/app.rs` / relevant impl blocks
  - [ ] open category manager session
  - [ ] close category manager session
  - [ ] sync selection to current category ID
  - [ ] rebuild filtered visible rows from `category_rows`
  - [ ] clamp tree cursor after refresh/filter changes
- [ ] Keep old category manager fields in place temporarily
  - [ ] Mark cleanup targets in comments or plan references (avoid risky big-bang removal)
- [ ] Build new renderer shell in `crates/agenda-tui/src/render/mod.rs`
  - [ ] Two-pane layout (tree + details placeholder)
  - [ ] Filter line/box
  - [ ] Tree table/list with current columns
  - [ ] Details placeholder panel (read-only)
  - [ ] Inline action/status region placeholder
- [ ] Update footer help for `Mode::CategoryManager` to new baseline hints
- [ ] Route `c` / `F9` open path to initialize new state
- [ ] Keep navigation behavior working (`j/k`) with new state while preserving existing selection IDs
- [ ] Add/adjust TUI tests for new render-mode state initialization
  - [ ] `c` opens manager and creates state
  - [ ] closing manager clears state
  - [ ] selection is valid when categories list is empty/non-empty

### Phase 3: Filter + Inline Create/Rename/Delete (Single-Screen)

- [ ] Implement filter state behavior in category manager
  - [ ] Text input handling for filter buffer
  - [ ] Backspace/delete editing
  - [ ] Clear filter action
  - [ ] Recompute visible rows on each edit
  - [ ] Preserve selection by category ID when possible
  - [ ] Clamp cursor to visible list length
- [ ] Implement tree/list selection over filtered rows
  - [ ] Map visible row cursor -> underlying category ID
  - [ ] Handle empty filtered result gracefully
- [ ] Extract/reuse inline create confirm key handling pattern from board column UX
  - [ ] Move helper to shared module (if worthwhile) or duplicate intentionally with tests
  - [ ] Ensure confirm/cancel/dismiss semantics match decided contract
- [ ] Implement inline create action state(s)
  - [ ] `n` create child (selected row parent)
  - [ ] `N` create root
  - [ ] optional `a` create sibling (if enabled)
  - [ ] Inline text buffer rendering
  - [ ] Validation: empty name
  - [ ] Validation: reserved names (`Done`, `When`, `Entry`)
  - [ ] Validation: duplicate name under target parent (case-insensitive)
  - [ ] Inline confirm prompt
  - [ ] Final create via `agenda.create_category(...)`
  - [ ] Refresh + select created category by ID
  - [ ] Status messages (create success/error/cancel)
- [ ] Implement inline rename action state
  - [ ] `r` enters rename mode with selected category name prefilled
  - [ ] Reserved category rename blocked with status
  - [ ] Validation: empty/unchanged name handling
  - [ ] Validation: duplicate under same parent
  - [ ] Apply via `agenda.update_category(...)`
  - [ ] Refresh + preserve selection by ID
  - [ ] Status messages (rename success/error/cancel)
- [ ] Implement inline delete confirm in category manager
  - [ ] `x` enters inline delete confirm
  - [ ] `y` confirms delete
  - [ ] `n` / `Esc` cancels delete
  - [ ] Show core error when deleting non-leaf category
  - [ ] Post-delete selection fallback logic
- [ ] Stop using `InputPanel(NameInput)` for category manager create/rename path
  - [ ] Remove category-manager-specific transitions into `Mode::InputPanel`
  - [ ] Keep `InputPanel` category flows for other modes untouched
- [ ] Add TUI tests for inline create/rename/delete flows
  - [ ] create root success
  - [ ] create child success
  - [ ] create duplicate rejected
  - [ ] reserved-name create rejected
  - [ ] rename success
  - [ ] rename unchanged cancels cleanly
  - [ ] reserved rename blocked
  - [ ] delete cancel
  - [ ] delete leaf success
  - [ ] delete non-leaf error remains in manager

### Phase 4: Reparent + Reorder (Primary Structural Editing UX)

- [ ] Implement direct sibling reorder actions (`J/K`)
  - [ ] Resolve selected category ID and parent
  - [ ] Detect boundary conditions (first/last sibling)
  - [ ] Call core reorder API
  - [ ] Refresh + preserve selection
  - [ ] Emit clear status messages
- [ ] Implement direct reparent outdent (`H`)
  - [ ] Compute current parent and grandparent
  - [ ] No-op at root with status
  - [ ] Determine insertion position (after parent preferred)
  - [ ] Call core reparent/move API
  - [ ] Refresh + preserve selection
  - [ ] Status messages and invalid-case handling
- [ ] Implement direct reparent indent (`L`)
  - [ ] Compute valid indent target (per Phase 0 contract)
  - [ ] Reject invalid targets (none, descendant, reserved constraints)
  - [ ] Determine insertion position under new parent
  - [ ] Call core reparent/move API
  - [ ] Refresh + preserve selection
  - [ ] Status messages and invalid-case handling
- [ ] Implement filterable inline parent picker (`p`) as fallback
  - [ ] Build valid parent option source (including `(root)`)
  - [ ] Exclude self and descendants
  - [ ] Add filter buffer + list cursor state
  - [ ] Add render overlay/pane for parent picker
  - [ ] `j/k` navigate parent options
  - [ ] `Enter` apply reparent
  - [ ] `Esc` cancel picker
  - [ ] Preserve selection after apply/cancel
- [ ] Decide and implement behavior while filter is active
  - [ ] movement commands operate on actual tree order, not filtered visual adjacency (recommended) OR
  - [ ] movement commands disabled with filter-active status
  - [ ] Add explicit status messaging for chosen behavior
- [ ] Add TUI tests for structural movement
  - [ ] `K` reorder up
  - [ ] `J` reorder down
  - [ ] reorder boundary no-op
  - [ ] `H` outdent child -> root
  - [ ] `L` indent under previous sibling
  - [ ] invalid indent target handled safely
  - [ ] `p` reparent with filter
  - [ ] reparent cycle prevented
  - [ ] selection preserved after each move

### Phase 5: Details Pane Editing (Flags + Note)

- [ ] Implement details pane content model
  - [ ] Selected category metadata view (name, parent, child count, reserved status)
  - [ ] Flag rows/controls (exclusive, match-name, actionable)
  - [ ] Note editor area
- [ ] Implement focus transitions for details pane
  - [ ] `Tab` into details flags and note
  - [ ] `Shift+Tab` back to tree/filter
  - [ ] Visual focus styling updates
- [ ] Implement immediate flag toggles from details pane and quick keys
  - [ ] Exclusive toggle
  - [ ] Match-name toggle
  - [ ] Actionable toggle
  - [ ] Reserved-category restrictions/messages
  - [ ] Refresh + preserve selection after flag updates
- [ ] Implement inline note editing in details pane
  - [ ] `TextBuffer` for note draft tied to selected category ID
  - [ ] Save semantics (`S`) and cancel semantics (`Esc` in note focus)
  - [ ] Empty note clears note (`None`)
  - [ ] Switching selection with dirty note: decide behavior
    - [ ] auto-save
    - [ ] prompt
    - [ ] discard with warning
  - [ ] Status messages for note saved/canceled/unchanged
- [ ] Remove or deprecate `Mode::CategoryConfig` usage from category manager path
  - [ ] `Enter` no longer opens config popup (or only does so behind temporary fallback)
  - [ ] Footer hints updated accordingly
- [ ] Add TUI tests for details-pane editing
  - [ ] quick flag toggles still work
  - [ ] note edit + save
  - [ ] note edit + cancel
  - [ ] selection change with dirty note follows chosen contract

### Phase 6: Cleanup, Mode Deletion, and Documentation

- [ ] Remove obsolete category manager state fields from `App` (after full cutover)
  - [ ] `category_create_parent`
  - [ ] `category_reparent_options`
  - [ ] `category_reparent_index`
  - [ ] `category_config_editor`
  - [ ] any category-manager-only compatibility leftovers
- [ ] Retire superseded modes (if fully replaced)
  - [ ] `Mode::CategoryReparent`
  - [ ] `Mode::CategoryDelete`
  - [ ] `Mode::CategoryConfig`
  - [ ] remove dead input routing branches
  - [ ] remove dead renderer branches
- [ ] Clean up `NameInputContext::{CategoryCreate, CategoryRename}` usage for category manager path
  - [ ] Keep view-related NameInput contexts intact
  - [ ] Remove stale status strings referring to popup category create/rename
- [ ] Refactor/cleanup shared helpers
  - [ ] Consolidate inline create confirm helper if duplicated
  - [ ] Consolidate list/filter cursor clamp helpers if duplicated
  - [ ] Add comments for non-obvious tree-move behavior
- [ ] Update footer help text and status copy for final UX
  - [ ] Manager default hints
  - [ ] Inline create/rename/delete hints
  - [ ] Parent picker hints
  - [ ] Details note edit hints
- [ ] Update docs
  - [ ] `/Users/mds/src/aglet/docs/demo-tui-walkthrough-complete-e2e.md` category manager steps
  - [ ] `/Users/mds/src/aglet/docs/test-script-tui-smoke-e2e.md` category manager smoke flows
  - [ ] Any plan/spec docs that mention old keybindings if they are intended to track implementation
- [ ] Add/refresh final manual smoke script for category tree editor
- [ ] Run full relevant test suites and fix regressions
  - [ ] `agenda-core` tests for category move/reorder
  - [ ] `agenda-tui` tests for category manager
  - [ ] targeted board/column UX regressions

### Cross-Phase Validation / Tracking Tasks

- [ ] Keep a running checklist of temporary compatibility paths (to remove before Phase 6 done)
- [ ] Update plan status after each completed phase with notes on deviations from contract
- [ ] Capture any surprising implementation gotchas in `AGENTS.md` / `CLAUDE.md` per project instructions
- [ ] If keybinding conflicts emerge, record final decisions directly in this plan before proceeding
