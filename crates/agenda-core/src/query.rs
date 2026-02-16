use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime};

use crate::model::WhenBucket;

/// Resolve a `when_date` into its virtual `WhenBucket` for a given reference date.
pub fn resolve_when_bucket(
    when_date: Option<NaiveDateTime>,
    reference_date: NaiveDate,
) -> WhenBucket {
    let Some(when_datetime) = when_date else {
        return WhenBucket::NoDate;
    };

    let when_day = when_datetime.date();

    if when_day < reference_date {
        return WhenBucket::Overdue;
    }

    if when_day == reference_date {
        return WhenBucket::Today;
    }

    if let Some(tomorrow) = reference_date.succ_opt() {
        if when_day == tomorrow {
            return WhenBucket::Tomorrow;
        }
    }

    let this_week_start = start_of_iso_week(reference_date);
    let this_week_end = this_week_start
        .checked_add_signed(Duration::days(6))
        .expect("valid week range");

    if when_day > reference_date && when_day >= this_week_start && when_day <= this_week_end {
        return WhenBucket::ThisWeek;
    }

    let next_week_start = this_week_start
        .checked_add_signed(Duration::days(7))
        .expect("valid next week start");
    let next_week_end = next_week_start
        .checked_add_signed(Duration::days(6))
        .expect("valid next week range");

    if when_day >= next_week_start && when_day <= next_week_end {
        return WhenBucket::NextWeek;
    }

    if when_day.year() == reference_date.year() && when_day.month() == reference_date.month() {
        return WhenBucket::ThisMonth;
    }

    WhenBucket::Future
}

fn start_of_iso_week(date: NaiveDate) -> NaiveDate {
    date.checked_sub_signed(Duration::days(date.weekday().num_days_from_monday() as i64))
        .expect("valid ISO week start")
}

#[cfg(test)]
mod tests {
    use chrono::{NaiveDate, NaiveDateTime};

    use super::resolve_when_bucket;
    use crate::model::WhenBucket;

    fn day(year: i32, month: u32, date: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, date).unwrap()
    }

    fn datetime(year: i32, month: u32, date: u32, hour: u32, minute: u32) -> NaiveDateTime {
        day(year, month, date).and_hms_opt(hour, minute, 0).unwrap()
    }

    #[test]
    fn resolve_no_date_bucket() {
        let reference = day(2026, 2, 11);
        assert_eq!(resolve_when_bucket(None, reference), WhenBucket::NoDate);
    }

    #[test]
    fn resolve_overdue_bucket() {
        let reference = day(2026, 2, 11);
        let when_date = Some(datetime(2026, 2, 10, 9, 0));
        assert_eq!(
            resolve_when_bucket(when_date, reference),
            WhenBucket::Overdue
        );
    }

    #[test]
    fn resolve_today_bucket() {
        let reference = day(2026, 2, 11);
        let when_date = Some(datetime(2026, 2, 11, 9, 0));
        assert_eq!(resolve_when_bucket(when_date, reference), WhenBucket::Today);
    }

    #[test]
    fn resolve_tomorrow_bucket() {
        let reference = day(2026, 2, 11);
        let when_date = Some(datetime(2026, 2, 12, 9, 0));
        assert_eq!(resolve_when_bucket(when_date, reference), WhenBucket::Tomorrow);
    }

    #[test]
    fn resolve_this_week_bucket() {
        let reference = day(2026, 2, 11);
        let when_date = Some(datetime(2026, 2, 14, 9, 0));
        assert_eq!(resolve_when_bucket(when_date, reference), WhenBucket::ThisWeek);
    }

    #[test]
    fn resolve_next_week_bucket() {
        let reference = day(2026, 2, 11);
        let when_date = Some(datetime(2026, 2, 16, 9, 0));
        assert_eq!(resolve_when_bucket(when_date, reference), WhenBucket::NextWeek);
    }

    #[test]
    fn resolve_this_month_bucket() {
        let reference = day(2026, 2, 11);
        let when_date = Some(datetime(2026, 2, 27, 9, 0));
        assert_eq!(resolve_when_bucket(when_date, reference), WhenBucket::ThisMonth);
    }

    #[test]
    fn resolve_future_bucket() {
        let reference = day(2026, 2, 11);
        let when_date = Some(datetime(2026, 3, 15, 9, 0));
        assert_eq!(resolve_when_bucket(when_date, reference), WhenBucket::Future);
    }

    #[test]
    fn today_priority_over_this_week() {
        let reference = day(2026, 2, 9); // Monday
        let when_date = Some(datetime(2026, 2, 9, 9, 0));
        assert_eq!(resolve_when_bucket(when_date, reference), WhenBucket::Today);
    }

    #[test]
    fn tomorrow_priority_over_this_week() {
        let reference = day(2026, 2, 9); // Monday
        let when_date = Some(datetime(2026, 2, 10, 9, 0));
        assert_eq!(resolve_when_bucket(when_date, reference), WhenBucket::Tomorrow);
    }

    #[test]
    fn time_component_is_ignored_for_bucketing() {
        let reference = day(2026, 2, 11);
        let when_date = Some(datetime(2026, 2, 11, 9, 0));
        assert_eq!(resolve_when_bucket(when_date, reference), WhenBucket::Today);
    }

    #[test]
    fn far_future_is_future_bucket() {
        let reference = day(2026, 2, 11);
        let when_date = Some(datetime(2027, 2, 11, 9, 0));
        assert_eq!(resolve_when_bucket(when_date, reference), WhenBucket::Future);
    }

    #[test]
    fn week_boundary_saturday_to_sunday_and_monday() {
        let reference = day(2026, 2, 14); // Saturday
        let sunday = Some(datetime(2026, 2, 15, 9, 0));
        let monday = Some(datetime(2026, 2, 16, 9, 0));

        assert_eq!(resolve_when_bucket(sunday, reference), WhenBucket::Tomorrow);
        assert_eq!(resolve_when_bucket(monday, reference), WhenBucket::NextWeek);
    }
}
