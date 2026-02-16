# Literate Demo: Global Priority Reuse Across Project Branches

This run demonstrates the intended model: categories are globally unique and reusable across all project branches.

```text
# Demo DB path
$ echo /tmp/aglet-global-priority-reuse-1771269915.ag
/tmp/aglet-global-priority-reuse-1771269915.ag
# Reset demo database
$ rm -f /tmp/aglet-global-priority-reuse-1771269915.ag
# Create project categories and one global Priority hierarchy
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-global-priority-reuse-1771269915.ag category create Work
created category Work (processed_items=0, affected_items=0)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-global-priority-reuse-1771269915.ag category create 'Project X' --parent Work
created category Project X (processed_items=0, affected_items=0)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-global-priority-reuse-1771269915.ag category create 'Project Y' --parent Work
created category Project Y (processed_items=0, affected_items=0)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-global-priority-reuse-1771269915.ag category create Priority --exclusive
created category Priority (processed_items=0, affected_items=0)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-global-priority-reuse-1771269915.ag category create High --parent Priority
created category High (processed_items=0, affected_items=0)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-global-priority-reuse-1771269915.ag category create Medium --parent Priority
created category Medium (processed_items=0, affected_items=0)
# Attempt to create duplicate Priority under Project X (should explain how to reuse existing category)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-global-priority-reuse-1771269915.ag category create Priority --parent 'Project X'
error: category "Priority" already exists (existing id: e34dac20-2310-4bb6-a9f5-32016fd91860). Category names are global across the database, so it cannot be created under parent "Project X". Use `agenda category assign <item-id> "Priority"` to assign items to the existing category.
# exit_code=1 (expected non-zero)
# Add two items from different project branches
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-global-priority-reuse-1771269915.ag add 'Project X: frabulator release checklist'
created 54d9e667-c2d8-481d-8bbe-946a47e59a1f
new_assignments=1
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-global-priority-reuse-1771269915.ag add 'Project Y: integration readiness review'
created a5b849a5-8583-4c32-bd22-d6af9bdab964
new_assignments=1
# Resolved item IDs
$ echo ITEM_X=54d9e667-c2d8-481d-8bbe-946a47e59a1f
ITEM_X=54d9e667-c2d8-481d-8bbe-946a47e59a1f
$ echo ITEM_Y=a5b849a5-8583-4c32-bd22-d6af9bdab964
ITEM_Y=a5b849a5-8583-4c32-bd22-d6af9bdab964
# Assign each item to its project branch and to shared High priority
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-global-priority-reuse-1771269915.ag category assign 54d9e667-c2d8-481d-8bbe-946a47e59a1f 'Project X'
assigned item 54d9e667-c2d8-481d-8bbe-946a47e59a1f to category Project X
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-global-priority-reuse-1771269915.ag category assign a5b849a5-8583-4c32-bd22-d6af9bdab964 'Project Y'
assigned item a5b849a5-8583-4c32-bd22-d6af9bdab964 to category Project Y
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-global-priority-reuse-1771269915.ag category assign 54d9e667-c2d8-481d-8bbe-946a47e59a1f High
assigned item 54d9e667-c2d8-481d-8bbe-946a47e59a1f to category High
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-global-priority-reuse-1771269915.ag category assign a5b849a5-8583-4c32-bd22-d6af9bdab964 High
assigned item a5b849a5-8583-4c32-bd22-d6af9bdab964 to category High
# Exercise exclusivity on ITEM_X: High -> Medium -> High
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-global-priority-reuse-1771269915.ag category assign 54d9e667-c2d8-481d-8bbe-946a47e59a1f Medium
assigned item 54d9e667-c2d8-481d-8bbe-946a47e59a1f to category Medium
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-global-priority-reuse-1771269915.ag category assign 54d9e667-c2d8-481d-8bbe-946a47e59a1f High
assigned item 54d9e667-c2d8-481d-8bbe-946a47e59a1f to category High
# Inspect all items
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-global-priority-reuse-1771269915.ag list --include-done
a5b849a5-8583-4c32-bd22-d6af9bdab964 | open | - | Project Y: integration readiness review
  categories: High, Priority, Project Y, Work
54d9e667-c2d8-481d-8bbe-946a47e59a1f | open | - | Project X: frabulator release checklist
  categories: High, Priority, Project X, Work
# Inspect global High category view
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-global-priority-reuse-1771269915.ag list --category High --include-done
a5b849a5-8583-4c32-bd22-d6af9bdab964 | open | - | Project Y: integration readiness review
  categories: High, Priority, Project Y, Work
54d9e667-c2d8-481d-8bbe-946a47e59a1f | open | - | Project X: frabulator release checklist
  categories: High, Priority, Project X, Work
```
