# Claude.md

The role of this file is to describe common mistakes and confusion points that
agents might encounter as they work in this project. If you ever encounter
something in the project that surprises you, please alert the developer working
with you and indicate that this is the case in the AGENTS.md file (AKA
CLAUDE.md) to help prevent future agents from having the same issue.

This project is super greenfield. Don't assume that the data matters.
Everything in this project is test data and can be recreated. We can change the
schema's entirely if needed. It's more important to get the project in the
right shape and to get the data models right than it is to preserve any kind of
backward compatibility.

## Reserved Categories

The database has reserved categories that cannot be modified or used as child
category names:
- `Done` - reserved category for marking items complete
- `When` - reserved for date/time parsing
- `Entry` - reserved for entry metadata

If you try to create a child category named `Done` under an exclusive parent
like `Status`, you'll get: `error: cannot modify reserved category: Done`

Use alternative names like `Completed` instead.

## View Include Semantics

View `--include` filters are **AND-based**, not OR-based. This means:
- `view create "My View" --include High --include Pending` requires items to
  have BOTH High AND Pending categories
- You CANNOT use multiple includes for mutually exclusive categories (e.g.,
  `--include Pending --include "In Progress"` where both are children of an
  exclusive `Status` parent)

To show items with different mutually exclusive values:
- Create separate views for each value
- Or use sections (TUI feature, not yet exposed in CLI)

## CLI Default Behavior

Running `agenda-cli list` without arguments shows a default view, which may be
empty if not configured correctly. Use `agenda-cli view show "All Items"` to
see all items, or create views that match your data.

## Category Assignment in Items

When viewing items, the categories list includes both the assigned category and
all its parent categories. For example, an item assigned to `High` will show
`High, Priority` in its categories list (both the child and the parent).

## Database Files

Aglet databases use the `.ag` extension and are SQLite files. The CLI accepts
`--db <path>` (or `AGENDA_DB` env var) to target a specific database.

The project has two binaries: `agenda-cli` and `agenda-tui`. Use
`cargo run --bin agenda-cli` or `cargo run --bin agenda-tui` to run them.

## Feature Requests Database

`feature-requests.ag` in the project root is the dogfooding database used to
track feature requests for aglet itself. It uses these categories:

- **Area** (non-exclusive): CLI, UX, Validation, Display, Automation
- **Priority** (exclusive): High, Medium, Low
- **Status** (exclusive): Pending, In Progress, Completed, Deferred

Views defined: All Items, Backlog, CLI, Deferred, High Priority, Pending, UX.

