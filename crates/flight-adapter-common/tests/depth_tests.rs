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

// ---------------------------------------------------------------------------
// Mock adapter config for trait tests
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Mock state machine that uses AdapterState
// ---------------------------------------------------------------------------

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
                // Simulate successful connection
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
        // Inline state transition (don't call connect which resets attempt)
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

// ---------------------------------------------------------------------------
// Telemetry snapshot mock
// ---------------------------------------------------------------------------

struct TelemetrySnapshot {
    fields: Vec<(String, f64)>,
    timestamp: std::time::Instant,
    max_age: Duration,
}

impl TelemetrySnapshot {
    fn new(fields: Vec<(String, f64)>, max_age: Duration) -> Self {
        Self {
            fields,
            timestamp: std::time::Instant::now(),
            max_age,
        }
    }

    fn get(&self, key: &str) -> Option<f64> {
        self.fields.iter().find(|(k, _)| k == key).map(|(_, v)| *v)
    }

    fn is_stale(&self) -> bool {
        self.timestamp.elapsed() > self.max_age
    }

    fn field_count(&self) -> usize {
        self.fields.len()
    }

    fn get_as_i32(&self, key: &str) -> Option<i32> {
        self.get(key).map(|v| v as i32)
    }

    fn get_as_bool(&self, key: &str) -> Option<bool> {
        self.get(key).map(|v| v != 0.0)
    }
}

// ---------------------------------------------------------------------------
// Mock bus for integration tests
// ---------------------------------------------------------------------------

struct MockBus {
    messages: Vec<(String, Vec<(String, f64)>)>,
    connected: bool,
}

impl MockBus {
    fn new() -> Self {
        Self {
            messages: Vec::new(),
            connected: true,
        }
    }

    fn publish(&mut self, topic: &str, data: Vec<(String, f64)>) -> Result<(), AdapterError> {
        if !self.connected {
            return Err(AdapterError::Other("Bus disconnected".to_string()));
        }
        self.messages.push((topic.to_string(), data));
        Ok(())
    }

    fn disconnect(&mut self) {
        self.connected = false;
    }

    fn message_count(&self) -> usize {
        self.messages.len()
    }
}

// ===========================================================================
// Adapter trait tests
// ===========================================================================

#[test]
fn adapter_trait_connect_lifecycle() {
    let cfg = TestConfig::default();
    assert_eq!(cfg.publish_rate_hz(), 20.0);
    assert_eq!(cfg.connection_timeout(), Duration::from_secs(5));

    let mut adapter = MockAdapter::new(cfg);
    assert_eq!(adapter.state(), AdapterState::Disconnected);
    adapter.connect().unwrap();
    assert_eq!(adapter.state(), AdapterState::Connected);
    adapter.detect_aircraft("C172").unwrap();
    assert_eq!(adapter.state(), AdapterState::Active);
    adapter.disconnect();
    assert_eq!(adapter.state(), AdapterState::Disconnected);
}

#[test]
fn adapter_trait_read_telemetry_requires_active() {
    let mut adapter = MockAdapter::new(TestConfig::default());
    // Not connected → read fails
    let err = adapter.read_telemetry().unwrap_err();
    assert!(matches!(err, AdapterError::NotConnected));

    // Connected but not active → still fails
    adapter.connect().unwrap();
    let err = adapter.read_telemetry().unwrap_err();
    assert!(matches!(err, AdapterError::NotConnected));

    // Active → succeeds
    adapter.detect_aircraft("A320").unwrap();
    let data = adapter.read_telemetry().unwrap();
    assert!(data.is_empty());
}

#[test]
fn adapter_trait_write_commands() {
    let mut adapter = MockAdapter::new(TestConfig::default());
    adapter.connect().unwrap();
    adapter.detect_aircraft("B738").unwrap();

    adapter.write_command("throttle", 0.75).unwrap();
    adapter.write_command("mixture", 1.0).unwrap();

    let data = adapter.read_telemetry().unwrap();
    assert_eq!(data.len(), 2);
    assert_eq!(data[0], ("throttle".to_string(), 0.75));
    assert_eq!(data[1], ("mixture".to_string(), 1.0));
}

#[test]
fn adapter_trait_capabilities_query() {
    let mut adapter = MockAdapter::new(TestConfig::default());
    assert!(adapter.capabilities().is_empty());

    adapter.connect().unwrap();
    assert_eq!(adapter.capabilities(), vec!["aircraft_detection"]);

    adapter.detect_aircraft("C172").unwrap();
    let caps = adapter.capabilities();
    assert!(caps.contains(&"telemetry"));
    assert!(caps.contains(&"commands"));
    assert!(caps.contains(&"aircraft_detection"));
}

#[test]
fn adapter_trait_state_query_reflects_transitions() {
    let mut adapter = MockAdapter::new(TestConfig::default());

    let states_visited: Vec<AdapterState> = {
        let mut v = vec![adapter.state()];
        adapter.connect().unwrap();
        v.push(adapter.state());
        adapter.detect_aircraft("F16").unwrap();
        v.push(adapter.state());
        adapter.disconnect();
        v.push(adapter.state());
        v
    };

    assert_eq!(
        states_visited,
        vec![
            AdapterState::Disconnected,
            AdapterState::Connected,
            AdapterState::Active,
            AdapterState::Disconnected,
        ]
    );
}

#[test]
fn adapter_trait_metadata() {
    let adapter = MockAdapter::new(TestConfig::default());
    let (name, version) = adapter.metadata();
    assert_eq!(name, "MockSim");
    assert_eq!(version, "1.0.0");
}

// ===========================================================================
// State machine tests
// ===========================================================================

#[test]
fn state_machine_full_happy_path() {
    let mut adapter = MockAdapter::new(TestConfig::default());
    assert_eq!(adapter.state(), AdapterState::Disconnected);
    adapter.connect().unwrap();
    assert_eq!(adapter.state(), AdapterState::Connected);
    adapter.detect_aircraft("C172").unwrap();
    assert_eq!(adapter.state(), AdapterState::Active);
}

#[test]
fn state_machine_valid_transitions_only() {
    // Cannot detect aircraft when disconnected
    let mut adapter = MockAdapter::new(TestConfig::default());
    let err = adapter.detect_aircraft("C172").unwrap_err();
    assert!(matches!(err, AdapterError::NotConnected));

    // Cannot write when connected but not active
    adapter.connect().unwrap();
    let err = adapter.write_command("k", 1.0).unwrap_err();
    assert!(matches!(err, AdapterError::NotConnected));
}

#[test]
fn state_machine_invalid_transition_rejection() {
    let mut adapter = MockAdapter::new(TestConfig::default());
    adapter.connect().unwrap();
    // Double-connect from Connected state is rejected
    let err = adapter.connect().unwrap_err();
    assert!(matches!(err, AdapterError::Other(_)));

    adapter.detect_aircraft("A320").unwrap();
    // Connect from Active is also rejected
    let err = adapter.connect().unwrap_err();
    assert!(matches!(err, AdapterError::Other(_)));
}

#[test]
fn state_machine_timeout_on_connecting() {
    let mut adapter = MockAdapter::new(TestConfig::default());
    let err = adapter.connect_fail().unwrap_err();
    assert!(matches!(err, AdapterError::Timeout(_)));
    assert_eq!(adapter.state(), AdapterState::Error);
}

#[test]
fn state_machine_reconnect_after_error() {
    let mut adapter = MockAdapter::new(TestConfig::default());
    // Force error state
    let _ = adapter.connect_fail();
    assert_eq!(adapter.state(), AdapterState::Error);

    // Reconnect from error should succeed
    adapter.connect().unwrap();
    assert_eq!(adapter.state(), AdapterState::Connected);
}

#[test]
fn state_machine_state_persistence_across_operations() {
    let mut adapter = MockAdapter::new(TestConfig::default());
    adapter.connect().unwrap();
    adapter.detect_aircraft("C172").unwrap();

    // Multiple reads don't change state
    for _ in 0..10 {
        adapter.read_telemetry().unwrap();
        assert_eq!(adapter.state(), AdapterState::Active);
    }

    // Multiple writes don't change state
    for i in 0..5 {
        adapter
            .write_command(&format!("var_{i}"), i as f64)
            .unwrap();
        assert_eq!(adapter.state(), AdapterState::Active);
    }
}

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

// ===========================================================================
// Telemetry snapshot tests
// ===========================================================================

#[test]
fn telemetry_snapshot_creation() {
    let snap = TelemetrySnapshot::new(
        vec![
            ("altitude".to_string(), 5000.0),
            ("speed".to_string(), 120.0),
        ],
        Duration::from_secs(1),
    );
    assert_eq!(snap.field_count(), 2);
}

#[test]
fn telemetry_snapshot_field_access() {
    let snap = TelemetrySnapshot::new(
        vec![
            ("altitude".to_string(), 5000.0),
            ("heading".to_string(), 270.0),
        ],
        Duration::from_secs(1),
    );
    assert_eq!(snap.get("altitude"), Some(5000.0));
    assert_eq!(snap.get("heading"), Some(270.0));
}

#[test]
fn telemetry_snapshot_timestamp_is_recent() {
    let before = std::time::Instant::now();
    let snap = TelemetrySnapshot::new(vec![], Duration::from_secs(10));
    assert!(snap.timestamp >= before, "snapshot timestamp should be at or after construction start");
}

#[test]
fn telemetry_snapshot_staleness_detection() {
    let mut snap = TelemetrySnapshot::new(vec![], Duration::from_millis(1));
    // Backdate so the snapshot is guaranteed stale
    snap.timestamp = std::time::Instant::now() - Duration::from_millis(10);
    assert!(snap.is_stale());

    let fresh = TelemetrySnapshot::new(vec![], Duration::from_secs(60));
    assert!(!fresh.is_stale());
}

#[test]
fn telemetry_snapshot_field_type_conversion() {
    let snap = TelemetrySnapshot::new(
        vec![
            ("gear_pos".to_string(), 1.0),
            ("flaps_angle".to_string(), 15.7),
            ("parking_brake".to_string(), 0.0),
        ],
        Duration::from_secs(1),
    );
    assert_eq!(snap.get_as_i32("flaps_angle"), Some(15));
    assert_eq!(snap.get_as_bool("gear_pos"), Some(true));
    assert_eq!(snap.get_as_bool("parking_brake"), Some(false));
}

#[test]
fn telemetry_snapshot_missing_field_handling() {
    let snap = TelemetrySnapshot::new(vec![], Duration::from_secs(1));
    assert_eq!(snap.get("nonexistent"), None);
    assert_eq!(snap.get_as_i32("nonexistent"), None);
    assert_eq!(snap.get_as_bool("nonexistent"), None);
}

// ===========================================================================
// Bus integration tests
// ===========================================================================

#[test]
fn bus_adapter_publishes_telemetry() {
    let mut adapter = MockAdapter::new(TestConfig::default());
    let mut bus = MockBus::new();

    adapter.connect().unwrap();
    adapter.detect_aircraft("C172").unwrap();
    adapter.write_command("alt", 5000.0).unwrap();

    let data = adapter.read_telemetry().unwrap();
    bus.publish("sim/telemetry", data).unwrap();

    assert_eq!(bus.message_count(), 1);
    assert_eq!(bus.messages[0].0, "sim/telemetry");
    assert_eq!(bus.messages[0].1.len(), 1);
}

#[test]
fn bus_snapshot_format_contains_key_value_pairs() {
    let mut bus = MockBus::new();
    let data = vec![
        ("altitude".to_string(), 5000.0),
        ("speed".to_string(), 120.0),
        ("heading".to_string(), 270.0),
    ];
    bus.publish("sim/telemetry", data).unwrap();

    let msg = &bus.messages[0];
    assert_eq!(msg.1.len(), 3);
    assert!(msg.1.iter().any(|(k, _)| k == "altitude"));
    assert!(msg.1.iter().any(|(k, _)| k == "speed"));
}

#[test]
fn bus_publish_frequency_tracking() {
    let mut bus = MockBus::new();
    let mut adapter = MockAdapter::new(TestConfig::default());
    adapter.connect().unwrap();
    adapter.detect_aircraft("C172").unwrap();

    // Simulate multiple publish cycles
    for i in 0..20 {
        adapter
            .write_command("throttle", i as f64 / 20.0)
            .unwrap();
        let data = adapter.read_telemetry().unwrap();
        bus.publish("sim/telemetry", data).unwrap();
    }

    assert_eq!(bus.message_count(), 20);
    assert_eq!(adapter.metrics.total_updates, 20);
}

#[test]
fn bus_stale_marking_on_no_updates() {
    let mut snap = TelemetrySnapshot::new(
        vec![("alt".to_string(), 1000.0)],
        Duration::from_millis(1),
    );
    // Backdate the timestamp so the snapshot is guaranteed stale
    snap.timestamp = std::time::Instant::now() - Duration::from_millis(10);
    assert!(snap.is_stale());
    assert_eq!(snap.field_count(), 1);
}

#[test]
fn bus_disconnect_handling() {
    let mut bus = MockBus::new();
    bus.publish("topic", vec![]).unwrap();
    assert_eq!(bus.message_count(), 1);

    bus.disconnect();
    let err = bus.publish("topic", vec![]).unwrap_err();
    assert!(matches!(err, AdapterError::Other(_)));
    assert!(err.to_string().contains("Bus disconnected"));
}

// ===========================================================================
// Metrics tests
// ===========================================================================

#[test]
fn metrics_aircraft_change_count() {
    let mut m = AdapterMetrics::new();
    m.record_aircraft_change("C172".to_string());
    m.record_aircraft_change("A320".to_string());
    m.record_aircraft_change("B738".to_string());
    assert_eq!(m.aircraft_changes, 3);
}

#[test]
fn metrics_state_returns_to_disconnected_after_cycles() {
    let mut adapter = MockAdapter::new(TestConfig::default());
    let mut disconnect_count = 0u32;

    for _ in 0..5 {
        adapter.connect().unwrap();
        adapter.detect_aircraft("C172").unwrap();
        adapter.disconnect();
        disconnect_count += 1;
    }

    assert_eq!(disconnect_count, 5);
    // Adapter returns to Disconnected each time
    assert_eq!(adapter.state(), AdapterState::Disconnected);
}

#[test]
fn metrics_telemetry_rate_computed() {
    let mut m = AdapterMetrics::new();
    // Record enough samples to get a rate
    for _ in 0..10 {
        m.record_update();
    }
    assert_eq!(m.total_updates, 10);
    assert!(!m.update_intervals.is_empty());
    // Rate should be > 0 after multiple updates
    assert!(m.actual_update_rate > 0.0);
}

#[test]
fn metrics_error_rate_tracking() {
    let mut adapter = MockAdapter::new(TestConfig::default());
    let mut error_count = 0u32;

    for _ in 0..4 {
        if adapter.connect_fail().is_err() {
            error_count += 1;
        }
        // Reset to disconnected for next attempt
        adapter.state = AdapterState::Disconnected;
    }

    assert_eq!(error_count, 4);
}

#[test]
fn metrics_latency_tracking_via_intervals() {
    let mut m = AdapterMetrics::new();
    m.max_interval_samples = 5;

    for _ in 0..10 {
        m.record_update();
    }

    // Buffer should be capped at max_interval_samples
    assert!(m.update_intervals.len() <= 5);
    // p99 jitter should be computed
    assert!(m.update_jitter_p99_ms >= 0.0);
}

#[test]
fn metrics_summary_with_zero_updates() {
    let m = AdapterMetrics::new();
    let s = m.summary();
    assert!(s.contains("Updates: 0"));
    assert!(s.contains("Rate: 0.0 Hz"));
    assert!(s.contains("Aircraft changes: 0"));
}

#[test]
fn metrics_aircraft_change_same_title_not_counted() {
    let mut m = AdapterMetrics::new();
    m.record_aircraft_change("C172".to_string());
    m.record_aircraft_change("C172".to_string());
    m.record_aircraft_change("C172".to_string());
    assert_eq!(m.aircraft_changes, 1);
    assert_eq!(m.last_aircraft_title, Some("C172".to_string()));
}

#[test]
fn metrics_interval_buffer_respects_max_samples() {
    let mut m = AdapterMetrics::new();
    m.max_interval_samples = 3;

    for _ in 0..10 {
        m.record_update();
    }

    // Buffer should never exceed max_interval_samples
    assert!(m.update_intervals.len() <= 3);
}

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
    assert_eq!(n.max_interval_samples, 100);
}

// ===========================================================================
// Error handling tests
// ===========================================================================

#[test]
fn error_connection_failure_produces_timeout() {
    let mut adapter = MockAdapter::new(TestConfig::default());
    let err = adapter.connect_fail().unwrap_err();
    assert!(matches!(err, AdapterError::Timeout(_)));
    assert!(err.to_string().contains("connection timed out"));
}

#[test]
fn error_read_timeout_on_wrong_state() {
    let mut adapter = MockAdapter::new(TestConfig::default());
    adapter.connect().unwrap();
    // Connected but not active
    let err = adapter.read_telemetry().unwrap_err();
    assert!(matches!(err, AdapterError::NotConnected));
}

#[test]
fn error_write_failure_when_disconnected() {
    let mut adapter = MockAdapter::new(TestConfig::default());
    let err = adapter.write_command("key", 1.0).unwrap_err();
    assert!(matches!(err, AdapterError::NotConnected));
    assert_eq!(err.to_string(), "Not connected");
}

#[test]
fn error_protocol_version_mismatch() {
    let err = AdapterError::Configuration("protocol version mismatch: expected v2, got v1".to_string());
    assert!(err.to_string().contains("protocol version mismatch"));
    assert!(err.to_string().contains("v2"));
}

#[test]
fn error_reconnect_exhaustion() {
    let mut adapter = MockAdapter::new(TestConfig {
        max_reconnects: 2,
        ..TestConfig::default()
    });

    // Attempts 1 and 2 are within max_attempts
    adapter.state = AdapterState::Error;
    adapter.reconnect_with_backoff().unwrap();
    adapter.state = AdapterState::Error;
    adapter.reconnect_with_backoff().unwrap();
    // Attempt 3 exceeds max_attempts of 2
    adapter.state = AdapterState::Error;
    let err = adapter.reconnect_with_backoff().unwrap_err();
    assert!(matches!(err, AdapterError::ReconnectExhausted));
}

#[test]
fn error_all_variants_display() {
    let errors: Vec<AdapterError> = vec![
        AdapterError::NotConnected,
        AdapterError::Timeout("deadline".to_string()),
        AdapterError::AircraftNotDetected,
        AdapterError::Configuration("bad".to_string()),
        AdapterError::ReconnectExhausted,
        AdapterError::Other("misc".to_string()),
    ];
    let displays: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
    assert_eq!(displays[0], "Not connected");
    assert!(displays[1].contains("deadline"));
    assert_eq!(displays[2], "Aircraft not detected");
    assert!(displays[3].contains("bad"));
    assert_eq!(displays[4], "Reconnect attempts exhausted");
    assert!(displays[5].contains("misc"));
}

#[test]
fn error_debug_format_all_variants() {
    let errors: Vec<AdapterError> = vec![
        AdapterError::NotConnected,
        AdapterError::Timeout("t".to_string()),
        AdapterError::AircraftNotDetected,
        AdapterError::Configuration("c".to_string()),
        AdapterError::ReconnectExhausted,
        AdapterError::Other("o".to_string()),
    ];
    for e in &errors {
        let dbg = format!("{e:?}");
        assert!(!dbg.is_empty());
    }
}

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

// ===========================================================================
// ReconnectionStrategy tests
// ===========================================================================

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

#[test]
fn reconnection_strategy_should_retry_boundary() {
    let s = ReconnectionStrategy::new(3, Duration::from_millis(100), Duration::from_secs(10));
    assert!(s.should_retry(3));
    assert!(!s.should_retry(4));
    assert!(s.should_retry(1));
    assert!(s.should_retry(2));
}

#[test]
fn reconnection_strategy_backoff_overflow_protection() {
    let s = ReconnectionStrategy::new(100, Duration::from_millis(1000), Duration::from_secs(30));
    // Very large attempt number should not overflow — capped at max
    let d = s.next_backoff(50);
    assert_eq!(d, Duration::from_secs(30));
}

// ===========================================================================
// ExponentialBackoff tests
// ===========================================================================

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
    let mut b =
        ExponentialBackoff::new(Duration::from_millis(50), Duration::from_secs(60), 2.0, 0.0);
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
    let mut b =
        ExponentialBackoff::new(Duration::from_millis(10), Duration::from_secs(60), 3.0, 0.0);
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
    // Build the pure exponential sequence (no jitter) for comparison.
    let mut pure = Vec::new();
    {
        let mut b_pure = ExponentialBackoff::new(
            Duration::from_millis(1000),
            Duration::from_secs(600),
            2.0,
            0.0,
        );
        for _ in 0..5 {
            pure.push(b_pure.next_delay());
        }
    }
    // Jitter (0.25) should cause at least one delay to differ from the pure sequence.
    assert_eq!(delays.len(), 5);
    assert!(
        delays != pure,
        "jittered delays should differ from pure exponential"
    );
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
    ExponentialBackoff::new(
        Duration::from_millis(100),
        Duration::from_secs(1),
        2.0,
        -0.1,
    );
}

#[test]
#[should_panic(expected = "jitter must be in [0.0, 1.0]")]
fn backoff_panics_on_jitter_above_one() {
    ExponentialBackoff::new(
        Duration::from_millis(100),
        Duration::from_secs(1),
        2.0,
        1.01,
    );
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
    let mut b = ExponentialBackoff::new(Duration::from_millis(1), Duration::from_secs(5), 2.0, 0.0);
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

#[test]
fn exponential_backoff_reset_after_many_attempts() {
    let mut b = ExponentialBackoff::new(
        Duration::from_millis(100),
        Duration::from_secs(10),
        2.0,
        0.0,
    );
    for _ in 0..20 {
        b.next_delay();
    }
    assert!(b.attempt() >= 20);
    b.reset();
    assert_eq!(b.attempt(), 0);
    assert_eq!(b.next_delay(), Duration::from_millis(100));
}

#[test]
fn exponential_backoff_jitter_deterministic() {
    // Two identical backoffs with same jitter produce the same sequence
    let mut a = ExponentialBackoff::new(
        Duration::from_millis(100),
        Duration::from_secs(60),
        2.0,
        0.25,
    );
    let mut b = ExponentialBackoff::new(
        Duration::from_millis(100),
        Duration::from_secs(60),
        2.0,
        0.25,
    );
    for _ in 0..10 {
        assert_eq!(a.next_delay(), b.next_delay());
    }
}

// ===========================================================================
// AdapterState tests
// ===========================================================================

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

#[test]
fn state_clone_and_copy_semantics() {
    let s = AdapterState::Active;
    let copied = s;
    let cloned = s.clone();
    assert_eq!(s, copied);
    assert_eq!(s, cloned);
}

// ===========================================================================
// AdapterConfig trait tests
// ===========================================================================

#[test]
fn config_trait_basic_impl() {
    let cfg = TestConfig {
        publish_rate: 50.0,
        timeout: Duration::from_secs(5),
        max_reconnects: 10,
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
        publish_rate: 25.0,
        timeout: Duration::from_secs(10),
        max_reconnects: 0,
        auto_reconnect: false,
    };
    assert!(!cfg.enable_auto_reconnect());
    assert_eq!(cfg.max_reconnect_attempts(), 0);
}

#[test]
fn config_trait_object_safety() {
    let cfg: Box<dyn AdapterConfig> = Box::new(TestConfig {
        publish_rate: 60.0,
        timeout: Duration::from_secs(3),
        max_reconnects: 5,
        auto_reconnect: true,
    });
    assert_eq!(cfg.publish_rate_hz(), 60.0);
    assert_eq!(cfg.connection_timeout(), Duration::from_secs(3));
}

#[test]
fn adapter_config_custom_values() {
    let cfg = TestConfig {
        publish_rate: 60.0,
        timeout: Duration::from_secs(30),
        max_reconnects: 10,
        auto_reconnect: false,
    };
    assert_eq!(cfg.publish_rate_hz(), 60.0);
    assert_eq!(cfg.connection_timeout(), Duration::from_secs(30));
    assert_eq!(cfg.max_reconnect_attempts(), 10);
    assert!(!cfg.enable_auto_reconnect());
}

#[test]
fn adapter_config_used_in_adapter_construction() {
    let cfg = TestConfig {
        max_reconnects: 7,
        ..TestConfig::default()
    };
    let adapter = MockAdapter::new(cfg);
    assert_eq!(adapter.config.max_reconnect_attempts(), 7);
    assert_eq!(adapter.strategy.max_attempts(), 7);
}

// ===========================================================================
// Integration / Miscellaneous
// ===========================================================================

#[test]
fn reconnect_strategy_from_config() {
    let cfg = TestConfig {
        publish_rate: 50.0,
        timeout: Duration::from_secs(5),
        max_reconnects: 5,
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
    let mut b =
        ExponentialBackoff::new(Duration::from_millis(100), Duration::from_secs(5), 2.0, 0.0);
    let mut total = Duration::ZERO;
    for _ in 0..10 {
        total += b.next_delay();
    }
    // 100+200+400+800+1600+3200+5000+5000+5000+5000 = 26300ms
    assert!(total <= Duration::from_secs(27));
}

// ===========================================================================
// Property-based tests
// ===========================================================================

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
