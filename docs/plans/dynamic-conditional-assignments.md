---
title: Dynamic Conditional Assignments Follow-Up
status: shipped
created: 2026-03-20
shipped: 2026-03-29
---

# Dynamic Conditional Assignments Follow-Up

## Summary

Create a follow-up implementation branch from PR #100 (`profile-conditions`) and add Lotus-style auto-breaking only for destination-centric condition-derived assignments. Manual assignments, accepted suggestions, and action-produced assignments stay sticky.

This pass also makes non-sticky subsumption assignments live so ancestors derived only from supporting descendants disappear when support disappears, while manually assigned ancestors remain.

## Semantics

- Condition-derived assignments become live and auto-breaking:
  - implicit string matches
  - profile conditions
- Keep these sticky:
  - `Manual`
  - `SuggestionAccepted`
  - `Action`
- Keep accepted suggestion behavior explicit:
  - once accepted, the assignment remains even if the original condition later stops matching
- Keep action behavior one-shot:
  - if a live condition assigns category A and A fires an action assigning B, B remains after A later auto-breaks
- Treat non-sticky `Subsumption` as live derived state:
  - ancestors should exist only while supported by current direct assignments
  - manually assigned ancestors remain even if structural support disappears

## Engine / Model Behavior

- Reuse existing `Assignment.sticky`; do not add a new schema field or enum variant.
- Write condition-derived assignments with `sticky = false`.
- Write subsumption assignments with `sticky = false`.
- Refactor item reprocessing so it reconciles live derived assignments instead of only adding:
  - retain all sticky assignments and any non-sticky non-derived assignments
  - rebuild live derived assignments in memory
  - remove stale non-sticky derived assignments that no longer match
  - add newly matched live assignments
  - diff the final desired assignment set back to SQLite
- Automatic removal is limited to non-sticky engine-derived assignments only.
- Preserve manual and accepted assignments even if they happen to match the same category as a dynamic rule.
- Keep compatibility narrow:
  - existing persisted sticky auto-derived assignments are not retroactively converted in this pass
  - only newly created live derived assignments use `sticky = false`

## Agenda / Preview Integration

- Update agenda helper paths so structural subsumption writes use `sticky = false`.
- Keep acceptance paths using `SuggestionAccepted` with `sticky = true`.
- Ensure preview and reprocess paths share the same reconciliation logic so previews match actual saves.

## Test Matrix

- Implicit string auto-break:
  - item matches `Phone Calls`
  - edit text so the match disappears
  - `Phone Calls` assignment is removed on reprocess
- Profile auto-break:
  - `Urgent + Project Alpha -> Escalated`
  - remove one prerequisite
  - `Escalated` is removed on reprocess
- Accepted suggestion persists:
  - accept a suggestion into a category
  - later stop matching
  - assignment remains
- Action persistence:
  - live condition assigns `Escalated`
  - `Escalated` action assigns `Notify`
  - later `Escalated` auto-breaks
  - `Notify` remains
- Subsumption reconciliation:
  - ancestor assigned only through descendants disappears when the supporting descendant disappears
  - manually assigned ancestor remains
- Exclusivity:
  - live derived assignment into an exclusive family still removes conflicting sibling on assignment
  - later loss of the live assignment removes only that live assignment, not unrelated sticky assignments
- Preview parity:
  - previewed results match actual save/reprocess results

## Rollout Note

This is a product behavior change, not just an engine refactor. Items can now silently lose non-sticky derived assignments when their triggering conditions stop matching. That is intentional for live condition-derived state, but sticky/manual/accepted/action assignments remain stable.
