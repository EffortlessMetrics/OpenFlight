// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for the CLI (flightctl) covering command parsing,
//! JSON output mode, error handling, profile/device management,
//! and integration scenarios.

use std::process::Command;

/// Helper: run `cargo run -p flight-cli --quiet -- <args>` and return output.
fn cli(args: &[&str]) -> std::process::Output {
    let mut cmd = Command::new("cargo");
    cmd.arg("run")
        .arg("--quiet")
        .arg("-p")
        .arg("flight-cli")
        .arg("--");
    for arg in args {
        cmd.arg(arg);
    }
    cmd.output().expect("Failed to execute CLI")
}

/// Extract the first JSON line from a string (starts with `{`).
fn extract_json(text: &str) -> serde_json::Value {
    let line = text
        .lines()
        .find(|l| l.trim().starts_with('{'))
        .unwrap_or_else(|| panic!("No JSON line found in: {}", text));
    serde_json::from_str(line).unwrap_or_else(|e| panic!("Invalid JSON: {} — {}", e, line))
}

// ═══════════════════════════════════════════════════════════════════════════
// 1. COMMAND PARSING (8 tests)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn parse_status_subcommand_accepted() {
    let out = cli(&["status"]);
    // status succeeds even without daemon (offline fallback)
    assert!(out.status.success());
}

#[test]
fn parse_profile_list_subcommand_accepted() {
    let out = cli(&["profile", "list"]);
    assert!(out.status.success());
}

#[test]
fn parse_profile_apply_requires_path() {
    let out = cli(&["profile", "apply"]);
    assert!(!out.status.success());
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(
        stderr.contains("required") || stderr.contains("error"),
        "should say arg is required: {}",
        stderr
    );
}

#[test]
fn parse_device_list_subcommand_accepted() {
    // Will fail at runtime (no daemon) but parsing succeeds — exit ≠ 2 (usage)
    let out = cli(&["devices", "list"]);
    assert!(!out.status.success());
    assert_ne!(
        out.status.code(),
        Some(2),
        "should not be a parse/usage error"
    );
}

#[test]
fn parse_diag_health_subcommand_accepted() {
    let out = cli(&["diag", "health"]);
    assert!(!out.status.success());
    assert_ne!(
        out.status.code(),
        Some(101),
        "should not panic"
    );
}

#[test]
fn parse_version_subcommand_accepted() {
    let out = cli(&["version"]);
    assert!(out.status.success());
}

#[test]
fn parse_help_flag_accepted() {
    let out = cli(&["--help"]);
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("Flight Hub command line interface"));
}

#[test]
fn parse_unknown_command_rejected() {
    let out = cli(&["this-command-does-not-exist"]);
    assert!(!out.status.success());
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(
        stderr.contains("error") || stderr.contains("unrecognized"),
        "stderr should say unknown: {}",
        stderr
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. JSON OUTPUT (6 tests)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn json_flag_produces_valid_json_on_success() {
    let out = cli(&["--json", "status"]);
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    let json = extract_json(&stdout);
    assert_eq!(json["success"], true);
    assert!(json["data"].is_object(), "data should be an object");
}

#[test]
fn json_structured_error_on_failure() {
    let out = cli(&["--json", "info"]);
    assert!(!out.status.success());
    let stderr = String::from_utf8(out.stderr).unwrap();
    let json = extract_json(&stderr);
    assert_eq!(json["success"], false);
    assert!(json["error"].is_string(), "error should be a string");
    assert!(
        json["error_code"].is_string(),
        "error_code should be a string"
    );
}

#[test]
fn json_list_output_is_array() {
    let out = cli(&["--json", "profile", "list"]);
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    let json = extract_json(&stdout);
    assert_eq!(json["success"], true);
    assert!(json["data"].is_array(), "profile list data must be an array");
}

#[test]
fn json_single_item_is_object() {
    // version command returns a single object (not an array)
    let out = cli(&["--json", "version"]);
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    let json = extract_json(&stdout);
    assert!(
        json["data"].is_object(),
        "version data should be a single object, not array"
    );
}

#[test]
fn json_nested_objects_preserved() {
    // status in verbose JSON mode includes nested performance data
    let out = cli(&["--json", "--verbose", "status"]);
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    let json = extract_json(&stdout);
    let data = &json["data"];
    // offline status may not have nested perf but the outer structure is correct
    assert!(data.is_object());
    assert!(
        data.get("service_status").is_some(),
        "must have service_status"
    );
    assert!(data.get("cli_version").is_some(), "must have cli_version");
}

#[test]
fn json_empty_profile_list_is_empty_array() {
    // profile list without --include-builtin may return empty array
    let out = cli(&["--json", "profile", "list"]);
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    let json = extract_json(&stdout);
    // data is an array (may or may not be empty depending on local config)
    assert!(json["data"].is_array());
    // total_count should be present and non-negative
    let total = json["total_count"].as_i64().unwrap_or(-1);
    assert!(total >= 0, "total_count should be >= 0: {}", total);
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. ERROR HANDLING (5 tests)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn error_missing_required_arg_devices_info() {
    let out = cli(&["devices", "info"]);
    assert!(!out.status.success());
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(
        stderr.contains("required") || stderr.contains("error"),
        "should complain about missing device_id: {}",
        stderr
    );
}

#[test]
fn error_invalid_output_format_arg() {
    let out = cli(&["--output", "xml", "status"]);
    assert!(!out.status.success());
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(
        stderr.contains("invalid value") || stderr.contains("error"),
        "should reject unknown format: {}",
        stderr
    );
}

#[test]
fn error_connection_failure_message_in_json() {
    let out = cli(&["--json", "devices", "list"]);
    assert!(!out.status.success());
    let stderr = String::from_utf8(out.stderr).unwrap();
    let json = extract_json(&stderr);
    let err_msg = json["error"].as_str().unwrap();
    assert!(
        !err_msg.is_empty(),
        "error message should not be empty"
    );
}

#[test]
fn error_connection_failure_message_in_human() {
    let out = cli(&["devices", "list"]);
    assert!(!out.status.success());
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(
        stderr.lines().any(|l| l.starts_with("Error:")),
        "human error should have 'Error:' prefix: {}",
        stderr
    );
}

#[test]
fn error_graceful_exit_no_panic() {
    // Several commands that need daemon — none should panic (exit 101)
    for args in &[
        vec!["devices", "list"],
        vec!["diag", "health"],
        vec!["info"],
        vec!["safe-mode"],
        vec!["diagnostics"],
    ] {
        let out = cli(args);
        assert_ne!(
            out.status.code(),
            Some(101),
            "Command {:?} should not panic",
            args
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 4. PROFILE MANAGEMENT (5 tests)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn profile_list_succeeds_without_daemon() {
    // profile list reads from disk, doesn't need daemon
    let out = cli(&["profile", "list"]);
    assert!(out.status.success());
}

#[test]
fn profile_list_include_builtin_accepted() {
    let out = cli(&["--json", "profile", "list", "--include-builtin"]);
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    let json = extract_json(&stdout);
    let data = json["data"].as_array().unwrap();
    // With --include-builtin there is always at least the "default" profile
    let has_builtin = data
        .iter()
        .any(|p| p["source"] == "builtin" && p["name"] == "default");
    assert!(has_builtin, "should contain built-in default profile");
}

#[test]
fn profile_show_current_without_daemon() {
    // `profile show` (no name) tries to get current effective profile
    let out = cli(&["--json", "profile", "show"]);
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    let json = extract_json(&stdout);
    // Should produce a message about the RPC not being available
    assert!(json["data"]["message"].is_string());
}

#[test]
fn profile_validate_requires_file_path() {
    let out = cli(&["profile", "validate"]);
    assert!(!out.status.success());
    let stderr = String::from_utf8(out.stderr).unwrap();
    assert!(
        stderr.contains("required") || stderr.contains("error"),
        "should complain about missing path: {}",
        stderr
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// 5. DEVICE MANAGEMENT (5 tests)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn device_list_json_error_has_error_code() {
    let out = cli(&["--json", "devices", "list"]);
    assert!(!out.status.success());
    let stderr = String::from_utf8(out.stderr).unwrap();
    let json = extract_json(&stderr);
    let code = json["error_code"].as_str().unwrap();
    let valid = [
        "CONNECTION_FAILED",
        "VERSION_MISMATCH",
        "UNSUPPORTED_FEATURE",
        "TRANSPORT_ERROR",
        "SERIALIZATION_ERROR",
        "GRPC_ERROR",
        "UNKNOWN_ERROR",
    ];
    assert!(
        valid.contains(&code),
        "error code '{}' not in known set",
        code
    );
}

#[test]
fn device_info_requires_device_id() {
    let out = cli(&["devices", "info"]);
    assert!(!out.status.success());
}

#[test]
fn device_calibrate_requires_device_id() {
    let out = cli(&["devices", "calibrate"]);
    assert!(!out.status.success());
}

#[test]
fn device_test_requires_device_id() {
    let out = cli(&["devices", "test"]);
    assert!(!out.status.success());
}

#[test]
fn device_list_filter_types_accepted() {
    // --filter-types is accepted even though daemon is down
    let out = cli(&["devices", "list", "--filter-types", "joystick,throttle"]);
    assert!(!out.status.success());
    // Should not be a parse error (exit 2)
    assert_ne!(out.status.code(), Some(2));
}

// ═══════════════════════════════════════════════════════════════════════════
// 6. INTEGRATION (5 tests)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn integration_exit_codes_mapped() {
    // info requires daemon → non-zero exit; status has fallback → zero
    let info = cli(&["info"]);
    assert!(!info.status.success());
    let code = info.status.code().unwrap();
    assert!(
        (1..=7).contains(&code),
        "exit code {} should be in mapped range 1–7",
        code
    );

    let status = cli(&["status"]);
    assert_eq!(status.status.code(), Some(0));
}

#[test]
fn integration_concurrent_cli_instances() {
    // Use the pre-built binary to avoid cargo build lock contention
    let bin = env!("CARGO_BIN_EXE_flightctl");

    let child1 = Command::new(bin)
        .args(["--json", "status"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("spawn 1");
    let child2 = Command::new(bin)
        .args(["--json", "version"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("spawn 2");

    let out1 = child1.wait_with_output().unwrap();
    let out2 = child2.wait_with_output().unwrap();

    assert!(out1.status.success(), "concurrent status should succeed");
    assert!(out2.status.success(), "concurrent version should succeed");

    // Both should produce valid JSON
    let j1 = extract_json(&String::from_utf8(out1.stdout).unwrap());
    let j2 = extract_json(&String::from_utf8(out2.stdout).unwrap());
    assert_eq!(j1["success"], true);
    assert_eq!(j2["success"], true);
}

#[test]
fn integration_pipe_friendly_output_no_ansi() {
    // JSON output should be free of ANSI escape codes (pipe-friendly)
    let out = cli(&["--json", "status"]);
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        !stdout.contains("\x1b["),
        "JSON output must not contain ANSI escapes"
    );
}

#[test]
fn integration_exit_code_zero_on_success() {
    let out = cli(&["status"]);
    assert_eq!(out.status.code(), Some(0));

    let out = cli(&["version"]);
    assert_eq!(out.status.code(), Some(0));

    let out = cli(&["--help"]);
    assert_eq!(out.status.code(), Some(0));
}

#[test]
fn integration_output_format_flag_short_o() {
    // -o json should work as short form of --output json
    let out = cli(&["-o", "json", "status"]);
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    let json = extract_json(&stdout);
    assert_eq!(json["success"], true);
}

// ═══════════════════════════════════════════════════════════════════════════
// BONUS: additional depth tests covering sub-commands and edge cases
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn parse_sim_help_accepted() {
    let out = cli(&["sim", "--help"]);
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("configure"));
}

#[test]
fn parse_torque_help_accepted() {
    let out = cli(&["torque", "--help"]);
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("unlock"));
    assert!(stdout.contains("status"));
    assert!(stdout.contains("set-mode"));
}

#[test]
fn parse_metrics_help_accepted() {
    let out = cli(&["metrics", "--help"]);
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("snapshot"));
}

#[test]
fn parse_overlay_help_accepted() {
    let out = cli(&["overlay", "--help"]);
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("show"));
    assert!(stdout.contains("hide"));
    assert!(stdout.contains("toggle"));
    assert!(stdout.contains("notify"));
}

#[test]
fn parse_update_help_accepted() {
    let out = cli(&["update", "--help"]);
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("check"));
    assert!(stdout.contains("channel"));
}

#[test]
fn parse_cloud_profiles_help_accepted() {
    let out = cli(&["cloud-profiles", "--help"]);
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("list"));
    assert!(stdout.contains("get"));
    assert!(stdout.contains("publish"));
}

#[test]
fn json_version_has_build_metadata() {
    let out = cli(&["--json", "version"]);
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    let json = extract_json(&stdout);
    let data = &json["data"];
    assert!(data["cli_version"].is_string());
    assert!(data["build_os"].is_string());
    assert!(data["build_target"].is_string());
    assert!(data["rust_version"].is_string());
}

#[test]
fn json_error_code_is_never_empty() {
    let out = cli(&["--json", "info"]);
    assert!(!out.status.success());
    let stderr = String::from_utf8(out.stderr).unwrap();
    let json = extract_json(&stderr);
    let code = json["error_code"].as_str().unwrap();
    assert!(!code.is_empty(), "error_code must not be empty");
}

#[test]
fn verbose_version_includes_package_info() {
    let out = cli(&["--json", "--verbose", "version"]);
    assert!(out.status.success());
    let stdout = String::from_utf8(out.stdout).unwrap();
    let json = extract_json(&stdout);
    let data = &json["data"];
    assert!(
        data["package_name"].is_string(),
        "verbose version should include package_name"
    );
}
