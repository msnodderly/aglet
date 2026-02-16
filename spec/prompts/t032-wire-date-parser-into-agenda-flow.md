# Task: T032 - Wire Date Parser into Item Create/Update Flow

## Context

T028-T031 implemented `DateParser` and `BasicDateParser` behavior. T032 wires
that parser into item lifecycle operations so `when_date` is populated
automatically from item text and visible to virtual When buckets.

This is integration in `agenda-core` only (no CLI/TUI wiring in this task).

## What to read

1. `spec/phase5-overview.md` - T032 behavior and caveats.
2. `spec/mvp-spec.md` section 2.7 - parser contract and intended use.
3. `spec/mvp-tasks.md` - T032 checkpoint expectations.
4. `spec/prompts/t031-basic-date-parser-time-and-compound.md` - parser capabilities.
5. `crates/agenda-core/src/agenda.rs` - create/update integration surface.
6. `crates/agenda-core/src/model.rs` (`Item.when_date`, `Assignment.origin`).
7. `crates/agenda-core/src/query.rs` (`resolve_when_bucket`) for integration assertions.

## What to build

**Primary file**: `crates/agenda-core/src/agenda.rs`

Integrate date parsing into item create/update lifecycle.

### Behavioral rules

- On item create and update:
  1. Run parser on item text with `reference_date`.
  2. If parse succeeds, set `item.when_date = Some(parsed.datetime)` before
     engine processing.
  3. If parse fails, do not auto-clear existing `when_date`.
- When parse succeeds, record provenance by assigning the reserved `When`
  category with:
  - `source = AutoMatch`
  - `origin = "nlp:date"`
- Keep parser wiring compatible with existing engine/store flow.
- Do not store bucket assignments; bucketing remains virtual via query logic.

### API expectations

- Preserve existing public API usability.
- If needed for deterministic tests, add a reference-date-aware variant while
  keeping current convenience methods.

## Tests to write

Add integration tests in `crates/agenda-core/src/agenda.rs` (or nearby existing
integration tests) validating:

1. **Create path sets when_date** from parsed text and persists it.
2. **Update path sets when_date** when text gains a parseable date.
3. **No parse does not clear when_date** on update.
4. **Provenance assignment set** on parse success:
   - assigned to reserved `When` category
   - `source = AutoMatch`
   - `origin = "nlp:date"`
5. **When bucket integration**: item parsed on create/update resolves to the
   expected `WhenBucket` for a known `reference_date`.

## What NOT to do

- Do not change parser extraction logic (T029-T031 scope).
- Do not add UI/CLI behavior in this task.
- Do not introduce bucket persistence.

## Definition of done

- [ ] Create/update flows call parser and persist parsed `when_date`.
- [ ] Parse success records `origin = "nlp:date"` on reserved `When` assignment.
- [ ] Parse miss leaves existing `when_date` unchanged.
- [ ] Integration tests validate end-to-end behavior through query bucketing.
- [ ] `cargo test -p agenda-core` passes.
- [ ] `cargo clippy -p agenda-core` is clean for this scope.
