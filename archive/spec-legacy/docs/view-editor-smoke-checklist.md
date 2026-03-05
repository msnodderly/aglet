# View Editor Smoke Checklist (Split Picker + Full-Screen Editor)

Use this checklist after TUI changes to validate the streamlined view workflow.

## Setup

- Run: `cargo run --bin agenda-tui -- --db /Users/mds/src/aglet/aglet-features.ag`
- Open the quick view picker with `v`

## Quick Picker (Must Stay Lightweight)

- [ ] `j/k` moves selection
- [ ] `Enter` switches active view
- [ ] `e` opens full-screen editor for selected view
- [ ] `Esc` closes picker without switching

## Create View (Streamlined)

- [ ] In picker, press `N`
- [ ] Enter a view name and save
- [ ] Editor opens directly (no create-time category picker)
- [ ] First section exists automatically
- [ ] First section title is immediately in inline edit mode

## Full-Screen View Editor (Core Flow)

- [ ] `Tab` / `Shift-Tab` cycles panes (`Sections`, `Details`, optional `Preview`)
- [ ] `n` inserts section below current and starts title edit
- [ ] `N` inserts section above current and starts title edit
- [ ] `J/K` reorders selected section
- [ ] `x` prompts before deleting a section
- [ ] `r` on `View:` row starts view rename
- [ ] `Enter/Space` in `DETAILS` edits/toggles selected field

## Filter + Preview

- [ ] `/` opens section filter inline input
- [ ] Typing filters the left `SECTIONS` list
- [ ] `Esc` clears active section filter before trying to close editor
- [ ] `p` toggles preview pane
- [ ] Preview shows match count + lane summary
- [ ] Narrow terminal still renders without layout breakage

## Save / Cancel

- [ ] `S` saves and returns to quick picker
- [ ] `Esc` on dirty draft prompts `Discard unsaved changes? y/n`
- [ ] `n` / `Esc` from discard prompt keeps editing
- [ ] `y` discards and returns to picker
