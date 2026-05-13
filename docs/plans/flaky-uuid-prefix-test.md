---
title: Stabilize TUI test — search_bar_filters_match_item_uuid_prefix
status: draft
created: 2026-05-12
---

# Stabilize TUI test — search_bar_filters_match_item_uuid_prefix

## Context

`crates/aglet-tui/src/tests.rs:20442` exercises the TUI search bar's
ability to filter items by UUID prefix. The fixture
(`make_two_section_store`, line 19451) creates four items with **random**
UUIDs; the test then grabs the first **3 hex characters** of one item's
UUID and asserts that exactly one item matches.

With 4 random UUIDs and a 3-char prefix (4096 possibilities per character
position), there is a measurable chance two items in the Work section
share their first 3 chars, causing `assert_eq!(app.slots[0].items.len(),
1, ...)` to fail with `left: 2, right: 1`. Observed in a full
`cargo test --workspace` run on 2026-05-12; rerun of the same test in
isolation passed, confirming flakiness, not a regression.

Tracking bug: `54225c8f` in `../bugtracker.ag`.

## Approach

Two complementary changes — use a longer prefix and choose it deterministically
from the actual UUID, so the test asserts what it claims to assert
(unique-prefix matching) without depending on random-UUID luck.

### Step 1 — Pick a unique prefix at runtime

Replace the hard-coded 3-char slice at `tests.rs:20454`:

```rust
let uuid_prefix = work_items[0].id.to_string()[..3].to_string();
```

with a helper that walks all items in the store and returns the shortest
prefix of `work_items[0].id` that does not match any other item's UUID:

```rust
fn unique_uuid_prefix(target: uuid::Uuid, others: &[uuid::Uuid]) -> String {
    let target_str = target.to_string();
    for len in 4..=target_str.len() {
        let candidate = &target_str[..len];
        if others.iter().all(|o| !o.to_string().starts_with(candidate)) {
            return candidate.to_string();
        }
    }
    target_str
}
```

Call site:

```rust
let all_items = store.list_items().expect(...);
let other_ids: Vec<_> = all_items.iter()
    .filter(|i| i.id != work_items[0].id)
    .map(|i| i.id)
    .collect();
let uuid_prefix = unique_uuid_prefix(work_items[0].id, &other_ids);
```

Start at length 4 (not 1) so we still exercise short-prefix matching
behavior, but extend as needed so the assertion is logically guaranteed
to hold regardless of fixture UUID values.

### Step 2 — Place the helper in test-support, not in the test body

Put `unique_uuid_prefix` next to `make_two_section_store` (or in a
sibling helper module) since other UUID-prefix-search tests
(`fuzzy_search_ranks_title_matches_before_substring_fallback_...` and
neighbors) may want the same guarantee. Scoped to the `tests` module —
not production code.

### Step 3 — Tighten the related compact-UUID test if affected

Audit nearby tests for the same 3/8-char hard-coded slice pattern:

```sh
rg "id.to_string\(\)\[\.\.[0-9]+\]" crates/aglet-tui/src/tests.rs
```

For each match that asserts uniqueness, apply the same `unique_uuid_prefix`
treatment. Do **not** rewrite tests that intentionally exercise
ambiguous-prefix behavior.

## Files to modify

- `crates/aglet-tui/src/tests.rs` — add `unique_uuid_prefix` helper near
  `make_two_section_store`; replace the 3-char slice at line 20454;
  audit + fix any sibling tests with the same pattern.

## Verification

```sh
# Run the previously flaky test in a loop. With the fix, all 1000
# iterations must pass.
for i in $(seq 1 1000); do
  cargo test -p aglet-tui --lib tests::search_bar_filters_match_item_uuid_prefix \
    --quiet 2>&1 | grep -q "test result: ok" || { echo "FAIL on iter $i"; break; }
done
echo "1000 iterations passed"

# Confirm related tests still pass after any sibling fixes
cargo test -p aglet-tui --lib search_bar_filters
cargo test -p aglet-tui --lib uuid_prefix

# Full TUI suite is still green
cargo test -p aglet-tui --lib
```

## Out of scope

- Changing the production UUID-prefix-search semantics. The bug is in
  the **test** — production behavior (resolve a prefix to a unique item
  or report ambiguity) is correct and already exercised by the CLI
  `short_uuid_prefix_works_for_show_and_edit` integration test added in
  the harness work.
