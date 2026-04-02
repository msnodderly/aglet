# Category Actions Authoring Plan

Status: Proposed
Tracking issue: `befa2fdb-3874-4cff-a9ff-05ac422fbb8a`

## Summary

Expose the existing category action system in shipped UX without changing the
core rule semantics. Aglet already supports `Action::Assign` and
`Action::Remove` in the model, store, agenda layer, and fixed-point engine; the
missing work is authoring, editing, and visibility in CLI/TUI.

## Goals

- Add first-class CLI commands to create, edit, and remove category actions.
- Add first-class TUI visibility and editing for category actions in Category
  Manager.
- Keep action semantics unchanged:
  - actions are source-centric
  - they fire when the owning category is assigned
  - action-produced assignments remain sticky

## Non-Goals

- Add new action kinds such as date-setting, mark-done, delete, or export.
- Change action firing order or fixed-point behavior.
- Add per-category execution timing controls.

## Current Reality

- Core model already has `Category.actions: Vec<Action>`.
- Supported action kinds are `Assign { targets }` and `Remove { targets }`.
- Engine behavior already persists action output as sticky `AssignmentSource::Action`.
- CLI can display actions in `category show`, but cannot create or remove them.
- TUI profile-condition editing exists, but action editing does not.

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
- level 2: action kind picker (`Assign` or `Remove`)
- level 3: category target picker using the same category selection affordances
  as profile conditions

## Implementation Plan

1. Extend CLI command surface and help text.
2. Add shared agenda/category mutation helpers for action list updates so CLI
   and TUI do not hand-roll the same validation logic.
3. Add TUI action-edit state alongside existing condition-edit state.
4. Surface action counts and summaries in Category Manager details.
5. Add regression tests for CLI, TUI state transitions, and action cascades.

## Validation Rules

- Disallow empty target sets.
- Resolve target categories by exact category identity, not alias text.
- Preserve existing action order in the stored vector.
- Keep reserved-category protections unchanged.

## Test Matrix

- CLI add assign-action updates category and retroactively reprocesses items.
- CLI add remove-action updates category and retroactively reprocesses items.
- CLI remove-action deletes the intended action by displayed index.
- TUI action list opens, edits, deletes, and survives refresh/reload.
- Action-created assignments remain after the triggering live condition
  auto-breaks.
- `Action::Remove` still defers removal until cascade completion.

## Risks

- The TUI action editor can easily duplicate condition-editor logic. Prefer a
  shared picker model where possible.
- Action loops are already bounded by the engine pass cap, but better
  validation and user-facing error messages will make failures easier to debug.

## Exit Criteria

- Users can author `Assign` and `Remove` actions from both CLI and TUI.
- Category Manager clearly shows when a category has actions attached.
- Existing engine tests still pass unchanged aside from UX-surface additions.
