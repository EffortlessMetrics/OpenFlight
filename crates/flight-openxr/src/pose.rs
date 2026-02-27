// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

/// Head pose from an OpenXR session (6 DOF).
///
/// Positions are in **metres**; angles are in **radians**.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeadPose {
    /// Left/right translation in metres (positive = right).
    pub x: f32,
    /// Up/down translation in metres (positive = up).
    pub y: f32,
    /// Forward/backward translation in metres (positive = forward).
    pub z: f32,
    /// Yaw angle in radians.
    pub yaw: f32,
    /// Pitch angle in radians.
    pub pitch: f32,
    /// Roll angle in radians.
    pub roll: f32,
}

impl HeadPose {
    /// A zeroed pose — origin, no rotation.
    pub fn zero() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            yaw: 0.0,
            pitch: 0.0,
            roll: 0.0,
        }
    }

    /// Returns `true` if every field is finite (not NaN or ±∞).
    pub fn is_finite(&self) -> bool {
        self.x.is_finite()
            && self.y.is_finite()
            && self.z.is_finite()
            && self.yaw.is_finite()
            && self.pitch.is_finite()
            && self.roll.is_finite()
    }
}
