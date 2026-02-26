# EGFR Follow-up + Bibliography Extension

Source page: https://en.wikipedia.org/wiki/Epidermal_growth_factor_receptor#Medical_applications

Database: `/tmp/aglet-egfr-wikipedia-1771272469.ag`

```text
# Using existing EGFR research database
$ echo /tmp/aglet-egfr-wikipedia-1771272469.ag
/tmp/aglet-egfr-wikipedia-1771272469.ag
# Existing views created so far
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag view list
All Items (sections=0, include=0, exclude=0)
EGFR Antibody Strategies (sections=0, include=2, exclude=0)
EGFR Disease Notes (sections=0, include=1, exclude=0)
EGFR Drug Resistance (sections=0, include=2, exclude=0)
EGFR Imaging NSCLC (sections=0, include=2, exclude=0)
EGFR Resistance Without T790M or MET (sections=0, include=1, exclude=2)
EGFR TKI Not Resistance (sections=0, include=2, exclude=1)
hint: use `agenda view show "<name>"` to see view contents
# Additional views useful to researcher
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag view create 'EGFR Disease Not Medical Applications' --include 'Role in human disease' --exclude 'Medical applications'
created view EGFR Disease Not Medical Applications
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag view create 'EGFR Drug Target With Adverse Effects' --include 'Drug target' --include 'Adverse effects'
created view EGFR Drug Target With Adverse Effects
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag view create 'EGFR Imaging Without NSCLC' --include 'Target for imaging agents' --exclude NSCLC
created view EGFR Imaging Without NSCLC
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag view create 'EGFR Resistance + Adverse Effects' --include Resistance --include 'Adverse effects'
created view EGFR Resistance + Adverse Effects
# Create follow-up taxonomy + priorities
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category create 'Follow-up' --parent EGFR
created category Follow-up (processed_items=9, affected_items=0)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category create 'Disease follow-up' --parent 'Follow-up'
created category Disease follow-up (processed_items=9, affected_items=0)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category create 'Drug resistance follow-up' --parent 'Follow-up'
created category Drug resistance follow-up (processed_items=9, affected_items=0)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category create Priority --parent EGFR --exclusive
created category Priority (processed_items=9, affected_items=0)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category create High --parent Priority
created category High (processed_items=9, affected_items=0)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category create Medium --parent Priority
created category Medium (processed_items=9, affected_items=0)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category create Low --parent Priority
created category Low (processed_items=9, affected_items=0)
# Add follow-up research items
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag add 'Follow-up disease: quantify EGFR overexpression prevalence across TCGA cohorts (lung, glioblastoma, head and neck) tomorrow at 11am'
created 91eef5ab-91c7-4ad8-83dd-07234a4f1267
new_assignments=2
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag add 'Follow-up disease: clarify mechanisms linking EGFR signaling to psoriasis and eczema this Thursday at 3pm'
created 5388a3b5-0724-4d79-bef9-ec7b1cfa8b97
new_assignments=2
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag add 'Follow-up resistance: summarize evidence for T790M-mediated resistance trajectories in NSCLC tomorrow at 4pm'
created a922b67d-c92e-435f-baf8-c4c715fdefd0
new_assignments=4
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag add 'Follow-up resistance: map MET amplification co-resistance pathways and combination strategies next Monday at 2pm'
created da97dbf6-3480-429c-a8a6-7fa4b0586bc6
new_assignments=3
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag add 'Follow-up translational: compare monoclonal antibody versus tyrosine kinase inhibitor response predictors this Friday at 10am'
created 8616bb1b-fcca-47db-8e24-80d570e3e46f
new_assignments=2
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag add 'Follow-up imaging: validate CT pattern features predicting EGFR mutation in NSCLC this Wednesday at 1pm'
created 840d39ed-0259-425f-8830-edcf06659089
new_assignments=3
# Capture follow-up IDs
$ echo FU_DISEASE_PREV_ID=91eef5ab-91c7-4ad8-83dd-07234a4f1267
FU_DISEASE_PREV_ID=91eef5ab-91c7-4ad8-83dd-07234a4f1267
$ echo FU_DISEASE_MECH_ID=5388a3b5-0724-4d79-bef9-ec7b1cfa8b97
FU_DISEASE_MECH_ID=5388a3b5-0724-4d79-bef9-ec7b1cfa8b97
$ echo FU_RES_T790M_ID=a922b67d-c92e-435f-baf8-c4c715fdefd0
FU_RES_T790M_ID=a922b67d-c92e-435f-baf8-c4c715fdefd0
$ echo FU_RES_MET_ID=da97dbf6-3480-429c-a8a6-7fa4b0586bc6
FU_RES_MET_ID=da97dbf6-3480-429c-a8a6-7fa4b0586bc6
$ echo FU_TRANS_ID=8616bb1b-fcca-47db-8e24-80d570e3e46f
FU_TRANS_ID=8616bb1b-fcca-47db-8e24-80d570e3e46f
$ echo FU_IMG_ID=840d39ed-0259-425f-8830-edcf06659089
FU_IMG_ID=840d39ed-0259-425f-8830-edcf06659089
# Assign follow-up categories + priorities
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category assign 91eef5ab-91c7-4ad8-83dd-07234a4f1267 'Disease follow-up'
assigned item 91eef5ab-91c7-4ad8-83dd-07234a4f1267 to category Disease follow-up
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category assign 91eef5ab-91c7-4ad8-83dd-07234a4f1267 High
assigned item 91eef5ab-91c7-4ad8-83dd-07234a4f1267 to category High
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category assign 91eef5ab-91c7-4ad8-83dd-07234a4f1267 Cancer
assigned item 91eef5ab-91c7-4ad8-83dd-07234a4f1267 to category Cancer
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category assign 5388a3b5-0724-4d79-bef9-ec7b1cfa8b97 'Disease follow-up'
assigned item 5388a3b5-0724-4d79-bef9-ec7b1cfa8b97 to category Disease follow-up
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category assign 5388a3b5-0724-4d79-bef9-ec7b1cfa8b97 Medium
assigned item 5388a3b5-0724-4d79-bef9-ec7b1cfa8b97 to category Medium
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category assign 5388a3b5-0724-4d79-bef9-ec7b1cfa8b97 'Inflammatory disease'
assigned item 5388a3b5-0724-4d79-bef9-ec7b1cfa8b97 to category Inflammatory disease
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category assign a922b67d-c92e-435f-baf8-c4c715fdefd0 'Drug resistance follow-up'
assigned item a922b67d-c92e-435f-baf8-c4c715fdefd0 to category Drug resistance follow-up
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category assign a922b67d-c92e-435f-baf8-c4c715fdefd0 Resistance
assigned item a922b67d-c92e-435f-baf8-c4c715fdefd0 to category Resistance
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category assign a922b67d-c92e-435f-baf8-c4c715fdefd0 High
assigned item a922b67d-c92e-435f-baf8-c4c715fdefd0 to category High
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category assign da97dbf6-3480-429c-a8a6-7fa4b0586bc6 'Drug resistance follow-up'
assigned item da97dbf6-3480-429c-a8a6-7fa4b0586bc6 to category Drug resistance follow-up
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category assign da97dbf6-3480-429c-a8a6-7fa4b0586bc6 Resistance
assigned item da97dbf6-3480-429c-a8a6-7fa4b0586bc6 to category Resistance
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category assign da97dbf6-3480-429c-a8a6-7fa4b0586bc6 High
assigned item da97dbf6-3480-429c-a8a6-7fa4b0586bc6 to category High
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category assign 8616bb1b-fcca-47db-8e24-80d570e3e46f 'Follow-up'
assigned item 8616bb1b-fcca-47db-8e24-80d570e3e46f to category Follow-up
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category assign 8616bb1b-fcca-47db-8e24-80d570e3e46f Medium
assigned item 8616bb1b-fcca-47db-8e24-80d570e3e46f to category Medium
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category assign 8616bb1b-fcca-47db-8e24-80d570e3e46f 'Drug target'
assigned item 8616bb1b-fcca-47db-8e24-80d570e3e46f to category Drug target
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category assign 840d39ed-0259-425f-8830-edcf06659089 'Follow-up'
assigned item 840d39ed-0259-425f-8830-edcf06659089 to category Follow-up
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category assign 840d39ed-0259-425f-8830-edcf06659089 Medium
assigned item 840d39ed-0259-425f-8830-edcf06659089 to category Medium
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category assign 840d39ed-0259-425f-8830-edcf06659089 'Target for imaging agents'
assigned item 840d39ed-0259-425f-8830-edcf06659089 to category Target for imaging agents
# Create follow-up views
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag view create 'EGFR Follow-up Queue' --include 'Follow-up'
created view EGFR Follow-up Queue
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag view create 'EGFR Disease Follow-up High' --include 'Disease follow-up' --include High
created view EGFR Disease Follow-up High
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag view create 'EGFR Resistance Follow-up' --include 'Drug resistance follow-up'
created view EGFR Resistance Follow-up
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag view create 'EGFR Resistance Follow-up High' --include 'Drug resistance follow-up' --include High
created view EGFR Resistance Follow-up High
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag view create 'EGFR Follow-up Not Low' --include 'Follow-up' --exclude Low
created view EGFR Follow-up Not Low
# Create bibliography taxonomy
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category create Bibliography --parent EGFR
created category Bibliography (processed_items=15, affected_items=0)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category create 'External links' --parent Bibliography
created category External links (processed_items=15, affected_items=0)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category create 'Further reading' --parent Bibliography
created category Further reading (processed_items=15, affected_items=0)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category create 'Source type' --parent Bibliography --exclusive
created category Source type (processed_items=15, affected_items=0)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category create 'Database resource' --parent 'Source type'
created category Database resource (processed_items=15, affected_items=0)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category create 'DOI article' --parent 'Source type'
created category DOI article (processed_items=15, affected_items=0)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category create 'Disease bibliography' --parent Bibliography
created category Disease bibliography (processed_items=15, affected_items=0)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category create 'Drug resistance bibliography' --parent Bibliography
created category Drug resistance bibliography (processed_items=15, affected_items=0)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category create 'Imaging bibliography' --parent Bibliography
created category Imaging bibliography (processed_items=15, affected_items=0)
# Add bibliography items from page bottom links (External links + Further reading near bottom)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag add 'Bibliography external: MeSH Browser EGFR / ErbB Receptors https://meshb.nlm.nih.gov/record/ui?name=Epidermal+Growth+Factor+Receptor'
created 700e976f-e2f0-4ed4-a7ca-90f6d5f0a988
new_assignments=2
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag add 'Bibliography external: PDBe-KB UniProt P00533 structure overview https://www.ebi.ac.uk/pdbe/pdbe-kb/proteins/P00533'
created 77c552d3-d14f-46fc-b844-9b77f7c80cda
new_assignments=1
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag add 'Bibliography further reading: Zhang X, Chang A 2007 Somatic mutations of EGFR and NSCLC https://doi.org/10.1136/jmg.2006.046102'
created c8275b94-ec76-406f-b828-131d19c66f70
new_assignments=4
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag add 'Bibliography further reading: Mellinghoff et al. 2007 PTEN-mediated resistance to EGFR kinase inhibitors https://doi.org/10.1158/1078-0432.CCR-06-1992'
created d03a9eee-5f18-4198-b935-89594210145d
new_assignments=4
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag add 'Bibliography further reading: Nakamura JL 2007 EGFR in malignant gliomas https://doi.org/10.1517/14728222.11.4.463'
created 12dd29c3-d9b8-495d-893c-1177a7e70d35
new_assignments=3
# Capture bibliography IDs
$ echo BIB_MESH_ID=700e976f-e2f0-4ed4-a7ca-90f6d5f0a988
BIB_MESH_ID=700e976f-e2f0-4ed4-a7ca-90f6d5f0a988
$ echo BIB_PDBE_ID=77c552d3-d14f-46fc-b844-9b77f7c80cda
BIB_PDBE_ID=77c552d3-d14f-46fc-b844-9b77f7c80cda
$ echo BIB_NSCLC_ID=c8275b94-ec76-406f-b828-131d19c66f70
BIB_NSCLC_ID=c8275b94-ec76-406f-b828-131d19c66f70
$ echo BIB_RES_ID=d03a9eee-5f18-4198-b935-89594210145d
BIB_RES_ID=d03a9eee-5f18-4198-b935-89594210145d
$ echo BIB_GLIO_ID=12dd29c3-d9b8-495d-893c-1177a7e70d35
BIB_GLIO_ID=12dd29c3-d9b8-495d-893c-1177a7e70d35
# Assign bibliography categories
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category assign 700e976f-e2f0-4ed4-a7ca-90f6d5f0a988 Bibliography
assigned item 700e976f-e2f0-4ed4-a7ca-90f6d5f0a988 to category Bibliography
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category assign 700e976f-e2f0-4ed4-a7ca-90f6d5f0a988 'External links'
assigned item 700e976f-e2f0-4ed4-a7ca-90f6d5f0a988 to category External links
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category assign 700e976f-e2f0-4ed4-a7ca-90f6d5f0a988 'Database resource'
assigned item 700e976f-e2f0-4ed4-a7ca-90f6d5f0a988 to category Database resource
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category assign 77c552d3-d14f-46fc-b844-9b77f7c80cda Bibliography
assigned item 77c552d3-d14f-46fc-b844-9b77f7c80cda to category Bibliography
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category assign 77c552d3-d14f-46fc-b844-9b77f7c80cda 'External links'
assigned item 77c552d3-d14f-46fc-b844-9b77f7c80cda to category External links
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category assign 77c552d3-d14f-46fc-b844-9b77f7c80cda 'Database resource'
assigned item 77c552d3-d14f-46fc-b844-9b77f7c80cda to category Database resource
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category assign 77c552d3-d14f-46fc-b844-9b77f7c80cda 'Imaging bibliography'
assigned item 77c552d3-d14f-46fc-b844-9b77f7c80cda to category Imaging bibliography
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category assign c8275b94-ec76-406f-b828-131d19c66f70 Bibliography
assigned item c8275b94-ec76-406f-b828-131d19c66f70 to category Bibliography
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category assign c8275b94-ec76-406f-b828-131d19c66f70 'Further reading'
assigned item c8275b94-ec76-406f-b828-131d19c66f70 to category Further reading
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category assign c8275b94-ec76-406f-b828-131d19c66f70 'DOI article'
assigned item c8275b94-ec76-406f-b828-131d19c66f70 to category DOI article
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category assign c8275b94-ec76-406f-b828-131d19c66f70 'Disease bibliography'
assigned item c8275b94-ec76-406f-b828-131d19c66f70 to category Disease bibliography
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category assign d03a9eee-5f18-4198-b935-89594210145d Bibliography
assigned item d03a9eee-5f18-4198-b935-89594210145d to category Bibliography
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category assign d03a9eee-5f18-4198-b935-89594210145d 'Further reading'
assigned item d03a9eee-5f18-4198-b935-89594210145d to category Further reading
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category assign d03a9eee-5f18-4198-b935-89594210145d 'DOI article'
assigned item d03a9eee-5f18-4198-b935-89594210145d to category DOI article
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category assign d03a9eee-5f18-4198-b935-89594210145d 'Drug resistance bibliography'
assigned item d03a9eee-5f18-4198-b935-89594210145d to category Drug resistance bibliography
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category assign 12dd29c3-d9b8-495d-893c-1177a7e70d35 Bibliography
assigned item 12dd29c3-d9b8-495d-893c-1177a7e70d35 to category Bibliography
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category assign 12dd29c3-d9b8-495d-893c-1177a7e70d35 'Further reading'
assigned item 12dd29c3-d9b8-495d-893c-1177a7e70d35 to category Further reading
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category assign 12dd29c3-d9b8-495d-893c-1177a7e70d35 'DOI article'
assigned item 12dd29c3-d9b8-495d-893c-1177a7e70d35 to category DOI article
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category assign 12dd29c3-d9b8-495d-893c-1177a7e70d35 'Disease bibliography'
assigned item 12dd29c3-d9b8-495d-893c-1177a7e70d35 to category Disease bibliography
# Create bibliography views
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag view create 'EGFR Bibliography' --include Bibliography
created view EGFR Bibliography
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag view create 'EGFR Bibliography External Resources' --include Bibliography --include 'External links'
created view EGFR Bibliography External Resources
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag view create 'EGFR Bibliography Drug Resistance' --include Bibliography --include 'Drug resistance bibliography'
created view EGFR Bibliography Drug Resistance
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag view create 'EGFR Bibliography DOI Not Resistance' --include Bibliography --include 'DOI article' --exclude 'Drug resistance bibliography'
created view EGFR Bibliography DOI Not Resistance
# Inspect follow-up and bibliography views
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag view show 'EGFR Follow-up Queue'
# EGFR Follow-up Queue

## Unassigned
840d39ed-0259-425f-8830-edcf06659089 | open | 2026-02-18 13:00:00 | Follow-up imaging: validate CT pattern features predicting EGFR mutation in NSCLC this Wednesday at 1pm
  categories: EGFR, Follow-up, Medical applications, Medium, NSCLC, Priority, Target for imaging agents, When
8616bb1b-fcca-47db-8e24-80d570e3e46f | open | 2026-02-20 10:00:00 | Follow-up translational: compare monoclonal antibody versus tyrosine kinase inhibitor response predictors this Friday at 10am
  categories: Drug target, EGFR, Follow-up, Medical applications, Medium, Priority, tyrosine kinase, When
da97dbf6-3480-429c-a8a6-7fa4b0586bc6 | open | 2026-02-23 14:00:00 | Follow-up resistance: map MET amplification co-resistance pathways and combination strategies next Monday at 2pm
  categories: Drug resistance follow-up, Drug target, EGFR, Follow-up, High, Medical applications, MET, Priority, Resistance, When
a922b67d-c92e-435f-baf8-c4c715fdefd0 | open | 2026-02-17 16:00:00 | Follow-up resistance: summarize evidence for T790M-mediated resistance trajectories in NSCLC tomorrow at 4pm
  categories: Drug resistance follow-up, Drug target, EGFR, Follow-up, High, Medical applications, NSCLC, Priority, Resistance, T790M, Target for imaging agents, When
5388a3b5-0724-4d79-bef9-ec7b1cfa8b97 | open | 2026-02-19 15:00:00 | Follow-up disease: clarify mechanisms linking EGFR signaling to psoriasis and eczema this Thursday at 3pm
  categories: Disease follow-up, EGFR, Follow-up, Inflammatory disease, Medium, Priority, Role in human disease, When
91eef5ab-91c7-4ad8-83dd-07234a4f1267 | open | 2026-02-17 11:00:00 | Follow-up disease: quantify EGFR overexpression prevalence across TCGA cohorts (lung, glioblastoma, head and neck) tomorrow at 11am
  categories: Cancer, Disease follow-up, EGFR, Follow-up, High, Priority, Role in human disease, When
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag view show 'EGFR Disease Follow-up High'
# EGFR Disease Follow-up High

## Unassigned
91eef5ab-91c7-4ad8-83dd-07234a4f1267 | open | 2026-02-17 11:00:00 | Follow-up disease: quantify EGFR overexpression prevalence across TCGA cohorts (lung, glioblastoma, head and neck) tomorrow at 11am
  categories: Cancer, Disease follow-up, EGFR, Follow-up, High, Priority, Role in human disease, When
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag view show 'EGFR Resistance Follow-up'
# EGFR Resistance Follow-up

## Unassigned
da97dbf6-3480-429c-a8a6-7fa4b0586bc6 | open | 2026-02-23 14:00:00 | Follow-up resistance: map MET amplification co-resistance pathways and combination strategies next Monday at 2pm
  categories: Drug resistance follow-up, Drug target, EGFR, Follow-up, High, Medical applications, MET, Priority, Resistance, When
a922b67d-c92e-435f-baf8-c4c715fdefd0 | open | 2026-02-17 16:00:00 | Follow-up resistance: summarize evidence for T790M-mediated resistance trajectories in NSCLC tomorrow at 4pm
  categories: Drug resistance follow-up, Drug target, EGFR, Follow-up, High, Medical applications, NSCLC, Priority, Resistance, T790M, Target for imaging agents, When
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag view show 'EGFR Resistance Follow-up High'
# EGFR Resistance Follow-up High

## Unassigned
da97dbf6-3480-429c-a8a6-7fa4b0586bc6 | open | 2026-02-23 14:00:00 | Follow-up resistance: map MET amplification co-resistance pathways and combination strategies next Monday at 2pm
  categories: Drug resistance follow-up, Drug target, EGFR, Follow-up, High, Medical applications, MET, Priority, Resistance, When
a922b67d-c92e-435f-baf8-c4c715fdefd0 | open | 2026-02-17 16:00:00 | Follow-up resistance: summarize evidence for T790M-mediated resistance trajectories in NSCLC tomorrow at 4pm
  categories: Drug resistance follow-up, Drug target, EGFR, Follow-up, High, Medical applications, NSCLC, Priority, Resistance, T790M, Target for imaging agents, When
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag view show 'EGFR Bibliography'
# EGFR Bibliography

## Unassigned
12dd29c3-d9b8-495d-893c-1177a7e70d35 | open | - | Bibliography further reading: Nakamura JL 2007 EGFR in malignant gliomas https://doi.org/10.1517/14728222.11.4.463
  categories: Bibliography, Disease bibliography, DOI article, EGFR, Further reading, Source type
d03a9eee-5f18-4198-b935-89594210145d | open | - | Bibliography further reading: Mellinghoff et al. 2007 PTEN-mediated resistance to EGFR kinase inhibitors https://doi.org/10.1158/1078-0432.CCR-06-1992
  categories: Bibliography, DOI article, Drug resistance bibliography, Drug target, EGFR, Further reading, Medical applications, Resistance, Source type
c8275b94-ec76-406f-b828-131d19c66f70 | open | - | Bibliography further reading: Zhang X, Chang A 2007 Somatic mutations of EGFR and NSCLC https://doi.org/10.1136/jmg.2006.046102
  categories: Bibliography, Disease bibliography, DOI article, EGFR, Further reading, Medical applications, NSCLC, Source type, Target for imaging agents
77c552d3-d14f-46fc-b844-9b77f7c80cda | open | - | Bibliography external: PDBe-KB UniProt P00533 structure overview https://www.ebi.ac.uk/pdbe/pdbe-kb/proteins/P00533
  categories: Bibliography, Database resource, EGFR, External links, Imaging bibliography, Source type
700e976f-e2f0-4ed4-a7ca-90f6d5f0a988 | open | - | Bibliography external: MeSH Browser EGFR / ErbB Receptors https://meshb.nlm.nih.gov/record/ui?name=Epidermal+Growth+Factor+Receptor
  categories: Bibliography, Database resource, EGFR, External links, Source type
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag view show 'EGFR Bibliography External Resources'
# EGFR Bibliography External Resources

## Unassigned
77c552d3-d14f-46fc-b844-9b77f7c80cda | open | - | Bibliography external: PDBe-KB UniProt P00533 structure overview https://www.ebi.ac.uk/pdbe/pdbe-kb/proteins/P00533
  categories: Bibliography, Database resource, EGFR, External links, Imaging bibliography, Source type
700e976f-e2f0-4ed4-a7ca-90f6d5f0a988 | open | - | Bibliography external: MeSH Browser EGFR / ErbB Receptors https://meshb.nlm.nih.gov/record/ui?name=Epidermal+Growth+Factor+Receptor
  categories: Bibliography, Database resource, EGFR, External links, Source type
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag view show 'EGFR Bibliography Drug Resistance'
# EGFR Bibliography Drug Resistance

## Unassigned
d03a9eee-5f18-4198-b935-89594210145d | open | - | Bibliography further reading: Mellinghoff et al. 2007 PTEN-mediated resistance to EGFR kinase inhibitors https://doi.org/10.1158/1078-0432.CCR-06-1992
  categories: Bibliography, DOI article, Drug resistance bibliography, Drug target, EGFR, Further reading, Medical applications, Resistance, Source type
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag view show 'EGFR Bibliography DOI Not Resistance'
# EGFR Bibliography DOI Not Resistance

## Unassigned
12dd29c3-d9b8-495d-893c-1177a7e70d35 | open | - | Bibliography further reading: Nakamura JL 2007 EGFR in malignant gliomas https://doi.org/10.1517/14728222.11.4.463
  categories: Bibliography, Disease bibliography, DOI article, EGFR, Further reading, Source type
c8275b94-ec76-406f-b828-131d19c66f70 | open | - | Bibliography further reading: Zhang X, Chang A 2007 Somatic mutations of EGFR and NSCLC https://doi.org/10.1136/jmg.2006.046102
  categories: Bibliography, Disease bibliography, DOI article, EGFR, Further reading, Medical applications, NSCLC, Source type, Target for imaging agents
# Final view inventory
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag view list
All Items (sections=0, include=0, exclude=0)
EGFR Antibody Strategies (sections=0, include=2, exclude=0)
EGFR Bibliography (sections=0, include=1, exclude=0)
EGFR Bibliography DOI Not Resistance (sections=0, include=2, exclude=1)
EGFR Bibliography Drug Resistance (sections=0, include=2, exclude=0)
EGFR Bibliography External Resources (sections=0, include=2, exclude=0)
EGFR Disease Follow-up High (sections=0, include=2, exclude=0)
EGFR Disease Not Medical Applications (sections=0, include=1, exclude=1)
EGFR Disease Notes (sections=0, include=1, exclude=0)
EGFR Drug Resistance (sections=0, include=2, exclude=0)
EGFR Drug Target With Adverse Effects (sections=0, include=2, exclude=0)
EGFR Follow-up Not Low (sections=0, include=1, exclude=1)
EGFR Follow-up Queue (sections=0, include=1, exclude=0)
EGFR Imaging NSCLC (sections=0, include=2, exclude=0)
EGFR Imaging Without NSCLC (sections=0, include=1, exclude=1)
EGFR Resistance + Adverse Effects (sections=0, include=2, exclude=0)
EGFR Resistance Follow-up (sections=0, include=1, exclude=0)
EGFR Resistance Follow-up High (sections=0, include=2, exclude=0)
EGFR Resistance Without T790M or MET (sections=0, include=1, exclude=2)
EGFR TKI Not Resistance (sections=0, include=2, exclude=1)
hint: use `agenda view show "<name>"` to see view contents
```
