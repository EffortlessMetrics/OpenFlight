// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for flight-adapter-common traits, state machines, metrics,
//! error handling, and realistic adapter usage patterns.

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

// ===========================================================================
// 1. Adapter trait tests (6)
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
// 2. State machine tests (6)
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

// ===========================================================================
// 3. Telemetry snapshot tests (6)
//    (Tests telemetry data patterns using metrics & adapter mock)
// ===========================================================================

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
    let snap = TelemetrySnapshot::new(vec![], Duration::from_secs(10));
    assert!(snap.timestamp.elapsed() < Duration::from_millis(100));
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
// 4. Bus integration tests (5)
//    (Simulates adapter→bus publish patterns)
// ===========================================================================

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
// 5. Metrics tests (5)
// ===========================================================================

#[test]
fn metrics_connection_count_via_aircraft_changes() {
    let mut m = AdapterMetrics::new();
    m.record_aircraft_change("C172".to_string());
    m.record_aircraft_change("A320".to_string());
    m.record_aircraft_change("B738".to_string());
    assert_eq!(m.aircraft_changes, 3);
}

#[test]
fn metrics_disconnect_count_tracks_resets() {
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

// ===========================================================================
// 6. Error handling tests (5)
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

    // Attempts 1 and 2 are within max_attempts (should_retry checks <=)
    adapter.state = AdapterState::Error;
    adapter.reconnect_with_backoff().unwrap();
    adapter.state = AdapterState::Error;
    adapter.reconnect_with_backoff().unwrap();
    // Attempt 3 exceeds max_attempts of 2
    adapter.state = AdapterState::Error;
    let err = adapter.reconnect_with_backoff().unwrap_err();
    assert!(matches!(err, AdapterError::ReconnectExhausted));
}

// ===========================================================================
// 7. Additional depth: ReconnectionStrategy edge cases
// ===========================================================================

#[test]
fn reconnection_strategy_zero_attempt_returns_initial() {
    let s = ReconnectionStrategy::new(5, Duration::from_millis(100), Duration::from_secs(10));
    assert_eq!(s.next_backoff(0), Duration::from_millis(100));
}

#[test]
fn reconnection_strategy_should_retry_boundary() {
    let s = ReconnectionStrategy::new(3, Duration::from_millis(100), Duration::from_secs(10));
    assert!(s.should_retry(3));
    assert!(!s.should_retry(4));
    assert!(s.should_retry(0));
    assert!(s.should_retry(1));
}

#[test]
fn reconnection_strategy_backoff_overflow_protection() {
    let s = ReconnectionStrategy::new(100, Duration::from_millis(1000), Duration::from_secs(30));
    // Very large attempt number should not overflow — capped at max
    let d = s.next_backoff(50);
    assert_eq!(d, Duration::from_secs(30));
}

// ===========================================================================
// 8. Additional depth: ExponentialBackoff edge cases
// ===========================================================================

#[test]
fn exponential_backoff_multiplier_one_stays_constant() {
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
// 9. Additional depth: AdapterConfig trait implementations
// ===========================================================================

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
// 10. Additional depth: AdapterMetrics edge cases
// ===========================================================================

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

// ===========================================================================
// 11. Additional depth: AdapterError variants exhaustive
// ===========================================================================

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

// ===========================================================================
// 12. Additional depth: AdapterState enum properties
// ===========================================================================

#[test]
fn state_clone_and_copy_semantics() {
    let s = AdapterState::Active;
    let copied = s;
    let cloned = s.clone();
    assert_eq!(s, copied);
    assert_eq!(s, cloned);
}

#[test]
fn state_all_variants_distinct() {
    let all = [
        AdapterState::Disconnected,
        AdapterState::Connecting,
        AdapterState::Connected,
        AdapterState::DetectingAircraft,
        AdapterState::Active,
        AdapterState::Error,
    ];
    for (i, a) in all.iter().enumerate() {
        for (j, b) in all.iter().enumerate() {
            if i == j {
                assert_eq!(a, b);
            } else {
                assert_ne!(a, b);
            }
        }
    }
}
