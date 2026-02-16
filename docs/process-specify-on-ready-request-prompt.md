# Reusable Request Prompt: Generate Specify-on-Ready Prompts

Use this prompt when you want an agent to run the full specify-on-ready review
and produce implementer prompts only for currently unblocked work.

```text
Act as the specify-on-ready reviewer.

1) Read `AGENTS.md` and `docs/process-specify-on-ready.md` first.
2) Read current product docs: `spec/product-current.md`, `spec/roadmap-current.md`, `spec/gaps.md`, `spec/tasks.md`, and relevant sections of `spec/product-spec-complete.md`.
3) Run `br ready` and treat it as the scheduling source of truth.
4) For each currently ready issue only:
   - Run `br show <id>`
   - Read the relevant current code and tests the implementer is expected to touch
   - Create or update exactly one prompt file named `docs/process-prompt-<issue-id>-<short-name>.md`
5) Prompt quality bar: match the structure/specificity of high-quality existing prompts (for example `docs/process-prompt-t019-subsumption.md`).
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
   - Do not create prompts for issues not currently in `br ready`
   - Do not run any `br` write commands (`update`, `close`, `create`, `sync`, comments)
   - Do not edit unrelated files
8) If no ready issues exist, report that and make no file changes.
9) At the end, report:
   - Ready issue IDs found
   - Prompt files created/updated
   - Any skipped items and why.
```

## Short Variant

```text
Run specify-on-ready: read `AGENTS.md`, `docs/process-specify-on-ready.md`, `spec/product-current.md`, `spec/roadmap-current.md`, `spec/gaps.md`, `spec/tasks.md`, and relevant sections of `spec/product-spec-complete.md`; run `br ready`; then create/update prompts only for those ready issues in `docs/process-prompt-*.md` (one per issue unless tightly coupled), using the filename pattern `docs/process-prompt-<issue-id>-<short-name>.md`. No `br` write commands. End with a report of ready IDs and files changed.
```
