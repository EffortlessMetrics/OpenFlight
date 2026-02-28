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
}
