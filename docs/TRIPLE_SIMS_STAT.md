# TRIPLE SIMS Test Coverage Stat — whyyoulying

**Method:** Sim1→2→3→4. f49 f50 f51. Same binary.  
**Command:** `cargo run -p whyyoulying -- --test`  
**Date:** 2026-02-27

---

## Test Counts

| Phase | Count | Description |
|-------|-------|-------------|
| cargo test (unit) | 27 | config(6), types(5), data(6), detect(6), export(4) |
| cargo test (integration) | 4 | run fixtures, min-confidence, ingest, missing-data-path |
| --test f49 | 4 | Config, LaborDetector, Alert serialization |
| --test f50 | 2 | Ingest + LaborDetector + GhostDetector with TempDir |
| --test f51 | 1 | E2E binary run |
| — | **38** | |

---

## TRIPLE SIMS Mapping

### Sim 1: User Story → Tests

| User Story | Test(s) |
|------------|---------|
| D1: Proactive labor alerts | f50 LaborQualBelow |
| D3: Ghost detection | f50 GHOST_NO_EMPLOYEE; run fixtures |
| S1: Data ingestion | f50 Ingest::load_from_path |
| S3: Audit trail | ReferralPackage.audit_entries |

### Sim 2: Feature Gap → Tests

| Criterion | Test(s) |
|-----------|---------|
| Labor detector | f49, f50 |
| Ghost detector | run fixtures |
| Config thresholds | f49 Config::default |

### Sim 3: CLI/API → Tests

| Criterion | Test(s) |
|-----------|---------|
| --test flag | f49 f50 f51 |
| Exit codes | f51 e2e |

### Sim 4: Output Schema

| Criterion | Test(s) |
|-----------|---------|
| Alert serialization | f49 |
| Export format | export-referral |

---

## Run

```bash
cargo run -p whyyoulying -- --test
```

Exit 0 = all pass.
