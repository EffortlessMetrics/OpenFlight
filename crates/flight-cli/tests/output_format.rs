// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Tests for CLI output formatting: --json flag, --output json/human, and JSON structure

use serde_json::Value;

fn cli() -> assert_cmd::Command {
    assert_cmd::Command::new(assert_cmd::cargo_bin!("flightctl"))
}

// ── JSON output via --output json ─────────────────────────────────────────

#[test]
fn output_json_flag_produces_valid_json_on_success() {
    let output = cli().args(["--output", "json", "status"]).output().unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: Value = parse_json_from(&stdout);
    assert_eq!(json["success"], true);
}

#[test]
fn output_json_flag_produces_valid_json_on_error() {
    let output = cli().args(["--output", "json", "info"]).output().unwrap();
    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).unwrap();
    let json: Value = parse_json_from(&stderr);
    assert_eq!(json["success"], false);
    assert!(json["error"].is_string());
    assert!(json["error_code"].is_string());
}

#[test]
fn output_json_list_response_has_data_array() {
    let output = cli()
        .args(["--output", "json", "profile", "list"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: Value = parse_json_from(&stdout);
    assert_eq!(json["success"], true);
    assert!(json["data"].is_array());
    assert!(json["total_count"].is_number());
}

// ── --json shorthand ──────────────────────────────────────────────────────

#[test]
fn json_shorthand_flag_equivalent_to_output_json() {
    let output_long = cli().args(["--output", "json", "status"]).output().unwrap();
    let output_short = cli().args(["--json", "status"]).output().unwrap();

    let json_long: Value = parse_json_from(&String::from_utf8(output_long.stdout.clone()).unwrap());
    let json_short: Value =
        parse_json_from(&String::from_utf8(output_short.stdout.clone()).unwrap());

    // Both should have the same structure
    assert_eq!(json_long["success"], json_short["success"]);
    assert_eq!(
        json_long["data"]["service_status"],
        json_short["data"]["service_status"]
    );
}

// ── Human output ──────────────────────────────────────────────────────────

#[test]
fn human_output_is_default_format() {
    let output = cli().args(["status"]).output().unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    // Human format should NOT start with '{'
    let first_non_empty = stdout.lines().find(|l| !l.trim().is_empty());
    if let Some(line) = first_non_empty {
        assert!(
            !line.trim().starts_with('{'),
            "Human output should not be JSON: {}",
            line
        );
    }
}

#[test]
fn human_error_output_starts_with_error_prefix() {
    let output = cli().args(["--output", "human", "info"]).output().unwrap();
    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.lines().any(|l| l.starts_with("Error:")),
        "Human error output should start with 'Error:': {}",
        stderr
    );
}

// ── Overlay commands work locally (no daemon needed) ──────────────────────

#[test]
fn overlay_show_json_output_is_valid() {
    let output = cli().args(["--json", "overlay", "show"]).output().unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(json["action"], "show");
    assert_eq!(json["queued"], true);
}

#[test]
fn overlay_hide_json_output_is_valid() {
    let output = cli().args(["--json", "overlay", "hide"]).output().unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(json["action"], "hide");
}

#[test]
fn overlay_toggle_json_output_is_valid() {
    let output = cli()
        .args(["--json", "overlay", "toggle"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(json["action"], "toggle");
    assert_eq!(json["queued"], true);
}

#[test]
fn overlay_notify_json_output_is_valid() {
    let output = cli()
        .args(["--json", "overlay", "notify", "test message"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(json["action"], "notify");
    assert_eq!(json["message"], "test message");
    assert_eq!(json["severity"], "info");
    assert_eq!(json["queued"], true);
}

#[test]
fn overlay_backends_json_output_is_array() {
    let output = cli()
        .args(["--json", "overlay", "backends"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: Value = serde_json::from_str(stdout.trim()).unwrap();
    assert!(json.is_array());
    assert!(!json.as_array().unwrap().is_empty());
}

// ── JSON contract stability ───────────────────────────────────────────────

#[test]
fn success_json_always_has_success_field() {
    let output = cli().args(["--json", "status"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: Value = parse_json_from(&stdout);
    assert!(
        json.get("success").is_some(),
        "Success responses must always have a 'success' field"
    );
}

#[test]
fn error_json_always_has_error_and_error_code_fields() {
    let output = cli().args(["--json", "info"]).output().unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    let json: Value = parse_json_from(&stderr);
    assert!(
        json.get("error").is_some(),
        "Error responses must have 'error' field"
    );
    assert!(
        json.get("error_code").is_some(),
        "Error responses must have 'error_code' field"
    );
}

// ── Helpers ───────────────────────────────────────────────────────────────

fn parse_json_from(text: &str) -> Value {
    text.lines()
        .find(|l| l.trim().starts_with('{'))
        .and_then(|l| serde_json::from_str(l).ok())
        .unwrap_or_else(|| panic!("No valid JSON line found in:\n{}", text))
}
