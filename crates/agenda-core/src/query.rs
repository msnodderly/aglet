use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime};

use crate::model::{Item, Query, WhenBucket};

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

/// Evaluate a query against a slice of items, preserving input order.
pub fn evaluate_query<'a>(
    query: &Query,
    items: &'a [Item],
    reference_date: NaiveDate,
) -> Vec<&'a Item> {
    let normalized_search = query.text_search.as_ref().map(|term| term.to_ascii_lowercase());

    items
        .iter()
        .filter(|item| item_matches_query(query, item, reference_date, normalized_search.as_deref()))
        .collect()
}

fn item_matches_query(
    query: &Query,
    item: &Item,
    reference_date: NaiveDate,
    normalized_search: Option<&str>,
) -> bool {
    let include_matches = query
        .include
        .iter()
        .all(|category_id| item.assignments.contains_key(category_id));
    if !include_matches {
        return false;
    }

    let exclude_matches = query
        .exclude
        .iter()
        .all(|category_id| !item.assignments.contains_key(category_id));
    if !exclude_matches {
        return false;
    }

    let bucket = if query.virtual_include.is_empty() && query.virtual_exclude.is_empty() {
        None
    } else {
        Some(resolve_when_bucket(item.when_date, reference_date))
    };

    if let Some(item_bucket) = bucket {
        let virtual_include_matches = query
            .virtual_include
            .iter()
            .all(|required_bucket| *required_bucket == item_bucket);
        if !virtual_include_matches {
            return false;
        }

        let virtual_exclude_matches = query
            .virtual_exclude
            .iter()
            .all(|blocked_bucket| *blocked_bucket != item_bucket);
        if !virtual_exclude_matches {
            return false;
        }
    }

    if let Some(search_term) = normalized_search {
        if !matches_text_search(item, search_term) {
            return false;
        }
    }

    true
}

fn matches_text_search(item: &Item, search_term: &str) -> bool {
    let text_matches = item.text.to_ascii_lowercase().contains(search_term);
    if text_matches {
        return true;
    }

    item.note
        .as_ref()
        .map(|note| note.to_ascii_lowercase().contains(search_term))
        .unwrap_or(false)
}

fn start_of_iso_week(date: NaiveDate) -> NaiveDate {
    date.checked_sub_signed(Duration::days(date.weekday().num_days_from_monday() as i64))
        .expect("valid ISO week start")
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use chrono::{NaiveDate, NaiveDateTime, Utc};
    use uuid::Uuid;

    use super::{evaluate_query, resolve_when_bucket};
    use crate::model::{Assignment, AssignmentSource, CategoryId, Item, Query, WhenBucket};

    fn day(year: i32, month: u32, date: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, date).unwrap()
    }

    fn datetime(year: i32, month: u32, date: u32, hour: u32, minute: u32) -> NaiveDateTime {
        day(year, month, date).and_hms_opt(hour, minute, 0).unwrap()
    }

    fn assignment() -> Assignment {
        Assignment {
            source: AssignmentSource::Manual,
            assigned_at: Utc::now(),
            sticky: true,
            origin: Some("manual:test".to_string()),
        }
    }

    fn item_with_assignments(
        text: &str,
        note: Option<&str>,
        when_date: Option<NaiveDateTime>,
        assigned_categories: &[CategoryId],
    ) -> Item {
        let mut item = Item::new(text.to_string());
        item.note = note.map(ToString::to_string);
        item.when_date = when_date;
        item.assignments = HashMap::new();

        for category_id in assigned_categories {
            item.assignments.insert(*category_id, assignment());
        }

        item
    }

    fn ids(items: &[&Item]) -> Vec<Uuid> {
        items.iter().map(|item| item.id).collect()
    }

    #[test]
    fn evaluate_query_empty_query_matches_everything() {
        let reference = day(2026, 2, 11);
        let items = vec![
            item_with_assignments("alpha", None, None, &[]),
            item_with_assignments("beta", Some("note"), Some(datetime(2026, 2, 12, 9, 0)), &[]),
            item_with_assignments("gamma", None, Some(datetime(2026, 2, 13, 9, 0)), &[]),
        ];

        let query = Query::default();
        let result = evaluate_query(&query, &items, reference);
        assert_eq!(ids(&result), items.iter().map(|item| item.id).collect::<Vec<_>>());
    }

    #[test]
    fn evaluate_query_include_single_category() {
        let reference = day(2026, 2, 11);
        let category_a = Uuid::new_v4();
        let items = vec![
            item_with_assignments("has a", None, None, &[category_a]),
            item_with_assignments("no a", None, None, &[]),
        ];

        let mut query = Query::default();
        query.include.insert(category_a);

        let result = evaluate_query(&query, &items, reference);
        assert_eq!(ids(&result), vec![items[0].id]);
    }

    #[test]
    fn evaluate_query_include_multiple_categories_and_semantics() {
        let reference = day(2026, 2, 11);
        let category_a = Uuid::new_v4();
        let category_b = Uuid::new_v4();
        let items = vec![
            item_with_assignments("a+b", None, None, &[category_a, category_b]),
            item_with_assignments("a only", None, None, &[category_a]),
            item_with_assignments("b only", None, None, &[category_b]),
        ];

        let mut query = Query::default();
        query.include.extend([category_a, category_b]);

        let result = evaluate_query(&query, &items, reference);
        assert_eq!(ids(&result), vec![items[0].id]);
    }

    #[test]
    fn evaluate_query_exclude_single_category() {
        let reference = day(2026, 2, 11);
        let category_a = Uuid::new_v4();
        let items = vec![
            item_with_assignments("has a", None, None, &[category_a]),
            item_with_assignments("no a", None, None, &[]),
        ];

        let mut query = Query::default();
        query.exclude.insert(category_a);

        let result = evaluate_query(&query, &items, reference);
        assert_eq!(ids(&result), vec![items[1].id]);
    }

    #[test]
    fn evaluate_query_exclude_multiple_categories() {
        let reference = day(2026, 2, 11);
        let category_a = Uuid::new_v4();
        let category_b = Uuid::new_v4();
        let items = vec![
            item_with_assignments("has a", None, None, &[category_a]),
            item_with_assignments("has b", None, None, &[category_b]),
            item_with_assignments("clean", None, None, &[]),
        ];

        let mut query = Query::default();
        query.exclude.extend([category_a, category_b]);

        let result = evaluate_query(&query, &items, reference);
        assert_eq!(ids(&result), vec![items[2].id]);
    }

    #[test]
    fn evaluate_query_include_and_exclude_combined() {
        let reference = day(2026, 2, 11);
        let category_a = Uuid::new_v4();
        let category_b = Uuid::new_v4();
        let items = vec![
            item_with_assignments("a+b", None, None, &[category_a, category_b]),
            item_with_assignments("a only", None, None, &[category_a]),
        ];

        let mut query = Query::default();
        query.include.insert(category_a);
        query.exclude.insert(category_b);

        let result = evaluate_query(&query, &items, reference);
        assert_eq!(ids(&result), vec![items[1].id]);
    }

    #[test]
    fn evaluate_query_virtual_include_filters_by_bucket() {
        let reference = day(2026, 2, 11);
        let items = vec![
            item_with_assignments("today", None, Some(datetime(2026, 2, 11, 9, 0)), &[]),
            item_with_assignments("tomorrow", None, Some(datetime(2026, 2, 12, 9, 0)), &[]),
            item_with_assignments("none", None, None, &[]),
        ];

        let mut query = Query::default();
        query.virtual_include.insert(WhenBucket::Today);

        let result = evaluate_query(&query, &items, reference);
        assert_eq!(ids(&result), vec![items[0].id]);
    }

    #[test]
    fn evaluate_query_virtual_exclude_filters_by_bucket() {
        let reference = day(2026, 2, 11);
        let items = vec![
            item_with_assignments("none", None, None, &[]),
            item_with_assignments("today", None, Some(datetime(2026, 2, 11, 9, 0)), &[]),
        ];

        let mut query = Query::default();
        query.virtual_exclude.insert(WhenBucket::NoDate);

        let result = evaluate_query(&query, &items, reference);
        assert_eq!(ids(&result), vec![items[1].id]);
    }

    #[test]
    fn evaluate_query_text_search_matches_item_text() {
        let reference = day(2026, 2, 11);
        let items = vec![
            item_with_assignments("Team meeting", None, None, &[]),
            item_with_assignments("Buy groceries", None, None, &[]),
        ];

        let mut query = Query::default();
        query.text_search = Some("meeting".to_string());

        let result = evaluate_query(&query, &items, reference);
        assert_eq!(ids(&result), vec![items[0].id]);
    }

    #[test]
    fn evaluate_query_text_search_matches_note() {
        let reference = day(2026, 2, 11);
        let items = vec![
            item_with_assignments("Title", Some("Discuss roadmap"), None, &[]),
            item_with_assignments("Other", Some("Random"), None, &[]),
        ];

        let mut query = Query::default();
        query.text_search = Some("roadmap".to_string());

        let result = evaluate_query(&query, &items, reference);
        assert_eq!(ids(&result), vec![items[0].id]);
    }

    #[test]
    fn evaluate_query_text_search_case_insensitive() {
        let reference = day(2026, 2, 11);
        let items = vec![
            item_with_assignments("urgent task", None, None, &[]),
            item_with_assignments("normal task", None, None, &[]),
        ];

        let mut query = Query::default();
        query.text_search = Some("URGENT".to_string());

        let result = evaluate_query(&query, &items, reference);
        assert_eq!(ids(&result), vec![items[0].id]);
    }

    #[test]
    fn evaluate_query_all_criteria_are_anded() {
        let reference = day(2026, 2, 11);
        let category_a = Uuid::new_v4();
        let category_b = Uuid::new_v4();
        let items = vec![
            item_with_assignments(
                "Team meeting",
                Some("today focus"),
                Some(datetime(2026, 2, 11, 9, 0)),
                &[category_a],
            ),
            item_with_assignments(
                "Team meeting",
                Some("today focus"),
                Some(datetime(2026, 2, 11, 9, 0)),
                &[category_a, category_b],
            ),
            item_with_assignments(
                "Team sync",
                Some("today focus"),
                Some(datetime(2026, 2, 11, 9, 0)),
                &[category_a],
            ),
        ];

        let mut query = Query::default();
        query.include.insert(category_a);
        query.exclude.insert(category_b);
        query.virtual_include.insert(WhenBucket::Today);
        query.text_search = Some("meeting".to_string());

        let result = evaluate_query(&query, &items, reference);
        assert_eq!(ids(&result), vec![items[0].id]);
    }

    #[test]
    fn evaluate_query_empty_include_is_permissive() {
        let reference = day(2026, 2, 11);
        let category_a = Uuid::new_v4();
        let items = vec![
            item_with_assignments("meeting", None, None, &[category_a]),
            item_with_assignments("meeting", None, None, &[]),
        ];

        let mut query = Query::default();
        query.text_search = Some("meeting".to_string());

        let result = evaluate_query(&query, &items, reference);
        assert_eq!(ids(&result), vec![items[0].id, items[1].id]);
    }

    #[test]
    fn evaluate_query_virtual_include_with_multiple_buckets_matches_none() {
        let reference = day(2026, 2, 11);
        let items = vec![
            item_with_assignments("today", None, Some(datetime(2026, 2, 11, 9, 0)), &[]),
            item_with_assignments("tomorrow", None, Some(datetime(2026, 2, 12, 9, 0)), &[]),
        ];

        let mut query = Query::default();
        query
            .virtual_include
            .extend([WhenBucket::Today, WhenBucket::Tomorrow]);

        let result = evaluate_query(&query, &items, reference);
        assert!(result.is_empty());
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
