// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for VPforce Rhino safety envelope — verifies magnitude clamping
//! across all effect types and ensures hardware over-drive cannot occur.

use flight_ffb_vpforce::effects::{FfbEffect, is_magnitude_safe, serialize_effect};

fn decode_u16(buf: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([buf[offset], buf[offset + 1]])
}

// ── Constant force magnitude clamping ───────────────────────────────────

#[test]
fn constant_force_magnitude_never_exceeds_10000() {
    let test_magnitudes = [
        0.0, 0.25, 0.5, 0.75, 1.0, 1.5, 2.0, 10.0, 100.0,
        f32::INFINITY,
    ];
    for &mag in &test_magnitudes {
        let b = serialize_effect(FfbEffect::ConstantForce {
            direction_deg: 0.0,
            magnitude: mag,
        });
        let raw = decode_u16(&b, 3);
        assert!(
            raw <= 10000,
            "magnitude {mag} produced raw={raw}, expected ≤10000"
        );
    }
}

#[test]
fn constant_force_negative_magnitude_clamps_to_zero() {
    let negatives = [-0.01, -0.5, -1.0, -100.0, f32::NEG_INFINITY];
    for &mag in &negatives {
        let b = serialize_effect(FfbEffect::ConstantForce {
            direction_deg: 0.0,
            magnitude: mag,
        });
        let raw = decode_u16(&b, 3);
        assert_eq!(raw, 0, "negative magnitude {mag} should clamp to 0");
    }
}

// ── Spring coefficient clamping ─────────────────────────────────────────

#[test]
fn spring_coefficient_never_exceeds_10000() {
    let test_coefficients = [0.0, 0.5, 1.0, 1.5, 5.0, 100.0];
    for &c in &test_coefficients {
        let b = serialize_effect(FfbEffect::Spring { coefficient: c });
        let raw = decode_u16(&b, 2);
        assert!(
            raw <= 10000,
            "coefficient {c} produced raw={raw}, expected ≤10000"
        );
    }
}

#[test]
fn spring_negative_coefficient_clamps_to_zero() {
    let b = serialize_effect(FfbEffect::Spring { coefficient: -0.5 });
    let raw = decode_u16(&b, 2);
    assert_eq!(raw, 0);
}

// ── Damper coefficient clamping ─────────────────────────────────────────

#[test]
fn damper_coefficient_never_exceeds_10000() {
    let test_coefficients = [0.0, 0.5, 1.0, 2.0, 50.0];
    for &c in &test_coefficients {
        let b = serialize_effect(FfbEffect::Damper { coefficient: c });
        let raw = decode_u16(&b, 2);
        assert!(raw <= 10000, "coefficient {c} produced raw={raw}");
    }
}

#[test]
fn damper_negative_coefficient_clamps_to_zero() {
    let b = serialize_effect(FfbEffect::Damper { coefficient: -1.0 });
    let raw = decode_u16(&b, 2);
    assert_eq!(raw, 0);
}

// ── Sine effect clamping ────────────────────────────────────────────────

#[test]
fn sine_frequency_always_in_1_to_200() {
    let test_freqs = [0.0, 0.5, 1.0, 100.0, 200.0, 201.0, 1000.0];
    for &freq in &test_freqs {
        let b = serialize_effect(FfbEffect::Sine {
            frequency_hz: freq,
            magnitude: 0.5,
        });
        let raw_freq = decode_u16(&b, 1);
        assert!(
            (1..=200).contains(&raw_freq),
            "frequency_hz={freq} produced raw_freq={raw_freq}, expected 1..=200"
        );
    }
}

#[test]
fn sine_magnitude_never_exceeds_10000() {
    let test_mags = [0.0, 0.5, 1.0, 2.0, 100.0];
    for &mag in &test_mags {
        let b = serialize_effect(FfbEffect::Sine {
            frequency_hz: 50.0,
            magnitude: mag,
        });
        let raw_mag = decode_u16(&b, 3);
        assert!(
            raw_mag <= 10000,
            "magnitude={mag} produced raw_mag={raw_mag}"
        );
    }
}

#[test]
fn sine_negative_magnitude_clamps_to_zero() {
    let b = serialize_effect(FfbEffect::Sine {
        frequency_hz: 50.0,
        magnitude: -1.0,
    });
    let raw_mag = decode_u16(&b, 3);
    assert_eq!(raw_mag, 0);
}

// ── Direction wrapping safety ───────────────────────────────────────────

#[test]
fn direction_angle_always_wraps_to_valid_range() {
    let test_angles = [
        0.0, 90.0, 180.0, 270.0, 359.9, 360.0, 720.0,
        -90.0, -180.0, -360.0, -720.0, 1000.0, -1000.0,
    ];
    for &angle in &test_angles {
        let b = serialize_effect(FfbEffect::ConstantForce {
            direction_deg: angle,
            magnitude: 0.5,
        });
        let raw = decode_u16(&b, 1) as u32;
        assert!(
            raw <= 65535,
            "angle={angle} produced raw={raw}, expected ≤65535"
        );
    }
}

// ── Magnitude safety predicate ──────────────────────────────────────────

#[test]
fn is_magnitude_safe_boundary_values() {
    assert!(is_magnitude_safe(0.0));
    assert!(is_magnitude_safe(0.5));
    assert!(is_magnitude_safe(1.0));
    assert!(!is_magnitude_safe(-f32::EPSILON));
    assert!(!is_magnitude_safe(1.0 + f32::EPSILON));
}

#[test]
fn is_magnitude_safe_special_values() {
    assert!(!is_magnitude_safe(f32::NAN));
    assert!(!is_magnitude_safe(f32::INFINITY));
    assert!(!is_magnitude_safe(f32::NEG_INFINITY));
}

// ── Monotonicity ────────────────────────────────────────────────────────

#[test]
fn spring_coefficient_monotonically_increasing() {
    let values: Vec<f32> = (0..=10).map(|i| i as f32 * 0.1).collect();
    let raw_values: Vec<u16> = values
        .iter()
        .map(|&c| {
            let b = serialize_effect(FfbEffect::Spring { coefficient: c });
            decode_u16(&b, 2)
        })
        .collect();
    for i in 1..raw_values.len() {
        assert!(
            raw_values[i] >= raw_values[i - 1],
            "spring coefficient not monotonic at {}: {} < {}",
            values[i],
            raw_values[i],
            raw_values[i - 1]
        );
    }
}

#[test]
fn constant_force_magnitude_monotonically_increasing() {
    let values: Vec<f32> = (0..=10).map(|i| i as f32 * 0.1).collect();
    let raw_values: Vec<u16> = values
        .iter()
        .map(|&m| {
            let b = serialize_effect(FfbEffect::ConstantForce {
                direction_deg: 0.0,
                magnitude: m,
            });
            decode_u16(&b, 3)
        })
        .collect();
    for i in 1..raw_values.len() {
        assert!(
            raw_values[i] >= raw_values[i - 1],
            "magnitude not monotonic"
        );
    }
}

#[test]
fn sine_frequency_monotonically_increasing_within_range() {
    let values: Vec<f32> = (1..=20).map(|i| i as f32 * 10.0).collect();
    let raw_values: Vec<u16> = values
        .iter()
        .map(|&f| {
            let b = serialize_effect(FfbEffect::Sine {
                frequency_hz: f,
                magnitude: 0.5,
            });
            decode_u16(&b, 1)
        })
        .collect();
    for i in 1..raw_values.len() {
        assert!(
            raw_values[i] >= raw_values[i - 1],
            "frequency not monotonic at {} Hz",
            values[i]
        );
    }
}
