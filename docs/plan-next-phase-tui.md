# Next Phase: TUI Completion After T010

Date: 2026-02-16

## Branch Progress Update (Not Merged Yet)

- `T012` implemented on branch `codex/bd-2ji-tui-inline-text-edit`:
  inline text edit (`e`) with save/cancel and re-evaluation refresh.
- `T013` implemented on branch `codex/bd-2ji-tui-inline-text-edit`:
  note create/edit (`m`) with empty-to-clear behavior.
- `T014` implemented on branch `codex/bd-2ji-tui-inline-text-edit`:
  inspect-panel unassign picker (`u`) with explicit select/confirm flow.
- Both features have local manual smoke validation; merge is intentionally
  pending user manual test confirmation.

This note references:
- `spec/product-current.md`
- `spec/gaps.md`
- `spec/roadmap-current.md`
- `spec/tasks.md`
- CLI demos in `docs/demo-*.md` (especially `demo-complete-cli-e2e-demo-log.md`, `demo-view-logic-demo-run.md`, and `demo-literate-cli-demo-global-priority-reuse.md`)

## Phase Summary

The next phase is still **Phase 1 / R2: TUI Completion For SLC**. With `T010` in place, the highest-value remaining slices are:

1. `T012` inline item text editing.
2. `T013` note create/edit flow.
3. `T014` inspect-panel unassign action.
4. `T015` empty/error state hardening.
5. `T016` smoke script proving daily loop end-to-end in TUI.

## Why This Order

CLI demos show the target low-friction workflows: fast capture, quick recategorization, view-based triage, and rapid correction when classification is imperfect. The TUI should make these paths at least as easy as CLI for day-to-day use.

- `T012` and `T013` remove the biggest edit-through gap (text + note still requiring CLI).
- `T014` closes the inspect-panel loop so provenance inspection can immediately drive correction.
- `T015` makes these flows resilient in real data conditions (empty views/sections, mutation failures).
- `T016` locks behavior with a repeatable acceptance script before moving to safety-contract work.

## Workflow Expectations From Demo Evidence

The TUI should optimize for these specific examples from existing CLI demos:

- Capture item -> immediate date parse feedback -> continue triage.
- Build category branches quickly (`Work -> Project Y -> Frabulator`) while staying in flow.
- Keep multi-file visibility across views with fast switching.
- Make include/exclude view outcomes and empty-result states obvious.
- Support correction loops (done/delete/restore or unassign/move/edit) without mode confusion.

## Exit Criteria For Remaining R2 Tasks

- No routine daily loop step requires dropping to CLI for basic capture/edit/triage/taxonomy operations.
- Error states are actionable and not silent.
- A smoke test script demonstrates add/move/remove/done/delete/edit/category flows in one run.
