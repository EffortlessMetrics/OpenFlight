//! Integration tests for Flight Hub CLI

use std::process::Command;

fn run_cli_command(args: &[&str]) -> std::process::Output {
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

#[test]
fn test_cli_help() {
    let output = run_cli_command(&["--help"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Flight Hub command line interface"));
    assert!(stdout.contains("devices"));
    assert!(stdout.contains("profile"));
    assert!(stdout.contains("sim"));
    assert!(stdout.contains("panels"));
    assert!(stdout.contains("torque"));
    assert!(stdout.contains("diag"));
    assert!(stdout.contains("ac7"));
    assert!(stdout.contains("status"));
    assert!(stdout.contains("info"));
}

#[test]
fn test_devices_help() {
    let output = run_cli_command(&["devices", "--help"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Device management commands"));
    assert!(stdout.contains("list"));
    assert!(stdout.contains("info"));
    assert!(stdout.contains("dump"));
    assert!(stdout.contains("calibrate"));
    assert!(stdout.contains("test"));
}

#[test]
fn test_devices_list_help() {
    let output = run_cli_command(&["devices", "list", "--help"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("List all connected devices"));
    assert!(stdout.contains("--include-disconnected"));
    assert!(stdout.contains("--filter-types"));
}

#[test]
fn test_profile_help() {
    let output = run_cli_command(&["profile", "--help"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Profile management commands"));
    assert!(stdout.contains("list"));
    assert!(stdout.contains("apply"));
    assert!(stdout.contains("show"));
    assert!(stdout.contains("activate"));
    assert!(stdout.contains("validate"));
    assert!(stdout.contains("export"));
}

#[test]
fn test_sim_help() {
    let output = run_cli_command(&["sim", "--help"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Simulator configuration commands"));
    assert!(stdout.contains("configure"));
    assert!(stdout.contains("detect-conflicts"));
    assert!(stdout.contains("resolve-conflict"));
    assert!(stdout.contains("one-click-resolve"));
}

#[test]
fn test_panels_help() {
    let output = run_cli_command(&["panels", "--help"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Panel management commands"));
    assert!(stdout.contains("verify"));
    assert!(stdout.contains("status"));
}

#[test]
fn test_torque_help() {
    let output = run_cli_command(&["torque", "--help"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Force feedback and torque commands"));
    assert!(stdout.contains("unlock"));
    assert!(stdout.contains("status"));
    assert!(stdout.contains("set-mode"));
}

#[test]
fn test_diag_help() {
    let output = run_cli_command(&["diag", "--help"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Diagnostics and recording commands"));
    assert!(stdout.contains("bundle"));
    assert!(stdout.contains("health"));
    assert!(stdout.contains("metrics"));
    assert!(stdout.contains("trace"));
    assert!(stdout.contains("record"));
    assert!(stdout.contains("replay"));
    assert!(stdout.contains("status"));
    assert!(stdout.contains("stop"));
}

#[test]
fn test_ac7_help() {
    let output = run_cli_command(&["ac7", "--help"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Ace Combat 7 integration commands"));
    assert!(stdout.contains("status"));
    assert!(stdout.contains("render-input"));
    assert!(stdout.contains("install-input"));
}

#[test]
fn test_json_output_format() {
    // Test that JSON output format is properly handled when service is not available
    let output = run_cli_command(&["--output", "json", "info"]);

    // Should fail with connection error but return proper JSON
    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(1));

    let stderr = String::from_utf8(output.stderr).unwrap();

    // Should be valid JSON
    // Find the line that looks like JSON (starts with {)
    let json_line = stderr
        .lines()
        .find(|l| l.trim().starts_with('{'))
        .unwrap_or_else(|| panic!("No JSON output found in stderr: {}", stderr));

    let json_result: Result<serde_json::Value, _> = serde_json::from_str(json_line);
    assert!(
        json_result.is_ok(),
        "Output should be valid JSON: {}",
        json_line
    );

    let json = json_result.unwrap();
    assert_eq!(json["success"], false);
    assert!(json["error"].is_string());
    assert!(json["error_code"].is_string());
}

#[test]
fn test_human_output_format() {
    // Test that human output format is properly handled when service is not available
    let output = run_cli_command(&["--output", "human", "info"]);

    // Should fail with connection error
    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(1));

    let stderr = String::from_utf8(output.stderr).unwrap();

    // Find the error line
    let error_line_exists = stderr.lines().any(|l| l.starts_with("Error:"));
    assert!(
        error_line_exists,
        "Stderr should contain 'Error:': {}",
        stderr
    );
}

#[test]
fn test_version_flag() {
    let output = run_cli_command(&["--version"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("0.1.0")); // Should match the version in Cargo.toml
}

#[test]
fn test_invalid_command() {
    let output = run_cli_command(&["invalid-command"]);

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("error:") || stderr.contains("unrecognized subcommand"));
}

#[test]
fn test_timeout_option() {
    let output = run_cli_command(&["--timeout", "1000", "--output", "json", "info"]);

    // Should still fail with connection error but should accept the timeout option
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();

    let json_line = stderr
        .lines()
        .find(|l| l.trim().starts_with('{'))
        .unwrap_or_else(|| panic!("No JSON output found in stderr: {}", stderr));

    let json_result: Result<serde_json::Value, _> = serde_json::from_str(json_line);
    assert!(json_result.is_ok());
}

#[test]
fn test_verbose_flag() {
    let output = run_cli_command(&["--verbose", "--output", "json", "info"]);

    // Should still fail with connection error but should accept the verbose flag
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();

    let json_line = stderr
        .lines()
        .find(|l| l.trim().starts_with('{'))
        .unwrap_or_else(|| panic!("No JSON output found in stderr: {}", stderr));

    let json_result: Result<serde_json::Value, _> = serde_json::from_str(json_line);
    assert!(json_result.is_ok());
}

#[test]
fn test_version_command() {
    let output = run_cli_command(&["--version"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let version_str = stdout.trim();

    // Verify the output contains a semver X.Y.Z pattern
    let has_semver = version_str.split_whitespace().any(|word| {
        let parts: Vec<&str> = word.split('.').collect();
        parts.len() == 3 && parts.iter().all(|p| p.parse::<u32>().is_ok())
    });
    assert!(
        has_semver,
        "Version output should contain semver X.Y.Z pattern: {}",
        version_str
    );
}

#[test]
fn test_status_command_no_daemon() {
    let output = run_cli_command(&["status"]);

    // Status should succeed even when daemon is not running (reports offline status)
    assert!(
        output.status.success(),
        "status should succeed with offline fallback"
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("unreachable"),
        "status should report service as unreachable: {}",
        stdout
    );
}

#[test]
fn test_devices_list_no_daemon() {
    let output = run_cli_command(&["devices", "list"]);

    // Should fail gracefully when daemon is not running — not panic
    assert!(
        !output.status.success(),
        "devices list should fail when daemon is not running"
    );
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(!stderr.is_empty(), "stderr should contain an error message");
    assert_ne!(
        output.status.code(),
        Some(101),
        "devices list should not panic: {}",
        stderr
    );
}

#[test]
fn test_profile_list_no_daemon() {
    // "profile list" is now a recognized subcommand
    let output = run_cli_command(&["profile", "list"]);

    // list is local (reads profile dir), should succeed
    assert!(
        output.status.success(),
        "profile list should succeed even without daemon"
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(!stdout.is_empty(), "profile list produced output");
}

#[test]
fn test_json_flag_help() {
    // --output json combined with --help should still show help normally
    let output = run_cli_command(&["--output", "json", "--help"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Flight Hub command line interface"));
}

#[test]
fn test_info_command() {
    // info requires daemon; verify it fails gracefully with a human-readable error
    let output = run_cli_command(&["info"]);

    assert!(
        !output.status.success(),
        "info should fail when daemon is not running"
    );
    let stderr = String::from_utf8(output.stderr).unwrap();
    // Human-format error should contain "Error:" prefix
    assert!(
        stderr.lines().any(|l| l.starts_with("Error:")),
        "stderr should contain an 'Error:' line: {}",
        stderr
    );
    assert_ne!(
        output.status.code(),
        Some(101),
        "info should not panic: {}",
        stderr
    );
}

#[test]
fn test_diag_command_help() {
    // Test that the record subcommand help shows the expected flags
    let output = run_cli_command(&["diag", "record", "--help"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Start recording diagnostics"));
    assert!(stdout.contains("--output"));
    assert!(stdout.contains("--duration"));
}

#[test]
fn test_status_json_error_format() {
    // Verify that status with JSON output returns well-formed JSON with offline status
    let output = run_cli_command(&["--output", "json", "status"]);

    assert!(
        output.status.success(),
        "status should succeed with offline fallback"
    );
    let stdout = String::from_utf8(output.stdout).unwrap();

    let json_line = stdout
        .lines()
        .find(|l| l.trim().starts_with('{'))
        .unwrap_or_else(|| panic!("No JSON output found in stdout: {}", stdout));

    let json: serde_json::Value = serde_json::from_str(json_line)
        .unwrap_or_else(|e| panic!("Invalid JSON in stdout: {} — {}", e, json_line));

    assert_eq!(json["success"], true);
    assert_eq!(json["data"]["service_status"], "unreachable");
    assert!(
        json["data"]["cli_version"].is_string(),
        "JSON should contain cli_version"
    );
}

#[test]
fn test_devices_list_json_error_format() {
    // Verify that devices list with JSON output returns well-formed JSON when daemon is unavailable
    let output = run_cli_command(&["--output", "json", "devices", "list"]);

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();

    let json_line = stderr
        .lines()
        .find(|l| l.trim().starts_with('{'))
        .unwrap_or_else(|| panic!("No JSON output found in stderr: {}", stderr));

    let json: serde_json::Value = serde_json::from_str(json_line)
        .unwrap_or_else(|e| panic!("Invalid JSON in stderr: {} — {}", e, json_line));

    assert_eq!(json["success"], false);
    assert!(
        json["error"].is_string(),
        "JSON error field should be a string"
    );
    assert!(
        json["error_code"].is_string(),
        "JSON error_code field should be a string"
    );
}

#[test]
fn test_json_flag_shorthand() {
    // --json flag should work as shorthand for --output json
    let output = run_cli_command(&["--json", "status"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();

    let json_line = stdout
        .lines()
        .find(|l| l.trim().starts_with('{'))
        .unwrap_or_else(|| panic!("No JSON output found in stdout: {}", stdout));

    let json: serde_json::Value = serde_json::from_str(json_line)
        .unwrap_or_else(|e| panic!("Invalid JSON: {} — {}", e, json_line));

    assert_eq!(json["success"], true);
    assert_eq!(json["data"]["service_status"], "unreachable");
}

#[test]
fn test_adapters_help() {
    let output = run_cli_command(&["adapters", "--help"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Simulator adapter management"));
    assert!(stdout.contains("status"));
    assert!(stdout.contains("enable"));
    assert!(stdout.contains("disable"));
    assert!(stdout.contains("reconnect"));
}

#[test]
fn test_adapters_status_no_daemon() {
    let output = run_cli_command(&["adapters", "status"]);

    // Should fail gracefully when daemon is not running
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(!stderr.is_empty());
    assert_ne!(
        output.status.code(),
        Some(101),
        "adapters status should not panic: {}",
        stderr
    );
}

#[test]
fn test_adapters_enable_no_daemon() {
    let output = run_cli_command(&["adapters", "enable", "msfs"]);

    assert!(!output.status.success());
    assert_ne!(output.status.code(), Some(101));
}

#[test]
fn test_adapters_reconnect_no_daemon() {
    let output = run_cli_command(&["adapters", "reconnect", "xplane"]);

    assert!(!output.status.success());
    assert_ne!(output.status.code(), Some(101));
}

#[test]
fn test_devices_calibrate_no_daemon() {
    let output = run_cli_command(&["devices", "calibrate", "test-device"]);

    assert!(!output.status.success());
    assert_ne!(output.status.code(), Some(101));
}

#[test]
fn test_devices_test_no_daemon() {
    let output = run_cli_command(&["devices", "test", "test-device"]);

    assert!(!output.status.success());
    assert_ne!(output.status.code(), Some(101));
}

#[test]
fn test_diag_bundle_no_daemon() {
    // bundle should work even without daemon (collects local info)
    let output = run_cli_command(&["--json", "diag", "bundle"]);

    // Bundle may succeed since it handles daemon being offline
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();

    // Should not panic
    assert_ne!(
        output.status.code(),
        Some(101),
        "diag bundle should not panic: stdout={}, stderr={}",
        stdout,
        stderr
    );
}

#[test]
fn test_diag_health_no_daemon() {
    let output = run_cli_command(&["diag", "health"]);

    assert!(!output.status.success());
    assert_ne!(output.status.code(), Some(101));
}

#[test]
fn test_diag_trace_help() {
    let output = run_cli_command(&["diag", "trace", "--help"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Record a trace"));
}

#[test]
fn test_profile_validate_help() {
    let output = run_cli_command(&["profile", "validate", "--help"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Validate a profile file"));
}

#[test]
fn test_profile_export_help() {
    let output = run_cli_command(&["profile", "export", "--help"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Export a profile"));
}

#[test]
fn test_cli_help_includes_adapters() {
    let output = run_cli_command(&["--help"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("adapters"));
}

// ── Version subcommand tests ──────────────────────────────────────────────

#[test]
fn test_version_subcommand() {
    let output = run_cli_command(&["version"]);

    assert!(output.status.success(), "version subcommand should succeed");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("cli_version") || stdout.contains("0.1.0"),
        "version output should contain version info: {}",
        stdout
    );
}

#[test]
fn test_version_subcommand_json() {
    let output = run_cli_command(&["--json", "version"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();

    let json_line = stdout
        .lines()
        .find(|l| l.trim().starts_with('{'))
        .unwrap_or_else(|| panic!("No JSON in stdout: {}", stdout));

    let json: serde_json::Value = serde_json::from_str(json_line)
        .unwrap_or_else(|e| panic!("Invalid JSON: {} — {}", e, json_line));

    assert_eq!(json["success"], true);
    assert!(json["data"]["cli_version"].is_string());
    assert!(json["data"]["build_profile"].is_string());
    assert!(json["data"]["build_target"].is_string());
    assert!(json["data"]["build_os"].is_string());
    assert!(json["data"]["rust_version"].is_string());
    // Service should be unreachable in test environment
    assert_eq!(json["data"]["service_status"], "unreachable");
}

#[test]
fn test_version_subcommand_verbose() {
    let output = run_cli_command(&["--verbose", "--json", "version"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();

    let json_line = stdout
        .lines()
        .find(|l| l.trim().starts_with('{'))
        .unwrap_or_else(|| panic!("No JSON in stdout: {}", stdout));

    let json: serde_json::Value = serde_json::from_str(json_line).unwrap();
    assert_eq!(json["success"], true);
    assert!(json["data"]["package_name"].is_string());
}

// ── Safe-mode subcommand tests ────────────────────────────────────────────

#[test]
fn test_safe_mode_no_daemon() {
    let output = run_cli_command(&["safe-mode"]);

    // Should fail gracefully when daemon is not running
    assert!(!output.status.success());
    assert_ne!(
        output.status.code(),
        Some(101),
        "safe-mode should not panic"
    );
}

#[test]
fn test_safe_mode_json_error() {
    let output = run_cli_command(&["--json", "safe-mode"]);

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();

    let json_line = stderr
        .lines()
        .find(|l| l.trim().starts_with('{'))
        .unwrap_or_else(|| panic!("No JSON in stderr: {}", stderr));

    let json: serde_json::Value = serde_json::from_str(json_line).unwrap();
    assert_eq!(json["success"], false);
    assert!(json["error"].is_string());
}

#[test]
fn test_safe_mode_help() {
    let output = run_cli_command(&["safe-mode", "--help"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("safe mode") || stdout.contains("Safe mode") || stdout.contains("FFB"),
        "safe-mode help should describe the command: {}",
        stdout
    );
}

// ── Diagnostics subcommand tests ──────────────────────────────────────────

#[test]
fn test_diagnostics_no_daemon() {
    let output = run_cli_command(&["diagnostics"]);

    // Shorthand for diag health, requires daemon
    assert!(!output.status.success());
    assert_ne!(
        output.status.code(),
        Some(101),
        "diagnostics should not panic"
    );
}

#[test]
fn test_diagnostics_json_error() {
    let output = run_cli_command(&["--json", "diagnostics"]);

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();

    let json_line = stderr
        .lines()
        .find(|l| l.trim().starts_with('{'))
        .unwrap_or_else(|| panic!("No JSON in stderr: {}", stderr));

    let json: serde_json::Value = serde_json::from_str(json_line).unwrap();
    assert_eq!(json["success"], false);
    assert!(json["error_code"].is_string());
}

#[test]
fn test_diagnostics_help() {
    let output = run_cli_command(&["diagnostics", "--help"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("diagnostic") || stdout.contains("diag"));
}

// ── CLI help includes new commands ────────────────────────────────────────

#[test]
fn test_cli_help_includes_version() {
    let output = run_cli_command(&["--help"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("version"),
        "help should list version command: {}",
        stdout
    );
}

#[test]
fn test_cli_help_includes_safe_mode() {
    let output = run_cli_command(&["--help"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("safe-mode"),
        "help should list safe-mode command: {}",
        stdout
    );
}

#[test]
fn test_cli_help_includes_diagnostics() {
    let output = run_cli_command(&["--help"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("diagnostics"),
        "help should list diagnostics command: {}",
        stdout
    );
}

// ── Error handling for bad inputs ─────────────────────────────────────────

#[test]
fn test_invalid_output_format() {
    let output = run_cli_command(&["--output", "xml", "status"]);

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("invalid value") || stderr.contains("error"),
        "should report invalid output format: {}",
        stderr
    );
}

#[test]
fn test_missing_required_arg_devices_info() {
    let output = run_cli_command(&["devices", "info"]);

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("required") || stderr.contains("error"),
        "should report missing required argument: {}",
        stderr
    );
}

#[test]
fn test_missing_required_arg_profile_activate() {
    let output = run_cli_command(&["profile", "activate"]);

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("required") || stderr.contains("error"),
        "should report missing required argument: {}",
        stderr
    );
}

#[test]
fn test_missing_required_arg_profile_apply() {
    let output = run_cli_command(&["profile", "apply"]);

    assert!(!output.status.success());
}

#[test]
fn test_invalid_timeout_value() {
    let output = run_cli_command(&["--timeout", "not-a-number", "status"]);

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("invalid value") || stderr.contains("error"),
        "should report invalid timeout: {}",
        stderr
    );
}

#[test]
fn test_extra_args_rejected() {
    let output = run_cli_command(&["status", "--nonexistent-flag"]);

    assert!(!output.status.success());
}

// ── JSON output stability tests ───────────────────────────────────────────

#[test]
fn test_status_json_has_stable_fields() {
    let output = run_cli_command(&["--json", "status"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();

    let json_line = stdout.lines().find(|l| l.trim().starts_with('{')).unwrap();

    let json: serde_json::Value = serde_json::from_str(json_line).unwrap();

    // Verify stable JSON contract fields
    assert!(json.get("success").is_some(), "must have 'success' field");
    assert!(json.get("data").is_some(), "must have 'data' field");

    let data = &json["data"];
    assert!(
        data.get("service_status").is_some(),
        "data must have 'service_status'"
    );
    assert!(
        data.get("cli_version").is_some(),
        "data must have 'cli_version'"
    );
}

#[test]
fn test_json_error_has_stable_fields() {
    let output = run_cli_command(&["--json", "info"]);

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();

    let json_line = stderr.lines().find(|l| l.trim().starts_with('{')).unwrap();
    let json: serde_json::Value = serde_json::from_str(json_line).unwrap();

    // Verify stable error JSON contract
    assert_eq!(json["success"], false);
    assert!(
        json.get("error").is_some(),
        "error response must have 'error' field"
    );
    assert!(
        json.get("error_code").is_some(),
        "error response must have 'error_code' field"
    );
}

#[test]
fn test_profile_list_json_has_stable_fields() {
    let output = run_cli_command(&["--json", "profile", "list"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();

    let json_line = stdout.lines().find(|l| l.trim().starts_with('{')).unwrap();

    let json: serde_json::Value = serde_json::from_str(json_line).unwrap();
    assert_eq!(json["success"], true);
    assert!(json["data"].is_array(), "profile list data should be array");
    assert!(
        json.get("total_count").is_some(),
        "profile list must have total_count"
    );
}

#[test]
fn test_version_json_has_stable_fields() {
    let output = run_cli_command(&["--json", "version"]);

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();

    let json_line = stdout.lines().find(|l| l.trim().starts_with('{')).unwrap();

    let json: serde_json::Value = serde_json::from_str(json_line).unwrap();
    assert_eq!(json["success"], true);

    let data = &json["data"];
    assert!(data["cli_version"].is_string());
    assert!(data["build_profile"].is_string());
    assert!(data["build_target"].is_string());
    assert!(data["build_os"].is_string());
    assert!(data["rust_version"].is_string());
    // service_status may be null or string
    assert!(data.get("service_status").is_some());
}

// ── Mock service connectivity test ────────────────────────────────────────

#[test]
fn test_connection_error_has_correct_exit_code() {
    // When the service is unreachable, commands that require it should exit non-zero
    let output = run_cli_command(&["info"]);
    assert!(!output.status.success());
    // Exit code 1 = generic error (service unreachable wraps as UNKNOWN_ERROR)
    let code = output.status.code().unwrap();
    assert!(
        (1..=7).contains(&code),
        "exit code should be in the mapped error range: {}",
        code
    );
}

#[test]
fn test_connection_error_json_has_error_code() {
    let output = run_cli_command(&["--json", "devices", "list"]);

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();

    let json_line = stderr.lines().find(|l| l.trim().starts_with('{')).unwrap();
    let json: serde_json::Value = serde_json::from_str(json_line).unwrap();

    let error_code = json["error_code"].as_str().unwrap();
    // Should be one of the defined error codes
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
