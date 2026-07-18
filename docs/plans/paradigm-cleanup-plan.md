# Paradigm cleanup plan

Follow-up to PR #155 (agenda-paradigm-repair). Implements the findings from the
2026-07-17 codebase improvement report. Branch `paradigm-cleanup`, stacked on
`agenda-paradigm-repair` (d1fe196) because every item builds on the veto and
special-action machinery that PR introduces.

Finding IDs (A1–D5) refer to the improvement report. Order below is
implementation order: correctness first, then consolidation, then the big
write-path unification, then cleanups that ride on it, then TUI feature work,
then mechanical splits last so they don't pollute the semantic diffs.

## Checklist

### Correctness
- [x] **A1 — Bulk reevaluation applies special-action effects.**
      `EvaluateAllItemsResult` carries per-item deferred specials;
      `process_category_change` and `reevaluate_temporal_conditions` apply them
      via `apply_deferred_specials`. Test: category edit that newly matches an
      item whose category has a MarkDone action → item is done afterwards.
- [x] **A2 — Depth-capped specials warn visibly.** Dropped specials surface as
      an assignment-event-style warning in `ProcessItemResult`, not just a
      debug log. Test: 4-deep SetWhen chain reports the drop.
- [x] **A3 — Rejecting a suggestion also vetoes.** Decision: "reject ⇒ veto;
      accept/manual-assign clears both." Implemented in
      `reject_classification_suggestion`; documented in product-decisions.md.
      Test: reject implicit-string suggestion, literal auto-apply cannot
      re-assign.
- [x] **D4 — Decision recorded: subsumption does not fire parent actions.**
      Paragraph in product-decisions.md documenting the divergence from
      Agenda's "or one of its children" wording, and why.

### Consolidation
- [x] **B4 — Migrations become an ordered table.** `apply_migrations` iterates
      `MIGRATIONS: &[(i32, fn)]` in version order; idempotent column repairs
      move to a separate labeled repair pass. Existing migration tests stay
      green.
- [x] **B3 — One renderer per condition.** `Condition::render(&self, resolve)`
      in model.rs; engine, CLI `category show`, and CLI `remove-condition`
      delegate. Delete the duplicated match arms.
- [x] **B2 — Origin derives from explanation.** `AssignmentExplanation::origin()`
      returns the canonical origin string; writers stop hand-assembling the
      pair. Storage column unchanged (values identical), so no migration.
- [x] **C4 — One manual-assignment helper.** `assign_item_manual`,
      `assign_item_numeric_manual`, and `mark_item_done` share a
      `write_manual_assignment` helper (veto clear + was-assigned check +
      write + subsumption + trigger computation).
- [x] **B1 — The engine is the single assignment write path.**
      `pending_action_triggers` generalizes to `pending_assignments:
      Vec<AssignmentIntent>` (category, source, sticky, origin/explanation,
      numeric_value). Aglet-layer writers (manual assign, classification
      auto-apply, suggestion accept, section insert) submit intents instead of
      writing rows; the engine applies them with uniform veto/exclusivity/
      subsumption handling and fires actions off the resulting events.
      `apply_category_assignment` and the duplicate exclusive-sibling checks
      are deleted. All 1,285 tests must stay green; behavior-identical except
      where the old path skipped checks the engine performs.

### Simplification
- [ ] **C2 — `Condition::ImplicitString` retired from storage.** Serde shim
      drops it on read; writers never emit it; variant kept one release for
      decode compatibility, all match arms collapse to a deprecation no-op.
- [ ] **C1 — Retire the store-clone toggle preview.**
      `item_assign_reapply_status` and `preview_manual_category_toggle` are
      removed; the picker relies on veto semantics and existing guard errors
      (subsumption-descendant unassign). TUI tests updated.
- [ ] **C3 — Calendar math delegates to jiff.** `days_in_month`,
      `weekday_from_u8`/`weekday_to_u8` wrappers, and manual month/year
      arithmetic in `RecurrenceRule::next_date` replaced with jiff
      equivalents. Clamping tests (Jan 31 → Feb 28/29 etc.) stay green.

### TUI feature work
- [ ] **D2 — Vetoes visible in the TUI.** Assign picker shows a `[-]` marker on
      vetoed categories; Space on a vetoed row clears the veto and assigns
      (matching `assign_item_manual` semantics); item inspector lists vetoes.
- [ ] **D1 — TUI authoring for new rule vocabulary.** Category manager action
      editor gains kind selection for AssignNumeric (target + value),
      SetWhen (date expression), MarkDone, and Delete (gated on
      allow_delete_action); numeric conditions get an editor row. Scope
      honestly: if the input plumbing balloons, land MarkDone/Delete +
      rendering first and file the rest.

### Mechanical (last, so diffs above stay readable)
- [ ] **C5a — Extract inline test modules.** `aglet.rs` (~4.0k) and
      `store.rs` (~2.4k) test modules move to sibling `*_tests.rs` files (or
      `#[path]` includes); `cli/main.rs` tests move to `main_tests.rs`.
- [ ] **C5b — Deferred: directory splits.** `tui/tests.rs` (23.7k),
      `board.rs` (7.2k), and CLI handler modules are split opportunistically
      in future PRs that touch them — recorded here, deliberately not done in
      this branch to keep it reviewable.

### Explicitly out of scope
- **A4** — test flake: watch-only, nothing to implement.
- **D3** — ValidationCondition: needs its own spec (pre-assignment gate is a
  different mechanism).
- **D5** — standing FR backlog: tracked in aglet-features.ag.

## Verification bar

Per item: workspace tests + clippy green, commit per finding. At the end:
end-to-end CLI + TUI (tmux) exercise of A1, A3, B1, C1, D2 behaviors, then PR
stacked on #155.
