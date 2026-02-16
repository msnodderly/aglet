# Risks

Updated: 2026-02-16

## Purpose

This document tracks the highest technical and product risks in Agenda Reborn MVP, with practical mitigations and places where creative improvements are most valuable.

## Risk Register

| ID | Risk | Severity | Why it matters | Current signal | Mitigation | Innovative improvement |
| --- | --- | --- | --- | --- | --- | --- |
| R1 | Silent data decode fallback in store layer | High | Invalid UUID/date/JSON can be silently defaulted, hiding corruption and causing wrong behavior | `store.rs` uses fallback patterns like `unwrap_or_default` in row mapping | Replace fallbacks with typed decode errors; fail fast on invalid rows | Add a quarantine table for corrupt rows plus a repair command that proposes fixes |
| R2 | Rule engine correctness and termination complexity | High | Fixed-point requeue + actions + cycle handling can create subtle bugs and non-obvious outcomes | Engine tasks (T017-T022) are not yet implemented | Define invariants before implementation; add targeted unit tests per invariant | Build an evaluation trace graph for each processed item to explain every step |
| R3 | Weak explainability for classification and rules | High | Core UX depends on user trust in auto-organization; opaque decisions reduce adoption | Inspect/provenance UI is planned later (US8) | Persist provenance details from day one in engine outputs | Add a first-class "why" timeline per item (condition hit, action fired, source path) |
| R4 | JSON-heavy schema evolution risk | Medium | Query/sections/columns/actions stored as JSON limits SQL validation and migration safety | `views`/category rule fields are JSON blobs | Add strict schema versioning and migration tests for JSON payload evolution | Introduce compile/validate step for queries/rules into normalized internal IR |
| R5 | Matcher recall/precision ceiling | Medium | ASCII word-boundary matching misses synonyms/morphology and can over/under-match in edge text | `SubstringClassifier` is intentionally simple (T015/T016) | Keep lexical matcher deterministic for MVP, add explicit tests for edge cases | Hybrid pipeline: lexical first, semantic fallback with confidence + provenance labels |
| R6 | Clippy baseline not warning-clean | Medium | `-D warnings` fails on existing `large_enum_variant`, hiding new lint regressions in CI | Clippy currently requires local exception to pass | Decide lint policy now: fix model shape or allow specific lint with rationale | Add lint budget with per-lint justifications checked in CI |
| R7 | Reserved/default bootstrap coupling in init path | Medium | Startup seeds core categories and view; future schema changes may accidentally break idempotence | Init logic now creates reserved categories + "All Items" view | Keep idempotence tests and add migration-path tests across versions | Add startup self-check report that verifies required entities and repairs safely |
| R8 | Regression risk from sparse property-based testing | Medium | Rule systems have combinatorial edge cases hard to cover with example-only tests | Mostly example-driven unit tests today | Add scenario matrix tests for exclusivity/subsumption/remove interactions | Introduce property-based tests (termination, monotonicity, exclusivity invariants) |

## Top Priorities

1. Eliminate silent decode fallback in persistence (`R1`).
2. Define engine invariants and traceability before deep engine build-out (`R2`, `R3`).
3. Establish explicit lint and migration policy to prevent quality drift (`R4`, `R6`).

## Immediate Next Actions

1. Create a `StoreDecodeError` path and remove default-on-parse-failure behavior.
2. Write an engine invariant spec note before T017 implementation.
3. Add a minimal provenance event struct now, even if UI wiring comes later.
4. Decide and document clippy policy for `large_enum_variant` in `model.rs`.
