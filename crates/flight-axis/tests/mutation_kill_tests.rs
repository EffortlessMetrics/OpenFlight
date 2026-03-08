// SPDX-License-Identifier: MIT OR Apache-2.0
// Targeted tests to improve mutation kill rate in flight-axis.
// Covers boundary mutations (< vs <=), arithmetic mutations (sign, coefficients),
// and return value mutations in deadzone, curve, normalize, and mixer modules.

use flight_axis::curve::{
    ControlPoint, CurveError, ExpoCurveConfig, InterpolationMode, ResponseCurve,
};
use flight_axis::deadzone::{
    AsymmetricDeadzoneConfig, DeadzoneConfig, DeadzoneError, DeadzoneProcessor,
};
use flight_axis::mixer::{AxisMixer, MixMode, MAX_MIXER_INPUTS};
use flight_axis::normalize::{AxisNormalizer, NormalizeConfig, NormalizerBank};

// ── Deadzone: boundary mutations ─────────────────────────────────────────

#[test]
fn deadzone_boundary_exact_center_is_zero() {
    // Catches < vs <= mutation on `abs <= center`
    let config = DeadzoneConfig::center_only(0.1).unwrap();
    let proc = DeadzoneProcessor::new(config);
    assert_eq!(proc.apply(0.1), 0.0, "exactly at center boundary must be zero");
    assert_eq!(
        proc.apply(-0.1),
        0.0,
        "negative exactly at center boundary must be zero"
    );
}

#[test]
fn deadzone_just_above_boundary_nonzero() {
    // Catches <= vs < mutation: value just above must produce output
    let config = DeadzoneConfig::center_only(0.1).unwrap();
    let proc = DeadzoneProcessor::new(config);
    let v = proc.apply(0.100001);
    assert!(v > 0.0, "just above center must be positive, got {v}");
}

#[test]
fn deadzone_sign_correct_for_positive_and_negative() {
    // Catches sign flip mutation on `if input >= 0.0`
    let config = DeadzoneConfig::center_only(0.05).unwrap();
    let proc = DeadzoneProcessor::new(config);
    let pos = proc.apply(0.5);
    let neg = proc.apply(-0.5);
    assert!(pos > 0.0, "positive input must give positive output");
    assert!(neg < 0.0, "negative input must give negative output");
    assert!(
        (pos + neg).abs() < 1e-6,
        "must be antisymmetric: {pos} + {neg}"
    );
}

#[test]
fn deadzone_config_validation_boundaries() {
    // Catches mutation of range bounds in DeadzoneConfig::new
    // center = 0.0 is valid (lower bound inclusive)
    assert!(DeadzoneConfig::new(0.0, 0.0).is_ok());
    // center = 0.5 is invalid (upper bound exclusive)
    assert_eq!(
        DeadzoneConfig::new(0.5, 0.0),
        Err(DeadzoneError::InvalidCenter)
    );
    // center just below 0.5 is valid
    assert!(DeadzoneConfig::new(0.499, 0.0).is_ok());
    // Negative center is invalid
    assert_eq!(
        DeadzoneConfig::new(-0.01, 0.0),
        Err(DeadzoneError::InvalidCenter)
    );
    // edge = 0.5 is invalid
    assert_eq!(
        DeadzoneConfig::new(0.0, 0.5),
        Err(DeadzoneError::InvalidEdge)
    );
}

#[test]
fn deadzone_overlap_checked_before_individual() {
    // Catches mutation in validation order
    assert_eq!(
        DeadzoneConfig::new(0.5, 0.5),
        Err(DeadzoneError::Overlap),
        "center+edge >= 1.0 should be Overlap"
    );
    assert_eq!(
        DeadzoneConfig::new(0.99, 0.01),
        Err(DeadzoneError::Overlap),
        "center+edge == 1.0 should be Overlap"
    );
}

#[test]
fn deadzone_rescaling_formula_correct() {
    // Catches arithmetic mutations: (abs - center) / (1 - center - edge)
    let config = DeadzoneConfig::new(0.1, 0.1).unwrap();
    let proc = DeadzoneProcessor::new(config);
    // Input 0.5: (0.5 - 0.1) / (1.0 - 0.1 - 0.1) = 0.4 / 0.8 = 0.5
    let v = proc.apply(0.5);
    assert!(
        (v - 0.5).abs() < 1e-5,
        "rescaling formula: expected 0.5, got {v}"
    );
}

// ── AsymmetricDeadzone: boundary mutations ───────────────────────────────

#[test]
fn asymmetric_deadzone_exact_positive_boundary() {
    // Catches < vs <= on `value < self.positive`
    let cfg = AsymmetricDeadzoneConfig::new(0.2, 0.0);
    // At boundary: (0.2 - 0.2) / 0.8 = 0.0
    assert!(
        cfg.apply(0.2).abs() < 1e-6,
        "at positive boundary should be ~0"
    );
    // Just below: still in deadzone
    assert_eq!(cfg.apply(0.19), 0.0, "below positive boundary must be 0");
}

#[test]
fn asymmetric_deadzone_exact_negative_boundary() {
    // Catches > vs >= on `value > -self.negative`
    let cfg = AsymmetricDeadzoneConfig::new(0.0, 0.2);
    // At boundary: (-0.2 + 0.2) / 0.8 = 0.0
    assert!(
        cfg.apply(-0.2).abs() < 1e-6,
        "at negative boundary should be ~0"
    );
    // Just above (less negative): still in deadzone
    assert_eq!(cfg.apply(-0.19), 0.0, "above negative boundary must be 0");
}

#[test]
fn asymmetric_deadzone_zero_input_goes_positive_path() {
    // Catches >= vs > on `value >= 0.0` (zero should go to positive branch)
    let cfg = AsymmetricDeadzoneConfig::new(0.1, 0.5);
    assert_eq!(cfg.apply(0.0), 0.0, "zero input must be in positive deadzone");
}

// ── Curve: boundary & arithmetic mutations ───────────────────────────────

#[test]
fn curve_evaluate_at_control_points_returns_exact_y() {
    // Catches off-by-one in segment finding (x <= w[1].x)
    let curve = ResponseCurve::from_points(
        vec![
            ControlPoint::new(0.0, 0.0),
            ControlPoint::new(0.5, 0.7),
            ControlPoint::new(1.0, 1.0),
        ],
        InterpolationMode::Linear,
    )
    .unwrap();

    assert!((curve.evaluate(0.0) - 0.0).abs() < 1e-5, "at first point");
    assert!((curve.evaluate(0.5) - 0.7).abs() < 1e-5, "at middle point");
    assert!((curve.evaluate(1.0) - 1.0).abs() < 1e-5, "at last point");
}

#[test]
fn curve_duplicate_x_values_rejected() {
    // Catches <= vs < mutation on `points[i].x <= points[i-1].x`
    let result = ResponseCurve::from_points(
        vec![
            ControlPoint::new(0.0, 0.0),
            ControlPoint::new(0.5, 0.3),
            ControlPoint::new(0.5, 0.7), // duplicate x
            ControlPoint::new(1.0, 1.0),
        ],
        InterpolationMode::Linear,
    );
    assert_eq!(result.unwrap_err(), CurveError::NotSortedByX);
}

#[test]
fn curve_point_at_exact_boundary_valid() {
    // Catches mutation on range check !(0.0..=1.0).contains
    assert!(ResponseCurve::from_points(
        vec![ControlPoint::new(0.0, 0.0), ControlPoint::new(1.0, 1.0)],
        InterpolationMode::Linear,
    )
    .is_ok());

    // Just outside
    assert!(ResponseCurve::from_points(
        vec![ControlPoint::new(-0.001, 0.0), ControlPoint::new(1.0, 1.0)],
        InterpolationMode::Linear,
    )
    .is_err());

    assert!(ResponseCurve::from_points(
        vec![ControlPoint::new(0.0, 0.0), ControlPoint::new(1.001, 1.0)],
        InterpolationMode::Linear,
    )
    .is_err());
}

#[test]
fn curve_is_monotone_detects_decrease() {
    // Catches >= vs > mutation on `w[1].y >= w[0].y`
    let curve = ResponseCurve::from_points(
        vec![
            ControlPoint::new(0.0, 0.5),
            ControlPoint::new(0.5, 0.3), // decreases
            ControlPoint::new(1.0, 1.0),
        ],
        InterpolationMode::Linear,
    )
    .unwrap();
    assert!(!curve.is_monotone(), "decreasing segment must not be monotone");

    // Equal y values should still be monotone
    let flat = ResponseCurve::from_points(
        vec![
            ControlPoint::new(0.0, 0.5),
            ControlPoint::new(0.5, 0.5),
            ControlPoint::new(1.0, 0.5),
        ],
        InterpolationMode::Linear,
    )
    .unwrap();
    assert!(flat.is_monotone(), "flat curve should be monotone");
}

// ── Expo curve: formula verification ─────────────────────────────────────

#[test]
fn expo_formula_exact_values() {
    // expo * v³ + (1 - expo) * v
    // For expo=0.5, v=0.5: 0.5*0.125 + 0.5*0.5 = 0.0625 + 0.25 = 0.3125
    let expo = ExpoCurveConfig::new(0.5);
    let v = expo.apply(0.5);
    assert!(
        (v - 0.3125).abs() < 1e-6,
        "expo formula: expected 0.3125, got {v}"
    );
}

#[test]
fn expo_full_deflection_unaffected() {
    // At v=1.0: e*1 + (1-e)*1 = 1.0 for any expo
    // At v=-1.0: e*(-1) + (1-e)*(-1) = -1.0 for any expo
    for e in [-1.0_f32, -0.5, 0.0, 0.5, 1.0] {
        let expo = ExpoCurveConfig::new(e);
        assert!(
            (expo.apply(1.0) - 1.0).abs() < 1e-5,
            "expo={e}: apply(1.0) should be 1.0"
        );
        assert!(
            (expo.apply(-1.0) + 1.0).abs() < 1e-5,
            "expo={e}: apply(-1.0) should be -1.0"
        );
    }
}

// ── Normalize: boundary & counter mutations ──────────────────────────────

#[test]
fn normalize_exact_boundaries_not_clamped() {
    // Catches mutation on range check `(-1.0..=1.0).contains`
    let mut n = AxisNormalizer::new(NormalizeConfig::default());
    assert_eq!(n.process(1.0), 1.0);
    assert_eq!(n.process(-1.0), -1.0);
    assert_eq!(n.clamp_count(), 0, "boundary values must not increment clamp counter");
}

#[test]
fn normalize_just_outside_boundary_is_clamped() {
    let mut n = AxisNormalizer::new(NormalizeConfig::default());
    assert_eq!(n.process(1.0001), 1.0);
    assert_eq!(n.clamp_count(), 1);
    assert_eq!(n.process(-1.0001), -1.0);
    assert_eq!(n.clamp_count(), 2);
}

#[test]
fn normalize_nan_counter_increments_by_one() {
    // Catches += 0 or += 2 mutation
    let mut n = AxisNormalizer::new(NormalizeConfig::default());
    n.process(f32::NAN);
    assert_eq!(n.nan_count(), 1);
    n.process(f32::INFINITY);
    assert_eq!(n.nan_count(), 2);
}

#[test]
fn normalize_bank_aggregates_correctly() {
    let mut bank: NormalizerBank<2> = NormalizerBank::new(NormalizeConfig::default());
    let inputs = [f32::NAN, 2.0];
    let mut outputs = [0.0f32; 2];
    bank.process(&inputs, &mut outputs);

    assert_eq!(outputs[0], 0.0);
    assert_eq!(outputs[1], 1.0);
    assert_eq!(bank.total_nan_count(), 1);
    assert_eq!(bank.total_clamp_count(), 1);
}

// ── Mixer: tie-breaking, mode, arithmetic mutations ──────────────────────

#[test]
fn mixer_priority_first_wins_on_tie() {
    // Catches > vs >= mutation: equal weights should pick first input
    let mixer = AxisMixer::with_weights(MixMode::Priority, &[5.0, 5.0, 5.0]);
    let result = mixer.combine(&[0.1, 0.9, 0.5]);
    assert!(
        (result - 0.1).abs() < 1e-10,
        "equal weights must pick first: got {result}"
    );
}

#[test]
fn mixer_weighted_sum_clamp_boundary() {
    // Catches removal of .clamp(-1.0, 1.0)
    let mixer = AxisMixer::with_weights(MixMode::WeightedSum, &[1.0, 1.0]);
    assert_eq!(
        mixer.combine(&[0.6, 0.6]),
        1.0,
        "sum > 1.0 must clamp to 1.0"
    );
    assert_eq!(
        mixer.combine(&[-0.6, -0.6]),
        -1.0,
        "sum < -1.0 must clamp to -1.0"
    );
}

#[test]
fn mixer_max_returns_actual_maximum() {
    // Catches mutation where Max picks Min or vice versa
    let mixer = AxisMixer::with_weights(MixMode::Max, &[1.0, 1.0, 1.0]);
    assert!(
        (mixer.combine(&[0.1, 0.9, 0.5]) - 0.9).abs() < 1e-10,
        "Max must pick 0.9"
    );

    let min_mixer = AxisMixer::with_weights(MixMode::Min, &[1.0, 1.0, 1.0]);
    assert!(
        (min_mixer.combine(&[0.1, 0.9, 0.5]) - 0.1).abs() < 1e-10,
        "Min must pick 0.1"
    );
}

#[test]
fn mixer_add_input_returns_false_at_max() {
    // Catches >= vs > on `self.count >= MAX_MIXER_INPUTS`
    let mut mixer = AxisMixer::new(MixMode::WeightedSum);
    for _ in 0..MAX_MIXER_INPUTS {
        assert!(mixer.add_input(1.0), "should accept up to MAX");
    }
    assert!(
        !mixer.add_input(1.0),
        "must reject beyond MAX_MIXER_INPUTS"
    );
    assert_eq!(mixer.input_count(), MAX_MIXER_INPUTS);
}

#[test]
fn mixer_set_weight_boundary() {
    // Catches >= vs > on `index >= self.count`
    let mut mixer = AxisMixer::with_weights(MixMode::WeightedSum, &[1.0, 1.0]);
    assert!(mixer.set_weight(0, 2.0), "index 0 valid");
    assert!(mixer.set_weight(1, 2.0), "index 1 valid");
    assert!(!mixer.set_weight(2, 2.0), "index 2 invalid (count=2)");
}

#[test]
fn mixer_n_uses_min_of_values_and_count() {
    // Catches .min() vs .max() mutation
    let mixer = AxisMixer::with_weights(MixMode::WeightedSum, &[1.0, 1.0]);
    // Only 2 inputs configured; providing 5 values should use only first 2
    let result = mixer.combine(&[0.1, 0.2, 0.3, 0.4, 0.5]);
    assert!(
        (result - 0.3).abs() < 1e-10,
        "should only sum first 2 values: 0.1+0.2=0.3, got {result}"
    );
}
