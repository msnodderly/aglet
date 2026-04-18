# TUI Smoke Testing via tmux

Procedure for automated and semi-automated TUI testing using tmux
`send-keys` / `capture-pane`. This replaces interactive manual testing with
reproducible, scriptable sequences.

## Why tmux

The TUI runs in alternate screen mode (`crossterm::terminal::EnterAlternateScreen`).
tmux captures this screen and lets us:

- Send exact keystroke sequences (`send-keys`)
- Read the rendered output (`capture-pane -p`)
- Assert on visible text (section headers, status messages, footer hints)
- Run headlessly in CI

## Setup

```bash
# Create a fresh DB with seed data
DB="/tmp/aglet-tui-smoke-$(date +%s).ag"
CLI="cargo run -q -p agenda-cli -- --db $DB"

$CLI category create Work
$CLI category create Personal
$CLI item add "Task one"
$CLI item add "Task two"
$CLI item add "Task three"
```

## Launch

```bash
# Kill any existing session, start fresh
tmux kill-session -t smoke 2>/dev/null
tmux new-session -d -s smoke -x 120 -y 40

# Launch TUI — allow ~3s for cargo build + startup
tmux send-keys -t smoke \
  "cargo run -q -p agenda-tui -- --db $DB" Enter
sleep 3

# Verify TUI is running
tmux capture-pane -t smoke -p | head -5
# Expected: "Agenda Reborn  view:All Items  mode:Normal"
```

## Interaction Patterns

### Send keys

```bash
# Single key
tmux send-keys -t smoke n

# Key sequence (no delay needed between non-dependent keys)
tmux send-keys -t smoke "Deploy server" Enter

# Control keys
tmux send-keys -t smoke C-s          # Ctrl+S
tmux send-keys -t smoke Escape        # Esc
tmux send-keys -t smoke Tab           # Tab
tmux send-keys -t smoke BTab          # Shift+Tab (BackTab)
tmux send-keys -t smoke C-a C-k       # Select-all + kill line (clear field)

# Capital letters (shift keys) — just send the character
tmux send-keys -t smoke S             # Capital S (save in ViewEdit)
tmux send-keys -t smoke J             # Jump cursor to next section
tmux send-keys -t smoke N             # New (view/category child)
```

### Read screen

```bash
# Full screen capture
tmux capture-pane -t smoke -p

# First N lines (header + content)
tmux capture-pane -t smoke -p | head -10

# Footer (status + hints)
tmux capture-pane -t smoke -p | tail -5

# Grep for specific text
tmux capture-pane -t smoke -p | grep -q "Item added" && echo PASS
```

### Timing

Wait ~1s after most operations for re-render. For operations that trigger
disk I/O (save, undo), 1s is usually sufficient. For cargo build on first
launch, 3-5s.

```bash
tmux send-keys -t smoke n && sleep 1 && tmux capture-pane -t smoke -p | head -20
```

## Key Bindings Reference

### Normal mode

| Key | Action |
|---|---|
| `n` | Add item |
| `e` | Edit item |
| `a` | Assign categories |
| `d` | Toggle done |
| `x` | Delete (confirm: y/Esc) |
| `r` | Remove from view |
| `p` | Toggle preview |
| `J`/`K` | Jump cursor to next/prev section |
| `j`/`k` | Navigate items within section |
| `Tab`/`S-Tab` | Jump cursor to next/prev section |
| `h`/`l` | Navigate sections (lane layout) |
| `[`/`]` | Move item to prev/next section |
| `/` | Section search |
| `g/` | Global search |
| `v` | View picker |
| `c` | Category manager |
| `m` | Toggle lane layout |
| `?` | Help panel |
| `q` | Quit (no confirm) |
| `C-z` | Undo |

### InputPanel (Add/Edit item)

| Key | Action |
|---|---|
| `Tab` | Next focus (Text -> When -> Note -> Categories -> Actions) |
| `C-s` | Save (from any focus) |
| `Esc` | Cancel / discard-confirm if dirty |
| `l` | Toggle link-note (Actions/Categories focus) |
| `Space` | Toggle category (Categories focus) |

### ViewEdit

| Key | Action |
|---|---|
| `Tab` | Cycle panes (sections <-> details) |
| `S` | Save view |
| `Enter` | Edit field / enter section detail |
| `n` | Add section (sections pane) |
| `e` | Rename section title (sections pane) |
| `J`/`K` | Reorder sections |
| `Esc` | Close (returns to view picker) |

### Category Manager

| Key | Action |
|---|---|
| `n` | New sibling category |
| `N` | New child category |
| `r` | Rename |
| `x` | Delete |
| `Tab` | Switch to details pane |
| `/` | Filter categories |
| `Esc` | Close |

## Common Test Sequences

### Create view with two sections

```bash
tmux send-keys -t smoke v          # open view picker
sleep 1
tmux send-keys -t smoke N          # new view
sleep 1
tmux send-keys -t smoke "My View" Enter  # name it
sleep 1
tmux send-keys -t smoke Tab        # sections pane
tmux send-keys -t smoke j Enter    # select default section, enter details
tmux send-keys -t smoke Enter      # edit title
tmux send-keys -t smoke C-a C-k "Section A" Enter  # rename
sleep 1
tmux send-keys -t smoke Tab j Enter  # details -> Filter -> open picker
tmux send-keys -t smoke Space Escape  # toggle category, close
tmux send-keys -t smoke Tab n        # back to sections, add new section
tmux send-keys -t smoke C-a C-k "Section B" Enter  # name it
tmux send-keys -t smoke S           # save view
```

### Add item with category

```bash
tmux send-keys -t smoke n           # add item
sleep 1
tmux send-keys -t smoke "New task"  # type name
tmux send-keys -t smoke Tab Tab Tab # skip When, Note -> Categories
tmux send-keys -t smoke Space       # toggle first category
tmux send-keys -t smoke C-s         # save
```

### Edit item, add note, link to file

```bash
tmux send-keys -t smoke e           # edit
sleep 1
tmux send-keys -t smoke Tab Tab     # skip When -> Note
tmux send-keys -t smoke "My note"   # type note
tmux send-keys -t smoke Tab         # -> Categories
tmux send-keys -t smoke Tab         # -> Actions
tmux send-keys -t smoke j j Space   # navigate to Link, toggle
tmux send-keys -t smoke C-s         # save
```

## Assertions

Check mode from header line:

```bash
MODE=$(tmux capture-pane -t smoke -p | head -1 | grep -oP 'mode:\K\w+')
[ "$MODE" = "Normal" ] && echo PASS
```

Check status from footer:

```bash
STATUS=$(tmux capture-pane -t smoke -p | tail -4 | head -1)
echo "$STATUS" | grep -q "Item added" && echo PASS
```

Check section item count:

```bash
tmux capture-pane -t smoke -p | grep -oP 'Work Items \(\K\d+'
```

## Teardown

```bash
tmux kill-session -t smoke 2>/dev/null
rm -f "$DB"
```
