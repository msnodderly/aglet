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
