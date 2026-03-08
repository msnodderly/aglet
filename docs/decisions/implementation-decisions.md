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
via dedicated linking keys (`b` / `B`) on the selected item, rather than being
primarily embedded inside the item edit panel.

Rationale:

- aligns with Lotus Agenda's dependency workflow (`ALT-O`) being a view-level action
- better fits future multi-select / marked-item batch linking
- avoids overloading the already-dense item edit panel

### Use a Link Wizard as the primary TUI linking UI (single-item first, batch later)

The preferred TUI direction is a dedicated Link Wizard opened from the view via
`b` / `B`:

- `b` opens the wizard with `blocked by` preselected
- `B` opens the wizard with `blocks` preselected

`L` remains reserved for board-column reordering. The wizard should keep a
batch-capable design, but batch/marked-item mode is a follow-up phase.

Planned behavior:

- current phase: single-item scope defaults to the current item
- later phase: if items are marked, wizard opens in batch mode over marked items
- wizard presents:
  - scope (selected item now; marked items later)
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

### Relationship-aware filtering/search is a core goal (prioritize "Ready")

The purpose of linked-item relationships is not only editing/visibility; Aglet
should eventually support filtering/searching based on dependency state.

Priority direction:

- first relationship-aware view/filter target is **Ready** = "not blocked"
- readiness should be driven by `depends-on` / blocking relationships (not
  `related`)
- batch/mark linking mode can be deferred behind core readiness filtering work

Exact query syntax and edge semantics (for example how done prerequisites affect
"ready") can be finalized in the filtering/query phase, but "Ready view" is a
planned primary outcome of this feature line.

### Save key: Capital S from non-text focus, Tab out of text fields first

The universal save key is `S` (capital). It works from any non-text-input focus
in all saveable modes (InputPanel, ViewEdit, NoteEdit, CategoryDirectEdit).

When focus is in a text field (item text, note, numeric value), `S` is consumed
as the letter S. To save, the user Tabs out of the text field first, then
presses `S`. This is a standard form-editing pattern: finish editing, then
submit.

`Enter` is NOT used as a save key for forms. `Enter` remains a contextual action
key (toggle criterion, activate field, select picker item). Overloading `Enter`
for both "activate" and "save" creates ambiguity about what will happen.

Footer hints should reflect the current focus:
- Text-field focus: `Tab:next field  Esc:cancel`
- Non-text focus: `S:save  Esc:cancel` (plus mode-specific actions)

Normalize CategoryDirectEdit to accept only capital `S` (not lowercase `s`).

### Two save patterns: explicit-save and immediate-apply

Both patterns are valid but must be applied consistently by action type:

**Explicit-save** (user presses `S` to commit, `Esc` to discard):
- InputPanel (add/edit item)
- ViewEdit (view/section editing)
- NoteEdit
- CategoryDirectEdit (column cell editing)
- Any mode editing multiple fields or text

**Immediate-apply** (each toggle/action takes effect instantly):
- ItemAssignPicker (Space toggles category assignment)
- CategoryManager flag toggles (`e`/`i`/`a`)
- Any single-toggle boolean action

Rules:
1. Immediate-apply modes must show `changes apply immediately` in the footer.
   They should NOT show an `S:save` hint (there is nothing to save).
2. Explicit-save modes must show a dirty indicator (`*` or `[modified]`) when
   the form has unsaved changes.
3. Auto-save-on-focus-change (currently used by CategoryManager note field) is
   eliminated. All text editing uses explicit save.
4. `Esc` in an explicit-save mode with unsaved changes should prompt for
   confirmation before discarding (ViewEdit already does this; other modes
   should follow).

### Marker column glyphs: standardized indicators

The marker column (currently used for `♪` note indicator) displays item state
at a glance. Standardized glyphs:

| Glyph | Meaning |
|-------|---------|
| `♪`   | Has a note |
| `✓`   | Done (replaces `[done]` prefix in item label) |
| `!`   | Blocked (has unresolved prerequisites) |

Multiple markers can appear together (e.g., `♪!` = has a note and is blocked).

The `[done]` prefix is removed from `board_item_label()`. Done state is shown
exclusively via the marker column glyph.

"Blocks others" is not shown in the marker column for now. That information
surfaces in preview/edit. The primary at-a-glance question is "can I work on
this?" — `!` answers that.

### Checkbox/boolean display conventions

| Glyph | Meaning | Context |
|-------|---------|---------|
| `[ ]` / `[x]` | Unassigned / assigned | All multi-select contexts |
| `( )` / `(*)` | Unselected / selected | Exclusive (radio) categories |
| `[ ]` / `[42.5]` | Unassigned / assigned numeric (showing value) | InputPanel and ItemAssignPicker |

Numeric categories in ItemAssignPicker show their current value read-only
(e.g., `[42.5]`). Editing the value is done in InputPanel only.

### Footer hint principles and per-mode specifications

**Principles:**

1. **Hints are reminders, not documentation.** Show 5-6 actions max per mode.
2. **Never show `j/k`.** Vertical list navigation is universal and assumed.
3. **No compound keys.** Use one key per action, not `Enter/Space` or `n/Esc`.
   Pick the primary key and use it consistently across all modes.
4. **Consistent action verbs.** Same word for same concept everywhere:
   - `save` (not "apply" or "done" or "confirm")
   - `cancel` (not "back" or "close" or "discard")
   - `toggle` (not "cycle" or "select" or "action")
   - `new` (not "create" or "add")
   - `delete` (not "remove")
5. **Always end with `S:save  Esc:cancel`** in explicit-save modes, or
   `Esc:cancel` in immediate-apply/picker modes.
6. **Saveable modes show save first.** `S:save` is always the leftmost hint
   in explicit-save modes.
7. **Confirmation dialogs are minimal.** Just `y:confirm  Esc:cancel`.

**Per-mode hint specifications:**

Normal (board view):
```
n:new  e:edit  d:done  a:assign  /:filter  v:views  c:categories  q:quit
```

InputPanel (all focus states):
```
S:save  Tab:next  Space:toggle  Esc:cancel
```

NoteEdit:
```
S:save  Esc:cancel
```

FilterInput:
```
Enter:apply  Esc:cancel
```

ViewPicker:
```
Enter:switch  N:new  r:rename  e:edit  x:delete  Esc:cancel
```

ViewEdit — Sections pane:
```
S:save  n:new  x:delete  Enter:details  Tab:pane  Esc:cancel
```

ViewEdit — Details/Criteria pane:
```
S:save  n:new  x:delete  Space:toggle  Tab:pane  Esc:cancel
```

ViewEdit — Preview pane:
```
S:save  p:hide  Tab:pane  Esc:cancel
```

CategoryManager:
```
n:new  r:rename  x:delete  Tab:pane  /:filter  Esc:close
```

CategoryDirectEdit:
```
S:save  Tab:focus  Enter:resolve  x:delete  Esc:cancel
```

CategoryColumnPicker:
```
Space:toggle  Enter:save  Tab:focus  Esc:cancel
```

BoardAddColumnPicker:
```
Enter:insert  Tab:complete  Esc:cancel
```

ItemAssignPicker:
```
Space:toggle  n:new  Enter:done  Esc:cancel
```

ItemAssignInput:
```
Enter:assign  Esc:cancel
```

LinkWizard:
```
Tab:focus  Enter:apply  Esc:cancel
```

InspectUnassign:
```
Enter:unassign  Esc:cancel
```

All confirmation dialogs (ConfirmDelete, ViewDeleteConfirm,
BoardColumnDeleteConfirm, CategoryCreateConfirm):
```
y:confirm  Esc:cancel
```

### Dirty state indicator in editor titles

All explicit-save modes show a `*` appended to the panel/mode title when the
form has unsaved changes:

- `Edit Item *`
- `Add Item *`
- `Note *`
- `View: MyView *`
- `Edit Column *` (CategoryDirectEdit)

The `*` appears in the title bar, not the footer (keeps footer clean for hints).

"Dirty" means any field differs from its value when the editor was opened.
The indicator clears on save or when changes are reverted to match the original.

This supports the discard-confirmation behavior: `Esc` on a dirty form prompts
`Discard changes? y/Esc` before canceling.

### Empty-state guidance for new users

When a view contains no items, display a centered help message instead of a
blank screen. Example:

```
No items in this view.

n  add item
v  switch view
c  manage categories
q  quit
```

The message shows the 3-4 most relevant actions for the current context.
Different empty states may show different hints:

- Empty Inbox/All Items: `n:add item` prominently
- Empty filtered view: `/:clear filter` or `Esc:clear filter`
- Empty custom view with criteria: hint that criteria may be too restrictive

This applies to first-run experience and to any view that becomes empty through
filtering or item movement.

### Lotus reference behavior adopted conceptually (with modernized UX)

Lotus Agenda used:

- mark items (`F7`)
- invoke a view-level dependency command (`ALT-O`)
- confirm in a "Make Item Dependent" box

Aglet will keep the same *workflow shape* (view-level, mark-aware, confirmation
before apply) but modernize it with a richer wizard/picker and explicit preview.
