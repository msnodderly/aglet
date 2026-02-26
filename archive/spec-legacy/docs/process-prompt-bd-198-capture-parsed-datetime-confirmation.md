# Task: bd-198 - Capture UX parsed date/time confirmation

## Context

Date parsing already sets `when_date` during create/update, but capture surfaces do not show the resolved datetime immediately. This task adds immediate, non-blocking feedback in both CLI and TUI add flows so users can spot ambiguous parses right away.

## What to read

1. `crates/agenda-cli/src/main.rs` - `cmd_add` behavior and output format.
2. `crates/agenda-tui/src/lib.rs` - add mode (`handle_add_key`, `create_item_in_current_context`) and status messaging.
3. `crates/agenda-core/src/agenda.rs` - create path and reference-date variants.
4. `spec/decisions.md` sections 22-23 - parser defaults and reference-date behavior.
5. `docs/guide-cli-manpage.md` - user-facing CLI docs.

## What to build

Implement capture-time confirmation for parsed datetime in add flows.

Behavioral rules:

- CLI `add` output includes parsed date/time when the created item has `when_date`.
- TUI add flow status/inline confirmation includes parsed date/time when the created item has `when_date`.
- Behavior is non-blocking: save proceeds without confirmation prompts.
- Keep datetime display format unchanged from current rendering conventions.
- For capture parsing in CLI/TUI add flows, use local calendar date as parser reference date.
- Confirmation should appear whenever `when_date` is present on the created item.

## Tests to write

1. CLI add-path output helper/logic test: parsed datetime line appears when `when_date` exists.
2. CLI add-path output helper/logic test: parsed datetime line omitted when `when_date` is absent.
3. TUI add-path status helper/logic test: status includes parsed datetime when `when_date` exists.
4. Existing behavior remains non-blocking (no additional prompt state required).

## What NOT to do

- Do not change date parser algorithms or disambiguation policy behavior.
- Do not introduce confirmation gates before save.
- Do not redesign datetime formatting style in this task.
- Do not add generalized prompting tunables/configuration in this task (capture-only UX fix first).

## How your code will be used

Users capture items via CLI/TUI and immediately see the resolved datetime used for `when_date`, improving trust and reducing hidden date misinterpretations.

## Workflow

Follow `AGENTS.md` and `PROMPT.md`.

- Issue ID: `bd-198`
- Worktree workflow: claim/close with `br` on main, code in worktree branch.

## Definition of done

- [ ] CLI add output shows parsed datetime when present.
- [ ] TUI add flow status/inline confirmation shows parsed datetime when present.
- [ ] Add flow remains non-blocking.
- [ ] Docs include examples and timezone/reference-date assumptions.
- [ ] `cargo test` and `cargo clippy` pass for touched crates.
