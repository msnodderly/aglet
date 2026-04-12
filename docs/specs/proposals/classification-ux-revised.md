---
title: Classification UX Revised
status: draft
created: 2026-03-21
---

# Classification UX — Revised Proposal

## Context

This proposal revises `unified-classification-ux-and-provider-architecture.md`
based on a close reading of the original Lotus Agenda reference manual, the
Kaplan/Kapor CACM paper, and the Fallows description. It also accounts for the
fact that our codebase already has a **fully implemented and tested engine** for
profile conditions, actions, and cascading — the missing piece is TUI surface
area.

### Key findings from the Lotus Agenda reference

1. **Classification is category-centric.** All rules (text conditions, profile
   conditions, actions) are properties of individual categories, configured
   through Category Properties. There is no separate "Classification Center."

2. **The `?` indicator + Utilities > Questions flow.** Pending unconfirmed
   assignments are signaled by a `?` in the status bar. Review is accessed
   through a utility menu, not a dedicated top-level mode or keybinding.

3. **Four condition/classification types:**
   - **String conditions** — match item text against category name/aliases.
     Deterministic, instant, zero-cost. This is what `Condition::ImplicitString`
     and the "Auto-match" toggle already do.
   - **Profile conditions** — "assign if item is assigned to X AND Y."
     Deterministic, instant, zero-cost. Already implemented in the engine as
     `Condition::Profile { criteria: Box<Query> }` with AND/OR/NOT semantics.
   - **Date conditions** — "assign if item date falls in range." Deterministic,
     instant. Not yet implemented, but the `Condition` enum can be extended with
     `DateRange { start, end }`. Deferred to a future phase.
   - **Semantic conditions (LLM)** — model infers category relevance from item
     text, note, and context even when no keywords match. Non-deterministic,
     expensive, variable latency. This is the province of future
     `OllamaProvider`, `AnthropicProvider`, etc. The `ClassificationProvider`
     trait already supports this; implementations are stubs for now.

   The first three types are Lotus Agenda's original model. The fourth is our
   extension for modern AI-backed classification.

   | Type | Deterministic | Cost | Async needed | Engine status |
   |------|--------------|------|-------------|---------------|
   | String | Yes | Trivial | No | Implemented |
   | Profile | Yes | Trivial | No | Implemented |
   | Date | Yes | Trivial | No | Future |
   | Semantic (LLM) | No | Expensive | Eventually (sync MVP first) | Stub |

   **LLM/Semantic sync-first strategy:** The first LLM provider implementation
   will be synchronous — blocking on the API call during item save or manual
   recalc, same as the rule-based providers today. This keeps the UX and code
   simple for the MVP: one request/response cycle, result appears immediately,
   no job queue or background polling needed. Async execution (background jobs,
   durable suggestion queue, progress indicators) is a follow-on phase for when
   bulk recalc or slow local models make blocking unacceptable. The provider
   trait already has `is_cheap()` to distinguish inline-safe providers from
   expensive ones — async will key off that flag when the infrastructure is
   ready.

4. **Conditions vs. Actions — a critical distinction:**

   Conditions and actions are both attached to categories, but they work in
   opposite directions and produce fundamentally different assignment types.

   **Conditions** answer: "should this item be in this category?"

   A condition is evaluated every time an item changes. If the condition is
   met, the item is *conditionally* assigned. If the condition later stops
   being true (the item text changes, or a prerequisite assignment is removed),
   the conditional assignment **automatically breaks** — the item silently
   leaves the category. Conditional assignments are temporary and reactive.

   Examples:
   - Category "Phone Calls" has a string condition matching "call" in item
     text. Item "Call Mom about birthday" is conditionally assigned. If the
     user edits the item to "Visit Mom about birthday," the assignment to
     "Phone Calls" automatically breaks.
   - Category "Urgent Work" has a profile condition: assigned to "Work" AND
     assigned to "High Priority." An item in both categories is conditionally
     assigned to "Urgent Work." If the user removes the "High Priority"
     assignment, the "Urgent Work" assignment automatically breaks.

   **Actions** answer: "what else should happen when this category is assigned?"

   An action fires once, at the moment an item is assigned to the category
   that owns the action. The resulting assignment is *explicit* — it persists
   permanently, even if the triggering assignment is later removed. Actions
   are one-shot consequences, not ongoing relationships.

   Examples:
   - Category "Done" has an action: `Assign { targets: [Archive] }`. When an
     item is marked Done, it is also explicitly assigned to "Archive." If the
     user later un-marks Done, the "Archive" assignment remains — the item
     stays archived unless manually removed.
   - Category "Escalated" has an action: `Remove { targets: [Low Priority] }`.
     When an item is assigned to "Escalated," it is explicitly removed from
     "Low Priority." The removal is permanent even if "Escalated" is later
     removed.

   This distinction maps to our existing `AssignmentSource` model:
   - Conditional assignments use `AssignmentSource::AutoMatch` (they can break)
   - Action assignments use `AssignmentSource::Action` (they persist)
   - Manual assignments use `AssignmentSource::Manual` (they persist)

   The cascade loop handles interactions between conditions and actions:
   assigning an item to a category via a condition may fire that category's
   actions, which may satisfy conditions on other categories, and so on up to
   10 passes until a fixed point is reached.

5. **Two independent knobs** (not relevant to our design — we intentionally
   keep `ContinuousMode` as a single setting for now):
   - Initiative = match sensitivity threshold
   - Authority = silent-apply vs. queue-for-review

### What we already have

The engine layer is complete:

- `Condition::ImplicitString` — text matching against category name
- `Condition::Profile { criteria: Box<Query> }` — AND/OR/NOT criteria on
  current assignments
- `Action::Assign { targets }` — assign to other categories on assignment
- `Action::Remove { targets }` — remove from categories on assignment
  (deferred until cascade completes)
- Fixed-point cascade loop (max 10 passes)
- Full persistence (`conditions_json`, `actions_json` on categories table)
- `ClassificationProvider` trait with `ImplicitStringProvider` and
  `WhenParserProvider`
- `ClassificationSuggestion` lifecycle (Pending/Accepted/Rejected/Superseded)
- `ClassificationConfig` with `ContinuousMode` (Off/AutoApply/SuggestReview)
- Comprehensive test coverage for all of the above

**Not yet implemented:**

- **Date conditions** — `Condition` enum needs a `DateRange` variant. Deferred.
- **Semantic/LLM providers** — the `ClassificationProvider` trait supports them,
  but no concrete `OllamaProvider` or `AnthropicProvider` exists yet. First
  implementation will be synchronous (blocking on API call, result appears
  immediately). Async execution is a follow-on phase.

**Implementation notes added in April 2026:**

- Semantic review suggestions are filtered against the item's **effective**
  current assignments, not only manual assignments. The prompt sees the full
  currently assigned category set, and queueing skips suggestions that are
  already satisfied, conflict with an assigned sibling under an exclusive
  parent, or would not produce a stable post-reprocess change.
- The TUI assignment/unassign flows now treat "can this change stick?" as a
  previewed question. If removing a category would immediately be re-applied by
  rules, the picker/inspect flow keeps the assignment in place and explains
  why instead of pretending the unassign succeeded.

### What is missing (TUI surface area)

- **No TUI to create/edit conditions or actions** — cannot visually define
  "if assigned to X AND Y, assign to Z"
- **No TUI to review pending suggestions** that integrates with the Category
  Manager (current `Mode::ClassificationReview` is a standalone top-level mode)
- **No `?` indicator** for pending suggestions in normal board view
- **No condition/action visibility** in the Category Manager details pane
  (conditions/actions exist but are invisible to the user)

## Design Principles

1. Classification is a behavior of categories, not a separate system.
2. Conditions and actions are category properties, edited in the Category Manager.
3. Two review paths: item-centric (board view) and category-centric (Category
   Manager). Same suggestion data, different entry points.
4. The `?` indicator in the board view is passive notification, not modal interruption.
5. Progressive disclosure: basic users see auto-match toggles; power users
   define profile conditions and actions.
6. Keep `ContinuousMode` as the single global policy knob (intentional design choice).
7. Reflect shipped engine semantics accurately: condition-derived assignments
   (implicit string and profile) are live and can auto-break when their
   triggering condition stops matching, while manual, action-produced, and
   accepted-suggestion assignments remain sticky. Conditions and actions differ
   both in trigger direction and in assignment lifecycle.

## Two Review Paths

Classification review has two natural orientations, corresponding to two
places in the TUI where the user is already working:

### Path 1: Item-centric review (board view)

The user is browsing items and notices a `?` indicator on one. They want to
know: "What does the system think this item should be categorized as?"

This is triage-oriented — the user works through items one at a time,
accepting or rejecting suggested categories for each.

**When this is natural:**
- Working through recently added items
- Reviewing imports after a bulk CSV ingest
- Checking suggestions after a recalc

**Entry point:** From Normal mode, navigate to an item with `?`, press a
review key to see that item's pending suggestions inline.

### Path 2: Category-centric review (Category Manager)

The user is looking at a category and wants to know: "Which items does the
system think belong in this category?"

This is curation-oriented — the user is focused on a specific category and
reviewing which items should be pulled in.

**When this is natural:**
- Just created a new category ("Reimbursement") and want to see retroactive matches
- Modified a category's conditions and want to see what changed
- Reviewing a specific category's auto-match results

**Entry point:** From Category Manager, select a category, press `R` to see
pending suggestions *for that category*.

### Both paths are filters on the same data

The underlying `classification_suggestions` table stores suggestions keyed by
`(item_id, category_id)`. Path 1 filters by item. Path 2 filters by category.
The accept/reject actions are identical — they update the same suggestion
record and trigger the same assignment logic.

## Proposed Changes

### 1. Remove `Mode::ClassificationReview` as a top-level mode

**Current:** `C` in Normal mode opens a standalone classification review screen.

**Proposed:** Remove the `C` keybinding and the standalone mode. Replace with
two context-appropriate review surfaces (see §3 and §4 below). The `c` key
continues to open the Category Manager as before.

### 2. Add `?` indicator to board view

When pending suggestions exist, show a `?` marker on affected items and a count
in the footer status line. This mirrors the original Lotus Agenda behavior.

```text
┌─ Ready ─────────────────────────┬─ In Progress ──────────────────────┐
│                                 │                                    │
│ ? Reimburse Sam for hotel       │   Draft release notes              │
│   Add Claude local provider     │   Fix batch assign bug             │
│ ? Call Mom about birthday       │                                    │
│   Grocery run                   │                                    │
│                                 │                                    │
├─────────────────────────────────┴────────────────────────────────────┤
│ ? 2 items have pending suggestions                                   │
│ n:add  e:edit  a:assign  c:categories  ?:help                        │
└──────────────────────────────────────────────────────────────────────┘
```

The `?` prefix on items is subtle — it doesn't change the item text or require
any action. Users notice it when they scan and can address it when ready.

### 3. Item-centric review (from board view)

When the user focuses an item with `?` and presses a review key (e.g., `Enter`
on the `?` item, or a dedicated key like `R`), the preview/info pane shows
that item's pending suggestions with accept/reject actions:

```text
┌─ Ready ─────────────────────────┬─ Suggestions ──────────────────────┐
│                                 │                                    │
│ ? Reimburse Sam for hotel  ←    │ Reimburse Sam for hotel            │
│   Add Claude local provider     │ Note: paid on card ending 1142     │
│ ? Call Mom about birthday       │                                    │
│   Grocery run                   │ 1. Suggest: Travel                 │
│                                 │    Provider: Rules (implicit match)│
├─ In Progress ───────────────────│    Why: matched "hotel" in text    │
│                                 │                                    │
│   Draft release notes           │ 2. Suggest: Reimbursement          │
│   Fix batch assign bug          │    Provider: LLM (ollama/mistral)  │
│                                 │    Why: reimbursement language      │
│                                 │                                    │
│                                 │ Current: Expense, Work             │
├─────────────────────────────────┴────────────────────────────────────┤
│ ? 2 items have pending suggestions                                   │
│ Enter:accept  x:reject  A:accept-all  j/k:navigate  Esc:back        │
└──────────────────────────────────────────────────────────────────────┘
```

This uses the existing preview pane area — no new mode needed. The user stays
in the board context, sees their items, and resolves suggestions without
leaving the view. After accepting/rejecting, the `?` clears and the user
moves to the next item naturally.

### 4. Category-centric review (from Category Manager)

When the user selects a category in the Category Manager and presses `R`,
the details pane switches to show pending suggestions *for that category* —
items the system wants to assign here. This is the curation path: "I'm
looking at Travel — which items should be in it?"

If pressed without a specific category selected (or from the global level),
it shows all pending suggestions across all categories.

```text
┌─ Category Manager ──────────────────────────────────────────────────┐
│ Classification: Auto-Apply | Ready Queue: Ready | Claim: In Progress│
├─ Filter ────────────────────────┬─ Details ──────────────────────────┤
│ Press / to filter               │ Name: Travel                      │
├─ Categories ────────────────────┤                                    │
│ ├── Budget                      │ Flags                              │
│ │   ├── Fuel                    │  [x] Auto-match                    │
│ │   ├── Groceries               │  [ ] Exclusive                     │
│ │   └── Lodging                 │  [ ] Actionable                    │
│ ├── People                      │                                    │
│ │   └── Fred Smith              │ Conditions (1)                     │
│ ├── Priority                    │  IF assigned to "Work"             │
│ │   ├── High                    │                                    │
│ │   └── Normal                  │ Actions (0)                        │
│ ├── Status [E]                  │  (none)                            │
│ │   ├── Ready [RQ]              │                                    │
│ │   ├── In Progress [CT]        │ Note                               │
│ │   └── Complete                │  Expenses related to travel...     │
│ └── Travel ←                    │                                    │
├─────────────────────────────────┴────────────────────────────────────┤
│ n:new  r:rename  x:del  R:review(2)  Tab:pane  /:filter  ?:help     │
└──────────────────────────────────────────────────────────────────────┘
```

When the user presses `R` with a category selected, the details pane shows
suggestions scoped to that category — items the system wants to assign here:

```text
┌─ Category Manager ──────────────────────────────────────────────────┐
│ Classification: Suggest/Review | Ready Queue: Ready | Claim: In Prog│
├─ Filter ────────────────────────┬─ Review: Travel (2 suggestions) ──┤
│ Press / to filter               │                                    │
├─ Categories ────────────────────┤ > Reimburse Sam for hotel          │
│ ├── Budget                      │   Provider: Rules (implicit match) │
│ │   ├── Fuel                    │   Why: matched "hotel" in text     │
│ │   ├── Groceries               │   Current: Expense, Work           │
│ │   └── Lodging                 │                                    │
│ ├── People                      │   Book train to Portland           │
│ │   └── Fred Smith              │   Provider: LLM (ollama/mistral)   │
│ ├── Priority                    │   Why: travel-related booking       │
│ │   ├── High                    │   Current: (none)                  │
│ │   └── Normal                  │                                    │
│ ├── Status [E]                  │                                    │
│ │   ├── Ready [RQ]              │                                    │
│ │   ├── In Progress [CT]        │                                    │
│ │   └── Complete                │                                    │
│ └── Travel ←                    │                                    │
├─────────────────────────────────┴────────────────────────────────────┤
│ Enter:accept  x:reject  A:accept-all  Esc:back-to-details  ?:help    │
└──────────────────────────────────────────────────────────────────────┘
```

If `R` is pressed without a specific category (or from the top level), it
shows all pending suggestions across all categories, grouped by category:

```text
┌─ Review: All Categories (4 suggestions) ────────────────────────────┤
│                                                                      │
│ ── Travel (2) ──────────────────────────────────────                 │
│ > Reimburse Sam for hotel          Rules (implicit)                  │
│   Book train to Portland           LLM (ollama)                     │
│                                                                      │
│ ── High Priority (1) ──────────────────────────────                  │
│   Call Mom about birthday          Rules (profile)                   │
│                                                                      │
│ ── Reimbursement (1) ──────────────────────────────                  │
│   Reimburse Sam for hotel          LLM (ollama)                     │
│                                                                      │
```

Key behaviors:
- `R` toggles into/out of review sub-view
- Review is scoped to selected category, or all categories if none selected
- `Enter` accepts the focused suggestion (applies the assignment, cascades run)
- `x` rejects the focused suggestion (persisted, won't re-suggest)
- `A` accepts all pending suggestions for the current scope
- `Esc` returns to normal details view
- If no pending suggestions, `R` shows "No pending suggestions" in status
- The `R:review(N)` hint in the footer shows the count; disappears when N=0

### 5. Condition editor in Category Manager details

Add a **Conditions** section to the details pane that shows existing conditions
and allows creating new ones. This is the primary missing UX for the existing
engine.

#### 5a. Conditions display

When a category has conditions, show them inline in the details pane:

```text
┌─ Details ──────────────────────────────────────┐
│ Name: High Priority                            │
│                                                │
│ Flags                                          │
│  [x] Auto-match                                │
│  [x] Exclusive                                 │
│  [ ] Actionable                                │
│                                                │
│ Conditions (2)                                 │
│  IF text matches "urgent" OR "asap"            │
│  IF assigned to "Work" AND "Overdue"           │
│                                                │
│ Actions (1)                                    │
│  THEN assign to "Follow Up"                    │
│                                                │
│ Note                                           │
│  Items that need immediate attention           │
└────────────────────────────────────────────────┘
```

#### 5b. Condition creation — profile conditions

When the user navigates to the Conditions section and presses `n` (new
condition), a popup appears for building a profile condition:

```text
┌─ New Condition ──────────────────────────────────┐
│                                                  │
│ Type: [Profile]  (Tab to switch)                 │
│                                                  │
│ Assign to "High Priority" IF item is:            │
│                                                  │
│  AND  [Work         ] ←                          │
│  AND  [Overdue      ]                            │
│  NOT  [              ]                            │
│  OR   [              ]                            │
│                                                  │
│ Each row is a category name. Type to autocomplete│
│ j/k:navigate  Tab:mode(AND/NOT/OR)  Enter:save   │
│ d:delete-row  n:add-row  Esc:cancel              │
└──────────────────────────────────────────────────┘
```

The popup uses the same category-name autocomplete that already exists in the
view criteria editor. Each row has a mode selector (AND/NOT/OR) and a category
name field.

#### 5c. Condition creation — text conditions (aliases / "Also match")

For text conditions beyond the basic auto-match-by-name, the user can add
explicit match patterns:

```text
┌─ New Condition ──────────────────────────────────┐
│                                                  │
│ Type: [Text]  (Tab to switch)                    │
│                                                  │
│ Also match text:                                 │
│  [urgent; asap; critical                      ]  │
│                                                  │
│ Separate terms with ; (OR) or , (AND)            │
│ Use * for wildcard, ! for negation               │
│                                                  │
│ Enter:save  Esc:cancel                           │
└──────────────────────────────────────────────────┘
```

This maps to setting additional match patterns on a category's string
condition, similar to Lotus Agenda's "Also match" field.

#### 5d. Action creation

When the user navigates to the Actions section and presses `n`:

```text
┌─ New Action ─────────────────────────────────────┐
│                                                  │
│ Type: [Assign]  (Tab: Assign/Remove)             │
│                                                  │
│ When item is assigned to "Done", ALSO assign to: │
│                                                  │
│  [Archive       ] ←                              │
│  [               ]                               │
│                                                  │
│ Each row is a target category.                   │
│ j/k:navigate  n:add-row  d:delete-row            │
│ Enter:save  Esc:cancel                           │
└──────────────────────────────────────────────────┘
```

Actions fire when an item is assigned to the category that owns the action.
`Action::Assign` adds the item to target categories. `Action::Remove` removes
the item from target categories (deferred until cascade completes).

### 6. Condition/action badges in category tree

Show compact indicators in the tree for categories that have conditions or
actions defined:

```text
├── Budget
│   ├── Fuel
│   ├── Groceries
│   └── Lodging
├── Priority [E]
│   ├── High [C2] [A1]          ← 2 conditions, 1 action
│   └── Normal
├── Status [E]
│   ├── Ready [RQ]
│   ├── In Progress [CT]
│   └── Complete [A1]           ← 1 action (assign to Archive)
└── Travel
```

Badge legend:
- `[C1]` / `[C2]` — condition count
- `[A1]` / `[A2]` — action count
- Existing badges unchanged: `[E]` exclusive, `[RQ]` ready-queue, `[CT]` claim-target

### 7. Normal mode footer with suggestion indicator

Update the Normal mode footer to show a `?` when suggestions are pending:

```text
Current footer (no suggestions):
  n:add  e:edit  a:assign  c:categories  v:views  ?:help

With pending suggestions:
  n:add  e:edit  a:assign  c:categories(? 3)  v:views  ?:help
```

The `(? 3)` badge on the `c:categories` hint tells the user that opening the
Category Manager will reveal 3 pending suggestions they can review with `R`.

### 8. Recalc from Normal mode

Keep `=` as the recalc key for the current item (already in the original
proposal). This re-runs classification for the focused item and produces
either auto-applied assignments or new suggestions depending on
`ContinuousMode`.

## What changes vs. the original proposal

| Aspect | Original | Revised |
|--------|----------|---------|
| Review entry point | `C` top-level mode | `R` inside Category Manager |
| Review location | Standalone split-pane screen | Details pane sub-view |
| `C` keybinding | Classification Review | Removed (freed) |
| `c` keybinding | Category Manager | Category Manager (unchanged) |
| Condition editing | Not addressed | New popup in details pane |
| Action editing | Not addressed | New popup in details pane |
| Profile conditions | Mentioned as future | First-class UX (engine exists) |
| Semantic/LLM | Async-first | Sync MVP first, async follow-on |
| Condition types | 1 (string) | 4 (string, profile, date, semantic) |
| `?` indicator | Mentioned but not specified | Concrete placement and format |
| Classification Center | Proposed as separate screen | Eliminated — not needed |

## What stays the same

- `ClassificationProvider` trait and provider pipeline
- `ClassificationSuggestion` lifecycle and persistence
- `ClassificationConfig` with `ContinuousMode`
- `CandidateAssignment` (Category / When)
- `AssignmentSource` variants (AutoClassified, SuggestionAccepted)
- Item revision hashing
- Engine cascade logic (conditions, actions, subsumption, exclusivity) —
  live condition-derived assignments auto-break; sticky action/manual/accepted
  assignments persist
- CLI structured capture (`--when`, `--category`, `--value`, import)
- CLI classify/review/accept/reject subcommands
- All Category Manager UX improvements (hints, badges, connectors, etc.)
- All TUI polish (persistent view, undo/redo, adaptive footer, error types)

## Implementation Sequence

### Phase A: Two review paths + remove standalone mode

1. **Category-centric review (Category Manager):**
   - Add `CategoryManagerFocus::Review` variant
   - Wire `R` key to toggle review sub-view in details pane
   - Scope suggestions to selected category (or all if none selected)
   - Move accept/reject logic from `handle_classification_review_key`
2. **Item-centric review (board view):**
   - Wire review key on `?` items to show suggestions in preview pane
   - Accept/reject inline without leaving board context
3. Remove `Mode::ClassificationReview` and `C` keybinding
4. Add `(? N)` badge to Normal mode footer hint for `c:categories`

### Phase B: Add `?` indicator to board view

1. Check pending suggestion count per item during refresh
2. Render `?` prefix on items with pending suggestions
3. Show count in footer status line

### Phase C: Condition/action display in details pane

1. Add "Conditions (N)" and "Actions (N)" sections to details rendering
2. Format conditions as human-readable rules ("IF assigned to X AND Y")
3. Format actions as human-readable rules ("THEN assign to Z")
4. Add `CategoryManagerDetailsFocus::Conditions` and `::Actions` variants

### Phase D: Condition editor popup

1. Create profile condition builder popup (AND/OR/NOT rows with category
   autocomplete)
2. Create text condition editor popup (alias/match patterns)
3. Wire `n` key on Conditions section to open popup
4. Wire `d` key to delete selected condition
5. Persist changes via `store.update_category()`

### Phase E: Action editor popup

1. Create action editor popup (Assign/Remove type selector + target category
   rows)
2. Wire `n` key on Actions section to open popup
3. Wire `d` key to delete selected action
4. Persist changes via `store.update_category()`

### Phase F: "Also match" text conditions

1. Extend `Condition::ImplicitString` or add `Condition::TextPattern` for
   explicit match patterns beyond category name
2. Add text condition popup for alias/wildcard entry
3. Evaluate text patterns alongside category name in classification

### Phase G: Semantic/LLM classification (sync MVP)

First LLM provider as a synchronous, blocking implementation:

1. Implement a concrete `ClassificationProvider` for one local provider
   (e.g., Ollama) — blocking HTTP call during item save or manual recalc
2. Provider returns `ClassificationCandidate` records through the same
   pipeline as rule-based providers
3. Results flow through the existing `ContinuousMode` policy: auto-applied
   or queued as suggestions depending on setting
4. Provider configuration UI in Category Manager Global Settings (endpoint
   URL, model name, enable/disable)
5. `is_cheap()` returns `false` — used to gate future async behavior but
   not enforced in this phase

This phase intentionally avoids background jobs, progress indicators, and
durable job queues. The user presses `=` (recalc) or saves an item, the LLM
call blocks briefly, and the result appears. Simple and testable.

### Phase H: Async LLM execution

When sync blocking becomes unacceptable (bulk recalc, slow local models,
hosted API latency):

1. Background job runner for classification tasks
2. Durable job/suggestion state
3. Progress indicator in footer status line
4. `is_cheap()` flag gates whether a provider runs inline or is dispatched
   to the background
5. Stale job cancellation when item text changes

### Phase I: Date conditions

1. Add `Condition::DateRange { start, end }` variant to `Condition` enum
2. Evaluate against `item.when_date` during engine passes
3. Add date condition popup in Category Manager (date range picker)
4. Useful for calendar views, deadline escalation, time-sensitive rules

## Open Questions

1. **Date conditions:** The original Lotus Agenda supports "assign if item date
   falls in range." We don't have this yet. Should it be a Phase F addition or
   deferred? The engine's `Condition` enum can be extended with
   `DateRange { start, end }`.

2. **Per-category provider overrides:** Lotus allows per-category override of
   global settings. We intentionally keep `ContinuousMode` global for now. If
   users need per-category "always auto-apply" vs "always suggest," that's a
   future extension.

3. **Bulk suggestion review:** Should `A` (accept all) in review mode accept
   all suggestions for the *focused item* or *all items globally*? Recommend:
   focused item only, with a separate `Shift-A` for global accept.

4. **Suggestion display in item preview:** Should the item preview pane (when
   viewing an item in Normal mode) also show pending suggestions? Probably yes,
   but this is an additive detail.
