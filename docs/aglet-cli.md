<!-- GENERATED from docs/src/.htm — DO NOT EDIT. Run make[4]: Nothing to be done for `md'. in docs/. -->

# Aglet CLI Reference

[« Home](index.md)  \| 
[Concepts](aglet-manual.md)  \| 
[TUI Guide](aglet-tui.md)

## How to Use This Manual

Purpose  
Complete reference for the aglet command-line interface. For interactive
use see the [TUI Guide](aglet-tui.md). Core concepts are in the
[Concepts Reference](aglet-manual.md).

**See also**   [Home](index.md)

## Overview

### About the CLI

Purpose  
Drive aglet from the command line — useful for scripting and for LLM coding
agents.

Usage  
    aglet [--db <PATH>] <COMMAND>

How it works  
Running aglet with no subcommand opens the TUI; with a subcommand it
performs that action and exits. Short item-id prefixes work anywhere an
item id is accepted (case-insensitive, hyphens stripped); an ambiguous
prefix returns an error listing the matches. `list` and
`search` default to compact one-line rows; use
`--verbose` for multi-line output and `--format json`
for scripts.

Examples  
    aglet --db notes.ag list
    aglet --db notes.ag add "Buy groceries"

Note  
`aglet list` without `--view` uses All Items when
present, then the first stored view.

**See also**  
[CLI command chart](#cli-command-chart),
[CLI item commands](#cli-item-commands),
[CLI category commands](#cli-category-commands),
[CLI view commands](#cli-view-commands),
[The AGLET_DB variable](#the-aglet-db-variable)

### The AGLET_DB Environment Variable

Purpose  
Choose which database aglet acts on without repeating `--db`.

How it works  
The CLI reads the database path from `--db <path>` or, if
that is absent, the `AGLET_DB` environment variable. Set
`AGLET_DB` once in your shell to run a series of commands against
the same file.

Examples  
    export AGLET_DB=~/notes.ag
    aglet list
    aglet add "Pick up parts"

Note  
`--db` on a single command overrides `AGLET_DB` for
that command.

**See also**  
[About .ag files](#about-ag-files),
[About the CLI](#about-the-cli)

### About .ag Files

Purpose  
Understand where aglet keeps your data.

How it works  
An aglet database is a single SQLite file with the `.ag`
extension. It holds every item, category, view, link, and the deletion log.
A new database is created automatically with the built-in categories (Done,
When, Entry) and the All Items view the first time you open it.

Examples  
    cargo run --bin aglet -- --db getting-started.ag

Note  
Because the database is one file, you back it up or move it by copying that
file. Schema repair runs idempotently on open.

**See also**  
[The AGLET_DB variable](#the-aglet-db-variable),
[Reserved categories](aglet-manual.md#reserved-categories),
[About the CLI](#about-the-cli)

### CLI: Command Chart

| Command    | Purpose                                           |
|------------|---------------------------------------------------|
| `add`      | Add a new item                                    |
| `edit`     | Edit an item's text, note, and/or done state      |
| `show`     | Show a single item with its assignments           |
| `list`     | List items (optionally filtered)                  |
| `search`   | Search item text and note                         |
| `export`   | Export items as Markdown                          |
| `delete`   | Delete an item (writes a deletion log entry)      |
| `deleted`  | List deletion log entries                         |
| `restore`  | Restore an item from the deletion log             |
| `claim`    | Atomically claim an eligible item for active work |
| `ready`    | List items eligible to be claimed                 |
| `release`  | Remove the active claim category (alias: unclaim) |
| `tui`      | Launch the interactive TUI                        |
| `category` | Category commands (see CLI: Category commands)    |
| `view`     | View commands (see CLI: View commands)            |
| `link`     | Item-to-item link commands                        |
| `unlink`   | Remove item-to-item links                         |
| `import`   | Structured import commands (CSV)                  |
| `item`     | Item commands in alternative noun-verb syntax     |

**Options**

|               |                                          |
|---------------|------------------------------------------|
| `--db <PATH>` | SQLite database path (or set AGLET_DB)   |
| `-h, --help`  | Print help for any command or subcommand |

**Note** Run `aglet` with no command to launch the TUI. Every command accepts `--help` for its own options.

**See also**  
[About the CLI](#about-the-cli),
[Item commands](#cli-item-commands),
[Category commands](#cli-category-commands),
[View commands](#cli-view-commands)

## Item Commands

### CLI Item Commands

**Purpose** Create, inspect, modify, and remove items from the command line.

**Commands**

|  |  |
|----|----|
| `add` | Add a new item (`--note`, returns the created id) |
| `edit` | Edit text, note, and/or done state (`--text`, `--note`, `--done`, `--not-done`) |
| `show` | Show a single item with its assignments |
| `list` | List items, optionally filtered |
| `search` | Search item text and note |
| `export` | Export items as Markdown |
| `delete` | Delete an item (writes deletion log) |
| `deleted` | List deletion-log entries |
| `restore` | Restore an item from the deletion log by log id |
| `item` | Alternative noun-verb syntax for item commands |

**Examples**

    aglet add "Plan offsite" --note "Book venue"
    aglet show <ITEM>
    aglet export > items.md

**Note** Parse the "created " line from `add` for the new id; it is not always the last line printed.

**See also**  
[Add an item](aglet-tui.md#add-an-item),
[Edit an item](aglet-tui.md#edit-an-item),
[Delete an item](aglet-tui.md#delete-an-item),
[CLI filtering](#cli-filtering),
[CLI import and export](#cli-import-export)

## Category Commands

### CLI Category Commands

**Purpose** Manage categories, assignments, conditions, and actions from the command line.

**Commands**

|  |  |
|----|----|
| `list` | List categories as a tree |
| `show` | Show details for a category |
| `create` | Create a category (`--parent`, `--exclusive`, `--type numeric`) |
| `delete` | Delete a category by name |
| `rename` | Rename a category |
| `reparent` | Reparent (`--root` makes it top-level) |
| `update` | Update category flags |
| `assign` | Assign an item to a category |
| `unassign` | Unassign an item from a category |
| `set-value` | Set a numeric value assignment |
| `format` | Configure numeric formatting |
| `add-condition` | Add a profile condition |
| `add-date-condition` | Add a date condition |
| `set-condition-mode` | Set how explicit conditions combine |
| `remove-condition` | Remove a condition by 1-based index |
| `add-action` | Add an action |
| `remove-action` | Remove an action by 1-based index |

**Examples**

    aglet category create "Priority" --exclusive
    aglet category create "High" --parent Priority
    aglet category assign <ITEM> High
    aglet category set-value <ITEM> Cost 450.00

**Note** Done, When, and Entry are reserved and cannot be created, renamed, or deleted.

**See also**  
[Add a category](aglet-tui.md#add-a-category),
[Assign a category](aglet-tui.md#assign-a-category),
[Profile conditions](#profile-conditions),
[Actions](#actions),
[Set a numeric value](aglet-tui.md#set-a-numeric-value)

### Profile Conditions

Purpose  
Assign a category based on structured rules about an item, beyond a plain
name match.

CLI steps  
    aglet category add-condition <CATEGORY> ...
    aglet category set-condition-mode <CATEGORY> ...
    aglet category remove-condition <CATEGORY> <INDEX>

How it works  
A category can carry one or more explicit conditions. The condition mode
controls how multiple conditions combine. When an item satisfies the
conditions, the category is derived automatically. Derived (non-sticky)
assignments can break if the item stops matching.

Note  
In the category manager the left pane shows a readable rule-count badge such
as \[2 conditions\]. Conditions are re-evaluated when an item's text or dates
change.

**See also**  
[Automatic assignment](aglet-manual.md#automatic-assignment),
[Date conditions](#date-conditions),
[Actions](#actions),
[CLI category commands](#cli-category-commands)

### Date Conditions

Purpose  
Assign a category based on an item's date — for example, to bucket items by
when they are due.

CLI steps  
    aglet category add-date-condition <CATEGORY> ...

How it works  
A date condition tests an item's When date against a range or relative
window. When it matches, the category is derived. Date conditions power
date-range categories used to build datebook views and date-grouped
sections.

Note  
Direct SQLite writes do not sync the reserved When assignment; use
aglet/CLI logic so date conditions see the date.

**See also**  
[Profile conditions](#profile-conditions),
[Datebook views](aglet-manual.md#datebook-views),
[Reserved categories](aglet-manual.md#reserved-categories)

### Actions

Purpose  
Make assigning one category automatically assign or remove another.

CLI steps  
    aglet category add-action <CATEGORY> ...
    aglet category remove-action <CATEGORY> <INDEX>

How it works  
An action fires when its owning category is assigned to an item. An Assign
action adds another category; a Remove action removes one. Actions are
event-driven: adding or editing an action does not retroactively fire for
items that already have the owning category. Action-applied assignments are
sticky.

Note  
The category manager shows an action badge such as \[1 action\] in the left
pane.

**See also**  
[Profile conditions](#profile-conditions),
[Automatic assignment](aglet-manual.md#automatic-assignment),
[CLI category commands](#cli-category-commands)

## View Commands

### CLI View Commands

**Purpose** Create and edit views, sections, columns, aliases, and datebooks from the command line.

**Commands**

|                             |                                           |
|-----------------------------|-------------------------------------------|
| `list`                      | List views                                |
| `show`                      | Show the contents of a view               |
| `create`                    | Create a view from include/exclude        |
| `edit`                      | Edit mutable view properties              |
| `clone`                     | Clone into a new mutable view             |
| `rename`                    | Rename a view                             |
| `delete`                    | Delete a view by name                     |
| `section add/remove/update` | Section authoring                         |
| `column add/remove/update`  | Column authoring                          |
| `alias set/clear`           | Per-view category display alias           |
| `set-summary`               | Set a column summary function             |
| `set-item-label`            | Set or clear the item column label        |
| `set-remove-from-view`      | Replace the remove-from-view category set |
| `create-datebook`           | Create a datebook (date-interval) view    |
| `datebook-browse`           | Shift a datebook view's window            |

**Examples**

    aglet view create "Work Queue" --include Work --exclude Done
    aglet view show "Work Queue"
    aglet view clone "All Items" "My Items"

**Note** `--include` criteria are AND-based; use sections or separate views for mutually exclusive siblings.

**See also**  
[Create a view](aglet-tui.md#create-a-view),
[Add a section](aglet-tui.md#add-a-section),
[Add a column](aglet-tui.md#add-a-column),
[View aliases](aglet-tui.md#view-aliases),
[Create a datebook view](aglet-tui.md#create-a-datebook-view)

## Link Commands

### CLI Link Commands

**Purpose** Create and remove item-to-item links from the command line.

**Commands**

|  |  |
|----|----|
| `link depends-on` | ITEM depends on DEPENDS_ON_ITEM |
| `link blocks` | BLOCKER blocks BLOCKED |
| `link related` | Bidirectional related link |
| `unlink depends-on / blocks / related` | Remove the corresponding link (canonical entry) |

**Examples**

    aglet link depends-on <ITEM> <PREREQUISITE>
    aglet link related <A> <B>
    aglet unlink depends-on <ITEM> <PREREQUISITE>

**Note** depends-on and blocks describe the same directed relationship from opposite ends; related is symmetric.

**See also**  
[Create a dependency](aglet-tui.md#create-a-dependency),
[Remove a link](aglet-tui.md#remove-a-link),
[Dependencies](aglet-manual.md#dependencies)

## Filtering and Sorting

### CLI Filtering and Sorting

**Purpose** Narrow and order the output of `list`, `search`, and `view show`.

**Flags**

|                              |                                    |
|------------------------------|------------------------------------|
| `--category <C>`             | Repeatable, AND semantics          |
| `--any-category <C>`         | Repeatable, OR semantics           |
| `--exclude-category <C>`     | Repeatable, NOT semantics          |
| `--blocked / --not-blocked`  | Dependency-state filters (derived) |
| `--value-eq <C> <V>`         | Numeric value equals               |
| `--value-in <C> <CSV>`       | Numeric value in a set             |
| `--value-max <C> <V>`        | Numeric value at most              |
| `--sort <COLUMN[:asc|desc]>` | Sort order                         |
| `--view <VIEW>`              | Use a specific view                |
| `--include-done`             | Include done items                 |
| `--format <FORMAT>`          | Output format (e.g. json)          |
| `--verbose`                  | Multi-line human output            |

**Examples**

    aglet list --category High --not-blocked
    aglet list --any-category Work --any-category Personal
    aglet list --sort Cost:desc --format json

**Note** Repeated `--category` is AND; for OR across siblings use `--any-category`. blocked/not-blocked are derived, not categories.

**See also**  
[View criteria](aglet-tui.md#view-criteria),
[Filter blocked items](aglet-tui.md#filter-blocked-items),
[Search items](aglet-tui.md#search-items),
[CLI item commands](#cli-item-commands)

## Import and Export

### CLI Import and Export

**Purpose** Move items into and out of aglet.

**Commands**

|          |                                       |
|----------|---------------------------------------|
| `import` | Structured import commands (e.g. CSV) |
| `export` | Export items as Markdown              |

**Examples**

    aglet export > items.md
    aglet import ...

**How it works** Export writes items as Markdown to standard output. Import brings structured data in. Prefer the CLI/import path over direct SQLite writes so dates and reserved When provenance are handled correctly.

**Note** Direct SQLite imports must use the store datetime format `YYYY-MM-DD HH:MM:SS`; ISO strings will not load as dates.

**See also**  
[CLI item commands](#cli-item-commands),
[About the CLI](#about-the-cli),
[About .ag files](#about-ag-files)

## Workflow Commands

### Claim an Item

Purpose  
Atomically take an eligible item for active work, marking it as yours.

CLI steps  
    aglet claim <ITEM>

How it works  
Claiming applies the configured claim-target category. An item is claimable
only if it has the configured Ready category, does not already have the
claim-target category, is not done, and is not blocked by an unresolved
depends-on prerequisite. Claimability is computed, not a link type.

Note  
If a claim races and fails because the item is already In Progress, pick
another item rather than force-assigning.

**See also**  
[The ready list](#the-ready-list),
[Release a claim](#release-a-claim),
[Filter blocked items](aglet-tui.md#filter-blocked-items),
[Global settings](aglet-tui.md#global-settings)

### Release a Claim

Purpose  
Give up an item you previously claimed.

CLI steps  
    aglet release <ITEM>
    aglet unclaim <ITEM>      # alias

How it works  
Releasing removes the active claim-target category, returning the item to
the pool of claimable work if it still has the Ready category.

Note  
release and unclaim are the same command.

**See also**  
[Claim an item](#claim-an-item),
[The ready list](#the-ready-list)

### The Ready List

Purpose  
See the items that are eligible to be claimed right now.

CLI steps  
    aglet ready

How it works  
`ready` lists items that have the Ready category and are not
done, not already claimed, and not blocked by unresolved dependencies. It
is the recommended way to pick the next piece of work.

Note  
Because `ready` already excludes done, claimed, and blocked
items, prefer it over scanning a full list when choosing work.

**See also**  
[Claim an item](#claim-an-item),
[Release a claim](#release-a-claim),
[Filter blocked items](aglet-tui.md#filter-blocked-items)
