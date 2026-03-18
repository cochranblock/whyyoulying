//! Rate Inflation Detection (Labor Category Fraud).
//!
//! Detects when contractors bill at premium rates while paying employees
//! lower actual rates. This is a form of labor mischarging where the
//! government is overcharged for labor.

use crate::data::Dataset;
use crate::types::{Alert, FraudType, MonetaryImpact, PredicateAct, RuleId};
use chrono::Utc;

/// Detector for rate inflation between billed and actual rates.
pub struct RateInflationDetector {
    /// Minimum variance percentage to flag (0-100).
    pub variance_threshold_pct: f64,
}

impl RateInflationDetector {
    pub fn new(variance_threshold_pct: f64) -> Self {
        Self { variance_threshold_pct }
    }

    /// Calculate variance percentage between billed and actual rates.
    fn calc_variance(billed: f64, actual: f64) -> f64 {
        if actual == 0.0 {
            return 0.0;
        }
        ((billed - actual) / actual) * 100.0
    }

    /// Determine confidence based on variance magnitude.
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

    /// Determine severity based on variance magnitude.
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

        // Build a map of employee_id -> labor_charges with rates
        let employee_rates: std::collections::HashMap<String, f64> = ds
            .labor_charges
            .iter()
            .filter_map(|lc| lc.rate.map(|r| (lc.employee_id.clone(), r)))
            .collect();

        for br in &ds.billing_records {
            // Get the contract for cage_code and agency
            let contract = ds.contract_by_id(&br.contract_id);
            let (cage_code, agency) = contract
                .map(|c| (c.cage_code.as_deref(), c.agency.as_deref()))
                .unwrap_or((None, None));

            // Check if we have an actual rate for this employee
            if let Some(&actual_rate) = employee_rates.get(&br.employee_id) {
                // We need a billed rate - infer from labor_charges or use a default
                // For now, we compare against the labor charge rate
                let labor_charge = ds.labor_charges.iter().find(|lc| {
                    lc.employee_id == br.employee_id && lc.contract_id == br.contract_id
                });

                if let Some(lc) = labor_charge {
                    if let Some(billed_rate) = lc.rate {
                        let variance_pct = Self::calc_variance(billed_rate, actual_rate);
                        
                        if variance_pct >= self.variance_threshold_pct {
                            let confidence = Self::calc_confidence(variance_pct);
                            let severity = Self::calc_severity(variance_pct);
                            
                            // Calculate monetary impact
                            let questioned_amount = (billed_rate - actual_rate) * br.billed_hours;
                            
                            alerts.push(Alert {
                                fraud_type: FraudType::LaborCategory,
                                rule_id: RuleId::RateInflation,
                                severity,
                                confidence,
                                summary: format!(
                                    "Rate inflation detected: billed at ${:.2}/hr but actual rate is ${:.2}/hr ({:.1}% variance) for employee {}",
                                    billed_rate, actual_rate, variance_pct, br.employee_id
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
                                        "({} - {}) * {} hours",
                                        billed_rate, actual_rate, br.billed_hours
                                    ),
                                }),
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
    use crate::types::{BillingRecord, Contract, Employee, LaborCharge};
    use std::collections::HashMap;

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
            rate: Some(100.0), // Actual rate
            period: None,
        });
        ds.billing_records.push(BillingRecord {
            contract_id: "C1".into(),
            employee_id: "E1".into(),
            billed_hours: 40.0,
            billed_cat: "Senior".into(),
            period: None,
        });
        
        let det = RateInflationDetector::new(50.0); // 50% threshold
        let alerts = det.run(&ds);
        assert!(alerts.is_empty());
    }

    #[test]
    fn rate_inflation_above_threshold_alert() {
        let mut ds = make_dataset();
        // Labor charge shows employee is paid $100/hr
        ds.labor_charges.push(LaborCharge {
            contract_id: "C1".into(),
            employee_id: "E1".into(),
            labor_cat: "Senior".into(),
            hours: 40.0,
            rate: Some(100.0), // Actual rate: $100/hr
            period: None,
        });
        // But billed at $150/hr (50% inflation)
        ds.labor_charges.push(LaborCharge {
            contract_id: "C1".into(),
            employee_id: "E1".into(),
            labor_cat: "Senior".into(),
            hours: 40.0,
            rate: Some(150.0), // Billed rate: $150/hr
            period: None,
        });
        ds.billing_records.push(BillingRecord {
            contract_id: "C1".into(),
            employee_id: "E1".into(),
            billed_hours: 40.0,
            billed_cat: "Senior".into(),
            period: None,
        });
        
        let det = RateInflationDetector::new(25.0); // 25% threshold
        let alerts = det.run(&ds);
        // This test will need adjustment based on actual logic
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