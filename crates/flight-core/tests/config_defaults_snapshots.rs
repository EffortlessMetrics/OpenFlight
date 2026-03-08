// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Snapshot tests for configuration defaults and stable display formats.
//!
//! Guards against accidental changes to default values and Display impls
//! that downstream code or users depend on.  Run `cargo insta review` to
//! accept new or changed snapshots.

use flight_core::error::FlightError;
use flight_core::WatchdogConfig;

// ── WatchdogConfig defaults ──────────────────────────────────────────────────

#[test]
fn snapshot_watchdog_config_default_debug() {
    let cfg = WatchdogConfig::default();
    insta::assert_debug_snapshot!("watchdog_config_default", cfg);
}

// ── FlightError display strings (all constructable variants) ─────────────────

#[test]
fn snapshot_flight_error_display_catalog() {
    let errors: Vec<String> = vec![
        FlightError::Configuration("missing api_key".into()).to_string(),
        FlightError::Hardware("USB stall on endpoint 3".into()).to_string(),
        FlightError::Writer("output channel closed".into()).to_string(),
        FlightError::RulesValidation("unknown operator '~~'".into()).to_string(),
        FlightError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "file gone")).to_string(),
    ];
    insta::assert_debug_snapshot!("flight_error_display_catalog", errors);
}

// ── SecurityConfig defaults ──────────────────────────────────────────────────

#[test]
fn snapshot_security_config_default_debug() {
    let cfg = flight_core::SecurityConfig::default();
    insta::assert_debug_snapshot!("security_config_default", cfg);
}

// ── ProcessDetectionConfig defaults ──────────────────────────────────────────
// NOTE: ProcessDetectionConfig contains a HashMap which has non-deterministic
// Debug ordering, so we snapshot only the deterministic scalar fields.

#[test]
fn snapshot_process_detection_config_scalars() {
    let cfg = flight_core::ProcessDetectionConfig::default();
    let output = format!(
        "detection_interval: {:?}\nenable_window_detection: {}\nmax_detection_time: {:?}\nnum_sim_definitions: {}",
        cfg.detection_interval,
        cfg.enable_window_detection,
        cfg.max_detection_time,
        cfg.process_definitions.len(),
    );
    insta::assert_snapshot!("process_detection_config_scalars", output);
}

// ── CircuitBreaker config default state ───────────────────────────────────────

#[test]
fn snapshot_circuit_breaker_config_debug() {
    let cfg = flight_core::circuit_breaker::CircuitBreakerConfig {
        failure_threshold: 5,
        success_threshold: 3,
        timeout: std::time::Duration::from_secs(30),
    };
    insta::assert_debug_snapshot!("circuit_breaker_config", cfg);
}
