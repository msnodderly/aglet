# Cross-Domain Scenario Run Results

Run date: 2026-02-16
Branch: `codex/slc-v1`

## Run Inventory

| Scenario | DB | Log |
|---|---|---|
| 01 Research | `/tmp/aglet-scenario-01-research-1771272018.ag` | `/tmp/aglet-scenario-01.log` |
| 02 Journalism | `/tmp/aglet-scenario-02-journalism-1771272035.ag` | `/tmp/aglet-scenario-02.log` |
| 03 Legal | `/tmp/aglet-scenario-03-legal-1771272052.ag` | `/tmp/aglet-scenario-03.log` |
| 04 Security | `/tmp/aglet-scenario-04-security-1771272070.ag` | `/tmp/aglet-scenario-04.log` |

## Results Matrix

### Scenario 01: Research Knowledge Base (Dinosaurs)

| Expected Outcome | Result |
|---|---|
| `Research Jurassic High` is non-empty | PASS |
| `Research Cretaceous Not Trackway` is empty or excludes trackway note | PASS |
| `Research High Not Trackway` is non-empty | PASS |
| `Research Impossible Confidence` is empty | PASS |
| Duplicate Jurassic text search returns distinct IDs | PASS |

### Scenario 02: Investigative Journalism Dossier

| Expected Outcome | Result |
|---|---|
| `Riverfront Verified LowRisk` is non-empty | PASS |
| `Riverfront Verified NotHighRisk` is non-empty | PASS |
| `Riverfront Unverified NotHighRisk` is empty | PASS |
| `Riverfront Impossible Reliability` is empty | PASS |

### Scenario 03: Legal Matter Intelligence

| Expected Outcome | Result |
|---|---|
| `Acme Discovery Privileged` is non-empty | PASS |
| `Nimbus Litigation NotOutsideCounsel` is empty | PASS |
| `Discovery NotPublicRecord` is non-empty | PASS |
| `Impossible Privilege Combination` is empty | PASS |
| Duplicate Acme hold notice search returns distinct IDs | PASS |

### Scenario 04: Security Incident Intelligence

| Expected Outcome | Result |
|---|---|
| `Security Confirmed High` is non-empty | PASS |
| `Security Critical NotConfirmed` is non-empty | PASS |
| `Security Phishing NotFalsePositive` is non-empty | PASS |
| `Security Impossible Verification` is empty | PASS |

## Mismatches / Notes

- No expectation mismatches found in this run.
- Date parsing behaved as expected for parser-supported phrases used in scripts.
- Empty views render as header-only output (`# <View Name>`) when no results match.

## Follow-up Recommendations

1. Convert these scripts into an automated harness that executes all scenarios and writes this matrix automatically.
2. Add `agenda-core` tests for one representative scenario from each domain (research, journalism, legal, security) to complement existing include/exclude tests.
3. Consider a CLI output mode that explicitly prints `(no items)` for empty view results to improve human readability.
