// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Snapshot tests for capability modes and config format examples.
//!
//! Run `cargo insta review` to accept new or changed snapshots.

use flight_core::calibration_store::{AxisCalibration, CalibrationStore};
use flight_core::profile::{CapabilityContext, CapabilityMode};

// ── Capability mode context snapshots ────────────────────────────────────────

#[test]
fn snapshot_capability_context_full_mode() {
    let ctx = CapabilityContext::for_mode(CapabilityMode::Full);
    insta::assert_json_snapshot!("capability_context_full", ctx);
}

#[test]
fn snapshot_capability_context_demo_mode() {
    let ctx = CapabilityContext::for_mode(CapabilityMode::Demo);
    insta::assert_json_snapshot!("capability_context_demo", ctx);
}

#[test]
fn snapshot_capability_context_kid_mode() {
    let ctx = CapabilityContext::for_mode(CapabilityMode::Kid);
    insta::assert_json_snapshot!("capability_context_kid", ctx);
}

// ── Config format example snapshots ──────────────────────────────────────────

#[test]
fn snapshot_calibration_store_toml_format() {
    let mut store = CalibrationStore::new();
    store.set(
        0x044F,
        0xB10A,
        vec![
            AxisCalibration::new(0, 0, 65535, 32768),
            AxisCalibration::new(1, 0, 65535, 32768),
        ],
    );
    let toml_str = toml::to_string_pretty(&store).unwrap();
    insta::assert_snapshot!("calibration_store_toml_format", toml_str);
}

// ── Error catalog snapshots ──────────────────────────────────────────────────

use flight_core::error_catalog::{ErrorCatalog, ErrorCategory};

#[test]
fn snapshot_error_catalog_all_entries() {
    let entries: Vec<String> = ErrorCatalog::all()
        .iter()
        .map(|e| format!("[{}] ({}) {}", e.code, e.category, e.message))
        .collect();
    insta::assert_yaml_snapshot!("error_catalog_all_entries", entries);
}

#[test]
fn snapshot_error_catalog_device_category() {
    let entries: Vec<String> = ErrorCatalog::by_category(ErrorCategory::Device)
        .iter()
        .map(|e| ErrorCatalog::format_error(e.code))
        .collect();
    insta::assert_snapshot!("error_catalog_device", entries.join("\n---\n"));
}

#[test]
fn snapshot_error_catalog_sim_category() {
    let entries: Vec<String> = ErrorCatalog::by_category(ErrorCategory::Sim)
        .iter()
        .map(|e| ErrorCatalog::format_error(e.code))
        .collect();
    insta::assert_snapshot!("error_catalog_sim", entries.join("\n---\n"));
}

#[test]
fn snapshot_error_catalog_profile_category() {
    let entries: Vec<String> = ErrorCatalog::by_category(ErrorCategory::Profile)
        .iter()
        .map(|e| ErrorCatalog::format_error(e.code))
        .collect();
    insta::assert_snapshot!("error_catalog_profile", entries.join("\n---\n"));
}

#[test]
fn snapshot_error_catalog_internal_category() {
    let entries: Vec<String> = ErrorCatalog::by_category(ErrorCategory::Internal)
        .iter()
        .map(|e| ErrorCatalog::format_error(e.code))
        .collect();
    insta::assert_snapshot!("error_catalog_internal", entries.join("\n---\n"));
}

#[test]
fn snapshot_format_known_errors() {
    insta::assert_snapshot!("format_dev_001", ErrorCatalog::format_error("DEV-001"));
    insta::assert_snapshot!("format_svc_001", ErrorCatalog::format_error("SVC-001"));
    insta::assert_snapshot!("format_plg_003", ErrorCatalog::format_error("PLG-003"));
}

#[test]
fn snapshot_format_unknown_error() {
    insta::assert_snapshot!(
        "format_unknown_error",
        ErrorCatalog::format_error("ZZZ-999")
    );
}

// ── Circuit breaker state transition snapshots ───────────────────────────────

use flight_core::circuit_breaker::{CircuitBreaker, CircuitBreakerConfig};
use std::time::Duration;

fn trace_cb(cb: &CircuitBreaker) -> String {
    format!(
        "state={:?} failures={} total_calls={}",
        cb.state(),
        cb.failure_count(),
        cb.total_calls(),
    )
}

#[test]
fn snapshot_circuit_breaker_lifecycle() {
    let cfg = CircuitBreakerConfig {
        failure_threshold: 3,
        success_threshold: 2,
        timeout: Duration::from_millis(500),
    };
    let mut cb = CircuitBreaker::new(cfg);
    let mut trace = Vec::new();

    trace.push(format!("initial: {}", trace_cb(&cb)));

    cb.call_allowed();
    cb.record_success();
    trace.push(format!("after success 1: {}", trace_cb(&cb)));

    cb.call_allowed();
    cb.record_success();
    trace.push(format!("after success 2: {}", trace_cb(&cb)));

    // Three failures → Open
    cb.call_allowed();
    cb.record_failure();
    trace.push(format!("after failure 1: {}", trace_cb(&cb)));

    cb.call_allowed();
    cb.record_failure();
    trace.push(format!("after failure 2: {}", trace_cb(&cb)));

    cb.call_allowed();
    cb.record_failure();
    trace.push(format!("after failure 3 (threshold): {}", trace_cb(&cb)));

    // Rejected in Open state
    let result = cb.call_allowed();
    trace.push(format!(
        "call in open: result={:?} {}",
        result,
        trace_cb(&cb)
    ));

    // Wait for timeout → HalfOpen
    std::thread::sleep(Duration::from_millis(600));
    let result = cb.call_allowed();
    trace.push(format!(
        "after timeout: result={:?} {}",
        result,
        trace_cb(&cb)
    ));

    // Success in HalfOpen
    cb.record_success();
    trace.push(format!("half-open success 1: {}", trace_cb(&cb)));

    cb.call_allowed();
    cb.record_success();
    trace.push(format!("half-open success 2 (close): {}", trace_cb(&cb)));

    // Reset
    cb.reset();
    trace.push(format!("after reset: {}", trace_cb(&cb)));

    insta::assert_snapshot!("circuit_breaker_lifecycle", trace.join("\n"));
}

#[test]
fn snapshot_circuit_breaker_half_open_failure() {
    let cfg = CircuitBreakerConfig {
        failure_threshold: 1,
        success_threshold: 2,
        timeout: Duration::from_millis(500),
    };
    let mut cb = CircuitBreaker::new(cfg);
    let mut trace = Vec::new();

    trace.push(format!("initial: {}", trace_cb(&cb)));

    cb.call_allowed();
    cb.record_failure();
    trace.push(format!("after failure (open): {}", trace_cb(&cb)));

    std::thread::sleep(Duration::from_millis(600));
    cb.call_allowed();
    trace.push(format!("after timeout (half-open): {}", trace_cb(&cb)));

    cb.record_failure();
    trace.push(format!("failure in half-open (re-open): {}", trace_cb(&cb)));

    insta::assert_snapshot!("circuit_breaker_half_open_failure", trace.join("\n"));
}
