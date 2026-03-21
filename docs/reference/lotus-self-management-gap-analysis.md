# Lotus Self-Management Gap Analysis

Date: 2026-03-21
Source material: `~/src/lotus_agenda_applied_self_management.pdf`
Scope: adapt the Lotus "Applied Self Management Using Lotus Agenda" tutorial to Aglet's current CLI/TUI without reproducing the Lotus datebook flow.

## Supported adaptation

The attached demo in [docs/demos/lotus-self-management-demo.md](/Users/mds/src/aglet/docs/demos/lotus-self-management-demo.md) covers the parts of the tutorial that map cleanly to Aglet today:

- project-planning tasks
- people assignment via categories
- saved views with sections and columns
- dependency hiding for a "ready" slice
- numeric values and section summaries for a small budgeting appendix

The demo intentionally skips Lotus datebook-specific walkthroughs such as:

- `Schedule` in Planning Projects (PDF topic 4)
- `Time Log` and `Weekly Tasks` datebook views (PDF topic 5)
- the quarter-by-week datebook `Budget` view (PDF topic 8)

## Concrete acceptance criteria

- A Showboat-backed demo exists at [docs/demos/lotus-self-management-demo.md](/Users/mds/src/aglet/docs/demos/lotus-self-management-demo.md).
- The demo creates a fresh temp database and verifies current CLI flows for items, categories, views, sections, columns, dependencies, and numeric summaries.
- The demo excludes datebook-only steps and explains the adaptation in prose.
- This document records the assignment-condition gaps and the friction encountered while following the PDF.
- Missing assignment-condition flows are described with hypothetical CLI and TUI UX mockups.

## Assignment-condition flows that cannot be completed today

Two PDF exercises depend on user-authored assignment conditions:

1. Topic 5, "Task List" (page 5.9)
   The tutorial asks for a condition on `Routine` so that items assigned to `Routine Tasks` are also assigned to `Routine`.
2. Topic 6, "Setting Goals" (page 6.3)
   The tutorial asks for a condition on `Goal` so that items assigned to `Goals` are also assigned to `Goal` under `Task Type`.

### Current state

Aglet's engine and model already have profile conditions, and `agenda-cli category show` can display them if they exist. The missing part is the authoring flow:

- no CLI command adds, edits, or removes profile conditions
- no TUI category-manager workflow exposes condition editing
- no user-facing flow exists for reviewing or re-running condition changes against affected items

Relevant current code paths:

- [crates/agenda-core/src/model.rs](/Users/mds/src/aglet/crates/agenda-core/src/model.rs)
- [crates/agenda-core/src/engine.rs](/Users/mds/src/aglet/crates/agenda-core/src/engine.rs)
- [crates/agenda-cli/src/main.rs](/Users/mds/src/aglet/crates/agenda-cli/src/main.rs)
- [crates/agenda-tui/src/render/mod.rs](/Users/mds/src/aglet/crates/agenda-tui/src/render/mod.rs)

### Missing features

- Category-condition CRUD in the CLI.
- Category-condition CRUD in the TUI Category Manager.
- A user-facing way to preview affected items before saving a new rule.
- A documented post-change reevaluation/cleanup story for derived assignments that are no longer valid.

### Why this blocks the Lotus workflow

Without condition authoring, the user can still manually assign categories, but they cannot reproduce the Lotus tutorial's "configure once, then let the database classify future items" behavior for `Routine` and `Goal`.

That means the core "hierarchy as program" lesson from the PDF is only partially represented in the current product.

## Hypothetical UX mockups

### Mock CLI flow

```bash
# Make "Routine" auto-assign when an item has "Routine Tasks"
agenda-cli category condition add Routine --if-assigned "Routine Tasks"

# Make "Goal" auto-assign when an item has any child of "Goals"
agenda-cli category condition add Goal --if-assigned Goals

# Inspect the result
agenda-cli category show Routine
agenda-cli category show Goal
```

Expected UX shape:

- `condition add`, `condition list`, `condition remove`, and `condition clear`
- condition syntax aligned with existing include/exclude query semantics
- save output reports `processed_items` and `affected_items`

Example `category show` rendering:

```text
conditions:
  - Profile (and=[Routine Tasks], not=[], or=[])
```

### Mock TUI flow

In `Category Manager`:

```text
Category: Routine

[ ] Exclusive
[ ] Auto-match
[x] Actionable

Conditions
> 1. Assigned to: Routine Tasks

n:new  e:edit  d:delete  p:preview matches  S:save
```

For `Goal`, the flow would be the same:

```text
Category: Goal

Conditions
> 1. Assigned to: Goals
```

The minimum viable interaction would be:

- open the selected category's `Conditions` panel
- add one profile condition using category pickers
- preview matching items before saving
- save and automatically reevaluate affected items

For a step-by-step TUI version of the first missing Lotus exercise, see [lotus-self-management-tui-hypothetical-walkthrough.md](/Users/mds/src/aglet/docs/reference/lotus-self-management-tui-hypothetical-walkthrough.md).

## What to add for full end-to-end support

This section answers a stricter question than "what is missing right now?"

It answers: what should Aglet add so the full Lotus self-management walkthrough can be completed in a way that feels native and low-friction in both CLI and TUI?

### CLI additions

#### P0: required for full coverage

- Category-condition authoring commands.
  Example shape:
  `agenda-cli category condition add|list|remove|clear`
- A user-facing reevaluation command.
  Example shape:
  `agenda-cli classify recalc --all`
  `agenda-cli classify recalc --item <id>`
  `agenda-cli classify recalc --category Routine`
- User-defined date category support.
  Example:
  `agenda-cli category create Finish --type date`
- View type authoring for `standard` vs `datebook`.
- Datebook view options:
  `--period`, `--interval`, `--start-category`, `--end-category`
- Saved sort authoring on views.
- Column date-format configuration.
  Example:
  `agenda-cli view column update Tasks 0 When --date-format relative`

#### P1: needed for natural, not just possible, workflow

- Structured item creation with category/date/value fields in one command.
  Example:
  `agenda-cli add \"Pay rent\" --assign \"Routine Tasks\" --when next-week --value Amount=-1200`
- Better negative numeric handling so `-1200` does not require `--`.
- Category short-label or display-label authoring for Lotus-like `A/B/C` priority display.
- View templates or recipe commands for common Lotus patterns:
  `task-list`, `task-assignments`, `schedule`, `time-log`, `budget`
- Preview mode for category-condition changes before commit.
  Example:
  `agenda-cli category condition add Routine --if-assigned \"Routine Tasks\" --dry-run`

### TUI additions

#### P0: required for full coverage

- Category Manager condition editor.
- Category Manager preview of affected items before saving rules.
- Explicit reevaluation affordance for item/category/database scope.
- Date category creation/editing in Category Manager.
- View editor support for datebook views and their period/interval settings.
- View editor support for saved sort configuration.
- View editor support for per-column date formatting.

#### P1: needed for natural Lotus-style workflow

- Spreadsheet-like direct entry in visible columns for common fields:
  `People`, `When`, `Finish`, `Amount`, `Prty`
- Fast in-place creation of child categories while editing a view or category cell.
- Category short-label editing in Category Manager.
- View creation templates or guided wizards for `Task List`, `Schedule`, `Time Log`, and `Budget`.
- A side-by-side "rule result" preview after saving a condition so users can confirm that the right items changed.

### Recommended split: possible vs natural

If the goal is only "make the demo possible," implement the P0 items first.

If the goal is "make Aglet feel like a modern Lotus successor," the P1 items matter just as much because the original tutorial assumes:

- low-friction row entry
- fast category programming
- immediate view reshaping
- datebook-oriented planning flows

### Smallest credible milestone

The smallest milestone that materially changes the Lotus adaptation story is:

- CLI condition CRUD
- TUI condition CRUD
- reevaluation command/workflow
- date category creation
- datebook view authoring

That milestone would let Aglet stop saying "we can only demo the view half" and start supporting the tutorial's category-programming half as well.

## Friction log while following the PDF

### 1. Lotus starts from seeded structure; Aglet starts mostly empty

The PDF repeatedly assumes an initial database with `Initial Section` and `Initial View` already present. Aglet CLI starts from an empty database plus reserved categories, so each exercise has to be translated into explicit `category create`, `view create`, `view section add`, and `view column add` commands.

### 2. Lotus uses inline cell editing; Aglet CLI is command-driven

The tutorial relies on typing directly into columns like `People`, `When`, `Finish`, and `Amount`. In the CLI, the equivalent flow is split across:

- `add`
- `category assign`
- `edit --when`
- `category set-value`

This is workable, but materially more verbose than the PDF's interaction model.

### 3. User-defined date categories are not available in the CLI

Topic 4 asks for a `Finish` date category. Current CLI category types are `tag` and `numeric`; there is no user-facing "date category" create flow beyond the reserved `When`/`Done`/`Entry` categories.

Result: the demo uses `When` only and drops the separate `Finish` field.

### 4. Datebook configuration is not available in the current CLI authoring flow

The PDF leans heavily on datebook views:

- monthly `Schedule`
- `Time Log`
- `Weekly Tasks`
- quarterly `Budget`

Current CLI view authoring exposes include/exclude criteria, sections, columns, summaries, and dependent-item hiding, but not:

- view type selection (`standard` vs `datebook`)
- datebook period/interval configuration
- datebook end-category selection

This is the main reason the issue explicitly skips the Lotus datebook flow.

### 5. Column date formatting is thinner than the PDF expects

The PDF asks for relative date formatting and width tuning on date columns. Current CLI column authoring supports:

- column kind
- width
- summary

There is no per-column date-format setting in the CLI flow used for this demo.

### 6. Persistent view-sort configuration is missing from the authoring flow

Topic 5 asks for `Weekly Tasks` sorted by `Prty` and `No`. Aglet can sort on `list`/`view show`, but the current CLI authoring flow does not expose saved sort settings on the view object itself.

### 7. Category short-name workflows from Lotus are missing

The PDF asks for short names `A`, `B`, and `C` for priority children. Aglet currently supports view aliases as display metadata, but not Lotus-style category short-name editing as part of core category authoring.

### 8. Negative numeric values require a CLI parsing workaround

The budgeting section expects expenses to be negative. In Aglet CLI, negative values must be passed after `--`:

```bash
agenda-cli category set-value <id> Amount -- -1200
```

Without `--`, Clap interprets `-1200` as a flag and rejects the command.

### 9. The PDF's "force re-execute all assignments" step has no direct current analog

Topic 5 asks the user to force Agenda to re-run assignments after adding a condition. Because condition authoring is missing, the exact flow cannot be tested end to end. More broadly, there is no dedicated CLI command named around "reevaluate all assignments" for user-authored rules.

## Bottom line

Aglet can already demonstrate a strong slice of the Lotus self-management tutorial, but it currently demonstrates the "view and categorize work" half better than the "configure reusable assignment logic" half.

To close the remaining gap, the next product step is not more demo polish. It is shipping first-class condition authoring and reevaluation UX in both CLI and TUI.
