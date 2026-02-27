// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Targeted coverage tests for the RT axis processing pipeline.
//!
//! Covers behaviours not already exercised by the main unit-test or proptest suites:
//! - expo=0.0 is a linear identity pass-through
//! - expo=1.0 produces maximum curvature (f(x) = sign(x) * x²)
//! - deadzone value exactly at, just inside, and just outside the threshold
//! - axis inversion via `AxisFrame::transform(-1.0, 0.0)`
//! - output is always clamped to [-1.0, 1.0] via `AxisFrame::clamp`
//! - combined deadzone → curve pipeline: order of operations and sign preservation

use flight_axis::{AxisFrame, CurveNode, DeadzoneNode, Node, PipelineBuilder};

// ─────────────────────────────────────────────────────────────────────────────
// Curve shape
// ─────────────────────────────────────────────────────────────────────────────

/// expo=0.0 → exponent = 1.0 + 0.0 = 1.0 → f(x) = x  (exact identity).
#[test]
fn test_curve_expo_zero_is_linear() {
    let mut node = CurveNode::new(0.0);

    for &input in &[-1.0f32, -0.5, -0.1, 0.0, 0.1, 0.5, 1.0] {
        let mut frame = AxisFrame::new(input, 1_000);
        node.step(&mut frame);
        assert!(
            (frame.out - input).abs() < 1e-6,
            "expo=0.0 should be identity: input={input}, output={}",
            frame.out
        );
    }
}

/// expo=1.0 → exponent = 2.0 → f(x) = sign(x) * x²  (maximum curvature).
#[test]
fn test_curve_expo_one_max_curvature() {
    let mut node = CurveNode::new(1.0);

    let cases: &[(f32, f32)] = &[(0.5, 0.25), (0.8, 0.64), (1.0, 1.0), (-0.5, -0.25)];
    for &(input, expected) in cases {
        let mut frame = AxisFrame::new(input, 1_000);
        node.step(&mut frame);
        assert!(
            (frame.out - expected).abs() < 1e-5,
            "expo=1.0: input={input} → expected {expected}, got {}",
            frame.out
        );
    }
}

/// S-curve (expo=0.5) reduces gain near centre and recovers at extremes.
#[test]
fn test_curve_scurve_reduces_centre_gain() {
    let mut node = CurveNode::new(0.5); // exponent = 1.5

    // At 0.5 input, output should be 0.5^1.5 ≈ 0.354 (less than 0.5).
    let mut frame = AxisFrame::new(0.5, 1_000);
    node.step(&mut frame);
    assert!(
        frame.out < 0.5,
        "S-curve should reduce gain near centre: input=0.5, output={}",
        frame.out
    );

    // At 1.0 input, output must still be exactly 1.0 (boundary preserved).
    let mut frame_max = AxisFrame::new(1.0, 1_000);
    node.step(&mut frame_max);
    assert!(
        (frame_max.out - 1.0).abs() < 1e-6,
        "S-curve must preserve 1.0 boundary, got {}",
        frame_max.out
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Deadzone boundaries
// ─────────────────────────────────────────────────────────────────────────────

/// Input strictly inside deadzone → zero output.
#[test]
fn test_deadzone_value_inside_is_zero() {
    let mut node = DeadzoneNode::new(0.1);

    let mut frame = AxisFrame::new(0.09, 1_000);
    node.step(&mut frame);
    assert_eq!(frame.out, 0.0, "value inside deadzone must be zeroed");
}

/// Input at the exact threshold → rescaled to 0.0 (edge point).
#[test]
fn test_deadzone_value_at_exact_threshold() {
    // Code: if abs < threshold → 0, else (abs - threshold)/(1 - threshold).
    // At threshold: (0.1 - 0.1) / (1.0 - 0.1) = 0.0.
    let threshold = 0.1f32;
    let mut node = DeadzoneNode::new(threshold);

    let mut frame = AxisFrame::new(threshold, 1_000);
    node.step(&mut frame);
    assert_eq!(frame.out, 0.0, "value at exact threshold rescales to 0.0");
}

/// Input just outside deadzone → small positive output (rescaled, not zero).
#[test]
fn test_deadzone_value_just_outside_is_nonzero() {
    let mut node = DeadzoneNode::new(0.1);

    let mut frame = AxisFrame::new(0.11, 1_000);
    node.step(&mut frame);
    assert!(
        frame.out > 0.0,
        "value just outside deadzone should be non-zero, got {}",
        frame.out
    );
}

/// Negative-side boundary mirrors positive-side behaviour.
#[test]
fn test_deadzone_negative_boundary() {
    let mut node = DeadzoneNode::new(0.1);

    // Inside negative deadzone
    let mut frame_in = AxisFrame::new(-0.05, 1_000);
    node.step(&mut frame_in);
    assert_eq!(frame_in.out, 0.0, "negative inside deadzone must be zero");

    // Outside negative deadzone
    let mut frame_out = AxisFrame::new(-0.5, 1_000);
    node.step(&mut frame_out);
    assert!(
        frame_out.out < 0.0,
        "negative outside deadzone must be negative, got {}",
        frame_out.out
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Axis inversion
// ─────────────────────────────────────────────────────────────────────────────

/// `AxisFrame::transform(-1.0, 0.0)` maps x → -x (hardware axis inversion).
#[test]
fn test_axis_inversion_via_transform() {
    let cases: &[(f32, f32)] = &[(0.5, -0.5), (-0.5, 0.5), (1.0, -1.0), (0.0, 0.0)];

    for &(input, expected) in cases {
        let mut frame = AxisFrame::new(input, 1_000);
        frame.transform(-1.0, 0.0);
        assert!(
            (frame.out - expected).abs() < 1e-6,
            "inversion: input={input} → expected {expected}, got {}",
            frame.out
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Range clamp
// ─────────────────────────────────────────────────────────────────────────────

/// `AxisFrame::clamp(-1.0, 1.0)` never lets output exceed [-1.0, 1.0].
#[test]
fn test_range_clamp_at_limits() {
    let cases: &[(f32, f32)] = &[
        (1.5, 1.0),
        (-2.3, -1.0),
        (0.7, 0.7),
        (f32::INFINITY, 1.0),
        (f32::NEG_INFINITY, -1.0),
    ];

    for &(raw, expected) in cases {
        let mut frame = AxisFrame::new(0.0, 1_000);
        frame.out = raw;
        frame.clamp(-1.0, 1.0);
        assert!(
            (frame.out - expected).abs() < 1e-6,
            "clamp({raw}) → expected {expected}, got {}",
            frame.out
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Combined deadzone + expo
// ─────────────────────────────────────────────────────────────────────────────

/// Deadzone applied before expo: inputs inside deadzone produce exactly zero.
#[test]
fn test_combined_deadzone_expo_zeroes_inside_zone() {
    let pipeline = PipelineBuilder::new()
        .deadzone(0.1)
        .curve(0.5)
        .expect("valid expo")
        .compile()
        .expect("compiles");
    let mut state = pipeline.create_state();

    let mut frame = AxisFrame::new(0.05, 1_000);
    pipeline.process(&mut frame, &mut state);
    assert_eq!(
        frame.out, 0.0,
        "input inside deadzone must give zero after combined deadzone+expo"
    );
}

/// Expo reduces gain vs linear for positive expo (output < linear rescaled value).
#[test]
fn test_combined_deadzone_expo_reduces_gain() {
    let pipeline = PipelineBuilder::new()
        .deadzone(0.1)
        .curve(0.5)
        .expect("valid expo")
        .compile()
        .expect("compiles");
    let mut state = pipeline.create_state();

    let mut frame = AxisFrame::new(0.5, 1_000);
    pipeline.process(&mut frame, &mut state);

    // Linear deadzone output for 0.5 with threshold 0.1: (0.5-0.1)/(0.9) ≈ 0.444
    let linear_dz = (0.5f32 - 0.1) / (1.0 - 0.1);
    assert!(
        frame.out > 0.0,
        "output outside deadzone must be positive, got {}",
        frame.out
    );
    assert!(
        frame.out < linear_dz,
        "expo=0.5 should reduce gain vs linear: output={}, linear_dz={linear_dz}",
        frame.out
    );
}

/// Sign is preserved through the combined pipeline for all quadrants.
#[test]
fn test_combined_deadzone_expo_sign_preservation() {
    let pipeline = PipelineBuilder::new()
        .deadzone(0.05)
        .curve(0.3)
        .expect("valid expo")
        .compile()
        .expect("compiles");
    let mut state_pos = pipeline.create_state();
    let mut state_neg = pipeline.create_state();

    let mut pos_frame = AxisFrame::new(0.8, 1_000);
    pipeline.process(&mut pos_frame, &mut state_pos);
    assert!(
        pos_frame.out > 0.0,
        "positive input must give positive output"
    );

    let mut neg_frame = AxisFrame::new(-0.8, 1_000);
    pipeline.process(&mut neg_frame, &mut state_neg);
    assert!(
        neg_frame.out < 0.0,
        "negative input must give negative output"
    );
}
