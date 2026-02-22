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
