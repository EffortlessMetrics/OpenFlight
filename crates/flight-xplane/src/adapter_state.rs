// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! X-Plane adapter state machine with validated transitions.
//!
//! Provides a strict state machine that enforces valid adapter lifecycle
//! transitions and tracks error/retry counts for reconnection logic.

use std::time::{Duration, Instant};
use thiserror::Error;

/// X-Plane adapter lifecycle states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XPlaneAdapterState {
    /// Not connected to X-Plane.
    Disconnected,
    /// Socket bound, waiting for first telemetry.
    Connecting,
    /// Connected but no telemetry data yet.
    Connected,
    /// Receiving valid telemetry.
    Active,
    /// Telemetry timeout — no packets within threshold.
    Stale,
    /// Unrecoverable or retry-exhausted error.
    Error,
}

/// Events that drive state transitions.
#[derive(Debug, Clone)]
pub enum AdapterEvent {
    /// UDP socket successfully bound.
    SocketBound,
    /// Valid telemetry packet received.
    TelemetryReceived,
    /// No telemetry within the stale threshold.
    TelemetryTimeout,
    /// Socket-level error.
    SocketError(String),
    /// Graceful shutdown requested.
    Shutdown,
}

/// Error returned when a transition is invalid.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum TransitionError {
    #[error("invalid transition from {from:?} on event {event}")]
    InvalidTransition {
        from: XPlaneAdapterState,
        event: String,
    },
    #[error("retry limit reached ({max_retries} retries exhausted)")]
    RetriesExhausted { max_retries: u32 },
}

/// State machine that enforces valid adapter lifecycle transitions.
pub struct AdapterStateMachine {
    state: XPlaneAdapterState,
    last_transition: Option<Instant>,
    stale_threshold_ms: u64,
    error_count: u32,
    max_retries: u32,
}

impl AdapterStateMachine {
    /// Create a new state machine starting in `Disconnected`.
    pub fn new(stale_threshold_ms: u64, max_retries: u32) -> Self {
        Self {
            state: XPlaneAdapterState::Disconnected,
            last_transition: None,
            stale_threshold_ms,
            error_count: 0,
            max_retries,
        }
    }

    /// Current state.
    pub fn state(&self) -> XPlaneAdapterState {
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

    /// `true` when state is `Connected` or `Active`.
    pub fn is_healthy(&self) -> bool {
        matches!(
            self.state,
            XPlaneAdapterState::Connected | XPlaneAdapterState::Active
        )
    }

    /// `true` when `error_count < max_retries`.
    pub fn is_recoverable(&self) -> bool {
        self.error_count < self.max_retries
    }

    /// Reset to `Disconnected` and clear error count.
    pub fn reset(&mut self) {
        self.state = XPlaneAdapterState::Disconnected;
        self.last_transition = Some(Instant::now());
        self.error_count = 0;
    }

    /// Duration since the last state transition, or `None` if no transition yet.
    pub fn time_in_state(&self) -> Option<Duration> {
        self.last_transition.map(|t| t.elapsed())
    }

    /// Attempt a state transition driven by `event`.
    ///
    /// Returns the new state on success, or a `TransitionError` if the
    /// transition is not allowed from the current state.
    pub fn transition(
        &mut self,
        event: AdapterEvent,
    ) -> Result<XPlaneAdapterState, TransitionError> {
        use AdapterEvent::*;
        use XPlaneAdapterState::*;

        let next = match (&self.state, &event) {
            // Shutdown from any state → Disconnected
            (_, Shutdown) => {
                self.error_count = 0;
                Disconnected
            }

            // SocketError from any state → Error
            (_, SocketError(_)) => {
                self.error_count += 1;
                Error
            }

            // Disconnected → Connecting
            (Disconnected, SocketBound) => Connecting,

            // Connecting → Connected
            (Connecting, SocketBound) => Connected,

            // Connected → Active
            (Connected, TelemetryReceived) => {
                self.error_count = 0;
                Active
            }

            // Active → Active (continuous telemetry)
            (Active, TelemetryReceived) => Active,

            // Active → Stale
            (Active, TelemetryTimeout) => Stale,

            // Stale → Stale (repeated timeout while already stale)
            (Stale, TelemetryTimeout) => Stale,

            // Stale → Active (recovery)
            (Stale, TelemetryReceived) => {
                self.error_count = 0;
                Active
            }

            // Error → Connecting (retry if allowed)
            (Error, SocketBound) => {
                if self.is_recoverable() {
                    Connecting
                } else {
                    return Err(TransitionError::RetriesExhausted {
                        max_retries: self.max_retries,
                    });
                }
            }

            // Everything else is invalid
            (from, _) => {
                return Err(TransitionError::InvalidTransition {
                    from: *from,
                    event: format!("{event:?}"),
                });
            }
        };

        self.state = next;
        self.last_transition = Some(Instant::now());
        Ok(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sm() -> AdapterStateMachine {
        AdapterStateMachine::new(2000, 3)
    }

    // --- happy-path transitions ---

    #[test]
    fn disconnected_to_connecting_on_socket_bound() {
        let mut sm = sm();
        let next = sm.transition(AdapterEvent::SocketBound).unwrap();
        assert_eq!(next, XPlaneAdapterState::Connecting);
    }

    #[test]
    fn connecting_to_connected_on_socket_bound() {
        let mut sm = sm();
        sm.transition(AdapterEvent::SocketBound).unwrap();
        let next = sm.transition(AdapterEvent::SocketBound).unwrap();
        assert_eq!(next, XPlaneAdapterState::Connected);
    }

    #[test]
    fn connected_to_active_on_telemetry() {
        let mut sm = sm();
        sm.transition(AdapterEvent::SocketBound).unwrap();
        sm.transition(AdapterEvent::SocketBound).unwrap();
        let next = sm.transition(AdapterEvent::TelemetryReceived).unwrap();
        assert_eq!(next, XPlaneAdapterState::Active);
    }

    #[test]
    fn active_stays_active_on_telemetry() {
        let mut sm = sm();
        sm.transition(AdapterEvent::SocketBound).unwrap();
        sm.transition(AdapterEvent::SocketBound).unwrap();
        sm.transition(AdapterEvent::TelemetryReceived).unwrap();
        let next = sm.transition(AdapterEvent::TelemetryReceived).unwrap();
        assert_eq!(next, XPlaneAdapterState::Active);
    }

    #[test]
    fn active_to_stale_on_timeout() {
        let mut sm = sm();
        sm.transition(AdapterEvent::SocketBound).unwrap();
        sm.transition(AdapterEvent::SocketBound).unwrap();
        sm.transition(AdapterEvent::TelemetryReceived).unwrap();
        let next = sm.transition(AdapterEvent::TelemetryTimeout).unwrap();
        assert_eq!(next, XPlaneAdapterState::Stale);
    }

    #[test]
    fn stale_to_active_on_telemetry() {
        let mut sm = sm();
        sm.transition(AdapterEvent::SocketBound).unwrap();
        sm.transition(AdapterEvent::SocketBound).unwrap();
        sm.transition(AdapterEvent::TelemetryReceived).unwrap();
        sm.transition(AdapterEvent::TelemetryTimeout).unwrap();
        let next = sm.transition(AdapterEvent::TelemetryReceived).unwrap();
        assert_eq!(next, XPlaneAdapterState::Active);
        assert_eq!(sm.error_count(), 0);
    }

    #[test]
    fn stale_stays_stale_on_repeated_timeout() {
        let mut sm = sm();
        sm.transition(AdapterEvent::SocketBound).unwrap();
        sm.transition(AdapterEvent::SocketBound).unwrap();
        sm.transition(AdapterEvent::TelemetryReceived).unwrap();
        sm.transition(AdapterEvent::TelemetryTimeout).unwrap();
        let next = sm.transition(AdapterEvent::TelemetryTimeout).unwrap();
        assert_eq!(next, XPlaneAdapterState::Stale);
    }

    // --- error & recovery ---

    #[test]
    fn any_state_to_error_on_socket_error() {
        let mut sm = sm();
        sm.transition(AdapterEvent::SocketBound).unwrap();
        sm.transition(AdapterEvent::SocketBound).unwrap();
        sm.transition(AdapterEvent::TelemetryReceived).unwrap();
        let next = sm
            .transition(AdapterEvent::SocketError("test".into()))
            .unwrap();
        assert_eq!(next, XPlaneAdapterState::Error);
        assert_eq!(sm.error_count(), 1);
    }

    #[test]
    fn error_to_connecting_on_socket_bound_if_recoverable() {
        let mut sm = sm();
        sm.transition(AdapterEvent::SocketError("err".into()))
            .unwrap();
        assert_eq!(sm.state(), XPlaneAdapterState::Error);
        let next = sm.transition(AdapterEvent::SocketBound).unwrap();
        assert_eq!(next, XPlaneAdapterState::Connecting);
    }

    #[test]
    fn error_retries_exhausted() {
        let mut sm = AdapterStateMachine::new(2000, 1);
        sm.transition(AdapterEvent::SocketError("e1".into()))
            .unwrap();
        let res = sm.transition(AdapterEvent::SocketBound);
        assert!(matches!(
            res,
            Err(TransitionError::RetriesExhausted { max_retries: 1 })
        ));
    }

    // --- shutdown from any ---

    #[test]
    fn shutdown_from_active() {
        let mut sm = sm();
        sm.transition(AdapterEvent::SocketBound).unwrap();
        sm.transition(AdapterEvent::SocketBound).unwrap();
        sm.transition(AdapterEvent::TelemetryReceived).unwrap();
        let next = sm.transition(AdapterEvent::Shutdown).unwrap();
        assert_eq!(next, XPlaneAdapterState::Disconnected);
    }

    #[test]
    fn shutdown_from_error_clears_error_count() {
        let mut sm = sm();
        sm.transition(AdapterEvent::SocketError("e".into()))
            .unwrap();
        assert_eq!(sm.error_count(), 1);
        sm.transition(AdapterEvent::Shutdown).unwrap();
        assert_eq!(sm.error_count(), 0);
        assert_eq!(sm.state(), XPlaneAdapterState::Disconnected);
    }

    // --- invalid transitions ---

    #[test]
    fn disconnected_rejects_telemetry() {
        let mut sm = sm();
        let res = sm.transition(AdapterEvent::TelemetryReceived);
        assert!(matches!(
            res,
            Err(TransitionError::InvalidTransition { .. })
        ));
    }

    #[test]
    fn connecting_rejects_timeout() {
        let mut sm = sm();
        sm.transition(AdapterEvent::SocketBound).unwrap();
        let res = sm.transition(AdapterEvent::TelemetryTimeout);
        assert!(matches!(
            res,
            Err(TransitionError::InvalidTransition { .. })
        ));
    }

    // --- helper methods ---

    #[test]
    fn is_healthy_true_when_connected_or_active() {
        let mut sm = sm();
        assert!(!sm.is_healthy());
        sm.transition(AdapterEvent::SocketBound).unwrap();
        assert!(!sm.is_healthy());
        sm.transition(AdapterEvent::SocketBound).unwrap();
        assert!(sm.is_healthy()); // Connected
        sm.transition(AdapterEvent::TelemetryReceived).unwrap();
        assert!(sm.is_healthy()); // Active
    }

    #[test]
    fn reset_returns_to_disconnected() {
        let mut sm = sm();
        sm.transition(AdapterEvent::SocketError("x".into()))
            .unwrap();
        sm.reset();
        assert_eq!(sm.state(), XPlaneAdapterState::Disconnected);
        assert_eq!(sm.error_count(), 0);
    }

    #[test]
    fn time_in_state_none_before_first_transition() {
        let sm = sm();
        assert!(sm.time_in_state().is_none());
    }

    #[test]
    fn time_in_state_some_after_transition() {
        let mut sm = sm();
        sm.transition(AdapterEvent::SocketBound).unwrap();
        assert!(sm.time_in_state().is_some());
    }

    #[test]
    fn config_accessors() {
        let sm = AdapterStateMachine::new(1500, 5);
        assert_eq!(sm.stale_threshold_ms(), 1500);
        assert_eq!(sm.max_retries(), 5);
    }
}
