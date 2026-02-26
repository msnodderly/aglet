# Task: T028 — Define DateParser Trait

## Context

Phase 5 adds deterministic date extraction so item text can populate
`Item.when_date` and drive virtual When buckets. T028 is the contract boundary:
shape the parser interface and result model that T029-T031 will implement and
T032 will call during item create/update flows.

This task should stay small and stable. A clear contract here prevents API churn
across the rest of the date-parsing chain.

## What to read

1. `spec/phase5-overview.md` — T028 through T032 flow and `reference_date` semantics.
2. `spec/mvp-spec.md` §2.7 — DateParser + ParsedDate contract.
3. `spec/mvp-spec.md` §2.1 and §2.3 — `Item.when_date` usage and When category behavior.
4. `spec/mvp-tasks.md` — Phase 5 dependency chain (T028 → T029 → T030 → T031 → T032).
5. `crates/agenda-core/src/dates.rs` — current contract/tests; verify scope and semantics.
6. `crates/agenda-core/src/lib.rs` — module export surface.
7. `crates/agenda-core/src/model.rs` (`Item.when_date`) and `crates/agenda-core/src/query.rs`
   (`resolve_when_bucket`) for downstream compatibility.

## What to build

**File**: `crates/agenda-core/src/dates.rs`

Define the public interface only.

### Behavioral rules

- Expose a `DateParser` trait (`Send + Sync`) with a parse method that takes
  item text plus `reference_date` and returns `Option<ParsedDate>`.
- Expose a public `ParsedDate` carrying:
  - resolved absolute local `NaiveDateTime`
  - matched source span in the original text
- Contract meaning:
  - `None` => no supported date expression found
  - `Some(ParsedDate)` => expression found and resolved at parse time
- Relative language is resolved using `reference_date`; no relative token should
  leak into model/state.
- Document span semantics explicitly and keep them stable for downstream
  provenance/highlighting.

### Key design decisions

- Keep this module dependency-light (no store/engine/UI coupling).
- Keep the contract deterministic and MVP-focused; no confidence scoring,
  parser metadata envelopes, or fallback pipelines.
- Keep `ParsedDate` ergonomic for assertions (`Debug`/equality-friendly derives).
- Prefer explicit span semantics that are Rust-slice-compatible to avoid
  ambiguity in later parser tasks.

## Tests to write

Use stub/fake parser implementations in `crates/agenda-core/src/dates.rs` tests
to validate interface behavior only (not parsing logic).

1. **No-match contract**: parser returns `None` and caller can treat it as
   "no date found."
2. **Successful shape**: parser returns `Some` preserving both datetime and span.
3. **Reference-date contract**: parser can branch on `reference_date`, proving
   the trait supports deterministic relative resolution.
4. **Span round-trip**: returned `(start, end)` span can be asserted exactly and
   used for precise source extraction.

## What NOT to do

- Do not implement absolute/relative/time parsing behavior (T029-T031).
- Do not wire parser invocation into item create/update or assignment provenance (T032).
- Do not modify engine/store/query/agenda modules for this task.
- Do not add extra abstractions beyond the trait + parsed result contract.

## How your code will be used

- **T029** will add `BasicDateParser` for absolute date forms using this trait.
- **T030/T031** will extend the same implementation for relative and time forms
  without changing the API.
- **T032** will call `parse(text, reference_date)` during item create/update,
  write `ParsedDate.datetime` into `Item.when_date`, and tag provenance as
  `origin = "nlp:date"`.

## Workflow

Follow `AGENTS.md`. Issue ID: `bd-toq`.

```bash
git checkout -b task/t028-date-parser-trait
# Edit crates/agenda-core/src/dates.rs
# cargo test -p agenda-core && cargo clippy -p agenda-core
```

## Definition of done

- [ ] `DateParser` trait exists in `crates/agenda-core/src/dates.rs` with MVP contract.
- [ ] `ParsedDate` public type exists with datetime + span provenance.
- [ ] Unit tests validate interface behavior using parser stubs.
- [ ] No real date parsing behavior is implemented yet.
- [ ] No unrelated files changed.
- [ ] `cargo test -p agenda-core` passes.
- [ ] `cargo clippy -p agenda-core` is clean for this scope.
