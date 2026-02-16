# Scenario 02: Investigative Journalism Dossier

This script models a newsroom investigation workspace.

## Focus

- organizing evidence by case, people, and source quality
- exclusive reliability and legal-risk categories
- include/exclude views for editorial filtering
- late category creation over existing notes

## Commands To Type

```text
DB=/tmp/aglet-scenario-02-journalism-$(date +%s).ag
rm -f "$DB"

# Core taxonomy
cargo run -q -p agenda-cli -- --db "$DB" category create Journalism
cargo run -q -p agenda-cli -- --db "$DB" category create Investigation --parent Journalism
cargo run -q -p agenda-cli -- --db "$DB" category create CaseRiverfront --parent Investigation

cargo run -q -p agenda-cli -- --db "$DB" category create SourceReliability --parent Journalism --exclusive
cargo run -q -p agenda-cli -- --db "$DB" category create Verified --parent SourceReliability
cargo run -q -p agenda-cli -- --db "$DB" category create Unverified --parent SourceReliability
cargo run -q -p agenda-cli -- --db "$DB" category create Disputed --parent SourceReliability

cargo run -q -p agenda-cli -- --db "$DB" category create LegalRisk --parent Journalism --exclusive
cargo run -q -p agenda-cli -- --db "$DB" category create LowRisk --parent LegalRisk
cargo run -q -p agenda-cli -- --db "$DB" category create MediumRisk --parent LegalRisk
cargo run -q -p agenda-cli -- --db "$DB" category create HighRisk --parent LegalRisk

# Add records before all people/org categories exist
cargo run -q -p agenda-cli -- --db "$DB" add "CaseRiverfront: leaked procurement memo references HarborCorp"
cargo run -q -p agenda-cli -- --db "$DB" add "CaseRiverfront: witness statement from Elena about bid process"
cargo run -q -p agenda-cli -- --db "$DB" add "CaseRiverfront: anonymous tip alleges offshore payments"

# Late category creation for entities
cargo run -q -p agenda-cli -- --db "$DB" category create HarborCorp --parent Investigation
cargo run -q -p agenda-cli -- --db "$DB" category create Elena --parent Investigation
cargo run -q -p agenda-cli -- --db "$DB" category create AnonymousSource --parent Investigation

# Item IDs
MEMO_ID=$(cargo run -q -p agenda-cli -- --db "$DB" list --include-done | rg "leaked procurement memo" | head -n1 | cut -d' ' -f1)
WITNESS_ID=$(cargo run -q -p agenda-cli -- --db "$DB" list --include-done | rg "witness statement from Elena" | head -n1 | cut -d' ' -f1)
TIP_ID=$(cargo run -q -p agenda-cli -- --db "$DB" list --include-done | rg "anonymous tip alleges offshore" | head -n1 | cut -d' ' -f1)

# Manual reliability/risk tagging
cargo run -q -p agenda-cli -- --db "$DB" category assign "$MEMO_ID" Verified
cargo run -q -p agenda-cli -- --db "$DB" category assign "$MEMO_ID" MediumRisk

cargo run -q -p agenda-cli -- --db "$DB" category assign "$WITNESS_ID" Verified
cargo run -q -p agenda-cli -- --db "$DB" category assign "$WITNESS_ID" LowRisk

cargo run -q -p agenda-cli -- --db "$DB" category assign "$TIP_ID" Unverified
cargo run -q -p agenda-cli -- --db "$DB" category assign "$TIP_ID" HighRisk

# Views exercising include/exclude logic
cargo run -q -p agenda-cli -- --db "$DB" view create "Riverfront Verified LowRisk" --include CaseRiverfront --include Verified --include LowRisk
cargo run -q -p agenda-cli -- --db "$DB" view create "Riverfront Verified NotHighRisk" --include CaseRiverfront --include Verified --exclude HighRisk
cargo run -q -p agenda-cli -- --db "$DB" view create "Riverfront Unverified NotHighRisk" --include CaseRiverfront --include Unverified --exclude HighRisk
cargo run -q -p agenda-cli -- --db "$DB" view create "Riverfront Impossible Reliability" --include Verified --include Unverified

# Inspect
cargo run -q -p agenda-cli -- --db "$DB" view show "Riverfront Verified LowRisk"
cargo run -q -p agenda-cli -- --db "$DB" view show "Riverfront Verified NotHighRisk"
cargo run -q -p agenda-cli -- --db "$DB" view show "Riverfront Unverified NotHighRisk"
cargo run -q -p agenda-cli -- --db "$DB" view show "Riverfront Impossible Reliability"
```

## Expected Outcomes

1. `Riverfront Verified LowRisk` is non-empty.
2. `Riverfront Verified NotHighRisk` is non-empty.
3. `Riverfront Unverified NotHighRisk` is empty because the unverified item is tagged `HighRisk`.
4. `Riverfront Impossible Reliability` is empty because `SourceReliability` is exclusive.
