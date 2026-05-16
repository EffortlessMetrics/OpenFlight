// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Tests for CLI error handling: invalid arguments, exit codes, error messages

mod common;

use common::{cli, parse_json_from};
use predicates::prelude::*;
use serde_json::Value;

// ── Invalid subcommand ────────────────────────────────────────────────────

#[test]
fn invalid_subcommand_fails() {
    cli()
        .arg("nonexistent-command")
        .assert()
        .failure()
        .stderr(predicate::str::contains("error"));
}

#[test]
fn no_subcommand_fails() {
    cli().assert().failure();
}

// ── Invalid output format ─────────────────────────────────────────────────

#[test]
fn invalid_output_format_fails() {
    cli()
        .args(["--output", "xml", "status"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid value"));
}

#[test]
fn invalid_output_format_yaml_fails() {
    cli()
        .args(["--output", "yaml", "status"])
        .assert()
        .failure();
}

// ── Invalid timeout ───────────────────────────────────────────────────────

#[test]
fn invalid_timeout_not_a_number_fails() {
    cli()
        .args(["--timeout", "not-a-number", "status"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid value"));
}

#[test]
fn negative_timeout_fails() {
    cli().args(["--timeout", "-1", "status"]).assert().failure();
}

// ── Missing required arguments ────────────────────────────────────────────

#[test]
fn devices_info_missing_device_id_fails() {
    cli()
        .args(["devices", "info"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}

#[test]
fn devices_dump_missing_device_id_fails() {
    cli()
        .args(["devices", "dump"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}

#[test]
fn devices_calibrate_missing_device_id_fails() {
    cli()
        .args(["devices", "calibrate"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}

#[test]
fn devices_test_missing_device_id_fails() {
    cli()
        .args(["devices", "test"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}

#[test]
fn profile_activate_missing_name_fails() {
    cli()
        .args(["profile", "activate"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}

#[test]
fn profile_validate_missing_path_fails() {
    cli()
        .args(["profile", "validate"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}

#[test]
fn profile_apply_missing_path_fails() {
    cli()
        .args(["profile", "apply"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}

#[test]
fn diag_trace_missing_duration_fails() {
    cli()
        .args(["diag", "trace"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}

#[test]
fn diag_record_missing_output_fails() {
    cli()
        .args(["diag", "record"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}

#[test]
fn torque_unlock_missing_device_id_fails() {
    cli()
        .args(["torque", "unlock"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}

#[test]
fn torque_set_mode_missing_mode_fails() {
    cli()
        .args(["torque", "set-mode"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("required"));
}

// ── Extra/unknown flags rejected ──────────────────────────────────────────

#[test]
fn unknown_global_flag_rejected() {
    cli()
        .args(["--nonexistent-flag", "status"])
        .assert()
        .failure();
}

#[test]
fn unknown_subcommand_flag_rejected() {
    cli()
        .args(["status", "--nonexistent-flag"])
        .assert()
        .failure();
}

// ── Exit codes ────────────────────────────────────────────────────────────

#[test]
fn connection_error_exit_code_in_valid_range() {
    let output = cli().args(["info"]).output().unwrap();

    if !output.status.success() {
        let code = output.status.code().unwrap();
        // Exit codes 1-7 are the defined error range
        assert!(
            (1..=7).contains(&code),
            "exit code should be in mapped range 1-7, got {}",
            code
        );
    }
}

#[test]
fn connection_error_json_exit_code_matches_error_code() {
    let output = cli().args(["--json", "info"]).output().unwrap();

    if !output.status.success() {
        let stderr = String::from_utf8(output.stderr).unwrap();
        let json: Value = parse_json_from(&stderr);

        let error_code = json["error_code"].as_str().unwrap();
        let exit_code = output.status.code().unwrap();

        // Verify the exit code maps to a known error code
        match error_code {
            "CONNECTION_FAILED" => assert_eq!(exit_code, 2),
            "VERSION_MISMATCH" => assert_eq!(exit_code, 3),
            "UNSUPPORTED_FEATURE" => assert_eq!(exit_code, 4),
            "TRANSPORT_ERROR" => assert_eq!(exit_code, 5),
            "SERIALIZATION_ERROR" => assert_eq!(exit_code, 6),
            "GRPC_ERROR" => assert_eq!(exit_code, 7),
            "UNKNOWN_ERROR" => assert_eq!(exit_code, 1),
            _ => panic!("Unknown error code: {}", error_code),
        }
    }
}

#[test]
fn parse_error_exit_code_is_nonzero() {
    let output = cli()
        .args(["--output", "invalid", "status"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_ne!(output.status.code(), Some(0));
}

// ── Overlay invalid severity ──────────────────────────────────────────────

#[test]
fn overlay_notify_invalid_severity_fails() {
    let output = cli()
        .args([
            "--json",
            "overlay",
            "notify",
            "test",
            "--severity",
            "invalid",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).unwrap();
    let json: Value = parse_json_from(&stderr);
    assert_eq!(json["success"], false);
    assert!(json["error"].as_str().unwrap().contains("severity"));
}

// ── Graceful handling (no panics) ─────────────────────────────────────────

#[test]
fn daemon_dependent_commands_do_not_panic() {
    let commands: Vec<Vec<&str>> = vec![
        vec!["status"],
        vec!["info"],
        vec!["devices", "list"],
        vec!["diag", "health"],
        vec!["safe-mode"],
        vec!["diagnostics"],
        vec!["adapters", "status"],
        vec!["adapters", "enable", "msfs"],
        vec!["adapters", "disable", "xplane"],
        vec!["adapters", "reconnect", "dcs"],
        vec!["devices", "calibrate", "test-dev"],
        vec!["devices", "test", "test-dev"],
        vec!["torque", "status"],
    ];

    for args in &commands {
        let output = cli().args(args).output().unwrap();
        assert_ne!(
            output.status.code(),
            Some(101),
            "'flightctl {}' panicked",
            args.join(" ")
        );
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────
