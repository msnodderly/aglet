# Agenda Reborn — NLSpec v0.6

> A modern clone of Lotus Agenda (1988–1992), the free-form personal information manager
> designed by Mitch Kapor, Ed Belove, and Jerry Kaplan.
>
> This is a Natural Language Spec (NLSpec): a human-readable specification intended to be
> directly usable by coding agents to implement and validate behavior. Scenarios follow the
> Cem Kaner (2003) scenario testing model, repurposed as end-to-end user stories validated
> by satisfaction rather than boolean pass/fail.

---

## 1. Historical Context and Design Philosophy

### 1.1 What Was Lotus Agenda?

Lotus Agenda was a DOS-based personal information manager released in 1988. It was described
as "a spreadsheet for words." The term PIM was coined by Agenda's marketing manager,
Connell Ryan, specifically to describe it.

Agenda's radical premise: **you should be able to type information in natural language and
have the system figure out how to organize it — not the other way around.** The user is never
required to fill out forms or define schemas upfront. 

The program had three core concepts:

- **Items** — atomic units of information (a phrase, a sentence, a task, a fact)
- **Categories** — hierarchical, multi-assignable labels that provide structure
- **Views** — dynamic, editable projections of items filtered and sectioned by categories

What made it extraordinary was the interplay between these three: automatic assignment via
NLP and rules (or manual assignment), editing through views with reverse-inference of intent,
and a category hierarchy that functioned as a declarative program executed against every 
changed item.

### 1.2 Why Every Clone Failed

Chandler (2001–2009) was Kapor's own attempt to rebuild Agenda as open-source software.
It burned through $8M+ over 7+ years and never shipped a usable 1.0. The failure modes:

- **Feature creep**: tried to be Outlook/Exchange instead of Agenda
- **Over-engineering**: chose architectural purity over working software
- **Loss of focus**: the core insight (schema-last, multifiling, views-as-interface) was
  diluted into a generic calendaring/email app
- **No throwaway prototypes**: committed to a grand architecture before validating the ideas


### 1.3 Design Principles for This Project

These are non-negotiable constraints derived from what made Agenda great and what killed
its successors:

1. **Schema-last**: The user types first. Structure is inferred or added after the fact.
   Never require the user to define a category before using it.
2. **One item, many homes**: Multifiling is the killer feature. An item can belong to
   any number of categories simultaneously. This is what paper can't do.
3. **Views are the interface**: Users don't interact with a raw database. They interact
   with views — filtered, sectioned, annotated projections. And they can *edit through*
   those views.
4. **The hierarchy is a program**: The category tree, with its conditions and actions,
   is a declarative rule engine that runs against every changed item.
5. **Start small, stay small**: Build the smallest thing that lets you experience the
   core interaction loop. Resist the urge to add calendaring, contacts, email, sync.
   Those are how Chandler died.
6. **Frontend-agnostic core**: The data model and rule engine should be a library with no
   UI dependencies. The TUI is the first client. macOS native is the second. The core
   never imports a UI framework.

### 1.4 What Kapor Said About It

> "The greatest thing about Agenda and the reason why it still has a cadre of followers
> is also the one thing that hasn't been incorporated into recent PIMs: multifiling."
>
> "Today's PIMs are very Web-influenced and they have connectivity features, but they're
> stuck in the old mindset. They're focused on managing contacts and calendars. Agenda
> was all about managing ideas."
>
> "God, I wish there was a modern version of this."

---

## 2. Core Data Model

### 2.1 Item

An Item is the atomic unit of content. It is a short piece of text — a thought, task,
fact, reminder, or note — with optional metadata.

```
Item {
  id:                       UUID
  text:                     String (max ~350 chars, following Agenda's original limit)
  note:                     Optional<String> (unlimited length; for extended detail)
  created_at:               Timestamp
  modified_at:              Timestamp
  entry_date:               Date                  // auto-set on creation, never null
  when_date:                Optional<DateTime>    // parsed from text or manually assigned
  recurrence:               Optional<RecurrenceRule>  // parsed from text or manually set
  recurrence_series_id:     Optional<UUID>        // links all instances in a recurring series
  recurrence_parent_item_id:Optional<UUID>        // the prior instance that generated this one
  done_date:                Optional<DateTime>    // when marked complete
  is_done:                  Boolean (default false)
  assignments:              Map<CategoryID, Assignment> // membership + provenance
  values:                   Map<CategoryID, Value> // typed values for column-categories
  rejected_suggestions:     Set<CategoryID>        // categories rejected by user; not re-suggested
}

Value = TextValue(String) | NumericValue(Float) | DateValue(DateTime)

Assignment {
  source:           enum { manual, auto_match, suggestion_accepted, action, subsumption }
  assigned_at:      Timestamp
  match_confidence: Optional<Float>   // for auto_match: the classification confidence (0.0–1.0)
  origin:           Optional<String>  // provenance string: which rule/parser created this?
  sticky:           Boolean (default true)  // assignment is not auto-removed on text edits
}

// Assignment.origin format:
// A short, human-debuggable provenance string using <namespace>:<name> convention.
// Leave null if no meaningful source. No user PII.
//
// Examples:
//   "cat:Urgent#string"         — category string match (implicit condition)
//   "cond:Escalated.profile"    — explicit profile condition on Escalated
//   "action:Done.remove"        — RemoveAction triggered by Done category
//   "nlp:date"                  — NLP date parser assigned to When
//   "nlp:name:Sarah"            — NLP entity extraction matched Sarah
//   "import:bulk-2026-02-15"    — bulk import default assignment
//   "subsumption:Projects"      — inherited from parent category Projects

RecurrenceRule {
  frequency:    enum { daily, weekly, monthly, yearly }
  interval:     Integer (default 1)       // e.g., 2 = every other
  day_of_week:  Optional<enum { mon, tue, wed, thu, fri, sat, sun }>
  day_of_month: Optional<Integer>         // e.g., 15 for "the 15th"
  starts_at:    DateTime                  // first occurrence (with time + timezone)
  timezone:     IANA_TZ
  end_date:     Optional<Date>            // null = no end
}
```

**Key behaviors:**

- An item's `text` is the primary content. It is never empty.
- An item can be assigned to zero or more categories (multifiling).
- The `entry_date` is always populated on creation; it is never null.
- The `when_date` is typically inferred by the NLP date parser from the item text,
  but can be manually overridden.
- The `note` is secondary content — details, context, longer text. Think of the item
  as the headline and the note as the body. Notes have no practical length limit
  (the original Agenda documentation described them as "unlimited"). The UI may
  truncate display but must preserve full content.
- Notes are **searchable** — full-text search includes note content, not just item text.
- Notes are indicated in grid views by a visual marker (e.g., a `♪` or `📝` icon) so
  the user can see at a glance which items have attached notes.
- Items are *never* owned by a single view or category. They exist in the database
  independently. Views and categories reference them.
- The `values` map stores typed data for column-categories. When a category has a
  `value_type`, items assigned to it carry a corresponding value. For example, an item
  assigned to an "Amount" category (numeric type) stores the actual dollar figure in
  `values["Amount"]`. Assigning a value implicitly assigns the category. An assignment
  without a value is a boolean membership (the classic case).
- **Values/assignments invariant**: Setting a value on an item for a column-category
  automatically creates an assignment entry (source: `manual`) if one doesn't exist.
  Removing an assignment for a column-category clears any corresponding value. These
  must always be kept in sync — there is no valid state where a value exists without
  an assignment or vice versa for column-categories.
- **Assignment provenance** enables the "why is this here?" query — the UI can display
  the source of each assignment (manual, auto-match with confidence score, suggestion
  accepted, fired by action, or inherited via subsumption).
- Assignments with source `manual` or `suggestion_accepted` are **sticky**: they are
  never automatically removed by the rule engine, even if the matched text is later
  edited away. Only the user can remove them explicitly.
- Assignments with source `auto_match` are also sticky once made — the rule engine does
  not retroactively remove assignments from prior passes. Re-evaluation only adds new
  assignments; it never revokes existing ones. Actions may still remove assignments via
  `RemoveAction`, but the rule engine itself never revokes matches.
- The `rejected_suggestions` set prevents re-suggestion of the same category for the
  same item. If the item text changes materially (e.g., Levenshtein edit distance > 20%
  of original length, or other implementation-defined threshold), the rejected set is
  cleared to allow fresh re-evaluation.
- **Recurring items**: (Phase 3) When a recurring item is marked `Done`, the
  system generates a new item with the same text, the `when_date` advanced to
  the next occurrence per the recurrence rule, and the same category
  assignments (minus `Done`). The completed instance retains its `Done`
  assignment and `done_date`. The new instance is a separate item linked to the
  original via `recurrence_series_id` / `recurrence_parent_item_id`.

### 2.2 Category

A Category is a named concept used to organize items. Categories form a tree hierarchy.

```
Category {
  id:                  UUID
  name:                String
  aliases:             List<String>           // alternative names for text matching
  parent:              Optional<CategoryID>   // null = root-level category
  children:            Ordered<List<CategoryID>>
  is_exclusive:        Boolean (default false) // if true, an item can be in at most one child
  value_type:          Optional<enum { text, numeric, date }>  // if set, items carry typed values
  numeric_precision:   Optional<Integer> // decimal places for numeric display (default 2)
  note:                Optional<String>       // documents the intent of the category
  enable_implicit_string: Boolean (default true) // disable to opt-out of name-based matching
  condition_mode:      enum { any, all } (default any) // OR vs AND for conditions
  conditions:          List<Condition>        // rules for automatic assignment
  actions:             List<Action>           // triggered when an item is assigned here
  allow_delete_action: Boolean (default false) // must be true to attach a DeleteAction
}
```

**Category roles:**

- **Column categories** — categories with a `value_type` are used as typed data columns
  in views. Users enter values directly into the column cell (e.g., typing "500" into
  an "Amount" column stores `NumericValue(500)` on the item). Column categories serve
  as both classification tags and data fields simultaneously.
- **Boolean categories** — categories without a `value_type` (the default). Assignment
  is a simple membership flag. These are used for tagging, filtering, and sectioning.
- **Hidden categories** — categories not displayed as columns in any view, used purely
  for classification, filtering, search, and automation triggers.

**Key behaviors:**

- **Subsumption (inheritance)**: If an item is assigned to a child category, it is
  implicitly assigned to all ancestor categories up to the root. If item X is in
  "Bug Fixes" which is a child of "Engineering", then X appears in "Engineering" views.
- **Mutual exclusion**: If a category is marked `exclusive`, an item assigned to one
  child will be automatically unassigned from sibling children. Example: "Priority"
  is exclusive with children "High", "Medium", "Low" — an item can only be one.
- **Text matching**: By default, every category has an implicit string condition based
  on its name. An item containing the word "meeting" may auto-assign to a "Meetings"
  category without the user doing anything. Set `enable_implicit_string = false` to
  disable this for specific categories (e.g., "Done" shouldn't match text containing
  the word "done").
- **Aliases**: A category "Fred Smith" might have aliases ["Fred", "Smith", "FS"].
  These broaden the text matching surface.
- **Condition composition**: `condition_mode = any` means conditions are ORed.
  `condition_mode = all` means conditions are ANDed.
- **Name safety (MVP)**: Category names are globally unique (case-insensitive).
  Reserved names (`When`, `Entry`, `Done`, `Entry When Done`) cannot be reused.

### 2.3 Category Hierarchy

The hierarchy is a forest (multiple root categories). Several special **reserved
categories** are built-in and always exist:

- `When` — a date-type category. Items with dates parsed from text are automatically
  assigned to date-range subcategories (Overdue, Today, Tomorrow, This Week, Next Week,
  This Month, Future, No Date).
- `Entry` — automatically tracks when items were created.
- `Done` — a reserved boolean category. When an item is assigned to `Done`:
  - The item's `is_done` flag is set to `true` and `done_date` is recorded.
  - The `Done` category functions as a standard category for filtering (e.g., views
    can exclude `Done` items).
  - Actions attached to `Done` fire normally (e.g., a `RemoveAction` to unassign
    from active project categories, making the item disappear from work views).
  - If the item has a `recurrence` rule, the system generates the next instance.
- `Entry When Done` — a **sequencing trigger** category. When an item is assigned to
  `Done`, the system first processes all of `Done`'s own actions (e.g., `RemoveAction`
  to unassign from active project categories). After `Done` processing completes and
  the item reaches a stable state, the system then assigns the item to `Entry When
  Done`, which fires its own actions. This two-phase design allows users to separate
  "what happens immediately on completion" (actions on `Done`) from "what happens
  after completion cleanup" (actions on `Entry When Done`). Example: `Done` removes
  the item from "Active Projects"; then `Entry When Done` assigns it to "Archive" and
  triggers an export. Without this separation, the archive action might capture stale
  assignment state.

Reserved categories cannot be deleted. They can have user-defined actions and conditions
attached to them like any other category.

**`When` subcategories** are virtual computed buckets, **not stored Category records**.
They are evaluated at query time based on the current system date and the system's local
timezone. The standard set is: `Overdue`, `Today`, `Tomorrow`, `This Week`, `Next Week`,
`This Month`, `Future`, `No Date`. An item's `when_date` determines which bucket it
falls into; items with no `when_date` fall into `No Date`.

**Virtual category representation:**

These computed buckets are NOT CategoryIDs and must not appear in the category manager
as editable entities. They are represented in queries using virtual tokens:

```
enum WhenBucket {
  overdue,
  today,
  tomorrow,
  this_week,
  next_week,
  this_month,
  future,
  no_date
}

// Virtual categories are queryable but evaluated as date range tests, not category lookups
VirtualCategory = WhenBucket(WhenBucket)
```

The `When` parent category itself is a real stored Category record; its children are
computed projections of time. See §2.4 Query model for how virtual categories are
referenced in view and section criteria.

The hierarchy is **the rule engine**. When an item is created or modified, it is queued
for processing. The engine walks the hierarchy in depth-first order, evaluating each
category's conditions against the item.

### 2.4 View

A View is a saved, dynamic, editable projection of items.

```
View {
  id:           UUID
  name:         String
  criteria:     Query                  // selects which items appear
  sections:     Ordered<List<Section>> // how items are grouped
  columns:      Ordered<List<Column>>  // what annotations appear
  sort:         Optional<SortSpec>
  section_membership: enum {
                        show_in_all_matching_sections,
                        first_match_only
                      } (default show_in_all_matching_sections)
  show_unmatched: Boolean (default true)
  unmatched_label: String (default "Unassigned")
  remove_from_view_unassign: Set<CategoryID> // explicit semantics for "remove from view"
}

Section {
  title:        String
  criteria:     Query                  // evaluated within the parent view's item set
  on_insert_assign: Set<CategoryID>    // assignments added when user inserts in section
  on_remove_unassign: Set<CategoryID>  // assignments removed when user removes from section
  show_children: Boolean (default false)
                // if true and criteria is a single category include, generate subsections
                // for each direct child of that category (see below)
}

// show_children behavior:
//
// Activates ONLY when section criteria is:
//   - Single include of one category (e.g., include: [Projects])
//   - No excludes, virtual_include/exclude, date_range, or text_search
//
// When active, auto-generates subsections (one per direct child):
//   - title: child category name
//   - criteria: parent section criteria + include(child category)
//   - on_insert_assign: inherits parent's on_insert_assign + child category
//   - on_remove_unassign: inherits parent's on_remove_unassign
//
// Rendering:
//   - Items may appear in multiple subsections if assigned to multiple children,
//     UNLESS the parent category is exclusive, in which case items appear in at most one.
//   - Items matching parent criteria but no child go to the view's unmatched section
//     (if show_unmatched = true); they don't silently vanish.
//   - Subsections are ordered using the parent category's stored child order.
//   - Depth is one level only (no recursive grandchildren).
//   - Subsections inherit the parent view's columns, sort, and computations.
//
// Example:
//   Section { title: "Projects", criteria: { include: [Projects] }, show_children: true }
//   → Generates subsections: "Project Alpha", "Project Beta", "Project Gamma"

Column {
  heading:      CategoryID             // shows assignments under this category subtree
  width:        Integer
  // Note: Column computations (sum, average, max, min, count) are deferred to Phase 2
}

Query {
  include:          Set<CategoryID>        // items must be assigned to ALL of these
  exclude:          Set<CategoryID>        // items must NOT be assigned to ANY of these
  virtual_include:  Set<VirtualCategory>   // items must match ALL virtual buckets
  virtual_exclude:  Set<VirtualCategory>   // items must NOT match ANY virtual buckets
  date_range:       Optional<DateRange>
  text_search:      Optional<String>       // full-text search across item text AND notes
}

// Virtual categories (currently only When buckets) are evaluated as date tests,
// not category table lookups. Example:
//
//   Query {
//     include: [Sarah],
//     virtual_include: [WhenBucket(today)],
//     exclude: [Done]
//   }
//
// This query selects items assigned to Sarah, with when_date matching today's date,
// and not assigned to Done. The evaluator maps WhenBucket(today) to a date range test
// rather than a category assignment check.
```

**Key behaviors:**

- Views are *live*. When items or categories change, views update immediately.
- Views support **edit-through semantics**:
  - inserting an item in a section assigns all categories in `on_insert_assign` and all
    categories in `view.criteria.include`.
  - removing an item from a section unassigns categories listed in
    `section.on_remove_unassign` (non-destructive).
  - removing an item from the view (not a specific section) unassigns categories listed in
    `view.remove_from_view_unassign` (non-destructive).
- A separate `delete` command permanently removes an item from the database.
- Switching between views is a primary navigation action (not a secondary menu).
- Query synthesis: when the user selects categories for inclusion, the system auto-builds
  an appropriate boolean expression. Exclusive sibling categories are ORed (since ANDing
  them would always produce an empty set). Non-exclusive categories are ANDed.
- Section membership is deterministic:
  - default behavior: an item appears in every matching section.
  - optional behavior: first matching section wins (`first_match_only`).
  - if no sections match and `show_unmatched = true`, item appears under
    `unmatched_label`.
- **Unmatched items (generated section)**: When `show_unmatched = true`, the system
  generates an implicit section for items matching view criteria but not matching any
  explicit section criteria. This generated section has:
  - title: `view.unmatched_label`
  - criteria: (implicit - computed at query time as "items in view NOT in any explicit section")
  - `on_insert_assign`: `view.criteria.include`
  - `on_remove_unassign`: `view.remove_from_view_unassign`

  Items appear in explicit sections OR the unmatched section, never both. The unmatched
  section only shows items that don't pass any explicit section's criteria. This ensures
  items never vanish from a view due to section configuration.

- **Section criteria evaluation**: Sections use full Query syntax (include/exclude sets),
  enabling compound criteria like "Urgent AND Blocked" or "Project Alpha but NOT Done".
  This is essential for real-world triage workflows where sections represent boolean
  combinations of categories, not just single-category membership.

### 2.5 Conditions and Actions

Conditions determine when an item should be automatically assigned to a category.
Actions fire when an assignment (automatic or manual) occurs.

```
Condition = StringCondition | ProfileCondition | DateCondition | ValidationCondition

StringCondition {
  // Implicit from category name + aliases (unless enable_implicit_string = false).
  // Compares item text against category name after:
  //   - suffix stripping / stemming
  //   - proper name detection
  //   - date/time literal detection
  //   - limited syntactic analysis
  // Match strength is compared against the global confidence_threshold.
  //
  // In 2026, this will likely be implemented with or augmented by LLM-based
  // classification for better semantic understanding (e.g., "March Madness"
  // correctly identified as an event, not the month March).
}

ProfileCondition {
  // A query (same form as View criteria) evaluated against the item's
  // current assignments. Enables if-then rules:
  // "If item is assigned to both 'Project Alpha' AND 'Urgent', assign to 'Escalated'"
  criteria: Query
}

DateCondition {
  // Tests one of the item's dates against a range.
  date_field:   enum { entry_date, when_date, done_date }
  range:        DateRange  // can be relative, e.g., "within 3 days", "overdue"
}

Action = AssignAction | DateAction | RemoveAction | DeleteAction

AssignAction {
  // Push item onto additional categories
  target_categories: Set<CategoryID>
}

DateAction {
  // Set or modify one of the item's date fields
  target_field: enum { when_date, done_date }
  expression:   DateExpression
}

RemoveAction {
  // Unassign the item from a specific category or set of categories.
  // e.g., marking an item "done" removes it from active views
  // without deleting it. This is the non-destructive equivalent.
  target_categories: Set<CategoryID>  // categories to unassign from
}

DeleteAction {
  // Permanently remove the item from the database. Use with extreme caution.
  // This is distinct from RemoveAction — delete is data destruction.
  //
  // Guardrails:
  // - DeleteAction can only be attached to a category whose
  //   `allow_delete_action` flag is explicitly set to true.
  // - Every deletion triggered by a DeleteAction is logged to the
  //   undo history and to a persistent deletion log with the item's
  //   full content, assignments, and the triggering category.
  // - DeleteAction is never created by default on any category,
  //   including reserved categories.
}
```

**Processing model:**

1. When an item is created or modified, it enters the processing queue.
2. The engine walks the category hierarchy depth-first.
3. For each category, conditions are evaluated using that category's `condition_mode`.
4. For string-based matches, confidence is routed by classification settings (§2.8):
   assign, suggest, or prompt based on confidence thresholds and assignment mode.
5. For each accepted assignment (automatic or manual), the engine writes
   `assignments[category_id]` with provenance.
6. For each new assignment, it fires that category's actions.
7. If an action modifies the item (e.g., adds another assignment, changes a date), the
   item re-enters the queue for another pass.
8. Processing terminates when a pass produces no new changes (fixed-point).
9. **Termination guarantees**:
   - A maximum of **10 passes** is enforced. If the fixed-point is not reached in 10
     passes, processing halts and a warning is logged identifying the categories still
     producing changes.
   - The cycle detector tracks `(ItemID, CategoryID)` pairs. If the same assignment is
     attempted twice in the same processing run, it is skipped.
   - During cascade processing, assignments are **monotonic** — the engine may add
     assignments and fire actions, but `RemoveAction` results are deferred until the
     current cascade completes. This prevents oscillation where removing assignment A
     re-triggers the condition that adds assignment A.

### 2.6 Validation and Entry-Time Logic

Categories can enforce constraints on item entry and assignment, not just trigger
side-effects after the fact.

```
Condition = StringCondition | ProfileCondition | DateCondition | ValidationCondition

ValidationCondition {
  // Evaluated before an assignment is accepted. If it fails, the assignment
  // is rejected (or the user is warned, depending on assignment_mode setting).
  // Enables cross-field validation and preconditions.
  required_assignments: Set<CategoryID>   // item must already be assigned to these
  required_values:      Map<CategoryID, ValuePredicate>  // e.g., "Amount > 0"
  message:              String            // shown to user on rejection
}

ValuePredicate = GreaterThan(Value) | LessThan(Value) | NotEmpty | Equals(Value)
```

**Validation behaviors:**

- **Preconditions on entry**: A category can require that certain other categories are
  already assigned before an item can be filed there. Example: "Escalated" requires
  both "Urgent" and a project category.
- **Cross-field validation**: A validation condition can check typed values in other
  columns. Example: an "Approved" category requires `Amount < 10000`.
- **Rejection behavior**:
  - `automatic`: assignment is rejected and logged in non-blocking status output.
  - `assisted`: assignment appears in suggestion review as a warning item.
  - `manual`: user is prompted with the validation message and may override.
- **Authoring UX (MVP)**: validation rules are created/edited in the Category Manager
  inspector (Validation tab) with a live preview showing the count of items that would
  currently fail the rule.

### 2.7 Scripting and Programmable Automation

Lotus Agenda included a full built-in programming language used for data validation,
automatic view creation (e.g., auto-creating a review view per person), export triggers,
cross-view field linking, and conditional business logic. It also supported recordable
macros for repetitive operations.

**For the MVP**, the declarative condition/action system (§2.5) combined with validation
conditions (§2.6) covers the most important automation patterns without requiring a
scripting language. The following capabilities are **deferred to Phase 2** but are
documented here as design intent:

- **Programmatic view creation** — actions that create new views (e.g., "when a new
  person is added, create a 1:1 review view for them").
- **Value transformation** — actions that compute or transform field values based on
  other fields (e.g., "set total = quantity × price").
- **Cross-view field linking** — a value in one view's column automatically reflects
  or aggregates values from another view.
- **Trigger surface expansion** — triggers on value change, on view entry, on item
  deletion (currently only assignment triggers exist).
- **Recordable macros** — user-recordable sequences of commands for repetitive operations.

These are listed here (not in §6 Out of Scope) because they are part of the original
Agenda design and will be needed to support the full range of "application builder"
use cases. They are deferred only because the declarative subset comes first.

### 2.8 Classification Settings (Modernized for LLM Era)

Three global settings control the automatic classification engine's behavior. In 2026,
classification will use LLMs (in addition to or instead of rule-based string matching).
LLMs naturally produce confidence scores, making this threshold system directly applicable.

```
ClassificationSettings {
  // Auto-assign when model confidence >= this threshold
  confidence_threshold: Float (default 0.5)

  // Show suggestions when confidence >= this (must be <= confidence_threshold)
  review_threshold: Float (default 0.3)

  // How to handle automatic categorization
  assignment_mode: enum {
    automatic,  // high confidence auto-assigns; below threshold ignores
    assisted,   // high auto-assigns; medium suggests; low ignores
    manual      // all above review_threshold prompt; below ignores
  }

  Default: assisted
}
```

**Behavior table:**

| Confidence | automatic | assisted | manual |
|------------|-----------|----------|--------|
| ≥ confidence_threshold (0.5) | ✅ Auto-assign | ✅ Auto-assign | 💬 Prompt |
| review_threshold to confidence_threshold (0.3–0.5) | ❌ Ignore | 💡 Queue suggestion | 💬 Prompt |
| < review_threshold (0.3) | ❌ Ignore | ❌ Ignore | ❌ Ignore |

**Mode descriptions:**

- **automatic mode**: Auto-assign matches above confidence_threshold. Below threshold,
  ignore. This is the most aggressive mode — the system makes all decisions.

- **assisted mode** (recommended default): Auto-assign high-confidence matches (≥ threshold).
  Queue medium-confidence matches (between review_threshold and confidence_threshold) for
  user review. Ignore low-confidence matches. This creates a collaborative workflow where
  the system handles obvious cases and asks for help on uncertain ones.

- **manual mode**: Prompt for all matches above review_threshold. The user approves or
  rejects each suggestion. During **bulk operations** (import, retroactive assignment on
  category creation), `manual` mode switches to a batch review list presented after the
  operation completes to avoid hundreds of sequential prompts.

**Special behaviors:**

- When a suggestion is rejected, it is added to the item's `rejected_suggestions` set.
  The same category will not be suggested again for that item unless the item text
  changes materially (edit distance > 20% of original length).
- Accepted suggestions (whether from queue or prompt) create assignments with
  `source: suggestion_accepted`, which are sticky and never auto-removed.

### 2.9 Undo

Single-level undo is an MVP feature. The system maintains an **in-memory** undo stack of
the most recent operation. Undoable operations include:

- Item creation (undo = delete the item)
- Item deletion (undo = restore the item with all assignments)
- Item text edit (undo = restore previous text, re-run classification engine)
- Category assignment/unassignment (undo = reverse the assignment change)
- Item move between sections (undo = reverse the implied assignment changes)
- Bulk operations (e.g., retroactive assignment) are undoable as a single unit

The undo stack depth is **1** for the MVP (single undo, no redo). This is sufficient to
recover from the most common error: accidentally moving or removing an item in a view.

**Persistence:** The undo stack is in-memory only and resets on application restart.
This keeps the implementation simple for MVP. Phase 2 may add persistent undo/redo.

A separate **deletion log** persists permanently to disk and is not affected by the undo
stack. Any item deleted (whether by user action or `DeleteAction`) is recorded in the
deletion log with full content, assignments, and timestamp. The deletion log can be
browsed and items can be restored from it even after application restart.

---

## 3. NLP: Date and Entity Extraction

### 3.1 Date Parsing

The date parser extracts temporal expressions from item text and populates `when_date`.
For recurring expressions, it also populates `recurrence`. Parsing is timezone-aware and
uses the database timezone (default: system local timezone). This was one of Agenda's
most polished features. The parser must handle:

**Absolute dates:**
- "May 25, 2026"
- "2026-05-25"
- "12/5/26"
- "December 5"

**Relative dates:**
- "today", "tomorrow", "yesterday"
- "next Tuesday", "this Friday", "last Monday"
- "in 3 days", "in two weeks"
- "a week from Thursday"
- "the day after tomorrow"
- "two weeks from last Tuesday"
- "the last week in June"

**Recurring dates:**
- "every Tuesday"
- "every four months starting Tuesday"
- "the 1st of every month"
- "every other Friday"

Recurring parse output must include:
- normalized recurrence frequency/interval
- recurrence anchor/start datetime
- initial concrete `when_date` (next occurrence)

**Times:**
- "at 3pm", "at 15:00", "at noon", "at midnight"
- "3:30 PM"

**Compound expressions:**
- "next Tuesday at 3pm"
- "Call Sarah this Friday to give her feedback on her proposal"
  → extracts: this Friday
- "Meeting with Tom next Tuesday at 10 AM"
  → extracts: next Tuesday, 10 AM
- "Check data retention policy every four months starting Tuesday"
  → extracts: recurring, every 4 months, starting next Tuesday

**Non-date text must not be misinterpreted:**
- "March forward with the plan" → "March" is not a month here
- "May I ask a question?" → "May" is not a month here
- "I met her in the spring" → vague, do not assign a date

### 3.2 Entity Extraction

Beyond dates, the system should detect:

- **Proper names**: "Call Fred Smith" → potential match against a "Fred Smith" category
- **Action verbs**: "Call", "Email", "Meet", "Review", "Buy" → potential category matches
- **Amounts/currency**: "$500 for the new monitor" → for expense-tracking views

Entity extraction drives the string-matching condition system. It does not need to be
perfect — the classification settings exist precisely to handle uncertainty. In 2026,
LLM-based classification will significantly improve semantic understanding beyond
rule-based entity extraction.

---

## 4. Architecture Constraints

### 4.1 Layered Architecture

```
┌─────────────────────────────┐
│  Frontend (TUI / macOS / …) │   Presentation layer. Renders views.
├─────────────────────────────┤   Sends commands. Subscribes to changes.
│  Command Interface (API)    │   Stateless command handlers.
├─────────────────────────────┤   create_item, assign, create_view, etc.
│  Core Engine                │   Rule engine, query evaluator,
│  (Items, Categories, Views) │   NLP pipeline, classification processor.
├─────────────────────────────┤   Pure logic. No I/O.
│  Storage Adapter            │   Persistence. File-based for prototype.
└─────────────────────────────┘   SQLite or flat files.
```

The core engine MUST have zero UI dependencies. It should be testable as a pure library
that takes commands and emits events/state.

### 4.2 Storage (Prototype)

For the throwaway prototype, use the simplest possible persistence:

- A single JSON or SQLite file containing all items, categories, views, and settings.
- File path configurable. Default: `~/.agenda/default.ag`
- **Crash-safety requirements**:
  - **SQLite**: use WAL (Write-Ahead Logging) mode.
  - **JSON**: write to temp file + fsync + atomic rename.
- No server. No network. No sync. Local-only.

### 4.3 TUI (Prototype Frontend)

The TUI should evoke the spirit of the original DOS interface while being usable in a
modern terminal:

- Full-screen application using a TUI framework (e.g., ratatui for Rust, bubbletea for
  Go, textual for Python, blessed/ink for Node).
- Primary interaction: a view fills the screen. Items are rows. Categories are columns
  or section headings.
- A persistent input bar at the bottom for typing new items (free-form). The input bar
  is **context-sensitive**: if the cursor is positioned within a section, new items are
  assigned to that section's heading category (edit-through semantics). If no section is
  focused (e.g., the cursor is in the input bar directly), the item is created with no
  section assignment — it enters the database and is processed by the classification engine,
  but will only appear in sections it matches. A subtle indicator in the input bar shows
  the current section context (e.g., `[→ Urgent]` or `[no section]`).
- `F8` or equivalent to switch views (view browser).
- `/` opens in-view text search (incremental filter over item text + notes).
- `F9` or equivalent opens Category Manager (create/edit/reparent categories,
  conditions, actions, and validations).
- `Tab` / arrow keys to navigate items and columns.
- Direct editing of item text and category assignments inline.
- A note indicator (e.g., `♪` or `📎`) next to items that have attached notes.
- A status indicator (e.g., `?`) when pending assignment suggestions exist.
- **Search**: Two modes:
  - **View filter** (`/` key): narrows the current view to items matching the search
    text. The filter is applied on top of the view's criteria. Pressing `Esc` clears
    the filter and restores the full view. Searches match against item text and note
    content.
  - **Global search** (`Ctrl-/` or equivalent): searches across all items regardless
    of the current view. Results are displayed in a transient "Search Results" view.
    The user can jump to any result item, which navigates to a view containing that item.
- **Inline category creation**: Categories can be created without leaving the current view:
  - Typing a new name into a column header cell creates a new category and adds it as
    a column.
  - A quick-create command (e.g., `:cat NewCategoryName` or `Ctrl-K`) in the input bar
    creates a category and triggers retroactive assignment.
  - The Category Manager (`F9`) remains available for full hierarchy editing, alias
    management, condition/action configuration, and bulk operations.
- **Inspect assignments** (`i` key or equivalent on a selected item): displays all
  category assignments for the item with provenance information (source, confidence,
  timestamp, origin). Allows the user to understand *why* an item appears where it does.
- **Undo** (`Ctrl-Z`): reverts the most recent operation (see §2.9).

### 4.4 Performance Constraints

- **View switching** must complete in <100ms for databases under 10,000 items.
- **Retroactive assignment** (triggered by category creation against existing items)
  must run **asynchronously** and not block the UI. A progress indicator shows
  "Evaluating N items..." during retroactive scans. The user can continue interacting
  with the application while evaluation runs in the background. Results appear
  incrementally as items are processed.
- **Rule engine processing** for a single item change should complete in <50ms for
  category hierarchies under 500 categories.

---

## 5. Scenarios (NLSpec)

> These scenarios are end-to-end user stories in the Cem Kaner sense. They describe
> observable behavior from the user's perspective. A coding agent should be able to read
> each scenario and implement/validate the described behavior.
>
> **Satisfaction** is defined as: of all observed trajectories through a scenario, what
> fraction likely satisfy the user's intent? Scenarios are not boolean pass/fail — they
> describe desired outcomes with room for heuristic judgment.

---

### Scenario 01: First Launch — Empty State

**Context:** User launches the application for the first time with no existing database.

**Expected behavior:**

1. The system creates a new database with four reserved built-in categories:
   - `When` (date-type, with auto-generated children like Today, This Week, This Month)
   - `Entry` (date-type, auto-tracking creation dates)
   - `Done` (reserved boolean completion category)
   - `Entry When Done` (reserved trigger category assigned when an item is marked Done)
2. A default view named "All Items" is created, showing all items with a `When` column.
3. The screen displays the empty default view with a clear prompt or input area indicating
   the user can start typing items.
4. No wizard, no setup dialog, no onboarding modal. The interface is immediately usable.

**Satisfaction criteria:** The user can type their first item within 2 seconds of launch
without clicking anything or reading instructions.

---

### Scenario 02: Free-Form Item Entry with Date Extraction

**Context:** The database is empty. The user types into the input bar:

```
Call Sarah this Friday to give her feedback on her proposal
```

**Expected behavior:**

1. A new item is created with the full text as entered.
2. The NLP parser extracts "this Friday" and computes the correct absolute date. The
   item's `when_date` is set to that date.
3. The item appears in the current view. If the view has a `When` column, the parsed
   date is displayed.
4. No categories beyond `When` exist yet. The item is assigned to the appropriate `When`
   subcategory (e.g., "This Week" if Friday is within the current week).
5. The original text is preserved exactly as typed. The system does NOT rewrite or
   reformat the item text.

**Satisfaction criteria:** The date is correctly parsed at least 95% of the time for
common English date expressions. The item text is never mutated.

---

### Scenario 03: Creating a Category and Retroactive Assignment

**Context:** The database has 10 items, some of which contain the word "Sarah" in their
text. The user creates a new category named "Sarah."

**Expected behavior:**

1. The category "Sarah" is created at the root level of the hierarchy.
2. The classification engine runs against all existing items asynchronously.
3. Items containing "Sarah" (via string matching or LLM classification) are evaluated.
   Those exceeding the confidence_threshold are assigned to the "Sarah" category.
4. Routing follows assignment_mode policy:
   - `automatic`: matches >= confidence_threshold are assigned immediately.
   - `assisted`: matches >= confidence_threshold are assigned; lower-confidence matches in
     the review band are queued under `?`.
   - `manual`: matches >= review_threshold trigger prompts (batched for bulk operations).
5. Any view that includes "Sarah" as a section or column immediately reflects the
   new assignments.

**Satisfaction criteria:** Retroactive assignment is automatic and immediate. The user
does not need to manually re-file old items into a new category. This is the "aha moment"
that distinguishes this system from a tag-based notes app.

---

### Scenario 04: Multifiling — One Item, Many Categories

**Context:** The user has categories "Engineering", "Urgent", and "Sarah". They type:

```
Review Sarah's PR for the auth service — urgent, need it before deploy
```

**Expected behavior:**

1. A new item is created.
2. The string matcher/classifier detects "Sarah" → assigns to "Sarah" category.
3. The string matcher/classifier detects "urgent" → assigns to "Urgent" category.
4. If the user has previously associated "PR" or "auth service" with "Engineering",
   that assignment is also made.
5. The item now appears in views filtered by "Sarah", views filtered by "Urgent",
   and views filtered by "Engineering" — all simultaneously.
6. The item exists once in the database. It is not duplicated.

**Satisfaction criteria:** The user can switch between a "Sarah" view, an "Urgent" view,
and an "Engineering" view and see the same item in all three. Editing the item text in
any view updates it everywhere.

---

### Scenario 05: View Creation and Edit-Through Semantics

**Context:** The user creates a new view called "Sarah 1:1" with criteria: include "Sarah",
exclude "Done". The view has sections:
- "Urgent" (criteria: `include: [Urgent]`, on_insert_assign: `[Urgent]`)
- "Normal" (criteria: `exclude: [Urgent]`, on_insert_assign: `[Normal]`)

The view has `show_unmatched: true` and a column showing "When" dates.

**Step A — Viewing:**

1. The view displays all items assigned to "Sarah" that are not marked Done.
2. Items assigned to "Urgent" appear under the "Urgent" section.
3. Items assigned to "Normal" (and not Urgent) appear under the "Normal" section.
4. Items that match the view criteria but match neither section appear under the
   "Unassigned" section (the generated unmatched section).
5. The "When" column shows parsed dates for each item.

**Step B — Inserting through the view:**

6. The user navigates to the "Urgent" section and types a new item:
   ```
   Discuss deployment timeline with Sarah
   ```
7. The new item is automatically assigned to BOTH "Sarah" (from view criteria include set)
   AND "Urgent" (from section `on_insert_assign`). The user did not manually assign either.

**Step C — Moving between sections:**

8. The user drags/moves an item from the "Urgent" section to the "Normal" section.
9. The item is unassigned from "Urgent" and assigned to "Normal" based on section
   on_remove/on_insert semantics. It remains assigned to "Sarah".
10. If "Urgent" and "Normal" are exclusive siblings under a "Priority" parent, mutual
   exclusion is enforced automatically.

**Step D — Removing from view vs. deleting:**

11. The user "removes" an item from the view. The item is unassigned from categories in
    `remove_from_view_unassign` (here: "Sarah") but still exists in the database and may
    appear in other views.
12. The user "deletes" a different item. It is permanently removed from the database
    and disappears from all views.

**Satisfaction criteria:** The user never explicitly calls an "assign" or "categorize"
command during normal view interaction. Filing happens as a side effect of where they
place items. The mental model is "I'm organizing my list" not "I'm tagging database records."

---

### Scenario 06: Hierarchical Categories with Inheritance

**Context:** The user builds this category hierarchy:

```
Projects
├── Project Alpha
│   ├── Backend
│   └── Frontend
├── Project Beta
└── Project Gamma
```

**Expected behavior:**

1. An item assigned to "Backend" (under "Project Alpha") is implicitly visible in
   views filtered by "Project Alpha" and "Projects".
2. A view filtered by "Projects" shows ALL items assigned to any descendant.
3. A view sectioned by the children of "Projects" shows three sections: Alpha, Beta,
   Gamma. Items assigned to "Backend" appear under the "Project Alpha" section.
4. If the user creates a new child under "Project Alpha" (e.g., "DevOps"), existing
   views auto-adapt: a view showing all "Project Alpha" children as sections gains
   a new "DevOps" section without manual reconfiguration.

**Satisfaction criteria:** The hierarchy behaves like a real ontology. Adding structure
never requires rebuilding views. Views are defined in terms of categories, and as the
category tree evolves, views evolve with it.

---

### Scenario 07: Automatic Assignment via Profile Conditions (If-Then Rules)

**Context:** The user configures a category "Escalated" with a profile condition:
"If item is assigned to BOTH 'Urgent' AND 'Project Alpha', assign to 'Escalated'."

They also configure an action on "Escalated": assign to "Notify:Manager".

**Expected behavior:**

1. The user types: `Auth service is down — urgent, blocking Alpha release`
2. String matching/classification assigns to "Urgent" and "Project Alpha".
3. The profile condition on "Escalated" fires: item is now also in "Escalated".
4. The action on "Escalated" fires: item is now also in "Notify:Manager".
5. The cascading assignment completes in a single processing pass (or at most two,
   with the cycle detector preventing runaway).

**Satisfaction criteria:** The user can build meaningful automation through the category
hierarchy without writing code. The rule engine is the hierarchy. Non-programmers can
create "if this then that" behavior by configuring category conditions.

---

### Scenario 08: Mutual Exclusion

**Context:** The user creates an exclusive category "Status" with children:
"Not Started", "In Progress", "Done".

**Expected behavior:**

1. The user assigns an item to "In Progress".
2. Later, the user assigns the same item to "Done".
3. The system automatically removes the "In Progress" assignment because "Status" is
   exclusive — an item can be in at most one child.
4. The item now shows "Done" in any view column displaying "Status".
5. No prompt or confirmation is needed. The exclusion is a structural constraint.

**Satisfaction criteria:** Exclusive categories enforce data consistency automatically.
The user never sees an item that is simultaneously "In Progress" and "Done."

---

### Scenario 09: Dynamic Date Categories

**Context:** Today is Wednesday, Feb 11, 2026. The built-in `When` category has
auto-generated children: "Overdue", "Today", "Tomorrow", "This Week", "Next Week",
"This Month", "Future", "No Date". These are virtual computed buckets, not stored
category records.

**Expected behavior:**

1. An item with `when_date` of Feb 10 (yesterday) appears under "Overdue".
2. An item with `when_date` of Feb 11 appears under "Today".
3. An item with `when_date` of Feb 14 appears under "This Week".
4. When the clock rolls past midnight to Feb 12:
   - The Feb 11 item moves from "Today" to "Overdue".
   - The Feb 12 item (if any) moves into "Today".
5. These transitions happen automatically. No user action required.
6. An item with no `when_date` appears under "No Date".
7. A view can filter by these buckets using virtual_include in its criteria:
   ```
   Query {
     include: [Work],
     virtual_include: [WhenBucket(today)],
     exclude: [Done]
   }
   ```
   This creates a "Work Today" view showing only work items due today.

**Satisfaction criteria:** The `When` hierarchy is always current. Items naturally flow
through temporal categories as time passes. The user's "Today" view always reflects
actual reality. Virtual buckets are queryable in view criteria without polluting the
category table.

---

### Scenario 10: Text Matching with Classification Control

**Context:** The user has a category "Marketing" and types the item:

```
Buy March Madness tickets for the marketing team outing
```

**Expected behavior at confidence_threshold = 0.3 (aggressive):**

1. "marketing" matches "Marketing" — assigned (high confidence).
2. "March" might weakly match a "March" date category — possibly assigned (false positive).

**Expected behavior at confidence_threshold = 0.7 (conservative):**

1. "marketing" matches "Marketing" — assigned (strong match, word is exact).
2. "March" is recognized as part of a compound noun phrase "March Madness" — not
   interpreted as a month. Not assigned to a date.

**Expected behavior at confidence_threshold = 0.5 (default) with assignment_mode = `assisted`:**

1. "marketing" → assigned automatically (high confidence).
2. "March" → queued as a suggestion with low confidence. The `?` indicator appears.
   The user can review and dismiss.

**Satisfaction criteria:** The classification settings give the user a tunable
tradeoff between automation and accuracy. The system is transparent about its confidence.

---

### Scenario 11: Recurring Items

**Context:** The user types:

```
Review infrastructure costs every month on the 15th
```

**Expected behavior:**

1. The item is created with a `RecurrenceRule { frequency: monthly, interval: 1,
   day_of_month: 15 }`. The `when_date` is set to the next 15th.
2. The item appears in the current or next appropriate date section.
3. When the user marks the item "Done", the system creates a new item — a separate
   database record — with identical text, the same category assignments (minus `Done`),
   and `when_date` advanced to the 15th of the following month. The new item's
   `recurrence_parent_item_id` links back to the completed item. Both items share the
   same `recurrence_series_id`. The new item carries the same `RecurrenceRule`.
4. The completed instance retains its `Done` assignment and `done_date`. It moves to
   the "Done" view/category. The new instance appears in the appropriate future date
   section.

**Satisfaction criteria:** Recurring items work like Agenda's original behavior. The
user types the recurrence in natural language and never interacts with a recurrence
configuration dialog.

---

### Scenario 12: View Switching as Primary Navigation

**Context:** The user has five views: "Today", "Sarah 1:1", "Project Alpha", "All Items",
and "Done This Week".

**Expected behavior:**

1. The user presses the view-switch key (e.g., F8 or Ctrl-V).
2. A list or picker of available views appears.
3. The user selects "Sarah 1:1" and the display immediately updates to show that view.
4. Alternatively, "next view" / "previous view" keys cycle through views sequentially.
5. View switching is near-instantaneous (<100ms for databases under 10,000 items).

**Satisfaction criteria:** Views are the primary way the user changes context. Switching
views feels like flipping pages in a book, not running a database query. The system is
responsive enough that the user builds a habit of switching views frequently.

---

### Scenario 13: Editing an Item Updates All Views

**Context:** An item "Review Sarah's PR for auth service" appears in three views:
"Sarah 1:1", "Engineering", and "Today".

**Expected behavior:**

1. The user edits the item text in the "Sarah 1:1" view, changing it to:
   "Review Sarah's PR for auth service — LGTM, just needs tests"
2. The modified text immediately appears in the "Engineering" and "Today" views.
3. The modification triggers a re-evaluation by the classification engine. If the new text
   matches additional categories (e.g., "Testing"), new assignments may occur.
4. If the edit removes a matched word (e.g., the user deletes "Sarah" from the text),
   the string-match assignment to "Sarah" is NOT automatically removed — assignments
   are sticky regardless of provenance (see §2.1). Only new text changes trigger new
   evaluations; the engine never revokes existing assignments.

**Satisfaction criteria:** There is one item, one truth. Edits propagate everywhere.
The user never has to wonder "which copy did I update?"

---

### Scenario 14: Category Reparenting Without Data Loss

**Context:** The user has a flat category "Design" at the root level. They decide it
should be a child of "Engineering".

**Expected behavior:**

1. The user moves "Design" to be a child of "Engineering" in the category manager.
2. All items assigned to "Design" are now implicitly visible under "Engineering"
   (via subsumption).
3. Views filtered by "Engineering" now include "Design" items.
4. Views that directly reference "Design" continue to work unchanged.
5. No items lose their assignments. No data is deleted or duplicated.

**Satisfaction criteria:** Reorganizing the category tree is a safe operation. The user
can refactor their organizational structure as their understanding evolves — exactly as
Agenda intended.

---

### Scenario 15: Importing Free-Form Text

**Context:** The user has a plain text file with paragraphs of notes separated by blank
lines. They import it.

**Expected behavior:**

1. Each paragraph (separated by double newlines) becomes a separate item.
2. The date parser runs on each item, extracting any temporal expressions.
3. The classification engine runs against all existing categories.
4. Items are assigned to matching categories according to classification settings.
5. The user can review all imported items in the "All Items" view and refine assignments.

**Satisfaction criteria:** Bulk import of messy, unstructured text is a first-class
operation. The system handles the "I have a pile of notes and I want to make sense of
them" use case gracefully. This was Agenda's secret weapon for research and reference
material.

---

### Scenario 16: Suggested Assignments Review

**Context:** assignment_mode is set to `assisted`. The user has been entering items. Several
string matches fell below confidence_threshold but above review_threshold.
The `?` indicator is visible.

**Expected behavior:**

1. The user activates the suggestion review (e.g., presses `?` or a review command).
2. A list of pending suggestions appears, each showing:
   - The item text
   - The suggested category
   - The match confidence (e.g., "weak", "moderate")
3. For each suggestion, the user can: Accept (assigns), Reject (dismisses), or
   Skip (leave pending).
4. Accepted assignments trigger actions, just like manual assignments.
5. Rejected suggestions are remembered — the same match will not be suggested again
   for that item unless the item text changes materially (e.g., >20% edit distance).

**Satisfaction criteria:** The suggestion system is a collaborative dialogue between the
user and the engine. It surfaces opportunities without being intrusive. Over time, the
user's accept/reject patterns refine what the system suggests (if learning is implemented
in a later phase).

---

### Scenario 17: Database Resilience — Crash Recovery

**Context:** The application crashes mid-operation (e.g., during a bulk import or
assignment cascade).

**Expected behavior:**

1. On next launch, the system detects an unclean shutdown.
2. The database is in a consistent state — either the last complete operation is
   reflected, or a write-ahead log allows recovery to the last consistent state.
3. No items are silently corrupted. No phantom assignments appear.
4. If recovery is not possible, the system reports what was lost (e.g., "3 items from
   the last import were not saved") rather than silently losing data.

**Satisfaction criteria:** The user trusts the system with their data. Lotus Agenda had
a notorious reputation for catastrophic file corruption. This is a known failure mode
that must be explicitly designed against.

---

### Scenario 18: Empty Category Cleanup

**Context:** Over time, the user has created many categories. Some now have zero items
assigned to them.

**Expected behavior:**

1. The category manager shows item counts next to each category.
2. A command or filter can list categories with zero assignments.
3. Deleting an empty category is simple and safe.
4. Deleting a non-empty category prompts: "Category 'X' has N items. Remove category
   only (items keep other assignments) or delete category and all items?"
5. Removing the category only is the default (safe) option.

**Satisfaction criteria:** The system supports housekeeping without risk. Categories are
lightweight and disposable. The user is never afraid to create a category because
cleanup is easy.

---

### Scenario 19: Quick-Add from Outside the Application

**Context:** The user is working in another application and has a thought they want to
capture.

**Expected behavior:**

1. A CLI command (e.g., `agenda add "Call Fred about the deploy schedule Friday"`)
   creates an item in the database without launching the full TUI.
2. The date parser and classification engine run against the new item.
3. Next time the user opens the TUI, the item is there, properly assigned.

**Satisfaction criteria:** Capture is fast and frictionless. The user doesn't need to
context-switch into the full application to record a thought. This supports the same
"capture first, organize later" philosophy as the original.

---

### Scenario 20: The "Agenda Moment" — Emergence of Structure

**Context:** A new user has been using the system for two weeks. They started by just
typing items — no categories, no views, no rules. Just raw text capture.

Now they have 150+ items and decide it's time to organize.

**Expected behavior:**

1. The user opens the category manager and creates their first category: "Work".
2. The classification engine retroactively scans all 150+ items. Those mentioning work-
   related terms are assigned. A progress indicator shows "Evaluating 150 items..."
   and the user can continue working while evaluation runs in the background.
3. The user creates more categories: "Personal", "Health", "Project X".
4. Each new category triggers retroactive assignment. The pile of items self-organizes.
5. The user creates a view filtered by "Work" + "This Week". Suddenly, their week's
   work tasks are laid out in front of them — from two weeks of unstructured notes.
6. The user realizes: "I didn't have to organize this upfront. The structure emerged."

**Satisfaction criteria:** This is the "aha moment." The system proves its value by
making retroactive organization feel like magic. If this scenario doesn't produce a
moment of delight, the implementation has missed the point of the entire project.

---

### Scenario 21: Understanding Why an Item Is Here

**Context:** The user is in the "Engineering" view and sees an item they don't recognize:
"Review the March deployment checklist." They wonder how it got assigned to Engineering.

**Expected behavior:**

1. The user selects the item and invokes the inspect command (e.g., `i` key).
2. The system displays all category assignments for the item, each with its provenance:
   - "Engineering" — source: `auto_match`, confidence: 0.72, origin: "deployment"
   - "When: This Month" — source: `subsumption` (computed from when_date: March 15)
   - "Checklist" — source: `manual`, assigned at: Feb 10 2026
3. The user can see at a glance which assignments were automatic vs manual.
4. From this view, the user can remove any assignment (which also removes it from the
   provenance record).

**Satisfaction criteria:** The user is never mystified by the system's organizational
decisions. Transparency builds trust. If the user can't understand why an item is where
it is, they will stop trusting the automatic assignment system.

---

### Scenario 22: Undo After Accidental Move

**Context:** The user accidentally drags an item from the "Urgent" section to "Normal"
in a view.

**Expected behavior:**

1. The item moves to "Normal" — it is unassigned from "Urgent" and (if applicable)
   assigned to "Normal".
2. The user immediately presses `Ctrl-Z`.
3. The item returns to the "Urgent" section. The assignment to "Urgent" is restored;
   the assignment to "Normal" is removed.
4. All views update to reflect the restored state.

**Satisfaction criteria:** The user feels safe manipulating items in views because
mistakes are recoverable. The undo operation is instantaneous and obvious.

---

### Scenario 23: Section Gaps and the Unmatched Section

**Context:** A view includes "Project Alpha" in its criteria but has explicit sections only
for "Urgent" and "Blocked". `show_unmatched` is true (default).

**Expected behavior:**

1. Items matching "Project Alpha" but matching neither "Urgent" nor "Blocked" remain visible.
2. These items appear under the view's unmatched section (label "Unassigned" by default).
3. No item silently disappears from the view because of section configuration.
4. The unmatched section behaves like a generated section: inserting an item there assigns
   it to the view criteria categories only (here: "Project Alpha").

**Satisfaction criteria:** Users never experience "I know this item exists but it vanished"
because of section logic. The unmatched section is a safety net ensuring all matching items
remain visible.

---

## 6. Out of Scope (MVP)

These features are explicitly deferred. They are listed here as a guard against
Chandler-style scope creep.

### Never (Out of Scope Permanently)
- **Contacts management** — This is not an address book.
- **Email integration** — This is not an email client.
- **Calendar UI** — Items have dates. That's not the same as needing a calendar widget.
- **Collaboration / multi-user** — Local, single-user only.
- **Sync / cloud** — Local file only. No server.
- **Mobile app** — TUI works over SSH if needed.
- **Printing** — Not in 2026.

### Phase 2 (Deferred but Planned)
- **LLM-powered categorization** — MVP uses rule-based string matching and produces
  confidence scores. The data model (§2.8) is designed for LLM integration. Phase 2
  will add LLM classifier support (OpenAI API, local models, or hybrid) to improve
  semantic understanding and reduce false positives.
- **Column computations** — Aggregate functions (sum, average, max, min, count) displayed
  in view column footers. Deferred to Phase 2 to keep MVP focused.
- **Multi-level undo/redo** — Single undo is MVP (§2.9). Full undo/redo stack is Phase 2.
- **Macro language / scripting** — Design intent documented in §2.7. Deferred to Phase 2.
- **Per-category confidence thresholds** — Thresholds are global for MVP. Per-category
  override is a Phase 2 enhancement.
- **Plugin system** — Premature for MVP. Build the core first.

---

## 7. Glossary

| Term | Definition |
|---|---|
| **Item** | An atomic unit of information: a sentence, phrase, task, fact, or thought. |
| **Category** | A named concept in a hierarchy used to organize items. Items are "assigned to" categories. |
| **Assignment** | The relationship between an item and a category, including provenance (source, confidence, timestamp, origin). One item can have many assignments (multifiling). |
| **View** | A saved, dynamic, editable projection of items filtered by category criteria. |
| **Section** | A grouping within a view defined by criteria. Items matching no section appear in the view's unmatched section when enabled. |
| **Unmatched Section** | A generated section (when `show_unmatched = true`) that displays items matching view criteria but not matching any explicit section criteria. Ensures items never vanish. |
| **Column** | An annotation in a view showing which subcategories an item is assigned to. |
| **Criteria / Query** | The selection filter for a view: which categories to include/exclude. |
| **Condition** | A rule attached to a category that determines when items should be auto-assigned. |
| **Action** | A side-effect triggered when an item is assigned to a category. |
| **Subsumption** | Child category assignments inherit upward: assigned to child → implicitly assigned to parent. |
| **Mutual Exclusion** | An exclusive category ensures an item can be assigned to at most one of its children. |
| **confidence_threshold** | Global threshold (0–1) for automatic assignment. Was called "Initiative" in earlier versions. |
| **review_threshold** | Global threshold (0–1) for showing suggestions. Must be <= confidence_threshold. Was called "Suggestion Floor" in earlier versions. |
| **assignment_mode** | Global setting controlling automation level: automatic, assisted, or manual. Was called "Authority" in earlier versions. |
| **Remove** | Unassign an item from a category (non-destructive; item remains in the database). |
| **Delete** | Permanently remove an item from the database (destructive; disappears from all views; logged to deletion log). |
| **Note** | Extended text body attached to an item or category for additional detail. Unlimited length. Searchable. |
| **Value** | A typed datum (text, numeric, or date) stored on an item for a column-category. |
| **Value Type** | The data type of a column-category: text, numeric, or date. Categories without a value type are boolean (membership-only). |
| **Computation** | An aggregate function (sum, average, max, min, count) displayed as a column footer in a view. |
| **Validation Condition** | A constraint evaluated before an assignment is accepted, enabling preconditions and cross-field validation. |
| **Reserved Category** | A built-in category (`When`, `Entry`, `Done`, `Entry When Done`) that cannot be deleted and has special system behavior. |
| **Provenance** | Metadata on an assignment recording how it was made (manual, auto_match, suggestion_accepted, action, subsumption), when, and with what confidence. |
| **Sticky Assignment** | An assignment that is never automatically removed by the rule engine, even if the text that triggered it is edited away. All accepted assignments are sticky. |
| **Deletion Log** | A permanent log of all deleted items with full content and assignments, browsable and restorable. |
| **RecurrenceRule** | A pattern (daily, weekly, monthly, yearly) defining when a recurring item repeats. Parsed from natural language. |
| **Virtual Category** | A computed bucket (like When:Today) evaluated at query time, not stored as a Category record. Represented in queries via `virtual_include`/`virtual_exclude` fields. |
| **WhenBucket** | Enum of virtual date buckets: overdue, today, tomorrow, this_week, next_week, this_month, future, no_date. Evaluated as date range tests. |

---

## 8. References

- Kapor, M., Belove, E., Kaplan, J. (1990). "AGENDA: A Personal Information Manager."
  *Communications of the ACM*, 33(7). — The foundational paper describing the data model.
- Fallows, J. (1992). "Agenda" (draft). *The Atlantic Monthly*. — The best plain-English
  description of what Agenda does and why it matters.
- Ormandy, T. (2020). "Lotus Agenda." — Modern hands-on exploration with working examples.
  https://lock.cmpxchg8b.com/lotusagenda.html
- Rosenberg, S. (2007). *Dreaming in Code*. — Documents Chandler's failure. Required
  reading for what NOT to do.
- System Stack (2025). "Hidden Agendas, Lost Cities." — Analysis of Agenda's philosophy
  vs. modern PKM/GTD tools.
- Kaner, C. (2003). "An Introduction to Scenario Testing." — The testing methodology
  underpinning this spec's scenario format.
- Bob Newell's Agenda Page. https://www.bobnewell.net/agenda.html — Archive of original
  documentation, FAQs, macros, and add-ons.

---

## Changelog

### v0.6 (2026-02-15)

**Consolidated v0.3, v0.4, v0.5 into a single authoritative spec.**

Major decisions and resolutions:

**§1.2 Section Model:**
- ✅ Confirmed **criteria-based section model** (v0.3/v0.5) over simplified heading model (v0.4)
- Rationale: Enables compound criteria sections ("Urgent AND Blocked") essential for real-world triage workflows
- Sections use full Query syntax with `on_insert_assign` and `on_remove_unassign` for explicit edit-through semantics

**§1.3 Unmatched Items Safety:**
- ✅ Restored **unmatched section** (v0.3/v0.5) as a generated section, not just rendering
- Renamed: `show_unsectioned_bucket` → **`show_unmatched`**
- Renamed: `unsectioned_bucket_label` → **`unmatched_label`**
- Clarified: Items appear in explicit sections OR unmatched section, never both
- Clarified: Unmatched criteria computed at query time (excludes items matching any explicit section)

**§1.4 Classification Settings (Modernized for 2026):**
- ✅ Kept three-tier threshold system with modernized naming for LLM era
- Renamed: `Initiative` → **`confidence_threshold`**
- Renamed: `Suggestion Floor` → **`review_threshold`**
- Renamed: `Authority` → **`assignment_mode`** with values {automatic, assisted, manual}
- Rationale: Modern ML terminology, designed for LLM-based classification

**Other resolutions:**
- ✅ Confirmed `entry_date` is **non-nullable** (v0.3/v0.5 correct, v0.4 error)
- ✅ Used **dual recurrence fields** (series_id + parent_item_id) from v0.3/v0.5
- ✅ Adopted **Assignment.origin** from v0.4 + **sticky flag** from v0.5
- ✅ Confirmed **"No Date" When bucket** (v0.4 omitted it)
- ✅ Kept **enable_implicit_string** and **condition_mode** (v0.4 dropped them)

Incorporated v0.4 enhancements:
- DeleteAction guardrails (allow_delete_action, deletion logging)
- Termination guarantees detail (max 10 passes, monotonic assignments)
- Performance constraints (§4.4): view switching <100ms, async retroactive assignment
- TUI enhancements (§4.3): search modes, inline category creation, inspect, undo
- Undo (§2.9): moved from Out of Scope to MVP
- Scenarios 21 (provenance), 22 (undo), 23 (unmatched section)

Clarifications added:
- Section model use case: compound criteria sections for triage workflows
- Unmatched section: behaves as generated section with edit-through semantics
- Classification modes: `assisted` auto-assigns high confidence, suggests medium, ignores low
- Crash recovery: SQLite WAL mode, JSON atomic rename (from v0.3)
- **Assignment.origin format**: Use `<namespace>:<name>` convention for human-debuggable provenance (e.g., "cat:Urgent#string", "nlp:date", "action:Done.remove"). No user PII.
- **Virtual categories**: When buckets (Today, Tomorrow, etc.) are NOT stored Category records. Represented in queries via `virtual_include`/`virtual_exclude` fields as WhenBucket enum values. Evaluated as date range tests, not category table lookups.
- **show_children behavior**: Activates only when section criteria is single category include (no excludes/text/date). Auto-generates subsections (one per direct child) with inherited criteria + child category. Items matching parent but no child go to unmatched section. One level depth only. Uses category's stored child order.
- **Column computations**: Moved to Phase 2 (sum, average, max, min, count aggregates deferred).
- **Undo stack**: In-memory only for MVP (resets on restart). Deletion log persists permanently.
- **Text fingerprint**: Implementation-defined threshold for re-evaluating rejected suggestions (e.g., >20% Levenshtein distance).
- **LLM integration**: Moved from "Out of Scope" to "Phase 2". MVP uses rule-based matching with confidence scores; data model designed for future LLM integration.

---

*This spec is version 0.6. It describes an MVP with Phase 2 design intent documented
inline. The right next step is to build, not to spec more.*
