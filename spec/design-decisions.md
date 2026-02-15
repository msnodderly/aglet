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
