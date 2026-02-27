// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

use crate::pose::VrSnapshot;

/// Errors produced by the VR adapter.
#[derive(Debug, thiserror::Error)]
pub enum VrError {
    /// The backend reports it is not connected to a VR runtime.
    #[error("Backend not connected")]
    NotConnected,
    /// A poll attempt failed for the given reason.
    #[error("Poll failed: {0}")]
    PollFailed(String),
    /// The pose data returned by the backend is invalid.
    #[error("Invalid pose data")]
    InvalidPose,
}

/// Interface that concrete VR backends must implement.
pub trait VrBackend: Send + Sync {
    /// Poll for the latest head pose snapshot.
    fn poll(&mut self) -> Result<VrSnapshot, VrError>;
    /// Return `true` when the backend has an active connection to a VR runtime.
    fn is_connected(&self) -> bool;
    /// Human-readable name of the backend (e.g. `"OpenVR"`, `"OpenXR"`).
    fn backend_name(&self) -> &str;
}

/// Stateful adapter that wraps a [`VrBackend`] and caches the last snapshot.
pub struct VrAdapter<B: VrBackend> {
    backend: B,
    last_snapshot: Option<VrSnapshot>,
    error_count: u32,
}

impl<B: VrBackend> VrAdapter<B> {
    /// Create a new adapter wrapping `backend`.
    pub fn new(backend: B) -> Self {
        tracing::info!(backend = backend.backend_name(), "VR adapter created");
        Self {
            backend,
            last_snapshot: None,
            error_count: 0,
        }
    }

    /// Poll the backend, cache the result, and return a reference to it.
    ///
    /// # Errors
    ///
    /// - [`VrError::NotConnected`] when the backend has no active runtime.
    /// - Any error propagated from [`VrBackend::poll`].
    pub fn update(&mut self) -> Result<&VrSnapshot, VrError> {
        if !self.backend.is_connected() {
            self.error_count += 1;
            return Err(VrError::NotConnected);
        }
        match self.backend.poll() {
            Ok(snapshot) => {
                self.error_count = 0;
                self.last_snapshot = Some(snapshot);
                Ok(self.last_snapshot.as_ref().unwrap())
            }
            Err(e) => {
                self.error_count += 1;
                tracing::warn!(
                    error = %e,
                    error_count = self.error_count,
                    "VR poll failed"
                );
                Err(e)
            }
        }
    }

    /// Return the most recently cached snapshot, if any.
    pub fn last_snapshot(&self) -> Option<&VrSnapshot> {
        self.last_snapshot.as_ref()
    }

    /// Return `true` when the backend is connected *and* at least one snapshot
    /// has been successfully cached.
    pub fn is_active(&self) -> bool {
        self.backend.is_connected() && self.last_snapshot.is_some()
    }
}
