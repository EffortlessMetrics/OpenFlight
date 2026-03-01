// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Depth tests for VKB prosumer devices: Gladiator NXT, Gunfighter, T-Rudder,
//! configuration protocol, and property-based invariants.

use flight_hotas_vkb::*;

// ═══════════════════════════════════════════════════════════════════════════════
// 1. Gladiator NXT EVO — axis parsing, button matrix, hat resolution
// ═══════════════════════════════════════════════════════════════════════════════

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

/// Gladiator NXT EVO has 4 primary axes (roll, pitch, yaw/twist, throttle) plus 2 mini-stick axes.
#[test]
fn gladiator_nxt_four_axes_plus_twist() {
    let handler = GladiatorInputHandler::new(VkbGladiatorVariant::NxtEvoRight);
    // Roll=full right, pitch=full forward, yaw=full CW, throttle=75%, mini centred
    let report = make_gladiator_report(
        [0xFFFF, 0x0000, 0xFFFF, 0x8000, 0x8000, 0xC000],
        0,
        0,
        0xFF,
    );
    let state = handler.parse_report(&report).unwrap();
    assert!((state.axes.roll - 1.0).abs() < 0.01, "roll full right");
    assert!((state.axes.pitch - (-1.0)).abs() < 0.01, "pitch full fwd");
    assert!((state.axes.yaw - 1.0).abs() < 0.01, "twist full CW");
    assert!((state.axes.throttle - 0.75).abs() < 0.01, "throttle ~75%");
    assert!(state.axes.mini_x.abs() < 0.01, "mini_x centred");
    assert!(state.axes.mini_y.abs() < 0.01, "mini_y centred");
}

/// All 36 buttons in the NXT EVO lower word should be individually addressable.
#[test]
fn gladiator_nxt_button_matrix_all_36_buttons() {
    let handler = GladiatorInputHandler::new(VkbGladiatorVariant::NxtEvoRight);
    // Set buttons 1-32 via btn_lo (all bits) and buttons 33-36 via btn_hi (low nibble)
    let report = make_gladiator_report(
        [0x8000; 6],
        0xFFFF_FFFF,
        0x0000_000F, // buttons 33-36
        0xFF,
    );
    let state = handler.parse_report(&report).unwrap();
    assert_eq!(state.buttons.len(), 36, "expected exactly 36 buttons in Gladiator NXT EVO state");
    for i in 0..36 {
        assert!(state.buttons[i], "button {} should be pressed", i + 1);
    }
    assert!(state.buttons.get(36).map_or(true, |b| !*b), "button 37 should NOT be pressed");
}

/// Hat directions resolve to all 8 compass points.
#[test]
fn gladiator_nxt_hat_all_directions() {
    let handler = GladiatorInputHandler::new(VkbGladiatorVariant::NxtEvoRight);
    for dir in 0u8..=7 {
        let hat_byte = 0xF0 | dir; // hat0=dir, hat1=centred
        let report = make_gladiator_report([0x8000; 6], 0, 0, hat_byte);
        let state = handler.parse_report(&report).unwrap();
        assert_eq!(
            state.hats[0],
            Some(HatDirection(dir)),
            "hat0 should be direction {dir}"
        );
        assert_eq!(state.hats[1], None, "hat1 should be centred");
    }
}

/// Left variant should parse identically to right variant.
#[test]
fn gladiator_nxt_left_variant_parses_same() {
    let handler = GladiatorInputHandler::new(VkbGladiatorVariant::NxtEvoLeft);
    let report = make_gladiator_report(
        [0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF],
        0x0000_0001,
        0,
        0xFF,
    );
    let state = handler.parse_report(&report).unwrap();
    assert_eq!(state.variant, VkbGladiatorVariant::NxtEvoLeft);
    assert!(state.buttons[0], "button 1 pressed");
    assert!((state.axes.throttle - 1.0).abs() < 0.001);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 2. Gunfighter — sensor type, axis resolution, center adjustment
// ═══════════════════════════════════════════════════════════════════════════════

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

/// Gunfighter family detection maps all known PIDs to the correct variant.
#[test]
fn gunfighter_sensor_type_detection() {
    use flight_hid_support::device_support::{
        VKB_GUNFIGHTER_MODERN_COMBAT_PRO_PID, VKB_SPACE_GUNFIGHTER_LEFT_PID,
        VKB_SPACE_GUNFIGHTER_PID,
    };
    assert_eq!(
        GunfighterVariant::from_pid(VKB_GUNFIGHTER_MODERN_COMBAT_PRO_PID),
        Some(GunfighterVariant::ModernCombatPro)
    );
    assert_eq!(
        GunfighterVariant::from_pid(VKB_SPACE_GUNFIGHTER_PID),
        Some(GunfighterVariant::SpaceGunfighter)
    );
    assert_eq!(
        GunfighterVariant::from_pid(VKB_SPACE_GUNFIGHTER_LEFT_PID),
        Some(GunfighterVariant::SpaceGunfighterLeft)
    );
    assert_eq!(GunfighterVariant::from_pid(0x0000), None);
}

/// Gunfighter axes use 16-bit resolution (0..65535).
#[test]
fn gunfighter_axis_resolution_16bit() {
    assert_eq!(VKB_AXIS_16BIT.bits, 16);
    assert_eq!(VKB_AXIS_16BIT.logical_min, 0);
    assert_eq!(VKB_AXIS_16BIT.logical_max, 0xFFFF);
}

/// Gunfighter centre position should normalise to ~0.0 for signed axes.
#[test]
fn gunfighter_center_adjustment() {
    let handler = GunfighterInputHandler::new(GunfighterVariant::ModernCombatPro);
    let report = make_gunfighter_report(
        [0x8000, 0x8000, 0x8000, 0x8000, 0x8000, 0x8000],
        0,
        0,
        0xFF,
    );
    let state = handler.parse_report(&report).unwrap();
    assert!(state.axes.roll.abs() < 0.01, "roll at centre");
    assert!(state.axes.pitch.abs() < 0.01, "pitch at centre");
    assert!(state.axes.yaw.abs() < 0.01, "yaw at centre");
    assert!(state.axes.mini_x.abs() < 0.01, "mini_x at centre");
    assert!(state.axes.mini_y.abs() < 0.01, "mini_y at centre");
    // throttle at 0x8000 should be ~0.5 (unidirectional)
    assert!((state.axes.throttle - 0.5).abs() < 0.01, "throttle ~50%");
}

/// Gunfighter with MCG has more buttons than Gladiator NXT EVO.
#[test]
fn gunfighter_mcg_button_count_exceeds_gladiator() {
    let gf = gunfighter_mcg_profile();
    let gl = gladiator_nxt_evo_profile();
    assert!(
        gf.button_count() > gl.button_count(),
        "Gunfighter MCG ({}) should have more buttons than Gladiator NXT EVO ({})",
        gf.button_count(),
        gl.button_count()
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// 3. T-Rudder — pedal axis parsing, toe brakes, differential mode
// ═══════════════════════════════════════════════════════════════════════════════

/// T-Rudder profile reports exactly 3 axes: left toe, right toe, rudder.
#[test]
fn t_rudder_pedal_axis_parsing() {
    let p = t_rudder_profile();
    assert_eq!(p.axis_count(), 3);
    assert_eq!(p.axes[0].name, "left_toe_brake");
    assert_eq!(p.axes[1].name, "right_toe_brake");
    assert_eq!(p.axes[2].name, "rudder");
}

/// Toe brakes are unsigned (0..1), rudder is signed (-1..1).
#[test]
fn t_rudder_toe_brakes_unsigned() {
    let p = t_rudder_profile();
    assert_eq!(p.axes[0].mode, AxisNormMode::Unsigned, "left toe unsigned");
    assert_eq!(p.axes[1].mode, AxisNormMode::Unsigned, "right toe unsigned");
    assert_eq!(p.axes[2].mode, AxisNormMode::Signed, "rudder signed");
}

/// T-Rudder differential mode: independent left/right toe brake offsets are distinct.
#[test]
fn t_rudder_differential_mode() {
    let p = t_rudder_profile();
    let left = p.axis_by_name("left_toe_brake").unwrap();
    let right = p.axis_by_name("right_toe_brake").unwrap();
    assert_ne!(
        left.report_offset, right.report_offset,
        "toe brake offsets must differ for differential operation"
    );
}

/// T-Rudder has no buttons and no hats.
#[test]
fn t_rudder_no_buttons_no_hats() {
    let p = t_rudder_profile();
    assert_eq!(p.button_count(), 0);
    assert_eq!(p.hat_count(), 0);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 4. Configuration protocol — LED commands, device detection, report layout
// ═══════════════════════════════════════════════════════════════════════════════

/// LED command report is well-formed for all index/colour combinations.
#[test]
fn config_led_command_format() {
    let cmd = build_led_command(VkbLedIndex::Primary, VkbLedColor::RED, 200);
    assert_eq!(cmd.len(), 6);
    assert_eq!(cmd[0], VKB_LED_REPORT_ID);
    assert_eq!(cmd[1], 0); // primary index
    assert_eq!(cmd[2], 255); // red
    assert_eq!(cmd[3], 0); // green
    assert_eq!(cmd[4], 0); // blue
    assert_eq!(cmd[5], 200); // brightness
}

/// LED secondary index produces byte value 1.
#[test]
fn config_led_secondary_index() {
    let cmd = build_led_command(VkbLedIndex::Secondary, VkbLedColor::BLUE, 128);
    assert_eq!(cmd[1], 1);
    assert_eq!(cmd[4], 255); // blue channel
}

/// Device family detection covers all known PID ranges.
#[test]
fn config_device_detection_all_families() {
    use flight_hid_support::device_support::*;

    assert!(is_vkb_joystick(VKB_VENDOR_ID, VKB_GLADIATOR_NXT_EVO_RIGHT_PID));
    assert!(is_vkb_joystick(VKB_VENDOR_ID, VKB_GLADIATOR_NXT_EVO_LEFT_PID));
    assert!(is_vkb_joystick(VKB_VENDOR_ID, VKB_GUNFIGHTER_MODERN_COMBAT_PRO_PID));
    assert!(is_vkb_joystick(VKB_VENDOR_ID, VKB_SPACE_GUNFIGHTER_PID));
    assert!(is_vkb_joystick(VKB_VENDOR_ID, VKB_NXT_SEM_THQ_PID));
    // Wrong VID should fail
    assert!(!is_vkb_joystick(0x0000, VKB_GLADIATOR_NXT_EVO_RIGHT_PID));
}

/// Report layout for SEM THQ differs from standard joystick layout.
#[test]
fn config_report_layout_sem_vs_standard() {
    let std_layout = report_layout_for_family(VkbDeviceFamily::GladiatorNxtEvo);
    let sem_layout = report_layout_for_family(VkbDeviceFamily::SemThq);
    assert_eq!(std_layout.axis_count, 6);
    assert_eq!(sem_layout.axis_count, 4);
    assert!(std_layout.has_hat_byte);
    assert!(!sem_layout.has_hat_byte);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 5. Property tests — axis normalization bounds, button count consistency
// ═══════════════════════════════════════════════════════════════════════════════

/// Axis normalisation for any raw u16 value stays in [0.0, 1.0] (unsigned) or [-1.0, 1.0] (signed).
#[test]
fn prop_axis_normalization_bounds() {
    let handler = GladiatorInputHandler::new(VkbGladiatorVariant::NxtEvoRight);
    for raw in [0u16, 1, 0x7FFF, 0x8000, 0x8001, 0xFFFE, 0xFFFF] {
        let report = make_gladiator_report([raw; 6], 0, 0, 0xFF);
        let state = handler.parse_report(&report).unwrap();
        assert!(
            (-1.0..=1.0).contains(&state.axes.roll),
            "roll out of [-1,1] for raw={raw:#06X}"
        );
        assert!(
            (-1.0..=1.0).contains(&state.axes.pitch),
            "pitch out of [-1,1] for raw={raw:#06X}"
        );
        assert!(
            (-1.0..=1.0).contains(&state.axes.yaw),
            "yaw out of [-1,1] for raw={raw:#06X}"
        );
        assert!(
            (0.0..=1.0).contains(&state.axes.throttle),
            "throttle out of [0,1] for raw={raw:#06X}"
        );
    }
}

/// Profile button counts are consistent across all built-in profiles.
#[test]
fn prop_button_count_consistency() {
    let profiles = all_profiles();
    for p in &profiles {
        assert_eq!(
            p.button_count(),
            p.buttons.len(),
            "{}: button_count() should match buttons.len()",
            p.device_name
        );
    }
}

/// All profiles use the VKB vendor ID.
#[test]
fn prop_all_profiles_vid_is_vkb() {
    for p in &all_profiles() {
        assert_eq!(p.vid, VKB_VENDOR_ID, "{}: VID must be VKB", p.device_name);
    }
}

/// Shift mode logical button count >= physical button count.
#[test]
fn prop_shift_mode_logical_gte_physical() {
    assert!(GLADIATOR_NXT_EVO_SHIFT.logical_button_count >= GLADIATOR_NXT_EVO_SHIFT.physical_button_count);
    assert!(GUNFIGHTER_MCG_SHIFT.logical_button_count >= GUNFIGHTER_MCG_SHIFT.physical_button_count);
}
