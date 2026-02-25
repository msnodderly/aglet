# View Picker + View Editor Keybindings (Current Implementation)

Current branch behavior for the split model:

- `v` opens the quick **View Picker**
- `e` from picker opens the full-screen **View Editor**

## View Picker

- `j/k`: move selection
- `Enter`: switch active view
- `N`: create view (name prompt, then open editor)
- `r`: rename selected view
- `x`: delete selected view (confirm)
- `e`: edit selected view (full-screen)
- `Esc`: close picker

## View Editor (Full-Screen)

- `Tab` / `Shift-Tab`: cycle pane focus (`Sections`, `Details`, optional `Preview`)
- `S`: save view and return to picker
- `Esc`: clear section filter (if active), otherwise cancel/discard flow
- `p`: toggle preview pane
- `/`: edit section filter

### Sections Pane

- `j/k`: move selected row (`View:` row + sections)
- `Enter`: expand/collapse section row, or open view details from `View:` row
- `n`: add section below current and start title edit
- `N`: add section above current and start title edit
- `r`: rename selected section, or rename view when `View:` row is selected
- `x`: delete selected section (confirm)
- `J/K` or `]`/`[`: reorder section down/up

### Details Pane (View or Section)

- `j/k`: move field/row
- `Enter` / `Space`: perform field action (toggle/open picker/start edit)
- `r` (view details): rename view
- Mnemonic shortcuts still work for section details:
  - `e/t` title
  - `f` criteria
  - `c` columns
  - `a` on-insert assign
  - `r` on-remove unassign
  - `h` show children
  - `m` display override
  - `x` delete section (confirm)

### Picker Overlays

- Category picker now supports type-to-filter
- `j/k`: move
- `Space` / `Enter`: toggle selected item
- `Esc`: close overlay
