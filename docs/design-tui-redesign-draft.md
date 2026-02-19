# TUI Redesign: Consolidated Keymap & Lower Friction

Status: Draft proposal
Date: 2026-02-18

## Problem Statement

The current TUI has:
- **Two overlapping view editing paths** (View Editor `v e` and View Manager `v V`) that share sub-editors but enter them through different state transitions.
- **31 modes** in the Mode enum, many of which are intermediate states for multi-step wizards.
- **A normal-mode keymap of 25+ bindings**, displayed as a wall of text in the footer that nobody can read.
- **Item add (`n`) is a bare text line** in the footer — no affordance for categories, notes, or dates at creation time.
- **Item edit (`e`/`Enter`) opens a popup** with Tab-cycled focus between Text, Note, Categories button, Save, Cancel — 5 focus stops for a common operation.
- **Category assignment (`a`) is a separate top-level action** from item edit, creating two mental models for "change what's on this item."

## Design Principles

1. **One way to do each thing.** No parallel paths to the same destination.
2. **Progressive disclosure.** The main board shows only navigation + the 5 most common actions. Everything else is behind a command palette or contextual menus.
3. **Inline over popup.** Quick edits happen in-place. Popups are for multi-field forms.
4. **Consistent grammar.** Verb keys mean the same thing everywhere.

---

## Proposed Layout

```
┌─────────────────────────────────────────────────────────┐
│ View: My Tasks  ▸ Open (3)          filter: none        │  ← header
├─────────────────────────────────────────────────────────┤
│ Priority   Type     Item                         Status │  ← column headers
│ ──────────────────────────────────────────────────────── │
│ high       bug      Fix login timeout            open   │
│ medium     task   ▸ Write migration script        open   │  ← selected
│ low        feature  Add dark mode                open   │
├─────────────────────────────────────────────────────────┤
│ ▾ Closed (2)                                            │  ← section divider
│ Priority   Type     Item                         Status │
│ ──────────────────────────────────────────────────────── │
│ high       bug      Fix crash on empty input     closed │
│ medium     task     Update README                closed │
├─────────────────────────────────────────────────────────┤
│ n:new  e:edit  a:assign  d:done  /:filter  ?:help       │  ← footer (minimal)
│ Status: Marked item done                                │
└─────────────────────────────────────────────────────────┘
```

### Changes from Current

- **Sections stack vertically within one pane** instead of each section being a separate equal-height bordered box. Sections are separated by a thin divider line with a collapsible header.
- **Column headers repeat per section** (or once at top if all sections share columns — TBD).
- **Footer shows only 6 core keys** plus `?:help`. The `?` key opens a full help overlay.
- **Status message** is a single line below the key hints (replaces the dual-purpose prompt/status area).

---

## Proposed Mode Reduction

### Current: 31 Modes

### Proposed: 14 Modes

| Keep | Mode | Purpose |
|------|------|---------|
| ✅ | `Normal` | Board navigation |
| ✅ | `AddInput` | New item text entry (enhanced — see below) |
| ✅ | `ItemEditPopup` | Edit item (text + note + categories in one popup) |
| ✅ | `CategoryPicker` | Unified picker for any category selection context |
| ✅ | `FilterInput` | Text search |
| ✅ | `ConfirmDelete` | y/n confirmation |
| ✅ | `ViewManager` | Unified view editing (replaces ViewEditor + ViewManagerScreen + 5 sub-modes) |
| ✅ | `ViewPicker` | Quick switch between views |
| ✅ | `ViewCreateInput` | Name input for new view |
| ✅ | `CategoryManager` | Category CRUD |
| ✅ | `CategoryCreateInput` | New category name |
| ✅ | `CategoryConfigEditor` | Category settings popup |
| ✅ | `HelpOverlay` | Full keymap reference |
| ✅ | `BucketPicker` | Virtual bucket selection |

### Removed (absorbed into other modes)

| Remove | Was | Absorbed Into |
|--------|-----|---------------|
| ❌ | `NoteEditInput` | `ItemEditPopup` (note is a field in the popup) |
| ❌ | `ItemAssignCategoryPicker` | `CategoryPicker` (unified) |
| ❌ | `ItemAssignCategoryInput` | `CategoryPicker` (has inline type-to-filter) |
| ❌ | `InspectUnassignPicker` | `CategoryPicker` (toggle removes) |
| ❌ | `ViewEditor` | `ViewManager` |
| ❌ | `ViewEditorCategoryPicker` | `CategoryPicker` |
| ❌ | `ViewEditorBucketPicker` | `BucketPicker` |
| ❌ | `ViewManagerCategoryPicker` | `CategoryPicker` |
| ❌ | `ViewSectionEditor` | `ViewManager` (sections pane inline) |
| ❌ | `ViewSectionDetail` | `ViewManager` (section detail inline) |
| ❌ | `ViewSectionTitleInput` | `ViewManager` (inline rename) |
| ❌ | `ViewUnmatchedSettings` | `ViewManager` (settings row in sections pane) |
| ❌ | `ViewUnmatchedLabelInput` | `ViewManager` (inline edit) |
| ❌ | `ViewCreateCategoryPicker` | Dropped — create a blank view, then configure in ViewManager |
| ❌ | `ViewDeleteConfirm` | `ConfirmDelete` (reuse) |
| ❌ | `ViewRenameInput` | `ViewManager` (inline rename in views list) |
| ❌ | `CategoryRenameInput` | `CategoryManager` (inline rename) |
| ❌ | `CategoryReparentPicker` | `CategoryPicker` (reuse) |
| ❌ | `CategoryDeleteConfirm` | `ConfirmDelete` (reuse) |

---

## Normal Mode Keymap (Proposed)

### Navigation

| Key | Action |
|-----|--------|
| `j` / `↓` | Move cursor down within section |
| `k` / `↑` | Move cursor up within section |
| `Tab` | Jump to next section |
| `Shift+Tab` | Jump to previous section |
| `g` | Jump to "All Items" view |
| `,` / `.` | Cycle views prev / next |

### Core Item Actions

| Key | Action |
|-----|--------|
| `n` | New item (inline in footer, with hashtag parsing for categories and date) |
| `e` / `Enter` | Edit selected item (opens edit popup with text, note, categories) |
| `a` | Assign/unassign categories on selected item (opens category picker) |
| `d` | Toggle done on selected item |
| `x` | Delete selected item (with confirmation) |
| `[` / `]` | Move item to prev/next section (edit-through) |

### View & Filter

| Key | Action |
|-----|--------|
| `/` | Filter items in current view |
| `Esc` | Clear filter (if active) |
| `v` | Open view picker (switch, create, delete) |
| `V` | Open view manager (edit current view: criteria, columns, sections) |

### Meta

| Key | Action |
|-----|--------|
| `p` | Toggle preview panel |
| `c` | Open category manager |
| `?` | Show help overlay (full keymap + context-sensitive hints) |
| `q` | Quit |

### Removed from Normal Mode

| Was | Why |
|-----|-----|
| `m` (note edit) | Absorbed into `e` (item edit popup has note field) |
| `u` (unassign) | Absorbed into `a` (category picker toggles on/off) |
| `r` (remove from view) | Moved to `x` context or item edit |
| `f` (focus toggle) | Preview focus via `p` + scroll keys |
| `o` (preview mode toggle) | Preview mode via `p` submenu or auto |
| `J`/`K` (preview scroll) | When preview focused: `j`/`k` scroll it |
| `h`/`l` / `←`/`→` (slot cursor) | Removed — sections are vertical, not side-by-side |
| `F8`/`F9` (aliases) | Dropped — `v` and `c` are sufficient |

---

## View Manager Redesign

**One screen, three tabs, no sub-editors.**

Enter: `V` from normal mode (opens view manager for current view) or `v` then `e` on a selected view.

```
┌─ View Manager: My Tasks ──────────────────────────────────────┐
│  [Criteria]   [Columns]   [Sections]       Tab: switch tab    │
│───────────────────────────────────────────────────────────────│
│                                                               │
│  Criteria (Tab 1):                                            │
│  + Status                      ← include                     │
│  - Done/archived               ← exclude                     │
│  v+ Today, ThisWeek            ← virtual include              │
│                                                               │
│  N:add row  x:remove  Space:toggle +/-  Enter:pick category   │
│  ]:add virtual include  [:add virtual exclude                 │
│                                                               │
│───────────────────────────────────────────────────────────────│
│  s:save  r:rename view  Esc:back                              │
└───────────────────────────────────────────────────────────────┘
```

```
│  Columns (Tab 2):                                             │
│  > Priority      w:12                                         │
│    Type           w:10                                         │
│    Status         w:10                                         │
│                                                               │
│  N:add  x:remove  [/]:reorder  w:set width  Enter:change heading │
```

```
│  Sections (Tab 3):                                            │
│  > Open                                                       │
│      include: Status/open                                     │
│    Closed                                                     │
│      include: Status/closed                                   │
│    ──────────                                                 │
│    Unmatched: on (label: "Other")                             │
│                                                               │
│  N:add  x:remove  [/]:reorder  Enter:edit  t:rename           │
│  (in section edit): +/-:criteria  ]/[:virtual  h:children     │
```

### Key Changes

- **Virtual bucket editing** integrated into Criteria tab (was only in View Editor).
- **Sections pane shows criteria summary inline** — no need to Enter→drill down just to see what a section filters on.
- **Section detail editing is inline expansion**, not a separate popup mode.
- **One save action** (`s`) persists everything (criteria + columns + sections).
- **Rename** is `r` in this screen, not a separate mode.

---

## Item Add Enhancement

Currently `n` opens a bare `Add>` prompt. Proposal: keep it as a fast inline prompt but make it smarter.

```
Add> Fix login timeout #bug #high tomorrow
```

- **Hashtag parsing** already exists (see `unknown_hashtag_tokens`). Make it the primary way to assign categories at creation time.
- **Date parsing** already exists (see `BasicDateParser`). Dates in the text are already extracted.
- After pressing Enter, the item is created with categories and date assigned. No wizard.
- If you want to fine-tune after creation, press `e` on the item.

No changes needed to the input mechanics — just ensure the status line confirms what was parsed:

```
Status: Created "Fix login timeout" — assigned: bug, high — when: 2026-02-19
```

---

## Item Edit Popup Simplification

Current popup has 5 Tab-stops: Text, Note, CategoriesButton, Save, Cancel.

Proposed:

```
┌─ Edit Item ───────────────────────────────────┐
│ Text> Fix login timeout                       │
│                                               │
│ Note (Enter for newline, Tab to next field):  │
│ ┌───────────────────────────────────────────┐ │
│ │ This happens when the session expires     │ │
│ │ after 30 minutes of inactivity.           │ │
│ └───────────────────────────────────────────┘ │
│                                               │
│ Categories: bug, high, Status/open            │
│                                               │
│           [Save: Ctrl+S]  [Cancel: Esc]       │
│ a:edit categories  Tab:next field             │
└───────────────────────────────────────────────┘
```

Changes:
- **Save is `Ctrl+S`** (not a Tab-stop button). Reduces Tab stops from 5 to 3 (Text, Note, Categories line).
- **Cancel is always `Esc`** (not a Tab-stop button).
- **Categories line shows current assignments inline** as a read-only summary. Press `a` to open the category picker.
- **Tab** cycles only between Text and Note fields. No button-hunting.

---

## Help Overlay (`?`)

Replaces the unreadable footer string. Full-screen overlay, dismiss with `?` or `Esc`.

```
┌─ Help ──────────────────────────────────────────────────┐
│                                                         │
│ NAVIGATION                     ITEMS                    │
│ j/k or ↑/↓   move cursor      n       new item         │
│ Tab/S-Tab     next/prev section  e/Enter  edit item       │
│ ,/.           prev/next view   a       assign categories │
│ g             all items view   d       toggle done       │
│                                x       delete            │
│ VIEW & FILTER                  [/]     move to section   │
│ /             filter items                               │
│ Esc           clear filter     META                      │
│ v             view picker      p       toggle preview    │
│ V             view manager     c       category manager  │
│ q             quit             ?       this help         │
│                                                         │
│                        Esc to close                     │
└─────────────────────────────────────────────────────────┘
```

---

## Migration Path

This is a large refactor. Suggested phasing:

### Phase 1: Drop View Editor
- Remove `Mode::ViewEditor` and all sub-modes (`ViewEditorCategoryPicker`, `ViewEditorBucketPicker`).
- Add virtual bucket editing to View Manager's criteria tab.
- Remove `e` from view picker (or redirect it to open View Manager for that view).
- **Net mode reduction: 4 modes removed.**

### Phase 2: Consolidate Pickers
- Unify `ItemAssignCategoryPicker`, `ItemAssignCategoryInput`, `ViewManagerCategoryPicker`, `InspectUnassignPicker` into a single `CategoryPicker` mode parameterized by context.
- **Net mode reduction: 3 more modes removed.**

### Phase 3: Inline Section Editing
- Absorb `ViewSectionEditor`, `ViewSectionDetail`, `ViewSectionTitleInput` into ViewManager inline expansion.
- Absorb `ViewUnmatchedSettings`, `ViewUnmatchedLabelInput` into ViewManager sections tab.
- **Net mode reduction: 5 more modes removed.**

### Phase 4: Simplify Item Edit
- Remove `NoteEditInput` (note editing lives in item edit popup only).
- Streamline popup Tab stops.
- Add `?` help overlay.
- **Net mode reduction: 1 more mode, plus 1 new mode added.**

### Phase 5: Consolidate Confirmations & Rename
- Unify `ViewDeleteConfirm`, `CategoryDeleteConfirm` into `ConfirmDelete`.
- Absorb `ViewRenameInput`, `CategoryRenameInput` into inline editing.
- Absorb `ViewCreateCategoryPicker` — new views start blank.
- Remove `CategoryReparentPicker` — reuse `CategoryPicker`.
- **Net mode reduction: 5 more modes removed.**

### Final: 14 modes (down from 31).
