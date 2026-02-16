# Complete CLI End-to-End Demo Log

```text
# Working directory
$ cd /Users/mds/src/aglet-slc

# Verify branch
$ git rev-parse --abbrev-ref HEAD
codex/slc-v1

# Reset demo database
$ rm -f /tmp/aglet-complete-demo-74143.ag

# Add several real-world items
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-complete-demo-74143.ag add "Follow up with Sarah on Frabulator integration next Friday at 3pm"
created f85583ec-2567-4cf3-ae15-96c2f9765037
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-complete-demo-74143.ag add "Buy groceries (milk, eggs, bread) tomorrow at 6pm"
created c9234bfb-4633-4816-978a-6b9e39ea2faa
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-complete-demo-74143.ag add "Review Project Y architecture doc Monday"
created 49f1b389-9d5e-4501-9cce-e06c51b3b763
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-complete-demo-74143.ag add "Call dentist next Tuesday at 9am"
created 686d909d-59ef-4938-87a1-99167ed6ff7e
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-complete-demo-74143.ag add "Green field prototype brainstorming"
created 82484bc2-8a85-4cf6-8a1d-49f01af4bddf
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-complete-demo-74143.ag add "Book flights for personal trip in March"
created 4565ecce-a317-454a-932a-f2047e971723

# List items after initial capture (quick-add workflow)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-complete-demo-74143.ag list --include-done
4565ecce-a317-454a-932a-f2047e971723 | open | - | Book flights for personal trip in March
82484bc2-8a85-4cf6-8a1d-49f01af4bddf | open | - | Green field prototype brainstorming
686d909d-59ef-4938-87a1-99167ed6ff7e | open | 2026-02-17 09:00:00 | Call dentist next Tuesday at 9am
  categories: When
49f1b389-9d5e-4501-9cce-e06c51b3b763 | open | - | Review Project Y architecture doc Monday
c9234bfb-4633-4816-978a-6b9e39ea2faa | open | 2026-02-17 18:00:00 | Buy groceries (milk, eggs, bread) tomorrow at 6pm
  categories: When
f85583ec-2567-4cf3-ae15-96c2f9765037 | open | 2026-02-20 15:00:00 | Follow up with Sarah on Frabulator integration next Friday at 3pm
  categories: When

# Capture key item IDs for later operations
$ SARAH_ID=$(cargo run -q -p agenda-cli -- --db /tmp/aglet-complete-demo-74143.ag list --include-done | rg 'Follow up with Sarah' | head -n1 | cut -d' ' -f1)
SARAH_ID=f85583ec-2567-4cf3-ae15-96c2f9765037
$ ARCH_ID=$(cargo run -q -p agenda-cli -- --db /tmp/aglet-complete-demo-74143.ag list --include-done | rg 'Review Project Y architecture' | head -n1 | cut -d' ' -f1)
ARCH_ID=49f1b389-9d5e-4501-9cce-e06c51b3b763

# Create category hierarchy (including nested Work > Project Y > Frabulator/Green field)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-complete-demo-74143.ag category create Work
created category Work (processed_items=6, affected_items=0)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-complete-demo-74143.ag category create Personal
created category Personal (processed_items=6, affected_items=1)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-complete-demo-74143.ag category create Sarah
created category Sarah (processed_items=6, affected_items=1)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-complete-demo-74143.ag category create "Project Y" --parent Work
created category Project Y (processed_items=6, affected_items=1)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-complete-demo-74143.ag category create Frabulator --parent "Project Y"
created category Frabulator (processed_items=6, affected_items=1)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-complete-demo-74143.ag category create "Green field" --parent "Project Y"
created category Green field (processed_items=6, affected_items=1)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-complete-demo-74143.ag category create Groceries --parent Personal
created category Groceries (processed_items=6, affected_items=1)

# Show the resulting category tree
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-complete-demo-74143.ag category list
- Done [no-implicit-string]
- Entry [no-implicit-string]
- Personal
  - Groceries
- Sarah
- When [no-implicit-string]
- Work
  - Project Y
    - Frabulator
    - Green field

# Explicitly assign Frabulator to Sarah item (manual assign workflow)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-complete-demo-74143.ag category assign f85583ec-2567-4cf3-ae15-96c2f9765037 Frabulator
assigned Frabulator to f85583ec-2567-4cf3-ae15-96c2f9765037

# Confirm multifiling + subsumption from nested category assignment
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-complete-demo-74143.ag list --include-done
4565ecce-a317-454a-932a-f2047e971723 | open | - | Book flights for personal trip in March
  categories: Personal
82484bc2-8a85-4cf6-8a1d-49f01af4bddf | open | - | Green field prototype brainstorming
  categories: Green field, Project Y, Work
686d909d-59ef-4938-87a1-99167ed6ff7e | open | 2026-02-17 09:00:00 | Call dentist next Tuesday at 9am
  categories: When
49f1b389-9d5e-4501-9cce-e06c51b3b763 | open | - | Review Project Y architecture doc Monday
  categories: Project Y, Work
c9234bfb-4633-4816-978a-6b9e39ea2faa | open | 2026-02-17 18:00:00 | Buy groceries (milk, eggs, bread) tomorrow at 6pm
  categories: Groceries, Personal, When
f85583ec-2567-4cf3-ae15-96c2f9765037 | open | 2026-02-20 15:00:00 | Follow up with Sarah on Frabulator integration next Friday at 3pm
  categories: Frabulator, Project Y, Sarah, When, Work

# Create views to demonstrate cross-view visibility
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-complete-demo-74143.ag view create "Work View" --include Work
created view Work View
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-complete-demo-74143.ag view create "Sarah View" --include Sarah
created view Sarah View
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-complete-demo-74143.ag view create "Personal View" --include Personal
created view Personal View

# Show Sarah item appears in both Sarah View and Work View
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-complete-demo-74143.ag list --view "Sarah View"
# Sarah View

## Unassigned
f85583ec-2567-4cf3-ae15-96c2f9765037 | open | 2026-02-20 15:00:00 | Follow up with Sarah on Frabulator integration next Friday at 3pm
  categories: Frabulator, Project Y, Sarah, When, Work
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-complete-demo-74143.ag list --view "Work View"
# Work View

## Unassigned
82484bc2-8a85-4cf6-8a1d-49f01af4bddf | open | - | Green field prototype brainstorming
  categories: Green field, Project Y, Work
49f1b389-9d5e-4501-9cce-e06c51b3b763 | open | - | Review Project Y architecture doc Monday
  categories: Project Y, Work
f85583ec-2567-4cf3-ae15-96c2f9765037 | open | 2026-02-20 15:00:00 | Follow up with Sarah on Frabulator integration next Friday at 3pm
  categories: Frabulator, Project Y, Sarah, When, Work

# Exercise search workflow
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-complete-demo-74143.ag search prototype
82484bc2-8a85-4cf6-8a1d-49f01af4bddf | open | - | Green field prototype brainstorming
  categories: Green field, Project Y, Work

# Mark one item done (Done semantic path)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-complete-demo-74143.ag done 49f1b389-9d5e-4501-9cce-e06c51b3b763
marked done 49f1b389-9d5e-4501-9cce-e06c51b3b763
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-complete-demo-74143.ag list
4565ecce-a317-454a-932a-f2047e971723 | open | - | Book flights for personal trip in March
  categories: Personal
82484bc2-8a85-4cf6-8a1d-49f01af4bddf | open | - | Green field prototype brainstorming
  categories: Green field, Project Y, Work
686d909d-59ef-4938-87a1-99167ed6ff7e | open | 2026-02-17 09:00:00 | Call dentist next Tuesday at 9am
  categories: When
c9234bfb-4633-4816-978a-6b9e39ea2faa | open | 2026-02-17 18:00:00 | Buy groceries (milk, eggs, bread) tomorrow at 6pm
  categories: Groceries, Personal, When
f85583ec-2567-4cf3-ae15-96c2f9765037 | open | 2026-02-20 15:00:00 | Follow up with Sarah on Frabulator integration next Friday at 3pm
  categories: Frabulator, Project Y, Sarah, When, Work
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-complete-demo-74143.ag list --include-done
4565ecce-a317-454a-932a-f2047e971723 | open | - | Book flights for personal trip in March
  categories: Personal
82484bc2-8a85-4cf6-8a1d-49f01af4bddf | open | - | Green field prototype brainstorming
  categories: Green field, Project Y, Work
686d909d-59ef-4938-87a1-99167ed6ff7e | open | 2026-02-17 09:00:00 | Call dentist next Tuesday at 9am
  categories: When
49f1b389-9d5e-4501-9cce-e06c51b3b763 | done | - | Review Project Y architecture doc Monday
  categories: Done, Project Y, Work
c9234bfb-4633-4816-978a-6b9e39ea2faa | open | 2026-02-17 18:00:00 | Buy groceries (milk, eggs, bread) tomorrow at 6pm
  categories: Groceries, Personal, When
f85583ec-2567-4cf3-ae15-96c2f9765037 | open | 2026-02-20 15:00:00 | Follow up with Sarah on Frabulator integration next Friday at 3pm
  categories: Frabulator, Project Y, Sarah, When, Work

# Delete the done item and inspect deletion log
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-complete-demo-74143.ag delete 49f1b389-9d5e-4501-9cce-e06c51b3b763
deleted 49f1b389-9d5e-4501-9cce-e06c51b3b763
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-complete-demo-74143.ag deleted
770d90fe-505c-4609-aeb4-c30d9b382f53 | item=49f1b389-9d5e-4501-9cce-e06c51b3b763 | deleted_at=2026-02-16T17:40:55.965729+00:00 | by=user:cli | Review Project Y architecture doc Monday
$ LOG_ID=$(cargo run -q -p agenda-cli -- --db /tmp/aglet-complete-demo-74143.ag deleted | head -n1 | cut -d' ' -f1)
LOG_ID=770d90fe-505c-4609-aeb4-c30d9b382f53

# Restore deleted item and verify recovery
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-complete-demo-74143.ag restore 770d90fe-505c-4609-aeb4-c30d9b382f53
restored item 49f1b389-9d5e-4501-9cce-e06c51b3b763
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-complete-demo-74143.ag list --include-done
49f1b389-9d5e-4501-9cce-e06c51b3b763 | done | - | Review Project Y architecture doc Monday
  categories: Done, Project Y, Work
4565ecce-a317-454a-932a-f2047e971723 | open | - | Book flights for personal trip in March
  categories: Personal
82484bc2-8a85-4cf6-8a1d-49f01af4bddf | open | - | Green field prototype brainstorming
  categories: Green field, Project Y, Work
686d909d-59ef-4938-87a1-99167ed6ff7e | open | 2026-02-17 09:00:00 | Call dentist next Tuesday at 9am
  categories: When
c9234bfb-4633-4816-978a-6b9e39ea2faa | open | 2026-02-17 18:00:00 | Buy groceries (milk, eggs, bread) tomorrow at 6pm
  categories: Groceries, Personal, When
f85583ec-2567-4cf3-ae15-96c2f9765037 | open | 2026-02-20 15:00:00 | Follow up with Sarah on Frabulator integration next Friday at 3pm
  categories: Frabulator, Project Y, Sarah, When, Work

# Optional: list views for navigation context
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-complete-demo-74143.ag view list
All Items (sections=0, include=0, exclude=0)
Personal View (sections=0, include=1, exclude=0)
Sarah View (sections=0, include=1, exclude=0)
Work View (sections=0, include=1, exclude=0)
```
