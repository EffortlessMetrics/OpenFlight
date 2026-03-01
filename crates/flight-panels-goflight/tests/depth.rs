// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for GoFlight avionics panel HID report parsing and LED commands.
//!
//! Covers all four GoFlight module variants (GF-46, GF-45, GF-LGT, GF-WCP)
//! with report parsing, button/encoder state tracking, LED control commands,
//! error handling, and panel identification.

use flight_panels_goflight::{
    GoFlightError, GoFlightModule, GoFlightReport, GOFLIGHT_MIN_REPORT_BYTES, build_led_command,
    parse_report,
};

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Build a raw 8-byte HID input report from structured fields.
fn make_report(report_id: u8, enc: [i8; 4], buttons: u16, leds: u8) -> [u8; 8] {
    let [bl, bh] = buttons.to_le_bytes();
    [
        report_id,
        enc[0] as u8,
        enc[1] as u8,
        enc[2] as u8,
        enc[3] as u8,
        bl,
        bh,
        leds,
    ]
}

/// All module variants for exhaustive iteration.
const ALL_MODULES: [GoFlightModule; 4] = [
    GoFlightModule::Gf46,
    GoFlightModule::Gf45,
    GoFlightModule::GfLgt,
    GoFlightModule::GfWcp,
];

// ── HID report parsing — per-panel variant coverage ─────────────────────────

#[test]
fn parse_idle_report_for_every_module() {
    let data = make_report(0x01, [0, 0, 0, 0], 0, 0);
    for module in ALL_MODULES {
        let r = parse_report(&data, module).unwrap();
        assert_eq!(r.module, module);
        assert_eq!(r.encoders, [0, 0, 0, 0]);
        assert_eq!(r.buttons, 0);
        assert_eq!(r.leds, 0);
    }
}

#[test]
fn parse_full_activity_report_for_every_module() {
    let data = make_report(0x01, [3, -2, 1, -1], 0xA5A5, 0x3C);
    for module in ALL_MODULES {
        let r = parse_report(&data, module).unwrap();
        assert_eq!(r.module, module);
        assert_eq!(r.encoders, [3, -2, 1, -1]);
        assert_eq!(r.buttons, 0xA5A5);
        assert_eq!(r.leds, 0x3C);
    }
}

#[test]
fn parse_gf46_com_nav_radio_typical_tuning() {
    // GF-46: outer encoder CW +2, inner encoder CCW -1, flip-flop button pressed
    let data = make_report(0x01, [2, -1, 0, 0], 0x0001, 0x00);
    let r = parse_report(&data, GoFlightModule::Gf46).unwrap();
    assert_eq!(r.module, GoFlightModule::Gf46);
    assert_eq!(r.encoders[0], 2, "outer encoder CW");
    assert_eq!(r.encoders[1], -1, "inner encoder CCW");
    assert!(r.buttons & 0x01 != 0, "flip-flop button");
}

#[test]
fn parse_gf45_autopilot_heading_altitude() {
    // GF-45: heading encoder +5, altitude encoder -10
    let data = make_report(0x01, [5, 0, -10, 0], 0x0003, 0x0F);
    let r = parse_report(&data, GoFlightModule::Gf45).unwrap();
    assert_eq!(r.module, GoFlightModule::Gf45);
    assert_eq!(r.encoders[0], 5);
    assert_eq!(r.encoders[2], -10);
    assert_eq!(r.buttons, 0x0003, "HDG + ALT buttons");
    assert_eq!(r.leds, 0x0F, "AP mode LEDs");
}

#[test]
fn parse_gflgt_gear_lights_toggle() {
    // GF-LGT: gear up/down buttons
    let data = make_report(0x01, [0, 0, 0, 0], 0x0007, 0x07);
    let r = parse_report(&data, GoFlightModule::GfLgt).unwrap();
    assert_eq!(r.module, GoFlightModule::GfLgt);
    assert_eq!(r.encoders, [0, 0, 0, 0], "no encoders active");
    assert_eq!(r.buttons, 0x0007, "3 gear switches");
    assert_eq!(r.leds, 0x07, "3 gear indicator LEDs");
}

#[test]
fn parse_gfwcp_weather_dials() {
    // GF-WCP: all four encoders active simultaneously
    let data = make_report(0x01, [1, -1, 2, -2], 0x0000, 0x00);
    let r = parse_report(&data, GoFlightModule::GfWcp).unwrap();
    assert_eq!(r.module, GoFlightModule::GfWcp);
    assert_eq!(r.encoders, [1, -1, 2, -2]);
    assert_eq!(r.buttons, 0);
}

// ── Encoder state tracking ──────────────────────────────────────────────────

#[test]
fn encoder_i8_min_max_boundaries() {
    let data = make_report(0x01, [i8::MAX, i8::MIN, 0, 0], 0, 0);
    let r = parse_report(&data, GoFlightModule::Gf46).unwrap();
    assert_eq!(r.encoders[0], 127);
    assert_eq!(r.encoders[1], -128);
}

#[test]
fn encoder_single_click_each_direction() {
    let cw = make_report(0x01, [1, 0, 0, 0], 0, 0);
    let ccw = make_report(0x01, [-1, 0, 0, 0], 0, 0);
    let r_cw = parse_report(&cw, GoFlightModule::Gf46).unwrap();
    let r_ccw = parse_report(&ccw, GoFlightModule::Gf46).unwrap();
    assert_eq!(r_cw.encoders[0], 1, "CW single click");
    assert_eq!(r_ccw.encoders[0], -1, "CCW single click");
}

#[test]
fn encoder_all_four_positive() {
    let data = make_report(0x01, [10, 20, 30, 40], 0, 0);
    let r = parse_report(&data, GoFlightModule::Gf45).unwrap();
    assert_eq!(r.encoders, [10, 20, 30, 40]);
}

#[test]
fn encoder_all_four_negative() {
    let data = make_report(0x01, [-10, -20, -30, -40], 0, 0);
    let r = parse_report(&data, GoFlightModule::Gf45).unwrap();
    assert_eq!(r.encoders, [-10, -20, -30, -40]);
}

#[test]
fn encoder_alternating_directions() {
    let data = make_report(0x01, [5, -5, 5, -5], 0, 0);
    let r = parse_report(&data, GoFlightModule::GfWcp).unwrap();
    assert_eq!(r.encoders, [5, -5, 5, -5]);
}

#[test]
fn encoder_zero_means_no_movement() {
    let data = make_report(0x01, [0, 0, 0, 0], 0x1234, 0xAB);
    let r = parse_report(&data, GoFlightModule::Gf46).unwrap();
    for (i, &enc) in r.encoders.iter().enumerate() {
        assert_eq!(enc, 0, "encoder {i} should be idle");
    }
}

// ── Button state tracking ───────────────────────────────────────────────────

#[test]
fn button_individual_isolation() {
    for bit in 0..16u16 {
        let mask = 1u16 << bit;
        let data = make_report(0x01, [0, 0, 0, 0], mask, 0);
        let r = parse_report(&data, GoFlightModule::Gf46).unwrap();
        assert_eq!(r.buttons, mask, "only button {bit} should be set");
        // Verify no other buttons are set
        for other in 0..16u16 {
            if other == bit {
                assert!(r.buttons & (1 << other) != 0);
            } else {
                assert!(
                    r.buttons & (1 << other) == 0,
                    "button {other} should NOT be set when only {bit} is pressed"
                );
            }
        }
    }
}

#[test]
fn button_all_pressed() {
    let data = make_report(0x01, [0, 0, 0, 0], 0xFFFF, 0);
    let r = parse_report(&data, GoFlightModule::GfLgt).unwrap();
    assert_eq!(r.buttons, 0xFFFF);
    for bit in 0..16 {
        assert!(r.buttons & (1 << bit) != 0, "button {bit} must be set");
    }
}

#[test]
fn button_none_pressed() {
    let data = make_report(0x01, [0, 0, 0, 0], 0x0000, 0);
    let r = parse_report(&data, GoFlightModule::GfLgt).unwrap();
    assert_eq!(r.buttons, 0);
}

#[test]
fn button_high_byte_only() {
    let data = make_report(0x01, [0, 0, 0, 0], 0xFF00, 0);
    let r = parse_report(&data, GoFlightModule::Gf45).unwrap();
    assert_eq!(r.buttons, 0xFF00);
    for bit in 0..8 {
        assert_eq!(r.buttons & (1 << bit), 0, "low bit {bit} must be clear");
    }
    for bit in 8..16 {
        assert!(r.buttons & (1 << bit) != 0, "high bit {bit} must be set");
    }
}

#[test]
fn button_low_byte_only() {
    let data = make_report(0x01, [0, 0, 0, 0], 0x00FF, 0);
    let r = parse_report(&data, GoFlightModule::Gf45).unwrap();
    assert_eq!(r.buttons, 0x00FF);
}

#[test]
fn button_checkerboard_pattern() {
    let data = make_report(0x01, [0, 0, 0, 0], 0xAAAA, 0);
    let r = parse_report(&data, GoFlightModule::Gf46).unwrap();
    assert_eq!(r.buttons, 0xAAAA);
    // Even bits clear, odd bits set
    for bit in 0..16u16 {
        if bit % 2 == 0 {
            assert_eq!(r.buttons & (1 << bit), 0);
        } else {
            assert!(r.buttons & (1 << bit) != 0);
        }
    }
}

#[test]
fn button_endianness_verified() {
    // Buttons = 0x0201: low byte = 0x01, high byte = 0x02
    let data = make_report(0x01, [0, 0, 0, 0], 0x0201, 0);
    let r = parse_report(&data, GoFlightModule::Gf46).unwrap();
    assert_eq!(r.buttons, 0x0201);
    assert!(r.buttons & 0x0001 != 0, "bit 0 (low byte)");
    assert!(r.buttons & 0x0200 != 0, "bit 9 (high byte)");
}

// ── LED control commands ────────────────────────────────────────────────────

#[test]
fn led_command_individual_bits() {
    for bit in 0..16u16 {
        let mask = 1u16 << bit;
        let cmd = build_led_command(mask);
        assert_eq!(cmd[0], 0x01, "report ID");
        let parsed = u16::from_le_bytes([cmd[1], cmd[2]]);
        assert_eq!(parsed, mask, "LED bit {bit} should be set");
        assert_eq!(&cmd[3..], &[0, 0, 0, 0, 0], "padding must be zero");
    }
}

#[test]
fn led_command_alternating_pattern() {
    let cmd = build_led_command(0x5555);
    assert_eq!(cmd[0], 0x01);
    assert_eq!(cmd[1], 0x55);
    assert_eq!(cmd[2], 0x55);
}

#[test]
fn led_command_report_id_always_0x01() {
    for leds in [0x0000, 0x00FF, 0xFF00, 0xFFFF, 0x1234, 0xABCD] {
        let cmd = build_led_command(leds);
        assert_eq!(cmd[0], 0x01, "report ID must always be 0x01 for leds={leds:#06X}");
    }
}

#[test]
fn led_command_total_length_is_8() {
    let cmd = build_led_command(0x1234);
    assert_eq!(cmd.len(), 8);
}

#[test]
fn led_command_padding_always_zero() {
    for leds in [0x0000, 0xFFFF, 0x8080, 0x0101] {
        let cmd = build_led_command(leds);
        for (i, &byte) in cmd[3..].iter().enumerate() {
            assert_eq!(byte, 0, "padding byte {i}+3 must be 0 for leds={leds:#06X}");
        }
    }
}

#[test]
fn led_command_low_byte_high_byte_split() {
    let cmd = build_led_command(0xBEEF);
    assert_eq!(cmd[1], 0xEF, "low byte");
    assert_eq!(cmd[2], 0xBE, "high byte");
}

// ── Display digit encoding via LED bitmask patterns ─────────────────────────

#[test]
fn led_patterns_for_seven_segment_digits() {
    // Common seven-segment LED patterns mapped to LED byte positions.
    // Real GoFlight panels use LED bitmask subsets for digit segments.
    let digit_patterns: [(u16, &str); 4] = [
        (0x00, "blank"),
        (0x01, "single segment"),
        (0xFF, "all low segments"),
        (0xFFFF, "all segments lit"),
    ];
    for (pattern, label) in digit_patterns {
        let cmd = build_led_command(pattern);
        let roundtrip = u16::from_le_bytes([cmd[1], cmd[2]]);
        assert_eq!(roundtrip, pattern, "digit pattern '{label}' must round-trip");
    }
}

#[test]
fn led_read_back_from_report_matches_low_byte() {
    // LED state in parsed report comes from byte[7] as u16 (0..=255 range).
    for leds_byte in [0x00, 0x01, 0x55, 0xAA, 0xFF] {
        let data = make_report(0x01, [0, 0, 0, 0], 0, leds_byte);
        let r = parse_report(&data, GoFlightModule::Gf46).unwrap();
        assert_eq!(r.leds, leds_byte as u16, "LED readback for {leds_byte:#04X}");
    }
}

#[test]
fn led_report_value_capped_at_u8_range() {
    // Parsed LED state from report byte[7] is promoted to u16 but can never exceed 0xFF.
    let data = make_report(0x01, [0, 0, 0, 0], 0, 0xFF);
    let r = parse_report(&data, GoFlightModule::Gf46).unwrap();
    assert!(r.leds <= 0xFF, "LED from report cannot exceed single-byte range");
}

// ── Error handling for malformed reports ─────────────────────────────────────

#[test]
fn error_empty_buffer() {
    let err = parse_report(&[], GoFlightModule::Gf46).unwrap_err();
    assert_eq!(
        err,
        GoFlightError::TooShort {
            expected: 8,
            actual: 0
        }
    );
}

#[test]
fn error_each_length_below_minimum() {
    for len in 0..GOFLIGHT_MIN_REPORT_BYTES {
        let buf = vec![0xFFu8; len];
        let err = parse_report(&buf, GoFlightModule::Gf46).unwrap_err();
        assert_eq!(
            err,
            GoFlightError::TooShort {
                expected: GOFLIGHT_MIN_REPORT_BYTES,
                actual: len,
            },
            "length {len} should produce TooShort error"
        );
    }
}

#[test]
fn error_one_byte_short() {
    let buf = [0u8; 7];
    assert!(parse_report(&buf, GoFlightModule::Gf45).is_err());
}

#[test]
fn error_for_all_module_variants() {
    let short = [0u8; 3];
    for module in ALL_MODULES {
        let err = parse_report(&short, module).unwrap_err();
        assert!(
            matches!(err, GoFlightError::TooShort { expected: 8, actual: 3 }),
            "module {module:?} should reject short report"
        );
    }
}

#[test]
fn error_display_message_format() {
    let err = GoFlightError::TooShort {
        expected: 8,
        actual: 2,
    };
    let msg = err.to_string();
    assert!(
        msg.contains("8") && msg.contains("2"),
        "error message should include expected and actual: {msg}"
    );
}

#[test]
fn exact_minimum_length_succeeds() {
    let buf = [0u8; 8];
    assert!(parse_report(&buf, GoFlightModule::Gf46).is_ok());
}

#[test]
fn oversized_buffer_still_parses_first_8_bytes() {
    let mut buf = [0u8; 64];
    buf[0] = 0x02; // report_id
    buf[1] = 7; // encoder 0
    buf[5] = 0x42; // buttons low
    buf[6] = 0x00; // buttons high
    buf[7] = 0xAB; // leds
    // Extra bytes after byte 7 should be ignored
    buf[8] = 0xFF;
    buf[9] = 0xEE;
    let r = parse_report(&buf, GoFlightModule::Gf46).unwrap();
    assert_eq!(r.encoders[0], 7);
    assert_eq!(r.buttons, 0x0042);
    assert_eq!(r.leds, 0xAB);
}

#[test]
fn report_id_byte_is_ignored_by_parser() {
    // The parser does not validate report_id; it only reads fields from bytes 1-7.
    for report_id in [0x00, 0x01, 0x02, 0x7F, 0xFF] {
        let data = make_report(report_id, [1, 2, 3, 4], 0x1234, 0x56);
        let r = parse_report(&data, GoFlightModule::Gf46).unwrap();
        assert_eq!(r.encoders, [1, 2, 3, 4]);
        assert_eq!(r.buttons, 0x1234);
        assert_eq!(r.leds, 0x56);
    }
}

// ── Panel identification and capability detection ───────────────────────────

#[test]
fn module_enum_clone_and_copy() {
    let m = GoFlightModule::Gf46;
    let cloned = m.clone();
    let copied = m;
    assert_eq!(m, cloned);
    assert_eq!(m, copied);
}

#[test]
fn module_enum_equality() {
    assert_eq!(GoFlightModule::Gf46, GoFlightModule::Gf46);
    assert_eq!(GoFlightModule::Gf45, GoFlightModule::Gf45);
    assert_eq!(GoFlightModule::GfLgt, GoFlightModule::GfLgt);
    assert_eq!(GoFlightModule::GfWcp, GoFlightModule::GfWcp);
}

#[test]
fn module_enum_inequality() {
    assert_ne!(GoFlightModule::Gf46, GoFlightModule::Gf45);
    assert_ne!(GoFlightModule::Gf46, GoFlightModule::GfLgt);
    assert_ne!(GoFlightModule::Gf46, GoFlightModule::GfWcp);
    assert_ne!(GoFlightModule::Gf45, GoFlightModule::GfLgt);
    assert_ne!(GoFlightModule::Gf45, GoFlightModule::GfWcp);
    assert_ne!(GoFlightModule::GfLgt, GoFlightModule::GfWcp);
}

#[test]
fn module_debug_representation() {
    let dbg = format!("{:?}", GoFlightModule::Gf46);
    assert!(!dbg.is_empty(), "Debug output should be non-empty");
    assert!(dbg.contains("Gf46"), "Debug should contain variant name");
}

#[test]
fn module_preserved_through_parsing() {
    let data = make_report(0x01, [0, 0, 0, 0], 0, 0);
    for module in ALL_MODULES {
        let r = parse_report(&data, module).unwrap();
        assert_eq!(
            r.module, module,
            "module type must be preserved: {module:?}"
        );
    }
}

#[test]
fn same_report_different_modules_yield_same_data() {
    let data = make_report(0x01, [5, -3, 0, 2], 0x8001, 0x42);
    let results: Vec<GoFlightReport> = ALL_MODULES
        .iter()
        .map(|&m| parse_report(&data, m).unwrap())
        .collect();

    // All reports should have identical data fields, only module differs
    for (i, r) in results.iter().enumerate() {
        assert_eq!(r.encoders, [5, -3, 0, 2], "encoders mismatch at index {i}");
        assert_eq!(r.buttons, 0x8001, "buttons mismatch at index {i}");
        assert_eq!(r.leds, 0x42, "leds mismatch at index {i}");
    }
    // But modules should be distinct
    assert_ne!(results[0].module, results[1].module);
    assert_ne!(results[0].module, results[2].module);
    assert_ne!(results[0].module, results[3].module);
}

// ── Combined state scenarios ────────────────────────────────────────────────

#[test]
fn simultaneous_encoder_button_led_activity() {
    let data = make_report(0x01, [10, -5, 3, -1], 0xDEAD, 0xBE);
    let r = parse_report(&data, GoFlightModule::Gf46).unwrap();
    assert_eq!(r.encoders, [10, -5, 3, -1]);
    assert_eq!(r.buttons, 0xDEAD);
    assert_eq!(r.leds, 0xBE);
}

#[test]
fn maximal_state_all_fields_saturated() {
    let data = make_report(0xFF, [i8::MAX, i8::MIN, i8::MAX, i8::MIN], 0xFFFF, 0xFF);
    let r = parse_report(&data, GoFlightModule::GfWcp).unwrap();
    assert_eq!(r.encoders, [127, -128, 127, -128]);
    assert_eq!(r.buttons, 0xFFFF);
    assert_eq!(r.leds, 0xFF);
}

#[test]
fn sequential_report_simulation() {
    // Simulate a sequence of reports as would occur during knob turning
    let reports: Vec<[u8; 8]> = vec![
        make_report(0x01, [1, 0, 0, 0], 0, 0),      // CW click 1
        make_report(0x01, [1, 0, 0, 0], 0, 0),      // CW click 2
        make_report(0x01, [2, 0, 0, 0], 0, 0),      // Fast CW
        make_report(0x01, [-1, 0, 0, 0], 0, 0),     // Reverse CCW
        make_report(0x01, [0, 0, 0, 0], 0x0001, 0), // Button press
        make_report(0x01, [0, 0, 0, 0], 0x0000, 0), // Button release
    ];

    let mut cumulative_encoder_0: i32 = 0;
    for (i, raw) in reports.iter().enumerate() {
        let r = parse_report(raw, GoFlightModule::Gf46).unwrap();
        cumulative_encoder_0 += r.encoders[0] as i32;
        // Each parse must succeed independently
        assert_eq!(r.module, GoFlightModule::Gf46, "report {i}");
    }
    // 1 + 1 + 2 + (-1) + 0 + 0 = 3
    assert_eq!(cumulative_encoder_0, 3, "cumulative encoder delta");
}

#[test]
fn led_command_then_verify_structure() {
    // Scenario: Set specific LEDs, verify entire command structure
    let leds = 0b0000_0000_1010_1010u16; // LEDs 1,3,5,7 on
    let cmd = build_led_command(leds);

    assert_eq!(cmd.len(), 8);
    assert_eq!(cmd[0], 0x01, "report ID");
    assert_eq!(cmd[1], 0xAA, "low byte: bits 1,3,5,7");
    assert_eq!(cmd[2], 0x00, "high byte: no LEDs");
    assert_eq!(cmd[3..], [0, 0, 0, 0, 0], "padding");
}

// ── GoFlightError trait coverage ────────────────────────────────────────────

#[test]
fn error_clone_and_eq() {
    let err1 = GoFlightError::TooShort {
        expected: 8,
        actual: 3,
    };
    let err2 = err1.clone();
    assert_eq!(err1, err2);
}

#[test]
fn error_debug_format() {
    let err = GoFlightError::TooShort {
        expected: 8,
        actual: 0,
    };
    let dbg = format!("{err:?}");
    assert!(dbg.contains("TooShort"), "Debug should contain variant name");
    assert!(dbg.contains("8"), "Debug should contain expected");
    assert!(dbg.contains("0"), "Debug should contain actual");
}

#[test]
fn error_display_is_human_readable() {
    let err = GoFlightError::TooShort {
        expected: 8,
        actual: 5,
    };
    let display = format!("{err}");
    assert!(
        display.contains("report too short"),
        "Display should be human-readable: {display}"
    );
    assert!(display.contains("8") && display.contains("5"));
}

// ── Constant validation ─────────────────────────────────────────────────────

#[test]
fn minimum_report_bytes_is_8() {
    assert_eq!(GOFLIGHT_MIN_REPORT_BYTES, 8);
}

// ── GoFlightReport struct coverage ──────────────────────────────────────────

#[test]
fn report_clone() {
    let data = make_report(0x01, [1, -2, 3, -4], 0x5678, 0x9A);
    let r = parse_report(&data, GoFlightModule::Gf46).unwrap();
    let cloned = r.clone();
    assert_eq!(cloned.module, r.module);
    assert_eq!(cloned.encoders, r.encoders);
    assert_eq!(cloned.buttons, r.buttons);
    assert_eq!(cloned.leds, r.leds);
}

#[test]
fn report_debug() {
    let data = make_report(0x01, [0, 0, 0, 0], 0, 0);
    let r = parse_report(&data, GoFlightModule::Gf46).unwrap();
    let dbg = format!("{r:?}");
    assert!(dbg.contains("GoFlightReport"));
    assert!(dbg.contains("Gf46"));
}
