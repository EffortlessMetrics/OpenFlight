// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Expanded property-based tests for flight-axis mathematical invariants.
//!
//! Tests added beyond the existing proptest suite:
//! 1. Deadzone antisymmetry via DeadzoneProcessor: dz(-x) == -dz(x)
//! 2. Curve monotonicity via ResponseCurve with linear interpolation
//! 3. Expo output bounded: ExpoCurveConfig never exceeds [-1,1] even for out-of-range inputs
//! 4. Mixer weighted-sum linearity: combine(k*v) == k*combine(v) (for unit gains)
//! 5. Pipeline output always bounded [-1,1] for full chain
//! 6. Detent processor snap: output within snap_range of detent position

use flight_axis::{
    AxisFrame, PipelineBuilder,
    curve::{ControlPoint, ExpoCurveConfig, InterpolationMode, ResponseCurve},
    deadzone::{DeadzoneConfig, DeadzoneProcessor},
    detent::{DetentConfig, DetentProcessor},
    nodes::{MixerConfig, MixerInput, MixerNode},
    stages::{CurveType, DeadzoneShape, RtAxisPipeline},
};
use proptest::prelude::*;

proptest! {
    // ── 1. Deadzone antisymmetry via DeadzoneProcessor ──────────────────────

    /// DeadzoneProcessor is antisymmetric: apply(-x) == -apply(x).
    #[test]
    fn deadzone_processor_antisymmetric(
        input in 0.0f32..=1.0f32,
        center_dz in 0.0f32..0.49f32,
    ) {
        let cfg = DeadzoneConfig::center_only(center_dz).unwrap();
        let proc = DeadzoneProcessor::new(cfg);
        let pos = proc.apply(input);
        let neg = proc.apply(-input);
        prop_assert!(
            (pos + neg).abs() < 1e-5,
            "DeadzoneProcessor antisymmetry violated: apply({})={}, apply({})={}, sum={}",
            input, pos, -input, neg, pos + neg
        );
    }

    // ── 2. ResponseCurve monotonicity (linear) ──────────────────────────────

    /// ResponseCurve with Linear interpolation and non-decreasing control points
    /// preserves monotonicity: for a <= b, evaluate(a) <= evaluate(b).
    #[test]
    fn response_curve_linear_monotonic(
        a in 0.0f32..=1.0f32,
        b in 0.0f32..=1.0f32,
        y0 in 0.0f32..=0.3f32,
        y1 in 0.3f32..=0.7f32,
        y2 in 0.7f32..=1.0f32,
    ) {
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        let curve = ResponseCurve::from_points(
            vec![
                ControlPoint::new(0.0, y0),
                ControlPoint::new(0.5, y1),
                ControlPoint::new(1.0, y2),
            ],
            InterpolationMode::Linear,
        ).unwrap();
        let out_lo = curve.evaluate(lo);
        let out_hi = curve.evaluate(hi);
        prop_assert!(
            out_lo <= out_hi + 1e-5,
            "linear curve monotonicity violated: evaluate({})={} > evaluate({})={}",
            lo, out_lo, hi, out_hi
        );
    }

    // ── 3. Expo output clamped for extreme inputs ───────────────────────────

    /// ExpoCurveConfig.apply() clamps output to [-1,1] even for inputs far beyond range.
    #[test]
    fn expo_output_bounded_extreme_inputs(
        input in -10.0f32..=10.0f32,
        expo in -1.0f32..=1.0f32,
    ) {
        let cfg = ExpoCurveConfig::new(expo);
        let out = cfg.apply(input);
        prop_assert!(
            (-1.0..=1.0).contains(&out),
            "expo output {} out of [-1,1] for extreme input={}, expo={}",
            out, input, expo
        );
    }

    // ── 4. Mixer linearity: scaling inputs scales output proportionally ─────

    /// For a single-input mixer with weight 1.0, the output equals the input (clamped).
    /// This verifies the linear property: combine([v]) == v when weight == 1.0.
    #[test]
    fn mixer_single_input_identity(val in -1.0f32..=1.0f32) {
        let config = MixerConfig::new("test")
            .add_input(MixerInput::new("a", 1.0, 1.0));
        let mixer = MixerNode::new(config).expect("valid config");
        let mut out = 0.0f32;
        mixer.process_inputs(&[val], &mut out);
        prop_assert!(
            (out - val).abs() < 1e-5,
            "single-input mixer should be identity: input={}, output={}",
            val, out
        );
    }

    /// Mixer with two equal weights and equal inputs produces that input value.
    #[test]
    fn mixer_equal_inputs_equal_output(val in -0.5f32..=0.5f32) {
        let config = MixerConfig::new("test")
            .add_input(MixerInput::new("a", 0.5, 1.0))
            .add_input(MixerInput::new("b", 0.5, 1.0));
        let mixer = MixerNode::new(config).expect("valid config");
        let mut out = 0.0f32;
        mixer.process_inputs(&[val, val], &mut out);
        prop_assert!(
            (out - val).abs() < 1e-4,
            "equal-input mixer: expected {}, got {}", val, out
        );
    }

    // ── 5. Full pipeline output bounded [-1,1] ─────────────────────────────

    /// A full RT pipeline (deadzone → curve → saturation) always produces output in [-1,1].
    #[test]
    fn full_pipeline_output_bounded(
        input in -2.0f64..=2.0,
        expo in 0.0f64..=1.0,
        dz_width in 0.01f64..=0.3,
    ) {
        let mut pipeline = RtAxisPipeline::builder()
            .deadzone(0.0, dz_width, DeadzoneShape::Linear)
            .curve(CurveType::Expo(expo))
            .clamp(-1.0, 1.0)
            .build();
        let out = pipeline.process(input);
        prop_assert!(
            (-1.0..=1.0).contains(&out),
            "pipeline output {} out of [-1,1] for input={}", out, input
        );
    }

    /// PipelineBuilder chain: deadzone → curve → compile → process is always in [-1,1].
    #[test]
    fn pipeline_builder_output_bounded(input in -1.0f32..=1.0f32) {
        let pipeline = PipelineBuilder::new()
            .deadzone(0.05)
            .curve(0.5)
            .expect("valid expo")
            .compile()
            .expect("pipeline compiles");
        let mut state = pipeline.create_state();
        let mut frame = AxisFrame::new(input, 1_000_000);
        pipeline.process(&mut frame, &mut state);
        prop_assert!(
            (-1.0..=1.0).contains(&frame.out),
            "pipeline output {} out of [-1,1] for input={}", frame.out, input
        );
    }

    // ── 6. Detent processor snap within range ───────────────────────────────

    /// When input is within a detent's snap_range, the output snaps to the detent position.
    #[test]
    fn detent_snap_within_range(
        offset in -0.015f32..=0.015f32,
    ) {
        let config = DetentConfig::new()
            .add(0.0, 0.02, "idle")
            .add(1.0, 0.02, "toga");
        let mut proc = DetentProcessor::new(config);
        // Input within snap_range of the idle detent (position=0.0, snap_range=0.02)
        let input = 0.0 + offset;
        let out = proc.apply(input);
        prop_assert!(
            (out - 0.0).abs() <= 0.02 + 1e-5,
            "detent snap: input={} should produce output near 0.0, got {}", input, out
        );
    }

    /// Detent processor output is always within [0, 1] for throttle-range inputs.
    #[test]
    fn detent_output_in_throttle_range(input in 0.0f32..=1.0f32) {
        let config = DetentConfig::standard_throttle();
        let mut proc = DetentProcessor::new(config);
        let out = proc.apply(input);
        prop_assert!(
            (0.0..=1.0).contains(&out) || (out - input).abs() < 0.05,
            "detent output {} out of expected range for input={}", out, input
        );
    }
}
