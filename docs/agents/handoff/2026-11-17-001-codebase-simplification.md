---
title: Codebase Simplification Handoff
updated: 2026-11-17
---

# Handoff: Codebase Simplification Plan

Written by Claude (Opus 5.5, Claude Code) for the next agent. Earlier work in
this series was done by Claude Fable 5.2 in the same session.

**Plan:** `docs/plans/codebase-review-and-simplification-plan.md` (status
`active`). Read its Findings and Plan sections first; this handoff only covers
where execution stands.

**Worktree:** `/home/code/src/aglet/.claude/worktrees/simplify`
**Current branch:** `simplify/phase-3-core-cli-consolidation` (pushed, no PR yet)

## Where things stand

| Phase | Branch | PR | Base | State |
|---|---|---|---|---|
| 0. CI gate | `simplify/phase-0-gate` | #157 | `main` | Done, CI green |
| 1. Bulk transactions | `simplify/phase-1-bulk-transactions` | #158 | phase-0 | Done |
| 2. Refresh queries | `simplify/phase-2-refresh-queries` | #159 | phase-1 | Done |
| 3. Core and CLI consolidation | `simplify/phase-3-core-cli-consolidation` | none | phase-2 | Items 1 done, 2 half done |
| 4. TUI | not started | | | |

The PRs form a stack: each targets the previous phase's branch. When a lower
PR changes, merge it upward (phase-0 into phase-1, phase-1 into phase-2, and so
on) and push each branch. That is how the clippy fix `1171d2a` reached #158
and #159.

The user asked to use `gh stack` for stacking. The `github/gh-stack` extension
is not installed, and installing it was blocked by the permission classifier.
Ask the user to run `gh extension install github/gh-stack` (or to allow it)
before switching the stack over; until then the manual base-branch stacking
above is in place.

## Measured results so far

| What | Before | After |
|---|---|---|
| `import csv`, 2,000 rows, btrfs, debug | 26.2 s | 2.5 s |
| `category create` matching 2,000 items, btrfs | 23.5 s | 1.1 s |
| `App::refresh`, 2,000 items, 400 links | 606 ms | 76 ms |
| `classification.rs` and callers (Phase 3 item 1) | | 343 added, 670 removed |

Test count on the tip: 1,297 passed, 1 ignored. fmt and clippy `-D warnings`
clean with local Rust 1.99. CI uses stable 1.100, which caught one extra
lint, so check the CI run, not only local clippy.

## Phase 3 detail

**Item 1, one LLM client: done** (`8fa2a00`). `ChatEndpoint`, `ChatTransport`,
`ReqwestChatTransport`, `SemanticProvider`, `classification_providers`, and
`BackgroundClassificationJob::run` in `crates/aglet-core/src/classification.rs`.
`Aglet::with_chat_transport` replaced `with_ollama_transport` and
`with_transports`. Debug log is
`std::env::temp_dir()/aglet-classification-debug.log`.

**Item 2, shared helpers: half done** (`808cdb2`). Added to core, not yet used:

- `aglet_core::query::{SortKey, SortField, SortDirection, compare_items_by_sort_keys, category_name_map}`
- `aglet_core::model::is_reserved_category_name`

Remaining for item 2:

1. CLI: delete `CliSortDirection`, `CliSortField`, `CliSortKey`,
   `compare_items_by_sort_keys`, `compare_items_by_sort_key`,
   `category_sort_display_value`, `compare_optional_values`,
   `compare_required_values`, and `category_name_map` from
   `crates/aglet-cli/src/main.rs`; use the core versions. `parse_sort_spec` and
   `parse_sort_field_and_direction` stay in the CLI but return core types.
   Update `main_tests.rs` (about 15 references).
2. TUI: `SlotSortKey` and `SlotSortColumn` are TUI-only (columns carry a
   `ColumnKind`). Map them to `SortKey` at the sort call site in
   `app.rs` (around line 2771): `ItemText` to `SortField::ItemText`,
   `SectionColumn { kind: When, .. }` to `SortField::WhenDate`,
   `SectionColumn { heading, kind: Standard }` to `SortField::Category(heading)`.
   Then delete `compare_items_for_sort_keys`, `compare_items_for_sort_key`,
   `standard_sort_value_for_heading`, `compare_some_values`, and
   `compare_optional_values` from `app.rs`. Replace `SlotSortDirection` with
   `SortDirection` if the churn is small.
   Behavior difference: for a tag column whose heading has no children, the
   TUI's `standard_sort_value_for_heading` returns `None` for every item, so
   the column does not sort. The CLI (and the new core helper) sorts assigned
   items by the category's own name. Switching the TUI to the core helper
   adopts the CLI behavior; call that out in the PR.
3. Delete `ui_support::category_name_map` and
   `ui_support::is_reserved_category_name`, and the private
   `is_reserved_category_name` in `store.rs` and `aglet.rs`; use the core
   ones.
4. Done when `rg 'fn (category_name_map|is_reserved_category_name|compare_items_by_sort_keys)'`
   shows one definition each.

**Items 3 to 6:** not started. See the plan for scope.

**PR split.** The plan says one PR per item. Items 1 and 2 are on one branch.
Simplest path: finish item 2, open one PR titled "Phase 3a: one LLM client
and shared sort helpers" based on `simplify/phase-2-refresh-queries`, then
branch `simplify/phase-3b-...` from it for items 3 and 4, and so on.

## Decisions taken this session

Recorded in `docs/decisions/implementation-decisions.md` under 2026-11-17:

- Nested transactions become savepoints.
- D2 from the plan: LLM classification runs after the import commit.
- One OpenAI-compatible client; stored settings unchanged.

Still open, all listed in the plan's "Decisions needed": D1 (squash
migrations), D3 (collapse assignment provenance), D4 (screen enum or stack).
D1 and D3 change stored data and need the user's decision before any work.

## Known gaps and follow-ups

- During bulk re-evaluation, a MarkDone special on a recurring item creates a
  successor and classifies it with the full provider list, including the LLM,
  inside the transaction. Low frequency; noted in PR #158.
- `import csv --dry-run` no longer creates categories (behavior change, called
  out in PR #158).
- `with_immediate_transaction` uses a fixed savepoint name
  `aglet_nested_txn`. SQLite resolves nested same-name savepoints to the
  innermost one, which is correct here; tested in
  `transaction_nests_inner_failure_keeps_outer_and_outer_failure_drops_all`.

## Working notes

- Build with `CARGO_TARGET_DIR=/home/code/src/aglet/target` to reuse the main
  checkout's build cache.
- The worktree session blocks compound shell commands with redirects or
  `git -C`; keep commands simple.
- Benchmark fixture: generate a 2,000-row CSV (`title,note` columns) and run
  `aglet --db <tmp>.ag import csv <csv> --title-col title --note-col note`.
  Use a path on a real disk (not tmpfs) for commit-cost numbers. The refresh
  timing used a temporary `#[ignore]` test calling `App::refresh` five times;
  it was not committed.
- Commit attribution line now in use: `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Smoke test for what has merged so far

On branch `simplify/phase-3-core-cli-consolidation`:

```bash
cargo run --bin aglet -- --db /tmp/smoke.ag import csv items.csv --title-col title --dry-run
cargo run --bin aglet -- --db /tmp/smoke.ag category list   # no new categories from the dry run
cargo run --bin aglet -- --db /tmp/smoke.ag import csv items.csv --title-col title
cargo run --bin aglet -- --db /tmp/smoke.ag category create Vendor
cargo run --bin aglet -- --db /tmp/smoke.ag                 # TUI: board loads, link and blocked markers show
```

With an LLM provider configured (`OPENAI_API_KEY` or a local Ollama), add an
item in the TUI and confirm a suggestion appears in review, and that
`aglet tui --debug` writes to `aglet-classification-debug.log` in the temp dir.
