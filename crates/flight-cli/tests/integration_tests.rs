//! Integration tests for Flight Hub CLI

use std::process::Command;

fn run_cli_command(args: &[&str]) -> std::process::Output {
    let mut cmd = Command::new("cargo");
    cmd.arg("run")
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
fn test_json_output_format() {
    // Test that JSON output format is properly handled when service is not available
    let output = run_cli_command(&["--output", "json", "info"]);
    
    // Should fail with connection error but return proper JSON
    assert!(!output.status.success());
    assert_eq!(output.status.code(), Some(1));
    
    let stderr = String::from_utf8(output.stderr).unwrap();
    
    // Should be valid JSON
    let json_result: Result<serde_json::Value, _> = serde_json::from_str(&stderr);
    assert!(json_result.is_ok(), "Output should be valid JSON: {}", stderr);
    
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
    assert!(stderr.starts_with("Error:"));
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
    let json_result: Result<serde_json::Value, _> = serde_json::from_str(&stderr);
    assert!(json_result.is_ok());
}

#[test]
fn test_verbose_flag() {
    let output = run_cli_command(&["--verbose", "--output", "json", "info"]);
    
    // Should still fail with connection error but should accept the verbose flag
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    let json_result: Result<serde_json::Value, _> = serde_json::from_str(&stderr);
    assert!(json_result.is_ok());
}