//! Data ingestion and normalization.

use crate::config::Config;
use crate::types::{BillingRecord, Contract, Employee, LaborCharge};
use anyhow::{Context, Result};
use std::collections::HashSet;
use std::path::Path;

/// Normalized dataset for detection pipeline.
#[derive(Debug, Clone, Default)]
pub struct Dataset {
    pub contracts: Vec<Contract>,
    pub employees: Vec<Employee>,
    pub labor_charges: Vec<LaborCharge>,
    pub billing_records: Vec<BillingRecord>,
}

impl Dataset {
    pub fn contract_by_id(&self, id: &str) -> Option<&Contract> {
        self.contracts.iter().find(|c| c.id == id)
    }

    pub fn employee_by_id(&self, id: &str) -> Option<&Employee> {
        self.employees.iter().find(|e| e.id == id)
    }

    pub fn employee_ids(&self) -> HashSet<&str> {
        self.employees.iter().map(|e| e.id.as_str()).collect()
    }

    /// DoD nexus filter (D5): contract IDs matching agency and/or CAGE.
    pub fn nexus_contract_ids(
        &self,
        filter_agency: Option<&str>,
        filter_cage_code: Option<&str>,
    ) -> std::collections::HashSet<&str> {
        if filter_agency.is_none() && filter_cage_code.is_none() {
            return self.contracts.iter().map(|c| c.id.as_str()).collect();
        }
        self.contracts
            .iter()
            .filter(|c| {
                let agency_ok = filter_agency
                    .map(|a| c.agency.as_deref().map_or(false, |x| x.eq_ignore_ascii_case(a)))
                    .unwrap_or(true);
                let cage_ok = filter_cage_code
                    .map(|g| c.cage_code.as_deref().map_or(false, |x| x.eq_ignore_ascii_case(g)))
                    .unwrap_or(true);
                agency_ok && cage_ok
            })
            .map(|c| c.id.as_str())
            .collect()
    }
}

pub struct Ingest;

impl Ingest {
    /// Load and normalize data from config.data_path.
    pub fn load(config: &Config) -> Result<Dataset> {
        let path = config
            .data_path
            .as_deref()
            .context("data_path required for ingest")?;
        Self::load_from_path(Path::new(path))
    }

    pub fn load_from_path(path: &Path) -> Result<Dataset> {
        let mut ds = Dataset::default();

        let contracts_path = path.join("contracts.json");
        if contracts_path.exists() {
            let s = std::fs::read_to_string(&contracts_path)
                .with_context(|| format!("read {}", contracts_path.display()))?;
            let raw: Vec<Contract> = serde_json::from_str(&s)
                .with_context(|| format!("parse {}", contracts_path.display()))?;
            ds.contracts = raw;
        }

        let employees_path = path.join("employees.json");
        if employees_path.exists() {
            let s = std::fs::read_to_string(&employees_path)
                .with_context(|| format!("read {}", employees_path.display()))?;
            let raw: Vec<Employee> = serde_json::from_str(&s)
                .with_context(|| format!("parse {}", employees_path.display()))?;
            ds.employees = raw;
        }

        let labor_path = path.join("labor_charges.json");
        if labor_path.exists() {
            let s = std::fs::read_to_string(&labor_path)
                .with_context(|| format!("read {}", labor_path.display()))?;
            let raw: Vec<LaborCharge> = serde_json::from_str(&s)
                .with_context(|| format!("parse {}", labor_path.display()))?;
            ds.labor_charges = raw;
        }

        let billing_path = path.join("billing_records.json");
        if billing_path.exists() {
            let s = std::fs::read_to_string(&billing_path)
                .with_context(|| format!("read {}", billing_path.display()))?;
            let raw: Vec<BillingRecord> = serde_json::from_str(&s)
                .with_context(|| format!("parse {}", billing_path.display()))?;
            ds.billing_records = raw;
        }

        Ok(ds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Contract;
    use std::collections::HashMap;

    #[test]
    fn load_from_path_empty_dir() {
        let tmp = tempfile::TempDir::new().unwrap();
        let ds = Ingest::load_from_path(tmp.path()).unwrap();
        assert!(ds.contracts.is_empty());
        assert!(ds.employees.is_empty());
        assert!(ds.labor_charges.is_empty());
        assert!(ds.billing_records.is_empty());
    }

    #[test]
    fn load_from_path_partial() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(
            tmp.path().join("contracts.json"),
            r#"[{"id":"C1","cage_code":"1X","agency":"DoD","labor_cats":{}}]"#,
        )
        .unwrap();
        let ds = Ingest::load_from_path(tmp.path()).unwrap();
        assert_eq!(ds.contracts.len(), 1);
        assert_eq!(ds.contracts[0].id, "C1");
        assert!(ds.employees.is_empty());
    }

    #[test]
    fn contract_by_id() {
        let mut ds = Dataset::default();
        ds.contracts.push(Contract {
            id: "C1".into(),
            cage_code: Some("1X".into()),
            agency: Some("DoD".into()),
            labor_cats: HashMap::new(),
        });
        assert!(ds.contract_by_id("C1").is_some());
        assert!(ds.contract_by_id("C2").is_none());
    }

    #[test]
    fn nexus_contract_ids_no_filter_returns_all() {
        let mut ds = Dataset::default();
        ds.contracts.push(Contract {
            id: "C1".into(),
            cage_code: None,
            agency: None,
            labor_cats: HashMap::new(),
        });
        let ids = ds.nexus_contract_ids(None, None);
        assert_eq!(ids.len(), 1);
        assert!(ids.contains("C1"));
    }

    #[test]
    fn nexus_contract_ids_filter_agency() {
        let mut ds = Dataset::default();
        ds.contracts.push(Contract {
            id: "C1".into(),
            cage_code: None,
            agency: Some("DoD".into()),
            labor_cats: HashMap::new(),
        });
        ds.contracts.push(Contract {
            id: "C2".into(),
            cage_code: None,
            agency: Some("GSA".into()),
            labor_cats: HashMap::new(),
        });
        let ids = ds.nexus_contract_ids(Some("DoD"), None);
        assert_eq!(ids.len(), 1);
        assert!(ids.contains("C1"));
    }

    #[test]
    fn nexus_contract_ids_filter_cage() {
        let mut ds = Dataset::default();
        ds.contracts.push(Contract {
            id: "C1".into(),
            cage_code: Some("1ABC".into()),
            agency: None,
            labor_cats: HashMap::new(),
        });
        let ids = ds.nexus_contract_ids(None, Some("1ABC"));
        assert_eq!(ids.len(), 1);
    }
}
