# PM & Grooming Process (Aglet)

## Overview

This document describes issue grooming and project management for aglet.

Goal: break down work into items suitable for parallel coding agents, with clear dependencies, acceptance criteria, and unambiguous execution order.

---

## Issue Tracking System

This project tracks work in `aglet-features.ag` using `agenda-cli`.

Use full UUIDs for item commands.

### Core Commands

```bash
# Aglet-only backlog views
./scripts/list-open-project-items.sh aglet
cargo run --bin agenda-cli -- --db aglet-features.ag list --any-category Aglet --view "All Items" --sort Priority
cargo run --bin agenda-cli -- --db aglet-features.ag show <ITEM_ID>

# Create/edit
cargo run --bin agenda-cli -- --db aglet-features.ag add "<TITLE>" --note "<NOTE>"
cargo run --bin agenda-cli -- --db aglet-features.ag edit <ITEM_ID> "<NEW_TITLE>"
cargo run --bin agenda-cli -- --db aglet-features.ag edit <ITEM_ID> --note "<NOTE>"

# Categories
cargo run --bin agenda-cli -- --db aglet-features.ag category list
cargo run --bin agenda-cli -- --db aglet-features.ag category assign <ITEM_ID> "<CATEGORY>"
cargo run --bin agenda-cli -- --db aglet-features.ag category unassign <ITEM_ID> "<CATEGORY>"

# Dependency links
cargo run --bin agenda-cli -- --db aglet-features.ag link blocks <BLOCKER_ITEM_ID> <BLOCKED_ITEM_ID>
cargo run --bin agenda-cli -- --db aglet-features.ag link depends-on <ITEM_ID> <DEPENDS_ON_ITEM_ID>
cargo run --bin agenda-cli -- --db aglet-features.ag link unlink-blocks <BLOCKER_ITEM_ID> <BLOCKED_ITEM_ID>
cargo run --bin agenda-cli -- --db aglet-features.ag link unlink-depends-on <ITEM_ID> <DEPENDS_ON_ITEM_ID>
```

### Aglet-Only Query Patterns

```bash
# Ready candidates (open, not blocked, not currently being worked)
./scripts/list-open-project-items.sh aglet

# All open Aglet work (includes In Progress and Waiting/Blocked; excludes completed)
cargo run --bin agenda-cli -- --db aglet-features.ag list \
  --any-category Aglet \
  --exclude-category Done \
  --exclude-category Complete \
  --view "All Items" \
  --sort Priority

# Aglet items flagged for PM grooming/refinement
cargo run --bin agenda-cli -- --db aglet-features.ag list \
  --any-category Aglet \
  --category "Needs Refinement" \
  --view "All Items" \
  --sort Priority

# Aglet items currently in progress
cargo run --bin agenda-cli -- --db aglet-features.ag list \
  --any-category Aglet \
  --category "In Progress" \
  --view "All Items" \
  --sort Priority

# Aglet items currently blocked
cargo run --bin agenda-cli -- --db aglet-features.ag list \
  --any-category Aglet \
  --category "Waiting/Blocked" \
  --view "All Items" \
  --sort Priority
```

### Helper Script (Preferred for New Items)

```bash
./scripts/add-aglet-issue.sh "<TITLE>" "<NOTE>" "<PRIORITY>" "<STATUS>" "<ISSUE_TYPE>" "Aglet" aglet-features.ag
```

---

## Required Category Model (`aglet-features.ag`)

Every item should include all of these:

- `Issue type`: `Bug`, `Idea`, or `Feature request`
- `Priority`: `Critical`, `High`, `Normal`, or `Low`
- `Software Project(s)`: `Aglet` (plus `NeoNV` only if truly cross-project)
- `Status`: `Needs Refinement`, `Ready`, `In Progress`, `Waiting/Blocked`, or `Complete`

Status guidance:

- Use `Ready` as the default actionable queue status.
- Use `Needs Refinement` to explicitly flag items requiring PM grooming before implementation.
- Use `Next Action` only for legacy continuity while it still exists in the taxonomy.

---

## Grooming Process

### 1. Preparation

```bash
cd /Users/mds/src/aglet
git switch main
git pull --ff-only
git switch -c codex/pm-grooming-$(date +%Y-%m-%d)

./scripts/list-open-project-items.sh aglet
cargo run --bin agenda-cli -- --db aglet-features.ag list --any-category Aglet --exclude-category Done --exclude-category Complete --view "All Items" --sort Priority
cargo run --bin agenda-cli -- --db aglet-features.ag category list
```

### 2. Review Criteria

For each item, verify:

- Clear, specific title
- Note explains WHAT and WHY
- Acceptance criteria present in note
- Correct issue type, priority, project, and status categories
- Work is implementable in one focused session, or decomposed
- Dependencies are correctly linked and directionally correct

If an item is missing acceptance criteria, required categories, or has unclear scope, assign `Needs Refinement` until groomed.

### 3. Common Grooming Actions

#### Improve an Item Note

```bash
cargo run --bin agenda-cli -- --db aglet-features.ag edit <ITEM_ID> --note "What: ...
Why: ...

Acceptance Criteria:
- [ ] ...
- [ ] ...

Notes:
- ..."
```

#### Break Down a Large Item

1. Create subtasks.
2. Link them so subtasks block parent.
3. Optionally sequence subtasks.

```bash
./scripts/add-aglet-issue.sh "<PREFIX>: Foundation" "..." "Normal" "Ready" "Feature request" "Aglet" aglet-features.ag
./scripts/add-aglet-issue.sh "<PREFIX>: Core logic" "..." "Normal" "Ready" "Feature request" "Aglet" aglet-features.ag
./scripts/add-aglet-issue.sh "<PREFIX>: Integration" "..." "Normal" "Ready" "Feature request" "Aglet" aglet-features.ag

# Parent depends on each subtask
cargo run --bin agenda-cli -- --db aglet-features.ag link blocks <SUBTASK1_ID> <PARENT_ID>
cargo run --bin agenda-cli -- --db aglet-features.ag link blocks <SUBTASK2_ID> <PARENT_ID>
cargo run --bin agenda-cli -- --db aglet-features.ag link blocks <SUBTASK3_ID> <PARENT_ID>

# Optional sequence
cargo run --bin agenda-cli -- --db aglet-features.ag link blocks <SUBTASK1_ID> <SUBTASK2_ID>
cargo run --bin agenda-cli -- --db aglet-features.ag link blocks <SUBTASK2_ID> <SUBTASK3_ID>
```

#### Mark an Item as Blocked

```bash
cargo run --bin agenda-cli -- --db aglet-features.ag category assign <ITEM_ID> "Waiting/Blocked"
```

Then update the note with explicit blocker details and required decision/input.

#### Mark an Item as Needs Refinement

```bash
cargo run --bin agenda-cli -- --db aglet-features.ag category assign <ITEM_ID> "Needs Refinement"
```

After grooming is complete, move it back to actionable:

```bash
cargo run --bin agenda-cli -- --db aglet-features.ag category assign <ITEM_ID> "Ready"
```

### 4. Dependency Direction (Critical)

Canonical blocker syntax:

```bash
cargo run --bin agenda-cli -- --db aglet-features.ag link blocks <BLOCKER_ITEM_ID> <BLOCKED_ITEM_ID>
```

Meaning: `BLOCKER_ITEM_ID` must be completed before `BLOCKED_ITEM_ID`.

Equivalent form:

```bash
cargo run --bin agenda-cli -- --db aglet-features.ag link depends-on <BLOCKED_ITEM_ID> <BLOCKER_ITEM_ID>
```

Always verify direction with:

```bash
cargo run --bin agenda-cli -- --db aglet-features.ag show <BLOCKER_ITEM_ID>
cargo run --bin agenda-cli -- --db aglet-features.ag show <BLOCKED_ITEM_ID>
```

### 5. Questions and Design Decisions

Track unresolved architectural questions in `docs/questions.md`.

When a decision blocks multiple items, create a dedicated design item and block implementation items on it.

### 6. Final Steps After Grooming

```bash
./scripts/list-open-project-items.sh aglet
cargo run --bin agenda-cli -- --db aglet-features.ag list --any-category Aglet --exclude-category Done --exclude-category Complete --view "All Items" --sort Priority
cargo run --bin agenda-cli -- --db aglet-features.ag list --any-category Aglet --category "Needs Refinement" --view "All Items" --sort Priority

git add PM.md docs/questions.md aglet-features.ag
git commit -m "PM grooming: <summary>"
git push -u origin HEAD
gh pr create --title "PM Grooming: <date>" --body "<summary>"
```

> [!WARNING]
> PM work is not complete until it is pushed. Local-only changes in abandoned worktrees/branches are incomplete and at risk of loss.

---

## Quality Indicators

### Healthy Backlog

- Items include clear intent and acceptance criteria
- Required categories are consistently assigned
- Dependencies clearly encode execution order
- `Ready` items are actionable
- `Needs Refinement` queue is small and actively burned down
- Blocked items explicitly state blockers

### Needs Grooming

- Missing required categories
- Vague titles/notes
- Overly large tasks without decomposition
- Confusing dependency graphs
- `Ready` items that are not actually implementable
- Many stale `Needs Refinement` items with no grooming progress

---

## Session Checklist

Before opening a grooming PR:

- [ ] Open Aglet backlog reviewed
- [ ] Groomed items have clear title/note/acceptance criteria
- [ ] Required categories assigned (Issue type, Priority, Project, Status)
- [ ] Items needing PM work explicitly marked `Needs Refinement`
- [ ] Refined items moved to `Ready` when implementation-ready
- [ ] Large items split into subtasks where needed
- [ ] Dependency direction verified (`blocks <BLOCKER> <BLOCKED>`)
- [ ] Blocked items marked `Waiting/Blocked` with explicit reason
- [ ] `docs/questions.md` updated where needed
- [ ] `aglet-features.ag` + docs committed
- [ ] Branch pushed and PR opened
