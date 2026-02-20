use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime};
use std::collections::{HashMap, HashSet};

use crate::model::{Category, CategoryId, Item, Query, Section, View, WhenBucket};

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
    reference_date: NaiveDate,
) -> ViewResult {
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

    if !section.criteria.exclude.is_empty()
        || !section.criteria.virtual_include.is_empty()
        || !section.criteria.virtual_exclude.is_empty()
        || section.criteria.text_search.is_some()
        || section.criteria.include.len() != 1
    {
        return None;
    }

    section.criteria.include.iter().next().copied()
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
    use std::collections::{HashMap, HashSet};

    use chrono::{NaiveDate, NaiveDateTime, Utc};
    use uuid::Uuid;

    use super::{evaluate_query, resolve_view, resolve_when_bucket};
    use crate::model::{
        Assignment, AssignmentSource, Category, CategoryId, Item, Query, Section, View, WhenBucket,
    };

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

    fn item_ids(items: &[Item]) -> Vec<Uuid> {
        items.iter().map(|item| item.id).collect()
    }

    fn include_query(category_id: CategoryId) -> Query {
        let mut query = Query::default();
        query.include.insert(category_id);
        query
    }

    fn section(title: &str, criteria: Query) -> Section {
        Section {
            title: title.to_string(),
            criteria,
            columns: Vec::new(),
            on_insert_assign: HashSet::new(),
            on_remove_unassign: HashSet::new(),
            show_children: false,
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
        query.include.insert(category_a);
        query.exclude.insert(category_b);
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
        criteria.include.extend([work, miguel]);
        criteria.exclude.insert(alice);
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
        criteria.include.extend([project_atlas, high]);
        criteria.exclude.insert(sarah);
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
        high_without_priority.include.insert(high);
        high_without_priority.exclude.insert(priority);
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
        cicada_without_priya.include.insert(project_cicada);
        cicada_without_priya.exclude.insert(priya);
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
        complex_query.include.extend([parent, child_alpha]);
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
    fn resolve_view_show_children_empty_children_has_only_parent_other() {
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
        assert!(section_result.items.is_empty());
        assert_eq!(section_result.subsections.len(), 1);
        assert_eq!(section_result.subsections[0].title, "Projects (Other)");
        assert_eq!(
            item_ids(&section_result.subsections[0].items),
            vec![items[0].id, items[1].id]
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
}
