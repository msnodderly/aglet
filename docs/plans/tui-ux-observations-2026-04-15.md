---
title: TUI UX Observations
date: 2026-04-15
updated: 2026-04-16
source: tmux smoke test of view creation, item CRUD, category management
status: observations
---

# TUI UX Observations (2026-04-15)

Full walkthrough: created a view with two sections + filters, added/assigned
items, edited notes, toggled done, used preview/undo/search/category manager,
tested lane layout. Tested on 120x40 terminal via tmux.

---

## Pain Points

### 1. J/K move items instead of navigating sections (high friction)

`J` and `K` **move the selected item** between sections. The only way to
jump the cursor between sections without moving the item is `Tab`/`S-Tab`.
This is counter-intuitive: in vim, `J` joins lines but doesn't move them;
in most TUIs, capital versions are "bigger jumps" not "actions". During
testing I accidentally moved an item out of its section twice before
realizing the keybinding.

**Impact**: Unintended item moves that silently change category assignments
(via on_remove_unassign). The undo works but the user may not notice the
move happened.

**Suggestion**: Consider swapping: `J`/`K` for section-cursor-jump (what
`Tab`/`S-Tab` do now), and `]`/`[` exclusively for item moves. Or add a
visual confirmation / status flash when an item is moved via `J`/`K`.

### 2. View editor detail navigation is non-obvious

When editing a section's details, `Enter` on the title row opens the title
editor, and `Enter` on Filter opens the criteria picker — but the cursor
position is invisible. There's no visual highlight on which detail row is
focused. The user has to count `j` presses and guess which row they're on.

**Impact**: Easy to accidentally edit the wrong field (I renamed a section
when trying to open its filter twice).

**Suggestion**: Highlight the currently-focused detail row with a background
color or cursor indicator (`>` prefix or `[selected]` styling).

### 3. "Deploy staging server" lost its Work category after J-move

After adding "Deploy staging server" with Work category assigned via the
InputPanel, pressing `J` (intending to navigate) moved it to the next
section. The view's section had `on_remove_unassign` configured, which
silently stripped the Work category. The item appeared in Unassigned with
no categories and no visible indication that a category was removed.

**Impact**: Data loss of category assignments via accidental section moves.

**Suggestion**: When `on_remove_unassign` strips categories during a `J`/`K`
move, show the removed categories in the status message:
`Moved to Unassigned (removed: Work)`.

### 4. Section split by child category is surprising

After creating a "Shopping" subcategory under "Personal" in the category
manager, the "Personal Items" section in the view automatically split into
sub-lanes: "Personal Items / Shopping" and "Personal Items / Personal
(Other)". This happened because the section layout defaulted to "Split by
direct child" during the view editor session (I accidentally toggled it
while navigating detail rows).

**Impact**: View layout changes unexpectedly when creating categories.

**Suggestion**: Default section layout to "Flat" for new sections. The
"Split by direct child" should require explicit opt-in, not be a toggle
on a detail row that's easy to accidentally hit.

### 5. Search is per-section only (discoverability)

Pressing `/` opens section-scoped search. The header shows
`[Personal Items] 0 matches` even when matching items exist in other
sections. `g/` for global search isn't shown in the footer hints when `/`
is active — users don't know it exists.

**Impact**: Users think search is broken when it returns 0 results for
items visible in adjacent sections.

**Suggestion**: When section search returns 0 results, add a hint:
`0 matches in Personal Items. g/:search all sections`.

### 6. Empty sections take up significant vertical space

With 4 sections (Work Items, Shopping, Personal (Other), Unassigned), the
three empty sections consume ~75% of the vertical space showing
"No items in this section." This pushes the one section with actual items
(Unassigned with 3-4 items) down to the bottom with minimal visible rows.

**Impact**: Wastes screen real estate; users must scroll mentally to find
where their items are.

**Suggestion**: Collapse empty sections to 2-3 lines (header + "empty"
indicator) in vertical layout. Expand when focused or when items are added.

---

## Minor Observations

### 7. No "q to quit" confirmation

`q` exits immediately with no confirmation, even with unsaved state in a
view editor. (Tested: `q` from Normal mode exits instantly.) This is fine
for normal operation but risky if the user meant to type `q` into a search
or other text field.

### 8. Footer hint bar doesn't show all available keys

In Normal mode, the hints show ~10 keys but omit several important ones:
`J`/`K` (section move), `Tab` (section jump), `c` (category manager),
`C-z` (undo), `z` (card size). The `?` help panel has the complete list
but the footer is the primary discoverability surface.

### 9. Category manager detail pane cursor is invisible

In the category manager details pane, the cursor position (which flag
row is selected) is indicated by `>` but only in the Flags sub-panel. The
Note field, Conditions, and Actions sections have no visible focus
indicator.

### 10. Preview pane note display doesn't wrap long lines

In the preview pane, a long note line extends beyond the pane width without
wrapping, making the full content unreadable without opening the edit panel.

---

## Second tmux Pass (2026-04-16)

Test setup: fresh `/tmp/aglet-tui-second-pass-*.ag` database, 130x42 tmux pane,
seeded categories (`Work`, `Personal`, `Errands`, `ProjectA`, exclusive
`Priority` with `High`/`Normal`) and items with categories and notes. Walked
through creating a view, adding filtered sections, switching layouts, adding
and assigning items, preview, search, done/undo, help, and category manager.

### What Worked Well

- Creating `Second Pass Board` from the view picker and adding filtered
  `Work Focus` / `Personal Focus` sections worked end-to-end.
- The section preview in ViewEdit updated immediately after selecting criteria,
  which made it easy to confirm the filter effect before saving.
- Horizontal lane layout was substantially easier to scan than stacked sections
  once the custom view had multiple lanes.
- Add item + category assignment worked cleanly; saving from the category focus
  returned to the board with clear `Item added` feedback.
- The preview pane tracked lane navigation correctly and `Ctrl-Z` undo after a
  done toggle was fast and understandable.
- Category manager filtering and child-category creation worked once the create
  input received literal text.

### New / Reconfirmed Pain Points

### 11. `All Items` renders categorized items under `Unassigned`

The first render of `All Items` showed all four seeded items in a section titled
`Unassigned`, even though three had categories. This may be technically a
system-view section label, but it reads as if category assignment failed.

**Suggestion**: Rename the system section to `All Items` or `Items`, or reserve
`Unassigned` only for items that truly do not match any category/view section.

### 12. ViewEdit focus is still too implicit

The second pass reproduced the earlier ViewEdit focus issue. While configuring
`Personal Focus`, it was easy to press `j Enter` expecting the Filter row and
open Columns instead. The details pane has a good `◀` marker while editing a
field, but the pre-edit selected row is not prominent enough.

**Suggestion**: Add a persistent selected-row indicator in the details pane, not
only during inline editing. A simple `>` gutter or inverted row background would
remove the guesswork.

### 13. View save flow is a little disorienting

After saving a newly created view, the header changed to
`view:Second Pass Board` while the view palette remained open over the previous
body. Pressing `Enter` then switched the main board. The sequence worked, but
the mixed state made it briefly unclear whether the view was already active.

**Suggestion**: After creating and saving a new view, either switch the board
immediately and close the picker, or keep the old header until the user confirms
the switch.

### 14. Section-scoped search needs an escape hatch

Searching for `launch` while focused on `Personal Focus` showed `0 matches`,
even though `Write launch checklist` was visibly present in the neighboring
`Work Focus` lane. Global search (`g/`) works and returns to the prior view on
`Esc`, but the search footer only says `Enter:jump/create`, `Tab:browse`, and
`Esc:clear`.

**Suggestion**: When section search has no matches, show
`g/:search all sections` in the status/help line. The help panel documents this,
but the moment of failure is where the hint matters.

### 15. Global search temporarily changes the header to `All Items`

Global search for `launch` changed the header from `Second Pass Board` to
`All Items`, then returned after `Esc`. The behavior is functional, but without
prior knowledge it looks like the current view changed rather than a temporary
search scope opening.

**Suggestion**: Keep the current view name in the header and add a scope marker
such as `search: global`, or make the temporary nature explicit in the status
line.

### 16. Assignment picker symbols are dense

The assignment picker is powerful, but rows like `[+] [ ] Work` and
`[-] [x] Personal Focus` require decoding. The category pane and view/section
pane use similar glyphs for different effects, and the meaning is hard to
recover from the footer alone.

**Suggestion**: Add a one-line legend in the picker, or replace symbolic
prefixes with short text (`add`, `remove`, `has`) where space allows.

### 17. Category create has conflicting save/cancel hints

In the Category Manager create modal, the inline panel hint said
`Type name  Enter/Esc:save`, while the footer said `Esc:cancel`. The footer is
consistent with current editor semantics, so the inline hint appears stale.

**Suggestion**: Change the inline hint to `Enter:save  Esc:cancel`.

### 18. Category Manager details can look focused when the tree is active

While the tree was active, the Details pane still showed a `>` marker on the
first flag row and contextual help like "Only one child can be assigned..." for
the Exclusive flag. This can read as if the details pane has focus, or as if the
selected category is already exclusive.

**Suggestion**: Dim or remove details-row focus markers unless the Details pane
is active, and phrase flag help as "If enabled..." so it does not imply current
state.

### 19. Preview wrapping is improved but continuation indentation is uneven

The long Project A note wrapped in the preview pane during this pass, unlike the
earlier observation. The continuation line started at column 1 instead of
aligning with the note text indentation, which makes wrapped note bodies look
like two different blocks.

**Suggestion**: Preserve the note body indentation on wrapped continuation
lines.

---

## Summary

The highest-value improvements are:

1. **#1 + #3**: Fix J/K semantics or add guardrails (prevents data loss)
2. **#2**: Add visible cursor to ViewEdit detail rows (prevents wrong-field edits)
3. **#6**: Collapse empty sections (screen real estate)
4. **#5**: Cross-section search hint (discoverability)
