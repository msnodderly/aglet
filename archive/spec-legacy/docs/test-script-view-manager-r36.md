# View Manager R3.6 Focused Manual Script

Date: 2026-02-18
Scope: T090/T091/T092 manager-specific checks

## 1) Setup

```bash
cd /Users/mds/src/aglet
DB="/tmp/aglet-view-manager-r36-$(date +%s).ag"
echo "$DB"

cargo run -q -p agenda-cli -- --db "$DB" category create Alpha
cargo run -q -p agenda-cli -- --db "$DB" category create Beta
cargo run -q -p agenda-cli -- --db "$DB" view create "Manager Demo"
```

## 2) Launch

```bash
cargo run -q -p agenda-tui -- --db "$DB"
```

## 3) Checklist

1. Open manager:
- Press `v`, select `Manager Demo`, press `V`.

2. Definition pane validation gate:
- Press `Tab` to Definition.
- Press `N` to add a row.
- Press `o` to set row join to OR.
- Press `s`.
- Expected: status starts with `Cannot save criteria:`.
- Press `a` to return to AND.
- Press `s`.
- Expected: save succeeds.

3. Sections pane operations:
- Press `Tab` to Sections.
- Press `N` to add section.
- Press `N` again to add another.
- Press `[` and `]` to reorder.
- Press `Enter` to open section editor.
- Press `N` to add one section in editor.
- Press `Esc` to return to manager.
- Expected: manager returns with dirty state and updated section count.

4. Unmatched settings from manager:
- In Sections pane press `u`.
- Press `t` to toggle unmatched.
- Press `l`, set label to `Backlog`, Enter.
- Press `Esc` to return to manager.
- Press `s` to persist.
- Press `Esc` to leave manager.

## 4) Verify

```bash
cargo run -q -p agenda-cli -- --db "$DB" view show "Manager Demo"
```

Expected in output:
- Updated sections list/order.
- Updated unmatched settings (`show_unmatched` toggled and label set).
