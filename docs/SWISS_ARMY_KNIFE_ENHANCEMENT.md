# whyyoulying: Swiss Army Knife Enhancement Plan
## FBI White-Collar Crime Detection Platform

**Author:** AI Analysis following KOVA's recent code changes  
**Date:** 2026-03-16  
**Version:** Based on commit 983f226

---

## Executive Summary

The whyyoulying project has established a solid foundation for Labor Category Fraud and Ghost Billing detection with proper DoD IG and FBI compliance frameworks. Following KOVA's recent enhancements (DoD/FBI features, industry tests, anti-pattern fixes), this document proposes transforming the tool into a comprehensive "Swiss Army Knife" for white-collar crime investigation.

---

## Current Architecture Analysis

### Strengths (KOVA's Recent Improvements)

1. **Solid Domain Model** - Well-structured types for Contract, Employee, LaborCharge, BillingRecord
2. **Compliance-First Design** - GAGAS chain of custody, FBI predicate acts, DoD nexus filtering
3. **Flexible CLI** - Subcommands (run, ingest, export-referral), multiple output formats
4. **Confidence Scoring** - S4 false-positive control with min_confidence filtering
5. **Agency-Specific Routing** - --agency and --cage-code filtering for DoD nexus
6. **FBI Integration** - predicate_acts field, FBI case-opening export format

### Current Detection Rules

| Rule ID | Type | Description | Confidence |
|---------|------|-------------|------------|
| LABOR_VARIANCE | Labor | Labor category not in contract | 85 |
| LABOR_QUAL_BELOW | Labor | Employee charged above qualification | 90 |
| GHOST_NO_EMPLOYEE | Ghost | Billed employee not in roster | 95 |
| GHOST_NOT_VERIFIED | Ghost | No floorcheck verification | 70 |
| GHOST_BILLED_NOT_PERFORMED | Ghost | Billed hours exceed performed | 80-90 |

---

## Swiss Army Knife Enhancement Proposals

### TIER 1: Critical White-Collar Crime Patterns (High Value, High Priority)

#### 1.1 Rate Inflation Detection (RATE_INFLATION)

**Fraud Type:** Labor Category  
**Scenario:** Contractor bills at premium rates while paying employees lower rates

```rust
// New Rule: RATE_INFLATION
// Detection: Compare billed rate vs. actual employee rate
// Confidence: Based on variance magnitude (10% = 60, 25% = 80, 50%+ = 95)
// Predicate Acts: False Claims, Wire Fraud

pub struct RateInflationDetector {
    pub variance_threshold_pct: f64,
}

impl RateInflationDetector {
    pub fn run(&self, ds: &Dataset) -> Vec<Alert> {
        // Cross-reference labor_charges.rate with billing_records
        // Flag when billed rate significantly exceeds employee's actual rate
        // Calculate inflation percentage for severity
    }
}
```

**CLI Addition:** `--rate-inflation-threshold PCT`

---

#### 1.2 Overtime Padding Detection (OVERTIME_PADDING)

**Fraud Type:** Labor Category  
**Scenario:** Excessive overtime claims beyond reasonable thresholds

```rust
// New Rule: OVERTIME_PADDING
// Detection: Hours per period exceeding thresholds (e.g., >60 hrs/week)
// Confidence: Based on hours deviation (50+ hrs = 70, 60+ hrs = 85, 80+ hrs = 95)
// Predicate Acts: False Claims

pub struct OvertimePaddingDetector {
    pub weekly_threshold: f64,      // Default: 60 hours
    pub monthly_threshold: f64,     // Default: 240 hours
}
```

**New Data Required:** Time period in labor_charges or billing_records

---

#### 1.3 Duplicate Billing Detection (DUPLICATE_BILLING)

**Fraud Type:** Ghost Billing  
**Scenario:** Same hours billed to multiple contracts

```rust
// New Rule: DUPLICATE_BILLING
// Detection: Same employee, same hours, different contracts in same period
// Confidence: 85-95 based on match precision
// Predicate Acts: False Claims, Wire Fraud

pub struct DuplicateBillingDetector;

impl DuplicateBillingDetector {
    pub fn run(&self, ds: &Dataset) -> Vec<Alert> {
        // Group billing_records by employee_id + period
        // Flag overlapping hours across contracts
    }
}
```

---

#### 1.4 Shell Company Indicators (SHELL_COMPANY)

**Fraud Type:** Ghost Billing  
**Scenario:** Small business "face" billing for prime work

```rust
// New Rule: SHELL_COMPANY
// Detection: CAGE code anomalies, address mismatches, unusual billing patterns
// Confidence: 70-85 based on indicator count
// Predicate Acts: False Claims, Wire Fraud, Conspiracy

pub struct ShellCompanyDetector {
    // Requires external CAGE code validation data
}
```

**New Data Required:** Vendor master data, CAGE code registry access

---

### TIER 2: Enhanced Investigation Capabilities

#### 2.1 Temporal Pattern Analysis

**New Command:** `analyze-patterns`

```rust
pub enum PatternType {
    WeeklyVariance,      // Unusual weekly fluctuations
    EndOfPeriodSpike,    // End-of-quarter billing spikes
    WeekendBilling,      // Non-business day charges
    HolidayBilling,      // Federal holiday charges
}

// New subcommand
whyyoulying analyze-patterns --pattern weekly-variance --data-path ./data
```

---

#### 2.2 Employee Risk Scoring

**New Feature:** Aggregate risk score per employee

```rust
pub struct EmployeeRiskScore {
    pub employee_id: String,
    pub total_alerts: usize,
    pub risk_score: f64,          // 0-100 composite
    pub alert_breakdown: HashMap<RuleId, usize>,
    pub total_questionable_hours: f64,
    pub total_questionable_amount: Option<f64>,
}

// New command
whyyoulying risk-score --employee E1 --data-path ./data
```

---

#### 2.3 Contract Risk Heat Map

**New Export:** Contract-level aggregation

```rust
pub struct ContractRiskHeatMap {
    pub contract_id: String,
    pub cage_code: Option<String>,
    pub agency: Option<String>,
    pub total_alerts: usize,
    pub employees_flagged: usize,
    pub total_questionable_hours: f64,
    pub risk_tier: RiskTier,  // Low, Medium, High, Critical
    pub top_rules: Vec<(RuleId, usize)>,
}

// New command
whyyoulying heat-map --output html --data-path ./data
```

---

### TIER 3: Data Integration & Enrichment

#### 3.1 Multi-Source Data Ingestion

**Current:** JSON files only  
**Enhanced:**

```rust
pub enum DataSource {
    JsonDirectory(PathBuf),
    SingleJsonFile(PathBuf),
    CsvDirectory(PathBuf),
    Database(PostgresConfig),
    ApiEndpoint(ApiConfig),
    S3Bucket(S3Config),
}

// Support for:
// - SAM.gov API (System for Award Management)
// - FPDS data feeds
// - USAspending.gov API
// - DCAA system exports
```

---

#### 3.2 CAGE Code Validation

**New Module:** `src/validate/cage.rs`

```rust
pub struct CageValidator {
    pub cache: HashMap<String, CageInfo>,
}

pub struct CageInfo {
    pub cage_code: String,
    pub company_name: String,
    pub address: Address,
    pub status: CageStatus,  // Active, Inactive, Suspended
    pub sam_exclusion: bool, // Debarred/Suspended
}

// Integration with SAM.gov API
whyyoulying validate --cage-code 1ABC2 --check-sam-exclusion
```

---

#### 3.3 Employee Verification Enrichment

**New Data Sources:**

```rust
pub struct EmployeeVerification {
    pub employee_id: String,
    pub background_check: VerificationStatus,
    pub security_clearance: Option<ClearanceLevel>,
    pub education_verified: bool,
    pub certifications: Vec<Certification>,
}
```

---

### TIER 4: Advanced Analytics

#### 4.1 Statistical Anomaly Detection

**New Detector:** Statistical outlier analysis

```rust
pub struct StatisticalAnomalyDetector {
    pub z_score_threshold: f64,  // Default: 2.5
    pub methods: Vec<StatisticalMethod>,
}

pub enum StatisticalMethod {
    ZScore,           // Standard deviations from mean
    IQR,              // Inter-quartile range outliers
    MovingAverage,    // Time series deviation
    BenfordLaw,       // First-digit distribution analysis
}

// Benford's Law for fabricated numbers
// Detects made-up billing amounts/hours
```

---

#### 4.2 Network Analysis

**New Module:** Relationship mapping

```rust
pub struct NetworkAnalyzer;

impl NetworkAnalyzer {
    // Detect employee overlap across contractors
    pub fn detect_employee_overlap(&self, ds: &Dataset) -> Vec<OverlapAlert>;
    
    // Detect common addresses/phones across entities
    pub fn detect_entity_connections(&self, ds: &Dataset) -> Vec<ConnectionAlert>;
    
    // Identify potential bid-rigging patterns
    pub fn detect_bid_patterns(&self, ds: &Dataset) -> Vec<BidRiggingAlert>;
}
```

---

#### 4.3 Predictive Risk Modeling

**New Feature:** ML-based risk prediction

```rust
pub struct RiskPredictor {
    pub model: Box<dyn RiskModel>,
}

pub trait RiskModel {
    fn predict_contract_risk(&self, contract: &Contract) -> f64;
    fn predict_employee_risk(&self, employee: &Employee) -> f64;
}

// Features:
// - Historical alert patterns
// - Contract characteristics
// - Employee qualification gaps
// - Vendor past performance
```

---

## Implementation Roadmap

### Phase 1: Core Enhancements (Weeks 1-4)

| Priority | Item | Effort | Impact |
|----------|------|--------|--------|
| 1 | Rate Inflation Detection | Medium | High |
| 2 | Overtime Padding Detection | Low | High |
| 3 | Duplicate Billing Detection | Medium | High |
| 4 | Enhanced CSV output with risk scores | Low | Medium |

### Phase 2: Investigation Tools (Weeks 5-8)

| Priority | Item | Effort | Impact |
|----------|------|--------|--------|
| 5 | Employee Risk Scoring | Medium | High |
| 6 | Contract Risk Heat Map | Medium | High |
| 7 | Temporal Pattern Analysis | Medium | Medium |
| 8 | Enhanced filtering (--date-range, --employee) | Low | Medium |

### Phase 3: Data Integration (Weeks 9-12)

| Priority | Item | Effort | Impact |
|----------|------|--------|--------|
| 9 | CSV data source support | Low | Medium |
| 10 | CAGE code validation API | High | High |
| 11 | SAM.gov exclusion check | High | High |
| 12 | Database connectivity | High | Medium |

### Phase 4: Advanced Analytics (Weeks 13-16)

| Priority | Item | Effort | Impact |
|----------|------|--------|--------|
| 13 | Statistical anomaly detection | Medium | High |
| 14 | Benford's Law analysis | Low | Medium |
| 15 | Network analysis | High | High |
| 16 | Predictive modeling | Very High | Medium |

---

## New Types to Add

```rust
// src/types.rs additions

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RuleId {
    // Existing
    LaborVariance,
    LaborQualBelow,
    GhostNoEmployee,
    GhostNotVerified,
    GhostBilledNotPerformed,
    
    // New Tier 1
    RateInflation,
    OvertimePadding,
    DuplicateBilling,
    ShellCompanyIndicators,
    
    // New Tier 4
    StatisticalAnomaly,
    BenfordViolation,
    NetworkAnomaly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PredicateAct {
    FalseClaims,
    WireFraud,
    IdentityFraud,
    // New
    Conspiracy,
    MailFraud,
    ProgramFraud,
    ProcurementFraud,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonetaryImpact {
    pub questioned_amount: f64,
    pub currency: String,
    pub calculation_method: String,
}

// Extend Alert to include monetary impact
pub struct Alert {
    // ... existing fields ...
    pub monetary_impact: Option<MonetaryImpact>,
    pub related_alerts: Option<Vec<String>>,  // Alert IDs
}
```

---

## CLI Enhancements

### New Subcommands

```bash
# Risk analysis
whyyoulying risk-score --employee E1 --data-path ./data
whyyoulying heat-map --output html --agency DoD

# Pattern analysis
whyyoulying analyze-patterns --pattern end-of-period-spike
whyyoulying analyze-patterns --pattern weekend-billing

# Validation
whyyoulying validate --cage-code 1ABC2
whyyoulying validate --employee E1 --check-clearance

# Statistical analysis
whyyoulying statistics --method benford --field billed_hours
whyyoulying statistics --method z-score --threshold 2.5

# Network analysis
whyyoulying network --detect-overlap
whyyoulying network --detect-connections
```

### New Global Options

```bash
--date-range START:END     # Filter by date range
--employee ID              # Filter by specific employee
--contract ID              # Filter by specific contract
--min-amount AMOUNT        # Filter by minimum questioned amount
--include-monetary         # Calculate and include monetary impact
--format json|csv|html     # Output format (add HTML for heat maps)
```

---

## Testing Strategy

### Unit Tests to Add

```rust
#[cfg(test)]
mod tests {
    // Rate inflation tests
    #[test]
    fn rate_inflation_detector_above_threshold() { ... }
    #[test]
    fn rate_inflation_detector_below_threshold_no_alert() { ... }
    
    // Overtime padding tests
    #[test]
    fn overtime_padding_weekly_exceeded() { ... }
    #[test]
    fn overtime_padding_normal_hours_no_alert() { ... }
    
    // Duplicate billing tests
    #[test]
    fn duplicate_billing_same_employee_diff_contracts() { ... }
    #[test]
    fn duplicate_billing_no_overlap_no_alert() { ... }
    
    // Statistical tests
    #[test]
    fn benford_analysis_detects_fabricated_numbers() { ... }
    #[test]
    fn z_score_detects_outliers() { ... }
}
```

### Integration Tests to Add

```rust
// tests/integration.rs additions

#[test]
fn f60_rate_inflation_end_to_end() {
    let ds = load_fixture("rate_inflation_scenario");
    let alerts = RateInflationDetector::new(15.0).run(&ds);
    assert!(!alerts.is_empty());
    assert!(alerts.iter().any(|a| a.rule_id == RuleId::RateInflation));
}

#[test]
fn f61_overtime_padding_detection() { ... }

#[test]
fn f62_duplicate_billing_detection() { ... }

#[test]
fn f63_employee_risk_scoring() { ... }

#[test]
fn f64_contract_heat_map_generation() { ... }
```

---

## Compliance Considerations

### DoD IG Alignment

- Maintain DoDI 5505.02/03 compliance for all new detectors
- Ensure DoD nexus filtering applies to all new alerts
- Add audit trail entries for all new rule types

### FBI Alignment

- Map new rules to appropriate predicate acts
- Update FBI case-opening export for new fraud types
- Maintain AG Guidelines compliance

### GAGAS Compliance

- Extend chain_of_custody to new detectors
- Document calculation methodologies
- Maintain audit trail integrity

---

## Conclusion

This Swiss Army Knife enhancement plan transforms whyyoulying from a focused labor/ghost billing detector into a comprehensive white-collar crime investigation platform while maintaining its strong compliance foundation. The tiered approach allows for incremental value delivery while building toward advanced analytics capabilities.

**Key Success Metrics:**
- Detection coverage: 5 rules → 12+ rules
- Investigation efficiency: Manual analysis → Automated risk scoring
- Case building: Alerts → Full monetary impact calculation
- Integration: Static JSON → Multi-source data ingestion