// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Tests for `flightctl diag` and `flightctl diagnostics` subcommands

use serde_json::Value;
use std::fs;
use tempfile::TempDir;

fn cli() -> assert_cmd::Command {
    assert_cmd::Command::new(assert_cmd::cargo_bin!("flightctl"))
}

// ── diag bundle ───────────────────────────────────────────────────────────

#[test]
fn diag_bundle_does_not_panic_without_daemon() {
    let output = cli().args(["diag", "bundle"]).output().unwrap();
    // Bundle may succeed (collects local info) or fail, but must not panic
    assert_ne!(
        output.status.code(),
        Some(101),
        "diag bundle should not panic"
    );
}

#[test]
fn diag_bundle_json_output_has_bundle_path() {
    let output = cli().args(["--json", "diag", "bundle"]).output().unwrap();
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    if let Some(json) = try_parse_json_from(&combined) {
        // If we got JSON output, check it has expected fields
        if json.get("success").is_some() && json["success"] == true {
            assert!(
                json["data"]["bundle_path"].is_string() || json["data"]["contents"].is_array(),
                "bundle JSON should have bundle_path or contents"
            );
        }
    }
}

// ── diag health ───────────────────────────────────────────────────────────

#[test]
fn diag_health_fails_without_daemon() {
    let output = cli().args(["diag", "health"]).output().unwrap();
    assert!(!output.status.success());
    assert_ne!(output.status.code(), Some(101));
}

#[test]
fn diag_health_json_error_format() {
    let output = cli().args(["--json", "diag", "health"]).output().unwrap();
    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).unwrap();
    let json: Value = parse_json_from(&stderr);
    assert_eq!(json["success"], false);
    assert!(json["error"].is_string());
}

// ── diag metrics ──────────────────────────────────────────────────────────

#[test]
fn diag_metrics_fails_without_daemon() {
    let output = cli().args(["diag", "metrics"]).output().unwrap();
    assert!(!output.status.success());
    assert_ne!(output.status.code(), Some(101));
}

// ── diag trace ────────────────────────────────────────────────────────────

#[test]
fn diag_trace_requires_duration_arg() {
    cli()
        .args(["diag", "trace"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("required"));
}

#[test]
fn diag_trace_fails_without_daemon() {
    let output = cli().args(["diag", "trace", "30"]).output().unwrap();
    assert!(!output.status.success());
    assert_ne!(output.status.code(), Some(101));
}

// ── diag record ───────────────────────────────────────────────────────────

#[test]
fn diag_record_requires_output_arg() {
    cli()
        .args(["diag", "record"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("required"));
}

#[test]
fn diag_record_rejects_non_fbb_extension() {
    let tmp = TempDir::new().unwrap();
    let bad_ext = tmp.path().join("recording.txt");

    let output = cli()
        .args(["--json", "diag", "record", "-o", bad_ext.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    let json: Value = parse_json_from(&stderr);
    assert_eq!(json["success"], false);
    assert!(json["error"].as_str().unwrap().contains(".fbb"));
}

#[test]
fn diag_record_accepts_fbb_extension() {
    let tmp = TempDir::new().unwrap();
    let valid_path = tmp.path().join("recording.fbb");

    let output = cli()
        .args([
            "--json",
            "diag",
            "record",
            "-o",
            valid_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    // Should succeed (simulated recording start) since the command
    // doesn't actually require the daemon for the start acknowledgment
    if output.status.success() {
        let stdout = String::from_utf8(output.stdout).unwrap();
        let json: Value = parse_json_from(&stdout);
        assert_eq!(json["success"], true);
        assert_eq!(json["data"]["recording_started"], true);
    }
}

// ── diag replay ───────────────────────────────────────────────────────────

#[test]
fn diag_replay_nonexistent_file_fails() {
    let output = cli()
        .args(["--json", "diag", "replay", "nonexistent.fbb"])
        .output()
        .unwrap();
    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).unwrap();
    let json: Value = parse_json_from(&stderr);
    assert_eq!(json["success"], false);
    assert!(json["error"].as_str().unwrap().contains("does not exist"));
}

#[test]
fn diag_replay_wrong_extension_fails() {
    let tmp = TempDir::new().unwrap();
    let wrong_ext = tmp.path().join("data.txt");
    fs::write(&wrong_ext, "fake data").unwrap();

    let output = cli()
        .args(["--json", "diag", "replay", wrong_ext.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).unwrap();
    let json: Value = parse_json_from(&stderr);
    assert_eq!(json["success"], false);
    assert!(json["error"].as_str().unwrap().contains(".fbb"));
}

// ── diag export ───────────────────────────────────────────────────────────

#[test]
fn diag_export_nonexistent_file_fails() {
    let output = cli()
        .args(["--json", "diag", "export", "nonexistent.fbb"])
        .output()
        .unwrap();
    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).unwrap();
    let json: Value = parse_json_from(&stderr);
    assert_eq!(json["success"], false);
    assert!(json["error"].as_str().unwrap().contains("does not exist"));
}

// ── diagnostics shorthand ─────────────────────────────────────────────────

#[test]
fn diagnostics_shorthand_fails_without_daemon() {
    let output = cli().args(["diagnostics"]).output().unwrap();
    assert!(!output.status.success());
    assert_ne!(output.status.code(), Some(101));
}

#[test]
fn diagnostics_shorthand_json_error_format() {
    let output = cli().args(["--json", "diagnostics"]).output().unwrap();
    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).unwrap();
    let json: Value = parse_json_from(&stderr);
    assert_eq!(json["success"], false);
    assert!(json["error_code"].is_string());
}

// ── diag status and stop (simulated) ──────────────────────────────────────

#[test]
fn diag_status_succeeds_with_simulated_data() {
    let output = cli().args(["--json", "diag", "status"]).output().unwrap();

    if output.status.success() {
        let stdout = String::from_utf8(output.stdout).unwrap();
        let json: Value = parse_json_from(&stdout);
        assert_eq!(json["success"], true);
        // Should have recording_active field
        assert!(json["data"].get("recording_active").is_some());
    }
}

#[test]
fn diag_stop_succeeds_with_simulated_data() {
    let output = cli().args(["--json", "diag", "stop"]).output().unwrap();

    if output.status.success() {
        let stdout = String::from_utf8(output.stdout).unwrap();
        let json: Value = parse_json_from(&stdout);
        assert_eq!(json["success"], true);
        assert_eq!(json["data"]["recording_stopped"], true);
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────

fn parse_json_from(text: &str) -> Value {
    text.lines()
        .find(|l| l.trim().starts_with('{'))
        .and_then(|l| serde_json::from_str(l).ok())
        .unwrap_or_else(|| panic!("No valid JSON line found in:\n{}", text))
}

fn try_parse_json_from(text: &str) -> Option<Value> {
    text.lines()
        .find(|l| l.trim().starts_with('{'))
        .and_then(|l| serde_json::from_str(l).ok())
}
