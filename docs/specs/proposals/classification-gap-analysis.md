# Classification Gap Analysis: Aglet Implementation vs. Lotus Agenda

This document compares the **actual aglet implementation** (not the proposal doc) against
the original Lotus Agenda auto-classification system as described in the Kaplan/Kapor
CACM paper, the Fallows description, and the AgendaHelp reference manual.

Each gap is rated:

- **Critical** — core classification fidelity; without this, the system behaves
  fundamentally differently from Lotus Agenda in ways users will notice
- **Important** — significant UX or capability gap that limits usefulness
- **Nice-to-have** — enhancement that would improve fidelity but isn't load-bearing
- **Intentional divergence** — aglet deliberately chose a different design

---

## 1. Text Matching

### 1a. "Also match" field / aliases — CRITICAL

**Lotus Agenda:** Each category had an "Also match" field (up to 69 characters) supporting
a mini-language for additional match patterns beyond the category name:

- Semicolons for alternatives: `urgent; asap; critical` (any one matches)
- Commas for precision: `board, meeting` (all terms required)
- Wildcards: `Pat*` matches Pat, Patty, Patricia; `?` for single char
- Negation: `!draft` — item must NOT contain "draft"
- Substring acceptance: `~port` matches "report," "important," "transport"
- Quoting: `"exact phrase"` for literal matching

This was the primary mechanism for tuning classification beyond simple name matching.
A category named "Travel" could also match `hotel; flight; train; airbnb; lodging`.

**Aglet:** Only matches the literal category name via `SubstringClassifier`. No alias
storage, no pattern language, no additional match terms. The `Condition::ImplicitString`
variant carries no data — it's a marker, not a pattern container.

**Impact:** Users cannot tune text matching without renaming the category. A category
named "Phone Calls" will match "call" but not "ring," "dial," or "phone" (because
the matcher does word-boundary matching on the full multi-word name "Phone Calls" as
a substring, and "phone" alone without "calls" adjacent won't match). This makes
text matching far less useful in practice.

**What would need to change:**
- Add a `text_patterns: Vec<String>` (or structured alias type) to `Category`
- Persist alongside `conditions_json` or in a dedicated column
- Extend `SubstringClassifier` or `ImplicitStringProvider` to check patterns
- TUI: add an "Also match" field to Category Manager details pane

### 1b. Suffix stripping — IMPORTANT

**Lotus Agenda:** Words are matched after stripping common English suffixes:
`s`, `es`, `d`, `ed`, `er`, `est`, `ing`, `ful`, `wise`, `able`, `ible`, `ly`,
`ally`, `ment`, `al`, `ies`, `ier`, `ied`, `iful`, `ily`, `y`. Words under 4
letters are never suffix-stripped. Possessive `'s` is always ignored.

This means "call" matches "calls," "calling," "called," "caller." "Design" matches
"designing," "designed," "designer."

**Aglet:** Pure exact substring match (case-insensitive, word-boundary). "call" does
NOT match "calls." "travel" does NOT match "traveling" or "travels."

**Impact:** Users must create aliases for every inflected form, or the system misses
obvious matches. This is the single largest source of false negatives compared to
Lotus Agenda.

**What would need to change:**
- Implement a suffix-stripping function (not a full stemmer — Lotus used a fixed list)
- Apply to both category name/patterns and item text before comparison
- Add a global setting: "Ignore suffixes: Yes/No" (Lotus had this)

### 1c. Multi-word match strength (Initiative) — IMPORTANT

**Lotus Agenda:** For multi-word category names, the "Initiative" setting controlled
how many words needed to match:

| Setting | Threshold | Example: category "Policy Committee Meeting" |
|---------|-----------|----------------------------------------------|
| Exact | 100% | All 3 words must appear in item |
| Partial | 50% | At least 2 of 3 words must appear |
| Minimal | ~2% | At least 1 word must appear |

This was configurable globally and per-category.

**Aglet:** Matches the entire category name as a single substring. "Policy Committee
Meeting" only matches if those three words appear contiguously in that order. There is
no word-level decomposition or partial matching.

**Impact:** Multi-word category names are effectively useless for text matching unless
the exact phrase appears in items. A category named "Project Alpha" matches
`discuss Project Alpha today` but NOT `Alpha milestone for the project`.

**What would need to change:**
- Decompose multi-word category names into individual words
- Implement word-level matching with configurable threshold
- Add Initiative-equivalent setting (or fold into existing ContinuousMode, or add
  a match_strength field)

### 1d. Ignored delimiters — NICE-TO-HAVE

**Lotus Agenda:** Global setting to ignore text enclosed by quotes, slashes, angle
brackets, parentheses, braces, brackets, or single quotes. This prevented false
matches on quoted material, code snippets, or parenthetical asides.

**Aglet:** No delimiter awareness. All text is matched uniformly.

### 1e. Match scope control — NICE-TO-HAVE

**Lotus Agenda:** Per-category and global setting: match on "Item text only," "Note
text only," or "Both item & note."

**Aglet:** Always concatenates item text and note text via `match_text()` and matches
against the combined string. No per-field scoping.

---

## 2. Match Confirmation and Review (The "Questions Queue")

### 2a. Review workflow shape — CRITICAL

**Lotus Agenda:** The review workflow was:

1. `?` appears in upper-right control panel (passive, non-modal)
2. User navigates to F10 → Utilities → Questions when ready
3. System presents items ONE AT A TIME, each with its list of suggested categories
4. Per-item actions:
   - **TAB** — accept ALL suggested categories for this item
   - **Arrow keys** — navigate to highlight individual suggestions
   - **SPACEBAR** — toggle individual suggestion on/off
   - **ENTER** — confirm and advance to next item
5. When all items resolved, `?` disappears

Key UX properties:
- **Item-sequential**: user works through one item at a time, resolving all its
  suggestions before moving to the next
- **Toggle-based**: suggestions are individually toggled, not binary accept/reject
- **Non-destructive default**: suggestions start as "on" (proposed), user deselects
  unwanted ones
- **Batch resolution**: TAB for quick "accept all" when user trusts the system

**Aglet (current):** Standalone `Mode::ClassificationReview` modal with two panes
(Items list + Suggestions list). The user must:

1. Press `Shift+C` from Normal mode to open the modal
2. Navigate to an item in the left pane
3. Press `Enter` to focus the right pane (suggestions)
4. Accept (`Enter`) or reject (`r`) suggestions one at a time
5. Or use `A`/`R` to accept/reject all for the focused item

**Gaps vs. Lotus:**
- No toggle semantics — each suggestion is binary accept/reject, not a checkbox
- No "advance to next item" flow — user must manually navigate back to items pane
- Suggestions pane requires explicit focus switch (Tab or Enter), adding friction
- No "accept selected, skip unselected" — must explicitly reject unwanted ones
- The two-pane layout forces context-switching between items and suggestions
  rather than presenting them together

**The deeper issue:** The current modal feels like a database browser rather than a
triage workflow. Lotus's design was optimized for rapid sequential resolution — see
item, scan suggestions, toggle off the wrong ones, press Enter, repeat. The aglet
design requires more navigation steps per item.

### 2b. `?` indicator on individual items — IMPORTANT

**Lotus Agenda:** A single `?` appeared in the control panel area when ANY unreviewed
suggestions existed. It was a global signal, not per-item.

**Aglet:** Footer status text shows `"N classification suggestion(s) pending"` in
Normal mode. No `?` on individual items in the board view. The proposal doc describes
per-item `?` prefixes but these are NOT implemented.

**What exists:** The `ClassificationUiState.pending_count` is computed during refresh
and displayed in the footer. The item preview/info pane shows "Suggestions: N pending"
for the focused item. But there is no visual marker on board cells.

### 2c. Per-category confirmation control — IMPORTANT

**Lotus Agenda:** Each category could override the global Authority setting:
- Always confirm (even if global says "never")
- Sometimes confirm (only weak matches)
- Never confirm (always auto-apply)

This let users auto-apply assignments for high-confidence categories (like "Phone
Calls" with exact text match) while requiring review for ambiguous ones.

**Aglet:** `ContinuousMode` is global only. All categories get the same treatment.
No per-category override.

---

## 3. Assignment Lifecycle

### 3a. Conditional (auto-breaking) assignments — SHIPPED BEHAVIOR

**Lotus Agenda:** Condition-based assignments were *conditional* (`*c` in the
assignment profile). They:
- Existed only as long as the triggering condition remained true
- Auto-broke when item text changed and no longer matched
- Auto-broke when a prerequisite assignment was removed
- Were a fundamentally different type from explicit assignments

Example: Item "Call Mom" is conditionally assigned to "Phone Calls" via text match.
User edits to "Visit Mom" → assignment to "Phone Calls" automatically breaks.

**Aglet:** Current engine behavior now matches this distinction for
destination-centric rule output:
- implicit-string and profile-condition assignments are written as live
  non-sticky derived assignments
- they auto-break when the text/prerequisite state no longer matches
- action-produced, manual, and accepted-suggestion assignments remain sticky

**Implications:**
- The database can now shed stale condition-derived classifications on reprocess
- Users can see categories disappear when live conditions stop matching
- Historical sticky derived rows may still exist in older DBs until explicitly
  cleared, so provenance still matters during debugging

### 3b. Confirmation promotes live/suggested → explicit — PARTIAL PARITY

**Lotus Agenda:** When a user accepted a suggestion in the Questions queue, the
assignment was promoted from *conditional* to *explicit*. This meant:
- It survived future text edits
- It was the user's seal of approval
- It behaved differently from an unreviewed auto-assignment

**Aglet:** There is now a meaningful lifecycle distinction, but it is narrower:
- accepted suggestions are stored as sticky `SuggestionAccepted` assignments
- action/manual assignments are sticky
- live condition-derived assignments (`AutoMatch`) can auto-break

This means accepted suggestions do carry stronger durability than live
condition-derived assignments, even though the suggestion queue does not model
Lotus's exact "conditional first, then promote" flow for every provider path.

---

## 4. Condition and Action System

### 4a. TUI for creating/editing conditions — CRITICAL

**Lotus Agenda:** Conditions and actions were configured through Category Properties,
a dialog accessible from any category. Users could:
- Toggle "Match category name" on/off
- Set "Also match" text patterns
- Add assignment conditions ("if assigned to X")
- Add actions ("when assigned, also assign to Y")
- Set per-category execution timing

**Aglet:** The engine fully supports `Condition::Profile` and `Action::Assign/Remove`
with persistence in `conditions_json`/`actions_json` columns. But there is **no TUI
to create, edit, view, or delete them**. They are invisible to TUI users.

The only way to define conditions or actions is through the CLI or direct database
manipulation. The proposal doc (Phase C–E) describes condition/action display and
editor popups but none of this is implemented.

**Impact:** The most powerful part of the classification system — cascading rules
that create sophisticated automation — is completely inaccessible in the TUI. This
is the single largest TUI gap.

### 4b. Date conditions — IMPORTANT

**Lotus Agenda:** Categories could test whether an item's date fell within a range:
"assign if When date is between March 1 and March 31." Useful for "This Month's
Tasks," "Overdue Items," time-based escalation.

**Aglet:** No `Condition::DateRange` variant exists. The `Condition` enum has only
`ImplicitString` and `Profile`. The proposal mentions this as future work (Phase I).

### 4c. Date actions — NICE-TO-HAVE

**Lotus Agenda:** Actions could set dates: "When assigned to Received, set When date
to today." Could use natural language: "2 weeks from today," "next Friday."

**Aglet:** Actions only support `Assign` (to categories) and `Remove` (from categories).
No date-setting action.

### 4d. Discard/Done actions — NICE-TO-HAVE

**Lotus Agenda:** Special action types:
- "Discard item" — auto-trash items assigned to this category
- "Designate as done" — auto-mark done with current date
- "Export item" — save to file

**Aglet:** No equivalent action types. Users must manually mark items done or discard.

### 4e. Execution timing control — NICE-TO-HAVE

**Lotus Agenda:** Per-category setting: Automatically / On demand / Never. This let
users keep expensive or noisy conditions dormant until manually triggered (Alt-E for
one category, Alt-X for entire file).

**Aglet:** Global `run_on_item_save` and `run_on_category_change` booleans control
when classification runs. No per-category timing control. The `=` key triggers manual
recalc for the focused item, which is a rough equivalent of "on demand."

### 4f. Conflict resolution policy — NICE-TO-HAVE

**Lotus Agenda:** Global and per-category setting: "If assignment conflicts: Keep the
old / Override the old." This controlled what happened when a new condition match
conflicted with an existing assignment (particularly relevant for exclusive categories).

**Aglet:** Implicit "first match wins" behavior. Manual assignments block auto-match
of exclusive siblings (`has_manual_exclusive_sibling()`). No explicit conflict
resolution setting.

### 4g. Text-and-assignment condition relationship — NICE-TO-HAVE

**Lotus Agenda:** Global setting: OR (text OR assignment condition suffices) vs. AND
(text condition AND assignment condition both required). This controlled whether
text matching and profile conditions were alternatives or conjunctive.

**Aglet:** Text matching (`enable_implicit_string`) and profile conditions
(`Condition::Profile`) are evaluated independently. If either matches, the category
is assigned. Effectively OR-only. No AND mode.

---

## 5. Condition/Action Visibility and Badges

### 5a. Conditions/actions invisible in Category Manager — IMPORTANT

**Lotus Agenda:** Category Properties showed all conditions, actions, and settings
for a category in a single dialog. Users could see at a glance what rules a category
had.

**Aglet:** The Category Manager details pane shows name, flags (auto-match, exclusive,
actionable), and note. It does NOT show conditions or actions, even though they may
exist in the database. Users have no way to know a category has rules attached.

The proposal doc (Phase C) describes adding "Conditions (N)" and "Actions (N)"
sections to the details pane, and Phase D–E describe editor popups. None implemented.

### 5b. Tree badges for conditions/actions — NICE-TO-HAVE

**Lotus Agenda:** No specific tree badges (categories were shown in a flat or
hierarchical list without inline indicators for rules).

**Aglet proposal:** Describes `[C2]` and `[A1]` badges in the category tree. Not
implemented, but would be useful once conditions/actions are visible.

---

## 6. Review Path Integration

### 6a. Item-centric review from board view — IMPORTANT

**Lotus Agenda:** The review workflow was accessible from a utility menu while viewing
items. The user didn't need to leave their current view context.

**Aglet:** Review requires opening a standalone modal (`Mode::ClassificationReview`
via `Shift+C`), which replaces the board view entirely. The proposal describes
inline review via the preview pane (staying in board context), but this is not
implemented.

### 6b. Category-centric review — IMPORTANT

**Lotus Agenda:** Category Properties let users see which items matched a category's
conditions. This was category-focused curation: "What items should be in Travel?"

**Aglet:** No category-centric review path. The standalone modal shows items with
pending suggestions, but you can't filter by category. The proposal describes
pressing `R` in Category Manager to see suggestions scoped to the selected category.
Not implemented.

---

## 7. Provider and Matching Infrastructure

### 7a. No semantic/LLM providers — EXPECTED (FUTURE)

**Lotus Agenda:** No AI/ML matching (1988 software). All matching was rule-based.

**Aglet:** The `ClassificationProvider` trait is designed for extensibility. Stubs
exist for future `OllamaProvider`, `AnthropicProvider`. Not implemented, but the
infrastructure is ready. This is a planned aglet extension beyond Lotus fidelity.

### 7b. Confidence is always 1.0 — EXPECTED (FUTURE)

**Aglet:** Both built-in providers return confidence 1.0. The `confidence` field
exists on `ClassificationSuggestion` for future use by ML providers that would
return graduated confidence scores.

---

## 8. Summary: Priority-Ordered Gap List

### Must-have for classification fidelity

| # | Gap | Lotus Feature | Aglet Status |
|---|-----|--------------|-------------|
| 1 | Also-match aliases | Per-category text patterns with `;` OR, `,` AND, `*` wildcard, `!` negation | Not implemented; only category name matched |
| 2 | Suffix stripping | `s/es/ed/ing/er/est/ful/ly/ment/al/...` stripped before comparison | Not implemented; exact substring only |
| 3 | Multi-word decomposition | Initiative: Exact/Partial/Minimal word-count thresholds | Not implemented; full phrase must appear contiguously |
| 4 | TUI condition/action editor | Category Properties dialog for defining rules | Not implemented; rules invisible in TUI |
| 5 | Review workflow shape | Sequential item triage with toggle semantics | Modal with two-pane browser; higher friction |

### Important for usable classification

| # | Gap | Lotus Feature | Aglet Status |
|---|-----|--------------|-------------|
| 6 | Per-category confirmation | Authority override per category | Global ContinuousMode only |
| 7 | `?` on board items | Visual indicator on items with pending suggestions | Footer count only; no per-item markers |
| 8 | Date conditions | Assign if item date falls in range | Not implemented |
| 9 | Inline board review | Review suggestions without leaving board context | Requires standalone modal |
| 10 | Category-centric review | Review items matching a specific category | Not implemented |
| 11 | Condition/action display | See rules on a category in the details pane | Not implemented |

### Nice-to-have for completeness

| # | Gap | Lotus Feature | Aglet Status |
|---|-----|--------------|-------------|
| 12 | Ignored delimiters | Skip text in quotes, parens, brackets | Not implemented |
| 13 | Match scope | Item text / Note text / Both (per-category) | Always both |
| 14 | Date actions | Set When date on category assignment | Not implemented |
| 15 | Discard/Done actions | Auto-trash or auto-complete items | Not implemented |
| 16 | Execution timing | Automatically / On demand / Never (per-category) | Global only |
| 17 | Conflict resolution | Keep old / Override old policy | Implicit first-match-wins |
| 18 | Text+Assignment AND mode | Require both text AND profile match | OR-only |
| 19 | Tree badges | [C2] [A1] indicators in category tree | Not implemented |

### Intentional divergences (not gaps)

| # | Decision | Lotus Behavior | Aglet Choice | Rationale |
|---|----------|---------------|-------------|-----------|
| 20 | Sticky assignments | Conditional assignments auto-break | All assignments permanent | Simpler mental model; items don't silently disappear |
| 21 | Single control knob | Initiative + Authority (2 knobs) | ContinuousMode (1 knob) | Reduced complexity; revisit if users need granularity |
| 22 | No Classification Center | Category Properties dialog | Category Manager integration | Classification is category behavior, not separate system |

---

## 9. Current Implementation Inventory

For reference, here is what IS fully implemented and working:

**Engine layer (complete):**
- `SubstringClassifier`: case-insensitive, word-boundary matching on category name
- `Condition::Profile { criteria: Box<Query> }`: AND/OR/NOT on current assignments
- `Action::Assign { targets }` / `Action::Remove { targets }`: with deferred removal
- Fixed-point cascade loop (max 10 passes, cycle detection, savepoint rollback)
- Subsumption: automatic parent assignment up the hierarchy
- Mutual exclusion: exclusive parent siblings, manual-wins precedence
- `ClassificationSuggestion` lifecycle: Pending/Accepted/Rejected/Superseded
- Deterministic suggestion IDs (v5 UUID from item+revision+provider+assignment)
- Item revision hashing (detects text/assignment changes, supersedes old suggestions)

**Providers (complete):**
- `ImplicitStringProvider`: word-boundary substring match on category name
- `WhenParserProvider`: natural language date parsing → When assignment
- `ClassificationProvider` trait: extensible for future providers

**Configuration (complete):**
- `ClassificationConfig` with `ContinuousMode` (Off/AutoApply/SuggestReview)
- `run_on_item_save`, `run_on_category_change` toggles
- `ProviderConfig` with enable/disable per provider
- Persistence in `app_settings` table

**Store layer (complete):**
- `classification_suggestions` table with full schema
- `conditions_json`, `actions_json` columns on categories
- CRUD for suggestions, supersession, status updates

**TUI (partial):**
- `Mode::ClassificationReview` standalone modal (functional but design-questionable)
- Classification mode picker in Category Manager (`m` key)
- Footer pending count in Normal mode
- Item preview shows pending suggestion count

**CLI (not audited in this analysis)**

---

## 10. Decisions (2026-03-21)

1. **Gap 1 (Also-match aliases):** APPROVED. Add "Also match" text patterns to
   categories. Also add a stub for LLM-based semantic matching, toggleable both
   globally and per-category. The only affordance today is the auto-match toggle;
   we need the alias field alongside it.

2. **Gap 2 (Suffix stripping):** APPROVED. Basic feature, should be added.

3. **Gap 3 (Multi-word decomposition / Initiative):** SKIP. Will be superseded by
   eventual LLM-based matching, which handles this more naturally.

4. **Gap 4 (Condition/action editor TUI):** Plumbing exists, TUI does not. This is
   a large project needing its own research — the design question (complete boolean
   language? GUI picker? something else?) is unresolved. Deferred to separate effort.

5. **Gap 5 (Review workflow shape):** TOP PRIORITY. This is the main focus of
   current work. See §11 below.

6. **Gap 4b (Date conditions):** Important for time-based views ("This Week's Tasks,"
   "Overdue Items"). Should be planned but not top priority.

## 11. Review Workflow Design — Current Focus

See `classification-review-workflow.md` for the detailed design.
