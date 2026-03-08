// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for Brunner CLS-E FFB integration.
//!
//! Covers force calculation, effect synthesis, trim system, protocol format,
//! safety interlocks, and property-based invariants for the CLS-E yoke
//! (VID 0x25BB, PID 0x0063) operating as a force feedback device.
//!
//! The CLS-E is a professional-grade USB HID FFB base with 16-bit force
//! resolution and up to 500 Hz update rate, supporting spring, damper,
//! friction, constant force, and periodic effects.

use crate::{parse_cls_e_report, CLS_E_MIN_REPORT_BYTES};
use flight_ffb::effects::{
    CompositeEffect, ConstantForceParams, DamperParams, EffectInput, FfbEffect, ForceScaling,
    FrictionParams, PeriodicParams, RampParams, SpringParams, Waveform,
};
use flight_ffb::safety::{FaultReason, SafetyState, SafetyStateManager};
use flight_ffb::safety_envelope::{SafetyEnvelope, SafetyEnvelopeConfig};
use flight_ffb::trim::{SetpointChange, TrimController, TrimLimits, TrimMode, TrimOutput};

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Brunner CLS-E maximum torque: 15 Nm (professional FFB base).
const CLS_E_MAX_TORQUE_NM: f32 = 15.0;

/// CLS-E maximum slew rate for safety envelope (Nm/s).
const CLS_E_MAX_SLEW_RATE: f32 = 50.0;

/// CLS-E report ID byte.
const CLS_E_REPORT_ID: u8 = 0x01;

/// CLS-E VID.
const BRUNNER_VID: u16 = 0x25BB;

/// CLS-E PID.
const CLS_E_PID: u16 = 0x0063;

/// Build a minimal CLS-E HID input report.
fn make_report(roll: i16, pitch: i16, buttons: [u8; 4]) -> Vec<u8> {
    let mut data = vec![CLS_E_REPORT_ID];
    data.extend_from_slice(&roll.to_le_bytes());
    data.extend_from_slice(&pitch.to_le_bytes());
    data.extend_from_slice(&buttons);
    data
}

/// Convert normalised axis position (−1.0…+1.0) to i16 raw value.
fn axis_to_raw(normalised: f32) -> i16 {
    (normalised * 32767.0).round() as i16
}

/// Create an [`EffectInput`] for the given normalised CLS-E axis position.
fn cls_e_input(position: f32, velocity: f32, elapsed_s: f32) -> EffectInput {
    EffectInput {
        position,
        velocity,
        elapsed_s,
        tick: 0,
    }
}

/// Create a safety envelope configured for the CLS-E.
fn cls_e_safety_envelope() -> SafetyEnvelope {
    SafetyEnvelope::new(SafetyEnvelopeConfig {
        max_torque_nm: CLS_E_MAX_TORQUE_NM,
        max_slew_rate_nm_per_s: CLS_E_MAX_SLEW_RATE,
        max_jerk_nm_per_s2: 500.0,
        fault_ramp_time: std::time::Duration::from_millis(50),
        timestep_s: 0.002, // 500 Hz CLS-E update rate
    })
    .expect("CLS-E safety envelope config should be valid")
}

// ═══════════════════════════════════════════════════════════════════════════════
// 1. Force Calculation (8 tests)
// ═══════════════════════════════════════════════════════════════════════════════

/// Spring force: displaced CLS-E axis → restoring force toward center.
#[test]
fn force_calc_spring_restoring_force() {
    let spring = FfbEffect::Spring(SpringParams {
        coefficient: 0.8,
        center: 0.0,
        deadband: 0.0,
        saturation: 1.0,
    });
    // Yoke displaced 50% right → force should push left (negative).
    let input = cls_e_input(0.5, 0.0, 0.0);
    let force = spring.compute(&input);
    assert!(force < 0.0, "spring should push back toward center, got {force}");
    assert!(
        (force - -0.4).abs() < 1e-5,
        "spring force should be −coeff*displacement = −0.4, got {force}"
    );
}

/// Damper force: CLS-E axis velocity → opposing force.
#[test]
fn force_calc_damper_opposes_velocity() {
    let damper = FfbEffect::Damper(DamperParams { coefficient: 0.6 });
    let input = cls_e_input(0.0, 0.7, 0.0);
    let force = damper.compute(&input);
    assert!(
        force < 0.0,
        "damper should oppose positive velocity, got {force}"
    );
    assert!(
        (force - -0.42).abs() < 1e-5,
        "damper force = −0.6 × 0.7 = −0.42, got {force}"
    );
}

/// Friction force: constant magnitude opposing motion direction on the CLS-E.
#[test]
fn force_calc_friction_constant_magnitude() {
    let friction = FfbEffect::Friction(FrictionParams { coefficient: 0.5 });
    let slow = cls_e_input(0.0, 0.1, 0.0);
    let fast = cls_e_input(0.0, 0.9, 0.0);
    let f_slow = friction.compute(&slow).abs();
    let f_fast = friction.compute(&fast).abs();
    assert!(
        (f_slow - f_fast).abs() < 1e-5,
        "friction magnitude must be speed-independent: slow={f_slow}, fast={f_fast}"
    );
    assert!(
        (f_slow - 0.5).abs() < 1e-5,
        "friction magnitude should equal coefficient"
    );
}

/// Constant force applied through the CLS-E at a specific magnitude.
#[test]
fn force_calc_constant_force() {
    let constant = FfbEffect::ConstantForce(ConstantForceParams { magnitude: 0.75 });
    let input = cls_e_input(0.0, 0.0, 0.0);
    let force = constant.compute(&input);
    assert!(
        (force - 0.75).abs() < 1e-6,
        "constant force = 0.75, got {force}"
    );
}

/// Composite forces: spring + damper + constant blended on the CLS-E.
#[test]
fn force_calc_composite_forces() {
    let mut composite = CompositeEffect::new();
    composite.add(
        FfbEffect::Spring(SpringParams {
            coefficient: 0.5,
            center: 0.0,
            deadband: 0.0,
            saturation: 1.0,
        }),
        1.0,
    );
    composite.add(
        FfbEffect::Damper(DamperParams { coefficient: 0.3 }),
        1.0,
    );
    composite.add(
        FfbEffect::ConstantForce(ConstantForceParams { magnitude: 0.1 }),
        1.0,
    );

    // Yoke at position 0.4 moving right at velocity 0.2.
    let input = cls_e_input(0.4, 0.2, 0.0);
    let force = composite.compute(&input);
    // spring: −0.5 × 0.4 = −0.2, damper: −0.3 × 0.2 = −0.06, constant: +0.1
    let expected = -0.2 + -0.06 + 0.1;
    assert!(
        (force - expected).abs() < 1e-5,
        "composite force should be {expected}, got {force}"
    );
}

/// Force limits: output is clamped to ±1.0 even when effects would exceed.
#[test]
fn force_calc_force_limits_clamped() {
    let mut composite = CompositeEffect::new();
    // Two full-magnitude constant forces in the same direction → should clamp.
    composite.add(
        FfbEffect::ConstantForce(ConstantForceParams { magnitude: 0.8 }),
        1.0,
    );
    composite.add(
        FfbEffect::ConstantForce(ConstantForceParams { magnitude: 0.7 }),
        1.0,
    );
    let input = cls_e_input(0.0, 0.0, 0.0);
    let force = composite.compute(&input);
    assert!(
        force <= 1.0 + 1e-6,
        "composite output must be clamped to 1.0, got {force}"
    );
    assert!(
        (force - 1.0).abs() < 1e-6,
        "saturated composite should be exactly 1.0, got {force}"
    );
}

/// Slew rate limiting through the CLS-E safety envelope.
#[test]
fn force_calc_slew_rate_limiting() {
    let mut envelope = cls_e_safety_envelope();
    // First sample at zero → ok.
    let out0 = envelope.apply(0.0, true).unwrap();
    assert!(out0.abs() < 1e-4, "initial torque should be ~0");

    // Request a large step → envelope must slew-rate-limit.
    let out1 = envelope.apply(CLS_E_MAX_TORQUE_NM, true).unwrap();
    let delta = (out1 - out0).abs();
    let max_delta = CLS_E_MAX_SLEW_RATE * 0.002; // dt=2ms at 500 Hz
    assert!(
        delta <= max_delta + 1e-3,
        "torque step {delta} Nm exceeds slew limit {max_delta} Nm/step"
    );
}

/// Force scaling: global gain and per-axis gain applied to CLS-E output.
#[test]
fn force_calc_scaling_gain() {
    let scaling = ForceScaling {
        global_gain: 0.5,
        axis_gains: [0.8, 1.0], // pitch=0.8, roll=1.0
    };
    let raw_force = 1.0;
    let pitch_force = scaling.apply(raw_force, 0);
    let roll_force = scaling.apply(raw_force, 1);
    assert!(
        (pitch_force - 0.4).abs() < 1e-6,
        "pitch: 1.0 × 0.5 × 0.8 = 0.4, got {pitch_force}"
    );
    assert!(
        (roll_force - 0.5).abs() < 1e-6,
        "roll: 1.0 × 0.5 × 1.0 = 0.5, got {roll_force}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// 2. Effect Synthesis (8 tests)
// ═══════════════════════════════════════════════════════════════════════════════

/// Sine periodic effect on CLS-E at quarter period → peak force.
#[test]
fn effect_synth_sine_peak() {
    let effect = FfbEffect::Periodic(PeriodicParams {
        waveform: Waveform::Sine,
        frequency_hz: 10.0,
        amplitude: 0.8,
        phase_deg: 0.0,
        offset: 0.0,
    });
    // Quarter period at 10 Hz = 0.025 s.
    let input = cls_e_input(0.0, 0.0, 0.025);
    let force = effect.compute(&input);
    assert!(
        (force - 0.8).abs() < 1e-3,
        "sine peak should be amplitude 0.8, got {force}"
    );
}

/// Square wave effect: positive half-cycle returns +amplitude.
#[test]
fn effect_synth_square_wave() {
    let effect = FfbEffect::Periodic(PeriodicParams {
        waveform: Waveform::Square,
        frequency_hz: 5.0,
        amplitude: 0.6,
        phase_deg: 0.0,
        offset: 0.0,
    });
    // Early in first half-period (t < 0.1s) → positive.
    let input = cls_e_input(0.0, 0.0, 0.01);
    let force = effect.compute(&input);
    assert!(
        (force - 0.6).abs() < 1e-3,
        "square wave positive half should be 0.6, got {force}"
    );
}

/// Triangle wave stays bounded within [-1, 1] after clamping on CLS-E.
#[test]
fn effect_synth_triangle_wave_bounded() {
    let effect = FfbEffect::Periodic(PeriodicParams {
        waveform: Waveform::Triangle,
        frequency_hz: 8.0,
        amplitude: 0.7,
        phase_deg: 0.0,
        offset: 0.0,
    });
    for i in 0..200 {
        let t = i as f32 * 0.001; // 1 ms steps
        let input = cls_e_input(0.0, 0.0, t);
        let force = effect.compute(&input);
        assert!(
            (-1.0..=1.0).contains(&force),
            "triangle wave out of [-1, 1] at t={t}: {force}"
        );
    }
}

/// Sawtooth wave ramps from negative to positive within each period.
#[test]
fn effect_synth_sawtooth_ramp() {
    let effect = FfbEffect::Periodic(PeriodicParams {
        waveform: Waveform::Sawtooth,
        frequency_hz: 4.0,
        amplitude: 1.0,
        phase_deg: 0.0,
        offset: 0.0,
    });
    let early = cls_e_input(0.0, 0.0, 0.01);
    let late = cls_e_input(0.0, 0.0, 0.24);
    let f_early = effect.compute(&early);
    let f_late = effect.compute(&late);
    assert!(
        f_late > f_early,
        "sawtooth should ramp up: early={f_early}, late={f_late}"
    );
}

/// Envelope simulation: ramp effect for attack/decay on CLS-E.
#[test]
fn effect_synth_envelope_attack_decay() {
    let ramp_up = FfbEffect::Ramp(RampParams {
        start: 0.0,
        end: 1.0,
        duration_ticks: 100,
    });
    let ramp_down = FfbEffect::Ramp(RampParams {
        start: 1.0,
        end: 0.0,
        duration_ticks: 100,
    });

    // Attack at 50%
    let input_50 = EffectInput {
        position: 0.0,
        velocity: 0.0,
        elapsed_s: 0.0,
        tick: 50,
    };
    let attack = ramp_up.compute(&input_50);
    assert!(
        (attack - 0.5).abs() < 1e-4,
        "attack ramp at 50% should be 0.5, got {attack}"
    );

    // Decay at 50%
    let decay = ramp_down.compute(&input_50);
    assert!(
        (decay - 0.5).abs() < 1e-4,
        "decay ramp at 50% should be 0.5, got {decay}"
    );
}

/// Gain adjustment: composite effect with per-effect gains.
#[test]
fn effect_synth_gain_adjustment() {
    let mut composite = CompositeEffect::new();
    composite.add(
        FfbEffect::ConstantForce(ConstantForceParams { magnitude: 1.0 }),
        0.3, // 30% gain
    );
    composite.add(
        FfbEffect::ConstantForce(ConstantForceParams { magnitude: 1.0 }),
        0.2, // 20% gain
    );
    let input = cls_e_input(0.0, 0.0, 0.0);
    let force = composite.compute(&input);
    assert!(
        (force - 0.5).abs() < 1e-5,
        "summed gains 0.3+0.2 = 0.5, got {force}"
    );
}

/// Phase offset: 90° phase shift on sine should peak at t=0.
#[test]
fn effect_synth_phase_offset() {
    let effect = FfbEffect::Periodic(PeriodicParams {
        waveform: Waveform::Sine,
        frequency_hz: 1.0,
        amplitude: 1.0,
        phase_deg: 90.0,
        offset: 0.0,
    });
    // At t=0, sin(π/2) = 1.0.
    let input = cls_e_input(0.0, 0.0, 0.0);
    let force = effect.compute(&input);
    assert!(
        (force - 1.0).abs() < 1e-4,
        "90° phase-shifted sine at t=0 should be 1.0, got {force}"
    );
}

/// DC offset on periodic effect shifts the baseline.
#[test]
fn effect_synth_dc_offset() {
    let effect = FfbEffect::Periodic(PeriodicParams {
        waveform: Waveform::Sine,
        frequency_hz: 1.0,
        amplitude: 0.3,
        phase_deg: 0.0,
        offset: 0.5,
    });
    // At t=0 sine=0, so output = 0 × 0.3 + 0.5 = 0.5.
    let input = cls_e_input(0.0, 0.0, 0.0);
    let force = effect.compute(&input);
    assert!(
        (force - 0.5).abs() < 1e-4,
        "DC offset should shift baseline to 0.5, got {force}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// 3. Trim System (6 tests)
// ═══════════════════════════════════════════════════════════════════════════════

/// Trim position: FFB trim controller targets the requested setpoint.
#[test]
fn trim_position_targets_setpoint() {
    let mut ctrl = TrimController::new(CLS_E_MAX_TORQUE_NM);
    ctrl.set_mode(TrimMode::ForceFeedback);
    let change = SetpointChange {
        target_nm: 5.0,
        limits: TrimLimits {
            max_rate_nm_per_s: 10.0,
            max_jerk_nm_per_s2: 100.0,
        },
    };
    ctrl.apply_setpoint_change(change).unwrap();
    assert_eq!(ctrl.target_setpoint_nm(), 5.0);
    assert!(ctrl.is_changing());
}

/// Trim follow: repeated updates converge toward the target.
#[test]
fn trim_follow_converges() {
    let mut ctrl = TrimController::new(CLS_E_MAX_TORQUE_NM);
    ctrl.set_mode(TrimMode::ForceFeedback);
    let change = SetpointChange {
        target_nm: 3.0,
        limits: TrimLimits {
            max_rate_nm_per_s: 10.0,
            max_jerk_nm_per_s2: 100.0,
        },
    };
    ctrl.apply_setpoint_change(change).unwrap();

    // Run updates with small sleeps to allow real time to pass.
    for _ in 0..200 {
        std::thread::sleep(std::time::Duration::from_millis(1));
        ctrl.update();
    }
    assert!(
        (ctrl.current_setpoint_nm() - 3.0).abs() < 0.5,
        "should converge toward 3.0 Nm, got {}",
        ctrl.current_setpoint_nm()
    );
}

/// Trim release-to-center: spring-centered mode freezes, then re-enables.
#[test]
fn trim_release_to_center() {
    let mut ctrl = TrimController::new(CLS_E_MAX_TORQUE_NM);
    ctrl.set_mode(TrimMode::SpringCentered);

    let change = SetpointChange {
        target_nm: 7.5, // maps to center=0.5
        limits: TrimLimits::default(),
    };
    ctrl.apply_setpoint_change(change).unwrap();

    // Immediately after change, spring should be frozen.
    let output = ctrl.update();
    match output {
        TrimOutput::SpringCentered { frozen, config } => {
            assert!(frozen, "spring should freeze during trim change");
            let expected_center = 7.5 / CLS_E_MAX_TORQUE_NM;
            assert!(
                (config.center - expected_center).abs() < 1e-5,
                "center should be {expected_center}, got {}",
                config.center
            );
        }
        _ => panic!("expected SpringCentered output"),
    }
}

/// Trim in-flight adjustment: changing target mid-slew re-targets correctly.
#[test]
fn trim_in_flight_adjustment() {
    let mut ctrl = TrimController::new(CLS_E_MAX_TORQUE_NM);
    ctrl.set_mode(TrimMode::ForceFeedback);
    let limits = TrimLimits {
        max_rate_nm_per_s: 10.0,
        max_jerk_nm_per_s2: 100.0,
    };

    // Start moving toward 5.0 Nm.
    ctrl.apply_setpoint_change(SetpointChange {
        target_nm: 5.0,
        limits: limits.clone(),
    })
    .unwrap();
    for _ in 0..50 {
        std::thread::sleep(std::time::Duration::from_millis(1));
        ctrl.update();
    }
    let mid = ctrl.current_setpoint_nm();
    assert!(mid > 0.0 && mid < 5.0, "should be in transit, got {mid}");

    // Re-target to 2.0 Nm.
    ctrl.apply_setpoint_change(SetpointChange {
        target_nm: 2.0,
        limits,
    })
    .unwrap();
    assert_eq!(ctrl.target_setpoint_nm(), 2.0);

    // Continue updating — should converge to new target.
    for _ in 0..200 {
        std::thread::sleep(std::time::Duration::from_millis(1));
        ctrl.update();
    }
    assert!(
        (ctrl.current_setpoint_nm() - 2.0).abs() < 0.5,
        "should converge to new target 2.0, got {}",
        ctrl.current_setpoint_nm()
    );
}

/// Trim save/load: trim state roundtrips through get/set.
#[test]
fn trim_save_load_state() {
    let mut ctrl = TrimController::new(CLS_E_MAX_TORQUE_NM);
    ctrl.set_mode(TrimMode::ForceFeedback);
    let limits = TrimLimits {
        max_rate_nm_per_s: 10.0,
        max_jerk_nm_per_s2: 100.0,
    };
    ctrl.apply_setpoint_change(SetpointChange {
        target_nm: 4.0,
        limits: limits.clone(),
    })
    .unwrap();
    for _ in 0..200 {
        std::thread::sleep(std::time::Duration::from_millis(1));
        ctrl.update();
    }

    let state = ctrl.get_trim_state();
    assert_eq!(state.mode, TrimMode::ForceFeedback);
    // After 200ms of updates the setpoint should have advanced toward 4.0.
    assert!(
        state.current_setpoint_nm > 0.0,
        "saved setpoint should have advanced from 0"
    );

    // Re-create and verify we can apply the same setpoint.
    let mut ctrl2 = TrimController::new(CLS_E_MAX_TORQUE_NM);
    ctrl2.set_mode(state.mode);
    ctrl2.set_limits(state.limits);
    ctrl2
        .apply_setpoint_change(SetpointChange {
            target_nm: state.current_setpoint_nm,
            limits: TrimLimits {
                max_rate_nm_per_s: 10.0,
                max_jerk_nm_per_s2: 100.0,
            },
        })
        .unwrap();
    assert!(
        (ctrl2.target_setpoint_nm() - state.current_setpoint_nm).abs() < 1e-5,
        "restored target should match saved state"
    );
}

/// Trim rejects setpoints exceeding CLS-E max torque.
#[test]
fn trim_rejects_exceeding_torque() {
    let mut ctrl = TrimController::new(CLS_E_MAX_TORQUE_NM);
    let result = ctrl.apply_setpoint_change(SetpointChange {
        target_nm: CLS_E_MAX_TORQUE_NM + 1.0,
        limits: TrimLimits::default(),
    });
    assert!(result.is_err(), "should reject setpoint above device max");
}

// ═══════════════════════════════════════════════════════════════════════════════
// 4. Protocol (6 tests)
// ═══════════════════════════════════════════════════════════════════════════════

/// USB HID command format: report ID byte is 0x01.
#[test]
fn protocol_report_id() {
    let report = make_report(0, 0, [0u8; 4]);
    assert_eq!(report[0], CLS_E_REPORT_ID);
    assert_eq!(report.len(), CLS_E_MIN_REPORT_BYTES);
}

/// Status query: valid report parses axes and buttons correctly.
#[test]
fn protocol_status_query() {
    let report = make_report(1000, -2000, [0x05, 0x00, 0x00, 0x00]);
    let state = parse_cls_e_report(&report).unwrap();
    // roll ≈ 1000/32767 ≈ 0.0305
    assert!(
        (state.axes.roll - 1000.0 / 32767.0).abs() < 1e-4,
        "roll parse mismatch"
    );
    // pitch ≈ −2000/32767 ≈ −0.0610
    assert!(
        (state.axes.pitch - (-2000.0 / 32767.0)).abs() < 1e-4,
        "pitch parse mismatch"
    );
    // buttons 1 and 3 (0x05 = 0b00000101)
    assert!(state.buttons.is_pressed(1));
    assert!(!state.buttons.is_pressed(2));
    assert!(state.buttons.is_pressed(3));
}

/// Effect creation: verify parsed CLS-E input can feed effect computation.
#[test]
fn protocol_effect_creation_from_report() {
    let report = make_report(axis_to_raw(0.3), axis_to_raw(-0.5), [0u8; 4]);
    let state = parse_cls_e_report(&report).unwrap();

    let spring = FfbEffect::Spring(SpringParams {
        coefficient: 1.0,
        center: 0.0,
        deadband: 0.0,
        saturation: 1.0,
    });
    let input = cls_e_input(state.axes.roll, 0.0, 0.0);
    let force = spring.compute(&input);
    // Displaced ≈0.3 right → force ≈ −0.3
    assert!(
        (force - -state.axes.roll).abs() < 1e-3,
        "spring force from parsed position should be −position"
    );
}

/// Effect update: sequential reports update force output.
#[test]
fn protocol_effect_update_sequential() {
    let spring = FfbEffect::Spring(SpringParams {
        coefficient: 1.0,
        center: 0.0,
        deadband: 0.0,
        saturation: 1.0,
    });

    let positions = [0.0, 0.2, 0.5, 0.8, 0.5, 0.0];
    let mut prev_force = 0.0_f32;
    for &pos in &positions {
        let report = make_report(axis_to_raw(pos), 0, [0u8; 4]);
        let state = parse_cls_e_report(&report).unwrap();
        let input = cls_e_input(state.axes.roll, 0.0, 0.0);
        let force = spring.compute(&input);
        // Force should change when position changes.
        if (pos - 0.0).abs() > 1e-3 {
            assert!(
                (force - prev_force).abs() > 1e-4 || (pos - 0.0).abs() < 1e-3,
                "force should track position changes"
            );
        }
        prev_force = force;
    }
}

/// Effect destroy: clearing a composite yields zero force.
#[test]
fn protocol_effect_destroy() {
    let mut composite = CompositeEffect::new();
    composite.add(
        FfbEffect::ConstantForce(ConstantForceParams { magnitude: 0.8 }),
        1.0,
    );
    let input = cls_e_input(0.0, 0.0, 0.0);
    assert!(composite.compute(&input).abs() > 0.1, "should have force before destroy");

    composite.clear();
    assert!(composite.is_empty());
    assert!(
        composite.compute(&input).abs() < 1e-6,
        "destroyed effects should produce zero force"
    );
}

/// Firmware version: VID/PID constants match CLS-E specification.
#[test]
fn protocol_vid_pid_constants() {
    assert_eq!(BRUNNER_VID, 0x25BB, "Brunner VID mismatch");
    assert_eq!(CLS_E_PID, 0x0063, "CLS-E PID mismatch");
    // Verify re-exported identifiers from flight-hid-support match.
    assert_eq!(
        crate::BRUNNER_VENDOR_ID, BRUNNER_VID,
        "re-exported VID should match"
    );
    assert_eq!(
        crate::BRUNNER_CLS_E_YOKE_PID, CLS_E_PID,
        "re-exported PID should match"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// 5. Safety Interlocks (4 tests)
// ═══════════════════════════════════════════════════════════════════════════════

/// Maximum force limit: safety state enforces CLS-E torque ceiling.
#[test]
fn safety_max_force_limit() {
    let safe_max = SafetyState::SafeTorque.max_torque_nm(CLS_E_MAX_TORQUE_NM);
    let high_max = SafetyState::HighTorque.max_torque_nm(CLS_E_MAX_TORQUE_NM);
    let faulted_max = SafetyState::Faulted.max_torque_nm(CLS_E_MAX_TORQUE_NM);

    assert!(
        safe_max < CLS_E_MAX_TORQUE_NM,
        "SafeTorque should limit below device max"
    );
    assert_eq!(
        high_max, CLS_E_MAX_TORQUE_NM,
        "HighTorque should allow full CLS-E range"
    );
    assert_eq!(faulted_max, 0.0, "Faulted must output zero");
}

/// Rate of force change limit: safety envelope constrains slew rate.
#[test]
fn safety_rate_of_change_limit() {
    let mut envelope = cls_e_safety_envelope();
    // Prime with zero.
    let _ = envelope.apply(0.0, true).unwrap();
    // Request maximum torque immediately.
    let _ = envelope.apply(CLS_E_MAX_TORQUE_NM, true).unwrap();
    let slew = envelope.get_last_slew_rate().abs();
    assert!(
        slew <= CLS_E_MAX_SLEW_RATE + 1.0,
        "slew rate {slew} Nm/s exceeds limit {CLS_E_MAX_SLEW_RATE} Nm/s"
    );
}

/// Emergency stop: entering faulted state disallows torque.
#[test]
fn safety_emergency_stop() {
    let mut mgr = SafetyStateManager::new();
    mgr.enter_faulted(FaultReason::UserEmergencyStop).unwrap();
    assert_eq!(mgr.current_state(), SafetyState::Faulted);
    assert!(
        !mgr.current_state().allows_torque(),
        "faulted state must not allow torque"
    );
    assert_eq!(
        mgr.current_state().max_torque_nm(CLS_E_MAX_TORQUE_NM),
        0.0
    );

    // Transient fault → clearable.
    mgr.clear_fault().unwrap();
    assert_eq!(mgr.current_state(), SafetyState::SafeTorque);
}

/// Device disconnect → zero force: safety envelope ramps to zero on fault.
#[test]
fn safety_disconnect_zero_force() {
    let mut mgr = SafetyStateManager::new();
    mgr.enter_faulted(FaultReason::DeviceDisconnect).unwrap();
    assert_eq!(mgr.current_state(), SafetyState::Faulted);
    assert!(!mgr.current_state().allows_torque());

    let fault = mgr.current_fault().unwrap();
    assert_eq!(fault.reason, FaultReason::DeviceDisconnect);
    assert!(
        fault.reason.is_transient(),
        "disconnect should be transient"
    );
    assert!(
        fault.reason.requires_torque_cutoff(),
        "disconnect requires torque cutoff"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// 6. Property Tests (3 tests)
// ═══════════════════════════════════════════════════════════════════════════════

use proptest::prelude::*;

proptest! {
    /// Force output is always bounded to [−1.0, +1.0] for arbitrary CLS-E inputs.
    #[test]
    fn prop_force_output_bounded(
        pos in -1.0f32..=1.0,
        vel in -2.0f32..=2.0,
        coeff in 0.0f32..=1.0,
        magnitude in -1.5f32..=1.5,
        elapsed in 0.0f32..=10.0
    ) {
        let effects: Vec<FfbEffect> = vec![
            FfbEffect::Spring(SpringParams {
                coefficient: coeff,
                center: 0.0,
                deadband: 0.0,
                saturation: 1.0,
            }),
            FfbEffect::Damper(DamperParams { coefficient: coeff }),
            FfbEffect::Friction(FrictionParams { coefficient: coeff }),
            FfbEffect::ConstantForce(ConstantForceParams { magnitude }),
            FfbEffect::Periodic(PeriodicParams {
                waveform: Waveform::Sine,
                frequency_hz: 1.0,
                amplitude: coeff,
                phase_deg: 0.0,
                offset: 0.0,
            }),
        ];

        let input = cls_e_input(pos, vel, elapsed);
        for effect in &effects {
            let force = effect.compute(&input);
            prop_assert!(
                (-1.0..=1.0).contains(&force),
                "force {force} out of bounds for effect {effect:?}"
            );
        }
    }

    /// Effect synthesis is deterministic: same inputs → same outputs.
    #[test]
    fn prop_effect_synthesis_deterministic(
        pos in -1.0f32..=1.0,
        vel in -2.0f32..=2.0,
        elapsed in 0.0f32..=10.0,
        coeff in 0.01f32..=1.0
    ) {
        let effect = FfbEffect::Spring(SpringParams {
            coefficient: coeff,
            center: 0.0,
            deadband: 0.0,
            saturation: 1.0,
        });
        let input = cls_e_input(pos, vel, elapsed);
        let f1 = effect.compute(&input);
        let f2 = effect.compute(&input);
        prop_assert_eq!(f1, f2, "effect computation must be deterministic");
    }

    /// Trim position persists: after updates, setpoint moves toward target.
    #[test]
    fn prop_trim_position_persists(target in -14.0f32..=14.0) {
        let mut ctrl = TrimController::new(CLS_E_MAX_TORQUE_NM);
        ctrl.set_mode(TrimMode::ForceFeedback);
        let change = SetpointChange {
            target_nm: target,
            limits: TrimLimits {
                max_rate_nm_per_s: 20.0,
                max_jerk_nm_per_s2: 200.0,
            },
        };
        ctrl.apply_setpoint_change(change).unwrap();

        // Run updates with real time passage.
        for _ in 0..100 {
            std::thread::sleep(std::time::Duration::from_millis(1));
            ctrl.update();
        }
        let final_sp = ctrl.current_setpoint_nm();
        // Verify trim is moving in the correct direction.
        if target.abs() > 0.1 {
            prop_assert!(
                final_sp.signum() == target.signum(),
                "trim should move toward {target}, but at {final_sp}"
            );
        }
    }
}
