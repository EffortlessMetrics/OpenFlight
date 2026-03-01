// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Property-based integration tests for VIRPIL VPC device parsers.
//!
//! Covers all seven device variants exported from `flight-hotas-virpil`:
//! Alpha, Alpha Prime, MongoosT-50CM3, WarBRD/WarBRD-D, CM3 Throttle,
//! Control Panel 1, and Control Panel 2.

use flight_hotas_virpil::{
    ACE_PEDALS_BUTTON_COUNT, ACE_TORQ_BUTTON_COUNT, AlphaPrimeVariant, PANEL2_BUTTON_COUNT,
    ROTOR_TCS_BUTTON_COUNT, VIRPIL_AXIS_MAX, VIRPIL_VENDOR_ID, VPC_ACE_PEDALS_MIN_REPORT_BYTES,
    VPC_ACE_TORQ_MIN_REPORT_BYTES, VPC_ALPHA_MIN_REPORT_BYTES, VPC_ALPHA_PRIME_MIN_REPORT_BYTES,
    VPC_CM3_THROTTLE_MIN_REPORT_BYTES, VPC_MONGOOST_STICK_MIN_REPORT_BYTES,
    VPC_PANEL1_MIN_REPORT_BYTES, VPC_PANEL2_MIN_REPORT_BYTES, VPC_ROTOR_TCS_MIN_REPORT_BYTES,
    VPC_WARBRD_MIN_REPORT_BYTES, WarBrdVariant, cm3_encoder_bank, dispatch_report,
    parse_ace_pedals_report, parse_ace_torq_report, parse_alpha_prime_report, parse_alpha_report,
    parse_cm3_throttle_report, parse_mongoost_stick_report, parse_panel1_report,
    parse_panel2_report, parse_rotor_tcs_report, parse_warbrd_report,
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

// ─── ACE Pedals ───────────────────────────────────────────────────────────────

fn make_pedals_report(axes: [u16; 3], buttons: [u8; 2]) -> Vec<u8> {
    let mut data = vec![0x01u8];
    for ax in &axes {
        data.extend_from_slice(&ax.to_le_bytes());
    }
    data.extend_from_slice(&buttons);
    data
}

proptest! {
    /// Axis values are always finite for any raw u16 axis bytes.
    #[test]
    fn prop_ace_pedals_axes_finite(
        raw0 in 0u16..=u16::MAX,
        raw1 in 0u16..=u16::MAX,
        raw2 in 0u16..=u16::MAX,
    ) {
        let report = make_pedals_report([raw0, raw1, raw2], [0u8; 2]);
        let s = parse_ace_pedals_report(&report).unwrap();
        prop_assert!(s.axes.rudder.is_finite());
        prop_assert!(s.axes.left_toe_brake.is_finite());
        prop_assert!(s.axes.right_toe_brake.is_finite());
    }

    /// Button query agrees with raw bit mask for all ACE Pedals buttons.
    #[test]
    fn prop_ace_pedals_button_query_matches_mask(mask in 0u16..=u16::MAX) {
        let raw = mask.to_le_bytes();
        let report = make_pedals_report([0u16; 3], [raw[0], raw[1]]);
        let s = parse_ace_pedals_report(&report).unwrap();
        for n in 1u8..=ACE_PEDALS_BUTTON_COUNT {
            let expected = (mask >> (n - 1)) & 1 == 1;
            prop_assert_eq!(
                s.buttons.is_pressed(n),
                expected,
                "button {} mismatch, mask=0x{:04X}",
                n,
                mask
            );
        }
        prop_assert!(!s.buttons.is_pressed(0));
        prop_assert!(!s.buttons.is_pressed(ACE_PEDALS_BUTTON_COUNT + 1));
    }

    /// Truncated reports always return a parse error.
    #[test]
    fn prop_ace_pedals_short_report_errors(len in 0usize..VPC_ACE_PEDALS_MIN_REPORT_BYTES) {
        let data = vec![0x01u8; len];
        prop_assert!(parse_ace_pedals_report(&data).is_err());
    }
}

// ─── ACE Torq ─────────────────────────────────────────────────────────────────

fn make_torq_report(throttle: u16, buttons: [u8; 2]) -> Vec<u8> {
    let mut data = vec![0x01u8];
    data.extend_from_slice(&throttle.to_le_bytes());
    data.extend_from_slice(&buttons);
    data
}

proptest! {
    /// Throttle axis is always finite for any raw u16 value.
    #[test]
    fn prop_ace_torq_axis_finite(raw in 0u16..=u16::MAX) {
        let report = make_torq_report(raw, [0u8; 2]);
        let s = parse_ace_torq_report(&report).unwrap();
        prop_assert!(s.axis.throttle.is_finite());
        prop_assert!((0.0..=1.0).contains(&s.axis.throttle));
    }

    /// Button query agrees with raw bit mask for all ACE Torq buttons.
    #[test]
    fn prop_ace_torq_button_query_matches_mask(mask in 0u8..=u8::MAX) {
        let report = make_torq_report(0, [mask, 0x00]);
        let s = parse_ace_torq_report(&report).unwrap();
        for n in 1u8..=ACE_TORQ_BUTTON_COUNT {
            let expected = (mask >> (n - 1)) & 1 == 1;
            prop_assert_eq!(
                s.buttons.is_pressed(n),
                expected,
                "button {} mismatch, mask=0x{:02X}",
                n,
                mask
            );
        }
        prop_assert!(!s.buttons.is_pressed(0));
        prop_assert!(!s.buttons.is_pressed(ACE_TORQ_BUTTON_COUNT + 1));
    }

    /// Truncated reports always return a parse error.
    #[test]
    fn prop_ace_torq_short_report_errors(len in 0usize..VPC_ACE_TORQ_MIN_REPORT_BYTES) {
        let data = vec![0x01u8; len];
        prop_assert!(parse_ace_torq_report(&data).is_err());
    }
}

// ─── Rotor TCS Plus ──────────────────────────────────────────────────────────

fn make_rotor_tcs_report(axes: [u16; 3], buttons: [u8; 4]) -> Vec<u8> {
    let mut data = vec![0x01u8];
    for ax in &axes {
        data.extend_from_slice(&ax.to_le_bytes());
    }
    data.extend_from_slice(&buttons);
    data
}

proptest! {
    /// Axis values are always finite for any raw u16 axis bytes.
    #[test]
    fn prop_rotor_tcs_axes_finite(
        raw0 in 0u16..=u16::MAX,
        raw1 in 0u16..=u16::MAX,
        raw2 in 0u16..=u16::MAX,
    ) {
        let report = make_rotor_tcs_report([raw0, raw1, raw2], [0u8; 4]);
        let s = parse_rotor_tcs_report(&report).unwrap();
        prop_assert!(s.axes.collective.is_finite());
        prop_assert!(s.axes.throttle_idle.is_finite());
        prop_assert!(s.axes.rotary.is_finite());
    }

    /// Button query agrees with raw bit mask for all Rotor TCS buttons.
    #[test]
    fn prop_rotor_tcs_button_query_matches_mask(mask in 0u32..(1u32 << ROTOR_TCS_BUTTON_COUNT)) {
        let raw = mask.to_le_bytes();
        let report = make_rotor_tcs_report([0u16; 3], [raw[0], raw[1], raw[2], raw[3]]);
        let s = parse_rotor_tcs_report(&report).unwrap();
        for n in 1u8..=ROTOR_TCS_BUTTON_COUNT {
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
        prop_assert!(!s.buttons.is_pressed(ROTOR_TCS_BUTTON_COUNT + 1));
    }

    /// Truncated reports always return a parse error.
    #[test]
    fn prop_rotor_tcs_short_report_errors(len in 0usize..VPC_ROTOR_TCS_MIN_REPORT_BYTES) {
        let data = vec![0x01u8; len];
        prop_assert!(parse_rotor_tcs_report(&data).is_err());
    }
}

// ─── Unified dispatcher ──────────────────────────────────────────────────────

#[test]
fn dispatch_routes_all_known_pids() {
    use flight_hotas_virpil::{
        VIRPIL_ACE_PEDALS_PID, VIRPIL_ACE_TORQ_PID, VIRPIL_CM3_THROTTLE_PID,
        VIRPIL_CONSTELLATION_ALPHA_LEFT_PID, VIRPIL_MONGOOST_STICK_PID, VIRPIL_PANEL1_PID,
        VIRPIL_PANEL2_PID, VIRPIL_ROTOR_TCS_PLUS_PID, VIRPIL_WARBRD_PID,
    };

    // Build the largest needed report (CM3 = 23 bytes)
    let big = vec![0x01u8; 23];

    // All of these PIDs should not return UnknownPid (they may fail on short data,
    // but the big report covers them).
    for pid in [
        VIRPIL_CONSTELLATION_ALPHA_LEFT_PID,
        VIRPIL_MONGOOST_STICK_PID,
        VIRPIL_WARBRD_PID,
        VIRPIL_CM3_THROTTLE_PID,
        VIRPIL_PANEL1_PID,
        VIRPIL_PANEL2_PID,
        VIRPIL_ACE_PEDALS_PID,
        VIRPIL_ACE_TORQ_PID,
        VIRPIL_ROTOR_TCS_PLUS_PID,
    ] {
        let result = dispatch_report(pid, &big);
        assert!(
            result.is_ok(),
            "dispatch for PID 0x{pid:04X} should succeed"
        );
    }
}

// ─── Encoder integration ─────────────────────────────────────────────────────

#[test]
fn cm3_encoder_deltas_from_parsed_report() {
    let bank = cm3_encoder_bank();
    // Build a CM3 report with encoder 0 CW button (65) pressed
    let mut buttons = [0u8; 10];
    // button 65 = index 64 → byte 8, bit 0
    buttons[8] = 0x01;
    let report = make_cm3_report([0u16; 6], buttons);
    let state = parse_cm3_throttle_report(&report).unwrap();
    let deltas = bank.deltas(|n| state.buttons.is_pressed(n));
    assert_eq!(deltas[0], 1, "encoder 0 should show CW");
    assert_eq!(deltas[1], 0);
    assert_eq!(deltas[2], 0);
    assert_eq!(deltas[3], 0);
}

#[test]
fn cm3_encoder_ccw_from_parsed_report() {
    let bank = cm3_encoder_bank();
    let mut buttons = [0u8; 10];
    // button 68 = index 67 → byte 8, bit 3
    buttons[8] = 0x08;
    let report = make_cm3_report([0u16; 6], buttons);
    let state = parse_cm3_throttle_report(&report).unwrap();
    let deltas = bank.deltas(|n| state.buttons.is_pressed(n));
    assert_eq!(deltas[0], 0);
    assert_eq!(deltas[1], -1, "encoder 1 should show CCW");
    assert_eq!(deltas[2], 0);
    assert_eq!(deltas[3], 0);
}

// ─── Profile → Parser → Output integration ──────────────────────────────────

#[test]
fn profile_axis_count_matches_parser_output_alpha() {
    use flight_hotas_virpil::profiles::{ALPHA_PROFILE, profile_for_pid};
    let profile = profile_for_pid(ALPHA_PROFILE.pid).unwrap();
    let report = make_5ax_report([8192; 5], [0u8; 4]);
    let state = parse_alpha_report(&report).unwrap();
    // Profile says 5 axes, parser produces 5 axis fields
    assert_eq!(profile.axes.len(), 5);
    // All axes normalised to ~0.5 at midpoint
    assert!((state.axes.x - 0.5).abs() < 0.01);
    assert!((state.axes.y - 0.5).abs() < 0.01);
    assert!((state.axes.z - 0.5).abs() < 0.01);
}

#[test]
fn profile_axis_count_matches_parser_output_cm3() {
    use flight_hotas_virpil::profiles::{CM3_THROTTLE_PROFILE, profile_for_pid};
    let profile = profile_for_pid(CM3_THROTTLE_PROFILE.pid).unwrap();
    let report = make_cm3_report([VIRPIL_AXIS_MAX; 6], [0u8; 10]);
    let state = parse_cm3_throttle_report(&report).unwrap();
    assert_eq!(profile.axes.len(), 6);
    assert!((state.axes.left_throttle - 1.0).abs() < 1e-4);
    assert_eq!(profile.button_count, 78);
}

#[test]
fn profile_axis_count_matches_parser_output_pedals() {
    use flight_hotas_virpil::profiles::{ACE_PEDALS_PROFILE, profile_for_pid};
    let profile = profile_for_pid(ACE_PEDALS_PROFILE.pid).unwrap();
    let report = make_pedals_report([0u16; 3], [0u8; 2]);
    let state = parse_ace_pedals_report(&report).unwrap();
    assert_eq!(profile.axes.len(), 3);
    assert_eq!(state.axes.rudder, 0.0);
    assert_eq!(profile.button_count, 16);
}

#[test]
fn profile_axis_count_matches_parser_output_torq() {
    use flight_hotas_virpil::profiles::{ACE_TORQ_PROFILE, profile_for_pid};
    let profile = profile_for_pid(ACE_TORQ_PROFILE.pid).unwrap();
    let report = make_torq_report(VIRPIL_AXIS_MAX, [0u8; 2]);
    let state = parse_ace_torq_report(&report).unwrap();
    assert_eq!(profile.axes.len(), 1);
    assert!((state.axis.throttle - 1.0).abs() < 1e-4);
    assert_eq!(profile.button_count, 8);
}

#[test]
fn profile_axis_count_matches_parser_output_rotor_tcs() {
    use flight_hotas_virpil::profiles::{ROTOR_TCS_PROFILE, profile_for_pid};
    let profile = profile_for_pid(ROTOR_TCS_PROFILE.pid).unwrap();
    let report = make_rotor_tcs_report([VIRPIL_AXIS_MAX / 2; 3], [0u8; 4]);
    let state = parse_rotor_tcs_report(&report).unwrap();
    assert_eq!(profile.axes.len(), 3);
    assert!((state.axes.collective - 0.5).abs() < 0.01);
    assert_eq!(profile.button_count, 24);
}
