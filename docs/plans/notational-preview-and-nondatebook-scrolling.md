---
title: NV-Style Inline Note Preview + Non-Datebook Board Scrolling
status: active
created: 2026-04-12
updated: 2026-04-12
---

# Plan: NV-Style Inline Note Preview + Non-Datebook Board Scrolling

## Context

Current TUI preview mode is a secondary inspector-style panel (`Summary` / `Info`) that includes metadata, IDs, suggestions, categories, and note content. This is useful for debugging but does not match Notational Velocity / nvALT / Neo-NV workflows where the lower pane is primarily a full note editor.

Datebook views recently gained vertical board scrolling so sections can exceed viewport height. Standard (non-datebook) sectioned board views still assume all sections fit vertically, which will become more problematic once we allocate larger persistent space to note editing.

## UX Target

When preview is open in Normal mode:

1. Bottom pane behaves as a **note-first editor** for the currently selected item.
2. Pane title and footer communicate explicit mode/state, e.g. `Preview: Note` + `Enter edit | S save | Esc cancel`.
3. Metadata moves to a secondary “info” mode rather than sharing primary space.
4. Cursor/focus rules stay consistent with existing board/preview focus model.

This should feel similar to NV/nvALT split: list/table on top, full editable note below.

## Proposed Interaction Model

### Preview modes

Replace/extend current preview modes:

- `Note` (default when opening preview)
  - Displays full note body (or empty placeholder)
  - Supports read-only scroll when unfocused or when not editing
  - Supports inline edit session when focused + edit command is triggered
- `Info`
  - Existing provenance/summary diagnostics

`p` toggles pane visibility, `i/o` (or simplified single key) cycles Note/Info.

### Edit session in Note mode

- Entering edit:
  - Key: `Enter` when preview has focus in `Note` mode (or dedicated `e` while focused)
  - Initializes textarea buffer from selected item note
- While editing:
  - Typed characters mutate local draft buffer
  - Navigation/editing keys go to textarea
  - Board movement keys are suspended
- Save/cancel:
  - `S` save note (matching existing complex-editor convention)
  - `Esc` cancel edit (with the same dirty-confirm pattern used by other editors: `y` save, `n` discard, `Esc` keep editing)
  - On save, persist via existing agenda edit/update path, then refresh projections

This mirrors existing InputPanel/editor semantics where possible (notably `S` for save and `Esc` for cancel) to reduce cognitive/implementation load.

## Rendering/Data-Flow Design

### 1) Add preview note editor state

Add dedicated state for preview editing (draft text, editing flag, cursor, viewport scroll). Keep this state owned by app-level Normal mode so it survives render passes but resets appropriately on item change/view change.

Suggested fields:

- `preview_note_editing: bool`
- `preview_note_dirty: bool`
- `preview_note_item_id: Option<ItemId>`
- `preview_note_editor: TextArea` (or shared wrapper used by InputPanel note)
- `preview_note_scroll: usize`

### 2) Selection synchronization

When selected item changes and preview is **not editing**, rehydrate note buffer from item note.
When selected item changes and preview **is editing**, either:

- block navigation with status hint, or
- allow navigation and prompt to save/discard

Recommendation: block by default while editing to avoid accidental context switch data loss.

### 3) Render note pane

In `Note` mode:

- Render border/title with focused/editing cues.
- Render full note text area in pane body.
- Render vertical scrollbar derived from wrapped lines and cursor position.
- Reuse explicit cursor placement patterns already used in InputPanel note editing so caret is always visible.

### 4) Save path

Persist through same mutation path as item edit note updates, then trigger item/category recomputation as already done by edit flows.

## Non-Datebook Board Scrolling Alignment

Generalize recent datebook board vertical scrolling so all vertical-flow sectioned views can exceed viewport height:

1. Move slot-height estimation + visible-window logic to a reusable helper.
2. Enable windowed rendering for non-datebook section views (respecting empty-section mode and collapsed rows).
3. Keep horizontal-flow behavior unchanged for this phase.
4. Ensure focused slot auto-scroll remains stable when preview height changes.

Key requirement: changing preview size or mode must recompute board viewport and preserve slot/item visibility.

## Incremental Delivery

### Phase 1: Note-first preview (read-only)

- Add `PreviewMode::Note`
- Render full note body only
- Keep existing summary/info accessible
- Preserve existing preview scroll controls

### Phase 2: Inline note editing

- Add editor state + cursor
- Enter/edit/save/cancel controls
- Dirty guardrails on focus/navigation changes

### Phase 3: Non-datebook board vertical scrolling

- Reuse datebook scrolling helpers for standard views
- Add viewport scrollbar at board level where needed
- Validate with large multi-section views and preview open/closed transitions

## Validation Checklist

- Preview open on item with long note: full note visible and scrollable.
- Focus preview, enter edit, type/save, and verify item note persisted.
- Dirty cancel path works and does not silently discard text.
- Switching between Note and Info modes preserves expected scroll/cursor state.
- Non-datebook view with many sections remains navigable with preview open.
- Datebook scrolling behavior remains unchanged.

## Risks / Open Questions

1. **Keybinding collisions in focused preview edit mode**
   - Need clear precedence between text editing keys and Normal-mode navigation keys.
2. **Textarea reuse vs bespoke editor widget**
   - Reuse reduces bugs but may require adapter for existing InputPanel assumptions.
3. **Slot height estimation accuracy in multiline rows**
   - Existing approximation may cause slight scrollbar imprecision; acceptable initially.
4. **Autosave policy**
   - Keep explicit save in v1 to avoid accidental writes while moving around list.

## Out of Scope (for this plan)

- External file linking/editing parity with Neo-NV
- Rich text/Markdown formatting UI
- Multi-item split comparison editing
