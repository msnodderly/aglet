# Aglet Lotus Self-Management Demo

*2026-03-21T17:11:00Z by Showboat 0.6.1*
<!-- showboat-id: f33d4c1d-a31f-4dcb-8070-0b49739fd731 -->

This executable walkthrough adapts the Lotus *Applied Self Management Using Lotus Agenda* tutorial to Aglet's current CLI.

It focuses on the supported parts of the tutorial: project planning with tasks, people assignment, saved views with sections and columns, dependency hiding, and a small budgeting appendix with numeric summaries.

It intentionally skips the Lotus datebook flow. Assignment-condition gaps and PDF-to-Aglet friction are documented in [docs/reference/lotus-self-management-gap-analysis.md](/Users/mds/src/aglet/docs/reference/lotus-self-management-gap-analysis.md).

```bash
cd /Users/mds/src/aglet
A=target/debug/agenda-cli
DB=/tmp/aglet-lotus-self-management-demo.ag
cargo build --bin agenda-cli >/dev/null 2>&1
rm -f "$DB"

"$A" --db "$DB" category create Task >/dev/null
"$A" --db "$DB" category create People >/dev/null
"$A" --db "$DB" category create Mira --parent People >/dev/null
"$A" --db "$DB" category create Sam --parent People >/dev/null
"$A" --db "$DB" category create Lee --parent People >/dev/null
"$A" --db "$DB" category create Type >/dev/null
"$A" --db "$DB" category create Income --parent Type >/dev/null
"$A" --db "$DB" category create Expenses --parent Type >/dev/null
"$A" --db "$DB" category create Amount --type numeric >/dev/null

"$A" --db "$DB" view create Tasks --include Task >/dev/null
"$A" --db "$DB" view section add Tasks Tasks --include Task >/dev/null
"$A" --db "$DB" view column add Tasks 0 People --width 12 >/dev/null
"$A" --db "$DB" view column add Tasks 0 When --kind when --width 12 >/dev/null

"$A" --db "$DB" view clone Tasks "Ready Tasks" >/dev/null
"$A" --db "$DB" view edit "Ready Tasks" --hide-dependent-items true --hide-unmatched true >/dev/null

"$A" --db "$DB" view create "Task Assignments" --include Task --hide-unmatched >/dev/null
"$A" --db "$DB" view section add "Task Assignments" People --include People --show-children >/dev/null
"$A" --db "$DB" view column add "Task Assignments" 0 When --kind when --width 12 >/dev/null

"$A" --db "$DB" view create Budget --include Type --hide-unmatched >/dev/null
"$A" --db "$DB" view section add Budget Type --include Type --show-children >/dev/null
"$A" --db "$DB" view column add Budget 0 Amount --width 12 --summary sum >/dev/null
"$A" --db "$DB" view column add Budget 0 When --kind when --width 12 >/dev/null

create_item() {
  "$A" --db "$DB" add "$1" 2>&1 | awk '/^created /{print $2; exit}'
}

TASK1=$(create_item "Draft launch brief")
TASK2=$(create_item "Review copy with stakeholders")
TASK3=$(create_item "Publish launch post")
INC1=$(create_item "Consulting invoice")
EXP1=$(create_item "Venue deposit")

"$A" --db "$DB" category assign "$TASK1" Task >/dev/null
"$A" --db "$DB" category assign "$TASK2" Task >/dev/null
"$A" --db "$DB" category assign "$TASK3" Task >/dev/null
"$A" --db "$DB" category assign "$TASK1" Mira >/dev/null
"$A" --db "$DB" category assign "$TASK2" Sam >/dev/null
"$A" --db "$DB" category assign "$TASK3" Lee >/dev/null
"$A" --db "$DB" edit "$TASK1" --when 2026-03-24 >/dev/null
"$A" --db "$DB" edit "$TASK2" --when 2026-03-26 >/dev/null
"$A" --db "$DB" edit "$TASK3" --when 2026-03-28 >/dev/null
"$A" --db "$DB" link depends-on "$TASK3" "$TASK2" >/dev/null

"$A" --db "$DB" category assign "$INC1" Income >/dev/null
"$A" --db "$DB" category assign "$EXP1" Expenses >/dev/null
"$A" --db "$DB" category set-value "$INC1" Amount 3500 >/dev/null
"$A" --db "$DB" category set-value "$EXP1" Amount -- -1200 >/dev/null
"$A" --db "$DB" edit "$INC1" --when 2026-03-31 >/dev/null
"$A" --db "$DB" edit "$EXP1" --when 2026-03-25 >/dev/null

printf 'Created demo database: %s\n\n' "$DB"
printf 'Categories:\n'
"$A" --db "$DB" category list
printf '\nViews:\n'
"$A" --db "$DB" view list

```

```output
Created demo database: /tmp/aglet-lotus-self-management-demo.ag

Categories:
- Amount [numeric]
- Done [no-implicit-string] [non-actionable]
- Entry [no-implicit-string] [non-actionable]
- People
  - Mira
  - Sam
  - Lee
- Task
- Type
  - Income
  - Expenses
- When [no-implicit-string] [non-actionable]

Views:
All Items (sections=0, and=0, not=0, or=0, hide_dependent_items=false)
Budget (sections=1, and=1, not=0, or=0, hide_dependent_items=false)
Ready Tasks (sections=1, and=1, not=0, or=0, hide_dependent_items=true)
Task Assignments (sections=1, and=1, not=0, or=0, hide_dependent_items=false)
Tasks (sections=1, and=1, not=0, or=0, hide_dependent_items=false)
hint: use `agenda view show "<name>"` to see view contents
```

The first view mirrors the PDF's project-planning task list in a standard Aglet view instead of a Lotus datebook. The second view proves that `hide_dependent_items` can hide blocked work and leave only currently-actionable tasks visible.

```bash
cd /Users/mds/src/aglet
A=target/debug/agenda-cli
DB=/tmp/aglet-lotus-self-management-demo.ag
printf 'Tasks view\n'
"$A" --db "$DB" view show Tasks --format json | jq -r '.sections[] | .title as $title | "SECTION " + $title, (.items[] | "- " + .text + " | when=" + (.when|split(" ")[0]) + " | owner=" + ((.categories - ["People","Task","When"])[0]))'
printf '\nReady Tasks view\n'
"$A" --db "$DB" view show "Ready Tasks" --format json | jq -r '.sections[] | .title as $title | "SECTION " + $title, (.items[]? | "- " + .text)'

```

```output
Tasks view
SECTION Tasks
- Publish launch post | when=2026-03-28 | owner=Lee
- Review copy with stakeholders | when=2026-03-26 | owner=Sam
- Draft launch brief | when=2026-03-24 | owner=Mira

Ready Tasks view
SECTION Tasks
- Review copy with stakeholders
- Draft launch brief
```

The assignment view uses `show_children` on the `People` section so each person gets a subsection. This is the closest current CLI analogue to the tutorial's "Task Assignments" view.

```bash
cd /Users/mds/src/aglet
A=target/debug/agenda-cli
DB=/tmp/aglet-lotus-self-management-demo.ag
"$A" --db "$DB" view show "Task Assignments" --format json | jq -r '.sections[] | .subsections[] | "SECTION " + .title, (.items[]? | "- " + .text + " | when=" + (.when|split(" ")[0]))'

```

```output
SECTION Mira
- Draft launch brief | when=2026-03-24
SECTION Sam
- Review copy with stakeholders | when=2026-03-26
SECTION Lee
- Publish launch post | when=2026-03-28
SECTION People (Other)
```

The budgeting appendix keeps the tutorial's `Income`, `Expenses`, and `Amount` structure, but uses a standard sectioned view with numeric summaries instead of Lotus's quarterly datebook budget.

```bash
cd /Users/mds/src/aglet
A=target/debug/agenda-cli
DB=/tmp/aglet-lotus-self-management-demo.ag
"$A" --db "$DB" view show Budget --format json | jq -r '.sections[] | .subsections[] | "SECTION " + .title, (.items[]? | "- " + .text + " | when=" + (.when|split(" ")[0])), (.summaries[]? | "SUMMARY " + .)'

```

```output
SECTION Income
- Consulting invoice | when=2026-03-31
SUMMARY Amount(sum)=3500
SECTION Expenses
- Venue deposit | when=2026-03-25
SUMMARY Amount(sum)=-1200
SECTION Type (Other)
```

This demo intentionally stops before the PDF's datebook and assignment-condition exercises. Those flows currently require product features that are not exposed in the CLI/TUI yet; see the companion gap analysis for details and mock UX.

```bash
DB=/tmp/aglet-lotus-self-management-demo.ag
rm -f "$DB"
printf 'Removed demo database: %s\n' "$DB"

```

```output
Removed demo database: /tmp/aglet-lotus-self-management-demo.ag
```
