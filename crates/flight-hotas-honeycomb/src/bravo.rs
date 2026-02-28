// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! HID input parsing for the Honeycomb Bravo Throttle Quadrant.
//!
//! # Report layout (estimated)
//!
//! The exact HID descriptor for the Bravo Throttle is not publicly documented.
//! This layout is inferred from the HID joystick specification and community
//! documentation. **Hardware validation required before production use.**
//!
//! ```text
//! Byte 0:       Report ID = 0x01
//! Bytes  1– 2:  Throttle 1  — u16 LE, 0–4095 (12-bit, unipolar)
//! Bytes  3– 4:  Throttle 2  — same
//! Bytes  5– 6:  Throttle 3  — same
//! Bytes  7– 8:  Throttle 4  — same
//! Bytes  9–10:  Throttle 5  — same
//! Bytes 11–12:  Flap lever  — same
//! Bytes 13–14:  Spoiler     — same
//! Bytes 15–22:  Button bitmask (8 bytes = 64 bits)
//! ```
//!
//! Axis resolution: 12-bit (0–4095), stored in 16-bit LE fields.
//! Bravo VID: 0x294B  PID: 0x1901 (confirmed)

/// Expected minimum report length in bytes.
pub const BRAVO_REPORT_LEN: usize = 23;

/// Axis values for the Bravo Throttle Quadrant, normalised to \[0.0, 1.0\].
#[derive(Debug, Clone, PartialEq)]
pub struct BravoAxes {
    /// Throttle lever 1 — \[0.0, 1.0\]; idle = 0.0, full = 1.0.
    pub throttle1: f32,
    /// Throttle lever 2 — \[0.0, 1.0\].
    pub throttle2: f32,
    /// Throttle lever 3 — \[0.0, 1.0\].
    pub throttle3: f32,
    /// Throttle lever 4 — \[0.0, 1.0\].
    pub throttle4: f32,
    /// Throttle lever 5 — \[0.0, 1.0\].
    pub throttle5: f32,
    /// Flap lever — \[0.0, 1.0\]; 0.0 = retracted, 1.0 = full.
    pub flap_lever: f32,
    /// Spoiler lever — \[0.0, 1.0\]; 0.0 = retracted, 1.0 = full.
    pub spoiler: f32,
}

/// Button state for the Bravo Throttle Quadrant (up to 64 buttons).
///
/// Known button assignments (0-indexed bit positions):
///
/// | Bit | Function |
/// |-----|----------|
/// | 0   | HDG (AP mode) |
/// | 1   | NAV |
/// | 2   | APR |
/// | 3   | REV |
/// | 4   | ALT |
/// | 5   | VS |
/// | 6   | IAS |
/// | 7   | AP MASTER (CMD) |
/// | 8   | Throttle 2 reverse handle |
/// | 9   | Throttle 3 reverse handle |
/// | 10  | Throttle 4 reverse handle |
/// | 11  | Throttle 5 reverse handle |
/// | 12  | Encoder increment (CW) |
/// | 13  | Encoder decrement (CCW) |
/// | 14  | Flaps down |
/// | 15  | Flaps up |
/// | 16  | AP mode: IAS |
/// | 17  | AP mode: CRS |
/// | 18  | AP mode: HDG |
/// | 19  | AP mode: VS |
/// | 20  | AP mode: ALT |
/// | 21  | Trim down |
/// | 22  | Trim up |
/// | 23  | Throttle 1 reverse zone |
/// | 24  | Throttle 2 reverse zone |
/// | 25  | Throttle 3 reverse zone |
/// | 26  | Throttle 4 reverse zone |
/// | 27  | Throttle 5 reverse zone |
/// | 28  | Throttle 1 reverse handle / Throttle 1+2 2nd function |
/// | 29  | Throttle 3 2nd function |
/// | 30  | Gear UP |
/// | 31  | Gear DOWN |
/// | 32  | Throttle 6 reverse zone |
/// | 33  | Toggle switch 1 UP |
/// | 34  | Toggle switch 1 DOWN |
/// | 35  | Toggle switch 2 UP |
/// | 36  | Toggle switch 2 DOWN |
/// | 37  | Toggle switch 3 UP |
/// | 38  | Toggle switch 3 DOWN |
/// | 39  | Toggle switch 4 UP |
/// | 40  | Toggle switch 4 DOWN |
/// | 41  | Toggle switch 5 UP |
/// | 42  | Toggle switch 5 DOWN |
/// | 43  | Toggle switch 6 UP |
/// | 44  | Toggle switch 6 DOWN |
/// | 45  | Toggle switch 7 UP |
/// | 46  | Toggle switch 7 DOWN |
/// | 47  | Throttle 4 2nd function |
/// | 48–63 | (reserved / unassigned) |
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BravoButtons {
    /// 64-bit bitmask; bit n corresponds to button (n+1) (0-indexed internally).
    pub mask: u64,
}

impl BravoButtons {
    /// Returns `true` if button `n` (1-based, 1–64) is currently pressed.
    pub fn is_pressed(&self, n: u8) -> bool {
        (1u8..=64).contains(&n) && (self.mask >> (n - 1)) & 1 == 1
    }

    /// Returns `true` if the autopilot master button is pressed.
    pub fn ap_master(&self) -> bool {
        self.mask & (1 << 7) != 0
    }

    /// Returns `true` if the landing gear UP lever is active.
    pub fn gear_up(&self) -> bool {
        self.mask & (1 << 30) != 0
    }

    /// Returns `true` if the landing gear DOWN lever is active.
    pub fn gear_down(&self) -> bool {
        self.mask & (1 << 31) != 0
    }
}

/// Parsed state from a single Bravo Throttle HID input report.
#[derive(Debug, Clone)]
pub struct BravoInputState {
    pub axes: BravoAxes,
    pub buttons: BravoButtons,
}

/// Parse a raw HID input report from the Bravo Throttle Quadrant.
///
/// # Errors
///
/// Returns [`BravoParseError`] if the report is too short or has an unexpected
/// report ID byte.
pub fn parse_bravo_report(data: &[u8]) -> Result<BravoInputState, BravoParseError> {
    if data.len() < BRAVO_REPORT_LEN {
        return Err(BravoParseError::TooShort {
            expected: BRAVO_REPORT_LEN,
            got: data.len(),
        });
    }
    if data[0] != 0x01 {
        return Err(BravoParseError::UnknownReportId { id: data[0] });
    }

    let t1 = norm_12bit_unipolar(u16::from_le_bytes([data[1], data[2]]));
    let t2 = norm_12bit_unipolar(u16::from_le_bytes([data[3], data[4]]));
    let t3 = norm_12bit_unipolar(u16::from_le_bytes([data[5], data[6]]));
    let t4 = norm_12bit_unipolar(u16::from_le_bytes([data[7], data[8]]));
    let t5 = norm_12bit_unipolar(u16::from_le_bytes([data[9], data[10]]));
    let flap = norm_12bit_unipolar(u16::from_le_bytes([data[11], data[12]]));
    let spoiler = norm_12bit_unipolar(u16::from_le_bytes([data[13], data[14]]));

    let mask = u64::from_le_bytes(data[15..23].try_into().unwrap());

    Ok(BravoInputState {
        axes: BravoAxes {
            throttle1: t1,
            throttle2: t2,
            throttle3: t3,
            throttle4: t4,
            throttle5: t5,
            flap_lever: flap,
            spoiler,
        },
        buttons: BravoButtons { mask },
    })
}

/// Errors returned by [`parse_bravo_report`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BravoParseError {
    #[error("report too short: expected ≥{expected} bytes, got {got}")]
    TooShort { expected: usize, got: usize },
    #[error("unknown report ID: 0x{id:02X}")]
    UnknownReportId { id: u8 },
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Normalise a 12-bit unsigned value to \[0.0, 1.0\].
fn norm_12bit_unipolar(raw: u16) -> f32 {
    let raw = raw.min(4095);
    (raw as f32 / 4095.0).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bravo_report(throttles: [u16; 7], buttons: u64) -> [u8; BRAVO_REPORT_LEN] {
        let mut r = [0u8; BRAVO_REPORT_LEN];
        r[0] = 0x01;
        for (i, &t) in throttles.iter().enumerate() {
            let off = 1 + i * 2;
            r[off..off + 2].copy_from_slice(&t.to_le_bytes());
        }
        r[15..23].copy_from_slice(&buttons.to_le_bytes());
        r
    }

    #[test]
    fn test_all_throttles_min() {
        let state = parse_bravo_report(&bravo_report([0; 7], 0)).unwrap();
        assert!(state.axes.throttle1 < 0.001);
        assert!(state.axes.throttle5 < 0.001);
        assert!(state.axes.flap_lever < 0.001);
        assert!(state.axes.spoiler < 0.001);
    }

    #[test]
    fn test_all_throttles_max() {
        let state = parse_bravo_report(&bravo_report([4095; 7], 0)).unwrap();
        assert!((state.axes.throttle1 - 1.0).abs() < 1e-4);
        assert!((state.axes.flap_lever - 1.0).abs() < 1e-4);
        assert!((state.axes.spoiler - 1.0).abs() < 1e-4);
    }

    #[test]
    fn test_throttle_mid() {
        let state = parse_bravo_report(&bravo_report([2048, 0, 0, 0, 0, 0, 0], 0)).unwrap();
        let expected = 2048.0 / 4095.0;
        assert!((state.axes.throttle1 - expected).abs() < 1e-3);
    }

    #[test]
    fn test_gear_buttons() {
        // Bit 30 = gear up, bit 31 = gear down
        let gear_up: u64 = 1 << 30;
        let state = parse_bravo_report(&bravo_report([0; 7], gear_up)).unwrap();
        assert!(state.buttons.gear_up(), "gear up should be active");
        assert!(!state.buttons.gear_down(), "gear down should not be active");
    }

    #[test]
    fn test_ap_master_button() {
        let ap: u64 = 1 << 7;
        let state = parse_bravo_report(&bravo_report([0; 7], ap)).unwrap();
        assert!(state.buttons.ap_master());
        assert!(state.buttons.is_pressed(8)); // bit 7 = button 8 (1-indexed)
    }

    #[test]
    fn test_report_too_short() {
        let err = parse_bravo_report(&[0x01; 5]).unwrap_err();
        assert!(matches!(err, BravoParseError::TooShort { .. }));
    }

    #[test]
    fn test_unknown_report_id() {
        let mut r = [0u8; BRAVO_REPORT_LEN];
        r[0] = 0x02;
        assert!(matches!(
            parse_bravo_report(&r),
            Err(BravoParseError::UnknownReportId { id: 0x02 })
        ));
    }

    #[cfg(test)]
    mod proptest_tests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn throttles_within_bounds(
                t1 in 0u16..=4095u16,
                t2 in 0u16..=4095u16,
            ) {
                let state = parse_bravo_report(&super::bravo_report(
                    [t1, t2, 0, 0, 0, 0, 0], 0
                )).unwrap();
                prop_assert!((0.0..=1.0001).contains(&state.axes.throttle1));
                prop_assert!((0.0..=1.0001).contains(&state.axes.throttle2));
            }

            #[test]
            fn any_valid_report_parses(
                t1 in 0u16..=4095u16,
                t2 in 0u16..=4095u16,
                t3 in 0u16..=4095u16,
                t4 in 0u16..=4095u16,
                t5 in 0u16..=4095u16,
                flap in 0u16..=4095u16,
                spoiler in 0u16..=4095u16,
                buttons in 0u64..u64::MAX,
            ) {
                let r = super::bravo_report([t1, t2, t3, t4, t5, flap, spoiler], buttons);
                prop_assert!(parse_bravo_report(&r).is_ok());
            }
        }
    }
}
