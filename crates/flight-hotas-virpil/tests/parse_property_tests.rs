// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Property-based integration tests for VIRPIL VPC device parsers.
//!
//! Covers all seven device variants exported from `flight-hotas-virpil`:
//! Alpha, Alpha Prime, MongoosT-50CM3, WarBRD/WarBRD-D, CM3 Throttle,
//! Control Panel 1, and Control Panel 2.

use flight_hotas_virpil::{
    AlphaPrimeVariant, PANEL2_BUTTON_COUNT, VIRPIL_AXIS_MAX, VIRPIL_VENDOR_ID,
    VPC_ALPHA_MIN_REPORT_BYTES, VPC_ALPHA_PRIME_MIN_REPORT_BYTES,
    VPC_CM3_THROTTLE_MIN_REPORT_BYTES, VPC_MONGOOST_STICK_MIN_REPORT_BYTES,
    VPC_PANEL1_MIN_REPORT_BYTES, VPC_PANEL2_MIN_REPORT_BYTES, VPC_WARBRD_MIN_REPORT_BYTES,
    WarBrdVariant, parse_alpha_prime_report, parse_alpha_report, parse_cm3_throttle_report,
    parse_mongoost_stick_report, parse_panel1_report, parse_panel2_report, parse_warbrd_report,
};
use proptest::prelude::*;

// ─── Report builders ─────────────────────────────────────────────────────────

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

fn make_panel1_report(buttons: [u8; 6]) -> Vec<u8> {
    let mut data = vec![0x01u8];
    data.extend_from_slice(&buttons);
    data
}

fn make_panel2_report(a1: u16, a2: u16, buttons: [u8; 6]) -> Vec<u8> {
    let mut data = vec![0x01u8];
    data.extend_from_slice(&a1.to_le_bytes());
    data.extend_from_slice(&a2.to_le_bytes());
    data.extend_from_slice(&buttons);
    data
}

// ─── Constellation Alpha ──────────────────────────────────────────────────────

proptest! {
    /// Axis values are always finite (never NaN or Inf) for any raw byte input.
    #[test]
    fn prop_alpha_axes_finite(
        raw0 in 0u16..=u16::MAX,
        raw1 in 0u16..=u16::MAX,
        raw2 in 0u16..=u16::MAX,
        raw3 in 0u16..=u16::MAX,
        raw4 in 0u16..=u16::MAX,
    ) {
        let report = make_5ax_report([raw0, raw1, raw2, raw3, raw4], [0u8; 4]);
        let s = parse_alpha_report(&report).unwrap();
        prop_assert!(s.axes.x.is_finite(), "x={} is not finite", s.axes.x);
        prop_assert!(s.axes.y.is_finite(), "y={} is not finite", s.axes.y);
        prop_assert!(s.axes.z.is_finite(), "z={} is not finite", s.axes.z);
        prop_assert!(s.axes.sz.is_finite(), "sz={} is not finite", s.axes.sz);
        prop_assert!(s.axes.sl.is_finite(), "sl={} is not finite", s.axes.sl);
    }

    /// Button query agrees with raw bit mask for all 28 Alpha buttons.
    #[test]
    fn prop_alpha_button_query_matches_mask(mask in 0u32..(1u32 << 28)) {
        let raw = mask.to_le_bytes();
        let report = make_5ax_report([0u16; 5], [raw[0], raw[1], raw[2], raw[3]]);
        let s = parse_alpha_report(&report).unwrap();
        for n in 1u8..=28 {
            let expected = (mask >> (n - 1)) & 1 == 1;
            prop_assert_eq!(
                s.buttons.is_pressed(n),
                expected,
                "button {} mismatch, mask=0x{:08X}",
                n,
                mask
            );
        }
        // Out-of-range buttons never claimed pressed.
        prop_assert!(!s.buttons.is_pressed(0));
        prop_assert!(!s.buttons.is_pressed(29));
    }

    /// Truncated reports always return a parse error.
    #[test]
    fn prop_alpha_short_report_errors(len in 0usize..VPC_ALPHA_MIN_REPORT_BYTES) {
        let data = vec![0x01u8; len];
        prop_assert!(parse_alpha_report(&data).is_err());
    }
}

#[test]
fn alpha_boundary_min_len_succeeds() {
    let r = make_5ax_report([0u16; 5], [0u8; 4]);
    assert_eq!(r.len(), VPC_ALPHA_MIN_REPORT_BYTES);
    assert!(parse_alpha_report(&r).is_ok());
}

#[test]
fn alpha_one_byte_short_fails() {
    assert!(parse_alpha_report(&[0u8; VPC_ALPHA_MIN_REPORT_BYTES - 1]).is_err());
}

// ─── MongoosT-50CM3 ───────────────────────────────────────────────────────────

proptest! {
    /// Axis values are always finite for any raw u16 axis bytes.
    #[test]
    fn prop_mongoost_axes_finite(
        raw0 in 0u16..=u16::MAX,
        raw1 in 0u16..=u16::MAX,
        raw2 in 0u16..=u16::MAX,
        raw3 in 0u16..=u16::MAX,
        raw4 in 0u16..=u16::MAX,
    ) {
        let report = make_5ax_report([raw0, raw1, raw2, raw3, raw4], [0u8; 4]);
        let s = parse_mongoost_stick_report(&report).unwrap();
        prop_assert!(s.axes.x.is_finite());
        prop_assert!(s.axes.y.is_finite());
        prop_assert!(s.axes.z.is_finite());
        prop_assert!(s.axes.sz.is_finite());
        prop_assert!(s.axes.sl.is_finite());
    }

    /// Button query agrees with raw bit mask for all 28 MongoosT buttons.
    #[test]
    fn prop_mongoost_button_query_matches_mask(mask in 0u32..(1u32 << 28)) {
        let raw = mask.to_le_bytes();
        let report = make_5ax_report([0u16; 5], [raw[0], raw[1], raw[2], raw[3]]);
        let s = parse_mongoost_stick_report(&report).unwrap();
        for n in 1u8..=28 {
            let expected = (mask >> (n - 1)) & 1 == 1;
            prop_assert_eq!(
                s.buttons.is_pressed(n),
                expected,
                "button {} mismatch, mask=0x{:08X}",
                n,
                mask
            );
        }
        prop_assert!(!s.buttons.is_pressed(0));
        prop_assert!(!s.buttons.is_pressed(29));
    }

    /// Truncated reports always return a parse error.
    #[test]
    fn prop_mongoost_short_report_errors(len in 0usize..VPC_MONGOOST_STICK_MIN_REPORT_BYTES) {
        let data = vec![0x01u8; len];
        prop_assert!(parse_mongoost_stick_report(&data).is_err());
    }
}

// ─── WarBRD / WarBRD-D ────────────────────────────────────────────────────────

proptest! {
    /// Axes always in [0, 1] for both WarBRD variants.
    #[test]
    fn prop_warbrd_axes_in_range(
        raw0 in 0u16..=u16::MAX,
        raw1 in 0u16..=u16::MAX,
        raw2 in 0u16..=u16::MAX,
        raw3 in 0u16..=u16::MAX,
        raw4 in 0u16..=u16::MAX,
    ) {
        let report = make_5ax_report([raw0, raw1, raw2, raw3, raw4], [0u8; 4]);
        let s = parse_warbrd_report(&report, WarBrdVariant::D).unwrap();
        prop_assert!((0.0..=1.0).contains(&s.inner.axes.x));
        prop_assert!((0.0..=1.0).contains(&s.inner.axes.y));
        prop_assert!((0.0..=1.0).contains(&s.inner.axes.z));
        prop_assert!((0.0..=1.0).contains(&s.inner.axes.sz));
        prop_assert!((0.0..=1.0).contains(&s.inner.axes.sl));
    }

    /// Truncated reports always return a parse error.
    #[test]
    fn prop_warbrd_short_report_errors(len in 0usize..VPC_WARBRD_MIN_REPORT_BYTES) {
        let data = vec![0x01u8; len];
        prop_assert!(parse_warbrd_report(&data, WarBrdVariant::D).is_err());
    }
}

#[test]
fn warbrd_variant_preserved_in_state() {
    let r = make_5ax_report([0u16; 5], [0u8; 4]);
    assert_eq!(
        parse_warbrd_report(&r, WarBrdVariant::Original)
            .unwrap()
            .variant,
        WarBrdVariant::Original
    );
    assert_eq!(
        parse_warbrd_report(&r, WarBrdVariant::D).unwrap().variant,
        WarBrdVariant::D
    );
}

// ─── CM3 Throttle ─────────────────────────────────────────────────────────────

proptest! {
    /// All six throttle axes are always finite.
    #[test]
    fn prop_cm3_axes_finite(
        raw0 in 0u16..=u16::MAX,
        raw1 in 0u16..=u16::MAX,
        raw2 in 0u16..=u16::MAX,
        raw3 in 0u16..=u16::MAX,
        raw4 in 0u16..=u16::MAX,
        raw5 in 0u16..=u16::MAX,
    ) {
        let report = make_cm3_report([raw0, raw1, raw2, raw3, raw4, raw5], [0u8; 10]);
        let s = parse_cm3_throttle_report(&report).unwrap();
        prop_assert!(s.axes.left_throttle.is_finite());
        prop_assert!(s.axes.right_throttle.is_finite());
        prop_assert!(s.axes.flaps.is_finite());
        prop_assert!(s.axes.scx.is_finite());
        prop_assert!(s.axes.scy.is_finite());
        prop_assert!(s.axes.slider.is_finite());
    }

    /// Out-of-range button indices always return false regardless of raw bytes.
    #[test]
    fn prop_cm3_button_oob_returns_false(mask_lo in 0u64..=u64::MAX, mask_hi in 0u16..=u16::MAX) {
        let mut buttons = [0u8; 10];
        buttons[..8].copy_from_slice(&mask_lo.to_le_bytes());
        buttons[8..10].copy_from_slice(&mask_hi.to_le_bytes());
        let report = make_cm3_report([0u16; 6], buttons);
        let s = parse_cm3_throttle_report(&report).unwrap();
        prop_assert!(!s.buttons.is_pressed(0), "button 0 should always be false");
        prop_assert!(!s.buttons.is_pressed(79), "button 79 should always be false");
    }

    /// Truncated reports always return a parse error.
    #[test]
    fn prop_cm3_short_report_errors(len in 0usize..VPC_CM3_THROTTLE_MIN_REPORT_BYTES) {
        let data = vec![0x01u8; len];
        prop_assert!(parse_cm3_throttle_report(&data).is_err());
    }
}

// ─── Constellation Alpha Prime ────────────────────────────────────────────────

proptest! {
    /// Axes always in [0, 1] for both Alpha Prime variants.
    #[test]
    fn prop_alpha_prime_axes_in_range(
        raw0 in 0u16..=u16::MAX,
        raw1 in 0u16..=u16::MAX,
        raw2 in 0u16..=u16::MAX,
        raw3 in 0u16..=u16::MAX,
        raw4 in 0u16..=u16::MAX,
    ) {
        let report = make_5ax_report([raw0, raw1, raw2, raw3, raw4], [0u8; 4]);
        let s = parse_alpha_prime_report(&report, AlphaPrimeVariant::Left).unwrap();
        prop_assert!((0.0..=1.0).contains(&s.axes.x));
        prop_assert!((0.0..=1.0).contains(&s.axes.y));
        prop_assert!((0.0..=1.0).contains(&s.axes.z));
        prop_assert!((0.0..=1.0).contains(&s.axes.sz));
        prop_assert!((0.0..=1.0).contains(&s.axes.sl));
    }

    /// Truncated reports always return a parse error.
    #[test]
    fn prop_alpha_prime_short_report_errors(len in 0usize..VPC_ALPHA_PRIME_MIN_REPORT_BYTES) {
        let data = vec![0x01u8; len];
        prop_assert!(parse_alpha_prime_report(&data, AlphaPrimeVariant::Right).is_err());
    }
}

#[test]
fn alpha_prime_variant_preserved_in_state() {
    let r = make_5ax_report([0u16; 5], [0u8; 4]);
    assert_eq!(
        parse_alpha_prime_report(&r, AlphaPrimeVariant::Left)
            .unwrap()
            .variant,
        AlphaPrimeVariant::Left
    );
    assert_eq!(
        parse_alpha_prime_report(&r, AlphaPrimeVariant::Right)
            .unwrap()
            .variant,
        AlphaPrimeVariant::Right
    );
}

// ─── Control Panel 1 ──────────────────────────────────────────────────────────

proptest! {
    /// Button query agrees with raw bit mask for all 48 Panel 1 buttons.
    #[test]
    fn prop_panel1_button_query_matches_mask(mask in 0u64..(1u64 << 48)) {
        let raw = mask.to_le_bytes();
        let buttons = [raw[0], raw[1], raw[2], raw[3], raw[4], raw[5]];
        let report = make_panel1_report(buttons);
        let s = parse_panel1_report(&report).unwrap();
        for n in 1u8..=48 {
            let expected = (mask >> (n - 1)) & 1 == 1;
            prop_assert_eq!(
                s.buttons.is_pressed(n),
                expected,
                "button {} mismatch, mask=0x{:012X}",
                n,
                mask
            );
        }
        prop_assert!(!s.buttons.is_pressed(0));
        prop_assert!(!s.buttons.is_pressed(49));
    }

    /// Truncated reports always return a parse error.
    #[test]
    fn prop_panel1_short_report_errors(len in 0usize..VPC_PANEL1_MIN_REPORT_BYTES) {
        let data = vec![0x01u8; len];
        prop_assert!(parse_panel1_report(&data).is_err());
    }
}

#[test]
fn panel1_boundary_min_len_succeeds() {
    let r = make_panel1_report([0u8; 6]);
    assert_eq!(r.len(), VPC_PANEL1_MIN_REPORT_BYTES);
    assert!(parse_panel1_report(&r).is_ok());
}

// ─── Control Panel 2 ──────────────────────────────────────────────────────────

proptest! {
    /// Normalised axes are non-negative and finite for valid axis values.
    #[test]
    fn prop_panel2_axes_normalised_non_negative_finite(
        a1 in 0u16..=VIRPIL_AXIS_MAX,
        a2 in 0u16..=VIRPIL_AXIS_MAX,
    ) {
        let report = make_panel2_report(a1, a2, [0u8; 6]);
        let s = parse_panel2_report(&report).unwrap();
        let n1 = s.axes.a1_normalised();
        let n2 = s.axes.a2_normalised();
        prop_assert!((0.0..=1.0).contains(&n1), "a1_normalised={n1} out of range");
        prop_assert!((0.0..=1.0).contains(&n2), "a2_normalised={n2} out of range");
        prop_assert!(n1.is_finite(), "a1_normalised={n1} not finite");
        prop_assert!(n2.is_finite(), "a2_normalised={n2} not finite");
    }

    /// Button query agrees with raw bit mask for all 47 Panel 2 buttons.
    #[test]
    fn prop_panel2_button_query_matches_mask(mask in 0u64..(1u64 << 47)) {
        let raw = mask.to_le_bytes();
        let buttons = [raw[0], raw[1], raw[2], raw[3], raw[4], raw[5]];
        let report = make_panel2_report(0, 0, buttons);
        let s = parse_panel2_report(&report).unwrap();
        for n in 1u8..=PANEL2_BUTTON_COUNT {
            let expected = (mask >> (n - 1)) & 1 == 1;
            prop_assert_eq!(
                s.buttons.is_pressed(n),
                expected,
                "button {} mismatch, mask=0x{:012X}",
                n,
                mask
            );
        }
        prop_assert!(!s.buttons.is_pressed(0));
        prop_assert!(!s.buttons.is_pressed(PANEL2_BUTTON_COUNT + 1));
    }

    /// Truncated reports always return a parse error.
    #[test]
    fn prop_panel2_short_report_errors(len in 0usize..VPC_PANEL2_MIN_REPORT_BYTES) {
        let data = vec![0x01u8; len];
        prop_assert!(parse_panel2_report(&data).is_err());
    }
}

#[test]
fn panel2_axes_raw_round_trip() {
    let report = make_panel2_report(VIRPIL_AXIS_MAX, VIRPIL_AXIS_MAX / 2, [0u8; 6]);
    let s = parse_panel2_report(&report).unwrap();
    assert_eq!(s.axes.a1_raw, VIRPIL_AXIS_MAX);
    assert_eq!(s.axes.a2_raw, VIRPIL_AXIS_MAX / 2);
}

// ─── Misc ─────────────────────────────────────────────────────────────────────

#[test]
fn virpil_vendor_id_is_correct() {
    // VIRPIL Controls, UAB — VID 0x3344 (confirmed via the-sz.com USB ID DB)
    assert_eq!(VIRPIL_VENDOR_ID, 0x3344);
}
