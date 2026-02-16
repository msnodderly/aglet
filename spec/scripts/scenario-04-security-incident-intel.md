# Scenario 04: Security Incident Intelligence

This script models security analysis records and triage metadata.

## Focus

- IOC and incident taxonomy
- exclusive severity and verification categories
- include/exclude views for analyst triage
- late category creation

## Commands To Type

```text
DB=/tmp/aglet-scenario-04-security-$(date +%s).ag
rm -f "$DB"

# Core taxonomy
cargo run -q -p agenda-cli -- --db "$DB" category create Security
cargo run -q -p agenda-cli -- --db "$DB" category create IncidentFamily --parent Security --exclusive
cargo run -q -p agenda-cli -- --db "$DB" category create Phishing --parent IncidentFamily
cargo run -q -p agenda-cli -- --db "$DB" category create Ransomware --parent IncidentFamily
cargo run -q -p agenda-cli -- --db "$DB" category create CredentialAbuse --parent IncidentFamily

cargo run -q -p agenda-cli -- --db "$DB" category create Severity --parent Security --exclusive
cargo run -q -p agenda-cli -- --db "$DB" category create Critical --parent Severity
cargo run -q -p agenda-cli -- --db "$DB" category create HighSeverity --parent Severity
cargo run -q -p agenda-cli -- --db "$DB" category create MediumSeverity --parent Severity
cargo run -q -p agenda-cli -- --db "$DB" category create LowSeverity --parent Severity

cargo run -q -p agenda-cli -- --db "$DB" category create Verification --parent Security --exclusive
cargo run -q -p agenda-cli -- --db "$DB" category create Confirmed --parent Verification
cargo run -q -p agenda-cli -- --db "$DB" category create Suspected --parent Verification
cargo run -q -p agenda-cli -- --db "$DB" category create FalsePositive --parent Verification

# Add records before IOC categories exist
cargo run -q -p agenda-cli -- --db "$DB" add "Phishing campaign invoice lure domain observed tomorrow at 10am"
cargo run -q -p agenda-cli -- --db "$DB" add "Ransomware beacon callback IP observed next Monday at 9am"
cargo run -q -p agenda-cli -- --db "$DB" add "CredentialAbuse unusual login telemetry summary"

# Late IOC taxonomy
cargo run -q -p agenda-cli -- --db "$DB" category create IOCType --parent Security --exclusive
cargo run -q -p agenda-cli -- --db "$DB" category create DomainIOC --parent IOCType
cargo run -q -p agenda-cli -- --db "$DB" category create IPIOC --parent IOCType
cargo run -q -p agenda-cli -- --db "$DB" category create HashIOC --parent IOCType

# IDs
PHISH_ID=$(cargo run -q -p agenda-cli -- --db "$DB" list --include-done | rg "Phishing campaign invoice lure" | head -n1 | cut -d' ' -f1)
RANSOM_ID=$(cargo run -q -p agenda-cli -- --db "$DB" list --include-done | rg "Ransomware beacon callback" | head -n1 | cut -d' ' -f1)
CRED_ID=$(cargo run -q -p agenda-cli -- --db "$DB" list --include-done | rg "CredentialAbuse unusual login" | head -n1 | cut -d' ' -f1)

# Manual triage assignments
cargo run -q -p agenda-cli -- --db "$DB" category assign "$PHISH_ID" HighSeverity
cargo run -q -p agenda-cli -- --db "$DB" category assign "$PHISH_ID" Confirmed
cargo run -q -p agenda-cli -- --db "$DB" category assign "$PHISH_ID" DomainIOC

cargo run -q -p agenda-cli -- --db "$DB" category assign "$RANSOM_ID" Critical
cargo run -q -p agenda-cli -- --db "$DB" category assign "$RANSOM_ID" Suspected
cargo run -q -p agenda-cli -- --db "$DB" category assign "$RANSOM_ID" IPIOC

cargo run -q -p agenda-cli -- --db "$DB" category assign "$CRED_ID" MediumSeverity
cargo run -q -p agenda-cli -- --db "$DB" category assign "$CRED_ID" FalsePositive
cargo run -q -p agenda-cli -- --db "$DB" category assign "$CRED_ID" HashIOC

# Views
cargo run -q -p agenda-cli -- --db "$DB" view create "Security Confirmed High" --include Security --include Confirmed --include HighSeverity
cargo run -q -p agenda-cli -- --db "$DB" view create "Security Critical NotConfirmed" --include Security --include Critical --exclude Confirmed
cargo run -q -p agenda-cli -- --db "$DB" view create "Security Phishing NotFalsePositive" --include Phishing --exclude FalsePositive
cargo run -q -p agenda-cli -- --db "$DB" view create "Security Impossible Verification" --include Confirmed --include FalsePositive

# Inspect
cargo run -q -p agenda-cli -- --db "$DB" view show "Security Confirmed High"
cargo run -q -p agenda-cli -- --db "$DB" view show "Security Critical NotConfirmed"
cargo run -q -p agenda-cli -- --db "$DB" view show "Security Phishing NotFalsePositive"
cargo run -q -p agenda-cli -- --db "$DB" view show "Security Impossible Verification"
```

## Expected Outcomes

1. `Security Confirmed High` is non-empty.
2. `Security Critical NotConfirmed` is non-empty (critical record is suspected, not confirmed).
3. `Security Phishing NotFalsePositive` is non-empty.
4. `Security Impossible Verification` is empty because `Verification` is exclusive.
