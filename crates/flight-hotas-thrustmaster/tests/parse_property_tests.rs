// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Property-based invariant tests for Thrustmaster HOTAS HID report parsing.
//!
//! Covers the Warthog stick/throttle, T.16000M joystick, TWCS throttle,
//! TFRP rudder pedals, and TPR pendular rudder parsers.

use flight_hotas_thrustmaster::t16000m::{T16000M_MIN_REPORT_BYTES, TWCS_MIN_REPORT_BYTES};
use flight_hotas_thrustmaster::{
    TFRP_MIN_REPORT_BYTES, THRUSTMASTER_VENDOR_ID, WARTHOG_STICK_MIN_REPORT_BYTES,
    WARTHOG_THROTTLE_MIN_REPORT_BYTES, parse_t16000m_report, parse_tfrp_report, parse_tpr_report,
    parse_twcs_report, parse_warthog_stick, parse_warthog_throttle,
};
use proptest::prelude::*;

// ─── Report builders ─────────────────────────────────────────────────────────

fn make_warthog_stick(x: u16, y: u16, rz: u16, btn_low: u16, btn_high: u8, hat: u8) -> [u8; 10] {
    let mut r = [0u8; 10];
    r[0..2].copy_from_slice(&x.to_le_bytes());
    r[2..4].copy_from_slice(&y.to_le_bytes());
    r[4..6].copy_from_slice(&rz.to_le_bytes());
    r[6..8].copy_from_slice(&btn_low.to_le_bytes());
    r[8] = btn_high;
    r[9] = hat;
    r
}

fn make_warthog_throttle(
    scx: u16,
    scy: u16,
    tl: u16,
    tr: u16,
    tc: u16,
    btn_low: u16,
    btn_mid: u16,
    btn_high: u8,
    toggles: u8,
    hat_dms: u8,
    hat_csl: u8,
) -> [u8; 20] {
    let mut r = [0u8; 20];
    r[0..2].copy_from_slice(&scx.to_le_bytes());
    r[2..4].copy_from_slice(&scy.to_le_bytes());
    r[4..6].copy_from_slice(&tl.to_le_bytes());
    r[6..8].copy_from_slice(&tr.to_le_bytes());
    r[8..10].copy_from_slice(&tc.to_le_bytes());
    r[10..12].copy_from_slice(&btn_low.to_le_bytes());
    r[12..14].copy_from_slice(&btn_mid.to_le_bytes());
    r[14] = btn_high;
    r[15] = toggles;
    r[16] = hat_dms;
    r[17] = hat_csl;
    r
}

fn make_t16000m(x: u16, y: u16, rz: u16, slider: u16, buttons: u16, hat: u8) -> [u8; 11] {
    let mut r = [0u8; 11];
    r[0..2].copy_from_slice(&x.to_le_bytes());
    r[2..4].copy_from_slice(&y.to_le_bytes());
    r[4..6].copy_from_slice(&rz.to_le_bytes());
    r[6..8].copy_from_slice(&slider.to_le_bytes());
    r[8..10].copy_from_slice(&buttons.to_le_bytes());
    r[10] = hat;
    r
}

fn make_twcs(throttle: u16, rx: u16, ry: u16, rz: u16, buttons: u16) -> [u8; 10] {
    let mut r = [0u8; 10];
    r[0..2].copy_from_slice(&throttle.to_le_bytes());
    r[2..4].copy_from_slice(&rx.to_le_bytes());
    r[4..6].copy_from_slice(&ry.to_le_bytes());
    r[6..8].copy_from_slice(&rz.to_le_bytes());
    r[8..10].copy_from_slice(&buttons.to_le_bytes());
    r
}

fn make_tfrp(rz: u16, z: u16, rx: u16) -> [u8; 6] {
    let mut r = [0u8; 6];
    r[0..2].copy_from_slice(&rz.to_le_bytes());
    r[2..4].copy_from_slice(&z.to_le_bytes());
    r[4..6].copy_from_slice(&rx.to_le_bytes());
    r
}

// ─── Warthog Joystick ────────────────────────────────────────────────────────

proptest! {
    /// All three Warthog stick axes are always within [-1.0, 1.0].
    #[test]
    fn prop_warthog_stick_axes_in_range(
        x in 0u16..=u16::MAX,
        y in 0u16..=u16::MAX,
        rz in 0u16..=u16::MAX,
    ) {
        let r = make_warthog_stick(x, y, rz, 0, 0, 0xFF);
        let s = parse_warthog_stick(&r).unwrap();
        prop_assert!((-1.0..=1.0).contains(&s.axes.x), "x={}", s.axes.x);
        prop_assert!((-1.0..=1.0).contains(&s.axes.y), "y={}", s.axes.y);
        prop_assert!((-1.0..=1.0).contains(&s.axes.rz), "rz={}", s.axes.rz);
    }

    /// Warthog stick axes are always finite (no NaN or Inf).
    #[test]
    fn prop_warthog_stick_axes_finite(
        x in 0u16..=u16::MAX,
        y in 0u16..=u16::MAX,
        rz in 0u16..=u16::MAX,
    ) {
        let r = make_warthog_stick(x, y, rz, 0, 0, 0xFF);
        let s = parse_warthog_stick(&r).unwrap();
        prop_assert!(s.axes.x.is_finite(), "x not finite: {}", s.axes.x);
        prop_assert!(s.axes.y.is_finite(), "y not finite: {}", s.axes.y);
        prop_assert!(s.axes.rz.is_finite(), "rz not finite: {}", s.axes.rz);
    }

    /// Reports shorter than the minimum always return an error.
    #[test]
    fn prop_warthog_stick_short_errors(
        data in proptest::collection::vec(any::<u8>(), 0..WARTHOG_STICK_MIN_REPORT_BYTES),
    ) {
        prop_assert!(parse_warthog_stick(&data).is_err());
    }

    /// button() query result is consistent with the raw bitmask for all inputs.
    #[test]
    fn prop_warthog_stick_buttons_consistent(
        buttons_low in any::<u16>(),
        buttons_high in any::<u8>(),
    ) {
        let r = make_warthog_stick(32768, 32768, 32768, buttons_low, buttons_high, 0xFF);
        let s = parse_warthog_stick(&r).unwrap();
        for n in 1u8..=16 {
            let expected = (buttons_low >> (n - 1)) & 1 != 0;
            prop_assert_eq!(s.buttons.button(n), expected, "stick button {}", n);
        }
        for n in 17u8..=19 {
            let expected = (buttons_high >> (n - 17)) & 1 != 0;
            prop_assert_eq!(s.buttons.button(n), expected, "stick button {}", n);
        }
        // Out-of-range queries always return false.
        prop_assert!(!s.buttons.button(0));
        prop_assert!(!s.buttons.button(20));
    }

    /// Arbitrary reports of valid length must not panic.
    #[test]
    fn prop_warthog_stick_no_panic(
        data in proptest::collection::vec(any::<u8>(), WARTHOG_STICK_MIN_REPORT_BYTES..128),
    ) {
        let _ = parse_warthog_stick(&data);
    }
}

// ─── Warthog Throttle ────────────────────────────────────────────────────────

proptest! {
    /// Warthog throttle bipolar axes (slew) are always within [-1.0, 1.0].
    #[test]
    fn prop_warthog_throttle_slew_in_range(
        scx in 0u16..=u16::MAX,
        scy in 0u16..=u16::MAX,
    ) {
        let r = make_warthog_throttle(scx, scy, 0, 0, 0, 0, 0, 0, 0, 0xFF, 0xFF);
        let s = parse_warthog_throttle(&r).unwrap();
        prop_assert!((-1.0..=1.0).contains(&s.axes.slew_x), "slew_x={}", s.axes.slew_x);
        prop_assert!((-1.0..=1.0).contains(&s.axes.slew_y), "slew_y={}", s.axes.slew_y);
    }

    /// Warthog throttle unipolar axes are always within [0.0, 1.0].
    #[test]
    fn prop_warthog_throttle_unipolar_in_range(
        tl in 0u16..=u16::MAX,
        tr in 0u16..=u16::MAX,
        tc in 0u16..=u16::MAX,
    ) {
        let r = make_warthog_throttle(32768, 32768, tl, tr, tc, 0, 0, 0, 0, 0xFF, 0xFF);
        let s = parse_warthog_throttle(&r).unwrap();
        prop_assert!(
            (0.0..=1.0).contains(&s.axes.throttle_left),
            "throttle_left={}", s.axes.throttle_left
        );
        prop_assert!(
            (0.0..=1.0).contains(&s.axes.throttle_right),
            "throttle_right={}", s.axes.throttle_right
        );
        prop_assert!(
            (0.0..=1.0).contains(&s.axes.throttle_combined),
            "throttle_combined={}", s.axes.throttle_combined
        );
    }

    /// Warthog throttle axes are always finite (no NaN or Inf).
    #[test]
    fn prop_warthog_throttle_axes_finite(
        scx in 0u16..=u16::MAX,
        tl in 0u16..=u16::MAX,
    ) {
        let r = make_warthog_throttle(scx, 32768, tl, tl, tl, 0, 0, 0, 0, 0xFF, 0xFF);
        let s = parse_warthog_throttle(&r).unwrap();
        prop_assert!(s.axes.slew_x.is_finite());
        prop_assert!(s.axes.slew_y.is_finite());
        prop_assert!(s.axes.throttle_left.is_finite());
        prop_assert!(s.axes.throttle_right.is_finite());
        prop_assert!(s.axes.throttle_combined.is_finite());
    }

    /// Reports shorter than the minimum always return an error.
    #[test]
    fn prop_warthog_throttle_short_errors(
        data in proptest::collection::vec(any::<u8>(), 0..WARTHOG_THROTTLE_MIN_REPORT_BYTES),
    ) {
        prop_assert!(parse_warthog_throttle(&data).is_err());
    }

    /// button() query result is consistent with the raw bitmask for all inputs.
    #[test]
    fn prop_warthog_throttle_buttons_consistent(
        btn_low in any::<u16>(),
        btn_mid in any::<u16>(),
        btn_high in any::<u8>(),
    ) {
        let r = make_warthog_throttle(
            32768, 32768, 0, 0, 0, btn_low, btn_mid, btn_high, 0, 0xFF, 0xFF,
        );
        let s = parse_warthog_throttle(&r).unwrap();
        for n in 1u8..=16 {
            let expected = (btn_low >> (n - 1)) & 1 != 0;
            prop_assert_eq!(s.buttons.button(n), expected, "throttle button {}", n);
        }
        for n in 17u8..=32 {
            let expected = (btn_mid >> (n - 17)) & 1 != 0;
            prop_assert_eq!(s.buttons.button(n), expected, "throttle button {}", n);
        }
        for n in 33u8..=40 {
            let expected = (btn_high >> (n - 33)) & 1 != 0;
            prop_assert_eq!(s.buttons.button(n), expected, "throttle button {}", n);
        }
        // Out-of-range queries always return false.
        prop_assert!(!s.buttons.button(0));
        prop_assert!(!s.buttons.button(41));
    }
}

// ─── T.16000M FCS Joystick ───────────────────────────────────────────────────

proptest! {
    /// T.16000M bipolar axes are in [-1.0, 1.0]; throttle lever is in [0.0, 1.0].
    #[test]
    fn prop_t16000m_axes_in_range(
        x in 0u16..=16383u16,
        y in 0u16..=16383u16,
        rz in 0u16..=16383u16,
        slider in 0u16..=u16::MAX,
    ) {
        let r = make_t16000m(x, y, rz, slider, 0, 0x0F);
        let s = parse_t16000m_report(&r).unwrap();
        prop_assert!((-1.0..=1.0).contains(&s.axes.x), "x={}", s.axes.x);
        prop_assert!((-1.0..=1.0).contains(&s.axes.y), "y={}", s.axes.y);
        prop_assert!((-1.0..=1.0).contains(&s.axes.twist), "twist={}", s.axes.twist);
        prop_assert!(
            (0.0..=1.0).contains(&s.axes.throttle),
            "throttle={}", s.axes.throttle
        );
    }

    /// T.16000M axes are always finite (no NaN or Inf) for any raw u16 input.
    #[test]
    fn prop_t16000m_axes_finite(
        x in 0u16..=u16::MAX,
        slider in 0u16..=u16::MAX,
    ) {
        let r = make_t16000m(x, x, x, slider, 0, 0x0F);
        let s = parse_t16000m_report(&r).unwrap();
        prop_assert!(s.axes.x.is_finite(), "x not finite: {}", s.axes.x);
        prop_assert!(s.axes.y.is_finite(), "y not finite: {}", s.axes.y);
        prop_assert!(s.axes.twist.is_finite(), "twist not finite: {}", s.axes.twist);
        prop_assert!(s.axes.throttle.is_finite(), "throttle not finite: {}", s.axes.throttle);
    }

    /// Reports strictly shorter than the minimum always return an error.
    #[test]
    fn prop_t16000m_short_errors(
        data in proptest::collection::vec(any::<u8>(), 0..T16000M_MIN_REPORT_BYTES),
    ) {
        prop_assert!(parse_t16000m_report(&data).is_err());
    }

    /// The raw button field round-trips through the parser unchanged.
    #[test]
    fn prop_t16000m_buttons_roundtrip(buttons in any::<u16>()) {
        let r = make_t16000m(0, 0, 0, 0, buttons, 0x0F);
        let s = parse_t16000m_report(&r).unwrap();
        prop_assert_eq!(s.buttons.buttons, buttons);
    }
}

// ─── TWCS Throttle ───────────────────────────────────────────────────────────

proptest! {
    /// TWCS throttle lever is in [0.0, 1.0]; bipolar axes are in [-1.0, 1.0].
    #[test]
    fn prop_twcs_axes_in_range(
        throttle in 0u16..=u16::MAX,
        rx in 0u16..=u16::MAX,
        ry in 0u16..=u16::MAX,
        rz in 0u16..=u16::MAX,
    ) {
        let r = make_twcs(throttle, rx, ry, rz, 0);
        let s = parse_twcs_report(&r).unwrap();
        prop_assert!(
            (0.0..=1.0).contains(&s.axes.throttle),
            "throttle={}", s.axes.throttle
        );
        prop_assert!(
            (-1.0..=1.0).contains(&s.axes.mini_stick_x),
            "mini_stick_x={}", s.axes.mini_stick_x
        );
        prop_assert!(
            (-1.0..=1.0).contains(&s.axes.mini_stick_y),
            "mini_stick_y={}", s.axes.mini_stick_y
        );
        prop_assert!(
            (-1.0..=1.0).contains(&s.axes.rocker),
            "rocker={}", s.axes.rocker
        );
    }

    /// TWCS axes are always finite (no NaN or Inf).
    #[test]
    fn prop_twcs_axes_finite(
        throttle in 0u16..=u16::MAX,
        rx in 0u16..=u16::MAX,
    ) {
        let r = make_twcs(throttle, rx, rx, rx, 0);
        let s = parse_twcs_report(&r).unwrap();
        prop_assert!(s.axes.throttle.is_finite());
        prop_assert!(s.axes.mini_stick_x.is_finite());
        prop_assert!(s.axes.mini_stick_y.is_finite());
        prop_assert!(s.axes.rocker.is_finite());
    }

    /// Reports strictly shorter than the minimum always return an error.
    #[test]
    fn prop_twcs_short_errors(
        data in proptest::collection::vec(any::<u8>(), 0..TWCS_MIN_REPORT_BYTES),
    ) {
        prop_assert!(parse_twcs_report(&data).is_err());
    }

    /// The button mask is always limited to 14 bits (bits 14-15 are cleared).
    #[test]
    fn prop_twcs_buttons_masked(buttons in any::<u16>()) {
        let r = make_twcs(0, 32768, 32768, 32768, buttons);
        let s = parse_twcs_report(&r).unwrap();
        prop_assert_eq!(
            s.buttons.buttons,
            buttons & 0x3FFF,
            "expected {:016b} & 0x3FFF = {:016b}, got {:016b}",
            buttons, buttons & 0x3FFF, s.buttons.buttons
        );
    }
}

// ─── TFRP Rudder Pedals ──────────────────────────────────────────────────────

proptest! {
    /// All three TFRP axes are always within [0.0, 1.0].
    #[test]
    fn prop_tfrp_axes_in_range(
        rz in 0u16..=u16::MAX,
        z in 0u16..=u16::MAX,
        rx in 0u16..=u16::MAX,
    ) {
        let r = make_tfrp(rz, z, rx);
        let s = parse_tfrp_report(&r).unwrap();
        prop_assert!((0.0..=1.0).contains(&s.axes.rudder), "rudder={}", s.axes.rudder);
        prop_assert!(
            (0.0..=1.0).contains(&s.axes.right_pedal),
            "right_pedal={}", s.axes.right_pedal
        );
        prop_assert!(
            (0.0..=1.0).contains(&s.axes.left_pedal),
            "left_pedal={}", s.axes.left_pedal
        );
    }

    /// TFRP axes are always finite (no NaN or Inf).
    #[test]
    fn prop_tfrp_axes_finite(rz in 0u16..=u16::MAX, z in 0u16..=u16::MAX) {
        let r = make_tfrp(rz, z, rz);
        let s = parse_tfrp_report(&r).unwrap();
        prop_assert!(s.axes.rudder.is_finite());
        prop_assert!(s.axes.right_pedal.is_finite());
        prop_assert!(s.axes.left_pedal.is_finite());
    }

    /// Reports shorter than the minimum always return an error.
    #[test]
    fn prop_tfrp_short_errors(
        data in proptest::collection::vec(any::<u8>(), 0..TFRP_MIN_REPORT_BYTES),
    ) {
        prop_assert!(parse_tfrp_report(&data).is_err());
    }
}

// ─── TPR Pendular Rudder ─────────────────────────────────────────────────────

proptest! {
    /// TPR (same HID layout as TFRP) axes are always within [0.0, 1.0].
    #[test]
    fn prop_tpr_axes_in_range(
        rz in 0u16..=u16::MAX,
        z in 0u16..=u16::MAX,
        rx in 0u16..=u16::MAX,
    ) {
        let r = make_tfrp(rz, z, rx);
        let s = parse_tpr_report(&r).unwrap();
        prop_assert!((0.0..=1.0).contains(&s.axes.rudder));
        prop_assert!((0.0..=1.0).contains(&s.axes.right_pedal));
        prop_assert!((0.0..=1.0).contains(&s.axes.left_pedal));
    }
}

// ─── Misc ────────────────────────────────────────────────────────────────────

#[test]
fn vendor_id_is_thrustmaster() {
    assert_eq!(THRUSTMASTER_VENDOR_ID, 0x044F);
}

#[test]
fn warthog_stick_centered_axes_near_zero() {
    let r = make_warthog_stick(32768, 32768, 32768, 0, 0, 0xFF);
    let s = parse_warthog_stick(&r).unwrap();
    assert!(s.axes.x.abs() < 0.001, "x not near 0: {}", s.axes.x);
    assert!(s.axes.y.abs() < 0.001, "y not near 0: {}", s.axes.y);
    assert!(s.axes.rz.abs() < 0.001, "rz not near 0: {}", s.axes.rz);
}
