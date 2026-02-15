# Deferred Findings from Phase 2 Review

Items identified during the Phase 1-2 code review that are not blockers
for Phase 3 but should be addressed in later phases.

---

## 1. Box `Condition::Profile` criteria (clippy `large_enum_variant`)

**Phase**: 3 or 11 (Hardening)
**File**: `crates/agenda-core/src/model.rs:55`

`Condition::ImplicitString` carries no data; `Condition::Profile { criteria: Query }`
carries ~216 bytes inline. Clippy suggests `Box<Query>` to reduce enum size.

At MVP scale this is harmless (conditions are rarely bulk-allocated), but if
conditions are stored in large vectors or frequently cloned, boxing would help.

**Fix**: Change to `Profile { criteria: Box<Query> }` and update all
construction/match sites.

---

## 2. N+1 assignment loading in `list_items`

**Phase**: 11 (Hardening / T069 performance check)
**File**: `crates/agenda-core/src/store.rs` — `list_items()` and `load_assignments()`

`list_items` loads all items, then calls `load_assignments` per item (one
SELECT per item). For 1,000 items this is 1,001 queries.

**Fix**: Use a single JOIN query or batch-load all assignments into a
`HashMap<ItemId, Vec<...>>` and distribute them.

---

## 3. `update_category` ignores caller's `modified_at`

**Phase**: Defer indefinitely (by design)
**File**: `crates/agenda-core/src/store.rs` — `update_category()`

The store always sets `modified_at = Utc::now()` regardless of the value on the
incoming `Category` struct. This is correct (the store is the timestamp authority),
but means tests cannot control the exact timestamp. If this becomes a testing
pain point, consider accepting an optional override parameter.
