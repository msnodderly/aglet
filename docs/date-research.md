# Date Management And `When` Field Research

Date of analysis: 2026-03-04 (America/Los_Angeles)
Repository: `aglet`

## 1. Executive Summary

Aglet has three persisted date fields on items:

- `entry_date: NaiveDate` (creation-day metadata)
- `when_date: Option<NaiveDateTime>` (schedule/due datetime inferred from item text)
- `done_date: Option<NaiveDateTime>` (completion timestamp)

The `When` concept has **three distinct layers** that are easy to confuse:

1. `Item.when_date` (the canonical persisted datetime)
2. Reserved category `When` (provenance marker, not a date bucket)
3. Virtual `WhenBucket` filters (`Today`, `Overdue`, etc.) computed at query time from `when_date`

The most important implementation truth: **view/date grouping behavior is driven by `when_date`, not by category assignment to `When`**.

---

## 2. Where Date Logic Lives

Primary code paths:

- Core model: `crates/agenda-core/src/model.rs`
- Date parsing: `crates/agenda-core/src/dates.rs`
- Create/update integration: `crates/agenda-core/src/agenda.rs`
- Query bucket evaluation: `crates/agenda-core/src/query.rs`
- Persistence/migrations: `crates/agenda-core/src/store.rs`
- CLI flows/output: `crates/agenda-cli/src/main.rs`
- TUI flows/rendering: `crates/agenda-tui/src/modes/board.rs`, `crates/agenda-tui/src/render/mod.rs`, `crates/agenda-tui/src/ui_support.rs`, `crates/agenda-tui/src/modes/view_edit2.rs`

Decision records:

- `spec/decisions.md` sections 12, 13, 20, 21, 22, 23, 24

---

## 3. Data Model Semantics

## 3.1 Item fields

From `Item` in `model.rs`:

- `entry_date`: required date-only field
- `when_date`: optional date+time
- `done_date`: optional date+time
- `is_done`: boolean status

`Item::new()` sets:

- `entry_date = Utc::now().date_naive()`
- `when_date = None`
- `done_date = None`
- `is_done = false`

Implication: `entry_date` is UTC-date based by default, not local-date based.

## 3.2 Reserved categories

Reserved names:

- `When`
- `Entry`
- `Done`

They are bootstrap-created in `Store::ensure_reserved_categories()` and inserted with:

- `is_actionable = false`
- `enable_implicit_string = false`

Purpose:

- Prevent accidental implicit matches on common words like "done" / "when" / "entry"
- Provide system semantics/provenance hooks

Important nuance: names are protected from create/delete/rename collisions, but reserved categories are still updatable in-place (for non-name fields) via `update_category`.

---

## 4. Persistence Layer (SQLite)

Schema (`store.rs`, `SCHEMA_SQL`):

- `items.entry_date TEXT NOT NULL`
- `items.when_date TEXT`
- `items.done_date TEXT`
- index: `idx_items_when_date`

Serialization format:

- Dates written via `.to_string()` (`YYYY-MM-DD` for `NaiveDate`, `YYYY-MM-DD HH:MM:SS` for `NaiveDateTime` in current flows)

Deserialization (`row_to_item`):

- `entry_date` parsed with `%Y-%m-%d`
- `when_date` / `done_date` parsed with `%Y-%m-%d %H:%M:%S`
- parse failures fall back (date defaults, datetime `None`) instead of panicking

Deletion log also snapshots all three date fields and restores them.

---

## 5. `when_date` Lifecycle (Creation/Update)

`Agenda::create_item_with_reference_date` and `Agenda::update_item_with_reference_date` do:

1. Parse `item.text` with `BasicDateParser`
2. If parse succeeds, set `when_date = parsed_datetime`
3. Persist item
4. If parse succeeded, upsert assignment to reserved `When` with:
   - `source = AutoMatch`
   - `origin = "nlp:date"`
5. Run rule engine `process_item`

Critical behavior:

- Parse success overwrites `when_date`
- Parse failure does **not** clear existing `when_date`
- There is currently no dedicated CLI/TUI flag to clear `when_date`

### 5.1 Reference date source

- Default `Agenda::create_item` / `update_item` use `Utc::now().date_naive()`
- CLI add/edit explicitly use `Local::now().date_naive()`
- TUI add/edit/note-save flows explicitly use `Local::now().date_naive()`

This means user-facing parsing is local-calendar anchored, while low-level default API helpers are UTC-calendar anchored.

### 5.2 Reparse on non-text edits

Because update flows always reparse `item.text`, note-only edits that call update can recompute relative phrases (like "tomorrow") against a new reference date.

Result: `when_date` can drift over time if text contains relative expressions and item is edited later.

---

## 6. BasicDateParser Deep Dive

Parser type: deterministic, rule-based, no ML.

Selection rule (`choose_best`):

- choose earliest start offset in text
- if same start offset, choose longer span

The parser scans for candidate dates, then optionally attaches trailing `at <time>`.

## 6.1 Supported date expressions

### Relative keywords

- `today`
- `tomorrow`
- `yesterday`

### Relative weekdays

- `this <weekday>`
- `next <weekday>`

Weekday words are full names (`monday`..`sunday`), case-insensitive.

### Month-name forms

- `May 25`
- `May 25, 2026`
- `May 25 2026`

Rules:

- year must be 4 digits when present
- if year omitted, resolver uses this year unless date already passed; then next year
- only tries `this_year` or `this_year + 1` for omitted-year resolution

Leap-day edge case:

- `February 29` may return `None` if this year is past Feb 29 and next year is non-leap

### ISO forms

- dashed: `YYYY-MM-DD`
- compact: `YYYYMMDD`

### Numeric slash form

- `M/D/YY` only (two-digit year required)
- year policy: `YY -> 2000 + YY`

Notably unsupported in current implementation:

- `M/D/YYYY`
- month abbreviations (`Jan`, `Feb`)
- ordinal suffixes (`May 25th`)
- richer natural-language constructs (for example recurrence phrases)

## 6.2 Time attachment

Time attaches only to an already matched date via trailing `at ...`:

- `at 3pm`
- `at 3:15pm`
- `at 15:00`
- `at noon`

Rules:

- date-only defaults to `00:00`
- time-only text is no-match
- invalid trailing time keeps date-only match (no hard failure)

## 6.3 Weekday disambiguation policy

Enum:

- `StrictNextWeek` (default)
- `InclusiveNext`

Default behavior in agenda flows is `StrictNextWeek`.

---

## 7. `When` Category vs `WhenBucket` vs `when_date`

## 7.1 Reserved `When` category

Used for provenance (`nlp:date`) when parser succeeds.

It is not itself a bucket and does not encode today/tomorrow semantics.

## 7.2 Virtual `WhenBucket` values

`WhenBucket` enum:

- `Overdue`
- `Today`
- `Tomorrow`
- `ThisWeek`
- `NextWeek`
- `ThisMonth`
- `Future`
- `NoDate`

Buckets are computed in `resolve_when_bucket(when_date, reference_date)`.

### Priority order

1. `NoDate` when missing
2. `Overdue` if date < reference
3. `Today`
4. `Tomorrow`
5. `ThisWeek`
6. `NextWeek`
7. `ThisMonth`
8. `Future`

Time component is ignored (`when_date.date()` only).

Week boundaries are ISO (Monday start).

## 7.3 Query semantics

`Query` has:

- category criteria (`And`, `Not`, `Or`)
- `virtual_include: HashSet<WhenBucket>`
- `virtual_exclude: HashSet<WhenBucket>`
- `text_search`

All query dimensions are AND-composed.

Important gotcha:

- `virtual_include` uses intersection semantics.
- If include has multiple different buckets (for example `{Today, Tomorrow}`), nothing matches (single item can only be one bucket).

---

## 8. CLI Date Behavior

## 8.1 Add

`cmd_add`:

- parses with local reference date
- prints `created <uuid>`
- prints `parsed_when=<datetime>` only when `when_date` is set

## 8.2 Edit

`cmd_edit`:

- text/note edits call `update_item_with_reference_date` (local date)
- prints `parsed_when=...` after update if `when_date` present
- `--note-stdin` empty payload is no-op for note replacement

## 8.3 Show

`cmd_show` prints:

- `status` (`open`/`done`)
- `when` (or `-`)
- `entry_date`
- optional `done_date`

## 8.4 List/View/Search

View resolution and query evaluation use local reference date for bucket resolution.

Sorting by `when`:

- compares `when_date`
- `None` (`no date`) sorts after dated items

## 8.5 Done interactions

CLI special-cases category assignment/unassignment for `Done`:

- assign `Done` => calls `mark_item_done` (sets `is_done`, `done_date`, and Done assignment)
- unassign `Done` => toggles back to not-done if currently done

---

## 9. TUI Date Behavior

## 9.1 Add/Edit flows

Board input-panel save paths call create/update with local reference date.

Add status uses `add_capture_status_message`:

- `Item added (parsed when: <datetime>)` when parsed

## 9.2 View editor date filters

View editor exposes date bucket filters (virtual include/exclude):

- displayed as "Date range (include)" and "Date range (exclude)"
- editable through bucket picker overlay
- keybindings include `]` for include picker and `[` for exclude picker

## 9.3 Board `When` column

`ColumnKind::When` renders `item.when_date` directly (or dash placeholder).

Rules:

- top-level `When` category is a valid board heading
- non-top-level `When` heading is rejected
- inline direct editing of `When` date in board columns is currently not implemented (explicit status message in code)

---

## 10. `entry_date` and `done_date` specifics

## 10.1 `entry_date`

- Set at item creation (`Item::new`, UTC date)
- Not updated in normal `Store::update_item` path
- Persisted and restored from deletion log

## 10.2 `done_date`

`Agenda::mark_item_done`:

- requires item has at least one actionable category
- sets `is_done = true`
- sets `done_date = now.naive_utc()` (nanoseconds truncated)
- assigns reserved `Done` with origin `manual:done`

`mark_item_not_done` clears `done_date` and removes Done assignment.

---

## 11. High-Value Gotchas

1. `when_date` is parser-driven from text, not directly editable in current UI flows.
2. Update reparsing means note-only edits can shift relative dates.
3. Parse miss does not clear stale `when_date`.
4. `entry_date`/`done_date` use UTC-naive timestamps; parser reference is local date in CLI/TUI.
5. Reserved `When` assignment is provenance metadata; bucket placement still comes from `when_date`.
6. Multiple `virtual_include` buckets produce empty results by design.
7. Numeric slash parser accepts `M/D/YY` but not `M/D/YYYY`.
8. `Entry` reserved category exists in schema/bootstrap but is not auto-assigned by current flows.
9. Board `When` column is display/sort-capable, but inline date editing is intentionally unimplemented right now.

---

## 12. Current Capability Boundaries

From scenario matrix and code:

- Dynamic date categories (`WhenBucket`) are implemented.
- Free-form date extraction is partial (deterministic subset only).
- Recurrence and richer NL date understanding are not implemented.

---

## 13. Practical Mental Model

Use this model when reasoning about Aglet dates:

- `entry_date` = immutable creation-day metadata
- `when_date` = mutable schedule datetime inferred from text at create/update time
- `done_date` = completion timestamp set by done workflow
- Reserved `When` category = "parser set a date" provenance flag
- `WhenBucket` = live query-time projection of `when_date` relative to "today"

If behavior looks wrong in views, debug in this order:

1. Check `when_date` on the item (`agenda show <id>`)
2. Check reference date source (local vs UTC caller path)
3. Check view/query `virtual_include` / `virtual_exclude`
4. Check for relative-phrase reparsing side effects after edits

---

## 14. Evidence Pointers

Core:

- `crates/agenda-core/src/model.rs`
- `crates/agenda-core/src/dates.rs`
- `crates/agenda-core/src/agenda.rs`
- `crates/agenda-core/src/query.rs`
- `crates/agenda-core/src/store.rs`

CLI:

- `crates/agenda-cli/src/main.rs`

TUI:

- `crates/agenda-tui/src/modes/board.rs`
- `crates/agenda-tui/src/modes/view_edit2.rs`
- `crates/agenda-tui/src/render/mod.rs`
- `crates/agenda-tui/src/ui_support.rs`

Specs/decisions:

- `spec/decisions.md`
- `spec/scenario-capability-matrix.md`
- `spec/product-current.md`
