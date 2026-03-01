// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Tests for `flightctl profile` subcommands

use serde_json::Value;
use std::fs;
use tempfile::TempDir;

fn cli() -> assert_cmd::Command {
    assert_cmd::Command::new(assert_cmd::cargo_bin!("flightctl"))
}

// ── profile list ──────────────────────────────────────────────────────────

#[test]
fn profile_list_succeeds_without_daemon() {
    cli().args(["profile", "list"]).assert().success();
}

#[test]
fn profile_list_json_returns_array_data() {
    let output = cli().args(["--json", "profile", "list"]).output().unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: Value = parse_json_from(&stdout);

    assert_eq!(json["success"], true);
    assert!(json["data"].is_array());
    assert!(json["total_count"].is_number());
}

#[test]
fn profile_list_with_builtin_includes_default() {
    let output = cli()
        .args(["--json", "profile", "list", "--include-builtin"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: Value = parse_json_from(&stdout);

    let profiles = json["data"].as_array().unwrap();
    let has_builtin = profiles.iter().any(|p| p["source"] == "builtin");
    assert!(
        has_builtin,
        "Should include builtin profiles when --include-builtin is set"
    );
}

// ── profile validate ──────────────────────────────────────────────────────

#[test]
fn profile_validate_nonexistent_file_fails() {
    let output = cli()
        .args([
            "--json",
            "profile",
            "validate",
            "nonexistent_file_12345.json",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    let json: Value = parse_json_from(&stderr);
    assert_eq!(json["success"], false);
    assert!(json["error"].as_str().unwrap().contains("Failed to read"));
}

#[test]
fn profile_validate_invalid_json_fails() {
    let tmp = TempDir::new().unwrap();
    let bad_json = tmp.path().join("bad.json");
    fs::write(&bad_json, "{ not valid json }").unwrap();

    let output = cli()
        .args(["--json", "profile", "validate", bad_json.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    let json: Value = parse_json_from(&stderr);
    assert_eq!(json["success"], false);
    assert!(json["error"].as_str().unwrap().contains("Invalid JSON"));
}

// ── profile apply ─────────────────────────────────────────────────────────

#[test]
fn profile_apply_nonexistent_file_fails() {
    let output = cli()
        .args(["--json", "profile", "apply", "nonexistent_file_12345.json"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    let json: Value = parse_json_from(&stderr);
    assert_eq!(json["success"], false);
    assert!(json["error"].as_str().unwrap().contains("Failed to read"));
}

#[test]
fn profile_apply_invalid_json_fails() {
    let tmp = TempDir::new().unwrap();
    let bad_json = tmp.path().join("bad.json");
    fs::write(&bad_json, "not json at all").unwrap();

    let output = cli()
        .args(["--json", "profile", "apply", bad_json.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    let json: Value = parse_json_from(&stderr);
    assert_eq!(json["success"], false);
    assert!(json["error"].as_str().unwrap().contains("Invalid JSON"));
}

#[test]
fn profile_apply_missing_path_arg_fails() {
    cli()
        .args(["profile", "apply"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("required"));
}

// ── profile export ────────────────────────────────────────────────────────

#[test]
fn profile_export_missing_name_arg_fails() {
    cli()
        .args(["profile", "export"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("required"));
}

#[test]
fn profile_export_missing_path_arg_fails() {
    cli()
        .args(["profile", "export", "somename"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("required"));
}

// ── profile activate ──────────────────────────────────────────────────────

#[test]
fn profile_activate_missing_name_fails() {
    cli()
        .args(["profile", "activate"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("required"));
}

// ── profile show ──────────────────────────────────────────────────────────

#[test]
fn profile_show_without_name_returns_message() {
    // When no profile name given and daemon is down, should still succeed with a message
    let output = cli().args(["--json", "profile", "show"]).output().unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: Value = parse_json_from(&stdout);
    assert_eq!(json["success"], true);
    assert!(json["data"]["message"].is_string());
}

// ── Helpers ───────────────────────────────────────────────────────────────

fn parse_json_from(text: &str) -> Value {
    text.lines()
        .find(|l| l.trim().starts_with('{'))
        .and_then(|l| serde_json::from_str(l).ok())
        .unwrap_or_else(|| panic!("No valid JSON line found in:\n{}", text))
}
