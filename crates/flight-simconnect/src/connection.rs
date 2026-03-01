// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! SimConnect connection management with auto-reconnect and health monitoring.
//!
//! Provides a `SimConnectConnection` state machine that manages the full
//! lifecycle from MSFS detection through connection, health monitoring, and
//! automatic reconnection with exponential back-off.

use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Connection state machine
// ---------------------------------------------------------------------------

/// Connection lifecycle states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// Not connected; no reconnection in progress.
    Disconnected,
    /// Attempting to connect (or reconnect).
    Connecting,
    /// SimConnect session is open but no sim data flowing yet.
    Connected,
    /// Receiving live sim data.
    Active,
}

/// Events that drive connection state transitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionEvent {
    /// A connection attempt has been initiated.
    ConnectAttempted,
    /// The SimConnect `Open` handshake completed successfully.
    ConnectSucceeded,
    /// A connection attempt failed with a reason string.
    ConnectFailed(String),
    /// Sim data has been received (confirms Active state).
    DataReceived,
    /// The connection has been lost.
    ConnectionLost(String),
    /// Graceful disconnect requested.
    DisconnectRequested,
}

/// Error returned when a state transition is invalid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionTransitionError {
    pub from: ConnectionState,
    pub event: String,
}

impl std::fmt::Display for ConnectionTransitionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid connection transition from {:?} on event {}",
            self.from, self.event
        )
    }
}

impl std::error::Error for ConnectionTransitionError {}

// ---------------------------------------------------------------------------
// Exponential back-off
// ---------------------------------------------------------------------------

/// Exponential back-off calculator for reconnection delays.
#[derive(Debug, Clone)]
pub struct ExponentialBackoff {
    base: Duration,
    max: Duration,
    attempt: u32,
}

impl ExponentialBackoff {
    /// Create a new back-off starting at `base`, doubling up to `max`.
    pub fn new(base: Duration, max: Duration) -> Self {
        Self {
            base,
            max,
            attempt: 0,
        }
    }

    /// Get the delay for the current attempt and advance the counter.
    pub fn next_delay(&mut self) -> Duration {
        let multiplier = 2u64.saturating_pow(self.attempt);
        let delay = self.base.saturating_mul(multiplier as u32).min(self.max);
        self.attempt += 1;
        delay
    }

    /// Reset the attempt counter (e.g. after a successful connection).
    pub fn reset(&mut self) {
        self.attempt = 0;
    }

    /// Current attempt number (0-based).
    pub fn attempt(&self) -> u32 {
        self.attempt
    }
}

// ---------------------------------------------------------------------------
// Health monitor
// ---------------------------------------------------------------------------

/// Tracks connection health based on message timestamps.
#[derive(Debug, Clone)]
pub struct HealthMonitor {
    last_message_time: Option<Instant>,
    timeout: Duration,
}

impl HealthMonitor {
    /// Create a monitor that considers the connection stale after `timeout`.
    pub fn new(timeout: Duration) -> Self {
        Self {
            last_message_time: None,
            timeout,
        }
    }

    /// Record that a message was received right now.
    pub fn record_message(&mut self) {
        self.last_message_time = Some(Instant::now());
    }

    /// Record that a message was received at a specific instant (for testing).
    pub fn record_message_at(&mut self, at: Instant) {
        self.last_message_time = Some(at);
    }

    /// Time since the last message, or `None` if no message was ever received.
    pub fn time_since_last_message(&self) -> Option<Duration> {
        self.last_message_time.map(|t| t.elapsed())
    }

    /// `true` if no message has been received within the configured timeout.
    pub fn is_stale(&self) -> bool {
        match self.last_message_time {
            Some(t) => t.elapsed() > self.timeout,
            None => false, // No message yet ≠ stale
        }
    }

    /// `true` if at least one message has been received.
    pub fn has_received(&self) -> bool {
        self.last_message_time.is_some()
    }

    /// Reset the monitor (e.g. on disconnect).
    pub fn reset(&mut self) {
        self.last_message_time = None;
    }
}

// ---------------------------------------------------------------------------
// ReconnectPolicy
// ---------------------------------------------------------------------------

/// Reconnect policy with configurable exponential back-off and retry limits.
#[derive(Debug, Clone)]
pub struct ReconnectPolicy {
    /// Maximum number of retries before giving up.
    pub max_retries: u32,
    /// Base delay in milliseconds.
    pub backoff_base_ms: u64,
    /// Maximum delay in milliseconds.
    pub backoff_max_ms: u64,
    /// Multiplier applied per attempt (e.g. 2.0 = double each time).
    pub backoff_multiplier: f64,
    /// Number of consecutive failures observed so far.
    consecutive_failures: u32,
}

impl ReconnectPolicy {
    /// Create a new policy.
    pub fn new(
        max_retries: u32,
        backoff_base_ms: u64,
        backoff_max_ms: u64,
        backoff_multiplier: f64,
    ) -> Self {
        Self {
            max_retries,
            backoff_base_ms,
            backoff_max_ms,
            backoff_multiplier,
            consecutive_failures: 0,
        }
    }

    /// Whether another retry should be attempted.
    pub fn should_retry(&self) -> bool {
        self.consecutive_failures < self.max_retries
    }

    /// Record a failed attempt.
    pub fn record_failure(&mut self) {
        self.consecutive_failures = self.consecutive_failures.saturating_add(1);
    }

    /// Reset consecutive failures (e.g. after a successful connection).
    pub fn record_success(&mut self) {
        self.consecutive_failures = 0;
    }

    /// Number of consecutive failures so far.
    pub fn consecutive_failures(&self) -> u32 {
        self.consecutive_failures
    }

    /// Compute the back-off delay for the current failure count.
    pub fn current_delay(&self) -> Duration {
        let multiplier = self.backoff_multiplier.powi(self.consecutive_failures as i32);
        let delay_ms = (self.backoff_base_ms as f64 * multiplier) as u64;
        let capped = delay_ms.min(self.backoff_max_ms);
        Duration::from_millis(capped)
    }
}

impl Default for ReconnectPolicy {
    fn default() -> Self {
        Self::new(10, 1_000, 60_000, 2.0)
    }
}

// ---------------------------------------------------------------------------
// ConnectionHealth
// ---------------------------------------------------------------------------

const LATENCY_BUFFER_SIZE: usize = 64;

/// Connection health tracker with latency sampling and error-rate monitoring.
#[derive(Debug, Clone)]
pub struct ConnectionHealth {
    last_heartbeat: Option<Instant>,
    heartbeat_timeout: Duration,
    latency_samples: [Duration; LATENCY_BUFFER_SIZE],
    latency_write_idx: usize,
    latency_count: usize,
    packet_count: u64,
    error_count: u64,
    /// Maximum error rate (errors / packets) before the connection is unhealthy.
    error_rate_threshold: f64,
}

impl ConnectionHealth {
    /// Create a new health tracker.
    pub fn new(heartbeat_timeout: Duration, error_rate_threshold: f64) -> Self {
        Self {
            last_heartbeat: None,
            heartbeat_timeout,
            latency_samples: [Duration::ZERO; LATENCY_BUFFER_SIZE],
            latency_write_idx: 0,
            latency_count: 0,
            packet_count: 0,
            error_count: 0,
            error_rate_threshold,
        }
    }

    /// Record a successful packet with its measured latency.
    pub fn record_packet(&mut self, latency: Duration) {
        self.last_heartbeat = Some(Instant::now());
        self.packet_count += 1;
        self.latency_samples[self.latency_write_idx] = latency;
        self.latency_write_idx = (self.latency_write_idx + 1) % LATENCY_BUFFER_SIZE;
        if self.latency_count < LATENCY_BUFFER_SIZE {
            self.latency_count += 1;
        }
    }

    /// Record a successful packet at a specific instant (for testing).
    pub fn record_packet_at(&mut self, latency: Duration, at: Instant) {
        self.last_heartbeat = Some(at);
        self.packet_count += 1;
        self.latency_samples[self.latency_write_idx] = latency;
        self.latency_write_idx = (self.latency_write_idx + 1) % LATENCY_BUFFER_SIZE;
        if self.latency_count < LATENCY_BUFFER_SIZE {
            self.latency_count += 1;
        }
    }

    /// Record an error.
    pub fn record_error(&mut self) {
        self.error_count += 1;
    }

    /// `true` if the connection is considered healthy (recent heartbeat + acceptable error rate).
    pub fn is_healthy(&self) -> bool {
        let heartbeat_ok = match self.last_heartbeat {
            Some(t) => t.elapsed() <= self.heartbeat_timeout,
            None => false,
        };
        let error_rate_ok = self.error_rate() <= self.error_rate_threshold;
        heartbeat_ok && error_rate_ok
    }

    /// Current error rate (errors / total traffic). Returns 0.0 when no traffic.
    pub fn error_rate(&self) -> f64 {
        let total = self.packet_count + self.error_count;
        if total == 0 {
            return 0.0;
        }
        self.error_count as f64 / total as f64
    }

    /// Average latency over the buffered samples. `None` if no samples yet.
    pub fn average_latency(&self) -> Option<Duration> {
        if self.latency_count == 0 {
            return None;
        }
        let sum: Duration = self.latency_samples[..self.latency_count].iter().sum();
        Some(sum / self.latency_count as u32)
    }

    /// Total packets successfully received.
    pub fn packet_count(&self) -> u64 {
        self.packet_count
    }

    /// Total errors recorded.
    pub fn error_count(&self) -> u64 {
        self.error_count
    }

    /// Reset all counters and samples.
    pub fn reset(&mut self) {
        self.last_heartbeat = None;
        self.latency_samples = [Duration::ZERO; LATENCY_BUFFER_SIZE];
        self.latency_write_idx = 0;
        self.latency_count = 0;
        self.packet_count = 0;
        self.error_count = 0;
    }
}

// ---------------------------------------------------------------------------
// SimConnectConnection
// ---------------------------------------------------------------------------

/// Configuration for `SimConnectConnection`.
#[derive(Debug, Clone)]
pub struct ConnectionConfig {
    /// Maximum number of reconnection attempts before giving up.
    pub max_reconnect_attempts: u32,
    /// Base delay for exponential back-off.
    pub backoff_base: Duration,
    /// Maximum delay for exponential back-off.
    pub backoff_max: Duration,
    /// Time without messages before declaring the connection stale.
    pub health_timeout: Duration,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            max_reconnect_attempts: 10,
            backoff_base: Duration::from_secs(1),
            backoff_max: Duration::from_secs(60),
            health_timeout: Duration::from_secs(10),
        }
    }
}

/// High-level SimConnect connection manager.
///
/// Encapsulates state transitions, automatic reconnection with exponential
/// back-off, and health monitoring into a single facade.
pub struct SimConnectConnection {
    state: ConnectionState,
    config: ConnectionConfig,
    backoff: ExponentialBackoff,
    health: HealthMonitor,
    connect_attempts: u32,
    total_reconnects: u32,
    last_error: Option<String>,
}

impl SimConnectConnection {
    /// Create a new connection manager in the `Disconnected` state.
    pub fn new(config: ConnectionConfig) -> Self {
        let backoff = ExponentialBackoff::new(config.backoff_base, config.backoff_max);
        let health = HealthMonitor::new(config.health_timeout);
        Self {
            state: ConnectionState::Disconnected,
            config,
            backoff,
            health,
            connect_attempts: 0,
            total_reconnects: 0,
            last_error: None,
        }
    }

    // -- state queries --

    pub fn state(&self) -> ConnectionState {
        self.state
    }

    pub fn is_connected(&self) -> bool {
        matches!(
            self.state,
            ConnectionState::Connected | ConnectionState::Active
        )
    }

    pub fn is_active(&self) -> bool {
        self.state == ConnectionState::Active
    }

    pub fn connect_attempts(&self) -> u32 {
        self.connect_attempts
    }

    pub fn total_reconnects(&self) -> u32 {
        self.total_reconnects
    }

    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    pub fn can_reconnect(&self) -> bool {
        self.connect_attempts < self.config.max_reconnect_attempts
    }

    pub fn health(&self) -> &HealthMonitor {
        &self.health
    }

    /// Get the delay to wait before the next reconnection attempt.
    pub fn next_reconnect_delay(&mut self) -> Duration {
        self.backoff.next_delay()
    }

    // -- state transitions --

    /// Initiate a connection attempt.
    pub fn connect(&mut self) -> Result<ConnectionState, ConnectionTransitionError> {
        self.transition(ConnectionEvent::ConnectAttempted)
    }

    /// Signal that the connection attempt succeeded.
    pub fn on_connected(&mut self) -> Result<ConnectionState, ConnectionTransitionError> {
        self.transition(ConnectionEvent::ConnectSucceeded)
    }

    /// Signal that a connection attempt failed.
    pub fn on_connect_failed(
        &mut self,
        reason: &str,
    ) -> Result<ConnectionState, ConnectionTransitionError> {
        self.transition(ConnectionEvent::ConnectFailed(reason.to_string()))
    }

    /// Signal that sim data was received.
    pub fn on_data_received(&mut self) -> Result<ConnectionState, ConnectionTransitionError> {
        self.transition(ConnectionEvent::DataReceived)
    }

    /// Signal that the connection was lost.
    pub fn on_connection_lost(
        &mut self,
        reason: &str,
    ) -> Result<ConnectionState, ConnectionTransitionError> {
        self.transition(ConnectionEvent::ConnectionLost(reason.to_string()))
    }

    /// Request a graceful disconnect.
    pub fn disconnect(&mut self) -> Result<ConnectionState, ConnectionTransitionError> {
        self.transition(ConnectionEvent::DisconnectRequested)
    }

    /// Attempt a reconnection (combines connect-attempt from Disconnected).
    pub fn reconnect(&mut self) -> Result<ConnectionState, ConnectionTransitionError> {
        if !self.can_reconnect() {
            return Err(ConnectionTransitionError {
                from: self.state,
                event: "reconnect: max attempts reached".to_string(),
            });
        }
        self.total_reconnects += 1;
        self.connect()
    }

    // -- internal --

    fn transition(
        &mut self,
        event: ConnectionEvent,
    ) -> Result<ConnectionState, ConnectionTransitionError> {
        use ConnectionEvent::*;
        use ConnectionState::*;

        let next = match (&self.state, &event) {
            // -- DisconnectRequested from any state --
            (_, DisconnectRequested) => {
                self.reset_internal();
                Disconnected
            }

            // -- Disconnected --
            (Disconnected, ConnectAttempted) => {
                self.connect_attempts += 1;
                Connecting
            }

            // -- Connecting --
            (Connecting, ConnectSucceeded) => {
                self.backoff.reset();
                self.connect_attempts = 0;
                Connected
            }
            (Connecting, ConnectFailed(reason)) => {
                self.last_error = Some(reason.clone());
                Disconnected
            }
            (Connecting, ConnectionLost(reason)) => {
                self.last_error = Some(reason.clone());
                Disconnected
            }

            // -- Connected --
            (Connected, DataReceived) => {
                self.health.record_message();
                Active
            }
            (Connected, ConnectionLost(reason)) => {
                self.last_error = Some(reason.clone());
                self.health.reset();
                Disconnected
            }

            // -- Active --
            (Active, DataReceived) => {
                self.health.record_message();
                Active
            }
            (Active, ConnectionLost(reason)) => {
                self.last_error = Some(reason.clone());
                self.health.reset();
                Disconnected
            }

            // Everything else is invalid.
            (from, _) => {
                return Err(ConnectionTransitionError {
                    from: *from,
                    event: format!("{event:?}"),
                });
            }
        };

        self.state = next;
        Ok(next)
    }

    fn reset_internal(&mut self) {
        self.backoff.reset();
        self.connect_attempts = 0;
        self.health.reset();
        self.last_error = None;
    }
}

impl Default for SimConnectConnection {
    fn default() -> Self {
        Self::new(ConnectionConfig::default())
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -- ConnectionState basics --

    #[test]
    fn initial_state_is_disconnected() {
        let conn = SimConnectConnection::default();
        assert_eq!(conn.state(), ConnectionState::Disconnected);
        assert!(!conn.is_connected());
        assert!(!conn.is_active());
    }

    // -- happy-path lifecycle --

    #[test]
    fn happy_path_lifecycle() {
        let mut conn = SimConnectConnection::default();

        // Disconnected → Connecting
        let s = conn.connect().unwrap();
        assert_eq!(s, ConnectionState::Connecting);

        // Connecting → Connected
        let s = conn.on_connected().unwrap();
        assert_eq!(s, ConnectionState::Connected);
        assert!(conn.is_connected());
        assert!(!conn.is_active());

        // Connected → Active
        let s = conn.on_data_received().unwrap();
        assert_eq!(s, ConnectionState::Active);
        assert!(conn.is_active());

        // Active stays Active on more data
        let s = conn.on_data_received().unwrap();
        assert_eq!(s, ConnectionState::Active);

        // Graceful disconnect
        let s = conn.disconnect().unwrap();
        assert_eq!(s, ConnectionState::Disconnected);
    }

    // -- connection failure --

    #[test]
    fn connect_failed_returns_to_disconnected() {
        let mut conn = SimConnectConnection::default();
        conn.connect().unwrap();

        let s = conn.on_connect_failed("MSFS not running").unwrap();
        assert_eq!(s, ConnectionState::Disconnected);
        assert_eq!(conn.last_error(), Some("MSFS not running"));
    }

    // -- connection lost --

    #[test]
    fn connection_lost_from_active() {
        let mut conn = SimConnectConnection::default();
        conn.connect().unwrap();
        conn.on_connected().unwrap();
        conn.on_data_received().unwrap();

        let s = conn.on_connection_lost("pipe broken").unwrap();
        assert_eq!(s, ConnectionState::Disconnected);
        assert_eq!(conn.last_error(), Some("pipe broken"));
    }

    #[test]
    fn connection_lost_from_connected() {
        let mut conn = SimConnectConnection::default();
        conn.connect().unwrap();
        conn.on_connected().unwrap();

        let s = conn.on_connection_lost("timeout").unwrap();
        assert_eq!(s, ConnectionState::Disconnected);
    }

    #[test]
    fn connection_lost_from_connecting() {
        let mut conn = SimConnectConnection::default();
        conn.connect().unwrap();

        let s = conn.on_connection_lost("refused").unwrap();
        assert_eq!(s, ConnectionState::Disconnected);
    }

    // -- disconnect from any state --

    #[test]
    fn disconnect_from_connecting() {
        let mut conn = SimConnectConnection::default();
        conn.connect().unwrap();
        let s = conn.disconnect().unwrap();
        assert_eq!(s, ConnectionState::Disconnected);
    }

    #[test]
    fn disconnect_from_connected() {
        let mut conn = SimConnectConnection::default();
        conn.connect().unwrap();
        conn.on_connected().unwrap();
        let s = conn.disconnect().unwrap();
        assert_eq!(s, ConnectionState::Disconnected);
    }

    #[test]
    fn disconnect_clears_state() {
        let mut conn = SimConnectConnection::default();
        conn.connect().unwrap();
        conn.on_connect_failed("err").unwrap();
        conn.connect().unwrap();
        conn.on_connected().unwrap();
        conn.disconnect().unwrap();
        assert_eq!(conn.connect_attempts(), 0);
        assert!(conn.last_error().is_none());
    }

    // -- invalid transitions --

    #[test]
    fn data_received_from_disconnected_is_invalid() {
        let mut conn = SimConnectConnection::default();
        let res = conn.on_data_received();
        assert!(res.is_err());
    }

    #[test]
    fn connect_succeeded_from_disconnected_is_invalid() {
        let mut conn = SimConnectConnection::default();
        let res = conn.on_connected();
        assert!(res.is_err());
    }

    #[test]
    fn connect_from_active_is_invalid() {
        let mut conn = SimConnectConnection::default();
        conn.connect().unwrap();
        conn.on_connected().unwrap();
        conn.on_data_received().unwrap();
        let res = conn.connect();
        assert!(res.is_err());
    }

    // -- reconnection --

    #[test]
    fn reconnect_increments_counter() {
        let mut conn = SimConnectConnection::default();
        conn.connect().unwrap();
        conn.on_connect_failed("err").unwrap();

        conn.reconnect().unwrap();
        assert_eq!(conn.total_reconnects(), 1);
        assert_eq!(conn.state(), ConnectionState::Connecting);
    }

    #[test]
    fn reconnect_fails_after_max_attempts() {
        let mut conn = SimConnectConnection::new(ConnectionConfig {
            max_reconnect_attempts: 2,
            ..Default::default()
        });

        // Attempt 1
        conn.connect().unwrap();
        conn.on_connect_failed("err").unwrap();

        // Attempt 2
        conn.reconnect().unwrap();
        conn.on_connect_failed("err").unwrap();

        // Attempt 3 should fail (connect_attempts is now 2 == max)
        let res = conn.reconnect();
        assert!(res.is_err());
    }

    #[test]
    fn successful_connect_resets_attempt_counter() {
        let mut conn = SimConnectConnection::default();
        conn.connect().unwrap();
        conn.on_connect_failed("err").unwrap();
        assert_eq!(conn.connect_attempts(), 1);

        conn.reconnect().unwrap();
        conn.on_connected().unwrap();
        assert_eq!(conn.connect_attempts(), 0);
    }

    // -- ExponentialBackoff --

    #[test]
    fn backoff_doubles() {
        let mut b = ExponentialBackoff::new(Duration::from_secs(1), Duration::from_secs(30));
        assert_eq!(b.next_delay(), Duration::from_secs(1));
        assert_eq!(b.next_delay(), Duration::from_secs(2));
        assert_eq!(b.next_delay(), Duration::from_secs(4));
        assert_eq!(b.next_delay(), Duration::from_secs(8));
        assert_eq!(b.next_delay(), Duration::from_secs(16));
        assert_eq!(b.next_delay(), Duration::from_secs(30)); // capped
    }

    #[test]
    fn backoff_reset() {
        let mut b = ExponentialBackoff::new(Duration::from_secs(1), Duration::from_secs(60));
        b.next_delay();
        b.next_delay();
        assert_eq!(b.attempt(), 2);
        b.reset();
        assert_eq!(b.attempt(), 0);
        assert_eq!(b.next_delay(), Duration::from_secs(1));
    }

    #[test]
    fn backoff_caps_at_max() {
        let mut b = ExponentialBackoff::new(Duration::from_secs(10), Duration::from_secs(20));
        assert_eq!(b.next_delay(), Duration::from_secs(10));
        assert_eq!(b.next_delay(), Duration::from_secs(20)); // capped
        assert_eq!(b.next_delay(), Duration::from_secs(20)); // still capped
    }

    // -- HealthMonitor --

    #[test]
    fn health_monitor_initial_state() {
        let hm = HealthMonitor::new(Duration::from_secs(5));
        assert!(!hm.is_stale());
        assert!(!hm.has_received());
        assert!(hm.time_since_last_message().is_none());
    }

    #[test]
    fn health_monitor_records_message() {
        let mut hm = HealthMonitor::new(Duration::from_secs(5));
        hm.record_message();
        assert!(hm.has_received());
        assert!(!hm.is_stale());
        assert!(hm.time_since_last_message().unwrap() < Duration::from_secs(1));
    }

    #[test]
    fn health_monitor_detects_stale() {
        let mut hm = HealthMonitor::new(Duration::from_millis(1));
        hm.record_message();
        std::thread::sleep(Duration::from_millis(5));
        assert!(hm.is_stale());
    }

    #[test]
    fn health_monitor_reset() {
        let mut hm = HealthMonitor::new(Duration::from_secs(5));
        hm.record_message();
        hm.reset();
        assert!(!hm.has_received());
    }

    // -- reconnect timing integration --

    #[test]
    fn reconnect_delay_increases() {
        let mut conn = SimConnectConnection::new(ConnectionConfig {
            backoff_base: Duration::from_millis(100),
            backoff_max: Duration::from_secs(5),
            ..Default::default()
        });

        let d1 = conn.next_reconnect_delay();
        let d2 = conn.next_reconnect_delay();
        let d3 = conn.next_reconnect_delay();
        assert!(d2 > d1, "d2={d2:?} should exceed d1={d1:?}");
        assert!(d3 > d2, "d3={d3:?} should exceed d2={d2:?}");
    }

    // -- full reconnect scenario --

    #[test]
    fn full_reconnect_scenario() {
        let mut conn = SimConnectConnection::new(ConnectionConfig {
            max_reconnect_attempts: 5,
            ..Default::default()
        });

        // Initial successful connection
        conn.connect().unwrap();
        conn.on_connected().unwrap();
        conn.on_data_received().unwrap();
        assert!(conn.is_active());

        // Connection lost
        conn.on_connection_lost("pipe broken").unwrap();
        assert_eq!(conn.state(), ConnectionState::Disconnected);

        // Reconnect successfully
        conn.reconnect().unwrap();
        conn.on_connected().unwrap();
        conn.on_data_received().unwrap();
        assert!(conn.is_active());
        assert_eq!(conn.total_reconnects(), 1);
    }

    // ===================================================================
    // ReconnectPolicy tests
    // ===================================================================

    #[test]
    fn reconnect_policy_should_retry_initially() {
        let policy = ReconnectPolicy::new(3, 100, 5000, 2.0);
        assert!(policy.should_retry());
        assert_eq!(policy.consecutive_failures(), 0);
    }

    #[test]
    fn reconnect_policy_stops_after_max_retries() {
        let mut policy = ReconnectPolicy::new(3, 100, 5000, 2.0);
        policy.record_failure();
        policy.record_failure();
        policy.record_failure();
        assert!(!policy.should_retry());
    }

    #[test]
    fn reconnect_policy_success_resets_failures() {
        let mut policy = ReconnectPolicy::new(3, 100, 5000, 2.0);
        policy.record_failure();
        policy.record_failure();
        assert_eq!(policy.consecutive_failures(), 2);
        policy.record_success();
        assert_eq!(policy.consecutive_failures(), 0);
        assert!(policy.should_retry());
    }

    #[test]
    fn reconnect_policy_backoff_calculation() {
        let mut policy = ReconnectPolicy::new(10, 100, 60_000, 2.0);
        // attempt 0 → 100ms * 2^0 = 100ms
        assert_eq!(policy.current_delay(), Duration::from_millis(100));
        policy.record_failure();
        // attempt 1 → 100ms * 2^1 = 200ms
        assert_eq!(policy.current_delay(), Duration::from_millis(200));
        policy.record_failure();
        // attempt 2 → 100ms * 2^2 = 400ms
        assert_eq!(policy.current_delay(), Duration::from_millis(400));
    }

    #[test]
    fn reconnect_policy_backoff_caps_at_max() {
        let mut policy = ReconnectPolicy::new(20, 1000, 5000, 2.0);
        for _ in 0..10 {
            policy.record_failure();
        }
        assert!(policy.current_delay() <= Duration::from_millis(5000));
    }

    #[test]
    fn reconnect_policy_custom_multiplier() {
        let mut policy = ReconnectPolicy::new(10, 100, 100_000, 3.0);
        // attempt 0 → 100ms
        assert_eq!(policy.current_delay(), Duration::from_millis(100));
        policy.record_failure();
        // attempt 1 → 300ms
        assert_eq!(policy.current_delay(), Duration::from_millis(300));
        policy.record_failure();
        // attempt 2 → 900ms
        assert_eq!(policy.current_delay(), Duration::from_millis(900));
    }

    #[test]
    fn reconnect_policy_default() {
        let policy = ReconnectPolicy::default();
        assert_eq!(policy.max_retries, 10);
        assert_eq!(policy.backoff_base_ms, 1_000);
        assert_eq!(policy.backoff_max_ms, 60_000);
        assert!(policy.should_retry());
    }

    // ===================================================================
    // ConnectionHealth tests
    // ===================================================================

    #[test]
    fn connection_health_initially_unhealthy() {
        let health = ConnectionHealth::new(Duration::from_secs(5), 0.5);
        assert!(!health.is_healthy());
        assert_eq!(health.packet_count(), 0);
        assert_eq!(health.error_count(), 0);
        assert!(health.average_latency().is_none());
    }

    #[test]
    fn connection_health_healthy_after_packet() {
        let mut health = ConnectionHealth::new(Duration::from_secs(5), 0.5);
        health.record_packet(Duration::from_millis(10));
        assert!(health.is_healthy());
        assert_eq!(health.packet_count(), 1);
    }

    #[test]
    fn connection_health_average_latency() {
        let mut health = ConnectionHealth::new(Duration::from_secs(5), 0.5);
        health.record_packet(Duration::from_millis(10));
        health.record_packet(Duration::from_millis(20));
        health.record_packet(Duration::from_millis(30));
        let avg = health.average_latency().unwrap();
        assert_eq!(avg, Duration::from_millis(20));
    }

    #[test]
    fn connection_health_error_rate_degrades_health() {
        let mut health = ConnectionHealth::new(Duration::from_secs(5), 0.1);
        health.record_packet(Duration::from_millis(5));
        // 1 packet, 0 errors → healthy
        assert!(health.is_healthy());

        // Add errors to exceed 10% threshold
        health.record_error();
        health.record_error();
        // 1 packet + 2 errors = 3 total, error_rate = 2/3 ≈ 0.67 > 0.1
        assert!(!health.is_healthy());
    }

    #[test]
    fn connection_health_error_rate_calculation() {
        let mut health = ConnectionHealth::new(Duration::from_secs(5), 0.5);
        health.record_packet(Duration::from_millis(5));
        health.record_packet(Duration::from_millis(5));
        health.record_packet(Duration::from_millis(5));
        health.record_error();
        // 3 packets + 1 error = 4 total, rate = 1/4 = 0.25
        let rate = health.error_rate();
        assert!((rate - 0.25).abs() < f64::EPSILON);
    }

    #[test]
    fn connection_health_stale_heartbeat_unhealthy() {
        let mut health = ConnectionHealth::new(Duration::from_millis(1), 0.5);
        health.record_packet(Duration::from_millis(5));
        std::thread::sleep(Duration::from_millis(5));
        assert!(!health.is_healthy());
    }

    #[test]
    fn connection_health_circular_buffer_wraps() {
        let mut health = ConnectionHealth::new(Duration::from_secs(5), 0.5);
        // Fill the buffer (64 entries) and overflow
        for i in 0..100 {
            health.record_packet(Duration::from_millis(i));
        }
        assert_eq!(health.packet_count(), 100);
        // Buffer size is capped at LATENCY_BUFFER_SIZE
        let avg = health.average_latency().unwrap();
        // Last 64 samples are 36..100, avg = (36+99)/2 = 67.5ms
        assert!(avg > Duration::from_millis(30));
    }

    #[test]
    fn connection_health_reset_clears_all() {
        let mut health = ConnectionHealth::new(Duration::from_secs(5), 0.5);
        health.record_packet(Duration::from_millis(10));
        health.record_error();
        health.reset();
        assert_eq!(health.packet_count(), 0);
        assert_eq!(health.error_count(), 0);
        assert!(health.average_latency().is_none());
        assert!(!health.is_healthy());
    }

    // ===================================================================
    // Integration: state machine + reconnect policy
    // ===================================================================

    #[test]
    fn state_machine_with_reconnect_policy_interaction() {
        let mut conn = SimConnectConnection::default();
        let mut policy = ReconnectPolicy::new(3, 100, 5000, 2.0);

        // Successful initial connection
        conn.connect().unwrap();
        conn.on_connected().unwrap();
        conn.on_data_received().unwrap();
        assert!(conn.is_active());

        // Connection lost — policy decides retry
        conn.on_connection_lost("network error").unwrap();
        policy.record_failure();
        assert!(policy.should_retry());

        // Reconnect with policy delay
        let delay = policy.current_delay();
        assert!(delay > Duration::ZERO);
        conn.reconnect().unwrap();
        conn.on_connected().unwrap();
        conn.on_data_received().unwrap();
        policy.record_success();
        assert!(conn.is_active());
        assert_eq!(policy.consecutive_failures(), 0);
    }

    #[test]
    fn state_machine_with_health_tracking() {
        let mut conn = SimConnectConnection::default();
        let mut health = ConnectionHealth::new(Duration::from_secs(5), 0.2);

        conn.connect().unwrap();
        conn.on_connected().unwrap();
        conn.on_data_received().unwrap();

        // Simulate active data flow with health tracking
        for _ in 0..10 {
            conn.on_data_received().unwrap();
            health.record_packet(Duration::from_millis(8));
        }
        assert!(conn.is_active());
        assert!(health.is_healthy());
        assert_eq!(health.packet_count(), 10);

        // Errors degrade health
        for _ in 0..5 {
            health.record_error();
        }
        // 10 packets + 5 errors = 15 total, rate = 5/15 ≈ 0.33 > 0.2
        assert!(!health.is_healthy());
    }

    #[test]
    fn reconnect_policy_exhaustion_with_state_machine() {
        let mut conn = SimConnectConnection::new(ConnectionConfig {
            max_reconnect_attempts: 5,
            ..Default::default()
        });
        let mut policy = ReconnectPolicy::new(2, 100, 5000, 2.0);

        // First failure
        conn.connect().unwrap();
        conn.on_connect_failed("timeout").unwrap();
        policy.record_failure();
        assert!(policy.should_retry());

        // Second failure
        conn.reconnect().unwrap();
        conn.on_connect_failed("timeout").unwrap();
        policy.record_failure();
        assert!(!policy.should_retry());

        // Policy says stop — no more retries
        assert_eq!(policy.consecutive_failures(), 2);
    }
}
