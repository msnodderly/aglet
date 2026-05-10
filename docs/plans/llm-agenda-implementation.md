---
title: LLM Agenda implementation plan
status: draft
created: 2026-05-10
updated: 2026-05-10
---

## Goal

Implement an "LLM Agenda" operating model in aglet where an LLM agent incrementally builds and maintains a persistent knowledge database from curated sources, rather than relying on stateless per-query retrieval.

## Success criteria

- A reproducible ingest workflow exists for adding a source and producing structured items, categories, and view updates.
- A documented schema contract exists for agent behavior (naming, category families, provenance, lint rules, and review workflow).
- Query workflows can produce answer artifacts that are persisted back into the database (items/views/categories).
- A periodic lint workflow exists for database health checks and maintenance suggestions.
- The workflow scales to at least a moderate local corpus (target: ~100 sources / low-thousands of items) without external embedding infrastructure.

## Scope

### In scope

1. **Schema and operating conventions**
   - Define category taxonomy patterns (source/entity/concept/theme/status/priority/operations).
   - Define exclusive vs non-exclusive families and assignment rules.
   - Define naming, note-format, and provenance conventions.

2. **Core operations**
   - Ingest (source -> items/categories/views/meta-log).
   - Query (view-driven answer synthesis with item-level citations).
   - Lint (health checks + recommended repairs).

3. **Indexing and observability artifacts**
   - Category manager hygiene conventions.
   - Operational log view for ingest/query/lint activity.

4. **Optional CLI accelerators**
   - Light scripts/helpers only after manual workflow is stable.

### Out of scope (initial release)

- Building an external vector index / RAG service.
- Fully autonomous unattended ingest of all sources.
- Hard backward-compatibility guarantees for early schema iterations.


## Practical starting point (agent runbook)

### Directory and files

- **Ingest inbox (raw immutable sources):** `inbox/llm-agenda/`
- **Processed archive marker (optional):** `inbox/llm-agenda/processed/`
- **Working database file:** `llm-agenda.ag`
- **Schema/agent contract doc:** `docs/process/llm-agenda-agent-schema.md` (create in Phase 0)

### One-time bootstrap commands

```bash
# from repo root
mkdir -p inbox/llm-agenda/processed

# initialize a dedicated local DB for this workflow
cargo run --bin aglet -- --db llm-agenda.ag category list 2>&1 | head -20
```

If the DB is empty, create the baseline families (example):

```bash
cargo run --bin aglet -- --db llm-agenda.ag category create "Source"
cargo run --bin aglet -- --db llm-agenda.ag category create "Entity"
cargo run --bin aglet -- --db llm-agenda.ag category create "Concept"
cargo run --bin aglet -- --db llm-agenda.ag category create "Operation"
cargo run --bin aglet -- --db llm-agenda.ag category create "Status" --exclusive
cargo run --bin aglet -- --db llm-agenda.ag category create "Priority" --exclusive
cargo run --bin aglet -- --db llm-agenda.ag category create "Signal"
```

### Per-source ingest procedure (specific agent actions)

1. **Pick the next file from inbox**  
   Agent checks `inbox/llm-agenda/` and selects one unprocessed source (e.g. `2026-05-10-openai-agents-notes.md`).

2. **Create/resolve source category**  
   Agent normalizes a source slug (`src:2026-05-10-openai-agents-notes`) and ensures category exists:

```bash
cargo run --bin aglet -- --db llm-agenda.ag category create "src:2026-05-10-openai-agents-notes" --parent "Source" 2>&1 | tail -1
```

3. **Create lead synthesis item and capture id**

```bash
lead_id=$(cargo run --bin aglet -- --db llm-agenda.ag add "Source ingest: 2026-05-10 openai agents notes" --note "Lead summary for source; include key thesis, context, and confidence notes." 2>&1 | awk '/^created /{print $2; exit}')
```

4. **Assign required baseline categories to lead item**

```bash
cargo run --bin aglet -- --db llm-agenda.ag category assign "$lead_id" "src:2026-05-10-openai-agents-notes"
cargo run --bin aglet -- --db llm-agenda.ag category assign "$lead_id" "Ingest" 2>&1 | tail -1
cargo run --bin aglet -- --db llm-agenda.ag category assign "$lead_id" "Open" 2>&1 | tail -1
cargo run --bin aglet -- --db llm-agenda.ag category assign "$lead_id" "Normal" 2>&1 | tail -1
```

5. **Create atomic claim items (10-30 typical)**  
   For each substantive claim extracted by the agent, run `add`, then assign source + entity + concept + theme categories.

6. **Create operation log meta-item**

```bash
op_id=$(cargo run --bin aglet -- --db llm-agenda.ag add "Ingest run for 2026-05-10-openai-agents-notes" --note "Operation=Ingest; Source=src:2026-05-10-openai-agents-notes; ItemsCreated=<N>; Notes=<lint/review summary>." 2>&1 | awk '/^created /{print $2; exit}')
cargo run --bin aglet -- --db llm-agenda.ag category assign "$op_id" "Ingest"
cargo run --bin aglet -- --db llm-agenda.ag category assign "$op_id" "src:2026-05-10-openai-agents-notes"
```

7. **Run immediate QA checks**

```bash
cargo run --bin aglet -- --db llm-agenda.ag list --category "src:2026-05-10-openai-agents-notes" 2>&1 | head -40
cargo run --bin aglet -- --db llm-agenda.ag list --category "Ingest" --any-category "Contradiction" --any-category "Gap" 2>&1 | head -40
cargo run --bin aglet -- --db llm-agenda.ag show "$lead_id" 2>&1 | head -80
```

8. **Mark source as processed**  
   Move the raw file to `inbox/llm-agenda/processed/` (or append `.done`) so it is not ingested twice.

### First views to create (practical minimum)

```bash
cargo run --bin aglet -- --db llm-agenda.ag view create "Ingest Log" --include "Ingest"
cargo run --bin aglet -- --db llm-agenda.ag view create "Contradictions" --include "Contradiction"
cargo run --bin aglet -- --db llm-agenda.ag view create "Source: 2026-05-10 openai agents notes" --include "src:2026-05-10-openai-agents-notes"
```


## Proposed architecture in aglet terms

1. **Raw source layer (immutable)**
   - Sources are stored outside aglet item state and treated as authoritative input.
   - Each source gets stable metadata fields (id, type, origin URL/path, ingest date, checksum/version marker).

2. **Agenda database layer (mutable, persistent)**
   - **Items**: one substantive claim/quote/fact/task per item.
   - **Categories**: overlapping hierarchy for source/entity/concept/theme; exclusive families for status/priority/stage.
   - **Views**: saved lenses for source summaries, entities, concepts, contradictions, timelines, orphans, and recent changes.

3. **Agent schema layer (governance contract)**
   - Agent instructions file(s) describing ingestion protocol, assignment policy, conflict policy, and lint obligations.
   - Includes explicit criteria for when to create categories, when to merge, and when to mark parent categories exclusive.

## Data model plan

### Category taxonomy baseline

- `Source` (non-exclusive parent)
  - children per document/source collection.
- `Entity` (non-exclusive parent)
  - people, organizations, products, places.
- `Concept` (non-exclusive parent)
  - theories, frameworks, themes.
- `Operation` (non-exclusive parent)
  - `Ingest`, `Query`, `Lint`, `Maintenance`.
- `Status` (exclusive parent)
  - `Open`, `In Progress`, `Done`, `Superseded`, `Needs Review`.
- `Priority` (exclusive parent)
  - `Critical`, `High`, `Normal`, `Low`.
- `Signal` (non-exclusive parent)
  - `Contradiction`, `Gap`, `Follow-up`, `Hypothesis`.

### Item conventions

- Keep title text atomic and assertion-oriented.
- Put richer synthesis/quotes/context in note body.
- Include dates in natural language where useful to leverage date parsing.
- Prefer one lead synthesis item per source ingest, linked by shared source/category assignment with atomic sub-items.

### Provenance conventions

Each item should preserve:
- Source identity and section/chunk location where possible.
- Ingest operation id (log reference).
- Assignment source explanation (manual, auto, action, etc.) for debugging/repair.

## View strategy plan

Create and maintain the following baseline views:

1. **All Items (system)**
   - sanity/default exploration.
2. **Ingest Log**
   - includes `Operation:Ingest` meta-items, sectioned by `When`.
3. **Query Log**
   - includes `Operation:Query` outputs and reusable answer artifacts.
4. **Lint Log**
   - includes `Operation:Lint` findings and resolutions.
5. **Source Summary: <source>**
   - per-source rollups.
6. **Entity: <entity>**
   - people/org/project-specific slices.
7. **Concept: <concept>**
   - thematic aggregation.
8. **Contradictions**
   - items tagged with conflicting interpretations/status markers.
9. **Orphans / Under-classified**
   - items missing minimum assignment coverage.
10. **Recent Changes**
   - recent ingest/query/lint activity for quick review.

## Workflow design

### 1) Ingest workflow

1. Register source metadata and create/resolve source category.
2. Read source with the user and extract substantive claims.
3. Create atomic items and one lead synthesis item.
4. Assign source/entity/concept/theme categories.
5. Apply status/priority defaults under exclusive families.
6. Create an `Operation:Ingest` meta-item with timestamp, counts, and notes.
7. Validate for obvious mutex violations and orphaned records.
8. Review results with user in source and summary views.

### 2) Query workflow

1. Identify relevant categories from category tree and existing views.
2. Open target view(s), inspect surfaced items, and synthesize answer.
3. Produce item-level citations to supporting records.
4. Persist valuable outputs back as:
   - new items (insights/conclusions),
   - new/updated views,
   - new categories for recurring patterns.
5. Create an `Operation:Query` meta-item summarizing what was asked and filed.

### 3) Lint workflow (scheduled or manual)

Checks:
- Exclusive family conflicts/mutex anomalies.
- Stale/superseded claims lacking explicit status updates.
- Orphan/under-tagged items.
- High-frequency terms lacking category coverage.
- Redundant categories (merge candidates).
- Critical views missing expected criteria/columns/sections.

Output:
- `Operation:Lint` meta-item plus repair suggestions.
- Optional queued maintenance tasks categorized under `Operation:Maintenance`.

## Conditions and actions rollout

### Phase A: conservative automation

- Enable limited implicit matches for well-bounded entity/source names.
- Keep manual review gate for new category creation.
- Avoid broad substring rules that can overclassify notes.

### Phase B: targeted expansion

- Add condition/action rules for high-confidence repetitive patterns.
- Add contradiction or follow-up auto-tagging for defined triggers.
- Tune precedence in exclusive families via child order.

### Phase C: maintenance hardening

- Periodically evaluate false positives/false negatives of rule-derived assignments.
- Prune/adjust rules that drift or become noisy.

## Execution phases and deliverables

### Phase 0 — foundation (1-2 sessions)

Deliverables:
- Initial schema instructions draft for agent behavior.
- Baseline category tree with exclusive families defined.
- Baseline operational views (ingest/query/lint log + orphans/contradictions).

Exit criteria:
- A new user can ingest one source end-to-end with consistent outputs.

### Phase 1 — supervised ingest loop (3-5 sessions)

Deliverables:
- Repeatable ingest checklist.
- Provenance and citation conventions documented.
- First 10-20 sources ingested with review notes.

Exit criteria:
- Ingest quality is stable (minimal rework per source).

### Phase 2 — query compounding (ongoing)

Deliverables:
- Reusable query views for key entities/concepts.
- Query answers consistently persisted as reusable artifacts.

Exit criteria:
- Repeated questions become faster due to existing curated structure.

### Phase 3 — lint and scaling discipline (ongoing)

Deliverables:
- Periodic lint cadence and checklist.
- Merge/split guidelines for taxonomy evolution.
- Lightweight optional CLI helpers for repetitive checks.

Exit criteria:
- Database health remains high as source count grows.

## Risks and mitigations

1. **Taxonomy sprawl / synonym drift**
   - Mitigation: enforce naming conventions and periodic merge reviews.
2. **Over-aggressive auto-classification**
   - Mitigation: start narrow; keep review queue; prefer explicit conditions.
3. **Contradiction blind spots**
   - Mitigation: dedicate contradiction views and lint checks.
4. **User trust erosion from opaque edits**
   - Mitigation: provenance logging + operation meta-items for every significant run.
5. **Workflow inconsistency across sessions/agents**
   - Mitigation: keep schema instructions updated and versioned.

## Metrics

Track per week:
- Sources ingested.
- Items created.
- Average category assignments per item.
- Orphan rate (% items missing required assignment set).
- Mutex violation count.
- Query reuse rate (answers leveraging existing views/items).
- Lint issue closure rate.

## Immediate next actions

1. Draft/update the agent schema file with this workflow contract.
2. Instantiate baseline categories and views in a fresh/agreed database.
3. Run a pilot ingest on 3 representative sources.
4. Run first lint pass and refine taxonomy/rules.
5. Document pilot findings and promote proven conventions into schema.
