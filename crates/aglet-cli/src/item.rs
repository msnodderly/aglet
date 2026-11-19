//! Item commands: add, edit, show, list, search, export, delete, restore, and the claim workflow.

use crate::*;

pub(crate) fn cmd_add(
    aglet: &Aglet<'_>,
    text: String,
    note: Option<String>,
    when: Option<String>,
    categories: Vec<String>,
    values: Vec<String>,
) -> CliResult {
    if text.trim().is_empty() {
        return Err("text cannot be empty".into());
    }
    let category_names: Vec<String> = aglet
        .store()
        .get_hierarchy()?
        .into_iter()
        .map(|category| category.name)
        .collect();
    let unknown_hashtags = unknown_hashtag_tokens(&text, &category_names);
    let categories_hierarchy = aglet.store().get_hierarchy()?;
    let tag_assignments = resolve_tag_category_assignments(&categories_hierarchy, &categories)?;
    let value_assignments = resolve_value_assignments(&categories_hierarchy, &values)?;
    let parsed_when = when.as_deref().map(parse_when_datetime_input).transpose()?;

    let mut item = Item::new(text);
    item.note = note;

    let reference_date = jiff::Zoned::now().date();
    let result = aglet.create_item_with_reference_date(&item, reference_date)?;
    if let Some(explicit_when) = parsed_when {
        aglet.set_item_when_date(
            item.id,
            Some(explicit_when),
            Some("manual:cli.when".to_string()),
        )?;
    }
    for (category_id, category_name) in tag_assignments {
        apply_tag_category_assignment(aglet, item.id, category_id, &category_name)?;
    }
    for assignment in value_assignments {
        apply_numeric_value_assignment(aglet, item.id, assignment)?;
    }
    let created = aglet.store().get_item(item.id)?;

    println!("created {}", item.id);
    if let Some(line) = parsed_when_feedback_line(created.when_date) {
        println!("{line}");
    }
    if !result.new_assignments.is_empty() {
        println!("new_assignments={}", result.new_assignments.len());
    }
    print_process_warnings(&result);
    if let Some(line) = unknown_hashtag_feedback_line(&unknown_hashtags) {
        println!("{line}");
    }
    Ok(())
}

pub(crate) fn temporal_reevaluate_before_command(aglet: &Aglet<'_>) -> CliResult {
    if aglet.has_date_conditions()? {
        let _ = aglet.reevaluate_temporal_conditions()?;
    }
    Ok(())
}

pub(crate) fn build_date_matcher_from_args(
    on: Option<String>,
    before: Option<String>,
    after: Option<String>,
    at_or_before: Option<String>,
    at_or_after: Option<String>,
    from: Option<String>,
    through: Option<String>,
) -> CliResult<DateMatcher> {
    let compare_inputs = [
        ("on", on.as_ref(), DateCompareOp::On),
        ("before", before.as_ref(), DateCompareOp::Before),
        ("after", after.as_ref(), DateCompareOp::After),
        (
            "at-or-before",
            at_or_before.as_ref(),
            DateCompareOp::AtOrBefore,
        ),
        (
            "at-or-after",
            at_or_after.as_ref(),
            DateCompareOp::AtOrAfter,
        ),
    ];

    let compare_count = compare_inputs
        .iter()
        .filter(|(_, value, _)| value.is_some())
        .count();
    let has_range = from.is_some() || through.is_some();

    if has_range {
        if compare_count > 0 {
            return Err(
                "use either a compare flag (--on/--before/--after/--at-or-before/--at-or-after) or a range (--from/--through), not both"
                    .into(),
            );
        }
        let from = from.ok_or_else(|| "--from requires a value".to_string())?;
        let through = through.ok_or_else(|| "--from also requires --through".to_string())?;
        return Ok(DateMatcher::Range {
            from: parse_date_value_expr(&from)?,
            through: parse_date_value_expr(&through)?,
        });
    }

    if compare_count != 1 {
        return Err(
            "specify exactly one of --on, --before, --after, --at-or-before, --at-or-after, or --from/--through"
                .into(),
        );
    }

    for (_label, maybe_value, op) in compare_inputs {
        if let Some(value) = maybe_value {
            return Ok(DateMatcher::Compare {
                op,
                value: parse_date_value_expr(value)?,
            });
        }
    }

    unreachable!("validated compare inputs should always return a matcher")
}

pub(crate) fn read_note_from_stdin(reader: &mut impl Read) -> CliResult<String> {
    let mut input = String::new();
    reader
        .read_to_string(&mut input)
        .map_err(|e| format!("failed to read --note-stdin input: {e}"))?;
    Ok(input)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn cmd_edit(
    aglet: &Aglet<'_>,
    item_id_str: String,
    text: Option<String>,
    note: Option<String>,
    append_note: Option<String>,
    note_stdin: Option<String>,
    clear_note: bool,
    done: Option<bool>,
    when: Option<String>,
    clear_when: bool,
    recurrence: Option<String>,
    clear_recurrence: bool,
) -> CliResult {
    let item_id = resolve_item_id(&item_id_str, aglet.store())?;

    if text.is_none()
        && note.is_none()
        && append_note.is_none()
        && note_stdin.is_none()
        && !clear_note
        && done.is_none()
        && when.is_none()
        && !clear_when
        && recurrence.is_none()
        && !clear_recurrence
    {
        return Err(
            "nothing to update\n\nUsage: aglet edit <ITEM_ID> [TEXT] [--note <NOTE>] [--append-note <TEXT>] [--note-stdin] [--clear-note] [--done <true|false>] [--when <DATE>] [--clear-when] [--recurrence <RULE>] [--clear-recurrence]\n\nExamples:\n  aglet edit <id> \"new text here\"\n  aglet edit <id> --note \"updated note\"\n  aglet edit <id> --append-note \"extra info\"\n  printf \"line one\\nline two\\n\" | aglet edit <id> --note-stdin\n  aglet edit <id> \"new text\" --note \"and note\"\n  aglet edit <id> --clear-note\n  aglet edit <id> --done true\n  aglet edit <id> --done false\n  aglet edit <id> --when 2026-02-20\n  aglet edit <id> --clear-when\n  aglet edit <id> --recurrence \"every friday\"\n  aglet edit <id> --clear-recurrence".into()
        );
    }

    if when.is_some() && clear_when {
        return Err("--when and --clear-when are mutually exclusive".into());
    }

    // Validate mutually exclusive note flags
    let note_flag_count = note.is_some() as u8
        + append_note.is_some() as u8
        + note_stdin.is_some() as u8
        + clear_note as u8;
    if note_flag_count > 1 {
        return Err(
            "--note, --append-note, --note-stdin, and --clear-note are mutually exclusive".into(),
        );
    }

    if let Some(done_value) = done {
        if done_value {
            let result = aglet.mark_item_done(item_id)?;
            if let Some(successor_id) = result.successor_item_id {
                let successor = aglet.store().get_item(successor_id)?;
                let when_str = successor
                    .when_date
                    .map(|dt| dt.to_string())
                    .unwrap_or_else(|| "-".to_string());
                println!(
                    "marked done {}; successor created: {} (when: {})",
                    item_id, successor_id, when_str
                );
            } else {
                println!("marked done {}", item_id);
            }
        } else {
            aglet.mark_item_not_done(item_id)?;
            println!("marked not-done {}", item_id);
        }
    }

    let mut item = aglet.store().get_item(item_id)?;

    let note_stdin_has_content = note_stdin.as_ref().is_some_and(|value| !value.is_empty());
    if text.is_some()
        || note.is_some()
        || append_note.is_some()
        || note_stdin_has_content
        || clear_note
    {
        if let Some(new_text) = text {
            if new_text.is_empty() {
                return Err("text cannot be empty".into());
            }
            item.text = new_text;
        }
        if clear_note {
            item.note = None;
        } else if let Some(new_note) = note {
            item.note = if new_note.is_empty() {
                None
            } else {
                Some(new_note)
            };
        } else if let Some(new_note_from_stdin) = note_stdin {
            if !new_note_from_stdin.is_empty() {
                item.note = Some(new_note_from_stdin);
            }
        } else if let Some(extra) = append_note {
            if !extra.is_empty() {
                item.note = Some(match item.note {
                    Some(existing) => format!("{}\n{}", existing, extra),
                    None => extra,
                });
            }
        }

        item.modified_at = jiff::Timestamp::now();
        let reference_date = jiff::Zoned::now().date();
        aglet.update_item_with_reference_date(&item, reference_date)?;

        let updated = aglet.store().get_item(item_id)?;
        println!("updated {}", item_id);
        if let Some(line) = parsed_when_feedback_line(updated.when_date) {
            println!("{line}");
        }
    }

    if let Some(explicit_when) = when {
        let parsed = parse_when_datetime_input(&explicit_when)?;
        aglet.set_item_when_date(item_id, Some(parsed), Some("manual:cli.when".to_string()))?;
        let updated = aglet.store().get_item(item_id)?;
        println!("updated {}", item_id);
        if let Some(line) = parsed_when_feedback_line(updated.when_date) {
            println!("{line}");
        }
    } else if clear_when {
        aglet.set_item_when_date(item_id, None, Some("manual:cli.when-clear".to_string()))?;
        println!("updated {}", item_id);
    }

    if recurrence.is_some() && clear_recurrence {
        return Err("--recurrence and --clear-recurrence are mutually exclusive".into());
    }
    if let Some(recurrence_text) = recurrence {
        let parser = aglet_core::dates::BasicDateParser::default();
        let reference = jiff::Zoned::now().date();
        match parser.parse_with_recurrence(&recurrence_text, reference) {
            Some(aglet_core::dates::DateParseResult::Recurring { rule, .. }) => {
                let mut item = aglet.store().get_item(item_id)?;
                item.recurrence_rule = Some(rule.clone());
                item.modified_at = jiff::Timestamp::now();
                aglet.store().update_item(&item)?;
                println!("set recurrence: {}", rule.display());
            }
            _ => {
                return Err(format!(
                    "unrecognized recurrence pattern: \"{}\"\n\nExamples: daily, weekly, every friday, every 2 weeks, monthly on the 15th",
                    recurrence_text
                ).into());
            }
        }
    } else if clear_recurrence {
        let mut item = aglet.store().get_item(item_id)?;
        item.recurrence_rule = None;
        item.modified_at = jiff::Timestamp::now();
        aglet.store().update_item(&item)?;
        println!("cleared recurrence");
    }

    Ok(())
}

pub(crate) fn cmd_show(store: &Store, item_id_str: String) -> CliResult {
    let item_id = resolve_item_id(&item_id_str, store)?;
    let item = store.get_item(item_id)?;
    let categories = store.get_hierarchy()?;
    let category_names = category_name_map(&categories);
    let categories_by_id: HashMap<CategoryId, &Category> =
        categories.iter().map(|c| (c.id, c)).collect();

    let done = if item.is_done { "done" } else { "open" };
    let when = item
        .when_date
        .map(|dt| dt.to_string())
        .unwrap_or_else(|| "-".to_string());

    println!("id:         {}", item.id);
    println!("text:       {}", item.text);
    println!("status:     {}", done);
    println!("when:       {}", when);
    println!("created_at: {}", item.created_at);
    println!("modified_at: {}", item.modified_at);
    if let Some(done_date) = item.done_date {
        println!("done_date:  {}", done_date);
    }
    if let Some(rule) = &item.recurrence_rule {
        println!("recurrence: {}", rule.display());
    }
    if let Some(series_id) = item.recurrence_series_id {
        println!("series_id:  {}", series_id);
    }
    if let Some(parent_id) = item.recurrence_parent_item_id {
        println!("parent_id:  {}", parent_id);
    }
    if let Some(note) = &item.note {
        println!("note:       {}", note);
    }

    if item.assignments.is_empty() {
        println!("assignments: (none)");
    } else {
        println!("assignments:");
        let mut rows: Vec<_> = item
            .assignments
            .iter()
            .map(|(cat_id, assignment)| {
                let name = category_names
                    .get(cat_id)
                    .cloned()
                    .unwrap_or_else(|| cat_id.to_string());
                (*cat_id, name, assignment)
            })
            .collect();
        rows.sort_by_key(|(_, name, _)| name.to_ascii_lowercase());
        for (cat_id, name, assignment) in rows {
            let origin = assignment.origin.as_deref().unwrap_or("-");
            let value_suffix = match assignment.numeric_value {
                Some(v) => {
                    let fmt = categories_by_id
                        .get(&cat_id)
                        .and_then(|c| c.numeric_format.as_ref());
                    format!(
                        " = {}",
                        aglet_core::numeric_format::format_numeric_cell(Some(v), fmt)
                    )
                }
                None => String::new(),
            };
            println!(
                "  {}{} | {:?} | {}",
                name, value_suffix, assignment.source, origin
            );
            if let Some(explanation) = &assignment.explanation {
                println!("    {}", explanation.summary());
            }
        }
    }

    for line in item_link_section_lines(store, item.id)? {
        println!("{line}");
    }

    Ok(())
}

pub(crate) fn resolved_workflow_or_err(
    store: &Store,
) -> CliResult<aglet_core::workflow::ResolvedWorkflowConfig> {
    resolve_workflow_config(store)?.ok_or_else(|| workflow_setup_error_message().into())
}

pub(crate) type ReadyQueueData = (
    View,
    Vec<Item>,
    Vec<Category>,
    HashMap<CategoryId, String>,
    HashSet<ItemId>,
);

pub(crate) fn ready_queue_data(store: &Store) -> CliResult<ReadyQueueData> {
    let workflow = resolved_workflow_or_err(store)?;
    let view = build_ready_queue_view(store, workflow)?;
    let categories = store.get_hierarchy()?;
    let category_names = category_name_map(&categories);
    let all_items = store.list_items()?;
    let claimable_ids = claimable_item_ids(store, &all_items, workflow)?;
    let items = all_items
        .into_iter()
        .filter(|item| claimable_ids.contains(&item.id))
        .collect();
    Ok((view, items, categories, category_names, HashSet::new()))
}

pub(crate) fn cmd_claim(aglet: &Aglet<'_>, store: &Store, item_id_str: String) -> CliResult {
    let item_id = resolve_item_id(&item_id_str, store)?;
    let workflow = resolved_workflow_or_err(store)?;
    let claim_category = store.get_category(workflow.claim_category_id)?;
    let result = aglet.claim_item_workflow(item_id)?;
    println!(
        "claimed item {} to category {}",
        item_id, claim_category.name
    );
    if !result.new_assignments.is_empty() {
        println!("new_assignments={}", result.new_assignments.len());
    }
    print_process_warnings(&result);
    Ok(())
}

pub(crate) fn cmd_release(aglet: &Aglet<'_>, store: &Store, item_id_str: String) -> CliResult {
    let item_id = resolve_item_id(&item_id_str, store)?;
    let workflow = resolved_workflow_or_err(store)?;
    let claim_category = store.get_category(workflow.claim_category_id)?;
    let result = aglet.release_item_claim(item_id)?;
    println!(
        "released item {} from category {}",
        item_id, claim_category.name
    );
    if !result.new_assignments.is_empty() {
        println!("new_assignments={}", result.new_assignments.len());
    }
    print_process_warnings(&result);
    Ok(())
}

pub(crate) fn cmd_ready(
    store: &Store,
    sort_args: Vec<String>,
    output_format: OutputFormatArg,
) -> CliResult {
    let (view, items, categories, category_names, blocked_item_ids) = ready_queue_data(store)?;
    let sort_keys = parse_sort_specs(&sort_args, &categories)?;
    print_items_for_view(
        &view,
        &items,
        &categories,
        &category_names,
        &sort_keys,
        output_format,
        &blocked_item_ids,
        TableStyle::Verbose,
    )
}

pub(crate) fn parsed_when_feedback_line(when_date: Option<DateTime>) -> Option<String> {
    when_date.map(|when| format!("parsed_when={when}"))
}

pub(crate) fn unknown_hashtag_feedback_line(unknown_hashtags: &[String]) -> Option<String> {
    if unknown_hashtags.is_empty() {
        return None;
    }
    Some(format!(
        "warning: unknown_hashtags={}",
        unknown_hashtags.join(",")
    ))
}

pub(crate) struct ListFilters {
    pub(crate) all_categories: Vec<String>,
    pub(crate) any_categories: Vec<String>,
    pub(crate) exclude_categories: Vec<String>,
    pub(crate) dependency_state_filter: Option<DependencyStateFilter>,
    pub(crate) value_eq: Vec<String>,
    pub(crate) value_in: Vec<String>,
    pub(crate) value_max: Vec<String>,
    pub(crate) include_done: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DependencyStateFilter {
    Blocked,
    NotBlocked,
}

pub(crate) fn dependency_state_filter_from_flags(
    blocked: bool,
    not_blocked: bool,
) -> Option<DependencyStateFilter> {
    if blocked {
        Some(DependencyStateFilter::Blocked)
    } else if not_blocked {
        Some(DependencyStateFilter::NotBlocked)
    } else {
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NumericPredicate {
    Eq(Decimal),
    In(Vec<Decimal>),
    Max(Decimal),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NumericFilter {
    pub(crate) category_id: CategoryId,
    pub(crate) category_name: String,
    pub(crate) predicate: NumericPredicate,
}

pub(crate) fn table_style_from_verbose(verbose: bool) -> TableStyle {
    if verbose {
        TableStyle::Verbose
    } else {
        TableStyle::Compact
    }
}

pub(crate) fn cmd_list(
    store: &Store,
    view_name: Option<String>,
    filters: ListFilters,
    sort_args: Vec<String>,
    output_format: OutputFormatArg,
    table_style: TableStyle,
) -> CliResult {
    let all_items = store.list_items()?;
    let blocked_item_ids = blocked_item_ids(store, &all_items)?;
    let mut items = all_items.clone();
    if !filters.include_done {
        items.retain(|item| !item.is_done);
    }

    let categories = store.get_hierarchy()?;
    let category_names = category_name_map(&categories);
    let sort_keys = parse_sort_specs(&sort_args, &categories)?;
    let numeric_filters = build_numeric_filters(&categories, &filters)?;

    if !filters.all_categories.is_empty() {
        let category_ids: Vec<CategoryId> = filters
            .all_categories
            .into_iter()
            .map(|name| category_id_by_name(&categories, &name))
            .collect::<Result<Vec<_>, _>>()?;
        retain_items_with_all_categories(&mut items, &category_ids);
    }
    if !filters.any_categories.is_empty() {
        let category_ids: Vec<CategoryId> = filters
            .any_categories
            .into_iter()
            .map(|name| category_id_by_name(&categories, &name))
            .collect::<Result<Vec<_>, _>>()?;
        retain_items_with_any_categories(&mut items, &category_ids);
    }
    if !filters.exclude_categories.is_empty() {
        let category_ids: Vec<CategoryId> = filters
            .exclude_categories
            .into_iter()
            .map(|name| category_id_by_name(&categories, &name))
            .collect::<Result<Vec<_>, _>>()?;
        reject_items_with_any_categories(&mut items, &category_ids);
    }
    if !numeric_filters.is_empty() {
        retain_items_matching_numeric_filters(&mut items, &numeric_filters);
    }
    if let Some(filter) = filters.dependency_state_filter {
        retain_items_by_dependency_state(
            store,
            &mut items,
            filter == DependencyStateFilter::Blocked,
        )?;
    }

    let resolved_view = if let Some(view_name) = view_name {
        Some(view_by_name(store, &view_name)?)
    } else {
        let views = store.list_views()?;
        views
            .iter()
            .find(|v| v.name.eq_ignore_ascii_case(DEFAULT_VIEW_NAME))
            .cloned()
            .or_else(|| views.into_iter().next())
    };

    if let Some(ref view) = resolved_view {
        if view.name.eq_ignore_ascii_case(READY_QUEUE_VIEW_NAME) {
            if let Some(workflow) = resolve_workflow_config(store)? {
                let claimable = claimable_item_ids(store, &items, workflow)?;
                items.retain(|item| claimable.contains(&item.id));
            }
        }
    }

    if let Some(view) = resolved_view {
        print_items_for_view(
            &view,
            &items,
            &categories,
            &category_names,
            &sort_keys,
            output_format,
            &blocked_item_ids,
            table_style,
        )?;
    } else if output_format == OutputFormatArg::Json {
        print_items_json(&items, &category_names, &sort_keys, &categories)?;
    } else {
        print_item_table(
            &items,
            &category_names,
            &sort_keys,
            &categories,
            table_style,
        );
    }
    Ok(())
}

pub(crate) fn retain_items_with_all_categories(items: &mut Vec<Item>, category_ids: &[CategoryId]) {
    items.retain(|item| {
        category_ids
            .iter()
            .all(|id| item.assignments.contains_key(id))
    });
}

pub(crate) fn retain_items_with_any_categories(items: &mut Vec<Item>, category_ids: &[CategoryId]) {
    items.retain(|item| {
        category_ids
            .iter()
            .any(|id| item.assignments.contains_key(id))
    });
}

pub(crate) fn reject_items_with_any_categories(items: &mut Vec<Item>, category_ids: &[CategoryId]) {
    items.retain(|item| {
        category_ids
            .iter()
            .all(|id| !item.assignments.contains_key(id))
    });
}

pub(crate) fn build_numeric_filters(
    categories: &[Category],
    filters: &ListFilters,
) -> CliResult<Vec<NumericFilter>> {
    let mut out = Vec::new();

    for (category_name, value) in parse_arg_pairs(&filters.value_eq, "--value-eq")? {
        let (category_id, resolved_name) =
            resolve_numeric_filter_category(categories, &category_name)?;
        let parsed = parse_decimal_value(&value)?;
        out.push(NumericFilter {
            category_id,
            category_name: resolved_name,
            predicate: NumericPredicate::Eq(parsed),
        });
    }

    for (category_name, values_csv) in parse_arg_pairs(&filters.value_in, "--value-in")? {
        let (category_id, resolved_name) =
            resolve_numeric_filter_category(categories, &category_name)?;
        let parsed_values = parse_csv_decimals(&values_csv, &resolved_name)?;
        out.push(NumericFilter {
            category_id,
            category_name: resolved_name,
            predicate: NumericPredicate::In(parsed_values),
        });
    }

    for (category_name, value) in parse_arg_pairs(&filters.value_max, "--value-max")? {
        let (category_id, resolved_name) =
            resolve_numeric_filter_category(categories, &category_name)?;
        let parsed = parse_decimal_value(&value)?;
        out.push(NumericFilter {
            category_id,
            category_name: resolved_name,
            predicate: NumericPredicate::Max(parsed),
        });
    }

    Ok(out)
}

pub(crate) fn parse_arg_pairs(
    args: &[String],
    flag_name: &str,
) -> CliResult<Vec<(String, String)>> {
    let (pairs, remainder) = args.as_chunks::<2>();
    if !remainder.is_empty() {
        return Err(format!(
            "invalid {flag_name} arguments: expected repeated <CATEGORY> <VALUE> pairs"
        )
        .into());
    }
    Ok(pairs
        .iter()
        .map(|[category, value]| (category.clone(), value.clone()))
        .collect())
}

pub(crate) fn resolve_numeric_filter_category(
    categories: &[Category],
    category_name: &str,
) -> CliResult<(CategoryId, String)> {
    let category_id = category_id_by_name(categories, category_name)?;
    let category = categories
        .iter()
        .find(|c| c.id == category_id)
        .ok_or_else(|| format!("category not found: {category_name}"))?;
    if category.value_kind != CategoryValueKind::Numeric {
        return Err(format!(
            "category '{}' is not Numeric; numeric value filters require a Numeric category",
            category.name
        )
        .into());
    }
    Ok((category.id, category.name.clone()))
}

pub(crate) fn parse_csv_decimals(input: &str, category_name: &str) -> CliResult<Vec<Decimal>> {
    let mut values = Vec::new();
    for token in input.split(',') {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            return Err(format!(
                "invalid --value-in for category '{}': empty value in CSV list",
                category_name
            )
            .into());
        }
        values.push(parse_decimal_value(trimmed)?);
    }
    Ok(values)
}

pub(crate) fn retain_items_matching_numeric_filters(
    items: &mut Vec<Item>,
    numeric_filters: &[NumericFilter],
) {
    items.retain(|item| {
        numeric_filters.iter().all(|filter| {
            let numeric_value = item
                .assignments
                .get(&filter.category_id)
                .and_then(|assignment| assignment.numeric_value);
            match &filter.predicate {
                NumericPredicate::Eq(expected) => numeric_value.is_some_and(|v| v == *expected),
                NumericPredicate::In(allowed) => {
                    numeric_value.is_some_and(|v| allowed.contains(&v))
                }
                NumericPredicate::Max(max_value) => numeric_value.is_some_and(|v| v <= *max_value),
            }
        })
    });
}

pub(crate) fn cmd_search(
    store: &Store,
    query: String,
    output_format: OutputFormatArg,
    dependency_state_filter: Option<DependencyStateFilter>,
    include_done: bool,
    table_style: TableStyle,
) -> CliResult {
    let mut items = store.list_items()?;
    if !include_done {
        items.retain(|item| !item.is_done);
    }

    let categories = store.get_hierarchy()?;
    let category_names = category_name_map(&categories);

    let q = Query {
        text_search: Some(query),
        ..Query::default()
    };
    let reference_date = jiff::Zoned::now().date();
    let matches = evaluate_query(&q, &items, reference_date);

    let mut matched_items: Vec<Item> = matches.into_iter().cloned().collect();
    if let Some(filter) = dependency_state_filter {
        retain_items_by_dependency_state(
            store,
            &mut matched_items,
            filter == DependencyStateFilter::Blocked,
        )?;
    }
    if output_format == OutputFormatArg::Json {
        print_items_json(&matched_items, &category_names, &[], &categories)?;
    } else {
        print_item_table(
            &matched_items,
            &category_names,
            &[],
            &categories,
            table_style,
        );
    }
    Ok(())
}

pub(crate) fn cmd_export(
    store: &Store,
    view_name: Option<String>,
    include_links: bool,
) -> CliResult {
    let body = build_markdown_export(store, view_name.as_deref(), include_links)?;
    write_stdout_allow_broken_pipe(&body)?;
    Ok(())
}

pub(crate) fn write_stdout_allow_broken_pipe(body: &str) -> CliResult {
    let mut stdout = io::stdout().lock();
    write_output_allow_broken_pipe(&mut stdout, body)
}

pub(crate) fn write_output_allow_broken_pipe<W: Write>(writer: &mut W, body: &str) -> CliResult {
    match writer
        .write_all(body.as_bytes())
        .and_then(|_| writer.flush())
    {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        Err(err) => Err(format!("failed writing to stdout: {err}").into()),
    }
}

pub(crate) fn cmd_delete(aglet: &Aglet<'_>, item_id_str: String) -> CliResult {
    let item_id = resolve_item_id(&item_id_str, aglet.store())?;
    aglet.delete_item(item_id, "user:cli")?;
    println!("deleted {}", item_id);
    Ok(())
}

pub(crate) fn cmd_deleted(store: &Store) -> CliResult {
    let deleted = store.list_deleted_items()?;
    if deleted.is_empty() {
        println!("no deleted items");
        return Ok(());
    }

    for entry in deleted {
        println!(
            "{} | item={} | deleted_at={} | by={} | {}",
            entry.id, entry.item_id, entry.deleted_at, entry.deleted_by, entry.text
        );
    }
    Ok(())
}

pub(crate) fn cmd_restore(store: &Store, log_id_str: String) -> CliResult {
    let log_id = Uuid::parse_str(&log_id_str).map_err(|e| format!("invalid log id: {e}"))?;
    let item_id = store.restore_deleted_item(log_id)?;
    println!("restored item {}", item_id);
    Ok(())
}
