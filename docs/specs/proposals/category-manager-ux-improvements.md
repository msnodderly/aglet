# Category Manager UX Improvements

## Context

The Category Manager (`F9` / mode:CategoryManager) is a multi-pane modal that handles category CRUD, flag editing, workflow role assignment, classification mode selection, and per-category notes. The current layout packs all of this into a single screen: a Global Settings pane (5 rows, top), Filter + Tree (left 62%), and Details (right 38%).

The UI is functional but has usability and discoverability issues at scale (20+ categories). Badges accumulate as text noise, actionable controls are buried below metadata, the footer hint bar lists every possible keybinding regardless of context, and the Global Settings pane permanently occupies space for rarely-changed settings.

Screenshot reference: `screenshots/category-manager-2026-03-20.png`

## Problems

### P1. Footer hint bar is a flat wall of shortcuts
The footer shows all keybindings at once:
```
S:save  n:new  r:rename  x:delete  m:classify  Tab:pane  /:filter  w:workflow  Esc:close  ?:help
```
With 10+ shortcuts in a single line, users can't quickly find the action they need. Shortcuts for tree manipulation (`H`/`L`/`J`/`K`) aren't shown at all — they're only discoverable through `?:help`.

### P2. Details pane leads with metadata, not controls
The Details pane renders in this order:
1. Metadata: Selected name, Depth, Children, Items, Parent, Reserved
2. Flags: Exclusive, Auto-match, Actionable
3. Note

Users open Details to toggle flags or edit notes, not to read `Depth: 0`. The editable content starts halfway down the pane.

### P3. Flags lack inline explanations
Each flag shows a checkbox and a label:
```
[ ] Exclusive
[ ] Auto-match
[ ] Actionable
```
The hint text at the bottom changes per-focused-flag, but only one description is visible at a time. A new user must focus each flag individually to learn what it does.

### P4. Global Settings pane is always expanded
The Global Settings block uses 5 rows (border + 2 items + border + gap) for two settings that change infrequently. This pushes the tree and filter down, reducing visible categories — especially painful at 40-row terminal heights.

### P5. Tree badges are text-heavy
Badges like `[reserved]`, `[exclusive]`, `[ready-queue]`, `[claim-target]` are appended as plain text. At deep nesting with multiple badges, lines overflow or wrap:
```
    Ready [exclusive] [ready-queue]
    In Progress [exclusive] [claim-target]
```

### P6. Tree hierarchy lacks visual connectors
Indentation uses 2-space offsets with no tree-drawing characters. With many categories at mixed depths, it's hard to trace parent→child relationships, especially when scrolling mid-tree.

### P7. Reserved categories compete for attention
`When`, `Entry`, and `Done` are read-only (can't rename, delete, or toggle flags) but are visually identical to user-created categories. They take up tree rows and selection stops without offering any editable interaction.

## Proposals

### S1. Context-sensitive footer hints
**Priority: high | Effort: low**

Show different hint sets based on `CategoryManagerFocus`:

```
Tree focused:
  n:new  r:rename  x:delete  H/L:indent  J/K:reorder  /:filter  Tab:details  ?:help

Details focused:
  Space:toggle  Enter:edit  j/k:field  S:save note  Tab:tree  ?:help

Filter focused:
  type to filter  Esc:clear  Enter/Tab:tree  ?:help

Global focused:
  Enter:open  j/k:select  Tab:filter  ?:help
```

Implementation: branch on `manager_focus` in `footer_hint_text()` for `Mode::CategoryManager`, returning a focus-specific hint string. The data is already available via `self.category_manager.as_ref().map(|s| s.focus)`.

### S2. Reorder Details: flags first, metadata second
**Priority: high | Effort: low**

Change the Details pane layout order to:
1. **Flags** (or Numeric Format) — the actionable content
2. **Note** — the editable content
3. **Metadata summary** — collapsed to a single line

The metadata line could be:
```
Parent: Status  Children: 5  Items: 12  Depth: 1
```
Shown as a dim footer line within the Details pane, not as a multi-line header. For reserved categories, append `(read-only)`.

Layout constraint change:
```rust
// Before:
[Length(info_height), Length(flags_height), Min(5), Length(2)]
// After:
[Length(flags_height), Min(5), Length(1), Length(2)]
```

### S3. Inline flag descriptions
**Priority: high | Effort: low**

Add a short description after each flag label:

```
Flags ──────────────────────────────
[ ] Exclusive      one child per item
[ ] Auto-match     assign by name in text
[ ] Actionable     required to mark done
```

For numeric categories:
```
Numeric Format ─────────────────────
[ ] Integer
    Decimal places: 2
    Currency symbol: $
[x] Thousands separator
```

Implementation: append the description as a dimmed `Span` to each flag `Line`. Use `MUTED_TEXT_COLOR` for the description text. Pad flag labels to align descriptions.

### S4. Collapse Global Settings to a summary line
**Priority: medium | Effort: medium**

Replace the 5-row bordered Global Settings pane with a single line above the filter:

```
Classification: Auto-apply │ Ready: Ready │ Claim: In Progress
```

Keep `m` and `w` as the interaction keys (already functional). The `GlobalSettings` focus variant either activates this line (highlight it, show picker on Enter) or can be removed entirely since `m`/`w` work from any focus.

This reclaims 4 vertical rows for the tree. At a 40-row terminal, that's ~10% more visible categories.

Alternative (less aggressive): make the Global Settings pane collapsible — show full pane only when `CategoryManagerFocus::Global`, collapse to summary line otherwise.

### S4b. Make global options read like actions, not labels
**Priority: medium | Effort: medium**

The current summary line is compact, but it still reads like passive metadata. The two global controls that matter most here are:

- how auto-classification behaves for new/edited items
- which categories power the ready/claim workflow

Those should read like actionable settings with direct hints, not just nouns with values.

Suggested mockup:

```text
Agenda Reborn  view:Aglet  mode:CategoryManager
Auto classification: Suggest/Review (m change)  |  Ready queue: Ready  |  Claim result: In Progress (w roles)

Action / Filter
Press / to filter categories. Press m to change auto-classification. Press w to edit ready/claim queues.

Category Manager                                  Details
+ Categories are shared across the database.      + Flags
+ Shift-Up/Down: reorder siblings                 | [x] Exclusive      one child per item
+ H/L or << / >>: change level                    | [ ] Auto-match     assign by name in item text
+ Tab: details                                    | [x] Actionable     required before marking done
                                                   |
                                                   | Also Match
                                                   | ...
                                                   |
                                                   | Note
                                                   | ...
                                                   |
                                                   | Parent: Status  Children: 5  Items: 12
```

Why this is better:

- `m change` and `w roles` explain intent, not just the raw shortcut.
- The action/filter strip can carry one line of "what can I do here?" guidance without needing a permanent global-settings pane.
- Tree movement hints become visible in the body, where users look while reorganizing categories.
- Ready/claim roles stay visible as database-level settings instead of feeling like per-category toggles.

Implementation notes:

- Keep the single-line global summary from S4, but rename labels to `Auto classification`, `Ready queue`, and `Claim result`.
- Default filter/action copy should mention `/`, `m`, and `w`.
- Footer hints should be focus-sensitive:
  - Tree: create/rename/delete/reorder/filter/global options
  - Details: field nav/toggle/save
  - Filter: type/clear/leave
  - Overlays: picker-specific controls only

### S5. Style-based badges instead of text tags
**Priority: medium | Effort: medium**

Replace text badges with visual styling:

| Current | Proposed |
|---------|----------|
| `[reserved]` | Dim/italic name, no badge |
| `[exclusive]` | Small marker or color accent on name |
| `[numeric]` | `♯` prefix or distinct color |
| `[ready-queue]` | Colored dot or `◆` prefix |
| `[claim-target]` | Colored dot or `◇` prefix |

Example tree rendering:
```
  Status                          (dimmed: exclusive)
    Needs Refinement
    Waiting/Blocked
  ◆ Ready                         (green accent)
  ◇ In Progress                   (cyan accent)
    Complete
```

Reserved categories render with `Style::default().fg(MUTED_TEXT_COLOR).add_modifier(Modifier::ITALIC)`.

This reduces line length and makes scanning faster. Badge details remain visible in the Details pane.

### S6. Tree-drawing characters
**Priority: low | Effort: low**

Add box-drawing connectors to the tree:

```
Status [exclusive]
├─ Needs Refinement
├─ Waiting/Blocked
├─ Ready [ready-queue]
├─ In Progress [claim-target]
└─ Complete
Priority [exclusive]
├─ Critical
├─ High
├─ Normal
└─ Low
```

Implementation: in the tree rendering loop, replace `"  ".repeat(row.depth)` with computed prefix using `├─`, `└─`, and `│ ` based on whether the row is the last child of its parent. Requires passing sibling position info into `CategoryListRow` or computing it during rendering.

### S7. Dim reserved categories
**Priority: low | Effort: low**

Apply `Modifier::DIM` to reserved category rows (`When`, `Entry`, `Done`). When selected, show a note in the Details pane: "This is a reserved system category. Its configuration cannot be changed."

Skip rendering the Flags section entirely for reserved categories — replace with a brief explanation of what the reserved category does.

## Phasing

**Phase 1 — Quick wins (S1, S2, S3):**
Context-sensitive hints, reorder Details layout, inline flag descriptions. All are localized rendering changes with no state or data model impact.

**Phase 2 — Layout refinement (S4, S5):**
Collapse Global Settings, style-based badges. S4 requires adjusting the vertical layout constraints; S5 requires changing how badges are computed and rendered in the tree loop.

**Phase 3 — Polish (S6, S7):**
Tree connectors and reserved category treatment. Low priority, can be deferred.

## Non-goals

- Changing the category data model or storage
- Adding new category features (sorting, bulk operations)
- Modifying the CategoryColumnPicker or CategoryDirectEdit UIs (separate components)
- Changing keybindings (only changing which hints are shown when)
