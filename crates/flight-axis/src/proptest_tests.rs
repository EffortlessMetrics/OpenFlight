// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Property-based tests for the axis processing pipeline invariants.
//!
//! Tests verify correctness invariants using proptest:
//! - Curve/deadzone output bounds
//! - Monotonicity of curves
//! - Deadzone symmetry
//! - Mixer clamping
//! - Robustness against NaN/Inf inputs

use proptest::prelude::*;

use crate::{
    AxisFrame,
    nodes::{
        CurveNode, DeadzoneNode, DetentRole, DetentZone, MixerConfig, MixerInput, MixerNode, Node,
    },
};

proptest! {
    /// Curve output must remain in [-1.0, 1.0] for any valid input and expo.
    #[test]
    fn curve_output_in_range(
        input in -1.0f32..=1.0f32,
        expo in -1.0f32..=1.0f32,
    ) {
        let mut curve = CurveNode::new(expo.clamp(-1.0, 1.0));
        let mut frame = AxisFrame::new(input, 1_000_000);
        curve.step(&mut frame);
        prop_assert!(
            (-1.0..=1.0).contains(&frame.out),
            "curve output {} out of [-1, 1] for input={}, expo={}",
            frame.out, input, expo
        );
    }

    /// Curve with expo=0 is the identity mapping: output equals input for all inputs.
    #[test]
    fn curve_identity_at_zero_expo(input in -1.0f32..=1.0f32) {
        let mut curve = CurveNode::new(0.0);
        let mut frame = AxisFrame::new(input, 1_000_000);
        curve.step(&mut frame);
        prop_assert!(
            (frame.out - input).abs() < f32::EPSILON * 4.0,
            "identity curve: output {} != input {}",
            frame.out, input
        );
    }

    /// Curve is monotone: for a <= b, curve(a) <= curve(b).
    #[test]
    fn curve_is_monotone(
        a in -1.0f32..=1.0f32,
        b in -1.0f32..=1.0f32,
        expo in -1.0f32..=1.0f32,
    ) {
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        let mut curve = CurveNode::new(expo.clamp(-1.0, 1.0));

        let mut frame_lo = AxisFrame::new(lo, 1_000_000);
        let mut frame_hi = AxisFrame::new(hi, 1_000_000);
        curve.step(&mut frame_lo);
        curve.step(&mut frame_hi);

        prop_assert!(
            frame_lo.out <= frame_hi.out + 1e-5,
            "monotonicity violated: curve({})={} > curve({})={} with expo={}",
            lo, frame_lo.out, hi, frame_hi.out, expo
        );
    }

    /// Deadzone zeroes any input whose absolute value is strictly less than the threshold.
    #[test]
    fn deadzone_zeroes_within_threshold(
        input in -1.0f32..=1.0f32,
        threshold in 0.001f32..=1.0f32,
    ) {
        prop_assume!(input.abs() < threshold);
        let mut deadzone = DeadzoneNode::new(threshold);
        let mut frame = AxisFrame::new(input, 1_000_000);
        deadzone.step(&mut frame);
        prop_assert_eq!(
            frame.out,
            0.0,
            "deadzone should zero input {} with threshold {}",
            input, threshold
        );
    }

    /// Deadzone preserves sign: positive input outside the deadzone yields positive output.
    #[test]
    fn deadzone_preserves_positive_sign(
        input in 0.0f32..=1.0f32,
        threshold in 0.0f32..0.99f32,
    ) {
        prop_assume!(input > threshold);
        let mut deadzone = DeadzoneNode::new(threshold);
        let mut frame = AxisFrame::new(input, 1_000_000);
        deadzone.step(&mut frame);
        prop_assert!(
            frame.out >= 0.0,
            "positive sign not preserved: input={}, threshold={}, output={}",
            input, threshold, frame.out
        );
    }

    /// Deadzone preserves sign: negative input outside the deadzone yields negative output.
    #[test]
    fn deadzone_preserves_negative_sign(
        input in 0.0f32..=1.0f32,
        threshold in 0.0f32..0.99f32,
    ) {
        prop_assume!(input > threshold);
        let mut deadzone = DeadzoneNode::new(threshold);
        let mut frame = AxisFrame::new(-input, 1_000_000);
        deadzone.step(&mut frame);
        prop_assert!(
            frame.out <= 0.0,
            "negative sign not preserved: input={}, threshold={}, output={}",
            -input, threshold, frame.out
        );
    }

    /// Deadzone output is always within [-1.0, 1.0] for inputs in [-1.0, 1.0].
    #[test]
    fn deadzone_output_in_range(
        input in -1.0f32..=1.0f32,
        threshold in 0.0f32..0.9999f32,
    ) {
        let mut deadzone = DeadzoneNode::new(threshold);
        let mut frame = AxisFrame::new(input, 1_000_000);
        deadzone.step(&mut frame);
        prop_assert!(
            (-1.0..=1.0).contains(&frame.out),
            "deadzone output {} out of [-1, 1] for input={}, threshold={}",
            frame.out, input, threshold
        );
    }

    /// Mixer with clamping enabled always produces output in [-1.0, 1.0].
    #[test]
    fn mixer_clamped_output_in_range(
        val_a in -1.0f32..=1.0f32,
        val_b in -1.0f32..=1.0f32,
        scale_a in -2.0f32..=2.0f32,
        scale_b in -2.0f32..=2.0f32,
    ) {
        let config = MixerConfig::new("test")
            .add_input(MixerInput::new("a", scale_a, 1.0))
            .add_input(MixerInput::new("b", scale_b, 1.0));
        let mixer = MixerNode::new(config).expect("valid mixer config");
        let mut out = 0.0f32;
        mixer.process_inputs(&[val_a, val_b], &mut out);
        prop_assert!(
            (-1.0..=1.0).contains(&out),
            "mixer clamped output {} out of [-1, 1]",
            out
        );
    }

    /// CurveNode must not panic when the frame contains NaN.
    #[test]
    fn curve_nan_does_not_panic(expo in -1.0f32..=1.0f32) {
        let mut curve = CurveNode::new(expo.clamp(-1.0, 1.0));
        let mut frame = AxisFrame::new(0.0, 1_000_000);
        frame.out = f32::NAN;
        curve.step(&mut frame); // must not panic
    }

    /// CurveNode must not panic when the frame contains infinity.
    #[test]
    fn curve_infinity_does_not_panic(expo in -1.0f32..=1.0f32) {
        let mut curve = CurveNode::new(expo.clamp(-1.0, 1.0));
        let mut frame = AxisFrame::new(0.0, 1_000_000);
        frame.out = f32::INFINITY;
        curve.step(&mut frame); // must not panic
    }

    /// DetentZone.contains_entry returns true for any position within half_width of center.
    #[test]
    fn detent_zone_contains_entry_for_interior_positions(
        center in -0.8f32..=0.8f32,
        half_width in 0.001f32..=0.1f32,
        frac in -1.0f32..=1.0f32,
    ) {
        let zone = DetentZone::new(center, half_width, 0.0, DetentRole::Idle);
        // Build a position inside [center - half_width, center + half_width], clamped to [-1, 1].
        let position = (center + frac * half_width).clamp(-1.0, 1.0);
        if (position - center).abs() <= half_width {
            prop_assert!(
                zone.contains_entry(position),
                "DetentZone should report entry for position={} (center={}, half_width={})",
                position, center, half_width
            );
        }
    }

    /// DetentZone.contains_entry returns false for positions outside the entry band.
    #[test]
    fn detent_zone_no_entry_outside_half_width(
        center in -0.5f32..=0.5f32,
        half_width in 0.001f32..=0.1f32,
        gap in 0.001f32..=0.3f32,
    ) {
        let zone = DetentZone::new(center, half_width, 0.0, DetentRole::Idle);
        // Positions strictly outside the band.
        let pos_above = center + half_width + gap;
        let pos_below = center - half_width - gap;
        if pos_above <= 1.0 {
            prop_assert!(
                !zone.contains_entry(pos_above),
                "DetentZone should NOT report entry for position={} (center={}, half_width={})",
                pos_above, center, half_width
            );
        }
        if pos_below >= -1.0 {
            prop_assert!(
                !zone.contains_entry(pos_below),
                "DetentZone should NOT report entry for position={} (center={}, half_width={})",
                pos_below, center, half_width
            );
        }
    }
}

// ── Additional property tests: curve, scale, normalize, invert ──────────────

use crate::{
    curve::{ControlPoint, ExpoCurveConfig, InterpolationMode, ResponseCurve},
    deadzone::{AsymmetricDeadzoneConfig, DeadzoneConfig, DeadzoneProcessor},
    invert::AxisInvert,
    normalize::{AxisNormalizer, NormalizeConfig},
    scale::AxisScale,
};

proptest! {
    // ── ExpoCurveConfig invariants ──────────────────────────────────────────

    /// ExpoCurveConfig.apply() always returns values in [-1.0, 1.0].
    #[test]
    fn expo_output_always_bounded(
        input in -2.0f32..=2.0f32,
        expo in -1.0f32..=1.0f32,
    ) {
        let cfg = ExpoCurveConfig::new(expo);
        let out = cfg.apply(input);
        prop_assert!(
            (-1.0..=1.0).contains(&out),
            "expo output {} out of [-1, 1] for input={}, expo={}",
            out, input, expo
        );
    }

    /// ExpoCurveConfig is monotone: for a <= b, apply(a) <= apply(b).
    #[test]
    fn expo_is_monotone(
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

    /// ExpoCurveConfig is an odd function: apply(-x) == -apply(x).
    #[test]
    fn expo_is_antisymmetric(
        input in -1.0f32..=1.0f32,
        expo in -1.0f32..=1.0f32,
    ) {
        let cfg = ExpoCurveConfig::new(expo);
        let pos = cfg.apply(input);
        let neg = cfg.apply(-input);
        prop_assert!(
            (pos + neg).abs() < 1e-5,
            "expo antisymmetry violated: apply({})={}, apply({})={}, sum={}",
            input, pos, -input, neg, pos + neg
        );
    }

    // ── ResponseCurve invariants ────────────────────────────────────────────

    /// ResponseCurve.evaluate() always returns [0.0, 1.0] for any input,
    /// regardless of interpolation mode.
    #[test]
    fn response_curve_output_bounded(
        x in -0.5f32..=1.5f32,
        mid_y in 0.0f32..=1.0f32,
        mode_idx in 0u8..3u8,
    ) {
        let mode = match mode_idx {
            0 => InterpolationMode::Linear,
            1 => InterpolationMode::CubicHermite,
            _ => InterpolationMode::MonotoneCubic,
        };
        let curve = ResponseCurve::from_points(
            vec![
                ControlPoint::new(0.0, 0.0),
                ControlPoint::new(0.5, mid_y),
                ControlPoint::new(1.0, 1.0),
            ],
            mode,
        )
        .unwrap();
        let y = curve.evaluate(x);
        prop_assert!(
            (0.0..=1.0).contains(&y),
            "ResponseCurve output {} out of [0, 1] for x={}, mid_y={}, mode={:?}",
            y, x, mid_y, mode
        );
    }

    /// MonotoneCubic preserves monotonicity for arbitrary non-decreasing control points.
    #[test]
    fn response_curve_monotone_cubic_preserves_order(
        a_raw in 0.0f32..=0.5f32,
        b_raw in 0.0f32..=0.5f32,
        y0 in 0.0f32..=0.3f32,
        y1 in 0.3f32..=0.7f32,
        y2 in 0.7f32..=1.0f32,
    ) {
        let (lo, hi) = if a_raw <= b_raw { (a_raw, b_raw) } else { (b_raw, a_raw) };
        let curve = ResponseCurve::from_points(
            vec![
                ControlPoint::new(0.0, y0),
                ControlPoint::new(0.5, y1),
                ControlPoint::new(1.0, y2),
            ],
            InterpolationMode::MonotoneCubic,
        )
        .unwrap();
        let out_lo = curve.evaluate(lo);
        let out_hi = curve.evaluate(hi);
        prop_assert!(
            out_lo <= out_hi + 1e-5,
            "monotone cubic violated: evaluate({})={} > evaluate({})={} with points y=[{},{},{}]",
            lo, out_lo, hi, out_hi, y0, y1, y2
        );
    }

    // ── AxisScale invariants ────────────────────────────────────────────────

    /// AxisScale.apply() output is always within [min, max].
    #[test]
    fn scale_output_within_bounds(
        input in -2.0f32..=2.0f32,
        factor in -3.0f32..=3.0f32,
    ) {
        prop_assume!(factor.is_finite());
        let scale = AxisScale::new(factor, -1.0, 1.0).unwrap();
        let out = scale.apply(input);
        prop_assert!(
            (-1.0..=1.0).contains(&out),
            "AxisScale output {} out of [-1, 1] for input={}, factor={}",
            out, input, factor
        );
    }

    /// Default AxisScale is the identity for inputs in [-1.0, 1.0].
    #[test]
    fn scale_default_is_identity(input in -1.0f32..=1.0f32) {
        let scale = AxisScale::default();
        let out = scale.apply(input);
        prop_assert!(
            (out - input).abs() < f32::EPSILON * 4.0,
            "default scale should be identity: input={}, output={}",
            input, out
        );
    }

    // ── AxisNormalizer invariants ────────────────────────────────────────────

    /// AxisNormalizer.process() always returns values in [-1.0, 1.0].
    #[test]
    fn normalizer_output_always_bounded(input in -100.0f32..=100.0f32) {
        let mut norm = AxisNormalizer::new(NormalizeConfig::default());
        let out = norm.process(input);
        prop_assert!(
            (-1.0..=1.0).contains(&out),
            "normalizer output {} out of [-1, 1] for input={}",
            out, input
        );
    }

    /// AxisNormalizer sanitizes NaN and Inf to 0.0.
    #[test]
    fn normalizer_sanitizes_nonfinite(
        mode in 0u8..3u8,
    ) {
        let input = match mode {
            0 => f32::NAN,
            1 => f32::INFINITY,
            _ => f32::NEG_INFINITY,
        };
        let mut norm = AxisNormalizer::new(NormalizeConfig::default());
        let out = norm.process(input);
        prop_assert_eq!(out, 0.0, "non-finite input {} should map to 0.0, got {}", input, out);
        prop_assert_eq!(norm.nan_count(), 1);
    }

    // ── AxisInvert invariants ───────────────────────────────────────────────

    /// Applying inversion twice is the identity.
    #[test]
    fn invert_is_involution(input in -1.0f32..=1.0f32) {
        let inv = AxisInvert::new(true);
        let once = inv.apply(input);
        let twice = inv.apply(once);
        prop_assert!(
            (twice - input).abs() < f32::EPSILON,
            "invert is not involution: input={}, once={}, twice={}",
            input, once, twice
        );
    }

    /// Disabled inversion is the identity.
    #[test]
    fn invert_disabled_is_identity(input in -1.0f32..=1.0f32) {
        let inv = AxisInvert::new(false);
        let out = inv.apply(input);
        prop_assert_eq!(out, input, "disabled invert should be identity");
    }

    // ── Deadzone symmetry (symmetric AsymmetricDeadzoneConfig) ──────────────

    /// AsymmetricDeadzoneConfig::symmetric behaves antisymmetrically: apply(-x) == -apply(x).
    #[test]
    fn asymmetric_dz_symmetric_is_antisymmetric(
        input in 0.0f32..=1.0f32,
        width in 0.0f32..=0.99f32,
    ) {
        let cfg = AsymmetricDeadzoneConfig::symmetric(width);
        let pos = cfg.apply(input);
        let neg = cfg.apply(-input);
        prop_assert!(
            (pos + neg).abs() < 1e-5,
            "symmetric AsymmetricDZ antisymmetry violated: apply({})={}, apply({})={}",
            input, pos, -input, neg
        );
    }
}
