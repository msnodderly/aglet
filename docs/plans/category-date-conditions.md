# Category Date Conditions Plan

Status: Proposed
Tracking issue: `436f17e4-35aa-4cbe-9e35-2cf17cd7cff3`

## Summary

Add category-level date conditions as a new destination-centric rule type, but
do it with bucket-based semantics first instead of arbitrary free-form date
ranges. This keeps the first implementation aligned with aglet's current
`when_date` and `WhenBucket` model and avoids conflating view filters with
category rule state.

## Goals

- Add a category condition variant that evaluates against an item's `when_date`.
- Reuse existing `WhenBucket` semantics (`Overdue`, `Today`, `Tomorrow`,
  `ThisWeek`, `NextWeek`, `ThisMonth`, `Future`, `NoDate`) for the MVP.
- Make these assignments live/non-sticky like other condition-derived output.
- Expose date-condition authoring in CLI and TUI.

## Non-Goals

- Arbitrary absolute date-range authoring in the first slice.
- Date-setting actions.
- Recurrence parsing or next-instance generation.
- Replacing view-level virtual bucket filters.

## Key Design Decision

Use a new condition variant based on view/query buckets rather than a raw
`DateRange { start, end }` MVP. Proposed shape:

```rust
Condition::WhenBuckets {
    include: HashSet<WhenBucket>,
    exclude: HashSet<WhenBucket>,
}
```

This gives us immediate leverage from already-shipped query semantics and keeps
category rules easy to explain.

## Semantics

- Condition source of truth is `item.when_date`, not merely presence of the
  reserved `When` category.
- The reserved `When` assignment remains provenance/sync state; it is not the
  bucket engine.
- Bucket evaluation depends on a reference date and must be deterministic in
  tests.
- Matching date conditions produce live auto-breaking assignments just like
  implicit-string/profile conditions.

## Engine Changes

1. Extend `Condition` enum with bucket-based date-condition support.
2. Thread a `reference_date` into engine condition evaluation.
3. Reuse `resolve_when_bucket(...)` from `query.rs` for category matching.
4. Treat date-condition matches as `AssignmentSource::AutoMatch` with a
   date-condition explanation/origin string.

## Agenda/Runtime Changes

- `process_item_save(...)` already has a `reference_date`; pass it through to
  engine processing.
- For manual reprocess, category change reprocess, and evaluate-all paths, use
  the current local date unless a test/helper provides an override.
- Keep preview/reprocess parity so move previews and save behavior agree.

## CLI/TUI UX

### CLI

Add commands such as:

- `category add-date-condition <name> --include Today --include Tomorrow`
- `category add-date-condition <name> --exclude NoDate`
- `category remove-condition <name> <index>` continues to work

### TUI

- Add date-condition creation inside the existing Conditions surface.
- Reuse picker/list patterns from view bucket editing where possible.
- Show human-readable summaries like `When in [Overdue, Today]`.

## Test Matrix

- `when_date` in `Today` bucket assigns category.
- advancing reference date causes prior `Today` assignment to auto-break.
- `NoDate` include/exclude behaves as expected.
- multiple included buckets are OR-like within the condition, while separate
  conditions remain category-level OR entries.
- preview/save parity for date-conditioned categories.
- CLI/TUI round-trip persists date conditions via SQLite JSON.

## Risks

- Today's engine APIs are not explicitly date-aware; threading reference dates
  through them is the main refactor.
- Bucket-based MVP is intentionally narrower than Lotus's full date-condition
  surface; document that clearly to avoid overclaiming.

## Follow-On

If bucket-based rules prove too coarse, add a second condition variant later
for explicit relative/absolute date ranges. Do not skip the bucket-based slice
in favor of a larger one.
