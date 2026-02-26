# Decisions

## 2026-02-22

### Phase 0 scope: state extraction only, no UX change

Phase 0 introduces a dedicated `CategoryDirectEditState` and related structs, but
keeps the current single-entry direct-edit picker behavior intact. The goal is to
reduce risk before adding multi-entry editing.

### Keep existing direct-edit globals temporarily

The current direct-edit flow still uses shared fields like `self.input`,
`category_suggest`, and `category_direct_edit_create_confirm` during Phase 0.
The new `CategoryDirectEditState` is scaffolding for later phases, not yet the
single source of truth.

### Initialize direct-edit draft rows from current column assignments

When opening direct-edit, the new state captures current child-category
assignments under the selected column heading as draft rows (plus one blank row
if none exist). This data is not user-visible yet, but it sets up Phase 1.

### Read modal context from direct-edit state when available

The direct-edit modal header/context text now reads from `CategoryDirectEditState`
when present (with fallback to the previous computed path), which starts the
Phase 0 “light rewiring” without changing visible behavior.

### Multi-entry row ordering on open: use parent child order

For the future multi-entry editor, draft rows should open in the parent
category's child ordering (`parent.children`) when available. This preserves a
hierarchy-defined order and should feel more stable than alphabetical sorting.

Fallback may be alphabetical if the parent/child order is unavailable.

### Exclusive parent behavior in multi-entry UI: block second row immediately

If a column parent category is exclusive, attempting to add a second row/value in
the multi-entry editor should be blocked immediately with a clear message.

We are explicitly not auto-replacing the existing row/value and not deferring the
error to save/apply.

### Empty active row + Enter behavior in multi-entry editor

`Enter` on an empty active row should operate on that row only:

- remove the row if multiple rows exist
- keep a single blank row if it is the only row

This avoids accidental suggestion selection and preserves explicit row-level
editing semantics.

### Multi-line board rendering defaults (initial)

Confirmed defaults for the future multi-line board mode:

- visible category-line cap per cell/row: `8`
- overflow summary format: `+N more`
- item text wraps to full available width of the item column

### Add-column insertion scope for first release

Initial add-column-left/right implementation will target **current section only**
to minimize ambiguity and reduce implementation complexity. View-wide insertion
policies are a later enhancement.

## 2026-02-26

### Linked-item editing is a view-level workflow (not item-edit-panel-first)

Linked-item creation/removal should be triggered directly from the view screen
with `L` on the selected item, rather than being primarily embedded inside the
item edit panel.

Rationale:

- aligns with Lotus Agenda's dependency workflow (`ALT-O`) being a view-level action
- better fits future multi-select / marked-item batch linking
- avoids overloading the already-dense item edit panel

### Use a Link Wizard (batch-capable) as the primary TUI linking UI

The preferred TUI direction is a dedicated Link Wizard opened from the view via
`L`, using a batch-capable flow even for single-item operations.

Planned behavior:

- if no items are marked, wizard scope defaults to the current item
- if items are marked, wizard opens in batch mode over marked items
- wizard presents:
  - scope (selected/marked items)
  - relationship choice (`blocked by`, `depends on`, `blocks`, `related to`,
    and `clear dependencies`)
  - target-item picker/search (not needed for `clear dependencies`)
  - plain-language preview of resulting operations before apply

### Keep dependency summary and clear-dependencies convenience in item edit panel

The item edit panel should remain focused on text/note/categories, but include:

- read-only dependency/link summary (`Prereqs`, `Blocks`, `Related`)
- a single-item `Clear dependencies` convenience action
- hint/action to open the Link Wizard

This preserves quick cleanup and discoverability without making item editing the
primary linking workflow.

### Keep and extend dependency markers in the view

Aglet should display dependency markers in the view (similar in spirit to
Lotus Agenda's `&` dependency marker and our current note indicator).

Direction:

- mark items that have prerequisites (blocked / depends on others)
- also indicate items that block others (has dependents)
- exact glyph set may evolve, but the marker area should make dependency state
  visible without opening preview or item edit

### Dependency tree browsing is a separate workflow from link editing

Tree/chain exploration ("items blocking items blocking items") should be a
separate viewer/command, not embedded into the Link Wizard.

Planned direction:

- one-level and all-level traversal modes
- prerequisites and dependents views
- eventual hierarchy/tree display for dependency chains

This mirrors Lotus Agenda's "Utilities Show Prereqs / Depends" model while
keeping editing and browsing distinct.

### Lotus reference behavior adopted conceptually (with modernized UX)

Lotus Agenda used:

- mark items (`F7`)
- invoke a view-level dependency command (`ALT-O`)
- confirm in a "Make Item Dependent" box

Aglet will keep the same *workflow shape* (view-level, mark-aware, confirmation
before apply) but modernize it with a richer wizard/picker and explicit preview.
