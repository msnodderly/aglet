---
title: Add OpenRouter as a Classification Provider
status: shipped
created: 2026-03-24
shipped: 2026-03-24
---

# Plan: Add OpenRouter as a Classification Provider

## Status: In Progress — Architecture Research Done, Implementation Not Started

## Context

We're adding OpenRouter as an alternative semantic classification provider alongside local Ollama. OpenRouter hosts models like `nvidia/nemotron-3-super-120b-a12b:free` and `stepfun/step-3.5-flash:free` that gave better results than local models in our benchmarks (see `docs/ollama-model-comparison.md`).

## What's Done

- Benchmarked 5 models (see `docs/ollama-model-comparison.md`)
- Optimized prompt for token efficiency (~60% smaller system prompt)
- Fixed confidence template (0.0 → 0.85), group filtering, already-assigned exclusion
- User-friendly error messages for timeouts and connection failures
- Ollama model discovery picker in Global Settings
- Read the architecture: `Aglet` struct, `OllamaTransport` trait, `OllamaProvider`, `ClassificationConfig`, `classification_service()` wiring

## Architecture Analysis

Key files:
- `crates/aglet-core/src/classification.rs` — providers, transport trait, config, prompts
- `crates/aglet-core/src/aglet.rs` — `Aglet` struct wires providers into `ClassificationService`

Current flow:
1. `ClassificationConfig` has `ollama: OllamaProviderSettings` (base_url, model, timeout_secs, enabled)
2. `OllamaTransport` trait abstracts HTTP calls (`complete()` method)
3. `ReqwestOllamaTransport` implements it for local Ollama
4. `OllamaProvider` uses the transport + shared system/user prompt logic
5. `Aglet::classification_service()` wires providers based on config flags
6. `PROVIDER_ID_OLLAMA_OPENAI_COMPAT` is the provider ID string

OpenRouter API is OpenAI-compatible (`/v1/chat/completions`), so the transport layer is nearly identical. Key differences:
- Different base URL: `https://openrouter.ai/api/v1`
- Requires `Authorization: Bearer <API_KEY>` header
- Model IDs include org prefix: `nvidia/nemotron-3-super-120b-a12b:free`
- No `response_format` needed (worked without it in testing)

## Implementation Plan

### 1. Add OpenRouter settings to ClassificationConfig
```rust
pub struct OpenRouterProviderSettings {
    pub enabled: bool,
    pub api_key: String,       // from env or config
    pub model: String,         // e.g. "nvidia/nemotron-3-super-120b-a12b:free"
    pub timeout_secs: u64,     // default 60 (network latency)
}
```
Add `openrouter: OpenRouterProviderSettings` to `ClassificationConfig`.
Add `PROVIDER_ID_OPENROUTER` constant.

### 2. Implement OpenRouter transport
Either:
- (a) Reuse `OllamaTransport` trait with an `OpenRouterTransport` impl that adds the auth header, OR
- (b) Create a shared `OpenAiCompatTransport` that takes base_url + optional api_key + timeout

Option (b) is cleaner since both are OpenAI-compatible. The `OllamaProvider` can be generalized or a new `OpenRouterProvider` can share the same prompt logic.

### 3. Wire into Aglet
- Add `openrouter_transport` to `Aglet` struct (or reuse shared transport)
- Add OpenRouter provider to `classification_service()` when enabled
- Pass API key securely (env var `OPENROUTER_API_KEY` or stored in config)

### 4. Add to Global Settings TUI
New rows in GlobalSettings:
- OpenRouter Enabled (toggle)
- OpenRouter API Key (text input, masked display)
- OpenRouter Model (text input or picker — could query OpenRouter models API)
- OpenRouter Timeout (text input)

`GlobalSettingsRow` enum gets 4 new variants. Rendering adds the rows.

### 5. API key handling
Options:
- Read from env var `OPENROUTER_API_KEY` at startup, store in config
- Allow manual entry via Global Settings text input
- Display masked in settings (show last 4 chars only)

### 6. Tests
- Unit test for OpenRouter transport (mock HTTP)
- Test that parser handles OpenRouter responses (same JSON format)
- Test Global Settings rows for OpenRouter config
- Integration test with fake transport

## Open Questions
- Should we generalize `OllamaProvider` into a generic `OpenAiCompatProvider` that works for both? (Probably yes — the prompt logic is identical)
- Should the API key be persisted in the .ag database or only read from env? (Env is more secure, but less convenient)
- Model picker for OpenRouter — worth querying their API? Or just text input since model IDs are org-prefixed strings?
