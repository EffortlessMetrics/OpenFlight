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
    assert!(stdout.contains("apply"));
    assert!(stdout.contains("show"));
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

    // Should fail gracefully when daemon is not running — not panic
    assert!(
        !output.status.success(),
        "status should fail when daemon is not running"
    );
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(!stderr.is_empty(), "stderr should contain an error message");
    // Exit code 101 would indicate a Rust panic
    assert_ne!(
        output.status.code(),
        Some(101),
        "status should not panic: {}",
        stderr
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
    // "profile list" is not a recognized subcommand; verify clap rejects it gracefully (no panic)
    let output = run_cli_command(&["profile", "list"]);

    assert!(
        !output.status.success(),
        "unrecognized profile subcommand should fail"
    );
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(!stderr.is_empty(), "stderr should contain an error message");
    // Exit code 101 would indicate a Rust panic
    assert_ne!(
        output.status.code(),
        Some(101),
        "profile list should not panic: {}",
        stderr
    );
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
    // Verify that status with JSON output returns well-formed JSON when daemon is unavailable
    let output = run_cli_command(&["--output", "json", "status"]);

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
