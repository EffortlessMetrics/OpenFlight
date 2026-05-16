// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Tests verifying all subcommands have help text and descriptions

mod common;

use common::cli;
use predicates::prelude::*;

// ── Top-level help ────────────────────────────────────────────────────────

#[test]
fn top_level_help_lists_all_subcommands() {
    let assert = cli().arg("--help").assert().success();

    let output = assert.get_output();
    let stdout = String::from_utf8_lossy(&output.stdout);

    let expected_subcommands = [
        "devices",
        "profile",
        "sim",
        "panels",
        "torque",
        "diag",
        "metrics",
        "dcs",
        "xplane",
        "ac7",
        "update",
        "cloud-profiles",
        "adapters",
        "overlay",
        "status",
        "info",
        "version",
        "safe-mode",
        "diagnostics",
    ];

    for cmd in &expected_subcommands {
        assert!(
            stdout.contains(cmd),
            "Top-level help should list '{}' subcommand.\nActual output:\n{}",
            cmd,
            stdout
        );
    }
}

#[test]
fn top_level_help_contains_description() {
    cli()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Flight Hub command line interface",
        ));
}

// ── Subcommand help descriptions ──────────────────────────────────────────

#[test]
fn devices_help_has_description() {
    cli()
        .args(["devices", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Device management commands"));
}

#[test]
fn devices_help_lists_subcommands() {
    let output = cli().args(["devices", "--help"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    for subcmd in &["list", "info", "dump", "calibrate", "test"] {
        assert!(
            stdout.contains(subcmd),
            "devices help missing '{}': {}",
            subcmd,
            stdout
        );
    }
}

#[test]
fn profile_help_has_description() {
    cli()
        .args(["profile", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Profile management commands"));
}

#[test]
fn profile_help_lists_subcommands() {
    let output = cli().args(["profile", "--help"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    for subcmd in &["list", "apply", "show", "activate", "validate", "export"] {
        assert!(
            stdout.contains(subcmd),
            "profile help missing '{}': {}",
            subcmd,
            stdout
        );
    }
}

#[test]
fn sim_help_has_description() {
    cli()
        .args(["sim", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Simulator configuration commands"));
}

#[test]
fn panels_help_has_description() {
    cli()
        .args(["panels", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Panel management commands"));
}

#[test]
fn torque_help_has_description() {
    cli()
        .args(["torque", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Force feedback and torque commands",
        ));
}

#[test]
fn diag_help_has_description() {
    cli()
        .args(["diag", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Diagnostics and recording commands",
        ));
}

#[test]
fn diag_help_lists_subcommands() {
    let output = cli().args(["diag", "--help"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    for subcmd in &[
        "bundle", "health", "metrics", "trace", "record", "replay", "status", "stop", "export",
    ] {
        assert!(
            stdout.contains(subcmd),
            "diag help missing '{}': {}",
            subcmd,
            stdout
        );
    }
}

#[test]
fn metrics_help_has_description() {
    cli()
        .args(["metrics", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("System-wide metrics"));
}

#[test]
fn dcs_help_has_description() {
    cli()
        .args(["dcs", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("DCS World integration commands"));
}

#[test]
fn xplane_help_has_description() {
    cli()
        .args(["xplane", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("X-Plane integration commands"));
}

#[test]
fn ac7_help_has_description() {
    cli()
        .args(["ac7", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Ace Combat 7 integration commands",
        ));
}

#[test]
fn adapters_help_has_description() {
    cli()
        .args(["adapters", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Simulator adapter management"));
}

#[test]
fn adapters_help_lists_subcommands() {
    let output = cli().args(["adapters", "--help"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    for subcmd in &["status", "enable", "disable", "reconnect"] {
        assert!(
            stdout.contains(subcmd),
            "adapters help missing '{}': {}",
            subcmd,
            stdout
        );
    }
}

#[test]
fn overlay_help_has_description() {
    cli()
        .args(["overlay", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("VR overlay management"));
}

#[test]
fn overlay_help_lists_subcommands() {
    let output = cli().args(["overlay", "--help"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    for subcmd in &["status", "show", "hide", "toggle", "notify", "backends"] {
        assert!(
            stdout.contains(subcmd),
            "overlay help missing '{}': {}",
            subcmd,
            stdout
        );
    }
}

#[test]
fn update_help_has_description() {
    cli()
        .args(["update", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Update channel management"));
}

#[test]
fn cloud_profiles_help_has_description() {
    cli()
        .args(["cloud-profiles", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Community cloud profile"));
}

// ── Leaf command help texts ───────────────────────────────────────────────

#[test]
fn devices_list_help_shows_flags() {
    let output = cli().args(["devices", "list", "--help"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(stdout.contains("--include-disconnected"));
    assert!(stdout.contains("--filter-types"));
}

#[test]
fn profile_validate_help_describes_command() {
    cli()
        .args(["profile", "validate", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Validate a profile file"));
}

#[test]
fn profile_export_help_describes_command() {
    cli()
        .args(["profile", "export", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Export a profile"));
}

#[test]
fn diag_record_help_shows_flags() {
    let output = cli().args(["diag", "record", "--help"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(stdout.contains("Start recording diagnostics"));
    assert!(stdout.contains("--output"));
    assert!(stdout.contains("--duration"));
}

#[test]
fn diag_trace_help_shows_flags() {
    let output = cli().args(["diag", "trace", "--help"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(stdout.contains("Record a trace"));
}
