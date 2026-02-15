# Autonomous Agent Workflow

**Project:** aglet

## Prime Directive

Select ONE task. Complete it. Stop.

---

## Session Start

1. **Read context**: `AGENTS.md` (workflow), `spec/mvp-spec.md` (product), `spec/mvp-tasks.md` (tasks)
2. **Pick task**: `br ready` — select highest priority NOT already `in_progress`
3. **Claim task**: `br update <id> --status in_progress`
4. **Create worktree**: `git worktree add -b task/<id>-short-desc <name> main`

**Note:** Worktrees share parent's `.beads/` — `br` commands work from any worktree.

---

## Implementation

### Before Coding
- `br show <id>` for full task description
- Read `spec/mvp-spec.md` for context
- Complex features: create design doc in `docs/plans/`

### While Coding
- Only work on selected task—no drive-by refactors
- Commit frequently with clear messages
- Build regularly: `cargo build --workspace`

### After Coding
- Update `AGENTS.md` with new patterns/gotchas
- Do NOT modify product spec unless instructed

---

## Session End

1. Build: `cargo build --workspace && cargo test --workspace`
2. Close task: `br close <id> --reason "Completed"`
3. Merge branch directly to main (no pull requests — local-only project):
   ```bash
   git checkout main
   git merge task/<branch> -m "Merge <branch>: <summary>"
   git worktree remove <worktree-path> --force
   git branch -d task/<branch>
   ```
4. Sync beads:
   ```bash
   br sync --flush-only
   git add .beads/ && git commit -m "br sync: Update issues"
   ```
5. **STOP** — Do not pick another task

**No remote:** Skip `git pull/push` — no remote configured. Always merge to main directly, never use PR workflow.