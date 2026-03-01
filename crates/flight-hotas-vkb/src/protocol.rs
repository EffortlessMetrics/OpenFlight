// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! VKB HID protocol details: report descriptors, multi-mode buttons, LED control,
//! and device family identification for VKB Gladiator, Gunfighter, and accessory products.
//!
//! # Supported device families
//!
//! | Family | PIDs | Notes |
//! |--------|------|-------|
//! | Gladiator NXT EVO | 0x0200, 0x0201 | 6-axis stick, twist, 64 buttons, 2 hats |
//! | Gunfighter + MCG | 0x0125, 0x0126, 0x0127 | Metal gimbal, same axis layout, more buttons |
//! | Gladiator MCP | 0x0131 | Gladiator base + MCG Pro grip |
//! | SEM THQ | 0x2214 | Side Extension Module throttle quadrant |
//! | Gladiator Mk.II | 0x0121 | Legacy Gladiator (~2014–2017) |
//! | T-Rudder | 0x0132 (est.) | Pedals: 3 axes, no buttons |
//! | STECS Modern | 0x012B, 0x012E | Modern Throttle Mini/Max |
//!
//! # HID report conventions
//!
//! All VKB joystick-class devices share a common firmware family authored by
//! Alex Oz / VKBsim.  The report layout follows the same general pattern:
//!
//! - Axes are **16-bit unsigned little-endian** (u16 LE).
//! - Buttons are packed in **32-bit words** (u32 LE, LSB = lowest button number).
//! - Hat switches are encoded as **4-bit nibbles** (0=N … 7=NW, 0xF=centred).
//! - The firmware may prepend a 1-byte HID report ID (`0x01`).
//!
//! VKBDevCfg can remap hats, ministicks, and axes; default mappings should be
//! treated as hints.  Prefer descriptor-first discovery when available.

use flight_hid_support::device_support::{
    VKB_GLADIATOR_MK2_PID, VKB_GLADIATOR_MODERN_COMBAT_PRO_PID, VKB_GLADIATOR_NXT_EVO_LEFT_PID,
    VKB_GLADIATOR_NXT_EVO_RIGHT_PID, VKB_GLADIATOR_NXT_EVO_RIGHT_SEM_PID,
    VKB_GUNFIGHTER_MODERN_COMBAT_PRO_PID, VKB_NXT_SEM_THQ_PID, VKB_SPACE_GUNFIGHTER_LEFT_PID,
    VKB_SPACE_GUNFIGHTER_PID, VKB_STECS_MODERN_THROTTLE_MAX_PID,
    VKB_STECS_MODERN_THROTTLE_MINI_PID, VKB_VENDOR_ID,
};

// ─── T-Rudder PIDs (not yet in flight-hid-support) ───────────────────────────

/// Community-estimated PID for the VKB T-Rudder Mk.V pedals.
///
/// **UNCONFIRMED** — community estimate. Verify with `lsusb` or VKBDevCfg.
pub const VKB_T_RUDDER_MK5_PID: u16 = 0x0132;

// ─── Device family classification ─────────────────────────────────────────────

/// High-level VKB device family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VkbDeviceFamily {
    /// Gladiator NXT EVO (consumer plastic gimbal, left/right).
    GladiatorNxtEvo,
    /// Gunfighter pedestal base with MCG or SCG grip.
    Gunfighter,
    /// Gladiator base with Modern Combat Pro grip.
    GladiatorMcp,
    /// Side Extension Module throttle quadrant (SEM THQ).
    SemThq,
    /// NXT EVO with SEM attached (combined PID).
    GladiatorNxtEvoSem,
    /// Original Gladiator Mk.II (~2014–2017).
    GladiatorMk2,
    /// T-Rudder pedals (Mk.IV / Mk.V).
    TRudder,
    /// STECS Modern Throttle (Mini / Max variants).
    StecsModernThrottle,
}

impl VkbDeviceFamily {
    /// Classify a VKB device by product ID.  Returns `None` for unknown PIDs.
    pub fn from_pid(pid: u16) -> Option<Self> {
        match pid {
            VKB_GLADIATOR_NXT_EVO_RIGHT_PID | VKB_GLADIATOR_NXT_EVO_LEFT_PID => {
                Some(Self::GladiatorNxtEvo)
            }
            VKB_GUNFIGHTER_MODERN_COMBAT_PRO_PID
            | VKB_SPACE_GUNFIGHTER_PID
            | VKB_SPACE_GUNFIGHTER_LEFT_PID => Some(Self::Gunfighter),
            VKB_GLADIATOR_MODERN_COMBAT_PRO_PID => Some(Self::GladiatorMcp),
            VKB_NXT_SEM_THQ_PID => Some(Self::SemThq),
            VKB_GLADIATOR_NXT_EVO_RIGHT_SEM_PID => Some(Self::GladiatorNxtEvoSem),
            VKB_GLADIATOR_MK2_PID => Some(Self::GladiatorMk2),
            VKB_T_RUDDER_MK5_PID => Some(Self::TRudder),
            VKB_STECS_MODERN_THROTTLE_MINI_PID | VKB_STECS_MODERN_THROTTLE_MAX_PID => {
                Some(Self::StecsModernThrottle)
            }
            _ => None,
        }
    }

    /// Human-readable family name.
    pub fn name(self) -> &'static str {
        match self {
            Self::GladiatorNxtEvo => "VKB Gladiator NXT EVO",
            Self::Gunfighter => "VKB Gunfighter",
            Self::GladiatorMcp => "VKB Gladiator Modern Combat Pro",
            Self::SemThq => "VKB SEM Throttle Quadrant",
            Self::GladiatorNxtEvoSem => "VKB Gladiator NXT EVO + SEM",
            Self::GladiatorMk2 => "VKB Gladiator Mk.II",
            Self::TRudder => "VKB T-Rudder",
            Self::StecsModernThrottle => "VKB STECS Modern Throttle",
        }
    }
}

// ─── Joystick report layout ───────────────────────────────────────────────────

/// Report layout descriptor for VKB joystick-class devices.
///
/// All VKB sticks (Gladiator, Gunfighter, MCG grips) share this base layout.
/// The firmware may expose additional fields in longer reports; extra bytes are
/// silently ignored by the parser.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VkbJoystickReportLayout {
    /// Number of 16-bit axis fields at the start of the report.
    pub axis_count: u8,
    /// Number of 32-bit button words following the axes.
    pub button_word_count: u8,
    /// Whether hat switches are encoded after buttons (1 byte, 2 nibbles).
    pub has_hat_byte: bool,
    /// Minimum report payload length (excluding optional report ID).
    pub min_payload_bytes: usize,
}

/// Standard report layout for Gladiator NXT EVO and Gunfighter sticks.
///
/// 6 axes (12 bytes) + 2 button words (8 bytes) + 1 hat byte = 21 bytes.
pub const VKB_JOYSTICK_STANDARD_LAYOUT: VkbJoystickReportLayout = VkbJoystickReportLayout {
    axis_count: 6,
    button_word_count: 2,
    has_hat_byte: true,
    min_payload_bytes: 21,
};

/// Compact report layout for SEM THQ (throttle quadrant).
///
/// 4 axes (8 bytes) + 2 button words (8 bytes) = 16 bytes.
/// No hat switches on the SEM.
pub const VKB_SEM_THQ_LAYOUT: VkbJoystickReportLayout = VkbJoystickReportLayout {
    axis_count: 4,
    button_word_count: 2,
    has_hat_byte: false,
    min_payload_bytes: 16,
};

/// Report layout for VKB T-Rudder pedals.
///
/// 3 axes (6 bytes), no buttons, no hats.
pub const VKB_T_RUDDER_LAYOUT: VkbJoystickReportLayout = VkbJoystickReportLayout {
    axis_count: 3,
    button_word_count: 0,
    has_hat_byte: false,
    min_payload_bytes: 6,
};

/// Report layout for VKB STECS Modern Throttle (Mini / Max).
///
/// 4 axes (8 bytes) + 2 button words (8 bytes) = 16 bytes payload.
/// The report includes a 1-byte report ID prefix (17 bytes total minimum).
pub const VKB_STECS_MODERN_LAYOUT: VkbJoystickReportLayout = VkbJoystickReportLayout {
    axis_count: 4,
    button_word_count: 2,
    has_hat_byte: false,
    min_payload_bytes: 16,
};

/// Return the expected report layout for a given device family.
pub fn report_layout_for_family(family: VkbDeviceFamily) -> VkbJoystickReportLayout {
    match family {
        VkbDeviceFamily::SemThq => VKB_SEM_THQ_LAYOUT,
        VkbDeviceFamily::TRudder => VKB_T_RUDDER_LAYOUT,
        VkbDeviceFamily::StecsModernThrottle => VKB_STECS_MODERN_LAYOUT,
        _ => VKB_JOYSTICK_STANDARD_LAYOUT,
    }
}

// ─── VKB Device Info Table ────────────────────────────────────────────────────

/// PID entry for a VKB device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VkbDeviceInfo {
    /// USB Product ID.
    pub pid: u16,
    /// Device family.
    pub family: VkbDeviceFamily,
    /// Human-readable product name.
    pub name: &'static str,
    /// Number of analogue axes.
    pub axis_count: u8,
    /// Maximum number of digital buttons.
    pub max_buttons: u8,
    /// Number of hat switches.
    pub hat_count: u8,
    /// Minimum HID report payload size in bytes (excluding report ID).
    pub min_report_bytes: usize,
}

/// Table of all known VKB joystick-class devices with their parameters.
pub const VKB_DEVICE_TABLE: &[VkbDeviceInfo] = &[
    VkbDeviceInfo {
        pid: VKB_GLADIATOR_NXT_EVO_RIGHT_PID,
        family: VkbDeviceFamily::GladiatorNxtEvo,
        name: "VKB Gladiator NXT EVO (Right)",
        axis_count: 6,
        max_buttons: 64,
        hat_count: 2,
        min_report_bytes: 21,
    },
    VkbDeviceInfo {
        pid: VKB_GLADIATOR_NXT_EVO_LEFT_PID,
        family: VkbDeviceFamily::GladiatorNxtEvo,
        name: "VKB Gladiator NXT EVO (Left)",
        axis_count: 6,
        max_buttons: 64,
        hat_count: 2,
        min_report_bytes: 21,
    },
    VkbDeviceInfo {
        pid: VKB_GUNFIGHTER_MODERN_COMBAT_PRO_PID,
        family: VkbDeviceFamily::Gunfighter,
        name: "VKB Gunfighter Modern Combat Pro",
        axis_count: 6,
        max_buttons: 64,
        hat_count: 2,
        min_report_bytes: 21,
    },
    VkbDeviceInfo {
        pid: VKB_SPACE_GUNFIGHTER_PID,
        family: VkbDeviceFamily::Gunfighter,
        name: "VKB Space Gunfighter",
        axis_count: 6,
        max_buttons: 64,
        hat_count: 2,
        min_report_bytes: 21,
    },
    VkbDeviceInfo {
        pid: VKB_SPACE_GUNFIGHTER_LEFT_PID,
        family: VkbDeviceFamily::Gunfighter,
        name: "VKB Space Gunfighter (Left)",
        axis_count: 6,
        max_buttons: 64,
        hat_count: 2,
        min_report_bytes: 21,
    },
    VkbDeviceInfo {
        pid: VKB_GLADIATOR_MODERN_COMBAT_PRO_PID,
        family: VkbDeviceFamily::GladiatorMcp,
        name: "VKB Gladiator Modern Combat Pro",
        axis_count: 6,
        max_buttons: 64,
        hat_count: 2,
        min_report_bytes: 21,
    },
    VkbDeviceInfo {
        pid: VKB_NXT_SEM_THQ_PID,
        family: VkbDeviceFamily::SemThq,
        name: "VKB NXT SEM Throttle Quadrant",
        axis_count: 4,
        max_buttons: 64,
        hat_count: 0,
        min_report_bytes: 16,
    },
    VkbDeviceInfo {
        pid: VKB_GLADIATOR_NXT_EVO_RIGHT_SEM_PID,
        family: VkbDeviceFamily::GladiatorNxtEvoSem,
        name: "VKB Gladiator NXT EVO + SEM (Right)",
        axis_count: 6,
        max_buttons: 64,
        hat_count: 2,
        min_report_bytes: 21,
    },
    VkbDeviceInfo {
        pid: VKB_GLADIATOR_MK2_PID,
        family: VkbDeviceFamily::GladiatorMk2,
        name: "VKB Gladiator Mk.II",
        axis_count: 6,
        max_buttons: 64,
        hat_count: 2,
        min_report_bytes: 21,
    },
    VkbDeviceInfo {
        pid: VKB_T_RUDDER_MK5_PID,
        family: VkbDeviceFamily::TRudder,
        name: "VKB T-Rudder Mk.V",
        axis_count: 3,
        max_buttons: 0,
        hat_count: 0,
        min_report_bytes: 6,
    },
    VkbDeviceInfo {
        pid: VKB_STECS_MODERN_THROTTLE_MINI_PID,
        family: VkbDeviceFamily::StecsModernThrottle,
        name: "VKB STECS Modern Throttle Mini",
        axis_count: 4,
        max_buttons: 64,
        hat_count: 0,
        min_report_bytes: 17,
    },
    VkbDeviceInfo {
        pid: VKB_STECS_MODERN_THROTTLE_MAX_PID,
        family: VkbDeviceFamily::StecsModernThrottle,
        name: "VKB STECS Modern Throttle Max",
        axis_count: 4,
        max_buttons: 64,
        hat_count: 0,
        min_report_bytes: 17,
    },
];

/// Look up a [`VkbDeviceInfo`] entry by USB Product ID.
///
/// Returns `None` for unknown PIDs.
pub fn vkb_device_info(pid: u16) -> Option<&'static VkbDeviceInfo> {
    VKB_DEVICE_TABLE.iter().find(|d| d.pid == pid)
}

// ─── Axis resolution ──────────────────────────────────────────────────────────

/// Axis resolution details for VKB devices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VkbAxisResolution {
    /// Number of bits per axis sample.
    pub bits: u8,
    /// Logical minimum value reported by the HID descriptor.
    pub logical_min: u16,
    /// Logical maximum value reported by the HID descriptor.
    pub logical_max: u16,
}

/// All VKB joystick-class axes use 16-bit resolution (0..65535).
pub const VKB_AXIS_16BIT: VkbAxisResolution = VkbAxisResolution {
    bits: 16,
    logical_min: 0,
    logical_max: 0xFFFF,
};

// ─── Multi-mode / shifted button support ──────────────────────────────────────

/// VKB firmware supports shifted button layers via a "virtual button shift" mechanism.
/// A physical button can be mapped to emit different logical button numbers depending
/// on the current shift state.
///
/// Shift modes are configured via VKBDevCfg and cannot be detected from the HID
/// descriptor alone.  This struct describes the shift model for documentation and
/// mapping purposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VkbShiftMode {
    /// Number of supported shift layers (typically 1–3).
    pub layer_count: u8,
    /// Maximum physical buttons before shifting.
    pub physical_button_count: u8,
    /// Maximum logical buttons after shift expansion.
    pub logical_button_count: u8,
}

/// Default shift model for Gladiator NXT EVO grips.
///
/// The NXT EVO has ~30 physical buttons and supports up to 2 shift layers
/// in VKBDevCfg, expanding to a maximum of 64 logical buttons.
pub const GLADIATOR_NXT_EVO_SHIFT: VkbShiftMode = VkbShiftMode {
    layer_count: 2,
    physical_button_count: 34,
    logical_button_count: 64,
};

/// Default shift model for Gunfighter + MCG Ultimate grip.
///
/// The MCG Ultimate has many physical buttons; with 3 shift layers the
/// firmware can expose up to 128 logical buttons across multiple VCs.
pub const GUNFIGHTER_MCG_SHIFT: VkbShiftMode = VkbShiftMode {
    layer_count: 3,
    physical_button_count: 42,
    logical_button_count: 128,
};

// ─── LED control protocol ─────────────────────────────────────────────────────

/// LED index for VKB devices that support programmable indicator LEDs.
///
/// Gladiator NXT EVO and Gunfighter grips have 1–2 programmable LEDs.
/// LED commands are sent via HID feature reports (report ID 0x09 on most firmware).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VkbLedIndex {
    /// Primary indicator LED (base LED on Gladiator, or grip LED on MCG).
    Primary,
    /// Secondary indicator LED (present on some grips).
    Secondary,
}

/// LED colour for VKB devices with RGB capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VkbLedColor {
    /// Red channel (0–255).
    pub r: u8,
    /// Green channel (0–255).
    pub g: u8,
    /// Blue channel (0–255).
    pub b: u8,
}

impl VkbLedColor {
    /// Create a new LED colour.
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Off (all channels zero).
    pub const OFF: Self = Self::new(0, 0, 0);
    /// Red.
    pub const RED: Self = Self::new(255, 0, 0);
    /// Green.
    pub const GREEN: Self = Self::new(0, 255, 0);
    /// Blue.
    pub const BLUE: Self = Self::new(0, 0, 255);
}

/// LED command report ID used by VKB firmware.
///
/// **ASSUMED** — not captured from hardware.  The VKB firmware family uses
/// report ID 0x09 for LED feature reports on most devices.
pub const VKB_LED_REPORT_ID: u8 = 0x09;

/// Build a VKB LED control feature report.
///
/// The returned byte array can be sent as an HID feature report to the device.
///
/// **Layout (ASSUMED):**
/// ```text
/// byte 0: report_id (0x09)
/// byte 1: LED index (0 = primary, 1 = secondary)
/// byte 2: red channel
/// byte 3: green channel
/// byte 4: blue channel
/// byte 5: brightness (0–255, 0 = off)
/// ```
///
/// Returns a 6-byte feature report.
pub fn build_led_command(led: VkbLedIndex, color: VkbLedColor, brightness: u8) -> [u8; 6] {
    let led_idx = match led {
        VkbLedIndex::Primary => 0u8,
        VkbLedIndex::Secondary => 1u8,
    };
    [
        VKB_LED_REPORT_ID,
        led_idx,
        color.r,
        color.g,
        color.b,
        brightness,
    ]
}

// ─── Device identification helpers ────────────────────────────────────────────

/// Check whether a VID/PID pair belongs to a known VKB joystick-class device.
pub fn is_vkb_joystick(vid: u16, pid: u16) -> bool {
    vid == VKB_VENDOR_ID && VkbDeviceFamily::from_pid(pid).is_some()
}

/// Return the VKB device family for a VID/PID pair, or `None`.
pub fn vkb_device_family(vid: u16, pid: u16) -> Option<VkbDeviceFamily> {
    if vid != VKB_VENDOR_ID {
        return None;
    }
    VkbDeviceFamily::from_pid(pid)
}

// ─── SEM THQ report parsing ──────────────────────────────────────────────────

/// Parsed axes from one VKB SEM THQ report.
///
/// The SEM THQ exposes 4 analogue axes: two throttle levers and two rotary encoders
/// (analogue pots, not detented).  All axes are unidirectional: `0.0..=1.0`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SemThqAxes {
    /// Left throttle lever, `0.0..=1.0`.
    pub throttle_left: f32,
    /// Right throttle lever, `0.0..=1.0`.
    pub throttle_right: f32,
    /// Left rotary, `0.0..=1.0`.
    pub rotary_left: f32,
    /// Right rotary, `0.0..=1.0`.
    pub rotary_right: f32,
}

/// Parsed state from one VKB SEM THQ HID report.
#[derive(Debug, Clone, PartialEq)]
pub struct SemThqInputState {
    /// Normalised axes.
    pub axes: SemThqAxes,
    /// Up to 64 digital buttons (`true` = pressed).
    pub buttons: [bool; 64],
}

impl SemThqInputState {
    /// Return 1-based indices of all currently pressed buttons.
    pub fn pressed_buttons(&self) -> Vec<u16> {
        self.buttons
            .iter()
            .enumerate()
            .filter_map(|(i, &pressed)| if pressed { Some((i + 1) as u16) } else { None })
            .collect()
    }
}

/// Parse errors for SEM THQ reports.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SemThqParseError {
    /// Report payload is shorter than the minimum required size.
    #[error("SEM THQ report too short: expected at least {expected} bytes, got {actual}")]
    ReportTooShort { expected: usize, actual: usize },
}

/// Parser for VKB SEM THQ HID reports.
///
/// ## Expected HID report layout (ASSUMED)
///
/// | Bytes | Content |
/// |-------|---------|
/// | 0–1   | Left throttle (u16 LE, 0..65535 → 0.0..=1.0) |
/// | 2–3   | Right throttle (u16 LE) |
/// | 4–5   | Left rotary (u16 LE) |
/// | 6–7   | Right rotary (u16 LE) |
/// | 8–11  | Button bitmap word 0 (u32 LE, bits 0–31) |
/// | 12–15 | Button bitmap word 1 (u32 LE, bits 32–63) |
#[derive(Debug, Clone, Copy)]
pub struct SemThqInputHandler {
    has_report_id: bool,
}

impl SemThqInputHandler {
    /// Create a new SEM THQ parser.
    pub fn new() -> Self {
        Self {
            has_report_id: false,
        }
    }

    /// Enable stripping a 1-byte HID report ID prefix.
    pub fn with_report_id(mut self, enabled: bool) -> Self {
        self.has_report_id = enabled;
        self
    }

    /// Parse one SEM THQ HID report.
    pub fn parse_report(&self, report: &[u8]) -> Result<SemThqInputState, SemThqParseError> {
        let payload = if self.has_report_id {
            report.get(1..).unwrap_or(&[])
        } else {
            report
        };

        const MIN_LEN: usize = 16;
        if payload.len() < MIN_LEN {
            return Err(SemThqParseError::ReportTooShort {
                expected: MIN_LEN,
                actual: payload.len(),
            });
        }

        let axes = SemThqAxes {
            throttle_left: normalize_u16(le_u16(payload, 0)),
            throttle_right: normalize_u16(le_u16(payload, 2)),
            rotary_left: normalize_u16(le_u16(payload, 4)),
            rotary_right: normalize_u16(le_u16(payload, 6)),
        };

        let mut buttons = [false; 64];
        let btn_lo = le_u32(payload, 8);
        for (bit, btn) in buttons[..32].iter_mut().enumerate() {
            *btn = ((btn_lo >> bit) & 1) != 0;
        }
        let btn_hi = le_u32(payload, 12);
        for (bit, btn) in buttons[32..64].iter_mut().enumerate() {
            *btn = ((btn_hi >> bit) & 1) != 0;
        }

        Ok(SemThqInputState { axes, buttons })
    }
}

impl Default for SemThqInputHandler {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Gunfighter / MCG report parsing ──────────────────────────────────────────

/// Parsed axes from one VKB Gunfighter + grip report.
///
/// The Gunfighter base shares the same axis layout as the Gladiator NXT EVO
/// (roll, pitch, yaw/twist, mini-stick XY, throttle wheel) but MCG grips
/// add analog mini-sticks and triggers that map to Rx/Ry/Rz/Slider channels.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct GunfighterAxes {
    /// Main stick roll (X axis), `−1.0..=1.0`.
    pub roll: f32,
    /// Main stick pitch (Y axis), `−1.0..=1.0`.
    pub pitch: f32,
    /// Stick twist / yaw (Z axis), `−1.0..=1.0`.
    pub yaw: f32,
    /// Throttle wheel on base, `0.0..=1.0`.
    pub throttle: f32,
    /// Mini-stick analogue X axis, `−1.0..=1.0`.
    pub mini_x: f32,
    /// Mini-stick analogue Y axis, `−1.0..=1.0`.
    pub mini_y: f32,
}

/// Gunfighter device variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GunfighterVariant {
    /// Modern Combat Pro (Gunfighter base + MCG Pro grip).
    ModernCombatPro,
    /// Space Gunfighter (right-hand or unspecified).
    SpaceGunfighter,
    /// Space Gunfighter Left.
    SpaceGunfighterLeft,
}

impl GunfighterVariant {
    /// Human-readable product name.
    pub fn name(self) -> &'static str {
        match self {
            Self::ModernCombatPro => "VKB Gunfighter Modern Combat Pro",
            Self::SpaceGunfighter => "VKB Space Gunfighter",
            Self::SpaceGunfighterLeft => "VKB Space Gunfighter Left",
        }
    }

    /// Resolve variant from USB product ID.
    pub fn from_pid(pid: u16) -> Option<Self> {
        match pid {
            VKB_GUNFIGHTER_MODERN_COMBAT_PRO_PID => Some(Self::ModernCombatPro),
            VKB_SPACE_GUNFIGHTER_PID => Some(Self::SpaceGunfighter),
            VKB_SPACE_GUNFIGHTER_LEFT_PID => Some(Self::SpaceGunfighterLeft),
            _ => None,
        }
    }
}

/// Maximum button count for Gunfighter-class devices (2× u32 words).
pub const GUNFIGHTER_MAX_BUTTONS: usize = 64;
/// Maximum hat count for Gunfighter-class devices.
pub const GUNFIGHTER_MAX_HATS: usize = 2;

/// Parsed state from one VKB Gunfighter HID report.
#[derive(Debug, Clone, PartialEq)]
pub struct GunfighterInputState {
    /// Device variant.
    pub variant: GunfighterVariant,
    /// All six analogue axes.
    pub axes: GunfighterAxes,
    /// Up to 64 digital buttons (`true` = pressed).
    pub buttons: [bool; GUNFIGHTER_MAX_BUTTONS],
    /// POV hat states (`None` = centred).
    pub hats: [Option<u8>; GUNFIGHTER_MAX_HATS],
}

impl GunfighterInputState {
    fn new(variant: GunfighterVariant) -> Self {
        Self {
            variant,
            axes: GunfighterAxes::default(),
            buttons: [false; GUNFIGHTER_MAX_BUTTONS],
            hats: [None; GUNFIGHTER_MAX_HATS],
        }
    }

    /// Return 1-based indices of all currently pressed buttons.
    pub fn pressed_buttons(&self) -> Vec<u16> {
        self.buttons
            .iter()
            .enumerate()
            .filter_map(|(i, &pressed)| if pressed { Some((i + 1) as u16) } else { None })
            .collect()
    }
}

/// Parse errors for Gunfighter reports.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GunfighterParseError {
    /// Report payload is shorter than the minimum required size.
    #[error("Gunfighter report too short: expected at least {expected} bytes, got {actual}")]
    ReportTooShort { expected: usize, actual: usize },
}

/// Parser for VKB Gunfighter-class HID reports.
///
/// Uses the same 21-byte layout as the Gladiator NXT EVO (see [`VKB_JOYSTICK_STANDARD_LAYOUT`]).
#[derive(Debug, Clone, Copy)]
pub struct GunfighterInputHandler {
    variant: GunfighterVariant,
    has_report_id: bool,
}

impl GunfighterInputHandler {
    /// Create a parser for the given Gunfighter variant.
    pub fn new(variant: GunfighterVariant) -> Self {
        Self {
            variant,
            has_report_id: false,
        }
    }

    /// Enable stripping a 1-byte HID report ID prefix.
    pub fn with_report_id(mut self, enabled: bool) -> Self {
        self.has_report_id = enabled;
        self
    }

    /// Return the associated variant.
    pub fn variant(&self) -> GunfighterVariant {
        self.variant
    }

    /// Parse one Gunfighter HID report.
    pub fn parse_report(
        &self,
        report: &[u8],
    ) -> Result<GunfighterInputState, GunfighterParseError> {
        let payload = if self.has_report_id {
            report.get(1..).unwrap_or(&[])
        } else {
            report
        };

        const MIN_LEN: usize = 12;
        if payload.len() < MIN_LEN {
            return Err(GunfighterParseError::ReportTooShort {
                expected: MIN_LEN,
                actual: payload.len(),
            });
        }

        let mut state = GunfighterInputState::new(self.variant);

        state.axes.roll = normalize_signed(le_u16(payload, 0));
        state.axes.pitch = normalize_signed(le_u16(payload, 2));
        state.axes.yaw = normalize_signed(le_u16(payload, 4));
        state.axes.mini_x = normalize_signed(le_u16(payload, 6));
        state.axes.mini_y = normalize_signed(le_u16(payload, 8));
        state.axes.throttle = normalize_u16(le_u16(payload, 10));

        if payload.len() >= 16 {
            let btn_lo = le_u32(payload, 12);
            for bit in 0..32usize {
                state.buttons[bit] = ((btn_lo >> bit) & 1) != 0;
            }
        }
        if payload.len() >= 20 {
            let btn_hi = le_u32(payload, 16);
            for bit in 0..32usize {
                state.buttons[32 + bit] = ((btn_hi >> bit) & 1) != 0;
            }
        }

        if let Some(&hat_byte) = payload.get(20) {
            state.hats[0] = decode_hat_nibble(hat_byte & 0x0F);
            state.hats[1] = decode_hat_nibble((hat_byte >> 4) & 0x0F);
        }

        Ok(state)
    }
}

// ─── Shared helpers ───────────────────────────────────────────────────────────

/// Decode a 4-bit HID hat-switch nibble.
/// Values 0–7 map to N/NE/E/SE/S/SW/W/NW; 0xF (15) means centred.
fn decode_hat_nibble(nibble: u8) -> Option<u8> {
    if nibble <= 7 { Some(nibble) } else { None }
}

pub(crate) fn le_u16(bytes: &[u8], offset: usize) -> u16 {
    let low = bytes.get(offset).copied().unwrap_or(0);
    let high = bytes.get(offset + 1).copied().unwrap_or(0);
    u16::from_le_bytes([low, high])
}

fn le_u32(bytes: &[u8], offset: usize) -> u32 {
    let b0 = bytes.get(offset).copied().unwrap_or(0);
    let b1 = bytes.get(offset + 1).copied().unwrap_or(0);
    let b2 = bytes.get(offset + 2).copied().unwrap_or(0);
    let b3 = bytes.get(offset + 3).copied().unwrap_or(0);
    u32::from_le_bytes([b0, b1, b2, b3])
}

/// Normalise a raw u16 axis value to `0.0..=1.0` (unidirectional).
pub(crate) fn normalize_u16(raw: u16) -> f32 {
    (raw as f32 / u16::MAX as f32).clamp(0.0, 1.0)
}

/// Normalise a raw u16 axis value to `−1.0..=1.0` (bidirectional).
///
/// 0x0000 → −1.0, 0x8000 → ~0.0, 0xFFFF → ~1.0
pub(crate) fn normalize_signed(raw: u16) -> f32 {
    ((raw as f32 / 32767.5) - 1.0).clamp(-1.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── Device family classification ─────────────────────────────────────

    #[test]
    fn family_from_pid_gladiator_nxt_evo() {
        assert_eq!(
            VkbDeviceFamily::from_pid(VKB_GLADIATOR_NXT_EVO_RIGHT_PID),
            Some(VkbDeviceFamily::GladiatorNxtEvo)
        );
        assert_eq!(
            VkbDeviceFamily::from_pid(VKB_GLADIATOR_NXT_EVO_LEFT_PID),
            Some(VkbDeviceFamily::GladiatorNxtEvo)
        );
    }

    #[test]
    fn family_from_pid_gunfighter() {
        assert_eq!(
            VkbDeviceFamily::from_pid(VKB_GUNFIGHTER_MODERN_COMBAT_PRO_PID),
            Some(VkbDeviceFamily::Gunfighter)
        );
        assert_eq!(
            VkbDeviceFamily::from_pid(VKB_SPACE_GUNFIGHTER_PID),
            Some(VkbDeviceFamily::Gunfighter)
        );
        assert_eq!(
            VkbDeviceFamily::from_pid(VKB_SPACE_GUNFIGHTER_LEFT_PID),
            Some(VkbDeviceFamily::Gunfighter)
        );
    }

    #[test]
    fn family_from_pid_sem_thq() {
        assert_eq!(
            VkbDeviceFamily::from_pid(VKB_NXT_SEM_THQ_PID),
            Some(VkbDeviceFamily::SemThq)
        );
    }

    #[test]
    fn family_from_pid_gladiator_mk2() {
        assert_eq!(
            VkbDeviceFamily::from_pid(VKB_GLADIATOR_MK2_PID),
            Some(VkbDeviceFamily::GladiatorMk2)
        );
    }

    #[test]
    fn family_from_pid_t_rudder() {
        assert_eq!(
            VkbDeviceFamily::from_pid(VKB_T_RUDDER_MK5_PID),
            Some(VkbDeviceFamily::TRudder)
        );
    }

    #[test]
    fn family_from_pid_stecs_modern_throttle() {
        assert_eq!(
            VkbDeviceFamily::from_pid(VKB_STECS_MODERN_THROTTLE_MINI_PID),
            Some(VkbDeviceFamily::StecsModernThrottle)
        );
        assert_eq!(
            VkbDeviceFamily::from_pid(VKB_STECS_MODERN_THROTTLE_MAX_PID),
            Some(VkbDeviceFamily::StecsModernThrottle)
        );
    }

    #[test]
    fn family_from_pid_unknown() {
        assert_eq!(VkbDeviceFamily::from_pid(0x9999), None);
    }

    #[test]
    fn is_vkb_joystick_known_device() {
        assert!(is_vkb_joystick(
            VKB_VENDOR_ID,
            VKB_GLADIATOR_NXT_EVO_RIGHT_PID
        ));
        assert!(is_vkb_joystick(
            VKB_VENDOR_ID,
            VKB_GUNFIGHTER_MODERN_COMBAT_PRO_PID
        ));
    }

    #[test]
    fn is_vkb_joystick_wrong_vid() {
        assert!(!is_vkb_joystick(0x0000, VKB_GLADIATOR_NXT_EVO_RIGHT_PID));
    }

    #[test]
    fn is_vkb_joystick_unknown_pid() {
        assert!(!is_vkb_joystick(VKB_VENDOR_ID, 0x9999));
    }

    // ─── Report layout ────────────────────────────────────────────────────

    #[test]
    fn standard_layout_dimensions() {
        let layout = VKB_JOYSTICK_STANDARD_LAYOUT;
        assert_eq!(layout.axis_count, 6);
        assert_eq!(layout.button_word_count, 2);
        assert!(layout.has_hat_byte);
        assert_eq!(layout.min_payload_bytes, 21);
    }

    #[test]
    fn sem_thq_layout_dimensions() {
        let layout = VKB_SEM_THQ_LAYOUT;
        assert_eq!(layout.axis_count, 4);
        assert_eq!(layout.button_word_count, 2);
        assert!(!layout.has_hat_byte);
        assert_eq!(layout.min_payload_bytes, 16);
    }

    #[test]
    fn report_layout_for_family_returns_correct_layout() {
        assert_eq!(
            report_layout_for_family(VkbDeviceFamily::GladiatorNxtEvo),
            VKB_JOYSTICK_STANDARD_LAYOUT
        );
        assert_eq!(
            report_layout_for_family(VkbDeviceFamily::Gunfighter),
            VKB_JOYSTICK_STANDARD_LAYOUT
        );
        assert_eq!(
            report_layout_for_family(VkbDeviceFamily::SemThq),
            VKB_SEM_THQ_LAYOUT
        );
        assert_eq!(
            report_layout_for_family(VkbDeviceFamily::TRudder),
            VKB_T_RUDDER_LAYOUT
        );
    }

    // ─── T-Rudder layout ──────────────────────────────────────────────────

    #[test]
    fn t_rudder_layout_dimensions() {
        let layout = VKB_T_RUDDER_LAYOUT;
        assert_eq!(layout.axis_count, 3);
        assert_eq!(layout.button_word_count, 0);
        assert!(!layout.has_hat_byte);
        assert_eq!(layout.min_payload_bytes, 6);
    }

    // ─── VKB Device Info Table ────────────────────────────────────────────

    #[test]
    fn device_table_has_expected_entries() {
        assert!(VKB_DEVICE_TABLE.len() >= 12, "expected ≥12 devices");
    }

    #[test]
    fn device_info_lookup_gladiator_nxt_evo() {
        let info = vkb_device_info(VKB_GLADIATOR_NXT_EVO_RIGHT_PID).unwrap();
        assert_eq!(info.family, VkbDeviceFamily::GladiatorNxtEvo);
        assert_eq!(info.axis_count, 6);
        assert_eq!(info.hat_count, 2);
    }

    #[test]
    fn device_info_lookup_t_rudder() {
        let info = vkb_device_info(VKB_T_RUDDER_MK5_PID).unwrap();
        assert_eq!(info.family, VkbDeviceFamily::TRudder);
        assert_eq!(info.axis_count, 3);
        assert_eq!(info.max_buttons, 0);
        assert_eq!(info.hat_count, 0);
    }

    #[test]
    fn device_info_unknown_pid_is_none() {
        assert!(vkb_device_info(0x9999).is_none());
    }

    #[test]
    fn all_table_entries_have_unique_pids() {
        let mut pids: Vec<u16> = VKB_DEVICE_TABLE.iter().map(|d| d.pid).collect();
        pids.sort();
        pids.dedup();
        assert_eq!(
            pids.len(),
            VKB_DEVICE_TABLE.len(),
            "duplicate PIDs in table"
        );
    }

    // ─── Axis resolution ──────────────────────────────────────────────────

    #[test]
    fn axis_resolution_16bit() {
        assert_eq!(VKB_AXIS_16BIT.bits, 16);
        assert_eq!(VKB_AXIS_16BIT.logical_min, 0);
        assert_eq!(VKB_AXIS_16BIT.logical_max, 0xFFFF);
    }

    // ─── Shift mode ───────────────────────────────────────────────────────

    #[test]
    fn gladiator_shift_model() {
        const { assert!(GLADIATOR_NXT_EVO_SHIFT.layer_count == 2) };
        const {
            assert!(
                GLADIATOR_NXT_EVO_SHIFT.logical_button_count
                    >= GLADIATOR_NXT_EVO_SHIFT.physical_button_count
            )
        };
    }

    #[test]
    fn gunfighter_mcg_shift_model() {
        const { assert!(GUNFIGHTER_MCG_SHIFT.layer_count == 3) };
        const {
            assert!(
                GUNFIGHTER_MCG_SHIFT.logical_button_count
                    >= GUNFIGHTER_MCG_SHIFT.physical_button_count
            )
        };
    }

    // ─── LED commands ─────────────────────────────────────────────────────

    #[test]
    fn led_command_primary_red() {
        let cmd = build_led_command(VkbLedIndex::Primary, VkbLedColor::RED, 128);
        assert_eq!(cmd[0], VKB_LED_REPORT_ID);
        assert_eq!(cmd[1], 0); // primary
        assert_eq!(cmd[2], 255); // r
        assert_eq!(cmd[3], 0); // g
        assert_eq!(cmd[4], 0); // b
        assert_eq!(cmd[5], 128); // brightness
    }

    #[test]
    fn led_command_secondary_green() {
        let cmd = build_led_command(VkbLedIndex::Secondary, VkbLedColor::GREEN, 255);
        assert_eq!(cmd[1], 1); // secondary
        assert_eq!(cmd[2], 0);
        assert_eq!(cmd[3], 255);
        assert_eq!(cmd[4], 0);
    }

    #[test]
    fn led_command_off() {
        let cmd = build_led_command(VkbLedIndex::Primary, VkbLedColor::OFF, 0);
        assert_eq!(cmd[2..5], [0, 0, 0]);
        assert_eq!(cmd[5], 0);
    }

    #[test]
    fn led_command_custom_color() {
        let color = VkbLedColor::new(100, 150, 200);
        let cmd = build_led_command(VkbLedIndex::Primary, color, 64);
        assert_eq!(cmd[2], 100);
        assert_eq!(cmd[3], 150);
        assert_eq!(cmd[4], 200);
        assert_eq!(cmd[5], 64);
    }

    // ─── GunfighterVariant ────────────────────────────────────────────────

    #[test]
    fn gunfighter_variant_from_pid() {
        assert_eq!(
            GunfighterVariant::from_pid(VKB_GUNFIGHTER_MODERN_COMBAT_PRO_PID),
            Some(GunfighterVariant::ModernCombatPro)
        );
        assert_eq!(
            GunfighterVariant::from_pid(VKB_SPACE_GUNFIGHTER_PID),
            Some(GunfighterVariant::SpaceGunfighter)
        );
        assert_eq!(
            GunfighterVariant::from_pid(VKB_SPACE_GUNFIGHTER_LEFT_PID),
            Some(GunfighterVariant::SpaceGunfighterLeft)
        );
        assert_eq!(GunfighterVariant::from_pid(0x9999), None);
    }

    #[test]
    fn gunfighter_variant_names() {
        assert!(!GunfighterVariant::ModernCombatPro.name().is_empty());
        assert!(!GunfighterVariant::SpaceGunfighter.name().is_empty());
        assert!(!GunfighterVariant::SpaceGunfighterLeft.name().is_empty());
    }

    // ─── Gunfighter report parsing ────────────────────────────────────────

    fn make_gunfighter_report(axes: [u16; 6], btn_lo: u32, btn_hi: u32, hat_byte: u8) -> Vec<u8> {
        let mut report = vec![0u8; 21];
        for (i, &v) in axes.iter().enumerate() {
            let bytes = v.to_le_bytes();
            report[i * 2] = bytes[0];
            report[i * 2 + 1] = bytes[1];
        }
        report[12..16].copy_from_slice(&btn_lo.to_le_bytes());
        report[16..20].copy_from_slice(&btn_hi.to_le_bytes());
        report[20] = hat_byte;
        report
    }

    #[test]
    fn gunfighter_report_too_short() {
        let handler = GunfighterInputHandler::new(GunfighterVariant::ModernCombatPro);
        let err = handler.parse_report(&[0u8; 10]);
        assert!(matches!(
            err,
            Err(GunfighterParseError::ReportTooShort {
                expected: 12,
                actual: 10
            })
        ));
    }

    #[test]
    fn gunfighter_centre_axes_normalise_to_zero() {
        let handler = GunfighterInputHandler::new(GunfighterVariant::ModernCombatPro);
        let report =
            make_gunfighter_report([0x8000, 0x8000, 0x8000, 0x8000, 0x8000, 0x0000], 0, 0, 0xFF);
        let state = handler.parse_report(&report).unwrap();
        assert!(state.axes.roll.abs() < 0.01);
        assert!(state.axes.pitch.abs() < 0.01);
        assert!(state.axes.yaw.abs() < 0.01);
        assert!(state.axes.mini_x.abs() < 0.01);
        assert!(state.axes.mini_y.abs() < 0.01);
    }

    #[test]
    fn gunfighter_full_deflection_axes() {
        let handler = GunfighterInputHandler::new(GunfighterVariant::SpaceGunfighter);
        let report =
            make_gunfighter_report([0xFFFF, 0x0000, 0xFFFF, 0x8000, 0x8000, 0xFFFF], 0, 0, 0xFF);
        let state = handler.parse_report(&report).unwrap();
        assert!((state.axes.roll - 1.0).abs() < 0.01);
        assert!((state.axes.pitch - (-1.0)).abs() < 0.01);
        assert!((state.axes.yaw - 1.0).abs() < 0.01);
        assert!((state.axes.throttle - 1.0).abs() < 0.001);
    }

    #[test]
    fn gunfighter_buttons_parsed() {
        let handler = GunfighterInputHandler::new(GunfighterVariant::ModernCombatPro);
        let report = make_gunfighter_report(
            [0x8000; 6],
            0x8000_0001, // buttons 1 and 32
            0x0000_0004, // button 35
            0xFF,
        );
        let state = handler.parse_report(&report).unwrap();
        assert!(state.buttons[0], "button 1");
        assert!(state.buttons[31], "button 32");
        assert!(state.buttons[34], "button 35");
        assert_eq!(state.pressed_buttons(), vec![1, 32, 35]);
    }

    #[test]
    fn gunfighter_hat_decoded() {
        let handler = GunfighterInputHandler::new(GunfighterVariant::SpaceGunfighterLeft);
        let report = make_gunfighter_report([0x8000; 6], 0, 0, 0xF2); // hat0=E(2), hat1=centred(F)
        let state = handler.parse_report(&report).unwrap();
        assert_eq!(state.hats[0], Some(2)); // East
        assert_eq!(state.hats[1], None);
    }

    #[test]
    fn gunfighter_with_report_id() {
        let handler =
            GunfighterInputHandler::new(GunfighterVariant::ModernCombatPro).with_report_id(true);
        let mut report = vec![0x01u8];
        report.extend_from_slice(&make_gunfighter_report(
            [0x8000, 0x8000, 0x8000, 0x8000, 0x8000, 0xFFFF],
            0,
            0,
            0xFF,
        ));
        let state = handler.parse_report(&report).unwrap();
        assert!((state.axes.throttle - 1.0).abs() < 0.001);
    }

    // ─── SEM THQ report parsing ───────────────────────────────────────────

    fn make_sem_thq_report(axes: [u16; 4], btn_lo: u32, btn_hi: u32) -> Vec<u8> {
        let mut report = vec![0u8; 16];
        for (i, &v) in axes.iter().enumerate() {
            let bytes = v.to_le_bytes();
            report[i * 2] = bytes[0];
            report[i * 2 + 1] = bytes[1];
        }
        report[8..12].copy_from_slice(&btn_lo.to_le_bytes());
        report[12..16].copy_from_slice(&btn_hi.to_le_bytes());
        report
    }

    #[test]
    fn sem_thq_report_too_short() {
        let handler = SemThqInputHandler::new();
        let err = handler.parse_report(&[0u8; 14]);
        assert!(matches!(
            err,
            Err(SemThqParseError::ReportTooShort {
                expected: 16,
                actual: 14
            })
        ));
    }

    #[test]
    fn sem_thq_idle_position() {
        let handler = SemThqInputHandler::new();
        let report = make_sem_thq_report([0, 0, 0, 0], 0, 0);
        let state = handler.parse_report(&report).unwrap();
        assert_eq!(state.axes.throttle_left, 0.0);
        assert_eq!(state.axes.throttle_right, 0.0);
        assert_eq!(state.axes.rotary_left, 0.0);
        assert_eq!(state.axes.rotary_right, 0.0);
        assert!(state.pressed_buttons().is_empty());
    }

    #[test]
    fn sem_thq_full_throttle() {
        let handler = SemThqInputHandler::new();
        let report = make_sem_thq_report([0xFFFF, 0xFFFF, 0x8000, 0x8000], 0, 0);
        let state = handler.parse_report(&report).unwrap();
        assert!((state.axes.throttle_left - 1.0).abs() < 0.001);
        assert!((state.axes.throttle_right - 1.0).abs() < 0.001);
        assert!((state.axes.rotary_left - 0.5).abs() < 0.01);
    }

    #[test]
    fn sem_thq_buttons_parsed() {
        let handler = SemThqInputHandler::new();
        let report = make_sem_thq_report([0; 4], 0x0000_0005, 0x0000_0001);
        let state = handler.parse_report(&report).unwrap();
        assert!(state.buttons[0], "button 1");
        assert!(state.buttons[2], "button 3");
        assert!(state.buttons[32], "button 33");
        assert_eq!(state.pressed_buttons(), vec![1, 3, 33]);
    }

    #[test]
    fn sem_thq_with_report_id() {
        let handler = SemThqInputHandler::new().with_report_id(true);
        let mut report = vec![0x01u8];
        report.extend_from_slice(&make_sem_thq_report([0xFFFF; 4], 0, 0));
        let state = handler.parse_report(&report).unwrap();
        assert!((state.axes.throttle_left - 1.0).abs() < 0.001);
    }

    // ─── Normalisation helpers ────────────────────────────────────────────

    #[test]
    fn normalize_u16_endpoints() {
        assert_eq!(normalize_u16(0), 0.0);
        assert!((normalize_u16(u16::MAX) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn normalize_signed_endpoints() {
        assert!((normalize_signed(0) - (-1.0)).abs() < 0.01);
        assert!(normalize_signed(0x8000).abs() < 0.01);
        assert!((normalize_signed(0xFFFF) - 1.0).abs() < 0.01);
    }

    #[test]
    fn normalize_u16_midpoint() {
        let mid = normalize_u16(0x8000);
        assert!((mid - 0.5).abs() < 0.01);
    }
}
