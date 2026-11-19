//! Rendering: tables, JSON rows, Markdown export, and category trees.

use crate::*;

#[derive(Serialize)]
pub(crate) struct JsonItemRow {
    pub(crate) id: String,
    pub(crate) text: String,
    pub(crate) status: String,
    pub(crate) is_done: bool,
    pub(crate) when: Option<String>,
    pub(crate) categories: Vec<String>,
    pub(crate) note: Option<String>,
    /// Configured section column values (present only when the section
    /// defines columns).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) column_values: Option<Vec<JsonColumnValueOutput>>,
}

#[derive(Serialize)]
pub(crate) struct JsonColumnValueOutput {
    pub(crate) column: String,
    pub(crate) value: String,
}

#[derive(Serialize)]
pub(crate) struct JsonItemsOutput {
    pub(crate) items: Vec<JsonItemRow>,
}

#[derive(Serialize)]
pub(crate) struct JsonViewSubsectionOutput {
    pub(crate) title: String,
    pub(crate) items: Vec<JsonItemRow>,
    pub(crate) summaries: Vec<String>,
}

#[derive(Serialize)]
pub(crate) struct JsonViewSectionOutput {
    pub(crate) title: String,
    pub(crate) items: Vec<JsonItemRow>,
    pub(crate) subsections: Vec<JsonViewSubsectionOutput>,
    pub(crate) summaries: Vec<String>,
}

#[derive(Serialize)]
pub(crate) struct JsonViewCategoryAliasOutput {
    pub(crate) category_id: String,
    pub(crate) category: String,
    pub(crate) alias: String,
}

#[derive(Serialize)]
pub(crate) struct JsonViewOutput {
    pub(crate) view: String,
    pub(crate) hide_dependent_items: bool,
    pub(crate) category_aliases: Vec<JsonViewCategoryAliasOutput>,
    pub(crate) sections: Vec<JsonViewSectionOutput>,
    pub(crate) unmatched_label: Option<String>,
    pub(crate) unmatched: Option<Vec<JsonItemRow>>,
}

pub(crate) struct ViewCategoryAliasRow {
    pub(crate) category_id: CategoryId,
    pub(crate) category_name: String,
    pub(crate) alias: String,
}

pub(crate) fn sorted_rows<'a>(
    items: &'a [Item],
    sort_keys: &[SortKey],
    categories: &[Category],
) -> Vec<&'a Item> {
    let mut rows: Vec<&Item> = items.iter().collect();
    if !sort_keys.is_empty() {
        rows.sort_by(|left, right| compare_items_by_sort_keys(left, right, sort_keys, categories));
    }
    rows
}

pub(crate) fn item_categories(
    item: &Item,
    category_names: &HashMap<CategoryId, String>,
) -> Vec<String> {
    let mut names: Vec<String> = item
        .assignments
        .keys()
        .filter_map(|id| category_names.get(id))
        .cloned()
        .collect();
    names.sort_by_key(|name| name.to_ascii_lowercase());
    names
}

pub(crate) fn item_row(item: &Item, category_names: &HashMap<CategoryId, String>) -> JsonItemRow {
    JsonItemRow {
        id: item.id.to_string(),
        text: item.text.clone(),
        status: if item.is_done {
            "done".to_string()
        } else {
            "open".to_string()
        },
        is_done: item.is_done,
        when: item.when_date.map(|dt| dt.to_string()),
        categories: item_categories(item, category_names),
        note: item.note.clone(),
        column_values: None,
    }
}

pub(crate) fn rows_to_json(
    items: &[Item],
    category_names: &HashMap<CategoryId, String>,
    sort_keys: &[SortKey],
    categories: &[Category],
) -> Vec<JsonItemRow> {
    sorted_rows(items, sort_keys, categories)
        .into_iter()
        .map(|item| item_row(item, category_names))
        .collect()
}

/// Like `rows_to_json`, attaching configured section column values when the
/// section defines columns (UX audit P2-CLI-1).
pub(crate) fn rows_to_json_for_section(
    view: &View,
    section_index: usize,
    items: &[Item],
    category_names: &HashMap<CategoryId, String>,
    sort_keys: &[SortKey],
    categories: &[Category],
) -> Vec<JsonItemRow> {
    let columns = view
        .sections
        .get(section_index)
        .map(|section| section.columns.as_slice())
        .unwrap_or(&[]);
    sorted_rows(items, sort_keys, categories)
        .into_iter()
        .map(|item| {
            let mut row = item_row(item, category_names);
            if !columns.is_empty() {
                row.column_values = Some(
                    columns
                        .iter()
                        .map(|column| JsonColumnValueOutput {
                            column: aglet_core::query::section_column_header(
                                view,
                                column,
                                category_names,
                            ),
                            value: aglet_core::query::section_column_cell(
                                item,
                                column,
                                categories,
                                category_names,
                            ),
                        })
                        .collect(),
                );
            }
            row
        })
        .collect()
}

pub(crate) fn markdown_sorted_rows(items: &[Item]) -> Vec<&Item> {
    let mut rows: Vec<&Item> = items.iter().collect();
    rows.sort_by(|left, right| {
        left.text
            .to_ascii_lowercase()
            .cmp(&right.text.to_ascii_lowercase())
            .then_with(|| left.id.cmp(&right.id))
    });
    rows
}

pub(crate) fn append_markdown_items(
    out: &mut String,
    items: &[Item],
    category_names: &HashMap<CategoryId, String>,
    store: &Store,
    include_links: bool,
    heading_prefix: &str,
) -> CliResult {
    let rows = markdown_sorted_rows(items);
    if rows.is_empty() {
        out.push_str("(no items)\n");
        return Ok(());
    }

    for (index, item) in rows.into_iter().enumerate() {
        if index > 0 {
            out.push('\n');
        }
        out.push_str(&format!("{heading_prefix} {}\n", item.text));
        out.push_str(&format!("- ID: `{}`\n", item.id));
        out.push_str(&format!(
            "- Status: `{}`\n",
            if item.is_done { "done" } else { "open" }
        ));
        out.push_str(&format!(
            "- When: `{}`\n",
            item.when_date
                .map(|dt| dt.to_string())
                .unwrap_or_else(|| "-".to_string())
        ));

        let categories = item_categories(item, category_names);
        if categories.is_empty() {
            out.push_str("- Categories: (none)\n");
        } else {
            out.push_str(&format!("- Categories: {}\n", categories.join(", ")));
        }

        if let Some(note) = &item.note {
            out.push_str("- Note:\n");
            out.push_str("```text\n");
            out.push_str(note);
            if !note.ends_with('\n') {
                out.push('\n');
            }
            out.push_str("```\n");
        } else {
            out.push_str("- Note: (none)\n");
        }

        if include_links {
            let link_lines = item_link_section_lines(store, item.id)?;
            out.push_str("- Links:\n");
            out.push_str("```text\n");
            for line in link_lines {
                out.push_str(&line);
                out.push('\n');
            }
            out.push_str("```\n");
        }
    }

    Ok(())
}

pub(crate) fn build_markdown_export(
    store: &Store,
    view_name: Option<&str>,
    include_links: bool,
) -> CliResult<String> {
    let items = store.list_items()?;
    let categories = store.get_hierarchy()?;
    let category_names = category_name_map(&categories);
    let mut out = String::new();

    if let Some(name) = view_name {
        let view = view_by_name(store, name)?;
        out.push_str(&format!("# {}\n\n", view.name));

        let reference_date = jiff::Zoned::now().date();
        let result = resolve_view(&view, &items, &categories, reference_date);
        let mut rendered_any = false;

        for section in result.sections {
            out.push_str(&format!("## {}\n\n", section.title));
            if section.subsections.is_empty() {
                append_markdown_items(
                    &mut out,
                    &section.items,
                    &category_names,
                    store,
                    include_links,
                    "###",
                )?;
                out.push('\n');
                rendered_any = true;
                continue;
            }

            for subsection in section.subsections {
                out.push_str(&format!("### {}\n\n", subsection.title));
                append_markdown_items(
                    &mut out,
                    &subsection.items,
                    &category_names,
                    store,
                    include_links,
                    "####",
                )?;
                out.push('\n');
                rendered_any = true;
            }
        }

        if let Some(unmatched) = result.unmatched {
            if !unmatched.is_empty() {
                let heading = result
                    .unmatched_label
                    .unwrap_or_else(|| "Unassigned".to_string());
                out.push_str(&format!("## {}\n\n", heading));
                append_markdown_items(
                    &mut out,
                    &unmatched,
                    &category_names,
                    store,
                    include_links,
                    "###",
                )?;
                out.push('\n');
                rendered_any = true;
            }
        }

        if !rendered_any {
            out.push_str("(no items)\n");
        }
    } else {
        out.push_str("# Items\n\n");
        append_markdown_items(
            &mut out,
            &items,
            &category_names,
            store,
            include_links,
            "##",
        )?;
        out.push('\n');
    }

    if !out.ends_with('\n') {
        out.push('\n');
    }
    Ok(out)
}

pub(crate) fn print_items_json(
    items: &[Item],
    category_names: &HashMap<CategoryId, String>,
    sort_keys: &[SortKey],
    categories: &[Category],
) -> CliResult {
    let payload = JsonItemsOutput {
        items: rows_to_json(items, category_names, sort_keys, categories),
    };
    let body = serde_json::to_string_pretty(&payload)?;
    println!("{body}");
    Ok(())
}

pub(crate) fn view_category_alias_rows(
    view: &View,
    category_names: &HashMap<CategoryId, String>,
) -> Vec<ViewCategoryAliasRow> {
    let mut rows: Vec<ViewCategoryAliasRow> = view
        .category_aliases
        .iter()
        .filter_map(|(category_id, alias)| {
            let alias = alias.trim();
            if alias.is_empty() {
                return None;
            }
            let category_name = category_names
                .get(category_id)
                .cloned()
                .unwrap_or_else(|| format!("(deleted:{category_id})"));
            Some(ViewCategoryAliasRow {
                category_id: *category_id,
                category_name,
                alias: alias.to_string(),
            })
        })
        .collect();
    rows.sort_by(|left, right| {
        left.category_name
            .to_ascii_lowercase()
            .cmp(&right.category_name.to_ascii_lowercase())
            .then_with(|| {
                left.alias
                    .to_ascii_lowercase()
                    .cmp(&right.alias.to_ascii_lowercase())
            })
            .then_with(|| left.category_id.cmp(&right.category_id))
    });
    rows
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn print_items_for_view(
    view: &View,
    items: &[Item],
    categories: &[Category],
    category_names: &HashMap<CategoryId, String>,
    sort_keys: &[SortKey],
    output_format: OutputFormatArg,
    blocked_item_ids: &HashSet<ItemId>,
    table_style: TableStyle,
) -> CliResult {
    let reference_date = jiff::Zoned::now().date();
    let mut result = resolve_view(view, items, categories, reference_date);
    if view.hide_dependent_items {
        for section in &mut result.sections {
            section
                .items
                .retain(|item| !blocked_item_ids.contains(&item.id));
            for subsection in &mut section.subsections {
                subsection
                    .items
                    .retain(|item| !blocked_item_ids.contains(&item.id));
            }
        }
        if let Some(unmatched) = &mut result.unmatched {
            unmatched.retain(|item| !blocked_item_ids.contains(&item.id));
        }
    }
    let has_sections = !result.sections.is_empty();
    let alias_rows = view_category_alias_rows(view, category_names);

    if output_format == OutputFormatArg::Json {
        let mut sections = Vec::new();
        for section in result.sections {
            let summaries = section_summary_entries(
                view,
                section.section_index,
                &section.items,
                categories,
                category_names,
            );
            if section.subsections.is_empty() {
                sections.push(JsonViewSectionOutput {
                    title: section.title,
                    items: rows_to_json_for_section(
                        view,
                        section.section_index,
                        &section.items,
                        category_names,
                        sort_keys,
                        categories,
                    ),
                    subsections: Vec::new(),
                    summaries,
                });
                continue;
            }

            let mut subsections = Vec::new();
            for subsection in section.subsections {
                let subsection_summaries = section_summary_entries(
                    view,
                    section.section_index,
                    &subsection.items,
                    categories,
                    category_names,
                );
                subsections.push(JsonViewSubsectionOutput {
                    title: subsection.title,
                    items: rows_to_json_for_section(
                        view,
                        section.section_index,
                        &subsection.items,
                        category_names,
                        sort_keys,
                        categories,
                    ),
                    summaries: subsection_summaries,
                });
            }

            sections.push(JsonViewSectionOutput {
                title: section.title,
                items: Vec::new(),
                subsections,
                summaries,
            });
        }

        let unmatched = result.unmatched.and_then(|rows| {
            if rows.is_empty() {
                None
            } else {
                Some(rows_to_json(&rows, category_names, sort_keys, categories))
            }
        });
        let unmatched_label = if unmatched.is_some() {
            Some(
                result
                    .unmatched_label
                    .unwrap_or_else(|| "Unassigned".to_string()),
            )
        } else {
            None
        };

        let payload = JsonViewOutput {
            view: view.name.clone(),
            hide_dependent_items: view.hide_dependent_items,
            category_aliases: alias_rows
                .iter()
                .map(|row| JsonViewCategoryAliasOutput {
                    category_id: row.category_id.to_string(),
                    category: row.category_name.clone(),
                    alias: row.alias.clone(),
                })
                .collect(),
            sections,
            unmatched_label,
            unmatched,
        };
        let body = serde_json::to_string_pretty(&payload)?;
        println!("{body}");
        return Ok(());
    }

    println!("# {}", view.name);
    println!("hide_dependent_items: {}", view.hide_dependent_items);
    if !alias_rows.is_empty() {
        println!("\nAliases:");
        for row in &alias_rows {
            println!("- {} => {}", row.category_name, row.alias);
        }
    }

    for section in result.sections {
        let section_index = section.section_index;
        println!("\n## {}", section.title);
        if section.subsections.is_empty() {
            if let Some(table) = render_section_column_table(
                view,
                section_index,
                &section.items,
                category_names,
                sort_keys,
                categories,
            ) {
                // Configured columns render as real table columns with an
                // inline TOTAL row; the definitions line and the separate
                // summary line are redundant here.
                print!("{table}");
            } else {
                print_item_table(
                    &section.items,
                    category_names,
                    sort_keys,
                    categories,
                    table_style,
                );
                if let Some(summary_line) = section_summary_line(
                    view,
                    section_index,
                    &section.items,
                    categories,
                    category_names,
                ) {
                    println!("{summary_line}");
                }
            }
            continue;
        }

        for subsection in section.subsections {
            println!("\n### {}", subsection.title);
            if let Some(table) = render_section_column_table(
                view,
                section_index,
                &subsection.items,
                category_names,
                sort_keys,
                categories,
            ) {
                print!("{table}");
            } else {
                print_item_table(
                    &subsection.items,
                    category_names,
                    sort_keys,
                    categories,
                    table_style,
                );
                if let Some(summary_line) = section_summary_line(
                    view,
                    section_index,
                    &subsection.items,
                    categories,
                    category_names,
                ) {
                    println!("{summary_line}");
                }
            }
        }
    }

    if let Some(unmatched) = result.unmatched {
        if !unmatched.is_empty() {
            if !has_sections {
                print_item_table(
                    &unmatched,
                    category_names,
                    sort_keys,
                    categories,
                    table_style,
                );
                return Ok(());
            }

            let heading = result
                .unmatched_label
                .unwrap_or_else(|| "Unassigned".to_string());
            let heading = if heading == "Unassigned" {
                "Other".to_string()
            } else {
                heading
            };
            println!("\n## {}", heading);
            print_item_table(
                &unmatched,
                category_names,
                sort_keys,
                categories,
                table_style,
            );
        }
    }
    Ok(())
}

pub(crate) fn section_summary_line(
    view: &View,
    section_index: usize,
    items: &[Item],
    categories: &[Category],
    category_names: &HashMap<CategoryId, String>,
) -> Option<String> {
    let entries = section_summary_entries(view, section_index, items, categories, category_names);
    if entries.is_empty() {
        return None;
    }
    Some(format!("summary: {}", entries.join(" | ")))
}

pub(crate) fn section_summary_entries(
    view: &View,
    section_index: usize,
    items: &[Item],
    categories: &[Category],
    category_names: &HashMap<CategoryId, String>,
) -> Vec<String> {
    let Some(section) = view.sections.get(section_index) else {
        return Vec::new();
    };

    let categories_by_id: HashMap<CategoryId, &Category> = categories
        .iter()
        .map(|category| (category.id, category))
        .collect();
    section
        .columns
        .iter()
        .filter_map(|column| {
            let summary_fn = column.summary_fn.unwrap_or(SummaryFn::None);
            if summary_fn == SummaryFn::None {
                return None;
            }

            let value = column_summary_value(summary_fn, column.heading, items, &categories_by_id)?;
            let heading = category_names
                .get(&column.heading)
                .cloned()
                .unwrap_or_else(|| format!("(deleted:{})", column.heading));
            Some(format!(
                "{}({})={}",
                heading,
                summary_fn_label(summary_fn),
                value.normalize()
            ))
        })
        .collect()
}

pub(crate) fn column_summary_value(
    summary_fn: SummaryFn,
    heading_id: CategoryId,
    items: &[Item],
    categories_by_id: &HashMap<CategoryId, &Category>,
) -> Option<Decimal> {
    if categories_by_id
        .get(&heading_id)
        .map(|category| category.value_kind != CategoryValueKind::Numeric)
        .unwrap_or(true)
    {
        return None;
    }

    let values: Vec<Decimal> = items
        .iter()
        .filter_map(|item| {
            item.assignments
                .get(&heading_id)
                .and_then(|assignment| assignment.numeric_value)
        })
        .collect();

    match summary_fn {
        SummaryFn::None => None,
        SummaryFn::Sum => {
            if values.is_empty() {
                None
            } else {
                Some(values.iter().copied().sum())
            }
        }
        SummaryFn::Avg => {
            if values.is_empty() {
                None
            } else {
                let sum: Decimal = values.iter().copied().sum();
                Some(sum / Decimal::from(values.len() as u32))
            }
        }
        SummaryFn::Min => values.iter().copied().min(),
        SummaryFn::Max => values.iter().copied().max(),
        SummaryFn::Count => Some(Decimal::from(values.len() as u32)),
    }
}

pub(crate) fn summary_fn_label(summary_fn: SummaryFn) -> &'static str {
    match summary_fn {
        SummaryFn::None => "none",
        SummaryFn::Sum => "sum",
        SummaryFn::Avg => "avg",
        SummaryFn::Min => "min",
        SummaryFn::Max => "max",
        SummaryFn::Count => "count",
    }
}

/// Compact one-line rows (UX audit P2-CLI-2): 8-char id, honest DONE?
/// column, humane date, title (with a note glyph), and direct leaf
/// categories only. `--verbose` restores the original multi-line format.
pub(crate) fn render_compact_item_table(
    items: &[Item],
    category_names: &HashMap<CategoryId, String>,
    sort_keys: &[SortKey],
    categories: &[Category],
) -> String {
    if items.is_empty() {
        return "(no items)\n".to_string();
    }

    let rows = sorted_rows(items, sort_keys, categories);
    let body: Vec<(String, &str, String, String)> = rows
        .iter()
        .map(|item| {
            let id8: String = item.id.to_string().chars().take(8).collect();
            let done = if item.is_done { "done" } else { "open" };
            let when = item
                .when_date
                .map(aglet_core::dates::format_human_datetime)
                .unwrap_or_else(|| "-".to_string());
            let mut title = item.text.clone();
            if item.note.as_deref().is_some_and(|note| !note.is_empty()) {
                title.push_str(" \u{266A}");
            }
            let leaf_ids = aglet_core::query::display_category_ids(item, categories);
            if !leaf_ids.is_empty() {
                let names: Vec<String> = leaf_ids
                    .iter()
                    .map(|category_id| {
                        category_names
                            .get(category_id)
                            .cloned()
                            .unwrap_or_else(|| category_id.to_string())
                    })
                    .collect();
                title.push_str(&format!("  [{}]", names.join(", ")));
            }
            (id8, done, when, title)
        })
        .collect();

    let when_width = body
        .iter()
        .map(|(_, _, when, _)| when.chars().count())
        .max()
        .unwrap_or(4)
        .max("WHEN".len());

    let mut out = String::new();
    out.push_str(&format!(
        "{:<8}  {:<5}  {:<when_width$}  TITLE\n",
        "ID",
        "DONE?",
        "WHEN",
        when_width = when_width
    ));
    out.push_str(&format!(
        "{}  {}  {}  -----\n",
        "-".repeat(8),
        "-".repeat(5),
        "-".repeat(when_width)
    ));
    for (id8, done, when, title) in body {
        out.push_str(&format!(
            "{:<8}  {:<5}  {:<when_width$}  {}\n",
            id8,
            done,
            when,
            title,
            when_width = when_width
        ));
    }
    out
}

/// Renders a section's items as a table with the configured section columns
/// as real columns (numeric formatting matches the TUI cells) and a TOTAL
/// row when any column has a summary configured (UX audit P2-CLI-1).
/// Returns `None` when the section defines no columns (caller falls back to
/// the generic table).
pub(crate) fn render_section_column_table(
    view: &View,
    section_index: usize,
    items: &[Item],
    category_names: &HashMap<CategoryId, String>,
    sort_keys: &[SortKey],
    categories: &[Category],
) -> Option<String> {
    let section = view.sections.get(section_index)?;
    if section.columns.is_empty() {
        return None;
    }

    let rows = sorted_rows(items, sort_keys, categories);
    let categories_by_id: HashMap<CategoryId, &Category> = categories
        .iter()
        .map(|category| (category.id, category))
        .collect();

    // Cell matrix: ID, STATUS, then section columns with TITLE spliced in at
    // the section's item column position.
    let item_column_index = section.item_column_index.min(section.columns.len());
    let mut headers: Vec<String> = vec!["ID".to_string(), "STATUS".to_string()];
    for (idx, column) in section.columns.iter().enumerate() {
        if idx == item_column_index {
            headers.push("TITLE".to_string());
        }
        headers.push(aglet_core::query::section_column_header(
            view,
            column,
            category_names,
        ));
    }
    if item_column_index >= section.columns.len() {
        headers.push("TITLE".to_string());
    }

    let mut body: Vec<Vec<String>> = Vec::new();
    for item in &rows {
        let mut cells: Vec<String> = vec![
            item.id.to_string(),
            if item.is_done { "done" } else { "open" }.to_string(),
        ];
        for (idx, column) in section.columns.iter().enumerate() {
            if idx == item_column_index {
                cells.push(item.text.clone());
            }
            cells.push(aglet_core::query::section_column_cell(
                item,
                column,
                categories,
                category_names,
            ));
        }
        if item_column_index >= section.columns.len() {
            cells.push(item.text.clone());
        }
        body.push(cells);
    }

    // TOTAL row when any column has a summary configured.
    let has_summaries = section
        .columns
        .iter()
        .any(|column| column.summary_fn.unwrap_or(SummaryFn::None) != SummaryFn::None);
    let mut totals: Option<Vec<String>> = None;
    if has_summaries {
        let mut cells: Vec<String> = vec![String::new(), String::new()];
        for (idx, column) in section.columns.iter().enumerate() {
            if idx == item_column_index {
                cells.push("TOTAL".to_string());
            }
            let summary_fn = column.summary_fn.unwrap_or(SummaryFn::None);
            let rendered = if summary_fn == SummaryFn::None {
                String::new()
            } else {
                column_summary_value(summary_fn, column.heading, items, &categories_by_id)
                    .map(|value| {
                        let format = categories_by_id
                            .get(&column.heading)
                            .and_then(|category| category.numeric_format.as_ref());
                        format!(
                            "{} ({})",
                            aglet_core::numeric_format::format_numeric_cell(Some(value), format),
                            summary_fn_label(summary_fn)
                        )
                    })
                    .unwrap_or_default()
            };
            cells.push(rendered);
        }
        if item_column_index >= section.columns.len() {
            cells.push("TOTAL".to_string());
        }
        totals = Some(cells);
    }

    let mut widths: Vec<usize> = headers
        .iter()
        .map(|header| header.chars().count())
        .collect();
    for row in body.iter().chain(totals.iter()) {
        for (idx, cell) in row.iter().enumerate() {
            widths[idx] = widths[idx].max(cell.chars().count());
        }
    }

    let render_row = |cells: &[String]| -> String {
        cells
            .iter()
            .enumerate()
            .map(|(idx, cell)| format!("{:<width$}", cell, width = widths[idx]))
            .collect::<Vec<_>>()
            .join("  ")
            .trim_end()
            .to_string()
    };

    let mut out = String::new();
    out.push_str(&render_row(&headers));
    out.push('\n');
    out.push_str(
        &widths
            .iter()
            .map(|width| "-".repeat(*width))
            .collect::<Vec<_>>()
            .join("  "),
    );
    out.push('\n');
    if body.is_empty() {
        out.push_str("(no items)\n");
    }
    for row in &body {
        out.push_str(&render_row(row));
        out.push('\n');
    }
    if let Some(totals) = totals {
        out.push_str(
            &widths
                .iter()
                .map(|width| "-".repeat(*width))
                .collect::<Vec<_>>()
                .join("  "),
        );
        out.push('\n');
        out.push_str(&render_row(&totals));
        out.push('\n');
    }
    Some(out)
}

pub(crate) fn print_item_table(
    items: &[Item],
    category_names: &HashMap<CategoryId, String>,
    sort_keys: &[SortKey],
    categories: &[Category],
    style: TableStyle,
) {
    if style == TableStyle::Compact {
        print!(
            "{}",
            render_compact_item_table(items, category_names, sort_keys, categories)
        );
        return;
    }
    if items.is_empty() {
        println!("(no items)");
        return;
    }

    let rows = sorted_rows(items, sort_keys, categories);
    let id_width = rows
        .iter()
        .map(|item| item.id.to_string().len())
        .max()
        .unwrap_or(8)
        .max(8);
    let status_width = 6usize;
    let when_width = 19usize;

    println!(
        "{:<id_width$}  {:<status_width$}  {:<when_width$}  TITLE",
        "ID",
        "STATUS",
        "WHEN",
        id_width = id_width,
        status_width = status_width,
        when_width = when_width
    );
    println!(
        "{}  {}  {}  -----",
        "-".repeat(id_width),
        "-".repeat(status_width),
        "-".repeat(when_width)
    );

    for item in rows {
        let when = item
            .when_date
            .map(|dt| dt.to_string())
            .unwrap_or_else(|| "-".to_string());
        let status = if item.is_done { "done" } else { "open" };
        println!(
            "{:<id_width$}  {:<status_width$}  {:<when_width$}  {}",
            item.id,
            status,
            when,
            item.text,
            id_width = id_width,
            status_width = status_width,
            when_width = when_width
        );

        let categories = item_categories(item, category_names);
        if !categories.is_empty() {
            println!(
                "{:<id_width$}  {:<status_width$}  {:<when_width$}  categories: {}",
                "",
                "",
                "",
                categories.join(", "),
                id_width = id_width,
                status_width = status_width,
                when_width = when_width
            );
        }

        if let Some(note) = &item.note {
            println!(
                "{:<id_width$}  {:<status_width$}  {:<when_width$}  note: {}",
                "",
                "",
                "",
                note.replace('\n', " "),
                id_width = id_width,
                status_width = status_width,
                when_width = when_width
            );
        }
    }
}

pub(crate) fn print_category_tree(categories: &[Category]) {
    let by_id: HashMap<CategoryId, &Category> = categories.iter().map(|c| (c.id, c)).collect();

    let mut roots: Vec<&Category> = categories.iter().filter(|c| c.parent.is_none()).collect();
    roots.sort_by_key(|c| c.name.to_ascii_lowercase());

    for root in roots {
        print_category_subtree(root, 0, &by_id);
    }
}

pub(crate) fn print_category_subtree(
    category: &Category,
    depth: usize,
    by_id: &HashMap<CategoryId, &Category>,
) {
    let indent = "  ".repeat(depth);
    let flags = format!(
        "{}{}{}{}",
        if category.is_exclusive {
            " [exclusive]"
        } else {
            ""
        },
        if !category.enable_implicit_string {
            " [no-implicit-string]"
        } else {
            ""
        },
        if !category.is_actionable {
            " [non-actionable]"
        } else {
            ""
        },
        if category.value_kind == CategoryValueKind::Numeric {
            " [numeric]"
        } else {
            ""
        }
    );
    println!("{}- {}{}", indent, category.name, flags);

    for child_id in &category.children {
        if let Some(child) = by_id.get(child_id) {
            print_category_subtree(child, depth + 1, by_id);
        }
    }
}
