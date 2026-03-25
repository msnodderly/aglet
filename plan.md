# Separate Literal + LLM Classification MVP With Ollama

## Summary

Replace the single global classification mode with two independent
provider-family policies:

- `literal_mode` for deterministic category matching and the existing
  `when_parser`
- `semantic_mode` for Ollama-backed category inference

This gives the user exactly the combinations requested:

- literal only
- LLM only
- both
- neither

For the first usable slice:

- literal remains configurable as `Off | Auto-apply | Suggest/Review`
- semantic is configurable as `Off | Suggest/Review`
- `when_parser` follows the literal policy
- Ollama runs synchronously on current-item save/edit when
  `semantic_mode=SuggestReview`
- Ollama category results always queue review in MVP
- advanced LLM-based `When` suggestions are explicitly deferred to a later
  phase

## Key Changes

### Public model and config changes

- Replace `ClassificationConfig.continuous_mode` with:
  - `literal_mode: LiteralClassificationMode`
  - `semantic_mode: SemanticClassificationMode`
- Add enums:
  - `LiteralClassificationMode = Off | AutoApply | SuggestReview`
  - `SemanticClassificationMode = Off | SuggestReview`
- Keep `enabled_providers`, but make runtime honor it.
- Add Ollama settings to classification config:
  - `enabled`
  - `base_url`
  - `model`
  - `timeout_secs`
- Add per-category semantic participation:
  - `Category.enable_semantic_classification: bool`
- Keep `Category.enable_implicit_string` as the literal/rule control only.

### Runtime behavior

- Literal family:
  - includes `implicit_string`
  - includes current deterministic `when_parser`
  - runs according to `literal_mode`
- Semantic family:
  - includes `ollama_openai_compat`
  - runs only when `semantic_mode=SuggestReview`
  - stores category suggestions as pending review
  - never auto-applies in MVP
- Resulting combined behavior examples:
  - literal `AutoApply` + semantic `Off`: current deterministic behavior only
  - literal `AutoApply` + semantic `SuggestReview`: deterministic matches
    auto-apply, Ollama queues review
  - literal `Off` + semantic `SuggestReview`: Ollama-only category
    suggestions, no deterministic matching
  - literal `SuggestReview` + semantic `SuggestReview`: both families queue
    review, except deterministic `When` still applies inline per current
    behavior
- Category-change and bulk semantic runs remain out of scope for MVP.

### Ollama provider and prompt contract

- Add an `OllamaProvider` in `agenda-core` using OpenAI-compatible chat
  completions over blocking HTTP.
- Use `mistral` as the default model.
- Semantic category candidate pool:
  - non-reserved
  - non-numeric
  - `enable_semantic_classification=true`
- Literal candidate pool:
  - existing implicit-string eligibility rules
- Prompt includes:
  - item text
  - note
  - manual assignments
  - current `when`
  - numeric assignments
  - semantic-eligible category descriptors
- Response must be strict JSON with exact category names, confidence, and
  rationale.
- Unknown names, duplicates, malformed JSON, or request failures yield no
  semantic suggestions rather than failing the save.

### TUI and settings

- Replace the single Global Settings classification row with:
  - `Literal classification`
  - `Semantic classification`
  - `Ollama enabled`
  - `Ollama base URL`
  - `Ollama model`
- Add a Category Manager details toggle for the new semantic control, with MVP
  label `Semantic Match`.
- Update review surfaces to show provider/model/confidence for semantic
  suggestions.
- Keep existing `C` review overlay and edit-panel inline review flow unchanged.

## Detailed TODO Checklist

### 1. Config and schema
- [x] Add `LiteralClassificationMode` and `SemanticClassificationMode` enums.
- [x] Replace `ClassificationConfig.continuous_mode` with `literal_mode` and
  `semantic_mode`.
- [x] Add backward-compatible deserialization from old single-mode configs.
- [x] Choose migration mapping for old configs:
  - old `Off` -> literal `Off`, semantic `Off`
  - old `AutoApply` -> literal `AutoApply`, semantic `Off`
  - old `SuggestReview` -> literal `SuggestReview`, semantic `SuggestReview`
- [x] Add `Category.enable_semantic_classification` with SQLite
  migration/default `1`.
- [x] Add Ollama settings to persisted classification config with defaults:
  - enabled `false`
  - base URL `http://127.0.0.1:11434/v1`
  - model `mistral`
  - timeout `10`

### 2. Provider-family runtime refactor
- [x] Refactor classification service construction to be config-driven.
- [x] Split runtime routing into literal-family vs semantic-family providers.
- [x] Make literal provider execution depend on `literal_mode`.
- [x] Make semantic provider execution depend on `semantic_mode`.
- [x] Preserve existing pending-suggestion supersede/reject memory behavior.
- [x] Leave category-change semantic runs disabled in MVP.

### 3. Ollama provider implementation
- [x] Add blocking HTTP client dependency to `agenda-core`.
- [x] Add testable transport abstraction.
- [x] Implement OpenAI-compatible request builder for Ollama.
- [x] Implement strict JSON response parsing.
- [x] Map exact returned names to category IDs.
- [x] Return `ClassificationCandidate` with
  provider/model/confidence/rationale.
- [x] Drop invalid/unknown/duplicate category results.
- [x] Treat connection, timeout, or parse errors as “no semantic suggestions.”

### 4. Category eligibility and prompt context
- [x] Keep literal eligibility tied to `enable_implicit_string`.
- [x] Add semantic eligibility tied to `enable_semantic_classification`.
- [x] Exclude numeric and reserved categories from semantic prompts.
- [x] Include item-local context only in MVP; do not thread visible
  view/section context yet.

### 5. TUI settings and category controls
- [x] Replace Global Settings `Classification mode` row with separate
  literal/semantic rows.
- [x] Add Global Settings rows for Ollama enabled/base URL/model.
- [x] Reuse existing picker/name-input patterns for editing these values.
- [x] Add Category Manager details row for `Semantic Match`.
- [x] Persist and reload the new category flag cleanly.
- [x] Update status/help copy to explain literal vs semantic controls.

### 6. Review UI metadata
- [x] Show provider and model in suggestion review.
- [x] Show confidence when present.
- [x] Keep rationale visible.
- [x] Add compact provider detail to edit-panel pending suggestions.

### 7. Tests
- [x] Config roundtrip test for new dual-mode config and Ollama settings.
- [x] Backward-compat test for loading old `continuous_mode` configs.
- [x] Store migration test for `enable_semantic_classification`.
- [x] Agenda tests for all key combinations:
  - literal `AutoApply`, semantic `Off`
  - literal `AutoApply`, semantic `SuggestReview`
  - literal `Off`, semantic `SuggestReview`
  - literal `SuggestReview`, semantic `SuggestReview`
- [x] Test that `when_parser` follows literal policy.
- [x] Test that semantic-disabled categories are excluded from Ollama prompts.
- [x] Test that implicit-string-disabled but semantic-enabled categories can
  still be suggested by Ollama.
- [x] Ollama provider parsing tests:
  - valid response
  - unknown category
  - duplicate category
  - malformed JSON
  - timeout/error path
- [x] TUI test for new Global Settings rows and persistence.
- [x] TUI test for Category Manager semantic toggle persistence.
- [x] TUI render test for provider/model/confidence in review overlay.

## Test Plan

- Unit tests in `agenda-core` for config compatibility, provider routing,
  semantic candidate filtering, and Ollama response parsing.
- Agenda-level tests for combined literal + semantic mode behavior and
  suggestion persistence.
- TUI tests for:
  - separate literal/semantic mode rows
  - Ollama config editing
  - category semantic toggle
  - review metadata rendering
- Manual smoke test:
  - enable literal `AutoApply`
  - enable semantic `SuggestReview`
  - enable Ollama with `mistral`
  - save an item with vague text and rich note
  - verify deterministic matches auto-apply
  - verify Ollama suggestions appear pending in `C`
  - verify accepting one records `SuggestionAccepted`

## Assumptions and Defaults

- Independent provider-family policy is the intended long-term config shape.
- Semantic category participation defaults enabled for eligible categories.
- MVP label for the new per-category control is `Semantic Match`.
- `when_parser` follows literal policy in MVP.
- Semantic mode in MVP is `Off | Suggest/Review` only; no semantic auto-apply
  yet.
- Ollama config is edited through Global Settings in MVP.
- Advanced LLM-based `When` suggestions are a later phase and are not part of
  this slice.
