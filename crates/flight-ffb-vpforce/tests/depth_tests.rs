// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for the `flight-ffb-vpforce` crate.
//!
//! Covers FFB effect serialisation, input report parsing, health monitoring,
//! preset configuration, and cross-module integration for the VPforce Rhino.

use flight_ffb_vpforce::effects::{
    FFB_REPORT_LEN, FfbEffect, REPORT_CONSTANT_FORCE, REPORT_SPRING, is_magnitude_safe,
    serialize_effect,
};
use flight_ffb_vpforce::health::RhinoHealthMonitor;
use flight_ffb_vpforce::input::{
    RHINO_PID_V2, RHINO_PID_V3, RHINO_REPORT_LEN, RhinoParseError, VPFORCE_VENDOR_ID,
    parse_report,
};
use flight_ffb_vpforce::presets::recommended_axis_config;
use flight_ffb_vpforce::RHINO_PIDS;

// ──────────────────────────────────────────────────────────────────────────────
// Helpers
// ──────────────────────────────────────────────────────────────────────────────

fn centred_report() -> [u8; RHINO_REPORT_LEN] {
    let mut r = [0u8; RHINO_REPORT_LEN];
    r[0] = 0x01;
    r
}

fn report_with_axes(roll: i16, pitch: i16, throttle: i16, rocker: i16, twist: i16) -> [u8; RHINO_REPORT_LEN] {
    let mut r = centred_report();
    r[1..3].copy_from_slice(&roll.to_le_bytes());
    r[3..5].copy_from_slice(&pitch.to_le_bytes());
    r[5..7].copy_from_slice(&throttle.to_le_bytes());
    r[7..9].copy_from_slice(&rocker.to_le_bytes());
    r[11..13].copy_from_slice(&twist.to_le_bytes());
    r
}

fn report_with_buttons(mask: u32, hat: u8) -> [u8; RHINO_REPORT_LEN] {
    let mut r = centred_report();
    r[13..17].copy_from_slice(&mask.to_le_bytes());
    r[17] = hat;
    r
}

// ──────────────────────────────────────────────────────────────────────────────
// §1 — FFB Effect Serialisation: Constant Force
// ──────────────────────────────────────────────────────────────────────────────

/// Constant force report ID is 0x10.
#[test]
fn constant_force_uses_correct_report_id() {
    let b = serialize_effect(FfbEffect::ConstantForce {
        direction_deg: 0.0,
        magnitude: 0.5,
    });
    assert_eq!(b[0], REPORT_CONSTANT_FORCE);
}

/// Direction 0° encodes angle_raw = 0.
#[test]
fn constant_force_zero_degrees_encodes_zero_angle() {
    let b = serialize_effect(FfbEffect::ConstantForce {
        direction_deg: 0.0,
        magnitude: 0.5,
    });
    let angle = u16::from_le_bytes([b[1], b[2]]);
    assert_eq!(angle, 0);
}

/// Direction 180° encodes approximately half of u16 range.
#[test]
fn constant_force_180_degrees_encodes_half_range() {
    let b = serialize_effect(FfbEffect::ConstantForce {
        direction_deg: 180.0,
        magnitude: 0.5,
    });
    let angle = u16::from_le_bytes([b[1], b[2]]);
    // 180/360 * 65535 = 32767.5, truncated to 32767
    assert!((angle as i32 - 32767).abs() <= 1, "angle={angle}");
}

/// Negative direction wraps into [0, 360).
#[test]
fn constant_force_negative_direction_wraps() {
    let b = serialize_effect(FfbEffect::ConstantForce {
        direction_deg: -90.0,
        magnitude: 0.3,
    });
    let angle = u16::from_le_bytes([b[1], b[2]]);
    // -90 → 270, 270/360 * 65535 ≈ 49151
    let expected = (270.0 / 360.0 * 65535.0) as u16;
    assert_eq!(angle, expected);
}

/// Direction >360° wraps correctly.
#[test]
fn constant_force_direction_above_360_wraps() {
    let b = serialize_effect(FfbEffect::ConstantForce {
        direction_deg: 450.0,
        magnitude: 0.1,
    });
    let angle = u16::from_le_bytes([b[1], b[2]]);
    // 450 % 360 = 90
    let expected = (90.0 / 360.0 * 65535.0) as u16;
    assert_eq!(angle, expected);
}

/// Magnitude clamped below 0.0 yields raw 0.
#[test]
fn constant_force_negative_magnitude_clamped_to_zero() {
    let b = serialize_effect(FfbEffect::ConstantForce {
        direction_deg: 0.0,
        magnitude: -5.0,
    });
    let mag = u16::from_le_bytes([b[3], b[4]]);
    assert_eq!(mag, 0);
}

/// Exact quarter-magnitude encodes correctly (0.25 → 2500).
#[test]
fn constant_force_quarter_magnitude() {
    let b = serialize_effect(FfbEffect::ConstantForce {
        direction_deg: 0.0,
        magnitude: 0.25,
    });
    let mag = u16::from_le_bytes([b[3], b[4]]);
    assert_eq!(mag, 2500);
}

/// Trailing bytes of a constant-force report are zero.
#[test]
fn constant_force_trailing_bytes_zero() {
    let b = serialize_effect(FfbEffect::ConstantForce {
        direction_deg: 45.0,
        magnitude: 0.8,
    });
    assert_eq!(b[5], 0);
    assert_eq!(b[6], 0);
    assert_eq!(b[7], 0);
}

// ──────────────────────────────────────────────────────────────────────────────
// §2 — FFB Effect Serialisation: Spring & Damper
// ──────────────────────────────────────────────────────────────────────────────

/// Spring with coefficient 0.0 encodes raw 0.
#[test]
fn spring_zero_coefficient() {
    let b = serialize_effect(FfbEffect::Spring { coefficient: 0.0 });
    let raw = u16::from_le_bytes([b[2], b[3]]);
    assert_eq!(raw, 0);
}

/// Spring with coefficient 1.0 encodes raw 10000.
#[test]
fn spring_max_coefficient() {
    let b = serialize_effect(FfbEffect::Spring { coefficient: 1.0 });
    let raw = u16::from_le_bytes([b[2], b[3]]);
    assert_eq!(raw, 10000);
}

/// Spring coefficient >1.0 is clamped to 1.0 (raw 10000).
#[test]
fn spring_over_one_clamped() {
    let b = serialize_effect(FfbEffect::Spring { coefficient: 3.5 });
    let raw = u16::from_le_bytes([b[2], b[3]]);
    assert_eq!(raw, 10000);
}

/// Damper coefficient <0.0 is clamped to 0.0.
#[test]
fn damper_negative_coefficient_clamped() {
    let b = serialize_effect(FfbEffect::Damper {
        coefficient: -1.0,
    });
    let raw = u16::from_le_bytes([b[2], b[3]]);
    assert_eq!(raw, 0);
}

/// Spring and Damper share report ID but differ on mode byte.
#[test]
fn spring_and_damper_share_report_id_differ_on_mode() {
    let spring = serialize_effect(FfbEffect::Spring { coefficient: 0.7 });
    let damper = serialize_effect(FfbEffect::Damper { coefficient: 0.7 });
    assert_eq!(spring[0], damper[0]);
    assert_ne!(spring[1], damper[1]);
}

/// Spring trailing bytes (4..8) are zero.
#[test]
fn spring_trailing_bytes_zero() {
    let b = serialize_effect(FfbEffect::Spring { coefficient: 0.5 });
    for (i, &byte) in b.iter().enumerate().skip(4) {
        assert_eq!(byte, 0, "byte {i} non-zero");
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// §3 — FFB Effect Serialisation: Sine / Periodic
// ──────────────────────────────────────────────────────────────────────────────

/// Sine at exactly 1 Hz encodes freq raw = 1.
#[test]
fn sine_minimum_frequency_boundary() {
    let b = serialize_effect(FfbEffect::Sine {
        frequency_hz: 1.0,
        magnitude: 1.0,
    });
    let freq = u16::from_le_bytes([b[1], b[2]]);
    assert_eq!(freq, 1);
}

/// Sine at exactly 200 Hz encodes freq raw = 200.
#[test]
fn sine_maximum_frequency_boundary() {
    let b = serialize_effect(FfbEffect::Sine {
        frequency_hz: 200.0,
        magnitude: 0.5,
    });
    let freq = u16::from_le_bytes([b[1], b[2]]);
    assert_eq!(freq, 200);
}

/// Sine below 1 Hz is clamped to 1 Hz.
#[test]
fn sine_below_minimum_frequency_clamped() {
    let b = serialize_effect(FfbEffect::Sine {
        frequency_hz: 0.1,
        magnitude: 0.5,
    });
    let freq = u16::from_le_bytes([b[1], b[2]]);
    assert_eq!(freq, 1);
}

/// Sine magnitude 0.0 encodes raw 0.
#[test]
fn sine_zero_magnitude() {
    let b = serialize_effect(FfbEffect::Sine {
        frequency_hz: 50.0,
        magnitude: 0.0,
    });
    let mag = u16::from_le_bytes([b[3], b[4]]);
    assert_eq!(mag, 0);
}

/// Sine trailing bytes (5..8) are zero.
#[test]
fn sine_trailing_bytes_zero() {
    let b = serialize_effect(FfbEffect::Sine {
        frequency_hz: 100.0,
        magnitude: 1.0,
    });
    for (i, &byte) in b.iter().enumerate().skip(5) {
        assert_eq!(byte, 0, "byte {i} non-zero");
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// §4 — FFB Effect Serialisation: StopAll
// ──────────────────────────────────────────────────────────────────────────────

/// StopAll payload is all-zero except report ID.
#[test]
fn stop_all_payload_is_zero_padded() {
    let b = serialize_effect(FfbEffect::StopAll);
    assert_eq!(b[0], 0xFF);
    for (i, &byte) in b.iter().enumerate().skip(1) {
        assert_eq!(byte, 0, "StopAll byte {i} should be 0");
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// §5 — FFB Report Size Constraint
// ──────────────────────────────────────────────────────────────────────────────

/// Every effect variant produces exactly FFB_REPORT_LEN bytes.
#[test]
fn all_effects_produce_correct_length_reports() {
    let effects = [
        FfbEffect::ConstantForce { direction_deg: 0.0, magnitude: 1.0 },
        FfbEffect::Spring { coefficient: 0.5 },
        FfbEffect::Damper { coefficient: 0.5 },
        FfbEffect::Sine { frequency_hz: 50.0, magnitude: 0.5 },
        FfbEffect::StopAll,
    ];
    for effect in &effects {
        let b = serialize_effect(*effect);
        assert_eq!(b.len(), FFB_REPORT_LEN, "effect {effect:?}");
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// §6 — Magnitude Safety
// ──────────────────────────────────────────────────────────────────────────────

/// Boundary values for is_magnitude_safe.
#[test]
fn magnitude_safe_boundary_values() {
    assert!(is_magnitude_safe(0.0));
    assert!(is_magnitude_safe(1.0));
    assert!(is_magnitude_safe(0.5));
    assert!(!is_magnitude_safe(-f32::EPSILON));
    assert!(!is_magnitude_safe(1.0 + f32::EPSILON));
}

/// NaN and infinity are not safe magnitudes.
#[test]
fn magnitude_safe_rejects_nan_and_infinity() {
    assert!(!is_magnitude_safe(f32::NAN));
    assert!(!is_magnitude_safe(f32::INFINITY));
    assert!(!is_magnitude_safe(f32::NEG_INFINITY));
}

// ──────────────────────────────────────────────────────────────────────────────
// §7 — Input Report Parsing: Axis Linearity
// ──────────────────────────────────────────────────────────────────────────────

/// Quarter-deflection on roll axis is approximately 0.25.
#[test]
fn axis_quarter_deflection_linearity() {
    let quarter = (32767.0 * 0.25) as i16; // ~8191
    let r = report_with_axes(quarter, 0, 0, 0, 0);
    let s = parse_report(&r).unwrap();
    assert!((s.axes.roll - 0.25).abs() < 0.01, "roll={}", s.axes.roll);
}

/// Symmetry: positive and negative deflections have equal magnitude.
#[test]
fn axis_symmetry_positive_vs_negative() {
    let val = 16000i16;
    let r_pos = report_with_axes(val, 0, 0, 0, 0);
    let r_neg = report_with_axes(-val, 0, 0, 0, 0);
    let pos = parse_report(&r_pos).unwrap().axes.roll;
    let neg = parse_report(&r_neg).unwrap().axes.roll;
    assert!((pos + neg).abs() < 1e-3, "pos={pos}, neg={neg}");
}

/// All five axes are independent: setting one does not affect others.
#[test]
fn axes_are_independent() {
    let r = report_with_axes(i16::MAX, 0, 0, 0, 0);
    let s = parse_report(&r).unwrap();
    assert!((s.axes.roll - 1.0).abs() < 1e-4);
    assert!(s.axes.pitch.abs() < 1e-4);
    assert!(s.axes.twist.abs() < 1e-4);
    assert!(s.axes.rocker.abs() < 1e-4);
}

/// Twist axis full positive deflection.
#[test]
fn twist_full_positive() {
    let r = report_with_axes(0, 0, 0, 0, i16::MAX);
    let s = parse_report(&r).unwrap();
    assert!((s.axes.twist - 1.0).abs() < 1e-4, "twist={}", s.axes.twist);
}

/// Rocker axis full negative deflection.
#[test]
fn rocker_full_negative() {
    let r = report_with_axes(0, 0, 0, i16::MIN, 0);
    let s = parse_report(&r).unwrap();
    assert!((s.axes.rocker + 1.0).abs() < 1e-3, "rocker={}", s.axes.rocker);
}

// ──────────────────────────────────────────────────────────────────────────────
// §8 — Input Report Parsing: Buttons & Hat
// ──────────────────────────────────────────────────────────────────────────────

/// All 32 buttons on → is_pressed returns true for all.
#[test]
fn all_buttons_on_pressed() {
    let r = report_with_buttons(u32::MAX, 0xFF);
    let s = parse_report(&r).unwrap();
    for n in 1u8..=32 {
        assert!(s.buttons.is_pressed(n), "button {n} should be pressed");
    }
}

/// No buttons pressed → is_pressed returns false for all.
#[test]
fn no_buttons_pressed() {
    let r = report_with_buttons(0, 0xFF);
    let s = parse_report(&r).unwrap();
    for n in 1u8..=32 {
        assert!(!s.buttons.is_pressed(n), "button {n} should not be pressed");
    }
}

/// Button 0 (out-of-range low) always returns false.
#[test]
fn button_zero_always_false() {
    let r = report_with_buttons(u32::MAX, 0);
    let s = parse_report(&r).unwrap();
    assert!(!s.buttons.is_pressed(0));
}

/// Button 33 (out-of-range high) always returns false.
#[test]
fn button_33_always_false() {
    let r = report_with_buttons(u32::MAX, 0);
    let s = parse_report(&r).unwrap();
    assert!(!s.buttons.is_pressed(33));
}

/// Single isolated button (bit 15 → button 16).
#[test]
fn single_isolated_button() {
    let r = report_with_buttons(1u32 << 15, 0xFF);
    let s = parse_report(&r).unwrap();
    assert!(s.buttons.is_pressed(16));
    assert!(!s.buttons.is_pressed(15));
    assert!(!s.buttons.is_pressed(17));
}

/// All 8 cardinal hat directions are preserved.
#[test]
fn hat_all_cardinal_directions() {
    for hat in 0u8..=7 {
        let r = report_with_buttons(0, hat);
        let s = parse_report(&r).unwrap();
        assert_eq!(s.buttons.hat, hat, "hat direction {hat}");
    }
}

/// Hat centred value 0xFF is preserved.
#[test]
fn hat_centred_value() {
    let r = report_with_buttons(0, 0xFF);
    let s = parse_report(&r).unwrap();
    assert_eq!(s.buttons.hat, 0xFF);
}

// ──────────────────────────────────────────────────────────────────────────────
// §9 — Input Report Parsing: Error Paths
// ──────────────────────────────────────────────────────────────────────────────

/// Every length from 0 to RHINO_REPORT_LEN-1 returns TooShort.
#[test]
fn every_short_length_returns_error() {
    for len in 0..RHINO_REPORT_LEN {
        let data = vec![0x01; len];
        assert!(
            matches!(parse_report(&data), Err(RhinoParseError::TooShort { .. })),
            "len={len} should fail"
        );
    }
}

/// Report IDs 0x00 and 0x02–0xFF all return UnknownReportId.
#[test]
fn all_invalid_report_ids_rejected() {
    for id in (0u8..=0xFF).filter(|&x| x != 0x01) {
        let mut r = [0u8; RHINO_REPORT_LEN];
        r[0] = id;
        assert!(
            matches!(parse_report(&r), Err(RhinoParseError::UnknownReportId { .. })),
            "id=0x{id:02X} should fail"
        );
    }
}

/// Oversized reports (> RHINO_REPORT_LEN) parse successfully using the first 20 bytes.
#[test]
fn oversized_report_parses_first_20_bytes() {
    let mut data = vec![0u8; 64];
    data[0] = 0x01;
    data[1..3].copy_from_slice(&i16::MAX.to_le_bytes()); // roll
    let s = parse_report(&data).unwrap();
    assert!((s.axes.roll - 1.0).abs() < 1e-4);
}

// ──────────────────────────────────────────────────────────────────────────────
// §10 — Health Monitor: Lifecycle
// ──────────────────────────────────────────────────────────────────────────────

/// New monitor starts connected and online.
#[test]
fn health_new_is_connected() {
    let m = RhinoHealthMonitor::new();
    assert!(!m.is_offline());
    let s = m.status();
    assert!(s.connected);
    assert_eq!(s.consecutive_failures, 0);
    assert!(s.ghost_rate < 1e-6);
}

/// Default impl matches new().
#[test]
fn health_default_matches_new() {
    let d = RhinoHealthMonitor::default();
    let n = RhinoHealthMonitor::new();
    assert_eq!(d.is_offline(), n.is_offline());
    assert_eq!(d.status().consecutive_failures, n.status().consecutive_failures);
}

/// Exactly threshold failures → offline.
#[test]
fn health_exact_threshold_triggers_offline() {
    let m = RhinoHealthMonitor::new();
    let threshold = RhinoHealthMonitor::DEFAULT_FAILURE_THRESHOLD;
    let mut m = m;
    for _ in 0..threshold {
        m.record_failure();
    }
    assert!(m.is_offline());
}

/// One fewer than threshold → still online.
#[test]
fn health_one_below_threshold_still_online() {
    let threshold = RhinoHealthMonitor::DEFAULT_FAILURE_THRESHOLD;
    let mut m = RhinoHealthMonitor::new();
    for _ in 0..(threshold - 1) {
        m.record_failure();
    }
    assert!(!m.is_offline());
}

/// Success after failures resets count.
#[test]
fn health_success_resets_consecutive_failures() {
    let mut m = RhinoHealthMonitor::new();
    m.record_failure();
    m.record_failure();
    m.record_success(false);
    assert_eq!(m.status().consecutive_failures, 0);
    assert!(!m.is_offline());
}

/// Custom failure threshold via builder.
#[test]
fn health_custom_threshold() {
    let mut m = RhinoHealthMonitor::new().with_failure_threshold(5);
    for _ in 0..4 {
        m.record_failure();
    }
    assert!(!m.is_offline());
    m.record_failure();
    assert!(m.is_offline());
}

/// Ghost rate with zero reports is 0.0.
#[test]
fn health_ghost_rate_zero_reports() {
    let m = RhinoHealthMonitor::new();
    assert!(m.status().ghost_rate < 1e-6);
}

/// 100% ghost rate → unhealthy.
#[test]
fn health_full_ghost_rate_unhealthy() {
    let mut m = RhinoHealthMonitor::new();
    for _ in 0..10 {
        m.record_success(true);
    }
    let s = m.status();
    assert!((s.ghost_rate - 1.0).abs() < 1e-6);
    assert!(!s.is_healthy());
}

/// Mixed ghost/normal reports compute correct ghost rate.
#[test]
fn health_mixed_ghost_rate() {
    let mut m = RhinoHealthMonitor::new();
    // 3 ghost out of 10 total successes
    for i in 0..10 {
        m.record_success(i < 3);
    }
    let s = m.status();
    assert!((s.ghost_rate - 0.3).abs() < 1e-6, "ghost_rate={}", s.ghost_rate);
}

/// Failures count toward total_reports for ghost rate denominator.
#[test]
fn health_failures_in_ghost_rate_denominator() {
    let mut m = RhinoHealthMonitor::new();
    m.record_success(true); // 1 ghost
    m.record_failure();     // 1 failure
    // total_reports = 2, ghost_reports = 1, rate = 0.5
    let s = m.status();
    assert!((s.ghost_rate - 0.5).abs() < 1e-6);
}

/// time_since_last_success is None when no successes recorded.
#[test]
fn health_no_success_time_is_none() {
    let m = RhinoHealthMonitor::new();
    assert!(m.time_since_last_success().is_none());
}

/// time_since_last_success returns Some after a success.
#[test]
fn health_has_time_after_success() {
    let mut m = RhinoHealthMonitor::new();
    m.record_success(false);
    assert!(m.time_since_last_success().is_some());
}

/// is_healthy requires connected AND low failures AND low ghost rate.
#[test]
fn health_status_is_healthy_all_conditions() {
    let mut m = RhinoHealthMonitor::new();
    m.record_success(false);
    let s = m.status();
    assert!(s.is_healthy());

    // High ghost rate → not healthy
    let mut m2 = RhinoHealthMonitor::new();
    for _ in 0..20 {
        m2.record_success(true);
    }
    assert!(!m2.status().is_healthy());

    // Disconnected → not healthy
    let mut m3 = RhinoHealthMonitor::new();
    for _ in 0..3 {
        m3.record_failure();
    }
    assert!(!m3.status().is_healthy());
}

// ──────────────────────────────────────────────────────────────────────────────
// §11 — Preset Configuration
// ──────────────────────────────────────────────────────────────────────────────

/// Presets return exactly 5 axis configs.
#[test]
fn presets_count_is_five() {
    assert_eq!(recommended_axis_config().len(), 5);
}

/// Preset names are unique.
#[test]
fn preset_names_unique() {
    let cfg = recommended_axis_config();
    let mut names: Vec<&str> = cfg.iter().map(|c| c.name).collect();
    names.sort();
    names.dedup();
    assert_eq!(names.len(), 5, "preset names must be unique");
}

/// All deadzones are positive and below 0.1.
#[test]
fn preset_deadzones_in_sane_range() {
    for c in &recommended_axis_config() {
        assert!(c.deadzone > 0.0, "{}: deadzone must be positive", c.name);
        assert!(c.deadzone < 0.1, "{}: deadzone too large ({})", c.name, c.deadzone);
    }
}

/// Hall-effect axes (roll, pitch) have smaller deadzones than resistive axes.
#[test]
fn preset_hall_effect_smaller_deadzone_than_resistive() {
    let cfg = recommended_axis_config();
    let roll_dz = cfg.iter().find(|c| c.name == "roll").unwrap().deadzone;
    let throttle_dz = cfg.iter().find(|c| c.name == "throttle").unwrap().deadzone;
    assert!(roll_dz < throttle_dz, "Hall-effect roll ({roll_dz}) should have smaller deadzone than resistive throttle ({throttle_dz})");
}

/// Filter alpha values are between 0.0 and 1.0 when present.
#[test]
fn preset_filter_alpha_in_range() {
    for c in &recommended_axis_config() {
        if let Some(alpha) = c.filter_alpha {
            assert!(alpha > 0.0 && alpha < 1.0, "{}: alpha out of range ({})", c.name, alpha);
        }
    }
}

/// Slew rate, when present, is positive.
#[test]
fn preset_slew_rate_positive() {
    for c in &recommended_axis_config() {
        if let Some(sr) = c.slew_rate {
            assert!(sr > 0.0, "{}: slew_rate must be positive ({})", c.name, sr);
        }
    }
}

/// Hall-effect axes don't need filtering.
#[test]
fn preset_hall_effect_no_filter() {
    let cfg = recommended_axis_config();
    for name in &["roll", "pitch"] {
        let c = cfg.iter().find(|c| c.name == *name).unwrap();
        assert!(c.filter_alpha.is_none(), "{name} should not have filter_alpha");
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// §12 — Device Identity Constants
// ──────────────────────────────────────────────────────────────────────────────

/// VID matches STMicroelectronics.
#[test]
fn vid_is_stmicro() {
    assert_eq!(VPFORCE_VENDOR_ID, 0x0483);
}

/// PID v2 and v3 are distinct.
#[test]
fn pid_v2_and_v3_distinct() {
    assert_ne!(RHINO_PID_V2, RHINO_PID_V3);
}

/// RHINO_PIDS contains exactly 2 entries.
#[test]
fn rhino_pids_contains_both_revisions() {
    assert_eq!(RHINO_PIDS.len(), 2);
    assert!(RHINO_PIDS.contains(&RHINO_PID_V2));
    assert!(RHINO_PIDS.contains(&RHINO_PID_V3));
}

// ──────────────────────────────────────────────────────────────────────────────
// §13 — Cross-Module Integration
// ──────────────────────────────────────────────────────────────────────────────

/// Parse a report, assert health, then generate a force-feedback response.
#[test]
fn end_to_end_parse_health_ffb() {
    let mut monitor = RhinoHealthMonitor::new();

    // Simulate receiving a valid centred report
    let r = centred_report();
    let state = parse_report(&r).unwrap();
    monitor.record_success(false);

    // Axes near zero → small spring centering effect
    let spring_coeff = state.axes.roll.abs().max(state.axes.pitch.abs());
    assert!(is_magnitude_safe(spring_coeff));
    let report = serialize_effect(FfbEffect::Spring {
        coefficient: spring_coeff,
    });
    assert_eq!(report[0], REPORT_SPRING);

    // Monitor should be healthy
    assert!(monitor.status().is_healthy());
}

/// Rapid failure sequence → offline → success recovers.
#[test]
fn health_failure_recovery_cycle() {
    let mut m = RhinoHealthMonitor::new();
    // Drive offline
    for _ in 0..3 {
        m.record_failure();
    }
    assert!(m.is_offline());

    // Recovery
    m.record_success(false);
    assert!(!m.is_offline());
    assert!(m.status().is_healthy());
}

/// StopAll is the safe fallback when device goes offline.
#[test]
fn stop_all_on_device_offline() {
    let mut m = RhinoHealthMonitor::new();
    for _ in 0..3 {
        m.record_failure();
    }
    assert!(m.is_offline());
    let report = serialize_effect(FfbEffect::StopAll);
    assert_eq!(report[0], 0xFF, "must send StopAll when offline");
}
