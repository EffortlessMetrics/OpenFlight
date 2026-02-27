// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Integration tests for the calibration → EMA smoothing → rate-limit → trim pipeline.
//!
//! Validates the complete pre-RT processing chain used before the axis signal
//! enters the RT spine.

use flight_axis::calibration::AxisCalibration;
use flight_axis::rate_limit::AxisRateLimiter;
use flight_axis::smoothing::EmaFilter;
use flight_axis::trim::AxisTrim;

/// Runs a single shot through the full pre-RT pipeline with freshly constructed components.
fn full_pipeline(raw: u16, alpha: f32, max_rate: f32, trim_offset: f32) -> f32 {
    let cal = AxisCalibration::default_full_range();
    let normalized = cal.normalize(raw);

    let mut ema = EmaFilter::new(alpha);
    let smoothed = ema.apply(normalized);

    let mut rate = AxisRateLimiter::new(max_rate);
    let rate_limited = rate.apply(smoothed);

    let mut trim = AxisTrim::default();
    trim.set_offset(trim_offset);
    trim.apply(rate_limited)
}

#[test]
fn test_pipeline_center_no_processing() {
    // Center raw value, passthrough EMA, unlimited rate, no trim → ~0.0.
    let out = full_pipeline(32767, 1.0, 0.0, 0.0);
    assert!(out.abs() < 0.01, "center input expected ~0.0, got {out}");
}

#[test]
fn test_pipeline_max_input_no_trim() {
    // Maximum raw value with all passthrough settings → 1.0.
    let out = full_pipeline(65535, 1.0, 0.0, 0.0);
    assert!(
        (out - 1.0).abs() < 1e-6,
        "max input expected 1.0, got {out}"
    );
}

#[test]
fn test_pipeline_min_input_no_trim() {
    // Minimum raw value with all passthrough settings → -1.0.
    let out = full_pipeline(0, 1.0, 0.0, 0.0);
    assert!(
        (out - (-1.0)).abs() < 1e-6,
        "min input expected -1.0, got {out}"
    );
}

#[test]
fn test_pipeline_max_input_max_trim_clamps() {
    // max raw + positive trim offset → output is still clamped at 1.0.
    let out = full_pipeline(65535, 1.0, 0.0, 0.3);
    assert!(
        (out - 1.0).abs() < 1e-6,
        "max input + max trim expected 1.0 (clamped), got {out}"
    );
}

#[test]
fn test_pipeline_smoothing_reduces_step_response() {
    // Seed two EMA filters at 0.0 (center), then step to 1.0 (max).
    // Alpha=1.0 (passthrough) immediately outputs 1.0.
    // Alpha=0.1 (heavy smoothing) outputs ~0.1 on the first post-seed sample.
    let cal = AxisCalibration::default_full_range();
    let max_normalized = cal.normalize(65535); // 1.0

    let mut ema_passthrough = EmaFilter::new(1.0);
    ema_passthrough.apply(0.0); // seed state at 0.0

    let mut ema_smooth = EmaFilter::new(0.1);
    ema_smooth.apply(0.0); // seed state at 0.0

    let out_passthrough = ema_passthrough.apply(max_normalized);
    let out_smooth = ema_smooth.apply(max_normalized);

    assert!(
        (out_passthrough - 1.0).abs() < 1e-6,
        "passthrough should immediately reach 1.0, got {out_passthrough}"
    );
    assert!(
        out_smooth < out_passthrough,
        "smoothed output {out_smooth} should lag behind passthrough {out_passthrough}"
    );
    // EMA formula: 0.1 * 1.0 + 0.9 * 0.0 = 0.1
    assert!(
        (out_smooth - 0.1).abs() < 1e-5,
        "alpha=0.1 step from 0→1 should give ~0.1, got {out_smooth}"
    );
}

#[test]
fn test_pipeline_rate_limit_slows_changes() {
    // Rate of 0.1/tick: a step from 0.0 to 1.0 requires exactly 10 ticks.
    let mut rate = AxisRateLimiter::new(0.1);

    let out1 = rate.apply(1.0);
    assert!(
        (out1 - 0.1).abs() < 1e-6,
        "tick 1 should be 0.1, got {out1}"
    );

    for _ in 0..4 {
        rate.apply(1.0);
    }
    let out6 = rate.apply(1.0);
    assert!(
        (out6 - 0.6).abs() < 1e-6,
        "tick 6 should be 0.6, got {out6}"
    );
}

#[test]
fn test_pipeline_output_always_bounded() {
    // Sweep representative raw values and trim offsets; output must always be in [-1.0, 1.0].
    for raw in [0u16, 16383, 32767, 49152, 65535] {
        for trim in [-0.3f32, 0.0, 0.3] {
            let out = full_pipeline(raw, 0.8, 0.05, trim);
            assert!(
                out >= -1.0 && out <= 1.0,
                "raw={raw} trim={trim} → out={out} out of bounds [-1.0, 1.0]"
            );
        }
    }
}
