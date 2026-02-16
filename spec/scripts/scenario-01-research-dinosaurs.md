# Scenario 01: Research Knowledge Base (Dinosaurs)

This script treats `aglet` as a research information manager.

## Focus

- taxonomy-oriented categories
- late category creation over existing notes
- include/exclude views for research slicing
- exclusive category behavior
- duplicate note text handling

## Commands To Type

```text
# Isolated database
DB=/tmp/aglet-scenario-01-research-$(date +%s).ag
rm -f "$DB"

# Baseline categories
cargo run -q -p agenda-cli -- --db "$DB" category create Research
cargo run -q -p agenda-cli -- --db "$DB" category create Dinosaurs --parent Research

# Exclusive taxonomies
cargo run -q -p agenda-cli -- --db "$DB" category create Era --parent Dinosaurs --exclusive
cargo run -q -p agenda-cli -- --db "$DB" category create Triassic --parent Era
cargo run -q -p agenda-cli -- --db "$DB" category create Jurassic --parent Era
cargo run -q -p agenda-cli -- --db "$DB" category create Cretaceous --parent Era

cargo run -q -p agenda-cli -- --db "$DB" category create Confidence --parent Dinosaurs --exclusive
cargo run -q -p agenda-cli -- --db "$DB" category create HighConfidence --parent Confidence
cargo run -q -p agenda-cli -- --db "$DB" category create MediumConfidence --parent Confidence
cargo run -q -p agenda-cli -- --db "$DB" category create LowConfidence --parent Confidence

# Add notes first; some categories will be created later
cargo run -q -p agenda-cli -- --db "$DB" add "Jurassic sauropod growth rates from bone histology"
cargo run -q -p agenda-cli -- --db "$DB" add "Cretaceous predator trackway evidence summary"
cargo run -q -p agenda-cli -- --db "$DB" add "Triassic transition species candidate review"
cargo run -q -p agenda-cli -- --db "$DB" add "Jurassic sauropod growth rates from bone histology"

# Late category creation for evidence source taxonomy
cargo run -q -p agenda-cli -- --db "$DB" category create EvidenceType --parent Dinosaurs --exclusive
cargo run -q -p agenda-cli -- --db "$DB" category create Histology --parent EvidenceType
cargo run -q -p agenda-cli -- --db "$DB" category create Trackway --parent EvidenceType
cargo run -q -p agenda-cli -- --db "$DB" category create Morphology --parent EvidenceType

# Capture item IDs
JURASSIC_ID=$(cargo run -q -p agenda-cli -- --db "$DB" list --include-done | rg "Jurassic sauropod growth rates" | head -n1 | cut -d' ' -f1)
CRET_TRACK_ID=$(cargo run -q -p agenda-cli -- --db "$DB" list --include-done | rg "Cretaceous predator trackway evidence" | head -n1 | cut -d' ' -f1)
TRIASSIC_ID=$(cargo run -q -p agenda-cli -- --db "$DB" list --include-done | rg "Triassic transition species" | head -n1 | cut -d' ' -f1)

# Manual confidence and evidence assignments
cargo run -q -p agenda-cli -- --db "$DB" category assign "$JURASSIC_ID" HighConfidence
cargo run -q -p agenda-cli -- --db "$DB" category assign "$JURASSIC_ID" Histology

cargo run -q -p agenda-cli -- --db "$DB" category assign "$CRET_TRACK_ID" MediumConfidence
cargo run -q -p agenda-cli -- --db "$DB" category assign "$CRET_TRACK_ID" Trackway

cargo run -q -p agenda-cli -- --db "$DB" category assign "$TRIASSIC_ID" LowConfidence
cargo run -q -p agenda-cli -- --db "$DB" category assign "$TRIASSIC_ID" Morphology

# Exercise exclusivity switch
cargo run -q -p agenda-cli -- --db "$DB" category assign "$TRIASSIC_ID" HighConfidence

# Views: include/exclude combinations
cargo run -q -p agenda-cli -- --db "$DB" view create "Research Jurassic High" --include Research --include Jurassic --include HighConfidence
cargo run -q -p agenda-cli -- --db "$DB" view create "Research Cretaceous Not Trackway" --include Research --include Cretaceous --exclude Trackway
cargo run -q -p agenda-cli -- --db "$DB" view create "Research High Not Trackway" --include Research --include HighConfidence --exclude Trackway
cargo run -q -p agenda-cli -- --db "$DB" view create "Research Impossible Confidence" --include HighConfidence --include MediumConfidence

# Inspect views
cargo run -q -p agenda-cli -- --db "$DB" view show "Research Jurassic High"
cargo run -q -p agenda-cli -- --db "$DB" view show "Research Cretaceous Not Trackway"
cargo run -q -p agenda-cli -- --db "$DB" view show "Research High Not Trackway"
cargo run -q -p agenda-cli -- --db "$DB" view show "Research Impossible Confidence"

# Duplicate text check
cargo run -q -p agenda-cli -- --db "$DB" search "Jurassic sauropod growth rates"
```

## Expected Outcomes

1. `Research Jurassic High` is non-empty.
2. `Research Cretaceous Not Trackway` is empty (or does not include the trackway note).
3. `Research High Not Trackway` is non-empty.
4. `Research Impossible Confidence` is empty because `Confidence` is exclusive.
5. Search for duplicated Jurassic text returns two items with distinct IDs.
