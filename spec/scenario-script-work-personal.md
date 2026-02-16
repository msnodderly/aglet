# Scenario Script: Work + Personal + Shared Priority

This is a command script to run manually. It has not been executed in this step.

## Scenario Goals

- Capture mixed personal and work tasks.
- Model 4 work projects with people and subtasks.
- Use one global `Priority` taxonomy across all contexts.
- Demonstrate both pre-existing categories and categories created later.
- Demonstrate implicit and manual assignment flows.
- Demonstrate exclusivity (`High`/`Medium`/`Low`) behavior.
- Demonstrate view filtering and category filtering.

## Assumptions

- You are in `/Users/mds/src/aglet-slc`.
- You are using `cargo run -q -p agenda-cli --`.
- Fresh DB path is used for this run.

## Commands To Type

```text
# Choose an isolated demo database
DB=/tmp/aglet-scenario-work-personal-$(date +%s).ag
rm -f "$DB"

# Baseline check (reserved categories are pre-existing: When, Entry, Done)
cargo run -q -p agenda-cli -- --db "$DB" category list

# ------------------------------------------------------------
# Phase 1: Create initial/global categories (pre-existing for later steps)
# ------------------------------------------------------------

# Top-level contexts
cargo run -q -p agenda-cli -- --db "$DB" category create Work
cargo run -q -p agenda-cli -- --db "$DB" category create Personal

# One global priority tree (reused across all work/personal items)
cargo run -q -p agenda-cli -- --db "$DB" category create Priority --exclusive
cargo run -q -p agenda-cli -- --db "$DB" category create High --parent Priority
cargo run -q -p agenda-cli -- --db "$DB" category create Medium --parent Priority
cargo run -q -p agenda-cli -- --db "$DB" category create Low --parent Priority

# Optional personal subcategories
cargo run -q -p agenda-cli -- --db "$DB" category create Home --parent Personal
cargo run -q -p agenda-cli -- --db "$DB" category create Finance --parent Personal

# ------------------------------------------------------------
# Phase 2: Capture items first (some categories not yet created)
# ------------------------------------------------------------

# Work projects and subtasks (project categories will be created later)
# Use parser-supported date phrases: today/tomorrow/yesterday, this/next <weekday>,
# Month Day[, Year], YYYY-MM-DD, YYYYMMDD, M/D/YY, and optional "at <time>".
cargo run -q -p agenda-cli -- --db "$DB" add "Project Atlas: Sarah finalize API contract tomorrow at 2pm"
cargo run -q -p agenda-cli -- --db "$DB" add "Project Atlas: Miguel draft rollout checklist"
cargo run -q -p agenda-cli -- --db "$DB" add "Project Borealis: Priya review migration plan next Monday at 10am"
cargo run -q -p agenda-cli -- --db "$DB" add "Project Borealis: Alex prepare test dataset"
cargo run -q -p agenda-cli -- --db "$DB" add "Project Cicada: Sarah and Priya incident rehearsal this Wednesday at 1pm"
cargo run -q -p agenda-cli -- --db "$DB" add "Project Delta: Miguel close open QA defects"
cargo run -q -p agenda-cli -- --db "$DB" add "Project Delta: Alex send stakeholder update"

# Personal tasks
cargo run -q -p agenda-cli -- --db "$DB" add "Clean out the garage tomorrow at 9am"
cargo run -q -p agenda-cli -- --db "$DB" add "Pay the bills next Monday at 8am"
cargo run -q -p agenda-cli -- --db "$DB" add "Buy groceries tomorrow at 6pm"
cargo run -q -p agenda-cli -- --db "$DB" add "Schedule annual physical May 25, 2026 at 3pm"

# Snapshot current state
cargo run -q -p agenda-cli -- --db "$DB" list --include-done

# ------------------------------------------------------------
# Phase 3: Create categories later (retroactive classification path)
# ------------------------------------------------------------

# Project categories created after items already exist
cargo run -q -p agenda-cli -- --db "$DB" category create "Project Atlas" --parent Work
cargo run -q -p agenda-cli -- --db "$DB" category create "Project Borealis" --parent Work
cargo run -q -p agenda-cli -- --db "$DB" category create "Project Cicada" --parent Work
cargo run -q -p agenda-cli -- --db "$DB" category create "Project Delta" --parent Work

# People categories created later; many existing items mention these names
cargo run -q -p agenda-cli -- --db "$DB" category create Sarah --parent Work
cargo run -q -p agenda-cli -- --db "$DB" category create Miguel --parent Work
cargo run -q -p agenda-cli -- --db "$DB" category create Priya --parent Work
cargo run -q -p agenda-cli -- --db "$DB" category create Alex --parent Work

# Inspect hierarchy after staged category creation
cargo run -q -p agenda-cli -- --db "$DB" category list

# ------------------------------------------------------------
# Phase 4: Manual assignment workflow
# ------------------------------------------------------------

# Capture IDs for targeted manual assignment
GARAGE_ID=$(cargo run -q -p agenda-cli -- --db "$DB" list --include-done | rg "Clean out the garage" | head -n1 | cut -d' ' -f1)
BILLS_ID=$(cargo run -q -p agenda-cli -- --db "$DB" list --include-done | rg "Pay the bills" | head -n1 | cut -d' ' -f1)
ATLAS_MIGUEL_ID=$(cargo run -q -p agenda-cli -- --db "$DB" list --include-done | rg "Project Atlas: Miguel draft rollout checklist" | head -n1 | cut -d' ' -f1)
DELTA_QA_ID=$(cargo run -q -p agenda-cli -- --db "$DB" list --include-done | rg "Project Delta: Miguel close open QA defects" | head -n1 | cut -d' ' -f1)

# Personal manual assignments
cargo run -q -p agenda-cli -- --db "$DB" category assign "$GARAGE_ID" Personal
cargo run -q -p agenda-cli -- --db "$DB" category assign "$GARAGE_ID" Home
cargo run -q -p agenda-cli -- --db "$DB" category assign "$BILLS_ID" Personal
cargo run -q -p agenda-cli -- --db "$DB" category assign "$BILLS_ID" Finance

# Priority assignments requested in scenario
cargo run -q -p agenda-cli -- --db "$DB" category assign "$GARAGE_ID" High
cargo run -q -p agenda-cli -- --db "$DB" category assign "$BILLS_ID" Low

# Work priority assignments
cargo run -q -p agenda-cli -- --db "$DB" category assign "$ATLAS_MIGUEL_ID" High
cargo run -q -p agenda-cli -- --db "$DB" category assign "$DELTA_QA_ID" Medium

# Demonstrate exclusivity replacement: High -> Medium -> High on one item
cargo run -q -p agenda-cli -- --db "$DB" category assign "$ATLAS_MIGUEL_ID" Medium
cargo run -q -p agenda-cli -- --db "$DB" category assign "$ATLAS_MIGUEL_ID" High

# ------------------------------------------------------------
# Phase 5: Reuse existing category vs duplicate create
# ------------------------------------------------------------

# Intentional duplicate-name create attempt (should fail with guidance)
cargo run -q -p agenda-cli -- --db "$DB" category create Priority --parent "Project Atlas"

# Correct behavior: assign existing global category instead of creating another
cargo run -q -p agenda-cli -- --db "$DB" category assign "$DELTA_QA_ID" Priority

# ------------------------------------------------------------
# Phase 6: Views for user workflows
# ------------------------------------------------------------

cargo run -q -p agenda-cli -- --db "$DB" view create "Work View" --include Work
cargo run -q -p agenda-cli -- --db "$DB" view create "Personal View" --include Personal
cargo run -q -p agenda-cli -- --db "$DB" view create "High Priority View" --include High
cargo run -q -p agenda-cli -- --db "$DB" view list

# Inspect contents
cargo run -q -p agenda-cli -- --db "$DB" view show "Work View"
cargo run -q -p agenda-cli -- --db "$DB" view show "Personal View"
cargo run -q -p agenda-cli -- --db "$DB" view show "High Priority View"

# Alternate filter checks
cargo run -q -p agenda-cli -- --db "$DB" list --category High --include-done
cargo run -q -p agenda-cli -- --db "$DB" list --category "Project Atlas" --include-done
cargo run -q -p agenda-cli -- --db "$DB" list --view "Work View" --include-done

# ------------------------------------------------------------
# Phase 7: End-to-end status lifecycle (optional)
# ------------------------------------------------------------

# Mark one item done, then inspect include/exclude done behavior
cargo run -q -p agenda-cli -- --db "$DB" done "$BILLS_ID"
cargo run -q -p agenda-cli -- --db "$DB" list
cargo run -q -p agenda-cli -- --db "$DB" list --include-done

# Delete and restore one item
cargo run -q -p agenda-cli -- --db "$DB" delete "$GARAGE_ID"
cargo run -q -p agenda-cli -- --db "$DB" deleted
LOG_ID=$(cargo run -q -p agenda-cli -- --db "$DB" deleted | head -n1 | cut -d' ' -f1)
cargo run -q -p agenda-cli -- --db "$DB" restore "$LOG_ID"

# Final state
cargo run -q -p agenda-cli -- --db "$DB" list --include-done
```

## What This Scenario Covers

- Mixed personal/work item capture.
- 4 work projects and people tagging.
- Subtask-style work item granularity.
- Pre-existing categories plus categories added later.
- Global category reuse (`Priority`) across all branches.
- Manual and implicit category assignment behavior.
- Exclusive priority switching behavior.
- Views, filtering, done/delete/restore lifecycle.
