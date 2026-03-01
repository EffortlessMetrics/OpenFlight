// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Depth tests for the `flight-hotas-vkb` crate.
//!
//! These integration tests exercise cross-module interactions, edge cases,
//! boundary conditions, and multi-step workflows across all VKB device families.

use flight_hotas_vkb::*;

// ═══════════════════════════════════════════════════════════════════════════════
// Helper functions
// ═══════════════════════════════════════════════════════════════════════════════

/// Build a Gladiator NXT EVO HID report (21 bytes, no report ID prefix).
fn make_gladiator_report(axes: [u16; 6], btn_lo: u32, btn_hi: u32, hat_byte: u8) -> Vec<u8> {
    let mut report = vec![0u8; 21];
    for (i, &v) in axes.iter().enumerate() {
        let bytes = v.to_le_bytes();
        report[i * 2] = bytes[0];
        report[i * 2 + 1] = bytes[1];
    }
    report[12..16].copy_from_slice(&btn_lo.to_le_bytes());
    report[16..20].copy_from_slice(&btn_hi.to_le_bytes());
    report[20] = hat_byte;
    report
}

/// Build a STECS interface report: 14 bytes (5 axes + 4 button bytes).
fn make_stecs_report_with_axes(axes: [u16; 5], buttons: u32) -> Vec<u8> {
    let mut report = vec![0u8; 14];
    for (i, &v) in axes.iter().enumerate() {
        let bytes = v.to_le_bytes();
        report[i * 2] = bytes[0];
        report[i * 2 + 1] = bytes[1];
    }
    report[10..14].copy_from_slice(&buttons.to_le_bytes());
    report
}

/// Build a STECS Modern Throttle report (17 bytes, includes report ID byte).
fn make_stecs_mt_report(
    throttle: u16,
    mini_left: u16,
    mini_right: u16,
    rotary: u16,
    word0: u32,
    word1: u32,
) -> Vec<u8> {
    let mut data = vec![0x01u8]; // report_id
    data.extend_from_slice(&throttle.to_le_bytes());
    data.extend_from_slice(&mini_left.to_le_bytes());
    data.extend_from_slice(&mini_right.to_le_bytes());
    data.extend_from_slice(&rotary.to_le_bytes());
    data.extend_from_slice(&word0.to_le_bytes());
    data.extend_from_slice(&word1.to_le_bytes());
    data
}

/// Build a Gunfighter HID report (21 bytes).
fn make_gunfighter_report(axes: [u16; 6], btn_lo: u32, btn_hi: u32, hat_byte: u8) -> Vec<u8> {
    let mut report = vec![0u8; 21];
    for (i, &v) in axes.iter().enumerate() {
        let bytes = v.to_le_bytes();
        report[i * 2] = bytes[0];
        report[i * 2 + 1] = bytes[1];
    }
    report[12..16].copy_from_slice(&btn_lo.to_le_bytes());
    report[16..20].copy_from_slice(&btn_hi.to_le_bytes());
    report[20] = hat_byte;
    report
}

// ═══════════════════════════════════════════════════════════════════════════════
// 1. STECS aggregator — multi-VC merge workflows
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn stecs_aggregator_begin_poll_clears_previous_state() {
    let mut agg = StecsInputAggregator::new(VkbStecsVariant::RightSpaceThrottleGripStandard);
    agg.begin_poll();
    agg.merge_interface_report(0, &make_stecs_report_with_axes([0xFFFF; 5], 0xFF))
        .unwrap();
    assert!(agg.snapshot().axes.is_some());

    agg.begin_poll();
    let snap = agg.snapshot();
    assert!(snap.axes.is_none(), "axes should be cleared after begin_poll");
    assert!(
        snap.pressed_buttons().is_empty(),
        "buttons should be cleared"
    );
    assert!(
        snap.active_virtual_controllers.iter().all(|&vc| !vc),
        "VCs should be inactive after begin_poll"
    );
}

#[test]
fn stecs_aggregator_three_vcs_all_marked_active() {
    let mut agg = StecsInputAggregator::new(VkbStecsVariant::LeftSpaceThrottleGripStandard);
    agg.begin_poll();
    for vc in 0..3u8 {
        agg.merge_interface_report(vc, &[0x00, 0x00, 0x00, 0x00])
            .unwrap();
    }
    let snap = agg.snapshot();
    assert!(snap.active_virtual_controllers[0]);
    assert!(snap.active_virtual_controllers[1]);
    assert!(snap.active_virtual_controllers[2]);
}

#[test]
fn stecs_aggregator_buttons_across_all_vcs_merge_correctly() {
    let mut agg = StecsInputAggregator::new(VkbStecsVariant::RightSpaceThrottleGripMini);
    agg.begin_poll();

    // VC0: button 1 (bit 0)
    agg.merge_interface_report(0, &[0x01, 0x00, 0x00, 0x00])
        .unwrap();
    // VC1: button 33 (bit 0 of VC1 → global index 32)
    agg.merge_interface_report(1, &[0x01, 0x00, 0x00, 0x00])
        .unwrap();
    // VC2: button 65 (bit 0 of VC2 → global index 64)
    agg.merge_interface_report(2, &[0x01, 0x00, 0x00, 0x00])
        .unwrap();

    let pressed = agg.snapshot().pressed_buttons();
    assert_eq!(pressed, vec![1, 33, 65]);
}

#[test]
fn stecs_aggregator_vc0_axes_preferred_over_vc1() {
    let mut agg = StecsInputAggregator::new(VkbStecsVariant::LeftSpaceThrottleGripMiniPlus);
    agg.begin_poll();

    // Feed VC1 first with low axis values
    let vc1_report = make_stecs_report_with_axes([0x1000, 0x1000, 0x1000, 0x1000, 0x1000], 0);
    agg.merge_interface_report(1, &vc1_report).unwrap();

    // Feed VC0 with high axis values
    let vc0_report = make_stecs_report_with_axes([0xE000, 0xE000, 0xE000, 0xE000, 0xE000], 0);
    agg.merge_interface_report(0, &vc0_report).unwrap();

    let axes = agg.snapshot().axes.expect("axes from VC0 expected");
    // VC0 axes should win since it has lower index
    assert!(axes.rx > 0.8, "VC0 axis should be used, got {}", axes.rx);
}

#[test]
fn stecs_aggregator_vc1_axes_used_if_vc0_has_no_axes() {
    let mut agg = StecsInputAggregator::new(VkbStecsVariant::RightSpaceThrottleGripMiniPlus);
    agg.begin_poll();

    // VC0: buttons-only (4 bytes, no axes)
    agg.merge_interface_report(0, &[0x00, 0x00, 0x00, 0x00])
        .unwrap();
    // VC1: full report with axes
    let vc1 = make_stecs_report_with_axes([0x8000, 0x8000, 0x8000, 0x8000, 0x8000], 0);
    agg.merge_interface_report(1, &vc1).unwrap();

    let axes = agg.snapshot().axes.expect("should have VC1 axes");
    assert!(
        (axes.z - 0.5).abs() < 0.01,
        "VC1 midpoint axis expected, got {}",
        axes.z
    );
}

#[test]
fn stecs_aggregator_variant_preserved() {
    let agg = StecsInputAggregator::new(VkbStecsVariant::LeftSpaceThrottleGripMini);
    assert_eq!(agg.variant(), VkbStecsVariant::LeftSpaceThrottleGripMini);
    assert_eq!(
        agg.snapshot().variant,
        VkbStecsVariant::LeftSpaceThrottleGripMini
    );
}

#[test]
fn stecs_handler_report_id_strips_first_byte() {
    let handler = StecsInputHandler::new(VkbStecsVariant::RightSpaceThrottleGripStandard)
        .with_report_id(true);
    // 1 byte report ID + 4 bytes payload
    let report = [0x01, 0x03, 0x00, 0x00, 0x00];
    let parsed = handler.parse_interface_report(&report).unwrap();
    assert_eq!(parsed.buttons, 0x0000_0003); // buttons 1 and 2
}

#[test]
fn stecs_handler_report_id_with_empty_payload_fails() {
    let handler = StecsInputHandler::new(VkbStecsVariant::LeftSpaceThrottleGripMini)
        .with_report_id(true);
    let report = [0x01]; // only report ID, no payload
    let err = handler.parse_interface_report(&report);
    assert!(matches!(
        err,
        Err(StecsParseError::ReportTooShort {
            expected: 4,
            actual: 0
        })
    ));
}

#[test]
fn stecs_max_button_index_96_reachable() {
    let mut agg = StecsInputAggregator::new(VkbStecsVariant::RightSpaceThrottleGripStandard);
    agg.begin_poll();
    // VC2: set bit 31 → global button 96
    agg.merge_interface_report(2, &[0x00, 0x00, 0x00, 0x80])
        .unwrap();
    let pressed = agg.snapshot().pressed_buttons();
    assert_eq!(pressed, vec![96]);
}

#[test]
fn stecs_multiple_polls_are_independent() {
    let mut agg = StecsInputAggregator::new(VkbStecsVariant::LeftSpaceThrottleGripStandard);

    // Poll 1: press button 1
    agg.begin_poll();
    agg.merge_interface_report(0, &[0x01, 0x00, 0x00, 0x00])
        .unwrap();
    assert_eq!(agg.snapshot().pressed_buttons(), vec![1]);

    // Poll 2: press button 2 (button 1 should not persist)
    agg.begin_poll();
    agg.merge_interface_report(0, &[0x02, 0x00, 0x00, 0x00])
        .unwrap();
    assert_eq!(agg.snapshot().pressed_buttons(), vec![2]);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 2. Gladiator NXT EVO — axis & button edge cases
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn gladiator_full_negative_deflection() {
    let handler = GladiatorInputHandler::new(VkbGladiatorVariant::NxtEvoRight);
    let report = make_gladiator_report([0x0000, 0x0000, 0x0000, 0x8000, 0x8000, 0], 0, 0, 0xFF);
    let state = handler.parse_report(&report).unwrap();
    assert!(
        (state.axes.roll - (-1.0)).abs() < 0.01,
        "roll should be -1.0 at 0x0000"
    );
    assert!(
        (state.axes.pitch - (-1.0)).abs() < 0.01,
        "pitch should be -1.0"
    );
    assert!(
        (state.axes.yaw - (-1.0)).abs() < 0.01,
        "yaw should be -1.0"
    );
}

#[test]
fn gladiator_full_positive_deflection() {
    let handler = GladiatorInputHandler::new(VkbGladiatorVariant::NxtEvoLeft);
    let report = make_gladiator_report([0xFFFF, 0xFFFF, 0xFFFF, 0x8000, 0x8000, 0xFFFF], 0, 0, 0xFF);
    let state = handler.parse_report(&report).unwrap();
    assert!((state.axes.roll - 1.0).abs() < 0.01);
    assert!((state.axes.pitch - 1.0).abs() < 0.01);
    assert!((state.axes.yaw - 1.0).abs() < 0.01);
    assert!((state.axes.throttle - 1.0).abs() < 0.001);
}

#[test]
fn gladiator_all_64_buttons_pressable() {
    let handler = GladiatorInputHandler::new(VkbGladiatorVariant::NxtEvoRight);
    let report = make_gladiator_report(
        [0x8000; 6],
        u32::MAX, // buttons 1–32
        u32::MAX, // buttons 33–64
        0xFF,
    );
    let state = handler.parse_report(&report).unwrap();
    assert_eq!(state.pressed_buttons().len(), 64);
    assert_eq!(*state.pressed_buttons().first().unwrap(), 1u16);
    assert_eq!(*state.pressed_buttons().last().unwrap(), 64u16);
}

#[test]
fn gladiator_no_buttons_when_zero() {
    let handler = GladiatorInputHandler::new(VkbGladiatorVariant::NxtEvoLeft);
    let report = make_gladiator_report([0x8000; 6], 0, 0, 0xFF);
    let state = handler.parse_report(&report).unwrap();
    assert!(state.pressed_buttons().is_empty());
}

#[test]
fn gladiator_hat_all_8_directions() {
    let handler = GladiatorInputHandler::new(VkbGladiatorVariant::NxtEvoRight);
    for direction in 0u8..=7 {
        let hat_byte = 0xF0 | direction; // hat0 = direction, hat1 = centred
        let report = make_gladiator_report([0x8000; 6], 0, 0, hat_byte);
        let state = handler.parse_report(&report).unwrap();
        assert_eq!(
            state.hats[0],
            Some(HatDirection(direction)),
            "hat0 should be direction {direction}"
        );
        assert_eq!(state.hats[1], None, "hat1 should be centred");
    }
}

#[test]
fn gladiator_both_hats_active() {
    let handler = GladiatorInputHandler::new(VkbGladiatorVariant::NxtEvoRight);
    let hat_byte = 0x40; // hat0=N(0), hat1=S(4)
    let report = make_gladiator_report([0x8000; 6], 0, 0, hat_byte);
    let state = handler.parse_report(&report).unwrap();
    assert_eq!(state.hats[0], Some(HatDirection(0)));
    assert_eq!(state.hats[1], Some(HatDirection(4)));
}

#[test]
fn gladiator_minimum_report_axes_only() {
    let handler = GladiatorInputHandler::new(VkbGladiatorVariant::NxtEvoRight);
    // Exactly 12 bytes — axes only, no buttons or hats
    let report = vec![0x00, 0x80, 0x00, 0x80, 0x00, 0x80, 0x00, 0x80, 0x00, 0x80, 0x00, 0x00];
    let state = handler.parse_report(&report).unwrap();
    assert!(state.axes.roll.abs() < 0.01);
    assert!(state.pressed_buttons().is_empty());
    assert_eq!(state.hats, [None, None]);
}

#[test]
fn gladiator_variant_preserved_in_state() {
    let handler_r = GladiatorInputHandler::new(VkbGladiatorVariant::NxtEvoRight);
    let handler_l = GladiatorInputHandler::new(VkbGladiatorVariant::NxtEvoLeft);
    let report = make_gladiator_report([0x8000; 6], 0, 0, 0xFF);
    assert_eq!(
        handler_r.parse_report(&report).unwrap().variant,
        VkbGladiatorVariant::NxtEvoRight
    );
    assert_eq!(
        handler_l.parse_report(&report).unwrap().variant,
        VkbGladiatorVariant::NxtEvoLeft
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// 3. Gunfighter — parsing & variant edge cases
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn gunfighter_all_variants_parse_same_report() {
    let variants = [
        GunfighterVariant::ModernCombatPro,
        GunfighterVariant::SpaceGunfighter,
        GunfighterVariant::SpaceGunfighterLeft,
    ];
    let report = make_gunfighter_report([0x8000; 6], 0x01, 0, 0xFF);
    for variant in variants {
        let handler = GunfighterInputHandler::new(variant);
        let state = handler.parse_report(&report).unwrap();
        assert!(state.buttons[0], "{:?} button 1 should be pressed", variant);
        assert_eq!(state.variant, variant);
    }
}

#[test]
fn gunfighter_full_deflection_all_axes() {
    let handler = GunfighterInputHandler::new(GunfighterVariant::SpaceGunfighter);
    let report = make_gunfighter_report(
        [0xFFFF, 0x0000, 0xFFFF, 0x0000, 0xFFFF, 0xFFFF],
        0,
        0,
        0xFF,
    );
    let state = handler.parse_report(&report).unwrap();
    assert!((state.axes.roll - 1.0).abs() < 0.01);
    assert!((state.axes.pitch - (-1.0)).abs() < 0.01);
    assert!((state.axes.yaw - 1.0).abs() < 0.01);
    assert!((state.axes.mini_x - (-1.0)).abs() < 0.01);
    assert!((state.axes.mini_y - 1.0).abs() < 0.01);
    assert!((state.axes.throttle - 1.0).abs() < 0.001);
}

#[test]
fn gunfighter_hat_both_active() {
    let handler = GunfighterInputHandler::new(GunfighterVariant::ModernCombatPro);
    // hat0=E(2), hat1=W(6)
    let report = make_gunfighter_report([0x8000; 6], 0, 0, 0x62);
    let state = handler.parse_report(&report).unwrap();
    assert_eq!(state.hats[0], Some(2));
    assert_eq!(state.hats[1], Some(6));
}

#[test]
fn gunfighter_all_64_buttons() {
    let handler = GunfighterInputHandler::new(GunfighterVariant::SpaceGunfighterLeft);
    let report = make_gunfighter_report([0x8000; 6], u32::MAX, u32::MAX, 0xFF);
    let state = handler.parse_report(&report).unwrap();
    assert_eq!(state.pressed_buttons().len(), 64);
}

#[test]
fn gunfighter_minimum_report_12_bytes() {
    let handler = GunfighterInputHandler::new(GunfighterVariant::ModernCombatPro);
    let report = vec![0x00, 0x80, 0x00, 0x80, 0x00, 0x80, 0x00, 0x80, 0x00, 0x80, 0x00, 0x00];
    let state = handler.parse_report(&report).unwrap();
    assert!(state.axes.roll.abs() < 0.01);
    assert!(state.pressed_buttons().is_empty());
}

#[test]
fn gunfighter_variant_name_not_empty() {
    assert!(!GunfighterVariant::ModernCombatPro.name().is_empty());
    assert!(!GunfighterVariant::SpaceGunfighter.name().is_empty());
    assert!(!GunfighterVariant::SpaceGunfighterLeft.name().is_empty());
}

// ═══════════════════════════════════════════════════════════════════════════════
// 4. SEM THQ — dual throttle quadrant
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn sem_thq_independent_throttle_axes() {
    let handler = SemThqInputHandler::new();
    // Left full, right idle
    let mut report = vec![0u8; 16];
    report[0..2].copy_from_slice(&u16::MAX.to_le_bytes()); // throttle_left
    report[2..4].copy_from_slice(&0u16.to_le_bytes()); // throttle_right
    let state = handler.parse_report(&report).unwrap();
    assert!((state.axes.throttle_left - 1.0).abs() < 0.001);
    assert_eq!(state.axes.throttle_right, 0.0);
}

#[test]
fn sem_thq_rotary_midpoint() {
    let handler = SemThqInputHandler::new();
    let mut report = vec![0u8; 16];
    report[4..6].copy_from_slice(&0x8000u16.to_le_bytes()); // rotary_left
    report[6..8].copy_from_slice(&0x8000u16.to_le_bytes()); // rotary_right
    let state = handler.parse_report(&report).unwrap();
    assert!((state.axes.rotary_left - 0.5).abs() < 0.01);
    assert!((state.axes.rotary_right - 0.5).abs() < 0.01);
}

#[test]
fn sem_thq_all_buttons_pressable() {
    let handler = SemThqInputHandler::new();
    let mut report = vec![0u8; 16];
    report[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
    report[12..16].copy_from_slice(&u32::MAX.to_le_bytes());
    let state = handler.parse_report(&report).unwrap();
    assert_eq!(state.pressed_buttons().len(), 64);
}

#[test]
fn sem_thq_default_handler_same_as_new() {
    let h1 = SemThqInputHandler::new();
    let h2 = SemThqInputHandler::default();
    let report = vec![0u8; 16];
    let s1 = h1.parse_report(&report).unwrap();
    let s2 = h2.parse_report(&report).unwrap();
    assert_eq!(s1.axes, s2.axes);
    assert_eq!(s1.buttons, s2.buttons);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 5. STECS Modern Throttle — parse & button checks
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn stecs_mt_midpoint_axes() {
    let report = make_stecs_mt_report(0x8000, 0x8000, 0x8000, 0x8000, 0, 0);
    let state = parse_stecs_mt_report(&report, StecsMtVariant::Mini).unwrap();
    assert!((state.axes.throttle - 0.5).abs() < 0.01);
    assert!((state.axes.mini_left - 0.5).abs() < 0.01);
    assert!((state.axes.mini_right - 0.5).abs() < 0.01);
    assert!((state.axes.rotary - 0.5).abs() < 0.01);
}

#[test]
fn stecs_mt_all_buttons_pressed_produces_64() {
    let report = make_stecs_mt_report(0, 0, 0, 0, u32::MAX, u32::MAX);
    let state = parse_stecs_mt_report(&report, StecsMtVariant::Max).unwrap();
    let pressed = state.buttons.pressed();
    assert_eq!(pressed.len(), 64);
    assert_eq!(*pressed.first().unwrap(), 1u8);
    assert_eq!(*pressed.last().unwrap(), 64u8);
}

#[test]
fn stecs_mt_scattered_buttons() {
    // Buttons 1, 16, 33, 48
    let word0 = (1u32 << 0) | (1u32 << 15);
    let word1 = (1u32 << 0) | (1u32 << 15);
    let report = make_stecs_mt_report(0, 0, 0, 0, word0, word1);
    let state = parse_stecs_mt_report(&report, StecsMtVariant::Mini).unwrap();
    assert!(state.buttons.is_pressed(1));
    assert!(state.buttons.is_pressed(16));
    assert!(state.buttons.is_pressed(33));
    assert!(state.buttons.is_pressed(48));
    assert!(!state.buttons.is_pressed(2));
    assert_eq!(state.buttons.pressed(), vec![1, 16, 33, 48]);
}

#[test]
fn stecs_mt_variant_product_names() {
    assert_eq!(
        StecsMtVariant::Mini.product_name(),
        "VKB S-TECS Modern Throttle Mini"
    );
    assert_eq!(
        StecsMtVariant::Max.product_name(),
        "VKB S-TECS Modern Throttle Max"
    );
}

#[test]
fn stecs_mt_boundary_16_bytes_too_short() {
    let report = vec![0x01u8; 16]; // one byte short
    let err = parse_stecs_mt_report(&report, StecsMtVariant::Mini);
    assert!(matches!(err, Err(StecsMtParseError::TooShort(16))));
}

#[test]
fn stecs_mt_extra_bytes_ignored() {
    let mut report = make_stecs_mt_report(0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF, 0, 0);
    report.extend_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);
    let state = parse_stecs_mt_report(&report, StecsMtVariant::Max).unwrap();
    assert!((state.axes.throttle - 1.0).abs() < 1e-4);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 6. LED command construction
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn led_command_report_id_always_correct() {
    let cmd = build_led_command(VkbLedIndex::Primary, VkbLedColor::RED, 255);
    assert_eq!(cmd[0], VKB_LED_REPORT_ID);
    assert_eq!(cmd[0], 0x09);
}

#[test]
fn led_command_max_brightness() {
    let cmd = build_led_command(VkbLedIndex::Primary, VkbLedColor::BLUE, 255);
    assert_eq!(cmd[5], 255);
    assert_eq!(cmd[4], 255); // blue channel
    assert_eq!(cmd[2], 0); // red
    assert_eq!(cmd[3], 0); // green
}

#[test]
fn led_command_secondary_index() {
    let cmd = build_led_command(VkbLedIndex::Secondary, VkbLedColor::OFF, 0);
    assert_eq!(cmd[1], 1);
    assert_eq!(cmd[2..5], [0, 0, 0]);
    assert_eq!(cmd[5], 0);
}

#[test]
fn led_command_custom_rgb() {
    let color = VkbLedColor::new(10, 20, 30);
    let cmd = build_led_command(VkbLedIndex::Primary, color, 42);
    assert_eq!(cmd, [0x09, 0, 10, 20, 30, 42]);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 7. Device family & identification
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn device_family_names_non_empty() {
    let families = [
        VkbDeviceFamily::GladiatorNxtEvo,
        VkbDeviceFamily::Gunfighter,
        VkbDeviceFamily::GladiatorMcp,
        VkbDeviceFamily::SemThq,
        VkbDeviceFamily::GladiatorNxtEvoSem,
        VkbDeviceFamily::GladiatorMk2,
    ];
    for family in families {
        assert!(!family.name().is_empty(), "{family:?} name should not be empty");
    }
}

#[test]
fn vkb_device_family_requires_correct_vid() {
    // Wrong VID should return None even with valid PID
    assert!(vkb_device_family(0x0000, VKB_GLADIATOR_NXT_EVO_RIGHT_PID).is_none());
    // Correct VID + valid PID should succeed
    assert!(vkb_device_family(VKB_VENDOR_ID, VKB_GLADIATOR_NXT_EVO_RIGHT_PID).is_some());
}

#[test]
fn is_vkb_joystick_rejects_non_vkb_vendor() {
    assert!(!is_vkb_joystick(0x1234, VKB_GLADIATOR_NXT_EVO_RIGHT_PID));
}

#[test]
fn report_layout_sem_thq_no_hat() {
    let layout = report_layout_for_family(VkbDeviceFamily::SemThq);
    assert!(!layout.has_hat_byte);
    assert_eq!(layout.axis_count, 4);
}

#[test]
fn report_layout_standard_has_hat() {
    let layout = report_layout_for_family(VkbDeviceFamily::GladiatorNxtEvo);
    assert!(layout.has_hat_byte);
    assert_eq!(layout.min_payload_bytes, 21);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 8. Profile cross-validation
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn all_profiles_vid_is_vkb() {
    for profile in all_profiles() {
        assert_eq!(
            profile.vid, VKB_VENDOR_ID,
            "{} should have VKB vendor ID",
            profile.device_name
        );
    }
}

#[test]
fn profile_pids_do_not_overlap_across_profiles() {
    let profiles = all_profiles();
    for (i, p1) in profiles.iter().enumerate() {
        for (j, p2) in profiles.iter().enumerate() {
            if i != j {
                for pid in p1.pids {
                    assert!(
                        !p2.pids.contains(pid),
                        "PID 0x{pid:04X} found in both '{}' and '{}'",
                        p1.device_name,
                        p2.device_name
                    );
                }
            }
        }
    }
}

#[test]
fn gladiator_profile_trigger_buttons_are_first() {
    let p = gladiator_nxt_evo_profile();
    let btn1 = p.button_by_number(1).unwrap();
    let btn2 = p.button_by_number(2).unwrap();
    assert_eq!(btn1.kind, ButtonKind::Trigger);
    assert_eq!(btn2.kind, ButtonKind::Trigger);
}

#[test]
fn gunfighter_profile_has_castle_switch() {
    let p = gunfighter_mcg_profile();
    let castle_buttons: Vec<_> = p
        .buttons
        .iter()
        .filter(|b| b.name.contains("Castle"))
        .collect();
    assert!(
        castle_buttons.len() >= 4,
        "MCG should have castle switch directions"
    );
}

#[test]
fn stecs_profile_has_encoder_buttons() {
    let p = stecs_throttle_profile();
    let encoders: Vec<_> = p
        .buttons
        .iter()
        .filter(|b| b.kind == ButtonKind::Encoder)
        .collect();
    assert!(
        encoders.len() >= 4,
        "STECS should have at least 4 encoder buttons (2 encoders × CW/CCW)"
    );
}

#[test]
fn t_rudder_profile_has_rudder_axis() {
    let p = t_rudder_profile();
    let rudder = p.axis_by_name("rudder");
    assert!(rudder.is_some(), "T-Rudder must have a rudder axis");
    assert_eq!(rudder.unwrap().mode, AxisNormMode::Signed);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 9. Health monitor depth tests
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn health_monitor_failure_threshold_exact_boundary() {
    let mut monitor = StecsHealthMonitor::new(VkbStecsVariant::RightSpaceThrottleGripStandard);
    // 2 failures: below threshold (default=3)
    assert!(!monitor.record_failure());
    assert!(!monitor.record_failure());
    // 3rd failure: at threshold
    assert!(monitor.record_failure());
    assert!(monitor.is_failed());
}

#[test]
fn health_monitor_success_resets_failure_count() {
    let mut monitor = StecsHealthMonitor::new(VkbStecsVariant::LeftSpaceThrottleGripMini);
    monitor.record_failure();
    monitor.record_failure();
    monitor.record_success();
    // After success, 3 more failures needed
    assert!(!monitor.record_failure());
    assert!(!monitor.record_failure());
    assert!(monitor.record_failure());
}

#[test]
fn health_monitor_failure_count_saturates() {
    let mut monitor = StecsHealthMonitor::new(VkbStecsVariant::RightSpaceThrottleGripMini);
    // Record many failures — should not overflow
    for _ in 0..1_000_000 {
        monitor.record_failure();
    }
    assert!(monitor.is_failed());
}

#[test]
fn health_status_connected_with_failures_below_threshold() {
    let mut monitor = StecsHealthMonitor::new(VkbStecsVariant::LeftSpaceThrottleGripStandard);
    monitor.record_failure();
    monitor.record_failure();
    let status = monitor.status(true, 3, 2);
    assert!(status.is_healthy(), "2 failures < threshold should be healthy");
}

#[test]
fn health_status_disconnected_is_unhealthy() {
    let monitor = StecsHealthMonitor::new(VkbStecsVariant::RightSpaceThrottleGripMiniPlus);
    let status = monitor.status(false, 0, 0);
    assert!(!status.is_healthy());
}

#[test]
fn health_monitor_reset_allows_fresh_start() {
    let mut monitor = StecsHealthMonitor::new(VkbStecsVariant::LeftSpaceThrottleGripMiniPlus);
    for _ in 0..10 {
        monitor.record_failure();
    }
    assert!(monitor.is_failed());
    monitor.reset();
    assert!(!monitor.is_failed());
    assert!(monitor.should_check_health());
}

// ═══════════════════════════════════════════════════════════════════════════════
// 10. Shift mode & axis resolution constants
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn gladiator_shift_logical_ge_physical() {
    const { assert!(GLADIATOR_NXT_EVO_SHIFT.logical_button_count >= GLADIATOR_NXT_EVO_SHIFT.physical_button_count) };
}

#[test]
fn gunfighter_shift_logical_ge_physical() {
    const { assert!(GUNFIGHTER_MCG_SHIFT.logical_button_count >= GUNFIGHTER_MCG_SHIFT.physical_button_count) };
}

#[test]
fn axis_resolution_16bit_range() {
    assert_eq!(VKB_AXIS_16BIT.bits, 16);
    assert_eq!(VKB_AXIS_16BIT.logical_max - VKB_AXIS_16BIT.logical_min, 0xFFFF);
}

#[test]
fn joystick_standard_layout_min_payload() {
    // 6 axes × 2 bytes + 2 button words × 4 bytes + 1 hat byte = 21
    assert_eq!(
        VKB_JOYSTICK_STANDARD_LAYOUT.min_payload_bytes,
        6 * 2 + 2 * 4 + 1
    );
}

#[test]
fn sem_thq_layout_min_payload() {
    // 4 axes × 2 bytes + 2 button words × 4 bytes = 16
    assert_eq!(VKB_SEM_THQ_LAYOUT.min_payload_bytes, 4 * 2 + 2 * 4);
}
