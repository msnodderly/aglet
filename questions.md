# Questions

## Open Questions (as of 2026-02-22)

### Multi-entry row ordering on open

When a column already has multiple assigned child categories, should the draft
rows open in:

- alphabetical order (current Phase 0 scaffolding behavior), or
- assignment timestamp/order, or
- section/view-defined display order?

This affects perceived stability when reopening the editor.

### Exclusive parent behavior with multi-entry UI

If the column parent category is exclusive, what should happen when the user
tries to add a second row?

Options:

- block with a clear message (recommended in plan)
- auto-replace the existing row/value
- allow multiple draft rows but fail on save/apply

### Empty active row semantics in multi-entry editor

The plan proposes “empty `Enter` affects the active row only” to avoid accidental
selection. Should empty `Enter`:

- remove the active row immediately, or
- clear the row text only (keeping a blank row), or
- no-op unless an explicit `x` is used?

### Multi-line board rendering defaults / cap details

The plan proposes multi-line rendering with per-category lines and an overflow
cap (example: 8). Please confirm:

- default cap should be `8`
- overflow label style (`+N more`) is acceptable
- item text wrapping should use full available width for the item column in multi-line mode

### Add-column insertion scope (initial release)

Plan assumes “current section only” insertion for the first implementation.
Please confirm that is still the intended first milestone before supporting
view-wide insertion policies.
