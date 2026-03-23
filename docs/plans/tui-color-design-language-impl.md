# Implementation Plan: TUI Color Design Language

**Spec:** [`docs/specs/proposals/tui-color-design-language.md`](../specs/proposals/tui-color-design-language.md)
**Branch:** `claude/tui-color-design-language-iMXXH`
**Status:** Not started

---

## Overview

The implementation has three phases:

1. **Foundation** — create `theme.rs` with all named constants and style
   constructors; update `ui_support.rs` helpers to delegate to it.
2. **Render migration** — systematically replace raw `Color::*` literals in
   `render/mod.rs` by render function, using the new constructors.
3. **Cleanup** — delete old local constants, verify no raw literals remain,
   visual smoke-test.

Total affected files:
- `crates/agenda-tui/src/render/mod.rs` (~82 affected color sites)
- `crates/agenda-tui/src/ui_support.rs` (2 sites + 4 helper fns)
- `crates/agenda-tui/src/lib.rs` (add `mod theme` declaration)
- New: `crates/agenda-tui/src/theme.rs`

---

## Phase 1 — Foundation

### 1.1 Create `theme.rs`

File: `crates/agenda-tui/src/theme.rs`

Define all constants and style constructors exactly as specified in the
proposal. No logic, just named values and simple `Style` builders.

- [ ] Define `COLOR_FOCUS`, `COLOR_IDLE`, `COLOR_SPECIAL_MODE`
- [ ] Define `COLOR_SELECTED_BG`, `COLOR_CURSOR_BG`, `COLOR_EDIT_BG`,
      `COLOR_CELL_CURSOR`
- [ ] Define `COLOR_PENDING`, `COLOR_SUCCESS`, `COLOR_ERROR`
- [ ] Define `COLOR_TEXT_PRIMARY`, `COLOR_TEXT_SECONDARY`, `COLOR_TEXT_MUTED`
- [ ] Add `style_focus_border()` → `Style::default().fg(COLOR_FOCUS)`
- [ ] Add `style_idle_border()` → `Style::default().fg(COLOR_IDLE)`
- [ ] Add `style_selected_row()` → `Black` fg on `COLOR_SELECTED_BG` bg
- [ ] Add `style_cursor_row()` → `COLOR_CURSOR_BG` bg
- [ ] Add `style_edit_area()` → `White` fg, `COLOR_EDIT_BG` bg, `BOLD`
- [ ] Add `style_cell_cursor()` → `Black` fg on `COLOR_CELL_CURSOR` bg
- [ ] Add `style_pending()` → `COLOR_PENDING` fg
- [ ] Add `style_success()` → `COLOR_SUCCESS` fg
- [ ] Add `style_error()` → `COLOR_ERROR` fg
- [ ] Add `style_text_secondary()` → `COLOR_TEXT_SECONDARY` fg
- [ ] Add `style_text_muted()` → `COLOR_TEXT_MUTED` fg

### 1.2 Wire up `theme.rs`

- [ ] Add `pub(crate) mod theme;` to `crates/agenda-tui/src/lib.rs`
- [ ] Add `use crate::theme::*;` (or explicit imports) at the top of
      `render/mod.rs` and `ui_support.rs`

### 1.3 Migrate `ui_support.rs` helpers

Four functions; replace inline color literals with theme constructors:

- [ ] `selected_row_style()` → return `style_selected_row()`
- [ ] `selected_board_row_style()` → return `style_cursor_row()`
  - Note: currently `DarkGray` bg only — this aligns exactly with
    `COLOR_CURSOR_BG`
- [ ] `marked_board_row_style()` → keep `Rgb(40, 70, 120)` bg for now (no
  theme constant for "marked" state; consider adding `COLOR_MARKED_BG` in a
  follow-up)
- [ ] `focused_cell_style()` → return `style_cell_cursor()` (single-cell
  table cursor, *not* `style_edit_area()`)

---

## Phase 2 — Render Migration

Work through `render/mod.rs` render function by render function. Each item
below is a self-contained commit.

### 2.1 Delete old local constants (lines 3–7)

```rust
// Remove:
const CATEGORY_MANAGER_PANE_IDLE: Color = Color::Rgb(82, 92, 112);
const CATEGORY_MANAGER_PANE_FOCUS: Color = Color::LightCyan;
const CATEGORY_MANAGER_TEXT_ENTRY: Color = Color::LightMagenta;
const CATEGORY_MANAGER_EDIT_FOCUS: Color = Color::Yellow;
const MUTED_TEXT_COLOR: Color = Color::Rgb(140, 140, 140);
```

Replacements:
- `CATEGORY_MANAGER_PANE_IDLE` → `COLOR_IDLE`
- `CATEGORY_MANAGER_PANE_FOCUS` → `COLOR_FOCUS`
- `CATEGORY_MANAGER_TEXT_ENTRY` → `COLOR_SPECIAL_MODE` (LightMagenta → Magenta;
  or keep as-is with a new `COLOR_INPUT_ACTIVE` constant — decide before
  implementing)
- `CATEGORY_MANAGER_EDIT_FOCUS` → `COLOR_EDIT_BG` (used as bg)
- `MUTED_TEXT_COLOR` → `COLOR_TEXT_MUTED`

- [ ] Remove constants block
- [ ] Fix all call sites that referenced those names

### 2.2 `render_category_direct_edit_picker` (lines 323–618)

Affected sites: lines 460, 470, 490, 496

- [ ] Line 496: `Color::Yellow` label `"Category> "` → `style_pending()` (it
  is an active input prompt, reads as "you are typing a category")
- [ ] Border focus/idle pattern → `style_focus_border()` / `style_idle_border()`

### 2.3 `render_link_wizard` (lines 619–1011)

Affected sites: lines 678, 701, 716, 718, 755, 914, 1844

- [ ] Line 678: `DarkGray` idle border → `style_idle_border()` ✓ (already
  correct semantics, just use constant)
- [ ] Lines 701, 716, 755, 914: `Color::Yellow` focus borders →
  `style_focus_border()` (these are the inverted-semantics cases)
- [ ] Line 718: `DarkGray` → `style_idle_border()`
- [ ] Line 1844: `Color::Blue` optional-field border → `style_idle_border()`

### 2.4 `render_category_column_picker` (lines 1034–1244)

Affected sites: lines 1104, 1110, 1176, 1748, 1759, 1763

- [ ] Lines 1104, 1176: focus border → `style_focus_border()`
- [ ] Line 1110: `Color::Yellow` "Filter> " prompt → `style_pending()`
- [ ] Lines 1748, 1763: `DarkGray` inactive → `style_idle_border()`
- [ ] Line 1759: `Color::Yellow` text → `style_pending()`

### 2.5 `render_board_add_column_picker` / board columns (lines 1245–2507)

Affected sites: lines 1303, 1915, 2195, 2197, 2533

- [ ] Line 1303: `Color::Yellow` "Category> " prompt → `style_pending()`
- [ ] Lines 1915, 2533: `Color::Blue` inactive border → `style_idle_border()`
- [ ] Lines 2195, 2197: `DarkGray` board row padding bg → `style_cursor_row()`
  or keep inline if it is structural padding (not selection state)

### 2.6 Preview panels (lines 2753–2964)

Affected sites: lines 2799–2803, 2957

- [ ] Lines 2799–2801, 2955: `Color::Cyan` focus border → `style_focus_border()`
- [ ] Lines 2803, 2957: `Color::Yellow` unfocused border → `style_idle_border()`
  (this is the primary inverted-semantics fix in the preview panel)

### 2.7 `render_suggestion_review` (lines 2965–3192)

Affected sites: lines 2996, 3043, 3058, 3088, 3098, 3099, 3104, 3123, 3125,
3145, 3156, 3172, 3220, 3233

- [ ] Lines 2996, 3058: `Color::Yellow` focus border → `style_focus_border()`
  (suggestion review also has the inverted pattern)
- [ ] Line 3043: `Color::Yellow` count label → `style_pending()` ✓
- [ ] Lines 3088, 3098, 3099, 3104, 3172: `Color::Gray` labels →
  `style_text_secondary()`
- [ ] Lines 3123/3125: accept/reject → already `LightGreen`/`LightRed`;
  switch to `style_success()` / `style_error()`
- [ ] Line 3145: `Color::LightRed` reject → `style_error()`
- [ ] Line 3156: `DarkGray` text, `Cyan` bg → keep `Cyan` bg (selection), use
  `COLOR_TEXT_MUTED` for fg
- [ ] Lines 3220, 3233: `Color::LightCyan` query prefix → `style_text_secondary()`

### 2.8 `render_help_panel` (lines 3689–3787)

Affected sites: lines 3698, 3774

- [ ] Line 3698: `Color::Yellow` key label fg → `style_pending()` or consider
  a dedicated `style_key_binding()` using `COLOR_FOCUS` (Cyan) since these
  are navigation hints, not pending state — **decide**: key bindings are UI
  chrome, so `COLOR_TEXT_SECONDARY` may be more appropriate for the
  description text and `COLOR_FOCUS` for the key itself
- [ ] Line 3774: `DarkGray` text → `style_text_muted()`

### 2.9 `render_input_panel` (lines 3788–4326)

Affected sites: lines 3844, 3889, 3893, 3896, 3918, 3928, 3932, 3958, 3989,
4001–4003, 4010, 4016, 4025, 4030, 4032, 4042, 4083, 4091, 4126

This function is the most complex. Break into sub-tasks:

- [ ] Lines 3844, 3889, 3918, 3958, 3989: `Color::Yellow` fg text →
  `style_pending()` (all are "in-flight edit" indicators)
- [ ] Line 3893: `DarkGray` cursor-line bg → `style_cursor_row()`
- [ ] Line 3896: `Yellow` bg active cell → `style_cell_cursor()`
- [ ] Line 3928: `DarkGray` cursor-line bg on note widget → `style_cursor_row()`
- [ ] Line 3932: `Yellow` bg note cursor → `style_edit_area()` (full text
  area, not single cell — this is the dark-amber case)
- [ ] Lines 4001–4003: `Color::Yellow` separator spans and `Color::Gray` →
  `style_pending()` and `style_text_secondary()`
- [ ] Line 4010: `SuggestionDecision::Pending` → `style_pending()` ✓
- [ ] Line 4016: `DarkGray` bg row → `style_cursor_row()`
- [ ] Lines 4025, 4030, 4032: `DarkGray` bg with `White`/`Gray` fg →
  `style_cursor_row()` + `style_text_secondary()`
- [ ] Line 4042: `Color::Yellow` fg → `style_pending()`
- [ ] Line 4083: `DarkGray` fg text → `style_text_muted()`
- [ ] Line 4091: `DarkGray` fg, `Cyan` bg → `COLOR_TEXT_MUTED` + `COLOR_SELECTED_BG`
- [ ] Line 4126: `LightCyan` bg → `COLOR_FOCUS` (Cyan); this appears to be a
  selection highlight variant

### 2.10 `render_category_manager` (lines 4451–5465)

Affected sites: lines 4994, 4998, 5046, 5050

- [ ] Lines 4994, 5046: `DarkGray` cursor-line bg → `style_cursor_row()`
- [ ] Lines 4998, 5050: `Yellow` bg text area → `style_edit_area()` (full
  input boxes)

### 2.11 Overlay pickers/popups that should remain normal focus surfaces

Affected sites: lines 4333, 4420

- [ ] Line 4333: View Palette currently uses `Color::Magenta` border. Replace
  with `style_focus_border()` because this is a normal picker, not a special
  app mode.
- [ ] Line 4420: Assign Item popup should also use `style_focus_border()` for
  consistency with the shared focus language.

### 2.12 `render_view_edit_screen` (lines 5466–6448)

Affected sites: lines 5502, 5556, 5581, 5871, plus remaining
`Modifier::REVERSED` selection paths in the same screen

- [ ] Line 5502: `let inactive_border = Color::Blue` → `COLOR_IDLE`
- [ ] Line 5556: `inactive_border` usage propagates from above fix
- [ ] Lines 5581, 5871: `DarkGray` separator → `style_text_muted()`
- [ ] Replace ViewEdit detail-field selection from `Modifier::REVERSED` with
  theme-backed selection styling so focused rows use the shared palette.
- [ ] Replace inline-edit and section-row reverse-video highlights with
  `style_selected_row()` or an equivalent theme-backed focused-row style.
- [ ] Replace overlay picker reverse-video highlights in ViewEdit with
  theme-backed selection styling so the pickers do not depend on terminal
  defaults.

### 2.13 Picker border group (lines 6448–6549)

Affected sites: lines 6448, 6481, 6511, 6542

All four are `border_style(Style::default().fg(Color::Yellow))` — all are
focus borders with wrong color.

- [ ] Lines 6448, 6481, 6511, 6542: Yellow focus borders → `style_focus_border()`

---

## Phase 3 — Cleanup & Verification

### 3.1 Lint for remaining raw literals

After all render function commits, verify no banned literals remain:

- [ ] `grep -n "Color::Yellow" render/mod.rs` → zero results (or annotate any
  intentional exceptions)
- [ ] `grep -n "Color::Blue\b" render/mod.rs` → zero results
- [ ] `grep -n "Color::LightCyan" render/mod.rs` → zero results
- [ ] `grep -n 'Color::Gray\b' render/mod.rs` → zero results
- [ ] `grep -n 'Color::DarkGray' render/mod.rs` → only structural bg uses
  (board row padding), not text or border colors

### 3.2 Compile check

- [ ] `cargo build -p agenda-tui` passes with no warnings related to unused
  constants or imports

### 3.3 Visual smoke-test

Exercise each affected view and confirm:

- [ ] Any pane border is Cyan when focused, DarkGray when not
- [ ] No Yellow pane borders visible anywhere
- [ ] Input prompts ("Category> ", "Filter> ") appear in Yellow (pending)
- [ ] Text areas in edit mode show dark amber background, not bright yellow
- [ ] Suggestion review: accept = green, reject = red, pending = yellow
- [ ] Preview pane: focused = Cyan border, unfocused = DarkGray border
- [ ] Help panel key labels readable; descriptions in secondary gray
- [ ] Category manager edit areas use dark amber (not bright yellow)
- [ ] View Palette uses the standard focus border, not magenta
- [ ] ViewEdit highlighted rows/pickers use theme colors rather than reverse
  video

### 3.4 PR

- [ ] Open PR from `claude/tui-color-design-language-iMXXH` → `main`
- [ ] Link spec in PR description

---

## Open Questions (resolve before starting Phase 2)

1. **`CATEGORY_MANAGER_TEXT_ENTRY` (LightMagenta)** — currently used to
   indicate "this pane is for text entry." Should this become `COLOR_SPECIAL_MODE`
   (Magenta) or a new `COLOR_INPUT_ACTIVE` constant? Magenta is already
   spec'd for "unusual mode" — a text entry pane may not be unusual enough
   to warrant it.

2. **Key binding labels in help panel** — are these `COLOR_PENDING` (Yellow)
   or `COLOR_FOCUS` (Cyan)? The spec doesn't explicitly cover help panel
   chrome. Cyan reads as "you can press this," which may be more intuitive
   than Yellow.

3. **`marked_board_row_style()` (`Rgb(40, 70, 120)`)** — the dark blue
   "marked/flagged" row has no theme constant yet. Worth adding
   `COLOR_MARKED_BG` to `theme.rs` as part of this work, or defer?

4. **Board row padding `DarkGray` bg (lines 2195, 2197)** — this is
   structural padding to fill empty cells, not a selection state. Should it
   use `COLOR_CURSOR_BG` (same color, correct constant) or stay inline to
   signal it is intentionally structural?
