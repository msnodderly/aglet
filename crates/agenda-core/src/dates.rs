use chrono::{NaiveDate, NaiveDateTime};

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

#[cfg(test)]
mod tests {
    use super::{DateParser, ParsedDate};
    use chrono::{Datelike, NaiveDate, NaiveDateTime};

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).expect("valid date")
    }

    fn datetime(y: i32, m: u32, d: u32, h: u32, min: u32) -> NaiveDateTime {
        date(y, m, d).and_hms_opt(h, min, 0).expect("valid time")
    }

    struct NoMatchParser;

    impl DateParser for NoMatchParser {
        fn parse(&self, _text: &str, _reference_date: NaiveDate) -> Option<ParsedDate> {
            None
        }
    }

    #[test]
    fn no_match_contract_returns_none() {
        let parser = NoMatchParser;
        let result = parser.parse("review docs sometime", date(2026, 2, 16));
        assert_eq!(result, None);
    }

    struct FixedParser;

    impl DateParser for FixedParser {
        fn parse(&self, _text: &str, _reference_date: NaiveDate) -> Option<ParsedDate> {
            Some(ParsedDate {
                datetime: datetime(2026, 5, 25, 15, 0),
                span: (5, 17),
            })
        }
    }

    #[test]
    fn successful_parse_shape_keeps_datetime_and_span() {
        let parser = FixedParser;
        let parsed = parser
            .parse("meet May 25, 2026 at 3pm", date(2026, 2, 16))
            .expect("expected parse");

        assert_eq!(parsed.datetime, datetime(2026, 5, 25, 15, 0));
        assert_eq!(parsed.span, (5, 17));
    }

    struct ReferenceAwareParser;

    impl DateParser for ReferenceAwareParser {
        fn parse(&self, _text: &str, reference_date: NaiveDate) -> Option<ParsedDate> {
            let resolved = if reference_date.year() >= 2026 {
                datetime(2026, 1, 2, 0, 0)
            } else {
                datetime(2025, 1, 2, 0, 0)
            };

            Some(ParsedDate {
                datetime: resolved,
                span: (0, 4),
            })
        }
    }

    #[test]
    fn reference_date_is_part_of_trait_contract() {
        let parser = ReferenceAwareParser;

        let older = parser
            .parse("next day", date(2025, 12, 31))
            .expect("expected parse");
        let newer = parser
            .parse("next day", date(2026, 1, 1))
            .expect("expected parse");

        assert_ne!(older.datetime, newer.datetime);
        assert_eq!(older.datetime, datetime(2025, 1, 2, 0, 0));
        assert_eq!(newer.datetime, datetime(2026, 1, 2, 0, 0));
    }

    struct SpanParser;

    impl DateParser for SpanParser {
        fn parse(&self, text: &str, _reference_date: NaiveDate) -> Option<ParsedDate> {
            let matched = "tomorrow";
            let start = text.find(matched)?;
            let end = start + matched.len();

            Some(ParsedDate {
                datetime: datetime(2026, 2, 17, 0, 0),
                span: (start, end),
            })
        }
    }

    #[test]
    fn span_round_trip_is_exact_tuple() {
        let parser = SpanParser;
        let text = "Call Sarah tomorrow";

        let parsed = parser
            .parse(text, date(2026, 2, 16))
            .expect("expected parse");

        assert_eq!(parsed.span, (11, 19));
        assert_eq!(&text[parsed.span.0..parsed.span.1], "tomorrow");
    }
}
