# aglet

Rust CLI/TUI for managing agenda items, categories, views, and item dependency links.

## Quick Start

```bash
cargo run --bin aglet -- --db aglet-features.ag
cargo run --bin aglet -- --db aglet-features.ag list --view "All Items"
```

Running `aglet` without a subcommand opens the TUI. Use `aglet list` for
non-interactive list output.

## Multi-Agent Claim Workflow

Use `claim` to atomically move an item into active work:

```bash
# Default claim behavior:
# - assigns "In Progress"
# - fails if item already has "In Progress" or "Complete"
cargo run --bin aglet -- --db aglet-features.ag claim <ITEM_ID>
```

Equivalent explicit criteria:

```bash
cargo run --bin aglet -- --db aglet-features.ag claim <ITEM_ID> \
  --claim-category "In Progress" \
  --must-not-have "In Progress" \
  --must-not-have "Complete"
```

`claim` requires the target category to exist. If `In Progress` is missing, create
it as a category or sub-category first. Feature DB-style setup:

```bash
cargo run --bin aglet -- --db aglet-features.ag category create Status --exclusive
cargo run --bin aglet -- --db aglet-features.ag category create Ready --parent Status
cargo run --bin aglet -- --db aglet-features.ag category create "In Progress" --parent Status
cargo run --bin aglet -- --db aglet-features.ag category create "Waiting/Blocked" --parent Status
cargo run --bin aglet -- --db aglet-features.ag category create Complete --parent Status
```

Custom preconditions:

```bash
cargo run --bin aglet -- --db aglet-features.ag claim <ITEM_ID> \
  --claim-category "In Progress" \
  --must-not-have "In Progress" \
  --must-not-have "Complete" \
  --must-not-have "Waiting/Blocked"
```

## Workflow Docs

- [PM process](/Users/mds/src/aglet/docs/process/project-management.md)
- [Agent workflow prompt](/Users/mds/src/aglet/docs/process/agent-workflow.md)
- [Open-item query reference](/Users/mds/src/aglet/docs/reference/open-project-items-by-priority.md)
