// SPDX-License-Identifier: MIT OR Apache-2.0
// Targeted mutation-killing tests for flight-ffb.
// Each test asserts exact values to catch sign-flip, off-by-one, and logic mutations.

use flight_ffb::effects::{
    CompositeEffect, ConstantForceParams, EffectInput, EffectWatchdog, FfbEffect,
    SpringParams,
};
use flight_ffb::ramp::EffectRamp;
use flight_ffb::safety_envelope::{SafetyEnvelope, SafetyEnvelopeConfig};
use std::time::Duration;

fn zero_input() -> EffectInput {
    EffectInput {
        position: 0.0,
        velocity: 0.0,
        elapsed_s: 0.0,
        tick: 0,
    }
}

/// Kills sign-flip mutations in the spring formula.
/// Spring force = -coefficient * (|displacement| - deadband) * signum(displacement)
/// position=+0.5 → force = -0.5 * 0.5 * 1.0 = -0.25
/// position=-0.5 → force = -0.5 * 0.5 * (-1.0) = +0.25
#[test]
fn spring_force_direction_correct() {
    let spring = FfbEffect::Spring(SpringParams {
        coefficient: 0.5,
        center: 0.0,
        deadband: 0.0,
        saturation: 1.0,
    });

    // Positive displacement → negative (restoring) force
    let input_pos = EffectInput {
        position: 0.5,
        ..zero_input()
    };
    let force_pos = spring.compute(&input_pos);
    assert!(
        (force_pos - (-0.25)).abs() < f32::EPSILON,
        "expected -0.25, got {force_pos}"
    );

    // Negative displacement → positive (restoring) force
    let input_neg = EffectInput {
        position: -0.5,
        ..zero_input()
    };
    let force_neg = spring.compute(&input_neg);
    assert!(
        (force_neg - 0.25).abs() < f32::EPSILON,
        "expected +0.25, got {force_neg}"
    );

    // Opposite signs — any sign mutation breaks this
    assert!(force_pos < 0.0, "positive displacement must yield negative force");
    assert!(force_neg > 0.0, "negative displacement must yield positive force");
}

/// Kills mutations that remove torque clamping or break the safe_for_ffb gate.
/// With large slew/jerk limits the envelope should ramp up to max_torque quickly.
/// safe_for_ffb=false must always yield 0.0.
#[test]
fn safety_envelope_clamps_torque() {
    let config = SafetyEnvelopeConfig {
        max_torque_nm: 10.0,
        max_slew_rate_nm_per_s: 100_000.0, // effectively unlimited
        max_jerk_nm_per_s2: 100_000_000.0, // effectively unlimited
        fault_ramp_time: Duration::from_millis(50),
        timestep_s: 0.004,
    };
    let mut envelope = SafetyEnvelope::new(config).unwrap();

    // Repeatedly apply 15.0 to let the envelope ramp up past any rate limits
    let mut last_torque = 0.0_f32;
    for _ in 0..200 {
        last_torque = envelope.apply(15.0, true).unwrap();
    }
    // Output must be clamped to max_torque_nm
    assert!(
        last_torque <= 10.0,
        "torque {last_torque} must not exceed max_torque_nm=10.0"
    );
    assert!(
        (last_torque - 10.0).abs() < 0.01,
        "torque should have reached 10.0 after enough ticks, got {last_torque}"
    );

    // safe_for_ffb=false → output must be 0.0 (after enough ticks to ramp down)
    let mut env2 = SafetyEnvelope::new(SafetyEnvelopeConfig {
        max_torque_nm: 10.0,
        max_slew_rate_nm_per_s: 100_000.0,
        max_jerk_nm_per_s2: 100_000_000.0,
        fault_ramp_time: Duration::from_millis(50),
        timestep_s: 0.004,
    })
    .unwrap();
    // With safe_for_ffb=false, target is 0.0 from the start
    let torque = env2.apply(15.0, false).unwrap();
    assert!(
        torque.abs() < f32::EPSILON,
        "safe_for_ffb=false must yield 0.0, got {torque}"
    );
}

/// Kills mutations that replace summation with last-wins or first-wins.
/// CompositeEffect must SUM individual effects: 0.3 + 0.2 = 0.5, not 0.2 or 0.3.
#[test]
fn effect_superposition_sums_not_replaces() {
    let input = zero_input();

    let mut composite = CompositeEffect::new();
    composite.add(
        FfbEffect::ConstantForce(ConstantForceParams { magnitude: 0.3 }),
        1.0,
    );
    composite.add(
        FfbEffect::ConstantForce(ConstantForceParams { magnitude: 0.2 }),
        1.0,
    );

    let result = composite.compute(&input);
    assert!(
        (result - 0.5).abs() < f32::EPSILON,
        "expected sum 0.5, got {result}"
    );

    // Adding a third effect: total = 0.3 + 0.2 + 0.1 = 0.6
    composite.add(
        FfbEffect::ConstantForce(ConstantForceParams { magnitude: 0.1 }),
        1.0,
    );
    let result3 = composite.compute(&input);
    assert!(
        (result3 - 0.6).abs() < 1e-6,
        "expected sum 0.6, got {result3}"
    );
}

/// Kills off-by-one mutations in EffectRamp tick progression.
/// Ramp(0.0→1.0, 10 ticks): tick() reads t=current_tick/duration THEN increments.
/// Call 0 → t=0/10=0.0, call 5 → t=5/10=0.5, after 10 calls → is_complete()=true & value=1.0.
#[test]
fn fade_in_timing_linear_progression() {
    let mut ramp = EffectRamp::new(0.0, 1.0, 10);

    // First tick: current_tick=0 → t=0.0 → value=0.0
    let v0 = ramp.tick();
    assert!(
        v0.abs() < f32::EPSILON,
        "tick 0 should return 0.0, got {v0}"
    );

    // Ticks 1..4 (4 more calls)
    for _ in 0..4 {
        ramp.tick();
    }

    // 6th call: current_tick=5 → t=5/10=0.5 → value=0.5
    let v5 = ramp.tick();
    assert!(
        (v5 - 0.5).abs() < f32::EPSILON,
        "tick 5 should return 0.5, got {v5}"
    );

    // Ticks 6..9 (4 more calls, total 10)
    for _ in 0..4 {
        ramp.tick();
    }

    // After 10 tick() calls, current_tick=10 which equals duration
    assert!(
        ramp.is_complete(),
        "ramp must be complete after 10 ticks"
    );
    // Next tick returns target since current_tick >= duration
    let v_final = ramp.tick();
    assert!(
        (v_final - 1.0).abs() < f32::EPSILON,
        "completed ramp must return target 1.0, got {v_final}"
    );
}

/// Kills mutations in CompositeEffect::clear and EffectWatchdog boundary.
/// 1) CompositeEffect with forces → non-zero; after clear() → 0.0.
/// 2) Watchdog(timeout=5): feed, tick 4 times → not tripped; tick once more → tripped.
///    Changing `>=` to `>` in the watchdog would make tick 5 not trip.
#[test]
fn emergency_stop_zeroes_all_forces() {
    let input = EffectInput {
        position: 0.3,
        velocity: 0.1,
        elapsed_s: 0.0,
        tick: 0,
    };

    // Build a composite with real forces
    let mut composite = CompositeEffect::new();
    composite.add(
        FfbEffect::ConstantForce(ConstantForceParams { magnitude: 0.8 }),
        1.0,
    );
    composite.add(
        FfbEffect::Spring(SpringParams {
            coefficient: 0.5,
            center: 0.0,
            deadband: 0.0,
            saturation: 1.0,
        }),
        1.0,
    );

    let before = composite.compute(&input);
    // ConstantForce=0.8, Spring at pos=0.3 → -0.5*0.3 = -0.15, total = 0.65
    assert!(
        (before - 0.65).abs() < f32::EPSILON,
        "pre-clear force should be 0.65, got {before}"
    );

    // After clear, compute must return 0.0
    composite.clear();
    assert!(composite.is_empty(), "composite must be empty after clear");
    let after = composite.compute(&input);
    assert!(
        after.abs() < f32::EPSILON,
        "cleared composite must return 0.0, got {after}"
    );

    // EffectWatchdog boundary test: timeout_ticks=5
    // feed() resets counter to 0. Trips when ticks_since_update >= timeout_ticks.
    let mut watchdog = EffectWatchdog::new(5);
    watchdog.feed();

    // 4 ticks: counter goes 1,2,3,4 — must NOT trip
    for i in 1..=4 {
        let tripped = watchdog.tick();
        assert!(
            !tripped,
            "watchdog must not trip after {i} ticks (timeout=5)"
        );
    }
    assert!(
        !watchdog.is_tripped(),
        "watchdog must not be tripped after 4 ticks"
    );

    // 5th tick: counter=5 >= timeout=5 → must trip
    let tripped = watchdog.tick();
    assert!(tripped, "watchdog must trip on tick 5 (>= timeout_ticks=5)");
    assert!(
        watchdog.is_tripped(),
        "is_tripped() must be true after 5 ticks"
    );
}
