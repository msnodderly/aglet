# Find Open Project Items (Generic Parameter)

Goal:
- not done
- not in `Complete`, `Done`, `In Progress`, or `Waiting/Blocked`
- matches one or more project category values (for example `neonv`, `aglet`, `project3`)
- sorted by `Priority`

`list` excludes done items by default, so you do not need a separate `not done` filter unless you pass `--include-done`.

## Native one-off commands

Single project category:

```bash
cargo run --bin agenda-cli -- --db aglet-features.ag list --any-category neonv --exclude-category Done --exclude-category Complete --exclude-category "In Progress" --exclude-category "Waiting/Blocked" --sort Priority
```

Equivalent shorthand:

```bash
cargo run --bin agenda-cli -- --db aglet-features.ag list --project neonv --open-ready --sort Priority
```

OR across multiple project categories:

```bash
cargo run --bin agenda-cli -- --db aglet-features.ag list --any-category aglet --any-category neonv --exclude-category Done --exclude-category Complete --exclude-category "In Progress" --exclude-category "Waiting/Blocked" --sort Priority
```

Equivalent shorthand:

```bash
cargo run --bin agenda-cli -- --db aglet-features.ag list --project aglet --project neonv --open-ready --sort Priority
```

You can combine both styles:
- `--category <name>`: AND semantics (must match all)
- `--any-category <name>`: OR semantics (must match at least one)
- `--project <name>`: OR semantics shorthand for `--any-category`
- `--exclude-category <name>`: NOT semantics (must match none)
- `--open-ready`: shorthand to exclude `Done`, `Complete`, `In Progress`, and `Waiting/Blocked`

## Reusable script

Use:

```bash
scripts/list-open-project-items.sh [--db <db-path>] <project-category> [<project-category> ...]
```

Examples:

```bash
scripts/list-open-project-items.sh --db aglet-features.ag neonv
scripts/list-open-project-items.sh --db aglet-features.ag aglet neonv
scripts/list-open-project-items.sh --db aglet-features.ag project3
```

The script uses native `list` filters (`--any-category`, `--exclude-category`) and sorts by `Priority`.

## Atomic Pickup

After selecting an `ITEM_ID` from the open-item list, claim it atomically:

```bash
cargo run --bin agenda-cli -- --db aglet-features.ag claim <ITEM_ID>
```

Default claim preconditions fail when the item already has `In Progress` or `Complete`, which reduces multi-agent pickup races.

## Notes

- Category matching is case-insensitive (`neonv` and `NeoNV` are equivalent).
- If your DB uses a parent category like `Software Project` or `Software Projects`, filtering on a child value (for example `NeoNV`) is enough.
- `--sort Priority` sorts by category label text. For explicit severity order, use:

```bash
cargo run --bin agenda-cli -- --db aglet-features.ag list --any-category neonv --exclude-category Done --exclude-category Complete --exclude-category "In Progress" --exclude-category "Waiting/Blocked" --sort Critical:asc --sort High:asc --sort Normal:asc --sort Low:asc
```
