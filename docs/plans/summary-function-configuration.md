# Plan: Summary Function Configuration

## Problem

PR #57 added `SummaryFn` (sum/avg/min/max/count) to the `Column` model and
rendering in both CLI and TUI. However, there is no way to **set** a column's
summary function — it always defaults to `None`. The TUI summary row never
appears because no column ever has a non-None summary_fn.

## Scope

Add configuration of `summary_fn` in both TUI Normal mode and CLI, keeping
changes minimal and focused.

### TUI: cycle summary_fn on focused numeric column

**Keybinding**: `F` (Shift+F) in Normal mode when cursor is on a numeric column.
Cycles: None → Sum → Avg → Min → Max → Count → None.

**Why `F`?** "F for function" — `s`/`S` are taken (sort), lowercase `f` is
taken (toggle preview focus). Non-numeric columns show a status message.

**Implementation** (in `board.rs` `handle_normal_key`):

1. Resolve the focused section + section_column_index from `self.column_index`
2. Check that the column's heading category is Numeric — if not, no-op
3. Clone the current view, mutate `section.columns[idx].summary_fn` to the next
   variant in the cycle
4. `store.update_view(&view)` + `self.refresh(store)`
5. Set status: `"Column summary: sum"` (or whatever the new fn is)

**Summary row rendering** — already implemented in `render/mod.rs:1910-1953`.
Once `summary_fn != None`, the SUMMARY row appears automatically.

### CLI: `view set-summary` subcommand

```
agenda-cli view set-summary <VIEW_NAME> <SECTION_INDEX> <COLUMN_NAME> <FN>
```

Where `<FN>` is one of: none, sum, avg, min, max, count.

**Implementation** (in `main.rs`):

1. Add `SetSummary` variant to `ViewCommand` enum
2. Look up the view by name, resolve the section and column by index/name
3. Mutate `column.summary_fn`, call `store.update_view()`
4. Print confirmation

### Footer hint

Update the Normal-mode footer hints to include `F:summary`.

### Tests

1. **TUI test**: `F` on numeric column cycles summary_fn, verify persisted via store
2. **TUI test**: `F` on non-numeric column is a no-op with status message
3. **CLI test**: `view set-summary` round-trips through view show output
4. **CLI test**: `view set-summary` on non-existent column errors cleanly

## Files changed

| File | Change |
|------|--------|
| `crates/agenda-core/src/model.rs` | `SummaryFn::next()` and `label()` methods |
| `crates/agenda-tui/src/modes/board.rs` | `F` keybinding handler |
| `crates/agenda-tui/src/render/mod.rs` | Footer hint for `F` |
| `crates/agenda-cli/src/main.rs` | `view set-summary` subcommand |
| `crates/agenda-tui/src/lib.rs` | TUI tests |

## Out of scope

- ViewEdit columns region (FR `cf6b7dd8`) — will subsume this later
- Per-section different summary_fn for same column (current model already supports this)
