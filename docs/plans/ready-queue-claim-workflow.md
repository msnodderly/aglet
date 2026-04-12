---
title: Workflow-Backed Ready Queue, Claim, and Release
status: shipped
created: 2026-02-20
shipped: 2026-03-21
---

# Plan: Workflow-Backed Ready Queue, Claim, and Release

## Summary

Implement a workflow model where:

- `Ready` remains a durable manual grooming/approval category.
- `Claimed` / `In Progress` is a separate configurable claim-target category.
- `Done` remains the built-in terminal state.
- "Eligible to be claimed" is computed, not stored: `Ready && !blocked && !done && !claimed`.

User-facing surfaces:

- TUI exposes a built-in immutable `Ready Queue` view.
- CLI adds `agenda ready` to list claimable items.
- CLI redesigns `agenda claim` to use workflow config only.
- CLI adds `agenda release` with visible alias `agenda unclaim`.

## Public API and Interface Changes

Add workflow config persisted per database in `app_settings` as JSON under key `workflow.ready_queue.v1`.

Add a new core type:

```json
{
  "ready_category_id": "UUID-or-null",
  "claim_category_id": "UUID-or-null"
}
```

Add a new public model type in `agenda-core`:

```rust
pub struct WorkflowConfig {
    pub ready_category_id: Option<CategoryId>,
    pub claim_category_id: Option<CategoryId>,
}
```

Add core/store helpers:

- `Store::get_workflow_config() -> Result<WorkflowConfig>`
- `Store::set_workflow_config(&WorkflowConfig) -> Result<()>`

Add new CLI commands:

- `agenda ready`
- `agenda release <ITEM_ID>`
- `agenda unclaim <ITEM_ID>` as a visible alias of `release`

Redesign existing CLI command:

- `agenda claim <ITEM_ID>` becomes config-driven only.
- Remove `--claim-category` and `--must-not-have` from the public interface.
- Update help/docs/tests accordingly.

Reserve a second built-in system view name:

- `Ready Queue`

## Behavior Contract

### Workflow configuration

A DB is "workflow-configured" only when both configured category IDs exist and are distinct.

If workflow config is incomplete:

- TUI does not show `Ready Queue`.
- `agenda ready`, `agenda claim`, and `agenda release` fail with a clear setup error and a hint to configure roles in the TUI Category Manager.

Invalid config rules:

- `ready_category_id` and `claim_category_id` must be different.
- Neither role may point to a reserved category.
- Numeric categories cannot be assigned either role.
- Missing/deleted category IDs are treated as unconfigured for runtime behavior.

### Claimable predicate

An item is claimable if all of the following are true:

- it has the configured `ready_category_id`
- it does not have the configured `claim_category_id`
- `item.is_done == false`
- it is not dependency-blocked by any unresolved `depends-on` prerequisite

This predicate must be shared across:

- TUI `Ready Queue`
- CLI `agenda ready`
- CLI `agenda claim`

### Claim

`agenda claim <ITEM_ID>` is strict and has no `--force` in v1.

`claim` must fail unless the item is currently claimable.

Precondition failures use deterministic messages in this order:

1. item is done
2. item is already claimed
3. item is missing the configured Ready category
4. item is dependency-blocked

Implementation requirement:

- perform all checks and the assignment inside one `BEGIN IMMEDIATE` transaction
- then assign the configured claim target category
- preserve existing assignment side effects:
  - exclusive sibling clearing
  - subsumption
  - category `on assign` / `on remove` actions
  - item reprocessing

This preserves the "begin transaction / reserve this item for active work" semantics and prevents two actors from claiming the same item successfully.

### Release / Unclaim

`agenda release <ITEM_ID>` removes the configured claim target category.

Rules:

- fail if workflow config is incomplete
- fail if the item is not currently claimed
- do not touch the `Ready` category
- do not touch `Done`

Because `release` only removes the claim category, an item automatically returns it to the queue if it is still:

- Ready
- not blocked
- not done

### Done / reopen

Keep built-in `Done` exactly as the terminal state.

When marking an item done:

- do not special-case `Ready`; existing category schema side effects remain authoritative
- automatically remove the configured claim target if present
- do not auto-edit any other workflow category

When marking an item not-done:

- do not auto-restore the claim target
- do not auto-edit `Ready`

This means reopened items naturally re-enter the ready queue if they are still Ready, not blocked, and not claimed.

## Implementation Approach

### 1. Core workflow config and claimability helpers

Add `WorkflowConfig` to `agenda-core`.

Add store helpers to serialize/deserialize it through `app_settings`.

Add core helpers for workflow evaluation so CLI and TUI do not re-implement the rules separately.

Recommended shape:

- one helper that evaluates single-item claimability and returns a typed outcome/reason
- one helper that filters a slice/list of items down to claimable items
- one helper that builds the synthetic `Ready Queue` view definition from workflow config

Do not persist `Ready Queue` as a normal DB row. The current `View` query model cannot encode `not blocked`, `not done`, or `not claimed`, so this must be a generated/system view backed by workflow logic.

### 2. Core claim/release operations

Replace the current name-based `claim` wrapper behavior with workflow-aware core operations.

Add a new transactional claim method in `Agenda` that:

- loads current item state inside `with_immediate_transaction`
- re-evaluates claimability inside the transaction
- assigns the configured claim category if allowed
- returns `ProcessItemResult`

Add a release method in `Agenda` that:

- checks the configured claim category is present
- removes it
- reprocesses the item

Update `mark_item_done` in `Agenda` so it auto-clears the configured claim category before final processing.

### 3. CLI surface

Add `Command::Ready`.

`agenda ready` should support list-like output ergonomics:

- repeated `--sort <KEY>`
- `--format table|json`

Do not add category or blocked/done override flags to `ready`; that would undermine the fixed queue semantics.

Rendering behavior:

- build the synthetic `Ready Queue` view
- prefilter items to only claimable items
- render via the existing view/table output path so the output matches current CLI view/list style

Redesign `Command::Claim`:

- keep only `item_id`
- remove the old explicit precondition flags from help/parser/tests
- on config failure, return a setup hint
- on precondition failure, return the specific reason

Add `Command::Release` with visible alias `unclaim`:

- signature: `agenda release <ITEM_ID>`
- same item-id resolution behavior as other item commands
- success message should clearly mention the claim category removed

Reserve `Ready Queue` as a built-in system view name so users cannot create or rename a mutable view to that name.

Teach `view show "Ready Queue"` to render the generated system view using the same workflow predicate. Do not support editing, renaming, deleting, or cloning this view.

### 4. TUI surface

Expose workflow config in the existing Category Manager details pane.

Extend `CategoryManagerDetailsFocus` for tag categories with two new toggle rows:

- `Ready Queue`
- `Claim Target`

Rules:

- only one category can own each role
- the same category cannot own both roles
- toggling a role on a category replaces the previous owner of that role
- attempting to assign both roles to the same category is rejected with a status message
- workflow-role toggles are unavailable for reserved or numeric categories

Keep the current Details pane structure and add these toggles to the non-numeric flags list rather than creating a new mode or settings screen.

Add a generated immutable `Ready Queue` view to the TUI view list when workflow config is complete.

Placement:

- show after `All Items`
- before user-defined mutable views

Behavior:

- view picker can select it normally
- it renders through the standard board/list pipeline
- it is immutable like `All Items`
- rename/edit/delete/clone actions must be blocked with explicit system-view status text

Implementation detail:

- `App::refresh()` should load workflow config, synthesize the `Ready Queue` view, inject it into `self.views`, and prefilter items by the shared claimable predicate before calling `resolve_view(...)`

No dedicated hotkey for `Ready Queue` in v1. View-picker access is sufficient.

## Data and Migration Notes

No schema migration is needed.

Use `app_settings` only.

Add a constant key for workflow config:

- `workflow.ready_queue.v1`

Treat a missing setting as `WorkflowConfig::default()`.

Treat malformed JSON as unconfigured at runtime rather than crashing the TUI refresh loop. Workflow-specific commands should still fail cleanly because the config will appear incomplete.

## Test Plan

### Core tests

- workflow config round-trips through `app_settings`
- missing workflow config returns default empty config
- claimability helper returns claimable only for `Ready && !blocked && !done && !claimed`
- claim fails when config is incomplete
- claim fails when item is done
- claim fails when item is already claimed
- claim fails when item lacks Ready
- claim fails when item is dependency-blocked
- claim succeeds when item is eligible
- concurrent claim race still allows exactly one winner
- release fails when config is incomplete
- release fails when item is not claimed
- release succeeds and removes only the claim category
- mark done clears claim category
- mark not-done does not restore claim
- reopened Ready item becomes claimable again if otherwise eligible

### CLI tests

- clap parses `ready`
- clap parses `release` and alias `unclaim`
- clap no longer exposes `claim` override flags
- `agenda ready` lists only claimable items
- `agenda ready --format json` uses the expected ready-queue output shape
- `agenda ready --sort ...` uses existing sort parsing
- `agenda claim` success path moves eligible item into claim target
- `agenda claim` failure messages match the specified precondition order
- `agenda release` success/failure messages are correct
- `agenda view show "Ready Queue"` renders the generated system view
- view create/rename/clone rejects reserved name `Ready Queue`

### TUI tests

- Category Manager shows and persists `Ready Queue` role toggle
- Category Manager shows and persists `Claim Target` role toggle
- setting one role replaces the previous owner of that role
- selecting the same category for both roles is rejected
- `Ready Queue` view appears when config is complete
- `Ready Queue` view is hidden when config is incomplete
- `Ready Queue` is immutable in view picker/edit actions
- `Ready Queue` displays only claimable items
- claiming an item removes it from `Ready Queue`
- releasing a claimed Ready item returns it to `Ready Queue`
- marking a claimed item done removes it from `Ready Queue` and clears claim
- reopening a Ready item leaves it unclaimed and visible again if not blocked

## Files Expected to Change

- `crates/agenda-core/src/model.rs`
- `crates/agenda-core/src/store.rs`
- `crates/agenda-core/src/agenda.rs`
- `crates/agenda-cli/src/main.rs`
- `crates/agenda-tui/src/lib.rs`
- `crates/agenda-tui/src/app.rs`
- `crates/agenda-tui/src/modes/category.rs`
- `crates/agenda-tui/src/render/mod.rs`
- relevant unit/integration test sections in CLI/TUI/core crates
- `AGENTS.md` if any surprising implementation gotchas are discovered during implementation

## Assumptions and Defaults Chosen

- `Ready` is the durable grooming approval category, but claim/done continue to honor any existing schema side effects such as exclusive-sibling clearing or category actions.
- `Done` remains the built-in terminal state.
- `claim` is strict and has no `--force` in v1.
- `done` clears the claim target automatically.
- reopening an item does not auto-reclaim it.
- primary release command is `release`; `unclaim` is a visible alias.
- `Ready Queue` is a generated immutable system view.
- `agenda ready` supports list-like sorting and output formatting, but not arbitrary filter overrides.
- no CLI workflow-config editing is added in v1; configuration is TUI-accessible via Category Manager.
