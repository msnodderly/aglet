#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -lt 1 ] || [ "$#" -gt 7 ]; then
  echo "usage: $0 <title> [note] [priority] [status] [issue-type] [project] [db-path]" >&2
  echo "example: $0 \"Support OR category filters\" \"Allow Aglet OR NeoNV in list queries\" High \"In Progress\" \"Feature request\" Aglet aglet-features.ag" >&2
  exit 1
fi

if [ "$1" = "-h" ] || [ "$1" = "--help" ]; then
  echo "usage: $0 <title> [note] [priority] [status] [issue-type] [project] [db-path]"
  echo "example: $0 \"Support OR category filters\" \"Allow Aglet OR NeoNV in list queries\" High \"In Progress\" \"Feature request\" Aglet aglet-features.ag"
  exit 0
fi

title="$1"
note="${2:-}"
priority="${3:-Normal}"
status="${4:-Ready}"
issue_type="${5:-Feature request}"
project="${6:-Aglet}"
db_path="${7:-aglet-features.ag}"

if [ -n "$note" ]; then
  create_output="$(cargo run --bin aglet -- --db "$db_path" add "$title" --note "$note" 2>&1)"
else
  create_output="$(cargo run --bin aglet -- --db "$db_path" add "$title" 2>&1)"
fi

item_id="$(printf '%s\n' "$create_output" | awk '/^created / { print $2 }' | tail -1)"
if [ -z "$item_id" ]; then
  echo "error: failed to parse created item id" >&2
  printf '%s\n' "$create_output" >&2
  exit 1
fi

cargo run --bin aglet -- --db "$db_path" category assign "$item_id" "$issue_type"
cargo run --bin aglet -- --db "$db_path" category assign "$item_id" "$project"
cargo run --bin aglet -- --db "$db_path" category assign "$item_id" "$priority"
cargo run --bin aglet -- --db "$db_path" category assign "$item_id" "$status"

echo "created_issue_id=$item_id"
