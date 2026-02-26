# Task: T030 — BasicDateParser Relative Date Parsing

## Context

T029 added deterministic absolute date parsing to `BasicDateParser`. T030 extends
the same parser with relative date phrases while preserving the existing trait
contract and absolute behavior.

This task remains date-only. Time parsing and full date+time compound resolution
are handled in T031.

## What to read

1. `spec/phase5-overview.md` — relative date scope and weekday semantics.
2. `spec/mvp-spec.md` §2.7 — supported MVP relative forms.
3. `spec/mvp-tasks.md` — Phase 5 chain (T029 -> T030 -> T031 -> T032).
4. `spec/prompts/t029-basic-date-parser-absolute.md` — current parser baseline.
5. `spec/design-decisions.md` §20 — existing candidate selection and M/D/YY policy.
6. `crates/agenda-core/src/dates.rs` — `BasicDateParser` implementation and tests.

## What to build

**File**: `crates/agenda-core/src/dates.rs`

Extend `BasicDateParser` to parse deterministic relative date expressions.

### Behavioral rules

- Preserve all T029 absolute-date behavior.
- Add support for:
  - `today`
  - `tomorrow`
  - `yesterday`
  - `this <weekday>`
  - `next <weekday>`
- Weekday semantics:
  - `this <weekday>`: next occurrence on or after `reference_date`.
  - `next <weekday>`: next occurrence strictly after `reference_date`.
- Return resolved local `NaiveDateTime` at `00:00`.
- Keep `ParsedDate.span` as exact byte offsets (`[start, end)`).
- If no supported absolute/relative date is found, return `None`.
- Keep parser deterministic and conservative (word boundaries, no fuzzy NLP).

### Selection behavior

- Keep existing parser selection rule from T029:
  - Earliest match in text wins.
  - If candidates share start offset, longer span wins.

## Tests to write

Update tests in `crates/agenda-core/src/dates.rs` to cover relative parsing:

1. **today** resolves to reference date at midnight with exact span.
2. **tomorrow** resolves to reference date +1 day.
3. **yesterday** resolves to reference date -1 day.
4. **this weekday (same day)** includes today when weekday matches.
5. **this weekday (roll forward)** resolves to upcoming weekday when not same day.
6. **next weekday (same day)** resolves to +7 days.
7. **next weekday (different day)** resolves to next occurrence strictly after.
8. **Case-insensitive phrases** parse (`NEXT tuesday`, `This Friday`).
9. **Boundary/no-false-positive** text like `todayish` or `annext tuesday` returns `None`.
10. Keep/adjust T029 out-of-scope coverage so **time-only** input (`at 3pm`) is still `None`.

Use table-driven tests where practical for weekday boundary coverage.

## What NOT to do

- Do not parse time expressions (`at 3pm`, `at 15:00`, `at noon`) — T031.
- Do not implement full date+time compound resolution — T031.
- Do not wire parser into create/update flows or assignment provenance — T032.
- Do not modify engine/store/query/agenda modules for this task.

## How your code will be used

- **T031** will extend this parser with time extraction and compound composition.
- **T032** will invoke `BasicDateParser::parse(text, reference_date)` and write
  `ParsedDate.datetime` into `Item.when_date`.

## Workflow

Follow `AGENTS.md`. Issue ID: `bd-1mq`.

```bash
git checkout -b task/t030-basic-date-parser-relative
# Edit crates/agenda-core/src/dates.rs
# cargo test -p agenda-core && cargo clippy -p agenda-core
```

## Definition of done

- [ ] `BasicDateParser` supports all T030 relative phrases.
- [ ] Weekday semantics match Phase 5 definitions exactly.
- [ ] Absolute-date behavior from T029 remains intact.
- [ ] `ParsedDate.span` remains accurate for relative phrases.
- [ ] Time-only inputs remain out of scope in T030.
- [ ] No unrelated files changed.
- [ ] `cargo test -p agenda-core` passes.
- [ ] `cargo clippy -p agenda-core` is clean for this scope.
