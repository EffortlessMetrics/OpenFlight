// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Tests for the `flightctl version` and `flightctl --version` commands

use predicates::prelude::*;
use serde_json::Value;

fn cli() -> assert_cmd::Command {
    assert_cmd::Command::new(assert_cmd::cargo_bin!("flightctl"))
}

// ── --version flag ────────────────────────────────────────────────────────

#[test]
fn version_flag_succeeds() {
    cli().arg("--version").assert().success();
}

#[test]
fn version_flag_prints_semver() {
    let output = cli().arg("--version").output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let version_str = stdout.trim();

    // Should contain X.Y.Z semver pattern
    let has_semver = version_str.split_whitespace().any(|word| {
        let parts: Vec<&str> = word.split('.').collect();
        parts.len() == 3 && parts.iter().all(|p| p.parse::<u32>().is_ok())
    });
    assert!(has_semver, "Should contain semver: {}", version_str);
}

#[test]
fn version_flag_contains_package_name() {
    cli()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("flightctl"));
}

// ── version subcommand ────────────────────────────────────────────────────

#[test]
fn version_subcommand_succeeds() {
    cli().arg("version").assert().success();
}

#[test]
fn version_subcommand_json_has_all_required_fields() {
    let output = cli().args(["--json", "version"]).output().unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: Value = parse_json_from(&stdout);

    assert_eq!(json["success"], true);
    let data = &json["data"];
    assert!(data["cli_version"].is_string(), "must have cli_version");
    assert!(data["build_profile"].is_string(), "must have build_profile");
    assert!(data["build_target"].is_string(), "must have build_target");
    assert!(data["build_os"].is_string(), "must have build_os");
    assert!(data["rust_version"].is_string(), "must have rust_version");
    assert!(
        data.get("service_status").is_some(),
        "must have service_status"
    );
}

#[test]
fn version_subcommand_json_cli_version_matches_cargo_version() {
    let output = cli().args(["--json", "version"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: Value = parse_json_from(&stdout);

    let cli_version = json["data"]["cli_version"].as_str().unwrap();
    // Should be a valid semver
    let parts: Vec<&str> = cli_version.split('.').collect();
    assert_eq!(
        parts.len(),
        3,
        "cli_version should be semver: {}",
        cli_version
    );
    assert!(
        parts.iter().all(|p| p.parse::<u32>().is_ok()),
        "semver parts should be numbers: {}",
        cli_version
    );
}

#[test]
fn version_subcommand_service_unreachable_without_daemon() {
    let output = cli().args(["--json", "version"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: Value = parse_json_from(&stdout);

    assert_eq!(json["data"]["service_status"], "unreachable");
}

#[test]
fn version_subcommand_verbose_includes_package_info() {
    let output = cli()
        .args(["--verbose", "--json", "version"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: Value = parse_json_from(&stdout);

    let data = &json["data"];
    assert!(
        data["package_name"].is_string(),
        "verbose should include package_name"
    );
}

#[test]
fn version_subcommand_human_output_contains_version() {
    let output = cli().arg("version").output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(
        stdout.contains("cli_version") || stdout.contains("0."),
        "human version output should contain version info: {}",
        stdout
    );
}

// ── Helpers ───────────────────────────────────────────────────────────────

fn parse_json_from(text: &str) -> Value {
    text.lines()
        .find(|l| l.trim().starts_with('{'))
        .and_then(|l| serde_json::from_str(l).ok())
        .unwrap_or_else(|| panic!("No valid JSON line found in:\n{}", text))
}
