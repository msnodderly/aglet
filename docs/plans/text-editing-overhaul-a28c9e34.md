# Execute Plan: Save + Implement `a28c9e34`

## Summary
- Save the approved plan to `./plans`.
- Create the implementation worktree/branch.
- Migrate text editing to `tui-textarea-2` + `ratatui 0.30`.
- Remove obsolete code introduced by the old text system.
- Verify with fmt/clippy/tests.

## Execution Steps
1. Create plan file in main repo:
- Path: `/Users/mds/src/aglet/docs/plans/text-editing-overhaul-a28c9e34.md`
- Content: the full approved implementation plan (worktree, migration, dead-code cleanup, test gates).

2. Create implementation worktree:
- `git worktree add /Users/mds/src/aglet-a28c9e34-textarea2 -b codex/a28c9e34-textarea2-overhaul`

3. Implement in `/Users/mds/src/aglet-a28c9e34-textarea2`:
- Update dependencies in `/Users/mds/src/aglet-a28c9e34-textarea2/crates/agenda-tui/Cargo.toml`.
- Refactor key pipeline to pass `KeyEvent` to text handlers.
- Replace legacy text editing core with persistent `tui-textarea-2` state.
- Migrate all text-entry surfaces (InputPanel, NoteEdit, filters, view-edit inline inputs, category-manager note/filter, etc.).
- Enable multiline soft-wrap (`WrapMode::WordOrGlyph`).
- Preserve existing mode semantics (`Esc`, `Tab`, `S`, `Enter` behavior).

4. Remove unused code from migration:
- Delete dead old `TextBuffer` logic and helpers no longer referenced.
- Delete obsolete manual cursor/scroll math in render paths replaced by editor widget behavior.
- Remove stale state fields/functions/tests tied only to removed behavior.

5. Validate:
- `cargo fmt`
- `cargo clippy --all-targets --all-features -D warnings`
- `cargo test`

6. Report:
- List changed files.
- Summarize removed dead code.
- Note any follow-up risks or edge cases.
