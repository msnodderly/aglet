# TUI View + Category Workflow Spec

Date: 2026-02-17
Status: Proposed for implementation
Scope: TUI interaction model for view management, section layout, and category workflows.

## 1. Problem Statement

Current TUI behavior creates workflow friction:

- Section navigation is split into a dedicated left column instead of being a board-like section layout.
- View editing is limited to a single include category and does not support include/exclude composition.
- High-friction function-key entry points (`F8`, `F9`) are awkward on laptop keyboards.
- The unmatched/"Unassigned" section is over-prominent even when it adds little value.

This document defines the target interaction model for a streamlined daily loop.

## 2. Design Goals

- Keep "views are the interface" as the primary interaction model.
- Make section-based triage visually direct and cursor-driven.
- Support full query authoring for views (multi-include, multi-exclude, virtual filters).
- Keep category evolution fast without leaving context.
- Reduce keyboard friction on MacBooks while preserving compatibility.

## 3. Main Board Layout

### 3.1 Section Presentation

- Sections are rendered as horizontal bands across the board area.
- The board is the primary navigation surface; sections are not managed in a separate selector pane.
- Each section shows:
  - section title
  - item count
  - rows of items

### 3.2 Cursor Model

- Cursor movement is spatial in the board:
  - up/down: item navigation in current section
  - left/right: adjacent section navigation
- Add/edit/remove actions always apply to the currently focused section/item context.

## 4. Unmatched Section Policy

- Unmatched semantics remain enabled as a safety net.
- Items still appear in explicit sections OR unmatched, never both.
- Default presentation policy:
  - hide unmatched when empty
  - show unmatched when it has items
- View editor exposes unmatched options:
  - show when empty (pin)
  - hide when empty (default)
  - rename unmatched label

## 5. View Management Workflow

## 5.1 View Palette

The view palette supports:

- switch view
- create view
- rename view
- delete view
- open view editor

## 5.2 View Editor

View editor supports full criteria editing:

- include categories (multiple)
- exclude categories (multiple)
- virtual include buckets (e.g. `WhenBucket(today)`)
- virtual exclude buckets

Authoring behavior:

- category picker supports multi-select toggles
- include/exclude can be modified incrementally without resetting existing criteria
- live preview count shows expected matching items before save

## 5.3 Section Configuration

View editor supports explicit section definitions:

- add/remove/reorder sections
- edit section criteria (full query fields)
- edit `on_insert_assign` and `on_remove_unassign`
- toggle `show_children` where criteria is compatible

## 6. Category Workflow Integration

- Category Manager remains the place for structural edits (create, rename, reparent, toggles, delete).
- View editor and assignment flows provide inline category creation from search/matcher when no match exists.
- Newly created categories can be immediately inserted into include/exclude sets without leaving the current workflow.

## 7. Keyboard Shortcut Model

## 7.1 Primary Shortcuts (Laptop Friendly)

- `v`: open view palette
- `c`: open category manager
- `,`: previous view
- `.`: next view

## 7.2 Compatibility Aliases

- `F8` remains an alias for view palette.
- `F9` remains an alias for category manager.

## 7.3 Editor-Specific Shortcuts

Inside view editor:

- `+`: add/include category token
- `-`: add/exclude category token
- `s`: section editor
- `u`: unmatched settings
- `Enter`: save
- `Esc`: cancel

## 8. UX Language

- Use item-first phrasing consistently:
  - "assign item to category"
  - "remove item from view"
  - "insert item in section"
- Avoid implementation-oriented language in status text.

## 9. Acceptance Criteria

- Board displays sectioned content as horizontal section bands, not as a separate section list column.
- User can create and edit views with multi-include and multi-exclude criteria entirely inside TUI.
- Unmatched section behavior is configurable and non-intrusive by default.
- Primary view/category entry points are usable without function keys on Mac keyboards.
- Help/footer text reflects the new shortcut model and workflow terms.
