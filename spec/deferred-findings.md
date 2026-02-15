# Deferred Findings

Items identified during code reviews that are not blockers for the current
phase but should be addressed later.

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

---

## 4. SubstringClassifier: underscore is not a word character

**Phase**: 2+ (Unicode / advanced matching)
**File**: `crates/agenda-core/src/matcher.rs:57`

`is_ascii_word_char` checks `is_ascii_alphanumeric()` only — underscore is
not treated as a word character. Standard regex `\b` treats `_` as a word
char, so the behaviors differ: `"Sarah_Jones"` would match category `"Sarah"`
because the underscore acts as a word boundary.

This is arguably correct for natural-language text (underscores are rare in
free-form items), but worth revisiting if matching behavior feels surprising.

**Fix**: Add `|| byte == b'_'` to `is_ascii_word_char` if regex-compatible
`\b` semantics are desired.
