// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! 6DOF motion frame — the output unit of the motion platform pipeline.

use serde::{Deserialize, Serialize};

/// A single 6-degree-of-freedom motion frame.
///
/// Each channel is normalized to the range **-1.0 to +1.0**, where:
/// - `+1.0` = maximum positive platform excursion
/// - `-1.0` = maximum negative platform excursion
/// - `0.0`  = platform at neutral/center position
///
/// Convention (matches SimTools and most motion platforms):
/// - **Surge**: forward (+) / backward (-)
/// - **Sway**: right (+) / left (-)
/// - **Heave**: up (+) / down (-)
/// - **Roll**: right-wing-down (+) / left-wing-down (-)
/// - **Pitch**: nose-up (+) / nose-down (-)
/// - **Yaw**: nose-right (+) / nose-left (-)
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MotionFrame {
    /// Longitudinal surge: forward/backward translation.
    pub surge: f32,
    /// Lateral sway: left/right translation.
    pub sway: f32,
    /// Vertical heave: up/down translation.
    pub heave: f32,
    /// Roll tilt.
    pub roll: f32,
    /// Pitch tilt.
    pub pitch: f32,
    /// Yaw rotation.
    pub yaw: f32,
}

impl MotionFrame {
    /// A neutral frame with all channels at zero.
    pub const NEUTRAL: Self = Self {
        surge: 0.0,
        sway: 0.0,
        heave: 0.0,
        roll: 0.0,
        pitch: 0.0,
        yaw: 0.0,
    };

    /// Returns `true` if all channels are zero (neutral).
    pub fn is_neutral(&self) -> bool {
        self.surge == 0.0
            && self.sway == 0.0
            && self.heave == 0.0
            && self.roll == 0.0
            && self.pitch == 0.0
            && self.yaw == 0.0
    }

    /// Clamp all channels to the -1.0..=1.0 range.
    pub fn clamped(self) -> Self {
        Self {
            surge: self.surge.clamp(-1.0, 1.0),
            sway: self.sway.clamp(-1.0, 1.0),
            heave: self.heave.clamp(-1.0, 1.0),
            roll: self.roll.clamp(-1.0, 1.0),
            pitch: self.pitch.clamp(-1.0, 1.0),
            yaw: self.yaw.clamp(-1.0, 1.0),
        }
    }

    /// Scale all channels by a uniform factor.
    pub fn scaled(self, factor: f32) -> Self {
        Self {
            surge: self.surge * factor,
            sway: self.sway * factor,
            heave: self.heave * factor,
            roll: self.roll * factor,
            pitch: self.pitch * factor,
            yaw: self.yaw * factor,
        }
    }

    /// Convert to SimTools format: values in **-100..100** integer range.
    ///
    /// SimTools expects: `"A{surge}B{sway}C{heave}D{roll}E{pitch}F{yaw}\n"`
    pub fn to_simtools_string(&self) -> String {
        let f = self.clamped();
        let to_i = |v: f32| (v * 100.0).round() as i32;
        format!(
            "A{}B{}C{}D{}E{}F{}\n",
            to_i(f.surge),
            to_i(f.sway),
            to_i(f.heave),
            to_i(f.roll),
            to_i(f.pitch),
            to_i(f.yaw),
        )
    }

    /// Returns an array of channel values in [surge, sway, heave, roll, pitch, yaw] order.
    pub fn to_array(&self) -> [f32; 6] {
        [self.surge, self.sway, self.heave, self.roll, self.pitch, self.yaw]
    }
}

impl Default for MotionFrame {
    fn default() -> Self {
        Self::NEUTRAL
    }
}

impl std::fmt::Display for MotionFrame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "surge={:.3} sway={:.3} heave={:.3} roll={:.3} pitch={:.3} yaw={:.3}",
            self.surge, self.sway, self.heave, self.roll, self.pitch, self.yaw
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_neutral_frame() {
        let f = MotionFrame::NEUTRAL;
        assert!(f.is_neutral());
    }

    #[test]
    fn test_clamp() {
        let f = MotionFrame {
            surge: 2.0,
            sway: -3.0,
            heave: 0.5,
            roll: -0.5,
            pitch: 1.0,
            yaw: -1.0,
        };
        let c = f.clamped();
        assert_eq!(c.surge, 1.0);
        assert_eq!(c.sway, -1.0);
        assert_eq!(c.heave, 0.5);
    }

    #[test]
    fn test_simtools_string() {
        let f = MotionFrame {
            surge: 0.5,
            sway: -0.5,
            heave: 1.0,
            roll: 0.0,
            pitch: 0.0,
            yaw: 0.0,
        };
        assert_eq!(f.to_simtools_string(), "A50B-50C100D0E0F0\n");
    }

    #[test]
    fn test_scaled() {
        let f = MotionFrame {
            surge: 1.0,
            sway: 0.5,
            heave: -0.5,
            roll: 0.0,
            pitch: 0.0,
            yaw: 0.0,
        };
        let s = f.scaled(0.5);
        assert_eq!(s.surge, 0.5);
        assert_eq!(s.sway, 0.25);
    }
}
