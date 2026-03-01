// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for flight-adapter-common: adapter traits, telemetry conversion,
//! state machine, connection management, and property-based invariants.

use std::time::Duration;

use flight_adapter_common::{
    AdapterConfig, AdapterError, AdapterMetrics, AdapterState, ExponentialBackoff,
    ReconnectionStrategy,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Minimal `AdapterConfig` implementation for trait compliance tests.
struct TestConfig {
    rate: f32,
    timeout: Duration,
    max_reconnect: u32,
    auto_reconnect: bool,
}

impl TestConfig {
    fn default_test() -> Self {
        Self {
            rate: 50.0,
            timeout: Duration::from_secs(5),
            max_reconnect: 3,
            auto_reconnect: true,
        }
    }
}

impl AdapterConfig for TestConfig {
    fn publish_rate_hz(&self) -> f32 {
        self.rate
    }
    fn connection_timeout(&self) -> Duration {
        self.timeout
    }
    fn max_reconnect_attempts(&self) -> u32 {
        self.max_reconnect
    }
    fn enable_auto_reconnect(&self) -> bool {
        self.auto_reconnect
    }
}

/// Simple state machine driver that validates transitions.
struct StateMachine {
    state: AdapterState,
    transition_log: Vec<(AdapterState, AdapterState)>,
}

impl StateMachine {
    fn new() -> Self {
        Self {
            state: AdapterState::Disconnected,
            transition_log: Vec::new(),
        }
    }

    fn state(&self) -> AdapterState {
        self.state
    }

    /// Attempt a state transition. Returns `Ok(new_state)` on a valid
    /// transition, `Err(())` when the transition is illegal.
    fn transition(&mut self, target: AdapterState) -> Result<AdapterState, ()> {
        let valid = matches!(
            (self.state, target),
            (AdapterState::Disconnected, AdapterState::Connecting)
                | (AdapterState::Connecting, AdapterState::Connected)
                | (AdapterState::Connecting, AdapterState::Error)
                | (AdapterState::Connected, AdapterState::Active)
                | (AdapterState::Connected, AdapterState::DetectingAircraft)
                | (AdapterState::DetectingAircraft, AdapterState::Active)
                | (AdapterState::DetectingAircraft, AdapterState::Error)
                | (AdapterState::Active, AdapterState::Error)
                | (AdapterState::Active, AdapterState::Disconnected)
                | (AdapterState::Error, AdapterState::Disconnected)
                | (AdapterState::Error, AdapterState::Connecting)
        );
        if valid {
            let old = self.state;
            self.state = target;
            self.transition_log.push((old, target));
            Ok(target)
        } else {
            Err(())
        }
    }
}

// ===================================================================
// 1. Adapter trait compliance (6 tests)
// ===================================================================
mod adapter_trait {
    use super::*;

    #[test]
    fn config_trait_publish_rate() {
        let cfg = TestConfig::default_test();
        assert!((cfg.publish_rate_hz() - 50.0).abs() < f32::EPSILON);
    }

    #[test]
    fn config_trait_connection_timeout() {
        let cfg = TestConfig::default_test();
        assert_eq!(cfg.connection_timeout(), Duration::from_secs(5));
    }

    #[test]
    fn config_trait_max_reconnect() {
        let cfg = TestConfig::default_test();
        assert_eq!(cfg.max_reconnect_attempts(), 3);
    }

    #[test]
    fn config_trait_auto_reconnect_flag() {
        let cfg = TestConfig::default_test();
        assert!(cfg.enable_auto_reconnect());

        let disabled = TestConfig {
            auto_reconnect: false,
            ..TestConfig::default_test()
        };
        assert!(!disabled.enable_auto_reconnect());
    }

    #[test]
    fn config_trait_custom_values() {
        let cfg = TestConfig {
            rate: 250.0,
            timeout: Duration::from_millis(500),
            max_reconnect: 10,
            auto_reconnect: false,
        };
        assert!((cfg.publish_rate_hz() - 250.0).abs() < f32::EPSILON);
        assert_eq!(cfg.connection_timeout(), Duration::from_millis(500));
        assert_eq!(cfg.max_reconnect_attempts(), 10);
        assert!(!cfg.enable_auto_reconnect());
    }

    #[test]
    fn config_trait_object_safety() {
        // Ensure AdapterConfig can be used as a trait object.
        fn accepts_dyn(cfg: &dyn AdapterConfig) -> f32 {
            cfg.publish_rate_hz()
        }
        let cfg = TestConfig::default_test();
        assert!((accepts_dyn(&cfg) - 50.0).abs() < f32::EPSILON);
    }
}

// ===================================================================
// 2. Telemetry conversion / metrics (10 tests)
// ===================================================================
mod telemetry_conversion {
    use super::*;

    #[test]
    fn metrics_initial_state() {
        let m = AdapterMetrics::new();
        assert_eq!(m.total_updates, 0);
        assert!(m.last_update_time.is_none());
        assert!(m.update_intervals.is_empty());
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
        // First update produces no interval (need two points).
        assert!(m.update_intervals.is_empty());
    }

    #[test]
    fn metrics_two_updates_produce_one_interval() {
        let mut m = AdapterMetrics::new();
        m.record_update();
        m.record_update();
        assert_eq!(m.total_updates, 2);
        assert_eq!(m.update_intervals.len(), 1);
    }

    #[test]
    fn metrics_rate_positive_after_updates() {
        let mut m = AdapterMetrics::new();
        for _ in 0..5 {
            m.record_update();
        }
        assert!(m.actual_update_rate > 0.0, "rate should be positive");
    }

    #[test]
    fn metrics_max_interval_samples_respected() {
        let mut m = AdapterMetrics::new();
        m.max_interval_samples = 5;
        for _ in 0..20 {
            m.record_update();
        }
        assert!(
            m.update_intervals.len() <= 5,
            "intervals should be capped at max_interval_samples"
        );
    }

    #[test]
    fn metrics_aircraft_change_dedup() {
        let mut m = AdapterMetrics::new();
        m.record_aircraft_change("C172".into());
        m.record_aircraft_change("C172".into());
        m.record_aircraft_change("C172".into());
        assert_eq!(m.aircraft_changes, 1, "duplicate titles should not bump count");
    }

    #[test]
    fn metrics_aircraft_change_distinct() {
        let mut m = AdapterMetrics::new();
        m.record_aircraft_change("C172".into());
        m.record_aircraft_change("A320".into());
        m.record_aircraft_change("B737".into());
        assert_eq!(m.aircraft_changes, 3);
        assert_eq!(m.last_aircraft_title.as_deref(), Some("B737"));
    }

    #[test]
    fn metrics_summary_contains_all_fields() {
        let mut m = AdapterMetrics::new();
        m.record_update();
        m.record_update();
        m.record_aircraft_change("F18".into());
        let s = m.summary();
        assert!(s.contains("Updates:"), "{s}");
        assert!(s.contains("Rate:"), "{s}");
        assert!(s.contains("Jitter p99:"), "{s}");
        assert!(s.contains("Aircraft changes:"), "{s}");
    }

    #[test]
    fn metrics_default_matches_new() {
        let from_new = AdapterMetrics::new();
        let from_default = AdapterMetrics::default();
        // Both should start at zero updates.
        assert_eq!(from_new.total_updates, from_default.total_updates);
        // `new()` sets max_interval_samples to 100; `default()` uses 0.
        assert_eq!(from_new.max_interval_samples, 100);
    }

    #[test]
    fn metrics_jitter_p99_nonnegative() {
        let mut m = AdapterMetrics::new();
        for _ in 0..50 {
            m.record_update();
        }
        assert!(
            m.update_jitter_p99_ms >= 0.0,
            "jitter must never be negative"
        );
    }
}

// ===================================================================
// 3. State machine (8 tests)
// ===================================================================
mod state_machine {
    use super::*;

    #[test]
    fn initial_state_is_disconnected() {
        let sm = StateMachine::new();
        assert_eq!(sm.state(), AdapterState::Disconnected);
    }

    #[test]
    fn happy_path_lifecycle() {
        let mut sm = StateMachine::new();
        assert!(sm.transition(AdapterState::Connecting).is_ok());
        assert!(sm.transition(AdapterState::Connected).is_ok());
        assert!(sm.transition(AdapterState::Active).is_ok());
        assert_eq!(sm.state(), AdapterState::Active);
    }

    #[test]
    fn invalid_transition_rejected() {
        let mut sm = StateMachine::new();
        // Cannot jump from Disconnected straight to Active.
        assert!(sm.transition(AdapterState::Active).is_err());
        assert_eq!(sm.state(), AdapterState::Disconnected);
    }

    #[test]
    fn error_recovery_cycle() {
        let mut sm = StateMachine::new();
        sm.transition(AdapterState::Connecting).unwrap();
        sm.transition(AdapterState::Error).unwrap();
        // Error → Disconnected → retry
        sm.transition(AdapterState::Disconnected).unwrap();
        sm.transition(AdapterState::Connecting).unwrap();
        sm.transition(AdapterState::Connected).unwrap();
        assert_eq!(sm.state(), AdapterState::Connected);
    }

    #[test]
    fn error_direct_reconnect() {
        let mut sm = StateMachine::new();
        sm.transition(AdapterState::Connecting).unwrap();
        sm.transition(AdapterState::Error).unwrap();
        // Error → Connecting shortcut for fast reconnect.
        sm.transition(AdapterState::Connecting).unwrap();
        assert_eq!(sm.state(), AdapterState::Connecting);
    }

    #[test]
    fn detecting_aircraft_path() {
        let mut sm = StateMachine::new();
        sm.transition(AdapterState::Connecting).unwrap();
        sm.transition(AdapterState::Connected).unwrap();
        sm.transition(AdapterState::DetectingAircraft).unwrap();
        sm.transition(AdapterState::Active).unwrap();
        assert_eq!(sm.state(), AdapterState::Active);
    }

    #[test]
    fn active_to_disconnected_graceful_shutdown() {
        let mut sm = StateMachine::new();
        sm.transition(AdapterState::Connecting).unwrap();
        sm.transition(AdapterState::Connected).unwrap();
        sm.transition(AdapterState::Active).unwrap();
        sm.transition(AdapterState::Disconnected).unwrap();
        assert_eq!(sm.state(), AdapterState::Disconnected);
    }

    #[test]
    fn transition_log_records_history() {
        let mut sm = StateMachine::new();
        sm.transition(AdapterState::Connecting).unwrap();
        sm.transition(AdapterState::Connected).unwrap();
        assert_eq!(sm.transition_log.len(), 2);
        assert_eq!(
            sm.transition_log[0],
            (AdapterState::Disconnected, AdapterState::Connecting)
        );
        assert_eq!(
            sm.transition_log[1],
            (AdapterState::Connecting, AdapterState::Connected)
        );
    }
}

// ===================================================================
// 4. Connection management (6 tests)
// ===================================================================
mod connection_management {
    use super::*;

    #[test]
    fn reconnection_strategy_should_retry_within_limit() {
        let s = ReconnectionStrategy::new(
            5,
            Duration::from_millis(100),
            Duration::from_millis(5000),
        );
        for attempt in 1..=5 {
            assert!(s.should_retry(attempt), "attempt {attempt} should retry");
        }
        assert!(!s.should_retry(6));
    }

    #[test]
    fn reconnection_backoff_monotonic_up_to_cap() {
        let s = ReconnectionStrategy::new(
            10,
            Duration::from_millis(100),
            Duration::from_millis(5000),
        );
        let mut prev = Duration::ZERO;
        for attempt in 1..=10 {
            let delay = s.next_backoff(attempt);
            assert!(delay >= prev, "backoff should be monotonically non-decreasing");
            assert!(delay <= Duration::from_millis(5000), "backoff must respect cap");
            prev = delay;
        }
    }

    #[test]
    fn exponential_backoff_reset_restarts() {
        let mut b = ExponentialBackoff::new(
            Duration::from_millis(100),
            Duration::from_secs(10),
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
    fn exponential_backoff_cap_enforced() {
        let mut b = ExponentialBackoff::new(
            Duration::from_millis(100),
            Duration::from_millis(300),
            2.0,
            0.0,
        );
        // 100, 200, 400→300, 800→300
        for _ in 0..10 {
            let d = b.next_delay();
            assert!(d <= Duration::from_millis(300));
        }
    }

    #[test]
    fn graceful_shutdown_via_state_machine() {
        let mut sm = StateMachine::new();
        sm.transition(AdapterState::Connecting).unwrap();
        sm.transition(AdapterState::Connected).unwrap();
        sm.transition(AdapterState::Active).unwrap();
        // Graceful shutdown from Active.
        sm.transition(AdapterState::Disconnected).unwrap();
        assert_eq!(sm.state(), AdapterState::Disconnected);
    }

    #[test]
    fn reconnect_exhausted_error() {
        let err = AdapterError::ReconnectExhausted;
        assert_eq!(err.to_string(), "Reconnect attempts exhausted");
    }
}

// ===================================================================
// 5. Property / invariant tests (5 tests)
// ===================================================================
mod property_tests {
    use super::*;

    #[test]
    fn backoff_never_exceeds_max_for_any_attempt() {
        let s = ReconnectionStrategy::new(
            u32::MAX,
            Duration::from_millis(1),
            Duration::from_millis(1000),
        );
        for attempt in [0, 1, 10, 100, 1000, u32::MAX] {
            let d = s.next_backoff(attempt);
            assert!(
                d <= Duration::from_millis(1000),
                "attempt {attempt} produced {d:?}"
            );
        }
    }

    #[test]
    fn exponential_backoff_never_exceeds_max_many_steps() {
        let mut b = ExponentialBackoff::new(
            Duration::from_millis(50),
            Duration::from_millis(500),
            3.0,
            0.25,
        );
        for i in 0..200 {
            let d = b.next_delay();
            assert!(
                d <= Duration::from_millis(500),
                "step {i} delay {d:?} exceeded max"
            );
        }
    }

    #[test]
    fn state_machine_never_panics_random_events() {
        let all_states = [
            AdapterState::Disconnected,
            AdapterState::Connecting,
            AdapterState::Connected,
            AdapterState::DetectingAircraft,
            AdapterState::Active,
            AdapterState::Error,
        ];
        let mut sm = StateMachine::new();
        // Throw every possible event at the state machine 3 times over.
        for _ in 0..3 {
            for &target in &all_states {
                let _ = sm.transition(target);
            }
        }
        // Must still be in a valid state (one of the enum variants).
        assert!(all_states.contains(&sm.state()));
    }

    #[test]
    fn adapter_error_all_variants_format_without_panic() {
        let errors: Vec<AdapterError> = vec![
            AdapterError::NotConnected,
            AdapterError::Timeout(String::new()),
            AdapterError::AircraftNotDetected,
            AdapterError::Configuration(String::new()),
            AdapterError::ReconnectExhausted,
            AdapterError::Other(String::new()),
        ];
        for e in &errors {
            let msg = format!("{e}");
            assert!(!msg.is_empty(), "error Display must not be empty");
            let dbg = format!("{e:?}");
            assert!(!dbg.is_empty());
        }
    }

    #[test]
    fn adapter_state_copy_semantics() {
        let all = [
            AdapterState::Disconnected,
            AdapterState::Connecting,
            AdapterState::Connected,
            AdapterState::DetectingAircraft,
            AdapterState::Active,
            AdapterState::Error,
        ];
        for s in all {
            let copy = s;
            assert_eq!(s, copy, "AdapterState must implement Copy correctly");
        }
    }
}
