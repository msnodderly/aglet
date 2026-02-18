# TUI Smoke Test Script (E2E Daily Loop)

Date: 2026-02-16
Task: T016 (`bd-pmr`)

This script validates the TUI daily loop across:
- add
- move (`[` / `]`) between sections
- remove from view (`r`)
- done (`d`)
- delete (`x`)
- inline text edit (`e`)
- note create/edit (`m`)
- preview provenance unassign (`p` + `o` + `Tab` + `u`)
- category manager create/rename/reparent/toggle/delete (`c` / `F9`)
- full view manager criteria/section/unmatched flows (`v` + `V`)

## 1) Setup

```bash
cd /Users/mds/src/aglet
DB="/tmp/aglet-tui-smoke-$(date +%s).ag"
echo "$DB"
```

Create categories and views used in the smoke run:

```bash
cargo run -q -p agenda-cli -- --db "$DB" category create SlotA
cargo run -q -p agenda-cli -- --db "$DB" category create SlotB
cargo run -q -p agenda-cli -- --db "$DB" view create "Smoke Board"
```

Configure `Smoke Board` with two explicit sections and remove-from-view behavior.
This is needed so `[` / `]` and `r` are meaningfully testable in TUI.

```bash
A=$(sqlite3 "$DB" "select id from categories where lower(name)='slota';")
B=$(sqlite3 "$DB" "select id from categories where lower(name)='slotb';")
SECTIONS=$(cat <<JSON
[{"title":"Slot A","criteria":{"include":["$A"],"exclude":[],"virtual_include":[],"virtual_exclude":[],"text_search":null},"on_insert_assign":["$A"],"on_remove_unassign":["$A"],"show_children":false},{"title":"Slot B","criteria":{"include":["$B"],"exclude":[],"virtual_include":[],"virtual_exclude":[],"text_search":null},"on_insert_assign":["$B"],"on_remove_unassign":["$B"],"show_children":false}]
JSON
)
CRITERIA=$(cat <<JSON
{"include":["$A","$B"],"exclude":[],"virtual_include":[],"virtual_exclude":[],"text_search":null}
JSON
)
REMOVE=$(cat <<JSON
["$A","$B"]
JSON
)
sqlite3 "$DB" "update views set criteria_json='$CRITERIA', sections_json='$SECTIONS', remove_from_view_unassign_json='$REMOVE' where name='Smoke Board';"
```

## 2) Run TUI Smoke Flow

Launch:

```bash
cargo run -q -p agenda-tui -- --db "$DB"
```

Inside TUI, run the following checklist.

1. Category manager and structural edits (`c` / `F9`):
- Press `c`.
- Press `N`, create `Work`.
- Select `Work`, press `n`, create `Project X`.
- Select `Project X`, press `r`, rename to `Project X2`.
- With `Project X2` selected, press `t` (toggle exclusive) and `i` (toggle implicit).
- Press `p`, choose `(root)`, Enter (reparent).
- Press `Esc` (or `F9`) to close manager.

2. View create/edit flow (`v` / `F8` + full editor + manager):
- Press `v`.
- Press `N`, type `Work Focus`, Enter.
- In include-category picker, select `Work`, Enter.
- Press `v`, select `Work Focus`, press `r`, rename to `Work Board`, Enter.
- Press `v`, press `N`, type `Temp Delete`, Enter; in include picker press Enter.
- Select `Temp Delete`, press `x`, then `n` (cancel) and verify view remains.
- Press `x`, then `y` and verify `Temp Delete` is removed.
- Press `v`, select `Work Board`, press `V` to open View Manager.
  - Press `Tab` to move to Definition pane.
  - Press `N` to add a row.
  - Press `o` (set row join OR), then press `s`.
    - Expected: save is rejected with `Cannot save criteria...`.
  - Press `a` (switch back to AND), then press `s`.
    - Expected: save succeeds.
  - Press `Tab` to move to Sections pane.
  - Press `N` to add section, then `]` and `[` to reorder.
  - Press `u` for unmatched settings, `t` to toggle unmatched, `Esc` back.
  - Press `s` to persist manager changes.
  - Press `Esc` to return to view palette.
- Press `v`, select `Work Board`, press `e` to open view editor.
  - Press `+`, toggle `Project X2` with Space, press Enter.
  - Press `-`, toggle `Done` with Space (or another category), press Enter.
  - Press `]`, toggle `Today` bucket with Space, press Enter.
  - Press `s` to open section editor.
    - Press `N` to add section.
    - Press `Enter` to open section detail.
    - Press `t`, rename section to `Focus`, Enter.
    - Press `+`, add at least one include category, Enter.
    - Press `Esc` back to section list, `Esc` back to view editor.
  - Press `u` to open unmatched settings.
    - Press `l`, set label to `Backlog`, Enter.
    - Press `Esc` back to view editor.
  - Press `Enter` to save view editor.

3. Sectioned move/remove flow:
- Press `v`, switch to `Smoke Board`.
- Ensure `Slot A` section is selected, press `n`, add: `smoke flow item`.
- Press `]` to move to `Slot B`.
- Press `r` to remove from view.
  - Expected: item disappears from `Smoke Board`.

4. Edit/note/preview-unassign flow:
- Press `v`, switch to `All Items`.
- Select any non-done item.
- Press `e`, append ` Foo`, Enter.
- Press `m`, type `smoke note`, Enter.
- Press `p` to open preview pane.
- Press `o` to switch to provenance mode.
- Press `Tab` to focus preview pane.
- Press `u`, choose assignment, Enter to unassign one category.

5. Done/delete flow:
- With selected item, press `d`.
- Press `x`, then `y` to confirm delete.
- Press `q` to exit.

## 3) Verification Commands

```bash
# Category structure/toggles changed by TUI manager
cargo run -q -p agenda-cli -- --db "$DB" category list

# View create/edit persisted
cargo run -q -p agenda-cli -- --db "$DB" view list

# Note edit visible via search
cargo run -q -p agenda-cli -- --db "$DB" search "smoke note"

# Smoke board should not include removed item anymore
cargo run -q -p agenda-cli -- --db "$DB" view show "Smoke Board"

# Deleted log contains the deleted item
cargo run -q -p agenda-cli -- --db "$DB" deleted
```

## 4) Pass Criteria

- TUI remains stable through the full sequence (no crash/forced exit).
- Category create/rename/reparent/toggle operations succeed and persist.
- View create/rename/full-editor operations succeed and persist.
- View manager save gate blocks non-representable criteria (`OR`, nesting) and allows representable saves.
- View manager sections pane can add/remove/reorder sections and edit unmatched settings, then persist with `s`.
- View delete/cancel flow behaves correctly (`x` then `n`/`y`).
- Item can be moved between `Slot A`/`Slot B` and removed from `Smoke Board`.
- Inline text edit and note edit persist.
- Preview provenance unassign removes selected assignment.
- Done + delete flow succeeds and appears in deletion log.
