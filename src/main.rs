//! whyyoulying CLI — proactive labor category and ghost billing detection

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use whyyoulying::{Alert, Config, GhostDetector, Ingest, LaborDetector};

#[derive(Parser)]
#[command(name = "whyyoulying")]
#[command(about = "Proactive Labor Category Fraud and Ghost Billing detection")]
struct Cli {
    #[arg(long, global = true)]
    config: Option<PathBuf>,

    #[arg(long, global = true)]
    data_path: Option<PathBuf>,

    #[arg(long, global = true)]
    threshold: Option<f64>,

    #[arg(long, global = true, help = "Min confidence 0-100 (S4 false-positive control)")]
    min_confidence: Option<u8>,

    #[arg(long, global = true, help = "DoD nexus: filter by agency (e.g. DoD, Army)")]
    agency: Option<String>,

    #[arg(long, global = true, help = "DoD nexus: filter by CAGE code")]
    cage_code: Option<String>,

    #[arg(long, short, global = true, default_value = "json")]
    output: OutputFormat,

    #[command(subcommand)]
    command: Option<Commands>,

    #[arg(long)]
    test: bool,
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum OutputFormat {
    Json,
    Csv,
}

#[derive(Subcommand)]
enum Commands {
    Run,
    Ingest {
        #[arg(long)]
        path: Option<PathBuf>,
    },
    ExportReferral {
        #[arg(long)]
        path: Option<PathBuf>,
        #[arg(long, default_value_t = false, help = "FBI case-opening format (AG Guidelines)")]
        fbi: bool,
    },
}

fn main() {
    let cli = Cli::parse();

    if cli.test {
        std::process::exit(run_tests());
    }

    let result = match &cli.command {
        None | Some(Commands::Run) => run(&cli),
        Some(Commands::Ingest { path }) => cmd_ingest(&cli, path.as_deref()),
        Some(Commands::ExportReferral { path, fbi }) => cmd_export_referral(&cli, path.as_deref(), *fbi),
    };

    match result {
        Ok(exit_code) => std::process::exit(exit_code),
        Err(e) => {
            eprintln!("error: {e:?}");
            std::process::exit(2);
        }
    }
}

fn load_config(cli: &Cli) -> Result<Config> {
    let mut cfg = if let Some(ref p) = cli.config {
        Config::load_from_path(p)?
    } else {
        Config::load()?
    };
    cfg.apply_cli_overrides(
        cli.data_path.as_ref().map(|p| p.to_string_lossy().into_owned()),
        cli.threshold,
        cli.min_confidence,
        cli.agency.clone(),
        cli.cage_code.clone(),
    );
    Ok(cfg)
}

fn run(cli: &Cli) -> Result<i32> {
    let config = load_config(cli)?;
    let data_path = config
        .data_path
        .as_ref()
        .map(PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("--data-path or config data_path required"))?;

    eprintln!("loading data from {}", data_path.display());
    let ds = Ingest::load_from_path(&data_path)?;
    eprintln!(
        "loaded {} contracts, {} employees, {} labor charges, {} billing records",
        ds.contracts.len(),
        ds.employees.len(),
        ds.labor_charges.len(),
        ds.billing_records.len()
    );

    let labor = LaborDetector::new(config.labor_variance_threshold_pct);
    let ghost = GhostDetector::new();
    let labor_alerts = labor.run(&ds);
    let ghost_alerts = ghost.run(&ds);
    let mut alerts: Vec<Alert> = labor_alerts
        .into_iter()
        .chain(ghost_alerts.into_iter())
        .collect();

    let nexus_ids = ds.nexus_contract_ids(
        config.filter_agency.as_deref(),
        config.filter_cage_code.as_deref(),
    );
    alerts.retain(|a| {
        a.confidence >= config.min_confidence
            && a.contract_id
                .as_ref()
                .map_or(true, |id| nexus_ids.contains(id.as_str()))
    });

    match cli.output {
        OutputFormat::Json => {
            let out = serde_json::to_string_pretty(&alerts)?;
            println!("{out}");
        }
        OutputFormat::Csv => {
            println!("fraud_type,rule_id,severity,confidence,summary,contract_id,employee_id,cage_code,agency,timestamp");
            for a in &alerts {
                println!(
                    "{:?},{:?},{},{},{},{},{},{},{},{}",
                    a.fraud_type,
                    a.rule_id,
                    a.severity,
                    a.confidence,
                    escape_csv(&a.summary),
                    a.contract_id.as_deref().unwrap_or(""),
                    a.employee_id.as_deref().unwrap_or(""),
                    a.cage_code.as_deref().unwrap_or(""),
                    a.agency.as_deref().unwrap_or(""),
                    a.timestamp.as_deref().unwrap_or("")
                );
            }
        }
    }

    Ok(if alerts.is_empty() { 0 } else { 1 })
}

fn cmd_ingest(cli: &Cli, path: Option<&std::path::Path>) -> Result<i32> {
    let config = load_config(cli)?;
    let p = path
        .map(PathBuf::from)
        .or_else(|| config.data_path.as_ref().map(PathBuf::from))
        .ok_or_else(|| anyhow::anyhow!("--path or --data-path required"))?;
    let ds = Ingest::load_from_path(&p)?;
    eprintln!(
        "ingested: {} contracts, {} employees, {} labor charges, {} billing records",
        ds.contracts.len(),
        ds.employees.len(),
        ds.labor_charges.len(),
        ds.billing_records.len()
    );
    Ok(0)
}

fn cmd_export_referral(cli: &Cli, path: Option<&std::path::Path>, fbi_format: bool) -> Result<i32> {
    let config = load_config(cli)?;
    let data_path = config
        .data_path
        .as_ref()
        .map(PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("--data-path required for export-referral"))?;
    let ds = Ingest::load_from_path(&data_path)?;
    let labor = LaborDetector::new(config.labor_variance_threshold_pct);
    let ghost = GhostDetector::new();
    let mut alerts: Vec<Alert> = labor
        .run(&ds)
        .into_iter()
        .chain(ghost.run(&ds).into_iter())
        .collect();

    let nexus_ids = ds.nexus_contract_ids(
        config.filter_agency.as_deref(),
        config.filter_cage_code.as_deref(),
    );
    alerts.retain(|a| {
        a.confidence >= config.min_confidence
            && a.contract_id
                .as_ref()
                .map_or(true, |id| nexus_ids.contains(id.as_str()))
    });

    let out = if fbi_format {
        serde_json::to_string_pretty(&whyyoulying::export::fbi_case_opening(&alerts))?
    } else {
        serde_json::to_string_pretty(&whyyoulying::export::referral_package(&alerts))?
    };

    if let Some(p) = path {
        std::fs::write(p, &out)?;
        eprintln!("wrote {} package to {}", if fbi_format { "FBI case-opening" } else { "GAGAS referral" }, p.display());
    } else {
        println!("{out}");
    }
    Ok(if alerts.is_empty() { 0 } else { 1 })
}

fn escape_csv(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn run_tests() -> i32 {
    let mut failed = 0;

    let f49_pass = run_f49();
    let f50_pass = run_f50();
    let f51_pass = run_f51();

    let green = "\x1b[32m";
    let red = "\x1b[31m";
    let reset = "\x1b[0m";

    print!("f49 unit: ");
    if f49_pass {
        println!("{green}PASS{reset}");
    } else {
        println!("{red}FAIL{reset}");
        failed += 1;
    }
    print!("f50 integration: ");
    if f50_pass {
        println!("{green}PASS{reset}");
    } else {
        println!("{red}FAIL{reset}");
        failed += 1;
    }
    print!("f51 e2e: ");
    if f51_pass {
        println!("{green}PASS{reset}");
    } else {
        println!("{red}FAIL{reset}");
        failed += 1;
    }

    if failed > 0 {
        eprintln!("\n{failed} test(s) failed");
        1
    } else {
        eprintln!("\nall tests passed");
        0
    }
}

fn run_f49() -> bool {
    use whyyoulying::{Alert, Config, FraudType, LaborDetector, RuleId};

    let cfg = Config::default();
    assert!(cfg.labor_variance_threshold_pct > 0.0);

    let labor = LaborDetector::new(15.0);
    let ds = whyyoulying::Dataset::default();
    let alerts = labor.run(&ds);
    assert!(alerts.is_empty());

    let alert = Alert {
        fraud_type: FraudType::LaborCategory,
        rule_id: RuleId::LaborVariance,
        severity: 5,
        confidence: 85,
        summary: "test".to_string(),
        contract_id: Some("C1".to_string()),
        employee_id: Some("E1".to_string()),
        cage_code: None,
        agency: None,
        predicate_acts: None,
        timestamp: Some("2026-01-01T00:00:00Z".to_string()),
    };
    let json = serde_json::to_string(&alert).unwrap();
    assert!(json.contains("labor_category"));
    assert!(json.contains("LABOR_VARIANCE"));

    true
}

fn run_f50() -> bool {
    use tempfile::TempDir;

    let tmp = TempDir::new().unwrap();
    let p = tmp.path();

    let contracts = serde_json::json!([{"id":"C1","cage_code":null,"agency":null,"labor_cats":{"Senior":"BA"}}]);
    std::fs::write(p.join("contracts.json"), contracts.to_string()).unwrap();

    let employees = serde_json::json!([{"id":"E1","quals":["BA"],"labor_cat_min":"Junior","verified":false}]);
    std::fs::write(p.join("employees.json"), employees.to_string()).unwrap();

    let labor = serde_json::json!([{"contract_id":"C1","employee_id":"E1","labor_cat":"Principal","hours":40.0,"rate":150.0}]);
    std::fs::write(p.join("labor_charges.json"), labor.to_string()).unwrap();

    let billing = serde_json::json!([{"contract_id":"C1","employee_id":"E99","billed_hours":10.0,"billed_cat":"Junior","period":"2026-01"}]);
    std::fs::write(p.join("billing_records.json"), billing.to_string()).unwrap();

    let ds = whyyoulying::Ingest::load_from_path(p).unwrap();
    assert_eq!(ds.contracts.len(), 1);
    assert_eq!(ds.employees.len(), 1);
    assert_eq!(ds.labor_charges.len(), 1);
    assert_eq!(ds.billing_records.len(), 1);

    let labor_det = whyyoulying::LaborDetector::new(15.0);
    let ghost_det = whyyoulying::GhostDetector::new();
    let labor_alerts = labor_det.run(&ds);
    let ghost_alerts = ghost_det.run(&ds);
    assert!(!labor_alerts.is_empty());
    assert!(labor_alerts.iter().any(|a| format!("{:?}", a.rule_id).contains("LaborQualBelow")));
    assert!(!ghost_alerts.is_empty());
    assert!(ghost_alerts.iter().any(|a| format!("{:?}", a.rule_id).contains("GhostNoEmployee")));

    true
}

fn run_f51() -> bool {
    use std::path::PathBuf;
    use std::process::Command;

    let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures");
    assert!(fixtures.exists(), "f51: fixtures dir required");
    let out = Command::new(std::env::current_exe().unwrap())
        .arg("--data-path")
        .arg(&fixtures)
        .output()
        .unwrap();
    assert!(out.status.success() || out.status.code() == Some(1));
    let stdout = String::from_utf8(out.stdout).unwrap();
    let stderr = String::from_utf8(out.stderr).unwrap();
    if stdout.is_empty() && !out.status.success() {
        eprintln!("f51 stderr: {stderr}");
    }
    if !stdout.is_empty() {
        let parsed: Result<Vec<serde_json::Value>, _> = serde_json::from_str(&stdout);
        assert!(parsed.is_ok(), "f51: stdout should be valid JSON array");
    }
    true
}
