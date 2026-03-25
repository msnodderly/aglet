# Unified Classification UX And Provider Architecture

Revision note: this document supersedes earlier discussion with a March 24,
2026 snapshot of what is already implemented in `agenda-core` and `agenda-tui`.
It is intentionally grounded in the current code, not the older imagined UX.

## Context

Aglet already has a meaningful classification system:

- deterministic implicit-string category matching
- deterministic `When` parsing
- persisted classification config in `app_settings`
- persisted suggestion records in `classification_suggestions`
- distinct assignment provenance for auto-applied vs accepted suggestions
- a pending-review overlay opened with `C`
- inline suggestion review inside the edit-item panel

That means the next step is not "invent classification from scratch." The next
step is to extend the existing system so it can support LLM-backed category
classification cleanly, starting with a local-first Ollama integration.

The project is still greenfield, so schema and UX shape can change when needed.
Backward compatibility is less important than getting the classification model
and product direction right.

## What Exists Today

### Core model and persistence

The current classification model already includes:

- `ClassificationConfig`
  - `enabled`
  - `continuous_mode`: `Off | AutoApply | SuggestReview`
  - `run_on_item_save`
  - `run_on_category_change`
  - `enabled_providers: Vec<ProviderConfig>`
- `ClassificationCandidate`
- `ClassificationSuggestion`
- `SuggestionStatus`
  - `Pending`
  - `Accepted`
  - `Rejected`
  - `Superseded`
- `ClassificationProvider`
- persisted suggestion rows in `classification_suggestions`
- persisted classification config under `classification.config.v1`

Accepted suggestions and auto-applied classifications already use distinct
assignment sources:

- `AssignmentSource::AutoClassified`
- `AssignmentSource::SuggestionAccepted`

So the provenance baseline is better than older proposal drafts assumed.

### Providers that actually run today

Two providers exist and are wired:

- `implicit_string`
- `when_parser`

Current behavior:

- implicit-string matching runs against `item.text` plus full note body
- `When` parsing runs against `item.text`
- both providers return `ClassificationCandidate`
- candidates are deduplicated by `(provider, assignment)`

### Continuous behavior today

On item create/edit, if classification is enabled and `run_on_item_save=true`:

- candidates are collected
- stale pending suggestions for older item revisions are superseded
- `AutoApply`
  - category candidates are persisted as accepted and applied immediately
  - `When` candidates are persisted as accepted and applied immediately
- `SuggestReview`
  - category candidates are persisted as pending suggestions
  - `When` candidates are still persisted as accepted and applied immediately

This last point matters: current `Suggest/Review` is already category-review
mode, not full "review every kind of classification" mode.

### TUI behavior today

The current user-facing surfaces are:

- Global Settings (`g s` / `F10`) for classification mode
- board/item rows with a `?` pending-suggestion indicator
- preview/info pane showing pending suggestion counts
- edit-item panel with inline pending suggestions and three-state decisions
  - `Pending -> Accept -> Reject -> Pending`
- suggestion review overlay opened with `C`
  - left pane: items with pending suggestions
  - right pane: selected item context and suggestions
  - `Space` toggles, `Enter` confirms current item, `s` skips, `A` accepts all

That means the review panel user mentioned is already real and tested. We
should extend it, not replace it with a different first-pass concept.

## Current Gaps And Mismatches

The existing implementation is a strong base, but several important pieces are
still missing or only partly wired.

### 1. Provider config exists, but runtime selection does not

`ClassificationConfig.enabled_providers` and `ProviderMode` exist in the data
model, but the runtime service currently instantiates providers directly and
does not honor provider enablement or per-provider execution mode.

Implication:

- provider architecture is partially modeled, not fully implemented

### 2. Candidate category scoping is still rule-centric

`ClassificationService::build_request()` currently includes only categories
where `enable_implicit_string=true`, excluding numeric and reserved categories.

That is correct for implicit string matching, but it is too restrictive for LLM
classification. An LLM provider should not require "rule auto-match" to be on
before a category can even be considered semantically.

### 3. View/section context is modeled but not populated

`ClassificationRequest` already has:

- `visible_view_name`
- `visible_section_title`
- `manual_category_ids`
- `numeric_values`

But the current request builder always leaves visible view/section context as
`None`. The envelope is ahead of the actual wiring.

### 4. Category-change reprocessing is still auto-apply-only

Retroactive category-change processing currently flows through the older engine
path and only supports implicit-string reprocessing in `AutoApply`.

It does not yet:

- queue category suggestions in `SuggestReview`
- route through the same provider abstraction as item-save classification
- support expensive providers safely

### 5. Review UI does not yet expose full provider metadata

The current review overlay and edit-panel suggestion rows show rationale, but
they do not yet prominently show:

- provider
- model
- confidence

That is acceptable for deterministic rules, but it will not be sufficient once
LLM providers are active.

### 6. Manual recalc is still underspecified

The proposal has talked about explicit recalc workflows for a while, but the
current UX still lacks a clear, dedicated "reclassify this item now" command.

## Design Principles

### 1. Extend the current UX; do not restart it

The first LLM-backed version should fit into the already-shipped surfaces:

- Global Settings for policy
- `C` for bulk review
- edit-item inline suggestion handling
- item-level `?` markers and preview counts

We do not need a new "Classification Center" before we can ship useful LLM
classification.

### 2. One pipeline, multiple provider classes

Rule-based providers and semantic providers should share:

- candidate generation
- suggestion persistence
- provenance
- review
- recalc entry points

They should differ in latency, confidence, and routing policy, not in whether
they "count" as classification.

### 3. Preserve trust

Users should be able to answer:

- what was suggested?
- by which provider?
- using which model?
- why?
- was it auto-applied or accepted by review?

### 4. Keep local-first support first-class

Hosted providers can come later. The first semantic provider target should be a
local model through Ollama.

## Proposal Updates

### 1. Make the current TUI surfaces the official phase-1 UX

The proposal should stop treating these as future ideas and instead treat them
as the baseline product UX:

- `C` is the bulk review entry point
- the review overlay is item-grouped and two-pane
- edit-item inline suggestion review remains the per-item side channel
- Global Settings is the canonical home of the global classification mode toggle
- the `?` board indicator remains the passive pending-review signal

Follow-up UX work should refine these surfaces rather than invent parallel ones.

### 2. Split category-level controls by provider family

The current per-category `Auto-match` toggle should be explicitly documented as
the implicit-string provider gate, not the gate for all automatic
classification.

Updated product rule:

- `enable_implicit_string` controls only the implicit-string provider
- semantic providers should have a parallel per-category enablement control
  independent of `enable_implicit_string`
- exact naming is TBD, but the product model should support separate rule-based
  and LLM-based category participation
- numeric categories and reserved categories remain excluded from LLM category
  suggestions in the initial slice

Open follow-up:

- choose the final user-facing label for the semantic control
- decide whether semantic participation defaults on or off for newly-created
  categories

### 3. Honor provider registration in runtime code

The implementation should move from "hard-coded provider construction" to a
provider registry that is actually driven by persisted config.

Minimum phase-1 behavior:

- only enabled providers are instantiated
- provider execution mode is read from config
- cheap deterministic providers may still run inline
- expensive providers can be selectively enabled without affecting providers the
  user is not using

### 4. Add provider-specific settings

The current config model is missing provider-specific runtime settings.

For phase 1, add provider settings sufficient for Ollama:

- provider enabled
- base URL
- model name
- timeout
- optional system-prompt version marker

These settings can live inside classification config or under dedicated
provider-specific app settings. The important part is that they are persisted
per database and available to both CLI/TUI flows.

### 5. Ollama is the first semantic provider

Initial semantic provider target:

- provider id: `ollama_openai_compat`
- transport: OpenAI-compatible chat-completions style API against Ollama
- default model: `mistral`
- default stance: local-first, no hosted dependency required

This keeps the transport layer reusable later for:

- LM Studio in OpenAI-compatible mode
- actual hosted OpenAI-compatible providers if desired

### 6. Ollama MVP should be suggestion-first

For the first semantic slice, Ollama-backed category classification should be
review-first, not silent.

Recommended MVP policy:

- `Off`
  - Ollama does not run
- `SuggestReview`
  - Ollama may generate category suggestions
  - results are persisted as pending suggestions
  - users review them through `C` or the edit panel
- `AutoApply`
  - deterministic providers may still auto-apply
  - Ollama does not auto-apply in the MVP

Reason:

- semantic confidence is uncalibrated at first
- local models can be slow
- the review workflow already exists and gives us a safe integration point

This is intentionally conservative. We can revisit semantic auto-apply after we
have real usage data and a confidence policy we trust.

### 7. Keep `When` parsing separate in behavior, unified in architecture

The current behavior is good and should remain explicit in the proposal:

- `When` parsing is part of the classification architecture
- `When` parsing remains deterministic and inline
- even in `SuggestReview`, `When` results continue to apply immediately

That preserves the current fast capture workflow while letting category
classification become smarter and more review-oriented.

At the same time, the roadmap should explicitly leave room for a later semantic
`When` provider:

- current hardcoded/date-parser coverage remains the first-line `When` system
- later phases may add LLM-backed `When` suggestions when the parser cannot
  confidently interpret the text
- semantic `When` inference should be treated as a distinct provider, not
  conflated with category suggestion generation

### 8. Populate richer prompt context from real TUI state

The request envelope already anticipates richer context. Phase 1 should begin
using it for semantic providers.

When available, the LLM request should include:

- item text
- note text
- existing manual assignments
- current `when` value
- current numeric values
- visible view name
- visible section title
- candidate category descriptors

For the first slice, "visible" context only needs to be provided when an item
save/edit originates from a known TUI slot. CLI-originated saves can omit it.

### 9. Expand review UI to show provider details

Before semantic suggestions ship, the current review surfaces should be updated
so the suggestion metadata is not rule-centric.

Recommended additions:

- review overlay right pane shows provider and model
- confidence is shown when present
- rationale remains visible
- edit-item suggestion rows show a compact provider tag or expose provider/model
  in a secondary detail line for the selected suggestion

The user must be able to distinguish:

- rules matched a word
- `When` parser extracted a date
- Ollama inferred a semantic category

## Ollama MVP Contract

### Scope

In scope for the first implementation:

- local Ollama provider using an OpenAI-compatible API shape
- category suggestions only
- suggestion persistence
- suggestion review through existing `C` overlay and edit panel
- provider/model/rationale metadata visible in review
- per-database Ollama config

Out of scope for the first implementation:

- hosted providers
- background jobs
- database-wide semantic rescans
- category-change semantic backfills
- semantic auto-apply
- semantic `When` extraction

### Output shape

The semantic provider should return `ClassificationCandidate` values using the
same model as the existing providers:

- `assignment = CandidateAssignment::Category(category_id)`
- `provider = "ollama_openai_compat"`
- `model = Some(<configured model>)`
- `confidence = optional`
- `rationale = short explanation`

### Category universe for the first slice

For the first semantic slice, candidate categories should be:

- non-reserved
- non-numeric

and should not be limited by `enable_implicit_string`.

If category counts become too large for prompt quality, we should solve that
with candidate narrowing or ranking, not by implicitly reusing the rule toggle.

### Execution policy for the first slice

To get to a usable slice quickly and avoid overcommitting before async
infrastructure exists:

- Ollama runs only for current-item classification, not bulk recalc
- Ollama runs only when semantic classification is enabled and configured
- bulk and category-change semantic work is deferred to a later async phase

This gives us a small, testable first step that matches the current app shape.

## Revised Phase Plan

### Phase 1: Align architecture with shipped UX

- update the proposal and implementation to match current `C` review and edit
  panel flows
- honor enabled providers in runtime wiring
- add provider-specific settings storage
- add a separate per-category semantic/LLM participation control in parallel
  with implicit-string `Auto-match`
- enrich review UI with provider/model/confidence display

### Phase 2: Ollama semantic provider

- add an Ollama provider using an OpenAI-compatible API shape
- use `mistral` as the default model
- generate category suggestions for current-item save/edit flows
- route results into existing suggestion persistence and review flows

### Phase 3: Explicit recalc

- add "reclassify current item" command
- add focused recalc entry points before any large async job system

### Phase 4: Async and bulk infrastructure

- background jobs for slow providers
- category-change semantic rescans
- manual bulk recalc
- durable job progress/status

### Phase 5: Advanced `When` intelligence

- add an LLM-backed `When` suggestion provider for cases the deterministic parser
  cannot confidently handle
- keep deterministic `When` parsing as the first pass
- decide review vs auto-apply policy for semantic `When` suggestions
- reuse the same provider metadata and suggestion persistence model

### Phase 6: Hosted providers and richer scoping

- hosted providers if still desired
- better category narrowing
- confidence calibration
- optional semantic auto-apply policy

## Risks

### 1. Reusing the rule toggle accidentally constrains LLM behavior

If semantic providers keep inheriting `enable_implicit_string`, users will have
to enable "rule auto-match" just to let AI consider a category.

Mitigation:

- document and implement separate semantics now
- add an explicit per-category semantic participation control

### 2. Synchronous local-model latency can feel rough

Even local models may be slow enough to interrupt save/edit workflows.

Mitigation:

- keep Ollama scope to current-item flows only at first
- defer bulk semantic work until async infrastructure exists
- surface clear status text while classification is running

### 3. Review UI may become too opaque once provider diversity grows

Rationale-only display is not enough when multiple providers are active.

Mitigation:

- show provider/model/confidence before semantic suggestions ship

### 4. Semantic suggestions may overreach in mixed-purpose databases

Aglet databases can mix issue tracking, budgets, contacts, and general notes.

Mitigation:

- keep semantic classification suggestion-first
- use available view/section/manual context
- keep deterministic `When` behavior simple and separate

## Recommendation Summary

Recommended direction:

- treat the current classification pipeline as real infrastructure, not an MVP
  stub
- make the existing TUI surfaces the official first-phase UX
- keep `Auto-match` as the implicit-string control only
- add a parallel per-category semantic/LLM control with separate semantics
- wire provider config into actual runtime provider selection
- add provider-specific settings
- ship Ollama first, through an OpenAI-compatible transport layer
- use `mistral` as the default local model
- keep Ollama suggestion-first in the MVP
- keep `When` parsing deterministic and inline
- add LLM-backed `When` suggestions in a later phase
- postpone async/bulk semantic work and semantic auto-apply until later phases

## Separate Literal + LLM Classification MVP With Ollama

### Implementation Checklist

- [x] Replace the single global mode with independent `literal_mode` and
  `semantic_mode` settings
- [x] Add backward-compatible config loading from legacy
  `continuous_mode`
- [x] Add persisted Ollama settings for enabled/base URL/model/timeout
- [x] Add persisted per-category semantic participation
  (`enable_semantic_classification`)
- [x] Keep `enable_implicit_string` scoped to literal matching only
- [x] Honor `enabled_providers` in runtime provider construction
- [x] Split runtime execution into literal-family and semantic-family provider
  policy
- [x] Keep deterministic `When` parsing under the literal policy
- [x] Add synchronous Ollama provider wiring through an OpenAI-compatible
  transport
- [x] Use `mistral` as the default Ollama model
- [x] Restrict semantic candidate categories to non-reserved, non-numeric,
  semantic-enabled categories
- [x] Use exact category-name validation, duplicate filtering, and
  malformed-response fallback for Ollama outputs
- [x] Route semantic category suggestions into pending review rather than
  auto-apply
- [x] Add Global Settings rows for literal mode, semantic mode, Ollama enabled,
  Ollama base URL, and Ollama model
- [x] Add Category Manager `Semantic Match` toggle
- [x] Show provider/model/confidence metadata in review surfaces
- [x] Add initial unit/integration coverage for legacy config loading and core
  literal + semantic mode combinations
- [x] Update the local Aglet feature tracker with an in-progress feature request
  for this work
- [ ] Add dedicated TUI persistence coverage for the `Semantic Match` category
  toggle
- [ ] Add explicit TUI coverage for Ollama config string editing and review
  metadata rendering
- [ ] Run a real local Ollama smoke test against `mistral`
- [ ] Decide whether to expose timeout editing in Global Settings during this
  phase or leave it config-only
- [ ] Plan the later semantic `When` phase separately from the category MVP

### What Is Next

The next highest-value step is a real end-to-end smoke test with local Ollama:

- enable semantic classification in Global Settings
- point it at local Ollama
- save a few representative items
- verify pending semantic suggestions appear in `C` with provider/model metadata
- verify acceptance persists as `SuggestionAccepted`

After that, the next code step is to close the remaining TUI coverage gaps:

- `Semantic Match` toggle persistence test
- Ollama base URL/model editing test
- review metadata rendering test
