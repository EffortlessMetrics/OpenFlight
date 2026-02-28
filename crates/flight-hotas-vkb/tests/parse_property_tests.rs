// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Property-based tests for VKB STECS, Gladiator NXT EVO, and STECS Modern Throttle parsing.

use flight_hotas_vkb::{
    GladiatorInputHandler, GladiatorParseError, StecsInputHandler, StecsMtParseError,
    StecsMtVariant, StecsParseError, VKB_VENDOR_ID, VKC_STECS_MT_MIN_REPORT_BYTES,
    VkbGladiatorVariant, VkbStecsVariant, parse_stecs_mt_report,
};
use proptest::prelude::*;

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn make_stecs_report(rx: u16, ry: u16, x: u16, y: u16, z: u16, buttons: u32) -> [u8; 14] {
    let mut r = [0u8; 14];
    r[0..2].copy_from_slice(&rx.to_le_bytes());
    r[2..4].copy_from_slice(&ry.to_le_bytes());
    r[4..6].copy_from_slice(&x.to_le_bytes());
    r[6..8].copy_from_slice(&y.to_le_bytes());
    r[8..10].copy_from_slice(&z.to_le_bytes());
    r[10..14].copy_from_slice(&buttons.to_le_bytes());
    r
}

#[allow(clippy::too_many_arguments)]
fn make_gladiator_report(
    let mut r = [0u8; 21];
    r[0..2].copy_from_slice(&roll.to_le_bytes());
    r[2..4].copy_from_slice(&pitch.to_le_bytes());
    r[4..6].copy_from_slice(&yaw.to_le_bytes());
    r[6..8].copy_from_slice(&mini_x.to_le_bytes());
    r[8..10].copy_from_slice(&mini_y.to_le_bytes());
    r[10..12].copy_from_slice(&throttle.to_le_bytes());
    r[12..16].copy_from_slice(&btn_lo.to_le_bytes());
    r[16..20].copy_from_slice(&btn_hi.to_le_bytes());
    r[20] = hat;
    r
}

fn make_stecs_mt_report(
    throttle: u16,
    mini_left: u16,
    mini_right: u16,
    rotary: u16,
    word0: u32,
    word1: u32,
) -> [u8; VKC_STECS_MT_MIN_REPORT_BYTES] {
    let mut r = [0u8; VKC_STECS_MT_MIN_REPORT_BYTES];
    r[0] = 0x01;
    r[1..3].copy_from_slice(&throttle.to_le_bytes());
    r[3..5].copy_from_slice(&mini_left.to_le_bytes());
    r[5..7].copy_from_slice(&mini_right.to_le_bytes());
    r[7..9].copy_from_slice(&rotary.to_le_bytes());
    r[9..13].copy_from_slice(&word0.to_le_bytes());
    r[13..17].copy_from_slice(&word1.to_le_bytes());
    r
}

// ─── STECS Interface ──────────────────────────────────────────────────────────

proptest! {
    /// STECS interface axes are always in [0.0, 1.0] for any raw u16 input.
    #[test]
    fn prop_stecs_axes_in_range(
        rx in 0u16..=u16::MAX,
        ry in 0u16..=u16::MAX,
        x  in 0u16..=u16::MAX,
        y  in 0u16..=u16::MAX,
        z  in 0u16..=u16::MAX,
    ) {
        let handler = StecsInputHandler::new(VkbStecsVariant::RightSpaceThrottleGripStandard);
        let report = make_stecs_report(rx, ry, x, y, z, 0);
        let state = handler.parse_interface_report(&report).unwrap();
        let axes = state.axes.expect("axes must be present in a 14-byte report");
        prop_assert!((0.0..=1.0).contains(&axes.rx), "rx={}", axes.rx);
        prop_assert!((0.0..=1.0).contains(&axes.ry), "ry={}", axes.ry);
        prop_assert!((0.0..=1.0).contains(&axes.x),  "x={}",  axes.x);
        prop_assert!((0.0..=1.0).contains(&axes.y),  "y={}",  axes.y);
        prop_assert!((0.0..=1.0).contains(&axes.z),  "z={}",  axes.z);
    }

    /// STECS axes are always finite (no NaN or Inf).
    #[test]
    fn prop_stecs_axes_finite(
        rx in 0u16..=u16::MAX,
        ry in 0u16..=u16::MAX,
        x  in 0u16..=u16::MAX,
        y  in 0u16..=u16::MAX,
        z  in 0u16..=u16::MAX,
    ) {
        let handler = StecsInputHandler::new(VkbStecsVariant::LeftSpaceThrottleGripMini);
        let report = make_stecs_report(rx, ry, x, y, z, 0);
        let state = handler.parse_interface_report(&report).unwrap();
        let axes = state.axes.expect("axes must be present");
        prop_assert!(axes.rx.is_finite(), "rx not finite");
        prop_assert!(axes.ry.is_finite(), "ry not finite");
        prop_assert!(axes.x.is_finite(),  "x  not finite");
        prop_assert!(axes.y.is_finite(),  "y  not finite");
        prop_assert!(axes.z.is_finite(),  "z  not finite");
    }

    /// Reports shorter than 4 payload bytes always return TooShort.
    #[test]
    fn prop_stecs_short_report_errors(len in 0usize..4usize) {
        let handler = StecsInputHandler::new(VkbStecsVariant::RightSpaceThrottleGripMini);
        let data = vec![0u8; len];
        prop_assert!(
            matches!(
                handler.parse_interface_report(&data),
                Err(StecsParseError::ReportTooShort { .. })
            ),
            "expected TooShort for len={}", len
        );
    }

    /// STECS button u32 is fully preserved through parsing.
    #[test]
    fn prop_stecs_button_mask_round_trips(mask in 0u32..=u32::MAX) {
        let handler = StecsInputHandler::new(VkbStecsVariant::RightSpaceThrottleGripStandard);
        let report = make_stecs_report(0, 0, 0, 0, 0, mask);
        let state = handler.parse_interface_report(&report).unwrap();
        prop_assert_eq!(state.buttons, mask);
    }
}

// ─── Gladiator NXT EVO ────────────────────────────────────────────────────────

proptest! {
    /// Signed axes (roll/pitch/yaw/mini_x/mini_y) are always in [-1.0, 1.0].
    #[test]
    fn prop_gladiator_signed_axes_in_range(
        roll  in 0u16..=u16::MAX,
        pitch in 0u16..=u16::MAX,
        yaw   in 0u16..=u16::MAX,
        mx    in 0u16..=u16::MAX,
        my    in 0u16..=u16::MAX,
    ) {
        let handler = GladiatorInputHandler::new(VkbGladiatorVariant::NxtEvoRight);
        let report = make_gladiator_report(roll, pitch, yaw, mx, my, 0x8000, 0, 0, 0xFF);
        let state = handler.parse_report(&report).unwrap();
        prop_assert!((-1.0..=1.0).contains(&state.axes.roll),   "roll={}",   state.axes.roll);
        prop_assert!((-1.0..=1.0).contains(&state.axes.pitch),  "pitch={}",  state.axes.pitch);
        prop_assert!((-1.0..=1.0).contains(&state.axes.yaw),    "yaw={}",    state.axes.yaw);
        prop_assert!((-1.0..=1.0).contains(&state.axes.mini_x), "mini_x={}", state.axes.mini_x);
        prop_assert!((-1.0..=1.0).contains(&state.axes.mini_y), "mini_y={}", state.axes.mini_y);
    }

    /// Throttle wheel is always in [0.0, 1.0].
    #[test]
    fn prop_gladiator_throttle_in_range(throttle in 0u16..=u16::MAX) {
        let handler = GladiatorInputHandler::new(VkbGladiatorVariant::NxtEvoLeft);
        let report =
            make_gladiator_report(0x8000, 0x8000, 0x8000, 0x8000, 0x8000, throttle, 0, 0, 0xFF);
        let state = handler.parse_report(&report).unwrap();
        prop_assert!(
            (0.0..=1.0).contains(&state.axes.throttle),
            "throttle={}", state.axes.throttle
        );
    }

    /// All Gladiator axes are always finite (no NaN or Inf).
    #[test]
    fn prop_gladiator_axes_finite(
        roll     in 0u16..=u16::MAX,
        pitch    in 0u16..=u16::MAX,
        yaw      in 0u16..=u16::MAX,
        throttle in 0u16..=u16::MAX,
    ) {
        let handler = GladiatorInputHandler::new(VkbGladiatorVariant::NxtEvoRight);
        let report =
            make_gladiator_report(roll, pitch, yaw, 0x8000, 0x8000, throttle, 0, 0, 0xFF);
        let state = handler.parse_report(&report).unwrap();
        prop_assert!(state.axes.roll.is_finite(),     "roll not finite");
        prop_assert!(state.axes.pitch.is_finite(),    "pitch not finite");
        prop_assert!(state.axes.yaw.is_finite(),      "yaw not finite");
        prop_assert!(state.axes.throttle.is_finite(), "throttle not finite");
    }

    /// Reports shorter than 12 bytes always return TooShort.
    #[test]
    fn prop_gladiator_short_report_errors(len in 0usize..12usize) {
        let handler = GladiatorInputHandler::new(VkbGladiatorVariant::NxtEvoRight);
        let data = vec![0u8; len];
        prop_assert!(
            matches!(
                handler.parse_report(&data),
                Err(GladiatorParseError::ReportTooShort { .. })
            ),
            "expected TooShort for len={}", len
        );
    }

    /// Button bits from btn_lo/btn_hi are correctly reflected in the bool array (64 buttons total).
    #[test]
    fn prop_gladiator_button_bits_match(
        btn_lo in 0u32..=u32::MAX,
        btn_hi in 0u32..=u32::MAX,
    ) {
        let handler = GladiatorInputHandler::new(VkbGladiatorVariant::NxtEvoRight);
        let report =
            make_gladiator_report(0x8000, 0x8000, 0x8000, 0x8000, 0x8000, 0, btn_lo, btn_hi, 0xFF);
        let state = handler.parse_report(&report).unwrap();
        for bit in 0..32usize {
            let expected = ((btn_lo >> bit) & 1) != 0;
            prop_assert_eq!(state.buttons[bit], expected, "buttons[{}] mismatch", bit);
        }
        for bit in 0..32usize {
            let expected = ((btn_hi >> bit) & 1) != 0;
            prop_assert_eq!(state.buttons[32 + bit], expected, "buttons[{}] mismatch", 32 + bit);
        }
    }
}

// ─── STECS Modern Throttle ────────────────────────────────────────────────────

proptest! {
    /// All STECS Modern Throttle axes are always in [0.0, 1.0].
    #[test]
    fn prop_stecs_mt_axes_in_range(
        throttle   in 0u16..=u16::MAX,
        mini_left  in 0u16..=u16::MAX,
        mini_right in 0u16..=u16::MAX,
        rotary     in 0u16..=u16::MAX,
    ) {
        let report = make_stecs_mt_report(throttle, mini_left, mini_right, rotary, 0, 0);
        let state = parse_stecs_mt_report(&report, StecsMtVariant::Mini).unwrap();
        prop_assert!((0.0..=1.0).contains(&state.axes.throttle));
        prop_assert!((0.0..=1.0).contains(&state.axes.mini_left));
        prop_assert!((0.0..=1.0).contains(&state.axes.mini_right));
        prop_assert!((0.0..=1.0).contains(&state.axes.rotary));
    }

    /// All STECS Modern Throttle axes are always finite (no NaN or Inf).
    #[test]
    fn prop_stecs_mt_axes_finite(
        throttle   in 0u16..=u16::MAX,
        mini_left  in 0u16..=u16::MAX,
        mini_right in 0u16..=u16::MAX,
        rotary     in 0u16..=u16::MAX,
    ) {
        let report = make_stecs_mt_report(throttle, mini_left, mini_right, rotary, 0, 0);
        let state = parse_stecs_mt_report(&report, StecsMtVariant::Max).unwrap();
        prop_assert!(state.axes.throttle.is_finite(),   "throttle not finite");
        prop_assert!(state.axes.mini_left.is_finite(),  "mini_left not finite");
        prop_assert!(state.axes.mini_right.is_finite(), "mini_right not finite");
        prop_assert!(state.axes.rotary.is_finite(),     "rotary not finite");
    }

    /// Reports shorter than VKC_STECS_MT_MIN_REPORT_BYTES always return TooShort.
    #[test]
    fn prop_stecs_mt_short_errors(len in 0usize..VKC_STECS_MT_MIN_REPORT_BYTES) {
        let data = vec![0x01u8; len];
        prop_assert!(
            matches!(
                parse_stecs_mt_report(&data, StecsMtVariant::Mini),
                Err(StecsMtParseError::TooShort(_))
            ),
            "expected TooShort for len={}", len
        );
    }

    /// Both STECS MT button words are fully preserved through parsing.
    #[test]
    fn prop_stecs_mt_buttons_preserve(
        word0 in 0u32..=u32::MAX,
        word1 in 0u32..=u32::MAX,
    ) {
        let report = make_stecs_mt_report(0, 0, 0, 0, word0, word1);
        let state = parse_stecs_mt_report(&report, StecsMtVariant::Mini).unwrap();
        prop_assert_eq!(state.buttons.word0, word0);
        prop_assert_eq!(state.buttons.word1, word1);
    }
}

// ─── Unit tests ───────────────────────────────────────────────────────────────

#[test]
fn vendor_id_is_vkb() {
    assert_eq!(VKB_VENDOR_ID, 0x231D);
}

#[test]
fn stecs_mt_variant_mini_preserved() {
    let report = make_stecs_mt_report(0, 0, 0, 0, 0, 0);
    let state = parse_stecs_mt_report(&report, StecsMtVariant::Mini).unwrap();
    assert_eq!(state.variant, StecsMtVariant::Mini);
}

#[test]
fn gladiator_hat_nibble_range() {
    let handler = GladiatorInputHandler::new(VkbGladiatorVariant::NxtEvoRight);
    // Hat nibbles 0..=7 produce Some, 8..=15 produce None (centred).
    for nibble in 0u8..=15 {
        let report = make_gladiator_report(0x8000, 0x8000, 0x8000, 0x8000, 0x8000, 0, 0, 0, nibble);
        let state = handler.parse_report(&report).unwrap();
        if nibble <= 7 {
            assert!(
                state.hats[0].is_some(),
                "nibble {nibble} should give Some hat"
            );
        } else {
            assert!(
                state.hats[0].is_none(),
                "nibble {nibble} should give None (centred)"
            );
        }
    }
}
