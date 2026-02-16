# Task: T031 - BasicDateParser Time Expressions and Compound Parsing

## Context

T029 established absolute date parsing and T030 added relative date parsing.
T031 extends the same `BasicDateParser` with time extraction and date+time
composition while preserving deterministic behavior and existing parser APIs.

This task should parse supported time phrases and combine them with resolved
date matches. Wiring parser output into item create/update flow is still T032.

## What to read

1. `spec/phase5-overview.md` - T031 scope and time-only behavior.
2. `spec/mvp-spec.md` section 2.7 - supported MVP time/compound forms.
3. `spec/mvp-tasks.md` - Phase 5 chain (T029 -> T030 -> T031 -> T032).
4. `spec/prompts/t030-basic-date-parser-relative.md` - current baseline semantics.
5. `spec/design-decisions.md` section 20 - existing candidate selection and `M/D/YY` policy.
6. `crates/agenda-core/src/dates.rs` - current `BasicDateParser` implementation/tests.

## What to build

**File**: `crates/agenda-core/src/dates.rs`

Extend `BasicDateParser` to support these time forms:

- `at 3pm`
- `at 15:00`
- `at noon`

And support compound date+time expressions such as:

- `next Tuesday at 3pm`
- `May 25, 2026 at 15:00`

### Behavioral rules

- Preserve all existing absolute and relative date parsing behavior.
- If a date match is found and a supported trailing time expression is attached,
  return combined `NaiveDateTime`.
- If a date match is found without time, keep default `00:00`.
- If text has time-only input with no supported date, return `None`.
- Keep parser deterministic and conservative (no fuzzy NLP).
- Keep byte-accurate span semantics (`[start, end)`) for returned expression.
- Keep trait/API unchanged.

### Deterministic defaults

- Date-only parse -> `00:00`.
- `at noon` -> `12:00`.
- Time-only phrase (for example `at 3pm`) -> `None`.

## Tests to write

Add/adjust tests in `crates/agenda-core/src/dates.rs`:

1. **12-hour time compound**: `"next Tuesday at 3pm"` resolves to `15:00`.
2. **24-hour time compound**: `"May 25, 2026 at 15:00"` resolves to `15:00`.
3. **Noon compound**: `"today at noon"` resolves to `12:00`.
4. **Date-only still midnight**: existing date-only cases remain `00:00`.
5. **Time-only still rejected**: `"at 3pm"`, `"at 15:00"`, `"at noon"` return `None`.
6. **Invalid time rejected for compound**: date plus invalid time (for example `at 25:00`) falls back to date-only parse at `00:00`.
7. **Case-insensitive time keywords**: parse `"AT 3PM"` and `"at NOON"` in compound context.
8. **Span coverage**: for compound parse, span should cover the full matched date+time expression.

## What NOT to do

- Do not wire parser into engine/store create/update flow (T032).
- Do not add LLM/fuzzy parsing or unsupported natural-language forms.
- Do not modify engine/store/query/agenda modules for this task.

## How your code will be used

- **T032** will call `BasicDateParser::parse(text, reference_date)` during item
  create/update and store `ParsedDate.datetime` in `Item.when_date`.

## Workflow

Follow `AGENTS.md`. Issue ID: `bd-3iu`.

```bash
git checkout -b task/t031-basic-date-parser-time-and-compound
# Edit crates/agenda-core/src/dates.rs
# cargo test -p agenda-core && cargo clippy -p agenda-core
```

## Definition of done

- [ ] Supported time expressions parse in compound date+time inputs.
- [ ] Date-only behavior remains stable with default `00:00`.
- [ ] Time-only inputs return `None`.
- [ ] Compound spans are accurate and test-covered.
- [ ] No unrelated files changed.
- [ ] `cargo test -p agenda-core` passes.
- [ ] `cargo clippy -p agenda-core` is clean for this scope.
