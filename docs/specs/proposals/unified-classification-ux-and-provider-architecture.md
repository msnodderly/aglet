# Unified Classification UX And Provider Architecture

## Context

Aglet already has the beginnings of a classification engine, but the current UX
does not yet match the product direction or the flexibility of the underlying
model.

Today:

- item create/edit runs classification eagerly and inline
- category create/update/reparent can retroactively scan all items
- the main TUI affordance is a per-category `Auto-match` toggle
- there is no first-class suggestion review queue
- there is no explicit manual recalc workflow
- there is no LLM provider abstraction

At the same time, Aglet is intentionally not "just a task manager." It is a
general-purpose, schemaless personal database in the Lotus Agenda tradition.
The same database can feel like:

- a feature tracker
- a budget tracker
- a contact database
- a personal CRM
- a research notebook
- a mixed system that combines all of the above

That breadth changes the UX bar:

- classification should feel visible and useful, not magical or intrusive
- spreadsheet-like use cases must not be overwhelmed by chatty AI behavior
- recommendations should use enough context to feel smart
- the TUI must stay responsive even when hosted or local LLM providers are in play

## Problem Summary

The current implementation is internally coherent but externally surprising.

### What exists now

- Rule-based name matching is implemented via a simple word-boundary substring
  classifier.
- Matching is evaluated against `item.text` plus the full note body.
- Retroactive classification runs as a side effect of category mutations.
- Manual assignment re-runs the engine so profile-based rules can cascade.
- Assignment provenance is visible after the fact in the preview/info pane.

### What is missing

- No unified concept of "classification" that covers both rules and future LLMs.
- No distinction between "candidate generation" and "assignment application."
- No review queue for suggestion-first operation.
- No persistent record of rejected suggestions.
- No provider abstraction for OpenAI, Anthropic, Ollama, or LM Studio.
- No explicit UI for "recalculate this item/category/database now."
- No clear way to make intelligence visible without forcing it into every workflow.

## Goals

- Unify rule-based and LLM-based categorization behind one classification model.
- Support both `auto-apply` and `suggest/review` behavior for all automatic
  categorization providers.
- Support on-demand classification regardless of continuous policy.
- Treat fuzzy `When` extraction as part of the same classification and assignment system.
- Keep the TUI responsive; classification may be async where appropriate.
- Make classification more visible and discoverable without making every workflow
  feel AI-centric.
- Use richer context than just item title text when generating recommendations.
- Make general-purpose uses like budgeting, issue tracking, and contacts feel
  natural.
- Preserve trust through clear provenance, review, and scoped control.

## Non-goals

- Replacing manual category assignment.
- Making LLM classification mandatory.
- Requiring online APIs; local providers must be first-class.
- Turning Aglet into a chat-first interface.

## Design Principles

### 1. One classification pipeline, many providers

Rule-based and LLM-based categorization should feel like the same feature from
the user's perspective. They differ in provider, cost, speed, and confidence,
not in overall UX shape.

### 2. Separate detection from application

The engine should be able to say:

- "these are likely categories"
- "this item likely has this `When` value"
- "these should be auto-applied"
- "these need user review"

without conflating those steps.

### 3. Structural derivation is not the same as classification

Subsumption, exclusivity, and action/profile cascades are still important, but
they should happen after an assignment is accepted or applied. They are
consequences of classification, not competing classification providers.

### 4. Visible, but progressively disclosed

Classification should be easier to discover than it is now, but it should not
dominate databases that behave more like spreadsheets or compact registries.

### 5. Context-rich, not title-only

Recommendations should be able to consider:

- item title
- note body
- existing manual assignments
- current section/view context
- relevant numeric values
- `when` data
- category metadata and parentage

### 6. Trust beats cleverness

Users should always be able to answer:

- why was this assigned?
- which provider suggested it?
- how confident was it?
- can I stop this from happening again?

## Current State Recap

The current architecture is a good base, but it is optimized for deterministic,
inline MVP behavior:

- single-item classification is synchronous and immediate
- category retroactive scans are synchronous and database-wide
- the current matcher returns `Some(1.0)` or `None`, so there is no meaningful
  low/medium/high confidence routing yet
- `AssignmentSource` has no concept of suggestion acceptance or LLM-originated
  review decisions
- the TUI has manual assignment UI, but that UI is autocomplete for
  manual category entry, not review of system-generated classification candidates

Aglet should keep eager interpretation as the default for ordinary item work:

- saving a changed item should eagerly classify it by default
- manually assigning a category should eagerly reprocess the item by default
- changing `When` should eagerly reprocess the item by default

What needs to change is not the default eagerness of normal item workflows, but
the handling of bulk and expensive provider-backed work.

This proposal intentionally reuses the existing strengths:

- provenance
- category hierarchy
- structural cascades
- item/category manager workflows
- store-backed settings

while changing the shape of automatic categorization around them.

It also intentionally revisits the current product decision that classification
is always synchronous and inline for all triggering operations. That decision is
still a good default for cheap structural and single-item deterministic work,
but it is too restrictive for bulk recalc and future hosted/local LLM providers.

## Proposed Model

## 1. Classification becomes a first-class database feature

Add a database-level classification configuration that is visible in the TUI and
persisted in `app_settings`.

Proposed high-level settings:

- `classification.enabled`
- `classification.continuous_mode`
  - `off`
  - `auto_apply`
  - `suggest_review`
- `classification.run_on_item_save`
- `classification.run_on_category_change`
- `classification.default_bulk_mode`
  - `background`
  - `foreground`
- `classification.pending_indicator`
  - `off`
  - `count_only`
  - `count_and_status`

This separates:

- whether classification exists
- whether it runs continuously
- whether results apply automatically or queue for review
- whether bulk operations block the caller

Recommended defaults:

- `classification.enabled = true`
- `classification.continuous_mode = auto_apply`
- `classification.run_on_item_save = true`
- `classification.run_on_category_change = true`

## 2. Provider model

Introduce a provider abstraction:

```rust
trait ClassificationProvider {
    fn id(&self) -> &str;
    fn classify(&self, request: ClassificationRequest) -> Result<Vec<ClassificationCandidate>>;
    fn supports_sync(&self) -> bool;
    fn supports_async(&self) -> bool;
}
```

Initial provider families:

- `implicit_string`
- `hashtag`
- `when_parser`
- `openai`
- `anthropic`
- `ollama`
- `lmstudio`

### Recommendation

- deterministic local providers can run inline for the current item if they are fast
- all LLM providers should be safe to run async
- bulk recalc should default to async even for rule-based providers once item
  counts are non-trivial

## 3. Candidate generation, not immediate assignment

Providers return `ClassificationCandidate` records, not assignments:

```rust
struct ClassificationCandidate {
    item_id: ItemId,
    assignment: CandidateAssignment,
    provider: String,
    model: Option<String>,
    confidence: Option<f32>,
    rationale: Option<String>,
    context_hash: String,
}

enum CandidateAssignment {
    Category(CategoryId),
    When(NaiveDateTime),
}
```

Then policy decides:

- ignore
- auto-apply
- queue for review

This is the key unlock for unifying rule-based and LLM-based behavior.

## 4. Suggestion-first persistence

Add a `classification_suggestions` table (or equivalent persisted store) for:

- pending suggestions
- accepted suggestions
- rejected suggestions
- stale suggestions tied to an old item revision/config hash

This enables:

- a real review queue
- not re-suggesting rejected matches forever
- background classification without losing work
- stable UX when jobs finish after the user moves elsewhere in the TUI

## 5. Accepted suggestions become assignments, then cascades run

When a suggestion is accepted or auto-applied:

1. write the assignment with provider-aware provenance
2. run structural/cascade logic synchronously
3. refresh affected views

That means:

- profile conditions still work
- actions still work
- subsumption still works
- exclusive parent handling still works

but they are downstream of accepted classification, not mixed into provider selection.

## 6. Classification context envelope

Providers should receive richer context than a bare title string.

Proposed request shape:

```rust
struct ClassificationRequest {
    item_id: ItemId,
    text: String,
    note: Option<String>,
    when_date: Option<NaiveDateTime>,
    manual_category_ids: Vec<CategoryId>,
    visible_view_name: Option<String>,
    visible_section_title: Option<String>,
    numeric_values: Vec<(CategoryId, Decimal)>,
    candidate_categories: Vec<CategoryDescriptor>,
}
```

### Important constraint

The provider should not always see the entire category universe.

Candidate categories should be narrowed by:

- active view/section context when available
- category hierarchy and existing category semantics
- current assignments and current `When`

This avoids wasting tokens and keeps results coherent in mixed-purpose databases.

## UX Proposal

## 1. Surface classification more visibly, but not everywhere all the time

The answer to "should we surface classification more visibly?" is yes, with
progressive disclosure.

### Recommendation

Make classification visible at four levels:

- database level: whether it is on, off, auto-apply, or suggest/review
- item level: whether this item has pending suggestions or recent auto-applies
- category level: the existing rule-based `Auto-match` toggle and related provenance
- review level: a dedicated place to accept/reject suggestions

Do not force classification UI into every pane by default.

## 2. Rename the current `Auto-match` concept

The existing `Auto-match` checkbox should remain, but it should be understood as
the per-category control for deterministic rule-based implicit matching, not as
the master switch for the whole classification system.

Recommendation:

- keep the checkbox
- optionally relabel it to `Rule Auto-match` or `Implicit Match`
- keep provider selection and suggestion policy at the database level

This preserves a useful local control without overloading the category UI.

## 3. Add a Classification Center

Introduce a dedicated TUI surface for classification status and review.

For the first implementation, the "Classification Center" and the "Review
Queue" may share one screen, as long as that screen clearly separates
database-level settings/status from suggestion triage. We do not need two
totally separate modal systems on day one.

Potential entry points:

- `C` opens classification status/settings and pending review
- `=` runs recalc for current item
- `?` remains help in the existing TUI and should not be repurposed for review
- if we later split the surfaces, `C` can remain the center entry and another
  key can open review directly

The exact keys can be refined later, but the workflow needs dedicated affordances.

### Classification Center responsibilities

- show continuous mode
- show pending suggestion count
- show recent completed jobs
- provide recalc actions
- provide access to global classification mode settings
- provide a direct path into pending review

### Recommended initial screen shape

For the first slice, prefer one screen with three bands:

- summary band: mode, provider status, pending count
- settings band: global classification mode
- review band: pending suggestions grouped by item

This makes classification discoverable without forcing users through nested
dialogs.

For built-in providers:

- implicit category matching remains a category-level concern via `Auto-match`
- natural-language `When` parsing stays always-on while continuous
  classification is enabled
- inline vs background execution should be decided by implementation cost, not
  by a user-facing toggle

## 4. Review queue UX

In `suggest/review` mode, users need a fast accept/reject loop.

The queue should be **item-grouped**, not flat suggestion-grouped. Users should
normally select an item first, then review that item's pending suggestions in a
detail pane. This avoids repeating the same item context for each suggestion.

### Review item fields

- item text
- note excerpt
- suggested category or `When`
- provider and model
- confidence or confidence band
- short rationale
- current assignments
- accept / reject actions

### Recommended phase-1 layout

- left pane: items with pending suggestions plus a count badge
- right pane top: selected item context (`text`, note excerpt, current assignments)
- right pane bottom: the selected item's pending suggestions

Suggested actions:

- `Enter`: accept selected suggestion
- `r`: reject selected suggestion
- `A`: accept all suggestions for selected item
- `R`: reject all suggestions for selected item
- `Tab`: move between item list and suggestion list
- `Esc`: close

Navigation is a sufficient "skip" mechanism in phase 1; no persisted skip state
is required yet.

### Mockup: review queue

```text
┌ Classification Review ──────────────────────────────────────────────────────┐
│ Mode: Suggest/Review     Pending: 12     Providers: Rules, Ollama:mistral │
│ Scope: This Database                                                  [C] │
├─────────────────────────────────────────────────────────────────────────────┤
│ > "Reimburse Sam for conference hotel"                                    │
│   Note: paid on card ending 1142, include parking                         │
│   Suggest: Travel                                                         │
│   Provider: Rules                                                         │
│   Why: matched note text + prior accepted payee pattern                   │
│   Current: Expense, Work                                                  │
│                                                                            │
│   "Reimburse Sam for conference hotel"                                    │
│   Suggest: Reimbursement                                                  │
│   Provider: Ollama / mistral-small                                        │
│   Confidence: medium                                                      │
│   Why: reimbursement language in title and note                           │
│   Current: Expense, Work                                                  │
│                                                                            │
│   "Lunch with Sam next Tuesday at noon"                                   │
│   Suggest: When = 2026-03-24 12:00                                        │
│   Provider: When Parser                                                   │
│   Why: parsed "next Tuesday at noon"                                      │
│   Current: Work                                                           │
├─────────────────────────────────────────────────────────────────────────────┤
│ Enter accept   r reject   A accept-all-item   R reject-all-item   Esc close │
└─────────────────────────────────────────────────────────────────────────────┘
```

## 5. Item-level visibility

Items with pending suggestions should expose that state without demanding action.

Examples:

- small `?` marker in board/list row
- preview line: `Suggestions: 2 pending`
- status after save: `Saved. 2 classification suggestions pending. Open review queue to inspect.`

### Mockup: normal board with subtle signal

```text
Ready                         In Progress
──────────────────────────    ──────────────────────────
? Reimburse Sam hotel         Draft release notes
  Add Claude local provider   Fix batch assign bug
  Grocery run

Status: Classification on (Suggest/Review). 2 pending suggestions.
Footer: a assign  e edit  p preview  = recalc  C classify
```

This keeps the feature visible but does not force modal interruption.

## 6. Category Manager visibility

Classification should be configurable from the Category Manager because that is
where category semantics already live.

### Proposed details block

```text
┌ Category Details ───────────────────────────────┐
│ Name: Travel                                    │
│ Type: Tag                                       │
│ Exclusive: [ ]                                  │
│ Actionable: [ ]                                 │
│ Rule Auto-match: [x]                            │
│ Recent derived assignments: 14                  │
│                                                │
│ Note: ...                                       │
└──────────────────────────────────────────────────┘
```

### Recommendation

Keep the current category-local rule toggle in the category manager, but keep
provider selection and suggestion policy database-wide. That avoids introducing
new per-category provider toggles we have not committed to implement.

## 7. Recalc workflows

Recalc needs to be explicit and easy.

Start simple:

- expose a manual "re-run classification" action
- do not commit this proposal to a detailed scope-picker UX that is not yet implemented
- keep current-item work eager and let expensive bulk work move to the background

## Example Use Cases

## 1. Feature tracker workflow

### Scenario

The database is used like a lightweight local issue tracker.

Categories:

- `Issue Type -> Bug / Feature request / Idea`
- `Priority -> High / Normal / Low`
- `Project -> Aglet / NeoNV`
- `Status -> Ready / In Progress / Complete`

### Desired behavior

- Rules catch obvious exact-name matches like `Aglet`.
- LLM can suggest `Feature request` from description text even when title is vague.
- Suggestions appear after save, not while typing.
- Accepting `Ready` or `Feature request` still triggers downstream profile/action logic.

### Why this feels good

It supports low-friction capture while still keeping workflow categories auditable.

## 2. Budget tracker workflow

### Scenario

The database behaves more like a spreadsheet or personal ledger.

Categories:

- `Account -> Checking / Cash / Visa`
- `Expense Type -> Fuel / Groceries / Lodging / Maintenance`
- numeric categories like `Amount`

### Desired behavior

- Numeric categories are never LLM targets by default.
- Existing section/view context narrows suggestions. If the user enters an item
  in a `March Budget` or `Visa` context, that is part of the classification envelope.
- The UI stays calm: a subtle pending indicator is fine; modal prompts are not.

### Why this feels good

The system remains natural for structured entry and analysis instead of turning
every expense row into an "AI workflow."

## 3. Contact database workflow

### Scenario

The database is used to track people, orgs, relationships, and follow-ups.

Categories:

- `People`
- `Company`
- `Relationship -> Friend / Vendor / Colleague`
- `Follow Up`

### Desired behavior

- Rules catch direct proper-name matches when categories exist.
- LLM can infer likely relationship tags from note text.
- fuzzy `When` extraction can infer follow-up timing from free text
- Review mode is especially valuable because contact categorization is nuanced.

### Why this feels good

This is exactly the kind of "general-purpose personal database" workload where
semantic recommendations can feel like a multiplier instead of a gimmick.

## 4. Retroactive category creation workflow

### Scenario

The user creates a new category `Reimbursement` after months of entering expense items.

### Desired behavior

- The system does not surprise-freeze the UI with a full blocking scan by default.
- It preserves eager defaults for ordinary item work, while expensive retroactive
  work can move to the background automatically when needed.
- In `auto_apply` mode, accepted high-confidence rule matches can apply automatically.
- In `suggest/review` mode, category results land in the review queue, while
  `When` parser results still apply inline.

This is a much better fit for both large databases and future LLM-backed scans.

## Provider Strategy

## 1. Hosted providers

Supported examples:

- OpenAI fast classification models
- Anthropic fast classification models

Requirements:

- API key configuration
- timeouts
- retry policy
- concurrency limits
- clear model and provider provenance in suggestions

## 2. Local providers

Supported examples:

- Ollama
- LM Studio

Requirements:

- local endpoint configuration
- model selection
- health checks
- timeouts
- background execution
- non-blocking UX

### Important product point

Local provider support is not a nice-to-have. It should be treated as a first-
class path, not an afterthought behind hosted APIs.

## 3. Rule-based providers

Rule-based classification should also move into the provider model.

That allows:

- unified provenance
- unified review UX
- shared recalc infrastructure
- the same auto-apply vs suggest/review policy surface
- one conceptual home for category inference and fuzzy `When` inference

## Provenance Model

Add richer provenance for assignments and suggestions.

### Suggestions

- provider
- model
- confidence
- rationale
- generated_at
- item_revision

### Assignments created from suggestions

Options:

1. add new assignment sources such as `SuggestionAccepted` and `AutoClassified`
2. keep `Manual` / `AutoMatch` but store richer structured origins

### Recommendation

Prefer richer assignment-source modeling over origin-string overloading.

Suggested additions:

- `SuggestionAccepted`
- `AutoClassified`

with origin metadata including provider/model. This will age better once both
rules and LLM providers are active.

## Sync vs Async Strategy

## 1. What should stay synchronous

- explicit manual assignment
- structural invariant enforcement
- cascade/application after an accepted assignment
- very cheap current-item rule scans when continuous classification is enabled
- date and `When` updates once a suggestion has been accepted

## 2. What should support async

- all LLM-backed classification
- bulk recalc
- retroactive scans after category changes
- background suggestion refresh

## 3. UX rules for async work

- never block keystrokes on provider calls
- show job progress in the TUI status area or Classification Center
- make pending results durable
- allow refresh/review after job completion
- cancel or supersede stale jobs when item text changes

## Implementation Outline

## Phase 1: Unify model without LLM calls

- Introduce classification settings and provider abstraction.
- Move implicit-string matching into provider form.
- Introduce suggestion persistence.
- Add review queue UI.
- Add recalc actions.
- Rename and expand category-level classification controls.

## Phase 2: Async job infrastructure

- Add background job runner for classification tasks.
- Add durable job/suggestion state.
- Add foreground vs background choice for bulk recalc.

## Phase 3: Local provider support

- Add Ollama provider.
- Add LM Studio provider.
- Add provider settings UI and health checks.

## Phase 4: Hosted provider support

- Add OpenAI and Anthropic providers.
- Add secure API-key configuration and model selection.

## Phase 5: Smarter context and ranking

- add view/section context weighting
- add category-family scoping helpers
- improve duplicate/rejection memory
- refine confidence bands and rationale formatting

## Risks

### 1. Overexposure in structured databases

If classification is too prominent, budget-tracker and ledger workflows will
feel noisy.

Mitigation:

- progressive disclosure
- subtle item-level indicators
- database-level `off` and `suggest/review` modes

### 2. Slow or flaky provider experience

Hosted APIs and local models can both be slow.

Mitigation:

- async by default for expensive providers
- durable queue
- provider timeouts
- cancel stale requests

### 3. Mixed mental models

Users may confuse manual assignment autocomplete with system-generated classification suggestions.

Mitigation:

- separate language in the UI
- separate panels
- separate icons/badges
- separate footer hints

### 4. Provenance debt

If richer suggestion state is added without better provenance types, the model
will get messy quickly.

Mitigation:

- add explicit suggestion/auto-classification provenance now

## Recommendation Summary

Recommend the following product direction:

- unify all automatic categorization behind one classification system
- treat fuzzy `When` assignment as part of that same system
- support `off`, `auto_apply`, and `suggest_review` continuous behavior
- always support explicit on-demand recalc
- treat rules and LLMs as providers in the same pipeline
- keep structural cascades synchronous after acceptance/application
- make expensive and bulk classification async
- add a dedicated Classification Center and review queue
- make classification more visible in the UI, but via progressive disclosure
- use richer item context and narrowed candidate scopes so recommendations feel
  intelligent across tasks, budgets, contacts, and mixed personal databases
- keep eager interpretation as the default for normal item workflows

## Open Questions

- Should rule-based current-item classification remain inline by default when
  continuous mode is on, or should all classification unify behind async job
  completion for consistency?
- `When` parser results should not appear as category-style review suggestions;
  keep them inline even when category suggestions use the review queue.
- Should accepted suggestions be undoable as one batch action from the review queue?
- Should LLM providers ever be allowed to auto-apply, or should they remain
  suggestion-first even when the global mode is `auto_apply`?

## Suggested Next Step

If this direction is accepted, the next document should be a narrower
implementation plan covering:

- model/schema additions
- provider trait and request/response contracts
- TUI mode/state changes
- persistence for suggestions/jobs/settings
- rollout sequencing and migration strategy
- roadmap hooks for broader Catalyst-style application triggers after the core
  classification system is in place
