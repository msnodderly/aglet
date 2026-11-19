//! `category` subcommands.

use crate::*;

pub(crate) fn cmd_category(
    aglet: &Aglet<'_>,
    store: &Store,
    command: CategoryCommand,
) -> CliResult {
    match command {
        CategoryCommand::List => {
            let categories = store.get_hierarchy()?;
            print_category_tree(&categories);
            Ok(())
        }
        CategoryCommand::Show { name } => {
            let categories = store.get_hierarchy()?;
            let category_id = category_id_by_name(&categories, &name)?;
            let category = store.get_category(category_id)?;
            let category_names = category_name_map(&categories);

            println!("id:              {}", category.id);
            println!("name:            {}", category.name);
            let parent_label = category
                .parent
                .and_then(|id| category_names.get(&id))
                .map(|s| s.as_str())
                .unwrap_or("(root)");
            println!("parent:          {}", parent_label);
            println!(
                "type:            {}",
                category_value_kind_label(category.value_kind)
            );
            println!("exclusive:       {}", category.is_exclusive);
            println!("actionable:      {}", category.is_actionable);
            println!("implicit_string: {}", category.enable_implicit_string);
            if category.value_kind == CategoryValueKind::Numeric {
                if let Some(format) = &category.numeric_format {
                    println!("numeric.decimals: {}", format.decimal_places);
                    println!(
                        "numeric.currency: {}",
                        format.currency_symbol.as_deref().unwrap_or("(none)")
                    );
                    println!(
                        "numeric.thousands_separator: {}",
                        format.use_thousands_separator
                    );
                }
            }
            if let Some(note) = &category.note {
                println!("note:            {}", note);
            }
            if !category.children.is_empty() {
                let child_names: Vec<&str> = category
                    .children
                    .iter()
                    .filter_map(|id| category_names.get(id).map(|s| s.as_str()))
                    .collect();
                println!("children:        {}", child_names.join(", "));
            }
            if !category.conditions.is_empty() {
                println!("conditions:");
                let resolve = |id: CategoryId| {
                    category_names
                        .get(&id)
                        .cloned()
                        .unwrap_or_else(|| "(deleted)".to_string())
                };
                for (i, condition) in category.conditions.iter().enumerate() {
                    let kind = match condition {
                        aglet_core::model::Condition::ImplicitString => "ImplicitString",
                        aglet_core::model::Condition::Profile { .. } => "Profile",
                        aglet_core::model::Condition::Date { .. } => "Date",
                        aglet_core::model::Condition::Numeric { .. } => "Numeric",
                    };
                    println!(
                        "  {}. [{}] {} -> {}",
                        i + 1,
                        kind,
                        condition.render(&resolve),
                        category.name
                    );
                }
            }
            if !category.actions.is_empty() {
                println!("actions:");
                for (index, action) in category.actions.iter().enumerate() {
                    println!(
                        "  {}",
                        indexed_category_action_row(index, action, &category_names)
                    );
                }
            }
            println!("created_at:      {}", category.created_at);
            println!("modified_at:     {}", category.modified_at);
            println!(
                "condition_mode:  {}",
                match category.condition_match_mode {
                    ConditionMatchMode::Any => "any",
                    ConditionMatchMode::All => "all",
                }
            );
            Ok(())
        }
        CategoryCommand::Create {
            name,
            parent,
            exclusive,
            disable_implicit_string,
            category_type,
        } => {
            let categories = store.get_hierarchy()?;
            let requested_name = name.clone();
            let mut category = Category::new(name);
            category.parent = parent
                .as_deref()
                .map(|parent_name| category_id_by_name(&categories, parent_name))
                .transpose()?;
            category.is_exclusive = exclusive;
            category.enable_implicit_string = !disable_implicit_string;
            if let Some(category_type) = category_type {
                category.value_kind = category_type.into_model();
            }

            let result = match aglet.create_category(&category) {
                Ok(result) => result,
                Err(AgletError::DuplicateName {
                    name: duplicate_name,
                }) => {
                    let existing_id = categories
                        .iter()
                        .find(|existing| existing.name.eq_ignore_ascii_case(&duplicate_name))
                        .map(|existing| existing.id);
                    return Err(duplicate_category_create_error(
                        &requested_name,
                        parent.as_deref(),
                        existing_id,
                    )
                    .into());
                }
                Err(other) => return Err(other.into()),
            };
            println!(
                "created category {} (type={}, processed_items={}, affected_items={})",
                category.name,
                category_value_kind_label(category.value_kind),
                result.processed_items,
                result.affected_items
            );
            print_bulk_warnings(&result);
            Ok(())
        }
        CategoryCommand::Delete { name } => {
            let categories = store.get_hierarchy()?;
            let category_id = category_id_by_name(&categories, &name)?;
            store.delete_category(category_id)?;
            println!("deleted category {}", name);
            Ok(())
        }
        CategoryCommand::Rename { name, new_name } => {
            let categories = store.get_hierarchy()?;
            let category_id = category_id_by_name(&categories, &name)?;
            let mut category = store.get_category(category_id)?;
            category.name = new_name.clone();
            let result = aglet.update_category(&category)?;
            println!(
                "renamed {} -> {} (processed_items={}, affected_items={})",
                name, new_name, result.processed_items, result.affected_items
            );
            print_bulk_warnings(&result);
            Ok(())
        }
        CategoryCommand::Reparent { name, parent, root } => {
            let categories = store.get_hierarchy()?;
            let category_id = category_id_by_name(&categories, &name)?;
            let mut category = store.get_category(category_id)?;
            if root {
                category.parent = None;
            } else if let Some(parent_name) = parent {
                category.parent = Some(category_id_by_name(&categories, &parent_name)?);
            } else {
                return Err("specify --parent <name> or --root to make top-level".into());
            }
            let result = aglet.update_category(&category)?;
            let new_parent = category
                .parent
                .and_then(|id| categories.iter().find(|c| c.id == id))
                .map(|c| c.name.as_str())
                .unwrap_or("(root)");
            println!(
                "reparented {} under {} (processed_items={}, affected_items={})",
                name, new_parent, result.processed_items, result.affected_items
            );
            print_bulk_warnings(&result);
            Ok(())
        }
        CategoryCommand::Update {
            name,
            exclusive,
            actionable,
            implicit_string,
            note,
            clear_note,
            category_type,
        } => {
            let categories = store.get_hierarchy()?;
            let category_id = category_id_by_name(&categories, &name)?;
            let mut category = store.get_category(category_id)?;
            if exclusive.is_none()
                && actionable.is_none()
                && implicit_string.is_none()
                && note.is_none()
                && !clear_note
                && category_type.is_none()
            {
                return Err("nothing to update: specify --exclusive, --actionable, --implicit-string, --type, --note, or --clear-note".into());
            }
            if let Some(val) = exclusive {
                category.is_exclusive = val;
            }
            if let Some(val) = actionable {
                category.is_actionable = val;
            }
            if let Some(val) = implicit_string {
                category.enable_implicit_string = val;
            }
            if clear_note {
                category.note = None;
            } else if let Some(new_note) = note {
                category.note = if new_note.is_empty() {
                    None
                } else {
                    Some(new_note)
                };
            }
            if let Some(category_type) = category_type {
                category.value_kind = category_type.into_model();
            }
            let result = aglet.update_category(&category)?;
            println!(
                "updated {} (type={}, exclusive={}, actionable={}, implicit_string={}, processed_items={}, affected_items={})",
                category.name,
                category_value_kind_label(category.value_kind),
                category.is_exclusive,
                category.is_actionable,
                category.enable_implicit_string,
                result.processed_items,
                result.affected_items
            );
            print_bulk_warnings(&result);
            Ok(())
        }
        CategoryCommand::Assign {
            item_id,
            category_name,
        } => {
            let item_id = resolve_item_id(&item_id, store)?;
            let categories = store.get_hierarchy()?;
            let category_id = category_id_by_name(&categories, &category_name)?;
            let category = categories
                .iter()
                .find(|c| c.id == category_id)
                .ok_or_else(|| format!("category not found: {category_name}"))?;

            if category_name.eq_ignore_ascii_case("Done") {
                aglet.mark_item_done(item_id)?;
                println!(
                    "assigned item {} to category Done (is_done and done_date updated)",
                    item_id
                );
                return Ok(());
            }
            if category.value_kind == CategoryValueKind::Numeric {
                return Err(format!(
                    "category '{}' is Numeric; use `aglet category set-value <item-id> \"{}\" <number>`",
                    category.name, category.name
                ).into());
            }

            let result = aglet.assign_item_manual(
                item_id,
                category_id,
                Some("manual:cli.assign".to_string()),
            )?;
            println!("assigned item {} to category {}", item_id, category_name);
            if !result.new_assignments.is_empty() {
                println!("new_assignments={}", result.new_assignments.len());
            }
            print_process_warnings(&result);
            Ok(())
        }
        CategoryCommand::SetValue {
            item_id,
            category_name,
            value,
        } => {
            let item_id = resolve_item_id(&item_id, store)?;
            let categories = store.get_hierarchy()?;
            let category_id = category_id_by_name(&categories, &category_name)?;
            let numeric_value = parse_decimal_value(&value)?;
            let result = aglet.assign_item_numeric_manual(
                item_id,
                category_id,
                numeric_value,
                Some("manual:cli.set-value".to_string()),
            )?;
            println!(
                "set value for item {} category {} = {}",
                item_id, category_name, numeric_value
            );
            if !result.new_assignments.is_empty() {
                println!("new_assignments={}", result.new_assignments.len());
            }
            print_process_warnings(&result);
            Ok(())
        }
        CategoryCommand::Format {
            name,
            decimals,
            currency,
            clear_currency,
            thousands,
            no_thousands,
        } => {
            if decimals.is_none()
                && currency.is_none()
                && !clear_currency
                && !thousands
                && !no_thousands
            {
                return Err(
                    "nothing to update: specify --decimals, --currency, --clear-currency, --thousands, or --no-thousands"
                        .into(),
                );
            }
            if currency.is_some() && clear_currency {
                return Err("--currency and --clear-currency are mutually exclusive".into());
            }

            let categories = store.get_hierarchy()?;
            let category_id = category_id_by_name(&categories, &name)?;
            let mut category = store.get_category(category_id)?;
            if category.value_kind != CategoryValueKind::Numeric {
                return Err(format!(
                    "category '{}' is not Numeric; numeric formatting only applies to Numeric categories",
                    category.name
                ).into());
            }

            let mut format = category.numeric_format.clone().unwrap_or_default();
            if let Some(decimals) = decimals {
                format.decimal_places = decimals;
            }
            if let Some(currency) = currency {
                format.currency_symbol = if currency.is_empty() {
                    None
                } else {
                    Some(currency)
                };
            } else if clear_currency {
                format.currency_symbol = None;
            }
            if thousands {
                format.use_thousands_separator = true;
            } else if no_thousands {
                format.use_thousands_separator = false;
            }
            category.numeric_format = Some(format.clone());
            let result = aglet.update_category(&category)?;
            println!(
                "updated numeric format for {} (decimals={}, currency={}, thousands={}, processed_items={}, affected_items={})",
                category.name,
                format.decimal_places,
                format.currency_symbol.as_deref().unwrap_or("(none)"),
                format.use_thousands_separator,
                result.processed_items,
                result.affected_items
            );
            print_bulk_warnings(&result);
            Ok(())
        }
        CategoryCommand::Unassign {
            item_id,
            category_name,
        } => {
            let item_id = resolve_item_id(&item_id, store)?;
            let categories = store.get_hierarchy()?;
            let category_id = category_id_by_name(&categories, &category_name)?;

            if category_name.eq_ignore_ascii_case("Done") {
                let item = store.get_item(item_id)?;
                if item.is_done {
                    aglet.toggle_item_done(item_id)?;
                    println!(
                        "unassigned item {} from category Done (marked not-done)",
                        item_id
                    );
                    return Ok(());
                }
            }

            aglet.unassign_item_manual(item_id, category_id)?;
            let vetoed = store.get_vetoes_for_item(item_id)?.contains(&category_id);
            if vetoed {
                println!(
                    "unassigned item {} from category {} (vetoed: will not be auto-assigned again; clear with `category clear-veto`)",
                    item_id, category_name
                );
            } else {
                println!(
                    "unassigned item {} from category {}",
                    item_id, category_name
                );
            }
            Ok(())
        }
        CategoryCommand::ListVetoes { item_id } => {
            let item_id = resolve_item_id(&item_id, store)?;
            let vetoes = store.get_vetoes_for_item(item_id)?;
            if vetoes.is_empty() {
                println!("no vetoes for item {item_id}");
                return Ok(());
            }
            let categories = store.get_hierarchy()?;
            let names = category_name_map(&categories);
            let mut labels: Vec<String> = vetoes
                .iter()
                .map(|id| names.get(id).cloned().unwrap_or_else(|| id.to_string()))
                .collect();
            labels.sort();
            for label in labels {
                println!("{label}");
            }
            Ok(())
        }
        CategoryCommand::ClearVeto {
            item_id,
            category_name,
        } => {
            let item_id = resolve_item_id(&item_id, store)?;
            let categories = store.get_hierarchy()?;
            let category_id = category_id_by_name(&categories, &category_name)?;
            store.remove_assignment_veto(item_id, category_id)?;
            println!("cleared veto on {category_name} for item {item_id}");
            Ok(())
        }
        CategoryCommand::AddCondition {
            name,
            and_categories,
            not_categories,
            or_categories,
        } => {
            if and_categories.is_empty() && not_categories.is_empty() && or_categories.is_empty() {
                return Err("at least one criterion required: use --and, --not, or --or".into());
            }
            let categories = store.get_hierarchy()?;
            let category_id = category_id_by_name(&categories, &name)?;
            let mut category = store.get_category(category_id)?;

            let mut criteria = Vec::new();
            for cat_name in &and_categories {
                let id = category_id_by_name(&categories, cat_name)?;
                criteria.push(Criterion {
                    mode: CriterionMode::And,
                    category_id: id,
                });
            }
            for cat_name in &not_categories {
                let id = category_id_by_name(&categories, cat_name)?;
                criteria.push(Criterion {
                    mode: CriterionMode::Not,
                    category_id: id,
                });
            }
            for cat_name in &or_categories {
                let id = category_id_by_name(&categories, cat_name)?;
                criteria.push(Criterion {
                    mode: CriterionMode::Or,
                    category_id: id,
                });
            }

            let query = Query {
                criteria,
                ..Query::default()
            };
            category.conditions.push(Condition::Profile {
                criteria: Box::new(query),
            });
            let result = aglet.update_category(&category)?;

            let condition_index = category.conditions.len();
            println!(
                "added profile condition #{} to {} (processed_items={}, affected_items={})",
                condition_index, name, result.processed_items, result.affected_items
            );
            print_bulk_warnings(&result);
            Ok(())
        }
        CategoryCommand::AddDateCondition {
            name,
            source,
            on,
            before,
            after,
            at_or_before,
            at_or_after,
            from,
            through,
        } => {
            let categories = store.get_hierarchy()?;
            let category_id = category_id_by_name(&categories, &name)?;
            let mut category = store.get_category(category_id)?;

            let matcher = build_date_matcher_from_args(
                on,
                before,
                after,
                at_or_before,
                at_or_after,
                from,
                through,
            )?;
            let condition = Condition::Date {
                source: source.into_model(),
                matcher: matcher.clone(),
            };
            category.conditions.push(condition);
            let result = aglet.update_category(&category)?;

            let condition_index = category.conditions.len();
            println!(
                "added date condition #{} to {}: {} (processed_items={}, affected_items={})",
                condition_index,
                name,
                render_date_condition(source.into_model(), &matcher),
                result.processed_items,
                result.affected_items
            );
            print_bulk_warnings(&result);
            Ok(())
        }
        CategoryCommand::AddNumericCondition {
            name,
            numeric_category,
            min,
            max,
            outside,
        } => {
            let categories = store.get_hierarchy()?;
            let category_id = category_id_by_name(&categories, &name)?;
            let numeric_category_id = category_id_by_name(&categories, &numeric_category)?;
            let target = store.get_category(numeric_category_id)?;
            if target.value_kind != aglet_core::model::CategoryValueKind::Numeric {
                return Err(format!("category '{}' is not Numeric", target.name).into());
            }

            let parse_bound = |label: &str, raw: &Option<String>| -> CliResult<Option<Decimal>> {
                raw.as_deref()
                    .map(|value| {
                        value
                            .parse::<Decimal>()
                            .map_err(|_| format!("invalid {label} value: {value}").into())
                    })
                    .transpose()
            };
            let min = parse_bound("--min", &min)?;
            let max = parse_bound("--max", &max)?;
            if let (Some(low), Some(high)) = (min, max) {
                if low > high {
                    return Err(format!("--min {low} is greater than --max {high}").into());
                }
            }
            if outside && min.is_none() && max.is_none() {
                return Err("--outside requires --min and/or --max".into());
            }

            let mut category = store.get_category(category_id)?;
            let condition = Condition::Numeric {
                category_id: numeric_category_id,
                min,
                max,
                outside,
            };
            category.conditions.push(condition);
            let result = aglet.update_category(&category)?;

            println!(
                "added numeric condition #{} to {}: {} (processed_items={}, affected_items={})",
                category.conditions.len(),
                name,
                aglet_core::model::render_numeric_condition(&target.name, min, max, outside),
                result.processed_items,
                result.affected_items
            );
            print_bulk_warnings(&result);
            Ok(())
        }
        CategoryCommand::SetConditionMode { name, mode } => {
            let categories = store.get_hierarchy()?;
            let category_id = category_id_by_name(&categories, &name)?;
            let mut category = store.get_category(category_id)?;
            category.condition_match_mode = mode.into_model();
            let result = aglet.update_category(&category)?;
            println!(
                "set condition mode on {} to {} (processed_items={}, affected_items={})",
                name,
                match category.condition_match_mode {
                    ConditionMatchMode::Any => "any",
                    ConditionMatchMode::All => "all",
                },
                result.processed_items,
                result.affected_items
            );
            print_bulk_warnings(&result);
            Ok(())
        }
        CategoryCommand::RemoveCondition { name, index } => {
            let categories = store.get_hierarchy()?;
            let category_id = category_id_by_name(&categories, &name)?;
            let mut category = store.get_category(category_id)?;

            if index == 0 || index > category.conditions.len() {
                return Err(format!(
                    "condition index {} out of range: {} has {} condition(s)",
                    index,
                    name,
                    category.conditions.len()
                )
                .into());
            }
            let removed = category.conditions.remove(index - 1);
            let result = aglet.update_category(&category)?;

            let category_names = category_name_map(&categories);
            let desc = removed.render(&|id: CategoryId| {
                category_names
                    .get(&id)
                    .cloned()
                    .unwrap_or_else(|| "(deleted)".to_string())
            });
            println!(
                "removed condition #{} ({}) from {} (processed_items={}, affected_items={})",
                index, desc, name, result.processed_items, result.affected_items
            );
            print_bulk_warnings(&result);
            Ok(())
        }
        CategoryCommand::AddAction {
            name,
            assign_categories,
            remove_categories,
            assign_numeric,
            value,
            set_when,
            mark_done,
            delete,
        } => {
            let kinds_requested = usize::from(!assign_categories.is_empty())
                + usize::from(!remove_categories.is_empty())
                + usize::from(assign_numeric.is_some())
                + usize::from(set_when.is_some())
                + usize::from(mark_done)
                + usize::from(delete);
            if kinds_requested != 1 {
                return Err(
                    "specify exactly one action kind: --assign, --remove, --assign-numeric, --set-when, --mark-done, or --delete"
                        .into(),
                );
            }

            let categories = store.get_hierarchy()?;
            let category_id = category_id_by_name(&categories, &name)?;

            let (action, action_kind) =
                if !assign_categories.is_empty() || !remove_categories.is_empty() {
                    let assign_requested = !assign_categories.is_empty();
                    let target_names = if assign_requested {
                        &assign_categories
                    } else {
                        &remove_categories
                    };
                    let mut targets = HashSet::new();
                    for target_name in target_names {
                        let target_id = category_id_by_name(&categories, target_name)?;
                        targets.insert(target_id);
                    }
                    if assign_requested {
                        (Action::Assign { targets }, "assign")
                    } else {
                        (Action::Remove { targets }, "remove")
                    }
                } else if let Some(numeric_name) = &assign_numeric {
                    let target_id = category_id_by_name(&categories, numeric_name)?;
                    let raw = value.as_deref().expect("clap enforces --value");
                    let parsed = raw
                        .parse::<Decimal>()
                        .map_err(|_| format!("invalid --value: {raw}"))?;
                    (
                        Action::AssignNumeric {
                            target: target_id,
                            value: parsed,
                        },
                        "assign-numeric",
                    )
                } else if let Some(expr) = &set_when {
                    let parsed = parse_date_value_expr(expr)?;
                    (Action::SetWhen { value: parsed }, "set-when")
                } else if mark_done {
                    (Action::MarkDone, "mark-done")
                } else {
                    (Action::Delete, "delete")
                };

            let (action_index, result) = aglet.add_category_action(category_id, action)?;
            println!(
                "added {} action #{} to {} (processed_items={}, affected_items={})",
                action_kind,
                action_index + 1,
                name,
                result.processed_items,
                result.affected_items
            );
            print_bulk_warnings(&result);
            Ok(())
        }
        CategoryCommand::SetAllowDelete { name, enabled } => {
            let enabled = match enabled.to_ascii_lowercase().as_str() {
                "true" | "on" | "yes" | "1" => true,
                "false" | "off" | "no" | "0" => false,
                other => return Err(format!("expected true/false, got '{other}'").into()),
            };
            let categories = store.get_hierarchy()?;
            let category_id = category_id_by_name(&categories, &name)?;
            let mut category = store.get_category(category_id)?;
            category.allow_delete_action = enabled;
            aglet.update_category(&category)?;
            println!(
                "allow_delete_action on {} is now {}",
                name,
                if enabled { "enabled" } else { "disabled" }
            );
            Ok(())
        }
        CategoryCommand::RemoveAction { name, index } => {
            let categories = store.get_hierarchy()?;
            let category_id = category_id_by_name(&categories, &name)?;
            let category = store.get_category(category_id)?;

            if index == 0 || index > category.actions.len() {
                return Err(format!(
                    "action index {} out of range: {} has {} action(s)",
                    index,
                    name,
                    category.actions.len()
                )
                .into());
            }
            let (removed, result) = aglet.remove_category_action(category_id, index - 1)?;
            let category_names = category_name_map(&categories);
            let desc = describe_category_action(&removed, &category_names);
            println!(
                "removed action #{} ({}) from {} (processed_items={}, affected_items={})",
                index, desc, name, result.processed_items, result.affected_items
            );
            print_bulk_warnings(&result);
            Ok(())
        }
    }
}
