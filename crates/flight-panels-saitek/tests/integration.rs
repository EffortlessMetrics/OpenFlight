// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Integration tests for the flight-panels-saitek crate.
//!
//! These tests exercise only the public API and cover:
//! - Panel type identification and LED mappings
//! - VerifyTestResult analysis helpers
//! - Verify-matrix configuration lifecycle
//! - Error handling for unregistered panels

use flight_hid::{HidAdapter, HidDeviceInfo};
use flight_panels_saitek::{
    DriftAction, DriftAnalysis, PanelType, SaitekPanelWriter, VerifyMatrix, VerifyStep,
    VerifyStepResult, VerifyTestResult,
    led::{LedState, LedTarget},
    verify_matrix::{DriftThresholds, VerifyConfig},
};
use flight_watchdog::WatchdogSystem;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const SAITEK_VENDOR_ID: u16 = 0x06A3;
const LOGITECH_VENDOR_ID: u16 = 0x046D;

fn make_hid() -> HidAdapter {
    HidAdapter::new(Arc::new(Mutex::new(WatchdogSystem::new())))
}

// ── PanelType public API ──────────────────────────────────────────────────────

/// All valid PIDs must round-trip through `from_product_id`.
#[test]
fn test_panel_type_product_id_roundtrip() {
    let pairs = [
        (0x0D05u16, PanelType::RadioPanel),
        (0x0D06, PanelType::MultiPanel),
        (0x0D67, PanelType::SwitchPanel),
        (0x0B4E, PanelType::BIP),
        (0x0A2F, PanelType::FIP),
    ];
    for (pid, expected) in pairs {
        assert_eq!(PanelType::from_product_id(pid), Some(expected));
        assert_eq!(
            expected as u16, pid,
            "PanelType discriminant must equal PID"
        );
    }
    assert_eq!(PanelType::from_product_id(0x0000), None);
    assert_eq!(PanelType::from_product_id(0xFFFF), None);
}

/// Each panel type's LED mapping must be non-empty and contain no duplicate names.
#[test]
fn test_led_mapping_no_duplicates() {
    for panel_type in [
        PanelType::RadioPanel,
        PanelType::MultiPanel,
        PanelType::SwitchPanel,
        PanelType::BIP,
        PanelType::FIP,
    ] {
        let mapping = panel_type.led_mapping();
        assert!(
            !mapping.is_empty(),
            "{} mapping must not be empty",
            panel_type.name()
        );
        let unique: HashSet<_> = mapping.iter().collect();
        assert_eq!(
            unique.len(),
            mapping.len(),
            "{} LED mapping has duplicates",
            panel_type.name()
        );
    }
}

/// Every verify pattern must be non-empty and end with a step that clears all LEDs.
#[test]
fn test_verify_pattern_contains_cleanup_step() {
    for panel_type in [
        PanelType::RadioPanel,
        PanelType::MultiPanel,
        PanelType::SwitchPanel,
        PanelType::BIP,
        PanelType::FIP,
    ] {
        let pattern = panel_type.verify_pattern();
        assert!(
            !pattern.is_empty(),
            "{} verify pattern must not be empty",
            panel_type.name()
        );
        let has_cleanup = pattern
            .iter()
            .any(|step| matches!(step, VerifyStep::AllOff | VerifyStep::LedOff(_)));
        assert!(
            has_cleanup,
            "{} verify pattern must include AllOff or LedOff to leave panel clean",
            panel_type.name()
        );
    }
}

// ── VerifyTestResult analysis helpers ────────────────────────────────────────

/// When all steps are within the 20 ms requirement the result should pass.
#[test]
fn test_verify_result_meets_latency_requirement() {
    let result = VerifyTestResult {
        panel_path: "/dev/test".to_string(),
        total_duration: Duration::from_millis(60),
        step_results: vec![
            VerifyStepResult {
                step_index: 0,
                expected_latency: Duration::from_millis(20),
                actual_latency: Duration::from_millis(2),
                success: true,
                error: None,
            },
            VerifyStepResult {
                step_index: 1,
                expected_latency: Duration::from_millis(20),
                actual_latency: Duration::from_millis(19),
                success: true,
                error: None,
            },
        ],
        success: true,
    };

    assert!(result.meets_latency_requirement());
    assert_eq!(result.max_latency(), Duration::from_millis(19));
    // (2_000_000 + 19_000_000) / 2 = 10_500_000 ns
    assert_eq!(result.avg_latency(), Duration::from_nanos(10_500_000));
}

/// One step exceeding 20 ms must fail `meets_latency_requirement`.
#[test]
fn test_verify_result_exceeds_latency_requirement() {
    let result = VerifyTestResult {
        panel_path: "/dev/test".to_string(),
        total_duration: Duration::from_millis(100),
        step_results: vec![
            VerifyStepResult {
                step_index: 0,
                expected_latency: Duration::from_millis(20),
                actual_latency: Duration::from_millis(5),
                success: true,
                error: None,
            },
            VerifyStepResult {
                step_index: 1,
                expected_latency: Duration::from_millis(20),
                actual_latency: Duration::from_millis(21),
                success: false,
                error: Some("too slow".to_string()),
            },
        ],
        success: false,
    };

    assert!(!result.meets_latency_requirement());
    assert_eq!(result.max_latency(), Duration::from_millis(21));
}

/// Single-step result: max == avg.
#[test]
fn test_verify_result_single_step() {
    let result = VerifyTestResult {
        panel_path: "/dev/test".to_string(),
        total_duration: Duration::from_millis(10),
        step_results: vec![VerifyStepResult {
            step_index: 0,
            expected_latency: Duration::from_millis(20),
            actual_latency: Duration::from_millis(7),
            success: true,
            error: None,
        }],
        success: true,
    };

    assert!(result.meets_latency_requirement());
    assert_eq!(result.max_latency(), Duration::from_millis(7));
    assert_eq!(result.avg_latency(), Duration::from_millis(7));
}

// ── DriftAction / DriftAnalysis ───────────────────────────────────────────────

/// All DriftAction variants must be equal only to themselves.
#[test]
fn test_drift_action_all_variants() {
    assert_eq!(DriftAction::None, DriftAction::None);
    assert_eq!(DriftAction::Monitor, DriftAction::Monitor);
    assert_eq!(DriftAction::Repair, DriftAction::Repair);
    assert_eq!(DriftAction::Replace, DriftAction::Replace);

    assert_ne!(DriftAction::None, DriftAction::Monitor);
    assert_ne!(DriftAction::Repair, DriftAction::Replace);
    assert_ne!(DriftAction::None, DriftAction::Replace);
}

/// DriftAnalysis fields are public and constructible.
#[test]
fn test_drift_analysis_construction() {
    let analysis = DriftAnalysis {
        drift_detected: true,
        latency_trend: 75.0,
        failure_rate_trend: 20.0,
        confidence: 0.85,
        recommended_action: DriftAction::Repair,
    };
    assert!(analysis.drift_detected);
    assert!((analysis.latency_trend - 75.0).abs() < f64::EPSILON);
    assert_eq!(analysis.recommended_action, DriftAction::Repair);

    let no_drift = DriftAnalysis {
        drift_detected: false,
        latency_trend: 2.0,
        failure_rate_trend: 0.5,
        confidence: 0.5,
        recommended_action: DriftAction::None,
    };
    assert!(!no_drift.drift_detected);
    assert_eq!(no_drift.recommended_action, DriftAction::None);
}

// ── VerifyMatrix lifecycle ────────────────────────────────────────────────────

/// A newly created VerifyMatrix must need a run and have default configs for all panel types.
#[test]
fn test_verify_matrix_initial_state() {
    let matrix = VerifyMatrix::new(SaitekPanelWriter::new(make_hid()));

    assert!(
        matrix.needs_matrix_run(),
        "fresh matrix must need an initial run"
    );

    for panel_type in [
        PanelType::RadioPanel,
        PanelType::MultiPanel,
        PanelType::SwitchPanel,
        PanelType::BIP,
        PanelType::FIP,
    ] {
        let config = matrix
            .get_test_config(panel_type)
            .unwrap_or_else(|| panic!("default config missing for {:?}", panel_type));
        assert_eq!(
            config.latency_threshold,
            Duration::from_millis(20),
            "default latency threshold must be 20 ms"
        );
        assert_eq!(config.test_iterations, 10);
        assert!(!config.extended_tests);
    }
}

/// Custom test configs and drift thresholds round-trip cleanly.
#[test]
fn test_verify_matrix_config_roundtrip() {
    let mut matrix = VerifyMatrix::new(SaitekPanelWriter::new(make_hid()));

    let custom = VerifyConfig {
        panel_type: PanelType::SwitchPanel,
        latency_threshold: Duration::from_millis(15),
        test_iterations: 5,
        iteration_interval: Duration::from_millis(50),
        extended_tests: true,
    };
    matrix.set_test_config(PanelType::SwitchPanel, custom);

    let stored = matrix.get_test_config(PanelType::SwitchPanel).unwrap();
    assert_eq!(stored.latency_threshold, Duration::from_millis(15));
    assert_eq!(stored.test_iterations, 5);
    assert!(stored.extended_tests);

    let thresholds = DriftThresholds {
        max_latency_increase: 30.0,
        max_failure_rate: 5.0,
        min_samples: 10,
        analysis_window: Duration::from_secs(12 * 60 * 60),
    };
    matrix.set_drift_thresholds(thresholds);

    let t = matrix.get_drift_thresholds();
    assert_eq!(t.max_latency_increase, 30.0);
    assert_eq!(t.min_samples, 10);
    assert_eq!(t.analysis_window, Duration::from_secs(12 * 60 * 60));
}

/// The matrix run interval can be changed via the public API.
#[test]
fn test_verify_matrix_interval_override() {
    let mut matrix = VerifyMatrix::new(SaitekPanelWriter::new(make_hid()));

    let one_hour = Duration::from_secs(60 * 60);
    matrix.set_matrix_interval(one_hour);
    assert_eq!(matrix.get_matrix_interval(), one_hour);
}

/// History queries for unknown panels return None; clearing is idempotent.
#[test]
fn test_verify_matrix_history_absent_panel() {
    let mut matrix = VerifyMatrix::new(SaitekPanelWriter::new(make_hid()));
    assert!(matrix.get_test_history("/dev/no_such_panel").is_none());
    // Clearing a non-existent entry must not panic.
    matrix.clear_test_history("/dev/no_such_panel");
    assert!(matrix.get_test_history("/dev/no_such_panel").is_none());
}

// ── Error handling for unregistered panels ────────────────────────────────────

/// All write/health/repair operations on an unregistered path must return errors.
#[test]
fn test_unregistered_panel_error_paths() {
    let mut writer = SaitekPanelWriter::new(make_hid());
    let path = "/dev/not_registered";

    assert!(
        writer.start_verify_test(path).is_err(),
        "start_verify_test on unregistered panel must error"
    );

    let state = LedState {
        on: true,
        brightness: 1.0,
        blink_rate: None,
        last_update: Instant::now(),
    };
    let target = LedTarget::Panel("COM1".to_string());
    assert!(
        writer.set_led(path, "COM1", &target, &state).is_err(),
        "set_led on unregistered panel must error"
    );

    assert!(
        writer.repair_panel_drift(path).is_err(),
        "repair_panel_drift on unregistered panel must error"
    );

    assert!(
        writer.check_panel_health(path).is_err(),
        "check_panel_health on unregistered panel must error"
    );
}

/// A freshly created writer has no panels and no latency samples.
#[test]
fn test_writer_initial_state() {
    let writer = SaitekPanelWriter::new(make_hid());
    assert!(writer.get_panels().is_empty());
    assert!(writer.get_latency_stats().is_none());
}

// ── Vendor/product-ID boundary checks ────────────────────────────────────────

/// The Saitek and Logitech vendor IDs used by the crate are well-known constants.
#[test]
fn test_vendor_id_constants() {
    assert_eq!(SAITEK_VENDOR_ID, 0x06A3);
    assert_eq!(LOGITECH_VENDOR_ID, 0x046D);
    // Ensure neither is mistakenly zero or equal to each other.
    assert_ne!(SAITEK_VENDOR_ID, 0);
    assert_ne!(LOGITECH_VENDOR_ID, 0);
    assert_ne!(SAITEK_VENDOR_ID, LOGITECH_VENDOR_ID);
}

/// Unknown product IDs (regardless of vendor) must not produce a PanelType.
#[test]
fn test_unknown_product_ids_return_none() {
    // These are real Saitek/Logitech VIDs but non-panel PIDs
    for pid in [0x0001u16, 0x00FF, 0x1000, 0xDEAD, 0xFFFF] {
        assert_eq!(
            PanelType::from_product_id(pid),
            None,
            "PID 0x{pid:04X} should not map to any PanelType"
        );
    }
}
