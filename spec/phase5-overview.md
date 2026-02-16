# Phase 5: Date Parsing

## The big picture

Phase 4 made **Views** work, including virtual **When buckets** (Overdue, Today,
ThisWeek, ...). But buckets are only as useful as the data behind them. Right
now, items have `when_date = None` unless something sets it.

Phase 5 builds a small, deterministic date parser that extracts a date/time
from item text and populates `Item.when_date`. This turns natural language like
"next Tuesday at 3pm" into structured data that Views can query and section.

This is library code in `agenda-core`. No UI in this phase.

## What the user experiences

### Before Phase 5

The user can type:

```
Call Sarah next Friday at 3pm
```

But nothing sets `when_date`, so:

- The When column has no date to show.
- Views that section by virtual When buckets show the item as `NoDate`.

### After Phase 5

The user types the same text and `when_date` is auto-populated.
Now the item shows up under the right virtual bucket in a "My Week" style view.

## What gets built

Phase 5 corresponds to **US3** in `spec/mvp-tasks.md`.

### T028 — DateParser trait

Define a small trait in `agenda-core/src/dates.rs`:

- Input: item text + `reference_date` (usually "today")
- Output: `Option<ParsedDate>`, where:
  - `ParsedDate.datetime` is the parsed local `NaiveDateTime`
  - `ParsedDate.span` is the matched character range in the original text
- All relative expressions ("next Tuesday", "tomorrow") are resolved to absolute
  `NaiveDateTime` values at parse time. No relative strings are stored.

### T029 — BasicDateParser: absolute dates

Implement a basic parser that recognizes common absolute forms (MVP list from
`spec/mvp-spec.md`):

- "May 25, 2026"
- "2026-05-25"
- "12/5/26" (M/D/YY for MVP)
- "December 5" (year inferred from `reference_date`)

For partial dates without a year, the parser must be deterministic. A simple
MVP rule: use `reference_date.year()` unless that date is already in the past,
then roll forward to the next year.

### T030 — BasicDateParser: relative dates

Extend the parser to handle:

- "today", "tomorrow", "yesterday"
- "this <weekday>", "next <weekday>"

For MVP, define weekday semantics precisely:

- "this <weekday>": the next occurrence of that weekday on or after
  `reference_date` (including today if it matches).
- "next <weekday>": the next occurrence strictly after `reference_date`
  (so on Wednesday, "next Tuesday" means the following Tuesday, +6 days;
  on Tuesday, "next Tuesday" means +7 days).

### T031 — BasicDateParser: time expressions + compound

Support time expressions and combining them with a date:

- "at 3pm", "at 15:00", "at noon"
- "next Tuesday at 3pm"

If a date is present without a time, choose a fixed default time (MVP can use
`00:00`). If a time is present without a date, MVP ignores it (no parse).

> **Deferred**: A future phase could merge a standalone time expression into an
> existing `when_date` (e.g., "at 3pm" updates the time component). For MVP,
> time-only input produces no parse result.

### T032 — Wire the parser into item create/update

On item creation and text updates:

1. Run the parser on the item text with a `reference_date` (the caller's "today").
2. If a date is found, populate `Item.when_date` before the engine runs.
3. Record provenance by assigning the item to the reserved **When** category with
   `source = AutoMatch` and `origin = "nlp:date"`. This assignment is for
   provenance/inspection only — bucket resolution uses `when_date` directly via
   `virtual_include`/`virtual_exclude`, not this assignment.
4. Do not store bucket assignments. Buckets remain virtual and are computed from
   `when_date` at query time.
5. If no date is found, do not auto-clear `when_date` (leave it unchanged).

> **Caveat**: This preserves manually-set dates but can leave stale parser-set
> dates. Example: user types "meet tomorrow" → parser sets `when_date`; user
> edits to "meet about project" → stale date persists. For MVP this is
> acceptable. A future refinement could auto-clear only when the item has an
> existing `origin = "nlp:date"` When assignment (i.e., the parser set it).

This wiring is what makes Phase 4's When buckets light up for real data.

## Dependency chain

```
T028 (DateParser trait)
  └→ T029 (absolute dates)
       └→ T030 (relative dates)
            └→ T031 (times + compound)
                 └→ T032 (wire into create/update)
```

## Phase checkpoint

When Phase 5 is complete:

- Common date phrases in item text populate `when_date` automatically
- Relative expressions resolve against an explicit `reference_date`
- Views using virtual When buckets place items in the expected sections
- Date parsing is deterministic and unit-tested (no UI required)

## What's next

- Phase 5 has no dependency on Phase 4. They share no files and can be built in
  any order.
- Phase 6 (CLI) depends on Phase 5: `agenda add` should parse dates before it
  runs the classification engine and writes the item.
