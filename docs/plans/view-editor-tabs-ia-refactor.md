---
title: View Editor Tab IA Refactor
status: draft
created: 2026-04-25
updated: 2026-04-25
---

# View Editor Tab IA Refactor

## Context

PR 125 introduces a tabbed ViewEdit shell to reduce the clutter of the previous
single details pane. The direction is good: users need separate places for
deciding what a view contains, how it is organized, and how it looks.

The first pass uses `Criteria / Sections / Display`. Review found that this is
an improvement over one crowded pane, but the tab names and field placement do
not fully match the user's mental model:

- `Criteria` now contains view type and name, so the label is too narrow.
- `Display` contains behavior and generated-bucket controls, so the label is
  too broad in some places and too narrow in others.
- Implementation state still relies on legacy `region` plus numeric row indexes,
  which makes focus order and hidden cross-tab actions easy to get wrong.

This plan refactors the PR 125 experiment into a clearer information
architecture while keeping the current view model and rule semantics.

## Target Layout

Use three tabs:

1. **Scope**
2. **Sections**
3. **Appearance**

This gives the editor a simple conceptual rhythm:

- Scope: what the view covers.
- Sections: how matching items are organized.
- Appearance: how the view is presented.

### Alternatives Considered

`General / Sections / Display`

- Reject for now. `General` is vague and risks becoming a miscellaneous drawer.

`Setup / Filter / Sections / Display`

- Reject for now. Four tabs over-split the current surface. `Setup` would mostly
  hold name and type unless identity is promoted to a persistent header.

`Filters / Sections / Display`

- Good if name and view type move into a persistent header. That is a reasonable
  later direction, but it is a larger layout change than PR 125 needs.

`Scope / Sections / Display`

- Good, but `Appearance` is more user-facing than `Display` for aliases,
  presentation, and layout choices.

## Field Ownership

### Scope

Scope answers: "What items can appear in this view?"

- View type: Board or Datebook.
- View name.
- Category criteria: require, exclude, match-any.
- Date range include/exclude filters.
- Hide dependent items.

Rationale:

- Name and type are part of defining the lens, not presentation.
- Hide dependent items changes membership, so it belongs with filters rather
  than Appearance.

### Sections

Sections answers: "How are the matching items organized?"

For board views:

- Section list.
- Add, delete, and reorder sections.
- Section title.
- Section filter criteria.
- Section columns.
- Section display override.
- Auto-assign on add.
- Auto-unassign on remove.
- Section layout: flat or split by direct child category.
- Unmatched bucket visibility and label.

For datebook views:

- Read-only/generated sections explanation.
- Datebook period.
- Datebook interval.
- Datebook anchor.
- Date source.

Rationale:

- Unmatched is a generated section, so it belongs here instead of Appearance.
- Datebook period/interval controls define generated section structure, not
  visual presentation.
- Section columns are stored per section, so they belong with section editing
  even though they affect visible output.

### Appearance

Appearance answers: "How should this view look when rendered?"

- View default display mode.
- Section flow: vertical stacked sections or horizontal lanes.
- Empty-section presentation: show, collapse, hide.
- Category display aliases.

Rationale:

- These controls do not change which items match the view.
- Empty-section behavior is presentation of generated/resolved sections, so it
  fits better here than in Scope.

## Tab Navigation Decision

Keep `H` / `L` as the primary tab switch keys, with `1` / `2` / `3` as direct
jump shortcuts.

Do not make `Tab` switch tabs in this editor.

### Why `H` / `L` Is Acceptable

- `Tab` already has useful local meaning: move between panes or focus the
  preview inside the active tab.
- A tabbed editor can still have nested focus. Reusing `Tab` for top-level tab
  switching would make the details/sections/preview model harder to reason
  about.
- Uppercase `H` / `L` signals a larger structural movement than row-level
  `h` / `l` or `j` / `k`.
- Direct number shortcuts make the system recoverable and quick even if users
  forget the lateral key.

### Keys To Avoid

- Lowercase `h` / `l`: too easy to hit accidentally for users with Vim muscle
  memory, and likely to collide with future horizontal/detail behavior.
- `[` / `]`: already used for section reordering and bucket picker semantics.
- `Ctrl-H` / `Ctrl-L`: less discoverable and more terminal-dependent.
- `gt` / `gT`: familiar to some Vim users but too obscure for this UI.

### Required UX Support

- The header tab strip must show stable numbering: `1 Scope`, `2 Sections`,
  `3 Appearance`.
- Footer hints should always include `H/L:tab` and `1-3:jump` when no overlay or
  inline input is active.
- Overlay keymaps keep their local meanings. For example, `1` / `2` / `3` still
  mean require/exclude/or inside criteria pickers.
- `Tab` and `Shift-Tab` remain pane/focus movement inside the active tab.

## Implementation Plan

### Phase 1: Rename The Tabs

- Rename `ViewEditTab::Criteria` to `Scope`.
- Rename `ViewEditTab::Display` to `Appearance`.
- Keep `ViewEditTab::Sections`.
- Update header labels, footer hints, statuses, tests, and docs that mention
  `Criteria / Sections / Display`.
- Preserve `1` / `2` / `3` ordering.

### Phase 2: Move Fields Into Honest Tabs

- Move Hide dependent into Scope.
- Move Show unmatched and Unmatched label into Sections.
- Move Datebook period/interval/anchor/date-source controls into Sections.
- Keep Display mode, Section flow, Empty sections, and Aliases in Appearance.
- Confirm new-view creation starts on the Scope tab with View type focused.

### Phase 3: Replace Numeric Cross-Tab Row Mapping

Short-term goal:

- Introduce explicit row enums or row-list helpers for Scope and Appearance.
- Use the same row list for rendering, navigation, and `Enter`/`Space` dispatch.
- Stop using `unmatched_field_index` as a shared row identifier across unrelated
  tab concepts.

Possible row types:

```rust
enum ViewScopeRow {
    ViewType,
    Name,
    Criterion(usize),
    DateInclude,
    DateExclude,
    HideDependent,
}

enum ViewAppearanceRow {
    DisplayMode,
    SectionFlow,
    EmptySections,
    AliasSummary,
}
```

Longer-term follow-up:

- Consider replacing `active_tab + region + pane_focus + numeric indexes` with a
  single focus model, for example `ViewEditFocus`.
- Do this only after the tab IA is stable; it is not required to ship the PR 125
  refactor safely.

### Phase 4: Fix Known Review Findings

- Align Appearance focus order with rendered row order.
- Remove hidden cross-tab mutations from Scope. Keys like `m` and `w` should not
  change Appearance-only state while the user is on Scope.
- Fix datebook Appearance navigation so there are no inert bottom rows.

### Phase 5: Tests

Add or update focused TUI tests:

- `H` / `L` cycles `Scope -> Sections -> Appearance` and back.
- `1` / `2` / `3` jump directly to the expected tabs.
- `Tab` changes pane or preview focus, not tabs.
- Scope row order matches the rendered order.
- Appearance row order matches the rendered order.
- Pressing `m` or `w` on Scope does not dirty or mutate Appearance fields.
- Datebook views show generated section controls under Sections.
- Datebook Appearance has no inert focus positions.
- Unmatched visibility and label are reachable from Sections.
- Hide dependent is reachable from Scope.

### Phase 6: Manual Smoke Test

When implementation is complete, use a worktree/branch-specific smoke test:

1. Run `cargo run --bin agenda-tui -- --db aglet-features.ag`.
2. Press `v`, then `n`.
3. Confirm the editor opens on Scope with View type focused.
4. Toggle Board/Datebook and confirm generated section controls appear under
   Sections for Datebook.
5. Add a category/date filter under Scope.
6. Switch tabs with `H` / `L`; jump with `1` / `2` / `3`.
7. Confirm `Tab` changes local focus instead of switching tabs.
8. Create or edit a board section under Sections.
9. Toggle unmatched visibility and edit unmatched label under Sections.
10. Change display mode, section flow, empty-section presentation, and aliases
    under Appearance.
11. Press `S` to save and verify the rendered view matches the draft.

## Acceptance Criteria

- The tab names are `Scope / Sections / Appearance`.
- Every visible row belongs to the tab whose name best describes it.
- Focus movement follows rendered order exactly.
- Hidden keys do not mutate fields from another tab.
- `H` / `L` and `1` / `2` / `3` are documented in the header/footer.
- `Tab` remains local pane/focus navigation.
- Board and Datebook views both have coherent tab content.
- Existing view model persistence remains unchanged.
- Focused tests cover the navigation and row-order contracts.

## Open Questions

- Should name and view type eventually move into a persistent header, allowing
  `Scope` to become `Filters`? This is attractive but should be a later
  experiment.
- Should aliases eventually live near category manager/category display settings
  instead of ViewEdit? For now, keep them in Appearance because they are
  persisted view-level display metadata.
