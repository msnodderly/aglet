# Frabulator CLI Demo Output

```text
Using DB: /tmp/aglet-frabulator-demo-70612.ag
created 87c504fa-4e5a-415b-bd0e-960bb768f9b0
--- list after add ---
87c504fa-4e5a-415b-bd0e-960bb768f9b0 | open | - | Follow up with Sarah on integration
ITEM_ID=87c504fa-4e5a-415b-bd0e-960bb768f9b0
created category Work (processed_items=1, affected_items=0)
created category Project Y (processed_items=1, affected_items=0)
created category Frabulator (processed_items=1, affected_items=0)
created category Green field (processed_items=1, affected_items=0)
--- category tree ---
- Done [no-implicit-string]
- Entry [no-implicit-string]
- When [no-implicit-string]
- Work
  - Project Y
    - Frabulator
    - Green field
assigned Frabulator to 87c504fa-4e5a-415b-bd0e-960bb768f9b0
--- list after assignment ---
87c504fa-4e5a-415b-bd0e-960bb768f9b0 | open | - | Follow up with Sarah on integration
  categories: Frabulator, Project Y, Work
created view Project Y Board
--- view output ---
# Project Y Board

## Unassigned
87c504fa-4e5a-415b-bd0e-960bb768f9b0 | open | - | Follow up with Sarah on integration
  categories: Frabulator, Project Y, Work
```
