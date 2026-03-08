// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Property-based tests for mixer and pipeline determinism invariants.
//!
//! - Mixer WeightedSum output always in [-1.0, 1.0]
//! - Mixer Max returns a value present among finite inputs
//! - Mixer Min returns a value present among finite inputs
//! - Mixer Priority returns value from the highest-weight input
//! - Pipeline is deterministic: same input always gives same output
//! - Deadzone symmetry: apply(-x) == -apply(x)

use flight_axis::{
    AxisFrame, PipelineBuilder,
    mixer::{AxisMixer, MixMode},
    deadzone::{DeadzoneConfig, DeadzoneProcessor},
    curve::ExpoCurveConfig,
};
use proptest::prelude::*;

proptest! {
    // ── Mixer: WeightedSum always clamped ────────────────────────────────────

    /// WeightedSum output is always in [-1.0, 1.0] regardless of weights.
    #[test]
    fn mixer_weighted_sum_output_bounded(
        w0 in -5.0f64..=5.0,
        w1 in -5.0f64..=5.0,
        v0 in -1.0f64..=1.0,
        v1 in -1.0f64..=1.0,
    ) {
        let mixer = AxisMixer::with_weights(MixMode::WeightedSum, &[w0, w1]);
        let out = mixer.combine(&[v0, v1]);
        prop_assert!(
            (-1.0..=1.0).contains(&out),
            "WeightedSum output {} out of [-1, 1] for weights=[{}, {}], values=[{}, {}]",
            out, w0, w1, v0, v1
        );
    }

    // ── Mixer: Max returns value among finite inputs ────────────────────────

    /// Max mode returns a value that is one of the finite inputs.
    #[test]
    fn mixer_max_returns_input_value(
        v0 in -1.0f64..=1.0,
        v1 in -1.0f64..=1.0,
        v2 in -1.0f64..=1.0,
    ) {
        let mixer = AxisMixer::with_weights(MixMode::Max, &[1.0, 1.0, 1.0]);
        let out = mixer.combine(&[v0, v1, v2]);
        let inputs = [v0, v1, v2];
        prop_assert!(
            inputs.iter().any(|&v| (out - v).abs() < 1e-10),
            "Max output {} is not among inputs {:?}",
            out, inputs
        );
    }

    // ── Mixer: Min returns value among finite inputs ────────────────────────

    /// Min mode returns a value that is one of the finite inputs.
    #[test]
    fn mixer_min_returns_input_value(
        v0 in -1.0f64..=1.0,
        v1 in -1.0f64..=1.0,
        v2 in -1.0f64..=1.0,
    ) {
        let mixer = AxisMixer::with_weights(MixMode::Min, &[1.0, 1.0, 1.0]);
        let out = mixer.combine(&[v0, v1, v2]);
        let inputs = [v0, v1, v2];
        prop_assert!(
            inputs.iter().any(|&v| (out - v).abs() < 1e-10),
            "Min output {} is not among inputs {:?}",
            out, inputs
        );
    }

    // ── Mixer: Max >= Min ───────────────────────────────────────────────────

    /// Max mode output is always >= Min mode output for the same inputs.
    #[test]
    fn mixer_max_gte_min(
        v0 in -1.0f64..=1.0,
        v1 in -1.0f64..=1.0,
    ) {
        let max_mixer = AxisMixer::with_weights(MixMode::Max, &[1.0, 1.0]);
        let min_mixer = AxisMixer::with_weights(MixMode::Min, &[1.0, 1.0]);
        let max_out = max_mixer.combine(&[v0, v1]);
        let min_out = min_mixer.combine(&[v0, v1]);
        prop_assert!(
            max_out >= min_out - 1e-10,
            "Max {} < Min {} for values [{}, {}]",
            max_out, min_out, v0, v1
        );
    }

    // ── Mixer: Priority returns correct input ───────────────────────────────

    /// Priority mode returns the value of the input with the highest weight.
    #[test]
    fn mixer_priority_uses_highest_weight(
        v0 in -1.0f64..=1.0,
        v1 in -1.0f64..=1.0,
    ) {
        // w0 > w1, so priority should pick v0
        let mixer = AxisMixer::with_weights(MixMode::Priority, &[2.0, 1.0]);
        let out = mixer.combine(&[v0, v1]);
        prop_assert!(
            (out - v0).abs() < 1e-10,
            "Priority should pick v0={} (weight 2.0) over v1={} (weight 1.0), got {}",
            v0, v1, out
        );
    }

    // ── Pipeline: deterministic output ──────────────────────────────────────

    /// Pipeline produces identical output for identical input on repeated runs.
    #[test]
    fn pipeline_deterministic(input in -1.0f32..=1.0f32) {
        let pipeline = PipelineBuilder::new()
            .deadzone(0.05)
            .curve(0.3)
            .expect("valid expo")
            .compile()
            .expect("pipeline compiles");

        let mut state1 = pipeline.create_state();
        let mut frame1 = AxisFrame::new(input, 1_000_000);
        pipeline.process(&mut frame1, &mut state1);

        let mut state2 = pipeline.create_state();
        let mut frame2 = AxisFrame::new(input, 1_000_000);
        pipeline.process(&mut frame2, &mut state2);

        prop_assert_eq!(
            frame1.out, frame2.out,
            "pipeline not deterministic: first={}, second={} for input={}",
            frame1.out, frame2.out, input
        );
    }

    // ── Deadzone: symmetry ──────────────────────────────────────────────────

    /// Symmetric deadzone is antisymmetric: apply(-x) == -apply(x).
    #[test]
    fn deadzone_antisymmetric(
        input in -1.0f32..=1.0f32,
        center in 0.0f32..0.49f32,
    ) {
        let config = DeadzoneConfig::center_only(center).unwrap();
        let proc = DeadzoneProcessor::new(config);
        let pos = proc.apply(input);
        let neg = proc.apply(-input);
        prop_assert!(
            (pos + neg).abs() < 1e-6,
            "deadzone antisymmetry violated: apply({})={}, apply({})={}, sum={}",
            input, pos, -input, neg, pos + neg
        );
    }

    // ── Expo: monotonicity ──────────────────────────────────────────────────

    /// Expo curve is monotonic: if x > y then expo(x) >= expo(y).
    #[test]
    fn expo_monotonic(
        a in -1.0f32..=1.0f32,
        b in -1.0f32..=1.0f32,
        expo in -1.0f32..=1.0f32,
    ) {
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        let cfg = ExpoCurveConfig::new(expo);
        let out_lo = cfg.apply(lo);
        let out_hi = cfg.apply(hi);
        prop_assert!(
            out_lo <= out_hi + 1e-5,
            "expo monotonicity violated: apply({})={} > apply({})={} with expo={}",
            lo, out_lo, hi, out_hi, expo
        );
    }

    // ── Mixer: empty inputs return 0.0 ──────────────────────────────────────

    /// Mixer with no inputs always returns 0.0.
    #[test]
    fn mixer_empty_returns_zero(mode_idx in 0u8..4u8) {
        let mode = match mode_idx {
            0 => MixMode::WeightedSum,
            1 => MixMode::Max,
            2 => MixMode::Min,
            _ => MixMode::Priority,
        };
        let mixer = AxisMixer::new(mode);
        let out = mixer.combine(&[]);
        prop_assert_eq!(out, 0.0, "empty mixer should return 0.0, got {}", out);
    }
}
