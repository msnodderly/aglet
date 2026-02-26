# Task: T023 — WhenBucket Resolution

## Context

Items in Agenda can have a `when_date` — a date/time parsed from the item's
text (e.g., "Call Sarah next Friday" → Friday's date). The When category has
virtual subcategories (Overdue, Today, Tomorrow, etc.) that items fall into
based on their `when_date` relative to the current date.

These buckets are computed at query time, not stored. An item that was "Tomorrow"
yesterday is "Today" today and "Overdue" tomorrow. The resolution function is
pure date math — no database, no engine interaction.

This function is consumed by the Query evaluator (T024) to filter items by
`virtual_include` and `virtual_exclude` criteria on Views.

## What to read

1. `spec/mvp-spec.md` §2.3 — Reserved Categories, the When category and its
   buckets: Overdue, Today, Tomorrow, ThisWeek, NextWeek, ThisMonth, Future,
   NoDate.
2. `crates/agenda-core/src/model.rs` — `WhenBucket` enum, `Item.when_date`
   (`Option<NaiveDateTime>`).
3. `crates/agenda-core/src/query.rs` — currently empty. This is where the
   implementation goes.

## What to build

**File**: `crates/agenda-core/src/query.rs`

### The resolution function

A public function that takes an item's `when_date` (`Option<NaiveDateTime>`)
and a reference date (`NaiveDate`) and returns the appropriate `WhenBucket`.

**Bucket definitions:**

- **NoDate**: `when_date` is `None`.
- **Overdue**: `when_date` is before the start of `reference_date` (i.e.,
  strictly in the past).
- **Today**: `when_date` falls on `reference_date`.
- **Tomorrow**: `when_date` falls on the day after `reference_date`.
- **ThisWeek**: `when_date` falls within the same ISO week as `reference_date`,
  but after Tomorrow. (Monday-based weeks. If today is Thursday, "this week"
  means Friday through Sunday.)
- **NextWeek**: `when_date` falls within the ISO week after `reference_date`'s
  week.
- **ThisMonth**: `when_date` falls within the same calendar month as
  `reference_date`, but after NextWeek.
- **Future**: `when_date` is after the current month.

**Edge cases to handle:**

- **Today/Tomorrow take priority over ThisWeek.** If today is Monday,
  tomorrow (Tuesday) is "Tomorrow", not "ThisWeek." ThisWeek only covers
  the remainder of the week after tomorrow.
- **Week boundary.** If today is Saturday, "ThisWeek" is just Sunday (if
  using ISO weeks where Monday is day 1). NextWeek is the full following
  Mon-Sun.
- **Month boundary.** If today is Jan 30, "ThisMonth" is Jan 30-31 (minus
  Today/Tomorrow/ThisWeek/NextWeek). Feb 1 is "Future" (unless it falls in
  NextWeek).
- **Time component.** An item dated "today at 9am" when it's now 5pm is
  still "Today", not "Overdue." The bucket is based on the date portion,
  not the time.

**Design choice — reference date vs current time:** The function takes a
`NaiveDate` reference date, not `Utc::now()`. This makes it testable
(no time-dependent tests) and lets the caller decide the timezone. The
caller will convert "now" to the user's local date before calling.

## Tests to write

1. **NoDate**: `when_date = None` → `NoDate`.
2. **Overdue**: `when_date` is yesterday → `Overdue`.
3. **Today**: `when_date` is same day as reference → `Today`.
4. **Tomorrow**: `when_date` is day after reference → `Tomorrow`.
5. **ThisWeek**: `when_date` is 3 days from reference (same ISO week) →
   `ThisWeek`. Verify it's not `Tomorrow`.
6. **NextWeek**: `when_date` is in the following ISO week → `NextWeek`.
7. **ThisMonth**: `when_date` is in the same month but past NextWeek →
   `ThisMonth`.
8. **Future**: `when_date` is in a later month → `Future`.
9. **Today takes priority over ThisWeek**: Reference is Monday,
   `when_date` is Monday → `Today` (not `ThisWeek`).
10. **Tomorrow takes priority over ThisWeek**: Reference is Monday,
    `when_date` is Tuesday → `Tomorrow` (not `ThisWeek`).
11. **Time component ignored for bucketing**: `when_date` is today at 9am,
    reference date is today → `Today` (not `Overdue` even though 9am < now).
12. **Far future**: `when_date` is a year from now → `Future`.
13. **Week boundary**: Reference is Saturday, `when_date` is Sunday →
    `ThisWeek`. `when_date` is Monday → `NextWeek`.

## What NOT to do

- **Don't implement the query evaluator** — that's T024. This function only
  resolves a single item's bucket.
- **Don't access the database** — this is pure computation.
- **Don't use `Utc::now()`** — take the reference date as a parameter.
- **Don't handle timezone conversion** — the caller provides a `NaiveDate`
  already in the user's timezone.

## Workflow

Follow `AGENTS.md`. Issue ID: `bd-2b6`.

```bash
git checkout -b task/t023-when-bucket-resolution
# Implement in crates/agenda-core/src/query.rs
# cargo test -p agenda-core && cargo clippy -p agenda-core
```

## Definition of done

- [ ] Public function resolves `Option<NaiveDateTime>` + `NaiveDate` → `WhenBucket`
- [ ] All 8 buckets correctly resolved with proper priority ordering
- [ ] Edge cases handled (Today/Tomorrow priority, week/month boundaries)
- [ ] Pure function — no database, no global state, no `Utc::now()`
- [ ] All tests pass — `cargo test -p agenda-core`
- [ ] `cargo clippy -p agenda-core` clean
- [ ] Changes limited to `crates/agenda-core/src/query.rs`
