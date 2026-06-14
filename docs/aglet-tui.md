<!-- GENERATED from docs/src/.htm — DO NOT EDIT. Run   MD   aglet-cli.md in docs/. -->

# Aglet TUI Guide

[« Home](index.md)  \| 
[Concepts](aglet-manual.md)  \|  [CLI Reference](aglet-cli.md)

## <span id="how to use this manual">How to Use This Manual</span>

    PURPOSE    Complete guide to the aglet terminal interface: keybindings,
               interactive workflows, and the view/category editors. For
               scripting and batch use see the
               CLI Reference.
               Core concepts are in the
               Concepts Reference.

#### SEE ALSO

> [Home](index.md)

## <span id="index">Index</span>

    KEYS AND COMMANDS
      TUI: Normal Mode Keys
      TUI: Category Manager Keys
      TUI: View Editor Keys
      TUI: Item Editor Keys
      TUI: Datebook Keys

    WORKING WITH ITEMS
      Add an Item
      Edit an Item
      Add a Note to an Item
      Mark an Item Done
      Recurrence
      Delete an Item
      Restore a Deleted Item
      Move an Item Between Sections
      Search Items
      Select Multiple Items

    WORKING WITH CATEGORIES
      Add a Category
      Assign a Category to an Item
      Unassign a Category
      Review Classification Suggestions
      Set a Numeric Value
      Format a Numeric Column
      Organize the Category Hierarchy
      Discard a Category

    WORKING WITH VIEWS
      Create a View
      View Criteria
      Add a Section to a View
      Add a Column to a Section
      Column Summary Functions
      Create a Datebook View
      Browse a Datebook View
      The View Editor
      View Aliases
      Clone a View
      Discard a View

    WORKING WITH DEPENDENCIES
      Create a Dependency Link
      Remove a Link
      Filter Blocked / Not-Blocked Items
      The Link Wizard

    SETTINGS AND INDICATORS
      Global Settings
      The Status and Hint Footer
      The Item Assignment Profile

# Keys and Commands

## <span id="tui normal mode keys">TUI: Normal Mode Keys</span>

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

> [Category manager keys](#tui%20category%20manager%20keys),   [View editor keys](#tui%20view%20editor%20keys),   [Select multiple items](#select%20multiple%20items),   [» Index](#index)

## <span id="tui category manager keys">TUI: Category Manager Keys</span>

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

> [Categories](aglet-manual.md#categories),   [Organize the category hierarchy](#organize%20the%20hierarchy),   [Actions](aglet-cli.md#actions),   [» Index](#index)

## <span id="tui view editor keys">TUI: View Editor Keys</span>

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

> [Views](aglet-manual.md#views),   [Create a view](#create%20a%20view),   [View criteria](#view%20criteria),   [View aliases](#view%20aliases),   [» Index](#index)

## <span id="tui item editor keys">TUI: Item Editor Keys</span>

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

> [Add an item](#add%20an%20item),   [Edit an item](#edit%20an%20item),   [Add a note](#add%20a%20note%20to%20an%20item),   [Assign a category](#assign%20a%20category),   [» Index](#index)

## <span id="tui datebook keys">TUI: Datebook Keys</span>

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

> [Datebook views](aglet-manual.md#datebook%20views),   [Create a datebook view](#create%20a%20datebook),   [Browse a datebook view](#browse%20a%20datebook),   [» Index](#index)

# Working with Items

## <span id="add an item">Add an Item</span>

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

> [Items](aglet-manual.md#items),   [Edit an item](#edit%20an%20item),   [Automatic assignment](aglet-manual.md#automatic%20assignment),   [Add a note](#add%20a%20note%20to%20an%20item),   [Item editor keys](#tui%20item%20editor%20keys),   [» Index](#index)

## <span id="edit an item">Edit an Item</span>

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

> [Add an item](#add%20an%20item),   [Add a note](#add%20a%20note%20to%20an%20item),   [Mark an item done](#mark%20an%20item%20done),   [Automatic assignment](aglet-manual.md#automatic%20assignment),   [» Index](#index)

## <span id="add a note to an item">Add a Note to an Item</span>

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

> [Notes](aglet-manual.md#notes),   [Edit an item](#edit%20an%20item),   [Automatic assignment](aglet-manual.md#automatic%20assignment),   [» Index](#index)

## <span id="mark an item done">Mark an Item Done</span>

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

> [Reserved categories](aglet-manual.md#reserved%20categories),   [Recurrence](#recurrence),   [Dependencies](aglet-manual.md#dependencies),   [» Index](#index)

## <span id="recurrence">Recurrence</span>

    PURPOSE       Make an item repeat on a schedule. When a recurring item is
                  completed, aglet generates the next occurrence automatically.

    HOW IT WORKS  A recurrence rule is attached to a dated item. Completing the
                  current occurrence (marking it Done) advances the When date to
                  the next scheduled date and reopens the item, so recurring chores,
                  bills, and maintenance reappear without re-entry.

    EXAMPLES      A monthly "Pay insurance" item set to recur reappears with next
                  month's date each time you mark it done.

    NOTE          Recurrence works together with the reserved When and Done
                  categories.

#### SEE ALSO

> [Mark an item done](#mark%20an%20item%20done),   [Reserved categories](aglet-manual.md#reserved%20categories),   [Datebook views](aglet-manual.md#datebook%20views),   [» Index](#index)

## <span id="delete an item">Delete an Item</span>

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

> [Restore a deleted item](#restore%20a%20deleted%20item),   [Items](aglet-manual.md#items),   [CLI item commands](aglet-cli.md#cli%20item%20commands),   [» Index](#index)

## <span id="restore a deleted item">Restore a Deleted Item</span>

    PURPOSE       Bring back an item that was deleted, using the deletion log.

    CLI STEPS     1. aglet deleted               # find the log entry id
                  2. aglet restore       # restore that entry

    HOW IT WORKS  Every delete appends an entry to the deletion log. Restoring
                  recreates the item with its text, note, and recorded state.

    NOTE          Restore brings the item back exactly as it was deleted; the
                  deletion log keeps a history you can recover from.

#### SEE ALSO

> [Delete an item](#delete%20an%20item),   [CLI item commands](aglet-cli.md#cli%20item%20commands),   [» Index](#index)

## <span id="move an item between sections">Move an Item Between Sections</span>

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

> [Sections](aglet-manual.md#sections),   [Add a section](#add%20a%20section),   [Normal mode keys](#tui%20normal%20mode%20keys),   [» Index](#index)

## <span id="search items">Search Items</span>

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

> [Items](aglet-manual.md#items),   [Select multiple items](#select%20multiple%20items),   [CLI filtering](aglet-cli.md#cli%20filtering),   [» Index](#index)

## <span id="select multiple items">Select Multiple Items</span>

    PURPOSE       Operate on several items at once - assign, complete, delete, or
                  classify them together.

    TUI STEPS     1. Press Space to toggle selection on the current item; repeat to
                     build a selection.
                  2. Apply a batch operation: a (assign categories), d (done),
                     x (delete), = (classify now), or b / B (link with a
                     dependency).
                  3. Press Esc to clear the selection.

    HOW IT WORKS  Selection lets a single command act on many items at once.
                  Batch operations act on every selected item.

    NOTE          With no explicit selection, the same keys act on the single
                  item under the cursor.

#### SEE ALSO

> [Assign a category](#assign%20a%20category),   [Mark an item done](#mark%20an%20item%20done),   [Create a dependency](#create%20a%20dependency),   [Review suggestions](#review%20classification%20suggestions),   [» Index](#index)

# Working with Categories

## <span id="add a category">Add a Category</span>

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

> [Categories](aglet-manual.md#categories),   [Tag categories](aglet-manual.md#tag%20categories),   [Numeric categories](aglet-manual.md#numeric%20categories),   [Exclusive categories](aglet-manual.md#exclusive%20categories),   [Organize the hierarchy](#organize%20the%20hierarchy),   [» Index](#index)

## <span id="assign a category">Assign a Category to an Item</span>

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

> [Unassign a category](#unassign%20a%20category),   [Automatic assignment](aglet-manual.md#automatic%20assignment),   [Exclusive categories](aglet-manual.md#exclusive%20categories),   [Set a numeric value](#set%20a%20numeric%20value),   [» Index](#index)

## <span id="unassign a category">Unassign a Category</span>

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

> [Assign a category](#assign%20a%20category),   [Automatic assignment](aglet-manual.md#automatic%20assignment),   [Discard a category](#discard%20a%20category),   [» Index](#index)

## <span id="review classification suggestions">Review Classification Suggestions</span>

    PURPOSE       Accept or reject category suggestions, including experimental
                  LLM-based ones, before they are applied.

    TUI STEPS     Press = to classify the selected item(s) now. Press C to review
                  pending classification suggestions in the suggestion-review mode,
                  where you can accept or dismiss each one.

    HOW IT WORKS  Classification proposes categories for an item from its text and
                  rules. Accepting a suggestion creates a sticky assignment;
                  dismissing it does not. It runs aglet's rule-based and
                  LLM-based suggestions on demand for review.

    NOTE          aglet has experimental support for LLM-based categorization in
                  addition to implicit-string and profile-condition matching.

#### SEE ALSO

> [Automatic assignment](aglet-manual.md#automatic%20assignment),   [Profile conditions](aglet-cli.md#profile%20conditions),   [Assign a category](#assign%20a%20category),   [» Index](#index)

## <span id="set a numeric value">Set a Numeric Value</span>

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

> [Numeric categories](aglet-manual.md#numeric%20categories),   [Format a numeric column](#format%20a%20numeric%20column),   [Column summaries](#column%20summaries),   [Add a column](#add%20a%20column),   [» Index](#index)

## <span id="format a numeric column">Format a Numeric Column</span>

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

> [Numeric categories](aglet-manual.md#numeric%20categories),   [Set a numeric value](#set%20a%20numeric%20value),   [Column summaries](#column%20summaries),   [» Index](#index)

## <span id="organize the hierarchy">Organize the Category Hierarchy</span>

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

> [The category hierarchy](aglet-manual.md#the%20category%20hierarchy),   [Exclusive categories](aglet-manual.md#exclusive%20categories),   [Subsumption](aglet-manual.md#subsumption),   [Global settings](#global%20settings),   [» Index](#index)

## <span id="discard a category">Discard a Category</span>

    PURPOSE       Delete a category you no longer need.

    TUI STEPS     In the category manager, select the category and delete it.

    CLI STEPS     aglet category delete "Old Category"

    HOW IT WORKS  Deleting a category removes it and its assignments. Reserved
                  categories (Done, When, Entry) cannot be deleted.

    NOTE          Consider turning off implicit matching instead of deleting if you
                  only want to stop auto-assignment but keep past filings.

#### SEE ALSO

> [Add a category](#add%20a%20category),   [Unassign a category](#unassign%20a%20category),   [Reserved categories](aglet-manual.md#reserved%20categories),   [» Index](#index)

# Working with Views

## <span id="create a view">Create a View</span>

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

> [Views](aglet-manual.md#views),   [View criteria](#view%20criteria),   [Add a section](#add%20a%20section),   [The view editor](#the%20view%20editor),   [Clone a view](#clone%20a%20view),   [» Index](#index)

## <span id="view criteria">View Criteria</span>

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

> [Create a view](#create%20a%20view),   [CLI filtering](aglet-cli.md#cli%20filtering),   [Filter blocked items](#filter%20blocked%20items),   [Add a section](#add%20a%20section),   [» Index](#index)

## <span id="add a section">Add a Section to a View</span>

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

> [Sections](aglet-manual.md#sections),   [Add a column](#add%20a%20column),   [Move an item between sections](#move%20an%20item%20between%20sections),   [The view editor](#the%20view%20editor),   [» Index](#index)

## <span id="add a column">Add a Column to a Section</span>

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

> [Columns](aglet-manual.md#columns),   [Column summaries](#column%20summaries),   [Set a numeric value](#set%20a%20numeric%20value),   [Format a numeric column](#format%20a%20numeric%20column),   [» Index](#index)

## <span id="column summaries">Column Summary Functions</span>

    PURPOSE       Aggregate a numeric column across a section - a total, average, or
                  extreme.

    TUI STEPS     Press F to cycle a numeric column's summary (Sum / Avg / Min /
                  Max). Sort a section by a column with s / S (or < / >).

    CLI STEPS     aglet view set-summary  ...

    HOW IT WORKS  The summary is computed over the items in the section and shown in
                  the section footer - for example a budget total or average effort.

    NOTE          f cycles the value display format; F cycles the summary function.

#### SEE ALSO

> [Add a column](#add%20a%20column),   [Numeric categories](aglet-manual.md#numeric%20categories),   [Format a numeric column](#format%20a%20numeric%20column),   [» Index](#index)

## <span id="create a datebook">Create a Datebook View</span>

    PURPOSE       Build a view that buckets dated items into calendar ranges.

    CLI STEPS     aglet view create-datebook "Scheduling" ...

    HOW IT WORKS  A datebook view is a distinct view type that groups items by their
                  When date into date-interval buckets (days, weeks, etc.). It is
                  ideal for upcoming work, appointments, renewals, and deadlines.

    NOTE          A standard view and a datebook view are different types; one
                  cannot be converted into the other.

#### SEE ALSO

> [Datebook views](aglet-manual.md#datebook%20views),   [Browse a datebook view](#browse%20a%20datebook),   [Date conditions](aglet-cli.md#date%20conditions),   [Datebook keys](#tui%20datebook%20keys),   [» Index](#index)

## <span id="browse a datebook">Browse a Datebook View</span>

    PURPOSE       Move the visible date window of a datebook view forward and back.

    TUI STEPS     { and } step to the previous / next bucket. ( and ) step the
                  window. 0 jumps to today.

    CLI STEPS     aglet view datebook-browse  ...

    HOW IT WORKS  Browsing shifts which date interval is shown without changing the
                  view definition.

    NOTE          0 (today) is the quickest way to recenter after browsing.

#### SEE ALSO

> [Create a datebook view](#create%20a%20datebook),   [Datebook views](aglet-manual.md#datebook%20views),   [Datebook keys](#tui%20datebook%20keys),   [» Index](#index)

## <span id="the view editor">The View Editor</span>

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

> [Create a view](#create%20a%20view),   [View criteria](#view%20criteria),   [Add a section](#add%20a%20section),   [View aliases](#view%20aliases),   [View editor keys](#tui%20view%20editor%20keys),   [» Index](#index)

## <span id="view aliases">View Aliases</span>

    PURPOSE       Show a category under a different display name inside one view,
                  without changing the category itself.

    CLI STEPS     aglet view alias set   "Display Name"
                  aglet view alias clear  

    HOW IT WORKS  An alias is per-view display metadata mapping a category to a
                  label. It affects only how the category is shown in that view.

    NOTE          Aliases do not change category identity, filters, section titles,
                  generated subsection labels, or board column headings.

#### SEE ALSO

> [The view editor](#the%20view%20editor),   [Views](aglet-manual.md#views),   [CLI view commands](aglet-cli.md#cli%20view%20commands),   [» Index](#index)

## <span id="clone a view">Clone a View</span>

    PURPOSE       Make an editable copy of an existing view, including the immutable
                  All Items view.

    CLI STEPS     aglet view clone "All Items" "My Items"

    HOW IT WORKS  Cloning copies the source view's criteria, sections, columns, and
                  settings into a new, mutable view that you can then customize.

    NOTE          Cloning is the way to start from All Items, which cannot itself be
                  edited.

#### SEE ALSO

> [Create a view](#create%20a%20view),   [The All Items view](aglet-manual.md#the%20all%20items%20view),   [Discard a view](#discard%20a%20view),   [» Index](#index)

## <span id="discard a view">Discard a View</span>

    PURPOSE       Delete a view you no longer need.

    CLI STEPS     aglet view delete "Old View"
                  aglet view rename "Old Name" "New Name"

    HOW IT WORKS  Deleting a view removes the saved presentation only; the items it
                  showed remain in the database. The All Items view cannot be
                  deleted.

    NOTE          Deleting a view never deletes items - it only discards the lens.

#### SEE ALSO

> [Create a view](#create%20a%20view),   [Clone a view](#clone%20a%20view),   [The All Items view](aglet-manual.md#the%20all%20items%20view),   [» Index](#index)

# Working with Dependencies

## <span id="create a dependency">Create a Dependency Link</span>

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

> [Dependencies](aglet-manual.md#dependencies),   [Remove a link](#remove%20a%20link),   [Filter blocked items](#filter%20blocked%20items),   [The link wizard](#the%20link%20wizard),   [» Index](#index)

## <span id="remove a link">Remove a Link</span>

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

> [Create a dependency](#create%20a%20dependency),   [Dependencies](aglet-manual.md#dependencies),   [Filter blocked items](#filter%20blocked%20items),   [» Index](#index)

## <span id="filter blocked items">Filter Blocked / Not-Blocked Items</span>

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

> [Dependencies](aglet-manual.md#dependencies),   [Create a dependency](#create%20a%20dependency),   [The ready list](aglet-cli.md#the%20ready%20list),   [CLI filtering](aglet-cli.md#cli%20filtering),   [» Index](#index)

## <span id="the link wizard">The Link Wizard</span>

    PURPOSE       Create dependency links interactively in the TUI.

    TUI STEPS     Press b or B on an item or a multi-item selection to open the
                  wizard, then pick the other item and the relationship direction
                  (blocked-by or blocks).

    HOW IT WORKS  The wizard writes the same depends-on / blocks links as the CLI.
                  With a selection, it can link several items at once.

    NOTE          Use the wizard for prerequisites; use claim/release for "who is
                  working on it".

#### SEE ALSO

> [Create a dependency](#create%20a%20dependency),   [Select multiple items](#select%20multiple%20items),   [CLI link commands](aglet-cli.md#cli%20link%20commands),   [» Index](#index)

# Settings and Indicators

## <span id="global settings">Global Settings</span>

    PURPOSE       Adjust application-wide preferences and workflow roles.

    TUI STEPS     Press g s or F10 to open Global Settings.

    HOW IT WORKS  Global Settings control display and behavior preferences such as
                  auto-refresh, section borders, note glyphs, and search mode, and
                  they define workflow roles - which categories act as Ready and as
                  the claim target used by claim/release/ready.

    NOTE          Workflow roles live here, not in the category hierarchy; reorder
                  or rename categories without changing which one is "Ready".

#### SEE ALSO

> [Claim an item](aglet-cli.md#claim%20an%20item),   [The ready list](aglet-cli.md#the%20ready%20list),   [Organize the hierarchy](#organize%20the%20hierarchy),   [» Index](#index)

## <span id="status footer">The Status and Hint Footer</span>

    PURPOSE       Read the two-row footer at the bottom of the TUI.

    HOW IT WORKS  The footer has two rows. The top row shows transient status - the
                  result of your last action or a brief message. The bottom row
                  shows persistent key hints for the current mode, so the available
                  keys change as you move between Normal view, the category manager,
                  the view editor, and the item editor.

    NOTE          Press ? at any time to open the full in-app help panel, which lists
                  every key for the current context.

#### SEE ALSO

> [Normal mode keys](#tui%20normal%20mode%20keys),   [Assignment profile](#assignment%20profile),   [» Index](#index)

## <span id="assignment profile">The Item Assignment Profile</span>

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

> [Automatic assignment](aglet-manual.md#automatic%20assignment),   [Assign a category](#assign%20a%20category),   [Categories](aglet-manual.md#categories),   [CLI item commands](aglet-cli.md#cli%20item%20commands),   [» Index](#index)
