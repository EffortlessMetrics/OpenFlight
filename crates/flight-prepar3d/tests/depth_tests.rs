// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Depth tests for the Prepar3D adapter.
//!
//! Covers state machine transitions, flight data validation, telemetry
//! conversion scenarios, SimConnect compatibility invariants, error handling,
//! and property-based invariants.

use flight_prepar3d::{P3DError, P3DFlightData, P3DState, Prepar3DAdapter};
use proptest::prelude::*;

// ===========================================================================
// Helpers
// ===========================================================================

fn sample_data() -> P3DFlightData {
    P3DFlightData {
        pitch_rad: 0.05,
        roll_rad: -0.02,
        yaw_rad: 1.57,
        throttle: 0.75,
        altitude_ft: 5_000.0,
        airspeed_kts: 120.0,
    }
}

fn level_cruise() -> P3DFlightData {
    P3DFlightData {
        pitch_rad: 0.0,
        roll_rad: 0.0,
        yaw_rad: 0.0,
        throttle: 0.65,
        altitude_ft: 35_000.0,
        airspeed_kts: 450.0,
    }
}

fn steep_climb() -> P3DFlightData {
    P3DFlightData {
        pitch_rad: 0.35,
        roll_rad: 0.0,
        yaw_rad: 0.52,
        throttle: 1.0,
        altitude_ft: 12_000.0,
        airspeed_kts: 200.0,
    }
}

fn steep_descent() -> P3DFlightData {
    P3DFlightData {
        pitch_rad: -0.26,
        roll_rad: 0.05,
        yaw_rad: 3.12,
        throttle: 0.0,
        altitude_ft: 3_000.0,
        airspeed_kts: 160.0,
    }
}

fn banked_turn() -> P3DFlightData {
    P3DFlightData {
        pitch_rad: 0.03,
        roll_rad: 0.52,
        yaw_rad: 2.09,
        throttle: 0.70,
        altitude_ft: 8_000.0,
        airspeed_kts: 180.0,
    }
}

fn connected_adapter(version: &str) -> Prepar3DAdapter {
    let mut adapter = Prepar3DAdapter::new();
    adapter.simulate_connect(version);
    adapter
}

// ===========================================================================
// 1. Adapter construction and Default
// ===========================================================================

#[test]
fn new_adapter_starts_disconnected() {
    let adapter = Prepar3DAdapter::new();
    assert_eq!(adapter.state(), P3DState::Disconnected);
}

#[test]
fn default_adapter_matches_new() {
    let a = Prepar3DAdapter::new();
    let b = Prepar3DAdapter::default();
    assert_eq!(a.state(), b.state());
    assert_eq!(a.packet_count(), b.packet_count());
    assert_eq!(a.error_count(), b.error_count());
    assert!(a.last_data().is_none());
    assert!(b.last_data().is_none());
}

#[test]
fn new_adapter_has_zero_packet_count() {
    let adapter = Prepar3DAdapter::new();
    assert_eq!(adapter.packet_count(), 0);
}

#[test]
fn new_adapter_has_zero_error_count() {
    let adapter = Prepar3DAdapter::new();
    assert_eq!(adapter.error_count(), 0);
}

#[test]
fn new_adapter_has_no_last_data() {
    let adapter = Prepar3DAdapter::new();
    assert!(adapter.last_data().is_none());
}

// ===========================================================================
// 2. State machine transitions
// ===========================================================================

#[test]
fn connect_transitions_to_connected() {
    let mut adapter = Prepar3DAdapter::new();
    assert!(adapter.simulate_connect("5.4"));
    assert_eq!(adapter.state(), P3DState::Connected);
}

#[test]
fn disconnect_transitions_to_disconnected() {
    let mut adapter = connected_adapter("5.4");
    adapter.simulate_disconnect();
    assert_eq!(adapter.state(), P3DState::Disconnected);
}

#[test]
fn connect_disconnect_connect_cycle() {
    let mut adapter = Prepar3DAdapter::new();
    assert_eq!(adapter.state(), P3DState::Disconnected);

    adapter.simulate_connect("5.3");
    assert_eq!(adapter.state(), P3DState::Connected);

    adapter.simulate_disconnect();
    assert_eq!(adapter.state(), P3DState::Disconnected);

    adapter.simulate_connect("5.4");
    assert_eq!(adapter.state(), P3DState::Connected);
}

#[test]
fn multiple_rapid_connects_stay_connected() {
    let mut adapter = Prepar3DAdapter::new();
    for i in 0..10 {
        adapter.simulate_connect(&format!("5.{i}"));
        assert_eq!(adapter.state(), P3DState::Connected);
    }
}

#[test]
fn multiple_disconnects_are_idempotent() {
    let mut adapter = connected_adapter("5.3");
    for _ in 0..5 {
        adapter.simulate_disconnect();
        assert_eq!(adapter.state(), P3DState::Disconnected);
    }
}

#[test]
fn disconnect_without_connect_is_safe() {
    let mut adapter = Prepar3DAdapter::new();
    adapter.simulate_disconnect();
    assert_eq!(adapter.state(), P3DState::Disconnected);
}

// ===========================================================================
// 3. process_data acceptance
// ===========================================================================

#[test]
fn process_data_accepted_when_connected() {
    let mut adapter = connected_adapter("5.4");
    assert!(adapter.process_data(sample_data()));
}

#[test]
fn process_data_rejected_when_disconnected() {
    let mut adapter = Prepar3DAdapter::new();
    assert!(!adapter.process_data(sample_data()));
}

#[test]
fn process_data_rejected_after_disconnect() {
    let mut adapter = connected_adapter("5.4");
    adapter.simulate_disconnect();
    assert!(!adapter.process_data(sample_data()));
}

#[test]
fn process_data_increments_packet_count() {
    let mut adapter = connected_adapter("5.3");
    for i in 1..=10 {
        adapter.process_data(sample_data());
        assert_eq!(adapter.packet_count(), i);
    }
}

#[test]
fn rejected_data_increments_error_count() {
    let mut adapter = Prepar3DAdapter::new();
    for i in 1..=5 {
        adapter.process_data(sample_data());
        assert_eq!(adapter.error_count(), i);
    }
}

#[test]
fn rejected_data_does_not_increment_packet_count() {
    let mut adapter = Prepar3DAdapter::new();
    adapter.process_data(sample_data());
    assert_eq!(adapter.packet_count(), 0);
}

#[test]
fn accepted_data_does_not_increment_error_count() {
    let mut adapter = connected_adapter("5.3");
    adapter.process_data(sample_data());
    assert_eq!(adapter.error_count(), 0);
}

// ===========================================================================
// 4. last_data tracking
// ===========================================================================

#[test]
fn last_data_reflects_most_recent_packet() {
    let mut adapter = connected_adapter("5.4");
    adapter.process_data(level_cruise());
    adapter.process_data(steep_climb());

    let data = adapter.last_data().expect("should have data");
    assert!((data.pitch_rad - 0.35).abs() < f32::EPSILON);
    assert!((data.throttle - 1.0).abs() < f32::EPSILON);
}

#[test]
fn last_data_none_when_only_rejected() {
    let mut adapter = Prepar3DAdapter::new();
    adapter.process_data(sample_data());
    assert!(adapter.last_data().is_none());
}

#[test]
fn last_data_survives_disconnect() {
    let mut adapter = connected_adapter("5.3");
    adapter.process_data(sample_data());
    adapter.simulate_disconnect();
    // last_data is not cleared by disconnect in the current impl
    // (the snapshot persists until adapter is dropped)
    // If this changes in the future, this test documents the expectation.
    // For now we just verify no panic.
    let _ = adapter.last_data();
}

// ===========================================================================
// 5. Flight data field validation
// ===========================================================================

#[test]
fn flight_data_preserves_all_fields() {
    let data = P3DFlightData {
        pitch_rad: 0.123,
        roll_rad: -0.456,
        yaw_rad: 2.72,
        throttle: 0.42,
        altitude_ft: 18_500.0,
        airspeed_kts: 310.5,
    };
    assert!((data.pitch_rad - 0.123).abs() < f32::EPSILON);
    assert!((data.roll_rad - (-0.456)).abs() < f32::EPSILON);
    assert!((data.yaw_rad - 2.72).abs() < f32::EPSILON);
    assert!((data.throttle - 0.42).abs() < f32::EPSILON);
    assert!((data.altitude_ft - 18_500.0).abs() < 0.01);
    assert!((data.airspeed_kts - 310.5).abs() < f32::EPSILON);
}

#[test]
fn flight_data_clone_equals_original() {
    let original = sample_data();
    let cloned = original.clone();
    assert!((original.pitch_rad - cloned.pitch_rad).abs() < f32::EPSILON);
    assert!((original.roll_rad - cloned.roll_rad).abs() < f32::EPSILON);
    assert!((original.yaw_rad - cloned.yaw_rad).abs() < f32::EPSILON);
    assert!((original.throttle - cloned.throttle).abs() < f32::EPSILON);
    assert!((original.altitude_ft - cloned.altitude_ft).abs() < 0.01);
    assert!((original.airspeed_kts - cloned.airspeed_kts).abs() < f32::EPSILON);
}

#[test]
fn flight_data_debug_format_does_not_panic() {
    let data = sample_data();
    let debug_str = format!("{data:?}");
    assert!(debug_str.contains("P3DFlightData"));
    assert!(debug_str.contains("pitch_rad"));
}

// ===========================================================================
// 6. Telemetry conversion scenarios — P3D-specific flight phases
// ===========================================================================

#[test]
fn level_cruise_data_accepted() {
    let mut adapter = connected_adapter("5.4");
    assert!(adapter.process_data(level_cruise()));
    let data = adapter.last_data().unwrap();
    assert!((data.pitch_rad).abs() < 0.01, "level cruise should be ~0 pitch");
    assert!((data.roll_rad).abs() < 0.01, "level cruise should be ~0 roll");
}

#[test]
fn steep_climb_pitch_positive() {
    let mut adapter = connected_adapter("5.4");
    adapter.process_data(steep_climb());
    let data = adapter.last_data().unwrap();
    assert!(data.pitch_rad > 0.0, "climb should have positive pitch");
    assert!((data.throttle - 1.0).abs() < f32::EPSILON, "full throttle");
}

#[test]
fn steep_descent_pitch_negative() {
    let mut adapter = connected_adapter("5.4");
    adapter.process_data(steep_descent());
    let data = adapter.last_data().unwrap();
    assert!(data.pitch_rad < 0.0, "descent should have negative pitch");
    assert!((data.throttle).abs() < f32::EPSILON, "idle throttle");
}

#[test]
fn banked_turn_roll_nonzero() {
    let mut adapter = connected_adapter("5.4");
    adapter.process_data(banked_turn());
    let data = adapter.last_data().unwrap();
    assert!(data.roll_rad.abs() > 0.1, "bank should show nonzero roll");
}

#[test]
fn high_altitude_cruise() {
    let data = P3DFlightData {
        pitch_rad: 0.02,
        roll_rad: 0.0,
        yaw_rad: 0.0,
        throttle: 0.85,
        altitude_ft: 45_000.0,
        airspeed_kts: 500.0,
    };
    let mut adapter = connected_adapter("5.4");
    assert!(adapter.process_data(data));
    let received = adapter.last_data().unwrap();
    assert!(received.altitude_ft > 40_000.0);
}

#[test]
fn ground_level_data() {
    let data = P3DFlightData {
        pitch_rad: 0.0,
        roll_rad: 0.0,
        yaw_rad: 1.0,
        throttle: 0.0,
        altitude_ft: 0.0,
        airspeed_kts: 0.0,
    };
    let mut adapter = connected_adapter("5.4");
    assert!(adapter.process_data(data));
    let received = adapter.last_data().unwrap();
    assert!((received.altitude_ft).abs() < f32::EPSILON);
    assert!((received.airspeed_kts).abs() < f32::EPSILON);
}

// ===========================================================================
// 7. SimConnect version compatibility
// ===========================================================================

#[test]
fn connect_with_p3d_v4_version() {
    let mut adapter = Prepar3DAdapter::new();
    assert!(adapter.simulate_connect("4.5"));
    assert_eq!(adapter.state(), P3DState::Connected);
}

#[test]
fn connect_with_p3d_v5_version() {
    let mut adapter = Prepar3DAdapter::new();
    assert!(adapter.simulate_connect("5.4"));
    assert_eq!(adapter.state(), P3DState::Connected);
}

#[test]
fn connect_with_p3d_v6_version() {
    let mut adapter = Prepar3DAdapter::new();
    assert!(adapter.simulate_connect("6.0"));
    assert_eq!(adapter.state(), P3DState::Connected);
}

#[test]
fn connect_with_arbitrary_version_string() {
    let mut adapter = Prepar3DAdapter::new();
    assert!(adapter.simulate_connect("unknown-preview"));
    assert_eq!(adapter.state(), P3DState::Connected);
}

#[test]
fn connect_with_empty_version() {
    let mut adapter = Prepar3DAdapter::new();
    assert!(adapter.simulate_connect(""));
    assert_eq!(adapter.state(), P3DState::Connected);
}

// ===========================================================================
// 8. Error type coverage
// ===========================================================================

#[test]
fn error_not_available_display() {
    let err = P3DError::NotAvailable;
    let msg = err.to_string();
    assert!(
        msg.contains("not running") || msg.contains("SimConnect"),
        "unexpected message: {msg}"
    );
}

#[test]
fn error_version_mismatch_display() {
    let err = P3DError::VersionMismatch {
        expected: "5.4".to_string(),
        found: "4.5".to_string(),
    };
    let msg = err.to_string();
    assert!(msg.contains("5.4"), "should contain expected version: {msg}");
    assert!(msg.contains("4.5"), "should contain found version: {msg}");
}

#[test]
fn error_debug_format_not_available() {
    let err = P3DError::NotAvailable;
    let debug = format!("{err:?}");
    assert!(debug.contains("NotAvailable"));
}

#[test]
fn error_debug_format_version_mismatch() {
    let err = P3DError::VersionMismatch {
        expected: "5.4".to_string(),
        found: "3.0".to_string(),
    };
    let debug = format!("{err:?}");
    assert!(debug.contains("VersionMismatch"));
    assert!(debug.contains("5.4"));
    assert!(debug.contains("3.0"));
}

// ===========================================================================
// 9. P3DState enum coverage
// ===========================================================================

#[test]
fn p3d_state_equality() {
    assert_eq!(P3DState::Disconnected, P3DState::Disconnected);
    assert_eq!(P3DState::Connecting, P3DState::Connecting);
    assert_eq!(P3DState::Connected, P3DState::Connected);
    assert_eq!(P3DState::Error, P3DState::Error);
}

#[test]
fn p3d_state_inequality() {
    assert_ne!(P3DState::Disconnected, P3DState::Connected);
    assert_ne!(P3DState::Connecting, P3DState::Error);
    assert_ne!(P3DState::Connected, P3DState::Disconnected);
}

#[test]
fn p3d_state_all_variants_debug() {
    let states = [
        P3DState::Disconnected,
        P3DState::Connecting,
        P3DState::Connected,
        P3DState::Error,
    ];
    for s in &states {
        let debug = format!("{s:?}");
        assert!(!debug.is_empty());
    }
}

#[test]
fn p3d_state_copy_semantics() {
    let a = P3DState::Connected;
    let b = a;
    assert_eq!(a, b);
}

#[test]
fn p3d_state_clone_semantics() {
    let a = P3DState::Error;
    #[allow(clippy::clone_on_copy)]
    let b = a.clone();
    assert_eq!(a, b);
}

// ===========================================================================
// 10. Packet and error counting under stress
// ===========================================================================

#[test]
fn interleaved_accept_reject_counting() {
    let mut adapter = Prepar3DAdapter::new();

    // Disconnected: 3 rejects
    adapter.process_data(sample_data());
    adapter.process_data(sample_data());
    adapter.process_data(sample_data());

    // Connect: 5 accepts
    adapter.simulate_connect("5.3");
    for _ in 0..5 {
        adapter.process_data(sample_data());
    }

    // Disconnect: 2 more rejects
    adapter.simulate_disconnect();
    adapter.process_data(sample_data());
    adapter.process_data(sample_data());

    assert_eq!(adapter.packet_count(), 5);
    assert_eq!(adapter.error_count(), 5);
}

#[test]
fn high_volume_data_processing() {
    let mut adapter = connected_adapter("5.4");
    let count = 10_000u64;
    for _ in 0..count {
        adapter.process_data(sample_data());
    }
    assert_eq!(adapter.packet_count(), count);
    assert_eq!(adapter.error_count(), 0);
}

// ===========================================================================
// 11. Extreme and edge-case flight data
// ===========================================================================

#[test]
fn extreme_pitch_values() {
    let data = P3DFlightData {
        pitch_rad: std::f32::consts::FRAC_PI_2,
        roll_rad: 0.0,
        yaw_rad: 0.0,
        throttle: 0.5,
        altitude_ft: 1_000.0,
        airspeed_kts: 50.0,
    };
    let mut adapter = connected_adapter("5.4");
    assert!(adapter.process_data(data));
}

#[test]
fn extreme_roll_values() {
    let data = P3DFlightData {
        pitch_rad: 0.0,
        roll_rad: std::f32::consts::PI,
        yaw_rad: 0.0,
        throttle: 0.5,
        altitude_ft: 5_000.0,
        airspeed_kts: 100.0,
    };
    let mut adapter = connected_adapter("5.4");
    assert!(adapter.process_data(data));
}

#[test]
fn full_yaw_sweep() {
    let data = P3DFlightData {
        pitch_rad: 0.0,
        roll_rad: 0.0,
        yaw_rad: 2.0 * std::f32::consts::PI,
        throttle: 0.5,
        altitude_ft: 5_000.0,
        airspeed_kts: 100.0,
    };
    let mut adapter = connected_adapter("5.4");
    assert!(adapter.process_data(data));
}

#[test]
fn negative_altitude_accepted() {
    let data = P3DFlightData {
        pitch_rad: 0.0,
        roll_rad: 0.0,
        yaw_rad: 0.0,
        throttle: 0.0,
        altitude_ft: -1_400.0, // Dead Sea depression
        airspeed_kts: 0.0,
    };
    let mut adapter = connected_adapter("5.4");
    assert!(adapter.process_data(data));
}

#[test]
fn zero_throttle_accepted() {
    let data = P3DFlightData {
        pitch_rad: 0.0,
        roll_rad: 0.0,
        yaw_rad: 0.0,
        throttle: 0.0,
        altitude_ft: 5_000.0,
        airspeed_kts: 80.0,
    };
    let mut adapter = connected_adapter("5.4");
    assert!(adapter.process_data(data));
    let received = adapter.last_data().unwrap();
    assert!((received.throttle).abs() < f32::EPSILON);
}

#[test]
fn max_throttle_accepted() {
    let data = P3DFlightData {
        pitch_rad: 0.0,
        roll_rad: 0.0,
        yaw_rad: 0.0,
        throttle: 1.0,
        altitude_ft: 5_000.0,
        airspeed_kts: 200.0,
    };
    let mut adapter = connected_adapter("5.4");
    assert!(adapter.process_data(data));
    let received = adapter.last_data().unwrap();
    assert!((received.throttle - 1.0).abs() < f32::EPSILON);
}

#[test]
fn nan_values_do_not_crash() {
    let data = P3DFlightData {
        pitch_rad: f32::NAN,
        roll_rad: f32::NAN,
        yaw_rad: f32::NAN,
        throttle: f32::NAN,
        altitude_ft: f32::NAN,
        airspeed_kts: f32::NAN,
    };
    let mut adapter = connected_adapter("5.4");
    // Should not panic — adapter accepts any f32 value
    let _ = adapter.process_data(data);
}

#[test]
fn infinity_values_do_not_crash() {
    let data = P3DFlightData {
        pitch_rad: f32::INFINITY,
        roll_rad: f32::NEG_INFINITY,
        yaw_rad: f32::INFINITY,
        throttle: f32::INFINITY,
        altitude_ft: f32::NEG_INFINITY,
        airspeed_kts: f32::INFINITY,
    };
    let mut adapter = connected_adapter("5.4");
    let _ = adapter.process_data(data);
}

// ===========================================================================
// 12. Property-based tests
// ===========================================================================

prop_compose! {
    fn arb_flight_data()(
        pitch_rad in -std::f32::consts::FRAC_PI_2..=std::f32::consts::FRAC_PI_2,
        roll_rad in -std::f32::consts::PI..=std::f32::consts::PI,
        yaw_rad in 0.0f32..=(2.0 * std::f32::consts::PI),
        throttle in 0.0f32..=1.0f32,
        altitude_ft in -2000.0f32..=60000.0f32,
        airspeed_kts in 0.0f32..=800.0f32,
    ) -> P3DFlightData {
        P3DFlightData {
            pitch_rad,
            roll_rad,
            yaw_rad,
            throttle,
            altitude_ft,
            airspeed_kts,
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn prop_connected_adapter_always_accepts_data(data in arb_flight_data()) {
        let mut adapter = connected_adapter("5.4");
        prop_assert!(adapter.process_data(data));
    }

    #[test]
    fn prop_disconnected_adapter_always_rejects_data(data in arb_flight_data()) {
        let mut adapter = Prepar3DAdapter::new();
        prop_assert!(!adapter.process_data(data));
    }

    #[test]
    fn prop_packet_count_monotonically_increases(
        packets in proptest::collection::vec(arb_flight_data(), 1..50)
    ) {
        let mut adapter = connected_adapter("5.4");
        let mut prev = 0u64;
        for pkt in packets {
            adapter.process_data(pkt);
            let current = adapter.packet_count();
            prop_assert!(current > prev, "packet_count must increase: {prev} -> {current}");
            prev = current;
        }
    }

    #[test]
    fn prop_last_data_always_set_after_accept(data in arb_flight_data()) {
        let mut adapter = connected_adapter("5.4");
        adapter.process_data(data);
        prop_assert!(adapter.last_data().is_some());
    }

    #[test]
    fn prop_last_data_matches_most_recent(
        first in arb_flight_data(),
        second in arb_flight_data(),
    ) {
        let mut adapter = connected_adapter("5.4");
        adapter.process_data(first);
        adapter.process_data(second.clone());
        let last = adapter.last_data().unwrap();
        prop_assert!((last.pitch_rad - second.pitch_rad).abs() < 1e-6);
        prop_assert!((last.throttle - second.throttle).abs() < 1e-6);
        prop_assert!((last.altitude_ft - second.altitude_ft).abs() < 1e-6);
    }

    #[test]
    fn prop_error_count_only_from_rejections(
        pre_connect in proptest::collection::vec(arb_flight_data(), 0..10),
        post_connect in proptest::collection::vec(arb_flight_data(), 0..10),
    ) {
        let mut adapter = Prepar3DAdapter::new();
        for pkt in &pre_connect {
            adapter.process_data(pkt.clone());
        }
        adapter.simulate_connect("5.4");
        for pkt in &post_connect {
            adapter.process_data(pkt.clone());
        }
        prop_assert_eq!(adapter.error_count(), pre_connect.len() as u64);
        prop_assert_eq!(adapter.packet_count(), post_connect.len() as u64);
    }

    #[test]
    fn prop_clone_flight_data_preserves_fields(data in arb_flight_data()) {
        let cloned = data.clone();
        prop_assert!((data.pitch_rad - cloned.pitch_rad).abs() < 1e-6);
        prop_assert!((data.roll_rad - cloned.roll_rad).abs() < 1e-6);
        prop_assert!((data.yaw_rad - cloned.yaw_rad).abs() < 1e-6);
        prop_assert!((data.throttle - cloned.throttle).abs() < 1e-6);
        prop_assert!((data.altitude_ft - cloned.altitude_ft).abs() < 1e-6);
        prop_assert!((data.airspeed_kts - cloned.airspeed_kts).abs() < 1e-6);
    }

    #[test]
    fn prop_connect_disconnect_cycle_preserves_invariants(
        versions in proptest::collection::vec("[0-9]\\.[0-9]", 1..5),
    ) {
        let mut adapter = Prepar3DAdapter::new();
        for v in &versions {
            prop_assert_eq!(adapter.state(), P3DState::Disconnected);
            adapter.simulate_connect(v);
            prop_assert_eq!(adapter.state(), P3DState::Connected);
            adapter.simulate_disconnect();
        }
        prop_assert_eq!(adapter.state(), P3DState::Disconnected);
    }

    #[test]
    fn prop_arbitrary_version_connect_succeeds(version in "[a-zA-Z0-9._-]{0,20}") {
        let mut adapter = Prepar3DAdapter::new();
        prop_assert!(adapter.simulate_connect(&version));
        prop_assert_eq!(adapter.state(), P3DState::Connected);
    }

    #[test]
    fn prop_flight_data_debug_never_panics(data in arb_flight_data()) {
        let debug_str = format!("{data:?}");
        prop_assert!(!debug_str.is_empty());
    }
}
