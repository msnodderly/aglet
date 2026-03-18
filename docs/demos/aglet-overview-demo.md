# Aglet: Agenda Reborn — Project Overview Demo

*2026-03-18T06:37:02Z by Showboat 0.6.1*
<!-- showboat-id: d7a8c06e-bf80-4e40-b8d4-571aab1e4e94 -->

Aglet is a Rust-based personal information manager inspired by Lotus Agenda. It combines a powerful CLI, a rich TUI, and a SQLite-backed engine that supports hierarchical categories, numeric values, automatic category assignment, dependency tracking, and customizable views with sectioned layouts.

## Project Structure

```bash
echo "=== Workspace Crates ===" && cargo metadata --no-deps --format-version 1 2>/dev/null | python3 -c "import json,sys; d=json.load(sys.stdin); [print(f\"  {p.get(chr(110)+chr(97)+chr(109)+chr(101))} v{p.get(chr(118)+chr(101)+chr(114)+chr(115)+chr(105)+chr(111)+chr(110))}\") for p in d[\"packages\"]]"
```

```output
=== Workspace Crates ===
  agenda-core v0.1.0
  agenda-tui v0.1.0
  agenda-cli v0.1.0
```

```bash
echo "=== Source Lines by Crate ===" && for crate in agenda-core agenda-tui agenda-cli; do lines=$(find crates/$crate/src -name "*.rs" -exec cat {} + | wc -l | tr -d " "); echo "  $crate: $lines lines"; done
```

```output
=== Source Lines by Crate ===
  agenda-core: 11843 lines
  agenda-tui: 36135 lines
  agenda-cli: 5521 lines
```

## Core Data Model Demo

Let's create a fresh database and demonstrate the key concepts: items, hierarchical categories, numeric values, views with sections, and the auto-assignment engine.

```bash
A=/Users/mds/src/aglet/target/release/agenda-cli
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

Notice: Priority is **exclusive** (only one child can be assigned at a time), Area is hierarchical (assigning Backend auto-assigns its parent Area via subsumption), and Cost is **numeric** (carries a decimal value per item). The reserved categories Done, Entry, and When are auto-created.

## Items, Assignments, and the Auto-Assignment Engine
