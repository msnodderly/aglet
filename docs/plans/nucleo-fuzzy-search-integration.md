---
title: Nucleo fuzzy search integration plan
status: draft
created: 2026-04-24
updated: 2026-05-05
---

# Nucleo fuzzy search integration plan

## Objective

Add optional fuzzy, ranked narrowing with `helix-editor/nucleo` for interactive Aglet surfaces (TUI search bar and pickers) while preserving deterministic substring search as the default for CLI and persisted view criteria.

## Current baseline (source of truth)

- Text search semantics are centralized in `agenda-core::query::matches_text_search(...)`.
- Current behavior is case-insensitive substring matching over item text, note text, UUID-ish strings, and optionally assigned category names.
- TUI section/global search currently filters slot items through this matcher.

## Non-goals

- No breaking semantic change to existing `aglet search <query>` default behavior.
- No implicit migration of saved view criteria `text_search` from substring to fuzzy.
- No broad rewrite of query resolution logic in this phase.

## High-level architecture

### 1) Search mode model

Introduce explicit search mode selection:

- `Substring` (existing behavior, default)
- `Fuzzy` (new, opt-in)

Initial scope:

- TUI search bar and global search session only.
- Keep CLI and view criteria on `Substring` until later phases.

### 2) Adapter boundary

Create a narrow adapter module for fuzzy matching that:

- Accepts candidate items + query text.
- Produces ranked item IDs and optional highlight spans.
- Leaves filtering semantics (view sections, blocked/dependent filtering, etc.) to existing projection pipeline.

### 3) Candidate representation

Start with single-string candidate composition:

`"{item.text} {note?} {category_names_joined} {item.id}"`

Later phase can upgrade to weighted multi-column matching.

## Phased implementation

## Phase 0 — dependency and scaffolding

1. Add `nucleo` dependency to relevant crate(s) (likely `agenda-tui`, optionally `agenda-core` helper).
2. Add `SearchMode` enum in TUI app state with default `Substring`.
3. Add UI affordance to toggle mode in search bar/global search contexts.
4. Keep all existing tests green.

Exit criteria:

- Build passes with feature compiled in.
- Default UX unchanged when mode is `Substring`.

## Phase 1 — TUI fuzzy narrowing MVP

1. Implement `fuzzy_filter_and_rank(items, query, categories)` helper.
2. Integrate into `project_slots` filter path when active mode is `Fuzzy`.
3. Preserve existing behavior for empty query and `Substring` mode.
4. Add ranking-stability guardrails (deterministic tie-break using existing order/ID).

Tests:

- New unit tests for adapter scoring/ranking determinism.
- TUI behavior tests for:
  - fuzzy mode returns expected approximate matches,
  - substring mode remains byte-for-byte current semantics,
  - no-result behavior unchanged.

Exit criteria:

- Fuzzy mode works in section search + global search.
- No regression in existing substring tests.

## Phase 2 — picker surfaces

1. Reuse same adapter for category/item pickers (`Mode::ItemAssignInput` and related search flows).
2. Keep existing Enter precedence rules and create-new-category behavior.
3. Ensure visible-row selection and dirty-session semantics remain intact.

Tests:

- Picker tests for fuzzy ranking and exact-match precedence.
- Regression tests for "partial search text should not create new category" behavior.

Exit criteria:

- Picker UX improved without breaking existing command semantics.

## Phase 3 — optional CLI fuzzy mode (separate decision gate)

1. Add `--fuzzy` to `aglet search` only (not default).
2. Keep current output contract and filters; fuzzy only changes text matching/ranking.
3. Document scoring/ranking expectations.

Tests:

- CLI parse tests for `--fuzzy`.
- Integration tests verifying default search unchanged and fuzzy mode opt-in behavior.

Exit criteria:

- Script-safe defaults preserved.
- Fuzzy mode explicitly requested by user.

## Data model and persistence impact

- **No DB schema changes** required for Phases 0–2.
- Optional future: persist per-view or global preferred search mode only if product decision requires it.

## Performance plan

Measure on representative datasets: 1k, 10k, 50k items.

Metrics:

- keystroke-to-render latency (P50/P95)
- peak memory during active search
- ranking stability across repeated queries

Guardrails:

- Keep substring fallback path available.
- If latency exceeds threshold, auto-fallback to substring for pathological queries/list sizes.

## UX and behavior decisions to lock early

1. Query syntax policy: whether to expose raw nucleo operators (`!`, `^`, `$`, `'`) in UI help.
2. Ranking policy: title hits weighted above notes/categories (future multi-column step).
3. Tie-break policy: stable order by prior slot order then UUID.
4. Escape hatch: quick toggle key to switch `Substring <-> Fuzzy` during search.

## Risks and mitigations

- **Risk:** fuzzy ranking may hide expected substring matches.
  - **Mitigation:** mode toggle + clear label in header/footer.
- **Risk:** semantic drift between TUI and CLI search.
  - **Mitigation:** keep default substring shared path; make fuzzy explicitly scoped.
- **Risk:** picker behavior regressions.
  - **Mitigation:** preserve existing Enter precedence and expand regression tests.

## Deliverables checklist

- [ ] `SearchMode` state + UI toggle in TUI.
- [ ] Fuzzy adapter module + tests.
- [ ] TUI slot projection integration.
- [ ] Picker integration.
- [ ] Bench/latency notes committed under `docs/reference/`.
- [ ] Optional CLI `--fuzzy` proposal/plan update before implementation.

## Rollout and validation

1. Ship behind opt-in mode first.
2. Gather user feedback on ranking quality.
3. Promote fuzzy default only after explicit product decision and migration plan.

## Open questions

1. Should fuzzy mode be global for the TUI session or per-search session?
2. Do we expose score/highlight details in UI now or later?
3. Is a per-view stored search mode worth adding, given persistence complexity?
