# Special Actions Follow-Up Plan

Status: Deferred Proposal

## Summary

After category action authoring, category date conditions, and recurrence are in
place, extend the action system with higher-level side effects such as setting a
date, marking done, discarding an item, or exporting it. This should be a
separate phase because these actions touch core lifecycle code and are easier to
design once recurrence and done sequencing are stable.

## Why Deferred

- `Action::Assign` / `Action::Remove` already cover the current missing UX gap.
- Date-setting actions overlap with unresolved date-condition semantics.
- Mark-done/discard actions overlap directly with the recurrence/done pipeline.
- Export/file side effects introduce I/O policy questions that do not belong in
  the first rule-surface expansion.

## Proposed Action Families

### Phase 4A: Date-setting

Potential variants:

- `Action::SetWhenAbsolute { datetime }`
- `Action::SetWhenRelative { offset }`
- `Action::ClearWhen`

Recommendation: do not implement free-form natural-language action payloads at
first. Use structured values and let CLI/TUI render friendly controls.

### Phase 4B: Lifecycle

Potential variants:

- `Action::MarkDone`
- `Action::Discard`

These must route through shared agenda-layer helpers instead of bypassing item
lifecycle invariants.

### Phase 4C: External side effects

Potential variants:

- `Action::Export { profile_id }`
- future command/macro/integration hooks

This should come last because it introduces persistence and security questions
beyond the database itself.

## Core Design Constraints

- Do not let action variants sidestep existing agenda invariants.
- Route date mutations through `set_item_when_date(...)`-style helpers.
- Route done/discard mutations through agenda-layer lifecycle functions.
- Preserve deterministic ordering when multiple heterogeneous actions exist on a
  category.
- Keep action-produced side effects sticky and audit-friendly.

## Recommended Order

1. Ship action authoring for existing assign/remove actions.
2. Ship category date conditions.
3. Ship recurrence + done-pipeline sequencing.
4. Add date-setting actions.
5. Add lifecycle actions (`MarkDone`, `Discard`).
6. Revisit export/integration actions only after a connector/output policy
   exists.

## Test Matrix

- date-setting action updates `when_date` and reserved `When` sync together.
- mark-done action respects actionable checks and recurrence generation.
- discard action writes deletion log and does not leave orphaned side effects.
- mixed action lists execute in a documented, deterministic order.
- error handling is transactional when a later special action fails.

## Exit Criteria

- We have a documented, sequenced design for special actions.
- No special-action work starts before recurrence and done sequencing land.
- Any eventual implementation reuses agenda lifecycle APIs instead of open-coded
  store mutations.
