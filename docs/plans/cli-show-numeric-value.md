---
title: aglet show — render numeric values for numeric-category assignments
status: draft
created: 2026-05-12
---

# aglet show — render numeric values for numeric-category assignments

## Context

`aglet show <item>` currently prints one line per assigned category in the
form `Name | Source | origin`, but omits the numeric value carried on the
assignment edge for Numeric-kind categories. The model already stores
`Assignment.numeric_value: Option<Decimal>` and the TUI board renders these
values via `format_numeric_cell` — only the CLI text view is silent.

Bugtracker item `b36b6305` explicitly lists "numeric values" as expected
content for `aglet show` snapshots, and the new CLI integration tests
(`crates/aglet-cli/tests/add_show_edit.rs::add_with_numeric_value_persists`)
snapshot the current (buggy) output. This makes the gap easy to confirm:
the snapshot shows `Hours | Manual | manual:cli.set-value` but contains no
`2.5` anywhere.

Tracking bug: `5c5d5b54` in `../bugtracker.ag`.

## Approach

Lift the existing TUI formatter into `aglet-core` so both CLI and TUI use
it, then extend `cmd_show` to print the numeric value when the assignment
carries one.

### Step 1 — Move formatter to aglet-core

Move `format_numeric_cell` (and `add_thousands_separator` helper) from
`crates/aglet-tui/src/ui_support.rs:509` to a new `numeric_format` module
under `crates/aglet-core/src/`. Keep the public signature stable:

```rust
pub fn format_numeric_cell(
    value: Option<rust_decimal::Decimal>,
    format: Option<&NumericFormat>,
) -> String
```

Update `aglet-tui` to `use aglet_core::numeric_format::format_numeric_cell;`
and delete the local copy. The 4 existing unit tests
(`ui_support.rs:1462-1490`) move with the code.

### Step 2 — Render value in cmd_show

In `crates/aglet-cli/src/main.rs:1680-1744 (cmd_show)`, the assignment
loop at lines 1717-1736 builds `(name, assignment)` rows from
`item.assignments`. The assignment iterator yields
`(CategoryId, &Assignment)`. To format the value we need the
`NumericFormat` from the owning category — already fetched via
`categories = store.get_hierarchy()?` at line 1683.

Build a `HashMap<CategoryId, &Category>` alongside `category_names`, then
in the assignment row:

```rust
if let Some(value) = assignment.numeric_value {
    let format = by_id.get(cat_id)
        .and_then(|c| c.numeric_format.as_ref());
    let rendered = format_numeric_cell(Some(value), format);
    println!("  {} = {} | {:?} | {}", name, rendered, assignment.source, origin);
} else {
    println!("  {} | {:?} | {}", name, assignment.source, origin);
}
```

The `Name = value | Source | origin` layout keeps the existing line
structure backward-compatible for callers parsing tag assignments.

### Step 3 — Update tests

- Refresh the existing snapshot at
  `crates/aglet-cli/tests/snapshots/add_show_edit__add_with_numeric_value_persists.snap`
  to include the new `Hours = 2.5 | Manual | manual:cli.set-value` line.
- Add a dedicated test in `add_show_edit.rs` that exercises a non-default
  `NumericFormat` (currency symbol + thousands separator) so the
  formatter integration is covered, not just the default form.

### Step 4 — Update b36b6305 progress note

Strike the "numeric values" caveat from the progress note appended on the
b36b6305 item in `../bugtracker.ag`.

## Files to modify

- `crates/aglet-tui/src/ui_support.rs` — remove `format_numeric_cell` and
  helper; `use aglet_core::numeric_format::format_numeric_cell`
- `crates/aglet-core/src/lib.rs` + new `crates/aglet-core/src/numeric_format.rs`
- `crates/aglet-cli/src/main.rs` — `cmd_show` numeric branch
- `crates/aglet-cli/tests/add_show_edit.rs` — new currency/thousands test
- `crates/aglet-cli/tests/snapshots/add_show_edit__add_with_numeric_value_persists.snap`
  — refreshed
- `../bugtracker.ag` (out-of-tree) — mark `5c5d5b54` Resolved; trim
  b36b6305 note

## Verification

```sh
# 1. Manual reproducer matches the new expected output
cargo run --bin aglet -- --db /tmp/show-num.ag category create Hours --type numeric
cargo run --bin aglet -- --db /tmp/show-num.ag add "Work" --value "Hours=2.5"
cargo run --bin aglet -- --db /tmp/show-num.ag show <id>
# expect: "Hours = 2.5 | Manual | manual:cli.set-value"

# 2. Unit + integration tests all green
cargo test -p aglet-core --lib numeric_format
cargo test -p aglet-tui --lib    # existing format_numeric_cell tests pass after move
cargo test -p aglet-cli          # snapshot refreshes pass; new currency test passes

# 3. Spot-check the TUI board renders unchanged (no visual diff in
# Numeric column cells for default and currency formats).
```
