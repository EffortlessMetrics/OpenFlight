// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Property-based tests for Honeycomb Alpha Yoke and Bravo Throttle Quadrant HID report parsing.

use flight_hotas_honeycomb::alpha::ALPHA_REPORT_LEN;
use flight_hotas_honeycomb::bravo::BRAVO_REPORT_LEN;
use flight_hotas_honeycomb::{
    AlphaParseError, BravoParseError, HONEYCOMB_VENDOR_ID, parse_alpha_report, parse_bravo_report,
};
use proptest::prelude::*;

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn make_alpha_report(roll: u16, pitch: u16, buttons: u64, hat: u8) -> [u8; ALPHA_REPORT_LEN] {
    let mut r = [0u8; ALPHA_REPORT_LEN];
    r[0] = 0x01;
    r[1..3].copy_from_slice(&roll.to_le_bytes());
    r[3..5].copy_from_slice(&pitch.to_le_bytes());
    r[5] = (buttons & 0xFF) as u8;
    r[6] = ((buttons >> 8) & 0xFF) as u8;
    r[7] = ((buttons >> 16) & 0xFF) as u8;
    r[8] = ((buttons >> 24) & 0xFF) as u8;
    r[9] = ((buttons >> 32) & 0xFF) as u8;
    r[10] = hat & 0x0F;
    r
}

fn make_bravo_report(throttles: [u16; 7], buttons: u64) -> [u8; BRAVO_REPORT_LEN] {
    let mut r = [0u8; BRAVO_REPORT_LEN];
    r[0] = 0x01;
    for (i, &t) in throttles.iter().enumerate() {
        let off = 1 + i * 2;
        r[off..off + 2].copy_from_slice(&t.to_le_bytes());
    }
    r[15..23].copy_from_slice(&buttons.to_le_bytes());
    r
}

// ─── Alpha Yoke ───────────────────────────────────────────────────────────────

proptest! {
    /// Roll axis is always in [-1.0, 1.0] (actual min ≈ -1.0005 due to 12-bit centring).
    #[test]
    fn prop_alpha_roll_in_range(roll in 0u16..=u16::MAX) {
        let state = parse_alpha_report(&make_alpha_report(roll, 2048, 0, 15)).unwrap();
        prop_assert!(
            state.axes.roll >= -1.001 && state.axes.roll <= 1.001,
            "roll out of range: {}", state.axes.roll
        );
    }

    /// Pitch axis is always in [-1.0, 1.0].
    #[test]
    fn prop_alpha_pitch_in_range(pitch in 0u16..=u16::MAX) {
        let state = parse_alpha_report(&make_alpha_report(2048, pitch, 0, 15)).unwrap();
        prop_assert!(
            state.axes.pitch >= -1.001 && state.axes.pitch <= 1.001,
            "pitch out of range: {}", state.axes.pitch
        );
    }

    /// Both axes are always finite (no NaN or Inf).
    #[test]
    fn prop_alpha_axes_finite(
        roll  in 0u16..=u16::MAX,
        pitch in 0u16..=u16::MAX,
    ) {
        let state = parse_alpha_report(&make_alpha_report(roll, pitch, 0, 15)).unwrap();
        prop_assert!(state.axes.roll.is_finite(),  "roll not finite");
        prop_assert!(state.axes.pitch.is_finite(), "pitch not finite");
    }

    /// Reports shorter than ALPHA_REPORT_LEN always return TooShort.
    #[test]
    fn prop_alpha_short_report_errors(len in 0usize..ALPHA_REPORT_LEN) {
        let data = vec![0x01u8; len];
        prop_assert!(
            matches!(parse_alpha_report(&data), Err(AlphaParseError::TooShort { .. })),
            "expected TooShort for len={}", len
        );
    }

    /// Hat value parsed from any 4-bit nibble (0–15) is always in 0..=8.
    /// 0 = centred, 1–8 = N/NE/E/SE/S/SW/W/NW.
    #[test]
    fn prop_alpha_hat_always_valid(hat_nibble in 0u8..=15u8) {
        let state = parse_alpha_report(&make_alpha_report(2048, 2048, 0, hat_nibble)).unwrap();
        prop_assert!(
            state.buttons.hat <= 8,
            "hat out of valid range: {}", state.buttons.hat
        );
    }

    /// Reports with a wrong report ID (not 0x01) always return UnknownReportId.
    #[test]
    fn prop_alpha_wrong_id_errors(id in 2u8..=0xFFu8) {
        let mut r = [0u8; ALPHA_REPORT_LEN];
        r[0] = id;
        prop_assert!(
            matches!(parse_alpha_report(&r), Err(AlphaParseError::UnknownReportId { .. })),
            "expected UnknownReportId for id=0x{:02X}", id
        );
    }
}

// ─── Bravo Throttle Quadrant ──────────────────────────────────────────────────

proptest! {
    /// All 7 Bravo throttle levers are always in [0.0, 1.0].
    #[test]
    fn prop_bravo_all_throttles_in_range(
        t1      in 0u16..=u16::MAX,
        t2      in 0u16..=u16::MAX,
        t3      in 0u16..=u16::MAX,
        t4      in 0u16..=u16::MAX,
        t5      in 0u16..=u16::MAX,
        flap    in 0u16..=u16::MAX,
        spoiler in 0u16..=u16::MAX,
    ) {
        let state =
            parse_bravo_report(&make_bravo_report([t1, t2, t3, t4, t5, flap, spoiler], 0))
                .unwrap();
        prop_assert!(state.axes.throttle1  >= 0.0 && state.axes.throttle1  <= 1.0);
        prop_assert!(state.axes.throttle2  >= 0.0 && state.axes.throttle2  <= 1.0);
        prop_assert!(state.axes.throttle3  >= 0.0 && state.axes.throttle3  <= 1.0);
        prop_assert!(state.axes.throttle4  >= 0.0 && state.axes.throttle4  <= 1.0);
        prop_assert!(state.axes.throttle5  >= 0.0 && state.axes.throttle5  <= 1.0);
        prop_assert!(state.axes.flap_lever >= 0.0 && state.axes.flap_lever <= 1.0);
        prop_assert!(state.axes.spoiler    >= 0.0 && state.axes.spoiler    <= 1.0);
    }

    /// All Bravo axes are always finite (no NaN or Inf).
    #[test]
    fn prop_bravo_axes_finite(
        t1 in 0u16..=u16::MAX,
        t2 in 0u16..=u16::MAX,
        t3 in 0u16..=u16::MAX,
    ) {
        let state =
            parse_bravo_report(&make_bravo_report([t1, t2, t3, 0, 0, 0, 0], 0)).unwrap();
        prop_assert!(state.axes.throttle1.is_finite(), "throttle1 not finite");
        prop_assert!(state.axes.throttle2.is_finite(), "throttle2 not finite");
        prop_assert!(state.axes.throttle3.is_finite(), "throttle3 not finite");
    }

    /// Reports shorter than BRAVO_REPORT_LEN always return TooShort.
    #[test]
    fn prop_bravo_short_report_errors(len in 0usize..BRAVO_REPORT_LEN) {
        let data = vec![0x01u8; len];
        prop_assert!(
            matches!(parse_bravo_report(&data), Err(BravoParseError::TooShort { .. })),
            "expected TooShort for len={}", len
        );
    }

    /// Reports with a wrong report ID always return UnknownReportId.
    #[test]
    fn prop_bravo_wrong_id_errors(id in 2u8..=0xFFu8) {
        let mut r = [0u8; BRAVO_REPORT_LEN];
        r[0] = id;
        prop_assert!(
            matches!(parse_bravo_report(&r), Err(BravoParseError::UnknownReportId { .. })),
            "expected UnknownReportId for id=0x{:02X}", id
        );
    }

    /// Button mask (64-bit) round-trips exactly.
    #[test]
    fn prop_bravo_button_mask_roundtrip(
        mask_lo in 0u32..=u32::MAX,
        mask_hi in 0u32..=u32::MAX,
    ) {
        let mask = (mask_hi as u64) << 32 | mask_lo as u64;
        let state = parse_bravo_report(&make_bravo_report([0; 7], mask)).unwrap();
        prop_assert_eq!(state.buttons.mask, mask);
    }

    /// The first 24 meaningful buttons always match their corresponding mask bits.
    #[test]
    fn prop_bravo_first_24_buttons_match(mask_24 in 0u32..(1u32 << 24)) {
        let mask = mask_24 as u64;
        let state = parse_bravo_report(&make_bravo_report([0; 7], mask)).unwrap();
        for n in 1u8..=24u8 {
            let expected = (mask >> (n - 1)) & 1 == 1;
            prop_assert_eq!(
                state.buttons.is_pressed(n),
                expected,
                "button {} mismatch mask=0x{:06X}", n, mask_24
            );
        }
    }
}

// ─── Unit tests ───────────────────────────────────────────────────────────────

#[test]
fn honeycomb_vendor_id_is_correct() {
    assert_eq!(HONEYCOMB_VENDOR_ID, 0x294B);
}

#[test]
fn alpha_hat_centred_maps_to_zero() {
    // Hat nibble 0xF (15) = centred → hat field should be 0
    let state = parse_alpha_report(&make_alpha_report(2048, 2048, 0, 15)).unwrap();
    assert_eq!(state.buttons.hat, 0);
    assert_eq!(state.buttons.hat_direction(), "center");
}

#[test]
fn bravo_gear_up_bit_30() {
    let mask: u64 = 1 << 30;
    let state = parse_bravo_report(&make_bravo_report([0; 7], mask)).unwrap();
    assert!(
        state.buttons.gear_up(),
        "gear_up should be active at bit 30"
    );
    assert!(!state.buttons.gear_down());
}
