//! Clap argument and subcommand definitions.

use crate::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NumericValueAssignment {
    pub(crate) category_id: CategoryId,
    pub(crate) category_name: String,
    pub(crate) value: Decimal,
}

#[derive(Parser, Debug)]
#[command(name = "aglet")]
#[command(about = "Aglet CLI/TUI")]
#[command(
    after_help = "Run without a command to launch the TUI. Use `aglet list` for the scriptable list view."
)]
pub(crate) struct Cli {
    /// SQLite database path
    #[arg(long, env = "AGLET_DB")]
    pub(crate) db: Option<PathBuf>,

    #[command(subcommand)]
    pub(crate) command: Option<Command>,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub(crate) enum CategoryTypeArg {
    Tag,
    Numeric,
}

impl CategoryTypeArg {
    pub(crate) fn into_model(self) -> CategoryValueKind {
        match self {
            Self::Tag => CategoryValueKind::Tag,
            Self::Numeric => CategoryValueKind::Numeric,
        }
    }
}

#[derive(Clone, Copy, Debug, ValueEnum, PartialEq, Eq)]
pub(crate) enum OutputFormatArg {
    Table,
    Json,
}

#[derive(Subcommand, Debug)]
pub(crate) enum Command {
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
pub(crate) enum ItemCommand {
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
pub(crate) enum CategoryCommand {
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
pub(crate) enum ViewCommand {
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
pub(crate) enum TableStyle {
    Compact,
    Verbose,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum BrowseStepArg {
    /// Shift by the full window length (current behavior).
    Window,
    /// Shift by one bucket interval (one section).
    Bucket,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum DateSourceArg {
    When,
    Entry,
    Done,
}

impl DateSourceArg {
    pub(crate) fn into_model(self) -> DateSource {
        match self {
            Self::When => DateSource::When,
            Self::Entry => DateSource::Entry,
            Self::Done => DateSource::Done,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum ConditionMatchModeArg {
    Any,
    All,
}

impl ConditionMatchModeArg {
    pub(crate) fn into_model(self) -> ConditionMatchMode {
        match self {
            Self::Any => ConditionMatchMode::Any,
            Self::All => ConditionMatchMode::All,
        }
    }
}

#[derive(ValueEnum, Debug, Clone, Copy)]
pub(crate) enum CliSummaryFn {
    None,
    Sum,
    Avg,
    Min,
    Max,
    Count,
}

impl CliSummaryFn {
    pub(crate) fn to_model(self) -> SummaryFn {
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
pub(crate) enum CliColumnKind {
    Standard,
    When,
}

impl CliColumnKind {
    pub(crate) fn to_model(self) -> ColumnKind {
        match self {
            Self::Standard => ColumnKind::Standard,
            Self::When => ColumnKind::When,
        }
    }
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CliDatebookPeriod {
    Day,
    Week,
    Month,
    Quarter,
    Year,
}

impl CliDatebookPeriod {
    pub(crate) fn into_model(self) -> DatebookPeriod {
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
pub(crate) enum CliDatebookInterval {
    Hourly,
    Daily,
    Weekly,
    Monthly,
}

impl CliDatebookInterval {
    pub(crate) fn into_model(self) -> DatebookInterval {
        match self {
            Self::Hourly => DatebookInterval::Hourly,
            Self::Daily => DatebookInterval::Daily,
            Self::Weekly => DatebookInterval::Weekly,
            Self::Monthly => DatebookInterval::Monthly,
        }
    }
}

#[derive(ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CliDatebookAnchor {
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
    pub(crate) fn into_model(self) -> DatebookAnchor {
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
pub(crate) enum ViewSectionCommand {
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
pub(crate) enum ViewColumnCommand {
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
pub(crate) enum ViewAliasCommand {
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
pub(crate) enum ImportCommand {
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
pub(crate) enum LinkCommand {
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
pub(crate) enum UnlinkCommand {
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
