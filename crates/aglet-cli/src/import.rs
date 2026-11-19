//! `import` subcommands.

use crate::*;

pub(crate) fn cmd_import(aglet: &Aglet<'_>, store: &Store, command: ImportCommand) -> CliResult {
    match command {
        ImportCommand::Csv {
            path,
            title_col,
            date_col,
            note_col,
            category_cols,
            category_parent,
            category_separator,
            vendor_cols,
            value_cols,
            assign,
            dry_run,
        } => {
            let global_assignments =
                resolve_tag_category_assignments(&store.get_hierarchy()?, &assign)?;
            let vendor_mappings: Vec<(String, String)> = vendor_cols
                .iter()
                .map(|spec| parse_source_parent_mapping(spec, "--vendor-col"))
                .collect::<Result<_, _>>()?;
            let value_mappings: Vec<(String, String)> = value_cols
                .iter()
                .map(|spec| parse_source_parent_mapping(spec, "--value-col"))
                .collect::<Result<_, _>>()?;

            let mut reader = csv::ReaderBuilder::new()
                .trim(csv::Trim::All)
                .from_path(&path)
                .map_err(|e| format!("failed to read CSV '{}': {e}", path.display()))?;
            let headers = reader.headers()?.clone();
            // The whole import is one transaction: a bad row leaves the
            // database untouched, and rows are not fsynced one at a time. A
            // dry run goes through the same path and is always rolled back.
            enum ImportStop {
                DryRun(usize),
                Failed(Box<dyn std::error::Error>),
            }
            impl From<Box<dyn std::error::Error>> for ImportStop {
                fn from(err: Box<dyn std::error::Error>) -> Self {
                    ImportStop::Failed(err)
                }
            }
            impl From<String> for ImportStop {
                fn from(message: String) -> Self {
                    ImportStop::Failed(message.into())
                }
            }
            impl From<AgletError> for ImportStop {
                fn from(err: AgletError) -> Self {
                    ImportStop::Failed(err.into())
                }
            }
            impl From<csv::Error> for ImportStop {
                fn from(err: csv::Error) -> Self {
                    ImportStop::Failed(err.into())
                }
            }

            let today = jiff::Zoned::now().date();
            let outcome = aglet.transaction(|| -> Result<Vec<ItemId>, ImportStop> {
                let mut created = Vec::new();
                let mut dry_run_rows = 0usize;

                for record in reader.records() {
                    let record = record?;
                    let title = csv_record_value(&record, &headers, &title_col)?;
                    if title.is_empty() {
                        continue;
                    }
                    let note = note_col
                        .as_deref()
                        .map(|column| csv_record_value(&record, &headers, column))
                        .transpose()?
                        .filter(|value| !value.is_empty());
                    let parsed_when = date_col
                        .as_deref()
                        .map(|column| csv_record_value(&record, &headers, column))
                        .transpose()?
                        .filter(|value| !value.is_empty())
                        .map(|value| parse_when_datetime_input(&value))
                        .transpose()?;

                    let mut row_tag_assignments = global_assignments.clone();
                    for column in &category_cols {
                        let raw = csv_record_value(&record, &headers, column)?;
                        for token in raw
                            .split(&category_separator)
                            .map(str::trim)
                            .filter(|token| !token.is_empty())
                        {
                            let category_id = ensure_category_exists(
                                aglet,
                                store,
                                token,
                                category_parent.as_deref(),
                                CategoryValueKind::Tag,
                            )?;
                            row_tag_assignments.push((category_id, token.to_string()));
                        }
                    }
                    for (source_column, parent_name) in &vendor_mappings {
                        let vendor_name = csv_record_value(&record, &headers, source_column)?;
                        if vendor_name.is_empty() {
                            continue;
                        }
                        let category_id = ensure_category_exists(
                            aglet,
                            store,
                            &vendor_name,
                            Some(parent_name),
                            CategoryValueKind::Tag,
                        )?;
                        row_tag_assignments.push((category_id, vendor_name));
                    }

                    let mut row_value_assignments = Vec::new();
                    for (source_column, category_name) in &value_mappings {
                        let raw_value = csv_record_value(&record, &headers, source_column)?;
                        if raw_value.is_empty() {
                            continue;
                        }
                        let value = parse_decimal_value(&raw_value)?;
                        let category_id = ensure_category_exists(
                            aglet,
                            store,
                            category_name,
                            None,
                            CategoryValueKind::Numeric,
                        )?;
                        row_value_assignments.push(NumericValueAssignment {
                            category_id,
                            category_name: category_name.clone(),
                            value,
                        });
                    }

                    if dry_run {
                        dry_run_rows += 1;
                        continue;
                    }

                    let mut item = Item::new(title);
                    item.note = note;
                    aglet.create_item_with(&item, SaveOptions::on(today).without_semantic())?;
                    if let Some(when_date) = parsed_when {
                        aglet.set_item_when_date(
                            item.id,
                            Some(when_date),
                            Some("manual:cli.when".to_string()),
                        )?;
                    }
                    for (category_id, category_name) in row_tag_assignments {
                        apply_tag_category_assignment(aglet, item.id, category_id, &category_name)?;
                    }
                    for assignment in row_value_assignments {
                        apply_numeric_value_assignment(aglet, item.id, assignment)?;
                    }
                    created.push(item.id);
                }

                if dry_run {
                    return Err(ImportStop::DryRun(dry_run_rows));
                }
                Ok(created)
            });

            let created = match outcome {
                Ok(created) => created,
                Err(ImportStop::DryRun(rows)) => {
                    println!("dry-run imported_rows={rows}");
                    return Ok(());
                }
                Err(ImportStop::Failed(message)) => return Err(message),
            };

            // LLM suggestions run after the commit so the write lock is never
            // held across network calls.
            for item_id in &created {
                aglet.classify_item_semantic_after_save(*item_id)?;
            }
            println!("imported_rows={}", created.len());
            Ok(())
        }
    }
}
