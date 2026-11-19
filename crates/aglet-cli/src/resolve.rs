//! Shared lookups and argument parsing used by several commands.

use crate::*;

pub(crate) fn resolve_item_id(input: &str, store: &Store) -> CliResult<ItemId> {
    // Try full UUID parse first
    if let Ok(id) = ItemId::parse_str(input) {
        return Ok(id);
    }
    // Fall back to prefix resolution
    Ok(store.resolve_item_prefix(input)?)
}

pub(crate) fn print_process_warnings(result: &aglet_core::engine::ProcessItemResult) {
    for warning in &result.warnings {
        eprintln!("warning: {warning}");
    }
}

pub(crate) fn print_bulk_warnings(result: &aglet_core::engine::EvaluateAllItemsResult) {
    for warning in &result.warnings {
        eprintln!("warning: {warning}");
    }
}

pub(crate) fn describe_category_targets(
    targets: &HashSet<CategoryId>,
    category_names: &HashMap<CategoryId, String>,
) -> String {
    let mut names: Vec<String> = targets
        .iter()
        .map(|id| {
            category_names
                .get(id)
                .cloned()
                .unwrap_or_else(|| "(deleted)".to_string())
        })
        .collect();
    names.sort();
    format!("[{}]", names.join(", "))
}

pub(crate) fn describe_category_action(
    action: &Action,
    category_names: &HashMap<CategoryId, String>,
) -> String {
    match action.category_targets() {
        Some(targets) => format!(
            "{} {}",
            action.kind_label(),
            describe_category_targets(targets, category_names)
        ),
        None => action.kind_label().to_string(),
    }
}

pub(crate) fn indexed_category_action_row(
    index: usize,
    action: &Action,
    category_names: &HashMap<CategoryId, String>,
) -> String {
    format!(
        "{}. {}",
        index + 1,
        describe_category_action(action, category_names)
    )
}

pub(crate) fn category_id_by_name(categories: &[Category], name: &str) -> CliResult<CategoryId> {
    categories
        .iter()
        .find(|category| category.name.eq_ignore_ascii_case(name))
        .map(|category| category.id)
        .ok_or_else(|| format!("category not found: {name}").into())
}

pub(crate) fn names_to_category_ids(
    categories: &[Category],
    names: &[String],
) -> CliResult<HashSet<CategoryId>> {
    names
        .iter()
        .map(|name| category_id_by_name(categories, name))
        .collect()
}

pub(crate) fn view_by_name(store: &Store, name: &str) -> CliResult<View> {
    if name.eq_ignore_ascii_case(READY_QUEUE_VIEW_NAME) {
        let workflow = resolved_workflow_or_err(store)?;
        return Ok(build_ready_queue_view(store, workflow)?);
    }
    store
        .list_views()?
        .into_iter()
        .find(|view| view.name.eq_ignore_ascii_case(name))
        .ok_or_else(|| format!("view not found: {name}").into())
}

pub(crate) fn duplicate_category_create_error(
    requested_name: &str,
    requested_parent: Option<&str>,
    existing_id: Option<CategoryId>,
) -> String {
    let parent_context = requested_parent
        .map(|parent| format!(" under parent \"{parent}\""))
        .unwrap_or_default();
    let id_fragment = existing_id
        .map(|id| format!(" (existing id: {id})"))
        .unwrap_or_default();

    format!(
        "category \"{requested_name}\" already exists{id_fragment}. Category names are global across the database, so it cannot be created{parent_context}. Use `aglet category assign <item-id> \"{requested_name}\"` to assign items to the existing category."
    )
}

pub(crate) fn category_value_kind_label(kind: CategoryValueKind) -> &'static str {
    match kind {
        CategoryValueKind::Tag => "Tag",
        CategoryValueKind::Numeric => "Numeric",
    }
}

pub(crate) fn parse_decimal_value(input: &str) -> CliResult<Decimal> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("value cannot be empty".into());
    }
    let normalized = trimmed.replace(',', "");
    normalized
        .parse::<Decimal>()
        .map_err(|e| format!("invalid decimal value '{input}': {e}").into())
}

pub(crate) fn parse_when_datetime_input(input: &str) -> CliResult<DateTime> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("date/time cannot be empty".into());
    }

    if let Ok(value) = trimmed.replace(' ', "T").parse::<DateTime>() {
        return Ok(value);
    }
    if let Ok(date_only) = trimmed.parse::<Date>() {
        return Ok(date_only.at(0, 0, 0, 0));
    }

    let parser = BasicDateParser::default();
    if let Some(parsed) = parser.parse(trimmed, jiff::Zoned::now().date()) {
        return Ok(parsed.datetime);
    }

    Err(format!(
        "could not parse date/time from '{trimmed}'. Supported: today/tomorrow/yesterday, this|next <weekday>, month day[, year], YYYY-MM-DD, YYYYMMDD, M/D/YY (+ optional time like 'at 3pm')."
    ).into())
}

pub(crate) fn resolve_tag_category_assignments(
    categories: &[Category],
    names: &[String],
) -> CliResult<Vec<(CategoryId, String)>> {
    let mut assignments = Vec::new();
    let mut seen = HashSet::new();
    for name in names {
        let category_id = category_id_by_name(categories, name)?;
        if !seen.insert(category_id) {
            continue;
        }
        let category = categories
            .iter()
            .find(|category| category.id == category_id)
            .ok_or_else(|| format!("category not found: {name}"))?;
        if category.value_kind == CategoryValueKind::Numeric {
            return Err(format!(
                "category '{}' is Numeric; use --value \"{}=<number>\" instead",
                category.name, category.name
            )
            .into());
        }
        assignments.push((category.id, category.name.clone()));
    }
    Ok(assignments)
}

pub(crate) fn resolve_value_assignments(
    categories: &[Category],
    specs: &[String],
) -> CliResult<Vec<NumericValueAssignment>> {
    let mut assignments = Vec::new();
    let mut seen = HashSet::new();
    for spec in specs {
        let (category_name, raw_value) = spec
            .split_once('=')
            .ok_or_else(|| format!("invalid --value '{spec}': expected CATEGORY=NUMBER"))?;
        let category_name = category_name.trim();
        if category_name.is_empty() {
            return Err(
                format!("invalid --value '{spec}': missing category name before '='").into(),
            );
        }
        let value = parse_decimal_value(raw_value)?;
        let category_id = category_id_by_name(categories, category_name)?;
        let category = categories
            .iter()
            .find(|category| category.id == category_id)
            .ok_or_else(|| format!("category not found: {category_name}"))?;
        if category.value_kind != CategoryValueKind::Numeric {
            return Err(format!(
                "category '{}' is not Numeric; use --category \"{}\" instead",
                category.name, category.name
            )
            .into());
        }
        if !seen.insert(category_id) {
            assignments.retain(|assignment: &NumericValueAssignment| {
                assignment.category_id != category_id
            });
        }
        assignments.push(NumericValueAssignment {
            category_id,
            category_name: category.name.clone(),
            value,
        });
    }
    Ok(assignments)
}

pub(crate) fn apply_tag_category_assignment(
    aglet: &Aglet<'_>,
    item_id: ItemId,
    category_id: CategoryId,
    category_name: &str,
) -> CliResult {
    if category_name.eq_ignore_ascii_case("Done") {
        aglet.mark_item_done(item_id)?;
        return Ok(());
    }

    aglet.assign_item_manual(item_id, category_id, Some("manual:cli.assign".to_string()))?;
    Ok(())
}

pub(crate) fn apply_numeric_value_assignment(
    aglet: &Aglet<'_>,
    item_id: ItemId,
    assignment: NumericValueAssignment,
) -> CliResult {
    aglet.assign_item_numeric_manual(
        item_id,
        assignment.category_id,
        assignment.value,
        Some("manual:cli.set-value".to_string()),
    )?;
    Ok(())
}

pub(crate) fn query_from_category_names(
    categories: &[Category],
    include: &[String],
    or_include: &[String],
    exclude: &[String],
) -> CliResult<Query> {
    let mut query = Query::default();
    for category_id in names_to_category_ids(categories, include)? {
        query.set_criterion(CriterionMode::And, category_id);
    }
    for category_id in names_to_category_ids(categories, or_include)? {
        query.set_criterion(CriterionMode::Or, category_id);
    }
    for category_id in names_to_category_ids(categories, exclude)? {
        query.set_criterion(CriterionMode::Not, category_id);
    }
    Ok(query)
}

pub(crate) fn ensure_mutable_view(view: &View) -> CliResult {
    if view.name.eq_ignore_ascii_case(READY_QUEUE_VIEW_NAME)
        || view.name.eq_ignore_ascii_case(DEFAULT_VIEW_NAME)
    {
        return Err(format!("cannot modify system view: {}", view.name).into());
    }
    Ok(())
}

pub(crate) fn section_mut(view: &mut View, section_index: usize) -> CliResult<&mut Section> {
    let section_count = view.sections.len();
    view.sections.get_mut(section_index).ok_or_else(|| {
        format!(
            "section index {} out of range (view has {} sections)",
            section_index, section_count
        )
        .into()
    })
}

pub(crate) fn find_column_index(
    section: &Section,
    category_names: &HashMap<CategoryId, String>,
    column_name: &str,
) -> CliResult<usize> {
    let wanted = column_name.to_ascii_lowercase();
    section
        .columns
        .iter()
        .position(|column| {
            category_names
                .get(&column.heading)
                .is_some_and(|name| name.eq_ignore_ascii_case(&wanted))
        })
        .ok_or_else(|| format!("column '{}' not found", column_name).into())
}

pub(crate) fn parse_source_parent_mapping(
    spec: &str,
    flag_name: &str,
) -> CliResult<(String, String)> {
    let (source, parent) = spec
        .split_once('=')
        .ok_or_else(|| format!("invalid {flag_name} '{spec}': expected SOURCE=PARENT"))?;
    let source = source.trim();
    let parent = parent.trim();
    if source.is_empty() || parent.is_empty() {
        return Err(format!(
            "invalid {flag_name} '{spec}': SOURCE and PARENT must both be non-empty"
        )
        .into());
    }
    Ok((source.to_string(), parent.to_string()))
}

pub(crate) fn ensure_category_exists(
    aglet: &Aglet<'_>,
    store: &Store,
    name: &str,
    parent_name: Option<&str>,
    value_kind: CategoryValueKind,
) -> CliResult<CategoryId> {
    if parent_name.is_some_and(|parent| parent.eq_ignore_ascii_case(name)) {
        return Err(format!("category '{}' cannot use itself as a parent", name).into());
    }

    let existing_categories = store.get_hierarchy()?;
    if let Some(existing) = existing_categories
        .iter()
        .find(|category| category.name.eq_ignore_ascii_case(name))
    {
        if existing.value_kind != value_kind {
            return Err(format!(
                "category '{}' already exists with type {}; expected {}",
                existing.name,
                category_value_kind_label(existing.value_kind),
                category_value_kind_label(value_kind)
            )
            .into());
        }
        return Ok(existing.id);
    }

    let parent_id = if let Some(parent_name) = parent_name {
        Some(ensure_category_exists(
            aglet,
            store,
            parent_name,
            None,
            CategoryValueKind::Tag,
        )?)
    } else {
        None
    };

    let mut category = Category::new(name.to_string());
    category.parent = parent_id;
    category.value_kind = value_kind;
    aglet.create_category(&category)?;
    Ok(category.id)
}

pub(crate) fn csv_record_value(
    record: &csv::StringRecord,
    headers: &csv::StringRecord,
    column_name: &str,
) -> CliResult<String> {
    let wanted = column_name.trim();
    let index = headers
        .iter()
        .position(|header| header.eq_ignore_ascii_case(wanted))
        .ok_or_else(|| format!("CSV column not found: {column_name}"))?;
    Ok(record.get(index).unwrap_or_default().trim().to_string())
}

pub(crate) fn parse_sort_specs(
    args: &[String],
    categories: &[Category],
) -> CliResult<Vec<SortKey>> {
    args.iter()
        .map(|arg| parse_sort_spec(arg, categories))
        .collect()
}

pub(crate) fn parse_sort_spec(arg: &str, categories: &[Category]) -> CliResult<SortKey> {
    let (raw_field, direction) = parse_sort_field_and_direction(arg)?;
    let field = if raw_field.eq_ignore_ascii_case("item") {
        SortField::ItemText
    } else if raw_field.eq_ignore_ascii_case("when") {
        SortField::WhenDate
    } else {
        SortField::Category(category_id_by_name(categories, raw_field)?)
    };

    Ok(SortKey { field, direction })
}

pub(crate) fn parse_sort_field_and_direction(arg: &str) -> CliResult<(&str, SortDirection)> {
    let trimmed = arg.trim();
    if trimmed.is_empty() {
        return Err("sort key cannot be empty".into());
    }

    if let Some((field, direction_suffix)) = trimmed.rsplit_once(':') {
        let direction = if direction_suffix.eq_ignore_ascii_case("asc") {
            SortDirection::Asc
        } else if direction_suffix.eq_ignore_ascii_case("desc") {
            SortDirection::Desc
        } else {
            return Ok((trimmed, SortDirection::Asc));
        };
        let field = field.trim();
        if field.is_empty() {
            return Err(format!("invalid sort key '{arg}': missing column name").into());
        }
        Ok((field, direction))
    } else {
        Ok((trimmed, SortDirection::Asc))
    }
}
