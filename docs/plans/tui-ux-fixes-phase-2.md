---
title: TUI UX Fixes (Phase 2)
status: draft
created: 2026-04-15
supersedes: tui-ux-observations-2026-04-15.md (observations → actionable plan)
---

# TUI UX Fixes (Phase 2)

## Context

Five actionable improvements derived from the 2026-04-15 tmux smoke test
(see `tui-ux-observations-2026-04-15.md`). User-confirmed priorities:

1. **Fix A (priority)**: Invisible / subtle focus indicators across panes.
2. **Fix B**: Repurpose `J`/`K` from duplicate item-move to section-cursor
   navigation (Tab/BackTab equivalent). Keep `[`/`]` for item moves.
3. **Fix C**: Category-strip notification when `on_remove_unassign` fires
   during an item section move.
4. **Fix D**: Search discoverability when per-section search returns 0
   hits but global would match.
5. **Fix E**: Promote `EmptySections` from `DatebookConfig` to a
   view-level setting so non-datebook views can collapse/hide empty
   sections.

Priority order reflects what the user emphasized: **A first**, then B+C
together (they touch the same code paths), then D, then E.

---

## Fix A: Strengthen focus indicators (priority)

**Problem**: In ViewEdit section details, Category Manager sub-panes
(Note, AlsoMatch, Conditions, Actions), and a handful of list rows, the
focused row/pane is signaled only by a subtle cyan background on one
line or a coloured border. New users miss it entirely; the tmux
observations reported "invisible cursor" in multiple spots.

### Audit findings

The code DOES render focus in all the spots called out:

- `render/mod.rs:8715-8718` — `sel_style` (Black fg on Cyan bg, Bold)
  used for ViewEdit section detail rows at indices 0, 1, 6 and any
  other `section_details_field_index` match.
- `render/mod.rs:7419-7438` — Category Manager `flag_line` prefixes
  `>` when active and applies `focused_cell_style()`.
- `render/mod.rs:7610-7628` — Flags / Note / AlsoMatch / Conditions /
  Actions sub-panes change border colour via `CATEGORY_MANAGER_EDIT_FOCUS`
  vs `pane_idle`.

So the indication *exists* but is weak or inconsistent:

- Border-only signalling (Category Manager sub-panes): a single-pixel
  colour change is easy to miss on dimly-themed terminals.
- Missing `>` prefix on ViewEdit detail rows (only cyan background).
- No consistent convention between `>` prefix vs background highlight.

### Change

Adopt a consistent three-part convention everywhere a focus cursor
needs to be visible:

1. **`>` prefix** on the focused row (or `▶` if monospace allows) —
   works even on colour-stripped captures and screen readers.
2. **Bold label text** in addition to the background cyan.
3. **Border style bumped** from single-line to double-line (`Borders::ALL`
   + `BorderType::Double`) when a bordered pane is focused, so the
   container focus is unmistakable.

### Files

- `crates/agenda-tui/src/render/mod.rs`

### Implementation

**A1. ViewEdit section details** (render/mod.rs:8720-8940):

In `style_for_section_field`, keep the `sel_style` application but also
prepend a `"> "` span / remove leading `"  "` padding from the label
format string when the row is the focused one. Current format is:

```rust
format!("  {:<width$}{}", "Title", title_text, width = pad)
```

Replace with a selected-aware variant:

```rust
let indicator = if selected { "▶ " } else { "  " };
format!("{indicator}{:<width$}{}", "Title", ...)
```

Apply the same pattern at every `items.push(ListItem::new(...))` call
in this block (Title, Filter, Columns, Layout/Split rows, Actions rows
at indices 0-6+). The selection must be visible when cyan isn't
rendered (screenshots, low-colour terminals).

**A2. Category Manager Note / AlsoMatch / Conditions / Actions**
(render/mod.rs:7610-7870):

Current:
```rust
.border_style(Style::default().fg(if flags_border_focused {
    CATEGORY_MANAGER_EDIT_FOCUS
} else {
    pane_idle
}))
```

Augment with `.border_type(BorderType::Double)` when focused and
`.border_type(BorderType::Plain)` otherwise. Apply to all four
sub-panes (search for each `.border_style(...)` call inside the
Category Manager details block).

**A3. Flags pane and flag rows** (render/mod.rs:7419-7438):

The `focus_prefix()` helper (imported in modes/category.rs and render)
already returns `"> "` when active. Verify it is applied consistently
across all flag rows (Exclusive, Auto-match, Semantic Match, Match
category name, Actionable, and numeric Format rows). No new code
needed; just audit.

**A4. Footer hint for focused pane**:

When the user enters ViewEdit details, add a single hint token to the
footer showing which field has focus: `field: Title` / `field: Filter`
/ etc. This complements the visual indicator and is especially useful
to the screen reader / colour-stripped case. Hook point: `footer_hint_text`
in render/mod.rs, add a branch for `ViewEditRegion::Sections` with
`sections_view_row_selected == false`.

### Verification

tmux smoke (visual, use `tmux capture-pane -e` to preserve ANSI if
checking colours):

- ViewEdit → section details: press `j` through each row. Every row
  shows `▶` prefix while focused. Pressing `Enter` on the `▶` row
  opens the correct field every time (no wrong-field edits).
- Category Manager → Tab through all sub-panes: each pane shows a
  double-line border when it owns focus; single-line when idle.
- Footer bar: `field: <name>` visible during detail navigation.

---

## Fix B: Repurpose `J`/`K` for section-cursor navigation

**Problem**: `J`/`K` (board.rs:2549-2553) and `[`/`]` (board.rs:2591-2595)
**both** call `self.move_selected_item_between_slots` — exact
duplicates. New users reach for `J`/`K` expecting "bigger jump" (vim
convention) and silently move items instead of the cursor.

### Change

- `J`/`K` → section-cursor navigation (call `move_slot_cursor(+1/-1)`,
  same as `Tab` / `BackTab`).
- `[`/`]` → item move (unchanged).
- `Shift+↑` / `Shift+↓` → item move (alternate binding, already
  used elsewhere per the prior plan's notes — verify).

### Files

- `crates/agenda-tui/src/modes/board.rs` (the key handler arms at 2549-2553)
- `crates/agenda-tui/src/render/mod.rs` (footer hints, help panel text)
- `docs/process/tui-tmux-testing-procedure.md` (update key table)

### Implementation

**board.rs:2549-2553** — swap the action:

```rust
KeyCode::Char('J') => self.move_slot_cursor(1),
KeyCode::Char('K') => self.move_slot_cursor(-1),
```

Leave `[`/`]` (board.rs:2591-2595) as-is for item moves.

**Help panel** (grep `render/mod.rs` for the help text that lists
`J`/`K` — it currently says "Move item to next/prev section"). Change
to: `J/K  Jump cursor to next/prev section` and add `[/]  Move item
to next/prev section` alongside.

**Footer hint bar** (render/mod.rs `footer_hint_text`): currently omits
`J`/`K` per Observation #8. Add `J/K:jump section` once the repurpose
lands.

**Gotchas note** in `docs/process/tui-tmux-testing-procedure.md:236-239`:
delete the "`J`/`K` move items" warning and replace with the new
semantics.

### Verification

tmux smoke:

1. `j`/`k` still navigates items within a section.
2. `J`/`K` now jumps section cursor (same result as Tab/S-Tab).
3. `[`/`]` still moves items (tested with on_remove_unassign view,
   verify category stripping still occurs).
4. All existing unit tests in `modes/board.rs` that exercise J/K (grep
   for `KeyCode::Char('J')`) must be updated or replaced. Add a
   regression test asserting `J` does not change `selected_item_id()`'s
   section.

---

## Fix C: Category strip notification on item section move

**Problem**: When a view's section has `on_remove_unassign` configured
and the user moves an item out of that section (now via `[`/`]`), the
stripped categories vanish silently. Observation #3 reported Work being
stripped with no warning; undo works but users don't know to undo.

### Change

Extend `move_selected_item_between_slots` (app.rs:772 onwards) to
return a delta describing which category assignments were removed or
added. Display the delta in the status bar.

### Files

- `crates/agenda-tui/src/app.rs` (the function body + return type)
- `crates/agenda-core/src/agenda.rs` — the core
  `move_item_between_sections` / `remove_item_from_section` already
  apply the strip; surface what was removed.

### Implementation

**Core**: have `Agenda::move_item_between_sections` return a struct:

```rust
pub struct SectionMoveOutcome {
    pub removed: Vec<CategoryId>,
    pub added: Vec<CategoryId>,
}
```

`remove_item_from_section` similarly returns the `removed` vec (no
adds). The existing implementations already compute these sets
internally when applying `on_remove_unassign` / `on_add_assign`; wire
them into the return value.

**TUI**: in `move_selected_item_between_slots`, resolve the returned
IDs to category names via `agenda.store().categories()` and format:

```rust
let mut parts = Vec::new();
if !outcome.added.is_empty() {
    parts.push(format!("+{}", outcome.added.iter()
        .filter_map(|id| name_of(*id)).join(", ")));
}
if !outcome.removed.is_empty() {
    parts.push(format!("-{}", outcome.removed.iter()
        .filter_map(|id| name_of(*id)).join(", ")));
}
self.status = if parts.is_empty() {
    format!("Moved to {}", target_section_title)
} else {
    format!("Moved to {} ({})", target_section_title, parts.join(" "))
};
```

### Verification

Construct a view with two sections; source section has
`on_remove_unassign: [Work]`; target has none. Add an item with Work
assigned. `]` to move. Status bar: `Moved to Unassigned (-Work)`. Undo
(`Ctrl+Z`) restores both the section and the Work assignment.

---

## Fix D: Search discoverability hint

**Problem**: Per-section search returns "0 matches" even when items
matching the query exist in adjacent sections. `g/` (global search)
isn't advertised.

### Change

When section search concludes with 0 hits in the focused section,
*also* run a cheap global-scope count. If the global count is > 0,
append a hint to the status: `0 matches in Personal Items · 2 in other
sections (g/ to search all)`.

### Files

- `crates/agenda-tui/src/modes/board.rs` — wherever the search result
  count is computed for the footer/status. Grep `search_match_count`
  or `section_filter_result_count`.

### Implementation

After computing the focused-section count, when it's 0:

```rust
let global_count = self.count_global_matches(&query, agenda)?;
if global_count > 0 {
    self.status = format!(
        "0 matches in {sec} · {global_count} in other sections (g/ to search all)",
        sec = focused_section_title
    );
}
```

`count_global_matches` iterates all sections applying the same
predicate the per-section filter uses. Keep it cheap — early-exit when
count > 99 and show `99+`.

### Verification

Populate items across three sections so a search term matches only in
sections B and C. From section A, press `/`, type the term. Status
shows `0 matches in A · 2 in other sections (g/ to search all)`.
Press Esc, then `g/`, same query — finds both.

---

## Fix E: Promote `EmptySections` to view-level setting

**Problem**: With 4 sections, 3 empty, the board wastes 75% vertical
space on `"No items in this section."` placeholders for non-datebook
views. `EmptySections` (agenda-core/src/model.rs:891-922) is already
defined as `Show` / `Collapse` / `Hide` but is embedded only in
`DatebookConfig` (model.rs:933).

**Status check**: Confirmed — `EmptySections` currently only appears
in `DatebookConfig` (4 references, all in `model.rs:895-971`). Regular
`View` has no equivalent field. The enum's doc comment explicitly
anticipates promotion ("can later be promoted to a view-level setting
without restructuring").

### Change

Add an `empty_sections: EmptySections` field to the `View` struct
itself (or a sibling `ViewLayoutConfig` if we want to group layout
toggles later). Render code respects it for non-datebook views.

### Files

- `crates/agenda-core/src/model.rs` — add field to `View`, keep
  `DatebookConfig.empty_sections` for now but mark the datebook field
  redundant; plan to collapse both into the view-level field once
  migration is complete.
- `crates/agenda-core/src/store.rs` (or wherever View is serialized)
  — migration for existing views: default to `Show` for all existing
  views; set to `Collapse` via an explicit user toggle.
- `crates/agenda-tui/src/modes/view_edit/*` — expose the setting in
  the view details pane (new row: `Empty sections  Show/Collapse/Hide`).
- `crates/agenda-tui/src/projection.rs` or wherever sections are
  assembled — filter/collapse sections based on the setting.
- `crates/agenda-tui/src/render/mod.rs` — when a section is collapsed,
  render a single-line header (`▸ Section Title (0)`) instead of the
  full placeholder block. When hidden, skip it entirely.

### Implementation sketch

1. **Model**: `pub empty_sections: EmptySections` on `View`, default
   `Show`. Serde `#[serde(default)]` so existing serialized views
   load cleanly.
2. **ViewEdit**: add a new detail row (after Columns / Layout rows)
   with keybinding to cycle through Show → Collapse → Hide (mirroring
   datebook's existing cycle).
3. **Projection**: when building slots, for each empty section:
   - `Show`: keep as today.
   - `Collapse`: mark the slot with a `collapsed: true` flag.
   - `Hide`: skip the section entirely.
4. **Render**: collapsed slots render as a single-line header with
   `(0)` suffix; no placeholder block.
5. **Datebook**: once view-level field lands, deprecate
   `DatebookConfig.empty_sections`. For now, have the datebook projection
   prefer `view.empty_sections` but fall back to
   `datebook_config.empty_sections` if the view field is unset (or
   simpler: always prefer the view field; drop the datebook-specific
   one in a follow-up).

### Verification

- Existing datebook view with `Collapse` still collapses (behaviour
  preserved via fallback or migration).
- Non-datebook view with 4 sections, 3 empty, `empty_sections =
  Collapse`: 3 collapsed headers (1 line each) + 1 full section. Total
  screen usage ≈ 3 lines + full remaining area for the populated
  section.
- `Hide` setting: 3 sections vanish, full area for the populated one.
- Toggle cycles cleanly in ViewEdit and persists across restarts.

---

## Files Modified Summary

| Fix | Files |
|---|---|
| A | `render/mod.rs` (ViewEdit detail indicators, Category Manager border types, footer hint) |
| B | `modes/board.rs` (J/K handlers), `render/mod.rs` (help + footer), `docs/process/tui-tmux-testing-procedure.md` |
| C | `agenda-core/src/agenda.rs` (return `SectionMoveOutcome`), `agenda-tui/src/app.rs` (consume + format status) |
| D | `modes/board.rs` (global count on 0-match) |
| E | `agenda-core/src/model.rs` (View field), serialization migration, `view_edit/*` (new detail row), `projection.rs` (slot collapse), `render/mod.rs` (collapsed header style) |

---

## Verification (end-to-end)

After all fixes land:

1. `cargo build` — no warnings.
2. `cargo test --lib` — all TUI + core tests (+ new tests) pass.
3. **Tmux smoke** (full walkthrough similar to the 2026-04-15 test):
   - Every focused pane / row has a visible `▶` or double-line border.
   - `J`/`K` navigates, `[`/`]` moves items.
   - Item move between `on_remove_unassign` sections shows
     `Moved to X (-Work)` in status.
   - Section search miss shows global-match hint.
   - View with empty sections collapses them when `Collapse` is set.

---

## Out of scope / follow-ups

- Observation #4 ("Section split by child category is surprising"):
  deferred pending Fix A. Once focus is visible, the layout toggle is
  less likely to be hit accidentally. Revisit if still surfaced.
- Observation #7 (no `q` confirm): deferred; risky only in edge
  cases.
- Observation #10 (preview pane long-line wrap): separate fix,
  render-only.
