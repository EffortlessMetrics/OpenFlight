// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Default device configuration profiles for Turtle Beach VelocityOne devices.
//!
//! Each profile describes the axis mapping, button assignments, and
//! device-specific features. These are used by the profile pipeline to generate
//! baseline configurations that users can customise.

use crate::devices::{self, VelocityOneDevice};

/// Axis mapping entry in a device profile.
#[derive(Debug, Clone, PartialEq)]
pub struct AxisMapping {
    /// Human-readable axis name (e.g., "roll", "throttle_left").
    pub name: &'static str,
    /// Axis index in the HID report (0-based).
    pub index: u8,
    /// Whether the axis is bipolar (centred) or unipolar (zero-to-max).
    pub bipolar: bool,
    /// Default deadzone as a fraction \[0.0, 1.0\].
    pub deadzone: f32,
    /// Default expo (response curve) as a fraction \[0.0, 1.0\].
    pub expo: f32,
    /// Sim variable binding hint (e.g., "ELEVATOR", "THROTTLE 1").
    pub sim_var_hint: &'static str,
}

/// Button assignment entry in a device profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ButtonMapping {
    /// Human-readable button name.
    pub name: &'static str,
    /// Button number (1-indexed, matching HID report).
    pub button_num: u8,
    /// Sim event binding hint (e.g., "AP_MASTER", "GEAR_UP").
    pub sim_event_hint: &'static str,
}

/// A complete device configuration profile.
#[derive(Debug, Clone)]
pub struct DeviceProfile {
    /// Device model name.
    pub name: &'static str,
    /// USB Vendor ID.
    pub vendor_id: u16,
    /// USB Product ID.
    pub product_id: u16,
    /// Axis mappings.
    pub axes: &'static [AxisMapping],
    /// Button mappings (notable buttons only).
    pub buttons: &'static [ButtonMapping],
    /// Whether the device has LED output support.
    pub has_leds: bool,
    /// Whether the device has a display.
    pub has_display: bool,
    /// Whether the device has a gear lever.
    pub has_gear_lever: bool,
    /// Number of toggle switches.
    pub toggle_switch_count: u8,
}

// ── Default axis/deadzone constants ──────────────────────────────────────────

/// Default deadzone for yoke primary axes (roll/pitch).
const YOKE_DEADZONE: f32 = 0.02;

/// Default expo for yoke axes.
const YOKE_EXPO: f32 = 0.15;

/// Default deadzone for rudder (twist or pedals).
const RUDDER_DEADZONE: f32 = 0.03;

/// Default expo for rudder axes.
const RUDDER_EXPO: f32 = 0.10;

/// Default deadzone for throttle levers.
const THROTTLE_DEADZONE: f32 = 0.01;

/// Default deadzone for joystick primary axes.
const STICK_DEADZONE: f32 = 0.03;

/// Default expo for joystick axes.
const STICK_EXPO: f32 = 0.20;

/// Default deadzone for toe brake axes.
const BRAKE_DEADZONE: f32 = 0.02;

// ── VelocityOne Flight ───────────────────────────────────────────────────────

/// Axis mappings for the VelocityOne Flight.
pub static FLIGHT_AXES: &[AxisMapping] = &[
    AxisMapping {
        name: "roll",
        index: 0,
        bipolar: true,
        deadzone: YOKE_DEADZONE,
        expo: YOKE_EXPO,
        sim_var_hint: "AILERON",
    },
    AxisMapping {
        name: "pitch",
        index: 1,
        bipolar: true,
        deadzone: YOKE_DEADZONE,
        expo: YOKE_EXPO,
        sim_var_hint: "ELEVATOR",
    },
    AxisMapping {
        name: "rudder_twist",
        index: 2,
        bipolar: true,
        deadzone: RUDDER_DEADZONE,
        expo: RUDDER_EXPO,
        sim_var_hint: "RUDDER",
    },
    AxisMapping {
        name: "throttle_left",
        index: 3,
        bipolar: false,
        deadzone: THROTTLE_DEADZONE,
        expo: 0.0,
        sim_var_hint: "THROTTLE 1",
    },
    AxisMapping {
        name: "throttle_right",
        index: 4,
        bipolar: false,
        deadzone: THROTTLE_DEADZONE,
        expo: 0.0,
        sim_var_hint: "THROTTLE 2",
    },
    AxisMapping {
        name: "trim_wheel",
        index: 5,
        bipolar: true,
        deadzone: 0.0,
        expo: 0.0,
        sim_var_hint: "ELEVATOR TRIM",
    },
];

/// Notable button mappings for the VelocityOne Flight.
pub static FLIGHT_BUTTONS: &[ButtonMapping] = &[
    ButtonMapping {
        name: "AP Master",
        button_num: 1,
        sim_event_hint: "AP_MASTER",
    },
    ButtonMapping {
        name: "HDG",
        button_num: 2,
        sim_event_hint: "AP_HDG_HOLD",
    },
    ButtonMapping {
        name: "NAV",
        button_num: 3,
        sim_event_hint: "AP_NAV1_HOLD",
    },
    ButtonMapping {
        name: "ALT",
        button_num: 4,
        sim_event_hint: "AP_ALT_HOLD",
    },
    ButtonMapping {
        name: "VS",
        button_num: 5,
        sim_event_hint: "AP_VS_HOLD",
    },
    ButtonMapping {
        name: "Hat Up",
        button_num: 10,
        sim_event_hint: "VIEW_UP",
    },
    ButtonMapping {
        name: "Hat Right",
        button_num: 11,
        sim_event_hint: "VIEW_RIGHT",
    },
    ButtonMapping {
        name: "Hat Down",
        button_num: 12,
        sim_event_hint: "VIEW_DOWN",
    },
    ButtonMapping {
        name: "Hat Left",
        button_num: 13,
        sim_event_hint: "VIEW_LEFT",
    },
    ButtonMapping {
        name: "Gear UP",
        button_num: 31,
        sim_event_hint: "GEAR_UP",
    },
    ButtonMapping {
        name: "Gear DOWN",
        button_num: 32,
        sim_event_hint: "GEAR_DOWN",
    },
    ButtonMapping {
        name: "Display Mode",
        button_num: 40,
        sim_event_hint: "MFD_PAGE_SELECT",
    },
];

/// VelocityOne Flight default profile.
pub static FLIGHT_PROFILE: DeviceProfile = DeviceProfile {
    name: "Turtle Beach VelocityOne Flight",
    vendor_id: devices::TURTLE_BEACH_VID,
    product_id: devices::VELOCITYONE_FLIGHT_PID,
    axes: FLIGHT_AXES,
    buttons: FLIGHT_BUTTONS,
    has_leds: true,
    has_display: true,
    has_gear_lever: true,
    toggle_switch_count: 7,
};

// ── VelocityOne Flightstick ──────────────────────────────────────────────────

/// Axis mappings for the VelocityOne Flightstick.
pub static FLIGHTSTICK_AXES: &[AxisMapping] = &[
    AxisMapping {
        name: "x",
        index: 0,
        bipolar: true,
        deadzone: STICK_DEADZONE,
        expo: STICK_EXPO,
        sim_var_hint: "AILERON",
    },
    AxisMapping {
        name: "y",
        index: 1,
        bipolar: true,
        deadzone: STICK_DEADZONE,
        expo: STICK_EXPO,
        sim_var_hint: "ELEVATOR",
    },
    AxisMapping {
        name: "twist",
        index: 2,
        bipolar: true,
        deadzone: RUDDER_DEADZONE,
        expo: RUDDER_EXPO,
        sim_var_hint: "RUDDER",
    },
    AxisMapping {
        name: "throttle",
        index: 3,
        bipolar: false,
        deadzone: THROTTLE_DEADZONE,
        expo: 0.0,
        sim_var_hint: "THROTTLE 1",
    },
];

/// Notable button mappings for the VelocityOne Flightstick.
pub static FLIGHTSTICK_BUTTONS: &[ButtonMapping] = &[
    ButtonMapping {
        name: "Trigger",
        button_num: 1,
        sim_event_hint: "WEAPON_FIRE",
    },
    ButtonMapping {
        name: "Thumb",
        button_num: 2,
        sim_event_hint: "WEAPON_SECONDARY",
    },
    ButtonMapping {
        name: "Hat Up",
        button_num: 5,
        sim_event_hint: "VIEW_UP",
    },
    ButtonMapping {
        name: "Hat Right",
        button_num: 6,
        sim_event_hint: "VIEW_RIGHT",
    },
    ButtonMapping {
        name: "Hat Down",
        button_num: 7,
        sim_event_hint: "VIEW_DOWN",
    },
    ButtonMapping {
        name: "Hat Left",
        button_num: 8,
        sim_event_hint: "VIEW_LEFT",
    },
];

/// VelocityOne Flightstick default profile.
pub static FLIGHTSTICK_PROFILE: DeviceProfile = DeviceProfile {
    name: "Turtle Beach VelocityOne Flightstick",
    vendor_id: devices::TURTLE_BEACH_VID,
    product_id: devices::VELOCITYONE_FLIGHTSTICK_PID,
    axes: FLIGHTSTICK_AXES,
    buttons: FLIGHTSTICK_BUTTONS,
    has_leds: false,
    has_display: false,
    has_gear_lever: false,
    toggle_switch_count: 0,
};

// ── VelocityOne Rudder ───────────────────────────────────────────────────────

/// Axis mappings for the VelocityOne Rudder pedals.
pub static RUDDER_AXES: &[AxisMapping] = &[
    AxisMapping {
        name: "rudder",
        index: 0,
        bipolar: true,
        deadzone: RUDDER_DEADZONE,
        expo: RUDDER_EXPO,
        sim_var_hint: "RUDDER",
    },
    AxisMapping {
        name: "left_toe_brake",
        index: 1,
        bipolar: false,
        deadzone: BRAKE_DEADZONE,
        expo: 0.0,
        sim_var_hint: "BRAKE LEFT",
    },
    AxisMapping {
        name: "right_toe_brake",
        index: 2,
        bipolar: false,
        deadzone: BRAKE_DEADZONE,
        expo: 0.0,
        sim_var_hint: "BRAKE RIGHT",
    },
];

/// Rudder pedals have no buttons.
pub static RUDDER_BUTTONS: &[ButtonMapping] = &[];

/// VelocityOne Rudder default profile.
pub static RUDDER_PROFILE: DeviceProfile = DeviceProfile {
    name: "Turtle Beach VelocityOne Rudder",
    vendor_id: devices::TURTLE_BEACH_VID,
    product_id: devices::VELOCITYONE_RUDDER_PID,
    axes: RUDDER_AXES,
    buttons: RUDDER_BUTTONS,
    has_leds: false,
    has_display: false,
    has_gear_lever: false,
    toggle_switch_count: 0,
};

// ── Profile lookup ───────────────────────────────────────────────────────────

/// Look up the default profile for a VelocityOne device.
///
/// Returns profiles for the three primary devices. Other variants (Flight Pro,
/// Flight Universal, Flight Yoke) share the Flight profile as a baseline.
pub fn profile_for_device(device: VelocityOneDevice) -> &'static DeviceProfile {
    match device {
        VelocityOneDevice::Flight
        | VelocityOneDevice::FlightPro
        | VelocityOneDevice::FlightUniversal
        | VelocityOneDevice::FlightYoke => &FLIGHT_PROFILE,
        VelocityOneDevice::Flightstick => &FLIGHTSTICK_PROFILE,
        VelocityOneDevice::Rudder => &RUDDER_PROFILE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flight_profile_completeness() {
        assert_eq!(FLIGHT_PROFILE.axes.len(), 6);
        assert!(FLIGHT_PROFILE.has_leds);
        assert!(FLIGHT_PROFILE.has_display);
        assert!(FLIGHT_PROFILE.has_gear_lever);
        assert!(FLIGHT_PROFILE.toggle_switch_count >= 6);
    }

    #[test]
    fn test_flightstick_profile_completeness() {
        assert_eq!(FLIGHTSTICK_PROFILE.axes.len(), 4);
        assert!(!FLIGHTSTICK_PROFILE.has_leds);
        assert!(!FLIGHTSTICK_PROFILE.has_display);
        assert_eq!(FLIGHTSTICK_PROFILE.toggle_switch_count, 0);
    }

    #[test]
    fn test_rudder_profile_completeness() {
        assert_eq!(RUDDER_PROFILE.axes.len(), 3);
        assert!(!RUDDER_PROFILE.has_leds);
        assert!(RUDDER_PROFILE.buttons.is_empty());
        // Rudder is bipolar, brakes are unipolar
        assert!(RUDDER_PROFILE.axes[0].bipolar);
        assert!(!RUDDER_PROFILE.axes[1].bipolar);
        assert!(!RUDDER_PROFILE.axes[2].bipolar);
    }

    #[test]
    fn test_profile_for_device_returns_correct() {
        let flight = profile_for_device(VelocityOneDevice::Flight);
        assert_eq!(flight.name, "Turtle Beach VelocityOne Flight");

        let stick = profile_for_device(VelocityOneDevice::Flightstick);
        assert_eq!(stick.name, "Turtle Beach VelocityOne Flightstick");

        let rudder = profile_for_device(VelocityOneDevice::Rudder);
        assert_eq!(rudder.name, "Turtle Beach VelocityOne Rudder");
    }

    #[test]
    fn test_flight_pro_uses_flight_profile() {
        let pro = profile_for_device(VelocityOneDevice::FlightPro);
        assert_eq!(pro.name, FLIGHT_PROFILE.name);
    }

    #[test]
    fn test_all_axes_have_names() {
        for profile in [&FLIGHT_PROFILE, &FLIGHTSTICK_PROFILE, &RUDDER_PROFILE] {
            for axis in profile.axes {
                assert!(!axis.name.is_empty(), "axis name must not be empty");
                assert!(
                    !axis.sim_var_hint.is_empty(),
                    "sim_var_hint must not be empty for {}",
                    axis.name
                );
            }
        }
    }

    #[test]
    fn test_all_buttons_have_valid_numbers() {
        for profile in [&FLIGHT_PROFILE, &FLIGHTSTICK_PROFILE, &RUDDER_PROFILE] {
            for btn in profile.buttons {
                assert!(btn.button_num >= 1, "button number must be ≥1");
                assert!(!btn.name.is_empty());
                assert!(!btn.sim_event_hint.is_empty());
            }
        }
    }

    #[test]
    fn test_all_profiles_have_correct_vendor_id() {
        for profile in [&FLIGHT_PROFILE, &FLIGHTSTICK_PROFILE, &RUDDER_PROFILE] {
            assert_eq!(profile.vendor_id, devices::TURTLE_BEACH_VID);
        }
    }

    #[test]
    fn test_axis_deadzones_in_range() {
        for profile in [&FLIGHT_PROFILE, &FLIGHTSTICK_PROFILE, &RUDDER_PROFILE] {
            for axis in profile.axes {
                assert!(
                    axis.deadzone >= 0.0 && axis.deadzone <= 1.0,
                    "deadzone out of range for {}",
                    axis.name
                );
            }
        }
    }

    #[test]
    fn test_axis_expos_in_range() {
        for profile in [&FLIGHT_PROFILE, &FLIGHTSTICK_PROFILE, &RUDDER_PROFILE] {
            for axis in profile.axes {
                assert!(
                    axis.expo >= 0.0 && axis.expo <= 1.0,
                    "expo out of range for {}",
                    axis.name
                );
            }
        }
    }
}
