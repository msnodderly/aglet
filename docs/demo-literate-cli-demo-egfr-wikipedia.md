# EGFR Wikipedia Research Workflow Demo

Source: https://en.wikipedia.org/wiki/Epidermal_growth_factor_receptor#Medical_applications

Database: `/tmp/aglet-egfr-wikipedia-1771272469.ag`

```text
# Source page used for notes
$ echo https://en.wikipedia.org/wiki/Epidermal_growth_factor_receptor#Medical_applications
https://en.wikipedia.org/wiki/Epidermal_growth_factor_receptor#Medical_applications
# Working database
$ echo /tmp/aglet-egfr-wikipedia-1771272469.ag
/tmp/aglet-egfr-wikipedia-1771272469.ag
$ rm -f /tmp/aglet-egfr-wikipedia-1771272469.ag
# Pass 1: create broad taxonomy from headings
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category create EGFR
created category EGFR (processed_items=0, affected_items=0)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category create Function --parent EGFR
created category Function (processed_items=0, affected_items=0)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category create 'Biological roles' --parent EGFR
created category Biological roles (processed_items=0, affected_items=0)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category create 'Role in human disease' --parent EGFR
created category Role in human disease (processed_items=0, affected_items=0)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category create 'Medical applications' --parent EGFR
created category Medical applications (processed_items=0, affected_items=0)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category create Interactions --parent EGFR
created category Interactions (processed_items=0, affected_items=0)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category create Cancer --parent 'Role in human disease'
created category Cancer (processed_items=0, affected_items=0)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category create 'Inflammatory disease' --parent 'Role in human disease'
created category Inflammatory disease (processed_items=0, affected_items=0)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category create 'Monogenic disease' --parent 'Role in human disease'
created category Monogenic disease (processed_items=0, affected_items=0)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category create 'Wound healing and fibrosis' --parent 'Role in human disease'
created category Wound healing and fibrosis (processed_items=0, affected_items=0)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category create 'Drug target' --parent 'Medical applications'
created category Drug target (processed_items=0, affected_items=0)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category create 'Target for imaging agents' --parent 'Medical applications'
created category Target for imaging agents (processed_items=0, affected_items=0)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category list
- Done [no-implicit-string]
- EGFR
  - Function
  - Biological roles
  - Role in human disease
    - Cancer
    - Inflammatory disease
    - Monogenic disease
    - Wound healing and fibrosis
  - Medical applications
    - Drug target
    - Target for imaging agents
  - Interactions
- Entry [no-implicit-string]
- When [no-implicit-string]
# Pass 2: add detailed notes as items
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag add 'Cancer: EGFR overexpression/amplification is associated with lung adenocarcinoma (~40%), glioblastoma (~50%), and head/neck epithelial tumors (80-100%).'
created 8a41c8ed-5f8e-4805-beac-207075d74f90
new_assignments=2
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag add 'Cancer: EGFRvIII mutation is often observed in glioblastoma.'
created 5786039e-01c8-4864-8cf0-1d0af9325be6
new_assignments=1
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag add 'Inflammatory disease: aberrant EGFR signaling has been implicated in psoriasis, eczema, and atherosclerosis, but roles remain ill-defined.'
created e5b17c39-a11c-4678-8e31-1392267c2225
new_assignments=2
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag add 'Monogenic disease: homozygous EGFR loss-of-function in a child caused multi-organ epithelial inflammation with rash, diarrhoea, hair/breathing/electrolyte issues.'
created 101b498d-01d9-4400-9b65-00dbd9df26cd
new_assignments=3
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag add 'Wound healing and fibrosis: EGFR contributes to TGF-beta1-dependent fibroblast-to-myofibroblast differentiation; persistent myofibroblasts can drive fibrosis.'
created e8658d41-359e-4f98-9fe4-edf8e5ec8a20
new_assignments=2
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag add 'Drug target: EGFR inhibitors include gefitinib, erlotinib, afatinib, brigatinib, icotinib (lung cancer) and cetuximab (colon cancer). Osimertinib is a third-generation tyrosine kinase inhibitor.'
created 8007160e-8d8d-431e-bf53-88a2533196d9
new_assignments=3
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag add 'Drug target: monoclonal antibodies (cetuximab/panitumumab) block the extracellular ligand-binding domain, while small molecules inhibit intracellular tyrosine kinase activity.'
created a27adede-bdcd-49ce-8627-b6466bcd3f28
new_assignments=1
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag add 'Drug target: resistance includes T790M mutation and MET oncogene; papulopustular rash is common (>90%), and EGFR-positive patients had about 60% response vs conventional chemotherapy.'
created 49f63f23-02f5-48b6-95dc-448c2b2c81d2
new_assignments=2
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag add 'Target for imaging agents: labeled EGF can identify EGFR-dependent cancers in vivo; certain CT patterns may predict EGFR mutation in NSCLC.'
created 62a5c86b-906d-4830-a76d-cc6d998efe24
new_assignments=2
# Capture item IDs
$ echo CANCER_ID=8a41c8ed-5f8e-4805-beac-207075d74f90
CANCER_ID=8a41c8ed-5f8e-4805-beac-207075d74f90
$ echo EGFRVIII_ID=5786039e-01c8-4864-8cf0-1d0af9325be6
EGFRVIII_ID=5786039e-01c8-4864-8cf0-1d0af9325be6
$ echo INFLAM_ID=e5b17c39-a11c-4678-8e31-1392267c2225
INFLAM_ID=e5b17c39-a11c-4678-8e31-1392267c2225
$ echo MONO_ID=101b498d-01d9-4400-9b65-00dbd9df26cd
MONO_ID=101b498d-01d9-4400-9b65-00dbd9df26cd
$ echo FIBRO_ID=e8658d41-359e-4f98-9fe4-edf8e5ec8a20
FIBRO_ID=e8658d41-359e-4f98-9fe4-edf8e5ec8a20
$ echo DRUG1_ID=8007160e-8d8d-431e-bf53-88a2533196d9
DRUG1_ID=8007160e-8d8d-431e-bf53-88a2533196d9
$ echo DRUG2_ID=a27adede-bdcd-49ce-8627-b6466bcd3f28
DRUG2_ID=a27adede-bdcd-49ce-8627-b6466bcd3f28
$ echo RESIST_ID=49f63f23-02f5-48b6-95dc-448c2b2c81d2
RESIST_ID=49f63f23-02f5-48b6-95dc-448c2b2c81d2
$ echo IMG_ID=62a5c86b-906d-4830-a76d-cc6d998efe24
IMG_ID=62a5c86b-906d-4830-a76d-cc6d998efe24
# Manual assignments to broad section categories
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category assign 8a41c8ed-5f8e-4805-beac-207075d74f90 'Role in human disease'
assigned item 8a41c8ed-5f8e-4805-beac-207075d74f90 to category Role in human disease
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category assign 5786039e-01c8-4864-8cf0-1d0af9325be6 'Role in human disease'
assigned item 5786039e-01c8-4864-8cf0-1d0af9325be6 to category Role in human disease
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category assign e5b17c39-a11c-4678-8e31-1392267c2225 'Role in human disease'
assigned item e5b17c39-a11c-4678-8e31-1392267c2225 to category Role in human disease
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category assign 101b498d-01d9-4400-9b65-00dbd9df26cd 'Role in human disease'
assigned item 101b498d-01d9-4400-9b65-00dbd9df26cd to category Role in human disease
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category assign e8658d41-359e-4f98-9fe4-edf8e5ec8a20 'Role in human disease'
assigned item e8658d41-359e-4f98-9fe4-edf8e5ec8a20 to category Role in human disease
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category assign 8007160e-8d8d-431e-bf53-88a2533196d9 'Medical applications'
assigned item 8007160e-8d8d-431e-bf53-88a2533196d9 to category Medical applications
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category assign a27adede-bdcd-49ce-8627-b6466bcd3f28 'Medical applications'
assigned item a27adede-bdcd-49ce-8627-b6466bcd3f28 to category Medical applications
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category assign 49f63f23-02f5-48b6-95dc-448c2b2c81d2 'Medical applications'
assigned item 49f63f23-02f5-48b6-95dc-448c2b2c81d2 to category Medical applications
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category assign 62a5c86b-906d-4830-a76d-cc6d998efe24 'Medical applications'
assigned item 62a5c86b-906d-4830-a76d-cc6d998efe24 to category Medical applications
# Inspect notes after pass 2
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag list --include-done
62a5c86b-906d-4830-a76d-cc6d998efe24 | open | - | Target for imaging agents: labeled EGF can identify EGFR-dependent cancers in vivo; certain CT patterns may predict EGFR mutation in NSCLC.
  categories: EGFR, Medical applications, Target for imaging agents
49f63f23-02f5-48b6-95dc-448c2b2c81d2 | open | - | Drug target: resistance includes T790M mutation and MET oncogene; papulopustular rash is common (>90%), and EGFR-positive patients had about 60% response vs conventional chemotherapy.
  categories: Drug target, EGFR, Medical applications
a27adede-bdcd-49ce-8627-b6466bcd3f28 | open | - | Drug target: monoclonal antibodies (cetuximab/panitumumab) block the extracellular ligand-binding domain, while small molecules inhibit intracellular tyrosine kinase activity.
  categories: Drug target, EGFR, Medical applications
8007160e-8d8d-431e-bf53-88a2533196d9 | open | - | Drug target: EGFR inhibitors include gefitinib, erlotinib, afatinib, brigatinib, icotinib (lung cancer) and cetuximab (colon cancer). Osimertinib is a third-generation tyrosine kinase inhibitor.
  categories: Cancer, Drug target, EGFR, Medical applications, Role in human disease
e8658d41-359e-4f98-9fe4-edf8e5ec8a20 | open | - | Wound healing and fibrosis: EGFR contributes to TGF-beta1-dependent fibroblast-to-myofibroblast differentiation; persistent myofibroblasts can drive fibrosis.
  categories: EGFR, Role in human disease, Wound healing and fibrosis
101b498d-01d9-4400-9b65-00dbd9df26cd | open | - | Monogenic disease: homozygous EGFR loss-of-function in a child caused multi-organ epithelial inflammation with rash, diarrhoea, hair/breathing/electrolyte issues.
  categories: EGFR, Function, Monogenic disease, Role in human disease
e5b17c39-a11c-4678-8e31-1392267c2225 | open | - | Inflammatory disease: aberrant EGFR signaling has been implicated in psoriasis, eczema, and atherosclerosis, but roles remain ill-defined.
  categories: EGFR, Inflammatory disease, Role in human disease
5786039e-01c8-4864-8cf0-1d0af9325be6 | open | - | Cancer: EGFRvIII mutation is often observed in glioblastoma.
  categories: Cancer, EGFR, Role in human disease
8a41c8ed-5f8e-4805-beac-207075d74f90 | open | - | Cancer: EGFR overexpression/amplification is associated with lung adenocarcinoma (~40%), glioblastoma (~50%), and head/neck epithelial tumors (80-100%).
  categories: Cancer, EGFR, Role in human disease
# Pass 3: add concept categories retroactively (expect auto-assignment to existing notes)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category create Resistance --parent 'Drug target'
created category Resistance (processed_items=9, affected_items=1)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category create 'Adverse effects' --parent 'Drug target'
created category Adverse effects (processed_items=9, affected_items=0)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category create T790M --parent Resistance
created category T790M (processed_items=9, affected_items=1)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category create MET --parent Resistance
created category MET (processed_items=9, affected_items=1)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category create 'papulopustular rash' --parent 'Adverse effects'
created category papulopustular rash (processed_items=9, affected_items=1)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category create osimertinib --parent 'Drug target'
created category osimertinib (processed_items=9, affected_items=1)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category create 'monoclonal antibodies' --parent 'Drug target'
created category monoclonal antibodies (processed_items=9, affected_items=1)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category create 'tyrosine kinase' --parent 'Drug target'
created category tyrosine kinase (processed_items=9, affected_items=2)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category create NSCLC --parent 'Target for imaging agents'
created category NSCLC (processed_items=9, affected_items=1)
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag category list
- Done [no-implicit-string]
- EGFR
  - Function
  - Biological roles
  - Role in human disease
    - Cancer
    - Inflammatory disease
    - Monogenic disease
    - Wound healing and fibrosis
  - Medical applications
    - Drug target
      - Resistance
        - T790M
        - MET
      - Adverse effects
        - papulopustular rash
      - osimertinib
      - monoclonal antibodies
      - tyrosine kinase
    - Target for imaging agents
      - NSCLC
  - Interactions
- Entry [no-implicit-string]
- When [no-implicit-string]
# Show how retroactive categories attached to existing notes
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag list --include-done
62a5c86b-906d-4830-a76d-cc6d998efe24 | open | - | Target for imaging agents: labeled EGF can identify EGFR-dependent cancers in vivo; certain CT patterns may predict EGFR mutation in NSCLC.
  categories: EGFR, Medical applications, NSCLC, Target for imaging agents
49f63f23-02f5-48b6-95dc-448c2b2c81d2 | open | - | Drug target: resistance includes T790M mutation and MET oncogene; papulopustular rash is common (>90%), and EGFR-positive patients had about 60% response vs conventional chemotherapy.
  categories: Adverse effects, Drug target, EGFR, Medical applications, MET, papulopustular rash, Resistance, T790M
a27adede-bdcd-49ce-8627-b6466bcd3f28 | open | - | Drug target: monoclonal antibodies (cetuximab/panitumumab) block the extracellular ligand-binding domain, while small molecules inhibit intracellular tyrosine kinase activity.
  categories: Drug target, EGFR, Medical applications, monoclonal antibodies, tyrosine kinase
8007160e-8d8d-431e-bf53-88a2533196d9 | open | - | Drug target: EGFR inhibitors include gefitinib, erlotinib, afatinib, brigatinib, icotinib (lung cancer) and cetuximab (colon cancer). Osimertinib is a third-generation tyrosine kinase inhibitor.
  categories: Cancer, Drug target, EGFR, Medical applications, osimertinib, Role in human disease, tyrosine kinase
e8658d41-359e-4f98-9fe4-edf8e5ec8a20 | open | - | Wound healing and fibrosis: EGFR contributes to TGF-beta1-dependent fibroblast-to-myofibroblast differentiation; persistent myofibroblasts can drive fibrosis.
  categories: EGFR, Role in human disease, Wound healing and fibrosis
101b498d-01d9-4400-9b65-00dbd9df26cd | open | - | Monogenic disease: homozygous EGFR loss-of-function in a child caused multi-organ epithelial inflammation with rash, diarrhoea, hair/breathing/electrolyte issues.
  categories: EGFR, Function, Monogenic disease, Role in human disease
e5b17c39-a11c-4678-8e31-1392267c2225 | open | - | Inflammatory disease: aberrant EGFR signaling has been implicated in psoriasis, eczema, and atherosclerosis, but roles remain ill-defined.
  categories: EGFR, Inflammatory disease, Role in human disease
5786039e-01c8-4864-8cf0-1d0af9325be6 | open | - | Cancer: EGFRvIII mutation is often observed in glioblastoma.
  categories: Cancer, EGFR, Role in human disease
8a41c8ed-5f8e-4805-beac-207075d74f90 | open | - | Cancer: EGFR overexpression/amplification is associated with lung adenocarcinoma (~40%), glioblastoma (~50%), and head/neck epithelial tumors (80-100%).
  categories: Cancer, EGFR, Role in human disease
# Create views for study slices
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag view create 'EGFR Disease Notes' --include 'Role in human disease'
created view EGFR Disease Notes
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag view create 'EGFR Drug Resistance' --include 'Drug target' --include Resistance
created view EGFR Drug Resistance
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag view create 'EGFR Antibody Strategies' --include 'Drug target' --include 'monoclonal antibodies'
created view EGFR Antibody Strategies
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag view create 'EGFR Imaging NSCLC' --include 'Target for imaging agents' --include NSCLC
created view EGFR Imaging NSCLC
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag view create 'EGFR TKI Not Resistance' --include 'Drug target' --include 'tyrosine kinase' --exclude Resistance
created view EGFR TKI Not Resistance
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag view create 'EGFR Resistance Without T790M or MET' --include Resistance --exclude T790M --exclude MET
created view EGFR Resistance Without T790M or MET
# Inspect views
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag view show 'EGFR Disease Notes'
# EGFR Disease Notes

## Unassigned
8007160e-8d8d-431e-bf53-88a2533196d9 | open | - | Drug target: EGFR inhibitors include gefitinib, erlotinib, afatinib, brigatinib, icotinib (lung cancer) and cetuximab (colon cancer). Osimertinib is a third-generation tyrosine kinase inhibitor.
  categories: Cancer, Drug target, EGFR, Medical applications, osimertinib, Role in human disease, tyrosine kinase
e8658d41-359e-4f98-9fe4-edf8e5ec8a20 | open | - | Wound healing and fibrosis: EGFR contributes to TGF-beta1-dependent fibroblast-to-myofibroblast differentiation; persistent myofibroblasts can drive fibrosis.
  categories: EGFR, Role in human disease, Wound healing and fibrosis
101b498d-01d9-4400-9b65-00dbd9df26cd | open | - | Monogenic disease: homozygous EGFR loss-of-function in a child caused multi-organ epithelial inflammation with rash, diarrhoea, hair/breathing/electrolyte issues.
  categories: EGFR, Function, Monogenic disease, Role in human disease
e5b17c39-a11c-4678-8e31-1392267c2225 | open | - | Inflammatory disease: aberrant EGFR signaling has been implicated in psoriasis, eczema, and atherosclerosis, but roles remain ill-defined.
  categories: EGFR, Inflammatory disease, Role in human disease
5786039e-01c8-4864-8cf0-1d0af9325be6 | open | - | Cancer: EGFRvIII mutation is often observed in glioblastoma.
  categories: Cancer, EGFR, Role in human disease
8a41c8ed-5f8e-4805-beac-207075d74f90 | open | - | Cancer: EGFR overexpression/amplification is associated with lung adenocarcinoma (~40%), glioblastoma (~50%), and head/neck epithelial tumors (80-100%).
  categories: Cancer, EGFR, Role in human disease
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag view show 'EGFR Drug Resistance'
# EGFR Drug Resistance

## Unassigned
49f63f23-02f5-48b6-95dc-448c2b2c81d2 | open | - | Drug target: resistance includes T790M mutation and MET oncogene; papulopustular rash is common (>90%), and EGFR-positive patients had about 60% response vs conventional chemotherapy.
  categories: Adverse effects, Drug target, EGFR, Medical applications, MET, papulopustular rash, Resistance, T790M
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag view show 'EGFR Antibody Strategies'
# EGFR Antibody Strategies

## Unassigned
a27adede-bdcd-49ce-8627-b6466bcd3f28 | open | - | Drug target: monoclonal antibodies (cetuximab/panitumumab) block the extracellular ligand-binding domain, while small molecules inhibit intracellular tyrosine kinase activity.
  categories: Drug target, EGFR, Medical applications, monoclonal antibodies, tyrosine kinase
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag view show 'EGFR Imaging NSCLC'
# EGFR Imaging NSCLC

## Unassigned
62a5c86b-906d-4830-a76d-cc6d998efe24 | open | - | Target for imaging agents: labeled EGF can identify EGFR-dependent cancers in vivo; certain CT patterns may predict EGFR mutation in NSCLC.
  categories: EGFR, Medical applications, NSCLC, Target for imaging agents
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag view show 'EGFR TKI Not Resistance'
# EGFR TKI Not Resistance

## Unassigned
a27adede-bdcd-49ce-8627-b6466bcd3f28 | open | - | Drug target: monoclonal antibodies (cetuximab/panitumumab) block the extracellular ligand-binding domain, while small molecules inhibit intracellular tyrosine kinase activity.
  categories: Drug target, EGFR, Medical applications, monoclonal antibodies, tyrosine kinase
8007160e-8d8d-431e-bf53-88a2533196d9 | open | - | Drug target: EGFR inhibitors include gefitinib, erlotinib, afatinib, brigatinib, icotinib (lung cancer) and cetuximab (colon cancer). Osimertinib is a third-generation tyrosine kinase inhibitor.
  categories: Cancer, Drug target, EGFR, Medical applications, osimertinib, Role in human disease, tyrosine kinase
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag view show 'EGFR Resistance Without T790M or MET'
# EGFR Resistance Without T790M or MET
$ cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag view list
All Items (sections=0, include=0, exclude=0)
EGFR Antibody Strategies (sections=0, include=2, exclude=0)
EGFR Disease Notes (sections=0, include=1, exclude=0)
EGFR Drug Resistance (sections=0, include=2, exclude=0)
EGFR Imaging NSCLC (sections=0, include=2, exclude=0)
EGFR Resistance Without T790M or MET (sections=0, include=1, exclude=2)
EGFR TKI Not Resistance (sections=0, include=2, exclude=1)
hint: use `agenda view show "<name>"` to see view contents
```
