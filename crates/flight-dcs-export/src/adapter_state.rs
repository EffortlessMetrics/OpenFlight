// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! DCS adapter state machine with validated transitions.
//!
//! Provides a strict state machine that enforces valid adapter lifecycle
//! transitions and tracks error/retry counts for reconnection logic.
//! Follows the same pattern as the X-Plane adapter state machine.

use std::time::{Duration, Instant};
use thiserror::Error;

/// DCS adapter lifecycle states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DcsAdapterState {
    /// Not connected to DCS.
    Disconnected,
    /// Socket bound, waiting for DCS Export.lua connection.
    Connecting,
    /// Socket ready and actively listening for incoming DCS data.
    Listening,
    /// Handshake completed, waiting for first telemetry.
    Connected,
    /// Receiving valid telemetry from DCS.
    Active,
    /// Telemetry timeout — no packets within threshold.
    Stale,
    /// Unrecoverable or retry-exhausted error.
    Error,
}

impl std::fmt::Display for DcsAdapterState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DcsAdapterState::Disconnected => write!(f, "Disconnected"),
            DcsAdapterState::Connecting => write!(f, "Connecting"),
            DcsAdapterState::Listening => write!(f, "Listening"),
            DcsAdapterState::Connected => write!(f, "Connected"),
            DcsAdapterState::Active => write!(f, "Active"),
            DcsAdapterState::Stale => write!(f, "Stale"),
            DcsAdapterState::Error => write!(f, "Error"),
        }
    }
}

/// Events that drive state transitions.
#[derive(Debug, Clone)]
pub enum DcsAdapterEvent {
    /// TCP/UDP socket successfully bound and listening.
    SocketBound,
    /// Socket is ready and actively listening for incoming data (UDP).
    ListeningStarted,
    /// DCS Export.lua handshake completed successfully.
    HandshakeCompleted,
    /// Valid telemetry packet received from DCS.
    TelemetryReceived,
    /// No telemetry within the stale threshold.
    TelemetryTimeout,
    /// Stale count exceeded `max_stale_before_disconnect`.
    StaleExhausted,
    /// Socket-level or connection error.
    ConnectionError(String),
    /// DCS process exited or Export.lua disconnected.
    DcsDisconnected,
    /// Graceful shutdown requested.
    Shutdown,
}

/// Error returned when a transition is invalid.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum DcsTransitionError {
    #[error("invalid transition from {from} on event {event}")]
    InvalidTransition {
        from: DcsAdapterState,
        event: String,
    },
    #[error("retry limit reached ({max_retries} retries exhausted)")]
    RetriesExhausted { max_retries: u32 },
}

/// State machine that enforces valid DCS adapter lifecycle transitions.
///
/// Tracks error counts, retry limits, and reconnection backoff for
/// robust handling of DCS restarts and network interruptions.
pub struct DcsAdapterStateMachine {
    state: DcsAdapterState,
    last_transition: Option<Instant>,
    stale_threshold_ms: u64,
    error_count: u32,
    max_retries: u32,
    consecutive_stale_count: u32,
    max_stale_before_disconnect: u32,
    reconnect_delay: Duration,
    base_reconnect_delay: Duration,
    max_reconnect_delay: Duration,
}

impl DcsAdapterStateMachine {
    /// Create a new state machine starting in `Disconnected`.
    pub fn new(stale_threshold_ms: u64, max_retries: u32) -> Self {
        Self {
            state: DcsAdapterState::Disconnected,
            last_transition: None,
            stale_threshold_ms,
            error_count: 0,
            max_retries,
            consecutive_stale_count: 0,
            max_stale_before_disconnect: 10,
            reconnect_delay: Duration::from_secs(1),
            base_reconnect_delay: Duration::from_secs(1),
            max_reconnect_delay: Duration::from_secs(30),
        }
    }

    /// Create with a custom `max_stale_before_disconnect` threshold.
    pub fn with_max_stale(mut self, max_stale: u32) -> Self {
        self.max_stale_before_disconnect = max_stale;
        self
    }

    /// Maximum consecutive stale timeouts before auto-disconnect.
    pub fn max_stale_before_disconnect(&self) -> u32 {
        self.max_stale_before_disconnect
    }

    /// Whether the stale count has reached the disconnect threshold.
    pub fn is_stale_exhausted(&self) -> bool {
        self.consecutive_stale_count >= self.max_stale_before_disconnect
    }

    /// Current state.
    pub fn state(&self) -> DcsAdapterState {
        self.state
    }

    /// Stale threshold in milliseconds.
    pub fn stale_threshold_ms(&self) -> u64 {
        self.stale_threshold_ms
    }

    /// Number of consecutive errors observed.
    pub fn error_count(&self) -> u32 {
        self.error_count
    }

    /// Maximum retries before the machine refuses recovery.
    pub fn max_retries(&self) -> u32 {
        self.max_retries
    }

    /// Number of consecutive stale timeouts.
    pub fn consecutive_stale_count(&self) -> u32 {
        self.consecutive_stale_count
    }

    /// Current reconnection delay (increases with exponential backoff).
    pub fn reconnect_delay(&self) -> Duration {
        self.reconnect_delay
    }

    /// `true` when state is `Connected` or `Active`.
    pub fn is_healthy(&self) -> bool {
        matches!(
            self.state,
            DcsAdapterState::Connected | DcsAdapterState::Active
        )
    }

    /// `true` when `error_count < max_retries`.
    pub fn is_recoverable(&self) -> bool {
        self.error_count < self.max_retries
    }

    /// `true` when the adapter should attempt reconnection.
    pub fn should_reconnect(&self) -> bool {
        matches!(
            self.state,
            DcsAdapterState::Disconnected | DcsAdapterState::Error
        ) && self.is_recoverable()
    }

    /// Reset to `Disconnected` and clear error/stale counters.
    pub fn reset(&mut self) {
        self.state = DcsAdapterState::Disconnected;
        self.last_transition = Some(Instant::now());
        self.error_count = 0;
        self.consecutive_stale_count = 0;
        self.reconnect_delay = self.base_reconnect_delay;
    }

    /// Duration since the last state transition, or `None` if no transition yet.
    pub fn time_in_state(&self) -> Option<Duration> {
        self.last_transition.map(|t| t.elapsed())
    }

    /// Attempt a state transition driven by `event`.
    ///
    /// Returns the new state on success, or a `DcsTransitionError` if the
    /// transition is not allowed from the current state.
    pub fn transition(
        &mut self,
        event: DcsAdapterEvent,
    ) -> Result<DcsAdapterState, DcsTransitionError> {
        use DcsAdapterEvent::*;
        use DcsAdapterState::*;

        let next = match (&self.state, &event) {
            // Shutdown from any state → Disconnected
            (_, Shutdown) => {
                self.error_count = 0;
                self.consecutive_stale_count = 0;
                self.reconnect_delay = self.base_reconnect_delay;
                Disconnected
            }

            // ConnectionError from any state → Error
            (_, ConnectionError(_)) => {
                self.error_count += 1;
                self.consecutive_stale_count = 0;
                self.bump_reconnect_delay();
                Error
            }

            // DcsDisconnected from connected states → Disconnected
            (Listening | Connected | Active | Stale, DcsDisconnected) => {
                self.consecutive_stale_count = 0;
                Disconnected
            }

            // Disconnected → Connecting (socket bound)
            (Disconnected, SocketBound) => Connecting,

            // Connecting → Listening (UDP socket ready, waiting for data)
            (Connecting, ListeningStarted) => Listening,

            // Listening → Connected (handshake done via TCP or identified protocol)
            (Listening, HandshakeCompleted) => {
                self.consecutive_stale_count = 0;
                Connected
            }

            // Listening → Active (UDP shortcut: first telemetry without handshake)
            (Listening, TelemetryReceived) => {
                self.error_count = 0;
                self.consecutive_stale_count = 0;
                self.reconnect_delay = self.base_reconnect_delay;
                Active
            }

            // Connecting → Connected (handshake done)
            (Connecting, HandshakeCompleted) => {
                self.consecutive_stale_count = 0;
                Connected
            }

            // Connected → Active (first telemetry)
            (Connected, TelemetryReceived) => {
                self.error_count = 0;
                self.consecutive_stale_count = 0;
                self.reconnect_delay = self.base_reconnect_delay;
                Active
            }

            // Active → Active (continuous telemetry)
            (Active, TelemetryReceived) => Active,

            // Active → Stale (timeout)
            (Active, TelemetryTimeout) => {
                self.consecutive_stale_count = 1;
                Stale
            }

            // Stale → Stale (repeated timeout)
            (Stale, TelemetryTimeout) => {
                self.consecutive_stale_count += 1;
                Stale
            }

            // Stale → Active (recovery)
            (Stale, TelemetryReceived) => {
                self.error_count = 0;
                self.consecutive_stale_count = 0;
                self.reconnect_delay = self.base_reconnect_delay;
                Active
            }

            // Stale → Disconnected (stale count exhausted)
            (Stale, StaleExhausted) => {
                self.consecutive_stale_count = 0;
                Disconnected
            }

            // Error → Connecting (retry if allowed)
            (Error, SocketBound) => {
                if self.is_recoverable() {
                    Connecting
                } else {
                    return Err(DcsTransitionError::RetriesExhausted {
                        max_retries: self.max_retries,
                    });
                }
            }

            // Everything else is invalid
            (from, _) => {
                return Err(DcsTransitionError::InvalidTransition {
                    from: *from,
                    event: format!("{event:?}"),
                });
            }
        };

        self.state = next;
        self.last_transition = Some(Instant::now());
        Ok(next)
    }

    /// Apply exponential backoff to reconnection delay.
    fn bump_reconnect_delay(&mut self) {
        self.reconnect_delay = (self.reconnect_delay * 2).min(self.max_reconnect_delay);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sm() -> DcsAdapterStateMachine {
        DcsAdapterStateMachine::new(2000, 3)
    }

    // --- happy-path transitions ---

    #[test]
    fn disconnected_to_connecting_on_socket_bound() {
        let mut sm = sm();
        let next = sm.transition(DcsAdapterEvent::SocketBound).unwrap();
        assert_eq!(next, DcsAdapterState::Connecting);
    }

    #[test]
    fn connecting_to_connected_on_handshake() {
        let mut sm = sm();
        sm.transition(DcsAdapterEvent::SocketBound).unwrap();
        let next = sm.transition(DcsAdapterEvent::HandshakeCompleted).unwrap();
        assert_eq!(next, DcsAdapterState::Connected);
    }

    #[test]
    fn connected_to_active_on_telemetry() {
        let mut sm = sm();
        sm.transition(DcsAdapterEvent::SocketBound).unwrap();
        sm.transition(DcsAdapterEvent::HandshakeCompleted).unwrap();
        let next = sm.transition(DcsAdapterEvent::TelemetryReceived).unwrap();
        assert_eq!(next, DcsAdapterState::Active);
    }

    #[test]
    fn active_stays_active_on_telemetry() {
        let mut sm = sm();
        sm.transition(DcsAdapterEvent::SocketBound).unwrap();
        sm.transition(DcsAdapterEvent::HandshakeCompleted).unwrap();
        sm.transition(DcsAdapterEvent::TelemetryReceived).unwrap();
        let next = sm.transition(DcsAdapterEvent::TelemetryReceived).unwrap();
        assert_eq!(next, DcsAdapterState::Active);
    }

    #[test]
    fn active_to_stale_on_timeout() {
        let mut sm = sm();
        sm.transition(DcsAdapterEvent::SocketBound).unwrap();
        sm.transition(DcsAdapterEvent::HandshakeCompleted).unwrap();
        sm.transition(DcsAdapterEvent::TelemetryReceived).unwrap();
        let next = sm.transition(DcsAdapterEvent::TelemetryTimeout).unwrap();
        assert_eq!(next, DcsAdapterState::Stale);
    }

    #[test]
    fn stale_to_active_on_telemetry() {
        let mut sm = sm();
        sm.transition(DcsAdapterEvent::SocketBound).unwrap();
        sm.transition(DcsAdapterEvent::HandshakeCompleted).unwrap();
        sm.transition(DcsAdapterEvent::TelemetryReceived).unwrap();
        sm.transition(DcsAdapterEvent::TelemetryTimeout).unwrap();
        let next = sm.transition(DcsAdapterEvent::TelemetryReceived).unwrap();
        assert_eq!(next, DcsAdapterState::Active);
        assert_eq!(sm.error_count(), 0);
        assert_eq!(sm.consecutive_stale_count(), 0);
    }

    #[test]
    fn stale_stays_stale_on_repeated_timeout() {
        let mut sm = sm();
        sm.transition(DcsAdapterEvent::SocketBound).unwrap();
        sm.transition(DcsAdapterEvent::HandshakeCompleted).unwrap();
        sm.transition(DcsAdapterEvent::TelemetryReceived).unwrap();
        sm.transition(DcsAdapterEvent::TelemetryTimeout).unwrap();
        let next = sm.transition(DcsAdapterEvent::TelemetryTimeout).unwrap();
        assert_eq!(next, DcsAdapterState::Stale);
        assert_eq!(sm.consecutive_stale_count(), 2);
    }

    // --- DCS disconnect transitions ---

    #[test]
    fn active_to_disconnected_on_dcs_disconnect() {
        let mut sm = sm();
        sm.transition(DcsAdapterEvent::SocketBound).unwrap();
        sm.transition(DcsAdapterEvent::HandshakeCompleted).unwrap();
        sm.transition(DcsAdapterEvent::TelemetryReceived).unwrap();
        let next = sm.transition(DcsAdapterEvent::DcsDisconnected).unwrap();
        assert_eq!(next, DcsAdapterState::Disconnected);
    }

    #[test]
    fn connected_to_disconnected_on_dcs_disconnect() {
        let mut sm = sm();
        sm.transition(DcsAdapterEvent::SocketBound).unwrap();
        sm.transition(DcsAdapterEvent::HandshakeCompleted).unwrap();
        let next = sm.transition(DcsAdapterEvent::DcsDisconnected).unwrap();
        assert_eq!(next, DcsAdapterState::Disconnected);
    }

    #[test]
    fn stale_to_disconnected_on_dcs_disconnect() {
        let mut sm = sm();
        sm.transition(DcsAdapterEvent::SocketBound).unwrap();
        sm.transition(DcsAdapterEvent::HandshakeCompleted).unwrap();
        sm.transition(DcsAdapterEvent::TelemetryReceived).unwrap();
        sm.transition(DcsAdapterEvent::TelemetryTimeout).unwrap();
        let next = sm.transition(DcsAdapterEvent::DcsDisconnected).unwrap();
        assert_eq!(next, DcsAdapterState::Disconnected);
    }

    // --- error & recovery ---

    #[test]
    fn any_state_to_error_on_connection_error() {
        let mut sm = sm();
        sm.transition(DcsAdapterEvent::SocketBound).unwrap();
        sm.transition(DcsAdapterEvent::HandshakeCompleted).unwrap();
        sm.transition(DcsAdapterEvent::TelemetryReceived).unwrap();
        let next = sm
            .transition(DcsAdapterEvent::ConnectionError("test".into()))
            .unwrap();
        assert_eq!(next, DcsAdapterState::Error);
        assert_eq!(sm.error_count(), 1);
    }

    #[test]
    fn error_to_connecting_on_socket_bound_if_recoverable() {
        let mut sm = sm();
        sm.transition(DcsAdapterEvent::ConnectionError("err".into()))
            .unwrap();
        assert_eq!(sm.state(), DcsAdapterState::Error);
        let next = sm.transition(DcsAdapterEvent::SocketBound).unwrap();
        assert_eq!(next, DcsAdapterState::Connecting);
    }

    #[test]
    fn error_retries_exhausted() {
        let mut sm = DcsAdapterStateMachine::new(2000, 1);
        sm.transition(DcsAdapterEvent::ConnectionError("e1".into()))
            .unwrap();
        let res = sm.transition(DcsAdapterEvent::SocketBound);
        assert!(matches!(
            res,
            Err(DcsTransitionError::RetriesExhausted { max_retries: 1 })
        ));
    }

    #[test]
    fn multiple_errors_increment_count() {
        let mut sm = DcsAdapterStateMachine::new(2000, 10);
        // Each error increments; recovery resets count to 0
        sm.transition(DcsAdapterEvent::ConnectionError("e0".into()))
            .unwrap();
        assert_eq!(sm.error_count(), 1);

        sm.transition(DcsAdapterEvent::SocketBound).unwrap();
        sm.transition(DcsAdapterEvent::HandshakeCompleted).unwrap();
        sm.transition(DcsAdapterEvent::TelemetryReceived).unwrap();
        // Recovery resets error count
        assert_eq!(sm.error_count(), 0);

        // Consecutive errors without recovery accumulate
        sm.transition(DcsAdapterEvent::ConnectionError("e1".into()))
            .unwrap();
        assert_eq!(sm.error_count(), 1);
        // From Error, SocketBound → Connecting (doesn't reset count)
        sm.transition(DcsAdapterEvent::SocketBound).unwrap();
        sm.transition(DcsAdapterEvent::ConnectionError("e2".into()))
            .unwrap();
        assert_eq!(sm.error_count(), 2);
        sm.transition(DcsAdapterEvent::SocketBound).unwrap();
        sm.transition(DcsAdapterEvent::ConnectionError("e3".into()))
            .unwrap();
        assert_eq!(sm.error_count(), 3);
    }

    // --- shutdown from any ---

    #[test]
    fn shutdown_from_active() {
        let mut sm = sm();
        sm.transition(DcsAdapterEvent::SocketBound).unwrap();
        sm.transition(DcsAdapterEvent::HandshakeCompleted).unwrap();
        sm.transition(DcsAdapterEvent::TelemetryReceived).unwrap();
        let next = sm.transition(DcsAdapterEvent::Shutdown).unwrap();
        assert_eq!(next, DcsAdapterState::Disconnected);
    }

    #[test]
    fn shutdown_from_error_clears_error_count() {
        let mut sm = sm();
        sm.transition(DcsAdapterEvent::ConnectionError("e".into()))
            .unwrap();
        assert_eq!(sm.error_count(), 1);
        sm.transition(DcsAdapterEvent::Shutdown).unwrap();
        assert_eq!(sm.error_count(), 0);
        assert_eq!(sm.state(), DcsAdapterState::Disconnected);
    }

    #[test]
    fn shutdown_from_stale() {
        let mut sm = sm();
        sm.transition(DcsAdapterEvent::SocketBound).unwrap();
        sm.transition(DcsAdapterEvent::HandshakeCompleted).unwrap();
        sm.transition(DcsAdapterEvent::TelemetryReceived).unwrap();
        sm.transition(DcsAdapterEvent::TelemetryTimeout).unwrap();
        let next = sm.transition(DcsAdapterEvent::Shutdown).unwrap();
        assert_eq!(next, DcsAdapterState::Disconnected);
        assert_eq!(sm.consecutive_stale_count(), 0);
    }

    #[test]
    fn shutdown_from_connecting() {
        let mut sm = sm();
        sm.transition(DcsAdapterEvent::SocketBound).unwrap();
        let next = sm.transition(DcsAdapterEvent::Shutdown).unwrap();
        assert_eq!(next, DcsAdapterState::Disconnected);
    }

    // --- invalid transitions ---

    #[test]
    fn disconnected_rejects_telemetry() {
        let mut sm = sm();
        let res = sm.transition(DcsAdapterEvent::TelemetryReceived);
        assert!(matches!(
            res,
            Err(DcsTransitionError::InvalidTransition { .. })
        ));
    }

    #[test]
    fn connecting_rejects_timeout() {
        let mut sm = sm();
        sm.transition(DcsAdapterEvent::SocketBound).unwrap();
        let res = sm.transition(DcsAdapterEvent::TelemetryTimeout);
        assert!(matches!(
            res,
            Err(DcsTransitionError::InvalidTransition { .. })
        ));
    }

    #[test]
    fn connecting_rejects_telemetry() {
        let mut sm = sm();
        sm.transition(DcsAdapterEvent::SocketBound).unwrap();
        let res = sm.transition(DcsAdapterEvent::TelemetryReceived);
        assert!(matches!(
            res,
            Err(DcsTransitionError::InvalidTransition { .. })
        ));
    }

    #[test]
    fn disconnected_rejects_handshake() {
        let mut sm = sm();
        let res = sm.transition(DcsAdapterEvent::HandshakeCompleted);
        assert!(matches!(
            res,
            Err(DcsTransitionError::InvalidTransition { .. })
        ));
    }

    #[test]
    fn connected_rejects_timeout() {
        let mut sm = sm();
        sm.transition(DcsAdapterEvent::SocketBound).unwrap();
        sm.transition(DcsAdapterEvent::HandshakeCompleted).unwrap();
        let res = sm.transition(DcsAdapterEvent::TelemetryTimeout);
        assert!(matches!(
            res,
            Err(DcsTransitionError::InvalidTransition { .. })
        ));
    }

    // --- helper methods ---

    #[test]
    fn is_healthy_true_when_connected_or_active() {
        let mut sm = sm();
        assert!(!sm.is_healthy());
        sm.transition(DcsAdapterEvent::SocketBound).unwrap();
        assert!(!sm.is_healthy()); // Connecting
        sm.transition(DcsAdapterEvent::HandshakeCompleted).unwrap();
        assert!(sm.is_healthy()); // Connected
        sm.transition(DcsAdapterEvent::TelemetryReceived).unwrap();
        assert!(sm.is_healthy()); // Active
    }

    #[test]
    fn is_healthy_false_when_stale_or_error() {
        let mut machine = sm();
        machine.transition(DcsAdapterEvent::SocketBound).unwrap();
        machine
            .transition(DcsAdapterEvent::HandshakeCompleted)
            .unwrap();
        machine
            .transition(DcsAdapterEvent::TelemetryReceived)
            .unwrap();
        machine
            .transition(DcsAdapterEvent::TelemetryTimeout)
            .unwrap();
        assert!(!machine.is_healthy()); // Stale

        let mut sm2 = DcsAdapterStateMachine::new(2000, 3);
        sm2.transition(DcsAdapterEvent::ConnectionError("e".into()))
            .unwrap();
        assert!(!sm2.is_healthy()); // Error
    }

    #[test]
    fn reset_returns_to_disconnected() {
        let mut sm = sm();
        sm.transition(DcsAdapterEvent::ConnectionError("x".into()))
            .unwrap();
        sm.reset();
        assert_eq!(sm.state(), DcsAdapterState::Disconnected);
        assert_eq!(sm.error_count(), 0);
        assert_eq!(sm.consecutive_stale_count(), 0);
    }

    #[test]
    fn time_in_state_none_before_first_transition() {
        let sm = sm();
        assert!(sm.time_in_state().is_none());
    }

    #[test]
    fn time_in_state_some_after_transition() {
        let mut sm = sm();
        sm.transition(DcsAdapterEvent::SocketBound).unwrap();
        assert!(sm.time_in_state().is_some());
    }

    #[test]
    fn config_accessors() {
        let sm = DcsAdapterStateMachine::new(1500, 5);
        assert_eq!(sm.stale_threshold_ms(), 1500);
        assert_eq!(sm.max_retries(), 5);
    }

    // --- reconnection logic ---

    #[test]
    fn should_reconnect_from_disconnected() {
        let sm = sm();
        assert!(sm.should_reconnect());
    }

    #[test]
    fn should_reconnect_from_error_if_retries_left() {
        let mut sm = sm();
        sm.transition(DcsAdapterEvent::ConnectionError("e".into()))
            .unwrap();
        assert!(sm.should_reconnect());
    }

    #[test]
    fn should_not_reconnect_when_active() {
        let mut sm = sm();
        sm.transition(DcsAdapterEvent::SocketBound).unwrap();
        sm.transition(DcsAdapterEvent::HandshakeCompleted).unwrap();
        sm.transition(DcsAdapterEvent::TelemetryReceived).unwrap();
        assert!(!sm.should_reconnect());
    }

    #[test]
    fn should_not_reconnect_when_retries_exhausted() {
        let mut sm = DcsAdapterStateMachine::new(2000, 1);
        sm.transition(DcsAdapterEvent::ConnectionError("e".into()))
            .unwrap();
        assert!(!sm.should_reconnect());
    }

    #[test]
    fn reconnect_delay_increases_on_errors() {
        let mut sm = DcsAdapterStateMachine::new(2000, 10);
        let initial_delay = sm.reconnect_delay();

        sm.transition(DcsAdapterEvent::ConnectionError("e1".into()))
            .unwrap();
        assert!(sm.reconnect_delay() > initial_delay);

        let delay_after_first = sm.reconnect_delay();
        sm.transition(DcsAdapterEvent::SocketBound).unwrap();
        sm.transition(DcsAdapterEvent::HandshakeCompleted).unwrap();
        sm.transition(DcsAdapterEvent::ConnectionError("e2".into()))
            .unwrap();
        assert!(sm.reconnect_delay() > delay_after_first);
    }

    #[test]
    fn reconnect_delay_resets_on_successful_telemetry() {
        let mut sm = DcsAdapterStateMachine::new(2000, 10);
        sm.transition(DcsAdapterEvent::ConnectionError("e".into()))
            .unwrap();
        assert!(sm.reconnect_delay() > Duration::from_secs(1));

        sm.transition(DcsAdapterEvent::SocketBound).unwrap();
        sm.transition(DcsAdapterEvent::HandshakeCompleted).unwrap();
        sm.transition(DcsAdapterEvent::TelemetryReceived).unwrap();
        assert_eq!(sm.reconnect_delay(), Duration::from_secs(1));
    }

    #[test]
    fn reconnect_delay_capped_at_max() {
        let mut sm = DcsAdapterStateMachine::new(2000, 100);
        // Drive many errors to max out backoff
        for i in 0..20 {
            sm.transition(DcsAdapterEvent::ConnectionError(format!("e{i}")))
                .unwrap();
            if sm.is_recoverable() {
                sm.transition(DcsAdapterEvent::SocketBound).unwrap();
                sm.transition(DcsAdapterEvent::HandshakeCompleted).unwrap();
            }
        }
        assert!(sm.reconnect_delay() <= Duration::from_secs(30));
    }

    #[test]
    fn full_lifecycle_disconnect_reconnect() {
        let mut sm = sm();

        // Connect and go active
        sm.transition(DcsAdapterEvent::SocketBound).unwrap();
        sm.transition(DcsAdapterEvent::HandshakeCompleted).unwrap();
        sm.transition(DcsAdapterEvent::TelemetryReceived).unwrap();
        assert_eq!(sm.state(), DcsAdapterState::Active);

        // DCS disconnects
        sm.transition(DcsAdapterEvent::DcsDisconnected).unwrap();
        assert_eq!(sm.state(), DcsAdapterState::Disconnected);

        // Reconnect
        sm.transition(DcsAdapterEvent::SocketBound).unwrap();
        sm.transition(DcsAdapterEvent::HandshakeCompleted).unwrap();
        sm.transition(DcsAdapterEvent::TelemetryReceived).unwrap();
        assert_eq!(sm.state(), DcsAdapterState::Active);
    }

    #[test]
    fn stale_recovery_then_error_then_reconnect() {
        let mut sm = sm();

        // Go active
        sm.transition(DcsAdapterEvent::SocketBound).unwrap();
        sm.transition(DcsAdapterEvent::HandshakeCompleted).unwrap();
        sm.transition(DcsAdapterEvent::TelemetryReceived).unwrap();

        // Go stale
        sm.transition(DcsAdapterEvent::TelemetryTimeout).unwrap();
        assert_eq!(sm.state(), DcsAdapterState::Stale);

        // Recover
        sm.transition(DcsAdapterEvent::TelemetryReceived).unwrap();
        assert_eq!(sm.state(), DcsAdapterState::Active);

        // Error
        sm.transition(DcsAdapterEvent::ConnectionError("e".into()))
            .unwrap();
        assert_eq!(sm.state(), DcsAdapterState::Error);

        // Reconnect
        sm.transition(DcsAdapterEvent::SocketBound).unwrap();
        sm.transition(DcsAdapterEvent::HandshakeCompleted).unwrap();
        sm.transition(DcsAdapterEvent::TelemetryReceived).unwrap();
        assert_eq!(sm.state(), DcsAdapterState::Active);
    }

    #[test]
    fn display_all_states() {
        assert_eq!(DcsAdapterState::Disconnected.to_string(), "Disconnected");
        assert_eq!(DcsAdapterState::Connecting.to_string(), "Connecting");
        assert_eq!(DcsAdapterState::Listening.to_string(), "Listening");
        assert_eq!(DcsAdapterState::Connected.to_string(), "Connected");
        assert_eq!(DcsAdapterState::Active.to_string(), "Active");
        assert_eq!(DcsAdapterState::Stale.to_string(), "Stale");
        assert_eq!(DcsAdapterState::Error.to_string(), "Error");
    }

    #[test]
    fn shutdown_resets_reconnect_delay() {
        let mut sm = sm();
        sm.transition(DcsAdapterEvent::ConnectionError("e".into()))
            .unwrap();
        assert!(sm.reconnect_delay() > Duration::from_secs(1));
        sm.transition(DcsAdapterEvent::Shutdown).unwrap();
        assert_eq!(sm.reconnect_delay(), Duration::from_secs(1));
    }

    // --- Listening state transitions ---

    #[test]
    fn connecting_to_listening_on_listening_started() {
        let mut sm = sm();
        sm.transition(DcsAdapterEvent::SocketBound).unwrap();
        let next = sm.transition(DcsAdapterEvent::ListeningStarted).unwrap();
        assert_eq!(next, DcsAdapterState::Listening);
    }

    #[test]
    fn listening_to_connected_on_handshake() {
        let mut sm = sm();
        sm.transition(DcsAdapterEvent::SocketBound).unwrap();
        sm.transition(DcsAdapterEvent::ListeningStarted).unwrap();
        let next = sm.transition(DcsAdapterEvent::HandshakeCompleted).unwrap();
        assert_eq!(next, DcsAdapterState::Connected);
    }

    #[test]
    fn listening_to_active_on_telemetry_udp_shortcut() {
        let mut sm = sm();
        sm.transition(DcsAdapterEvent::SocketBound).unwrap();
        sm.transition(DcsAdapterEvent::ListeningStarted).unwrap();
        let next = sm.transition(DcsAdapterEvent::TelemetryReceived).unwrap();
        assert_eq!(next, DcsAdapterState::Active);
        assert_eq!(sm.error_count(), 0);
    }

    #[test]
    fn listening_to_disconnected_on_dcs_disconnect() {
        let mut sm = sm();
        sm.transition(DcsAdapterEvent::SocketBound).unwrap();
        sm.transition(DcsAdapterEvent::ListeningStarted).unwrap();
        let next = sm.transition(DcsAdapterEvent::DcsDisconnected).unwrap();
        assert_eq!(next, DcsAdapterState::Disconnected);
    }

    #[test]
    fn display_listening_state() {
        assert_eq!(DcsAdapterState::Listening.to_string(), "Listening");
    }

    // --- Stale exhaustion ---

    #[test]
    fn stale_to_disconnected_on_stale_exhausted() {
        let mut sm = sm();
        sm.transition(DcsAdapterEvent::SocketBound).unwrap();
        sm.transition(DcsAdapterEvent::HandshakeCompleted).unwrap();
        sm.transition(DcsAdapterEvent::TelemetryReceived).unwrap();
        sm.transition(DcsAdapterEvent::TelemetryTimeout).unwrap();
        let next = sm.transition(DcsAdapterEvent::StaleExhausted).unwrap();
        assert_eq!(next, DcsAdapterState::Disconnected);
        assert_eq!(sm.consecutive_stale_count(), 0);
    }

    #[test]
    fn is_stale_exhausted_false_initially() {
        let sm = sm();
        assert!(!sm.is_stale_exhausted());
    }

    #[test]
    fn is_stale_exhausted_after_max_stale() {
        let mut sm = DcsAdapterStateMachine::new(2000, 3).with_max_stale(2);
        sm.transition(DcsAdapterEvent::SocketBound).unwrap();
        sm.transition(DcsAdapterEvent::HandshakeCompleted).unwrap();
        sm.transition(DcsAdapterEvent::TelemetryReceived).unwrap();
        sm.transition(DcsAdapterEvent::TelemetryTimeout).unwrap();
        assert!(!sm.is_stale_exhausted());
        sm.transition(DcsAdapterEvent::TelemetryTimeout).unwrap();
        assert!(sm.is_stale_exhausted());
    }

    #[test]
    fn with_max_stale_configures_threshold() {
        let sm = DcsAdapterStateMachine::new(2000, 3).with_max_stale(5);
        assert_eq!(sm.max_stale_before_disconnect(), 5);
    }
}
