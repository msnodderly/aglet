---
title: Vertical Board Scrolling for Datebook Views
status: shipped
created: 2026-04-05
shipped: 2026-04-09
---

# Plan: Vertical Board Scrolling for Datebook Views

## Context

With Collapse mode and fine-grained intervals (e.g., Month period + Daily interval = 30 slots), collapsed empty sections still take 1 row each. A month of daily slots fills the entire viewport with collapsed lines, leaving no room for non-empty sections. Scrolling is needed so the board can contain more slots than fit on screen.

## Approach: Windowed Slot Rendering

Instead of refactoring to a virtual scrolling canvas, use a **windowed approach**: pre-compute the height each slot needs, determine which slots are visible at the current scroll offset, and only pass those to `Layout::split()`. This preserves the existing rendering architecture.

### How it works

1. **Pre-compute slot heights** — Before rendering, calculate how many terminal rows each slot needs:
   - Hidden (EmptySections::Hide + empty): 0 rows
   - Collapsed (EmptySections::Collapse + empty): 1 row
   - Non-empty, Full borders: 2 (borders) + 1 (header) + item_count rows (SingleLine mode)
   - Non-empty, Compact borders: 1 (top border/header) + item_count rows
   - Show mode, empty: same as non-empty but with 1 placeholder item row

2. **Compute visible window** — Starting from `board_scroll_offset` (index of the first visible slot), accumulate heights until the viewport is full. This gives the range `[first_visible..last_visible]`.

3. **Render only visible slots** — Build constraints and `Layout::split()` for only the visible range. Map visible indices back to logical slot indices.

4. **Auto-scroll on navigation** — When `move_slot_cursor` or `move_item_cursor` moves focus outside the visible window, adjust `board_scroll_offset` to bring the focused slot into view.

5. **Scrollbar** — Render a viewport-level scrollbar showing position within the full slot list.

### Why this is simpler than full virtual scrolling

- Still uses `Layout::split()` — just with fewer slots
- No pixel-level clipping of partially-visible slots needed (slots are always fully visible or fully hidden)
- No changes to how individual slot content (Table, collapsed line) is rendered
- Reuses existing `stable_table_offset` pattern for auto-scroll

## Complexity & Risk Assessment

### Overall: **Medium** (~200-300 lines, concentrated in 2 files)

### By component:

| Component | Complexity | Risk | Notes |
|-----------|-----------|------|-------|
| Slot height pre-computation | Low | **Medium** | SingleLine mode is trivial (1 row per item). MultiLine mode requires text wrapping pre-pass — but we can use SingleLine row counts as approximation and accept imperfect scroll for MultiLine initially. |
| Visible window computation | Low | Low | Simple accumulator loop. |
| Render loop windowing | Medium | **Medium** | Must remap all `columns[slot_index]` references to use a visible-index. Currently ~6 references. Risk of off-by-one or missed reference. |
| board_scroll_offset state | Low | Low | Single `usize` field on App, clamped on refresh. Pattern exists (`preview_scroll`). |
| Auto-scroll on navigation | Medium | Low | Follows existing `stable_table_offset` pattern. Adjust offset when focused slot is outside `[first_visible..last_visible]`. |
| Viewport scrollbar | Low | Low | Reuse existing `render_vertical_scrollbar()`. |
| Horizontal lane mode | **Skipped** | — | Horizontal lanes are a different layout axis. Defer vertical scrolling of lanes to a future task. |
| Interaction with EmptySections | Low | Low | Hidden slots have height 0 (skipped). Collapsed slots have height 1 (already handled). |

### Key risks:

1. **MultiLine display mode height accuracy** — In MultiLine mode, item row heights depend on text wrapping which depends on column width, which depends on viewport width. Pre-computing exact heights requires duplicating the text wrapping logic. **Mitigation**: Use `item_count` as height (1 row per item) for scroll calculations regardless of display mode. MultiLine items that take more rows than expected just means the viewport might not perfectly fill — acceptable for v1.

2. **Index remapping** — The render loop uses `slot_index` (logical) to index into `self.slots`, `self.section_filters`, `self.horizontal_slot_scroll_offsets`, etc. When windowing, `columns[i]` maps to visible index `i`, but slot data uses logical index. Must maintain a mapping. **Mitigation**: Create a `visible_slots: Vec<usize>` mapping visible position → logical slot index.

3. **Filter input state** — The `/` per-section filter uses `self.slot_index` to determine which section is being filtered. This still works since `slot_index` remains the logical index. No change needed.

## Implementation Steps

### 1. Add scroll state to App (`app.rs`, `lib.rs`)

```
board_scroll_offset: usize  // index of first visible slot
```

Clamp in `refresh()` after slot_index adjustment. Reset to 0 on view switch.

### 2. Add slot height estimation helper (`render/mod.rs` or `ui_support.rs`)

```rust
fn estimate_slot_heights(&self) -> Vec<u16> {
    // Returns desired row count per slot
}
```

### 3. Add visible window computation (`render/mod.rs`)

```rust
fn visible_slot_range(&self, heights: &[u16], viewport_height: u16) -> Vec<usize> {
    // Starting from board_scroll_offset, accumulate heights,
    // return logical slot indices that fit in viewport
}
```

### 4. Modify `render_board_columns` (`render/mod.rs` ~line 2261)

- Call height estimation + visible window
- Build constraints only for visible slots
- In the render loop, iterate `visible_slots` instead of `self.slots`
- Use `visible_slots[i]` to index into `self.slots`, filters, etc.
- Render viewport scrollbar after the loop

### 5. Auto-scroll in navigation (`app.rs`)

- After `move_slot_cursor`: if `self.slot_index` is outside visible window, adjust `board_scroll_offset`
- Simple approach: if focused slot is below viewport, set offset so focused slot is at bottom; if above, set offset to focused slot index

### 6. Scrollbar (`render/mod.rs`)

- After rendering all visible slots, render a scrollbar on the right edge showing position within total slots

## Files to modify

| File | Change |
|------|--------|
| `crates/agenda-tui/src/lib.rs` | Add `board_scroll_offset: usize` to App struct |
| `crates/agenda-tui/src/app.rs` | Clamp scroll offset in `refresh()`, adjust in `move_slot_cursor()` |
| `crates/agenda-tui/src/render/mod.rs` | Slot height estimation, visible window, windowed render loop, viewport scrollbar |

## What this does NOT change

- Individual slot rendering (Table, collapsed line, etc.) — untouched
- Horizontal lane mode — deferred, separate task
- Item-level scrolling within a section — already works, untouched
- Section filters, search, borders — all work through logical slot indices, untouched

## Verification

1. `cargo test --lib -p agenda-tui` — existing tests pass (windowing is render-only, doesn't affect test assertions on state)
2. Manual test: Month + Daily interval → 30+ slots should scroll smoothly with Tab
3. Manual test: Year + Monthly + Collapse → 12 slots, most collapsed, focused slot always visible
4. Manual test: Scroll position stable when items change (refresh doesn't jump)
