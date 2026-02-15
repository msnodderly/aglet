# Tasks: Agenda Reborn MVP

**Input**: `mvp-spec.md`
**Stack**: Rust, ratatui, rusqlite, clap
**Layout**: Cargo workspace with `crates/agenda-core`, `crates/agenda-tui`, `crates/agenda-cli`

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to

---

## Phase 1: Setup

**Purpose**: Workspace, dependencies, project skeleton

- [ ] T001 Create Cargo workspace with three crates: `agenda-core`, `agenda-tui`, `agenda-cli`
- [ ] T002 [P] Configure `agenda-core/Cargo.toml` — rusqlite (bundled, WAL), uuid, chrono, serde, serde_json
- [ ] T003 [P] Configure `agenda-tui/Cargo.toml` — ratatui, crossterm, agenda-core dependency
- [ ] T004 [P] Configure `agenda-cli/Cargo.toml` — clap (derive), agenda-core dependency
- [ ] T005 [P] Add rustfmt.toml and clippy configuration
- [ ] T006 [P] Create `crates/agenda-core/src/lib.rs` with module stubs: model, store, engine, matcher, dates, query, undo

---

## Phase 2: Foundation — Data Model & Storage (BLOCKS ALL)

**Purpose**: Core types and SQLite persistence. Everything depends on this.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete.

- [ ] T007 [US0] Define core types in `agenda-core/src/model.rs`: Item, Category, Assignment, AssignmentSource, View, Section, Column, Query, WhenBucket, Condition, Action, DeletionLogEntry
- [ ] T008 [US0] Define error types in `agenda-core/src/error.rs`: AgendaError enum (NotFound, DuplicateName, ReservedName, InvalidOperation, StorageError)
- [ ] T009 [US0] Implement SQLite schema in `agenda-core/src/store.rs`: items table, categories table, assignments table, views table (with sections/columns as JSON), deletion_log table. WAL mode on open.
- [ ] T010 [US0] Implement first-launch initialization in store: create reserved categories (When, Entry, Done), create default "All Items" view with When column
- [ ] T011 [US0] Implement Item CRUD in `agenda-core/src/store.rs`: create_item, get_item, update_item, delete_item (writes to deletion_log first), list_items
- [ ] T012 [US0] Implement Category CRUD in `agenda-core/src/store.rs`: create_category (enforce unique name, reject reserved names), get_category, update_category (touch modified_at), delete_category, get_hierarchy (full tree)
- [ ] T013 [US0] Implement Assignment persistence in store: assign_item (with source + origin), unassign_item, get_assignments_for_item
- [ ] T014 [US0] Implement View CRUD in store: create_view, get_view, update_view, list_views, delete_view

**Checkpoint**: All data can be persisted and retrieved. Reserved categories exist on first launch.

---

## Phase 3: US1 — Classification & Rule Engine (P1) 🎯 MVP

**Goal**: Create a category → existing items auto-assign. The "aha moment."

**Independent Test**: Create items via store, create a category, run engine, verify assignments appear.

- [ ] T015 [US1] Define Classifier trait in `agenda-core/src/matcher.rs` with `fn classify(&self, text: &str, category_name: &str) -> Option<f32>`
- [ ] T016 [US1] Implement SubstringClassifier in `agenda-core/src/matcher.rs`: case-insensitive word-boundary match, returns Some(1.0) or None. Respects `enable_implicit_string` flag.
- [ ] T017 [US1] Implement rule engine in `agenda-core/src/engine.rs`: process_item() — depth-first walk of category hierarchy, evaluate conditions (String + Profile), fire actions (Assign + Remove)
- [ ] T018 [US1] Implement fixed-point loop in engine: re-queue on action side-effects, max 10 passes, cycle detection via (ItemId, CategoryId) set, deferred RemoveAction
- [ ] T019 [US1] Implement subsumption in engine: when assigning to child, create implicit assignments for all ancestors (source: Subsumption)
- [ ] T020 [US1] Implement mutual exclusion in engine: when assigning to child of exclusive parent, unassign from sibling children
- [ ] T021 [US1] Implement retroactive assignment in engine: `evaluate_all_items(category_id)` — runs classifier against all items for a newly created/modified category
- [ ] T022 [US1] Wire engine into store operations: item create/update triggers process_item, category create triggers evaluate_all_items

**Checkpoint**: Creating a category named "Sarah" auto-assigns all items containing "Sarah." Profile conditions cascade. Exclusive categories enforce single-child.

---

## Phase 4: US2 — Query Evaluator & Views (P1) 🎯 MVP

**Goal**: Define a view with criteria and sections → see the right items in the right groups.

**Independent Test**: Create items with assignments, create a view with sections, evaluate query, verify correct grouping.

- [ ] T023 [US2] Implement WhenBucket resolution in `agenda-core/src/query.rs`: given a NaiveDateTime and current date/timezone, return which bucket (Overdue, Today, Tomorrow, ThisWeek, NextWeek, ThisMonth, Future, NoDate)
- [ ] T024 [US2] Implement Query evaluator in `agenda-core/src/query.rs`: `evaluate_query(query, items) -> Vec<Item>` — apply include/exclude (category assignments), virtual_include/virtual_exclude (When buckets), text_search (item text + note)
- [ ] T025 [US2] Implement View resolver in `agenda-core/src/query.rs`: `resolve_view(view) -> ViewResult` — evaluate view criteria, then group results by section criteria. Handle show_unmatched: items in view but no explicit section go to unmatched group.
- [ ] T026 [US2] Implement show_children expansion: when section criteria is single category include, generate subsections for each direct child. One level. Category child order.
- [ ] T027 [US2] Implement edit-through logic in `agenda-core/src/engine.rs` or `query.rs`: insert_in_section(item, section, view) → assigns on_insert_assign + view.criteria.include. remove_from_section → unassigns on_remove_unassign. remove_from_view → unassigns remove_from_view_unassign.

**Checkpoint**: Views correctly filter and section items. Edit-through insert/move/remove changes assignments as side effects. When buckets resolve dynamically.

---

## Phase 5: US3 — Date Parsing (P2)

**Goal**: Type "Call Sarah next Friday at 3pm" → when_date auto-populates.

**Independent Test**: Feed strings to parser, verify correct NaiveDateTime output.

- [ ] T028 [US3] Define DateParser trait in `agenda-core/src/dates.rs` with `fn parse(&self, text: &str, reference_date: NaiveDate) -> Option<ParsedDate>`
- [ ] T029 [US3] Implement BasicDateParser in `agenda-core/src/dates.rs`: absolute dates (May 25 2026, 2026-05-25, 12/5/26, December 5)
- [ ] T030 [US3] Extend BasicDateParser: relative dates (today, tomorrow, yesterday, next Tuesday, this Friday)
- [ ] T031 [US3] Extend BasicDateParser: time expressions (at 3pm, at 15:00, at noon) and compound (next Tuesday at 3pm)
- [ ] T032 [US3] Wire date parser into engine: on item create/update, run parser on text, populate when_date if found. Set origin to "nlp:date".

**Checkpoint**: Items with date expressions get when_date populated automatically. Items appear in correct When buckets.

---

## Phase 6: US4 — CLI (P2)

**Goal**: `agenda add "Call Sarah Friday"` works from any terminal. Quick capture without launching TUI.

**Independent Test**: Run CLI commands, inspect SQLite database for correct state.

- [ ] T033 [US4] Implement CLI skeleton in `agenda-cli/src/main.rs` with clap: subcommands add, list, search, done, deleted. Global --db flag and AGENDA_DB env var.
- [ ] T034 [US4] Implement `agenda add "text"` — create item, run date parser, run engine, print created item with assignments
- [ ] T035 [US4] Implement `agenda list` — default: all items. Flags: --view "name", --category "name". Tabular output.
- [ ] T036 [P] [US4] Implement `agenda search "query"` — text search across items + notes
- [ ] T037 [P] [US4] Implement `agenda done <item-id>` — assign to Done, fire actions
- [ ] T038 [P] [US4] Implement `agenda deleted` — list deletion log entries

**Checkpoint**: Full create → classify → query cycle works from the command line. User can capture items without the TUI.

---

## Phase 7: US5 — TUI Core (P1) 🎯 MVP

**Goal**: See views, navigate items, switch between views. Read-only first.

**Independent Test**: Launch TUI with pre-populated database, navigate views and items.

- [ ] T039 [US5] Implement app state and event loop in `agenda-tui/src/app.rs`: startup (open store, load default view), tick loop, crossterm event handling, clean shutdown
- [ ] T040 [US5] Implement main grid widget in `agenda-tui/src/views/grid.rs`: render sections as collapsible groups, items as rows, columns (When date, category assignments), note marker (+), selection cursor
- [ ] T041 [US5] Implement keyboard navigation in grid: arrow keys (up/down through items, across sections), Tab (between columns), section collapse/expand
- [ ] T042 [US5] Implement view switcher popup in `agenda-tui/src/views/picker.rs`: F8 opens list of views, arrow + Enter to select, typing filters list
- [ ] T043 [US5] Implement status bar / header in `agenda-tui/src/views/grid.rs`: view name, key hint bar (F8, F9, Ctrl-Z etc)

**Checkpoint**: TUI launches, displays views with sections and items, user can navigate and switch views. Read-only.

---

## Phase 8: US6 — TUI Edit-Through & Item Entry (P1) 🎯 MVP

**Goal**: Type items, move them between sections, remove from view. Assignments change as side effects.

**Independent Test**: Create items in sections, move between sections, verify assignments change in database.

- [ ] T044 [US6] Implement input bar in `agenda-tui/src/views/input.rs`: bottom bar, context indicator ([→ Section] or [no section]), text entry, Enter to submit
- [ ] T045 [US6] Wire input bar to engine: on submit, create item, run date parser + engine, apply edit-through (on_insert_assign + view criteria include based on current section context), refresh grid
- [ ] T046 [US6] Implement item move between sections: keybinding to move selected item to next/prev section. Calls remove_from_section + insert_in_section in engine. Grid refreshes.
- [ ] T047 [US6] Implement remove from view: `r` key, calls remove_from_view (unassigns remove_from_view_unassign). Item disappears from current view but exists in database.
- [ ] T048 [US6] Implement delete item: `x` key, confirmation prompt, calls delete_item (writes deletion_log), item removed from database and all views
- [ ] T049 [US6] Implement mark done: `d` key, assigns to Done category, fires Done actions, refreshes grid

**Checkpoint**: Full edit-through loop works. User types items into sections, moves them around, marks done. Never explicitly calls "assign." This is the Agenda experience.

---

## Phase 9: US7 — TUI Category Manager & Editing (P2)

**Goal**: Create and manage categories from within the TUI. Edit item text and notes.

- [ ] T050 [US7] Implement category manager in `agenda-tui/src/views/catmgr.rs`: F9 opens tree view of hierarchy, shows item count per category
- [ ] T051 [US7] Implement category operations in catmgr: create (at root or as child), rename, reparent (move in tree), delete (with prompt if non-empty), toggle is_exclusive, toggle enable_implicit_string
- [ ] T052 [US7] Implement condition/action editing in catmgr: add/remove ProfileCondition (select categories for criteria), add/remove AssignAction and RemoveAction (select target categories)
- [ ] T053 [US7] Wire category creation to retroactive assignment: on create, run evaluate_all_items async, show progress indicator in status bar
- [ ] T054 [US7] Implement inline item text editing: Enter on selected item → edit text in-place. On confirm, update item, re-run engine.
- [ ] T055 [US7] Implement $EDITOR integration: Ctrl+G → suspend TUI, open temp file (text\n---\nnote format) in $EDITOR, on return parse changes, update item, re-run engine
- [ ] T056 [US7] Implement note editor: `n` key → open/edit note for selected item (either inline overlay or $EDITOR)
- [ ] T057 [US7] Implement quick category assign: `a` key → popup list of categories, select to assign/unassign on current item

**Checkpoint**: Full category management without leaving TUI. Items editable inline and in $EDITOR. Notes supported.

---

## Phase 10: US8 — Inspect, Search, Undo (P3)

**Goal**: Transparency (why is this item here?), findability, and safety net.

- [ ] T058 [US8] Implement inspect panel in `agenda-tui/src/views/inspect.rs`: `i` key on selected item → shows all assignments with provenance (source, origin, assigned_at). Allow unassign from this view.
- [ ] T059 [US8] Implement view filter: `/` key → incremental text filter over current view (matches item text + note). Esc clears filter.
- [ ] T060 [US8] Implement undo stack in `agenda-core/src/undo.rs`: UndoStack with depth 1. Records inverse operation for: item create/delete, text edit, assign/unassign, section move.
- [ ] T061 [US8] Wire undo to TUI: Ctrl-Z pops undo stack, applies inverse operation, refreshes grid. Status bar shows "Undid: [description]" briefly.

**Checkpoint**: User can understand assignment provenance, filter views, and recover from mistakes.

---

## Phase 11: Hardening & Polish

**Purpose**: Edge cases, resilience, performance

- [ ] T062 [P] Verify deletion log is written on every delete path (user delete, action delete)
- [ ] T063 [P] Verify exclusive category enforcement across all assignment paths (manual, auto, action)
- [ ] T064 [P] Verify subsumption across all assignment paths
- [ ] T065 [P] Verify cycle detection in rule engine with adversarial condition/action setups
- [ ] T066 [P] Verify crash recovery: kill process mid-operation, relaunch, check database consistency (WAL)
- [ ] T067 Handle empty states: no items, no views, no categories (beyond reserved), no sections
- [ ] T068 Retroactive assignment progress indicator: async processing with item count in status bar
- [ ] T069 [P] Performance check: view switching with 1000+ items, rule engine with 100+ categories

---

## Dependencies & Execution Order

### Phase Dependencies

```
Phase 1: Setup ─────────────────────────────┐
Phase 2: Foundation (Data Model + Storage) ──┤ BLOCKS ALL
                                             ▼
         ┌───────────────────────────────────┤
         │                                   │
Phase 3: US1 Classification & Engine         │
         │                                   │
Phase 4: US2 Query & Views ─────────────────┤
         │                                   │
Phase 5: US3 Date Parsing (parallel w/ US2) ─┤
         │                                   │
Phase 6: US4 CLI (after US1 + US3) ──────────┤
         │                                   │
Phase 7: US5 TUI Core (after US2) ───────────┤
         │                                   │
Phase 8: US6 TUI Edit-Through (after US5) ───┤
         │                                   │
Phase 9: US7 TUI Cat Manager (after US6) ────┤
         │                                   │
Phase 10: US8 Inspect/Undo (after US6) ──────┤
         │                                   │
Phase 11: Hardening (after all) ─────────────┘
```

### Critical Path

**Setup → Foundation → US1 (Engine) → US2 (Views) → US5 (TUI Grid) → US6 (Edit-Through)**

This is the shortest path to the core Agenda experience. Everything else adds value but this chain delivers the "aha moment."

### Parallel Opportunities

- **US3 (Date Parsing)** can run in parallel with US2 (Views) — different files, no dependency
- **US4 (CLI)** can start after US1 + US3 complete, parallel with TUI work
- **US7 (Cat Manager)** and **US8 (Inspect/Undo)** can run in parallel — different TUI modules
- Within Phase 2, T011–T014 can run in parallel after T007–T010 (schema first, then CRUD)

### MVP Stopping Points

1. **After Phase 4 (US2)**: Engine + Views work. CLI works. Full data model validated. No UI yet but the core is solid.
2. **After Phase 8 (US6)**: TUI with edit-through. The Agenda experience works end-to-end. Ship it.
3. **After Phase 10 (US8)**: Polish. Inspect, search, undo. Complete MVP.

---

## Notes

- Commit after each task or logical group
- The engine (US1) is the heart — get it right, test it thoroughly
- TUI work should feel fast because the core is already solid
- $EDITOR integration (T055) avoids building a complex in-TUI text editor
- Undo (T060-T061) is low priority — implement last, skip if time-constrained
