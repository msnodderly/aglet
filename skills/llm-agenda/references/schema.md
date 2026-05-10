# LLM Agenda Schema Reference

This is a portable summary of the repo-local contract. Inside the Aglet repo,
prefer `docs/plans/llm-agenda-implementation.md` as the source of truth.

## Defaults

- Inbox: `inbox/llm-agenda/`
- Processed marker/archive: `inbox/llm-agenda/processed/`
- Default DB: `../llm-agenda.ag`
- Bootstrap:

```bash
./scripts/init-llm-agenda-db.sh ../llm-agenda.ag inbox/llm-agenda
```

## Primary Organization: PARA

Use PARA as the primary navigation model:

- `Projects`: active short-term outcomes with a clear finish line.
- `Areas`: ongoing responsibilities and standards requiring continuing
  attention.
- `Resources`: reference material, topics, assets, entities, concepts, and
  source collections.
- `Archives`: inactive, completed, paused, or no-longer-relevant material
  retained for future reference.

Default source-derived knowledge to `Resources` unless it directly supports an
active project, maintains an ongoing area, or should be archived. Do not use the
older `Source` / `Entity` / `Concept` / `Theme` top-level model.

## Metadata Families

- `Status`: `Open`, `In Progress`, `Completed`, `Superseded`, `Needs Review`
- `Priority`: `Critical`, `High`, `Normal`, `Low`
- `Signal`: `Contradiction`, `Gap`, `Follow-up`, `Hypothesis`

`Needs Review` is a status for items whose wording, interpretation, evidence, or
classification needs review. `Gap` is a signal for missing evidence or context.
`Contradiction` is a signal for a specific competing claim or source.

Use `Completed`, not `Done`, for the status category. `Done` is reserved by
Aglet.

## Category Promotion

Add a child category only when it has at least two current items, or one current
item plus obvious near-future recurrence. A committed active project can justify
an early `project:<slug>` child when the finish line is concrete.

Naming:

- `project:<slug>` under `Projects`
- `area:<slug>` under `Areas`
- `resource:<slug>` under `Resources`
- `archive:<slug>` under `Archives`

Do not create categories for provenance. Use note fields:

```text
Source: <path, URL, or promoted resource category>
Location: <heading/page/timestamp/chunk if known>
Evidence: <quote, paraphrase, or source-local citation>
Context: <why this matters and how it connects to existing items>
Confidence: high|medium|low, with reason
Related: <item ids, categories, or follow-up notes>
```

Promote source resources/views only when useful for repeated browsing, usually
as `resource:<slug>` under `Resources`.

## Minimum Item Assignments

For non-meta knowledge items:

- one PARA bucket or child, usually under `Projects`, `Areas`, or `Resources`
- one `Status` child, usually `Open`
- one `Priority` child, usually `Normal`
- optional `Signal` assignments when a review condition applies
- source provenance in the note

## Useful Commands

Create a promoted resource:

```bash
cargo run --bin aglet -- --db ../llm-agenda.ag category create "resource:example" --parent "Resources" --disable-implicit-string
```

Add an item:

```bash
cargo run --bin aglet -- --db ../llm-agenda.ag add "Item title" --note "Source: inbox/llm-agenda/example.md
Location: heading
Evidence: ...
Context: ...
Confidence: medium
Related: ..." --category "Open" --category "Normal" --category "Resources"
```

QA:

```bash
cargo run --bin aglet -- --db ../llm-agenda.ag view show "Resources"
cargo run --bin aglet -- --db ../llm-agenda.ag view show "Signal"
cargo run --bin aglet -- --db ../llm-agenda.ag category list
```

## View Shape

Every baseline top-level category should have a dedicated saved view:
`Projects`, `Areas`, `Resources`, `Archives`, `Signal`, `Status`, and
`Priority`.

Every view should have at least one section and at least one category-backed
column. Use `Priority` as the default column unless the local schema has a more
useful always-present category column.
