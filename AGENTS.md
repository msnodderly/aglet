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

`agenda-cli list --category` supports repeated flags with AND semantics.
For example, `agenda-cli list --category High --category Pending` returns items
that have both categories.

`agenda-cli list --any-category` supports repeated flags with OR semantics.
For example, `agenda-cli list --any-category Aglet --any-category NeoNV` returns
items that have either category.

`agenda-cli list --exclude-category` supports repeated flags with NOT semantics.
For example, `agenda-cli list --exclude-category Complete` removes completed
status/category matches from results.

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

## Aglet Features Database

`aglet-features.ag` in the project root tracks feature ideas and requests for
aglet itself (distinct from `feature-requests.ag`). Categories:

- **Issue type** (non-exclusive): Bug, Idea, Feature request
- **Priority** (exclusive): Critical, High, Normal, Low
- **Software Project / Software Projects**: Aglet, NeoNV
- **Status** (exclusive): Complete, In Progress, Next Action, Ready,
  Waiting/Blocked

Every item should have Issue type, Priority, Software Project(s), and Status.

Surprising gotcha: the parent category name in `aglet-features.ag` may be
`Software Projects` (plural) instead of `Software Project` (singular). If a
command errors with `category not found`, run `category list` and use the exact
name from that DB.

## `agenda-cli show` status vs Status category (Surprising)

In `aglet-features.ag`, assigning workflow categories like `Ready`,
`In Progress`, or `Complete` does not change the top-level `status:` field shown
by `agenda-cli show`; it can still print `status: open`.

Treat the `assignments:` section as the source of truth for workflow status
categories in this DB.

### Creating a feature request via CLI

Use the create-then-assign pattern. `add` prints the UUID, then assign
categories individually with the full UUID:

```bash
# 1. Create the item and extract the UUID from the "created ..." line
item_id=$(cargo run --bin agenda-cli -- --db aglet-features.ag add "Title here" --note "Description..." 2>&1 | awk '/^created /{print $2; exit}')

# 2. Assign categories (use full UUID)
cargo run --bin agenda-cli -- --db aglet-features.ag category assign "$item_id" "Feature request" 2>&1 | tail -1
cargo run --bin agenda-cli -- --db aglet-features.ag category assign "$item_id" Aglet 2>&1 | tail -1
cargo run --bin agenda-cli -- --db aglet-features.ag category assign "$item_id" Normal 2>&1 | tail -1
cargo run --bin agenda-cli -- --db aglet-features.ag category assign "$item_id" Ready 2>&1 | tail -1
```

Quote category names that contain spaces (e.g., `"Feature request"`,
`"In Progress"`, `"Next Action"`, `"Waiting/Blocked"`).

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

**`add` output parsing gotcha.** `agenda-cli add` can print additional lines
after `created <uuid>` (for example `parsed_when=...`). Do not assume the last
line is always `created ...`; extract the ID by matching the `^created ` prefix.

**`claim` race gotcha.** A candidate can appear in an open-item list but still
fail claim with `claim precondition failed ... already has category 'In Progress'`
if another agent claims it between your list step and claim step. If this
happens, re-run selection and claim the next eligible item; do not force-assign.

**Item ID prefix matching works.** You can use the first 8 hex characters of a
UUID instead of the full ID:

```bash
# These are equivalent:
cargo run --bin agenda-cli -- --db feature-requests.ag category assign be6f0754 High
cargo run --bin agenda-cli -- --db feature-requests.ag category assign be6f0754-a764-40ee-bb48-0bfc225b174b High
```

## Item ID Prefix Matching (Stale Guidance)

The CLI currently parses item IDs with `Uuid::parse_str`, so short UUID prefixes
like `be6f0754` do **not** work for item commands (for example `show`, `edit`,
and `category assign`). Use the full UUID until prefix resolution is
re-implemented in the CLI parser.

**Create-then-assign pattern.** Capture the ID from the `created ...` line, then
assign categories with `&&`-chained commands:

```bash
item_id=$(cargo run --bin agenda-cli -- --db feature-requests.ag add "My item" --note "..." 2>&1 | awk '/^created /{print $2; exit}')
# Then assign:
cargo run --bin agenda-cli -- --db feature-requests.ag category assign "$item_id" High 2>&1 | tail -1
cargo run --bin agenda-cli -- --db feature-requests.ag category assign "$item_id" Pending 2>&1 | tail -1
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

## `agenda-cli edit --note-stdin` Semantics (Surprising)

`agenda-cli edit <ITEM_ID> --note-stdin` replaces the entire note with stdin
content and is mutually exclusive with `--note`, `--append-note`, and
`--clear-note`.

Practical implications:
- Passing `--note-stdin` with any other note-operation flag returns a validation
  error.
- Empty stdin payload is a no-op (note content is preserved).
- Useful shell form: `printf "line one\nline two\n" | cargo run --bin agenda-cli -- --db <db> edit <ITEM_ID> --note-stdin`

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

## ViewEdit Criteria Row Ordering (Surprising Bug)

In the View Editor details pane, be careful not to sort a display-only copy of
criteria rows unless you also preserve a mapping back to the underlying draft
criteria vector. `state.criteria_index` edits the draft vector by index, so a
sorted rendered list can make the highlighted row and edited row diverge.

If you change ViewEdit criteria rendering, keep row order stable (draft order)
or explicitly carry source indices through render + input handling.

## View Columns Storage vs CLI Display (Surprising)

Board/table columns are stored on `View.sections[*].columns` (serialized inside
`views.sections_json`). The `views.columns_json` column is a legacy field and is
ignored by current `Store::row_to_view`.

Related gotcha: CLI `agenda-cli view show` prints section item tables but does
not render section column definitions at all, so board-column changes are only
visible in the TUI today.

## View Category Aliases Storage (Surprising)

View-level category display aliases are stored in `views.category_aliases_json`
as a map of `CategoryId -> alias` and are treated as display metadata only.

Do not apply these aliases to category identity, query/filter behavior, section
titles, generated subsection labels, or board column headings unless a separate
feature explicitly requests that behavior.

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

## Category Manager `<<` / `>>` Level Shift (Tree Editor)

Category Manager now supports Vim-style level shifts:
- `>>` indents the selected category under its previous sibling
- `<<` outdents the selected category to its parent's level

Behavior details:
- The first `<` or `>` arms the action; the second matching key applies it.
- Any non-`<`/`>` key clears the pending shift prefix.
- `<<` / `>>` are disabled while the Details pane is focused (same as `H/J/K/L`).

## Category Manager Action/Filter Cursor Rendering (Surprising)

Category Manager has multiple inline text-entry states in the top
Action/Filter pane (global filter editing, inline rename). These modes do not
use footer/input-panel cursor logic.

If you add or refactor Category Manager render code, explicitly position the
terminal cursor for these Action/Filter editing states; otherwise text editing
still works but the caret appears missing/intermittent.

## Category Create Parent Defaults (Surprising)

CategoryCreate (`Mode::InputPanel` with `NameInputContext::CategoryCreate`) no
longer has a parent-picker menu.

Practical implications:
- Parent is set when opening CategoryCreate (`n` uses selected category as the
  default parent when allowed, otherwise root).
- InputPanel focus cycle for CategoryCreate is now `Text -> Type -> Save ->
  Cancel` (no Parent focus row).
- To change hierarchy after create, use Category Manager structural moves
  (`H/J/K/L` and `<<`/`>>`).

## Item Assign Search Enter Behavior (Surprising)

In `Mode::ItemAssignInput` (`a`/`u` picker, then `n` or `/`), `Enter` resolves
category names with this precedence:

1. Exact existing category name match (case-insensitive)
2. If no exact match, and the typed query matches exactly one visible category
   row, auto-select that category
3. Otherwise create a new category from the typed text

This avoids accidental category creation when there is a single clear match,
while preserving exact-match and create-new behavior.

## Esc One-Step Exit Semantics (Surprising)

`Esc` now exits several editing panes in a single step and discards dirty
changes immediately (no hidden discard-confirm sub-state):

- `Mode::InputPanel` (`AddItem`/`EditItem`/name/numeric/category-create panels)
- `Mode::NoteEdit`
- `Mode::ViewEdit`

Practical implications:
- Do not reintroduce hidden booleans like `*_discard_confirm` for these flows.
- Footer/status hints should reflect one-step exit behavior (no `y/n/Esc` prompt).
- In InputPanel category-filter editing, `Esc` now closes filter editing directly
  and keeps the typed filter text (it no longer does clear-then-exit).

## Board Table Column Spacing Budget (Surprising)

Board rendering uses ratatui `Table::column_spacing` for inter-column gaps. If
you increase spacing above zero (for readability), you must subtract spacing
budget from width calculations (`compute_board_layout` and
`board_column_widths`) or total column widths can exceed slot width and visually
collapse adjacent values.

Related gotcha: dynamic board rendering can append a synthetic "All Categories"
column. If that synthetic column is enabled, reserve one additional spacing
slot; otherwise the synthetic column can consume the separator budget and defeat
the minimum visible gap guarantee.

## Done Toggle Blocker-Cleanup Prompt Uses `Mode::ConfirmDelete` (Surprising)

The TUI prompt shown when marking an item done that currently blocks other
items reuses `Mode::ConfirmDelete` with additional `App.done_blocks_confirm`
state instead of introducing a separate mode.

Practical implications:
- `handle_confirm_delete_key` now multiplexes two flows:
  item deletion (legacy) and done-with-blocker cleanup confirmation.
- Opening delete confirm (`x`) should clear `done_blocks_confirm` first so stale
  done-state prompts do not leak into delete behavior.
- Footer status/hints for `Mode::ConfirmDelete` are dynamic; if you touch
  confirm UI copy, update both delete and done-cleanup branches.

## View-Edit Tests Must Not Assume `views[0]` Is Editable (Surprising)

`Store::open` includes the immutable system view `All Items`. Depending on
ordering, `All Items` may be the first row returned by `list_views()` and the
first element in `app.views`.

Practical implications for tests:
- Do not use `app.views[0]` or `list_views().next()` when a test needs to edit
  or save a view.
- Select a named mutable view explicitly (for example `TestView` or
  `Work Board`) before calling `open_view_edit` or invoking ViewPicker edit
  keys.
- If a test unexpectedly stays in `Mode::ViewPicker` with an "immutable" status
  message, verify it did not accidentally target `All Items`.

## Preview Info Pane Scroll Clamp Must Use Rendered Line Count (Surprising)

Normal-mode preview `Info` now includes metadata and link-summary lines in
addition to assignment provenance rows.

Practical implications:
- Do not clamp `preview_provenance_scroll` using only
  `inspect_assignment_rows_for_item(item).len()`.
- Clamp against the full rendered info line count (header + metadata + links +
  assignment rows/fallback line) or users will be unable to scroll to all
  info content.

## Add-Item Context Text Must Use Fixed Help Row (Surprising)

In the InputPanel add-item flow, rendering `preview_context` (for example
`Adding to "Unassigned"`) on the same line as `Text>` causes the helper text to
appear to "float" as the cursor moves/typing changes line content.

Practical implications:
- Keep add-item context in the fixed help/status row (`regions.help`) and
  include destination + auto-assign count.
- Do not render add-item context near the text cursor row; numeric-value panels
  may still use the context row near the value input.
- Preserve a regression test that asserts the `Text>` line does not contain the
  `Adding to ...` context string.

## Normal Mode Enter On Empty Slot Opens Add Item (Behavior)

In `Mode::Normal`, `Enter` on the item column has dual behavior:
- If an item is selected, it opens item edit (`InputPanel(EditItem)`).
- If no item is selected (for example an empty section row), it opens add item
  (`InputPanel(AddItem)`) for the current slot context.

Do not regress this back to a "No selected item to edit" status in empty-slot
flows; tests cover the add-item fallback.

## Link Wizard Target List Must Be Stateful To Scroll (Surprising)

In `Mode::LinkWizard`, rendering target matches with a plain `List` plus manual
`>` markers does not keep the selected row visible when navigating deep lists.
Selection index updates continue, but the viewport stays pinned at top rows.

Practical implications:
- Render target matches with `render_stateful_widget` and `ListState`
  (`list_state_for`) using the selected index.
- Keep/restore a visual selection style (`highlight_symbol`/`highlight_style`)
  so users can see which target is active.
- If you change this path, ensure tests cover both filtering and deep-list
  navigation visibility in the wizard popup.

## Generated Section Insert Must Preserve Base Criteria (Surprising)

When inserting/removing items through `SlotContext::GeneratedSection` (for
example sections expanded via `show_children`), do not construct a synthetic
section with empty criteria for `insert_item_in_section`.

Practical implications:
- Generated-slot inserts must preserve the backing section criteria so
  `Agenda::insert_item_in_section` can apply section/view `And` criteria
  assignments.
- Regression example: a `Ready` section with `show_children=true` and no child
  categories renders `Ready (Other)` as a generated slot; adding there must
  still assign `Ready`.

## `show_children` With No Children Falls Back To Base Section (Behavior)

If a section enables `show_children=true` but the parent category currently has
no child categories, Aglet does not generate a synthetic `(Other)` subsection.
It renders the base section as a normal section instead.

Practical implications:
- Expect `SlotContext::Section` (not `GeneratedSection`) in this case.
- Add-item flows still apply section criteria assignments (for example `Ready`)
  through the normal section insert path.
