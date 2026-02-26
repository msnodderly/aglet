# View Logic Demo Run

Database: `/tmp/aglet-scenario-work-personal-1771270781.ag`

```text
# Using existing scenario database
$ echo /tmp/aglet-scenario-work-personal-1771270781.ag
/tmp/aglet-scenario-work-personal-1771270781.ag
# Ensure Alice category exists so we can test include/exclude with Miguel
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-scenario-work-personal-1771270781.ag category create Alice --parent Work
created category Alice (processed_items=11, affected_items=0)
# Create a collaboration item that includes both Miguel and Alice
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-scenario-work-personal-1771270781.ag add 'Project Atlas: Miguel and Alice triage defects tomorrow at noon'
created e0fe7230-b5dc-4728-8f1e-197da191b9a5
new_assignments=3
$ echo PAIR_ID=e0fe7230-b5dc-4728-8f1e-197da191b9a5
PAIR_ID=e0fe7230-b5dc-4728-8f1e-197da191b9a5
# Ensure it has explicit priority for atlas-high views
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-scenario-work-personal-1771270781.ag category assign e0fe7230-b5dc-4728-8f1e-197da191b9a5 High
assigned item e0fe7230-b5dc-4728-8f1e-197da191b9a5 to category High
# Create demo views to exercise include/exclude logic
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-scenario-work-personal-1771270781.ag view create 'Demo Miguel Work' --include Work --include Miguel
created view Demo Miguel Work
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-scenario-work-personal-1771270781.ag view create 'Demo Miguel Without Alice' --include Work --include Miguel --exclude Alice
created view Demo Miguel Without Alice
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-scenario-work-personal-1771270781.ag view create 'Demo Atlas High' --include 'Project Atlas' --include High
created view Demo Atlas High
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-scenario-work-personal-1771270781.ag view create 'Demo Atlas High Not Sarah' --include 'Project Atlas' --include High --exclude Sarah
created view Demo Atlas High Not Sarah
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-scenario-work-personal-1771270781.ag view create 'Demo High Without Priority (Empty)' --include High --exclude Priority
created view Demo High Without Priority (Empty)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-scenario-work-personal-1771270781.ag view create 'Demo Cicada Without Priya (Empty)' --include 'Project Cicada' --exclude Priya
created view Demo Cicada Without Priya (Empty)
# Show all new views
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-scenario-work-personal-1771270781.ag view show 'Demo Miguel Work'
# Demo Miguel Work

## Unassigned
e0fe7230-b5dc-4728-8f1e-197da191b9a5 | open | 2026-02-17 12:00:00 | Project Atlas: Miguel and Alice triage defects tomorrow at noon
  categories: Alice, High, Miguel, Priority, Project Atlas, When, Work
62cac780-0d47-4143-b8d5-3d72de568102 | open | - | Project Delta: Miguel close open QA defects
  categories: Medium, Miguel, Priority, Project Delta, Work
7fe51615-bfa2-4717-8d8a-8af50cfda73d | open | - | Project Atlas: Miguel draft rollout checklist
  categories: High, Miguel, Priority, Project Atlas, Work
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-scenario-work-personal-1771270781.ag view show 'Demo Miguel Without Alice'
# Demo Miguel Without Alice

## Unassigned
62cac780-0d47-4143-b8d5-3d72de568102 | open | - | Project Delta: Miguel close open QA defects
  categories: Medium, Miguel, Priority, Project Delta, Work
7fe51615-bfa2-4717-8d8a-8af50cfda73d | open | - | Project Atlas: Miguel draft rollout checklist
  categories: High, Miguel, Priority, Project Atlas, Work
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-scenario-work-personal-1771270781.ag view show 'Demo Atlas High'
# Demo Atlas High

## Unassigned
e0fe7230-b5dc-4728-8f1e-197da191b9a5 | open | 2026-02-17 12:00:00 | Project Atlas: Miguel and Alice triage defects tomorrow at noon
  categories: Alice, High, Miguel, Priority, Project Atlas, When, Work
7fe51615-bfa2-4717-8d8a-8af50cfda73d | open | - | Project Atlas: Miguel draft rollout checklist
  categories: High, Miguel, Priority, Project Atlas, Work
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-scenario-work-personal-1771270781.ag view show 'Demo Atlas High Not Sarah'
# Demo Atlas High Not Sarah

## Unassigned
e0fe7230-b5dc-4728-8f1e-197da191b9a5 | open | 2026-02-17 12:00:00 | Project Atlas: Miguel and Alice triage defects tomorrow at noon
  categories: Alice, High, Miguel, Priority, Project Atlas, When, Work
7fe51615-bfa2-4717-8d8a-8af50cfda73d | open | - | Project Atlas: Miguel draft rollout checklist
  categories: High, Miguel, Priority, Project Atlas, Work
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-scenario-work-personal-1771270781.ag view show 'Demo High Without Priority (Empty)'
# Demo High Without Priority (Empty)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-scenario-work-personal-1771270781.ag view show 'Demo Cicada Without Priya (Empty)'
# Demo Cicada Without Priya (Empty)
# List views for quick inventory
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-scenario-work-personal-1771270781.ag view list
All Items (sections=0, include=0, exclude=0)
Demo Atlas High (sections=0, include=2, exclude=0)
Demo Atlas High Not Sarah (sections=0, include=2, exclude=1)
Demo Cicada Without Priya (Empty) (sections=0, include=1, exclude=1)
Demo High Without Priority (Empty) (sections=0, include=1, exclude=1)
Demo Miguel Without Alice (sections=0, include=2, exclude=1)
Demo Miguel Work (sections=0, include=2, exclude=0)
High Priority View (sections=0, include=1, exclude=0)
Personal View (sections=0, include=1, exclude=0)
Work View (sections=0, include=1, exclude=0)
hint: use `agenda view show "<name>"` to see view contents
# Add a Sarah+High Atlas item so exclude Sarah changes result set
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-scenario-work-personal-1771270781.ag add 'Project Atlas: Sarah high-priority production hotfix tomorrow at 4pm'
created 8e19b5c3-019f-453a-a4d1-6e5e8c2f0be0
new_assignments=4
$ echo SARAH_HIGH_ID=8e19b5c3-019f-453a-a4d1-6e5e8c2f0be0
SARAH_HIGH_ID=8e19b5c3-019f-453a-a4d1-6e5e8c2f0be0
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-scenario-work-personal-1771270781.ag category assign 8e19b5c3-019f-453a-a4d1-6e5e8c2f0be0 High
assigned item 8e19b5c3-019f-453a-a4d1-6e5e8c2f0be0 to category High
# Re-show Atlas high views to prove exclude Sarah logic
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-scenario-work-personal-1771270781.ag view show 'Demo Atlas High'
# Demo Atlas High

## Unassigned
8e19b5c3-019f-453a-a4d1-6e5e8c2f0be0 | open | 2026-02-17 16:00:00 | Project Atlas: Sarah high-priority production hotfix tomorrow at 4pm
  categories: High, Priority, Project Atlas, Sarah, When, Work
e0fe7230-b5dc-4728-8f1e-197da191b9a5 | open | 2026-02-17 12:00:00 | Project Atlas: Miguel and Alice triage defects tomorrow at noon
  categories: Alice, High, Miguel, Priority, Project Atlas, When, Work
7fe51615-bfa2-4717-8d8a-8af50cfda73d | open | - | Project Atlas: Miguel draft rollout checklist
  categories: High, Miguel, Priority, Project Atlas, Work
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-scenario-work-personal-1771270781.ag view show 'Demo Atlas High Not Sarah'
# Demo Atlas High Not Sarah

## Unassigned
e0fe7230-b5dc-4728-8f1e-197da191b9a5 | open | 2026-02-17 12:00:00 | Project Atlas: Miguel and Alice triage defects tomorrow at noon
  categories: Alice, High, Miguel, Priority, Project Atlas, When, Work
7fe51615-bfa2-4717-8d8a-8af50cfda73d | open | - | Project Atlas: Miguel draft rollout checklist
  categories: High, Miguel, Priority, Project Atlas, Work
```
