# Lotus Agenda Views — Feature Behavior Summary

Extracted from external Lotus Agenda documentation (not included in repo) for potential implementation in Aglet.

Date: 2026-02-17

---

## 1. Core Concept

A **view** is a named, user-configured perspective on the items in a file. Each view is composed of **sections**, and each section is composed of **columns**. A single file can contain many views. The **view manager** lists all views and lets the user switch between them, add, copy, rename, reorder, and delete views.

Key insight: views never own data — they are lenses. Items exist once in the file; views just decide which items to show, how to group them, and what metadata columns to display.

---

## 2. View Types

| Type | Purpose | Sections defined by |
|------|---------|---------------------|
| **Standard** | General-purpose grouping by any category | User-chosen categories as section heads |
| **Datebook** | Calendar/scheduling display by date ranges | Auto-generated from a date category + period/interval settings |
| **Show** (special) | Temporary query result views (match, prereqs, depends, done, alarms, schedule, every item) | System-generated; read-only; only one at a time |

Standard and Datebook types cannot be converted to each other.

---

## 3. View Properties / Settings

When creating or editing a view, the following settings apply:

| Setting | Behavior |
|---------|----------|
| **View name** | 1–37 character unique name |
| **Type** | Standard or Datebook (set at creation, immutable) |
| **Section(s)** | One or more categories to use as section heads |
| **Section sorting** | None / Category order / Alphabetic / Numeric, with ascending/descending direction |
| **Item sorting** | Per-view default (overridable per-section): alphabetic, numeric, date, or category order |
| **Hide empty sections** | Yes/No — suppress sections with no matching items |
| **Hide done items** | Yes/No — filter out items marked done |
| **Hide dependent items** | Yes/No — filter out items that depend on other items |
| **Hide inherited items** | Yes/No — filter out items inherited via category hierarchy |
| **Hide column heads** | Yes/No — show column headers only in first section |
| **Section separators** | Yes/No — display divider lines between sections |
| **Number items** | Yes/No — sequential numbering within each section |
| **View protection** | No protection / Append only / Full protection / Global default |
| **Filter** | Optional category-based filter (see §5 below) |

### Datebook-specific settings

| Setting | Behavior |
|---------|----------|
| **Date category** | Which date category organizes the view (default: When) |
| **End category** | Optional date category for event duration |
| **Period** | Day / Week / Month / Quarter — the time span covered |
| **Interval** | Granularity of sections: 15 min / 30 min / Hourly / Daily / Weekly / Monthly (valid combinations depend on Period) |
| **Start/End at** | Day or time boundaries for the datebook |
| **Base date on** | Absolute or relative date anchor (e.g., "today", "this week", "11/05/90") |

---

## 4. Sections

A section is a category-headed group of items within a view.

- The **section head** is a category. Items assigned to that category appear in the section.
- Items can appear in multiple sections if assigned to multiple section-head categories.
- Items assigned to **child categories** are **inherited** by parent sections (can be hidden via `Hide inherited items`).
- Each section has its own column layout (can differ from other sections in the same view).
- Sections can be added, removed, moved, and individually configured.
- Per-section **filter** can be applied independently of the view-level filter.
- Per-section **item sorting** can override the view default.
- Section statistics show item count.

---

## 5. Filters

Filters are category-based criteria that control which items appear. They can be attached at view level or section level. Items must pass **both** view and section filters to display.

### Filter types

| Filter type | Criteria |
|-------------|----------|
| **Standard** | Show items assigned / not assigned to a specific category |
| **Date** | Show items assigned to a date category, optionally within a date range (inside/outside range) |
| **Numeric** | Show items assigned to a numeric category, optionally within a numeric range (inside/outside range) |

### Compound filters

Multiple filter categories can be combined — items must pass ALL filter criteria (AND logic).

### Filter display

Active filters display in brackets next to the section head or view name.

---

## 6. Columns

Each section is composed of columns. Every section has a mandatory **item column** (headed by the section-head category). Additional columns can be added.

### Column types

| Type | Column head | Column entries |
|------|-------------|----------------|
| **Item** | Section head category | Items (text) — always present, cannot be removed |
| **Standard** | Any standard category | Child categories of that category |
| **Numeric** | Numeric category | Numbers — supports aggregate functions (count, total, average, min, max) |
| **Date** | Date category | Dates and/or times — configurable display format |
| **Unindexed** | Unindexed category | Free-text values (not categories themselves) |
| **Category Note** | A category | Content from category notes (specific line number) |

Columns can be added, removed, moved, and have adjustable width. Column layout can differ per section.

---

## 7. View Manager

The view manager is the hub for view operations:

| Action | Description |
|--------|-------------|
| Switch to view | Select and open a view |
| Add view | Create a new view (INS) |
| Edit view name | Rename a view (F2) |
| Delete view | Remove a view from the file |
| View properties | Display/edit view settings |
| Copy view | Duplicate a view |
| Sort view names | Alphabetize the view list |
| Reposition view | Move a view in the list order |

Views display in the manager in the order they were added, unless sorted alphabetically.

---

## 8. Browse (Date Navigation)

In datebook views or standard views/sections with date filters, **Browse** lets users shift the date window forward or backward:

| In standard views | Shift by |
|---|---|
| Arrow keys | ±1 day |
| Ctrl+Arrow | ±1 week |
| PgDn/PgUp | ±1 month |
| Ctrl+PgDn/PgUp | ±1 year |

| In datebook views | Shift by |
|---|---|
| Arrow keys | ±1 period |

Browse changes can be accepted (ENTER) or cancelled (ESC) to restore original dates.

---

## 9. Show Views (Query Views)

Eight types of system-generated, read-only query views:

1. **Match** — items containing a word/phrase
2. **Prereqs (one level)** — immediate prerequisites of current item
3. **Prereqs (all levels)** — full prerequisite chain
4. **Depends (one level)** — immediate dependents of current item
5. **Depends (all levels)** — full dependent chain
6. **Items Done** — all items marked done
7. **Circular** — items caught in circular dependency references
8. **Alarm** — items with pending/triggered alarms
9. **Sched** — items assigned to a particular date
10. **Every** — all items in the file

Only one show view can exist at a time. Creating a new one replaces the existing one.

---

## 10. Conditions & Actions (Relevant to View Behavior)

Though not view-specific, these directly affect what appears in views:

- **Conditions** on categories create automatic (conditional) assignments: items gain/lose category membership automatically based on text content, other assignments, numeric ranges, or date ranges.
- **Actions** on categories trigger explicit assignments when items are assigned to a category.
- Both mechanisms cause items to dynamically appear/disappear in sections and pass/fail filters without manual intervention.

---

## 11. Gap Analysis vs. Current Aglet Implementation

Based on implementation snapshots captured in the active product docs, the following view behaviors are already implemented or partially implemented:

| Feature | Status |
|---------|--------|
| View CRUD (create/show/list/delete) | ✅ CLI + TUI |
| Sections with category heads | ✅ Core + TUI |
| Unmatched/generated sections | ✅ Core |
| `show_children` expansion | ✅ Core |
| View palette + switching | ✅ TUI |
| View editing (criteria/sections/unmatched) | ✅ TUI |
| Query include/exclude category logic | ✅ Core |
| Virtual WhenBucket include/exclude | ✅ Core |
| Text search (in-view `/` filter) | ✅ TUI |
| Rule engine (text match conditions, profile conditions) | ✅ Core |
| Done items hiding | ✅ CLI (`--include-done` flag) |

### Not yet implemented (potential implementation targets):

| Feature | Notes |
|---------|-------|
| **Datebook views** (period/interval/base-date) | Calendar-style auto-generated sections by time intervals |
| **Per-section column layout** | Sections currently share column config; Agenda allows per-section columns |
| **Numeric/Date/Unindexed column types** | Typed columns with aggregation, date formatting, free-text |
| **Numeric/Date filter ranges** | Filters with inside/outside range criteria |
| **Compound filters (AND logic)** | Multiple filter categories combined |
| **View-level vs section-level filters** | Two-tier filtering hierarchy |
| **Browse (date navigation)** | Shifting date windows forward/backward |
| **Show views (query views)** | System-generated read-only views for dependency chains, done items, text match, etc. |
| **View protection levels** | No protection / append-only / full protection |
| **Section sorting options** | Alphabetic / numeric / category order with direction |
| **Column reorder/width** | Interactive column layout manipulation |
| **View copy** | Duplicate a view as a starting point |
| **Hide inherited items** | Suppress items inherited from child categories |
| **Hide dependent items** | Suppress items with dependency relationships |
| **Section separators / item numbering** | Display formatting options |
| **Category note columns** | Display specific lines from category notes as column data |
| **Numeric aggregation** | Count/total/average/min/max at section footer |
