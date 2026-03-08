// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

/// 6DOF head pose from VR headset.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeadPose {
    /// Position in meters from tracking origin — left/right.
    pub x: f32,
    /// Position in meters from tracking origin — up/down.
    pub y: f32,
    /// Position in meters from tracking origin — forward/back.
    pub z: f32,
    /// Rotation in degrees — horizontal look.
    pub yaw: f32,
    /// Rotation in degrees — vertical look.
    pub pitch: f32,
    /// Rotation in degrees — tilt.
    pub roll: f32,
}

impl HeadPose {
    /// Return a pose with all fields set to zero.
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

    /// Normalize rotation angles to `(-180.0, 180.0]`.
    ///
    /// Positional fields (`x`, `y`, `z`) are left unchanged.
    pub fn normalize(&self) -> Self {
        Self {
            x: self.x,
            y: self.y,
            z: self.z,
            yaw: wrap_angle(self.yaw),
            pitch: wrap_angle(self.pitch),
            roll: wrap_angle(self.roll),
        }
    }
}

/// Wrap an angle in degrees to `(-180.0, 180.0]`.
fn wrap_angle(deg: f32) -> f32 {
    let mut d = deg % 360.0;
    if d > 180.0 {
        d -= 360.0;
    } else if d <= -180.0 {
        d += 360.0;
    }
    d
}

/// Head tracking quality indicator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackingQuality {
    /// Full 6DOF tracking available.
    Good,
    /// Partial tracking (e.g., controllers visible but not head).
    Degraded,
    /// No tracking signal.
    Lost,
}

/// A snapshot of VR state at a single point in time.
#[derive(Debug, Clone)]
pub struct VrSnapshot {
    /// 6DOF head pose.
    pub pose: HeadPose,
    /// Tracking confidence level.
    pub quality: TrackingQuality,
    /// Proximity sensor — `true` when the headset is being worn.
    pub is_worn: bool,
}
