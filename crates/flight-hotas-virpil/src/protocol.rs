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
}

// ─── Unified report dispatcher ────────────────────────────────────────────────

use crate::{
    VpcAcePedalsInputState, VpcAcePedalsParseError, VpcAceTorqInputState, VpcAceTorqParseError,
    VpcAlphaInputState, VpcAlphaParseError, VpcCm3ParseError, VpcCm3ThrottleInputState,
    VpcMongoostInputState, VpcMongoostParseError, VpcPanel1InputState, VpcPanel1ParseError,
    VpcPanel2InputState, VpcPanel2ParseError, VpcRotorTcsInputState, VpcRotorTcsParseError,
    VpcWarBrdInputState, VpcWarBrdParseError, WarBrdVariant, parse_ace_pedals_report,
    parse_ace_torq_report, parse_alpha_report, parse_cm3_throttle_report,
    parse_mongoost_stick_report, parse_panel1_report, parse_panel2_report, parse_rotor_tcs_report,
    parse_warbrd_report,
};

/// Error from the unified report dispatcher.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchError {
    /// The Product ID is not in the known VIRPIL device table.
    UnknownPid(u16),
    /// Alpha stick parser error.
    Alpha(VpcAlphaParseError),
    /// MongoosT-50CM3 stick parser error.
    Mongoost(VpcMongoostParseError),
    /// WarBRD base parser error.
    WarBrd(VpcWarBrdParseError),
    /// CM3 Throttle parser error.
    Cm3Throttle(VpcCm3ParseError),
    /// Control Panel 1 parser error.
    Panel1(VpcPanel1ParseError),
    /// Control Panel 2 parser error.
    Panel2(VpcPanel2ParseError),
    /// ACE Pedals parser error.
    AcePedals(VpcAcePedalsParseError),
    /// ACE Torq parser error.
    AceTorq(VpcAceTorqParseError),
    /// Rotor TCS Plus parser error.
    RotorTcs(VpcRotorTcsParseError),
}

impl core::fmt::Display for DispatchError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnknownPid(pid) => write!(f, "unknown VIRPIL PID: 0x{pid:04X}"),
            Self::Alpha(e) => write!(f, "{e}"),
            Self::Mongoost(e) => write!(f, "{e}"),
            Self::WarBrd(e) => write!(f, "{e}"),
            Self::Cm3Throttle(e) => write!(f, "{e}"),
            Self::Panel1(e) => write!(f, "{e}"),
            Self::Panel2(e) => write!(f, "{e}"),
            Self::AcePedals(e) => write!(f, "{e}"),
            Self::AceTorq(e) => write!(f, "{e}"),
            Self::RotorTcs(e) => write!(f, "{e}"),
        }
    }
}

/// Parsed input state from any VIRPIL VPC device.
///
/// The dispatcher returns one of these variants based on the USB PID.
#[derive(Debug, Clone, PartialEq)]
pub enum VirpilInputState {
    Alpha(VpcAlphaInputState),
    Mongoost(VpcMongoostInputState),
    WarBrd(VpcWarBrdInputState),
    Cm3Throttle(VpcCm3ThrottleInputState),
    Panel1(VpcPanel1InputState),
    Panel2(VpcPanel2InputState),
    AcePedals(VpcAcePedalsInputState),
    AceTorq(VpcAceTorqInputState),
    RotorTcs(VpcRotorTcsInputState),
}

/// Route a raw HID report to the correct VIRPIL device parser based on PID.
///
/// This is the primary entry point when the caller knows the USB PID but
/// doesn't want to match on device family manually.
///
/// Alpha Prime grips are routed to the Alpha parser (identical protocol).
pub fn dispatch_report(pid: u16, data: &[u8]) -> Result<VirpilInputState, DispatchError> {
    match pid {
        VIRPIL_CONSTELLATION_ALPHA_LEFT_PID
        | VIRPIL_CONSTELLATION_ALPHA_PRIME_LEFT_PID
        | VIRPIL_CONSTELLATION_ALPHA_PRIME_RIGHT_PID => parse_alpha_report(data)
            .map(VirpilInputState::Alpha)
            .map_err(DispatchError::Alpha),

        VIRPIL_MONGOOST_STICK_PID => parse_mongoost_stick_report(data)
            .map(VirpilInputState::Mongoost)
            .map_err(DispatchError::Mongoost),

        VIRPIL_WARBRD_PID => parse_warbrd_report(data, WarBrdVariant::Original)
            .map(VirpilInputState::WarBrd)
            .map_err(DispatchError::WarBrd),

        VIRPIL_WARBRD_D_PID => parse_warbrd_report(data, WarBrdVariant::D)
            .map(VirpilInputState::WarBrd)
            .map_err(DispatchError::WarBrd),

        VIRPIL_CM3_THROTTLE_PID => parse_cm3_throttle_report(data)
            .map(VirpilInputState::Cm3Throttle)
            .map_err(DispatchError::Cm3Throttle),

        VIRPIL_PANEL1_PID => parse_panel1_report(data)
            .map(VirpilInputState::Panel1)
            .map_err(DispatchError::Panel1),

        VIRPIL_PANEL2_PID => parse_panel2_report(data)
            .map(VirpilInputState::Panel2)
            .map_err(DispatchError::Panel2),

        VIRPIL_ACE_PEDALS_PID => parse_ace_pedals_report(data)
            .map(VirpilInputState::AcePedals)
            .map_err(DispatchError::AcePedals),

        VIRPIL_ACE_TORQ_PID => parse_ace_torq_report(data)
            .map(VirpilInputState::AceTorq)
            .map_err(DispatchError::AceTorq),

        VIRPIL_ROTOR_TCS_PLUS_PID => parse_rotor_tcs_report(data)
            .map(VirpilInputState::RotorTcs)
            .map_err(DispatchError::RotorTcs),

        _ => Err(DispatchError::UnknownPid(pid)),
    }
}

#[cfg(test)]
mod dispatch_tests {
    use super::*;

    fn make_5ax_report(axes: [u16; 5], buttons: [u8; 4]) -> Vec<u8> {
        let mut data = vec![0x01u8];
        for ax in &axes {
            data.extend_from_slice(&ax.to_le_bytes());
        }
        data.extend_from_slice(&buttons);
        data
    }

    fn make_cm3_report(axes: [u16; 6], buttons: [u8; 10]) -> Vec<u8> {
        let mut data = vec![0x01u8];
        for ax in &axes {
            data.extend_from_slice(&ax.to_le_bytes());
        }
        data.extend_from_slice(&buttons);
        data
    }

    fn make_pedals_report(axes: [u16; 3], buttons: [u8; 2]) -> Vec<u8> {
        let mut data = vec![0x01u8];
        for ax in &axes {
            data.extend_from_slice(&ax.to_le_bytes());
        }
        data.extend_from_slice(&buttons);
        data
    }

    fn make_torq_report(throttle: u16, buttons: [u8; 2]) -> Vec<u8> {
        let mut data = vec![0x01u8];
        data.extend_from_slice(&throttle.to_le_bytes());
        data.extend_from_slice(&buttons);
        data
    }

    #[test]
    fn dispatch_alpha_left() {
        let r = make_5ax_report([8192; 5], [0u8; 4]);
        let state = dispatch_report(VIRPIL_CONSTELLATION_ALPHA_LEFT_PID, &r).unwrap();
        assert!(matches!(state, VirpilInputState::Alpha(_)));
    }

    #[test]
    fn dispatch_alpha_prime_left() {
        let r = make_5ax_report([0; 5], [0u8; 4]);
        let state = dispatch_report(VIRPIL_CONSTELLATION_ALPHA_PRIME_LEFT_PID, &r).unwrap();
        assert!(matches!(state, VirpilInputState::Alpha(_)));
    }

    #[test]
    fn dispatch_mongoost() {
        let r = make_5ax_report([0; 5], [0u8; 4]);
        let state = dispatch_report(VIRPIL_MONGOOST_STICK_PID, &r).unwrap();
        assert!(matches!(state, VirpilInputState::Mongoost(_)));
    }

    #[test]
    fn dispatch_warbrd_original() {
        let r = make_5ax_report([0; 5], [0u8; 4]);
        let state = dispatch_report(VIRPIL_WARBRD_PID, &r).unwrap();
        match state {
            VirpilInputState::WarBrd(s) => {
                assert_eq!(s.variant, WarBrdVariant::Original);
            }
            _ => panic!("expected WarBrd variant"),
        }
    }

    #[test]
    fn dispatch_warbrd_d() {
        let r = make_5ax_report([0; 5], [0u8; 4]);
        let state = dispatch_report(VIRPIL_WARBRD_D_PID, &r).unwrap();
        match state {
            VirpilInputState::WarBrd(s) => {
                assert_eq!(s.variant, WarBrdVariant::D);
            }
            _ => panic!("expected WarBrd-D variant"),
        }
    }

    #[test]
    fn dispatch_cm3_throttle() {
        let r = make_cm3_report([0; 6], [0u8; 10]);
        let state = dispatch_report(VIRPIL_CM3_THROTTLE_PID, &r).unwrap();
        assert!(matches!(state, VirpilInputState::Cm3Throttle(_)));
    }

    #[test]
    fn dispatch_panel1() {
        let mut r = vec![0x01u8];
        r.extend_from_slice(&[0u8; 6]);
        let state = dispatch_report(VIRPIL_PANEL1_PID, &r).unwrap();
        assert!(matches!(state, VirpilInputState::Panel1(_)));
    }

    #[test]
    fn dispatch_panel2() {
        let mut r = vec![0x01u8];
        r.extend_from_slice(&[0u8; 10]);
        let state = dispatch_report(VIRPIL_PANEL2_PID, &r).unwrap();
        assert!(matches!(state, VirpilInputState::Panel2(_)));
    }

    #[test]
    fn dispatch_ace_pedals() {
        let r = make_pedals_report([8192; 3], [0u8; 2]);
        let state = dispatch_report(VIRPIL_ACE_PEDALS_PID, &r).unwrap();
        assert!(matches!(state, VirpilInputState::AcePedals(_)));
    }

    #[test]
    fn dispatch_ace_torq() {
        let r = make_torq_report(8192, [0u8; 2]);
        let state = dispatch_report(VIRPIL_ACE_TORQ_PID, &r).unwrap();
        assert!(matches!(state, VirpilInputState::AceTorq(_)));
    }

    #[test]
    fn dispatch_rotor_tcs() {
        let mut r = vec![0x01u8];
        for _ in 0..3 {
            r.extend_from_slice(&8192u16.to_le_bytes());
        }
        r.extend_from_slice(&[0u8; 4]);
        let state = dispatch_report(VIRPIL_ROTOR_TCS_PLUS_PID, &r).unwrap();
        assert!(matches!(state, VirpilInputState::RotorTcs(_)));
    }

    #[test]
    fn dispatch_unknown_pid() {
        let r = vec![0x01u8; 23];
        let err = dispatch_report(0xFFFF, &r).unwrap_err();
        assert!(matches!(err, DispatchError::UnknownPid(0xFFFF)));
    }

    #[test]
    fn dispatch_error_display() {
        let err = DispatchError::UnknownPid(0xBEEF);
        assert!(err.to_string().contains("0xBEEF"));
    }

    #[test]
    fn dispatch_too_short_report() {
        let r = vec![0x01u8; 2];
        let err = dispatch_report(VIRPIL_CM3_THROTTLE_PID, &r).unwrap_err();
        assert!(matches!(err, DispatchError::Cm3Throttle(_)));
    }
}
