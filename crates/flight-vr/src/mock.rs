// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

use crate::adapter::{VrBackend, VrError};
use crate::pose::{HeadPose, TrackingQuality, VrSnapshot};

/// A scripted mock backend useful for unit and integration tests.
///
/// `new_connected` walks through a fixed sequence of snapshots (wrapping
/// back to the start when the sequence is exhausted).  `new_disconnected`
/// always returns [`VrError::NotConnected`].
pub struct MockVrBackend {
    connected: bool,
    sequence: Vec<VrSnapshot>,
    index: usize,
    name: String,
}

impl MockVrBackend {
    /// Create a connected backend that cycles through `sequence`.
    pub fn new_connected(sequence: Vec<VrSnapshot>) -> Self {
        Self {
            connected: true,
            sequence,
            index: 0,
            name: "MockVrBackend".to_owned(),
        }
    }

    /// Create a backend that reports as disconnected on every call.
    pub fn new_disconnected() -> Self {
        Self {
            connected: false,
            sequence: Vec::new(),
            index: 0,
            name: "MockVrBackend(disconnected)".to_owned(),
        }
    }
}

impl VrBackend for MockVrBackend {
    fn poll(&mut self) -> Result<VrSnapshot, VrError> {
        if !self.connected {
            return Err(VrError::NotConnected);
        }
        if self.sequence.is_empty() {
            return Err(VrError::PollFailed("empty sequence".to_owned()));
        }
        let snapshot = self.sequence[self.index % self.sequence.len()].clone();
        self.index += 1;
        Ok(snapshot)
    }

    fn is_connected(&self) -> bool {
        self.connected
    }

    fn backend_name(&self) -> &str {
        &self.name
    }
}

/// Convenience constructor for a single-item snapshot used in tests.
pub fn make_snapshot(yaw: f32, quality: TrackingQuality, is_worn: bool) -> VrSnapshot {
    VrSnapshot {
        pose: HeadPose {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            yaw,
            pitch: 0.0,
            roll: 0.0,
        },
        quality,
        is_worn,
    }
}
