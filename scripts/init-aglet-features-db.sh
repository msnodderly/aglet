#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -gt 1 ]; then
  echo "usage: $0 [db-path]" >&2
  echo "example: $0 ../aglet-features.ag" >&2
  exit 1
fi

db_path="${1:-../aglet-features.ag}"

if [ -e "$db_path" ]; then
  echo "error: database already exists at $db_path" >&2
  exit 1
fi

# Issue type (non-exclusive)
cargo run --bin aglet -- --db "$db_path" category create "Issue type" 2>&1 | tail -1
cargo run --bin aglet -- --db "$db_path" category create "Bug" --parent "Issue type" 2>&1 | tail -1
cargo run --bin aglet -- --db "$db_path" category create "Idea" --parent "Issue type" 2>&1 | tail -1
cargo run --bin aglet -- --db "$db_path" category create "Feature request" --parent "Issue type" 2>&1 | tail -1

# Priority (exclusive)
cargo run --bin aglet -- --db "$db_path" category create "Priority" --exclusive 2>&1 | tail -1
cargo run --bin aglet -- --db "$db_path" category create "Critical" --parent "Priority" 2>&1 | tail -1
cargo run --bin aglet -- --db "$db_path" category create "High" --parent "Priority" 2>&1 | tail -1
cargo run --bin aglet -- --db "$db_path" category create "Normal" --parent "Priority" 2>&1 | tail -1
cargo run --bin aglet -- --db "$db_path" category create "Low" --parent "Priority" 2>&1 | tail -1

# Software Projects (non-exclusive)
cargo run --bin aglet -- --db "$db_path" category create "Software Projects" 2>&1 | tail -1
cargo run --bin aglet -- --db "$db_path" category create "Aglet" --parent "Software Projects" 2>&1 | tail -1
cargo run --bin aglet -- --db "$db_path" category create "NeoNV" --parent "Software Projects" 2>&1 | tail -1

# Status (exclusive)
cargo run --bin aglet -- --db "$db_path" category create "Status" --exclusive 2>&1 | tail -1
cargo run --bin aglet -- --db "$db_path" category create "Complete" --parent "Status" 2>&1 | tail -1
cargo run --bin aglet -- --db "$db_path" category create "In Progress" --parent "Status" 2>&1 | tail -1
cargo run --bin aglet -- --db "$db_path" category create "Next Action" --parent "Status" 2>&1 | tail -1
cargo run --bin aglet -- --db "$db_path" category create "Ready" --parent "Status" 2>&1 | tail -1
cargo run --bin aglet -- --db "$db_path" category create "Waiting/Blocked" --parent "Status" 2>&1 | tail -1

echo "created_db=$db_path"
