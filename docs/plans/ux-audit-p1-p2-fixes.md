---
title: UX Audit P1/P2 Fixes (+ P3 TUI Polish)
status: draft
created: 2026-06-09
updated: 2026-06-09
---

# UX Audit P1/P2 Fixes (+ P3 TUI Polish)

## Summary

Implementation plan for the Priority 1 (trust & data integrity) and Priority 2
(discoverability & consistency) findings from the 2026-06-09 UX audit
(`reports/ux-audit-2026-06-09.html`), plus a final phase covering the
Priority 3 TUI polish findings. The audit's constraint carries over:
improve usability **without changing core behavior** — every phase here is
presentation, feedback, or confirmation UX except where explicitly flagged as
a behavior decision (Phase 2's note-match scope).

Findings are grouped into phases by shared mechanism, not by audit order, so
one design change retires several symptoms at once. Phases are independently
shippable and ordered by trust payoff per unit of risk.

| Phase | Theme | Audit findings |
| --- | --- | --- |
| 1 | Trust quick wins (feedback at mutation points) | P1-1, P1-2, P1-4, P2-10 |
| 2 | Provenance-forward assignment UX | P1-3, P2-4 |
| 3 | Single keymap source (footer/help/README) | P2-1, P2-2 |
| 4 | Picker & search interaction fixes | P2-3, P2-5, P2-6 |
| 5 | Panel & board layout consistency | P2-7, P2-8, P2-9 |
| 6 | CLI output parity | P2-CLI-1, P2-CLI-2 |
| 7 | P3 TUI polish (formats, category display, link wizard) | P3-1, P3-2, P3-3 |

---

## Phase 1 — Trust Quick Wins

Small diffs, no behavior change, each one closes a "the app did something I
couldn't see" gap.

### 1.1 When-field parse feedback (P1-1)

**Problem.** `save_input_panel_edit` / `save_input_panel_add` route the When
text through `parse_when_datetime_input` (`modes/board.rs:5157-5195`), whose
`BasicDateParser` fallback extracts any recognizable date token and silently
ignores the rest. `2026-06-12 00:00X` or a typo'd `next wek` appended to a
valid date saves with no message; the user cannot tell whether the edit took.

**Change.**
- After a successful save where the When text changed, set status to the
  normalized result: `When saved as 2026-06-12` — and when the parsed
  result was extracted from a longer string (input != canonical rendering of
  the parse), append the interpretation:
  `When saved as 2026-06-12 (interpreted from "2026-06-12 00:00X")`.
- Add a live parse-preview row in the AddItem/EditItem panel under the When
  field, reusing the pane-local feedback pattern the inline `WhenDate` popup
  already has (per AGENTS.md "Inline When Validation Feedback Is Pane-Local").
  Render the parse result (or error) on every When-field edit, not only on
  save.
- Do **not** tighten the parser itself in this phase (rejecting trailing
  garbage changes accepted inputs — defer; the preview makes leniency safe).

**Files.** `crates/aglet-tui/src/modes/board.rs` (save paths ~4853, ~4995;
parse helper ~5157), `crates/aglet-tui/src/input_panel.rs` +
`render/mod.rs::render_input_panel` (preview row).

**Tests.** Unit: normalized-echo status for changed/unchanged/garbage-suffix
When input. Render: panel shows parse preview while When focused; preview
shows error text for unparseable input. Keep
`parse_when_datetime_input_reports_unparsable_input` green.

**Acceptance.** Saving any When edit reports what was actually stored;
garbage-suffixed input is visibly interpreted before the panel closes.

### 1.2 Name auto-assign categories at add time (P1-2)

**Problem.** Add panel help row says `Adding to "TODOs" (auto-assign 1
categories)` — count only, never the names (also a grammar bug). The live DB
had a section configured with `Auto-assign on add: Overdue`; every new item
silently gained Priority "Overdue" and the misconfiguration was only visible
four levels deep in the View Editor.

**Change.**
- Help row lists names: `Adding to "TODOs" — will assign: Overdue` (truncate
  with `+N more` past ~3 names). Fix singular/plural.
- After save, include applied auto-assignments in the status line alongside
  the existing classification message.
- View Editor lint (display-only): in section details, render a `⚠` next to
  `Auto-assign on add` when an auto-assign category is not part of the
  section's criteria — the configuration that produced this finding.

**Files.** `crates/aglet-tui/src/modes/board.rs` (add-context string — keep it
in the fixed help row per AGENTS.md "Add-Item Context Text Must Use Fixed Help
Row"), `crates/aglet-tui/src/modes/view_edit/details.rs` (lint badge).

**Tests.** Render test asserting category names appear in the help row (and
not on the `Text>` line — preserve the existing regression). Lint test:
warning shown when auto-assign ∉ criteria, absent otherwise.

### 1.3 Delete confirmation names the target (P1-4)

**Problem.** `x` shows `Delete item? y:confirm Esc:cancel` in the footer only
— no item title, visually identical to any status line, and multiplexed with
the done-blocker-cleanup prompt in `Mode::ConfirmDelete`.

**Change.**
- Replace the footer-only prompt with a small centered confirm popup:
  title-quoting (`Delete "Wash DRZ"?`), count for batch deletes
  (`Delete 3 items?` + first titles), and a recoverability note
  (`Logged — restorable via 'aglet deleted' / 'restore'`).
- Keep `y` / `Esc` keys and the existing mode; this is render-only plus the
  popup. Update **both** branches of the multiplexed mode (delete and
  done-blocker cleanup) since footer copy for `Mode::ConfirmDelete` is dynamic
  (AGENTS.md note).
- Out of scope (tracked as follow-up FR): TUI deleted-items browser and
  Ctrl-Z coverage for delete.

**Files.** `crates/aglet-tui/src/render/mod.rs` (popup),
`crates/aglet-tui/src/modes/board.rs::handle_confirm_delete_key` (no logic
change expected; verify `done_blocks_confirm` clearing still holds).

**Tests.** Render tests for single, batch, and done-blocker variants; assert
item title present.

### 1.4 Classification provider warning in settings, not per-save (P2-10)

**Problem.** Every save flashes `Classification complete: no new suggestions
(OpenAI: OPENAI_API_KEY not set)`; Global Settings shows the provider as
configured with no hint anything is wrong. The fixable place is silent, the
unfixable place is noisy.

**Change.**
- Global Settings: inline `⚠ OPENAI_API_KEY not set` (or provider-appropriate
  check, e.g. Ollama reachability is out of scope — env-var check only) next
  to the `Semantic provider` row.
- Save-path status: report the missing-key failure once per session, then
  collapse subsequent saves to the classification result without the
  parenthetical.
- Category Manager top status line: hide `Ready queue: (unset)` /
  `Claim result: (unset)` segments when unconfigured (jargon noise for
  non-claim users).

**Files.** `crates/aglet-tui/src/modes/global_settings.rs`,
`crates/aglet-tui/src/async_classify.rs` or the status-formatting call sites
in `modes/board.rs`, `crates/aglet-tui/src/modes/category.rs` (status line).

**Tests.** Settings render test with/without env key (use env override in
test); status suppression after first occurrence; category-manager status
hides unset workflow segments.

---

## Phase 2 — Provenance-Forward Assignment UX

The audit's strongest asset is assignment provenance (`aglet show`
explanations). This phase pushes it to the moment of assignment. **2.2 is the
only item in this plan that changes behavior; it ships behind the existing
classification-mode setting.**

### 2.1 Auto-assignment toast + picker provenance (P1-3 display half, P2-4)

**Problem A (P1-3 display).** Implicit/also-match assignments land silently;
the live DB had a bill filed into `Someday/Maybe` because its note said
"maybe only works during biz hours?". The user only discovers this in views
or `aglet show`.

**Problem B (P2-4).** Assign picker rows read `[-] [x] TODO [exclusive]` —
two adjacent markers (pending delta + current state) needing a legend to
decode, derived assignments indistinguishable from manual ones, and exclusive
radio behavior unsignaled.

**Change.**
- Toast on auto-assignment: when a save/classify pass adds categories, set
  status to `Auto-assigned: Someday/Maybe (matched "maybe" in note)` using the
  existing explanation data (normalize `ImplicitMatch` and
  `AutoClassified{implicit_string}` to one UX string per AGENTS.md). Multiple:
  `Auto-assigned: A, B (+1 more)`.
- Assign picker rendering: one marker per row encoding state + delta —
  `[x] TODO` (assigned), `[x→ ] TODO  will remove`, `[ →x] Home  will add` —
  colored (green add, red remove); drop the two-bracket layout and shrink the
  legend.
- Derived assignments render dimmed with a source suffix:
  `[x] Finance  (via Sheffield Financial)`; toggling one prompts the same
  confirm the inspect-unassign flow uses (no semantic change — manual
  unassign of derived rows already reprocesses, AGENTS.md).
- Exclusive siblings: when a pending add will displace a sibling, show
  `(replaces: Low)` on the row before apply.

**Files.** `crates/aglet-tui/src/state/assign.rs` (row model: state, delta,
source, displaced sibling), `crates/aglet-tui/src/render/mod.rs` (picker
render), `crates/aglet-tui/src/modes/board.rs` (toast at classify/save
completion).

**Tests.** Row-model unit tests (assigned/derived/pending-add/pending-remove/
exclusive-displacement); render snapshots; preserve the "Enter must not
re-toggle dirty selection" regression (AGENTS.md).

### 2.2 Note-text matches go through Suggest/Review (P1-3 behavior half)

**Problem.** Implicit-string/also-match evaluation includes the full note
body. Notes are prose; matches there are far less reliable than title matches
(the audit's live misfire, plus the AGENTS.md `Ready`/`CLI`/`TUI` examples).

**Decision required before implementation** (flagged in the audit as a design
discussion; this plan proposes the conservative option):
- Keep title matches on the current path (Literal: Auto-apply).
- Route matches found **only in the note** through the existing
  `classification_suggestions` queue (Suggest/Review, `C` to review) instead
  of auto-applying — reusing the literal/semantic split that already exists,
  no new modes.
- No retroactive changes: existing assignments keep their provenance and
  stickiness; this only affects new evaluations (consistent with the
  established "mixed compatibility" stance in AGENTS.md).
- Add a per-category escape hatch: `note_match: auto | suggest` (default
  `suggest`), surfaced as a checkbox in Category Manager details and a
  `category update --note-match` CLI flag, for categories where note matching
  is intentional.

**Files.** `crates/aglet-core/src/classification.rs` / `engine.rs` /
`matcher.rs` (split match location: title vs note-only),
`crates/aglet-core/src/model.rs` + `store.rs` (category flag + idempotent
migration), `crates/aglet-tui/src/modes/category.rs` (flag checkbox),
`crates/aglet-cli/src/main.rs` (`category update` flag).

**Tests.** Core: title-only match auto-applies; note-only match creates a
suggestion, not an assignment; flag set to `auto` restores old behavior;
migration roundtrip. Update AGENTS.md "Implicit String Matching Uses Note
Text Too" section when this ships.

**Risk.** Highest in the plan — touches the engine. Ship after Phase 1, alone
in its own PR, with the flag defaulting to `suggest` only after a session of
dogfooding with default `auto` + toast (2.1) to gauge real misfire volume.

---

## Phase 3 — Single Keymap Source

One root cause, four symptoms: contradictory footer rows, clipped/misaligned
help panel, stale README cheat-sheet, missing context hints.

### 3.1 Keymap table per mode (P2-2)

**Problem.** `footer_status_text`, `footer_hint_text`, `render_help_panel`,
and the README each hand-maintain key lists. Observed: view palette status row
says `n new, d datebook`, hint row says `N:new c:clone` (different keys!);
category manager rows disagree on move keys; Normal-mode hints (~20) silently
drop entries as state changes; datebook keys appear in no footer at all.

**Change.**
- Introduce `keymap.rs`: per-mode static tables of
  `(key, action_label, short_hint, help_section, context_predicate)`.
- Footer hint row, footer status-row key mentions, and the help panel all
  render from this table. Hint row takes the top-N entries flagged
  `primary` (curate Normal mode to ≤10 stable entries + `?:help`); help panel
  takes everything grouped by `help_section`.
- Context predicates handle conditional hints (datebook `{`/`}`/`0` when the
  active view is a datebook; redo after undo; `Esc:clear search` with active
  filters) so additions don't evict unrelated hints.
- Add a small generator (test or xtask) that renders the keymap to the README
  cheat-sheet table and fails CI when README drifts (same spirit as the
  existing clap help-coverage test).

**Files.** New `crates/aglet-tui/src/keymap.rs`;
`render/mod.rs::{footer_hint_text, footer_status_text, render_help_panel}`;
`README.md` (generated section markers).

**Tests.** Per-mode: every key handled in the mode's input handler appears in
its keymap (walk the table, not the handler — full handler-coverage is a
stretch goal); footer/help/README all derive from the same rows; preserve the
`p:preview` discoverability regression (AGENTS.md).

**Note.** Mechanical but wide. Land as: (a) introduce table + wire help panel,
(b) wire footers mode-by-mode, (c) README generator. No keybinding changes.

### 3.2 Help panel scroll + layout (P2-1)

**Problem.** `render_help_panel` (`render/mod.rs:4830`) renders a fixed
`Paragraph`; at 50 rows the GLOBAL section (undo, settings, quit) is clipped
with no scroll affordance. Key-column padding (`12_usize` chars) breaks for
wide keys (`v/V/F8 ,/. ga` overflows into "gaViews…"); unicode arrows misalign.

**Change.**
- Scroll state + `j/k`, `PgUp/PgDn`, `Home/End` (same keys as the edit-item
  inspector popup), scrollbar, and a `…more` indicator when clipped.
- Compute the key gutter from the longest key string in the keymap (3.1);
  pad by display width, not `len()`.
- At ≥160 cols render two columns (content comfortably fits side-by-side).

**Files.** `render/mod.rs::render_help_panel`, help-mode key handling in
`modes/board.rs` (`Mode::HelpPanel` currently only closes).

**Tests.** Render at 220×50 and 100×30: last help section reachable by
scrolling; no row where a key string runs into its description.

---

## Phase 4 — Picker & Search Interaction Fixes

### 4.1 Popup-local text inputs (P2-3)

**Problem.** Assign-picker filter input renders in the global footer
(`Category> moto`) while results live in a centered popup — 20+ rows of eye
travel; the typed query is easy to miss. The Link Wizard already renders its
`Search>` box inside the popup; that's the correct pattern.

**Change.** Move `ItemAssignInput` text entry into a bordered input row inside
the Assign Item popup (below the legend, above the panes), with the terminal
cursor positioned there (remember the Category Manager cursor gotcha in
AGENTS.md — set cursor coordinates explicitly). Footer keeps only hints.
Audit other footer-hosted inputs (`InspectUnassign` filter, if any) and
migrate them in the same pass.

**Files.** `render/mod.rs` (assign popup), `modes/board.rs`
(`Mode::ItemAssignInput` no logic change).

**Tests.** Render: query text + cursor inside popup; footer shows hints not
the query.

### 4.2 Filtered category trees keep ancestry (P2-5)

**Problem.** Filtering the category tree (assign picker, category manager)
keeps original indent depths but hides non-matching ancestors — indentation
relative to nothing; ambiguous leaves (`2025`, `2026` exist under multiple
parents).

**Change.** In filtered mode, render matches flat with a dimmed breadcrumb
suffix: `Moto Expenses 2026  — Finance ▸ Expenses`. (Chosen over dimmed
ancestor rows: cheaper, no mixed selectable/unselectable rows.) Applies to
the assign picker, the Add/Edit inline category pane filter, and Category
Manager `/` filter.

**Files.** Shared helper in `crates/aglet-tui/src/ui_support.rs` (breadcrumb
for a category id), call sites in `render/mod.rs` + `modes/category.rs`.

**Tests.** Breadcrumb truncation for deep paths; duplicate-leaf-name
disambiguation visible in snapshot.

### 4.3 Explicit create commit in search/pickers (P2-6)

**Problem.** Enter is overloaded as jump-or-create in section/global search
and select-or-create in the assign-picker input. The precedence rules (exact
match → unique visible match → create) are sensible but invisible; a typo'd
query + Enter mints a new item or category.

**Change (confirmation, not rebinding).**
- When Enter's resolution would be **create**, do not create on the first
  press: show `No match — Enter again to create item "reserach: flights"`
  (or `…create category "moto"`), arm a one-shot confirm, and create on the
  second Enter. Any other key disarms. Single extra keystroke only in the
  create case; jump/select behavior is untouched.
- The search bar placeholder and footer hint name the armed state explicitly.
- Keep the existing match-precedence logic byte-for-byte (AGENTS.md documents
  it; tests cover it) — only the final create step gains a confirm.

**Files.** `modes/board.rs` (search Enter handling, `ItemAssignInput` Enter
handling), small armed-state field on `App`.

**Tests.** Exact match: unchanged single-Enter. No match: first Enter arms
(no item created), second creates; intervening keypress disarms. Global
search session survives the armed state (`g/` session regression).

### 4.4 Global-search Enter reveals instead of editing (P2-6 sub-finding)

**Problem.** Enter on a global-search hit opened the full Edit panel rather
than navigating to the item.

**Change.** Enter selects/reveals the item (jump to its slot, keep the
session's Esc-returns-to-origin contract); `e` edits as everywhere else.
*Check first* whether this is load-bearing for the documented
"creating from global search keeps the session active" flow — if Enter-to-edit
is intentional there, scope this to exact-match jumps only.

**Tests.** `g/` + Enter lands selection on the item in `All Items` without
opening InputPanel; Esc still restores the prior view context.

---

## Phase 5 — Panel & Board Layout Consistency

### 5.1 Converge Edit Item on the inline category checklist (P2-7)

**Problem.** Add Item shows the inline Categories checklist; Edit Item shows
an "Actions" pane (`a` assign, `i` inspect) with categories a hop away. Two
mental models for the same concept.

**Change.** Edit Item gets the same inline Categories pane as Add Item
(checkbox init from full assignment keys already exists per AGENTS.md),
keeping `a` as a shortcut to the full picker and `I` for the inspector popup.
Pending suggestions (currently in the side pane) move below the category list
in the same column. Focus cycle becomes identical to Add Item.

**Files.** `crates/aglet-tui/src/input_panel.rs`,
`render/mod.rs::render_input_panel`, `modes/board.rs` edit-focus handling.

**Tests.** Edit panel shows checked derived+manual categories; toggle + save
produces the same diff the picker would; focus-cycle test updated.

**Risk.** Medium — InputPanel just finished its Phase 5 rework; coordinate
with any in-flight InputPanel work before starting.

### 5.2 Section headers: title first, always (P2-8)

**Problem.** Board section dividers render as
`(4)─────When ─── TODOs ─── Priority ─── Area` — bare count, then column
headers, with the section's identity carried only by the item-column label
(and a label like "Done" collides with done-state reading). Other sections
render `Overdue (1)` title-first. Which text is the section name is guesswork.

**Change.** Divider always leads with `Title (count)` styled distinctly
(bold/accent) before column headers; the item-column label renders in the
column-header style and never substitutes for the title. When a custom
item-label is set, it appears over the item column as a normal header.

**Files.** `render/mod.rs` board header path (`render_board_columns` /
section divider builder).

**Tests.** Snapshot: section with custom item-label shows both title and
label; first-section header contains its title.

### 5.3 Datebook stepping (P2-9, display half)

**Problem.** In a month-bucketed datebook, `}` jumps a full year
(window-length), though README and the help panel say "next period";
datebook keys also missing from footer (fixed by 3.1's context predicates).

**Change.** `{`/`}` step by **one bucket interval**; `Shift-{`/`Shift-}` (or
`(`/`)`) step by the full window; `0` unchanged. Status line reports the new
range either way (existing "Datebook: this year" pattern).

**Out of scope (design discussion first, per audit):** projecting recurrence
occurrences into future buckets as ghost rows — interacts with the recurrence
engine and series-carry semantics; file as a proposal doc instead.

**Files.** `modes/board.rs` datebook key handling; CLI `view datebook-browse`
gains a `--step bucket|window` flag for parity (default `window`, current
behavior preserved).

**Tests.** Month-bucket view: `}` advances one month; shift variant advances
the window; CLI default unchanged.

---

## Phase 6 — CLI Output Parity

### 6.1 Render view columns in `view show` (P2-CLI-1)

**Problem.** `view show` prints column *definitions*
(`columns: When [when,w=12] | Vendor … | Cost`) then a generic
ID/STATUS/WHEN/TITLE table — no column values, no section sums. Finance
views are unusable from the CLI; the columns header is a tease. (Columns live
on `View.sections[*].columns`; `views.columns_json` is legacy — AGENTS.md.)

**Change.**
- Table format: render configured section columns as real table columns
  (numeric formatting via the existing `numeric_format` machinery, same as
  TUI cells); render a totals row per section where summaries are configured.
- JSON format: include per-item column values and per-section summaries.
- Keep the current output when a section defines no columns.

**Files.** `crates/aglet-cli/src/main.rs` (view show rendering), reuse
column-value resolution from `aglet-core` (extract from TUI projection if it
currently lives in `aglet-tui` — move shared logic down to core rather than
duplicating).

**Tests.** Snapshot with numeric column + currency format + Sum summary;
JSON schema test; alias display (`When => Date`) honored in headers; dangling
aliases print `(deleted category)` not the raw UUID (audit P3 freebie while
in the code).

### 6.2 Compact `list` default + honest STATUS (P2-CLI-2)

**Problem.** Three lines per item, full 36-char UUIDs, raw ISO timestamps,
full ancestor/reserved category closure, and a STATUS column that says `open`
while the workflow Status category says "In Progress" (documented gotcha).

**Change.**
- New default row: `8-char id  date  title  leaf-categories` (one line; note
  presence indicated by a glyph). Direct/leaf assignments only — drop
  subsumed parents and reserved plumbing (`When`, `Entry`) from the default
  category list.
- `--verbose` (or `--wide`) restores today's full output **unchanged** so
  existing agent scripts migrate deliberately; `--format json` untouched.
- STATUS column renamed `DONE?` (values `open`/`done`) to stop implying
  workflow status. (Populating it from the Status family is a behavior/
  config question — out of scope, noted as follow-up.)
- Humane date rendering: date-only when time is midnight (shared formatter
  with 6.1; consider lifting into `aglet-core` for TUI reuse later — the
  audit's P3-raw-formats finding rides along free).

**Files.** `crates/aglet-cli/src/main.rs` (list/search rendering — search
gets the same row format).

**Tests.** Snapshot both formats; update existing
`list_search_delete_export` integration tests; assert prefix shown resolves
via existing prefix matching (8 chars unique in test fixtures).

**Migration note.** This changes default CLI output that agents parse. Land
with an AGENTS.md update in the same PR, and grep `docs/` + scripts for
`aglet list` consumers first.

---

## Phase 7 — P3 TUI Polish

Low-stakes presentation cleanups from the audit's Priority 3 section, scoped
to the TUI. (The CLI halves of P3-1/P3-2 already ride along with Phase 6; the
docs-drift finding P3-4 is covered under "Doc Updates Shipped With Code".)

### 7.1 Humane date/time formatting in TUI surfaces (P3-1)

**Problem.** Preview/Info shows `When: 2026-06-13 00:00:00` (midnight noise on
date-only items) and `Created: 2026-05-09T17:53:53.026833Z` (microsecond UTC
timestamps), while the board already renders `2026-06-13` humanely. Machine
formats leak into exactly the pane meant for humans.

**Change.**
- Add a shared formatter in `aglet-core` (e.g. `dates::format_human`):
  date-only when the time component is midnight, otherwise `YYYY-MM-DD HH:MM`;
  created/modified rendered in local time without sub-second precision, with
  an optional relative suffix (`2026-05-09 (31 days ago)`).
- Apply to: preview Summary/Info metadata rows, edit-item inspector popup,
  When-field display value in InputPanel (`2026-06-12` instead of
  `2026-06-12 00:00` — also reduces the Phase 1.1 "interpreted from" noise),
  and the Phase 6 CLI surfaces switch to the same helper so TUI and CLI agree.
- Storage format untouched (`YYYY-MM-DD HH:MM:SS` store contract per
  AGENTS.md); this is render-only.

**Files.** `crates/aglet-core/src/dates.rs` (formatter + unit tests),
`crates/aglet-tui/src/render/mod.rs` (preview/info, inspector),
`crates/aglet-tui/src/input_panel.rs` (When display value).

**Tests.** Formatter units (midnight elision, non-midnight, relative suffix);
render assertions that Info pane contains no `T…Z`/microsecond timestamps;
preview scroll-clamp regression stays green (line count changes).

### 7.2 Leaf-first category display (P3-2)

**Problem.** Category lists in preview Summary, board "All Categories"
columns, and the dynamic-board synthetic column flatten the full closure —
direct assignments, every subsumed ancestor, and reserved plumbing (`When`,
`Entry`) — so a bill item lists 14 categories and the 3 meaningful ones drown.

**Change.**
- Shared helper (likely `aglet-core::query` or `ui_support.rs`):
  `display_categories(item) -> Vec<...>` returning direct/leaf assignments
  only, excluding reserved categories, sorted leaf-first.
- Preview Summary uses it by default and gains a toggle line item: `o` (or
  reuse the existing preview-mode cycle) switches `Categories` between
  `direct` and `all` — full closure stays one keystroke away, with parents
  rendered dimmed and suffixed `(via X)` using provenance where cheap.
- Board "All Categories"/synthetic column uses the same leaf-first list
  (display change only; criteria/filter semantics untouched).
- Phase 6.2's CLI leaf-list rule reuses this helper — one definition of
  "displayable categories" everywhere.

**Files.** Helper in `aglet-core` (+ re-export), `render/mod.rs` (preview,
board cell rendering), coordinate with 6.2 so CLI and TUI call the same code.

**Tests.** Helper units (subsumption-only parents excluded, reserved
excluded, manual parent assignment retained — a parent assigned *manually*
is a direct assignment and must stay); render snapshot for the 14-category
bill case showing the short list; toggle render test.

**Risk note.** "Direct" must be computed from assignment provenance
(non-`Subsumption` rows), not tree position — an item can be manually
assigned a parent category with no child assigned. The existing
`inspect_assignment_rows_for_item` source data already distinguishes this.

### 7.3 Link Wizard header + match-row glyphs (P3-3)

**Problem.** The wizard's top region renders as an empty bordered box (the
source-item slot, unpopulated), and match rows read `open | 2wheels track day`
— status-as-pipe-prefix looks like debug output.

**Change.**
- Header box shows `Source: <item title>` (truncated), or
  `Source: 3 items — "first title", …` for multi-select; remove the box
  entirely if there is genuinely nothing to show in some state (no empty
  bordered rectangles).
- Match rows render done-state as the board's existing glyph vocabulary
  (`✓` done / ` ` open) before the title instead of `open |` / `done |`.
- Keep the stateful-list scrolling behavior (AGENTS.md regression: deep-list
  navigation must stay visible).

**Files.** `render/mod.rs` link-wizard render, `modes/board.rs` only if the
source titles aren't already on the wizard state.

**Tests.** Render: header contains source title (single + multi); no
`open |` prefix in match rows; deep-list scroll regression preserved.

---

## Sequencing & Risk Summary

```
Phase 1 (1.1–1.4)   low risk, ship first, individually or as one PR
Phase 3 (3.1→3.2)   mechanical, unblocks help/footer/README; start in parallel
Phase 4 (4.1–4.4)   small, independent PRs
Phase 2.1           display-only provenance work, any time after Phase 1
Phase 2.2           ★ behavior change; own PR, after 2.1 dogfooding, decision gate
Phase 5 (5.1–5.3)   coordinate 5.1 with InputPanel ownership; 5.2/5.3 anytime
Phase 6 (6.1–6.2)   independent of TUI phases; 6.2 needs AGENTS.md migration note
Phase 7 (7.1–7.3)   pure polish, lowest priority; 7.1/7.2 helpers should land
                    before or with Phase 6 so CLI and TUI share formatters
```

Decision gates needing the developer's sign-off before implementation:
1. **2.2** note-match → suggest default (behavior; conservative path proposed).
2. **4.4** global-search Enter semantics (verify against the `g/` create flow).
3. **6.2** default `list` format change (agent-script compatibility).

Everything else is presentation/feedback and can proceed under the audit's
"no core behavior changes" constraint.

## Test Strategy (cross-cutting)

- Render assertions follow the existing pattern (string-contains on rendered
  buffers + insta snapshots where already used, e.g. CLI export snapshot).
- Every footer/help change must keep the documented regressions: `p:preview`
  hints, add-item context not on the `Text>` line, Enter-not-retoggling in
  the assign picker, view-edit tests not assuming `views[0]` is editable.
- New keymap table gets its own coverage test in the spirit of
  `clap_help_docs_cover_all_commands_and_arguments`.

## Doc Updates Shipped With Code

- AGENTS.md: note-match scope (2.2), CLI list format (6.2), keymap
  single-source (3.1), delete-confirm popup (1.3).
- README cheat-sheet: generated from keymap (3.1).
- `docs/specs/product/gaps.md`: remove the stale "no undo" claim while in
  the area (audit P3 doc-drift finding).
