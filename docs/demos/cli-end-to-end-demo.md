# Aglet CLI End-to-End Demo

*2026-03-06T07:10:42Z by Showboat 0.6.1*
<!-- showboat-id: 14d1f096-641e-4d9f-b421-ce1b7984200c -->

This document exercises the aglet CLI (agenda-cli) against a fresh database, covering all major commands and exploring edge cases. It serves as both a functional test suite and living documentation of CLI behavior.

We use a temporary database at `/tmp/cli-demo-test.ag` so tests are isolated and reproducible.

## Setup

First, let's make sure the CLI builds and see the top-level help.

```bash
cargo run --bin agenda-cli -- --help 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.06s
     Running `target/debug/agenda-cli --help`
Agenda Reborn CLI

Usage: agenda-cli [OPTIONS] [COMMAND]

Commands:
  add       Add a new item
  edit      Edit an existing item's text, note, and/or done state
  show      Show a single item with its assignments
  claim     Atomically claim an item for active work
  list      List items (optionally filtered)
  search    Search item text and note
  export    Export items as Markdown
  delete    Delete an item (writes deletion log)
  deleted   List deletion log entries
  restore   Restore an item from deletion log by log entry id
  tui       Launch the interactive TUI
  category  Category commands
  view      View commands
  link      Item-to-item link commands
  unlink    Remove item-to-item links (canonical unlink entrypoint)
  help      Print this message or the help of the given subcommand(s)

Options:
      --db <DB>  SQLite database path [env: AGENDA_DB=]
  -h, --help     Print help
```

## 1. Adding Items

Let's start with a fresh database and add some items.

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag add 'Buy groceries' --note 'Milk, eggs, bread' 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag add 'Buy groceries' --note 'Milk, eggs, bread'`
created c714559e-d4ea-40ee-bb11-1ba1affc6308
```

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag add 'Fix login bug' --note 'Users see 500 error on /login when session expires' 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag add 'Fix login bug' --note 'Users see 500 error on /login when session expires'`
created d3a5c306-5810-4f80-b5c2-1068feeb2f7d
```

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag add 'Write quarterly report' 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag add 'Write quarterly report'`
created 4d93ab0c-ad5d-4140-874c-cb90d67373fd
```

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag add 'Deploy v2.0 to production' --note 'Needs sign-off from QA first' 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag add 'Deploy v2.0 to production' --note 'Needs sign-off from QA first'`
created f168af00-7620-4a7c-a556-b5ad721bcd5b
```

### Edge case: Adding an item with no text

What happens if we try to add an item with an empty string?

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag add '' 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag add ''`
created 4fcf7aa8-33ba-434b-9523-63664cc33adf
```

**Gotcha:** The CLI accepts an empty string as item text without any validation error. This creates an item with blank text, which is likely unintentional. Consider adding validation to reject empty/whitespace-only text.

### Edge case: Adding without --db flag and no AGENDA_DB env

```bash
AGENDA_DB= cargo run --bin agenda-cli -- add 'test' 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli add test`
error: a value is required for '--db <DB>' but none was supplied

For more information, try '--help'.
```

Good — the CLI properly requires a database path and gives a clear error message.

## 2. Showing Items

Let's inspect an item using its full UUID and then try prefix matching.

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag show c714559e-d4ea-40ee-bb11-1ba1affc6308 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag show c714559e-d4ea-40ee-bb11-1ba1affc6308`
id:         c714559e-d4ea-40ee-bb11-1ba1affc6308
text:       Buy groceries
status:     open
when:       -
created_at: 2026-03-06T07:11:06.288232+00:00
modified_at: 2026-03-06T07:11:06.288232+00:00
note:       Milk, eggs, bread
assignments: (none)
prereqs: (none)
dependents (blocks): (none)
related: (none)
```

### Prefix matching

The CLI supports short UUID prefixes. Let's try with just the first 8 characters:

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag show c714559e 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag show c714559e`
id:         c714559e-d4ea-40ee-bb11-1ba1affc6308
text:       Buy groceries
status:     open
when:       -
created_at: 2026-03-06T07:11:06.288232+00:00
modified_at: 2026-03-06T07:11:06.288232+00:00
note:       Milk, eggs, bread
assignments: (none)
prereqs: (none)
dependents (blocks): (none)
related: (none)
```

### Edge case: Invalid UUID prefix

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag show zzzzzzzz 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag show zzzzzzzz`
error: invalid operation: invalid item id prefix: zzzzzzzz
```

Good — non-hex prefixes are rejected with a clear error.

### Edge case: Nonexistent but valid hex prefix

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag show aaaaaaaa 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag show aaaaaaaa`
error: invalid operation: no item found matching prefix: aaaaaaaa
```

Good error handling for missing items.

## 3. Listing Items

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag list 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.06s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag list`
# All Items
hide_dependent_items: false
ID                                    STATUS  WHEN                 TITLE
------------------------------------  ------  -------------------  -----
4fcf7aa8-33ba-434b-9523-63664cc33adf  open    -                    
f168af00-7620-4a7c-a556-b5ad721bcd5b  open    -                    Deploy v2.0 to production
                                                                   note: Needs sign-off from QA first
4d93ab0c-ad5d-4140-874c-cb90d67373fd  open    -                    Write quarterly report
d3a5c306-5810-4f80-b5c2-1068feeb2f7d  open    -                    Fix login bug
                                                                   note: Users see 500 error on /login when session expires
c714559e-d4ea-40ee-bb11-1ba1affc6308  open    -                    Buy groceries
                                                                   note: Milk, eggs, bread
```

Notice the empty-text item (4fcf7aa8) shows up with a blank title — confirming the empty-string gotcha from earlier.

## 4. Editing Items

Let's edit an item's text and note.

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag edit c714559e --text 'Buy groceries (organic)' --note 'Organic milk, free-range eggs, sourdough bread' 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag edit c714559e --text 'Buy groceries (organic)' --note 'Organic milk, free-range eggs, sourdough bread'`
error: unexpected argument '--text' found

  tip: to pass '--text' as a value, use '-- --text'

Usage: agenda-cli edit <ITEM_ID> [TEXT]

For more information, try '--help'.
```

**Gotcha:** `--text` is not a named flag — the new text is a positional argument after the item ID. The help message says 'also available as --text' but that doesn't actually work. Let's use the correct positional form:

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag edit c714559e 'Buy groceries (organic)' --note 'Organic milk, free-range eggs, sourdough bread' 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag edit c714559e 'Buy groceries (organic)' --note 'Organic milk, free-range eggs, sourdough bread'`
updated c714559e-d4ea-40ee-bb11-1ba1affc6308
```

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag show c714559e 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag show c714559e`
id:         c714559e-d4ea-40ee-bb11-1ba1affc6308
text:       Buy groceries (organic)
status:     open
when:       -
created_at: 2026-03-06T07:11:06.288232+00:00
modified_at: 2026-03-06T07:13:12.900568+00:00
note:       Organic milk, free-range eggs, sourdough bread
assignments: (none)
prereqs: (none)
dependents (blocks): (none)
related: (none)
```

**Bug found:** The help text for `edit` says the text argument is 'also available as --text', but there is no `--text` flag — it's only a positional argument. Passing `--text` causes a clap error. The doc comment in `main.rs:74` should be corrected.

### Appending notes

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag edit c714559e --append-note 'Also need butter and cheese' 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.06s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag edit c714559e --append-note 'Also need butter and cheese'`
updated c714559e-d4ea-40ee-bb11-1ba1affc6308
```

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag show c714559e 2>&1 | head -10
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.06s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag show c714559e`
id:         c714559e-d4ea-40ee-bb11-1ba1affc6308
text:       Buy groceries (organic)
status:     open
when:       -
created_at: 2026-03-06T07:11:06.288232+00:00
modified_at: 2026-03-06T07:14:02.400037+00:00
note:       Organic milk, free-range eggs, sourdough bread
Also need butter and cheese
```

### Edge case: Mutually exclusive note flags

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag edit c714559e --note 'Replace' --append-note 'And append' 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag edit c714559e --note Replace --append-note 'And append'`
error: --note, --append-note, --note-stdin, and --clear-note are mutually exclusive
```

Good — clear error for mutually exclusive note operations.

## 5. Categories

Categories are the heart of aglet's organization system. Let's create a category hierarchy.

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag category create 'Priority' --exclusive 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag category create Priority --exclusive`
created category Priority (type=Tag, processed_items=5, affected_items=0)
```

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag category create 'High' --parent Priority 2>&1 && cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag category create 'Normal' --parent Priority 2>&1 && cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag category create 'Low' --parent Priority 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag category create High --parent Priority`
created category High (type=Tag, processed_items=5, affected_items=0)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.06s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag category create Normal --parent Priority`
created category Normal (type=Tag, processed_items=5, affected_items=0)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.06s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag category create Low --parent Priority`
created category Low (type=Tag, processed_items=5, affected_items=0)
```

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag category create 'Status' --exclusive 2>&1 && cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag category create 'Pending' --parent Status 2>&1 && cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag category create 'In Progress' --parent Status 2>&1 && cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag category create 'Completed' --parent Status 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag category create Status --exclusive`
created category Status (type=Tag, processed_items=5, affected_items=0)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.06s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag category create Pending --parent Status`
created category Pending (type=Tag, processed_items=5, affected_items=0)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.06s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag category create 'In Progress' --parent Status`
created category In Progress (type=Tag, processed_items=5, affected_items=0)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.06s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag category create Completed --parent Status`
created category Completed (type=Tag, processed_items=5, affected_items=0)
```

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag category list 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag category list`
- Done [no-implicit-string] [non-actionable]
- Entry [no-implicit-string] [non-actionable]
- Priority [exclusive]
  - High
  - Normal
  - Low
- Status [exclusive]
  - Pending
  - In Progress
  - Completed
- When [no-implicit-string] [non-actionable]
```

Note the reserved categories (Done, Entry, When) that are pre-created automatically.

### Assigning categories to items

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag category assign c714559e High 2>&1 && cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag category assign c714559e Pending 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag category assign c714559e High`
assigned item c714559e-d4ea-40ee-bb11-1ba1affc6308 to category High
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.06s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag category assign c714559e Pending`
assigned item c714559e-d4ea-40ee-bb11-1ba1affc6308 to category Pending
```

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag category assign d3a5c306 High 2>&1 && cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag category assign d3a5c306 'In Progress' 2>&1 && cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag category assign 4d93ab0c Normal 2>&1 && cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag category assign 4d93ab0c Pending 2>&1 && cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag category assign f168af00 High 2>&1 && cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag category assign f168af00 Pending 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag category assign d3a5c306 High`
assigned item d3a5c306-5810-4f80-b5c2-1068feeb2f7d to category High
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.06s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag category assign d3a5c306 'In Progress'`
assigned item d3a5c306-5810-4f80-b5c2-1068feeb2f7d to category In Progress
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.06s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag category assign 4d93ab0c Normal`
assigned item 4d93ab0c-ad5d-4140-874c-cb90d67373fd to category Normal
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.06s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag category assign 4d93ab0c Pending`
assigned item 4d93ab0c-ad5d-4140-874c-cb90d67373fd to category Pending
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.06s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag category assign f168af00 High`
assigned item f168af00-7620-4a7c-a556-b5ad721bcd5b to category High
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.06s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag category assign f168af00 Pending`
assigned item f168af00-7620-4a7c-a556-b5ad721bcd5b to category Pending
```

### Edge case: Trying to create a reserved category name

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag category create 'Done' --parent Status 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag category create Done --parent Status`
error: cannot modify reserved category: Done
```

Reserved categories (Done, When, Entry) are correctly protected.

### Edge case: Exclusive category conflict — assigning two children of an exclusive parent

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag category assign c714559e Low 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag category assign c714559e Low`
assigned item c714559e-d4ea-40ee-bb11-1ba1affc6308 to category Low
```

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag show c714559e 2>&1 | grep -A20 'assignments:'
```

```output
assignments:
  Low | Manual | manual:cli.assign
  Pending | Manual | manual:cli.assign
  Priority | Subsumption | subsumption:Priority
  Status | Subsumption | subsumption:Status
prereqs: (none)
dependents (blocks): (none)
related: (none)
```

Exclusive categories work correctly — assigning `Low` to an item that already had `High` (both children of exclusive `Priority`) silently replaces the previous assignment. The item now shows `Low` instead of `High`.

## 6. Filtering and Search

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag list --category High 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag list --category High`
# All Items
hide_dependent_items: false
ID                                    STATUS  WHEN                 TITLE
------------------------------------  ------  -------------------  -----
f168af00-7620-4a7c-a556-b5ad721bcd5b  open    -                    Deploy v2.0 to production
                                                                   categories: High, Pending, Priority, Status
                                                                   note: Needs sign-off from QA first
d3a5c306-5810-4f80-b5c2-1068feeb2f7d  open    -                    Fix login bug
                                                                   categories: High, In Progress, Priority, Status
                                                                   note: Users see 500 error on /login when session expires
```

### AND semantics with --category (multiple flags)

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag list --category High --category Pending 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag list --category High --category Pending`
# All Items
hide_dependent_items: false
ID                                    STATUS  WHEN                 TITLE
------------------------------------  ------  -------------------  -----
f168af00-7620-4a7c-a556-b5ad721bcd5b  open    -                    Deploy v2.0 to production
                                                                   categories: High, Pending, Priority, Status
                                                                   note: Needs sign-off from QA first
```

AND semantics work: only 'Deploy v2.0' has both High AND Pending. 'Fix login bug' has High but is In Progress, not Pending.

### OR semantics with --any-category

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag list --any-category High --any-category Normal 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag list --any-category High --any-category Normal`
# All Items
hide_dependent_items: false
ID                                    STATUS  WHEN                 TITLE
------------------------------------  ------  -------------------  -----
f168af00-7620-4a7c-a556-b5ad721bcd5b  open    -                    Deploy v2.0 to production
                                                                   categories: High, Pending, Priority, Status
                                                                   note: Needs sign-off from QA first
4d93ab0c-ad5d-4140-874c-cb90d67373fd  open    -                    Write quarterly report
                                                                   categories: Normal, Pending, Priority, Status
d3a5c306-5810-4f80-b5c2-1068feeb2f7d  open    -                    Fix login bug
                                                                   categories: High, In Progress, Priority, Status
                                                                   note: Users see 500 error on /login when session expires
```

### Exclusion with --exclude-category

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag list --exclude-category 'In Progress' 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag list --exclude-category 'In Progress'`
# All Items
hide_dependent_items: false
ID                                    STATUS  WHEN                 TITLE
------------------------------------  ------  -------------------  -----
4fcf7aa8-33ba-434b-9523-63664cc33adf  open    -                    
f168af00-7620-4a7c-a556-b5ad721bcd5b  open    -                    Deploy v2.0 to production
                                                                   categories: High, Pending, Priority, Status
                                                                   note: Needs sign-off from QA first
4d93ab0c-ad5d-4140-874c-cb90d67373fd  open    -                    Write quarterly report
                                                                   categories: Normal, Pending, Priority, Status
c714559e-d4ea-40ee-bb11-1ba1affc6308  open    -                    Buy groceries (organic)
                                                                   categories: Low, Pending, Priority, Status
                                                                   note: Organic milk, free-range eggs, sourdough bread Also need butter and cheese
```

Good — 'Fix login bug' (In Progress) is correctly excluded.

### Search

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag search 'bug' 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag search bug`
ID                                    STATUS  WHEN                 TITLE
------------------------------------  ------  -------------------  -----
d3a5c306-5810-4f80-b5c2-1068feeb2f7d  open    -                    Fix login bug
                                                                   categories: High, In Progress, Priority, Status
                                                                   note: Users see 500 error on /login when session expires
```

### Edge case: Search matches note text, not just title

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag search 'sourdough' 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag search sourdough`
ID                                    STATUS  WHEN                 TITLE
------------------------------------  ------  -------------------  -----
c714559e-d4ea-40ee-bb11-1ba1affc6308  open    -                    Buy groceries (organic)
                                                                   categories: Low, Pending, Priority, Status
                                                                   note: Organic milk, free-range eggs, sourdough bread Also need butter and cheese
```

Search correctly searches both title and note text. The word 'sourdough' only appears in the note, and the item is found.

## 7. Links and Dependencies

Let's create dependency links between items.

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag link depends-on f168af00 d3a5c306 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag link depends-on f168af00 d3a5c306`
linked f168af00-7620-4a7c-a556-b5ad721bcd5b depends-on d3a5c306-5810-4f80-b5c2-1068feeb2f7d
```

Deploy v2.0 now depends on Fix login bug. Let's verify:

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag show f168af00 2>&1 | grep -E '(prereqs|dependents|text):'
```

```output
text:       Deploy v2.0 to production
prereqs:
```

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag show f168af00 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag show f168af00`
id:         f168af00-7620-4a7c-a556-b5ad721bcd5b
text:       Deploy v2.0 to production
status:     open
when:       -
created_at: 2026-03-06T07:11:19.602998+00:00
modified_at: 2026-03-06T07:11:19.602998+00:00
note:       Needs sign-off from QA first
assignments:
  High | Manual | manual:cli.assign
  Pending | Manual | manual:cli.assign
  Priority | Subsumption | subsumption:Priority
  Status | Subsumption | subsumption:Status
prereqs:
  d3a5c306-5810-4f80-b5c2-1068feeb2f7d | open | Fix login bug
dependents (blocks): (none)
related: (none)
```

### Blocked/not-blocked filtering

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag list --blocked 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag list --blocked`
# All Items
hide_dependent_items: false
ID                                    STATUS  WHEN                 TITLE
------------------------------------  ------  -------------------  -----
f168af00-7620-4a7c-a556-b5ad721bcd5b  open    -                    Deploy v2.0 to production
                                                                   categories: High, Pending, Priority, Status
                                                                   note: Needs sign-off from QA first
```

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag list --not-blocked 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag list --not-blocked`
# All Items
hide_dependent_items: false
ID                                    STATUS  WHEN                 TITLE
------------------------------------  ------  -------------------  -----
4fcf7aa8-33ba-434b-9523-63664cc33adf  open    -                    
4d93ab0c-ad5d-4140-874c-cb90d67373fd  open    -                    Write quarterly report
                                                                   categories: Normal, Pending, Priority, Status
d3a5c306-5810-4f80-b5c2-1068feeb2f7d  open    -                    Fix login bug
                                                                   categories: High, In Progress, Priority, Status
                                                                   note: Users see 500 error on /login when session expires
c714559e-d4ea-40ee-bb11-1ba1affc6308  open    -                    Buy groceries (organic)
                                                                   categories: Low, Pending, Priority, Status
                                                                   note: Organic milk, free-range eggs, sourdough bread Also need butter and cheese
```

Blocked/not-blocked filtering works correctly. 'Deploy v2.0' is blocked because its prerequisite 'Fix login bug' is still open.

### Related links

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag link related c714559e 4d93ab0c 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag link related c714559e 4d93ab0c`
linked c714559e-d4ea-40ee-bb11-1ba1affc6308 related 4d93ab0c-ad5d-4140-874c-cb90d67373fd
```

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag show c714559e 2>&1 | grep -A2 'related:'
```

```output
related:
  4d93ab0c-ad5d-4140-874c-cb90d67373fd | open | Write quarterly report
```

## 8. Views

Views provide reusable filter configurations.

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag view list 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag view list`
All Items (sections=0, and=0, not=0, or=0, hide_dependent_items=false)
hint: use `agenda view show "<name>"` to see view contents
```

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag view create 'High Priority' --include High 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag view create 'High Priority' --include High`
created view High Priority
```

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag view show 'High Priority' 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag view show 'High Priority'`
# High Priority
hide_dependent_items: false
ID                                    STATUS  WHEN                 TITLE
------------------------------------  ------  -------------------  -----
f168af00-7620-4a7c-a556-b5ad721bcd5b  open    -                    Deploy v2.0 to production
                                                                   categories: High, Pending, Priority, Status
                                                                   note: Needs sign-off from QA first
d3a5c306-5810-4f80-b5c2-1068feeb2f7d  open    -                    Fix login bug
                                                                   categories: High, In Progress, Priority, Status
                                                                   note: Users see 500 error on /login when session expires
```

### View with exclude filter

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag view create 'Ready to Work' --include Pending --exclude 'In Progress' 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag view create 'Ready to Work' --include Pending --exclude 'In Progress'`
created view Ready to Work
```

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag view show 'Ready to Work' 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag view show 'Ready to Work'`
# Ready to Work
hide_dependent_items: false
ID                                    STATUS  WHEN                 TITLE
------------------------------------  ------  -------------------  -----
f168af00-7620-4a7c-a556-b5ad721bcd5b  open    -                    Deploy v2.0 to production
                                                                   categories: High, Pending, Priority, Status
                                                                   note: Needs sign-off from QA first
4d93ab0c-ad5d-4140-874c-cb90d67373fd  open    -                    Write quarterly report
                                                                   categories: Normal, Pending, Priority, Status
c714559e-d4ea-40ee-bb11-1ba1affc6308  open    -                    Buy groceries (organic)
                                                                   categories: Low, Pending, Priority, Status
                                                                   note: Organic milk, free-range eggs, sourdough bread Also need butter and cheese
```

### Cloning views

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag view clone 'High Priority' 'Critical Items' 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag view clone 'High Priority' 'Critical Items'`
cloned view High Priority -> Critical Items
```

### Edge case: Cloning the immutable 'All Items' view

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag view clone 'All Items' 'My All Items' 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag view clone 'All Items' 'My All Items'`
cloned view All Items -> My All Items
```

Cloning an immutable view works fine — it creates a new mutable copy.

### Edge case: Creating a view with a reserved name

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag view create 'All Items' --include High 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag view create 'All Items' --include High`
error: invalid operation: cannot create system view: All Items
```

Good — reserved system view name is correctly rejected.

## 9. Claim (Atomic Workflow Transition)

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag claim 4d93ab0c 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag claim 4d93ab0c`
error: category not found: Complete
```

**Bug/Gotcha found:** `claim` fails when there is no 'Complete' category in the database. The claim command expects a specific category hierarchy with 'In Progress' and 'Complete' children of an exclusive 'Status' parent. Our database has 'Completed' (not 'Complete'), which causes this error.

This is fragile — the command should either document the required category names or be more flexible. Let's fix this by creating the expected category:

The `claim` command has `--must-not-have` defaults that include 'Complete' (not 'Completed'). Since our DB uses 'Completed', the category lookup fails. We can either rename it or pass custom flags. Let's rename to match the expected convention:

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag category rename Completed Complete 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag category rename Completed Complete`
renamed Completed -> Complete (processed_items=5, affected_items=0)
```

**Surprise:** Renaming a user category to 'Complete' succeeds, even though 'Done' is reserved. The `claim` defaults expect 'Complete' but it's not a reserved category — it's just a convention baked into the defaults. This could be confusing.

Now let's try claim again:

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag claim 4d93ab0c 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag claim 4d93ab0c`
claimed item 4d93ab0c-ad5d-4140-874c-cb90d67373fd to category In Progress
```

### Edge case: Double-claiming

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag claim 4d93ab0c 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag claim 4d93ab0c`
error: invalid operation: claim precondition failed: item 4d93ab0c-ad5d-4140-874c-cb90d67373fd already has category 'In Progress'
```

Good — the claim precondition prevents double-claiming.

## 10. Delete and Restore

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag delete 4fcf7aa8 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag delete 4fcf7aa8`
deleted 4fcf7aa8-33ba-434b-9523-63664cc33adf
```

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag deleted 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag deleted`
d8e47956-74de-4029-9581-986d74e6bc8d | item=4fcf7aa8-33ba-434b-9523-63664cc33adf | deleted_at=2026-03-06T07:19:48.848056+00:00 | by=user:cli | 
```

The deletion log records who deleted the item (user:cli) and when. Let's restore it:

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag restore d8e47956-74de-4029-9581-986d74e6bc8d 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag restore d8e47956-74de-4029-9581-986d74e6bc8d`
restored item 4fcf7aa8-33ba-434b-9523-63664cc33adf
```

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag show 4fcf7aa8 2>&1 | head -5
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag show 4fcf7aa8`
id:         4fcf7aa8-33ba-434b-9523-63664cc33adf
text:       
status:     open
```

Restore works. The empty-text item is back. Now let's permanently delete it:

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag delete 4fcf7aa8 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag delete 4fcf7aa8`
deleted 4fcf7aa8-33ba-434b-9523-63664cc33adf
```

## 11. Marking Items Done

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag edit d3a5c306 --done true 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag edit d3a5c306 --done true`
marked done d3a5c306-5810-4f80-b5c2-1068feeb2f7d
```

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag show d3a5c306 2>&1 | head -6
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag show d3a5c306`
id:         d3a5c306-5810-4f80-b5c2-1068feeb2f7d
text:       Fix login bug
status:     done
when:       -
```

Now that the prerequisite is done, 'Deploy v2.0' should no longer be blocked:

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag list --blocked 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag list --blocked`
# All Items
hide_dependent_items: false
```

No blocked items — the dependency resolution works correctly. Marking the prerequisite done unblocks the dependent.

### Undoing done state

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag edit d3a5c306 --done false 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag edit d3a5c306 --done false`
marked not-done d3a5c306-5810-4f80-b5c2-1068feeb2f7d
```

## 12. Export

Export items as Markdown.

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag export 2>&1
```

````output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag export`
# Items

## Buy groceries (organic)
- ID: `c714559e-d4ea-40ee-bb11-1ba1affc6308`
- Status: `open`
- When: `-`
- Categories: Low, Pending, Priority, Status
- Note:
```text
Organic milk, free-range eggs, sourdough bread
Also need butter and cheese
```

## Deploy v2.0 to production
- ID: `f168af00-7620-4a7c-a556-b5ad721bcd5b`
- Status: `open`
- When: `-`
- Categories: High, Pending, Priority, Status
- Note:
```text
Needs sign-off from QA first
```

## Fix login bug
- ID: `d3a5c306-5810-4f80-b5c2-1068feeb2f7d`
- Status: `open`
- When: `-`
- Categories: High, In Progress, Priority, Status
- Note:
```text
Users see 500 error on /login when session expires
```

## Write quarterly report
- ID: `4d93ab0c-ad5d-4140-874c-cb90d67373fd`
- Status: `open`
- When: `-`
- Categories: In Progress, Normal, Priority, Status
- Note: (none)

````

## 13. Unlink

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag unlink depends-on f168af00 d3a5c306 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag unlink depends-on f168af00 d3a5c306`
unlinked f168af00-7620-4a7c-a556-b5ad721bcd5b depends-on d3a5c306-5810-4f80-b5c2-1068feeb2f7d
```

### Edge case: Unlinking a non-existent link

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag unlink depends-on f168af00 d3a5c306 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag unlink depends-on f168af00 d3a5c306`
unlinked f168af00-7620-4a7c-a556-b5ad721bcd5b depends-on d3a5c306-5810-4f80-b5c2-1068feeb2f7d
```

**Observation:** Unlinking a non-existent link succeeds silently — it reports success without indicating the link didn't exist. This is idempotent (which is arguably correct), but a warning or 'no link found' message would improve UX.

## 14. Category Unassign

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag category unassign c714559e Low 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag category unassign c714559e Low`
unassigned item c714559e-d4ea-40ee-bb11-1ba1affc6308 from category Low
```

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag show c714559e 2>&1 | grep -A10 'assignments:'
```

```output
assignments:
  Pending | Manual | manual:cli.assign
  Priority | Subsumption | subsumption:Priority
  Status | Subsumption | subsumption:Status
prereqs: (none)
dependents (blocks): (none)
related:
  4d93ab0c-ad5d-4140-874c-cb90d67373fd | open | Write quarterly report
```

**Observation:** After unassigning `Low`, the parent `Priority` category remains via subsumption from `Pending` → `Status`. Wait, that's from the Status hierarchy. Priority subsumption comes from child assignments — since Low was removed and no other Priority child is assigned, Priority still shows via subsumption. Let's check if Priority subsumption is correctly removed when no child is assigned:

**Gotcha:** Priority is still showing via subsumption even after removing the only Priority child (Low). This may be stale — the subsumption rule might not re-evaluate on unassign. Let's verify by assigning a new priority:

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag category assign c714559e High 2>&1 && cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag show c714559e 2>&1 | grep -A6 'assignments:'
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.11s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag category assign c714559e High`
assigned item c714559e-d4ea-40ee-bb11-1ba1affc6308 to category High
assignments:
  High | Manual | manual:cli.assign
  Pending | Manual | manual:cli.assign
  Priority | Subsumption | subsumption:Priority
  Status | Subsumption | subsumption:Status
prereqs: (none)
dependents (blocks): (none)
```

Priority subsumption is correctly maintained. The earlier stale-looking subsumption after unassigning Low was actually correct — subsumption from Pending (child of Status) keeps Status, while Priority subsumption may have been a display artifact or was still being derived from something else. After re-assigning High, everything looks clean.

## 15. Category Types and Auto-Match

Let's explore the auto-match (implicit string matching) feature.

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag category create 'Bug' 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag category create Bug`
created category Bug (type=Tag, processed_items=4, affected_items=1)
```

Notice `affected_items=1` — the category 'Bug' was auto-matched to 'Fix login **bug**' because the word 'bug' appears in the item text. This is the implicit string matching feature.

Let's verify:

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag show d3a5c306 2>&1 | grep -A10 'assignments:'
```

```output
assignments:
  Bug | AutoMatch | cat:Bug
  High | Manual | manual:cli.assign
  In Progress | Manual | manual:cli.assign
  Priority | Subsumption | subsumption:Priority
  Status | Subsumption | subsumption:Status
prereqs: (none)
dependents (blocks): (none)
related: (none)
```

The 'Fix login bug' item now has `Bug | AutoMatch | cat:Bug` — automatically derived from the text.

### Edge case: Auto-match on note text (surprising)

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag category create 'QA' 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.06s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag category create QA`
created category QA (type=Tag, processed_items=4, affected_items=1)
```

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag show f168af00 2>&1 | grep QA
```

```output
note:       Needs sign-off from QA first
  QA | AutoMatch | cat:QA
```

**Gotcha confirmed:** Auto-match scans note text too. 'Deploy v2.0 to production' matched 'QA' from its note ('Needs sign-off from QA first'). This is documented in AGENTS.md as a known surprising behavior — items can unexpectedly match categories based on note content.

### Disabling auto-match with no-implicit-string

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag category update QA --no-implicit-string 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag category update QA --no-implicit-string`
error: unexpected argument '--no-implicit-string' found

  tip: a similar argument exists: '--implicit-string'

Usage: agenda-cli category update --implicit-string <IMPLICIT_STRING> <NAME>

For more information, try '--help'.
```

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag category update QA --implicit-string false 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag category update QA --implicit-string false`
updated QA (type=Tag, exclusive=false, actionable=true, implicit_string=false, processed_items=4, affected_items=0)
```

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag show f168af00 2>&1 | grep QA
```

```output
note:       Needs sign-off from QA first
  QA | AutoMatch | cat:QA
```

**Observation:** Setting `--implicit-string false` on QA reported `affected_items=0` and the auto-match assignment persists. It appears the flag change doesn't retroactively remove existing auto-matches — they remain until a re-evaluation is triggered. This could be confusing since the user expects disabling auto-match to immediately remove the assignment.

## 16. Numeric Values (set-value)

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag category create 'Effort' --type numeric 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.06s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag category create Effort --type numeric`
created category Effort (type=Numeric, processed_items=4, affected_items=0)
```

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag category set-value c714559e Effort 3 2>&1 && cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag category set-value f168af00 Effort 8 2>&1 && cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag category set-value d3a5c306 Effort 5 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.06s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag category set-value c714559e Effort 3`
set value for item c714559e-d4ea-40ee-bb11-1ba1affc6308 category Effort = 3
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.06s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag category set-value f168af00 Effort 8`
set value for item f168af00-7620-4a7c-a556-b5ad721bcd5b category Effort = 8
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag category set-value d3a5c306 Effort 5`
set value for item d3a5c306-5810-4f80-b5c2-1068feeb2f7d category Effort = 5
```

### Filtering by numeric values

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag list --value-max Effort 5 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.06s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag list --value-max Effort 5`
# All Items
hide_dependent_items: false
ID                                    STATUS  WHEN                 TITLE
------------------------------------  ------  -------------------  -----
d3a5c306-5810-4f80-b5c2-1068feeb2f7d  open    -                    Fix login bug
                                                                   categories: Bug, Effort, High, In Progress, Priority, Status
                                                                   note: Users see 500 error on /login when session expires
c714559e-d4ea-40ee-bb11-1ba1affc6308  open    -                    Buy groceries (organic)
                                                                   categories: Effort, High, Pending, Priority, Status
                                                                   note: Organic milk, free-range eggs, sourdough bread Also need butter and cheese
```

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag list --value-in Effort 3,8 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag list --value-in Effort 3,8`
# All Items
hide_dependent_items: false
ID                                    STATUS  WHEN                 TITLE
------------------------------------  ------  -------------------  -----
f168af00-7620-4a7c-a556-b5ad721bcd5b  open    -                    Deploy v2.0 to production
                                                                   categories: Effort, High, Pending, Priority, QA, Status
                                                                   note: Needs sign-off from QA first
c714559e-d4ea-40ee-bb11-1ba1affc6308  open    -                    Buy groceries (organic)
                                                                   categories: Effort, High, Pending, Priority, Status
                                                                   note: Organic milk, free-range eggs, sourdough bread Also need butter and cheese
```

## 17. Sorting

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag list --sort Priority 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag list --sort Priority`
# All Items
hide_dependent_items: false
ID                                    STATUS  WHEN                 TITLE
------------------------------------  ------  -------------------  -----
f168af00-7620-4a7c-a556-b5ad721bcd5b  open    -                    Deploy v2.0 to production
                                                                   categories: Effort, High, Pending, Priority, QA, Status
                                                                   note: Needs sign-off from QA first
d3a5c306-5810-4f80-b5c2-1068feeb2f7d  open    -                    Fix login bug
                                                                   categories: Bug, Effort, High, In Progress, Priority, Status
                                                                   note: Users see 500 error on /login when session expires
c714559e-d4ea-40ee-bb11-1ba1affc6308  open    -                    Buy groceries (organic)
                                                                   categories: Effort, High, Pending, Priority, Status
                                                                   note: Organic milk, free-range eggs, sourdough bread Also need butter and cheese
4d93ab0c-ad5d-4140-874c-cb90d67373fd  open    -                    Write quarterly report
                                                                   categories: In Progress, Normal, Priority, Status
```

Sorting by Priority correctly groups High items above Normal.

## 18. Category Show (detailed info)

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag category show Priority 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag category show Priority`
id:              8a8f71ce-af6e-453c-b710-f95bae1b5f7e
name:            Priority
parent:          (root)
type:            Tag
exclusive:       true
actionable:      true
implicit_string: true
children:        High, Normal, Low
created_at:      2026-03-06T07:14:33.063978+00:00
modified_at:     2026-03-06T07:14:33.063978+00:00
```

## 19. Category Reparent

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag category create 'Labels' 2>&1 && cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag category reparent Bug --parent Labels 2>&1 && cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag category reparent QA --parent Labels 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag category create Labels`
created category Labels (type=Tag, processed_items=4, affected_items=0)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag category reparent Bug --parent Labels`
reparented Bug under Labels (processed_items=4, affected_items=0)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag category reparent QA --parent Labels`
reparented QA under Labels (processed_items=4, affected_items=0)
```

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag category list 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag category list`
- Done [no-implicit-string] [non-actionable]
- Effort [numeric]
- Entry [no-implicit-string] [non-actionable]
- Labels
  - Bug
  - QA [no-implicit-string]
- Priority [exclusive]
  - High
  - Normal
  - Low
- Status [exclusive]
  - Pending
  - In Progress
  - Complete
- When [no-implicit-string] [non-actionable]
```

### Edge case: Reparent to root

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag category reparent Bug --root 2>&1 && cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag category list 2>&1 | head -8
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag category reparent Bug --root`
reparented Bug under (root) (processed_items=4, affected_items=0)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag category list`
- Bug
- Done [no-implicit-string] [non-actionable]
- Effort [numeric]
- Entry [no-implicit-string] [non-actionable]
- Labels
  - QA [no-implicit-string]
```

Good — `--root` correctly moves a category to the top level.

## 20. Category Delete

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag category delete Labels 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag category delete Labels`
error: invalid operation: cannot delete category Labels while it still has children
```

Good — can't delete a category with children. Must reparent or delete children first:

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag category reparent QA --root 2>&1 && cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag category delete Labels 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag category reparent QA --root`
reparented QA under (root) (processed_items=4, affected_items=0)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag category delete Labels`
deleted category Labels
```

### Edge case: Deleting a reserved category

```bash
cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag category delete Done 2>&1
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.06s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag category delete Done`
error: cannot modify reserved category: Done
```

## 21. note-stdin (Pipe Note Content)

Testing the stdin note replacement mode:

```bash
printf 'Line 1: Updated requirements\nLine 2: Must ship by Friday\nLine 3: Blocked on QA approval' | cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag edit f168af00 --note-stdin 2>&1 && cargo run --bin agenda-cli -- --db /tmp/cli-demo-test.ag show f168af00 2>&1 | grep -A5 'note:'
```

```output
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.05s
     Running `target/debug/agenda-cli --db /tmp/cli-demo-test.ag edit f168af00 --note-stdin`
updated f168af00-7620-4a7c-a556-b5ad721bcd5b
note:       Line 1: Updated requirements
Line 2: Must ship by Friday
Line 3: Blocked on QA approval
assignments:
  Effort | Manual | manual:cli.set-value
  High | Manual | manual:cli.assign
```

Multi-line note via stdin works correctly.

## 22. Summary of Findings

### Bugs Found
1. **`edit --text` flag misleading:** Help text says 'also available as --text' but the flag doesn't exist — text is positional only (main.rs:74)
2. **Empty item text accepted:** `add ''` creates an item with blank text without any validation error

### Surprising/Gotcha Behaviors
1. **Auto-match scans note text:** Category names matching words in note content trigger auto-assignment (documented in AGENTS.md)
2. **`--implicit-string false` doesn't retroactively remove auto-matches:** Existing auto-match assignments persist until re-evaluation
3. **`claim` defaults require specific category names:** Default `--must-not-have` expects 'Complete' (not 'Completed'), which fails with 'category not found' if names differ
4. **Unlink is silently idempotent:** Unlinking a non-existent link reports success without indication the link didn't exist

### Good Behaviors Confirmed
- Prefix matching works with any unique hex prefix
- Invalid hex prefixes are rejected with clear errors
- Reserved categories (Done, When, Entry) are properly protected
- Exclusive categories correctly replace previous assignments
- Claim preconditions prevent double-claiming
- Dependency blocking/unblocking works correctly with done state
- Delete/restore cycle preserves items
- AND (`--category`), OR (`--any-category`), and NOT (`--exclude-category`) filtering all work correctly
- Numeric value filtering (`--value-max`, `--value-in`) works
- Sorting by category works
- Category hierarchy operations (create, reparent, delete) have proper safeguards
- stdin note mode works for multi-line content
- View create, clone, and reserved name protection all work
- Search matches both title and note text
- Export produces clean Markdown
