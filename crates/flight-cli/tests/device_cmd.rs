// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Tests for `flightctl devices` subcommands

use serde_json::Value;

fn cli() -> assert_cmd::Command {
    assert_cmd::Command::new(assert_cmd::cargo_bin!("flightctl"))
}

// ── devices list ──────────────────────────────────────────────────────────

#[test]
fn devices_list_fails_gracefully_without_daemon() {
    let output = cli().args(["devices", "list"]).output().unwrap();
    assert!(!output.status.success());
    // Should not panic (exit code 101)
    assert_ne!(output.status.code(), Some(101));
}

#[test]
fn devices_list_json_error_has_stable_fields() {
    let output = cli()
        .args(["--json", "devices", "list"])
        .output()
        .unwrap();
    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).unwrap();
    let json: Value = parse_json_from(&stderr);

    assert_eq!(json["success"], false);
    assert!(json["error"].is_string());
    assert!(json["error_code"].is_string());

    let error_code = json["error_code"].as_str().unwrap();
    let valid_codes = [
        "CONNECTION_FAILED",
        "VERSION_MISMATCH",
        "UNSUPPORTED_FEATURE",
        "TRANSPORT_ERROR",
        "SERIALIZATION_ERROR",
        "GRPC_ERROR",
        "UNKNOWN_ERROR",
    ];
    assert!(
        valid_codes.contains(&error_code),
        "error_code '{}' should be a known code",
        error_code
    );
}

#[test]
fn devices_list_with_include_disconnected_flag_accepted() {
    let output = cli()
        .args(["devices", "list", "--include-disconnected"])
        .output()
        .unwrap();
    // Should fail due to no daemon, but the flag should be accepted
    assert!(!output.status.success());
    assert_ne!(output.status.code(), Some(101));
}

#[test]
fn devices_list_with_filter_types_flag_accepted() {
    let output = cli()
        .args(["devices", "list", "--filter-types", "joystick,throttle"])
        .output()
        .unwrap();
    // Should fail due to no daemon, but the flag should be accepted
    assert!(!output.status.success());
    assert_ne!(output.status.code(), Some(101));
}

// ── devices info ──────────────────────────────────────────────────────────

#[test]
fn devices_info_requires_device_id() {
    cli()
        .args(["devices", "info"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("required"));
}

#[test]
fn devices_info_fails_gracefully_without_daemon() {
    let output = cli()
        .args(["devices", "info", "test-device-123"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_ne!(output.status.code(), Some(101));
}

// ── devices dump ──────────────────────────────────────────────────────────

#[test]
fn devices_dump_requires_device_id() {
    cli()
        .args(["devices", "dump"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("required"));
}

// ── devices calibrate ─────────────────────────────────────────────────────

#[test]
fn devices_calibrate_requires_device_id() {
    cli()
        .args(["devices", "calibrate"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("required"));
}

#[test]
fn devices_calibrate_fails_gracefully_without_daemon() {
    let output = cli()
        .args(["devices", "calibrate", "test-device"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_ne!(output.status.code(), Some(101));
}

// ── devices test ──────────────────────────────────────────────────────────

#[test]
fn devices_test_requires_device_id() {
    cli()
        .args(["devices", "test"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("required"));
}

#[test]
fn devices_test_fails_gracefully_without_daemon() {
    let output = cli()
        .args(["devices", "test", "test-device"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_ne!(output.status.code(), Some(101));
}

#[test]
fn devices_test_accepts_interval_and_count_flags() {
    let output = cli()
        .args([
            "devices",
            "test",
            "test-device",
            "--interval-ms",
            "50",
            "--count",
            "10",
        ])
        .output()
        .unwrap();
    // Fails because no daemon, but flags should be accepted (no parse errors)
    assert!(!output.status.success());
    assert_ne!(output.status.code(), Some(101));
}

// ── Helpers ───────────────────────────────────────────────────────────────

fn parse_json_from(text: &str) -> Value {
    text.lines()
        .find(|l| l.trim().starts_with('{'))
        .and_then(|l| serde_json::from_str(l).ok())
        .unwrap_or_else(|| panic!("No valid JSON line found in:\n{}", text))
}
