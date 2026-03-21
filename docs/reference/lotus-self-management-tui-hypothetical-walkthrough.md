# Hypothetical TUI Walkthrough: `Routine Tasks -> Routine`

Date: 2026-03-21
Source scenario: Lotus "Task List" assignment-condition exercise (topic 5, page 5.9) from `~/src/lotus_agenda_applied_self_management.pdf`

## Goal

Replicate the first currently-missing Lotus scenario in the TUI:

- database contains `Task Type`
- `Task Type` has child `Routine`
- database also contains category `Routine Tasks`
- user configures `Routine` so that any item assigned to `Routine Tasks` is also assigned to `Routine`

This is a hypothetical walkthrough. It describes the TUI flow Aglet should have, not the one it ships today.

## Desired workflow shape

The workflow should stay inside the existing `Category Manager` because that is already the place where users think about:

- category structure
- auto-match behavior
- workflow roles
- category metadata

The missing concept is a first-class `Conditions` editor for the selected category.

## Walkthrough

### 1. Open the tasks database and enter Category Manager

From the normal tasks view:

```text
Ad-hoc Tasks

Planning              this week
Meeting with Sam      tomorrow
Renew gym membership  next month

Footer: n:new  e:edit  a:assign  c:categories  v:views  p:preview
```

Press `c`.

Expected result:

```text
Category Manager

Tree                               Details
Task Type                          Category: Task Type
  Planning                         [ ] Exclusive
  Meeting                          [x] Auto-match
> Routine                          [x] Actionable
Routine Tasks
Prty

Footer: S:save  n:new  r:rename  x:delete  Tab:pane  /:filter  w:workflow  Esc:close
```

### 2. Select `Routine`

Use `j` / `k` in the tree until `Routine` is selected.

Expected details pane:

```text
Category: Routine

[ ] Exclusive
[ ] Auto-match
[x] Actionable

Conditions
(none)

Actions
(none)

Note
-
```

This is the key missing state today: the details pane needs a visible `Conditions` region, not just boolean flags.

### 3. Enter condition-edit mode

Press `Enter` on the `Conditions` row or press a dedicated key such as `C`.

Expected popup or inline editor:

```text
Edit Conditions for: Routine

Conditions
> (none)

n:new  e:edit  d:delete  p:preview matches  Esc:back
```

### 4. Add the profile condition

Press `n`.

Expected condition composer:

```text
New Condition

Type: Profile

Match when item is:
> assigned to [Routine Tasks           ]
  and assigned to [                    ]
  not assigned to [                    ]

Preview: 0 matching items

Enter: pick category  S:save  Esc:cancel
```

Type or pick `Routine Tasks`, then press `S`.

Expected return state:

```text
Edit Conditions for: Routine

Conditions
> 1. Assigned to: Routine Tasks

n:new  e:edit  d:delete  p:preview matches  Esc:back
```

### 5. Preview matches before saving category changes

Press `p`.

Expected preview panel:

```text
Condition Preview: Routine

Would assign Routine to 3 items:

> Pay rent
  Review recurring subscriptions
  Submit weekly status

Current categories shown for selected item:
Routine Tasks, When

Enter: inspect item  S:accept  Esc:back
```

This preview step matters because assignment conditions are retroactive and can affect many existing items.

### 6. Save and re-run affected assignments

Press `S`.

Expected status:

```text
saved category Routine
processed_items=42 affected_items=3
```

This save should:

- persist the profile condition on `Routine`
- run reevaluation for affected items
- apply `Routine` where the condition matches
- preserve provenance so the user can inspect why the assignment happened

### 7. Verify from the item view

Press `Esc` to leave Category Manager and return to the task list.

Select an item such as `Pay rent`, then press `p` for preview/info.

Expected info pane:

```text
Pay rent

Assignments
- Routine Tasks | Manual | manual:tui.assign
- Routine       | AutoClassified | cond:Routine.profile
- Task Type     | Subsumption | subsumption:Task Type
```

This is the payoff of the Lotus scenario: the user assigns one category and sees the category program add the second one.

## Why this should be the first TUI condition workflow

This scenario is the smallest non-trivial proof that the TUI supports category logic, not just category storage.

It exercises:

- condition authoring
- category picking
- preview before save
- reevaluation
- provenance visibility

It is also easier to validate than more complex datebook or multi-condition scenarios.

## Minimum implementation contract

To support this walkthrough, the TUI needs:

- a `Conditions` section in Category Manager details
- a condition list for the selected category
- an add/edit/delete flow for profile conditions
- category-picker inputs for `and` / `or` / `not` terms
- preview-before-save
- save feedback with `processed_items` and `affected_items`
- post-save provenance visible in the normal item preview/info pane

## Out of scope for this walkthrough

This walkthrough does not require:

- datebook view authoring
- validation conditions
- AI/LLM classification
- action-rule authoring

It is the narrowest TUI slice that would make the first Lotus assignment-condition exercise real.
