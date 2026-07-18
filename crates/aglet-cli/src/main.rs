use std::cmp::Ordering;
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
use aglet_core::query::{evaluate_query, resolve_view};
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct NumericValueAssignment {
    category_id: CategoryId,
    category_name: String,
    value: Decimal,
}

#[derive(Parser, Debug)]
#[command(name = "aglet")]
#[command(about = "Aglet CLI/TUI")]
#[command(
    after_help = "Run without a command to launch the TUI. Use `aglet list` for the scriptable list view."
)]
struct Cli {
    /// SQLite database path
    #[arg(long, env = "AGLET_DB")]
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
        /// Item title/text.
        text: String,
        /// Optional note/body text stored with the item.
        #[arg(long)]
        note: Option<String>,
        /// Explicit date/time override for the item's `when` value.
        #[arg(long)]
        when: Option<String>,
        /// Category to assign after item creation. Repeat for multiple categories.
        #[arg(long = "category")]
        categories: Vec<String>,
        /// Numeric category assignment in CATEGORY=NUMBER form. Repeat as needed.
        #[arg(long = "value")]
        values: Vec<String>,
    },

    /// Edit an existing item's text, note, and/or done state
    #[command(
        after_help = "Note operations:\n  --note <TEXT>          Replace the entire note\n  --append-note <TEXT>   Append text to the existing note (separated by newline)\n  --note-stdin           Replace the entire note with stdin content\n  --clear-note           Remove the note entirely\n\nExamples:\n  aglet edit <id> --append-note \"Claimed 2026-03-02: branch=feature\"\n  aglet edit <id> --append-note \"Implementation plan:\\n1. Step one\\n2. Step two\"\n  printf \"line one\\nline two\\n\" | aglet edit <id> --note-stdin"
    )]
    Edit {
        /// Item id (full UUID or unique hex prefix).
        item_id: String,
        /// New text (positional argument)
        text: Option<String>,
        /// Replace the entire note. Mutually exclusive with other note flags.
        #[arg(long)]
        note: Option<String>,
        /// Append text to the existing note (separated by newline)
        #[arg(long = "append-note")]
        append_note: Option<String>,
        /// Replace the note with stdin content
        #[arg(long = "note-stdin")]
        note_stdin: bool,
        /// Remove the note entirely. Mutually exclusive with other note flags.
        #[arg(long = "clear-note")]
        clear_note: bool,
        /// Mark item done (`true`) or not done (`false`).
        #[arg(long)]
        done: Option<bool>,
        /// Explicit date/time override for the item's `when` value.
        #[arg(long)]
        when: Option<String>,
        /// Clear the item's explicit `when` value.
        #[arg(long = "clear-when")]
        clear_when: bool,
        /// Set recurrence rule (e.g., "daily", "weekly", "every friday", "monthly on the 15th").
        #[arg(long)]
        recurrence: Option<String>,
        /// Remove the recurrence rule from the item.
        #[arg(long = "clear-recurrence")]
        clear_recurrence: bool,
    },

    /// Show a single item with its assignments
    Show {
        /// Item id (full UUID or unique hex prefix).
        item_id: String,
    },

    /// Atomically claim an eligible item for active work
    Claim {
        /// Item id (full UUID or unique hex prefix).
        item_id: String,
    },

    /// List items that are eligible to be claimed
    Ready {
        /// Sort key(s): item, when, or category name. Repeat for multi-key sorting.
        /// Optional suffix `:asc` or `:desc` (default: asc).
        #[arg(long = "sort", value_name = "COLUMN[:asc|desc]")]
        sort: Vec<String>,
        /// Output format.
        #[arg(long = "format", value_enum, default_value_t = OutputFormatArg::Table)]
        format: OutputFormatArg,
    },

    /// Remove the active claim category from an item
    #[command(visible_alias = "unclaim")]
    Release {
        /// Item id (full UUID or unique hex prefix).
        item_id: String,
    },

    /// List items (optionally filtered)
    #[command(
        after_help = "Default behavior:\n  If `--view` is omitted, `list` uses the \"All Items\" view when present;\n  otherwise falls back to the first stored view.\n\nDependency-state filter examples:\n  aglet list --blocked\n  aglet list --not-blocked --sort Priority\n\nNumeric value filter examples:\n  aglet list --value-eq Complexity 2\n  aglet list --value-in Complexity 1,2\n  aglet list --value-max Complexity 2\n\nSemantics:\n  Dependency state is derived from depends-on links and done state.\n  Numeric value filters are AND-composed with each other and with category filters."
    )]
    List {
        /// View to render. If omitted, defaults to "All Items"; falls back to
        /// the first stored view when "All Items" is unavailable.
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
        /// Only include items blocked by at least one unresolved dependency.
        #[arg(long, conflicts_with = "not_blocked")]
        blocked: bool,
        /// Only include items that are not blocked by unresolved dependencies.
        #[arg(long = "not-blocked", conflicts_with = "blocked")]
        not_blocked: bool,
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
        /// Include done items (default excludes them).
        #[arg(long)]
        include_done: bool,
        /// Restore the full multi-line row output (pre-compact format).
        #[arg(long)]
        verbose: bool,
    },

    /// Search item text and note
    Search {
        /// Text query matched against item text and note.
        query: String,
        /// Output format.
        #[arg(long = "format", value_enum, default_value_t = OutputFormatArg::Table)]
        format: OutputFormatArg,
        /// Only include items blocked by at least one unresolved dependency.
        #[arg(long, conflicts_with = "not_blocked")]
        blocked: bool,
        /// Only include items that are not blocked by unresolved dependencies.
        #[arg(long = "not-blocked", conflicts_with = "blocked")]
        not_blocked: bool,
        /// Include done items in search results (default excludes them).
        #[arg(long)]
        include_done: bool,
        /// Restore the full multi-line row output (pre-compact format).
        #[arg(long)]
        verbose: bool,
    },

    /// Export items as Markdown
    #[command(
        after_help = "Examples:\n  aglet export\n  aglet export --view \"All Items\"\n  aglet export --view \"Backlog\" --include-links"
    )]
    Export {
        /// Optional view scope (case-insensitive view name).
        #[arg(long)]
        view: Option<String>,
        /// Include prereq/dependent/related link details for each item.
        #[arg(long = "include-links")]
        include_links: bool,
    },

    /// Delete an item (writes deletion log)
    Delete {
        /// Item id (full UUID or unique hex prefix).
        item_id: String,
    },

    /// List deletion log entries
    Deleted,

    /// Restore an item from deletion log by log entry id
    Restore {
        /// Deletion log entry id to restore.
        log_id: String,
    },

    /// Launch the interactive TUI
    Tui {
        /// Enable debug logging while running the TUI.
        #[arg(long)]
        debug: bool,
    },

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

    /// Structured import commands
    Import {
        #[command(subcommand)]
        command: ImportCommand,
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

    /// Item commands (alternative noun-verb syntax)
    #[command(subcommand)]
    Item(ItemCommand),
}

/// Noun-verb aliases for item operations: `aglet item add`, `aglet item list`, etc.
#[derive(Subcommand, Debug)]
enum ItemCommand {
    /// Add a new item
    Add {
        /// Item title/text.
        text: String,
        /// Optional note/body text stored with the item.
        #[arg(long)]
        note: Option<String>,
        /// Explicit date/time override for the item's `when` value.
        #[arg(long)]
        when: Option<String>,
        /// Category to assign after item creation. Repeat for multiple categories.
        #[arg(long = "category")]
        categories: Vec<String>,
        /// Numeric category assignment in CATEGORY=NUMBER form. Repeat as needed.
        #[arg(long = "value")]
        values: Vec<String>,
    },

    /// List items (optionally filtered)
    List {
        /// View to render.
        #[arg(long)]
        view: Option<String>,
        /// Category filter (repeat for AND).
        #[arg(long)]
        category: Vec<String>,
        /// OR-category filter (repeat for OR).
        #[arg(long = "any-category")]
        any_category: Vec<String>,
        /// Exclude-category filter (repeat for OR).
        #[arg(long = "exclude-category")]
        exclude_category: Vec<String>,
        /// Only include items blocked by at least one unresolved dependency.
        #[arg(long, conflicts_with = "not_blocked")]
        blocked: bool,
        /// Only include items that are not blocked by unresolved dependencies.
        #[arg(long = "not-blocked", conflicts_with = "blocked")]
        not_blocked: bool,
        /// Numeric equality filter: category value must equal VALUE.
        #[arg(long = "value-eq", value_names = ["CATEGORY", "VALUE"], num_args = 2)]
        value_eq: Vec<String>,
        /// Numeric membership filter: category value must be in CSV_VALUES.
        #[arg(long = "value-in", value_names = ["CATEGORY", "CSV_VALUES"], num_args = 2)]
        value_in: Vec<String>,
        /// Numeric max filter: category value must be <= VALUE.
        #[arg(long = "value-max", value_names = ["CATEGORY", "VALUE"], num_args = 2)]
        value_max: Vec<String>,
        /// Sort key(s): item, when, or category name.
        #[arg(long = "sort", value_name = "COLUMN[:asc|desc]")]
        sort: Vec<String>,
        /// Output format.
        #[arg(long = "format", value_enum, default_value_t = OutputFormatArg::Table)]
        format: OutputFormatArg,
        /// Include done items (default excludes them).
        #[arg(long)]
        include_done: bool,
        /// Restore the full multi-line row output (pre-compact format).
        #[arg(long)]
        verbose: bool,
    },

    /// Show a single item with its assignments
    Show {
        /// Item id (full UUID or unique hex prefix).
        item_id: String,
    },

    /// Edit an existing item's text, note, and/or done state
    Edit {
        /// Item id (full UUID or unique hex prefix).
        item_id: String,
        /// New text (positional argument).
        text: Option<String>,
        /// Replace the entire note.
        #[arg(long)]
        note: Option<String>,
        /// Append text to the existing note (separated by newline).
        #[arg(long = "append-note")]
        append_note: Option<String>,
        /// Replace the note with stdin content.
        #[arg(long = "note-stdin")]
        note_stdin: bool,
        /// Remove the note entirely.
        #[arg(long = "clear-note")]
        clear_note: bool,
        /// Mark item done (`true`) or not done (`false`).
        #[arg(long)]
        done: Option<bool>,
        /// Explicit date/time override for the item's `when` value.
        #[arg(long)]
        when: Option<String>,
        /// Clear the item's explicit `when` value.
        #[arg(long = "clear-when")]
        clear_when: bool,
        /// Set recurrence rule.
        #[arg(long)]
        recurrence: Option<String>,
        /// Remove the recurrence rule from the item.
        #[arg(long = "clear-recurrence")]
        clear_recurrence: bool,
    },

    /// Delete an item (writes deletion log)
    Delete {
        /// Item id (full UUID or unique hex prefix).
        item_id: String,
    },
}

#[derive(Subcommand, Debug)]
enum CategoryCommand {
    /// List categories as a tree
    List,

    /// Show detailed info for a category
    Show {
        /// Category name (case-insensitive).
        name: String,
    },

    /// Create a category
    Create {
        /// New category name.
        name: String,
        /// Parent category name (case-insensitive).
        #[arg(long)]
        parent: Option<String>,
        /// Mark this category as exclusive among siblings.
        #[arg(long)]
        exclusive: bool,
        /// Disable implicit string matching for this category.
        #[arg(long = "disable-implicit-string")]
        disable_implicit_string: bool,
        /// Category value type (`tag` or `numeric`).
        #[arg(long = "type", value_enum)]
        category_type: Option<CategoryTypeArg>,
    },

    /// Delete a category by name
    Delete {
        /// Category name (case-insensitive).
        name: String,
    },

    /// Rename a category
    Rename {
        /// Existing category name (case-insensitive).
        name: String,
        /// New category name.
        new_name: String,
    },

    /// Reparent a category (use --root to make top-level)
    Reparent {
        /// Category name to move.
        name: String,
        /// New parent category name.
        #[arg(long)]
        parent: Option<String>,
        /// Move category to root (top-level).
        #[arg(long)]
        root: bool,
    },

    /// Update category flags
    Update {
        /// Category name (case-insensitive).
        name: String,
        /// Set exclusive mode (`true`/`false`).
        #[arg(long)]
        exclusive: Option<bool>,
        /// Set actionable mode (`true`/`false`).
        #[arg(long)]
        actionable: Option<bool>,
        /// Set implicit string matching (`true`/`false`).
        #[arg(long = "implicit-string")]
        implicit_string: Option<bool>,
        /// Replace note text (empty string clears note).
        #[arg(long)]
        note: Option<String>,
        /// Clear note text.
        #[arg(long = "clear-note")]
        clear_note: bool,
        /// Set category value type (`tag` or `numeric`).
        #[arg(long = "type", value_enum)]
        category_type: Option<CategoryTypeArg>,
    },

    /// Assign an item to a category by id/name
    Assign {
        /// Item id (full UUID or unique hex prefix).
        item_id: String,
        /// Category name (case-insensitive).
        category_name: String,
    },

    /// Set a numeric value assignment for a numeric category
    SetValue {
        /// Item id (full UUID or unique hex prefix).
        item_id: String,
        /// Numeric category name (case-insensitive).
        category_name: String,
        /// Numeric value to assign.
        value: String,
    },

    /// Configure numeric formatting for a numeric category
    Format {
        /// Numeric category name (case-insensitive).
        name: String,
        /// Number of decimal places to render.
        #[arg(long)]
        decimals: Option<u8>,
        /// Currency symbol to render before numeric values.
        #[arg(long)]
        currency: Option<String>,
        /// Clear any configured currency symbol.
        #[arg(long = "clear-currency")]
        clear_currency: bool,
        /// Enable thousands separators.
        #[arg(long, conflicts_with = "no_thousands")]
        thousands: bool,
        /// Disable thousands separators.
        #[arg(long = "no-thousands", conflicts_with = "thousands")]
        no_thousands: bool,
    },

    /// Unassign an item from a category
    Unassign {
        /// Item id (full UUID or unique hex prefix).
        item_id: String,
        /// Category name (case-insensitive).
        category_name: String,
    },

    /// List an item's vetoed (never-auto-assign) categories
    ListVetoes {
        /// Item id (full UUID or unique hex prefix).
        item_id: String,
    },

    /// Clear a veto so the category may be auto-assigned again
    ClearVeto {
        /// Item id (full UUID or unique hex prefix).
        item_id: String,
        /// Category name (case-insensitive).
        category_name: String,
    },

    /// Add a profile condition to a category
    AddCondition {
        /// Category name to add the condition to (case-insensitive).
        name: String,
        /// Categories that must ALL be assigned (AND logic).
        #[arg(long = "and", value_name = "CATEGORY")]
        and_categories: Vec<String>,
        /// Categories that must NOT be assigned.
        #[arg(long = "not", value_name = "CATEGORY")]
        not_categories: Vec<String>,
        /// Categories where at least one must be assigned (OR logic).
        #[arg(long = "or", value_name = "CATEGORY")]
        or_categories: Vec<String>,
    },

    /// Add a date condition to a category
    AddDateCondition {
        /// Category name to add the condition to (case-insensitive).
        name: String,
        /// Which intrinsic item date to evaluate.
        #[arg(long, value_enum)]
        source: DateSourceArg,
        /// Match items whose date falls on the given expression.
        #[arg(long)]
        on: Option<String>,
        /// Match items whose date falls before the given expression.
        #[arg(long)]
        before: Option<String>,
        /// Match items whose date falls after the given expression.
        #[arg(long)]
        after: Option<String>,
        /// Match items whose date falls at or before the given expression.
        #[arg(long = "at-or-before")]
        at_or_before: Option<String>,
        /// Match items whose date falls at or after the given expression.
        #[arg(long = "at-or-after")]
        at_or_after: Option<String>,
        /// Range start expression for an inclusive date range.
        #[arg(long)]
        from: Option<String>,
        /// Range end expression for an inclusive date range; requires `--from`.
        #[arg(long)]
        through: Option<String>,
    },

    /// Add a numeric condition: assign when a numeric category's value is in range
    AddNumericCondition {
        /// Category name to add the condition to (case-insensitive).
        name: String,
        /// Numeric category whose value is tested.
        #[arg(long = "category", value_name = "NUMERIC_CATEGORY")]
        numeric_category: String,
        /// Minimum value (inclusive). Omit for an open lower bound.
        #[arg(long)]
        min: Option<String>,
        /// Maximum value (inclusive). Omit for an open upper bound.
        #[arg(long)]
        max: Option<String>,
        /// Match values OUTSIDE the [min, max] range instead of inside.
        #[arg(long)]
        outside: bool,
    },

    /// Set how a category combines its explicit conditions
    SetConditionMode {
        /// Category name (case-insensitive).
        name: String,
        /// Whether explicit conditions are combined with ANY or ALL semantics.
        mode: ConditionMatchModeArg,
    },

    /// Remove a profile condition from a category by index (1-based)
    RemoveCondition {
        /// Category name (case-insensitive).
        name: String,
        /// Condition index (1-based, as shown in `category show`).
        index: usize,
    },

    /// Add an action to a category
    AddAction {
        /// Category name to add the action to (case-insensitive).
        name: String,
        /// Categories to assign when this category is assigned.
        #[arg(long = "assign", value_name = "CATEGORY")]
        assign_categories: Vec<String>,
        /// Categories to remove when this category is assigned.
        #[arg(long = "remove", value_name = "CATEGORY")]
        remove_categories: Vec<String>,
        /// Numeric category to assign with a value (requires --value).
        #[arg(long = "assign-numeric", value_name = "CATEGORY", requires = "value")]
        assign_numeric: Option<String>,
        /// Value for --assign-numeric (example: 100 or 12.50).
        #[arg(long = "value", value_name = "NUMBER")]
        value: Option<String>,
        /// Stamp the item's When date (today, tomorrow, +N, -N, or YYYY-MM-DD).
        #[arg(long = "set-when", value_name = "DATE_EXPR")]
        set_when: Option<String>,
        /// Mark the item done when assigned here.
        #[arg(long = "mark-done")]
        mark_done: bool,
        /// Delete the item when assigned here (requires allow-delete on the category).
        #[arg(long = "delete")]
        delete: bool,
    },

    /// Enable or disable Delete actions on a category
    SetAllowDelete {
        /// Category name (case-insensitive).
        name: String,
        /// true/on to allow Delete actions, false/off to forbid them.
        enabled: String,
    },

    /// Remove an action from a category by index (1-based)
    RemoveAction {
        /// Category name (case-insensitive).
        name: String,
        /// Action index (1-based, as shown in `category show`).
        index: usize,
    },
}

#[derive(Subcommand, Debug)]
enum ViewCommand {
    /// List views
    List,

    /// Show the contents of a view
    Show {
        /// View name (case-insensitive).
        name: String,
        /// Sort key(s): item, when, or category name. Repeat for multi-key sorting.
        /// Optional suffix `:asc` or `:desc` (default: asc).
        #[arg(long = "sort", value_name = "COLUMN[:asc|desc]")]
        sort: Vec<String>,
        /// Output format.
        #[arg(long = "format", value_enum, default_value_t = OutputFormatArg::Table)]
        format: OutputFormatArg,
        /// Only include items blocked by at least one unresolved dependency.
        #[arg(long, conflicts_with = "not_blocked")]
        blocked: bool,
        /// Only include items that are not blocked by unresolved dependencies.
        #[arg(long = "not-blocked", conflicts_with = "blocked")]
        not_blocked: bool,
    },

    /// Create a basic view from include/exclude categories
    Create {
        /// New view name.
        name: String,
        /// Include-category criterion (repeat for AND semantics).
        #[arg(long = "include")]
        include: Vec<String>,
        /// OR-include criterion (repeat for OR semantics).
        #[arg(long = "or-include")]
        or_include: Vec<String>,
        /// Exclude-category criterion (repeat for NOT semantics).
        #[arg(long = "exclude")]
        exclude: Vec<String>,
        /// Hide items that do not match any section.
        #[arg(long = "hide-unmatched")]
        hide_unmatched: bool,
        /// Hide items blocked by unresolved dependencies.
        #[arg(long = "hide-dependent-items")]
        hide_dependent_items: bool,
    },

    /// Edit mutable view properties
    Edit {
        /// Existing view name.
        name: String,
        /// Set whether unmatched items are shown.
        #[arg(long = "hide-unmatched")]
        hide_unmatched: Option<bool>,
        /// Set whether blocked items are hidden.
        #[arg(long = "hide-dependent-items")]
        hide_dependent_items: Option<bool>,
    },

    /// Clone a view into a new mutable view
    Clone {
        /// Name of the view to clone.
        source_name: String,
        /// Name for the new cloned view.
        new_name: String,
    },

    /// Rename a view
    Rename {
        /// Existing view name (case-insensitive).
        name: String,
        /// New view name.
        new_name: String,
    },

    /// Delete a view by name
    Delete {
        /// View name (case-insensitive).
        name: String,
    },

    /// Set a summary function on a section column
    #[command(name = "set-summary")]
    SetSummary {
        /// View name (case-insensitive).
        name: String,
        /// Section index (0-based).
        section: usize,
        /// Column category name (case-insensitive).
        column: String,
        /// Summary function: none, sum, avg, min, max, count.
        #[arg(value_enum)]
        func: CliSummaryFn,
    },

    /// Section authoring commands
    Section {
        #[command(subcommand)]
        command: ViewSectionCommand,
    },

    /// Column authoring commands
    Column {
        #[command(subcommand)]
        command: ViewColumnCommand,
    },

    /// View alias commands
    Alias {
        #[command(subcommand)]
        command: ViewAliasCommand,
    },

    /// Set or clear the item column label for a view
    #[command(name = "set-item-label")]
    SetItemLabel {
        /// View name (case-insensitive).
        name: String,
        /// New item column label.
        label: Option<String>,
        /// Clear the configured item column label.
        #[arg(long)]
        clear: bool,
    },

    /// Replace the remove-from-view category set
    #[command(name = "set-remove-from-view")]
    SetRemoveFromView {
        /// View name (case-insensitive).
        name: String,
        /// Categories to remove when an item is removed from the view.
        categories: Vec<String>,
        /// Clear the remove-from-view set.
        #[arg(long)]
        clear: bool,
    },

    /// Create a datebook (date-interval) view
    #[command(name = "create-datebook")]
    CreateDatebook {
        /// New view name.
        name: String,
        /// Time window size.
        #[arg(long, value_enum, default_value_t = CliDatebookPeriod::Week)]
        period: CliDatebookPeriod,
        /// Section granularity within the period.
        #[arg(long, value_enum, default_value_t = CliDatebookInterval::Daily)]
        interval: CliDatebookInterval,
        /// Date anchor for the window start.
        #[arg(long, value_enum, default_value_t = CliDatebookAnchor::StartOfWeek)]
        anchor: CliDatebookAnchor,
        /// Which date field to use for item placement.
        #[arg(long, value_enum, default_value_t = DateSourceArg::When)]
        date_source: DateSourceArg,
    },

    /// Shift a datebook view's browse window
    #[command(name = "datebook-browse")]
    DatebookBrowse {
        /// View name (case-insensitive).
        name: String,
        /// Offset to apply: +N forward, -N backward, 0 reset to anchor.
        #[arg(long, default_value_t = 1)]
        offset: i32,
        /// Step granularity: a whole window (default) or one bucket interval.
        #[arg(long, value_enum, default_value_t = BrowseStepArg::Window)]
        step: BrowseStepArg,
    },
}

/// Row rendering for table output: compact one-line rows (default) or the
/// pre-compact multi-line format (`--verbose`) — UX audit P2-CLI-2.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TableStyle {
    Compact,
    Verbose,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum BrowseStepArg {
    /// Shift by the full window length (current behavior).
    Window,
    /// Shift by one bucket interval (one section).
    Bucket,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum DateSourceArg {
    When,
    Entry,
    Done,
}

impl DateSourceArg {
    fn into_model(self) -> DateSource {
        match self {
            Self::When => DateSource::When,
            Self::Entry => DateSource::Entry,
            Self::Done => DateSource::Done,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum ConditionMatchModeArg {
    Any,
    All,
}

impl ConditionMatchModeArg {
    fn into_model(self) -> ConditionMatchMode {
        match self {
            Self::Any => ConditionMatchMode::Any,
            Self::All => ConditionMatchMode::All,
        }
    }
}

#[derive(ValueEnum, Debug, Clone, Copy)]
enum CliSummaryFn {
    None,
    Sum,
    Avg,
    Min,
    Max,
    Count,
}

impl CliSummaryFn {
    fn to_model(self) -> SummaryFn {
        match self {
            Self::None => SummaryFn::None,
            Self::Sum => SummaryFn::Sum,
            Self::Avg => SummaryFn::Avg,
            Self::Min => SummaryFn::Min,
            Self::Max => SummaryFn::Max,
            Self::Count => SummaryFn::Count,
        }
    }
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
enum CliColumnKind {
    Standard,
    When,
}

impl CliColumnKind {
    fn to_model(self) -> ColumnKind {
        match self {
            Self::Standard => ColumnKind::Standard,
            Self::When => ColumnKind::When,
        }
    }
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
enum CliDatebookPeriod {
    Day,
    Week,
    Month,
    Quarter,
    Year,
}

impl CliDatebookPeriod {
    fn into_model(self) -> DatebookPeriod {
        match self {
            Self::Day => DatebookPeriod::Day,
            Self::Week => DatebookPeriod::Week,
            Self::Month => DatebookPeriod::Month,
            Self::Quarter => DatebookPeriod::Quarter,
            Self::Year => DatebookPeriod::Year,
        }
    }
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
enum CliDatebookInterval {
    Hourly,
    Daily,
    Weekly,
    Monthly,
}

impl CliDatebookInterval {
    fn into_model(self) -> DatebookInterval {
        match self {
            Self::Hourly => DatebookInterval::Hourly,
            Self::Daily => DatebookInterval::Daily,
            Self::Weekly => DatebookInterval::Weekly,
            Self::Monthly => DatebookInterval::Monthly,
        }
    }
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
enum CliDatebookAnchor {
    Today,
    #[value(name = "start-of-week")]
    StartOfWeek,
    #[value(name = "start-of-month")]
    StartOfMonth,
    #[value(name = "start-of-quarter")]
    StartOfQuarter,
    #[value(name = "start-of-year")]
    StartOfYear,
}

impl CliDatebookAnchor {
    fn into_model(self) -> DatebookAnchor {
        match self {
            Self::Today => DatebookAnchor::Today,
            Self::StartOfWeek => DatebookAnchor::StartOfWeek,
            Self::StartOfMonth => DatebookAnchor::StartOfMonth,
            Self::StartOfQuarter => DatebookAnchor::StartOfQuarter,
            Self::StartOfYear => DatebookAnchor::StartOfYear,
        }
    }
}

#[derive(Subcommand, Debug)]
enum ViewSectionCommand {
    /// Add a section to a view
    Add {
        /// View name (case-insensitive).
        name: String,
        /// Section title.
        title: String,
        /// Include-category criterion (repeat for AND semantics).
        #[arg(long = "include")]
        include: Vec<String>,
        /// OR-include criterion (repeat for OR semantics).
        #[arg(long = "or-include")]
        or_include: Vec<String>,
        /// Exclude-category criterion (repeat for NOT semantics).
        #[arg(long = "exclude")]
        exclude: Vec<String>,
        /// Show child-generated subsections for this section.
        #[arg(long = "show-children")]
        show_children: bool,
    },

    /// Remove a section from a view
    Remove {
        /// View name (case-insensitive).
        name: String,
        /// Section index (0-based).
        section: usize,
    },

    /// Update a section in a view
    Update {
        /// View name (case-insensitive).
        name: String,
        /// Section index (0-based).
        section: usize,
        /// New section title.
        #[arg(long)]
        title: Option<String>,
        /// Replace include criteria (repeat for AND semantics).
        #[arg(long = "include")]
        include: Vec<String>,
        /// Replace OR criteria (repeat for OR semantics).
        #[arg(long = "or-include")]
        or_include: Vec<String>,
        /// Replace exclude criteria (repeat for NOT semantics).
        #[arg(long = "exclude")]
        exclude: Vec<String>,
        /// Clear all section criteria before applying any provided criteria flags.
        #[arg(long = "clear-criteria")]
        clear_criteria: bool,
        /// Set whether this section should show children.
        #[arg(long = "show-children")]
        show_children: Option<bool>,
    },
}

#[derive(Subcommand, Debug)]
enum ViewColumnCommand {
    /// Add a column to a view section
    Add {
        /// View name (case-insensitive).
        name: String,
        /// Section index (0-based).
        section: usize,
        /// Column heading category name (case-insensitive).
        column: String,
        /// Column kind (`standard` or `when`).
        #[arg(long = "kind", value_enum)]
        kind: Option<CliColumnKind>,
        /// Column width.
        #[arg(long)]
        width: Option<u16>,
        /// Summary function for the column.
        #[arg(long = "summary", value_enum)]
        summary: Option<CliSummaryFn>,
    },

    /// Remove a column from a view section
    Remove {
        /// View name (case-insensitive).
        name: String,
        /// Section index (0-based).
        section: usize,
        /// Column heading category name (case-insensitive).
        column: String,
    },

    /// Update a column in a view section
    Update {
        /// View name (case-insensitive).
        name: String,
        /// Section index (0-based).
        section: usize,
        /// Column heading category name (case-insensitive).
        column: String,
        /// New column kind (`standard` or `when`).
        #[arg(long = "kind", value_enum)]
        kind: Option<CliColumnKind>,
        /// New column width.
        #[arg(long)]
        width: Option<u16>,
        /// New summary function for the column.
        #[arg(long = "summary", value_enum)]
        summary: Option<CliSummaryFn>,
    },
}

#[derive(Subcommand, Debug)]
enum ViewAliasCommand {
    /// Set a display alias for a category inside a view
    Set {
        /// View name (case-insensitive).
        name: String,
        /// Category name (case-insensitive).
        category: String,
        /// Alias text.
        alias: String,
    },

    /// Clear a display alias for a category inside a view
    Clear {
        /// View name (case-insensitive).
        name: String,
        /// Category name (case-insensitive).
        category: String,
    },
}

#[derive(Subcommand, Debug)]
enum ImportCommand {
    /// Import rows from a CSV file
    Csv {
        /// CSV file path.
        path: PathBuf,
        /// Column containing the item title/text.
        #[arg(long = "title-col")]
        title_col: String,
        /// Column containing explicit item date/time values.
        #[arg(long = "date-col")]
        date_col: Option<String>,
        /// Column containing item note text.
        #[arg(long = "note-col")]
        note_col: Option<String>,
        /// Column containing category tokens.
        #[arg(long = "category-col")]
        category_cols: Vec<String>,
        /// Parent category to use for tokens coming from `--category-col`.
        #[arg(long = "category-parent")]
        category_parent: Option<String>,
        /// Separator used to split `--category-col` values.
        #[arg(long = "category-separator", default_value = ",")]
        category_separator: String,
        /// Vendor column mapping in SOURCE=PARENT form.
        #[arg(long = "vendor-col")]
        vendor_cols: Vec<String>,
        /// Numeric value column mapping in SOURCE=CATEGORY form.
        #[arg(long = "value-col")]
        value_cols: Vec<String>,
        /// Categories to assign to every imported row.
        #[arg(long = "assign")]
        assign: Vec<String>,
        /// Print what would be imported without writing changes.
        #[arg(long = "dry-run")]
        dry_run: bool,
    },
}

#[derive(Subcommand, Debug)]
enum LinkCommand {
    /// Create a dependency link: ITEM depends on DEPENDS_ON_ITEM
    #[command(name = "depends-on")]
    DependsOn {
        /// Item id that depends on another item.
        item_id: String,
        /// Item id that is required by `item_id`.
        depends_on_item_id: String,
    },

    /// Create inverse dependency vocabulary: BLOCKER blocks BLOCKED
    Blocks {
        /// Blocking item id.
        blocker_item_id: String,
        /// Blocked item id.
        blocked_item_id: String,
    },

    /// Create a bidirectional related link
    Related {
        /// First item id.
        item_a_id: String,
        /// Second item id.
        item_b_id: String,
    },
}

#[derive(Subcommand, Debug)]
enum UnlinkCommand {
    /// Remove inverse dependency vocabulary: BLOCKER no longer blocks BLOCKED
    Blocks {
        /// Blocking item id.
        blocker_item_id: String,
        /// Blocked item id.
        blocked_item_id: String,
    },

    /// Remove a dependency link: ITEM no longer depends on DEPENDS_ON_ITEM
    #[command(name = "depends-on")]
    DependsOn {
        /// Item id that currently depends on another item.
        item_id: String,
        /// Item id currently depended on by `item_id`.
        depends_on_item_id: String,
    },

    /// Remove a related link
    Related {
        /// First item id.
        item_a_id: String,
        /// Second item id.
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
    if let Some(debug) = tui_launch_debug(&cli.command) {
        return aglet_tui::run_with_options(&db_path, debug).map_err(|e| e.to_string());
    }
    let command = cli.command.expect("non-TUI command should be present");

    let store = Store::open(&db_path).map_err(|e| e.to_string())?;
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

fn cmd_add(
    aglet: &Aglet<'_>,
    text: String,
    note: Option<String>,
    when: Option<String>,
    categories: Vec<String>,
    values: Vec<String>,
) -> Result<(), String> {
    if text.trim().is_empty() {
        return Err("text cannot be empty".to_string());
    }
    let category_names: Vec<String> = aglet
        .store()
        .get_hierarchy()
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|category| category.name)
        .collect();
    let unknown_hashtags = unknown_hashtag_tokens(&text, &category_names);
    let categories_hierarchy = aglet.store().get_hierarchy().map_err(|e| e.to_string())?;
    let tag_assignments = resolve_tag_category_assignments(&categories_hierarchy, &categories)?;
    let value_assignments = resolve_value_assignments(&categories_hierarchy, &values)?;
    let parsed_when = when.as_deref().map(parse_when_datetime_input).transpose()?;

    let mut item = Item::new(text);
    item.note = note;

    let reference_date = jiff::Zoned::now().date();
    let result = aglet
        .create_item_with_reference_date(&item, reference_date)
        .map_err(|e| e.to_string())?;
    if let Some(explicit_when) = parsed_when {
        aglet
            .set_item_when_date(
                item.id,
                Some(explicit_when),
                Some("manual:cli.when".to_string()),
            )
            .map_err(|e| e.to_string())?;
    }
    for (category_id, category_name) in tag_assignments {
        apply_tag_category_assignment(aglet, item.id, category_id, &category_name)?;
    }
    for assignment in value_assignments {
        apply_numeric_value_assignment(aglet, item.id, assignment)?;
    }
    let created = aglet.store().get_item(item.id).map_err(|e| e.to_string())?;

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

fn temporal_reevaluate_before_command(aglet: &Aglet<'_>) -> Result<(), String> {
    if aglet.has_date_conditions().map_err(|e| e.to_string())? {
        let _ = aglet
            .reevaluate_temporal_conditions()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn build_date_matcher_from_args(
    on: Option<String>,
    before: Option<String>,
    after: Option<String>,
    at_or_before: Option<String>,
    at_or_after: Option<String>,
    from: Option<String>,
    through: Option<String>,
) -> Result<DateMatcher, String> {
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
                    .to_string(),
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
                .to_string(),
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

fn read_note_from_stdin(reader: &mut impl Read) -> Result<String, String> {
    let mut input = String::new();
    reader
        .read_to_string(&mut input)
        .map_err(|e| format!("failed to read --note-stdin input: {e}"))?;
    Ok(input)
}

#[allow(clippy::too_many_arguments)]
fn cmd_edit(
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
) -> Result<(), String> {
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
            "nothing to update\n\nUsage: aglet edit <ITEM_ID> [TEXT] [--note <NOTE>] [--append-note <TEXT>] [--note-stdin] [--clear-note] [--done <true|false>] [--when <DATE>] [--clear-when] [--recurrence <RULE>] [--clear-recurrence]\n\nExamples:\n  aglet edit <id> \"new text here\"\n  aglet edit <id> --note \"updated note\"\n  aglet edit <id> --append-note \"extra info\"\n  printf \"line one\\nline two\\n\" | aglet edit <id> --note-stdin\n  aglet edit <id> \"new text\" --note \"and note\"\n  aglet edit <id> --clear-note\n  aglet edit <id> --done true\n  aglet edit <id> --done false\n  aglet edit <id> --when 2026-02-20\n  aglet edit <id> --clear-when\n  aglet edit <id> --recurrence \"every friday\"\n  aglet edit <id> --clear-recurrence".to_string()
        );
    }

    if when.is_some() && clear_when {
        return Err("--when and --clear-when are mutually exclusive".to_string());
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
            let result = aglet.mark_item_done(item_id).map_err(|e| e.to_string())?;
            if let Some(successor_id) = result.successor_item_id {
                let successor = aglet
                    .store()
                    .get_item(successor_id)
                    .map_err(|e| e.to_string())?;
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
            aglet
                .mark_item_not_done(item_id)
                .map_err(|e| e.to_string())?;
            println!("marked not-done {}", item_id);
        }
    }

    let mut item = aglet.store().get_item(item_id).map_err(|e| e.to_string())?;

    let note_stdin_has_content = note_stdin.as_ref().is_some_and(|value| !value.is_empty());
    if text.is_some()
        || note.is_some()
        || append_note.is_some()
        || note_stdin_has_content
        || clear_note
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

        item.modified_at = jiff::Timestamp::now();
        let reference_date = jiff::Zoned::now().date();
        aglet
            .update_item_with_reference_date(&item, reference_date)
            .map_err(|e| e.to_string())?;

        let updated = aglet.store().get_item(item_id).map_err(|e| e.to_string())?;
        println!("updated {}", item_id);
        if let Some(line) = parsed_when_feedback_line(updated.when_date) {
            println!("{line}");
        }
    }

    if let Some(explicit_when) = when {
        let parsed = parse_when_datetime_input(&explicit_when)?;
        aglet
            .set_item_when_date(item_id, Some(parsed), Some("manual:cli.when".to_string()))
            .map_err(|e| e.to_string())?;
        let updated = aglet.store().get_item(item_id).map_err(|e| e.to_string())?;
        println!("updated {}", item_id);
        if let Some(line) = parsed_when_feedback_line(updated.when_date) {
            println!("{line}");
        }
    } else if clear_when {
        aglet
            .set_item_when_date(item_id, None, Some("manual:cli.when-clear".to_string()))
            .map_err(|e| e.to_string())?;
        println!("updated {}", item_id);
    }

    if recurrence.is_some() && clear_recurrence {
        return Err("--recurrence and --clear-recurrence are mutually exclusive".to_string());
    }
    if let Some(recurrence_text) = recurrence {
        let parser = aglet_core::dates::BasicDateParser::default();
        let reference = jiff::Zoned::now().date();
        match parser.parse_with_recurrence(&recurrence_text, reference) {
            Some(aglet_core::dates::DateParseResult::Recurring { rule, .. }) => {
                let mut item = aglet.store().get_item(item_id).map_err(|e| e.to_string())?;
                item.recurrence_rule = Some(rule.clone());
                item.modified_at = jiff::Timestamp::now();
                aglet
                    .store()
                    .update_item(&item)
                    .map_err(|e| e.to_string())?;
                println!("set recurrence: {}", rule.display());
            }
            _ => {
                return Err(format!(
                    "unrecognized recurrence pattern: \"{}\"\n\nExamples: daily, weekly, every friday, every 2 weeks, monthly on the 15th",
                    recurrence_text
                ));
            }
        }
    } else if clear_recurrence {
        let mut item = aglet.store().get_item(item_id).map_err(|e| e.to_string())?;
        item.recurrence_rule = None;
        item.modified_at = jiff::Timestamp::now();
        aglet
            .store()
            .update_item(&item)
            .map_err(|e| e.to_string())?;
        println!("cleared recurrence");
    }

    Ok(())
}

fn cmd_show(store: &Store, item_id_str: String) -> Result<(), String> {
    let item_id = resolve_item_id(&item_id_str, store)?;
    let item = store.get_item(item_id).map_err(|e| e.to_string())?;
    let categories = store.get_hierarchy().map_err(|e| e.to_string())?;
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

fn resolved_workflow_or_err(
    store: &Store,
) -> Result<aglet_core::workflow::ResolvedWorkflowConfig, String> {
    resolve_workflow_config(store)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| workflow_setup_error_message().to_string())
}

type ReadyQueueData = (
    View,
    Vec<Item>,
    Vec<Category>,
    HashMap<CategoryId, String>,
    HashSet<ItemId>,
);

fn ready_queue_data(store: &Store) -> Result<ReadyQueueData, String> {
    let workflow = resolved_workflow_or_err(store)?;
    let view = build_ready_queue_view(store, workflow).map_err(|e| e.to_string())?;
    let categories = store.get_hierarchy().map_err(|e| e.to_string())?;
    let category_names = category_name_map(&categories);
    let all_items = store.list_items().map_err(|e| e.to_string())?;
    let claimable_ids =
        claimable_item_ids(store, &all_items, workflow).map_err(|e| e.to_string())?;
    let items = all_items
        .into_iter()
        .filter(|item| claimable_ids.contains(&item.id))
        .collect();
    Ok((view, items, categories, category_names, HashSet::new()))
}

fn cmd_claim(aglet: &Aglet<'_>, store: &Store, item_id_str: String) -> Result<(), String> {
    let item_id = resolve_item_id(&item_id_str, store)?;
    let workflow = resolved_workflow_or_err(store)?;
    let claim_category = store
        .get_category(workflow.claim_category_id)
        .map_err(|e| e.to_string())?;
    let result = aglet
        .claim_item_workflow(item_id)
        .map_err(|e| e.to_string())?;
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

fn cmd_release(aglet: &Aglet<'_>, store: &Store, item_id_str: String) -> Result<(), String> {
    let item_id = resolve_item_id(&item_id_str, store)?;
    let workflow = resolved_workflow_or_err(store)?;
    let claim_category = store
        .get_category(workflow.claim_category_id)
        .map_err(|e| e.to_string())?;
    let result = aglet
        .release_item_claim(item_id)
        .map_err(|e| e.to_string())?;
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

fn cmd_ready(
    store: &Store,
    sort_args: Vec<String>,
    output_format: OutputFormatArg,
) -> Result<(), String> {
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

fn parsed_when_feedback_line(when_date: Option<DateTime>) -> Option<String> {
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
    dependency_state_filter: Option<DependencyStateFilter>,
    value_eq: Vec<String>,
    value_in: Vec<String>,
    value_max: Vec<String>,
    include_done: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DependencyStateFilter {
    Blocked,
    NotBlocked,
}

fn dependency_state_filter_from_flags(
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

fn table_style_from_verbose(verbose: bool) -> TableStyle {
    if verbose {
        TableStyle::Verbose
    } else {
        TableStyle::Compact
    }
}

fn cmd_list(
    store: &Store,
    view_name: Option<String>,
    filters: ListFilters,
    sort_args: Vec<String>,
    output_format: OutputFormatArg,
    table_style: TableStyle,
) -> Result<(), String> {
    let all_items = store.list_items().map_err(|e| e.to_string())?;
    let blocked_item_ids = blocked_item_ids(store, &all_items).map_err(|e| e.to_string())?;
    let mut items = all_items.clone();
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
    if let Some(filter) = filters.dependency_state_filter {
        retain_items_by_dependency_state(
            store,
            &mut items,
            filter == DependencyStateFilter::Blocked,
        )
        .map_err(|e| e.to_string())?;
    }

    let resolved_view = if let Some(view_name) = view_name {
        Some(view_by_name(store, &view_name)?)
    } else {
        let views = store.list_views().map_err(|e| e.to_string())?;
        views
            .iter()
            .find(|v| v.name.eq_ignore_ascii_case(DEFAULT_VIEW_NAME))
            .cloned()
            .or_else(|| views.into_iter().next())
    };

    if let Some(ref view) = resolved_view {
        if view.name.eq_ignore_ascii_case(READY_QUEUE_VIEW_NAME) {
            if let Some(workflow) = resolve_workflow_config(store).map_err(|e| e.to_string())? {
                let claimable =
                    claimable_item_ids(store, &items, workflow).map_err(|e| e.to_string())?;
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
    dependency_state_filter: Option<DependencyStateFilter>,
    include_done: bool,
    table_style: TableStyle,
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
    let reference_date = jiff::Zoned::now().date();
    let matches = evaluate_query(&q, &items, reference_date);

    let mut matched_items: Vec<Item> = matches.into_iter().cloned().collect();
    if let Some(filter) = dependency_state_filter {
        retain_items_by_dependency_state(
            store,
            &mut matched_items,
            filter == DependencyStateFilter::Blocked,
        )
        .map_err(|e| e.to_string())?;
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

fn cmd_export(store: &Store, view_name: Option<String>, include_links: bool) -> Result<(), String> {
    let body = build_markdown_export(store, view_name.as_deref(), include_links)?;
    write_stdout_allow_broken_pipe(&body)?;
    Ok(())
}

fn write_stdout_allow_broken_pipe(body: &str) -> Result<(), String> {
    let mut stdout = io::stdout().lock();
    write_output_allow_broken_pipe(&mut stdout, body)
}

fn write_output_allow_broken_pipe<W: Write>(writer: &mut W, body: &str) -> Result<(), String> {
    match writer
        .write_all(body.as_bytes())
        .and_then(|_| writer.flush())
    {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        Err(err) => Err(format!("failed writing to stdout: {err}")),
    }
}

fn cmd_delete(aglet: &Aglet<'_>, item_id_str: String) -> Result<(), String> {
    let item_id = resolve_item_id(&item_id_str, aglet.store())?;
    aglet
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
            entry.id, entry.item_id, entry.deleted_at, entry.deleted_by, entry.text
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

fn cmd_link(aglet: &Aglet<'_>, command: LinkCommand) -> Result<(), String> {
    match command {
        LinkCommand::DependsOn {
            item_id,
            depends_on_item_id,
        } => {
            let item_id = resolve_item_id(&item_id, aglet.store())?;
            let depends_on_item_id = resolve_item_id(&depends_on_item_id, aglet.store())?;
            let result = aglet
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
            let blocker_item_id = resolve_item_id(&blocker_item_id, aglet.store())?;
            let blocked_item_id = resolve_item_id(&blocked_item_id, aglet.store())?;
            let result = aglet
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
            let item_a_id = resolve_item_id(&item_a_id, aglet.store())?;
            let item_b_id = resolve_item_id(&item_b_id, aglet.store())?;
            let result = aglet
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

fn cmd_unlink(aglet: &Aglet<'_>, command: UnlinkCommand) -> Result<(), String> {
    match command {
        UnlinkCommand::Blocks {
            blocker_item_id,
            blocked_item_id,
        } => unlink_blocks(aglet, blocker_item_id, blocked_item_id),
        UnlinkCommand::DependsOn {
            item_id,
            depends_on_item_id,
        } => unlink_depends_on(aglet, item_id, depends_on_item_id),
        UnlinkCommand::Related {
            item_a_id,
            item_b_id,
        } => unlink_related(aglet, item_a_id, item_b_id),
    }
}

fn unlink_depends_on(
    aglet: &Aglet<'_>,
    item_id: String,
    depends_on_item_id: String,
) -> Result<(), String> {
    let item_id = resolve_item_id(&item_id, aglet.store())?;
    let depends_on_item_id = resolve_item_id(&depends_on_item_id, aglet.store())?;
    aglet
        .unlink_items_depends_on(item_id, depends_on_item_id)
        .map_err(|e| e.to_string())?;
    println!("unlinked {} depends-on {}", item_id, depends_on_item_id);
    Ok(())
}

fn unlink_blocks(
    aglet: &Aglet<'_>,
    blocker_item_id: String,
    blocked_item_id: String,
) -> Result<(), String> {
    let blocker_item_id = resolve_item_id(&blocker_item_id, aglet.store())?;
    let blocked_item_id = resolve_item_id(&blocked_item_id, aglet.store())?;
    aglet
        .unlink_items_blocks(blocker_item_id, blocked_item_id)
        .map_err(|e| e.to_string())?;
    println!("unlinked {} blocks {}", blocker_item_id, blocked_item_id);
    Ok(())
}

fn unlink_related(aglet: &Aglet<'_>, item_a_id: String, item_b_id: String) -> Result<(), String> {
    let item_a_id = resolve_item_id(&item_a_id, aglet.store())?;
    let item_b_id = resolve_item_id(&item_b_id, aglet.store())?;
    aglet
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

fn cmd_category(aglet: &Aglet<'_>, store: &Store, command: CategoryCommand) -> Result<(), String> {
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
            print_bulk_warnings(&result);
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
            let result = aglet
                .update_category(&category)
                .map_err(|e| e.to_string())?;
            println!(
                "renamed {} -> {} (processed_items={}, affected_items={})",
                name, new_name, result.processed_items, result.affected_items
            );
            print_bulk_warnings(&result);
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
            let result = aglet
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
            let result = aglet
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
            print_bulk_warnings(&result);
            Ok(())
        }
        CategoryCommand::Assign {
            item_id,
            category_name,
        } => {
            let item_id = resolve_item_id(&item_id, store)?;
            let categories = store.get_hierarchy().map_err(|e| e.to_string())?;
            let category_id = category_id_by_name(&categories, &category_name)?;
            let category = categories
                .iter()
                .find(|c| c.id == category_id)
                .ok_or_else(|| format!("category not found: {category_name}"))?;

            if category_name.eq_ignore_ascii_case("Done") {
                aglet.mark_item_done(item_id).map_err(|e| e.to_string())?;
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
                ));
            }

            let result = aglet
                .assign_item_manual(item_id, category_id, Some("manual:cli.assign".to_string()))
                .map_err(|e| e.to_string())?;
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
            let categories = store.get_hierarchy().map_err(|e| e.to_string())?;
            let category_id = category_id_by_name(&categories, &category_name)?;
            let numeric_value = parse_decimal_value(&value)?;
            let result = aglet
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
                        .to_string(),
                );
            }
            if currency.is_some() && clear_currency {
                return Err("--currency and --clear-currency are mutually exclusive".to_string());
            }

            let categories = store.get_hierarchy().map_err(|e| e.to_string())?;
            let category_id = category_id_by_name(&categories, &name)?;
            let mut category = store.get_category(category_id).map_err(|e| e.to_string())?;
            if category.value_kind != CategoryValueKind::Numeric {
                return Err(format!(
                    "category '{}' is not Numeric; numeric formatting only applies to Numeric categories",
                    category.name
                ));
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
            let result = aglet
                .update_category(&category)
                .map_err(|e| e.to_string())?;
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
            let categories = store.get_hierarchy().map_err(|e| e.to_string())?;
            let category_id = category_id_by_name(&categories, &category_name)?;

            if category_name.eq_ignore_ascii_case("Done") {
                let item = store.get_item(item_id).map_err(|e| e.to_string())?;
                if item.is_done {
                    aglet.toggle_item_done(item_id).map_err(|e| e.to_string())?;
                    println!(
                        "unassigned item {} from category Done (marked not-done)",
                        item_id
                    );
                    return Ok(());
                }
            }

            aglet
                .unassign_item_manual(item_id, category_id)
                .map_err(|e| e.to_string())?;
            let vetoed = store
                .get_vetoes_for_item(item_id)
                .map_err(|e| e.to_string())?
                .contains(&category_id);
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
            let vetoes = store.get_vetoes_for_item(item_id).map_err(|e| e.to_string())?;
            if vetoes.is_empty() {
                println!("no vetoes for item {item_id}");
                return Ok(());
            }
            let categories = store.get_hierarchy().map_err(|e| e.to_string())?;
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
            let categories = store.get_hierarchy().map_err(|e| e.to_string())?;
            let category_id = category_id_by_name(&categories, &category_name)?;
            store
                .remove_assignment_veto(item_id, category_id)
                .map_err(|e| e.to_string())?;
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
                return Err(
                    "at least one criterion required: use --and, --not, or --or".to_string()
                );
            }
            let categories = store.get_hierarchy().map_err(|e| e.to_string())?;
            let category_id = category_id_by_name(&categories, &name)?;
            let mut category = store.get_category(category_id).map_err(|e| e.to_string())?;

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
            let result = aglet
                .update_category(&category)
                .map_err(|e| e.to_string())?;

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
            let categories = store.get_hierarchy().map_err(|e| e.to_string())?;
            let category_id = category_id_by_name(&categories, &name)?;
            let mut category = store.get_category(category_id).map_err(|e| e.to_string())?;

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
            let result = aglet
                .update_category(&category)
                .map_err(|e| e.to_string())?;

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
            let categories = store.get_hierarchy().map_err(|e| e.to_string())?;
            let category_id = category_id_by_name(&categories, &name)?;
            let numeric_category_id = category_id_by_name(&categories, &numeric_category)?;
            let target = store
                .get_category(numeric_category_id)
                .map_err(|e| e.to_string())?;
            if target.value_kind != aglet_core::model::CategoryValueKind::Numeric {
                return Err(format!("category '{}' is not Numeric", target.name));
            }

            let parse_bound = |label: &str, raw: &Option<String>| -> Result<Option<Decimal>, String> {
                raw.as_deref()
                    .map(|value| {
                        value
                            .parse::<Decimal>()
                            .map_err(|_| format!("invalid {label} value: {value}"))
                    })
                    .transpose()
            };
            let min = parse_bound("--min", &min)?;
            let max = parse_bound("--max", &max)?;
            if let (Some(low), Some(high)) = (min, max) {
                if low > high {
                    return Err(format!("--min {low} is greater than --max {high}"));
                }
            }
            if outside && min.is_none() && max.is_none() {
                return Err("--outside requires --min and/or --max".to_string());
            }

            let mut category = store.get_category(category_id).map_err(|e| e.to_string())?;
            let condition = Condition::Numeric {
                category_id: numeric_category_id,
                min,
                max,
                outside,
            };
            category.conditions.push(condition);
            let result = aglet
                .update_category(&category)
                .map_err(|e| e.to_string())?;

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
            let categories = store.get_hierarchy().map_err(|e| e.to_string())?;
            let category_id = category_id_by_name(&categories, &name)?;
            let mut category = store.get_category(category_id).map_err(|e| e.to_string())?;
            category.condition_match_mode = mode.into_model();
            let result = aglet
                .update_category(&category)
                .map_err(|e| e.to_string())?;
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
            let categories = store.get_hierarchy().map_err(|e| e.to_string())?;
            let category_id = category_id_by_name(&categories, &name)?;
            let mut category = store.get_category(category_id).map_err(|e| e.to_string())?;

            if index == 0 || index > category.conditions.len() {
                return Err(format!(
                    "condition index {} out of range: {} has {} condition(s)",
                    index,
                    name,
                    category.conditions.len()
                ));
            }
            let removed = category.conditions.remove(index - 1);
            let result = aglet
                .update_category(&category)
                .map_err(|e| e.to_string())?;

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
                        .to_string(),
                );
            }

            let categories = store.get_hierarchy().map_err(|e| e.to_string())?;
            let category_id = category_id_by_name(&categories, &name)?;

            let (action, action_kind) = if !assign_categories.is_empty()
                || !remove_categories.is_empty()
            {
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

            let (action_index, result) = aglet
                .add_category_action(category_id, action)
                .map_err(|e| e.to_string())?;
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
                other => return Err(format!("expected true/false, got '{other}'")),
            };
            let categories = store.get_hierarchy().map_err(|e| e.to_string())?;
            let category_id = category_id_by_name(&categories, &name)?;
            let mut category = store.get_category(category_id).map_err(|e| e.to_string())?;
            category.allow_delete_action = enabled;
            aglet.update_category(&category).map_err(|e| e.to_string())?;
            println!(
                "allow_delete_action on {} is now {}",
                name,
                if enabled { "enabled" } else { "disabled" }
            );
            Ok(())
        }
        CategoryCommand::RemoveAction { name, index } => {
            let categories = store.get_hierarchy().map_err(|e| e.to_string())?;
            let category_id = category_id_by_name(&categories, &name)?;
            let category = store.get_category(category_id).map_err(|e| e.to_string())?;

            if index == 0 || index > category.actions.len() {
                return Err(format!(
                    "action index {} out of range: {} has {} action(s)",
                    index,
                    name,
                    category.actions.len()
                ));
            }
            let (removed, result) = aglet
                .remove_category_action(category_id, index - 1)
                .map_err(|e| e.to_string())?;
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

fn cmd_view(aglet: &Aglet<'_>, store: &Store, command: ViewCommand) -> Result<(), String> {
    let _ = aglet;
    match command {
        ViewCommand::List => {
            let mut views = store.list_views().map_err(|e| e.to_string())?;
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
                        "Ready Queue only shows claimable items; --blocked is not supported"
                            .to_string(),
                    );
                }
                if not_blocked {
                    return Err(
                        "Ready Queue already excludes blocked items; --not-blocked is redundant"
                            .to_string(),
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
                let categories = store.get_hierarchy().map_err(|e| e.to_string())?;
                let category_names = category_name_map(&categories);
                let all_items = store.list_items().map_err(|e| e.to_string())?;
                let blocked_item_ids =
                    blocked_item_ids(store, &all_items).map_err(|e| e.to_string())?;
                let mut items = all_items;
                if let Some(filter) = dependency_state_filter_from_flags(blocked, not_blocked) {
                    retain_items_by_dependency_state(
                        store,
                        &mut items,
                        filter == DependencyStateFilter::Blocked,
                    )
                    .map_err(|e| e.to_string())?;
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
            let categories = store.get_hierarchy().map_err(|e| e.to_string())?;
            let mut view = View::new(name);
            view.show_unmatched = !hide_unmatched;
            view.hide_dependent_items = hide_dependent_items;
            view.criteria =
                query_from_category_names(&categories, &include, &or_include, &exclude)?;

            store.create_view(&view).map_err(|e| e.to_string())?;
            println!("created view {}", view.name);
            Ok(())
        }
        ViewCommand::Edit {
            name,
            hide_unmatched,
            hide_dependent_items,
        } => {
            if name.eq_ignore_ascii_case(READY_QUEUE_VIEW_NAME) {
                return Err(format!(
                    "cannot modify system view: {READY_QUEUE_VIEW_NAME}"
                ));
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
                        .to_string(),
                );
            }
            store.update_view(&view).map_err(|e| e.to_string())?;
            println!("updated view {}", view.name);
            Ok(())
        }
        ViewCommand::Clone {
            source_name,
            new_name,
        } => {
            if source_name.eq_ignore_ascii_case(READY_QUEUE_VIEW_NAME) {
                return Err(format!(
                    "cannot modify system view: {READY_QUEUE_VIEW_NAME}"
                ));
            }
            let source = view_by_name(store, &source_name)?;
            let cloned = store
                .clone_view(source.id, new_name)
                .map_err(|e| e.to_string())?;
            println!("cloned view {} -> {}", source_name, cloned.name);
            Ok(())
        }
        ViewCommand::Rename { name, new_name } => {
            if name.eq_ignore_ascii_case(READY_QUEUE_VIEW_NAME) {
                return Err(format!(
                    "cannot modify system view: {READY_QUEUE_VIEW_NAME}"
                ));
            }
            let mut view = view_by_name(store, &name)?;
            view.name = new_name.clone();
            store.update_view(&view).map_err(|e| e.to_string())?;
            println!("renamed view {} -> {}", name, new_name);
            Ok(())
        }
        ViewCommand::Delete { name } => {
            if name.eq_ignore_ascii_case(READY_QUEUE_VIEW_NAME) {
                return Err(format!(
                    "cannot modify system view: {READY_QUEUE_VIEW_NAME}"
                ));
            }
            let view = view_by_name(store, &name)?;
            store.delete_view(view.id).map_err(|e| e.to_string())?;
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
                ));
            }
            let col_lower = column.to_lowercase();
            let category_names: HashMap<CategoryId, String> = store
                .get_hierarchy()
                .map_err(|e| e.to_string())?
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
            store.update_view(&view).map_err(|e| e.to_string())?;
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
                let categories = store.get_hierarchy().map_err(|e| e.to_string())?;
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
                store.update_view(&view).map_err(|e| e.to_string())?;
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
                    ));
                }
                let removed = view.sections.remove(section);
                store.update_view(&view).map_err(|e| e.to_string())?;
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
                let categories = store.get_hierarchy().map_err(|e| e.to_string())?;
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
                store.update_view(&view).map_err(|e| e.to_string())?;
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
                let categories = store.get_hierarchy().map_err(|e| e.to_string())?;
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
                store.update_view(&view).map_err(|e| e.to_string())?;
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
                let categories = store.get_hierarchy().map_err(|e| e.to_string())?;
                let category_names = category_name_map(&categories);
                let mut view = view_by_name(store, &name)?;
                ensure_mutable_view(&view)?;
                let section_ref = section_mut(&mut view, section)?;
                let column_index = find_column_index(section_ref, &category_names, &column)?;
                section_ref.columns.remove(column_index);
                store.update_view(&view).map_err(|e| e.to_string())?;
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
                let categories = store.get_hierarchy().map_err(|e| e.to_string())?;
                let category_names = category_name_map(&categories);
                if kind.is_none() && width.is_none() && summary.is_none() {
                    return Err(
                        "nothing to update: specify --kind, --width, and/or --summary".to_string(),
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
                store.update_view(&view).map_err(|e| e.to_string())?;
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
                let categories = store.get_hierarchy().map_err(|e| e.to_string())?;
                let category_id = category_id_by_name(&categories, &category)?;
                let mut view = view_by_name(store, &name)?;
                ensure_mutable_view(&view)?;
                view.category_aliases.insert(category_id, alias.clone());
                store.update_view(&view).map_err(|e| e.to_string())?;
                println!(
                    "set alias for {} in view {} to {}",
                    category, view.name, alias
                );
                Ok(())
            }
            ViewAliasCommand::Clear { name, category } => {
                let categories = store.get_hierarchy().map_err(|e| e.to_string())?;
                let category_id = category_id_by_name(&categories, &category)?;
                let mut view = view_by_name(store, &name)?;
                ensure_mutable_view(&view)?;
                view.category_aliases.remove(&category_id);
                store.update_view(&view).map_err(|e| e.to_string())?;
                println!("cleared alias for {} in view {}", category, view.name);
                Ok(())
            }
        },
        ViewCommand::SetItemLabel { name, label, clear } => {
            if clear && label.is_some() {
                return Err("--clear and <label> are mutually exclusive".to_string());
            }
            if !clear && label.is_none() {
                return Err("provide a label or pass --clear".to_string());
            }
            let mut view = view_by_name(store, &name)?;
            ensure_mutable_view(&view)?;
            view.item_column_label = if clear { None } else { label };
            store.update_view(&view).map_err(|e| e.to_string())?;
            println!("updated item column label for view {}", view.name);
            Ok(())
        }
        ViewCommand::SetRemoveFromView {
            name,
            categories,
            clear,
        } => {
            if clear && !categories.is_empty() {
                return Err("--clear cannot be combined with category names".to_string());
            }
            if !clear && categories.is_empty() {
                return Err("provide one or more categories or pass --clear".to_string());
            }
            let hierarchy = store.get_hierarchy().map_err(|e| e.to_string())?;
            let mut view = view_by_name(store, &name)?;
            ensure_mutable_view(&view)?;
            view.remove_from_view_unassign = if clear {
                HashSet::new()
            } else {
                names_to_category_ids(&hierarchy, &categories)?
            };
            store.update_view(&view).map_err(|e| e.to_string())?;
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
                ));
            }
            let mut view = View::new(name);
            view.datebook_config = Some(config);
            store.create_view(&view).map_err(|e| e.to_string())?;
            println!("created datebook view \"{}\"", view.name);
            Ok(())
        }

        ViewCommand::DatebookBrowse { name, offset, step } => {
            let mut view = view_by_name(store, &name)?;
            ensure_mutable_view(&view)?;
            if view.datebook_config.is_none() {
                return Err(format!("\"{}\" is not a datebook view", view.name));
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
            store.update_view(&view).map_err(|e| e.to_string())?;
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

fn cmd_import(aglet: &Aglet<'_>, store: &Store, command: ImportCommand) -> Result<(), String> {
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
            let global_assignments = resolve_tag_category_assignments(
                &store.get_hierarchy().map_err(|e| e.to_string())?,
                &assign,
            )?;
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
            let headers = reader.headers().map_err(|e| e.to_string())?.clone();
            let mut imported = 0usize;

            for record in reader.records() {
                let record = record.map_err(|e| e.to_string())?;
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
                    imported += 1;
                    continue;
                }

                let mut item = Item::new(title);
                item.note = note;
                aglet
                    .create_item_with_reference_date(&item, jiff::Zoned::now().date())
                    .map_err(|e| e.to_string())?;
                if let Some(when_date) = parsed_when {
                    aglet
                        .set_item_when_date(
                            item.id,
                            Some(when_date),
                            Some("manual:cli.when".to_string()),
                        )
                        .map_err(|e| e.to_string())?;
                }
                for (category_id, category_name) in row_tag_assignments {
                    apply_tag_category_assignment(aglet, item.id, category_id, &category_name)?;
                }
                for assignment in row_value_assignments {
                    apply_numeric_value_assignment(aglet, item.id, assignment)?;
                }
                imported += 1;
            }

            if dry_run {
                println!("dry-run imported_rows={imported}");
            } else {
                println!("imported_rows={imported}");
            }
            Ok(())
        }
    }
}

fn resolve_db_path(db_opt: Option<PathBuf>) -> Result<PathBuf, String> {
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

fn ensure_db_parent(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn resolve_item_id(input: &str, store: &Store) -> Result<ItemId, String> {
    // Try full UUID parse first
    if let Ok(id) = ItemId::parse_str(input) {
        return Ok(id);
    }
    // Fall back to prefix resolution
    store.resolve_item_prefix(input).map_err(|e| e.to_string())
}

fn category_name_map(categories: &[Category]) -> HashMap<CategoryId, String> {
    categories
        .iter()
        .map(|category| (category.id, category.name.clone()))
        .collect()
}

fn print_process_warnings(result: &aglet_core::engine::ProcessItemResult) {
    for warning in &result.warnings {
        eprintln!("warning: {warning}");
    }
}

fn print_bulk_warnings(result: &aglet_core::engine::EvaluateAllItemsResult) {
    for warning in &result.warnings {
        eprintln!("warning: {warning}");
    }
}

fn describe_category_targets(
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

fn describe_category_action(
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

fn indexed_category_action_row(
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
    if name.eq_ignore_ascii_case(READY_QUEUE_VIEW_NAME) {
        let workflow = resolved_workflow_or_err(store)?;
        return build_ready_queue_view(store, workflow).map_err(|e| e.to_string());
    }
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
        "category \"{requested_name}\" already exists{id_fragment}. Category names are global across the database, so it cannot be created{parent_context}. Use `aglet category assign <item-id> \"{requested_name}\"` to assign items to the existing category."
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

fn parse_when_datetime_input(input: &str) -> Result<DateTime, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("date/time cannot be empty".to_string());
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
    ))
}

fn resolve_tag_category_assignments(
    categories: &[Category],
    names: &[String],
) -> Result<Vec<(CategoryId, String)>, String> {
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
            ));
        }
        assignments.push((category.id, category.name.clone()));
    }
    Ok(assignments)
}

fn resolve_value_assignments(
    categories: &[Category],
    specs: &[String],
) -> Result<Vec<NumericValueAssignment>, String> {
    let mut assignments = Vec::new();
    let mut seen = HashSet::new();
    for spec in specs {
        let (category_name, raw_value) = spec
            .split_once('=')
            .ok_or_else(|| format!("invalid --value '{spec}': expected CATEGORY=NUMBER"))?;
        let category_name = category_name.trim();
        if category_name.is_empty() {
            return Err(format!(
                "invalid --value '{spec}': missing category name before '='"
            ));
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
            ));
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

fn apply_tag_category_assignment(
    aglet: &Aglet<'_>,
    item_id: ItemId,
    category_id: CategoryId,
    category_name: &str,
) -> Result<(), String> {
    if category_name.eq_ignore_ascii_case("Done") {
        aglet.mark_item_done(item_id).map_err(|e| e.to_string())?;
        return Ok(());
    }

    aglet
        .assign_item_manual(item_id, category_id, Some("manual:cli.assign".to_string()))
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn apply_numeric_value_assignment(
    aglet: &Aglet<'_>,
    item_id: ItemId,
    assignment: NumericValueAssignment,
) -> Result<(), String> {
    aglet
        .assign_item_numeric_manual(
            item_id,
            assignment.category_id,
            assignment.value,
            Some("manual:cli.set-value".to_string()),
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn query_from_category_names(
    categories: &[Category],
    include: &[String],
    or_include: &[String],
    exclude: &[String],
) -> Result<Query, String> {
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

fn ensure_mutable_view(view: &View) -> Result<(), String> {
    if view.name.eq_ignore_ascii_case(READY_QUEUE_VIEW_NAME)
        || view.name.eq_ignore_ascii_case(DEFAULT_VIEW_NAME)
    {
        return Err(format!("cannot modify system view: {}", view.name));
    }
    Ok(())
}

fn section_mut(view: &mut View, section_index: usize) -> Result<&mut Section, String> {
    let section_count = view.sections.len();
    view.sections.get_mut(section_index).ok_or_else(|| {
        format!(
            "section index {} out of range (view has {} sections)",
            section_index, section_count
        )
    })
}

fn find_column_index(
    section: &Section,
    category_names: &HashMap<CategoryId, String>,
    column_name: &str,
) -> Result<usize, String> {
    let wanted = column_name.to_ascii_lowercase();
    section
        .columns
        .iter()
        .position(|column| {
            category_names
                .get(&column.heading)
                .is_some_and(|name| name.eq_ignore_ascii_case(&wanted))
        })
        .ok_or_else(|| format!("column '{}' not found", column_name))
}

fn parse_source_parent_mapping(spec: &str, flag_name: &str) -> Result<(String, String), String> {
    let (source, parent) = spec
        .split_once('=')
        .ok_or_else(|| format!("invalid {flag_name} '{spec}': expected SOURCE=PARENT"))?;
    let source = source.trim();
    let parent = parent.trim();
    if source.is_empty() || parent.is_empty() {
        return Err(format!(
            "invalid {flag_name} '{spec}': SOURCE and PARENT must both be non-empty"
        ));
    }
    Ok((source.to_string(), parent.to_string()))
}

fn ensure_category_exists(
    aglet: &Aglet<'_>,
    store: &Store,
    name: &str,
    parent_name: Option<&str>,
    value_kind: CategoryValueKind,
) -> Result<CategoryId, String> {
    if parent_name.is_some_and(|parent| parent.eq_ignore_ascii_case(name)) {
        return Err(format!("category '{}' cannot use itself as a parent", name));
    }

    let existing_categories = store.get_hierarchy().map_err(|e| e.to_string())?;
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
            ));
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
    aglet
        .create_category(&category)
        .map_err(|e| e.to_string())?;
    Ok(category.id)
}

fn csv_record_value(
    record: &csv::StringRecord,
    headers: &csv::StringRecord,
    column_name: &str,
) -> Result<String, String> {
    let wanted = column_name.trim();
    let index = headers
        .iter()
        .position(|header| header.eq_ignore_ascii_case(wanted))
        .ok_or_else(|| format!("CSV column not found: {column_name}"))?;
    Ok(record.get(index).unwrap_or_default().trim().to_string())
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
    /// Configured section column values (present only when the section
    /// defines columns).
    #[serde(skip_serializing_if = "Option::is_none")]
    column_values: Option<Vec<JsonColumnValueOutput>>,
}

#[derive(Serialize)]
struct JsonColumnValueOutput {
    column: String,
    value: String,
}

#[derive(Serialize)]
struct JsonItemsOutput {
    items: Vec<JsonItemRow>,
}

#[derive(Serialize)]
struct JsonViewSubsectionOutput {
    title: String,
    items: Vec<JsonItemRow>,
    summaries: Vec<String>,
}

#[derive(Serialize)]
struct JsonViewSectionOutput {
    title: String,
    items: Vec<JsonItemRow>,
    subsections: Vec<JsonViewSubsectionOutput>,
    summaries: Vec<String>,
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
    hide_dependent_items: bool,
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
        column_values: None,
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

/// Like `rows_to_json`, attaching configured section column values when the
/// section defines columns (UX audit P2-CLI-1).
fn rows_to_json_for_section(
    view: &View,
    section_index: usize,
    items: &[Item],
    category_names: &HashMap<CategoryId, String>,
    sort_keys: &[CliSortKey],
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

fn markdown_sorted_rows(items: &[Item]) -> Vec<&Item> {
    let mut rows: Vec<&Item> = items.iter().collect();
    rows.sort_by(|left, right| {
        left.text
            .to_ascii_lowercase()
            .cmp(&right.text.to_ascii_lowercase())
            .then_with(|| left.id.cmp(&right.id))
    });
    rows
}

fn append_markdown_items(
    out: &mut String,
    items: &[Item],
    category_names: &HashMap<CategoryId, String>,
    store: &Store,
    include_links: bool,
    heading_prefix: &str,
) -> Result<(), String> {
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

fn build_markdown_export(
    store: &Store,
    view_name: Option<&str>,
    include_links: bool,
) -> Result<String, String> {
    let items = store.list_items().map_err(|e| e.to_string())?;
    let categories = store.get_hierarchy().map_err(|e| e.to_string())?;
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

#[allow(clippy::too_many_arguments)]
fn print_items_for_view(
    view: &View,
    items: &[Item],
    categories: &[Category],
    category_names: &HashMap<CategoryId, String>,
    sort_keys: &[CliSortKey],
    output_format: OutputFormatArg,
    blocked_item_ids: &HashSet<ItemId>,
    table_style: TableStyle,
) -> Result<(), String> {
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
        let body = serde_json::to_string_pretty(&payload).map_err(|e| e.to_string())?;
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

fn section_summary_line(
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

fn section_summary_entries(
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

fn column_summary_value(
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

fn summary_fn_label(summary_fn: SummaryFn) -> &'static str {
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
fn render_compact_item_table(
    items: &[Item],
    category_names: &HashMap<CategoryId, String>,
    sort_keys: &[CliSortKey],
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
fn render_section_column_table(
    view: &View,
    section_index: usize,
    items: &[Item],
    category_names: &HashMap<CategoryId, String>,
    sort_keys: &[CliSortKey],
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

fn print_item_table(
    items: &[Item],
    category_names: &HashMap<CategoryId, String>,
    sort_keys: &[CliSortKey],
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
#[path = "main_tests.rs"]
mod tests;
