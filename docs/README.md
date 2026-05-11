# Aglet Documentation Index

Entry point for all project documentation. Every doc file has YAML frontmatter
with at minimum `title` and `updated` (or `created`/`status` for plans and
proposals).

## Directory Taxonomy

| Directory | Purpose |
|---|---|
| `plans/` | Implementation plans. `status: draft \| active \| shipped \| abandoned` |
| `decisions/` | Decision records: accepted proposals + legacy decision logs |
| `specs/product/` | Product specification (NLSpec), roadmap, gaps, tasks |
| `specs/tui/` | TUI-specific specs |
| `specs/proposals/` | Design proposals. `status: draft \| rejected \| deferred` (accepted ones move to `decisions/`) |
| `reference/` | Durable reference docs (codebase walkthrough, comparisons) |
| `process/` | PM workflow, agent workflow |
| `demos/` | Executable demos |
| `agents/handoff/` | Session handoff logs (`YYYY-MM-DD-NNN-feature.md`) |
| `backlog/` | Feature requests |
| `images/` | Screenshot assets referenced by the root README and docs |

## Frontmatter Conventions

**Plans** (`plans/*.md`):
```yaml
---
title: Feature Name
status: draft | active | shipped | abandoned
created: YYYY-MM-DD
shipped: YYYY-MM-DD  # added when status -> shipped
---
```

**Proposals** (`specs/proposals/*.md`):
```yaml
---
title: Proposal Name
status: draft | accepted | rejected | deferred
created: YYYY-MM-DD
decided: YYYY-MM-DD  # date of decision
---
```

**Decision records** (`decisions/*.md`) -- accepted proposals land here:
```yaml
---
title: Decision Name
status: accepted
created: YYYY-MM-DD
decided: YYYY-MM-DD
origin: specs/proposals/original-filename.md
---
```

**All other docs**:
```yaml
---
title: Doc Title
updated: YYYY-MM-DD
---
```

## Lifecycle Rules

- **Plans** stay in `plans/` permanently. Status is updated in place.
- **Proposals** that are accepted move to `decisions/` with an `origin:` breadcrumb.
  Rejected/deferred proposals stay in `specs/proposals/`.
- **Archive** (`../archive/`) is frozen pre-v0.6 material. No new files go there.

## Handoff Doc-Update Step

Every agent session must, before writing the handoff doc:
1. Update `status` of any plan touched (add `shipped:` date if shipped)
2. Write a decision record for any non-trivial design choice
3. Move accepted proposals to `decisions/` with updated status

## Archive

`../archive/` holds frozen historical content from before the v0.6 reorg:
- `spec-legacy/` -- pre-v0.6 specification suite (flat prefix-based naming)
- `source-material/` -- Lotus Agenda research extracts
- `notes/` -- historical Q&A
- `plans-superseded/` -- explicitly superseded plans
