# Plan: R3.6 View Manager UI Implementation

Date: 2026-02-17
Worktree: `/Users/mds/src/aglet-view-ui`
Branch: `codex/view-ui-r36`
Primary contract: `/Users/mds/src/aglet-view-ui/spec/tui-view-workflow-implementation.md`

## 1. Objective

Implement the R3.6 View Manager refactor in iterative, test-backed slices:

- T089: full-screen view manager shell
- T090: row-based boolean criteria authoring + preview summary
- T091: section authoring integrated into same screen
- T092: regression + smoke coverage

## 2. Constraints

- No schema/model changes unless persistence gate is explicitly triggered.
- Keep current view behavior stable while introducing the new screen.
- Use explicit save (`s`) and cancel (`q`/`Esc`) semantics in new manager.
- Keep old flow reachable behind fallback until shell is stable.

## 3. Detailed TODO List

Legend:

- `[ ]` pending
- `[~]` active
- `[x]` complete

### T089 - View Manager Shell

- [x] Add `Mode::ViewManagerScreen` and state container for 3 panes in `crates/agenda-tui/src/lib.rs`.
- [x] Add pane focus enum (`Views`, `Definition`, `Sections`) and navigation helpers.
- [x] Add entry path from normal/view palette into full-screen manager.
- [x] Render full-screen manager layout scaffold and footer hints.
- [x] Implement core key routing (`Tab`, `Shift+Tab`, `j/k`, `Enter`, `Esc`, `s`, `q`) with status feedback.
- [x] Migrate basic view list actions into left pane (`N`, `r`, `x`, `C` stub if needed).
- [x] Add tests for mode transitions and pane focus routing.

### T090 - Boolean Criteria Builder + Preview

- [x] Introduce criteria draft row model (`sign`, `category_id`, `join`, `depth`).
- [x] Render criteria rows in center pane.
- [x] Implement row editing keys (`N`, `x`, `Space`, `a`, `o`, `(`, `)`, `c`).
- [x] Add category picker integration for current row.
- [x] Implement representability validation against current query model.
- [x] Add live preview summary row (matching count, delta).
- [x] Add tests for row edits, validation failures, and preview updates.

### T091 - Section Authoring Integration

- [x] Render section list in right pane.
- [x] Implement section add/remove/reorder (`N`, `x`, `[`, `]`).
- [x] Implement section detail editor (`t`, row edits, `i`, `r`, `h`).
- [x] Wire section draft save path to existing store update flow.
- [x] Ensure unmatched config entry is reachable in same screen (`u`).
- [x] Add tests for section mutations and persistence handoff.

### T092 - Regression + Smoke Coverage

- [x] Add unit tests for complete view-manager key map routing.
- [x] Add tests for explicit save/cancel behavior and no-implicit-save guarantees.
- [x] Add tests for representative invalid criteria structures.
- [x] Update `docs/test-script-tui-smoke-e2e.md` with R3.6 coverage steps.
- [x] Add a focused manual script for view-manager authoring path.

## 4. Commit Strategy (Frequent)

Planned checkpoints (minimum):

- [x] Commit A: T089 shell state + mode + empty render scaffold + tests.
- [x] Commit B: T089 key routing + pane focus + entry path + tests.
- [~] Commit C: T090 criteria row model + rendering + edits + tests.
- [x] Commit D: T090 preview + validation + tests.
- [x] Commit E: T091 sections integration + tests.
- [x] Commit F: T092 smoke/docs updates.

Rule:

- Commit at the end of each vertical slice and after any non-trivial passing test milestone.

## 5. Daily Tracking Log

### 2026-02-17

- [x] Created new worktree and branch for R3.6 implementation.
- [x] Wrote detailed plan and TODO checklist.
- [x] Began T089 coding.
- [x] Implemented full-screen view manager shell scaffold and pane-navigation tests.
- [x] Committed checkpoint A (`6a2e621`).
- [x] Wired view-manager left-pane actions (`N/r/x/C`) with return-to-manager flows.
- [x] Added tests for manager create-cancel, delete-cancel, and clone behavior.
- [x] Started T090 with definition-pane criteria rows, row edits, and save path.
- [x] Added tests for criteria toggle/save and add/remove/save flows.
- [x] Added definition-pane category picker modal for criteria-row category selection.
### 2026-02-18

- [x] Added T090 representability validation for OR/nesting/duplicate-conflict rows.
- [x] Added T090 save-blocking + validation error surfacing in Definition pane.
- [x] Added T090 validation-failure tests (`OR` and nested depth).
- [x] Switched normal browsing to unified preview pane (`p/o`, Tab focus, provenance drill-in).
- [x] Added first T091 integration slice:
  section add/remove/reorder in manager pane, unmatched settings entry from manager, and manager-return draft handoff from section editor.
- [x] Added T091 tests for section pane mutations, editor return handoff, unmatched save path.
- [x] Updated `docs/test-script-tui-smoke-e2e.md` with manager + preview flow coverage.
- [x] Added focused manager test script: `docs/test-script-view-manager-r36.md`.
- [x] Added T092 save/cancel regression (`Esc` from manager discards unsaved draft changes).
- [x] Added T091 section-detail regression for `h`, on-insert assign, and on-remove unassign in manager path.
