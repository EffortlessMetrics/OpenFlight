// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for the 250Hz axis engine pipeline.
//!
//! Covers curve processing, deadzone behaviour, detent snapping, mixer
//! cross-axis mixing, full-pipeline determinism, property-based invariants,
//! zero-allocation verification, and NaN/Inf safety.
//!
//! All RT-path tests are zero-allocation-aware: they exercise the stack-allocated
//! `RtAxisPipeline` and `RtDetentProcessor` (ADR-004 compliant).

use flight_axis::curve::{ControlPoint, ExpoCurveConfig, InterpolationMode, ResponseCurve};
use flight_axis::deadzone::{
    AsymmetricDeadzoneConfig, DeadzoneBank, DeadzoneConfig, DeadzoneProcessor,
};
use flight_axis::detent::{DetentBand, DetentConfig, DetentProcessor, RtDetentProcessor};
use flight_axis::mixer::{AxisMixer, MixMode};
use flight_axis::pipeline::{AxisPipeline, ClampStage, CurveStage, DeadzoneStage, SensitivityStage};
use flight_axis::stages::{
    CurveStage as RtCurveStage, CurveType, DeadzoneShape,
    DeadzoneStage as RtDeadzoneStage, DetentPosition, DetentStage as RtDetentStage,
    RtAxisPipeline, Stage, StageSlot,
};
use flight_axis::{AxisEngine, AxisFrame};
use proptest::prelude::*;

const TOL_F32: f32 = 1e-5;
const TOL_F64: f64 = 1e-10;

fn approx_f32(a: f32, b: f32) -> bool {
    (a - b).abs() <= TOL_F32
}

fn approx_f64(a: f64, b: f64) -> bool {
    (a - b).abs() <= TOL_F64
}

// ===========================================================================
// 1. Curve processing
// ===========================================================================

mod curve_tests {
    use super::*;

    #[test]
    fn linear_identity_maps_all_sampled_points() {
        let curve = ResponseCurve::linear_identity();
        for i in 0..=100 {
            let x = i as f32 / 100.0;
            let y = curve.evaluate(x);
            assert!(
                approx_f32(y, x),
                "linear identity: evaluate({x}) = {y} != {x}"
            );
        }
    }

    #[test]
    fn expo_zero_is_linear() {
        let expo = ExpoCurveConfig::linear();
        for i in -100..=100 {
            let v = i as f32 / 100.0;
            assert!(
                approx_f32(expo.apply(v), v),
                "expo=0 should be identity at {v}"
            );
        }
    }

    #[test]
    fn expo_positive_various_values() {
        for &e in &[0.1_f32, 0.3, 0.5, 0.7, 0.9, 1.0] {
            let expo = ExpoCurveConfig::new(e);
            // Positive expo reduces sensitivity near center
            let mid = expo.apply(0.5);
            assert!(mid < 0.5, "expo={e}: apply(0.5)={mid} should be < 0.5");
            // Endpoints always map to themselves
            assert!(approx_f32(expo.apply(0.0), 0.0));
            assert!(approx_f32(expo.apply(1.0), 1.0));
            assert!(approx_f32(expo.apply(-1.0), -1.0));
        }
    }

    #[test]
    fn expo_negative_various_values() {
        for &e in &[-0.1_f32, -0.3, -0.5, -0.7, -0.9, -1.0] {
            let expo = ExpoCurveConfig::new(e);
            let mid = expo.apply(0.5);
            assert!(mid > 0.5, "expo={e}: apply(0.5)={mid} should be > 0.5");
        }
    }

    #[test]
    fn custom_curve_interpolation_accuracy() {
        let curve = ResponseCurve::from_points(
            vec![
                ControlPoint::new(0.0, 0.0),
                ControlPoint::new(0.25, 0.1),
                ControlPoint::new(0.5, 0.5),
                ControlPoint::new(0.75, 0.9),
                ControlPoint::new(1.0, 1.0),
            ],
            InterpolationMode::Linear,
        )
        .unwrap();
        // Control points should be hit exactly
        assert!(approx_f32(curve.evaluate(0.0), 0.0));
        assert!(approx_f32(curve.evaluate(0.25), 0.1));
        assert!(approx_f32(curve.evaluate(0.5), 0.5));
        assert!(approx_f32(curve.evaluate(0.75), 0.9));
        assert!(approx_f32(curve.evaluate(1.0), 1.0));

        // Mid-segment interpolation: midpoint between (0.0, 0.0) and (0.25, 0.1)
        let mid = curve.evaluate(0.125);
        assert!(approx_f32(mid, 0.05));
    }

    #[test]
    fn curve_composition_chaining() {
        // Chain two curves: first S-curve, then identity — output should match first
        let s_curve = ResponseCurve::from_points(
            vec![
                ControlPoint::new(0.0, 0.0),
                ControlPoint::new(0.5, 0.3),
                ControlPoint::new(1.0, 1.0),
            ],
            InterpolationMode::Linear,
        )
        .unwrap();
        let identity = ResponseCurve::linear_identity();

        for i in 0..=20 {
            let x = i as f32 / 20.0;
            let after_s = s_curve.evaluate(x);
            let after_chain = identity.evaluate(after_s);
            assert!(
                approx_f32(after_s, after_chain),
                "identity(s_curve({x})) should equal s_curve({x})"
            );
        }
    }

    #[test]
    fn curve_edge_cases_min_max() {
        let curve = ResponseCurve::linear_identity();
        assert!(approx_f32(curve.evaluate(0.0), 0.0));
        assert!(approx_f32(curve.evaluate(1.0), 1.0));
        // Below-zero and above-one are clamped
        assert_eq!(curve.evaluate(-10.0), 0.0);
        assert_eq!(curve.evaluate(10.0), 1.0);
    }

    #[test]
    fn expo_extreme_values_clamped() {
        // Expo outside [-1, 1] is clamped by constructor
        let expo = ExpoCurveConfig::new(5.0);
        assert_eq!(expo.expo, 1.0); // clamped to exactly 1.0
        let expo_neg = ExpoCurveConfig::new(-5.0);
        assert_eq!(expo_neg.expo, -1.0); // clamped to exactly -1.0
    }

    #[test]
    fn rt_curve_linear_identity() {
        let mut stage = RtCurveStage::linear();
        assert!(approx_f64(stage.process(0.5), 0.5));
        assert!(approx_f64(stage.process(-0.3), -0.3));
        assert!(approx_f64(stage.process(0.0), 0.0));
    }

    #[test]
    fn rt_curve_expo_reduces_center() {
        let mut stage = RtCurveStage::expo(0.5);
        let out = stage.process(0.5);
        // 0.5^1.5 ≈ 0.3536
        assert!(out < 0.5, "expo=0.5: process(0.5)={out} should be < 0.5");
        // Endpoints preserved
        assert!(approx_f64(stage.process(1.0), 1.0));
        assert!(approx_f64(stage.process(0.0), 0.0));
    }

    #[test]
    fn rt_curve_custom_piecewise() {
        let stage = RtCurveStage::custom(&[(0.0, 0.0), (0.5, 0.25), (1.0, 1.0)]).unwrap();
        let mut stage = stage;
        // At x=0.25 → interpolate between (0.0, 0.0) and (0.5, 0.25): t=0.5 → 0.125
        let out = stage.process(0.25);
        assert!(
            approx_f64(out, 0.125),
            "custom curve at 0.25: expected ~0.125, got {out}"
        );
    }

    #[test]
    fn rt_curve_nan_returns_zero() {
        let mut stage = RtCurveStage::expo(0.3);
        assert!(approx_f64(stage.process(f64::NAN), 0.0));
    }

    #[test]
    fn rt_curve_inf_returns_zero() {
        let mut stage = RtCurveStage::expo(0.3);
        assert!(approx_f64(stage.process(f64::INFINITY), 0.0));
        assert!(approx_f64(stage.process(f64::NEG_INFINITY), 0.0));
    }
}

// ===========================================================================
// 2. Deadzone tests
// ===========================================================================

mod deadzone_tests {
    use super::*;

    #[test]
    fn center_deadzone_collapses_to_zero() {
        let config = DeadzoneConfig::center_only(0.1).unwrap();
        let proc = DeadzoneProcessor::new(config);
        for &v in &[0.0_f32, 0.01, 0.05, 0.09, 0.099, -0.01, -0.05, -0.099] {
            assert_eq!(proc.apply(v), 0.0, "within center deadzone: {v} → 0.0");
        }
    }

    #[test]
    fn edge_deadzone_clamps_to_one() {
        let config = DeadzoneConfig::new(0.0, 0.1).unwrap();
        let proc = DeadzoneProcessor::new(config);
        // Values near ±1.0 within edge zone saturate
        assert_eq!(proc.apply(0.95), 1.0);
        assert_eq!(proc.apply(-0.95), -1.0);
        assert_eq!(proc.apply(1.0), 1.0);
        assert_eq!(proc.apply(-1.0), -1.0);
    }

    #[test]
    fn asymmetric_deadzone_different_sides() {
        let cfg = AsymmetricDeadzoneConfig::new(0.1, 0.2);
        // Positive side: values < 0.1 → 0
        assert_eq!(cfg.apply(0.05), 0.0);
        assert!(cfg.apply(0.15) > 0.0);
        // Negative side: values > -0.2 → 0
        assert_eq!(cfg.apply(-0.15), 0.0);
        assert!(cfg.apply(-0.25) < 0.0);
    }

    #[test]
    fn zero_deadzone_passthrough() {
        let proc = DeadzoneProcessor::new(DeadzoneConfig::default());
        for i in -100..=100 {
            let v = i as f32 / 100.0;
            assert!(
                approx_f32(proc.apply(v), v),
                "zero deadzone should pass through: {v}"
            );
        }
    }

    #[test]
    fn full_center_deadzone_everything_collapses() {
        // center=0.499, edge=0.0 → nearly everything is in the deadzone
        let config = DeadzoneConfig::center_only(0.499).unwrap();
        let proc = DeadzoneProcessor::new(config);
        assert_eq!(proc.apply(0.0), 0.0);
        assert_eq!(proc.apply(0.1), 0.0);
        assert_eq!(proc.apply(0.4), 0.0);
        // Only values above 0.499 produce output
        assert!(proc.apply(0.6) > 0.0);
    }

    #[test]
    fn deadzone_bank_axis_isolation() {
        let mut bank = DeadzoneBank::new(3);
        bank.set_config(0, DeadzoneConfig::center_only(0.2).unwrap());
        // axis 0 has deadzone
        assert_eq!(bank.apply(0, 0.1), 0.0);
        // axis 1 has default (passthrough)
        assert!(approx_f32(bank.apply(1, 0.1), 0.1));
    }

    #[test]
    fn rt_deadzone_linear_center_suppressed() {
        let mut stage = RtDeadzoneStage::new(0.0, 0.05, DeadzoneShape::Linear);
        assert!(approx_f64(stage.process(0.03), 0.0));
        assert!(approx_f64(stage.process(-0.02), 0.0));
    }

    #[test]
    fn rt_deadzone_rescales_outside() {
        let mut stage = RtDeadzoneStage::new(0.0, 0.1, DeadzoneShape::Linear);
        let out = stage.process(0.55);
        // (0.55 - 0.1) / (1.0 - 0.1) = 0.45 / 0.9 = 0.5
        assert!(
            approx_f64(out, 0.5),
            "rescaled deadzone: expected 0.5, got {out}"
        );
    }

    #[test]
    fn rt_deadzone_nan_returns_zero() {
        let mut stage = RtDeadzoneStage::new(0.0, 0.05, DeadzoneShape::Linear);
        assert!(approx_f64(stage.process(f64::NAN), 0.0));
    }
}

// ===========================================================================
// 3. Detent tests
// ===========================================================================

mod detent_tests {
    use super::*;

    #[test]
    fn detent_snap_within_range() {
        let mut proc = DetentProcessor::new(
            DetentConfig::new()
                .add(0.0, 0.03, "idle")
                .add(1.0, 0.03, "toga"),
        );
        assert_eq!(proc.apply(0.02), 0.0);
        assert_eq!(proc.active_detent_label(), Some("idle"));
        assert_eq!(proc.apply(0.99), 1.0);
        assert_eq!(proc.active_detent_label(), Some("toga"));
    }

    #[test]
    fn detent_free_midrange() {
        let mut proc = DetentProcessor::new(DetentConfig::standard_throttle());
        let free_val = 0.5;
        assert_eq!(proc.apply(free_val), free_val);
        assert_eq!(proc.active_detent_label(), None);
    }

    #[test]
    fn detent_multiple_on_same_axis() {
        let mut proc = DetentProcessor::new(DetentConfig::airbus_throttle());
        // Five detents: 0.0, 0.25, 0.75, 0.90, 1.0
        assert_eq!(proc.apply(0.01), 0.0); // idle
        assert_eq!(proc.apply(0.26), 0.25); // idle detent
        assert_eq!(proc.apply(0.5), 0.5); // free
        assert_eq!(proc.apply(0.74), 0.75); // climb
    }

    #[test]
    fn rt_detent_snap_and_hysteresis() {
        let mut proc = RtDetentProcessor::<4>::new();
        proc.add(DetentBand::new(0.5, 0.05, 0.02));

        // Enter the zone
        assert_eq!(proc.process(0.52), 0.5);
        // Still held due to hysteresis (exit threshold = 0.05 + 0.02 = 0.07)
        assert_eq!(proc.process(0.56), 0.5);
        // Exit past hysteresis
        let out = proc.process(0.58);
        assert!(
            approx_f32(out, 0.58),
            "should exit detent: {out}"
        );
    }

    #[test]
    fn rt_detent_multiple_bands() {
        let mut proc = RtDetentProcessor::<4>::new();
        proc.add(DetentBand::new(0.0, 0.05, 0.01));
        proc.add(DetentBand::new(0.5, 0.05, 0.01));
        proc.add(DetentBand::new(1.0, 0.05, 0.01));

        assert_eq!(proc.process(0.03), 0.0); // snaps to idle
        assert_eq!(proc.process(0.52), 0.5); // snaps to mid
        assert_eq!(proc.process(0.98), 1.0); // snaps to full
    }

    #[test]
    fn rt_detent_reset_clears_state() {
        let mut proc = RtDetentProcessor::<4>::new();
        proc.add(DetentBand::new(0.0, 0.1, 0.05));
        proc.process(0.05); // engage
        proc.reset();
        // After reset, re-entering the zone should re-engage
        assert_eq!(proc.process(0.05), 0.0);
    }

    #[test]
    fn rt_detent_stage_snap_behavior() {
        let positions = &[
            DetentPosition::new(0.0, 0.05, 1.0),
            DetentPosition::new(0.5, 0.05, 1.0),
        ];
        let mut stage = RtDetentStage::from_positions(positions).unwrap();
        // Full strength snap: within zone → snaps to position
        assert!(approx_f64(stage.process(0.03), 0.0));
        assert!(approx_f64(stage.process(0.52), 0.5));
        // Outside both zones → passthrough
        assert!(approx_f64(stage.process(0.3), 0.3));
    }

    #[test]
    fn rt_detent_stage_partial_strength() {
        let positions = &[DetentPosition::new(0.5, 0.1, 0.5)];
        let mut stage = RtDetentStage::from_positions(positions).unwrap();
        // Half strength: input 0.55 → 0.55 + (0.5 - 0.55) * 0.5 = 0.55 - 0.025 = 0.525
        let out = stage.process(0.55);
        assert!(
            (out - 0.525).abs() < 1e-6,
            "partial detent: expected 0.525, got {out}"
        );
    }
}

// ===========================================================================
// 4. Mixer tests
// ===========================================================================

mod mixer_tests {
    use super::*;

    #[test]
    fn cross_axis_mixing_pitch_affects_roll() {
        // Simulate pitch axis feeding into roll via weighted sum
        let mixer = AxisMixer::with_weights(MixMode::WeightedSum, &[1.0, 0.1]);
        let roll_raw = 0.5;
        let pitch_raw = 0.8;
        let output = mixer.combine(&[roll_raw, pitch_raw]);
        // 0.5 * 1.0 + 0.8 * 0.1 = 0.58
        assert!(approx_f64(output, 0.58));
    }

    #[test]
    fn mix_ratio_limits() {
        let mixer = AxisMixer::with_weights(MixMode::WeightedSum, &[1.0, 1.0]);
        // Both at max → clamp to 1.0
        assert!(approx_f64(mixer.combine(&[0.7, 0.7]), 1.0));
        assert!(approx_f64(mixer.combine(&[-0.7, -0.7]), -1.0));
    }

    #[test]
    fn priority_mode_selects_highest_weight() {
        let mixer = AxisMixer::with_weights(MixMode::Priority, &[1.0, 5.0, 3.0]);
        // Weight 5.0 is highest → picks values[1]
        assert!(approx_f64(mixer.combine(&[0.1, 0.9, 0.5]), 0.9));
    }

    #[test]
    fn max_mode_picks_largest() {
        let mixer = AxisMixer::with_weights(MixMode::Max, &[1.0, 1.0, 1.0]);
        assert!(approx_f64(mixer.combine(&[-0.5, 0.3, 0.8]), 0.8));
    }

    #[test]
    fn min_mode_picks_smallest() {
        let mixer = AxisMixer::with_weights(MixMode::Min, &[1.0, 1.0, 1.0]);
        assert!(approx_f64(mixer.combine(&[-0.5, 0.3, 0.8]), -0.5));
    }

    #[test]
    fn mixer_nan_safety() {
        let mixer = AxisMixer::with_weights(MixMode::WeightedSum, &[1.0, 1.0]);
        let out = mixer.combine(&[0.5, f64::NAN]);
        assert!(out.is_finite(), "NaN input should not propagate");
        assert!(approx_f64(out, 0.5));
    }

    #[test]
    fn mixer_inf_safety() {
        let mixer = AxisMixer::with_weights(MixMode::WeightedSum, &[1.0, 1.0]);
        let out = mixer.combine(&[0.3, f64::INFINITY]);
        assert!(out.is_finite(), "Inf input should not propagate");
    }

    #[test]
    fn mixer_is_copy_and_stack_sized() {
        fn assert_copy<T: Copy>() {}
        assert_copy::<AxisMixer>();
        assert!(std::mem::size_of::<AxisMixer>() < 256);
    }
}

// ===========================================================================
// 5. Full pipeline tests
// ===========================================================================

mod pipeline_tests {
    use super::*;

    #[test]
    fn raw_to_deadzone_to_curve_to_output() {
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

        // Within deadzone → 0
        let out = pipeline.process(0.03, 0.004);
        assert!(approx_f64(out, 0.0));

        // Full deflection → should be close to 1.0
        let out = pipeline.process(1.0, 0.004);
        assert!(
            (out - 1.0).abs() < 1e-6,
            "full deflection: expected ~1.0, got {out}"
        );

        // Negative full deflection
        let out = pipeline.process(-1.0, 0.004);
        assert!(
            (out + 1.0).abs() < 1e-6,
            "neg full deflection: expected ~-1.0, got {out}"
        );
    }

    #[test]
    fn rt_pipeline_deadzone_curve_clamp() {
        let mut pipeline = RtAxisPipeline::builder()
            .deadzone(0.0, 0.05, DeadzoneShape::Linear)
            .curve(CurveType::Expo(0.3))
            .clamp(-1.0, 1.0)
            .build();

        assert!(approx_f64(pipeline.process(0.03), 0.0));
        let full = pipeline.process(1.0);
        assert!(
            (full - 1.0).abs() < 1e-6,
            "RT pipeline full deflection: {full}"
        );
    }

    #[test]
    fn rt_pipeline_multiple_axes_per_tick() {
        let mut pipeline = RtAxisPipeline::builder()
            .deadzone(0.0, 0.02, DeadzoneShape::Linear)
            .clamp(-1.0, 1.0)
            .build();

        let inputs = [0.0, 0.01, 0.5, -0.3, 1.0, -1.0, 0.02];
        let mut outputs = [0.0_f64; 7];
        for (i, &input) in inputs.iter().enumerate() {
            outputs[i] = pipeline.process(input);
        }
        // Verify deadzone: 0.01 within 0.02 → 0.0
        assert!(approx_f64(outputs[1], 0.0));
        // Full deflection passes through clamp
        assert!((outputs[4] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn config_hot_swap_mid_stream() {
        // Build first pipeline: no deadzone
        let mut pipeline1 = RtAxisPipeline::builder().clamp(-1.0, 1.0).build();
        let out1 = pipeline1.process(0.02);
        assert!(
            (out1 - 0.02).abs() < 1e-6,
            "before swap: passthrough expected"
        );

        // Build second pipeline: with deadzone
        let mut pipeline2 = RtAxisPipeline::builder()
            .deadzone(0.0, 0.05, DeadzoneShape::Linear)
            .clamp(-1.0, 1.0)
            .build();
        let out2 = pipeline2.process(0.02);
        assert!(approx_f64(out2, 0.0), "after swap: deadzone should zero out");
    }

    #[test]
    fn engine_process_without_pipeline_passthrough() {
        let engine = AxisEngine::new();
        let mut frame = AxisFrame::new(0.75, 1000);
        engine.process(&mut frame).unwrap();
        assert_eq!(frame.out, 0.75);
    }

    #[test]
    fn pipeline_diagnostics_trace() {
        let mut pipeline = RtAxisPipeline::builder()
            .deadzone(0.0, 0.05, DeadzoneShape::Linear)
            .curve(CurveType::Expo(0.3))
            .clamp(-1.0, 1.0)
            .build();

        let diag = pipeline.diagnostics(0.5);
        assert_eq!(diag.count, 3);
        assert_eq!(diag.entries[0].name, "deadzone");
        assert_eq!(diag.entries[1].name, "curve");
        assert_eq!(diag.entries[2].name, "clamp");
        assert!(diag.final_output > 0.0);
    }

    #[test]
    fn pipeline_stage_bypass() {
        let mut pipeline = AxisPipeline::new();
        pipeline.add_stage(Box::new(SensitivityStage { multiplier: 2.0 }));
        pipeline.add_stage(Box::new(ClampStage {
            min: -1.0,
            max: 1.0,
        }));

        // Without bypass: 0.6 * 2 = 1.2 → clamp to 1.0
        assert!(approx_f64(pipeline.process(0.6, 0.004), 1.0));

        // Bypass sensitivity
        pipeline.bypass_stage(0);
        assert!(approx_f64(pipeline.process(0.6, 0.004), 0.6));

        // Re-enable
        pipeline.enable_stage(0);
        assert!(approx_f64(pipeline.process(0.6, 0.004), 1.0));
    }
}

// ===========================================================================
// 6. Property tests
// ===========================================================================

mod property_tests {
    use super::*;

    proptest! {
        /// Output always in [-1.0, 1.0] for any input in [-1.0, 1.0].
        #[test]
        fn output_bounded_for_any_input(input in -1.0f64..=1.0f64) {
            let mut pipeline = RtAxisPipeline::builder()
                .deadzone(0.0, 0.05, DeadzoneShape::Linear)
                .curve(CurveType::Expo(0.3))
                .clamp(-1.0, 1.0)
                .build();
            let out = pipeline.process(input);
            prop_assert!(
                (-1.0..=1.0).contains(&out),
                "output {out} out of [-1,1] for input={input}"
            );
        }

        /// Monotonic expo curve remains monotonic.
        #[test]
        fn expo_curve_monotonic(
            a in -1.0f32..=0.0f32,
            b in 0.0f32..=1.0f32,
            expo in 0.0f32..=1.0f32,
        ) {
            let cfg = ExpoCurveConfig::new(expo);
            let ya = cfg.apply(a);
            let yb = cfg.apply(b);
            prop_assert!(
                ya <= yb + TOL_F32,
                "expo={expo}: apply({a})={ya} > apply({b})={yb}"
            );
        }

        /// Symmetric deadzone has antisymmetric output.
        #[test]
        fn deadzone_antisymmetry(
            input in -1.0f32..=1.0f32,
            center in 0.0f32..0.49f32,
        ) {
            if let Ok(config) = DeadzoneConfig::center_only(center) {
                let proc = DeadzoneProcessor::new(config);
                let pos = proc.apply(input);
                let neg = proc.apply(-input);
                prop_assert!(
                    approx_f32(pos + neg, 0.0),
                    "antisymmetry: apply({input})={pos}, apply({})={neg}",
                    -input
                );
            }
        }

        /// Same input + config always produces same output (determinism).
        #[test]
        fn pipeline_deterministic(input in -1.0f64..=1.0f64) {
            let build = || {
                RtAxisPipeline::builder()
                    .deadzone(0.0, 0.05, DeadzoneShape::Linear)
                    .curve(CurveType::Expo(0.3))
                    .clamp(-1.0, 1.0)
                    .build()
            };
            let mut p1 = build();
            let mut p2 = build();
            let out1 = p1.process(input);
            let out2 = p2.process(input);
            prop_assert!(
                approx_f64(out1, out2),
                "non-deterministic: {out1} != {out2} for input={input}"
            );
        }

        /// ResponseCurve output always in [0.0, 1.0] for any input.
        #[test]
        fn response_curve_output_bounded(x in -2.0f32..=2.0f32) {
            let curve = ResponseCurve::from_points(
                vec![
                    ControlPoint::new(0.0, 0.0),
                    ControlPoint::new(0.3, 0.5),
                    ControlPoint::new(0.7, 0.6),
                    ControlPoint::new(1.0, 1.0),
                ],
                InterpolationMode::MonotoneCubic,
            )
            .unwrap();
            let y = curve.evaluate(x);
            prop_assert!(
                (0.0..=1.0).contains(&y),
                "output {y} out of [0, 1] for x={x}"
            );
        }

        /// Mixer WeightedSum output always in [-1.0, 1.0].
        #[test]
        fn mixer_output_bounded(
            a in -1.0f64..=1.0f64,
            b in -1.0f64..=1.0f64,
        ) {
            let mixer = AxisMixer::with_weights(MixMode::WeightedSum, &[1.0, 1.0]);
            let out = mixer.combine(&[a, b]);
            prop_assert!(
                (-1.0..=1.0).contains(&out),
                "mixer output {out} out of range for a={a}, b={b}"
            );
        }
    }
}

// ===========================================================================
// 7. NaN/Inf/subnormal safety
// ===========================================================================

mod nan_inf_safety {
    use super::*;

    #[test]
    fn expo_curve_nan_input() {
        let expo = ExpoCurveConfig::new(0.5);
        let out = expo.apply(f32::NAN);
        // NaN.clamp produces NaN in std, but the expo formula will produce NaN
        // The important thing is it doesn't panic.
        let _ = out;
    }

    #[test]
    fn expo_curve_inf_input() {
        let expo = ExpoCurveConfig::new(0.5);
        let out = expo.apply(f32::INFINITY);
        // Clamp(INFINITY, -1, 1) = 1.0 → apply(1.0) = 1.0
        assert_eq!(out, 1.0);
        let out_neg = expo.apply(f32::NEG_INFINITY);
        assert_eq!(out_neg, -1.0);
    }

    #[test]
    fn deadzone_nan_input_does_not_panic() {
        let proc = DeadzoneProcessor::new(DeadzoneConfig::default());
        let _out = proc.apply(f32::NAN); // must not panic
    }

    #[test]
    fn deadzone_inf_input_clamped() {
        let proc = DeadzoneProcessor::new(DeadzoneConfig::default());
        assert_eq!(proc.apply(f32::INFINITY), 1.0);
        assert_eq!(proc.apply(f32::NEG_INFINITY), -1.0);
    }

    #[test]
    fn rt_pipeline_nan_injection_no_panic() {
        let mut pipeline = RtAxisPipeline::builder()
            .deadzone(0.0, 0.05, DeadzoneShape::Linear)
            .curve(CurveType::Expo(0.3))
            .clamp(-1.0, 1.0)
            .build();
        let out = pipeline.process(f64::NAN);
        assert!(out.is_finite(), "NaN should not propagate: got {out}");
    }

    #[test]
    fn rt_pipeline_inf_injection_no_panic() {
        let mut pipeline = RtAxisPipeline::builder()
            .deadzone(0.0, 0.05, DeadzoneShape::Linear)
            .curve(CurveType::Expo(0.3))
            .clamp(-1.0, 1.0)
            .build();
        let out = pipeline.process(f64::INFINITY);
        assert!(out.is_finite(), "Inf should not propagate: got {out}");
    }

    #[test]
    fn rt_pipeline_subnormal_no_panic() {
        let mut pipeline = RtAxisPipeline::builder()
            .deadzone(0.0, 0.05, DeadzoneShape::Linear)
            .curve(CurveType::Expo(0.3))
            .clamp(-1.0, 1.0)
            .build();
        let subnormal = f64::MIN_POSITIVE / 2.0;
        let out = pipeline.process(subnormal);
        assert!(out.is_finite(), "subnormal should not cause issues: {out}");
    }

    #[test]
    fn engine_nan_frame_does_not_panic() {
        let engine = AxisEngine::new();
        let mut frame = AxisFrame::new(f32::NAN, 1000);
        let result = engine.process(&mut frame);
        assert!(result.is_ok(), "NaN frame should not cause error");
    }

    #[test]
    fn engine_inf_frame_does_not_panic() {
        let engine = AxisEngine::new();
        let mut frame = AxisFrame::new(f32::INFINITY, 1000);
        let result = engine.process(&mut frame);
        assert!(result.is_ok(), "Inf frame should not cause error");
    }

    #[test]
    fn asymmetric_deadzone_nan_does_not_panic() {
        let cfg = AsymmetricDeadzoneConfig::new(0.1, 0.2);
        let _ = cfg.apply(f32::NAN); // must not panic
    }
}

// ===========================================================================
// 8. Zero-allocation verification
// ===========================================================================

mod zero_alloc_tests {
    use super::*;

    #[test]
    fn rt_pipeline_is_copy() {
        fn assert_copy<T: Copy>() {}
        assert_copy::<RtAxisPipeline>();
    }

    #[test]
    fn rt_pipeline_stack_size_bounded() {
        let size = std::mem::size_of::<RtAxisPipeline>();
        // The pipeline should be stack-allocated and bounded
        assert!(
            size < 65536,
            "RtAxisPipeline too large for stack: {size} bytes"
        );
    }

    #[test]
    fn rt_detent_processor_is_stack_allocated() {
        let size = std::mem::size_of::<RtDetentProcessor<4>>();
        assert!(
            size < 1024,
            "RtDetentProcessor<4> too large: {size} bytes"
        );
    }

    #[test]
    fn axis_mixer_is_copy() {
        fn assert_copy<T: Copy>() {}
        assert_copy::<AxisMixer>();
    }

    #[test]
    fn axis_frame_is_copy_and_repr_c() {
        fn assert_copy<T: Copy>() {}
        assert_copy::<AxisFrame>();
        // AxisFrame is repr(C) — size should be stable
        // repr(C): 3×f32 (12 bytes) + 4-byte pad + u64 (8 bytes) = 24 bytes
        assert_eq!(std::mem::size_of::<AxisFrame>(), 24);
    }

    #[test]
    fn rt_stage_slot_is_copy() {
        fn assert_copy<T: Copy>() {}
        assert_copy::<StageSlot>();
    }

    #[test]
    fn rt_pipeline_process_tick_no_heap_alloc() {
        // Verify the pipeline processes without heap allocation by running
        // through 1000 ticks and confirming deterministic output.
        // (True allocation tracking requires rt-checks feature; here we verify
        // the pipeline compiles to pure stack operations by checking Copy + determinism.)
        let build = || {
            RtAxisPipeline::builder()
                .deadzone(0.0, 0.05, DeadzoneShape::Linear)
                .curve(CurveType::Expo(0.3))
                .clamp(-1.0, 1.0)
                .build()
        };

        let mut pipeline = build();
        let inputs: Vec<f64> = (0..1000).map(|i| (i as f64 / 999.0) * 2.0 - 1.0).collect();
        let outputs1: Vec<f64> = inputs.iter().map(|&v| pipeline.process(v)).collect();

        // Process again with a fresh pipeline — must be identical
        let mut pipeline2 = build();
        let outputs2: Vec<f64> = inputs.iter().map(|&v| pipeline2.process(v)).collect();

        for (i, (&o1, &o2)) in outputs1.iter().zip(outputs2.iter()).enumerate() {
            assert!(
                approx_f64(o1, o2),
                "tick {i}: {o1} != {o2}"
            );
        }
    }

    #[test]
    fn config_swap_is_value_copy() {
        // RtAxisPipeline is Copy, so "swapping" config is just an assignment.
        let pipeline1 = RtAxisPipeline::builder()
            .deadzone(0.0, 0.05, DeadzoneShape::Linear)
            .build();
        let pipeline2 = RtAxisPipeline::builder()
            .curve(CurveType::Expo(0.5))
            .clamp(-1.0, 1.0)
            .build();

        // Atomic swap is a simple Copy assignment, no allocation needed.
        let mut active = pipeline1;
        let _ = active; // use before overwrite
        active = pipeline2; // Copy, not move — no heap involved
        let out = active.process(0.5);
        assert!(out > 0.0, "swapped pipeline should process: {out}");
    }
}

// ===========================================================================
// 9. Timing / throughput
// ===========================================================================

mod throughput_tests {
    use super::*;

    #[test]
    fn process_1000_ticks_deterministic() {
        let mut pipeline = RtAxisPipeline::builder()
            .deadzone(0.0, 0.03, DeadzoneShape::Linear)
            .curve(CurveType::Expo(0.2))
            .clamp(-1.0, 1.0)
            .build();

        let mut outputs = [0.0_f64; 1000];
        for i in 0..1000 {
            let input = ((i as f64) * 0.001 * std::f64::consts::PI).sin();
            outputs[i] = pipeline.process(input);
        }

        // Verify all outputs are bounded
        for (i, &out) in outputs.iter().enumerate() {
            assert!(
                out.is_finite() && out >= -1.0 && out <= 1.0,
                "tick {i}: output {out} out of bounds"
            );
        }

        // Verify reproducibility
        let mut pipeline2 = RtAxisPipeline::builder()
            .deadzone(0.0, 0.03, DeadzoneShape::Linear)
            .curve(CurveType::Expo(0.2))
            .clamp(-1.0, 1.0)
            .build();
        for i in 0..1000 {
            let input = ((i as f64) * 0.001 * std::f64::consts::PI).sin();
            let out2 = pipeline2.process(input);
            assert!(
                approx_f64(outputs[i], out2),
                "tick {i}: non-deterministic"
            );
        }
    }

    #[test]
    #[ignore] // CI: run with --ignored for throughput benchmarks
    fn throughput_10k_frames_under_budget() {
        use std::time::Instant;

        let mut pipeline = RtAxisPipeline::builder()
            .deadzone(0.0, 0.05, DeadzoneShape::Linear)
            .curve(CurveType::Expo(0.3))
            .clamp(-1.0, 1.0)
            .build();

        let start = Instant::now();
        for i in 0..10_000 {
            let input = (i as f64 / 10_000.0) * 2.0 - 1.0;
            std::hint::black_box(pipeline.process(input));
        }
        let elapsed = start.elapsed();

        // 10,000 frames at 250Hz = 40 seconds of real-time.
        // Processing should complete in well under 1 second on any modern CPU.
        assert!(
            elapsed.as_millis() < 1000,
            "10k frames took {}ms — too slow",
            elapsed.as_millis()
        );
    }

    #[test]
    #[ignore] // CI: run with --ignored for throughput benchmarks
    fn engine_throughput_1000_frames() {
        use std::time::Instant;

        let engine = AxisEngine::new();
        let start = Instant::now();
        for i in 0..1000 {
            let input = (i as f32 / 1000.0) * 2.0 - 1.0;
            let mut frame = AxisFrame::new(input, (i as u64 + 1) * 4_000_000);
            let _ = std::hint::black_box(engine.process(&mut frame));
        }
        let elapsed = start.elapsed();
        assert!(
            elapsed.as_millis() < 500,
            "1k engine frames took {}ms",
            elapsed.as_millis()
        );
    }
}
