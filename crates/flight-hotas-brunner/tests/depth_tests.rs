// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for the Brunner CLS-E Force Feedback Yoke HID parser.
//!
//! These integration tests exercise the public API of `flight-hotas-brunner`
//! covering: report parsing, axis normalisation boundary conditions, button
//! extraction, struct invariants, error handling, device identification, and
//! property-based fuzzing.

use flight_hotas_brunner::{
    BRUNNER_CLS_E_JOYSTICK_PID, BRUNNER_CLS_E_NG_YOKE_PID, BRUNNER_CLS_E_RUDDER_PID,
    BRUNNER_CLS_E_YOKE_PID, BRUNNER_VENDOR_ID, BrunnerModel, CLS_E_MIN_REPORT_BYTES, ClsEAxes,
    ClsEButtons, ClsEInputState, ClsEParseError, brunner_model, is_brunner_device,
    parse_cls_e_report,
};

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Build a minimal (9-byte) CLS-E HID report from raw axis values and button bytes.
fn make_report(roll: i16, pitch: i16, buttons: [u8; 4]) -> Vec<u8> {
    let mut data = vec![0x01u8]; // report_id
    data.extend_from_slice(&roll.to_le_bytes());
    data.extend_from_slice(&pitch.to_le_bytes());
    data.extend_from_slice(&buttons);
    data
}

/// Build a report with an arbitrary report-ID byte.
fn make_report_with_id(report_id: u8, roll: i16, pitch: i16, buttons: [u8; 4]) -> Vec<u8> {
    let mut data = vec![report_id];
    data.extend_from_slice(&roll.to_le_bytes());
    data.extend_from_slice(&pitch.to_le_bytes());
    data.extend_from_slice(&buttons);
    data
}

// ═══════════════════════════════════════════════════════════════════════════════
// 1.  CONSTANTS & INVARIANTS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn min_report_bytes_equals_nine() {
    assert_eq!(CLS_E_MIN_REPORT_BYTES, 9);
}

#[test]
fn vendor_id_is_correct() {
    assert_eq!(BRUNNER_VENDOR_ID, 0x25BB);
}

#[test]
fn cls_e_yoke_pid_is_correct() {
    assert_eq!(BRUNNER_CLS_E_YOKE_PID, 0x0063);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 2.  ERROR / REJECTION CASES
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn empty_report_is_rejected() {
    assert_eq!(parse_cls_e_report(&[]), Err(ClsEParseError::TooShort(0)));
}

#[test]
fn single_byte_is_rejected() {
    assert_eq!(
        parse_cls_e_report(&[0x01]),
        Err(ClsEParseError::TooShort(1))
    );
}

#[test]
fn seven_bytes_is_rejected() {
    let data = [0x01, 0, 0, 0, 0, 0, 0];
    assert_eq!(parse_cls_e_report(&data), Err(ClsEParseError::TooShort(7)));
}

#[test]
fn eight_bytes_is_rejected() {
    let data = [0x01, 0, 0, 0, 0, 0, 0, 0];
    assert_eq!(parse_cls_e_report(&data), Err(ClsEParseError::TooShort(8)));
}

#[test]
fn every_length_below_minimum_is_rejected() {
    for len in 0..CLS_E_MIN_REPORT_BYTES {
        let data = vec![0u8; len];
        assert_eq!(
            parse_cls_e_report(&data),
            Err(ClsEParseError::TooShort(len)),
            "length {len} should be rejected"
        );
    }
}

#[test]
fn error_display_contains_actual_length() {
    let err = ClsEParseError::TooShort(3);
    let msg = err.to_string();
    assert!(msg.contains('3'), "error message should contain '3': {msg}");
}

#[test]
fn error_display_contains_minimum_length() {
    let err = ClsEParseError::TooShort(3);
    let msg = err.to_string();
    assert!(
        msg.contains(&CLS_E_MIN_REPORT_BYTES.to_string()),
        "error message should mention minimum: {msg}"
    );
}

#[test]
fn error_clone_eq() {
    let a = ClsEParseError::TooShort(5);
    let b = a.clone();
    assert_eq!(a, b);
}

#[test]
fn error_debug_format() {
    let err = ClsEParseError::TooShort(4);
    let dbg = format!("{err:?}");
    assert!(
        dbg.contains("TooShort"),
        "debug should contain variant name: {dbg}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// 3.  AXIS NORMALISATION — BOUNDARY VALUES
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn zero_axes_normalise_to_zero() {
    let state = parse_cls_e_report(&make_report(0, 0, [0; 4])).unwrap();
    assert_eq!(state.axes.roll, 0.0);
    assert_eq!(state.axes.pitch, 0.0);
}

#[test]
fn positive_one_raw_normalises_near_zero() {
    let state = parse_cls_e_report(&make_report(1, 1, [0; 4])).unwrap();
    let expected = 1.0_f32 / 32767.0;
    assert!((state.axes.roll - expected).abs() < 1e-7);
    assert!((state.axes.pitch - expected).abs() < 1e-7);
}

#[test]
fn negative_one_raw_normalises_near_zero() {
    let state = parse_cls_e_report(&make_report(-1, -1, [0; 4])).unwrap();
    let expected = -1.0_f32 / 32767.0;
    assert!((state.axes.roll - expected).abs() < 1e-7);
    assert!((state.axes.pitch - expected).abs() < 1e-7);
}

#[test]
fn max_positive_normalises_to_exactly_one() {
    let state = parse_cls_e_report(&make_report(i16::MAX, i16::MAX, [0; 4])).unwrap();
    assert_eq!(state.axes.roll, 1.0);
    assert_eq!(state.axes.pitch, 1.0);
}

#[test]
fn max_negative_clamped_to_minus_one() {
    // i16::MIN = -32768, which would be -32768/32767 ≈ -1.00003 without clamping
    let state = parse_cls_e_report(&make_report(i16::MIN, i16::MIN, [0; 4])).unwrap();
    assert_eq!(state.axes.roll, -1.0);
    assert_eq!(state.axes.pitch, -1.0);
}

#[test]
fn negative_32767_normalises_to_minus_one() {
    let state = parse_cls_e_report(&make_report(-32767, -32767, [0; 4])).unwrap();
    assert_eq!(state.axes.roll, -1.0);
    assert_eq!(state.axes.pitch, -1.0);
}

#[test]
fn half_positive_normalises_correctly() {
    let half = 16383_i16; // ~0.5
    let state = parse_cls_e_report(&make_report(half, half, [0; 4])).unwrap();
    let expected = half as f32 / 32767.0;
    assert!((state.axes.roll - expected).abs() < 1e-5);
    assert!((state.axes.pitch - expected).abs() < 1e-5);
}

#[test]
fn half_negative_normalises_correctly() {
    let half = -16384_i16; // ~-0.5
    let state = parse_cls_e_report(&make_report(half, half, [0; 4])).unwrap();
    let expected = (half as f32 / 32767.0).clamp(-1.0, 1.0);
    assert!((state.axes.roll - expected).abs() < 1e-5);
    assert!((state.axes.pitch - expected).abs() < 1e-5);
}

#[test]
fn roll_and_pitch_are_independent() {
    let state = parse_cls_e_report(&make_report(i16::MAX, i16::MIN, [0; 4])).unwrap();
    assert_eq!(state.axes.roll, 1.0);
    assert_eq!(state.axes.pitch, -1.0);
}

#[test]
fn axes_are_little_endian() {
    // Roll = 0x0100 = 256 LE (bytes: 0x00, 0x01)
    let data = [0x01, 0x00, 0x01, 0x00, 0x00, 0, 0, 0, 0];
    let state = parse_cls_e_report(&data).unwrap();
    let expected_roll = 256.0_f32 / 32767.0;
    assert!(
        (state.axes.roll - expected_roll).abs() < 1e-5,
        "roll={} expected={}",
        state.axes.roll,
        expected_roll
    );
    assert_eq!(state.axes.pitch, 0.0);
}

#[test]
fn axis_normalisation_is_symmetric_around_zero() {
    for &v in &[100_i16, 1000, 10000, 16383, 32000] {
        let pos = parse_cls_e_report(&make_report(v, 0, [0; 4]))
            .unwrap()
            .axes
            .roll;
        let neg = parse_cls_e_report(&make_report(-v, 0, [0; 4]))
            .unwrap()
            .axes
            .roll;
        assert!(
            (pos + neg).abs() < 1e-5,
            "symmetry broken for ±{v}: pos={pos}, neg={neg}"
        );
    }
}

#[test]
fn normalisation_is_monotonic() {
    let mut prev = -2.0_f32;
    for raw in (i16::MIN..=i16::MAX).step_by(1024) {
        let cur = parse_cls_e_report(&make_report(raw, 0, [0; 4]))
            .unwrap()
            .axes
            .roll;
        assert!(
            cur >= prev,
            "monotonicity violated: raw={raw}, prev={prev}, cur={cur}"
        );
        prev = cur;
    }
}

#[test]
fn axes_never_nan_or_inf() {
    for &raw in &[i16::MIN, -32767, -1, 0, 1, 32766, i16::MAX] {
        let state = parse_cls_e_report(&make_report(raw, raw, [0; 4])).unwrap();
        assert!(!state.axes.roll.is_nan(), "roll NaN at raw={raw}");
        assert!(!state.axes.roll.is_infinite(), "roll Inf at raw={raw}");
        assert!(!state.axes.pitch.is_nan(), "pitch NaN at raw={raw}");
        assert!(!state.axes.pitch.is_infinite(), "pitch Inf at raw={raw}");
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 4.  BUTTON EXTRACTION
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn no_buttons_pressed_when_zeroed() {
    let state = parse_cls_e_report(&make_report(0, 0, [0; 4])).unwrap();
    assert!(state.buttons.pressed().is_empty());
    for n in 1..=32 {
        assert!(!state.buttons.is_pressed(n));
    }
}

#[test]
fn all_32_buttons_pressed() {
    let state = parse_cls_e_report(&make_report(0, 0, [0xFF; 4])).unwrap();
    let pressed = state.buttons.pressed();
    assert_eq!(pressed.len(), 32);
    for n in 1..=32 {
        assert!(state.buttons.is_pressed(n), "button {n} should be pressed");
    }
}

#[test]
fn individual_buttons_1_through_8() {
    for bit in 0..8u8 {
        let mut btns = [0u8; 4];
        btns[0] = 1 << bit;
        let state = parse_cls_e_report(&make_report(0, 0, btns)).unwrap();
        let expected_button = bit + 1;
        assert!(
            state.buttons.is_pressed(expected_button),
            "button {expected_button} not detected with byte[0]=0x{:02X}",
            btns[0]
        );
        assert_eq!(
            state.buttons.pressed(),
            vec![expected_button],
            "only button {expected_button} should be pressed"
        );
    }
}

#[test]
fn individual_buttons_9_through_16() {
    for bit in 0..8u8 {
        let mut btns = [0u8; 4];
        btns[1] = 1 << bit;
        let state = parse_cls_e_report(&make_report(0, 0, btns)).unwrap();
        let expected_button = bit + 9;
        assert!(state.buttons.is_pressed(expected_button));
        assert_eq!(state.buttons.pressed(), vec![expected_button]);
    }
}

#[test]
fn individual_buttons_17_through_24() {
    for bit in 0..8u8 {
        let mut btns = [0u8; 4];
        btns[2] = 1 << bit;
        let state = parse_cls_e_report(&make_report(0, 0, btns)).unwrap();
        let expected_button = bit + 17;
        assert!(state.buttons.is_pressed(expected_button));
        assert_eq!(state.buttons.pressed(), vec![expected_button]);
    }
}

#[test]
fn individual_buttons_25_through_32() {
    for bit in 0..8u8 {
        let mut btns = [0u8; 4];
        btns[3] = 1 << bit;
        let state = parse_cls_e_report(&make_report(0, 0, btns)).unwrap();
        let expected_button = bit + 25;
        assert!(state.buttons.is_pressed(expected_button));
        assert_eq!(state.buttons.pressed(), vec![expected_button]);
    }
}

#[test]
fn button_0_is_always_false() {
    let state = parse_cls_e_report(&make_report(0, 0, [0xFF; 4])).unwrap();
    assert!(!state.buttons.is_pressed(0));
}

#[test]
fn button_33_is_always_false() {
    let state = parse_cls_e_report(&make_report(0, 0, [0xFF; 4])).unwrap();
    assert!(!state.buttons.is_pressed(33));
}

#[test]
fn button_255_is_always_false() {
    let state = parse_cls_e_report(&make_report(0, 0, [0xFF; 4])).unwrap();
    assert!(!state.buttons.is_pressed(255));
}

#[test]
fn buttons_pressed_returns_sorted_ascending() {
    let btns = [0b0000_0101, 0b0000_0010, 0, 0]; // buttons 1, 3, 10
    let state = parse_cls_e_report(&make_report(0, 0, btns)).unwrap();
    let pressed = state.buttons.pressed();
    assert_eq!(pressed, vec![1, 3, 10]);
    // Verify already sorted
    let mut sorted = pressed.clone();
    sorted.sort();
    assert_eq!(pressed, sorted);
}

#[test]
fn simultaneous_buttons_across_all_bytes() {
    // Button 1 (byte0 bit0), button 9 (byte1 bit0), button 17 (byte2 bit0), button 25 (byte3 bit0)
    let btns = [0x01, 0x01, 0x01, 0x01];
    let state = parse_cls_e_report(&make_report(0, 0, btns)).unwrap();
    assert_eq!(state.buttons.pressed(), vec![1, 9, 17, 25]);
}

#[test]
fn button_boundary_byte_transitions() {
    // Buttons 8 and 9 straddle byte 0/1 boundary
    let btns = [0x80, 0x01, 0, 0]; // button 8 (byte0 bit7) and button 9 (byte1 bit0)
    let state = parse_cls_e_report(&make_report(0, 0, btns)).unwrap();
    assert!(state.buttons.is_pressed(8));
    assert!(state.buttons.is_pressed(9));
    assert_eq!(state.buttons.pressed(), vec![8, 9]);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 5.  REPORT TOLERANCE — EXTRA BYTES / VARYING REPORT-ID
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn extra_trailing_bytes_are_ignored() {
    let mut report = make_report(1000, -1000, [0; 4]);
    report.extend_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]);
    let state = parse_cls_e_report(&report).unwrap();
    let expected = 1000.0_f32 / 32767.0;
    assert!((state.axes.roll - expected).abs() < 1e-5);
    assert!((state.axes.pitch + expected).abs() < 1e-5);
}

#[test]
fn report_id_byte_is_skipped_not_validated() {
    // The parser skips byte 0 regardless of its value
    let report_a = make_report_with_id(0x01, 500, 500, [0; 4]);
    let report_b = make_report_with_id(0xFF, 500, 500, [0; 4]);
    let state_a = parse_cls_e_report(&report_a).unwrap();
    let state_b = parse_cls_e_report(&report_b).unwrap();
    assert_eq!(state_a.axes.roll, state_b.axes.roll);
    assert_eq!(state_a.axes.pitch, state_b.axes.pitch);
}

#[test]
fn report_id_zero_parses_normally() {
    let report = make_report_with_id(0x00, 0, 0, [0; 4]);
    assert!(parse_cls_e_report(&report).is_ok());
}

#[test]
fn exactly_minimum_length_succeeds() {
    let report = make_report(0, 0, [0; 4]);
    assert_eq!(report.len(), CLS_E_MIN_REPORT_BYTES);
    assert!(parse_cls_e_report(&report).is_ok());
}

#[test]
fn large_report_succeeds() {
    let mut report = make_report(0, 0, [0; 4]);
    report.extend_from_slice(&[0u8; 256]);
    assert!(parse_cls_e_report(&report).is_ok());
}

// ═══════════════════════════════════════════════════════════════════════════════
// 6.  STRUCT DEFAULTS & TRAITS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn default_axes_are_zero() {
    let axes = ClsEAxes::default();
    assert_eq!(axes.roll, 0.0);
    assert_eq!(axes.pitch, 0.0);
}

#[test]
fn default_buttons_are_unpressed() {
    let buttons = ClsEButtons::default();
    assert_eq!(buttons.raw, [0; 4]);
    assert!(buttons.pressed().is_empty());
}

#[test]
fn default_input_state_is_neutral() {
    let state = ClsEInputState::default();
    assert_eq!(state.axes.roll, 0.0);
    assert_eq!(state.axes.pitch, 0.0);
    assert!(state.buttons.pressed().is_empty());
}

#[test]
fn axes_clone_is_equal() {
    let axes = ClsEAxes {
        roll: 0.5,
        pitch: -0.25,
    };
    let cloned = axes.clone();
    assert_eq!(axes, cloned);
}

#[test]
fn buttons_clone_is_equal() {
    let buttons = ClsEButtons {
        raw: [0x01, 0x02, 0x04, 0x08],
    };
    let cloned = buttons.clone();
    assert_eq!(buttons, cloned);
}

#[test]
fn input_state_clone_is_equal() {
    let state = parse_cls_e_report(&make_report(1234, -5678, [0xAA, 0xBB, 0xCC, 0xDD])).unwrap();
    let cloned = state.clone();
    assert_eq!(state, cloned);
}

#[test]
fn axes_debug_contains_field_names() {
    let axes = ClsEAxes {
        roll: 0.5,
        pitch: -0.5,
    };
    let dbg = format!("{axes:?}");
    assert!(dbg.contains("roll"), "debug missing 'roll': {dbg}");
    assert!(dbg.contains("pitch"), "debug missing 'pitch': {dbg}");
}

#[test]
fn buttons_debug_contains_raw() {
    let buttons = ClsEButtons { raw: [1, 2, 3, 4] };
    let dbg = format!("{buttons:?}");
    assert!(dbg.contains("raw"), "debug missing 'raw': {dbg}");
}

#[test]
fn input_state_debug_format() {
    let state = ClsEInputState::default();
    let dbg = format!("{state:?}");
    assert!(dbg.contains("axes"));
    assert!(dbg.contains("buttons"));
}

// ═══════════════════════════════════════════════════════════════════════════════
// 7.  DEVICE IDENTIFICATION (re-exported from flight-hid-support)
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn brunner_yoke_is_detected() {
    assert!(is_brunner_device(BRUNNER_VENDOR_ID, BRUNNER_CLS_E_YOKE_PID));
}

#[test]
fn brunner_joystick_is_detected() {
    assert!(is_brunner_device(
        BRUNNER_VENDOR_ID,
        BRUNNER_CLS_E_JOYSTICK_PID
    ));
}

#[test]
fn brunner_ng_yoke_is_detected() {
    assert!(is_brunner_device(
        BRUNNER_VENDOR_ID,
        BRUNNER_CLS_E_NG_YOKE_PID
    ));
}

#[test]
fn brunner_rudder_is_detected() {
    assert!(is_brunner_device(
        BRUNNER_VENDOR_ID,
        BRUNNER_CLS_E_RUDDER_PID
    ));
}

#[test]
fn wrong_vendor_id_is_not_detected() {
    assert!(!is_brunner_device(0x1234, 0x0063));
}

#[test]
fn wrong_product_id_is_not_detected() {
    assert!(!is_brunner_device(0x25BB, 0x9999));
}

#[test]
fn zero_vid_pid_is_not_brunner() {
    assert!(!is_brunner_device(0, 0));
}

#[test]
fn brunner_model_yoke() {
    assert_eq!(brunner_model(0x0063), Some(BrunnerModel::ClsE));
}

#[test]
fn brunner_model_joystick() {
    assert_eq!(brunner_model(0x0067), Some(BrunnerModel::ClsEJoystick));
}

#[test]
fn brunner_model_ng_yoke() {
    assert_eq!(brunner_model(0x006D), Some(BrunnerModel::ClsENgYoke));
}

#[test]
fn brunner_model_rudder() {
    assert_eq!(brunner_model(0x006B), Some(BrunnerModel::ClsERudder));
}

#[test]
fn brunner_model_unknown_pid_is_none() {
    assert_eq!(brunner_model(0x0000), None);
    assert_eq!(brunner_model(0xFFFF), None);
}

#[test]
fn brunner_model_names_are_nonempty() {
    let models = [
        BrunnerModel::ClsE,
        BrunnerModel::ClsEJoystick,
        BrunnerModel::ClsENgYoke,
        BrunnerModel::ClsERudder,
    ];
    for m in &models {
        let name = m.name();
        assert!(!name.is_empty(), "{m:?} has empty name");
        assert!(
            name.contains("Brunner"),
            "{m:?} name should contain 'Brunner': {name}"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 8.  ROUND-TRIP / IDEMPOTENCE
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn same_input_produces_same_output() {
    let report = make_report(12345, -23456, [0xDE, 0xAD, 0xBE, 0xEF]);
    let a = parse_cls_e_report(&report).unwrap();
    let b = parse_cls_e_report(&report).unwrap();
    assert_eq!(a, b);
}

#[test]
fn parse_is_deterministic_across_100_calls() {
    let report = make_report(-10000, 20000, [0x55, 0xAA, 0x55, 0xAA]);
    let reference = parse_cls_e_report(&report).unwrap();
    for _ in 0..100 {
        assert_eq!(parse_cls_e_report(&report).unwrap(), reference);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 9.  COMBINED AXES + BUTTONS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn full_deflection_with_all_buttons() {
    let state = parse_cls_e_report(&make_report(i16::MAX, i16::MIN, [0xFF; 4])).unwrap();
    assert_eq!(state.axes.roll, 1.0);
    assert_eq!(state.axes.pitch, -1.0);
    assert_eq!(state.buttons.pressed().len(), 32);
}

#[test]
fn mixed_axes_and_sparse_buttons() {
    let btns = [0b0000_0001, 0, 0, 0b1000_0000]; // buttons 1 and 32
    let state = parse_cls_e_report(&make_report(5000, -5000, btns)).unwrap();
    let expected_roll = 5000.0_f32 / 32767.0;
    assert!((state.axes.roll - expected_roll).abs() < 1e-5);
    assert!((state.axes.pitch + expected_roll).abs() < 1e-5);
    assert_eq!(state.buttons.pressed(), vec![1, 32]);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 10. BYTE-LEVEL ENCODING VERIFICATION
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn manual_byte_layout_roll_positive() {
    // roll = 0x7FFF (32767) LE: [0xFF, 0x7F]
    let data = [0x01, 0xFF, 0x7F, 0x00, 0x00, 0, 0, 0, 0];
    let state = parse_cls_e_report(&data).unwrap();
    assert_eq!(state.axes.roll, 1.0);
    assert_eq!(state.axes.pitch, 0.0);
}

#[test]
fn manual_byte_layout_pitch_negative() {
    // pitch = 0x8000 (-32768) LE: [0x00, 0x80]
    let data = [0x01, 0x00, 0x00, 0x00, 0x80, 0, 0, 0, 0];
    let state = parse_cls_e_report(&data).unwrap();
    assert_eq!(state.axes.roll, 0.0);
    assert_eq!(state.axes.pitch, -1.0);
}

#[test]
fn manual_byte_layout_negative_one() {
    // -1 as i16 LE = [0xFF, 0xFF]
    let data = [0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0, 0, 0, 0];
    let state = parse_cls_e_report(&data).unwrap();
    let expected = -1.0_f32 / 32767.0;
    assert!((state.axes.roll - expected).abs() < 1e-7);
    assert!((state.axes.pitch - expected).abs() < 1e-7);
}

#[test]
fn manual_byte_layout_button_bit_positions() {
    // byte 5 = 0b10101010 → buttons 2, 4, 6, 8
    let data = [0x01, 0, 0, 0, 0, 0xAA, 0, 0, 0];
    let state = parse_cls_e_report(&data).unwrap();
    assert_eq!(state.buttons.pressed(), vec![2, 4, 6, 8]);
}

// ═══════════════════════════════════════════════════════════════════════════════
// 11. PROPTEST — PROPERTY-BASED FUZZING
// ═══════════════════════════════════════════════════════════════════════════════

mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn axes_always_within_bounds(roll in i16::MIN..=i16::MAX, pitch in i16::MIN..=i16::MAX) {
            let report = make_report(roll, pitch, [0u8; 4]);
            let state = parse_cls_e_report(&report).unwrap();
            prop_assert!((-1.0..=1.0).contains(&state.axes.roll),
                "roll={} out of [-1,1] for raw={}", state.axes.roll, roll);
            prop_assert!((-1.0..=1.0).contains(&state.axes.pitch),
                "pitch={} out of [-1,1] for raw={}", state.axes.pitch, pitch);
        }

        #[test]
        fn parsed_button_count_le_32(b0 in 0u8..=255, b1 in 0u8..=255, b2 in 0u8..=255, b3 in 0u8..=255) {
            let report = make_report(0, 0, [b0, b1, b2, b3]);
            let state = parse_cls_e_report(&report).unwrap();
            prop_assert!(state.buttons.pressed().len() <= 32);
        }

        #[test]
        fn button_count_matches_popcount(b0 in 0u8..=255, b1 in 0u8..=255, b2 in 0u8..=255, b3 in 0u8..=255) {
            let btns = [b0, b1, b2, b3];
            let report = make_report(0, 0, btns);
            let state = parse_cls_e_report(&report).unwrap();
            let popcount = btns.iter().map(|b| b.count_ones() as usize).sum::<usize>();
            prop_assert_eq!(state.buttons.pressed().len(), popcount);
        }

        #[test]
        fn random_valid_report_never_panics(data in proptest::collection::vec(0u8..=255, 9..64)) {
            let _ = parse_cls_e_report(&data);
        }

        #[test]
        fn short_reports_always_error(data in proptest::collection::vec(0u8..=255, 0..9usize)) {
            let result = parse_cls_e_report(&data);
            prop_assert!(result.is_err());
            match result.unwrap_err() {
                ClsEParseError::TooShort(len) => prop_assert_eq!(len, data.len()),
            }
        }

        #[test]
        fn axes_are_finite(roll in i16::MIN..=i16::MAX, pitch in i16::MIN..=i16::MAX) {
            let report = make_report(roll, pitch, [0u8; 4]);
            let state = parse_cls_e_report(&report).unwrap();
            prop_assert!(state.axes.roll.is_finite());
            prop_assert!(state.axes.pitch.is_finite());
        }

        #[test]
        fn positive_raw_yields_nonneg_axis(raw in 0i16..=i16::MAX) {
            let report = make_report(raw, raw, [0u8; 4]);
            let state = parse_cls_e_report(&report).unwrap();
            prop_assert!(state.axes.roll >= 0.0, "raw={raw} gave roll={}", state.axes.roll);
            prop_assert!(state.axes.pitch >= 0.0, "raw={raw} gave pitch={}", state.axes.pitch);
        }

        #[test]
        fn negative_raw_yields_nonpos_axis(raw in i16::MIN..=0i16) {
            let report = make_report(raw, raw, [0u8; 4]);
            let state = parse_cls_e_report(&report).unwrap();
            prop_assert!(state.axes.roll <= 0.0, "raw={raw} gave roll={}", state.axes.roll);
            prop_assert!(state.axes.pitch <= 0.0, "raw={raw} gave pitch={}", state.axes.pitch);
        }
    }
}
