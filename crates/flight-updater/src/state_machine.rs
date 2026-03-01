// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Update lifecycle state machine.
//!
//! Models the full update flow:
//!
//! ```text
//! Idle → Checking → Downloading → Verifying → Applying → Complete
//!   ↑                                  ↓          ↓          │
//!   └──────────── RolledBack ←─────────┘──────────┘          │
//!   └────────────────────────────────────────────────────────-┘
//! ```
//!
//! The state machine enforces valid transitions and prevents updates
//! while a flight simulation is active (mid-flight protection).

use crate::channels::Channel;
use crate::manifest::SemVer;
use serde::{Deserialize, Serialize};
use std::fmt;

// ---------------------------------------------------------------------------
// UpdateState
// ---------------------------------------------------------------------------

/// States in the update lifecycle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UpdateState {
    /// No update in progress; ready to check.
    Idle,
    /// Querying the update server for a new version.
    Checking,
    /// Downloading the update package.
    Downloading {
        version: String,
        channel: Channel,
        /// Bytes downloaded so far.
        bytes_downloaded: u64,
        /// Total bytes expected.
        bytes_total: u64,
    },
    /// Verifying checksums and signatures of the downloaded package.
    Verifying { version: String, channel: Channel },
    /// Applying the update (file replacement / delta patch).
    Applying { version: String, channel: Channel },
    /// Update completed successfully.
    Complete {
        version: String,
        channel: Channel,
        previous_version: String,
    },
    /// Update failed and the previous version was restored.
    RolledBack {
        failed_version: String,
        restored_version: String,
        reason: String,
    },
    /// Update was blocked because a sim is currently running.
    BlockedMidFlight,
    /// A recoverable error occurred; the machine can return to Idle.
    Failed { reason: String },
}

impl fmt::Display for UpdateState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Checking => write!(f, "Checking"),
            Self::Downloading { version, .. } => write!(f, "Downloading {version}"),
            Self::Verifying { version, .. } => write!(f, "Verifying {version}"),
            Self::Applying { version, .. } => write!(f, "Applying {version}"),
            Self::Complete { version, .. } => write!(f, "Complete ({version})"),
            Self::RolledBack {
                restored_version, ..
            } => write!(f, "RolledBack to {restored_version}"),
            Self::BlockedMidFlight => write!(f, "Blocked (mid-flight)"),
            Self::Failed { reason } => write!(f, "Failed: {reason}"),
        }
    }
}

// ---------------------------------------------------------------------------
// UpdateEvent
// ---------------------------------------------------------------------------

/// Events that drive state transitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateEvent {
    /// User or scheduler initiates an update check.
    CheckForUpdate,
    /// Server reports a new version available.
    UpdateAvailable {
        version: String,
        channel: Channel,
        size: u64,
    },
    /// Server reports no update available.
    NoUpdateAvailable,
    /// The check itself failed (network error, etc.).
    CheckFailed(String),
    /// Download progress update.
    DownloadProgress { bytes_downloaded: u64 },
    /// Download completed successfully.
    DownloadComplete,
    /// Download failed.
    DownloadFailed(String),
    /// Verification passed.
    VerificationPassed,
    /// Verification failed (bad checksum / signature).
    VerificationFailed(String),
    /// Application of the update succeeded.
    ApplySuccess { previous_version: String },
    /// Application of the update failed.
    ApplyFailed(String),
    /// Rollback completed after a failure.
    RollbackComplete {
        restored_version: String,
        reason: String,
    },
    /// A flight simulation started — block further updates.
    SimStarted,
    /// The flight simulation ended — allow updates again.
    SimStopped,
    /// Acknowledge a terminal state and return to Idle.
    Reset,
}

// ---------------------------------------------------------------------------
// Transition errors
// ---------------------------------------------------------------------------

/// Error returned when a transition is invalid for the current state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidTransition {
    pub from: String,
    pub event: String,
}

impl fmt::Display for InvalidTransition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid transition: event '{}' in state '{}'",
            self.event, self.from
        )
    }
}

impl std::error::Error for InvalidTransition {}

// ---------------------------------------------------------------------------
// UpdateStateMachine
// ---------------------------------------------------------------------------

/// Drives the update lifecycle, enforcing valid transitions and mid-flight
/// protection.
#[derive(Debug)]
pub struct UpdateStateMachine {
    state: UpdateState,
    sim_running: bool,
    current_version: SemVer,
    transition_history: Vec<(UpdateState, UpdateState)>,
}

impl UpdateStateMachine {
    /// Create a new state machine starting in [`UpdateState::Idle`].
    pub fn new(current_version: SemVer) -> Self {
        Self {
            state: UpdateState::Idle,
            sim_running: false,
            current_version,
            transition_history: Vec::new(),
        }
    }

    /// The current state.
    pub fn state(&self) -> &UpdateState {
        &self.state
    }

    /// Whether a simulator is currently running.
    pub fn is_sim_running(&self) -> bool {
        self.sim_running
    }

    /// The installed version.
    pub fn current_version(&self) -> &SemVer {
        &self.current_version
    }

    /// Full transition history (from, to) pairs.
    pub fn transition_history(&self) -> &[(UpdateState, UpdateState)] {
        &self.transition_history
    }

    /// Apply an event, returning the new state or an error.
    pub fn handle_event(&mut self, event: UpdateEvent) -> Result<&UpdateState, InvalidTransition> {
        let new_state = self.next_state(&event)?;
        let old_state = std::mem::replace(&mut self.state, new_state);
        self.transition_history
            .push((old_state, self.state.clone()));

        // Side-effects for sim events
        match &event {
            UpdateEvent::SimStarted => self.sim_running = true,
            UpdateEvent::SimStopped => self.sim_running = false,
            _ => {}
        }

        Ok(&self.state)
    }

    /// Compute the next state without mutating.
    fn next_state(&self, event: &UpdateEvent) -> Result<UpdateState, InvalidTransition> {
        let err = |ev: &str| InvalidTransition {
            from: self.state.to_string(),
            event: ev.to_string(),
        };

        match (&self.state, event) {
            // ── Idle ────────────────────────────────────────────────
            (UpdateState::Idle, UpdateEvent::CheckForUpdate) => {
                if self.sim_running {
                    Ok(UpdateState::BlockedMidFlight)
                } else {
                    Ok(UpdateState::Checking)
                }
            }
            (UpdateState::Idle, UpdateEvent::SimStarted) => Ok(UpdateState::Idle),
            (UpdateState::Idle, UpdateEvent::SimStopped) => Ok(UpdateState::Idle),

            // ── Checking ────────────────────────────────────────────
            (
                UpdateState::Checking,
                UpdateEvent::UpdateAvailable {
                    version,
                    channel,
                    size,
                },
            ) => Ok(UpdateState::Downloading {
                version: version.clone(),
                channel: *channel,
                bytes_downloaded: 0,
                bytes_total: *size,
            }),
            (UpdateState::Checking, UpdateEvent::NoUpdateAvailable) => Ok(UpdateState::Idle),
            (UpdateState::Checking, UpdateEvent::CheckFailed(reason)) => Ok(UpdateState::Failed {
                reason: reason.clone(),
            }),

            // ── Downloading ─────────────────────────────────────────
            (
                UpdateState::Downloading {
                    version,
                    channel,
                    bytes_total,
                    ..
                },
                UpdateEvent::DownloadProgress { bytes_downloaded },
            ) => Ok(UpdateState::Downloading {
                version: version.clone(),
                channel: *channel,
                bytes_downloaded: *bytes_downloaded,
                bytes_total: *bytes_total,
            }),
            (
                UpdateState::Downloading {
                    version, channel, ..
                },
                UpdateEvent::DownloadComplete,
            ) => Ok(UpdateState::Verifying {
                version: version.clone(),
                channel: *channel,
            }),
            (UpdateState::Downloading { .. }, UpdateEvent::DownloadFailed(reason)) => {
                Ok(UpdateState::Failed {
                    reason: reason.clone(),
                })
            }
            // Mid-flight guard during download
            (UpdateState::Downloading { .. }, UpdateEvent::SimStarted) => {
                Ok(UpdateState::BlockedMidFlight)
            }

            // ── Verifying ───────────────────────────────────────────
            (UpdateState::Verifying { version, channel }, UpdateEvent::VerificationPassed) => {
                Ok(UpdateState::Applying {
                    version: version.clone(),
                    channel: *channel,
                })
            }
            (UpdateState::Verifying { .. }, UpdateEvent::VerificationFailed(reason)) => {
                Ok(UpdateState::Failed {
                    reason: reason.clone(),
                })
            }

            // ── Applying ────────────────────────────────────────────
            (
                UpdateState::Applying { version, channel },
                UpdateEvent::ApplySuccess { previous_version },
            ) => Ok(UpdateState::Complete {
                version: version.clone(),
                channel: *channel,
                previous_version: previous_version.clone(),
            }),
            (UpdateState::Applying { .. }, UpdateEvent::ApplyFailed(reason)) => {
                // ApplyFailed triggers automatic rollback path —
                // caller should send RollbackComplete next.
                Ok(UpdateState::Failed {
                    reason: reason.clone(),
                })
            }

            // ── Terminal states → Reset ─────────────────────────────
            (UpdateState::Complete { .. }, UpdateEvent::Reset) => Ok(UpdateState::Idle),
            (UpdateState::RolledBack { .. }, UpdateEvent::Reset) => Ok(UpdateState::Idle),
            (UpdateState::Failed { .. }, UpdateEvent::Reset) => Ok(UpdateState::Idle),
            (UpdateState::BlockedMidFlight, UpdateEvent::Reset) => Ok(UpdateState::Idle),
            (UpdateState::BlockedMidFlight, UpdateEvent::SimStopped) => Ok(UpdateState::Idle),

            // ── Failed → RollbackComplete ───────────────────────────
            (
                UpdateState::Failed { .. },
                UpdateEvent::RollbackComplete {
                    restored_version,
                    reason,
                },
            ) => Ok(UpdateState::RolledBack {
                failed_version: "unknown".to_string(),
                restored_version: restored_version.clone(),
                reason: reason.clone(),
            }),

            // ── Everything else is invalid ──────────────────────────
            (_, event) => Err(err(&format!("{event:?}"))),
        }
    }

    /// Convenience: returns `true` when the machine is in a terminal state
    /// that can be reset.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.state,
            UpdateState::Complete { .. }
                | UpdateState::RolledBack { .. }
                | UpdateState::Failed { .. }
                | UpdateState::BlockedMidFlight
        )
    }

    /// Convenience: returns `true` when an update is actively in progress
    /// (downloading, verifying, or applying).
    pub fn is_in_progress(&self) -> bool {
        matches!(
            self.state,
            UpdateState::Checking
                | UpdateState::Downloading { .. }
                | UpdateState::Verifying { .. }
                | UpdateState::Applying { .. }
        )
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn v(major: u32, minor: u32, patch: u32) -> SemVer {
        SemVer::new(major, minor, patch)
    }

    // ── Happy-path: full lifecycle ──────────────────────────────────────

    #[test]
    fn full_lifecycle_idle_to_complete() {
        let mut sm = UpdateStateMachine::new(v(1, 0, 0));
        assert_eq!(*sm.state(), UpdateState::Idle);

        // Check
        sm.handle_event(UpdateEvent::CheckForUpdate).unwrap();
        assert_eq!(*sm.state(), UpdateState::Checking);

        // Update available
        sm.handle_event(UpdateEvent::UpdateAvailable {
            version: "2.0.0".into(),
            channel: Channel::Stable,
            size: 4096,
        })
        .unwrap();
        assert!(matches!(sm.state(), UpdateState::Downloading { .. }));

        // Progress
        sm.handle_event(UpdateEvent::DownloadProgress {
            bytes_downloaded: 2048,
        })
        .unwrap();
        if let UpdateState::Downloading {
            bytes_downloaded, ..
        } = sm.state()
        {
            assert_eq!(*bytes_downloaded, 2048);
        } else {
            panic!("expected Downloading state");
        }

        // Download complete
        sm.handle_event(UpdateEvent::DownloadComplete).unwrap();
        assert!(matches!(sm.state(), UpdateState::Verifying { .. }));

        // Verification passed
        sm.handle_event(UpdateEvent::VerificationPassed).unwrap();
        assert!(matches!(sm.state(), UpdateState::Applying { .. }));

        // Apply success
        sm.handle_event(UpdateEvent::ApplySuccess {
            previous_version: "1.0.0".into(),
        })
        .unwrap();
        assert!(matches!(sm.state(), UpdateState::Complete { .. }));
        assert!(sm.is_terminal());

        // Reset back to idle
        sm.handle_event(UpdateEvent::Reset).unwrap();
        assert_eq!(*sm.state(), UpdateState::Idle);
    }

    // ── No update available ─────────────────────────────────────────────

    #[test]
    fn checking_no_update_returns_to_idle() {
        let mut sm = UpdateStateMachine::new(v(1, 0, 0));
        sm.handle_event(UpdateEvent::CheckForUpdate).unwrap();
        sm.handle_event(UpdateEvent::NoUpdateAvailable).unwrap();
        assert_eq!(*sm.state(), UpdateState::Idle);
    }

    // ── Check failure ───────────────────────────────────────────────────

    #[test]
    fn check_failure_goes_to_failed() {
        let mut sm = UpdateStateMachine::new(v(1, 0, 0));
        sm.handle_event(UpdateEvent::CheckForUpdate).unwrap();
        sm.handle_event(UpdateEvent::CheckFailed("timeout".into()))
            .unwrap();
        assert!(matches!(sm.state(), UpdateState::Failed { .. }));
    }

    // ── Download failure ────────────────────────────────────────────────

    #[test]
    fn download_failure_goes_to_failed() {
        let mut sm = UpdateStateMachine::new(v(1, 0, 0));
        sm.handle_event(UpdateEvent::CheckForUpdate).unwrap();
        sm.handle_event(UpdateEvent::UpdateAvailable {
            version: "2.0.0".into(),
            channel: Channel::Stable,
            size: 1024,
        })
        .unwrap();
        sm.handle_event(UpdateEvent::DownloadFailed("connection reset".into()))
            .unwrap();
        assert!(matches!(sm.state(), UpdateState::Failed { .. }));
    }

    // ── Verification failure ────────────────────────────────────────────

    #[test]
    fn verification_failure_goes_to_failed() {
        let mut sm = UpdateStateMachine::new(v(1, 0, 0));
        sm.handle_event(UpdateEvent::CheckForUpdate).unwrap();
        sm.handle_event(UpdateEvent::UpdateAvailable {
            version: "2.0.0".into(),
            channel: Channel::Stable,
            size: 1024,
        })
        .unwrap();
        sm.handle_event(UpdateEvent::DownloadComplete).unwrap();
        sm.handle_event(UpdateEvent::VerificationFailed("bad checksum".into()))
            .unwrap();
        assert!(matches!(sm.state(), UpdateState::Failed { .. }));
    }

    // ── Apply failure → rollback ────────────────────────────────────────

    #[test]
    fn apply_failure_then_rollback() {
        let mut sm = UpdateStateMachine::new(v(1, 0, 0));
        sm.handle_event(UpdateEvent::CheckForUpdate).unwrap();
        sm.handle_event(UpdateEvent::UpdateAvailable {
            version: "2.0.0".into(),
            channel: Channel::Stable,
            size: 1024,
        })
        .unwrap();
        sm.handle_event(UpdateEvent::DownloadComplete).unwrap();
        sm.handle_event(UpdateEvent::VerificationPassed).unwrap();
        sm.handle_event(UpdateEvent::ApplyFailed("permission denied".into()))
            .unwrap();
        assert!(matches!(sm.state(), UpdateState::Failed { .. }));

        // Rollback completes
        sm.handle_event(UpdateEvent::RollbackComplete {
            restored_version: "1.0.0".into(),
            reason: "permission denied".into(),
        })
        .unwrap();
        assert!(matches!(sm.state(), UpdateState::RolledBack { .. }));
        if let UpdateState::RolledBack {
            restored_version,
            reason,
            ..
        } = sm.state()
        {
            assert_eq!(restored_version, "1.0.0");
            assert_eq!(reason, "permission denied");
        }

        // Reset
        sm.handle_event(UpdateEvent::Reset).unwrap();
        assert_eq!(*sm.state(), UpdateState::Idle);
    }

    // ── Mid-flight protection: check blocked ────────────────────────────

    #[test]
    fn mid_flight_blocks_check() {
        let mut sm = UpdateStateMachine::new(v(1, 0, 0));
        sm.handle_event(UpdateEvent::SimStarted).unwrap();
        assert!(sm.is_sim_running());

        sm.handle_event(UpdateEvent::CheckForUpdate).unwrap();
        assert_eq!(*sm.state(), UpdateState::BlockedMidFlight);
    }

    #[test]
    fn mid_flight_blocked_clears_on_sim_stop() {
        let mut sm = UpdateStateMachine::new(v(1, 0, 0));
        sm.handle_event(UpdateEvent::SimStarted).unwrap();
        sm.handle_event(UpdateEvent::CheckForUpdate).unwrap();
        assert_eq!(*sm.state(), UpdateState::BlockedMidFlight);

        sm.handle_event(UpdateEvent::SimStopped).unwrap();
        assert_eq!(*sm.state(), UpdateState::Idle);
        assert!(!sm.is_sim_running());
    }

    // ── Mid-flight protection: download interrupted ─────────────────────

    #[test]
    fn sim_start_during_download_blocks() {
        let mut sm = UpdateStateMachine::new(v(1, 0, 0));
        sm.handle_event(UpdateEvent::CheckForUpdate).unwrap();
        sm.handle_event(UpdateEvent::UpdateAvailable {
            version: "2.0.0".into(),
            channel: Channel::Stable,
            size: 4096,
        })
        .unwrap();

        sm.handle_event(UpdateEvent::SimStarted).unwrap();
        assert_eq!(*sm.state(), UpdateState::BlockedMidFlight);
        assert!(sm.is_sim_running());
    }

    // ── Invalid transitions ─────────────────────────────────────────────

    #[test]
    fn invalid_event_in_idle() {
        let mut sm = UpdateStateMachine::new(v(1, 0, 0));
        let result = sm.handle_event(UpdateEvent::DownloadComplete);
        assert!(result.is_err());
    }

    #[test]
    fn invalid_event_in_checking() {
        let mut sm = UpdateStateMachine::new(v(1, 0, 0));
        sm.handle_event(UpdateEvent::CheckForUpdate).unwrap();
        let result = sm.handle_event(UpdateEvent::VerificationPassed);
        assert!(result.is_err());
    }

    #[test]
    fn invalid_event_in_complete() {
        let mut sm = UpdateStateMachine::new(v(1, 0, 0));
        sm.handle_event(UpdateEvent::CheckForUpdate).unwrap();
        sm.handle_event(UpdateEvent::UpdateAvailable {
            version: "2.0.0".into(),
            channel: Channel::Stable,
            size: 1024,
        })
        .unwrap();
        sm.handle_event(UpdateEvent::DownloadComplete).unwrap();
        sm.handle_event(UpdateEvent::VerificationPassed).unwrap();
        sm.handle_event(UpdateEvent::ApplySuccess {
            previous_version: "1.0.0".into(),
        })
        .unwrap();

        // Can't check for update while in Complete state
        let result = sm.handle_event(UpdateEvent::CheckForUpdate);
        assert!(result.is_err());
    }

    // ── Transition history ──────────────────────────────────────────────

    #[test]
    fn transition_history_is_recorded() {
        let mut sm = UpdateStateMachine::new(v(1, 0, 0));
        sm.handle_event(UpdateEvent::CheckForUpdate).unwrap();
        sm.handle_event(UpdateEvent::NoUpdateAvailable).unwrap();

        let history = sm.transition_history();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].0, UpdateState::Idle);
        assert_eq!(history[0].1, UpdateState::Checking);
        assert_eq!(history[1].0, UpdateState::Checking);
        assert_eq!(history[1].1, UpdateState::Idle);
    }

    // ── is_in_progress / is_terminal ────────────────────────────────────

    #[test]
    fn is_in_progress_states() {
        let mut sm = UpdateStateMachine::new(v(1, 0, 0));
        assert!(!sm.is_in_progress());

        sm.handle_event(UpdateEvent::CheckForUpdate).unwrap();
        assert!(sm.is_in_progress());
    }

    #[test]
    fn is_terminal_states() {
        let mut sm = UpdateStateMachine::new(v(1, 0, 0));
        assert!(!sm.is_terminal());

        sm.handle_event(UpdateEvent::CheckForUpdate).unwrap();
        sm.handle_event(UpdateEvent::CheckFailed("err".into()))
            .unwrap();
        assert!(sm.is_terminal());
    }

    // ── Display ─────────────────────────────────────────────────────────

    #[test]
    fn state_display_formatting() {
        assert_eq!(UpdateState::Idle.to_string(), "Idle");
        assert_eq!(UpdateState::Checking.to_string(), "Checking");
        assert_eq!(
            UpdateState::BlockedMidFlight.to_string(),
            "Blocked (mid-flight)"
        );
        assert!(
            UpdateState::Downloading {
                version: "2.0.0".into(),
                channel: Channel::Stable,
                bytes_downloaded: 0,
                bytes_total: 1024,
            }
            .to_string()
            .contains("2.0.0")
        );
    }

    // ── Failed reset ────────────────────────────────────────────────────

    #[test]
    fn failed_state_resets_to_idle() {
        let mut sm = UpdateStateMachine::new(v(1, 0, 0));
        sm.handle_event(UpdateEvent::CheckForUpdate).unwrap();
        sm.handle_event(UpdateEvent::CheckFailed("network".into()))
            .unwrap();
        sm.handle_event(UpdateEvent::Reset).unwrap();
        assert_eq!(*sm.state(), UpdateState::Idle);
    }

    // ── Sim events while idle are no-ops ────────────────────────────────

    #[test]
    fn sim_started_while_idle_stays_idle() {
        let mut sm = UpdateStateMachine::new(v(1, 0, 0));
        sm.handle_event(UpdateEvent::SimStarted).unwrap();
        assert_eq!(*sm.state(), UpdateState::Idle);
        assert!(sm.is_sim_running());
    }

    #[test]
    fn sim_stopped_while_idle_stays_idle() {
        let mut sm = UpdateStateMachine::new(v(1, 0, 0));
        sm.handle_event(UpdateEvent::SimStopped).unwrap();
        assert_eq!(*sm.state(), UpdateState::Idle);
        assert!(!sm.is_sim_running());
    }

    // ── Channel preserved through transitions ───────────────────────────

    #[test]
    fn channel_preserved_through_lifecycle() {
        let mut sm = UpdateStateMachine::new(v(1, 0, 0));
        sm.handle_event(UpdateEvent::CheckForUpdate).unwrap();
        sm.handle_event(UpdateEvent::UpdateAvailable {
            version: "2.0.0".into(),
            channel: Channel::Beta,
            size: 1024,
        })
        .unwrap();

        if let UpdateState::Downloading { channel, .. } = sm.state() {
            assert_eq!(*channel, Channel::Beta);
        } else {
            panic!("expected Downloading");
        }

        sm.handle_event(UpdateEvent::DownloadComplete).unwrap();
        if let UpdateState::Verifying { channel, .. } = sm.state() {
            assert_eq!(*channel, Channel::Beta);
        } else {
            panic!("expected Verifying");
        }

        sm.handle_event(UpdateEvent::VerificationPassed).unwrap();
        if let UpdateState::Applying { channel, .. } = sm.state() {
            assert_eq!(*channel, Channel::Beta);
        } else {
            panic!("expected Applying");
        }

        sm.handle_event(UpdateEvent::ApplySuccess {
            previous_version: "1.0.0".into(),
        })
        .unwrap();
        if let UpdateState::Complete { channel, .. } = sm.state() {
            assert_eq!(*channel, Channel::Beta);
        } else {
            panic!("expected Complete");
        }
    }

    // ── InvalidTransition Display ───────────────────────────────────────

    #[test]
    fn invalid_transition_display() {
        let err = InvalidTransition {
            from: "Idle".into(),
            event: "DownloadComplete".into(),
        };
        assert!(err.to_string().contains("Idle"));
        assert!(err.to_string().contains("DownloadComplete"));
    }
}
