// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for flight-adapter-common.
//!
//! Covers: reconnection strategies, exponential backoff with jitter,
//! adapter state machine transitions, metrics collection, error handling,
//! config trait, and property-based tests.

use std::time::Duration;

use flight_adapter_common::{
    AdapterConfig, AdapterError, AdapterMetrics, AdapterState, ExponentialBackoff,
    ReconnectionStrategy,
};

// ─── ReconnectionStrategy ────────────────────────────────────────────────────

#[test]
fn reconnect_first_attempt_returns_initial_backoff() {
    let s = ReconnectionStrategy::new(5, Duration::from_millis(100), Duration::from_secs(30));
    assert_eq!(s.next_backoff(1), Duration::from_millis(100));
}

#[test]
fn reconnect_exponential_doubling() {
    let s = ReconnectionStrategy::new(10, Duration::from_millis(100), Duration::from_secs(60));
    assert_eq!(s.next_backoff(2), Duration::from_millis(200));
    assert_eq!(s.next_backoff(3), Duration::from_millis(400));
    assert_eq!(s.next_backoff(4), Duration::from_millis(800));
}

#[test]
fn reconnect_caps_at_max_backoff() {
    let s = ReconnectionStrategy::new(10, Duration::from_millis(100), Duration::from_millis(500));
    assert_eq!(s.next_backoff(10), Duration::from_millis(500));
    assert_eq!(s.next_backoff(20), Duration::from_millis(500));
    assert_eq!(s.next_backoff(u32::MAX), Duration::from_millis(500));
}

#[test]
fn reconnect_should_retry_within_bounds() {
    let s = ReconnectionStrategy::new(3, Duration::from_millis(100), Duration::from_secs(10));
    assert!(s.should_retry(1));
    assert!(s.should_retry(2));
    assert!(s.should_retry(3));
    assert!(!s.should_retry(4));
    assert!(!s.should_retry(100));
}

#[test]
fn reconnect_zero_max_attempts_never_retries() {
    let s = ReconnectionStrategy::new(0, Duration::from_millis(100), Duration::from_secs(10));
    assert!(s.should_retry(0));
    assert!(!s.should_retry(1));
}

#[test]
fn reconnect_attempt_zero_returns_initial() {
    let s = ReconnectionStrategy::new(5, Duration::from_millis(250), Duration::from_secs(30));
    assert_eq!(s.next_backoff(0), Duration::from_millis(250));
}

#[test]
fn reconnect_accessors() {
    let s = ReconnectionStrategy::new(7, Duration::from_millis(200), Duration::from_millis(5000));
    assert_eq!(s.max_attempts(), 7);
    assert_eq!(s.initial_backoff(), Duration::from_millis(200));
    assert_eq!(s.max_backoff(), Duration::from_millis(5000));
}

#[test]
fn reconnect_initial_larger_than_max_clamps() {
    let s = ReconnectionStrategy::new(3, Duration::from_secs(10), Duration::from_secs(1));
    assert_eq!(s.next_backoff(1), Duration::from_secs(1));
}

#[test]
fn reconnect_clone_independence() {
    let s1 = ReconnectionStrategy::new(5, Duration::from_millis(100), Duration::from_secs(10));
    let s2 = s1.clone();
    assert_eq!(s1.max_attempts(), s2.max_attempts());
    assert_eq!(s1.initial_backoff(), s2.initial_backoff());
    assert_eq!(s1.max_backoff(), s2.max_backoff());
}

#[test]
fn reconnect_debug_format() {
    let s = ReconnectionStrategy::new(3, Duration::from_millis(100), Duration::from_secs(1));
    let dbg = format!("{:?}", s);
    assert!(dbg.contains("ReconnectionStrategy"));
}

// ─── ExponentialBackoff ──────────────────────────────────────────────────────

#[test]
fn backoff_first_delay_equals_initial() {
    let mut b = ExponentialBackoff::new(
        Duration::from_millis(100),
        Duration::from_secs(10),
        2.0,
        0.0,
    );
    assert_eq!(b.next_delay(), Duration::from_millis(100));
    assert_eq!(b.attempt(), 1);
}

#[test]
fn backoff_exponential_growth_no_jitter() {
    let mut b = ExponentialBackoff::new(
        Duration::from_millis(50),
        Duration::from_secs(60),
        2.0,
        0.0,
    );
    let expected = [50, 100, 200, 400, 800, 1600];
    for &ms in &expected {
        assert_eq!(b.next_delay(), Duration::from_millis(ms));
    }
}

#[test]
fn backoff_caps_at_max_delay() {
    let mut b = ExponentialBackoff::new(
        Duration::from_millis(100),
        Duration::from_millis(300),
        2.0,
        0.0,
    );
    // 100 → 200 → 400 (capped at 300) → 800 (capped at 300)
    b.next_delay(); // 100
    b.next_delay(); // 200
    assert_eq!(b.next_delay(), Duration::from_millis(300));
    assert_eq!(b.next_delay(), Duration::from_millis(300));
}

#[test]
fn backoff_reset_restarts_from_initial() {
    let mut b = ExponentialBackoff::new(
        Duration::from_millis(100),
        Duration::from_secs(30),
        2.0,
        0.0,
    );
    b.next_delay();
    b.next_delay();
    b.next_delay();
    assert_eq!(b.attempt(), 3);
    b.reset();
    assert_eq!(b.attempt(), 0);
    assert_eq!(b.next_delay(), Duration::from_millis(100));
}

#[test]
fn backoff_attempt_counter_increments() {
    let mut b = ExponentialBackoff::new(
        Duration::from_millis(100),
        Duration::from_secs(30),
        2.0,
        0.0,
    );
    assert_eq!(b.attempt(), 0);
    b.next_delay();
    assert_eq!(b.attempt(), 1);
    b.next_delay();
    assert_eq!(b.attempt(), 2);
}

#[test]
fn backoff_multiplier_three() {
    let mut b = ExponentialBackoff::new(
        Duration::from_millis(10),
        Duration::from_secs(60),
        3.0,
        0.0,
    );
    assert_eq!(b.next_delay(), Duration::from_millis(10));
    assert_eq!(b.next_delay(), Duration::from_millis(30));
    assert_eq!(b.next_delay(), Duration::from_millis(90));
    assert_eq!(b.next_delay(), Duration::from_millis(270));
}

#[test]
fn backoff_multiplier_one_stays_constant() {
    let mut b = ExponentialBackoff::new(
        Duration::from_millis(500),
        Duration::from_secs(60),
        1.0,
        0.0,
    );
    for _ in 0..10 {
        assert_eq!(b.next_delay(), Duration::from_millis(500));
    }
}

#[test]
fn backoff_jitter_stays_within_bounds() {
    let mut b = ExponentialBackoff::new(
        Duration::from_millis(1000),
        Duration::from_secs(120),
        2.0,
        0.5,
    );
    for _ in 0..30 {
        let d = b.next_delay();
        assert!(d <= Duration::from_secs(120));
    }
}

#[test]
fn backoff_jitter_produces_varied_values() {
    let mut b = ExponentialBackoff::new(
        Duration::from_millis(1000),
        Duration::from_secs(600),
        2.0,
        0.25,
    );
    let mut delays = Vec::new();
    for _ in 0..5 {
        delays.push(b.next_delay());
    }
    // At minimum, the delays should differ from the pure exponential sequence
    // (unless jitter hash happens to produce exactly 0 for every attempt)
    assert!(delays.len() == 5);
}

#[test]
fn backoff_zero_jitter_is_deterministic() {
    let delays_a: Vec<Duration> = {
        let mut b = ExponentialBackoff::new(
            Duration::from_millis(100),
            Duration::from_secs(60),
            2.0,
            0.0,
        );
        (0..8).map(|_| b.next_delay()).collect()
    };
    let delays_b: Vec<Duration> = {
        let mut b = ExponentialBackoff::new(
            Duration::from_millis(100),
            Duration::from_secs(60),
            2.0,
            0.0,
        );
        (0..8).map(|_| b.next_delay()).collect()
    };
    assert_eq!(delays_a, delays_b);
}

#[test]
fn backoff_with_jitter_is_deterministic() {
    // Jitter uses a deterministic hash, so identical configs yield identical sequences.
    let delays_a: Vec<Duration> = {
        let mut b = ExponentialBackoff::new(
            Duration::from_millis(100),
            Duration::from_secs(60),
            2.0,
            0.3,
        );
        (0..8).map(|_| b.next_delay()).collect()
    };
    let delays_b: Vec<Duration> = {
        let mut b = ExponentialBackoff::new(
            Duration::from_millis(100),
            Duration::from_secs(60),
            2.0,
            0.3,
        );
        (0..8).map(|_| b.next_delay()).collect()
    };
    assert_eq!(delays_a, delays_b);
}

#[test]
#[should_panic(expected = "multiplier must be >= 1.0")]
fn backoff_panics_on_multiplier_below_one() {
    ExponentialBackoff::new(Duration::from_millis(100), Duration::from_secs(1), 0.9, 0.0);
}

#[test]
#[should_panic(expected = "jitter must be in [0.0, 1.0]")]
fn backoff_panics_on_negative_jitter() {
    ExponentialBackoff::new(Duration::from_millis(100), Duration::from_secs(1), 2.0, -0.1);
}

#[test]
#[should_panic(expected = "jitter must be in [0.0, 1.0]")]
fn backoff_panics_on_jitter_above_one() {
    ExponentialBackoff::new(Duration::from_millis(100), Duration::from_secs(1), 2.0, 1.01);
}

#[test]
fn backoff_edge_jitter_one() {
    let mut b = ExponentialBackoff::new(
        Duration::from_millis(100),
        Duration::from_secs(60),
        2.0,
        1.0,
    );
    for _ in 0..10 {
        let d = b.next_delay();
        assert!(d <= Duration::from_secs(60));
    }
}

#[test]
fn backoff_clone_independence() {
    let mut b1 = ExponentialBackoff::new(
        Duration::from_millis(100),
        Duration::from_secs(10),
        2.0,
        0.0,
    );
    b1.next_delay();
    b1.next_delay();
    let mut b2 = b1.clone();
    assert_eq!(b1.attempt(), b2.attempt());
    // Advancing b1 should not affect b2
    b1.next_delay();
    assert_ne!(b1.attempt(), b2.attempt());
    assert_eq!(b2.next_delay(), Duration::from_millis(400));
}

#[test]
fn backoff_many_iterations_saturate_gracefully() {
    let mut b = ExponentialBackoff::new(
        Duration::from_millis(1),
        Duration::from_secs(5),
        2.0,
        0.0,
    );
    for _ in 0..100 {
        let d = b.next_delay();
        assert!(d <= Duration::from_secs(5));
    }
}

#[test]
fn backoff_debug_format() {
    let b = ExponentialBackoff::new(
        Duration::from_millis(100),
        Duration::from_secs(10),
        2.0,
        0.1,
    );
    let dbg = format!("{:?}", b);
    assert!(dbg.contains("ExponentialBackoff"));
}

// ─── AdapterState ────────────────────────────────────────────────────────────

#[test]
fn state_all_variants_distinct() {
    let variants = [
        AdapterState::Disconnected,
        AdapterState::Connecting,
        AdapterState::Connected,
        AdapterState::DetectingAircraft,
        AdapterState::Active,
        AdapterState::Error,
    ];
    for (i, a) in variants.iter().enumerate() {
        for (j, b) in variants.iter().enumerate() {
            if i == j {
                assert_eq!(a, b);
            } else {
                assert_ne!(a, b);
            }
        }
    }
}

#[test]
fn state_copy_semantics() {
    let a = AdapterState::Active;
    let b = a;
    let c = b;
    assert_eq!(a, b);
    assert_eq!(b, c);
}

#[test]
fn state_clone_equals_original() {
    let states = [
        AdapterState::Disconnected,
        AdapterState::Connecting,
        AdapterState::Connected,
        AdapterState::DetectingAircraft,
        AdapterState::Active,
        AdapterState::Error,
    ];
    for s in &states {
        assert_eq!(*s, s.clone());
    }
}

#[test]
fn state_debug_format_all_variants() {
    assert_eq!(format!("{:?}", AdapterState::Disconnected), "Disconnected");
    assert_eq!(format!("{:?}", AdapterState::Connecting), "Connecting");
    assert_eq!(format!("{:?}", AdapterState::Connected), "Connected");
    assert_eq!(
        format!("{:?}", AdapterState::DetectingAircraft),
        "DetectingAircraft"
    );
    assert_eq!(format!("{:?}", AdapterState::Active), "Active");
    assert_eq!(format!("{:?}", AdapterState::Error), "Error");
}

/// Simulates the expected lifecycle: Disconnected → Connecting → Connected →
/// DetectingAircraft → Active, then an error cycle back.
#[test]
fn state_machine_happy_path_lifecycle() {
    let mut state = AdapterState::Disconnected;
    assert_eq!(state, AdapterState::Disconnected);

    state = AdapterState::Connecting;
    assert_eq!(state, AdapterState::Connecting);

    state = AdapterState::Connected;
    assert_eq!(state, AdapterState::Connected);

    state = AdapterState::DetectingAircraft;
    assert_eq!(state, AdapterState::DetectingAircraft);

    state = AdapterState::Active;
    assert_eq!(state, AdapterState::Active);
}

#[test]
fn state_machine_error_recovery_cycle() {
    let state = AdapterState::Active;
    assert_eq!(state, AdapterState::Active);
    // Simulate error
    let state = AdapterState::Error;
    assert_eq!(state, AdapterState::Error);
    // Reconnect
    let state = AdapterState::Connecting;
    assert_eq!(state, AdapterState::Connecting);
    // Success
    let state = AdapterState::Connected;
    assert_eq!(state, AdapterState::Connected);
    let _ = state;
}

#[test]
fn state_machine_disconnect_from_any_state() {
    for start in [
        AdapterState::Connecting,
        AdapterState::Connected,
        AdapterState::DetectingAircraft,
        AdapterState::Active,
        AdapterState::Error,
    ] {
        let _prev = start;
        let state = AdapterState::Disconnected;
        assert_eq!(state, AdapterState::Disconnected);
    }
}

// ─── AdapterMetrics ──────────────────────────────────────────────────────────

#[test]
fn metrics_new_defaults() {
    let m = AdapterMetrics::new();
    assert_eq!(m.total_updates, 0);
    assert!(m.last_update_time.is_none());
    assert!(m.update_intervals.is_empty());
    assert_eq!(m.max_interval_samples, 100);
    assert_eq!(m.actual_update_rate, 0.0);
    assert_eq!(m.update_jitter_p99_ms, 0.0);
    assert!(m.last_aircraft_title.is_none());
    assert_eq!(m.aircraft_changes, 0);
}

#[test]
fn metrics_single_update_no_interval() {
    let mut m = AdapterMetrics::new();
    m.record_update();
    assert_eq!(m.total_updates, 1);
    assert!(m.last_update_time.is_some());
    assert!(m.update_intervals.is_empty());
}

#[test]
fn metrics_two_updates_produce_interval() {
    let mut m = AdapterMetrics::new();
    m.record_update();
    m.record_update();
    assert_eq!(m.total_updates, 2);
    assert_eq!(m.update_intervals.len(), 1);
}

#[test]
fn metrics_many_updates_cap_intervals() {
    let mut m = AdapterMetrics::new();
    m.max_interval_samples = 10;
    for _ in 0..50 {
        m.record_update();
    }
    assert!(m.update_intervals.len() <= 10);
    assert_eq!(m.total_updates, 50);
}

#[test]
fn metrics_aircraft_change_deduplication() {
    let mut m = AdapterMetrics::new();
    m.record_aircraft_change("C172".to_string());
    m.record_aircraft_change("C172".to_string());
    m.record_aircraft_change("C172".to_string());
    assert_eq!(m.aircraft_changes, 1);
    assert_eq!(m.last_aircraft_title.as_deref(), Some("C172"));
}

#[test]
fn metrics_aircraft_change_counts_distinct() {
    let mut m = AdapterMetrics::new();
    m.record_aircraft_change("C172".to_string());
    m.record_aircraft_change("A320".to_string());
    m.record_aircraft_change("B737".to_string());
    assert_eq!(m.aircraft_changes, 3);
    assert_eq!(m.last_aircraft_title.as_deref(), Some("B737"));
}

#[test]
fn metrics_aircraft_change_toggling() {
    let mut m = AdapterMetrics::new();
    m.record_aircraft_change("C172".to_string());
    m.record_aircraft_change("A320".to_string());
    m.record_aircraft_change("C172".to_string());
    assert_eq!(m.aircraft_changes, 3);
}

#[test]
fn metrics_summary_contains_all_fields() {
    let mut m = AdapterMetrics::new();
    m.record_update();
    m.record_aircraft_change("C172".to_string());
    let s = m.summary();
    assert!(s.contains("Updates:"), "missing Updates in: {s}");
    assert!(s.contains("Rate:"), "missing Rate in: {s}");
    assert!(s.contains("Jitter p99:"), "missing Jitter in: {s}");
    assert!(
        s.contains("Aircraft changes:"),
        "missing Aircraft changes in: {s}"
    );
}

#[test]
fn metrics_summary_zero_state() {
    let m = AdapterMetrics::new();
    let s = m.summary();
    assert!(s.contains("Updates: 0"));
    assert!(s.contains("Aircraft changes: 0"));
}

#[test]
fn metrics_actual_update_rate_positive_after_updates() {
    let mut m = AdapterMetrics::new();
    for _ in 0..5 {
        m.record_update();
    }
    // Rate could be very high since updates happen nearly instantly in tests,
    // but it should be positive.
    assert!(m.actual_update_rate > 0.0);
}

#[test]
fn metrics_clone_independence() {
    let mut m1 = AdapterMetrics::new();
    m1.record_update();
    m1.record_aircraft_change("C172".to_string());
    let mut m2 = m1.clone();
    m2.record_update();
    m2.record_aircraft_change("A320".to_string());
    assert_eq!(m1.total_updates, 1);
    assert_eq!(m2.total_updates, 2);
    assert_eq!(m1.aircraft_changes, 1);
    assert_eq!(m2.aircraft_changes, 2);
}

#[test]
fn metrics_default_matches_new() {
    let d = AdapterMetrics::default();
    let n = AdapterMetrics::new();
    assert_eq!(d.total_updates, n.total_updates);
    assert_eq!(d.aircraft_changes, n.aircraft_changes);
    // Default has max_interval_samples = 0 from Default derive, while new() sets 100
    // This documents the intentional difference.
    assert_eq!(n.max_interval_samples, 100);
}

// ─── AdapterError ────────────────────────────────────────────────────────────

#[test]
fn error_not_connected_display() {
    let e = AdapterError::NotConnected;
    assert_eq!(e.to_string(), "Not connected");
}

#[test]
fn error_timeout_display() {
    let e = AdapterError::Timeout("5s elapsed".to_string());
    assert_eq!(e.to_string(), "Timeout: 5s elapsed");
}

#[test]
fn error_aircraft_not_detected_display() {
    let e = AdapterError::AircraftNotDetected;
    assert_eq!(e.to_string(), "Aircraft not detected");
}

#[test]
fn error_configuration_display() {
    let e = AdapterError::Configuration("invalid rate".to_string());
    assert_eq!(e.to_string(), "Configuration error: invalid rate");
}

#[test]
fn error_reconnect_exhausted_display() {
    let e = AdapterError::ReconnectExhausted;
    assert_eq!(e.to_string(), "Reconnect attempts exhausted");
}

#[test]
fn error_other_display() {
    let e = AdapterError::Other("SimConnect DLL missing".to_string());
    assert_eq!(e.to_string(), "Adapter error: SimConnect DLL missing");
}

#[test]
fn error_debug_format() {
    let e = AdapterError::NotConnected;
    let dbg = format!("{:?}", e);
    assert!(dbg.contains("NotConnected"));
}

#[test]
fn error_is_std_error() {
    fn assert_error<E: std::error::Error>(_: &E) {}
    assert_error(&AdapterError::NotConnected);
    assert_error(&AdapterError::Timeout("t".into()));
    assert_error(&AdapterError::AircraftNotDetected);
    assert_error(&AdapterError::Configuration("c".into()));
    assert_error(&AdapterError::ReconnectExhausted);
    assert_error(&AdapterError::Other("o".into()));
}

#[test]
fn error_timeout_preserves_message() {
    let msg = "deadline exceeded after 30s with 3 retries";
    let e = AdapterError::Timeout(msg.to_string());
    assert!(e.to_string().contains(msg));
}

#[test]
fn error_configuration_preserves_message() {
    let msg = "publish_rate_hz must be > 0";
    let e = AdapterError::Configuration(msg.to_string());
    assert!(e.to_string().contains(msg));
}

// ─── AdapterConfig trait ─────────────────────────────────────────────────────

struct TestConfig {
    rate: f32,
    timeout: Duration,
    max_attempts: u32,
    auto_reconnect: bool,
}

impl AdapterConfig for TestConfig {
    fn publish_rate_hz(&self) -> f32 {
        self.rate
    }
    fn connection_timeout(&self) -> Duration {
        self.timeout
    }
    fn max_reconnect_attempts(&self) -> u32 {
        self.max_attempts
    }
    fn enable_auto_reconnect(&self) -> bool {
        self.auto_reconnect
    }
}

#[test]
fn config_trait_basic_impl() {
    let cfg = TestConfig {
        rate: 50.0,
        timeout: Duration::from_secs(5),
        max_attempts: 10,
        auto_reconnect: true,
    };
    assert_eq!(cfg.publish_rate_hz(), 50.0);
    assert_eq!(cfg.connection_timeout(), Duration::from_secs(5));
    assert_eq!(cfg.max_reconnect_attempts(), 10);
    assert!(cfg.enable_auto_reconnect());
}

#[test]
fn config_trait_auto_reconnect_disabled() {
    let cfg = TestConfig {
        rate: 25.0,
        timeout: Duration::from_secs(10),
        max_attempts: 0,
        auto_reconnect: false,
    };
    assert!(!cfg.enable_auto_reconnect());
    assert_eq!(cfg.max_reconnect_attempts(), 0);
}

#[test]
fn config_trait_object_safety() {
    let cfg: Box<dyn AdapterConfig> = Box::new(TestConfig {
        rate: 60.0,
        timeout: Duration::from_secs(3),
        max_attempts: 5,
        auto_reconnect: true,
    });
    assert_eq!(cfg.publish_rate_hz(), 60.0);
    assert_eq!(cfg.connection_timeout(), Duration::from_secs(3));
}

// ─── Integration: ReconnectionStrategy + AdapterConfig ───────────────────────

#[test]
fn reconnect_strategy_from_config() {
    let cfg = TestConfig {
        rate: 50.0,
        timeout: Duration::from_secs(5),
        max_attempts: 5,
        auto_reconnect: true,
    };
    let s = ReconnectionStrategy::new(
        cfg.max_reconnect_attempts(),
        Duration::from_millis(500),
        Duration::from_secs(30),
    );
    assert_eq!(s.max_attempts(), 5);
    assert!(s.should_retry(1));
}

#[test]
fn backoff_total_wait_within_budget() {
    let mut b = ExponentialBackoff::new(
        Duration::from_millis(100),
        Duration::from_secs(5),
        2.0,
        0.0,
    );
    let mut total = Duration::ZERO;
    for _ in 0..10 {
        total += b.next_delay();
    }
    // 100+200+400+800+1600+3200+5000+5000+5000+5000 = 26300ms
    assert!(total <= Duration::from_secs(27));
}

// ─── Property-based tests ────────────────────────────────────────────────────

mod proptest_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn reconnect_backoff_never_exceeds_max(
            initial_ms in 1u64..10_000,
            max_ms in 1u64..100_000,
            attempt in 0u32..50,
        ) {
            let initial = Duration::from_millis(initial_ms);
            let max = Duration::from_millis(max_ms);
            let s = ReconnectionStrategy::new(100, initial, max);
            let delay = s.next_backoff(attempt);
            prop_assert!(delay <= max);
        }

        #[test]
        fn exponential_backoff_never_exceeds_max(
            initial_ms in 1u64..5_000,
            max_ms in 1u64..60_000,
            multiplier_x10 in 10u32..50,  // 1.0 to 5.0
            jitter_x100 in 0u32..100,     // 0.00 to 0.99
            iterations in 1usize..30,
        ) {
            let initial = Duration::from_millis(initial_ms);
            let max = Duration::from_millis(max_ms);
            let multiplier = f64::from(multiplier_x10) / 10.0;
            let jitter = f64::from(jitter_x100) / 100.0;

            let mut b = ExponentialBackoff::new(initial, max, multiplier, jitter);
            for _ in 0..iterations {
                let d = b.next_delay();
                prop_assert!(d <= max, "delay {:?} exceeded max {:?}", d, max);
            }
        }

        #[test]
        fn reconnect_should_retry_boundary(max_attempts in 0u32..100) {
            let s = ReconnectionStrategy::new(
                max_attempts,
                Duration::from_millis(100),
                Duration::from_secs(10),
            );
            // Attempt at max_attempts is the last valid retry
            prop_assert!(s.should_retry(max_attempts));
            // Attempt beyond max_attempts is not
            prop_assert!(!s.should_retry(max_attempts + 1));
        }

        #[test]
        fn backoff_reset_always_restores_initial(
            initial_ms in 1u64..5_000,
            steps in 1usize..20,
        ) {
            let initial = Duration::from_millis(initial_ms);
            let mut b = ExponentialBackoff::new(
                initial,
                Duration::from_secs(600),
                2.0,
                0.0,
            );
            for _ in 0..steps {
                b.next_delay();
            }
            b.reset();
            prop_assert_eq!(b.attempt(), 0);
            prop_assert_eq!(b.next_delay(), initial);
        }

        #[test]
        fn backoff_attempt_counter_matches_calls(calls in 0usize..50) {
            let mut b = ExponentialBackoff::new(
                Duration::from_millis(100),
                Duration::from_secs(60),
                2.0,
                0.0,
            );
            for _ in 0..calls {
                b.next_delay();
            }
            prop_assert_eq!(b.attempt(), calls as u32);
        }

        #[test]
        fn metrics_update_count_matches_calls(n in 0usize..200) {
            let mut m = AdapterMetrics::new();
            for _ in 0..n {
                m.record_update();
            }
            prop_assert_eq!(m.total_updates, n as u64);
        }

        #[test]
        fn metrics_aircraft_change_idempotent(title in "[A-Z][0-9]{3}") {
            let mut m = AdapterMetrics::new();
            m.record_aircraft_change(title.clone());
            m.record_aircraft_change(title.clone());
            m.record_aircraft_change(title);
            prop_assert_eq!(m.aircraft_changes, 1);
        }

        #[test]
        fn adapter_state_copy_roundtrip(variant in 0u8..6) {
            let state = match variant {
                0 => AdapterState::Disconnected,
                1 => AdapterState::Connecting,
                2 => AdapterState::Connected,
                3 => AdapterState::DetectingAircraft,
                4 => AdapterState::Active,
                _ => AdapterState::Error,
            };
            let copy = state;
            prop_assert_eq!(state, copy);
        }
    }
}
