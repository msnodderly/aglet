# Wordsheet Agent Guide

## Issue Tracker (`br`)

Database in `.beads/`. Core workflow:

```bash
br ready                              # next unblocked issue
br show <id>                          # details + deps
br update <id> --status in_progress   # claim
br close <id>                         # done
```

### Commands

| Command | Purpose |
|---------|---------|
| `br ready` | Unblocked issues |
| `br show <id>` | Details, deps, labels |
| `br blocked` | Blocked issues |
| `br list` | All issues |
| `br search <query>` | Search |

### Creating Issues

```bash
br create "Title" -d "description" -p 1 -t task -l phase:x
br create "Title" --deps blocks:bd-xyz   # bd-xyz must finish first
```

### Dependencies

```bash
br dep add <issue> <depends_on>    # issue waits for depends_on
br dep remove <issue> <depends_on>
```

Direction: `br dep add bd-abc bd-xyz` = "bd-abc depends on bd-xyz"

### Statuses

`open` | `in_progress` | `blocked` | `deferred` | `closed` (use underscores)

### Claiming Issues

- Only claim issues with status `open`. **Never commandeer `in_progress` issues** — another agent may be working on them.
- If an `in_progress` issue looks stale, flag it for the user to coordinate rather than taking it over.
- Use `br ready` to find unblocked `open` issues, then check status before claiming.

### Troubleshooting

- `br <cmd> --help` for authoritative syntax
- `br ready` is scheduling truth; if it shows ready but `br show` lists blockers, check each dep's status directly

## Branching Workflow

### Why this workflow exists

`.beads/beads.db` is tracked in git. Each worktree gets an **independent copy**
checked out from the branch's committed state. `br` commands modify only the
local worktree's copy. This means beads state diverges between worktrees and
must be reconciled at merge time.

**The simplest solution: all `br` write operations happen on main only.**
Worktrees are for code changes. This eliminates reconciliation entirely.

### Single-agent workflow (branches, no worktrees)

```bash
# 1. Claim (from main)
br update <id> --status in_progress
br comments add <id> "Claimed $(date '+%Y-%m-%d %H:%M'). Plan: <brief approach>"
br sync --flush-only
git add .beads/ && git commit -m "br sync: Claim <id>"

# 2. Branch and work
git checkout -b task/<id>-short-description
# ... write code, commit ...

# 3. Merge and close (back on main)
git checkout main
git merge task/<id>-short-description
br comments add <id> "Done. <summary of what was implemented/changed>"
br close <id>
br sync --flush-only
git add .beads/ && git commit -m "br sync: Close <id>"

# 4. Clean up
git branch -d task/<id>-short-description
```

### Multi-agent workflow (worktrees for parallel work)

Use worktrees for code isolation. **Never run `br` write commands from a worktree.**

```bash
# 1. Claim (from main worktree)
br update <id> --status in_progress
br comments add <id> "Claimed $(date '+%Y-%m-%d %H:%M'). Plan: <brief approach>"
br sync --flush-only
git add .beads/ && git commit -m "br sync: Claim <id>"

# 2. Create worktree
git worktree add ../aglet-<id> -b task/<id>-short-description

# 3. Work in worktree (code only — no br commands)
cd ../aglet-<id>
# ... write code, commit, test ...
# All commits go on the task branch.

# 4. Merge (from main worktree)
cd /path/to/main
git merge task/<id>-short-description

# 5. Remove worktree, close issue (from main worktree)
git worktree remove ../aglet-<id> --force
br comments add <id> "Done. <summary of what was implemented/changed>"
br close <id>
br sync --flush-only
git add .beads/ && git commit -m "br sync: Close <id>"
git branch -d task/<id>-short-description
```

### Issue comments

Comments create a breadcrumb trail so other agents can understand and resume work.

**When claiming** — include date/time and your planned approach:
```bash
br comments add <id> "Claimed 2026-02-15 14:30. Plan: implement SubstringClassifier with word-boundary matching"
```

**When closing** — summarize what was done:
```bash
br comments add <id> "Done. Added SubstringClassifier in matcher.rs (120 lines, 8 tests). Uses regex word boundaries, respects enable_implicit_string flag."
```

**If interrupted** — leave a checkpoint comment before stopping:
```bash
br comments add <id> "Paused 2026-02-15 16:00. Progress: trait defined, basic matching works. TODO: word-boundary edge cases, tests for Unicode. Branch: task/t016-substring-classifier (3 commits)."
```

This lets another agent pick up an `in_progress` issue by reading the comments
to understand what's done and what remains.

### Rules

- **`br` writes only on main**: `br update`, `br close`, `br create`, `br sync`
  must run from the main worktree. This keeps `.beads/` in sync with main.
- **Worktrees are code-only**: Read commands (`br show`, `br ready`, `br list`)
  are fine anywhere, but their output may be stale in a worktree.
- **Commit `.beads/` changes immediately**: Every `br` mutation should be
  followed by `br sync --flush-only && git add .beads/ && git commit`.
- **Never leave dirty `.beads/`**: If `git status` shows `.beads/` modified,
  commit it before switching tasks or ending a session.
- **`br sync --full` unsupported**: use `--flush-only` or `--import-only`.
- **Worktree removal needs `--force`**: beads files trigger git's modified-files guard.
- Keep commits atomic: commit only the files you touched and list each path explicitly. For tracked files run git commit -m "<scoped message>" -- path/to/file1 path/to/file2. For brand-new files, use the one-liner git restore --staged :/ && git add "path/to/file1" "path/to/file2" && git commit -m "<scoped message>" -- path/to/file1 path/to/file2.

## Specify-on-Ready

When an issue becomes unblocked, a reviewer writes a detailed implementer
prompt before any agent claims it. See `docs/process-specify-on-ready.md` for the
full procedure.

**The flywheel:**
1. Agent closes an issue → `br close` + commit
2. `br ready` reveals newly unblocked issues
3. Reviewer writes `docs/process-prompt-<task-id>-<name>.md` for each
4. Agent claims and implements using the prompt as context

Implementer agents: check `docs/process-prompt-*.md` for your task's prompt before
starting work. If no prompt exists, flag it — don't start without one.

## Project Reference

- Product spec (current): `spec/product-current.md`
- Product roadmap/tasks (current): `spec/roadmap-current.md`
- Design decisions: `spec/decisions.md`
- Known domain gaps: `spec/gaps.md`
- Implementer prompts: `docs/process-prompt-*.md`
- Specify-on-ready procedure: `docs/process-specify-on-ready.md`
- Issue creation procedure: `docs/process-issue-creation-procedure.md`
- External reference material: `reference/`
- Local-only project: no pull requests, merge branches directly to main
