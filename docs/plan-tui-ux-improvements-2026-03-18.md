# Plan: TUI UX Improvements — ViewEdit, Category Manager, Footer, Undo

**Date:** 2026-03-18
**Branch:** `tui-ux-improvements-viewedit-catmgr-footer-undo`
**Scope:** Features 3, 4, 5, 6 from the TUI improvement analysis

---

## Feature 3: ViewEdit Aliases Expansion

**Problem:** The Aliases field in ViewEdit Details shows "4 configured" but you can't
see _which_ categories have aliases without opening the full alias picker overlay.

**Solution:** Expand configured aliases inline as indented sub-rows below the Aliases
summary row.

### Changes

- **`render/mod.rs` (ViewEdit Details rendering, ~line 4720-4743):**
  After the "Aliases" summary row, iterate `state.draft.category_aliases` and emit
  one indented `ListItem` per alias: `    CategoryName → AliasValue`. These are
  read-only display rows (not navigable via j/k — they're visual context, not
  interactive fields).

- **No new keybindings needed.** The existing Enter/Space on the Aliases row (field
  index 7) already opens the alias picker overlay. The expansion just adds visibility.

### Testing
- Verify alias sub-rows appear when aliases are configured.
- Verify they disappear when aliases are cleared.
- Verify j/k navigation skips the sub-rows (they're display-only separators).

---

## Feature 4: Category Manager — Item Count in Details

**Problem:** The Details pane shows Selected, Depth, Children, Parent, Reserved — but
not how many items currently have this category assigned. That's the most useful
contextual info when deciding whether to rename/delete a category.

**Solution:** Add an "Assigned items" count to the info section of the Details pane.

### Changes

- **`lib.rs` (App struct):** Add `category_assignment_counts: HashMap<CategoryId, usize>`
  field. Populated during `refresh()` by scanning `self.all_items` and counting
  assignments per category.

- **`app.rs` (refresh method):** After loading `all_items`, build the assignment count
  map:
  ```rust
  self.category_assignment_counts.clear();
  for item in &self.all_items {
      for cat_id in &item.category_ids {
          *self.category_assignment_counts.entry(*cat_id).or_insert(0) += 1;
      }
  }
  ```

- **`render/mod.rs` (CategoryManager details, ~line 4028-4051):** Insert a new line
  after "Depth: N  Children: N":
  ```
  Items assigned: 42
  ```
  Uses `self.category_assignment_counts.get(&row.id).copied().unwrap_or(0)`.
  Adjust `info_height` from 5/6 to 6/7.

### Testing
- Create items with various categories, open CategoryManager, verify counts match.
- Delete items, refresh, verify counts update.

---

## Feature 5: Adaptive Footer Hints

**Problem:** Footer hints are static strings per mode. On narrow terminals they wrap
badly. They show all keys even when many are irrelevant to the current context.

**Solution:** Make hints width-aware: show the most important keys that fit, with a
trailing `?:more` that opens the help panel. Use styled `Span`s to color-code key
names vs. descriptions.

### Changes

- **`render/mod.rs` (`footer_hint_text`):** Change return type from `&'static str` to
  `Vec<(&'static str, &'static str)>` (key, description pairs). Each mode returns an
  ordered list of hint tuples, most important first.

- **`render/mod.rs` (`render_footer`):** Build the hints line by iterating the pairs,
  measuring cumulative width, stopping when adding the next pair would exceed
  `width - 8` (reserving space for `?:help`). Render keys in `LightCyan` and
  descriptions in `DarkGray` using styled `Span`s.

- **Normal mode context sensitivity:**
  - When `column_index > 0` and the column is numeric: prepend `Enter:edit value`
  - When `column_index > 0` and the column is a When column: prepend `Enter:edit date`
  - When items are selected: show selection-relevant keys first

### Hint ordering by mode (first = highest priority):

**Normal (no selection, no filter):**
```
n:new  e:edit  a:assign  d:done  /:search  v:views  m:lanes  s:sort  f:col fmt  F:col summary  p:preview  u:deps  g/:global  z:cards  Ctrl-L:reload  Ctrl-R:auto-refresh  q:quit  ?:help
```

**Normal (with selection):**
```
Space:toggle  a:assign  d:done  b:link  x:delete  Esc:clear  /:search  v:views  ?:help
```

**Normal (with filter):**
```
n:new  e:edit  Esc:clear filter  a:assign  d:done  /:search  v:views  ?:help
```

### Testing
- Resize terminal to various widths, verify hints truncate gracefully.
- Verify `?:help` always appears as the last hint.
- Verify context-sensitive hints appear when cursor is on a numeric column.

---

## Feature 6: Undo Stack

**Problem:** Every mutation is immediate and permanent. Users have no safety net when
experimenting with category assignments, item edits, or deletions.

**Solution:** A bounded undo stack (last 50 operations) that records reversible actions
and provides `Ctrl-Z` to undo the most recent one.

### Design

**UndoEntry enum** — each variant captures enough state to reverse the operation:

```rust
enum UndoEntry {
    ItemCreated { item_id: ItemId },
    ItemEdited { item_id: ItemId, old_text: String, old_note: Option<String> },
    ItemDeleted { item: Item, assignments: Vec<(CategoryId, Assignment)> },
    ItemDoneToggled { item_id: ItemId, was_done: bool },
    CategoryAssigned { item_id: ItemId, category_id: CategoryId },
    CategoryUnassigned { item_id: ItemId, category_id: CategoryId, assignment: Assignment },
    NumericValueSet { item_id: ItemId, category_id: CategoryId, old_value: Option<Decimal> },
    LinkCreated { item_id: ItemId, other_id: ItemId, kind: ItemLinkKind },
    LinkRemoved { item_id: ItemId, other_id: ItemId, kind: ItemLinkKind },
    BatchDone { item_ids: Vec<ItemId> },
}
```

**Undo stack field** on App:
```rust
undo_stack: Vec<UndoEntry>,  // bounded to 50 entries
```

### Changes

- **`lib.rs` (App struct):** Add `undo_stack: Vec<UndoEntry>` with max capacity 50.
  Add `push_undo(&mut self, entry: UndoEntry)` helper that pushes and truncates.

- **`lib.rs` (new `undo` module or inline):** `fn apply_undo(app, agenda, entry)` that
  dispatches on UndoEntry variant to reverse the operation:
  - `ItemCreated` → `agenda.delete_item(item_id, "undo")`
  - `ItemEdited` → load item, restore text+note, `agenda.update_item()`
  - `ItemDeleted` → `agenda.create_item(&item)` + re-assign all categories
  - `ItemDoneToggled` → `agenda.toggle_item_done(item_id)`
  - `CategoryAssigned` → `agenda.unassign_item_manual(item_id, category_id)`
  - `CategoryUnassigned` → `agenda.assign_item_manual(item_id, category_id)`
  - `NumericValueSet` → restore old value or unassign
  - `LinkCreated` → `agenda.store().delete_item_link()`
  - `LinkRemoved` → `agenda.store().create_item_link()`
  - `BatchDone` → toggle each back

- **Mode handlers (board.rs, category.rs):** Before each mutation, push the
  corresponding UndoEntry. Key mutation sites:
  - `save_input_panel_add()` → push `ItemCreated`
  - `save_input_panel_edit()` → push `ItemEdited` (capture old text/note before update)
  - `apply_done_toggle_action()` → push `ItemDoneToggled` or `BatchDone`
  - `delete_item` path → push `ItemDeleted` (capture item + assignments before delete)
  - `assign/unassign` in ItemAssignPicker → push `CategoryAssigned`/`CategoryUnassigned`
  - `link_wizard` confirm → push `LinkCreated`
  - Numeric value edit → push `NumericValueSet`

- **Keybinding (`Ctrl-Z`):** In Normal mode, pop from `undo_stack`, call `apply_undo`,
  refresh, set transient status `"Undid: {description}"`.

- **Footer hint:** Add `Ctrl-Z:undo` to Normal mode hints (only shown when stack
  is non-empty).

### Limitations
- Undo is TUI-session-scoped (stack is lost on restart).
- Category mutations (create/rename/delete) are NOT undoable in this phase — they're
  rarer and more deliberate.
- No redo support.

### Testing
- Add item → Ctrl-Z → item disappears from view.
- Edit item text → Ctrl-Z → old text restored.
- Toggle done → Ctrl-Z → item restored to open.
- Delete item → Ctrl-Z → item restored with categories.
- Assign category → Ctrl-Z → assignment removed.

---

## Implementation Order

1. **Feature 4** (Category Manager item count) — DONE
2. **Feature 3** (ViewEdit aliases expansion) — DONE
3. **Feature 5** (Adaptive footer hints) — DONE
4. **Feature 6** (Undo stack) — DONE

## Implementation Notes

All features implemented. Undo tracking covers:
- Item create, edit, delete (single), done toggle (single)
- Category assign/unassign (single item via ItemAssignPicker)
- Numeric value edits (board column click)
- **Redo (Ctrl-Shift-Z)**: Full redo support via inverse entry generation. Each
  undo pushes an inverse to the redo stack; each redo pushes an inverse back to
  the undo stack. New mutations clear the redo stack (standard undo/redo semantics).

Not yet tracked by undo (future work):
- Batch delete, batch done toggle, batch assign
- Link creation/removal (wizard operations)
- Category mutations (create/rename/delete — rarer, more deliberate)
