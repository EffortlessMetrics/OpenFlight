// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Session lifecycle state machine.
//!
//! [`SessionLifecycle`] tracks the current phase of a session through a strict
//! set of valid transitions:
//!
//! ```text
//! Initializing → Ready → Active ⇄ Paused → ShuttingDown → Terminated
//! ```
//!
//! Each transition records the reason and the time spent in the previous state.

use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

/// The possible states of a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SessionState {
    Initializing,
    Ready,
    Active,
    Paused,
    ShuttingDown,
    Terminated,
}

impl std::fmt::Display for SessionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Initializing => write!(f, "Initializing"),
            Self::Ready => write!(f, "Ready"),
            Self::Active => write!(f, "Active"),
            Self::Paused => write!(f, "Paused"),
            Self::ShuttingDown => write!(f, "ShuttingDown"),
            Self::Terminated => write!(f, "Terminated"),
        }
    }
}

/// Reason for a state transition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransitionReason {
    /// Explicit user action (e.g. pause/resume).
    User,
    /// Automatic transition (e.g. initialisation complete).
    Auto,
    /// An error forced the transition.
    Error(String),
    /// The simulator disconnected.
    SimDisconnect,
}

impl std::fmt::Display for TransitionReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::User => write!(f, "user"),
            Self::Auto => write!(f, "auto"),
            Self::Error(msg) => write!(f, "error: {msg}"),
            Self::SimDisconnect => write!(f, "sim-disconnect"),
        }
    }
}

/// A recorded state transition.
#[derive(Debug, Clone)]
pub struct TransitionRecord {
    pub from: SessionState,
    pub to: SessionState,
    pub reason: TransitionReason,
    /// Time spent in the `from` state before the transition.
    pub duration_in_state: Duration,
}

/// Errors returned when an invalid transition is attempted.
#[derive(Debug, thiserror::Error)]
#[error("invalid transition from {from} to {to}")]
pub struct TransitionError {
    pub from: SessionState,
    pub to: SessionState,
}

/// Session lifecycle state machine.
///
/// Tracks the current state, enforces valid transitions, and records how long
/// the session has spent in each state.
pub struct SessionLifecycle {
    state: SessionState,
    state_entered: Instant,
    history: Vec<TransitionRecord>,
    cumulative: std::collections::HashMap<SessionState, Duration>,
}

impl SessionLifecycle {
    /// Create a new lifecycle starting in [`SessionState::Initializing`].
    pub fn new() -> Self {
        let mut cumulative = std::collections::HashMap::new();
        cumulative.insert(SessionState::Initializing, Duration::ZERO);
        cumulative.insert(SessionState::Ready, Duration::ZERO);
        cumulative.insert(SessionState::Active, Duration::ZERO);
        cumulative.insert(SessionState::Paused, Duration::ZERO);
        cumulative.insert(SessionState::ShuttingDown, Duration::ZERO);
        cumulative.insert(SessionState::Terminated, Duration::ZERO);

        Self {
            state: SessionState::Initializing,
            state_entered: Instant::now(),
            history: Vec::new(),
            cumulative,
        }
    }

    /// The current state.
    pub fn state(&self) -> SessionState {
        self.state
    }

    /// Attempt to transition to `target` for the given `reason`.
    ///
    /// Returns the [`TransitionRecord`] on success.
    pub fn transition(
        &mut self,
        target: SessionState,
        reason: TransitionReason,
    ) -> std::result::Result<TransitionRecord, TransitionError> {
        if !Self::is_valid_transition(self.state, target) {
            return Err(TransitionError {
                from: self.state,
                to: target,
            });
        }

        let now = Instant::now();
        let duration_in_state = now.duration_since(self.state_entered);

        // Accumulate time in the previous state.
        *self.cumulative.entry(self.state).or_default() += duration_in_state;

        let record = TransitionRecord {
            from: self.state,
            to: target,
            reason,
            duration_in_state,
        };

        self.state = target;
        self.state_entered = now;
        self.history.push(record.clone());

        Ok(record)
    }

    /// Total cumulative time spent in `state`.
    pub fn time_in_state(&self, state: SessionState) -> Duration {
        let mut total = self.cumulative.get(&state).copied().unwrap_or_default();
        // If we are currently in that state, add the live elapsed time.
        if self.state == state {
            total += self.state_entered.elapsed();
        }
        total
    }

    /// Duration elapsed since the current state was entered.
    pub fn current_state_elapsed(&self) -> Duration {
        self.state_entered.elapsed()
    }

    /// Full transition history.
    pub fn history(&self) -> &[TransitionRecord] {
        &self.history
    }

    /// Whether transitioning from `from` to `to` is allowed.
    pub fn is_valid_transition(from: SessionState, to: SessionState) -> bool {
        matches!(
            (from, to),
            (SessionState::Initializing, SessionState::Ready)
                | (SessionState::Ready, SessionState::Active)
                | (SessionState::Active, SessionState::Paused)
                | (SessionState::Active, SessionState::ShuttingDown)
                | (SessionState::Paused, SessionState::Active)
                | (SessionState::Paused, SessionState::ShuttingDown)
                | (SessionState::Ready, SessionState::ShuttingDown)
                | (SessionState::ShuttingDown, SessionState::Terminated)
                // Error/disconnect can jump to ShuttingDown from Initializing
                | (SessionState::Initializing, SessionState::ShuttingDown)
        )
    }
}

impl Default for SessionLifecycle {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_in_initializing() {
        let lc = SessionLifecycle::new();
        assert_eq!(lc.state(), SessionState::Initializing);
    }

    #[test]
    fn valid_transition_initializing_to_ready() {
        let mut lc = SessionLifecycle::new();
        let rec = lc
            .transition(SessionState::Ready, TransitionReason::Auto)
            .unwrap();
        assert_eq!(rec.from, SessionState::Initializing);
        assert_eq!(rec.to, SessionState::Ready);
        assert_eq!(lc.state(), SessionState::Ready);
    }

    #[test]
    fn valid_full_lifecycle() {
        let mut lc = SessionLifecycle::new();
        lc.transition(SessionState::Ready, TransitionReason::Auto)
            .unwrap();
        lc.transition(SessionState::Active, TransitionReason::User)
            .unwrap();
        lc.transition(SessionState::Paused, TransitionReason::User)
            .unwrap();
        lc.transition(SessionState::Active, TransitionReason::User)
            .unwrap();
        lc.transition(SessionState::ShuttingDown, TransitionReason::User)
            .unwrap();
        lc.transition(SessionState::Terminated, TransitionReason::Auto)
            .unwrap();
        assert_eq!(lc.state(), SessionState::Terminated);
        assert_eq!(lc.history().len(), 6);
    }

    #[test]
    fn invalid_transition_rejected() {
        let mut lc = SessionLifecycle::new();
        let err = lc
            .transition(SessionState::Active, TransitionReason::Auto)
            .unwrap_err();
        assert_eq!(err.from, SessionState::Initializing);
        assert_eq!(err.to, SessionState::Active);
        // State unchanged
        assert_eq!(lc.state(), SessionState::Initializing);
    }

    #[test]
    fn cannot_transition_from_terminated() {
        let mut lc = SessionLifecycle::new();
        lc.transition(SessionState::Ready, TransitionReason::Auto)
            .unwrap();
        lc.transition(SessionState::ShuttingDown, TransitionReason::User)
            .unwrap();
        lc.transition(SessionState::Terminated, TransitionReason::Auto)
            .unwrap();
        let err = lc
            .transition(SessionState::Initializing, TransitionReason::Auto)
            .unwrap_err();
        assert_eq!(err.from, SessionState::Terminated);
    }

    #[test]
    fn cannot_skip_ready_to_paused() {
        let mut lc = SessionLifecycle::new();
        lc.transition(SessionState::Ready, TransitionReason::Auto)
            .unwrap();
        let err = lc
            .transition(SessionState::Paused, TransitionReason::User)
            .unwrap_err();
        assert_eq!(err.from, SessionState::Ready);
        assert_eq!(err.to, SessionState::Paused);
    }

    #[test]
    fn duration_tracking_accumulates() {
        let mut lc = SessionLifecycle::new();
        let t0 = lc.time_in_state(SessionState::Initializing);
        lc.transition(SessionState::Ready, TransitionReason::Auto)
            .unwrap();
        let t1 = lc.time_in_state(SessionState::Initializing);
        // After leaving Initializing, accumulated time is frozen and >= t0.
        assert!(t1 >= t0);
        assert!(t1 > Duration::ZERO);
        // Past-state time is stable (no longer ticking).
        let t2 = lc.time_in_state(SessionState::Initializing);
        assert_eq!(t1, t2);
    }

    #[test]
    fn current_state_elapsed_grows() {
        let lc = SessionLifecycle::new();
        let e1 = lc.current_state_elapsed();
        let e2 = lc.current_state_elapsed();
        // Monotonically non-decreasing.
        assert!(e2 >= e1);
    }

    #[test]
    fn time_in_current_state_includes_live_elapsed() {
        let lc = SessionLifecycle::new();
        let t1 = lc.time_in_state(SessionState::Initializing);
        let t2 = lc.time_in_state(SessionState::Initializing);
        // Live elapsed grows monotonically.
        assert!(t2 >= t1);
    }

    #[test]
    fn transition_reason_display() {
        assert_eq!(TransitionReason::User.to_string(), "user");
        assert_eq!(TransitionReason::Auto.to_string(), "auto");
        assert_eq!(
            TransitionReason::SimDisconnect.to_string(),
            "sim-disconnect"
        );
        assert_eq!(
            TransitionReason::Error("oops".into()).to_string(),
            "error: oops"
        );
    }

    #[test]
    fn reason_recorded_in_history() {
        let mut lc = SessionLifecycle::new();
        lc.transition(SessionState::Ready, TransitionReason::Auto)
            .unwrap();
        lc.transition(
            SessionState::ShuttingDown,
            TransitionReason::Error("fatal".into()),
        )
        .unwrap();
        assert_eq!(lc.history()[0].reason, TransitionReason::Auto);
        assert_eq!(
            lc.history()[1].reason,
            TransitionReason::Error("fatal".into())
        );
    }

    #[test]
    fn error_can_jump_initializing_to_shutting_down() {
        let mut lc = SessionLifecycle::new();
        lc.transition(
            SessionState::ShuttingDown,
            TransitionReason::Error("startup failure".into()),
        )
        .unwrap();
        assert_eq!(lc.state(), SessionState::ShuttingDown);
    }

    #[test]
    fn sim_disconnect_reason_paused_to_shutdown() {
        let mut lc = SessionLifecycle::new();
        lc.transition(SessionState::Ready, TransitionReason::Auto)
            .unwrap();
        lc.transition(SessionState::Active, TransitionReason::User)
            .unwrap();
        lc.transition(SessionState::Paused, TransitionReason::SimDisconnect)
            .unwrap();
        lc.transition(SessionState::ShuttingDown, TransitionReason::SimDisconnect)
            .unwrap();
        assert_eq!(lc.state(), SessionState::ShuttingDown);
    }

    #[test]
    fn default_creates_initializing() {
        let lc = SessionLifecycle::default();
        assert_eq!(lc.state(), SessionState::Initializing);
    }

    #[test]
    fn state_display() {
        assert_eq!(SessionState::Initializing.to_string(), "Initializing");
        assert_eq!(SessionState::Ready.to_string(), "Ready");
        assert_eq!(SessionState::Active.to_string(), "Active");
        assert_eq!(SessionState::Paused.to_string(), "Paused");
        assert_eq!(SessionState::ShuttingDown.to_string(), "ShuttingDown");
        assert_eq!(SessionState::Terminated.to_string(), "Terminated");
    }
}
