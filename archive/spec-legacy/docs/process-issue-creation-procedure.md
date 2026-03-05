# Issue Creation Procedure

How to translate a phase from `mvp-tasks.md` into `br` issues ready for
implementation. This is a thinking process, not just mechanical transcription.

---

## 1. Pre-review: Understand the phase intent

Before creating any issues, read the phase as a whole and answer:

- **What is the "aha moment"?** What does the user experience when this phase
  is done? (e.g., Phase 3: "create a category named Sarah → all items
  containing Sarah get auto-assigned")
- **What is the checkpoint?** The paragraph at the end of each phase describes
  what should be demonstrably working. This is the acceptance test for the
  entire phase.
- **What does this phase need from prior phases?** Trace the data flow. Which
  store methods, model types, or traits does this phase consume? Verify they
  exist and are sufficient.
- **What does the next phase need from this one?** Look ahead. Will Phase 4
  assume something this phase should have established?

## 2. Trace the execution path

Walk through the spec's processing model step by step, as if you were the
code. For Phase 3 this means:

1. Item created → enters processing queue
2. Engine walks category hierarchy depth-first
3. For each category → evaluate conditions
4. Match → assign → fire actions
5. Actions may modify → re-queue
6. Fixed-point → stop

At each step, ask:
- **What code exists for this?** (from prior phases)
- **What code needs to be written?** (this phase's tasks)
- **What's ambiguous?** (spec says one thing, model implies another)

Document any ambiguities you find — these become clarifications in task
descriptions or questions to resolve before starting.

## 3. Identify gaps between spec and model

The spec is the intent. The model/store is the implementation. They may
have diverged. Common gaps:

- **Dual representations**: A concept exists as both a flag and an enum
  variant (e.g., `enable_implicit_string` bool vs `Condition::ImplicitString`
  in the conditions vec). Decide which is authoritative and document it.
- **Partial query evaluation**: A Phase N feature uses a struct that won't
  be fully implemented until Phase N+2. Identify the minimal subset needed
  now (e.g., Profile conditions need set-membership, not the full View
  query evaluator).
- **Missing store methods**: The engine may need a method that doesn't exist
  yet (e.g., "get all children of an exclusive category"). Check the store
  API surface.
- **Naming mismatches**: The spec may use a name that the code changed for
  good reasons (e.g., `Condition::String` → `Condition::ImplicitString`).
  Update the spec to match the code.

## 4. Design the dependency graph

Before creating issues, sketch the dependency chain:

- **What can start immediately?** (no dependencies beyond completed phases)
- **What enables parallelism?** Tasks touching different files/modules with
  no data dependency can run in parallel.
- **What is the critical path?** The longest chain determines minimum time
  to completion.
- **Are any tasks coupled tightly enough to merge?** If task A takes 5
  minutes and task B depends on A and can't start without it, consider
  combining them.

## 5. Write issue descriptions with implementer context

Each issue description should be self-contained enough that an agent can
implement it without re-reading the entire spec. Include:

- **What to build**: The specific functions/methods/types to implement.
- **Where it lives**: File path(s).
- **Key design decisions**: Anything non-obvious that came out of step 2-3
  (e.g., "check the bool flag, not the conditions vec").
- **Edge cases to handle**: From the spec's processing model or your trace.
- **What NOT to do**: Boundaries — "don't implement X, that's Phase N+1."

Avoid copying the spec verbatim. The spec describes *what the system does*.
The issue should describe *what the implementer should write*.

## 6. Create issues and verify the graph

```bash
# Create issues with descriptions, labels, and dependencies
br create "T0XX: Title" \
  -d "description..." \
  -p 0 -t task \
  -l phase:<phase-name> -l story:<story> \
  --deps blocks:<dep-id>

# After all issues are created, verify:
br ready          # Should show the entry point(s)
br blocked        # Should show correct dependency chains
```

Verify:
- `br ready` shows exactly the task(s) that can start now
- `br blocked` shows the right dependency chains
- No orphaned tasks (tasks with no path from a ready task)
- No missing dependencies (task assumes something that isn't tracked)

## 7. Post-review: Walk the checkpoint

Re-read the phase checkpoint. For each claim it makes, trace back to which
issue(s) deliver it:

| Checkpoint claim | Delivered by |
|---|---|
| "Creating category Sarah auto-assigns items containing Sarah" | T016 + T017 + T021 + T022 |
| "Profile conditions cascade" | T017 + T018 |
| "Exclusive categories enforce single-child" | T020 |

If a checkpoint claim can't be traced to any issue, something is missing.
If an issue doesn't contribute to any checkpoint claim, question whether
it belongs in this phase.

## 8. Commit the issues

```bash
br sync --flush-only
git add aglet-features.ag
git commit -m "br sync: Create Phase N issues (T0XX-T0YY)"
```

---

## Checklist summary

- [ ] Read the full phase and its checkpoint
- [ ] Trace the execution path through existing code + new code
- [ ] Identify spec/model gaps and ambiguities
- [ ] Resolve ambiguities in issue descriptions (not left for implementer to guess)
- [ ] Sketch dependency graph, identify parallelism and critical path
- [ ] Write self-contained issue descriptions with implementer context
- [ ] Create issues with labels and dependencies
- [ ] `br ready` and `br blocked` show correct graph
- [ ] Walk the checkpoint — every claim traces to an issue
- [ ] Commit `aglet-features.ag`
