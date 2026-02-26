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
    nodes::{CurveNode, DeadzoneNode, DetentRole, DetentZone, MixerConfig, MixerInput, MixerNode, Node},
    AxisFrame,
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
            frame.out >= -1.0 && frame.out <= 1.0,
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
            frame.out >= -1.0 && frame.out <= 1.0,
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
            out >= -1.0 && out <= 1.0,
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
