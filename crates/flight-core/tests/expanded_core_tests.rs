// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Expanded tests for flight-core: error types, calibration store,
//! circuit breaker, profile watcher, and error catalog.

use flight_core::calibration_store::{AxisCalibration, CalibrationStore, CalibrationStoreError};
use flight_core::circuit_breaker::{
    CallResult, CircuitBreaker, CircuitBreakerConfig, CircuitState,
};
use flight_core::error::FlightError;
use flight_core::error_catalog::{ErrorCatalog, ErrorCategory};
use flight_core::profile_watcher::{FileChangeKind, ProfileWatcher, ReloadNotifier};
use proptest::prelude::*;
use std::path::PathBuf;
use std::time::Duration;

// ── FlightError coverage ────────────────────────────────────────────────────

#[test]
fn flight_error_io_variant_from_io_error() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
    let err: FlightError = io_err.into();
    assert!(err.to_string().contains("IO error"));
}

#[test]
fn flight_error_serialization_variant_from_serde() {
    let bad_json = "not json at all{{{";
    let serde_err = serde_json::from_str::<serde_json::Value>(bad_json).unwrap_err();
    let err: FlightError = serde_err.into();
    assert!(err.to_string().contains("Serialization error"));
}

#[test]
fn flight_error_all_string_variants_display() {
    let variants: Vec<FlightError> = vec![
        FlightError::RulesValidation("bad rule".into()),
        FlightError::Configuration("bad config".into()),
        FlightError::Writer("write fail".into()),
        FlightError::Hardware("device fail".into()),
    ];
    for err in &variants {
        let msg = err.to_string();
        assert!(!msg.is_empty(), "Display should not be empty for {:?}", err);
    }
}

#[test]
fn flight_error_debug_format_includes_variant() {
    let err = FlightError::Configuration("missing key".to_string());
    let dbg = format!("{:?}", err);
    assert!(dbg.contains("Configuration"));
    assert!(dbg.contains("missing key"));
}

// ── CalibrationStore: expanded coverage ─────────────────────────────────────

#[test]
fn calibration_store_overwrite_device() {
    let mut store = CalibrationStore::new();
    store.set(0x1234, 0x5678, vec![AxisCalibration::new(0, 0, 1000, 500)]);
    store.set(0x1234, 0x5678, vec![AxisCalibration::new(0, 100, 900, 500)]);
    let cals = store.get(0x1234, 0x5678).unwrap();
    assert_eq!(cals.len(), 1);
    assert_eq!(cals[0].raw_min, 100, "overwrite should replace data");
    assert_eq!(store.device_count(), 1);
}

#[test]
fn calibration_store_multiple_devices() {
    let mut store = CalibrationStore::new();
    for i in 0..10u16 {
        store.set(i, i + 100, vec![AxisCalibration::new(0, 0, 65535, 32767)]);
    }
    assert_eq!(store.device_count(), 10);
    for i in 0..10u16 {
        assert!(store.get(i, i + 100).is_some());
    }
}

#[test]
fn calibration_store_remove_nonexistent() {
    let mut store = CalibrationStore::new();
    assert!(!store.remove(0x0000, 0x0000));
}

#[test]
fn calibration_store_get_nonexistent() {
    let store = CalibrationStore::new();
    assert!(store.get(0xFFFF, 0xFFFF).is_none());
}

#[test]
fn calibration_normalize_zero_range_above_center() {
    // raw_max == raw_center → 0 range above center
    let cal = AxisCalibration::new(0, 0, 500, 500);
    assert_eq!(cal.normalize(500), 0.0);
    assert_eq!(cal.normalize(600), 0.0);
}

#[test]
fn calibration_normalize_zero_range_below_center() {
    // raw_min == raw_center → 0 range below center
    let cal = AxisCalibration::new(0, 500, 1000, 500);
    assert_eq!(cal.normalize(500), 0.0);
    assert_eq!(cal.normalize(400), 0.0);
}

#[test]
fn calibration_normalize_clamping() {
    let cal = AxisCalibration::new(0, 0, 1000, 500);
    // Values beyond max should clamp to 1.0
    let above_max = cal.normalize(1500);
    assert!((above_max - 1.0).abs() < f32::EPSILON);
    // Values below min should clamp to -1.0
    let below_min = cal.normalize(-500);
    assert!((below_min - (-1.0)).abs() < f32::EPSILON);
}

#[test]
fn calibration_store_save_load_empty() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("empty_cal.toml");
    let store = CalibrationStore::new();
    store.save_to_file(&path).unwrap();

    let loaded = CalibrationStore::load_from_file(&path).unwrap();
    assert_eq!(loaded.device_count(), 0);
}

#[test]
fn calibration_store_load_invalid_toml() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bad.toml");
    std::fs::write(&path, "this is not valid toml {{{}}}").unwrap();
    let result = CalibrationStore::load_from_file(&path);
    assert!(result.is_err());
}

#[test]
fn calibration_store_error_display() {
    let err = CalibrationStoreError::TomlParse("bad input".to_string());
    assert!(err.to_string().contains("TOML parse error"));

    let err2 = CalibrationStoreError::TomlSerialize("serialize fail".to_string());
    assert!(err2.to_string().contains("TOML serialize error"));
}

// ── CalibrationStore: proptest ──────────────────────────────────────────────

proptest! {
    #[test]
    fn prop_calibration_normalize_in_range(
        raw_min in -10000i32..0,
        raw_max in 1..10000i32,
        raw in -10000i32..10000,
    ) {
        let center = (raw_min + raw_max) / 2;
        let cal = AxisCalibration::new(0, raw_min, raw_max, center);
        let normalized = cal.normalize(raw);
        prop_assert!(normalized >= -1.0, "normalize({}) = {} < -1.0", raw, normalized);
        prop_assert!(normalized <= 1.0, "normalize({}) = {} > 1.0", raw, normalized);
    }

    #[test]
    fn prop_calibration_normalize_center_is_zero(
        raw_min in -10000i32..-1,
        raw_max in 1..10000i32,
    ) {
        let center = (raw_min + raw_max) / 2;
        let cal = AxisCalibration::new(0, raw_min, raw_max, center);
        let at_center = cal.normalize(center);
        prop_assert!(
            at_center.abs() < 0.01,
            "normalize(center={}) = {}, expected ~0.0",
            center,
            at_center
        );
    }

    #[test]
    fn prop_calibration_store_set_get_roundtrip(
        vid in 0u16..=0xFFFF,
        pid in 0u16..=0xFFFF,
        axis_id in 0u8..=7,
    ) {
        let mut store = CalibrationStore::new();
        let cal = AxisCalibration::new(axis_id, 0, 65535, 32767);
        store.set(vid, pid, vec![cal.clone()]);
        let got = store.get(vid, pid).unwrap();
        prop_assert_eq!(got.len(), 1);
        prop_assert_eq!(got[0].axis_id, axis_id);
    }
}

// ── CircuitBreaker: expanded coverage ───────────────────────────────────────

#[test]
fn circuit_breaker_success_resets_failure_count_in_closed() {
    let mut cb = CircuitBreaker::new(CircuitBreakerConfig {
        failure_threshold: 5,
        success_threshold: 1,
        timeout: Duration::from_secs(60),
    });
    cb.record_failure();
    cb.record_failure();
    assert_eq!(cb.failure_count(), 2);
    cb.record_success();
    assert_eq!(cb.failure_count(), 0);
}

#[test]
fn circuit_breaker_record_success_in_open_state_noop() {
    let mut cb = CircuitBreaker::new(CircuitBreakerConfig {
        failure_threshold: 1,
        success_threshold: 1,
        timeout: Duration::from_secs(60),
    });
    cb.record_failure(); // Opens
    assert_eq!(cb.state(), CircuitState::Open);
    cb.record_success(); // Should be a no-op in Open
    assert_eq!(cb.state(), CircuitState::Open);
}

#[test]
fn circuit_breaker_half_open_needs_enough_successes() {
    let cfg = CircuitBreakerConfig {
        failure_threshold: 1,
        success_threshold: 3,
        timeout: Duration::from_millis(10),
    };
    let mut cb = CircuitBreaker::new(cfg);
    cb.record_failure(); // Open
    std::thread::sleep(Duration::from_millis(20));
    let _ = cb.call_allowed(); // HalfOpen
    assert_eq!(cb.state(), CircuitState::HalfOpen);

    cb.record_success();
    assert_eq!(cb.state(), CircuitState::HalfOpen);
    cb.record_success();
    assert_eq!(cb.state(), CircuitState::HalfOpen);
    cb.record_success();
    assert_eq!(cb.state(), CircuitState::Closed);
}

#[test]
fn circuit_breaker_total_rejections_increments() {
    let cfg = CircuitBreakerConfig {
        failure_threshold: 1,
        success_threshold: 1,
        timeout: Duration::from_secs(60),
    };
    let mut cb = CircuitBreaker::new(cfg);
    assert_eq!(cb.total_rejections(), 0);
    cb.record_failure(); // Opens circuit
    let _ = cb.call_allowed(); // Rejected
    let _ = cb.call_allowed(); // Rejected again
    assert_eq!(cb.total_rejections(), 2);
}

#[test]
fn circuit_breaker_reset_clears_last_failure_time() {
    let cfg = CircuitBreakerConfig {
        failure_threshold: 1,
        success_threshold: 1,
        timeout: Duration::from_millis(10),
    };
    let mut cb = CircuitBreaker::new(cfg);
    cb.record_failure(); // Open, sets last_failure_time
    cb.reset();
    // After reset, calling call_allowed should be Allowed (Closed state)
    assert_eq!(cb.call_allowed(), CallResult::Allowed);
    assert_eq!(cb.state(), CircuitState::Closed);
}

proptest! {
    #[test]
    fn prop_circuit_breaker_rejections_le_total_calls(events in proptest::collection::vec(0u8..3, 1..50)) {
        let mut cb = CircuitBreaker::new(CircuitBreakerConfig {
            failure_threshold: 2,
            success_threshold: 2,
            timeout: Duration::from_millis(1),
        });
        for event in events {
            match event {
                0 => { let _ = cb.call_allowed(); }
                1 => cb.record_success(),
                _ => cb.record_failure(),
            }
        }
        prop_assert!(
            cb.total_rejections() <= cb.total_calls(),
            "rejections {} > total_calls {}",
            cb.total_rejections(),
            cb.total_calls()
        );
    }
}

// ── ProfileWatcher: expanded coverage ───────────────────────────────────────

#[test]
fn profile_watcher_ignores_non_yaml_toml_files() {
    let dir = tempfile::tempdir().unwrap();
    let mut watcher = ProfileWatcher::with_default_interval(dir.path().to_path_buf());
    watcher.poll(); // initial scan

    // Create non-yaml/toml files
    std::fs::write(dir.path().join("readme.txt"), "hello").unwrap();
    std::fs::write(dir.path().join("config.json"), "{}").unwrap();

    let events = watcher.poll();
    assert!(events.is_empty(), "non-yaml/toml files should be ignored");
}

#[test]
fn profile_watcher_detects_toml_files() {
    let dir = tempfile::tempdir().unwrap();
    let mut watcher = ProfileWatcher::with_default_interval(dir.path().to_path_buf());
    watcher.poll(); // initial scan

    let file = dir.path().join("profile.toml");
    std::fs::write(&file, "key = 'value'").unwrap();

    let events = watcher.poll();
    assert!(events.iter().any(|e| e.kind == FileChangeKind::Created && e.path == file));
}

#[test]
fn profile_watcher_poll_interval_getter() {
    let watcher = ProfileWatcher::new(PathBuf::from("/tmp/test"), Duration::from_secs(5));
    assert_eq!(watcher.poll_interval(), Duration::from_secs(5));
}

#[test]
fn profile_watcher_second_poll_no_changes_is_empty() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("stable.yaml");
    std::fs::write(&file, "stable: true").unwrap();

    let mut watcher = ProfileWatcher::with_default_interval(dir.path().to_path_buf());
    let _ = watcher.poll(); // first scan picks up Created
    let events = watcher.poll(); // second poll, no changes
    assert!(events.is_empty());
}

// ── ReloadNotifier: expanded coverage ───────────────────────────────────────

#[test]
fn reload_notifier_drain_twice_returns_empty_second_time() {
    let notifier = ReloadNotifier::new();
    notifier.notify(PathBuf::from("a.yaml"));
    assert_eq!(notifier.drain().len(), 1);
    assert_eq!(notifier.drain().len(), 0);
}

#[test]
fn reload_notifier_multiple_distinct_paths() {
    let notifier = ReloadNotifier::new();
    notifier.notify(PathBuf::from("a.yaml"));
    notifier.notify(PathBuf::from("b.yaml"));
    notifier.notify(PathBuf::from("c.yaml"));
    let drained = notifier.drain();
    assert_eq!(drained.len(), 3);
}

#[test]
fn reload_notifier_default_is_empty() {
    let notifier = ReloadNotifier::default();
    assert!(!notifier.has_pending());
    assert!(notifier.drain().is_empty());
}

// ── ErrorCatalog: expanded coverage ─────────────────────────────────────────

#[test]
fn error_catalog_all_entries_have_non_empty_fields() {
    for info in ErrorCatalog::all() {
        assert!(!info.code.is_empty(), "code should not be empty");
        assert!(!info.message.is_empty(), "message should not be empty for {}", info.code);
        assert!(
            !info.description.is_empty(),
            "description should not be empty for {}",
            info.code
        );
        assert!(
            !info.resolution.is_empty(),
            "resolution should not be empty for {}",
            info.code
        );
    }
}

#[test]
fn error_catalog_by_category_covers_all() {
    let categories = [
        ErrorCategory::Device,
        ErrorCategory::Sim,
        ErrorCategory::Profile,
        ErrorCategory::Service,
        ErrorCategory::Plugin,
        ErrorCategory::Network,
        ErrorCategory::Config,
        ErrorCategory::Internal,
    ];
    let mut total = 0;
    for cat in categories {
        total += ErrorCatalog::by_category(cat).len();
    }
    assert_eq!(total, ErrorCatalog::all().len());
}

#[test]
fn error_catalog_format_all_known_codes() {
    for info in ErrorCatalog::all() {
        let formatted = ErrorCatalog::format_error(info.code);
        assert!(formatted.contains(info.message));
        assert!(formatted.contains("Resolution:"));
    }
}

#[test]
fn error_category_display_all_variants() {
    let expected = [
        (ErrorCategory::Device, "Device"),
        (ErrorCategory::Sim, "Simulator"),
        (ErrorCategory::Profile, "Profile"),
        (ErrorCategory::Service, "Service"),
        (ErrorCategory::Plugin, "Plugin"),
        (ErrorCategory::Network, "Network"),
        (ErrorCategory::Config, "Configuration"),
        (ErrorCategory::Internal, "Internal"),
    ];
    for (cat, name) in expected {
        assert_eq!(cat.to_string(), name);
    }
}

#[test]
fn error_catalog_lookup_every_known_code() {
    for info in ErrorCatalog::all() {
        let found = ErrorCatalog::lookup(info.code).expect("should find by code");
        assert_eq!(found.code, info.code);
        assert_eq!(found.category, info.category);
    }
}

#[test]
fn error_catalog_format_unknown_contains_code() {
    let formatted = ErrorCatalog::format_error("ABC-999");
    assert!(formatted.contains("ABC-999"));
    assert!(formatted.contains("Unknown error code"));
}

proptest! {
    #[test]
    fn prop_error_catalog_by_category_all_match(idx in 0usize..100) {
        let all = ErrorCatalog::all();
        if idx < all.len() {
            let info = &all[idx];
            let cat_entries = ErrorCatalog::by_category(info.category);
            prop_assert!(
                cat_entries.iter().any(|e| e.code == info.code),
                "code {} not found in its own category {:?}",
                info.code,
                info.category
            );
        }
    }
}
