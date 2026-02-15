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

### Troubleshooting

- `br <cmd> --help` for authoritative syntax
- `br ready` is scheduling truth; if it shows ready but `br show` lists blockers, check each dep's status directly
- **Beads + worktree close ordering**: Worktree removal can clobber `.beads/` state. The safe sequence is: merge branch → remove worktree → *then* `br close` + `br sync --flush-only` + commit `.beads/`. Always verify with `br show <id>` after.

## Session Learnings

- Product spec: `spec/mvp-spec.md`
- Product tasks: `spec/mvp-tasks.md`
- `br sync --full` unsupported; use `--flush-only` or `--import-only`
- Git worktree: `git worktree add <path> -b <branch>` (not `create`)
- Worktree shares parent's `.beads/` — `br` commands work from any worktree
- Worktree removal needs `--force` (beads files trigger modified-files guard)
- `.gitignore` should be created early (target/, IDE files) — prevents accidental commits
- Local-only project: no pull requests, merge branch directly to main
