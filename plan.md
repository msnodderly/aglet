# Multi-Entry Column Category Editor (Modern TUI Reimagining)

## Goal

Implement a modern, laptop-friendly, typing-first column editor for `Mode::CategoryDirectEdit`
that supports multiple category assignments in a single column.

This is a reimagining of Lotus Agenda's column-entry workflow, not a 1:1 clone:

- Keep the core idea: column editing is fast, spatial, and typeahead-driven
- Avoid legacy F-key / INS / DEL dependencies
- Use terminal-friendly keys and a clear, discoverable modal/pane UI

The editor should:

- Support one or many category assignments under the current column heading
- Make adding another category low-friction
- Keep "create new category" confirmation inside the same picker/editor
- Preserve a fast type-to-narrow workflow
- Use a draft/apply model so `Esc` cancels the whole edit cleanly

This plan also includes two adjacent UX/features that should share the same
design language and category-picking primitives:

- Configurable **single-line vs multi-line item rendering** in board views
  (view-level default with optional section override)
- **Add column left/right of current column** using a modern typeahead workflow
  (Lotus-inspired, not a 1:1 keybinding clone)


## Background / Context

### What Lotus Agenda got right (relevant behavior)

Lotus column editing was not checkbox-first. It was:

- Cursor into a column entry
- Type category name directly
- Auto-complete / choice assistance available
- Multiple entries in same column via an explicit "insert another entry" action

That suggests our primary UX should remain:

- **typing-first**
- **column-cell focused**
- **multi-entry capable**

Checkbox-style hierarchy toggling should remain a separate item-level workflow
(similar to the existing `ItemAssignPicker` / assignment profile behavior).

## Companion Docs

- `decisions.md`: accepted decisions and defaults that this plan assumes during implementation
- `questions.md`: open questions (and resolved history) to review before starting new phases

When `plan.md` and `decisions.md` differ, treat `decisions.md` as the current
source of truth for confirmed choices and update `plan.md` accordingly.

### Why the current `CategoryDirectEdit` is insufficient

The current UI (recently improved) is still single-entry in practice:

- It displays suggestions and supports inline create confirm
- But apply logic replaces sibling assignments in the column
- So it cannot intentionally retain multiple child assignments under the same parent


## UX Direction (Proposed)

## Summary

Use a **single modal editor** with:

- a list of current entries (one line per category)
- an active edit line (typing-first)
- a suggestion list for the active line
- inline create confirmation (same modal)
- explicit apply/cancel for the whole draft

This preserves speed and clarity while enabling multi-entry editing.

### Conceptual layout

1. Context line
   - Column heading + item label
2. Entries list ("Assigned in this column")
   - One row per category assignment in the current column
   - One active row (editable)
3. Input line (for active row)
   - `Category> ...`
4. Suggestions list (for active row)
   - Starts full, narrows as you type
5. Help / action hints
6. Inline create confirmation (replaces suggestions area while active)

### Why this layout

- Keeps the "one line per category" mental model
- Preserves typeahead speed
- Makes multiple assignments obvious and editable
- Avoids comma-separated parsing UX complexity
- Avoids turning column editing into a checkbox picker


## Interaction Model (Modern Mac/Terminal Friendly)

Avoid F-keys and insert/delete key assumptions. Prefer keys that work in common macOS terminals.

### Core navigation

- `Up` / `Down` and `j` / `k`: move selection in suggestions or entries (depending on focus)
- `Tab` / `Shift-Tab`: cycle focus between regions (`Entries`, `Input`, `Suggestions`)
- `Esc`: cancel editor (discard all draft changes) OR cancel inline create confirm
- `S`: apply all draft changes and close editor

### Entry editing (active row)

- Type text: edits the active entry's text buffer
- `Enter`:
  - If input is empty: clear/remove the active entry line (or noop if it's the only blank draft row)
  - If exact match exists: set active entry to that category
  - If highlighted suggestion exists and no exact match: use highlighted suggestion
  - If no match: open inline create confirmation in the same modal
- `Tab` (while suggestions focused or visible): copy highlighted suggestion into active line

### Multi-entry actions (laptop-friendly replacements for INS/DEL)

- `n`: add a new entry line (focus input on new line)
- `x`: remove current entry line from the draft
- `a`: quick-add another blank entry line (alias for `n`)
- `Backspace` on an empty active line:
  - If more than one line exists, optionally remove line (nice-to-have)

### Inline create confirmation (same picker)

When user presses `Enter` with unknown text:

- Editor remains open
- Show confirmation panel in-place:
  - `Enter` / `y`: create category under current column heading and set active entry
  - `n` / `Esc`: cancel create and return to editing

### Apply vs cancel semantics

- `S`: commit the full set of column-child assignments from draft to backend
- `Esc`: cancel all changes made during this edit session

This is intentionally different from the current immediate-write behavior.


## Behavior Details

### Single-value columns vs multi-value columns

Column behavior depends on the parent category:

- If parent category is **exclusive**:
  - Editor may still use the same UI
  - But only one entry is allowed
  - Adding a second entry should be **blocked immediately** with a clear status message
  - Do not auto-replace implicitly
  - Do not defer the error until save/apply

- If parent category is **non-exclusive**:
  - Multiple entries are allowed
  - Draft and apply support arbitrary count

### Suggestions behavior

For the active row:

- Empty input -> show full valid child list (excluding special `When`)
- Typed input -> narrowed suggestions (current substring behavior is acceptable initially)
- Exact typed match should take precedence over highlighted suggestion on `Enter`

### Empty category / removing categories

We previously bound empty `Enter` to "clear column value".
With multi-entry support:

- Empty `Enter` should operate on the **active row**, not auto-select suggestion row
- Empty `Enter` removes the active row if multiple rows exist
- Empty `Enter` keeps a single blank row if it is the only row
- `x` should remove the current row explicitly
- `S` commits resulting entry list (including zero entries => clear all values in that column)


## State Model Changes (Recommended)

The current implementation overloads global fields (`self.input`, `category_suggest`, etc.).
For multi-entry editing, add a dedicated draft state.

### New state struct (conceptual)

In `lib.rs`, introduce something like:

```rust
#[derive(Clone, Debug)]
struct CategoryDirectEditState {
    parent_id: CategoryId,
    parent_name: String,
    item_id: ItemId,
    item_label: String,

    // Draft entries, one row per column-child assignment.
    rows: Vec<CategoryDirectEditRow>,
    active_row: usize,

    // Region focus for keyboard routing.
    focus: CategoryDirectEditFocus,

    // Suggestion cursor for the active row.
    suggest_index: usize,

    // Inline create-confirm flow.
    create_confirm_name: Option<String>,
}

#[derive(Clone, Debug)]
struct CategoryDirectEditRow {
    // Draft text user is editing for this row.
    input: text_buffer::TextBuffer,

    // Resolved category if selected/matched, if any.
    category_id: Option<CategoryId>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum CategoryDirectEditFocus {
    Entries,
    Input,
    Suggestions,
}
```

### App struct integration

Add:

```rust
category_direct_edit: Option<CategoryDirectEditState>,
```

This should replace the current ad hoc direct-edit helper state over time:

- `category_suggest`
- `category_direct_edit_create_confirm`
- reliance on shared `self.input` for direct-edit

### Why dedicated state is important

- Multiple rows each need their own draft text
- `Esc` needs clean draft cancellation
- Focus routing becomes much clearer
- Easier to reuse later for "choose column heading category"


## Implementation Plan (Phased)

### Phase 0: Prep / refactor (state extraction)

Goal: Introduce dedicated `CategoryDirectEditState` without changing behavior yet.

Tasks:

- Add `category_direct_edit: Option<CategoryDirectEditState>` to `App`
- Populate it when entering `Mode::CategoryDirectEdit`
- Move direct-edit render/input logic to read from the new state
- Preserve current single-entry behavior temporarily

Acceptance:

- Existing direct-edit workflow still works
- No regressions in add/edit item or item assignment picker

### Phase 1: Multi-entry draft model

Goal: Represent multiple child assignments in the draft.

Tasks:

- Initialize draft rows from current item's assignments that are children of the column heading
- If none exist, create one blank editable row
- Add helpers:
  - `current_direct_edit_state()`
  - `active_direct_edit_row()`
  - `active_direct_edit_suggestions()`
  - `add_direct_edit_row()`
  - `remove_direct_edit_row()`
- Keep active row index clamped

Acceptance:

- Opening a column with multiple child assignments shows multiple rows
- Opening an empty column shows one blank row

### Phase 2: Keyboard model for multi-entry editing

Goal: Add modern, laptop-friendly multi-entry interactions.

Tasks:

- Implement region focus (`Entries`, `Input`, `Suggestions`)
- Add keybindings:
  - `n` / `a` add row
  - `x` remove row
  - `Tab` / `Shift-Tab` cycle region focus
  - `Up` / `Down` and `j` / `k` navigation
  - `Enter` semantics for active row
  - `S` apply
  - `Esc` cancel
- Keep exact typed match precedence on `Enter`
- Keep inline create confirm in same modal

Acceptance:

- User can add multiple rows and navigate between them
- Empty `Enter` does not accidentally apply first suggestion
- `Esc` cancels entire draft

### Phase 3: Commit logic (draft -> backend assignments)

Goal: Apply the draft set to backend correctly and predictably.

Tasks:

- Build `desired_child_ids` from resolved draft rows
- Compare against `current_child_ids_assigned`
- Unassign removed child assignments
- Assign added child assignments
- Preserve non-column assignments
- Respect exclusivity rules and show clear errors/status if invalid

Implementation note:

- Use `agenda.unassign_item_manual(...)` and `agenda.assign_item_manual(...)`
- Order matters for exclusivity and descendants; unassign removed values before assigning new ones

Acceptance:

- Can add, remove, and reorder draft rows without duplicating backend assignments
- Final displayed column value matches committed set (comma-joined in board)

### Phase 4: Rendering polish (TUI quality)

Goal: Make the multi-entry editor feel fast and readable.

Tasks:

- Render clear sections:
  - context
  - assigned rows
  - active input
  - suggested categories
  - help
- Highlight active row and focused region distinctly
- Use shared muted text color constant (already introduced in `render/mod.rs`)
- Preserve inline create-confirm render inside same modal
- Show explicit draft actions (`S apply`, `Esc cancel`, `n add`, `x remove`)

Acceptance:

- User can understand current state without reading status line only
- Modal remains readable on smaller terminal sizes

### Phase 5: Optional refinements (recommended after core works)

- Backspace-on-empty-row removes row
- Reorder rows (`[` / `]`) if row order matters visually
- Duplicate-row prevention in draft
- Better matching/ranking (prefix-first or fuzzy later)
- "Promote exact match to top" in suggestions
- Explicit `[Apply]` / `[Cancel]` buttons in modal footer

### Phase 6: Multi-line board rendering (view/section configurable)

Goal: Support optional multi-line display in board sections while preserving
current single-line behavior as the default.

#### UX requirements

- `single-line` mode (existing behavior)
  - item text is truncated
  - column category values are comma-separated
  - overflow shows ellipsis
- `multi-line` mode (new)
  - item text can wrap across multiple lines
  - category column values render one category per line
  - cap visible category lines per cell/row at `8` and summarize overflow
    (e.g. `+3 more` / `+N more`)
- configurable at:
  - view level (default for all sections)
  - section level (optional override)

#### Tasks

- Add display-mode fields to view/section models (view default + section override)
- Update serialization/storage round-trips
- Update board row rendering to support variable row height
- In multi-line mode:
  - wrap item text
  - render category values as line list
  - cap category lines (configurable constant to start)
- Preserve row selection, scrolling, and focused-cell highlighting
- Add view-edit controls for toggling display mode (view default + section override)

#### Acceptance

- Existing views render exactly as before unless mode is enabled
- Multi-line mode makes category-heavy rows readable
- Row height and scrolling remain stable in both modes

### Phase 7: Add column left/right of current column (modern reimagining)

Goal: Add a low-friction workflow to insert a category-based column to the left
or right of the current column using typeahead.

This is inspired by Lotus Agenda's ALT-L / ALT-R column insertion flow, but
modernized for macOS/laptop terminal usage.

#### Proposed workflow

1. User places cursor on a column (header or a cell within a column)
2. Trigger add-column-left or add-column-right command
3. Open a typeahead category picker/editor (same design language as direct-edit)
4. Type to narrow and choose/create category
5. Confirm insertion
6. Column is inserted left/right of current column in the relevant scope

#### Scope decision (initial release)

Start with **current section only** insertion (confirmed) to reduce ambiguity and
avoid surprising view-wide structural edits. View-wide insertion can be a follow-up.

#### Keybindings (modern/laptop-friendly)

Target (after modifier support):

- `Ctrl-L`: add column to the left of current column
- `Ctrl-R`: add column to the right of current column

Fallback (until modifier support is implemented):

- `[` / `]` are already used in board mode; avoid collisions
- temporary text commands are acceptable (e.g. `H`/`L`, `,`/`.` in a dedicated
  "column edit" submode), but the preferred end state is `Ctrl-L` / `Ctrl-R`

#### Tasks

- Add input-event plumbing so handlers can see modifiers (`KeyEvent`, not only `KeyCode`)
- Add board-mode commands for "insert column left/right"
- Capture insertion anchor (section + current column index)
- Reuse a category typeahead picker for selecting the new column heading category
- Support creating a new category inline from the picker
- Insert `Column { heading, kind: Standard, ... }` at target index
- Refresh board and keep selection near the inserted column
- Provide clear status messages and cancel behavior

#### Acceptance

- User can add a column left/right without leaving the board
- Typeahead create/select flow is consistent with direct category editing
- Column insertion updates the active section and persists through refresh/restart

### Phase 8: Shared category-picker primitives (cleanup + reuse)

Goal: Reduce duplicated typeahead/picker logic so direct-edit and add-column
workflows remain consistent.

#### Tasks

- Extract shared category suggestion/filtering helpers for scoped category sets
- Extract shared inline create-confirm UI/logic where practical
- Standardize copy, muted text styles, and help messaging across pickers

#### Acceptance

- Direct-edit and add-column flows feel consistent
- New picker features can be added in one place with minimal duplication


## Keybinding Proposal (Draft)

This is the recommended initial mapping (modern terminal-friendly):

### Global (inside multi-entry column editor)

- `S`: apply draft and close
- `Esc`: cancel editor (discard draft)

### Row management

- `n` / `a`: add new row
- `x`: remove active row

### Focus and navigation

- `Tab` / `Shift-Tab`: cycle focus (`Entries` -> `Input` -> `Suggestions`)
- `Up` / `Down`: navigate in focused region
- `j` / `k`: aliases for `Up` / `Down`

### Active row input

- Type: edit active row text
- `Enter`: resolve/apply to row (exact match > highlighted suggestion > create confirm)
- `Backspace`: normal editing; optional row-delete if empty (later)

### Suggestions

- `Tab`: copy highlighted suggestion into active row input
- `Enter`: same as input `Enter` when suggestions focused

### Inline create confirm

- `Enter` / `y`: confirm create and set active row
- `n` / `Esc`: cancel create


## File-by-File Implementation Targets

### `crates/agenda-tui/src/lib.rs`

- Add `CategoryDirectEditState` and related structs/enums
- Add `category_direct_edit: Option<...>` field on `App`
- Initialize in `Default`
- Add state for add-column workflow (anchor + direction + picker draft), if kept in `App`

### `crates/agenda-tui/src/modes/board.rs`

- Rework `open_category_direct_edit` to build draft state from current assignments
- Replace single-entry direct-edit handlers with multi-entry draft handlers
- Add apply/cancel logic for full draft
- Add row add/remove/focus/suggestion helpers
- Keep inline create confirm in this mode (do not bounce to `Mode::CategoryCreateConfirm`)
- Add board commands for insert-column-left/right
- Add modifier-aware command handling once `KeyEvent` plumbing lands
- Apply column insertion into current section and maintain selection

### `crates/agenda-tui/src/render/mod.rs`

- Render multi-entry direct-edit modal
- Render entries list + active input + suggestions + help
- Render inline create confirm panel in same modal
- Ensure cursor placement tracks active row input
- Render add-column picker modal (or shared picker UI)
- Support multi-line board row/cell rendering (variable row heights)

### `crates/agenda-tui/src/ui_support.rs` (optional/minor)

- Reuse existing `filter_child_categories`
- Optionally add ranking helper (prefix-first ordering) later
- Add helpers for multi-line category cell formatting (line-per-category, overflow summary)

### `crates/agenda-core/src/model.rs`

- Add view/section display-mode configuration fields (view default + section override)
- Ensure defaults preserve current single-line behavior

### `crates/agenda-core/src/store.rs`

- Persist and load new display-mode fields for view/section config
- Add/adjust round-trip tests for new fields

### `crates/agenda-tui/src/app.rs` and input dispatch

- Preserve `KeyEvent` modifiers through dispatch (required for `Ctrl-L` / `Ctrl-R`)
- Update handler signatures as needed (`KeyCode` -> `KeyEvent` or equivalent abstraction)


## Data / Backend Notes

- **Multi-entry column editing:** no schema changes required (backend already supports multiple assignments)
- **Multi-line board display config:** likely requires model/storage changes (view default + section override fields)
- Backend already supports multiple assignments
- UI must stop forcing single-value replacement for non-exclusive parents
- For exclusive parents, backend enforcement remains source of truth
- Column insertion reuses existing view/section column structures (no new column schema required)


## Testing Plan

## Manual test scenarios (core)

1. Open column with one existing category -> row list shows one row
2. Add second category (`n`, type, `Enter`) -> draft shows two rows
3. `S` apply -> board column shows both values
4. Reopen -> both values load back into rows
5. Remove one row (`x`) -> `S` apply -> board updates correctly
6. Empty all rows / remove all -> `S` apply -> column clears
7. Unknown name -> `Enter` -> inline create confirm -> `Enter` creates and assigns
8. Unknown name -> `Enter` -> `Esc` cancels create and stays in editor
9. `Esc` from editor discards all draft changes
10. Exact typed match wins over highlighted suggestion

## Manual test scenarios (multi-line board rendering)

1. View in `single-line` mode renders exactly like current behavior
2. Enable view-level `multi-line` mode -> item text wraps and category values show one-per-line
3. Section override to `single-line` within a `multi-line` view works
4. Section override to `multi-line` within a `single-line` view works
5. Long category lists cap at max visible lines and show overflow summary (`+N more`)
6. Selection/highlight and scrolling remain correct with tall rows

## Manual test scenarios (add column left/right)

1. Trigger add-column-left on a selected column -> picker opens anchored to current context
2. Type existing category and confirm -> column inserted left of current column
3. Type unknown category and confirm inline create -> new category created + column inserted
4. Trigger add-column-right -> inserts on correct side
5. Cancel from picker -> no structural change
6. Reopen app/view -> inserted column persists

## Manual test scenarios (edge cases)

1. Parent category is exclusive -> adding second row is blocked clearly
2. Duplicate row attempts do not create duplicate backend assignment
3. Reserved names (`When`, `Done`, `Entry`) cannot be created as new child categories
4. Long suggestion list scrolls correctly
5. Small terminal window still renders usable modal (degrades gracefully)

## Automated tests (recommended additions)

- Draft initialization from current column assignments
- Add/remove row helpers
- Exact-match precedence behavior
- Apply diff logic (added/removed/unchanged child assignments)
- Inline create confirm state transitions
- Empty apply clears column child assignments
- View/section display-mode serialization round-trip
- Multi-line formatting helper outputs (`single-line`, `multi-line`, overflow summary)
- Column insertion helper inserts at correct index left/right
- Add-column picker cancel/confirm state transitions


## Non-Goals (For This Plan)

- Full inline spreadsheet editing directly in the board cell (no modal)
- Replacing the item-level assignment picker (`ItemAssignPicker`)
- Fuzzy matching / advanced ranking beyond current substring filtering
- View-wide column insertion policy customization (start with current-section insertion)


## Follow-on Opportunities

This plan should produce a reusable category selection/editing primitive that can later support:

- low-friction "add column heading" flows (`Ctrl-L` / `Ctrl-R`) with richer structure options
- section column configuration shortcuts
- reusable typeahead category selector component across view edit and board modes
- additional board presentation modes (compact, wrapped, density presets)


## Implementation Notes / Risk Areas

- The biggest risk is trying to layer multi-entry behavior on top of shared `self.input`.
  Mitigation: move to dedicated `CategoryDirectEditState` early (Phase 0).
- `Esc` semantics can become confusing if inline create confirm and editor-level cancel overlap.
  Mitigation: explicit precedence: create-confirm cancel first, editor cancel second.
- Exclusive-category behavior may be surprising.
  Mitigation: clear status/help text when second entry is blocked.
- Variable-height rows in a table can complicate scrolling/selection math.
  Mitigation: isolate multi-line formatting + row-height calculation behind helpers and add focused rendering tests.
- Modifier-key support (`Ctrl-L` / `Ctrl-R`) requires input dispatch refactor across modes.
  Mitigation: phase the `KeyEvent` plumbing first, then bind new commands.


## Phase Guide (Why + Expected End Result)

This section explains the purpose of each phase in plain language so the TODO
checklist is easier to follow while implementation is in progress.

The checklist answers "what to do." This section answers:

- Why this phase exists
- What problem it reduces
- What "done" should feel like before moving on

### Phase 0: CategoryDirectEdit State Extraction (No UX Change Yet)

#### Why

Right now, direct-edit behavior is spread across generic app fields (`self.input`,
ad hoc suggestion state, inline create flags). That was fine for a single-row
editor, but it becomes brittle once we support multiple rows, row-local text,
focus regions, and draft/cancel semantics.

If we skip this refactor and start layering behavior on top, we risk:

- confusing state bugs
- `Esc` cancel not restoring correctly
- row edits overwriting each other
- difficulty reusing the picker for column insertion later

#### Purpose

Create a dedicated container for all state that belongs to direct column editing.
This isolates the feature and makes later phases much safer.

#### Expected end result

- Direct-edit still works the same to the user (or nearly the same)
- Internally, state is centralized in `CategoryDirectEditState`
- Future phases can add rows/focus/create-confirm without touching unrelated app state

### Phase 1: Multi-Entry Draft Initialization + Data Helpers

#### Why

Before we can edit multiple values, we need a reliable in-memory "draft" model
that mirrors the actual current column assignments. This phase makes sure the
editor opens with the correct data and has basic row operations.

#### Purpose

Teach the editor what it is editing:

- which column parent category is in scope
- which child assignments are currently present on the item
- how many editable rows should appear initially

This phase establishes all row manipulation primitives (`add`, `remove`, `clamp`,
`ensure one row`) without committing to final keyboard UX yet.

#### Expected end result

- Opening direct-edit on an item with multiple column values shows multiple draft rows
- Opening direct-edit on an empty column shows one blank row
- Internal row helper functions are stable and testable

### Phase 2: Suggestions + Matching for Active Row

#### Why

The typing-first experience depends on suggestions behaving correctly for the
currently active row, not a global input buffer. If suggestions stay global,
multi-row editing will feel unpredictable.

#### Purpose

Move the suggestion system to be row-aware:

- suggestions follow the active row input
- exact typed match takes precedence
- empty input shows full scoped list

This preserves the good parts of the current picker while making it compatible
with multi-entry editing.

#### Expected end result

- Changing the active row changes the suggestion list accordingly
- Typing into a row narrows suggestions for that row
- `Enter` can safely interpret exact match vs highlighted suggestion

### Phase 3: Multi-Entry Keyboard Model (Draft Editing, Not Apply Yet)

#### Why

This is where the feature becomes usable, but we intentionally delay backend
writes. Separating "draft editing UX" from "commit to backend" reduces debugging
complexity and makes cancel behavior easier to reason about.

#### Purpose

Implement the modern terminal-friendly editing workflow:

- add/remove rows
- move between regions
- edit the active row
- resolve rows from typeahead
- avoid accidental actions (especially empty `Enter`)

#### Expected end result

- User can fully shape a draft of multiple values in the modal
- No backend changes are applied yet until an explicit save/apply step
- The editor feels coherent even before persistence is wired up

### Phase 4: Inline Create-Category Confirm in Multi-Entry Editor

#### Why

We already learned that bouncing out to a separate yes/no prompt breaks the flow.
For multi-entry editing, context-switching would be even more disruptive.

#### Purpose

Keep category creation inside the same modal so users can stay in the editing
context and continue building a multi-row draft.

#### Expected end result

- Unknown typed category opens an inline create confirmation panel
- Confirming creation resolves the active row and returns to editing flow
- Canceling creation stays in the editor without losing draft context

### Phase 5: Draft Apply / Cancel Semantics (Commit Multi-Entry Edits)

#### Why

Multi-entry support is not complete until the draft can be applied atomically
and canceled cleanly. This phase turns the editor from a UI demo into a real
editing workflow.

#### Purpose

Translate the draft rows into backend assignments:

- remove values the user deleted
- add values the user added
- preserve unrelated assignments
- respect exclusivity constraints

This phase also defines the final `S`/`Esc` semantics.

#### Expected end result

- `S` applies all draft changes for the column
- `Esc` discards all draft changes
- Board display matches committed backend state

### Phase 6: Multi-Entry Modal Rendering (TUI Polish)

#### Why

A multi-entry editor can be functionally correct but still hard to use if
visual hierarchy is unclear. TUI usability depends heavily on obvious focus,
selection, and action hints.

#### Purpose

Make the modal visually legible and consistent with the recently improved
single-entry picker:

- clear sections
- focused region cues
- active row highlighting
- explicit hints/buttons

#### Expected end result

- Users can tell what is selected, what is editable, and what `Enter` will do
- The modal remains usable on smaller terminal sizes

### Phase 7: Multi-Line Board Rendering Config (Model + Storage)

#### Why

The "multi-line item rendering" feature is not just a rendering tweak; it needs
a persisted setting so views/sections remember the chosen mode. If we only hack
rendering, the setting can't survive save/reload and becomes hard to evolve.

#### Purpose

Add the data model and storage representation for board display mode choices:

- view default
- optional section override

#### Expected end result

- Display mode config persists with views/sections
- Existing data loads safely with defaults (single-line)

### Phase 8: Multi-Line Board Rendering (TUI Implementation)

#### Why

Once config exists, the TUI must actually render rows differently. This is a
separate risk area because variable-height rows can impact selection, scroll,
and layout logic.

#### Purpose

Implement the visual behavior for multi-line rows:

- wrapped item text
- one-category-per-line cells
- overflow cap/summarization
- stable scrolling/highlighting

#### Expected end result

- Multi-line mode is visibly more readable for category-heavy items
- Single-line mode remains unchanged

### Phase 9: View/Section UI Controls for Display Mode

#### Why

A persisted setting is only useful if users can discover and change it in the UI.
This phase wires the new model fields into actual configuration surfaces.

#### Purpose

Expose display mode controls in view/section editing so users can choose:

- default behavior for a view
- overrides for specific sections

#### Expected end result

- Users can toggle single-line vs multi-line without manual file/db edits
- Changes persist through save/reload

### Phase 10: Input Event Plumbing for Modifier Keys (`Ctrl-L` / `Ctrl-R`)

#### Why

Current input dispatch mostly passes `KeyCode`, which drops modifier info. That
makes `Ctrl-L` / `Ctrl-R` impossible (or unreliable) to implement correctly.

This is a cross-cutting refactor and should be handled explicitly before binding
new shortcuts.

#### Purpose

Preserve full key event information (including modifiers) through input dispatch.

#### Expected end result

- Mode handlers can distinguish plain keys from modified keys
- Existing shortcuts continue to work
- `Ctrl-*` shortcuts become implementable in a clean way

### Phase 11: Add Column Left/Right Workflow (Current Section Scope)

#### Why

This is a high-leverage workflow for view shaping, but it depends on two earlier
investments:

- reusable typeahead category selection UX
- modifier-aware input handling

Starting with current-section scope keeps the UX clear and implementation small.

#### Purpose

Let users insert a category-based column adjacent to the current column with a
fast typeahead/create workflow, without leaving the board.

#### Expected end result

- `Ctrl-L` / `Ctrl-R` (or temporary fallback) opens a picker
- User picks or creates a category
- Column is inserted left/right in the current section and persists

### Phase 12: Shared Picker Primitive Cleanup (Optional but Recommended)

#### Why

By this point, direct-edit and add-column flows will likely share a lot of
logic and presentation patterns. Leaving them duplicated increases maintenance
cost and makes future tweaks inconsistent.

#### Purpose

Extract shared picker pieces (suggestions, create-confirm UI/copy, style tokens)
so both workflows evolve together.

#### Expected end result

- Less duplicated code
- Consistent UX across category-picking surfaces

### Phase 13: Final Verification / Polish Pass

#### Why

The plan spans multiple interacting systems (rendering, input, model/store,
view editing). Final integration bugs often show up only after all pieces land.

#### Purpose

Run a deliberate cross-feature validation pass and tighten rough edges before
declaring the work complete.

#### Expected end result

- Major workflows work together without regressions
- Known deviations/compromises are documented
- The feature set is ready for normal usage and iteration


## Detailed TODO List

This checklist is intended to be executable work planning for the branch, but
does **not** imply implementation order must be strictly linear. Where possible,
land small refactors first, then behavior changes.

### Phase 0: CategoryDirectEdit State Extraction (No UX Change Yet)

- [x] Add `CategoryDirectEditState` struct(s) in `crates/agenda-tui/src/lib.rs`
- [x] Add `CategoryDirectEditRow` struct with per-row `TextBuffer` + resolved category
- [x] Add `CategoryDirectEditFocus` enum (`Entries`, `Input`, `Suggestions`)
- [x] Add `category_direct_edit: Option<CategoryDirectEditState>` to `App`
- [x] Initialize `category_direct_edit: None` in `App::default()`
- [x] Keep existing `Mode::CategoryDirectEdit` mode enum (do not replace mode yet)
- [x] Update `open_category_direct_edit` to initialize `category_direct_edit` draft state
- [x] Ensure existing `category_suggest` / inline create state are reset consistently on open
- [x] Add helper accessors for direct-edit state (`current`, `current_mut`, etc.)
- [x] Keep current single-entry behavior functionally unchanged after refactor
- [x] Run `cargo test -p agenda-tui --lib`

### Phase 1: Multi-Entry Draft Initialization + Data Helpers

- [x] Add helper to collect current column metadata:
  - [x] parent category id
  - [x] parent category name
  - [x] current section/column anchor info
  - [x] selected item id + label
- [x] Add helper to collect currently assigned child categories for the active column
- [x] Initialize draft rows from current child assignments (one row per category)
- [x] Add one blank row if there are no existing column-child assignments
- [x] Implement row ordering on open using parent category child order (`parent.children`) with alphabetical fallback
- [x] Add row-level helpers:
  - [x] get active row
  - [x] get active row mutable
  - [x] clamp active row index
  - [x] add blank row
  - [x] remove row by index
  - [x] ensure at least one row exists
- [x] Add duplicate prevention helper (draft-level check)
- [x] Add exclusivity helper (is current column parent exclusive?)
- [x] Add helper/guard to block adding a second row immediately for exclusive parents
- [x] Add unit tests for draft initialization and row helper invariants
- [x] Run `cargo test -p agenda-tui --lib`

### Phase 2: Suggestions + Matching for Active Row

- [x] Refactor suggestions to read from active row input instead of shared `self.input`
- [x] Preserve existing matching scope: current column's child categories only
- [x] Preserve "full list on empty input" behavior
- [x] Preserve `When` exclusion from suggestions
- [x] Ensure exact typed match helper reads from active row input
- [x] Ensure exact typed match takes precedence over highlighted suggestion on `Enter`
- [x] Track suggestion cursor per editor state (not global ad hoc state)
- [x] Decide whether suggestion cursor is global-per-editor or per-row
- [x] Implement clamped/wrapping suggestion navigation helpers
- [x] Add tests for:
  - [x] full suggestions on empty
  - [x] exact-match precedence
  - [x] active-row suggestion updates when switching rows
- [x] Run `cargo test -p agenda-tui --lib`

### Phase 3: Multi-Entry Keyboard Model (Draft Editing, Not Apply Yet)

- [ ] Add focus routing for `Entries` / `Input` / `Suggestions`
- [ ] Implement `Tab` / `Shift-Tab` focus cycling in `CategoryDirectEdit`
- [ ] Implement `Up` / `Down` and `j` / `k` navigation by focused region
- [ ] Implement `n` / `a` to add a new row and focus it
- [ ] Implement `x` to remove active row (with safe behavior for last row)
- [ ] Implement active-row text editing using row-local `TextBuffer`
- [ ] Implement `Tab` to copy highlighted suggestion into active row input
- [ ] Implement `Enter` on active row:
  - [ ] empty row => remove row if multiple rows exist; keep one blank row if it is the only row
  - [ ] exact typed match => resolve active row
  - [ ] highlighted suggestion => resolve active row
  - [ ] no match => open inline create confirmation
- [ ] Ensure empty `Enter` never auto-applies the first suggestion
- [ ] Add status/help copy for each substate (normal edit / create confirm / exclusive restriction)
- [ ] Add status/help copy for empty-row `Enter` behavior (remove-row vs keep-single-blank-row)
- [ ] Add tests for row add/remove/navigation and `Enter` semantics where feasible
- [ ] Run `cargo test -p agenda-tui --lib`

### Phase 4: Inline Create-Category Confirm in Multi-Entry Editor

- [ ] Move inline create-confirm state into `CategoryDirectEditState`
- [ ] Preserve current create-confirm-in-same-modal behavior
- [ ] Implement create-confirm key handling precedence:
  - [ ] `Enter` / `y` confirm create
  - [ ] `n` / `Esc` cancel create
  - [ ] other input exits confirm and returns to editing (or explicitly block; decide and document)
- [ ] Create new category under current column heading
- [ ] Resolve the active row to the newly created category
- [ ] Prevent reserved-name creation (`When`, `Done`, `Entry`)
- [ ] Preserve duplicate-name checks across hierarchy / parent constraints
- [ ] Add tests for create-confirm state transitions and new-category resolution
- [ ] Run `cargo test -p agenda-tui --lib`

### Phase 5: Draft Apply / Cancel Semantics (Commit Multi-Entry Edits)

- [ ] Define exact commit contract:
  - [ ] `S` applies full draft and closes editor
  - [ ] `Esc` cancels full draft and closes editor
- [ ] Add helper to compute `desired_child_ids` from resolved draft rows
- [ ] Add helper to compute current assigned child ids for active column
- [ ] Diff current vs desired:
  - [ ] `to_remove`
  - [ ] `to_add`
- [ ] Unassign removed child assignments first
- [ ] Assign added child assignments second
- [ ] Preserve non-column assignments
- [ ] Handle duplicates gracefully (ignore duplicates in draft or collapse on apply)
- [ ] Handle exclusive-parent columns:
  - [ ] revalidate exclusivity at apply even though UI blocks second-row add earlier
  - [ ] show clear status error
- [ ] Ensure selection and board focus restore correctly after apply
- [ ] Ensure draft state clears on apply and on cancel
- [ ] Remove/retire old single-value "replace sibling assignment" path where no longer appropriate
- [ ] Add tests for add/remove/mixed diff application
- [ ] Run `cargo test -p agenda-tui --lib`

### Phase 6: Multi-Entry Modal Rendering (TUI Polish)

- [ ] Render `Assigned In This Column` row list (one line per draft row)
- [ ] Render active-row input section (`Category> ...`)
- [ ] Render suggestions section for active row
- [ ] Render inline create-confirm panel in same modal
- [ ] Render focus styling for:
  - [ ] active row
  - [ ] focused region
  - [ ] selected suggestion
- [ ] Render row count / current value summary in header
- [ ] Render explicit action hints and/or button row (`Add Row`, `Remove Row`, `Save`, `Cancel`)
- [ ] Keep shared muted text color token usage consistent
- [ ] Add cursor-position helper for active row input
- [ ] Confirm small terminal fallback/compact rendering behavior
- [ ] Manually verify no regressions to existing InputPanel and other popups

### Phase 7: Multi-Line Board Rendering Config (Model + Storage)

- [ ] Define display mode model:
  - [ ] view-level default field (e.g. `BoardDisplayMode`)
  - [ ] section-level optional override
- [ ] Choose enum shape (e.g. `SingleLine`, `MultiLine`)
- [ ] Add fields to `agenda-core` model structs (`View`, `Section`)
- [ ] Set defaults preserving current single-line behavior
- [ ] Update store serialization/deserialization
- [ ] Add migration/backward-compat handling for existing stored views (default if missing)
- [ ] Add store round-trip tests for new fields
- [ ] Add unit tests for default behavior when fields absent
- [ ] Run relevant core tests (`cargo test -p agenda-core`)

### Phase 8: Multi-Line Board Rendering (TUI Implementation)

- [ ] Add helper(s) for category column formatting:
  - [ ] single-line comma-joined
  - [ ] multi-line one-per-line
  - [ ] overflow summary line (`+N more`)
- [ ] Add constants for multi-line defaults (category-line cap = `8`, overflow label formatting)
- [ ] Add helper(s) for wrapped item text in multi-line mode
- [ ] Add row-height calculation for board rows in multi-line mode
- [ ] Update board render path to support variable-height rows
- [ ] Ensure selected row highlighting still reads clearly across multiple lines
- [ ] Ensure focused-cell indication remains visible in multi-line rows
- [ ] Revisit scrollbar/offset calculations if row heights vary
- [ ] Add config-aware rendering branch:
  - [ ] effective mode = section override or view default
- [ ] Ensure item text wraps to full available item-column width in multi-line mode
- [ ] Add tests for formatting helpers
- [ ] Manual test with category-heavy items and long item text
- [ ] Verify single-line mode remains byte-for-byte/visually unchanged where possible

### Phase 9: View/Section UI Controls for Display Mode

- [ ] Add view-edit UI affordance for view default display mode
- [ ] Add section-level override control in ViewEdit (section config area or inline controls)
- [ ] Define copy/labels for display mode settings
- [ ] Ensure toggles persist through save/cancel in ViewEdit
- [ ] Add tests for ViewEdit draft -> persisted display mode fields
- [ ] Manual test switching between single-line and multi-line in a real view

### Phase 10: Input Event Plumbing for Modifier Keys (`Ctrl-L` / `Ctrl-R`)

- [ ] Audit input dispatch signatures currently using `KeyCode`
- [ ] Introduce an input abstraction or pass `KeyEvent` through dispatch
- [ ] Update top-level app loop dispatch to preserve modifiers
- [ ] Update mode handlers to accept new event type or compatible wrapper
- [ ] Preserve existing behavior for all non-modified keys
- [ ] Add regression tests for key handling if test harness supports modifier events
- [ ] Run `cargo test -p agenda-tui --lib`

### Phase 11: Add Column Left/Right Workflow (Current Section Scope)

- [ ] Define insertion anchor model:
  - [ ] section index
  - [ ] current column index
  - [ ] direction (`Left` / `Right`)
- [ ] Add `Ctrl-L` / `Ctrl-R` commands in board mode (post-KeyEvent refactor)
- [ ] Add fallback commands if modifier support is deferred (document temporary mapping)
- [ ] Enforce/implement initial scope as current-section-only insertion (confirmed)
- [ ] Open an add-column picker modal using shared typeahead UI patterns
- [ ] Scope suggestions to all categories valid for column headings (clarify filtering)
- [ ] Support exact-match select + inline create-confirm in add-column workflow
- [ ] Insert column into current section at correct target index
- [ ] Refresh board and keep user focus on/near inserted column
- [ ] Persist and verify through reload
- [ ] Add tests for insertion index calculations (`left` / `right`)
- [ ] Add tests for picker confirm/cancel transitions if practical
- [ ] Manual test end-to-end on real views with multiple sections

### Phase 12: Shared Picker Primitive Cleanup (Optional but Recommended)

- [ ] Identify duplicated logic between:
  - [ ] direct-edit multi-entry suggestions
  - [ ] add-column picker suggestions
  - [ ] inline create-confirm rendering and key handling
- [ ] Extract shared helpers/components with minimal coupling
- [ ] Standardize wording/copy across pickers
- [ ] Standardize style tokens (muted text, titles, help rows)
- [ ] Retest both flows after extraction

### Phase 13: Final Verification / Polish Pass

- [ ] Manual smoke test all affected workflows:
  - [ ] item add/edit InputPanel
  - [ ] item assignment picker
  - [ ] direct column category edit (single + multi entry)
  - [ ] inline create confirm
  - [ ] multi-line board mode
  - [ ] column insert left/right
- [ ] Check behavior on narrow terminal sizes
- [ ] Check behavior on macOS Terminal and iTerm (if available)
- [ ] Run `cargo test` (or targeted crate tests) and summarize failures/warnings
- [ ] Run `cargo clippy` (targeted crates if needed) and triage warnings
- [ ] Update plan notes / implementation notes with deviations from original design

## Suggested Milestones (Shipping Increments)

- [ ] Milestone A: Multi-entry draft editor works + explicit apply/cancel (single-line board only)
- [ ] Milestone B: Inline create confirm + exclusivity handling polished
- [ ] Milestone C: Multi-line board rendering + config persistence
- [ ] Milestone D: `Ctrl-L` / `Ctrl-R` column insertion (current-section scope)
- [ ] Milestone E: Shared picker cleanup and UX polish





## UI Mockups

### Column editor
Column Editor (Normal State)

┌────────────────────────────── Set Column Categories ──────────────────────────────┐
│ Column: Status                      Item: Add --category flag to agenda-cli ...   │
│ Scope: This column only             Parent category: Status (exclusive: no)       │
│                                                                                   │
│ Assigned In This Column                                                     (3)   │
│ ┌───────────────────────────────────────────────────────────────────────────────┐  │
│ │ > 1. test                                                                    │  │
│ │   2. Not Started                                                             │  │
│ │   3. (new row)                                                               │  │
│ └───────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                   │
│ Edit Active Row                                                                   │
│ Category> no_                                                                     │
│                                                                                   │
│ Suggested Categories                                                              │
│ ┌───────────────────────────────────────────────────────────────────────────────┐  │
│ │ > Not Started                                                                 │  │
│ │   No Review Yet                                                               │  │
│ │   Notes Needed                                                                │  │
│ └───────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                   │
│ Tab/Shift-Tab focus  Up/Down move  Enter apply row  Tab copy suggestion          │
│ n/a add row  x remove row  S save column edits  Esc cancel                        │
└───────────────────────────────────────────────────────────────────────────────────┘
Column Editor (Inline Create Confirm State)

┌────────────────────────────── Set Column Categories ──────────────────────────────┐
│ Column: Status                      Item: Add --category flag to agenda-cli ...   │
│ Scope: This column only             Parent category: Status (exclusive: no)       │
│                                                                                   │
│ Assigned In This Column                                                     (2)   │
│ ┌───────────────────────────────────────────────────────────────────────────────┐  │
│ │   1. test                                                                    │  │
│ │ > 2. (new row)                                                               │  │
│ └───────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                   │
│ Edit Active Row                                                                   │
│ Category> not started-ish                                                         │
│                                                                                   │
│ Create Category                                                                   │
│ ┌───────────────────────────────────────────────────────────────────────────────┐  │
│ │ Create "not started-ish" as a child of "Status"?                             │  │
│ │                                                                               │  │
│ │ Enter / y = create and use     n / Esc = cancel                              │  │
│ └───────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                   │
│ S save all edits  Esc cancel editor                                               │
└───────────────────────────────────────────────────────────────────────────────────┘
Column Editor (Exclusive Parent Variant)

┌────────────────────────────── Set Column Category ────────────────────────────────┐
│ Column: Priority                    Item: Fix parser edge case                    │
│ Parent category: Priority (exclusive: yes)                                        │
│                                                                                   │
│ Assigned In This Column                                                     (1)   │
│ ┌───────────────────────────────────────────────────────────────────────────────┐  │
│ │ > 1. Medium                                                                  │  │
│ └───────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                   │
│ Category> high                                                                    │
│                                                                                   │
│ Suggested Categories: High, Medium, Low                                           │
│                                                                                   │
│ Note: This column allows one value only. Enter replaces the current value.        │
│ S save  Esc cancel                                                                │
└───────────────────────────────────────────────────────────────────────────────────┘
Add Column Left/Right (Related Modal, same design language)

┌────────────────────────────── Insert Column (Right) ──────────────────────────────┐
│ Section: Backlog                     Anchor: [Priority] -> insert to the right    │
│                                                                                   │
│ Column Heading Category> effort                                                    │
│                                                                                   │
│ Suggested Categories                                                              │
│ ┌───────────────────────────────────────────────────────────────────────────────┐  │
│ │ > Effort                                                                      │  │
│ │   Estimate                                                                    │  │
│ │   Complexity                                                                  │  │
│ └───────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                   │
│ Enter insert column  Tab copy suggestion  n create new category  Esc cancel       │
└───────────────────────────────────────────────────────────────────────────────────┘
Notes

Empty Category> + Enter should clear/remove the active row (not select first suggestion).
S is explicit apply for the whole draft (safer for multi-row edits).
This layout keeps the workflow typing-first while making multi-category rows visible.



## Enhanced Category Picker (Single-Entry Version, Polished)

┌────────────────────────────── Set Category ───────────────────────────────┐
│ Column: Status                  Item: Add --category flag to agenda-cli…  │
│ Current value: test             Scope: This column only                   │
│                                                                          │
│ Category> not st_                                                        │
│                                                                          │
│ Suggested Categories                                                     │
│ ┌──────────────────────────────────────────────────────────────────────┐ │
│ │ > Not Started                                                       │ │
│ │   Not Reviewed                                                      │ │
│ │   Needs Test                                                        │ │
│ └──────────────────────────────────────────────────────────────────────┘ │
│                                                                          │
│ Enter applies exact match (if typed) or selected suggestion             │
│ Tab copies suggestion   Esc cancels   ⌘-friendly keys: j/k also work    │
│                                                                          │
│ [Clear Value]   [Cancel]   [Apply]                                       │
└──────────────────────────────────────────────────────────────────────────┘
Enhanced Category Picker (Empty Input / Full List Visible)

┌────────────────────────────── Set Category ───────────────────────────────┐
│ Column: Status                  Item: Add --category flag to agenda-cli…  │
│ Current value: test             Scope: This column only                   │
│                                                                          │
│ Category> _                                                               │
│                                                                          │
│ Suggested Categories (start typing to narrow)                             │
│ ┌──────────────────────────────────────────────────────────────────────┐ │
│ │ > Completed                                                         │ │
│ │   Not Started                                                       │ │
│ │   test                                                              │ │
│ └──────────────────────────────────────────────────────────────────────┘ │
│                                                                          │
│ Enter with empty input clears the current value                          │
│ Tab copies suggestion   Esc cancels                                      │
│                                                                          │
│ [Clear Value]   [Cancel]   [Apply]                                       │
└──────────────────────────────────────────────────────────────────────────┘
Enhanced Category Picker (Inline Create Confirm, Same Workflow)

┌────────────────────────────── Set Category ───────────────────────────────┐
│ Column: Status                  Item: Add --category flag to agenda-cli…  │
│ Current value: test             Scope: This column only                   │
│                                                                          │
│ Category> not started-ish                                                │
│                                                                          │
│ Create Category                                                           │
│ ┌──────────────────────────────────────────────────────────────────────┐ │
│ │ Create "not started-ish" under parent category "Status"?            │ │
│ │                                                                      │ │
│ │ Enter / y = create and apply    n / Esc = cancel                     │ │
│ └──────────────────────────────────────────────────────────────────────┘ │
│                                                                          │
│ [Cancel Create]                                        [Create & Apply]  │
└──────────────────────────────────────────────────────────────────────────┘




Proposed updates (behavior + wording) for this picker
Keep full list visible on empty input (already requested)
Make empty Enter = clear value explicit in UI copy
Keep exact typed match wins over highlighted row
Keep create-confirm inline (same modal)
Add explicit action row/buttons (Clear, Cancel, Apply) for discoverability
Use shared muted text color token (same as footer hints)
How this fits with the future multi-entry editor
This single-entry picker can become the “row editor” visual template for the multi-entry modal:

same header/context
same Category> input row
same suggestions panel
same inline create-confirm panel
same language/buttons/hints




## Enhanced Category Picker (Multi-entry Version, Future roadmap)

┌────────────────────────────── Set Column Categories ──────────────────────────────┐
│ Column: Status                      Item: Add --category flag to agenda-cli ...   │
│ Current values: test, Not Started   Scope: This column only (multi-value)         │
│                                                                                   │
│ Assigned In This Column                                                      (3)  │
│ ┌───────────────────────────────────────────────────────────────────────────────┐  │
│ │ > 1. test                                                                    │  │
│ │   2. Not Started                                                             │  │
│ │   3. (new row)                                                               │  │
│ └───────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                   │
│ Edit Active Row                                                                   │
│ Category> not rev_                                                                 │
│                                                                                   │
│ Suggested Categories                                                              │
│ ┌───────────────────────────────────────────────────────────────────────────────┐  │
│ │ > Not Reviewed                                                                │  │
│ │   Notes Needed                                                                │  │
│ │   Not Started                                                                 │  │
│ └───────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                   │
│ Enter applies to active row  Tab copies suggestion  n adds row  x removes row    │
│ Tab/Shift-Tab focus  Up/Down or j/k move  S save all  Esc cancel                  │
│                                                                                   │
│ [Add Row]   [Remove Row]   [Cancel]                              [Save Column]     │
└───────────────────────────────────────────────────────────────────────────────────┘
┌────────────────────────────── Set Column Categories ──────────────────────────────┐
│ Column: Status                      Item: Add --category flag to agenda-cli ...   │
│ Current values: test, Not Started   Scope: This column only (multi-value)         │
│                                                                                   │
│ Assigned In This Column                                                      (2)  │
│ ┌───────────────────────────────────────────────────────────────────────────────┐  │
│ │   1. test                                                                    │  │
│ │ > 2. (new row)                                                               │  │
│ └───────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                   │
│ Edit Active Row                                                                   │
│ Category> not started-ish                                                         │
│                                                                                   │
│ Create Category                                                                   │
│ ┌───────────────────────────────────────────────────────────────────────────────┐  │
│ │ Create "not started-ish" as a child of "Status"?                             │  │
│ │                                                                               │  │
│ │ Enter / y = create and use in this row     n / Esc = cancel                  │  │
│ └───────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                   │
│ [Cancel Create]                                               [Create & Use]      │
└───────────────────────────────────────────────────────────────────────────────────┘
┌────────────────────────────── Set Column Categories ──────────────────────────────┐
│ Column: Priority                    Item: Fix parser edge case                    │
│ Current value: Medium               Scope: This column only (single-value)        │
│ Parent category: Priority (exclusive)                                             │
│                                                                                   │
│ Assigned In This Column                                                      (1)  │
│ ┌───────────────────────────────────────────────────────────────────────────────┐  │
│ │ > 1. Medium                                                                  │  │
│ └───────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                   │
│ Edit Active Row                                                                   │
│ Category> high                                                                    │
│                                                                                   │
│ Suggested Categories                                                              │
│ ┌───────────────────────────────────────────────────────────────────────────────┐  │
│ │ > High                                                                       │  │
│ │   Medium                                                                     │  │
│ │   Low                                                                        │  │
│ └───────────────────────────────────────────────────────────────────────────────┘  │
│                                                                                   │
│ This column accepts one value. Enter replaces the current value.                  │
│ S save  Esc cancel                                                                 │
└───────────────────────────────────────────────────────────────────────────────────┘
┌────────────────────── Set Column Categories ──────────────────────┐
│ Status • Add --category flag to agenda-cli…                       │
│ Values: test, Not Started                                         │
│                                                                   │
│ Rows                                                               │
│ > 1. test                                                         │
│   2. (new row)                                                    │
│                                                                   │
│ Category> no_                                                     │
│                                                                   │
│ Suggestions                                                        │
│ > Not Started                                                     │
│   Notes Needed                                                    │
│   Not Reviewed                                                    │
│                                                                   │
│ Enter apply row  n add  x remove  S save  Esc cancel              │
└───────────────────────────────────────────────────────────────────┘
Design notes to carry into implementation:

Enter should affect the active row only.
S is the explicit commit for the whole draft.
Empty active row + Enter should clear/remove that row, not choose first suggestion.
Exact typed match should win over highlighted suggestion.
This can share the same suggestion/create-confirm subcomponents as the current single-entry picker.
