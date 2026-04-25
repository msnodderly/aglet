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

## Design Decisions

These decisions supersede the first PR 125 tab split and should guide the
implementation:

- **Datebook config lives in Scope.** Proximity wins: when the user changes
  View type to Datebook, the next question is period/interval/anchor/date
  source. Putting that configuration in another tab breaks the creation flow.
- **Hide dependent lives in Scope.** Membership wins: this option changes which
  items appear in the view, so it belongs with the other filters.
- **Unmatched lives in Sections.** It is an implicit fallback section, not an
  appearance setting.
- **`Tab` remains local focus navigation.** Consistency wins: `Tab` moves
  between panes/preview inside the active tab; `H` / `L` changes tabs.
- **Row state should use enums, not magic numbers.** Exhaustive matching gives
  compiler help when rows move between tabs.

## Field Ownership

### Scope

Scope answers: "What gets into the view, and what kind of view is it?"

- View type: Board or Datebook.
- Datebook period, interval, anchor, and date source, shown inline when the view
  type is Datebook.
- View name.
- Category criteria: require, exclude, match-any.
- Date range include/exclude filters.
- Hide dependent items.

Rationale:

- Name and type are part of defining the lens, not presentation.
- Datebook configuration stays near the View type control because it is the next
  step after choosing Datebook.
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

- Read-only generated-section explanation.
- Summary of the current Datebook settings from Scope.
- Preview of generated section names, ideally with counts when cheap.
- Wayfinding action back to Scope's Datebook controls.

Rationale:

- Unmatched is a generated section, so it belongs here instead of Appearance.
- Datebook configuration belongs in Scope for action proximity, but Datebook
  users still need Sections to explain what will be generated.
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
- For Datebook views, `Enter` on the read-only generated-section summary may
  jump to Scope and focus the Datebook configuration. This is an edit affordance,
  not tab navigation.

## Implementation Plan

Preferred implementation base: rebuild on the clean v2 worktree/branch if it is
still the flat single-pane editor, then port only the useful PR 125 patterns
(tab enum shape, header rendering, and Sections two-pane geometry). Avoid
porting the shared `unmatched_field_index` encoding or the monolithic
tab-conditional renderer.

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
- Move Datebook period/interval/anchor/date-source controls into Scope,
  immediately below View type when the view is Datebook.
- Add a useful Datebook state to the Sections tab: generated-section
  explanation, current config summary, generated section preview, and a local
  wayfinding action back to Scope.
- Keep Display mode, Section flow, Empty sections, and Aliases in Appearance.
- Confirm new-view creation starts on the Scope tab with View type focused.

### Phase 3: Replace Numeric Cross-Tab Row Mapping

Short-term goal:

- Introduce explicit row enums for Scope, Sections view-settings, and
  Appearance.
- Use the same row list for rendering, navigation, and `Enter`/`Space` dispatch.
- Stop using `unmatched_field_index` as a shared row identifier across unrelated
  tab concepts.

Possible row types:

```rust
enum ViewScopeRow {
    ViewType,
    DatebookPeriod,
    DatebookInterval,
    DatebookAnchor,
    DatebookDateSource,
    Name,
    Criterion(usize),
    DateInclude,
    DateExclude,
    HideDependent,
}

enum ViewSectionsSettingsRow {
    ShowUnmatched,
    UnmatchedLabel,
    DatebookGeneratedPreview,
}

enum ViewAppearanceRow {
    DisplayMode,
    SectionFlow,
    EmptySections,
    AliasSummary,
}
```

The Datebook-specific Scope rows should be absent from the row list when the
draft is a Board view. The Datebook generated-preview row should be absent from
manual Board section settings.

Longer-term follow-up:

- Consider replacing `active_tab + region + pane_focus + numeric indexes` with a
  single focus model, for example `ViewEditFocus`.
- Do this only after the tab IA is stable; it is not required to ship the PR 125
  refactor safely.

### Phase 4: Split Rendering By Tab

- Keep a small `render_view_edit_screen` shell for layout, overlay placement,
  and tab dispatch.
- Extract `render_view_edit_tab_header`.
- Extract `render_view_edit_scope_tab`.
- Extract `render_view_edit_sections_tab`.
- Extract `render_view_edit_appearance_tab`.
- Each tab renderer should build rows from the same enum/list used by navigation
  and dispatch.

### Phase 5: Fix Known Review Findings

- Align Appearance focus order with rendered row order.
- Remove hidden cross-tab mutations from Scope. Keys like `m` and `w` should not
  change Appearance-only state while the user is on Scope.
- Fix datebook Appearance navigation so there are no inert bottom rows.

### Phase 6: Tests

Add or update focused TUI tests:

- `H` / `L` cycles `Scope -> Sections -> Appearance` and back.
- `1` / `2` / `3` jump directly to the expected tabs.
- `Tab` changes pane or preview focus, not tabs.
- Datebook config rows appear in Scope immediately under View type.
- Scope row order matches the rendered order.
- Appearance row order matches the rendered order.
- Pressing `m` or `w` on Scope does not dirty or mutate Appearance fields.
- Datebook views show generated-section explanation/preview under Sections.
- `Enter` on the Datebook generated-section summary jumps back to Scope's
  Datebook controls if that affordance is implemented.
- Datebook Appearance has no inert focus positions.
- Unmatched visibility and label are reachable from Sections.
- Hide dependent is reachable from Scope.

### Phase 7: Manual Smoke Test

When implementation is complete, use a worktree/branch-specific smoke test:

1. Run `cargo run --bin agenda-tui -- --db aglet-features.ag`.
2. Press `v`, then `n`.
3. Confirm the editor opens on Scope with View type focused.
4. Toggle Board/Datebook and confirm Datebook config rows appear inline under
   View type in Scope.
5. Add a category/date filter under Scope.
6. Switch tabs with `H` / `L`; jump with `1` / `2` / `3`.
7. Confirm `Tab` changes local focus instead of switching tabs.
8. On a Datebook view, open Sections and confirm the generated-section preview
   explains that sections come from Scope Datebook settings.
9. Create or edit a board section under Sections.
10. Toggle unmatched visibility and edit unmatched label under Sections.
11. Change display mode, section flow, empty-section presentation, and aliases
    under Appearance.
12. Press `S` to save and verify the rendered view matches the draft.

## Acceptance Criteria

- The tab names are `Scope / Sections / Appearance`.
- Every visible row belongs to the tab whose name best describes it.
- Focus movement follows rendered order exactly.
- Hidden keys do not mutate fields from another tab.
- `H` / `L` and `1` / `2` / `3` are documented in the header/footer.
- `Tab` remains local pane/focus navigation.
- Board and Datebook views both have coherent tab content.
- Datebook configuration is edited in Scope, while Sections provides a useful
  read-only generated-section preview and path back to the Scope config.
- Row focus is represented by named row enum variants, not shared magic numeric
  indexes.
- Existing view model persistence remains unchanged.
- Focused tests cover the navigation and row-order contracts.

## Open Questions

- Should name and view type eventually move into a persistent header, allowing
  `Scope` to become `Filters`? This is attractive but should be a later
  experiment.
- Should aliases eventually live near category manager/category display settings
  instead of ViewEdit? For now, keep them in Appearance because they are
  persisted view-level display metadata.
