---
title: Agent Workflow
updated: 2026-03-21
---

# Autonomous Agent Workflow

**Project:** aglet (Rust CLI/TUI app)

## Prime Directive

Select ONE task. Complete it. Stop.

Do not start additional tasks after completing your assigned work.

> [!WARNING]
> Work is **not complete** until commits are pushed to remote (`git push` / `git push origin HEAD:main` when explicitly requested). Unpushed work in worktrees/branches is incomplete.

---

## Session Start

1. **Read context**:
   - `AGENTS.md`
   - `docs/` files relevant to the selected issue (for example `docs/implement_plan.md`)

2. **Pick task**: run open-item listing and select the SINGLE highest-priority item that is not already in progress, while respecting model complexity limits.

   ```bash
   git switch main
   ./scripts/list-open-project-items.sh aglet
   ```

   **Query candidates by complexity (required):**
   - Standard/unknown model lane (`1,2`):
   ```bash
   cargo run --bin agenda-cli -- --db /Users/mds/src/aglet/aglet-features.ag list \
     --any-category Aglet \
     --exclude-category Done \
     --exclude-category Complete \
     --exclude-category "In Progress" \
     --exclude-category "Waiting/Blocked" \
     --value-in Complexity 1,2 \
     --sort Priority
   ```
   Equivalent low-lane form:
   ```bash
   cargo run --bin agenda-cli -- --db /Users/mds/src/aglet/aglet-features.ag list \
     --any-category Aglet \
     --exclude-category Done \
     --exclude-category Complete \
     --exclude-category "In Progress" \
     --exclude-category "Waiting/Blocked" \
     --value-max Complexity 2 \
     --sort Priority
   ```
   - High-capability lane (`1,2,3,5`, if the model is any `Opus` or `GPT-5/Codex` variant):
   ```bash
   cargo run --bin agenda-cli -- --db /Users/mds/src/aglet/aglet-features.ag list \
     --any-category Aglet \
     --exclude-category Done \
     --exclude-category Complete \
     --exclude-category "In Progress" \
     --exclude-category "Waiting/Blocked" \
     --value-in Complexity 1,2,3,5 \
     --sort Priority
   ```

   **Selection rules (priority + complexity):**
   - Pick by **highest priority first**, but only from tasks in your allowed complexity lane.
   - If the model is any `Opus` or `GPT-5/Codex` variant: allowed complexity is `1, 2, 3, 5`.
   - Otherwise: allowed complexity is `1, 2` only.
   - Never pick `Complexity=7` for implementation; it must be split into smaller stories first.
   - If complexity is missing or cannot be confirmed from CLI output, skip that item and pick the next candidate.

   **IMPORTANT:** Skip items already categorized as `In Progress`.

3. **Claim task**:

   ```bash
   cargo run --bin agenda-cli -- --db aglet-features.ag claim <ITEM_ID>
   ```

4. **Create worktree** (isolated branch/workdir):

   ```bash
   git worktree add ../aglet-<ITEM_ID>-<SHORT_SLUG> -b codex/<ITEM_ID>-<SHORT_SLUG> main
   cd ../aglet-<ITEM_ID>-<SHORT_SLUG>
   ```

5. **Record claim metadata in issue note** (single-line update with branch + worktree):

   ```bash
   branch_name=$(git branch --show-current)
   worktree_path=$(pwd)
   claim_note=$(printf 'Claimed %s: branch=%s worktree=%s\n' "$(date +%F)" "$branch_name" "$worktree_path")
   cargo run --bin agenda-cli -- --db /Users/mds/src/aglet/aglet-features.ag edit <ITEM_ID> --append-note "$claim_note"
   ```

6. **Read item details**:

   ```bash
   cargo run --bin agenda-cli -- --db /Users/mds/src/aglet/aglet-features.ag show <ITEM_ID>
   ```

7. **Create plan and save as note** (append concrete implementation steps):

   ```bash
   plan_note=$(cat <<'PLAN'
   Implementation plan (<YYYY-MM-DD>):
   1. <step 1>
   2. <step 2>
   3. <step 3>
   4. <step 4>
   PLAN
   )
   cargo run --bin agenda-cli -- --db /Users/mds/src/aglet/aglet-features.ag edit <ITEM_ID> --append-note "$plan_note"
   ```

---

## Implementation

### Before Coding

- Confirm scope from issue note and acceptance criteria.
- For larger changes, add a design note in `docs/plans/`.

### While Coding

- Work only on the selected item.
- Follow existing Rust/project patterns.
- Commit in logical chunks.
- Run checks regularly and resolve all lints:

```bash
cargo fmt
cargo clippy --all-targets --all-features
cargo test
```

(Use targeted tests when faster, but ensure adequate coverage for changed behavior. Address all Clippy findings before finishing.)

### After Coding

- Update `AGENTS.md` with new gotchas/patterns discovered.
- Re-run relevant tests/build checks.

---

## Session End

> [!WARNING]
> Do not end the session before pushing. A local-only commit in a worktree is not considered completed work.

1. **Commit changes**:

   ```bash
   git add <FILES>
   git commit -m "<CLEAR_COMMIT_MESSAGE>"
   ```

2. **Push code branch**:

   ```bash
   git push -u origin <BRANCH_NAME>
   ```

3. **Mark issue complete + add completion note (only after push succeeds)**:

   ```bash
   completion_note=$(cat <<'DONE'
   Implementation summary (<YYYY-MM-DD>):
   - <what changed>
   - <tests/checks run>
   - <known caveats, if any>
   DONE
   )
   cargo run --bin agenda-cli -- --db /Users/mds/src/aglet/aglet-features.ag edit <ITEM_ID> --append-note "$completion_note"
   cargo run --bin agenda-cli -- --db /Users/mds/src/aglet/aglet-features.ag category assign <ITEM_ID> "Complete"
   ```

4. **Create PR** (for normal code-review flow):

   ```bash
   pr_body_file=$(mktemp /tmp/aglet-pr-body-XXXX.md)
   cat > "$pr_body_file" <<'EOF'
   <BODY>
   EOF
   gh pr create --title "<TITLE>" --body-file "$pr_body_file"
   ```

5. **If explicitly asked to merge directly to `main`**:

   ```bash
   git fetch origin main
   git rebase origin/main
   git push origin HEAD:main
   ```

6. **Docs update** (required before handoff):
   - For any plan touched this session: update its YAML frontmatter `status`
     field (add `shipped: YYYY-MM-DD` if the plan shipped this session)
   - For any non-trivial design choice made this session: create a decision
     record in `docs/decisions/<topic>.md` with rationale, tradeoffs, and outcome
   - If a proposal was accepted/rejected: update its `status` field and move
     to `docs/decisions/` if accepted (add `origin:` pointing to original path)

7. **STOP** - do not pick another task.

---

## Exact Command Sequence (Copy/Paste Template)

```bash
# 0) Start on main in base repo
cd /Users/mds/src/aglet
git switch main

# 1) List open aglet items and pick one ITEM_ID
./scripts/list-open-project-items.sh aglet

# 1b) Query complexity-scoped candidates and choose ITEM_ID from your allowed lane
# Standard/unknown model: complexity 1 or 2
cargo run --bin agenda-cli -- --db /Users/mds/src/aglet/aglet-features.ag list \
  --any-category Aglet \
  --exclude-category Done \
  --exclude-category Complete \
  --exclude-category "In Progress" \
  --exclude-category "Waiting/Blocked" \
  --value-in Complexity 1,2 \
  --sort Priority

# High-capability model (any Opus or GPT-5/Codex variant): complexity 1,2,3,5
# Replace the value-in line with:
# --value-in Complexity 1,2,3,5

# 2) Claim item
cargo run --bin agenda-cli -- --db aglet-features.ag claim <ITEM_ID>

# 3) Create isolated worktree + branch
git worktree add ../aglet-<ITEM_ID>-<SHORT_SLUG> -b codex/<ITEM_ID>-<SHORT_SLUG> main
cd ../aglet-<ITEM_ID>-<SHORT_SLUG>

# 4) Add claim metadata to note (branch + worktree)
branch_name=$(git branch --show-current)
worktree_path=$(pwd)
claim_note=$(printf 'Claimed %s: branch=%s worktree=%s\n' "$(date +%F)" "$branch_name" "$worktree_path")
cargo run --bin agenda-cli -- --db /Users/mds/src/aglet/aglet-features.ag edit <ITEM_ID> --append-note "$claim_note"

# 5) Read item details
cargo run --bin agenda-cli -- --db /Users/mds/src/aglet/aglet-features.ag show <ITEM_ID>

# 6) Save implementation plan to note
plan_note=$(cat <<'PLAN'
Implementation plan (<YYYY-MM-DD>):
1. <step 1>
2. <step 2>
3. <step 3>
4. <step 4>
PLAN
)
cargo run --bin agenda-cli -- --db /Users/mds/src/aglet/aglet-features.ag edit <ITEM_ID> --append-note "$plan_note"

# 7) Implement + verify
cargo fmt
cargo clippy --all-targets --all-features
cargo test

# 8) Commit
git add <FILES>
git commit -m "<CLEAR_COMMIT_MESSAGE>"

# 9) Push branch first
git push -u origin <BRANCH_NAME>

# 10) Mark complete + update note (only after push succeeds)
completion_note=$(cat <<'DONE'
Implementation summary (<YYYY-MM-DD>):
- <what changed>
- <tests/checks run>
- <known caveats, if any>
DONE
)
cargo run --bin agenda-cli -- --db /Users/mds/src/aglet/aglet-features.ag edit <ITEM_ID> --append-note "$completion_note"
cargo run --bin agenda-cli -- --db /Users/mds/src/aglet/aglet-features.ag category assign <ITEM_ID> "Complete"

# 11) Create PR unless direct-merge is explicitly requested
# pr_body_file=$(mktemp /tmp/aglet-pr-body-XXXX.md)
# cat > "$pr_body_file" <<'EOF'
# <BODY>
# EOF
# gh pr create --title "<TITLE>" --body-file "$pr_body_file"

# 12) Only when explicitly requested: push directly to main
git fetch origin main
git rebase origin/main
git push origin HEAD:main
```

---

## Verification Checklist

Before finishing, confirm:

- [ ] `cargo fmt` and `cargo test` pass
- [ ] `cargo clippy --all-targets --all-features` runs clean and all findings are addressed
- [ ] `git status` is clean (or only expected uncommitted work if not finishing yet)
- [ ] Branch has been pushed to remote (`git push` completed successfully)
- [ ] `agenda-cli show <ITEM_ID>` reflects latest plan/completion note and includes `Complete` in `assignments:` (set after push)
- [ ] `AGENTS.md` updated for newly discovered gotchas (if any)
