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

## `agenda-cli view clone` Semantics

`agenda-cli view clone "<source>" "<new name>"` creates a new mutable view by
copying the source view configuration (criteria, sections, unmatched settings,
aliases, and display metadata) with a fresh ID.

Practical implications:
- Cloning **does not mutate** the source view (including immutable system views
  like `All Items`).
- Target-name validation still uses create rules; reserved system target names
  (for example `All Items`) are rejected.

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

Dependency-state filtering is available via derived flags (not assignable
categories):
- `agenda-cli list --blocked` / `--not-blocked`
- `agenda-cli search <query> --blocked` / `--not-blocked`
- `agenda-cli view show "<name>" --blocked` / `--not-blocked`

`blocked` means the item has at least one unresolved `depends-on` prerequisite.
This state is computed from links + done state at query time.

## Claim Workflow Uses Dependency State But Is Not A Link Type (Current)

The workflow-backed claim flow (`agenda-cli ready`, `claim`, `release`) uses
dependency state as one input, but it is a **separate concept** from item
links.

Current claimability is computed as:
- item has the configured Ready category
- item does not already have the configured claim-target category
- item is not done
- item is not dependency-blocked by unresolved `depends-on` links

Practical implications:
- Use `link depends-on` / `link blocks` to model real prerequisite or ordering
  relationships between items.
- Use `claim` / `release` to reserve or unreserve an otherwise-claimable item
  for active work.
- Do **not** create synthetic `blocks` / `depends-on` links just to mean
  "someone is working on this"; that is what the claim-target category is for.
- Claiming does not create/remove links, and linked blocked/unblocked state
  should stay derivable from dependency graph + done state.

## Category Assignment in Items

When viewing items, the categories list includes both the assigned category and
all its parent categories. For example, an item assigned to `High` will show
`High, Priority` in its categories list (both the child and the parent).

## Exclusive Family Order Now Defines Derived Precedence (Current)

For an exclusive parent category, child order is now meaningful for
rule-derived conflicts.

Practical implications:
- If multiple derived children under the same exclusive parent match, the
  earlier child in the parent's ordered child list wins.
- Later derived siblings are suppressed instead of replacing the earlier match.
- Manual and accepted-suggestion assignments still act as durable user choices
  and should not be replaced by later derived siblings.
- This is a generic exclusive-family rule, not a special case for categories
  named `Priority` or `Status`.

## Implicit String Matching Uses Note Text Too (Surprising)

Implicit-string auto-match is evaluated against `item.text` and the full note
body, not only the title.

Practical implications:
- Example commands or acceptance criteria inside notes can accidentally match
  categories like `Ready`, `CLI`, or `TUI`
- If an item appears in a status/project view unexpectedly, inspect
  `agenda-cli show` assignment provenance before assuming the visible category
  was manually assigned

## Continuous Implicit-String Matches Persist As `AutoClassified` (Surprising)

The modern continuous classification path stores substring/category-name matches
as `AssignmentSource::AutoClassified` with provider
`implicit_string`, not as the older `AssignmentExplanation::ImplicitMatch`
shape used by some lower-level engine/tests.

Practical implications:
- In TUI/CLI provenance, a category that feels like "auto-match from item text"
  may arrive via `AutoClassified { provider_id: "implicit_string", ... }`
  rather than `ImplicitMatch`.
- If you are adding user-facing badges or explanations, normalize both forms to
  the same UX concept instead of assuming only `ImplicitMatch` represents
  name/text matching.
- When a test expects "matched category name ..." behavior, inspect the
  explanation/provider before deciding whether the wrong path fired.

## Disabling Implicit Match Only Evicts Live `AutoMatch` Assignments (Surprising)

Turning a category's `enable_implicit_string` flag off and re-running category
evaluation now removes **live/non-sticky** implicit-string assignments for that
category, but it still does **not** retroactively evict older sticky derived
assignments created before dynamic conditions were introduced.

Practical implications:
- Engine/evaluate-all paths now reconcile non-sticky `AssignmentSource::AutoMatch`
  rows and remove them when the category no longer matches.
- Compatibility is intentionally mixed: newly created implicit/profile-derived
  assignments are live, while legacy sticky derived assignments remain until
  explicitly cleared.
- If a category still appears assigned after disabling `Auto-match`, inspect
  `sticky`/provenance before assuming the new dynamic behavior is broken.

## Some Older Proposal Docs Still Describe Pre-Live Rule Semantics (Surprising)

Some planning/proposal docs still describe an older simplification where "all
assignments are sticky" and condition-derived assignments never auto-break.

Practical implications:
- Treat the shipped engine/tests as source of truth: new implicit/profile
  assignments are live and can auto-break; action/manual/accepted assignments
  remain sticky.
- If a proposal doc disagrees with current behavior, update the doc before
  using it as implementation guidance.

## `Agenda::unassign_item_manual` Reprocesses After Removal (Updated)

`Agenda::unassign_item_manual(...)` now mirrors other manual assignment flows:
after validating descendant constraints and removing the explicit assignment
row, it immediately reprocesses the item.

Practical implications:
- Live profile and subsumption assignments can auto-break immediately after a
  manual unassign, including from the TUI `a` item-assign picker.
- Older tests or callers that manually invoked reprocessing after
  `unassign_item_manual(...)` may now be doing redundant work.

## Adding A Category Action Does Not Retroactively Fire It (Surprising)

Updating a category to add or edit an `Action::Assign` / `Action::Remove`
definition reprocesses items for category-change bookkeeping, but it does **not**
retroactively execute that action for items already assigned to the owning
category.

Practical implications:
- Treat category actions as event-driven "on assignment" behavior, not as
  destination-style live conditions.
- If you add an action to `Escalated`, existing items already in `Escalated`
  will not immediately gain/remove the target categories just because the
  action was added.
- Do not write tests that assume `agenda.update_category(...)` or action-authoring
  commands will backfill action effects across historical assignments unless we
  intentionally change that semantic later.

## CLI And TUI Search Semantics Are Centralized In `agenda-core` (Updated)

CLI `agenda-cli search <query>` and the TUI per-lane `/` search now both route
through `agenda_core::query::matches_text_search(...)`.

Practical implications:
- Do not reintroduce TUI-local search helpers for text/note/UUID/category-name
  matching; shared semantics live in `agenda-core`
- If search behavior changes, update `agenda-core` matcher tests first and then
  re-run both `agenda-core` and `agenda-tui` search-focused tests

## `view_edit2.rs` Was An Incremental Bridge, Not A Stable Boundary (Surprising)

The former `crates/agenda-tui/src/modes/view_edit2.rs` existed as an
incremental bridge during the unified ViewEdit rollout, not because cargo tests
needed a split implementation.

Practical implications:
- Treat the view editor as one feature rooted at
  `crates/agenda-tui/src/modes/view_edit/`
- Organize the code by responsibility (`picker`, `editor`, `inline`,
  `overlay`, `sections`, `details`, `state`), not by historical spillover
- If you need to extend view editing, prefer the feature module directory over
  reviving a sibling `view_edit2.rs` file

## Database Files

Aglet databases use the `.ag` extension and are SQLite files. The CLI accepts
`--db <path>` (or `AGENDA_DB` env var) to target a specific database.

The project has two binaries: `agenda-cli` and `agenda-tui`. Use
`cargo run --bin agenda-cli` or `cargo run --bin agenda-tui` to run them.

## Schema Version Drift Can Hide Missing Tables/Columns (Surprising)

Some existing `.ag` files can already be stamped with `PRAGMA user_version = 11`
but still be missing the current `classification_suggestions` table and its
indexes.

Another observed drift case: a local DB stamped with the current schema version
was missing the `views.empty_sections` column, causing TUI startup to fail when
the view query selected that column.

Practical implications:
- `Store::init()` now runs idempotent migrations on every open to repair
  missing columns like `views.empty_sections`, even when `user_version` already
  equals `SCHEMA_VERSION`.
- `SCHEMA_SQL` still only runs when `user_version < SCHEMA_VERSION`, so simply
  opening a DB may not recreate a completely missing table if the DB is already
  stamped current.
- If a DB reports version 11 but lacks `classification_suggestions`, patch the
  table/indexes idempotently with SQLite (or add a newer migration/version bump
  in code) instead of assuming `agenda-cli` open is sufficient.

## Documentation Layout

Active project docs live under `docs/` grouped by purpose. The full index is
in `docs/README.md`. Every doc file must have YAML frontmatter (see below).

### Directory taxonomy

- `docs/plans/` — implementation plans (`status: draft | active | shipped | abandoned`)
- `docs/decisions/` — decision records: accepted proposals + legacy decision logs
- `docs/specs/product/` — product spec (target.md = NLSpec), roadmap, gaps, tasks
- `docs/specs/tui/` — TUI-specific specs
- `docs/specs/proposals/` — design proposals (`status: draft | rejected | deferred`;
  accepted proposals move to `decisions/`)
- `docs/reference/` — durable reference docs (codebase walkthrough, comparisons)
- `docs/process/` — PM workflow, agent workflow
- `docs/demos/` — executable demos
- `docs/agents/handoff/` — session handoff logs (`YYYY-MM-DD-NNN-feature.md`)
- `docs/backlog/` — feature requests

### Frontmatter requirements

Every doc file must have YAML frontmatter with at minimum `title` and `updated`.

Plans add: `status` (draft|active|shipped|abandoned), `created`, optional `shipped`.
Proposals add: `status` (draft|accepted|rejected|deferred), `created`, optional
`decided` and `origin` (when moved to decisions/).

To check plan status, read the frontmatter `status:` field, not file mtime.

### Lifecycle rules

- **Plans** stay in `docs/plans/` permanently. Status is updated in place.
  When shipped: set `status: shipped` and add `shipped: YYYY-MM-DD`.
- **Proposals** that are accepted move to `docs/decisions/` and become decision
  records. Add an `origin:` field pointing back to the original proposal path.
  Rejected/deferred proposals stay in `docs/specs/proposals/`.
- **Archive** (`archive/`) is frozen pre-v0.6 material. Do not add new files there.

### Handoff doc-update step (required)

Every agent session must, before writing the handoff doc:
1. Update `status` of any plan touched this session (add `shipped:` date if shipped)
2. Write a decision record in `decisions/` for any non-trivial design choice made
3. Move accepted proposals to `decisions/` and update their status

## Aglet Features Database

`aglet-features.ag` in the project root is the canonical issue-tracking database
for aglet. The DB file is local-only and is not committed to git; create it
locally as needed (see `scripts/init-aglet-features-db.sh`). Categories:

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

## Claimed Items Can Show Stale `Status` Provenance Text (Surprising)

After `agenda-cli claim <ITEM_ID>` moves an item from `Ready` to the workflow
claim target (for example `In Progress`), `agenda-cli ready` correctly removes
the item from the queue and `agenda-cli show` shows the new claim assignment.
However, the `assignments:` section can still show a `Status | Subsumption`
explanation that says it was inherited from child `Ready`, even when `Ready`
is no longer listed as an active assignment.

Practical implications:
- Do not assume a successful claim failed just because `agenda-cli show`
  mentions `subsumption:Status` from `Ready`.
- Verify the actual claimed category assignment (`In Progress | Manual |
  manual:cli.claim`) and/or re-run `agenda-cli ready` to confirm the item left
  the queue.

## Edit Panel Category Checks Include Derived Assignments (Current)

In `agenda-tui` `Mode::InputPanel` edit-item flow, category checkboxes are
initialized from all current assignment keys, including derived sources
(`AutoMatch`, `Subsumption`), so Edit Item matches Assign/Column picker state.

Practical implications:
- If `agenda-cli show` reports `In Progress | AutoMatch | cat:In Progress`,
  `In Progress` should now appear checked in Edit Item categories.
- Category diffs in Edit Item are computed against the full current assignment
  key set (not just manual/action rows).

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
CLI="cargo run --bin agenda-cli -- --db aglet-features.ag"
$CLI list   # ERROR: command not found
```

Write the full command each time, or use `&&` to chain them:

```bash
cargo run --bin agenda-cli -- --db aglet-features.ag add "Title" --note "..." 2>&1 | tail -2
```

**`add` output parsing gotcha.** `agenda-cli add` can print additional lines
after `created <uuid>` (for example `parsed_when=...`). Do not assume the last
line is always `created ...`; extract the ID by matching the `^created ` prefix.

**`claim` race gotcha.** A candidate can appear in an open-item list but still
fail claim with `claim precondition failed ... already has category 'In Progress'`
if another agent claims it between your list step and claim step. If this
happens, re-run selection and claim the next eligible item; do not force-assign.

**Current claim CLI syntax is workflow-based.** The public commands are:

```bash
cargo run --bin agenda-cli -- --db aglet-features.ag ready
cargo run --bin agenda-cli -- --db aglet-features.ag claim <ITEM_ID>
cargo run --bin agenda-cli -- --db aglet-features.ag release <ITEM_ID>
# alias:
cargo run --bin agenda-cli -- --db aglet-features.ag unclaim <ITEM_ID>
```

Practical implications:
- Older examples showing `agenda-cli claim <ITEM_ID> --must-not-have ...` are
  stale; current `claim` accepts only the item id.
- Prefer `agenda-cli ready` when picking work; it already excludes done,
  claimed, and dependency-blocked items using workflow config.
- `agenda-cli view show "Ready Queue" --blocked` is invalid and
  `--not-blocked` is redundant because the Ready Queue already shows only
  claimable items.
- Marking an item done clears the configured claim assignment automatically; you
  do not need to `release` before completing it.

**Item ID prefix matching works.** You can use the first 8 hex characters of a
UUID instead of the full ID:

```bash
# These are equivalent:
cargo run --bin agenda-cli -- --db aglet-features.ag category assign be6f0754 High
cargo run --bin agenda-cli -- --db aglet-features.ag category assign be6f0754-a764-40ee-bb48-0bfc225b174b High
```

## Item ID Prefix Matching

The CLI supports short UUID prefix matching for all item commands (`show`,
`edit`, `category assign`, `claim`, `delete`, `link`, `unlink`). Any unique
hex prefix works (e.g., `d157` resolves to `d15772e9-b608-...`).

- Prefix is matched case-insensitively with hyphens stripped.
- Ambiguous prefixes return an error listing all matching full UUIDs.
- Full UUIDs still work unchanged.
- Only valid hex characters are accepted in prefixes.

**Create-then-assign pattern.** Capture the ID from the `created ...` line, then
assign categories with `&&`-chained commands:

```bash
item_id=$(cargo run --bin agenda-cli -- --db aglet-features.ag add "My item" --note "..." 2>&1 | awk '/^created /{print $2; exit}')
# Then assign:
cargo run --bin agenda-cli -- --db aglet-features.ag category assign "$item_id" Normal 2>&1 | tail -1
cargo run --bin agenda-cli -- --db aglet-features.ag category assign "$item_id" Ready 2>&1 | tail -1
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

## `cargo fmt` / `rustfmt` File-Scoped Runs Can Touch Sibling Modules (Surprising)

Running `cargo fmt --all -- <file>` in this workspace can still reformat other
Rust files in the same crate/module tree (not just the listed file), because
rustfmt follows `mod` declarations from the entry file.

Surprising follow-up observed in this repo: direct `rustfmt <entry-file>` can
also spill into sibling module files when the entry file declares `mod ...`.

Practical implications:
- After file-scoped fmt runs, always check `git status` for incidental
  formatting diffs outside your task scope.
- `rustfmt <file>` is not guaranteed to stay single-file here; still verify for
  spillover changes.

## `agenda-cli edit --note-stdin` Semantics (Surprising)

`agenda-cli edit <ITEM_ID> --note-stdin` replaces the entire note with stdin
content and is mutually exclusive with `--note`, `--append-note`, and
`--clear-note`.

Practical implications:
- Passing `--note-stdin` with any other note-operation flag returns a validation
  error.
- Empty stdin payload is a no-op (note content is preserved).
- Useful shell form: `printf "line one\nline two\n" | cargo run --bin agenda-cli -- --db <db> edit <ITEM_ID> --note-stdin`

## Direct SQLite `when_date` Imports Need Store Format + Do Not Sync `When` Assignment (Surprising)

If you write `items.when_date` directly with SQLite, the value must use the
store's exact datetime format: `YYYY-MM-DD HH:MM:SS`.

Practical implications:
- ISO-style strings like `2025-10-11T00:00:00` are present in SQLite but load as
  `when: -` in CLI/TUI because `Store::row_to_item` currently parses
  `"%Y-%m-%d %H:%M:%S"` only.
- Direct SQLite updates to `items.when_date` do **not** create/update the
  reserved `When` assignment or provenance rows; use agenda/CLI app logic if
  you need the item to carry a synced `When | Manual | ...` assignment in
  addition to the datetime value itself.

## TUI Spec vs. Implementation

`docs/specs/proposals/tui-ux-redesign.md` is the design target. The Phase 2a implementation
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

## View `hide_dependent_items` Semantics

View-level hide-dependent mode is persisted in `views.hide_dependent_items`
(`View.hide_dependent_items`, default `false`).

Practical implications:
- "Dependent/blocked" means an item has at least one unresolved `depends-on`
  link to an item that is **not done**.
- Done dependencies do not block.
- Current filtering is applied in CLI/TUI view rendering paths using link data;
  if you add another view consumer, wire this filter there too.

## View `section_flow` Storage + Horizontal Navigation (Behavior)

Per-view lane direction is now persisted in `views.section_flow`
(`View.section_flow`, default `Vertical`).

Practical implications:
- `Horizontal` flow renders sections left-to-right using compact card rows.
- In horizontal flow, `h/l` move between section lanes and `j/k` move within
  the current lane.
- Lane item selection is remembered per section in horizontal flow; moving away
  and back restores that lane's prior item index.

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

## InputPanel Note Cursor Requires Explicit Position (Surprising)

`Mode::InputPanel` note editing (`InputPanelFocus::Note`) does not automatically
show a terminal caret unless `render` sets cursor coordinates explicitly.

Practical implications:
- Keep `input_panel_cursor_position()` returning a cursor position for Note
  focus (line/column mapped into the note viewport with scroll clamp).
- If you only style the `tui-textarea` cursor but do not set terminal cursor
  position, cursor visibility can appear inconsistent across text-entry panes.

## Blocking Save Overlay Must Queue After Validation (Surprising)

The TUI "Working" popup for synchronous classification should only be queued
when semantic/Ollama classification is actually enabled **and** the current
input-panel contents have already passed local validation.

Practical implications:
- If you queue the blocking overlay before validating edit/add input, users can
  see a fast yellow flash and then remain in the editor with no obvious save.
- Preflight checks should cover at least empty item text and edit-panel `When` /
  numeric parse errors before scheduling the blocking UI action.
- Deterministic-only saves should not show the blocking overlay; reserve it for
  real semantic/Ollama work.

## Category Create Parent Defaults (Surprising)

CategoryCreate (`Mode::InputPanel` with `NameInputContext::CategoryCreate`) no
longer has a parent-picker menu.

Practical implications:
- Parent is set when opening CategoryCreate:
  `n` creates at the selected category's level (same parent, or root for
  top-level categories), while `N` creates a child of the selected category
  when allowed.
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

## Item Assign Picker Enter Must Not Re-Toggle Dirty Selection (Surprising)

In `Mode::ItemAssignPicker`, `Space` applies the category/view toggle
immediately. If the session is already dirty, `Enter` should close the picker,
not replay the current toggle a second time.

Practical implications:
- `Space` then `Enter` should keep the applied assignment and close the picker.
- Reusing `Enter` as "call the Space handler, then close" will accidentally
  undo the just-applied assignment for single-item category toggles.

## Esc Exit Semantics (Updated)

`Esc` now consistently means **cancel** across all editing surfaces:

- `Mode::InputPanel` `AddItem`/`EditItem`: Esc cancels. If the panel is dirty,
  a discard-confirm prompt appears (`y` save, `n` discard, `Esc` keep editing).
  If clean, the panel closes immediately.
- `Mode::InputPanel` single-field panels (`NameInput`, `WhenDate`,
  `NumericValue`, `CategoryCreate`): Esc cancels immediately (no confirm).
- `Mode::ViewEdit`: Esc cancels with dirty-confirm if changes exist.
- `Mode::CategoryDirectEdit`: Esc cancels immediately.

Practical implications:
- `Esc` never saves. Use `S` (capital) to save complex editors
  (AddItem/EditItem/ViewEdit/CategoryDirectEdit) or `Enter` for single-field
  panels and from the Text focus in AddItem/EditItem.
- The discard-confirm prompt uses the same `y/n/Esc` pattern as ViewEdit.
- Footer hints show `S:save  Esc:cancel` for complex editors and
  `Enter:save  Esc:cancel` for single-field editors.
- In InputPanel category-filter editing, `Esc` still closes filter editing
  directly and keeps the typed filter text.

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

## TUI Auto-Refresh Interval Is DB-Backed In `app_settings` (Surprising)

TUI auto-refresh mode (`off` / `1s` / `5s`) now persists per database in
`agenda-core` table `app_settings` under key `tui.auto_refresh_interval`.

Practical implications:
- `App::run` must load this setting at startup; do not assume `Off` default is
  always the active runtime value.
- `Ctrl-R` interval cycling must persist the new value immediately after
  changing it.
- Unknown/missing persisted values must safely fall back to `Off` (no panic,
  no invalid state).
- Keep coverage for migration/table-creation plus reopen roundtrip persistence.

## TUI Run Loop Must Not Redraw While Idle (Surprising)

`App::run` used to call `terminal.draw(...)` unconditionally before every
`event::poll(Duration::from_millis(200))`, which meant the main board view
re-ran Ratatui table/layout work about 5 times per second even when no input
or data changed.

Practical implications:
- Treat redraw as state-driven, not timer-driven: only draw after actual UI
  changes (key handling, resize, auto-refresh, background classification
  completion, transient-status expiry, etc.).
- Idle wakeups for polling/background work are fine; the expensive part was the
  unconditional board redraw path (`render_board_columns` /
  Ratatui table layout).
- If idle CPU spikes in Activity Monitor or `sample`, inspect
  `App::run` first for accidental unconditional draw regressions before
  optimizing render helpers.

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

## Clap `--help` Coverage Requires Per-Arg Doc Comments (Surprising)

In `agenda-cli`, Clap renders blank lines for options/arguments that have no
doc comment/help string, even when the command itself is documented.

Practical implications:
- Add explicit doc comments for every user-facing option and positional arg in
  parser enums (`Command`, `CategoryCommand`, `ViewCommand`, etc.).
- Keep a parser regression test that walks the command tree and fails when any
  non-`help` argument lacks help text (current test:
  `clap_help_docs_cover_all_commands_and_arguments`).
## Normal Mode Preview Hint Must Be In Footer (Discoverability)

`p` already toggles item preview in `Mode::Normal`, but discoverability depends
on `footer_hint_text()` in `crates/agenda-tui/src/render/mod.rs`.

Practical implications:
- Keep `p:preview` in both normal footer variants:
  with section filters (`Esc:clear search`) and without filters.
- If you edit normal footer hints, preserve preview discoverability and update
  rendering tests that assert `p:preview` is visible.

## Edit Item Inspector Is Popup-Only (Current)

`Mode::InputPanel` for `EditItem` no longer keeps an always-visible `Details`
pane in the main edit layout. Instead, `I` opens a separate read-only inspector
popup from non-text edit focus states.

Practical implications:
- Edit-item tab cycle is back to `Text -> When -> Note -> Categories -> Save -> Cancel`
- The inspector popup omits note text and note preview; keep note editing in the
  main edit panel and reserve the popup for metadata, links, and assignment
  provenance
- While the popup is open, `Esc` and `I` close it, and `j/k`, `PgUp/PgDn`,
  `Home`, and `End` scroll it
- If you change popup-open eligibility or footer hints, update
  `handle_input_panel_key`, edit-panel help text, and tests that cover the
  inspector flow

## Global Search `g/` Uses Temporary All-Items Session (Behavior)

In `Mode::Normal`, `g/` now starts a temporary global search session:
- Saves the current view context (view + slot/item/column focus + section filters)
- Switches to `All Items`
- Opens the search bar
- Applies the typed filter across **all slots** (not only the active slot)

Practical implications:
- While this session is active, `Esc` returns to the prior view context instead
  of only clearing the current slot filter.
- `Enter` exact-match resolution searches across all visible slots in `All Items`.
- Creating from global search (`g/` + query + `Enter`) must keep the session
  active through add/edit save flows so `Esc` can still return to the original
  view afterwards.
- Keep `ga` behavior unchanged; `g` prefix help/status should mention both
  commands (`ga` and `g/`).

## View Creation Wizard Defers Persistence Until `S` In ViewEdit (Behavior)

`ViewPicker` -> `n` now opens a name input, then enters `ViewEdit` with an
unsaved draft (`is_new_view=true`) after saving the name. The new view is not
written to the DB until `S` is pressed in `ViewEdit`.

Practical implications:
- Do not call `store.create_view()` in the name-input save path for
  `NameInputContext::ViewCreate`; open `ViewEdit` with a draft instead.
- `handle_view_edit_save` must branch: `create_view` for new drafts,
  `update_view` for existing views.
- Cancel paths (`Esc`/discard confirm) in new-view `ViewEdit` must not persist
  partial drafts.
- Initial wizard focus starts in inline section-title input; first `Esc` exits
  inline editing, then `Esc` again closes/cancels the wizard.

## NameInput Enter-Save Behavior (Current)

`InputPanelKind::NameInput` now saves on `Enter` from the text field directly
(same as numeric value panels).

Practical implications:
- NameInput-backed flows (for example board inline `When` editing and view-name
  create/rename) do not require tabbing to the Save button before Enter.
- Save/Cancel buttons still work normally for mouse/keyboard navigation flows.

## Inline When Validation Feedback Is Pane-Local (Surprising)

`When` inline editing now shows parse/validation feedback in the popup pane
itself (help/feedback row), not in the global footer status line.

Practical implications:
- Keep parse/validation failures visible in the `When` editor pane while it is
  open.
- Do not rely on footer status text for inline `When` feedback.
- Invalid `When` input must keep the panel open and show a clear parse error
  including the attempted text.
- Invalid `When` help text should list supported date forms and explicitly note
  that phrases like `last week` / `next week` are not supported yet.

## Inline When Editor Must Keep Full Item Context Visible (UX)

The board inline `When` editor uses a dedicated compact popup with a single
context line that should display the full item text (not a short truncated
label).

Practical implications:
- Do not pass `truncate_board_cell(...)` output as `When` editor context text.
- Keep `WhenDate` context as one concise line (`Item: ...`) to avoid vertical
  space bloat.

## Manual PR Review Session Workflow (Process)

1. Enumerate open PRs sorted by PR number and review sequentially.
2. For each PR, provide:
   - an intent + product-fit assessment
   - copy/paste smoke-test commands
   - expected pass/fail signals.
3. Use explicit decision gating per PR:
   - reviewer responds `accept <PR#>` or `reject <PR#>`
   - record a running decision log
   - work exactly one PR at a time; do not begin reviewing the next PR until the current PR has an explicit accept/reject decision
   - do not merge/close during the active review loop.
4. Operational guardrails:
   - prefer existing per-PR worktrees when `gh pr checkout` reports branch/worktree conflicts
   - avoid `set -e` / `set -o pipefail` in user-facing pasted commands
   - use deterministic temp-DB smoke scripts for CLI features and clean up seeded data.
5. Review quality bar:
   - verify both behavior and product/API shape (not only green tests)
   - call out concrete findings with severity and file/line references
   - separate blocking issues from follow-up issues.
6. Tracking hygiene:
   - keep accept/reject log visible throughout the session
   - search tracking DBs for existing matching feature items before creating new ones
   - complete existing items instead of creating duplicates when appropriate.
7. Finalization phase (when session ends):
   - run finalization steps strictly serially (never in parallel): merge/close actions must be one command at a time
   - fetch latest remote refs immediately before finalization (`git fetch origin`)
   - sync `main`
   - merge all accepted PRs in order
   - if conflicts occur, fetch again, merge current `origin/main` into each accepted PR branch, resolve, test, push, then merge
   - do not resolve conflicts against stale `origin/main`; always refresh refs first
   - close rejected PRs with a short comment
   - report remaining open PRs for the next session.
8. Finalization workspace hygiene:
   - prefer a clean integration worktree for finalization to avoid local dirty-file interference
   - if local `main` cannot fast-forward because of local modifications, do not force-reset; report the exact blocking files/paths and current `HEAD` vs `origin/main`
   - after each merge/close action, re-check PR state before moving to the next step.
