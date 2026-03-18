//! Overtime Padding Detection (Labor Category Fraud).
//!
//! Detects excessive overtime claims that exceed reasonable thresholds.
//! This is a form of time overcharging fraud where employees claim
//! unrealistic hours to inflate billing.

use crate::data::Dataset;
use crate::types::{Alert, FraudType, MonetaryImpact, PredicateAct, RuleId};
use chrono::Utc;
use std::collections::HashMap;

/// Detector for overtime padding based on hours thresholds.
pub struct OvertimePaddingDetector {
    /// Weekly hours threshold (default: 60 hours).
    pub weekly_threshold: f64,
    /// Monthly hours threshold (default: 240 hours).
    pub monthly_threshold: f64,
}

impl Default for OvertimePaddingDetector {
    fn default() -> Self {
        Self {
            weekly_threshold: 60.0,
            monthly_threshold: 240.0,
        }
    }
}

impl OvertimePaddingDetector {
    pub fn new(weekly_threshold: f64, monthly_threshold: f64) -> Self {
        Self {
            weekly_threshold,
            monthly_threshold,
        }
    }

    /// Calculate confidence based on hours deviation.
    fn calc_confidence(hours: f64, threshold: f64) -> u8 {
        let overage_pct = ((hours - threshold) / threshold) * 100.0;
        if overage_pct >= 50.0 {
            95
        } else if overage_pct >= 25.0 {
            85
        } else if overage_pct >= 10.0 {
            75
        } else {
            65
        }
    }

    /// Calculate severity based on hours deviation.
    fn calc_severity(hours: f64, threshold: f64) -> u8 {
        let overage_pct = ((hours - threshold) / threshold) * 100.0;
        if overage_pct >= 50.0 {
            8
        } else if overage_pct >= 25.0 {
            6
        } else {
            5
        }
    }

    /// Extract period key from period string (e.g., "2026-01-W1" -> ("2026-01", "W1")).
    fn parse_period(period: &str) -> Option<(String, String)> {
        // Expected format: "YYYY-MM-WN" or "YYYY-MM-DD"
        let parts: Vec<&str> = period.split('-').collect();
        if parts.len() >= 3 {
            let month = format!("{}-{}", parts[0], parts[1]);
            let week_or_day = parts[2].to_string();
            Some((month, week_or_day))
        } else if parts.len() == 2 {
            // Just month
            Some((period.to_string(), String::new()))
        } else {
            None
        }
    }

    #[must_use]
    pub fn run(&self, ds: &Dataset) -> Vec<Alert> {
        let mut alerts = Vec::new();

        // Aggregate hours by employee and period
        let mut weekly_hours: HashMap<(String, String, String), f64> = HashMap::new();
        let mut monthly_hours: HashMap<(String, String), f64> = HashMap::new();

        for lc in &ds.labor_charges {
            if let Some(ref period) = lc.period {
                if let Some((month, week)) = Self::parse_period(period) {
                    // Weekly aggregation
                    if !week.is_empty() {
                        let week_key = (lc.employee_id.clone(), month.clone(), week);
                        *weekly_hours.entry(week_key).or_insert(0.0) += lc.hours;
                    }
                    // Monthly aggregation
                    let month_key = (lc.employee_id.clone(), month);
                    *monthly_hours.entry(month_key).or_insert(0.0) += lc.hours;
                }
            }
        }

        // Check weekly thresholds
        for ((employee_id, month, week), hours) in &weekly_hours {
            if *hours > self.weekly_threshold {
                if let Some(emp) = ds.employee_by_id(employee_id) {
                    let contract = ds.labor_charges
                        .iter()
                        .find(|lc| lc.employee_id == *employee_id)
                        .and_then(|lc| ds.contract_by_id(&lc.contract_id));
                    
                    let (cage_code, agency) = contract
                        .map(|c| (c.cage_code.as_deref(), c.agency.as_deref()))
                        .unwrap_or((None, None));

                    let confidence = Self::calc_confidence(*hours, self.weekly_threshold);
                    let severity = Self::calc_severity(*hours, self.weekly_threshold);

                    alerts.push(Alert {
                        fraud_type: FraudType::LaborCategory,
                        rule_id: RuleId::OvertimePadding,
                        severity,
                        confidence,
                        summary: format!(
                            "Weekly overtime padding detected: {} hours in {} week {} (threshold: {}) for employee {}",
                            hours, month, week, self.weekly_threshold, employee_id
                        ),
                        contract_id: contract.map(|c| c.id.clone()),
                        employee_id: Some(employee_id.clone()),
                        cage_code: cage_code.map(String::from),
                        agency: agency.map(String::from),
                        predicate_acts: Some(vec![PredicateAct::FalseClaims]),
                        timestamp: Some(Utc::now().to_rfc3339()),
                        monetary_impact: None, // Would need rate info
                        related_alerts: None,
                    });
                }
            }
        }

        // Check monthly thresholds
        for ((employee_id, month), hours) in &monthly_hours {
            if *hours > self.monthly_threshold {
                if let Some(emp) = ds.employee_by_id(employee_id) {
                    let contract = ds.labor_charges
                        .iter()
                        .find(|lc| lc.employee_id == *employee_id)
                        .and_then(|lc| ds.contract_by_id(&lc.contract_id));
                    
                    let (cage_code, agency) = contract
                        .map(|c| (c.cage_code.as_deref(), c.agency.as_deref()))
                        .unwrap_or((None, None));

                    let confidence = Self::calc_confidence(*hours, self.monthly_threshold);
                    let severity = Self::calc_severity(*hours, self.monthly_threshold);

                    alerts.push(Alert {
                        fraud_type: FraudType::LaborCategory,
                        rule_id: RuleId::OvertimePadding,
                        severity,
                        confidence,
                        summary: format!(
                            "Monthly overtime padding detected: {} hours in {} (threshold: {}) for employee {}",
                            hours, month, self.monthly_threshold, employee_id
                        ),
                        contract_id: contract.map(|c| c.id.clone()),
                        employee_id: Some(employee_id.clone()),
                        cage_code: cage_code.map(String::from),
                        agency: agency.map(String::from),
                        predicate_acts: Some(vec![PredicateAct::FalseClaims]),
                        timestamp: Some(Utc::now().to_rfc3339()),
                        monetary_impact: None,
                        related_alerts: None,
                    });
                }
            }
        }

        alerts
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Contract, Employee, LaborCharge};
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
    fn overtime_empty_ds_no_alerts() {
        let ds = Dataset::default();
        let det = OvertimePaddingDetector::default();
        assert!(det.run(&ds).is_empty());
    }

    #[test]
    fn overtime_no_period_no_alerts() {
        let mut ds = make_dataset();
        ds.labor_charges.push(LaborCharge {
            contract_id: "C1".into(),
            employee_id: "E1".into(),
            labor_cat: "Senior".into(),
            hours: 80.0, // Excessive but no period
            rate: None,
            period: None,
        });
        
        let det = OvertimePaddingDetector::default();
        let alerts = det.run(&ds);
        assert!(alerts.is_empty());
    }

    #[test]
    fn overtime_weekly_exceeded() {
        let mut ds = make_dataset();
        ds.labor_charges.push(LaborCharge {
            contract_id: "C1".into(),
            employee_id: "E1".into(),
            labor_cat: "Senior".into(),
            hours: 70.0, // 70 hours > 60 threshold
            rate: None,
            period: Some("2026-01-W1".into()),
        });
        
        let det = OvertimePaddingDetector::new(60.0, 240.0);
        let alerts = det.run(&ds);
        assert!(!alerts.is_empty());
        assert!(alerts.iter().any(|a| a.rule_id == RuleId::OvertimePadding));
    }

    #[test]
    fn overtime_normal_hours_no_alert() {
        let mut ds = make_dataset();
        ds.labor_charges.push(LaborCharge {
            contract_id: "C1".into(),
            employee_id: "E1".into(),
            labor_cat: "Senior".into(),
            hours: 40.0, // Normal hours
            rate: None,
            period: Some("2026-01-W1".into()),
        });
        
        let det = OvertimePaddingDetector::default();
        let alerts = det.run(&ds);
        assert!(alerts.is_empty());
    }

    #[test]
    fn calc_confidence_levels() {
        let threshold = 60.0;
        assert_eq!(OvertimePaddingDetector::calc_confidence(100.0, threshold), 95); // 66% over
        assert_eq!(OvertimePaddingDetector::calc_confidence(80.0, threshold), 85);  // 33% over
        assert_eq!(OvertimePaddingDetector::calc_confidence(68.0, threshold), 75);  // 13% over
        assert_eq!(OvertimePaddingDetector::calc_confidence(62.0, threshold), 65);  // 3% over
    }

    #[test]
    fn parse_period_valid() {
        let (month, week) = OvertimePaddingDetector::parse_period("2026-01-W1").unwrap();
        assert_eq!(month, "2026-01");
        assert_eq!(week, "W1");
    }

    #[test]
    fn parse_period_month_only() {
        let (month, week) = OvertimePaddingDetector::parse_period("2026-01").unwrap();
        assert_eq!(month, "2026-01");
        assert!(week.is_empty());
    }

    #[test]
    fn parse_period_invalid() {
        assert!(OvertimePaddingDetector::parse_period("invalid").is_none());
    }
}