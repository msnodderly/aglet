---
title: Jiff Migration
status: shipped
created: 2026-03-01
shipped: 2026-03-21
---

# Jiff Migration

**Status:** Complete
**Branch:** `claude/evaluate-jiff-library-M0vi2`
**Scope:** Replace `chrono` with `jiff` across `agenda-core`, `agenda-cli`, and `agenda-tui`

---

## Background

Aglet currently uses `chrono` v0.4 throughout. All date/time values are stored and manipulated as
`NaiveDate`/`NaiveDateTime` — civil (timezone-unaware) types. This has worked fine, but the
planned datebook view and Lotus Agenda-style date features require calendar-unit arithmetic
(months, weeks, days as a single unit) that chrono does not support natively.

**Why jiff:**

1. **`Span` type** handles calendar arithmetic correctly and ergonomically. Chrono's `Duration` is
   purely time-based (seconds/nanoseconds); "add 1 month" requires manual logic accounting for
   month lengths and leap years. Jiff handles this out of the box.

2. **Future features depend on it:** Relative date filters ("3 months from now"), recurrence rules
   ("every 4 months"), and date-range queries all need calendar-unit spans. Building these on
   chrono would mean reimplementing what jiff already provides.

3. **TC39 Temporal design** — jiff separates civil time, timestamps, and zoned time as distinct
   types with clear semantics, following the TC39 Temporal proposal.

**Accepted risk:** Jiff is pre-1.0 (targeting Spring/Summer 2026). The API is largely stable but
not guaranteed. We accept this because:
- The civil time types (`jiff::civil::Date`, `jiff::civil::DateTime`) map 1-to-1 to our current
  chrono types
- The type boundary is narrow — date values flow through a small number of well-defined seams
- Our extensive test suite will catch regressions

---

## Decisions

1. **Storage format: Option A (schema migration).** Bump schema version, run a one-time
   `REPLACE` to convert `"YYYY-MM-DD HH:MM:SS"` → `"YYYY-MM-DDTHH:MM:SS"` (ISO 8601). No
   dual-parse fallback — the `T` separator is standard ISO 8601, not a jiff quirk.

2. **Recurrence: deferred.** `RecurrenceRule`, NLP recurrence parsing, and series semantics are
   out of scope for this plan. They will be designed in a separate spec once the migration lands.

3. **Datebook view: deferred.** Calendar grid views (month/week), `DateRangeFilter`, and related
   TUI work are out of scope. The migration stands alone as a prerequisite.

4. **Timezone: civil time only.** No `jiff::Zoned` in this migration. All date/time values remain
   civil (timezone-unaware). Timezone awareness will be addressed in a future plan if/when
   iCal/CalDAV sync requires it.

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
- `jiff::civil::DateTime` display format and parse behaviour
- `jiff::Timestamp` round-trips via RFC 3339 (identical to chrono's `DateTime<Utc>`)
- Schema migration `REPLACE` correctly converts existing rows

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

#### Schema migration (Decision 1: Option A)

Bump to the next schema version. Run a one-time data migration to rewrite existing datetime
strings from space-separated to ISO 8601 `T`-separated format:

```sql
-- Migration: version N → N+1
-- Rewrite when_date / done_date from "YYYY-MM-DD HH:MM:SS" to "YYYY-MM-DDTHH:MM:SS"
UPDATE items
   SET when_date = REPLACE(when_date, ' ', 'T')
 WHERE when_date IS NOT NULL;

UPDATE items
   SET done_date = REPLACE(done_date, ' ', 'T')
 WHERE done_date IS NOT NULL;
```

After migration, all datetime columns use ISO 8601. Jiff's `civil::DateTime` parses and emits
this format natively — no custom parse logic needed.

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

**`start_of_iso_week`** becomes simpler with jiff:

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

**`resolve_when_bucket`** — update the signature and arithmetic:

```rust
// Before
pub fn resolve_when_bucket(
    when_date: Option<NaiveDateTime>,
    reference_date: NaiveDate,
) -> WhenBucket {
    let when_day = when_datetime.date();
    let this_week_end = this_week_start
        .checked_add_signed(Duration::days(6))
        .expect("valid week range");

// After
pub fn resolve_when_bucket(
    when_date: Option<jiff::civil::DateTime>,
    reference_date: jiff::civil::Date,
) -> WhenBucket {
    let when_day = when_datetime.date();
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

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| Jiff API breaks before 1.0 | Low–Medium | Medium | Pin to a specific version; review changelog on upgrade |
| SQLite format mismatch causes silent data corruption | Low (with tests) | High | Schema migration (Option A) ensures one format; add round-trip tests |
| `civil::DateTime` serde format differs from chrono's | Low (verified in spike) | High | Confirm in Phase 0 spike before any production change |
| Weekday offset arithmetic regression | Low (covered by test suite) | Medium | Existing 40+ tests in dates.rs pin all edge cases |

---

## Implementation Checklist

### Phase 0: Spike
- [x] Add `jiff = { version = "0.2", features = ["serde"] }` to `agenda-core/Cargo.toml` as dev-dependency
- [x] Write spike test: `jiff::civil::DateTime` display produces `YYYY-MM-DDTHH:MM:SS`
- [x] Write spike test: `jiff::civil::DateTime` parses ISO 8601 with `T` separator
- [x] Write spike test: `jiff::Timestamp` round-trips via RFC 3339 (matches chrono `DateTime<Utc>`)
- [x] Write spike test: `jiff::Span` day/week arithmetic produces correct results
- [x] Write spike test: serde round-trip for `civil::DateTime` and `Timestamp`

### Phase 1: Add jiff dependency
- [x] Add `jiff = { version = "0.2", features = ["serde"] }` to `agenda-core/Cargo.toml` (production dep)
- [x] Add `jiff = "0.2"` to `agenda-cli/Cargo.toml`
- [x] Add `jiff = "0.2"` to `agenda-tui/Cargo.toml`
- [x] Verify clean compile with both chrono and jiff present

### Phase 2: Migrate `dates.rs`
- [x] Replace `use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, Weekday}` with jiff imports
- [x] Update `ParsedDate.datetime` type: `NaiveDateTime` → `jiff::civil::DateTime`
- [x] Update `DateParser` trait: `reference_date: NaiveDate` → `jiff::civil::Date`
- [x] Update `BasicDateParser::parse` signature and implementation
- [x] Replace `Weekday::Mon/Tue/...` → `Weekday::Monday/Tuesday/...` in WEEKDAYS const
- [x] Replace `Duration::days()` → `Span::new().days()` in `scan_relative_dates`
- [x] Replace `num_days_from_monday()` → `to_monday_zero_offset()` in weekday delta functions
- [x] Update `at_midnight`: `date.and_hms_opt(0,0,0)` → `date.at(0,0,0,0)`
- [x] Update `attach_trailing_time`: `and_hms_opt` → `date.at(h, m, 0, 0)`
- [x] Update `resolve_month_day_without_year`: `NaiveDate::from_ymd_opt` → `jiff::civil::Date::new`
- [x] Update `scan_absolute_dates` / `scan_month_day_dates` / `scan_year_month_day`: all `from_ymd_opt` calls
- [x] Update test module imports and helpers (`date()`, `datetime()` helpers)
- [x] Verify all 40+ existing tests pass

### Phase 3: Migrate `model.rs`
- [x] Replace `use chrono::{DateTime, NaiveDateTime, Utc}` with jiff imports
- [x] Update `Item` struct: `DateTime<Utc>` → `Timestamp`, `NaiveDateTime` → `civil::DateTime`
- [x] Update `Assignment.assigned_at`: `DateTime<Utc>` → `Timestamp`
- [x] Update `ItemLink.created_at`: `DateTime<Utc>` → `Timestamp`
- [x] Update `Category.created_at/modified_at`: `DateTime<Utc>` → `Timestamp`
- [x] Update `DeletionLogEntry` date fields
- [x] Update `Item::new()`: `Utc::now()` → `Timestamp::now()`
- [x] Update `Category::new()`: `Utc::now()` → `Timestamp::now()`
- [x] Fix all downstream compile errors in files that use model types

### Phase 4: Migrate `store.rs`
- [x] Bump `SCHEMA_VERSION` from 10 to 11
- [x] Add migration SQL: `REPLACE(when_date, ' ', 'T')` and `REPLACE(done_date, ' ', 'T')`
- [x] Replace `DateTime::parse_from_rfc3339` → `Timestamp` `.parse()` for created_at/modified_at/assigned_at
- [x] Replace `.with_timezone(&Utc)` calls (no longer needed with Timestamp)
- [x] Replace `.to_rfc3339()` → `.to_string()` for Timestamp serialization
- [x] Replace `NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")` → `civil::DateTime` `.parse()` (ISO 8601)
- [x] Replace `NaiveDateTime.to_string()` → `civil::DateTime.to_string()` for when_date/done_date writes
- [x] Update `Utc::now()` → `Timestamp::now()` throughout store.rs
- [x] Write test: schema migration v10→v11 correctly rewrites existing rows
- [x] Verify all existing store tests pass

### Phase 5: Migrate `query.rs`
- [x] Replace `use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime}` with jiff imports
- [x] Update `resolve_when_bucket` signature: `NaiveDateTime`/`NaiveDate` → jiff types
- [x] Replace `Duration::days()` → `Span::new().days()` in week calculations
- [x] Replace `succ_opt()` → `checked_add(Span::new().days(1))`
- [x] Update `start_of_iso_week`: `checked_sub_signed` → `checked_sub`, `num_days_from_monday` → `to_monday_zero_offset`
- [x] Update test module helpers and imports
- [x] Verify all existing query tests pass

### Phase 6: Migrate remaining files and remove chrono
- [x] Migrate `agenda.rs`: `Utc::now()` → `Timestamp::now()`, update `NaiveDate`/`NaiveDateTime` refs, remove `Timelike` import
- [x] Migrate `engine.rs`: `Utc::now()` → `Timestamp::now()`
- [x] Migrate `classification.rs`: update date type references (if file exists)
- [x] Migrate `agenda-cli/src/main.rs`: `Local::now().date_naive()` → `jiff::Zoned::now().date()`, update parse helpers
- [x] Migrate `agenda-tui/src/lib.rs`: same Local→Zoned pattern, update `Utc::now()` calls
- [x] Migrate `agenda-tui/src/app.rs`: `Local::now().date_naive()` → `jiff::Zoned::now().date()`
- [x] Migrate `agenda-tui/src/modes/board.rs`: update `parse_when_datetime_input`, `.format()` → `.strftime()`, Local/Utc calls
- [x] Migrate `agenda-tui/src/modes/view_edit.rs`: Local→Zoned
- [x] Migrate `agenda-tui/src/render/mod.rs`: Local→Zoned
- [x] Migrate `agenda-tui/src/ui_support.rs`: update test fixtures
- [x] Remove `chrono` from `agenda-core/Cargo.toml`
- [x] Remove `chrono` from `agenda-cli/Cargo.toml`
- [x] Remove `chrono` from `agenda-tui/Cargo.toml`
- [x] Verify no remaining `use chrono` imports
- [x] Full test suite green: `cargo test --workspace`
