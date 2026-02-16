# Agenda Reborn — NLSpec

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
have the system figure out how to organize it — not the other way around.** The user never
fills out forms or defines schemas upfront. Structure emerges from use.

The program had three core concepts:

- **Items** — atomic units of information (a phrase, a sentence, a task, a fact)
- **Categories** — hierarchical, multi-assignable labels that provide structure
- **Views** — dynamic, editable projections of items filtered and sectioned by categories

What made it extraordinary was the interplay between these three: automatic assignment via
NLP and rules, editing through views with reverse-inference of intent, and a category
hierarchy that functioned as a declarative program executed against every changed item.

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
  id:           UUID
  text:         String (max ~350 chars, following Agenda's original limit)
  note:         Optional<String> (unlimited length; for extended detail)
  created_at:   Timestamp
  modified_at:  Timestamp
  entry_date:   Optional<Date>        // when the item was entered (auto-set)
  when_date:    Optional<DateTime>     // parsed from text or manually assigned
  done_date:    Optional<DateTime>     // when marked complete
  is_done:      Boolean (default false)
  assignments:  Set<CategoryID>        // the categories this item belongs to
  values:       Map<CategoryID, Value> // typed values for column-categories
}

Value = TextValue(String) | NumericValue(Float) | DateValue(DateTime)
```

**Key behaviors:**

- An item's `text` is the primary content. It is never empty.
- An item can be assigned to zero or more categories (multifiling).
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

### 2.2 Category

A Category is a named concept used to organize items. Categories form a tree hierarchy.

```
Category {
  id:           UUID
  name:         String
  aliases:      List<String>           // alternative names for text matching
  parent:       Optional<CategoryID>   // null = root-level category
  children:     Ordered<List<CategoryID>>
  is_exclusive: Boolean (default false) // if true, an item can be in at most one child
  value_type:   Optional<enum { text, numeric, date }>  // if set, items carry typed values
  numeric_precision: Optional<Integer> // decimal places for numeric display (default 2)
  note:         Optional<String>       // documents the intent of the category
  conditions:   List<Condition>        // rules for automatic assignment
  actions:      List<Action>           // triggered when an item is assigned here
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
  category without the user doing anything.
- **Aliases**: A category "Fred Smith" might have aliases ["Fred", "Smith", "FS"].
  These broaden the text matching surface.

### 2.3 Category Hierarchy

The hierarchy is a forest (multiple root categories). Several special **reserved
categories** are built-in and always exist:

- `When` — a date-type category. Items with dates parsed from text are automatically
  assigned to date-range subcategories (Today, This Week, This Month, etc.)
- `Entry` — automatically tracks when items were created.
- `Done` — a reserved boolean category. When an item is assigned to `Done`:
  - The item's `is_done` flag is set to `true` and `done_date` is recorded.
  - The `Done` category functions as a standard category for filtering (e.g., views
    can exclude `Done` items).
  - Actions attached to `Done` fire normally (e.g., a `RemoveAction` to unassign
    from active project categories, making the item disappear from work views).
- `Entry When Done` — a trigger category. Items are automatically assigned to this
  category when they are marked `Done`. This enables completion-triggered workflows:
  attach actions to `Entry When Done` to automate post-completion behavior (e.g.,
  assign to an "Archive" category, log to an export file, or advance a recurring item).

Reserved categories cannot be deleted. They can have user-defined actions and conditions
attached to them like any other category.

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
}

Section {
  heading:      CategoryID             // items assigned to this category appear here
  show_children: Boolean (default true)
                // if true, subsections are generated for each child of the heading
                // category, allowing different classification hierarchies per section
}

# this may be deferred to later phase
Column {
  heading:      CategoryID             // shows assignments under this category subtree
  width:        Integer
  computation:  Optional<enum { sum, average, max, min, count }>
                // for numeric columns: displayed as a footer/summary row at column bottom
                // count is valid for any column type (counts non-empty assignments)
}

Query {
  include:      Set<CategoryID>        // items must be assigned to ALL of these
  exclude:      Set<CategoryID>        // items must NOT be assigned to ANY of these
  date_range:   Optional<DateRange>
  text_search:  Optional<String>       // full-text search across item text AND notes
}
```

**Key behaviors:**

- Views are *live*. When items or categories change, views update immediately.
- Views support **edit-through semantics**: inserting an item into a section automatically
  assigns it to that section's heading category. Deleting an item from a section
  unassigns it from that category (does not delete the item from the database).
- A separate `delete` command permanently removes an item from the database.
- Switching between views is a primary navigation action (not a secondary menu).
- Query synthesis: when the user selects categories for inclusion, the system auto-builds
  an appropriate boolean expression. Exclusive sibling categories are ORed (since ANDing
  them would always produce an empty set). Non-exclusive categories are ANDed.
- **Column computations**: When a column has a `computation` set and its heading category
  has `value_type: numeric`, the view displays a footer row with the computed aggregate
  (sum, average, max, min, or count). Computations update dynamically as items are added,
  edited, or removed. The `count` computation works on any column type and counts items
  with a non-empty assignment. Numeric precision in the footer follows the category's
  `numeric_precision` setting.  This feature may be deferred to a later implementation phase

### 2.5 Conditions and Actions

Conditions determine when an item should be automatically assigned to a category.
Actions fire when an assignment (automatic or manual) occurs.

```
Condition = StringCondition | ProfileCondition | DateCondition | ValidationCondition

StringCondition {
  // Implicit from category name + aliases.
  // Compares item text against category name after:
  //   - suffix stripping / stemming
  //   - proper name detection
  //   - date/time literal detection
  //   - limited syntactic analysis
  // Match strength is compared against the global Initiative threshold.
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
}
```

**Processing model:**

1. When an item is created or modified, it enters the processing queue.
2. The engine walks the category hierarchy depth-first.
3. For each category, it evaluates conditions. If conditions pass and the item is not
   already assigned, it assigns the item.
4. For each new assignment (including manual ones), it fires that category's actions.
5. If an action modifies the item (e.g., adds another assignment, changes a date), the
   item re-enters the queue for another pass.
6. Processing terminates when a pass produces no new changes (fixed-point).
7. A cycle detector prevents infinite loops.

### 2.6 Validation and Entry-Time Logic

Categories can enforce constraints on item entry and assignment, not just trigger
side-effects after the fact.

```
Condition = StringCondition | ProfileCondition | DateCondition | ValidationCondition

ValidationCondition {
  // Evaluated before an assignment is accepted. If it fails, the assignment
  // is rejected (or the user is warned, depending on Authority setting).
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
- **Rejection behavior**: When a validation fails under `Authority = auto`, the
  assignment is silently skipped. Under `suggest`, it appears as a warning. Under
  `ask`, the user is prompted with the rejection message and can override.

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

### 2.8 Initiative and Authority

Two global settings control the automatic assignment engine's behavior:

- **Initiative** (Float, 0.0–1.0): How confident a string match must be before the
  system acts on it. Low initiative = aggressive matching (more auto-assignments,
  more false positives). High initiative = conservative (fewer assignments, fewer errors).
  Default: 0.5.

- **Authority** (enum: `auto`, `suggest`, `ask`):
  - `auto` — matches above the initiative threshold are assigned immediately.
  - `suggest` — matches are queued as suggestions. A `?` indicator appears. The user
    can review and accept/reject.
  - `ask` — the system prompts for each potential assignment interactively.
  Default: `auto`.

---

## 3. NLP: Date and Entity Extraction

### 3.1 Date Parsing

The date parser extracts temporal expressions from item text and populates `when_date`.
This was one of Agenda's most polished features. The parser must handle:

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
perfect — the Initiative/Authority settings exist precisely to handle uncertainty.

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
│  (Items, Categories, Views) │   NLP pipeline, assignment processor.
├─────────────────────────────┤   Pure logic. No I/O.
│  Storage Adapter            │   Persistence. File-based for prototype.
└─────────────────────────────┘   SQLite or flat files.
```

The core engine MUST have zero UI dependencies. It should be testable as a pure library
that takes commands and emits events/state.

### 4.2 Storage (Prototype)

For the throwaway prototype, use the simplest possible persistence:

- A single JSON or SQLite file containing all items, categories, and views.
- File path configurable. Default: `~/.agenda/default.ag`
- No server. No network. No sync. Local-only.

### 4.3 TUI (Prototype Frontend)

The TUI should evoke the spirit of the original DOS interface while being usable in a
modern terminal:

- Full-screen application using a TUI framework (e.g., ratatui for Rust, bubbletea for
  Go, textual for Python, blessed/ink for Node).
- Primary interaction: a view fills the screen. Items are rows. Categories are columns
  or section headings.
- A persistent input bar at the bottom for typing new items (free-form).
- `F8` or equivalent to switch views (view browser).
- `Tab` / arrow keys to navigate items and columns.
- Direct editing of item text and category assignments inline.
- A note indicator (e.g., `♪` or `📎`) next to items that have attached notes.
- A status indicator (e.g., `?`) when pending assignment suggestions exist.

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
2. The assignment engine runs against all existing items.
3. Items containing "Sarah" (via string matching) are evaluated. Those exceeding the
   Initiative threshold are assigned to the "Sarah" category.
4. If Authority is `auto`, assignments happen silently. If `suggest`, a `?` indicator
   appears and the user can review. If `ask`, the user is prompted for each.
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
2. The string matcher detects "Sarah" → assigns to "Sarah" category.
3. The string matcher detects "urgent" → assigns to "Urgent" category.
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
exclude "Done". The view has sections headed by "Urgent" and "Normal", and a column
showing "When" dates.

**Step A — Viewing:**

1. The view displays all items assigned to "Sarah" that are not marked Done.
2. Items assigned to "Urgent" appear under the "Urgent" section.
3. Items not assigned to "Urgent" appear under the "Normal" section.
4. The "When" column shows parsed dates for each item.

**Step B — Inserting through the view:**

5. The user navigates to the "Urgent" section and types a new item:
   ```
   Discuss deployment timeline with Sarah
   ```
6. The new item is automatically assigned to BOTH "Sarah" (from view criteria) AND
   "Urgent" (from the section heading). The user did not manually assign either.

**Step C — Moving between sections:**

7. The user drags/moves an item from the "Urgent" section to the "Normal" section.
8. The item is unassigned from "Urgent". It remains assigned to "Sarah".
9. If "Urgent" and "Normal" are exclusive siblings under a "Priority" parent, mutual
   exclusion is enforced automatically.

**Step D — Removing from view vs. deleting:**

10. The user "removes" an item from the view. The item is unassigned from "Sarah"
    but still exists in the database and may appear in other views.
11. The user "deletes" a different item. It is permanently removed from the database
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
2. String matching assigns to "Urgent" and "Project Alpha".
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
"This Month", "Future".

**Expected behavior:**

1. An item with `when_date` of Feb 10 (yesterday) appears under "Overdue".
2. An item with `when_date` of Feb 11 appears under "Today".
3. An item with `when_date` of Feb 14 appears under "This Week".
4. When the clock rolls past midnight to Feb 12:
   - The Feb 11 item moves from "Today" to "Overdue" (or a "Past" category).
   - The Feb 12 item (if any) moves into "Today".
5. These transitions happen automatically. No user action required.

**Satisfaction criteria:** The `When` hierarchy is always current. Items naturally flow
through temporal categories as time passes. The user's "Today" view always reflects
actual reality.

---

### Scenario 10: Text Matching with Initiative Control

**Context:** The user has a category "Marketing" and types the item:

```
Buy March Madness tickets for the marketing team outing
```

**Expected behavior at Initiative = 0.3 (aggressive):**

1. "marketing" matches "Marketing" — assigned.
2. "March" might weakly match a "March" date category — possibly assigned (false positive).

**Expected behavior at Initiative = 0.7 (conservative):**

1. "marketing" matches "Marketing" — assigned (strong match, word is exact).
2. "March" is recognized as part of a compound noun phrase "March Madness" — not
   interpreted as a month. Not assigned to a date.

**Expected behavior at Initiative = 0.5 (default) with Authority = `suggest`:**

1. "marketing" → assigned automatically (high confidence).
2. "March" → queued as a suggestion with low confidence. The `?` indicator appears.
   The user can review and dismiss.

**Satisfaction criteria:** The Initiative/Authority controls give the user a tunable
tradeoff between automation and accuracy. The system is transparent about its confidence.

---

### Scenario 11: Recurring Items

**Context:** The user types:

```
Review infrastructure costs every month on the 15th
```

**Expected behavior:**

1. The item is created with a recurring `when_date`: monthly on the 15th.
2. The item appears in the current or next appropriate date section.
3. When the user marks the item "Done", a new instance is generated with the
   `when_date` advanced to the 15th of the following month.
4. The completed instance moves to the "Done" view/category. The new instance
   appears in the appropriate future date section.

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
3. The modification triggers a re-evaluation by the assignment engine. If the new text
   matches additional categories (e.g., "Testing"), new assignments may occur.
4. If the edit removes a matched word (e.g., the user deletes "Sarah" from the text),
   the string-match assignment to "Sarah" is NOT automatically removed — explicit
   assignments and previously-accepted auto-assignments are sticky. Only new text
   changes trigger new evaluations.

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
3. The string matcher runs against all existing categories.
4. Items are assigned to matching categories according to Initiative/Authority settings.
5. The user can review all imported items in the "All Items" view and refine assignments.

**Satisfaction criteria:** Bulk import of messy, unstructured text is a first-class
operation. The system handles the "I have a pile of notes and I want to make sense of
them" use case gracefully. This was Agenda's secret weapon for research and reference
material.

---

### Scenario 16: Suggested Assignments Review

**Context:** Authority is set to `suggest`. The user has been entering items. Several
string matches fell below the auto-assign threshold but above the suggestion threshold.
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
   for that item unless the item text changes.

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
2. The date parser and assignment engine run against the new item.
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
2. The assignment engine retroactively scans all 150+ items. Those mentioning work-
   related terms are assigned.
3. The user creates more categories: "Personal", "Health", "Project X".
4. Each new category triggers retroactive assignment. The pile of items self-organizes.
5. The user creates a view filtered by "Work" + "This Week". Suddenly, their week's
   work tasks are laid out in front of them — from two weeks of unstructured notes.
6. The user realizes: "I didn't have to organize this upfront. The structure emerged."

**Satisfaction criteria:** This is the "aha moment." The system proves its value by
making retroactive organization feel like magic. If this scenario doesn't produce a
moment of delight, the implementation has missed the point of the entire project.

---

## 6. Out of Scope (MVP)

These features are explicitly deferred. They are listed here as a guard against
Chandler-style scope creep.

- **Contacts management** — This is not an address book.
- **Email integration** — This is not an email client.
- **Calendar UI** — Items have dates. That's not the same as needing a calendar widget.
- **Collaboration / multi-user** — Local, single-user only.
- **Sync / cloud** — Local file only. No server.
- **Mobile app** — TUI works over SSH if needed.
- **Plugin system** — Premature. Build the core first.
- **LLM-powered categorization** — Tempting, but start with deterministic rules.
  LLM integration is a Phase 2 exploration once the rule engine proves out.
- **Printing** — Not in 2026.
- **Undo/redo** — Nice to have, not MVP.
- **Macro language / scripting** — Deferred to Phase 2. Design intent documented in §2.7.

---

## 7. Glossary

| Term | Definition |
|---|---|
| **Item** | An atomic unit of information: a sentence, phrase, task, fact, or thought. |
| **Category** | A named concept in a hierarchy used to organize items. Items are "assigned to" categories. |
| **Assignment** | The relationship between an item and a category. One item can have many assignments (multifiling). |
| **View** | A saved, dynamic, editable projection of items filtered by category criteria. |
| **Section** | A grouping within a view, headed by a category. Items assigned to that category appear in the section. |
| **Column** | An annotation in a view showing which subcategories an item is assigned to. |
| **Criteria / Query** | The selection filter for a view: which categories to include/exclude. |
| **Condition** | A rule attached to a category that determines when items should be auto-assigned. |
| **Action** | A side-effect triggered when an item is assigned to a category. |
| **Subsumption** | Child category assignments inherit upward: assigned to child → implicitly assigned to parent. |
| **Mutual Exclusion** | An exclusive category ensures an item can be assigned to at most one of its children. |
| **Initiative** | Global threshold (0–1) controlling how aggressive string matching is. |
| **Authority** | Global setting controlling whether auto-assignments happen silently, as suggestions, or interactively. |
| **Remove** | Unassign an item from a category (non-destructive; item remains in the database). |
| **Delete** | Permanently remove an item from the database (destructive; disappears from all views). |
| **Note** | Extended text body attached to an item or category for additional detail. Unlimited length. Searchable. |
| **Value** | A typed datum (text, numeric, or date) stored on an item for a column-category. |
| **Value Type** | The data type of a column-category: text, numeric, or date. Categories without a value type are boolean (membership-only). |
| **Computation** | An aggregate function (sum, average, max, min, count) displayed as a column footer in a view. |
| **Validation Condition** | A constraint evaluated before an assignment is accepted, enabling preconditions and cross-field validation. |
| **Reserved Category** | A built-in category (`When`, `Entry`, `Done`, `Entry When Done`) that cannot be deleted and has special system behavior. |

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

*This spec is version 0.2. It describes an MVP with Phase 2 design intent documented
inline. The right next step is to build, not to spec more.*
