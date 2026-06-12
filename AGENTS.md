# AGENTS.md

This file is the always-loaded operating guide for agents working on Aglet.
Keep it short. Put narrow implementation notes in `docs/` or near the code and
link them from here only when they are broadly useful.

Aglet is greenfield. Local `.ag` databases are test data unless the user says
otherwise, so prefer getting the model and implementation right over preserving
backward compatibility with local fixtures.

When implementing a new user-visible feature that needs manual testing, finish
with a basic smoke-test procedure and include the current branch or worktree.

Prefer `bash` over `zsh` for user-facing reusable shell scripts unless there is
a specific need for `zsh`. After editing non-ephemeral user-facing bash scripts,
run `shellcheck` when available and fix actionable warnings.

## Project Basics

- Aglet databases use the `.ag` extension and are SQLite files.
- The CLI accepts `--db <path>` or `AGLET_DB`.
- The only user-facing binary is `aglet`.
- Use `cargo run --bin aglet -- ...` for CLI commands.
- Running `cargo run --bin aglet` without a subcommand opens the TUI.

Reserved categories cannot be modified or used as child category names:
`Done`, `When`, and `Entry`. If you need a workflow category under an exclusive
parent like `Status`, use names such as `Complete` or `Completed`, not `Done`.

## Documentation Rules

Active docs live under `docs/`; the full index is `docs/README.md`.

- `docs/plans/`: implementation plans with `status: draft | active | shipped | abandoned`
- `docs/decisions/`: accepted proposals and decision records
- `docs/specs/product/`: product target, roadmap, gaps, tasks
- `docs/specs/tui/`: TUI-specific specs
- `docs/specs/proposals/`: draft/rejected/deferred proposals
- `docs/reference/`: durable implementation/reference notes
- `docs/process/`: workflow/process docs
- `docs/demos/`: executable demos
- `docs/agents/handoff/`: session handoffs
- `docs/backlog/`: feature requests

Every doc file needs YAML frontmatter with at least `title` and `updated`.
Plans also need `status` and `created`; shipped plans add `shipped`.
Proposals also need `status` and `created`; moved decisions should record
`origin`.

Before writing a handoff doc:

1. Update touched plan statuses, adding `shipped:` when relevant.
2. Write a decision record for any non-trivial design choice.
3. Move accepted proposals to `docs/decisions/` and update their status.

`archive/` is frozen pre-v0.6 material. Do not add new files there.

## CLI Usage

`aglet list` and `aglet search` default to compact one-line rows: 8-char id
prefix, `open`/`done`, humane date, title, note marker, and direct leaf
categories. Use `--verbose` for the old multi-line human output and
`--format json` for scripts.

`aglet list` without `--view` uses `All Items` when present, then falls back to
the first stored view. `aglet view show "All Items"` is the clearest way to
inspect every item.

Short item-id prefixes work anywhere an item id is accepted (`show`, `edit`,
`category assign`, `claim`, `delete`, `link`, `unlink`). Prefix matching is
case-insensitive with hyphens stripped; ambiguous prefixes return an error with
matching full UUIDs.

Create-then-assign pattern:

```bash
item_id=$(cargo run --bin aglet -- --db ../aglet-features.ag add "Title" --note "Description" 2>&1 | awk '/^created /{print $2; exit}')
cargo run --bin aglet -- --db ../aglet-features.ag category assign "$item_id" "Feature request"
cargo run --bin aglet -- --db ../aglet-features.ag category assign "$item_id" Aglet
cargo run --bin aglet -- --db ../aglet-features.ag category assign "$item_id" Normal
cargo run --bin aglet -- --db ../aglet-features.ag category assign "$item_id" Ready
```

Do not assume the last line of `aglet add` is the created id; parse the
`^created ` line.

Do not put multiple bare `cargo run` commands on consecutive lines and expect
one final pipe to apply to all of them. Each command needs its own pipeline, or
chain explicitly.

Do not use shell variable shorthand for commands, for example:

```bash
CLI="cargo run --bin aglet -- --db ../aglet-features.ag"
$CLI list
```

Write the full command or use a shell function/script when needed.

## Feature Tracking DB

`../aglet-features.ag` (`/Users/mds/src/aglet-features.ag`) is the local
issue-tracking database for Aglet. It is not committed; create it locally with
`scripts/init-aglet-features-db.sh` when needed.

Expected categories:

- Issue type: `Bug`, `Idea`, `Feature request`
- Priority: `Critical`, `High`, `Normal`, `Low`
- Software project(s): `Aglet`, `NeoNV`
- Status: `Complete`, `In Progress`, `Next Action`, `Ready`, `Waiting/Blocked`

Every tracked item should have issue type, priority, project, and status
categories. The project parent may be named `Software Projects` rather than
`Software Project`; run `category list` if assignment fails.

In this DB, workflow categories such as `Ready`, `In Progress`, and `Complete`
do not change the top-level `status:` field printed by `aglet show`; check the
`assignments:` section for workflow state.

## Claim Workflow

Use the workflow-backed commands:

```bash
cargo run --bin aglet -- --db ../aglet-features.ag ready
cargo run --bin aglet -- --db ../aglet-features.ag claim <ITEM_ID>
cargo run --bin aglet -- --db ../aglet-features.ag release <ITEM_ID>
cargo run --bin aglet -- --db ../aglet-features.ag unclaim <ITEM_ID>
```

Claimability is not a link type. It is computed from:

- item has the configured Ready category
- item does not already have the configured claim-target category
- item is not done
- item is not blocked by unresolved `depends-on` links

Use `link depends-on` / `link blocks` for real prerequisites. Do not create
synthetic links just to mean "someone is working on this"; `claim` applies the
claim-target category for that.

Prefer `aglet ready` when picking work. It already excludes done, claimed, and
dependency-blocked items. If claiming races and fails because the item is
already `In Progress`, rerun selection and claim another item; do not
force-assign.

## Query And View Semantics

View `--include` filters are AND-based:

```bash
aglet view create "My View" --include High --include Pending
```

This requires both `High` and `Pending`. Do not use repeated includes for
mutually exclusive siblings such as `Pending` and `In Progress`; use separate
sections or views.

List/search category filters:

- `--category` repeats with AND semantics.
- `--any-category` repeats with OR semantics.
- `--exclude-category` repeats with NOT semantics.

Dependency-state filters are derived, not assignable categories:

- `aglet list --blocked` / `--not-blocked`
- `aglet search <query> --blocked` / `--not-blocked`
- `aglet view show "<name>" --blocked` / `--not-blocked`

`blocked` means the item has at least one unresolved `depends-on` prerequisite.
Done dependencies do not block.

View `section_flow` is persisted on `View.section_flow`; horizontal flow uses
left/right lane navigation and remembers per-lane selection. View-level
`hide_dependent_items` is persisted on `View.hide_dependent_items` and is
applied in CLI/TUI view rendering paths.

View-level category aliases live in `views.category_aliases_json` as
`CategoryId -> alias` display metadata. Do not apply aliases to category
identity, filters, section titles, generated subsection labels, or board column
headings without a feature explicitly requesting it.

## Category And Classification Semantics

Displayed category lists include assigned categories and parent categories. For
example, assigning `High` also displays `Priority`.

For exclusive parent categories, child order defines rule-derived precedence:
when multiple derived siblings match, the earlier child in parent order wins.
Manual and accepted-suggestion assignments remain durable user choices.

Implicit-string auto-match checks both item title and full note text. Example
commands or acceptance criteria inside notes can accidentally match categories
such as `Ready`, `CLI`, or `TUI`; inspect `aglet show` provenance before
assuming a visible category was manually assigned.

Continuous implicit-string matches can appear as
`AssignmentSource::AutoClassified` with provider `implicit_string`, not only as
older `AssignmentExplanation::ImplicitMatch`. Normalize both as the same UX
concept when presenting user-facing provenance.

New implicit/profile-derived assignments are live and can auto-break; manual,
action, and accepted-suggestion assignments remain sticky. Older proposal docs
may still describe the pre-live simplification where all assignments were
sticky; trust shipped code and tests first.

Turning off a category's `enable_implicit_string` evicts live/non-sticky
implicit assignments for that category, but does not retroactively remove older
sticky derived assignments. Check sticky/provenance before assuming a bug.

`Aglet::unassign_item_manual(...)` reprocesses the item after removing the
explicit assignment row, so callers generally do not need a second manual
reprocess.

Category actions are event-driven on assignment. Adding or editing an
`Action::Assign` / `Action::Remove` does not retroactively fire for items that
already had the owning category.

CLI and TUI text search both route through
`aglet_core::query::matches_text_search(...)`. Change matcher tests in
`aglet-core` first, then cover CLI/TUI behavior as needed.

## Store And Migration Notes

`Store::init()` runs idempotent repair on open for known drift, including
`views.empty_sections` and `classification_suggestions`, even when
`PRAGMA user_version` is already current. Current schema version is defined in
`crates/aglet-core/src/store.rs`.

If direct SQLite imports write `items.when_date`, use the store datetime format
`YYYY-MM-DD HH:MM:SS`. ISO strings such as `2025-10-11T00:00:00` will not load
as dates through current row parsing. Direct SQLite writes also do not sync the
reserved `When` assignment/provenance; use Aglet/CLI logic when that matters.

Running `cargo fmt --all -- <file>` or direct `rustfmt <entry-file>` can still
format sibling Rust modules because rustfmt follows `mod` declarations. Always
check `git status` afterward for incidental diffs.

## TUI Implementation Guide

Treat proposal docs as design background, not source of truth. For TUI code,
check current structs and handlers in `crates/aglet-tui/src/`.

Important current anchors:

- View editor code lives under `crates/aglet-tui/src/modes/view_edit/`.
- Current `ViewEditRegion` includes `Criteria`, `Datebook`, `Sections`, and
  `Unmatched`; there is no standalone Columns region.
- ViewEdit saves with `S`/Ctrl-S through `handle_view_edit_save`; `Enter`
  operates focused rows/inline inputs.
- Key documentation is single-sourced in `crates/aglet-tui/src/keymap.rs`.
  Help, footer hints, and the README keybinding table derive from it.
- Regenerate the README keymap with:
  `UPDATE_README=1 cargo test -p aglet-tui readme_keymap`

For narrow TUI regression notes, see `docs/reference/tui-implementation-notes.md`
before touching the relevant mode.
