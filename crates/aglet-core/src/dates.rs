use jiff::civil::{Date, DateTime, Weekday};
use jiff::Span;

use crate::model::{weekday_to_u8, RecurrenceFrequency, RecurrenceRule};

/// Parses date/time expressions from item text.
pub trait DateParser: Send + Sync {
    /// Extract a date/time from item text.
    ///
    /// Returns `None` when no supported date expression is found.
    /// Returns `Some(ParsedDate)` when an expression is found and resolved
    /// against `reference_date`.
    fn parse(&self, text: &str, reference_date: Date) -> Option<ParsedDate>;
}

/// Parsed date/time data and source provenance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParsedDate {
    /// Absolute local datetime resolved during parsing.
    pub datetime: DateTime,
    /// Matched source span as UTF-8 byte offsets in `text`, half-open: `[start, end)`.
    ///
    /// When valid, `&text[start..end]` yields the matched expression.
    pub span: (usize, usize),
}

/// Policy for interpreting ambiguous `this <weekday>` and `next <weekday>` phrases.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum WeekdayDisambiguationPolicy {
    /// `next <weekday>` means the weekday in the following calendar week.
    ///
    /// Example from Monday 2026-02-16:
    /// - `this Tuesday` => 2026-02-17
    /// - `next Tuesday` => 2026-02-24
    #[default]
    StrictNextWeek,
    /// `next <weekday>` means the next occurrence strictly after the reference date.
    ///
    /// Example from Monday 2026-02-16:
    /// - `this Tuesday` => 2026-02-17
    /// - `next Tuesday` => 2026-02-17
    InclusiveNext,
}

/// Deterministic parser for supported date expressions.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct BasicDateParser {
    weekday_policy: WeekdayDisambiguationPolicy,
}

impl BasicDateParser {
    pub const fn with_weekday_policy(policy: WeekdayDisambiguationPolicy) -> Self {
        Self {
            weekday_policy: policy,
        }
    }

    pub const fn weekday_policy(&self) -> WeekdayDisambiguationPolicy {
        self.weekday_policy
    }
}

impl DateParser for BasicDateParser {
    fn parse(&self, text: &str, reference_date: Date) -> Option<ParsedDate> {
        let bytes = text.as_bytes();
        let mut best = None;

        scan_relative_dates(bytes, reference_date, self.weekday_policy, &mut best);
        scan_relative_period_phrases(bytes, reference_date, &mut best);
        scan_in_n_phrases(bytes, reference_date, &mut best);
        scan_month_name_dates(bytes, reference_date, &mut best);
        scan_iso_dashed_dates(bytes, &mut best);
        scan_iso_compact_dates(bytes, &mut best);
        scan_numeric_mdy_dates(bytes, &mut best);

        best.map(|parsed| attach_trailing_time(bytes, parsed))
    }
}

const MONTHS: [(&str, u32); 12] = [
    ("january", 1),
    ("february", 2),
    ("march", 3),
    ("april", 4),
    ("may", 5),
    ("june", 6),
    ("july", 7),
    ("august", 8),
    ("september", 9),
    ("october", 10),
    ("november", 11),
    ("december", 12),
];

const MONTHS_ABBREV: [(&str, u32); 12] = [
    ("jan", 1),
    ("feb", 2),
    ("mar", 3),
    ("apr", 4),
    ("may", 5),
    ("jun", 6),
    ("jul", 7),
    ("aug", 8),
    ("sep", 9),
    ("oct", 10),
    ("nov", 11),
    ("dec", 12),
];

const WEEKDAYS: [(&str, Weekday); 7] = [
    ("monday", Weekday::Monday),
    ("tuesday", Weekday::Tuesday),
    ("wednesday", Weekday::Wednesday),
    ("thursday", Weekday::Thursday),
    ("friday", Weekday::Friday),
    ("saturday", Weekday::Saturday),
    ("sunday", Weekday::Sunday),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RelativeWeekdayPrefix {
    This,
    Next,
}

impl RelativeWeekdayPrefix {
    const fn keyword(self) -> &'static str {
        match self {
            RelativeWeekdayPrefix::This => "this",
            RelativeWeekdayPrefix::Next => "next",
        }
    }
}

fn scan_relative_dates(
    bytes: &[u8],
    reference_date: Date,
    weekday_policy: WeekdayDisambiguationPolicy,
    best: &mut Option<ParsedDate>,
) {
    for start in 0..bytes.len() {
        if !has_left_boundary(bytes, start) {
            continue;
        }

        for (keyword, day_offset) in [("today", 0_i64), ("tomorrow", 1_i64), ("yesterday", -1_i64)]
        {
            if !matches_ascii_insensitive(bytes, start, keyword.as_bytes()) {
                continue;
            }

            let end = start + keyword.len();
            if !has_right_boundary(bytes, end) {
                continue;
            }

            if let Ok(date) = reference_date.checked_add(Span::new().days(day_offset)) {
                choose_best(
                    best,
                    ParsedDate {
                        datetime: at_midnight(date),
                        span: (start, end),
                    },
                );
            }
        }

        if let Some(candidate) = parse_relative_weekday(
            bytes,
            start,
            reference_date,
            RelativeWeekdayPrefix::This,
            weekday_policy,
        ) {
            choose_best(best, candidate);
        }

        if let Some(candidate) = parse_relative_weekday(
            bytes,
            start,
            reference_date,
            RelativeWeekdayPrefix::Next,
            weekday_policy,
        ) {
            choose_best(best, candidate);
        }

        // Bare weekday without prefix (e.g. "tuesday") → same as "this tuesday".
        if let Some(candidate) = parse_bare_weekday(bytes, start, reference_date, weekday_policy) {
            choose_best(best, candidate);
        }
    }
}

fn parse_relative_weekday(
    bytes: &[u8],
    start: usize,
    reference_date: Date,
    prefix: RelativeWeekdayPrefix,
    policy: WeekdayDisambiguationPolicy,
) -> Option<ParsedDate> {
    if !matches_ascii_insensitive(bytes, start, prefix.keyword().as_bytes()) {
        return None;
    }

    let mut pos = start + prefix.keyword().len();
    if pos >= bytes.len() || !bytes[pos].is_ascii_whitespace() {
        return None;
    }

    pos = skip_whitespace(bytes, pos);
    for (weekday_name, weekday) in WEEKDAYS {
        if !matches_ascii_insensitive(bytes, pos, weekday_name.as_bytes()) {
            continue;
        }

        let end = pos + weekday_name.len();
        if !has_right_boundary(bytes, end) {
            continue;
        }

        let day_delta =
            days_until_relative_weekday(reference_date.weekday(), weekday, prefix, policy);
        let date = reference_date
            .checked_add(Span::new().days(day_delta))
            .ok()?;

        return Some(ParsedDate {
            datetime: at_midnight(date),
            span: (start, end),
        });
    }

    None
}

/// Bare weekday without prefix (e.g. "tuesday") → next occurrence, 1–7 days forward.
/// Unlike "this tuesday" which can return today (0-day delta), a bare weekday
/// always advances: typing "monday" on a Monday means next Monday.
fn parse_bare_weekday(
    bytes: &[u8],
    start: usize,
    reference_date: Date,
    _policy: WeekdayDisambiguationPolicy,
) -> Option<ParsedDate> {
    for (weekday_name, weekday) in WEEKDAYS {
        if !matches_ascii_insensitive(bytes, start, weekday_name.as_bytes()) {
            continue;
        }

        let end = start + weekday_name.len();
        if !has_right_boundary(bytes, end) {
            continue;
        }

        let mut day_delta = days_until_weekday_this(reference_date.weekday(), weekday);
        if day_delta == 0 {
            day_delta = 7; // same day → advance to next week
        }
        let date = reference_date
            .checked_add(Span::new().days(day_delta))
            .ok()?;

        return Some(ParsedDate {
            datetime: at_midnight(date),
            span: (start, end),
        });
    }

    None
}

fn days_until_relative_weekday(
    current: Weekday,
    target: Weekday,
    prefix: RelativeWeekdayPrefix,
    policy: WeekdayDisambiguationPolicy,
) -> i64 {
    match prefix {
        RelativeWeekdayPrefix::This => days_until_weekday_this(current, target),
        RelativeWeekdayPrefix::Next => days_until_weekday_next(current, target, policy),
    }
}

fn days_until_weekday_this(current: Weekday, target: Weekday) -> i64 {
    let current_idx = current.to_monday_zero_offset() as i64;
    let target_idx = target.to_monday_zero_offset() as i64;
    (target_idx - current_idx + 7) % 7
}

fn days_until_weekday_next(
    current: Weekday,
    target: Weekday,
    policy: WeekdayDisambiguationPolicy,
) -> i64 {
    let current_idx = current.to_monday_zero_offset() as i64;
    let target_idx = target.to_monday_zero_offset() as i64;

    match policy {
        WeekdayDisambiguationPolicy::InclusiveNext => {
            let mut delta = (target_idx - current_idx + 7) % 7;
            if delta == 0 {
                delta = 7;
            }
            delta
        }
        WeekdayDisambiguationPolicy::StrictNextWeek => 7 + target_idx - current_idx,
    }
}

const PERIOD_PHRASES: &[&str] = &[
    "next week",
    "last week",
    "next month",
    "last month",
    "end of week",
    "end of month",
    "next year",
];

fn resolve_period_phrase(phrase_index: usize, d: Date) -> Option<Date> {
    match phrase_index {
        // next week → Monday of next ISO week
        0 => {
            let days = 7 - d.weekday().to_monday_zero_offset() as i64;
            d.checked_add(Span::new().days(days)).ok()
        }
        // last week → Monday of previous ISO week
        1 => {
            let days = d.weekday().to_monday_zero_offset() as i64 + 7;
            d.checked_add(Span::new().days(-days)).ok()
        }
        // next month → 1st of next month
        2 => {
            if d.month() == 12 {
                Date::new(d.year() + 1, 1, 1).ok()
            } else {
                Date::new(d.year(), d.month() + 1, 1).ok()
            }
        }
        // last month → 1st of previous month
        3 => {
            if d.month() == 1 {
                Date::new(d.year() - 1, 12, 1).ok()
            } else {
                Date::new(d.year(), d.month() - 1, 1).ok()
            }
        }
        // end of week → Friday of current ISO week
        4 => {
            let offset = 4_i64 - d.weekday().to_monday_zero_offset() as i64;
            d.checked_add(Span::new().days(offset)).ok()
        }
        // end of month → last day of current month
        5 => Date::new(d.year(), d.month(), 1)
            .ok()?
            .checked_add(Span::new().months(1))
            .ok()?
            .checked_add(Span::new().days(-1))
            .ok(),
        // next year → January 1st of next year
        6 => Date::new(d.year() + 1, 1, 1).ok(),
        _ => None,
    }
}

/// Scan for "next week", "last week", "next month", "last month",
/// "end of week", "end of month".
fn scan_relative_period_phrases(bytes: &[u8], reference_date: Date, best: &mut Option<ParsedDate>) {
    for start in 0..bytes.len() {
        if !has_left_boundary(bytes, start) {
            continue;
        }

        for (i, phrase) in PERIOD_PHRASES.iter().enumerate() {
            if !matches_ascii_insensitive(bytes, start, phrase.as_bytes()) {
                continue;
            }

            let end = start + phrase.len();
            if !has_right_boundary(bytes, end) {
                continue;
            }

            if let Some(date) = resolve_period_phrase(i, reference_date) {
                choose_best(
                    best,
                    ParsedDate {
                        datetime: at_midnight(date),
                        span: (start, end),
                    },
                );
            }
        }
    }
}

fn resolve_in_n_unit(n: u32, unit: &[u8]) -> Option<Span> {
    let lower: Vec<u8> = unit.iter().map(|b| b.to_ascii_lowercase()).collect();
    match lower.as_slice() {
        b"day" | b"days" => Some(Span::new().days(n as i64)),
        b"week" | b"weeks" => Some(Span::new().days(n as i64 * 7)),
        b"month" | b"months" => Some(Span::new().months(n as i64)),
        _ => None,
    }
}

/// Scan for "in N days", "in N weeks", "in N months".
fn scan_in_n_phrases(bytes: &[u8], reference_date: Date, best: &mut Option<ParsedDate>) {
    for start in 0..bytes.len() {
        if !has_left_boundary(bytes, start) {
            continue;
        }

        if !matches_ascii_insensitive(bytes, start, b"in") {
            continue;
        }

        let after_in = start + 2;
        if after_in >= bytes.len() || !bytes[after_in].is_ascii_whitespace() {
            continue;
        }

        let num_start = skip_whitespace(bytes, after_in);
        let Some((n, num_end)) = parse_digits(bytes, num_start, 1, 4) else {
            continue;
        };

        if num_end >= bytes.len() || !bytes[num_end].is_ascii_whitespace() {
            continue;
        }

        let unit_start = skip_whitespace(bytes, num_end);

        // Find the end of the unit word.
        let mut unit_end = unit_start;
        while unit_end < bytes.len() && bytes[unit_end].is_ascii_alphabetic() {
            unit_end += 1;
        }
        if unit_end == unit_start || !has_right_boundary(bytes, unit_end) {
            continue;
        }

        let unit_word = &bytes[unit_start..unit_end];
        if let Some(span) = resolve_in_n_unit(n, unit_word) {
            if let Ok(date) = reference_date.checked_add(span) {
                choose_best(
                    best,
                    ParsedDate {
                        datetime: at_midnight(date),
                        span: (start, unit_end),
                    },
                );
            }
        }
    }
}

fn scan_month_name_dates(bytes: &[u8], reference_date: Date, best: &mut Option<ParsedDate>) {
    for start in 0..bytes.len() {
        if !has_left_boundary(bytes, start) {
            continue;
        }

        for (name, month) in MONTHS.iter().chain(MONTHS_ABBREV.iter()) {
            if !matches_ascii_insensitive(bytes, start, name.as_bytes()) {
                continue;
            }

            let mut pos = start + name.len();
            if pos >= bytes.len() || !bytes[pos].is_ascii_whitespace() {
                continue;
            }

            pos = skip_whitespace(bytes, pos);
            let Some((day, day_end)) = parse_digits(bytes, pos, 1, 2) else {
                continue;
            };

            if day == 0 || day > 31 {
                continue;
            }

            let mut full_date_candidate = None;
            let mut year_pos = day_end;
            let had_comma = year_pos < bytes.len() && bytes[year_pos] == b',';
            if had_comma {
                year_pos += 1;
            }

            let whitespace_start = year_pos;
            year_pos = skip_whitespace(bytes, year_pos);
            let had_space = year_pos > whitespace_start;

            if had_comma || had_space {
                if let Some((year, year_end)) = parse_digits(bytes, year_pos, 4, 4) {
                    if has_right_boundary(bytes, year_end) {
                        if let Ok(date) = Date::new(year as i16, *month as i8, day as i8) {
                            full_date_candidate = Some(ParsedDate {
                                datetime: at_midnight(date),
                                span: (start, year_end),
                            });
                        }
                    }
                }
            }

            if let Some(candidate) = full_date_candidate {
                choose_best(best, candidate);
            }

            if has_right_boundary(bytes, day_end) {
                if let Some(date) = resolve_month_day_without_year(reference_date, *month, day) {
                    choose_best(
                        best,
                        ParsedDate {
                            datetime: at_midnight(date),
                            span: (start, day_end),
                        },
                    );
                }
            }
        }
    }
}

fn scan_iso_dashed_dates(bytes: &[u8], best: &mut Option<ParsedDate>) {
    if bytes.len() < 10 {
        return;
    }

    for start in 0..=bytes.len() - 10 {
        if !has_left_boundary(bytes, start) {
            continue;
        }

        let Some(year) = parse_fixed_digits(bytes, start, 4) else {
            continue;
        };
        if bytes[start + 4] != b'-' {
            continue;
        }

        let Some(month) = parse_fixed_digits(bytes, start + 5, 2) else {
            continue;
        };
        if bytes[start + 7] != b'-' {
            continue;
        }

        let Some(day) = parse_fixed_digits(bytes, start + 8, 2) else {
            continue;
        };

        let end = start + 10;
        if !has_right_boundary(bytes, end) {
            continue;
        }

        if let Ok(date) = Date::new(year as i16, month as i8, day as i8) {
            choose_best(
                best,
                ParsedDate {
                    datetime: at_midnight(date),
                    span: (start, end),
                },
            );
        }
    }
}

fn scan_iso_compact_dates(bytes: &[u8], best: &mut Option<ParsedDate>) {
    if bytes.len() < 8 {
        return;
    }

    for start in 0..=bytes.len() - 8 {
        if !has_left_boundary(bytes, start) {
            continue;
        }

        let Some(year) = parse_fixed_digits(bytes, start, 4) else {
            continue;
        };
        let Some(month) = parse_fixed_digits(bytes, start + 4, 2) else {
            continue;
        };
        let Some(day) = parse_fixed_digits(bytes, start + 6, 2) else {
            continue;
        };

        let end = start + 8;
        if !has_right_boundary(bytes, end) {
            continue;
        }

        if let Ok(date) = Date::new(year as i16, month as i8, day as i8) {
            choose_best(
                best,
                ParsedDate {
                    datetime: at_midnight(date),
                    span: (start, end),
                },
            );
        }
    }
}

fn scan_numeric_mdy_dates(bytes: &[u8], best: &mut Option<ParsedDate>) {
    for start in 0..bytes.len() {
        if !has_left_boundary(bytes, start) {
            continue;
        }

        let Some((month, month_end)) = parse_digits(bytes, start, 1, 2) else {
            continue;
        };
        if month_end >= bytes.len() || bytes[month_end] != b'/' {
            continue;
        }

        let Some((day, day_end)) = parse_digits(bytes, month_end + 1, 1, 2) else {
            continue;
        };
        if day_end >= bytes.len() || bytes[day_end] != b'/' {
            continue;
        }

        let Some((year, year_end)) = parse_digits(bytes, day_end + 1, 2, 2) else {
            continue;
        };
        if !has_right_boundary(bytes, year_end) {
            continue;
        }

        let full_year = 2000 + year as i16;
        if let Ok(date) = Date::new(full_year, month as i8, day as i8) {
            choose_best(
                best,
                ParsedDate {
                    datetime: at_midnight(date),
                    span: (start, year_end),
                },
            );
        }
    }
}

fn resolve_month_day_without_year(reference_date: Date, month: u32, day: u32) -> Option<Date> {
    let this_year = reference_date.year();
    let this_year_date = Date::new(this_year, month as i8, day as i8).ok()?;

    if this_year_date < reference_date {
        Date::new(this_year + 1, month as i8, day as i8).ok()
    } else {
        Some(this_year_date)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ParsedTime {
    hour: u32,
    minute: u32,
    end: usize,
}

fn attach_trailing_time(bytes: &[u8], parsed: ParsedDate) -> ParsedDate {
    let Some(time) = parse_trailing_time(bytes, parsed.span.1) else {
        return parsed;
    };

    let date = parsed.datetime.date();
    let datetime = date.at(time.hour as i8, time.minute as i8, 0, 0);

    ParsedDate {
        datetime,
        span: (parsed.span.0, time.end),
    }
}

fn parse_trailing_time(bytes: &[u8], start_after_date: usize) -> Option<ParsedTime> {
    let mut pos = skip_whitespace(bytes, start_after_date);
    if pos < bytes.len() && bytes[pos] == b',' {
        pos = skip_whitespace(bytes, pos + 1);
    }

    if !matches_ascii_insensitive(bytes, pos, b"at") {
        return None;
    }

    let at_end = pos + 2;
    if at_end >= bytes.len() || !bytes[at_end].is_ascii_whitespace() {
        return None;
    }

    let value_start = skip_whitespace(bytes, at_end);
    parse_time_value(bytes, value_start)
}

fn parse_time_value(bytes: &[u8], start: usize) -> Option<ParsedTime> {
    if matches_ascii_insensitive(bytes, start, b"noon") {
        let end = start + 4;
        if has_right_boundary(bytes, end) {
            return Some(ParsedTime {
                hour: 12,
                minute: 0,
                end,
            });
        }
    }

    let (hour, hour_end) = parse_digits(bytes, start, 1, 2)?;
    let mut minute = 0;
    let mut pos = hour_end;

    if pos < bytes.len() && bytes[pos] == b':' {
        let minute_start = pos + 1;
        let parsed_minute = parse_fixed_digits(bytes, minute_start, 2)?;
        if parsed_minute > 59 {
            return None;
        }

        minute = parsed_minute;
        pos = minute_start + 2;
    }

    if let Some((is_pm, marker_end)) = parse_am_pm(bytes, pos) {
        if hour == 0 || hour > 12 {
            return None;
        }

        let hour_24 = match (hour, is_pm) {
            (12, false) => 0,
            (12, true) => 12,
            (_, true) => hour + 12,
            (_, false) => hour,
        };

        return Some(ParsedTime {
            hour: hour_24,
            minute,
            end: marker_end,
        });
    }

    if pos == hour_end {
        return None;
    }

    if hour > 23 {
        return None;
    }
    if !has_right_boundary(bytes, pos) {
        return None;
    }

    Some(ParsedTime {
        hour,
        minute,
        end: pos,
    })
}

fn parse_am_pm(bytes: &[u8], start: usize) -> Option<(bool, usize)> {
    let pos = skip_whitespace(bytes, start);

    if matches_ascii_insensitive(bytes, pos, b"am") {
        let end = pos + 2;
        if has_right_boundary(bytes, end) {
            return Some((false, end));
        }
    }

    if matches_ascii_insensitive(bytes, pos, b"pm") {
        let end = pos + 2;
        if has_right_boundary(bytes, end) {
            return Some((true, end));
        }
    }

    None
}

fn choose_best(best: &mut Option<ParsedDate>, candidate: ParsedDate) {
    let should_replace = match best {
        None => true,
        Some(current) => {
            let current_len = current.span.1 - current.span.0;
            let candidate_len = candidate.span.1 - candidate.span.0;

            candidate.span.0 < current.span.0
                || (candidate.span.0 == current.span.0 && candidate_len > current_len)
        }
    };

    if should_replace {
        *best = Some(candidate);
    }
}

fn parse_fixed_digits(bytes: &[u8], start: usize, width: usize) -> Option<u32> {
    if start + width > bytes.len() {
        return None;
    }

    let slice = &bytes[start..start + width];
    if !slice.iter().all(|byte| byte.is_ascii_digit()) {
        return None;
    }

    std::str::from_utf8(slice).ok()?.parse().ok()
}

fn parse_digits(
    bytes: &[u8],
    start: usize,
    min_len: usize,
    max_len: usize,
) -> Option<(u32, usize)> {
    if start >= bytes.len() || !bytes[start].is_ascii_digit() {
        return None;
    }

    let mut end = start;
    while end < bytes.len() && bytes[end].is_ascii_digit() && (end - start) < max_len {
        end += 1;
    }

    let digit_count = end - start;
    if digit_count < min_len {
        return None;
    }

    let value = std::str::from_utf8(&bytes[start..end]).ok()?.parse().ok()?;
    Some((value, end))
}

fn matches_ascii_insensitive(haystack: &[u8], start: usize, needle_lower_ascii: &[u8]) -> bool {
    if start + needle_lower_ascii.len() > haystack.len() {
        return false;
    }

    haystack[start..start + needle_lower_ascii.len()]
        .iter()
        .zip(needle_lower_ascii.iter())
        .all(|(input, expected)| input.to_ascii_lowercase() == *expected)
}

fn skip_whitespace(bytes: &[u8], mut pos: usize) -> usize {
    while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
        pos += 1;
    }
    pos
}

fn has_left_boundary(bytes: &[u8], start: usize) -> bool {
    start == 0 || !is_word_byte(bytes[start - 1])
}

fn has_right_boundary(bytes: &[u8], end: usize) -> bool {
    end == bytes.len() || !is_word_byte(bytes[end])
}

fn is_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn at_midnight(date: Date) -> DateTime {
    date.at(0, 0, 0, 0)
}

// ---------------------------------------------------------------------------
// Recurrence-aware parsing
// ---------------------------------------------------------------------------

/// Result of parsing a date expression that may be a one-time date or a
/// recurring pattern.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DateParseResult {
    OneTime(ParsedDate),
    Recurring {
        first_date: ParsedDate,
        rule: RecurrenceRule,
    },
}

impl BasicDateParser {
    /// Parse `text` and detect recurrence patterns ("every Monday", "daily",
    /// "monthly on the 15th") in addition to one-time dates.
    ///
    /// Recurrence scanners run first; if none match, falls back to the standard
    /// `DateParser::parse` wrapped in `OneTime`.
    pub fn parse_with_recurrence(
        &self,
        text: &str,
        reference_date: Date,
    ) -> Option<DateParseResult> {
        if let Some(result) = scan_recurrence(text.as_bytes(), reference_date) {
            return Some(result);
        }
        self.parse(text, reference_date)
            .map(DateParseResult::OneTime)
    }
}

/// Scan for recurrence patterns. Returns the best match or None.
fn scan_recurrence(bytes: &[u8], reference_date: Date) -> Option<DateParseResult> {
    let mut best: Option<DateParseResult> = None;

    scan_every_weekday(bytes, reference_date, &mut best);
    scan_every_business_day(bytes, reference_date, &mut best);
    scan_every_n_unit(bytes, reference_date, &mut best);
    scan_every_month_day(bytes, reference_date, &mut best);
    scan_ordinal_of_every_month(bytes, reference_date, &mut best);
    scan_every_month_date(bytes, reference_date, &mut best);
    scan_single_word_frequency(bytes, reference_date, &mut best);

    // Attach trailing time ("at 6pm", "at 9:30am") to the first_date if present.
    let best = best.map(|result| match result {
        DateParseResult::Recurring { first_date, rule } => DateParseResult::Recurring {
            first_date: attach_trailing_time(bytes, first_date),
            rule,
        },
        other => other,
    });

    // Attach "starting <date>" anchor to override first_date if present.
    best.map(|result| match result {
        DateParseResult::Recurring { first_date, rule } => {
            let with_anchor = attach_starting_anchor(bytes, first_date, &rule, reference_date);
            DateParseResult::Recurring {
                first_date: with_anchor,
                rule,
            }
        }
        other => other,
    })
}

/// Check for "starting <date>" after the recurrence span and override first_date.
fn attach_starting_anchor(
    bytes: &[u8],
    parsed: ParsedDate,
    rule: &RecurrenceRule,
    reference_date: Date,
) -> ParsedDate {
    let mut pos = skip_whitespace(bytes, parsed.span.1);
    if !matches_ascii_insensitive(bytes, pos, b"starting ") {
        return parsed;
    }
    pos += 9; // len("starting ")
    pos = skip_whitespace(bytes, pos);

    // Parse the anchor date using the standard one-time parser.
    let remaining = &bytes[pos..];
    let remaining_str = match std::str::from_utf8(remaining) {
        Ok(s) => s,
        Err(_) => return parsed,
    };
    let anchor_parser = BasicDateParser::default();
    let Some(anchor_parsed) = anchor_parser.parse(remaining_str, reference_date) else {
        return parsed;
    };
    let anchor_date = anchor_parsed.datetime.date();

    // For weekly rules with a specific weekday, find the first occurrence
    // of that weekday on or after the anchor date.
    let first_date = if rule.frequency == RecurrenceFrequency::Weekly {
        if let Some(wd_u8) = rule.weekday {
            let target_wd = crate::model::weekday_from_u8(wd_u8);
            let days_ahead = days_until_weekday_this(anchor_date.weekday(), target_wd);
            anchor_date
                .checked_add(Span::new().days(days_ahead))
                .unwrap_or(anchor_date)
        } else {
            anchor_date
        }
    } else {
        anchor_date
    };

    // Preserve time from the original parsed result (e.g., "at 6pm" was already attached).
    let datetime = first_date.at(
        parsed.datetime.hour(),
        parsed.datetime.minute(),
        parsed.datetime.second(),
        0,
    );
    let anchor_span_end = pos + anchor_parsed.span.1;
    ParsedDate {
        datetime,
        span: (parsed.span.0, anchor_span_end),
    }
}

fn choose_best_recurrence(best: &mut Option<DateParseResult>, candidate: DateParseResult) {
    let candidate_span = match &candidate {
        DateParseResult::OneTime(p) => p.span,
        DateParseResult::Recurring { first_date, .. } => first_date.span,
    };
    let should_replace = match best {
        None => true,
        Some(current) => {
            let current_span = match current {
                DateParseResult::OneTime(p) => p.span,
                DateParseResult::Recurring { first_date, .. } => first_date.span,
            };
            let current_len = current_span.1 - current_span.0;
            let candidate_len = candidate_span.1 - candidate_span.0;
            candidate_span.0 < current_span.0
                || (candidate_span.0 == current_span.0 && candidate_len > current_len)
        }
    };
    if should_replace {
        *best = Some(candidate);
    }
}

/// "every Monday", "every friday", "every other tuesday"
fn scan_every_weekday(bytes: &[u8], reference_date: Date, best: &mut Option<DateParseResult>) {
    let every_keywords = [b"every " as &[u8], b"each "];
    for start in 0..bytes.len() {
        if !has_left_boundary(bytes, start) {
            continue;
        }
        for keyword in &every_keywords {
            if !matches_ascii_insensitive(bytes, start, keyword) {
                continue;
            }
            let after_every = start + keyword.len();
            // Check for "other " modifier → interval=2
            let (wd_start, interval) = if matches_ascii_insensitive(bytes, after_every, b"other ") {
                (after_every + 6, 2u16)
            } else {
                (after_every, 1u16)
            };
            for &(name, weekday) in &WEEKDAYS {
                if !matches_ascii_insensitive(bytes, wd_start, name.as_bytes()) {
                    continue;
                }
                let end = wd_start + name.len();
                if !has_right_boundary(bytes, end) {
                    continue;
                }
                let days = days_until_weekday_this(reference_date.weekday(), weekday);
                let days = if days == 0 { 7 } else { days };
                if let Ok(first) = reference_date.checked_add(Span::new().days(days)) {
                    let rule = RecurrenceRule {
                        frequency: RecurrenceFrequency::Weekly,
                        interval,
                        weekday: Some(weekday_to_u8(weekday)),
                        day_of_month: None,
                        month: None,
                        weekdays_only: None,
                    };
                    choose_best_recurrence(
                        best,
                        DateParseResult::Recurring {
                            first_date: ParsedDate {
                                datetime: at_midnight(first),
                                span: (start, end),
                            },
                            rule,
                        },
                    );
                }
            }
        }
    }
}

/// "every weekday", "every business day"
fn scan_every_business_day(bytes: &[u8], reference_date: Date, best: &mut Option<DateParseResult>) {
    let patterns: &[&[u8]] = &[
        b"every weekday",
        b"each weekday",
        b"every business day",
        b"each business day",
    ];
    for start in 0..bytes.len() {
        if !has_left_boundary(bytes, start) {
            continue;
        }
        for pattern in patterns {
            if !matches_ascii_insensitive(bytes, start, pattern) {
                continue;
            }
            let end = start + pattern.len();
            if !has_right_boundary(bytes, end) {
                continue;
            }
            // first_date: next weekday (Mon–Fri) from reference_date
            let mut first = reference_date
                .checked_add(Span::new().days(1))
                .expect("day advance overflow");
            loop {
                match first.weekday() {
                    Weekday::Saturday => {
                        first = first.checked_add(Span::new().days(2)).unwrap();
                    }
                    Weekday::Sunday => {
                        first = first.checked_add(Span::new().days(1)).unwrap();
                    }
                    _ => break,
                }
            }
            let rule = RecurrenceRule {
                frequency: RecurrenceFrequency::Daily,
                interval: 1,
                weekday: None,
                day_of_month: None,
                month: None,
                weekdays_only: Some(true),
            };
            choose_best_recurrence(
                best,
                DateParseResult::Recurring {
                    first_date: ParsedDate {
                        datetime: at_midnight(first),
                        span: (start, end),
                    },
                    rule,
                },
            );
        }
    }
}

/// "every 2 weeks", "every 3 months", "every 5 days"
fn scan_every_n_unit(bytes: &[u8], reference_date: Date, best: &mut Option<DateParseResult>) {
    let every_keywords = [b"every " as &[u8], b"each "];
    for start in 0..bytes.len() {
        if !has_left_boundary(bytes, start) {
            continue;
        }
        for keyword in &every_keywords {
            if !matches_ascii_insensitive(bytes, start, keyword) {
                continue;
            }
            let num_start = start + keyword.len();
            // Parse a number
            let mut num_end = num_start;
            while num_end < bytes.len() && bytes[num_end].is_ascii_digit() {
                num_end += 1;
            }
            if num_end == num_start {
                continue;
            }
            let n: u16 = match std::str::from_utf8(&bytes[num_start..num_end])
                .ok()
                .and_then(|s| s.parse().ok())
            {
                Some(n) if n > 0 => n,
                _ => continue,
            };
            // Skip space
            if num_end >= bytes.len() || bytes[num_end] != b' ' {
                continue;
            }
            let unit_start = num_end + 1;

            let units: &[(&[u8], RecurrenceFrequency)] = &[
                (b"day", RecurrenceFrequency::Daily),
                (b"days", RecurrenceFrequency::Daily),
                (b"week", RecurrenceFrequency::Weekly),
                (b"weeks", RecurrenceFrequency::Weekly),
                (b"month", RecurrenceFrequency::Monthly),
                (b"months", RecurrenceFrequency::Monthly),
                (b"year", RecurrenceFrequency::Yearly),
                (b"years", RecurrenceFrequency::Yearly),
            ];

            for &(unit_word, freq) in units {
                if !matches_ascii_insensitive(bytes, unit_start, unit_word) {
                    continue;
                }
                let end = unit_start + unit_word.len();
                if !has_right_boundary(bytes, end) {
                    continue;
                }
                let first = match freq {
                    RecurrenceFrequency::Daily => reference_date
                        .checked_add(Span::new().days(i64::from(n)))
                        .ok(),
                    RecurrenceFrequency::Weekly => reference_date
                        .checked_add(Span::new().weeks(i64::from(n)))
                        .ok(),
                    RecurrenceFrequency::Monthly => reference_date
                        .checked_add(Span::new().months(i32::from(n)))
                        .ok(),
                    RecurrenceFrequency::Yearly => reference_date
                        .checked_add(Span::new().years(i32::from(n)))
                        .ok(),
                };
                if let Some(first_date) = first {
                    let rule = RecurrenceRule {
                        frequency: freq,
                        interval: n,
                        weekday: None,
                        day_of_month: None,
                        month: None,
                        weekdays_only: None,
                    };
                    choose_best_recurrence(
                        best,
                        DateParseResult::Recurring {
                            first_date: ParsedDate {
                                datetime: at_midnight(first_date),
                                span: (start, end),
                            },
                            rule,
                        },
                    );
                }
            }
        }
    }
}

/// "every month on the 15th", "monthly on the 1st"
fn scan_every_month_day(bytes: &[u8], reference_date: Date, best: &mut Option<DateParseResult>) {
    let prefixes: &[&[u8]] = &[
        b"every month on the ",
        b"each month on the ",
        b"monthly on the ",
    ];
    for start in 0..bytes.len() {
        if !has_left_boundary(bytes, start) {
            continue;
        }
        for prefix in prefixes {
            if !matches_ascii_insensitive(bytes, start, prefix) {
                continue;
            }
            let num_start = start + prefix.len();
            let mut num_end = num_start;
            while num_end < bytes.len() && bytes[num_end].is_ascii_digit() {
                num_end += 1;
            }
            if num_end == num_start {
                continue;
            }
            let day: u8 = match std::str::from_utf8(&bytes[num_start..num_end])
                .ok()
                .and_then(|s| s.parse().ok())
            {
                Some(d) if (1..=31).contains(&d) => d,
                _ => continue,
            };
            // Accept optional ordinal suffix (st, nd, rd, th)
            let mut end = num_end;
            if end + 2 <= bytes.len() {
                let suffix = &bytes[end..end + 2];
                if suffix.eq_ignore_ascii_case(b"st")
                    || suffix.eq_ignore_ascii_case(b"nd")
                    || suffix.eq_ignore_ascii_case(b"rd")
                    || suffix.eq_ignore_ascii_case(b"th")
                {
                    end += 2;
                }
            }
            if !has_right_boundary(bytes, end) {
                continue;
            }
            // Compute first date: the target day in the next applicable month
            let target_month = if reference_date.day() as u8 >= day {
                // Already past this day in current month, go to next
                reference_date.checked_add(Span::new().months(1)).ok()
            } else {
                Some(reference_date)
            };
            if let Some(base) = target_month {
                let max_day =
                    crate::model::days_in_month(i32::from(base.year()), base.month() as u8);
                let clamped = day.min(max_day);
                if let Ok(first) = Date::new(base.year(), base.month(), clamped as i8) {
                    let rule = RecurrenceRule {
                        frequency: RecurrenceFrequency::Monthly,
                        interval: 1,
                        weekday: None,
                        day_of_month: Some(day),
                        month: None,
                        weekdays_only: None,
                    };
                    choose_best_recurrence(
                        best,
                        DateParseResult::Recurring {
                            first_date: ParsedDate {
                                datetime: at_midnight(first),
                                span: (start, end),
                            },
                            rule,
                        },
                    );
                }
            }
        }
    }
}

/// "1st of every month", "13th of each month"
fn scan_ordinal_of_every_month(
    bytes: &[u8],
    reference_date: Date,
    best: &mut Option<DateParseResult>,
) {
    for start in 0..bytes.len() {
        if !has_left_boundary(bytes, start) {
            continue;
        }
        // Parse day digits
        let Some((day, day_end)) = parse_digits(bytes, start, 1, 2) else {
            continue;
        };
        if day == 0 || day > 31 {
            continue;
        }
        let day = day as u8;
        // Skip optional ordinal suffix
        let mut pos = day_end;
        if pos + 2 <= bytes.len() {
            let suffix = &bytes[pos..pos + 2];
            if suffix.eq_ignore_ascii_case(b"st")
                || suffix.eq_ignore_ascii_case(b"nd")
                || suffix.eq_ignore_ascii_case(b"rd")
                || suffix.eq_ignore_ascii_case(b"th")
            {
                pos += 2;
            }
        }
        // " of every month" or " of each month"
        let pos = skip_whitespace(bytes, pos);
        if !matches_ascii_insensitive(bytes, pos, b"of ") {
            continue;
        }
        let pos = skip_whitespace(bytes, pos + 3);
        let after_keyword = if matches_ascii_insensitive(bytes, pos, b"every ") {
            pos + 6
        } else if matches_ascii_insensitive(bytes, pos, b"each ") {
            pos + 5
        } else {
            continue;
        };
        if !matches_ascii_insensitive(bytes, after_keyword, b"month") {
            continue;
        }
        let end = after_keyword + 5;
        if !has_right_boundary(bytes, end) {
            continue;
        }
        // Compute first date (same logic as scan_every_month_day)
        let target_month = if reference_date.day() as u8 >= day {
            reference_date.checked_add(Span::new().months(1)).ok()
        } else {
            Some(reference_date)
        };
        if let Some(base) = target_month {
            let max_day = crate::model::days_in_month(i32::from(base.year()), base.month() as u8);
            let clamped = day.min(max_day);
            if let Ok(first) = Date::new(base.year(), base.month(), clamped as i8) {
                choose_best_recurrence(
                    best,
                    DateParseResult::Recurring {
                        first_date: ParsedDate {
                            datetime: at_midnight(first),
                            span: (start, end),
                        },
                        rule: RecurrenceRule {
                            frequency: RecurrenceFrequency::Monthly,
                            interval: 1,
                            weekday: None,
                            day_of_month: Some(day),
                            month: None,
                            weekdays_only: None,
                        },
                    },
                );
            }
        }
    }
}

/// "every january 1", "every mar 15", "every march 15th"
fn scan_every_month_date(bytes: &[u8], reference_date: Date, best: &mut Option<DateParseResult>) {
    let every_keywords = [b"every " as &[u8], b"each "];
    for start in 0..bytes.len() {
        if !has_left_boundary(bytes, start) {
            continue;
        }
        for keyword in &every_keywords {
            if !matches_ascii_insensitive(bytes, start, keyword) {
                continue;
            }
            let month_start = start + keyword.len();
            // Try full month names first, then abbreviations
            let mut found_month = None;
            for &(name, month_num) in MONTHS.iter().chain(MONTHS_ABBREV.iter()) {
                if matches_ascii_insensitive(bytes, month_start, name.as_bytes()) {
                    let after_name = month_start + name.len();
                    // Must be followed by space (not just a boundary — "may5" shouldn't match)
                    if after_name < bytes.len() && bytes[after_name].is_ascii_whitespace() {
                        // For abbreviations, make sure a longer full name doesn't also match
                        // (prefer full name). Since full names come first, just take first match.
                        if found_month.is_none()
                            || name.len()
                                > found_month
                                    .map(|(_, _, len): (u32, usize, usize)| len)
                                    .unwrap_or(0)
                        {
                            found_month = Some((month_num, after_name, name.len()));
                        }
                    }
                }
            }
            let Some((month_num, after_name, _)) = found_month else {
                continue;
            };
            let day_start = skip_whitespace(bytes, after_name);
            let Some((day, day_end)) = parse_digits(bytes, day_start, 1, 2) else {
                continue;
            };
            if day == 0 || day > 31 {
                continue;
            }
            // Skip optional ordinal suffix
            let mut end = day_end;
            if end + 2 <= bytes.len() {
                let suffix = &bytes[end..end + 2];
                if suffix.eq_ignore_ascii_case(b"st")
                    || suffix.eq_ignore_ascii_case(b"nd")
                    || suffix.eq_ignore_ascii_case(b"rd")
                    || suffix.eq_ignore_ascii_case(b"th")
                {
                    end += 2;
                }
            }
            if !has_right_boundary(bytes, end) {
                continue;
            }
            let day = day as u8;
            let month = month_num as u8;
            // Compute first_date: this year if not past, else next year
            let this_year = reference_date.year();
            let max_day = crate::model::days_in_month(i32::from(this_year), month);
            let clamped = day.min(max_day);
            let (first_year, first_day) =
                if let Ok(candidate) = Date::new(this_year, month as i8, clamped as i8) {
                    if candidate > reference_date {
                        (this_year, clamped)
                    } else {
                        let next_max = crate::model::days_in_month(i32::from(this_year + 1), month);
                        (this_year + 1, day.min(next_max))
                    }
                } else {
                    continue;
                };
            if let Ok(first) = Date::new(first_year, month as i8, first_day as i8) {
                choose_best_recurrence(
                    best,
                    DateParseResult::Recurring {
                        first_date: ParsedDate {
                            datetime: at_midnight(first),
                            span: (start, end),
                        },
                        rule: RecurrenceRule {
                            frequency: RecurrenceFrequency::Yearly,
                            interval: 1,
                            weekday: None,
                            day_of_month: Some(day),
                            month: Some(month),
                            weekdays_only: None,
                        },
                    },
                );
            }
        }
    }
}

/// "daily", "weekly", "monthly", "yearly", "quarterly", "biweekly", "bimonthly"
fn scan_single_word_frequency(
    bytes: &[u8],
    reference_date: Date,
    best: &mut Option<DateParseResult>,
) {
    let keywords: &[(&[u8], RecurrenceFrequency)] = &[
        (b"daily", RecurrenceFrequency::Daily),
        (b"weekly", RecurrenceFrequency::Weekly),
        (b"monthly", RecurrenceFrequency::Monthly),
        (b"yearly", RecurrenceFrequency::Yearly),
        (b"annually", RecurrenceFrequency::Yearly),
        (b"biweekly", RecurrenceFrequency::Weekly),
        (b"bimonthly", RecurrenceFrequency::Monthly),
        (b"quarterly", RecurrenceFrequency::Monthly),
    ];
    for start in 0..bytes.len() {
        if !has_left_boundary(bytes, start) {
            continue;
        }
        for &(keyword, freq) in keywords {
            if !matches_ascii_insensitive(bytes, start, keyword) {
                continue;
            }
            let end = start + keyword.len();
            // "monthly on the Nth" is handled by scan_every_month_day; skip if followed by " on"
            if freq == RecurrenceFrequency::Monthly
                && end + 4 <= bytes.len()
                && matches_ascii_insensitive(bytes, end, b" on ")
            {
                continue;
            }
            if !has_right_boundary(bytes, end) {
                continue;
            }
            let is_bi = keyword == b"biweekly" || keyword == b"bimonthly";
            let is_quarterly = keyword == b"quarterly";
            let interval: u16 = if is_bi {
                2
            } else if is_quarterly {
                3
            } else {
                1
            };
            let (first, day_of_month) = if is_quarterly {
                // Snap to next quarter boundary: Jan 1, Apr 1, Jul 1, Oct 1.
                let m = reference_date.month() as u32;
                let next_q = ((m - 1) / 3 + 1) * 3 + 1; // 4, 7, 10, or 13
                let (y, qm) = if next_q > 12 {
                    (reference_date.year() + 1, 1u8)
                } else {
                    (reference_date.year(), next_q as u8)
                };
                (Date::new(y, qm as i8, 1).ok(), Some(1u8))
            } else {
                let first = match freq {
                    RecurrenceFrequency::Daily => reference_date
                        .checked_add(Span::new().days(i64::from(interval)))
                        .ok(),
                    RecurrenceFrequency::Weekly => reference_date
                        .checked_add(Span::new().weeks(i64::from(interval)))
                        .ok(),
                    RecurrenceFrequency::Monthly => reference_date
                        .checked_add(Span::new().months(i32::from(interval)))
                        .ok(),
                    RecurrenceFrequency::Yearly => reference_date
                        .checked_add(Span::new().years(i32::from(interval)))
                        .ok(),
                };
                (first, None)
            };
            if let Some(first_date) = first {
                let rule = RecurrenceRule {
                    frequency: freq,
                    interval,
                    weekday: None,
                    day_of_month,
                    month: None,
                    weekdays_only: None,
                };
                choose_best_recurrence(
                    best,
                    DateParseResult::Recurring {
                        first_date: ParsedDate {
                            datetime: at_midnight(first_date),
                            span: (start, end),
                        },
                        rule,
                    },
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{BasicDateParser, DateParser, WeekdayDisambiguationPolicy};
    use jiff::civil::{Date, DateTime};

    fn date(y: i16, m: i8, d: i8) -> Date {
        Date::new(y, m, d).expect("valid date")
    }

    fn datetime(y: i16, m: i8, d: i8, h: i8, min: i8) -> DateTime {
        date(y, m, d).at(h, min, 0, 0)
    }

    fn parser_with_policy(policy: WeekdayDisambiguationPolicy) -> BasicDateParser {
        BasicDateParser::with_weekday_policy(policy)
    }

    #[test]
    fn month_name_full_date_parses_with_exact_span() {
        let parser = BasicDateParser::default();
        let text = "meet May 25, 2026";

        let parsed = parser
            .parse(text, date(2026, 2, 16))
            .expect("expected parse");

        let expected = "May 25, 2026";
        let start = text
            .find(expected)
            .expect("expected substring in test text");
        let end = start + expected.len();

        assert_eq!(parsed.datetime, datetime(2026, 5, 25, 0, 0));
        assert_eq!(parsed.span, (start, end));
        assert_eq!(&text[parsed.span.0..parsed.span.1], expected);
    }

    #[test]
    fn iso_dashed_date_parses() {
        let parser = BasicDateParser::default();
        let text = "deadline 2026-05-25";

        let parsed = parser
            .parse(text, date(2026, 2, 16))
            .expect("expected parse");

        assert_eq!(parsed.datetime, datetime(2026, 5, 25, 0, 0));
        assert_eq!(&text[parsed.span.0..parsed.span.1], "2026-05-25");
    }

    #[test]
    fn iso_compact_date_parses() {
        let parser = BasicDateParser::default();
        let text = "deadline 20260525";

        let parsed = parser
            .parse(text, date(2026, 2, 16))
            .expect("expected parse");

        assert_eq!(parsed.datetime, datetime(2026, 5, 25, 0, 0));
        assert_eq!(&text[parsed.span.0..parsed.span.1], "20260525");
    }

    #[test]
    fn numeric_mdy_with_two_digit_year_parses() {
        let parser = BasicDateParser::default();
        let text = "ship by 12/5/26";

        let parsed = parser
            .parse(text, date(2026, 2, 16))
            .expect("expected parse");

        assert_eq!(parsed.datetime, datetime(2026, 12, 5, 0, 0));
        assert_eq!(&text[parsed.span.0..parsed.span.1], "12/5/26");
    }

    #[test]
    fn month_day_without_year_stays_in_reference_year_when_not_past() {
        let parser = BasicDateParser::default();

        let parsed = parser
            .parse("December 5", date(2026, 2, 16))
            .expect("expected parse");

        assert_eq!(parsed.datetime, datetime(2026, 12, 5, 0, 0));
    }

    #[test]
    fn month_day_without_year_rolls_forward_if_past() {
        let parser = BasicDateParser::default();

        let parsed = parser
            .parse("December 5", date(2026, 12, 10))
            .expect("expected parse");

        assert_eq!(parsed.datetime, datetime(2027, 12, 5, 0, 0));
    }

    #[test]
    fn invalid_date_is_rejected() {
        let parser = BasicDateParser::default();

        assert_eq!(parser.parse("2026-02-30", date(2026, 2, 16)), None);
    }

    #[test]
    fn today_parses_with_exact_span() {
        let parser = BasicDateParser::default();
        let text = "do this today please";

        let parsed = parser
            .parse(text, date(2026, 2, 16))
            .expect("expected parse");

        assert_eq!(parsed.datetime, datetime(2026, 2, 16, 0, 0));
        assert_eq!(&text[parsed.span.0..parsed.span.1], "today");
    }

    #[test]
    fn tomorrow_and_yesterday_parse_relative_to_reference_date() {
        let parser = BasicDateParser::default();

        let tomorrow = parser
            .parse("tomorrow", date(2026, 2, 16))
            .expect("expected parse");
        let yesterday = parser
            .parse("yesterday", date(2026, 2, 16))
            .expect("expected parse");

        assert_eq!(tomorrow.datetime, datetime(2026, 2, 17, 0, 0));
        assert_eq!(yesterday.datetime, datetime(2026, 2, 15, 0, 0));
    }

    #[test]
    fn default_weekday_disambiguation_policy_is_strict_next_week() {
        assert_eq!(
            BasicDateParser::default().weekday_policy(),
            WeekdayDisambiguationPolicy::StrictNextWeek
        );
    }

    #[test]
    fn this_and_next_tuesday_behavior_is_pinned_per_mode() {
        let reference = date(2026, 2, 16); // Monday
        let strict = parser_with_policy(WeekdayDisambiguationPolicy::StrictNextWeek);
        let inclusive = parser_with_policy(WeekdayDisambiguationPolicy::InclusiveNext);

        let strict_this = strict
            .parse("this Tuesday", reference)
            .expect("expected strict this parse");
        let strict_next = strict
            .parse("next Tuesday", reference)
            .expect("expected strict next parse");
        let inclusive_this = inclusive
            .parse("this Tuesday", reference)
            .expect("expected inclusive this parse");
        let inclusive_next = inclusive
            .parse("next Tuesday", reference)
            .expect("expected inclusive next parse");

        assert_eq!(strict_this.datetime, datetime(2026, 2, 17, 0, 0));
        assert_eq!(strict_next.datetime, datetime(2026, 2, 24, 0, 0));
        assert_eq!(inclusive_this.datetime, datetime(2026, 2, 17, 0, 0));
        assert_eq!(inclusive_next.datetime, datetime(2026, 2, 17, 0, 0));
    }

    #[test]
    fn weekday_disambiguation_modes_are_deterministic_for_all_weekdays() {
        let reference = date(2026, 2, 16); // Monday
        let strict = parser_with_policy(WeekdayDisambiguationPolicy::StrictNextWeek);
        let inclusive = parser_with_policy(WeekdayDisambiguationPolicy::InclusiveNext);

        struct Case {
            weekday: &'static str,
            this_expected: DateTime,
            strict_next_expected: DateTime,
            inclusive_next_expected: DateTime,
        }

        let cases = [
            Case {
                weekday: "Monday",
                this_expected: datetime(2026, 2, 16, 0, 0),
                strict_next_expected: datetime(2026, 2, 23, 0, 0),
                inclusive_next_expected: datetime(2026, 2, 23, 0, 0),
            },
            Case {
                weekday: "Tuesday",
                this_expected: datetime(2026, 2, 17, 0, 0),
                strict_next_expected: datetime(2026, 2, 24, 0, 0),
                inclusive_next_expected: datetime(2026, 2, 17, 0, 0),
            },
            Case {
                weekday: "Wednesday",
                this_expected: datetime(2026, 2, 18, 0, 0),
                strict_next_expected: datetime(2026, 2, 25, 0, 0),
                inclusive_next_expected: datetime(2026, 2, 18, 0, 0),
            },
            Case {
                weekday: "Thursday",
                this_expected: datetime(2026, 2, 19, 0, 0),
                strict_next_expected: datetime(2026, 2, 26, 0, 0),
                inclusive_next_expected: datetime(2026, 2, 19, 0, 0),
            },
            Case {
                weekday: "Friday",
                this_expected: datetime(2026, 2, 20, 0, 0),
                strict_next_expected: datetime(2026, 2, 27, 0, 0),
                inclusive_next_expected: datetime(2026, 2, 20, 0, 0),
            },
            Case {
                weekday: "Saturday",
                this_expected: datetime(2026, 2, 21, 0, 0),
                strict_next_expected: datetime(2026, 2, 28, 0, 0),
                inclusive_next_expected: datetime(2026, 2, 21, 0, 0),
            },
            Case {
                weekday: "Sunday",
                this_expected: datetime(2026, 2, 22, 0, 0),
                strict_next_expected: datetime(2026, 3, 1, 0, 0),
                inclusive_next_expected: datetime(2026, 2, 22, 0, 0),
            },
        ];

        for case in cases {
            let this_phrase = format!("this {}", case.weekday);
            let next_phrase = format!("next {}", case.weekday);

            let strict_this = strict
                .parse(&this_phrase, reference)
                .expect("expected strict this parse");
            let strict_next = strict
                .parse(&next_phrase, reference)
                .expect("expected strict next parse");
            let inclusive_this = inclusive
                .parse(&this_phrase, reference)
                .expect("expected inclusive this parse");
            let inclusive_next = inclusive
                .parse(&next_phrase, reference)
                .expect("expected inclusive next parse");

            assert_eq!(
                strict_this.datetime, case.this_expected,
                "strict this failed"
            );
            assert_eq!(
                strict_next.datetime, case.strict_next_expected,
                "strict next failed"
            );
            assert_eq!(
                inclusive_this.datetime, case.this_expected,
                "inclusive this failed"
            );
            assert_eq!(
                inclusive_next.datetime, case.inclusive_next_expected,
                "inclusive next failed"
            );
        }
    }

    #[test]
    fn relative_weekday_parsing_is_case_insensitive() {
        let parser = BasicDateParser::default();

        let parsed = parser
            .parse("NEXT tuesday", date(2026, 2, 16))
            .expect("expected parse");

        assert_eq!(parsed.datetime, datetime(2026, 2, 24, 0, 0));
    }

    #[test]
    fn relative_phrase_boundaries_prevent_false_positives() {
        let parser = BasicDateParser::default();

        assert_eq!(parser.parse("todayish", date(2026, 2, 16)), None);
        // "annext tuesday" — "annext" is not "next", but bare "tuesday" is valid.
        let parsed = parser
            .parse("annext tuesday", date(2026, 2, 16))
            .expect("bare weekday should match");
        assert_eq!(&"annext tuesday"[parsed.span.0..parsed.span.1], "tuesday");
    }

    #[test]
    fn compound_with_12_hour_time_parses() {
        let parser = BasicDateParser::default();
        let text = "next Tuesday at 3pm";

        let parsed = parser
            .parse(text, date(2026, 2, 18))
            .expect("expected parse");

        assert_eq!(parsed.datetime, datetime(2026, 2, 24, 15, 0));
        assert_eq!(parsed.span, (0, text.len()));
        assert_eq!(&text[parsed.span.0..parsed.span.1], text);
    }

    #[test]
    fn compound_with_24_hour_time_parses() {
        let parser = BasicDateParser::default();
        let text = "meet May 25, 2026 at 15:00";

        let parsed = parser
            .parse(text, date(2026, 2, 16))
            .expect("expected parse");

        assert_eq!(parsed.datetime, datetime(2026, 5, 25, 15, 0));
        assert_eq!(&text[parsed.span.0..parsed.span.1], "May 25, 2026 at 15:00");
    }

    #[test]
    fn compound_with_noon_parses() {
        let parser = BasicDateParser::default();
        let text = "today at noon";

        let parsed = parser
            .parse(text, date(2026, 2, 16))
            .expect("expected parse");

        assert_eq!(parsed.datetime, datetime(2026, 2, 16, 12, 0));
        assert_eq!(parsed.span, (0, text.len()));
    }

    #[test]
    fn invalid_compound_time_falls_back_to_date_only() {
        let parser = BasicDateParser::default();

        let parsed = parser
            .parse("today at 25:00", date(2026, 2, 16))
            .expect("expected parse");

        assert_eq!(parsed.datetime, datetime(2026, 2, 16, 0, 0));
        assert_eq!(parsed.span, (0, 5));
    }

    #[test]
    fn compound_time_parsing_is_case_insensitive() {
        let parser = BasicDateParser::default();

        let pm = parser
            .parse("today AT 3PM", date(2026, 2, 16))
            .expect("expected parse");
        let noon = parser
            .parse("today at NOON", date(2026, 2, 16))
            .expect("expected parse");

        assert_eq!(pm.datetime, datetime(2026, 2, 16, 15, 0));
        assert_eq!(noon.datetime, datetime(2026, 2, 16, 12, 0));
    }

    #[test]
    fn time_only_phrases_are_still_out_of_scope() {
        let parser = BasicDateParser::default();

        assert_eq!(parser.parse("at 3pm", date(2026, 2, 16)), None);
        assert_eq!(parser.parse("at 15:00", date(2026, 2, 16)), None);
        assert_eq!(parser.parse("at noon", date(2026, 2, 16)), None);
    }

    #[test]
    fn non_date_text_does_not_false_positive() {
        let parser = BasicDateParser::default();

        assert_eq!(parser.parse("May I ask", date(2026, 2, 16)), None);
    }

    // ── Year-boundary edge cases ───────────────────────────────────────────────

    #[test]
    fn yesterday_on_jan_1_wraps_to_previous_year() {
        let parser = BasicDateParser::default();
        let parsed = parser
            .parse("yesterday", date(2026, 1, 1))
            .expect("expected parse");
        assert_eq!(parsed.datetime, datetime(2025, 12, 31, 0, 0));
    }

    #[test]
    fn tomorrow_on_dec_31_wraps_to_next_year() {
        let parser = BasicDateParser::default();
        let parsed = parser
            .parse("tomorrow", date(2025, 12, 31))
            .expect("expected parse");
        assert_eq!(parsed.datetime, datetime(2026, 1, 1, 0, 0));
    }

    #[test]
    fn this_weekday_crossing_year_boundary() {
        // 2026-12-28 is a Monday; "this Sunday" is 6 days later = 2027-01-03.
        let parser = BasicDateParser::default();
        let parsed = parser
            .parse("this Sunday", date(2026, 12, 28))
            .expect("expected parse");
        assert_eq!(parsed.datetime, datetime(2027, 1, 3, 0, 0));
    }

    #[test]
    fn next_weekday_crossing_year_boundary_strict() {
        // 2026-12-28 is a Monday; "next Sunday" (StrictNextWeek) is 13 days
        // later = 2027-01-10.
        let parser = parser_with_policy(WeekdayDisambiguationPolicy::StrictNextWeek);
        let parsed = parser
            .parse("next Sunday", date(2026, 12, 28))
            .expect("expected parse");
        assert_eq!(parsed.datetime, datetime(2027, 1, 10, 0, 0));
    }

    // ── Leap-year edge cases ───────────────────────────────────────────────────

    #[test]
    fn feb_29_parses_on_leap_year() {
        // 2024 is a leap year; Feb 29 should be accepted.
        let parser = BasicDateParser::default();
        let parsed = parser
            .parse("February 29", date(2024, 1, 1))
            .expect("expected parse on leap year");
        assert_eq!(parsed.datetime, datetime(2024, 2, 29, 0, 0));
    }

    #[test]
    fn feb_29_on_non_leap_year_is_rejected() {
        // 2026 is not a leap year; Feb 29 2026 is not a valid date.
        let parser = BasicDateParser::default();
        assert_eq!(
            parser.parse("February 29", date(2026, 1, 1)),
            None,
            "Feb 29 should not parse in a non-leap year"
        );
    }

    #[test]
    fn feb_29_rolls_forward_past_reference_date_returns_none_when_next_year_is_not_leap() {
        // Reference is 2024-03-01, which is after Feb 29 2024.
        // resolve_month_day_without_year only tries this_year + 1 (= 2025),
        // which is not a leap year, so Feb 29 is invalid → None.
        let parser = BasicDateParser::default();
        assert_eq!(
            parser.parse("February 29", date(2024, 3, 1)),
            None,
            "when next year is not a leap year, Feb 29 roll-forward should return None"
        );
    }

    // ── Relative period phrases ──────────────────────────────────────────────

    #[test]
    fn next_week_resolves_to_monday_of_following_week() {
        let parser = BasicDateParser::default();
        // 2026-02-18 is a Wednesday
        let parsed = parser
            .parse("next week", date(2026, 2, 18))
            .expect("expected parse");
        // Monday of next week = 2026-02-23
        assert_eq!(parsed.datetime, datetime(2026, 2, 23, 0, 0));
    }

    #[test]
    fn next_week_from_sunday_resolves_to_next_monday() {
        let parser = BasicDateParser::default();
        // 2026-02-22 is a Sunday
        let parsed = parser
            .parse("next week", date(2026, 2, 22))
            .expect("expected parse");
        // Monday of next week = 2026-02-23
        assert_eq!(parsed.datetime, datetime(2026, 2, 23, 0, 0));
    }

    #[test]
    fn last_week_resolves_to_monday_of_previous_week() {
        let parser = BasicDateParser::default();
        // 2026-02-18 is a Wednesday
        let parsed = parser
            .parse("last week", date(2026, 2, 18))
            .expect("expected parse");
        // Monday of previous week = 2026-02-09
        assert_eq!(parsed.datetime, datetime(2026, 2, 9, 0, 0));
    }

    #[test]
    fn next_month_resolves_to_first_of_following_month() {
        let parser = BasicDateParser::default();
        let parsed = parser
            .parse("next month", date(2026, 2, 18))
            .expect("expected parse");
        assert_eq!(parsed.datetime, datetime(2026, 3, 1, 0, 0));
    }

    #[test]
    fn next_month_from_december_wraps_to_january() {
        let parser = BasicDateParser::default();
        let parsed = parser
            .parse("next month", date(2026, 12, 15))
            .expect("expected parse");
        assert_eq!(parsed.datetime, datetime(2027, 1, 1, 0, 0));
    }

    #[test]
    fn last_month_resolves_to_first_of_previous_month() {
        let parser = BasicDateParser::default();
        let parsed = parser
            .parse("last month", date(2026, 3, 18))
            .expect("expected parse");
        assert_eq!(parsed.datetime, datetime(2026, 2, 1, 0, 0));
    }

    #[test]
    fn last_month_from_january_wraps_to_december() {
        let parser = BasicDateParser::default();
        let parsed = parser
            .parse("last month", date(2026, 1, 15))
            .expect("expected parse");
        assert_eq!(parsed.datetime, datetime(2025, 12, 1, 0, 0));
    }

    #[test]
    fn end_of_week_resolves_to_friday() {
        let parser = BasicDateParser::default();
        // 2026-02-18 is a Wednesday
        let parsed = parser
            .parse("end of week", date(2026, 2, 18))
            .expect("expected parse");
        assert_eq!(parsed.datetime, datetime(2026, 2, 20, 0, 0));
    }

    #[test]
    fn end_of_week_on_friday_returns_same_day() {
        let parser = BasicDateParser::default();
        let parsed = parser
            .parse("end of week", date(2026, 2, 20))
            .expect("expected parse");
        assert_eq!(parsed.datetime, datetime(2026, 2, 20, 0, 0));
    }

    #[test]
    fn end_of_week_on_saturday_returns_previous_friday() {
        let parser = BasicDateParser::default();
        // 2026-02-21 is Saturday; end of (work) week = Friday = 2026-02-20
        let parsed = parser
            .parse("end of week", date(2026, 2, 21))
            .expect("expected parse");
        assert_eq!(parsed.datetime, datetime(2026, 2, 20, 0, 0));
    }

    #[test]
    fn end_of_month_resolves_to_last_day() {
        let parser = BasicDateParser::default();
        let parsed = parser
            .parse("end of month", date(2026, 2, 18))
            .expect("expected parse");
        assert_eq!(parsed.datetime, datetime(2026, 2, 28, 0, 0));
    }

    #[test]
    fn end_of_month_in_leap_year_february() {
        let parser = BasicDateParser::default();
        let parsed = parser
            .parse("end of month", date(2024, 2, 10))
            .expect("expected parse");
        assert_eq!(parsed.datetime, datetime(2024, 2, 29, 0, 0));
    }

    #[test]
    fn end_of_month_in_december() {
        let parser = BasicDateParser::default();
        let parsed = parser
            .parse("end of month", date(2026, 12, 5))
            .expect("expected parse");
        assert_eq!(parsed.datetime, datetime(2026, 12, 31, 0, 0));
    }

    // ── "in N <unit>" phrases ────────────────────────────────────────────────

    #[test]
    fn in_3_days_resolves() {
        let parser = BasicDateParser::default();
        let parsed = parser
            .parse("in 3 days", date(2026, 2, 18))
            .expect("expected parse");
        assert_eq!(parsed.datetime, datetime(2026, 2, 21, 0, 0));
    }

    #[test]
    fn in_1_day_singular() {
        let parser = BasicDateParser::default();
        let parsed = parser
            .parse("in 1 day", date(2026, 2, 18))
            .expect("expected parse");
        assert_eq!(parsed.datetime, datetime(2026, 2, 19, 0, 0));
    }

    #[test]
    fn in_2_weeks_resolves() {
        let parser = BasicDateParser::default();
        let parsed = parser
            .parse("in 2 weeks", date(2026, 2, 18))
            .expect("expected parse");
        assert_eq!(parsed.datetime, datetime(2026, 3, 4, 0, 0));
    }

    #[test]
    fn in_1_week_singular() {
        let parser = BasicDateParser::default();
        let parsed = parser
            .parse("in 1 week", date(2026, 2, 18))
            .expect("expected parse");
        assert_eq!(parsed.datetime, datetime(2026, 2, 25, 0, 0));
    }

    #[test]
    fn in_3_months_resolves() {
        let parser = BasicDateParser::default();
        let parsed = parser
            .parse("in 3 months", date(2026, 2, 18))
            .expect("expected parse");
        assert_eq!(parsed.datetime, datetime(2026, 5, 18, 0, 0));
    }

    #[test]
    fn in_1_month_singular() {
        let parser = BasicDateParser::default();
        let parsed = parser
            .parse("in 1 month", date(2026, 2, 18))
            .expect("expected parse");
        assert_eq!(parsed.datetime, datetime(2026, 3, 18, 0, 0));
    }

    #[test]
    fn in_n_days_crossing_year_boundary() {
        let parser = BasicDateParser::default();
        let parsed = parser
            .parse("in 5 days", date(2026, 12, 29))
            .expect("expected parse");
        assert_eq!(parsed.datetime, datetime(2027, 1, 3, 0, 0));
    }

    #[test]
    fn in_n_phrases_are_case_insensitive() {
        let parser = BasicDateParser::default();
        let parsed = parser
            .parse("IN 3 DAYS", date(2026, 2, 18))
            .expect("expected parse");
        assert_eq!(parsed.datetime, datetime(2026, 2, 21, 0, 0));
    }

    #[test]
    fn relative_period_phrases_are_case_insensitive() {
        let parser = BasicDateParser::default();
        let parsed = parser
            .parse("NEXT WEEK", date(2026, 2, 18))
            .expect("expected parse");
        assert_eq!(parsed.datetime, datetime(2026, 2, 23, 0, 0));
    }

    #[test]
    fn relative_period_phrases_support_trailing_time() {
        let parser = BasicDateParser::default();
        let parsed = parser
            .parse("next week at 9am", date(2026, 2, 18))
            .expect("expected parse");
        assert_eq!(parsed.datetime, datetime(2026, 2, 23, 9, 0));
    }

    #[test]
    fn in_n_phrases_support_trailing_time() {
        let parser = BasicDateParser::default();
        let parsed = parser
            .parse("in 3 days at 2pm", date(2026, 2, 18))
            .expect("expected parse");
        assert_eq!(parsed.datetime, datetime(2026, 2, 21, 14, 0));
    }

    #[test]
    fn relative_period_boundary_prevents_false_positives() {
        let parser = BasicDateParser::default();
        assert_eq!(parser.parse("nextweek", date(2026, 2, 18)), None);
        assert_eq!(parser.parse("lastmonthly", date(2026, 2, 18)), None);
    }

    #[test]
    fn in_n_boundary_prevents_false_positives() {
        let parser = BasicDateParser::default();
        assert_eq!(parser.parse("sin 3 days", date(2026, 2, 18)), None);
    }

    // ── Bare weekday (no prefix) ─────────────────────────────────────────────

    #[test]
    fn bare_weekday_resolves_like_this_weekday() {
        let parser = BasicDateParser::default();
        // 2026-02-18 is a Wednesday
        let parsed = parser
            .parse("friday", date(2026, 2, 18))
            .expect("expected parse");
        // "this friday" from Wednesday = 2 days later
        assert_eq!(parsed.datetime, datetime(2026, 2, 20, 0, 0));
    }

    #[test]
    fn bare_weekday_same_day_advances_one_week() {
        let parser = BasicDateParser::default();
        // 2026-02-18 is a Wednesday; bare "wednesday" on Wednesday = next Wednesday
        let parsed = parser
            .parse("wednesday", date(2026, 2, 18))
            .expect("expected parse");
        assert_eq!(parsed.datetime, datetime(2026, 2, 25, 0, 0));
    }

    #[test]
    fn bare_weekday_supports_trailing_time() {
        let parser = BasicDateParser::default();
        // 2026-02-18 is a Wednesday
        let parsed = parser
            .parse("tuesday at 9am", date(2026, 2, 18))
            .expect("expected parse");
        // "this tuesday" from Wednesday wraps to next week = 2026-02-24
        assert_eq!(parsed.datetime, datetime(2026, 2, 24, 9, 0));
    }

    #[test]
    fn bare_weekday_is_case_insensitive() {
        let parser = BasicDateParser::default();
        let parsed = parser
            .parse("FRIDAY", date(2026, 2, 18))
            .expect("expected parse");
        assert_eq!(parsed.datetime, datetime(2026, 2, 20, 0, 0));
    }

    #[test]
    fn bare_weekday_boundary_prevents_false_positives() {
        let parser = BasicDateParser::default();
        assert_eq!(parser.parse("sundayish", date(2026, 2, 18)), None);
    }

    // ── Next year ────────────────────────────────────────────────────────────

    #[test]
    fn next_year_resolves_to_jan_1() {
        let parser = BasicDateParser::default();
        let parsed = parser
            .parse("next year", date(2026, 7, 15))
            .expect("expected parse");
        assert_eq!(parsed.datetime, datetime(2027, 1, 1, 0, 0));
    }

    #[test]
    fn next_year_from_december_31() {
        let parser = BasicDateParser::default();
        let parsed = parser
            .parse("next year", date(2026, 12, 31))
            .expect("expected parse");
        assert_eq!(parsed.datetime, datetime(2027, 1, 1, 0, 0));
    }

    // ── Edge case coverage for identified gotchas ────────────────────────────

    #[test]
    fn bare_monday_works() {
        let parser = BasicDateParser::default();
        // 2026-02-18 is a Wednesday; bare "monday" = this Monday = wraps to next week
        let parsed = parser
            .parse("monday", date(2026, 2, 18))
            .expect("bare monday should parse");
        // days_until_weekday_this(Wed=2, Mon=0) = (0-2+7)%7 = 5 → 2026-02-23
        assert_eq!(parsed.datetime, datetime(2026, 2, 23, 0, 0));
    }

    #[test]
    fn bare_monday_at_time_works() {
        let parser = BasicDateParser::default();
        let parsed = parser
            .parse("monday at 10am", date(2026, 2, 18))
            .expect("bare monday with time should parse");
        assert_eq!(parsed.datetime, datetime(2026, 2, 23, 10, 0));
    }

    #[test]
    fn last_tuesday_does_not_parse_as_relative_weekday() {
        // "last <weekday>" is NOT supported — only "last week" and "last month".
        // The bare weekday scanner should still pick up "tuesday" though.
        let parser = BasicDateParser::default();
        // 2026-02-18 is a Wednesday
        let parsed = parser
            .parse("last tuesday", date(2026, 2, 18))
            .expect("bare tuesday within 'last tuesday' should parse");
        // Bare "tuesday" from Wednesday = (1-2+7)%7 = 6 → next Tuesday 2026-02-24
        assert_eq!(parsed.datetime, datetime(2026, 2, 24, 0, 0));
        // The span should cover only "tuesday", not "last"
        assert_eq!(&"last tuesday"[parsed.span.0..parsed.span.1], "tuesday");
    }

    #[test]
    fn in_0_days_resolves_to_today() {
        let parser = BasicDateParser::default();
        let parsed = parser
            .parse("in 0 days", date(2026, 2, 18))
            .expect("in 0 days should parse");
        assert_eq!(parsed.datetime, datetime(2026, 2, 18, 0, 0));
    }

    #[test]
    fn end_of_week_on_sunday_returns_previous_friday() {
        let parser = BasicDateParser::default();
        // 2026-02-22 is a Sunday (to_monday_zero_offset = 6)
        // Friday offset = 4 - 6 = -2 → 2026-02-20 (previous Friday)
        let parsed = parser
            .parse("end of week", date(2026, 2, 22))
            .expect("end of week on Sunday should parse");
        assert_eq!(parsed.datetime, datetime(2026, 2, 20, 0, 0));
    }

    #[test]
    fn end_of_week_on_monday_returns_same_week_friday() {
        let parser = BasicDateParser::default();
        // 2026-02-16 is a Monday
        let parsed = parser
            .parse("end of week", date(2026, 2, 16))
            .expect("end of week on Monday should parse");
        assert_eq!(parsed.datetime, datetime(2026, 2, 20, 0, 0));
    }

    #[test]
    fn all_seven_bare_weekdays_parse() {
        let parser = BasicDateParser::default();
        let reference = date(2026, 2, 16); // Monday
        for (name, _) in super::WEEKDAYS {
            assert!(
                parser.parse(name, reference).is_some(),
                "bare '{name}' should parse"
            );
        }
    }

    // --- Recurrence parsing tests ---

    use super::{DateParseResult, RecurrenceFrequency, RecurrenceRule};

    fn assert_recurring(
        result: Option<DateParseResult>,
        expected_freq: RecurrenceFrequency,
        expected_interval: u16,
        expected_dt: DateTime,
    ) -> RecurrenceRule {
        let result = result.expect("expected recurrence parse");
        match result {
            DateParseResult::Recurring { first_date, rule } => {
                assert_eq!(first_date.datetime, expected_dt);
                assert_eq!(rule.frequency, expected_freq);
                assert_eq!(rule.interval, expected_interval);
                rule
            }
            DateParseResult::OneTime(_) => panic!("expected Recurring, got OneTime"),
        }
    }

    #[test]
    fn recurrence_every_monday() {
        let parser = BasicDateParser::default();
        // 2026-04-01 is a Wednesday
        let result = parser.parse_with_recurrence("every Monday", date(2026, 4, 1));
        let rule = assert_recurring(
            result,
            RecurrenceFrequency::Weekly,
            1,
            datetime(2026, 4, 6, 0, 0), // next Monday
        );
        assert_eq!(rule.weekday, Some(1)); // 1 = Monday
    }

    #[test]
    fn recurrence_every_friday_case_insensitive() {
        let parser = BasicDateParser::default();
        let result = parser.parse_with_recurrence("every FRIDAY", date(2026, 4, 1));
        let rule = assert_recurring(
            result,
            RecurrenceFrequency::Weekly,
            1,
            datetime(2026, 4, 3, 0, 0), // next Friday
        );
        assert_eq!(rule.weekday, Some(5)); // 5 = Friday
    }

    #[test]
    fn recurrence_daily() {
        let parser = BasicDateParser::default();
        let result = parser.parse_with_recurrence("daily", date(2026, 4, 1));
        assert_recurring(
            result,
            RecurrenceFrequency::Daily,
            1,
            datetime(2026, 4, 2, 0, 0),
        );
    }

    #[test]
    fn recurrence_weekly() {
        let parser = BasicDateParser::default();
        let result = parser.parse_with_recurrence("weekly", date(2026, 4, 1));
        assert_recurring(
            result,
            RecurrenceFrequency::Weekly,
            1,
            datetime(2026, 4, 8, 0, 0),
        );
    }

    #[test]
    fn recurrence_monthly() {
        let parser = BasicDateParser::default();
        let result = parser.parse_with_recurrence("monthly", date(2026, 4, 1));
        assert_recurring(
            result,
            RecurrenceFrequency::Monthly,
            1,
            datetime(2026, 5, 1, 0, 0),
        );
    }

    #[test]
    fn recurrence_yearly() {
        let parser = BasicDateParser::default();
        let result = parser.parse_with_recurrence("yearly", date(2026, 4, 1));
        assert_recurring(
            result,
            RecurrenceFrequency::Yearly,
            1,
            datetime(2027, 4, 1, 0, 0),
        );
    }

    #[test]
    fn recurrence_every_2_weeks() {
        let parser = BasicDateParser::default();
        let result = parser.parse_with_recurrence("every 2 weeks", date(2026, 4, 1));
        assert_recurring(
            result,
            RecurrenceFrequency::Weekly,
            2,
            datetime(2026, 4, 15, 0, 0),
        );
    }

    #[test]
    fn recurrence_every_3_months() {
        let parser = BasicDateParser::default();
        let result = parser.parse_with_recurrence("every 3 months", date(2026, 4, 1));
        assert_recurring(
            result,
            RecurrenceFrequency::Monthly,
            3,
            datetime(2026, 7, 1, 0, 0),
        );
    }

    #[test]
    fn recurrence_monthly_on_the_15th() {
        let parser = BasicDateParser::default();
        let result = parser.parse_with_recurrence("monthly on the 15th", date(2026, 4, 1));
        let rule = assert_recurring(
            result,
            RecurrenceFrequency::Monthly,
            1,
            datetime(2026, 4, 15, 0, 0),
        );
        assert_eq!(rule.day_of_month, Some(15));
    }

    #[test]
    fn recurrence_monthly_on_the_1st_past_day() {
        let parser = BasicDateParser::default();
        // Reference is April 10, day 1 already passed → next month
        let result = parser.parse_with_recurrence("monthly on the 1st", date(2026, 4, 10));
        let rule = assert_recurring(
            result,
            RecurrenceFrequency::Monthly,
            1,
            datetime(2026, 5, 1, 0, 0),
        );
        assert_eq!(rule.day_of_month, Some(1));
    }

    #[test]
    fn recurrence_embedded_in_text() {
        let parser = BasicDateParser::default();
        let result =
            parser.parse_with_recurrence("pay rent every month on the 1st", date(2026, 4, 10));
        assert!(result.is_some());
        match result.unwrap() {
            DateParseResult::Recurring { rule, .. } => {
                assert_eq!(rule.frequency, RecurrenceFrequency::Monthly);
                assert_eq!(rule.day_of_month, Some(1));
            }
            _ => panic!("expected Recurring"),
        }
    }

    #[test]
    fn non_recurrence_falls_through_to_onetime() {
        let parser = BasicDateParser::default();
        let result = parser.parse_with_recurrence("next friday", date(2026, 4, 1));
        match result.expect("should parse") {
            DateParseResult::OneTime(_) => {}
            DateParseResult::Recurring { .. } => panic!("expected OneTime"),
        }
    }

    #[test]
    fn no_false_positive_on_every_without_unit() {
        let parser = BasicDateParser::default();
        // "every" alone should not match
        let result = parser.parse_with_recurrence("review every student", date(2026, 4, 1));
        // Should not produce a Recurring result (may produce OneTime or None)
        if let Some(DateParseResult::Recurring { .. }) = result {
            panic!("unexpected Recurring");
        }
    }

    #[test]
    fn recurrence_every_weekday_at_time() {
        let parser = BasicDateParser::default();
        // 2026-04-01 is a Wednesday → next Saturday = 2026-04-04
        let rule = assert_recurring(
            parser.parse_with_recurrence("every saturday at 6pm", date(2026, 4, 1)),
            RecurrenceFrequency::Weekly,
            1,
            datetime(2026, 4, 4, 18, 0),
        );
        assert_eq!(rule.weekday, Some(6)); // 6 = Saturday
    }

    #[test]
    fn recurrence_daily_at_time() {
        let parser = BasicDateParser::default();
        assert_recurring(
            parser.parse_with_recurrence("daily at 9am", date(2026, 4, 1)),
            RecurrenceFrequency::Daily,
            1,
            datetime(2026, 4, 2, 9, 0),
        );
    }

    #[test]
    fn recurrence_every_n_unit_at_time() {
        let parser = BasicDateParser::default();
        assert_recurring(
            parser.parse_with_recurrence("every 2 weeks at 10:30am", date(2026, 4, 1)),
            RecurrenceFrequency::Weekly,
            2,
            datetime(2026, 4, 15, 10, 30),
        );
    }

    // --- Slice 1: word aliases ---

    #[test]
    fn recurrence_quarterly() {
        let parser = BasicDateParser::default();
        // 2026-04-01 → next quarter = Jul 1
        let rule = assert_recurring(
            parser.parse_with_recurrence("quarterly", date(2026, 4, 1)),
            RecurrenceFrequency::Monthly,
            3,
            datetime(2026, 7, 1, 0, 0),
        );
        assert_eq!(rule.day_of_month, Some(1));
    }

    #[test]
    fn recurrence_quarterly_in_december() {
        let parser = BasicDateParser::default();
        // Dec → next quarter = Jan 1 next year
        let rule = assert_recurring(
            parser.parse_with_recurrence("quarterly", date(2026, 12, 15)),
            RecurrenceFrequency::Monthly,
            3,
            datetime(2027, 1, 1, 0, 0),
        );
        assert_eq!(rule.day_of_month, Some(1));
    }

    #[test]
    fn recurrence_quarterly_display() {
        let rule = RecurrenceRule {
            frequency: RecurrenceFrequency::Monthly,
            interval: 3,
            weekday: None,
            day_of_month: Some(1),
            month: None,
            weekdays_only: None,
        };
        assert_eq!(rule.display(), "quarterly");
    }

    #[test]
    fn recurrence_biweekly() {
        let parser = BasicDateParser::default();
        assert_recurring(
            parser.parse_with_recurrence("biweekly", date(2026, 4, 1)),
            RecurrenceFrequency::Weekly,
            2,
            datetime(2026, 4, 15, 0, 0),
        );
    }

    #[test]
    fn recurrence_bimonthly() {
        let parser = BasicDateParser::default();
        assert_recurring(
            parser.parse_with_recurrence("bimonthly", date(2026, 4, 1)),
            RecurrenceFrequency::Monthly,
            2,
            datetime(2026, 6, 1, 0, 0),
        );
    }

    // --- Slice 2: inverted day-of-month order ---

    #[test]
    fn recurrence_1st_of_every_month() {
        let parser = BasicDateParser::default();
        // Apr 10 → past 1st, so May 1
        let rule = assert_recurring(
            parser.parse_with_recurrence("1st of every month", date(2026, 4, 10)),
            RecurrenceFrequency::Monthly,
            1,
            datetime(2026, 5, 1, 0, 0),
        );
        assert_eq!(rule.day_of_month, Some(1));
    }

    #[test]
    fn recurrence_13th_of_every_month() {
        let parser = BasicDateParser::default();
        // Apr 10 → 13th is ahead, so Apr 13
        let rule = assert_recurring(
            parser.parse_with_recurrence("13th of every month", date(2026, 4, 10)),
            RecurrenceFrequency::Monthly,
            1,
            datetime(2026, 4, 13, 0, 0),
        );
        assert_eq!(rule.day_of_month, Some(13));
    }

    #[test]
    fn recurrence_15th_of_each_month() {
        let parser = BasicDateParser::default();
        assert_recurring(
            parser.parse_with_recurrence("15th of each month", date(2026, 4, 1)),
            RecurrenceFrequency::Monthly,
            1,
            datetime(2026, 4, 15, 0, 0),
        );
    }

    #[test]
    fn recurrence_3rd_of_every_month() {
        let parser = BasicDateParser::default();
        // Apr 10 → past 3rd, so May 3
        assert_recurring(
            parser.parse_with_recurrence("3rd of every month", date(2026, 4, 10)),
            RecurrenceFrequency::Monthly,
            1,
            datetime(2026, 5, 3, 0, 0),
        );
    }

    // --- Slice 3: every other ---

    #[test]
    fn recurrence_every_other_tuesday() {
        let parser = BasicDateParser::default();
        // 2026-04-01 is a Wednesday → next Tuesday = Apr 7
        let rule = assert_recurring(
            parser.parse_with_recurrence("every other tuesday", date(2026, 4, 1)),
            RecurrenceFrequency::Weekly,
            2,
            datetime(2026, 4, 7, 0, 0),
        );
        assert_eq!(rule.weekday, Some(2)); // Tuesday
    }

    #[test]
    fn recurrence_every_other_friday() {
        let parser = BasicDateParser::default();
        // 2026-04-01 is a Wednesday → next Friday = Apr 3
        let rule = assert_recurring(
            parser.parse_with_recurrence("every other friday", date(2026, 4, 1)),
            RecurrenceFrequency::Weekly,
            2,
            datetime(2026, 4, 3, 0, 0),
        );
        assert_eq!(rule.weekday, Some(5)); // Friday
    }

    // --- Slice 5: yearly dates ---

    #[test]
    fn recurrence_every_january_1() {
        let parser = BasicDateParser::default();
        // Apr 2026 → past Jan 1, so Jan 1 2027
        let rule = assert_recurring(
            parser.parse_with_recurrence("every january 1", date(2026, 4, 1)),
            RecurrenceFrequency::Yearly,
            1,
            datetime(2027, 1, 1, 0, 0),
        );
        assert_eq!(rule.month, Some(1));
        assert_eq!(rule.day_of_month, Some(1));
    }

    #[test]
    fn recurrence_every_mar_15() {
        let parser = BasicDateParser::default();
        // Feb 2026 → Mar 15 is ahead, so this year
        let rule = assert_recurring(
            parser.parse_with_recurrence("every mar 15", date(2026, 2, 1)),
            RecurrenceFrequency::Yearly,
            1,
            datetime(2026, 3, 15, 0, 0),
        );
        assert_eq!(rule.month, Some(3));
        assert_eq!(rule.day_of_month, Some(15));
    }

    #[test]
    fn recurrence_every_march_15th() {
        let parser = BasicDateParser::default();
        let rule = assert_recurring(
            parser.parse_with_recurrence("every march 15th", date(2026, 2, 1)),
            RecurrenceFrequency::Yearly,
            1,
            datetime(2026, 3, 15, 0, 0),
        );
        assert_eq!(rule.month, Some(3));
        assert_eq!(rule.day_of_month, Some(15));
    }

    #[test]
    fn recurrence_every_december_25() {
        let parser = BasicDateParser::default();
        // Nov → Dec 25 is ahead, so this year
        assert_recurring(
            parser.parse_with_recurrence("every december 25", date(2026, 11, 1)),
            RecurrenceFrequency::Yearly,
            1,
            datetime(2026, 12, 25, 0, 0),
        );
    }

    #[test]
    fn recurrence_yearly_date_display() {
        let rule = RecurrenceRule {
            frequency: RecurrenceFrequency::Yearly,
            interval: 1,
            weekday: None,
            day_of_month: Some(15),
            month: Some(3),
            weekdays_only: None,
        };
        assert_eq!(rule.display(), "every March 15");
    }

    // --- Slice 6: starting anchor ---

    #[test]
    fn recurrence_every_3_months_starting_jan_1() {
        let parser = BasicDateParser::default();
        // From Apr 1: "starting jan 1" → past, so next Jan 1 2027
        // But the recurrence first_date should be the starting anchor
        let result =
            parser.parse_with_recurrence("every 3 months starting january 1", date(2026, 4, 1));
        let rule = assert_recurring(
            result,
            RecurrenceFrequency::Monthly,
            3,
            datetime(2027, 1, 1, 0, 0),
        );
        assert!(rule.weekday.is_none());
    }

    #[test]
    fn recurrence_weekly_starting_march_1() {
        let parser = BasicDateParser::default();
        // From Feb 1: "starting march 1" → Mar 1 is a Sunday
        // Weekly rule, so first_date = Mar 1
        assert_recurring(
            parser.parse_with_recurrence("weekly starting march 1", date(2026, 2, 1)),
            RecurrenceFrequency::Weekly,
            1,
            datetime(2026, 3, 1, 0, 0),
        );
    }

    #[test]
    fn recurrence_every_monday_starting_april_15() {
        let parser = BasicDateParser::default();
        // Apr 15 2026 is a Wednesday → first Monday on/after = Apr 20
        let rule = assert_recurring(
            parser.parse_with_recurrence("every monday starting april 15", date(2026, 4, 1)),
            RecurrenceFrequency::Weekly,
            1,
            datetime(2026, 4, 20, 0, 0),
        );
        assert_eq!(rule.weekday, Some(1)); // Monday
    }

    #[test]
    fn recurrence_biweekly_starting_next_monday() {
        let parser = BasicDateParser::default();
        // 2026-04-01 is Wednesday → next Monday = Apr 6
        assert_recurring(
            parser.parse_with_recurrence("biweekly starting next monday", date(2026, 4, 1)),
            RecurrenceFrequency::Weekly,
            2,
            datetime(2026, 4, 6, 0, 0),
        );
    }

    // --- every weekday / business day ---

    #[test]
    fn recurrence_every_weekday() {
        let parser = BasicDateParser::default();
        // 2026-04-01 is Wednesday → next weekday = Thu Apr 2
        let rule = assert_recurring(
            parser.parse_with_recurrence("every weekday", date(2026, 4, 1)),
            RecurrenceFrequency::Daily,
            1,
            datetime(2026, 4, 2, 0, 0),
        );
        assert_eq!(rule.weekdays_only, Some(true));
    }

    #[test]
    fn recurrence_every_business_day() {
        let parser = BasicDateParser::default();
        // Same as "every weekday"
        let rule = assert_recurring(
            parser.parse_with_recurrence("every business day", date(2026, 4, 1)),
            RecurrenceFrequency::Daily,
            1,
            datetime(2026, 4, 2, 0, 0),
        );
        assert_eq!(rule.weekdays_only, Some(true));
    }

    #[test]
    fn recurrence_every_weekday_from_friday_skips_weekend() {
        let parser = BasicDateParser::default();
        // 2026-04-03 is Friday → next weekday = Mon Apr 6
        let rule = assert_recurring(
            parser.parse_with_recurrence("every weekday", date(2026, 4, 3)),
            RecurrenceFrequency::Daily,
            1,
            datetime(2026, 4, 6, 0, 0),
        );
        assert_eq!(rule.weekdays_only, Some(true));
    }

    #[test]
    fn recurrence_every_weekday_from_saturday_skips_weekend() {
        let parser = BasicDateParser::default();
        // 2026-04-04 is Saturday → next weekday = Mon Apr 6
        let rule = assert_recurring(
            parser.parse_with_recurrence("every weekday", date(2026, 4, 4)),
            RecurrenceFrequency::Daily,
            1,
            datetime(2026, 4, 6, 0, 0),
        );
        assert_eq!(rule.weekdays_only, Some(true));
    }

    #[test]
    fn recurrence_every_weekday_with_time() {
        let parser = BasicDateParser::default();
        // 2026-04-01 is Wednesday → next weekday = Thu Apr 2 at 9am
        let rule = assert_recurring(
            parser.parse_with_recurrence("every weekday at 9am", date(2026, 4, 1)),
            RecurrenceFrequency::Daily,
            1,
            datetime(2026, 4, 2, 9, 0),
        );
        assert_eq!(rule.weekdays_only, Some(true));
    }

    #[test]
    fn recurrence_every_weekday_display() {
        let rule = RecurrenceRule {
            frequency: RecurrenceFrequency::Daily,
            interval: 1,
            weekday: None,
            day_of_month: None,
            month: None,
            weekdays_only: Some(true),
        };
        assert_eq!(rule.display(), "every weekday");
    }

    #[test]
    fn recurrence_weekday_next_date_friday_to_monday() {
        // Friday → next weekday should be Monday
        let rule = RecurrenceRule {
            frequency: RecurrenceFrequency::Daily,
            interval: 1,
            weekday: None,
            day_of_month: None,
            month: None,
            weekdays_only: Some(true),
        };
        // 2026-04-03 is Friday
        let anchor = datetime(2026, 4, 3, 9, 0);
        let next = rule.next_date(anchor);
        assert_eq!(next, datetime(2026, 4, 6, 9, 0)); // Monday
    }

    #[test]
    fn recurrence_weekday_next_date_thursday_to_friday() {
        let rule = RecurrenceRule {
            frequency: RecurrenceFrequency::Daily,
            interval: 1,
            weekday: None,
            day_of_month: None,
            month: None,
            weekdays_only: Some(true),
        };
        // 2026-04-02 is Thursday
        let anchor = datetime(2026, 4, 2, 9, 0);
        let next = rule.next_date(anchor);
        assert_eq!(next, datetime(2026, 4, 3, 9, 0)); // Friday
    }

    #[test]
    fn recurrence_weekday_next_date_saturday_to_monday() {
        // If somehow anchored on Saturday, next weekday should be Monday
        let rule = RecurrenceRule {
            frequency: RecurrenceFrequency::Daily,
            interval: 1,
            weekday: None,
            day_of_month: None,
            month: None,
            weekdays_only: Some(true),
        };
        // 2026-04-04 is Saturday
        let anchor = datetime(2026, 4, 4, 9, 0);
        let next = rule.next_date(anchor);
        assert_eq!(next, datetime(2026, 4, 6, 9, 0)); // Monday
    }
}
