---
title: Nucleo integration notes for Aglet search and narrowing
updated: 2026-05-05
---

# Nucleo integration notes for Aglet search and narrowing

## TL;DR

`nucleo` looks like a strong fit for Aglet's interactive narrowing paths (TUI search bar, global search, category/item pickers) where users expect fuzzy matching and ranked results.

For CLI `aglet search` and strict view filtering, we should keep today's deterministic substring/UUID/category semantics by default, and optionally add a fuzzy mode.

## What nucleo provides

From upstream docs and README:

- `nucleo` is a high-level, async worker-based matcher intended for "fzf-like" large interactive lists.
- `nucleo-matcher` is the lower-level API if you only need match/score calls.
- Pattern syntax supports fuzzy atoms by default, with exact/anchored variants using `'`, `^`, `$`, and `!` (negative atoms).
- Matching supports case and normalization settings, and has a designed path for multi-column matching.

## Aglet current search/narrowing model (today)

Current shared text matching in `agenda-core::query::matches_text_search(...)` is a case-insensitive **substring** matcher over:

- item title text
- note text
- UUID / UUID-compact prefix-like contents (for hex-like query strings)
- assigned category names (when provided by the caller)

TUI and CLI both route through this shared matcher for text search behavior, giving consistent semantics.

## Fit assessment by surface area

### 1) TUI section/global search (best fit)

Good candidate for nucleo because:

- interactive, as-you-type narrowing
- often many visible candidates
- ranking matters (best items first)
- fuzzy behavior usually feels better than strict substring for discovery

Suggested approach:

- Keep exact current behavior as baseline toggle (`search_mode = substring | fuzzy`).
- Add optional fuzzy mode first in TUI only.
- For fuzzy mode, create one candidate string per item combining title + notes + category names + UUID (or multi-column fields).
- Use score ordering but keep existing section/group constraints in Aglet view logic.

### 2) Item/category picker dialogs (very good fit)

The assign/unassign/category picker flows are classic fuzzy-picker UX.

- Fuzzy + ranked results will reduce keystrokes.
- Could eventually leverage query syntax (`^`, `'`, `!`) for power users.

### 3) CLI `aglet search <query>` (mixed fit)

For scripting, strict semantics are often better and more predictable.

Recommendation:

- Keep current substring behavior as default.
- Introduce opt-in flag later (example: `--fuzzy`) returning ranked output.
- Preserve existing filters (`--blocked`, category filters, done-state) around fuzzy scorer.

### 4) View criteria `text_search` persistence (caution)

View criteria are durable/stored config. Automatically changing semantics from substring to fuzzy could be surprising and break user expectations.

Recommendation:

- Do **not** silently switch existing `text_search` criteria to fuzzy.
- If added, model fuzzy as explicit new criterion mode or per-view option.

## Proposed incremental integration plan

1. **Prototype in TUI search bar only** behind a feature flag or runtime config.
2. Add a thin adapter in `agenda-core` (or `agenda-tui`) that converts Aglet item fields into nucleo match inputs.
3. Measure latency on representative databases (1k / 10k / 50k items).
4. Add highlighting support from nucleo indices where feasible.
5. If UX is good, extend to pickers.
6. Only then consider CLI `--fuzzy`.

## Data-shaping idea (single-column vs multi-column)

Two options:

- **Single-column**: concatenate searchable fields into one text blob (fastest to implement).
- **Multi-column**: separate title, notes, categories, UUID into columns and weight/order them consistently.

Start single-column for MVP; move to multi-column when we want better ranking control (e.g., title hits > notes hits).

## Risks / caveats

- Ranking can hide matches users expect from deterministic substring ordering.
- Query syntax (`!`, `^`, `$`, `'`) may conflict with users expecting literal chars unless escaped.
- For very small lists, nucleo overhead may not beat simple substring.
- Need careful fallback behavior when fuzzy returns no hits but substring would have matched due to normalization differences.

## Recommendation

Yes—nucleo is a good fit for **interactive narrowing surfaces** in Aglet (TUI search + pickers).

Treat it as an additive capability, not a global semantic replacement:

- keep deterministic substring as default for stable CLI/view behavior
- add fuzzy as opt-in, then graduate based on user feedback and perf measurements
