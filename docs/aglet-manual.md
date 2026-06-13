# Aglet Help File

[» Index](#index)

Aglet is a free-form personal information manager modeled after Lotus
Agenda, re-imagined as a modern terminal application. This manual is written
in the spirit of the original Agenda help file: each topic is a short article
you can read on its own, with cross-references at the bottom.

## How to Use This Manual
    PURPOSE       This manual explains aglet's concepts, its terminal interface
                  (TUI), and its command-line interface (CLI).

    HOW TO READ   Topics are grouped from general to specific:

                  1. Overview .............. What aglet is and how to start.
                  2. Keys and commands ..... Quick reference charts.
                  3. Concepts .............. Items, categories, views, and more.
                  4. Tasks ................. Step-by-step procedures.
                  5. The CLI ............... Every command and flag.

    LINKS         Each article ends with a SEE ALSO list of related topics.
                  Click a link to jump there, or return to the Index at any time.

    CONVENTIONS   Keys are shown literally: n, Enter, Ctrl-S, Esc.
                  CLI commands are shown as you would type them in a shell:
                  aglet --db work.ag add "Buy groceries"

#### SEE ALSO

> [About aglet](#about-aglet),   [Quick start](#quick-start),   [» Index](#index)

## Index
    OVERVIEW
      About aglet
      Starter workflows
      Quick start

    KEYS AND COMMANDS
      TUI: Normal mode keys
      TUI: Category manager keys
      TUI: View editor keys
      TUI: Item editor keys
      TUI: Datebook keys
      CLI: Command chart

    CONCEPTS
      Items
      Categories
      Views
      Sections
      Columns
      Notes
      Dependencies
      Reserved categories

    CATEGORY TYPES
      Tag categories
      Numeric categories
      Exclusive categories
      The category hierarchy
      Subsumption

    VIEW TYPES
      Standard views
      Datebook views
      The All Items view

    WORKING WITH ITEMS
      Add an item
      Edit an item
      Add a note to an item
      Mark an item done
      Recurrence
      Delete an item
      Restore a deleted item
      Move an item between sections
      Search items
      Select multiple items

    WORKING WITH CATEGORIES
      Add a category
      Assign a category to an item
      Unassign a category
      Automatic assignment
      Profile conditions
      Date conditions
      Actions
      Review classification suggestions
      Set a numeric value
      Format a numeric column
      Organize the category hierarchy
      Discard a category

    WORKING WITH VIEWS
      Create a view
      View criteria
      Add a section to a view
      Add a column to a section
      Column summary functions
      Create a datebook view
      Browse a datebook view
      The view editor
      View aliases
      Clone a view
      Discard a view

    WORKING WITH DEPENDENCIES
      Create a dependency link
      Remove a link
      Filter blocked / not-blocked items
      The link wizard

    WORKFLOW
      Claim an item
      Release a claim
      The ready list

    SETTINGS AND FILES
      Global settings
      About .ag files
      The AGLET_DB variable

    THE CLI
      About the CLI
      CLI: Item commands
      CLI: Category commands
      CLI: View commands
      CLI: Link commands
      CLI: Filtering and sorting
      CLI: Import and export

    INDICATORS
      The status footer
      The assignment profile

------------------------------------------------------------------------

# Overview

## About Aglet
    PURPOSE       Aglet is a personal information manager that gives you control
                  over tasks, notes, facts, numbers, and dates. You capture
                  information as short items, then organize them with categories
                  and look at them through views.

    USES          People use aglet to:

                  · Keep a GTD-style to-do list   · Track budgets and bills
                  · Plan and track projects        · Collect research notes
                  · Build a personal knowledgebase  · Track maintenance logs

    HOW IT WORKS  You type information first and structure it afterward. Aglet
                  can also assign categories automatically when a category name
                  appears in an item's text or note. The same items can appear in
                  many views at once, each a different perspective on the same
                  database.

    BASIC STEPS   1. Enter items of information.
                  2. Organize them into categories (by hand or automatically).
                  3. Display views of those items and categories.

    INTERFACES    Aglet has two faces over one SQLite database:
                  · A TUI  - the interactive terminal interface (run "aglet").
                  · A CLI  - one-shot commands for scripts and agents.

#### SEE ALSO

> [Items](#items),   [Categories](#categories),   [Views](#views),   [Quick start](#quick-start),   [» Index](#index)

## Starter Workflows
    PURPOSE       Aglet has no fixed application templates. Instead, the same
                  items can serve many purposes by organizing them into views.
                  Here are common ways people set aglet up.

    TO-DO LIST    Capture tasks as items. Create a Priority category (exclusive,
                  with High/Normal/Low children) and a Status category. Make a
                  "Today" or "Next" view that includes the categories you want to
                  focus on and excludes Done.

    FINANCE       Create numeric categories such as Cost or Amount. Group bills
                  and budget lines into a view with sections per area and a
                  per-section column total (Sum).

    PROJECTS      Create a Project category with one child per project. View the
                  same items grouped by project, with columns for status,
                  priority, and effort, or as a Kanban board by status.

    KNOWLEDGEBASE Capture reference notes as items with longer notes attached.
                  Tag them by topic and group them in a research view alongside
                  follow-up tasks that share the same status model.

    SCHEDULING    Give items a When date. A datebook view buckets dated items
                  into calendar ranges so upcoming work and deadlines line up.

#### SEE ALSO

> [Views](#views),   [Numeric categories](#numeric-categories),   [Datebook views](#datebook-views),   [» Index](#index)

## Quick Start
    PURPOSE       Get a working database and add your first items.

    THE DATABASE  Aglet keeps everything in one SQLite file with a .ag extension.
                  Choose a path; it is created on first use.

                  You can name the database two ways:
                  · --db work.ag      on every command, or
                  · AGLET_DB=work.ag  exported once in your shell.

    OPEN THE TUI  aglet --db work.ag

                  Running aglet with no command opens the TUI. On a new database
                  aglet creates the reserved categories and the All Items view
                  for you. Press ? at any time for the in-app help panel.

    ADD AN ITEM   1. Press n to open the new-item editor.
                  2. Type an item, such as: Review Work budget Friday
                  3. Press Tab to move to the note field; add longer context.
                  4. Press Ctrl-S to save. (Enter also saves from the title
                     field; Esc cancels.)

    FROM THE CLI  aglet --db work.ag add "Buy groceries" --note "Milk, eggs"
                  aglet --db work.ag list

    NEXT          Add categories (c), then a view (v) to focus the list.

#### SEE ALSO

> [Add an item](#add-an-item),   [Add a category](#add-a-category),   [Create a view](#create-a-view),   [About .ag files](#about-ag-files),   [» Index](#index)

------------------------------------------------------------------------

# Keys and Commands

## TUI: Normal Mode Keys
    PURPOSE       Normal mode is the main board where items are displayed. These
                  keys work while the highlight is on an item or column.

    ITEMS         n .............. Add a new item to the focused section
                  e / Enter ...... Edit selected item (Enter adds when empty)
                  a .............. Assign categories to current item or selection
                  d / D .......... Toggle done on selected item(s)
                  r / x .......... Remove from view / delete selected item(s)
                  b / B .......... Open dependency link wizard (blocked-by/blocks)
                  = .............. Classify selected item(s) now
                  p / i / o ...... Toggle preview sidebar / cycle preview mode

    SELECTION     Space .......... Toggle selection on current item
                  a / d / x / = .. Batch assign, done, delete, or classify
                  b / B .......... Link selected items with a dependency
                  Esc ............ Clear selection

    NAVIGATION    Up/k Down/j .... Move between items; scroll preview when focused
                  Left/h Right/l . Move between sections or columns
                  Tab / S-Tab .... Next / previous section (J/K jump section)
                  [ / ] .......... Move item to previous / next section
                  (or S-Up/S-Down)
                  m / z .......... Cycle lane layout / card size

    SEARCH        / .............. Search the focused section
                  g/ ............. Search all sections
                  Esc ............ Clear the active section filter

    COLUMNS       Enter .......... Edit column value (on a column cell)
                  + / - .......... Add / remove a board column
                  H / L .......... Move board column left / right
                  f .............. Cycle numeric column format
                  F .............. Cycle column summary (Sum/Avg/Min/Max)
                  s / S or < / > . Sort section by column (ascending/descending)

    VIEWS         v / V / F8 ..... Open the view picker
                  , / . .......... Previous / next view
                  ga ............. Jump to the All Items view

    DATEBOOK      { / } .......... Step previous / next date bucket
                  ( / ) .......... Step the browse window
                  0 .............. Jump to today

    GLOBAL        C .............. Review pending classification suggestions
                  g s / F10 ...... Open Global Settings
                  c / F9 ......... Open the category manager
                  u .............. Toggle the hide-dependent-items filter
                  Ctrl-L ......... Reload data from disk
                  Ctrl-Z ......... Undo
                  Ctrl-Shift-Z ... Redo
                  ? .............. Toggle the help panel
                  q .............. Quit

#### SEE ALSO

> [Category manager keys](#tui-category-manager-keys),   [View editor keys](#tui-view-editor-keys),   [Select multiple items](#select-multiple-items),   [» Index](#index)

## TUI: Category Manager Keys
    PURPOSE       The category manager is a full-screen mode for working with the
                  category hierarchy. Open it with c or F9; press Esc to return.

    NAVIGATION    Up/k Down/j .... Move through the category tree
                  Tab ............ Move focus between the tree and the details pane

    EDIT          n .............. Create a sibling at the selected level
                  N .............. Create a child of the selected category
                  e / F2 ......... Edit the selected category name
                  S / Ctrl-S ..... Save category edits
                  Esc ............ Return to the main view

    REORDER       H / J / K / L .. Reorder the selected category
                  << / >> ........ Change the category's depth (promote / demote)

    DETAILS       The details pane shows flags (Exclusive, Auto-match,
                  Actionable), the value type (Tag or Numeric), conditions,
                  actions, and a free-form note. Workflow roles (such as the
                  claim category) are set in Global Settings.

#### SEE ALSO

> [Categories](#categories),   [Organize the category hierarchy](#organize-the-category-hierarchy),   [Actions](#actions),   [» Index](#index)

## TUI: View Editor Keys
    PURPOSE       The view editor configures a view's filter criteria, sections,
                  columns, layout, and aliases.

    NAVIGATION    Tab ............ Move between the editor regions (such as
                                   SECTIONS and DETAILS)
                  Up/k Down/j .... Move within a region

    EDIT          Edit criteria (include / exclude / OR-include), date ranges,
                  display mode (single-line or multi-line), section flow
                  (vertical stacked or horizontal lanes), unmatched-item
                  visibility, and category aliases.

    SAVE          S / Ctrl-S ..... Save the view
                  Esc ............ Cancel without saving

    NOTE          Section and per-section filters are reset when you switch views
                  or save the editor.

#### SEE ALSO

> [Views](#views),   [Create a view](#create-a-view),   [View criteria](#view-criteria),   [View aliases](#view-aliases),   [» Index](#index)

## TUI: Item Editor Keys
    PURPOSE       The item editor (opened with n to add or e to edit) is a panel
                  with a title field, a note field, and an inline category list.

    FIELDS        Tab / S-Tab .... Cycle focus: Title -> Note -> Categories ->
                                   Save -> Cancel
                  Ctrl-S ......... Save from any field
                  Enter .......... Save from the title field
                  Esc ............ Cancel

    CATEGORIES    Within the inline category list:
                  j / k .......... Move through categories
                  Space .......... Toggle a tag assignment
                  (a numeric category shows an inline editable value field)
                  / .............. Filter the category list
                  n .............. Create a new category inline

    NOTES         Ctrl-G ......... Open $EDITOR to edit the title or note in your
                                   external editor

#### SEE ALSO

> [Add an item](#add-an-item),   [Edit an item](#edit-an-item),   [Add a note](#add-a-note-to-an-item),   [Assign a category](#assign-a-category-to-an-item),   [» Index](#index)

## TUI: Datebook Keys
    PURPOSE       A datebook view buckets dated items into calendar ranges. These
                  keys move the visible date window.

    KEYS          { .............. Step to the previous date bucket
                  } .............. Step to the next date bucket
                  ( .............. Step the browse window backward
                  ) .............. Step the browse window forward
                  0 .............. Jump to today

    NOTE          A datebook view groups items by a date source (When, Entry,
                  Done, or a date category) over a period such as day, week, or
                  month.

#### SEE ALSO

> [Datebook views](#datebook-views),   [Create a datebook view](#create-a-datebook-view),   [Browse a datebook view](#browse-a-datebook-view),   [» Index](#index)

## CLI: Command Chart
    COMMAND       PURPOSE
    ------------  -----------------------------------------------------------
    add           Add a new item
    edit          Edit an item's text, note, and/or done state
    show          Show a single item with its assignments
    list          List items (optionally filtered)
    search        Search item text and note
    export        Export items as Markdown
    delete        Delete an item (writes a deletion log entry)
    deleted       List deletion log entries
    restore       Restore an item from the deletion log
    claim         Atomically claim an eligible item for active work
    ready         List items eligible to be claimed
    release       Remove the active claim category (alias: unclaim)
    tui           Launch the interactive TUI
    category      Category commands (see CLI: Category commands)
    view          View commands (see CLI: View commands)
    link          Item-to-item link commands
    unlink        Remove item-to-item links
    import        Structured import commands (CSV)
    item          Item commands in alternative noun-verb syntax

    OPTIONS       --db     SQLite database path (or set AGLET_DB)
                  -h, --help     Print help for any command or subcommand

    NOTE          Run "aglet" with no command to launch the TUI. Every command
                  accepts --help for its own options.

#### SEE ALSO

> [About the CLI](#about-the-cli),   [Item commands](#cli-item-commands),   [Category commands](#cli-category-commands),   [View commands](#cli-view-commands),   [» Index](#index)

------------------------------------------------------------------------

# Concepts

## Items
    PURPOSE       An item is a line or two of information that you want to keep
                  track of in aglet. You can assign each item to any number of
                  categories.

    EXAMPLES      · Call newspapers today to respond to stories about costs.
                  · Fix database connection pooling.
                  · Meeting with whole staff every Thursday at 11:00.

    PARTS         · Text   - the short title (up to about 350 characters).
                  · Note   - optional longer text attached to the item.
                  · Dates  - Entry (created), When (to happen), Done.
                  · Status - open or done.
                  · Links  - dependencies and relations to other items.

    NOTE          To add more text than the title holds, attach a note. Aglet
                  parses dates from item text when it can, and matching category
                  names in the text or note can be assigned automatically.

#### SEE ALSO

> [Add an item](#add-an-item),   [Notes](#notes),   [Categories](#categories),   [Mark an item done](#mark-an-item-done),   [Dependencies](#dependencies),   [» Index](#index)

## Categories
    PURPOSE       Categories are names you use to group related items. An item can
                  be assigned to many categories at once (multifiling).

    HOW THEY      Categories are hierarchical: each category can have a parent and
    WORK          children. You display and change the hierarchy in the category
                  manager.

                      Priority
                        High
                        Normal
                        Low

    TYPES         Aglet has two value kinds of category:

                  KIND     SYMBOL  CONTENTS
                  -------  ------  ----------------------------------------------
                  Tag              Boolean membership: the item is in it or not.
                  Numeric  N       Carries a decimal value per item (Cost, Qty).

                  In addition, categories can be marked Exclusive (only one child
                  assignable per item) and can use automatic assignment (implicit
                  string matching), conditions, and actions.

    RESERVED      Every database has the reserved categories Done, When, and
                  Entry. They cannot be modified, deleted, or used as child
                  category names.

#### SEE ALSO

> [Tag categories](#tag-categories),   [Numeric categories](#numeric-categories),   [Exclusive categories](#exclusive-categories),   [The hierarchy](#the-category-hierarchy),   [Automatic assignment](#automatic-assignment-implicit-string-matching),   [Reserved categories](#reserved-categories),   [» Index](#index)

## Views
    PURPOSE       A view is a saved perspective over the same items. Each view can
                  filter to certain categories, hide completed items, group items
                  into sections, and show custom columns. A database can hold many
                  views.

    EXAMPLES      · A "Work Queue" view shows open work tasks by priority.
                  · A "Finance" view shows bills grouped by area with totals.
                  · A "Projects" view groups the same items by project.
                  · A "Scheduling" datebook view shows dated items by week.

    HOW THEY      A view does not own items; it selects them. Add an item and it
    WORK          appears in every view whose criteria it meets. Views are saved
                  lenses, not separate lists.

#### SEE ALSO

> [Standard views](#standard-views),   [Datebook views](#datebook-views),   [The All Items view](#the-all-items-view),   [Create a view](#create-a-view),   [Sections](#sections),   [» Index](#index)

## Sections
    PURPOSE       A section is a group within a view that collects items matching
                  its own criteria, under a heading. Sections let one view show
                  several lanes of related items.

    EXAMPLE       A "Calls" section and a "Letters" section in the same view:

                  Calls
                    · Call John re: DW deal
                    · Call Karla for quotes

                  Letters
                    · Send Wendy an offer
                    · Answer client request

    HOW THEY      Each section has its own include / exclude / OR criteria, and
    WORK          can optionally show its children as sub-groupings. Sections can
                  be laid out stacked vertically or as horizontal lanes (a Kanban
                  board). Each section can carry a column summary such as a total.

    FILTERS       In Normal mode, / scopes a search to the focused section only;
                  Esc clears that section's filter.

#### SEE ALSO

> [Views](#views),   [Add a section](#add-a-section-to-a-view),   [Columns](#columns),   [Column summaries](#column-summary-functions),   [» Index](#index)

## Columns
    PURPOSE       A column shows a piece of data next to each item in a section,
                  such as a numeric category value or a date.

    TYPES         · Numeric column - shows a numeric category's value per item,
                    and can carry a per-section summary (Sum, Avg, Min, Max).
                  · Date column    - shows a date such as When or Entry.
                  · Category value - shows whether/which category applies.

    IN THE TUI    + adds a column, - removes one, H / L move it left or right.
                  With the highlight on a column cell, Enter edits that value.
                  f cycles a numeric column's display format; F cycles its
                  summary function. s / S (or < / >) sort the section by the
                  column.

    EXAMPLE       A finance section with a Cost column and a Sum footer:

                  Renewals                        Cost
                    · Domain renewal                12.00
                    · Insurance                    480.00
                                              Sum  492.00

#### SEE ALSO

> [Add a column](#add-a-column-to-a-section),   [Column summaries](#column-summary-functions),   [Numeric categories](#numeric-categories),   [Format a numeric column](#format-a-numeric-column),   [» Index](#index)

## Notes
    PURPOSE       A note lets you add longer information to an item. The title
                  stays short; the note holds the detail.

    EXAMPLES      · A meeting agenda attached to a reminder item.
                  · The full description of a bug attached to a task.
                  · Reference text for a knowledgebase entry.

    HOW TO ADD    In the item editor, press Tab to reach the note field and type.
                  To edit a long note in your external editor, press Ctrl-G to
                  open $EDITOR. From the CLI, use --note, --append-note, or
                  --note-stdin on add and edit.

    NOTE          Automatic assignment scans note text as well as item text. A
                  category name appearing only in the note can still trigger an
                  automatic assignment.

#### SEE ALSO

> [Add a note to an item](#add-a-note-to-an-item),   [Automatic assignment](#automatic-assignment-implicit-string-matching),   [Edit an item](#edit-an-item),   [» Index](#index)

## Dependencies
    PURPOSE       Dependencies are typed links between items. They let aglet track
                  which items are waiting on others.

    TYPES         LINK         MEANING
                  -----------  ---------------------------------------------------
                  depends-on   This item needs the other item done first.
                  blocks       This item is the prerequisite of the other (the
                               inverse of depends-on).
                  related      A non-blocking, bidirectional association.

    BLOCKED       An item is "blocked" when it has at least one prerequisite
                  (depends-on) that is not yet done. When the prerequisite is
                  marked done, the dependent item is no longer blocked. Blocked
                  state is computed at query time from the link graph.

    HOW TO ADD    In the TUI, press b or B to open the link wizard. From the CLI,
                  use "aglet link depends-on", "link blocks", or "link related".

#### SEE ALSO

> [Create a dependency](#create-a-dependency-link),   [Filter blocked items](#filter-blocked--not-blocked-items),   [The link wizard](#the-link-wizard),   [Mark an item done](#mark-an-item-done),   [» Index](#index)

## Reserved Categories
    PURPOSE       Every aglet database contains a few built-in categories with
                  special meaning. They are created automatically and cannot be
                  modified, deleted, or reused as child category names.

    THE RESERVED  CATEGORY  MEANING
    CATEGORIES    --------  -----------------------------------------------------
                  Entry     The date/time the item was created.
                  When      The date/time the item is to happen.
                  Done      The date/time the item was marked done.

    NOTE          These categories do not use implicit string matching and are
                  non-actionable. To create a workflow category that means
                  "finished" under an exclusive Status parent, use a name such as
                  Complete or Completed - not Done, which is reserved.

#### SEE ALSO

> [Categories](#categories),   [Mark an item done](#mark-an-item-done),   [Claim an item](#claim-an-item),   [» Index](#index)

------------------------------------------------------------------------

# Category Types

## Tag Categories
    PURPOSE       A tag category records boolean membership: an item either has it
                  or does not. This is the default kind of category.

    EXAMPLES      Work, Personal, Urgent, Bug, Frontend, Backend.

    HOW TO MAKE   aglet category create "Urgent"

                  In the category manager, press n (sibling) or N (child) and
                  type the name.

    NOTE          A tag category can be a parent of other categories, can be
                  exclusive, and can use automatic assignment, conditions, and
                  actions.

#### SEE ALSO

> [Numeric categories](#numeric-categories),   [Add a category](#add-a-category),   [Assign a category](#assign-a-category-to-an-item),   [» Index](#index)

## Numeric Categories
    PURPOSE       A numeric category carries a decimal value per item, instead of
                  plain membership. The name is up to you: Cost, Miles, Qty,
                  Effort, Amount.

    HOW TO MAKE   aglet category create "Cost" --type numeric

                  The value lives on the assignment between an item and the
                  category, so each item can have its own Cost.

    SET A VALUE   aglet category set-value  Cost 450.00

                  In the TUI, put the highlight on the numeric column cell and
                  press Enter, or edit it inline in the item editor's category
                  list. Setting a value for the first time is the usual way to
                  assign a numeric category to an item.

    DISPLAY       Numeric columns can show a per-section summary (Sum, Avg, Min,
                  Max) and can be formatted with decimals, a currency symbol, and
                  thousands separators.

#### SEE ALSO

> [Set a numeric value](#set-a-numeric-value),   [Format a numeric column](#format-a-numeric-column),   [Columns](#columns),   [Column summaries](#column-summary-functions),   [» Index](#index)

## Exclusive Categories
    PURPOSE       An exclusive category allows an item to be assigned to only one
                  of its children at a time. Assigning a second child replaces the
                  first.

    EXAMPLE       Priority (exclusive) with children High, Normal, Low. An item
                  can be High or Normal or Low, but never two at once. Assigning
                  Low to an item already marked High silently switches it to Low.

    HOW TO MAKE   aglet category create "Priority" --exclusive

                  In the category manager, the details pane shows an Exclusive
                  flag you can toggle.

    USES          Priority, Status, Stage - anything where exactly one value
                  should apply.

#### SEE ALSO

> [Categories](#categories),   [The hierarchy](#the-category-hierarchy),   [Assign a category](#assign-a-category-to-an-item),   [» Index](#index)

## The Category Hierarchy
    PURPOSE       Categories form a tree. A category can have a parent and any
                  number of children, and the database can have several root
                  categories. The hierarchy groups related categories so you can
                  organize and filter at different levels.

    EXAMPLE       - Area
                      - Backend
                      - Frontend
                  - Priority [exclusive]
                      - High
                      - Normal
                      - Low

    WHERE         You see and change the hierarchy in the category manager (c or
                  F9). H/J/K/L reorder a category; << and >> change its depth.

    NOTE          A category cannot be deleted while it still has children;
                  reparent or delete the children first. Reparenting to the root
                  level is done with --root on the CLI.

#### SEE ALSO

> [Organize the hierarchy](#organize-the-category-hierarchy),   [Subsumption](#subsumption),   [Discard a category](#discard-a-category),   [» Index](#index)

## Subsumption
    PURPOSE       Subsumption is the rule that assigning a child category also
                  implies its parent. If an item is in Backend, it is also treated
                  as being in Backend's parent, Area.

    EXAMPLE       Assigning "Backend" to an item makes it match a view that
                  filters on "Area", because Backend is a child of Area. The
                  parent assignment is shown with reason "Subsumption".

    WHY           Subsumption lets you filter broadly (everything in Area) or
                  narrowly (just Backend) from the same assignments, without
                  tagging each item twice.

#### SEE ALSO

> [The hierarchy](#the-category-hierarchy),   [The assignment profile](#the-item-assignment-profile),   [View criteria](#view-criteria),   [» Index](#index)

------------------------------------------------------------------------

# View Types

## Standard Views
    PURPOSE       A standard view displays any items and any categories, filtered
                  by criteria and optionally grouped into sections. It is the
                  general-purpose view type.

    CRITERIA      · Include  - item must have ALL of these categories (AND).
                  · OR-include - item must have AT LEAST ONE of these.
                  · Exclude  - item must have NONE of these.

    LAYOUT        Items can be shown one line each or multi-line, and sections can
                  be stacked vertically or arranged as horizontal lanes for a
                  Kanban board (toggle with m).

#### SEE ALSO

> [Views](#views),   [View criteria](#view-criteria),   [Datebook views](#datebook-views),   [Create a view](#create-a-view),   [» Index](#index)

## Datebook Views
    PURPOSE       A datebook view buckets dated items into calendar ranges, so
                  upcoming work, appointments, renewals, and deadlines line up by
                  date.

    CONFIGURE     A datebook view has:
                  · A date source - When, Entry, Done, or a date category.
                  · A period      - day, week, month, and so on.
                  · An interval   - how many periods per bucket.
                  · An anchor     - where the buckets start.

    BROWSE        In the TUI, { and } step buckets, ( and ) step the window, and
                  0 jumps to today. From the CLI, "aglet view datebook-browse"
                  shifts the window with --offset and --step.

    NOTE          You create a datebook view with "aglet view create-datebook";
                  a standard view and a datebook view are different types and one
                  cannot be converted to the other.

#### SEE ALSO

> [Create a datebook view](#create-a-datebook-view),   [Browse a datebook view](#browse-a-datebook-view),   [Datebook keys](#tui-datebook-keys),   [» Index](#index)

## The All Items View
    PURPOSE       All Items is the built-in view that shows every item in the
                  database with no filtering. It is created automatically and is a
                  system view.

    ACCESS        In the TUI, press ga to jump to it. From the CLI:
                  aglet view show "All Items"

    NOTE          All Items is immutable - you cannot edit or delete it - but you
                  can clone it into a new, mutable view if you want a copy to
                  customize. "aglet list" without --view uses All Items when it is
                  present, then falls back to the first stored view.

#### SEE ALSO

> [Views](#views),   [Clone a view](#clone-a-view),   [Create a view](#create-a-view),   [» Index](#index)

## Add an Item
    PURPOSE       Add a new item to the database. An item is a single line of
                  free-form text, optionally with a longer note.

    TUI STEPS     1. Press n to open the new-item editor (the item is added to the
                     focused section).
                  2. Type the item text, such as "Review Work budget Friday".
                  3. Press Tab to move to the Note field and the inline category
                     checklist if you want to add detail or assignments.
                  4. Press Ctrl-S to save from any field. Enter also saves from the
                     title field; Esc cancels.

    CLI STEPS     aglet add "Review Work budget Friday"
                  aglet add "Pay insurance" --note "Renews annually in March"

    HOW IT WORKS  When an item is saved, aglet scans its text and note against
                  category names. Categories whose names appear are assigned
                  automatically (see Automatic assignment), and recognizable dates
                  are parsed into the When date. The CLI prints the new item id and
                  a new_assignments count.

    NOTE          Short item-id prefixes returned by "aglet add" can be used
                  anywhere an item id is accepted. Parse the "created " line for the
                  id; it is not always the last line of output.

#### SEE ALSO

> [Items](#items),   [Edit an item](#edit-an-item),   [Automatic assignment](#automatic-assignment-implicit-string-matching),   [Add a note](#add-a-note-to-an-item),   [Item editor keys](#tui-item-editor-keys),   [» Index](#index)

## Edit an Item
    PURPOSE       Change an item's text, note, or done state.

    TUI STEPS     1. Select the item.
                  2. Press e (or Enter) to open the editor.
                  3. Edit the text or Tab to the note. Toggle inline category
                     assignments if desired.
                  4. Press Ctrl-S to save, Esc to cancel.

    CLI STEPS     aglet edit  --text "New title"
                  aglet edit  --note "Updated note text"
                  aglet edit  --done            # mark done
                  aglet edit  --not-done        # reopen

    HOW IT WORKS  Editing text re-runs automatic assignment: implicit-string
                  matches may be added or evicted, but manual and accepted-
                  suggestion assignments stay sticky. Press Ctrl-G in the TUI text
                  or note field to open the item in $EDITOR.

    NOTE          On an empty section, pressing Enter starts a new item rather than
                  editing.

#### SEE ALSO

> [Add an item](#add-an-item),   [Add a note](#add-a-note-to-an-item),   [Mark an item done](#mark-an-item-done),   [Automatic assignment](#automatic-assignment-implicit-string-matching),   [» Index](#index)

## Add a Note to an Item
    PURPOSE       Attach a longer body of text to an item. Notes hold detail that
                  does not belong in the one-line title.

    TUI STEPS     1. Open the item with e (or n for a new item).
                  2. Press Tab to move from the title to the Note field.
                  3. Type freely; the note is multi-line.
                  4. Press Ctrl-G to edit the note in $EDITOR for longer text.
                  5. Press Ctrl-S to save.

    CLI STEPS     aglet add "Plan offsite" --note "Book venue, send agenda"
                  aglet edit  --note "Revised plan"

    HOW IT WORKS  Note text participates in automatic assignment: a category name
                  appearing only in the note still triggers an implicit-string
                  match. Inspect "aglet show" provenance before assuming a visible
                  category was assigned manually.

    NOTE          In compact list output an item with a note shows a note marker;
                  use --verbose or "aglet show" to read the full note.

#### SEE ALSO

> [Notes](#notes),   [Edit an item](#edit-an-item),   [Automatic assignment](#automatic-assignment-implicit-string-matching),   [» Index](#index)

## Mark an Item Done
    PURPOSE       Record that an item is complete. Done is a reserved category that
                  also drives recurrence and dependency resolution.

    TUI STEPS     Select an item (or several with Space) and press d to toggle done.
                  D toggles done on the whole current selection.

    CLI STEPS     aglet edit  --done
                  aglet edit  --not-done

    HOW IT WORKS  Completing an item assigns the reserved Done category. A done
                  prerequisite no longer blocks items that depend on it. If the
                  item carries a recurrence rule, completing it schedules the next
                  occurrence (see Recurrence).

    NOTE          Done items are hidden from most views that exclude Done and from
                  "aglet list" unless you pass --include-done.

#### SEE ALSO

> [Reserved categories](#reserved-categories),   [Recurrence](#recurrence),   [Dependencies](#dependencies),   [» Index](#index)

## Recurrence
    PURPOSE       Make an item repeat on a schedule. When a recurring item is
                  completed, aglet generates the next occurrence automatically.

    HOW IT WORKS  A recurrence rule is attached to a dated item. Completing the
                  current occurrence (marking it Done) advances the When date to
                  the next scheduled date and reopens the item, so recurring chores,
                  bills, and maintenance reappear without re-entry.

    EXAMPLES      A monthly "Pay insurance" item set to recur reappears with next
                  month's date each time you mark it done.

    NOTE          Recurrence is an aglet feature with no Lotus Agenda analog. It
                  works together with the reserved When and Done categories.

#### SEE ALSO

> [Mark an item done](#mark-an-item-done),   [Reserved categories](#reserved-categories),   [Datebook views](#datebook-views),   [» Index](#index)

## Delete an Item
    PURPOSE       Remove an item from the database. Deletion is logged so the item
                  can be restored.

    TUI STEPS     Select the item(s) and press x to delete. (Press r to remove an
                  item from the current view without deleting it from the database.)

    CLI STEPS     aglet delete 
                  aglet deleted              # list deletion-log entries
                  aglet restore     # restore by log entry id

    HOW IT WORKS  Deleting writes a deletion-log entry rather than erasing the item
                  immediately. "aglet deleted" lists log entries with their ids;
                  "aglet restore" brings an item back.

    NOTE          r (remove from view) and x (delete) are different. Remove only
                  changes view membership; delete affects the whole database.

#### SEE ALSO

> [Restore a deleted item](#restore-a-deleted-item),   [Items](#items),   [CLI item commands](#cli-item-commands),   [» Index](#index)

## Restore a Deleted Item
    PURPOSE       Bring back an item that was deleted, using the deletion log.

    CLI STEPS     1. aglet deleted               # find the log entry id
                  2. aglet restore       # restore that entry

    HOW IT WORKS  Every delete appends an entry to the deletion log. Restoring
                  recreates the item with its text, note, and recorded state.

    NOTE          Restore is an aglet feature with no Lotus Agenda analog; Agenda's
                  discard was not reversible in the same way.

#### SEE ALSO

> [Delete an item](#delete-an-item),   [CLI item commands](#cli-item-commands),   [» Index](#index)

## Move an Item Between Sections
    PURPOSE       Reposition an item into a different section of the current view.

    TUI STEPS     Select the item and press [ to move it to the previous section or
                  ] to move it to the next section. Shift-Up and Shift-Down do the
                  same. Use h/l (or Left/Right) to move between sections or columns,
                  and Tab / Shift-Tab to move focus between sections.

    HOW IT WORKS  Sections in a standard view are defined by category criteria.
                  Moving an item between sections adjusts its category assignments
                  so it matches the destination section's criteria.

    NOTE          In horizontal (kanban) lane layouts, moving an item between lanes
                  is the same operation; aglet remembers the per-lane selection.

#### SEE ALSO

> [Sections](#sections),   [Add a section](#add-a-section-to-a-view),   [Normal mode keys](#tui-normal-mode-keys),   [» Index](#index)

## Search Items
    PURPOSE       Find items by text in their title or note.

    TUI STEPS     Press / to search within the focused section, or g/ to search
                  across all sections. Type the query; press Esc to clear the
                  active section filter.

    CLI STEPS     aglet search "budget"
                  aglet search "budget" --not-blocked

    HOW IT WORKS  CLI and TUI search both route through the same matcher over item
                  title and note text. Per-section filters in the TUI are scoped to
                  the focused section and reset on view switch.

    NOTE          Search matches note text, so a query can match items whose title
                  does not contain the term.

#### SEE ALSO

> [Items](#items),   [Select multiple items](#select-multiple-items),   [CLI filtering](#cli-filtering-and-sorting),   [» Index](#index)

## Select Multiple Items
    PURPOSE       Operate on several items at once - assign, complete, delete, or
                  classify them together.

    TUI STEPS     1. Press Space to toggle selection on the current item; repeat to
                     build a selection.
                  2. Apply a batch operation: a (assign categories), d (done),
                     x (delete), = (classify now), or b / B (link with a
                     dependency).
                  3. Press Esc to clear the selection.

    HOW IT WORKS  Selection is the aglet analog of Agenda's marked items. Batch
                  operations act on every selected item.

    NOTE          With no explicit selection, the same keys act on the single
                  item under the cursor.

#### SEE ALSO

> [Assign a category](#assign-a-category-to-an-item),   [Mark an item done](#mark-an-item-done),   [Create a dependency](#create-a-dependency-link),   [Review suggestions](#review-classification-suggestions),   [» Index](#index)

## Add a Category
    PURPOSE       Create a new category - the basic filing unit. Categories can be
                  top-level or nested under a parent.

    TUI STEPS     1. Press c or F9 to open the category manager.
                  2. Press n to create a category at the selected level, or N to
                     create a child of the selected category.
                  3. Type the name (Work, Personal, Urgent, ...).
                  4. Adjust flags such as exclusive, implicit matching, and notes in
                     the details pane.
                  5. Press Ctrl-S to save, Esc to return.

    CLI STEPS     aglet category create "Work"
                  aglet category create "High" --parent Priority
                  aglet category create "Priority" --exclusive
                  aglet category create "Cost" --type numeric

    NOTE          The reserved names Done, When, and Entry cannot be used. For a
                  workflow child under an exclusive Status parent, use names such as
                  Complete or Completed, not Done.

#### SEE ALSO

> [Categories](#categories),   [Tag categories](#tag-categories),   [Numeric categories](#numeric-categories),   [Exclusive categories](#exclusive-categories),   [Organize the hierarchy](#organize-the-category-hierarchy),   [» Index](#index)

## Assign a Category to an Item
    PURPOSE       Manually file an item under a category.

    TUI STEPS     1. Select the item (or several with Space).
                  2. Press a to open the inline category picker.
                  3. Press Space to toggle a category's assignment without closing,
                     or Enter to apply the current result and close.
                  4. Press / to filter; from the filter box Tab, Shift-Tab, Up, or
                     Down returns focus to the narrowed list.

    CLI STEPS     aglet category assign  Work
                  aglet category assign  High

    HOW IT WORKS  Manual assignments are durable user choices: they stay sticky even
                  when text changes. Assigning a child also displays its parent
                  (assigning High also shows Priority). For exclusive parents, only
                  one child can be assigned.

    NOTE          Use the inline checklist in the item editor to assign categories
                  while adding or editing an item.

#### SEE ALSO

> [Unassign a category](#unassign-a-category),   [Automatic assignment](#automatic-assignment-implicit-string-matching),   [Exclusive categories](#exclusive-categories),   [Set a numeric value](#set-a-numeric-value),   [» Index](#index)

## Unassign a Category
    PURPOSE       Remove a category from an item.

    TUI STEPS     Press a on the item, then Space on the assigned category to toggle
                  it off; Enter applies and closes.

    CLI STEPS     aglet category unassign  Work

    HOW IT WORKS  Unassigning removes the explicit assignment row and reprocesses
                  the item. If the category still matches by implicit string or
                  profile condition, a live (non-sticky) assignment may reappear;
                  sticky manual/action/accepted-suggestion assignments do not.

    NOTE          To stop a category from auto-matching entirely, turn off its
                  implicit-string matching rather than repeatedly unassigning.

#### SEE ALSO

> [Assign a category](#assign-a-category-to-an-item),   [Automatic assignment](#automatic-assignment-implicit-string-matching),   [Discard a category](#discard-a-category),   [» Index](#index)

## Automatic Assignment (Implicit String Matching)
    PURPOSE       Let aglet file items for you when a category name appears in an
                  item's text or note.

    HOW IT WORKS  When implicit string matching is enabled for a category, aglet
                  checks both the item title and the full note text. If the category
                  name is present, the category is assigned automatically. Assigning
                  a child also subsumes its parent. These live matches can break
                  automatically if the text changes; manual, action, and accepted-
                  suggestion assignments remain sticky.

    EXAMPLES      Adding "Refactor Backend auth module" auto-assigns Backend (and
                  its parent Area) because "Backend" appears in the title.

    NOTE          Command examples or acceptance criteria inside a note can
                  accidentally match categories such as Ready, CLI, or TUI. Inspect
                  "aglet show" provenance before assuming an assignment was manual.
                  Turning off enable_implicit_string evicts live matches but not
                  older sticky derived assignments.

#### SEE ALSO

> [Categories](#categories),   [Assign a category](#assign-a-category-to-an-item),   [Profile conditions](#profile-conditions),   [Subsumption](#subsumption),   [Review suggestions](#review-classification-suggestions),   [» Index](#index)

## Profile Conditions
    PURPOSE       Assign a category based on structured rules about an item, beyond
                  a plain name match.

    CLI STEPS     aglet category add-condition  ...
                  aglet category set-condition-mode  ...
                  aglet category remove-condition  

    HOW IT WORKS  A category can carry one or more explicit conditions. The
                  condition mode controls how multiple conditions combine. When an
                  item satisfies the conditions, the category is derived
                  automatically. Derived (non-sticky) assignments can break if the
                  item stops matching.

    NOTE          In the category manager the left pane shows a readable rule-count
                  badge such as [2 conditions]. Conditions are the aglet analog of
                  Agenda's assignment conditions.

#### SEE ALSO

> [Automatic assignment](#automatic-assignment-implicit-string-matching),   [Date conditions](#date-conditions),   [Actions](#actions),   [CLI category commands](#cli-category-commands),   [» Index](#index)

## Date Conditions
    PURPOSE       Assign a category based on an item's date - for example, to bucket
                  items by when they are due.

    CLI STEPS     aglet category add-date-condition  ...

    HOW IT WORKS  A date condition tests an item's When date against a range or
                  relative window. When it matches, the category is derived. Date
                  conditions power date-range categories used to build datebook
                  views and date-grouped sections.

    NOTE          Direct SQLite writes do not sync the reserved When assignment; use
                  aglet/CLI logic so date conditions see the date.

#### SEE ALSO

> [Profile conditions](#profile-conditions),   [Datebook views](#datebook-views),   [Reserved categories](#reserved-categories),   [» Index](#index)

## Actions
    PURPOSE       Make assigning one category automatically assign or remove another.

    CLI STEPS     aglet category add-action  ...
                  aglet category remove-action  

    HOW IT WORKS  An action fires when its owning category is assigned to an item.
                  An Assign action adds another category; a Remove action removes
                  one. Actions are event-driven: adding or editing an action does
                  not retroactively fire for items that already have the owning
                  category. Action-applied assignments are sticky.

    NOTE          The category manager shows an action badge such as [1 action] in
                  the left pane.

#### SEE ALSO

> [Profile conditions](#profile-conditions),   [Automatic assignment](#automatic-assignment-implicit-string-matching),   [CLI category commands](#cli-category-commands),   [» Index](#index)

## Review Classification Suggestions
    PURPOSE       Accept or reject category suggestions, including experimental
                  LLM-based ones, before they are applied.

    TUI STEPS     Press = to classify the selected item(s) now. Press C to review
                  pending classification suggestions in the suggestion-review mode,
                  where you can accept or dismiss each one.

    HOW IT WORKS  Classification proposes categories for an item from its text and
                  rules. Accepting a suggestion creates a sticky assignment;
                  dismissing it does not. This is the aglet analog of Agenda's
                  "execute conditions" plus modern suggestion review.

    NOTE          aglet has experimental support for LLM-based categorization in
                  addition to implicit-string and profile-condition matching.

#### SEE ALSO

> [Automatic assignment](#automatic-assignment-implicit-string-matching),   [Profile conditions](#profile-conditions),   [Assign a category](#assign-a-category-to-an-item),   [» Index](#index)

## Set a Numeric Value
    PURPOSE       Give an item a number under a numeric category - a cost, quantity,
                  mileage, or effort estimate.

    TUI STEPS     On a numeric column cell, press Enter to edit the value inline. In
                  the item editor's inline category list, an assigned numeric
                  category shows - [N] with an editable value field.

    CLI STEPS     aglet category set-value  Cost 450.00

    HOW IT WORKS  The value is stored on the assignment edge between the item and
                  the numeric category. A numeric column then displays the value and
                  can be summarized per section (sum, average, min, max).

    NOTE          Editing a numeric column cell is the primary way to assign a value
                  to an item for the first time.

#### SEE ALSO

> [Numeric categories](#numeric-categories),   [Format a numeric column](#format-a-numeric-column),   [Column summaries](#column-summary-functions),   [Add a column](#add-a-column-to-a-section),   [» Index](#index)

## Format a Numeric Column
    PURPOSE       Control how a numeric category's values are displayed - decimal
                  places, currency, and thousands separators.

    TUI STEPS     On a numeric column, press f to cycle the column's display format.

    CLI STEPS     aglet category format  ...

    HOW IT WORKS  Formatting is a display property of the numeric category; it does
                  not change the stored value. The same value can appear as 1450,
                  1,450.00, or $1,450.00 depending on format.

    NOTE          Use F (capital) to cycle the column's summary function; f sets the
                  value format. See Column summaries.

#### SEE ALSO

> [Numeric categories](#numeric-categories),   [Set a numeric value](#set-a-numeric-value),   [Column summaries](#column-summary-functions),   [» Index](#index)

## Organize the Category Hierarchy
    PURPOSE       Rearrange categories - reparent, promote, demote, and reorder
                  siblings.

    TUI STEPS     In the category manager, use H/J/K/L to reorder and move
                  categories, and << / >> to change a category's depth level.

    CLI STEPS     aglet category reparent  --parent 
                  aglet category reparent  --root      # make top-level
                  aglet category rename  "New Name"

    HOW IT WORKS  Child order matters for exclusive parents: when several derived
                  siblings match, the earlier child in parent order wins. Manual and
                  accepted-suggestion assignments remain durable regardless of order.

    NOTE          Workflow roles (which categories mean Ready, claim-target, etc.)
                  are set in Global Settings, not by hierarchy position.

#### SEE ALSO

> [The category hierarchy](#the-category-hierarchy),   [Exclusive categories](#exclusive-categories),   [Subsumption](#subsumption),   [Global settings](#global-settings),   [» Index](#index)

## Discard a Category
    PURPOSE       Delete a category you no longer need.

    TUI STEPS     In the category manager, select the category and delete it.

    CLI STEPS     aglet category delete "Old Category"

    HOW IT WORKS  Deleting a category removes it and its assignments. Reserved
                  categories (Done, When, Entry) cannot be deleted.

    NOTE          Consider turning off implicit matching instead of deleting if you
                  only want to stop auto-assignment but keep past filings.

#### SEE ALSO

> [Add a category](#add-a-category),   [Unassign a category](#unassign-a-category),   [Reserved categories](#reserved-categories),   [» Index](#index)

## Create a View
    PURPOSE       Save a lens over the item database - a filtered, sectioned, and
                  columned presentation you can return to.

    TUI STEPS     1. Press v, V, or F8 to open the view picker.
                  2. Press n to create a new view and name it (Work Queue).
                  3. Add include/exclude criteria and any sections.
                  4. Press Ctrl-S (or S) to save.

    CLI STEPS     aglet view create "Work Queue" --include Work --exclude Done
                  aglet view list
                  aglet view show "Work Queue"

    HOW IT WORKS  A view stores its criteria, sections, columns, layout, and
                  aliases. It updates automatically as items are added or
                  reassigned.

    NOTE          Include criteria are AND-based: --include Work --include Urgent
                  matches only items with both categories.

#### SEE ALSO

> [Views](#views),   [View criteria](#view-criteria),   [Add a section](#add-a-section-to-a-view),   [The view editor](#the-view-editor),   [Clone a view](#clone-a-view),   [» Index](#index)

## View Criteria
    PURPOSE       Control which items a view includes.

    CLI STEPS     aglet view create "My View" --include High --include Pending
                  aglet view edit  ...

    HOW IT WORKS  Include filters are AND-based - every included category must be
                  present. Do not use repeated includes for mutually exclusive
                  siblings such as Pending and In Progress; use separate sections or
                  views instead. Views also persist hide_dependent_items, which
                  hides items that are blocked dependents.

    NOTE          Toggle the hide-dependent-items filter in the TUI with u.

#### SEE ALSO

> [Create a view](#create-a-view),   [CLI filtering](#cli-filtering-and-sorting),   [Filter blocked items](#filter-blocked--not-blocked-items),   [Add a section](#add-a-section-to-a-view),   [» Index](#index)

## Add a Section to a View
    PURPOSE       Group a view's items into labelled lanes by category criteria.

    CLI STEPS     aglet view section add  ...
                  aglet view section update  ...
                  aglet view section remove  ...

    TUI STEPS     Open the view editor (from the view picker), focus the SECTIONS
                  pane, and add or edit sections there.

    HOW IT WORKS  Each section has its own include criteria. Items that match land
                  in that section; section flow can be vertical (stacked) or
                  horizontal (kanban lanes). Unmatched-item visibility is a view
                  setting.

    NOTE          Moving an item between sections adjusts its category assignments
                  to match the destination section.

#### SEE ALSO

> [Sections](#sections),   [Add a column](#add-a-column-to-a-section),   [Move an item between sections](#move-an-item-between-sections),   [The view editor](#the-view-editor),   [» Index](#index)

## Add a Column to a Section
    PURPOSE       Show a numeric category's value as a column beside each item, with
                  an optional per-section summary.

    TUI STEPS     Press + to add a board column and - to remove one. Use H / L to
                  move a column left or right, and Enter on a column cell to edit a
                  value.

    CLI STEPS     aglet view column add  ...
                  aglet view column update  ...
                  aglet view column remove  ...

    HOW IT WORKS  Columns display numeric category values. Each column can carry a
                  summary function shown in the section footer.

    NOTE          Columns are numeric in aglet; there is no free-text column type.

#### SEE ALSO

> [Columns](#columns),   [Column summaries](#column-summary-functions),   [Set a numeric value](#set-a-numeric-value),   [Format a numeric column](#format-a-numeric-column),   [» Index](#index)

## Column Summary Functions
    PURPOSE       Aggregate a numeric column across a section - a total, average, or
                  extreme.

    TUI STEPS     Press F to cycle a numeric column's summary (Sum / Avg / Min /
                  Max). Sort a section by a column with s / S (or < / >).

    CLI STEPS     aglet view set-summary  ...

    HOW IT WORKS  The summary is computed over the items in the section and shown in
                  the section footer - for example a budget total or average effort.

    NOTE          f cycles the value display format; F cycles the summary function.

#### SEE ALSO

> [Add a column](#add-a-column-to-a-section),   [Numeric categories](#numeric-categories),   [Format a numeric column](#format-a-numeric-column),   [» Index](#index)

## Create a Datebook View
    PURPOSE       Build a view that buckets dated items into calendar ranges.

    CLI STEPS     aglet view create-datebook "Scheduling" ...

    HOW IT WORKS  A datebook view is a distinct view type that groups items by their
                  When date into date-interval buckets (days, weeks, etc.). It is
                  ideal for upcoming work, appointments, renewals, and deadlines.

    NOTE          A standard view and a datebook view are different types; one
                  cannot be converted into the other.

#### SEE ALSO

> [Datebook views](#datebook-views),   [Browse a datebook view](#browse-a-datebook-view),   [Date conditions](#date-conditions),   [Datebook keys](#tui-datebook-keys),   [» Index](#index)

## Browse a Datebook View
    PURPOSE       Move the visible date window of a datebook view forward and back.

    TUI STEPS     { and } step to the previous / next bucket. ( and ) step the
                  window. 0 jumps to today.

    CLI STEPS     aglet view datebook-browse  ...

    HOW IT WORKS  Browsing shifts which date interval is shown without changing the
                  view definition.

    NOTE          0 (today) is the quickest way to recenter after browsing.

#### SEE ALSO

> [Create a datebook view](#create-a-datebook-view),   [Datebook views](#datebook-views),   [Datebook keys](#tui-datebook-keys),   [» Index](#index)

## The View Editor
    PURPOSE       Configure a view's filters, sections, columns, layout, aliases,
                  and preview behavior in one screen.

    TUI STEPS     Open a view in the editor from the view picker. Tab moves between
                  regions (Criteria, Datebook, Sections, Unmatched). Enter operates
                  the focused row or inline input. Save with S or Ctrl-S.

    HOW IT WORKS  The editor edits mutable view properties: include/exclude
                  criteria, date ranges, display mode (single- or multi-line),
                  section flow (stacked or lanes), unmatched-item visibility, and
                  category aliases. A live preview reflects changes.

    NOTE          The All Items view is immutable; clone it first to edit a copy.

#### SEE ALSO

> [Create a view](#create-a-view),   [View criteria](#view-criteria),   [Add a section](#add-a-section-to-a-view),   [View aliases](#view-aliases),   [View editor keys](#tui-view-editor-keys),   [» Index](#index)

## View Aliases
    PURPOSE       Show a category under a different display name inside one view,
                  without changing the category itself.

    CLI STEPS     aglet view alias set   "Display Name"
                  aglet view alias clear  

    HOW IT WORKS  An alias is per-view display metadata mapping a category to a
                  label. It affects only how the category is shown in that view.

    NOTE          Aliases do not change category identity, filters, section titles,
                  generated subsection labels, or board column headings.

#### SEE ALSO

> [The view editor](#the-view-editor),   [Views](#views),   [CLI view commands](#cli-view-commands),   [» Index](#index)

## Clone a View
    PURPOSE       Make an editable copy of an existing view, including the immutable
                  All Items view.

    CLI STEPS     aglet view clone "All Items" "My Items"

    HOW IT WORKS  Cloning copies the source view's criteria, sections, columns, and
                  settings into a new, mutable view that you can then customize.

    NOTE          Cloning is the way to start from All Items, which cannot itself be
                  edited.

#### SEE ALSO

> [Create a view](#create-a-view),   [The All Items view](#the-all-items-view),   [Discard a view](#discard-a-view),   [» Index](#index)

## Discard a View
    PURPOSE       Delete a view you no longer need.

    CLI STEPS     aglet view delete "Old View"
                  aglet view rename "Old Name" "New Name"

    HOW IT WORKS  Deleting a view removes the saved presentation only; the items it
                  showed remain in the database. The All Items view cannot be
                  deleted.

    NOTE          Deleting a view never deletes items - it only discards the lens.

#### SEE ALSO

> [Create a view](#create-a-view),   [Clone a view](#clone-a-view),   [The All Items view](#the-all-items-view),   [» Index](#index)

## Create a Dependency Link
    PURPOSE       Record that one item must wait for another, or relate two items.

    TUI STEPS     Press b or B on an item (or selection) to open the dependency link
                  wizard and choose the blocked-by / blocks relationship.

    CLI STEPS     aglet link depends-on  
                  aglet link blocks  
                  aglet link related  

    HOW IT WORKS  depends-on and blocks are two vocabularies for the same directed
                  relationship; related is bidirectional. An item with an unresolved
                  depends-on prerequisite is "blocked". Done prerequisites do not
                  block.

    NOTE          Do not create synthetic links to mean "someone is working on
                  this" - use claim for that. Links are for real prerequisites.

#### SEE ALSO

> [Dependencies](#dependencies),   [Remove a link](#remove-a-link),   [Filter blocked items](#filter-blocked--not-blocked-items),   [The link wizard](#the-link-wizard),   [» Index](#index)

## Remove a Link
    PURPOSE       Delete a dependency or related link between two items.

    CLI STEPS     aglet unlink depends-on  
                  aglet unlink blocks  
                  aglet unlink related  

    HOW IT WORKS  Removing the last unresolved depends-on prerequisite unblocks the
                  dependent item, which can make it claimable again.

    NOTE          unlink is the canonical entry point; the depends-on and blocks
                  forms remove the same underlying relationship from either
                  direction.

#### SEE ALSO

> [Create a dependency](#create-a-dependency-link),   [Dependencies](#dependencies),   [Filter blocked items](#filter-blocked--not-blocked-items),   [» Index](#index)

## Filter Blocked / Not-Blocked Items
    PURPOSE       Show only items that are waiting on a prerequisite, or only those
                  that are free to start.

    TUI STEPS     Press u to toggle the hide-dependent-items filter for the current
                  view.

    CLI STEPS     aglet list --blocked
                  aglet list --not-blocked
                  aglet search  --blocked
                  aglet view show "" --not-blocked

    HOW IT WORKS  blocked means the item has at least one unresolved depends-on
                  prerequisite, computed at query time from the dependency graph.
                  Done dependencies do not count.

    NOTE          Dependency-state filters are derived, not assignable categories;
                  you cannot assign "blocked" to an item.

#### SEE ALSO

> [Dependencies](#dependencies),   [Create a dependency](#create-a-dependency-link),   [The ready list](#the-ready-list),   [CLI filtering](#cli-filtering-and-sorting),   [» Index](#index)

## The Link Wizard
    PURPOSE       Create dependency links interactively in the TUI.

    TUI STEPS     Press b or B on an item or a multi-item selection to open the
                  wizard, then pick the other item and the relationship direction
                  (blocked-by or blocks).

    HOW IT WORKS  The wizard writes the same depends-on / blocks links as the CLI.
                  With a selection, it can link several items at once.

    NOTE          Use the wizard for prerequisites; use claim/release for "who is
                  working on it".

#### SEE ALSO

> [Create a dependency](#create-a-dependency-link),   [Select multiple items](#select-multiple-items),   [CLI link commands](#cli-link-commands),   [» Index](#index)

## Claim an Item
    PURPOSE       Atomically take an eligible item for active work, marking it as
                  yours.

    CLI STEPS     aglet claim 

    HOW IT WORKS  Claiming applies the configured claim-target category. An item is
                  claimable only if it has the configured Ready category, does not
                  already have the claim-target category, is not done, and is not
                  blocked by an unresolved depends-on prerequisite. Claimability is
                  computed, not a link type.

    NOTE          If a claim races and fails because the item is already In
                  Progress, pick another item rather than force-assigning.

#### SEE ALSO

> [The ready list](#the-ready-list),   [Release a claim](#release-a-claim),   [Filter blocked items](#filter-blocked--not-blocked-items),   [Global settings](#global-settings),   [» Index](#index)

## Release a Claim
    PURPOSE       Give up an item you previously claimed.

    CLI STEPS     aglet release 
                  aglet unclaim       # alias

    HOW IT WORKS  Releasing removes the active claim-target category, returning the
                  item to the pool of claimable work if it still has the Ready
                  category.

    NOTE          release and unclaim are the same command.

#### SEE ALSO

> [Claim an item](#claim-an-item),   [The ready list](#the-ready-list),   [» Index](#index)

## The Ready List
    PURPOSE       See the items that are eligible to be claimed right now.

    CLI STEPS     aglet ready

    HOW IT WORKS  ready lists items that have the Ready category and are not done,
                  not already claimed, and not blocked by unresolved dependencies.
                  It is the recommended way to pick the next piece of work.

    NOTE          Because ready already excludes done, claimed, and blocked items,
                  prefer it over scanning a full list when choosing work.

#### SEE ALSO

> [Claim an item](#claim-an-item),   [Release a claim](#release-a-claim),   [Filter blocked items](#filter-blocked--not-blocked-items),   [» Index](#index)

## Global Settings
    PURPOSE       Adjust application-wide preferences and workflow roles.

    TUI STEPS     Press g s or F10 to open Global Settings.

    HOW IT WORKS  Global Settings control display and behavior preferences such as
                  auto-refresh, section borders, note glyphs, and search mode, and
                  they define workflow roles - which categories act as Ready and as
                  the claim target used by claim/release/ready.

    NOTE          Workflow roles live here, not in the category hierarchy; reorder
                  or rename categories without changing which one is "Ready".

#### SEE ALSO

> [Claim an item](#claim-an-item),   [The ready list](#the-ready-list),   [Organize the hierarchy](#organize-the-category-hierarchy),   [» Index](#index)

## About .ag Files
    PURPOSE       Understand where aglet keeps your data.

    HOW IT WORKS  An aglet database is a single SQLite file with the .ag extension.
                  It holds every item, category, view, link, and the deletion log.
                  A new database is created automatically with the built-in
                  categories (Done, When, Entry) and the All Items view the first
                  time you open it.

    EXAMPLES      cargo run --bin aglet -- --db getting-started.ag

    NOTE          Because the database is one file, you back it up or move it by
                  copying that file. Schema repair runs idempotently on open.

#### SEE ALSO

> [The AGLET_DB variable](#the-aglet_db-environment-variable),   [Reserved categories](#reserved-categories),   [About the CLI](#about-the-cli),   [» Index](#index)

## The AGLET_DB Environment Variable
    PURPOSE       Choose which database aglet acts on without repeating --db.

    HOW IT WORKS  The CLI reads the database path from --db  or, if that is
                  absent, the AGLET_DB environment variable. Set AGLET_DB once in
                  your shell to run a series of commands against the same file.

    EXAMPLES      export AGLET_DB=~/notes.ag
                  aglet list
                  aglet add "Pick up parts"

    NOTE          --db on a single command overrides AGLET_DB for that command.

#### SEE ALSO

> [About .ag files](#about-ag-files),   [About the CLI](#about-the-cli),   [» Index](#index)

## About the CLI
    PURPOSE       Drive aglet from the command line - useful for scripting and for
                  LLM coding agents.

    USAGE         aglet [--db ] 

    HOW IT WORKS  Running aglet with no subcommand opens the TUI; with a subcommand
                  it performs that action and exits. Short item-id prefixes work
                  anywhere an item id is accepted (case-insensitive, hyphens
                  stripped); an ambiguous prefix returns an error listing the
                  matches. list and search default to compact one-line rows; use
                  --verbose for multi-line output and --format json for scripts.

    EXAMPLES      aglet --db notes.ag list
                  aglet --db notes.ag add "Buy groceries"

    NOTE          aglet list without --view uses All Items when present, then the
                  first stored view.

#### SEE ALSO

> [CLI command chart](#cli-command-chart),   [CLI item commands](#cli-item-commands),   [CLI category commands](#cli-category-commands),   [CLI view commands](#cli-view-commands),   [The AGLET_DB variable](#the-aglet_db-environment-variable),   [» Index](#index)

## CLI Item Commands
    PURPOSE       Create, inspect, modify, and remove items from the command line.

    COMMANDS      add       Add a new item (--note, returns the created id)
                  edit      Edit text, note, and/or done state (--text, --note,
                            --done, --not-done)
                  show      Show a single item with its assignments
                  list      List items, optionally filtered
                  search    Search item text and note
                  export    Export items as Markdown
                  delete    Delete an item (writes deletion log)
                  deleted   List deletion-log entries
                  restore   Restore an item from the deletion log by log id
                  item      Alternative noun-verb syntax for item commands

    EXAMPLES      aglet add "Plan offsite" --note "Book venue"
                  aglet show 
                  aglet export > items.md

    NOTE          Parse the "created " line from add for the new id; it is not
                  always the last line printed.

#### SEE ALSO

> [Add an item](#add-an-item),   [Edit an item](#edit-an-item),   [Delete an item](#delete-an-item),   [CLI filtering](#cli-filtering-and-sorting),   [CLI import and export](#cli-import-and-export),   [» Index](#index)

## CLI Category Commands
    PURPOSE       Manage categories, assignments, conditions, and actions from the
                  command line.

    COMMANDS      list                List categories as a tree
                  show                Show details for a category
                  create              Create a category (--parent, --exclusive,
                                      --type numeric)
                  delete              Delete a category by name
                  rename              Rename a category
                  reparent            Reparent (--root makes it top-level)
                  update              Update category flags
                  assign              Assign an item to a category
                  unassign            Unassign an item from a category
                  set-value           Set a numeric value assignment
                  format              Configure numeric formatting
                  add-condition       Add a profile condition
                  add-date-condition  Add a date condition
                  set-condition-mode  Set how explicit conditions combine
                  remove-condition    Remove a condition by 1-based index
                  add-action          Add an action
                  remove-action       Remove an action by 1-based index

    EXAMPLES      aglet category create "Priority" --exclusive
                  aglet category create "High" --parent Priority
                  aglet category assign  High
                  aglet category set-value  Cost 450.00

    NOTE          Done, When, and Entry are reserved and cannot be created, renamed,
                  or deleted.

#### SEE ALSO

> [Add a category](#add-a-category),   [Assign a category](#assign-a-category-to-an-item),   [Profile conditions](#profile-conditions),   [Actions](#actions),   [Set a numeric value](#set-a-numeric-value),   [» Index](#index)

## CLI View Commands
    PURPOSE       Create and edit views, sections, columns, aliases, and datebooks
                  from the command line.

    COMMANDS      list                  List views
                  show                  Show the contents of a view
                  create                Create a view from include/exclude
                  edit                  Edit mutable view properties
                  clone                 Clone into a new mutable view
                  rename                Rename a view
                  delete                Delete a view by name
                  section add/remove/update    Section authoring
                  column add/remove/update     Column authoring
                  alias set/clear              Per-view category display alias
                  set-summary           Set a column summary function
                  set-item-label        Set or clear the item column label
                  set-remove-from-view  Replace the remove-from-view category set
                  create-datebook       Create a datebook (date-interval) view
                  datebook-browse       Shift a datebook view's window

    EXAMPLES      aglet view create "Work Queue" --include Work --exclude Done
                  aglet view show "Work Queue"
                  aglet view clone "All Items" "My Items"

    NOTE          --include criteria are AND-based; use sections or separate views
                  for mutually exclusive siblings.

#### SEE ALSO

> [Create a view](#create-a-view),   [Add a section](#add-a-section-to-a-view),   [Add a column](#add-a-column-to-a-section),   [View aliases](#view-aliases),   [Create a datebook view](#create-a-datebook-view),   [» Index](#index)

## CLI Link Commands
    PURPOSE       Create and remove item-to-item links from the command line.

    COMMANDS      link depends-on   ITEM depends on DEPENDS_ON_ITEM
                  link blocks       BLOCKER blocks BLOCKED
                  link related      Bidirectional related link
                  unlink depends-on / blocks / related   Remove the corresponding
                                                         link (canonical entry)

    EXAMPLES      aglet link depends-on  
                  aglet link related  
                  aglet unlink depends-on  

    NOTE          depends-on and blocks describe the same directed relationship from
                  opposite ends; related is symmetric.

#### SEE ALSO

> [Create a dependency](#create-a-dependency-link),   [Remove a link](#remove-a-link),   [Dependencies](#dependencies),   [» Index](#index)

## CLI Filtering and Sorting
    PURPOSE       Narrow and order the output of list, search, and view show.

    FLAGS         --category             Repeatable, AND semantics
                  --any-category         Repeatable, OR semantics
                  --exclude-category     Repeatable, NOT semantics
                  --blocked / --not-blocked Dependency-state filters (derived)
                  --value-eq          Numeric value equals
                  --value-in        Numeric value in a set
                  --value-max         Numeric value at most
                  --sort   Sort order
                  --view              Use a specific view
                  --include-done            Include done items
                  --format          Output format (e.g. json)
                  --verbose                 Multi-line human output

    EXAMPLES      aglet list --category High --not-blocked
                  aglet list --any-category Work --any-category Personal
                  aglet list --sort Cost:desc --format json

    NOTE          Repeated --category is AND; for OR across siblings use
                  --any-category. blocked/not-blocked are derived, not categories.

#### SEE ALSO

> [View criteria](#view-criteria),   [Filter blocked items](#filter-blocked--not-blocked-items),   [Search items](#search-items),   [CLI item commands](#cli-item-commands),   [» Index](#index)

## CLI Import and Export
    PURPOSE       Move items into and out of aglet.

    COMMANDS      import   Structured import commands (e.g. CSV)
                  export   Export items as Markdown

    EXAMPLES      aglet export > items.md
                  aglet import ...

    HOW IT WORKS  Export writes items as Markdown to standard output. Import brings
                  structured data in. Prefer the CLI/import path over direct SQLite
                  writes so dates and reserved When provenance are handled
                  correctly.

    NOTE          Direct SQLite imports must use the store datetime format
                  YYYY-MM-DD HH:MM:SS; ISO strings will not load as dates.

#### SEE ALSO

> [CLI item commands](#cli-item-commands),   [About the CLI](#about-the-cli),   [About .ag files](#about-ag-files),   [» Index](#index)

## The Status and Hint Footer
    PURPOSE       Read the two-row footer at the bottom of the TUI.

    HOW IT WORKS  The footer has two rows. The top row shows transient status - the
                  result of your last action or a brief message. The bottom row
                  shows persistent key hints for the current mode, so the available
                  keys change as you move between Normal view, the category manager,
                  the view editor, and the item editor.

    NOTE          Press ? at any time to open the full in-app help panel, which lists
                  every key for the current context.

#### SEE ALSO

> [Normal mode keys](#tui-normal-mode-keys),   [Assignment profile](#the-item-assignment-profile),   [» Index](#index)

## The Item Assignment Profile
    PURPOSE       Understand the assignments and provenance shown by "aglet show".

    HOW IT WORKS  "aglet show" prints an item's text, note, status, and its
                  assignments. Displayed category lists include both assigned
                  categories and their parents (assigning High also shows Priority).
                  Provenance distinguishes manual assignments from auto-classified
                  ones (implicit_string or other providers), so you can see why a
                  category is present.

    NOTE          Implicit matches scan the note as well as the title, so a category
                  can appear because of text inside the note. Check provenance
                  before assuming a visible category was set by hand.

#### SEE ALSO

> [Automatic assignment](#automatic-assignment-implicit-string-matching),   [Assign a category](#assign-a-category-to-an-item),   [Categories](#categories),   [CLI item commands](#cli-item-commands),   [» Index](#index)
