#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo "usage: $0 [db-path] [inbox-dir]" >&2
  echo "example: $0 ../llm-agenda.ag inbox/llm-agenda" >&2
}

if [ "$#" -gt 2 ]; then
  usage
  exit 1
fi

if [ "${1:-}" = "-h" ] || [ "${1:-}" = "--help" ]; then
  usage
  exit 0
fi

db_path="${1:-../llm-agenda.ag}"
inbox_dir="${2:-inbox/llm-agenda}"
processed_dir="$inbox_dir/processed"

mkdir -p "$processed_dir"

run_aglet() {
  cargo run --quiet --bin aglet -- --db "$db_path" "$@"
}

category_exists() {
  run_aglet category show "$1" >/dev/null 2>&1
}

ensure_category() {
  local name="$1"
  local note="$2"
  shift 2

  if category_exists "$name"; then
    echo "category_exists=$name"
    return
  fi

  run_aglet category create "$name" "$@" 2>&1 | tail -1
  if [ -n "$note" ]; then
    run_aglet category update "$name" --note "$note" 2>&1 | tail -1
  fi
}

view_exists() {
  run_aglet view show "$1" >/dev/null 2>&1
}

view_sections_json() {
  if ! command -v sqlite3 >/dev/null 2>&1; then
    return 1
  fi
  sqlite3 "$db_path" "SELECT sections_json FROM views WHERE name = '$1';"
}

ensure_view() {
  local name="$1"
  shift

  if view_exists "$name"; then
    echo "view_exists=$name"
    return
  fi

  run_aglet view create "$name" "$@" 2>&1 | tail -1
}

ensure_view_section_and_column() {
  local name="$1"
  local title="$2"
  local sections_json
  shift 2

  ensure_view "$name" "$@"
  sections_json="$(view_sections_json "$name" || true)"

  if [ -z "$sections_json" ] || [ "$sections_json" = "[]" ]; then
    run_aglet view section add "$name" "$title" "$@" 2>&1 | tail -1
    sections_json="$(view_sections_json "$name" || true)"
  fi

  if [ -z "$sections_json" ] || printf '%s\n' "$sections_json" | grep -q '"columns":\[\]'; then
    run_aglet view column add "$name" 0 Priority --width 12 2>&1 | tail -1
  else
    echo "view_column_exists=$name section=0"
  fi
}

ensure_all_items_section_and_column() {
  local priority_id
  local sections_json
  local next_sections_json

  if ! command -v sqlite3 >/dev/null 2>&1; then
    echo "warning: sqlite3 not found; cannot ensure All Items section/column" >&2
    return
  fi

  priority_id="$(sqlite3 "$db_path" "SELECT id FROM categories WHERE name = 'Priority';")"
  if [ -z "$priority_id" ]; then
    echo "warning: Priority category not found; cannot ensure All Items column" >&2
    return
  fi

  sections_json="$(view_sections_json "All Items" || true)"
  if [ -n "$sections_json" ] && [ "$sections_json" != "[]" ] \
    && ! printf '%s\n' "$sections_json" | grep -q '"columns":\[\]'; then
    echo "view_column_exists=All Items section=0"
    return
  fi

  next_sections_json='[{"title":"All","criteria":{"criteria":[],"virtual_include":[],"virtual_exclude":[],"text_search":null},"columns":[{"kind":"Standard","heading":"'"$priority_id"'","width":12,"summary_fn":null}],"item_column_index":0,"on_insert_assign":[],"on_remove_unassign":[],"show_children":false,"board_display_mode_override":null}]'
  sqlite3 "$db_path" "UPDATE views SET sections_json = '$next_sections_json' WHERE name = 'All Items';"
  echo "updated system view All Items section/column"
}

# Top-level PARA organization buckets. Implicit string matching starts disabled
# so broad words like "projects" or "resources" do not over-classify notes.
ensure_category "Projects" "Active short-term outcomes with a clear finish line. Prefer project:<slug> children for committed work." --disable-implicit-string
ensure_category "Areas" "Ongoing responsibilities and standards that need continuing attention. Prefer area:<slug> children." --disable-implicit-string
ensure_category "Resources" "Reference material, topics, assets, entities, and concepts not tied to an active outcome. Prefer resource:<slug> children." --disable-implicit-string
ensure_category "Archives" "Inactive, completed, paused, or no-longer-relevant material retained for future reference." --disable-implicit-string
ensure_category "Signal" "Review signals such as contradictions, gaps, follow-ups, and hypotheses." --disable-implicit-string

# Exclusive workflow families. Use Completed instead of Done because Done is a
# reserved system category name in aglet.
ensure_category "Status" "Exclusive lifecycle state for knowledge items." --exclusive --disable-implicit-string
ensure_category "Open" "Default active status for unresolved claims and findings." --parent "Status" --disable-implicit-string
ensure_category "In Progress" "Status for work currently being maintained or reviewed." --parent "Status" --disable-implicit-string
ensure_category "Completed" "Status for finished review or maintenance work. Do not use the reserved Done category name." --parent "Status" --disable-implicit-string
ensure_category "Superseded" "Status for claims replaced by newer or more authoritative items." --parent "Status" --disable-implicit-string
ensure_category "Needs Review" "Status for items whose wording, interpretation, evidence, or classification needs review before being trusted." --parent "Status" --disable-implicit-string

ensure_category "Priority" "Exclusive importance or urgency level." --exclusive --disable-implicit-string
ensure_category "Critical" "Highest-priority finding or follow-up." --parent "Priority" --disable-implicit-string
ensure_category "High" "Important finding or follow-up." --parent "Priority" --disable-implicit-string
ensure_category "Normal" "Default priority." --parent "Priority" --disable-implicit-string
ensure_category "Low" "Low-priority finding or follow-up." --parent "Priority" --disable-implicit-string

ensure_category "Contradiction" "Signal for claims that conflict with another item or source." --parent "Signal" --disable-implicit-string
ensure_category "Gap" "Signal for missing evidence, missing coverage, or unresolved questions." --parent "Signal" --disable-implicit-string
ensure_category "Follow-up" "Signal for items that should become future work or investigation." --parent "Signal" --disable-implicit-string
ensure_category "Hypothesis" "Signal for provisional synthesis that needs supporting evidence." --parent "Signal" --disable-implicit-string

ensure_all_items_section_and_column
ensure_view_section_and_column "Projects" "Projects" --include "Projects"
ensure_view_section_and_column "Areas" "Areas" --include "Areas"
ensure_view_section_and_column "Resources" "Resources" --include "Resources"
ensure_view_section_and_column "Archives" "Archives" --include "Archives"
ensure_view_section_and_column "Signal" "Signal" --include "Signal"
ensure_view_section_and_column "Status" "Status" --include "Status"
ensure_view_section_and_column "Priority" "Priority" --include "Priority"

echo "created_or_verified_db=$db_path"
echo "created_or_verified_inbox=$inbox_dir"
echo "processed_dir=$processed_dir"
