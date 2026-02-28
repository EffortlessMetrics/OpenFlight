// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Comprehensive snapshot tests for `flight-core` structured outputs.
//!
//! Covers error catalog messages, config structure serialization, and
//! FlightError display strings. Run `cargo insta review` to accept changes.

use flight_core::calibration_store::{AxisCalibration, CalibrationStore};
use flight_core::error::FlightError;
use flight_core::error_catalog::{ErrorCatalog, ErrorCategory};

// ── Error catalog messages (all error codes) ─────────────────────────────────

#[test]
fn snapshot_error_catalog_all_formatted() {
    let mut messages: Vec<String> = ErrorCatalog::all()
        .iter()
        .map(|e| ErrorCatalog::format_error(e.code))
        .collect();
    messages.sort();
    let output = messages.join("\n\n");
    insta::assert_snapshot!("error_catalog_all_formatted", output);
}

#[test]
fn snapshot_error_catalog_device_category() {
    let entries = ErrorCatalog::by_category(ErrorCategory::Device);
    let output: Vec<String> = entries
        .iter()
        .map(|e| format!("{}: {}", e.code, e.message))
        .collect();
    insta::assert_debug_snapshot!("error_catalog_device_category", output);
}

#[test]
fn snapshot_error_catalog_sim_category() {
    let entries = ErrorCatalog::by_category(ErrorCategory::Sim);
    let output: Vec<String> = entries
        .iter()
        .map(|e| format!("{}: {}", e.code, e.message))
        .collect();
    insta::assert_debug_snapshot!("error_catalog_sim_category", output);
}

#[test]
fn snapshot_error_catalog_profile_category() {
    let entries = ErrorCatalog::by_category(ErrorCategory::Profile);
    let output: Vec<String> = entries
        .iter()
        .map(|e| format!("{}: {}", e.code, e.message))
        .collect();
    insta::assert_debug_snapshot!("error_catalog_profile_category", output);
}

#[test]
fn snapshot_error_catalog_service_category() {
    let entries = ErrorCatalog::by_category(ErrorCategory::Service);
    let output: Vec<String> = entries
        .iter()
        .map(|e| format!("{}: {}", e.code, e.message))
        .collect();
    insta::assert_debug_snapshot!("error_catalog_service_category", output);
}

#[test]
fn snapshot_error_catalog_plugin_category() {
    let entries = ErrorCatalog::by_category(ErrorCategory::Plugin);
    let output: Vec<String> = entries
        .iter()
        .map(|e| format!("{}: {}", e.code, e.message))
        .collect();
    insta::assert_debug_snapshot!("error_catalog_plugin_category", output);
}

#[test]
fn snapshot_error_catalog_network_category() {
    let entries = ErrorCatalog::by_category(ErrorCategory::Network);
    let output: Vec<String> = entries
        .iter()
        .map(|e| format!("{}: {}", e.code, e.message))
        .collect();
    insta::assert_debug_snapshot!("error_catalog_network_category", output);
}

#[test]
fn snapshot_error_catalog_config_category() {
    let entries = ErrorCatalog::by_category(ErrorCategory::Config);
    let output: Vec<String> = entries
        .iter()
        .map(|e| format!("{}: {}", e.code, e.message))
        .collect();
    insta::assert_debug_snapshot!("error_catalog_config_category", output);
}

#[test]
fn snapshot_error_catalog_internal_category() {
    let entries = ErrorCatalog::by_category(ErrorCategory::Internal);
    let output: Vec<String> = entries
        .iter()
        .map(|e| format!("{}: {}", e.code, e.message))
        .collect();
    insta::assert_debug_snapshot!("error_catalog_internal_category", output);
}

#[test]
fn snapshot_error_catalog_code_index() {
    let mut codes: Vec<&str> = ErrorCatalog::all().iter().map(|e| e.code).collect();
    codes.sort();
    insta::assert_debug_snapshot!("error_catalog_code_index", codes);
}

// ── FlightError display strings ──────────────────────────────────────────────

#[test]
fn snapshot_flight_error_variants() {
    let errors: Vec<String> = vec![
        FlightError::Configuration("missing api_key".to_string()).to_string(),
        FlightError::Hardware("device stall on USB3".to_string()).to_string(),
        FlightError::Writer("output channel closed".to_string()).to_string(),
        FlightError::RulesValidation("invalid condition syntax".to_string()).to_string(),
    ];
    insta::assert_debug_snapshot!("flight_error_variants", errors);
}

// ── Config structure serialization ───────────────────────────────────────────

#[test]
fn snapshot_calibration_store_empty_json() {
    let store = CalibrationStore::new();
    insta::assert_json_snapshot!("calibration_store_empty", store);
}

#[test]
fn snapshot_calibration_store_with_devices_json() {
    let mut store = CalibrationStore::new();
    store.set(
        0x044F,
        0xB10A,
        vec![
            AxisCalibration::new(0, 0, 65535, 32768),
            AxisCalibration::new(1, 0, 65535, 32768),
            AxisCalibration::new(2, 0, 65535, 32768),
        ],
    );
    store.set(
        0x06A3,
        0x0C2D,
        vec![
            AxisCalibration::new(0, 100, 65000, 32550),
            AxisCalibration::new(1, 50, 64000, 32000),
        ],
    );
    insta::with_settings!({sort_maps => true}, {
        insta::assert_json_snapshot!("calibration_store_with_devices", store);
    });
}

#[test]
fn snapshot_axis_calibration_json() {
    let cal = AxisCalibration::new(0, 0, 65535, 32768);
    insta::assert_json_snapshot!("axis_calibration_single", cal);
}
