---
title: Category Actions Authoring
status: shipped
created: 2026-03-28
shipped: 2026-04-02
---

# Category Actions Authoring Plan

Status: Proposed
Tracking issue: `befa2fdb-3874-4cff-a9ff-05ac422fbb8a`

## Summary

Expose the existing category action system in shipped UX without changing the
core rule semantics. Aglet already supports `Action::Assign` and
`Action::Remove` in the model, store, workspace layer, and fixed-point engine; the
missing work is authoring, editing, and visibility in CLI/TUI.

This should be treated as a phase-1, category-targeted action editor. The UI
surface should center on "action kind + payload" even though the only shipped
payload type in this phase is "category targets".

## Goals

- Add first-class CLI commands to create, edit, and remove category actions.
- Add first-class TUI visibility and editing for category actions in Category
  Manager.
- Centralize action labeling/summary code so future action kinds can plug into
  one surface instead of duplicating `Assign`/`Remove` checks in multiple UI
  paths.
- Keep action semantics unchanged:
  - actions are source-centric
  - they fire when the owning category is assigned
  - action-produced assignments remain sticky

## Non-Goals

- Add new action kinds such as date-setting, mark-done, delete, or export.
- Change action firing order or fixed-point behavior.
- Add per-category execution timing controls.
- Design the final editor UX for non-category-targeted action payloads. This
  phase should leave an obvious extension point for those later kinds.

## Current Reality

- Core model already has `Category.actions: Vec<Action>`.
- Supported action kinds are `Assign { targets }` and `Remove { targets }`.
- Engine behavior already persists action output as sticky `AssignmentSource::Action`.
- Adding/editing an action reprocesses items for category changes, but it does
  **not** retroactively fire that action for items already assigned to the
  owning category. Actions still fire on assignment events.
- CLI can display actions in `category show`, but cannot create or remove them.
- TUI profile-condition editing exists, but action editing does not.
- The first shipped editor can safely assume a category-target payload, but its
  surrounding navigation and summaries should be action-kind-oriented.

## Proposed UX

### CLI

Add category subcommands:

- `category add-action <name> --assign <cat>...`
- `category add-action <name> --remove <cat>...`
- `category remove-action <name> <index>`

Rules:

- exactly one action kind per invocation
- at least one target required
- 1-based indexing, matching `category show`
- reject self-targeting when it would create trivial loops

### TUI

Mirror the existing condition-editor pattern:

- add an `Actions (N)` row in Category Manager details
- `Enter` opens an action list view
- `a` adds a new action
- `Enter` edits the selected action
- `x` deletes the selected action

Editor model:

- level 1: action list
- level 2: action kind entry point
- level 3: payload editor for the selected kind

Phase 1 payload editor:

- `Assign` / `Remove` both use a category target picker built on the same
  category selection affordances as profile conditions

## Implementation Plan

1. Extend CLI command surface and help text.
2. Centralize action kind/summary formatting so CLI and TUI use the same naming
   and future kinds only add one new branch.
3. Add shared workspace/category mutation helpers for action list updates so CLI
   and TUI do not hand-roll the same validation logic.
4. Add TUI action-edit state alongside existing condition-edit state.
5. Surface action counts and summaries in Category Manager details.
6. Add regression tests for CLI, TUI state transitions, and action cascades.

## Validation Rules

- Disallow empty target sets.
- Resolve target categories by exact category identity, not alias text.
- Preserve existing action order in the stored vector.
- Keep reserved-category protections unchanged.

## Test Matrix

- CLI add assign-action stores the action and preserves event-driven semantics
  for already-assigned items.
- CLI add remove-action stores the action and preserves event-driven semantics
  for already-assigned items.
- CLI remove-action deletes the intended action by displayed index.
- TUI action list opens, edits, deletes, and survives refresh/reload.
- Action-created assignments remain after the triggering live condition
  auto-breaks.
- `Action::Remove` still defers removal until cascade completion.

## Risks

- The TUI action editor can easily duplicate condition-editor logic. Prefer a
  shared picker model where possible.
- If we overfit the authoring flow to category-target actions, later `SetWhen`
  or `MarkDone` work will require redoing the overlay instead of extending it.
- Action loops are already bounded by the engine pass cap, but better
  validation and user-facing error messages will make failures easier to debug.

## Exit Criteria

- Users can author `Assign` and `Remove` actions from both CLI and TUI.
- Category Manager clearly shows when a category has actions attached.
- Existing engine tests still pass unchanged aside from UX-surface additions.
