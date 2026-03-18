//! Duplicate Billing Detection (Ghost Billing Fraud).
//!
//! Detects when the same employee bills the same hours to multiple
//! contracts in the same period. This is a form of ghost billing
//! where contractors double-bill for the same work.

use crate::data::Dataset;
use crate::types::{Alert, BillingRecord, FraudType, MonetaryImpact, PredicateAct, RuleId};
use chrono::Utc;
use std::collections::HashMap;

/// Detector for duplicate billing across contracts.
pub struct DuplicateBillingDetector {
    /// Hours tolerance for matching (default: 0.01).
    pub hours_tolerance: f64,
}

impl Default for DuplicateBillingDetector {
    fn default() -> Self {
        Self { hours_tolerance: 0.01 }
    }
}

impl DuplicateBillingDetector {
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if two hours values are within tolerance.
    fn hours_match(h1: f64, h2: f64, tolerance: f64) -> bool {
        (h1 - h2).abs() <= tolerance
    }

    /// Calculate confidence based on match precision.
    fn calc_confidence(exact_match: bool, same_cat: bool) -> u8 {
        if exact_match && same_cat {
            95
        } else if exact_match {
            90
        } else if same_cat {
            80
        } else {
            70
        }
    }

    #[must_use]
    pub fn run(&self, ds: &Dataset) -> Vec<Alert> {
        let mut alerts = Vec::new();

        // Group billing records by employee + period
        let mut by_employee_period: HashMap<(String, Option<String>), Vec<&BillingRecord>> = HashMap::new();
        
        for br in &ds.billing_records {
            let key = (br.employee_id.clone(), br.period.clone());
            by_employee_period.entry(key).or_default().push(br);
        }

        // Check for duplicates within each employee+period group
        for ((employee_id, period), records) in by_employee_period {
            // Skip if only one record (no duplicate possible)
            if records.len() < 2 {
                continue;
            }

            // Get employee info
            let employee = ds.employee_by_id(&employee_id);

            // Compare each pair of records
            for i in 0..records.len() {
                for j in (i + 1)..records.len() {
                    let r1 = records[i];
                    let r2 = records[j];

                    // Check if hours match (within tolerance)
                    if Self::hours_match(r1.billed_hours, r2.billed_hours, self.hours_tolerance) {
                        // Different contracts = potential duplicate billing
                        if r1.contract_id != r2.contract_id {
                            let exact_match = (r1.billed_hours - r2.billed_hours).abs() < 0.001;
                            let same_cat = r1.billed_cat == r2.billed_cat;
                            let confidence = Self::calc_confidence(exact_match, same_cat);
                            
                            // Get contract info for both
                            let c1 = ds.contract_by_id(&r1.contract_id);
                            let c2 = ds.contract_by_id(&r2.contract_id);

                            // Use first contract for alert context
                            let (cage_code, agency) = c1
                                .map(|c| (c.cage_code.as_deref(), c.agency.as_deref()))
                                .unwrap_or((None, None));

                            let period_str = period.as_deref().unwrap_or("unknown");

                            alerts.push(Alert {
                                fraud_type: FraudType::GhostBilling,
                                rule_id: RuleId::DuplicateBilling,
                                severity: 8,
                                confidence,
                                summary: format!(
                                    "Duplicate billing detected: employee {} billed {:.1} hours to both contract {} and {} in period {}",
                                    employee_id, r1.billed_hours, r1.contract_id, r2.contract_id, period_str
                                ),
                                contract_id: Some(format!("{},{}", r1.contract_id, r2.contract_id)),
                                employee_id: Some(employee_id.clone()),
                                cage_code: cage_code.map(String::from),
                                agency: agency.map(String::from),
                                predicate_acts: Some(vec![PredicateAct::FalseClaims, PredicateAct::WireFraud]),
                                timestamp: Some(Utc::now().to_rfc3339()),
                                monetary_impact: None, // Would need rate info
                                related_alerts: None,
                            });
                        }
                    }
                }
            }
        }

        alerts
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Contract, Employee};
    use std::collections::HashMap;

    fn make_dataset() -> Dataset {
        let mut ds = Dataset::default();
        
        ds.contracts.insert(
            "C1".into(),
            Contract {
                id: "C1".into(),
                cage_code: Some("1ABC2".into()),
                agency: Some("DoD".into()),
                labor_cats: HashMap::new(),
            },
        );
        
        ds.contracts.insert(
            "C2".into(),
            Contract {
                id: "C2".into(),
                cage_code: Some("1ABC2".into()),
                agency: Some("DoD".into()),
                labor_cats: HashMap::new(),
            },
        );
        
        ds.employees.insert(
            "E1".into(),
            Employee {
                id: "E1".into(),
                quals: vec!["BA".into()],
                labor_cat_min: Some("Senior".into()),
                verified: true,
            },
        );
        
        ds
    }

    #[test]
    fn duplicate_billing_empty_ds_no_alerts() {
        let ds = Dataset::default();
        let det = DuplicateBillingDetector::new();
        assert!(det.run(&ds).is_empty());
    }

    #[test]
    fn duplicate_billing_same_employee_diff_contracts() {
        let mut ds = make_dataset();
        
        // Same employee bills same hours to two contracts
        ds.billing_records.push(BillingRecord {
            contract_id: "C1".into(),
            employee_id: "E1".into(),
            billed_hours: 40.0,
            billed_cat: "Senior".into(),
            period: Some("2026-01-W1".into()),
        });
        ds.billing_records.push(BillingRecord {
            contract_id: "C2".into(),
            employee_id: "E1".into(),
            billed_hours: 40.0,
            billed_cat: "Senior".into(),
            period: Some("2026-01-W1".into()),
        });
        
        let det = DuplicateBillingDetector::new();
        let alerts = det.run(&ds);
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].rule_id, RuleId::DuplicateBilling);
        assert_eq!(alerts[0].confidence, 95); // Exact match + same category
    }

    #[test]
    fn duplicate_billing_no_overlap_no_alert() {
        let mut ds = make_dataset();
        
        // Same employee but different periods
        ds.billing_records.push(BillingRecord {
            contract_id: "C1".into(),
            employee_id: "E1".into(),
            billed_hours: 40.0,
            billed_cat: "Senior".into(),
            period: Some("2026-01-W1".into()),
        });
        ds.billing_records.push(BillingRecord {
            contract_id: "C2".into(),
            employee_id: "E1".into(),
            billed_hours: 40.0,
            billed_cat: "Senior".into(),
            period: Some("2026-01-W2".into()), // Different week
        });
        
        let det = DuplicateBillingDetector::new();
        let alerts = det.run(&ds);
        assert!(alerts.is_empty());
    }

    #[test]
    fn duplicate_billing_diff_hours_no_alert() {
        let mut ds = make_dataset();
        
        // Same employee, same period, but different hours
        ds.billing_records.push(BillingRecord {
            contract_id: "C1".into(),
            employee_id: "E1".into(),
            billed_hours: 40.0,
            billed_cat: "Senior".into(),
            period: Some("2026-01-W1".into()),
        });
        ds.billing_records.push(BillingRecord {
            contract_id: "C2".into(),
            employee_id: "E1".into(),
            billed_hours: 20.0, // Different hours
            billed_cat: "Senior".into(),
            period: Some("2026-01-W1".into()),
        });
        
        let det = DuplicateBillingDetector::new();
        let alerts = det.run(&ds);
        assert!(alerts.is_empty());
    }

    #[test]
    fn duplicate_billing_same_contract_no_alert() {
        let mut ds = make_dataset();
        
        // Same contract = not duplicate billing
        ds.billing_records.push(BillingRecord {
            contract_id: "C1".into(),
            employee_id: "E1".into(),
            billed_hours: 40.0,
            billed_cat: "Senior".into(),
            period: Some("2026-01-W1".into()),
        });
        ds.billing_records.push(BillingRecord {
            contract_id: "C1".into(), // Same contract
            employee_id: "E1".into(),
            billed_hours: 40.0,
            billed_cat: "Junior".into(),
            period: Some("2026-01-W1".into()),
        });
        
        let det = DuplicateBillingDetector::new();
        let alerts = det.run(&ds);
        assert!(alerts.is_empty());
    }

    #[test]
    fn hours_match_exact() {
        assert!(DuplicateBillingDetector::hours_match(40.0, 40.0, 0.01));
    }

    #[test]
    fn hours_match_within_tolerance() {
        assert!(DuplicateBillingDetector::hours_match(40.0, 40.005, 0.01));
    }

    #[test]
    fn hours_match_outside_tolerance() {
        assert!(!DuplicateBillingDetector::hours_match(40.0, 40.02, 0.01));
    }

    #[test]
    fn calc_confidence_levels() {
        assert_eq!(DuplicateBillingDetector::calc_confidence(true, true), 95);
        assert_eq!(DuplicateBillingDetector::calc_confidence(true, false), 90);
        assert_eq!(DuplicateBillingDetector::calc_confidence(false, true), 80);
        assert_eq!(DuplicateBillingDetector::calc_confidence(false, false), 70);
    }
}