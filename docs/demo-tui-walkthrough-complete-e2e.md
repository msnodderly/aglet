# TUI Walkthrough: Complete E2E Demo (Adapted)

Date: 2026-02-16  
Source demo: `docs/demo-complete-cli-e2e-demo-log.md`

This walkthrough replays the core "capture -> organize -> done/delete -> recover"
workflow with TUI-first interaction where possible.

## 1) Setup

```bash
cd /Users/mds/src/aglet-bd-1ns
DB="/tmp/aglet-tui-complete-walkthrough-$(date +%s).ag"
echo "$DB"
```

Launch TUI:

```bash
cargo run -q -p agenda-tui -- --db "$DB"
```

## 2) TUI Steps

### A. Capture six items (from complete demo)

For each item:
- Press `n`
- Type text
- Press `Enter`

Items:

1. `Follow up with Sarah on Frabulator integration next Friday at 3pm`
2. `Buy groceries (milk, eggs, bread) tomorrow at 6pm`
3. `Review Project Y architecture doc Monday`
4. `Call dentist next Tuesday at 9am`
5. `Green field prototype brainstorming`
6. `Book flights for personal trip in March`

### B. Create category hierarchy in TUI manager

- Press `F9` to open category manager.
- Press `N`, create `Work`.
- Press `N`, create `Personal`.
- Press `N`, create `Sarah`.
- Select `Work`, press `n`, create `Project Y`.
- Select `Project Y`, press `n`, create `Frabulator`.
- Select `Project Y`, press `n`, create `Green field`.
- Select `Personal`, press `n`, create `Groceries`.
- Press `F9` to close category manager.

### C. Perform recovery-relevant mutations

- Select `Follow up with Sarah on Frabulator integration next Friday at 3pm`.
- Press `a`, choose `Sarah`, Enter.
- Press `a` again, choose `Frabulator`, Enter.
- Press `F8`, then `N`, type `Work View`, Enter.
- In include-category picker, choose `Work`, Enter.
- Press `F8`, select `Work View`, press `r`, rename to `Work Board`, Enter.
- Press `F8`, select `Work Board`, press `e`, choose `Project Y`, Enter.
- Press `F8`, select `Work Board`, Enter to switch.
- Select `Review Project Y architecture doc Monday` (use `j/k`).
- Press `d` to mark done.
- Press `x`, then `y` to delete the done item.
- Press `q` to exit TUI.

## 3) Verify with CLI

Use the same DB path:

```bash
# Category hierarchy and reserved/system categories
cargo run -q -p agenda-cli -- --db "$DB" category list

# View create/rename/edit results
cargo run -q -p agenda-cli -- --db "$DB" view list
cargo run -q -p agenda-cli -- --db "$DB" view show "Work Board"

# Item state after TUI done + delete
cargo run -q -p agenda-cli -- --db "$DB" list --include-done

# Deletion log entry should exist for the deleted item
cargo run -q -p agenda-cli -- --db "$DB" deleted
```

Restore the deleted item:

```bash
LOG_ID=$(cargo run -q -p agenda-cli -- --db "$DB" deleted | head -n1 | cut -d' ' -f1)
cargo run -q -p agenda-cli -- --db "$DB" restore "$LOG_ID"
cargo run -q -p agenda-cli -- --db "$DB" list --include-done
```

## 4) Expected Outcome

- All six capture entries are created via TUI.
- Category hierarchy is created via TUI manager.
- Item-to-category assignment works directly in TUI (`a`).
- View create/rename/include-edit works directly in TUI (`F8`).
- One item is marked done and deleted in TUI.
- Item is recoverable from deletion log via CLI restore.

## 5) Remaining TUI Gaps During This Demo

- No in-TUI deletion-log browser/restore command; recovery requires CLI.
- No undo (`Ctrl-Z`) by v1 decision; recovery is explicit via delete log + restore.
