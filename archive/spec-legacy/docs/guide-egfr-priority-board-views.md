# EGFR Priority Board Views (CLI)

Database:

```text
/tmp/aglet-egfr-wikipedia-1771272469.ag
```

## 1) Create the priority views

Run once:

```bash
cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag view create "EGFR Follow-up High" --include Follow-up --include High
cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag view create "EGFR Follow-up Medium" --include Follow-up --include Medium
cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag view create "EGFR Follow-up Low" --include Follow-up --include Low
```

## 2) Show the priority views

```bash
cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag view show "EGFR Follow-up High"
cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag view show "EGFR Follow-up Medium"
cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag view show "EGFR Follow-up Low"
```

## 3) Current expected snapshot

### High

- `91eef5ab-91c7-4ad8-83dd-07234a4f1267` Disease follow-up: TCGA overexpression prevalence (`2026-02-17 11:00`)
- `a922b67d-c92e-435f-baf8-c4c715fdefd0` Drug resistance follow-up: T790M trajectories (`2026-02-17 16:00`)
- `da97dbf6-3480-429c-a8a6-7fa4b0586bc6` Drug resistance follow-up: MET co-resistance pathways (`2026-02-23 14:00`)

### Medium

- `840d39ed-0259-425f-8830-edcf06659089` Imaging follow-up: CT features for EGFR mutation (`2026-02-18 13:00`)
- `5388a3b5-0724-4d79-bef9-ec7b1cfa8b97` Disease follow-up: psoriasis/eczema mechanism (`2026-02-19 15:00`)
- `8616bb1b-fcca-47db-8e24-80d570e3e46f` Translational follow-up: mAb vs TKI predictors (`2026-02-20 10:00`)

### Low

- No items currently.

## 4) Optional quick list check

```bash
cargo run -q -p agenda-cli -- --db /tmp/aglet-egfr-wikipedia-1771272469.ag view list | rg "EGFR Follow-up (High|Medium|Low)"
```
