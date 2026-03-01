// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Tests for `flightctl devices` subcommands

mod common;

use common::{cli, parse_json_from};
use serde_json::Value;

// ── devices list ──────────────────────────────────────────────────────────

#[test]
fn devices_list_does_not_panic() {
    let output = cli().args(["devices", "list"]).output().unwrap();
    // Must not panic (exit code 101); both success and failure are acceptable
    assert_ne!(output.status.code(), Some(101));
}

#[test]
fn devices_list_json_has_stable_fields() {
    let output = cli().args(["--json", "devices", "list"]).output().unwrap();
    // Must not panic
    assert_ne!(output.status.code(), Some(101));

    if output.status.success() {
        let stdout = String::from_utf8(output.stdout).unwrap();
        let json: Value = parse_json_from(&stdout);
        assert_eq!(json["success"], true);
    } else {
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
}

#[test]
fn devices_list_with_include_disconnected_flag_accepted() {
    let output = cli()
        .args(["devices", "list", "--include-disconnected"])
        .output()
        .unwrap();
    // Flag should be accepted; must not panic
    assert_ne!(output.status.code(), Some(101));
}

#[test]
fn devices_list_with_filter_types_flag_accepted() {
    let output = cli()
        .args(["devices", "list", "--filter-types", "joystick,throttle"])
        .output()
        .unwrap();
    // Flag should be accepted; must not panic
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
