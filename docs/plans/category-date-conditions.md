# Category-Level Date Conditions Replacement Plan

Status: In Progress
Tracking issue: `436f17e4-35aa-4cbe-9e35-2cf17cd7cff3`

## Summary

Replace the existing bucket-only `when_date` proposal with a full
destination-centric date-condition system for categories. A category may
automatically pull items in or drop them out based on `When`, `Entry`, or
`Done` dates, and membership must update both when item dates change and when
the local clock advances.

This plan preserves current aglet patterns:

- date conditions live in the existing category `Conditions` surface
- date conditions are dynamic, non-sticky `AutoMatch` assignments
- view/query `WhenBucket` support remains separate and is not the long-term
  storage model for category date conditions

## Public Types And Behavior

- Add a new condition variant:
  - `Condition::Date { source, matcher }`
- Add new supporting types:
  - `DateSource = When | Entry | Done`
  - `DateMatcher = Compare { op, value } | Range { from, through }`
  - `DateCompareOp = On | Before | After | AtOrBefore | AtOrAfter`
  - `DateValueExpr = Today | Tomorrow | DaysFromToday(i32) | DaysAgo(i32) |
    AbsoluteDate(Date) | AbsoluteDateTime(DateTime) | TimeToday(Time)`
- Add provenance support:
  - `AssignmentExplanation::DateCondition { owner_category_name,
    condition_index, rendered_rule }`
- Matching semantics:
  - category conditions remain ORed at the category level
  - a single date condition passes only when its configured compare/range check
    passes
  - date-condition assignments are `AssignmentSource::AutoMatch` and
    `sticky = false`
  - exclusive-family precedence, live subsumption, and actions behave the same
    as other condition-derived assignments
- Date-source semantics:
  - `When` evaluates `item.when_date`
  - `Done` evaluates `item.done_date`
  - `Entry` evaluates `item.created_at` converted into local civil time using
    the current timezone
  - missing source date means no match
- Bound semantics:
  - `On today` means `>= start_of_today` and `< start_of_tomorrow`
  - `Before today` means `< start_of_today`
  - `After today` means `>= start_of_tomorrow`
  - `At or before 12:00pm today` compares against a concrete local datetime
    boundary
  - `From X through Y` is inclusive on both ends
- Scope for v1:
  - must support the six concrete examples discussed
  - must not require Lotus-compatible free-form condition syntax in v1

## Implementation Changes

- Engine/runtime:
  - introduce an explicit evaluation context containing local `now`, local
    `today`, and timezone
  - thread it through item save, manual reprocess, category-change reevaluation,
    preview paths, undo/replay reprocessing, and any evaluate-all path
  - extend category matching to evaluate `Condition::Date` alongside existing
    `Profile`
  - do not reuse `Query.virtual_include` or `Query.virtual_exclude` as the
    date-condition storage/evaluation shape
- Time-driven reevaluation:
  - add a temporal reevaluation entrypoint that reevaluates all items against
    date conditions using the current local evaluation context
  - CLI runs temporal reevaluation before commands that read or mutate
    category/view/item state when any date condition exists
  - TUI runs temporal reevaluation on startup and when auto-refresh notices
    that the local minute changed
  - correctness wins over optimization in v1; global reevaluation is acceptable
- Persistence:
  - persist date conditions in existing category `conditions_json`
  - no SQL schema migration is required
  - preserve backward compatibility with existing `Condition` serialization
- CLI:
  - add `category add-date-condition <name> --source <when|entry|done>`
  - support exactly one of:
    - `--on <expr>`
    - `--before <expr>`
    - `--after <expr>`
    - `--at-or-before <expr>`
    - `--at-or-after <expr>`
    - `--from <expr> --through <expr>`
  - accepted expressions:
    - `today`
    - `tomorrow`
    - `<N> days from today`
    - `<N> days ago`
    - `YYYY-MM-DD`
    - `YYYY-MM-DD HH:MM`
    - `<time> today`
  - `category show` and `remove-condition` must render mixed `[Profile]` and
    `[Date]` conditions clearly
- TUI:
  - extend the existing `Conditions` pane and overlay instead of adding a new
    panel
  - conditions list shows mixed rule types with type labels
  - `a` adds a profile condition, `d` adds a date condition, `Enter` edits,
    `x` deletes
  - date-condition editor is structured, not free-form

## Category Panel UI Mockup

### Main Category Panel

```text
┌ Categories ───────────────────────┐┌ Category Details: Overdue ───────────────────────────────┐
│ Work                             ││ Flags                                                     │
│ Status                           ││   [ ] Exclusive                                           │
│   Ready                          ││   [x] Auto-match                                          │
│ > Overdue                        ││   [ ] Semantic Match                                      │
│   In Progress                    ││   [x] Match category name                                 │
│   Complete                       ││   [x] Actionable                                          │
│ When                             ││                                                           │
│ Entry                            ││ Also Match                                                │
│ Done                             ││   late                                                    │
│                                  ││                                                           │
│ Filter: over                     ││ > Conditions (Enter to edit)                              │
│                                  ││   1. [Date]   When before today -> Overdue               │
│                                  ││   2. [Profile] Ready + Work -> Overdue                   │
│                                  ││                                                           │
│                                  ││ Actions (Enter to edit)                                  │
│                                  ││   (none)                                                  │
│                                  ││                                                           │
│                                  ││ Note                                                      │
│                                  ││   Smart bucket for tasks whose due date has passed.       │
└──────────────────────────────────┘└───────────────────────────────────────────────────────────┘
Status: j/k focus field  Enter/Space: toggle/edit
```

### Conditions Overlay

```text
┌ Conditions: Overdue ───────────────────────────────────────────────────────────────┐
│ Items matching ANY rule get assigned:                                             │
│                                                                                    │
│ > 1. [Date]    When before today                                                  │
│   2. [Profile] Ready + Work                                                       │
│                                                                                    │
│   a:add profile   d:add date   Enter:edit   x:delete   Esc:close                  │
└────────────────────────────────────────────────────────────────────────────────────┘
```

### Date Condition Editor

```text
┌ New Date Condition: Overdue ───────────────────────────────────────────────────────┐
│ Rule preview: When before today -> Overdue                                        │
│                                                                                    │
│ > Source:      When                     (When / Entry / Done)                      │
│   Match:       Before                   (On / Before / After / AtOrBefore /        │
│                                          AtOrAfter / Range)                        │
│   Value:       today                    (today / tomorrow / 7 days from today /    │
│                                          2 days ago / 1990-11-12 / 12:00pm today)  │
│                                                                                    │
│   Include time: no                                                                 │
│                                                                                    │
│   Enter:save   Esc:cancel   Tab:j/k move   ←/→ cycle                              │
└────────────────────────────────────────────────────────────────────────────────────┘
```

### Range Variant

```text
┌ Edit Date Condition: Conference Tasks ─────────────────────────────────────────────┐
│ Rule preview: When from 1990-11-12 through 1990-11-15 -> Conference Tasks         │
│                                                                                    │
│   Source:      When                                                                │
│ > Match:       Range                                                               │
│   From:        1990-11-12                                                          │
│   Through:     1990-11-15                                                          │
│   Include time: no                                                                 │
│                                                                                    │
│   Enter:save   Esc:cancel                                                          │
└────────────────────────────────────────────────────────────────────────────────────┘
```

### Time-Of-Day Variant

```text
┌ New Date Condition: Morning Rush ──────────────────────────────────────────────────┐
│ Rule preview: When at or before 12:00pm today -> Morning Rush                     │
│                                                                                    │
│   Source:      When                                                                │
│   Match:       AtOrBefore                                                          │
│ > Value:       12:00pm today                                                       │
│   Include time: yes                                                                │
│                                                                                    │
│   Enter:save   Esc:cancel                                                          │
└────────────────────────────────────────────────────────────────────────────────────┘
```

## Test Plan

- Engine/unit coverage:
  - `When before today` pulls overdue items in and auto-breaks when `when_date`
    moves forward
  - rolling 7-day window behaves correctly across day changes
  - `Entry on today` matches newly created items and drops them after midnight
  - `Done within last 2 days` pulls in newly completed items and drops them when
    stale
  - absolute date range matches conference-window items
  - time-of-day condition drops items after the local cutoff
  - missing source date does not match
  - `Entry` uses local timezone conversion from timestamp
  - mixed `Profile` and `Date` conditions on one category are ORed
  - removal summaries and provenance text are correct
- Runtime coverage:
  - manual edits to `when_date` re-evaluate date-conditioned categories
    immediately
  - marking done and unmarking done re-evaluate done-based categories
    immediately
  - preview/save parity holds for date-conditioned categories
  - CLI temporal reevaluation updates assignments before read flows
  - TUI startup and auto-refresh minute rollover trigger temporal reevaluation
- Persistence/UI coverage:
  - category JSON round-trips `Condition::Date`
  - CLI `category show` renders profile/date conditions distinctly
  - TUI conditions list and editor handle mixed rule sets
  - TUI navigation remains consistent with current category-manager focus
    behavior

## Execution Handoff

- Create a new worktree from the current repo using:
  - branch: `codex/category-date-conditions`
  - sibling worktree path: `/Users/mds/src/aglet-category-date-conditions`
- Implementation order:
  1. add model/types/serialization and explanation text
  2. add engine evaluation context and date-condition matching
  3. add temporal reevaluation entrypoints for core, CLI, and TUI
  4. add CLI authoring/rendering
  5. add TUI conditions list/editor support
  6. add tests for the six examples and rollover behavior
- Keep AGENTS/CLAUDE notes updated if implementation reveals new surprising
  temporal or timezone behavior

## Assumptions And Defaults

- This plan fully replaces the previous bucket-only date-condition plan.
- Lotus-style behavior is the target capability, but Lotus free-form condition
  text entry is not required in v1.
- Global temporal reevaluation is acceptable in this greenfield repo; no
  optimized scheduler is required for the first implementation.
- The current view/query `WhenBucket` system remains unchanged and is not reused
  as the stored category-rule representation.
