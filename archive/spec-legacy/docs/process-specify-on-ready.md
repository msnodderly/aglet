# Specify-on-Ready Workflow

When an issue becomes unblocked (`br ready`), a reviewer writes an implementer
prompt before any agent claims it. The prompt should reflect current code and
current roadmap, not legacy assumptions.

---

## When this runs

After closing an issue on `main`:

```bash
br close <id>
# (legacy beads reference removed) -m "br sync: Close <id>"

# Scheduling source of truth
br ready
```

For each newly ready issue, write or update a prompt before implementation work
is claimed.

## Reviewer workflow

1. **Read scheduling output** — run `br ready` and treat it as source of truth.

2. **Read each ready issue** — `br show <id>`.

3. **Read current product context** — use current docs first:
   - `docs/specs/product/target.md`
   - `docs/specs/product/roadmap.md`
   - `docs/specs/product/gaps.md`
   - `docs/specs/product/tasks.md`
   - `docs/specs/product/target.md` (relevant scenario sections)

4. **Read relevant code/tests** — inspect files the implementer is expected to
   touch plus neighboring tests and integration paths.

5. **Write prompt file** — create/update:
   - `docs/process-prompt-<issue-id>-<short-name>.md`

6. **Commit prompt docs** —
   `git add docs/process-prompt-*.md && git commit -m "docs: add/update prompt for <issue-id>"`

## Prompt structure

Each prompt should include:

- Context
- What to read
- What to build
- Tests to write
- What NOT to do
- How your code will be used
- Workflow
- Definition of done

Quality bar: match the specificity and practical guidance level in strong
existing prompts (for example `docs/process-prompt-t019-subsumption.md`).

## Prompt guidance

### What to read

List code + docs in priority order. Include enough path-level detail that an
implementer can start immediately.

### What to build

Describe behavior and invariants, not exact internals.

Include:

- required behavior
- edge cases
- known constraints from current model/roadmap

Do not include:

- exact function signatures
- prescribed algorithm internals
- step-by-step code instructions

### Tests to write

Use explicit behavioral acceptance examples (`given X, expect Y`).

### What NOT to do

Fence scope to prevent accidental spillover into deferred roadmap areas.

## Scope rules

- Create/update prompts only for issues currently shown by `br ready`.
- Do not run `br` write commands during this review pass
  (`update`, `close`, `create`, `sync`, comments).
- Do not edit unrelated files.
- If no ready issues exist, report that and make no file changes.

## Naming convention

`docs/process-prompt-<issue-id>-<short-name>.md`

Examples:

- `docs/process-prompt-bd-3fl-next-weekday-policy.md`
- `docs/process-prompt-t017-rule-engine.md`
- `docs/process-prompt-t018-t019-engine-features.md`

Tightly-coupled ready issues may share one prompt file; independent ready issues
should get separate prompt files.
