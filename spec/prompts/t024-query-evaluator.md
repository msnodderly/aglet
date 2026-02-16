# Task: T024 — Query Evaluator

## Context

A `Query` is the filter that determines which items appear in a View or
Section. It has five criteria fields, all ANDed together:

- `include`: item must be assigned to ALL of these categories
- `exclude`: item must NOT be assigned to ANY of these categories
- `virtual_include`: item's WhenBucket must match ALL of these
- `virtual_exclude`: item's WhenBucket must NOT match ANY of these
- `text_search`: case-insensitive substring match on item text + note

An empty Query (all fields empty/None) matches everything.

The Query evaluator is the foundation for Views — every View and every
Section within a View uses a Query to select its items. T025 (View resolver)
will call this function repeatedly.

## What to read

1. `spec/mvp-spec.md` §2.6 — Query struct definition and semantics.
2. `crates/agenda-core/src/model.rs` — `Query`, `Item`, `WhenBucket`,
   `Assignment`, `CategoryId`.
3. `crates/agenda-core/src/query.rs` — `resolve_when_bucket` (T023). Your
   code goes in the same file.
4. `spec/design-decisions.md` §1 — Profile conditions use Query but only
   `include`/`exclude`. The engine ignores other fields. The query evaluator
   uses ALL fields.

## What to build

**File**: `crates/agenda-core/src/query.rs` (extend T023's code)

### The evaluator function

A public function that takes a `Query`, a slice of `Item`s, and a reference
date (`NaiveDate`), and returns the items that match all criteria.

**Criteria evaluation — all ANDed:**

1. **include** (intersection): For each category ID in `query.include`, the
   item must have that category in its `assignments` map. If `include` is
   empty, this criterion passes for all items.

2. **exclude** (disjunction): For each category ID in `query.exclude`, the
   item must NOT have that category in its `assignments` map. If `exclude`
   is empty, this criterion passes for all items.

3. **virtual_include** (intersection): Resolve the item's `when_date` to a
   `WhenBucket` using `resolve_when_bucket`. The bucket must be in
   `query.virtual_include`. If `virtual_include` is empty, this criterion
   passes for all items.

4. **virtual_exclude** (disjunction): The item's resolved WhenBucket must
   NOT be in `query.virtual_exclude`. If `virtual_exclude` is empty, this
   criterion passes for all items.

5. **text_search**: Case-insensitive substring match against the item's
   `text` field. If the item has a `note`, also search the note. Match in
   either field passes. If `text_search` is `None`, this criterion passes
   for all items.

**Return type**: Return a `Vec<&Item>` or `Vec<Item>` — whichever fits the
ownership model better. The caller (T025) will need to group these items
into sections, so consider what's most ergonomic.

**Reference date**: The caller provides the reference date (today in the
user's timezone). Don't call `Utc::now()`. This keeps the function
deterministic and testable.

## Tests to write

1. **Empty query matches everything**: Empty Query, several items → all
   returned.

2. **Include single category**: Query includes category A. Items assigned
   to A returned, others not.

3. **Include multiple categories (AND)**: Query includes A and B. Only items
   assigned to BOTH A and B returned.

4. **Exclude single category**: Query excludes category A. Items assigned
   to A filtered out, others returned.

5. **Exclude multiple categories**: Query excludes A and B. Items assigned
   to A OR B filtered out.

6. **Include + exclude combined**: Query includes A, excludes B. Item
   assigned to both A and B is filtered out (exclude wins).

7. **virtual_include filters by WhenBucket**: Query virtual_include =
   {Today}. Only items with when_date resolving to Today returned.

8. **virtual_exclude filters by WhenBucket**: Query virtual_exclude =
   {NoDate}. Items with no when_date filtered out.

9. **text_search matches item text**: Query text_search = "meeting". Items
   containing "meeting" (case-insensitive) returned.

10. **text_search matches item note**: Item with note containing the search
    term returned, even if text doesn't match.

11. **text_search is case-insensitive**: Search for "URGENT" matches item
    with text "urgent task".

12. **All criteria combined**: Query with include, exclude, virtual_include,
    and text_search. Verify all are ANDed.

13. **Empty include means no category filter**: Query with empty include but
    non-empty text_search. All items matching text returned regardless of
    assignments.

## What NOT to do

- **Don't implement View resolution** — that's T025. This function evaluates
  a single Query against items, not a full View with sections.
- **Don't modify `resolve_when_bucket`** — use it as-is.
- **Don't access the database** — the caller provides the items slice.
- **Don't sort results** — return items in the order they appear in the input.

## Workflow

Follow `AGENTS.md`. Issue ID: `bd-pc9`.

```bash
git checkout -b task/t024-query-evaluator
# Extend crates/agenda-core/src/query.rs
# cargo test -p agenda-core && cargo clippy -p agenda-core
```

## Definition of done

- [ ] Public function evaluates a Query against a slice of Items
- [ ] All five criteria fields evaluated and ANDed
- [ ] Empty fields are permissive (match everything)
- [ ] Reference date passed as parameter (no `Utc::now()`)
- [ ] All tests pass — `cargo test -p agenda-core`
- [ ] `cargo clippy -p agenda-core` clean
- [ ] Changes limited to `crates/agenda-core/src/query.rs`
