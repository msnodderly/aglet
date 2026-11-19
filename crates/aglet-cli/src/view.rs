//! `view` subcommands.

use crate::*;

pub(crate) fn cmd_view(aglet: &Aglet<'_>, store: &Store, command: ViewCommand) -> CliResult {
    let _ = aglet;
    match command {
        ViewCommand::List => {
            let mut views = store.list_views()?;
            if let Ok(Some(workflow)) = resolve_workflow_config(store) {
                if let Ok(rq_view) = build_ready_queue_view(store, workflow) {
                    views.insert(0, rq_view);
                }
            }
            if views.is_empty() {
                println!("no views");
                return Ok(());
            }
            for view in views {
                println!(
                    "{} (sections={}, and={}, not={}, or={}, hide_dependent_items={})",
                    view.name,
                    view.sections.len(),
                    view.criteria.and_category_ids().count(),
                    view.criteria.not_category_ids().count(),
                    view.criteria.or_category_ids().count(),
                    view.hide_dependent_items
                );
            }
            println!("hint: use `aglet view show \"<name>\"` to see view contents");
            Ok(())
        }
        ViewCommand::Show {
            name,
            sort,
            format,
            blocked,
            not_blocked,
        } => {
            if name.eq_ignore_ascii_case(READY_QUEUE_VIEW_NAME) {
                if blocked {
                    return Err(
                        "Ready Queue only shows claimable items; --blocked is not supported".into(),
                    );
                }
                if not_blocked {
                    return Err(
                        "Ready Queue already excludes blocked items; --not-blocked is redundant"
                            .into(),
                    );
                }
                let (view, items, categories, category_names, blocked_item_ids) =
                    ready_queue_data(store)?;
                let sort_keys = parse_sort_specs(&sort, &categories)?;
                print_items_for_view(
                    &view,
                    &items,
                    &categories,
                    &category_names,
                    &sort_keys,
                    format,
                    &blocked_item_ids,
                    TableStyle::Verbose,
                )?;
            } else {
                let categories = store.get_hierarchy()?;
                let category_names = category_name_map(&categories);
                let all_items = store.list_items()?;
                let blocked_item_ids = blocked_item_ids(store, &all_items)?;
                let mut items = all_items;
                if let Some(filter) = dependency_state_filter_from_flags(blocked, not_blocked) {
                    retain_items_by_dependency_state(
                        store,
                        &mut items,
                        filter == DependencyStateFilter::Blocked,
                    )?;
                }
                let view = view_by_name(store, &name)?;
                let sort_keys = parse_sort_specs(&sort, &categories)?;
                print_items_for_view(
                    &view,
                    &items,
                    &categories,
                    &category_names,
                    &sort_keys,
                    format,
                    &blocked_item_ids,
                    TableStyle::Verbose,
                )?;
            }
            Ok(())
        }
        ViewCommand::Create {
            name,
            include,
            or_include,
            exclude,
            hide_unmatched,
            hide_dependent_items,
        } => {
            let categories = store.get_hierarchy()?;
            let mut view = View::new(name);
            view.show_unmatched = !hide_unmatched;
            view.hide_dependent_items = hide_dependent_items;
            view.criteria =
                query_from_category_names(&categories, &include, &or_include, &exclude)?;

            store.create_view(&view)?;
            println!("created view {}", view.name);
            Ok(())
        }
        ViewCommand::Edit {
            name,
            hide_unmatched,
            hide_dependent_items,
        } => {
            if name.eq_ignore_ascii_case(READY_QUEUE_VIEW_NAME) {
                return Err(format!("cannot modify system view: {READY_QUEUE_VIEW_NAME}").into());
            }
            let mut view = view_by_name(store, &name)?;
            let mut changed = false;
            if let Some(hide_unmatched) = hide_unmatched {
                let next_show_unmatched = !hide_unmatched;
                changed = changed || view.show_unmatched != next_show_unmatched;
                view.show_unmatched = next_show_unmatched;
            }
            if let Some(hide_dependent_items) = hide_dependent_items {
                changed = changed || view.hide_dependent_items != hide_dependent_items;
                view.hide_dependent_items = hide_dependent_items;
            }
            if !changed {
                return Err(
                    "no editable view changes requested (pass --hide-unmatched and/or --hide-dependent-items)"
                        .into(),
                );
            }
            store.update_view(&view)?;
            println!("updated view {}", view.name);
            Ok(())
        }
        ViewCommand::Clone {
            source_name,
            new_name,
        } => {
            if source_name.eq_ignore_ascii_case(READY_QUEUE_VIEW_NAME) {
                return Err(format!("cannot modify system view: {READY_QUEUE_VIEW_NAME}").into());
            }
            let source = view_by_name(store, &source_name)?;
            let cloned = store.clone_view(source.id, new_name)?;
            println!("cloned view {} -> {}", source_name, cloned.name);
            Ok(())
        }
        ViewCommand::Rename { name, new_name } => {
            if name.eq_ignore_ascii_case(READY_QUEUE_VIEW_NAME) {
                return Err(format!("cannot modify system view: {READY_QUEUE_VIEW_NAME}").into());
            }
            let mut view = view_by_name(store, &name)?;
            view.name = new_name.clone();
            store.update_view(&view)?;
            println!("renamed view {} -> {}", name, new_name);
            Ok(())
        }
        ViewCommand::Delete { name } => {
            if name.eq_ignore_ascii_case(READY_QUEUE_VIEW_NAME) {
                return Err(format!("cannot modify system view: {READY_QUEUE_VIEW_NAME}").into());
            }
            let view = view_by_name(store, &name)?;
            store.delete_view(view.id)?;
            println!("deleted view {}", name);
            Ok(())
        }
        ViewCommand::SetSummary {
            name,
            section,
            column,
            func,
        } => {
            let mut view = view_by_name(store, &name)?;
            ensure_mutable_view(&view)?;
            let num_sections = view.sections.len();
            if section >= num_sections {
                return Err(format!(
                    "section index {} out of range (view has {} sections)",
                    section, num_sections
                )
                .into());
            }
            let col_lower = column.to_lowercase();
            let category_names: HashMap<CategoryId, String> = store
                .get_hierarchy()?
                .into_iter()
                .map(|c| (c.id, c.name))
                .collect();
            let col_idx = view.sections[section]
                .columns
                .iter()
                .position(|c| {
                    category_names
                        .get(&c.heading)
                        .map(|n| n.to_lowercase() == col_lower)
                        .unwrap_or(false)
                })
                .ok_or_else(|| format!("column '{}' not found in section {}", column, section))?;
            let heading_id = view.sections[section].columns[col_idx].heading;
            view.sections[section].columns[col_idx].summary_fn = Some(func.to_model());
            store.update_view(&view)?;
            let col_name = category_names
                .get(&heading_id)
                .cloned()
                .unwrap_or_else(|| "?".to_string());
            println!(
                "set summary on view '{}' section {} column '{}' to {}",
                view.name,
                section,
                col_name,
                func.to_model().label()
            );
            Ok(())
        }
        ViewCommand::Section { command } => match command {
            ViewSectionCommand::Add {
                name,
                title,
                include,
                or_include,
                exclude,
                show_children,
            } => {
                let categories = store.get_hierarchy()?;
                let mut view = view_by_name(store, &name)?;
                ensure_mutable_view(&view)?;
                let section = Section {
                    title,
                    criteria: query_from_category_names(
                        &categories,
                        &include,
                        &or_include,
                        &exclude,
                    )?,
                    columns: Vec::new(),
                    item_column_index: 0,
                    on_insert_assign: HashSet::new(),
                    on_remove_unassign: HashSet::new(),
                    show_children,
                    board_display_mode_override: None,
                };
                view.sections.push(section);
                store.update_view(&view)?;
                println!(
                    "added section {} to view {}",
                    view.sections
                        .last()
                        .map(|section| section.title.as_str())
                        .unwrap_or("?"),
                    view.name
                );
                Ok(())
            }
            ViewSectionCommand::Remove { name, section } => {
                let mut view = view_by_name(store, &name)?;
                ensure_mutable_view(&view)?;
                if section >= view.sections.len() {
                    return Err(format!(
                        "section index {} out of range (view has {} sections)",
                        section,
                        view.sections.len()
                    )
                    .into());
                }
                let removed = view.sections.remove(section);
                store.update_view(&view)?;
                println!("removed section {} from view {}", removed.title, view.name);
                Ok(())
            }
            ViewSectionCommand::Update {
                name,
                section,
                title,
                include,
                or_include,
                exclude,
                clear_criteria,
                show_children,
            } => {
                let categories = store.get_hierarchy()?;
                let mut view = view_by_name(store, &name)?;
                ensure_mutable_view(&view)?;
                let has_criteria_flags = clear_criteria
                    || !include.is_empty()
                    || !or_include.is_empty()
                    || !exclude.is_empty();
                let section_ref = section_mut(&mut view, section)?;
                if let Some(title) = title {
                    section_ref.title = title;
                }
                if has_criteria_flags {
                    section_ref.criteria =
                        query_from_category_names(&categories, &include, &or_include, &exclude)?;
                }
                if let Some(show_children) = show_children {
                    section_ref.show_children = show_children;
                }
                store.update_view(&view)?;
                println!("updated section {} in view {}", section, view.name);
                Ok(())
            }
        },
        ViewCommand::Column { command } => match command {
            ViewColumnCommand::Add {
                name,
                section,
                column,
                kind,
                width,
                summary,
            } => {
                let categories = store.get_hierarchy()?;
                let category_names = category_name_map(&categories);
                let heading = category_id_by_name(&categories, &column)?;
                let mut view = view_by_name(store, &name)?;
                ensure_mutable_view(&view)?;
                let section_ref = section_mut(&mut view, section)?;
                let default_kind = if column.eq_ignore_ascii_case("When") {
                    CliColumnKind::When
                } else {
                    CliColumnKind::Standard
                };
                let _ = &category_names;
                section_ref.columns.push(Column {
                    kind: kind.unwrap_or(default_kind).to_model(),
                    heading,
                    width: width.unwrap_or(12),
                    summary_fn: summary.map(CliSummaryFn::to_model),
                });
                store.update_view(&view)?;
                println!(
                    "added column {} to view {} section {}",
                    column, view.name, section
                );
                Ok(())
            }
            ViewColumnCommand::Remove {
                name,
                section,
                column,
            } => {
                let categories = store.get_hierarchy()?;
                let category_names = category_name_map(&categories);
                let mut view = view_by_name(store, &name)?;
                ensure_mutable_view(&view)?;
                let section_ref = section_mut(&mut view, section)?;
                let column_index = find_column_index(section_ref, &category_names, &column)?;
                section_ref.columns.remove(column_index);
                store.update_view(&view)?;
                println!(
                    "removed column {} from view {} section {}",
                    column, view.name, section
                );
                Ok(())
            }
            ViewColumnCommand::Update {
                name,
                section,
                column,
                kind,
                width,
                summary,
            } => {
                let categories = store.get_hierarchy()?;
                let category_names = category_name_map(&categories);
                if kind.is_none() && width.is_none() && summary.is_none() {
                    return Err(
                        "nothing to update: specify --kind, --width, and/or --summary".into(),
                    );
                }
                let mut view = view_by_name(store, &name)?;
                ensure_mutable_view(&view)?;
                let section_ref = section_mut(&mut view, section)?;
                let column_index = find_column_index(section_ref, &category_names, &column)?;
                let column_ref = &mut section_ref.columns[column_index];
                if let Some(kind) = kind {
                    column_ref.kind = kind.to_model();
                }
                if let Some(width) = width {
                    column_ref.width = width;
                }
                if let Some(summary) = summary {
                    column_ref.summary_fn = Some(summary.to_model());
                }
                store.update_view(&view)?;
                println!(
                    "updated column {} in view {} section {}",
                    column, view.name, section
                );
                Ok(())
            }
        },
        ViewCommand::Alias { command } => match command {
            ViewAliasCommand::Set {
                name,
                category,
                alias,
            } => {
                let categories = store.get_hierarchy()?;
                let category_id = category_id_by_name(&categories, &category)?;
                let mut view = view_by_name(store, &name)?;
                ensure_mutable_view(&view)?;
                view.category_aliases.insert(category_id, alias.clone());
                store.update_view(&view)?;
                println!(
                    "set alias for {} in view {} to {}",
                    category, view.name, alias
                );
                Ok(())
            }
            ViewAliasCommand::Clear { name, category } => {
                let categories = store.get_hierarchy()?;
                let category_id = category_id_by_name(&categories, &category)?;
                let mut view = view_by_name(store, &name)?;
                ensure_mutable_view(&view)?;
                view.category_aliases.remove(&category_id);
                store.update_view(&view)?;
                println!("cleared alias for {} in view {}", category, view.name);
                Ok(())
            }
        },
        ViewCommand::SetItemLabel { name, label, clear } => {
            if clear && label.is_some() {
                return Err("--clear and <label> are mutually exclusive".into());
            }
            if !clear && label.is_none() {
                return Err("provide a label or pass --clear".into());
            }
            let mut view = view_by_name(store, &name)?;
            ensure_mutable_view(&view)?;
            view.item_column_label = if clear { None } else { label };
            store.update_view(&view)?;
            println!("updated item column label for view {}", view.name);
            Ok(())
        }
        ViewCommand::SetRemoveFromView {
            name,
            categories,
            clear,
        } => {
            if clear && !categories.is_empty() {
                return Err("--clear cannot be combined with category names".into());
            }
            if !clear && categories.is_empty() {
                return Err("provide one or more categories or pass --clear".into());
            }
            let hierarchy = store.get_hierarchy()?;
            let mut view = view_by_name(store, &name)?;
            ensure_mutable_view(&view)?;
            view.remove_from_view_unassign = if clear {
                HashSet::new()
            } else {
                names_to_category_ids(&hierarchy, &categories)?
            };
            store.update_view(&view)?;
            println!("updated remove-from-view categories for view {}", view.name);
            Ok(())
        }

        ViewCommand::CreateDatebook {
            name,
            period,
            interval,
            anchor,
            date_source,
        } => {
            let config = DatebookConfig {
                period: period.into_model(),
                interval: interval.into_model(),
                anchor: anchor.into_model(),
                date_source: date_source.into_model(),
                browse_offset: 0,
                browse_interval_offset: 0,
                ..Default::default()
            };
            if !config.is_valid() {
                return Err(format!(
                    "invalid datebook config: {} interval is too coarse for {} period",
                    config.interval.label(),
                    config.period.label(),
                )
                .into());
            }
            let mut view = View::new(name);
            view.datebook_config = Some(config);
            store.create_view(&view)?;
            println!("created datebook view \"{}\"", view.name);
            Ok(())
        }

        ViewCommand::DatebookBrowse { name, offset, step } => {
            let mut view = view_by_name(store, &name)?;
            ensure_mutable_view(&view)?;
            if view.datebook_config.is_none() {
                return Err(format!("\"{}\" is not a datebook view", view.name).into());
            }
            let config = view.datebook_config.as_mut().unwrap();
            if offset == 0 {
                config.browse_offset = 0;
                config.browse_interval_offset = 0;
            } else {
                match step {
                    BrowseStepArg::Window => config.browse_offset += offset,
                    BrowseStepArg::Bucket => config.browse_interval_offset += offset,
                }
            }
            let new_offset = config.browse_offset;
            let new_interval_offset = config.browse_interval_offset;
            store.update_view(&view)?;
            if new_interval_offset == 0 {
                println!("browse offset for \"{}\" set to {}", view.name, new_offset);
            } else {
                println!(
                    "browse offset for \"{}\" set to {} (bucket offset {})",
                    view.name, new_offset, new_interval_offset
                );
            }
            Ok(())
        }
    }
}
