---
title: CLI Structured Capture and View Authoring
status: draft
created: 2026-03-15
---

# CLI-Only Structured Capture and View Authoring

## Context

The current CLI is strong for basic item/category/view workflows, but it falls
short once capture becomes structured.

In the recent moto budget workflow, importing expense rows and building saved
budget views required direct SQLite writes even though the underlying Aglet
model already supports the needed concepts:

- item datetimes via `when_date`
- numeric category values
- saved views with multiple sections and configured columns
- category aliases and remove-from-view behavior

The problem was not missing model capability. The problem was missing CLI
surface area to exercise that capability cleanly.

Current CLI baseline:

- `agenda-cli add` supports item text plus optional `--note`
- `agenda-cli edit` supports text/note/done changes, but not explicit
  `when_date` editing
- `agenda-cli category set-value` supports numeric category assignment
- `agenda-cli view create` and `agenda-cli view edit` support only basic view
  creation and a small set of mutable properties
- `agenda-cli view set-summary` exists for section-column summaries
- richer view authoring operations are not exposed in CLI

This gap forced a fallback to direct SQL for both item dates and view shaping.
That fallback works, but it is brittle:

- direct `when_date` writes must use the store's exact datetime format
- direct writes bypass normal app-level `When` syncing/provenance behavior
- direct view JSON updates require internal storage knowledge instead of stable
  command-level affordances

The moto budget scenario is a useful motivating example, but the gap is broader
than expense tracking. Any structured import or sectioned-view authoring
workflow hits the same boundary.

## Goals

- Make common structured capture workflows possible via CLI only.
- Make common structured import workflows possible via CLI only.
- Make saved-view authoring possible via CLI only for sectioned and tabular
  views.
- Keep the proposal additive and composable with the existing command tree.
- Preserve current TUI workflows; this is CLI expansion, not a TUI replacement.

## Non-goals

- Changing the underlying item/category/view model.
- Replacing the TUI as the primary advanced editing surface.
- Introducing a daemon, sync service, or background import process.
- Requiring whole-object JSON editing as the primary CLI interface.
- Prescribing low-level implementation details beyond enough command-shape
  specificity to make the proposal actionable.

## Proposed CLI Surface

This section describes proposed CLI additions. These commands do not exist yet.

### 1. Explicit Item Date Support

Add first-class `when_date` editing to both capture and update flows.

Proposed commands:

```bash
agenda-cli add "DRZ Payment" --when 2025-12-11
agenda-cli add "Track day" --when "2026-02-20 09:00"
agenda-cli edit <ITEM_ID> --when 2025-12-11
agenda-cli edit <ITEM_ID> --clear-when
```

Notes:

- `--when` should accept either a date-only value or a full datetime value.
- `--clear-when` should explicitly remove the current `when_date`.
- These paths should use normal application logic rather than raw store updates.

### 2. One-Shot Structured Capture

Extend `add` so an item can be created, categorized, and given numeric values in
one command.

Proposed commands:

```bash
agenda-cli add "DRZ Payment" \
  --when 2025-12-11 \
  --note "Monthly payment" \
  --category "Moto Budget 2025" \
  --category "Sheffield Financial" \
  --category DRZ4SM \
  --value Cost=245.96
```

```bash
agenda-cli add "YCRS" \
  --when 2026-02-20 \
  --category "Moto Budget 2026" \
  --category Track \
  --category YCRS \
  --value Cost=4000
```

Notes:

- `--category` should be repeatable.
- `--value` should be repeatable and use `CATEGORY=NUMBER` syntax.
- Numeric values remain category-backed assignments, not item-native fields.

### 3. CSV Import

Add a structured import entrypoint for repeated rows.

Proposed commands:

```bash
agenda-cli import csv expenses.csv \
  --title-col Expense \
  --date-col Date \
  --note-col Notes \
  --category-col Category \
  --category-separator "," \
  --value-col Cost=Cost \
  --assign "Moto Budget 2025"
```

```bash
agenda-cli import csv expenses.csv \
  --title-col Expense \
  --date-col Date \
  --category-col Category \
  --vendor-col Vendor=Vendor \
  --value-col Cost=Cost \
  --assign "Moto Budget 2026" \
  --dry-run
```

Capabilities this proposal should cover:

- explicit column mapping for title/date/note/categories/vendor/cost-like
  numeric fields
- repeated `--assign <CATEGORY>` for categories applied to every imported row
- splitting a source column into repeated category assignments
- dry-run preview without persisting changes

Recommended semantics:

- `--vendor-col Vendor=Vendor` means "read the CSV `Vendor` column and assign
  or create a matching child under the `Vendor` category tree"
- `--value-col Cost=Cost` means "read the CSV `Cost` column and assign it as
  the numeric value for the `Cost` category"

### 4. View Authoring

Add incremental view-authoring commands for criteria, sections, columns, and
display metadata.

Current gap:

- `view create` supports basic include/exclude criteria
- repeated `--include` uses AND semantics
- sections, column definitions, aliases, and remove-from-view behavior are not
  fully configurable from CLI

Proposed commands:

```bash
agenda-cli view create "Moto Budget Combined" \
  --or-include "Moto Budget 2025" \
  --or-include "Moto Budget 2026" \
  --hide-unmatched
```

```bash
agenda-cli view section add "Expenses by Year" "2025" --include "Moto Budget 2025"
agenda-cli view section add "Expenses by Year" "2026" --include "Moto Budget 2026"
```

```bash
agenda-cli view column add "Expenses by Year" 0 When --kind when --width 12
agenda-cli view column add "Expenses by Year" 0 Vendor --width 26
agenda-cli view column add "Expenses by Year" 0 "Budget Tags" --width 22
agenda-cli view column add "Expenses by Year" 0 Cost --width 12 --summary sum
```

```bash
agenda-cli view alias set "Expenses by Year" When Date
agenda-cli view alias set "Expenses by Year" "Budget Tags" Category
agenda-cli view set-item-label "Expenses by Year" Expense
agenda-cli view set-remove-from-view "Expenses by Year" "Moto Budget 2025" "Moto Budget 2026"
```

Proposed authoring coverage:

- OR-capable criteria entry for combined views
- section add/remove/update commands
- column add/remove/update commands
- alias configuration commands
- item-column-label configuration
- remove-from-view configuration

### 5. Numeric Category Formatting

Add CLI control over numeric display formatting.

Proposed commands:

```bash
agenda-cli category format Cost --decimals 2 --currency '$' --thousands
agenda-cli category format Hours --decimals 1
agenda-cli category format Count --decimals 0 --no-thousands
```

This should cover:

- decimal precision
- optional currency symbol
- thousands separator behavior

## Concrete Example Workflows

The examples below are proposed CLI flows, not current behavior.

### Example A: Import a Budget CSV Into a Fresh Database

```bash
agenda-cli --db moto.ag category create Budget
agenda-cli --db moto.ag category create "Moto Budget 2025" --parent Budget --disable-implicit-string
agenda-cli --db moto.ag category create Vendor --disable-implicit-string
agenda-cli --db moto.ag category create "Budget Tags" --disable-implicit-string
agenda-cli --db moto.ag category create Cost --type numeric --disable-implicit-string
agenda-cli --db moto.ag category format Cost --decimals 2 --currency '$' --thousands

agenda-cli --db moto.ag import csv expenses.csv \
  --title-col Expense \
  --date-col Date \
  --vendor-col Vendor=Vendor \
  --category-col Category \
  --category-parent "Budget Tags" \
  --category-separator "," \
  --value-col Cost=Cost \
  --assign "Moto Budget 2025"
```

### Example B: Capture One Dated Expense Row Without a CSV

```bash
agenda-cli --db moto.ag add "DRZ Payment" \
  --when 2025-12-11 \
  --note "Monthly payment" \
  --category "Moto Budget 2025" \
  --category "Sheffield Financial" \
  --category DRZ4SM \
  --value Cost=245.96
```

### Example C: Create a Combined Budget View Across 2025 and 2026

```bash
agenda-cli --db moto.ag view create "Moto Budget Combined" \
  --or-include "Moto Budget 2025" \
  --or-include "Moto Budget 2026" \
  --hide-unmatched

agenda-cli --db moto.ag view section add "Moto Budget Combined" "All Moto Budget Items"
agenda-cli --db moto.ag view column add "Moto Budget Combined" 0 When --kind when --width 12
agenda-cli --db moto.ag view column add "Moto Budget Combined" 0 Vendor --width 26
agenda-cli --db moto.ag view column add "Moto Budget Combined" 0 "Budget Tags" --width 22
agenda-cli --db moto.ag view column add "Moto Budget Combined" 0 Cost --width 12 --summary sum
agenda-cli --db moto.ag view alias set "Moto Budget Combined" When Date
agenda-cli --db moto.ag view alias set "Moto Budget Combined" "Budget Tags" Category
agenda-cli --db moto.ag view set-item-label "Moto Budget Combined" Expense
```

### Example D: Create an “Expenses by Year” View With Separate 2025 and 2026 Sections

```bash
agenda-cli --db moto.ag view create "Expenses by Year" \
  --or-include "Moto Budget 2025" \
  --or-include "Moto Budget 2026" \
  --hide-unmatched

agenda-cli --db moto.ag view section add "Expenses by Year" "2025" --include "Moto Budget 2025"
agenda-cli --db moto.ag view section add "Expenses by Year" "2026" --include "Moto Budget 2026"

agenda-cli --db moto.ag view column add "Expenses by Year" 0 When --kind when --width 12
agenda-cli --db moto.ag view column add "Expenses by Year" 0 Vendor --width 26
agenda-cli --db moto.ag view column add "Expenses by Year" 0 "Budget Tags" --width 22
agenda-cli --db moto.ag view column add "Expenses by Year" 0 Cost --width 12 --summary sum
agenda-cli --db moto.ag view column add "Expenses by Year" 1 When --kind when --width 12
agenda-cli --db moto.ag view column add "Expenses by Year" 1 Vendor --width 26
agenda-cli --db moto.ag view column add "Expenses by Year" 1 "Budget Tags" --width 22
agenda-cli --db moto.ag view column add "Expenses by Year" 1 Cost --width 12 --summary sum
```

### Example E: Format `Cost` as Currency

```bash
agenda-cli --db moto.ag category format Cost --decimals 2 --currency '$' --thousands
```

## Design Constraints / Policy

This proposal assumes the following defaults:

- `--when` must go through normal app logic, not raw store writes.
- Date-only input resolves to a stable midnight datetime in local calendar
  terms.
- Repeated category flags remain explicit and deterministic.
- Numeric values are assigned via named numeric categories, not special-case
  item fields.
- View-authoring commands mutate named views incrementally instead of requiring
  whole-object JSON editing.
- Proposed CLI additions should fit naturally under the existing command tree
  rather than introducing a second configuration language.

## Alternatives Considered

### 1. “Just use direct SQLite”

Rejected because it requires internal storage knowledge, bypasses domain logic,
and turns common workflows into unsafe ad hoc migrations.

### 2. “Make TUI the only advanced editor”

Rejected because it blocks automation, shell scripting, reproducible imports,
and documentation-friendly examples. Advanced TUI support is valuable, but it
does not replace a scriptable CLI.

### 3. “Ship CSV import only and leave view authoring for later”

Rejected because it solves only half of the problem. The structured data still
needs a CLI path to become useful in saved views without manual TUI work or
direct SQL.

## Follow-up Work

Suggested implementation split:

- Phase 1: `--when`, one-shot add flags, numeric format commands
- Phase 2: CSV import
- Phase 3: view-authoring CLI
- Phase 4: dry-run polish, better error messages, import ergonomics

## Review Notes

This proposal should be reviewed against the current shipped CLI, not an ideal
future interface. In particular, the proposal is intentionally grounded in the
current command tree where:

- `add` only supports text + note
- `edit` does not support explicit `when_date` editing
- `category set-value` already exists
- `view create/edit` remain basic
- `view set-summary` already exists

The goal is CLI parity with the model and with important TUI workflows, while
keeping the public interface additive and script-friendly.
