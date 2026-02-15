# Task: T015 + T016 — Classifier Trait & SubstringClassifier

## Context

You are working on **Agenda Reborn**, a Rust clone of Lotus Agenda (1988) — a
free-form personal information manager where you type items in natural language
and the system organizes them automatically.

The core idea: when a user creates a category called "Sarah", all existing items
containing the word "Sarah" should be auto-assigned to that category. The
**Classifier** is the component that decides whether an item's text matches a
category's name.

## What to read

Before writing code, read these files to understand the architecture:

1. `spec/mvp-spec.md` §2.5 (The Classifier Trait) — the spec for what you're building
2. `spec/mvp-spec.md` §2.4 (Conditions and Actions) — how the classifier fits
   into the engine's processing model (you're not building the engine, but you
   need to understand how your code will be called)
3. `crates/agenda-core/src/model.rs` — the Category struct (has
   `enable_implicit_string: bool` which controls whether classification runs)
4. `crates/agenda-core/src/lib.rs` — module declarations (matcher is already stubbed)
5. `AGENTS.md` — branching workflow and issue comment protocol

## What to build

**File**: `crates/agenda-core/src/matcher.rs`

### T015: Classifier trait

```rust
pub trait Classifier: Send + Sync {
    /// Returns None = no match, Some(confidence) = match.
    /// MVP: SubstringClassifier returns Some(1.0) or None.
    /// Future: LlmClassifier returns graded confidence scores.
    fn classify(&self, text: &str, category_name: &str) -> Option<f32>;
}
```

The trait must be `Send + Sync` so it can be shared across threads in future
phases. For MVP it's single-threaded, but the bounds cost nothing to add now.

### T016: SubstringClassifier

A struct implementing `Classifier` that does **case-insensitive word-boundary
matching**. Given item text and a category name, it returns `Some(1.0)` if the
category name appears as a whole word in the text, or `None` otherwise.

**Matching rules:**
- Case-insensitive: "sarah" matches "Sarah" and "SARAH"
- Word-boundary: "Sarah" matches in "Call Sarah tomorrow" but NOT in "Sarahville"
- Word-boundary: "Done" matches in "Get it Done!" but NOT in "Condone"
- Multi-word category names should match as a phrase: category "Project Alpha"
  matches "discuss Project Alpha today"

**Implementation approach:** Use `regex` crate with `\b` word boundaries, or
implement with character-class checks (alphanumeric boundary detection). If using
regex, add the `regex` dependency to `crates/agenda-core/Cargo.toml`. Either
approach is fine — correctness and readability matter more than cleverness.

**What `enable_implicit_string` means for you:** The classifier itself does NOT
check this flag. The caller (the engine, T017) checks `category.enable_implicit_string`
and only calls the classifier if it's true. Your classifier is a pure function:
text in, category name in, match result out.

## Tests to write

At minimum:

1. **Basic match**: "Call Sarah tomorrow" matches category "Sarah" → Some(1.0)
2. **Case insensitive**: "call sarah tomorrow" matches "Sarah" → Some(1.0)
3. **No match**: "Call Bob tomorrow" does NOT match "Sarah" → None
4. **Word boundary — no partial match**: "Sarahville" does NOT match "Sarah" → None
5. **Word boundary — punctuation**: "Done!" matches "Done" → Some(1.0)
6. **Word boundary — start of string**: "Sarah called" matches "Sarah"
7. **Word boundary — end of string**: "Call Sarah" matches "Sarah"
8. **Multi-word**: "discuss Project Alpha today" matches "Project Alpha"
9. **No match for unrelated text**: "Buy groceries" does NOT match "Sarah" → None

## What NOT to do

- Don't implement the engine or process_item — that's T017
- Don't check `enable_implicit_string` inside the classifier — the engine handles that
- Don't add confidence thresholds or suggestion queues — MVP is binary match/no-match
- Don't worry about Unicode word boundaries beyond ASCII — defer to Phase 2+ (future)

## Workflow

Follow the branching workflow in `AGENTS.md`:

```bash
# The issue claim and br operations should already be done on main
# before you start. If not, do them on main first:
#   br update bd-2dg --status in_progress
#   br comments add bd-2dg "Claimed <date>. Plan: define Classifier trait + SubstringClassifier with word-boundary matching"
#   br sync --flush-only && git add .beads/ && git commit -m "br sync: Claim bd-2dg"

# Create your branch from main
git checkout -b task/t015-t016-classifier

# Implement in crates/agenda-core/src/matcher.rs
# Run tests: cargo test -p agenda-core
# Run clippy: cargo clippy -p agenda-core
# Commit your work on the branch

# When done, the merge and br close happen on main
```

## Definition of done

- [ ] `Classifier` trait defined with `classify` method
- [ ] `SubstringClassifier` struct implements `Classifier`
- [ ] All tests listed above pass (add more if you find edge cases)
- [ ] `cargo clippy -p agenda-core` clean (no warnings)
- [ ] `cargo test -p agenda-core` passes (all existing + new tests)
- [ ] No changes to files outside `crates/agenda-core/src/matcher.rs` and
      `crates/agenda-core/Cargo.toml` (if adding regex)

## How your code will be used (by T017, the engine)

```rust
// The engine will call your classifier like this:
fn evaluate_category(&self, item: &Item, category: &Category) -> bool {
    if category.enable_implicit_string {
        if let Some(_confidence) = self.classifier.classify(&item.text, &category.name) {
            return true;
        }
    }
    // ... also check Profile conditions ...
    false
}
```

This is why the classifier doesn't need to know about `enable_implicit_string` —
the engine gates on that flag before calling you.
