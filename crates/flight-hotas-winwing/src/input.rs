// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID report parsing for WinWing HOTAS devices.
//!
//! WinWing uses VID **0x4098** across all products.  Each product has a
//! distinct PID and report layout.  This module covers:
//!
//! - **Orion 2 Throttle** (PID 0xBE62) — dual throttle + 3 sliders, 50 buttons, 5 encoders
//! - **Orion 2 F/A-18C Stick** (PID 0xBE63) — 20 buttons, 2 axes (X/Y), 2 hats
//! - **TFRP Rudder Pedals** (PID 0xBE64) — toe brake left/right + rudder axis

/// USB Vendor ID for all WinWing products.
pub const WINWING_VENDOR_ID: u16 = 0x4098;

/// WinWing product IDs.
pub const ORION2_THROTTLE_PID: u16 = 0xBE62;
pub const ORION2_F18_STICK_PID: u16 = 0xBE63;
pub const TFRP_RUDDER_PID: u16 = 0xBE64;

/// Report length for the Orion 2 Throttle (bytes including report ID).
pub const THROTTLE_REPORT_LEN: usize = 24;
/// Report length for the Orion 2 Stick.
pub const STICK_REPORT_LEN: usize = 12;
/// Report length for the TFRP Rudder Pedals.
pub const RUDDER_REPORT_LEN: usize = 8;

// ─── Orion 2 Throttle ────────────────────────────────────────────────────────

/// Axis snapshot for the Orion 2 Throttle.
#[derive(Debug, Clone, PartialEq)]
pub struct ThrottleAxes {
    /// Left throttle — \[0.0, 1.0\].
    pub throttle_left: f32,
    /// Right throttle — \[0.0, 1.0\].
    pub throttle_right: f32,
    /// Combined throttle (average of left and right) — \[0.0, 1.0\].
    pub throttle_combined: f32,
    /// Friction slider — \[0.0, 1.0\].
    pub friction: f32,
    /// Mouse stick X — \[-1.0, 1.0\].
    pub mouse_x: f32,
    /// Mouse stick Y — \[-1.0, 1.0\].
    pub mouse_y: f32,
}

/// Button state for the Orion 2 Throttle (50 buttons + 5 encoders).
#[derive(Debug, Clone, Default)]
pub struct ThrottleButtons {
    /// 64-bit bitmask covering buttons 1–50.
    pub mask: u64,
    /// Encoder detent deltas (positive = CW, negative = CCW), 5 encoders.
    pub encoders: [i8; 5],
}

impl ThrottleButtons {
    pub fn is_pressed(&self, n: u8) -> bool {
        (1u8..=50).contains(&n) && (self.mask >> (n - 1)) & 1 == 1
    }
}

/// Parsed state from a single Orion 2 Throttle HID report.
#[derive(Debug, Clone)]
pub struct ThrottleInputState {
    pub axes: ThrottleAxes,
    pub buttons: ThrottleButtons,
}

/// Parse a raw HID report from the Orion 2 Throttle.
pub fn parse_throttle_report(data: &[u8]) -> Result<ThrottleInputState, WinWingParseError> {
    if data.len() < THROTTLE_REPORT_LEN {
        return Err(WinWingParseError::TooShort {
            expected: THROTTLE_REPORT_LEN,
            got: data.len(),
        });
    }
    if data[0] != 0x01 {
        return Err(WinWingParseError::UnknownReportId { id: data[0] });
    }

    // Layout: [ID, TL_lo, TL_hi, TR_lo, TR_hi, FR_lo, FR_hi, MX_lo, MX_hi, MY_lo, MY_hi,
    //          B0..B7 (8 bytes = 64 bits), E0..E4 (5 bytes)]
    let tl = read_u16(data, 1) as f32 / 65535.0;
    let tr = read_u16(data, 3) as f32 / 65535.0;
    let friction = read_u16(data, 5) as f32 / 65535.0;
    let mouse_x = norm_i16_u16(read_u16(data, 7));
    let mouse_y = norm_i16_u16(read_u16(data, 9));

    let mask = u64::from_le_bytes(data[11..19].try_into().unwrap());
    let encoders = [
        data[19] as i8,
        data[20] as i8,
        data[21] as i8,
        data[22] as i8,
        data[23] as i8,
    ];

    Ok(ThrottleInputState {
        axes: ThrottleAxes {
            throttle_left: tl,
            throttle_right: tr,
            throttle_combined: (tl + tr) * 0.5,
            friction,
            mouse_x,
            mouse_y,
        },
        buttons: ThrottleButtons { mask, encoders },
    })
}

// ─── Orion 2 Stick ────────────────────────────────────────────────────────────

/// Axis snapshot for the Orion 2 F/A-18C Stick.
#[derive(Debug, Clone, PartialEq)]
pub struct StickAxes {
    /// Roll — \[-1.0, 1.0\].
    pub roll: f32,
    /// Pitch — \[-1.0, 1.0\].
    pub pitch: f32,
}

/// Button state for the Orion 2 Stick (20 buttons + 2 HATs).
#[derive(Debug, Clone, Default)]
pub struct StickButtons {
    pub mask: u32,
    pub hat_a: u8,
    pub hat_b: u8,
}

impl StickButtons {
    pub fn is_pressed(&self, n: u8) -> bool {
        (1u8..=20).contains(&n) && (self.mask >> (n - 1)) & 1 == 1
    }
}

/// Parsed state from a single Orion 2 Stick HID report.
#[derive(Debug, Clone)]
pub struct StickInputState {
    pub axes: StickAxes,
    pub buttons: StickButtons,
}

/// Parse a raw HID report from the Orion 2 Stick.
pub fn parse_stick_report(data: &[u8]) -> Result<StickInputState, WinWingParseError> {
    if data.len() < STICK_REPORT_LEN {
        return Err(WinWingParseError::TooShort {
            expected: STICK_REPORT_LEN,
            got: data.len(),
        });
    }
    if data[0] != 0x02 {
        return Err(WinWingParseError::UnknownReportId { id: data[0] });
    }

    let roll = norm_i16_val(i16::from_le_bytes([data[1], data[2]]));
    let pitch = norm_i16_val(i16::from_le_bytes([data[3], data[4]]));
    let mask = u32::from_le_bytes([data[5], data[6], data[7], data[8]]);
    let hat_a = data[9];
    let hat_b = data[10];

    Ok(StickInputState {
        axes: StickAxes { roll, pitch },
        buttons: StickButtons { mask, hat_a, hat_b },
    })
}

// ─── TFRP Rudder Pedals ───────────────────────────────────────────────────────

/// Axis snapshot for the TFRP Rudder Pedals.
#[derive(Debug, Clone, PartialEq)]
pub struct RudderAxes {
    /// Rudder axis — \[-1.0, 1.0\].
    pub rudder: f32,
    /// Left toe brake — \[0.0, 1.0\].
    pub brake_left: f32,
    /// Right toe brake — \[0.0, 1.0\].
    pub brake_right: f32,
}

/// Parse a raw HID report from the TFRP Rudder Pedals.
pub fn parse_rudder_report(data: &[u8]) -> Result<RudderAxes, WinWingParseError> {
    if data.len() < RUDDER_REPORT_LEN {
        return Err(WinWingParseError::TooShort {
            expected: RUDDER_REPORT_LEN,
            got: data.len(),
        });
    }
    if data[0] != 0x03 {
        return Err(WinWingParseError::UnknownReportId { id: data[0] });
    }

    let rudder = norm_i16_val(i16::from_le_bytes([data[1], data[2]]));
    let brake_left = read_u16(data, 3) as f32 / 65535.0;
    let brake_right = read_u16(data, 5) as f32 / 65535.0;

    Ok(RudderAxes {
        rudder,
        brake_left,
        brake_right,
    })
}

// ─── Error type ──────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum WinWingParseError {
    #[error("report too short: expected ≥{expected} bytes, got {got}")]
    TooShort { expected: usize, got: usize },
    #[error("unknown report ID: 0x{id:02X}")]
    UnknownReportId { id: u8 },
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn read_u16(data: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([data[offset], data[offset + 1]])
}

fn norm_i16_u16(v: u16) -> f32 {
    // Treat unsigned u16 as signed relative to midpoint 32768
    let signed = v.wrapping_sub(32768) as i16;
    norm_i16_val(signed)
}

fn norm_i16_val(v: i16) -> f32 {
    v as f32 / 32767.0
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Throttle ──────────────────────────────────────────────────────────

    fn throttle_report(tl: u16, tr: u16) -> [u8; THROTTLE_REPORT_LEN] {
        let mut r = [0u8; THROTTLE_REPORT_LEN];
        r[0] = 0x01;
        r[1..3].copy_from_slice(&tl.to_le_bytes());
        r[3..5].copy_from_slice(&tr.to_le_bytes());
        r
    }

    #[test]
    fn test_throttle_min_position() {
        let state = parse_throttle_report(&throttle_report(0, 0)).unwrap();
        assert!(state.axes.throttle_left < 0.001);
        assert!(state.axes.throttle_right < 0.001);
        assert!(state.axes.throttle_combined < 0.001);
    }

    #[test]
    fn test_throttle_max_position() {
        let state = parse_throttle_report(&throttle_report(0xFFFF, 0xFFFF)).unwrap();
        assert!((state.axes.throttle_left - 1.0).abs() < 1e-4);
        assert!((state.axes.throttle_combined - 1.0).abs() < 1e-4);
    }

    #[test]
    fn test_throttle_combined_is_average() {
        let state = parse_throttle_report(&throttle_report(0xFFFF, 0)).unwrap();
        assert!((state.axes.throttle_combined - 0.5).abs() < 1e-3);
    }

    #[test]
    fn test_throttle_button_detection() {
        let mut r = throttle_report(0, 0);
        r[11] = 0b0000_1001; // buttons 1 and 4
        let state = parse_throttle_report(&r).unwrap();
        assert!(state.buttons.is_pressed(1));
        assert!(!state.buttons.is_pressed(2));
        assert!(state.buttons.is_pressed(4));
    }

    #[test]
    fn test_throttle_report_too_short() {
        assert!(parse_throttle_report(&[0u8; 4]).is_err());
    }

    // ── Stick ─────────────────────────────────────────────────────────────

    fn stick_report(roll: i16, pitch: i16) -> [u8; STICK_REPORT_LEN] {
        let mut r = [0u8; STICK_REPORT_LEN];
        r[0] = 0x02;
        r[1..3].copy_from_slice(&roll.to_le_bytes());
        r[3..5].copy_from_slice(&pitch.to_le_bytes());
        r
    }

    #[test]
    fn test_stick_centred() {
        let state = parse_stick_report(&stick_report(0, 0)).unwrap();
        assert!(state.axes.roll.abs() < 1e-4);
        assert!(state.axes.pitch.abs() < 1e-4);
    }

    #[test]
    fn test_stick_full_roll() {
        let state = parse_stick_report(&stick_report(32767, 0)).unwrap();
        assert!((state.axes.roll - 1.0).abs() < 1e-4);
    }

    // ── Rudder ────────────────────────────────────────────────────────────

    fn rudder_report(rudder: i16, bl: u16, br: u16) -> [u8; RUDDER_REPORT_LEN] {
        let mut r = [0u8; RUDDER_REPORT_LEN];
        r[0] = 0x03;
        r[1..3].copy_from_slice(&rudder.to_le_bytes());
        r[3..5].copy_from_slice(&bl.to_le_bytes());
        r[5..7].copy_from_slice(&br.to_le_bytes());
        r
    }

    #[test]
    fn test_rudder_centred() {
        let axes = parse_rudder_report(&rudder_report(0, 0, 0)).unwrap();
        assert!(axes.rudder.abs() < 1e-4);
        assert!(axes.brake_left < 0.001);
        assert!(axes.brake_right < 0.001);
    }

    #[test]
    fn test_rudder_full_deflection() {
        let axes = parse_rudder_report(&rudder_report(32767, 0xFFFF, 0xFFFF)).unwrap();
        assert!((axes.rudder - 1.0).abs() < 1e-4);
        assert!((axes.brake_left - 1.0).abs() < 1e-4);
    }

    #[test]
    fn test_rudder_report_too_short() {
        assert!(parse_rudder_report(&[0u8; 4]).is_err());
    }
}
