# Reusable Request Prompt: Generate Flywheel Task Prompts

Use this prompt when you want the agent to run the full specify-on-ready process and produce implementer prompts only for currently unblocked work.

```text
Act as the flywheel "specify-on-ready" reviewer.

1) Read `AGENTS.md` and `docs/process-specify-on-ready.md` first.
2) Read `archive/spec-legacy/phase5-overview.md`, `archive/spec-legacy/mvp-spec.md`, and `archive/spec-legacy/mvp-tasks.md`.
3) Run `br ready` and treat it as the scheduling source of truth.
4) For each currently ready issue only:
   - Run `br show <id>`
   - Read the relevant current code files the implementer is expected to touch
   - Create or update exactly one prompt file named `docs/process-prompt-t<task>-<short-name>.md`
5) Prompt quality bar: match `docs/process-prompt-t019-subsumption.md` for structure and specificity.
6) Each prompt must include these sections:
   - Context
   - What to read
   - What to build
   - Tests to write
   - What NOT to do
   - How your code will be used
   - Workflow
   - Definition of done
7) Scope rules:
   - Do not create prompts for tasks that are not currently `br ready`
   - Do not run any `br` write commands (`update`, `close`, `create`, `sync`, comments)
   - Do not edit unrelated files
8) If no ready tasks exist, report that and make no file changes.
9) At the end, report:
   - Ready issue IDs found
   - Prompt files created/updated
   - Any skipped items and why.
```

## Short Variant

```text
Run specify-on-ready: read `AGENTS.md`, `docs/process-specify-on-ready.md`, `archive/spec-legacy/phase5-overview.md`, `archive/spec-legacy/mvp-spec.md`, `archive/spec-legacy/mvp-tasks.md`; then run `br ready` and create/update prompts only for those ready issues in `docs/process-prompt-*.md` (one file per task, `docs/process-prompt-t<task>-<short-name>.md`), matching `docs/process-prompt-t019-subsumption.md` specificity and section structure. No `br` write commands. End with a report of ready IDs and files changed.
```
