// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Default axis and button mapping profiles for VKB device families.
//!
//! Each profile describes the factory-default control layout for a specific
//! VKB product.  Profiles are returned as static data and can be used as
//! baseline hints for auto-configuration or user-facing mapping UIs.
//!
//! # Supported devices
//!
//! - **Gladiator NXT EVO** (left/right): 6 axes, 34 buttons, 4 hat switches
//! - **Gunfighter + MCG**: 6 axes, 42+ buttons, 4 hat switches, analog mini-sticks
//! - **STECS (Standard/Plus)**: dual throttle axes, buttons for detents, rotary encoders
//! - **T-Rudder Mk.IV**: 3 axes (left toe, right toe, rudder)
//!
//! # Notes
//!
//! VKBDevCfg can remap every axis, button, and hat; these profiles reflect
//! factory defaults only.  Prefer descriptor-first discovery when available.

// ─── Axis mapping entry ───────────────────────────────────────────────────────

/// One axis in a device profile.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AxisMapping {
    /// Axis name (e.g., "roll", "pitch", "throttle_left").
    pub name: &'static str,
    /// Byte offset of the u16 LE value in the HID report payload (after report ID).
    pub report_offset: usize,
    /// Normalisation mode for this axis.
    pub mode: AxisNormMode,
    /// Human-readable description of the physical control.
    pub description: &'static str,
}

/// How a raw 16-bit axis value should be normalised.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AxisNormMode {
    /// Bidirectional: 0x0000 → −1.0, 0x8000 → 0.0, 0xFFFF → 1.0.
    /// Used for joystick roll/pitch/yaw, mini-sticks.
    Signed,
    /// Unidirectional: 0x0000 → 0.0, 0xFFFF → 1.0.
    /// Used for throttle levers, sliders, rotaries, toe brakes.
    Unsigned,
}

// ─── Button mapping entry ─────────────────────────────────────────────────────

/// One button in a device profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ButtonMapping {
    /// 1-based button number as exposed by HID.
    pub number: u8,
    /// Human-readable name of the physical control.
    pub name: &'static str,
    /// Physical control type.
    pub kind: ButtonKind,
}

/// Physical button type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonKind {
    /// Standard pushbutton.
    Pushbutton,
    /// Toggle switch (latching).
    Toggle,
    /// Hat direction (virtual button from hat-switch).
    HatDirection,
    /// Trigger (primary or secondary stage).
    Trigger,
    /// Rotary encoder CW/CCW virtual button.
    Encoder,
}

// ─── Hat mapping entry ────────────────────────────────────────────────────────

/// One hat switch in a device profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HatMapping {
    /// 0-based hat index.
    pub index: u8,
    /// Human-readable name.
    pub name: &'static str,
    /// Hat type.
    pub kind: HatKind,
}

/// Physical hat type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HatKind {
    /// Standard 8-way POV hat.
    Pov8Way,
    /// 4-way trim hat.
    Trim4Way,
    /// Mini-stick used in digital (hat) mode.
    MiniStickDigital,
}

// ─── Device profile ───────────────────────────────────────────────────────────

/// Complete default profile for a VKB device.
#[derive(Debug, Clone, PartialEq)]
pub struct VkbDeviceProfile {
    /// Human-readable device name.
    pub device_name: &'static str,
    /// USB Vendor ID.
    pub vid: u16,
    /// USB Product IDs that match this profile (may be multiple for L/R variants).
    pub pids: &'static [u16],
    /// Default axis mappings.
    pub axes: &'static [AxisMapping],
    /// Default button mappings.
    pub buttons: &'static [ButtonMapping],
    /// Default hat mappings.
    pub hats: &'static [HatMapping],
    /// Notes about the device or profile.
    pub notes: &'static [&'static str],
}

impl VkbDeviceProfile {
    /// Total number of axes in this profile.
    pub fn axis_count(&self) -> usize {
        self.axes.len()
    }

    /// Total number of buttons in this profile.
    pub fn button_count(&self) -> usize {
        self.buttons.len()
    }

    /// Total number of hat switches in this profile.
    pub fn hat_count(&self) -> usize {
        self.hats.len()
    }

    /// Look up an axis by name.
    pub fn axis_by_name(&self, name: &str) -> Option<&AxisMapping> {
        self.axes.iter().find(|a| a.name == name)
    }

    /// Look up a button by number (1-based).
    pub fn button_by_number(&self, number: u8) -> Option<&ButtonMapping> {
        self.buttons.iter().find(|b| b.number == number)
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Gladiator NXT EVO profile
// ═══════════════════════════════════════════════════════════════════════════════

use flight_hid_support::device_support::{
    VKB_GLADIATOR_NXT_EVO_LEFT_PID, VKB_GLADIATOR_NXT_EVO_RIGHT_PID,
    VKB_GUNFIGHTER_MODERN_COMBAT_PRO_PID, VKB_NXT_SEM_THQ_PID, VKB_SPACE_GUNFIGHTER_LEFT_PID,
    VKB_SPACE_GUNFIGHTER_PID, VKB_STECS_LEFT_SPACE_STANDARD_PID,
    VKB_STECS_RIGHT_SPACE_STANDARD_PID, VKB_VENDOR_ID,
};

static GLADIATOR_NXT_EVO_AXES: [AxisMapping; 6] = [
    AxisMapping {
        name: "roll",
        report_offset: 0,
        mode: AxisNormMode::Signed,
        description: "Main stick X axis (roll)",
    },
    AxisMapping {
        name: "pitch",
        report_offset: 2,
        mode: AxisNormMode::Signed,
        description: "Main stick Y axis (pitch)",
    },
    AxisMapping {
        name: "yaw",
        report_offset: 4,
        mode: AxisNormMode::Signed,
        description: "Stick twist Z axis (yaw)",
    },
    AxisMapping {
        name: "mini_x",
        report_offset: 6,
        mode: AxisNormMode::Signed,
        description: "Analog mini-stick X axis",
    },
    AxisMapping {
        name: "mini_y",
        report_offset: 8,
        mode: AxisNormMode::Signed,
        description: "Analog mini-stick Y axis",
    },
    AxisMapping {
        name: "throttle",
        report_offset: 10,
        mode: AxisNormMode::Unsigned,
        description: "Base throttle wheel / slider",
    },
];

static GLADIATOR_NXT_EVO_BUTTONS: [ButtonMapping; 34] = [
    ButtonMapping { number: 1, name: "Trigger Stage 1", kind: ButtonKind::Trigger },
    ButtonMapping { number: 2, name: "Trigger Stage 2", kind: ButtonKind::Trigger },
    ButtonMapping { number: 3, name: "Side Button (A3)", kind: ButtonKind::Pushbutton },
    ButtonMapping { number: 4, name: "Side Button (A4)", kind: ButtonKind::Pushbutton },
    ButtonMapping { number: 5, name: "Pinkie Button", kind: ButtonKind::Pushbutton },
    ButtonMapping { number: 6, name: "Pinkie Lever", kind: ButtonKind::Toggle },
    ButtonMapping { number: 7, name: "HAT 2 Up", kind: ButtonKind::HatDirection },
    ButtonMapping { number: 8, name: "HAT 2 Right", kind: ButtonKind::HatDirection },
    ButtonMapping { number: 9, name: "HAT 2 Down", kind: ButtonKind::HatDirection },
    ButtonMapping { number: 10, name: "HAT 2 Left", kind: ButtonKind::HatDirection },
    ButtonMapping { number: 11, name: "HAT 3 Up", kind: ButtonKind::HatDirection },
    ButtonMapping { number: 12, name: "HAT 3 Right", kind: ButtonKind::HatDirection },
    ButtonMapping { number: 13, name: "HAT 3 Down", kind: ButtonKind::HatDirection },
    ButtonMapping { number: 14, name: "HAT 3 Left", kind: ButtonKind::HatDirection },
    ButtonMapping { number: 15, name: "HAT 4 Up", kind: ButtonKind::HatDirection },
    ButtonMapping { number: 16, name: "HAT 4 Right", kind: ButtonKind::HatDirection },
    ButtonMapping { number: 17, name: "HAT 4 Down", kind: ButtonKind::HatDirection },
    ButtonMapping { number: 18, name: "HAT 4 Left", kind: ButtonKind::HatDirection },
    ButtonMapping { number: 19, name: "Encoder CW", kind: ButtonKind::Encoder },
    ButtonMapping { number: 20, name: "Encoder CCW", kind: ButtonKind::Encoder },
    ButtonMapping { number: 21, name: "Encoder Press", kind: ButtonKind::Pushbutton },
    ButtonMapping { number: 22, name: "Mini-stick Press", kind: ButtonKind::Pushbutton },
    ButtonMapping { number: 23, name: "Base Button (B1)", kind: ButtonKind::Pushbutton },
    ButtonMapping { number: 24, name: "Base Button (B2)", kind: ButtonKind::Pushbutton },
    ButtonMapping { number: 25, name: "Base Button (B3)", kind: ButtonKind::Pushbutton },
    ButtonMapping { number: 26, name: "Base Button (B4)", kind: ButtonKind::Pushbutton },
    ButtonMapping { number: 27, name: "Rapid Fire Fwd", kind: ButtonKind::Pushbutton },
    ButtonMapping { number: 28, name: "Rapid Fire Rev", kind: ButtonKind::Pushbutton },
    ButtonMapping { number: 29, name: "Flip Trigger Stage", kind: ButtonKind::Toggle },
    ButtonMapping { number: 30, name: "Throttle Brake", kind: ButtonKind::Pushbutton },
    ButtonMapping { number: 31, name: "Reserved 31", kind: ButtonKind::Pushbutton },
    ButtonMapping { number: 32, name: "Reserved 32", kind: ButtonKind::Pushbutton },
    ButtonMapping { number: 33, name: "Reserved 33", kind: ButtonKind::Pushbutton },
    ButtonMapping { number: 34, name: "Reserved 34", kind: ButtonKind::Pushbutton },
];

static GLADIATOR_NXT_EVO_HATS: [HatMapping; 4] = [
    HatMapping { index: 0, name: "POV Hat 1 (8-way)", kind: HatKind::Pov8Way },
    HatMapping { index: 1, name: "Trim Hat 2 (4-way)", kind: HatKind::Trim4Way },
    HatMapping { index: 2, name: "Trim Hat 3 (4-way)", kind: HatKind::Trim4Way },
    HatMapping { index: 3, name: "Trim Hat 4 (4-way)", kind: HatKind::Trim4Way },
];

static GLADIATOR_NXT_EVO_NOTES: [&str; 3] = [
    "Factory default profile for VKB Gladiator NXT EVO (left and right).",
    "HATs 2-4 are mapped as virtual buttons (4 buttons each) by default.",
    "VKBDevCfg can remap any control; treat this as a baseline hint.",
];

/// Factory-default profile for the VKB Gladiator NXT EVO.
pub fn gladiator_nxt_evo_profile() -> VkbDeviceProfile {
    VkbDeviceProfile {
        device_name: "VKB Gladiator NXT EVO",
        vid: VKB_VENDOR_ID,
        pids: &[VKB_GLADIATOR_NXT_EVO_RIGHT_PID, VKB_GLADIATOR_NXT_EVO_LEFT_PID],
        axes: &GLADIATOR_NXT_EVO_AXES,
        buttons: &GLADIATOR_NXT_EVO_BUTTONS,
        hats: &GLADIATOR_NXT_EVO_HATS,
        notes: &GLADIATOR_NXT_EVO_NOTES,
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Gunfighter + MCG profile
// ═══════════════════════════════════════════════════════════════════════════════

static GUNFIGHTER_MCG_AXES: [AxisMapping; 6] = [
    AxisMapping {
        name: "roll",
        report_offset: 0,
        mode: AxisNormMode::Signed,
        description: "Main stick X axis (roll) — metal gimbal",
    },
    AxisMapping {
        name: "pitch",
        report_offset: 2,
        mode: AxisNormMode::Signed,
        description: "Main stick Y axis (pitch) — metal gimbal",
    },
    AxisMapping {
        name: "yaw",
        report_offset: 4,
        mode: AxisNormMode::Signed,
        description: "Stick twist Z axis (yaw) — if twist adapter installed",
    },
    AxisMapping {
        name: "mini_x",
        report_offset: 6,
        mode: AxisNormMode::Signed,
        description: "MCG analog mini-stick X axis",
    },
    AxisMapping {
        name: "mini_y",
        report_offset: 8,
        mode: AxisNormMode::Signed,
        description: "MCG analog mini-stick Y axis",
    },
    AxisMapping {
        name: "throttle",
        report_offset: 10,
        mode: AxisNormMode::Unsigned,
        description: "Base throttle wheel / slider (Gunfighter base)",
    },
];

static GUNFIGHTER_MCG_BUTTONS: [ButtonMapping; 42] = [
    ButtonMapping { number: 1, name: "Trigger Stage 1", kind: ButtonKind::Trigger },
    ButtonMapping { number: 2, name: "Trigger Stage 2", kind: ButtonKind::Trigger },
    ButtonMapping { number: 3, name: "Side Button (A3)", kind: ButtonKind::Pushbutton },
    ButtonMapping { number: 4, name: "Side Button (A4)", kind: ButtonKind::Pushbutton },
    ButtonMapping { number: 5, name: "Pinkie Button", kind: ButtonKind::Pushbutton },
    ButtonMapping { number: 6, name: "Pinkie Lever", kind: ButtonKind::Toggle },
    ButtonMapping { number: 7, name: "HAT 2 Up", kind: ButtonKind::HatDirection },
    ButtonMapping { number: 8, name: "HAT 2 Right", kind: ButtonKind::HatDirection },
    ButtonMapping { number: 9, name: "HAT 2 Down", kind: ButtonKind::HatDirection },
    ButtonMapping { number: 10, name: "HAT 2 Left", kind: ButtonKind::HatDirection },
    ButtonMapping { number: 11, name: "HAT 3 Up", kind: ButtonKind::HatDirection },
    ButtonMapping { number: 12, name: "HAT 3 Right", kind: ButtonKind::HatDirection },
    ButtonMapping { number: 13, name: "HAT 3 Down", kind: ButtonKind::HatDirection },
    ButtonMapping { number: 14, name: "HAT 3 Left", kind: ButtonKind::HatDirection },
    ButtonMapping { number: 15, name: "HAT 4 Up", kind: ButtonKind::HatDirection },
    ButtonMapping { number: 16, name: "HAT 4 Right", kind: ButtonKind::HatDirection },
    ButtonMapping { number: 17, name: "HAT 4 Down", kind: ButtonKind::HatDirection },
    ButtonMapping { number: 18, name: "HAT 4 Left", kind: ButtonKind::HatDirection },
    ButtonMapping { number: 19, name: "Encoder CW", kind: ButtonKind::Encoder },
    ButtonMapping { number: 20, name: "Encoder CCW", kind: ButtonKind::Encoder },
    ButtonMapping { number: 21, name: "Encoder Press", kind: ButtonKind::Pushbutton },
    ButtonMapping { number: 22, name: "Mini-stick Press", kind: ButtonKind::Pushbutton },
    ButtonMapping { number: 23, name: "Folding Trigger", kind: ButtonKind::Trigger },
    ButtonMapping { number: 24, name: "MCG Castle Up", kind: ButtonKind::HatDirection },
    ButtonMapping { number: 25, name: "MCG Castle Right", kind: ButtonKind::HatDirection },
    ButtonMapping { number: 26, name: "MCG Castle Down", kind: ButtonKind::HatDirection },
    ButtonMapping { number: 27, name: "MCG Castle Left", kind: ButtonKind::HatDirection },
    ButtonMapping { number: 28, name: "MCG Castle Press", kind: ButtonKind::Pushbutton },
    ButtonMapping { number: 29, name: "MCG Thumb Wheel Up", kind: ButtonKind::Encoder },
    ButtonMapping { number: 30, name: "MCG Thumb Wheel Down", kind: ButtonKind::Encoder },
    ButtonMapping { number: 31, name: "MCG Thumb Wheel Press", kind: ButtonKind::Pushbutton },
    ButtonMapping { number: 32, name: "MCG Brake Lever", kind: ButtonKind::Pushbutton },
    ButtonMapping { number: 33, name: "MCG TDC Slew Press", kind: ButtonKind::Pushbutton },
    ButtonMapping { number: 34, name: "MCG Paddle", kind: ButtonKind::Pushbutton },
    ButtonMapping { number: 35, name: "Base Button (B1)", kind: ButtonKind::Pushbutton },
    ButtonMapping { number: 36, name: "Base Button (B2)", kind: ButtonKind::Pushbutton },
    ButtonMapping { number: 37, name: "Base Button (B3)", kind: ButtonKind::Pushbutton },
    ButtonMapping { number: 38, name: "Base Button (B4)", kind: ButtonKind::Pushbutton },
    ButtonMapping { number: 39, name: "Rapid Fire Fwd", kind: ButtonKind::Pushbutton },
    ButtonMapping { number: 40, name: "Rapid Fire Rev", kind: ButtonKind::Pushbutton },
    ButtonMapping { number: 41, name: "Reserved 41", kind: ButtonKind::Pushbutton },
    ButtonMapping { number: 42, name: "Reserved 42", kind: ButtonKind::Pushbutton },
];

static GUNFIGHTER_MCG_HATS: [HatMapping; 4] = [
    HatMapping { index: 0, name: "POV Hat 1 (8-way)", kind: HatKind::Pov8Way },
    HatMapping { index: 1, name: "Trim Hat 2 (4-way)", kind: HatKind::Trim4Way },
    HatMapping { index: 2, name: "Trim Hat 3 (4-way)", kind: HatKind::Trim4Way },
    HatMapping {
        index: 3,
        name: "Castle Switch (4-way + press)",
        kind: HatKind::Trim4Way,
    },
];

static GUNFIGHTER_MCG_NOTES: [&str; 4] = [
    "Factory default for Gunfighter Mk.III/IV + MCG Pro/Ultimate grip.",
    "Metal gimbal provides higher precision than Gladiator NXT EVO.",
    "MCG adds castle switch, TDC slew, paddle, and thumb wheel over NXT EVO.",
    "VKBDevCfg can remap any control; treat this as a baseline hint.",
];

/// Factory-default profile for VKB Gunfighter with MCG grip.
pub fn gunfighter_mcg_profile() -> VkbDeviceProfile {
    VkbDeviceProfile {
        device_name: "VKB Gunfighter + MCG",
        vid: VKB_VENDOR_ID,
        pids: &[
            VKB_GUNFIGHTER_MODERN_COMBAT_PRO_PID,
            VKB_SPACE_GUNFIGHTER_PID,
            VKB_SPACE_GUNFIGHTER_LEFT_PID,
        ],
        axes: &GUNFIGHTER_MCG_AXES,
        buttons: &GUNFIGHTER_MCG_BUTTONS,
        hats: &GUNFIGHTER_MCG_HATS,
        notes: &GUNFIGHTER_MCG_NOTES,
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// STECS Standard/Plus throttle profile
// ═══════════════════════════════════════════════════════════════════════════════

static STECS_THROTTLE_AXES: [AxisMapping; 5] = [
    AxisMapping {
        name: "rx",
        report_offset: 0,
        mode: AxisNormMode::Unsigned,
        description: "RX axis (SpaceBrake on baseline maps)",
    },
    AxisMapping {
        name: "ry",
        report_offset: 2,
        mode: AxisNormMode::Unsigned,
        description: "RY axis (Laser Power on baseline maps)",
    },
    AxisMapping {
        name: "x",
        report_offset: 4,
        mode: AxisNormMode::Unsigned,
        description: "X axis (secondary throttle / range knob)",
    },
    AxisMapping {
        name: "y",
        report_offset: 6,
        mode: AxisNormMode::Unsigned,
        description: "Y axis (secondary throttle / antenna elevation)",
    },
    AxisMapping {
        name: "z",
        report_offset: 8,
        mode: AxisNormMode::Unsigned,
        description: "Z axis (main throttle lever)",
    },
];

static STECS_THROTTLE_BUTTONS: [ButtonMapping; 24] = [
    ButtonMapping { number: 1, name: "Detent Button 1", kind: ButtonKind::Pushbutton },
    ButtonMapping { number: 2, name: "Detent Button 2", kind: ButtonKind::Pushbutton },
    ButtonMapping { number: 3, name: "Throttle Idle Switch", kind: ButtonKind::Toggle },
    ButtonMapping { number: 4, name: "Throttle AB Switch", kind: ButtonKind::Toggle },
    ButtonMapping { number: 5, name: "Engine Start L", kind: ButtonKind::Pushbutton },
    ButtonMapping { number: 6, name: "Engine Start R", kind: ButtonKind::Pushbutton },
    ButtonMapping { number: 7, name: "Pinkie Switch", kind: ButtonKind::Toggle },
    ButtonMapping { number: 8, name: "Speed Brake", kind: ButtonKind::Pushbutton },
    ButtonMapping { number: 9, name: "Flaps Up", kind: ButtonKind::Pushbutton },
    ButtonMapping { number: 10, name: "Flaps Down", kind: ButtonKind::Pushbutton },
    ButtonMapping { number: 11, name: "Landing Gear Toggle", kind: ButtonKind::Toggle },
    ButtonMapping { number: 12, name: "EAC", kind: ButtonKind::Toggle },
    ButtonMapping { number: 13, name: "RDR Altimeter", kind: ButtonKind::Toggle },
    ButtonMapping { number: 14, name: "Autopilot Engage", kind: ButtonKind::Pushbutton },
    ButtonMapping { number: 15, name: "Encoder 1 CW", kind: ButtonKind::Encoder },
    ButtonMapping { number: 16, name: "Encoder 1 CCW", kind: ButtonKind::Encoder },
    ButtonMapping { number: 17, name: "Encoder 1 Press", kind: ButtonKind::Pushbutton },
    ButtonMapping { number: 18, name: "Encoder 2 CW", kind: ButtonKind::Encoder },
    ButtonMapping { number: 19, name: "Encoder 2 CCW", kind: ButtonKind::Encoder },
    ButtonMapping { number: 20, name: "Encoder 2 Press", kind: ButtonKind::Pushbutton },
    ButtonMapping { number: 21, name: "Coolie Hat Up", kind: ButtonKind::HatDirection },
    ButtonMapping { number: 22, name: "Coolie Hat Right", kind: ButtonKind::HatDirection },
    ButtonMapping { number: 23, name: "Coolie Hat Down", kind: ButtonKind::HatDirection },
    ButtonMapping { number: 24, name: "Coolie Hat Left", kind: ButtonKind::HatDirection },
];

static STECS_THROTTLE_HATS: [HatMapping; 0] = [];

static STECS_THROTTLE_NOTES: [&str; 3] = [
    "Factory default for VKB STECS Standard/Plus throttle.",
    "STECS uses multi-virtual-controller HID (VC0..VC2); aggregate before mapping.",
    "Standard has 2 rotary encoders; Plus adds extra buttons in higher VC slots.",
];

/// Factory-default profile for VKB STECS Standard/Plus throttle.
pub fn stecs_throttle_profile() -> VkbDeviceProfile {
    VkbDeviceProfile {
        device_name: "VKB STECS Standard/Plus",
        vid: VKB_VENDOR_ID,
        pids: &[
            VKB_STECS_RIGHT_SPACE_STANDARD_PID,
            VKB_STECS_LEFT_SPACE_STANDARD_PID,
        ],
        axes: &STECS_THROTTLE_AXES,
        buttons: &STECS_THROTTLE_BUTTONS,
        hats: &STECS_THROTTLE_HATS,
        notes: &STECS_THROTTLE_NOTES,
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// SEM THQ (Side Extension Module – Throttle Quadrant) profile
// ═══════════════════════════════════════════════════════════════════════════════

static SEM_THQ_AXES: [AxisMapping; 4] = [
    AxisMapping {
        name: "throttle_left",
        report_offset: 0,
        mode: AxisNormMode::Unsigned,
        description: "Left throttle lever",
    },
    AxisMapping {
        name: "throttle_right",
        report_offset: 2,
        mode: AxisNormMode::Unsigned,
        description: "Right throttle lever",
    },
    AxisMapping {
        name: "rotary_left",
        report_offset: 4,
        mode: AxisNormMode::Unsigned,
        description: "Left rotary knob",
    },
    AxisMapping {
        name: "rotary_right",
        report_offset: 6,
        mode: AxisNormMode::Unsigned,
        description: "Right rotary knob",
    },
];

static SEM_THQ_BUTTONS: [ButtonMapping; 12] = [
    ButtonMapping { number: 1, name: "Detent Left", kind: ButtonKind::Pushbutton },
    ButtonMapping { number: 2, name: "Detent Right", kind: ButtonKind::Pushbutton },
    ButtonMapping { number: 3, name: "Idle Gate Left", kind: ButtonKind::Toggle },
    ButtonMapping { number: 4, name: "Idle Gate Right", kind: ButtonKind::Toggle },
    ButtonMapping { number: 5, name: "AB Gate Left", kind: ButtonKind::Toggle },
    ButtonMapping { number: 6, name: "AB Gate Right", kind: ButtonKind::Toggle },
    ButtonMapping { number: 7, name: "Pinkie Left", kind: ButtonKind::Pushbutton },
    ButtonMapping { number: 8, name: "Pinkie Right", kind: ButtonKind::Pushbutton },
    ButtonMapping { number: 9, name: "Encoder 1 CW", kind: ButtonKind::Encoder },
    ButtonMapping { number: 10, name: "Encoder 1 CCW", kind: ButtonKind::Encoder },
    ButtonMapping { number: 11, name: "Encoder 2 CW", kind: ButtonKind::Encoder },
    ButtonMapping { number: 12, name: "Encoder 2 CCW", kind: ButtonKind::Encoder },
];

static SEM_THQ_HATS: [HatMapping; 0] = [];

static SEM_THQ_NOTES: [&str; 2] = [
    "Factory default for VKB SEM THQ (Side Extension Module throttle quadrant).",
    "Dual throttle + dual rotary with detent/gate switches.",
];

/// Factory-default profile for VKB SEM THQ.
pub fn sem_thq_profile() -> VkbDeviceProfile {
    VkbDeviceProfile {
        device_name: "VKB SEM Throttle Quadrant",
        vid: VKB_VENDOR_ID,
        pids: &[VKB_NXT_SEM_THQ_PID],
        axes: &SEM_THQ_AXES,
        buttons: &SEM_THQ_BUTTONS,
        hats: &SEM_THQ_HATS,
        notes: &SEM_THQ_NOTES,
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// T-Rudder Mk.IV profile
// ═══════════════════════════════════════════════════════════════════════════════

static T_RUDDER_AXES: [AxisMapping; 3] = [
    AxisMapping {
        name: "left_toe_brake",
        report_offset: 0,
        mode: AxisNormMode::Unsigned,
        description: "Left toe brake pedal (0.0 = released, 1.0 = fully pressed)",
    },
    AxisMapping {
        name: "right_toe_brake",
        report_offset: 2,
        mode: AxisNormMode::Unsigned,
        description: "Right toe brake pedal (0.0 = released, 1.0 = fully pressed)",
    },
    AxisMapping {
        name: "rudder",
        report_offset: 4,
        mode: AxisNormMode::Signed,
        description: "Rudder axis (−1.0 = full left, 1.0 = full right)",
    },
];

static T_RUDDER_BUTTONS: [ButtonMapping; 0] = [];
static T_RUDDER_HATS: [HatMapping; 0] = [];

static T_RUDDER_NOTES: [&str; 2] = [
    "Factory default for VKB T-Rudder Mk.IV pedals.",
    "Three axes: two toe brakes (unidirectional) and one rudder (bidirectional).",
];

/// Factory-default profile for VKB T-Rudder Mk.IV pedals.
///
/// The T-Rudder has 3 axes and no buttons or hats.
/// Note: The Mk.V PID is not yet confirmed; this profile uses the Mk.IV layout
/// which is expected to be identical.
pub fn t_rudder_profile() -> VkbDeviceProfile {
    VkbDeviceProfile {
        device_name: "VKB T-Rudder Mk.IV",
        vid: VKB_VENDOR_ID,
        // T-Rudder PID not in device_support.rs yet; leave empty for now.
        // Users should match by descriptor discovery or explicit configuration.
        pids: &[],
        axes: &T_RUDDER_AXES,
        buttons: &T_RUDDER_BUTTONS,
        hats: &T_RUDDER_HATS,
        notes: &T_RUDDER_NOTES,
    }
}

// ─── Profile registry ─────────────────────────────────────────────────────────

/// Return all built-in VKB device profiles.
pub fn all_profiles() -> [VkbDeviceProfile; 5] {
    [
        gladiator_nxt_evo_profile(),
        gunfighter_mcg_profile(),
        stecs_throttle_profile(),
        sem_thq_profile(),
        t_rudder_profile(),
    ]
}

/// Look up a profile by USB product ID.
///
/// Returns `None` if no built-in profile matches the given PID.
pub fn profile_for_pid(pid: u16) -> Option<VkbDeviceProfile> {
    all_profiles().into_iter().find(|p| p.pids.contains(&pid))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── Profile completeness ─────────────────────────────────────────────

    #[test]
    fn gladiator_nxt_evo_profile_completeness() {
        let p = gladiator_nxt_evo_profile();
        assert_eq!(p.axis_count(), 6, "NXT EVO should have 6 axes");
        assert!(p.button_count() >= 30, "NXT EVO should have 30+ buttons");
        assert_eq!(p.hat_count(), 4, "NXT EVO should have 4 hat switches");
        assert!(!p.pids.is_empty(), "must have at least one PID");
        assert_eq!(p.vid, VKB_VENDOR_ID);
    }

    #[test]
    fn gunfighter_mcg_profile_completeness() {
        let p = gunfighter_mcg_profile();
        assert_eq!(p.axis_count(), 6, "Gunfighter+MCG should have 6 axes");
        assert!(p.button_count() >= 40, "MCG adds buttons over NXT EVO");
        assert_eq!(p.hat_count(), 4, "Gunfighter+MCG should have 4 hat switches");
        assert!(!p.pids.is_empty());
    }

    #[test]
    fn stecs_throttle_profile_completeness() {
        let p = stecs_throttle_profile();
        assert_eq!(p.axis_count(), 5, "STECS should have 5 axes (rx,ry,x,y,z)");
        assert!(p.button_count() >= 20, "STECS should have 20+ buttons");
        assert!(!p.pids.is_empty());
    }

    #[test]
    fn sem_thq_profile_completeness() {
        let p = sem_thq_profile();
        assert_eq!(p.axis_count(), 4, "SEM THQ should have 4 axes");
        assert!(p.button_count() >= 10, "SEM THQ should have 10+ buttons");
        assert!(!p.pids.is_empty());
    }

    #[test]
    fn t_rudder_profile_completeness() {
        let p = t_rudder_profile();
        assert_eq!(p.axis_count(), 3, "T-Rudder should have 3 axes");
        assert_eq!(p.button_count(), 0, "T-Rudder has no buttons");
        assert_eq!(p.hat_count(), 0, "T-Rudder has no hats");
    }

    // ─── Axis normalisation modes ─────────────────────────────────────────

    #[test]
    fn gladiator_joystick_axes_are_signed() {
        let p = gladiator_nxt_evo_profile();
        for name in &["roll", "pitch", "yaw", "mini_x", "mini_y"] {
            let axis = p.axis_by_name(name).unwrap_or_else(|| panic!("missing axis: {name}"));
            assert_eq!(axis.mode, AxisNormMode::Signed, "{name} should be signed");
        }
    }

    #[test]
    fn gladiator_throttle_axis_is_unsigned() {
        let p = gladiator_nxt_evo_profile();
        let axis = p.axis_by_name("throttle").expect("missing throttle axis");
        assert_eq!(axis.mode, AxisNormMode::Unsigned);
    }

    #[test]
    fn stecs_all_axes_are_unsigned() {
        let p = stecs_throttle_profile();
        for axis in p.axes {
            assert_eq!(
                axis.mode,
                AxisNormMode::Unsigned,
                "STECS axis '{}' should be unsigned",
                axis.name
            );
        }
    }

    #[test]
    fn sem_thq_all_axes_are_unsigned() {
        let p = sem_thq_profile();
        for axis in p.axes {
            assert_eq!(
                axis.mode,
                AxisNormMode::Unsigned,
                "SEM THQ axis '{}' should be unsigned",
                axis.name
            );
        }
    }

    #[test]
    fn t_rudder_toe_brakes_unsigned_rudder_signed() {
        let p = t_rudder_profile();
        let left = p.axis_by_name("left_toe_brake").unwrap();
        let right = p.axis_by_name("right_toe_brake").unwrap();
        let rudder = p.axis_by_name("rudder").unwrap();
        assert_eq!(left.mode, AxisNormMode::Unsigned);
        assert_eq!(right.mode, AxisNormMode::Unsigned);
        assert_eq!(rudder.mode, AxisNormMode::Signed);
    }

    // ─── Button numbering ─────────────────────────────────────────────────

    #[test]
    fn button_numbers_are_sequential_and_unique() {
        for profile in all_profiles() {
            let numbers: Vec<u8> = profile.buttons.iter().map(|b| b.number).collect();
            for (i, num) in numbers.iter().enumerate() {
                assert!(
                    *num >= 1,
                    "{}: button number must be >= 1",
                    profile.device_name
                );
                // Check uniqueness
                assert_eq!(
                    numbers.iter().filter(|&&n| n == *num).count(),
                    1,
                    "{}: duplicate button number {}",
                    profile.device_name,
                    num
                );
                // Check sequential
                if i > 0 {
                    assert!(
                        *num > numbers[i - 1],
                        "{}: buttons not in ascending order",
                        profile.device_name
                    );
                }
            }
        }
    }

    // ─── Axis offsets ─────────────────────────────────────────────────────

    #[test]
    fn axis_offsets_do_not_overlap() {
        for profile in all_profiles() {
            let offsets: Vec<usize> = profile.axes.iter().map(|a| a.report_offset).collect();
            for (i, offset) in offsets.iter().enumerate() {
                // Each u16 axis occupies 2 bytes; offsets must not overlap.
                for (j, other) in offsets.iter().enumerate() {
                    if i != j {
                        assert!(
                            (*offset as isize - *other as isize).unsigned_abs() >= 2,
                            "{}: axis offset overlap between '{}' (offset {}) and '{}' (offset {})",
                            profile.device_name,
                            profile.axes[i].name,
                            offset,
                            profile.axes[j].name,
                            other,
                        );
                    }
                }
            }
        }
    }

    // ─── Profile lookup ───────────────────────────────────────────────────

    #[test]
    fn profile_for_known_pid() {
        let p = profile_for_pid(VKB_GLADIATOR_NXT_EVO_RIGHT_PID);
        assert!(p.is_some());
        assert_eq!(p.unwrap().device_name, "VKB Gladiator NXT EVO");
    }

    #[test]
    fn profile_for_gunfighter_pid() {
        let p = profile_for_pid(VKB_GUNFIGHTER_MODERN_COMBAT_PRO_PID);
        assert!(p.is_some());
        assert_eq!(p.unwrap().device_name, "VKB Gunfighter + MCG");
    }

    #[test]
    fn profile_for_stecs_pid() {
        let p = profile_for_pid(VKB_STECS_RIGHT_SPACE_STANDARD_PID);
        assert!(p.is_some());
        assert_eq!(p.unwrap().device_name, "VKB STECS Standard/Plus");
    }

    #[test]
    fn profile_for_sem_thq_pid() {
        let p = profile_for_pid(VKB_NXT_SEM_THQ_PID);
        assert!(p.is_some());
        assert_eq!(p.unwrap().device_name, "VKB SEM Throttle Quadrant");
    }

    #[test]
    fn profile_for_unknown_pid_returns_none() {
        assert!(profile_for_pid(0x9999).is_none());
    }

    // ─── All profiles registry ────────────────────────────────────────────

    #[test]
    fn all_profiles_count() {
        assert_eq!(all_profiles().len(), 5);
    }

    #[test]
    fn all_profiles_have_names() {
        for p in all_profiles() {
            assert!(!p.device_name.is_empty(), "profile must have a name");
        }
    }

    #[test]
    fn all_profiles_have_notes() {
        for p in all_profiles() {
            assert!(!p.notes.is_empty(), "{} should have notes", p.device_name);
        }
    }

    // ─── Lookup helpers ───────────────────────────────────────────────────

    #[test]
    fn axis_by_name_found() {
        let p = gladiator_nxt_evo_profile();
        assert!(p.axis_by_name("roll").is_some());
        assert!(p.axis_by_name("pitch").is_some());
        assert!(p.axis_by_name("yaw").is_some());
        assert!(p.axis_by_name("throttle").is_some());
    }

    #[test]
    fn axis_by_name_not_found() {
        let p = gladiator_nxt_evo_profile();
        assert!(p.axis_by_name("nonexistent").is_none());
    }

    #[test]
    fn button_by_number_found() {
        let p = gladiator_nxt_evo_profile();
        let btn = p.button_by_number(1).expect("button 1 should exist");
        assert_eq!(btn.kind, ButtonKind::Trigger);
    }

    #[test]
    fn button_by_number_not_found() {
        let p = t_rudder_profile();
        assert!(p.button_by_number(1).is_none());
    }

    // ─── Gunfighter has more buttons than Gladiator ───────────────────────

    #[test]
    fn gunfighter_has_more_buttons_than_gladiator() {
        let glad = gladiator_nxt_evo_profile();
        let gun = gunfighter_mcg_profile();
        assert!(
            gun.button_count() > glad.button_count(),
            "Gunfighter+MCG ({}) should have more buttons than NXT EVO ({})",
            gun.button_count(),
            glad.button_count()
        );
    }
}
