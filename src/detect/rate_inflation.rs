//! Rate Inflation Detection.
//!
//! Detects when an invoice bills the customer at a higher rate than the
//! employee is actually paid (per `LaborCharge.rate`). Compares
//! `BillingRecord.billed_rate` against the employee's payroll rate from
//! labor charges on the same contract+employee.
//!
//! Useful both ways: contractors can self-audit before invoicing; customers
//! can verify invoices before paying.

use crate::data::Dataset;
use crate::types::{Alert, FraudType, MonetaryImpact, PredicateAct, RuleId};
use chrono::Utc;
use std::collections::HashMap;

/// Detector for rate inflation between billed rate and payroll/actual rate.
pub struct RateInflationDetector {
    /// Minimum variance percentage to flag (0-100).
    pub variance_threshold_pct: f64,
}

impl RateInflationDetector {
    pub fn new(variance_threshold_pct: f64) -> Self {
        Self { variance_threshold_pct }
    }

    fn calc_variance(billed: f64, actual: f64) -> f64 {
        if actual == 0.0 {
            return 0.0;
        }
        ((billed - actual) / actual) * 100.0
    }

    fn calc_confidence(variance_pct: f64) -> u8 {
        if variance_pct >= 50.0 {
            95
        } else if variance_pct >= 25.0 {
            85
        } else if variance_pct >= 15.0 {
            75
        } else {
            60
        }
    }

    fn calc_severity(variance_pct: f64) -> u8 {
        if variance_pct >= 50.0 {
            9
        } else if variance_pct >= 25.0 {
            7
        } else if variance_pct >= 15.0 {
            5
        } else {
            4
        }
    }

    #[must_use]
    pub fn run(&self, ds: &Dataset) -> Vec<Alert> {
        let mut alerts = Vec::new();

        // Average payroll rate per (contract, employee) from LaborCharges.
        let mut sums: HashMap<(String, String), (f64, f64)> = HashMap::new();
        for lc in &ds.labor_charges {
            if let Some(rate) = lc.rate {
                let key = (lc.contract_id.clone(), lc.employee_id.clone());
                let entry = sums.entry(key).or_insert((0.0, 0.0));
                entry.0 += rate * lc.hours;
                entry.1 += lc.hours;
            }
        }
        let payroll_rate: HashMap<(String, String), f64> = sums
            .into_iter()
            .filter_map(|(k, (weighted, hours))| {
                if hours > 0.0 { Some((k, weighted / hours)) } else { None }
            })
            .collect();

        for br in &ds.billing_records {
            let Some(billed_rate) = br.billed_rate else { continue };
            let key = (br.contract_id.clone(), br.employee_id.clone());
            let Some(&actual_rate) = payroll_rate.get(&key) else { continue };

            let variance_pct = Self::calc_variance(billed_rate, actual_rate);
            if variance_pct < self.variance_threshold_pct {
                continue;
            }

            let contract = ds.contract_by_id(&br.contract_id);
            let (cage_code, agency) = contract
                .map(|c| (c.cage_code.as_deref(), c.agency.as_deref()))
                .unwrap_or((None, None));

            let confidence = Self::calc_confidence(variance_pct);
            let severity = Self::calc_severity(variance_pct);
            let questioned_amount = (billed_rate - actual_rate) * br.billed_hours;

            alerts.push(Alert {
                fraud_type: FraudType::LaborCategory,
                rule_id: RuleId::RateInflation,
                severity,
                confidence,
                summary: format!(
                    "Rate variance: billed ${billed_rate:.2}/hr vs payroll ${actual_rate:.2}/hr ({variance_pct:.1}%) for employee {} on contract {} — review or correct before invoicing",
                    br.employee_id, br.contract_id
                ),
                contract_id: Some(br.contract_id.clone()),
                employee_id: Some(br.employee_id.clone()),
                cage_code: cage_code.map(String::from),
                agency: agency.map(String::from),
                predicate_acts: Some(vec![PredicateAct::FalseClaims, PredicateAct::WireFraud]),
                timestamp: Some(Utc::now().to_rfc3339()),
                monetary_impact: Some(MonetaryImpact {
                    questioned_amount,
                    currency: "USD".to_string(),
                    calculation_method: format!(
                        "(billed_rate {billed_rate} - payroll_rate {actual_rate}) * {} billed_hours",
                        br.billed_hours
                    ),
                }),
                related_alerts: None,
            });
        }

        alerts
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{BillingRecord, Contract, Employee, LaborCharge};

    fn make_dataset() -> Dataset {
        let mut ds = Dataset::default();
        ds.contracts.insert(
            "C1".into(),
            Contract {
                id: "C1".into(),
                cage_code: Some("1ABC2".into()),
                agency: Some("DoD".into()),
                labor_cats: [("Senior".to_string(), "BA".to_string())]
                    .into_iter()
                    .collect(),
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
    fn rate_inflation_empty_ds_no_alerts() {
        let ds = Dataset::default();
        let det = RateInflationDetector::new(15.0);
        assert!(det.run(&ds).is_empty());
    }

    #[test]
    fn rate_inflation_below_threshold_no_alert() {
        let mut ds = make_dataset();
        ds.labor_charges.push(LaborCharge {
            contract_id: "C1".into(),
            employee_id: "E1".into(),
            labor_cat: "Senior".into(),
            hours: 40.0,
            rate: Some(100.0),
            period: None,
        });
        ds.billing_records.push(BillingRecord {
            contract_id: "C1".into(),
            employee_id: "E1".into(),
            billed_hours: 40.0,
            billed_cat: "Senior".into(),
            period: None,
            billed_rate: Some(105.0), // 5% over — below threshold
        });

        let det = RateInflationDetector::new(15.0);
        assert!(det.run(&ds).is_empty());
    }

    #[test]
    fn rate_inflation_above_threshold_alert() {
        let mut ds = make_dataset();
        ds.labor_charges.push(LaborCharge {
            contract_id: "C1".into(),
            employee_id: "E1".into(),
            labor_cat: "Senior".into(),
            hours: 40.0,
            rate: Some(100.0), // payroll
            period: None,
        });
        ds.billing_records.push(BillingRecord {
            contract_id: "C1".into(),
            employee_id: "E1".into(),
            billed_hours: 40.0,
            billed_cat: "Senior".into(),
            period: None,
            billed_rate: Some(150.0), // 50% inflation
        });

        let det = RateInflationDetector::new(15.0);
        let alerts = det.run(&ds);
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].rule_id, RuleId::RateInflation);
        assert_eq!(alerts[0].confidence, 95);
        let mi = alerts[0].monetary_impact.as_ref().unwrap();
        assert!((mi.questioned_amount - 2000.0).abs() < 0.01);
    }

    #[test]
    fn rate_inflation_no_billed_rate_skipped() {
        let mut ds = make_dataset();
        ds.labor_charges.push(LaborCharge {
            contract_id: "C1".into(),
            employee_id: "E1".into(),
            labor_cat: "Senior".into(),
            hours: 40.0,
            rate: Some(100.0),
            period: None,
        });
        ds.billing_records.push(BillingRecord {
            contract_id: "C1".into(),
            employee_id: "E1".into(),
            billed_hours: 40.0,
            billed_cat: "Senior".into(),
            period: None,
            billed_rate: None,
        });
        let det = RateInflationDetector::new(15.0);
        assert!(det.run(&ds).is_empty());
    }

    #[test]
    fn calc_variance_positive() {
        assert!((RateInflationDetector::calc_variance(150.0, 100.0) - 50.0).abs() < 0.01);
    }

    #[test]
    fn calc_variance_zero_actual() {
        assert_eq!(RateInflationDetector::calc_variance(100.0, 0.0), 0.0);
    }

    #[test]
    fn calc_confidence_levels() {
        assert_eq!(RateInflationDetector::calc_confidence(60.0), 95);
        assert_eq!(RateInflationDetector::calc_confidence(30.0), 85);
        assert_eq!(RateInflationDetector::calc_confidence(20.0), 75);
        assert_eq!(RateInflationDetector::calc_confidence(10.0), 60);
    }

    #[test]
    fn calc_severity_levels() {
        assert_eq!(RateInflationDetector::calc_severity(60.0), 9);
        assert_eq!(RateInflationDetector::calc_severity(30.0), 7);
        assert_eq!(RateInflationDetector::calc_severity(20.0), 5);
        assert_eq!(RateInflationDetector::calc_severity(10.0), 4);
    }
}
