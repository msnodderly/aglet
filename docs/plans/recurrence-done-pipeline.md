---
title: Recurrence And Post-Done Pipeline
status: shipped
created: 2026-03-28
shipped: 2026-04-02
---

# Recurrence And Post-Done Pipeline Plan

Status: Proposed
Tracking issue: `95d70980-088d-4c5c-a09a-499a0d1be7aa`

## Summary

Implement recurrence as part of the item completion pipeline, not as a generic
category action. The core hook is `Workspace::mark_item_done(...)`: once an item is
completed, aglet should be able to generate the next instance and preserve
clear sequencing for future post-done automation.

## Goals

- Add recurrence fields to the item model and store.
- Generate the next recurrence instance from the done pipeline.
- Preserve completed-instance history (`done_date`, `Done` assignment) while
  creating a fresh open successor.
- Create an internal sequencing seam that can later support an
  `Entry When Done`-style trigger without forcing that UX now.

## Non-Goals

- Full natural-language recurrence parsing in the first implementation slice.
- Calendar/datebook UI.
- Generic date-setting or delete/export category actions.

## Proposed Model Additions

Add item fields along the lines of:

- `recurrence_rule: Option<RecurrenceRule>`
- `recurrence_series_id: Option<Uuid>`
- `recurrence_parent_item_id: Option<Uuid>`

The first shippable `RecurrenceRule` can stay intentionally small:

- daily
- weekly
- monthly
- yearly
- interval
- anchor datetime

## Completion Pipeline Design

Current done flow:

1. validate actionable state
2. set `is_done = true`
3. stamp `done_date`
4. assign reserved `Done`
5. clear workflow claim assignment
6. reprocess

Proposed flow:

1. perform current done mutation
2. snapshot recurrence metadata from the completed item
3. if recurrence exists, compute the next occurrence
4. create the successor item
5. copy stable fields/categorizations according to recurrence policy
6. keep completion cleanup and successor creation in one transaction boundary
7. reprocess both completed and successor items as needed

## Category/Assignment Policy

Completed instance keeps:

- original text/note
- `Done` assignment
- `done_date`
- series metadata

Successor instance gets:

- same text/note by default
- next `when_date`
- same recurrence rule and series id
- no `Done`
- no claim category

Open question for implementation:

- whether all non-done manual categories copy by default, or whether we need an
  explicit carry-forward policy

Recommendation: start with copying manual and sticky non-done category
assignments, then trim if user workflows prove too noisy.

## Internal Sequencing Seam

Do not expose `Entry When Done` yet, but structure the code so the done path can
eventually support ordered post-completion hooks:

- completion cleanup
- recurrence successor generation
- future post-done triggers/actions

This can be an internal `DoneProcessingPlan`/`DoneProcessingResult` abstraction
rather than a new user-facing category in the first slice.

## Test Matrix

- marking a recurring item done generates exactly one successor.
- successor gets the next expected `when_date`.
- completed instance retains `Done` and `done_date`.
- successor is open and unclaimed.
- repeated completion creates a consistent series chain.
- transaction failure does not leave partial successor creation behind.
- non-recurring done path remains unchanged.

## Risks

- The hardest part is defining carry-forward assignment policy clearly.
- Recurrence arithmetic must use `jiff` calendar-aware spans and must be
  covered with month-end edge cases.
- Done-pipeline side effects can become tangled if recurrence and future
  post-done hooks are mixed too early.

## Sequencing Recommendation

Ship this in two slices:

1. recurrence model + next-instance generation in `mark_item_done(...)`
2. internal post-done sequencing abstraction for later `Entry When Done` work
