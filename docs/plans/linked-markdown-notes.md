---
title: Linked Markdown Notes
status: draft
created: 2026-04-12
---

# Linked Markdown Notes

## Context

Item notes are currently stored inline as `Option<String>` in SQLite. For
longer-form notes — meeting minutes, specs, checklists — users want to edit
in their preferred markdown editor and keep notes as portable files on disk.
This plan adds the ability for an item's note to be backed by an external
markdown file instead of the inline DB column.

## Design Decisions

| Decision | Choice |
|---|---|
| Scope | Items only (not Categories) |
| Link semantics | File **replaces** inline note; `note` column is NULL when `note_file` is set |
| File naming | `{slug}-{uuid8}.md` (e.g., `build-auth-middleware-a3f8b2c1.md`). Never renames. |
| Directory | Convention `<db-stem>-notes/` with optional `notes_dir` override in `app_settings` |
| Absolute paths | Not allowed — always relative to `.ag` file's parent directory |
| Attach existing | Supported via CLI `note link --file <path>` |
| Default mode | `app_settings` key `notes_default_linked` (`true`/`false`). Default: `false` (inline). |
| Unlink | **Not supported.** Once linked, always linked. One-time opt-in. |
| File lifecycle | Auto-create on link. **Delete file on item deletion.** |
| TUI editing | Load file content into TextBuffer, edit inline, write back on save |
| TUI control | Checkbox near Note field label — activates once (becomes read-only indicator after linking) |
| Board rendering | Same as inline notes — truncated preview, `♪` glyph |
| Recurrence | Successor inherits `note_file` (points to same file) |
| Search/classify | "Hydrate on load" — file content loaded into `item.note` at the TUI/CLI boundary |

---

## Phase 1: Core Data Model and Helpers

### 1a. Add `note_file` to Item struct

**File**: `crates/agenda-core/src/model.rs` (line 305)

Add after `note`:
```rust
pub note_file: Option<String>,
```

Update `Item::new()` (line 1258): `note_file: None`.

### 1b. Schema migration v18 → v19

**File**: `crates/agenda-core/src/store.rs`

- Bump `SCHEMA_VERSION` to 19 (line 23).
- Add `note_file TEXT` to `SCHEMA_SQL` items table (after line 45).
- Add migration in `apply_migrations()`:
  ```rust
  if !self.column_exists("items", "note_file")? {
      self.conn.execute_batch("ALTER TABLE items ADD COLUMN note_file TEXT;");
  }
  ```

### 1c. Update Store CRUD

**File**: `crates/agenda-core/src/store.rs`

| Function | Line | Change |
|---|---|---|
| `create_item` | 400 | Add `note_file` to INSERT columns and params |
| `update_item` | 483 | Add `note_file` to UPDATE SET clause and params |
| `get_item` | 462 | Add `note_file` to SELECT |
| `list_items` | 549 | Add `note_file` to SELECT |
| `row_to_item` | 1205 | Read new column (index 12) |
| `delete_item` | 508 | After deleting the DB row, delete the linked file from disk |

Note: `delete_item` gains a `db_path: &Path` parameter (or the caller
handles file deletion). Since core shouldn't do file I/O, the store returns
`note_file` value and the caller (CLI/TUI) deletes the file.

### 1d. New module: `note_file.rs`

**File**: `crates/agenda-core/src/note_file.rs` (new)

Pure functions — no file I/O in core:

```rust
pub const NOTES_DIR_SETTING_KEY: &str = "notes_dir";
pub const NOTES_DEFAULT_LINKED_KEY: &str = "notes_default_linked";

/// Convert text to filename-safe slug. Lowercase, non-alphanum → hyphens,
/// collapse runs, trim, truncate to 60 chars.
pub fn slugify(text: &str) -> String;

/// `{slug}-{first 8 hex chars of UUID}.md`
pub fn note_filename(text: &str, item_id: Uuid) -> String;

/// Resolve the notes directory path.
/// Override (from app_settings) is relative to db parent dir.
/// Default: `<db_stem>-notes/` sibling to the .ag file.
pub fn resolve_notes_dir(db_path: &Path, override_dir: Option<&str>) -> PathBuf;

/// Full path to a specific note file.
pub fn resolve_note_path(
    db_path: &Path, override_dir: Option<&str>, filename: &str,
) -> PathBuf;

/// Default content for a new linked note file.
pub fn note_template(item_text: &str) -> String;  // "# {item_text}\n\n"
```

Register in `crates/agenda-core/src/lib.rs`: `pub mod note_file;`

### 1e. Recurrence successor

**File**: `crates/agenda-core/src/agenda.rs` (line 799-812)

Copy `note_file` to successor:
```rust
note_file: completed.note_file.clone(),
```
Set `note: None` on the successor (file content hydrated at the boundary).

---

## Phase 2: CLI Commands

### 2a. New `Note` subcommand group

**File**: `crates/agenda-cli/src/main.rs`

```rust
/// Manage linked note files
Note {
    #[command(subcommand)]
    command: NoteCommand,
},
```

| Command | Description |
|---|---|
| `note link <item_id>` | Create linked note file. Moves inline note to file if present. |
| `note link <item_id> --file <path>` | Attach existing file (must be in notes dir or error). |
| `note path <item_id>` | Print resolved absolute path to linked note file. |
| `note edit <item_id>` | Open linked file in `$VISUAL` / `$EDITOR`. |

### 2b. Note hydration helper

Shared utility for both CLI and TUI — reads file content into `item.note`
after loading from DB:

```rust
fn hydrate_note_files(
    items: &mut [Item], db_path: &Path, notes_dir_override: Option<&str>,
) {
    for item in items.iter_mut() {
        if let Some(ref filename) = item.note_file {
            let path = note_file::resolve_note_path(
                db_path, notes_dir_override, filename,
            );
            if let Ok(content) = std::fs::read_to_string(&path) {
                item.note = Some(content);
            }
        }
    }
}
```

Call in all CLI commands that display items: `show`, `list`, `search`, `export`.

### 2c. Update existing CLI commands

- **`cmd_edit`**: When item has `note_file`, `--note`/`--append-note`/`--note-stdin`
  write to the file instead of the DB column. `--clear-note` clears file content.
- **`cmd_show`**: Print `Note file: <filename>` when `note_file` is set.
- **`cmd_add`**: When `notes_default_linked` is `true` and `--note` is provided,
  auto-create a linked file after the item is persisted (need the item ID first).

### 2d. File deletion on item delete

In the CLI `delete` command handler, after `agenda.delete_item()` returns, check
if the deleted item had `note_file` and delete the file from disk.

### 2e. Settings

Use existing `app_settings` API. Add a `config` subcommand if one doesn't exist:

- `agenda-cli config set notes_dir "../shared-notes"`
- `agenda-cli config set notes_default_linked true`
- `agenda-cli config get notes_dir`

---

## Phase 3: TUI

### 3a. Store `db_path` in App

**File**: `crates/agenda-tui/src/app.rs`

Add `db_path: PathBuf` field. Set from `run_with_options()` (`lib.rs:167`)
which already receives `db_path: &Path`.

### 3b. Hydrate on refresh

In `App::refresh()`, after loading items from the store, call
`hydrate_note_files()` on `self.all_items`. Cache `notes_dir_override` from
`store.get_app_setting(NOTES_DIR_SETTING_KEY)`.

### 3c. Linked-note checkbox in InputPanel

**File**: `crates/agenda-tui/src/input_panel.rs`

Add to `InputPanel`:
```rust
pub(crate) link_note: bool,          // user checked the "link to file" box
pub(crate) is_already_linked: bool,  // item already has note_file (read-only)
```

**UI rendering** (`crates/agenda-tui/src/render/mod.rs`):

The Note section border/label shows the link state:

- Not linked, checkbox available: `── Note [  ] Link to file ──`
- Not linked, checkbox checked: `── Note [x] Link to file ──`
- Already linked (read-only): `── Note (→ build-auth-a3f8b2c1.md) ──`

The checkbox is togglable via Space when Note focus is active and
`is_already_linked` is false. Once the item is saved with `link_note = true`,
subsequent edits show the read-only indicator.

When `notes_default_linked` is `true`, `link_note` defaults to `true` for
new items (AddItem kind).

### 3d. Save-path branching

**File**: `crates/agenda-tui/src/modes/board.rs`

**`save_input_panel_add()`** (line 4812):
After the item is created and has an ID:
- If `panel.link_note` is true and note is non-empty:
  compute filename, create notes dir, write content to file,
  set `item.note_file`, clear `item.note`, update item in DB.

**`save_input_panel_edit()`** (line 4953):
- If `item.note_file.is_some()`: write note buffer content to the linked
  file, set `item.note = None` before persisting to DB.
- Otherwise: existing inline behavior.

### 3e. File deletion on item delete

In the TUI delete-item handler, after `agenda.delete_item()`, delete the
linked file from disk if `note_file` was set.

### 3f. Footer hints

When Note focus is active and `is_already_linked` is false, show
`Space: toggle link-to-file` in the hint bar.

### 3g. Preview mode (P)

No changes needed — preview mode reads `item.note` which is hydrated from
the file. Works as-is.

---

## Phase 4: Tests and Verification

### Unit tests (agenda-core)

- `slugify`: empty, unicode, long strings, special chars, consecutive hyphens
- `note_filename`: format `{slug}-{8hex}.md`
- `resolve_notes_dir`: default convention, override, relative resolution
- Migration: v18 DB opens, `note_file` column exists
- CRUD roundtrip: item with `note_file` set, `note` NULL

### Integration tests

- CLI `note link` + `show`: file created, content displayed
- CLI `note link --file`: existing file attached
- CLI `note edit`: opens editor
- CLI `edit --note` with linked item: file content updated
- CLI `search`: matches text inside linked note files
- CLI delete of linked item: file removed from disk

### Manual TUI verification

1. Create item with inline note → works as before
2. Create item, check "Link to file" box, add note → file created in
   `<db>-notes/`, note label shows filename
3. Edit linked item → note loads from file, saves back to file
4. P (preview) on linked item → note content displays
5. Delete linked item → file deleted from disk
6. Set `notes_default_linked = true` → new items have checkbox pre-checked
7. Recurring item with linked note → successor points to same file
8. `note link --file existing.md` via CLI → TUI shows read-only indicator

---

## Files to Modify

| File | Change |
|---|---|
| `crates/agenda-core/src/model.rs` | Add `note_file` field to `Item` |
| `crates/agenda-core/src/store.rs` | Migration v19, CRUD updates |
| `crates/agenda-core/src/note_file.rs` | **New** — slug, path, template helpers |
| `crates/agenda-core/src/lib.rs` | Register `note_file` module |
| `crates/agenda-core/src/agenda.rs` | Recurrence successor copies `note_file` |
| `crates/agenda-cli/src/main.rs` | `Note` subcommand group, hydration, edit/show updates, delete cleanup |
| `crates/agenda-tui/src/app.rs` | `db_path` field, hydration in refresh |
| `crates/agenda-tui/src/lib.rs` | Pass `db_path` to App |
| `crates/agenda-tui/src/input_panel.rs` | `link_note` / `is_already_linked` fields, checkbox logic |
| `crates/agenda-tui/src/modes/board.rs` | Save branching, file creation, delete cleanup |
| `crates/agenda-tui/src/render/mod.rs` | Note label with link state, footer hints |
