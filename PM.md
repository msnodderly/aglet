# PM & Grooming Process (Aglet)

## Overview

This document describes issue grooming and project management for aglet.

Goal: break down work into items suitable for parallel coding agents, with clear dependencies, acceptance criteria, and unambiguous execution order.

---

## Issue Tracking System

This project tracks work in `aglet-features.ag` using `agenda-cli`.
`aglet-features.ag` is the canonical PM backlog database for Aglet.

Do not use `feature-requests.ag` for Aglet PM grooming, triage, or execution
tracking. Every PM command in this doc should explicitly target:

```bash
--db aglet-features.ag
```

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
cargo run --bin agenda-cli -- --db aglet-features.ag category set-value <ITEM_ID> Complexity <1|2|3|5|7>

# Atomic claim (preferred for multi-agent task pickup)
cargo run --bin agenda-cli -- --db aglet-features.ag claim <ITEM_ID>
cargo run --bin agenda-cli -- --db aglet-features.ag claim <ITEM_ID> --must-not-have "In Progress" --must-not-have "Complete" --must-not-have "Waiting/Blocked"

# Dependency links
cargo run --bin agenda-cli -- --db aglet-features.ag link blocks <BLOCKER_ITEM_ID> <BLOCKED_ITEM_ID>
cargo run --bin agenda-cli -- --db aglet-features.ag link depends-on <ITEM_ID> <DEPENDS_ON_ITEM_ID>
cargo run --bin agenda-cli -- --db aglet-features.ag unlink blocks <BLOCKER_ITEM_ID> <BLOCKED_ITEM_ID>
cargo run --bin agenda-cli -- --db aglet-features.ag unlink depends-on <ITEM_ID> <DEPENDS_ON_ITEM_ID>
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

# Aglet open items missing complexity score (targeted PM cleanup batch)
cargo run --bin agenda-cli -- --db aglet-features.ag list \
  --any-category Aglet \
  --exclude-category Done \
  --exclude-category Complete \
  --exclude-category Complexity \
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
- `Complexity` (numeric): `1`, `2`, `3`, `5`, or `7`

Status guidance:

- Use `Ready` as the default actionable queue status.
- Use `Needs Refinement` to explicitly flag items requiring PM grooming before implementation.
- Use `Next Action` only for legacy continuity while it still exists in the taxonomy.

Complexity guidance:

- `1`: trivial change; suitable for smaller-capability agents
- `2`: small change; suitable for smaller-capability agents
- `3`: medium change; frontier agent required, no full architecture review required
- `5`: large/complex change; strongest frontier agents only, requires detailed implementation plan and architecture guide
- `7`: too large/unclear for single story; must be broken into smaller items before implementation

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
- Complexity score assigned (`1|2|3|5|7`)
- Work is implementable in one focused session, or decomposed
- Dependencies are correctly linked and directionally correct

If an item is missing acceptance criteria, required categories (including complexity), or has unclear scope, assign `Needs Refinement` until groomed.
If an item is scored `7`, split it into smaller stories and rescore the child stories before marking them `Ready`.

### 2.1 Complexity Cleanup Pass (Learned Workflow)

Use this focused pass whenever backlog hygiene drifts:

1. List only open Aglet items missing complexity:
```bash
cargo run --bin agenda-cli -- --db aglet-features.ag list \
  --any-category Aglet \
  --exclude-category Done \
  --exclude-category Complete \
  --exclude-category Complexity \
  --view "All Items" \
  --sort Priority
```
2. For each result, add missing required categories (`Issue type`, `Priority`, `Status`) and tighten note/acceptance criteria.
3. Set complexity only if missing; never overwrite existing scores unless explicitly requested.
4. Move to `Ready` only when implementation-ready. Keep `Needs Refinement` for large/underspecified items.
5. Treat `Complexity=5` as requiring a detailed plan/architecture notes before implementation.
6. Treat `Complexity=7` as mandatory decomposition before implementation.

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

#### Set Complexity Score

```bash
cargo run --bin agenda-cli -- --db aglet-features.ag category set-value <ITEM_ID> Complexity <1|2|3|5|7>
```

Use this on every groomed item that does not already have a complexity value.
Do not overwrite an existing complexity score unless a PM explicitly requests a rescore.
If score is `7`, do not leave the item implementation-ready; break it down first.

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
cargo run --bin agenda-cli -- --db aglet-features.ag list --any-category Aglet --exclude-category Done --exclude-category Complete --exclude-category Complexity --view "All Items" --sort Priority

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
- [ ] Required categories assigned (Issue type, Priority, Project, Status, Complexity)
- [ ] Complexity score set to one of `1|2|3|5|7`
- [ ] Any `Complexity=7` item is split into smaller items before implementation
- [ ] Items needing PM work explicitly marked `Needs Refinement`
- [ ] Refined items moved to `Ready` when implementation-ready
- [ ] Large items split into subtasks where needed
- [ ] Dependency direction verified (`blocks <BLOCKER> <BLOCKED>`)
- [ ] Blocked items marked `Waiting/Blocked` with explicit reason
- [ ] `docs/questions.md` updated where needed
- [ ] Open Aglet items missing complexity reviewed (or none found)
- [ ] `aglet-features.ag` + docs committed
- [ ] Branch pushed and PR opened

---

## Smoke Test: `agenda claim`

Use this against `aglet-features.ag` to validate claim behavior quickly.

```bash
# 1) Create a disposable test item (capture the UUID from output)
cargo run --bin agenda-cli -- --db aglet-features.ag add "Smoke test claim"

# 2) Seed it as Ready
cargo run --bin agenda-cli -- --db aglet-features.ag category assign <ITEM_ID> "Ready"

# 3) Claim once (should succeed)
cargo run --bin agenda-cli -- --db aglet-features.ag claim <ITEM_ID>

# 4) Claim again (should fail due to precondition, non-zero exit)
cargo run --bin agenda-cli -- --db aglet-features.ag claim <ITEM_ID>

# 5) Verify assignments include In Progress and not Complete
cargo run --bin agenda-cli -- --db aglet-features.ag show <ITEM_ID>
```

Expected outcomes:
- Step 3 prints `claimed item ... to category In Progress`.
- Step 4 prints `error: ... claim precondition failed ...` and exits non-zero.
- Step 5 shows `In Progress` under `assignments:`.
