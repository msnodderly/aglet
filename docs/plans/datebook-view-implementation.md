---
title: Datebook View Implementation
status: shipped
created: 2026-03-21
shipped: 2026-04-08
---

# Datebook View Implementation Plan

## Context

Lotus Agenda's datebook view auto-generates sections from time intervals (Day/Week/Month/Quarter) rather than relying on manually configured category-based sections. Aglet's item-level date infrastructure (when_date, NLP parsing, recurrence, WhenBucket filtering, jiff arithmetic) is already complete, but there is no datebook view type. This plan adds it in three tiers: model/engine first, then TUI surface, then polish.

---

## Implementation Checklist

### Tier 1 -- Model & Engine
- [x] 1a. DatebookConfig types in model.rs (DatebookPeriod, DatebookInterval, DatebookAnchor, DatebookConfig, cycling/label/validation, View field)
- [x] 1b. Section generation engine in query.rs (generate_datebook_sections, window computation, title formatting, extract_item_date)
- [x] 1c. Integrate datebook into resolve_view (datebook branch + resolve_datebook_view)
- [x] 1d. Store schema migration v17→v18 (datebook_config_json column, CRUD updates)
- [x] 1e. CLI create-datebook and datebook-browse commands
- [x] 1f. Unit tests for datebook engine (12 tests, all passing)

### Tier 2 -- TUI Surface
- [x] 2a. Datebook view creation in ViewPicker ('d' keybinding)
- [x] 2b. ViewEdit datebook config region (ViewEditRegion::Datebook, field cycling)
- [x] 2c. Render datebook config in ViewEdit details pane
- [x] 2d. Browse keybindings in Normal mode (}/{ for fwd/back, 0 for today)
- [x] 2e. Today-section highlighting (yellow border in vertical + horizontal layouts)
- [x] 2f. Footer hints for datebook views (Normal mode browse hints, ViewPicker 'd' hint)

### Tier 3 -- Polish & Extensions (defer)
- [ ] 3a. End date on items (event duration)
- [ ] 3b. Calendar popup widget
- [ ] 3c. Configurable date display format
- [ ] 3d. Arbitrary date-range filters for standard views

---

## Tier 1 -- Model & Engine

### 1a. New types in `model.rs`

**File:** `crates/aglet-core/src/model.rs` (after `SectionFlow` ~line 766)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DatebookPeriod {
    Day,
    Week,
    Month,
    Quarter,
    Year,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DatebookInterval {
    Hourly,
    Daily,
    Weekly,
    Monthly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DatebookAnchor {
    Today,
    StartOfWeek,
    StartOfMonth,
    StartOfQuarter,
    StartOfYear,
    Absolute(jiff::civil::Date),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DatebookConfig {
    pub period: DatebookPeriod,
    pub interval: DatebookInterval,
    pub anchor: DatebookAnchor,
    pub date_source: DateSource,        // reuse existing enum (When/Entry/Done)
    #[serde(default)]
    pub browse_offset: i32,             // signed; +1 = forward one period
}
```

**Design notes:**
- Omitting `FifteenMin` and `ThirtyMin` intervals from initial implementation. Sub-hourly intervals generate excessive sections and Aglet items don't carry sub-hour precision in practice. Can add later.
- `browse_offset` is persisted on the view so it survives app restarts.
- Reuses existing `DateSource` enum rather than creating a new one.

Add to `View` struct (line 734):

```rust
#[serde(default)]
pub datebook_config: Option<DatebookConfig>,
```

Update `View::new()` (line 1095) to include `datebook_config: None`.

Add cycling/label methods following the `SummaryFn::next()`/`label()` pattern:

```rust
impl DatebookPeriod {
    pub fn next(self) -> Self { /* Day -> Week -> Month -> Quarter -> Year -> Day */ }
    pub fn label(self) -> &'static str { /* "Day", "Week", etc. */ }
}
impl DatebookInterval {
    pub fn next(self) -> Self { /* Hourly -> Daily -> Weekly -> Monthly -> Hourly */ }
    pub fn label(self) -> &'static str { /* "Hourly", "Daily", etc. */ }
}
impl DatebookAnchor {
    pub fn next(self) -> Self { /* Today -> StartOfWeek -> ... -> Today */ }
    pub fn label(&self) -> &'static str { /* "Today", "Start of week", etc. */ }
}
```

Add validation:

```rust
impl DatebookConfig {
    pub fn is_valid(&self) -> bool {
        // Interval must be finer than period
        match self.period {
            DatebookPeriod::Day => matches!(self.interval, DatebookInterval::Hourly),
            DatebookPeriod::Week => matches!(self.interval,
                DatebookInterval::Hourly | DatebookInterval::Daily),
            DatebookPeriod::Month => matches!(self.interval,
                DatebookInterval::Daily | DatebookInterval::Weekly),
            DatebookPeriod::Quarter => matches!(self.interval,
                DatebookInterval::Weekly | DatebookInterval::Monthly),
            DatebookPeriod::Year => matches!(self.interval,
                DatebookInterval::Weekly | DatebookInterval::Monthly),
        }
    }
}

impl Default for DatebookConfig {
    fn default() -> Self {
        Self {
            period: DatebookPeriod::Week,
            interval: DatebookInterval::Daily,
            anchor: DatebookAnchor::StartOfWeek,
            date_source: DateSource::When,
            browse_offset: 0,
        }
    }
}
```

### 1b. Section generation engine

**File:** `crates/aglet-core/src/query.rs`

New public types and functions (add before the `#[cfg(test)]` block):

```rust
/// A dynamically generated datebook section with time boundaries.
#[derive(Debug, Clone)]
pub struct DatebookSection {
    pub title: String,
    pub range_start: DateTime,  // inclusive
    pub range_end: DateTime,    // exclusive
}

/// Generate the section boundaries for a datebook view.
pub fn generate_datebook_sections(
    config: &DatebookConfig,
    reference_date: Date,
) -> Vec<DatebookSection> {
    let (window_start, window_end) = compute_datebook_window(config, reference_date);
    let mut sections = Vec::new();
    let mut cursor = window_start;
    while cursor < window_end {
        let next = advance_by_interval(cursor, config.interval);
        let clamped = if next > window_end { window_end } else { next };
        sections.push(DatebookSection {
            title: format_datebook_section_title(cursor, clamped, config),
            range_start: cursor,
            range_end: clamped,
        });
        cursor = next;
    }
    sections
}
```

**Window computation:**

```rust
fn compute_datebook_window(
    config: &DatebookConfig,
    reference_date: Date,
) -> (DateTime, DateTime) {
    let base = resolve_datebook_anchor(&config.anchor, reference_date);
    let shifted = apply_browse_offset(base, config.period, config.browse_offset);
    let end = advance_by_period(shifted, config.period);
    (shifted, end)
}

fn resolve_datebook_anchor(anchor: &DatebookAnchor, ref_date: Date) -> DateTime {
    match anchor {
        DatebookAnchor::Today => ref_date.at(0, 0, 0, 0),
        DatebookAnchor::StartOfWeek => start_of_iso_week(ref_date).at(0, 0, 0, 0),
        DatebookAnchor::StartOfMonth => {
            Date::new(ref_date.year(), ref_date.month() as i8, 1)
                .expect("first of month is valid").at(0, 0, 0, 0)
        }
        DatebookAnchor::StartOfQuarter => {
            let q_month = ((ref_date.month() as i8 - 1) / 3) * 3 + 1;
            Date::new(ref_date.year(), q_month, 1)
                .expect("quarter start is valid").at(0, 0, 0, 0)
        }
        DatebookAnchor::StartOfYear => {
            Date::new(ref_date.year(), 1, 1)
                .expect("jan 1 is valid").at(0, 0, 0, 0)
        }
        DatebookAnchor::Absolute(d) => d.at(0, 0, 0, 0),
    }
}

// Reuses existing `start_of_iso_week` at line 372.

fn advance_by_period(dt: DateTime, period: DatebookPeriod) -> DateTime {
    let span = match period {
        DatebookPeriod::Day => Span::new().days(1),
        DatebookPeriod::Week => Span::new().weeks(1),
        DatebookPeriod::Month => Span::new().months(1),
        DatebookPeriod::Quarter => Span::new().months(3),
        DatebookPeriod::Year => Span::new().years(1),
    };
    dt.checked_add(span).expect("period advance overflow")
}

fn advance_by_interval(dt: DateTime, interval: DatebookInterval) -> DateTime {
    let span = match interval {
        DatebookInterval::Hourly => Span::new().hours(1),
        DatebookInterval::Daily => Span::new().days(1),
        DatebookInterval::Weekly => Span::new().weeks(1),
        DatebookInterval::Monthly => Span::new().months(1),
    };
    dt.checked_add(span).expect("interval advance overflow")
}

fn apply_browse_offset(base: DateTime, period: DatebookPeriod, offset: i32) -> DateTime {
    if offset == 0 { return base; }
    let span = match period {
        DatebookPeriod::Day => Span::new().days(i64::from(offset)),
        DatebookPeriod::Week => Span::new().weeks(i64::from(offset)),
        DatebookPeriod::Month => Span::new().months(i32::from(offset)),
        DatebookPeriod::Quarter => Span::new().months(i32::from(offset) * 3),
        DatebookPeriod::Year => Span::new().years(i32::from(offset)),
    };
    base.checked_add(span).expect("browse offset overflow")
}
```

**Section title formatting:**

```rust
fn format_datebook_section_title(
    start: DateTime,
    _end: DateTime,
    config: &DatebookConfig,
) -> String {
    match config.interval {
        DatebookInterval::Hourly => {
            // "Mon Apr 7, 9:00 AM"
            format!("{} {:02}:{:02}",
                format_date_short(start.date()),
                start.hour(), start.minute())
        }
        DatebookInterval::Daily => {
            // "Mon, Apr 7"
            format_date_with_weekday(start.date())
        }
        DatebookInterval::Weekly => {
            // "Apr 7 - Apr 13"
            let end_date = _end.checked_sub(Span::new().days(1))
                .unwrap_or(_end).date();
            format!("{} - {}", format_date_short(start.date()), format_date_short(end_date))
        }
        DatebookInterval::Monthly => {
            // "April 2026"
            format!("{} {}", month_name(start.date().month() as u8), start.date().year())
        }
    }
}
```

Reuse `month_name()` and `weekday_name()` already in `model.rs`.

### 1c. Integration with `resolve_view`

**File:** `crates/aglet-core/src/query.rs` -- modify `resolve_view` (line 103)

```rust
pub fn resolve_view(
    view: &View,
    items: &[Item],
    categories: &[Category],
    reference_date: Date,
) -> ViewResult {
    // NEW: datebook branch
    if let Some(config) = &view.datebook_config {
        return resolve_datebook_view(view, config, items, reference_date);
    }
    // ... existing standard-view logic unchanged ...
}
```

New function:

```rust
fn resolve_datebook_view(
    view: &View,
    config: &DatebookConfig,
    items: &[Item],
    reference_date: Date,
) -> ViewResult {
    // 1. Apply view-level criteria filter
    let view_items: Vec<Item> = evaluate_query(&view.criteria, items, reference_date)
        .into_iter()
        .cloned()
        .collect();

    // 2. Generate time-interval sections
    let db_sections = generate_datebook_sections(config, reference_date);

    // 3. Bucket items into sections by date
    let mut matched_ids = HashSet::new();
    let sections: Vec<ViewSectionResult> = db_sections
        .iter()
        .enumerate()
        .map(|(idx, ds)| {
            let section_items: Vec<Item> = view_items
                .iter()
                .filter(|item| {
                    let dt = extract_item_date(item, config.date_source);
                    matches!(dt, Some(d) if d >= ds.range_start && d < ds.range_end)
                })
                .cloned()
                .collect();
            matched_ids.extend(section_items.iter().map(|i| i.id));
            ViewSectionResult {
                section_index: idx,
                title: ds.title.clone(),
                items: section_items,
                subsections: Vec::new(),
            }
        })
        .collect();

    // 4. Unmatched: items with no date or date outside window
    let (unmatched, unmatched_label) = if view.show_unmatched {
        let unmatched_items = view_items
            .into_iter()
            .filter(|item| !matched_ids.contains(&item.id))
            .collect();
        (Some(unmatched_items), Some(view.unmatched_label.clone()))
    } else {
        (None, None)
    };

    ViewResult { sections, unmatched, unmatched_label }
}

fn extract_item_date(item: &Item, source: DateSource) -> Option<DateTime> {
    match source {
        DateSource::When => item.when_date,
        DateSource::Done => item.done_date,
        DateSource::Entry => {
            // Derive from created_at (UTC Timestamp -> civil DateTime)
            let zdt = item.created_at.to_zoned(jiff::tz::TimeZone::UTC);
            Some(zdt.datetime())
        }
    }
}
```

### 1d. Store schema migration (v17 -> v18)

**File:** `crates/aglet-core/src/store.rs`

1. Bump `SCHEMA_VERSION` from 17 to 18 (line 24)

2. Add migration guard in `apply_migrations`:
   ```rust
   if !self.column_exists("views", "datebook_config_json")? {
       self.conn.execute_batch(
           "ALTER TABLE views ADD COLUMN datebook_config_json TEXT;",
       )?;
   }
   ```

3. Update `insert_view` -- add `datebook_config_json` to INSERT:
   ```rust
   let datebook_config_json: Option<String> = view.datebook_config
       .as_ref()
       .map(|c| serde_json::to_string(c).expect("DatebookConfig serializable"));
   // Add as parameter to INSERT ... VALUES (... ?14)
   ```

4. Update `update_view` -- add `datebook_config_json = ?N` to UPDATE SET clause

5. Update `row_to_view` -- read new column:
   ```rust
   let datebook_config_json: Option<String> = row.get("datebook_config_json")?;
   let datebook_config = datebook_config_json
       .and_then(|json| serde_json::from_str(&json).ok());
   ```

6. Update all SELECT queries that read views to include `datebook_config_json`

**Backwards compat:** Nullable TEXT column + `#[serde(default)]` on the View field = existing views load with `datebook_config: None`.

### 1e. CLI commands

**File:** `crates/aglet-cli/src/main.rs`

Add under `ViewCommand`:

```rust
/// Create a datebook (date-interval) view
#[command(name = "create-datebook")]
CreateDatebook {
    name: String,
    #[arg(long, value_enum, default_value_t = CliDatebookPeriod::Week)]
    period: CliDatebookPeriod,
    #[arg(long, value_enum, default_value_t = CliDatebookInterval::Daily)]
    interval: CliDatebookInterval,
    #[arg(long, value_enum, default_value_t = CliDatebookAnchor::StartOfWeek)]
    anchor: CliDatebookAnchor,
    #[arg(long, value_enum, default_value_t = CliDateSource::When)]
    date_source: CliDateSource,
},

/// Shift a datebook view's browse window
#[command(name = "datebook-browse")]
DatebookBrowse {
    name: String,
    /// +N forward, -N backward, 0 reset
    #[arg(long, default_value_t = 1)]
    offset: i32,
},
```

Add `ValueEnum` wrapper types with `into_model()` conversions following the existing `CliDateSource` pattern.

### 1f. Tests

**File:** `crates/aglet-core/src/query.rs` (test module)

- `test_generate_datebook_sections_week_daily` -- 7 sections, correct titles
- `test_generate_datebook_sections_month_weekly` -- 4-5 sections, correct date ranges
- `test_generate_datebook_sections_quarter_monthly` -- 3 sections
- `test_generate_datebook_sections_day_hourly` -- 24 sections
- `test_datebook_browse_offset` -- shifting forward/backward produces correct windows
- `test_resolve_datebook_view_buckets_items` -- items land in correct sections
- `test_resolve_datebook_view_no_date_unmatched` -- items without dates go to unmatched
- `test_resolve_datebook_view_outside_window_unmatched` -- items outside window go to unmatched
- `test_resolve_datebook_view_boundary` -- item exactly on section boundary goes to later section
- `test_datebook_config_validation` -- invalid combos rejected
- `test_datebook_month_end_clamping` -- quarter starting Jan in a leap year, etc.

**File:** `crates/aglet-core/src/store.rs` (test module)

- `test_datebook_view_round_trip` -- create, save, reload, config identical
- `test_migration_v17_null_datebook_config` -- old view loads with None

---

## Tier 2 -- TUI Surface

### 2a. Datebook view creation in ViewPicker

**File:** `crates/aglet-tui/src/modes/view_edit/picker.rs`

Add `'d'` keybinding alongside existing `'n'`:

```rust
KeyCode::Char('d') | KeyCode::Char('D') => {
    let mut view = View::new("Untitled Datebook".to_string());
    view.datebook_config = Some(DatebookConfig::default());
    // No manual sections needed -- sections are generated dynamically
    self.open_view_edit_new_view_focus_name(view);
}
```

Update footer hints to show `d: new datebook`.

### 2b. ViewEdit datebook config region

**File:** `crates/aglet-tui/src/modes/view_edit/state.rs`

Add `Datebook` variant to `ViewEditRegion`:

```rust
pub(crate) enum ViewEditRegion {
    Criteria,
    Sections,
    Unmatched,
    Datebook,    // NEW: datebook-specific config fields
}
```

Add field to `ViewEditState`:

```rust
pub(crate) datebook_field_index: usize,  // 0=Period, 1=Interval, 2=Anchor, 3=DateSource
```

**File:** `crates/aglet-tui/src/modes/view_edit/editor.rs`

When a datebook view is being edited, Tab cycles through: `Criteria -> Datebook -> Unmatched` (skipping `Sections` since sections are auto-generated).

Add dispatch in the key handler (line ~351):

```rust
ViewEditRegion::Datebook => self.handle_view_edit_datebook_key(code),
```

New handler (new file or in `details.rs`):

```rust
pub(crate) fn handle_view_edit_datebook_key(
    &mut self, code: KeyCode,
) -> TuiResult<bool> {
    let state = self.view_edit_state.as_mut().unwrap();
    let config = state.draft.datebook_config.as_mut().unwrap();
    match code {
        KeyCode::Char('j') | KeyCode::Down => {
            state.datebook_field_index = (state.datebook_field_index + 1).min(3);
        }
        KeyCode::Char('k') | KeyCode::Up => {
            state.datebook_field_index = state.datebook_field_index.saturating_sub(1);
        }
        KeyCode::Char(' ') | KeyCode::Enter => {
            match state.datebook_field_index {
                0 => { config.period = config.period.next(); }
                1 => { config.interval = config.interval.next(); }
                2 => { config.anchor = config.anchor.next(); }
                3 => { config.date_source = config.date_source.next(); }
                _ => {}
            }
            // Auto-fix invalid combos: if interval is now too coarse for period, advance it
            while !config.is_valid() {
                config.interval = config.interval.next();
            }
            state.dirty = true;
        }
        _ => {}
    }
    Ok(true)
}
```

**Need to add** `DateSource::next()` method to `model.rs`:

```rust
impl DateSource {
    pub fn next(self) -> Self {
        match self { Self::When => Self::Entry, Self::Entry => Self::Done, Self::Done => Self::When }
    }
    pub fn label(self) -> &'static str {
        match self { Self::When => "When", Self::Entry => "Entry", Self::Done => "Done" }
    }
}
```

### 2c. Rendering datebook config in ViewEdit details pane

**File:** `crates/aglet-tui/src/render/mod.rs` (in the ViewEdit render section ~line 7888)

When `state.draft.datebook_config.is_some()`, render datebook fields in the details pane:

```
 Datebook Config
 ─────────────────────
 > Period:      Week        (Space to cycle)
   Interval:    Daily
   Anchor:      Start of week
   Date source: When
```

Highlighted row uses the focused style. This replaces the Sections pane in the layout -- datebook views have no manually-editable sections list.

Below this, render the standard Criteria rows (category include/exclude, WhenBucket include/exclude) so users can combine datebook sectioning with category filtering.

### 2d. Browse keybindings in Normal mode

**File:** `crates/aglet-tui/src/modes/board.rs` (or wherever Normal-mode keys are handled)

When the active view has `datebook_config.is_some()`:

| Key | Action |
|-----|--------|
| `]` or `>` | `browse_offset += 1`, persist, refresh |
| `[` or `<` | `browse_offset -= 1`, persist, refresh |
| `0` | `browse_offset = 0`, persist, refresh |

Status line shows: `"This week"` / `"Next week"` / `"2 weeks ago"` etc., derived from `browse_offset` and `period`.

Persist immediately via `store.update_view()` so the offset survives restart.

### 2e. Today-section highlighting

**File:** `crates/aglet-tui/src/render/mod.rs`

When rendering a datebook view's board, if a section's `range_start <= today_midnight < range_end`, apply a subtle highlight (e.g., `Color::Yellow` border or bold title). This provides the "current time" anchor that Lotus Agenda's datebook had visually.

### 2f. Footer hints

Update footer hints for datebook views:
- Normal mode: `] fwd  [ back  0 today` (in addition to standard hints)
- ViewEdit mode: `Space cycle  Tab region  S save  Esc cancel`

---

## Tier 3 -- Polish & Extensions (defer)

### 3a. End date on items (event duration)

Add `end_date: Option<jiff::civil::DateTime>` to `Item`. Schema migration adds nullable column. Datebook bucketing changes from point-in-time to range overlap:

```rust
// Item spans multiple sections if it has an end_date
let item_start = extract_item_date(item, source)?;
let item_end = item.end_date.unwrap_or(item_start);
item_start < ds.range_end && item_end >= ds.range_start
```

### 3b. Calendar popup widget

A month-grid overlay triggered by a keybinding when editing `when_date` or setting an absolute datebook anchor. Renders a 7-column grid with cursor navigation. Standalone widget, no model changes.

### 3c. Configurable date display format

Add `DateDisplayFormat` enum (`Short`, `Long`, `ISO`, `Relative`) to `Column` (when `kind == When`). Affects rendering only.

### 3d. Arbitrary date-range filters for standard views

Add `DateRangeFilter { source: DateSource, from: DateValueExpr, through: DateValueExpr }` to `Query`. This would let standard views filter by "items due between March 1-15" without needing a full datebook view. The building blocks (`DateValueExpr`, `DateMatcher::Range`) already exist in `model.rs`.

---

## Critical Files

| File | Changes |
|------|---------|
| `crates/aglet-core/src/model.rs` | DatebookConfig, enums, View field, cycling/label methods |
| `crates/aglet-core/src/query.rs` | Section generation, resolve_datebook_view, extract_item_date |
| `crates/aglet-core/src/store.rs` | Schema v18, migration, view CRUD updates |
| `crates/aglet-cli/src/main.rs` | create-datebook, datebook-browse commands |
| `crates/aglet-tui/src/modes/view_edit/state.rs` | ViewEditRegion::Datebook, datebook_field_index |
| `crates/aglet-tui/src/modes/view_edit/picker.rs` | 'd' keybinding for new datebook |
| `crates/aglet-tui/src/modes/view_edit/editor.rs` | Datebook key handler, Tab cycle changes |
| `crates/aglet-tui/src/modes/view_edit/details.rs` | Datebook config detail rendering |
| `crates/aglet-tui/src/modes/board.rs` | Browse keybindings in Normal mode |
| `crates/aglet-tui/src/render/mod.rs` | Datebook config pane, today highlighting |

## Reusable Existing Code

| What | Where | How |
|------|-------|-----|
| `start_of_iso_week()` | `query.rs:372` | Anchor resolution for StartOfWeek |
| `month_name()`, `weekday_name()` | `model.rs` | Section title formatting |
| `DateSource` enum | `model.rs:673` | Reused directly in DatebookConfig |
| `DateValueExpr` | `model.rs:690` | Future: arbitrary date-range filters (Tier 3d) |
| `evaluate_query()` | `query.rs:60` | View-level criteria still apply to datebook views |
| `SummaryFn` + `compute_column_aggregates()` | `model.rs:791`, `ui_support.rs` | Works per-section; composes with datebook sections automatically |
| `column_exists()` migration guard | `store.rs` | Schema migration pattern |

## Verification

1. **Unit tests:** `cargo test --workspace` -- all existing tests pass, new tests cover section generation, bucketing, round-trip, validation
2. **CLI smoke test:**
   ```sh
   cargo run --bin aglet -- --db test.ag view create-datebook "This Week" --period week --interval daily
   cargo run --bin aglet -- --db test.ag view show "This Week"
   cargo run --bin aglet -- --db test.ag view datebook-browse "This Week" --offset 1
   cargo run --bin aglet -- --db test.ag view show "This Week"  # should show next week
   ```
3. **TUI smoke test:**
   - Launch TUI, press `v` then `d`, name the view, configure period/interval via Space, save with `S`
   - Verify sections render with date headers and items bucketed correctly
   - Press `]` to browse forward, `[` back, `0` to reset
   - Verify numeric aggregation still works in datebook section footers
4. **Migration test:** Open a v17 database with the new binary, verify existing views load unchanged
