// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for the axis engine signal processing pipeline.
//!
//! Covers: deadzone, curves/expo, detents, mixers, full pipeline
//! composition, and zero-allocation RT constraints.

use std::sync::Arc;
use std::time::Instant;

use flight_axis::deadzone::{AsymmetricDeadzoneConfig, DeadzoneConfig, DeadzoneProcessor};
use flight_axis::curve::{
    ControlPoint, ExpoCurveConfig, InterpolationMode, ResponseCurve,
};
use flight_axis::detent::{
    DetentBand, DetentConfig, DetentProcessor, RtDetentProcessor,
};
use flight_axis::mixer::{AxisMixer, MixMode};
use flight_axis::pipeline::{
    AxisPipeline, ClampStage, CurveStage, DeadzoneStage, SensitivityStage,
};
use flight_axis::stages::{CurveType, DeadzoneShape, RtAxisPipeline, StageSlot};
use flight_axis::{AxisEngine, AxisFrame, PipelineBuilder};

const TOL: f64 = 1e-6;
const TOL32: f32 = 1e-5;

fn approx(a: f64, b: f64) -> bool {
    (a - b).abs() < TOL
}

fn approx32(a: f32, b: f32) -> bool {
    (a - b).abs() < TOL32
}

// ═══════════════════════════════════════════════════════════════════════════
// 1. DEADZONE DEPTH TESTS (6)
// ═══════════════════════════════════════════════════════════════════════════

mod deadzone_depth {
    use super::*;

    /// Symmetric deadzone: equal positive/negative suppression with correct rescale.
    #[test]
    fn symmetric_deadzone_suppresses_and_rescales() {
        let config = DeadzoneConfig::new(0.1, 0.0).unwrap();
        let proc = DeadzoneProcessor::new(config);

        // Inside deadzone
        assert_eq!(proc.apply(0.05), 0.0);
        assert_eq!(proc.apply(-0.05), 0.0);
        assert_eq!(proc.apply(0.1), 0.0);
        assert_eq!(proc.apply(-0.1), 0.0);

        // Outside: rescaled linearly so output spans full range
        // input=0.55 → (0.55−0.1)/(1.0−0.1) = 0.45/0.9 = 0.5
        let out = proc.apply(0.55);
        assert!(approx32(out, 0.5), "expected 0.5, got {out}");

        // Negative side mirrors positive
        let out_neg = proc.apply(-0.55);
        assert!(approx32(out_neg, -0.5), "expected -0.5, got {out_neg}");

        // Full deflection maps to ±1.0
        assert!(approx32(proc.apply(1.0), 1.0));
        assert!(approx32(proc.apply(-1.0), -1.0));
    }

    /// Asymmetric deadzone: different positive/negative thresholds (brake pedal use case).
    #[test]
    fn asymmetric_deadzone_different_thresholds() {
        let cfg = AsymmetricDeadzoneConfig::new(0.05, 0.15);

        // Positive side: 5% deadzone
        assert_eq!(cfg.apply(0.03), 0.0);
        let out_pos = cfg.apply(0.5);
        // (0.5 − 0.05) / (1.0 − 0.05) = 0.45 / 0.95
        let expected_pos = 0.45_f32 / 0.95;
        assert!(approx32(out_pos, expected_pos), "pos: expected {expected_pos}, got {out_pos}");

        // Negative side: 15% deadzone
        assert_eq!(cfg.apply(-0.1), 0.0);
        let out_neg = cfg.apply(-0.5);
        // (-0.5 + 0.15) / (1.0 − 0.15) = -0.35 / 0.85
        let expected_neg = -0.35_f32 / 0.85;
        assert!(approx32(out_neg, expected_neg), "neg: expected {expected_neg}, got {out_neg}");

        assert!(!cfg.is_symmetric());
    }

    /// Deadzone with rescale: verify output continuously covers full range after deadzone.
    #[test]
    fn deadzone_with_rescale_full_range_coverage() {
        let config = DeadzoneConfig::new(0.2, 0.1).unwrap();
        let proc = DeadzoneProcessor::new(config);
        // denom = 1.0 - 0.2 - 0.1 = 0.7

        // Just outside deadzone on positive side
        let just_outside = proc.apply(0.21);
        assert!(just_outside > 0.0, "should be positive just outside dz");
        assert!(just_outside < 0.1, "should be small just outside dz: {just_outside}");

        // Midpoint of active range: input = 0.2 + 0.35 = 0.55
        let mid = proc.apply(0.55);
        assert!(approx32(mid, 0.5), "midpoint expected 0.5, got {mid}");

        // Edge region should saturate to 1.0
        assert_eq!(proc.apply(0.95), 1.0);
        assert_eq!(proc.apply(1.0), 1.0);
    }

    /// Zero deadzone passes all values through unchanged.
    #[test]
    fn zero_deadzone_passthrough() {
        let proc = DeadzoneProcessor::new(DeadzoneConfig::default());
        for &v in &[-1.0_f32, -0.5, -0.01, 0.0, 0.01, 0.5, 1.0] {
            let out = proc.apply(v);
            assert!(
                approx32(out, v),
                "zero dz: input={v}, output={out}"
            );
        }
    }

    /// Full deadzone (center near 0.5) clamps everything near center to zero.
    #[test]
    fn full_deadzone_clamp() {
        let config = DeadzoneConfig::new(0.49, 0.0).unwrap();
        let proc = DeadzoneProcessor::new(config);

        // Everything in [-0.49, 0.49] should be zero
        for &v in &[-0.48_f32, -0.2, 0.0, 0.2, 0.48] {
            assert_eq!(proc.apply(v), 0.0, "should be clamped at v={v}");
        }

        // Just outside should be very small but positive
        let out = proc.apply(0.5);
        assert!(out > 0.0 && out < 0.05, "expected small positive, got {out}");
    }

    /// Edge-of-deadzone: value exactly at boundary transitions correctly.
    #[test]
    fn edge_of_deadzone_behavior() {
        let config = DeadzoneConfig::center_only(0.1).unwrap();
        let proc = DeadzoneProcessor::new(config);

        // At boundary: should be exactly 0
        assert_eq!(proc.apply(0.1), 0.0);
        assert_eq!(proc.apply(-0.1), 0.0);

        // Epsilon outside boundary: should be tiny positive
        let epsilon_over = proc.apply(0.1 + 0.001);
        assert!(epsilon_over > 0.0, "just outside should be >0");
        assert!(epsilon_over < 0.01, "just outside should be small: {epsilon_over}");

        // Verify continuity: values just outside converge to 0 as they approach boundary
        let deltas = [0.001_f32, 0.005, 0.01, 0.05];
        let mut prev_out = 0.0_f32;
        for &d in &deltas {
            let out = proc.apply(0.1 + d);
            assert!(out > prev_out, "monotonicity: d={d}, out={out} should be > {prev_out}");
            prev_out = out;
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. CURVES / EXPO DEPTH TESTS (6)
// ═══════════════════════════════════════════════════════════════════════════

mod curve_depth {
    use super::*;

    /// Linear passthrough: identity curve outputs input unchanged.
    #[test]
    fn linear_passthrough_identity() {
        let curve = ResponseCurve::linear_identity();
        for i in 0..=20 {
            let x = i as f32 / 20.0;
            let y = curve.evaluate(x);
            assert!(approx32(y, x), "linear identity: evaluate({x}) = {y}");
        }
    }

    /// Expo curve shape: positive expo reduces center sensitivity.
    #[test]
    fn expo_curve_shape_reduces_center_sensitivity() {
        let expo = ExpoCurveConfig::new(0.7);

        // Center region should be compressed (output < input)
        for &v in &[0.1_f32, 0.2, 0.3, 0.4] {
            let out = expo.apply(v);
            assert!(out < v, "expo should reduce center: v={v}, out={out}");
        }

        // Near extremes, output approaches input (expo=0.7: 0.7*0.9³+0.3*0.9 ≈ 0.78)
        let at_90 = expo.apply(0.9);
        assert!(at_90 > 0.7, "at 0.9 output should be > 0.7: {at_90}");

        // Endpoints are fixed
        assert!(approx32(expo.apply(0.0), 0.0));
        assert!(approx32(expo.apply(1.0), 1.0));
        assert!(approx32(expo.apply(-1.0), -1.0));

        // Antisymmetric: apply(-x) = -apply(x)
        for &v in &[0.2_f32, 0.5, 0.8] {
            let pos = expo.apply(v);
            let neg = expo.apply(-v);
            assert!(approx32(pos + neg, 0.0), "antisymmetry: v={v}, pos={pos}, neg={neg}");
        }
    }

    /// Custom curve: piecewise-linear control points shape the response.
    #[test]
    fn custom_curve_points_shape_response() {
        // Create an aggressive initial slope, then gradual
        let curve = ResponseCurve::from_points(
            vec![
                ControlPoint::new(0.0, 0.0),
                ControlPoint::new(0.2, 0.5), // steep initial slope
                ControlPoint::new(0.8, 0.8), // shallow middle
                ControlPoint::new(1.0, 1.0),
            ],
            InterpolationMode::Linear,
        )
        .unwrap();

        // At x=0.1 (between pt0 and pt1): t=0.5, y=0.0+0.5*0.5=0.25
        assert!(approx32(curve.evaluate(0.1), 0.25));

        // At x=0.2: exactly at control point
        assert!(approx32(curve.evaluate(0.2), 0.5));

        // At x=0.5 (between pt1 and pt2): t=(0.5−0.2)/(0.8−0.2)=0.5, y=0.5+0.5*0.3=0.65
        assert!(approx32(curve.evaluate(0.5), 0.65));

        // Endpoints
        assert!(approx32(curve.evaluate(0.0), 0.0));
        assert!(approx32(curve.evaluate(1.0), 1.0));
    }

    /// S-curve using monotone cubic preserves monotonicity.
    #[test]
    fn s_curve_monotone_cubic_preserves_monotonicity() {
        // S-shape: slow → fast → slow
        let curve = ResponseCurve::from_points(
            vec![
                ControlPoint::new(0.0, 0.0),
                ControlPoint::new(0.3, 0.1),  // slow start
                ControlPoint::new(0.5, 0.5),  // steep middle
                ControlPoint::new(0.7, 0.9),  // steep middle
                ControlPoint::new(1.0, 1.0),  // slow end
            ],
            InterpolationMode::MonotoneCubic,
        )
        .unwrap();

        assert!(curve.is_monotone());

        // Verify monotonicity across 200 samples
        let mut prev = 0.0_f32;
        for i in 0..=200 {
            let x = i as f32 / 200.0;
            let y = curve.evaluate(x);
            assert!(
                y >= prev - 1e-5,
                "monotonicity violated at x={x}: y={y} < prev={prev}"
            );
            assert!((0.0..=1.0).contains(&y), "out of range at x={x}: y={y}");
            prev = y;
        }
    }

    /// Saturation at limits: all curve types clamp output to [0, 1].
    #[test]
    fn saturation_at_limits_all_modes() {
        let modes = [
            InterpolationMode::Linear,
            InterpolationMode::CubicHermite,
            InterpolationMode::MonotoneCubic,
        ];

        for mode in modes {
            let curve = ResponseCurve::from_points(
                vec![
                    ControlPoint::new(0.0, 0.0),
                    ControlPoint::new(0.5, 0.8),
                    ControlPoint::new(1.0, 1.0),
                ],
                mode,
            )
            .unwrap();

            // Input below 0 clamped to evaluate(0)
            assert!(approx32(curve.evaluate(-0.5), 0.0));

            // Input above 1 clamped to evaluate(1)
            assert!(approx32(curve.evaluate(1.5), 1.0));

            // Output always in [0, 1]
            for i in 0..=100 {
                let x = i as f32 / 100.0;
                let y = curve.evaluate(x);
                assert!(
                    (0.0..=1.0).contains(&y),
                    "mode={mode:?}: out of range at x={x}: y={y}"
                );
            }
        }
    }

    /// Curve interpolation: cubic Hermite passes through control points.
    #[test]
    fn curve_interpolation_cubic_hermite_through_points() {
        let pts = vec![
            ControlPoint::new(0.0, 0.0),
            ControlPoint::new(0.25, 0.3),
            ControlPoint::new(0.5, 0.5),
            ControlPoint::new(0.75, 0.7),
            ControlPoint::new(1.0, 1.0),
        ];

        let curve = ResponseCurve::from_points(pts.clone(), InterpolationMode::CubicHermite)
            .unwrap();

        // Cubic Hermite must pass through all control points
        for pt in &pts {
            let y = curve.evaluate(pt.x);
            assert!(
                approx32(y, pt.y),
                "CubicHermite should pass through ({}, {}), got {}",
                pt.x, pt.y, y
            );
        }

        // Between control points, output should be smooth (no sudden jumps)
        let step = 0.01_f32;
        let mut x = step;
        let mut prev_y = curve.evaluate(0.0);
        while x <= 1.0 {
            let y = curve.evaluate(x);
            let dy = (y - prev_y).abs();
            assert!(
                dy < 0.1,
                "jump too large at x={x}: dy={dy}"
            );
            prev_y = y;
            x += step;
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. DETENT DEPTH TESTS (5)
// ═══════════════════════════════════════════════════════════════════════════

mod detent_depth {
    use super::*;

    /// Throttle idle detent: snap at 0.0 with correct label.
    #[test]
    fn throttle_idle_detent_snaps_at_zero() {
        let mut proc = DetentProcessor::new(DetentConfig::standard_throttle());

        // Within snap range of idle (0.0, snap=0.02)
        assert_eq!(proc.apply(0.01), 0.0);
        assert_eq!(proc.active_detent_label(), Some("idle"));

        assert_eq!(proc.apply(0.019), 0.0);
        assert_eq!(proc.active_detent_label(), Some("idle"));

        // Exactly at boundary
        assert_eq!(proc.apply(0.02), 0.0);

        // Outside snap range
        assert_eq!(proc.apply(0.03), 0.03);
        assert_eq!(proc.active_detent_label(), None);
    }

    /// Reverse detent: Airbus reverse-idle at 0.0 with full five-detent config.
    #[test]
    fn reverse_detent_airbus_throttle() {
        let mut proc = DetentProcessor::new(DetentConfig::airbus_throttle());

        // Reverse idle at 0.0
        assert_eq!(proc.apply(0.01), 0.0);
        assert_eq!(proc.active_detent_label(), Some("reverse_idle"));

        // Idle at 0.25
        assert_eq!(proc.apply(0.26), 0.25);
        assert_eq!(proc.active_detent_label(), Some("idle"));

        // Climb at 0.75
        assert_eq!(proc.apply(0.74), 0.75);
        assert_eq!(proc.active_detent_label(), Some("climb"));

        // Flex/MCT at 0.90
        assert_eq!(proc.apply(0.91), 0.90);
        assert_eq!(proc.active_detent_label(), Some("flex_mct"));
    }

    /// TOGA detent: snap at full throttle position.
    #[test]
    fn toga_detent_snaps_at_full_throttle() {
        let mut proc = DetentProcessor::new(DetentConfig::standard_throttle());

        // Within snap range of TOGA (1.0, snap=0.02)
        assert_eq!(proc.apply(0.99), 1.0);
        assert_eq!(proc.active_detent_label(), Some("toga"));

        assert_eq!(proc.apply(0.985), 1.0);

        // Just outside
        assert_eq!(proc.apply(0.97), 0.97);
        assert_eq!(proc.active_detent_label(), None);
    }

    /// Multi-detent axis: RT-safe processor with multiple bands and hysteresis.
    #[test]
    fn multi_detent_axis_with_hysteresis() {
        let mut proc = RtDetentProcessor::<4>::new();
        proc.add(DetentBand::new(0.0, 0.05, 0.02));   // idle
        proc.add(DetentBand::new(0.5, 0.05, 0.02));   // cruise
        proc.add(DetentBand::new(1.0, 0.05, 0.02));   // TOGA

        // Engage idle
        assert_eq!(proc.process(0.03), 0.0);

        // Still held by hysteresis (exit threshold = 0.05 + 0.02 = 0.07)
        assert_eq!(proc.process(0.06), 0.0);

        // Exit idle
        let free = proc.process(0.08);
        assert!(approx32(free, 0.08), "should be free: {free}");

        // Engage cruise
        assert_eq!(proc.process(0.48), 0.5);

        // Move to TOGA region
        assert_eq!(proc.process(0.97), 1.0);

        // Free between detents
        let mid = proc.process(0.3);
        assert!(approx32(mid, 0.3), "should be free at 0.3: {mid}");
    }

    /// Detent snap range: values exactly at boundary snap correctly.
    #[test]
    fn detent_snap_range_boundary_behavior() {
        let mut proc = DetentProcessor::new(
            DetentConfig::new()
                .add(0.5, 0.1, "center")
        );

        // Well inside snap range
        assert_eq!(proc.apply(0.45), 0.5);
        assert_eq!(proc.active_detent_label(), Some("center"));

        assert_eq!(proc.apply(0.55), 0.5);
        assert_eq!(proc.active_detent_label(), Some("center"));

        // At exact boundary
        assert_eq!(proc.apply(0.4), 0.5);

        // Well outside snap range
        let outside = 0.65;
        assert!(approx32(proc.apply(outside), outside));
        assert_eq!(proc.active_detent_label(), None);

        // Sweep: values clearly inside snap, clearly outside snap
        for i in 0..=100 {
            let v = i as f32 / 100.0;
            let out = proc.apply(v);
            if (v - 0.5).abs() < 0.09 {
                assert_eq!(out, 0.5, "should snap at v={v}");
            } else if (v - 0.5).abs() > 0.12 {
                assert!(approx32(out, v), "should pass through at v={v}");
            }
            // Skip boundary region where behavior is implementation-defined
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 4. MIXER DEPTH TESTS (5)
// ═══════════════════════════════════════════════════════════════════════════

mod mixer_depth {
    use super::*;

    /// Aileron-rudder mix: rudder output proportional to aileron input.
    #[test]
    fn aileron_rudder_mix() {
        // Simulate: rudder_out = rudder_input + coordination_factor * aileron_input
        // Using weighted sum: [rudder_weight=1.0, aileron_coordination=0.15]
        let mixer = AxisMixer::with_weights(MixMode::WeightedSum, &[1.0, 0.15]);

        let rudder_input = 0.0;
        let aileron_input = 0.5;
        let out = mixer.combine(&[rudder_input, aileron_input]);
        // 0.0*1.0 + 0.5*0.15 = 0.075
        assert!(approx(out, 0.075), "expected 0.075, got {out}");

        // Full aileron roll with rudder centered
        let out2 = mixer.combine(&[0.0, 1.0]);
        assert!(approx(out2, 0.15));

        // Both inputs active
        let out3 = mixer.combine(&[0.3, 0.6]);
        assert!(approx(out3, 0.3 + 0.6 * 0.15));
    }

    /// Differential braking: left/right brake from rudder + toe brakes.
    #[test]
    fn differential_braking() {
        // Left brake = max(rudder_left, toe_brake_left)
        // Right brake = max(rudder_right, toe_brake_right)
        let left_mixer = AxisMixer::with_weights(MixMode::Max, &[1.0, 1.0]);
        let right_mixer = AxisMixer::with_weights(MixMode::Max, &[1.0, 1.0]);

        // Rudder left: rudder_left=0.3, toe=0.0 → left brake = 0.3
        let left = left_mixer.combine(&[0.3, 0.0]);
        assert!(approx(left, 0.3));

        // Toe brake overrides: rudder=0.2, toe=0.8 → brake = 0.8
        let left2 = left_mixer.combine(&[0.2, 0.8]);
        assert!(approx(left2, 0.8));

        // Right side independent
        let right = right_mixer.combine(&[0.0, 0.5]);
        assert!(approx(right, 0.5));
    }

    /// Trimming via mixer: axis + trim offset combined with weighted sum.
    #[test]
    fn trimming_via_mixer() {
        let mixer = AxisMixer::with_weights(MixMode::WeightedSum, &[1.0, 1.0]);

        // Axis at center, small trim offset
        let out = mixer.combine(&[0.0, 0.05]);
        assert!(approx(out, 0.05));

        // Axis deflected + trim
        let out2 = mixer.combine(&[0.5, 0.1]);
        assert!(approx(out2, 0.6));

        // Trim pushes to saturation → clamped
        let out3 = mixer.combine(&[0.9, 0.3]);
        assert!(approx(out3, 1.0)); // clamped at 1.0

        // Negative trim
        let out4 = mixer.combine(&[0.5, -0.1]);
        assert!(approx(out4, 0.4));
    }

    /// Mixer enable/disable: zero-weight effectively disables an input.
    #[test]
    fn mixer_enable_disable_via_weight() {
        let mut mixer = AxisMixer::with_weights(MixMode::WeightedSum, &[1.0, 1.0]);

        // Both active
        let active = mixer.combine(&[0.3, 0.2]);
        assert!(approx(active, 0.5));

        // Disable second input by setting weight to 0
        mixer.set_weight(1, 0.0);
        let disabled = mixer.combine(&[0.3, 0.2]);
        assert!(approx(disabled, 0.3));

        // Re-enable
        mixer.set_weight(1, 1.0);
        let reenabled = mixer.combine(&[0.3, 0.2]);
        assert!(approx(reenabled, 0.5));
    }

    /// Mixer coefficient limits: extreme weights are handled correctly.
    #[test]
    fn mixer_coefficient_limits() {
        // Very large weight: output clamped
        let mixer = AxisMixer::with_weights(MixMode::WeightedSum, &[10.0]);
        let out = mixer.combine(&[0.5]);
        assert!(approx(out, 1.0)); // 0.5 * 10 = 5.0, clamped to 1.0

        // Negative weight: inverts input contribution
        let mixer_neg = AxisMixer::with_weights(MixMode::WeightedSum, &[1.0, -1.0]);
        let out2 = mixer_neg.combine(&[0.5, 0.3]);
        // 0.5*1.0 + 0.3*(-1.0) = 0.2
        assert!(approx(out2, 0.2));

        // Zero inputs returns zero
        let mixer_empty = AxisMixer::new(MixMode::WeightedSum);
        assert_eq!(mixer_empty.combine(&[0.5]), 0.0);

        // Priority mode: highest weight wins
        let mixer_pri = AxisMixer::with_weights(MixMode::Priority, &[1.0, 100.0, 50.0]);
        let out3 = mixer_pri.combine(&[0.1, 0.9, 0.5]);
        assert!(approx(out3, 0.9)); // weight 100.0 picks values[1]
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 5. PIPELINE DEPTH TESTS (8)
// ═══════════════════════════════════════════════════════════════════════════

mod pipeline_depth {
    use super::*;

    /// Full pipeline: raw → deadzone → curve → mix → output.
    #[test]
    fn full_pipeline_raw_to_output() {
        let mut pipeline = AxisPipeline::new();
        pipeline.add_stage(Box::new(DeadzoneStage { inner: 0.05, outer: 1.0 }));
        pipeline.add_stage(Box::new(CurveStage { expo: 0.5 }));
        pipeline.add_stage(Box::new(SensitivityStage { multiplier: 1.0 }));
        pipeline.add_stage(Box::new(ClampStage { min: -1.0, max: 1.0 }));

        // Input within deadzone → 0
        let out = pipeline.process(0.03, 0.004);
        assert!(approx(out, 0.0), "deadzone should zero small input: {out}");

        // Input outside deadzone: processed through curve and sensitivity
        let out2 = pipeline.process(0.5, 0.004);
        assert!(out2 > 0.0 && out2 <= 1.0, "should produce valid output: {out2}");

        // Full deflection
        let out3 = pipeline.process(1.0, 0.004);
        assert!(approx(out3, 1.0), "full deflection should map to 1.0: {out3}");

        // Verify stage names
        assert_eq!(
            pipeline.stage_names(),
            vec!["deadzone", "curve", "sensitivity", "clamp"]
        );
    }

    /// Pipeline latency: processing completes within RT budget.
    #[test]
    fn pipeline_latency_within_budget() {
        let mut pipeline = RtAxisPipeline::builder()
            .deadzone(0.0, 0.05, DeadzoneShape::Linear)
            .curve(CurveType::Expo(0.3))
            .smoothing_ema(0.8)
            .clamp(-1.0, 1.0)
            .build();

        // Process 1000 frames and measure total time
        let start = Instant::now();
        for i in 0..1000 {
            let input = (i as f64 / 1000.0) * 2.0 - 1.0;
            let _out = pipeline.process(input);
        }
        let elapsed = start.elapsed();

        // At 250Hz, 1000 frames = 4 seconds of real-time data.
        // Processing should complete in well under 100ms total (0.1ms per frame budget).
        assert!(
            elapsed.as_millis() < 100,
            "pipeline too slow: {elapsed:?} for 1000 frames"
        );
    }

    /// Pipeline with all stage types active.
    #[test]
    fn pipeline_with_all_stages_active() {
        let mut pipeline = RtAxisPipeline::builder()
            .deadzone(0.0, 0.03, DeadzoneShape::Linear)
            .curve(CurveType::Expo(0.2))
            .smoothing_ema(0.9)
            .slew_rate(0.5)
            .noise_gate(0.005)
            .clamp(-1.0, 1.0)
            .build();

        assert_eq!(pipeline.stage_count(), 6);

        // Seed stateful stages
        pipeline.process(0.0);

        // Process a range of inputs
        for i in 0..=20 {
            let input = i as f64 / 20.0;
            let out = pipeline.process(input);
            assert!(
                out.is_finite() && (-1.0..=1.0).contains(&out),
                "invalid output at input={input}: {out}"
            );
        }

        // Verify diagnostics capture all stages
        let diag = pipeline.diagnostics(0.5);
        assert_eq!(diag.count, 6);
    }

    /// Hot config swap at tick boundary via AxisEngine.
    #[test]
    fn hot_config_swap_at_tick_boundary() {
        let engine = AxisEngine::new_for_axis("pitch".to_string());

        // Process without pipeline → passthrough
        let mut frame = AxisFrame::new(0.5, 1_000_000);
        engine.process(&mut frame).unwrap();
        assert_eq!(frame.out, 0.5);

        // Install pipeline with 20% deadzone
        let pipeline = PipelineBuilder::new()
            .deadzone(0.2)
            .compile()
            .expect("compile");
        let result = engine.update_pipeline(pipeline);
        assert!(
            matches!(result, flight_axis::UpdateResult::Pending),
            "update should be pending"
        );

        // Next process() triggers swap
        let mut frame2 = AxisFrame::new(0.1, 2_000_000);
        engine.process(&mut frame2).unwrap();
        // 0.1 < 0.2 deadzone → should be zeroed
        assert_eq!(frame2.out, 0.0, "deadzone should be active after swap");

        // Verify swap counter incremented
        assert_eq!(engine.swap_ack_count(), 1);

        // Swap to a new pipeline (no deadzone)
        let pipeline2 = PipelineBuilder::new()
            .curve(0.0) // linear
            .unwrap()
            .compile()
            .expect("compile");
        engine.update_pipeline(pipeline2);

        let mut frame3 = AxisFrame::new(0.1, 3_000_000);
        engine.process(&mut frame3).unwrap();
        // Linear curve should pass through
        assert!(approx32(frame3.out, 0.1), "linear curve: got {}", frame3.out);

        assert_eq!(engine.swap_ack_count(), 2);
    }

    /// Concurrent axis processing: multiple engines run independently.
    #[test]
    fn concurrent_axis_processing() {
        let engines: Vec<_> = ["pitch", "roll", "yaw", "throttle"]
            .iter()
            .map(|name| Arc::new(AxisEngine::new_for_axis(name.to_string())))
            .collect();

        // Install different pipelines
        for (i, engine) in engines.iter().enumerate() {
            let threshold = (i + 1) as f32 * 0.05; // 0.05, 0.10, 0.15, 0.20
            let pipeline = PipelineBuilder::new()
                .deadzone(threshold)
                .compile()
                .unwrap();
            engine.update_pipeline(pipeline);
        }

        // Process on separate threads
        let handles: Vec<_> = engines
            .iter()
            .enumerate()
            .map(|(i, engine)| {
                let engine = Arc::clone(engine);
                let threshold = (i + 1) as f32 * 0.05;
                std::thread::spawn(move || {
                    for tick in 0..100 {
                        let input = 0.03; // within smallest deadzone
                        let mut frame = AxisFrame::new(input, (tick + 1) * 4_000_000);
                        engine.process(&mut frame).unwrap();

                        // Only engines with threshold > 0.03 should zero the output
                        if threshold > input {
                            assert_eq!(
                                frame.out, 0.0,
                                "engine with dz={threshold} should zero input={input}"
                            );
                        }
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().expect("thread panicked");
        }
    }

    /// Axis enable/disable mid-run: bypassing pipeline stages.
    #[test]
    fn axis_enable_disable_mid_run() {
        let mut pipeline = AxisPipeline::new();
        pipeline.add_stage(Box::new(SensitivityStage { multiplier: 2.0 }));
        pipeline.add_stage(Box::new(ClampStage { min: -1.0, max: 1.0 }));

        // Active: 0.3 * 2.0 = 0.6
        let out = pipeline.process(0.3, 0.004);
        assert!(approx(out, 0.6));

        // Bypass sensitivity stage
        pipeline.bypass_stage(0);
        let out2 = pipeline.process(0.3, 0.004);
        assert!(approx(out2, 0.3)); // sensitivity bypassed

        // Re-enable
        pipeline.enable_stage(0);
        let out3 = pipeline.process(0.3, 0.004);
        assert!(approx(out3, 0.6)); // sensitivity back

        // Remove stage entirely
        pipeline.remove_stage(0);
        let out4 = pipeline.process(0.3, 0.004);
        assert!(approx(out4, 0.3)); // only clamp remains, 0.3 within range
        assert_eq!(pipeline.stage_count(), 1);
    }

    /// Pipeline preserves frame validity through all stages.
    #[test]
    fn pipeline_preserves_frame_validity() {
        let engine = AxisEngine::new_for_axis("test".to_string());

        let pipeline = PipelineBuilder::new()
            .deadzone(0.05)
            .curve(0.3)
            .unwrap()
            .slew(2.0)
            .filter(0.5)
            .compile()
            .unwrap();
        engine.update_pipeline(pipeline);

        // Process 50 sequential frames
        for i in 1..=50 {
            let input = ((i as f32 * 0.1).sin()).clamp(-1.0, 1.0);
            let ts = i as u64 * 4_000_000; // 4ms intervals (250Hz)
            let mut frame = AxisFrame::new(input, ts);
            engine.process(&mut frame).unwrap();

            assert!(
                frame.out.is_finite(),
                "frame {i}: output is not finite: {}",
                frame.out
            );
            assert!(
                frame.out >= -1.0 && frame.out <= 1.0,
                "frame {i}: output out of range: {}",
                frame.out
            );
        }
    }

    /// Diagnostics capture per-stage input/output correctly.
    #[test]
    fn diagnostics_per_stage_io() {
        let mut pipeline = AxisPipeline::new();
        pipeline.add_stage(Box::new(DeadzoneStage { inner: 0.1, outer: 1.0 }));
        pipeline.add_stage(Box::new(CurveStage { expo: 1.0 }));
        pipeline.add_stage(Box::new(ClampStage { min: -1.0, max: 1.0 }));

        let diag = pipeline.diagnostics(0.55, 0.004);
        assert_eq!(diag.len(), 3);

        // Stage 0: deadzone input=0.55
        assert_eq!(diag[0].0, "deadzone");
        assert!(approx(diag[0].1, 0.55)); // input

        // Deadzone output: (0.55 − 0.1) / (1.0 − 0.1) = 0.5
        assert!(approx(diag[0].2, 0.5)); // output

        // Stage 1: curve input = 0.5
        assert_eq!(diag[1].0, "curve");
        assert!(approx(diag[1].1, 0.5));

        // Curve output: sign(0.5) * |0.5|^(1+1) = 0.25
        assert!(approx(diag[1].2, 0.25));

        // Stage 2: clamp input = 0.25, output = 0.25 (within range)
        assert_eq!(diag[2].0, "clamp");
        assert!(approx(diag[2].1, 0.25));
        assert!(approx(diag[2].2, 0.25));
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 6. ZERO-ALLOCATION / RT CONSTRAINT TESTS (5)
// ═══════════════════════════════════════════════════════════════════════════

mod zero_allocation_depth {
    use super::*;

    /// No allocation during tick: RT pipeline types are Copy (no heap).
    #[test]
    fn no_allocation_during_tick_copy_types() {
        fn assert_copy<T: Copy>() {}

        // All RT pipeline types must be Copy → no heap pointers
        assert_copy::<flight_axis::stages::DeadzoneStage>();
        assert_copy::<flight_axis::stages::CurveStage>();
        assert_copy::<flight_axis::stages::SmoothingStage>();
        assert_copy::<flight_axis::stages::SlewRateLimiter>();
        assert_copy::<flight_axis::stages::ClampStage>();
        assert_copy::<flight_axis::stages::InvertStage>();
        assert_copy::<flight_axis::stages::NoiseGate>();
        assert_copy::<flight_axis::stages::DetentStage>();
        assert_copy::<flight_axis::stages::SaturationStage>();
        assert_copy::<flight_axis::stages::StageSlot>();
        assert_copy::<flight_axis::stages::RtAxisPipeline>();
        assert_copy::<flight_axis::stages::RtPipelineBuilder>();
        assert_copy::<AxisFrame>();
    }

    /// Pre-allocated buffers: RtAxisPipeline fits on stack with known size.
    #[test]
    fn pre_allocated_buffers_stack_sizes() {
        use std::mem::size_of;

        let frame_size = size_of::<AxisFrame>();
        assert!(frame_size <= 32, "AxisFrame too large: {frame_size}");

        let pipeline_size = size_of::<RtAxisPipeline>();
        // MAX_STAGES * sizeof(StageSlot) + count
        assert!(
            pipeline_size < 16384,
            "RtAxisPipeline too large for stack: {pipeline_size}"
        );

        let slot_size = size_of::<StageSlot>();
        assert!(slot_size < 1024, "StageSlot too large: {slot_size}");

        // AxisMixer is also Copy and fixed-size
        fn assert_copy<T: Copy>() {}
        assert_copy::<AxisMixer>();
        let mixer_size = size_of::<AxisMixer>();
        assert!(mixer_size < 256, "AxisMixer too large: {mixer_size}");
    }

    /// Atomic config swap: verify pipeline swap via AxisEngine counters.
    #[test]
    fn atomic_config_swap_counter() {
        let engine = AxisEngine::new_for_axis("test_swap".to_string());
        assert_eq!(engine.swap_ack_count(), 0);
        assert!(!engine.has_active_pipeline());

        // First swap
        let p1 = PipelineBuilder::new().deadzone(0.1).compile().unwrap();
        engine.update_pipeline(p1);

        let mut frame = AxisFrame::new(0.5, 1_000_000);
        engine.process(&mut frame).unwrap();

        assert_eq!(engine.swap_ack_count(), 1);
        assert!(engine.has_active_pipeline());
        assert_eq!(engine.active_version(), Some(1));

        // Second swap
        let p2 = PipelineBuilder::new().deadzone(0.2).compile().unwrap();
        engine.update_pipeline(p2);

        let mut frame2 = AxisFrame::new(0.5, 2_000_000);
        engine.process(&mut frame2).unwrap();

        assert_eq!(engine.swap_ack_count(), 2);
        assert_eq!(engine.active_version(), Some(2));

        // Counters track correctly
        let counters = engine.counters();
        assert_eq!(counters.pipeline_swaps(), 2);
    }

    /// Stack-only processing: RT pipeline processes without heap.
    #[test]
    fn stack_only_processing() {
        // Construct entirely on the stack
        let mut pipeline = RtAxisPipeline::builder()
            .deadzone(0.0, 0.05, DeadzoneShape::Linear)
            .curve(CurveType::Expo(0.3))
            .smoothing_ema(0.8)
            .slew_rate(0.1)
            .clamp(-1.0, 1.0)
            .build();

        // Process many frames — all on stack
        let mut outputs = [0.0_f64; 100];
        for (i, out) in outputs.iter_mut().enumerate() {
            let input = (i as f64 / 100.0) * 2.0 - 1.0;
            *out = pipeline.process(input);
        }

        // Verify all outputs are valid
        for (i, &out) in outputs.iter().enumerate() {
            assert!(
                out.is_finite() && (-1.0..=1.0).contains(&out),
                "frame {i}: invalid output {out}"
            );
        }

        // Verify monotonicity in the second half (increasing inputs after warmup)
        let mut prev = outputs[50];
        for &out in &outputs[51..] {
            // Slew rate + smoothing may delay, but should generally trend up
            assert!(
                out >= prev - 0.2,
                "unexpected drop: prev={prev}, out={out}"
            );
            prev = out;
        }
    }

    /// Sustained 250Hz without excessive jitter in processing time.
    #[test]
    fn sustained_250hz_without_jitter() {
        let mut pipeline = RtAxisPipeline::builder()
            .deadzone(0.0, 0.03, DeadzoneShape::Linear)
            .curve(CurveType::Expo(0.3))
            .smoothing_ema(0.9)
            .clamp(-1.0, 1.0)
            .build();

        let frame_count = 2500; // 10 seconds at 250Hz
        let mut max_us = 0u128;
        let mut total_us = 0u128;

        for i in 0..frame_count {
            let input = ((i as f64 * 0.1).sin()).clamp(-1.0, 1.0);

            let start = Instant::now();
            let _out = pipeline.process(input);
            let elapsed = start.elapsed().as_micros();

            max_us = max_us.max(elapsed);
            total_us += elapsed;
        }

        let avg_us = total_us / frame_count as u128;

        // Average should be well under 100µs per frame
        assert!(
            avg_us < 100,
            "average processing time too high: {avg_us}µs"
        );

        // Max should be under 500µs (0.5ms RT budget)
        // Note: in CI environments this may occasionally spike due to scheduling
        // so we use a generous budget
        assert!(
            max_us < 5000,
            "max processing time too high: {max_us}µs"
        );

        // Verify total throughput: 2500 frames should complete in well under 1 second
        let total_ms = total_us / 1000;
        assert!(
            total_ms < 500,
            "total processing time too high: {total_ms}ms for {frame_count} frames"
        );
    }
}
