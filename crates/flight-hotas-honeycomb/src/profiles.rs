// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Default device configuration profiles for Honeycomb Aeronautical products.
//!
//! Each profile describes the axis mapping, button assignments, and
//! device-specific features for a Honeycomb device. These are used by the
//! profile pipeline to generate baseline configurations that users can
//! customise.

use crate::presets;

/// Axis mapping entry in a device profile.
#[derive(Debug, Clone, PartialEq)]
pub struct AxisMapping {
    /// Human-readable axis name (e.g., "roll", "throttle1").
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
    /// Human-readable button name (e.g., "AP Master", "Gear Up").
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
    /// Button mappings (notable buttons only; not all 36/64 are listed).
    pub buttons: &'static [ButtonMapping],
    /// Whether the device has LED output support.
    pub has_leds: bool,
    /// Whether the device has encoder inputs.
    pub has_encoders: bool,
    /// Whether the device has a magneto switch.
    pub has_magneto: bool,
}

// ── Alpha Flight Controls XPC ────────────────────────────────────────────────

/// Axis mappings for the Honeycomb Alpha Yoke.
pub static ALPHA_AXES: &[AxisMapping] = &[
    AxisMapping {
        name: presets::alpha_axes::ROLL,
        index: 0,
        bipolar: true,
        deadzone: presets::ALPHA_AXIS_DEADZONE,
        expo: presets::ALPHA_AXIS_EXPO,
        sim_var_hint: "AILERON",
    },
    AxisMapping {
        name: presets::alpha_axes::PITCH,
        index: 1,
        bipolar: true,
        deadzone: presets::ALPHA_AXIS_DEADZONE,
        expo: presets::ALPHA_AXIS_EXPO,
        sim_var_hint: "ELEVATOR",
    },
];

/// Notable button mappings for the Alpha Yoke.
pub static ALPHA_BUTTONS: &[ButtonMapping] = &[
    ButtonMapping {
        name: "PTT (Push-To-Talk)",
        button_num: 1,
        sim_event_hint: "PILOT_TRANSMIT",
    },
    ButtonMapping {
        name: "AP Disconnect",
        button_num: 2,
        sim_event_hint: "AUTOPILOT_OFF",
    },
    ButtonMapping {
        name: "Hat Up",
        button_num: 3,
        sim_event_hint: "VIEW_UP",
    },
    ButtonMapping {
        name: "Hat Right",
        button_num: 4,
        sim_event_hint: "VIEW_RIGHT",
    },
    ButtonMapping {
        name: "Hat Down",
        button_num: 5,
        sim_event_hint: "VIEW_DOWN",
    },
    ButtonMapping {
        name: "Hat Left",
        button_num: 6,
        sim_event_hint: "VIEW_LEFT",
    },
    ButtonMapping {
        name: "Magneto A (Right)",
        button_num: 25,
        sim_event_hint: "MAGNETO_RIGHT",
    },
    ButtonMapping {
        name: "Magneto B (Left)",
        button_num: 26,
        sim_event_hint: "MAGNETO_LEFT",
    },
    ButtonMapping {
        name: "Starter",
        button_num: 27,
        sim_event_hint: "MAGNETO_START",
    },
];

/// Alpha Yoke default profile.
pub static ALPHA_PROFILE: DeviceProfile = DeviceProfile {
    name: "Honeycomb Alpha Flight Controls XPC",
    vendor_id: crate::HONEYCOMB_VENDOR_ID,
    product_id: crate::HONEYCOMB_ALPHA_YOKE_PID,
    axes: ALPHA_AXES,
    buttons: ALPHA_BUTTONS,
    has_leds: false,
    has_encoders: false,
    has_magneto: true,
};

// ── Bravo Throttle Quadrant ──────────────────────────────────────────────────

/// Axis mappings for the Honeycomb Bravo Throttle Quadrant.
pub static BRAVO_AXES: &[AxisMapping] = &[
    AxisMapping {
        name: presets::bravo_axes::THROTTLE1,
        index: 0,
        bipolar: false,
        deadzone: presets::BRAVO_THROTTLE_DEADZONE_IDLE,
        expo: 0.0,
        sim_var_hint: "THROTTLE 1",
    },
    AxisMapping {
        name: presets::bravo_axes::THROTTLE2,
        index: 1,
        bipolar: false,
        deadzone: presets::BRAVO_THROTTLE_DEADZONE_IDLE,
        expo: 0.0,
        sim_var_hint: "THROTTLE 2",
    },
    AxisMapping {
        name: presets::bravo_axes::THROTTLE3,
        index: 2,
        bipolar: false,
        deadzone: presets::BRAVO_THROTTLE_DEADZONE_IDLE,
        expo: 0.0,
        sim_var_hint: "PROPELLER 1",
    },
    AxisMapping {
        name: presets::bravo_axes::THROTTLE4,
        index: 3,
        bipolar: false,
        deadzone: presets::BRAVO_THROTTLE_DEADZONE_IDLE,
        expo: 0.0,
        sim_var_hint: "MIXTURE 1",
    },
    AxisMapping {
        name: presets::bravo_axes::FLAP_LEVER,
        index: 5,
        bipolar: false,
        deadzone: 0.0,
        expo: 0.0,
        sim_var_hint: "FLAPS",
    },
    AxisMapping {
        name: presets::bravo_axes::SPOILER,
        index: 6,
        bipolar: false,
        deadzone: 0.0,
        expo: 0.0,
        sim_var_hint: "SPOILERS",
    },
];

/// Notable button mappings for the Bravo Throttle Quadrant.
pub static BRAVO_BUTTONS: &[ButtonMapping] = &[
    ButtonMapping {
        name: "HDG",
        button_num: 1,
        sim_event_hint: "AP_HDG_HOLD",
    },
    ButtonMapping {
        name: "NAV",
        button_num: 2,
        sim_event_hint: "AP_NAV1_HOLD",
    },
    ButtonMapping {
        name: "APR",
        button_num: 3,
        sim_event_hint: "AP_APR_HOLD",
    },
    ButtonMapping {
        name: "REV",
        button_num: 4,
        sim_event_hint: "AP_BC_HOLD",
    },
    ButtonMapping {
        name: "ALT",
        button_num: 5,
        sim_event_hint: "AP_ALT_HOLD",
    },
    ButtonMapping {
        name: "VS",
        button_num: 6,
        sim_event_hint: "AP_VS_HOLD",
    },
    ButtonMapping {
        name: "IAS",
        button_num: 7,
        sim_event_hint: "AP_AIRSPEED_HOLD",
    },
    ButtonMapping {
        name: "AP Master",
        button_num: 8,
        sim_event_hint: "AP_MASTER",
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
];

/// Bravo Throttle Quadrant default profile.
pub static BRAVO_PROFILE: DeviceProfile = DeviceProfile {
    name: "Honeycomb Bravo Throttle Quadrant",
    vendor_id: crate::HONEYCOMB_VENDOR_ID,
    product_id: crate::HONEYCOMB_BRAVO_PID,
    axes: BRAVO_AXES,
    buttons: BRAVO_BUTTONS,
    has_leds: true,
    has_encoders: true,
    has_magneto: false,
};

// ── Charlie Rudder Pedals ────────────────────────────────────────────────────

/// Axis mappings for the Honeycomb Charlie Rudder Pedals.
pub static CHARLIE_AXES: &[AxisMapping] = &[
    AxisMapping {
        name: presets::charlie_axes::RUDDER,
        index: 0,
        bipolar: true,
        deadzone: presets::CHARLIE_RUDDER_DEADZONE,
        expo: presets::CHARLIE_RUDDER_EXPO,
        sim_var_hint: "RUDDER",
    },
    AxisMapping {
        name: presets::charlie_axes::LEFT_BRAKE,
        index: 1,
        bipolar: false,
        deadzone: presets::CHARLIE_BRAKE_DEADZONE,
        expo: 0.0,
        sim_var_hint: "BRAKE LEFT",
    },
    AxisMapping {
        name: presets::charlie_axes::RIGHT_BRAKE,
        index: 2,
        bipolar: false,
        deadzone: presets::CHARLIE_BRAKE_DEADZONE,
        expo: 0.0,
        sim_var_hint: "BRAKE RIGHT",
    },
];

/// Charlie Rudder Pedals have no buttons.
pub static CHARLIE_BUTTONS: &[ButtonMapping] = &[];

/// Charlie Rudder Pedals default profile.
pub static CHARLIE_PROFILE: DeviceProfile = DeviceProfile {
    name: "Honeycomb Charlie Rudder Pedals",
    vendor_id: crate::HONEYCOMB_VENDOR_ID,
    product_id: crate::HONEYCOMB_CHARLIE_PID,
    axes: CHARLIE_AXES,
    buttons: CHARLIE_BUTTONS,
    has_leds: false,
    has_encoders: false,
    has_magneto: false,
};

/// Look up the default profile for a Honeycomb device by model.
pub fn profile_for_model(model: crate::HoneycombModel) -> &'static DeviceProfile {
    match model {
        crate::HoneycombModel::AlphaYoke => &ALPHA_PROFILE,
        crate::HoneycombModel::BravoThrottle => &BRAVO_PROFILE,
        crate::HoneycombModel::CharliePedals => &CHARLIE_PROFILE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alpha_profile_completeness() {
        assert_eq!(ALPHA_PROFILE.axes.len(), 2);
        assert!(ALPHA_PROFILE.has_magneto);
        assert!(!ALPHA_PROFILE.has_leds);
        assert!(!ALPHA_PROFILE.has_encoders);
        assert!(ALPHA_PROFILE.axes.iter().all(|a| a.bipolar));
        assert!(ALPHA_PROFILE.axes.iter().all(|a| a.deadzone >= 0.0));
    }

    #[test]
    fn test_bravo_profile_completeness() {
        assert_eq!(BRAVO_PROFILE.axes.len(), 6);
        assert!(BRAVO_PROFILE.has_leds);
        assert!(BRAVO_PROFILE.has_encoders);
        assert!(!BRAVO_PROFILE.has_magneto);
        assert!(BRAVO_PROFILE.axes.iter().all(|a| !a.bipolar));
    }

    #[test]
    fn test_charlie_profile_completeness() {
        assert_eq!(CHARLIE_PROFILE.axes.len(), 3);
        assert!(!CHARLIE_PROFILE.has_leds);
        assert!(!CHARLIE_PROFILE.has_encoders);
        assert!(!CHARLIE_PROFILE.has_magneto);
        assert!(CHARLIE_PROFILE.buttons.is_empty());

        // Rudder is bipolar, brakes are unipolar
        assert!(CHARLIE_PROFILE.axes[0].bipolar);
        assert!(!CHARLIE_PROFILE.axes[1].bipolar);
        assert!(!CHARLIE_PROFILE.axes[2].bipolar);
    }

    #[test]
    fn test_profile_for_model_returns_correct_profile() {
        let alpha = profile_for_model(crate::HoneycombModel::AlphaYoke);
        assert_eq!(alpha.name, "Honeycomb Alpha Flight Controls XPC");

        let bravo = profile_for_model(crate::HoneycombModel::BravoThrottle);
        assert_eq!(bravo.name, "Honeycomb Bravo Throttle Quadrant");

        let charlie = profile_for_model(crate::HoneycombModel::CharliePedals);
        assert_eq!(charlie.name, "Honeycomb Charlie Rudder Pedals");
    }

    #[test]
    fn test_all_axes_have_names() {
        for profile in [&ALPHA_PROFILE, &BRAVO_PROFILE, &CHARLIE_PROFILE] {
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
        for profile in [&ALPHA_PROFILE, &BRAVO_PROFILE, &CHARLIE_PROFILE] {
            for btn in profile.buttons {
                assert!(btn.button_num >= 1, "button number must be ≥1");
                assert!(!btn.name.is_empty());
                assert!(!btn.sim_event_hint.is_empty());
            }
        }
    }

    #[test]
    fn test_all_profiles_have_correct_vendor_id() {
        for profile in [&ALPHA_PROFILE, &BRAVO_PROFILE, &CHARLIE_PROFILE] {
            assert_eq!(profile.vendor_id, crate::HONEYCOMB_VENDOR_ID);
        }
    }

    #[test]
    fn test_axis_deadzones_non_negative() {
        for profile in [&ALPHA_PROFILE, &BRAVO_PROFILE, &CHARLIE_PROFILE] {
            for axis in profile.axes {
                assert!(
                    axis.deadzone >= 0.0,
                    "deadzone must be non-negative for {}",
                    axis.name
                );
                assert!(
                    axis.deadzone <= 1.0,
                    "deadzone must be ≤1.0 for {}",
                    axis.name
                );
            }
        }
    }

    #[test]
    fn test_axis_expos_in_range() {
        for profile in [&ALPHA_PROFILE, &BRAVO_PROFILE, &CHARLIE_PROFILE] {
            for axis in profile.axes {
                assert!(
                    axis.expo >= 0.0,
                    "expo must be non-negative for {}",
                    axis.name
                );
                assert!(axis.expo <= 1.0, "expo must be ≤1.0 for {}", axis.name);
            }
        }
    }
}
