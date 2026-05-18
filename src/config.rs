//! Configuration for data sources and detection thresholds.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("labor_variance_threshold_pct must be in (0, 100], got {0}")]
    InvalidThreshold(f64),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub labor_variance_threshold_pct: f64,
    pub data_path: Option<String>,
    /// Min confidence 0-100 to include (S4 false-positive control).
    #[serde(default = "default_min_confidence")]
    pub min_confidence: u8,
    /// DoD nexus: filter by agency (e.g. DoD, Army, Navy).
    pub filter_agency: Option<String>,
    /// DoD nexus: filter by CAGE code.
    pub filter_cage_code: Option<String>,
    /// Rate inflation threshold (% variance billed vs payroll).
    #[serde(default = "default_rate_inflation_threshold_pct")]
    pub rate_inflation_threshold_pct: f64,
    /// Weekly hours threshold for overtime padding detector.
    #[serde(default = "default_overtime_weekly")]
    pub overtime_weekly: f64,
    /// Monthly hours threshold for overtime padding detector.
    #[serde(default = "default_overtime_monthly")]
    pub overtime_monthly: f64,
}

fn default_min_confidence() -> u8 {
    50
}

fn default_rate_inflation_threshold_pct() -> f64 {
    15.0
}

fn default_overtime_weekly() -> f64 {
    60.0
}

fn default_overtime_monthly() -> f64 {
    240.0
}

impl Default for Config {
    fn default() -> Self {
        Self {
            labor_variance_threshold_pct: 15.0,
            data_path: None,
            min_confidence: 50,
            filter_agency: None,
            filter_cage_code: None,
            rate_inflation_threshold_pct: default_rate_inflation_threshold_pct(),
            overtime_weekly: default_overtime_weekly(),
            overtime_monthly: default_overtime_monthly(),
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        Ok(Self::default())
    }

    pub fn load_from_path(path: &Path) -> Result<Self> {
        let s = std::fs::read_to_string(path)
            .with_context(|| format!("read config: {}", path.display()))?;
        let cfg: Self = serde_json::from_str(&s)
            .with_context(|| format!("parse config: {}", path.display()))?;
        if cfg.labor_variance_threshold_pct <= 0.0 || cfg.labor_variance_threshold_pct > 100.0 {
            return Err(ConfigError::InvalidThreshold(cfg.labor_variance_threshold_pct).into());
        }
        Ok(cfg)
    }

    pub fn apply_cli_overrides(
        &mut self,
        data_path: Option<String>,
        threshold: Option<f64>,
        min_confidence: Option<u8>,
        filter_agency: Option<String>,
        filter_cage_code: Option<String>,
    ) -> Result<(), ConfigError> {
        if let Some(p) = data_path {
            self.data_path = Some(p);
        }
        if let Some(t) = threshold {
            if t <= 0.0 || t > 100.0 {
                return Err(ConfigError::InvalidThreshold(t));
            }
            self.labor_variance_threshold_pct = t;
        }
        if let Some(c) = min_confidence {
            self.min_confidence = c;
        }
        if filter_agency.is_some() {
            self.filter_agency = filter_agency;
        }
        if filter_cage_code.is_some() {
            self.filter_cage_code = filter_cage_code;
        }
        Ok(())
    }

    pub fn apply_detector_overrides(
        &mut self,
        rate_inflation_threshold: Option<f64>,
        overtime_weekly: Option<f64>,
        overtime_monthly: Option<f64>,
    ) {
        if let Some(v) = rate_inflation_threshold {
            self.rate_inflation_threshold_pct = v;
        }
        if let Some(v) = overtime_weekly {
            self.overtime_weekly = v;
        }
        if let Some(v) = overtime_monthly {
            self.overtime_monthly = v;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn default_has_valid_threshold() {
        let c = Config::default();
        assert!(c.labor_variance_threshold_pct > 0.0);
        assert!(c.labor_variance_threshold_pct <= 100.0);
        assert!(c.min_confidence <= 100);
    }

    #[test]
    fn load_returns_default() {
        let c = Config::load().unwrap();
        assert_eq!(c.labor_variance_threshold_pct, 15.0);
    }

    #[test]
    fn load_from_path_valid() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        write!(tmp.as_file(), r#"{{"labor_variance_threshold_pct":20.0,"data_path":"/x"}}"#).unwrap();
        let c = Config::load_from_path(tmp.path()).unwrap();
        assert_eq!(c.labor_variance_threshold_pct, 20.0);
        assert_eq!(c.data_path.as_deref(), Some("/x"));
    }

    #[test]
    fn load_from_path_rejects_zero_threshold() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        write!(tmp.as_file(), r#"{{"labor_variance_threshold_pct":0}}"#).unwrap();
        assert!(Config::load_from_path(tmp.path()).is_err());
    }

    #[test]
    fn load_from_path_rejects_over_100_threshold() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        write!(tmp.as_file(), r#"{{"labor_variance_threshold_pct":101}}"#).unwrap();
        assert!(Config::load_from_path(tmp.path()).is_err());
    }

    #[test]
    fn apply_cli_overrides() {
        let mut c = Config::default();
        c.apply_cli_overrides(Some("x".into()), Some(25.0), Some(80), Some("DoD".into()), None)
            .unwrap();
        assert_eq!(c.data_path.as_deref(), Some("x"));
        assert_eq!(c.labor_variance_threshold_pct, 25.0);
        assert_eq!(c.min_confidence, 80);
        assert_eq!(c.filter_agency.as_deref(), Some("DoD"));
    }

    #[test]
    fn apply_cli_overrides_cage_code() {
        let mut c = Config::default();
        c.apply_cli_overrides(None, None, None, None, Some("1ABC2".into()))
            .unwrap();
        assert_eq!(c.filter_cage_code.as_deref(), Some("1ABC2"));
    }

    #[test]
    fn apply_cli_overrides_rejects_invalid_threshold() {
        let mut c = Config::default();
        assert!(c.apply_cli_overrides(None, Some(0.0), None, None, None).is_err());
        assert!(c.apply_cli_overrides(None, Some(101.0), None, None, None).is_err());
    }

    #[test]
    fn load_from_path_missing_file() {
        let tmp = tempfile::TempDir::new().unwrap();
        let p = tmp.path().join("nonexistent.json");
        assert!(Config::load_from_path(&p).is_err());
    }

    #[test]
    fn load_from_path_invalid_json() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        write!(tmp.as_file(), "not json").unwrap();
        assert!(Config::load_from_path(tmp.path()).is_err());
    }

    #[test]
    fn load_from_path_uses_default_min_confidence() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        write!(tmp.as_file(), r#"{{"labor_variance_threshold_pct":10}}"#).unwrap();
        let c = Config::load_from_path(tmp.path()).unwrap();
        assert_eq!(c.min_confidence, 50);
    }
}
