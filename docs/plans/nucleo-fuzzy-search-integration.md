---
title: Nucleo fuzzy search integration plan
status: draft
created: 2026-04-24
updated: 2026-05-09
---

# Nucleo fuzzy search integration plan

## Summary

Add opt-in fuzzy, ranked narrowing to TUI search and filter surfaces with
`helix-editor/nucleo`, while preserving deterministic substring behavior as the
default for the CLI and persisted view criteria.

## Key decisions

- Use the real workspace crates: `aglet-core` owns existing substring query
  semantics, and `aglet-tui` owns the fuzzy adapter and TUI search mode.
- Add `nucleo` only to `aglet-tui`.
- Add `SearchMode::{Substring,Fuzzy}` to TUI state, defaulting to `Substring`.
- Persist the preferred mode in `app_settings` as `tui.search_mode` with values
  `substring` and `fuzzy`; no schema migration is required.
- Set search mode only in the Global Settings (`gs`) menu with a
  `Search mode < substring|fuzzy >` row. Do not add a quick-toggle key in v1.
- Show the active search mode in TUI search/filter UI so users can tell which
  behavior is active.
- Keep `aglet search <query>` and saved view `text_search` criteria on the
  existing substring matcher.

## Behavior

- Fuzzy queries are treated as literal text; nucleo/fzf operators such as `!`,
  `^`, `$`, and quotes are not exposed in v1.
- TUI item search in fuzzy mode ranks item title/text matches with nucleo.
- Note text, assigned category names, and UUID search remain substring fallback
  channels. UUIDs are not fuzzy-scored and keep the existing hex-like substring
  rule.
- Fuzzy title matches appear before substring-fallback-only matches.
- Existing slot sort order is applied before fuzzy search and becomes the
  deterministic tie-break for fuzzy results; fuzzy rank is not overwritten by
  slot sorting while a non-empty fuzzy query is active.
- Empty queries and `Substring` mode preserve existing behavior.
- Category and picker flows preserve exact-match and create semantics.

## Implementation Targets

- Add a TUI-owned fuzzy adapter module around high-level `nucleo` worker and
  snapshot APIs.
- Apply fuzzy mode to existing interactive surfaces that already have query
  buffers: section/global search, link target filtering, item/category
  assignment filters, input-panel category filters, category manager filter,
  board/category column pickers, board add-column suggestions, and ViewEdit
  section/category filters.
- Do not render match highlights or scores in the MVP.

## Validation

- Unit-test fuzzy ranking, deterministic fallback, and literal operator queries.
- TUI tests should cover persisted default/missing mode, Global Settings
  persistence, unchanged substring behavior, fuzzy title ranking, note/category/
  UUID substring fallback, slot-sort tie-break behavior, and category
  exact/create precedence.
- Run `cargo test -p aglet-tui` plus targeted `aglet-core` query tests.

## Manual smoke test

In worktree `/Users/mds/src/aglet-nucleo-fuzzy-search-integration` on branch
`codex/nucleo-fuzzy-search-integration`:

1. Launch the TUI with a test database.
2. Open `gs`, switch `Search mode` to `fuzzy`, close settings.
3. Search with an approximate title query and confirm ranked fuzzy matches.
4. Search by note/category/UUID and confirm substring fallback still works.
5. Restart the TUI and confirm the fuzzy mode setting reloads.
