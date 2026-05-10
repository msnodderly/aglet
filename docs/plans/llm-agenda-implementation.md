---
title: LLM Agenda implementation and operating contract
status: active
created: 2026-05-10
updated: 2026-05-10
---

# LLM Agenda Implementation And Operating Contract

## Goal

Implement an "LLM Agenda" operating model in aglet where an LLM agent
incrementally builds and maintains a persistent knowledge database from curated
sources, rather than relying on stateless per-query retrieval.

Raw sources stay immutable. The agent creates and maintains items, categories,
and views under a consistent schema so the database compounds across sessions.

## Current Status

Phase 0 foundation is implemented:

- Bootstrap script: `scripts/init-llm-agenda-db.sh`
- Repo-local skill: `skills/llm-agenda/`
- Working database default: `../llm-agenda.ag`
- Raw source inbox default: `inbox/llm-agenda/`
- Processed source archive default: `inbox/llm-agenda/processed/`
- Baseline category tree and top-level category views are created
  reproducibly by the bootstrap script.
- The primary navigation model is PARA: `Projects`, `Areas`, `Resources`, and
  `Archives`, with review metadata kept in separate top-level families.
- Baseline categories start with implicit string matching disabled to avoid
  noisy auto-classification during early schema evolution.
- Baseline views have at least one section and one category-backed column.
- `Completed` is used instead of `Done`, because `Done` is a reserved aglet
  category name.

Remaining phases focus on supervised ingest, query compounding, and
lint/scaling discipline.

## Success Criteria

- A reproducible ingest workflow exists for adding a source and producing
  structured items, categories, and view updates.
- A documented schema contract exists for agent behavior: naming, category
  families, provenance, lint rules, and review workflow.
- Query workflows can produce answer artifacts that are persisted back into the
  database as items, views, or categories.
- The workflow scales to at least a moderate local corpus, roughly 100 sources
  and low-thousands of items, without external embedding infrastructure.

## Scope

### In Scope

1. Schema and operating conventions:
   - PARA category taxonomy patterns.
   - Exclusive and non-exclusive metadata families.
   - Assignment, naming, note-format, and provenance conventions.
2. Core operations:
   - Ingest: source to items, categories, and views.
   - Query: view-driven answer synthesis with item-level citations.
   - Lint: health checks, recommended repairs, and category/view suggestions.
3. Repo-local enablement:
   - Bootstrap script for baseline categories, views, and inbox layout.
   - Repo-local skill that points agents to this contract.

### Out Of Scope For Initial Release

- Building an external vector index or RAG service.
- Fully autonomous unattended ingest of all sources.
- Hard backward-compatibility guarantees for early schema iterations.
- A dedicated Rust CLI subcommand before the script workflow proves too
  limited.

## Bootstrap

Run from the repository root:

```bash
./scripts/init-llm-agenda-db.sh ../llm-agenda.ag inbox/llm-agenda
```

The script creates the inbox directories, baseline categories, and top-level
category views. It is safe to re-run; existing categories and views are
reported and left unchanged.

## Ownership Rules

- Raw sources are authoritative input. Do not edit them during ingest.
- The Agenda database is maintained by the agent, with the user reviewing the
  results in the TUI or CLI.
- PARA is the primary organization model: put information where it will be
  useful for action first, then use metadata categories for workflow control.
- Categories are reusable navigation structure, not provenance bookkeeping.
  Keep source paths, source-local locations, URLs, and ingest details in item
  notes unless a source or collection becomes useful for repeated browsing as a
  resource.
- Prefer explicit manual assignments during early schema evolution. Enable
  implicit matching only for narrow, reviewed child categories.

## Category Contract

Baseline top-level categories:

| Category | Exclusive | Purpose |
|---|---:|---|
| `Projects` | no | Active short-term outcomes with a clear finish line |
| `Areas` | no | Ongoing responsibilities and standards requiring continuing attention |
| `Resources` | no | Reference material, topics, assets, entities, concepts, and source collections |
| `Archives` | no | Inactive or completed material retained for future reference |
| `Status` | yes | Item lifecycle or trust state |
| `Priority` | yes | Importance or urgency |
| `Signal` | no | Analytic flags that identify knowledge-base conditions or next steps |

Baseline child categories:

- `Status`: `Open`, `In Progress`, `Completed`, `Superseded`, `Needs Review`
- `Priority`: `Critical`, `High`, `Normal`, `Low`
- `Signal`: `Contradiction`, `Gap`, `Follow-up`, `Hypothesis`

Use `Open` as the default active status for knowledge items. Use `Completed`,
not `Done`, for finished workflow status.

### Child Category Intent

`Signal` children are analytic annotations, not lifecycle states:

- `Contradiction`: two or more claims, items, or sources conflict or are in
  clear tension.
- `Gap`: the database lacks evidence, context, or coverage needed to answer a
  question confidently.
- `Follow-up`: a concrete research, review, or maintenance action should be
  pursued later.
- `Hypothesis`: a tentative synthesis or explanation is worth tracking but is
  not yet established.

`Status` children describe item lifecycle and review state:

- `Open`: active default state for knowledge items.
- `In Progress`: currently being worked or reviewed.
- `Completed`: work or review is finished.
- `Superseded`: replaced by newer or more authoritative information.
- `Needs Review`: the item itself needs human or agent review before being
  trusted.

Use `Needs Review` when the item's wording, interpretation, evidence, or
classification is suspect. Use `Gap` when the item can be valid but the broader
knowledge base is missing information. Use `Contradiction` only when there is a
specific competing claim or source to compare against.

## Naming

- Project categories use `project:<slug>` under `Projects` for committed
  active outcomes. Create one when there is a real finish line, not just an
  interesting topic.
- Area categories use `area:<slug>` under `Areas` for ongoing responsibilities
  and standards.
- Resource categories use `resource:<slug>` under `Resources` for durable
  topics, entities, concepts, assets, and promoted source collections.
- Archive categories use `archive:<slug>` under `Archives` only when inactive
  material deserves a reusable browsing surface.
- Source paths, URLs, and source-local locations belong in notes. Promote a
  source or source collection as `resource:<slug>` only when it becomes useful
  for repeated browsing.
- Keep category names stable once sources have been ingested; prefer aliases in
  views or notes for display polish.
- Do not revive the older `Source`, `Entity`, `Concept`, and `Theme` top-level
  taxonomy. Entities, concepts, and source collections usually become promoted
  children under `Resources`.

## Category Promotion

Start broad and let the category tree evolve from the data. Add a child
category only when it has at least two current items, or one current item plus
an obvious near-future recurrence. A committed active project may justify an
early `project:<slug>` child when the finish line is concrete and more related
items are likely. Otherwise, assign the item to a broader parent or an existing
general child.

Do not create categories just to preserve provenance. If an item needs source
bookkeeping, put it in the note:

```text
Source: inbox/llm-agenda/projectideas.org
Location: heading or source-local line/date
```

Promote a source resource or source-collection view only when it becomes a
recurring navigation surface, such as a long-lived source collection, a heavily
referenced book/report, or a source with enough items that browsing by source is
useful.

## Item Contract

Each substantive item should be atomic: one claim, quote, fact, observation, or
task. The title should be readable as a standalone assertion. Use the note for
context.

Minimum assignments for non-meta knowledge items:

- one PARA bucket or child, usually under `Projects`, `Areas`, or `Resources`
- one `Status` child, usually `Open`
- one `Priority` child, usually `Normal`
- optional `Signal` assignments when a review condition applies
- source provenance recorded in the note

Recommended note sections:

```text
Source: <path, URL, or promoted resource category>
Location: <heading/page/timestamp/chunk if known>
Evidence: <quote, paraphrase, or source-local citation>
Context: <why this matters and how it connects to existing items>
Confidence: high|medium|low, with reason
Related: <item ids, categories, or follow-up notes>
```

## Baseline Views

The bootstrap script creates these saved views:

- `Projects`
- `Areas`
- `Resources`
- `Archives`
- `Signal`
- `Status`
- `Priority`

It also ensures the system `All Items` view has at least one section and one
category-backed column when `sqlite3` is available.

Every baseline view should have at least one section and at least one
category-backed column. The default bootstrap column is `Priority`, because
every knowledge item should carry a priority assignment.

Do not create child-specific default views such as `Contradictions`,
`Follow-ups`, or `Needs Review`. The corresponding parent views already
exist as `Signal` and `Status`. Create a narrower view only when a real
recurring workflow needs it. Likewise, do not create source-specific,
entity-specific, or topic-specific views unless repeated use shows they deserve
space outside the PARA parent view.

For criteria-only views, do not use `--hide-unmatched`; empty-section views with
no sections can render as blank.

## Ingest Workflow

1. Pick one unprocessed file from `inbox/llm-agenda/`.
2. Read it before creating items. Discuss key takeaways with the user when the
   source is ambiguous or high-value.
3. Decide the source's PARA destination. Default answer: `Resources`, unless
   the source directly supports an active project, maintains an ongoing area, or
   is inactive enough to archive. Record source path and location in notes.
4. Create one lead synthesis item for the source when it helps review.
5. Create one item per substantive claim, quote, fact, contradiction, or useful
   task.
6. Assign status, priority, a PARA bucket/child, and any relevant signal
   categories to every item. Use a promoted resource category only when one was
   explicitly justified.
7. Record source path, review notes, and any category/view changes in the lead
   item note when they are useful for future review.
8. QA relevant top-level category views, especially the target PARA view,
   `Signal`, and representative items before moving the raw source
   to `inbox/llm-agenda/processed/`.

Optional promoted resource-category pattern:

```bash
cargo run --bin aglet -- --db ../llm-agenda.ag category create "resource:example" --parent "Resources" --disable-implicit-string
```

Lead item pattern:

```bash
lead_id=$(cargo run --bin aglet -- --db ../llm-agenda.ag add "Source ingest: 2026-05-10 example" --note "Source: inbox/llm-agenda/example.md
Location: full file
Evidence: Lead synthesis item.
Context: Key thesis, scope, and source quality.
Confidence: medium
Related: Items created in this ingest." 2>&1 | awk '/^created /{print $2; exit}')
cargo run --bin aglet -- --db ../llm-agenda.ag category assign "$lead_id" "Open"
cargo run --bin aglet -- --db ../llm-agenda.ag category assign "$lead_id" "Normal"
cargo run --bin aglet -- --db ../llm-agenda.ag category assign "$lead_id" "Resources"
```

## Query Workflow

1. Inspect the category tree first, then use existing views or create a new view
   that matches the question.
2. Read the surfaced items and cite item ids or short prefixes in the answer.
3. If the answer is reusable, persist it as one or more new items and/or a saved
   view.
4. Record unresolved gaps with `Gap` or `Follow-up` items when they should stay
   visible after the answer.

## Lint Workflow

Run a lint pass periodically or after a major ingest batch. Create durable
findings as normal items when they should remain visible after the pass.

Checks:

- items missing source provenance in notes, status, priority, or PARA
  assignments
- claims tagged `Contradiction` without a linked or referenced counterpart
- stale claims that should be `Superseded`
- high-frequency terms that deserve a new `project:<slug>`, `area:<slug>`, or
  `resource:<slug>` category
- redundant categories that should be merged or renamed
- baseline views that are missing or no longer useful
- broad implicit-string rules that are creating noisy assignments

Use `Needs Review`, `Gap`, and `Follow-up` for lint findings that should remain
visible after the lint pass.

## Automation Policy

- Baseline categories are created with implicit string matching disabled.
- Only enable implicit matching for specific child categories whose names are
  distinctive enough to avoid matching prose examples or acceptance criteria.
- Prefer profile conditions and actions for rules that reflect deliberate
  schema policy.
- Document every non-obvious condition/action in the owning category note.
- Re-run a small QA list after adding automation to check for false positives.

## Conditions And Actions Rollout

### Phase A: Conservative Automation

- Enable limited implicit matches for well-bounded project, area, or resource
  child names.
- Keep manual review gate for new category creation.
- Avoid broad substring rules that can overclassify notes.

### Phase B: Targeted Expansion

- Add condition/action rules for high-confidence repetitive patterns.
- Add contradiction or follow-up auto-tagging for defined triggers.
- Tune precedence in exclusive families via child order.

### Phase C: Maintenance Hardening

- Periodically evaluate false positives and false negatives of rule-derived
  assignments.
- Prune or adjust rules that drift or become noisy.

## Implementation Decisions

### Phase 0 Foundation

Ship the first LLM Agenda implementation as a documented operating contract
plus a reusable Bash bootstrap script:

- `docs/plans/llm-agenda-implementation.md`
- `scripts/init-llm-agenda-db.sh`

Do not add a dedicated Rust CLI subcommand yet. New users can bootstrap a
working LLM Agenda database with one command, and future work can promote
stable workflows into the Rust CLI if repeated manual use shows the script is
too limited.

### Top-Level Category Views

Every top-level LLM Agenda category has a dedicated saved view: `Projects`,
`Areas`, `Resources`, `Archives`, `Signal`, `Status`, and `Priority`.

Do not create child-specific default views or source-specific views. Those
categories are visible through their parent views. Create narrower views only
when a real recurring workflow needs them.

### Category Promotion And Provenance

LLM Agenda categories are reusable navigation structure, not provenance
bookkeeping.

Do not create a category or view for every source ingest by default. Store
source path, URL, and source-local location in item notes. Promote a source or
source collection under `Resources`, or create a resource/source-collection
view, only when it becomes a recurring browsing surface.

### Repo-Local Skill

The canonical `llm-agenda` skill source lives in this repository at
`skills/llm-agenda/`.

The skill depends on repo-specific process docs, bootstrap scripts, and
evolving decisions. Keeping the skill beside those files makes the operating
contract reviewable, versioned, and easier to keep synchronized with this plan.

## Execution Phases

### Phase 0: Foundation

Status: implemented.

Deliverables:

- Initial schema instructions for agent behavior.
- Baseline category tree with exclusive families defined.
- Baseline top-level category views.
- Repo-local skill.

Exit criteria:

- A new user can ingest one source end-to-end with consistent outputs.

### Phase 1: Supervised Ingest Loop

Deliverables:

- Repeatable ingest checklist.
- Provenance and citation conventions exercised in real items.
- First 10-20 sources ingested with review notes.

Exit criteria:

- Ingest quality is stable with minimal rework per source.

### Phase 2: Query Compounding

Deliverables:

- Reusable query views for key projects, areas, and resources.
- Query answers consistently persisted as reusable artifacts.

Exit criteria:

- Repeated questions become faster due to existing curated structure.

### Phase 3: Lint And Scaling Discipline

Deliverables:

- Periodic lint cadence and checklist.
- Merge/split guidelines for taxonomy evolution.
- Lightweight optional CLI helpers for repetitive checks.

Exit criteria:

- Database health remains high as source count grows.

## Risks And Mitigations

1. Taxonomy sprawl or synonym drift:
   - Enforce naming conventions and periodic merge reviews.
2. Over-aggressive auto-classification:
   - Start narrow; keep review queue; prefer explicit conditions.
3. Contradiction blind spots:
   - Keep contradiction tagging visible in the `Signal` view and create a
     narrower contradiction view only when review volume justifies it.
4. User trust erosion from opaque edits:
   - Keep source provenance and review notes in durable item notes.
5. Workflow inconsistency across sessions/agents:
   - Keep this operating contract updated and versioned.

## Metrics

Track per week:

- Sources ingested.
- Items created.
- Average category assignments per item.
- Orphan rate: percent of items missing required assignment set.
- Mutex violation count.
- Query reuse rate: answers leveraging existing views/items.
- Lint issue closure rate.

## Immediate Next Actions

1. Run a pilot ingest on three representative sources.
2. Run the first lint pass and refine taxonomy/rules.
3. Document pilot findings and promote proven conventions into this contract.
