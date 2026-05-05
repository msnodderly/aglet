use jiff::civil::{Date, DateTime};
use jiff::Span;
use std::collections::{HashMap, HashSet};

use crate::model::{
    month_name, weekday_name, Category, CategoryId, DateSource, DatebookAnchor, DatebookConfig,
    DatebookInterval, DatebookPeriod, Item, Query, Section, View, WhenBucket,
};

/// Resolve a `when_date` into its most-specific virtual `WhenBucket` for a given reference date.
///
/// Used for grouping/display where a single bucket label is needed. For *filter matching*
/// against a set of buckets (where buckets can overlap, e.g. `ThisMonth` ⊂ `ThisYear`),
/// use [`bucket_contains`] instead.
pub fn resolve_when_bucket(when_date: Option<DateTime>, reference_date: Date) -> WhenBucket {
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

    if let Ok(tomorrow) = reference_date.checked_add(Span::new().days(1)) {
        if when_day == tomorrow {
            return WhenBucket::Tomorrow;
        }
    }

    let this_week_start = start_of_iso_week(reference_date);
    let this_week_end = this_week_start
        .checked_add(Span::new().days(6))
        .expect("valid week range");

    if when_day > reference_date && when_day >= this_week_start && when_day <= this_week_end {
        return WhenBucket::ThisWeek;
    }

    let next_week_start = this_week_start
        .checked_add(Span::new().days(7))
        .expect("valid next week start");
    let next_week_end = next_week_start
        .checked_add(Span::new().days(6))
        .expect("valid next week range");

    if when_day >= next_week_start && when_day <= next_week_end {
        return WhenBucket::NextWeek;
    }

    if when_day.year() == reference_date.year() && when_day.month() == reference_date.month() {
        return WhenBucket::ThisMonth;
    }

    WhenBucket::Future
}

/// Returns true if `when_date` falls within the date range described by `bucket`,
/// for the given reference date. Unlike [`resolve_when_bucket`], buckets here are
/// treated as overlapping ranges — a Today date is also in `ThisWeek`, `ThisMonth`,
/// `ThisYear`, `Next12Months`, and `Future`.
pub fn bucket_contains(
    bucket: WhenBucket,
    when_date: Option<DateTime>,
    reference_date: Date,
) -> bool {
    let Some(when_datetime) = when_date else {
        return matches!(bucket, WhenBucket::NoDate);
    };
    let when_day = when_datetime.date();

    match bucket {
        WhenBucket::NoDate => false,
        WhenBucket::Overdue => when_day < reference_date,
        WhenBucket::Today => when_day == reference_date,
        WhenBucket::Tomorrow => reference_date
            .checked_add(Span::new().days(1))
            .map(|d| when_day == d)
            .unwrap_or(false),
        WhenBucket::ThisWeek => {
            let start = start_of_iso_week(reference_date);
            let end = start
                .checked_add(Span::new().days(6))
                .expect("valid week range");
            when_day >= start && when_day <= end
        }
        WhenBucket::NextWeek => {
            let start = start_of_iso_week(reference_date)
                .checked_add(Span::new().days(7))
                .expect("valid next week start");
            let end = start
                .checked_add(Span::new().days(6))
                .expect("valid next week range");
            when_day >= start && when_day <= end
        }
        WhenBucket::ThisMonth => {
            when_day.year() == reference_date.year() && when_day.month() == reference_date.month()
        }
        WhenBucket::NextMonth => {
            let (year, month) = if reference_date.month() == 12 {
                (reference_date.year() + 1, 1u8)
            } else {
                (reference_date.year(), reference_date.month() as u8 + 1)
            };
            when_day.year() == year && when_day.month() as u8 == month
        }
        WhenBucket::ThisYear => when_day.year() == reference_date.year(),
        WhenBucket::Next12Months => {
            let end = reference_date
                .checked_add(Span::new().months(12))
                .unwrap_or(reference_date);
            when_day >= reference_date && when_day < end
        }
        WhenBucket::Future => when_day > reference_date,
    }
}

/// Evaluate a query against a slice of items, preserving input order.
pub fn evaluate_query<'a>(query: &Query, items: &'a [Item], reference_date: Date) -> Vec<&'a Item> {
    let normalized_search = query
        .text_search
        .as_ref()
        .map(|term| term.to_ascii_lowercase());

    items
        .iter()
        .filter(|item| {
            item_matches_query(query, item, reference_date, normalized_search.as_deref())
        })
        .collect()
}

#[derive(Debug, Clone)]
pub struct ViewSectionResult {
    pub section_index: usize,
    pub title: String,
    pub items: Vec<Item>,
    pub subsections: Vec<ViewSubsectionResult>,
}

#[derive(Debug, Clone)]
pub struct ViewSubsectionResult {
    pub subsection_index: usize,
    pub title: String,
    pub items: Vec<Item>,
    pub on_insert_assign: HashSet<CategoryId>,
    pub on_remove_unassign: HashSet<CategoryId>,
}

#[derive(Debug, Clone)]
pub struct ViewResult {
    pub sections: Vec<ViewSectionResult>,
    pub unmatched: Option<Vec<Item>>,
    pub unmatched_label: Option<String>,
}

/// Resolve a view into ordered section groups and an optional unmatched group.
pub fn resolve_view(
    view: &View,
    items: &[Item],
    categories: &[Category],
    reference_date: Date,
) -> ViewResult {
    if let Some(config) = &view.datebook_config {
        return resolve_datebook_view(view, config, items, reference_date);
    }

    let categories_by_id: HashMap<CategoryId, &Category> = categories
        .iter()
        .map(|category| (category.id, category))
        .collect();
    let view_items: Vec<Item> = evaluate_query(&view.criteria, items, reference_date)
        .into_iter()
        .cloned()
        .collect();

    let mut matched_in_sections = HashSet::new();
    let sections = view
        .sections
        .iter()
        .enumerate()
        .map(|(section_index, section)| {
            let section_items = evaluate_query(&section.criteria, &view_items, reference_date);
            matched_in_sections.extend(section_items.iter().map(|item| item.id));

            if let Some(subsections) =
                expand_show_children_subsections(section, &section_items, &categories_by_id)
            {
                return ViewSectionResult {
                    section_index,
                    title: section.title.clone(),
                    items: Vec::new(),
                    subsections,
                };
            }

            ViewSectionResult {
                section_index,
                title: section.title.clone(),
                items: section_items.into_iter().cloned().collect(),
                subsections: Vec::new(),
            }
        })
        .collect();

    let (unmatched, unmatched_label) = if view.show_unmatched {
        let unmatched_items = view_items
            .iter()
            .filter(|item| !matched_in_sections.contains(&item.id))
            .cloned()
            .collect();
        (Some(unmatched_items), Some(view.unmatched_label.clone()))
    } else {
        (None, None)
    };

    ViewResult {
        sections,
        unmatched,
        unmatched_label,
    }
}

fn expand_show_children_subsections(
    section: &Section,
    section_items: &[&Item],
    categories_by_id: &HashMap<CategoryId, &Category>,
) -> Option<Vec<ViewSubsectionResult>> {
    let parent_id = show_children_parent_category_id(section)?;
    let parent = categories_by_id.get(&parent_id)?;

    let mut child_entries = Vec::new();
    for child_id in &parent.children {
        if let Some(child) = categories_by_id.get(child_id) {
            child_entries.push((*child_id, child.name.clone()));
        }
    }
    if child_entries.is_empty() {
        // No child sections to expand into; keep the base section rendering.
        return None;
    }

    let mut child_ids_in_result = HashSet::new();
    let mut subsections = Vec::with_capacity(child_entries.len() + 1);
    for (subsection_index, (child_id, child_name)) in child_entries.iter().enumerate() {
        child_ids_in_result.insert(*child_id);
        let items = section_items
            .iter()
            .filter(|item| item.assignments.contains_key(child_id))
            .cloned()
            .cloned()
            .collect();
        let mut on_insert_assign = section.on_insert_assign.clone();
        on_insert_assign.insert(*child_id);

        subsections.push(ViewSubsectionResult {
            subsection_index,
            title: child_name.clone(),
            items,
            on_insert_assign,
            on_remove_unassign: section.on_remove_unassign.clone(),
        });
    }

    let unmatched_items = section_items
        .iter()
        .filter(|item| {
            !item
                .assignments
                .keys()
                .any(|assigned_id| child_ids_in_result.contains(assigned_id))
        })
        .cloned()
        .cloned()
        .collect();
    subsections.push(ViewSubsectionResult {
        subsection_index: subsections.len(),
        title: format!("{} (Other)", parent.name),
        items: unmatched_items,
        on_insert_assign: section.on_insert_assign.clone(),
        on_remove_unassign: section.on_remove_unassign.clone(),
    });

    Some(subsections)
}

fn show_children_parent_category_id(section: &Section) -> Option<CategoryId> {
    if !section.show_children {
        return None;
    }

    if !section.criteria.virtual_include.is_empty()
        || !section.criteria.virtual_exclude.is_empty()
        || section.criteria.text_search.is_some()
    {
        return None;
    }

    // Exactly 1 And criterion, 0 Not, 0 Or
    let and_ids: Vec<CategoryId> = section.criteria.and_category_ids().collect();
    let not_count = section.criteria.not_category_ids().count();
    let or_count = section.criteria.or_category_ids().count();

    if and_ids.len() != 1 || not_count != 0 || or_count != 0 {
        return None;
    }

    Some(and_ids[0])
}

fn item_matches_query(
    query: &Query,
    item: &Item,
    reference_date: Date,
    normalized_search: Option<&str>,
) -> bool {
    // And: ALL And-mode categories must be present
    let and_matches = query
        .and_category_ids()
        .all(|category_id| item.assignments.contains_key(&category_id));
    if !and_matches {
        return false;
    }

    // Not: ALL Not-mode categories must be absent
    let not_matches = query
        .not_category_ids()
        .all(|category_id| !item.assignments.contains_key(&category_id));
    if !not_matches {
        return false;
    }

    // Or: If any Or criteria exist, at least ONE must be present
    let mut or_ids = query.or_category_ids().peekable();
    if or_ids.peek().is_some()
        && !or_ids.any(|category_id| item.assignments.contains_key(&category_id))
    {
        return false;
    }

    if !query.virtual_include.is_empty() {
        let any_include_match = query
            .virtual_include
            .iter()
            .any(|bucket| bucket_contains(*bucket, item.when_date, reference_date));
        if !any_include_match {
            return false;
        }
    }

    if !query.virtual_exclude.is_empty() {
        let any_exclude_match = query
            .virtual_exclude
            .iter()
            .any(|bucket| bucket_contains(*bucket, item.when_date, reference_date));
        if any_exclude_match {
            return false;
        }
    }

    if let Some(search_term) = normalized_search {
        if !matches_text_search(item, search_term, None) {
            return false;
        }
    }

    true
}

/// Check whether an item matches a text search term.
///
/// The `search_term` must already be ASCII-lowercased by the caller.
/// When `category_names_lower` is provided, items whose assigned category
/// names contain the search term will also match.
pub fn matches_text_search(
    item: &Item,
    search_term: &str,
    category_names_lower: Option<&HashMap<CategoryId, String>>,
) -> bool {
    if item.text.to_ascii_lowercase().contains(search_term) {
        return true;
    }

    if is_uuid_search_candidate(search_term) {
        let item_id = item.id.to_string();
        if item_id.contains(search_term) {
            return true;
        }

        let compact_search = search_term.replace('-', "");
        if !compact_search.is_empty() && item.id.as_simple().to_string().contains(&compact_search) {
            return true;
        }
    }

    if item
        .note
        .as_ref()
        .map(|note| note.to_ascii_lowercase().contains(search_term))
        .unwrap_or(false)
    {
        return true;
    }

    if let Some(names) = category_names_lower {
        if item.assignments.keys().any(|category_id| {
            names
                .get(category_id)
                .map(|name| name.contains(search_term))
                .unwrap_or(false)
        }) {
            return true;
        }
    }

    false
}

fn is_uuid_search_candidate(search_term: &str) -> bool {
    let compact_len = search_term.chars().filter(|ch| *ch != '-').count();
    compact_len >= 3
        && search_term
            .chars()
            .all(|ch| ch.is_ascii_hexdigit() || ch == '-')
}

fn start_of_iso_week(date: Date) -> Date {
    let offset = date.weekday().to_monday_zero_offset() as i64;
    date.checked_sub(Span::new().days(offset))
        .expect("valid ISO week start")
}

// ── Datebook section generation ─────────────────────────────────────

/// A dynamically generated datebook section with time boundaries.
#[derive(Debug, Clone)]
pub struct DatebookSection {
    pub title: String,
    pub range_start: DateTime,
    pub range_end: DateTime, // exclusive
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

/// Compute the visible time window for a datebook config.
pub fn compute_datebook_window(
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
        DatebookAnchor::StartOfMonth => Date::new(ref_date.year(), ref_date.month(), 1)
            .expect("first of month is valid")
            .at(0, 0, 0, 0),
        DatebookAnchor::StartOfQuarter => {
            let q_month = ((ref_date.month() - 1) / 3) * 3 + 1;
            Date::new(ref_date.year(), q_month, 1)
                .expect("quarter start is valid")
                .at(0, 0, 0, 0)
        }
        DatebookAnchor::StartOfYear => Date::new(ref_date.year(), 1, 1)
            .expect("jan 1 is valid")
            .at(0, 0, 0, 0),
        DatebookAnchor::Absolute(d) => d.at(0, 0, 0, 0),
    }
}

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
    if offset == 0 {
        return base;
    }
    let span = match period {
        DatebookPeriod::Day => Span::new().days(i64::from(offset)),
        DatebookPeriod::Week => Span::new().weeks(i64::from(offset)),
        DatebookPeriod::Month => Span::new().months(offset),
        DatebookPeriod::Quarter => Span::new().months(offset * 3),
        DatebookPeriod::Year => Span::new().years(offset),
    };
    base.checked_add(span).expect("browse offset overflow")
}

fn format_datebook_section_title(
    start: DateTime,
    end: DateTime,
    config: &DatebookConfig,
) -> String {
    match config.interval {
        DatebookInterval::Hourly => {
            // "Mon Apr 7, 09:00"
            format!(
                "{}, {:02}:{:02}",
                format_date_short(start.date()),
                start.hour(),
                start.minute()
            )
        }
        DatebookInterval::Daily => {
            // "Mon, Apr 7"
            format_date_with_weekday(start.date())
        }
        DatebookInterval::Weekly => {
            // "Apr 7 - Apr 13"
            let end_date = end.checked_sub(Span::new().days(1)).unwrap_or(end).date();
            format!(
                "{} - {}",
                format_date_short(start.date()),
                format_date_short(end_date)
            )
        }
        DatebookInterval::Monthly => {
            // "April 2026"
            format!(
                "{} {}",
                month_name(start.date().month() as u8),
                start.date().year()
            )
        }
    }
}

/// Short date: "Apr 7" or "Apr 7, 2027" if year differs from section start context.
fn format_date_short(date: Date) -> String {
    let month_abbr = &month_name(date.month() as u8)[..3];
    format!("{} {}", month_abbr, date.day())
}

/// Date with weekday: "Mon, Apr 7"
fn format_date_with_weekday(date: Date) -> String {
    let wd = &weekday_name(date.weekday())[..3];
    let month_abbr = &month_name(date.month() as u8)[..3];
    format!("{}, {} {}", wd, month_abbr, date.day())
}

/// Extract the relevant date from an item based on the configured date source.
pub fn extract_item_date(item: &Item, source: DateSource) -> Option<DateTime> {
    match source {
        DateSource::When => item.when_date,
        DateSource::Done => item.done_date,
        DateSource::Entry => {
            let zdt = item.created_at.to_zoned(jiff::tz::TimeZone::UTC);
            Some(zdt.datetime())
        }
    }
}

/// Resolve a datebook view into ordered section groups.
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

    ViewResult {
        sections,
        unmatched,
        unmatched_label,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use jiff::civil::{Date, DateTime};
    use jiff::Timestamp;
    use uuid::Uuid;

    use super::{
        bucket_contains, evaluate_query, matches_text_search, resolve_view, resolve_when_bucket,
    };
    use crate::model::{
        Assignment, AssignmentSource, Category, CategoryId, CriterionMode, Item, Query, Section,
        View, WhenBucket,
    };

    fn day(year: i16, month: i8, date: i8) -> Date {
        Date::new(year, month, date).unwrap()
    }

    fn datetime(year: i16, month: i8, date: i8, hour: i8, minute: i8) -> DateTime {
        day(year, month, date).at(hour, minute, 0, 0)
    }

    fn assignment() -> Assignment {
        Assignment {
            source: AssignmentSource::Manual,
            assigned_at: Timestamp::now(),
            sticky: true,
            origin: Some("manual:test".to_string()),
            explanation: None,
            numeric_value: None,
        }
    }

    fn item_with_assignments(
        text: &str,
        note: Option<&str>,
        when_date: Option<DateTime>,
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

    fn item_ids(items: &[Item]) -> Vec<Uuid> {
        items.iter().map(|item| item.id).collect()
    }

    fn include_query(category_id: CategoryId) -> Query {
        let mut query = Query::default();
        query.set_criterion(CriterionMode::And, category_id);
        query
    }

    fn section(title: &str, criteria: Query) -> Section {
        Section {
            title: title.to_string(),
            criteria,
            columns: Vec::new(),
            item_column_index: 0,
            on_insert_assign: HashSet::new(),
            on_remove_unassign: HashSet::new(),
            show_children: false,
            board_display_mode_override: None,
        }
    }

    fn view(
        criteria: Query,
        sections: Vec<Section>,
        show_unmatched: bool,
        unmatched_label: &str,
    ) -> View {
        let mut view = View::new("Test View".to_string());
        view.criteria = criteria;
        view.sections = sections;
        view.show_unmatched = show_unmatched;
        view.unmatched_label = unmatched_label.to_string();
        view
    }

    fn category(
        id: CategoryId,
        name: &str,
        parent: Option<CategoryId>,
        children: &[CategoryId],
    ) -> Category {
        let mut category = Category::new(name.to_string());
        category.id = id;
        category.parent = parent;
        category.children = children.to_vec();
        category
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
        assert_eq!(
            ids(&result),
            items.iter().map(|item| item.id).collect::<Vec<_>>()
        );
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
        query.set_criterion(CriterionMode::And, category_a);

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
        query.set_criterion(CriterionMode::And, category_a);
        query.set_criterion(CriterionMode::And, category_b);

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
        query.set_criterion(CriterionMode::Not, category_a);

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
        query.set_criterion(CriterionMode::Not, category_a);
        query.set_criterion(CriterionMode::Not, category_b);

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
        query.set_criterion(CriterionMode::And, category_a);
        query.set_criterion(CriterionMode::Not, category_b);

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

        let query = Query {
            text_search: Some("meeting".to_string()),
            ..Query::default()
        };

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

        let query = Query {
            text_search: Some("roadmap".to_string()),
            ..Query::default()
        };

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

        let query = Query {
            text_search: Some("URGENT".to_string()),
            ..Query::default()
        };

        let result = evaluate_query(&query, &items, reference);
        assert_eq!(ids(&result), vec![items[0].id]);
    }

    #[test]
    fn evaluate_query_text_search_matches_item_uuid_prefix() {
        let reference = day(2026, 2, 11);
        let mut matching = item_with_assignments("uuid target", None, None, &[]);
        matching.id = Uuid::parse_str("123e4567-e89b-12d3-a456-426614174000").expect("valid uuid");
        let mut other = item_with_assignments("other", None, None, &[]);
        other.id = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").expect("valid uuid");
        let items = vec![matching, other];

        let query = Query {
            text_search: Some("123e4567".to_string()),
            ..Query::default()
        };

        let result = evaluate_query(&query, &items, reference);
        assert_eq!(ids(&result), vec![items[0].id]);
    }

    #[test]
    fn evaluate_query_text_search_matches_compact_uuid() {
        let reference = day(2026, 2, 11);
        let mut matching = item_with_assignments("uuid target", None, None, &[]);
        matching.id = Uuid::parse_str("123e4567-e89b-12d3-a456-426614174000").expect("valid uuid");
        let mut other = item_with_assignments("other", None, None, &[]);
        other.id = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").expect("valid uuid");
        let items = vec![matching, other];

        let query = Query {
            text_search: Some("123e4567e89b12d3a456426614174000".to_string()),
            ..Query::default()
        };

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

        let mut query = Query {
            text_search: Some("meeting".to_string()),
            ..Query::default()
        };
        query.set_criterion(CriterionMode::And, category_a);
        query.set_criterion(CriterionMode::Not, category_b);
        query.virtual_include.insert(WhenBucket::Today);

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

        let query = Query {
            text_search: Some("meeting".to_string()),
            ..Query::default()
        };

        let result = evaluate_query(&query, &items, reference);
        assert_eq!(ids(&result), vec![items[0].id, items[1].id]);
    }

    #[test]
    fn evaluate_query_virtual_include_multi_bucket_is_or() {
        let reference = day(2026, 2, 11);
        let items = vec![
            item_with_assignments("today", None, Some(datetime(2026, 2, 11, 9, 0)), &[]),
            item_with_assignments("tomorrow", None, Some(datetime(2026, 2, 12, 9, 0)), &[]),
            item_with_assignments("future", None, Some(datetime(2026, 6, 1, 9, 0)), &[]),
        ];

        let mut query = Query::default();
        query
            .virtual_include
            .extend([WhenBucket::Today, WhenBucket::Tomorrow]);

        let result = evaluate_query(&query, &items, reference);
        assert_eq!(ids(&result), vec![items[0].id, items[1].id]);
    }

    #[test]
    fn evaluate_query_virtual_include_overlapping_buckets() {
        // ThisMonth ⊂ ThisYear: a today item should match either bucket alone.
        let reference = day(2026, 2, 11);
        let items = vec![item_with_assignments(
            "today",
            None,
            Some(datetime(2026, 2, 11, 9, 0)),
            &[],
        )];

        let mut query = Query::default();
        query.virtual_include.insert(WhenBucket::ThisYear);
        let result = evaluate_query(&query, &items, reference);
        assert_eq!(ids(&result), vec![items[0].id]);

        let mut query = Query::default();
        query.virtual_include.insert(WhenBucket::ThisMonth);
        let result = evaluate_query(&query, &items, reference);
        assert_eq!(ids(&result), vec![items[0].id]);
    }

    #[test]
    fn resolve_view_basic_sections_and_unmatched() {
        let reference = day(2026, 2, 11);
        let work = Uuid::new_v4();
        let urgent = Uuid::new_v4();
        let normal = Uuid::new_v4();

        let items = vec![
            item_with_assignments("work urgent", None, None, &[work, urgent]),
            item_with_assignments("work normal", None, None, &[work, normal]),
            item_with_assignments("work only", None, None, &[work]),
            item_with_assignments("urgent only", None, None, &[urgent]),
        ];

        let view = view(
            include_query(work),
            vec![
                section("Urgent", include_query(urgent)),
                section("Normal", include_query(normal)),
            ],
            true,
            "Unassigned",
        );

        let result = resolve_view(&view, &items, &[], reference);

        assert_eq!(result.sections.len(), 2);
        assert_eq!(result.sections[0].section_index, 0);
        assert_eq!(result.sections[0].title, "Urgent");
        assert_eq!(item_ids(&result.sections[0].items), vec![items[0].id]);
        assert_eq!(result.sections[1].section_index, 1);
        assert_eq!(result.sections[1].title, "Normal");
        assert_eq!(item_ids(&result.sections[1].items), vec![items[1].id]);
        assert_eq!(
            item_ids(result.unmatched.as_ref().expect("unmatched")),
            vec![items[2].id]
        );
        assert_eq!(result.unmatched_label.as_deref(), Some("Unassigned"));
    }

    #[test]
    fn resolve_view_empty_criteria_matches_all_items() {
        let reference = day(2026, 2, 11);
        let work = Uuid::new_v4();
        let items = vec![
            item_with_assignments("work", None, None, &[work]),
            item_with_assignments("personal", None, None, &[]),
        ];

        let view = view(
            Query::default(),
            vec![section("Work", include_query(work))],
            true,
            "Unassigned",
        );

        let result = resolve_view(&view, &items, &[], reference);

        assert_eq!(item_ids(&result.sections[0].items), vec![items[0].id]);
        assert_eq!(
            item_ids(result.unmatched.as_ref().expect("unmatched")),
            vec![items[1].id]
        );
    }

    #[test]
    fn resolve_view_item_can_appear_in_multiple_sections() {
        let reference = day(2026, 2, 11);
        let work = Uuid::new_v4();
        let urgent = Uuid::new_v4();
        let important = Uuid::new_v4();

        let items = vec![item_with_assignments(
            "work urgent important",
            None,
            None,
            &[work, urgent, important],
        )];

        let view = view(
            include_query(work),
            vec![
                section("Urgent", include_query(urgent)),
                section("Important", include_query(important)),
            ],
            true,
            "Unassigned",
        );

        let result = resolve_view(&view, &items, &[], reference);

        assert_eq!(item_ids(&result.sections[0].items), vec![items[0].id]);
        assert_eq!(item_ids(&result.sections[1].items), vec![items[0].id]);
    }

    #[test]
    fn resolve_view_include_and_exclude_matches_miguel_without_alice_workflow() {
        let reference = day(2026, 2, 16);
        let work = Uuid::new_v4();
        let miguel = Uuid::new_v4();
        let alice = Uuid::new_v4();
        let project_atlas = Uuid::new_v4();
        let project_delta = Uuid::new_v4();
        let high = Uuid::new_v4();
        let medium = Uuid::new_v4();
        let priority = Uuid::new_v4();

        let items = vec![
            item_with_assignments(
                "Project Atlas: Miguel and Alice triage defects",
                None,
                None,
                &[work, miguel, alice, project_atlas, high, priority],
            ),
            item_with_assignments(
                "Project Delta: Miguel close open QA defects",
                None,
                None,
                &[work, miguel, project_delta, medium, priority],
            ),
            item_with_assignments(
                "Project Atlas: Miguel draft rollout checklist",
                None,
                None,
                &[work, miguel, project_atlas, high, priority],
            ),
        ];

        let mut criteria = Query::default();
        criteria.set_criterion(CriterionMode::And, work);
        criteria.set_criterion(CriterionMode::And, miguel);
        criteria.set_criterion(CriterionMode::Not, alice);
        let view = view(criteria, vec![], true, "Unassigned");

        let result = resolve_view(&view, &items, &[], reference);
        assert!(result.sections.is_empty());
        assert_eq!(
            item_ids(result.unmatched.as_ref().expect("unmatched items")),
            vec![items[1].id, items[2].id]
        );
    }

    #[test]
    fn resolve_view_include_and_exclude_matches_atlas_high_not_sarah_workflow() {
        let reference = day(2026, 2, 16);
        let project_atlas = Uuid::new_v4();
        let high = Uuid::new_v4();
        let priority = Uuid::new_v4();
        let sarah = Uuid::new_v4();
        let miguel = Uuid::new_v4();
        let alice = Uuid::new_v4();

        let items = vec![
            item_with_assignments(
                "Project Atlas: Sarah high-priority production hotfix",
                None,
                None,
                &[project_atlas, sarah, high, priority],
            ),
            item_with_assignments(
                "Project Atlas: Miguel and Alice triage defects",
                None,
                None,
                &[project_atlas, miguel, alice, high, priority],
            ),
            item_with_assignments(
                "Project Atlas: Miguel draft rollout checklist",
                None,
                None,
                &[project_atlas, miguel, high, priority],
            ),
        ];

        let mut criteria = Query::default();
        criteria.set_criterion(CriterionMode::And, project_atlas);
        criteria.set_criterion(CriterionMode::And, high);
        criteria.set_criterion(CriterionMode::Not, sarah);
        let view = view(criteria, vec![], true, "Unassigned");

        let result = resolve_view(&view, &items, &[], reference);
        assert!(result.sections.is_empty());
        assert_eq!(
            item_ids(result.unmatched.as_ref().expect("unmatched items")),
            vec![items[1].id, items[2].id]
        );
    }

    #[test]
    fn resolve_view_include_and_exclude_can_intentionally_result_in_empty_set() {
        let reference = day(2026, 2, 16);
        let high = Uuid::new_v4();
        let priority = Uuid::new_v4();
        let project_cicada = Uuid::new_v4();
        let priya = Uuid::new_v4();

        let items = vec![
            item_with_assignments("Clean out the garage", None, None, &[high, priority]),
            item_with_assignments(
                "Project Atlas: Miguel draft rollout checklist",
                None,
                None,
                &[high, priority],
            ),
            item_with_assignments(
                "Project Cicada: Sarah and Priya incident rehearsal",
                None,
                None,
                &[project_cicada, priya],
            ),
        ];

        let mut high_without_priority = Query::default();
        high_without_priority.set_criterion(CriterionMode::And, high);
        high_without_priority.set_criterion(CriterionMode::Not, priority);
        let high_without_priority_view = view(high_without_priority, vec![], true, "Unassigned");
        let high_without_priority_result =
            resolve_view(&high_without_priority_view, &items, &[], reference);

        assert!(high_without_priority_result.sections.is_empty());
        assert!(high_without_priority_result
            .unmatched
            .as_ref()
            .expect("unmatched")
            .is_empty());

        let mut cicada_without_priya = Query::default();
        cicada_without_priya.set_criterion(CriterionMode::And, project_cicada);
        cicada_without_priya.set_criterion(CriterionMode::Not, priya);
        let cicada_without_priya_view = view(cicada_without_priya, vec![], true, "Unassigned");
        let cicada_without_priya_result =
            resolve_view(&cicada_without_priya_view, &items, &[], reference);

        assert!(cicada_without_priya_result.sections.is_empty());
        assert!(cicada_without_priya_result
            .unmatched
            .as_ref()
            .expect("unmatched")
            .is_empty());
    }

    #[test]
    fn resolve_view_collects_unmatched_with_custom_label() {
        let reference = day(2026, 2, 11);
        let work = Uuid::new_v4();
        let urgent = Uuid::new_v4();
        let items = vec![item_with_assignments("work only", None, None, &[work])];

        let view = view(
            include_query(work),
            vec![section("Urgent", include_query(urgent))],
            true,
            "Other",
        );

        let result = resolve_view(&view, &items, &[], reference);

        assert_eq!(
            item_ids(result.unmatched.as_ref().expect("unmatched")),
            vec![items[0].id]
        );
        assert_eq!(result.unmatched_label.as_deref(), Some("Other"));
    }

    #[test]
    fn resolve_view_omits_unmatched_when_disabled() {
        let reference = day(2026, 2, 11);
        let work = Uuid::new_v4();
        let urgent = Uuid::new_v4();
        let items = vec![item_with_assignments("work only", None, None, &[work])];

        let view = view(
            include_query(work),
            vec![section("Urgent", include_query(urgent))],
            false,
            "Unused",
        );

        let result = resolve_view(&view, &items, &[], reference);

        assert!(result.unmatched.is_none());
        assert!(result.unmatched_label.is_none());
    }

    #[test]
    fn resolve_view_items_in_sections_do_not_appear_in_unmatched() {
        let reference = day(2026, 2, 11);
        let work = Uuid::new_v4();
        let urgent = Uuid::new_v4();
        let items = vec![item_with_assignments(
            "work urgent",
            None,
            None,
            &[work, urgent],
        )];

        let view = view(
            include_query(work),
            vec![section("Urgent", include_query(urgent))],
            true,
            "Unassigned",
        );

        let result = resolve_view(&view, &items, &[], reference);

        assert_eq!(item_ids(&result.sections[0].items), vec![items[0].id]);
        assert!(item_ids(result.unmatched.as_ref().expect("unmatched")).is_empty());
    }

    #[test]
    fn resolve_view_preserves_section_order_and_indexes() {
        let reference = day(2026, 2, 11);
        let work = Uuid::new_v4();
        let alpha = Uuid::new_v4();
        let beta = Uuid::new_v4();
        let items = vec![item_with_assignments(
            "work",
            None,
            None,
            &[work, alpha, beta],
        )];

        let view = view(
            include_query(work),
            vec![
                section("Beta First", include_query(beta)),
                section("Alpha Second", include_query(alpha)),
            ],
            true,
            "Unassigned",
        );

        let result = resolve_view(&view, &items, &[], reference);

        assert_eq!(result.sections[0].title, "Beta First");
        assert_eq!(result.sections[0].section_index, 0);
        assert_eq!(result.sections[1].title, "Alpha Second");
        assert_eq!(result.sections[1].section_index, 1);
    }

    #[test]
    fn resolve_view_empty_view_results_in_empty_groups() {
        let reference = day(2026, 2, 11);
        let work = Uuid::new_v4();
        let urgent = Uuid::new_v4();
        let items = vec![item_with_assignments("personal", None, None, &[urgent])];

        let view = view(
            include_query(work),
            vec![
                section("Urgent", include_query(urgent)),
                section("Normal", Query::default()),
            ],
            true,
            "Unassigned",
        );

        let result = resolve_view(&view, &items, &[], reference);

        assert!(item_ids(&result.sections[0].items).is_empty());
        assert!(item_ids(&result.sections[1].items).is_empty());
        assert!(item_ids(result.unmatched.as_ref().expect("unmatched")).is_empty());
    }

    #[test]
    fn resolve_view_supports_text_search_in_view_criteria() {
        let reference = day(2026, 2, 11);
        let work = Uuid::new_v4();
        let personal = Uuid::new_v4();
        let items = vec![
            item_with_assignments("Write report", None, None, &[work]),
            item_with_assignments("Plan trip", None, None, &[work]),
            item_with_assignments("Report receipts", None, None, &[personal]),
        ];

        let criteria = Query {
            text_search: Some("report".to_string()),
            ..Query::default()
        };

        let view = view(
            criteria,
            vec![section("Work", include_query(work))],
            true,
            "Unassigned",
        );

        let result = resolve_view(&view, &items, &[], reference);

        assert_eq!(item_ids(&result.sections[0].items), vec![items[0].id]);
        assert_eq!(
            item_ids(result.unmatched.as_ref().expect("unmatched")),
            vec![items[2].id]
        );
    }

    #[test]
    fn resolve_view_supports_virtual_include_in_view_criteria() {
        let reference = day(2026, 2, 11);
        let work = Uuid::new_v4();
        let personal = Uuid::new_v4();
        let items = vec![
            item_with_assignments(
                "work today",
                None,
                Some(datetime(2026, 2, 11, 9, 0)),
                &[work],
            ),
            item_with_assignments(
                "personal today",
                None,
                Some(datetime(2026, 2, 11, 10, 0)),
                &[personal],
            ),
            item_with_assignments(
                "work tomorrow",
                None,
                Some(datetime(2026, 2, 12, 9, 0)),
                &[work],
            ),
        ];

        let mut criteria = Query::default();
        criteria.virtual_include.insert(WhenBucket::Today);

        let view = view(
            criteria,
            vec![section("Work", include_query(work))],
            true,
            "Unassigned",
        );

        let result = resolve_view(&view, &items, &[], reference);

        assert_eq!(item_ids(&result.sections[0].items), vec![items[0].id]);
        assert_eq!(
            item_ids(result.unmatched.as_ref().expect("unmatched")),
            vec![items[1].id]
        );
    }

    #[test]
    fn resolve_view_expands_show_children_with_ordered_child_subsections_and_parent_other() {
        let reference = day(2026, 2, 11);
        let parent = Uuid::new_v4();
        let child_alpha = Uuid::new_v4();
        let child_beta = Uuid::new_v4();

        let categories = vec![
            category(parent, "Projects", None, &[child_alpha, child_beta]),
            category(child_alpha, "Project Alpha", Some(parent), &[]),
            category(child_beta, "Project Beta", Some(parent), &[]),
        ];
        let items = vec![
            item_with_assignments("alpha task", None, None, &[parent, child_alpha]),
            item_with_assignments("beta task", None, None, &[parent, child_beta]),
            item_with_assignments("parent only", None, None, &[parent]),
        ];

        let mut projects = section("Projects", include_query(parent));
        projects.show_children = true;
        let view = view(Query::default(), vec![projects], true, "View Unmatched");

        let result = resolve_view(&view, &items, &categories, reference);
        let section_result = &result.sections[0];

        assert!(section_result.items.is_empty());
        assert_eq!(section_result.subsections.len(), 3);
        assert_eq!(section_result.subsections[0].title, "Project Alpha");
        assert_eq!(
            item_ids(&section_result.subsections[0].items),
            vec![items[0].id]
        );
        assert_eq!(section_result.subsections[1].title, "Project Beta");
        assert_eq!(
            item_ids(&section_result.subsections[1].items),
            vec![items[1].id]
        );
        assert_eq!(section_result.subsections[2].title, "Projects (Other)");
        assert_eq!(
            item_ids(&section_result.subsections[2].items),
            vec![items[2].id]
        );

        assert!(item_ids(result.unmatched.as_ref().expect("unmatched")).is_empty());
    }

    #[test]
    fn resolve_view_show_children_preserves_child_order() {
        let reference = day(2026, 2, 11);
        let parent = Uuid::new_v4();
        let child_alpha = Uuid::new_v4();
        let child_beta = Uuid::new_v4();

        let categories = vec![
            category(parent, "Projects", None, &[child_beta, child_alpha]),
            category(child_alpha, "Project Alpha", Some(parent), &[]),
            category(child_beta, "Project Beta", Some(parent), &[]),
        ];
        let items = vec![item_with_assignments(
            "shared task",
            None,
            None,
            &[parent, child_alpha, child_beta],
        )];

        let mut projects = section("Projects", include_query(parent));
        projects.show_children = true;
        let view = view(Query::default(), vec![projects], true, "View Unmatched");

        let result = resolve_view(&view, &items, &categories, reference);
        let subsections = &result.sections[0].subsections;
        assert_eq!(subsections[0].title, "Project Beta");
        assert_eq!(subsections[1].title, "Project Alpha");
        assert_eq!(subsections[2].title, "Projects (Other)");
    }

    #[test]
    fn resolve_view_show_children_does_not_expand_when_disabled_or_complex() {
        let reference = day(2026, 2, 11);
        let parent = Uuid::new_v4();
        let child_alpha = Uuid::new_v4();
        let child_beta = Uuid::new_v4();
        let categories = vec![
            category(parent, "Projects", None, &[child_alpha, child_beta]),
            category(child_alpha, "Project Alpha", Some(parent), &[]),
            category(child_beta, "Project Beta", Some(parent), &[]),
        ];
        let items = vec![item_with_assignments(
            "task",
            None,
            None,
            &[parent, child_alpha],
        )];

        let mut disabled = section("Disabled", include_query(parent));
        disabled.show_children = false;
        let disabled_view = view(Query::default(), vec![disabled], true, "View Unmatched");
        let disabled_result = resolve_view(&disabled_view, &items, &categories, reference);
        assert!(disabled_result.sections[0].subsections.is_empty());
        assert_eq!(
            item_ids(&disabled_result.sections[0].items),
            vec![items[0].id]
        );

        let mut complex_query = Query::default();
        complex_query.set_criterion(CriterionMode::And, parent);
        complex_query.set_criterion(CriterionMode::And, child_alpha);
        let mut complex = section("Complex", complex_query);
        complex.show_children = true;
        let complex_view = view(Query::default(), vec![complex], true, "View Unmatched");
        let complex_result = resolve_view(&complex_view, &items, &categories, reference);
        assert!(complex_result.sections[0].subsections.is_empty());
        assert_eq!(
            item_ids(&complex_result.sections[0].items),
            vec![items[0].id]
        );
    }

    #[test]
    fn resolve_view_show_children_one_level_only_no_grandchild_subsections() {
        let reference = day(2026, 2, 11);
        let parent = Uuid::new_v4();
        let child = Uuid::new_v4();
        let grandchild = Uuid::new_v4();

        let categories = vec![
            category(parent, "Projects", None, &[child]),
            category(child, "Project Alpha", Some(parent), &[grandchild]),
            category(grandchild, "Alpha Backend", Some(child), &[]),
        ];
        let items = vec![item_with_assignments(
            "grandchild task",
            None,
            None,
            &[parent, child, grandchild],
        )];

        let mut projects = section("Projects", include_query(parent));
        projects.show_children = true;
        let view = view(Query::default(), vec![projects], true, "View Unmatched");

        let result = resolve_view(&view, &items, &categories, reference);
        let subsections = &result.sections[0].subsections;
        assert_eq!(subsections.len(), 2);
        assert_eq!(subsections[0].title, "Project Alpha");
        assert_eq!(subsections[1].title, "Projects (Other)");
    }

    #[test]
    fn resolve_view_show_children_with_no_children_keeps_base_section() {
        let reference = day(2026, 2, 11);
        let parent = Uuid::new_v4();
        let categories = vec![category(parent, "Projects", None, &[])];
        let items = vec![
            item_with_assignments("project one", None, None, &[parent]),
            item_with_assignments("project two", None, None, &[parent]),
        ];

        let mut projects = section("Projects", include_query(parent));
        projects.show_children = true;
        let view = view(Query::default(), vec![projects], true, "View Unmatched");

        let result = resolve_view(&view, &items, &categories, reference);
        let section_result = &result.sections[0];
        assert_eq!(
            item_ids(&section_result.items),
            vec![items[0].id, items[1].id]
        );
        assert!(
            section_result.subsections.is_empty(),
            "no child categories means no generated subsections"
        );
    }

    #[test]
    fn resolve_view_show_children_item_can_appear_in_multiple_child_subsections() {
        let reference = day(2026, 2, 11);
        let parent = Uuid::new_v4();
        let child_alpha = Uuid::new_v4();
        let child_beta = Uuid::new_v4();
        let categories = vec![
            category(parent, "Projects", None, &[child_alpha, child_beta]),
            category(child_alpha, "Project Alpha", Some(parent), &[]),
            category(child_beta, "Project Beta", Some(parent), &[]),
        ];
        let items = vec![item_with_assignments(
            "cross-cutting task",
            None,
            None,
            &[parent, child_alpha, child_beta],
        )];

        let mut projects = section("Projects", include_query(parent));
        projects.show_children = true;
        let view = view(Query::default(), vec![projects], true, "View Unmatched");

        let result = resolve_view(&view, &items, &categories, reference);
        let subsections = &result.sections[0].subsections;
        assert_eq!(item_ids(&subsections[0].items), vec![items[0].id]);
        assert_eq!(item_ids(&subsections[1].items), vec![items[0].id]);
    }

    #[test]
    fn resolve_view_show_children_subsections_include_effective_edit_through_sets() {
        let reference = day(2026, 2, 11);
        let parent = Uuid::new_v4();
        let child_alpha = Uuid::new_v4();
        let categories = vec![
            category(parent, "Projects", None, &[child_alpha]),
            category(child_alpha, "Project Alpha", Some(parent), &[]),
        ];
        let items = vec![
            item_with_assignments("alpha task", None, None, &[parent, child_alpha]),
            item_with_assignments("other task", None, None, &[parent]),
        ];
        let marker_insert = Uuid::new_v4();
        let marker_remove = Uuid::new_v4();

        let mut projects = section("Projects", include_query(parent));
        projects.show_children = true;
        projects.on_insert_assign.insert(marker_insert);
        projects.on_remove_unassign.insert(marker_remove);
        let view = view(Query::default(), vec![projects], true, "View Unmatched");

        let result = resolve_view(&view, &items, &categories, reference);
        let child_subsection = &result.sections[0].subsections[0];
        let other_subsection = &result.sections[0].subsections[1];

        assert!(child_subsection.on_insert_assign.contains(&marker_insert));
        assert!(child_subsection.on_insert_assign.contains(&child_alpha));
        assert_eq!(
            child_subsection.on_remove_unassign,
            HashSet::from([marker_remove])
        );
        assert_eq!(other_subsection.title, "Projects (Other)");
        assert_eq!(
            other_subsection.on_insert_assign,
            HashSet::from([marker_insert])
        );
        assert_eq!(
            other_subsection.on_remove_unassign,
            HashSet::from([marker_remove])
        );
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
        assert_eq!(
            resolve_when_bucket(when_date, reference),
            WhenBucket::Tomorrow
        );
    }

    #[test]
    fn resolve_this_week_bucket() {
        let reference = day(2026, 2, 11);
        let when_date = Some(datetime(2026, 2, 14, 9, 0));
        assert_eq!(
            resolve_when_bucket(when_date, reference),
            WhenBucket::ThisWeek
        );
    }

    #[test]
    fn resolve_next_week_bucket() {
        let reference = day(2026, 2, 11);
        let when_date = Some(datetime(2026, 2, 16, 9, 0));
        assert_eq!(
            resolve_when_bucket(when_date, reference),
            WhenBucket::NextWeek
        );
    }

    #[test]
    fn resolve_this_month_bucket() {
        let reference = day(2026, 2, 11);
        let when_date = Some(datetime(2026, 2, 27, 9, 0));
        assert_eq!(
            resolve_when_bucket(when_date, reference),
            WhenBucket::ThisMonth
        );
    }

    #[test]
    fn resolve_future_bucket() {
        let reference = day(2026, 2, 11);
        let when_date = Some(datetime(2026, 3, 15, 9, 0));
        assert_eq!(
            resolve_when_bucket(when_date, reference),
            WhenBucket::Future
        );
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
        assert_eq!(
            resolve_when_bucket(when_date, reference),
            WhenBucket::Tomorrow
        );
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
        assert_eq!(
            resolve_when_bucket(when_date, reference),
            WhenBucket::Future
        );
    }

    #[test]
    fn week_boundary_saturday_to_sunday_and_monday() {
        let reference = day(2026, 2, 14); // Saturday
        let sunday = Some(datetime(2026, 2, 15, 9, 0));
        let monday = Some(datetime(2026, 2, 16, 9, 0));

        assert_eq!(resolve_when_bucket(sunday, reference), WhenBucket::Tomorrow);
        assert_eq!(resolve_when_bucket(monday, reference), WhenBucket::NextWeek);
    }

    // ── bucket_contains tests ──

    #[test]
    fn bucket_contains_today_overlaps_this_year_and_this_month() {
        let reference = day(2026, 2, 11);
        let today = Some(datetime(2026, 2, 11, 9, 0));
        assert!(bucket_contains(WhenBucket::Today, today, reference));
        assert!(bucket_contains(WhenBucket::ThisWeek, today, reference));
        assert!(bucket_contains(WhenBucket::ThisMonth, today, reference));
        assert!(bucket_contains(WhenBucket::ThisYear, today, reference));
        assert!(bucket_contains(WhenBucket::Next12Months, today, reference));
        assert!(!bucket_contains(WhenBucket::Tomorrow, today, reference));
        assert!(!bucket_contains(WhenBucket::Future, today, reference));
        assert!(!bucket_contains(WhenBucket::NoDate, today, reference));
    }

    #[test]
    fn bucket_contains_next_month_handles_year_rollover() {
        let reference = day(2026, 12, 15);
        let jan_next = Some(datetime(2027, 1, 5, 9, 0));
        assert!(bucket_contains(WhenBucket::NextMonth, jan_next, reference));
        assert!(!bucket_contains(WhenBucket::ThisYear, jan_next, reference));
    }

    #[test]
    fn bucket_contains_next_12_months_excludes_past() {
        let reference = day(2026, 2, 11);
        let yesterday = Some(datetime(2026, 2, 10, 9, 0));
        let in_six_months = Some(datetime(2026, 8, 11, 9, 0));
        let in_two_years = Some(datetime(2028, 2, 11, 9, 0));
        assert!(!bucket_contains(
            WhenBucket::Next12Months,
            yesterday,
            reference
        ));
        assert!(bucket_contains(
            WhenBucket::Next12Months,
            in_six_months,
            reference
        ));
        assert!(!bucket_contains(
            WhenBucket::Next12Months,
            in_two_years,
            reference
        ));
    }

    #[test]
    fn bucket_contains_no_date_only_matches_no_date() {
        let reference = day(2026, 2, 11);
        let today = Some(datetime(2026, 2, 11, 9, 0));
        assert!(bucket_contains(WhenBucket::NoDate, None, reference));
        assert!(!bucket_contains(WhenBucket::NoDate, today, reference));
        assert!(!bucket_contains(WhenBucket::Today, None, reference));
    }

    // ── matches_text_search tests ──

    #[test]
    fn text_search_matches_item_text() {
        let item = item_with_assignments("Buy groceries", None, None, &[]);
        assert!(matches_text_search(&item, "groceries", None));
        assert!(!matches_text_search(&item, "meeting", None));
    }

    #[test]
    fn text_search_case_insensitive() {
        let item = item_with_assignments("URGENT task", None, None, &[]);
        assert!(matches_text_search(&item, "urgent", None));
    }

    #[test]
    fn text_search_matches_note() {
        let item = item_with_assignments("Title", Some("Discuss roadmap"), None, &[]);
        assert!(matches_text_search(&item, "roadmap", None));
        assert!(!matches_text_search(&item, "missing", None));
    }

    #[test]
    fn text_search_matches_uuid_prefix() {
        let mut item = item_with_assignments("target", None, None, &[]);
        item.id = Uuid::parse_str("123e4567-e89b-12d3-a456-426614174000").unwrap();
        assert!(matches_text_search(&item, "123e4567", None));
    }

    #[test]
    fn text_search_matches_compact_uuid() {
        let mut item = item_with_assignments("target", None, None, &[]);
        item.id = Uuid::parse_str("123e4567-e89b-12d3-a456-426614174000").unwrap();
        assert!(matches_text_search(&item, "123e4567e89b", None));
    }

    #[test]
    fn text_search_short_hex_not_treated_as_uuid() {
        let mut item = item_with_assignments("target", None, None, &[]);
        item.id = Uuid::parse_str("ab000000-0000-0000-0000-000000000000").unwrap();
        // "ab" is only 2 hex chars, below the 3-char threshold
        assert!(!matches_text_search(&item, "ab", None));
    }

    #[test]
    fn text_search_matches_category_name() {
        let cat_id = Uuid::new_v4();
        let item = item_with_assignments("Some task", None, None, &[cat_id]);
        let mut names = HashMap::new();
        names.insert(cat_id, "priority".to_string());
        assert!(matches_text_search(&item, "prio", Some(&names)));
    }

    #[test]
    fn text_search_skips_categories_when_none() {
        let cat_id = Uuid::new_v4();
        let item = item_with_assignments("Some task", None, None, &[cat_id]);
        // Without category names, "priority" should not match
        assert!(!matches_text_search(&item, "priority", None));
    }

    #[test]
    fn text_search_no_match_returns_false() {
        let item = item_with_assignments("alpha", None, None, &[]);
        assert!(!matches_text_search(&item, "zzz", None));
    }

    // ── Datebook tests ──────────────────────────────────────────────

    use super::{compute_datebook_window, extract_item_date, generate_datebook_sections};
    use crate::model::{
        DateSource, DatebookAnchor, DatebookConfig, DatebookInterval, DatebookPeriod,
    };

    fn default_datebook_config() -> DatebookConfig {
        DatebookConfig::default()
    }

    #[test]
    fn datebook_config_validation() {
        let mut c = default_datebook_config();
        // Week + Daily is valid
        assert!(c.is_valid());

        // Week + Monthly is invalid (too coarse)
        c.interval = DatebookInterval::Monthly;
        assert!(!c.is_valid());

        // Day + Hourly is valid
        c.period = DatebookPeriod::Day;
        c.interval = DatebookInterval::Hourly;
        assert!(c.is_valid());

        // Day + Daily is invalid
        c.interval = DatebookInterval::Daily;
        assert!(!c.is_valid());

        // Quarter + Monthly is valid
        c.period = DatebookPeriod::Quarter;
        c.interval = DatebookInterval::Monthly;
        assert!(c.is_valid());

        // Quarter + Daily is invalid
        c.interval = DatebookInterval::Daily;
        assert!(!c.is_valid());

        // Year + Weekly is valid
        c.period = DatebookPeriod::Year;
        c.interval = DatebookInterval::Weekly;
        assert!(c.is_valid());
    }

    #[test]
    fn datebook_week_daily_generates_7_sections() {
        let config = DatebookConfig {
            period: DatebookPeriod::Week,
            interval: DatebookInterval::Daily,
            anchor: DatebookAnchor::StartOfWeek,
            date_source: DateSource::When,
            browse_offset: 0,
            ..Default::default()
        };
        // 2026-04-06 is a Monday
        let reference = day(2026, 4, 6);
        let sections = generate_datebook_sections(&config, reference);
        assert_eq!(sections.len(), 7);
        assert_eq!(sections[0].range_start, datetime(2026, 4, 6, 0, 0));
        assert_eq!(sections[0].range_end, datetime(2026, 4, 7, 0, 0));
        assert_eq!(sections[6].range_start, datetime(2026, 4, 12, 0, 0));
        assert_eq!(sections[6].range_end, datetime(2026, 4, 13, 0, 0));
        // Title format: "Mon, Apr 6"
        assert!(sections[0].title.contains("Mon"));
        assert!(sections[0].title.contains("Apr"));
    }

    #[test]
    fn datebook_month_weekly_generates_sections() {
        let config = DatebookConfig {
            period: DatebookPeriod::Month,
            interval: DatebookInterval::Weekly,
            anchor: DatebookAnchor::StartOfMonth,
            date_source: DateSource::When,
            browse_offset: 0,
            ..Default::default()
        };
        // April 2026
        let reference = day(2026, 4, 15);
        let sections = generate_datebook_sections(&config, reference);
        // April 1 to May 1 = ~4-5 weekly sections
        assert!(sections.len() >= 4 && sections.len() <= 5);
        assert_eq!(sections[0].range_start, datetime(2026, 4, 1, 0, 0));
    }

    #[test]
    fn datebook_quarter_monthly_generates_3_sections() {
        let config = DatebookConfig {
            period: DatebookPeriod::Quarter,
            interval: DatebookInterval::Monthly,
            anchor: DatebookAnchor::StartOfQuarter,
            date_source: DateSource::When,
            browse_offset: 0,
            ..Default::default()
        };
        // Q2 2026
        let reference = day(2026, 5, 10);
        let sections = generate_datebook_sections(&config, reference);
        assert_eq!(sections.len(), 3);
        assert_eq!(sections[0].range_start, datetime(2026, 4, 1, 0, 0));
        assert_eq!(sections[1].range_start, datetime(2026, 5, 1, 0, 0));
        assert_eq!(sections[2].range_start, datetime(2026, 6, 1, 0, 0));
        assert!(sections[0].title.contains("April"));
        assert!(sections[1].title.contains("May"));
        assert!(sections[2].title.contains("June"));
    }

    #[test]
    fn datebook_day_hourly_generates_24_sections() {
        let config = DatebookConfig {
            period: DatebookPeriod::Day,
            interval: DatebookInterval::Hourly,
            anchor: DatebookAnchor::Today,
            date_source: DateSource::When,
            browse_offset: 0,
            ..Default::default()
        };
        let reference = day(2026, 4, 6);
        let sections = generate_datebook_sections(&config, reference);
        assert_eq!(sections.len(), 24);
        assert_eq!(sections[0].range_start, datetime(2026, 4, 6, 0, 0));
        assert_eq!(sections[9].range_start, datetime(2026, 4, 6, 9, 0));
        assert_eq!(sections[23].range_start, datetime(2026, 4, 6, 23, 0));
    }

    #[test]
    fn datebook_browse_offset_shifts_window() {
        let mut config = DatebookConfig {
            period: DatebookPeriod::Week,
            interval: DatebookInterval::Daily,
            anchor: DatebookAnchor::StartOfWeek,
            date_source: DateSource::When,
            browse_offset: 0,
            ..Default::default()
        };
        let reference = day(2026, 4, 6); // Monday

        // No offset: Apr 6 - Apr 12
        let (start, end) = compute_datebook_window(&config, reference);
        assert_eq!(start, datetime(2026, 4, 6, 0, 0));
        assert_eq!(end, datetime(2026, 4, 13, 0, 0));

        // +1 week forward: Apr 13 - Apr 19
        config.browse_offset = 1;
        let (start, end) = compute_datebook_window(&config, reference);
        assert_eq!(start, datetime(2026, 4, 13, 0, 0));
        assert_eq!(end, datetime(2026, 4, 20, 0, 0));

        // -1 week backward: Mar 30 - Apr 5
        config.browse_offset = -1;
        let (start, end) = compute_datebook_window(&config, reference);
        assert_eq!(start, datetime(2026, 3, 30, 0, 0));
        assert_eq!(end, datetime(2026, 4, 6, 0, 0));
    }

    #[test]
    fn datebook_resolve_buckets_items() {
        let config = DatebookConfig {
            period: DatebookPeriod::Week,
            interval: DatebookInterval::Daily,
            anchor: DatebookAnchor::StartOfWeek,
            date_source: DateSource::When,
            browse_offset: 0,
            ..Default::default()
        };
        let mut view = View::new("Week".to_string());
        view.datebook_config = Some(config);

        let reference = day(2026, 4, 6); // Monday

        let items = vec![
            item_with_assignments("Monday task", None, Some(datetime(2026, 4, 6, 10, 0)), &[]),
            item_with_assignments(
                "Wednesday task",
                None,
                Some(datetime(2026, 4, 8, 14, 30)),
                &[],
            ),
            item_with_assignments("No date task", None, None, &[]),
            item_with_assignments(
                "Outside window",
                None,
                Some(datetime(2026, 4, 20, 9, 0)),
                &[],
            ),
        ];

        let result = resolve_view(&view, &items, &[], reference);

        // 7 sections (one per day)
        assert_eq!(result.sections.len(), 7);

        // Monday task in section 0
        assert_eq!(result.sections[0].items.len(), 1);
        assert_eq!(result.sections[0].items[0].text, "Monday task");

        // Wednesday task in section 2
        assert_eq!(result.sections[2].items.len(), 1);
        assert_eq!(result.sections[2].items[0].text, "Wednesday task");

        // Unmatched: "No date task" + "Outside window"
        let unmatched = result.unmatched.as_ref().unwrap();
        assert_eq!(unmatched.len(), 2);
    }

    #[test]
    fn datebook_boundary_item_goes_to_later_section() {
        let config = DatebookConfig {
            period: DatebookPeriod::Week,
            interval: DatebookInterval::Daily,
            anchor: DatebookAnchor::StartOfWeek,
            date_source: DateSource::When,
            browse_offset: 0,
            ..Default::default()
        };
        let mut view = View::new("Week".to_string());
        view.datebook_config = Some(config);
        let reference = day(2026, 4, 6);

        // Item exactly at midnight Tuesday = start of section 1 (inclusive)
        let items = vec![item_with_assignments(
            "Midnight Tuesday",
            None,
            Some(datetime(2026, 4, 7, 0, 0)),
            &[],
        )];

        let result = resolve_view(&view, &items, &[], reference);
        assert_eq!(result.sections[1].items.len(), 1); // Tuesday section
        assert_eq!(result.sections[0].items.len(), 0); // Not Monday
    }

    #[test]
    fn datebook_year_monthly_sections() {
        let config = DatebookConfig {
            period: DatebookPeriod::Year,
            interval: DatebookInterval::Monthly,
            anchor: DatebookAnchor::StartOfYear,
            date_source: DateSource::When,
            browse_offset: 0,
            ..Default::default()
        };
        let reference = day(2026, 6, 15);
        let sections = generate_datebook_sections(&config, reference);
        assert_eq!(sections.len(), 12);
        assert!(sections[0].title.contains("January"));
        assert!(sections[11].title.contains("December"));
    }

    #[test]
    fn datebook_anchor_start_of_quarter() {
        let config = DatebookConfig {
            period: DatebookPeriod::Quarter,
            interval: DatebookInterval::Monthly,
            anchor: DatebookAnchor::StartOfQuarter,
            date_source: DateSource::When,
            browse_offset: 0,
            ..Default::default()
        };
        // January -> Q1 starts Jan 1
        let (start, _) = compute_datebook_window(&config, day(2026, 1, 15));
        assert_eq!(start, datetime(2026, 1, 1, 0, 0));

        // March -> still Q1
        let (start, _) = compute_datebook_window(&config, day(2026, 3, 31));
        assert_eq!(start, datetime(2026, 1, 1, 0, 0));

        // April -> Q2
        let (start, _) = compute_datebook_window(&config, day(2026, 4, 1));
        assert_eq!(start, datetime(2026, 4, 1, 0, 0));

        // December -> Q4
        let (start, _) = compute_datebook_window(&config, day(2026, 12, 25));
        assert_eq!(start, datetime(2026, 10, 1, 0, 0));
    }

    #[test]
    fn datebook_extract_item_date_sources() {
        let mut item = Item::new("test".to_string());
        item.when_date = Some(datetime(2026, 4, 6, 10, 0));
        item.done_date = Some(datetime(2026, 4, 7, 11, 0));

        assert_eq!(
            extract_item_date(&item, DateSource::When),
            Some(datetime(2026, 4, 6, 10, 0))
        );
        assert_eq!(
            extract_item_date(&item, DateSource::Done),
            Some(datetime(2026, 4, 7, 11, 0))
        );
        // Entry source derives from created_at (UTC timestamp)
        assert!(extract_item_date(&item, DateSource::Entry).is_some());
    }

    #[test]
    fn datebook_month_end_clamping_feb() {
        // Monthly interval across months with different lengths
        let config = DatebookConfig {
            period: DatebookPeriod::Quarter,
            interval: DatebookInterval::Monthly,
            anchor: DatebookAnchor::StartOfQuarter,
            date_source: DateSource::When,
            browse_offset: 0,
            ..Default::default()
        };
        // Q1 2024 (leap year)
        let reference = day(2024, 2, 15);
        let sections = generate_datebook_sections(&config, reference);
        assert_eq!(sections.len(), 3);
        // Feb in leap year has 29 days
        assert_eq!(sections[1].range_start, datetime(2024, 2, 1, 0, 0));
        assert_eq!(sections[1].range_end, datetime(2024, 3, 1, 0, 0));
    }
}
