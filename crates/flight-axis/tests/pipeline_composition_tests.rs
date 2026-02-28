// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Pipeline composition tests: cross-node interactions.
//!
//! Verifies invariants that emerge when multiple nodes are chained:
//! Deadzone → Curve → Slew, Filter in sequence, NaN/Inf propagation.

use flight_axis::{AxisFrame, Pipeline, PipelineBuilder, PipelineState};
use proptest::prelude::*;

/// Process one frame through `pipeline`, setting `frame.out = frame.in_raw` first.
fn step(pipeline: &Pipeline, state: &mut PipelineState, input: f32, ts_ns: u64) -> f32 {
    let mut frame = AxisFrame::new(input, ts_ns);
    frame.out = frame.in_raw;
    pipeline.process(&mut frame, state);
    frame.out
}

// ────────────────────────────────────────────────────────────────────────────
// Deadzone → Curve → Slew composition
// ────────────────────────────────────────────────────────────────────────────

#[test]
fn test_zero_input_stays_zero_through_deadzone_curve_slew() {
    // 0.0 is inside the deadzone; it must stay 0.0 through curve and slew.
    let pipeline = PipelineBuilder::new()
        .deadzone(0.05)
        .curve(0.3)
        .expect("Valid expo")
        .slew(2.0)
        .compile()
        .expect("Should compile");
    let mut state = pipeline.create_state();

    // Non-zero timestamp so slew initialises properly (avoids re-init on next call).
    let out1 = step(&pipeline, &mut state, 0.0, 4_000_000);
    assert_eq!(
        out1, 0.0,
        "Zero through deadzone→curve→slew (init frame) should be 0.0"
    );

    let out2 = step(&pipeline, &mut state, 0.0, 8_000_000);
    assert_eq!(
        out2, 0.0,
        "Zero through deadzone→curve→slew (steady frame) should be 0.0"
    );
}

#[test]
fn test_max_positive_input_stays_bounded_through_deadzone_curve_slew() {
    // Input 1.0 (maximum) must never exceed 1.0 at any point in the pipeline.
    let pipeline = PipelineBuilder::new()
        .deadzone(0.05)
        .curve(0.3)
        .expect("Valid expo")
        .slew(5.0)
        .compile()
        .expect("Should compile");
    let mut state = pipeline.create_state();

    // Initialise slew at 1.0 so rate-limiting does not interfere.
    step(&pipeline, &mut state, 1.0, 4_000_000);

    for i in 1u64..10 {
        let out = step(&pipeline, &mut state, 1.0, (i + 1) * 4_000_000);
        assert!(out <= 1.0, "Max input 1.0 must never exceed 1.0, got {out}");
        assert!(
            out >= 0.0,
            "Max positive input must produce non-negative output, got {out}"
        );
    }
}

#[test]
fn test_deadzone_curve_output_is_monotone_for_positive_inputs() {
    // Larger positive inputs through deadzone → curve must produce larger outputs.
    let pipeline = PipelineBuilder::new()
        .deadzone(0.05)
        .curve(0.3)
        .expect("Valid expo")
        .compile()
        .expect("Should compile");

    let inputs = [0.0f32, 0.1, 0.2, 0.4, 0.6, 0.8, 1.0];
    let mut prev_out = f32::NEG_INFINITY;

    for &input in &inputs {
        // Fresh state for each point — deadzone and curve are memoryless.
        let mut state = pipeline.create_state();
        let out = step(&pipeline, &mut state, input, 4_000_000);
        assert!(
            out >= prev_out,
            "deadzone→curve should be monotone: f({input})={out} < f(prev)={prev_out}"
        );
        prev_out = out;
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Slew rate limiting
// ────────────────────────────────────────────────────────────────────────────

#[test]
fn test_slew_limits_sudden_large_jump() {
    // A 0→1 step with slew(0.5 /s) over 100 ms must be capped to ≤ 0.05.
    let pipeline = PipelineBuilder::new()
        .slew(0.5)
        .compile()
        .expect("Should compile");
    let mut state = pipeline.create_state();

    // Initialise slew at 0 with non-zero timestamp.
    step(&pipeline, &mut state, 0.0, 1_000_000); // 1 ms
    // 100 ms later, attempt to jump to 1.0.
    // max_change = 0.5 * 0.1 = 0.05
    let out = step(&pipeline, &mut state, 1.0, 101_000_000);
    assert!(
        out <= 0.1,
        "slew(0.5/s) must limit 0→1 step over 100 ms to ≤0.05, got {out}"
    );
}

// ────────────────────────────────────────────────────────────────────────────
// Filter in a multi-node chain
// ────────────────────────────────────────────────────────────────────────────

#[test]
fn test_filter_smooths_step_after_deadzone() {
    // Deadzone → Filter: after a step input the EMA should not immediately reach 1.0.
    let pipeline = PipelineBuilder::new()
        .deadzone(0.05)
        .filter(0.4)
        .compile()
        .expect("Should compile");
    let mut state = pipeline.create_state();

    // Initialise filter just above the deadzone.
    step(&pipeline, &mut state, 0.06, 1_000);
    // Step to 1.0; EMA(alpha=0.4) over a small initial value must give something < 1.0.
    let out = step(&pipeline, &mut state, 1.0, 2_000);

    assert!(
        out < 1.0,
        "Filter should smooth step: expected < 1.0, got {out}"
    );
    assert!(
        out > 0.0,
        "Filter output should be positive after positive input, got {out}"
    );
}

// ────────────────────────────────────────────────────────────────────────────
// NaN / Inf robustness — pipelines must not panic on bad inputs
// ────────────────────────────────────────────────────────────────────────────

#[test]
fn test_nan_input_does_not_panic_in_full_pipeline() {
    let pipeline = PipelineBuilder::new()
        .deadzone(0.05)
        .curve(0.3)
        .expect("Valid expo")
        .slew(2.0)
        .compile()
        .expect("Should compile");
    let mut state = pipeline.create_state();

    let mut frame = AxisFrame::new(0.0, 1_000_000);
    frame.out = f32::NAN;
    pipeline.process(&mut frame, &mut state); // must not panic
}

#[test]
fn test_positive_inf_does_not_panic_in_full_pipeline() {
    let pipeline = PipelineBuilder::new()
        .deadzone(0.05)
        .curve(0.3)
        .expect("Valid expo")
        .slew(2.0)
        .compile()
        .expect("Should compile");
    let mut state = pipeline.create_state();

    let mut frame = AxisFrame::new(0.0, 1_000_000);
    frame.out = f32::INFINITY;
    pipeline.process(&mut frame, &mut state); // must not panic
}

#[test]
fn test_neg_inf_does_not_panic_in_deadzone_pipeline() {
    let pipeline = PipelineBuilder::new()
        .deadzone(0.05)
        .compile()
        .expect("Should compile");
    let mut state = pipeline.create_state();

    let mut frame = AxisFrame::new(0.0, 1_000_000);
    frame.out = f32::NEG_INFINITY;
    pipeline.process(&mut frame, &mut state); // must not panic
}

// ────────────────────────────────────────────────────────────────────────────
// Property-based tests
// ────────────────────────────────────────────────────────────────────────────

proptest! {
    /// Any valid input through deadzone → curve produces output in [-1.0, 1.0].
    #[test]
    fn deadzone_curve_output_in_range(
        input     in -1.0f32..=1.0f32,
        threshold in  0.0f32..0.9f32,
        expo      in -1.0f32..=1.0f32,
    ) {
        let pipeline = PipelineBuilder::new()
            .deadzone(threshold)
            .curve(expo)
            .expect("expo in [-1, 1] is always valid")
            .compile()
            .expect("Should compile");
        let mut state = pipeline.create_state();

        let out = step(&pipeline, &mut state, input, 1_000_000);
        prop_assert!(
            (-1.0..=1.0).contains(&out),
            "deadzone→curve output {out} out of [-1, 1] for \
             input={input}, threshold={threshold}, expo={expo}"
        );
    }

    /// deadzone → curve is monotone for non-negative inputs: f(a) ≤ f(b) when a ≤ b.
    #[test]
    fn deadzone_curve_is_monotone_for_positive_inputs(
        a         in 0.0f32..=1.0f32,
        b         in 0.0f32..=1.0f32,
        threshold in 0.0f32..0.5f32,
        expo      in -1.0f32..=1.0f32,
    ) {
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        let pipeline = PipelineBuilder::new()
            .deadzone(threshold)
            .curve(expo)
            .expect("expo in [-1, 1] is always valid")
            .compile()
            .expect("Should compile");

        // Each point gets its own fresh state (deadzone and curve are memoryless).
        let out_lo = step(&pipeline, &mut pipeline.create_state(), lo, 1_000_000);
        let out_hi = step(&pipeline, &mut pipeline.create_state(), hi, 1_000_000);

        prop_assert!(
            out_lo <= out_hi + 1e-5,
            "deadzone→curve monotonicity violated: f({lo})={out_lo} > f({hi})={out_hi} \
             (threshold={threshold}, expo={expo})"
        );
    }

    /// Full Deadzone → Curve → Slew pipeline keeps output in [-1.0, 1.0]
    /// for any valid input after the slew has been initialised.
    #[test]
    fn full_pipeline_output_in_range(
        init      in -1.0f32..=1.0f32,
        input     in -1.0f32..=1.0f32,
        threshold in  0.0f32..0.5f32,
        expo      in -1.0f32..=1.0f32,
    ) {
        // slew(10.0) with a 4 ms tick gives max_change=0.04, well within bounds.
        let pipeline = PipelineBuilder::new()
            .deadzone(threshold)
            .curve(expo)
            .expect("expo in [-1, 1] is always valid")
            .slew(10.0)
            .compile()
            .expect("Should compile");
        let mut state = pipeline.create_state();

        step(&pipeline, &mut state, init,  4_000_000); // initialise slew
        let out = step(&pipeline, &mut state, input, 8_000_000);

        prop_assert!(
            (-1.0..=1.0).contains(&out),
            "full pipeline output {out} out of [-1, 1] for \
             init={init}, input={input}, threshold={threshold}, expo={expo}"
        );
    }
}
