// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Axis engine pipeline end-to-end integration tests.
//!
//! Proves: raw → deadzone → curve → expo → output, profile switch,
//! multi-axis, clamping, tick timing, and latency measurement.

use flight_axis::pipeline::{
    AxisPipeline, ClampStage, CurveStage, DeadzoneStage, SensitivityStage,
};
use flight_axis::{AxisEngine, AxisFrame, PipelineBuilder};
use flight_test_helpers::{DeterministicClock, assert_approx_eq, assert_in_range};

// ---------------------------------------------------------------------------
// Helpers
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

fn expected_standard_output(raw: f64) -> f64 {
    let dz_inner = 0.05_f64;
    let dz_outer = 1.0_f64;
    let expo = 0.3_f64;

    let abs_raw = raw.abs();
    if abs_raw <= dz_inner {
        return 0.0;
    }
    let rescaled = (abs_raw - dz_inner) / (dz_outer - dz_inner);
    let curved = rescaled.powf(1.0 + expo);
    let result = raw.signum() * curved;
    result.clamp(-1.0, 1.0)
}

// ===========================================================================
// 1. Raw input → deadzone → curve → expo → output
// ===========================================================================

#[test]
fn e2e_axis_raw_through_full_pipeline() {
    let pipeline = standard_pipeline();
    let test_inputs: Vec<f64> = vec![
        0.0, 0.02, 0.05, 0.1, 0.25, 0.5, 0.75, 1.0, -0.5, -1.0,
    ];

    for &raw in &test_inputs {
        let out = pipeline.process(raw, 0.004);
        let expected = expected_standard_output(raw);
        assert_approx_eq(out, expected, 1e-6);
        assert!(out.is_finite(), "output for input {raw} must be finite");
        assert_in_range(out, -1.0, 1.0);
    }

    // Deadzone: inputs within ±0.05 produce zero
    assert_eq!(pipeline.process(0.0, 0.004), 0.0);
    assert_eq!(pipeline.process(0.02, 0.004), 0.0);
    assert_eq!(pipeline.process(-0.03, 0.004), 0.0);
    assert_eq!(pipeline.process(0.05, 0.004), 0.0);

    // Just outside deadzone produces non-zero
    let just_outside = pipeline.process(0.06, 0.004);
    assert!(just_outside > 0.0, "0.06 must produce positive output");

    // Full deflection
    let full = pipeline.process(1.0, 0.004);
    assert!((full - 1.0).abs() < 1e-6, "1.0 input must produce 1.0 output");
}

// ===========================================================================
// 2. Profile switch mid-stream (atomic swap)
// ===========================================================================

#[test]
fn e2e_axis_profile_switch_midstream() {
    let engine = AxisEngine::new_for_axis("pitch".to_string());

    // Pipeline A: small deadzone (5%), moderate curve
    let pipeline_a = PipelineBuilder::new()
        .deadzone(0.05)
        .curve(0.3)
        .unwrap()
        .compile()
        .expect("pipeline A");
    engine.update_pipeline(pipeline_a);

    // Process first frame with pipeline A
    let mut frame_a = AxisFrame::new(0.5, 0);
    engine.process(&mut frame_a).expect("process A");
    let output_a = frame_a.out;
    assert!(output_a > 0.0, "pipeline A should produce positive output");

    // Switch to Pipeline B: larger deadzone (15%), heavier curve
    let pipeline_b = PipelineBuilder::new()
        .deadzone(0.15)
        .curve(0.6)
        .unwrap()
        .compile()
        .expect("pipeline B");
    engine.update_pipeline(pipeline_b);

    // Process same input with pipeline B
    let mut frame_b = AxisFrame::new(0.5, 4_000_000);
    engine.process(&mut frame_b).expect("process B");
    let output_b = frame_b.out;

    // Different pipelines must produce different outputs for same input
    assert!(
        (output_a - output_b).abs() > 0.01,
        "profile switch must change output: A={output_a}, B={output_b}"
    );

    // Value within pipeline B's deadzone
    let mut frame_dz = AxisFrame::new(0.1, 8_000_000);
    engine.process(&mut frame_dz).expect("process dz");
    assert_eq!(
        frame_dz.out, 0.0,
        "0.1 input inside 15% deadzone must produce zero"
    );
}

// ===========================================================================
// 3. Multiple axis channels simultaneously
// ===========================================================================

#[test]
fn e2e_axis_multiple_channels_simultaneous() {
    let pitch_engine = AxisEngine::new_for_axis("pitch".to_string());
    let roll_engine = AxisEngine::new_for_axis("roll".to_string());
    let yaw_engine = AxisEngine::new_for_axis("yaw".to_string());
    let throttle_engine = AxisEngine::new_for_axis("throttle".to_string());

    // Each axis gets the same pipeline config
    for engine in [&pitch_engine, &roll_engine, &yaw_engine, &throttle_engine] {
        let pipeline = PipelineBuilder::new()
            .deadzone(0.05)
            .curve(0.3)
            .unwrap()
            .compile()
            .expect("compile");
        engine.update_pipeline(pipeline);
    }

    let inputs = [0.8_f32, -0.6, 0.02, 0.95];
    let engines = [&pitch_engine, &roll_engine, &yaw_engine, &throttle_engine];
    let mut outputs = Vec::new();

    for (i, engine) in engines.iter().enumerate() {
        let mut frame = AxisFrame::new(inputs[i], i as u64 * 4_000_000);
        engine.process(&mut frame).expect("process");
        outputs.push(frame.out);
    }

    // Verify independence: each engine processes its own input
    assert!(outputs[0] > 0.0, "pitch 0.8 → positive");
    assert!(outputs[1] < 0.0, "roll -0.6 → negative");
    assert_eq!(outputs[2], 0.0, "yaw 0.02 inside deadzone");
    assert!(outputs[3] > outputs[0], "throttle 0.95 > pitch 0.8");

    // All outputs finite and in range
    for &out in &outputs {
        assert!(out.is_finite());
        assert_in_range(out as f64, -1.0, 1.0);
    }
}

// ===========================================================================
// 4. Axis clamping at bounds
// ===========================================================================

#[test]
fn e2e_axis_clamping_at_bounds() {
    // Pipeline with high sensitivity to trigger clamping
    let mut pipeline = AxisPipeline::new();
    pipeline.add_stage(Box::new(SensitivityStage { multiplier: 3.0 }));
    pipeline.add_stage(Box::new(ClampStage {
        min: -1.0,
        max: 1.0,
    }));

    // Values that would exceed ±1.0 after sensitivity multiplication
    let test_cases = [
        (0.5, 1.0),   // 0.5 * 3.0 = 1.5 → clamped to 1.0
        (-0.5, -1.0),  // -0.5 * 3.0 = -1.5 → clamped to -1.0
        (0.3, 0.9),    // 0.3 * 3.0 = 0.9 → not clamped
        (1.0, 1.0),    // 1.0 * 3.0 = 3.0 → clamped to 1.0
        (-1.0, -1.0),  // -1.0 * 3.0 = -3.0 → clamped to -1.0
        (0.0, 0.0),    // 0 stays 0
    ];

    for &(input, expected) in &test_cases {
        let out = pipeline.process(input, 0.004);
        assert_approx_eq(out, expected, 1e-10);
        assert_in_range(out, -1.0, 1.0);
    }
}

// ===========================================================================
// 5. Axis engine tick timing (250Hz)
// ===========================================================================

#[test]
fn e2e_axis_engine_tick_timing_250hz() {
    let engine = AxisEngine::new_for_axis("pitch".to_string());
    let pipeline = PipelineBuilder::new()
        .deadzone(0.05)
        .curve(0.2)
        .unwrap()
        .compile()
        .expect("compile");
    engine.update_pipeline(pipeline);

    let mut clock = DeterministicClock::new(0);
    let mut outputs = Vec::new();
    let mut timestamps = Vec::new();

    // Simulate 1 second at 250Hz
    for tick in 0..250 {
        let t = tick as f32 / 250.0;
        let input = (t * std::f32::consts::TAU).sin() * 0.8;
        let ts_ns = clock.now_us() * 1_000;
        let mut frame = AxisFrame::new(input, ts_ns);
        engine.process(&mut frame).expect("process");
        outputs.push(frame.out);
        timestamps.push(clock.now_us());
        clock.advance_ticks(1);
    }

    assert_eq!(outputs.len(), 250, "must process exactly 250 frames");
    assert_eq!(clock.now_us(), 1_000_000, "250 ticks = 1 second");

    // Verify tick spacing
    for pair in timestamps.windows(2) {
        assert_eq!(pair[1] - pair[0], 4_000, "expected 4ms tick spacing");
    }

    // All outputs finite and in range
    for (i, &out) in outputs.iter().enumerate() {
        assert!(out.is_finite(), "output[{i}] not finite");
        assert_in_range(out as f64, -1.0, 1.0);
    }

    // Sinusoidal input should produce both positive and negative outputs
    let has_positive = outputs.iter().any(|&o| o > 0.1);
    let has_negative = outputs.iter().any(|&o| o < -0.1);
    assert!(has_positive, "sinusoid must have positive peaks");
    assert!(has_negative, "sinusoid must have negative peaks");
}

// ===========================================================================
// 6. Pipeline latency measurement
// ===========================================================================

#[test]
fn e2e_axis_pipeline_latency_measurement() {
    let engine = AxisEngine::new_for_axis("pitch".to_string());
    let pipeline = PipelineBuilder::new()
        .deadzone(0.05)
        .curve(0.3)
        .unwrap()
        .filter(0.3)
        .compile()
        .expect("compile");
    engine.update_pipeline(pipeline);

    let mut clock = DeterministicClock::new(0);

    // Measure processing time for a step input
    let mut step_outputs = Vec::new();
    let mut ticks_to_90_pct = 0_u32;

    // Step from 0.0 to 0.8 and measure convergence
    for tick in 0..100 {
        let input = if tick < 10 { 0.0_f32 } else { 0.8 };
        let ts_ns = clock.now_us() * 1_000;
        let mut frame = AxisFrame::new(input, ts_ns);
        engine.process(&mut frame).expect("process");
        step_outputs.push(frame.out);
        clock.advance_ticks(1);

        // Measure latency: how many ticks until output reaches 90% of target
        if tick >= 10 && ticks_to_90_pct == 0 {
            // Target is what the pipeline would produce for 0.8 at steady state
            let target = expected_standard_output(0.8);
            if (frame.out as f64) >= target * 0.9 {
                ticks_to_90_pct = tick - 10;
            }
        }
    }

    // All outputs must be finite
    for (i, &out) in step_outputs.iter().enumerate() {
        assert!(out.is_finite(), "output[{i}] not finite: {out}");
    }

    // Before step: outputs near zero (in deadzone)
    for &out in &step_outputs[..10] {
        assert_eq!(out, 0.0, "pre-step output should be zero");
    }

    // After step: output should eventually be significantly positive
    let final_output = *step_outputs.last().unwrap();
    assert!(final_output > 0.3, "output must converge to positive value");

    // Latency should be bounded (filter adds lag but shouldn't be excessive)
    assert!(
        ticks_to_90_pct < 50,
        "pipeline latency must converge within 50 ticks (200ms), got {ticks_to_90_pct}"
    );
}
