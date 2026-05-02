---
title: OpenAI Provider Handoff
updated: 2026-03-30
---

# Handoff: OpenAI Provider Support

**Date:** 2026-03-30
**Branch:** `feat/openrouter-provider`
**Status:** Implementation complete, all tests passing, uncommitted

## What was accomplished

Added OpenAI as the third semantic classification provider (alongside Ollama and OpenRouter). The full provider stack is implemented end-to-end:

### Core (`crates/aglet-core/src/classification.rs`)
- `PROVIDER_ID_OPENAI` constant
- `OpenAiProviderSettings` struct — model (`gpt-4.1-nano` default), timeout (60s), `api_key()` reads `$OPENAI_API_KEY`
- `OpenAiTransport` trait + `ReqwestOpenAiTransport` — `https://api.openai.com/v1/chat/completions`, Bearer auth
- `OpenAiProvider` implementing `ClassificationProvider` — reuses shared `SEMANTIC_SYSTEM_PROMPT`, `build_semantic_user_prompt`, `parse_semantic_suggestions`
- `SemanticProviderKind::OpenAi` variant
- `openai: OpenAiProviderSettings` on `ClassificationConfig` + wire struct + deserialization
- `openai_transport` on `BackgroundClassificationJob`

### Integration (`crates/aglet-core/src/aglet.rs`)
- `openai_transport: Arc<dyn OpenAiTransport>` on `Agenda` struct
- Threaded through `new()`, `with_ollama_transport()`, `with_transports()`, `with_debug()`
- `OpenAi` arm in `classification_service_inner()` match
- `PROVIDER_ID_OPENAI` added to `is_semantic` check and `candidate_status_for_config`
- `openai_transport` included in `BackgroundClassificationJob` construction

### TUI (`crates/aglet-tui/`)
- `async_classify.rs` — `OpenAi` arm in `run_classification_job` provider match
- `state/category.rs` — `GlobalSettingsRow::OpenAiModel`, `OpenAiTimeout` + `visible_rows()` arm
- `state/board.rs` — `NameInputContext::OpenAiModel`, `OpenAiTimeout`
- `modes/global_settings.rs` — provider cycling (Ollama -> OpenRouter -> OpenAI -> Ollama), `semantic_provider_label` returns "OpenAI", Enter/text-input dispatch for both rows
- `modes/board.rs` — save dispatch for `OpenAiModel`/`OpenAiTimeout`, return mode mapping
- `render/mod.rs` — rendering for OpenAI Model and Timeout rows

## Key decisions

- **Followed the exact OpenRouter pattern.** Transport trait abstraction, provider struct with debug logging to `/tmp/aglet-openai-debug.log`, shared prompt/parse functions. No new abstractions introduced.
- **Default model `gpt-4.1-nano`** — cheapest OpenAI model suitable for classification tasks.
- **Single-select provider model preserved.** Only one of Ollama/OpenRouter/OpenAI is active at a time, controlled by `SemanticProviderKind`. Dynamic settings rows show only the active provider's config.

## Important context for future sessions

- **All changes are uncommitted** on `feat/openrouter-provider`. The branch has prior OpenRouter work already committed. Run `git diff` to see the OpenAI additions.
- **No new tests were written** for OpenAI specifically — the existing test suite (996 tests) passes cleanly. The OpenAI provider follows identical patterns to OpenRouter, which also lacks dedicated unit tests (both rely on the transport trait for testability).
- **The three providers share identical API shape** (OpenAI chat completions format). A future refactor could unify all three behind a single generic "OpenAI-compatible" transport parameterized by base URL and auth, but that wasn't done here to keep the change minimal.
- **Open FR items** from memory are unrelated to this work — columns in ViewEdit, save-on-exit, boolean criteria, per-view sorting.
