---
title: TUI UX Fixes (Phase 2)
status: draft
created: 2026-04-15
updated: 2026-04-17
---

> **Phase 1 status**: shipped as commit `12265e0` on branch
> `feat/tui-ux-phase-2-focus`, followed by a hotfix that (a) wired
> **Ctrl-S** save across InputPanel focuses (bare `S` is typed as a
> character in note/text fields and cannot be the advertised save key),
> (b) corrected the canonical hint surface to the bottom footer
> (`footer_hint_pairs()` at `crates/agenda-tui/src/render/mod.rs:4573`),
> and (c) added `> ` title markers to the ViewEdit Sections / Details /
> Preview panes, which were previously color-only and invisible in
> plain `tmux capture-pane -p` output. Phase 2 and Phase 3a/3b remain
> pending.

# TUI UX Fixes: Focus, Navigation, Polish

## Context

Synthesized from the 2026-04-15 / 2026-04-16 tmux smoke tests captured in
`docs/plans/tui-ux-observations-2026-04-15.md` (19 findings, 6 significant).
This plan supersedes the earlier A–E draft on branch
`docs/tui-ux-phase-2-plan` and the locally-evolved revision dated
2026-04-18. Observations references below use `#N` from the observations doc.

Ordering reflects severity (data-loss > wrong-field edits > discoverability >
polish). Each phase is independently shippable.

## Priorities

1. **Focus/cursor clarity first.** Every focused row, pane, and editable
   field must be unmistakable in plain `tmux capture-pane` output — no
   color-only cues.
2. **Navigation and destructive-move feedback.** `J`/`K` repurposed to
   section-jump (currently redundant duplicates of `[`/`]`); surface
   removed categories when section moves strip via `on_remove_unassign`.
3. **Search, view state, and label polish.** Per-section search zero-match
   hint, global-search header preservation, system-view label, picker
   legend, wrapped-note indentation.
4. **State/contract changes kept minimal.** Only promotion of the existing
   `EmptySections` enum from `DatebookConfig` to a view-level field.

## Phase 1 — Focus and cursor clarity

Addresses observations #2, #9, #12, #17, #18.

### Focus convention (applies globally)

- Focused row: visible `> ` gutter prefix **plus** bold/reverse style.
- Focused pane: stronger title marker (`> Details`) and border style.
  Marker is a text prefix, not color — must be visible under plain
  `tmux capture-pane -p`. Applies to ViewEdit Sections, Details,
  Preview; Category Manager Tree / Details / Flags / Note / Also Match /
  Conditions / Actions.
- Inactive panes **must not** render row-level `>` markers.
- Editable fields preserve explicit terminal cursor placement.

### ViewEdit details

Today, only fields at indices 0, 1, 6 get a selected-row style via
`section_details_field_index` (`crates/agenda-tui/src/render/mod.rs:8570-8579`).
Extend coverage to all detail row types and expose the active field in
the footer:

- Persistent `> ` prefix on view fields, criteria rows, datebook rows,
  unmatched rows, and section detail rows.
- Footer context string such as `field: Filter` or `field: Section layout`.
- Preserve inline text cursor for view name, section title, unmatched
  label, section filter, and category alias edits.

### Category Manager

At `crates/agenda-tui/src/render/mod.rs:7123-7135` the `> Details` title is
gated on `CategoryManagerFocus::Details`, but flag-row `>` markers and
contextual help render regardless of focus (#18). Fix:

- Gate flag-row `>` prefix on `CategoryManagerFocus::Details`.
- Make Note, Also Match, Conditions, Actions, and Numeric Format focus
  visually obvious when active.
- Rephrase flag help as `If enabled, …` (not `Only one child can be
  assigned…`) so inactive flags don't read as enabled.
- Preserve explicit cursor for filter editing and inline rename.

### InputPanel copy audit

**There are two hint surfaces in the InputPanel popup and they render
simultaneously.** The canonical, user-visible one is the two-row
footer at the bottom of the screen, produced by `footer_hint_pairs()`
(`crates/agenda-tui/src/render/mod.rs:4573` for the `Mode::InputPanel`
arm). A second, popup-internal help row renders inside the popup body
(`render/mod.rs:5413-5436`). Every copy change below applies to
**both** — audits that only touch the popup-internal row leave the
stale text visible in the footer and do not land the fix.

**Save key: Ctrl-S in any field that accepts typed characters
(Text, Note, When).** Bare `S` is a letter keystroke in those focuses
and is consumed by the TextBuffer — advertising `S:save` there is
actively misleading. Bare `S` is reserved for non-typing modal
contexts elsewhere (e.g. ViewEdit Sections/Details list navigation
when no inline edit is active). See
`.claude/projects/-Users-mds-src-aglet/memory/feedback_ctrl_s_in_text_fields.md`.

Target copy (footer and popup-internal must match):

| Focus / state | Hint |
|---|---|
| Text (item title / view name / category name) | `Enter:save  Tab:next  Ctrl-S:save  Ctrl-G:$EDITOR  Esc:cancel` |
| Note | `Enter:new line  Tab:next  Ctrl-S:save  Ctrl-G:$EDITOR  Esc:cancel` |
| When | `Ctrl-S:save  Tab:next  Ctrl-G:$EDITOR  Esc:cancel` |
| Categories | `Space:toggle  /:filter  Tab:next  Ctrl-S:save  Esc:cancel` |
| Actions / Suggestions | `Ctrl-S:save  Tab:next  Esc:cancel` |
| NameInput (single-line name) | `Enter:save  Tab:next  Esc:cancel` |
| WhenDate / NumericValue (single-line override) | `Enter:save  Esc:cancel` |

Popup-internal help row strings (`render/mod.rs:5413-5436`) must use
`Ctrl-S:save` wherever the corresponding footer entry above does. In
particular, the rows previously written as `Esc:save and close` or
`S:save  Esc:cancel` in multi-line contexts must read `Ctrl-S:save
Esc:cancel`.

Ctrl-S is handled at the top of `InputPanel::handle_key_event` before
any focus routing (`crates/agenda-tui/src/input_panel.rs`), so it
works from every focus including fields that consume letter keys.

### Tests (Phase 1)

Render tests via `TestBackend`. Copy assertions must target the
footer hint row — extract it explicitly (`&lines[lines.len() - 2]`)
and assert on that slice. Substring-on-the-whole-frame assertions
match both `S:save` and `Ctrl-S:save` and will miss regressions.

- ViewEdit section details show `> Filter`, `> Columns`, and related rows
  as focus moves; footer shows active field name.
- ViewEdit pane titles include `>` prefix when focused (Sections /
  Details / Preview).
- Category Manager Tree-active: no flag-row `>`.
- Category Manager Details-active: flag-row `>` present; Note / Also
  Match / Conditions / Actions / Numeric Format focused pane titles
  render with `>`.
- CategoryCreate panel footer shows `Enter:save` and `Ctrl-S:save`;
  forbid bare ` S:save`.
- AddItem / EditItem footer in Note focus shows `Ctrl-S:save`; forbid
  bare ` S:save`. Ctrl-S from Note focus returns
  `InputPanelAction::Save`; bare `S` in Note focus inserts an `S`
  character into the buffer.

## Phase 2 — Navigation and section-move feedback

Addresses observations #1, #3.

### Repurpose `J` / `K` on the board

Today both `J`/`K` and `[`/`]` call `move_selected_item_between_slots`
(`crates/agenda-tui/src/modes/board.rs:2503-2549`) — `J`/`K` are
redundant. Repurpose them to section cursor jump:

- `J` = next section (same path as `Tab`).
- `K` = previous section (same path as `BackTab`).
- `[` / `]` and `Shift-Up` / `Shift-Down` remain item moves.
- **Keep** the ViewEdit Sections mode `J`/`K` = section-reorder mapping at
  `crates/agenda-tui/src/modes/view_edit/sections.rs:315-342`; that
  surface is unambiguous and the user is in a structural editor, not the
  board.

Update `?` help panel, footer hints, and
`docs/process/tui-tmux-testing-procedure.md`.

### Section-move outcome and status feedback

`move_item_between_sections` (`crates/agenda-core/src/agenda.rs:592-608`)
applies `on_remove_unassign` silently today. Change:

- Return a `SectionMoveOutcome { added: Vec<CategoryId>, removed:
  Vec<CategoryId> }` (or extend `ProcessItemResult` with those fields).
- TUI formats the status as `Moved to <section> (-Work +Personal)` with
  deterministic ordering (sort by name).
- If empty, omit the parenthetical.

### Tests (Phase 2)

- `J` / `K` change focused section without moving the selected item.
- `[` / `]` still move items between sections.
- `move_item_between_sections` returns the expected
  `added`/`removed` categories for both `on_remove_unassign` and
  `on_add_assign` paths.
- Help panel, footer hints, and tmux procedure doc match the new key
  meanings.

## Phase 3a — Contract/state changes

Addresses observation #6 (data-model promotion), #13 (view-save flow).

### Promote `EmptySections` to view-level

`EmptySections { Show, Collapse, Hide }` already exists at
`crates/agenda-core/src/model.rs:891-917`, currently on `DatebookConfig`
(line 928). Promote it:

- Add `empty_sections: EmptySections` to `View` in
  `crates/agenda-core/src/model.rs`.
- Default to `Show` for existing views on read (migration).
- Datebook keeps its field (datebook-specific behavior is already layered
  on top via `effective_empty_sections()` at
  `crates/agenda-tui/src/render/mod.rs:2327, 2382, 2408`).
- Wire **board rendering only** in this pass. No change to query
  semantics, membership, or section counts.
- Expose an editor row in ViewEdit details next to the existing datebook
  row at `crates/agenda-tui/src/modes/view_edit/details.rs:385`.

### View save flow

After saving a newly-created view from ViewEdit:

- Close the palette and switch the board to the new view unless save
  failed.
- Status reads: `Created and switched to view "<name>"`.

### Tests (Phase 3a)

- `View { empty_sections: Collapse }` renders empty lanes as header +
  one-line indicator; `Hide` elides them entirely; `Show` unchanged.
- Migration / store round-trip preserves the value (and defaults old
  rows to `Show`).
- View-create save path transitions view selection and palette state
  atomically.

## Phase 3b — Cosmetic polish

Addresses observations #5, #11, #14, #15, #16, #19.

- **Section-search zero-match hint**: when section search returns 0
  matches, include `g/:search all sections` in the footer/status
  (renders at `crates/agenda-tui/src/render/mod.rs:4511-4525`).
- **Global-search header**: keep the current view name; append
  `search: global` scope marker rather than showing `All Items`.
- **System-view lane label**: rename the default single-lane label from
  `Unassigned` to `All Items` (or `Items`) in the system view only
  (`crates/agenda-core/src/model.rs:1309`). Real unmatched lanes in
  custom views keep `Unassigned`. Requires coordinating with test
  fixtures at `crates/agenda-core/src/store.rs:97, 2647, 2715, 2750` and
  `query.rs:1029, 1045, 1061`.
- **Assignment picker legend**: one-line legend explaining `[+]`, `[-]`,
  `[x]`, `[ ]`. No behavior change.
- **Preview note wrapping**: preserve note-body indentation on wrapped
  continuation lines.

### Tests (Phase 3b)

- Section search with 0 matches includes the `g/:` hint in footer text.
- Global-search header retains the view name.
- System `All Items` view renders the default lane as `All Items`; a
  custom view with an unmatched lane still renders `Unassigned`.
- Wrapped note preserves leading indentation on continuation lines.

## Deferred / explicit non-goals

- **#4 Split-by-child default**: needs a separate design decision on
  section-layout defaults; not in this phase.
- **#7 Quit confirmation**: lower priority than focus/nav/search; track
  separately.
- **#8 Footer hint bar completeness**: the 2-row footer already lands
  (per project memory, 2026-02-21); adding `J/K`, `Tab`, `c`, `C-z`, `z`
  to Normal-mode hints is cosmetic and can ride with any of the above
  phases opportunistically, not gated work.
- **No CLI changes.** No schema changes beyond the `empty_sections`
  promotion.

## Interfaces and data model

- `SectionMoveOutcome` (or extension of `ProcessItemResult`): internal
  Rust type, no serialization.
- `View.empty_sections: EmptySections`: persisted, default `Show`,
  backwards-compatible on read.

## Test plan

```bash
cargo test -p agenda-tui view_edit
cargo test -p agenda-tui category_manager
cargo test -p agenda-tui footer
cargo test -p agenda-tui search
cargo test -p agenda-tui
cargo test -p agenda-core
```

Run `cargo test -p agenda-core` whenever Phase 2 (section-move outcome)
or Phase 3a (empty-section persistence) lands.

## Manual tmux smoke test

Use `docs/process/tui-tmux-testing-procedure.md` with a fresh DB.

Smoke flow:

1. Create a fresh DB and launch TUI.
2. Create a custom view with two filtered sections; verify ViewEdit
   details focus visible in `capture-pane -p` output (Phase 1).
3. Open Category Manager; switch focus Tree → Filter → Details and
   confirm focus markers move with focus (Phase 1).
4. Open CategoryCreate; confirm footer shows `Enter:save` and
   `Ctrl-S:save`, not bare ` S:save` (Phase 1).
5. Add/edit an item; in Note focus, confirm footer shows `Ctrl-S:save`
   and that pressing `S` inserts the letter while `Ctrl-S` saves and
   closes. Verify explicit text cursor (Phase 1).
6. Press `J` / `K` on the board and confirm section-focus jumps without
   moving the item; press `[` / `]` and confirm item move still works
   (Phase 2).
7. Move an item between sections with `on_remove_unassign`; verify
   status shows removed categories (Phase 2).
8. Set a view's `empty_sections` to `Collapse` and confirm empty lanes
   render as one line (Phase 3a).
9. Create a new view from ViewEdit; confirm automatic switch and
   `Created and switched to view "<name>"` status (Phase 3a).
10. Search for a non-existent term in a section and confirm the
    `g/:search all sections` hint (Phase 3b).
11. Run global search and confirm the current view name remains in the
    header with a `search: global` marker (Phase 3b).

## Notes

- `docs/specs/proposals/tui-ux-redesign.md` remains the long-term design
  target; this plan does not depend on it.
- ViewEdit implementation is rooted at
  `crates/agenda-tui/src/modes/view_edit/`.
- Cursor/focus work must be validated with plain `tmux capture-pane`,
  not only color-aware local rendering.
