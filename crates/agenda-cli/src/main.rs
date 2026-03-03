use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;

use agenda_core::agenda::Agenda;
use agenda_core::error::AgendaError;
use agenda_core::matcher::{unknown_hashtag_tokens, SubstringClassifier};
use agenda_core::model::{
    Category, CategoryId, CategoryValueKind, CriterionMode, Item, ItemId, Query, View,
};
use agenda_core::query::{evaluate_query, resolve_view};
use agenda_core::store::Store;
use chrono::{Local, NaiveDateTime};
use clap::{Parser, Subcommand, ValueEnum};
use rust_decimal::Decimal;
use serde::Serialize;
use uuid::Uuid;

#[derive(Parser, Debug)]
#[command(name = "agenda")]
#[command(about = "Agenda Reborn CLI")]
struct Cli {
    /// SQLite database path
    #[arg(long, env = "AGENDA_DB")]
    db: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum CategoryTypeArg {
    Tag,
    Numeric,
}

impl CategoryTypeArg {
    fn into_model(self) -> CategoryValueKind {
        match self {
            Self::Tag => CategoryValueKind::Tag,
            Self::Numeric => CategoryValueKind::Numeric,
        }
    }
}

#[derive(Clone, Copy, Debug, ValueEnum, PartialEq, Eq)]
enum OutputFormatArg {
    Table,
    Json,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Add a new item
    Add {
        text: String,
        #[arg(long)]
        note: Option<String>,
    },

    /// Edit an existing item's text, note, and/or done state
    #[command(
        after_help = "Note operations:\n  --note <TEXT>          Replace the entire note\n  --append-note <TEXT>   Append text to the existing note (separated by newline)\n  --note-stdin           Replace the entire note with stdin content\n  --clear-note           Remove the note entirely\n\nExamples:\n  agenda edit <id> --append-note \"Claimed 2026-03-02: branch=feature\"\n  agenda edit <id> --append-note \"Implementation plan:\\n1. Step one\\n2. Step two\"\n  printf \"line one\\nline two\\n\" | agenda edit <id> --note-stdin"
    )]
    Edit {
        item_id: String,
        /// New text (positional shorthand; also available as --text)
        text: Option<String>,
        #[arg(long)]
        note: Option<String>,
        /// Append text to the existing note (separated by newline)
        #[arg(long = "append-note")]
        append_note: Option<String>,
        /// Replace the note with stdin content
        #[arg(long = "note-stdin")]
        note_stdin: bool,
        #[arg(long = "clear-note")]
        clear_note: bool,
        #[arg(long)]
        done: Option<bool>,
    },

    /// Show a single item with its assignments
    Show { item_id: String },

    /// Atomically claim an item for active work
    #[command(
        after_help = "Defaults (`agenda claim <ITEM_ID>`):\n  --claim-category \"In Progress\"\n  --must-not-have \"In Progress\"\n  --must-not-have \"Complete\"\n\nSetup:\n  Create an `In Progress` category (or sub-category) before claiming.\n\n  Feature DB example (`aglet-features.ag`):\n  agenda category create Status --exclusive\n  agenda category create Ready --parent Status\n  agenda category create \"In Progress\" --parent Status\n  agenda category create \"Waiting/Blocked\" --parent Status\n  agenda category create Complete --parent Status\n\nExamples:\n  agenda claim <ITEM_ID>\n  agenda claim <ITEM_ID> --must-not-have \"In Progress\" --must-not-have \"Complete\"\n  agenda claim <ITEM_ID> --claim-category \"In Progress\" --must-not-have \"Waiting/Blocked\""
    )]
    Claim {
        item_id: String,
        /// Category to assign on successful claim.
        #[arg(long = "claim-category", default_value = "In Progress")]
        claim_category: String,
        /// Claim preconditions: fail if the item already has any of these categories.
        #[arg(
            long = "must-not-have",
            default_values = ["In Progress", "Complete"]
        )]
        must_not_have: Vec<String>,
    },

    /// List items (optionally filtered)
    #[command(
        after_help = "Numeric value filter examples:\n  agenda list --value-eq Complexity 2\n  agenda list --value-in Complexity 1,2\n  agenda list --value-max Complexity 2\n\nSemantics:\n  Numeric value filters are AND-composed with each other and with category filters."
    )]
    List {
        #[arg(long)]
        view: Option<String>,
        /// Category filter (repeat for AND). Item must have ALL specified categories.
        #[arg(long)]
        category: Vec<String>,
        /// OR-category filter (repeat for OR). Item must have AT LEAST ONE specified category.
        #[arg(long = "any-category")]
        any_category: Vec<String>,
        /// Exclude-category filter (repeat for OR). Item must have NONE of the specified categories.
        #[arg(long = "exclude-category")]
        exclude_category: Vec<String>,
        /// Numeric equality filter (repeat for AND): category value must equal VALUE.
        #[arg(
            long = "value-eq",
            value_names = ["CATEGORY", "VALUE"],
            num_args = 2
        )]
        value_eq: Vec<String>,
        /// Numeric membership filter (repeat for AND): category value must be in CSV_VALUES.
        #[arg(
            long = "value-in",
            value_names = ["CATEGORY", "CSV_VALUES"],
            num_args = 2
        )]
        value_in: Vec<String>,
        /// Numeric max filter (repeat for AND): category value must be <= VALUE.
        #[arg(
            long = "value-max",
            value_names = ["CATEGORY", "VALUE"],
            num_args = 2
        )]
        value_max: Vec<String>,
        /// Sort key(s): item, when, or category name. Repeat for multi-key sorting.
        /// Optional suffix `:asc` or `:desc` (default: asc).
        #[arg(long = "sort", value_name = "COLUMN[:asc|desc]")]
        sort: Vec<String>,
        /// Output format.
        #[arg(long = "format", value_enum, default_value_t = OutputFormatArg::Table)]
        format: OutputFormatArg,
        #[arg(long)]
        include_done: bool,
    },

    /// Search item text and note
    Search {
        query: String,
        /// Output format.
        #[arg(long = "format", value_enum, default_value_t = OutputFormatArg::Table)]
        format: OutputFormatArg,
        #[arg(long)]
        include_done: bool,
    },

    /// Delete an item (writes deletion log)
    Delete { item_id: String },

    /// List deletion log entries
    Deleted,

    /// Restore an item from deletion log by log entry id
    Restore { log_id: String },

    /// Launch the interactive TUI
    Tui,

    /// Category commands
    Category {
        #[command(subcommand)]
        command: CategoryCommand,
    },

    /// View commands
    View {
        #[command(subcommand)]
        command: ViewCommand,
    },

    /// Item-to-item link commands
    Link {
        #[command(subcommand)]
        command: LinkCommand,
    },

    /// Remove item-to-item links (canonical unlink entrypoint)
    Unlink {
        #[command(subcommand)]
        command: UnlinkCommand,
    },
}

#[derive(Subcommand, Debug)]
enum CategoryCommand {
    /// List categories as a tree
    List,

    /// Show detailed info for a category
    Show { name: String },

    /// Create a category
    Create {
        name: String,
        #[arg(long)]
        parent: Option<String>,
        #[arg(long)]
        exclusive: bool,
        #[arg(long = "disable-implicit-string")]
        disable_implicit_string: bool,
        #[arg(long = "type", value_enum)]
        category_type: Option<CategoryTypeArg>,
    },

    /// Delete a category by name
    Delete { name: String },

    /// Rename a category
    Rename { name: String, new_name: String },

    /// Reparent a category (use --root to make top-level)
    Reparent {
        name: String,
        #[arg(long)]
        parent: Option<String>,
        #[arg(long)]
        root: bool,
    },

    /// Update category flags
    Update {
        name: String,
        #[arg(long)]
        exclusive: Option<bool>,
        #[arg(long)]
        actionable: Option<bool>,
        #[arg(long = "implicit-string")]
        implicit_string: Option<bool>,
        #[arg(long)]
        note: Option<String>,
        #[arg(long = "clear-note")]
        clear_note: bool,
        #[arg(long = "type", value_enum)]
        category_type: Option<CategoryTypeArg>,
    },

    /// Assign an item to a category by id/name
    Assign {
        item_id: String,
        category_name: String,
    },

    /// Set a numeric value assignment for a numeric category
    SetValue {
        item_id: String,
        category_name: String,
        value: String,
    },

    /// Unassign an item from a category
    Unassign {
        item_id: String,
        category_name: String,
    },
}

#[derive(Subcommand, Debug)]
enum ViewCommand {
    /// List views
    List,

    /// Show the contents of a view
    Show {
        name: String,
        /// Sort key(s): item, when, or category name. Repeat for multi-key sorting.
        /// Optional suffix `:asc` or `:desc` (default: asc).
        #[arg(long = "sort", value_name = "COLUMN[:asc|desc]")]
        sort: Vec<String>,
        /// Output format.
        #[arg(long = "format", value_enum, default_value_t = OutputFormatArg::Table)]
        format: OutputFormatArg,
    },

    /// Create a basic view from include/exclude categories
    Create {
        name: String,
        #[arg(long = "include")]
        include: Vec<String>,
        #[arg(long = "exclude")]
        exclude: Vec<String>,
        #[arg(long = "hide-unmatched")]
        hide_unmatched: bool,
    },

    /// Rename a view
    Rename { name: String, new_name: String },

    /// Delete a view by name
    Delete { name: String },
}

#[derive(Subcommand, Debug)]
enum LinkCommand {
    /// Create a dependency link: ITEM depends on DEPENDS_ON_ITEM
    #[command(name = "depends-on")]
    DependsOn {
        item_id: String,
        depends_on_item_id: String,
    },

    /// Create inverse dependency vocabulary: BLOCKER blocks BLOCKED
    Blocks {
        blocker_item_id: String,
        blocked_item_id: String,
    },

    /// Create a bidirectional related link
    Related {
        item_a_id: String,
        item_b_id: String,
    },
}

#[derive(Subcommand, Debug)]
enum UnlinkCommand {
    /// Remove inverse dependency vocabulary: BLOCKER no longer blocks BLOCKED
    Blocks {
        blocker_item_id: String,
        blocked_item_id: String,
    },

    /// Remove a dependency link: ITEM no longer depends on DEPENDS_ON_ITEM
    #[command(name = "depends-on")]
    DependsOn {
        item_id: String,
        depends_on_item_id: String,
    },

    /// Remove a related link
    Related {
        item_a_id: String,
        item_b_id: String,
    },
}

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let cli = Cli::parse();
    let db_path = resolve_db_path(cli.db)?;
    let command = cli.command.unwrap_or(Command::List {
        view: None,
        category: Vec::new(),
        any_category: Vec::new(),
        exclude_category: Vec::new(),
        value_eq: Vec::new(),
        value_in: Vec::new(),
        value_max: Vec::new(),
        sort: Vec::new(),
        format: OutputFormatArg::Table,
        include_done: false,
    });

    if matches!(&command, Command::Tui) {
        return agenda_tui::run(&db_path);
    }

    let store = Store::open(&db_path).map_err(|e| e.to_string())?;
    let classifier = SubstringClassifier;
    let agenda = Agenda::new(&store, &classifier);

    match command {
        Command::Add { text, note } => cmd_add(&agenda, text, note),
        Command::Edit {
            item_id,
            text,
            note,
            append_note,
            note_stdin: note_stdin_flag,
            clear_note,
            done,
        } => {
            let note_stdin = if note_stdin_flag {
                let mut stdin = io::stdin().lock();
                Some(read_note_from_stdin(&mut stdin)?)
            } else {
                None
            };
            cmd_edit(
                &agenda,
                item_id,
                text,
                note,
                append_note,
                note_stdin,
                clear_note,
                done,
            )
        }
        Command::Show { item_id } => cmd_show(&store, item_id),
        Command::Claim {
            item_id,
            claim_category,
            must_not_have,
        } => cmd_claim(&agenda, &store, item_id, claim_category, must_not_have),
        Command::List {
            view,
            category,
            any_category,
            exclude_category,
            value_eq,
            value_in,
            value_max,
            sort,
            format,
            include_done,
        } => cmd_list(
            &store,
            view,
            ListFilters {
                all_categories: category,
                any_categories: any_category,
                exclude_categories: exclude_category,
                value_eq,
                value_in,
                value_max,
                include_done,
            },
            sort,
            format,
        ),
        Command::Search {
            query,
            format,
            include_done,
        } => cmd_search(&store, query, format, include_done),
        Command::Delete { item_id } => cmd_delete(&agenda, item_id),
        Command::Deleted => cmd_deleted(&store),
        Command::Restore { log_id } => cmd_restore(&store, log_id),
        Command::Category { command } => cmd_category(&agenda, &store, command),
        Command::View { command } => cmd_view(&agenda, &store, command),
        Command::Link { command } => cmd_link(&agenda, command),
        Command::Unlink { command } => cmd_unlink(&agenda, command),
        Command::Tui => Ok(()),
    }
}

fn cmd_add(agenda: &Agenda<'_>, text: String, note: Option<String>) -> Result<(), String> {
    let category_names: Vec<String> = agenda
        .store()
        .get_hierarchy()
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|category| category.name)
        .collect();
    let unknown_hashtags = unknown_hashtag_tokens(&text, &category_names);

    let mut item = Item::new(text);
    item.note = note;

    let reference_date = Local::now().date_naive();
    let result = agenda
        .create_item_with_reference_date(&item, reference_date)
        .map_err(|e| e.to_string())?;
    let created = agenda
        .store()
        .get_item(item.id)
        .map_err(|e| e.to_string())?;

    println!("created {}", item.id);
    if let Some(line) = parsed_when_feedback_line(created.when_date) {
        println!("{line}");
    }
    if !result.new_assignments.is_empty() {
        println!("new_assignments={}", result.new_assignments.len());
    }
    if let Some(line) = unknown_hashtag_feedback_line(&unknown_hashtags) {
        println!("{line}");
    }
    Ok(())
}

fn read_note_from_stdin(reader: &mut impl Read) -> Result<String, String> {
    let mut input = String::new();
    reader
        .read_to_string(&mut input)
        .map_err(|e| format!("failed to read --note-stdin input: {e}"))?;
    Ok(input)
}

#[allow(clippy::too_many_arguments)]
fn cmd_edit(
    agenda: &Agenda<'_>,
    item_id_str: String,
    text: Option<String>,
    note: Option<String>,
    append_note: Option<String>,
    note_stdin: Option<String>,
    clear_note: bool,
    done: Option<bool>,
) -> Result<(), String> {
    let item_id = parse_item_id(&item_id_str)?;

    if text.is_none()
        && note.is_none()
        && append_note.is_none()
        && note_stdin.is_none()
        && !clear_note
        && done.is_none()
    {
        return Err(
            "nothing to update\n\nUsage: agenda edit <ITEM_ID> [TEXT] [--note <NOTE>] [--append-note <TEXT>] [--note-stdin] [--clear-note] [--done <true|false>]\n\nExamples:\n  agenda edit <id> \"new text here\"\n  agenda edit <id> --note \"updated note\"\n  agenda edit <id> --append-note \"extra info\"\n  printf \"line one\\nline two\\n\" | agenda edit <id> --note-stdin\n  agenda edit <id> \"new text\" --note \"and note\"\n  agenda edit <id> --clear-note\n  agenda edit <id> --done true\n  agenda edit <id> --done false".to_string()
        );
    }

    // Validate mutually exclusive note flags
    let note_flag_count = note.is_some() as u8
        + append_note.is_some() as u8
        + note_stdin.is_some() as u8
        + clear_note as u8;
    if note_flag_count > 1 {
        return Err(
            "--note, --append-note, --note-stdin, and --clear-note are mutually exclusive"
                .to_string(),
        );
    }

    if let Some(done_value) = done {
        if done_value {
            agenda.mark_item_done(item_id).map_err(|e| e.to_string())?;
            println!("marked done {}", item_id);
        } else {
            agenda
                .mark_item_not_done(item_id)
                .map_err(|e| e.to_string())?;
            println!("marked not-done {}", item_id);
        }
    }

    let mut item = agenda
        .store()
        .get_item(item_id)
        .map_err(|e| e.to_string())?;

    let note_stdin_has_content = note_stdin.as_ref().is_some_and(|value| !value.is_empty());
    if text.is_some() || note.is_some() || append_note.is_some() || note_stdin_has_content || clear_note
    {
        if let Some(new_text) = text {
            if new_text.is_empty() {
                return Err("text cannot be empty".to_string());
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

        item.modified_at = chrono::Utc::now();
        let reference_date = Local::now().date_naive();
        agenda
            .update_item_with_reference_date(&item, reference_date)
            .map_err(|e| e.to_string())?;

        let updated = agenda
            .store()
            .get_item(item_id)
            .map_err(|e| e.to_string())?;
        println!("updated {}", item_id);
        if let Some(line) = parsed_when_feedback_line(updated.when_date) {
            println!("{line}");
        }
    }

    Ok(())
}

fn cmd_show(store: &Store, item_id_str: String) -> Result<(), String> {
    let item_id = parse_item_id(&item_id_str)?;
    let item = store.get_item(item_id).map_err(|e| e.to_string())?;
    let categories = store.get_hierarchy().map_err(|e| e.to_string())?;
    let category_names = category_name_map(&categories);

    let done = if item.is_done { "done" } else { "open" };
    let when = item
        .when_date
        .map(|dt| dt.to_string())
        .unwrap_or_else(|| "-".to_string());

    println!("id:         {}", item.id);
    println!("text:       {}", item.text);
    println!("status:     {}", done);
    println!("when:       {}", when);
    println!("entry_date: {}", item.entry_date);
    println!("created_at: {}", item.created_at.to_rfc3339());
    println!("modified_at: {}", item.modified_at.to_rfc3339());
    if let Some(done_date) = item.done_date {
        println!("done_date:  {}", done_date);
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
                (name, assignment)
            })
            .collect();
        rows.sort_by_key(|(name, _)| name.to_ascii_lowercase());
        for (name, assignment) in rows {
            let origin = assignment.origin.as_deref().unwrap_or("-");
            println!("  {} | {:?} | {}", name, assignment.source, origin);
        }
    }

    for line in item_link_section_lines(store, item.id)? {
        println!("{line}");
    }

    Ok(())
}

fn cmd_claim(
    agenda: &Agenda<'_>,
    store: &Store,
    item_id_str: String,
    claim_category_name: String,
    must_not_have_names: Vec<String>,
) -> Result<(), String> {
    let item_id = parse_item_id(&item_id_str)?;
    let categories = store.get_hierarchy().map_err(|e| e.to_string())?;
    let claim_category_id = category_id_by_name(&categories, &claim_category_name)?;
    let claim_category = categories
        .iter()
        .find(|category| category.id == claim_category_id)
        .ok_or_else(|| format!("category not found: {claim_category_name}"))?;
    if claim_category.value_kind == CategoryValueKind::Numeric {
        return Err(format!(
            "category '{}' is Numeric; claims require a non-numeric category",
            claim_category.name
        ));
    }

    let mut must_not_have_ids = Vec::new();
    for name in must_not_have_names {
        let category_id = category_id_by_name(&categories, &name)?;
        if !must_not_have_ids.contains(&category_id) {
            must_not_have_ids.push(category_id);
        }
    }

    let result = agenda
        .claim_item_manual(
            item_id,
            claim_category_id,
            &must_not_have_ids,
            Some("manual:cli.claim".to_string()),
        )
        .map_err(|e| e.to_string())?;
    println!(
        "claimed item {} to category {}",
        item_id, claim_category.name
    );
    if !result.new_assignments.is_empty() {
        println!("new_assignments={}", result.new_assignments.len());
    }
    Ok(())
}

fn parsed_when_feedback_line(when_date: Option<NaiveDateTime>) -> Option<String> {
    when_date.map(|when| format!("parsed_when={when}"))
}

fn unknown_hashtag_feedback_line(unknown_hashtags: &[String]) -> Option<String> {
    if unknown_hashtags.is_empty() {
        return None;
    }
    Some(format!(
        "warning: unknown_hashtags={}",
        unknown_hashtags.join(",")
    ))
}

struct ListFilters {
    all_categories: Vec<String>,
    any_categories: Vec<String>,
    exclude_categories: Vec<String>,
    value_eq: Vec<String>,
    value_in: Vec<String>,
    value_max: Vec<String>,
    include_done: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum NumericPredicate {
    Eq(Decimal),
    In(Vec<Decimal>),
    Max(Decimal),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NumericFilter {
    category_id: CategoryId,
    category_name: String,
    predicate: NumericPredicate,
}

fn cmd_list(
    store: &Store,
    view_name: Option<String>,
    filters: ListFilters,
    sort_args: Vec<String>,
    output_format: OutputFormatArg,
) -> Result<(), String> {
    let mut items = store.list_items().map_err(|e| e.to_string())?;
    if !filters.include_done {
        items.retain(|item| !item.is_done);
    }

    let categories = store.get_hierarchy().map_err(|e| e.to_string())?;
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

    let resolved_view = if let Some(view_name) = view_name {
        Some(view_by_name(store, &view_name)?)
    } else {
        store
            .list_views()
            .map_err(|e| e.to_string())?
            .into_iter()
            .next()
    };

    if let Some(view) = resolved_view {
        print_items_for_view(
            &view,
            &items,
            &categories,
            &category_names,
            &sort_keys,
            output_format,
        )?;
    } else if output_format == OutputFormatArg::Json {
        print_items_json(&items, &category_names, &sort_keys, &categories)?;
    } else {
        print_item_table(&items, &category_names, &sort_keys, &categories);
    }
    Ok(())
}

fn retain_items_with_all_categories(items: &mut Vec<Item>, category_ids: &[CategoryId]) {
    items.retain(|item| {
        category_ids
            .iter()
            .all(|id| item.assignments.contains_key(id))
    });
}

fn retain_items_with_any_categories(items: &mut Vec<Item>, category_ids: &[CategoryId]) {
    items.retain(|item| {
        category_ids
            .iter()
            .any(|id| item.assignments.contains_key(id))
    });
}

fn reject_items_with_any_categories(items: &mut Vec<Item>, category_ids: &[CategoryId]) {
    items.retain(|item| {
        category_ids
            .iter()
            .all(|id| !item.assignments.contains_key(id))
    });
}

fn build_numeric_filters(
    categories: &[Category],
    filters: &ListFilters,
) -> Result<Vec<NumericFilter>, String> {
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

fn parse_arg_pairs(args: &[String], flag_name: &str) -> Result<Vec<(String, String)>, String> {
    let mut out = Vec::new();
    let chunks = args.chunks_exact(2);
    if !chunks.remainder().is_empty() {
        return Err(format!(
            "invalid {flag_name} arguments: expected repeated <CATEGORY> <VALUE> pairs"
        ));
    }
    for pair in chunks {
        out.push((pair[0].clone(), pair[1].clone()));
    }
    Ok(out)
}

fn resolve_numeric_filter_category(
    categories: &[Category],
    category_name: &str,
) -> Result<(CategoryId, String), String> {
    let category_id = category_id_by_name(categories, category_name)?;
    let category = categories
        .iter()
        .find(|c| c.id == category_id)
        .ok_or_else(|| format!("category not found: {category_name}"))?;
    if category.value_kind != CategoryValueKind::Numeric {
        return Err(format!(
            "category '{}' is not Numeric; numeric value filters require a Numeric category",
            category.name
        ));
    }
    Ok((category.id, category.name.clone()))
}

fn parse_csv_decimals(input: &str, category_name: &str) -> Result<Vec<Decimal>, String> {
    let mut values = Vec::new();
    for token in input.split(',') {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            return Err(format!(
                "invalid --value-in for category '{}': empty value in CSV list",
                category_name
            ));
        }
        values.push(parse_decimal_value(trimmed)?);
    }
    Ok(values)
}

fn retain_items_matching_numeric_filters(items: &mut Vec<Item>, numeric_filters: &[NumericFilter]) {
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

fn cmd_search(
    store: &Store,
    query: String,
    output_format: OutputFormatArg,
    include_done: bool,
) -> Result<(), String> {
    let mut items = store.list_items().map_err(|e| e.to_string())?;
    if !include_done {
        items.retain(|item| !item.is_done);
    }

    let categories = store.get_hierarchy().map_err(|e| e.to_string())?;
    let category_names = category_name_map(&categories);

    let q = Query {
        text_search: Some(query),
        ..Query::default()
    };
    let reference_date = Local::now().date_naive();
    let matches = evaluate_query(&q, &items, reference_date);

    let matched_items: Vec<Item> = matches.into_iter().cloned().collect();
    if output_format == OutputFormatArg::Json {
        print_items_json(&matched_items, &category_names, &[], &categories)?;
    } else {
        print_item_table(&matched_items, &category_names, &[], &categories);
    }
    Ok(())
}

fn cmd_delete(agenda: &Agenda<'_>, item_id_str: String) -> Result<(), String> {
    let item_id = parse_item_id(&item_id_str)?;
    agenda
        .delete_item(item_id, "user:cli")
        .map_err(|e| e.to_string())?;
    println!("deleted {}", item_id);
    Ok(())
}

fn cmd_deleted(store: &Store) -> Result<(), String> {
    let deleted = store.list_deleted_items().map_err(|e| e.to_string())?;
    if deleted.is_empty() {
        println!("no deleted items");
        return Ok(());
    }

    for entry in deleted {
        println!(
            "{} | item={} | deleted_at={} | by={} | {}",
            entry.id,
            entry.item_id,
            entry.deleted_at.to_rfc3339(),
            entry.deleted_by,
            entry.text
        );
    }
    Ok(())
}

fn cmd_restore(store: &Store, log_id_str: String) -> Result<(), String> {
    let log_id = Uuid::parse_str(&log_id_str).map_err(|e| format!("invalid log id: {e}"))?;
    let item_id = store
        .restore_deleted_item(log_id)
        .map_err(|e| e.to_string())?;
    println!("restored item {}", item_id);
    Ok(())
}

fn cmd_link(agenda: &Agenda<'_>, command: LinkCommand) -> Result<(), String> {
    match command {
        LinkCommand::DependsOn {
            item_id,
            depends_on_item_id,
        } => {
            let item_id = parse_item_id(&item_id)?;
            let depends_on_item_id = parse_item_id(&depends_on_item_id)?;
            let result = agenda
                .link_items_depends_on(item_id, depends_on_item_id)
                .map_err(|e| e.to_string())?;
            if result.created {
                println!("linked {} depends-on {}", item_id, depends_on_item_id);
            } else {
                println!(
                    "link already exists: {} depends-on {}",
                    item_id, depends_on_item_id
                );
            }
            Ok(())
        }
        LinkCommand::Blocks {
            blocker_item_id,
            blocked_item_id,
        } => {
            let blocker_item_id = parse_item_id(&blocker_item_id)?;
            let blocked_item_id = parse_item_id(&blocked_item_id)?;
            let result = agenda
                .link_items_blocks(blocker_item_id, blocked_item_id)
                .map_err(|e| e.to_string())?;
            if result.created {
                println!("linked {} blocks {}", blocker_item_id, blocked_item_id);
            } else {
                println!(
                    "link already exists: {} blocks {}",
                    blocker_item_id, blocked_item_id
                );
            }
            Ok(())
        }
        LinkCommand::Related {
            item_a_id,
            item_b_id,
        } => {
            let item_a_id = parse_item_id(&item_a_id)?;
            let item_b_id = parse_item_id(&item_b_id)?;
            let result = agenda
                .link_items_related(item_a_id, item_b_id)
                .map_err(|e| e.to_string())?;
            if result.created {
                println!("linked {} related {}", item_a_id, item_b_id);
            } else {
                println!("link already exists: {} related {}", item_a_id, item_b_id);
            }
            Ok(())
        }
    }
}

fn cmd_unlink(agenda: &Agenda<'_>, command: UnlinkCommand) -> Result<(), String> {
    match command {
        UnlinkCommand::Blocks {
            blocker_item_id,
            blocked_item_id,
        } => unlink_blocks(agenda, blocker_item_id, blocked_item_id),
        UnlinkCommand::DependsOn {
            item_id,
            depends_on_item_id,
        } => unlink_depends_on(agenda, item_id, depends_on_item_id),
        UnlinkCommand::Related {
            item_a_id,
            item_b_id,
        } => unlink_related(agenda, item_a_id, item_b_id),
    }
}

fn unlink_depends_on(
    agenda: &Agenda<'_>,
    item_id: String,
    depends_on_item_id: String,
) -> Result<(), String> {
    let item_id = parse_item_id(&item_id)?;
    let depends_on_item_id = parse_item_id(&depends_on_item_id)?;
    agenda
        .unlink_items_depends_on(item_id, depends_on_item_id)
        .map_err(|e| e.to_string())?;
    println!("unlinked {} depends-on {}", item_id, depends_on_item_id);
    Ok(())
}

fn unlink_blocks(
    agenda: &Agenda<'_>,
    blocker_item_id: String,
    blocked_item_id: String,
) -> Result<(), String> {
    let blocker_item_id = parse_item_id(&blocker_item_id)?;
    let blocked_item_id = parse_item_id(&blocked_item_id)?;
    agenda
        .unlink_items_blocks(blocker_item_id, blocked_item_id)
        .map_err(|e| e.to_string())?;
    println!("unlinked {} blocks {}", blocker_item_id, blocked_item_id);
    Ok(())
}

fn unlink_related(agenda: &Agenda<'_>, item_a_id: String, item_b_id: String) -> Result<(), String> {
    let item_a_id = parse_item_id(&item_a_id)?;
    let item_b_id = parse_item_id(&item_b_id)?;
    agenda
        .unlink_items_related(item_a_id, item_b_id)
        .map_err(|e| e.to_string())?;
    println!("unlinked {} related {}", item_a_id, item_b_id);
    Ok(())
}

fn item_link_section_lines(store: &Store, item_id: ItemId) -> Result<Vec<String>, String> {
    let prereqs = resolve_link_neighbors(
        store,
        store
            .list_dependency_ids_for_item(item_id)
            .map_err(|e| e.to_string())?,
    )?;
    let dependents = resolve_link_neighbors(
        store,
        store
            .list_dependent_ids_for_item(item_id)
            .map_err(|e| e.to_string())?,
    )?;
    let related = resolve_link_neighbors(
        store,
        store
            .list_related_ids_for_item(item_id)
            .map_err(|e| e.to_string())?,
    )?;

    let mut lines = Vec::new();
    append_link_section_lines(&mut lines, "prereqs", &prereqs);
    append_link_section_lines(&mut lines, "dependents (blocks)", &dependents);
    append_link_section_lines(&mut lines, "related", &related);
    Ok(lines)
}

fn resolve_link_neighbors(
    store: &Store,
    ids: Vec<ItemId>,
) -> Result<Vec<(String, String)>, String> {
    let mut rows = Vec::new();
    for id in ids {
        match store.get_item(id) {
            Ok(item) => rows.push((
                item.text.to_ascii_lowercase(),
                format!(
                    "  {} | {} | {}",
                    id,
                    if item.is_done { "done" } else { "open" },
                    item.text
                ),
            )),
            Err(_) => rows.push((
                id.to_string(),
                format!("  {} | missing | (linked item unavailable)", id),
            )),
        }
    }
    rows.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(rows)
}

fn append_link_section_lines(lines: &mut Vec<String>, label: &str, rows: &[(String, String)]) {
    if rows.is_empty() {
        lines.push(format!("{label}: (none)"));
        return;
    }
    lines.push(format!("{label}:"));
    for (_, line) in rows {
        lines.push(line.clone());
    }
}

fn cmd_category(
    agenda: &Agenda<'_>,
    store: &Store,
    command: CategoryCommand,
) -> Result<(), String> {
    match command {
        CategoryCommand::List => {
            let categories = store.get_hierarchy().map_err(|e| e.to_string())?;
            print_category_tree(&categories);
            Ok(())
        }
        CategoryCommand::Show { name } => {
            let categories = store.get_hierarchy().map_err(|e| e.to_string())?;
            let category_id = category_id_by_name(&categories, &name)?;
            let category = store.get_category(category_id).map_err(|e| e.to_string())?;
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
                for condition in &category.conditions {
                    match condition {
                        agenda_core::model::Condition::ImplicitString => {
                            println!("  - ImplicitString");
                        }
                        agenda_core::model::Condition::Profile { criteria } => {
                            let and_names: Vec<&str> = criteria
                                .and_category_ids()
                                .filter_map(|id| category_names.get(&id).map(|s| s.as_str()))
                                .collect();
                            let not_names: Vec<&str> = criteria
                                .not_category_ids()
                                .filter_map(|id| category_names.get(&id).map(|s| s.as_str()))
                                .collect();
                            let or_names: Vec<&str> = criteria
                                .or_category_ids()
                                .filter_map(|id| category_names.get(&id).map(|s| s.as_str()))
                                .collect();
                            println!(
                                "  - Profile (and=[{}], not=[{}], or=[{}])",
                                and_names.join(", "),
                                not_names.join(", "),
                                or_names.join(", ")
                            );
                        }
                    }
                }
            }
            if !category.actions.is_empty() {
                println!("actions:");
                for action in &category.actions {
                    match action {
                        agenda_core::model::Action::Assign { targets } => {
                            let names: Vec<&str> = targets
                                .iter()
                                .filter_map(|id| category_names.get(id).map(|s| s.as_str()))
                                .collect();
                            println!("  - Assign [{}]", names.join(", "));
                        }
                        agenda_core::model::Action::Remove { targets } => {
                            let names: Vec<&str> = targets
                                .iter()
                                .filter_map(|id| category_names.get(id).map(|s| s.as_str()))
                                .collect();
                            println!("  - Remove [{}]", names.join(", "));
                        }
                    }
                }
            }
            println!("created_at:      {}", category.created_at.to_rfc3339());
            println!("modified_at:     {}", category.modified_at.to_rfc3339());
            Ok(())
        }
        CategoryCommand::Create {
            name,
            parent,
            exclusive,
            disable_implicit_string,
            category_type,
        } => {
            let categories = store.get_hierarchy().map_err(|e| e.to_string())?;
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

            let result = match agenda.create_category(&category) {
                Ok(result) => result,
                Err(AgendaError::DuplicateName {
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
                    ));
                }
                Err(other) => return Err(other.to_string()),
            };
            println!(
                "created category {} (type={}, processed_items={}, affected_items={})",
                category.name,
                category_value_kind_label(category.value_kind),
                result.processed_items,
                result.affected_items
            );
            Ok(())
        }
        CategoryCommand::Delete { name } => {
            let categories = store.get_hierarchy().map_err(|e| e.to_string())?;
            let category_id = category_id_by_name(&categories, &name)?;
            store
                .delete_category(category_id)
                .map_err(|e| e.to_string())?;
            println!("deleted category {}", name);
            Ok(())
        }
        CategoryCommand::Rename { name, new_name } => {
            let categories = store.get_hierarchy().map_err(|e| e.to_string())?;
            let category_id = category_id_by_name(&categories, &name)?;
            let mut category = store.get_category(category_id).map_err(|e| e.to_string())?;
            category.name = new_name.clone();
            let result = agenda
                .update_category(&category)
                .map_err(|e| e.to_string())?;
            println!(
                "renamed {} -> {} (processed_items={}, affected_items={})",
                name, new_name, result.processed_items, result.affected_items
            );
            Ok(())
        }
        CategoryCommand::Reparent { name, parent, root } => {
            let categories = store.get_hierarchy().map_err(|e| e.to_string())?;
            let category_id = category_id_by_name(&categories, &name)?;
            let mut category = store.get_category(category_id).map_err(|e| e.to_string())?;
            if root {
                category.parent = None;
            } else if let Some(parent_name) = parent {
                category.parent = Some(category_id_by_name(&categories, &parent_name)?);
            } else {
                return Err("specify --parent <name> or --root to make top-level".to_string());
            }
            let result = agenda
                .update_category(&category)
                .map_err(|e| e.to_string())?;
            let new_parent = category
                .parent
                .and_then(|id| categories.iter().find(|c| c.id == id))
                .map(|c| c.name.as_str())
                .unwrap_or("(root)");
            println!(
                "reparented {} under {} (processed_items={}, affected_items={})",
                name, new_parent, result.processed_items, result.affected_items
            );
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
            let categories = store.get_hierarchy().map_err(|e| e.to_string())?;
            let category_id = category_id_by_name(&categories, &name)?;
            let mut category = store.get_category(category_id).map_err(|e| e.to_string())?;
            if exclusive.is_none()
                && actionable.is_none()
                && implicit_string.is_none()
                && note.is_none()
                && !clear_note
                && category_type.is_none()
            {
                return Err("nothing to update: specify --exclusive, --actionable, --implicit-string, --type, --note, or --clear-note".to_string());
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
            let result = agenda
                .update_category(&category)
                .map_err(|e| e.to_string())?;
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
            Ok(())
        }
        CategoryCommand::Assign {
            item_id,
            category_name,
        } => {
            let item_id = parse_item_id(&item_id)?;
            let categories = store.get_hierarchy().map_err(|e| e.to_string())?;
            let category_id = category_id_by_name(&categories, &category_name)?;
            let category = categories
                .iter()
                .find(|c| c.id == category_id)
                .ok_or_else(|| format!("category not found: {category_name}"))?;

            if category_name.eq_ignore_ascii_case("Done") {
                agenda.mark_item_done(item_id).map_err(|e| e.to_string())?;
                println!(
                    "assigned item {} to category Done (is_done and done_date updated)",
                    item_id
                );
                return Ok(());
            }
            if category.value_kind == CategoryValueKind::Numeric {
                return Err(format!(
                    "category '{}' is Numeric; use `agenda category set-value <item-id> \"{}\" <number>`",
                    category.name, category.name
                ));
            }

            let result = agenda
                .assign_item_manual(item_id, category_id, Some("manual:cli.assign".to_string()))
                .map_err(|e| e.to_string())?;
            println!("assigned item {} to category {}", item_id, category_name);
            if !result.new_assignments.is_empty() {
                println!("new_assignments={}", result.new_assignments.len());
            }
            Ok(())
        }
        CategoryCommand::SetValue {
            item_id,
            category_name,
            value,
        } => {
            let item_id = parse_item_id(&item_id)?;
            let categories = store.get_hierarchy().map_err(|e| e.to_string())?;
            let category_id = category_id_by_name(&categories, &category_name)?;
            let numeric_value = parse_decimal_value(&value)?;
            let result = agenda
                .assign_item_numeric_manual(
                    item_id,
                    category_id,
                    numeric_value,
                    Some("manual:cli.set-value".to_string()),
                )
                .map_err(|e| e.to_string())?;
            println!(
                "set value for item {} category {} = {}",
                item_id, category_name, numeric_value
            );
            if !result.new_assignments.is_empty() {
                println!("new_assignments={}", result.new_assignments.len());
            }
            Ok(())
        }
        CategoryCommand::Unassign {
            item_id,
            category_name,
        } => {
            let item_id = parse_item_id(&item_id)?;
            let categories = store.get_hierarchy().map_err(|e| e.to_string())?;
            let category_id = category_id_by_name(&categories, &category_name)?;

            if category_name.eq_ignore_ascii_case("Done") {
                let item = store.get_item(item_id).map_err(|e| e.to_string())?;
                if item.is_done {
                    agenda
                        .toggle_item_done(item_id)
                        .map_err(|e| e.to_string())?;
                    println!(
                        "unassigned item {} from category Done (marked not-done)",
                        item_id
                    );
                    return Ok(());
                }
            }

            agenda
                .unassign_item_manual(item_id, category_id)
                .map_err(|e| e.to_string())?;
            println!(
                "unassigned item {} from category {}",
                item_id, category_name
            );
            Ok(())
        }
    }
}

fn cmd_view(agenda: &Agenda<'_>, store: &Store, command: ViewCommand) -> Result<(), String> {
    let _ = agenda;
    match command {
        ViewCommand::List => {
            let views = store.list_views().map_err(|e| e.to_string())?;
            if views.is_empty() {
                println!("no views");
                return Ok(());
            }
            for view in views {
                println!(
                    "{} (sections={}, and={}, not={}, or={})",
                    view.name,
                    view.sections.len(),
                    view.criteria.and_category_ids().count(),
                    view.criteria.not_category_ids().count(),
                    view.criteria.or_category_ids().count()
                );
            }
            println!("hint: use `agenda view show \"<name>\"` to see view contents");
            Ok(())
        }
        ViewCommand::Show { name, sort, format } => {
            let categories = store.get_hierarchy().map_err(|e| e.to_string())?;
            let category_names = category_name_map(&categories);
            let items = store.list_items().map_err(|e| e.to_string())?;
            let view = view_by_name(store, &name)?;
            let sort_keys = parse_sort_specs(&sort, &categories)?;
            print_items_for_view(
                &view,
                &items,
                &categories,
                &category_names,
                &sort_keys,
                format,
            )?;
            Ok(())
        }
        ViewCommand::Create {
            name,
            include,
            exclude,
            hide_unmatched,
        } => {
            let categories = store.get_hierarchy().map_err(|e| e.to_string())?;
            let mut view = View::new(name);
            view.show_unmatched = !hide_unmatched;
            for id in names_to_category_ids(&categories, &include)? {
                view.criteria.set_criterion(CriterionMode::And, id);
            }
            for id in names_to_category_ids(&categories, &exclude)? {
                view.criteria.set_criterion(CriterionMode::Not, id);
            }

            store.create_view(&view).map_err(|e| e.to_string())?;
            println!("created view {}", view.name);
            Ok(())
        }
        ViewCommand::Rename { name, new_name } => {
            let mut view = view_by_name(store, &name)?;
            view.name = new_name.clone();
            store.update_view(&view).map_err(|e| e.to_string())?;
            println!("renamed view {} -> {}", name, new_name);
            Ok(())
        }
        ViewCommand::Delete { name } => {
            let view = view_by_name(store, &name)?;
            store.delete_view(view.id).map_err(|e| e.to_string())?;
            println!("deleted view {}", name);
            Ok(())
        }
    }
}

fn resolve_db_path(db_opt: Option<PathBuf>) -> Result<PathBuf, String> {
    let path = if let Some(path) = db_opt {
        path
    } else {
        let home = env::var("HOME").map_err(|_| "HOME is not set".to_string())?;
        PathBuf::from(home).join(".agenda").join("default.ag")
    };

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    Ok(path)
}

fn parse_item_id(input: &str) -> Result<ItemId, String> {
    ItemId::parse_str(input).map_err(|e| format!("invalid item id: {e}"))
}

fn category_name_map(categories: &[Category]) -> HashMap<CategoryId, String> {
    categories
        .iter()
        .map(|category| (category.id, category.name.clone()))
        .collect()
}

fn category_id_by_name(categories: &[Category], name: &str) -> Result<CategoryId, String> {
    categories
        .iter()
        .find(|category| category.name.eq_ignore_ascii_case(name))
        .map(|category| category.id)
        .ok_or_else(|| format!("category not found: {name}"))
}

fn names_to_category_ids(
    categories: &[Category],
    names: &[String],
) -> Result<HashSet<CategoryId>, String> {
    names
        .iter()
        .map(|name| category_id_by_name(categories, name))
        .collect()
}

fn view_by_name(store: &Store, name: &str) -> Result<View, String> {
    store
        .list_views()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|view| view.name.eq_ignore_ascii_case(name))
        .ok_or_else(|| format!("view not found: {name}"))
}

fn duplicate_category_create_error(
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
        "category \"{requested_name}\" already exists{id_fragment}. Category names are global across the database, so it cannot be created{parent_context}. Use `agenda category assign <item-id> \"{requested_name}\"` to assign items to the existing category."
    )
}

fn category_value_kind_label(kind: CategoryValueKind) -> &'static str {
    match kind {
        CategoryValueKind::Tag => "Tag",
        CategoryValueKind::Numeric => "Numeric",
    }
}

fn parse_decimal_value(input: &str) -> Result<Decimal, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("value cannot be empty".to_string());
    }
    let normalized = trimmed.replace(',', "");
    normalized
        .parse::<Decimal>()
        .map_err(|e| format!("invalid decimal value '{input}': {e}"))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CliSortDirection {
    Asc,
    Desc,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CliSortField {
    ItemText,
    WhenDate,
    Category(CategoryId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CliSortKey {
    field: CliSortField,
    direction: CliSortDirection,
}

fn parse_sort_specs(args: &[String], categories: &[Category]) -> Result<Vec<CliSortKey>, String> {
    args.iter()
        .map(|arg| parse_sort_spec(arg, categories))
        .collect()
}

fn parse_sort_spec(arg: &str, categories: &[Category]) -> Result<CliSortKey, String> {
    let (raw_field, direction) = parse_sort_field_and_direction(arg)?;
    let field = if raw_field.eq_ignore_ascii_case("item") {
        CliSortField::ItemText
    } else if raw_field.eq_ignore_ascii_case("when") {
        CliSortField::WhenDate
    } else {
        CliSortField::Category(category_id_by_name(categories, raw_field)?)
    };

    Ok(CliSortKey { field, direction })
}

fn parse_sort_field_and_direction(arg: &str) -> Result<(&str, CliSortDirection), String> {
    let trimmed = arg.trim();
    if trimmed.is_empty() {
        return Err("sort key cannot be empty".to_string());
    }

    if let Some((field, direction_suffix)) = trimmed.rsplit_once(':') {
        let direction = if direction_suffix.eq_ignore_ascii_case("asc") {
            CliSortDirection::Asc
        } else if direction_suffix.eq_ignore_ascii_case("desc") {
            CliSortDirection::Desc
        } else {
            return Ok((trimmed, CliSortDirection::Asc));
        };
        let field = field.trim();
        if field.is_empty() {
            return Err(format!("invalid sort key '{arg}': missing column name"));
        }
        Ok((field, direction))
    } else {
        Ok((trimmed, CliSortDirection::Asc))
    }
}

#[derive(Serialize)]
struct JsonItemRow {
    id: String,
    text: String,
    status: String,
    is_done: bool,
    when: Option<String>,
    categories: Vec<String>,
    note: Option<String>,
}

#[derive(Serialize)]
struct JsonItemsOutput {
    items: Vec<JsonItemRow>,
}

#[derive(Serialize)]
struct JsonViewSubsectionOutput {
    title: String,
    items: Vec<JsonItemRow>,
}

#[derive(Serialize)]
struct JsonViewSectionOutput {
    title: String,
    items: Vec<JsonItemRow>,
    subsections: Vec<JsonViewSubsectionOutput>,
}

#[derive(Serialize)]
struct JsonViewCategoryAliasOutput {
    category_id: String,
    category: String,
    alias: String,
}

#[derive(Serialize)]
struct JsonViewOutput {
    view: String,
    category_aliases: Vec<JsonViewCategoryAliasOutput>,
    sections: Vec<JsonViewSectionOutput>,
    unmatched_label: Option<String>,
    unmatched: Option<Vec<JsonItemRow>>,
}

struct ViewCategoryAliasRow {
    category_id: CategoryId,
    category_name: String,
    alias: String,
}

fn sorted_rows<'a>(
    items: &'a [Item],
    sort_keys: &[CliSortKey],
    categories: &[Category],
) -> Vec<&'a Item> {
    let mut rows: Vec<&Item> = items.iter().collect();
    if !sort_keys.is_empty() {
        rows.sort_by(|left, right| compare_items_by_sort_keys(left, right, sort_keys, categories));
    }
    rows
}

fn item_categories(item: &Item, category_names: &HashMap<CategoryId, String>) -> Vec<String> {
    let mut names: Vec<String> = item
        .assignments
        .keys()
        .filter_map(|id| category_names.get(id))
        .cloned()
        .collect();
    names.sort_by_key(|name| name.to_ascii_lowercase());
    names
}

fn item_row(item: &Item, category_names: &HashMap<CategoryId, String>) -> JsonItemRow {
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
    }
}

fn rows_to_json(
    items: &[Item],
    category_names: &HashMap<CategoryId, String>,
    sort_keys: &[CliSortKey],
    categories: &[Category],
) -> Vec<JsonItemRow> {
    sorted_rows(items, sort_keys, categories)
        .into_iter()
        .map(|item| item_row(item, category_names))
        .collect()
}

fn print_items_json(
    items: &[Item],
    category_names: &HashMap<CategoryId, String>,
    sort_keys: &[CliSortKey],
    categories: &[Category],
) -> Result<(), String> {
    let payload = JsonItemsOutput {
        items: rows_to_json(items, category_names, sort_keys, categories),
    };
    let body = serde_json::to_string_pretty(&payload).map_err(|e| e.to_string())?;
    println!("{body}");
    Ok(())
}

fn view_category_alias_rows(
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

fn print_items_for_view(
    view: &View,
    items: &[Item],
    categories: &[Category],
    category_names: &HashMap<CategoryId, String>,
    sort_keys: &[CliSortKey],
    output_format: OutputFormatArg,
) -> Result<(), String> {
    let reference_date = Local::now().date_naive();
    let result = resolve_view(view, items, categories, reference_date);
    let has_sections = !result.sections.is_empty();
    let alias_rows = view_category_alias_rows(view, category_names);

    if output_format == OutputFormatArg::Json {
        let mut sections = Vec::new();
        for section in result.sections {
            if section.subsections.is_empty() {
                sections.push(JsonViewSectionOutput {
                    title: section.title,
                    items: rows_to_json(&section.items, category_names, sort_keys, categories),
                    subsections: Vec::new(),
                });
                continue;
            }

            let mut subsections = Vec::new();
            for subsection in section.subsections {
                subsections.push(JsonViewSubsectionOutput {
                    title: subsection.title,
                    items: rows_to_json(&subsection.items, category_names, sort_keys, categories),
                });
            }

            sections.push(JsonViewSectionOutput {
                title: section.title,
                items: Vec::new(),
                subsections,
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
        let body = serde_json::to_string_pretty(&payload).map_err(|e| e.to_string())?;
        println!("{body}");
        return Ok(());
    }

    println!("# {}", view.name);
    if !alias_rows.is_empty() {
        println!("\nAliases:");
        for row in &alias_rows {
            println!("- {} => {}", row.category_name, row.alias);
        }
    }

    for section in result.sections {
        println!("\n## {}", section.title);
        if section.subsections.is_empty() {
            print_item_table(&section.items, category_names, sort_keys, categories);
            continue;
        }

        for subsection in section.subsections {
            println!("\n### {}", subsection.title);
            print_item_table(&subsection.items, category_names, sort_keys, categories);
        }
    }

    if let Some(unmatched) = result.unmatched {
        if !unmatched.is_empty() {
            if !has_sections {
                print_item_table(&unmatched, category_names, sort_keys, categories);
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
            print_item_table(&unmatched, category_names, sort_keys, categories);
        }
    }
    Ok(())
}

fn print_item_table(
    items: &[Item],
    category_names: &HashMap<CategoryId, String>,
    sort_keys: &[CliSortKey],
    categories: &[Category],
) {
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

fn compare_items_by_sort_keys(
    left: &Item,
    right: &Item,
    sort_keys: &[CliSortKey],
    categories: &[Category],
) -> Ordering {
    for key in sort_keys {
        let order = compare_items_by_sort_key(left, right, *key, categories);
        if order != Ordering::Equal {
            return order;
        }
    }
    Ordering::Equal
}

fn compare_items_by_sort_key(
    left: &Item,
    right: &Item,
    key: CliSortKey,
    categories: &[Category],
) -> Ordering {
    match key.field {
        CliSortField::ItemText => compare_required_values(
            left.text.to_ascii_lowercase(),
            right.text.to_ascii_lowercase(),
            key.direction,
        ),
        CliSortField::WhenDate => {
            compare_optional_values(left.when_date, right.when_date, key.direction)
        }
        CliSortField::Category(category_id) => {
            let Some(category) = categories
                .iter()
                .find(|category| category.id == category_id)
            else {
                return Ordering::Equal;
            };
            if category.value_kind == CategoryValueKind::Numeric {
                let left_value = left
                    .assignments
                    .get(&category_id)
                    .and_then(|assignment| assignment.numeric_value);
                let right_value = right
                    .assignments
                    .get(&category_id)
                    .and_then(|assignment| assignment.numeric_value);
                compare_optional_values(left_value, right_value, key.direction)
            } else {
                let left_value = category_sort_display_value(left, category, categories);
                let right_value = category_sort_display_value(right, category, categories);
                compare_optional_values(left_value, right_value, key.direction)
            }
        }
    }
}

fn category_sort_display_value(
    item: &Item,
    category: &Category,
    categories: &[Category],
) -> Option<String> {
    if category.children.is_empty() {
        return item
            .assignments
            .contains_key(&category.id)
            .then(|| category.name.to_ascii_lowercase());
    }

    let mut values: Vec<String> = category
        .children
        .iter()
        .filter(|child_id| item.assignments.contains_key(child_id))
        .map(|child_id| {
            categories
                .iter()
                .find(|candidate| candidate.id == *child_id)
                .map(|candidate| candidate.name.clone())
                .unwrap_or_else(|| child_id.to_string())
        })
        .collect();

    if values.is_empty() {
        return None;
    }

    values.sort_by_key(|value| value.to_ascii_lowercase());
    Some(values.join(", ").to_ascii_lowercase())
}

fn compare_optional_values<T: Ord>(
    left: Option<T>,
    right: Option<T>,
    direction: CliSortDirection,
) -> Ordering {
    match (left, right) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(left), Some(right)) => compare_required_values(left, right, direction),
    }
}

fn compare_required_values<T: Ord>(left: T, right: T, direction: CliSortDirection) -> Ordering {
    match direction {
        CliSortDirection::Asc => left.cmp(&right),
        CliSortDirection::Desc => right.cmp(&left),
    }
}

fn print_category_tree(categories: &[Category]) {
    let by_id: HashMap<CategoryId, &Category> = categories.iter().map(|c| (c.id, c)).collect();

    let mut roots: Vec<&Category> = categories.iter().filter(|c| c.parent.is_none()).collect();
    roots.sort_by_key(|c| c.name.to_ascii_lowercase());

    for root in roots {
        print_category_subtree(root, 0, &by_id);
    }
}

fn print_category_subtree(
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

#[cfg(test)]
mod tests {
    use super::{
        build_numeric_filters, cmd_claim, cmd_edit, cmd_unlink, cmd_view,
        compare_items_by_sort_keys, duplicate_category_create_error, item_link_section_lines,
        parse_csv_decimals, parse_decimal_value, parse_sort_spec, parsed_when_feedback_line,
        read_note_from_stdin, reject_items_with_any_categories,
        retain_items_matching_numeric_filters, retain_items_with_all_categories,
        retain_items_with_any_categories, unknown_hashtag_feedback_line, view_category_alias_rows,
        Cli, CliSortDirection, CliSortField, CliSortKey, Command, LinkCommand, ListFilters,
        NumericFilter, NumericPredicate, OutputFormatArg, UnlinkCommand, ViewCommand,
    };
    use agenda_core::agenda::Agenda;
    use agenda_core::matcher::SubstringClassifier;
    use agenda_core::model::{Category, CategoryValueKind, Item, View};
    use agenda_core::store::Store;
    use chrono::NaiveDate;
    use clap::{CommandFactory, Parser};
    use rust_decimal::Decimal;
    use std::collections::HashMap;
    use std::io::Cursor;
    use uuid::Uuid;

    #[test]
    fn duplicate_category_error_includes_assign_guidance_and_parent_context() {
        let id = Uuid::parse_str("123e4567-e89b-12d3-a456-426614174000").expect("valid uuid");
        let msg = duplicate_category_create_error("Priority", Some("Project X"), Some(id));
        assert!(msg.contains("already exists"));
        assert!(msg.contains("Category names are global"));
        assert!(msg.contains("under parent \"Project X\""));
        assert!(msg.contains("agenda category assign <item-id> \"Priority\""));
        assert!(msg.contains("123e4567-e89b-12d3-a456-426614174000"));
    }

    #[test]
    fn parsed_when_feedback_line_includes_datetime_when_present() {
        let when = NaiveDate::from_ymd_opt(2026, 2, 24)
            .expect("valid date")
            .and_hms_opt(15, 0, 0)
            .expect("valid time");

        let line = parsed_when_feedback_line(Some(when)).expect("expected line");
        assert_eq!(line, "parsed_when=2026-02-24 15:00:00");
    }

    #[test]
    fn parsed_when_feedback_line_omits_output_when_absent() {
        assert_eq!(parsed_when_feedback_line(None), None);
    }

    #[test]
    fn unknown_hashtag_feedback_line_includes_unknown_tokens() {
        let line = unknown_hashtag_feedback_line(&["office".to_string(), "someday".to_string()]);
        assert_eq!(
            line.as_deref(),
            Some("warning: unknown_hashtags=office,someday")
        );
    }

    #[test]
    fn unknown_hashtag_feedback_line_omits_when_no_unknown_tokens() {
        assert_eq!(unknown_hashtag_feedback_line(&[]), None);
    }

    #[test]
    fn parse_decimal_value_accepts_commas() {
        assert_eq!(
            parse_decimal_value("1,234.50").unwrap(),
            Decimal::new(123450, 2)
        );
    }

    #[test]
    fn parse_decimal_value_rejects_empty() {
        assert!(parse_decimal_value("   ").is_err());
    }

    #[test]
    fn view_category_alias_rows_sort_and_skip_blank_aliases() {
        let alpha = Uuid::new_v4();
        let beta = Uuid::new_v4();
        let gamma = Uuid::new_v4();

        let mut view = View::new("Aliases".to_string());
        view.category_aliases.insert(alpha, "A".to_string());
        view.category_aliases.insert(beta, "   ".to_string());
        view.category_aliases.insert(gamma, "G".to_string());

        let category_names = HashMap::from([
            (alpha, "Alpha".to_string()),
            (beta, "Beta".to_string()),
            (gamma, "gamma".to_string()),
        ]);

        let rows = view_category_alias_rows(&view, &category_names);
        assert_eq!(rows.len(), 2, "blank aliases are omitted");
        assert_eq!(rows[0].category_name, "Alpha");
        assert_eq!(rows[0].alias, "A");
        assert_eq!(rows[1].category_name, "gamma");
        assert_eq!(rows[1].alias, "G");
    }

    #[test]
    fn view_category_alias_rows_shows_deleted_category_fallback() {
        let missing = Uuid::new_v4();
        let mut view = View::new("Aliases".to_string());
        view.category_aliases
            .insert(missing, "Archived".to_string());

        let rows = view_category_alias_rows(&view, &HashMap::new());
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].category_name, format!("(deleted:{missing})"));
        assert_eq!(rows[0].alias, "Archived");
    }

    #[test]
    fn clap_parses_claim_with_defaults() {
        let cli = Cli::try_parse_from(["agenda", "claim", "123e4567-e89b-12d3-a456-426614174000"])
            .expect("parse CLI");

        match cli.command {
            Some(Command::Claim {
                item_id,
                claim_category,
                must_not_have,
            }) => {
                assert_eq!(item_id, "123e4567-e89b-12d3-a456-426614174000");
                assert_eq!(claim_category, "In Progress");
                assert_eq!(
                    must_not_have,
                    vec!["In Progress".to_string(), "Complete".to_string()]
                );
            }
            other => panic!("unexpected parse result: {other:?}"),
        }
    }

    #[test]
    fn clap_parses_claim_with_custom_preconditions() {
        let cli = Cli::try_parse_from([
            "agenda",
            "claim",
            "123e4567-e89b-12d3-a456-426614174000",
            "--claim-category",
            "Ready",
            "--must-not-have",
            "In Progress",
            "--must-not-have",
            "Complete",
            "--must-not-have",
            "Waiting/Blocked",
        ])
        .expect("parse CLI");

        match cli.command {
            Some(Command::Claim {
                claim_category,
                must_not_have,
                ..
            }) => {
                assert_eq!(claim_category, "Ready");
                assert_eq!(
                    must_not_have,
                    vec![
                        "In Progress".to_string(),
                        "Complete".to_string(),
                        "Waiting/Blocked".to_string()
                    ]
                );
            }
            other => panic!("unexpected parse result: {other:?}"),
        }
    }

    #[test]
    fn claim_help_includes_agent_examples() {
        let mut cmd = Cli::command();
        let claim_cmd = cmd
            .find_subcommand_mut("claim")
            .expect("claim subcommand should exist");
        let help = claim_cmd.render_help().to_string();
        assert!(help.contains("Defaults (`agenda claim <ITEM_ID>`):"));
        assert!(help.contains("--claim-category \"In Progress\""));
        assert!(help.contains("--must-not-have \"Complete\""));
        assert!(help.contains("Create an `In Progress` category"));
        assert!(help.contains("agenda category create \"In Progress\" --parent Status"));
        assert!(help.contains("Examples:"));
        assert!(help.contains("agenda claim <ITEM_ID>"));
        assert!(help.contains("--must-not-have"));
    }

    #[test]
    fn cmd_claim_fails_when_precondition_category_already_assigned() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut status = Category::new("Status".to_string());
        status.is_exclusive = true;
        store.create_category(&status).expect("create status");
        let mut in_progress = Category::new("In Progress".to_string());
        in_progress.parent = Some(status.id);
        store
            .create_category(&in_progress)
            .expect("create in-progress");
        let mut complete = Category::new("Complete".to_string());
        complete.parent = Some(status.id);
        store.create_category(&complete).expect("create complete");

        let item = Item::new("Claim target".to_string());
        store.create_item(&item).expect("create item");
        agenda
            .assign_item_manual(
                item.id,
                in_progress.id,
                Some("manual:test.assign".to_string()),
            )
            .expect("seed in-progress");

        let err = cmd_claim(
            &agenda,
            &store,
            item.id.to_string(),
            "In Progress".to_string(),
            vec!["In Progress".to_string(), "Complete".to_string()],
        )
        .expect_err("claim should fail");
        assert!(err.contains("claim precondition failed"));
    }

    #[test]
    fn cmd_claim_assigns_claim_category_and_clears_exclusive_sibling() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut status = Category::new("Status".to_string());
        status.is_exclusive = true;
        store.create_category(&status).expect("create status");
        let mut ready = Category::new("Ready".to_string());
        ready.parent = Some(status.id);
        store.create_category(&ready).expect("create ready");
        let mut in_progress = Category::new("In Progress".to_string());
        in_progress.parent = Some(status.id);
        store
            .create_category(&in_progress)
            .expect("create in-progress");
        let mut complete = Category::new("Complete".to_string());
        complete.parent = Some(status.id);
        store.create_category(&complete).expect("create complete");

        let item = Item::new("Claim target".to_string());
        store.create_item(&item).expect("create item");
        agenda
            .assign_item_manual(item.id, ready.id, Some("manual:test.assign".to_string()))
            .expect("seed ready");

        cmd_claim(
            &agenda,
            &store,
            item.id.to_string(),
            "In Progress".to_string(),
            vec!["In Progress".to_string(), "Complete".to_string()],
        )
        .expect("claim should succeed");

        let assignments = store
            .get_assignments_for_item(item.id)
            .expect("load assignments");
        assert!(assignments.contains_key(&in_progress.id));
        assert!(!assignments.contains_key(&ready.id));
    }

    #[test]
    fn clap_parses_link_depends_on_subcommand() {
        let cli = Cli::try_parse_from([
            "agenda",
            "link",
            "depends-on",
            "123e4567-e89b-12d3-a456-426614174000",
            "123e4567-e89b-12d3-a456-426614174001",
        ])
        .expect("parse CLI");

        match cli.command {
            Some(Command::Link {
                command:
                    LinkCommand::DependsOn {
                        item_id,
                        depends_on_item_id,
                    },
            }) => {
                assert_eq!(item_id, "123e4567-e89b-12d3-a456-426614174000");
                assert_eq!(depends_on_item_id, "123e4567-e89b-12d3-a456-426614174001");
            }
            other => panic!("unexpected parse result: {other:?}"),
        }
    }

    #[test]
    fn clap_parses_top_level_unlink_depends_on_subcommand() {
        let cli =
            Cli::try_parse_from(["agenda", "unlink", "depends-on", "a", "b"]).expect("parse CLI");

        match cli.command {
            Some(Command::Unlink {
                command:
                    UnlinkCommand::DependsOn {
                        item_id,
                        depends_on_item_id,
                    },
            }) => {
                assert_eq!(item_id, "a");
                assert_eq!(depends_on_item_id, "b");
            }
            other => panic!("unexpected parse result: {other:?}"),
        }
    }

    #[test]
    fn clap_parses_list_with_repeated_sort_flags() {
        let cli = Cli::try_parse_from([
            "agenda",
            "list",
            "--sort",
            "item:desc",
            "--sort",
            "Priority:asc",
        ])
        .expect("parse CLI");

        match cli.command {
            Some(Command::List { sort, .. }) => {
                assert_eq!(
                    sort,
                    vec!["item:desc".to_string(), "Priority:asc".to_string()]
                );
            }
            other => panic!("unexpected parse result: {other:?}"),
        }
    }

    #[test]
    fn clap_parses_list_with_repeated_category_flags() {
        let cli = Cli::try_parse_from([
            "agenda",
            "list",
            "--category",
            "Feature request",
            "--category",
            "Ready",
        ])
        .expect("parse CLI");

        match cli.command {
            Some(Command::List { category, .. }) => {
                assert_eq!(
                    category,
                    vec!["Feature request".to_string(), "Ready".to_string()]
                );
            }
            other => panic!("unexpected parse result: {other:?}"),
        }
    }

    #[test]
    fn list_help_documents_repeated_category_and_semantics() {
        let mut cmd = Cli::command();
        let list_cmd = cmd
            .find_subcommand_mut("list")
            .expect("list subcommand should exist");
        let category_arg = list_cmd
            .get_arguments()
            .find(|arg| arg.get_id().as_str() == "category")
            .expect("list --category argument should exist");
        let help = category_arg
            .get_help()
            .expect("list --category should have help text")
            .to_string();

        assert!(help.contains("repeat for AND"));
        assert!(help.contains("ALL specified categories"));
    }

    #[test]
    fn clap_parses_list_with_repeated_any_category_flags() {
        let cli = Cli::try_parse_from([
            "agenda",
            "list",
            "--any-category",
            "Aglet",
            "--any-category",
            "NeoNV",
        ])
        .expect("parse CLI");

        match cli.command {
            Some(Command::List { any_category, .. }) => {
                assert_eq!(any_category, vec!["Aglet".to_string(), "NeoNV".to_string()]);
            }
            other => panic!("unexpected parse result: {other:?}"),
        }
    }

    #[test]
    fn clap_parses_list_with_repeated_exclude_category_flags() {
        let cli = Cli::try_parse_from([
            "agenda",
            "list",
            "--exclude-category",
            "Complete",
            "--exclude-category",
            "Deferred",
        ])
        .expect("parse CLI");

        match cli.command {
            Some(Command::List {
                exclude_category, ..
            }) => {
                assert_eq!(
                    exclude_category,
                    vec!["Complete".to_string(), "Deferred".to_string()]
                );
            }
            other => panic!("unexpected parse result: {other:?}"),
        }
    }

    #[test]
    fn clap_parses_list_with_repeated_value_eq_flags() {
        let cli = Cli::try_parse_from([
            "agenda",
            "list",
            "--value-eq",
            "Complexity",
            "2",
            "--value-eq",
            "Cost",
            "10",
        ])
        .expect("parse CLI");

        match cli.command {
            Some(Command::List { value_eq, .. }) => {
                assert_eq!(
                    value_eq,
                    vec![
                        "Complexity".to_string(),
                        "2".to_string(),
                        "Cost".to_string(),
                        "10".to_string()
                    ]
                );
            }
            other => panic!("unexpected parse result: {other:?}"),
        }
    }

    #[test]
    fn clap_parses_list_with_repeated_value_in_flags() {
        let cli = Cli::try_parse_from([
            "agenda",
            "list",
            "--value-in",
            "Complexity",
            "1,2",
            "--value-in",
            "Cost",
            "10,20",
        ])
        .expect("parse CLI");

        match cli.command {
            Some(Command::List { value_in, .. }) => {
                assert_eq!(
                    value_in,
                    vec![
                        "Complexity".to_string(),
                        "1,2".to_string(),
                        "Cost".to_string(),
                        "10,20".to_string()
                    ]
                );
            }
            other => panic!("unexpected parse result: {other:?}"),
        }
    }

    #[test]
    fn clap_parses_list_with_repeated_value_max_flags() {
        let cli = Cli::try_parse_from([
            "agenda",
            "list",
            "--value-max",
            "Complexity",
            "2",
            "--value-max",
            "Cost",
            "100",
        ])
        .expect("parse CLI");

        match cli.command {
            Some(Command::List { value_max, .. }) => {
                assert_eq!(
                    value_max,
                    vec![
                        "Complexity".to_string(),
                        "2".to_string(),
                        "Cost".to_string(),
                        "100".to_string()
                    ]
                );
            }
            other => panic!("unexpected parse result: {other:?}"),
        }
    }

    #[test]
    fn list_help_includes_numeric_filter_examples() {
        let mut cmd = Cli::command();
        let list_cmd = cmd
            .find_subcommand_mut("list")
            .expect("list subcommand should exist");
        let help = list_cmd.render_help().to_string();
        assert!(help.contains("Numeric value filter examples:"));
        assert!(help.contains("--value-in Complexity 1,2"));
        assert!(help.contains("--value-max Complexity 2"));
    }

    #[test]
    fn clap_parses_view_show_with_sort_flag() {
        let cli = Cli::try_parse_from(["agenda", "view", "show", "All Items", "--sort", "when"])
            .expect("parse CLI");

        match cli.command {
            Some(Command::View {
                command: ViewCommand::Show { name, sort, format },
            }) => {
                assert_eq!(name, "All Items");
                assert_eq!(sort, vec!["when".to_string()]);
                assert_eq!(format, OutputFormatArg::Table);
            }
            other => panic!("unexpected parse result: {other:?}"),
        }
    }

    #[test]
    fn clap_parses_list_with_json_format() {
        let cli = Cli::try_parse_from(["agenda", "list", "--format", "json"]).expect("parse CLI");

        match cli.command {
            Some(Command::List { format, .. }) => {
                assert_eq!(format, OutputFormatArg::Json);
            }
            other => panic!("unexpected parse result: {other:?}"),
        }
    }

    #[test]
    fn clap_parses_search_with_json_format() {
        let cli = Cli::try_parse_from(["agenda", "search", "foo", "--format", "json"])
            .expect("parse CLI");

        match cli.command {
            Some(Command::Search { format, .. }) => {
                assert_eq!(format, OutputFormatArg::Json);
            }
            other => panic!("unexpected parse result: {other:?}"),
        }
    }

    #[test]
    fn clap_parses_view_show_with_json_format() {
        let cli = Cli::try_parse_from(["agenda", "view", "show", "All Items", "--format", "json"])
            .expect("parse CLI");

        match cli.command {
            Some(Command::View {
                command: ViewCommand::Show { format, .. },
            }) => {
                assert_eq!(format, OutputFormatArg::Json);
            }
            other => panic!("unexpected parse result: {other:?}"),
        }
    }

    #[test]
    fn cmd_view_rename_rejects_all_items() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let err = cmd_view(
            &agenda,
            &store,
            ViewCommand::Rename {
                name: "All Items".to_string(),
                new_name: "Renamed".to_string(),
            },
        )
        .expect_err("rename should fail");
        assert!(err.contains("cannot modify system view"));
    }

    #[test]
    fn cmd_view_delete_rejects_all_items() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let err = cmd_view(
            &agenda,
            &store,
            ViewCommand::Delete {
                name: "All Items".to_string(),
            },
        )
        .expect_err("delete should fail");
        assert!(err.contains("cannot modify system view"));
    }

    #[test]
    fn parse_sort_spec_supports_when_and_direction_suffix() {
        let categories = vec![Category::new("Priority".to_string())];
        let when_key = parse_sort_spec("when:desc", &categories).expect("parse when desc");
        assert_eq!(
            when_key,
            CliSortKey {
                field: CliSortField::WhenDate,
                direction: CliSortDirection::Desc
            }
        );
        let item_key = parse_sort_spec("item", &categories).expect("parse item default");
        assert_eq!(
            item_key,
            CliSortKey {
                field: CliSortField::ItemText,
                direction: CliSortDirection::Asc
            }
        );
    }

    #[test]
    fn compare_items_by_sort_keys_numeric_missing_values_are_last() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut cost = Category::new("Cost".to_string());
        cost.value_kind = CategoryValueKind::Numeric;
        store.create_category(&cost).expect("create cost");

        let ten = Item::new("Ten".to_string());
        let missing = Item::new("Missing".to_string());
        let five = Item::new("Five".to_string());
        store.create_item(&ten).expect("create ten");
        store.create_item(&missing).expect("create missing");
        store.create_item(&five).expect("create five");

        agenda
            .assign_item_numeric_manual(
                ten.id,
                cost.id,
                Decimal::new(10, 0),
                Some("test:assign".to_string()),
            )
            .expect("assign ten");
        agenda
            .assign_item_numeric_manual(
                five.id,
                cost.id,
                Decimal::new(5, 0),
                Some("test:assign".to_string()),
            )
            .expect("assign five");

        let categories = store.get_hierarchy().expect("hierarchy");
        let key_asc = CliSortKey {
            field: CliSortField::Category(cost.id),
            direction: CliSortDirection::Asc,
        };
        let key_desc = CliSortKey {
            field: CliSortField::Category(cost.id),
            direction: CliSortDirection::Desc,
        };

        let mut rows = store.list_items().expect("list items");
        rows.sort_by(|left, right| {
            compare_items_by_sort_keys(left, right, &[key_asc], &categories)
        });
        let asc_texts: Vec<String> = rows.iter().map(|item| item.text.clone()).collect();
        assert_eq!(
            asc_texts,
            vec!["Five".to_string(), "Ten".to_string(), "Missing".to_string()]
        );

        rows.sort_by(|left, right| {
            compare_items_by_sort_keys(left, right, &[key_desc], &categories)
        });
        let desc_texts: Vec<String> = rows.iter().map(|item| item.text.clone()).collect();
        assert_eq!(
            desc_texts,
            vec!["Ten".to_string(), "Five".to_string(), "Missing".to_string()]
        );
    }

    #[test]
    fn retain_items_with_all_categories_enforces_and_semantics() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let issue_type = Category::new("Issue type".to_string());
        let status = Category::new("Status".to_string());
        store
            .create_category(&issue_type)
            .expect("create issue_type");
        store.create_category(&status).expect("create status");

        let both = Item::new("Both".to_string());
        let one = Item::new("One".to_string());
        let none = Item::new("None".to_string());
        store.create_item(&both).expect("create both");
        store.create_item(&one).expect("create one");
        store.create_item(&none).expect("create none");

        agenda
            .assign_item_manual(both.id, issue_type.id, Some("test:assign".to_string()))
            .expect("assign both issue_type");
        agenda
            .assign_item_manual(both.id, status.id, Some("test:assign".to_string()))
            .expect("assign both status");
        agenda
            .assign_item_manual(one.id, issue_type.id, Some("test:assign".to_string()))
            .expect("assign one issue_type");

        let mut rows = store.list_items().expect("list items");
        retain_items_with_all_categories(&mut rows, &[issue_type.id, status.id]);

        let remaining_texts: Vec<String> = rows.into_iter().map(|item| item.text).collect();
        assert_eq!(remaining_texts, vec!["Both".to_string()]);
    }

    #[test]
    fn retain_items_with_any_categories_enforces_or_semantics() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let aglet = Category::new("Aglet".to_string());
        let neonv = Category::new("NeoNV".to_string());
        let other = Category::new("Project3".to_string());
        store.create_category(&aglet).expect("create aglet");
        store.create_category(&neonv).expect("create neonv");
        store.create_category(&other).expect("create project3");

        let aglet_item = Item::new("Aglet item".to_string());
        let neonv_item = Item::new("NeoNV item".to_string());
        let other_item = Item::new("Project3 item".to_string());
        store.create_item(&aglet_item).expect("create aglet item");
        store.create_item(&neonv_item).expect("create neonv item");
        store
            .create_item(&other_item)
            .expect("create project3 item");

        agenda
            .assign_item_manual(aglet_item.id, aglet.id, Some("test:assign".to_string()))
            .expect("assign aglet");
        agenda
            .assign_item_manual(neonv_item.id, neonv.id, Some("test:assign".to_string()))
            .expect("assign neonv");
        agenda
            .assign_item_manual(other_item.id, other.id, Some("test:assign".to_string()))
            .expect("assign project3");

        let mut rows = store.list_items().expect("list items");
        retain_items_with_any_categories(&mut rows, &[aglet.id, neonv.id]);

        let mut remaining_texts: Vec<String> = rows.into_iter().map(|item| item.text).collect();
        remaining_texts.sort();
        assert_eq!(
            remaining_texts,
            vec!["Aglet item".to_string(), "NeoNV item".to_string()]
        );
    }

    #[test]
    fn parse_csv_decimals_rejects_empty_value_token() {
        let err = parse_csv_decimals("1,,2", "Complexity").expect_err("should fail");
        assert_eq!(
            err,
            "invalid --value-in for category 'Complexity': empty value in CSV list"
        );
    }

    #[test]
    fn build_numeric_filters_rejects_unknown_category() {
        let categories = vec![Category::new("Complexity".to_string())];
        let filters = ListFilters {
            all_categories: Vec::new(),
            any_categories: Vec::new(),
            exclude_categories: Vec::new(),
            value_eq: vec!["Nope".to_string(), "2".to_string()],
            value_in: Vec::new(),
            value_max: Vec::new(),
            include_done: false,
        };

        let err = build_numeric_filters(&categories, &filters).expect_err("should fail");
        assert_eq!(err, "category not found: Nope");
    }

    #[test]
    fn build_numeric_filters_rejects_tag_category() {
        let categories = vec![Category::new("Status".to_string())];
        let filters = ListFilters {
            all_categories: Vec::new(),
            any_categories: Vec::new(),
            exclude_categories: Vec::new(),
            value_eq: vec!["Status".to_string(), "2".to_string()],
            value_in: Vec::new(),
            value_max: Vec::new(),
            include_done: false,
        };

        let err = build_numeric_filters(&categories, &filters).expect_err("should fail");
        assert_eq!(
            err,
            "category 'Status' is not Numeric; numeric value filters require a Numeric category"
        );
    }

    #[test]
    fn build_numeric_filters_rejects_malformed_decimal() {
        let mut complexity = Category::new("Complexity".to_string());
        complexity.value_kind = CategoryValueKind::Numeric;
        let filters = ListFilters {
            all_categories: Vec::new(),
            any_categories: Vec::new(),
            exclude_categories: Vec::new(),
            value_eq: vec!["Complexity".to_string(), "abc".to_string()],
            value_in: Vec::new(),
            value_max: Vec::new(),
            include_done: false,
        };

        let err = build_numeric_filters(&[complexity], &filters).expect_err("should fail");
        assert!(err.contains("invalid decimal value 'abc'"));
    }

    #[test]
    fn retain_items_matching_numeric_filters_handles_eq_in_max_and_missing_values() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut complexity = Category::new("Complexity".to_string());
        complexity.value_kind = CategoryValueKind::Numeric;
        store
            .create_category(&complexity)
            .expect("create complexity");

        let one = Item::new("One".to_string());
        let two = Item::new("Two".to_string());
        let five = Item::new("Five".to_string());
        let missing = Item::new("Missing".to_string());
        store.create_item(&one).expect("create one");
        store.create_item(&two).expect("create two");
        store.create_item(&five).expect("create five");
        store.create_item(&missing).expect("create missing");

        agenda
            .assign_item_numeric_manual(
                one.id,
                complexity.id,
                Decimal::new(1, 0),
                Some("test:set".to_string()),
            )
            .expect("set one");
        agenda
            .assign_item_numeric_manual(
                two.id,
                complexity.id,
                Decimal::new(2, 0),
                Some("test:set".to_string()),
            )
            .expect("set two");
        agenda
            .assign_item_numeric_manual(
                five.id,
                complexity.id,
                Decimal::new(5, 0),
                Some("test:set".to_string()),
            )
            .expect("set five");

        let mut rows = store.list_items().expect("list items");
        retain_items_matching_numeric_filters(
            &mut rows,
            &[NumericFilter {
                category_id: complexity.id,
                category_name: "Complexity".to_string(),
                predicate: NumericPredicate::Eq(Decimal::new(2, 0)),
            }],
        );
        assert_eq!(
            rows.into_iter().map(|i| i.text).collect::<Vec<_>>(),
            vec!["Two".to_string()]
        );

        let mut rows = store.list_items().expect("list items");
        retain_items_matching_numeric_filters(
            &mut rows,
            &[NumericFilter {
                category_id: complexity.id,
                category_name: "Complexity".to_string(),
                predicate: NumericPredicate::Max(Decimal::new(2, 0)),
            }],
        );
        let mut max_texts: Vec<String> = rows.into_iter().map(|i| i.text).collect();
        max_texts.sort();
        assert_eq!(max_texts, vec!["One".to_string(), "Two".to_string()]);

        let mut rows = store.list_items().expect("list items");
        retain_items_matching_numeric_filters(
            &mut rows,
            &[NumericFilter {
                category_id: complexity.id,
                category_name: "Complexity".to_string(),
                predicate: NumericPredicate::In(vec![Decimal::new(1, 0), Decimal::new(5, 0)]),
            }],
        );
        let mut in_texts: Vec<String> = rows.into_iter().map(|i| i.text).collect();
        in_texts.sort();
        assert_eq!(in_texts, vec!["Five".to_string(), "One".to_string()]);
    }

    #[test]
    fn numeric_filters_compose_with_category_include_and_exclude_filters() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let project = Category::new("Aglet".to_string());
        let done = Category::new("Complete".to_string());
        let mut complexity = Category::new("Complexity".to_string());
        complexity.value_kind = CategoryValueKind::Numeric;
        store.create_category(&project).expect("create project");
        store.create_category(&done).expect("create complete");
        store
            .create_category(&complexity)
            .expect("create complexity");

        let include_keep = Item::new("IncludeKeep".to_string());
        let include_drop_value = Item::new("IncludeDropValue".to_string());
        let include_drop_excluded = Item::new("IncludeDropExcluded".to_string());
        store
            .create_item(&include_keep)
            .expect("create include keep");
        store
            .create_item(&include_drop_value)
            .expect("create include drop value");
        store
            .create_item(&include_drop_excluded)
            .expect("create include drop excluded");

        for item_id in [
            include_keep.id,
            include_drop_value.id,
            include_drop_excluded.id,
        ] {
            agenda
                .assign_item_manual(item_id, project.id, Some("test:assign".to_string()))
                .expect("assign project");
        }
        agenda
            .assign_item_manual(
                include_drop_excluded.id,
                done.id,
                Some("test:assign".to_string()),
            )
            .expect("assign complete");
        agenda
            .assign_item_numeric_manual(
                include_keep.id,
                complexity.id,
                Decimal::new(2, 0),
                Some("test:set".to_string()),
            )
            .expect("set keep");
        agenda
            .assign_item_numeric_manual(
                include_drop_value.id,
                complexity.id,
                Decimal::new(5, 0),
                Some("test:set".to_string()),
            )
            .expect("set drop value");
        agenda
            .assign_item_numeric_manual(
                include_drop_excluded.id,
                complexity.id,
                Decimal::new(2, 0),
                Some("test:set".to_string()),
            )
            .expect("set excluded");

        let mut rows = store.list_items().expect("list items");
        retain_items_with_all_categories(&mut rows, &[project.id]);
        reject_items_with_any_categories(&mut rows, &[done.id]);
        retain_items_matching_numeric_filters(
            &mut rows,
            &[NumericFilter {
                category_id: complexity.id,
                category_name: "Complexity".to_string(),
                predicate: NumericPredicate::Max(Decimal::new(2, 0)),
            }],
        );

        assert_eq!(
            rows.into_iter().map(|i| i.text).collect::<Vec<_>>(),
            vec!["IncludeKeep".to_string()]
        );
    }

    #[test]
    fn multiple_numeric_filters_use_and_semantics() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut complexity = Category::new("Complexity".to_string());
        complexity.value_kind = CategoryValueKind::Numeric;
        store
            .create_category(&complexity)
            .expect("create complexity");

        let one = Item::new("One".to_string());
        let two = Item::new("Two".to_string());
        let three = Item::new("Three".to_string());
        store.create_item(&one).expect("create one");
        store.create_item(&two).expect("create two");
        store.create_item(&three).expect("create three");
        agenda
            .assign_item_numeric_manual(
                one.id,
                complexity.id,
                Decimal::new(1, 0),
                Some("test:set".to_string()),
            )
            .expect("set one");
        agenda
            .assign_item_numeric_manual(
                two.id,
                complexity.id,
                Decimal::new(2, 0),
                Some("test:set".to_string()),
            )
            .expect("set two");
        agenda
            .assign_item_numeric_manual(
                three.id,
                complexity.id,
                Decimal::new(3, 0),
                Some("test:set".to_string()),
            )
            .expect("set three");

        let mut rows = store.list_items().expect("list items");
        retain_items_matching_numeric_filters(
            &mut rows,
            &[
                NumericFilter {
                    category_id: complexity.id,
                    category_name: "Complexity".to_string(),
                    predicate: NumericPredicate::In(vec![
                        Decimal::new(1, 0),
                        Decimal::new(2, 0),
                        Decimal::new(3, 0),
                    ]),
                },
                NumericFilter {
                    category_id: complexity.id,
                    category_name: "Complexity".to_string(),
                    predicate: NumericPredicate::Max(Decimal::new(2, 0)),
                },
            ],
        );
        let mut texts: Vec<String> = rows.into_iter().map(|i| i.text).collect();
        texts.sort();
        assert_eq!(texts, vec!["One".to_string(), "Two".to_string()]);
    }

    #[test]
    fn reject_items_with_any_categories_enforces_not_semantics() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let complete = Category::new("Complete".to_string());
        let in_progress = Category::new("In Progress".to_string());
        store.create_category(&complete).expect("create complete");
        store
            .create_category(&in_progress)
            .expect("create in-progress");

        let done_item = Item::new("Done item".to_string());
        let active_item = Item::new("Active item".to_string());
        store.create_item(&done_item).expect("create done item");
        store.create_item(&active_item).expect("create active item");

        agenda
            .assign_item_manual(done_item.id, complete.id, Some("test:assign".to_string()))
            .expect("assign complete");
        agenda
            .assign_item_manual(
                active_item.id,
                in_progress.id,
                Some("test:assign".to_string()),
            )
            .expect("assign in-progress");

        let mut rows = store.list_items().expect("list items");
        reject_items_with_any_categories(&mut rows, &[complete.id]);

        let remaining_texts: Vec<String> = rows.into_iter().map(|item| item.text).collect();
        assert_eq!(remaining_texts, vec!["Active item".to_string()]);
    }

    #[test]
    fn item_link_section_lines_include_prereqs_blocks_and_related() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let a = Item::new("Task A".to_string());
        let b = Item::new("Task B".to_string());
        let c = Item::new("Task C".to_string());
        let d = Item::new("Task D".to_string());
        store.create_item(&a).expect("create a");
        store.create_item(&b).expect("create b");
        store.create_item(&c).expect("create c");
        store.create_item(&d).expect("create d");

        agenda
            .link_items_depends_on(a.id, b.id)
            .expect("link depends-on");
        agenda.link_items_blocks(c.id, a.id).expect("link blocks");
        agenda.link_items_related(a.id, d.id).expect("link related");

        let lines = item_link_section_lines(&store, a.id).expect("render link lines");
        assert!(lines.iter().any(|line| line == "prereqs:"));
        assert!(lines
            .iter()
            .any(|line| line == "dependents (blocks): (none)" || line == "dependents (blocks):"));
        assert!(lines.iter().any(|line| line == "related:"));
        assert!(lines.iter().any(|line| line.contains("Task B")));
        assert!(lines.iter().any(|line| line.contains("Task C")));
        assert!(lines.iter().any(|line| line.contains("Task D")));
    }

    #[test]
    fn cmd_edit_append_note_to_empty() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let item = Item::new("Test item".to_string());
        store.create_item(&item).expect("create");

        cmd_edit(
            &agenda,
            item.id.to_string(),
            None,
            None,
            Some("first note".to_string()),
            None,
            false,
            None,
        )
        .expect("append to empty");

        let updated = store.get_item(item.id).expect("get item");
        assert_eq!(updated.note.as_deref(), Some("first note"));
    }

    #[test]
    fn cmd_edit_append_note_to_existing() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut item = Item::new("Test item".to_string());
        item.note = Some("existing note".to_string());
        store.create_item(&item).expect("create");

        cmd_edit(
            &agenda,
            item.id.to_string(),
            None,
            None,
            Some("appended text".to_string()),
            None,
            false,
            None,
        )
        .expect("append to existing");

        let updated = store.get_item(item.id).expect("get item");
        assert_eq!(
            updated.note.as_deref(),
            Some("existing note\nappended text")
        );
    }

    #[test]
    fn cmd_edit_append_note_multiline() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut item = Item::new("Test item".to_string());
        item.note = Some("line one".to_string());
        store.create_item(&item).expect("create");

        cmd_edit(
            &agenda,
            item.id.to_string(),
            None,
            None,
            Some("line two\nline three".to_string()),
            None,
            false,
            None,
        )
        .expect("append multiline");

        let updated = store.get_item(item.id).expect("get item");
        assert_eq!(
            updated.note.as_deref(),
            Some("line one\nline two\nline three")
        );
    }

    #[test]
    fn cmd_edit_append_note_rejects_with_note_flag() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let result = cmd_edit(
            &agenda,
            "123e4567-e89b-12d3-a456-426614174000".to_string(),
            None,
            Some("replace".to_string()),
            Some("append".to_string()),
            None,
            false,
            None,
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("mutually exclusive"));
    }

    #[test]
    fn cmd_edit_append_note_rejects_with_clear_note_flag() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let result = cmd_edit(
            &agenda,
            "123e4567-e89b-12d3-a456-426614174000".to_string(),
            None,
            None,
            Some("append".to_string()),
            None,
            true,
            None,
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("mutually exclusive"));
    }

    #[test]
    fn read_note_from_stdin_reads_multiline_payload() {
        let mut reader = Cursor::new("line one\nline two\n");
        let note = read_note_from_stdin(&mut reader).expect("read note");
        assert_eq!(note, "line one\nline two\n");
    }

    #[test]
    fn cmd_edit_note_stdin_replaces_existing_note() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut item = Item::new("Test item".to_string());
        item.note = Some("old note".to_string());
        store.create_item(&item).expect("create");

        cmd_edit(
            &agenda,
            item.id.to_string(),
            None,
            None,
            None,
            Some("stdin note\nnext line".to_string()),
            false,
            None,
        )
        .expect("replace from stdin");

        let updated = store.get_item(item.id).expect("get item");
        assert_eq!(updated.note.as_deref(), Some("stdin note\nnext line"));
    }

    #[test]
    fn cmd_edit_note_stdin_rejects_with_note_flag() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let result = cmd_edit(
            &agenda,
            "123e4567-e89b-12d3-a456-426614174000".to_string(),
            None,
            Some("replace".to_string()),
            None,
            Some("stdin".to_string()),
            false,
            None,
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("mutually exclusive"));
    }

    #[test]
    fn cmd_edit_note_stdin_empty_is_noop() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let mut item = Item::new("Test item".to_string());
        item.note = Some("existing".to_string());
        let previous_modified_at = item.modified_at;
        store.create_item(&item).expect("create");

        cmd_edit(
            &agenda,
            item.id.to_string(),
            None,
            None,
            None,
            Some(String::new()),
            false,
            None,
        )
        .expect("empty stdin no-op");

        let updated = store.get_item(item.id).expect("get item");
        assert_eq!(updated.note.as_deref(), Some("existing"));
        assert_eq!(updated.modified_at, previous_modified_at);
    }

    #[test]
    fn cmd_unlink_removes_dependency_link() {
        let store = Store::open_memory().expect("store");
        let classifier = SubstringClassifier;
        let agenda = Agenda::new(&store, &classifier);

        let a = Item::new("Task A".to_string());
        let b = Item::new("Task B".to_string());
        store.create_item(&a).expect("create a");
        store.create_item(&b).expect("create b");

        agenda
            .link_items_depends_on(a.id, b.id)
            .expect("link depends-on");

        cmd_unlink(
            &agenda,
            UnlinkCommand::DependsOn {
                item_id: a.id.to_string(),
                depends_on_item_id: b.id.to_string(),
            },
        )
        .expect("unlink via canonical command");
        assert!(store
            .list_dependency_ids_for_item(a.id)
            .expect("list dependencies")
            .is_empty());
    }
}
