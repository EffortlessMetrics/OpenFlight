// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for CLI (`flightctl`) — ensures CLI parity with service endpoints.
//!
//! These tests exercise command parsing, output formatting, error handling,
//! and JSON schema stability without requiring a running daemon.

use std::process::Command;

// ── Helpers ───────────────────────────────────────────────────────────────

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

fn stdout(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn stderr(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

/// Extract the first JSON object from a string (stdout or stderr).
fn extract_json(text: &str) -> serde_json::Value {
    let line = text
        .lines()
        .find(|l| l.trim().starts_with('{'))
        .unwrap_or_else(|| panic!("No JSON found in: {}", text));
    serde_json::from_str(line).unwrap_or_else(|e| panic!("Invalid JSON: {} — {}", e, line))
}

// ════════════════════════════════════════════════════════════════════════════
// 1. Command parsing tests
// ════════════════════════════════════════════════════════════════════════════

mod command_parsing {
    use super::*;

    #[test]
    fn status_parses_correctly() {
        let out = cli(&["status"]);
        assert!(out.status.success(), "status should parse and succeed");
    }

    #[test]
    fn devices_list_parses_correctly() {
        // Parses, but fails because daemon is not running
        let out = cli(&["devices", "list"]);
        // Must not be a parse error (exit 2 = clap error)
        assert_ne!(out.status.code(), Some(2), "devices list should parse");
    }

    #[test]
    fn profile_load_via_apply_parses_correctly() {
        // `profile apply <path>` — clap should accept the subcommand even if file missing
        let out = cli(&["profile", "apply", "nonexistent.json"]);
        // Should fail with file-not-found, not a parse error
        let err = stderr(&out);
        assert!(
            err.contains("Failed to read") || err.contains("Error"),
            "should get file error, not parse error: {}",
            err
        );
    }

    #[test]
    fn profile_validate_parses_correctly() {
        let out = cli(&["profile", "validate", "nonexistent.json"]);
        let err = stderr(&out);
        assert!(
            err.contains("Failed to read") || err.contains("Error"),
            "should get file error: {}",
            err
        );
    }

    #[test]
    fn json_flag_with_status_parses_correctly() {
        let out = cli(&["--json", "status"]);
        assert!(out.status.success());
        let json = extract_json(&stdout(&out));
        assert_eq!(json["success"], true);
    }

    #[test]
    fn unknown_command_gives_helpful_error() {
        let out = cli(&["frobnicate"]);
        assert!(!out.status.success());
        let err = stderr(&out);
        assert!(
            err.contains("unrecognized subcommand")
                || err.contains("error:")
                || err.contains("invalid"),
            "should contain helpful error: {}",
            err
        );
    }

    #[test]
    fn missing_argument_gives_helpful_error() {
        let out = cli(&["devices", "info"]);
        assert!(!out.status.success());
        let err = stderr(&out);
        assert!(
            err.contains("required") || err.contains("error:") || err.contains("Usage"),
            "should contain helpful error about missing arg: {}",
            err
        );
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 2. JSON output tests
// ════════════════════════════════════════════════════════════════════════════

mod json_output {
    use super::*;

    #[test]
    fn status_json_is_valid() {
        let out = cli(&["--json", "status"]);
        assert!(out.status.success());
        let json = extract_json(&stdout(&out));
        assert!(json.is_object());
    }

    #[test]
    fn version_json_is_valid() {
        let out = cli(&["--json", "version"]);
        assert!(out.status.success());
        let json = extract_json(&stdout(&out));
        assert!(json.is_object());
        assert_eq!(json["success"], true);
    }

    #[test]
    fn profile_list_json_is_valid() {
        let out = cli(&["--json", "profile", "list"]);
        assert!(out.status.success());
        let json = extract_json(&stdout(&out));
        assert_eq!(json["success"], true);
        assert!(json["data"].is_array());
    }

    #[test]
    fn json_schema_status_is_stable() {
        let out = cli(&["--json", "status"]);
        let json = extract_json(&stdout(&out));
        // Verify the contract — these fields MUST always exist
        assert!(json.get("success").is_some(), "must have 'success'");
        assert!(json.get("data").is_some(), "must have 'data'");
        let data = &json["data"];
        assert!(
            data.get("service_status").is_some(),
            "data.service_status must exist"
        );
        assert!(
            data.get("cli_version").is_some(),
            "data.cli_version must exist"
        );
    }

    #[test]
    fn json_schema_version_is_stable() {
        let out = cli(&["--json", "version"]);
        let json = extract_json(&stdout(&out));
        let data = &json["data"];
        for field in &[
            "cli_version",
            "build_profile",
            "build_target",
            "build_os",
            "rust_version",
            "service_status",
        ] {
            assert!(
                data.get(*field).is_some(),
                "version JSON must have field '{}'",
                field
            );
        }
    }

    #[test]
    fn error_response_in_json_format() {
        let out = cli(&["--json", "info"]);
        assert!(!out.status.success());
        let json = extract_json(&stderr(&out));
        assert_eq!(json["success"], false);
        assert!(json["error"].is_string(), "must have error string");
        assert!(json["error_code"].is_string(), "must have error_code");
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 3. Status command depth tests
// ════════════════════════════════════════════════════════════════════════════

mod status_command {
    use super::*;

    #[test]
    fn shows_service_health_when_offline() {
        let out = cli(&["--json", "status"]);
        assert!(out.status.success());
        let json = extract_json(&stdout(&out));
        assert_eq!(json["data"]["service_status"], "unreachable");
    }

    #[test]
    fn shows_cli_version() {
        let out = cli(&["--json", "status"]);
        let json = extract_json(&stdout(&out));
        let version = json["data"]["cli_version"].as_str().unwrap();
        assert!(
            !version.is_empty(),
            "cli_version should not be empty"
        );
        // Should be semver
        let parts: Vec<&str> = version.split('.').collect();
        assert_eq!(parts.len(), 3, "cli_version should be semver: {}", version);
    }

    #[test]
    fn shows_device_fields_when_offline() {
        let out = cli(&["--json", "status"]);
        let json = extract_json(&stdout(&out));
        let data = &json["data"];
        // When offline, device fields should be null
        assert!(
            data.get("connected_devices").is_some(),
            "must have connected_devices field"
        );
        assert!(
            data.get("total_devices").is_some(),
            "must have total_devices field"
        );
    }

    #[test]
    fn human_output_shows_unreachable() {
        let out = cli(&["status"]);
        assert!(out.status.success());
        let text = stdout(&out);
        assert!(
            text.contains("unreachable"),
            "human output should mention unreachable: {}",
            text
        );
    }

    #[test]
    fn verbose_flag_adds_detail() {
        let out = cli(&["--verbose", "--json", "status"]);
        assert!(out.status.success());
        let json = extract_json(&stdout(&out));
        let data = &json["data"];
        // Verbose adds connection_error when offline
        assert!(
            data.get("connection_error").is_some(),
            "verbose status should include connection_error"
        );
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 4. Device command depth tests
// ════════════════════════════════════════════════════════════════════════════

mod device_commands {
    use super::*;

    #[test]
    fn list_fails_gracefully_without_daemon() {
        let out = cli(&["devices", "list"]);
        assert!(!out.status.success());
        assert_ne!(
            out.status.code(),
            Some(101),
            "must not panic"
        );
    }

    #[test]
    fn list_json_error_has_structure() {
        let out = cli(&["--json", "devices", "list"]);
        assert!(!out.status.success());
        let json = extract_json(&stderr(&out));
        assert_eq!(json["success"], false);
        assert!(json["error"].is_string());
        assert!(json["error_code"].is_string());
    }

    #[test]
    fn info_requires_device_id() {
        let out = cli(&["devices", "info"]);
        assert!(!out.status.success());
        let err = stderr(&out);
        assert!(
            err.contains("required") || err.contains("error:"),
            "should report missing device_id: {}",
            err
        );
    }

    #[test]
    fn info_with_id_fails_without_daemon() {
        let out = cli(&["devices", "info", "test-dev-123"]);
        assert!(!out.status.success());
        assert_ne!(out.status.code(), Some(101), "must not panic");
    }

    #[test]
    fn calibrate_requires_device_id() {
        let out = cli(&["devices", "calibrate"]);
        assert!(!out.status.success());
        let err = stderr(&out);
        assert!(
            err.contains("required") || err.contains("error:"),
            "should report missing device_id: {}",
            err
        );
    }

    #[test]
    fn calibrate_with_id_fails_without_daemon() {
        let out = cli(&["devices", "calibrate", "dev-abc"]);
        assert!(!out.status.success());
        assert_ne!(out.status.code(), Some(101));
    }

    #[test]
    fn list_with_include_disconnected_parses() {
        let out = cli(&["devices", "list", "--include-disconnected"]);
        // Should parse — daemon error is expected
        assert_ne!(out.status.code(), Some(2));
    }

    #[test]
    fn list_with_filter_types_parses() {
        let out = cli(&["devices", "list", "--filter-types", "joystick,throttle"]);
        assert_ne!(out.status.code(), Some(2));
    }

    #[test]
    fn test_subcommand_parses_with_options() {
        let out = cli(&[
            "devices", "test", "dev-1", "--interval-ms", "50", "--count", "5",
        ]);
        // Should parse correctly — daemon error is expected
        assert_ne!(out.status.code(), Some(2));
        assert_ne!(out.status.code(), Some(101), "must not panic");
    }

    #[test]
    fn dump_requires_device_id() {
        let out = cli(&["devices", "dump"]);
        assert!(!out.status.success());
        let err = stderr(&out);
        assert!(err.contains("required") || err.contains("error:"));
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 5. Profile command depth tests
// ════════════════════════════════════════════════════════════════════════════

mod profile_commands {
    use super::*;

    #[test]
    fn list_succeeds_without_daemon() {
        let out = cli(&["profile", "list"]);
        assert!(
            out.status.success(),
            "profile list reads local dir, should succeed"
        );
    }

    #[test]
    fn list_json_has_array_data() {
        let out = cli(&["--json", "profile", "list"]);
        assert!(out.status.success());
        let json = extract_json(&stdout(&out));
        assert_eq!(json["success"], true);
        assert!(json["data"].is_array());
        assert!(json.get("total_count").is_some());
    }

    #[test]
    fn list_with_include_builtin_adds_default() {
        let out = cli(&["--json", "profile", "list", "--include-builtin"]);
        assert!(out.status.success());
        let json = extract_json(&stdout(&out));
        let profiles = json["data"].as_array().unwrap();
        let has_builtin = profiles
            .iter()
            .any(|p| p["source"] == "builtin" && p["name"] == "default");
        assert!(has_builtin, "should include builtin default profile");
    }

    #[test]
    fn validate_with_nonexistent_file_gives_error() {
        let out = cli(&["profile", "validate", "does_not_exist.json"]);
        assert!(!out.status.success());
        let err = stderr(&out);
        assert!(
            err.contains("Failed to read") || err.contains("Error"),
            "should report file not found: {}",
            err
        );
    }

    #[test]
    fn apply_with_nonexistent_file_gives_error() {
        let out = cli(&["profile", "apply", "missing_profile.json"]);
        assert!(!out.status.success());
        let err = stderr(&out);
        assert!(
            err.contains("Failed to read") || err.contains("Error"),
            "should report file error: {}",
            err
        );
    }

    #[test]
    fn activate_requires_name() {
        let out = cli(&["profile", "activate"]);
        assert!(!out.status.success());
        let err = stderr(&out);
        assert!(
            err.contains("required") || err.contains("error:"),
            "should report missing name: {}",
            err
        );
    }

    #[test]
    fn activate_nonexistent_profile_gives_error() {
        let out = cli(&["profile", "activate", "nonexistent-profile-xyz"]);
        assert!(!out.status.success());
        let err = stderr(&out);
        assert!(
            err.contains("not found") || err.contains("Error"),
            "should report profile not found: {}",
            err
        );
    }

    #[test]
    fn export_requires_name_and_path() {
        let out = cli(&["profile", "export"]);
        assert!(!out.status.success());
        let err = stderr(&out);
        assert!(
            err.contains("required") || err.contains("error:"),
            "should report missing arguments: {}",
            err
        );
    }

    #[test]
    fn show_without_name_succeeds() {
        // `profile show` without a name should succeed (shows effective or fallback)
        let out = cli(&["profile", "show"]);
        assert!(
            out.status.success(),
            "profile show should succeed without name: stderr={}",
            stderr(&out)
        );
    }

    #[test]
    fn show_nonexistent_name_fails() {
        let out = cli(&["profile", "show", "nonexistent-xyz"]);
        assert!(!out.status.success());
        let err = stderr(&out);
        assert!(
            err.contains("not found") || err.contains("Error"),
            "should report profile not found: {}",
            err
        );
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 6. Service control command depth tests
// ════════════════════════════════════════════════════════════════════════════

mod service_control {
    use super::*;

    #[test]
    fn safe_mode_fails_gracefully_without_daemon() {
        let out = cli(&["safe-mode"]);
        assert!(!out.status.success());
        assert_ne!(out.status.code(), Some(101), "must not panic");
    }

    #[test]
    fn safe_mode_json_error() {
        let out = cli(&["--json", "safe-mode"]);
        assert!(!out.status.success());
        let json = extract_json(&stderr(&out));
        assert_eq!(json["success"], false);
        assert!(json["error"].is_string());
    }

    #[test]
    fn diagnostics_shorthand_for_diag_health() {
        // Both `diagnostics` and `diag health` should behave the same
        let out_diag = cli(&["diagnostics"]);
        let out_health = cli(&["diag", "health"]);
        assert_eq!(
            out_diag.status.code(),
            out_health.status.code(),
            "diagnostics and diag health should have same exit code"
        );
    }

    #[test]
    fn diag_health_fails_without_daemon() {
        let out = cli(&["diag", "health"]);
        assert!(!out.status.success());
        assert_ne!(out.status.code(), Some(101));
    }

    #[test]
    fn diag_bundle_does_not_panic() {
        let out = cli(&["diag", "bundle"]);
        assert_ne!(
            out.status.code(),
            Some(101),
            "diag bundle should not panic: {}",
            stderr(&out)
        );
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 7. Output format tests
// ════════════════════════════════════════════════════════════════════════════

mod output_format {
    use super::*;

    #[test]
    fn human_is_default_format() {
        let out = cli(&["status"]);
        assert!(out.status.success());
        let text = stdout(&out);
        // Human format uses "key: value" lines, not JSON
        assert!(
            !text.trim().starts_with('{'),
            "default format should be human, not JSON: {}",
            text
        );
    }

    #[test]
    fn output_json_flag_produces_json() {
        let out = cli(&["--output", "json", "status"]);
        assert!(out.status.success());
        let json = extract_json(&stdout(&out));
        assert!(json.is_object());
    }

    #[test]
    fn json_shorthand_flag_produces_json() {
        let out = cli(&["--json", "status"]);
        assert!(out.status.success());
        let json = extract_json(&stdout(&out));
        assert_eq!(json["success"], true);
    }

    #[test]
    fn invalid_output_format_rejected() {
        let out = cli(&["--output", "xml", "status"]);
        assert!(!out.status.success());
        let err = stderr(&out);
        assert!(
            err.contains("invalid value") || err.contains("error"),
            "should reject invalid format: {}",
            err
        );
    }

    #[test]
    fn json_flag_overrides_output_human() {
        // When both --output human and --json are specified, --json wins
        let out = cli(&["--output", "human", "--json", "status"]);
        assert!(out.status.success());
        let text = stdout(&out);
        // Should be JSON
        assert!(
            text.trim().starts_with('{') || text.lines().any(|l| l.trim().starts_with('{')),
            "--json should override --output human: {}",
            text
        );
    }

    #[test]
    fn human_error_starts_with_error_prefix() {
        let out = cli(&["info"]);
        assert!(!out.status.success());
        let err = stderr(&out);
        assert!(
            err.lines().any(|l| l.starts_with("Error:")),
            "human error should start with 'Error:': {}",
            err
        );
    }

    #[test]
    fn json_error_contains_success_false() {
        let out = cli(&["--json", "info"]);
        assert!(!out.status.success());
        let json = extract_json(&stderr(&out));
        assert_eq!(json["success"], false);
    }

    #[test]
    fn version_human_contains_version_number() {
        let out = cli(&["version"]);
        assert!(out.status.success());
        let text = stdout(&out);
        assert!(
            text.contains("cli_version") || text.contains("0."),
            "human version output should contain version: {}",
            text
        );
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 8. Property tests — universal invariants
// ════════════════════════════════════════════════════════════════════════════

mod property_tests {
    use super::*;

    /// Every command should return a valid exit code (0-7), never 101 (panic).
    const COMMANDS: &[&[&str]] = &[
        &["status"],
        &["version"],
        &["info"],
        &["safe-mode"],
        &["diagnostics"],
        &["devices", "list"],
        &["profile", "list"],
        &["diag", "health"],
        &["diag", "bundle"],
    ];

    #[test]
    fn all_commands_return_valid_exit_codes() {
        for args in COMMANDS {
            let out = cli(args);
            let code = out.status.code().unwrap_or(-1);
            assert!(
                (0..=7).contains(&code),
                "command {:?} returned unexpected exit code {}",
                args,
                code
            );
        }
    }

    #[test]
    fn all_commands_never_panic() {
        for args in COMMANDS {
            let out = cli(args);
            assert_ne!(
                out.status.code(),
                Some(101),
                "command {:?} panicked",
                args
            );
        }
    }

    /// --help works for all top-level subcommands.
    const SUBCOMMANDS: &[&[&str]] = &[
        &["--help"],
        &["status", "--help"],
        &["version", "--help"],
        &["info", "--help"],
        &["safe-mode", "--help"],
        &["diagnostics", "--help"],
        &["devices", "--help"],
        &["profile", "--help"],
        &["sim", "--help"],
        &["panels", "--help"],
        &["torque", "--help"],
        &["diag", "--help"],
        &["metrics", "--help"],
        &["dcs", "--help"],
        &["xplane", "--help"],
        &["ac7", "--help"],
        &["update", "--help"],
        &["cloud-profiles", "--help"],
        &["adapters", "--help"],
        &["overlay", "--help"],
    ];

    #[test]
    fn help_works_for_all_subcommands() {
        for args in SUBCOMMANDS {
            let out = cli(args);
            assert!(
                out.status.success(),
                "--help for {:?} should succeed: {}",
                args,
                stderr(&out)
            );
            let text = stdout(&out);
            assert!(
                !text.is_empty(),
                "--help for {:?} should produce output",
                args
            );
        }
    }

    #[test]
    fn version_flag_shows_version_info() {
        let out = cli(&["--version"]);
        assert!(out.status.success());
        let text = stdout(&out);
        // Should contain a semver X.Y.Z pattern
        let has_semver = text.split_whitespace().any(|w| {
            let parts: Vec<&str> = w.split('.').collect();
            parts.len() == 3 && parts.iter().all(|p| p.parse::<u32>().is_ok())
        });
        assert!(has_semver, "--version should show semver: {}", text);
    }

    /// JSON mode should always produce valid JSON for both success and error paths.
    #[test]
    fn json_mode_always_produces_valid_json() {
        let success_cmds: &[&[&str]] = &[
            &["--json", "status"],
            &["--json", "version"],
            &["--json", "profile", "list"],
        ];
        for args in success_cmds {
            let out = cli(args);
            assert!(out.status.success(), "{:?} should succeed", args);
            let text = stdout(&out);
            let _ = extract_json(&text); // panics if invalid JSON
        }

        let error_cmds: &[&[&str]] = &[
            &["--json", "info"],
            &["--json", "devices", "list"],
            &["--json", "safe-mode"],
            &["--json", "diagnostics"],
        ];
        for args in error_cmds {
            let out = cli(args);
            assert!(!out.status.success(), "{:?} should fail without daemon", args);
            let text = stderr(&out);
            let _ = extract_json(&text); // panics if invalid JSON
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 9. Error code mapping tests
// ════════════════════════════════════════════════════════════════════════════

mod error_codes {
    use super::*;

    const VALID_ERROR_CODES: &[&str] = &[
        "CONNECTION_FAILED",
        "VERSION_MISMATCH",
        "UNSUPPORTED_FEATURE",
        "TRANSPORT_ERROR",
        "SERIALIZATION_ERROR",
        "GRPC_ERROR",
        "UNKNOWN_ERROR",
    ];

    #[test]
    fn connection_error_has_known_error_code() {
        let out = cli(&["--json", "devices", "list"]);
        assert!(!out.status.success());
        let json = extract_json(&stderr(&out));
        let code = json["error_code"].as_str().unwrap();
        assert!(
            VALID_ERROR_CODES.contains(&code),
            "error_code '{}' should be a known code",
            code
        );
    }

    #[test]
    fn connection_error_exit_code_in_valid_range() {
        let out = cli(&["info"]);
        assert!(!out.status.success());
        let code = out.status.code().unwrap();
        assert!(
            (1..=7).contains(&code),
            "exit code {} should be in mapped range 1-7",
            code
        );
    }

    #[test]
    fn different_daemon_commands_return_consistent_error_codes() {
        let cmds: &[&[&str]] = &[
            &["--json", "info"],
            &["--json", "devices", "list"],
            &["--json", "diag", "health"],
        ];
        let mut codes = Vec::new();
        for args in cmds {
            let out = cli(args);
            let json = extract_json(&stderr(&out));
            let code = json["error_code"].as_str().unwrap().to_string();
            codes.push((args.to_vec(), code));
        }
        // All should have a valid error code (not empty)
        for (args, code) in &codes {
            assert!(
                VALID_ERROR_CODES.contains(&code.as_str()),
                "{:?} returned unknown error code '{}'",
                args,
                code
            );
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 10. Adapter command depth tests
// ════════════════════════════════════════════════════════════════════════════

mod adapter_commands {
    use super::*;

    #[test]
    fn status_fails_without_daemon() {
        let out = cli(&["adapters", "status"]);
        assert!(!out.status.success());
        assert_ne!(out.status.code(), Some(101));
    }

    #[test]
    fn enable_requires_sim_arg() {
        let out = cli(&["adapters", "enable"]);
        assert!(!out.status.success());
        let err = stderr(&out);
        assert!(err.contains("required") || err.contains("error:"));
    }

    #[test]
    fn disable_requires_sim_arg() {
        let out = cli(&["adapters", "disable"]);
        assert!(!out.status.success());
        let err = stderr(&out);
        assert!(err.contains("required") || err.contains("error:"));
    }

    #[test]
    fn reconnect_requires_sim_arg() {
        let out = cli(&["adapters", "reconnect"]);
        assert!(!out.status.success());
        let err = stderr(&out);
        assert!(err.contains("required") || err.contains("error:"));
    }

    #[test]
    fn enable_msfs_fails_without_daemon() {
        let out = cli(&["adapters", "enable", "msfs"]);
        assert!(!out.status.success());
        assert_ne!(out.status.code(), Some(101));
    }

    #[test]
    fn reconnect_xplane_fails_without_daemon() {
        let out = cli(&["adapters", "reconnect", "xplane"]);
        assert!(!out.status.success());
        assert_ne!(out.status.code(), Some(101));
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 11. Diag subcommand depth tests
// ════════════════════════════════════════════════════════════════════════════

mod diag_commands {
    use super::*;

    #[test]
    fn trace_requires_duration() {
        let out = cli(&["diag", "trace"]);
        assert!(!out.status.success());
        let err = stderr(&out);
        assert!(err.contains("required") || err.contains("error:"));
    }

    #[test]
    fn record_requires_output_path() {
        let out = cli(&["diag", "record"]);
        assert!(!out.status.success());
        let err = stderr(&out);
        assert!(err.contains("required") || err.contains("error:"));
    }

    #[test]
    fn replay_requires_input_path() {
        let out = cli(&["diag", "replay"]);
        assert!(!out.status.success());
        let err = stderr(&out);
        assert!(err.contains("required") || err.contains("error:"));
    }

    #[test]
    fn metrics_subcommand_parses() {
        let out = cli(&["diag", "metrics"]);
        // Should parse (daemon needed for execution)
        assert_ne!(out.status.code(), Some(2));
    }

    #[test]
    fn stop_subcommand_parses() {
        let out = cli(&["diag", "stop"]);
        assert_ne!(out.status.code(), Some(2));
    }

    #[test]
    fn status_subcommand_parses() {
        let out = cli(&["diag", "status"]);
        assert_ne!(out.status.code(), Some(2));
    }

    #[test]
    fn export_requires_input() {
        let out = cli(&["diag", "export"]);
        assert!(!out.status.success());
        let err = stderr(&out);
        assert!(err.contains("required") || err.contains("error:"));
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 12. Torque / panels / sim command depth tests
// ════════════════════════════════════════════════════════════════════════════

mod misc_commands {
    use super::*;

    #[test]
    fn torque_unlock_requires_device_id() {
        let out = cli(&["torque", "unlock"]);
        assert!(!out.status.success());
        let err = stderr(&out);
        assert!(err.contains("required") || err.contains("error:"));
    }

    #[test]
    fn torque_set_mode_requires_mode() {
        let out = cli(&["torque", "set-mode"]);
        assert!(!out.status.success());
    }

    #[test]
    fn torque_status_fails_without_daemon() {
        let out = cli(&["torque", "status"]);
        assert!(!out.status.success());
        assert_ne!(out.status.code(), Some(101));
    }

    #[test]
    fn panels_status_fails_without_daemon() {
        let out = cli(&["panels", "status"]);
        assert!(!out.status.success());
        assert_ne!(out.status.code(), Some(101));
    }

    #[test]
    fn panels_verify_fails_without_daemon() {
        let out = cli(&["panels", "verify"]);
        assert!(!out.status.success());
        assert_ne!(out.status.code(), Some(101));
    }

    #[test]
    fn metrics_snapshot_succeeds_with_placeholder() {
        let out = cli(&["metrics", "snapshot"]);
        // metrics snapshot returns a placeholder without requiring daemon
        assert!(
            out.status.success(),
            "metrics snapshot should succeed: {}",
            stderr(&out)
        );
    }
}
