// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2026 Flight Hub Team

//! Depth tests for `FlightData` structure parsing and normalisation.
//!
//! Covers:
//! - Pitch, roll, yaw normalisation across the full domain
//! - Throttle clamping at boundaries
//! - Extreme / degenerate float values (NaN, Inf, subnormal)
//! - Zeroed struct defaults
//! - Field independence (setting one field doesn't affect others)

use approx::assert_relative_eq;
use bytemuck::Zeroable;
use flight_falcon_bms::FlightData;
use std::f32::consts;

// ── Helpers ──────────────────────────────────────────────────────────────────

fn zeroed() -> FlightData {
    FlightData::zeroed()
}

fn with_fields(f: impl FnOnce(&mut FlightData)) -> FlightData {
    let mut fd = zeroed();
    f(&mut fd);
    fd
}

// ── Pitch normalisation ─────────────────────────────────────────────────────

#[test]
fn pitch_zero_normalises_to_zero() {
    let fd = with_fields(|fd| fd.pitch = 0.0);
    assert_relative_eq!(fd.pitch_normalized(), 0.0, epsilon = 1e-6);
}

#[test]
fn pitch_pi_normalises_to_one() {
    let fd = with_fields(|fd| fd.pitch = consts::PI);
    assert_relative_eq!(fd.pitch_normalized(), 1.0, epsilon = 1e-6);
}

#[test]
fn pitch_negative_pi_normalises_to_minus_one() {
    let fd = with_fields(|fd| fd.pitch = -consts::PI);
    assert_relative_eq!(fd.pitch_normalized(), -1.0, epsilon = 1e-6);
}

#[test]
fn pitch_half_pi_normalises_to_half() {
    let fd = with_fields(|fd| fd.pitch = consts::FRAC_PI_2);
    assert_relative_eq!(fd.pitch_normalized(), 0.5, epsilon = 1e-6);
}

#[test]
fn pitch_beyond_pi_clamped_to_one() {
    let fd = with_fields(|fd| fd.pitch = 2.0 * consts::PI);
    assert_relative_eq!(fd.pitch_normalized(), 1.0, epsilon = 1e-6);
}

#[test]
fn pitch_below_neg_pi_clamped_to_minus_one() {
    let fd = with_fields(|fd| fd.pitch = -2.0 * consts::PI);
    assert_relative_eq!(fd.pitch_normalized(), -1.0, epsilon = 1e-6);
}

#[test]
fn pitch_quarter_pi() {
    let fd = with_fields(|fd| fd.pitch = consts::FRAC_PI_4);
    assert_relative_eq!(fd.pitch_normalized(), 0.25, epsilon = 1e-6);
}

// ── Roll normalisation ──────────────────────────────────────────────────────

#[test]
fn roll_zero_normalises_to_zero() {
    let fd = with_fields(|fd| fd.roll = 0.0);
    assert_relative_eq!(fd.roll_normalized(), 0.0, epsilon = 1e-6);
}

#[test]
fn roll_pi_normalises_to_one() {
    let fd = with_fields(|fd| fd.roll = consts::PI);
    assert_relative_eq!(fd.roll_normalized(), 1.0, epsilon = 1e-6);
}

#[test]
fn roll_negative_pi_normalises_to_minus_one() {
    let fd = with_fields(|fd| fd.roll = -consts::PI);
    assert_relative_eq!(fd.roll_normalized(), -1.0, epsilon = 1e-6);
}

#[test]
fn roll_half_pi_normalises_to_half() {
    let fd = with_fields(|fd| fd.roll = consts::FRAC_PI_2);
    assert_relative_eq!(fd.roll_normalized(), 0.5, epsilon = 1e-6);
}

#[test]
fn roll_beyond_pi_clamped() {
    let fd = with_fields(|fd| fd.roll = 5.0);
    assert_relative_eq!(fd.roll_normalized(), 1.0, epsilon = 1e-6);
}

// ── Yaw normalisation (±π/2 → ±1.0) ────────────────────────────────────────

#[test]
fn yaw_zero_normalises_to_zero() {
    let fd = with_fields(|fd| fd.yaw = 0.0);
    assert_relative_eq!(fd.yaw_normalized(), 0.0, epsilon = 1e-6);
}

#[test]
fn yaw_half_pi_normalises_to_one() {
    let fd = with_fields(|fd| fd.yaw = consts::FRAC_PI_2);
    assert_relative_eq!(fd.yaw_normalized(), 1.0, epsilon = 1e-6);
}

#[test]
fn yaw_neg_half_pi_normalises_to_minus_one() {
    let fd = with_fields(|fd| fd.yaw = -consts::FRAC_PI_2);
    assert_relative_eq!(fd.yaw_normalized(), -1.0, epsilon = 1e-6);
}

#[test]
fn yaw_quarter_pi_normalises_to_half() {
    let fd = with_fields(|fd| fd.yaw = consts::FRAC_PI_4);
    assert_relative_eq!(fd.yaw_normalized(), 0.5, epsilon = 1e-6);
}

#[test]
fn yaw_beyond_half_pi_clamped() {
    let fd = with_fields(|fd| fd.yaw = consts::PI);
    assert_relative_eq!(fd.yaw_normalized(), 1.0, epsilon = 1e-6);
}

#[test]
fn yaw_below_neg_half_pi_clamped() {
    let fd = with_fields(|fd| fd.yaw = -consts::PI);
    assert_relative_eq!(fd.yaw_normalized(), -1.0, epsilon = 1e-6);
}

// ── Throttle normalisation ──────────────────────────────────────────────────

#[test]
fn throttle_zero_normalises_to_zero() {
    let fd = with_fields(|fd| fd.throttle = 0.0);
    assert_relative_eq!(fd.throttle_normalized(), 0.0, epsilon = 1e-6);
}

#[test]
fn throttle_one_normalises_to_one() {
    let fd = with_fields(|fd| fd.throttle = 1.0);
    assert_relative_eq!(fd.throttle_normalized(), 1.0, epsilon = 1e-6);
}

#[test]
fn throttle_midpoint() {
    let fd = with_fields(|fd| fd.throttle = 0.5);
    assert_relative_eq!(fd.throttle_normalized(), 0.5, epsilon = 1e-6);
}

#[test]
fn throttle_above_one_clamped() {
    let fd = with_fields(|fd| fd.throttle = 1.5);
    assert_relative_eq!(fd.throttle_normalized(), 1.0, epsilon = 1e-6);
}

#[test]
fn throttle_below_zero_clamped() {
    let fd = with_fields(|fd| fd.throttle = -0.3);
    assert_relative_eq!(fd.throttle_normalized(), 0.0, epsilon = 1e-6);
}

#[test]
fn throttle_large_positive_clamped() {
    let fd = with_fields(|fd| fd.throttle = 100.0);
    assert_relative_eq!(fd.throttle_normalized(), 1.0, epsilon = 1e-6);
}

#[test]
fn throttle_large_negative_clamped() {
    let fd = with_fields(|fd| fd.throttle = -100.0);
    assert_relative_eq!(fd.throttle_normalized(), 0.0, epsilon = 1e-6);
}

// ── Extreme float values ────────────────────────────────────────────────────

#[test]
fn pitch_nan_clamps_to_valid_or_nan() {
    let fd = with_fields(|fd| fd.pitch = f32::NAN);
    // NaN / PI = NaN, clamp(NaN) → NaN per IEEE 754
    // Rust's f32::clamp returns NaN for NaN input
    let result = fd.pitch_normalized();
    assert!(result.is_nan());
}

#[test]
fn pitch_inf_clamped_to_one() {
    let fd = with_fields(|fd| fd.pitch = f32::INFINITY);
    assert_relative_eq!(fd.pitch_normalized(), 1.0, epsilon = 1e-6);
}

#[test]
fn pitch_neg_inf_clamped_to_minus_one() {
    let fd = with_fields(|fd| fd.pitch = f32::NEG_INFINITY);
    assert_relative_eq!(fd.pitch_normalized(), -1.0, epsilon = 1e-6);
}

#[test]
fn roll_nan_returns_nan() {
    let fd = with_fields(|fd| fd.roll = f32::NAN);
    assert!(fd.roll_normalized().is_nan());
}

#[test]
fn yaw_inf_clamped() {
    let fd = with_fields(|fd| fd.yaw = f32::INFINITY);
    assert_relative_eq!(fd.yaw_normalized(), 1.0, epsilon = 1e-6);
}

#[test]
fn throttle_nan_returns_nan() {
    let fd = with_fields(|fd| fd.throttle = f32::NAN);
    assert!(fd.throttle_normalized().is_nan());
}

#[test]
fn pitch_subnormal_normalises_near_zero() {
    let fd = with_fields(|fd| fd.pitch = f32::MIN_POSITIVE);
    assert_relative_eq!(fd.pitch_normalized(), 0.0, epsilon = 1e-6);
}

// ── Zeroed struct ───────────────────────────────────────────────────────────

#[test]
fn zeroed_struct_all_fields_zero() {
    let fd = zeroed();
    assert_eq!(fd.x, 0.0);
    assert_eq!(fd.y, 0.0);
    assert_eq!(fd.z, 0.0);
    assert_eq!(fd.x_dot, 0.0);
    assert_eq!(fd.y_dot, 0.0);
    assert_eq!(fd.z_dot, 0.0);
    assert_eq!(fd.alpha, 0.0);
    assert_eq!(fd.beta, 0.0);
    assert_eq!(fd.gamma, 0.0);
    assert_eq!(fd.pitch, 0.0);
    assert_eq!(fd.roll, 0.0);
    assert_eq!(fd.yaw, 0.0);
    assert_eq!(fd.mach, 0.0);
    assert_eq!(fd.cas, 0.0);
    assert_eq!(fd.alt, 0.0);
    assert_eq!(fd.throttle, 0.0);
    assert_eq!(fd.rpm, 0.0);
}

#[test]
fn zeroed_struct_normalisations_all_zero() {
    let fd = zeroed();
    assert_relative_eq!(fd.pitch_normalized(), 0.0, epsilon = 1e-6);
    assert_relative_eq!(fd.roll_normalized(), 0.0, epsilon = 1e-6);
    assert_relative_eq!(fd.yaw_normalized(), 0.0, epsilon = 1e-6);
    assert_relative_eq!(fd.throttle_normalized(), 0.0, epsilon = 1e-6);
}

// ── Field independence ──────────────────────────────────────────────────────

#[test]
fn setting_pitch_does_not_affect_roll() {
    let fd = with_fields(|fd| fd.pitch = consts::PI);
    assert_relative_eq!(fd.roll_normalized(), 0.0, epsilon = 1e-6);
}

#[test]
fn setting_roll_does_not_affect_yaw() {
    let fd = with_fields(|fd| fd.roll = consts::PI);
    assert_relative_eq!(fd.yaw_normalized(), 0.0, epsilon = 1e-6);
}

#[test]
fn setting_yaw_does_not_affect_throttle() {
    let fd = with_fields(|fd| fd.yaw = consts::FRAC_PI_2);
    assert_relative_eq!(fd.throttle_normalized(), 0.0, epsilon = 1e-6);
}

#[test]
fn setting_throttle_does_not_affect_pitch() {
    let fd = with_fields(|fd| fd.throttle = 0.75);
    assert_relative_eq!(fd.pitch_normalized(), 0.0, epsilon = 1e-6);
}

#[test]
fn multiple_fields_set_independently() {
    let fd = with_fields(|fd| {
        fd.pitch = consts::FRAC_PI_2;
        fd.roll = -consts::FRAC_PI_2;
        fd.yaw = consts::FRAC_PI_4;
        fd.throttle = 0.75;
    });
    assert_relative_eq!(fd.pitch_normalized(), 0.5, epsilon = 1e-6);
    assert_relative_eq!(fd.roll_normalized(), -0.5, epsilon = 1e-6);
    assert_relative_eq!(fd.yaw_normalized(), 0.5, epsilon = 1e-6);
    assert_relative_eq!(fd.throttle_normalized(), 0.75, epsilon = 1e-6);
}
