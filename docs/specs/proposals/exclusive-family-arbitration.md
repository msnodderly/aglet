# Exclusive Family Arbitration

## Context

Aglet supports exclusive parent categories: an item may be assigned to at most
one child of that parent at a time.

Today, exclusive-family conflicts can arise from several rule sources:

- implicit string matching
- profile conditions
- on-assignment actions
- accepted suggestions
- manual user assignment

This is already useful and should remain useful across arbitrary databases and
schemas. The mechanism is not specific to a `Priority` family.

Real examples in local databases:

- `/Users/mds/src/aglet-features.ag`
  - `Priority` is exclusive with children `Critical / High / Normal / Low`
  - `TUI` currently has action `Assign [High]`
  - `High` has condition `TUI -> High`
  - `Low` has condition `TUI -> Low`
  - `Critical` has condition `Bug + TUI -> Critical`
- `/Users/mds/src/death-star.ag`
  - `Priority` is exclusive with `High / Medium / Low`
- `/Users/mds/src/moto.ag`
  - no active exclusive family today, but future schemas could easily use one
    for lifecycle state, machine status, budget disposition, or other domains

The design question is not "how should Priority work?" but:

> How should Aglet resolve multiple matching derived children under any
> exclusive parent?

## Problem

Current behavior is deterministic, but the rule is implicit and surprising.

When multiple derived children under the same exclusive parent match in one
processing run, the winner is effectively whichever matching child is assigned
last during traversal. Because the engine walks categories in hierarchy order,
this makes conflict resolution depend on child ordering and processing order
rather than on a surfaced product rule.

This causes three problems:

1. Users cannot easily predict which derived child will win.
2. The winning child may not match user intent.
3. The UI does not explain suppressed derived matches, so the resolution feels
   arbitrary.

## Current Behavior

### Engine behavior today

- The fixed-point engine walks categories in hierarchy order.
- When a matching child under an exclusive parent is assigned, sibling children
  are removed immediately.
- Later matching children in the same family can then replace the earlier one.
- Manual assignments and accepted suggestions act like locks: derived siblings
  do not replace them.
- The engine prevents infinite loops with a seen-pair set and a 10-pass cap.

### Consequence

For an exclusive family with child order:

`Critical, High, Normal, Low`

and derived rules:

- `TUI -> High`
- `TUI -> Low`
- `Bug + TUI -> Critical`

the current winner for an item containing only `TUI` is `Low`, because `Low`
matches after `High` and clears it.

For an item matching both `Bug` and `TUI`, the current winner is still `Low`,
because `Critical` matches first, then `High` can replace it, then `Low` can
replace `High`.

This is deterministic, but not obvious, and not currently surfaced as a first-
class rule.

## Goals

- Preserve the expressive power of overlapping derived rules under exclusive
  parents.
- Support arbitrary user-defined schemas without hardcoding domain semantics.
- Make conflict resolution understandable and inspectable.
- Preserve loop safety and bounded processing.

## Non-Goals

- Hardcoding special behavior for `Priority`, `Status`, or any named family
- Banning overlapping rules under exclusive parents entirely
- Introducing custom domain semantics like "`Critical` always beats `Low`"
  unless the user explicitly models that precedence

## Proposal

### 1. Allow overlapping derived matches under exclusive parents

Do not reject conflicting conditions or actions purely because they target
children of the same exclusive parent.

These overlaps are useful for real modeling patterns:

- defaults plus escalation
- coarse and fine-grained matching
- temporary routing categories
- status inference from multiple signals

Example:

- `TUI -> Low`
- `Bug + TUI -> Critical`

This is a valid and useful setup. A generic default may apply broadly, while a
more specific rule narrows to a stronger child when additional evidence exists.

### 2. Replace implicit last-writer-wins with explicit family arbitration

When more than one derived child of the same exclusive parent matches during a
processing run, Aglet should treat that as an arbitration step:

1. collect all matching derived candidates for the family
2. choose one winner using a documented family rule
3. assign the winner
4. suppress the losing derived candidates

This should happen as a family-level resolution concept, not as an incidental
effect of traversal order.

### 3. Default precedence rule: child order

The initial generic rule should be:

> Within an exclusive family, derived conflicts are resolved by child order.

Recommended interpretation:

- earlier child wins over later child

Why this default:

- schema-agnostic
- already user-controlled through tree ordering
- understandable and explainable
- works for arbitrary families without introducing special fields

This lets the user express precedence by arranging children in the desired
order.

Examples:

- `Critical, High, Normal, Low`
  - `Critical` wins over `High`, `Normal`, and `Low`
- `New, Triaged, In Progress, Done`
  - `New` wins if the user intentionally ordered the family that way
- `Bike A, Bike B, Shared`
  - the first child in the family wins if multiple rules match

If the default later proves too limiting, we can add explicit per-child
precedence values as a follow-on feature. Child order is sufficient for the
first implementation.

### 4. Keep manual and accepted suggestions as family locks

Current behavior here is good and should remain:

- `Manual` assignment under an exclusive parent blocks derived siblings
- `SuggestionAccepted` assignment under an exclusive parent blocks derived
  siblings

This gives users a durable way to override rule-derived family outcomes.

### 5. Surface the rule and the suppressed matches

The system should explain exclusive-family arbitration in user-facing surfaces.

Desired explanation style:

- `Priority: multiple derived matches; chose High by family order`
- `Suppressed: Low (matched TUI), Critical (matched Bug + TUI)`

At minimum:

- assignment provenance should explain the winning child
- inspect/debug surfaces should show that other family matches were suppressed
- category/rule authoring UI should warn when a new rule overlaps an existing
  rule in the same exclusive family

Important:

- this is a warning/explanation, not a validation error

## Why Not Ban Conflicting Rules?

Banning overlapping rules under exclusive families would make the system less
expressive and would reject reasonable user models.

Examples that should remain valid:

- default + escalation:
  - `TUI -> Low`
  - `Bug + TUI -> Critical`
- coarse + specific:
  - `Needs Review -> Backlog`
  - `Needs Review + Customer Impact -> Escalated`
- multi-signal classification:
  - `Road -> DRZ400`
  - `Track + School -> R1`

The real problem is not that overlaps exist; it is that resolution is currently
implicit.

## Example Walkthrough

Using `/Users/mds/src/aglet-features.ag` with family order:

`Critical, High, Normal, Low`

and rules:

- `TUI` action: `Assign [High]`
- `High` condition: `TUI -> High`
- `Low` condition: `TUI -> Low`
- `Critical` condition: `Bug + TUI -> Critical`

### Case A: item matches `TUI`

Candidates:

- `High` via action/condition
- `Low` via condition

Proposed result:

- winner: `High` if earlier child wins by family order
- suppressed: `Low`

### Case B: item matches `Bug + TUI`

Candidates:

- `Critical`
- `High`
- `Low`

Proposed result:

- winner: `Critical`
- suppressed: `High`, `Low`

### Case C: user manually assigns `Low`

Result:

- `Low` remains assigned
- derived `Critical` / `High` do not replace it until the user clears the
  manual assignment

## Loop Safety

This proposal does not require changing the fundamental loop-safety model.

Existing protections should remain:

- seen `(item, category)` pair tracking
- bounded fixed-point processing
- deferred remove handling

The change is in conflict arbitration, not in whether cascades are allowed.

## Migration / Compatibility

This is a behavior change from current traversal-order semantics.

However, the current rule is not surfaced as a deliberate product contract, so
changing to explicit family arbitration is reasonable if documented clearly.

To reduce surprise:

- document the new rule in AGENTS/docs/help
- surface arbitration explanations in inspect/provenance UI
- note that child order now defines precedence for derived conflicts under
  exclusive parents

## Open Questions

1. Should the default be:
   - earlier child wins
   - later child wins

Recommendation: earlier child wins, because users typically read ordered lists
top-to-bottom as highest precedence first.

2. Should action-derived candidates and condition-derived candidates be treated
equally inside family arbitration?

Recommendation: yes for the first version. Both are derived, non-manual family
inputs.

3. Should we add authoring-time warnings for overlapping family rules?

Recommendation: yes, but informational only.

4. Do we eventually need explicit numeric precedence instead of child order?

Recommendation: maybe later; defer until child-order arbitration proves
insufficient.

## Recommendation

Adopt explicit exclusive-family arbitration.

- Allow overlapping derived rules under exclusive parents.
- Resolve them using documented family precedence.
- Use child order as the initial generic precedence rule.
- Preserve manual and accepted-suggestion family locks.
- Surface the winning child and the suppressed matches in provenance/debug UX.

This keeps Aglet flexible enough for arbitrary schemas while making rule
resolution understandable instead of incidental.

## Implementation Notes (April 2026)

The current implementation now follows the top-down family rule for derived
assignment arbitration:

- earlier child wins over later child inside an exclusive family
- later derived siblings are suppressed instead of replacing the winner
- manual and accepted-suggestion assignments still act as durable family locks

Semantic review suggestions remain slightly more conservative than assignment
arbitration:

- if an item already has a child assigned under an exclusive parent, Aglet does
  not queue a semantic suggestion for a different sibling in that family

This keeps review suggestions from surfacing obviously conflicting alternatives
once a family already has an effective winner.
