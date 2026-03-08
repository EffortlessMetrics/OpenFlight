// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! SimConnect adapter state machine with validated transitions.
//!
//! Provides a strict state machine that enforces valid adapter lifecycle
//! transitions and tracks error/retry counts for reconnection logic.
//! Modelled after the X-Plane adapter state machine (ADR-001 pattern).

use std::time::{Duration, Instant};
use thiserror::Error;

/// SimConnect adapter lifecycle states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimConnectAdapterState {
    /// Not connected to MSFS.
    Disconnected,
    /// SimConnect `Open` called, waiting for OPEN response.
    Connecting,
    /// Connected, waiting for aircraft identification data.
    Connected,
    /// Aircraft detected, data definitions registered.
    Active,
    /// No telemetry received within the stale threshold.
    Stale,
    /// Connection lost, waiting for backoff before reconnect attempt.
    Reconnecting,
    /// Unrecoverable or retry-exhausted error.
    Error,
}

/// Events that drive state transitions.
#[derive(Debug, Clone)]
pub enum SimConnectEvent {
    /// SimConnect_Open succeeded (OPEN message received).
    OpenReceived,
    /// Aircraft identification data received and processed.
    AircraftDetected,
    /// Valid telemetry data received.
    TelemetryReceived,
    /// No telemetry within the stale threshold.
    TelemetryTimeout,
    /// SimConnect connection lost (QUIT / E_FAIL / pipe broken).
    ConnectionLost(String),
    /// Begin a reconnection attempt after backoff.
    RetryConnect,
    /// Graceful shutdown requested.
    Shutdown,
}

/// Error returned when a transition is invalid.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum SimConnectTransitionError {
    #[error("invalid transition from {from:?} on event {event}")]
    InvalidTransition {
        from: SimConnectAdapterState,
        event: String,
    },
    #[error("retry limit reached ({max_retries} retries exhausted)")]
    RetriesExhausted { max_retries: u32 },
}

/// State machine that enforces valid SimConnect adapter lifecycle transitions.
pub struct SimConnectStateMachine {
    state: SimConnectAdapterState,
    last_transition: Option<Instant>,
    stale_threshold_ms: u64,
    error_count: u32,
    max_retries: u32,
}

impl SimConnectStateMachine {
    /// Create a new state machine starting in `Disconnected`.
    pub fn new(stale_threshold_ms: u64, max_retries: u32) -> Self {
        Self {
            state: SimConnectAdapterState::Disconnected,
            last_transition: None,
            stale_threshold_ms,
            error_count: 0,
            max_retries,
        }
    }

    /// Current state.
    pub fn state(&self) -> SimConnectAdapterState {
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
            SimConnectAdapterState::Connected | SimConnectAdapterState::Active
        )
    }

    /// `true` when `error_count < max_retries`.
    pub fn is_recoverable(&self) -> bool {
        self.error_count < self.max_retries
    }

    /// Reset to `Disconnected` and clear error count.
    pub fn reset(&mut self) {
        self.state = SimConnectAdapterState::Disconnected;
        self.last_transition = Some(Instant::now());
        self.error_count = 0;
    }

    /// Duration since the last state transition, or `None` if no transition yet.
    pub fn time_in_state(&self) -> Option<Duration> {
        self.last_transition.map(|t| t.elapsed())
    }

    /// Attempt a state transition driven by `event`.
    ///
    /// Returns the new state on success, or a `SimConnectTransitionError` if
    /// the transition is not allowed from the current state.
    pub fn transition(
        &mut self,
        event: SimConnectEvent,
    ) -> Result<SimConnectAdapterState, SimConnectTransitionError> {
        use SimConnectAdapterState::*;
        use SimConnectEvent::*;

        let next = match (&self.state, &event) {
            // Shutdown from any state → Disconnected
            (_, Shutdown) => {
                self.error_count = 0;
                Disconnected
            }

            // ConnectionLost from any connected state → Reconnecting (if recoverable)
            (Connected | Active | Stale, ConnectionLost(_)) => {
                self.error_count += 1;
                if self.is_recoverable() {
                    Reconnecting
                } else {
                    Error
                }
            }

            // ConnectionLost from Connecting or Reconnecting → Error
            (Connecting | Reconnecting, ConnectionLost(_)) => {
                self.error_count += 1;
                if self.is_recoverable() {
                    Reconnecting
                } else {
                    Error
                }
            }

            // ConnectionLost from any other state → Error
            (_, ConnectionLost(_)) => {
                self.error_count += 1;
                Error
            }

            // Disconnected → Connecting (SimConnect_Open called)
            (Disconnected, OpenReceived) => Connecting,

            // Connecting → Connected (OPEN message received and confirmed)
            (Connecting, OpenReceived) => {
                self.error_count = 0;
                Connected
            }

            // Connected → Active (aircraft detected + data defs set up)
            (Connected, AircraftDetected) => {
                self.error_count = 0;
                Active
            }

            // Active → Active (continuous telemetry)
            (Active, TelemetryReceived) => Active,

            // Active → Stale (telemetry timeout)
            (Active, TelemetryTimeout) => Stale,

            // Stale → Stale (repeated timeout while already stale)
            (Stale, TelemetryTimeout) => Stale,

            // Stale → Active (recovery — telemetry resumes)
            (Stale, TelemetryReceived) => {
                self.error_count = 0;
                Active
            }

            // Connected also accepts telemetry (direct data before detection)
            (Connected, TelemetryReceived) => Connected,

            // Reconnecting → Connecting (backoff elapsed, retry attempt)
            (Reconnecting, RetryConnect) => Connecting,

            // Reconnecting → Connecting via OpenReceived (immediate retry)
            (Reconnecting, OpenReceived) => Connecting,

            // Error → Connecting (retry if allowed)
            (Error, OpenReceived) => {
                if self.is_recoverable() {
                    Connecting
                } else {
                    return Err(SimConnectTransitionError::RetriesExhausted {
                        max_retries: self.max_retries,
                    });
                }
            }

            // Error → Reconnecting (schedule a retry)
            (Error, RetryConnect) => {
                if self.is_recoverable() {
                    Reconnecting
                } else {
                    return Err(SimConnectTransitionError::RetriesExhausted {
                        max_retries: self.max_retries,
                    });
                }
            }

            // Everything else is invalid
            (from, _) => {
                return Err(SimConnectTransitionError::InvalidTransition {
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

    fn sm() -> SimConnectStateMachine {
        SimConnectStateMachine::new(5000, 3)
    }

    // --- happy-path transitions ---

    #[test]
    fn disconnected_to_connecting_on_open() {
        let mut sm = sm();
        let next = sm.transition(SimConnectEvent::OpenReceived).unwrap();
        assert_eq!(next, SimConnectAdapterState::Connecting);
    }

    #[test]
    fn connecting_to_connected_on_open() {
        let mut sm = sm();
        sm.transition(SimConnectEvent::OpenReceived).unwrap();
        let next = sm.transition(SimConnectEvent::OpenReceived).unwrap();
        assert_eq!(next, SimConnectAdapterState::Connected);
    }

    #[test]
    fn connected_to_active_on_aircraft_detected() {
        let mut sm = sm();
        sm.transition(SimConnectEvent::OpenReceived).unwrap();
        sm.transition(SimConnectEvent::OpenReceived).unwrap();
        let next = sm.transition(SimConnectEvent::AircraftDetected).unwrap();
        assert_eq!(next, SimConnectAdapterState::Active);
    }

    #[test]
    fn active_stays_active_on_telemetry() {
        let mut sm = sm();
        sm.transition(SimConnectEvent::OpenReceived).unwrap();
        sm.transition(SimConnectEvent::OpenReceived).unwrap();
        sm.transition(SimConnectEvent::AircraftDetected).unwrap();
        let next = sm.transition(SimConnectEvent::TelemetryReceived).unwrap();
        assert_eq!(next, SimConnectAdapterState::Active);
    }

    #[test]
    fn active_to_stale_on_timeout() {
        let mut sm = sm();
        sm.transition(SimConnectEvent::OpenReceived).unwrap();
        sm.transition(SimConnectEvent::OpenReceived).unwrap();
        sm.transition(SimConnectEvent::AircraftDetected).unwrap();
        let next = sm.transition(SimConnectEvent::TelemetryTimeout).unwrap();
        assert_eq!(next, SimConnectAdapterState::Stale);
    }

    #[test]
    fn stale_to_active_on_telemetry_recovery() {
        let mut sm = sm();
        sm.transition(SimConnectEvent::OpenReceived).unwrap();
        sm.transition(SimConnectEvent::OpenReceived).unwrap();
        sm.transition(SimConnectEvent::AircraftDetected).unwrap();
        sm.transition(SimConnectEvent::TelemetryTimeout).unwrap();
        let next = sm.transition(SimConnectEvent::TelemetryReceived).unwrap();
        assert_eq!(next, SimConnectAdapterState::Active);
        assert_eq!(sm.error_count(), 0);
    }

    #[test]
    fn stale_stays_stale_on_repeated_timeout() {
        let mut sm = sm();
        sm.transition(SimConnectEvent::OpenReceived).unwrap();
        sm.transition(SimConnectEvent::OpenReceived).unwrap();
        sm.transition(SimConnectEvent::AircraftDetected).unwrap();
        sm.transition(SimConnectEvent::TelemetryTimeout).unwrap();
        let next = sm.transition(SimConnectEvent::TelemetryTimeout).unwrap();
        assert_eq!(next, SimConnectAdapterState::Stale);
    }

    #[test]
    fn connected_accepts_telemetry_without_change() {
        let mut sm = sm();
        sm.transition(SimConnectEvent::OpenReceived).unwrap();
        sm.transition(SimConnectEvent::OpenReceived).unwrap();
        let next = sm.transition(SimConnectEvent::TelemetryReceived).unwrap();
        assert_eq!(next, SimConnectAdapterState::Connected);
    }

    // --- error & recovery ---

    #[test]
    fn active_to_reconnecting_on_connection_lost() {
        let mut sm = sm();
        sm.transition(SimConnectEvent::OpenReceived).unwrap();
        sm.transition(SimConnectEvent::OpenReceived).unwrap();
        sm.transition(SimConnectEvent::AircraftDetected).unwrap();
        let next = sm
            .transition(SimConnectEvent::ConnectionLost("pipe broken".into()))
            .unwrap();
        assert_eq!(next, SimConnectAdapterState::Reconnecting);
        assert_eq!(sm.error_count(), 1);
    }

    #[test]
    fn reconnecting_to_connecting_on_retry() {
        let mut sm = sm();
        sm.transition(SimConnectEvent::OpenReceived).unwrap();
        sm.transition(SimConnectEvent::OpenReceived).unwrap();
        sm.transition(SimConnectEvent::AircraftDetected).unwrap();
        sm.transition(SimConnectEvent::ConnectionLost("err".into()))
            .unwrap();
        let next = sm.transition(SimConnectEvent::RetryConnect).unwrap();
        assert_eq!(next, SimConnectAdapterState::Connecting);
    }

    #[test]
    fn reconnecting_to_connecting_on_open() {
        let mut sm = sm();
        sm.transition(SimConnectEvent::OpenReceived).unwrap();
        sm.transition(SimConnectEvent::OpenReceived).unwrap();
        sm.transition(SimConnectEvent::AircraftDetected).unwrap();
        sm.transition(SimConnectEvent::ConnectionLost("err".into()))
            .unwrap();
        let next = sm.transition(SimConnectEvent::OpenReceived).unwrap();
        assert_eq!(next, SimConnectAdapterState::Connecting);
    }

    #[test]
    fn disconnected_to_error_on_connection_lost() {
        let mut sm = sm();
        let next = sm
            .transition(SimConnectEvent::ConnectionLost("err".into()))
            .unwrap();
        assert_eq!(next, SimConnectAdapterState::Error);
        assert_eq!(sm.error_count(), 1);
    }

    #[test]
    fn error_to_connecting_on_open_if_recoverable() {
        let mut sm = sm();
        // Disconnected → Error (ConnectionLost from Disconnected goes to Error)
        sm.transition(SimConnectEvent::ConnectionLost("err".into()))
            .unwrap();
        assert_eq!(sm.state(), SimConnectAdapterState::Error);
        let next = sm.transition(SimConnectEvent::OpenReceived).unwrap();
        assert_eq!(next, SimConnectAdapterState::Connecting);
    }

    #[test]
    fn error_retries_exhausted() {
        let mut sm = SimConnectStateMachine::new(5000, 1);
        // Disconnected → Error (1 error == max_retries → not recoverable)
        sm.transition(SimConnectEvent::ConnectionLost("e1".into()))
            .unwrap();
        let res = sm.transition(SimConnectEvent::OpenReceived);
        assert!(matches!(
            res,
            Err(SimConnectTransitionError::RetriesExhausted { max_retries: 1 })
        ));
    }

    #[test]
    fn multiple_errors_accumulate() {
        let mut sm = SimConnectStateMachine::new(5000, 10);
        for i in 0..5 {
            sm.transition(SimConnectEvent::ConnectionLost(format!("e{i}")))
                .unwrap();
            assert_eq!(sm.error_count(), i + 1);
            // Recover to allow next error: use RetryConnect for Reconnecting, OpenReceived for Error
            match sm.state() {
                SimConnectAdapterState::Reconnecting => {
                    sm.transition(SimConnectEvent::RetryConnect).unwrap();
                }
                SimConnectAdapterState::Error => {
                    sm.transition(SimConnectEvent::OpenReceived).unwrap();
                }
                _ => {}
            }
        }
    }

    // --- shutdown from any state ---

    #[test]
    fn shutdown_from_active() {
        let mut sm = sm();
        sm.transition(SimConnectEvent::OpenReceived).unwrap();
        sm.transition(SimConnectEvent::OpenReceived).unwrap();
        sm.transition(SimConnectEvent::AircraftDetected).unwrap();
        let next = sm.transition(SimConnectEvent::Shutdown).unwrap();
        assert_eq!(next, SimConnectAdapterState::Disconnected);
    }

    #[test]
    fn shutdown_from_error_clears_error_count() {
        let mut sm = sm();
        // Disconnected → Error
        sm.transition(SimConnectEvent::ConnectionLost("e".into()))
            .unwrap();
        assert_eq!(sm.error_count(), 1);
        sm.transition(SimConnectEvent::Shutdown).unwrap();
        assert_eq!(sm.error_count(), 0);
        assert_eq!(sm.state(), SimConnectAdapterState::Disconnected);
    }

    #[test]
    fn shutdown_from_reconnecting() {
        let mut sm = sm();
        sm.transition(SimConnectEvent::OpenReceived).unwrap();
        sm.transition(SimConnectEvent::OpenReceived).unwrap();
        sm.transition(SimConnectEvent::AircraftDetected).unwrap();
        sm.transition(SimConnectEvent::ConnectionLost("err".into()))
            .unwrap();
        assert_eq!(sm.state(), SimConnectAdapterState::Reconnecting);
        let next = sm.transition(SimConnectEvent::Shutdown).unwrap();
        assert_eq!(next, SimConnectAdapterState::Disconnected);
        assert_eq!(sm.error_count(), 0);
    }

    #[test]
    fn shutdown_from_stale() {
        let mut sm = sm();
        sm.transition(SimConnectEvent::OpenReceived).unwrap();
        sm.transition(SimConnectEvent::OpenReceived).unwrap();
        sm.transition(SimConnectEvent::AircraftDetected).unwrap();
        sm.transition(SimConnectEvent::TelemetryTimeout).unwrap();
        let next = sm.transition(SimConnectEvent::Shutdown).unwrap();
        assert_eq!(next, SimConnectAdapterState::Disconnected);
    }

    // --- invalid transitions ---

    #[test]
    fn disconnected_rejects_telemetry() {
        let mut sm = sm();
        let res = sm.transition(SimConnectEvent::TelemetryReceived);
        assert!(matches!(
            res,
            Err(SimConnectTransitionError::InvalidTransition { .. })
        ));
    }

    #[test]
    fn disconnected_rejects_aircraft_detected() {
        let mut sm = sm();
        let res = sm.transition(SimConnectEvent::AircraftDetected);
        assert!(matches!(
            res,
            Err(SimConnectTransitionError::InvalidTransition { .. })
        ));
    }

    #[test]
    fn connecting_rejects_timeout() {
        let mut sm = sm();
        sm.transition(SimConnectEvent::OpenReceived).unwrap();
        let res = sm.transition(SimConnectEvent::TelemetryTimeout);
        assert!(matches!(
            res,
            Err(SimConnectTransitionError::InvalidTransition { .. })
        ));
    }

    #[test]
    fn connecting_rejects_aircraft_detected() {
        let mut sm = sm();
        sm.transition(SimConnectEvent::OpenReceived).unwrap();
        let res = sm.transition(SimConnectEvent::AircraftDetected);
        assert!(matches!(
            res,
            Err(SimConnectTransitionError::InvalidTransition { .. })
        ));
    }

    // --- helper methods ---

    #[test]
    fn is_healthy_true_when_connected_or_active() {
        let mut sm = sm();
        assert!(!sm.is_healthy());
        sm.transition(SimConnectEvent::OpenReceived).unwrap();
        assert!(!sm.is_healthy()); // Connecting
        sm.transition(SimConnectEvent::OpenReceived).unwrap();
        assert!(sm.is_healthy()); // Connected
        sm.transition(SimConnectEvent::AircraftDetected).unwrap();
        assert!(sm.is_healthy()); // Active
    }

    #[test]
    fn is_healthy_false_when_stale() {
        let mut sm = sm();
        sm.transition(SimConnectEvent::OpenReceived).unwrap();
        sm.transition(SimConnectEvent::OpenReceived).unwrap();
        sm.transition(SimConnectEvent::AircraftDetected).unwrap();
        sm.transition(SimConnectEvent::TelemetryTimeout).unwrap();
        assert!(!sm.is_healthy());
    }

    #[test]
    fn reset_returns_to_disconnected() {
        let mut sm = sm();
        // Disconnected → Error
        sm.transition(SimConnectEvent::ConnectionLost("x".into()))
            .unwrap();
        sm.reset();
        assert_eq!(sm.state(), SimConnectAdapterState::Disconnected);
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
        sm.transition(SimConnectEvent::OpenReceived).unwrap();
        assert!(sm.time_in_state().is_some());
    }

    #[test]
    fn config_accessors() {
        let sm = SimConnectStateMachine::new(3000, 5);
        assert_eq!(sm.stale_threshold_ms(), 3000);
        assert_eq!(sm.max_retries(), 5);
    }

    #[test]
    fn full_lifecycle_happy_path() {
        let mut sm = sm();
        assert_eq!(sm.state(), SimConnectAdapterState::Disconnected);

        sm.transition(SimConnectEvent::OpenReceived).unwrap();
        assert_eq!(sm.state(), SimConnectAdapterState::Connecting);

        sm.transition(SimConnectEvent::OpenReceived).unwrap();
        assert_eq!(sm.state(), SimConnectAdapterState::Connected);

        sm.transition(SimConnectEvent::AircraftDetected).unwrap();
        assert_eq!(sm.state(), SimConnectAdapterState::Active);

        // Steady-state telemetry
        for _ in 0..10 {
            sm.transition(SimConnectEvent::TelemetryReceived).unwrap();
            assert_eq!(sm.state(), SimConnectAdapterState::Active);
        }

        // Stale and recover
        sm.transition(SimConnectEvent::TelemetryTimeout).unwrap();
        assert_eq!(sm.state(), SimConnectAdapterState::Stale);

        sm.transition(SimConnectEvent::TelemetryReceived).unwrap();
        assert_eq!(sm.state(), SimConnectAdapterState::Active);

        // Clean shutdown
        sm.transition(SimConnectEvent::Shutdown).unwrap();
        assert_eq!(sm.state(), SimConnectAdapterState::Disconnected);
    }

    #[test]
    fn error_recovery_lifecycle() {
        let mut sm = sm();

        // Connect
        sm.transition(SimConnectEvent::OpenReceived).unwrap();
        sm.transition(SimConnectEvent::OpenReceived).unwrap();
        sm.transition(SimConnectEvent::AircraftDetected).unwrap();

        // Connection lost → Reconnecting (recoverable)
        sm.transition(SimConnectEvent::ConnectionLost("pipe broken".into()))
            .unwrap();
        assert_eq!(sm.state(), SimConnectAdapterState::Reconnecting);
        assert_eq!(sm.error_count(), 1);

        // Retry → Connecting
        sm.transition(SimConnectEvent::RetryConnect).unwrap();
        assert_eq!(sm.state(), SimConnectAdapterState::Connecting);

        // Reconnect successfully
        sm.transition(SimConnectEvent::OpenReceived).unwrap();
        assert_eq!(sm.state(), SimConnectAdapterState::Connected);
        assert_eq!(sm.error_count(), 0); // Reset on successful connect
    }
}
