# Exclusive Manual Assign Fix Validation Demo

This demo was run on branch `codex/s60-exclusive-manual-assign-fix`.

```text
# Demo DB path
$ echo /tmp/aglet-exclusive-fix-demo-1771269260.ag
/tmp/aglet-exclusive-fix-demo-1771269260.ag
# Reset database
$ rm -f /tmp/aglet-exclusive-fix-demo-1771269260.ag
# Create base hierarchy
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-exclusive-fix-demo-1771269260.ag category create Work
created category Work (processed_items=0, affected_items=0)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-exclusive-fix-demo-1771269260.ag category create 'Project X' --parent Work
created category Project X (processed_items=0, affected_items=0)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-exclusive-fix-demo-1771269260.ag category create Priority --parent Work --exclusive
created category Priority (processed_items=0, affected_items=0)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-exclusive-fix-demo-1771269260.ag category create High --parent Priority
created category High (processed_items=0, affected_items=0)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-exclusive-fix-demo-1771269260.ag category create Medium --parent Priority
created category Medium (processed_items=0, affected_items=0)
# Attempt duplicate category name in another branch (expected to fail)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-exclusive-fix-demo-1771269260.ag category create Priority --parent 'Project X'
error: category name already exists: Priority
# exit_code=1 (expected non-zero)
# Create second priority branch with unique names
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-exclusive-fix-demo-1771269260.ag category create 'Project X Priority' --parent 'Project X' --exclusive
created category Project X Priority (processed_items=0, affected_items=0)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-exclusive-fix-demo-1771269260.ag category create 'Project X High' --parent 'Project X Priority'
created category Project X High (processed_items=0, affected_items=0)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-exclusive-fix-demo-1771269260.ag category create 'Project X Medium' --parent 'Project X Priority'
created category Project X Medium (processed_items=0, affected_items=0)
# Show hierarchy
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-exclusive-fix-demo-1771269260.ag category list
- Done [no-implicit-string]
- Entry [no-implicit-string]
- When [no-implicit-string]
- Work
  - Project X
    - Project X Priority [exclusive]
      - Project X High
      - Project X Medium
  - Priority [exclusive]
    - High
    - Medium
# Create demo items
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-exclusive-fix-demo-1771269260.ag add 'Prepare launch plan for Project X'
created 5e00f545-c50b-426c-aeab-d4cc15a8a1d0
new_assignments=1
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-exclusive-fix-demo-1771269260.ag add 'Pay electricity bill'
created 33f8be41-7cdd-4956-b317-cf70813a2b7f
# Capture item IDs
$ echo ITEM_PROJECT=5e00f545-c50b-426c-aeab-d4cc15a8a1d0
ITEM_PROJECT=5e00f545-c50b-426c-aeab-d4cc15a8a1d0
$ echo ITEM_BILL=33f8be41-7cdd-4956-b317-cf70813a2b7f
ITEM_BILL=33f8be41-7cdd-4956-b317-cf70813a2b7f
# Manual assign under Work/Priority: High then Medium (Medium should replace High)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-exclusive-fix-demo-1771269260.ag category assign 5e00f545-c50b-426c-aeab-d4cc15a8a1d0 High
assigned item 5e00f545-c50b-426c-aeab-d4cc15a8a1d0 to category High
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-exclusive-fix-demo-1771269260.ag category assign 5e00f545-c50b-426c-aeab-d4cc15a8a1d0 Medium
assigned item 5e00f545-c50b-426c-aeab-d4cc15a8a1d0 to category Medium
# Manual assign under Project X branch: Project X High then Project X Medium (Project X Medium should replace Project X High)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-exclusive-fix-demo-1771269260.ag category assign 5e00f545-c50b-426c-aeab-d4cc15a8a1d0 'Project X High'
assigned item 5e00f545-c50b-426c-aeab-d4cc15a8a1d0 to category Project X High
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-exclusive-fix-demo-1771269260.ag category assign 5e00f545-c50b-426c-aeab-d4cc15a8a1d0 'Project X Medium'
assigned item 5e00f545-c50b-426c-aeab-d4cc15a8a1d0 to category Project X Medium
# Assign second item to High to prove item-level independence
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-exclusive-fix-demo-1771269260.ag category assign 33f8be41-7cdd-4956-b317-cf70813a2b7f High
assigned item 33f8be41-7cdd-4956-b317-cf70813a2b7f to category High
# List all items to inspect final category sets
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-exclusive-fix-demo-1771269260.ag list --include-done
33f8be41-7cdd-4956-b317-cf70813a2b7f | open | - | Pay electricity bill
  categories: High, Priority, Work
5e00f545-c50b-426c-aeab-d4cc15a8a1d0 | open | - | Prepare launch plan for Project X
  categories: Medium, Priority, Project X, Project X Medium, Project X Priority, Work
# Create and inspect Work view
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-exclusive-fix-demo-1771269260.ag view create 'Work View' --include Work
created view Work View
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-exclusive-fix-demo-1771269260.ag view show 'Work View'
# Work View

## Unassigned
33f8be41-7cdd-4956-b317-cf70813a2b7f | open | - | Pay electricity bill
  categories: High, Priority, Work
5e00f545-c50b-426c-aeab-d4cc15a8a1d0 | open | - | Prepare launch plan for Project X
  categories: Medium, Priority, Project X, Project X Medium, Project X Priority, Work
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-exclusive-fix-demo-1771269260.ag list --view 'Work View' --include-done
# Work View

## Unassigned
33f8be41-7cdd-4956-b317-cf70813a2b7f | open | - | Pay electricity bill
  categories: High, Priority, Work
5e00f545-c50b-426c-aeab-d4cc15a8a1d0 | open | - | Prepare launch plan for Project X
  categories: Medium, Priority, Project X, Project X Medium, Project X Priority, Work
```
