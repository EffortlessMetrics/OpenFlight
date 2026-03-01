// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! VIRPIL Controls VPC HID protocol definitions and utilities.
//!
//! This module consolidates protocol-level knowledge about VIRPIL VPC devices:
//!
//! - **VID/PID identification table** for all known Virpil products
//! - **HID report format** constants and axis resolution
//! - **LED control** via HID feature reports
//! - **Unified report dispatcher** that routes raw HID data to the correct parser
//!
//! # HID Report Format (all VIRPIL devices)
//!
//! Every VIRPIL HID input report follows a common frame:
//!
//! ```text
//! byte  0         : report_id (always 0x01 for the primary usage=4 interface)
//! bytes 1..=2n    : axis values (u16 LE, one per axis)
//! bytes 2n+1..end : button bytes (1 byte per 8 buttons, LSB-first)
//! ```
//!
//! Axis resolution: 14-bit (0–16384). See [`VIRPIL_AXIS_MAX`].
//!
//! # LED Feature Reports
//!
//! VIRPIL devices with LED backlighting accept HID feature reports on the
//! LED control interface (usage page 0x0C, usage 0x01). The report format:
//!
//! ```text
//! byte 0    : report_id (0x02 for LED control)
//! byte 1    : LED index (0-based)
//! byte 2    : red channel   (0x00–0xFF)
//! byte 3    : green channel (0x00–0xFF)
//! byte 4    : blue channel  (0x00–0xFF)
//! ```
//!
//! LED indices and counts are device-specific.

use thiserror::Error;

use crate::VIRPIL_AXIS_MAX;
pub use flight_hid_support::device_support::{
    VIRPIL_ACE_PEDALS_PID, VIRPIL_ACE_TORQ_PID, VIRPIL_CM3_THROTTLE_PID,
    VIRPIL_CONSTELLATION_ALPHA_LEFT_PID, VIRPIL_CONSTELLATION_ALPHA_PRIME_LEFT_PID,
    VIRPIL_CONSTELLATION_ALPHA_PRIME_RIGHT_PID, VIRPIL_MONGOOST_STICK_PID, VIRPIL_PANEL1_PID,
    VIRPIL_PANEL2_PID, VIRPIL_ROTOR_TCS_PLUS_PID, VIRPIL_VENDOR_ID, VIRPIL_WARBRD_D_PID,
    VIRPIL_WARBRD_PID, VirpilModel, is_virpil_device, virpil_model,
};

// ─── Axis resolution ──────────────────────────────────────────────────────────

/// Maximum raw axis value for all VIRPIL VPC devices (14-bit resolution).
///
/// Raw axis values range from 0 to 16384 inclusive. This is the divisor for
/// normalising raw u16 values to the `[0.0, 1.0]` floating-point range.
pub const AXIS_MAX: u16 = VIRPIL_AXIS_MAX;

/// Number of effective bits of axis resolution across VIRPIL VPC devices.
pub const AXIS_RESOLUTION_BITS: u8 = 14;

/// Normalise a raw VIRPIL axis value to the `[0.0, 1.0]` range.
///
/// Values above [`AXIS_MAX`] are clamped to 1.0.
#[inline]
pub fn normalize_axis(raw: u16) -> f32 {
    (raw as f32 / AXIS_MAX as f32).clamp(0.0, 1.0)
}

/// Convert a normalised `[0.0, 1.0]` value back to a raw axis value.
///
/// The input is clamped to `[0.0, 1.0]` before conversion.
#[inline]
pub fn denormalize_axis(normalised: f32) -> u16 {
    (normalised.clamp(0.0, 1.0) * AXIS_MAX as f32).round() as u16
}

// ─── HID Report IDs ──────────────────────────────────────────────────────────

/// Standard report ID for VIRPIL VPC input reports.
pub const INPUT_REPORT_ID: u8 = 0x01;

/// Report ID used for LED control feature reports.
pub const LED_REPORT_ID: u8 = 0x02;

// ─── LED control ──────────────────────────────────────────────────────────────

/// Size of one LED control feature report in bytes.
pub const LED_REPORT_SIZE: usize = 5;

/// An RGB colour value for an LED.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LedColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl LedColor {
    /// Create a new LED colour.
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Predefined: LED off (all channels zero).
    pub const OFF: Self = Self::new(0, 0, 0);
    /// Predefined: full-brightness red.
    pub const RED: Self = Self::new(0xFF, 0, 0);
    /// Predefined: full-brightness green.
    pub const GREEN: Self = Self::new(0, 0xFF, 0);
    /// Predefined: full-brightness blue.
    pub const BLUE: Self = Self::new(0, 0, 0xFF);
    /// Predefined: full-brightness white.
    pub const WHITE: Self = Self::new(0xFF, 0xFF, 0xFF);
}

/// Build a raw HID feature report to set the colour of a single LED.
///
/// The returned 5-byte buffer can be sent to the device via
/// `hid_device.send_feature_report(&buf)`.
///
/// # Arguments
///
/// * `led_index` — zero-based index of the LED to control.
/// * `color` — the RGB colour to set.
pub fn build_led_report(led_index: u8, color: LedColor) -> [u8; LED_REPORT_SIZE] {
    [LED_REPORT_ID, led_index, color.r, color.g, color.b]
}

// ─── VID/PID device table ─────────────────────────────────────────────────────

/// Complete VID/PID entry for a VIRPIL VPC device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VirpilDeviceInfo {
    /// USB Product ID.
    pub pid: u16,
    /// Device model enum variant.
    pub model: VirpilModel,
    /// Human-readable product name.
    pub name: &'static str,
    /// Minimum HID input report size in bytes (including report_id).
    pub min_report_bytes: usize,
    /// Number of analogue axes in the input report.
    pub axis_count: u8,
    /// Number of discrete buttons.
    pub button_count: u8,
}

/// Complete table of all known VIRPIL VPC devices with their parameters.
pub const DEVICE_TABLE: &[VirpilDeviceInfo] = &[
    VirpilDeviceInfo {
        pid: VIRPIL_MONGOOST_STICK_PID,
        model: VirpilModel::MongoostStick,
        name: "VPC MongoosT-50CM3 Stick",
        min_report_bytes: 15,
        axis_count: 5,
        button_count: 28,
    },
    VirpilDeviceInfo {
        pid: VIRPIL_CONSTELLATION_ALPHA_LEFT_PID,
        model: VirpilModel::ConstellationAlphaLeft,
        name: "VPC Constellation Alpha Left (CM3)",
        min_report_bytes: 15,
        axis_count: 5,
        button_count: 28,
    },
    VirpilDeviceInfo {
        pid: VIRPIL_CONSTELLATION_ALPHA_PRIME_LEFT_PID,
        model: VirpilModel::ConstellationAlphaPrimeLeft,
        name: "VPC Constellation Alpha Prime Left",
        min_report_bytes: 15,
        axis_count: 5,
        button_count: 28,
    },
    VirpilDeviceInfo {
        pid: VIRPIL_CONSTELLATION_ALPHA_PRIME_RIGHT_PID,
        model: VirpilModel::ConstellationAlphaPrimeRight,
        name: "VPC Constellation Alpha Prime Right",
        min_report_bytes: 15,
        axis_count: 5,
        button_count: 28,
    },
    VirpilDeviceInfo {
        pid: VIRPIL_WARBRD_PID,
        model: VirpilModel::WarBrd,
        name: "VPC WarBRD Stick",
        min_report_bytes: 15,
        axis_count: 5,
        button_count: 28,
    },
    VirpilDeviceInfo {
        pid: VIRPIL_WARBRD_D_PID,
        model: VirpilModel::WarBrdD,
        name: "VPC WarBRD-D Stick",
        min_report_bytes: 15,
        axis_count: 5,
        button_count: 28,
    },
    VirpilDeviceInfo {
        pid: VIRPIL_CM3_THROTTLE_PID,
        model: VirpilModel::Cm3Throttle,
        name: "VPC Throttle CM3",
        min_report_bytes: 23,
        axis_count: 6,
        button_count: 78,
    },
    VirpilDeviceInfo {
        pid: VIRPIL_PANEL1_PID,
        model: VirpilModel::ControlPanel1,
        name: "VPC Control Panel 1",
        min_report_bytes: 7,
        axis_count: 0,
        button_count: 48,
    },
    VirpilDeviceInfo {
        pid: VIRPIL_PANEL2_PID,
        model: VirpilModel::ControlPanel2,
        name: "VPC Control Panel 2",
        min_report_bytes: 11,
        axis_count: 2,
        button_count: 47,
    },
    VirpilDeviceInfo {
        pid: VIRPIL_ACE_TORQ_PID,
        model: VirpilModel::AceTorq,
        name: "VPC ACE Torq",
        min_report_bytes: 5,
        axis_count: 1,
        button_count: 8,
    },
    VirpilDeviceInfo {
        pid: VIRPIL_ACE_PEDALS_PID,
        model: VirpilModel::AcePedals,
        name: "VPC ACE Collection Pedals",
        min_report_bytes: 9,
        axis_count: 3,
        button_count: 16,
    },
    VirpilDeviceInfo {
        pid: VIRPIL_ROTOR_TCS_PLUS_PID,
        model: VirpilModel::RotorTcsPlus,
        name: "VPC Rotor TCS Plus",
        min_report_bytes: 11,
        axis_count: 3,
        button_count: 24,
    },
];

/// Look up a [`VirpilDeviceInfo`] entry by USB Product ID.
///
/// Returns `None` for unknown PIDs.
pub fn device_info(pid: u16) -> Option<&'static VirpilDeviceInfo> {
    DEVICE_TABLE.iter().find(|d| d.pid == pid)
}

// ─── Device family ────────────────────────────────────────────────────────────

/// High-level grouping of VIRPIL VPC devices by function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VirpilDeviceFamily {
    /// Grip/stick devices: Alpha, Alpha Prime, MongoosT.
    Grip,
    /// Base/gimbal devices: WarBRD, WarBRD-D.
    Base,
    /// Throttle devices: CM3 Throttle, CM2 Throttle.
    Throttle,
    /// Pedal devices: ACE Collection Pedals.
    Pedals,
    /// Panel devices: Control Panel 1, Control Panel 2, Shark Panel.
    Panel,
    /// Collective/helicopter devices: Rotor TCS Plus, ACE Torq.
    Collective,
}

impl VirpilDeviceFamily {
    /// Determine the device family for a given [`VirpilModel`].
    pub fn from_model(model: VirpilModel) -> Self {
        match model {
            VirpilModel::ConstellationAlphaLeft
            | VirpilModel::ConstellationAlphaPrimeLeft
            | VirpilModel::ConstellationAlphaPrimeRight
            | VirpilModel::MongoostStick
            | VirpilModel::Cm2Stick => VirpilDeviceFamily::Grip,
            VirpilModel::WarBrd | VirpilModel::WarBrdD => VirpilDeviceFamily::Base,
            VirpilModel::Cm3Throttle | VirpilModel::Cm2Throttle => VirpilDeviceFamily::Throttle,
            VirpilModel::AcePedals => VirpilDeviceFamily::Pedals,
            VirpilModel::ControlPanel1 | VirpilModel::ControlPanel2 | VirpilModel::SharkPanel => {
                VirpilDeviceFamily::Panel
            }
            VirpilModel::RotorTcsPlus | VirpilModel::AceTorq => VirpilDeviceFamily::Collective,
        }
    }
}

// ─── VirpilProtocol — unified device handle ───────────────────────────────────

/// A protocol handle for a detected VIRPIL VPC device.
///
/// Created from a USB Product ID via [`VirpilProtocol::from_pid`]. Provides
/// access to the device's model, family, and static info.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VirpilProtocol {
    model: VirpilModel,
    info: &'static VirpilDeviceInfo,
}

impl VirpilProtocol {
    /// Detect a VIRPIL device from its USB Product ID.
    ///
    /// Returns `None` for unrecognised PIDs.
    pub fn from_pid(pid: u16) -> Option<Self> {
        let info = device_info(pid)?;
        Some(Self {
            model: info.model,
            info,
        })
    }

    /// The detected device model.
    pub fn model(&self) -> VirpilModel {
        self.model
    }

    /// The device family (grip, base, throttle, etc.).
    pub fn family(&self) -> VirpilDeviceFamily {
        VirpilDeviceFamily::from_model(self.model)
    }

    /// Static device info (axis count, button count, min report size, etc.).
    pub fn info(&self) -> &'static VirpilDeviceInfo {
        self.info
    }

    /// Human-readable product name.
    pub fn name(&self) -> &'static str {
        self.info.name
    }
}

// ─── Unified report types ─────────────────────────────────────────────────────

/// Error from the unified report parsers.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum VirpilParseError {
    #[error("report too short: got {0} bytes")]
    TooShort(usize),
}

/// Unified grip/stick state: normalised axes + button bitmap.
#[derive(Debug, Clone, PartialEq)]
pub struct GripState {
    /// Normalised axis values `[0.0, 1.0]`. 5 axes (X, Y, Z, SZ, SL).
    pub axes: [f32; GRIP_AXIS_COUNT],
    /// Raw button bytes (LSB-first per byte).
    pub buttons_raw: [u8; GRIP_BUTTON_BYTES],
}

impl GripState {
    /// Return `true` if button `n` (1-indexed) is pressed.
    pub fn is_pressed(&self, n: u8) -> bool {
        if n == 0 {
            return false;
        }
        let idx = (n - 1) as usize;
        let byte = idx / 8;
        let bit = idx % 8;
        self.buttons_raw
            .get(byte)
            .is_some_and(|b| (b >> bit) & 1 == 1)
    }
}

/// Unified base/gimbal state: normalised axes + button bitmap.
#[derive(Debug, Clone, PartialEq)]
pub struct BaseState {
    /// Normalised axis values `[0.0, 1.0]`. 5 axes (X, Y, Z, SZ, SL).
    pub axes: [f32; BASE_AXIS_COUNT],
    /// Raw button bytes (LSB-first per byte).
    pub buttons_raw: [u8; BASE_BUTTON_BYTES],
}

/// Unified throttle state: normalised axes + button bitmap.
#[derive(Debug, Clone, PartialEq)]
pub struct ThrottleState {
    /// Normalised axis values `[0.0, 1.0]`. 6 axes.
    pub axes: [f32; THROTTLE_AXIS_COUNT],
    /// Raw button bytes (LSB-first per byte).
    pub buttons_raw: [u8; THROTTLE_BUTTON_BYTES],
}

// ─── Unified parse functions ──────────────────────────────────────────────────

const GRIP_AXIS_COUNT: usize = 5;
const GRIP_BUTTON_BYTES: usize = 4;
const GRIP_MIN_BYTES: usize = 1 + GRIP_AXIS_COUNT * 2 + GRIP_BUTTON_BYTES; // 15

const BASE_AXIS_COUNT: usize = 5;
const BASE_BUTTON_BYTES: usize = 4;
const BASE_MIN_BYTES: usize = 1 + BASE_AXIS_COUNT * 2 + BASE_BUTTON_BYTES; // 15

const THROTTLE_AXIS_COUNT: usize = 6;
const THROTTLE_BUTTON_BYTES: usize = 10;
const THROTTLE_MIN_BYTES: usize = 1 + THROTTLE_AXIS_COUNT * 2 + THROTTLE_BUTTON_BYTES; // 23

/// Parse a generic grip/stick HID report into [`GripState`].
///
/// Compatible with Alpha, Alpha Prime, MongoosT-50CM3 grips (15-byte reports).
pub fn parse_grip_report(data: &[u8]) -> Result<GripState, VirpilParseError> {
    if data.len() < GRIP_MIN_BYTES {
        return Err(VirpilParseError::TooShort(data.len()));
    }
    let payload = &data[1..];
    let mut axes = [0.0f32; GRIP_AXIS_COUNT];
    for i in 0..GRIP_AXIS_COUNT {
        let raw = u16::from_le_bytes([payload[i * 2], payload[i * 2 + 1]]);
        axes[i] = normalize_axis(raw);
    }
    let btn_start = 1 + GRIP_AXIS_COUNT * 2;
    let mut buttons_raw = [0u8; GRIP_BUTTON_BYTES];
    buttons_raw.copy_from_slice(&data[btn_start..btn_start + GRIP_BUTTON_BYTES]);
    Ok(GripState { axes, buttons_raw })
}

/// Parse a generic base/gimbal HID report into [`BaseState`].
///
/// Compatible with WarBRD, WarBRD-D bases (15-byte reports, same format as grips).
pub fn parse_base_report(data: &[u8]) -> Result<BaseState, VirpilParseError> {
    if data.len() < BASE_MIN_BYTES {
        return Err(VirpilParseError::TooShort(data.len()));
    }
    let payload = &data[1..];
    let mut axes = [0.0f32; BASE_AXIS_COUNT];
    for i in 0..BASE_AXIS_COUNT {
        let raw = u16::from_le_bytes([payload[i * 2], payload[i * 2 + 1]]);
        axes[i] = normalize_axis(raw);
    }
    let btn_start = 1 + BASE_AXIS_COUNT * 2;
    let mut buttons_raw = [0u8; BASE_BUTTON_BYTES];
    buttons_raw.copy_from_slice(&data[btn_start..btn_start + BASE_BUTTON_BYTES]);
    Ok(BaseState { axes, buttons_raw })
}

/// Parse a generic throttle HID report into [`ThrottleState`].
///
/// Compatible with VPC Throttle CM3 (23-byte reports).
pub fn parse_throttle_report(data: &[u8]) -> Result<ThrottleState, VirpilParseError> {
    if data.len() < THROTTLE_MIN_BYTES {
        return Err(VirpilParseError::TooShort(data.len()));
    }
    let payload = &data[1..];
    let mut axes = [0.0f32; THROTTLE_AXIS_COUNT];
    for i in 0..THROTTLE_AXIS_COUNT {
        let raw = u16::from_le_bytes([payload[i * 2], payload[i * 2 + 1]]);
        axes[i] = normalize_axis(raw);
    }
    let btn_start = 1 + THROTTLE_AXIS_COUNT * 2;
    let mut buttons_raw = [0u8; THROTTLE_BUTTON_BYTES];
    buttons_raw.copy_from_slice(&data[btn_start..btn_start + THROTTLE_BUTTON_BYTES]);
    Ok(ThrottleState { axes, buttons_raw })
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Axis normalization ────────────────────────────────────────────────

    #[test]
    fn normalize_zero_is_zero() {
        assert_eq!(normalize_axis(0), 0.0);
    }

    #[test]
    fn normalize_max_is_one() {
        assert!((normalize_axis(AXIS_MAX) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn normalize_above_max_clamps_to_one() {
        assert_eq!(normalize_axis(u16::MAX), 1.0);
    }

    #[test]
    fn normalize_midpoint_is_half() {
        let mid = AXIS_MAX / 2;
        assert!((normalize_axis(mid) - 0.5).abs() < 0.01);
    }

    #[test]
    fn denormalize_zero_is_zero() {
        assert_eq!(denormalize_axis(0.0), 0);
    }

    #[test]
    fn denormalize_one_is_max() {
        assert_eq!(denormalize_axis(1.0), AXIS_MAX);
    }

    #[test]
    fn denormalize_clamps_negative() {
        assert_eq!(denormalize_axis(-1.0), 0);
    }

    #[test]
    fn denormalize_clamps_above_one() {
        assert_eq!(denormalize_axis(2.0), AXIS_MAX);
    }

    #[test]
    fn roundtrip_normalization() {
        for raw in [0u16, 1, 100, 8192, 16383, AXIS_MAX] {
            let norm = normalize_axis(raw);
            let back = denormalize_axis(norm);
            assert!(
                (raw as i32 - back as i32).unsigned_abs() <= 1,
                "roundtrip failed for {raw}: got {back}"
            );
        }
    }

    // ── LED report building ───────────────────────────────────────────────

    #[test]
    fn led_report_format() {
        let buf = build_led_report(3, LedColor::new(0xAA, 0xBB, 0xCC));
        assert_eq!(buf, [0x02, 3, 0xAA, 0xBB, 0xCC]);
    }

    #[test]
    fn led_report_off() {
        let buf = build_led_report(0, LedColor::OFF);
        assert_eq!(buf, [0x02, 0, 0, 0, 0]);
    }

    #[test]
    fn led_report_size_constant() {
        assert_eq!(LED_REPORT_SIZE, 5);
    }

    #[test]
    fn led_color_presets() {
        assert_eq!(LedColor::RED, LedColor::new(0xFF, 0, 0));
        assert_eq!(LedColor::GREEN, LedColor::new(0, 0xFF, 0));
        assert_eq!(LedColor::BLUE, LedColor::new(0, 0, 0xFF));
        assert_eq!(LedColor::WHITE, LedColor::new(0xFF, 0xFF, 0xFF));
    }

    // ── Device table ──────────────────────────────────────────────────────

    #[test]
    fn device_table_has_all_expected_entries() {
        assert!(DEVICE_TABLE.len() >= 12, "expected ≥12 devices");
    }

    #[test]
    fn device_info_lookup_cm3_throttle() {
        let info = device_info(VIRPIL_CM3_THROTTLE_PID).unwrap();
        assert_eq!(info.name, "VPC Throttle CM3");
        assert_eq!(info.axis_count, 6);
        assert_eq!(info.button_count, 78);
        assert_eq!(info.min_report_bytes, 23);
    }

    #[test]
    fn device_info_lookup_ace_pedals() {
        let info = device_info(VIRPIL_ACE_PEDALS_PID).unwrap();
        assert_eq!(info.name, "VPC ACE Collection Pedals");
        assert_eq!(info.axis_count, 3);
        assert_eq!(info.button_count, 16);
    }

    #[test]
    fn device_info_lookup_ace_torq() {
        let info = device_info(VIRPIL_ACE_TORQ_PID).unwrap();
        assert_eq!(info.name, "VPC ACE Torq");
        assert_eq!(info.axis_count, 1);
        assert_eq!(info.button_count, 8);
    }

    #[test]
    fn device_info_lookup_rotor_tcs() {
        let info = device_info(VIRPIL_ROTOR_TCS_PLUS_PID).unwrap();
        assert_eq!(info.name, "VPC Rotor TCS Plus");
        assert_eq!(info.axis_count, 3);
        assert_eq!(info.button_count, 24);
    }

    #[test]
    fn device_info_unknown_pid_is_none() {
        assert!(device_info(0xFFFF).is_none());
    }

    #[test]
    fn all_table_entries_have_unique_pids() {
        let mut pids: Vec<u16> = DEVICE_TABLE.iter().map(|d| d.pid).collect();
        pids.sort();
        pids.dedup();
        assert_eq!(pids.len(), DEVICE_TABLE.len(), "duplicate PIDs in table");
    }

    #[test]
    fn all_table_entries_have_nonzero_report_size() {
        for entry in DEVICE_TABLE {
            assert!(
                entry.min_report_bytes > 0,
                "{}: min_report_bytes must be > 0",
                entry.name
            );
        }
    }

    #[test]
    fn input_report_id_is_one() {
        assert_eq!(INPUT_REPORT_ID, 0x01);
    }

    #[test]
    fn axis_resolution_bits_matches_max() {
        assert_eq!(1u32 << AXIS_RESOLUTION_BITS, AXIS_MAX as u32);
    }

    // ── Device family ─────────────────────────────────────────────────────

    #[test]
    fn family_alpha_is_grip() {
        assert_eq!(
            VirpilDeviceFamily::from_model(VirpilModel::ConstellationAlphaLeft),
            VirpilDeviceFamily::Grip,
        );
    }

    #[test]
    fn family_alpha_prime_left_is_grip() {
        assert_eq!(
            VirpilDeviceFamily::from_model(VirpilModel::ConstellationAlphaPrimeLeft),
            VirpilDeviceFamily::Grip,
        );
    }

    #[test]
    fn family_mongoost_is_grip() {
        assert_eq!(
            VirpilDeviceFamily::from_model(VirpilModel::MongoostStick),
            VirpilDeviceFamily::Grip,
        );
    }

    #[test]
    fn family_warbrd_is_base() {
        assert_eq!(
            VirpilDeviceFamily::from_model(VirpilModel::WarBrd),
            VirpilDeviceFamily::Base,
        );
    }

    #[test]
    fn family_warbrd_d_is_base() {
        assert_eq!(
            VirpilDeviceFamily::from_model(VirpilModel::WarBrdD),
            VirpilDeviceFamily::Base,
        );
    }

    #[test]
    fn family_cm3_is_throttle() {
        assert_eq!(
            VirpilDeviceFamily::from_model(VirpilModel::Cm3Throttle),
            VirpilDeviceFamily::Throttle,
        );
    }

    #[test]
    fn family_ace_pedals_is_pedals() {
        assert_eq!(
            VirpilDeviceFamily::from_model(VirpilModel::AcePedals),
            VirpilDeviceFamily::Pedals,
        );
    }

    #[test]
    fn family_panel1_is_panel() {
        assert_eq!(
            VirpilDeviceFamily::from_model(VirpilModel::ControlPanel1),
            VirpilDeviceFamily::Panel,
        );
    }

    #[test]
    fn family_rotor_tcs_is_collective() {
        assert_eq!(
            VirpilDeviceFamily::from_model(VirpilModel::RotorTcsPlus),
            VirpilDeviceFamily::Collective,
        );
    }

    #[test]
    fn family_ace_torq_is_collective() {
        assert_eq!(
            VirpilDeviceFamily::from_model(VirpilModel::AceTorq),
            VirpilDeviceFamily::Collective,
        );
    }

    // ── VirpilProtocol ────────────────────────────────────────────────────

    #[test]
    fn protocol_detect_alpha() {
        let proto = VirpilProtocol::from_pid(VIRPIL_CONSTELLATION_ALPHA_LEFT_PID);
        assert!(proto.is_some());
        let proto = proto.unwrap();
        assert_eq!(proto.family(), VirpilDeviceFamily::Grip);
    }

    #[test]
    fn protocol_detect_throttle() {
        let proto = VirpilProtocol::from_pid(VIRPIL_CM3_THROTTLE_PID);
        assert!(proto.is_some());
        let proto = proto.unwrap();
        assert_eq!(proto.family(), VirpilDeviceFamily::Throttle);
    }

    #[test]
    fn protocol_detect_pedals() {
        let proto = VirpilProtocol::from_pid(VIRPIL_ACE_PEDALS_PID);
        assert!(proto.is_some());
        let proto = proto.unwrap();
        assert_eq!(proto.family(), VirpilDeviceFamily::Pedals);
    }

    #[test]
    fn protocol_detect_warbrd() {
        let proto = VirpilProtocol::from_pid(VIRPIL_WARBRD_PID);
        assert!(proto.is_some());
        let proto = proto.unwrap();
        assert_eq!(proto.family(), VirpilDeviceFamily::Base);
    }

    #[test]
    fn protocol_unknown_pid_is_none() {
        assert!(VirpilProtocol::from_pid(0xFFFF).is_none());
    }

    #[test]
    fn protocol_model_accessor() {
        let proto = VirpilProtocol::from_pid(VIRPIL_CM3_THROTTLE_PID).unwrap();
        assert_eq!(proto.model(), VirpilModel::Cm3Throttle);
    }

    #[test]
    fn protocol_info_accessor() {
        let proto = VirpilProtocol::from_pid(VIRPIL_ACE_PEDALS_PID).unwrap();
        assert_eq!(proto.info().axis_count, 3);
        assert_eq!(proto.info().button_count, 16);
    }

    // ── Unified parse dispatchers ─────────────────────────────────────────

    #[test]
    fn parse_grip_report_alpha_ok() {
        let mut data = vec![0x01u8]; // report_id
        for _ in 0..5 {
            data.extend_from_slice(&8192u16.to_le_bytes()); // mid-range axes
        }
        data.extend_from_slice(&[0u8; 4]); // 4 button bytes
        let state = parse_grip_report(&data).unwrap();
        assert!((state.axes[0] - 0.5).abs() < 0.01);
    }

    #[test]
    fn parse_grip_report_too_short() {
        assert!(parse_grip_report(&[0x01; 5]).is_err());
    }

    #[test]
    fn parse_base_report_ok() {
        let mut data = vec![0x01u8];
        for _ in 0..5 {
            data.extend_from_slice(&0u16.to_le_bytes());
        }
        data.extend_from_slice(&[0u8; 4]);
        let state = parse_base_report(&data).unwrap();
        assert_eq!(state.axes[0], 0.0);
    }

    #[test]
    fn parse_throttle_report_ok() {
        let mut data = vec![0x01u8];
        for _ in 0..6 {
            data.extend_from_slice(&AXIS_MAX.to_le_bytes());
        }
        data.extend_from_slice(&[0u8; 10]);
        let state = parse_throttle_report(&data).unwrap();
        assert!((state.axes[0] - 1.0).abs() < 1e-4);
    }

    #[test]
    fn parse_throttle_report_too_short() {
        assert!(parse_throttle_report(&[0x01; 10]).is_err());
    }

    #[test]
    fn grip_state_buttons() {
        let mut data = vec![0x01u8];
        for _ in 0..5 {
            data.extend_from_slice(&0u16.to_le_bytes());
        }
        // Set button 1 (bit 0 of first button byte)
        data.extend_from_slice(&[0x01, 0x00, 0x00, 0x00]);
        let state = parse_grip_report(&data).unwrap();
        assert!(state.is_pressed(1));
        assert!(!state.is_pressed(2));
    }
}
