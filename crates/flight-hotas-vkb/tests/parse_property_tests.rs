// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Property-based tests for VKB STECS, Gladiator NXT EVO, and STECS Modern Throttle parsing.

use flight_hotas_vkb::{
    GladiatorInputHandler, GladiatorParseError, GunfighterInputHandler, GunfighterParseError,
    GunfighterVariant, SemThqInputHandler, SemThqParseError, StecsInputHandler, StecsMtParseError,
    StecsMtVariant, StecsParseError, T_RUDDER_MIN_PAYLOAD_BYTES, TRudderInputHandler,
    TRudderParseError, TRudderVariant, VKB_VENDOR_ID, VKC_STECS_MT_MIN_REPORT_BYTES,
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
    roll: u16,
    pitch: u16,
    yaw: u16,
    mini_x: u16,
    mini_y: u16,
    throttle: u16,
    btn_lo: u32,
    btn_hi: u32,
    hat: u8,
) -> [u8; 21] {
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

// ─── Gunfighter ───────────────────────────────────────────────────────────────

fn make_gunfighter_report_prop(
    roll: u16,
    pitch: u16,
    yaw: u16,
    mx: u16,
    my: u16,
    throttle: u16,
    btn_lo: u32,
    btn_hi: u32,
    hat: u8,
) -> [u8; 21] {
    // Same layout as Gladiator
    make_gladiator_report(roll, pitch, yaw, mx, my, throttle, btn_lo, btn_hi, hat)
}

proptest! {
    /// Gunfighter signed axes are always in [-1.0, 1.0].
    #[test]
    fn prop_gunfighter_signed_axes_in_range(
        roll  in 0u16..=u16::MAX,
        pitch in 0u16..=u16::MAX,
        yaw   in 0u16..=u16::MAX,
        mx    in 0u16..=u16::MAX,
        my    in 0u16..=u16::MAX,
    ) {
        let handler = GunfighterInputHandler::new(GunfighterVariant::ModernCombatPro);
        let report = make_gunfighter_report_prop(roll, pitch, yaw, mx, my, 0x8000, 0, 0, 0xFF);
        let state = handler.parse_report(&report).unwrap();
        prop_assert!((-1.0..=1.0).contains(&state.axes.roll));
        prop_assert!((-1.0..=1.0).contains(&state.axes.pitch));
        prop_assert!((-1.0..=1.0).contains(&state.axes.yaw));
        prop_assert!((-1.0..=1.0).contains(&state.axes.mini_x));
        prop_assert!((-1.0..=1.0).contains(&state.axes.mini_y));
    }

    /// Gunfighter throttle is always in [0.0, 1.0].
    #[test]
    fn prop_gunfighter_throttle_in_range(throttle in 0u16..=u16::MAX) {
        let handler = GunfighterInputHandler::new(GunfighterVariant::SpaceGunfighter);
        let report = make_gunfighter_report_prop(0x8000, 0x8000, 0x8000, 0x8000, 0x8000, throttle, 0, 0, 0xFF);
        let state = handler.parse_report(&report).unwrap();
        prop_assert!((0.0..=1.0).contains(&state.axes.throttle));
    }

    /// Gunfighter button bits are correctly reflected.
    #[test]
    fn prop_gunfighter_button_bits_match(
        btn_lo in 0u32..=u32::MAX,
        btn_hi in 0u32..=u32::MAX,
    ) {
        let handler = GunfighterInputHandler::new(GunfighterVariant::ModernCombatPro);
        let report = make_gunfighter_report_prop(0x8000, 0x8000, 0x8000, 0x8000, 0x8000, 0, btn_lo, btn_hi, 0xFF);
        let state = handler.parse_report(&report).unwrap();
        for bit in 0..32usize {
            let expected = ((btn_lo >> bit) & 1) != 0;
            prop_assert_eq!(state.buttons[bit], expected, "buttons[{}]", bit);
        }
        for bit in 0..32usize {
            let expected = ((btn_hi >> bit) & 1) != 0;
            prop_assert_eq!(state.buttons[32 + bit], expected, "buttons[{}]", 32 + bit);
        }
    }

    /// Gunfighter short reports always error.
    #[test]
    fn prop_gunfighter_short_report_errors(len in 0usize..12usize) {
        let handler = GunfighterInputHandler::new(GunfighterVariant::ModernCombatPro);
        let data = vec![0u8; len];
        prop_assert!(
            matches!(
                handler.parse_report(&data),
                Err(GunfighterParseError::ReportTooShort { .. })
            ),
            "expected TooShort for len={}", len
        );
    }
}

// ─── SEM THQ ──────────────────────────────────────────────────────────────────

fn make_sem_thq_report_prop(axes: [u16; 4], btn_lo: u32, btn_hi: u32) -> [u8; 16] {
    let mut r = [0u8; 16];
    for (i, &v) in axes.iter().enumerate() {
        r[i * 2..i * 2 + 2].copy_from_slice(&v.to_le_bytes());
    }
    r[8..12].copy_from_slice(&btn_lo.to_le_bytes());
    r[12..16].copy_from_slice(&btn_hi.to_le_bytes());
    r
}

proptest! {
    /// SEM THQ axes are always in [0.0, 1.0].
    #[test]
    fn prop_sem_thq_axes_in_range(
        tl in 0u16..=u16::MAX,
        tr in 0u16..=u16::MAX,
        rl in 0u16..=u16::MAX,
        rr in 0u16..=u16::MAX,
    ) {
        let handler = SemThqInputHandler::new();
        let report = make_sem_thq_report_prop([tl, tr, rl, rr], 0, 0);
        let state = handler.parse_report(&report).unwrap();
        prop_assert!((0.0..=1.0).contains(&state.axes.throttle_left));
        prop_assert!((0.0..=1.0).contains(&state.axes.throttle_right));
        prop_assert!((0.0..=1.0).contains(&state.axes.rotary_left));
        prop_assert!((0.0..=1.0).contains(&state.axes.rotary_right));
    }

    /// SEM THQ axes are always finite.
    #[test]
    fn prop_sem_thq_axes_finite(
        tl in 0u16..=u16::MAX,
        tr in 0u16..=u16::MAX,
        rl in 0u16..=u16::MAX,
        rr in 0u16..=u16::MAX,
    ) {
        let handler = SemThqInputHandler::new();
        let report = make_sem_thq_report_prop([tl, tr, rl, rr], 0, 0);
        let state = handler.parse_report(&report).unwrap();
        prop_assert!(state.axes.throttle_left.is_finite());
        prop_assert!(state.axes.throttle_right.is_finite());
        prop_assert!(state.axes.rotary_left.is_finite());
        prop_assert!(state.axes.rotary_right.is_finite());
    }

    /// SEM THQ button bits are correctly reflected.
    #[test]
    fn prop_sem_thq_button_bits_match(
        btn_lo in 0u32..=u32::MAX,
        btn_hi in 0u32..=u32::MAX,
    ) {
        let handler = SemThqInputHandler::new();
        let report = make_sem_thq_report_prop([0; 4], btn_lo, btn_hi);
        let state = handler.parse_report(&report).unwrap();
        for bit in 0..32usize {
            let expected = ((btn_lo >> bit) & 1) != 0;
            prop_assert_eq!(state.buttons[bit], expected, "buttons[{}]", bit);
        }
        for bit in 0..32usize {
            let expected = ((btn_hi >> bit) & 1) != 0;
            prop_assert_eq!(state.buttons[32 + bit], expected, "buttons[{}]", 32 + bit);
        }
    }

    /// SEM THQ short reports always error.
    #[test]
    fn prop_sem_thq_short_report_errors(len in 0usize..16usize) {
        let handler = SemThqInputHandler::new();
        let data = vec![0u8; len];
        prop_assert!(
            matches!(
                handler.parse_report(&data),
                Err(SemThqParseError::ReportTooShort { .. })
            ),
            "expected TooShort for len={}", len
        );
    }
}

// ─── T-Rudder ─────────────────────────────────────────────────────────────────

fn make_t_rudder_report_prop(
    left: u16,
    right: u16,
    rudder: u16,
) -> [u8; T_RUDDER_MIN_PAYLOAD_BYTES] {
    let mut r = [0u8; T_RUDDER_MIN_PAYLOAD_BYTES];
    r[0..2].copy_from_slice(&left.to_le_bytes());
    r[2..4].copy_from_slice(&right.to_le_bytes());
    r[4..6].copy_from_slice(&rudder.to_le_bytes());
    r
}

proptest! {
    /// T-Rudder toe brakes are always in [0.0, 1.0].
    #[test]
    fn prop_t_rudder_toe_brakes_in_range(
        left  in 0u16..=u16::MAX,
        right in 0u16..=u16::MAX,
    ) {
        let handler = TRudderInputHandler::new(TRudderVariant::Mk4);
        let report = make_t_rudder_report_prop(left, right, 0x8000);
        let state = handler.parse_report(&report).unwrap();
        prop_assert!((0.0..=1.0).contains(&state.axes.left_toe_brake));
        prop_assert!((0.0..=1.0).contains(&state.axes.right_toe_brake));
    }

    /// T-Rudder rudder axis is always in [-1.0, 1.0].
    #[test]
    fn prop_t_rudder_rudder_in_range(rudder in 0u16..=u16::MAX) {
        let handler = TRudderInputHandler::new(TRudderVariant::Mk5);
        let report = make_t_rudder_report_prop(0, 0, rudder);
        let state = handler.parse_report(&report).unwrap();
        prop_assert!((-1.0..=1.0).contains(&state.axes.rudder));
    }

    /// T-Rudder all axes are always finite.
    #[test]
    fn prop_t_rudder_axes_finite(
        left   in 0u16..=u16::MAX,
        right  in 0u16..=u16::MAX,
        rudder in 0u16..=u16::MAX,
    ) {
        let handler = TRudderInputHandler::new(TRudderVariant::Mk4);
        let report = make_t_rudder_report_prop(left, right, rudder);
        let state = handler.parse_report(&report).unwrap();
        prop_assert!(state.axes.left_toe_brake.is_finite());
        prop_assert!(state.axes.right_toe_brake.is_finite());
        prop_assert!(state.axes.rudder.is_finite());
    }

    /// T-Rudder short reports always error.
    #[test]
    fn prop_t_rudder_short_report_errors(len in 0usize..T_RUDDER_MIN_PAYLOAD_BYTES) {
        let handler = TRudderInputHandler::new(TRudderVariant::Mk4);
        let data = vec![0u8; len];
        prop_assert!(
            matches!(
                handler.parse_report(&data),
                Err(TRudderParseError::ReportTooShort { .. })
            ),
            "expected TooShort for len={}", len
        );
    }
}

// ─── Round-trip tests ─────────────────────────────────────────────────────────

/// Build raw bytes from parsed Gladiator state and re-parse — axes and buttons must match.
#[test]
fn round_trip_gladiator_parse_serialize_parse() {
    let handler = GladiatorInputHandler::new(VkbGladiatorVariant::NxtEvoRight);
    let original = make_gladiator_report(
        0x1234,
        0x5678,
        0xABCD,
        0x1111,
        0x2222,
        0x3333,
        0xDEAD_BEEF,
        0xCAFE_BABE,
        0x35,
    );
    let state1 = handler.parse_report(&original).unwrap();

    // Serialize the parsed state back to bytes
    let mut rebuilt = [0u8; 21];
    // Re-encode axes: reverse the signed normalization
    let encode_signed = |v: f32| -> u16 { ((v + 1.0) * 32767.5).round() as u16 };
    let encode_unsigned = |v: f32| -> u16 { (v * u16::MAX as f32).round() as u16 };
    rebuilt[0..2].copy_from_slice(&encode_signed(state1.axes.roll).to_le_bytes());
    rebuilt[2..4].copy_from_slice(&encode_signed(state1.axes.pitch).to_le_bytes());
    rebuilt[4..6].copy_from_slice(&encode_signed(state1.axes.yaw).to_le_bytes());
    rebuilt[6..8].copy_from_slice(&encode_signed(state1.axes.mini_x).to_le_bytes());
    rebuilt[8..10].copy_from_slice(&encode_signed(state1.axes.mini_y).to_le_bytes());
    rebuilt[10..12].copy_from_slice(&encode_unsigned(state1.axes.throttle).to_le_bytes());
    // Re-encode buttons
    let mut btn_lo = 0u32;
    for bit in 0..32usize {
        if state1.buttons[bit] {
            btn_lo |= 1 << bit;
        }
    }
    let mut btn_hi = 0u32;
    for bit in 0..32usize {
        if state1.buttons[32 + bit] {
            btn_hi |= 1 << bit;
        }
    }
    rebuilt[12..16].copy_from_slice(&btn_lo.to_le_bytes());
    rebuilt[16..20].copy_from_slice(&btn_hi.to_le_bytes());
    // Hat
    let hat0 = state1.hats[0].map_or(0x0F, |h| h.0);
    let hat1 = state1.hats[1].map_or(0x0F, |h| h.0);
    rebuilt[20] = hat0 | (hat1 << 4);

    let state2 = handler.parse_report(&rebuilt).unwrap();
    // Axes should be within rounding tolerance
    assert!((state1.axes.roll - state2.axes.roll).abs() < 0.001);
    assert!((state1.axes.pitch - state2.axes.pitch).abs() < 0.001);
    assert!((state1.axes.yaw - state2.axes.yaw).abs() < 0.001);
    assert!((state1.axes.throttle - state2.axes.throttle).abs() < 0.001);
    // Buttons must be identical
    assert_eq!(state1.buttons, state2.buttons);
    // Hats must be identical
    assert_eq!(state1.hats, state2.hats);
}

/// Round-trip for SEM THQ.
#[test]
fn round_trip_sem_thq_parse_serialize_parse() {
    let handler = SemThqInputHandler::new();
    let original =
        make_sem_thq_report_prop([0x1234, 0x5678, 0xABCD, 0xEF01], 0xDEAD_BEEF, 0xCAFE_BABE);
    let state1 = handler.parse_report(&original).unwrap();

    let encode_unsigned = |v: f32| -> u16 { (v * u16::MAX as f32).round() as u16 };
    let mut rebuilt = [0u8; 16];
    rebuilt[0..2].copy_from_slice(&encode_unsigned(state1.axes.throttle_left).to_le_bytes());
    rebuilt[2..4].copy_from_slice(&encode_unsigned(state1.axes.throttle_right).to_le_bytes());
    rebuilt[4..6].copy_from_slice(&encode_unsigned(state1.axes.rotary_left).to_le_bytes());
    rebuilt[6..8].copy_from_slice(&encode_unsigned(state1.axes.rotary_right).to_le_bytes());
    let mut btn_lo = 0u32;
    for bit in 0..32usize {
        if state1.buttons[bit] {
            btn_lo |= 1 << bit;
        }
    }
    let mut btn_hi = 0u32;
    for bit in 0..32usize {
        if state1.buttons[32 + bit] {
            btn_hi |= 1 << bit;
        }
    }
    rebuilt[8..12].copy_from_slice(&btn_lo.to_le_bytes());
    rebuilt[12..16].copy_from_slice(&btn_hi.to_le_bytes());

    let state2 = handler.parse_report(&rebuilt).unwrap();
    assert!((state1.axes.throttle_left - state2.axes.throttle_left).abs() < 0.001);
    assert!((state1.axes.throttle_right - state2.axes.throttle_right).abs() < 0.001);
    assert_eq!(state1.buttons, state2.buttons);
}

/// Round-trip for T-Rudder.
#[test]
fn round_trip_t_rudder_parse_serialize_parse() {
    let handler = TRudderInputHandler::new(TRudderVariant::Mk4);
    let original = make_t_rudder_report_prop(0x1234, 0x5678, 0xABCD);
    let state1 = handler.parse_report(&original).unwrap();

    let encode_unsigned = |v: f32| -> u16 { (v * u16::MAX as f32).round() as u16 };
    let encode_signed = |v: f32| -> u16 { ((v + 1.0) * 32767.5).round() as u16 };
    let mut rebuilt = [0u8; 6];
    rebuilt[0..2].copy_from_slice(&encode_unsigned(state1.axes.left_toe_brake).to_le_bytes());
    rebuilt[2..4].copy_from_slice(&encode_unsigned(state1.axes.right_toe_brake).to_le_bytes());
    rebuilt[4..6].copy_from_slice(&encode_signed(state1.axes.rudder).to_le_bytes());

    let state2 = handler.parse_report(&rebuilt).unwrap();
    assert!((state1.axes.left_toe_brake - state2.axes.left_toe_brake).abs() < 0.001);
    assert!((state1.axes.right_toe_brake - state2.axes.right_toe_brake).abs() < 0.001);
    assert!((state1.axes.rudder - state2.axes.rudder).abs() < 0.001);
}
