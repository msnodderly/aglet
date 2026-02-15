# Task: T018 — Fixed-Point Loop

## Context

T017 built `process_item` — a single pass over the category hierarchy that
evaluates conditions and fires actions. But a single pass isn't enough.

Consider: category "Meetings" has an Assign action targeting "Calendar".
Category "Calendar" has a Profile condition: "if in Calendar AND Urgent,
assign to Reminders." In a single pass, the engine assigns the item to
"Meetings" (text match) and fires the action to assign to "Calendar." But
the Profile on "Calendar" → "Reminders" never gets evaluated because
"Calendar" was already walked before the action fired.

The fixed-point loop solves this. It re-runs the hierarchy walk until a
pass produces no new assignments. Each pass picks up assignments created by
the previous pass's actions, letting cascades resolve fully.

The loop must also handle termination — pathological rule configurations
could cascade indefinitely. And it must defer Remove actions until the
cascade completes, so that mid-cascade removals don't destabilize the
evaluation.

## What to read

1. `spec/mvp-spec.md` §2.4, steps 6-8 — the fixed-point and termination spec
2. `spec/design-decisions.md` §3 (Sticky assignments) — why termination works
3. `crates/agenda-core/src/engine.rs` — T017's implementation. Understand what
   `process_item` returns, what state it tracks, and where the extension
   points are.
4. `crates/agenda-core/src/model.rs` — `Action::Remove`, `Assignment`

## What to build

**File**: `crates/agenda-core/src/engine.rs` (extend T017's code)

### The loop

Wrap T017's single-pass logic in a loop that:

1. Runs a pass over the full hierarchy (T017's walk).
2. Checks whether the pass produced any new assignments.
3. If yes, runs another pass. If no, stops — fixed point reached.
4. Hard cap at **10 passes**. If the loop hasn't converged after 10 passes,
   stop and treat the current state as final. Don't error — just stop.
   (In practice, most items converge in 1-2 passes.)

### Cycle detection

Track which `(ItemId, CategoryId)` pairs have already been assigned during
this processing run. When the engine encounters a pair it's already seen,
skip it — don't re-assign, don't re-fire actions. This is the core
termination guarantee.

The set of seen pairs should persist across passes within a single
`process_item` call, not just within a single pass. This prevents the loop
from re-doing work each pass.

### Deferred Remove actions

T017 should already be collecting Remove actions rather than applying them
immediately. Your job is to apply them **after the loop exits**:

1. During each pass, Remove actions accumulate (target category IDs to
   unassign from).
2. After the final pass (convergence or max passes), apply all accumulated
   removals via `store.unassign_item()`.
3. Removals do NOT trigger re-evaluation. Once the cascade is done, the
   removals are applied as a final step. No further passes.

**Why defer?** If a Remove action fired mid-cascade, it could unassign an
item from a category whose Profile condition another category depends on.
The cascade would become order-dependent and unpredictable. Deferring makes
the cascade deterministic: first add everything, then remove.

### What "new assignment" means

A pass produces a "new assignment" if `store.assign_item()` was called for
a pair not already in the seen set. If every assignment in a pass was
skipped (already seen), the loop should stop — even if Remove actions were
collected, since removals don't trigger re-evaluation.

## Tests to write

1. **Single-pass convergence**: Category "Sarah" matches item text. No
   actions. Loop runs one pass, no new assignments on second check, stops.

2. **Two-pass cascade**: Category "Meetings" matches text, has Assign action
   → "Calendar". Second pass evaluates "Calendar" (now assigned). Verify both
   assignments exist.

3. **Profile cascade across passes**: "Meetings" matches text → Assign action
   → "Calendar". "Reminders" has Profile: include "Calendar". First pass
   assigns "Meetings" + "Calendar" (via action). Second pass: "Reminders"
   Profile matches. Third pass: no new assignments → stop. Verify all three
   assigned.

4. **Max passes cap**: Set up a configuration where each pass produces at
   least one new assignment (chain of N categories with Assign actions, N > 10).
   Verify the loop stops at 10 passes without error. Assignments made within
   those 10 passes should be present.

5. **Cycle detection**: Category A has Assign action → B. Category B has
   Assign action → A. Item matches A by text. Verify: A and B both assigned,
   loop terminates (doesn't spin), no errors.

6. **Deferred removes applied**: Category "Active" matches text, has Remove
   action targeting "Backlog". Item is manually assigned to "Backlog" before
   engine runs. After engine: item is assigned to "Active", item is
   unassigned from "Backlog."

7. **Deferred removes don't re-trigger**: Same as above. After removing from
   "Backlog", verify no additional pass runs. The removal is a final step.

8. **Idempotent re-run**: Run the engine on the same item twice. Second run
   should do nothing (all pairs already assigned, zero new assignments, zero
   passes of real work).

## What NOT to do

- **Don't implement subsumption** — T019. Don't walk parent chains.
- **Don't implement mutual exclusion** — T020. Don't check `is_exclusive`.
- **Don't implement `evaluate_all_items`** — T021.
- **Don't wire into store operations** — T022.
- **Don't error on max passes** — just stop silently. The user doesn't need
  to know the loop hit its cap.

## Workflow

Follow `AGENTS.md`. Issue ID: `bd-km0`.

```bash
# Claim on main:
#   br update bd-km0 --status in_progress
#   br comments add bd-km0 "Claimed <date>. Plan: <your approach>"
#   br sync --flush-only && git add .beads/ && git commit -m "br sync: Claim bd-km0"

git checkout -b task/t018-fixed-point-loop

# Extend crates/agenda-core/src/engine.rs
# Run: cargo test -p agenda-core
# Run: cargo clippy -p agenda-core
# Commit on branch

# Merge and close happen on main per AGENTS.md
```

## Definition of done

- [ ] `process_item` (or a wrapper) runs the hierarchy walk in a loop
- [ ] Loop stops when a pass produces no new assignments
- [ ] Loop stops at 10 passes max (no error)
- [ ] `(ItemId, CategoryId)` seen-set prevents re-processing across passes
- [ ] Remove actions are collected during cascade, applied after loop exits
- [ ] Removals don't trigger re-evaluation
- [ ] All tests pass — `cargo test -p agenda-core`
- [ ] `cargo clippy -p agenda-core` clean
- [ ] Changes limited to `crates/agenda-core/src/engine.rs`
