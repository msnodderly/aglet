---
name: llm-agenda
description: Maintain this repo's Aglet-backed LLM Agenda knowledge database using PARA: Projects, Areas, Resources, and Archives. Use when Codex needs to bootstrap an LLM Agenda database, ingest sources into durable items, answer questions through saved views, persist synthesis, lint taxonomy/data quality, or evolve the repo-local LLM Agenda schema. This skill is homed in the repo, not globally.
---

# LLM Agenda

This is the canonical skill for Aglet's LLM Agenda work. It is homed in this
repo at `skills/llm-agenda/`, not in `$HOME/.codex/skills/llm-agenda`.

Use Aglet as a persistent, compounding knowledge database maintained by an LLM.
Raw sources stay immutable; the agent creates and maintains items, categories,
and views.

## Principle

Organize by actionability, following PARA:

- `Projects`: active short-term outcomes with a clear finish line.
- `Areas`: ongoing responsibilities, standards, or relationships.
- `Resources`: reference material, topics, entities, concepts, assets, and
  source collections that may be useful later.
- `Archives`: inactive, completed, paused, or no-longer-relevant material.

Default to `Resources` for source-derived knowledge unless the source directly
serves an active project, maintains an ongoing area, or belongs in archive.
Do not revive the older `Source` / `Entity` / `Concept` / `Theme` top-level
model. Entities, concepts, and source collections are usually promoted children
under `Resources`.

## Start Here

1. Read `docs/plans/llm-agenda-implementation.md`; it is the local source of
   truth for schema, naming, ingest, query, lint, and view policy.
   If this skill is ever inspected outside the repo, use
   `references/schema.md` only as a portable summary.
2. Bootstrap with `./scripts/init-llm-agenda-db.sh ../llm-agenda.ag inbox/llm-agenda`
   when the database does not exist or needs baseline repair.
3. Keep source paths, URLs, source-local locations, and ingest details in item
   notes unless a source collection has been deliberately promoted under
   `Resources`.
4. Keep categories sparse. Add child categories only when they support repeated
   navigation or an active committed outcome.

## Ingest Workflow

1. Pick one unprocessed source from `inbox/llm-agenda/`.
2. Read the source before writing items.
3. Decide the source's PARA destination. Use `Resources` by default.
4. Record source path/URL and source-local location in item notes.
5. Create a lead synthesis item only when it helps review.
6. Create one item per substantive claim, quote, fact, project, or task.
7. Assign a PARA bucket or promoted child, `Status`, `Priority`, and any
   relevant `Signal` categories.
8. Record review notes and category/view changes in the lead item when useful.
9. QA the target PARA view, `Signal`, and representative items before marking
   the source processed.

## Query Workflow

1. Inspect the category tree and existing views first.
2. Use or create the smallest durable view that answers the question.
3. Cite supporting item ids or short prefixes.
4. Persist reusable answers as new items, categories, or views.
5. Persist unresolved gaps as `Gap` or `Follow-up` items when they should stay
   visible.

## Lint Workflow

Look for missing source provenance, missing status/priority/PARA assignments,
noisy categories, over-specific one-off categories, stale claims,
contradictions without counterparts, terms that now deserve promotion, and
missing or stale baseline views. Use `Needs Review`, `Gap`, or `Follow-up` when
the finding should stay visible.

## Core Rules

- PARA buckets are the primary navigation structure.
- `Status`, `Priority`, and `Signal` are cross-cutting metadata families.
- Categories are reusable navigation structure, not provenance.
- Source path, URL, section, page, timestamp, and ingest details belong in notes
  unless a source collection has been deliberately promoted.
- Start general and split later.
- Use `Completed`, not reserved `Done`, for finished workflow status.
- Keep broad bootstrap categories with implicit string matching disabled.
- Ensure each baseline view has at least one section and one category-backed
  column. Use `Priority` as the default column when there is no stronger local
  choice.
- For criteria-only views, do not use `--hide-unmatched`; empty-section views
  with no sections can render as blank.
