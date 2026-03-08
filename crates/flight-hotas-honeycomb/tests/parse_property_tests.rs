// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Property-based tests for Honeycomb Alpha Yoke and Bravo Throttle Quadrant HID report parsing.

use flight_hotas_honeycomb::alpha::ALPHA_REPORT_LEN;
use flight_hotas_honeycomb::bravo::BRAVO_REPORT_LEN;
use flight_hotas_honeycomb::button_delta::ButtonDelta;
use flight_hotas_honeycomb::{
    AlphaButton, AlphaParseError, BravoButton, BravoParseError, HONEYCOMB_ALPHA_YOKE_PID_LEGACY,
    HONEYCOMB_BRAVO_PID_LEGACY, HONEYCOMB_VENDOR_ID, HONEYCOMB_VENDOR_ID_LEGACY,
    honeycomb_model_from_vid_pid, is_honeycomb_device, parse_alpha_report, parse_bravo_report,
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
            (-1.001..=1.001).contains(&state.axes.roll),
            "roll out of range: {}", state.axes.roll
        );
    }

    /// Pitch axis is always in [-1.0, 1.0].
    #[test]
    fn prop_alpha_pitch_in_range(pitch in 0u16..=u16::MAX) {
        let state = parse_alpha_report(&make_alpha_report(2048, pitch, 0, 15)).unwrap();
        prop_assert!(
            (-1.001..=1.001).contains(&state.axes.pitch),
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

    /// Alpha button mask round-trips: button N pressed iff bit N-1 is set.
    #[test]
    fn prop_alpha_button_mask_roundtrip(
        mask_lo in 0u32..=u32::MAX,
        hat in 0u8..=15u8,
    ) {
        let mask = mask_lo as u64;
        let state = parse_alpha_report(&make_alpha_report(2048, 2048, mask, hat)).unwrap();
        // Only lower 40 bits are populated (5 button bytes)
        prop_assert_eq!(state.buttons.mask & 0xFF_FFFF_FFFF, mask & 0xFF_FFFF_FFFF);
    }

    /// Alpha roll/pitch are monotonically increasing with raw value.
    #[test]
    fn prop_alpha_roll_monotonic(a in 0u16..=4094u16) {
        let b = a + 1;
        let sa = parse_alpha_report(&make_alpha_report(a, 2048, 0, 15)).unwrap();
        let sb = parse_alpha_report(&make_alpha_report(b, 2048, 0, 15)).unwrap();
        prop_assert!(sb.axes.roll >= sa.axes.roll, "roll not monotonic");
    }

    /// Alpha pitch is monotonically increasing with raw value.
    #[test]
    fn prop_alpha_pitch_monotonic(a in 0u16..=4094u16) {
        let b = a + 1;
        let sa = parse_alpha_report(&make_alpha_report(2048, a, 0, 15)).unwrap();
        let sb = parse_alpha_report(&make_alpha_report(2048, b, 0, 15)).unwrap();
        prop_assert!(sb.axes.pitch >= sa.axes.pitch, "pitch not monotonic");
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
        prop_assert!((0.0..=1.0).contains(&state.axes.throttle1));
        prop_assert!((0.0..=1.0).contains(&state.axes.throttle2));
        prop_assert!((0.0..=1.0).contains(&state.axes.throttle3));
        prop_assert!((0.0..=1.0).contains(&state.axes.throttle4));
        prop_assert!((0.0..=1.0).contains(&state.axes.throttle5));
        prop_assert!((0.0..=1.0).contains(&state.axes.flap_lever));
        prop_assert!((0.0..=1.0).contains(&state.axes.spoiler));
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

    /// Bravo throttle axes are monotonically increasing with raw value.
    #[test]
    fn prop_bravo_throttle1_monotonic(a in 0u16..=4094u16) {
        let b = a + 1;
        let sa = parse_bravo_report(&make_bravo_report([a, 0, 0, 0, 0, 0, 0], 0)).unwrap();
        let sb = parse_bravo_report(&make_bravo_report([b, 0, 0, 0, 0, 0, 0], 0)).unwrap();
        prop_assert!(sb.axes.throttle1 >= sa.axes.throttle1, "throttle1 not monotonic");
    }

    /// Button delta: pressed and released masks are disjoint.
    #[test]
    fn prop_button_delta_disjoint(prev in 0u64..=u64::MAX, curr in 0u64..=u64::MAX) {
        let delta = ButtonDelta::compute(prev, curr);
        prop_assert_eq!(delta.pressed & delta.released, 0, "pressed and released overlap");
    }

    /// Button delta: pressed ∪ released ∪ unchanged = all buttons.
    #[test]
    fn prop_button_delta_partition(prev in 0u64..=u64::MAX, curr in 0u64..=u64::MAX) {
        let delta = ButtonDelta::compute(prev, curr);
        let unchanged = prev & curr;
        let still_off = !prev & !curr;
        // All 64 bits are accounted for
        prop_assert_eq!(
            delta.pressed | delta.released | unchanged | still_off,
            u64::MAX,
        );
    }
}

// ─── Unit tests ───────────────────────────────────────────────────────────────

#[test]
fn honeycomb_vendor_id_is_correct() {
    assert_eq!(HONEYCOMB_VENDOR_ID, 0x294B);
}

#[test]
fn legacy_vendor_id_is_microchip() {
    assert_eq!(HONEYCOMB_VENDOR_ID_LEGACY, 0x04D8);
}

#[test]
fn legacy_alpha_pid() {
    assert_eq!(HONEYCOMB_ALPHA_YOKE_PID_LEGACY, 0xE6D6);
}

#[test]
fn legacy_bravo_pid() {
    assert_eq!(HONEYCOMB_BRAVO_PID_LEGACY, 0xE6D5);
}

#[test]
fn is_honeycomb_device_legacy_alpha() {
    assert!(is_honeycomb_device(0x04D8, 0xE6D6));
}

#[test]
fn is_honeycomb_device_legacy_bravo() {
    assert!(is_honeycomb_device(0x04D8, 0xE6D5));
}

#[test]
fn legacy_vid_pid_model_detection() {
    use flight_hotas_honeycomb::HoneycombModel;
    assert_eq!(
        honeycomb_model_from_vid_pid(0x04D8, 0xE6D6),
        Some(HoneycombModel::AlphaYoke)
    );
    assert_eq!(
        honeycomb_model_from_vid_pid(0x04D8, 0xE6D5),
        Some(HoneycombModel::BravoThrottle)
    );
}

#[test]
fn unknown_vid_pid_returns_none() {
    assert_eq!(honeycomb_model_from_vid_pid(0x1234, 0x5678), None);
    assert!(!is_honeycomb_device(0x1234, 0x5678));
}

#[test]
fn alpha_hat_centred_maps_to_zero() {
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

#[test]
fn alpha_named_button_ptt_matches_bit0() {
    let mask: u64 = 1;
    let state = parse_alpha_report(&make_alpha_report(2048, 2048, mask, 15)).unwrap();
    assert!(AlphaButton::Ptt.is_active(&state.buttons));
}

#[test]
fn bravo_named_ap_master_matches_bit7() {
    let mask: u64 = 1 << 7;
    let state = parse_bravo_report(&make_bravo_report([0; 7], mask)).unwrap();
    assert!(BravoButton::ApMaster.is_active(&state.buttons));
}

#[test]
fn button_delta_single_press() {
    let delta = ButtonDelta::compute(0, 1 << 30);
    assert!(delta.was_pressed(31)); // bit 30 = button 31 (gear up)
    assert!(!delta.was_released(31));
}

#[test]
fn button_delta_single_release() {
    let delta = ButtonDelta::compute(1 << 7, 0);
    assert!(delta.was_released(8)); // bit 7 = button 8 (AP master)
}

#[test]
fn alpha_all_buttons_pressed() {
    let mask: u64 = (1u64 << 36) - 1; // bits 0-35 set
    let state = parse_alpha_report(&make_alpha_report(2048, 2048, mask, 15)).unwrap();
    for n in 1..=36u8 {
        assert!(state.buttons.is_pressed(n), "button {n} should be pressed");
    }
}

#[test]
fn bravo_toggle_switch_buttons_via_enum() {
    // Toggle 1 UP = button 34 = bit 33
    let mask: u64 = 1 << 33;
    let state = parse_bravo_report(&make_bravo_report([0; 7], mask)).unwrap();
    assert!(BravoButton::Toggle1Up.is_active(&state.buttons));
    assert!(!BravoButton::Toggle1Down.is_active(&state.buttons));
}
