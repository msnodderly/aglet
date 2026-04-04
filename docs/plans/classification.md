 The Three Classification Mechanisms (from the user's perspective)

  1. Text Matching (String Conditions)

  Every category could optionally match its name against item text. If the user had a category called "Phone Calls," then typing Call Mom about birthday would
  trigger a match — the system recognized that "call" and "calls" are close enough (suffix stripping: s, ed, ing, er, etc. are ignored; words under 4 letters are
  never suffix-stripped).

  Beyond the name, each category had an "Also match" field — up to 69 characters of additional match patterns using a mini-language:
  - Semicolons for alternatives: urgent; asap; critical — any one matches
  - Commas for precision (all terms required): board, meeting — both must appear
  - Wildcards: Pat* matches Pat, Patty, Patricia
  - Negation: !draft — item must NOT contain "draft"
  - Tildes for substrings: ~port matches "report," "important," "transport"

  The match algorithm was semantic-aware in a limited way: it identified proper names, dates, times, and numbers as distinct token types, avoided matching them
  against ordinary words, and performed case-insensitive comparison after suffix stripping.

  2. Profile Conditions (Assignment Conditions)

  A category could define a rule: "Assign items to me if they are assigned to category X" — or the converse, "if they are NOT assigned to X." These created chains
   of implication:

  - "Urgent Work" has profile condition: assigned to "Work" AND assigned to "High Priority" → item is conditionally assigned to "Urgent Work."
  - "Discuss with Manager" has condition: assigned to "John Smith" AND assigned to "Policy Committee" → conditionally assigned.

  Profile conditions were evaluated after text matching, so text matches could trigger profile conditions in a cascade: text match → assigns to "Phone Calls" →
  profile condition on "Follow Up" fires → profile condition on "High Priority Follow Up" fires → and so on, up to a fixed point.

  3. Date Conditions

  Categories could test whether an item's date (entry date, completion date, or user-assigned calendar date) fell within a specified range. These were useful for
  time-sensitive classification: "This Week's Meetings," "Overdue Items," "Q1 Bills."

  The Two Control Knobs: Initiative and Authority

  This is where the user experience gets interesting. Lotus Agenda gave users two global settings that controlled the aggressiveness/autonomy tradeoff:

  Initiative (Match Sensitivity Threshold)

  This was a global slider with three positions:

  ┌─────────┬──────────────────────────────────────────────────────────────────┬───────────┐
  │ Setting │                             Meaning                              │ Threshold │
  ├─────────┼──────────────────────────────────────────────────────────────────┼───────────┤
  │ Exact   │ All words in the category name/condition must appear in the item │ 100%      │
  ├─────────┼──────────────────────────────────────────────────────────────────┼───────────┤
  │ Partial │ At least half the words must match                               │ 50%       │
  ├─────────┼──────────────────────────────────────────────────────────────────┼───────────┤
  │ Minimal │ At least one word must match                                     │ ~2%       │
  └─────────┴──────────────────────────────────────────────────────────────────┴───────────┘

  At Exact, the system was conservative — Call Fred about the policy meeting would match "Phone Calls" only if all the words in the category name appeared (not
  just "call"). At Minimal, it was aggressive — a single word overlap was enough.

  Each category could override the global initiative setting, so a user might run globally at Partial but set "Important Clients" to Exact (to avoid false
  positives on partial name matches).

  Authority (Silent Apply vs. Queue for Review)

  This controlled what happened after a match was found:

  ┌───────────┬───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
  │  Setting  │                                                         Behavior                                                          │
  ├───────────┼───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┤
  │ Never     │ All matches are applied immediately, silently. The user sees the item appear in matching categories with no intervention. │
  ├───────────┼───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┤
  │ Sometimes │ Only weak/partial matches are queued for review. Strong (exact) matches are applied silently.                             │
  ├───────────┼───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┤
  │ Always    │ Every match, no matter how strong, is queued for review. Nothing is auto-applied.                                         │
  └───────────┴───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘

  Again, this could be overridden per-category.

  The ? Indicator and the Questions Queue

  When Authority was set to "Sometimes" or "Always," matches that needed review were placed in a questions queue. The user's only indication was a small ? symbol
  in the upper-right corner of the screen (the control panel area). This was a passive, non-modal notification — it didn't interrupt work.

  The ? appeared the moment the first unreviewed suggestion entered the queue. It stayed visible until all suggestions were resolved.

  The Review Workflow (Utilities > Questions)

  When the user was ready to review, they navigated to F10 → Utilities → Questions. This opened a dedicated review interface:

  1. Item-by-item presentation: The system showed the first item with pending suggestions.
  2. Suggested categories listed: Below the item text, the system displayed a list of categories it wanted to assign. Each suggestion could be individually
  selected.
  3. User actions per item:
    - TAB — Accept ALL suggested categories for this item (bulk accept)
    - Arrow keys (↑/↓, PgUp/PgDn) — Navigate to highlight a specific suggested category
    - SPACEBAR — Toggle an individual suggestion on/off (accept or reject it)
    - ENTER — Confirm choices and advance to the next item with pending suggestions
  4. Progression: After pressing ENTER, the system moved to the next item in the queue. When all items were resolved, the ? disappeared from the control panel.
  5. Result of acceptance: Once a suggested assignment was accepted, it was promoted from a conditional assignment to an explicit assignment. This was a crucial
  distinction.

  Conditional vs. Explicit Assignments: The Lifecycle

  This is the most nuanced part of the Agenda classification UX:

  Conditional assignments (marked *c in the assignment profile):
  - Created automatically by conditions (text match or profile match)
  - Temporary: They existed only as long as the triggering condition remained true
  - If the user edited Call Mom about birthday to Visit Mom about birthday, the conditional assignment to "Phone Calls" automatically broke — the item silently
  left the category
  - If a profile condition depended on assignment to "High Priority" and that assignment was removed, the downstream conditional assignment also broke
  - This gave the system a "living" quality — items flowed in and out of categories as their text and assignments changed

  Explicit assignments (marked * in the assignment profile):
  - Created by manual user action, by accepting a suggestion, or by an action rule firing
  - Permanent: They persisted regardless of changes to item text or other assignments
  - The user had to actively break them (unassign the category)

  The review workflow was the bridge between these two states. When the user accepted a suggestion, they were saying: "Yes, this is correct — make it permanent."
  The assignment was no longer dependent on the text match; it was a confirmed fact about the item.

  If the user didn't review suggestions (left Authority at "Never"), all text-match assignments remained conditional — meaning they could auto-break if the item
  text changed. This was by design: the system was acting autonomously but non-committally.

  Actions: One-Shot Consequences

  Separate from conditions, categories could have actions — rules that fired once when an item was assigned to the category:

  - Assign action: "When assigned to Done, also assign to Archive"
  - Remove action: "When assigned to Escalated, remove from Low Priority"
  - Date action: "When assigned to Received, set date to today"
  - Discard action: "When assigned to Spam, discard the item"

  Actions created explicit assignments — they were permanent, one-shot consequences. Even if the triggering assignment was later removed, the action's result
  persisted. This made actions fundamentally different from conditions:

  ┌─────────────────┬──────────────────────────────────┬────────────────────────────────┐
  │                 │            Conditions            │            Actions             │
  ├─────────────────┼──────────────────────────────────┼────────────────────────────────┤
  │ Direction       │ Pull items INTO this category    │ Push items to OTHER categories │
  ├─────────────────┼──────────────────────────────────┼────────────────────────────────┤
  │ Assignment type │ Conditional (temporary)          │ Explicit (permanent)           │
  ├─────────────────┼──────────────────────────────────┼────────────────────────────────┤
  │ Re-evaluation   │ Continuous                       │ One-shot                       │
  ├─────────────────┼──────────────────────────────────┼────────────────────────────────┤
  │ Breaking        │ Auto-breaks when condition fails │ Must be manually broken        │
  └─────────────────┴──────────────────────────────────┴────────────────────────────────┘

  The Cascade Loop

  Conditions and actions could interact in chains:

  1. User types Call Fred about the urgent policy review
  2. Text match → conditionally assigned to "Phone Calls"
  3. Text match → conditionally assigned to "Policy Committee"
  4. Text match → conditionally assigned to "Urgent"
  5. Profile condition on "Urgent Calls": assigned to "Phone Calls" AND "Urgent" → conditionally assigned
  6. Action on "Urgent Calls": assign to "High Priority" → explicitly assigned
  7. Profile condition on "Manager Review": assigned to "High Priority" AND "Policy Committee" → conditionally assigned

  This cascade ran until no new assignments were produced (fixed point), with a practical limit to prevent infinite loops.

  Execution Timing

  Users controlled when conditions were evaluated:

  ┌───────────────┬───────────────────────────────────────────────────────────────────────────────────────────────────┐
  │    Setting    │                                             Behavior                                              │
  ├───────────────┼───────────────────────────────────────────────────────────────────────────────────────────────────┤
  │ Automatically │ Conditions evaluate immediately on item entry or edit                                             │
  ├───────────────┼───────────────────────────────────────────────────────────────────────────────────────────────────┤
  │ On Demand     │ Conditions only run when user explicitly requests (Alt-E for one category, Alt-X for entire file) │
  ├───────────────┼───────────────────────────────────────────────────────────────────────────────────────────────────┤
  │ Never         │ Conditions are defined but dormant                                                                │
  └───────────────┴───────────────────────────────────────────────────────────────────────────────────────────────────┘

  This per-category setting let users keep expensive or noisy conditions from running constantly while still maintaining them for periodic use.

  The Natural Language Date Parser

  The most visible auto-classification to new users was the When category. Agenda parsed English date expressions from item text:

  - Call Fred next Tuesday → When: next Tuesday
  - File taxes by April 15 → When: April 15
  - Two weeks from last Thursday → When: [computed date]
  - Meet daily at 3pm → When: recurring daily at 3:00 PM

  This was always-on, always-auto-applied — there was no Authority/Initiative gating on date parsing. It "just worked," and was the gateway that introduced users
  to the broader classification system.

  Global Settings Box

  All of these controls were unified in a single Auto-Assign Global Settings dialog:

  - Text matching: On/Off
  - Match on: Item text / Note text / Both
  - Required match strength: Exact/Partial/Minimal
  - Confirm assignments: Always/Sometimes/Never
  - Ignore text enclosed by: quotes, parens, brackets, etc.
  - Ignore suffixes: Yes/No
  - Assignment conditions: On/Off
  - Assignment actions: On/Off
  - Apply conditions: Automatically/On demand/Never
  - If assignment conflicts: Keep old/Override old
  - Relationship of text & assignment conditions: OR/AND

  ---
  Relevance to Aglet's Current Design

  Your current implementation has already mapped these concepts, with one intentional simplification: ContinuousMode conflates Initiative and Authority into a
  single knob (Off / AutoApply / SuggestReview). This is noted in the proposal as intentional.

  The key Lotus Agenda UX concepts that remain relevant to your proposal work:

  1. The ? passive indicator — non-modal, non-interruptive notification
  2. The batch review workflow — item-by-item with per-suggestion toggle (TAB=all, SPACE=individual, ENTER=next)
  3. Conditional → Explicit promotion on acceptance — shipped semantics are now
  closer to Lotus than this older summary implied. Live implicit/profile
  assignments can auto-break, while accepted suggestions become sticky
  AssignmentSource::SuggestionAccepted assignments.
  4. Two review orientations — item-centric ("what should this item be?") and category-centric ("what belongs in this category?") — faithfully captured in your
  Path 1 / Path 2 design
  5. The cascade — already implemented in your engine with 10-pass fixed-point loop

  ---
  Current implementation notes (April 2026)

  - Semantic review suggestion filtering now checks the item's effective
    current assignment state, not just manual categories. That means review
    candidates are skipped when they are already satisfied, would conflict with
    an assigned sibling under an exclusive parent, or would be a no-op after
    reprocess.
  - The semantic prompt's "Already assigned" context now includes live derived
    assignments too, which keeps the model from being blind to categories such
    as `High` that were assigned by rules/actions rather than manually.
  - TUI assign/unassign flows now preview manual toggles before mutating. If a
    live rule would immediately re-apply a category, the UI keeps the
    assignment and shows an explanation instead of leaving the category looking
    mysteriously stuck.

  The biggest fidelity question is no longer whether live conditional
  assignments auto-break; they do. The remaining fidelity questions are around
  date conditions, recurrence/post-done behavior, and how much of Lotus's
  condition/action surface we want to expose in CLI/TUI.
