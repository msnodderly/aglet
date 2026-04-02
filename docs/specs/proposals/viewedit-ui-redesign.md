# ViewEdit UI Redesign

## Context

The current ViewEdit screen (view manager) is clunky and cryptic. Labels like "On insert assign", "On remove unassign", "Display override: inherit" are jargon. The expanded section info in the left pane (`criteria:1 columns:4 children:yes display:inherit`) is unreadable. No visual grouping of related settings. This redesign makes the screen pleasant, intuitive, and self-explanatory.

## UI Mockups

### Section Details (right pane) — Before → After

**Before:**
```
── DETAILS  Section 2 ──────────────────────
  Title: Ready
  Criteria: Include: Ready
  Columns: Issue type[w:12], Area[w:12], Priority[w:12], Complexity[w:12]
  On insert assign: (none)
  On remove unassign: (none)
  Show children: yes
  Display override: inherit
  Expand in Sections list: yes
  Tip: Enter/Space edits selected field (J/K or [/] reorder; shortcuts optional)
```

**After:**
```
── DETAILS: Ready ──────────────────────────
  Title: Ready
  Filter: Include: Ready
  ─────────────────────────────────────────
  Columns: Issue type, Area, Priority, Complexity
  Display mode: (use view default)
  ─────────────────────────────────────────
  Auto-assign on add: (none)
  Auto-unassign on remove: (none)
  Expand sub-categories: yes
```

Changes:
- **7 fields** (down from 8): removed "Expand in Sections list" entirely
- **Renamed labels**: Criteria→Filter, On insert assign→Auto-assign on add, On remove unassign→Auto-unassign on remove, Show children→Expand sub-categories, Display override→Display mode
- **Display mode values**: `inherit`→`(use view default)`, keep `single-line`/`multi-line`
- **Columns**: names only, no width numbers
- **Grouped** with thin `─` separator lines (DarkGray) between: [Title,Filter] / [Columns,Display mode] / [Auto-assign,Auto-unassign,Expand sub-categories]
- **Removed**: tip line, block title shows section name instead of number
- **Kept**: colon-separated label format

### View Details (right pane) — Before → After

**Before:**
```
── DETAILS  View Properties ────────────────
  Name (r): Aglet
  Criteria:
    Include: Ready
  When include: (none)
  When exclude: (none)
  Display mode: single-line
  Unmatched visible: yes
  Unmatched label: "Other"
  Aliases: (none)
  View keys: n:add x:remove Enter:pick/edit...
```

**After:**
```
── DETAILS: View ───────────────────────────
  Name: Aglet
  ─────────────────────────────────────────
  Filter criteria:
    Include: Ready
  ─────────────────────────────────────────
  Date range (include): (all)
  Date range (exclude): (none)
  ─────────────────────────────────────────
  Display mode: single-line
  Aliases: (none)
  ─────────────────────────────────────────
  Show unmatched: yes, as "Other"
  Unmatched label: "Other"
```

Changes:
- **Removed** "(r)" shortcut from Name label
- **Renamed**: Criteria→Filter criteria, When include→Date range (include), When exclude→Date range (exclude), Unmatched visible→Show unmatched
- **Show unmatched** combined display: `yes, as "Other"` or `hidden`
- **Grouped** with thin separators: [Name] / [Filter criteria] / [Date range] / [Display mode, Aliases] / [Unmatched]
- **Removed** view keys tip line, block title simplified

### Section List (left pane) — Before → After

**Before:**
```
── SECTIONS ────────────────────────
  View: Aglet
  ▸ 1. Backlog
  ▾ 2. Ready
       criteria:1  columns:4  children:yes  display:inherit
  ▸ 3. In Progress
  ▸ 4. Completed
```

**After:**
```
── SECTIONS ────────────────────────
  View: Aglet
  1. Backlog
> 2. Ready
  3. In Progress
  4. Completed
```

Title only. No expand/collapse icons. No inline summary. All details in right pane.

### Footer Hints

**Before:** `S:save  n:new  x:delete  Enter:details  Tab:pane  Esc:cancel`
**After:** `S:save  n:new  x:del  Tab:pane  Esc:close`

Keep colon style, shorter labels.

## Implementation Plan

### Step 1: Remove expand/collapse from sections list
**Files:** `crates/agenda-tui/src/lib.rs`, `crates/agenda-tui/src/modes/view_edit/sections.rs`, `crates/agenda-tui/src/modes/view_edit/details.rs`, `crates/agenda-tui/src/render/mod.rs`

- Remove `section_expanded: Option<usize>` from `ViewEditState` (lib.rs ~line 348)
- Remove all `section_expanded` references from the ViewEdit feature module (~15 occurrences in the old incremental bridge implementation: Enter toggle, tracking on add/delete/move)
- Simplify section list rendering in render/mod.rs (lines 3822-3887): remove expand icons (▸/▾), remove expanded detail lines, just show `{cursor} {n}. {title}`
- Remove field 7 from section details: change field count from 8 to 7
- Remove `7 => Some(KeyCode::Enter)` match arm from Enter/Space dispatch in the ViewEdit feature module

### Step 2: Redesign section details rendering
**File:** `crates/agenda-tui/src/render/mod.rs` lines 3597-3715

New field index mapping (0-6):

| Idx | Old label | New label |
|-----|-----------|-----------|
| 0 | Title: | Title: |
| 1 | Criteria: | Filter: |
| 2 | Columns: | Columns: |
| 3 | On insert assign: | Auto-assign on add: |
| 4 | On remove unassign: | Auto-unassign on remove: |
| 5 | Show children: | Expand sub-categories: |
| 6 | Display override: | Display mode: |

Additional:
- Columns: show names only, no `[w:N]` width suffix
- Display mode: `inherit`→`(use view default)`
- Add separator `ListItem`s between groups [0,1] / [2,6] / [3,4,5]
- Remove tip line
- Block title: `" DETAILS: {section_title} "`

Separator rendering:
```rust
items.push(ListItem::new(Line::from(Span::styled(
    "  ─────────────────────────────────",
    Style::default().fg(Color::DarkGray),
))));
```

### Step 3: Redesign view details rendering
**File:** `crates/agenda-tui/src/render/mod.rs` lines 3400-3596

unmatched_field_index mapping (0-5, unchanged):

| Idx | Old label | New label |
|-----|-----------|-----------|
| 0 | When include: | Date range (include): |
| 1 | When exclude: | Date range (exclude): |
| 2 | Display mode: | Display mode: |
| 3 | Unmatched visible: | Show unmatched: |
| 4 | Unmatched label: | Unmatched label: |
| 5 | Aliases: | Aliases: |

Additional:
- Name: remove "(r)" → just "Name:"
- Criteria header: → "Filter criteria:"
- Show unmatched value: `yes, as "{label}"` or `hidden`
- Date range (include) when empty: `(all)` not `(none)` — include=(none) means no date filter
- Add separators between groups
- Remove view keys tip line
- Block title: `" DETAILS: View "`

### Step 4: Footer hints
**File:** `crates/agenda-tui/src/render/mod.rs` (~line 2350, footer_hint_text function)

- `S:save  n:new  x:del  Tab:pane  Esc:close` (sections pane)
- Context-appropriate hints per pane focus (details, preview)

### Step 5: Create aglet tracking issue
- Create new issue in `aglet-features.ag` via CLI
- Title: "ViewEdit screen UI clarity redesign"
- Separate from 0e56 (wizard is different scope)

## Key Files
- `crates/agenda-tui/src/render/mod.rs` — all rendering (~lines 3298-4240, footer ~2350)
- `crates/agenda-tui/src/modes/view_edit/sections.rs` and `crates/agenda-tui/src/modes/view_edit/details.rs` — input handlers for field count and expand removal
- `crates/agenda-tui/src/lib.rs` — ViewEditState struct (remove section_expanded)

## Verification
1. `cargo build` — compilation
2. `cargo test` — no regressions
3. Manual TUI testing:
   - Open ViewEdit, navigate all 7 section detail fields (0-6) — highlight tracks with separators
   - Select "View" row, navigate all 6 view detail fields — grouping and labels correct
   - Sections list: clean title-only list, no expand icons
   - Footer hints: updated text per pane focus
   - Enter/Space on each field triggers correct edit action
   - Narrow terminal: labels don't break layout
