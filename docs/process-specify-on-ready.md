# Specify-on-Ready Workflow

When an issue becomes unblocked ("ready"), a reviewer specifies it with
implementer-level detail before an agent claims it. This bridges the gap
between the lightweight issue descriptions created during phase planning and
the context an implementer actually needs to do good work.

---

## When this runs

After closing an issue:

```bash
br close <id>
br sync --flush-only && git add .beads/ && git commit -m "br sync: Close <id>"

# Check what's newly ready
br ready
```

For each newly ready issue, the reviewer writes a spec prompt before any agent
claims it.

## What the reviewer does

1. **Read the issue** — `br show <id>` for the current description.

2. **Read the codebase** — Look at the files the implementer will touch and
   the files they'll depend on. Understand the current state, not just what
   the spec says should exist.

3. **Read the spec** — Find the relevant sections of `archive/spec-legacy/mvp-spec.md`. Note any
   gaps between spec intent and current code.

4. **Write the prompt** — Create `docs/process-prompt-<task-id>-<short-name>.md`
   with the sections below.

5. **Commit** — `git add docs/process-prompt-*.md && git commit -m "docs: Add prompt for <task-id>"`

## Prompt structure

Each prompt should have these sections. The goal is context, not prescription.
Tell the implementer what to achieve and what to watch out for — not how to
write the code.

### Context
2-3 sentences. What is this component? Why does it exist? How does it fit into
the system? Write this for someone who hasn't read the full spec.

### What to read
Ordered list of files and spec sections the implementer should read before
starting. Include section numbers or line ranges. Prioritize: most important
first.

### What to build
Describe the behavior, not the implementation. Include:
- **Behavioral rules** — what should happen in each scenario
- **Edge cases** — things that might be missed
- **Key design decisions** — things that came out of spec/model analysis
  (e.g., "check the bool flag, not the conditions vec")

Do NOT include:
- Exact function signatures or struct layouts
- Internal algorithm choices
- Step-by-step implementation instructions

The implementer designs the API. The spec describes what it should do.

### Tests to write
Concrete input/output examples as acceptance criteria. These are behavioral
tests — "given X, expect Y" — not implementation tests.

### What NOT to do
Scope fencing. What belongs to other tasks. What to defer. Common pitfalls
that would waste time.

### How your code will be used
Show how downstream code (the next task in the chain) will consume this work.
This helps the implementer design the right public API without being told
what it should look like.

### Workflow
Standard: point to `AGENTS.md`, include the `br` issue ID, remind about
branch naming.

### Definition of done
Checklist: tests pass, clippy clean, files touched are within scope.

## What this is NOT

- **Not a code review** — the reviewer writes specs, not code.
- **Not a rewrite of the issue** — the original `br` description stays. The
  prompt is a companion document that adds context.
- **Not required for trivial issues** — if an issue is self-explanatory and
  touches one function, skip the prompt.
- **Not prescriptive about internals** — the prompt describes the "what" and
  "why", never the "how". The implementer chooses the approach.

## Naming convention

`docs/process-prompt-<task-ids>-<short-name>.md`

Examples:
- `docs/process-prompt-t015-t016-classifier.md` (merged tasks)
- `docs/process-prompt-t017-rule-engine.md`
- `docs/process-prompt-t018-t019-t020-engine-features.md` (parallel tasks sharing context)

Tasks that can be specified together (same file, tight coupling) can share a
prompt. Tasks that are independent get separate prompts.
