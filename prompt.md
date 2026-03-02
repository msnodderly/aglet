# Autonomous Agent Workflow

**Project:** aglet (Rust CLI/TUI app)

## Prime Directive

Select ONE task. Complete it. Stop.

Do not start additional tasks after completing your assigned work.

> [!WARNING]
> Work is **not complete** until commits are pushed to remote (`git push` / `git push origin HEAD:main` when explicitly requested). Unpushed work in abandoned worktrees/branches is treated as at-risk and incomplete.

---

## Session Start

1. **Read context**:
   - `AGENTS.md`
   - `docs/` files relevant to the selected issue (for example `docs/implement_plan.md`)

2. **Pick task**: run open-item listing and select the SINGLE highest-priority item that is not already in progress.

   ```bash
   git switch main
   ./scripts/list-open-project-items.sh aglet
   ```

   **IMPORTANT:** Skip items already categorized as `In Progress`.

3. **Claim task**:

   ```bash
   cargo run --bin agenda-cli -- --db aglet-features.ag category assign <ITEM_ID> "In Progress"
   ```

4. **Create worktree** (isolated branch/workdir):

   ```bash
   git worktree add ../aglet-<ITEM_ID>-<SHORT_SLUG> -b codex/<ITEM_ID>-<SHORT_SLUG> main
   cd ../aglet-<ITEM_ID>-<SHORT_SLUG>
   ```

5. **Read item details**:

   ```bash
   cargo run --bin agenda-cli -- --db /Users/mds/src/aglet/aglet-features.ag show <ITEM_ID>
   ```

6. **Create plan and save as note** (append/update note with concrete implementation steps):

   ```bash
   new_note=$(cat <<'PLAN'
   <EXISTING_NOTE_TEXT>

   Implementation plan (<YYYY-MM-DD>):
   1. <step 1>
   2. <step 2>
   3. <step 3>
   4. <step 4>
   PLAN
   )

   cargo run --bin agenda-cli -- --db /Users/mds/src/aglet/aglet-features.ag edit <ITEM_ID> --note "$new_note"
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
   <UPDATED_NOTE_WITH_PLAN_AND_IMPLEMENTATION_SUMMARY>
   DONE
   )

   cargo run --bin agenda-cli -- --db /Users/mds/src/aglet/aglet-features.ag edit <ITEM_ID> --note "$completion_note"
   cargo run --bin agenda-cli -- --db /Users/mds/src/aglet/aglet-features.ag category assign <ITEM_ID> "Complete"
   ```

4. **Create PR** (for normal code-review flow):

   ```bash
   gh pr create --title "<TITLE>" --body "<BODY>"
   ```

5. **If explicitly asked to merge directly to `main`**:

   ```bash
   git fetch origin main
   git rebase origin/main
   git push origin HEAD:main
   ```

6. **STOP** - do not pick another task.

---

## Exact Command Sequence (Copy/Paste Template)

```bash
# 0) Start on main in base repo
cd /Users/mds/src/aglet
git switch main

# 1) List open aglet items and pick one ITEM_ID
./scripts/list-open-project-items.sh aglet

# 2) Claim item
cargo run --bin agenda-cli -- --db aglet-features.ag category assign <ITEM_ID> "In Progress"

# 3) Create isolated worktree + branch
git worktree add ../aglet-<ITEM_ID>-<SHORT_SLUG> -b codex/<ITEM_ID>-<SHORT_SLUG> main
cd ../aglet-<ITEM_ID>-<SHORT_SLUG>

# 4) Read item details
cargo run --bin agenda-cli -- --db /Users/mds/src/aglet/aglet-features.ag show <ITEM_ID>

# 5) Save implementation plan to note
new_note=$(cat <<'PLAN'
<EXISTING_NOTE_TEXT>

Implementation plan (<YYYY-MM-DD>):
1. <step 1>
2. <step 2>
3. <step 3>
4. <step 4>
PLAN
)
cargo run --bin agenda-cli -- --db /Users/mds/src/aglet/aglet-features.ag edit <ITEM_ID> --note "$new_note"

# 6) Implement + verify
cargo fmt
cargo clippy --all-targets --all-features
cargo test

# 7) Commit
git add <FILES>
git commit -m "<CLEAR_COMMIT_MESSAGE>"

# 8) Push branch first
git push -u origin <BRANCH_NAME>

# 9) Mark complete + update note (only after push succeeds)
completion_note=$(cat <<'DONE'
<UPDATED_NOTE_WITH_PLAN_AND_IMPLEMENTATION_SUMMARY>
DONE
)
cargo run --bin agenda-cli -- --db /Users/mds/src/aglet/aglet-features.ag edit <ITEM_ID> --note "$completion_note"
cargo run --bin agenda-cli -- --db /Users/mds/src/aglet/aglet-features.ag category assign <ITEM_ID> "Complete"

# 10) Create PR unless direct-merge is explicitly requested
# gh pr create --title "<TITLE>" --body "<BODY>"

# 11) Only when explicitly requested: push directly to main
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
- [ ] `agenda-cli show <ITEM_ID>` reflects latest plan/completion note and `Complete` status (set after push)
- [ ] `AGENTS.md` updated for newly discovered gotchas (if any)
