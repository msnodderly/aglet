#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: $0 [--db <db-path>] <project-category> [<project-category> ...]" >&2
  echo "example: $0 --db ../aglet-features.ag neonv" >&2
  echo "example: $0 --db ../aglet-features.ag aglet neonv" >&2
}

db_path="../aglet-features.ag"
project_categories=()

while [ "$#" -gt 0 ]; do
  case "$1" in
    --db)
      shift
      if [ "$#" -eq 0 ]; then
        echo "error: --db requires a path" >&2
        usage
        exit 1
      fi
      db_path="$1"
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      project_categories+=("$1")
      ;;
  esac
  shift
done

if [ "${#project_categories[@]}" -eq 0 ]; then
  usage
  exit 1
fi

cmd=(
  cargo run --bin aglet -- --db "$db_path"
  list
  --exclude-category Done
  --exclude-category Complete
  --exclude-category "In Progress"
  --exclude-category "Waiting/Blocked"
  --sort Priority
)

for project_category in "${project_categories[@]}"; do
  cmd+=(--any-category "$project_category")
done

if cargo run --bin aglet -- --db "$db_path" view list 2>/dev/null | rg -qi '^All Items '; then
  cmd+=(--view "All Items")
fi

"${cmd[@]}"
