# TUI Color Design Language

**Status:** Proposal
**Date:** 2026-03-22
**Context:** Audit of `agenda-tui` revealed that Cyan and Yellow are both used
to indicate "focused pane" depending on the view, creating contradictory
semantics. This spec defines a single, consistent color language for the entire
TUI.

---

## Goals

- Every color carries exactly one semantic meaning, everywhere in the app.
- A user can learn the language once and apply it to any view.
- Implementation is expressed through a shared set of named constants; no
  raw `Color::*` literals at call sites.

---

## Semantic Roles

### Focus & Structure

| Constant | Color | Meaning |
|---|---|---|
| `COLOR_FOCUS` | `Color::Cyan` | This pane / element has keyboard focus |
| `COLOR_IDLE` | `Color::DarkGray` | Visible but not focused (border, inactive pane) |
| `COLOR_SPECIAL_MODE` | `Color::Magenta` | App is in an unusual/modal mode (e.g. classification) |

**Rule:** A pane border is `COLOR_FOCUS` when it receives keyboard input,
`COLOR_IDLE` otherwise. Never Yellow for a focused border.

---

### Selection & Editing

| Constant | Colors | Meaning |
|---|---|---|
| `COLOR_SELECTED_BG` | `Color::Cyan` bg + `Color::Black` fg | Cursor row in a focused list |
| `COLOR_CURSOR_BG` | `Color::DarkGray` bg | Cursor row in an unfocused list |
| `COLOR_EDIT_BG` | `Color::Rgb(65, 55, 10)` bg + `Color::White` fg + Bold | Text area / input box in edit mode |
| `COLOR_CELL_CURSOR` | `Color::Yellow` bg + `Color::Black` fg | Single active cell in a table (cursor only, not whole row) |

**Rationale for `COLOR_EDIT_BG`:** A dark amber background reads as "you are
typing here" with warmth that signals pending input, while avoiding the
optical harshness of black-on-bright-yellow across a full input widget.
Black-on-Yellow is reserved for small single-cell cursors where the high
contrast is an asset.

---

### Status & Outcome

| Constant | Color | Meaning |
|---|---|---|
| `COLOR_PENDING` | `Color::Yellow` fg | Something not yet saved, resolved, or confirmed |
| `COLOR_SUCCESS` | `Color::LightGreen` fg | Accepted / saved / successful |
| `COLOR_ERROR` | `Color::LightRed` fg | Rejected / failed / destructive action |

**Rule:** Yellow as a foreground color on the default background means
"in-flight." It must not appear as a border color on any pane.

---

### Content Hierarchy

| Constant | Color | Meaning |
|---|---|---|
| `COLOR_TEXT_PRIMARY` | `Color::White` | Main readable content |
| `COLOR_TEXT_SECONDARY` | `Color::Rgb(170, 178, 198)` | Labels, column headers, UI chrome |
| `COLOR_TEXT_MUTED` | `Color::Rgb(140, 140, 140)` | Placeholders, hints, disabled fields |

**Rule:** Do not use `Color::Gray`, `Color::DarkGray`, or `Color::LightCyan`
as foreground text colors. Collapse all muted-text shades to the two Rgb
values above.

---

## Colors Removed / Reassigned

| Color | Previous use | Disposition |
|---|---|---|
| `Color::Yellow` (border) | Focused pane in some views | Removed. Borders use Cyan or DarkGray only. |
| `Color::Blue` (border) | Inactive optional-field borders | Removed. Use `COLOR_IDLE` (DarkGray). |
| `Color::LightCyan` | Category pane focus, query prefix | Merged into `COLOR_FOCUS` (Cyan) or `COLOR_TEXT_SECONDARY`. |
| `Color::Gray` (fg text) | Labels, separators | Replaced by `COLOR_TEXT_SECONDARY` or `COLOR_TEXT_MUTED`. |
| `Color::DarkGray` (fg text) | Dimmed content | Replaced by `COLOR_TEXT_MUTED`. |

---

## Constants Definition

All constants live in a single shared location (e.g.
`crates/agenda-tui/src/theme.rs`) so every render module imports from one
source of truth.

```rust
use ratatui::style::{Color, Modifier, Style};

// Focus & structure
pub const COLOR_FOCUS: Color       = Color::Cyan;
pub const COLOR_IDLE: Color        = Color::DarkGray;
pub const COLOR_SPECIAL_MODE: Color = Color::Magenta;

// Selection & editing
pub const COLOR_SELECTED_BG: Color = Color::Cyan;           // bg; pair with Color::Black fg
pub const COLOR_CURSOR_BG: Color   = Color::DarkGray;       // bg; unfocused list cursor
pub const COLOR_EDIT_BG: Color     = Color::Rgb(65, 55, 10);// bg; text area in edit mode; pair with White fg + Bold
pub const COLOR_CELL_CURSOR: Color = Color::Yellow;          // bg; single table cell cursor; pair with Black fg

// Status
pub const COLOR_PENDING: Color     = Color::Yellow;
pub const COLOR_SUCCESS: Color     = Color::LightGreen;
pub const COLOR_ERROR: Color       = Color::LightRed;

// Content hierarchy
pub const COLOR_TEXT_PRIMARY: Color   = Color::White;
pub const COLOR_TEXT_SECONDARY: Color = Color::Rgb(170, 178, 198);
pub const COLOR_TEXT_MUTED: Color     = Color::Rgb(140, 140, 140);

// Convenience style constructors
pub fn style_focus_border() -> Style {
    Style::default().fg(COLOR_FOCUS)
}
pub fn style_idle_border() -> Style {
    Style::default().fg(COLOR_IDLE)
}
pub fn style_selected_row() -> Style {
    Style::default().fg(Color::Black).bg(COLOR_SELECTED_BG)
}
pub fn style_cursor_row() -> Style {
    Style::default().bg(COLOR_CURSOR_BG)
}
pub fn style_edit_area() -> Style {
    Style::default()
        .fg(Color::White)
        .bg(COLOR_EDIT_BG)
        .add_modifier(Modifier::BOLD)
}
pub fn style_cell_cursor() -> Style {
    Style::default().fg(Color::Black).bg(COLOR_CELL_CURSOR)
}
pub fn style_pending() -> Style {
    Style::default().fg(COLOR_PENDING)
}
pub fn style_success() -> Style {
    Style::default().fg(COLOR_SUCCESS)
}
pub fn style_error() -> Style {
    Style::default().fg(COLOR_ERROR)
}
```

---

## Quick Reference

```
Cyan        → keyboard focus (borders, active row bg)
DarkGray    → idle/inactive (borders, unfocused row bg)
Yellow fg   → pending / unsaved / unresolved
Dark amber  → text area being edited
LightGreen  → accepted / success
LightRed    → rejected / error
White       → primary text
Blue-gray   → labels, chrome
Mid-gray    → placeholders, hints
Magenta     → special/modal mode
```

---

## Migration Notes

- Replace the four existing `CATEGORY_MANAGER_*` color constants with the
  shared palette above.
- `selected_row_style()` and `selected_board_row_style()` in `ui_support.rs`
  should delegate to the new style constructors.
- `focused_cell_style()` in `ui_support.rs` maps to `style_cell_cursor()` (not
  `style_edit_area()`; it is a single-cell cursor, not a text box).
- Audit every `border_style(Style::default().fg(Color::Yellow))` call site and
  replace with `style_focus_border()` or `style_idle_border()`.
- The `Modifier::REVERSED` usages for selection should be replaced with
  `style_selected_row()` for consistency.
