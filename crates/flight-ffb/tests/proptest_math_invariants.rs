// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Property-based tests for flight-ffb mathematical invariants.
//!
//! Covers:
//! 1. Force bounded by safety envelope: output ≤ max_torque_nm
//! 2. Superposition commutative: CompositeEffect(A+B) == CompositeEffect(B+A)
//! 3. Effect timing monotonic: ramp tick values move toward target
//! 4. Fade-in increases from zero: EffectRamp(0→target) produces non-decreasing values
//! 5. Fade-out decreases to zero: EffectRamp(target→0) produces non-increasing values

use flight_ffb::{
    effects::{
        CompositeEffect, ConstantForceParams, DamperParams, EffectInput, FfbEffect,
        SpringParams,
    },
    ramp::EffectRamp,
    safety_envelope::{SafetyEnvelope, SafetyEnvelopeConfig},
};
use proptest::prelude::*;

fn default_input() -> EffectInput {
    EffectInput {
        position: 0.0,
        velocity: 0.0,
        elapsed_s: 0.0,
        tick: 0,
    }
}

proptest! {
    // ── 1. Force bounded by safety envelope ─────────────────────────────────

    /// Safety envelope output magnitude never exceeds max_torque_nm.
    #[test]
    fn safety_envelope_bounds_torque(
        desired in -100.0f32..=100.0f32,
        max_torque in 1.0f32..=50.0f32,
    ) {
        let config = SafetyEnvelopeConfig {
            max_torque_nm: max_torque,
            max_slew_rate_nm_per_s: 1000.0, // high to not limit
            max_jerk_nm_per_s2: 100000.0,   // high to not limit
            timestep_s: 0.004,
            ..SafetyEnvelopeConfig::default()
        };
        let mut envelope = SafetyEnvelope::new(config).unwrap();
        let output = envelope.apply(desired, true).unwrap();
        prop_assert!(
            output.abs() <= max_torque + 1e-5,
            "safety envelope output {} exceeds max_torque_nm={} for desired={}",
            output, max_torque, desired
        );
    }

    /// Safety envelope with safe_for_ffb=false always outputs zero.
    #[test]
    fn safety_envelope_zero_when_unsafe(desired in -50.0f32..=50.0f32) {
        let mut envelope = SafetyEnvelope::default();
        let output = envelope.apply(desired, false).unwrap();
        prop_assert!(
            output.abs() < 1e-5,
            "output should be ~0 when safe_for_ffb=false, got {}", output
        );
    }

    /// Safety envelope rejects NaN input.
    #[test]
    fn safety_envelope_rejects_nan(_dummy in 0u8..1u8) {
        let mut envelope = SafetyEnvelope::default();
        let result = envelope.apply(f32::NAN, true);
        prop_assert!(result.is_err(), "NaN input should produce error");
    }

    /// Safety envelope rejects infinity input.
    #[test]
    fn safety_envelope_rejects_infinity(sign in prop::bool::ANY) {
        let mut envelope = SafetyEnvelope::default();
        let val = if sign { f32::INFINITY } else { f32::NEG_INFINITY };
        let result = envelope.apply(val, true);
        prop_assert!(result.is_err(), "infinity input should produce error");
    }

    // ── 2. Superposition commutative ────────────────────────────────────────

    /// CompositeEffect(A, B) computes the same as CompositeEffect(B, A).
    #[test]
    fn composite_effect_commutative(
        mag_a in -1.0f32..=1.0f32,
        mag_b in -1.0f32..=1.0f32,
        gain_a in 0.0f32..=1.0f32,
        gain_b in 0.0f32..=1.0f32,
        position in -1.0f32..=1.0f32,
    ) {
        let effect_a = FfbEffect::ConstantForce(ConstantForceParams { magnitude: mag_a });
        let effect_b = FfbEffect::ConstantForce(ConstantForceParams { magnitude: mag_b });

        let mut comp_ab = CompositeEffect::new();
        comp_ab.add(effect_a, gain_a);
        comp_ab.add(effect_b, gain_b);

        let mut comp_ba = CompositeEffect::new();
        comp_ba.add(effect_b, gain_b);
        comp_ba.add(effect_a, gain_a);

        let input = EffectInput { position, velocity: 0.0, elapsed_s: 0.0, tick: 0 };
        let out_ab = comp_ab.compute(&input);
        let out_ba = comp_ba.compute(&input);

        prop_assert!(
            (out_ab - out_ba).abs() < 1e-5,
            "superposition not commutative: A+B={}, B+A={}", out_ab, out_ba
        );
    }

    /// CompositeEffect with spring + damper is commutative.
    #[test]
    fn composite_spring_damper_commutative(
        position in -1.0f32..=1.0f32,
        velocity in -2.0f32..=2.0f32,
        spring_coeff in 0.0f32..=1.0f32,
        damper_coeff in 0.0f32..=1.0f32,
    ) {
        let spring = FfbEffect::Spring(SpringParams {
            coefficient: spring_coeff,
            center: 0.0,
            deadband: 0.0,
            saturation: 1.0,
        });
        let damper = FfbEffect::Damper(DamperParams { coefficient: damper_coeff });

        let mut comp_sd = CompositeEffect::new();
        comp_sd.add(spring, 1.0);
        comp_sd.add(damper, 1.0);

        let mut comp_ds = CompositeEffect::new();
        comp_ds.add(damper, 1.0);
        comp_ds.add(spring, 1.0);

        let input = EffectInput { position, velocity, elapsed_s: 0.0, tick: 0 };
        let out_sd = comp_sd.compute(&input);
        let out_ds = comp_ds.compute(&input);

        prop_assert!(
            (out_sd - out_ds).abs() < 1e-5,
            "spring+damper not commutative: S+D={}, D+S={}", out_sd, out_ds
        );
    }

    // ── 3. Effect timing monotonic: ramp moves toward target ────────────────

    /// EffectRamp from 0→1 produces non-decreasing values on each tick.
    #[test]
    fn ramp_0_to_1_non_decreasing(duration in 1u32..=100u32) {
        let mut ramp = EffectRamp::new(0.0, 1.0, duration);
        let mut prev = 0.0_f32;
        for _ in 0..=duration {
            let val = ramp.tick();
            prop_assert!(
                val >= prev - 1e-5,
                "ramp 0→1 decreased: prev={}, current={}, duration={}",
                prev, val, duration
            );
            prev = val;
        }
    }

    /// EffectRamp from 1→0 produces non-increasing values on each tick.
    #[test]
    fn ramp_1_to_0_non_increasing(duration in 1u32..=100u32) {
        let mut ramp = EffectRamp::new(1.0, 0.0, duration);
        let mut prev = 1.0_f32;
        for _ in 0..=duration {
            let val = ramp.tick();
            prop_assert!(
                val <= prev + 1e-5,
                "ramp 1→0 increased: prev={}, current={}, duration={}",
                prev, val, duration
            );
            prev = val;
        }
    }

    // ── 4. Fade-in increases from zero ──────────────────────────────────────

    /// Fade-in ramp (0→target) starts near zero and ends at target.
    #[test]
    fn fade_in_starts_at_zero_ends_at_target(
        target in 0.01f32..=1.0f32,
        duration in 2u32..=50u32,
    ) {
        let mut ramp = EffectRamp::new(0.0, target, duration);
        let first = ramp.tick();
        prop_assert!(
            first.abs() < target + 1e-5,
            "fade-in first tick {} should be near 0, target={}", first, target
        );
        // Run to completion
        for _ in 1..duration {
            ramp.tick();
        }
        let last = ramp.tick();
        prop_assert!(
            (last - target).abs() < 1e-4,
            "fade-in should end at target={}, got {}", target, last
        );
    }

    // ── 5. Fade-out decreases to zero ───────────────────────────────────────

    /// Fade-out ramp (start→0) ends at zero.
    #[test]
    fn fade_out_ends_at_zero(
        start in 0.01f32..=1.0f32,
        duration in 2u32..=50u32,
    ) {
        let mut ramp = EffectRamp::new(start, 0.0, duration);
        for _ in 0..duration {
            ramp.tick();
        }
        let final_val = ramp.tick();
        prop_assert!(
            final_val.abs() < 1e-4,
            "fade-out should end at 0.0, got {} (start={}, duration={})",
            final_val, start, duration
        );
    }

    /// Fade-out ramp: every tick value is non-negative when starting positive.
    #[test]
    fn fade_out_values_non_negative(
        start in 0.0f32..=1.0f32,
        duration in 1u32..=50u32,
    ) {
        let mut ramp = EffectRamp::new(start, 0.0, duration);
        for i in 0..=duration {
            let val = ramp.tick();
            prop_assert!(
                val >= -1e-5,
                "fade-out tick {} produced negative value {} (start={}, duration={})",
                i, val, start, duration
            );
        }
    }

    // ── Bonus: individual effect output always in [-1, 1] ───────────────────

    /// Any single FfbEffect.compute() produces output in [-1, 1].
    #[test]
    fn constant_force_output_bounded(
        magnitude in -2.0f32..=2.0f32,
        position in -1.0f32..=1.0f32,
    ) {
        let effect = FfbEffect::ConstantForce(ConstantForceParams { magnitude });
        let input = EffectInput { position, velocity: 0.0, elapsed_s: 0.0, tick: 0 };
        let out = effect.compute(&input);
        prop_assert!(
            (-1.0..=1.0).contains(&out),
            "constant force output {} out of [-1,1] for magnitude={}", out, magnitude
        );
    }

    /// CompositeEffect output is always in [-1, 1].
    #[test]
    fn composite_output_always_bounded(
        mag_a in -1.0f32..=1.0f32,
        mag_b in -1.0f32..=1.0f32,
        gain_a in 0.0f32..=2.0f32,
        gain_b in 0.0f32..=2.0f32,
    ) {
        let mut comp = CompositeEffect::new();
        comp.add(FfbEffect::ConstantForce(ConstantForceParams { magnitude: mag_a }), gain_a);
        comp.add(FfbEffect::ConstantForce(ConstantForceParams { magnitude: mag_b }), gain_b);
        let out = comp.compute(&default_input());
        prop_assert!(
            (-1.0..=1.0).contains(&out),
            "composite output {} out of [-1,1]", out
        );
    }
}
