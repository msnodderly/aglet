# Task: bd-3fl - Date parser disambiguation for this/next weekday phrases

## Context

`BasicDateParser` currently resolves relative weekday phrases with one fixed interpretation. The issue requires explicit policy modes so `this <weekday>` and `next <weekday>` behavior is configurable and deterministic. This protects user trust when interpreting common ambiguous phrases like "next Tuesday".

## What to read

1. `crates/agenda-core/src/dates.rs` - current relative weekday parsing and tests.
2. `crates/agenda-core/src/agenda.rs` - default parser construction used in create/update flows.
3. `docs/decisions/product-decisions.md` sections 20-22 - current date parser decisions and where to document defaults.
4. `AGENTS.md` and `docs/process/agent-workflow.md` - workflow and completion rules.

## What to build

Add explicit parser policy modes for `this/next <weekday>` resolution, with at least:

- `strict_next_week`: interpret `next <weekday>` as the weekday in the following week relative to the week containing the reference date.
- `inclusive_next`: interpret `next <weekday>` as the next occurrence strictly after the reference date.

Behavior expectations:

- Keep `this <weekday>` deterministic and documented for all weekdays.
- Keep existing absolute-date and time parsing behavior unchanged.
- Keep parsing deterministic and boundary-aware (no fuzzy NLP).
- Ensure there is a documented default mode used by normal agenda flows.

## Tests to write

Pin tests to reference date **2026-02-16 (Monday)** and cover:

1. `this Tuesday` under each mode.
2. `next Tuesday` under each mode.
3. Deterministic behavior table for all weekdays under each mode.
4. Existing boundary behavior still rejects false positives.
5. Existing absolute/time parsing behavior remains intact.

## What NOT to do

- Do not implement natural-language understanding beyond the declared policy modes.
- Do not change unrelated engine/query/store behavior.
- Do not alter agenda semantics outside date parsing configuration and docs updates needed for this issue.

## How your code will be used

`Agenda` create/update flows rely on `BasicDateParser` defaults. The chosen default mode will directly affect captured item `when_date` values and all downstream view bucketing behavior.

## Workflow

Follow `AGENTS.md` and `PROMPT.md`.

- Claim and close issue ID: `bd-3fl`
- Branch naming: `task/bd-3fl-next-weekday-policy`

## Definition of done

- [ ] Parser supports explicit weekday disambiguation modes.
- [ ] Default mode is documented in specs/decisions.
- [ ] Tests pin behavior on 2026-02-16 and cover both modes.
- [ ] `cargo test -p agenda-core` passes.
- [ ] `cargo clippy -p agenda-core` passes for touched scope.
