---
title: Feature Requests
updated: 2026-02-20
---

# Feature Requests

Date: 2026-02-20
Status: Backlog tracking for proposed features

## High Priority

### CLI Feature Parity with TUI
- Ensure the CLI has feature parity with the TUI
- Keep CLI up to date with all model changes

### View Editing UX Improvements
- Prompt to save before exiting when editing a view
- Use `S` to save instead of Enter when editing a view

### Section Validation
- When adding a section, prevent creating sections incompatible with overall view criteria
- Example: If view criteria is `p0` and user creates subsection excluding `p0`, that's invalid

### Views Without Overall Criteria
- Allow creating a view with no overall criteria (shows all items)

## Medium Priority

### Input/Editing Improvements
- Add readline-compatible editing (Ctrl-A, Ctrl-E, etc.)
- File picker (Ctrl-O style) to open a different database file

### Auto-Assignment Enhancement
- Auto-assignment should (optionally?) search notes field

### Display Options
- Option for rotating sections view between horizontal and vertical layouts

### Category Short Names
- Categories can optionally have short names
- Support emoji short names (e.g., up arrow for "high priority")

## Lower Priority / Future

### View Creation Query Syntax
- Structured query syntax for creating views

### Multi-Line Option Per Section
- Support multi-line content within section options

### AI Features
- Describe desired view in natural language and have AI create it

### Vim/Visidata-Style Commands
- Implement `:` command mode (e.g., `:assign`, `:delete`, etc.)

---

## Status Tracking

| Feature | Priority | Status | Notes |
|---------|----------|--------|-------|
| CLI feature parity | High | Pending | |
| Save prompt on view edit exit | High | Pending | |
| S to save (not Enter) | High | Pending | |
| Section compatibility validation | High | Pending | |
| Views with no criteria | High | Pending | |
| Readline editing | Medium | Pending | |
| File picker | Medium | Pending | Open different database file |
| Auto-assignment search notes | Medium | Pending | |
| Section layout rotation | Medium | Pending | |
| Category short names/emoji | Medium | Pending | |
| View query syntax | Low | Deferred | Requires design |
| Multi-line section options | Low | Deferred | |
| AI view creation | Low | Deferred | |
| Vim-style :commands | Low | Deferred | |
