# Questions

## Open Questions (as of 2026-02-22)

No open questions at the moment for Phase 1 start.

## Resolved Questions (2026-02-22)

### Multi-entry row ordering on open

Use **parent category child order** (`parent.children`) as the primary order for
draft rows when opening the editor.

Fallback: alphabetical ordering only if parent/child order cannot be recovered.

### Exclusive parent behavior with multi-entry UI

If the column parent category is exclusive, **block adding a second row
immediately** with a clear message.

Do not auto-replace implicitly, and do not wait until save/apply to fail.

### Empty active row semantics in multi-entry editor

`Enter` on an empty active row should:

- remove that row if multiple rows exist
- keep a single blank row if it is the only row

`x` remains the explicit remove-row action.

### Multi-line board rendering defaults / cap details

Confirmed defaults:

- default visible category-line cap: `8`
- overflow label style: `+N more`
- item text wraps to full available width of the item column in multi-line mode

### Add-column insertion scope (initial release)

Confirmed initial scope: **current section only**.

View-wide insertion policies can be a follow-up after the first release.
