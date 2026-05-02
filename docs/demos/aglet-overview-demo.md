---
title: Aglet Overview Demo
updated: 2026-04-19
---

# Aglet: Aglet Reborn — Project Overview Demo

*2026-03-18T06:37:02Z by Showboat 0.6.1*
<!-- showboat-id: d7a8c06e-bf80-4e40-b8d4-571aab1e4e94 -->

Aglet is a Rust-based personal information manager inspired by Lotus Agenda. It combines a powerful CLI, a rich TUI, and a SQLite-backed engine that supports hierarchical categories, numeric values, automatic category assignment, dependency tracking, and customizable views with sectioned layouts.

## Project Structure

```bash
echo "=== Workspace Crates ===" && cargo metadata --no-deps --format-version 1 2>/dev/null | python3 -c "import json,sys; d=json.load(sys.stdin); [print(f\"  {p.get(chr(110)+chr(97)+chr(109)+chr(101))} v{p.get(chr(118)+chr(101)+chr(114)+chr(115)+chr(105)+chr(111)+chr(110))}\") for p in d[\"packages\"]]"
```

```output
=== Workspace Crates ===
  aglet-core v0.1.0
  aglet-tui v0.1.0
  aglet-cli v0.1.0
```

```bash
echo "=== Source Lines by Crate ===" && for crate in aglet-core aglet-tui aglet-cli; do lines=$(find crates/$crate/src -name "*.rs" -exec cat {} + | wc -l | tr -d " "); echo "  $crate: $lines lines"; done
```

```output
=== Source Lines by Crate ===
  aglet-core: 11843 lines
  aglet-tui: 36135 lines
  aglet-cli: 5521 lines
```

The TUI is the bulk of the codebase (~66% of lines) — it implements 17 distinct modes, a shared TextBuffer for all text editing, an InputPanel abstraction for add/edit flows, per-section filters, a category manager, a view editor, numeric column displays, and a kanban board view.

## Core Data Model Demo

Let's create a fresh database and demonstrate the key concepts: items, hierarchical categories, numeric values, views with sections, and the auto-assignment engine.

```bash
A=/Users/mds/src/aglet/target/release/aglet
DB=/tmp/aglet-demo.ag
rm -f $DB
$A --db $DB category create "Priority" --exclusive
$A --db $DB category create "High" --parent Priority
$A --db $DB category create "Normal" --parent Priority
$A --db $DB category create "Low" --parent Priority
$A --db $DB category create "Area"
$A --db $DB category create "Backend" --parent Area
$A --db $DB category create "Frontend" --parent Area
$A --db $DB category create "Cost" --type numeric
echo ""
echo "=== Category Hierarchy ==="
$A --db $DB category list
```

```output
created category Priority (type=Tag, processed_items=0, affected_items=0)
created category High (type=Tag, processed_items=0, affected_items=0)
created category Normal (type=Tag, processed_items=0, affected_items=0)
created category Low (type=Tag, processed_items=0, affected_items=0)
created category Area (type=Tag, processed_items=0, affected_items=0)
created category Backend (type=Tag, processed_items=0, affected_items=0)
created category Frontend (type=Tag, processed_items=0, affected_items=0)
created category Cost (type=Numeric, processed_items=0, affected_items=0)

=== Category Hierarchy ===
- Area
  - Backend
  - Frontend
- Cost [numeric]
- Done [no-implicit-string] [non-actionable]
- Entry [no-implicit-string] [non-actionable]
- Priority [exclusive]
  - High
  - Normal
  - Low
- When [no-implicit-string] [non-actionable]
```

**Priority** is exclusive (only one child assignable per item). **Area** is hierarchical — assigning Backend auto-assigns parent Area via subsumption. **Cost** is numeric (carries a decimal value per item). Done, Entry, and When are reserved system categories.

## Items and Auto-Assignment

Item text is scanned against category names at creation time. Matching tokens trigger automatic assignment — no manual tagging needed for common patterns.

```bash
A=/Users/mds/src/aglet/target/release/aglet
DB=/tmp/aglet-demo.ag
$A --db $DB add "Refactor Backend auth module" --note "Needs security review"
$A --db $DB add "Design new Frontend dashboard"
$A --db $DB add "Fix database connection pooling"
echo ""
echo "=== All Items (auto-assignment in action) ==="
$A --db $DB view show "All Items"
```

```output
created be778352-26fc-419b-9c7c-0a097804f38c
new_assignments=1
created 040a207f-721b-4d0d-801a-64786197e663
new_assignments=1
created 9b31cdf0-dbbe-4b7e-9be8-93aee2b0b35f

=== All Items (auto-assignment in action) ===
# All Items
hide_dependent_items: false
ID                                    STATUS  WHEN                 TITLE
------------------------------------  ------  -------------------  -----
9b31cdf0-dbbe-4b7e-9be8-93aee2b0b35f  open    -                    Fix database connection pooling
040a207f-721b-4d0d-801a-64786197e663  open    -                    Design new Frontend dashboard
                                                                   categories: Area, Frontend
be778352-26fc-419b-9c7c-0a097804f38c  open    -                    Refactor Backend auth module
                                                                   categories: Area, Backend
                                                                   note: Needs security review
```

"Backend" and "Frontend" in the item text matched category names and were assigned automatically. Backend's parent Area was also assigned via subsumption. "Fix database connection pooling" had no matching tokens.

## Manual Assignment, Numeric Values, and Dependencies

```bash
A=/Users/mds/src/aglet/target/release/aglet
DB=/tmp/aglet-demo.ag
# Assign Priority manually (exclusive: only one child allowed)
$A --db $DB category assign be778352 High
$A --db $DB category assign 040a207f Normal
$A --db $DB category assign 9b31cdf0 High
$A --db $DB category assign 9b31cdf0 Backend
echo ""
# Set numeric Cost values
$A --db $DB category set-value be778352 Cost 450.00
$A --db $DB category set-value 040a207f Cost 200.00
$A --db $DB category set-value 9b31cdf0 Cost 75.00
echo ""
# Dependency: Refactor cannot start until Fix DB is done
$A --db $DB link depends-on be778352 9b31cdf0
echo ""
echo "=== Blocked items ==="
$A --db $DB list --blocked
echo ""
echo "=== High Priority view ==="
$A --db $DB view create "High Priority" --include High
$A --db $DB view show "High Priority"
```

```output
assigned item be778352-26fc-419b-9c7c-0a097804f38c to category High
assigned item 040a207f-721b-4d0d-801a-64786197e663 to category Normal
assigned item 9b31cdf0-dbbe-4b7e-9be8-93aee2b0b35f to category High
assigned item 9b31cdf0-dbbe-4b7e-9be8-93aee2b0b35f to category Backend

set value for item be778352-26fc-419b-9c7c-0a097804f38c category Cost = 450.00
set value for item 040a207f-721b-4d0d-801a-64786197e663 category Cost = 200.00
set value for item 9b31cdf0-dbbe-4b7e-9be8-93aee2b0b35f category Cost = 75.00

linked be778352-26fc-419b-9c7c-0a097804f38c depends-on 9b31cdf0-dbbe-4b7e-9be8-93aee2b0b35f

=== Blocked items ===
# All Items
hide_dependent_items: false
ID                                    STATUS  WHEN                 TITLE
------------------------------------  ------  -------------------  -----
be778352-26fc-419b-9c7c-0a097804f38c  open    -                    Refactor Backend auth module
                                                                   categories: Area, Backend, Cost, High, Priority
                                                                   note: Needs security review

=== High Priority view ===
created view High Priority
# High Priority
hide_dependent_items: false
ID                                    STATUS  WHEN                 TITLE
------------------------------------  ------  -------------------  -----
9b31cdf0-dbbe-4b7e-9be8-93aee2b0b35f  open    -                    Fix database connection pooling
                                                                   categories: Area, Backend, Cost, High, Priority
be778352-26fc-419b-9c7c-0a097804f38c  open    -                    Refactor Backend auth module
                                                                   categories: Area, Backend, Cost, High, Priority
                                                                   note: Needs security review
```

`--blocked` filters to items with at least one unresolved prerequisite — computed at query time from the dependency graph. Views act as saved filters: the "High Priority" view persists and updates automatically as items are added or reassigned.

## The TUI

The TUI (`aglet tui`) provides a full interactive interface over the same SQLite store. It is built on `ratatui` with a custom rendering pipeline.

### Normal View — Sectioned Column Layout

Items are displayed in a tabular layout with user-configurable columns (category values and numeric summaries). Sections group items by a category's children — here, by year via a date-range category. Each section footer shows a column aggregate (sum, average, count). The 2-row footer shows transient status above persistent per-mode key hints.

Key bindings in Normal mode: `n` add item, `e` edit, `a` assign categories, `m` lanes (kanban), `z` cards, `s` sort, `f`/`F` column format/summary, `v` views, `p` preview, `/` search.

### Category Assignment Panel

Pressing `a` opens an inline scrollable category picker. Categories are grouped by parent with `[x]` for assigned tags and `[N]` for assigned numeric values with an inline editable value field. `Space` toggles tag membership; `j`/`k` navigates; `n`/`/` creates/filters inline.

### Category Manager

A full-screen mode for managing the category tree. Left pane shows the hierarchy with indent and readable rule-count badges like `[2 conditions]` or `[1 action]`; right pane shows details, flags (Exclusive, Auto-match, Actionable), conditions, actions, and a freeform note field. `H`/`J`/`K`/`L` reorder; `<<`/`>>` change depth level; workflow roles live in Global Settings (`g s` / `F10`).

### View Editor

The view editor (Tab between SECTIONS and DETAILS panes) configures filter criteria, date ranges, display mode (single-line / multi-line), section flow (vertical stacked / horizontal lanes), unmatched-item visibility, and aliases. Changes are saved with `S`.
