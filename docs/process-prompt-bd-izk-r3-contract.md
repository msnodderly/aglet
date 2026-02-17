# Implementer Prompt: bd-izk - T070 R3 Contract Adoption

## Context

R3 is now the top-priority stream: TUI View + Category Workflow Streamlining.
The implementation contract is `spec/tui-view-category-workflow.md`.

This issue is the gate for downstream tasks:
- `bd-1mp` (T071 board layout)
- `bd-2nq` (T072 full view editor)
- `bd-2e8` (T074 shortcut model)

Current code already supports core view/query semantics in model/store/query,
but TUI still uses split-pane section selector and include-only view editing.

## What To Read

1. `spec/tui-view-category-workflow.md`
2. `spec/roadmap-current.md` (R3 section)
3. `spec/tasks.md` (Phase 2)
4. `crates/agenda-tui/src/lib.rs`
5. `docs/test-script-tui-smoke-e2e.md`
6. `spec/decisions.md` (TUI workflow and terminology decisions)

## What To Build

Define and lock the execution contract for R3 without schema/model expansion.

Required outputs:
- Confirm `spec/tui-view-category-workflow.md` is the source-of-truth for
  R3 behavior and wording.
- Record a scope note that unmatched “always show when empty” pin-mode is
  deferred and this phase keeps existing core model/store fields.
- Clarify R3 acceptance language so downstream tasks can implement directly.
- Keep CLI compatibility scope only (no new CLI feature surface required here).

## Tests To Write

No runtime tests required for this contract-only issue.

Validation expectation:
- Downstream issue prompts and implementation reference the same contract terms
  with no ambiguity around unmatched behavior, shortcuts, and editor scope.

## What NOT To Do

- Do not change `agenda-core` schema/model for unmatched policy in this issue.
- Do not implement board/editor/shortcut code here (those belong to T071+).
- Do not broaden scope into safety-contract tasks (T024/T025) in this issue.

## How Your Code Will Be Used

This issue provides a stable implementation baseline for all R3 coding slices.
Other agents should be able to execute T071-T076 with no product-level
ambiguity and minimal design churn.

## Workflow

1. Update docs/spec notes only.
2. Keep edits tightly scoped to R3 contract language.
3. Ensure acceptance criteria match planned T071-T076 slices.
4. Commit with a docs-scoped message.

## Definition of Done

- R3 contract is explicit and actionable.
- Deferred unmatched pin-mode note is present.
- No core schema/model changes introduced.
- Downstream task owners can implement without additional product decisions.
