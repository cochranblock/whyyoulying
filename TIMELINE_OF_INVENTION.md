<!-- Unlicense — cochranblock.org -->

# Timeline of Invention

*Dated, commit-level record of what was built, when, and why. Proves human-piloted AI development — not generated spaghetti.*

> Every entry below maps to real commits. Run `git log --oneline` to verify.

---

## Entries

### 2026-03 — Swiss Army Knife Phase 1: New Fraud Detectors

**What:** Added new fraud detection patterns beyond labor cat and ghost billing. Enhanced detection engine with additional white-collar crime patterns.
**Why:** Expanding coverage beyond the initial DoD IG use cases to broader federal fraud scenarios.
**Commit:** `d19556b`, `4cec672`
**AI Role:** AI implemented detection logic. Human directed which fraud patterns to target based on DoDI references.

### 2026-03 — Refactor + Test Hardening

**What:** Restructured into config/data/detect modules. Embedded test binary (P14 same-binary model). Added 50 unit + 10 integration tests. README rewrite with subcommand help.
**Why:** Code maturity — moving from prototype to auditable tooling suitable for DoD IG review.
**Commit:** `983f226`, `651cc2e`, `8d0c4eb`, `9c16bf1`
**AI Role:** AI executed refactoring and test generation. Human designed module boundaries and test scenarios based on real fraud patterns.

### 2026-03 — Initial Build: Sim 1-4

**What:** Labor category fraud detector, ghost billing detector, CLI with ingest/run/export-referral commands, GAGAS referral package export, FBI case-opening doc generation. Test functions f49–f51.
**Why:** Built for DoD IG and FBI fraud investigators per DoDI 5505.02/03 and AG Guidelines. Proactive detection replaces manual audit.
**Commit:** `41c5506`, `134c14d`
**AI Role:** AI built the detection pipeline and CLI. Human directed all legal basis references, detection thresholds, and output formats based on actual DoD fraud investigation procedures.
**Proof:** `cargo run --release -- --test`

---

*Part of the [CochranBlock](https://cochranblock.org) zero-cloud architecture. All source under the Unlicense.*
