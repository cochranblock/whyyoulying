//! Integration tests — full pipeline, CLI behavior.

use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

fn fixtures_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

#[test]
fn run_with_fixtures_produces_alerts() {
    let out = Command::new(env!("CARGO_BIN_EXE_whyyoulying"))
        .arg("--data-path")
        .arg(fixtures_path())
        .output()
        .unwrap();
    assert!(out.status.code() == Some(1));
    let stdout = String::from_utf8(out.stdout).unwrap();
    let parsed: Vec<serde_json::Value> = serde_json::from_str(&stdout).unwrap();
    assert!(!parsed.is_empty());
}

#[test]
fn run_with_min_confidence_filters() {
    let out = Command::new(env!("CARGO_BIN_EXE_whyyoulying"))
        .arg("--data-path")
        .arg(fixtures_path())
        .arg("--min-confidence")
        .arg("99")
        .output()
        .unwrap();
    let stdout = String::from_utf8(out.stdout).unwrap();
    let parsed: Vec<serde_json::Value> = serde_json::from_str(&stdout).unwrap();
    for a in &parsed {
        assert!(a["confidence"].as_u64().unwrap_or(0) >= 99);
    }
}

#[test]
fn ingest_subcommand() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(tmp.path().join("contracts.json"), "[]").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_whyyoulying"))
        .arg("ingest")
        .arg("--data-path")
        .arg(tmp.path())
        .output()
        .unwrap();
    assert!(out.status.success());
}

#[test]
fn run_missing_data_path_fails() {
    let out = Command::new(env!("CARGO_BIN_EXE_whyyoulying"))
        .arg("run")
        .output()
        .unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(stderr.contains("data_path") || stderr.contains("error"));
}
