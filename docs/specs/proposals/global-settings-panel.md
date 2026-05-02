---
title: Global Settings Panel
status: draft
created: 2026-03-15
---

# Global Settings Panel

Date: 2026-03-22
Status: Proposal

## Problem

Global behavioral settings are currently scattered across three different access points with inconsistent UX:

| Setting | Current location | Access |
|---|---|---|
| Auto-refresh interval | Normal mode | `Ctrl+R` cycles Off → 1s → 5s; no direct edit |
| Auto-categorization mode | Category Manager — Global Settings pane | `m` key opens a picker overlay |
| Classification providers | `ClassificationConfig` in store | No UI at all |
| Workflow role assignments | Category Manager — Global Settings pane | `w` key opens a workflow setup overlay |

The Category Manager already has a "Global Settings pane" containing classification mode and workflow roles, but:
- It is buried inside a category-management context
- It makes the Category Manager feel like two tools in one (P4 in the category-manager-ux-improvements proposal)
- Auto-refresh lives somewhere else entirely
- Classification provider toggles have no UI surface at all
- There is no obvious home for future global options (LLM provider config, etc.)

## Proposal

Add a dedicated `GlobalSettings` mode, opened from the main board (`Mode::Normal`) via a single key. Move the current user-facing app-scoped settings there. Remove the Global Settings pane from the Category Manager (or reduce it to a read-only summary line with a "press X to open settings" hint, per S4 in category-manager-ux-improvements).

---

## UI Mockup

The mockup below is illustrative, not prescriptive. Exact layout, copy, and row styling can change during implementation as long as the core interaction model stays simple and the first iteration remains narrowly scoped.

```
╔═ Global Settings ══════════════════════════════════════════╗
║                                                            ║
║  General                                                   ║
║  ─────────────────────────────────────────────────────    ║
║  Auto-refresh        ◀ Off ▶                              ║
║                                                            ║
║  Auto-categorization                                       ║
║  ─────────────────────────────────────────────────────    ║
║  Mode                ◀ Auto-apply ▶                       ║
║                                                            ║
║  Workflow                                                  ║
║  ─────────────────────────────────────────────────────    ║
║  Ready category      Ready                                 ║
║  Claim category      In Progress                          ║
║                                                            ║
║  j/k:move  Space/←→:cycle  Enter:pick  Esc:close         ║
╚════════════════════════════════════════════════════════════╝
```

**Navigation:**
- `j`/`k` or arrow keys: move focus between rows
- `Space` or `←`/`→`: cycle values for enum/toggle settings
- `Enter`: open a picker for workflow category assignments; also works as Space for other rows
- `Esc`: close, returning to `Mode::Normal`

**Inline cycling examples:**

Auto-refresh cycles: `Off` → `1s` → `5s` → `Off`

Mode cycles: `Off` → `Auto-apply` → `Suggest/Review` → `Off`

Workflow category rows open the existing workflow-role category picker semantics inline: only eligible tag categories are shown, cross-role conflicts are prevented, and assignment side-effects remain unchanged.

---

## Settings Included

### General

**Auto-refresh interval** — currently an app setting (`tui.auto_refresh_interval`). No data model change needed. Storage unchanged.

### Auto-categorization

**Mode** (`ClassificationConfig.continuous_mode`) — currently only accessible via the Category Manager picker. Values: Off, Auto-apply, Suggest/Review.

### Workflow

**Ready category** (`WorkflowConfig.ready_category_id`) — currently only accessible via the Category Manager workflow setup overlay.

**Claim category** (`WorkflowConfig.claim_category_id`) — same.

Both open the existing workflow-role picker behavior for assignment. The workflow-role side-effects (disabling auto-match on the assigned category, warning about other derived sources, preventing cross-role conflicts) remain unchanged.

---

## Settings NOT Included

These were considered and excluded:

| Setting | Reason excluded |
|---|---|
| `run_on_item_save` / `run_on_category_change` | Implementation details; not meaningful to expose |
| `tui.last_view_name` | Auto-persisted silently; not a user-facing setting |
| `show_preview`, `preview_mode`, `normal_focus` | Session navigation state, not behavioral config |
| `$EDITOR` / `$VISUAL` | Environment variable conventions; TUI should not override |
| `ClassificationConfig.enabled_providers` | Defer until provider enable/mode settings are actually enforced by runtime classification behavior |
| View-level settings (layout, display mode, columns) | Per-view, correctly scoped in the view editor |
| Category-level settings (flags, auto-match, etc.) | Per-category, correctly scoped in the category manager |

### Future Follow-up

When LLM-backed classification work begins, revisit this screen for provider-specific controls. That follow-up should:
- define runtime semantics for provider `enabled` and execution `mode`
- decide whether provider rows are dynamically generated or explicitly modeled
- add provider rows only once the backend behavior and copy are real, not aspirational

---

## Changes to Existing UI

### Category Manager

Remove the Global Settings pane (the 5-row bordered block at the top). Replace with a single read-only summary line above the filter:

```
Classification: Auto-apply  │  Ready: Ready  │  Claim: In Progress
```

This aligns with proposal S4 in category-manager-ux-improvements and reclaims 4 vertical rows for the category tree.

Remove `m` and `w` from Category Manager entirely. Users who want to change classification mode or workflow roles open GlobalSettings from Normal mode via `g s` or `F10`, then return to Category Manager if needed. The summary line (see S4 in category-manager-ux-improvements) remains as read-only context.

### Normal Mode

Remove the `Ctrl+R` auto-refresh cycling shortcut. Auto-refresh is configured exclusively via GlobalSettings.

Add the GlobalSettings key binding to the Normal mode footer hint.

---

## Key Binding

Primary: `g` `s` (two-key sequence in Normal mode, vim-style) — mnemonic for "global settings".

Secondary: `F10` — also opens GlobalSettings for discoverability and single-key access.

**Historical note:** In Lotus Agenda, `F10` was the general-purpose menu key (equivalent to a menu bar), not a settings shortcut. Global settings in Aglet lived at `F10 → File → Properties → Auto-assign settings` — two levels deep. There is no Aglet precedent for `F10` as a direct settings destination, so using it as a secondary convenience binding here is fine.

---

## Implementation

**New mode:** Add `Mode::GlobalSettings` to the `Mode` enum in `lib.rs`.

**New state struct (v1):**
```rust
struct GlobalSettingsState {
    selected_row: usize,
}

enum GlobalSettingsRow {
    AutoRefresh,
    ClassificationMode,
    WorkflowReady,
    WorkflowClaim,
}
```

**New file:** `crates/aglet-tui/src/modes/global_settings.rs` — key handling and render logic.

**Storage:** No schema changes. All settings continue to use their existing persistence:
- `store.set_app_setting("tui.auto_refresh_interval", ...)`
- `store.set_classification_config(&config)`
- `store.set_workflow_config(&config)`

**Entry point:** Open only from `Mode::Normal` (`g s` and `F10` in v1). Do not make this a globally available overlay over arbitrary editing modes.

**Load on entry:** Read all three configs from the store when GlobalSettings opens (same data already loaded at startup; this is a refresh to catch any external changes).

**Save on change:** Write immediately on each value change (same pattern as current behavior).

**Workflow category picker:** When the user presses `Enter` on a workflow row, reuse the existing workflow-role picker rules rather than the generic category column picker. On selection, the picker closes and GlobalSettings remains open with the updated value.

**Suggested implementation order:**
1. Add `Mode::GlobalSettings` plus a compact render/input path for the four v1 rows.
2. Wire `g s` and `F10` from `Mode::Normal`, and add footer/help discoverability there.
3. Remove `Ctrl+R` cycling from Normal mode once Auto-refresh is editable in GlobalSettings.
4. Remove Category Manager `m`/`w` edit flows and replace the old pane with the read-only summary line.
5. Leave provider controls for a future proposal tied to LLM/provider backend work.
