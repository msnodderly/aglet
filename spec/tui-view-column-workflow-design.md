# TUI View + Column Workflow Design (R3.5 Experiments)

Date: 2026-02-17  
Status: Draft design for next implementation slice  
Primary reference: `/Users/mds/src/aglet/reference/lotus-agenda-paper.txt`

## 1. Purpose

Define a concrete workflow and execution plan for Lotus-style "Annotation" columns in Aglet TUI, while keeping near-term changes UI-first and model-safe.

This document complements:

- `/Users/mds/src/aglet-experiments/spec/tui-view-category-workflow.md`

## 2. Design Anchors From Lotus Agenda

The Lotus model splits view behavior into:

- Selection: criteria query
- Demarcation: section headings and order
- Annotation: additional category information shown in columns

For Aglet:

- Selection maps to `criteria` and section criteria.
- Demarcation maps to section lanes.
- Annotation maps to in-lane row columns.

## 3. Locked Baseline (V1, Current Contract)

In this phase, all views use fixed annotation columns:

- `When`
- `Item`
- `All Categories`

Behavior rules:

- `All Items` view uses the same contract.
- `All Categories` contains all assigned categories for the item.
- Values are sorted by category display name.
- Values render as comma-separated text.

This baseline is intentionally implemented without schema changes.

## 4. Target Workflow (V2 Direction)

### 4.1 Column Heading Concept

A view can define one or more annotation column headings.

- A heading typically references a category family root.
- Cell values show assigned categories in that root's subtree.

Examples:

- Heading `Priority` -> values `High`, `Medium`, `Low`
- Heading `People` -> values `Mike`, `Dave`, `Sally`
- Heading `Department` -> values `Sales`, `Marketing`, `Engineering`

### 4.2 Section-Specific Columns

Columns may vary by section.

- View-level default column set applies globally.
- Optional section override replaces or extends the default set.

### 4.3 No Change to Core Model by Default

Near-term experiments should not require immediate model/store schema changes.

- Start with rendering and interaction prototypes.
- Introduce persistence/schema only after interaction value is validated.

## 5. Proposed TUI Authoring Flow

## 5.1 View Editor Entry

- `v` -> select view -> `e` to open view editor.
- Add a "column setup" entry point in view editor (proposed key: `o`).

## 5.2 Column Setup (Proposed)

Column list mode:

- `N`: add column
- `x`: remove column
- `[` / `]`: reorder column
- `Enter`: edit selected column
- `Esc`: back

Column detail mode:

- `t`: edit display title
- `k`: choose kind (`when`, `item`, `all_categories`, `category_family`)
- `f`: choose family category (only for `category_family`)
- `s`: toggle scope (`view_default` or `section_override`)
- `Esc`: back

This is a proposed design surface for experimentation; final bindings can change.

## 6. Rendering Design

## 6.1 Header Row

Each section lane should include a visible header row for column labels.

- Default labels: `When | Item | All Categories`
- Future labels: include category-family headings configured in view/section
- Current board arrangement is section lanes stacked top-to-bottom to maximize
  usable row width for annotation columns.

## 6.2 Width Policy

Initial policy (implemented in T079):

- Use a shared formatter for header and row cells so separators line up.
- `When`: fixed target width (narrow, date-oriented).
- `Item`: flexible dominant width.
- `All Categories`: bounded width with truncation.
- Selection marker (`>`) is a dedicated fixed-width prefix and does not shift
  column boundaries.
- Render rows as compact single lines; avoid wrap-induced spacer lines.

Fallback for narrow terminals:

- retain `Item` and at least one annotation column
- truncate additional columns first

## 6.3 Cell Formatting

- Multiple values in one cell are comma-separated.
- Duplicates are removed before render.
- Sorting is stable and case-insensitive by display name.

## 7. Update-Through-View Semantics (Column-Aware)

Insertion/removal/move rules from section context remain primary.

For future column-interactive behavior:

- Column edits should apply minimal logically consistent assignment changes.
- If multiple assignments satisfy intent, prefer the interpretation with least disruption to surrounding items/sections.

## 8. Model/Persistence Gate

Schema/model changes become justified only when at least one is true:

- users must persist custom column sets per view across sessions
- section-specific column overrides are required as shipped behavior
- column-interactive edits need structured metadata unavailable from existing view fields

If gate is met, first candidate shape:

- view-level `annotation_columns`
- optional section-level `annotation_columns_override`
- column kind enum with `category_family` reference

## 9. Experiment Plan

### E1: Baseline Visibility

- Show explicit column headers for current V1 columns in each lane.
- Add regression tests for row/header formatting.

### E2: Column Authoring UX Stub

- Add column-setup mode in view editor without persistence.
- Validate discoverability and keyboard ergonomics.

### E3: Category-Family Prototype

- Prototype one configurable category-family column using in-memory config.
- Validate usefulness with `Priority`/`People`/`Department` scenarios.

### E4: Persistence Decision

- Evaluate prototype outcomes against model/persistence gate.
- Decide "ship UI-only baseline" vs "introduce model extension".

## 10. Exit Criteria For R3.5

- Clear, testable TUI workflow spec exists for view columns.
- All Items baseline annotation contract is explicit and stable.
- Experimental path and model-change decision gate are documented.
- Follow-up implementation tasks are enumerated in `spec/tasks.md`.
