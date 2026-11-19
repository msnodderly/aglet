---
title: Codebase Review and Simplification Plan
status: active
created: 2026-11-16
updated: 2026-11-17
---

# Codebase Review and Simplification Plan

Written by Claude (Fable 5.2, Claude Code) from a read-only review of `main` at
`cb9ce95` on 2026-11-16. Nothing in this plan has been implemented.

Goal: keep every feature, cut the amount of code a reader has to hold in their
head, and close two correctness and performance gaps found along the way.

## Summary

| Crate | Product lines | Test lines | Rating | Main issue |
|---|---|---|---|---|
| `aglet-core` | ~13k | ~13k | 8/10 | Bulk writes commit per item; three copies of one LLM client |
| `aglet-cli` | ~5.7k | ~4.2k | 6/10 | One 5.7k-line file, `Result<(), String>` everywhere |
| `aglet-tui` | ~34k | ~23.7k | 4/10 | One `App` object, 10.4k-line render file, hand-indexed forms |

Overall about 6.5/10. The engine and the test suite are the strong parts. The
TUI holds 64% of the product code and most of the avoidable complexity.

## How this was checked

- `cargo test --workspace`: 1,291 passed, 1 ignored, under 5 seconds of test time.
- `cargo clippy --workspace --all-targets`: 1 warning (`main.rs:2179`).
- `cargo fmt --all -- --check`: 55 diffs across 10 files.
- Function lengths from an awk script over non-test files.
- Timing on a scratch 2,000-item database built with `aglet import csv`, debug
  build, once on tmpfs and once on btrfs. The scratch DB was deleted.
- Not done: driving the TUI interactively, a release-build benchmark.

## Findings

### F1. Nothing enforces tests, lints, or formatting

No workflow under `.github/workflows/` runs `cargo test`, clippy, or fmt.
`release.yml` only calls `scripts/build-standalone-aglet.sh`. There are no git
hooks. The formatting drift above is the visible result.

### F2. Bulk writes commit once per item

`evaluate_items_internal` (`engine.rs:267`) calls `process_item_with_options`
per item. Each call opens its own savepoint with no outer transaction, so each
item is a separate fsynced commit. The CSV import loop (`main.rs:3887`) has the
same shape.

| Operation, 2,000 items, debug build | tmpfs | btrfs |
|---|---|---|
| `import csv` | 3.4 s | 26.2 s |
| `category create` matching every item | 1.5 s | 23.5 s |
| `category create` matching nothing | 1.4 s | 1.3 s |

The tmpfs to btrfs gap is commit cost. The same structure means a crash or
error partway through leaves some items reclassified and others not.

### F3. TUI refresh issues at least 6 queries per item

`App::refresh` (`app.rs:340`) runs, per item: one assignment query inside
`list_items`, three link queries, and two dependency lookups inside
`blocked_item_ids` (`workflow.rs:102`), which also calls `get_item` per
dependency because `or_insert(store.get_item(..)?.is_done)` evaluates eagerly.
`refresh` has 88 call sites outside tests.

### F4. TUI state allows invalid combinations

- `App` (`lib.rs:425`) has about 85 fields.
- `Mode` has 19 variants, and each mode's state lives in a separate
  `Option<...>` field. 51 call sites start with
  `let Some(state) = &self.<x> else { return }`.
- Key modifiers reach handlers through `self.transient.key_modifiers`
  (`input/mod.rs:7`) because handlers take `KeyCode`, not `KeyEvent`.
- Every TUI module does `use crate::*`, so nothing shows what a file depends on.

### F5. Forms are hand-placed with indexes mirrored in two files

| Function | Location | Lines |
|---|---|---|
| `render_view_edit_screen` | `render/mod.rs:8424` | 2,000 |
| `render_category_manager` | `render/mod.rs:7182` | 1,237 |
| `render_board_columns` | `render/mod.rs:2549` | 876 |
| `render_input_panel` | `render/mod.rs:5017` | 591 |
| `handle_input_panel_key` | `modes/board.rs:4468` | 392 |
| `handle_normal_key` | `modes/board.rs:2300` | 371 |

`view_edit/details.rs:162` maps `unmatched_field_index` values 0 to 8 onto a
two-column grid, with field 8 sitting between 4 and 5. `render/mod.rs` repeats
those numbers in 27 places. Adding a field means editing both by hand.

### F6. Three copies of one chat-completions client

`classification.rs:479-1140` defines `OllamaTransport`, `OpenRouterTransport`,
and `OpenAiTransport`, each with a reqwest implementation and a provider. All
three POST to `/chat/completions`. Each provider also writes to its own
hard-coded `/tmp/aglet-*-debug.log`, which cannot work on Windows, a release
target.

### F7. Logic duplicated between CLI and TUI

- Item sorting: `main.rs:5539` and `app.rs:2787`.
- `is_reserved_category_name`: `store.rs:2210`, `aglet.rs:2205`,
  `ui_support.rs:852`.
- `category_name_map`: `main.rs:4045` and `ui_support.rs:110`.

### F8. CLI structure

`main.rs` is 5,696 lines. `cmd_category` is 795 lines and `cmd_view` is 533.
Every command returns `Result<(), String>`, which produces 162
`.map_err(|e| e.to_string())` calls.

### F9. Assignment provenance is stored four ways

`Assignment` (`model.rs:317`) has `source`, `sticky`, `origin`, and
`explanation`. `explanation` already carries what `source` and `origin` say.
AGENTS.md has to instruct readers to treat `AutoClassified` with provider
`implicit_string` and `ImplicitMatch` as one concept.

### F10. Smaller items

- `preview_manual_category_toggle` (`aglet.rs:731`) copies the hierarchy into
  an in-memory store and remaps ids both ways (about 170 lines including
  `aglet.rs:2032-2204`). A savepoint plus rollback on the real store may do the
  same. Why the copy was chosen has not been checked.
- `Aglet` has `x`, `x_with_reference_date`, `x_cheap`, and `x_inner` variants
  for create, update, and process. One options argument would replace them.
- AGENTS.md calls local DBs test data, yet `store.rs` carries 7 migrations and
  `repair_missing_columns` (about 350 lines) that run on every open.
- `tests.rs` is one 23.7k-line file with 268 direct writes to `App` fields, so
  tests pin the internal layout that F4 and F5 would change.

## Plan

Each step is its own branch and PR, leaves `main` working, and can be reverted
with `git revert` unless marked otherwise. Order is by payoff per unit of risk.

### Phase 0. Gate

1. Run `cargo fmt --all` once, as a commit containing nothing else.
2. Add a `ci.yml` workflow running `cargo fmt --all -- --check`,
   `cargo clippy --workspace --all-targets -- -D warnings`, and
   `cargo test --workspace --locked` on pull requests and pushes to `main`.
3. Fix the one clippy warning.

Done when: a PR with a failing test or unformatted file shows a red check.

Status: done 2026-11-16 (PR #157).

### Phase 1. Bulk writes in one transaction (F2)

1. Make `Store::with_immediate_transaction` (`store.rs:1926`) reentrant: if
   `conn.is_autocommit()` is false, use a savepoint. Claim and category-order
   paths already call it and must keep working when nested.
2. Wrap `process_category_change` and
   `reevaluate_temporal_conditions_with_context`, including
   `apply_bulk_deferred_specials`, in that transaction.
3. Add a public `Aglet` method that runs a closure in a transaction, and use it
   around the CSV import loop.

Done when:
- A new core test forces an error on the Nth item of a bulk run and asserts no
  assignments changed for items 1 to N-1.
- The 2,000-item btrfs timings for `import csv` and matching `category create`
  are within 2x of the tmpfs numbers above. Record both in the PR.

Risk: with a semantic provider enabled, import would hold the write lock across
HTTP calls. See decision D2.

Status: done 2026-11-17. D2 taken as recommended: import saves with the cheap
path inside the transaction and calls `classify_item_semantic_after_save` per
item after the commit. Measured on btrfs, 2,000 items, debug build: `import
csv` 26.2 s to 2.5 s, matching `category create` 23.5 s to 1.1 s. Side effect:
`import csv --dry-run` no longer creates categories, because the dry run is
now rolled back.

### Phase 2. Constant-count refresh queries (F3)

1. `Store::list_items` loads all assignments in one query and groups by item.
2. Add `Store::list_all_item_links()` returning every link row once.
3. `blocked_item_ids` and the TUI link map are computed from those in memory.

Done when: the existing `blocked_item_ids_matches_single_item_checks` test
passes unchanged, and refresh time on the 2,000-item fixture is recorded before
and after in the PR.

Status: done 2026-11-17. `App::refresh` on 2,000 items with 400 dependency
links, debug build: 606 ms to 76 ms. `claimable_item_ids` got the same
treatment, since the ready queue calls it on every refresh.

### Phase 3. Core and CLI consolidation

One PR per item. None changes stored data.

1. **One chat-completions provider (F6).** A single transport trait taking
   base URL, optional API key, model, and timeout. The three settings structs
   stay as they are so persisted config does not change. One debug log under
   `std::env::temp_dir()`. Done when the three provider impls and three
   transport traits are gone and existing classification tests pass.
2. **Shared presentation helpers in core (F7).** Move item sorting, the
   category name map, and the reserved-name check into `aglet-core`; delete the
   CLI and TUI copies. Done when each helper has one definition.
3. **CLI error type (F8).** A `CliError` with `From<AgletError>` and friends,
   so `?` works. Done when `map_err(|e| e.to_string())` no longer appears in
   the CLI.
4. **Split `main.rs` (F8).** Move command handlers to
   `commands/{item,category,view,import,link}.rs`. Pure moves, no logic edits,
   done last so earlier diffs stay readable.
5. **Options argument for `Aglet` create, update, process (F10).** Replace the
   `_cheap`, `_inner`, and `_with_reference_date` variants.
6. **Preview via savepoint (F10).** First find out why the in-memory copy
   exists. Replace only if nothing depends on it.

Status 2026-11-17: item 1 done (commit `8fa2a00`, 343 added, 670 removed).
The plan said "one PR per item", but the work so far is on a single branch,
`simplify/phase-3-core-cli-consolidation`; see the handoff for how to split
it. Item 2 is half done: core helpers exist (`808cdb2`), callers not yet
switched. Items 3 to 6 not started.

### Phase 4. TUI

Order matters here: the harness comes first so later steps do not rewrite
tests twice.

1. **Test harness.** A helper that builds an `App` over an in-memory store,
   feeds key strings, and returns the rendered screen as text. New tests use
   it. Existing tests are ported only when their mode is migrated in step 3.
   Done when one existing test per mode family has been ported as proof.
2. **Pass `KeyEvent` to handlers (F4).** Remove `transient.key_modifiers`.
   Mechanical.
3. **Mode owns its state (F4).** Replace `Mode` plus the per-mode `Option`
   fields with one enum whose variants carry their state, and split `App` into
   shared data (items, categories, views, slots) and the current screen so
   handlers borrow the two halves separately. Migrate one mode per PR in this
   order: HelpPanel, ViewPicker, the three confirm dialogs, LinkWizard,
   GlobalSettings, SuggestionReview, CategoryColumnPicker, CategoryDirectEdit,
   InputPanel, item assign, CategoryManager, ViewEdit, then Normal. Each
   migrated mode moves its render function next to its key handler and
   replaces `use crate::*` with explicit imports in the files it touches.
   Done per mode when its `let Some(state) else return` guards are gone and
   its tests run through the harness.
4. **Form descriptors (F5).** For ViewEdit settings, define the fields once as
   a list (label, value getter, activation). Render order, j/k/h/l movement,
   and Enter dispatch read that list. Then apply the same to CategoryManager
   details and GlobalSettings. Done when `unmatched_field_index` and its
   numeric literals are gone and adding a field is a one-place change.
5. **Split `tests.rs`** by mode as tests are ported. No separate PR needed.

`render/mod.rs` shrinks as a side effect of steps 3 and 4. It does not need
its own step.

### Not planned

- Changes to engine semantics. The fixed-point design in `engine.rs` is sound.
- More crates, async, or a TUI framework.
- Removing or rewriting tests beyond porting them to the harness.

## Decisions needed

| ID | Question | Why it blocks |
|---|---|---|
| D1 | Squash migrations to one schema, or declare the `.ag` format stable? | AGENTS.md says greenfield, the Homebrew release says otherwise. Squashing is not reversible for existing files and needs its own plan. |
| D2 | During import, run semantic classification after the transaction commits? | Otherwise Phase 1 holds the write lock across LLM calls. Recommended: yes, use the cheap path inside the transaction. |
| D3 | Collapse `Assignment` provenance into `explanation` (F9)? | Touches stored columns. Recommended first step is read-only: count rows where `explanation` is null and where `source` disagrees with it, then write a decision record. `sticky` stays either way, since older sticky derived rows cannot be re-derived. |
| D4 | Flat screen enum or a screen stack? | Some modes return to a caller (`ItemAssignReturnTarget`, `NameInputContext`). Settle after the first two mode migrations in Phase 4 step 3. |

## Rollback

Phases 0 to 4 are code-only and revert cleanly. D1 and D3 change stored data
and are excluded from this plan until each has a decision record and a
migration with a tested downgrade or backup step.
