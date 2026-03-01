// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for the `flight-hotas-logitech-wheel` crate covering HID
//! parsing, device constants, normalization, and error handling for both
//! G27/G25 and G29/G920/G923 wheels.

use flight_hotas_logitech_wheel::{
    WheelError, normalize_pedal, normalize_wheel, parse_g27, parse_g29,
    g27::{G25_PID, G27_PID, G27_REPORT_ID, G27_REPORT_LEN},
    g29::{G29_PID, G29_REPORT_ID, G29_REPORT_LEN, G920_PID, G923_PS_PID, G923_XBOX_PID, LOGITECH_VID},
};

// ── Helpers ──────────────────────────────────────────────────────────────────

fn g29_report(wheel: u16, gas: u16, brake: u16, clutch: u16, buttons: u16, hat: u8) -> [u8; 12] {
    let [wl, wh] = wheel.to_le_bytes();
    let [gl, gh] = gas.to_le_bytes();
    let [bl, bh] = brake.to_le_bytes();
    let [cl, ch] = clutch.to_le_bytes();
    let [btl, bth] = buttons.to_le_bytes();
    [0x01, wl, wh, gl, gh, bl, bh, cl, ch, btl, bth, hat]
}

fn g27_report(wheel: u16, gas: u16, brake: u16, clutch: u16, buttons: u16) -> [u8; 11] {
    let [wl, wh] = wheel.to_le_bytes();
    let [gl, gh] = gas.to_le_bytes();
    let [bl, bh] = brake.to_le_bytes();
    let [cl, ch] = clutch.to_le_bytes();
    let [btl, bth] = buttons.to_le_bytes();
    [0x01, wl, wh, gl, gh, bl, bh, cl, ch, btl, bth]
}

// ── Constants ────────────────────────────────────────────────────────────────

#[test]
fn logitech_vid_is_correct() {
    assert_eq!(LOGITECH_VID, 0x046D);
}

#[test]
fn all_pids_are_distinct() {
    let pids = [G29_PID, G920_PID, G923_PS_PID, G923_XBOX_PID, G27_PID, G25_PID];
    for i in 0..pids.len() {
        for j in (i + 1)..pids.len() {
            assert_ne!(pids[i], pids[j], "PIDs at index {i} and {j} must differ");
        }
    }
}

#[test]
fn report_ids_match() {
    assert_eq!(G27_REPORT_ID, 0x01);
    assert_eq!(G29_REPORT_ID, 0x01);
}

#[test]
fn report_lengths_correct() {
    assert_eq!(G27_REPORT_LEN, 11);
    assert_eq!(G29_REPORT_LEN, 12);
}

// ── G29 parsing ──────────────────────────────────────────────────────────────

#[test]
fn g29_center_all_released() {
    let r = g29_report(32768, 0, 0, 0, 0, 8);
    let s = parse_g29(&r).unwrap();
    assert_eq!(s.wheel, 32768);
    assert_eq!(s.gas, 0);
    assert_eq!(s.brake, 0);
    assert_eq!(s.clutch, 0);
    assert_eq!(s.buttons, 0);
    assert_eq!(s.hat, 8);
}

#[test]
fn g29_full_left() {
    let s = parse_g29(&g29_report(0, 0, 0, 0, 0, 8)).unwrap();
    assert_eq!(s.wheel, 0);
}

#[test]
fn g29_full_right() {
    let s = parse_g29(&g29_report(65535, 0, 0, 0, 0, 8)).unwrap();
    assert_eq!(s.wheel, 65535);
}

#[test]
fn g29_all_pedals_full() {
    let s = parse_g29(&g29_report(32768, 65535, 65535, 65535, 0, 8)).unwrap();
    assert_eq!(s.gas, 65535);
    assert_eq!(s.brake, 65535);
    assert_eq!(s.clutch, 65535);
}

#[test]
fn g29_all_buttons_set() {
    let s = parse_g29(&g29_report(32768, 0, 0, 0, 0xFFFF, 8)).unwrap();
    assert_eq!(s.buttons, 0xFFFF);
}

#[test]
fn g29_individual_button_bits() {
    for bit in 0..16u16 {
        let s = parse_g29(&g29_report(32768, 0, 0, 0, 1 << bit, 8)).unwrap();
        assert_eq!(s.buttons, 1 << bit, "bit {bit}");
    }
}

#[test]
fn g29_hat_all_directions() {
    for hat in 0u8..=8 {
        let s = parse_g29(&g29_report(32768, 0, 0, 0, 0, hat)).unwrap();
        assert_eq!(s.hat, hat);
    }
}

#[test]
fn g29_hat_values_above_8() {
    // Values above 8 are not standard but should parse without error
    for hat in 9u8..=15 {
        let s = parse_g29(&g29_report(32768, 0, 0, 0, 0, hat)).unwrap();
        assert_eq!(s.hat, hat);
    }
}

#[test]
fn g29_clutch_mid_value() {
    let s = parse_g29(&g29_report(32768, 0, 0, 32768, 0, 8)).unwrap();
    assert_eq!(s.clutch, 32768);
}

#[test]
fn g29_too_short_all_lengths() {
    for len in 0..G29_REPORT_LEN {
        let data = vec![0x01; len];
        let err = parse_g29(&data).unwrap_err();
        assert_eq!(err, WheelError::TooShort { need: G29_REPORT_LEN, got: len });
    }
}

#[test]
fn g29_invalid_report_id_various() {
    for id in [0x00, 0x02, 0x03, 0xFF] {
        let mut r = g29_report(32768, 0, 0, 0, 0, 8);
        r[0] = id;
        assert_eq!(parse_g29(&r).unwrap_err(), WheelError::InvalidReportId(id));
    }
}

#[test]
fn g29_extra_trailing_bytes_ok() {
    let mut data = g29_report(32768, 1000, 2000, 3000, 0x1234, 5).to_vec();
    data.extend_from_slice(&[0xAA; 20]);
    let s = parse_g29(&data).unwrap();
    assert_eq!(s.wheel, 32768);
    assert_eq!(s.gas, 1000);
    assert_eq!(s.buttons, 0x1234);
    assert_eq!(s.hat, 5);
}

// ── G27 parsing ──────────────────────────────────────────────────────────────

#[test]
fn g27_center_all_released() {
    let s = parse_g27(&g27_report(32768, 0, 0, 0, 0)).unwrap();
    assert_eq!(s.wheel, 32768);
    assert_eq!(s.gas, 0);
    assert_eq!(s.brake, 0);
    assert_eq!(s.clutch, 0);
    assert_eq!(s.buttons, 0);
}

#[test]
fn g27_full_left() {
    let s = parse_g27(&g27_report(0, 0, 0, 0, 0)).unwrap();
    assert_eq!(s.wheel, 0);
}

#[test]
fn g27_full_right() {
    let s = parse_g27(&g27_report(65535, 0, 0, 0, 0)).unwrap();
    assert_eq!(s.wheel, 65535);
}

#[test]
fn g27_all_pedals_full() {
    let s = parse_g27(&g27_report(32768, 65535, 65535, 65535, 0)).unwrap();
    assert_eq!(s.gas, 65535);
    assert_eq!(s.brake, 65535);
    assert_eq!(s.clutch, 65535);
}

#[test]
fn g27_all_buttons_set() {
    let s = parse_g27(&g27_report(32768, 0, 0, 0, 0xFFFF)).unwrap();
    assert_eq!(s.buttons, 0xFFFF_u32);
}

#[test]
fn g27_individual_button_bits() {
    for bit in 0..16u16 {
        let s = parse_g27(&g27_report(32768, 0, 0, 0, 1 << bit)).unwrap();
        assert_eq!(s.buttons, (1u32 << bit), "bit {bit}");
    }
}

#[test]
fn g27_buttons_stored_as_u32() {
    // G27 buttons come from u16 but are stored as u32
    let s = parse_g27(&g27_report(32768, 0, 0, 0, 0xABCD)).unwrap();
    assert_eq!(s.buttons, 0x0000_ABCD_u32);
    // Upper 16 bits should always be zero
    assert_eq!(s.buttons & 0xFFFF_0000, 0);
}

#[test]
fn g27_too_short_all_lengths() {
    for len in 0..G27_REPORT_LEN {
        let data = vec![0x01; len];
        let err = parse_g27(&data).unwrap_err();
        assert_eq!(err, WheelError::TooShort { need: G27_REPORT_LEN, got: len });
    }
}

#[test]
fn g27_invalid_report_id_various() {
    for id in [0x00, 0x02, 0x03, 0xFF] {
        let mut r = g27_report(32768, 0, 0, 0, 0);
        r[0] = id;
        assert_eq!(parse_g27(&r).unwrap_err(), WheelError::InvalidReportId(id));
    }
}

#[test]
fn g27_extra_trailing_bytes_ok() {
    let mut data = g27_report(32768, 1000, 2000, 3000, 0x5678).to_vec();
    data.extend_from_slice(&[0xBB; 10]);
    let s = parse_g27(&data).unwrap();
    assert_eq!(s.wheel, 32768);
    assert_eq!(s.gas, 1000);
    assert_eq!(s.brake, 2000);
    assert_eq!(s.clutch, 3000);
    assert_eq!(s.buttons, 0x5678_u32);
}

// ── normalize_wheel ──────────────────────────────────────────────────────────

#[test]
fn normalize_wheel_center() {
    let v = normalize_wheel(32768);
    assert!(v.abs() < 0.01, "center: {v}");
}

#[test]
fn normalize_wheel_full_left() {
    let v = normalize_wheel(0);
    assert!(v < -0.999, "full left: {v}");
}

#[test]
fn normalize_wheel_full_right() {
    let v = normalize_wheel(65535);
    assert!(v > 0.999, "full right: {v}");
}

#[test]
fn normalize_wheel_quarter_left() {
    let v = normalize_wheel(16384);
    assert!((-0.55..=-0.45).contains(&v), "quarter left: {v}");
}

#[test]
fn normalize_wheel_quarter_right() {
    let v = normalize_wheel(49152);
    assert!((0.45..=0.55).contains(&v), "quarter right: {v}");
}

// ── normalize_pedal ──────────────────────────────────────────────────────────

#[test]
fn normalize_pedal_released() {
    assert!(normalize_pedal(0).abs() < 0.001);
}

#[test]
fn normalize_pedal_full() {
    assert!((normalize_pedal(65535) - 1.0).abs() < 0.001);
}

#[test]
fn normalize_pedal_half() {
    let v = normalize_pedal(32768);
    assert!((0.49..=0.51).contains(&v), "half: {v}");
}

#[test]
fn normalize_pedal_never_negative() {
    // Even at raw=0, pedal should not be negative
    assert!(normalize_pedal(0) >= 0.0);
}

// ── Error display ────────────────────────────────────────────────────────────

#[test]
fn error_too_short_display() {
    let e = WheelError::TooShort { need: 12, got: 5 };
    let msg = format!("{e}");
    assert!(msg.contains("12"), "{msg}");
    assert!(msg.contains("5"), "{msg}");
}

#[test]
fn error_invalid_id_display() {
    let e = WheelError::InvalidReportId(0xAB);
    let msg = format!("{e}");
    assert!(
        msg.contains("0xab") || msg.contains("0xAB") || msg.contains("0x00ab") || msg.contains("0x00AB"),
        "{msg}"
    );
}

// ── Property-based tests ─────────────────────────────────────────────────────

mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn normalize_wheel_always_in_range(raw in 0u16..=65535) {
            let v = normalize_wheel(raw);
            prop_assert!((-1.0..=1.0).contains(&v), "got {v}");
            prop_assert!(!v.is_nan());
        }

        #[test]
        fn normalize_pedal_always_in_range(raw in 0u16..=65535) {
            let v = normalize_pedal(raw);
            prop_assert!((0.0..=1.0).contains(&v), "got {v}");
            prop_assert!(!v.is_nan());
        }

        #[test]
        fn normalize_wheel_monotonic(a in 0u16..65535u16) {
            let va = normalize_wheel(a);
            let vb = normalize_wheel(a + 1);
            prop_assert!(vb >= va, "not monotonic: {a}→{va}, {}→{vb}", a + 1);
        }

        #[test]
        fn normalize_pedal_monotonic(a in 0u16..65535u16) {
            let va = normalize_pedal(a);
            let vb = normalize_pedal(a + 1);
            prop_assert!(vb >= va, "not monotonic: {a}→{va}, {}→{vb}", a + 1);
        }

        #[test]
        fn g29_any_valid_report_parses(
            wheel in 0u16..=65535,
            gas in 0u16..=65535,
            brake in 0u16..=65535,
            clutch in 0u16..=65535,
            buttons in 0u16..=65535,
            hat in 0u8..=255,
        ) {
            let r = g29_report(wheel, gas, brake, clutch, buttons, hat);
            let s = parse_g29(&r).unwrap();
            prop_assert_eq!(s.wheel, wheel);
            prop_assert_eq!(s.gas, gas);
            prop_assert_eq!(s.brake, brake);
            prop_assert_eq!(s.clutch, clutch);
            prop_assert_eq!(s.buttons, buttons);
            prop_assert_eq!(s.hat, hat);
        }

        #[test]
        fn g27_any_valid_report_parses(
            wheel in 0u16..=65535,
            gas in 0u16..=65535,
            brake in 0u16..=65535,
            clutch in 0u16..=65535,
            buttons in 0u16..=65535,
        ) {
            let r = g27_report(wheel, gas, brake, clutch, buttons);
            let s = parse_g27(&r).unwrap();
            prop_assert_eq!(s.wheel, wheel);
            prop_assert_eq!(s.gas, gas);
            prop_assert_eq!(s.brake, brake);
            prop_assert_eq!(s.clutch, clutch);
            prop_assert_eq!(s.buttons, buttons as u32);
        }

        #[test]
        fn g29_random_bytes_either_parses_or_errors(
            data in proptest::collection::vec(any::<u8>(), 0..64),
        ) {
            let result = parse_g29(&data);
            if data.len() < 12 {
                let is_short = matches!(result, Err(WheelError::TooShort { .. }));
                prop_assert!(is_short);
            } else if data[0] != 0x01 {
                let is_bad_id = matches!(result, Err(WheelError::InvalidReportId(_)));
                prop_assert!(is_bad_id);
            } else {
                prop_assert!(result.is_ok());
            }
        }

        #[test]
        fn g27_random_bytes_either_parses_or_errors(
            data in proptest::collection::vec(any::<u8>(), 0..64),
        ) {
            let result = parse_g27(&data);
            if data.len() < 11 {
                let is_short = matches!(result, Err(WheelError::TooShort { .. }));
                prop_assert!(is_short);
            } else if data[0] != 0x01 {
                let is_bad_id = matches!(result, Err(WheelError::InvalidReportId(_)));
                prop_assert!(is_bad_id);
            } else {
                prop_assert!(result.is_ok());
            }
        }
    }
}
