# Task: T029 — BasicDateParser Absolute Date Parsing

## Context

Phase 5 turns natural-language date hints into structured `when_date` values.
T028 established the parser contract; T029 is the first concrete implementation.
This task should add only deterministic absolute-date parsing so later tasks can
layer relative phrases (T030) and time expressions (T031) without API churn.

## What to read

1. `spec/phase5-overview.md` — T029 scope and deterministic missing-year rule.
2. `spec/mvp-spec.md` §2.7 — Date parser contract and MVP supported forms.
3. `spec/mvp-tasks.md` — Phase 5 dependency chain (T029 -> T030 -> T031 -> T032).
4. `spec/prompts/t028-date-parser-trait.md` — existing contract expectations from T028.
5. `crates/agenda-core/src/dates.rs` — current `DateParser`/`ParsedDate` definitions and tests.
6. `crates/agenda-core/src/model.rs` (`Item.when_date`) and `crates/agenda-core/src/query.rs` (`resolve_when_bucket`) for downstream behavior expectations.

## What to build

**File**: `crates/agenda-core/src/dates.rs`

Implement a concrete parser for absolute date expressions only.

### Behavioral rules

- Add `BasicDateParser` implementing `DateParser`.
- Support these absolute forms in item text:
  - Month name day year (e.g., `May 25, 2026`; accept with/without comma).
  - ISO date (`2026-05-25`) or `20260525`.
  - Numeric month/day/year (`12/5/26`, MVP interpretation is M/D/YY).
  - Month name + day without year (`December 5`).
- Return `Some(ParsedDate)` when one supported absolute expression is found:
  - `datetime` must be normalized to local `NaiveDateTime` at `00:00`.
  - `span` must match the exact byte range in the original text (`[start, end)`).
- If no supported absolute date exists, return `None`.
- Missing-year rule for month+day forms must be deterministic:
  - Start with `reference_date.year()`.
  - If that resolved date is earlier than `reference_date`, roll to next year.

### Key design decisions

- Keep parser deterministic and lightweight (no fuzzy NLP behavior).
- Keep match behavior conservative: parse only clearly date-like patterns.
- Preserve trait contract from T028; do not change public parser API.

## Tests to write

Add/replace tests in `crates/agenda-core/src/dates.rs` for parser behavior.

1. **Month-name full date**: `"meet May 25, 2026"` resolves to `2026-05-25 00:00` with exact span over `May 25, 2026`.
2. **ISO date**: `"deadline 2026-05-25"` resolves to `2026-05-25 00:00`.
3. **Numeric M/D/YY**: `"ship by 12/5/26"` resolves to `2026-12-05 00:00`.
4. **Month-day without year, not past**: reference `2026-02-16`, `"December 5"` resolves to `2026-12-05 00:00`.
5. **Month-day without year, past roll-forward**: reference `2026-12-10`, `"December 5"` resolves to `2027-12-05 00:00`.
6. **Invalid date rejected**: `"2026-02-30"` returns `None`.
7. **Out-of-scope phrase rejected**: `"tomorrow"` and `"at 3pm"` return `None` in T029.
8. **No false positive text**: non-date text like `"May I ask"` returns `None`.

## What NOT to do

- Do not implement relative phrases (`today`, `tomorrow`, `next Tuesday`) — T030.
- Do not implement time parsing (`at 3pm`, `at 15:00`, `at noon`) or compound parsing — T031.
- Do not wire parser into item create/update or assignment provenance (`origin = "nlp:date"`) — T032.
- Do not modify engine/store/query/agenda modules for this task.
- Do not add new dependencies unless absolutely required; prefer std/chrono-based parsing.

## How your code will be used

- **T030** will extend the same `BasicDateParser` to resolve relative date phrases.
- **T031** will add time extraction and date+time composition on top of this absolute baseline.
- **T032** will call `BasicDateParser::parse(text, reference_date)` during item create/update, assign `ParsedDate.datetime` into `Item.when_date`, and rely on `query::resolve_when_bucket` for view bucketing.

## Workflow

Follow `AGENTS.md`. Issue ID: `bd-25j`.

```bash
git checkout -b task/t029-basic-date-parser-absolute
# Edit crates/agenda-core/src/dates.rs
# cargo test -p agenda-core && cargo clippy -p agenda-core
```

## Definition of done

- [ ] `BasicDateParser` exists in `crates/agenda-core/src/dates.rs` and implements `DateParser`.
- [ ] Supported absolute formats parse into deterministic `NaiveDateTime` values at `00:00`.
- [ ] Missing-year month/day forms use reference-year with past-date roll-forward.
- [ ] `ParsedDate.span` is accurate and test-covered.
- [ ] Relative/time-only inputs are not parsed in this task.
- [ ] No unrelated files changed.
- [ ] `cargo test -p agenda-core` passes.
- [ ] `cargo clippy -p agenda-core` is clean for this scope.
