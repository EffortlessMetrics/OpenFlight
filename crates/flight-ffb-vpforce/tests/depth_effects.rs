// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for VPforce Rhino FFB effect encoding — covers direction
//! wrapping, coefficient boundaries, report layout, and all effect variants.

use flight_ffb_vpforce::effects::{
    FFB_REPORT_LEN, FfbEffect, REPORT_CONSTANT_FORCE, REPORT_PERIODIC, REPORT_SPRING,
    is_magnitude_safe, serialize_effect,
};

fn decode_u16(buf: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([buf[offset], buf[offset + 1]])
}

// ── Constant force direction wrapping ───────────────────────────────────

#[test]
fn direction_zero_degrees() {
    let b = serialize_effect(FfbEffect::ConstantForce {
        direction_deg: 0.0,
        magnitude: 0.5,
    });
    let angle_raw = decode_u16(&b, 1);
    assert_eq!(angle_raw, 0, "0° should encode as 0");
}

#[test]
fn direction_90_degrees() {
    let b = serialize_effect(FfbEffect::ConstantForce {
        direction_deg: 90.0,
        magnitude: 0.5,
    });
    let angle_raw = decode_u16(&b, 1);
    // 90/360 * 65535 = 16383.75 → 16383
    assert_eq!(angle_raw, 16383);
}

#[test]
fn direction_180_degrees() {
    let b = serialize_effect(FfbEffect::ConstantForce {
        direction_deg: 180.0,
        magnitude: 0.5,
    });
    let angle_raw = decode_u16(&b, 1);
    // 180/360 * 65535 = 32767.5 → 32767
    assert_eq!(angle_raw, 32767);
}

#[test]
fn direction_270_degrees() {
    let b = serialize_effect(FfbEffect::ConstantForce {
        direction_deg: 270.0,
        magnitude: 0.5,
    });
    let angle_raw = decode_u16(&b, 1);
    // 270/360 * 65535 = 49151.25 → 49151
    assert_eq!(angle_raw, 49151);
}

#[test]
fn direction_negative_wraps_to_positive() {
    let b = serialize_effect(FfbEffect::ConstantForce {
        direction_deg: -90.0,
        magnitude: 0.5,
    });
    // -90 → (-90 % 360 + 360) % 360 = 270
    let angle_raw = decode_u16(&b, 1);
    let expected = (270.0 / 360.0 * 65535.0) as u16;
    assert_eq!(angle_raw, expected);
}

#[test]
fn direction_over_360_wraps() {
    let b = serialize_effect(FfbEffect::ConstantForce {
        direction_deg: 450.0,
        magnitude: 0.5,
    });
    // 450 → 450 % 360 = 90
    let angle_raw = decode_u16(&b, 1);
    let expected = (90.0 / 360.0 * 65535.0) as u16;
    assert_eq!(angle_raw, expected);
}

#[test]
fn direction_720_wraps_to_zero() {
    let b = serialize_effect(FfbEffect::ConstantForce {
        direction_deg: 720.0,
        magnitude: 0.5,
    });
    let angle_raw = decode_u16(&b, 1);
    assert_eq!(angle_raw, 0);
}

// ── Constant force magnitude encoding ───────────────────────────────────

#[test]
fn constant_force_magnitude_zero() {
    let b = serialize_effect(FfbEffect::ConstantForce {
        direction_deg: 0.0,
        magnitude: 0.0,
    });
    assert_eq!(decode_u16(&b, 3), 0);
}

#[test]
fn constant_force_magnitude_half() {
    let b = serialize_effect(FfbEffect::ConstantForce {
        direction_deg: 0.0,
        magnitude: 0.5,
    });
    assert_eq!(decode_u16(&b, 3), 5000);
}

#[test]
fn constant_force_magnitude_full() {
    let b = serialize_effect(FfbEffect::ConstantForce {
        direction_deg: 0.0,
        magnitude: 1.0,
    });
    assert_eq!(decode_u16(&b, 3), 10000);
}

#[test]
fn constant_force_negative_magnitude_clamped_to_zero() {
    let b = serialize_effect(FfbEffect::ConstantForce {
        direction_deg: 0.0,
        magnitude: -0.5,
    });
    assert_eq!(decode_u16(&b, 3), 0);
}

#[test]
fn constant_force_over_magnitude_clamped_to_max() {
    let b = serialize_effect(FfbEffect::ConstantForce {
        direction_deg: 0.0,
        magnitude: 5.0,
    });
    assert_eq!(decode_u16(&b, 3), 10000);
}

// ── Spring effect ───────────────────────────────────────────────────────

#[test]
fn spring_coefficient_zero() {
    let b = serialize_effect(FfbEffect::Spring { coefficient: 0.0 });
    assert_eq!(b[0], REPORT_SPRING);
    assert_eq!(b[1], 0x01);
    assert_eq!(decode_u16(&b, 2), 0);
}

#[test]
fn spring_coefficient_full() {
    let b = serialize_effect(FfbEffect::Spring { coefficient: 1.0 });
    assert_eq!(decode_u16(&b, 2), 10000);
}

#[test]
fn spring_coefficient_negative_clamped() {
    let b = serialize_effect(FfbEffect::Spring { coefficient: -0.5 });
    assert_eq!(decode_u16(&b, 2), 0);
}

#[test]
fn spring_coefficient_over_one_clamped() {
    let b = serialize_effect(FfbEffect::Spring { coefficient: 3.0 });
    assert_eq!(decode_u16(&b, 2), 10000);
}

// ── Damper effect ───────────────────────────────────────────────────────

#[test]
fn damper_coefficient_zero() {
    let b = serialize_effect(FfbEffect::Damper { coefficient: 0.0 });
    assert_eq!(b[0], REPORT_SPRING);
    assert_eq!(b[1], 0x02);
    assert_eq!(decode_u16(&b, 2), 0);
}

#[test]
fn damper_coefficient_full() {
    let b = serialize_effect(FfbEffect::Damper { coefficient: 1.0 });
    assert_eq!(decode_u16(&b, 2), 10000);
}

#[test]
fn damper_coefficient_negative_clamped() {
    let b = serialize_effect(FfbEffect::Damper { coefficient: -1.0 });
    assert_eq!(decode_u16(&b, 2), 0);
}

// ── Sine effect ─────────────────────────────────────────────────────────

#[test]
fn sine_minimum_frequency() {
    let b = serialize_effect(FfbEffect::Sine {
        frequency_hz: 0.1,
        magnitude: 0.5,
    });
    assert_eq!(b[0], REPORT_PERIODIC);
    let freq = decode_u16(&b, 1);
    assert_eq!(freq, 1, "frequency should be clamped to minimum 1 Hz");
}

#[test]
fn sine_maximum_frequency() {
    let b = serialize_effect(FfbEffect::Sine {
        frequency_hz: 500.0,
        magnitude: 0.5,
    });
    let freq = decode_u16(&b, 1);
    assert_eq!(freq, 200, "frequency should be clamped to maximum 200 Hz");
}

#[test]
fn sine_normal_frequency() {
    let b = serialize_effect(FfbEffect::Sine {
        frequency_hz: 100.0,
        magnitude: 0.8,
    });
    let freq = decode_u16(&b, 1);
    let mag = decode_u16(&b, 3);
    assert_eq!(freq, 100);
    assert_eq!(mag, 8000);
}

#[test]
fn sine_magnitude_clamped() {
    let b = serialize_effect(FfbEffect::Sine {
        frequency_hz: 50.0,
        magnitude: 2.0,
    });
    let mag = decode_u16(&b, 3);
    assert_eq!(mag, 10000, "magnitude should be clamped to 1.0 → 10000");
}

#[test]
fn sine_negative_magnitude_clamped() {
    let b = serialize_effect(FfbEffect::Sine {
        frequency_hz: 50.0,
        magnitude: -1.0,
    });
    let mag = decode_u16(&b, 3);
    assert_eq!(mag, 0, "negative magnitude should clamp to 0");
}

// ── StopAll effect ──────────────────────────────────────────────────────

#[test]
fn stop_all_report_format() {
    let b = serialize_effect(FfbEffect::StopAll);
    assert_eq!(b[0], 0xFF, "StopAll must use 0xFF report ID");
    // Remaining bytes should be zero
    assert!(b[1..].iter().all(|&byte| byte == 0), "StopAll payload should be all zeros");
}

// ── Report layout ───────────────────────────────────────────────────────

#[test]
fn all_effects_produce_correct_length() {
    let effects = [
        FfbEffect::ConstantForce { direction_deg: 45.0, magnitude: 0.5 },
        FfbEffect::Spring { coefficient: 0.5 },
        FfbEffect::Damper { coefficient: 0.5 },
        FfbEffect::Sine { frequency_hz: 50.0, magnitude: 0.5 },
        FfbEffect::StopAll,
    ];
    for effect in &effects {
        let b = serialize_effect(*effect);
        assert_eq!(b.len(), FFB_REPORT_LEN, "effect {:?} produced wrong length", effect);
    }
}

#[test]
fn report_length_constant() {
    assert_eq!(FFB_REPORT_LEN, 8, "report must be exactly 8 bytes");
}

#[test]
fn report_id_constants() {
    assert_eq!(REPORT_CONSTANT_FORCE, 0x10);
    assert_eq!(REPORT_SPRING, 0x11);
    assert_eq!(REPORT_PERIODIC, 0x12);
}

// ── is_magnitude_safe ───────────────────────────────────────────────────

#[test]
fn magnitude_safe_at_boundaries() {
    assert!(is_magnitude_safe(0.0));
    assert!(is_magnitude_safe(1.0));
    assert!(is_magnitude_safe(0.5));
}

#[test]
fn magnitude_unsafe_outside_range() {
    assert!(!is_magnitude_safe(-0.001));
    assert!(!is_magnitude_safe(1.001));
    assert!(!is_magnitude_safe(-1.0));
    assert!(!is_magnitude_safe(f32::INFINITY));
    assert!(!is_magnitude_safe(f32::NEG_INFINITY));
    assert!(!is_magnitude_safe(f32::NAN));
}

// ── FfbEffect traits ────────────────────────────────────────────────────

#[test]
fn ffb_effect_debug() {
    let effect = FfbEffect::Spring { coefficient: 0.3 };
    let dbg = format!("{effect:?}");
    assert!(dbg.contains("Spring"));
    assert!(dbg.contains("0.3"));
}

#[test]
fn ffb_effect_clone() {
    let effect = FfbEffect::ConstantForce { direction_deg: 90.0, magnitude: 0.5 };
    let cloned = effect.clone();
    assert_eq!(effect, cloned);
}

#[test]
fn ffb_effect_equality() {
    let a = FfbEffect::Sine { frequency_hz: 50.0, magnitude: 0.5 };
    let b = FfbEffect::Sine { frequency_hz: 50.0, magnitude: 0.5 };
    let c = FfbEffect::Sine { frequency_hz: 60.0, magnitude: 0.5 };
    assert_eq!(a, b);
    assert_ne!(a, c);
}

// ── Unused bytes are zeroed ─────────────────────────────────────────────

#[test]
fn constant_force_unused_bytes_zeroed() {
    let b = serialize_effect(FfbEffect::ConstantForce {
        direction_deg: 45.0,
        magnitude: 0.5,
    });
    // Bytes 5, 6, 7 are unused for constant force
    assert_eq!(b[5], 0);
    assert_eq!(b[6], 0);
    assert_eq!(b[7], 0);
}

#[test]
fn spring_unused_bytes_zeroed() {
    let b = serialize_effect(FfbEffect::Spring { coefficient: 0.5 });
    // Bytes 4, 5, 6, 7 are unused for spring
    assert_eq!(b[4], 0);
    assert_eq!(b[5], 0);
    assert_eq!(b[6], 0);
    assert_eq!(b[7], 0);
}

#[test]
fn sine_unused_bytes_zeroed() {
    let b = serialize_effect(FfbEffect::Sine {
        frequency_hz: 50.0,
        magnitude: 0.5,
    });
    // Bytes 5, 6, 7 are unused for sine
    assert_eq!(b[5], 0);
    assert_eq!(b[6], 0);
    assert_eq!(b[7], 0);
}
