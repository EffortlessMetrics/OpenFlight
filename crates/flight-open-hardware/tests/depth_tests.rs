// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for flight-open-hardware HID report protocol.
//!
//! Covers: protocol parsing, boundary values, malformed input, device identification,
//! axis/button mapping, normalization, state-machine-like transitions, and
//! property-based fuzzing with proptest.

use flight_open_hardware::firmware_version::{FIRMWARE_REPORT_ID, FirmwareVersionReport};
use flight_open_hardware::input_report::{INPUT_REPORT_ID, INPUT_REPORT_LEN, InputReport};
use flight_open_hardware::led_report::{LED_REPORT_ID, LED_REPORT_LEN, LedReport, led_flags};
use flight_open_hardware::output_report::{FFB_REPORT_ID, FFB_REPORT_LEN, FfbMode, FfbOutputReport};
use flight_open_hardware::{PRODUCT_ID, VENDOR_ID};

use proptest::prelude::*;

// ────────────────────────────────────────────────────────────────────────────
// Device identification and USB constants
// ────────────────────────────────────────────────────────────────────────────

#[test]
fn usb_vendor_id_is_pid_codes() {
    assert_eq!(VENDOR_ID, 0x1209);
}

#[test]
fn usb_product_id_is_openflight() {
    assert_eq!(PRODUCT_ID, 0xF170);
}

#[test]
fn report_ids_are_distinct() {
    let ids = [INPUT_REPORT_ID, FFB_REPORT_ID, LED_REPORT_ID, FIRMWARE_REPORT_ID];
    for i in 0..ids.len() {
        for j in (i + 1)..ids.len() {
            assert_ne!(ids[i], ids[j], "report IDs must be unique");
        }
    }
}

#[test]
fn report_lengths_match_spec() {
    assert_eq!(INPUT_REPORT_LEN, 16);
    assert_eq!(FFB_REPORT_LEN, 8);
    assert_eq!(LED_REPORT_LEN, 4);
}

// ────────────────────────────────────────────────────────────────────────────
// InputReport — protocol parsing
// ────────────────────────────────────────────────────────────────────────────

fn make_input_buf(
    x: i16,
    y: i16,
    twist: i16,
    throttle: u8,
    buttons: u16,
    hat: u8,
    fault: bool,
) -> [u8; INPUT_REPORT_LEN] {
    let mut buf = [0u8; INPUT_REPORT_LEN];
    buf[0] = INPUT_REPORT_ID;
    buf[1..3].copy_from_slice(&x.to_le_bytes());
    buf[3..5].copy_from_slice(&y.to_le_bytes());
    buf[5..7].copy_from_slice(&twist.to_le_bytes());
    buf[7] = throttle;
    buf[8..10].copy_from_slice(&buttons.to_le_bytes());
    buf[10] = hat;
    buf[11] = fault as u8;
    buf
}

#[test]
fn input_parse_full_positive_deflection() {
    let buf = make_input_buf(32767, 32767, 32767, 255, 0xFFFF, 8, false);
    let r = InputReport::parse(&buf).unwrap();
    assert_eq!(r.x, 32767);
    assert_eq!(r.y, 32767);
    assert_eq!(r.twist, 32767);
    assert_eq!(r.throttle, 255);
    assert_eq!(r.buttons, 0xFFFF);
    assert_eq!(r.hat, 8);
}

#[test]
fn input_parse_full_negative_deflection() {
    let buf = make_input_buf(-32767, -32767, -32767, 0, 0, 0, true);
    let r = InputReport::parse(&buf).unwrap();
    assert_eq!(r.x, -32767);
    assert_eq!(r.y, -32767);
    assert_eq!(r.twist, -32767);
    assert_eq!(r.throttle, 0);
    assert!(r.ffb_fault);
}

#[test]
fn input_parse_i16_min_edge() {
    let buf = make_input_buf(i16::MIN, i16::MIN, i16::MIN, 0, 0, 0, false);
    let r = InputReport::parse(&buf).unwrap();
    assert_eq!(r.x, i16::MIN);
    assert_eq!(r.y, i16::MIN);
    assert_eq!(r.twist, i16::MIN);
}

#[test]
fn input_reserved_bytes_ignored_on_parse() {
    let mut buf = make_input_buf(0, 0, 0, 0, 0, 0, false);
    buf[12] = 0xFF;
    buf[13] = 0xFF;
    buf[14] = 0xFF;
    buf[15] = 0xFF;
    let r = InputReport::parse(&buf).unwrap();
    assert_eq!(r.x, 0);
}

#[test]
fn input_serialise_reserved_bytes_are_zero() {
    let report = InputReport {
        x: 1000,
        y: -1000,
        twist: 500,
        throttle: 128,
        buttons: 0xABCD,
        hat: 7,
        ffb_fault: true,
    };
    let bytes = report.to_bytes();
    assert_eq!(bytes[12], 0);
    assert_eq!(bytes[13], 0);
    assert_eq!(bytes[14], 0);
    assert_eq!(bytes[15], 0);
}

#[test]
fn input_parse_exactly_16_bytes() {
    let buf = make_input_buf(100, 200, 300, 50, 0x0001, 1, false);
    assert!(InputReport::parse(&buf).is_some());
}

#[test]
fn input_parse_longer_buffer_accepted() {
    let mut buf = [0u8; 32];
    buf[0] = INPUT_REPORT_ID;
    buf[1..3].copy_from_slice(&500_i16.to_le_bytes());
    assert!(InputReport::parse(&buf).is_some());
}

#[test]
fn input_parse_15_bytes_rejected() {
    let buf = [INPUT_REPORT_ID; 15];
    assert!(InputReport::parse(&buf).is_none());
}

#[test]
fn input_parse_every_wrong_report_id() {
    let wrong_ids: Vec<u8> = [0x00, 0x02, 0x10, 0x20, 0xF0, 0xFF]
        .iter()
        .filter(|&&id| id != INPUT_REPORT_ID)
        .copied()
        .collect();
    for id in wrong_ids {
        let mut buf = [0u8; INPUT_REPORT_LEN];
        buf[0] = id;
        assert!(
            InputReport::parse(&buf).is_none(),
            "should reject report ID {id:#04x}"
        );
    }
}

// ────────────────────────────────────────────────────────────────────────────
// InputReport — axis normalization
// ────────────────────────────────────────────────────────────────────────────

#[test]
fn input_x_norm_zero() {
    let r = InputReport {
        x: 0,
        y: 0,
        twist: 0,
        throttle: 0,
        buttons: 0,
        hat: 0,
        ffb_fault: false,
    };
    assert!((r.x_norm()).abs() < f32::EPSILON);
}

#[test]
fn input_y_norm_boundaries() {
    let pos = InputReport {
        x: 0,
        y: 32767,
        twist: 0,
        throttle: 0,
        buttons: 0,
        hat: 0,
        ffb_fault: false,
    };
    let neg = InputReport {
        x: 0,
        y: -32767,
        twist: 0,
        throttle: 0,
        buttons: 0,
        hat: 0,
        ffb_fault: false,
    };
    assert!((pos.y_norm() - 1.0).abs() < 1e-4);
    assert!((neg.y_norm() + 1.0).abs() < 1e-4);
}

#[test]
fn input_throttle_norm_boundaries() {
    let zero = InputReport {
        x: 0,
        y: 0,
        twist: 0,
        throttle: 0,
        buttons: 0,
        hat: 0,
        ffb_fault: false,
    };
    let full = InputReport {
        x: 0,
        y: 0,
        twist: 0,
        throttle: 255,
        buttons: 0,
        hat: 0,
        ffb_fault: false,
    };
    let mid = InputReport {
        x: 0,
        y: 0,
        twist: 0,
        throttle: 128,
        buttons: 0,
        hat: 0,
        ffb_fault: false,
    };
    assert!(zero.throttle_norm().abs() < f32::EPSILON);
    assert!((full.throttle_norm() - 1.0).abs() < 1e-3);
    assert!((mid.throttle_norm() - 128.0 / 255.0).abs() < 1e-3);
}

#[test]
fn input_x_norm_clamped_at_i16_min() {
    let r = InputReport {
        x: i16::MIN,
        y: 0,
        twist: 0,
        throttle: 0,
        buttons: 0,
        hat: 0,
        ffb_fault: false,
    };
    assert_eq!(r.x_norm(), -1.0);
}

// ────────────────────────────────────────────────────────────────────────────
// InputReport — button mapping
// ────────────────────────────────────────────────────────────────────────────

#[test]
fn input_individual_button_bits() {
    for bit in 0..16u16 {
        let buttons = 1u16 << bit;
        let buf = make_input_buf(0, 0, 0, 0, buttons, 0, false);
        let r = InputReport::parse(&buf).unwrap();
        assert_eq!(r.buttons, buttons, "button bit {bit} not parsed correctly");
        assert_ne!(r.buttons & (1 << bit), 0);
    }
}

#[test]
fn input_all_buttons_pressed() {
    let buf = make_input_buf(0, 0, 0, 0, 0xFFFF, 0, false);
    let r = InputReport::parse(&buf).unwrap();
    assert_eq!(r.buttons, 0xFFFF);
}

#[test]
fn input_no_buttons_pressed() {
    let buf = make_input_buf(0, 0, 0, 0, 0, 0, false);
    let r = InputReport::parse(&buf).unwrap();
    assert_eq!(r.buttons, 0);
}

// ────────────────────────────────────────────────────────────────────────────
// InputReport — hat switch
// ────────────────────────────────────────────────────────────────────────────

#[test]
fn input_hat_all_positions() {
    for pos in 0..=8u8 {
        let buf = make_input_buf(0, 0, 0, 0, 0, pos, false);
        let r = InputReport::parse(&buf).unwrap();
        assert_eq!(r.hat, pos);
    }
}

// ────────────────────────────────────────────────────────────────────────────
// FfbOutputReport — protocol parsing
// ────────────────────────────────────────────────────────────────────────────

fn make_ffb_buf(force_x: i16, force_y: i16, mode: u8, gain: u8) -> [u8; FFB_REPORT_LEN] {
    let mut buf = [0u8; FFB_REPORT_LEN];
    buf[0] = FFB_REPORT_ID;
    buf[1..3].copy_from_slice(&force_x.to_le_bytes());
    buf[3..5].copy_from_slice(&force_y.to_le_bytes());
    buf[5] = mode;
    buf[6] = gain;
    buf
}

#[test]
fn ffb_parse_all_valid_modes() {
    for (byte, expected) in [
        (0, FfbMode::Off),
        (1, FfbMode::Constant),
        (2, FfbMode::Spring),
        (3, FfbMode::Damper),
        (4, FfbMode::Friction),
    ] {
        let buf = make_ffb_buf(0, 0, byte, 128);
        let r = FfbOutputReport::parse(&buf).unwrap();
        assert_eq!(r.mode, expected);
    }
}

#[test]
fn ffb_parse_invalid_mode_bytes() {
    for mode_byte in [5, 6, 100, 254, 255] {
        let buf = make_ffb_buf(0, 0, mode_byte, 128);
        assert!(
            FfbOutputReport::parse(&buf).is_none(),
            "mode byte {mode_byte} should be rejected"
        );
    }
}

#[test]
fn ffb_parse_max_force() {
    let buf = make_ffb_buf(32767, 32767, 1, 255);
    let r = FfbOutputReport::parse(&buf).unwrap();
    assert_eq!(r.force_x, 32767);
    assert_eq!(r.force_y, 32767);
    assert_eq!(r.gain, 255);
}

#[test]
fn ffb_parse_min_force() {
    let buf = make_ffb_buf(-32767, -32767, 1, 0);
    let r = FfbOutputReport::parse(&buf).unwrap();
    assert_eq!(r.force_x, -32767);
    assert_eq!(r.force_y, -32767);
    assert_eq!(r.gain, 0);
}

#[test]
fn ffb_parse_too_short() {
    let buf = [FFB_REPORT_ID; 7];
    assert!(FfbOutputReport::parse(&buf).is_none());
}

#[test]
fn ffb_parse_empty() {
    assert!(FfbOutputReport::parse(&[]).is_none());
}

#[test]
fn ffb_stop_is_zero_force_off_mode() {
    let stop = FfbOutputReport::stop();
    assert_eq!(stop.force_x, 0);
    assert_eq!(stop.force_y, 0);
    assert_eq!(stop.mode, FfbMode::Off);
    assert_eq!(stop.gain, 0);
}

#[test]
fn ffb_reserved_byte_is_zero() {
    let cmd = FfbOutputReport {
        force_x: 32767,
        force_y: -32767,
        mode: FfbMode::Friction,
        gain: 255,
    };
    let bytes = cmd.to_bytes();
    assert_eq!(bytes[7], 0, "reserved byte must be zero");
}

#[test]
fn ffb_byte_order_le() {
    let cmd = FfbOutputReport {
        force_x: 0x1234,
        force_y: -1, // 0xFFFF in LE
        mode: FfbMode::Constant,
        gain: 0,
    };
    let b = cmd.to_bytes();
    assert_eq!(b[1], 0x34); // low byte of 0x1234
    assert_eq!(b[2], 0x12); // high byte of 0x1234
    assert_eq!(b[3], 0xFF); // low byte of -1
    assert_eq!(b[4], 0xFF); // high byte of -1
}

// ────────────────────────────────────────────────────────────────────────────
// LedReport — protocol parsing
// ────────────────────────────────────────────────────────────────────────────

#[test]
fn led_power_flag() {
    let cmd = LedReport {
        leds: led_flags::POWER,
        brightness: 255,
    };
    let bytes = cmd.to_bytes();
    let parsed = LedReport::parse(&bytes).unwrap();
    assert_ne!(parsed.leds & led_flags::POWER, 0);
    assert_eq!(parsed.leds & led_flags::PC_MODE, 0);
}

#[test]
fn led_pc_mode_flag() {
    let cmd = LedReport {
        leds: led_flags::PC_MODE,
        brightness: 100,
    };
    let parsed = LedReport::parse(&cmd.to_bytes()).unwrap();
    assert_ne!(parsed.leds & led_flags::PC_MODE, 0);
    assert_eq!(parsed.leds & led_flags::POWER, 0);
}

#[test]
fn led_both_flags() {
    let cmd = LedReport {
        leds: led_flags::POWER | led_flags::PC_MODE,
        brightness: 50,
    };
    let parsed = LedReport::parse(&cmd.to_bytes()).unwrap();
    assert_eq!(parsed.leds, led_flags::POWER | led_flags::PC_MODE);
}

#[test]
fn led_brightness_boundaries() {
    for brightness in [0u8, 1, 127, 128, 254, 255] {
        let cmd = LedReport {
            leds: 0,
            brightness,
        };
        let parsed = LedReport::parse(&cmd.to_bytes()).unwrap();
        assert_eq!(parsed.brightness, brightness);
    }
}

#[test]
fn led_parse_too_short() {
    assert!(LedReport::parse(&[LED_REPORT_ID]).is_none());
    assert!(LedReport::parse(&[LED_REPORT_ID, 0]).is_none());
    assert!(LedReport::parse(&[LED_REPORT_ID, 0, 0]).is_none());
}

#[test]
fn led_parse_empty() {
    assert!(LedReport::parse(&[]).is_none());
}

#[test]
fn led_parse_longer_buffer_accepted() {
    let mut buf = [0u8; 16];
    buf[0] = LED_REPORT_ID;
    buf[1] = led_flags::POWER;
    buf[2] = 200;
    let parsed = LedReport::parse(&buf).unwrap();
    assert_eq!(parsed.brightness, 200);
}

// ────────────────────────────────────────────────────────────────────────────
// FirmwareVersionReport
// ────────────────────────────────────────────────────────────────────────────

#[test]
fn firmware_version_tuple() {
    let fw = FirmwareVersionReport {
        major: 2,
        minor: 5,
        patch: 10,
        build_hash: [0, 0, 0, 0],
    };
    assert_eq!(fw.version(), (2, 5, 10));
}

#[test]
fn firmware_max_version_values() {
    let fw = FirmwareVersionReport {
        major: 255,
        minor: 255,
        patch: 255,
        build_hash: [0xFF, 0xFF, 0xFF, 0xFF],
    };
    let parsed = FirmwareVersionReport::parse(&fw.to_bytes()).unwrap();
    assert_eq!(parsed.version(), (255, 255, 255));
    assert_eq!(parsed.build_hash, [0xFF; 4]);
}

#[test]
fn firmware_zero_version() {
    let fw = FirmwareVersionReport {
        major: 0,
        minor: 0,
        patch: 0,
        build_hash: [0; 4],
    };
    let parsed = FirmwareVersionReport::parse(&fw.to_bytes()).unwrap();
    assert_eq!(parsed.version(), (0, 0, 0));
}

#[test]
fn firmware_parse_too_short() {
    let buf = [FIRMWARE_REPORT_ID; 7];
    assert!(FirmwareVersionReport::parse(&buf).is_none());
}

#[test]
fn firmware_parse_empty() {
    assert!(FirmwareVersionReport::parse(&[]).is_none());
}

#[test]
fn firmware_parse_wrong_ids() {
    let wrong_ids: Vec<u8> = [0x00, 0x01, 0x10, 0x20, 0xFF]
        .iter()
        .filter(|&&id| id != FIRMWARE_REPORT_ID)
        .copied()
        .collect();
    for id in wrong_ids {
        let mut buf = [0u8; 8];
        buf[0] = id;
        assert!(FirmwareVersionReport::parse(&buf).is_none());
    }
}

#[test]
fn firmware_build_hash_preserved() {
    let fw = FirmwareVersionReport {
        major: 1,
        minor: 0,
        patch: 0,
        build_hash: [0xDE, 0xAD, 0xBE, 0xEF],
    };
    let parsed = FirmwareVersionReport::parse(&fw.to_bytes()).unwrap();
    assert_eq!(parsed.build_hash, [0xDE, 0xAD, 0xBE, 0xEF]);
}

// ────────────────────────────────────────────────────────────────────────────
// Cross-report: dispatch by report ID
// ────────────────────────────────────────────────────────────────────────────

#[test]
fn report_dispatch_by_id() {
    let input_buf = make_input_buf(0, 0, 0, 0, 0, 0, false);
    let ffb_buf = FfbOutputReport::stop().to_bytes();
    let led_buf = LedReport::all_off().to_bytes();
    let fw_buf = FirmwareVersionReport {
        major: 1,
        minor: 0,
        patch: 0,
        build_hash: [0; 4],
    }
    .to_bytes();

    assert_eq!(input_buf[0], INPUT_REPORT_ID);
    assert_eq!(ffb_buf[0], FFB_REPORT_ID);
    assert_eq!(led_buf[0], LED_REPORT_ID);
    assert_eq!(fw_buf[0], FIRMWARE_REPORT_ID);

    // Each report should only parse with its own parser
    assert!(InputReport::parse(&input_buf).is_some());
    assert!(InputReport::parse(&ffb_buf).is_none());
    assert!(InputReport::parse(&led_buf).is_none());

    assert!(FfbOutputReport::parse(&ffb_buf).is_some());
    assert!(FfbOutputReport::parse(&input_buf).is_none());

    assert!(LedReport::parse(&led_buf).is_some());
    assert!(LedReport::parse(&ffb_buf).is_none());

    assert!(FirmwareVersionReport::parse(&fw_buf).is_some());
    assert!(FirmwareVersionReport::parse(&led_buf).is_none());
}

// ────────────────────────────────────────────────────────────────────────────
// State-machine-like transitions
// ────────────────────────────────────────────────────────────────────────────

#[test]
fn state_machine_connect_identify_operate_disconnect() {
    // 1. Device announces firmware
    let fw = FirmwareVersionReport {
        major: 1,
        minor: 2,
        patch: 0,
        build_hash: [0xAB, 0xCD, 0x00, 0x01],
    };
    let fw_parsed = FirmwareVersionReport::parse(&fw.to_bytes()).unwrap();
    assert_eq!(fw_parsed.version(), (1, 2, 0));

    // 2. Host enables LEDs (PC mode + power)
    let led_on = LedReport {
        leds: led_flags::POWER | led_flags::PC_MODE,
        brightness: 200,
    };
    let led_parsed = LedReport::parse(&led_on.to_bytes()).unwrap();
    assert_eq!(led_parsed.leds, led_flags::POWER | led_flags::PC_MODE);

    // 3. Device sends input reports during operation
    let input = InputReport {
        x: 5000,
        y: -3000,
        twist: 100,
        throttle: 200,
        buttons: 0x0003,
        hat: 1,
        ffb_fault: false,
    };
    let input_parsed = InputReport::parse(&input.to_bytes()).unwrap();
    assert_eq!(input_parsed.x, 5000);

    // 4. Host sends FFB spring centering
    let ffb = FfbOutputReport {
        force_x: -500,
        force_y: 300,
        mode: FfbMode::Spring,
        gain: 180,
    };
    let ffb_parsed = FfbOutputReport::parse(&ffb.to_bytes()).unwrap();
    assert_eq!(ffb_parsed.mode, FfbMode::Spring);

    // 5. Disconnect: host sends stop, LEDs off
    let stop = FfbOutputReport::stop();
    assert_eq!(stop.mode, FfbMode::Off);
    let led_off = LedReport::all_off();
    assert_eq!(led_off.brightness, 0);
}

#[test]
fn state_machine_ffb_fault_triggers_stop() {
    let normal = InputReport::parse(&make_input_buf(0, 0, 0, 128, 0, 0, false)).unwrap();
    assert!(!normal.ffb_fault);

    let fault = InputReport::parse(&make_input_buf(0, 0, 0, 128, 0, 0, true)).unwrap();
    assert!(fault.ffb_fault);

    let stop = FfbOutputReport::stop();
    let stop_parsed = FfbOutputReport::parse(&stop.to_bytes()).unwrap();
    assert_eq!(stop_parsed.mode, FfbMode::Off);
    assert_eq!(stop_parsed.gain, 0);
}

#[test]
fn state_machine_ffb_mode_transitions() {
    let modes = [
        FfbMode::Off,
        FfbMode::Constant,
        FfbMode::Spring,
        FfbMode::Damper,
        FfbMode::Friction,
        FfbMode::Off,
    ];
    for window in modes.windows(2) {
        let from = window[0];
        let to = window[1];
        let cmd = FfbOutputReport {
            force_x: 100,
            force_y: -100,
            mode: to,
            gain: 128,
        };
        let parsed = FfbOutputReport::parse(&cmd.to_bytes()).unwrap();
        assert_eq!(parsed.mode, to, "transition from {from:?} to {to:?} failed");
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Malformed input resilience
// ────────────────────────────────────────────────────────────────────────────

#[test]
fn malformed_all_zeros() {
    let buf = [0u8; 16];
    assert!(InputReport::parse(&buf).is_none());
    assert!(FfbOutputReport::parse(&buf).is_none());
    assert!(LedReport::parse(&buf).is_none());
    assert!(FirmwareVersionReport::parse(&buf).is_none());
}

#[test]
fn malformed_all_ones() {
    let buf = [0xFF; 16];
    assert!(InputReport::parse(&buf).is_none());
    assert!(FfbOutputReport::parse(&buf).is_none());
    assert!(LedReport::parse(&buf).is_none());
    assert!(FirmwareVersionReport::parse(&buf).is_none());
}

#[test]
fn malformed_single_byte_buffers() {
    for byte in [0x01, 0x10, 0x20, 0xF0] {
        assert!(InputReport::parse(&[byte]).is_none());
        assert!(FfbOutputReport::parse(&[byte]).is_none());
        assert!(LedReport::parse(&[byte]).is_none());
        assert!(FirmwareVersionReport::parse(&[byte]).is_none());
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Clone / Debug / PartialEq trait coverage
// ────────────────────────────────────────────────────────────────────────────

#[test]
fn input_report_clone_eq() {
    let r = InputReport {
        x: 1,
        y: 2,
        twist: 3,
        throttle: 4,
        buttons: 5,
        hat: 6,
        ffb_fault: true,
    };
    let cloned = r.clone();
    assert_eq!(r, cloned);
}

#[test]
fn input_report_debug() {
    let r = InputReport {
        x: 0,
        y: 0,
        twist: 0,
        throttle: 0,
        buttons: 0,
        hat: 0,
        ffb_fault: false,
    };
    let debug = format!("{r:?}");
    assert!(debug.contains("InputReport"));
}

#[test]
fn ffb_output_report_clone_eq() {
    let r = FfbOutputReport {
        force_x: -100,
        force_y: 100,
        mode: FfbMode::Damper,
        gain: 50,
    };
    assert_eq!(r, r.clone());
}

#[test]
fn ffb_mode_clone_copy_eq() {
    let m = FfbMode::Spring;
    let copied = m;
    let cloned = m.clone();
    assert_eq!(m, copied);
    assert_eq!(m, cloned);
}

#[test]
fn led_report_clone_eq() {
    let r = LedReport {
        leds: 3,
        brightness: 128,
    };
    assert_eq!(r, r.clone());
}

#[test]
fn firmware_version_clone_eq() {
    let r = FirmwareVersionReport {
        major: 1,
        minor: 2,
        patch: 3,
        build_hash: [4, 5, 6, 7],
    };
    assert_eq!(r, r.clone());
}

// ────────────────────────────────────────────────────────────────────────────
// Property-based tests (proptest)
// ────────────────────────────────────────────────────────────────────────────

proptest! {
    #[test]
    fn prop_input_roundtrip(
        x in i16::MIN..=i16::MAX,
        y in i16::MIN..=i16::MAX,
        twist in i16::MIN..=i16::MAX,
        throttle in 0u8..=255,
        buttons in 0u16..=u16::MAX,
        hat in 0u8..=255,
        ffb_fault in proptest::bool::ANY,
    ) {
        let report = InputReport { x, y, twist, throttle, buttons, hat, ffb_fault };
        let bytes = report.to_bytes();
        let parsed = InputReport::parse(&bytes).unwrap();
        prop_assert_eq!(&report, &parsed);
    }

    #[test]
    fn prop_input_report_id_always_first(
        x in i16::MIN..=i16::MAX,
        y in i16::MIN..=i16::MAX,
    ) {
        let report = InputReport {
            x, y, twist: 0, throttle: 0, buttons: 0, hat: 0, ffb_fault: false,
        };
        let bytes = report.to_bytes();
        prop_assert_eq!(bytes[0], INPUT_REPORT_ID);
    }

    #[test]
    fn prop_input_normalisation_in_range(
        x in i16::MIN..=i16::MAX,
        y in i16::MIN..=i16::MAX,
        throttle in 0u8..=255,
    ) {
        let report = InputReport {
            x, y, twist: 0, throttle, buttons: 0, hat: 0, ffb_fault: false,
        };
        let xn = report.x_norm();
        let yn = report.y_norm();
        let tn = report.throttle_norm();
        prop_assert!((-1.0..=1.0).contains(&xn), "x_norm={xn} out of range");
        prop_assert!((-1.0..=1.0).contains(&yn), "y_norm={yn} out of range");
        prop_assert!((0.0..=1.0).contains(&tn), "throttle_norm={tn} out of range");
    }

    #[test]
    fn prop_ffb_roundtrip(
        force_x in i16::MIN..=i16::MAX,
        force_y in i16::MIN..=i16::MAX,
        mode_byte in 0u8..=4,
        gain in 0u8..=255,
    ) {
        let mode = match mode_byte {
            0 => FfbMode::Off,
            1 => FfbMode::Constant,
            2 => FfbMode::Spring,
            3 => FfbMode::Damper,
            _ => FfbMode::Friction,
        };
        let report = FfbOutputReport { force_x, force_y, mode, gain };
        let bytes = report.to_bytes();
        let parsed = FfbOutputReport::parse(&bytes).unwrap();
        prop_assert_eq!(&report, &parsed);
    }

    #[test]
    fn prop_ffb_invalid_mode_rejected(mode_byte in 5u8..=255) {
        let buf = make_ffb_buf(0, 0, mode_byte, 128);
        prop_assert!(FfbOutputReport::parse(&buf).is_none());
    }

    #[test]
    fn prop_led_roundtrip(leds in 0u8..=255, brightness in 0u8..=255) {
        let report = LedReport { leds, brightness };
        let bytes = report.to_bytes();
        let parsed = LedReport::parse(&bytes).unwrap();
        prop_assert_eq!(&report, &parsed);
    }

    #[test]
    fn prop_firmware_roundtrip(
        major in 0u8..=255,
        minor in 0u8..=255,
        patch in 0u8..=255,
        b0 in 0u8..=255,
        b1 in 0u8..=255,
        b2 in 0u8..=255,
        b3 in 0u8..=255,
    ) {
        let report = FirmwareVersionReport {
            major, minor, patch, build_hash: [b0, b1, b2, b3],
        };
        let bytes = report.to_bytes();
        let parsed = FirmwareVersionReport::parse(&bytes).unwrap();
        prop_assert_eq!(&report, &parsed);
    }

    #[test]
    fn prop_arbitrary_bytes_never_panic_input(
        data in proptest::collection::vec(any::<u8>(), 0..64),
    ) {
        let _ = InputReport::parse(&data);
    }

    #[test]
    fn prop_arbitrary_bytes_never_panic_ffb(
        data in proptest::collection::vec(any::<u8>(), 0..64),
    ) {
        let _ = FfbOutputReport::parse(&data);
    }

    #[test]
    fn prop_arbitrary_bytes_never_panic_led(
        data in proptest::collection::vec(any::<u8>(), 0..64),
    ) {
        let _ = LedReport::parse(&data);
    }

    #[test]
    fn prop_arbitrary_bytes_never_panic_firmware(
        data in proptest::collection::vec(any::<u8>(), 0..64),
    ) {
        let _ = FirmwareVersionReport::parse(&data);
    }
}
