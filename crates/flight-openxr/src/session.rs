// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

use crate::pose::HeadPose;
use thiserror::Error;

// ── Error ─────────────────────────────────────────────────────────────────────

/// Errors produced by the OpenXR adapter.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum OpenXrError {
    /// The OpenXR runtime or loader is not available.
    #[error("Runtime not available: {0}")]
    RuntimeNotAvailable(String),

    /// The session was unexpectedly lost by the runtime.
    #[error("Session lost")]
    SessionLost,

    /// A pose could not be obtained this tick.
    #[error("Pose unavailable")]
    PoseUnavailable,
}

// ── Session state ─────────────────────────────────────────────────────────────

/// Lifecycle state of the OpenXR session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// No session has been created yet.
    Uninitialized,
    /// Session creation is in progress.
    Initializing,
    /// Session created and ready to poll.
    Ready,
    /// Session is actively polling pose data.
    Running,
    /// Graceful shutdown in progress.
    Stopping,
    /// An unrecoverable error was encountered.
    Error,
}

// ── Runtime trait ─────────────────────────────────────────────────────────────

/// Abstraction over an OpenXR runtime.
///
/// Implement this trait to plug in either a real OpenXR session or a
/// [`MockRuntime`] for offline testing.
pub trait OpenXrRuntime: Send + Sync {
    /// Initialise the runtime and prepare for pose polling.
    fn initialize(&mut self) -> Result<(), OpenXrError>;

    /// Poll the current HMD pose.
    fn poll_pose(&mut self) -> Result<HeadPose, OpenXrError>;

    /// Shut the runtime down cleanly.
    fn shutdown(&mut self);
}

// ── Mock runtime ──────────────────────────────────────────────────────────────

/// Deterministic mock OpenXR runtime for use in tests.
///
/// Cycles through a pre-loaded list of [`HeadPose`] values, wrapping at the
/// end.  Initialisation always succeeds.
pub struct MockRuntime {
    /// The poses to cycle through.
    pub poses: Vec<HeadPose>,
    /// Current position in the pose list.
    pub index: usize,
    /// Set to `true` after [`initialize`](OpenXrRuntime::initialize) is called.
    pub initialized: bool,
    /// If `Some`, `poll_pose` returns this error once and then clears it.
    pub next_error: Option<OpenXrError>,
}

impl MockRuntime {
    /// Create a new mock with the given pose sequence.
    pub fn new(poses: Vec<HeadPose>) -> Self {
        Self {
            poses,
            index: 0,
            initialized: false,
            next_error: None,
        }
    }
}

impl OpenXrRuntime for MockRuntime {
    fn initialize(&mut self) -> Result<(), OpenXrError> {
        self.initialized = true;
        Ok(())
    }

    fn poll_pose(&mut self) -> Result<HeadPose, OpenXrError> {
        if let Some(err) = self.next_error.take() {
            return Err(err);
        }
        if self.poses.is_empty() {
            return Err(OpenXrError::PoseUnavailable);
        }
        let pose = self.poses[self.index % self.poses.len()];
        self.index += 1;
        Ok(pose)
    }

    fn shutdown(&mut self) {
        self.initialized = false;
    }
}
