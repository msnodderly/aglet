# Plan: Per-Section Text Filters

**Spec:** `docs/specs/proposals/tui-ux-redesign.md` §10
**Status:** Spec complete, implementation pending.

## Context

The TUI redesign (§10) extends the single view-wide text filter (`filter: Option<String>`) into per-section filters. Each rendered section has its own independent text search. `/` from Normal mode opens FilterInput scoped to the current section; `Esc` from Normal clears that section's filter only.

This is a prerequisite for or concurrent with the Phase 3 Esc-consistency fixes in the implementation sequence (now renumbered Phase 4).

## Files to Modify

| File | Change |
|------|--------|
| `crates/agenda-tui/src/lib.rs` | Replace `filter: Option<String>` with `section_filters: Vec<Option<String>>`, add `filter_target_section: usize` |
| `crates/agenda-tui/src/modes/board.rs` | `handle_filter_key`: store into `section_filters[filter_target_section]`; `/` sets target before entering FilterInput; `Esc` in Normal clears current section's filter |
| `crates/agenda-tui/src/app.rs` | `rebuild_section_filters()` called on view switch and board layout change; text filter application per-section |
| `crates/agenda-tui/src/render/mod.rs` | Section header: append `  filter:<needle>` when section has active filter; count reflects post-filter count |
| `crates/agenda-tui/src/input/mod.rs` | FilterInput Enter: `self.section_filters[self.filter_target_section] = value` |

## Implementation Steps

### 1. State change (`lib.rs`)
```rust
// Before:
filter: Option<String>,

// After:
section_filters: Vec<Option<String>>,
filter_target_section: usize,
```
Initialize `section_filters` as empty vec; `filter_target_section: 0`.

### 2. `rebuild_section_filters()` (`app.rs`)
Called whenever the rendered section count changes (view switch, board recompute). Resizes `section_filters` to match rendered section count, preserving existing values by index where possible, zeroing new slots.

```rust
fn rebuild_section_filters(&mut self, new_len: usize) {
    self.section_filters.resize(new_len, None);
    self.filter_target_section = self.filter_target_section.min(new_len.saturating_sub(1));
}
```

### 3. Text filter application (`app.rs`)
Currently the filter needle is applied once across all items. Change to apply per-section:

```rust
// When building rendered items for section i:
let needle = self.section_filters.get(i).and_then(|f| f.as_deref());
let displayed = section_items.iter().filter(|item| {
    needle.map_or(true, |n| item.text.to_ascii_lowercase().contains(n))
});
```

### 4. FilterInput entry (`modes/board.rs`)
Before entering FilterInput, capture the current section:
```rust
self.filter_target_section = self.current_section_index();
self.set_input(
    self.section_filters
        .get(self.filter_target_section)
        .and_then(|f| f.clone())
        .unwrap_or_default()
);
```

### 5. FilterInput confirm (`input/mod.rs`)
```rust
// Enter:
let value = self.input.trimmed().to_string();
self.section_filters[self.filter_target_section] =
    if value.is_empty() { None } else { Some(value) };
self.mode = Mode::Normal;

// Esc:
// do nothing — preserve section_filters[filter_target_section]
self.mode = Mode::Normal;
```

### 6. Esc in Normal (`modes/board.rs`)
```rust
KeyCode::Esc => {
    let idx = self.current_section_index();
    if let Some(f) = self.section_filters.get_mut(idx) {
        if f.is_some() {
            *f = None;
            self.status = "Filter cleared".into();
            return;
        }
    }
    // no-op if already clear
}
```

### 7. Section header rendering (`render/mod.rs`)
When rendering a section header at index `i`:
```rust
let filter_tag = match self.section_filters.get(i).and_then(|f| f.as_deref()) {
    Some(needle) => format!("  filter:{needle}"),
    None => String::new(),
};
let header = format!("{} ({count}){filter_tag}", section.title);
```

Style `filter:<needle>` with a dim/italic style to distinguish it from the title.

## Verification

1. `cargo test -p agenda-tui` — all existing tests pass.
2. Manual: start TUI, navigate to a view with 2+ sections, press `/`, type a word — only the current section filters. Items in other sections unaffected.
3. `Tab` to another section, press `/` — independent filter for that section.
4. `Esc` from Normal clears focused section's filter; other sections unaffected.
5. Switch view and return — `section_filters` resets cleanly (no index out of bounds).
