# Literate CLI Demo: Categories, Priority, Implicit/Manual Assignment

This demo exercises intended SLC workflows and calls out what is implemented vs missing.

## Demo goals

We want to validate all of the following in one narrative run:

1. Create a category hierarchy under `Work` and `Personal`.
2. Add a `Priority` hierarchy with `High`, `Medium`, `Low`, and set parent exclusivity.
3. Verify implicit assignment on item creation when categories already exist.
4. Verify manual assignment to a category even when text does not match.
5. Verify whether manual assignment requires category pre-existence.
6. Check whether exclusivity is enforced in practice through current CLI/UI paths.

## Setup

We used this database for the run:

```text
/tmp/aglet-literate-demo-1771267636.ag
```

## 1) Create hierarchy (Work/Personal + Priority)

We define nested categories first, before adding items.

```text
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-literate-demo-1771267636.ag category create Work
created category Work (processed_items=0, affected_items=0)

$ cargo run -q -p agenda-cli -- --db /tmp/aglet-literate-demo-1771267636.ag category create Personal
created category Personal (processed_items=0, affected_items=0)

$ cargo run -q -p agenda-cli -- --db /tmp/aglet-literate-demo-1771267636.ag category create Sarah
created category Sarah (processed_items=0, affected_items=0)

$ cargo run -q -p agenda-cli -- --db /tmp/aglet-literate-demo-1771267636.ag category create "Project Y" --parent Work
created category Project Y (processed_items=0, affected_items=0)

$ cargo run -q -p agenda-cli -- --db /tmp/aglet-literate-demo-1771267636.ag category create Frabulator --parent "Project Y"
created category Frabulator (processed_items=0, affected_items=0)

$ cargo run -q -p agenda-cli -- --db /tmp/aglet-literate-demo-1771267636.ag category create "Green field" --parent "Project Y"
created category Green field (processed_items=0, affected_items=0)

$ cargo run -q -p agenda-cli -- --db /tmp/aglet-literate-demo-1771267636.ag category create Groceries --parent Personal
created category Groceries (processed_items=0, affected_items=0)

$ cargo run -q -p agenda-cli -- --db /tmp/aglet-literate-demo-1771267636.ag category create Priority --exclusive
created category Priority (processed_items=0, affected_items=0)

$ cargo run -q -p agenda-cli -- --db /tmp/aglet-literate-demo-1771267636.ag category create High --parent Priority
created category High (processed_items=0, affected_items=0)

$ cargo run -q -p agenda-cli -- --db /tmp/aglet-literate-demo-1771267636.ag category create Medium --parent Priority
created category Medium (processed_items=0, affected_items=0)

$ cargo run -q -p agenda-cli -- --db /tmp/aglet-literate-demo-1771267636.ag category create Low --parent Priority
created category Low (processed_items=0, affected_items=0)

$ cargo run -q -p agenda-cli -- --db /tmp/aglet-literate-demo-1771267636.ag category list
- Done [no-implicit-string]
- Entry [no-implicit-string]
- Personal
  - Groceries
- Priority [exclusive]
  - High
  - Medium
  - Low
- Sarah
- When [no-implicit-string]
- Work
  - Project Y
    - Frabulator
    - Green field
```

## 2) Add items after categories exist (implicit assignment check)

Now we capture items in natural language and observe auto-assignment.

```text
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-literate-demo-1771267636.ag add "Follow up with Sarah on Frabulator integration next Friday at 3pm"
created 83fecb1e-cbfd-45c9-8701-6005e953460e
new_assignments=2

$ cargo run -q -p agenda-cli -- --db /tmp/aglet-literate-demo-1771267636.ag add "Buy groceries tomorrow at 6pm"
created 0e283d5e-dd07-4e4d-85cf-a7ea425d1bb6
new_assignments=1

$ cargo run -q -p agenda-cli -- --db /tmp/aglet-literate-demo-1771267636.ag add "Draft architecture review agenda"
created 277bd9f7-15da-4c48-9c74-6af4af4a46e4

$ cargo run -q -p agenda-cli -- --db /tmp/aglet-literate-demo-1771267636.ag list --include-done
277bd9f7-15da-4c48-9c74-6af4af4a46e4 | open | - | Draft architecture review agenda
0e283d5e-dd07-4e4d-85cf-a7ea425d1bb6 | open | 2026-02-17 18:00:00 | Buy groceries tomorrow at 6pm
  categories: Groceries, Personal, When
83fecb1e-cbfd-45c9-8701-6005e953460e | open | 2026-02-20 15:00:00 | Follow up with Sarah on Frabulator integration next Friday at 3pm
  categories: Frabulator, Project Y, Sarah, When, Work
```

### Observation

- Implicit assignment works for existing categories (`Sarah`, `Frabulator`, `Groceries`).
- Subsumption works for these implicit matches (`Frabulator -> Project Y -> Work`, `Groceries -> Personal`).

## 3) Manual assignment where text does not match

The text `Draft architecture review agenda` does not mention `Personal`, but manual assignment should still work.

```text
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-literate-demo-1771267636.ag category assign 277bd9f7-15da-4c48-9c74-6af4af4a46e4 Personal
assigned item 277bd9f7-15da-4c48-9c74-6af4af4a46e4 to category Personal
```

### Observation

Manual assignment to non-matching category is implemented and works.

## 4) Does manual assignment auto-create categories?

No. Category must already exist.

```text
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-literate-demo-1771267636.ag category assign 277bd9f7-15da-4c48-9c74-6af4af4a46e4 Nonexistent
error: category not found: Nonexistent
```

### Observation

Manual assign currently requires a pre-existing category.

## 5) Priority exclusivity test (High/Medium/Low)

Expected with exclusive parent `Priority`:
- assigning `Medium` after `High` should remove `High`.

Observed in current build:

```text
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-literate-demo-1771267636.ag category assign 83fecb1e-cbfd-45c9-8701-6005e953460e High
assigned item 83fecb1e-cbfd-45c9-8701-6005e953460e to category High

$ cargo run -q -p agenda-cli -- --db /tmp/aglet-literate-demo-1771267636.ag category assign 83fecb1e-cbfd-45c9-8701-6005e953460e Medium
assigned item 83fecb1e-cbfd-45c9-8701-6005e953460e to category Medium

$ cargo run -q -p agenda-cli -- --db /tmp/aglet-literate-demo-1771267636.ag list --include-done
83fecb1e-cbfd-45c9-8701-6005e953460e | open | 2026-02-20 15:00:00 | Follow up with Sarah on Frabulator integration next Friday at 3pm
  categories: Frabulator, High, Medium, Priority, Project Y, Sarah, When, Work
```

### Observation

- **Exclusivity is not being enforced for this manual assignment path right now.**
- Item ended up with both `High` and `Medium`.

## 6) View contents and discoverability

We can inspect view contents directly with `view show`.

```text
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-literate-demo-1771267636.ag view create "Work View" --include Work
created view Work View

$ cargo run -q -p agenda-cli -- --db /tmp/aglet-literate-demo-1771267636.ag view create "Priority View" --include Priority
created view Priority View

$ cargo run -q -p agenda-cli -- --db /tmp/aglet-literate-demo-1771267636.ag view show "Work View"
# Work View

## Unassigned
83fecb1e-cbfd-45c9-8701-6005e953460e | open | 2026-02-20 15:00:00 | Follow up with Sarah on Frabulator integration next Friday at 3pm
  categories: Frabulator, High, Medium, Priority, Project Y, Sarah, When, Work

$ cargo run -q -p agenda-cli -- --db /tmp/aglet-literate-demo-1771267636.ag view show "Priority View"
# Priority View

## Unassigned
83fecb1e-cbfd-45c9-8701-6005e953460e | open | 2026-02-20 15:00:00 | Follow up with Sarah on Frabulator integration next Friday at 3pm
  categories: Frabulator, High, Medium, Priority, Project Y, Sarah, When, Work
```

## Implementation status summary

## Underlying domain (`agenda-core`)

Implemented:
- Category hierarchy and parent/child model.
- `is_exclusive` field on categories.
- Implicit assignment + subsumption for existing categories.
- Manual assignment API.

Observed gap:
- Manual assignment path does not currently enforce exclusive sibling cleanup in this scenario (`High` + `Medium` both persisted).

## CLI (`agenda-cli`)

Implemented:
- Category create/delete/assign.
- View create/list/show/delete.
- Clear error when assigning to missing category.

Observed gap:
- No command-level warning when exclusivity invariant is violated downstream.

## TUI (`agenda-tui`)

Implemented:
- Daily workflow interactions (add/move/remove/done/delete/filter/view switching/inspect).

Missing for this specific demo workflow:
- No built-in category manager/editor in TUI for creating/editing hierarchy or toggling exclusivity.
- No direct arbitrary category assignment UI equivalent to `category assign`.
- Therefore, exclusivity testing from TUI is only indirect today (through preconfigured structures), not first-class.

---

This demo confirms key SLC workflows and highlights one important correctness gap: exclusive category enforcement for manual assign path.
