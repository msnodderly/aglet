use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use aglet_core::aglet::Aglet;
use aglet_core::date_rules::{parse_date_value_expr, render_date_condition};
use aglet_core::dates::{BasicDateParser, DateParser};
use aglet_core::error::AgletError;
use aglet_core::matcher::{unknown_hashtag_tokens, SubstringClassifier};
use aglet_core::model::{
    Action, Category, CategoryId, CategoryValueKind, Column, ColumnKind, Condition,
    ConditionMatchMode, Criterion, CriterionMode, DateCompareOp, DateMatcher, DateSource,
    DatebookAnchor, DatebookConfig, DatebookInterval, DatebookPeriod, Item, ItemId, Query, Section,
    SummaryFn, View,
};
use aglet_core::query::{
    category_name_map, compare_items_by_sort_keys, evaluate_query, resolve_view, SortDirection,
    SortField, SortKey,
};
use aglet_core::store::{Store, DEFAULT_VIEW_NAME};
use aglet_core::workflow::{
    blocked_item_ids, build_ready_queue_view, claimable_item_ids, resolve_workflow_config,
    retain_items_by_dependency_state, workflow_setup_error_message, READY_QUEUE_VIEW_NAME,
};
use clap::{Parser, Subcommand, ValueEnum};
use jiff::civil::{Date, DateTime};
use rust_decimal::Decimal;
use serde::Serialize;
use uuid::Uuid;

mod args;
mod category;
mod import;
mod item;
mod link;
mod output;
mod resolve;
mod view;

use args::*;
use category::*;
use import::*;
use item::*;
use link::*;
use output::*;
use resolve::*;
use view::*;

/// Command result. Any error type converts with `?`; `main` prints its message.
type CliResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn run() -> CliResult {
    let cli = Cli::parse();
    let db_path = resolve_db_path(cli.db)?;
    if let Some(debug) = tui_launch_debug(&cli.command) {
        return Ok(aglet_tui::run_with_options(&db_path, debug)?);
    }
    let command = cli.command.expect("non-TUI command should be present");

    let store = Store::open(&db_path)?;
    let classifier = SubstringClassifier;
    let aglet = Aglet::new(&store, &classifier);
    temporal_reevaluate_before_command(&aglet)?;

    match command {
        Command::Add {
            text,
            note,
            when,
            categories,
            values,
        } => cmd_add(&aglet, text, note, when, categories, values),
        Command::Edit {
            item_id,
            text,
            note,
            append_note,
            note_stdin: note_stdin_flag,
            clear_note,
            done,
            when,
            clear_when,
            recurrence,
            clear_recurrence,
        } => {
            let note_stdin = if note_stdin_flag {
                let mut stdin = io::stdin().lock();
                Some(read_note_from_stdin(&mut stdin)?)
            } else {
                None
            };
            cmd_edit(
                &aglet,
                item_id,
                text,
                note,
                append_note,
                note_stdin,
                clear_note,
                done,
                when,
                clear_when,
                recurrence,
                clear_recurrence,
            )
        }
        Command::Show { item_id } => cmd_show(&store, item_id),
        Command::Claim { item_id } => cmd_claim(&aglet, &store, item_id),
        Command::Ready { sort, format } => cmd_ready(&store, sort, format),
        Command::Release { item_id } => cmd_release(&aglet, &store, item_id),
        Command::List {
            view,
            category,
            any_category,
            exclude_category,
            blocked,
            not_blocked,
            value_eq,
            value_in,
            value_max,
            sort,
            format,
            include_done,
            verbose,
        } => cmd_list(
            &store,
            view,
            ListFilters {
                all_categories: category,
                any_categories: any_category,
                exclude_categories: exclude_category,
                dependency_state_filter: dependency_state_filter_from_flags(blocked, not_blocked),
                value_eq,
                value_in,
                value_max,
                include_done,
            },
            sort,
            format,
            table_style_from_verbose(verbose),
        ),
        Command::Search {
            query,
            format,
            blocked,
            not_blocked,
            include_done,
            verbose,
        } => cmd_search(
            &store,
            query,
            format,
            dependency_state_filter_from_flags(blocked, not_blocked),
            include_done,
            table_style_from_verbose(verbose),
        ),
        Command::Export {
            view,
            include_links,
        } => cmd_export(&store, view, include_links),
        Command::Delete { item_id } => cmd_delete(&aglet, item_id),
        Command::Deleted => cmd_deleted(&store),
        Command::Restore { log_id } => cmd_restore(&store, log_id),
        Command::Category { command } => cmd_category(&aglet, &store, command),
        Command::View { command } => cmd_view(&aglet, &store, command),
        Command::Import { command } => cmd_import(&aglet, &store, command),
        Command::Link { command } => cmd_link(&aglet, command),
        Command::Unlink { command } => cmd_unlink(&aglet, command),
        Command::Item(item_cmd) => match item_cmd {
            ItemCommand::Add {
                text,
                note,
                when,
                categories,
                values,
            } => cmd_add(&aglet, text, note, when, categories, values),
            ItemCommand::List {
                view,
                category,
                any_category,
                exclude_category,
                blocked,
                not_blocked,
                value_eq,
                value_in,
                value_max,
                sort,
                format,
                include_done,
                verbose,
            } => cmd_list(
                &store,
                view,
                ListFilters {
                    all_categories: category,
                    any_categories: any_category,
                    exclude_categories: exclude_category,
                    dependency_state_filter: dependency_state_filter_from_flags(
                        blocked,
                        not_blocked,
                    ),
                    value_eq,
                    value_in,
                    value_max,
                    include_done,
                },
                sort,
                format,
                table_style_from_verbose(verbose),
            ),
            ItemCommand::Show { item_id } => cmd_show(&store, item_id),
            ItemCommand::Edit {
                item_id,
                text,
                note,
                append_note,
                note_stdin: note_stdin_flag,
                clear_note,
                done,
                when,
                clear_when,
                recurrence,
                clear_recurrence,
            } => {
                let note_stdin = if note_stdin_flag {
                    let mut stdin = io::stdin().lock();
                    Some(read_note_from_stdin(&mut stdin)?)
                } else {
                    None
                };
                cmd_edit(
                    &aglet,
                    item_id,
                    text,
                    note,
                    append_note,
                    note_stdin,
                    clear_note,
                    done,
                    when,
                    clear_when,
                    recurrence,
                    clear_recurrence,
                )
            }
            ItemCommand::Delete { item_id } => cmd_delete(&aglet, item_id),
        },
        Command::Tui { .. } => Ok(()),
    }
}

fn tui_launch_debug(command: &Option<Command>) -> Option<bool> {
    match command {
        None => Some(false),
        Some(Command::Tui { debug }) => Some(*debug),
        Some(_) => None,
    }
}

fn resolve_db_path(db_opt: Option<PathBuf>) -> CliResult<PathBuf> {
    if let Some(path) = db_opt {
        ensure_db_parent(&path)?;
        return Ok(path);
    }

    if let Ok(legacy) = env::var("AGENDA_DB") {
        if !legacy.is_empty() {
            let path = PathBuf::from(legacy);
            ensure_db_parent(&path)?;
            return Ok(path);
        }
    }

    let home = env::var("HOME").map_err(|_| "HOME is not set".to_string())?;
    let new_path = PathBuf::from(&home).join(".aglet").join("default.ag");
    let legacy_path = PathBuf::from(&home).join(".agenda").join("default.ag");

    if !new_path.exists() && legacy_path.exists() {
        eprintln!(
            "note: using legacy database at {} (set AGLET_DB or --db to override)",
            legacy_path.display()
        );
        ensure_db_parent(&legacy_path)?;
        return Ok(legacy_path);
    }

    ensure_db_parent(&new_path)?;
    Ok(new_path)
}

fn ensure_db_parent(path: &Path) -> CliResult {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;
