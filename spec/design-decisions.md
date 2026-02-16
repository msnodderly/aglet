# Design Decisions

Outcomes of design discussions that clarify intent beyond what the spec
covers. These are reference material for implementers and reviewers.

---

## 1. Two levels of auto-assignment

**Date**: 2026-02-15
**Relevant tasks**: T015-T022 (Phase 3)

The engine evaluates two kinds of conditions, each serving a different
level of automation:

**Level 1 — ImplicitString (name-based, zero-config)**

Creating a category named "Sarah" automatically assigns all items containing
"Sarah." No user configuration needed. This is the entry-level "aha moment"
that makes the system feel intelligent.

Controlled by the `enable_implicit_string` bool on Category. Reserved
categories (When, Entry, Done) have this set to `false` so common words
don't trigger false matches.

**Level 2 — Profile (assignment-based, user-configured)**

A Profile condition matches when an item is already assigned to a specific
combination of categories. This lets users build rules like:

> "If an item is in both `Urgent` AND `Project Alpha`, also assign it to
> `Escalated`."

The input is the item's current assignment set, not its text. This creates
cascading behavior — one assignment can trigger another, which triggers
another. The fixed-point loop (T018) handles convergence.

**Example cascade:**

1. User types "Project Alpha deadline moved up" → `Project Alpha` matches
   by name (ImplicitString)
2. User manually marks it `Urgent`
3. Engine re-evaluates → item is in both `Urgent` AND `Project Alpha` →
   Profile on `Escalated` fires → auto-assigned to `Escalated`
4. `Escalated` has an Assign action targeting `Notify-Boss` → item also
   goes to `Notify-Boss`

**Why this matters:** The category hierarchy plus conditions and actions
forms a declarative rule engine. Users express organizational logic ("anything
that's both a Meeting and Overdue goes to Follow-Up") without writing code.
Profile conditions are what make the category tree a *program*, not just a
taxonomy.

**Implementation note:** Profile uses the `Query` struct but only the
`include` and `exclude` fields are evaluated. The other fields
(`virtual_include`, `virtual_exclude`, `text_search`) are for View filtering
in Phase 4. The engine must ignore them when evaluating Profile conditions.

---

## 2. `enable_implicit_string` is the source of truth

**Date**: 2026-02-15
**Relevant tasks**: T017 (engine condition evaluation)

There are two representations of implicit string matching in the model:
- `category.enable_implicit_string: bool` — the flag on the Category struct
- `Condition::ImplicitString` — an enum variant in the conditions vec

The bool flag is authoritative. The engine checks the flag, not the
conditions vec. The `Condition::ImplicitString` variant exists in the enum
definition but is never explicitly stored in a category's conditions list.
This avoids dual-representation bugs where the flag says one thing and the
conditions vec says another.

---

## 3. Assignments are sticky

**Date**: 2026-02-15
**Relevant tasks**: T017, T018

The engine never revokes existing assignments during re-evaluation. It only
adds new ones. If an item is already assigned to a category, the engine
skips it — no duplicate assignment, no re-firing of actions. Only the user
can remove assignments.

This is critical for:
- **Termination**: The fixed-point loop converges because the set of
  assignments is monotonically increasing.
- **User trust**: Auto-classification is additive. The system never silently
  un-organizes your items.

`Action::Remove` is the one exception — it can unassign — but remove results
are deferred until the cascade completes (T018), preventing mid-cascade
instability.

---

## 4. Remove actions apply regardless of assignment source

**Date**: 2026-02-15
**Relevant tasks**: T018

`Action::Remove` unassigns an item from a target category regardless of how
the assignment was created — Manual, AutoMatch, Action, or Subsumption.

This is intentional. Remove actions represent explicit workflow policy
configured by the user ("when an item is marked Done, remove it from active
project categories"). The user configured the rule knowing what it does; the
engine should honor it uniformly.

If Remove actions respected assignment source (e.g., "don't remove manual
assignments"), it would create confusing behavior: marking an item Done would
clean up auto-assigned categories but leave manually-assigned ones untouched,
even though the user set up a rule that says otherwise.

**Example**: Item is manually assigned to "Active Projects." Category "Done"
has a Remove action targeting "Active Projects." User marks item Done →
engine fires Remove → item is unassigned from "Active Projects" even though
it was manually assigned. This is correct — the user configured that
workflow.

---

## 5. Pass cap returns an error

**Date**: 2026-02-15
**Relevant tasks**: T018

If the fixed-point loop exceeds 10 passes, the engine returns an error
rather than silently stopping.

**Rationale**: A 10-pass cascade is a rule configuration bug (likely a cycle
or unbounded chain). Silently accepting partial results would hide the
problem — the user would see incomplete assignments with no indication that
anything went wrong. An error makes misconfigured rules fail loudly so the
user can fix them.

The error should be descriptive (e.g., "rule processing exceeded 10 passes
for item <id>; possible cycle"). If atomicity is feasible (DB transaction
around the processing run), the error should also roll back partial
assignments so the item isn't left in an inconsistent state.

**Atomicity guidance**: Attempt transaction wrapping if the Store API makes
it straightforward. If it requires substantial infrastructure changes, skip
it — document that partial writes may exist on cap-exceeded errors and
defer transactional execution to a later task.

---

## 6. Subsumption is bookkeeping, not classification

**Date**: 2026-02-15
**Relevant tasks**: T019

When assigning to a child category, subsumption creates implicit
assignments for all ancestors (`source: Subsumption`). These assignments
have two constraints:

1. **Subsumption does not fire ancestor actions.** If "Projects" has an
   Assign action targeting "Dashboard", and an item is subsumed into
   "Projects" via "Project Alpha", the "Dashboard" action does NOT fire.
   Only a direct condition match on "Projects" fires its actions.

2. **Subsumption does not count as a new assignment for the fixed-point
   loop.** Subsumption assignments are created during a pass but don't
   cause additional passes. They are bookkeeping to maintain the ancestor
   invariant, not classification events.

3. **Subsumption does not overwrite existing assignments.** If an item is
   already assigned to an ancestor (Manual, AutoMatch, etc.), the
   subsumption walk skips it. The original source and origin are preserved.

**Rationale for not firing actions**: Subsumption is structural — it means
"this item is in a child, so logically it's also in the parent." It does
not mean "this item matched the parent's conditions." Firing actions on
subsumption would create surprising cascades where adding a child category
triggers the parent's workflow rules, even though the parent's conditions
weren't evaluated.

**May revisit**: This is a reasonable MVP default, but real-world usage may
reveal cases where users expect ancestor actions to fire on subsumption.
For example, a "Work" category with an action to tag items for a dashboard
might expect all items in any child of "Work" to appear. If this becomes a
pain point, consider adding a per-category flag like
`fire_actions_on_subsumption: bool` (default false) so users can opt in.
For now, the simpler behavior avoids unexpected cascades.

---

## 7. Mutual exclusion is immediate, not deferred

**Date**: 2026-02-15
**Relevant tasks**: T020

When assigning to a child of an exclusive parent (`is_exclusive = true`),
sibling unassignment happens **immediately** — not deferred like
`Action::Remove`.

**Rationale**: Mutual exclusion is a structural invariant, not a workflow
action. The rule is: "an item can be in at most one child of this parent."
If we deferred the sibling unassignment, the item would be in two exclusive
siblings simultaneously during the cascade, violating the invariant. Other
conditions evaluated mid-cascade could see the inconsistent state and make
wrong decisions.

`Action::Remove` is deferred because it's a workflow side-effect that
shouldn't destabilize mid-cascade evaluation. Mutual exclusion is the
opposite — it *stabilizes* the state by enforcing a constraint.

**Stale subsumption**: When unassigning a sibling due to exclusion, any
subsumption assignments created for that sibling's ancestors may become
stale. For MVP, these are left in place — they're harmless bookkeeping.
Cleaning them up is deferred to hardening (Phase 11).

---

## 8. Retroactive assignment is unfiltered

**Date**: 2026-02-15
**Relevant tasks**: T021

When a category is created or modified, `evaluate_all_items` runs
`process_item` on **every item in the store** — not just items whose text
might match the category name.

This is intentionally unoptimized. Filtering items (e.g., text-searching for
the category name first) would miss:
- **Profile conditions**: A category with a Profile condition matches based on
  existing assignments, not text. No text filter would catch these.
- **Action cascades**: The new category's actions might assign to other
  categories, which have their own conditions and actions. The ripple effects
  are unpredictable from text alone.

The cost is O(items × categories) per `evaluate_all_items` call. This is
acceptable for MVP — personal information managers rarely exceed thousands of
items. If performance becomes a bottleneck, the optimization path is:
1. Text-index items for ImplicitString-only categories (no Profile, no actions).
2. Track "dirty" items that need re-evaluation.
3. Batch processing with change sets.

None of these are needed yet. Premature optimization here would add complexity
for a problem that doesn't exist at MVP scale.

---

## 9. Engine runs synchronously inline

**Date**: 2026-02-15
**Relevant tasks**: T022

The engine runs synchronously within the store operation that triggers it.
Creating an item blocks until classification completes. Creating a category
blocks until all items are retroactively evaluated.

There is no async queue, background worker, or eventual consistency. The
caller gets the fully classified result back from the same function call.

**Rationale**: Synchronous execution is simpler and guarantees consistency.
When `create_item` returns, the item is fully classified — the UI can
immediately show it in the right categories. An async model would require
the UI to poll or subscribe for classification completion, adding complexity
for negligible benefit at MVP scale.

**When to revisit**: If `evaluate_all_items` becomes noticeably slow (hundreds
of milliseconds) with large item counts, consider moving it to a background
task. But `process_item` (single item) should always remain synchronous — the
latency of classifying one item against the hierarchy is negligible.

---

## 10. Store mutation succeeds even if engine fails

**Date**: 2026-02-15
**Relevant tasks**: T022

When the integration layer creates an item and then runs the engine, a
classification failure (e.g., pass cap exceeded) does **not** roll back the
item creation. The item exists in the store; the engine error is propagated
to the caller separately.

**Rationale**: The user typed something and pressed enter. That data should
be saved regardless of whether the rule engine had trouble classifying it.
Losing user input because of a misconfigured rule would be a worse failure
than having an unclassified item.

The engine already has its own atomicity via SAVEPOINTs — if a `process_item`
run fails mid-cascade, the engine's partial assignments are rolled back. But
the store-level mutation (insert/update of the item or category itself) is
committed independently.

**Implication**: After an engine error, the item/category exists but may
have incomplete classification. The error should be surfaced to the user so
they know something went wrong with their rules, not with their data.

---

## 11. Manual assignment triggers the engine

**Date**: 2026-02-15
**Relevant tasks**: T022

When a user manually assigns an item to a category, the engine runs
`process_item` on that item afterward. This is necessary because the manual
assignment might satisfy a Profile condition on another category.

**Example**: User manually marks an item as "Urgent." Category "Escalated"
has a Profile condition: `include: {Urgent, Project Alpha}`. If the item is
already in "Project Alpha," the manual "Urgent" assignment completes the
Profile — the engine should fire and assign to "Escalated."

Without this, manual assignments would be "dead ends" that never trigger
cascading rules. The whole point of Profile conditions is that any assignment
— regardless of source — can trigger further classification.

---

## 12. WhenBuckets are date-only, computed at query time

**Date**: 2026-02-15
**Relevant tasks**: T023

WhenBucket resolution uses only the date portion of `when_date`, not the
time. An item scheduled for "today at 9am" is still in the Today bucket at
5pm — it doesn't become Overdue until tomorrow. This is intentional:
WhenBuckets represent calendar days, not moments.

Buckets are computed at query time, never stored. An item that was "Tomorrow"
yesterday is "Today" today and "Overdue" tomorrow. This means View results
are always fresh — no stale bucket assignments to clean up.

The resolution function takes a `NaiveDate` reference date rather than
reading the clock. The caller converts "now" to the user's local date before
calling. This keeps the function pure, deterministic, and testable.

**Bucket priority**: Today > Tomorrow > ThisWeek. If today is Monday,
tomorrow (Tuesday) is "Tomorrow", not "ThisWeek." ThisWeek only covers
the remainder of the week after tomorrow. There is no special-casing for
weekends — if today is Saturday, Sunday is Tomorrow.

---

## 13. Query criteria are ANDed, empty fields are permissive

**Date**: 2026-02-15
**Relevant tasks**: T024

All five Query fields (`include`, `exclude`, `virtual_include`,
`virtual_exclude`, `text_search`) are ANDed together. An item must satisfy
every non-empty criterion to match.

Empty fields are permissive — they match everything. An empty `include`
means "no category requirement." An empty `virtual_include` means "no
date bucket requirement." A `None` text_search means "no text filter."
This means `Query::default()` (all empty) matches every item.

This is the standard filter conjunction pattern. It composes well: a View
with `include: {Work}` and a Section with `virtual_include: {Today}`
effectively AND together to show "Work items due today."

**virtual_include is intersection, not union**: If `virtual_include`
contains `{Today, Tomorrow}`, an item must be in ALL listed buckets
simultaneously. Since an item can only be in one bucket, this matches
nothing. This is technically correct (intersection semantics) but
unlikely to be useful. If users need "Today OR Tomorrow," they should
use two sections. This may warrant revisiting if it becomes confusing,
but changing it to union semantics would break the consistency of "all
criteria are ANDed."

---

## 14. Query evaluator takes items as input, not a Store

**Date**: 2026-02-15
**Relevant tasks**: T024, T025

`evaluate_query` takes `&[Item]` rather than `&Store`. The caller fetches
items from the store and passes them in.

**Rationale**: This keeps the query evaluator as a pure function over data.
It's easier to test (construct items in memory, no database setup), easier
to compose (the View resolver can pass pre-filtered subsets), and has a
clearer contract (input → output, no side effects).

The downside is that the caller must load all items into memory. This is
acceptable for MVP scale. If the item set grows large enough to be a
problem, the optimization is to push filters into SQL queries — but that's
a different architecture (query planning) that doesn't belong in MVP.

---

## 15. View section membership is non-exclusive

**Date**: 2026-02-15
**Relevant tasks**: T025

An item can appear in multiple sections within the same View. If an item
matches the criteria of two sections, it appears in both. This follows
directly from the multifiling principle — one item, many homes.

The one exception is the unmatched bucket: an item appears in explicit
sections OR in unmatched, never both. If an item matches at least one
section, it is excluded from unmatched regardless of how many sections
it matches.

---

## 16. show_children only expands simple criteria

**Date**: 2026-02-15
**Relevant tasks**: T026

`show_children = true` on a Section only triggers expansion when the
section's criteria is a single category in `include` and everything else
is empty (no exclude, no virtual fields, no text_search).

**Rationale**: show_children means "break this category down by its
children." That only makes sense when the section IS a single category.
A section with `include: {A, B}` or `text_search: "meeting"` isn't
representing a single category — there's no meaningful way to "expand
its children."

Treating complex criteria as a normal flat section (ignoring show_children)
is the least surprising behavior. No error, no silent partial expansion.

**One level only**: Even if a child has its own children, subsections are
always leaf groups. Recursive expansion would produce deeply nested views
that are hard to navigate. If users want deeper structure, they can create
explicit sections for grandchildren.

---

## 17. Edit-through is assignment, not move

**Date**: 2026-02-15
**Relevant tasks**: T027

Edit-through operations are primitive assignment/unassignment operations,
not composite "move" operations. The library provides:

- `insert_in_section`: assigns categories
- `remove_from_section`: unassigns categories
- `remove_from_view`: unassigns categories

"Moving" an item between sections is a TUI-level composition: remove from
the old section, insert into the new one. The library doesn't need to know
about moves — it only knows about individual assignment changes.

**Why not a move primitive?** Because a "move" implies exclusivity — the
item leaves one section and enters another. But sections aren't exclusive
(§15). An item can be in multiple sections simultaneously. A "move" would
need to decide which sections to remove from, which depends on UI context
(which section the cursor is in) rather than data model rules. That logic
belongs in the TUI, not the core library.

---

## 18. Edit-through triggers the engine

**Date**: 2026-02-15
**Relevant tasks**: T027

Every edit-through operation runs `process_item` after applying its
assignment changes. This is necessary for the same reason as §11 (manual
assignment triggers the engine) — any assignment change might satisfy
a Profile condition or trigger an action cascade.

Insert is the obvious case: assigning to a section's categories might
complete a Profile. But remove also triggers the engine because, while
the engine won't auto-remove assignments (sticky, §3), the changed
assignment set is the new baseline for future evaluation. Running the
engine ensures any cascading from other concurrent changes is resolved.

---

## 19. View resolution is read-only, edit-through is write

**Date**: 2026-02-15
**Relevant tasks**: T025, T027

`resolve_view` is a pure read operation — it takes items and returns a
grouped result. It never modifies the store. Edit-through operations are
write operations — they modify assignments in the store and trigger the
engine.

These are deliberately separate code paths. The resolver answers "what
does the user see?" The edit-through functions answer "what happens when
the user acts?" Keeping them separate means the resolver can be called
freely without side effects (e.g., to refresh the display), and
edit-through operations are explicit, auditable mutations.

**Implication for the TUI**: The render loop calls `resolve_view`
repeatedly (on refresh, on view switch, on engine completion). User
actions call edit-through functions, which mutate the store, then the
TUI re-resolves to show the updated state. The TUI never caches
resolved results across mutations.
