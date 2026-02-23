# Plan: Streamline TUI Category Editing (View/Section Column Cells)

Date: 2026-02-23
Scope: TUI category editing workflow for board/view columns (current `CategoryDirectEdit` modal)
Primary code areas: `/Users/mds/src/aglet/crates/agenda-tui/src/modes/board.rs`, `/Users/mds/src/aglet/crates/agenda-tui/src/render/mod.rs`, `/Users/mds/src/aglet/crates/agenda-tui/src/lib.rs`

## 1. Objective

Make category assignment from board cells significantly more intuitive, especially for non-exclusive parents (for example `Area`) where users often need to assign multiple categories to one item.

Target outcome:

- Reduce keystrokes for multi-category assignment.
- Remove focus-mode confusion (`Entries` / `Input` / `Suggestions`) from the common path.
- Align behavior with column semantics:
  - exclusive parent -> single-select
  - non-exclusive parent -> multi-select

## 2. Current UX Problems (Observed)

Current flow (for non-exclusive columns) is row-based and draft-based:

- user opens `CategoryDirectEdit`
- resolves one row at a time
- manually adds more rows
- manually saves draft (`S`)

This is confusing because:

- adding another category requires focus switching to `Entries` first
- `Tab` cycles focus except in `Suggestions`, where it autocompletes instead
- `Enter` resolves a row but does not save
- save requires uppercase `S`
- the user must understand rows even when their mental model is just “select multiple values”

## 3. UX Direction (Proposed)

### 3.1 Column-Type-Aware Editors

Use different editing flows depending on the column parent category:

- **Exclusive parent** (for example `Status`, `Priority`)
  - single-select picker (radio-style behavior)
  - `Enter` selects and closes (or resolves + save)
- **Non-exclusive parent** (for example `Area`)
  - multi-select picker (checkbox-style behavior)
  - `Space` toggles highlighted category on/off
  - `Enter` saves and closes

### 3.2 Common Interaction Model

In both picker types:

- Typing filters the list
- `j/k` or arrows move highlight
- `Esc` cancels
- Optional create flow for missing category (child under current column parent)

### 3.3 Proposed Example (Non-Exclusive `Area`)

Assign `CLI` and `UX` to an item:

- `Enter` (open picker)
- type `cli`
- `Space` (toggle CLI on)
- type `ux` (filter resets/updates)
- `Space` (toggle UX on)
- `Enter` (save and close)

This replaces the current row-add / resolve / save workflow.

## 4. Scope and Non-Goals

### In Scope

- Replace or wrap current `CategoryDirectEdit` UX for board cell editing.
- Improve keymap and footer hints.
- Preserve inline child-category creation under the active column parent.
- Maintain validation for reserved names and exclusivity.
- Add tests for both exclusive and non-exclusive editing flows.

### Out of Scope (for this feature)

- Full category manager redesign (`Mode::CategoryManager`)
- Schema changes in `agenda-core`
- “When” column date editing redesign
- Bulk multi-item category editing

## 5. Delivery Strategy (Phased)

This feature should be implemented in two deliverable slices to reduce risk.

### Phase 1: Quick Wins on Current Modal (Low Risk, Immediate Relief)

Goal: improve current `CategoryDirectEdit` usability without changing the draft/row data model yet.

Changes:

- Accept lowercase `s` in addition to uppercase `S` for save.
- Add direct “new row” action from `Input` focus (no need to `Shift-Tab` to `Entries`).
- Make `Tab` behavior less surprising:
  - preferred: always cycle focus
  - move autocomplete to a dedicated key (for example `Ctrl-Space`) or `Right`
- Improve footer/help text with explicit common-path guidance:
  - “Enter resolves row”
  - “s saves”
  - “a adds row (from input)”
- Improve status messages after row resolve to clarify next action:
  - “Resolved to 'CLI'. Add another row or press s to save.”

Acceptance criteria:

- Two-category assignment in non-exclusive column can be completed without entering `Entries` focus.
- Save works with `s` and `S`.
- Footer hints match actual keys.
- Existing tests still pass; new key-routing tests added.

### Phase 2: Replace Row-Based UX with Picker-Based UX (Main Redesign)

Goal: make category editing match user intent (single-select vs multi-select) and remove row management from the common path.

Changes:

- Introduce column-aware picker mode(s), reusing the same entry path (`Enter` on category cell).
- Exclusive parent columns:
  - single-select picker
  - selecting an option updates the item and exits (or stages and save-on-enter)
- Non-exclusive parent columns:
  - multi-select picker with checkmarks/toggles
  - staged selection list + explicit save on `Enter`
- Integrated create-child flow:
  - when filter has no exact match, offer “Create '<typed>' under <Parent>”
- Visual cues:
  - selected values clearly marked in suggestions/list
  - concise header showing item + column + parent + exclusive/non-exclusive

Acceptance criteria:

- Multi-category assignment no longer requires row creation.
- Keystrokes for adding two categories are materially reduced.
- Exclusive columns prevent multi-selection by interaction design, not just save-time validation.
- Create-category flow works in both picker types and respects reserved-name constraints.

### Phase 3: Polish and Migration Cleanup

Goal: remove confusion and dead UI affordances after the redesign.

Changes:

- Remove or retire row-specific footer language if row UI is no longer primary.
- Tighten status/error messaging around duplicates/exclusivity/create flow.
- Add/update manual smoke script for category editing workflow.
- Document keybindings in TUI docs/spec notes.

Acceptance criteria:

- Footer/status copy is consistent with final UX.
- Smoke test script covers:
  - exclusive column edit
  - non-exclusive multi-select edit
  - create-and-assign child category
  - cancel flow

## 6. Technical Design Plan (Implementation Detail)

### 6.1 Entry Routing

Current entry point is `Enter` on a non-item column cell in normal board mode.

Plan:

- Keep the same entry key and routing point in `/Users/mds/src/aglet/crates/agenda-tui/src/modes/board.rs`.
- Dispatch to editor variant based on parent category exclusivity.

### 6.2 State Model (Proposed)

Add a picker-specific draft state (name TBD), for example:

- anchor/item/parent metadata (reuse current direct-edit anchor/meta shape)
- focus (`FilterInput`, `List`)
- filter text buffer
- list cursor
- selected IDs set (for multi-select) or selected ID (for single-select)
- create-confirm state

Notes:

- Phase 1 can keep `CategoryDirectEditState` intact.
- Phase 2 can either:
  - replace `CategoryDirectEditState`, or
  - introduce a new mode/state and leave old mode as fallback during transition.

Preferred approach:

- Introduce a new picker mode/state and keep current mode temporarily until tests pass and behavior is stable.

### 6.3 Rendering

Update `/Users/mds/src/aglet/crates/agenda-tui/src/render/mod.rs`:

- Render a simpler two-region layout for picker:
  - filter input
  - filtered category list (with selected markers)
- Footer hints are mode-specific and minimal.
- Show explicit selected markers:
  - `[x]` selected (multi)
  - `(•)` selected (single), or equivalent ASCII-safe marker

### 6.4 Data Application

Reuse existing assign/unassign logic patterns from `apply_category_direct_edit_draft` where possible:

- compute desired IDs under current parent
- diff against current IDs
- call `assign_item_manual` / `unassign_item_manual`

Important:

- Preserve source strings (or update intentionally) for audit/provenance consistency.
- Keep non-target-parent assignments untouched.

## 7. Testing Plan

### Unit Tests (TUI)

Add/extend tests for:

- entering editor from board cell
- non-exclusive multi-select toggle flow
- exclusive single-select flow
- save/cancel semantics
- lowercase `s` save (Phase 1)
- create-category flow from picker
- reserved-name rejection (`Done`, `When`, `Entry`)
- duplicate-prevention behavior (where applicable)

### Regression Tests

- ensure item-column `Enter` still opens item edit (not category picker)
- ensure `When` column still shows existing “not implemented inline” message
- ensure existing category manager and board navigation keybindings are unchanged

### Manual Smoke Script Update

Add a focused script in `docs/` covering:

- Assign two `Area` categories to one item
- Replace `Status` with a new value (exclusive)
- Create a new `Area` child in-place and assign it
- Cancel mid-edit without persisting changes

## 8. Risks and Mitigations

### Risk: Keybinding collisions / muscle memory breakage

Mitigation:

- Phase 1 first (incremental improvements)
- keep common keys (`Enter`, `Esc`, arrows/jk`)
- clearly update footer hints

### Risk: Regression in assign/unassign logic

Mitigation:

- reuse diff/apply logic rather than rewriting persistence paths
- add tests around unchanged assignments outside the edited parent

### Risk: Create flow complexity in picker

Mitigation:

- keep existing confirm behavior pattern
- gate Phase 2 merge on create/cancel test coverage

## 9. Open Decisions for Review (Need Your Signoff)

1. Should Phase 1 ship first, or go directly to the picker redesign?
2. For non-exclusive picker, should `Enter` save immediately, or should `Enter` toggle and `s` save?
   - Recommendation: `Space` toggles, `Enter` saves (faster and clearer)
3. For exclusive picker, should selection auto-save on `Enter` (single action) or require explicit save?
   - Recommendation: `Enter` selects and closes
4. Should we keep the old row-based editor behind a fallback key during rollout?
   - Recommendation: no fallback for end users; keep temporary internal code path only until tests are stable

## 10. Proposed Implementation Order (After Approval)

1. Phase 1 quick wins (small patch, tests)
2. Phase 2 picker state + render for non-exclusive columns
3. Phase 2 exclusive picker variant and create flow
4. Phase 3 polish, docs, smoke script updates

## 11. Detailed TODO List (Execution Checklist)

Legend:

- `[ ]` pending
- `[~]` active
- `[x]` complete

### Phase 1 - Quick Wins on Current `CategoryDirectEdit`

#### 11.1 Baseline and Safety Checks

- [x] Re-read current `CategoryDirectEdit` key routing in `/Users/mds/src/aglet/crates/agenda-tui/src/modes/board.rs` and confirm current behavior matches plan assumptions.
- [x] Inventory all footer/status strings for `Mode::CategoryDirectEdit` in `/Users/mds/src/aglet/crates/agenda-tui/src/render/mod.rs` and `/Users/mds/src/aglet/crates/agenda-tui/src/modes/board.rs`.
- [x] Identify existing unit tests covering `CategoryDirectEdit` and note gaps (save key variants, add-row-from-input, Tab behavior).

#### 11.2 Keymap Improvements (Current Modal)

- [x] Add lowercase `s` handling as alias for save in `handle_category_direct_edit_key`.
- [x] Confirm uppercase `S` behavior is unchanged.
- [x] Add “add row” action from `Input` focus (implemented as `+`) while preserving existing `Entries`-focus behavior.
- [x] Ensure add-row action is blocked with clear status message for exclusive parents.
- [x] Decide and implement `Tab` behavior change for suggestions (per approved option):
  - [x] Option A: `Tab` always cycles focus
  - [ ] Option B: keep autocomplete on `Tab` and add alternative focus key
- [x] If autocomplete key changes, add new key handler (implemented `Right`) and ensure it only applies in `Suggestions`.
- [x] Preserve `Shift-Tab` focus-back behavior.

#### 11.3 Messaging and Footer Hint Improvements

- [x] Update `CategoryDirectEdit` footer hint text to match actual keys after Phase 1 changes.
- [x] Make save hint show `s/S` if both are supported.
- [x] Clarify row resolve status message to explicitly instruct next action (“add another row or save”).
- [x] Review empty-row `Enter` messages for clarity and consistency with updated workflow.
- [x] Review cancel/save success messages so they distinguish “draft resolved” vs “saved to item”. (No code change needed; already distinct.)

#### 11.4 Phase 1 Tests

- [x] Add/extend test: lowercase `s` saves draft successfully.
- [x] Add/extend test: add row from `Input` focus works in non-exclusive parent.
- [x] Add/extend test: add row from `Input` focus is rejected in exclusive parent with status message.
- [x] Add/extend test: revised `Tab` behavior (cycling vs autocomplete) matches approved design.
- [x] Add/extend test: legacy `Entries`-focus row add/remove still works (if retained).
- [x] Run targeted `agenda-tui` tests for category direct edit flows.

#### 11.5 Phase 1 Manual Verification

- [x] Manual verify two-category assignment in `Area` can be completed without entering `Entries` focus. (Non-destructive spot-check of `+` row-add from `Input` in TUI; save/apply path covered by unit tests.)
- [x] Manual verify `Status`/`Priority` still enforce exclusivity. (Spot-checked `+` in `Status` direct edit; exclusive-row status shown.)
- [x] Manual verify existing create-category inline flow still works in current modal. (Spot-checked create-confirm open/cancel in direct edit; no save.)

### Phase 2 - Picker-Based Redesign (Main UX Replacement)

#### 11.6 UX Contract Finalization (Before Code)

- [ ] Lock approved keymap for non-exclusive picker (`Space` toggle, `Enter` save, etc.).
- [ ] Lock approved keymap for exclusive picker (`Enter` select+close vs select+save).
- [ ] Lock create flow interaction details:
  - [ ] how create is presented in list
  - [ ] confirm/cancel keys
  - [ ] post-create behavior (auto-select/auto-save vs remain open)
- [ ] Define footer hints for both picker variants (minimal and mode-specific).

#### 11.7 New Mode/State Plumbing

- [x] Add new mode enum variant(s) for picker-based category editing in `/Users/mds/src/aglet/crates/agenda-tui/src/lib.rs` (preferred over replacing row mode in one step).
- [x] Add picker state struct(s) in `/Users/mds/src/aglet/crates/agenda-tui/src/lib.rs`:
  - [x] metadata (anchor/item/parent)
  - [x] filter input buffer
  - [x] focus state
  - [x] list cursor
  - [x] selected IDs / selected ID
  - [x] create-confirm state
- [x] Add helpers to initialize picker state from current item assignments under the active parent.
- [x] Add helpers to compute filtered matches scoped to the current parent’s child categories.
- [ ] Add helper(s) to detect exact match, selected item presence, and create eligibility.

#### 11.8 Entry Routing and Mode Dispatch

- [x] Update `Enter` handling from board cell edit entry path to open picker mode instead of row-based direct edit (behind a temporary internal switch if needed).
- [x] Preserve item-column `Enter` path to item edit.
- [x] Preserve `When` column special-case messaging (no picker yet).
- [x] Add status message on picker open that matches final UX.
- [x] Ensure mode cleanup on cancel/save restores normal mode, selection, and column position.

#### 11.9 Input Handling - Non-Exclusive Multi-Select Picker

- [x] Implement typing/editing in filter input and live suggestion refresh.
- [x] Implement list navigation (`j/k`, arrows).
- [x] Implement `Space` to toggle highlighted category on/off.
- [x] Implement visual/list cursor clamping on filter result changes.
- [x] Implement duplicate-safe toggling semantics (toggling same category twice returns to previous state cleanly).
- [x] Implement `Enter` to save and close (per approved design).
- [x] Implement `Esc` cancel without persisting changes.
- [ ] Implement optional quick-clear behavior for filter input (if desired) and document key.

#### 11.10 Input Handling - Exclusive Single-Select Picker

- [x] Reuse picker layout and filtering behavior for exclusive parent columns.
- [x] Implement single-selection behavior (radio semantics).
- [x] Prevent multiple selected values in interaction state.
- [x] Implement `Enter` behavior per approved design (select+close or select then save). (Current behavior: `Space` selects, `Enter` saves.)
- [x] Ensure replacing existing selection removes prior assignment under the same parent.
- [x] Ensure cancel restores original assignment untouched.

#### 11.11 Inline Create Flow (Picker Mode)

- [x] Add create affordance when typed filter has no exact match (and input is non-empty). (Status-driven affordance via `Enter`; list-row affordance still pending.)
- [x] Reuse reserved-name validation (`Done`, `When`, `Entry`) in picker create path.
- [x] Reuse/extract inline create confirm key handling pattern for picker mode.
- [x] Create new child category under the current column parent.
- [x] Refresh category cache after create.
- [ ] Apply post-create behavior per approved UX:
  - [x] exclusive picker: new category becomes selected
  - [x] non-exclusive picker: new category toggled on
- [x] Confirm create cancel returns user to picker with filter preserved (or explicitly cleared, per chosen design). (Preserves typed filter in current implementation.)

#### 11.12 Render Layer - Picker UI

- [x] Add picker rendering branch in `/Users/mds/src/aglet/crates/agenda-tui/src/render/mod.rs`.
- [x] Render compact header with item label, column/parent name, and exclusivity indicator.
- [x] Render filter input panel.
- [ ] Render filtered list with selected markers:
  - [x] ASCII-safe selected marker for multi-select
  - [x] ASCII-safe selected marker for single-select
- [ ] Render create affordance row (if applicable).
- [x] Render clear footer hints for picker mode.
- [x] Ensure focus styling is clear if picker uses multiple focus regions.
- [ ] Verify layout works on narrower terminal widths/heights (no truncation crashes or unusable footers).

#### 11.13 Apply Logic / Persistence

- [x] Implement picker save path using diff-based assign/unassign under the target parent.
- [ ] Reuse logic from `apply_category_direct_edit_draft` where possible instead of duplicating behavior.
- [x] Ensure assignments outside the edited parent are preserved.
- [x] Ensure assignment source strings are set consistently for picker saves.
- [x] Refresh board state after save and restore user selection/column position.
- [x] Emit clear success status message identifying saved column edits.

#### 11.14 Transition and Fallback Handling

- [x] Keep legacy row-based direct edit code path temporarily available internally during Phase 2 implementation.
- [x] Update routing/tests to default to picker mode after feature is stable. (Non-exclusive columns now route to picker; exclusive columns still use direct edit.)
- [ ] Remove temporary fallback branching once Phase 2 tests pass (or defer removal to Phase 3 if safer).

#### 11.15 Phase 2 Tests (Unit/Regression)

- [x] Add test: opening picker from non-item category column enters correct mode/state.
- [x] Add test: non-exclusive picker initializes with current selections checked.
- [ ] Add test: multi-select toggle on/off updates staged selection only (before save).
- [x] Add test: `Enter` saves multi-select diff correctly (assign + unassign).
- [x] Add test: `Esc` cancels multi-select changes.
- [x] Add test: exclusive picker initializes with current single selection. (Routed to existing direct-edit mode for now.)
- [x] Add test: exclusive picker replaces selection correctly.
- [x] Add test: exclusive picker cannot stage multiple selections.
- [x] Add test: create-child flow from picker creates under correct parent and selects/toggles new child.
- [x] Add test: reserved-name create rejection in picker mode.
- [x] Add test: item-column `Enter` still opens item editor.
- [x] Add test: `When` column path still shows existing “not implemented inline” status.
- [ ] Run targeted `agenda-tui` test subset and then full `agenda-tui` tests. (Targeted subsets done for new picker, direct-edit, and board-add-column; full suite pending.)

#### 11.16 Phase 2 Manual Verification

- [ ] Manual verify `Area` multi-select flow for two categories matches approved keystrokes.
- [ ] Manual verify `Status` exclusive replacement flow works with minimal keystrokes.
- [ ] Manual verify create-new-child in `Area` then save.
- [ ] Manual verify cancel from picker preserves original assignments.
- [ ] Manual verify keyboard hints match actual behavior on-screen.

### Phase 3 - Polish, Cleanup, Docs, and Smoke Coverage

#### 11.17 Code Cleanup and Consistency

- [ ] Remove obsolete row-specific help/status strings if row editor is no longer user-facing.
- [ ] Remove deprecated key handling branches no longer used by default.
- [ ] Consolidate duplicated helper logic between old and new flows (filtering, create validation, save diff).
- [ ] Re-run formatting and ensure no dead-code warnings/errors are introduced.

#### 11.18 UX Copy and Hint Audit

- [ ] Audit all status messages for category editing modes for consistency in terminology (“select”, “toggle”, “save”, “cancel”).
- [ ] Ensure exclusive/non-exclusive language is user-facing only where it helps and is not noisy.
- [ ] Ensure footer hints are concise and do not mention unavailable actions.

#### 11.19 Documentation Updates

- [ ] Update or add TUI workflow documentation describing new category editing flow.
- [ ] Add exact keystroke examples for:
  - [ ] non-exclusive multi-select (`Area`)
  - [ ] exclusive single-select (`Status`/`Priority`)
  - [ ] create-and-assign child category
- [ ] Update any spec notes that currently describe row-based direct edit as the primary flow.
- [ ] If implementation differs from spec, note the divergence in the relevant spec/implementation doc.

#### 11.20 Manual Smoke Script

- [ ] Create or update a focused manual smoke script in `/Users/mds/src/aglet/docs/` for category editing.
- [ ] Include setup steps (sample categories/items) if needed.
- [ ] Include pass/fail checks for save/cancel behavior and persistence after refresh/restart.

#### 11.21 Final Verification

- [ ] Run full `cargo test -p agenda-tui` (and any impacted workspace tests if necessary).
- [ ] Perform one end-to-end manual walkthrough on `feature-requests.ag` (or disposable test DB) covering all key flows.
- [ ] Capture any surprising behavior and, if found, add a note to `/Users/mds/src/aglet/AGENTS.md` per repo instructions.
- [ ] Prepare a concise implementation summary and follow-up items (if any) for review.

No implementation work should start until this plan is approved.
