// SPDX-License-Identifier: MIT OR Apache-2.0
// Mutation-killing tests: each test asserts a SPECIFIC VALUE that would break
// if a mutant flipped a sign, changed a comparison, or swapped an operator.

use flight_axis::curve::ExpoCurveConfig;
use flight_axis::deadzone::{DeadzoneConfig, DeadzoneProcessor};
use flight_axis::detent::{DetentConfig, DetentProcessor};
use flight_axis::mixer::{AxisMixer, MixMode};
use flight_axis::stages::{
    CurveType, DeadzoneShape, RtAxisPipeline, SaturationStage, Stage,
};

const TOL: f32 = 1e-5;
const TOL64: f64 = 1e-10;

// ── 1. deadzone_boundary_exact_value ──────────────────────────────────────

#[test]
fn deadzone_boundary_exact_value() {
    // DeadzoneProcessor uses f32 and `abs <= center` → 0.0
    let config = DeadzoneConfig::center_only(0.05).unwrap();
    let proc = DeadzoneProcessor::new(config);

    // Exactly at the boundary: must collapse to 0.0
    // A mutant changing `<=` to `<` would return a non-zero value here.
    let at_boundary = proc.apply(0.05);
    assert_eq!(at_boundary, 0.0, "input exactly at boundary must be 0.0");

    // Just outside the boundary: must be a small positive value (rescaled)
    let just_outside = proc.apply(0.051);
    assert!(
        just_outside > 0.0,
        "input just outside boundary must be > 0.0, got {just_outside}"
    );
    // Expected: (0.051 - 0.05) / (1.0 - 0.05) ≈ 0.001 / 0.95 ≈ 0.001053
    assert!(
        (just_outside - 0.001_053).abs() < 0.001,
        "just_outside should be ~0.00105, got {just_outside}"
    );
}

// ── 2. curve_endpoints_exact ──────────────────────────────────────────────

#[test]
fn curve_endpoints_exact() {
    // ExpoCurveConfig::apply uses: expo * v³ + (1 − expo) * v
    // For ANY expo, apply(1.0) = expo*1 + (1-expo)*1 = 1.0
    // For ANY expo, apply(-1.0) = expo*(-1) + (1-expo)*(-1) = -1.0
    // For ANY expo, apply(0.0) = 0.0
    let expo = ExpoCurveConfig::new(0.5);

    let at_one = expo.apply(1.0);
    assert!(
        (at_one - 1.0).abs() < TOL,
        "apply(1.0) must equal 1.0, got {at_one}"
    );

    let at_neg_one = expo.apply(-1.0);
    assert!(
        (at_neg_one - (-1.0)).abs() < TOL,
        "apply(-1.0) must equal -1.0, got {at_neg_one}"
    );

    let at_zero = expo.apply(0.0);
    assert!(
        at_zero.abs() < TOL,
        "apply(0.0) must equal 0.0, got {at_zero}"
    );

    // Also verify a mid-range value: expo=0.5, v=0.5
    // 0.5 * 0.125 + 0.5 * 0.5 = 0.0625 + 0.25 = 0.3125
    let at_half = expo.apply(0.5);
    assert!(
        (at_half - 0.3125).abs() < TOL,
        "apply(0.5) with expo=0.5 must be 0.3125, got {at_half}"
    );
}

// ── 3. mixer_coefficient_sign_matters ─────────────────────────────────────

#[test]
fn mixer_coefficient_sign_matters() {
    // WeightedSum: sum = 1.0*0.6 + (-1.0)*0.4 = 0.6 - 0.4 = 0.2
    // Negating the second weight sign would give 1.0*0.6 + 1.0*0.4 = 1.0
    let mixer = AxisMixer::with_weights(MixMode::WeightedSum, &[1.0, -1.0]);
    let result = mixer.combine(&[0.6, 0.4]);

    assert!(
        (result - 0.2).abs() < TOL64,
        "weighted sum [1.0,-1.0] * [0.6,0.4] must be 0.2, got {result}"
    );

    // Verify the mutation scenario: if someone negated the -1.0 weight to +1.0
    let mutant_mixer = AxisMixer::with_weights(MixMode::WeightedSum, &[1.0, 1.0]);
    let mutant_result = mutant_mixer.combine(&[0.6, 0.4]);
    assert!(
        (mutant_result - 1.0).abs() < TOL64,
        "sanity: equal weights should give 1.0, got {mutant_result}"
    );
    assert!(
        (result - mutant_result).abs() > 0.5,
        "correct and mutant results must differ significantly"
    );
}

// ── 4. detent_snap_threshold_exact ────────────────────────────────────────

#[test]
fn detent_snap_threshold_exact() {
    // DetentProcessor::apply uses `(input - position).abs() <= snap_range`
    let config = DetentConfig::new().add(0.5, 0.03, "mid");
    let mut proc = DetentProcessor::new(config);

    // Input 0.53: distance = |0.53 - 0.5| = 0.03 <= 0.03 → snaps to 0.5
    let snapped = proc.apply(0.53);
    assert_eq!(
        snapped, 0.5,
        "input 0.53 within snap_range 0.03 of detent 0.5 must snap, got {snapped}"
    );

    // Input 0.54: distance = |0.54 - 0.5| = 0.04 > 0.03 → pass through
    let passthrough = proc.apply(0.54);
    assert!(
        (passthrough - 0.54).abs() < TOL,
        "input 0.54 outside snap_range must pass through as 0.54, got {passthrough}"
    );
}

// ── 5. pipeline_ordering_matters ──────────────────────────────────────────

#[test]
fn pipeline_ordering_matters() {
    // Order A: deadzone → expo curve
    let mut pipeline_a = RtAxisPipeline::builder()
        .deadzone(0.0, 0.1, DeadzoneShape::Linear)
        .curve(CurveType::Expo(1.0))
        .build();

    // Order B: expo curve → deadzone
    let mut pipeline_b = RtAxisPipeline::builder()
        .curve(CurveType::Expo(1.0))
        .deadzone(0.0, 0.1, DeadzoneShape::Linear)
        .build();

    let result_a = pipeline_a.process(0.5);
    let result_b = pipeline_b.process(0.5);

    // Both should produce valid output, but different values
    assert!(result_a.is_finite(), "pipeline A must produce finite output");
    assert!(result_b.is_finite(), "pipeline B must produce finite output");
    assert!(
        (result_a - result_b).abs() > 1e-6,
        "different stage ordering must produce different results: A={result_a}, B={result_b}"
    );
}

// ── 6. stage_enable_disable_changes_output ────────────────────────────────

#[test]
fn stage_enable_disable_changes_output() {
    // Pipeline with deadzone (width=0.1): input 0.05 is inside deadzone → 0.0
    let mut pipeline = RtAxisPipeline::builder()
        .deadzone(0.0, 0.1, DeadzoneShape::Linear)
        .build();

    let with_deadzone = pipeline.process(0.05);
    assert_eq!(
        with_deadzone, 0.0,
        "input 0.05 inside deadzone(width=0.1) must be 0.0, got {with_deadzone}"
    );

    // Remove the deadzone stage → input passes through unchanged
    pipeline.remove_stage(0);
    let without_deadzone = pipeline.process(0.05);
    assert!(
        (without_deadzone - 0.05).abs() < TOL64,
        "after removing deadzone, 0.05 must pass through as 0.05, got {without_deadzone}"
    );
}

// ── 7. zero_input_passthrough_all_stages ──────────────────────────────────

#[test]
fn zero_input_passthrough_all_stages() {
    // Pipeline: deadzone(center=0, width=0) + linear curve + bipolar saturation
    // Zero input must remain exactly zero through every stage.
    let mut pipeline = RtAxisPipeline::builder()
        .deadzone(0.0, 0.0, DeadzoneShape::Linear)
        .curve(CurveType::Linear)
        .saturation_bipolar()
        .build();

    let result = pipeline.process(0.0);
    assert_eq!(
        result, 0.0,
        "zero input through all stages must remain exactly 0.0, got {result}"
    );
}

// ── 8. saturation_clips_to_exact_bounds ───────────────────────────────────

#[test]
fn saturation_clips_to_exact_bounds() {
    let mut sat = SaturationStage::bipolar();

    // Above upper bound → clamp to 1.0
    let over = sat.process(1.5);
    assert_eq!(over, 1.0, "1.5 must clamp to 1.0, got {over}");

    // Below lower bound → clamp to -1.0
    let under = sat.process(-1.5);
    assert_eq!(under, -1.0, "-1.5 must clamp to -1.0, got {under}");

    // Within bounds → pass through unchanged
    let mid = sat.process(0.5);
    assert!(
        (mid - 0.5).abs() < TOL64,
        "0.5 within bounds must pass through as 0.5, got {mid}"
    );

    // NaN → must return 0.0 (SaturationStage checks is_nan first)
    let nan_out = sat.process(f64::NAN);
    assert_eq!(
        nan_out, 0.0,
        "NaN input must return 0.0, got {nan_out}"
    );
}
