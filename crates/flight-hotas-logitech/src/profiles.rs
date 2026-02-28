// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Default device profiles for Logitech/Saitek flight peripherals.
//!
//! Each profile describes the axis layout, button count, rotary encoders, and
//! (where applicable) MFD pages and RGB presets for a specific device. Profiles
//! are pure data — they do not perform I/O.
//!
//! # Profiles
//!
//! - [`x52_profile`] — Saitek X52 / X52 Pro HOTAS
//! - [`x56_profile`] — Saitek X56 Rhino HOTAS
//! - [`flight_yoke_profile`] — Logitech G Flight Yoke + Throttle Quadrant
//! - [`rudder_pedals_profile`] — Logitech Flight Rudder Pedals

use serde::Serialize;

use crate::protocol::{RgbColor, X52LedColor, X52LedId, X52Mode};

// ── Shared types ───────────────────────────────────────────────────────────────

/// Axis polarity (bipolar vs. unipolar).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum AxisKind {
    /// Bipolar axis: −1.0..=1.0 (e.g., joystick X/Y, twist).
    Bipolar,
    /// Unipolar axis: 0.0..=1.0 (e.g., throttle, slider).
    Unipolar,
}

/// Axis descriptor within a device profile.
#[derive(Debug, Clone, Serialize)]
pub struct AxisDescriptor {
    /// Human-readable axis name.
    pub name: &'static str,
    /// HID usage code (if known).
    pub hid_usage: u16,
    /// Raw axis resolution in bits.
    pub resolution_bits: u8,
    /// Axis polarity.
    pub kind: AxisKind,
    /// Whether this axis has a center detent / spring return.
    pub center_detent: bool,
}

/// Rotary encoder descriptor.
#[derive(Debug, Clone, Serialize)]
pub struct RotaryDescriptor {
    /// Human-readable encoder name.
    pub name: &'static str,
    /// Raw axis resolution in bits.
    pub resolution_bits: u8,
    /// Whether the encoder is continuous (infinite rotation) or bounded.
    pub continuous: bool,
}

/// MFD page definition for X52 devices.
#[derive(Debug, Clone, Serialize)]
pub struct MfdPage {
    /// Page name / title.
    pub name: &'static str,
    /// Default content for the 3 display lines.
    pub lines: [&'static str; 3],
}

/// RGB preset for X56 lighting.
#[derive(Debug, Clone, Serialize)]
pub struct RgbPreset {
    /// Preset name.
    pub name: &'static str,
    /// Color for stick base.
    pub stick_base: RgbColor,
    /// Color for stick grip.
    pub stick_grip: RgbColor,
    /// Color for throttle base.
    pub throttle_base: RgbColor,
    /// Color for throttle grip.
    pub throttle_grip: RgbColor,
}

/// LED default state for X52 devices.
#[derive(Debug, Clone, Serialize)]
pub struct LedDefault {
    /// LED identifier.
    pub led: X52LedId,
    /// Default color.
    pub color: X52LedColor,
}

/// Mode-specific button label table for X52 mode selector.
#[derive(Debug, Clone, Serialize)]
pub struct ModeButtonLabels {
    /// Mode this label set applies to.
    pub mode: X52Mode,
    /// Per-button labels (1-indexed; element 0 is unused).
    pub labels: Vec<&'static str>,
}

/// Complete device profile.
#[derive(Debug, Clone, Serialize)]
pub struct DeviceProfile {
    /// Device name.
    pub name: &'static str,
    /// Axis descriptors.
    pub axes: Vec<AxisDescriptor>,
    /// Total button count.
    pub button_count: u8,
    /// Hat switch count.
    pub hat_count: u8,
    /// Rotary encoder descriptors.
    pub rotaries: Vec<RotaryDescriptor>,
    /// MFD pages (X52 only; empty for devices without MFD).
    pub mfd_pages: Vec<MfdPage>,
    /// RGB presets (X56 only; empty for non-RGB devices).
    pub rgb_presets: Vec<RgbPreset>,
    /// Default LED states (X52 only).
    pub led_defaults: Vec<LedDefault>,
    /// Per-mode button labels (X52 only; empty for non-mode devices).
    pub mode_button_labels: Vec<ModeButtonLabels>,
    /// Whether the device supports a mode selector.
    pub has_mode_selector: bool,
}

// ── X52 / X52 Pro profile ──────────────────────────────────────────────────────

/// Default profile for the Saitek X52 / X52 Pro HOTAS.
///
/// # Axes
/// - Stick X (roll), Y (pitch), twist (yaw) — 11-bit bipolar
/// - Throttle — 8-bit unipolar
/// - Rotary encoders E/F on throttle — 8-bit
/// - Mouse mini-stick X/Y on throttle — 8-bit bipolar
/// - Slider on throttle — 8-bit unipolar
///
/// # Buttons
/// 34 buttons across 3 modes (some are physical toggles).
///
/// # MFD
/// 3-line, 16-character per line LCD on throttle base.
pub fn x52_profile() -> DeviceProfile {
    DeviceProfile {
        name: "Saitek X52 Pro HOTAS",
        axes: vec![
            AxisDescriptor {
                name: "Stick X",
                hid_usage: 0x30,
                resolution_bits: 11,
                kind: AxisKind::Bipolar,
                center_detent: true,
            },
            AxisDescriptor {
                name: "Stick Y",
                hid_usage: 0x31,
                resolution_bits: 11,
                kind: AxisKind::Bipolar,
                center_detent: true,
            },
            AxisDescriptor {
                name: "Stick Twist",
                hid_usage: 0x35,
                resolution_bits: 10,
                kind: AxisKind::Bipolar,
                center_detent: true,
            },
            AxisDescriptor {
                name: "Throttle",
                hid_usage: 0x32,
                resolution_bits: 8,
                kind: AxisKind::Unipolar,
                center_detent: false,
            },
            AxisDescriptor {
                name: "Rotary E",
                hid_usage: 0x33,
                resolution_bits: 8,
                kind: AxisKind::Unipolar,
                center_detent: false,
            },
            AxisDescriptor {
                name: "Rotary F",
                hid_usage: 0x34,
                resolution_bits: 8,
                kind: AxisKind::Unipolar,
                center_detent: false,
            },
            AxisDescriptor {
                name: "Mouse Mini-Stick X",
                hid_usage: 0x30,
                resolution_bits: 8,
                kind: AxisKind::Bipolar,
                center_detent: true,
            },
            AxisDescriptor {
                name: "Mouse Mini-Stick Y",
                hid_usage: 0x31,
                resolution_bits: 8,
                kind: AxisKind::Bipolar,
                center_detent: true,
            },
            AxisDescriptor {
                name: "Slider",
                hid_usage: 0x36,
                resolution_bits: 8,
                kind: AxisKind::Unipolar,
                center_detent: false,
            },
        ],
        button_count: 34,
        hat_count: 2,
        rotaries: vec![
            RotaryDescriptor {
                name: "Rotary E",
                resolution_bits: 8,
                continuous: true,
            },
            RotaryDescriptor {
                name: "Rotary F",
                resolution_bits: 8,
                continuous: true,
            },
        ],
        mfd_pages: vec![
            MfdPage {
                name: "Navigation",
                lines: ["  NAVIGATION    ", "HDG: ---  ALT:--", "SPD: ---  VS:---"],
            },
            MfdPage {
                name: "Engine",
                lines: ["    ENGINE      ", "RPM: ----  EGT:-", "FF:  ----  OIL:-"],
            },
            MfdPage {
                name: "Radio",
                lines: ["     RADIO      ", "COM1: 118.000   ", "NAV1: 110.00    "],
            },
        ],
        led_defaults: vec![
            LedDefault {
                led: X52LedId::Fire,
                color: X52LedColor::Green,
            },
            LedDefault {
                led: X52LedId::ButtonA,
                color: X52LedColor::Green,
            },
            LedDefault {
                led: X52LedId::ButtonB,
                color: X52LedColor::Green,
            },
            LedDefault {
                led: X52LedId::Toggle1,
                color: X52LedColor::Green,
            },
            LedDefault {
                led: X52LedId::Toggle2,
                color: X52LedColor::Green,
            },
            LedDefault {
                led: X52LedId::Toggle3,
                color: X52LedColor::Green,
            },
            LedDefault {
                led: X52LedId::Throttle,
                color: X52LedColor::Green,
            },
        ],
        rgb_presets: vec![],
        mode_button_labels: vec![
            ModeButtonLabels {
                mode: X52Mode::Mode1,
                labels: vec![
                    "",           // index 0 unused
                    "Trigger",    // 1
                    "Fire",       // 2
                    "Fire A",     // 3
                    "Fire B",     // 4
                    "Fire C",     // 5
                    "Pinky",      // 6
                    "Fire D",     // 7
                    "Fire E",     // 8
                    "Toggle 1",   // 9
                    "Toggle 2",   // 10
                    "Toggle 3",   // 11
                    "Toggle 4",   // 12
                    "Toggle 5",   // 13
                    "Toggle 6",   // 14
                    "POV2 Up",    // 15
                    "POV2 Right", // 16
                    "POV2 Down",  // 17
                    "POV2 Left",  // 18
                    "Clutch",     // 19
                ],
            },
            ModeButtonLabels {
                mode: X52Mode::Mode2,
                labels: vec![
                    "",
                    "M2 Trigger",
                    "M2 Fire",
                    "M2 Fire A",
                    "M2 Fire B",
                    "M2 Fire C",
                    "M2 Pinky",
                    "M2 Fire D",
                    "M2 Fire E",
                    "M2 Toggle 1",
                    "M2 Toggle 2",
                    "M2 Toggle 3",
                    "M2 Toggle 4",
                    "M2 Toggle 5",
                    "M2 Toggle 6",
                    "M2 POV2 Up",
                    "M2 POV2 Right",
                    "M2 POV2 Down",
                    "M2 POV2 Left",
                    "M2 Clutch",
                ],
            },
            ModeButtonLabels {
                mode: X52Mode::Mode3,
                labels: vec![
                    "",
                    "M3 Trigger",
                    "M3 Fire",
                    "M3 Fire A",
                    "M3 Fire B",
                    "M3 Fire C",
                    "M3 Pinky",
                    "M3 Fire D",
                    "M3 Fire E",
                    "M3 Toggle 1",
                    "M3 Toggle 2",
                    "M3 Toggle 3",
                    "M3 Toggle 4",
                    "M3 Toggle 5",
                    "M3 Toggle 6",
                    "M3 POV2 Up",
                    "M3 POV2 Right",
                    "M3 POV2 Down",
                    "M3 POV2 Left",
                    "M3 Clutch",
                ],
            },
        ],
        has_mode_selector: true,
    }
}

// ── X56 Rhino profile ──────────────────────────────────────────────────────────

/// Default profile for the Saitek X56 Rhino HOTAS.
///
/// # Stick
/// - X, Y, Twist — 16-bit bipolar
/// - 2 analog mini-sticks (4 axes) — 8-bit bipolar
///
/// # Throttle
/// - Dual throttle axes — 16-bit unipolar
/// - 6 rotary knobs — 8-bit
/// - Many buttons, 4 hat switches
/// - RGB LED zones (4 zones)
pub fn x56_profile() -> DeviceProfile {
    DeviceProfile {
        name: "Saitek X56 Rhino HOTAS",
        axes: vec![
            AxisDescriptor {
                name: "Stick X",
                hid_usage: 0x30,
                resolution_bits: 16,
                kind: AxisKind::Bipolar,
                center_detent: true,
            },
            AxisDescriptor {
                name: "Stick Y",
                hid_usage: 0x31,
                resolution_bits: 16,
                kind: AxisKind::Bipolar,
                center_detent: true,
            },
            AxisDescriptor {
                name: "Stick Twist",
                hid_usage: 0x35,
                resolution_bits: 16,
                kind: AxisKind::Bipolar,
                center_detent: true,
            },
            AxisDescriptor {
                name: "Mini-Stick 1 X",
                hid_usage: 0x30,
                resolution_bits: 8,
                kind: AxisKind::Bipolar,
                center_detent: true,
            },
            AxisDescriptor {
                name: "Mini-Stick 1 Y",
                hid_usage: 0x31,
                resolution_bits: 8,
                kind: AxisKind::Bipolar,
                center_detent: true,
            },
            AxisDescriptor {
                name: "Mini-Stick 2 X",
                hid_usage: 0x30,
                resolution_bits: 8,
                kind: AxisKind::Bipolar,
                center_detent: true,
            },
            AxisDescriptor {
                name: "Mini-Stick 2 Y",
                hid_usage: 0x31,
                resolution_bits: 8,
                kind: AxisKind::Bipolar,
                center_detent: true,
            },
            AxisDescriptor {
                name: "Left Throttle",
                hid_usage: 0x32,
                resolution_bits: 16,
                kind: AxisKind::Unipolar,
                center_detent: false,
            },
            AxisDescriptor {
                name: "Right Throttle",
                hid_usage: 0x33,
                resolution_bits: 16,
                kind: AxisKind::Unipolar,
                center_detent: false,
            },
        ],
        button_count: 32,
        hat_count: 4,
        rotaries: vec![
            RotaryDescriptor {
                name: "Rotary 1",
                resolution_bits: 8,
                continuous: true,
            },
            RotaryDescriptor {
                name: "Rotary 2",
                resolution_bits: 8,
                continuous: true,
            },
            RotaryDescriptor {
                name: "Rotary 3",
                resolution_bits: 8,
                continuous: true,
            },
            RotaryDescriptor {
                name: "Rotary 4",
                resolution_bits: 8,
                continuous: true,
            },
            RotaryDescriptor {
                name: "RTY 3 (Throttle)",
                resolution_bits: 8,
                continuous: true,
            },
            RotaryDescriptor {
                name: "RTY 4 (Throttle)",
                resolution_bits: 8,
                continuous: true,
            },
        ],
        mfd_pages: vec![],
        led_defaults: vec![],
        rgb_presets: vec![
            RgbPreset {
                name: "Default Blue",
                stick_base: RgbColor::BLUE,
                stick_grip: RgbColor::BLUE,
                throttle_base: RgbColor::BLUE,
                throttle_grip: RgbColor::BLUE,
            },
            RgbPreset {
                name: "Combat Red",
                stick_base: RgbColor::RED,
                stick_grip: RgbColor::RED,
                throttle_base: RgbColor::RED,
                throttle_grip: RgbColor::RED,
            },
            RgbPreset {
                name: "Night Green",
                stick_base: RgbColor::new(0, 64, 0),
                stick_grip: RgbColor::new(0, 64, 0),
                throttle_base: RgbColor::new(0, 64, 0),
                throttle_grip: RgbColor::new(0, 64, 0),
            },
            RgbPreset {
                name: "Amber Warm",
                stick_base: RgbColor::AMBER,
                stick_grip: RgbColor::AMBER,
                throttle_base: RgbColor::AMBER,
                throttle_grip: RgbColor::AMBER,
            },
            RgbPreset {
                name: "Off",
                stick_base: RgbColor::OFF,
                stick_grip: RgbColor::OFF,
                throttle_base: RgbColor::OFF,
                throttle_grip: RgbColor::OFF,
            },
        ],
        mode_button_labels: vec![],
        has_mode_selector: false,
    }
}

// ── Logitech G Flight Yoke + Throttle profile ─────────────────────────────────

/// Default profile for the Logitech G Flight Yoke System with Throttle Quadrant.
///
/// # Yoke
/// - Pitch (Y) and Roll (X) — 12-bit bipolar
/// - Prop pitch (Rz), Mixture (Slider), Carb heat (Slider2) — 8-bit unipolar
/// - 12 buttons, 1 hat switch
///
/// # Throttle Quadrant (separate USB device)
/// - 3 lever axes — 12-bit unipolar
/// - 6 buttons
pub fn flight_yoke_profile() -> DeviceProfile {
    DeviceProfile {
        name: "Logitech G Flight Yoke + Throttle Quadrant",
        axes: vec![
            // Yoke axes
            AxisDescriptor {
                name: "Roll (Yoke X)",
                hid_usage: 0x30,
                resolution_bits: 12,
                kind: AxisKind::Bipolar,
                center_detent: true,
            },
            AxisDescriptor {
                name: "Pitch (Yoke Y)",
                hid_usage: 0x31,
                resolution_bits: 12,
                kind: AxisKind::Bipolar,
                center_detent: true,
            },
            AxisDescriptor {
                name: "Prop Pitch (Rz)",
                hid_usage: 0x35,
                resolution_bits: 8,
                kind: AxisKind::Unipolar,
                center_detent: false,
            },
            AxisDescriptor {
                name: "Mixture (Slider)",
                hid_usage: 0x36,
                resolution_bits: 8,
                kind: AxisKind::Unipolar,
                center_detent: false,
            },
            AxisDescriptor {
                name: "Carb Heat (Slider2)",
                hid_usage: 0x37,
                resolution_bits: 8,
                kind: AxisKind::Unipolar,
                center_detent: false,
            },
            // Throttle Quadrant axes
            AxisDescriptor {
                name: "Throttle Left",
                hid_usage: 0x32,
                resolution_bits: 12,
                kind: AxisKind::Unipolar,
                center_detent: false,
            },
            AxisDescriptor {
                name: "Throttle Center",
                hid_usage: 0x35,
                resolution_bits: 12,
                kind: AxisKind::Unipolar,
                center_detent: false,
            },
            AxisDescriptor {
                name: "Throttle Right",
                hid_usage: 0x36,
                resolution_bits: 12,
                kind: AxisKind::Unipolar,
                center_detent: false,
            },
        ],
        button_count: 18, // 12 yoke + 6 throttle quadrant
        hat_count: 1,
        rotaries: vec![],
        mfd_pages: vec![],
        led_defaults: vec![],
        rgb_presets: vec![],
        mode_button_labels: vec![],
        has_mode_selector: false,
    }
}

// ── Logitech Flight Rudder Pedals profile ──────────────────────────────────────

/// Default profile for the Logitech Flight Rudder Pedals.
///
/// # Axes
/// - Left toe brake — unipolar
/// - Right toe brake — unipolar
/// - Rudder (yaw) — bipolar, center-sprung
pub fn rudder_pedals_profile() -> DeviceProfile {
    DeviceProfile {
        name: "Logitech Flight Rudder Pedals",
        axes: vec![
            AxisDescriptor {
                name: "Left Toe Brake",
                hid_usage: 0x32,
                resolution_bits: 8,
                kind: AxisKind::Unipolar,
                center_detent: false,
            },
            AxisDescriptor {
                name: "Right Toe Brake",
                hid_usage: 0x35,
                resolution_bits: 8,
                kind: AxisKind::Unipolar,
                center_detent: false,
            },
            AxisDescriptor {
                name: "Rudder",
                hid_usage: 0x36,
                resolution_bits: 10,
                kind: AxisKind::Bipolar,
                center_detent: true,
            },
        ],
        button_count: 0,
        hat_count: 0,
        rotaries: vec![],
        mfd_pages: vec![],
        led_defaults: vec![],
        rgb_presets: vec![],
        mode_button_labels: vec![],
        has_mode_selector: false,
    }
}

// ── Profile query helpers ──────────────────────────────────────────────────────

impl DeviceProfile {
    /// Return axes that have spring return / center detent.
    pub fn centered_axes(&self) -> Vec<&AxisDescriptor> {
        self.axes.iter().filter(|a| a.center_detent).collect()
    }

    /// Return all bipolar axes.
    pub fn bipolar_axes(&self) -> Vec<&AxisDescriptor> {
        self.axes
            .iter()
            .filter(|a| a.kind == AxisKind::Bipolar)
            .collect()
    }

    /// Return all unipolar axes.
    pub fn unipolar_axes(&self) -> Vec<&AxisDescriptor> {
        self.axes
            .iter()
            .filter(|a| a.kind == AxisKind::Unipolar)
            .collect()
    }

    /// Return the maximum raw axis value for a given resolution in bits.
    pub fn axis_max_raw(resolution_bits: u8) -> u32 {
        (1u32 << resolution_bits) - 1
    }

    /// Normalize a raw axis value given its descriptor.
    ///
    /// Bipolar: maps 0..max to −1.0..=1.0.
    /// Unipolar: maps 0..max to 0.0..=1.0.
    pub fn normalize_axis(raw: u32, desc: &AxisDescriptor) -> f32 {
        let max = Self::axis_max_raw(desc.resolution_bits) as f32;
        match desc.kind {
            AxisKind::Bipolar => ((raw as f32 / (max / 2.0)) - 1.0).clamp(-1.0, 1.0),
            AxisKind::Unipolar => (raw as f32 / max).clamp(0.0, 1.0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── X52 profile tests ──────────────────────────────────────────────────

    #[test]
    fn x52_profile_basic_structure() {
        let p = x52_profile();
        assert_eq!(p.name, "Saitek X52 Pro HOTAS");
        assert!(p.has_mode_selector);
        assert_eq!(p.hat_count, 2);
    }

    #[test]
    fn x52_profile_axis_count() {
        let p = x52_profile();
        assert_eq!(p.axes.len(), 9);
    }

    #[test]
    fn x52_profile_has_throttle_axis() {
        let p = x52_profile();
        let throttle = p.axes.iter().find(|a| a.name == "Throttle");
        assert!(throttle.is_some());
        let t = throttle.unwrap();
        assert_eq!(t.kind, AxisKind::Unipolar);
        assert_eq!(t.resolution_bits, 8);
        assert!(!t.center_detent);
    }

    #[test]
    fn x52_profile_stick_axes_are_bipolar() {
        let p = x52_profile();
        for name in &["Stick X", "Stick Y", "Stick Twist"] {
            let axis = p.axes.iter().find(|a| a.name == *name).unwrap();
            assert_eq!(axis.kind, AxisKind::Bipolar, "{} should be bipolar", name);
            assert!(axis.center_detent, "{} should have center detent", name);
        }
    }

    #[test]
    fn x52_profile_button_count() {
        let p = x52_profile();
        assert_eq!(p.button_count, 34);
    }

    #[test]
    fn x52_profile_has_mfd_pages() {
        let p = x52_profile();
        assert_eq!(p.mfd_pages.len(), 3);
        assert!(p.mfd_pages.iter().any(|pg| pg.name == "Navigation"));
        assert!(p.mfd_pages.iter().any(|pg| pg.name == "Engine"));
        assert!(p.mfd_pages.iter().any(|pg| pg.name == "Radio"));
    }

    #[test]
    fn x52_mfd_page_line_lengths() {
        let p = x52_profile();
        for page in &p.mfd_pages {
            for line in &page.lines {
                assert!(
                    line.len() <= 16,
                    "MFD line '{}' in page '{}' exceeds 16 chars ({})",
                    line,
                    page.name,
                    line.len()
                );
            }
        }
    }

    #[test]
    fn x52_profile_has_rotary_encoders() {
        let p = x52_profile();
        assert_eq!(p.rotaries.len(), 2);
        assert_eq!(p.rotaries[0].name, "Rotary E");
        assert_eq!(p.rotaries[1].name, "Rotary F");
    }

    #[test]
    fn x52_profile_led_defaults() {
        let p = x52_profile();
        assert!(!p.led_defaults.is_empty());
        // All defaults should be Green
        for led in &p.led_defaults {
            assert_eq!(led.color, X52LedColor::Green);
        }
    }

    #[test]
    fn x52_profile_mode_labels_per_mode() {
        let p = x52_profile();
        assert_eq!(p.mode_button_labels.len(), 3);
        assert_eq!(p.mode_button_labels[0].mode, X52Mode::Mode1);
        assert_eq!(p.mode_button_labels[1].mode, X52Mode::Mode2);
        assert_eq!(p.mode_button_labels[2].mode, X52Mode::Mode3);
    }

    #[test]
    fn x52_profile_mode1_labels_start_with_standard_names() {
        let p = x52_profile();
        let m1 = &p.mode_button_labels[0];
        assert_eq!(m1.labels[1], "Trigger");
        assert_eq!(m1.labels[2], "Fire");
    }

    #[test]
    fn x52_profile_no_rgb_presets() {
        let p = x52_profile();
        assert!(p.rgb_presets.is_empty(), "X52 has no RGB");
    }

    // ── X56 profile tests ──────────────────────────────────────────────────

    #[test]
    fn x56_profile_basic_structure() {
        let p = x56_profile();
        assert_eq!(p.name, "Saitek X56 Rhino HOTAS");
        assert!(!p.has_mode_selector);
        assert_eq!(p.hat_count, 4);
    }

    #[test]
    fn x56_profile_axis_count() {
        let p = x56_profile();
        assert_eq!(p.axes.len(), 9);
    }

    #[test]
    fn x56_profile_dual_throttle() {
        let p = x56_profile();
        let left = p.axes.iter().find(|a| a.name == "Left Throttle");
        let right = p.axes.iter().find(|a| a.name == "Right Throttle");
        assert!(left.is_some());
        assert!(right.is_some());
        assert_eq!(left.unwrap().kind, AxisKind::Unipolar);
        assert_eq!(right.unwrap().kind, AxisKind::Unipolar);
    }

    #[test]
    fn x56_profile_has_6_rotaries() {
        let p = x56_profile();
        assert_eq!(p.rotaries.len(), 6);
        for rot in &p.rotaries {
            assert!(rot.continuous, "{} should be continuous", rot.name);
        }
    }

    #[test]
    fn x56_profile_has_rgb_presets() {
        let p = x56_profile();
        assert!(p.rgb_presets.len() >= 3);
        assert!(p.rgb_presets.iter().any(|pr| pr.name == "Default Blue"));
        assert!(p.rgb_presets.iter().any(|pr| pr.name == "Combat Red"));
        assert!(p.rgb_presets.iter().any(|pr| pr.name == "Off"));
    }

    #[test]
    fn x56_rgb_preset_off_is_all_zeros() {
        let p = x56_profile();
        let off = p.rgb_presets.iter().find(|pr| pr.name == "Off").unwrap();
        assert_eq!(off.stick_base, RgbColor::OFF);
        assert_eq!(off.stick_grip, RgbColor::OFF);
        assert_eq!(off.throttle_base, RgbColor::OFF);
        assert_eq!(off.throttle_grip, RgbColor::OFF);
    }

    #[test]
    fn x56_profile_has_mini_sticks() {
        let p = x56_profile();
        let mini_axes: Vec<_> = p
            .axes
            .iter()
            .filter(|a| a.name.contains("Mini-Stick"))
            .collect();
        assert_eq!(mini_axes.len(), 4, "X56 should have 2 mini-sticks (4 axes)");
    }

    #[test]
    fn x56_profile_no_mfd() {
        let p = x56_profile();
        assert!(p.mfd_pages.is_empty(), "X56 has no MFD");
    }

    #[test]
    fn x56_profile_no_mode_selector() {
        let p = x56_profile();
        assert!(p.mode_button_labels.is_empty());
    }

    // ── Flight Yoke profile tests ──────────────────────────────────────────

    #[test]
    fn flight_yoke_profile_basic_structure() {
        let p = flight_yoke_profile();
        assert!(p.name.contains("Yoke"));
        assert!(!p.has_mode_selector);
        assert_eq!(p.hat_count, 1);
    }

    #[test]
    fn flight_yoke_profile_axis_count() {
        let p = flight_yoke_profile();
        // 5 yoke axes + 3 throttle quadrant axes = 8
        assert_eq!(p.axes.len(), 8);
    }

    #[test]
    fn flight_yoke_profile_pitch_roll_bipolar() {
        let p = flight_yoke_profile();
        let roll = p.axes.iter().find(|a| a.name.contains("Roll")).unwrap();
        let pitch = p.axes.iter().find(|a| a.name.contains("Pitch")).unwrap();
        assert_eq!(roll.kind, AxisKind::Bipolar);
        assert_eq!(pitch.kind, AxisKind::Bipolar);
        assert_eq!(roll.resolution_bits, 12);
        assert_eq!(pitch.resolution_bits, 12);
    }

    #[test]
    fn flight_yoke_profile_throttle_axes_unipolar() {
        let p = flight_yoke_profile();
        let throttle_axes: Vec<_> = p
            .axes
            .iter()
            .filter(|a| a.name.starts_with("Throttle"))
            .collect();
        assert_eq!(throttle_axes.len(), 3, "quadrant has 3 levers");
        for axis in &throttle_axes {
            assert_eq!(axis.kind, AxisKind::Unipolar);
            assert_eq!(axis.resolution_bits, 12);
        }
    }

    #[test]
    fn flight_yoke_profile_button_count() {
        let p = flight_yoke_profile();
        assert_eq!(p.button_count, 18); // 12 yoke + 6 quadrant
    }

    #[test]
    fn flight_yoke_profile_no_extras() {
        let p = flight_yoke_profile();
        assert!(p.mfd_pages.is_empty());
        assert!(p.rgb_presets.is_empty());
        assert!(p.led_defaults.is_empty());
        assert!(p.rotaries.is_empty());
    }

    // ── Rudder pedals profile tests ────────────────────────────────────────

    #[test]
    fn rudder_pedals_basic_structure() {
        let p = rudder_pedals_profile();
        assert!(p.name.contains("Rudder"));
        assert_eq!(p.axes.len(), 3);
        assert_eq!(p.button_count, 0);
        assert_eq!(p.hat_count, 0);
    }

    #[test]
    fn rudder_pedals_axis_types() {
        let p = rudder_pedals_profile();
        let brakes: Vec<_> = p.axes.iter().filter(|a| a.name.contains("Brake")).collect();
        assert_eq!(brakes.len(), 2);
        for brake in &brakes {
            assert_eq!(brake.kind, AxisKind::Unipolar);
        }
        let rudder = p.axes.iter().find(|a| a.name == "Rudder").unwrap();
        assert_eq!(rudder.kind, AxisKind::Bipolar);
        assert!(rudder.center_detent);
    }

    // ── Profile helper tests ───────────────────────────────────────────────

    #[test]
    fn axis_max_raw_8bit() {
        assert_eq!(DeviceProfile::axis_max_raw(8), 255);
    }

    #[test]
    fn axis_max_raw_10bit() {
        assert_eq!(DeviceProfile::axis_max_raw(10), 1023);
    }

    #[test]
    fn axis_max_raw_12bit() {
        assert_eq!(DeviceProfile::axis_max_raw(12), 4095);
    }

    #[test]
    fn axis_max_raw_16bit() {
        assert_eq!(DeviceProfile::axis_max_raw(16), 65535);
    }

    #[test]
    fn normalize_axis_bipolar_center() {
        let desc = AxisDescriptor {
            name: "test",
            hid_usage: 0,
            resolution_bits: 10,
            kind: AxisKind::Bipolar,
            center_detent: true,
        };
        let mid = 512; // ~center for 10-bit
        let val = DeviceProfile::normalize_axis(mid, &desc);
        assert!(
            val.abs() < 0.01,
            "bipolar center should be ~0.0, got {}",
            val
        );
    }

    #[test]
    fn normalize_axis_bipolar_min() {
        let desc = AxisDescriptor {
            name: "test",
            hid_usage: 0,
            resolution_bits: 10,
            kind: AxisKind::Bipolar,
            center_detent: true,
        };
        let val = DeviceProfile::normalize_axis(0, &desc);
        assert!(val < -0.99, "bipolar min should be ~-1.0, got {}", val);
    }

    #[test]
    fn normalize_axis_bipolar_max() {
        let desc = AxisDescriptor {
            name: "test",
            hid_usage: 0,
            resolution_bits: 10,
            kind: AxisKind::Bipolar,
            center_detent: true,
        };
        let val = DeviceProfile::normalize_axis(1023, &desc);
        assert!(val > 0.99, "bipolar max should be ~1.0, got {}", val);
    }

    #[test]
    fn normalize_axis_unipolar_min() {
        let desc = AxisDescriptor {
            name: "test",
            hid_usage: 0,
            resolution_bits: 8,
            kind: AxisKind::Unipolar,
            center_detent: false,
        };
        let val = DeviceProfile::normalize_axis(0, &desc);
        assert!(val < 0.001, "unipolar min should be ~0.0, got {}", val);
    }

    #[test]
    fn normalize_axis_unipolar_max() {
        let desc = AxisDescriptor {
            name: "test",
            hid_usage: 0,
            resolution_bits: 8,
            kind: AxisKind::Unipolar,
            center_detent: false,
        };
        let val = DeviceProfile::normalize_axis(255, &desc);
        assert!(val > 0.999, "unipolar max should be ~1.0, got {}", val);
    }

    #[test]
    fn normalize_axis_unipolar_mid() {
        let desc = AxisDescriptor {
            name: "test",
            hid_usage: 0,
            resolution_bits: 8,
            kind: AxisKind::Unipolar,
            center_detent: false,
        };
        let val = DeviceProfile::normalize_axis(128, &desc);
        let expected = 128.0 / 255.0;
        assert!(
            (val - expected).abs() < 0.01,
            "unipolar mid should be ~{}, got {}",
            expected,
            val
        );
    }

    #[test]
    fn centered_axes_filter() {
        let p = x52_profile();
        let centered = p.centered_axes();
        assert!(!centered.is_empty());
        for axis in &centered {
            assert!(axis.center_detent);
        }
    }

    #[test]
    fn bipolar_axes_filter() {
        let p = x56_profile();
        let bipolar = p.bipolar_axes();
        for axis in &bipolar {
            assert_eq!(axis.kind, AxisKind::Bipolar);
        }
    }

    #[test]
    fn unipolar_axes_filter() {
        let p = flight_yoke_profile();
        let unipolar = p.unipolar_axes();
        for axis in &unipolar {
            assert_eq!(axis.kind, AxisKind::Unipolar);
        }
    }

    // ── Profile completeness tests ─────────────────────────────────────────

    #[test]
    fn all_profiles_have_nonempty_names() {
        for p in &[
            x52_profile(),
            x56_profile(),
            flight_yoke_profile(),
            rudder_pedals_profile(),
        ] {
            assert!(!p.name.is_empty(), "profile name must not be empty");
        }
    }

    #[test]
    fn all_profiles_have_at_least_one_axis() {
        for p in &[
            x52_profile(),
            x56_profile(),
            flight_yoke_profile(),
            rudder_pedals_profile(),
        ] {
            assert!(!p.axes.is_empty(), "{} should have axes", p.name);
        }
    }

    #[test]
    fn all_axis_descriptors_have_valid_resolution() {
        for p in &[
            x52_profile(),
            x56_profile(),
            flight_yoke_profile(),
            rudder_pedals_profile(),
        ] {
            for axis in &p.axes {
                assert!(
                    axis.resolution_bits > 0 && axis.resolution_bits <= 16,
                    "{}: {} has invalid resolution {}",
                    p.name,
                    axis.name,
                    axis.resolution_bits
                );
            }
        }
    }

    #[test]
    fn all_axis_descriptors_have_nonempty_names() {
        for p in &[
            x52_profile(),
            x56_profile(),
            flight_yoke_profile(),
            rudder_pedals_profile(),
        ] {
            for axis in &p.axes {
                assert!(!axis.name.is_empty(), "{}: axis with empty name", p.name);
            }
        }
    }

    #[test]
    fn all_rotary_descriptors_have_valid_resolution() {
        for p in &[x52_profile(), x56_profile()] {
            for rot in &p.rotaries {
                assert!(
                    rot.resolution_bits > 0 && rot.resolution_bits <= 16,
                    "{}: {} has invalid resolution {}",
                    p.name,
                    rot.name,
                    rot.resolution_bits
                );
            }
        }
    }

    #[test]
    fn x56_rotary_encoder_positions_all_8bit() {
        let p = x56_profile();
        for rot in &p.rotaries {
            assert_eq!(rot.resolution_bits, 8, "{} should be 8-bit", rot.name);
        }
    }

    #[test]
    fn normalize_axis_per_device_x52_stick_x() {
        let p = x52_profile();
        let stick_x = p.axes.iter().find(|a| a.name == "Stick X").unwrap();
        // 11-bit: max = 2047
        let max_raw = DeviceProfile::axis_max_raw(stick_x.resolution_bits);
        assert_eq!(max_raw, 2047);
        let val = DeviceProfile::normalize_axis(max_raw, stick_x);
        assert!(val > 0.99, "X52 stick X at max should be ~1.0, got {}", val);
    }

    #[test]
    fn normalize_axis_per_device_x56_throttle() {
        let p = x56_profile();
        let throttle = p.axes.iter().find(|a| a.name == "Left Throttle").unwrap();
        // 16-bit: max = 65535
        let max_raw = DeviceProfile::axis_max_raw(throttle.resolution_bits);
        assert_eq!(max_raw, 65535);
        let val = DeviceProfile::normalize_axis(max_raw, throttle);
        assert!(
            val > 0.999,
            "X56 throttle at max should be ~1.0, got {}",
            val
        );
    }

    #[test]
    fn normalize_axis_per_device_yoke_roll() {
        let p = flight_yoke_profile();
        let roll = p.axes.iter().find(|a| a.name.contains("Roll")).unwrap();
        // 12-bit: max = 4095
        let max_raw = DeviceProfile::axis_max_raw(roll.resolution_bits);
        assert_eq!(max_raw, 4095);
        let val = DeviceProfile::normalize_axis(max_raw, roll);
        assert!(val > 0.99, "Yoke roll at max should be ~1.0, got {}", val);
    }
}
