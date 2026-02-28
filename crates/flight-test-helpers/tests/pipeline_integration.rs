// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! End-to-end pipeline integration tests.
//!
//! Proves the full axis processing pipeline works with fake device backends
//! and deterministic timing. Uses [`flight_test_helpers`] utilities for
//! reproducible, hardware-free testing.

use flight_axis::pipeline::{
    AxisPipeline, ClampStage, CurveStage, DeadzoneStage, SensitivityStage, SmoothingStage,
};
use flight_axis::{AxisEngine, AxisFrame, InputValidator, PipelineBuilder};
use flight_test_helpers::{DeterministicClock, FakeDevice, FakeInput};

// ---------------------------------------------------------------------------
// Helper: build a standard deadzone → curve → clamp pipeline
// ---------------------------------------------------------------------------

fn standard_pipeline() -> AxisPipeline {
    let mut pipeline = AxisPipeline::new();
    pipeline.add_stage(Box::new(DeadzoneStage {
        inner: 0.05,
        outer: 1.0,
    }));
    pipeline.add_stage(Box::new(CurveStage { expo: 0.3 }));
    pipeline.add_stage(Box::new(ClampStage {
        min: -1.0,
        max: 1.0,
    }));
    pipeline
}

// ===========================================================================
// 1. Raw input → deadzone → curve → output
// ===========================================================================

#[test]
fn deadzone_then_curve_end_to_end() {
    let pipeline = standard_pipeline();

    // Input within deadzone → output must be zero
    let out = pipeline.process(0.03, 0.004);
    assert!(
        out.abs() < f64::EPSILON,
        "input inside deadzone should produce 0, got {out}"
    );

    // Input at full deflection → output must be 1.0 (after curve)
    let out = pipeline.process(1.0, 0.004);
    assert!(
        (out - 1.0).abs() < 1e-10,
        "full deflection should produce 1.0, got {out}"
    );

    // Mid-range input: passes through deadzone rescale then curve
    let dz_inner: f64 = 0.05;
    let dz_outer: f64 = 1.0;
    let raw: f64 = 0.5;
    let after_dz = (raw - dz_inner) / (dz_outer - dz_inner); // ~0.4737
    let expected = after_dz.signum() * after_dz.abs().powf(1.0 + 0.3);
    let out = pipeline.process(raw, 0.004);
    assert!(
        (out - expected).abs() < 1e-6,
        "mid-range expected {expected}, got {out}"
    );
}

// ===========================================================================
// 2. Multiple axes process independently
// ===========================================================================

#[test]
fn multiple_axes_independent() {
    let pitch = standard_pipeline();
    let roll = standard_pipeline();

    // Feed different values into each axis
    let pitch_out = pitch.process(0.8, 0.004);
    let roll_out = roll.process(-0.3, 0.004);

    // They must not interfere with each other
    assert!(pitch_out > 0.0, "pitch should be positive");
    assert!(roll_out < 0.0, "roll should be negative");

    // Verify exact values through the same deadzone+curve math
    let dz_inner: f64 = 0.05;
    let dz_outer: f64 = 1.0;

    let pitch_dz: f64 = (0.8 - dz_inner) / (dz_outer - dz_inner);
    let expected_pitch = pitch_dz.abs().powf(1.3);
    assert!(
        (pitch_out - expected_pitch).abs() < 1e-6,
        "pitch: expected {expected_pitch}, got {pitch_out}"
    );

    let roll_dz: f64 = -(0.3 - dz_inner) / (dz_outer - dz_inner);
    let expected_roll = -(roll_dz.abs().powf(1.3));
    assert!(
        (roll_out - expected_roll).abs() < 1e-6,
        "roll: expected {expected_roll}, got {roll_out}"
    );
}

// ===========================================================================
// 3. Edge cases: zero, maximum deflection, NaN rejection
// ===========================================================================

#[test]
fn zero_input_produces_zero() {
    let pipeline = standard_pipeline();
    let out = pipeline.process(0.0, 0.004);
    assert!(
        out.abs() < f64::EPSILON,
        "zero input should produce 0, got {out}"
    );
}

#[test]
fn maximum_positive_deflection() {
    let pipeline = standard_pipeline();
    let out = pipeline.process(1.0, 0.004);
    assert!(
        (out - 1.0).abs() < 1e-10,
        "max positive should produce 1.0, got {out}"
    );
}

#[test]
fn maximum_negative_deflection() {
    let pipeline = standard_pipeline();
    let out = pipeline.process(-1.0, 0.004);
    assert!(
        (out - (-1.0)).abs() < 1e-10,
        "max negative should produce -1.0, got {out}"
    );
}

#[test]
fn nan_rejection_via_input_validator() {
    let mut validator = InputValidator::new();

    // First feed a valid value so the validator has a baseline
    validator.update(0.5);

    // NaN should be replaced with last valid value
    let sanitised = validator.update(f32::NAN);
    assert_eq!(sanitised, 0.5, "NaN should be replaced with last valid");
    assert_eq!(validator.nan_count(), 1);

    // Infinity should be clamped to ±1.0
    let sanitised = validator.update(f32::INFINITY);
    assert_eq!(sanitised, 1.0, "+Inf should clamp to 1.0");
    let sanitised = validator.update(f32::NEG_INFINITY);
    assert_eq!(sanitised, -1.0, "-Inf should clamp to -1.0");
    assert_eq!(validator.inf_count(), 2);
}

#[test]
fn nan_does_not_propagate_through_pipeline() {
    let mut validator = InputValidator::new();
    let pipeline = standard_pipeline();

    // Inject NaN — validator replaces with 0.0 (no prior valid)
    let clean = validator.update(f32::NAN) as f64;
    let out = pipeline.process(clean, 0.004);
    assert!(
        out.is_finite(),
        "pipeline output must be finite after NaN injection"
    );
    assert!(
        out.abs() < f64::EPSILON,
        "fallback 0.0 inside deadzone should produce 0"
    );
}

// ===========================================================================
// 4. Fake device backend → pipeline integration
// ===========================================================================

#[test]
fn fake_device_stream_through_pipeline() {
    let mut device = FakeDevice::new("Test Joystick", 0x044F, 0xB10A, 3, 12);
    device.connect();

    // Enqueue deterministic axis inputs
    let test_values = [0.0, 0.03, 0.25, 0.5, 0.75, 1.0, -0.5, -1.0];
    for &val in &test_values {
        device.enqueue_input(FakeInput {
            axes: vec![val, 0.0, 0.0],
            buttons: vec![false; 12],
            delay_ms: 4, // 250 Hz
        });
    }

    let pipeline = standard_pipeline();
    let mut outputs = Vec::new();

    while let Some(input) = device.next_input() {
        let axis_val = input.axes[0];
        let out = pipeline.process(axis_val, 0.004);
        outputs.push(out);
    }

    assert_eq!(outputs.len(), test_values.len());

    // Values inside deadzone (|v| <= 0.05) should be zero
    assert!(outputs[0].abs() < f64::EPSILON, "0.0 → 0"); // 0.0
    assert!(outputs[1].abs() < f64::EPSILON, "0.03 → 0"); // 0.03

    // All outputs must be finite and within [-1, 1]
    for (i, &o) in outputs.iter().enumerate() {
        assert!(o.is_finite(), "output[{i}] is not finite: {o}");
        assert!(
            (-1.0..=1.0).contains(&o),
            "output[{i}] = {o} outside [-1, 1]"
        );
    }
}

#[test]
fn three_axis_device_processes_independently() {
    let mut device = FakeDevice::new("3-Axis Stick", 0x06A3, 0x0762, 3, 0);
    device.connect();
    device.enqueue_input(FakeInput {
        axes: vec![0.8, -0.6, 0.02],
        buttons: vec![],
        delay_ms: 4,
    });

    let pitch_pipe = standard_pipeline();
    let roll_pipe = standard_pipeline();
    let yaw_pipe = standard_pipeline();

    let input = device.next_input().unwrap();
    let pitch_out = pitch_pipe.process(input.axes[0], 0.004);
    let roll_out = roll_pipe.process(input.axes[1], 0.004);
    let yaw_out = yaw_pipe.process(input.axes[2], 0.004);

    assert!(pitch_out > 0.0, "pitch axis should be positive");
    assert!(roll_out < 0.0, "roll axis should be negative");
    assert!(
        yaw_out.abs() < f64::EPSILON,
        "yaw 0.02 inside deadzone should be 0"
    );
}

// ===========================================================================
// 5. Deterministic clock + timing behaviour
// ===========================================================================

#[test]
fn deterministic_clock_drives_timestamps() {
    let mut clock = DeterministicClock::new(0);

    // Process frames at 250 Hz (4 ms per tick) using the deterministic clock
    let mut frames = Vec::new();
    for i in 0..10 {
        let ts_ns = clock.now_us() * 1_000; // µs → ns
        let input = (i as f32) * 0.1; // ramp 0.0 → 0.9
        frames.push(AxisFrame::new(input, ts_ns));
        clock.advance_ticks(1);
    }

    // Verify timestamps are 4 ms apart
    for pair in frames.windows(2) {
        let delta_ns = pair[1].ts_mono_ns - pair[0].ts_mono_ns;
        assert_eq!(delta_ns, 4_000_000, "expected 4ms tick spacing");
    }
}

#[test]
fn compiled_pipeline_with_deterministic_timing() {
    let pipeline = PipelineBuilder::new()
        .deadzone(0.05)
        .curve(0.3)
        .unwrap()
        .compile()
        .expect("pipeline should compile");

    let mut state = pipeline.create_state();
    let mut clock = DeterministicClock::new(1_000); // start at 1ms

    let inputs: [f32; 5] = [0.0, 0.03, 0.5, -0.8, 1.0];
    let mut outputs = Vec::new();

    for &raw_input in &inputs {
        let ts_ns = clock.now_us() * 1_000;
        let mut frame = AxisFrame::new(raw_input, ts_ns);
        pipeline.process(&mut frame, &mut state);
        outputs.push(frame.out);
        clock.advance_ticks(1);
    }

    // Zero and within-deadzone inputs must produce zero
    assert_eq!(outputs[0], 0.0, "zero input");
    assert_eq!(outputs[1], 0.0, "0.03 inside 5% deadzone");

    // Full deflection must produce ±1.0
    assert!(
        (outputs[4] - 1.0).abs() < 1e-6,
        "full positive should be 1.0, got {}",
        outputs[4]
    );

    // All must be finite
    for (i, &o) in outputs.iter().enumerate() {
        assert!(o.is_finite(), "output[{i}] not finite: {o}");
    }
}

#[test]
fn engine_pipeline_with_deterministic_timing() {
    let engine = AxisEngine::new_for_axis("pitch".to_string());

    let pipeline = PipelineBuilder::new()
        .deadzone(0.05)
        .curve(0.3)
        .unwrap()
        .compile()
        .expect("pipeline should compile");

    engine.update_pipeline(pipeline);

    let mut clock = DeterministicClock::new(1_000);
    let mut outputs = Vec::new();

    let inputs: [f32; 6] = [0.0, 0.02, 0.5, 0.9, -0.7, 1.0];

    for &raw_input in &inputs {
        let ts_ns = clock.now_us() * 1_000;
        let mut frame = AxisFrame::new(raw_input, ts_ns);
        engine.process(&mut frame).expect("process should succeed");
        outputs.push(frame.out);
        clock.advance_ticks(1);
    }

    // Deadzone coverage
    assert_eq!(outputs[0], 0.0, "zero → zero");
    assert_eq!(outputs[1], 0.0, "0.02 inside 5% deadzone");

    // Positive mid-range
    assert!(outputs[2] > 0.0, "0.5 should produce positive output");
    assert!(outputs[3] > outputs[2], "0.9 > 0.5 in output");

    // Negative
    assert!(outputs[4] < 0.0, "-0.7 should produce negative output");

    // All finite
    for (i, &o) in outputs.iter().enumerate() {
        assert!(o.is_finite(), "output[{i}] not finite: {o}");
    }
}

// ===========================================================================
// 6. Smoothing stage preserves timing coherence
// ===========================================================================

#[test]
fn smoothing_converges_over_time() {
    let mut pipeline = AxisPipeline::new();
    pipeline.add_stage(Box::new(SmoothingStage::new(0.3)));

    let mut clock = DeterministicClock::new(0);
    let mut prev = 0.0_f64;

    // Apply a step input of 1.0 for 20 ticks — output should converge
    for _ in 0..20 {
        let _ts = clock.now_us();
        let out = pipeline.process(1.0, 0.004);
        assert!(
            out >= prev,
            "smoothing must be monotonically non-decreasing"
        );
        prev = out;
        clock.advance_ticks(1);
    }

    // After 20 ticks with alpha=0.3, output should be close to 1.0
    assert!(
        prev > 0.99,
        "after 20 ticks the smoothed value should converge near 1.0, got {prev}"
    );
}

// ===========================================================================
// 7. Pipeline bypass
// ===========================================================================

#[test]
fn bypass_stage_passes_through_unmodified() {
    let mut pipeline = AxisPipeline::new();
    pipeline.add_stage(Box::new(DeadzoneStage {
        inner: 0.05,
        outer: 1.0,
    }));
    pipeline.add_stage(Box::new(CurveStage { expo: 0.3 }));

    // With all stages active
    let active_out = pipeline.process(0.5, 0.004);

    // Bypass the curve stage
    pipeline.bypass_stage(1);
    let bypass_out = pipeline.process(0.5, 0.004);

    // Without the curve, the deadzone-rescaled value should pass straight through
    let dz_inner = 0.05;
    let dz_outer = 1.0;
    let expected_no_curve = (0.5 - dz_inner) / (dz_outer - dz_inner);
    assert!(
        (bypass_out - expected_no_curve).abs() < 1e-6,
        "bypassed curve should give raw deadzone output {expected_no_curve}, got {bypass_out}"
    );
    assert!(
        (active_out - bypass_out).abs() > 1e-6,
        "active and bypass outputs should differ"
    );

    // Re-enable and verify we get the original result
    pipeline.enable_stage(1);
    let restored_out = pipeline.process(0.5, 0.004);
    assert!(
        (restored_out - active_out).abs() < 1e-10,
        "re-enabled pipeline should match original"
    );
}

// ===========================================================================
// 8. Sensitivity + clamp chain
// ===========================================================================

#[test]
fn sensitivity_then_clamp_limits_output() {
    let mut pipeline = AxisPipeline::new();
    pipeline.add_stage(Box::new(SensitivityStage { multiplier: 3.0 }));
    pipeline.add_stage(Box::new(ClampStage {
        min: -1.0,
        max: 1.0,
    }));

    // 0.5 * 3.0 = 1.5, clamped to 1.0
    let out = pipeline.process(0.5, 0.004);
    assert!(
        (out - 1.0).abs() < f64::EPSILON,
        "should clamp to 1.0, got {out}"
    );

    // -0.4 * 3.0 = -1.2, clamped to -1.0
    let out = pipeline.process(-0.4, 0.004);
    assert!(
        (out - (-1.0)).abs() < f64::EPSILON,
        "should clamp to -1.0, got {out}"
    );

    // 0.2 * 3.0 = 0.6, within range
    let out = pipeline.process(0.2, 0.004);
    assert!(
        (out - 0.6).abs() < 1e-10,
        "within range should be 0.6, got {out}"
    );
}
