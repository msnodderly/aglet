# Autonomous Agent Workflow

**Project:** aglet

## Prime Directive

Select ONE task. Complete it. Stop.

---

## Session Start

1. **Read context**: `AGENTS.md` (full workflow reference), then `br ready`
2. **Pick task**: Select highest priority `open` issue — never claim `in_progress`
3. **Read prompt**: Check `docs/process-prompt-*.md` for your task's implementer prompt.
   If no prompt exists, flag it — don't start without one.
4. **Claim on main**:
   ```bash
   br update <id> --status in_progress
   br comments add <id> "Claimed $(date '+%Y-%m-%d %H:%M'). Plan: <brief approach>"
   br sync --flush-only
   git add .beads/ && git commit -m "br sync: Claim <id>"
   ```
5. **Branch**: `git checkout -b task/<id>-short-description`

---

## Implementation

### Before Coding
- Read the implementer prompt in `docs/process-prompt-<task>.md`
- Read the files listed in its "What to read" section
- Understand how your code will be consumed downstream

### While Coding
- Only work on the selected task — no drive-by refactors
- Commit frequently with clear messages
- Build regularly: `cargo build -p agenda-core`
- Run tests: `cargo test -p agenda-core`
- Run clippy: `cargo clippy -p agenda-core`

### After Coding
- Do NOT modify product spec unless instructed
- Do NOT modify `AGENTS.md` unless you discovered a workflow issue

---

## Session End

1. **Verify on branch**:
   ```bash
   cargo test -p agenda-core
   cargo clippy -p agenda-core
   ```

2. **Merge to main**:
   ```bash
   git checkout main
   git merge task/<id>-short-description
   ```

3. **Close issue** (on main — all `br` writes happen on main):
   ```bash
   br comments add <id> "Done. <summary of what was implemented/changed>"
   br close <id>
   br sync --flush-only
   git add .beads/ && git commit -m "br sync: Close <id>"
   ```

4. **Clean up**:
   ```bash
   git branch -d task/<id>-short-description
   ```

5. **Clean gate** (mandatory):
   ```bash
   git status --short
   ```
   Must be empty. If `.beads/` is dirty, commit it.

6. **Verify**: `br show <id>` should show CLOSED

7. **STOP** — Do not pick another task.

---

## Rules

- **`br` writes only on main**: `br update`, `br close`, `br sync` must run
  from main. See `AGENTS.md` for why.
- **No remote**: Skip `git pull/push`. Merge to main directly, no PRs.
- **Sticky assignments**: The rule engine never revokes existing assignments.
  Only the user can remove them.
- **Scope**: Only touch files listed in your task prompt's definition of done.
