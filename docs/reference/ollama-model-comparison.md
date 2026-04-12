---
title: Ollama Model Comparison
updated: 2026-03-24
---

# Ollama Model Comparison for Semantic Classification

**Test item**: "another test item: figure out why auto-suggest is flaky."
**Note**: "related to terminal user interface. this is driving me crazy\nhighest priority. urgent."
**Already assigned**: Aglet, When, TUI, Bug (TUI/Bug auto-matched by literal classifier)
**Expected**: Critical or High (priority), possibly TODO

| Model | Time | Prompt tokens | Suggestions | Quality |
|-------|------|--------------|-------------|---------|
| **nemotron-3-super-120b** (OpenRouter) | 39.2s | 294 | CLI (0.92), Critical (0.96), TODO (0.78) | **Best** — nailed Critical + TODO. CLI is wrong (TUI already assigned). |
| step-3.5-flash (OpenRouter) | 49.3s | 289 | CLI (0.9), Critical (0.9), In Progress (0.7) | Good — got Critical right. CLI/TUI confusion. "In Progress" is a stretch. 4.5k completion tokens (thinking model). |
| **gemma:latest** (local) | **9.4s** | 301 | Critical (0.95), ~~Issue type (0.85)~~ [group, dropped] | **Best local** — nailed priority. Fast. Group suggestion filtered by parser. |
| mistral:latest (local) | 21.0s | 316 | In Progress (0.9), CLI (0.8), Needs Refinement (0.7) | Poor — all wrong. "In Progress" is a status guess, "CLI" confused with TUI. |
| nemotron-3-nano:4b (local) | 31.7s | 716 | Idea (0.95), Feature request (0.9) | Poor — classified a bug as "Idea". 2x tokens from reasoning overhead. |
| claude-opus-4-6 | — | — | Critical (0.95), TODO (0.75) | Reference — "highest priority, urgent" → Critical. Actionable investigation → TODO. |

## Notes

- nemotron-3-super-120b gave the best quality (Critical + TODO matches reference answer) but requires API access
- gemma is the best local option: 3x faster than local nemotron, half the tokens, correct priority pick
- nemotron-3-nano:4b (local) is a reasoning model; its chain-of-thought overhead hurts both speed and quality
- The `[group]` parser filter correctly catches gemma's "Issue type" suggestion
- CLI vs TUI confusion is common — TUI is already assigned so CLI suggestions get filtered as duplicates wouldn't apply (different category) but our already-assigned filter only removes TUI not CLI. Adding "tui, terminal" to TUI's also_match aliases helps models understand the distinction.
