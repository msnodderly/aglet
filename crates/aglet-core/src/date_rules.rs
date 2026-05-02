use jiff::civil::{Date, DateTime, Time};
use jiff::tz::TimeZone;
use jiff::{Span, Zoned};

use crate::dates::{BasicDateParser, DateParser};
use crate::model::{DateCompareOp, DateMatcher, DateSource, DateValueExpr, Item};

#[derive(Debug, Clone)]
pub struct EvaluationContext {
    now: Zoned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResolvedValue {
    Date(Date),
    DateTime(DateTime),
}

impl EvaluationContext {
    pub fn now() -> Self {
        Self { now: Zoned::now() }
    }

    pub fn from_zoned(now: Zoned) -> Self {
        Self { now }
    }

    pub fn for_date(today: Date) -> Self {
        let tz = TimeZone::system();
        let now = today
            .to_zoned(tz)
            .expect("midnight should be representable for evaluation date");
        Self { now }
    }

    pub fn now_local(&self) -> DateTime {
        self.now.datetime()
    }

    pub fn today(&self) -> Date {
        self.now.date()
    }

    pub fn timezone(&self) -> TimeZone {
        self.now.time_zone().clone()
    }
}

pub fn category_uses_date_conditions(conditions: &[crate::model::Condition]) -> bool {
    conditions
        .iter()
        .any(|condition| matches!(condition, crate::model::Condition::Date { .. }))
}

pub fn parse_date_value_expr(input: &str) -> Result<DateValueExpr, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("date value cannot be empty".to_string());
    }

    let lower = trimmed.to_ascii_lowercase();
    match lower.as_str() {
        "today" => return Ok(DateValueExpr::Today),
        "tomorrow" => return Ok(DateValueExpr::Tomorrow),
        _ => {}
    }

    if let Some(days) = parse_relative_day_count(&lower, "days from today") {
        return Ok(DateValueExpr::DaysFromToday(days));
    }
    if let Some(days) = parse_relative_day_count(&lower, "days ago") {
        return Ok(DateValueExpr::DaysAgo(days));
    }

    if let Some(prefix) = lower.strip_suffix(" today") {
        let time = parse_time_expr(prefix.trim())?;
        return Ok(DateValueExpr::TimeToday(time));
    }

    if let Ok(datetime) = trimmed.replace(' ', "T").parse::<DateTime>() {
        return Ok(DateValueExpr::AbsoluteDateTime(datetime));
    }
    if let Ok(date) = trimmed.parse::<Date>() {
        return Ok(DateValueExpr::AbsoluteDate(date));
    }

    let parser = BasicDateParser::default();
    if let Some(parsed) = parser.parse(trimmed, Zoned::now().date()) {
        if parsed.span == (0, trimmed.len()) {
            if expression_has_explicit_time(&lower) {
                return Ok(DateValueExpr::AbsoluteDateTime(parsed.datetime));
            }
            return Ok(DateValueExpr::AbsoluteDate(parsed.datetime.date()));
        }
    }

    Err(format!(
        "could not parse date condition value from '{trimmed}'"
    ))
}

pub fn render_date_condition(source: DateSource, matcher: &DateMatcher) -> String {
    match matcher {
        DateMatcher::Compare { op, value } => format!(
            "{} {} {}",
            render_date_source(source),
            render_compare_op(*op),
            render_date_value_expr(value)
        ),
        DateMatcher::Range { from, through } => match (from, through) {
            (DateValueExpr::TimeToday(time), DateValueExpr::Today)
                if *time == default_afternoon_start() =>
            {
                format!("{} this afternoon", render_date_source(source))
            }
            (DateValueExpr::TimeToday(time), DateValueExpr::Today) => format!(
                "{} today, after {}",
                render_date_source(source),
                render_time(*time)
            ),
            (DateValueExpr::Today, DateValueExpr::TimeToday(time)) => format!(
                "{} today, before {}",
                render_date_source(source),
                render_time(*time)
            ),
            _ => format!(
                "{} from {} through {}",
                render_date_source(source),
                render_date_value_expr(from),
                render_date_value_expr(through)
            ),
        },
    }
}

pub fn render_date_source(source: DateSource) -> &'static str {
    match source {
        DateSource::When => "When",
        DateSource::Entry => "Entry",
        DateSource::Done => "Done",
    }
}

pub fn render_compare_op(op: DateCompareOp) -> &'static str {
    match op {
        DateCompareOp::On => "on",
        DateCompareOp::Before => "before",
        DateCompareOp::After => "after",
        DateCompareOp::AtOrBefore => "at or before",
        DateCompareOp::AtOrAfter => "at or after",
    }
}

pub fn render_date_value_expr(value: &DateValueExpr) -> String {
    match value {
        DateValueExpr::Today => "today".to_string(),
        DateValueExpr::Tomorrow => "tomorrow".to_string(),
        DateValueExpr::DaysFromToday(days) => format!("{days} days from today"),
        DateValueExpr::DaysAgo(days) => format!("{days} days ago"),
        DateValueExpr::AbsoluteDate(date) => date.to_string(),
        DateValueExpr::AbsoluteDateTime(datetime) => datetime.to_string().replace('T', " "),
        DateValueExpr::TimeToday(time) => format!("{} today", render_time(*time)),
    }
}

pub fn item_matches_date_condition(
    item: &Item,
    source: DateSource,
    matcher: &DateMatcher,
    ctx: &EvaluationContext,
) -> bool {
    let Some(item_value) = item_date_value(item, source, ctx) else {
        return false;
    };

    match matcher {
        DateMatcher::Compare { op, value } => compare_item_value(item_value, *op, value, ctx),
        DateMatcher::Range { from, through } => item_in_range(item_value, from, through, ctx),
    }
}

fn compare_item_value(
    item_value: DateTime,
    op: DateCompareOp,
    value: &DateValueExpr,
    ctx: &EvaluationContext,
) -> bool {
    let resolved = resolve_value(value, ctx);
    match (op, resolved) {
        (DateCompareOp::On, ResolvedValue::Date(date)) => {
            let start = at_midnight(date);
            let end = start_of_next_day(date);
            item_value >= start && item_value < end
        }
        (DateCompareOp::On, ResolvedValue::DateTime(datetime)) => item_value == datetime,
        (DateCompareOp::Before, ResolvedValue::Date(date)) => item_value < at_midnight(date),
        (DateCompareOp::Before, ResolvedValue::DateTime(datetime)) => item_value < datetime,
        (DateCompareOp::After, ResolvedValue::Date(date)) => item_value >= start_of_next_day(date),
        (DateCompareOp::After, ResolvedValue::DateTime(datetime)) => item_value > datetime,
        (DateCompareOp::AtOrBefore, ResolvedValue::Date(date)) => {
            item_value < start_of_next_day(date)
        }
        (DateCompareOp::AtOrBefore, ResolvedValue::DateTime(datetime)) => item_value <= datetime,
        (DateCompareOp::AtOrAfter, ResolvedValue::Date(date)) => item_value >= at_midnight(date),
        (DateCompareOp::AtOrAfter, ResolvedValue::DateTime(datetime)) => item_value >= datetime,
    }
}

fn item_in_range(
    item_value: DateTime,
    from: &DateValueExpr,
    through: &DateValueExpr,
    ctx: &EvaluationContext,
) -> bool {
    let lower_ok = match resolve_value(from, ctx) {
        ResolvedValue::Date(date) => item_value >= at_midnight(date),
        ResolvedValue::DateTime(datetime) => item_value >= datetime,
    };
    let upper_ok = match resolve_value(through, ctx) {
        ResolvedValue::Date(date) => item_value < start_of_next_day(date),
        ResolvedValue::DateTime(datetime) => item_value <= datetime,
    };
    lower_ok && upper_ok
}

fn item_date_value(item: &Item, source: DateSource, ctx: &EvaluationContext) -> Option<DateTime> {
    match source {
        DateSource::When => item.when_date,
        DateSource::Done => item.done_date,
        DateSource::Entry => Some(item.created_at.to_zoned(ctx.timezone()).datetime()),
    }
}

fn resolve_value(value: &DateValueExpr, ctx: &EvaluationContext) -> ResolvedValue {
    match value {
        DateValueExpr::Today => ResolvedValue::Date(ctx.today()),
        DateValueExpr::Tomorrow => ResolvedValue::Date(
            ctx.today()
                .checked_add(Span::new().days(1))
                .expect("tomorrow should be representable"),
        ),
        DateValueExpr::DaysFromToday(days) => ResolvedValue::Date(
            ctx.today()
                .checked_add(Span::new().days(i64::from(*days)))
                .expect("relative future day should be representable"),
        ),
        DateValueExpr::DaysAgo(days) => ResolvedValue::Date(
            ctx.today()
                .checked_add(Span::new().days(-i64::from(*days)))
                .expect("relative past day should be representable"),
        ),
        DateValueExpr::AbsoluteDate(date) => ResolvedValue::Date(*date),
        DateValueExpr::AbsoluteDateTime(datetime) => ResolvedValue::DateTime(*datetime),
        DateValueExpr::TimeToday(time) => {
            ResolvedValue::DateTime(datetime_from_date_time(ctx.today(), *time))
        }
    }
}

fn datetime_from_date_time(date: Date, time: Time) -> DateTime {
    DateTime::new(
        date.year(),
        date.month(),
        date.day(),
        time.hour(),
        time.minute(),
        0,
        0,
    )
    .expect("combined civil datetime should be representable")
}

fn at_midnight(date: Date) -> DateTime {
    DateTime::new(date.year(), date.month(), date.day(), 0, 0, 0, 0)
        .expect("midnight should be representable")
}

fn start_of_next_day(date: Date) -> DateTime {
    let next = date
        .checked_add(Span::new().days(1))
        .expect("next day should be representable");
    at_midnight(next)
}

fn parse_relative_day_count(lower: &str, suffix: &str) -> Option<i32> {
    let prefix = lower.strip_suffix(suffix)?.trim();
    let number = prefix.strip_suffix(" day").unwrap_or(prefix);
    let number = number.strip_suffix(" days").unwrap_or(number);
    number.parse::<i32>().ok()
}

fn parse_time_expr(input: &str) -> Result<Time, String> {
    let value = input.trim();
    if value.is_empty() {
        return Err("time value before 'today' cannot be empty".to_string());
    }
    let parser = BasicDateParser::default();
    let phrase = format!("today at {value}");
    let reference = Zoned::now().date();
    let parsed = parser
        .parse(&phrase, reference)
        .ok_or_else(|| format!("could not parse time from '{value}'"))?;
    Ok(parsed.datetime.time())
}

fn expression_has_explicit_time(input: &str) -> bool {
    input.contains(':')
        || input.contains(" am")
        || input.contains(" pm")
        || input.ends_with("am")
        || input.ends_with("pm")
        || input.contains("noon")
        || input.contains("midnight")
}

fn render_time(time: Time) -> String {
    let hour24 = time.hour();
    let minute = time.minute();
    let meridiem = if hour24 >= 12 { "pm" } else { "am" };
    let mut hour12 = hour24 % 12;
    if hour12 == 0 {
        hour12 = 12;
    }
    format!("{hour12}:{minute:02}{meridiem}")
}

fn default_afternoon_start() -> Time {
    Time::new(13, 0, 0, 0).expect("1pm should be representable")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{DateMatcher, DateSource, DateValueExpr, Item};
    use jiff::civil::date;

    #[test]
    fn parse_relative_day_expressions() {
        assert_eq!(
            parse_date_value_expr("7 days from today").unwrap(),
            DateValueExpr::DaysFromToday(7)
        );
        assert_eq!(
            parse_date_value_expr("2 days ago").unwrap(),
            DateValueExpr::DaysAgo(2)
        );
    }

    #[test]
    fn parse_month_name_absolute_date() {
        assert_eq!(
            parse_date_value_expr("Nov 12, 1990").unwrap(),
            DateValueExpr::AbsoluteDate(date(1990, 11, 12))
        );
    }

    #[test]
    fn compare_before_today_uses_start_of_day() {
        let ctx = EvaluationContext::from_zoned(
            "2026-02-16T09:30:00-08:00[America/Los_Angeles]"
                .parse()
                .unwrap(),
        );
        let mut item = Item::new("demo".to_string());
        item.when_date = Some(DateTime::new(2026, 2, 15, 23, 59, 0, 0).unwrap());
        assert!(item_matches_date_condition(
            &item,
            DateSource::When,
            &DateMatcher::Compare {
                op: DateCompareOp::Before,
                value: DateValueExpr::Today,
            },
            &ctx
        ));
    }

    #[test]
    fn compare_at_or_after_time_today_is_open_ended() {
        let ctx = EvaluationContext::from_zoned(
            "2026-04-04T09:30:00-07:00[America/Los_Angeles]"
                .parse()
                .unwrap(),
        );
        let mut item = Item::new("demo".to_string());
        item.when_date = Some(DateTime::new(2026, 4, 5, 14, 0, 0, 0).unwrap());
        assert!(item_matches_date_condition(
            &item,
            DateSource::When,
            &DateMatcher::Compare {
                op: DateCompareOp::AtOrAfter,
                value: DateValueExpr::TimeToday(jiff::civil::Time::new(13, 0, 0, 0).unwrap()),
            },
            &ctx
        ));
    }
}
