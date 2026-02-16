use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, Weekday};

/// Parses date/time expressions from item text.
pub trait DateParser: Send + Sync {
    /// Extract a date/time from item text.
    ///
    /// Returns `None` when no supported date expression is found.
    /// Returns `Some(ParsedDate)` when an expression is found and resolved
    /// against `reference_date`.
    fn parse(&self, text: &str, reference_date: NaiveDate) -> Option<ParsedDate>;
}

/// Parsed date/time data and source provenance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParsedDate {
    /// Absolute local datetime resolved during parsing.
    pub datetime: NaiveDateTime,
    /// Matched source span as UTF-8 byte offsets in `text`, half-open: `[start, end)`.
    ///
    /// When valid, `&text[start..end]` yields the matched expression.
    pub span: (usize, usize),
}

/// Deterministic MVP parser for absolute date expressions.
#[derive(Debug, Default, Clone, Copy)]
pub struct BasicDateParser;

impl DateParser for BasicDateParser {
    fn parse(&self, text: &str, reference_date: NaiveDate) -> Option<ParsedDate> {
        let bytes = text.as_bytes();
        let mut best = None;

        scan_relative_dates(bytes, reference_date, &mut best);
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

const WEEKDAYS: [(&str, Weekday); 7] = [
    ("monday", Weekday::Mon),
    ("tuesday", Weekday::Tue),
    ("wednesday", Weekday::Wed),
    ("thursday", Weekday::Thu),
    ("friday", Weekday::Fri),
    ("saturday", Weekday::Sat),
    ("sunday", Weekday::Sun),
];

fn scan_relative_dates(bytes: &[u8], reference_date: NaiveDate, best: &mut Option<ParsedDate>) {
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

            if let Some(date) = reference_date.checked_add_signed(Duration::days(day_offset)) {
                choose_best(
                    best,
                    ParsedDate {
                        datetime: at_midnight(date),
                        span: (start, end),
                    },
                );
            }
        }

        if let Some(candidate) = parse_relative_weekday(bytes, start, reference_date, "this", false)
        {
            choose_best(best, candidate);
        }

        if let Some(candidate) = parse_relative_weekday(bytes, start, reference_date, "next", true)
        {
            choose_best(best, candidate);
        }
    }
}

fn parse_relative_weekday(
    bytes: &[u8],
    start: usize,
    reference_date: NaiveDate,
    prefix: &str,
    strictly_after: bool,
) -> Option<ParsedDate> {
    if !matches_ascii_insensitive(bytes, start, prefix.as_bytes()) {
        return None;
    }

    let mut pos = start + prefix.len();
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

        let day_delta = days_until_weekday(reference_date.weekday(), weekday, strictly_after);
        let date = reference_date.checked_add_signed(Duration::days(day_delta))?;

        return Some(ParsedDate {
            datetime: at_midnight(date),
            span: (start, end),
        });
    }

    None
}

fn days_until_weekday(current: Weekday, target: Weekday, strictly_after: bool) -> i64 {
    let current_idx = current.num_days_from_monday() as i64;
    let target_idx = target.num_days_from_monday() as i64;
    let mut delta = (target_idx - current_idx + 7) % 7;

    if strictly_after && delta == 0 {
        delta = 7;
    }

    delta
}

fn scan_month_name_dates(bytes: &[u8], reference_date: NaiveDate, best: &mut Option<ParsedDate>) {
    for start in 0..bytes.len() {
        if !has_left_boundary(bytes, start) {
            continue;
        }

        for (name, month) in MONTHS {
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
                        if let Some(date) = NaiveDate::from_ymd_opt(year as i32, month, day) {
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
                if let Some(date) = resolve_month_day_without_year(reference_date, month, day) {
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

        if let Some(date) = NaiveDate::from_ymd_opt(year as i32, month, day) {
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

        if let Some(date) = NaiveDate::from_ymd_opt(year as i32, month, day) {
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

        let full_year = 2000 + year as i32;
        if let Some(date) = NaiveDate::from_ymd_opt(full_year, month, day) {
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

fn resolve_month_day_without_year(
    reference_date: NaiveDate,
    month: u32,
    day: u32,
) -> Option<NaiveDate> {
    let this_year = reference_date.year();
    let this_year_date = NaiveDate::from_ymd_opt(this_year, month, day)?;

    if this_year_date < reference_date {
        NaiveDate::from_ymd_opt(this_year + 1, month, day)
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
    let datetime = date
        .and_hms_opt(time.hour, time.minute, 0)
        .expect("validated time should be valid");

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

fn at_midnight(date: NaiveDate) -> NaiveDateTime {
    date.and_hms_opt(0, 0, 0).expect("midnight time is valid")
}

#[cfg(test)]
mod tests {
    use super::{BasicDateParser, DateParser};
    use chrono::{NaiveDate, NaiveDateTime};

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).expect("valid date")
    }

    fn datetime(y: i32, m: u32, d: u32, h: u32, min: u32) -> NaiveDateTime {
        date(y, m, d).and_hms_opt(h, min, 0).expect("valid time")
    }

    #[test]
    fn month_name_full_date_parses_with_exact_span() {
        let parser = BasicDateParser;
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
        let parser = BasicDateParser;
        let text = "deadline 2026-05-25";

        let parsed = parser
            .parse(text, date(2026, 2, 16))
            .expect("expected parse");

        assert_eq!(parsed.datetime, datetime(2026, 5, 25, 0, 0));
        assert_eq!(&text[parsed.span.0..parsed.span.1], "2026-05-25");
    }

    #[test]
    fn iso_compact_date_parses() {
        let parser = BasicDateParser;
        let text = "deadline 20260525";

        let parsed = parser
            .parse(text, date(2026, 2, 16))
            .expect("expected parse");

        assert_eq!(parsed.datetime, datetime(2026, 5, 25, 0, 0));
        assert_eq!(&text[parsed.span.0..parsed.span.1], "20260525");
    }

    #[test]
    fn numeric_mdy_with_two_digit_year_parses() {
        let parser = BasicDateParser;
        let text = "ship by 12/5/26";

        let parsed = parser
            .parse(text, date(2026, 2, 16))
            .expect("expected parse");

        assert_eq!(parsed.datetime, datetime(2026, 12, 5, 0, 0));
        assert_eq!(&text[parsed.span.0..parsed.span.1], "12/5/26");
    }

    #[test]
    fn month_day_without_year_stays_in_reference_year_when_not_past() {
        let parser = BasicDateParser;

        let parsed = parser
            .parse("December 5", date(2026, 2, 16))
            .expect("expected parse");

        assert_eq!(parsed.datetime, datetime(2026, 12, 5, 0, 0));
    }

    #[test]
    fn month_day_without_year_rolls_forward_if_past() {
        let parser = BasicDateParser;

        let parsed = parser
            .parse("December 5", date(2026, 12, 10))
            .expect("expected parse");

        assert_eq!(parsed.datetime, datetime(2027, 12, 5, 0, 0));
    }

    #[test]
    fn invalid_date_is_rejected() {
        let parser = BasicDateParser;

        assert_eq!(parser.parse("2026-02-30", date(2026, 2, 16)), None);
    }

    #[test]
    fn today_parses_with_exact_span() {
        let parser = BasicDateParser;
        let text = "do this today please";

        let parsed = parser
            .parse(text, date(2026, 2, 16))
            .expect("expected parse");

        assert_eq!(parsed.datetime, datetime(2026, 2, 16, 0, 0));
        assert_eq!(&text[parsed.span.0..parsed.span.1], "today");
    }

    #[test]
    fn tomorrow_and_yesterday_parse_relative_to_reference_date() {
        let parser = BasicDateParser;

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
    fn this_and_next_weekday_semantics_match_spec() {
        let parser = BasicDateParser;

        struct Case {
            text: &'static str,
            reference: NaiveDate,
            expected: NaiveDateTime,
        }

        let cases = [
            Case {
                text: "this Monday",
                reference: date(2026, 2, 16),
                expected: datetime(2026, 2, 16, 0, 0),
            },
            Case {
                text: "this Friday",
                reference: date(2026, 2, 16),
                expected: datetime(2026, 2, 20, 0, 0),
            },
            Case {
                text: "next Monday",
                reference: date(2026, 2, 16),
                expected: datetime(2026, 2, 23, 0, 0),
            },
            Case {
                text: "next Tuesday",
                reference: date(2026, 2, 18),
                expected: datetime(2026, 2, 24, 0, 0),
            },
            Case {
                text: "this Tuesday",
                reference: date(2026, 2, 18),
                expected: datetime(2026, 2, 24, 0, 0),
            },
        ];

        for case in cases {
            let parsed = parser
                .parse(case.text, case.reference)
                .expect("expected parse");
            assert_eq!(parsed.datetime, case.expected, "failed for '{}'", case.text);
            assert_eq!(&case.text[parsed.span.0..parsed.span.1], case.text);
        }
    }

    #[test]
    fn relative_weekday_parsing_is_case_insensitive() {
        let parser = BasicDateParser;

        let parsed = parser
            .parse("NEXT tuesday", date(2026, 2, 16))
            .expect("expected parse");

        assert_eq!(parsed.datetime, datetime(2026, 2, 17, 0, 0));
    }

    #[test]
    fn relative_phrase_boundaries_prevent_false_positives() {
        let parser = BasicDateParser;

        assert_eq!(parser.parse("todayish", date(2026, 2, 16)), None);
        assert_eq!(parser.parse("annext tuesday", date(2026, 2, 16)), None);
    }

    #[test]
    fn compound_with_12_hour_time_parses() {
        let parser = BasicDateParser;
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
        let parser = BasicDateParser;
        let text = "meet May 25, 2026 at 15:00";

        let parsed = parser
            .parse(text, date(2026, 2, 16))
            .expect("expected parse");

        assert_eq!(parsed.datetime, datetime(2026, 5, 25, 15, 0));
        assert_eq!(&text[parsed.span.0..parsed.span.1], "May 25, 2026 at 15:00");
    }

    #[test]
    fn compound_with_noon_parses() {
        let parser = BasicDateParser;
        let text = "today at noon";

        let parsed = parser
            .parse(text, date(2026, 2, 16))
            .expect("expected parse");

        assert_eq!(parsed.datetime, datetime(2026, 2, 16, 12, 0));
        assert_eq!(parsed.span, (0, text.len()));
    }

    #[test]
    fn invalid_compound_time_falls_back_to_date_only() {
        let parser = BasicDateParser;

        let parsed = parser
            .parse("today at 25:00", date(2026, 2, 16))
            .expect("expected parse");

        assert_eq!(parsed.datetime, datetime(2026, 2, 16, 0, 0));
        assert_eq!(parsed.span, (0, 5));
    }

    #[test]
    fn compound_time_parsing_is_case_insensitive() {
        let parser = BasicDateParser;

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
        let parser = BasicDateParser;

        assert_eq!(parser.parse("at 3pm", date(2026, 2, 16)), None);
        assert_eq!(parser.parse("at 15:00", date(2026, 2, 16)), None);
        assert_eq!(parser.parse("at noon", date(2026, 2, 16)), None);
    }

    #[test]
    fn non_date_text_does_not_false_positive() {
        let parser = BasicDateParser;

        assert_eq!(parser.parse("May I ask", date(2026, 2, 16)), None);
    }
}
