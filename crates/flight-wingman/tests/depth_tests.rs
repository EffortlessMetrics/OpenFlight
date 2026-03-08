// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Depth tests for the `flight-wingman` crate.
//!
//! Covers: adapter lifecycle, virtual controller behaviour, config validation,
//! metrics accounting, snapshot correctness, error handling, and property-based
//! fuzz tests via `proptest`.

use std::time::Duration;

use flight_adapter_common::AdapterState;
use flight_bus::types::{AircraftId, SimId};
use flight_wingman::virtual_controller::{
    StubVirtualController, VirtualController, VirtualControllerError,
};
use flight_wingman::{WingmanAdapter, WingmanConfig, WingmanError};

// ── Helper ──────────────────────────────────────────────────────────────────

fn default_adapter() -> WingmanAdapter {
    WingmanAdapter::new(WingmanConfig::default())
}

fn started_adapter() -> WingmanAdapter {
    let mut a = default_adapter();
    a.start();
    a
}

// ═══════════════════════════════════════════════════════════════════════════
// 1. Adapter lifecycle & state machine
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn adapter_starts_disconnected() {
    assert_eq!(default_adapter().state(), AdapterState::Disconnected);
}

#[test]
fn start_transitions_to_connected() {
    let mut a = default_adapter();
    a.start();
    assert_eq!(a.state(), AdapterState::Connected);
}

#[test]
fn stop_transitions_to_disconnected() {
    let mut a = started_adapter();
    a.stop();
    assert_eq!(a.state(), AdapterState::Disconnected);
}

#[test]
fn double_start_stays_connected() {
    let mut a = default_adapter();
    a.start();
    a.start();
    assert_eq!(a.state(), AdapterState::Connected);
}

#[test]
fn double_stop_stays_disconnected() {
    let mut a = started_adapter();
    a.stop();
    a.stop();
    assert_eq!(a.state(), AdapterState::Disconnected);
}

#[test]
fn start_stop_start_cycle() {
    let mut a = default_adapter();
    a.start();
    assert_eq!(a.state(), AdapterState::Connected);
    a.stop();
    assert_eq!(a.state(), AdapterState::Disconnected);
    a.start();
    assert_eq!(a.state(), AdapterState::Connected);
}

#[test]
fn stop_without_start_is_noop() {
    let mut a = default_adapter();
    a.stop(); // should not panic
    assert_eq!(a.state(), AdapterState::Disconnected);
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. Polling & snapshot correctness
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn poll_once_returns_snapshot_when_started() {
    let mut a = started_adapter();
    let snap = a.poll_once().unwrap();
    assert!(snap.is_some());
}

#[test]
fn poll_once_not_started_is_err() {
    let mut a = default_adapter();
    assert!(matches!(a.poll_once(), Err(WingmanError::NotStarted)));
}

#[test]
fn poll_after_stop_is_err() {
    let mut a = started_adapter();
    a.stop();
    assert!(matches!(a.poll_once(), Err(WingmanError::NotStarted)));
}

#[test]
fn snapshot_sim_id_is_wingman() {
    let mut a = started_adapter();
    let snap = a.poll_once().unwrap().unwrap();
    assert_eq!(snap.sim, SimId::Wingman);
}

#[test]
fn snapshot_aircraft_id_is_wingman() {
    let mut a = started_adapter();
    let snap = a.poll_once().unwrap().unwrap();
    assert_eq!(snap.aircraft, AircraftId::new("WINGMAN"));
}

#[test]
fn snapshot_validity_flags_all_false() {
    let mut a = started_adapter();
    let snap = a.poll_once().unwrap().unwrap();
    assert!(!snap.validity.attitude_valid);
    assert!(!snap.validity.angular_rates_valid);
    assert!(!snap.validity.velocities_valid);
    assert!(!snap.validity.kinematics_valid);
    assert!(!snap.validity.position_valid);
    assert!(!snap.validity.safe_for_ffb);
}

#[test]
fn snapshot_timestamp_is_nonzero() {
    let mut a = started_adapter();
    let snap = a.poll_once().unwrap().unwrap();
    assert_ne!(snap.timestamp, 0, "timestamp should be non-zero");
}

#[test]
fn successive_snapshots_have_increasing_timestamps() {
    let mut a = started_adapter();
    let s1 = a.poll_once().unwrap().unwrap();
    let s2 = a.poll_once().unwrap().unwrap();
    assert!(s2.timestamp >= s1.timestamp, "timestamps must be non-decreasing");
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. Virtual controller — axis behaviour
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn send_axis_succeeds_when_started() {
    let mut a = started_adapter();
    assert!(a.send_axis(0, 0.5).is_ok());
}

#[test]
fn send_axis_not_started_is_err() {
    let mut a = default_adapter();
    assert!(matches!(
        a.send_axis(0, 0.5),
        Err(WingmanError::NotStarted)
    ));
}

#[test]
fn send_axis_all_valid_indices() {
    let mut a = started_adapter();
    for i in 0u8..8 {
        assert!(a.send_axis(i, 0.0).is_ok(), "axis {i} should be valid");
    }
}

#[test]
fn send_axis_after_stop_is_err() {
    let mut a = started_adapter();
    a.stop();
    assert!(matches!(
        a.send_axis(0, 0.0),
        Err(WingmanError::NotStarted)
    ));
}

// ═══════════════════════════════════════════════════════════════════════════
// 4. Virtual controller — button behaviour
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn send_button_succeeds_when_started() {
    let mut a = started_adapter();
    assert!(a.send_button(0, true).is_ok());
}

#[test]
fn send_button_not_started_is_err() {
    let mut a = default_adapter();
    assert!(matches!(
        a.send_button(0, true),
        Err(WingmanError::NotStarted)
    ));
}

#[test]
fn send_button_all_valid_indices() {
    let mut a = started_adapter();
    for i in 0u8..32 {
        assert!(
            a.send_button(i, true).is_ok(),
            "button {i} should be valid"
        );
    }
}

#[test]
fn send_button_after_stop_is_err() {
    let mut a = started_adapter();
    a.stop();
    assert!(matches!(
        a.send_button(0, true),
        Err(WingmanError::NotStarted)
    ));
}

// ═══════════════════════════════════════════════════════════════════════════
// 5. StubVirtualController — axis edge cases
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn stub_axis_stores_exact_value() {
    let mut c = StubVirtualController::new();
    c.send_axis(2, 0.75).unwrap();
    assert!((c.axis(2).unwrap() - 0.75).abs() < f32::EPSILON);
}

#[test]
fn stub_axis_clamps_positive_overflow() {
    let mut c = StubVirtualController::new();
    c.send_axis(0, 1.5).unwrap();
    assert_eq!(c.axis(0).unwrap(), 1.0);
}

#[test]
fn stub_axis_clamps_negative_overflow() {
    let mut c = StubVirtualController::new();
    c.send_axis(0, -1.5).unwrap();
    assert_eq!(c.axis(0).unwrap(), -1.0);
}

#[test]
fn stub_axis_boundary_minus_one() {
    let mut c = StubVirtualController::new();
    c.send_axis(0, -1.0).unwrap();
    assert_eq!(c.axis(0).unwrap(), -1.0);
}

#[test]
fn stub_axis_boundary_plus_one() {
    let mut c = StubVirtualController::new();
    c.send_axis(0, 1.0).unwrap();
    assert_eq!(c.axis(0).unwrap(), 1.0);
}

#[test]
fn stub_axis_boundary_zero() {
    let mut c = StubVirtualController::new();
    c.send_axis(0, 0.5).unwrap();
    c.send_axis(0, 0.0).unwrap();
    assert_eq!(c.axis(0).unwrap(), 0.0);
}

#[test]
fn stub_axis_out_of_range_returns_error() {
    let mut c = StubVirtualController::new();
    assert_eq!(
        c.send_axis(8, 0.0),
        Err(VirtualControllerError::AxisOutOfRange(8))
    );
    assert_eq!(
        c.send_axis(255, 0.0),
        Err(VirtualControllerError::AxisOutOfRange(255))
    );
}

#[test]
fn stub_axis_readback_out_of_range_returns_none() {
    let c = StubVirtualController::new();
    assert_eq!(c.axis(8), None);
    assert_eq!(c.axis(255), None);
}

#[test]
fn stub_axis_nan_clamped() {
    let mut c = StubVirtualController::new();
    // f32::NAN.clamp(-1.0, 1.0) is implementation-defined but should not panic.
    let res = c.send_axis(0, f32::NAN);
    assert!(res.is_ok(), "send_axis with NAN should not return an error");
}

#[test]
fn stub_axis_infinity_clamped() {
    let mut c = StubVirtualController::new();
    c.send_axis(0, f32::INFINITY).unwrap();
    assert_eq!(c.axis(0).unwrap(), 1.0);
    c.send_axis(0, f32::NEG_INFINITY).unwrap();
    assert_eq!(c.axis(0).unwrap(), -1.0);
}

#[test]
fn stub_axes_are_independent() {
    let mut c = StubVirtualController::new();
    c.send_axis(0, 0.1).unwrap();
    c.send_axis(7, 0.9).unwrap();
    assert!((c.axis(0).unwrap() - 0.1).abs() < f32::EPSILON);
    assert!((c.axis(7).unwrap() - 0.9).abs() < f32::EPSILON);
    // Untouched axis stays at default.
    assert_eq!(c.axis(3).unwrap(), 0.0);
}

// ═══════════════════════════════════════════════════════════════════════════
// 6. StubVirtualController — button edge cases
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn stub_button_press_and_release() {
    let mut c = StubVirtualController::new();
    c.send_button(5, true).unwrap();
    assert_eq!(c.button_state(5), Some(true));
    c.send_button(5, false).unwrap();
    assert_eq!(c.button_state(5), Some(false));
}

#[test]
fn stub_button_out_of_range_returns_error() {
    let mut c = StubVirtualController::new();
    assert_eq!(
        c.send_button(32, true),
        Err(VirtualControllerError::ButtonOutOfRange(32))
    );
    assert_eq!(
        c.send_button(255, false),
        Err(VirtualControllerError::ButtonOutOfRange(255))
    );
}

#[test]
fn stub_button_readback_out_of_range_returns_none() {
    let c = StubVirtualController::new();
    assert_eq!(c.button_state(32), None);
    assert_eq!(c.button_state(255), None);
}

#[test]
fn stub_button_highest_valid_index() {
    let mut c = StubVirtualController::new();
    c.send_button(31, true).unwrap();
    assert_eq!(c.button_state(31), Some(true));
}

#[test]
fn stub_buttons_are_independent() {
    let mut c = StubVirtualController::new();
    c.send_button(0, true).unwrap();
    c.send_button(31, true).unwrap();
    assert_eq!(c.button_state(0), Some(true));
    assert_eq!(c.button_state(31), Some(true));
    assert_eq!(c.button_state(15), Some(false)); // untouched
}

#[test]
fn stub_button_idempotent_press() {
    let mut c = StubVirtualController::new();
    c.send_button(0, true).unwrap();
    c.send_button(0, true).unwrap();
    assert_eq!(c.button_state(0), Some(true));
}

#[test]
fn stub_button_idempotent_release() {
    let mut c = StubVirtualController::new();
    c.send_button(0, false).unwrap();
    c.send_button(0, false).unwrap();
    assert_eq!(c.button_state(0), Some(false));
}

// ═══════════════════════════════════════════════════════════════════════════
// 7. StubVirtualController — initial state
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn stub_initial_axes_all_zero() {
    let c = StubVirtualController::new();
    for i in 0u8..8 {
        assert_eq!(c.axis(i), Some(0.0), "axis {i} should init to 0.0");
    }
}

#[test]
fn stub_initial_buttons_all_released() {
    let c = StubVirtualController::new();
    for i in 0u8..32 {
        assert_eq!(
            c.button_state(i),
            Some(false),
            "button {i} should init to false"
        );
    }
}

#[test]
fn stub_default_equals_new() {
    let a = StubVirtualController::new();
    let b = StubVirtualController::default();
    for i in 0u8..8 {
        assert_eq!(a.axis(i), b.axis(i));
    }
    for i in 0u8..32 {
        assert_eq!(a.button_state(i), b.button_state(i));
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 8. Configuration
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn default_config_process_name() {
    assert_eq!(
        WingmanConfig::default().process_name,
        "ProjectWingman.exe"
    );
}

#[test]
fn default_config_poll_rate_positive() {
    assert!(WingmanConfig::default().poll_rate_hz > 0.0);
}

#[test]
fn default_config_bus_max_rate_positive() {
    assert!(WingmanConfig::default().bus_max_rate_hz > 0.0);
}

#[test]
fn custom_config_is_respected() {
    let cfg = WingmanConfig {
        process_name: "custom.exe".to_string(),
        poll_rate_hz: 25.0,
        bus_max_rate_hz: 5.0,
    };
    let a = WingmanAdapter::new(cfg);
    let interval = a.poll_interval();
    let expected_ms = 40u64; // 1000 / 25
    let actual_ms = interval.as_millis() as u64;
    assert!(
        (actual_ms as i64 - expected_ms as i64).abs() <= 2,
        "expected ~{expected_ms}ms, got {actual_ms}ms"
    );
}

#[test]
fn poll_interval_default_rate() {
    let a = default_adapter();
    let interval = a.poll_interval();
    let expected_ms = (1000.0 / WingmanConfig::default().poll_rate_hz) as u64;
    let actual_ms = interval.as_millis() as u64;
    assert!(
        (actual_ms as i64 - expected_ms as i64).abs() <= 2,
        "expected ~{expected_ms}ms, got {actual_ms}ms"
    );
}

#[test]
fn poll_interval_clamps_zero_rate_to_one_hz() {
    let cfg = WingmanConfig {
        poll_rate_hz: 0.0,
        ..Default::default()
    };
    let a = WingmanAdapter::new(cfg);
    let interval = a.poll_interval();
    assert_eq!(interval.as_secs(), 1);
}

#[test]
fn poll_interval_clamps_negative_rate_to_one_hz() {
    let cfg = WingmanConfig {
        poll_rate_hz: -10.0,
        ..Default::default()
    };
    let a = WingmanAdapter::new(cfg);
    let interval = a.poll_interval();
    assert_eq!(interval.as_secs(), 1);
}

#[test]
fn config_serialization_roundtrip() {
    let cfg = WingmanConfig {
        process_name: "test.exe".to_string(),
        poll_rate_hz: 42.0,
        bus_max_rate_hz: 7.0,
    };
    let json = serde_json::to_string(&cfg).unwrap();
    let cfg2: WingmanConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(cfg2.process_name, "test.exe");
    assert!((cfg2.poll_rate_hz - 42.0).abs() < f32::EPSILON);
    assert!((cfg2.bus_max_rate_hz - 7.0).abs() < f32::EPSILON);
}

// ═══════════════════════════════════════════════════════════════════════════
// 9. Metrics accounting
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn metrics_start_at_zero_updates() {
    let a = default_adapter();
    assert_eq!(a.metrics().total_updates, 0);
}

#[test]
fn poll_increments_metrics_total_updates() {
    let mut a = started_adapter();
    a.poll_once().unwrap();
    assert_eq!(a.metrics().total_updates, 1);
    a.poll_once().unwrap();
    assert_eq!(a.metrics().total_updates, 2);
}

#[test]
fn metrics_registry_records_adapter_updates() {
    let mut a = started_adapter();
    a.poll_once().unwrap();
    let snap = a.metrics_registry().snapshot();
    let counter = snap.iter().find(|m| {
        matches!(m, flight_metrics::Metric::Counter { name, .. }
            if name == flight_metrics::common::ADAPTER_UPDATES_TOTAL)
    });
    assert!(counter.is_some(), "ADAPTER_UPDATES_TOTAL should be recorded");
}

#[test]
fn metrics_registry_is_shared_ref() {
    let a = started_adapter();
    // Just verify we can obtain a reference without consuming the adapter.
    let _reg = a.metrics_registry();
    assert_eq!(a.state(), AdapterState::Connected);
}

// ═══════════════════════════════════════════════════════════════════════════
// 10. Error variant coverage
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn wingman_error_not_started_display() {
    let e = WingmanError::NotStarted;
    assert_eq!(format!("{e}"), "adapter is not started");
}

#[test]
fn virtual_controller_error_axis_display() {
    let e = VirtualControllerError::AxisOutOfRange(10);
    assert!(format!("{e}").contains("10"));
}

#[test]
fn virtual_controller_error_button_display() {
    let e = VirtualControllerError::ButtonOutOfRange(33);
    assert!(format!("{e}").contains("33"));
}

#[test]
fn virtual_controller_error_not_initialized_display() {
    let e = VirtualControllerError::NotInitialized;
    assert!(format!("{e}").contains("not initialized"));
}

#[test]
fn virtual_controller_error_eq() {
    assert_eq!(
        VirtualControllerError::AxisOutOfRange(5),
        VirtualControllerError::AxisOutOfRange(5)
    );
    assert_ne!(
        VirtualControllerError::AxisOutOfRange(5),
        VirtualControllerError::AxisOutOfRange(6)
    );
    assert_ne!(
        VirtualControllerError::AxisOutOfRange(5),
        VirtualControllerError::ButtonOutOfRange(5)
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// 11. Multiple adapter instances
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn two_adapters_have_independent_state() {
    let mut a1 = default_adapter();
    let mut a2 = default_adapter();
    a1.start();
    assert_eq!(a1.state(), AdapterState::Connected);
    assert_eq!(a2.state(), AdapterState::Disconnected);
    a2.start();
    a1.stop();
    assert_eq!(a1.state(), AdapterState::Disconnected);
    assert_eq!(a2.state(), AdapterState::Connected);
}

#[test]
fn two_adapters_produce_independent_snapshots() {
    let mut a1 = started_adapter();
    let mut a2 = started_adapter();
    let s1 = a1.poll_once().unwrap().unwrap();
    let s2 = a2.poll_once().unwrap().unwrap();
    // Both should be Wingman snapshots but potentially different timestamps.
    assert_eq!(s1.sim, s2.sim);
    assert_eq!(s1.aircraft, s2.aircraft);
}

// ═══════════════════════════════════════════════════════════════════════════
// 12. Property-based tests (proptest)
// ═══════════════════════════════════════════════════════════════════════════

mod proptest_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn axis_send_never_panics(index in 0u8..8, value in proptest::num::f32::ANY) {
            let mut c = StubVirtualController::new();
            // Should never panic regardless of input (NaN, Inf, subnormals, etc.).
            let _ = c.send_axis(index, value);
        }

        #[test]
        fn finite_axis_value_always_clamped(index in 0u8..8, value in -100.0f32..100.0) {
            let mut c = StubVirtualController::new();
            c.send_axis(index, value).unwrap();
            let stored = c.axis(index).unwrap();
            prop_assert!(stored >= -1.0 && stored <= 1.0,
                "stored value {stored} out of [-1, 1] for input {value}");
        }

        #[test]
        fn out_of_range_axis_always_errors(index in 8u8..=255, value in -2.0f32..2.0) {
            let mut c = StubVirtualController::new();
            let result = c.send_axis(index, value);
            prop_assert!(result.is_err());
        }

        #[test]
        fn out_of_range_button_always_errors(index in 32u8..=255, pressed: bool) {
            let mut c = StubVirtualController::new();
            let result = c.send_button(index, pressed);
            prop_assert!(result.is_err());
        }

        #[test]
        fn button_state_matches_last_write(index in 0u8..32, pressed: bool) {
            let mut c = StubVirtualController::new();
            c.send_button(index, pressed).unwrap();
            prop_assert_eq!(c.button_state(index), Some(pressed));
        }

        #[test]
        fn axis_read_matches_last_write_clamped(index in 0u8..8, value in -1.0f32..=1.0) {
            let mut c = StubVirtualController::new();
            c.send_axis(index, value).unwrap();
            let stored = c.axis(index).unwrap();
            prop_assert!((stored - value).abs() < f32::EPSILON,
                "expected {value}, got {stored}");
        }

        #[test]
        fn poll_interval_is_positive(rate in 0.001f32..1000.0) {
            let cfg = WingmanConfig {
                poll_rate_hz: rate,
                ..Default::default()
            };
            let a = WingmanAdapter::new(cfg);
            prop_assert!(a.poll_interval() > Duration::ZERO);
        }

        #[test]
        fn multiple_polls_always_succeed_when_started(n in 1usize..50) {
            let mut a = started_adapter();
            for _ in 0..n {
                prop_assert!(a.poll_once().is_ok());
            }
        }

        #[test]
        fn config_serde_roundtrip(
            name in "[a-zA-Z0-9_]{1,20}\\.exe",
            poll_hz in 0.1f32..500.0,
            bus_hz in 0.1f32..500.0,
        ) {
            let cfg = WingmanConfig {
                process_name: name.clone(),
                poll_rate_hz: poll_hz,
                bus_max_rate_hz: bus_hz,
            };
            let json = serde_json::to_string(&cfg).unwrap();
            let cfg2: WingmanConfig = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(cfg2.process_name, name);
            prop_assert!((cfg2.poll_rate_hz - poll_hz).abs() < 0.01);
            prop_assert!((cfg2.bus_max_rate_hz - bus_hz).abs() < 0.01);
        }
    }
}
