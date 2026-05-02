---
title: Assignment Provenance, Explanation, and Debug Logging
status: shipped
created: 2026-03-22
shipped: 2026-03-29
---

# Assignment Provenance, Explanation, and Debug Logging Plan

## Summary

Add a small, consistent explanation/provenance layer for category assignments so
Aglet can answer:

- why an assignment currently exists
- what changed during a reprocess
- why an assignment was removed

This should be implemented mostly as internal plumbing, with only a few focused
user-visible surfaces:

- richer assignment inspector / CLI `show` explanation text
- concise status feedback for immediate assignment/removal side effects
- console logging of assignment-related events when debug mode is enabled

The plan intentionally avoids building a full history/audit log or a large new
UI. The goal is to make current automation explainable and debuggable without
turning provenance into a new subsystem of its own.

## Current State

Aglet already tracks a minimal assignment provenance tuple:

- `Assignment.source`
- `Assignment.origin`
- `Assignment.sticky`
- `Assignment.assigned_at`

This is enough for low-level debugging, but not enough for great explanation UX.

Examples of what we can say today:

- `Critical | AutoMatch | profile:Critical`
- `Priority | Subsumption | subsumption:Priority`

Examples of what we cannot currently say well:

- `Critical was added because profile rule 1 on Critical matched Waiting/Blocked + Escalated`
- `Critical was removed because Escalated was manually unassigned`
- `Phone Calls matched alias "call"`
- `Archive was assigned because Done fired Assign[Archive]`

We also have some event-like information transiently during processing:

- `ProcessItemResult.new_assignments`
- `ProcessItemResult.removed_assignments`
- `DeferredRemoval { target, triggered_by }` for `Action::Remove`
- semantic suggestion rationales/debug summaries

But these are not unified into a structured explanation/event stream.

## Goals

- Explain why a current assignment exists in user-facing inspector/CLI surfaces.
- Explain immediate side effects of a user action or item reprocess.
- Make auto-break behavior understandable instead of "silent magic".
- Improve developer/operator visibility under debug mode without affecting
  normal UX.
- Refactor provenance creation so assignment source/origin logic is more
  consistent across manual, implicit, profile, action, subsumption, and
  suggestion flows.

## Non-Goals

- full historical audit/event log
- arbitrary multi-hop causal graph visualization
- removal-history browser
- broad UI redesign
- new matching/rule semantics

## Proposal

### 1. Add persisted assignment explanation payload

Add a new optional structured explanation field on assignments, separate from
`source` and `origin`.

Sketch:

```rust
pub struct Assignment {
    pub source: AssignmentSource,
    pub assigned_at: Timestamp,
    pub sticky: bool,
    pub origin: Option<String>,
    pub explanation: Option<AssignmentExplanation>,
    pub numeric_value: Option<Decimal>,
}
```

```rust
pub enum AssignmentExplanation {
    Manual {
        origin: String,
    },
    ImplicitMatch {
        matched_term: String,
        matched_source: ImplicitMatchSource, // category name vs also_match
    },
    ProfileCondition {
        owner_category_id: CategoryId,
        owner_category_name: String,
        condition_index: usize,
        rendered_rule: String,
    },
    Action {
        trigger_category_id: CategoryId,
        trigger_category_name: String,
        kind: AssignmentActionKind, // assign/remove
    },
    Subsumption {
        parent_category_id: CategoryId,
        parent_category_name: String,
        via_child_category_id: CategoryId,
        via_child_category_name: String,
    },
    SuggestionAccepted {
        provider: String,
        confidence: Option<f32>,
        rationale: Option<String>,
    },
    AutoClassified {
        provider: String,
        confidence: Option<f32>,
        rationale: Option<String>,
    },
}
```

Notes:

- Keep `source` and `origin` for compatibility and debugging.
- `explanation` becomes the primary user-facing provenance payload.
- Persist it so inspectors/CLI `show` can explain current state, not just the
  last operation.

### 2. Add ephemeral assignment event stream for immediate feedback

Add a new result payload for processing/reprocessing that records what happened
this run.

Sketch:

```rust
pub struct AssignmentEvent {
    pub kind: AssignmentEventKind,
    pub category_id: CategoryId,
    pub category_name: String,
    pub detail: AssignmentEventDetail,
}

pub enum AssignmentEventKind {
    Assigned,
    Removed,
}

pub enum AssignmentEventDetail {
    Manual,
    ImplicitMatchAdded { matched_term: String, matched_source: ImplicitMatchSource },
    ProfileAdded { owner_category_name: String, condition_index: usize, rendered_rule: String },
    ActionAdded { trigger_category_name: String },
    ActionRemoved { trigger_category_name: String },
    SubsumptionAdded { via_child_category_name: String },
    AutoBreakProfile { owner_category_name: String, condition_index: usize },
    AutoBreakImplicit { matched_term: Option<String>, matched_source: Option<ImplicitMatchSource> },
    AutoBreakSubsumption { via_child_category_name: String },
}
```

`ProcessItemResult` should carry:

- `assignment_events: Vec<AssignmentEvent>`

This stays transient and is meant for:

- status line summaries
- debug console logging
- tests

This avoids overloading persisted assignment state with short-lived removal
messages.

### 3. Centralize provenance/explanation construction

Refactor assignment creation so all provenance/explanation construction happens
through a small shared set of helpers instead of ad hoc call sites.

Possible helpers:

- `Assignment::manual(...)`
- `Assignment::implicit_match(...)`
- `Assignment::profile_condition(...)`
- `Assignment::action(...)`
- `Assignment::subsumption(...)`
- `Assignment::suggestion_accepted(...)`
- `Assignment::auto_classified(...)`

And corresponding event helpers:

- `assignment_event_added_*`
- `assignment_event_removed_*`

This reduces drift between:

- `workspace.rs`
- `engine.rs`
- suggestion acceptance paths
- preview paths

### 4. Track richer implicit/profile context at assignment time

The matcher and engine already know more than we persist today.

We should preserve at least:

- matched implicit term
- whether it came from category name or `also_match`
- which profile condition index matched
- rendered profile rule text
- which category triggered an action

Refactor engine matching so assignment creation receives this structured context
instead of only `MatchReason` + stringified origin.

### 5. Surface explanation in minimal user-visible places

This effort should remain mostly internal plumbing, but a few surfaces should be
improved so the plumbing is actually useful.

#### Inspector / read-only details

Primary detailed surface:

- TUI assignment inspector popup
- CLI `aglet show`

Proposed UX:

- keep the compact source/origin row for debugging
- add one short human explanation line per assignment

Examples:

- `Critical`
  - `Derived from profile rule 1 on Critical: Waiting/Blocked + Escalated`
- `Phone Calls`
  - `Matched alias "call"`
- `Priority`
  - `Inherited from child High`

#### Immediate feedback surface

Status line / transient message after assignment-affecting operations:

- `Removed Escalated; auto-removed Critical`
- `Edited item; auto-removed Phone Calls`
- `Assigned Waiting/Blocked; auto-added Critical`

This should summarize immediate consequences, not dump a long event trace.

### 6. Add debug console logging for assignment-related events

When debug is enabled, emit structured console logs for assignment and related
events.

Use existing debug plumbing:

- `Workspace::with_debug(...)`
- TUI `--debug`

Plan:

- introduce a shared debug event emitter in `aglet-core`
- log to stderr / existing debug output path only when `debug == true`
- keep logs compact and line-oriented

Suggested event families:

- `assign.manual`
- `assign.implicit`
- `assign.profile`
- `assign.action`
- `assign.subsumption`
- `remove.manual`
- `remove.action`
- `remove.autobreak.profile`
- `remove.autobreak.implicit`
- `remove.autobreak.subsumption`
- `link.created`
- `link.removed`
- `when.auto_classified`
- `when.manual`

Example lines:

```text
assign.profile item=... category=Critical rule=1 owner=Critical
remove.autobreak.profile item=... category=Critical owner=Critical rule=1
assign.implicit item=... category=Phone Calls matched_term="call" matched_source=also_match
```

This logging is mainly for:

- development/debugging
- reproducing user reports
- validating cascades during tests/manual QA

### 7. Keep preview paths consistent

Preview APIs should reuse the same explanation/event plumbing where possible.

That means:

- preview assignment toggles should compute the same explanation payloads for
  hypothetical assignments
- preview-related UI can eventually use the same human-readable explanation
  formatting helpers

Do not fork separate "preview provenance" logic unless absolutely necessary.

## User-Visible Changes

This should remain mostly internal plumbing. The intentional user-visible
changes are:

- assignment inspector becomes more human-readable
- CLI `show` explains why assignments exist in friendlier terms
- status feedback after assignment/edit operations can mention auto-added or
  auto-removed categories
- debug mode prints assignment/link/date automation events to the console

Everything else is internal consistency/refactoring.

## Refactoring For Consistency

This work is a good opportunity to normalize a few uneven areas:

### A. Stop relying on raw `origin` strings as the primary explanation channel

Keep them, but make them secondary. Route user-facing explanation through
structured payloads and formatter helpers.

### B. Unify assignment constructors

Right now assignment structs are assembled in multiple places with slightly
different conventions. Shared constructors/helpers will make sources, origins,
sticky semantics, and explanations line up.

### C. Unify explanation formatting

Add shared formatter helpers for:

- compact labels
- human-readable inspector lines
- debug log lines
- status summaries

This avoids TUI and CLI drifting apart.

### D. Separate "current state explanation" from "event that just happened"

Persisted explanation answers:

- `why is this assigned now?`

Transient events answer:

- `what changed just now?`

Do not force one structure to do both jobs.

## Implementation Phases

### Phase 1: Core model + storage

- add `Assignment.explanation`
- add store persistence/migration support
- add shared constructors/helpers
- keep backward-compatible defaults for older rows

### Phase 2: Engine provenance context

- thread richer implicit/profile/action/subsumption context through engine
- populate explanation on assignment creation
- populate `assignment_events` during processing

### Phase 3: Aglet integration

- propagate explanation/event plumbing through manual assign/unassign, preview,
  suggestion acceptance, date parser flows, and action-trigger paths
- add debug event emitter wired to `workspace.debug`

### Phase 4: User-facing read-only surfaces

- TUI inspector explanation text
- CLI `show` explanation text
- concise status messages for immediate assignment side effects

### Phase 5: Tests and polish

- persistence roundtrip for explanations
- engine assignment-event coverage
- status message coverage
- TUI inspector rendering coverage
- CLI show snapshot coverage
- debug-log coverage for key event families

## Testing Matrix

### Core explanation cases

- implicit match via category name
- implicit match via alias
- profile condition with multiple rules on same category
- action assign
- action remove
- subsumption ancestor
- suggestion accepted

### Removal / auto-break cases

- profile auto-break after manual prerequisite removal
- implicit auto-break after text edit
- subsumption auto-break after descendant removal
- deferred remove action after cascade

### UI / presentation

- inspector renders human explanation line
- CLI `show` renders explanation line
- status line summarizes immediate side effects without being too noisy

### Debug logging

- no debug output when debug disabled
- expected log lines when debug enabled
- logs include enough detail to diagnose rule cascades

## Open Questions

1. Should explanation payloads be persisted immediately, or can a v1 keep some
   explanation detail transient and only persist a minimal subset?
2. Do we want CLI `show` to always print explanations, or only under
   `--verbose`?
3. For status summaries, how much detail is too much before it becomes noisy?
4. Should debug logging remain TUI-only initially because that is where the
   debug flag already exists, or should CLI gain a matching debug flag in the
   same effort?

## Recommendation

Do this as a plumbing-first refactor with a small number of read-only user
surfaces. The most important deliverables are:

- structured explanation on current assignments
- structured event stream for just-completed changes
- consistent debug logging under debug mode

If we get those right, user-facing explanation can grow incrementally without
reopening engine design later.
