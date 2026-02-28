use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
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

#[derive(Subcommand, Debug)]
enum Command {
    /// Add a new item
    Add {
        text: String,
        #[arg(long)]
        note: Option<String>,
    },

    /// Edit an existing item's text, note, and/or done state
    Edit {
        item_id: String,
        /// New text (positional shorthand; also available as --text)
        text: Option<String>,
        #[arg(long)]
        note: Option<String>,
        #[arg(long = "clear-note")]
        clear_note: bool,
        #[arg(long)]
        done: Option<bool>,
    },

    /// Show a single item with its assignments
    Show { item_id: String },

    /// List items (optionally filtered)
    List {
        #[arg(long)]
        view: Option<String>,
        #[arg(long)]
        category: Option<String>,
        #[arg(long)]
        include_done: bool,
    },

    /// Search item text and note
    Search {
        query: String,
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
    Show { name: String },

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

    /// Remove a dependency link: ITEM no longer depends on DEPENDS_ON_ITEM
    #[command(name = "unlink-depends-on")]
    UnlinkDependsOn {
        item_id: String,
        depends_on_item_id: String,
    },

    /// Remove inverse dependency vocabulary: BLOCKER no longer blocks BLOCKED
    #[command(name = "unlink-blocks")]
    UnlinkBlocks {
        blocker_item_id: String,
        blocked_item_id: String,
    },

    /// Remove a related link
    #[command(name = "unlink-related")]
    UnlinkRelated {
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
        category: None,
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
            clear_note,
            done,
        } => cmd_edit(&agenda, item_id, text, note, clear_note, done),
        Command::Show { item_id } => cmd_show(&store, item_id),
        Command::List {
            view,
            category,
            include_done,
        } => cmd_list(&store, view, category, include_done),
        Command::Search {
            query,
            include_done,
        } => cmd_search(&store, query, include_done),
        Command::Delete { item_id } => cmd_delete(&agenda, item_id),
        Command::Deleted => cmd_deleted(&store),
        Command::Restore { log_id } => cmd_restore(&store, log_id),
        Command::Category { command } => cmd_category(&agenda, &store, command),
        Command::View { command } => cmd_view(&agenda, &store, command),
        Command::Link { command } => cmd_link(&agenda, command),
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

fn cmd_edit(
    agenda: &Agenda<'_>,
    item_id_str: String,
    text: Option<String>,
    note: Option<String>,
    clear_note: bool,
    done: Option<bool>,
) -> Result<(), String> {
    let item_id = parse_item_id(&item_id_str)?;

    if text.is_none() && note.is_none() && !clear_note && done.is_none() {
        return Err(
            "nothing to update\n\nUsage: agenda edit <ITEM_ID> [TEXT] [--note <NOTE>] [--clear-note] [--done <true|false>]\n\nExamples:\n  agenda edit <id> \"new text here\"\n  agenda edit <id> --note \"updated note\"\n  agenda edit <id> \"new text\" --note \"and note\"\n  agenda edit <id> --clear-note\n  agenda edit <id> --done true\n  agenda edit <id> --done false".to_string()
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

    if text.is_some() || note.is_some() || clear_note {
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

fn cmd_list(
    store: &Store,
    view_name: Option<String>,
    category_name: Option<String>,
    include_done: bool,
) -> Result<(), String> {
    let mut items = store.list_items().map_err(|e| e.to_string())?;
    if !include_done {
        items.retain(|item| !item.is_done);
    }

    let categories = store.get_hierarchy().map_err(|e| e.to_string())?;
    let category_names = category_name_map(&categories);

    if let Some(category_name) = category_name {
        let category_id = category_id_by_name(&categories, &category_name)?;
        items.retain(|item| item.assignments.contains_key(&category_id));
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
        print_items_for_view(&view, &items, &categories, &category_names);
    } else {
        print_item_table(&items, &category_names);
    }
    Ok(())
}

fn cmd_search(store: &Store, query: String, include_done: bool) -> Result<(), String> {
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
    print_item_table(&matched_items, &category_names);
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
        LinkCommand::UnlinkDependsOn {
            item_id,
            depends_on_item_id,
        } => {
            let item_id = parse_item_id(&item_id)?;
            let depends_on_item_id = parse_item_id(&depends_on_item_id)?;
            agenda
                .unlink_items_depends_on(item_id, depends_on_item_id)
                .map_err(|e| e.to_string())?;
            println!("unlinked {} depends-on {}", item_id, depends_on_item_id);
            Ok(())
        }
        LinkCommand::UnlinkBlocks {
            blocker_item_id,
            blocked_item_id,
        } => {
            let blocker_item_id = parse_item_id(&blocker_item_id)?;
            let blocked_item_id = parse_item_id(&blocked_item_id)?;
            agenda
                .unlink_items_blocks(blocker_item_id, blocked_item_id)
                .map_err(|e| e.to_string())?;
            println!("unlinked {} blocks {}", blocker_item_id, blocked_item_id);
            Ok(())
        }
        LinkCommand::UnlinkRelated {
            item_a_id,
            item_b_id,
        } => {
            let item_a_id = parse_item_id(&item_a_id)?;
            let item_b_id = parse_item_id(&item_b_id)?;
            agenda
                .unlink_items_related(item_a_id, item_b_id)
                .map_err(|e| e.to_string())?;
            println!("unlinked {} related {}", item_a_id, item_b_id);
            Ok(())
        }
    }
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
        ViewCommand::Show { name } => {
            let categories = store.get_hierarchy().map_err(|e| e.to_string())?;
            let category_names = category_name_map(&categories);
            let items = store.list_items().map_err(|e| e.to_string())?;
            let view = view_by_name(store, &name)?;
            print_items_for_view(&view, &items, &categories, &category_names);
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

fn print_items_for_view(
    view: &View,
    items: &[Item],
    categories: &[Category],
    category_names: &HashMap<CategoryId, String>,
) {
    let reference_date = Local::now().date_naive();
    let result = resolve_view(view, items, categories, reference_date);
    let has_sections = !result.sections.is_empty();

    println!("# {}", view.name);

    for section in result.sections {
        println!("\n## {}", section.title);
        if section.subsections.is_empty() {
            print_item_table(&section.items, category_names);
            continue;
        }

        for subsection in section.subsections {
            println!("\n### {}", subsection.title);
            print_item_table(&subsection.items, category_names);
        }
    }

    if let Some(unmatched) = result.unmatched {
        if !unmatched.is_empty() {
            if !has_sections {
                print_item_table(&unmatched, category_names);
                return;
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
            print_item_table(&unmatched, category_names);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        duplicate_category_create_error, item_link_section_lines, parse_decimal_value,
        parsed_when_feedback_line, unknown_hashtag_feedback_line, Cli, Command, LinkCommand,
    };
    use agenda_core::agenda::Agenda;
    use agenda_core::matcher::SubstringClassifier;
    use agenda_core::model::Item;
    use agenda_core::store::Store;
    use chrono::NaiveDate;
    use clap::Parser;
    use rust_decimal::Decimal;
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
    fn clap_parses_link_unlink_related_subcommand() {
        let cli =
            Cli::try_parse_from(["agenda", "link", "unlink-related", "a", "b"]).expect("parse CLI");

        match cli.command {
            Some(Command::Link {
                command:
                    LinkCommand::UnlinkRelated {
                        item_a_id,
                        item_b_id,
                    },
            }) => {
                assert_eq!(item_a_id, "a");
                assert_eq!(item_b_id, "b");
            }
            other => panic!("unexpected parse result: {other:?}"),
        }
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
}

fn print_item_table(items: &[Item], category_names: &HashMap<CategoryId, String>) {
    if items.is_empty() {
        println!("(no items)");
        return;
    }

    for item in items {
        let when = item
            .when_date
            .map(|dt| dt.to_string())
            .unwrap_or_else(|| "-".to_string());
        let done = if item.is_done { "done" } else { "open" };
        println!("{} | {} | {} | {}", item.id, done, when, item.text);

        if !item.assignments.is_empty() {
            let mut names: Vec<String> = item
                .assignments
                .keys()
                .filter_map(|id| category_names.get(id))
                .cloned()
                .collect();
            names.sort_by_key(|name| name.to_ascii_lowercase());
            println!("  categories: {}", names.join(", "));
        }

        if let Some(note) = &item.note {
            println!("  note: {}", note.replace('\n', " "));
        }
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
