# Cross-Domain Workflow Scenario Pack

## Purpose

Validate `aglet` as a general information management tool, not only a task tracker.

This pack focuses on real-world workflows where users organize notes, evidence, records, and follow-ups using:

- categories and taxonomy
- include/exclude view logic
- exclusive category sets
- late category creation with retroactive matching
- manual assignment and subsumption
- intentionally empty result sets

## Coherent Request (Execution Scope)

Create and maintain a cross-domain workflow test pack that exercises category and view semantics with realistic data across multiple domains.

Deliverables:

1. Scenario scripts under `spec/scripts/`.
2. Expected outcome sections in each script.
3. Formal domain tests in `agenda-core` for high-value logic combinations.
4. Notes on unsupported or deferred behaviors.

Acceptance criteria:

1. At least 4 non-task domains.
2. Each scenario uses include and exclude view criteria.
3. Each scenario uses at least one exclusive category set.
4. At least 2 scenarios include intentionally empty views.
5. At least 1 scenario includes late category creation.
6. At least 1 scenario includes duplicate item text handling.

## Implemented Scenario Batch (v1)

1. `research-dinosaurs`
2. `investigative-journalism`
3. `legal-matter-intelligence`
4. `security-incident-intel`

## Script Index

- `spec/scripts/scenario-01-research-dinosaurs.md`
- `spec/scripts/scenario-02-investigative-journalism.md`
- `spec/scripts/scenario-03-legal-matter-intelligence.md`
- `spec/scripts/scenario-04-security-incident-intel.md`

## Coverage Map

| Capability | 01 Research | 02 Journalism | 03 Legal | 04 Security |
|---|---:|---:|---:|---:|
| Include + exclude views | yes | yes | yes | yes |
| Exclusive categories | yes | yes | yes | yes |
| Late category creation | yes | yes | yes | yes |
| Manual assignment | yes | yes | yes | yes |
| Intentionally empty view | yes | yes | yes | yes |
| Duplicate item text check | yes | no | yes | no |

## Notes

- These scripts are user-facing scenario tests and may include both actionable and informational records.
- Date phrases intentionally use parser-supported grammar (`tomorrow`, `this/next <weekday>`, explicit dates).
- Scripts are written as command lists; run them manually or via captured demo harness.
