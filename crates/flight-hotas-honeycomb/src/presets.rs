// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Default axis configuration presets for Honeycomb devices.

/// Default deadzone for the Alpha Yoke primary axes.
pub const ALPHA_AXIS_DEADZONE: f32 = 0.02;

/// Default expo (response curve) for the Alpha Yoke.
pub const ALPHA_AXIS_EXPO: f32 = 0.15;

/// Default deadzone for the Bravo throttle axes.
/// Throttles typically need a small deadzone at the idle end.
pub const BRAVO_THROTTLE_DEADZONE_IDLE: f32 = 0.01;

/// Default deadzone at the full-throttle end of the Bravo throttle axes.
pub const BRAVO_THROTTLE_DEADZONE_FULL: f32 = 0.01;

/// Axis name constants for the Alpha Yoke.
pub mod alpha_axes {
    pub const ROLL: &str = "roll";
    pub const PITCH: &str = "pitch";
}

/// Axis name constants for the Bravo Throttle Quadrant.
pub mod bravo_axes {
    pub const THROTTLE1: &str = "throttle1";
    pub const THROTTLE2: &str = "throttle2";
    pub const THROTTLE3: &str = "throttle3";
    pub const THROTTLE4: &str = "throttle4";
    pub const THROTTLE5: &str = "throttle5";
    pub const FLAP_LEVER: &str = "flap_lever";
    pub const SPOILER: &str = "spoiler";
}

/// All axis names for the Bravo, in report order.
pub const BRAVO_AXIS_NAMES: &[&str] = &[
    bravo_axes::THROTTLE1,
    bravo_axes::THROTTLE2,
    bravo_axes::THROTTLE3,
    bravo_axes::THROTTLE4,
    bravo_axes::THROTTLE5,
    bravo_axes::FLAP_LEVER,
    bravo_axes::SPOILER,
];

/// All axis names for the Alpha Yoke, in report order.
pub const ALPHA_AXIS_NAMES: &[&str] = &[alpha_axes::ROLL, alpha_axes::PITCH];

/// Default deadzone for the Charlie rudder axis.
pub const CHARLIE_RUDDER_DEADZONE: f32 = 0.03;

/// Default expo (response curve) for the Charlie rudder axis.
pub const CHARLIE_RUDDER_EXPO: f32 = 0.10;

/// Default deadzone for the Charlie toe brake axes.
pub const CHARLIE_BRAKE_DEADZONE: f32 = 0.02;

/// Axis name constants for the Charlie Rudder Pedals.
pub mod charlie_axes {
    pub const RUDDER: &str = "rudder";
    pub const LEFT_BRAKE: &str = "left_brake";
    pub const RIGHT_BRAKE: &str = "right_brake";
}

/// All axis names for the Charlie Rudder Pedals, in report order.
pub const CHARLIE_AXIS_NAMES: &[&str] = &[
    charlie_axes::RUDDER,
    charlie_axes::LEFT_BRAKE,
    charlie_axes::RIGHT_BRAKE,
];
