# Task: T028 — Define DateParser Trait

## Context

Phase 5 introduces date intelligence to the core library so natural-language
item text can populate `Item.when_date`. This task is the contract layer only:
define the parser interface and output shape that later tasks will implement
and wire into the engine flow.

A clean, stable trait here prevents churn in downstream tasks. T029-T031 will
add parsing behavior against this API, and T032 will integrate parser results
into item create/update paths.

## What to read

1. `spec/phase5-overview.md` — T028 section (trait contract, `ParsedDate`,
   `reference_date` semantics)
2. `spec/mvp-spec.md` §2.7 — DateParser + ParsedDate model contract
3. `spec/mvp-spec.md` §2.1 and §2.3 — how `when_date` is represented and used
4. `spec/mvp-tasks.md` — Phase 5 task chain (T028 → T029 → T030 → T031 → T032)
5. `crates/agenda-core/src/dates.rs` — currently empty target module
6. `crates/agenda-core/src/lib.rs` — module export surface
7. `crates/agenda-core/src/model.rs` (`Item.when_date`) and
   `crates/agenda-core/src/query.rs` (`WhenBucket` resolution) for downstream context

## What to build

**File**: `crates/agenda-core/src/dates.rs`

Define the public date-parsing interface used by the rest of the system.

### Required API surface

- Add a public `DateParser` trait with:
  - `Send + Sync` bounds
  - a single parse method taking item text and `reference_date`
  - return type `Option<ParsedDate>`
- Add a public `ParsedDate` type carrying:
  - parsed `NaiveDateTime`
  - source text span as `(usize, usize)` character range

### Behavioral contract to encode (docs + type semantics)

- `None` means "no date expression found".
- `Some(ParsedDate)` means "a date expression was found and resolved to an
  absolute local `NaiveDateTime`".
- Relative expressions are resolved at parse time using `reference_date`
  (implementation comes in later tasks, but the contract belongs here).
- Span represents where the matched expression came from in the input text, so
  downstream features can inspect/highlight provenance.

### Design constraints

- Keep this module dependency-light: no store, engine, or UI coupling.
- Keep the API deterministic and MVP-oriented; do not add optional confidence
  scoring, parser metadata objects, or suggestion queues.
- Make `ParsedDate` ergonomically testable (derives appropriate for equality/
  debug assertions).

## Tests to write

Add focused unit tests in `crates/agenda-core/src/dates.rs` using small fake
parsers to validate the interface contract (not parsing logic).

1. **No-match contract**: A parser implementation returning `None` is accepted
   and treated as "no date found."
2. **Successful parse shape**: A parser implementation returning `Some` carries
   both `datetime` and `span` intact.
3. **Reference date is part of API**: A parser implementation can branch on
   `reference_date`, proving the trait shape supports deterministic relative
   resolution.
4. **Span round-trip sanity**: Returned span values can be asserted as exact
   `(start, end)` tuples in tests (no hidden transformation).

These tests should compile and pass before any real parser implementation exists.

## What NOT to do

- **Do not implement date parsing behavior** — absolute/relative/time parsing is
  T029-T031.
- **Do not wire parser calls into item create/update flows** — that is T032.
- **Do not modify engine/store/agenda/query code in this task.**
- **Do not add extra parser abstractions** (confidence models, parser pipelines,
  fallback engines, external crates) beyond the MVP trait + result type.

## How your code will be used

- **T029** will introduce `BasicDateParser` implementing `DateParser` for
  absolute date formats.
- **T030/T031** will extend that implementation for relative and time
  expressions without changing the trait contract.
- **T032** will call `parse(text, reference_date)` during item create/update,
  then write `ParsedDate.datetime` into `Item.when_date` and provenance into
  assignments (`origin = "nlp:date"`).

## Workflow

Follow `AGENTS.md`. Issue ID: `bd-toq`.

```bash
git checkout -b task/t028-date-parser-trait
# Edit crates/agenda-core/src/dates.rs
# cargo test -p agenda-core && cargo clippy -p agenda-core
```

## Definition of done

- [ ] `DateParser` trait exists in `crates/agenda-core/src/dates.rs` with MVP contract
- [ ] `ParsedDate` public type exists with `datetime` and `span`
- [ ] Unit tests validate interface behavior using parser stubs
- [ ] No parsing logic implemented yet (kept scoped to interface/data shape)
- [ ] No unrelated files changed
- [ ] All tests pass — `cargo test -p agenda-core`
- [ ] `cargo clippy -p agenda-core` clean
