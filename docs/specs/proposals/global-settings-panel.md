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

Add a dedicated `GlobalSettings` mode, accessible from anywhere via a single key. Move all app-scoped settings there. Remove the Global Settings pane from the Category Manager (or reduce it to a read-only summary line with a "press X to open settings" hint, per S4 in category-manager-ux-improvements).

---

## UI Mockup

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
║  Providers                                                 ║
║    Implicit string   [x] enabled   ◀ Inline ▶             ║
║    Date/time parser  [x] enabled   ◀ Inline ▶             ║
║    LLM               [ ] enabled   ◀ Background ▶  (soon) ║
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
- `Esc`: close, returning to whichever mode was active before

**Inline cycling examples:**

Auto-refresh cycles: `Off` → `1s` → `5s` → `Off`

Mode cycles: `Off` → `Auto-apply` → `Suggest/Review` → `Off`

Provider mode cycles: `Inline` → `Background`

Workflow category rows open the existing category column picker inline (same picker reused from other category-assignment contexts).

---

## Settings Included

### General

**Auto-refresh interval** — currently an app setting (`tui.auto_refresh_interval`). No data model change needed. Storage unchanged.

### Auto-categorization

**Mode** (`ClassificationConfig.continuous_mode`) — currently only accessible via the Category Manager picker. Values: Off, Auto-apply, Suggest/Review.

**Providers** (`ClassificationConfig.enabled_providers`) — currently no UI.

Each provider row shows:
- Provider name (human-readable: "Implicit string", "Date/time parser", future: "LLM")
- Enabled toggle (`[x]` / `[ ]`)
- Mode selector (`Inline` / `Background`) — only relevant when enabled

The `enabled_providers` list is already persisted in `ClassificationConfig`. No data model change needed. Provider rows are rendered dynamically from the list, so adding an LLM provider in the future requires no UI code changes — it will appear automatically.

### Workflow

**Ready category** (`WorkflowConfig.ready_category_id`) — currently only accessible via the Category Manager workflow setup overlay.

**Claim category** (`WorkflowConfig.claim_category_id`) — same.

Both open the existing category column picker for assignment. The workflow-role side-effects (disabling auto-match on the assigned category) remain unchanged.

---

## Settings NOT Included

These were considered and excluded:

| Setting | Reason excluded |
|---|---|
| `run_on_item_save` / `run_on_category_change` | Implementation details; not meaningful to expose |
| `tui.last_view_name` | Auto-persisted silently; not a user-facing setting |
| `show_preview`, `preview_mode`, `normal_focus` | Session navigation state, not behavioral config |
| `$EDITOR` / `$VISUAL` | Environment variable conventions; TUI should not override |
| View-level settings (layout, display mode, columns) | Per-view, correctly scoped in the view editor |
| Category-level settings (flags, auto-match, etc.) | Per-category, correctly scoped in the category manager |

---

## Changes to Existing UI

### Category Manager

Remove the Global Settings pane (the 5-row bordered block at the top). Replace with a single read-only summary line above the filter:

```
Classification: Auto-apply  │  Ready: Ready  │  Claim: In Progress
```

This aligns with proposal S4 in category-manager-ux-improvements and reclaims 4 vertical rows for the category tree.

The `m` (classification mode) and `w` (workflow setup) keybindings in Category Manager can either:
- **Option A**: Open GlobalSettings directly, focused on the relevant row (preferred — single source of truth)
- **Option B**: Continue to open the current picker overlays (less churn, worse discoverability)

### Normal Mode

Remove the `Ctrl+R` auto-refresh cycling shortcut, or keep it as a convenience alias. The status bar label currently showing the refresh interval remains.

Add the GlobalSettings key binding to the Normal mode footer hint.

---

## Key Binding

`F10` — opens GlobalSettings from any mode that supports it (Normal, CategoryManager, ViewEdit).

Rationale: `F10` is a conventional "application menu" key in TUI applications (midnight commander, etc.), making it discoverable. It doesn't conflict with any current binding.

Alternative: `?` — mnemonic for "settings/help", but may conflict with a future help screen.

---

## Implementation

**New mode:** Add `Mode::GlobalSettings` to the `Mode` enum in `lib.rs`.

**New state struct:**
```rust
struct GlobalSettingsState {
    focus: GlobalSettingsFocus,
}

enum GlobalSettingsFocus {
    AutoRefresh,
    ClassificationMode,
    ProviderImplicitStringEnabled,
    ProviderImplicitStringMode,
    ProviderWhenParserEnabled,
    ProviderWhenParserMode,
    WorkflowReady,
    WorkflowClaim,
}
```

**New file:** `crates/agenda-tui/src/modes/global_settings.rs` — key handling and render logic.

**Storage:** No changes. All settings continue to use their existing persistence:
- `store.set_app_setting("tui.auto_refresh_interval", ...)`
- `store.set_classification_config(&config)`
- `store.set_workflow_config(&config)`

**Load on entry:** Read all three configs from the store when GlobalSettings opens (same data already loaded at startup; this is a refresh to catch any external changes).

**Save on change:** Write immediately on each value change (same pattern as current behavior).

---

## Open Questions

1. Should `Ctrl+R` be kept as a shortcut alias after GlobalSettings exists? Convenience vs. discoverability tension.
2. For workflow category assignment, should the category picker open inline within the settings panel, or as a separate overlay above it?
3. Should GlobalSettings be accessible from CategoryManager via `m`/`w` redirect (Option A above) or keep the existing overlays (Option B)?
