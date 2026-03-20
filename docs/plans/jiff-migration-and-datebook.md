# Jiff Migration and Datebook View

**Status:** Draft for review
**Branch:** `claude/evaluate-jiff-library-M0vi2`
**Scope:** Replace `chrono` with `jiff` across `agenda-core`; implement datebook calendar view in `agenda-tui`

---

## Background

Aglet currently uses `chrono` v0.4 throughout. All date/time values are stored and manipulated as
`NaiveDate`/`NaiveDateTime` — civil (timezone-unaware) types. This has worked fine, but two
factors motivate a migration:

1. **Jiff's `Span` type** handles calendar arithmetic (months, weeks, days together as a single
   unit) correctly and ergonomically. Chrono's `Duration` is purely time-based; calendar-unit
   arithmetic requires manual decomposition and is error-prone. This becomes critical for recurring
   events and Lotus Agenda-style date filters.

2. **The datebook view** (planned, see below) requires richer date math: "every four months
   starting Tuesday", "a week from today", date-range filters, and eventually iCal/CalDAV sync.
   Jiff is designed around these exact use cases (it follows the TC39 Temporal proposal).

Jiff is pre-1.0 (targeting Spring/Summer 2026). The API is largely stable but not guaranteed.
We accept this risk given:
- The civil time types (`jiff::civil::Date`, `jiff::civil::DateTime`) map 1-to-1 to our current
  chrono types
- The type boundary is narrow — date values flow through a small number of well-defined seams
- Our extensive test suite will catch regressions

---

## Type Mapping

| Chrono type | Jiff equivalent | Notes |
|---|---|---|
| `NaiveDate` | `jiff::civil::Date` | Direct analog; same civil semantics |
| `NaiveDateTime` | `jiff::civil::DateTime` | Direct analog |
| `DateTime<Utc>` | `jiff::Timestamp` | UTC instant; RFC 3339 compatible |
| `Duration` (day offsets) | `jiff::Span` | Far more capable; handles months/weeks |
| `Weekday` | `jiff::civil::Weekday` | Same enum values, different methods |
| `.num_days_from_monday()` | `.to_monday_zero_offset()` | Returns `i8` instead of `u32` |

---

## Migration Phases

### Phase 0: Spike (prerequisite)

Before touching production code, validate that jiff's serde integration, SQLite round-trip
format, and span arithmetic produce correct results:

```toml
# Cargo.toml (spike crate or test module)
[dependencies]
jiff = { version = "0.2", features = ["serde"] }
```

Confirm:
- `jiff::civil::DateTime` serialises to the same `"YYYY-MM-DD HH:MM:SS"` format currently stored
  in SQLite, OR determine the migration format and write a schema migration
- `jiff::Timestamp` round-trips via RFC 3339 (it does; this is identical to chrono's `DateTime<Utc>`)

---

### Phase 1: Add Jiff as a Dependency (no behaviour change)

Add jiff alongside chrono so the two can coexist during migration. Do not remove chrono yet.

```toml
# crates/agenda-core/Cargo.toml
[dependencies]
chrono  = { version = "0.4", features = ["serde"] }   # keep until migration complete
jiff    = { version = "0.2", features = ["serde"] }
```

At this stage, no code changes. The goal is to get a clean compile and ensure there are no
dependency conflicts.

---

### Phase 2: Migrate `dates.rs` — Internal Types Only

`dates.rs` is the largest and most self-contained seam. The NLP parsing logic itself is
library-agnostic (byte scanning, string matching); the only chrono dependency is the _output_
types `NaiveDate`, `NaiveDateTime`, and `Weekday`, and `Duration::days()` for offset arithmetic.

**`ParsedDate` struct** — change the stored datetime type:

```rust
// Before
pub struct ParsedDate {
    pub datetime: NaiveDateTime,
    pub span: (usize, usize),
}

// After
pub struct ParsedDate {
    pub datetime: jiff::civil::DateTime,
    pub span: (usize, usize),
}
```

**`DateParser` trait** — update the reference date and return type:

```rust
// Before
pub trait DateParser: Send + Sync {
    fn parse(&self, text: &str, reference_date: NaiveDate) -> Option<ParsedDate>;
}

// After
pub trait DateParser: Send + Sync {
    fn parse(&self, text: &str, reference_date: jiff::civil::Date) -> Option<ParsedDate>;
}
```

**Weekday constants** — replace `chrono::Weekday` with `jiff::civil::Weekday`:

```rust
// Before
use chrono::Weekday;
const WEEKDAYS: [(&str, Weekday); 7] = [
    ("monday",    Weekday::Mon),
    ("tuesday",   Weekday::Tue),
    // ...
];

// After
use jiff::civil::Weekday;
const WEEKDAYS: [(&str, Weekday); 7] = [
    ("monday",    Weekday::Monday),
    ("tuesday",   Weekday::Tuesday),
    // ...
];
```

**Offset arithmetic** — replace `Duration::days()` with `Span::new().days()`:

```rust
// Before (scan_relative_dates)
if let Some(date) = reference_date.checked_add_signed(Duration::days(day_offset)) {

// After
if let Ok(date) = reference_date.checked_add(Span::new().days(day_offset)) {
```

**Weekday delta arithmetic** — the core of `days_until_weekday_this` / `days_until_weekday_next`
uses `num_days_from_monday()`. Jiff's equivalent is `to_monday_zero_offset()` which returns `i8`:

```rust
// Before
fn days_until_weekday_this(current: Weekday, target: Weekday) -> i64 {
    let current_idx = current.num_days_from_monday() as i64;
    let target_idx  = target.num_days_from_monday()  as i64;
    (target_idx - current_idx + 7) % 7
}

// After
fn days_until_weekday_this(current: Weekday, target: Weekday) -> i64 {
    let current_idx = current.to_monday_zero_offset() as i64;
    let target_idx  = target.to_monday_zero_offset()  as i64;
    (target_idx - current_idx + 7) % 7
}
```

**`at_midnight` helper** — construct a `civil::DateTime` from a `civil::Date`:

```rust
// Before
fn at_midnight(date: NaiveDate) -> NaiveDateTime {
    date.and_hms_opt(0, 0, 0).expect("midnight time is valid")
}

// After
fn at_midnight(date: jiff::civil::Date) -> jiff::civil::DateTime {
    date.at(0, 0, 0, 0)  // hour, minute, second, subsecond_nanosecond
}
```

**`attach_trailing_time`** — uses `date()` and `and_hms_opt()`:

```rust
// Before
let date     = parsed.datetime.date();
let datetime = date.and_hms_opt(time.hour, time.minute, 0)
    .expect("validated time should be valid");

// After
let date     = parsed.datetime.date();
let datetime = date.at(time.hour as i8, time.minute as i8, 0, 0);
```

**`resolve_month_day_without_year`** — uses `.year()` and `from_ymd_opt()`:

```rust
// Before
fn resolve_month_day_without_year(reference: NaiveDate, month: u32, day: u32) -> Option<NaiveDate> {
    let this_year      = reference.year();
    let this_year_date = NaiveDate::from_ymd_opt(this_year, month, day)?;
    if this_year_date < reference {
        NaiveDate::from_ymd_opt(this_year + 1, month, day)
    } else {
        Some(this_year_date)
    }
}

// After
fn resolve_month_day_without_year(
    reference: jiff::civil::Date,
    month: u8,
    day: u8,
) -> Option<jiff::civil::Date> {
    let this_year      = reference.year();
    let this_year_date = jiff::civil::Date::new(this_year, month as i8, day as i8).ok()?;
    if this_year_date < reference {
        jiff::civil::Date::new(this_year + 1, month as i8, day as i8).ok()
    } else {
        Some(this_year_date)
    }
}
```

All 40+ tests in `dates.rs` should pass without semantic change — only type signatures change.

---

### Phase 3: Migrate `model.rs` — Item Struct

The `Item` struct carries the two date fields that flow through the entire system:

```rust
// Before
pub struct Item {
    pub created_at:  DateTime<Utc>,
    pub modified_at: DateTime<Utc>,
    pub when_date:   Option<NaiveDateTime>,
    pub done_date:   Option<NaiveDateTime>,
    // ...
}

// After
pub struct Item {
    pub created_at:  jiff::Timestamp,
    pub modified_at: jiff::Timestamp,
    pub when_date:   Option<jiff::civil::DateTime>,
    pub done_date:   Option<jiff::civil::DateTime>,
    // ...
}
```

`jiff::Timestamp` serialises as RFC 3339 by default with the `serde` feature — identical to how
`DateTime<Utc>` serialises — so existing JSON serialised state (API payloads, undo log) is
unaffected.

`Assignment::assigned_at` and `ItemLink::created_at` also use `DateTime<Utc>` and follow the same
pattern: swap to `jiff::Timestamp`.

---

### Phase 4: Migrate `store.rs` — SQLite Persistence

#### Storage format for `when_date` / `done_date`

Currently stored as `NaiveDateTime::to_string()` which produces `"YYYY-MM-DD HH:MM:SS"`.
Jiff's `civil::DateTime` `Display` produces `"YYYY-MM-DDTHH:MM:SS"` (ISO 8601 with a `T`
separator). **These formats differ.**

**Option A — Schema migration (recommended):** At `SCHEMA_VERSION = 11`, run a data migration to
rewrite existing rows into ISO 8601 format. This is the cleanest approach.

```sql
-- Migration: version 10 → 11
-- Rewrite when_date / done_date from "YYYY-MM-DD HH:MM:SS" to "YYYY-MM-DDTHH:MM:SS"
UPDATE items
   SET when_date = REPLACE(when_date, ' ', 'T')
 WHERE when_date IS NOT NULL;

UPDATE items
   SET done_date = REPLACE(done_date, ' ', 'T')
 WHERE done_date IS NOT NULL;
```

**Option B — Custom serialisation:** Keep the space-separated format by using a custom
`strptime`/`strftime` wrapper. This avoids a migration but adds boilerplate:

```rust
fn parse_civil_datetime(s: &str) -> Result<jiff::civil::DateTime> {
    // Try ISO 8601 with T first, fall back to space separator
    jiff::civil::DateTime::strptime("%Y-%m-%dT%H:%M:%S", s)
        .or_else(|_| jiff::civil::DateTime::strptime("%Y-%m-%d %H:%M:%S", s))
        .map_err(|e| AgendaError::DateParse(e.to_string()))
}
```

Option B is simpler to deploy (no migration) and handles both old and new rows during a
transition window. **Recommendation: start with Option B, then simplify to Option A once all
rows have been rewritten.**

#### `created_at` / `modified_at` / `assigned_at`

These are already RFC 3339. `jiff::Timestamp::from_str()` parses RFC 3339 natively, and
`jiff::Timestamp::to_string()` emits RFC 3339 — no format change needed.

```rust
// Before
fn row_to_item(row: &Row) -> rusqlite::Result<Item> {
    let created_at: String = row.get("created_at")?;
    let created_at = DateTime::<Utc>::parse_from_rfc3339(&created_at)
        .map_err(|_| rusqlite::Error::InvalidColumnType(...))?
        .with_timezone(&Utc);
    // ...
}

// After
fn row_to_item(row: &Row) -> rusqlite::Result<Item> {
    let created_at: String = row.get("created_at")?;
    let created_at: jiff::Timestamp = created_at.parse()
        .map_err(|_| rusqlite::Error::InvalidColumnType(...))?;
    // ...
}
```

---

### Phase 5: Migrate `query.rs` — `WhenBucket` Logic

`resolve_when_bucket` and `start_of_iso_week` are the two functions that need updating.

**`start_of_iso_week`** becomes simpler with jiff. Jiff's `Date::iso_week_date()` gives the ISO
week; we can reconstruct the Monday:

```rust
// Before
fn start_of_iso_week(date: NaiveDate) -> NaiveDate {
    date.checked_sub_signed(Duration::days(date.weekday().num_days_from_monday() as i64))
        .expect("valid ISO week start")
}

// After
fn start_of_iso_week(date: jiff::civil::Date) -> jiff::civil::Date {
    let offset = date.weekday().to_monday_zero_offset() as i64;
    date.checked_sub(Span::new().days(offset))
        .expect("valid ISO week start")
}
```

**`resolve_when_bucket`** — update the signature and `.date()` call:

```rust
// Before
pub fn resolve_when_bucket(
    when_date: Option<NaiveDateTime>,
    reference_date: NaiveDate,
) -> WhenBucket {
    let when_day = when_datetime.date();
    // ...
    let this_week_end = this_week_start
        .checked_add_signed(Duration::days(6))
        .expect("valid week range");

// After
pub fn resolve_when_bucket(
    when_date: Option<jiff::civil::DateTime>,
    reference_date: jiff::civil::Date,
) -> WhenBucket {
    let when_day = when_datetime.date();   // civil::DateTime::date() → civil::Date ✓
    // ...
    let this_week_end = this_week_start
        .checked_add(Span::new().days(6))
        .expect("valid week range");
}
```

The `succ_opt()` call for tomorrow becomes:

```rust
// Before
if let Some(tomorrow) = reference_date.succ_opt() {

// After
if let Ok(tomorrow) = reference_date.checked_add(Span::new().days(1)) {
```

---

### Phase 6: Remove Chrono

Once all crates compile clean against jiff with no remaining `use chrono::` imports, remove chrono
from all `Cargo.toml` files and verify tests pass.

---

## Datebook View

This is the new feature enabled by the migration. The Lotus Agenda reference describes the goal:
items with dates visible in a calendar grid, with recurring events, relative date filters, and
date-range queries.

### Data Model Additions

Add an optional `recurrence` field to `Item`:

```rust
/// Rule for repeating an item after it is completed or on a schedule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecurrenceRule {
    /// The base interval span, e.g. Span::new().months(1) for monthly.
    pub interval: SerializableSpan,
    /// Anchor date — the first occurrence; subsequent ones are computed from this.
    pub anchor: jiff::civil::Date,
    /// Optional end date; None means indefinite.
    pub until: Option<jiff::civil::Date>,
}
```

Because `jiff::Span` doesn't implement `Serialize`/`Deserialize` out of the box with serde in a
round-trip-stable way, wrap it:

```rust
/// Serialisable representation of a jiff Span for storage.
/// Stores only calendar units (years, months, weeks, days) — time units are
/// not needed for recurrence rules.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SerializableSpan {
    pub years:  i32,
    pub months: i32,
    pub weeks:  i32,
    pub days:   i32,
}

impl SerializableSpan {
    pub fn to_span(&self) -> jiff::Span {
        jiff::Span::new()
            .years(self.years)
            .months(self.months)
            .weeks(self.weeks)
            .days(self.days)
    }
}
```

Store `recurrence` as a JSON column in the `items` table (schema version 12):

```sql
ALTER TABLE items ADD COLUMN recurrence_json TEXT;
```

### Recurring Event Logic

When an item with a `RecurrenceRule` is marked done, the engine creates the next occurrence:

```rust
/// Compute the next occurrence date after `from_date` given a recurrence rule.
pub fn next_occurrence(
    rule: &RecurrenceRule,
    from_date: jiff::civil::Date,
) -> Option<jiff::civil::Date> {
    let span  = rule.interval.to_span();
    let mut d = rule.anchor;

    loop {
        let next = d.checked_add(span).ok()?;
        if next > from_date {
            if let Some(until) = rule.until {
                if next > until {
                    return None;
                }
            }
            return Some(next);
        }
        d = next;
    }
}
```

This handles "every 4 months starting Tuesday" — just set `anchor` to the first Tuesday and
`interval` to `SerializableSpan { months: 4, .. }`.

### Extended NLP Parser

Extend `dates.rs` to detect recurrence phrases and return them alongside the anchor date.

Add a new return variant:

```rust
pub enum ParseResult {
    /// A single resolved date/time.
    Once(ParsedDate),
    /// A date with an attached recurrence rule.
    Recurring(ParsedDate, RecurrenceRule),
}
```

Phrases to detect (examples):

| Input | Anchor | Interval |
|---|---|---|
| `"every week starting Friday"` | next Friday | `days: 7` |
| `"every month on the 15th"` | next 15th | `months: 1` |
| `"every 4 months starting Tuesday"` | next Tuesday | `months: 4` |
| `"every other week"` | today | `weeks: 2` |
| `"weekly"` | today | `weeks: 1` |
| `"daily"` | today | `days: 1` |

Scanner sketch:

```rust
fn scan_recurrence_phrases(
    bytes: &[u8],
    reference_date: jiff::civil::Date,
    best: &mut Option<ParseResult>,
) {
    // Match: "every" <N?> <unit> ("starting" <date>)?
    for start in 0..bytes.len() {
        if !has_left_boundary(bytes, start) { continue; }
        if !matches_ascii_insensitive(bytes, start, b"every") { continue; }

        let mut pos = skip_whitespace(bytes, start + 5);

        // Optional count: "every 4 months"
        let (count, pos) = if let Some((n, end)) = parse_digits(bytes, pos, 1, 2) {
            (n as i32, skip_whitespace(bytes, end))
        } else {
            (1, pos)
        };

        // Unit keyword
        let (unit, unit_end) = match () {
            _ if matches_ascii_insensitive(bytes, pos, b"day")   => (TimeUnit::Day,   pos + 3),
            _ if matches_ascii_insensitive(bytes, pos, b"week")  => (TimeUnit::Week,  pos + 4),
            _ if matches_ascii_insensitive(bytes, pos, b"month") => (TimeUnit::Month, pos + 5),
            _ if matches_ascii_insensitive(bytes, pos, b"year")  => (TimeUnit::Year,  pos + 4),
            _ => continue,
        };

        // Skip optional plural 's' and "other" (e.g. "every other week")
        // ...

        // Optional "starting <date>" anchor
        // ...

        let interval = match unit {
            TimeUnit::Day   => SerializableSpan { days: count, ..Default::default() },
            TimeUnit::Week  => SerializableSpan { weeks: count, ..Default::default() },
            TimeUnit::Month => SerializableSpan { months: count, ..Default::default() },
            TimeUnit::Year  => SerializableSpan { years: count, ..Default::default() },
        };
        // ...
    }
}
```

### Date Range Filter in Queries

Extend `Query` to support date range predicates (for "a week from today" style filters):

```rust
/// An optional absolute date range constraint on when_date.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct DateRangeFilter {
    pub from: Option<jiff::civil::Date>,   // inclusive
    pub until: Option<jiff::civil::Date>,  // inclusive
}
```

Add to `Query`:

```rust
pub struct Query {
    // ... existing fields ...
    pub date_range: Option<DateRangeFilter>,
}
```

Evaluate in `item_matches_query`:

```rust
if let Some(range) = &query.date_range {
    let item_day = match item.when_date {
        Some(dt) => dt.date(),
        None     => return false,
    };
    if let Some(from) = range.from {
        if item_day < from { return false; }
    }
    if let Some(until) = range.until {
        if item_day > until { return false; }
    }
}
```

This enables view sections like "a week from today":

```rust
// Compute dynamically when building the view, using jiff's span arithmetic:
let from  = reference_date;
let until = reference_date.checked_add(Span::new().weeks(1)).unwrap();
let filter = DateRangeFilter { from: Some(from), until: Some(until) };
```

### TUI: Calendar Grid View

Add a new `ViewMode::Datebook` to `agenda-tui`. The calendar grid renders items bucketed by their
`when_date` into a month or week layout.

#### Month grid layout

```
      March 2026
 Mo  Tu  We  Th  Fr  Sa  Su
                           1
  2   3   4   5   6   7   8
  9  10  11  12  13  14  15
 16  17  18  19  20  21  22
 23  24  25  26  27  28  29
 30  31
```

Each cell shows a count badge if items are due that day; selecting a cell opens a detail panel
listing those items.

Key rendering data structure:

```rust
pub struct CalendarMonth {
    pub year:  i16,
    pub month: i8,
    /// Sparse map from day-of-month to items due that day.
    pub items_by_day: BTreeMap<i8, Vec<Item>>,
    /// The reference (today) date for highlighting.
    pub reference_date: jiff::civil::Date,
}

impl CalendarMonth {
    /// Build from a flat item list. Items without when_date are excluded.
    pub fn from_items(
        year: i16,
        month: i8,
        items: &[Item],
        reference_date: jiff::civil::Date,
    ) -> Self {
        let mut items_by_day: BTreeMap<i8, Vec<Item>> = BTreeMap::new();
        for item in items {
            if let Some(dt) = item.when_date {
                let d = dt.date();
                if d.year() == year && d.month() == month as i8 {
                    items_by_day.entry(d.day()).or_default().push(item.clone());
                }
            }
        }
        CalendarMonth { year, month, items_by_day, reference_date }
    }
}
```

Jiff provides the first-day-of-month and days-in-month needed for grid layout:

```rust
let first = jiff::civil::Date::new(year, month, 1).unwrap();
let days_in_month = first
    .days_in_month();           // jiff built-in, handles leap years

let start_col = first.weekday().to_monday_zero_offset(); // 0=Mon ... 6=Sun
```

#### Week grid layout

Provides a 7-column, N-row view of a single ISO week with time-of-day rows for items that have a
time component. This is the view most similar to a traditional calendar app.

```rust
pub struct CalendarWeek {
    pub week_start: jiff::civil::Date, // always a Monday
    pub items_by_day: [Vec<Item>; 7],  // index 0 = Monday
}
```

Navigation: left/right arrows move by week; `m` key switches to month view.

#### Keyboard shortcuts (proposed)

| Key | Action |
|---|---|
| `d` | Jump to datebook view |
| `←` / `→` | Previous / next week or month |
| `t` | Jump to today |
| `m` | Toggle month / week layout |
| `Enter` | Open items for selected day |
| `n` | New item pre-filled with selected date |

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Jiff API breaks before 1.0 | Low–Medium | Medium | Pin to a specific version; review changelog on upgrade |
| SQLite format mismatch causes silent data corruption | Low (with tests) | High | Add explicit round-trip tests in store.rs for both formats; use Option B dual-parse |
| `civil::DateTime` serde format differs from chrono's | Low (verified in spike) | High | Confirm in Phase 0 spike before any production change |
| Weekday offset arithmetic regression | Low (covered by test suite) | Medium | Existing 40+ tests in dates.rs pin all edge cases |
| Recurrence rule creates unbounded loops | Low | Medium | Cap iteration in `next_occurrence` at e.g. 10,000 steps |

---

## Implementation Order

1. **Phase 0** — Spike: confirm format compatibility, add to dev dependencies
2. **Phase 1** — Add jiff dep alongside chrono, CI green
3. **Phase 2** — Migrate `dates.rs`, all existing tests pass
4. **Phase 3** — Migrate `model.rs`, update serde impls
5. **Phase 4** — Migrate `store.rs` with dual-parse fallback (Option B)
6. **Phase 5** — Migrate `query.rs`
7. **Phase 6** — Remove chrono, final cleanup
8. **Datebook M1** — `CalendarMonth` data structure + basic grid render in TUI (no interaction)
9. **Datebook M2** — Navigation, day selection, item detail panel
10. **Datebook M3** — `RecurrenceRule` model + NLP parser extension + `next_occurrence` engine
11. **Datebook M4** — `DateRangeFilter` in `Query`; view sections using relative date ranges
12. **Datebook M5** — Week grid view; time-of-day layout for timed items

Each milestone above is independently releasable.

---

## Open Questions

1. **Storage format (Option A vs B):** Do we want a clean schema migration now, or start with
   dual-parse and clean up later? Given jiff is still pre-1.0, Option B reduces risk — we can
   always run the migration once jiff stabilises.

2. **`RecurrenceRule` on `Item` vs separate table:** Storing recurrence as a JSON column on
   `items` is simple but means recurrence is per-item. If we ever need "series" semantics
   (editing one occurrence vs all future occurrences), a separate `recurrence_series` table would
   be cleaner. Worth deciding before writing migrations.

3. **Datebook scope for M1:** Month view only first, or month + week together? Month is simpler
   to implement; week grid is more useful for daily scheduling.

4. **Timezone handling:** The app is currently civil-time only. If we want iCal/CalDAV sync in
   the future, we need `jiff::Zoned`. That's a larger change and should be a separate plan. For
   now, all datebook items remain civil time.
