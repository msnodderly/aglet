<!-- GENERATED from docs/src/.htm — DO NOT EDIT. Run make[4]: Nothing to be done for `md'. in docs/. -->

# Aglet CLI Reference

[« Home](index.md)  \| 
[Concepts](aglet-manual.md)  \|  [TUI Guide](aglet-tui.md)

## <span id="how to use this manual">How to Use This Manual</span>

    PURPOSE    Complete reference for the aglet command-line interface.
               For interactive use see the
               TUI Guide.
               Core concepts are in the
               Concepts Reference.

#### SEE ALSO

> [Home](index.md)

## <span id="index">Index</span>

    OVERVIEW
      About the CLI
      The AGLET_DB Environment Variable
      About .ag Files
      CLI: Command Chart

    ITEM COMMANDS
      CLI Item Commands

    CATEGORY COMMANDS
      CLI Category Commands
      Profile Conditions
      Date Conditions
      Actions

    VIEW COMMANDS
      CLI View Commands

    LINK COMMANDS
      CLI Link Commands

    FILTERING AND SORTING
      CLI Filtering and Sorting

    IMPORT AND EXPORT
      CLI Import and Export

    WORKFLOW COMMANDS
      Claim an Item
      Release a Claim
      The Ready List

# Overview

## <span id="about the cli">About the CLI</span>

    PURPOSE       Drive aglet from the command line - useful for scripting and for
                  LLM coding agents.

    USAGE         aglet [--db ] 

    HOW IT WORKS  Running aglet with no subcommand opens the TUI; with a subcommand
                  it performs that action and exits. Short item-id prefixes work
                  anywhere an item id is accepted (case-insensitive, hyphens
                  stripped); an ambiguous prefix returns an error listing the
                  matches. list and search default to compact one-line rows; use
                  --verbose for multi-line output and --format json for scripts.

    EXAMPLES      aglet --db notes.ag list
                  aglet --db notes.ag add "Buy groceries"

    NOTE          aglet list without --view uses All Items when present, then the
                  first stored view.

#### SEE ALSO

> [CLI command chart](#cli%20command%20chart),   [CLI item commands](#cli%20item%20commands),   [CLI category commands](#cli%20category%20commands),   [CLI view commands](#cli%20view%20commands),   [The AGLET_DB variable](#the%20aglet%20db%20variable),   [» Index](#index)

## <span id="the aglet db variable">The AGLET_DB Environment Variable</span>

    PURPOSE       Choose which database aglet acts on without repeating --db.

    HOW IT WORKS  The CLI reads the database path from --db  or, if that is
                  absent, the AGLET_DB environment variable. Set AGLET_DB once in
                  your shell to run a series of commands against the same file.

    EXAMPLES      export AGLET_DB=~/notes.ag
                  aglet list
                  aglet add "Pick up parts"

    NOTE          --db on a single command overrides AGLET_DB for that command.

#### SEE ALSO

> [About .ag files](#about%20ag%20files),   [About the CLI](#about%20the%20cli),   [» Index](#index)

## <span id="about ag files">About .ag Files</span>

    PURPOSE       Understand where aglet keeps your data.

    HOW IT WORKS  An aglet database is a single SQLite file with the .ag extension.
                  It holds every item, category, view, link, and the deletion log.
                  A new database is created automatically with the built-in
                  categories (Done, When, Entry) and the All Items view the first
                  time you open it.

    EXAMPLES      cargo run --bin aglet -- --db getting-started.ag

    NOTE          Because the database is one file, you back it up or move it by
                  copying that file. Schema repair runs idempotently on open.

#### SEE ALSO

> [The AGLET_DB variable](#the%20aglet%20db%20variable),   [Reserved categories](aglet-manual.md#reserved%20categories),   [About the CLI](#about%20the%20cli),   [» Index](#index)

## <span id="cli command chart">CLI: Command Chart</span>

    COMMAND       PURPOSE
    ------------  -----------------------------------------------------------
    add           Add a new item
    edit          Edit an item's text, note, and/or done state
    show          Show a single item with its assignments
    list          List items (optionally filtered)
    search        Search item text and note
    export        Export items as Markdown
    delete        Delete an item (writes a deletion log entry)
    deleted       List deletion log entries
    restore       Restore an item from the deletion log
    claim         Atomically claim an eligible item for active work
    ready         List items eligible to be claimed
    release       Remove the active claim category (alias: unclaim)
    tui           Launch the interactive TUI
    category      Category commands (see CLI: Category commands)
    view          View commands (see CLI: View commands)
    link          Item-to-item link commands
    unlink        Remove item-to-item links
    import        Structured import commands (CSV)
    item          Item commands in alternative noun-verb syntax

    OPTIONS       --db     SQLite database path (or set AGLET_DB)
                  -h, --help     Print help for any command or subcommand

    NOTE          Run "aglet" with no command to launch the TUI. Every command
                  accepts --help for its own options.

#### SEE ALSO

> [About the CLI](#about%20the%20cli),   [Item commands](#cli%20item%20commands),   [Category commands](#cli%20category%20commands),   [View commands](#cli%20view%20commands),   [» Index](#index)

------------------------------------------------------------------------

# Concepts

# Item Commands

## <span id="cli item commands">CLI Item Commands</span>

    PURPOSE       Create, inspect, modify, and remove items from the command line.

    COMMANDS      add       Add a new item (--note, returns the created id)
                  edit      Edit text, note, and/or done state (--text, --note,
                            --done, --not-done)
                  show      Show a single item with its assignments
                  list      List items, optionally filtered
                  search    Search item text and note
                  export    Export items as Markdown
                  delete    Delete an item (writes deletion log)
                  deleted   List deletion-log entries
                  restore   Restore an item from the deletion log by log id
                  item      Alternative noun-verb syntax for item commands

    EXAMPLES      aglet add "Plan offsite" --note "Book venue"
                  aglet show 
                  aglet export > items.md

    NOTE          Parse the "created " line from add for the new id; it is not
                  always the last line printed.

#### SEE ALSO

> [Add an item](aglet-tui.md#add%20an%20item),   [Edit an item](aglet-tui.md#edit%20an%20item),   [Delete an item](aglet-tui.md#delete%20an%20item),   [CLI filtering](#cli%20filtering),   [CLI import and export](#cli%20import%20export),   [» Index](#index)

# Category Commands

## <span id="cli category commands">CLI Category Commands</span>

    PURPOSE       Manage categories, assignments, conditions, and actions from the
                  command line.

    COMMANDS      list                List categories as a tree
                  show                Show details for a category
                  create              Create a category (--parent, --exclusive,
                                      --type numeric)
                  delete              Delete a category by name
                  rename              Rename a category
                  reparent            Reparent (--root makes it top-level)
                  update              Update category flags
                  assign              Assign an item to a category
                  unassign            Unassign an item from a category
                  set-value           Set a numeric value assignment
                  format              Configure numeric formatting
                  add-condition       Add a profile condition
                  add-date-condition  Add a date condition
                  set-condition-mode  Set how explicit conditions combine
                  remove-condition    Remove a condition by 1-based index
                  add-action          Add an action
                  remove-action       Remove an action by 1-based index

    EXAMPLES      aglet category create "Priority" --exclusive
                  aglet category create "High" --parent Priority
                  aglet category assign  High
                  aglet category set-value  Cost 450.00

    NOTE          Done, When, and Entry are reserved and cannot be created, renamed,
                  or deleted.

#### SEE ALSO

> [Add a category](aglet-tui.md#add%20a%20category),   [Assign a category](aglet-tui.md#assign%20a%20category),   [Profile conditions](#profile%20conditions),   [Actions](#actions),   [Set a numeric value](aglet-tui.md#set%20a%20numeric%20value),   [» Index](#index)

## <span id="profile conditions">Profile Conditions</span>

    PURPOSE       Assign a category based on structured rules about an item, beyond
                  a plain name match.

    CLI STEPS     aglet category add-condition  ...
                  aglet category set-condition-mode  ...
                  aglet category remove-condition  

    HOW IT WORKS  A category can carry one or more explicit conditions. The
                  condition mode controls how multiple conditions combine. When an
                  item satisfies the conditions, the category is derived
                  automatically. Derived (non-sticky) assignments can break if the
                  item stops matching.

    NOTE          In the category manager the left pane shows a readable rule-count
                  badge such as [2 conditions]. Conditions are re-evaluated when an
                  item's text or dates change.

#### SEE ALSO

> [Automatic assignment](aglet-manual.md#automatic%20assignment),   [Date conditions](#date%20conditions),   [Actions](#actions),   [CLI category commands](#cli%20category%20commands),   [» Index](#index)

## <span id="date conditions">Date Conditions</span>

    PURPOSE       Assign a category based on an item's date - for example, to bucket
                  items by when they are due.

    CLI STEPS     aglet category add-date-condition  ...

    HOW IT WORKS  A date condition tests an item's When date against a range or
                  relative window. When it matches, the category is derived. Date
                  conditions power date-range categories used to build datebook
                  views and date-grouped sections.

    NOTE          Direct SQLite writes do not sync the reserved When assignment; use
                  aglet/CLI logic so date conditions see the date.

#### SEE ALSO

> [Profile conditions](#profile%20conditions),   [Datebook views](aglet-manual.md#datebook%20views),   [Reserved categories](aglet-manual.md#reserved%20categories),   [» Index](#index)

## <span id="actions">Actions</span>

    PURPOSE       Make assigning one category automatically assign or remove another.

    CLI STEPS     aglet category add-action  ...
                  aglet category remove-action  

    HOW IT WORKS  An action fires when its owning category is assigned to an item.
                  An Assign action adds another category; a Remove action removes
                  one. Actions are event-driven: adding or editing an action does
                  not retroactively fire for items that already have the owning
                  category. Action-applied assignments are sticky.

    NOTE          The category manager shows an action badge such as [1 action] in
                  the left pane.

#### SEE ALSO

> [Profile conditions](#profile%20conditions),   [Automatic assignment](aglet-manual.md#automatic%20assignment),   [CLI category commands](#cli%20category%20commands),   [» Index](#index)

# View Commands

## <span id="cli view commands">CLI View Commands</span>

    PURPOSE       Create and edit views, sections, columns, aliases, and datebooks
                  from the command line.

    COMMANDS      list                  List views
                  show                  Show the contents of a view
                  create                Create a view from include/exclude
                  edit                  Edit mutable view properties
                  clone                 Clone into a new mutable view
                  rename                Rename a view
                  delete                Delete a view by name
                  section add/remove/update    Section authoring
                  column add/remove/update     Column authoring
                  alias set/clear              Per-view category display alias
                  set-summary           Set a column summary function
                  set-item-label        Set or clear the item column label
                  set-remove-from-view  Replace the remove-from-view category set
                  create-datebook       Create a datebook (date-interval) view
                  datebook-browse       Shift a datebook view's window

    EXAMPLES      aglet view create "Work Queue" --include Work --exclude Done
                  aglet view show "Work Queue"
                  aglet view clone "All Items" "My Items"

    NOTE          --include criteria are AND-based; use sections or separate views
                  for mutually exclusive siblings.

#### SEE ALSO

> [Create a view](aglet-tui.md#create%20a%20view),   [Add a section](aglet-tui.md#add%20a%20section),   [Add a column](aglet-tui.md#add%20a%20column),   [View aliases](aglet-tui.md#view%20aliases),   [Create a datebook view](aglet-tui.md#create%20a%20datebook),   [» Index](#index)

# Link Commands

## <span id="cli link commands">CLI Link Commands</span>

    PURPOSE       Create and remove item-to-item links from the command line.

    COMMANDS      link depends-on   ITEM depends on DEPENDS_ON_ITEM
                  link blocks       BLOCKER blocks BLOCKED
                  link related      Bidirectional related link
                  unlink depends-on / blocks / related   Remove the corresponding
                                                         link (canonical entry)

    EXAMPLES      aglet link depends-on  
                  aglet link related  
                  aglet unlink depends-on  

    NOTE          depends-on and blocks describe the same directed relationship from
                  opposite ends; related is symmetric.

#### SEE ALSO

> [Create a dependency](aglet-tui.md#create%20a%20dependency),   [Remove a link](aglet-tui.md#remove%20a%20link),   [Dependencies](aglet-manual.md#dependencies),   [» Index](#index)

# Filtering and Sorting

## <span id="cli filtering">CLI Filtering and Sorting</span>

    PURPOSE       Narrow and order the output of list, search, and view show.

    FLAGS         --category             Repeatable, AND semantics
                  --any-category         Repeatable, OR semantics
                  --exclude-category     Repeatable, NOT semantics
                  --blocked / --not-blocked Dependency-state filters (derived)
                  --value-eq          Numeric value equals
                  --value-in        Numeric value in a set
                  --value-max         Numeric value at most
                  --sort   Sort order
                  --view              Use a specific view
                  --include-done            Include done items
                  --format          Output format (e.g. json)
                  --verbose                 Multi-line human output

    EXAMPLES      aglet list --category High --not-blocked
                  aglet list --any-category Work --any-category Personal
                  aglet list --sort Cost:desc --format json

    NOTE          Repeated --category is AND; for OR across siblings use
                  --any-category. blocked/not-blocked are derived, not categories.

#### SEE ALSO

> [View criteria](aglet-tui.md#view%20criteria),   [Filter blocked items](aglet-tui.md#filter%20blocked%20items),   [Search items](aglet-tui.md#search%20items),   [CLI item commands](#cli%20item%20commands),   [» Index](#index)

# Import and Export

## <span id="cli import export">CLI Import and Export</span>

    PURPOSE       Move items into and out of aglet.

    COMMANDS      import   Structured import commands (e.g. CSV)
                  export   Export items as Markdown

    EXAMPLES      aglet export > items.md
                  aglet import ...

    HOW IT WORKS  Export writes items as Markdown to standard output. Import brings
                  structured data in. Prefer the CLI/import path over direct SQLite
                  writes so dates and reserved When provenance are handled
                  correctly.

    NOTE          Direct SQLite imports must use the store datetime format
                  YYYY-MM-DD HH:MM:SS; ISO strings will not load as dates.

#### SEE ALSO

> [CLI item commands](#cli%20item%20commands),   [About the CLI](#about%20the%20cli),   [About .ag files](#about%20ag%20files),   [» Index](#index)

# Workflow Commands

## <span id="claim an item">Claim an Item</span>

    PURPOSE       Atomically take an eligible item for active work, marking it as
                  yours.

    CLI STEPS     aglet claim 

    HOW IT WORKS  Claiming applies the configured claim-target category. An item is
                  claimable only if it has the configured Ready category, does not
                  already have the claim-target category, is not done, and is not
                  blocked by an unresolved depends-on prerequisite. Claimability is
                  computed, not a link type.

    NOTE          If a claim races and fails because the item is already In
                  Progress, pick another item rather than force-assigning.

#### SEE ALSO

> [The ready list](#the%20ready%20list),   [Release a claim](#release%20a%20claim),   [Filter blocked items](aglet-tui.md#filter%20blocked%20items),   [Global settings](aglet-tui.md#global%20settings),   [» Index](#index)

## <span id="release a claim">Release a Claim</span>

    PURPOSE       Give up an item you previously claimed.

    CLI STEPS     aglet release 
                  aglet unclaim       # alias

    HOW IT WORKS  Releasing removes the active claim-target category, returning the
                  item to the pool of claimable work if it still has the Ready
                  category.

    NOTE          release and unclaim are the same command.

#### SEE ALSO

> [Claim an item](#claim%20an%20item),   [The ready list](#the%20ready%20list),   [» Index](#index)

## <span id="the ready list">The Ready List</span>

    PURPOSE       See the items that are eligible to be claimed right now.

    CLI STEPS     aglet ready

    HOW IT WORKS  ready lists items that have the Ready category and are not done,
                  not already claimed, and not blocked by unresolved dependencies.
                  It is the recommended way to pick the next piece of work.

    NOTE          Because ready already excludes done, claimed, and blocked items,
                  prefer it over scanning a full list when choosing work.

#### SEE ALSO

> [Claim an item](#claim%20an%20item),   [Release a claim](#release%20a%20claim),   [Filter blocked items](aglet-tui.md#filter%20blocked%20items),   [» Index](#index)
