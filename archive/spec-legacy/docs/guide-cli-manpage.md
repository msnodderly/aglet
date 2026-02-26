# AGENDA-CLI(1)

## NAME

agenda-cli - Agenda Reborn command-line interface

## SYNOPSIS

`agenda-cli [--db <DB>] [COMMAND]`

## DESCRIPTION

`agenda-cli` manages items, categories, views, deletion-log restore, and TUI launch for the Agenda database.

If no `COMMAND` is provided, the CLI defaults to:

`agenda-cli list`

## GLOBAL OPTIONS

`--db <DB>`
: SQLite database path. If omitted, defaults to `~/.agenda/default.ag`.

`-h, --help`
: Show help.

## ENVIRONMENT

`AGENDA_DB`
: Default database path when `--db` is not supplied.

## COMMANDS

### add

Add a new item.

Usage:

`agenda-cli add [--note <NOTE>] <TEXT>`

Notes:

- Natural-language date/time in `<TEXT>` may be parsed into `when_date`.
- Parsed date/time text can auto-assign the reserved `When` category.
- When a capture resolves to `when_date`, `add` prints `parsed_when=<datetime>`.
- Hashtag text like `#high` can match an existing category name (`High`) via implicit string matching.
- Unknown hashtags do not auto-create categories. `add` emits a non-blocking warning line (`warning: unknown_hashtags=...`) while keeping capture successful.
- Date parsing for capture uses the local calendar date as reference (`Local::now().date_naive()`).
  Weekday/date phrase resolution is therefore based on local date (not UTC date).

### edit

Edit an existing item's text, note, and/or done state.

Usage:

`agenda-cli edit [--note <NOTE>] [--clear-note] [--done <true|false>] <ITEM_ID> [TEXT]`

Arguments:

- `<ITEM_ID>`: The item to edit.
- `[TEXT]`: New text (optional positional shorthand; also available as `--text`).

Options:

- `--note <NOTE>`: Update the note field.
- `--clear-note`: Clear the note field.
- `--done <true|false>`: Set done state. When marking done, assigns reserved `Done` category.

### show

Show a single item with its assignments.

Usage:

`agenda-cli show <ITEM_ID>`

### list

List items, optionally filtered.

Usage:

`agenda-cli list [--view <VIEW>] [--category <CATEGORY>] [--include-done]`

### search

Search item text and note fields.

Usage:

`agenda-cli search [--include-done] <QUERY>`

### delete

Delete an item (writes deletion log entry).

Usage:

`agenda-cli delete <ITEM_ID>`

### deleted

List deletion-log entries.

Usage:

`agenda-cli deleted`

### restore

Restore an item from deletion log entry ID.

Usage:

`agenda-cli restore <LOG_ID>`

### tui

Launch interactive TUI.

Usage:

`agenda-cli tui`

### category

Category management commands.

Usage:

`agenda-cli category <COMMAND>`

Subcommands:

`agenda-cli category list`
: List categories as a tree.

`agenda-cli category show <NAME>`
: Show detailed info for a category.

`agenda-cli category create [--parent <PARENT>] [--exclusive] [--disable-implicit-string] <NAME>`
: Create category.

`agenda-cli category delete <NAME>`
: Delete category by name.

`agenda-cli category rename <NAME> <NEW_NAME>`
: Rename a category.

`agenda-cli category reparent [--parent <PARENT>] [--root] <NAME>`
: Reparent a category. Use `--root` to make top-level.

`agenda-cli category update [--exclusive <true|false>] [--actionable <true|false>] [--implicit-string <true|false>] [--note <NOTE>] [--clear-note] <NAME>`
: Update category flags.

`agenda-cli category assign <ITEM_ID> <CATEGORY_NAME>`
: Assign item to category by name.

`agenda-cli category unassign <ITEM_ID> <CATEGORY_NAME>`
: Unassign an item from a category.

Category semantics:

- Category names are globally unique across the database.
- Duplicate create attempts fail and now guide you to reuse existing categories via `category assign`.
- `--exclusive` on a parent category means item assignments are single-choice among that parent's children.
- Manual assignment respects exclusivity (assigning one child unassigns exclusive siblings).
- Special case: assigning category `Done` applies done semantics (same effect as `edit --done true`).

### view

View management commands.

Usage:

`agenda-cli view <COMMAND>`

Subcommands:

`agenda-cli view list`
: List views.

`agenda-cli view show <NAME>`
: Show the contents of a view.

`agenda-cli view create [--include <INCLUDE>] [--exclude <EXCLUDE>] [--hide-unmatched] <NAME>`
: Create a basic view from include/exclude categories.

`agenda-cli view rename <NAME> <NEW_NAME>`
: Rename a view.

`agenda-cli view delete <NAME>`
: Delete a view by name.

## RESERVED CATEGORIES

Store initialization includes:

- `When`
- `Entry`
- `Done`

These are reserved category names.

## EXIT STATUS

`0`
: Command succeeded.

Non-zero
: Command failed. Error text is printed to stderr.

## EXAMPLES

Create and list items:

`agenda-cli add "Follow up with Sarah next Tuesday at 3pm"`

`agenda-cli list`

Example add output when date parsing succeeds:

`created <item-id>`

`parsed_when=2026-02-24 15:00:00`

Edit an item:

`agenda-cli edit <item-id> "Updated text"`

`agenda-cli edit <item-id> --note "Added a note"`

Mark an item done:

`agenda-cli edit <item-id> --done true`

Create a global priority taxonomy:

`agenda-cli category create Priority --exclusive`

`agenda-cli category create High --parent Priority`

`agenda-cli category create Medium --parent Priority`

Assign an item to priority:

`agenda-cli category assign <item-id> High`

View-specific listing:

`agenda-cli view create "Work View" --include Work`

`agenda-cli list --view "Work View"`
