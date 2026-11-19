---
title: Codebase Simplification Handoff 2
updated: 2026-11-18
---

# Handoff: Codebase Simplification, after Phase 3, D1, D3, and Phase 4 start

Written by Claude (Opus 5.5, Claude Code). Supersedes
`2026-11-17-001-codebase-simplification.md`.

**Plan:** `docs/plans/codebase-review-and-simplification-plan.md` (active).
**D1 plan:** `docs/plans/schema-squash-v21.md` (active, implemented).
**Worktree:** `/home/code/src/aglet/.claude/worktrees/simplify`
**Top branch:** `simplify/phase-4c-small-modes`

## PR stack

Each PR targets the branch of the one before. After changing a lower branch,
merge it upward one branch at a time and push each.

| PR | Branch | Content |
|---|---|---|
| #157 | `simplify/phase-0-gate` | CI gate |
| #158 | `simplify/phase-1-bulk-transactions` | Bulk writes in one transaction |
| #159 | `simplify/phase-2-refresh-queries` | Batched refresh queries |
| #160 | `simplify/phase-3-core-cli-consolidation` | One LLM client, shared sort helpers |
| #161 | `simplify/phase-3b-cli-errors-and-split` | `CliResult`, `main.rs` split |
| #162 | `simplify/phase-3c-aglet-options` | `SaveOptions`, previews via rollback |
| #163 | `simplify/d3-source-from-explanation` | D3 step 1, code only |
| #164 | `simplify/d1-schema-squash` | D1 schema v21; **changes stored data** |
| #165 | `simplify/phase-4a-harness` | Key-driven TUI test harness |
| #166 | `simplify/phase-4b-key-events` | Handlers take `KeyEvent` |
| #167 | `simplify/phase-4c-small-modes` | HelpPanel owns its state |

Review comments on #158, #159, #162, #164, and #165 were fixed, merged up
the stack, and answered inline on 2026-11-18.

## Open items

- **D1 on real data:** not yet run against a real user file. Before merging
  #164, copy `aglet-features.ag`, open the copy with the branch build, and
  compare `aglet list` with `v0.1.0`.
- **D3 row count:** never run (the feature DB is on the user's Mac):
  `SELECT source, explanation_json = 'null', COUNT(*) FROM assignments GROUP BY 1, 2;`
- **gh stack:** still not installed; stacking is by base branch.

## Phase 4 step 3: how a mode migrates

HelpPanel (#167) is the pattern:

1. The variant carries the state: `Mode::HelpPanel { scroll }`. Delete the
   `App` field.
2. Move the key handler and renderer into `modes/<mode>.rs`. Prefer free
   functions over the mode's state. Use explicit imports, not
   `use crate::*`.
3. Replace `self.mode == Mode::X` with `matches!(self.mode, Mode::X { .. })`
   only for that variant (there are about 218 mode comparisons; unit
   variants keep working).
4. Port that mode's tests in `tests.rs` to `harness::Harness` and move them
   to `src/tests/<mode>.rs`. Bound any navigation with `press_until`.

**Next: ViewPicker.** `picker_index` outlives the mode today: the picker
hands off to rename or clone (InputPanel with `NameInputContext::ViewRename`
/ `ViewClone`, using `view_pending_edit_name` / `view_pending_clone_id`), to
ViewEdit (`modes/view_edit/editor.rs` returns to the picker on cancel or
save), and to `ViewDeleteConfirm`. Under D4 each of those children carries
the picker index back, for example `Mode::ViewDeleteConfirm { picker_index }`
and the rename context carrying `old_name` plus `picker_index`. The two
`view_pending_*` fields belong in `NameInputContext` variants.

Remaining order after ViewPicker: the confirm dialogs, LinkWizard,
GlobalSettings, SuggestionReview, CategoryColumnPicker, CategoryDirectEdit,
InputPanel, item assign, CategoryManager, ViewEdit, then Normal. Step 4 (form
descriptors) follows.

## Working notes

- Build with `CARGO_TARGET_DIR=/home/code/src/aglet/target`.
- The worktree guard rejects large heredocs and compound commands; write
  scripts to the scratchpad and run them with `python3 -I`.
- `cargo test` filters are plain substrings; `a\|b` matches nothing.
- Commit attribution: `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Smoke test (top branch)

```bash
cargo run --bin aglet -- --db /tmp/smoke.ag add "Pay rent"
cargo run --bin aglet -- --db /tmp/smoke.ag show <id-prefix>
cargo run --bin aglet -- --db /tmp/smoke.ag      # TUI: ? help scrolls, a assign picker previews
```

For the v20 upgrade, build a v20 file from
`crates/aglet-core/src/testdata/schema_v20.sql` with `sqlite3`, set
`PRAGMA user_version = 20`, open it, and check that `<file>.v20-backup.ag`
appears and is still v20.
