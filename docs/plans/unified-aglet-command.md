---
title: Unified Aglet Command
status: shipped
created: 2026-04-23
shipped: 2026-04-23
---

# Unified Aglet Command

## Goal

Ship one user-facing executable named `aglet` that contains both the scriptable
CLI and the interactive TUI.

## Shipped Behavior

`aglet` is the official binary target:

```bash
aglet
aglet --db path/to/db.ag
aglet --db path/to/db.ag list
aglet --db path/to/db.ag add "Follow up"
aglet --db path/to/db.ag tui
aglet --db path/to/db.ag tui --debug
```

Running `aglet` without a subcommand opens the TUI. Scriptable list behavior is
explicit:

```bash
aglet list
aglet --db path/to/db.ag list
```

The `tui` subcommand remains available for explicitness and supports `--debug`.

## Implementation Notes

- `agenda-core` remains the shared model, store, engine, query, and workflow
  crate.
- `agenda-tui` remains a library crate that owns the ratatui application and
  exposes `run_with_options(db_path, debug)`.
- `agenda-cli` remains the internal package/crate that owns the Clap command
  tree and command handlers, but its only binary target is now `aglet`.
- `agenda-tui` has `autobins = false`, and its old standalone `src/main.rs`
  wrapper was removed.
- `agenda-cli` has `autobins = false` with an explicit `[[bin]]` target named
  `aglet`.
- No database paths, schema, or workflow semantics changed.

## Follow-Up Options

- Consider renaming the internal `agenda-cli` package directory in a later
  cleanup if source names become more confusing than useful.
- Re-render long demo transcripts if exact captured help output matters.
- Keep examples explicit (`aglet list`) anywhere a command is intended to be
  non-interactive.

## Validation

- Parser coverage should confirm:
  - root command name is `aglet`
  - no subcommand maps to TUI launch without debug
  - `aglet tui --debug` maps to TUI launch with debug
  - existing CLI subcommands still parse
- Runtime smoke tests should confirm:
  - `cargo run --bin aglet -- --help`
  - `cargo run --bin aglet -- tui --help`
  - `cargo run --bin aglet -- --db /tmp/aglet-unified-smoke.ag list`
  - `cargo run --bin aglet -- --db /tmp/aglet-unified-smoke.ag add "Smoke item"`
  - `cargo run --bin aglet -- --db /tmp/aglet-unified-smoke.ag show <item-prefix>`
  - `cargo run --bin aglet -- --db /tmp/aglet-unified-smoke.ag` opens the TUI
