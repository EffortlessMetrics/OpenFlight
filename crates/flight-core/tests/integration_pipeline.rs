// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! End-to-end axis pipeline integration tests.
//!
//! Exercises the full pipeline: raw input → deadzone → curve → slew → output.
//! Each test is self-contained with no shared state.

use flight_axis::{
    AxisEngine, AxisFrame, Node, PipelineBuilder,
    nodes::{CurveNode, DeadzoneNode},
};

// ── helpers ────────────────────────────────────────────────────────────────

fn engine_with_pipeline(deadzone: f32, expo: f32) -> AxisEngine {
    let pipeline = PipelineBuilder::new()
        .deadzone(deadzone)
        .curve(expo)
        .unwrap()
        .compile()
        .expect("pipeline should compile");

    let engine = AxisEngine::new_for_axis("pitch".to_string());
    engine.update_pipeline(pipeline);
    // Tick once to swap the pending pipeline into active.
    let mut dummy = AxisFrame::new(0.0, 0);
    let _ = engine.process(&mut dummy);
    engine
}

fn process_value(engine: &AxisEngine, input: f32) -> f32 {
    let mut frame = AxisFrame::new(input, 1_000_000);
    engine.process(&mut frame).expect("process ok");
    frame.out
}

// ── 1. Zero input passes through pipeline unchanged ──────────────────────

#[test]
fn integration_zero_input_produces_zero_output() {
    let engine = engine_with_pipeline(0.05, 0.3);
    let out = process_value(&engine, 0.0);
    assert!(out.abs() < 1e-6, "expected ~0.0, got {out}");
}

// ── 2. Full-scale input (1.0) reaches full output ────────────────────────

#[test]
fn integration_full_scale_input_reaches_full_output() {
    let engine = engine_with_pipeline(0.05, 0.3);
    let out = process_value(&engine, 1.0);
    // After deadzone rescaling and expo curve, 1.0 input should give ~1.0 output.
    assert!((out - 1.0).abs() < 0.02, "expected ~1.0, got {out}");
}

// ── 3. Deadzone kills small inputs ───────────────────────────────────────

#[test]
fn integration_deadzone_kills_small_input() {
    let engine = engine_with_pipeline(0.10, 0.0);
    // Input below deadzone threshold should be zeroed.
    let out = process_value(&engine, 0.05);
    assert!(
        out.abs() < 1e-6,
        "expected 0.0 (inside deadzone), got {out}"
    );
}

// ── 4. Input just above deadzone produces small positive output ──────────

#[test]
fn integration_input_just_above_deadzone() {
    let engine = engine_with_pipeline(0.10, 0.0); // linear curve
    let out = process_value(&engine, 0.15);
    // Should be small positive after rescaling: (0.15 - 0.10) / (1.0 - 0.10) ≈ 0.0556
    assert!(
        out > 0.0 && out < 0.10,
        "expected small positive output, got {out}"
    );
}

// ── 5. Expo curves modify mid-range values ───────────────────────────────

#[test]
fn integration_expo_curve_modifies_midrange() {
    // With expo > 0, mid-range values should be reduced.
    let engine_linear = engine_with_pipeline(0.0, 0.0);
    let engine_expo = engine_with_pipeline(0.0, 0.5);

    let linear_out = process_value(&engine_linear, 0.5);
    let expo_out = process_value(&engine_expo, 0.5);

    // Expo curve should make 0.5 smaller than linear.
    assert!(
        expo_out < linear_out,
        "expo output ({expo_out}) should be less than linear ({linear_out})"
    );
}

// ── 6. Negative inputs are handled symmetrically ─────────────────────────

#[test]
fn integration_negative_input_symmetric() {
    let engine = engine_with_pipeline(0.05, 0.3);
    let pos_out = process_value(&engine, 0.5);
    let neg_out = process_value(&engine, -0.5);
    assert!(
        (pos_out + neg_out).abs() < 0.02,
        "outputs should be symmetric: +{pos_out} vs -{neg_out}"
    );
}

// ── 7. Full negative input reaches -1.0 ──────────────────────────────────

#[test]
fn integration_full_negative_input() {
    let engine = engine_with_pipeline(0.05, 0.3);
    let out = process_value(&engine, -1.0);
    assert!((out + 1.0).abs() < 0.02, "expected ~-1.0, got {out}");
}

// ── 8. Pipeline with only deadzone (no curve) ────────────────────────────

#[test]
fn integration_deadzone_only_pipeline() {
    let pipeline = PipelineBuilder::new()
        .deadzone(0.05)
        .compile()
        .expect("compile ok");

    let engine = AxisEngine::new_for_axis("throttle".to_string());
    engine.update_pipeline(pipeline);
    let mut dummy = AxisFrame::new(0.0, 0);
    let _ = engine.process(&mut dummy);

    let mut frame = AxisFrame::new(0.5, 1_000_000);
    engine.process(&mut frame).expect("process ok");
    // Linear pass-through after deadzone rescaling.
    let expected = (0.5_f32 - 0.05) / (1.0 - 0.05);
    assert!(
        (frame.out - expected).abs() < 0.01,
        "expected ~{expected}, got {}",
        frame.out
    );
}

// ── 9. Multiple frames processed sequentially ────────────────────────────

#[test]
fn integration_sequential_frame_processing() {
    let engine = engine_with_pipeline(0.03, 0.2);
    let values = [0.0, 0.1, 0.5, 0.9, 1.0, -0.3, -1.0];
    let mut prev_out = None;

    for &input in &values {
        let out = process_value(&engine, input);
        // Verify output is always in [-1.0, 1.0] range.
        assert!(
            (-1.0..=1.0).contains(&out),
            "output {out} out of range for input {input}"
        );
        prev_out = Some(out);
    }
    assert!(prev_out.is_some());
}

// ── 10. Engine without pipeline is pass-through ──────────────────────────

#[test]
fn integration_engine_without_pipeline_passthrough() {
    let engine = AxisEngine::new_for_axis("yaw".to_string());
    let mut frame = AxisFrame::new(0.75, 1_000_000);
    engine.process(&mut frame).expect("process ok");
    // With no pipeline, output should equal input (pass-through).
    assert!(
        (frame.out - 0.75).abs() < 0.02,
        "expected pass-through ~0.75, got {}",
        frame.out
    );
}

// ── 11. DeadzoneNode step directly zeroes small values ───────────────────

#[test]
fn integration_deadzone_node_direct_step() {
    let mut node = DeadzoneNode::new(0.05);
    let mut frame = AxisFrame::new(0.03, 1_000);
    node.step(&mut frame);
    assert!(
        frame.out.abs() < 1e-6,
        "deadzone should zero 0.03 with threshold 0.05"
    );
}

// ── 12. CurveNode with zero expo is identity ─────────────────────────────

#[test]
fn integration_curve_node_zero_expo_identity() {
    let mut node = CurveNode::new(0.0);
    let mut frame = AxisFrame::new(0.42, 1_000);
    node.step(&mut frame);
    assert!(
        (frame.out - 0.42).abs() < 1e-6,
        "zero expo should be identity, got {}",
        frame.out
    );
}

// ── 13. Pipeline compile rejects empty builder ───────────────────────────

#[test]
fn integration_empty_pipeline_rejected() {
    let result = PipelineBuilder::new().compile();
    assert!(result.is_err(), "empty pipeline should fail compilation");
}
