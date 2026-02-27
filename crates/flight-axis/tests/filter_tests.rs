// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Tests for the EMA (exponential moving average) filter node.
//!
//! The filter node formula: S_t = alpha * Y_t + (1 - alpha) * S_{t-1}
//!
//! - First sample initialises the filter state (pass-through, no smoothing yet).
//! - Subsequent samples apply EMA smoothing.
//! - Spike rejection (when configured) ignores large transient jumps.
//!
//! Note: `FilterNode::step()` is intentionally unimplemented (panics); all tests
//! exercise the node through a compiled `Pipeline` using `step_soa`.

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
// Initialisation and basic EMA behaviour
// ────────────────────────────────────────────────────────────────────────────

#[test]
fn test_filter_first_sample_passthrough() {
    // The first frame initialises the filter; output must equal input exactly.
    let pipeline = PipelineBuilder::new()
        .filter(0.5)
        .compile()
        .expect("Should compile");
    let mut state = pipeline.create_state();

    let out = step(&pipeline, &mut state, 0.75, 1_000);
    assert_eq!(out, 0.75, "First frame should pass through unchanged");
}

#[test]
fn test_filter_ema_smoothing_on_step_input() {
    // Step from 1.0 → 0.0: EMA(alpha=0.5) should give 0.5 on the second frame.
    let pipeline = PipelineBuilder::new()
        .filter(0.5)
        .compile()
        .expect("Should compile");
    let mut state = pipeline.create_state();

    step(&pipeline, &mut state, 1.0, 1_000); // initialise at 1.0
    let out = step(&pipeline, &mut state, 0.0, 2_000);
    // EMA: 0.5 * 0.0 + 0.5 * 1.0 = 0.5
    assert!(
        (out - 0.5).abs() < 1e-5,
        "EMA(alpha=0.5): step 1→0 should give 0.5, got {out}"
    );
}

#[test]
fn test_filter_converges_toward_constant_input() {
    // After many frames at the same value the filter must converge to that value.
    let pipeline = PipelineBuilder::new()
        .filter(0.3)
        .compile()
        .expect("Should compile");
    let mut state = pipeline.create_state();

    // Initialise at 0.0, then drive with 1.0 for many frames.
    step(&pipeline, &mut state, 0.0, 1_000);
    let mut out = 0.0f32;
    for i in 1u64..200 {
        out = step(&pipeline, &mut state, 1.0, i * 1_000 + 1_000);
    }
    assert!(
        out > 0.999,
        "Filter should converge to 1.0 after many samples, got {out}"
    );
}

// ────────────────────────────────────────────────────────────────────────────
// Alpha boundary behaviour
// ────────────────────────────────────────────────────────────────────────────

#[test]
fn test_filter_alpha_one_is_identity() {
    // alpha=1.0 → S_t = Y_t.  After initialisation the output tracks input exactly.
    let pipeline = PipelineBuilder::new()
        .filter(1.0)
        .compile()
        .expect("Should compile");
    let mut state = pipeline.create_state();

    step(&pipeline, &mut state, 0.0, 1_000); // initialise
    let out = step(&pipeline, &mut state, 0.7, 2_000);
    assert!(
        (out - 0.7).abs() < 1e-5,
        "alpha=1.0 should be identity, expected 0.7 got {out}"
    );
}

#[test]
fn test_filter_alpha_zero_holds_initial_value() {
    // alpha=0.0 → S_t = S_{t-1}.  Output is frozen at the initialisation value.
    let pipeline = PipelineBuilder::new()
        .filter(0.0)
        .compile()
        .expect("Should compile");
    let mut state = pipeline.create_state();

    step(&pipeline, &mut state, 0.3, 1_000); // initialise at 0.3
    let out = step(&pipeline, &mut state, 1.0, 2_000);
    // EMA: 0.0 * 1.0 + 1.0 * 0.3 = 0.3
    assert!(
        (out - 0.3).abs() < 1e-5,
        "alpha=0.0 should hold initial value 0.3, got {out}"
    );
}

// ────────────────────────────────────────────────────────────────────────────
// Output bounds
// ────────────────────────────────────────────────────────────────────────────

#[test]
fn test_filter_output_bounded_for_valid_inputs() {
    let pipeline = PipelineBuilder::new()
        .filter(0.2)
        .compile()
        .expect("Should compile");
    let mut state = pipeline.create_state();

    let inputs = [-1.0f32, -0.7, -0.3, 0.0, 0.3, 0.7, 1.0];
    for (i, &input) in inputs.iter().enumerate() {
        let out = step(&pipeline, &mut state, input, (i as u64 + 1) * 1_000);
        assert!(
            out >= -1.0 && out <= 1.0,
            "Filter output {out} out of [-1, 1] for input {input}"
        );
    }
}

// ────────────────────────────────────────────────────────────────────────────
// Spike rejection (B104 preset)
// ────────────────────────────────────────────────────────────────────────────

#[test]
fn test_b104_spike_rejection_ignores_single_transient() {
    // The B104 preset (alpha=0.15, spike_threshold=0.4, max_spike_count=5) should
    // reject a single large spike and keep the output near the stable value.
    let pipeline = PipelineBuilder::new()
        .b104_filter()
        .compile()
        .expect("Should compile");
    let mut state = pipeline.create_state();

    // Initialise and stabilise at 0.1.
    step(&pipeline, &mut state, 0.1, 1_000);
    for i in 1u64..10 {
        step(&pipeline, &mut state, 0.1, i * 1_000 + 1_000);
    }
    let stable = step(&pipeline, &mut state, 0.1, 12_000);

    // Single large spike (delta=0.8 > threshold=0.4).
    let spiked = step(&pipeline, &mut state, 0.9, 13_000);

    assert!(
        (spiked - stable).abs() < 0.01,
        "B104 spike rejection should ignore transient: stable={stable}, after spike={spiked}"
    );
}

// ────────────────────────────────────────────────────────────────────────────
// Robustness: NaN / Inf must not cause panics
// ────────────────────────────────────────────────────────────────────────────

#[test]
fn test_filter_nan_does_not_panic() {
    let pipeline = PipelineBuilder::new()
        .filter(0.3)
        .compile()
        .expect("Should compile");
    let mut state = pipeline.create_state();

    let mut frame = AxisFrame::new(0.0, 1_000);
    frame.out = f32::NAN;
    pipeline.process(&mut frame, &mut state); // must not panic
}

#[test]
fn test_filter_inf_does_not_panic() {
    let pipeline = PipelineBuilder::new()
        .filter(0.3)
        .compile()
        .expect("Should compile");
    let mut state = pipeline.create_state();

    let mut frame = AxisFrame::new(0.0, 1_000);
    frame.out = f32::INFINITY;
    pipeline.process(&mut frame, &mut state); // must not panic
}

// ────────────────────────────────────────────────────────────────────────────
// Property-based tests
// ────────────────────────────────────────────────────────────────────────────

proptest! {
    /// Filter output is always in [-1.0, 1.0] for any valid input sequence.
    #[test]
    fn filter_output_in_range(
        init  in -1.0f32..=1.0f32,
        input in -1.0f32..=1.0f32,
        alpha in  0.0f32..=1.0f32,
    ) {
        let pipeline = PipelineBuilder::new()
            .filter(alpha)
            .compile()
            .expect("Should compile");
        let mut state = pipeline.create_state();

        step(&pipeline, &mut state, init, 1_000); // initialise filter
        let out = step(&pipeline, &mut state, input, 2_000);

        prop_assert!(
            out >= -1.0 && out <= 1.0,
            "filter output {out} out of [-1, 1] for init={init}, input={input}, alpha={alpha}"
        );
    }
}
