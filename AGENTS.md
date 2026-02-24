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

Every item should have Priority, Status, and at least one Area category. If
you see an item missing any of these in `view show "All Items"`, assign them.

Manual TUI testing against `feature-requests.ag` can create SQLite sidecar files
`feature-requests.ag-wal` and `feature-requests.ag-shm`. Treat these as local
runtime artifacts and do not commit them.

## CLI Grooming Patterns

**Do not use shell variable shorthand for commands.** This does NOT work:

```bash
CLI="cargo run --bin agenda-cli -- --db feature-requests.ag"
$CLI list   # ERROR: command not found
```

Write the full command each time, or use `&&` to chain them:

```bash
cargo run --bin agenda-cli -- --db feature-requests.ag add "Title" --note "..." 2>&1 | tail -2
```

**Item ID prefix matching works.** You can use the first 8 hex characters of a
UUID instead of the full ID:

```bash
# These are equivalent:
cargo run --bin agenda-cli -- --db feature-requests.ag category assign be6f0754 High
cargo run --bin agenda-cli -- --db feature-requests.ag category assign be6f0754-a764-40ee-bb48-0bfc225b174b High
```

**Create-then-assign pattern.** `agenda-cli add` prints the new ID on the last
line. Capture it and assign categories with `&&`-chained commands:

```bash
cargo run --bin agenda-cli -- --db feature-requests.ag add "My item" --note "..." 2>&1 | tail -1
# Output: "created <uuid>"
# Then assign:
cargo run --bin agenda-cli -- --db feature-requests.ag category assign <uuid> High 2>&1 | tail -1
cargo run --bin agenda-cli -- --db feature-requests.ag category assign <uuid> Pending 2>&1 | tail -1
```

**Items appearing twice in `list` or `view show` is expected.** The "All Items"
view has two sections. Items that match both section criteria appear in each.
This is not a bug — it is the same item displayed in multiple sections.

**Piping `cargo run` output through `head` on multiple lines.** Each `cargo run`
command must be its own pipeline. Do not put multiple bare `cargo run` commands
on consecutive lines and expect them to share a single `| head`:

```bash
# WRONG — head receives file args, not piped input:
cargo run ... show <id1> 2>/dev/null | head -5
cargo run ... show <id2> 2>/dev/null | head -5

# RIGHT — chain with && or run in separate Bash tool calls:
cargo run ... show <id1> 2>&1 | head -5 && cargo run ... show <id2> 2>&1 | head -5
```

## TUI Spec vs. Implementation

`spec/tui-ux-redesign.md` is the design target. The Phase 2a implementation
deviated from spec in several places. **Read the implementation notes at the
bottom of each phase section** before assuming the spec describes current code:

- **Save key**: spec §4.7 says `S`; current code uses `Enter` in ViewEdit
- **ViewEdit regions**: spec says 4 (Criteria/Columns/Sections/Unmatched);
  current code has 3 (no Columns region)
- **ViewCriteriaRow**: spec includes `join_is_or` and `depth` fields; current
  struct has only `sign` and `category_id`

When writing TUI code, check `lib.rs` for the actual struct definitions rather
than trusting the spec shapes verbatim. File a feature request if you find a
gap that matters.

## Category Manager Details Pane Keybinding Conflict (Tree Editor Rewrite)

In the rewritten category manager (`c` / `F9`), the Details pane uses `j/k` for
field navigation. The Note field also supports direct typing without pressing
`Enter` first, which creates a keybinding conflict for lowercase `j`/`k`.

Current implementation behavior (intentional):
- When the Note field is focused, printable keys (including `j` and `k`) start
  note editing and type into the note.
- Use `Up/Down` to move away from the Note field without typing.
- `H/J/K/L` structural move/reorder keys are disabled while the Details pane is
  focused, and only work when the Tree pane is focused.
