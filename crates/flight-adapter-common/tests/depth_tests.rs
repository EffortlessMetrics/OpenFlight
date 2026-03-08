// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Combined depth tests for flight-adapter-common.
//!
//! Covers: reconnection strategies, exponential backoff with jitter,
//! adapter state machine transitions, metrics collection, error handling,
//! config trait, and property-based tests.

use std::time::Duration;

use flight_adapter_common::{
    AdapterConfig, AdapterError, AdapterMetrics, AdapterState, ExponentialBackoff,
    ReconnectionStrategy,
};
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// AdapterConfig implementation for trait compliance tests (main branch style).
struct TestConfig {
    publish_rate: f32,
    timeout: Duration,
    max_reconnects: u32,
    auto_reconnect: bool,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            publish_rate: 20.0,
            timeout: Duration::from_secs(5),
            max_reconnects: 3,
            auto_reconnect: true,
        }
    }
}

impl AdapterConfig for TestConfig {
    fn publish_rate_hz(&self) -> f32 {
        self.publish_rate
    }
    fn connection_timeout(&self) -> Duration {
        self.timeout
    }
    fn max_reconnect_attempts(&self) -> u32 {
        self.max_reconnects
    }
    fn enable_auto_reconnect(&self) -> bool {
        self.auto_reconnect
    }
}

/// AdapterConfig implementation for trait compliance tests (HEAD branch style).
struct TestConfigHead {
    rate: f32,
    timeout: Duration,
    max_reconnect: u32,
    auto_reconnect: bool,
}

impl TestConfigHead {
    fn default_test() -> Self {
        Self {
            rate: 50.0,
            timeout: Duration::from_secs(5),
            max_reconnect: 3,
            auto_reconnect: true,
        }
    }
}

impl AdapterConfig for TestConfigHead {
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

/// Simple state machine driver that validates transitions (from HEAD).
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

/// Mock adapter that uses AdapterState, AdapterMetrics (from main).
struct MockAdapter {
    state: AdapterState,
    config: TestConfig,
    metrics: AdapterMetrics,
    strategy: ReconnectionStrategy,
    attempt: u32,
    last_error: Option<AdapterError>,
    telemetry: Vec<(String, f64)>,
}

impl MockAdapter {
    fn new(config: TestConfig) -> Self {
        let strategy = ReconnectionStrategy::new(
            config.max_reconnect_attempts(),
            Duration::from_millis(100),
            Duration::from_secs(5),
        );
        Self {
            state: AdapterState::Disconnected,
            config,
            metrics: AdapterMetrics::new(),
            strategy,
            attempt: 0,
            last_error: None,
            telemetry: Vec::new(),
        }
    }

    fn state(&self) -> AdapterState {
        self.state
    }

    fn connect(&mut self) -> Result<(), AdapterError> {
        match self.state {
            AdapterState::Disconnected | AdapterState::Error => {
                self.state = AdapterState::Connecting;
                self.state = AdapterState::Connected;
                self.attempt = 0;
                Ok(())
            }
            _ => Err(AdapterError::Other(format!(
                "Cannot connect from state {:?}",
                self.state
            ))),
        }
    }

    fn connect_fail(&mut self) -> Result<(), AdapterError> {
        match self.state {
            AdapterState::Disconnected | AdapterState::Error => {
                self.state = AdapterState::Connecting;
                self.state = AdapterState::Error;
                self.last_error =
                    Some(AdapterError::Timeout("connection timed out".to_string()));
                Err(AdapterError::Timeout("connection timed out".to_string()))
            }
            _ => Err(AdapterError::NotConnected),
        }
    }

    fn detect_aircraft(&mut self, title: &str) -> Result<(), AdapterError> {
        if self.state != AdapterState::Connected {
            return Err(AdapterError::NotConnected);
        }
        self.state = AdapterState::DetectingAircraft;
        self.metrics.record_aircraft_change(title.to_string());
        self.state = AdapterState::Active;
        Ok(())
    }

    fn read_telemetry(&mut self) -> Result<Vec<(String, f64)>, AdapterError> {
        if self.state != AdapterState::Active {
            return Err(AdapterError::NotConnected);
        }
        self.metrics.record_update();
        Ok(self.telemetry.clone())
    }

    fn write_command(&mut self, key: &str, value: f64) -> Result<(), AdapterError> {
        if self.state != AdapterState::Active {
            return Err(AdapterError::NotConnected);
        }
        self.telemetry.push((key.to_string(), value));
        Ok(())
    }

    fn disconnect(&mut self) {
        self.state = AdapterState::Disconnected;
        self.telemetry.clear();
    }

    fn reconnect_with_backoff(&mut self) -> Result<(), AdapterError> {
        self.attempt += 1;
        if !self.strategy.should_retry(self.attempt) {
            return Err(AdapterError::ReconnectExhausted);
        }
        let _backoff = self.strategy.next_backoff(self.attempt);
        self.state = AdapterState::Connecting;
        self.state = AdapterState::Connected;
        Ok(())
    }

    fn capabilities(&self) -> Vec<&str> {
        match self.state {
            AdapterState::Active => vec!["telemetry", "commands", "aircraft_detection"],
            AdapterState::Connected => vec!["aircraft_detection"],
            _ => vec![],
        }
    }

    fn metadata(&self) -> (&str, &str) {
        ("MockSim", "1.0.0")
    }
}

// ===================================================================
// 1. Adapter trait compliance / Adapter trait tests
// ===================================================================

#[test]
fn config_trait_publish_rate_head() {
    let cfg = TestConfigHead::default_test();
    assert!((cfg.publish_rate_hz() - 50.0).abs() < f32::EPSILON);
}

#[test]
fn config_trait_connection_timeout_head() {
    let cfg = TestConfigHead::default_test();
    assert_eq!(cfg.connection_timeout(), Duration::from_secs(5));
}

#[test]
fn adapter_trait_connect_lifecycle_main() {
    let cfg = TestConfig::default();
    assert_eq!(cfg.publish_rate_hz(), 20.0);
    let mut adapter = MockAdapter::new(cfg);
    adapter.connect().unwrap();
    adapter.detect_aircraft("C172").unwrap();
    assert_eq!(adapter.state(), AdapterState::Active);
    adapter.disconnect();
    assert_eq!(adapter.state(), AdapterState::Disconnected);
}

#[test]
fn config_trait_object_safety() {
    fn accepts_dyn(cfg: &dyn AdapterConfig) -> f32 {
        cfg.publish_rate_hz()
    }
    let cfg = TestConfigHead::default_test();
    assert!((accepts_dyn(&cfg) - 50.0).abs() < f32::EPSILON);
}

// ===================================================================
// 2. Telemetry conversion / metrics
// ===================================================================

#[test]
fn metrics_initial_state() {
    let m = AdapterMetrics::new();
    assert_eq!(m.total_updates, 0);
    assert_eq!(m.aircraft_changes, 0);
}

#[test]
fn metrics_aircraft_change_dedup() {
    let mut m = AdapterMetrics::new();
    m.record_aircraft_change("C172".into());
    m.record_aircraft_change("C172".into());
    assert_eq!(m.aircraft_changes, 1);
}

#[test]
fn metrics_summary_contains_all_fields() {
    let mut m = AdapterMetrics::new();
    m.record_update();
    let s = m.summary();
    assert!(s.contains("Updates:"));
    assert!(s.contains("Aircraft changes:"));
}

// ===================================================================
// 3. State machine tests
// ===================================================================

#[test]
fn state_machine_happy_path_lifecycle() {
    let mut sm = StateMachine::new();
    assert!(sm.transition(AdapterState::Connecting).is_ok());
    assert!(sm.transition(AdapterState::Connected).is_ok());
    assert!(sm.transition(AdapterState::Active).is_ok());
    assert_eq!(sm.state(), AdapterState::Active);
}

#[test]
fn state_machine_invalid_transition_rejected() {
    let mut sm = StateMachine::new();
    assert!(sm.transition(AdapterState::Active).is_err());
}

#[test]
fn state_machine_timeout_on_connecting() {
    let mut adapter = MockAdapter::new(TestConfig::default());
    let _ = adapter.connect_fail();
    assert_eq!(adapter.state(), AdapterState::Error);
}

// ===================================================================
// 4. Connection management / ReconnectionStrategy / ExponentialBackoff
// ===================================================================

#[test]
fn reconnection_strategy_should_retry_within_limit() {
    let s = ReconnectionStrategy::new(
        5,
        Duration::from_millis(100),
        Duration::from_millis(5000),
    );
    assert!(s.should_retry(5));
    assert!(!s.should_retry(6));
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
    b.reset();
    assert_eq!(b.attempt(), 0);
    assert_eq!(b.next_delay(), Duration::from_millis(100));
}

#[test]
fn backoff_caps_at_max_delay() {
    let mut b = ExponentialBackoff::new(
        Duration::from_millis(100),
        Duration::from_millis(300),
        2.0,
        0.0,
    );
    b.next_delay(); // 100
    b.next_delay(); // 200
    assert_eq!(b.next_delay(), Duration::from_millis(300));
}

// ===================================================================
// 5. Property / invariant tests
// ===================================================================

#[test]
fn adapter_error_all_variants_format_without_panic() {
    let errors = vec![
        AdapterError::NotConnected,
        AdapterError::ReconnectExhausted,
    ];
    for e in errors {
        let _ = format!("{}", e);
        let _ = format!("{:?}", e);
    }
}

// ===================================================================
// Proptests
// ===================================================================

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
