---
title: Schema Squash to v21
status: active
created: 2026-11-18
updated: 2026-11-18
---

# Schema Squash to v21

Decision D1 in `codebase-review-and-simplification-plan.md`, approved by the
user on 2026-11-18. D3's column drop (step 2) rides along so there is one data
migration and one backup instead of two.

## Problem

`store.rs` carries seven versioned migrations (v2 to v20) plus
`repair_missing_columns`, about 350 lines that run on every open. `SCHEMA_SQL`
already describes the full current schema, so this code only exists for old
files. The `assignments.source` column duplicates `explanation_json` (D3).

## Facts this plan relies on

- The only tagged release, `v0.1.0` (2026-07-17, the Homebrew build), writes
  schema v20 and repairs column drift on every open. A v20 file opened by it
  is complete.
- Files below v20 come from development builds older than the release.

## Design

`SCHEMA_VERSION` becomes 21. `Store::init` handles exactly these cases:

| `user_version` | Action |
|---|---|
| 0 and no `items` table | Fresh file: run `SCHEMA_SQL`, stamp 21. |
| 21 | Open. |
| 20 | Back up, then upgrade to 21 in one transaction. |
| 1 to 19, or 0 with tables | Refuse: "open this file once with aglet v0.1.0, then retry". |
| above 21 | Refuse: "created by a newer aglet". |

The v20 to v21 upgrade:

1. `VACUUM INTO '<file>.v20-backup.ag'`. If that path exists, refuse and
   say so; never overwrite a backup.
2. In one `BEGIN IMMEDIATE` transaction:
   - Rewrite every `assignments.explanation_json` that is null or does not
     parse as the current type to `{"Legacy":{"source":<source>}}`. Today such
     rows already behave as "source only"; this keeps that.
   - Rebuild `assignments` without the `source` column, with
     `explanation_json TEXT NOT NULL` and no default, so a migrated file has
     the same schema as a fresh one.
   - Rewrite `deletion_log.assignments_json` from the old shape (`source` plus
     nullable `explanation`) to the new one. Entries that do not parse are left
     as they are; restore already treats those as having no assignments.
   - Stamp `user_version = 21`.

Deleted after the squash: `migrate_v2` through `migrate_v20`,
`repair_missing_columns`, `migrate_datebook_empty_sections_to_view`,
`AssignmentRecord`, and the `source` column read and write. The old-shape
assignment type lives only inside the upgrade function.

## Rollback

- Before the upgrade commits, nothing changes: any failure rolls the
  transaction back and leaves the v20 file as it was.
- After it commits: the backup is a complete v20 file. To go back, install
  `v0.1.0` (or check out the tag), move the backup over the original path, and
  open it. `v0.1.0` cannot read v21 files.

## Done when

- A v20 fixture with a null explanation, an unparsable explanation, a normal
  row, and an old-shape deletion-log entry upgrades to v21; each row has the
  expected explanation; restore of the deletion-log entry keeps its
  assignments; the backup file exists and is still v20.
- A v19 file and a v22 file are refused with the messages above and are not
  modified.
- A fresh file and a migrated file have the same `assignments` schema.
- `store.rs` has no `migrate_v*` or `repair_missing_columns`.
- AGENTS.md "Store And Migration Notes" describes the new rules.

## Status

2026-11-18: implemented on `simplify/d1-schema-squash`; all done criteria
met. Tests: `v20_file_upgrades_to_v21_with_backup`,
`v20_upgrade_refuses_to_overwrite_an_existing_backup`,
`files_older_than_v20_or_newer_than_v21_are_refused_untouched`. Also checked
on the built binary against a v20 file made with `sqlite3` from
`crates/aglet-core/src/testdata/schema_v20.sql` (identical to v0.1.0's
schema). Not yet run against a real user file.
