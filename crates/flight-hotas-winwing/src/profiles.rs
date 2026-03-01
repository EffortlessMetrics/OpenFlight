// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Default device profiles for all supported WinWing products.
//!
//! Each profile describes:
//! - **Axes** — named analog inputs with deadzone/filter recommendations.
//! - **Buttons** — count, grouped by section (stick, throttle base, panel).
//! - **HATs** — count and type (4-way / 8-way).
//! - **Encoders** — count and detent type.
//! - **Detents** — throttle detent positions (idle, afterburner, custom).
//! - **Displays** — field count and type for panels with screens.
//! - **Backlighting** — number of individually-addressable LEDs.
//!
//! # Supported devices
//!
//! | Device | Function |
//! |--------|----------|
//! | Orion 2 HOTAS Base | Joystick gimbal (X/Y/twist) |
//! | Orion 2 Throttle | Dual throttle + encoders + slew |
//! | F-16EX Grip | F-16 replica stick grip |
//! | F-18 Grip | F/A-18C replica stick grip |
//! | A-10 Grip | A-10C Warthog replica grip |
//! | Take Off Panel (TOP) | Encoders + 7-seg displays |
//! | Combat Ready Panel | Backlit buttons |
//! | FCU (Flight Control Unit) | Airbus FCU with encoders + displays |
//! | EFIS | Airbus EFIS panel with encoders + buttons |

use crate::presets::RecommendedAxisConfig;

// ── Common types ──────────────────────────────────────────────────────────────

/// A hat switch descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HatDescriptor {
    /// Human-readable name (e.g. "Trim Hat", "TDC").
    pub name: &'static str,
    /// Number of positions (4 for 4-way, 8 for 8-way), excluding neutral.
    pub positions: u8,
}

/// An encoder descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncoderDescriptor {
    pub name: &'static str,
    /// `true` if the encoder has a push-button.
    pub has_push: bool,
}

/// A detent position in a throttle profile.
#[derive(Debug, Clone, PartialEq)]
pub struct DetentDescriptor {
    pub name: &'static str,
    /// Typical normalised position \[0.0, 1.0\] for this detent.
    pub typical_position: f32,
}

/// A display field on a panel device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayFieldDescriptor {
    pub name: &'static str,
    /// Display type: `"7seg"`, `"lcd"`, `"led-annunciator"`.
    pub display_type: &'static str,
    /// Number of characters / digits the field can show.
    pub width: u8,
}

/// A button group descriptor (logical grouping of buttons).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ButtonGroupDescriptor {
    pub name: &'static str,
    pub count: u8,
}

// ── Device profile ────────────────────────────────────────────────────────────

/// Complete device profile for a WinWing product.
#[derive(Debug, Clone)]
pub struct DeviceProfile {
    /// Human-readable device name.
    pub name: &'static str,
    /// USB Vendor ID.
    pub vid: u16,
    /// USB Product ID.
    pub pid: u16,
    /// Named axis configurations.
    pub axes: Vec<RecommendedAxisConfig>,
    /// Total button count.
    pub button_count: u8,
    /// Button groups (logical sections).
    pub button_groups: Vec<ButtonGroupDescriptor>,
    /// HAT switches.
    pub hats: Vec<HatDescriptor>,
    /// Rotary encoders.
    pub encoders: Vec<EncoderDescriptor>,
    /// Throttle detent positions (empty for non-throttle devices).
    pub detents: Vec<DetentDescriptor>,
    /// Display fields (empty for devices without displays).
    pub displays: Vec<DisplayFieldDescriptor>,
    /// Number of individually-addressable backlight LEDs (0 if none).
    pub backlight_led_count: u8,
}

// ── Orion 2 HOTAS Base ───────────────────────────────────────────────────────

/// Profile for the WinWing Orion 2 HOTAS Base (joystick gimbal).
///
/// The gimbal provides X/Y/twist axes. Grips are separate USB devices.
pub fn orion2_base_profile() -> DeviceProfile {
    DeviceProfile {
        name: "WinWing Orion 2 HOTAS Base",
        vid: 0x4098,
        pid: 0xBE63,
        axes: vec![
            RecommendedAxisConfig {
                name: "roll",
                deadzone: 0.01,
                filter_alpha: None,
                slew_rate: None,
                notes: "X axis — Hall-effect gimbal, very low noise",
            },
            RecommendedAxisConfig {
                name: "pitch",
                deadzone: 0.01,
                filter_alpha: None,
                slew_rate: None,
                notes: "Y axis — Hall-effect gimbal, very low noise",
            },
        ],
        button_count: 20,
        button_groups: vec![ButtonGroupDescriptor {
            name: "grip",
            count: 20,
        }],
        hats: vec![
            HatDescriptor {
                name: "HAT A",
                positions: 8,
            },
            HatDescriptor {
                name: "HAT B",
                positions: 8,
            },
        ],
        encoders: vec![],
        detents: vec![],
        displays: vec![],
        backlight_led_count: 0,
    }
}

// ── Orion 2 Throttle ─────────────────────────────────────────────────────────

/// Profile for the WinWing Orion 2 Throttle.
///
/// Dual split throttle with Hall-effect sensors, friction slider,
/// slew/mouse stick, 50 buttons, and 5 rotary encoders.
/// Includes idle and afterburner detent positions.
pub fn orion2_throttle_profile() -> DeviceProfile {
    DeviceProfile {
        name: "WinWing Orion 2 Throttle",
        vid: 0x4098,
        pid: 0xBE62,
        axes: vec![
            RecommendedAxisConfig {
                name: "throttle_left",
                deadzone: 0.01,
                filter_alpha: None,
                slew_rate: None,
                notes: "Left throttle lever — Hall-effect, very low noise",
            },
            RecommendedAxisConfig {
                name: "throttle_right",
                deadzone: 0.01,
                filter_alpha: None,
                slew_rate: None,
                notes: "Right throttle lever — Hall-effect, very low noise",
            },
            RecommendedAxisConfig {
                name: "friction",
                deadzone: 0.03,
                filter_alpha: Some(0.10),
                slew_rate: None,
                notes: "Friction slider — resistive; light filter",
            },
            RecommendedAxisConfig {
                name: "mouse_x",
                deadzone: 0.08,
                filter_alpha: Some(0.20),
                slew_rate: None,
                notes: "Slew/mouse stick X — spring return; larger deadzone",
            },
            RecommendedAxisConfig {
                name: "mouse_y",
                deadzone: 0.08,
                filter_alpha: Some(0.20),
                slew_rate: None,
                notes: "Slew/mouse stick Y — spring return; larger deadzone",
            },
        ],
        button_count: 50,
        button_groups: vec![
            ButtonGroupDescriptor {
                name: "throttle_base",
                count: 30,
            },
            ButtonGroupDescriptor {
                name: "throttle_levers",
                count: 10,
            },
            ButtonGroupDescriptor {
                name: "switches",
                count: 10,
            },
        ],
        hats: vec![],
        encoders: vec![
            EncoderDescriptor {
                name: "ENC1",
                has_push: true,
            },
            EncoderDescriptor {
                name: "ENC2",
                has_push: true,
            },
            EncoderDescriptor {
                name: "ENC3",
                has_push: true,
            },
            EncoderDescriptor {
                name: "ENC4",
                has_push: true,
            },
            EncoderDescriptor {
                name: "ENC5",
                has_push: true,
            },
        ],
        detents: vec![
            DetentDescriptor {
                name: "idle",
                typical_position: 0.0,
            },
            DetentDescriptor {
                name: "afterburner",
                typical_position: 0.95,
            },
        ],
        displays: vec![],
        backlight_led_count: 0,
    }
}

// ── F-16EX Grip ───────────────────────────────────────────────────────────────

/// Profile for the WinWing F-16EX Grip.
///
/// F-16 replica side-stick with 20 buttons and a single 8-way HAT.
/// Maps to real F-16 controls: trigger, pickle, NWS, CMS, DMS, TMS, etc.
pub fn f16ex_grip_profile() -> DeviceProfile {
    DeviceProfile {
        name: "WinWing F-16EX Grip",
        vid: 0x4098,
        pid: 0xBEA8,
        axes: vec![
            RecommendedAxisConfig {
                name: "roll",
                deadzone: 0.01,
                filter_alpha: None,
                slew_rate: None,
                notes: "Roll axis — from gimbal base, not grip",
            },
            RecommendedAxisConfig {
                name: "pitch",
                deadzone: 0.01,
                filter_alpha: None,
                slew_rate: None,
                notes: "Pitch axis — from gimbal base, not grip",
            },
        ],
        button_count: 20,
        button_groups: vec![
            ButtonGroupDescriptor {
                name: "trigger",
                count: 2,
            },
            ButtonGroupDescriptor {
                name: "paddle",
                count: 1,
            },
            ButtonGroupDescriptor {
                name: "cms",
                count: 5,
            },
            ButtonGroupDescriptor {
                name: "dms",
                count: 4,
            },
            ButtonGroupDescriptor {
                name: "tms",
                count: 4,
            },
            ButtonGroupDescriptor {
                name: "misc",
                count: 4,
            },
        ],
        hats: vec![HatDescriptor {
            name: "CMS HAT",
            positions: 8,
        }],
        encoders: vec![],
        detents: vec![],
        displays: vec![],
        backlight_led_count: 0,
    }
}

// ── F-18 Grip ─────────────────────────────────────────────────────────────────

/// Profile for the WinWing F/A-18C Grip (attached to Orion 2 Base).
///
/// F/A-18C Hornet replica centre-stick with 20 buttons and 2 HATs.
/// Maps to real F-18 controls: trigger, nosewheel steering, weapon release,
/// sensor control, trim, etc.
pub fn f18_grip_profile() -> DeviceProfile {
    DeviceProfile {
        name: "WinWing F/A-18C Grip",
        vid: 0x4098,
        pid: 0xBE63,
        axes: vec![
            RecommendedAxisConfig {
                name: "roll",
                deadzone: 0.01,
                filter_alpha: None,
                slew_rate: None,
                notes: "Roll axis — Hall-effect gimbal",
            },
            RecommendedAxisConfig {
                name: "pitch",
                deadzone: 0.01,
                filter_alpha: None,
                slew_rate: None,
                notes: "Pitch axis — Hall-effect gimbal",
            },
        ],
        button_count: 20,
        button_groups: vec![
            ButtonGroupDescriptor {
                name: "trigger",
                count: 2,
            },
            ButtonGroupDescriptor {
                name: "nws",
                count: 1,
            },
            ButtonGroupDescriptor {
                name: "weapon_release",
                count: 1,
            },
            ButtonGroupDescriptor {
                name: "sensor_control",
                count: 4,
            },
            ButtonGroupDescriptor {
                name: "trim",
                count: 4,
            },
            ButtonGroupDescriptor {
                name: "misc",
                count: 8,
            },
        ],
        hats: vec![
            HatDescriptor {
                name: "Trim HAT",
                positions: 8,
            },
            HatDescriptor {
                name: "Sensor HAT",
                positions: 8,
            },
        ],
        encoders: vec![],
        detents: vec![],
        displays: vec![],
        backlight_led_count: 0,
    }
}

// ── A-10 Grip ─────────────────────────────────────────────────────────────────

/// Profile for the WinWing A-10C Grip (estimated PID).
///
/// A-10C Warthog replica centre-stick with ~24 buttons and 2 HATs.
/// Includes DMS, TMS, CMS, boat switch, and trigger stages.
pub fn a10_grip_profile() -> DeviceProfile {
    DeviceProfile {
        name: "WinWing A-10C Grip",
        vid: 0x4098,
        pid: 0xBEB0, // estimated
        axes: vec![
            RecommendedAxisConfig {
                name: "roll",
                deadzone: 0.01,
                filter_alpha: None,
                slew_rate: None,
                notes: "Roll axis — from gimbal base",
            },
            RecommendedAxisConfig {
                name: "pitch",
                deadzone: 0.01,
                filter_alpha: None,
                slew_rate: None,
                notes: "Pitch axis — from gimbal base",
            },
        ],
        button_count: 24,
        button_groups: vec![
            ButtonGroupDescriptor {
                name: "trigger",
                count: 2,
            },
            ButtonGroupDescriptor {
                name: "pickle",
                count: 1,
            },
            ButtonGroupDescriptor {
                name: "pinky",
                count: 1,
            },
            ButtonGroupDescriptor {
                name: "dms",
                count: 4,
            },
            ButtonGroupDescriptor {
                name: "tms",
                count: 4,
            },
            ButtonGroupDescriptor {
                name: "cms",
                count: 5,
            },
            ButtonGroupDescriptor {
                name: "boat_switch",
                count: 3,
            },
            ButtonGroupDescriptor {
                name: "misc",
                count: 4,
            },
        ],
        hats: vec![
            HatDescriptor {
                name: "Trim HAT",
                positions: 8,
            },
            HatDescriptor {
                name: "CMS HAT",
                positions: 8,
            },
        ],
        encoders: vec![],
        detents: vec![],
        displays: vec![],
        backlight_led_count: 0,
    }
}

// ── Take Off Panel (TOP) ─────────────────────────────────────────────────────

/// Profile for the WinWing Take Off Panel (TOP).
///
/// Instrument-style panel with encoders, push-buttons, toggle switches,
/// and 7-segment display fields for parameter readouts.
pub fn take_off_panel_profile() -> DeviceProfile {
    DeviceProfile {
        name: "WinWing Take Off Panel",
        vid: 0x4098,
        pid: 0xBEE0, // estimated
        axes: vec![],
        button_count: 32,
        button_groups: vec![
            ButtonGroupDescriptor {
                name: "toggle_switches",
                count: 12,
            },
            ButtonGroupDescriptor {
                name: "push_buttons",
                count: 12,
            },
            ButtonGroupDescriptor {
                name: "encoder_push",
                count: 8,
            },
        ],
        hats: vec![],
        encoders: vec![
            EncoderDescriptor {
                name: "ALT",
                has_push: true,
            },
            EncoderDescriptor {
                name: "HDG",
                has_push: true,
            },
            EncoderDescriptor {
                name: "CRS",
                has_push: true,
            },
            EncoderDescriptor {
                name: "SPD",
                has_push: true,
            },
            EncoderDescriptor {
                name: "VS",
                has_push: true,
            },
            EncoderDescriptor {
                name: "BARO",
                has_push: true,
            },
            EncoderDescriptor {
                name: "AUX1",
                has_push: true,
            },
            EncoderDescriptor {
                name: "AUX2",
                has_push: true,
            },
        ],
        detents: vec![],
        displays: vec![
            DisplayFieldDescriptor {
                name: "ALT",
                display_type: "7seg",
                width: 5,
            },
            DisplayFieldDescriptor {
                name: "HDG",
                display_type: "7seg",
                width: 3,
            },
            DisplayFieldDescriptor {
                name: "CRS",
                display_type: "7seg",
                width: 3,
            },
            DisplayFieldDescriptor {
                name: "SPD",
                display_type: "7seg",
                width: 3,
            },
            DisplayFieldDescriptor {
                name: "VS",
                display_type: "7seg",
                width: 4,
            },
            DisplayFieldDescriptor {
                name: "BARO",
                display_type: "7seg",
                width: 4,
            },
        ],
        backlight_led_count: 32,
    }
}

// ── Combat Ready Panel ───────────────────────────────────────────────────────

/// Profile for the WinWing Combat Ready Panel.
///
/// Button panel with individually back-lit push buttons.
/// No axes, encoders, or displays.
pub fn combat_ready_panel_profile() -> DeviceProfile {
    DeviceProfile {
        name: "WinWing Combat Ready Panel",
        vid: 0x4098,
        pid: 0xBEE2, // estimated
        axes: vec![],
        button_count: 30,
        button_groups: vec![
            ButtonGroupDescriptor {
                name: "row_1",
                count: 10,
            },
            ButtonGroupDescriptor {
                name: "row_2",
                count: 10,
            },
            ButtonGroupDescriptor {
                name: "row_3",
                count: 10,
            },
        ],
        hats: vec![],
        encoders: vec![],
        detents: vec![],
        displays: vec![],
        backlight_led_count: 30,
    }
}

// ── FCU (Flight Control Unit) ─────────────────────────────────────────────────

/// Profile for the WinWing FCU (Airbus Flight Control Unit).
///
/// Replica Airbus A320 FCU with speed/heading/altitude/VS encoders,
/// AP/ATHR push-buttons, and 7-segment displays for each parameter.
pub fn fcu_panel_profile() -> DeviceProfile {
    DeviceProfile {
        name: "WinWing FCU (Airbus)",
        vid: 0x4098,
        pid: 0xBEE4, // estimated
        axes: vec![],
        button_count: 16,
        button_groups: vec![
            ButtonGroupDescriptor {
                name: "autopilot",
                count: 6,
            },
            ButtonGroupDescriptor {
                name: "mode_select",
                count: 4,
            },
            ButtonGroupDescriptor {
                name: "encoder_push",
                count: 6,
            },
        ],
        hats: vec![],
        encoders: vec![
            EncoderDescriptor {
                name: "SPD",
                has_push: true,
            },
            EncoderDescriptor {
                name: "HDG",
                has_push: true,
            },
            EncoderDescriptor {
                name: "ALT",
                has_push: true,
            },
            EncoderDescriptor {
                name: "VS/FPA",
                has_push: true,
            },
        ],
        detents: vec![],
        displays: vec![
            DisplayFieldDescriptor {
                name: "SPD",
                display_type: "7seg",
                width: 3,
            },
            DisplayFieldDescriptor {
                name: "HDG/TRK",
                display_type: "7seg",
                width: 3,
            },
            DisplayFieldDescriptor {
                name: "ALT",
                display_type: "7seg",
                width: 5,
            },
            DisplayFieldDescriptor {
                name: "VS/FPA",
                display_type: "7seg",
                width: 5,
            },
            DisplayFieldDescriptor {
                name: "annunciators",
                display_type: "led-annunciator",
                width: 8,
            },
        ],
        backlight_led_count: 16,
    }
}

// ── EFIS ──────────────────────────────────────────────────────────────────────

/// Profile for the WinWing EFIS (Airbus Electronic Flight Instrument System).
///
/// Replica Airbus A320 EFIS panel with range/mode selectors,
/// barometric setting, and ND filter buttons.
pub fn efis_panel_profile() -> DeviceProfile {
    DeviceProfile {
        name: "WinWing EFIS (Airbus)",
        vid: 0x4098,
        pid: 0xBEE6, // estimated
        axes: vec![],
        button_count: 14,
        button_groups: vec![
            ButtonGroupDescriptor {
                name: "nd_mode",
                count: 5,
            },
            ButtonGroupDescriptor {
                name: "nd_range",
                count: 1,
            },
            ButtonGroupDescriptor {
                name: "nd_filter",
                count: 6,
            },
            ButtonGroupDescriptor {
                name: "baro_push",
                count: 2,
            },
        ],
        hats: vec![],
        encoders: vec![
            EncoderDescriptor {
                name: "BARO",
                has_push: true,
            },
            EncoderDescriptor {
                name: "ND_RANGE",
                has_push: false,
            },
            EncoderDescriptor {
                name: "ND_MODE",
                has_push: false,
            },
        ],
        detents: vec![],
        displays: vec![
            DisplayFieldDescriptor {
                name: "BARO",
                display_type: "7seg",
                width: 4,
            },
            DisplayFieldDescriptor {
                name: "annunciators",
                display_type: "led-annunciator",
                width: 6,
            },
        ],
        backlight_led_count: 14,
    }
}

// ── Super Taurus F-15EX Throttle ─────────────────────────────────────────────

/// Profile for the WinWing Super Taurus F-15EX Dual Throttle.
///
/// Premium dual throttle with Hall-effect axes, encoders, and a detachable
/// mouse stick.  PID 0xBD64 is confirmed via linux-hardware.org (USB string
/// "SuperTaurus F-15EX Throttle").
pub fn super_taurus_profile() -> DeviceProfile {
    DeviceProfile {
        name: "WinWing Super Taurus F-15EX Throttle",
        vid: 0x4098,
        pid: 0xBD64, // confirmed
        axes: vec![
            RecommendedAxisConfig {
                name: "throttle_left",
                deadzone: 0.01,
                filter_alpha: None,
                slew_rate: None,
                notes: "Left throttle lever — Hall-effect",
            },
            RecommendedAxisConfig {
                name: "throttle_right",
                deadzone: 0.01,
                filter_alpha: None,
                slew_rate: None,
                notes: "Right throttle lever — Hall-effect",
            },
            RecommendedAxisConfig {
                name: "friction",
                deadzone: 0.03,
                filter_alpha: Some(0.10),
                slew_rate: None,
                notes: "Friction slider — resistive",
            },
            RecommendedAxisConfig {
                name: "mouse_x",
                deadzone: 0.08,
                filter_alpha: Some(0.20),
                slew_rate: None,
                notes: "Mouse/slew X — spring return",
            },
            RecommendedAxisConfig {
                name: "mouse_y",
                deadzone: 0.08,
                filter_alpha: Some(0.20),
                slew_rate: None,
                notes: "Mouse/slew Y — spring return",
            },
        ],
        button_count: 58,
        button_groups: vec![
            ButtonGroupDescriptor {
                name: "left_panel",
                count: 20,
            },
            ButtonGroupDescriptor {
                name: "right_panel",
                count: 20,
            },
            ButtonGroupDescriptor {
                name: "centre",
                count: 10,
            },
            ButtonGroupDescriptor {
                name: "encoder_push",
                count: 8,
            },
        ],
        hats: vec![HatDescriptor {
            name: "Slew HAT",
            positions: 8,
        }],
        encoders: vec![
            EncoderDescriptor {
                name: "ENC1",
                has_push: true,
            },
            EncoderDescriptor {
                name: "ENC2",
                has_push: true,
            },
            EncoderDescriptor {
                name: "ENC3",
                has_push: true,
            },
            EncoderDescriptor {
                name: "ENC4",
                has_push: true,
            },
            EncoderDescriptor {
                name: "ENC5",
                has_push: true,
            },
            EncoderDescriptor {
                name: "ENC6",
                has_push: true,
            },
            EncoderDescriptor {
                name: "ENC7",
                has_push: true,
            },
            EncoderDescriptor {
                name: "ENC8",
                has_push: true,
            },
        ],
        detents: vec![
            DetentDescriptor {
                name: "idle",
                typical_position: 0.02,
            },
            DetentDescriptor {
                name: "afterburner",
                typical_position: 0.95,
            },
        ],
        displays: vec![],
        backlight_led_count: 0,
    }
}

// ── Super Libra Joystick Base ────────────────────────────────────────────────

/// Profile for the WinWing Super Libra centre-mount joystick base.
///
/// High-end Hall-effect gimbal (roll/pitch).  Grip buttons are reported
/// through the same USB composite device.  PID 0xBD70 is a community
/// estimate.
pub fn super_libra_profile() -> DeviceProfile {
    DeviceProfile {
        name: "WinWing Super Libra Joystick Base",
        vid: 0x4098,
        pid: 0xBD70, // community estimate
        axes: vec![
            RecommendedAxisConfig {
                name: "roll",
                deadzone: 0.01,
                filter_alpha: None,
                slew_rate: None,
                notes: "Roll axis — Hall-effect centre-mount gimbal",
            },
            RecommendedAxisConfig {
                name: "pitch",
                deadzone: 0.01,
                filter_alpha: None,
                slew_rate: None,
                notes: "Pitch axis — Hall-effect centre-mount gimbal",
            },
        ],
        button_count: 24,
        button_groups: vec![
            ButtonGroupDescriptor {
                name: "trigger",
                count: 2,
            },
            ButtonGroupDescriptor {
                name: "grip",
                count: 14,
            },
            ButtonGroupDescriptor {
                name: "base",
                count: 8,
            },
        ],
        hats: vec![HatDescriptor {
            name: "Main HAT",
            positions: 8,
        }],
        encoders: vec![],
        detents: vec![],
        displays: vec![],
        backlight_led_count: 0,
    }
}

// ── Lookup ────────────────────────────────────────────────────────────────────

/// Look up a device profile by USB Product ID.
///
/// Returns `None` for unknown PIDs.
pub fn profile_by_pid(pid: u16) -> Option<DeviceProfile> {
    match pid {
        0xBE63 => Some(orion2_base_profile()),
        0xBE62 => Some(orion2_throttle_profile()),
        0xBEA8 => Some(f16ex_grip_profile()),
        0xBEB0 => Some(a10_grip_profile()),
        0xBD64 => Some(super_taurus_profile()),
        0xBD70 => Some(super_libra_profile()),
        0xBE04 => Some(take_off_panel_profile()),
        0xBE05 => Some(combat_ready_panel_profile()),
        0xBEE4 => Some(fcu_panel_profile()),
        0xBEE6 => Some(efis_panel_profile()),
        _ => None,
    }
}

/// Return all known device profiles.
pub fn all_profiles() -> Vec<DeviceProfile> {
    vec![
        orion2_base_profile(),
        orion2_throttle_profile(),
        f16ex_grip_profile(),
        f18_grip_profile(),
        a10_grip_profile(),
        super_taurus_profile(),
        super_libra_profile(),
        take_off_panel_profile(),
        combat_ready_panel_profile(),
        fcu_panel_profile(),
        efis_panel_profile(),
    ]
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Profile completeness ──────────────────────────────────────────────

    #[test]
    fn test_all_profiles_have_valid_vid() {
        for p in all_profiles() {
            assert_eq!(p.vid, 0x4098, "{} should have WinWing VID", p.name);
        }
    }

    #[test]
    fn test_all_profiles_have_nonzero_pid() {
        for p in all_profiles() {
            assert_ne!(p.pid, 0, "{} should have a non-zero PID", p.name);
        }
    }

    #[test]
    fn test_all_profiles_have_name() {
        for p in all_profiles() {
            assert!(!p.name.is_empty());
        }
    }

    #[test]
    fn test_all_profiles_count() {
        assert_eq!(all_profiles().len(), 11);
    }

    #[test]
    fn test_button_groups_sum_to_button_count() {
        for p in all_profiles() {
            let sum: u8 = p.button_groups.iter().map(|g| g.count).sum();
            assert_eq!(
                sum, p.button_count,
                "{}: button groups sum ({sum}) != button_count ({})",
                p.name, p.button_count
            );
        }
    }

    // ── Orion 2 Base ──────────────────────────────────────────────────────

    #[test]
    fn test_orion2_base_axes() {
        let p = orion2_base_profile();
        assert_eq!(p.axes.len(), 2, "base should have roll + pitch");
        assert_eq!(p.axes[0].name, "roll");
        assert_eq!(p.axes[1].name, "pitch");
    }

    #[test]
    fn test_orion2_base_hats() {
        let p = orion2_base_profile();
        assert_eq!(p.hats.len(), 2);
        assert!(p.hats.iter().all(|h| h.positions == 8));
    }

    #[test]
    fn test_orion2_base_no_detents() {
        assert!(orion2_base_profile().detents.is_empty());
    }

    #[test]
    fn test_orion2_base_no_displays() {
        assert!(orion2_base_profile().displays.is_empty());
    }

    // ── Orion 2 Throttle ──────────────────────────────────────────────────

    #[test]
    fn test_orion2_throttle_axes() {
        let p = orion2_throttle_profile();
        assert_eq!(p.axes.len(), 5, "throttle should have 5 axes");
        let names: Vec<_> = p.axes.iter().map(|a| a.name).collect();
        assert!(names.contains(&"throttle_left"));
        assert!(names.contains(&"throttle_right"));
        assert!(names.contains(&"friction"));
        assert!(names.contains(&"mouse_x"));
        assert!(names.contains(&"mouse_y"));
    }

    #[test]
    fn test_orion2_throttle_hall_effect_deadzones() {
        let p = orion2_throttle_profile();
        for ax in &p.axes[..2] {
            assert!(
                ax.deadzone < 0.02,
                "Hall-effect axis {} should have small deadzone",
                ax.name
            );
        }
    }

    #[test]
    fn test_orion2_throttle_encoders() {
        let p = orion2_throttle_profile();
        assert_eq!(p.encoders.len(), 5);
        assert!(p.encoders.iter().all(|e| e.has_push));
    }

    #[test]
    fn test_orion2_throttle_buttons() {
        let p = orion2_throttle_profile();
        assert_eq!(p.button_count, 50);
    }

    #[test]
    fn test_orion2_throttle_detents() {
        let p = orion2_throttle_profile();
        assert_eq!(p.detents.len(), 2);
        assert_eq!(p.detents[0].name, "idle");
        assert!(p.detents[0].typical_position < 0.05);
        assert_eq!(p.detents[1].name, "afterburner");
        assert!(p.detents[1].typical_position > 0.90);
    }

    // ── F-16EX Grip ───────────────────────────────────────────────────────

    #[test]
    fn test_f16ex_grip_buttons() {
        let p = f16ex_grip_profile();
        assert_eq!(p.button_count, 20);
    }

    #[test]
    fn test_f16ex_grip_hat() {
        let p = f16ex_grip_profile();
        assert_eq!(p.hats.len(), 1);
        assert_eq!(p.hats[0].positions, 8);
    }

    #[test]
    fn test_f16ex_grip_has_trigger_group() {
        let p = f16ex_grip_profile();
        assert!(p.button_groups.iter().any(|g| g.name == "trigger"));
    }

    #[test]
    fn test_f16ex_grip_no_displays() {
        assert!(f16ex_grip_profile().displays.is_empty());
    }

    // ── F-18 Grip ─────────────────────────────────────────────────────────

    #[test]
    fn test_f18_grip_buttons() {
        let p = f18_grip_profile();
        assert_eq!(p.button_count, 20);
    }

    #[test]
    fn test_f18_grip_two_hats() {
        let p = f18_grip_profile();
        assert_eq!(p.hats.len(), 2);
    }

    #[test]
    fn test_f18_grip_has_trigger_group() {
        let p = f18_grip_profile();
        assert!(p.button_groups.iter().any(|g| g.name == "trigger"));
    }

    // ── A-10 Grip ─────────────────────────────────────────────────────────

    #[test]
    fn test_a10_grip_buttons() {
        let p = a10_grip_profile();
        assert_eq!(p.button_count, 24);
    }

    #[test]
    fn test_a10_grip_has_boat_switch() {
        let p = a10_grip_profile();
        assert!(
            p.button_groups.iter().any(|g| g.name == "boat_switch"),
            "A-10 grip should have a boat switch group"
        );
    }

    #[test]
    fn test_a10_grip_two_hats() {
        let p = a10_grip_profile();
        assert_eq!(p.hats.len(), 2);
    }

    // ── Super Taurus F-15EX ──────────────────────────────────────────────

    #[test]
    fn test_super_taurus_axes() {
        let p = super_taurus_profile();
        assert_eq!(p.axes.len(), 5, "Super Taurus should have 5 axes");
        let names: Vec<_> = p.axes.iter().map(|a| a.name).collect();
        assert!(names.contains(&"throttle_left"));
        assert!(names.contains(&"throttle_right"));
    }

    #[test]
    fn test_super_taurus_buttons() {
        let p = super_taurus_profile();
        assert_eq!(p.button_count, 58);
    }

    #[test]
    fn test_super_taurus_confirmed_pid() {
        let p = super_taurus_profile();
        assert_eq!(p.pid, 0xBD64);
    }

    #[test]
    fn test_super_taurus_encoders() {
        let p = super_taurus_profile();
        assert_eq!(p.encoders.len(), 8);
        assert!(p.encoders.iter().all(|e| e.has_push));
    }

    #[test]
    fn test_super_taurus_detents() {
        let p = super_taurus_profile();
        assert_eq!(p.detents.len(), 2);
        assert_eq!(p.detents[0].name, "idle");
        assert_eq!(p.detents[1].name, "afterburner");
    }

    // ── Super Libra ──────────────────────────────────────────────────────

    #[test]
    fn test_super_libra_axes() {
        let p = super_libra_profile();
        assert_eq!(p.axes.len(), 2);
        assert_eq!(p.axes[0].name, "roll");
        assert_eq!(p.axes[1].name, "pitch");
    }

    #[test]
    fn test_super_libra_buttons() {
        let p = super_libra_profile();
        assert_eq!(p.button_count, 24);
    }

    #[test]
    fn test_super_libra_has_hat() {
        let p = super_libra_profile();
        assert_eq!(p.hats.len(), 1);
        assert_eq!(p.hats[0].positions, 8);
    }

    #[test]
    fn test_super_libra_no_detents() {
        assert!(super_libra_profile().detents.is_empty());
    }

    #[test]
    fn test_super_libra_hall_effect_deadzones() {
        let p = super_libra_profile();
        for ax in &p.axes {
            assert!(
                ax.deadzone < 0.02,
                "Hall-effect axis {} has deadzone {} (expected < 0.02)",
                ax.name,
                ax.deadzone
            );
        }
    }

    // ── Take Off Panel (TOP) ──────────────────────────────────────────────

    #[test]
    fn test_top_no_axes() {
        assert!(take_off_panel_profile().axes.is_empty());
    }

    #[test]
    fn test_top_encoders() {
        let p = take_off_panel_profile();
        assert_eq!(p.encoders.len(), 8);
        // ALT, HDG, CRS, SPD, VS, BARO, AUX1, AUX2
        let enc_names: Vec<_> = p.encoders.iter().map(|e| e.name).collect();
        assert!(enc_names.contains(&"ALT"));
        assert!(enc_names.contains(&"HDG"));
        assert!(enc_names.contains(&"BARO"));
    }

    #[test]
    fn test_top_displays() {
        let p = take_off_panel_profile();
        assert_eq!(p.displays.len(), 6);
        assert!(p.displays.iter().all(|d| d.display_type == "7seg"));
    }

    #[test]
    fn test_top_display_widths() {
        let p = take_off_panel_profile();
        let alt_display = p.displays.iter().find(|d| d.name == "ALT").unwrap();
        assert_eq!(alt_display.width, 5); // altitude needs 5 digits
        let hdg_display = p.displays.iter().find(|d| d.name == "HDG").unwrap();
        assert_eq!(hdg_display.width, 3); // heading 000–359
    }

    #[test]
    fn test_top_backlighting() {
        let p = take_off_panel_profile();
        assert_eq!(p.backlight_led_count, 32);
    }

    // ── Combat Ready Panel ────────────────────────────────────────────────

    #[test]
    fn test_crp_buttons() {
        let p = combat_ready_panel_profile();
        assert_eq!(p.button_count, 30);
    }

    #[test]
    fn test_crp_all_buttons_backlit() {
        let p = combat_ready_panel_profile();
        assert_eq!(
            p.backlight_led_count, p.button_count,
            "every CRP button should have a backlight"
        );
    }

    #[test]
    fn test_crp_no_axes_or_encoders() {
        let p = combat_ready_panel_profile();
        assert!(p.axes.is_empty());
        assert!(p.encoders.is_empty());
    }

    #[test]
    fn test_crp_no_displays() {
        assert!(combat_ready_panel_profile().displays.is_empty());
    }

    // ── FCU ───────────────────────────────────────────────────────────────

    #[test]
    fn test_fcu_encoders() {
        let p = fcu_panel_profile();
        assert_eq!(p.encoders.len(), 4);
        let enc_names: Vec<_> = p.encoders.iter().map(|e| e.name).collect();
        assert!(enc_names.contains(&"SPD"));
        assert!(enc_names.contains(&"HDG"));
        assert!(enc_names.contains(&"ALT"));
        assert!(enc_names.contains(&"VS/FPA"));
    }

    #[test]
    fn test_fcu_displays() {
        let p = fcu_panel_profile();
        assert_eq!(p.displays.len(), 5);
        let disp_names: Vec<_> = p.displays.iter().map(|d| d.name).collect();
        assert!(disp_names.contains(&"SPD"));
        assert!(disp_names.contains(&"ALT"));
        assert!(disp_names.contains(&"VS/FPA"));
    }

    #[test]
    fn test_fcu_alt_display_width() {
        let p = fcu_panel_profile();
        let alt = p.displays.iter().find(|d| d.name == "ALT").unwrap();
        assert_eq!(alt.width, 5); // altitude up to 99999
    }

    #[test]
    fn test_fcu_has_annunciators() {
        let p = fcu_panel_profile();
        assert!(
            p.displays
                .iter()
                .any(|d| d.display_type == "led-annunciator")
        );
    }

    #[test]
    fn test_fcu_buttons() {
        let p = fcu_panel_profile();
        assert_eq!(p.button_count, 16);
    }

    #[test]
    fn test_fcu_backlight() {
        let p = fcu_panel_profile();
        assert_eq!(p.backlight_led_count, 16);
    }

    // ── EFIS ──────────────────────────────────────────────────────────────

    #[test]
    fn test_efis_encoders() {
        let p = efis_panel_profile();
        assert_eq!(p.encoders.len(), 3);
        let enc_names: Vec<_> = p.encoders.iter().map(|e| e.name).collect();
        assert!(enc_names.contains(&"BARO"));
        assert!(enc_names.contains(&"ND_RANGE"));
        assert!(enc_names.contains(&"ND_MODE"));
    }

    #[test]
    fn test_efis_displays() {
        let p = efis_panel_profile();
        assert_eq!(p.displays.len(), 2);
    }

    #[test]
    fn test_efis_baro_display() {
        let p = efis_panel_profile();
        let baro = p.displays.iter().find(|d| d.name == "BARO").unwrap();
        assert_eq!(baro.display_type, "7seg");
        assert_eq!(baro.width, 4);
    }

    #[test]
    fn test_efis_buttons() {
        let p = efis_panel_profile();
        assert_eq!(p.button_count, 14);
    }

    #[test]
    fn test_efis_backlight() {
        let p = efis_panel_profile();
        assert_eq!(p.backlight_led_count, 14);
    }

    // ── PID lookup ────────────────────────────────────────────────────────

    #[test]
    fn test_profile_by_pid_known() {
        assert!(profile_by_pid(0xBE62).is_some());
        assert!(profile_by_pid(0xBE63).is_some());
        assert!(profile_by_pid(0xBEA8).is_some());
        assert!(profile_by_pid(0xBD64).is_some());
        assert!(profile_by_pid(0xBD70).is_some());
        assert!(profile_by_pid(0xBE04).is_some());
        assert!(profile_by_pid(0xBE05).is_some());
    }

    #[test]
    fn test_profile_by_pid_unknown() {
        assert!(profile_by_pid(0x0000).is_none());
        assert!(profile_by_pid(0xFFFF).is_none());
    }

    #[test]
    fn test_profile_by_pid_returns_correct_name() {
        let p = profile_by_pid(0xBE62).unwrap();
        assert!(p.name.contains("Throttle"));
        let p = profile_by_pid(0xBEA8).unwrap();
        assert!(p.name.contains("F-16"));
        let p = profile_by_pid(0xBD64).unwrap();
        assert!(p.name.contains("Super Taurus"));
        let p = profile_by_pid(0xBD70).unwrap();
        assert!(p.name.contains("Super Libra"));
        let p = profile_by_pid(0xBE05).unwrap();
        assert!(p.name.contains("Combat Ready"));
    }

    // ── Cross-profile consistency ─────────────────────────────────────────

    #[test]
    fn test_panel_profiles_have_no_axes() {
        for p in [
            take_off_panel_profile(),
            combat_ready_panel_profile(),
            fcu_panel_profile(),
            efis_panel_profile(),
        ] {
            assert!(p.axes.is_empty(), "{} should have no axes", p.name);
        }
    }

    #[test]
    fn test_stick_profiles_have_axes() {
        for p in [
            orion2_base_profile(),
            f16ex_grip_profile(),
            f18_grip_profile(),
            a10_grip_profile(),
            super_libra_profile(),
        ] {
            assert!(!p.axes.is_empty(), "{} should have axes", p.name);
        }
    }

    #[test]
    fn test_no_duplicate_pids_in_all_profiles() {
        let profiles = all_profiles();
        let pids: Vec<u16> = profiles.iter().map(|p| p.pid).collect();
        let mut unique = pids.clone();
        unique.sort();
        unique.dedup();
        // F-18 grip and Orion 2 Base share PID 0xBE63 (same physical base)
        // so we allow at most one duplicate
        assert!(
            pids.len() - unique.len() <= 1,
            "too many duplicate PIDs: {pids:?}"
        );
    }

    #[test]
    fn test_hall_effect_stick_axes_have_small_deadzones() {
        for p in [
            orion2_base_profile(),
            f16ex_grip_profile(),
            super_libra_profile(),
        ] {
            for ax in &p.axes {
                assert!(
                    ax.deadzone < 0.02,
                    "{} axis {} has deadzone {} (expected < 0.02)",
                    p.name,
                    ax.name,
                    ax.deadzone
                );
            }
        }
    }
}
