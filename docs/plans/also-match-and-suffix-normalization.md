---
title: Also Match Terms + Suffix Normalization
status: shipped
created: 2026-03-15
shipped: 2026-03-22
---

# Implementation Plan: Also Match Terms + Suffix Normalization

## Goal

Improve fallback text-based category assignment in two narrowly scoped ways:

1. Let users add explicit per-category `Also match` terms/phrases.
2. Normalize obvious English inflections so implicit matching catches common
   word forms (`call`, `calls`, `calling`, `called`, `caller`).

This plan intentionally does **not** implement Lotus Agenda's full `Also match`
mini-language. Aglet's long-term direction is semantic category assignment via
LLM-backed providers, so the deterministic matcher should stay predictable,
cheap, and explainable.

## Status

- Branch: `codex/also-match-suffix-normalization`
- Worktree: `/Users/mds/src/aglet-also-match-suffix-normalization`

## Product Decisions

### 1. Two separate concepts

These are different features and should stay separate in the model:

- `Also match`
  - user-authored matcher hints stored on the category
  - also useful as future LLM prompt context
- suffix normalization
  - deterministic matching behavior
  - stored nowhere; applied at match time

### 2. `Also match` is literal in v1

Store exactly what the user enters as a list of strings.

In scope:

- single-word terms like `phone`, `dial`, `ring`
- multi-word phrases like `board meeting`
- one entry per line in the TUI editor

Out of scope:

- semicolon/comma expression syntax
- negation
- wildcards
- substring operators like `~port`
- per-category match strength / initiative controls

### 3. Matching stays boundary-based

The current substring matcher should be replaced for implicit classification
with token/phrase matching over normalized tokens. This keeps behavior
predictable and avoids false positives like:

- `ring` matching `spring`
- `dial` matching `redial`

### 4. Suffix normalization is conservative

We want better recall for obvious English variants without turning this into a
general-purpose stemmer.

Rules:

- lowercase first
- strip trailing possessive `'s`
- tokenize on non-alphanumeric boundaries
- only strip suffixes for tokens with length `>= 4`
- never strip below 3 visible characters

Initial suffix list:

- `ies -> y`
- `ied -> y`
- `ing`
- `ed`
- `ers`
- `er`
- `es`
- `s`
- `ly`

Examples:

- `call`, `calls`, `called`, `calling`, `caller` -> `call`
- `design`, `designed`, `designing`, `designer` -> `design`
- `parties` -> `party`

## Matching Semantics

The matcher evaluates these candidate terms for a category:

1. the category name
2. each `also_match` entry

Matching rules:

- Single-word candidate
  - match if any normalized token in item text equals the normalized candidate
- Multi-word candidate
  - match if any contiguous normalized token window equals the normalized
    candidate token sequence

Examples:

- Category `Phone Calls`
- `also_match = ["phone", "dial", "ring"]`

Matches:

- `call mom`
- `calling mom`
- `need to dial Sarah`
- `phone Bob about contract`

Does not match:

- `spring cleanup` via `ring`
- `redial customer` via `dial`

Phrase example:

- `also_match = ["board meeting"]`

Matches:

- `board meeting tomorrow`
- `board meetings tomorrow`

Does not match in v1:

- `meeting with the board`

## Model + Persistence

### `Category`

Add:

```rust
pub also_match: Vec<String>,
```

This belongs on `Category`, not in `Condition`, because it is matcher metadata
and future LLM context rather than a rule-programming primitive.

### Database

Add a new column to `categories`:

```sql
also_match_json TEXT NOT NULL DEFAULT '[]'
```

Migration requirements:

- bump schema version
- add `ALTER TABLE` path for existing databases
- default missing/invalid values to `[]`

## UI Design

### Category Manager details pane

Add an `Also match` field to the non-numeric category details pane.

Key behavior:

- shown only for tag categories
- focused as a normal details row
- `Enter` opens inline text editing
- edited as multi-line text, one entry per line
- `S` saves to the selected category
- `Esc` while editing discards inline changes and closes the editor

### Mockup: Category details pane

```text
Category Manager                                    Category
┌─────────────────────────────────────────────────┐  ┌──────────────────────────┐
│ Status                                          │  │ Selected: Phone Calls    │
│ > Phone Calls                                   │  │ Parent: Workflow         │
│   Travel                                        │  │ Depth: 1  Children: 0    │
│   Meetings                                      │  │                          │
└─────────────────────────────────────────────────┘  │ Flags                    │
                                                     │ [ ] Exclusive            │
                                                     │ [x] Auto-match           │
                                                     │ [x] Actionable           │
                                                     │                          │
                                                     │ Also match               │
                                                     │ phone                    │
                                                     │ dial                     │
                                                     │ ring                     │
                                                     │                          │
                                                     │ Note                     │
                                                     │ Calls that need          │
                                                     │ synchronous follow-up.   │
                                                     └──────────────────────────┘
Tab:pane  j/k:move  Enter:edit/toggle  S:save  Esc:close
```

### Mockup: `Also match` editing state

```text
Category Manager                                    Category
┌─────────────────────────────────────────────────┐  ┌──────────────────────────┐
│ Status                                          │  │ Flags                    │
│ > Phone Calls                                   │  │ [ ] Exclusive            │
│   Travel                                        │  │ [x] Auto-match           │
│   Meetings                                      │  │ [x] Actionable           │
└─────────────────────────────────────────────────┘  │                          │
                                                     │ Also match (editing)     │
                                                     │ phone                    │
                                                     │ dial                     │
                                                     │ ring_                    │
                                                     │                          │
                                                     │ One term or phrase per   │
                                                     │ line. S saves. Esc       │
                                                     │ discards inline edits.   │
                                                     └──────────────────────────┘
S:save  Enter:new line  Esc:cancel
```

Notes:

- one-entry-per-line avoids delimiter parsing/escaping complexity
- the edit surface should look like the existing inline/note editing model, not
  a modal popup
- we should keep the full Note pane below; `Also match` is not a replacement
  for note text

## Implementation Outline

### Phase 1: Core model + storage

- add `Category.also_match`
- add `also_match_json` column and migration
- include it in create/update/load paths
- update defaults/builders/tests that construct `Category`

Files likely affected:

- `crates/agenda-core/src/model.rs`
- `crates/agenda-core/src/store.rs`
- `crates/agenda-core/src/engine.rs`
- `crates/agenda-tui/src/ui_support.rs`
- `crates/agenda-tui/src/lib.rs`

### Phase 2: Replace raw substring implicit matching

- add tokenization + normalization helpers in `matcher.rs`
- match normalized single tokens and contiguous phrases
- expose enough match detail to explain whether the category name or an alias
  matched
- use the same matcher path in both:
  - engine implicit evaluation
  - classification provider implicit suggestions

Files likely affected:

- `crates/agenda-core/src/matcher.rs`
- `crates/agenda-core/src/engine.rs`
- `crates/agenda-core/src/classification.rs`

### Phase 3: Category Manager editing

- add a new details focus row for `Also match`
- add inline multi-line editing state
- save edited lines back into the selected category
- preserve current note-editing behavior

Files likely affected:

- `crates/agenda-tui/src/lib.rs`
- `crates/agenda-tui/src/app.rs`
- `crates/agenda-tui/src/modes/category.rs`
- `crates/agenda-tui/src/render/mod.rs`

### Phase 4: Tests

Core:

- store roundtrip for `also_match`
- migration/default behavior for old rows
- single-token suffix normalization
- possessive handling
- alias matching
- multi-word phrase matching
- boundary safety (`ring` does not match `spring`)

TUI:

- details pane includes `Also match`
- `Enter` on `Also match` starts editing
- save persists one-entry-per-line edits
- `Esc` discards inline `Also match` edits
- moving selection resets inline editor cleanly

## Risks / Watchouts

- Existing tests construct `Category` values directly in many places; adding a
  required field will have broad compile fallout.
- Current implicit matching runs against `item.text` plus full note body. Even
  conservative normalization increases match surface, so boundary-preserving
  phrase matching matters.
- The current Category Manager has separate note-editing and inline-input state.
  `Also match` editing should reuse that pattern rather than creating a third
  unrelated editor path if avoidable.

## Explicit Non-Goals

- full Lotus Agenda syntax
- fuzzy semantic guessing for aliases
- automatic alias suggestion
- per-category confidence/review policy
- global "ignore suffixes on/off" setting

Those can be revisited later if the future LLM-backed categorizer still leaves a
gap that deterministic rules need to cover.
