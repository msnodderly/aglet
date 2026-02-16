# Scenario 03: Legal Matter Intelligence

This script models legal information management across clients and matter phases.

## Focus

- client/matter taxonomy
- exclusive privilege and phase tags
- include/exclude views for counsel workflows
- late category creation
- duplicate text records with distinct IDs

## Commands To Type

```text
DB=/tmp/aglet-scenario-03-legal-$(date +%s).ag
rm -f "$DB"

# Core taxonomy
cargo run -q -p agenda-cli -- --db "$DB" category create Legal
cargo run -q -p agenda-cli -- --db "$DB" category create Client
cargo run -q -p agenda-cli -- --db "$DB" category create Matter

cargo run -q -p agenda-cli -- --db "$DB" category create PrivilegeLevel --parent Legal --exclusive
cargo run -q -p agenda-cli -- --db "$DB" category create Privileged --parent PrivilegeLevel
cargo run -q -p agenda-cli -- --db "$DB" category create Internal --parent PrivilegeLevel
cargo run -q -p agenda-cli -- --db "$DB" category create PublicRecord --parent PrivilegeLevel

cargo run -q -p agenda-cli -- --db "$DB" category create CasePhase --parent Legal --exclusive
cargo run -q -p agenda-cli -- --db "$DB" category create Intake --parent CasePhase
cargo run -q -p agenda-cli -- --db "$DB" category create Discovery --parent CasePhase
cargo run -q -p agenda-cli -- --db "$DB" category create Litigation --parent CasePhase
cargo run -q -p agenda-cli -- --db "$DB" category create Closed --parent CasePhase

# Add records before client and matter categories exist
cargo run -q -p agenda-cli -- --db "$DB" add "Acme antitrust hold notice draft"
cargo run -q -p agenda-cli -- --db "$DB" add "Acme antitrust hold notice draft"
cargo run -q -p agenda-cli -- --db "$DB" add "Nimbus contract dispute witness prep packet"
cargo run -q -p agenda-cli -- --db "$DB" add "Nimbus contract dispute filing checklist"

# Late category creation
cargo run -q -p agenda-cli -- --db "$DB" category create Acme --parent Client
cargo run -q -p agenda-cli -- --db "$DB" category create Nimbus --parent Client
cargo run -q -p agenda-cli -- --db "$DB" category create Antitrust --parent Matter
cargo run -q -p agenda-cli -- --db "$DB" category create ContractDispute --parent Matter
cargo run -q -p agenda-cli -- --db "$DB" category create OutsideCounsel --parent Legal

# IDs
ACME_ONE_ID=$(cargo run -q -p agenda-cli -- --db "$DB" list --include-done | rg "Acme antitrust hold notice draft" | head -n1 | cut -d' ' -f1)
ACME_TWO_ID=$(cargo run -q -p agenda-cli -- --db "$DB" list --include-done | rg "Acme antitrust hold notice draft" | tail -n1 | cut -d' ' -f1)
NIMBUS_WITNESS_ID=$(cargo run -q -p agenda-cli -- --db "$DB" list --include-done | rg "Nimbus contract dispute witness prep" | head -n1 | cut -d' ' -f1)
NIMBUS_CHECKLIST_ID=$(cargo run -q -p agenda-cli -- --db "$DB" list --include-done | rg "Nimbus contract dispute filing checklist" | head -n1 | cut -d' ' -f1)

# Manual tagging
cargo run -q -p agenda-cli -- --db "$DB" category assign "$ACME_ONE_ID" Privileged
cargo run -q -p agenda-cli -- --db "$DB" category assign "$ACME_ONE_ID" Discovery

cargo run -q -p agenda-cli -- --db "$DB" category assign "$ACME_TWO_ID" Internal
cargo run -q -p agenda-cli -- --db "$DB" category assign "$ACME_TWO_ID" Intake

cargo run -q -p agenda-cli -- --db "$DB" category assign "$NIMBUS_WITNESS_ID" Privileged
cargo run -q -p agenda-cli -- --db "$DB" category assign "$NIMBUS_WITNESS_ID" Litigation
cargo run -q -p agenda-cli -- --db "$DB" category assign "$NIMBUS_WITNESS_ID" OutsideCounsel

cargo run -q -p agenda-cli -- --db "$DB" category assign "$NIMBUS_CHECKLIST_ID" PublicRecord
cargo run -q -p agenda-cli -- --db "$DB" category assign "$NIMBUS_CHECKLIST_ID" Discovery

# Views
cargo run -q -p agenda-cli -- --db "$DB" view create "Acme Discovery Privileged" --include Acme --include Discovery --include Privileged
cargo run -q -p agenda-cli -- --db "$DB" view create "Nimbus Litigation NotOutsideCounsel" --include Nimbus --include Litigation --exclude OutsideCounsel
cargo run -q -p agenda-cli -- --db "$DB" view create "Discovery NotPublicRecord" --include Discovery --exclude PublicRecord
cargo run -q -p agenda-cli -- --db "$DB" view create "Impossible Privilege Combination" --include Privileged --include PublicRecord

# Inspect
cargo run -q -p agenda-cli -- --db "$DB" view show "Acme Discovery Privileged"
cargo run -q -p agenda-cli -- --db "$DB" view show "Nimbus Litigation NotOutsideCounsel"
cargo run -q -p agenda-cli -- --db "$DB" view show "Discovery NotPublicRecord"
cargo run -q -p agenda-cli -- --db "$DB" view show "Impossible Privilege Combination"

# Duplicate text verification
cargo run -q -p agenda-cli -- --db "$DB" search "Acme antitrust hold notice draft"
```

## Expected Outcomes

1. `Acme Discovery Privileged` is non-empty.
2. `Nimbus Litigation NotOutsideCounsel` is empty because the litigation item is tagged `OutsideCounsel`.
3. `Discovery NotPublicRecord` is non-empty.
4. `Impossible Privilege Combination` is empty because `PrivilegeLevel` is exclusive.
5. Duplicate search for Acme hold notice returns two distinct item IDs.
