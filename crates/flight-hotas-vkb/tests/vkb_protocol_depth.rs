// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Depth tests for VKB Gunfighter/Gladiator device protocols.
//!
//! Covers NJoy32 protocol details, axis precision, button matrix decoding,
//! profile mappings, and device discovery for VKB joystick-class devices.

use flight_hotas_vkb::{
    // Protocol types
    GunfighterInputHandler, GunfighterVariant,
    VKB_AXIS_16BIT, VKB_JOYSTICK_STANDARD_LAYOUT, VKB_LED_REPORT_ID, VKB_SEM_THQ_LAYOUT,
    VkbDeviceFamily, VkbLedColor, VkbLedIndex,
    build_led_command, is_vkb_joystick, report_layout_for_family, vkb_device_family,
    GLADIATOR_NXT_EVO_SHIFT, GUNFIGHTER_MCG_SHIFT,
    // Input types
    GladiatorInputHandler, HatDirection, VkbGladiatorVariant,
    // Profile types
    AxisNormMode, ButtonKind, HatKind,
    all_profiles, gladiator_nxt_evo_profile, gunfighter_mcg_profile, profile_for_pid,
    sem_thq_profile, stecs_throttle_profile, t_rudder_profile,
    // Device IDs
    VKB_VENDOR_ID, VKB_GLADIATOR_NXT_EVO_LEFT_PID, VKB_GLADIATOR_NXT_EVO_RIGHT_PID,
};

use flight_hid_support::device_support::{
    VKB_GLADIATOR_MK2_PID, VKB_GLADIATOR_MODERN_COMBAT_PRO_PID,
    VKB_GLADIATOR_NXT_EVO_RIGHT_SEM_PID, VKB_GUNFIGHTER_MODERN_COMBAT_PRO_PID,
    VKB_NXT_SEM_THQ_PID, VKB_SPACE_GUNFIGHTER_LEFT_PID, VKB_SPACE_GUNFIGHTER_PID,
    VKB_STECS_RIGHT_SPACE_STANDARD_PID,
};

// ─── Report builders ──────────────────────────────────────────────────────────

/// Build a standard 21-byte VKB joystick report (Gladiator / Gunfighter layout).
fn make_joystick_report(axes: [u16; 6], btn_lo: u32, btn_hi: u32, hat_byte: u8) -> Vec<u8> {
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

/// Prepend a report-ID byte to a payload.
fn with_report_id(id: u8, payload: &[u8]) -> Vec<u8> {
    let mut r = vec![id];
    r.extend_from_slice(payload);
    r
}

// ═══════════════════════════════════════════════════════════════════════════════
// 1. NJoy32 protocol tests (8)
// ═══════════════════════════════════════════════════════════════════════════════

/// NJoy32 reports use a fixed 21-byte payload for joystick-class devices.
#[test]
fn njoy32_report_format_standard_21_bytes() {
    let layout = VKB_JOYSTICK_STANDARD_LAYOUT;
    assert_eq!(layout.min_payload_bytes, 21);
    assert_eq!(layout.axis_count, 6);
    assert_eq!(layout.button_word_count, 2);
    assert!(layout.has_hat_byte);

    // Verify a full-size report parses on both Gladiator and Gunfighter
    let report = make_joystick_report([0x8000; 6], 0, 0, 0xFF);
    assert_eq!(report.len(), 21);

    let glad = GladiatorInputHandler::new(VkbGladiatorVariant::NxtEvoRight);
    assert!(glad.parse_report(&report).is_ok());

    let gun = GunfighterInputHandler::new(GunfighterVariant::ModernCombatPro);
    assert!(gun.parse_report(&report).is_ok());
}

/// NJoy32 axis resolution is always 16-bit unsigned (0..65535).
#[test]
fn njoy32_axis_resolution_16bit() {
    assert_eq!(VKB_AXIS_16BIT.bits, 16);
    assert_eq!(VKB_AXIS_16BIT.logical_min, 0);
    assert_eq!(VKB_AXIS_16BIT.logical_max, 0xFFFF);

    // Zero raw → minimum normalised
    let report = make_joystick_report([0x0000; 6], 0, 0, 0xFF);
    let handler = GunfighterInputHandler::new(GunfighterVariant::ModernCombatPro);
    let state = handler.parse_report(&report).unwrap();
    // Signed axes: 0x0000 → -1.0
    assert!((state.axes.roll - (-1.0)).abs() < 0.01);
    // Unsigned axis (throttle): 0x0000 → 0.0
    assert!(state.axes.throttle.abs() < 0.001);
}

/// NJoy32 button matrix uses two u32 LE words for 64 buttons.
#[test]
fn njoy32_button_matrix_decoding_64_buttons() {
    let handler = GunfighterInputHandler::new(GunfighterVariant::ModernCombatPro);

    // Set every other button in both words
    let btn_lo = 0xAAAA_AAAAu32; // bits 1,3,5,...,31
    let btn_hi = 0x5555_5555u32; // bits 0,2,4,...,30 → buttons 33,35,...,63
    let report = make_joystick_report([0x8000; 6], btn_lo, btn_hi, 0xFF);
    let state = handler.parse_report(&report).unwrap();

    // Verify word 0: even-indexed bits (0-indexed) are set
    for bit in 0..32usize {
        let expected = ((btn_lo >> bit) & 1) != 0;
        assert_eq!(
            state.buttons[bit], expected,
            "button {} mismatch",
            bit + 1
        );
    }
    // Verify word 1
    for bit in 0..32usize {
        let expected = ((btn_hi >> bit) & 1) != 0;
        assert_eq!(
            state.buttons[32 + bit], expected,
            "button {} mismatch",
            33 + bit
        );
    }
}

/// NJoy32 mode layers: Gladiator NXT EVO supports 2 shift layers.
#[test]
fn njoy32_mode_layers_gladiator() {
    let shift = GLADIATOR_NXT_EVO_SHIFT;
    assert_eq!(shift.layer_count, 2, "NXT EVO supports 2 shift layers");
    assert_eq!(shift.physical_button_count, 34);
    assert_eq!(shift.logical_button_count, 64);
    assert!(shift.logical_button_count >= shift.physical_button_count);
}

/// NJoy32 mode layers: Gunfighter MCG supports 3 shift layers expanding to 128 buttons.
#[test]
fn njoy32_shift_state_gunfighter_mcg() {
    let shift = GUNFIGHTER_MCG_SHIFT;
    assert_eq!(shift.layer_count, 3, "MCG supports 3 shift layers");
    assert_eq!(shift.physical_button_count, 42);
    assert_eq!(shift.logical_button_count, 128);
    // 42 physical × 3 layers = 126 logical ≤ 128
    assert!(
        (shift.physical_button_count as u16 * shift.layer_count as u16)
            <= shift.logical_button_count as u16
    );
}

/// NJoy32 LED feedback: report format is 6 bytes starting with report ID 0x09.
#[test]
fn njoy32_led_feedback_report_format() {
    assert_eq!(VKB_LED_REPORT_ID, 0x09);

    let cmd = build_led_command(VkbLedIndex::Primary, VkbLedColor::RED, 200);
    assert_eq!(cmd.len(), 6);
    assert_eq!(cmd[0], 0x09, "report ID");
    assert_eq!(cmd[1], 0, "primary LED index");
    assert_eq!(cmd[2], 255, "red channel");
    assert_eq!(cmd[3], 0, "green channel");
    assert_eq!(cmd[4], 0, "blue channel");
    assert_eq!(cmd[5], 200, "brightness");

    // Secondary LED
    let cmd2 = build_led_command(VkbLedIndex::Secondary, VkbLedColor::BLUE, 100);
    assert_eq!(cmd2[1], 1, "secondary LED index");
    assert_eq!(cmd2[2], 0);
    assert_eq!(cmd2[3], 0);
    assert_eq!(cmd2[4], 255);
}

/// NJoy32 firmware version detection via device family classification.
#[test]
fn njoy32_firmware_version_detection_by_family() {
    // Each PID maps to a unique device family — this is how firmware
    // generation / version class is discriminated.
    let families = [
        (VKB_GLADIATOR_NXT_EVO_RIGHT_PID, VkbDeviceFamily::GladiatorNxtEvo),
        (VKB_GLADIATOR_NXT_EVO_LEFT_PID, VkbDeviceFamily::GladiatorNxtEvo),
        (VKB_GUNFIGHTER_MODERN_COMBAT_PRO_PID, VkbDeviceFamily::Gunfighter),
        (VKB_SPACE_GUNFIGHTER_PID, VkbDeviceFamily::Gunfighter),
        (VKB_SPACE_GUNFIGHTER_LEFT_PID, VkbDeviceFamily::Gunfighter),
        (VKB_GLADIATOR_MODERN_COMBAT_PRO_PID, VkbDeviceFamily::GladiatorMcp),
        (VKB_NXT_SEM_THQ_PID, VkbDeviceFamily::SemThq),
        (VKB_GLADIATOR_NXT_EVO_RIGHT_SEM_PID, VkbDeviceFamily::GladiatorNxtEvoSem),
        (VKB_GLADIATOR_MK2_PID, VkbDeviceFamily::GladiatorMk2),
    ];
    for (pid, expected_family) in &families {
        let family = VkbDeviceFamily::from_pid(*pid);
        assert_eq!(
            family,
            Some(*expected_family),
            "PID 0x{pid:04X} should map to {expected_family:?}"
        );
    }
}

/// NJoy32 report with report-ID prefix byte is correctly stripped.
#[test]
fn njoy32_report_id_prefix_stripping() {
    let payload = make_joystick_report([0xFFFF, 0x0000, 0x8000, 0x8000, 0x8000, 0xFFFF], 0x01, 0, 0xFF);
    let report = with_report_id(0x01, &payload);

    let handler = GunfighterInputHandler::new(GunfighterVariant::ModernCombatPro)
        .with_report_id(true);
    let state = handler.parse_report(&report).unwrap();

    assert!((state.axes.roll - 1.0).abs() < 0.01, "roll should be ~1.0");
    assert!((state.axes.pitch - (-1.0)).abs() < 0.01, "pitch should be ~-1.0");
    assert!((state.axes.throttle - 1.0).abs() < 0.001, "throttle should be ~1.0");
    assert!(state.buttons[0], "button 1 should be pressed");
}

// ═══════════════════════════════════════════════════════════════════════════════
// 2. Axis precision tests (6)
// ═══════════════════════════════════════════════════════════════════════════════

/// 16-bit resolution: verify exact boundary values.
#[test]
fn axis_16bit_resolution_boundaries() {
    let handler = GunfighterInputHandler::new(GunfighterVariant::ModernCombatPro);

    // Minimum (0x0000): signed → -1.0, unsigned → 0.0
    let report = make_joystick_report([0x0000, 0x0000, 0x0000, 0x0000, 0x0000, 0x0000], 0, 0, 0xFF);
    let state = handler.parse_report(&report).unwrap();
    assert!((state.axes.roll - (-1.0)).abs() < 0.001);
    assert!(state.axes.throttle.abs() < 0.001);

    // Maximum (0xFFFF): signed → ~1.0, unsigned → ~1.0
    let report = make_joystick_report([0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF], 0, 0, 0xFF);
    let state = handler.parse_report(&report).unwrap();
    assert!((state.axes.roll - 1.0).abs() < 0.01);
    assert!((state.axes.throttle - 1.0).abs() < 0.001);

    // Midpoint (0x8000): signed → ~0.0, unsigned → ~0.5
    let report = make_joystick_report([0x8000, 0x8000, 0x8000, 0x8000, 0x8000, 0x8000], 0, 0, 0xFF);
    let state = handler.parse_report(&report).unwrap();
    assert!(state.axes.roll.abs() < 0.01);
    assert!((state.axes.throttle - 0.5).abs() < 0.01);
}

/// Calibration data: both Gladiator and Gunfighter use the same axis byte offsets.
#[test]
fn axis_calibration_data_consistent_offsets() {
    let glad_profile = gladiator_nxt_evo_profile();
    let gun_profile = gunfighter_mcg_profile();

    // Both should have 6 axes with identical offsets
    assert_eq!(glad_profile.axis_count(), 6);
    assert_eq!(gun_profile.axis_count(), 6);

    for (g, f) in glad_profile.axes.iter().zip(gun_profile.axes.iter()) {
        assert_eq!(
            g.report_offset, f.report_offset,
            "axis '{}' offset mismatch vs '{}'",
            g.name, f.name
        );
        assert_eq!(g.name, f.name, "axis name mismatch at offset {}", g.report_offset);
        assert_eq!(g.mode, f.mode, "axis '{}' norm mode mismatch", g.name);
    }
}

/// Center detection: 0x8000 produces near-zero on signed axes.
#[test]
fn axis_center_detection_signed() {
    let handler = GladiatorInputHandler::new(VkbGladiatorVariant::NxtEvoRight);
    let report = make_joystick_report([0x8000, 0x8000, 0x8000, 0x8000, 0x8000, 0x0000], 0, 0, 0xFF);
    let state = handler.parse_report(&report).unwrap();

    // All signed axes should be very close to 0.0
    let tolerance = 0.001;
    assert!(state.axes.roll.abs() < tolerance, "roll={}", state.axes.roll);
    assert!(state.axes.pitch.abs() < tolerance, "pitch={}", state.axes.pitch);
    assert!(state.axes.yaw.abs() < tolerance, "yaw={}", state.axes.yaw);
    assert!(state.axes.mini_x.abs() < tolerance, "mini_x={}", state.axes.mini_x);
    assert!(state.axes.mini_y.abs() < tolerance, "mini_y={}", state.axes.mini_y);
}

/// Linearity: axis output scales proportionally with input.
#[test]
fn axis_linearity_check() {
    let handler = GunfighterInputHandler::new(GunfighterVariant::SpaceGunfighter);

    // Test quarter positions for unsigned throttle axis
    let quarters: [(u16, f32); 5] = [
        (0x0000, 0.0),
        (0x4000, 0.25),
        (0x8000, 0.50),
        (0xC000, 0.75),
        (0xFFFF, 1.0),
    ];
    for (raw, expected) in &quarters {
        let report = make_joystick_report([0x8000, 0x8000, 0x8000, 0x8000, 0x8000, *raw], 0, 0, 0xFF);
        let state = handler.parse_report(&report).unwrap();
        assert!(
            (state.axes.throttle - expected).abs() < 0.01,
            "raw=0x{raw:04X}: expected throttle≈{expected}, got {}",
            state.axes.throttle
        );
    }
}

/// Full range sweep: stepping through 256 evenly-spaced values on roll axis.
#[test]
fn axis_full_range_sweep() {
    let handler = GladiatorInputHandler::new(VkbGladiatorVariant::NxtEvoLeft);
    let mut prev = f32::NEG_INFINITY;

    for step in 0..=255u16 {
        let raw = step * 256; // 0x0000, 0x0100, ..., 0xFF00
        let report = make_joystick_report([raw, 0x8000, 0x8000, 0x8000, 0x8000, 0x0000], 0, 0, 0xFF);
        let state = handler.parse_report(&report).unwrap();
        // Roll should be monotonically increasing
        assert!(
            state.axes.roll >= prev,
            "non-monotonic at step {step}: prev={prev}, current={}",
            state.axes.roll
        );
        prev = state.axes.roll;
        // Should always be in valid range
        assert!((-1.0..=1.0).contains(&state.axes.roll));
    }
}

/// Micro-axis sensitivity: adjacent raw values produce distinct outputs.
#[test]
fn axis_micro_sensitivity() {
    let handler = GunfighterInputHandler::new(GunfighterVariant::ModernCombatPro);

    // Two adjacent raw values should produce different normalised results
    let report_a = make_joystick_report([0x8000, 0x8000, 0x8000, 0x8000, 0x8000, 0x8000], 0, 0, 0xFF);
    let report_b = make_joystick_report([0x8001, 0x8000, 0x8000, 0x8000, 0x8000, 0x8000], 0, 0, 0xFF);
    let state_a = handler.parse_report(&report_a).unwrap();
    let state_b = handler.parse_report(&report_b).unwrap();

    // 16-bit resolution gives ~1/65535 step size ≈ 0.0000153
    assert!(
        (state_b.axes.roll - state_a.axes.roll).abs() > 0.0,
        "adjacent raw values should produce different outputs"
    );
    assert!(
        (state_b.axes.roll - state_a.axes.roll).abs() < 0.001,
        "single-step difference should be very small"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// 3. Button matrix tests (6)
// ═══════════════════════════════════════════════════════════════════════════════

/// Physical layout: Gladiator NXT EVO has trigger, side buttons, hats mapped as buttons.
#[test]
fn button_matrix_physical_layout_gladiator() {
    let profile = gladiator_nxt_evo_profile();

    // Button 1 = Trigger Stage 1
    let btn1 = profile.button_by_number(1).expect("button 1");
    assert_eq!(btn1.kind, ButtonKind::Trigger);
    assert!(btn1.name.contains("Trigger"));

    // Button 5 = Pinkie Button
    let btn5 = profile.button_by_number(5).expect("button 5");
    assert_eq!(btn5.kind, ButtonKind::Pushbutton);
    assert!(btn5.name.contains("Pinkie"));

    // Button 6 = Pinkie Lever (toggle)
    let btn6 = profile.button_by_number(6).expect("button 6");
    assert_eq!(btn6.kind, ButtonKind::Toggle);

    // Buttons 7-18 are hat directions
    for n in 7..=18 {
        let btn = profile.button_by_number(n).unwrap_or_else(|| panic!("button {n}"));
        assert_eq!(
            btn.kind,
            ButtonKind::HatDirection,
            "button {n} ({}) should be HatDirection",
            btn.name
        );
    }
}

/// Mode A/B/C: Gunfighter MCG has additional buttons beyond Gladiator.
#[test]
fn button_matrix_mode_abc_gunfighter_extras() {
    let gun = gunfighter_mcg_profile();
    let glad = gladiator_nxt_evo_profile();

    // MCG-specific buttons: castle switch (24-28), thumb wheel (29-31), etc.
    let btn23 = gun.button_by_number(23).expect("button 23");
    assert_eq!(btn23.kind, ButtonKind::Trigger, "Folding Trigger");

    let btn24 = gun.button_by_number(24).expect("button 24");
    assert_eq!(btn24.kind, ButtonKind::HatDirection, "MCG Castle Up");

    let btn28 = gun.button_by_number(28).expect("button 28");
    assert!(btn28.name.contains("Castle Press"));

    let btn34 = gun.button_by_number(34).expect("button 34");
    assert!(btn34.name.contains("Paddle"));

    // Gladiator doesn't have these buttons
    assert!(glad.button_by_number(34).is_none() || glad.button_count() < gun.button_count());
}

/// Hat 4-way and 8-way decoding via nibble values.
#[test]
fn button_matrix_hat_4way_8way_decoding() {
    let handler = GladiatorInputHandler::new(VkbGladiatorVariant::NxtEvoRight);

    // 8 cardinal/diagonal directions (0=N, 1=NE, 2=E, ..., 7=NW)
    for dir in 0u8..=7 {
        let hat_byte = 0xF0 | dir; // hat0 = dir, hat1 = centred
        let report = make_joystick_report([0x8000; 6], 0, 0, hat_byte);
        let state = handler.parse_report(&report).unwrap();
        assert_eq!(
            state.hats[0],
            Some(HatDirection(dir)),
            "hat0 should be direction {dir}"
        );
        assert_eq!(state.hats[1], None, "hat1 should be centred");
    }

    // Both hats active simultaneously
    let hat_byte = 0x40; // hat0 = N(0), hat1 = S(4)
    let report = make_joystick_report([0x8000; 6], 0, 0, hat_byte);
    let state = handler.parse_report(&report).unwrap();
    assert_eq!(state.hats[0], Some(HatDirection(0)), "hat0 = N");
    assert_eq!(state.hats[1], Some(HatDirection(4)), "hat1 = S");
}

/// Shift register: all 64 buttons can be independently set.
#[test]
fn button_matrix_shift_register_combinations() {
    let handler = GunfighterInputHandler::new(GunfighterVariant::ModernCombatPro);

    // Test individual button positions across both words
    for bit in 0..64usize {
        let (btn_lo, btn_hi) = if bit < 32 {
            (1u32 << bit, 0u32)
        } else {
            (0u32, 1u32 << (bit - 32))
        };
        let report = make_joystick_report([0x8000; 6], btn_lo, btn_hi, 0xFF);
        let state = handler.parse_report(&report).unwrap();

        // Exactly one button pressed
        let pressed = state.pressed_buttons();
        assert_eq!(
            pressed.len(), 1,
            "bit {bit}: expected exactly 1 button, got {:?}",
            pressed
        );
        assert_eq!(
            pressed[0] as usize,
            bit + 1,
            "bit {bit}: wrong button number"
        );
    }
}

/// Virtual buttons: pressed_buttons returns correct 1-based indices.
#[test]
fn button_matrix_virtual_buttons_pressed_list() {
    let handler = GladiatorInputHandler::new(VkbGladiatorVariant::NxtEvoRight);

    // No buttons pressed
    let report = make_joystick_report([0x8000; 6], 0, 0, 0xFF);
    let state = handler.parse_report(&report).unwrap();
    assert!(state.pressed_buttons().is_empty());

    // Buttons 1, 16, 33, 64
    let report = make_joystick_report(
        [0x8000; 6],
        0x0000_8001, // bit 0 (btn 1) + bit 15 (btn 16)
        0x8000_0001, // bit 0 (btn 33) + bit 31 (btn 64)
        0xFF,
    );
    let state = handler.parse_report(&report).unwrap();
    assert_eq!(state.pressed_buttons(), vec![1, 16, 33, 64]);
}

/// Hat centred state: 0xF nibble means no direction.
#[test]
fn button_matrix_hat_centred_all_nibble_values() {
    let handler = GunfighterInputHandler::new(GunfighterVariant::SpaceGunfighter);

    // Nibbles 8-15 should all produce None (centred)
    for high_nibble in 8u8..=15 {
        let hat_byte = (high_nibble << 4) | 0x0F; // hat0 = centred, hat1 = high_nibble
        let report = make_joystick_report([0x8000; 6], 0, 0, hat_byte);
        let state = handler.parse_report(&report).unwrap();
        assert_eq!(state.hats[0], None, "hat0 nibble 0xF should be None");
        if high_nibble <= 7 {
            assert!(state.hats[1].is_some());
        } else {
            assert_eq!(state.hats[1], None, "hat1 nibble 0x{high_nibble:X} should be None");
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 4. Profile tests (5)
// ═══════════════════════════════════════════════════════════════════════════════

/// Default mappings per model: each profile has correct axis counts.
#[test]
fn profile_default_mappings_per_model() {
    let glad = gladiator_nxt_evo_profile();
    let gun = gunfighter_mcg_profile();
    let stecs = stecs_throttle_profile();
    let sem = sem_thq_profile();
    let rudder = t_rudder_profile();

    assert_eq!(glad.axis_count(), 6, "Gladiator NXT EVO");
    assert_eq!(gun.axis_count(), 6, "Gunfighter MCG");
    assert_eq!(stecs.axis_count(), 5, "STECS");
    assert_eq!(sem.axis_count(), 4, "SEM THQ");
    assert_eq!(rudder.axis_count(), 3, "T-Rudder");

    assert!(glad.button_count() >= 30);
    assert!(gun.button_count() >= 40);
    assert!(stecs.button_count() >= 20);
    assert!(sem.button_count() >= 10);
    assert_eq!(rudder.button_count(), 0);
}

/// DCS profile: Gunfighter MCG has castle switch, folding trigger, paddle for DCS.
#[test]
fn profile_dcs_gunfighter_controls() {
    let gun = gunfighter_mcg_profile();

    // DCS-critical controls: castle switch for sensor management
    let castle_buttons: Vec<_> = gun.buttons.iter()
        .filter(|b| b.name.contains("Castle"))
        .collect();
    assert!(castle_buttons.len() >= 4, "should have castle Up/Right/Down/Left + Press");

    // Folding trigger for weapon release
    let folding = gun.buttons.iter().find(|b| b.name.contains("Folding Trigger"));
    assert!(folding.is_some(), "MCG should have folding trigger");
    assert_eq!(folding.unwrap().kind, ButtonKind::Trigger);

    // Paddle for countermeasures
    let paddle = gun.buttons.iter().find(|b| b.name.contains("Paddle"));
    assert!(paddle.is_some(), "MCG should have paddle");

    // TDC slew press for radar cursor
    let tdc = gun.buttons.iter().find(|b| b.name.contains("TDC"));
    assert!(tdc.is_some(), "MCG should have TDC Slew Press");
}

/// MSFS profile: Gladiator NXT EVO has throttle wheel and encoder for sim controls.
#[test]
fn profile_msfs_gladiator_controls() {
    let glad = gladiator_nxt_evo_profile();

    // Throttle axis exists
    let throttle = glad.axis_by_name("throttle");
    assert!(throttle.is_some(), "should have throttle axis");
    assert_eq!(throttle.unwrap().mode, AxisNormMode::Unsigned);

    // Encoder CW/CCW for heading/altitude adjustments
    let encoder_buttons: Vec<_> = glad.buttons.iter()
        .filter(|b| b.kind == ButtonKind::Encoder)
        .collect();
    assert!(encoder_buttons.len() >= 2, "should have at least CW+CCW encoder buttons");

    // Hat switches for view control
    assert!(glad.hat_count() >= 1, "should have at least one POV hat");
    let pov_hat = glad.hats.iter().find(|h| h.kind == HatKind::Pov8Way);
    assert!(pov_hat.is_some(), "should have an 8-way POV hat");
}

/// Model-specific axis counts match declared profile dimensions.
#[test]
fn profile_model_specific_axis_counts() {
    for profile in all_profiles() {
        // Each profile's axis_count() must match the slice length
        assert_eq!(
            profile.axis_count(),
            profile.axes.len(),
            "{}: axis_count mismatch",
            profile.device_name
        );
        assert_eq!(
            profile.button_count(),
            profile.buttons.len(),
            "{}: button_count mismatch",
            profile.device_name
        );
        assert_eq!(
            profile.hat_count(),
            profile.hats.len(),
            "{}: hat_count mismatch",
            profile.device_name
        );

        // VID must be VKB
        assert_eq!(profile.vid, VKB_VENDOR_ID, "{}", profile.device_name);
    }
}

/// Profile lookup by PID resolves all known joystick PIDs.
#[test]
fn profile_pid_lookup_comprehensive() {
    // Gladiator NXT EVO
    assert_eq!(
        profile_for_pid(VKB_GLADIATOR_NXT_EVO_RIGHT_PID).unwrap().device_name,
        "VKB Gladiator NXT EVO"
    );
    assert_eq!(
        profile_for_pid(VKB_GLADIATOR_NXT_EVO_LEFT_PID).unwrap().device_name,
        "VKB Gladiator NXT EVO"
    );

    // Gunfighter variants
    let gf = profile_for_pid(VKB_GUNFIGHTER_MODERN_COMBAT_PRO_PID).unwrap();
    assert_eq!(gf.device_name, "VKB Gunfighter + MCG");

    let sgf = profile_for_pid(VKB_SPACE_GUNFIGHTER_PID).unwrap();
    assert_eq!(sgf.device_name, "VKB Gunfighter + MCG");

    let sgfl = profile_for_pid(VKB_SPACE_GUNFIGHTER_LEFT_PID).unwrap();
    assert_eq!(sgfl.device_name, "VKB Gunfighter + MCG");

    // SEM THQ
    let sem = profile_for_pid(VKB_NXT_SEM_THQ_PID).unwrap();
    assert_eq!(sem.device_name, "VKB SEM Throttle Quadrant");

    // STECS
    let stecs = profile_for_pid(VKB_STECS_RIGHT_SPACE_STANDARD_PID).unwrap();
    assert_eq!(stecs.device_name, "VKB STECS Standard/Plus");

    // Unknown PID
    assert!(profile_for_pid(0xDEAD).is_none());
}

// ═══════════════════════════════════════════════════════════════════════════════
// 5. Device discovery tests (5)
// ═══════════════════════════════════════════════════════════════════════════════

/// VKB VID matching: is_vkb_joystick requires correct vendor ID.
#[test]
fn discovery_vkb_vid_matching() {
    assert_eq!(VKB_VENDOR_ID, 0x231D);

    // Correct VID + known PID → true
    assert!(is_vkb_joystick(VKB_VENDOR_ID, VKB_GLADIATOR_NXT_EVO_RIGHT_PID));
    assert!(is_vkb_joystick(VKB_VENDOR_ID, VKB_GUNFIGHTER_MODERN_COMBAT_PRO_PID));

    // Wrong VID → false even with valid PID
    assert!(!is_vkb_joystick(0x0000, VKB_GLADIATOR_NXT_EVO_RIGHT_PID));
    assert!(!is_vkb_joystick(0x046D, VKB_GUNFIGHTER_MODERN_COMBAT_PRO_PID)); // Logitech VID

    // Correct VID + unknown PID → false
    assert!(!is_vkb_joystick(VKB_VENDOR_ID, 0xFFFF));
    assert!(!is_vkb_joystick(VKB_VENDOR_ID, 0x0000));
}

/// Model discrimination by PID: each known PID maps to the correct family.
#[test]
fn discovery_model_discrimination_by_pid() {
    // Gladiator NXT EVO variants
    assert_eq!(
        vkb_device_family(VKB_VENDOR_ID, VKB_GLADIATOR_NXT_EVO_RIGHT_PID),
        Some(VkbDeviceFamily::GladiatorNxtEvo)
    );
    assert_eq!(
        vkb_device_family(VKB_VENDOR_ID, VKB_GLADIATOR_NXT_EVO_LEFT_PID),
        Some(VkbDeviceFamily::GladiatorNxtEvo)
    );

    // Gunfighter variants
    assert_eq!(
        vkb_device_family(VKB_VENDOR_ID, VKB_GUNFIGHTER_MODERN_COMBAT_PRO_PID),
        Some(VkbDeviceFamily::Gunfighter)
    );
    assert_eq!(
        vkb_device_family(VKB_VENDOR_ID, VKB_SPACE_GUNFIGHTER_PID),
        Some(VkbDeviceFamily::Gunfighter)
    );
    assert_eq!(
        vkb_device_family(VKB_VENDOR_ID, VKB_SPACE_GUNFIGHTER_LEFT_PID),
        Some(VkbDeviceFamily::Gunfighter)
    );

    // Special families
    assert_eq!(
        vkb_device_family(VKB_VENDOR_ID, VKB_GLADIATOR_MODERN_COMBAT_PRO_PID),
        Some(VkbDeviceFamily::GladiatorMcp)
    );
    assert_eq!(
        vkb_device_family(VKB_VENDOR_ID, VKB_NXT_SEM_THQ_PID),
        Some(VkbDeviceFamily::SemThq)
    );
    assert_eq!(
        vkb_device_family(VKB_VENDOR_ID, VKB_GLADIATOR_NXT_EVO_RIGHT_SEM_PID),
        Some(VkbDeviceFamily::GladiatorNxtEvoSem)
    );
    assert_eq!(
        vkb_device_family(VKB_VENDOR_ID, VKB_GLADIATOR_MK2_PID),
        Some(VkbDeviceFamily::GladiatorMk2)
    );
}

/// Firmware version compat: report layout varies by family.
#[test]
fn discovery_firmware_version_compat_report_layouts() {
    // All joystick-class families use the standard 21-byte layout
    let standard_families = [
        VkbDeviceFamily::GladiatorNxtEvo,
        VkbDeviceFamily::Gunfighter,
        VkbDeviceFamily::GladiatorMcp,
        VkbDeviceFamily::GladiatorNxtEvoSem,
        VkbDeviceFamily::GladiatorMk2,
    ];
    for family in &standard_families {
        let layout = report_layout_for_family(*family);
        assert_eq!(
            layout, VKB_JOYSTICK_STANDARD_LAYOUT,
            "{family:?} should use standard layout"
        );
    }

    // SEM THQ uses the compact 16-byte layout
    let sem_layout = report_layout_for_family(VkbDeviceFamily::SemThq);
    assert_eq!(sem_layout, VKB_SEM_THQ_LAYOUT);
    assert_eq!(sem_layout.axis_count, 4);
    assert!(!sem_layout.has_hat_byte);
}

/// Device family names are non-empty and human-readable.
#[test]
fn discovery_device_family_names() {
    let families = [
        VkbDeviceFamily::GladiatorNxtEvo,
        VkbDeviceFamily::Gunfighter,
        VkbDeviceFamily::GladiatorMcp,
        VkbDeviceFamily::SemThq,
        VkbDeviceFamily::GladiatorNxtEvoSem,
        VkbDeviceFamily::GladiatorMk2,
    ];
    for family in &families {
        let name = family.name();
        assert!(!name.is_empty(), "{family:?} should have a name");
        assert!(name.contains("VKB"), "{family:?} name should contain 'VKB'");
    }
}

/// Unknown PIDs: wrong VID or unknown PID returns None from all discovery functions.
#[test]
fn discovery_unknown_pids_rejected() {
    // Wrong VID
    assert_eq!(vkb_device_family(0x0000, VKB_GLADIATOR_NXT_EVO_RIGHT_PID), None);
    assert_eq!(vkb_device_family(0x046D, VKB_GUNFIGHTER_MODERN_COMBAT_PRO_PID), None);

    // Unknown PIDs (STECS Modern Throttle PIDs are not in the joystick family classifier)
    assert_eq!(VkbDeviceFamily::from_pid(0x0000), None);
    assert_eq!(VkbDeviceFamily::from_pid(0xFFFF), None);

    // Verify is_vkb_joystick returns false
    assert!(!is_vkb_joystick(VKB_VENDOR_ID, 0x0000));
    assert!(!is_vkb_joystick(0x0000, 0x0000));
}
