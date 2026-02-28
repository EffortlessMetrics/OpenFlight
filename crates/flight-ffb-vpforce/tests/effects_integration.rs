// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Integration tests covering FFB effect serialisation and device detection
//! for the VPforce Rhino FFB joystick base.

use flight_ffb_vpforce::RHINO_PIDS;
use flight_ffb_vpforce::effects::{
    FFB_REPORT_LEN, FfbEffect, REPORT_CONSTANT_FORCE, REPORT_PERIODIC, REPORT_SPRING,
    is_magnitude_safe, serialize_effect,
};
use flight_ffb_vpforce::input::{
    RHINO_PID_V2, RHINO_PID_V3, RHINO_REPORT_LEN, VPFORCE_VENDOR_ID, parse_report,
};

fn make_report(roll: i16, pitch: i16) -> [u8; RHINO_REPORT_LEN] {
    let mut r = [0u8; RHINO_REPORT_LEN];
    r[0] = 0x01;
    r[1..3].copy_from_slice(&roll.to_le_bytes());
    r[3..5].copy_from_slice(&pitch.to_le_bytes());
    r
}

/// Axes reach their full-scale values (±1.0) when the raw input is ±32767.
#[test]
fn full_deflection_x_and_y_axes() {
    let r = make_report(i16::MAX, i16::MIN);
    let s = parse_report(&r).unwrap();
    assert!((s.axes.roll - 1.0).abs() < 1e-4, "roll={}", s.axes.roll);
    // i16::MIN / 32767 ≈ −1.0003 → clamped to −1.0
    assert!((s.axes.pitch + 1.0).abs() < 1e-3, "pitch={}", s.axes.pitch);
}

/// Sine-wave effect encodes frequency and magnitude into the correct bytes.
#[test]
fn sine_wave_effect_encodes_frequency_and_magnitude() {
    let b = serialize_effect(FfbEffect::Sine {
        frequency_hz: 30.0,
        magnitude: 0.6,
    });
    assert_eq!(b[0], REPORT_PERIODIC, "wrong report ID for sine effect");
    let freq = u16::from_le_bytes([b[1], b[2]]);
    let mag = u16::from_le_bytes([b[3], b[4]]);
    assert_eq!(freq, 30, "frequency should be 30 Hz raw");
    assert_eq!(mag, 6000, "0.6 magnitude → 6000 raw");
}

/// Constant-force magnitude is serialised accurately (0.75 → 7500 raw units).
#[test]
fn constant_force_magnitude_calculation() {
    let b = serialize_effect(FfbEffect::ConstantForce {
        direction_deg: 0.0,
        magnitude: 0.75,
    });
    assert_eq!(b[0], REPORT_CONSTANT_FORCE);
    let mag = u16::from_le_bytes([b[3], b[4]]);
    assert_eq!(mag, 7500, "0.75 magnitude must encode as 7500");
    assert!(is_magnitude_safe(0.75));
}

/// Spring (centering) and Damper (friction) share report ID 0x11 but use
/// distinct mode bytes (0x01 vs 0x02) to combine both effects.
#[test]
fn spring_and_friction_combined_use_distinct_mode_bytes() {
    let spring = serialize_effect(FfbEffect::Spring { coefficient: 0.5 });
    let damper = serialize_effect(FfbEffect::Damper { coefficient: 0.5 });
    assert_eq!(spring[0], REPORT_SPRING, "spring must use REPORT_SPRING id");
    assert_eq!(spring[1], 0x01, "spring mode byte must be 0x01");
    assert_eq!(
        damper[0], REPORT_SPRING,
        "damper must reuse REPORT_SPRING id"
    );
    assert_eq!(damper[1], 0x02, "damper (friction) mode byte must be 0x02");
    // Both coefficients encode to the same raw value (0.5 * 10000 = 5000)
    let s_coeff = u16::from_le_bytes([spring[2], spring[3]]);
    let d_coeff = u16::from_le_bytes([damper[2], damper[3]]);
    assert_eq!(s_coeff, 5000);
    assert_eq!(d_coeff, 5000);
}

/// StopAll is the safe zero-force state: report ID 0xFF, magnitude 0.0 is safe.
#[test]
fn zero_force_stop_all_is_safe_to_publish() {
    let b = serialize_effect(FfbEffect::StopAll);
    assert_eq!(b[0], 0xFF, "StopAll must use sentinel report ID 0xFF");
    assert!(is_magnitude_safe(0.0));
    assert!(!is_magnitude_safe(1.01));
}

/// Effect report fits within one USB FS packet, supporting 250 Hz RT spine updates.
#[test]
fn effect_report_length_fits_250hz_budget() {
    const {
        assert!(
            FFB_REPORT_LEN <= 64,
            "FFB_REPORT_LEN must fit in one USB FS packet (64 bytes)"
        )
    };
}

/// VID/PID constants correctly identify all known VPforce Rhino variants.
#[test]
fn device_detection_by_vid_pid() {
    assert_eq!(VPFORCE_VENDOR_ID, 0x0483, "Rhino uses STM32 VID 0x0483");
    assert!(
        RHINO_PIDS.contains(&RHINO_PID_V2),
        "Rhino v2 PID must be in RHINO_PIDS"
    );
    assert!(
        RHINO_PIDS.contains(&RHINO_PID_V3),
        "Rhino v3 PID must be in RHINO_PIDS"
    );
    assert!(
        !RHINO_PIDS.contains(&0xFFFF),
        "unknown PID must not match any Rhino variant"
    );
}
