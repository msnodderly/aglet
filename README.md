# aglet

Rust CLI/TUI for managing agenda items, categories, views, and item dependency links.

## Quick Start

```bash
cargo run --bin agenda-cli -- --db aglet-features.ag list --view "All Items"
```

## Multi-Agent Claim Workflow

Use `claim` to atomically move an item into active work:

```bash
# Default claim behavior:
# - assigns "In Progress"
# - fails if item already has "In Progress" or "Complete"
cargo run --bin agenda-cli -- --db aglet-features.ag claim <ITEM_ID>
```

Custom preconditions:

```bash
cargo run --bin agenda-cli -- --db aglet-features.ag claim <ITEM_ID> \
  --claim-category "In Progress" \
  --must-not-have "In Progress" \
  --must-not-have "Complete" \
  --must-not-have "Waiting/Blocked"
```

## Workflow Docs

- [PM process](/Users/mds/src/aglet-ec938313-atomic-claim/PM.md)
- [Agent workflow prompt](/Users/mds/src/aglet-ec938313-atomic-claim/prompt.md)
- [Open-item query reference](/Users/mds/src/aglet-ec938313-atomic-claim/docs/open-project-items-by-priority.md)
