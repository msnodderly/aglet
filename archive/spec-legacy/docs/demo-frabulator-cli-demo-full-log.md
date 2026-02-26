# Frabulator CLI Full Output Log

```text
# Working directory
$ cd /Users/mds/src/aglet-slc

# Verify branch
$ git rev-parse --abbrev-ref HEAD
codex/slc-v1

# Reset demo database
$ rm -f /tmp/aglet-frabulator-demo-full-72007.ag

# Add a demo item
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-frabulator-demo-full-72007.ag add "Follow up with Sarah on integration"
created 9ada2600-c712-4e8f-9c8e-81d1151fe816

# List items and capture item id
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-frabulator-demo-full-72007.ag list
9ada2600-c712-4e8f-9c8e-81d1151fe816 | open | - | Follow up with Sarah on integration
$ ITEM_ID=$(cargo run -q -p agenda-cli -- --db /tmp/aglet-frabulator-demo-full-72007.ag list | head -n1 | cut -d' ' -f1)
ITEM_ID=9ada2600-c712-4e8f-9c8e-81d1151fe816

# Create nested categories
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-frabulator-demo-full-72007.ag category create Work
created category Work (processed_items=1, affected_items=0)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-frabulator-demo-full-72007.ag category create "Project Y" --parent Work
created category Project Y (processed_items=1, affected_items=0)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-frabulator-demo-full-72007.ag category create Frabulator --parent "Project Y"
created category Frabulator (processed_items=1, affected_items=0)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-frabulator-demo-full-72007.ag category create "Green field" --parent "Project Y"
created category Green field (processed_items=1, affected_items=0)

# Show category tree
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-frabulator-demo-full-72007.ag category list
- Done [no-implicit-string]
- Entry [no-implicit-string]
- When [no-implicit-string]
- Work
  - Project Y
    - Frabulator
    - Green field

# Assign Frabulator to the item
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-frabulator-demo-full-72007.ag category assign 9ada2600-c712-4e8f-9c8e-81d1151fe816 Frabulator
assigned Frabulator to 9ada2600-c712-4e8f-9c8e-81d1151fe816

# Verify subsumption: Frabulator + Project Y + Work
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-frabulator-demo-full-72007.ag list --include-done
9ada2600-c712-4e8f-9c8e-81d1151fe816 | open | - | Follow up with Sarah on integration
  categories: Frabulator, Project Y, Work

# Create and query a Project Y view
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-frabulator-demo-full-72007.ag view create "Project Y Board" --include "Project Y"
created view Project Y Board
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-frabulator-demo-full-72007.ag list --view "Project Y Board"
# Project Y Board

## Unassigned
9ada2600-c712-4e8f-9c8e-81d1151fe816 | open | - | Follow up with Sarah on integration
  categories: Frabulator, Project Y, Work
```
