// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for FFB safety systems: envelope limiting, interlock,
//! synthesis, trim, mode negotiation, and property-style invariants.
//!
//! **Validates: ADR-009 Safety Interlock Design, FFB-SAFETY-01.x, FFB-SAFETY-02/03**

use std::time::Duration;

use flight_ffb::effects::{
    CompositeEffect, ConstantForceParams, DamperParams, EffectInput, EffectWatchdog, FfbEffect,
    PeriodicParams, SpringParams, Waveform,
};
use flight_ffb::interlock::InterlockSystem;
use flight_ffb::mode_negotiation::ModeNegotiator;
use flight_ffb::safety::{FaultReason, SafetyState, SafetyStateManager, TransitionReason};
use flight_ffb::safety_envelope::{SafetyEnvelope, SafetyEnvelopeConfig};
use flight_ffb::safety_interlock::{SafetyConfig, SafetyInterlock, SafetyInterlockResult};
use flight_ffb::trim::{SetpointChange, TrimController, TrimLimits, TrimMode, TrimOutput};
use flight_ffb::{DeviceCapabilities, FfbMode};

// ═══════════════════════════════════════════════════════════════════════════
// Helper utilities
// ═══════════════════════════════════════════════════════════════════════════

fn default_safety_config() -> SafetyConfig {
    SafetyConfig {
        soft_limit_percent: 80.0,
        hard_limit_percent: 100.0,
        ramp_rate_limit: 10.0,
        initial_max_force: 100.0,
    }
}

fn input_at(position: f32, velocity: f32, elapsed_s: f32, tick: u32) -> EffectInput {
    EffectInput {
        position,
        velocity,
        elapsed_s,
        tick,
    }
}

fn rest_input() -> EffectInput {
    input_at(0.0, 0.0, 0.0, 0)
}

// ═══════════════════════════════════════════════════════════════════════════
// 1. Envelope limiting
// ═══════════════════════════════════════════════════════════════════════════

mod envelope_limiting {
    use super::*;

    #[test]
    fn force_never_exceeds_configured_maximum() {
        let config = SafetyEnvelopeConfig {
            max_torque_nm: 10.0,
            ..SafetyEnvelopeConfig::default()
        };
        let mut env = SafetyEnvelope::new(config).unwrap();

        // Feed extreme values for many ticks
        for _ in 0..500 {
            let out = env.apply(999.0, true).unwrap();
            assert!(
                out.abs() <= 10.0 + 1e-6,
                "output {out} exceeded max_torque_nm 10.0"
            );
        }
    }

    #[test]
    fn rate_limiting_caps_force_change_rate() {
        let config = SafetyEnvelopeConfig {
            max_torque_nm: 15.0,
            max_slew_rate_nm_per_s: 50.0,
            max_jerk_nm_per_s2: 500.0,
            timestep_s: 0.004,
            ..SafetyEnvelopeConfig::default()
        };
        let mut env = SafetyEnvelope::new(config).unwrap();

        let mut prev = 0.0_f32;
        for _ in 0..200 {
            let out = env.apply(15.0, true).unwrap();
            let delta = (out - prev).abs();
            // max_delta per step = 50.0 * 0.004 = 0.2 Nm
            assert!(
                delta <= 0.201,
                "delta {delta} exceeded slew rate limit per step (0.2)"
            );
            prev = out;
        }
    }

    #[test]
    fn smooth_clamping_no_sudden_jump_to_zero() {
        let mut env = SafetyEnvelope::default();

        // Ramp up to some force first
        for _ in 0..100 {
            env.apply(10.0, true).unwrap();
        }
        let before = env.get_last_torque();
        assert!(before > 0.0, "should have non-zero force");

        // Now set safe_for_ffb = false → target becomes 0
        let after = env.apply(10.0, false).unwrap();
        // The envelope rate-limits the drop; it should not instantly jump to zero
        // (unless already very close to zero).
        if before > 1.0 {
            assert!(
                after.abs() > 0.0,
                "force should ramp down smoothly, not jump to 0"
            );
        }
    }

    #[test]
    fn property_output_magnitude_le_max_force_for_varied_inputs() {
        let max = 8.0;
        let config = SafetyEnvelopeConfig {
            max_torque_nm: max,
            ..SafetyEnvelopeConfig::default()
        };
        let mut env = SafetyEnvelope::new(config).unwrap();

        let inputs = [
            0.0, 1.0, -1.0, 100.0, -100.0, 8.0, -8.0, 0.001, -0.001, f32::MAX, f32::MIN,
        ];
        for &input in &inputs {
            if input.is_finite() {
                let out = env.apply(input, true).unwrap();
                assert!(
                    out.abs() <= max + 1e-6,
                    "input={input}, output={out} exceeded {max}"
                );
            }
        }
    }

    #[test]
    fn negative_inputs_are_also_envelope_limited() {
        let config = SafetyEnvelopeConfig {
            max_torque_nm: 5.0,
            ..SafetyEnvelopeConfig::default()
        };
        let mut env = SafetyEnvelope::new(config).unwrap();

        for _ in 0..500 {
            let out = env.apply(-999.0, true).unwrap();
            assert!(
                out.abs() <= 5.0 + 1e-6,
                "negative input output {out} exceeded limit"
            );
        }
    }

    #[test]
    fn nan_input_returns_error() {
        let mut env = SafetyEnvelope::default();
        assert!(env.apply(f32::NAN, true).is_err());
    }

    #[test]
    fn infinity_input_returns_error() {
        let mut env = SafetyEnvelope::default();
        assert!(env.apply(f32::INFINITY, true).is_err());
        assert!(env.apply(f32::NEG_INFINITY, true).is_err());
    }

    #[test]
    fn fault_ramp_reaches_zero() {
        let mut env = SafetyEnvelope::default();

        // Build up some force
        for _ in 0..200 {
            env.apply(10.0, true).unwrap();
        }
        assert!(env.get_last_torque() > 1.0);

        // Trigger fault ramp
        env.trigger_fault_ramp();
        assert!(env.is_in_fault_ramp());

        // After sufficient time the output must be zero
        std::thread::sleep(Duration::from_millis(60));
        let out = env.apply(10.0, true).unwrap();
        assert!(
            out.abs() < 0.01,
            "after fault ramp timeout, torque should be ~0, got {out}"
        );
    }

    #[test]
    fn clear_fault_allows_force_resumption() {
        let mut env = SafetyEnvelope::default();

        for _ in 0..100 {
            env.apply(5.0, true).unwrap();
        }

        env.trigger_fault_ramp();
        std::thread::sleep(Duration::from_millis(60));
        let _ = env.apply(5.0, true).unwrap();

        // Clear fault
        env.clear_fault();
        assert!(!env.is_in_fault_ramp());

        // Force should start ramping back up
        for _ in 0..200 {
            env.apply(5.0, true).unwrap();
        }
        assert!(
            env.get_last_torque() > 0.5,
            "force should resume after clearing fault"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. Safety interlock (ADR-009)
// ═══════════════════════════════════════════════════════════════════════════

mod safety_interlock_tests {
    use super::*;

    #[test]
    fn watchdog_timeout_zeroes_forces() {
        // EffectWatchdog trips after timeout_ticks
        let mut wd = EffectWatchdog::new(5);
        for _ in 0..4 {
            assert!(!wd.tick(), "should not trip before timeout");
        }
        assert!(wd.tick(), "should trip at timeout");
        assert!(wd.is_tripped());
    }

    #[test]
    fn interlock_trip_immediate_force_cutoff() {
        let mut il = SafetyInterlock::new(default_safety_config());
        il.emergency_stop();

        // Any force request should produce zero
        let result = il.check_force(50.0);
        assert_eq!(result, SafetyInterlockResult::EmergencyStopped);
        assert_eq!(il.last_output(), 0.0);
    }

    #[test]
    fn recovery_interlock_reset_forces_resume() {
        let mut il = SafetyInterlock::new(default_safety_config());

        // Trip
        il.emergency_stop();
        assert_eq!(il.check_force(50.0), SafetyInterlockResult::EmergencyStopped);

        // Release
        il.release_emergency_stop();
        assert!(!il.is_emergency_stopped());

        // Force should pass through again (within ramp limits)
        let result = il.check_force(5.0);
        match result {
            SafetyInterlockResult::Passed(v) => assert!((v - 5.0).abs() < 1e-6),
            SafetyInterlockResult::RampLimited(v) => assert!(v > 0.0),
            other => panic!("expected Passed or RampLimited, got {other:?}"),
        }
    }

    #[test]
    fn double_trip_idempotent() {
        let mut il = SafetyInterlock::new(default_safety_config());

        il.emergency_stop();
        il.emergency_stop(); // second trip

        let result = il.check_force(50.0);
        assert_eq!(result, SafetyInterlockResult::EmergencyStopped);

        // Single release should restore
        il.release_emergency_stop();
        assert!(!il.is_emergency_stopped());
    }

    #[test]
    fn property_tripped_interlock_always_zero() {
        let mut il = SafetyInterlock::new(default_safety_config());
        il.emergency_stop();

        let forces = [0.0, 50.0, -50.0, 100.0, -100.0, 0.001, 99.99];
        for &f in &forces {
            let result = il.check_force(f);
            assert_eq!(
                result,
                SafetyInterlockResult::EmergencyStopped,
                "force {f} should be stopped"
            );
            assert_eq!(il.last_output(), 0.0);
        }
    }

    #[test]
    fn watchdog_feed_prevents_trip() {
        let mut wd = EffectWatchdog::new(5);
        for _ in 0..3 {
            wd.tick();
        }
        wd.feed(); // reset
        for _ in 0..4 {
            assert!(!wd.tick(), "should not trip after feed");
        }
        assert!(wd.tick(), "should trip after timeout_ticks without feed");
    }

    #[test]
    fn safety_state_faulted_disallows_torque() {
        assert!(!SafetyState::Faulted.allows_torque());
        assert_eq!(SafetyState::Faulted.max_torque_nm(15.0), 0.0);
    }

    #[test]
    fn safety_state_manager_fault_then_recovery() {
        let mut mgr = SafetyStateManager::new();
        assert_eq!(mgr.current_state(), SafetyState::SafeTorque);

        // Fault
        mgr.enter_faulted(FaultReason::UsbStall).unwrap();
        assert_eq!(mgr.current_state(), SafetyState::Faulted);
        assert!(!mgr.current_state().allows_torque());

        // Clear transient fault → back to SafeTorque
        mgr.clear_fault().unwrap();
        assert_eq!(mgr.current_state(), SafetyState::SafeTorque);
        assert!(mgr.current_state().allows_torque());
    }

    #[test]
    fn hardware_critical_fault_requires_power_cycle() {
        let mut mgr = SafetyStateManager::new();
        mgr.enter_faulted(FaultReason::OverTemp).unwrap();

        // clear_fault should fail for hardware-critical
        assert!(mgr.clear_fault().is_err());
        assert_eq!(mgr.current_state(), SafetyState::Faulted);

        // power cycle works
        mgr.reset_after_power_cycle().unwrap();
        assert_eq!(mgr.current_state(), SafetyState::SafeTorque);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. Force synthesis
// ═══════════════════════════════════════════════════════════════════════════

mod force_synthesis {
    use super::*;

    #[test]
    fn spring_effect_proportional_to_displacement() {
        let spring = FfbEffect::Spring(SpringParams {
            coefficient: 1.0,
            center: 0.0,
            deadband: 0.0,
            saturation: 1.0,
        });

        // Small displacement
        let f1 = spring.compute(&input_at(0.2, 0.0, 0.0, 0));
        let f2 = spring.compute(&input_at(0.4, 0.0, 0.0, 0));

        // Double the displacement should give ~double the force
        assert!((f2 / f1 - 2.0).abs() < 1e-5, "spring should be linear");
    }

    #[test]
    fn damper_effect_proportional_to_velocity() {
        let damper = FfbEffect::Damper(DamperParams { coefficient: 0.5 });

        let f1 = damper.compute(&input_at(0.0, 0.3, 0.0, 0));
        let f2 = damper.compute(&input_at(0.0, 0.6, 0.0, 0));

        assert!(
            (f2 / f1 - 2.0).abs() < 1e-5,
            "damper should be proportional to velocity"
        );
    }

    #[test]
    fn constant_force_steady_output() {
        let eff = FfbEffect::ConstantForce(ConstantForceParams { magnitude: 0.75 });

        // Output should be the same regardless of position/velocity/time
        for tick in 0..10 {
            let f = eff.compute(&input_at(0.5, 1.0, tick as f32 * 0.004, tick));
            assert!(
                (f - 0.75).abs() < 1e-6,
                "constant force should be steady, got {f}"
            );
        }
    }

    #[test]
    fn periodic_sine() {
        let eff = FfbEffect::Periodic(PeriodicParams {
            waveform: Waveform::Sine,
            frequency_hz: 1.0,
            amplitude: 1.0,
            phase_deg: 0.0,
            offset: 0.0,
        });

        // At t=0 → sin(0) = 0
        let f0 = eff.compute(&input_at(0.0, 0.0, 0.0, 0));
        assert!(f0.abs() < 1e-5, "sine at t=0 should be ~0, got {f0}");

        // At t=0.25 (quarter period) → sin(π/2) = 1
        let f1 = eff.compute(&input_at(0.0, 0.0, 0.25, 0));
        assert!((f1 - 1.0).abs() < 1e-5, "sine at t=0.25 should be ~1, got {f1}");

        // At t=0.5 → sin(π) ≈ 0
        let f2 = eff.compute(&input_at(0.0, 0.0, 0.5, 0));
        assert!(f2.abs() < 1e-5, "sine at t=0.5 should be ~0, got {f2}");
    }

    #[test]
    fn periodic_square() {
        let eff = FfbEffect::Periodic(PeriodicParams {
            waveform: Waveform::Square,
            frequency_hz: 1.0,
            amplitude: 0.8,
            phase_deg: 0.0,
            offset: 0.0,
        });

        // First half of period → positive
        let f = eff.compute(&input_at(0.0, 0.0, 0.1, 0));
        assert!(f > 0.0, "square wave first half should be positive");

        // Second half → negative
        let f2 = eff.compute(&input_at(0.0, 0.0, 0.6, 0));
        assert!(f2 < 0.0, "square wave second half should be negative");
    }

    #[test]
    fn periodic_triangle_bounded() {
        let eff = FfbEffect::Periodic(PeriodicParams {
            waveform: Waveform::Triangle,
            frequency_hz: 2.0,
            amplitude: 0.5,
            phase_deg: 0.0,
            offset: 0.0,
        });

        for i in 0..100 {
            let t = i as f32 * 0.01;
            let f = eff.compute(&input_at(0.0, 0.0, t, 0));
            assert!(
                f.abs() <= 1.0 + 1e-6,
                "triangle at t={t} out of bounds: {f}"
            );
        }
    }

    #[test]
    fn periodic_sawtooth_bounded() {
        let eff = FfbEffect::Periodic(PeriodicParams {
            waveform: Waveform::Sawtooth,
            frequency_hz: 3.0,
            amplitude: 1.0,
            phase_deg: 0.0,
            offset: 0.0,
        });

        for i in 0..100 {
            let t = i as f32 * 0.01;
            let f = eff.compute(&input_at(0.0, 0.0, t, 0));
            assert!(
                f.abs() <= 1.0 + 1e-6,
                "sawtooth at t={t} out of bounds: {f}"
            );
        }
    }

    #[test]
    fn combined_spring_damper_periodic() {
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
        composite.add(FfbEffect::Damper(DamperParams { coefficient: 0.3 }), 1.0);
        composite.add(
            FfbEffect::Periodic(PeriodicParams {
                waveform: Waveform::Sine,
                frequency_hz: 5.0,
                amplitude: 0.2,
                phase_deg: 0.0,
                offset: 0.0,
            }),
            1.0,
        );

        assert_eq!(composite.len(), 3);

        let input = input_at(0.5, 0.3, 0.1, 25);
        let f = composite.compute(&input);
        // Output should be bounded
        assert!(f.abs() <= 1.0 + 1e-6, "composite out of bounds: {f}");
        // Should have non-zero output with displacement + velocity + periodic
        assert!(f.abs() > 1e-6, "composite should produce non-zero force");
    }

    #[test]
    fn composite_clamped_to_unit_range() {
        let mut composite = CompositeEffect::new();
        // Stack several large effects
        for _ in 0..8 {
            composite.add(
                FfbEffect::ConstantForce(ConstantForceParams { magnitude: 1.0 }),
                1.0,
            );
        }

        let f = composite.compute(&rest_input());
        assert!(
            (f - 1.0).abs() < 1e-6,
            "composite should clamp to 1.0, got {f}"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 4. Trim system
// ═══════════════════════════════════════════════════════════════════════════

mod trim_system {
    use super::*;

    #[test]
    fn trim_sets_new_center_point() {
        let mut tc = TrimController::new(15.0);
        tc.set_mode(TrimMode::ForceFeedback);

        let change = SetpointChange {
            target_nm: 3.0,
            limits: TrimLimits::default(),
        };
        tc.apply_setpoint_change(change).unwrap();
        assert_eq!(tc.target_setpoint_nm(), 3.0);
    }

    #[test]
    fn trim_within_limits_accepted() {
        let mut tc = TrimController::new(10.0);
        let change = SetpointChange {
            target_nm: 9.0,
            limits: TrimLimits::default(),
        };
        assert!(tc.apply_setpoint_change(change).is_ok());
    }

    #[test]
    fn trim_beyond_limits_rejected() {
        let mut tc = TrimController::new(10.0);
        let change = SetpointChange {
            target_nm: 15.0, // exceeds 10.0 max
            limits: TrimLimits::default(),
        };
        assert!(tc.apply_setpoint_change(change).is_err());
    }

    #[test]
    fn trim_negative_beyond_limits_rejected() {
        let mut tc = TrimController::new(10.0);
        let change = SetpointChange {
            target_nm: -15.0,
            limits: TrimLimits::default(),
        };
        assert!(tc.apply_setpoint_change(change).is_err());
    }

    #[test]
    fn trim_converges_to_target() {
        let mut tc = TrimController::new(15.0);
        tc.set_mode(TrimMode::ForceFeedback);

        let change = SetpointChange {
            target_nm: 2.0,
            limits: TrimLimits {
                max_rate_nm_per_s: 10.0,
                max_jerk_nm_per_s2: 100.0,
            },
        };
        tc.apply_setpoint_change(change).unwrap();

        // Run with deterministic 1ms timesteps
        for _ in 0..1000 {
            tc.update();
            std::thread::sleep(Duration::from_millis(1));
        }

        assert!(
            (tc.current_setpoint_nm() - 2.0).abs() < 0.1,
            "trim should converge to 2.0, got {}",
            tc.current_setpoint_nm()
        );
    }

    #[test]
    fn trim_spring_mode_updates_center() {
        let mut tc = TrimController::new(10.0);
        tc.set_mode(TrimMode::SpringCentered);

        let change = SetpointChange {
            target_nm: 5.0, // 50% of max → center = 0.5
            limits: TrimLimits::default(),
        };
        tc.apply_setpoint_change(change).unwrap();
        assert!((tc.spring_config().center - 0.5).abs() < 1e-6);
    }

    #[test]
    fn trim_rate_respects_limits() {
        let mut tc = TrimController::new(15.0);
        tc.set_mode(TrimMode::ForceFeedback);

        let change = SetpointChange {
            target_nm: 10.0,
            limits: TrimLimits {
                max_rate_nm_per_s: 2.0,
                max_jerk_nm_per_s2: 8.0,
            },
        };
        tc.apply_setpoint_change(change).unwrap();

        let output = tc.update();
        if let TrimOutput::ForceFeedback { rate_nm_per_s, .. } = output {
            assert!(
                rate_nm_per_s.abs() <= 2.0 + 1e-3,
                "rate {rate_nm_per_s} exceeded limit"
            );
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 5. Mode negotiation
// ═══════════════════════════════════════════════════════════════════════════

mod mode_negotiation_tests {
    use super::*;

    fn high_end_caps() -> DeviceCapabilities {
        DeviceCapabilities {
            supports_pid: true,
            supports_raw_torque: true,
            max_torque_nm: 15.0,
            min_period_us: 1000,
            has_health_stream: true,
            supports_interlock: true,
        }
    }

    #[test]
    fn full_ffb_mode_raw_torque() {
        let neg = ModeNegotiator::new();
        let sel = neg.negotiate_mode(&high_end_caps());
        assert_eq!(sel.mode, FfbMode::RawTorque);
        assert!(sel.supports_high_torque);
    }

    #[test]
    fn passive_mode_no_forces() {
        let neg = ModeNegotiator::new();
        let caps = DeviceCapabilities {
            supports_pid: false,
            supports_raw_torque: false,
            max_torque_nm: 2.0,
            min_period_us: 0,
            has_health_stream: false,
            supports_interlock: false,
        };
        let sel = neg.negotiate_mode(&caps);
        assert_eq!(sel.mode, FfbMode::TelemetrySynth);
        assert!(!sel.supports_high_torque);
    }

    #[test]
    fn demo_mode_reduced_forces() {
        // DirectInput-only device: reduced capabilities
        let neg = ModeNegotiator::new();
        let caps = DeviceCapabilities {
            supports_pid: true,
            supports_raw_torque: false,
            max_torque_nm: 5.0,
            min_period_us: 0,
            has_health_stream: false,
            supports_interlock: false,
        };
        let sel = neg.negotiate_mode(&caps);
        assert_eq!(sel.mode, FfbMode::DirectInput);
        // Low torque device → no high torque
        assert!(!sel.supports_high_torque);
    }

    #[test]
    fn mode_transition_full_to_passive_via_state_machine() {
        let mut mgr = SafetyStateManager::new();

        // Full → HighTorque (full FFB)
        mgr.transition_to(SafetyState::HighTorque, TransitionReason::UserEnableHighTorque)
            .unwrap();
        assert!(mgr.current_state().allows_high_torque());

        // → SafeTorque (demo-like reduced)
        mgr.transition_to(
            SafetyState::SafeTorque,
            TransitionReason::UserDisableHighTorque,
        )
        .unwrap();
        assert!(mgr.current_state().allows_torque());
        assert!(!mgr.current_state().allows_high_torque());

        // → Faulted (passive, no forces)
        mgr.enter_faulted(FaultReason::DeviceDisconnect).unwrap();
        assert!(!mgr.current_state().allows_torque());

        // → SafeTorque (resume)
        mgr.clear_fault().unwrap();
        assert!(mgr.current_state().allows_torque());
    }

    #[test]
    fn excessive_torque_device_falls_back() {
        let neg = ModeNegotiator::new();
        let caps = DeviceCapabilities {
            max_torque_nm: 60.0, // exceeds safety limit
            ..high_end_caps()
        };
        let sel = neg.negotiate_mode(&caps);
        // Should fall back to safest mode
        assert_eq!(sel.mode, FfbMode::TelemetrySynth);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 6. Property tests
// ═══════════════════════════════════════════════════════════════════════════

mod property_tests {
    use super::*;

    #[test]
    fn envelope_always_respected_regardless_of_input_sequence() {
        let max = 12.0;
        let config = SafetyEnvelopeConfig {
            max_torque_nm: max,
            max_slew_rate_nm_per_s: 100.0,
            max_jerk_nm_per_s2: 1000.0,
            timestep_s: 0.004,
            ..SafetyEnvelopeConfig::default()
        };
        let mut env = SafetyEnvelope::new(config).unwrap();

        // Pseudo-random input sequence
        let inputs: Vec<f32> = (0..500)
            .map(|i| {
                let x = (i as f32 * 7.3 + 0.1).sin() * 50.0;
                x
            })
            .collect();

        for &input in &inputs {
            let out = env.apply(input, true).unwrap();
            assert!(
                out.abs() <= max + 1e-6,
                "envelope violated: input={input}, output={out}"
            );
        }
    }

    #[test]
    fn zero_input_produces_zero_output_for_all_effect_types() {
        let effects: Vec<FfbEffect> = vec![
            FfbEffect::ConstantForce(ConstantForceParams { magnitude: 0.0 }),
            FfbEffect::Spring(SpringParams {
                coefficient: 0.8,
                center: 0.0,
                deadband: 0.0,
                saturation: 1.0,
            }),
            FfbEffect::Damper(DamperParams { coefficient: 0.5 }),
            FfbEffect::Periodic(PeriodicParams {
                waveform: Waveform::Sine,
                frequency_hz: 1.0,
                amplitude: 0.0,
                phase_deg: 0.0,
                offset: 0.0,
            }),
        ];

        let input = rest_input();
        for eff in &effects {
            let f = eff.compute(&input);
            assert!(
                f.abs() < 1e-6,
                "effect {eff:?} should produce 0 for zero input, got {f}"
            );
        }
    }

    #[test]
    fn symmetric_inputs_produce_symmetric_outputs_spring() {
        let spring = FfbEffect::Spring(SpringParams {
            coefficient: 0.8,
            center: 0.0,
            deadband: 0.0,
            saturation: 1.0,
        });

        let positions = [0.1, 0.3, 0.5, 0.7, 0.9, 1.0];
        for &p in &positions {
            let f_pos = spring.compute(&input_at(p, 0.0, 0.0, 0));
            let f_neg = spring.compute(&input_at(-p, 0.0, 0.0, 0));
            assert!(
                (f_pos + f_neg).abs() < 1e-5,
                "spring not symmetric: f({p})={f_pos}, f({})={f_neg}",
                -p
            );
        }
    }

    #[test]
    fn symmetric_inputs_produce_symmetric_outputs_damper() {
        let damper = FfbEffect::Damper(DamperParams { coefficient: 0.6 });

        let velocities = [0.1, 0.3, 0.5, 0.8, 1.0];
        for &v in &velocities {
            let f_pos = damper.compute(&input_at(0.0, v, 0.0, 0));
            let f_neg = damper.compute(&input_at(0.0, -v, 0.0, 0));
            assert!(
                (f_pos + f_neg).abs() < 1e-5,
                "damper not symmetric: f({v})={f_pos}, f({})={f_neg}",
                -v
            );
        }
    }

    #[test]
    fn interlock_challenge_uniqueness() {
        let mut system = InterlockSystem::new(true);
        let mut ids = Vec::new();
        let mut tokens = Vec::new();

        for _ in 0..50 {
            let c = system.generate_challenge().unwrap();
            assert!(!ids.contains(&c.challenge_id), "duplicate challenge_id");
            assert!(!tokens.contains(&c.token), "duplicate token");
            ids.push(c.challenge_id);
            tokens.push(c.token);
        }
    }

    #[test]
    fn safety_interlock_hard_limit_property() {
        let config = SafetyConfig {
            soft_limit_percent: 80.0,
            hard_limit_percent: 100.0,
            ramp_rate_limit: 1000.0, // high rate to avoid ramp interference
            initial_max_force: 100.0,
        };
        let mut il = SafetyInterlock::new(config);

        // Ramp up gradually so last_output is near the test values
        let forces = [0.0, 50.0, 80.0, 99.9, 100.0, 100.1, 200.0, -50.0, -100.1, -200.0];
        for &f in &forces {
            let result = il.check_force(f);
            let output = match result {
                SafetyInterlockResult::Passed(v)
                | SafetyInterlockResult::SoftLimited(v)
                | SafetyInterlockResult::HardLimited(v)
                | SafetyInterlockResult::RampLimited(v) => v,
                SafetyInterlockResult::EmergencyStopped => 0.0,
            };
            assert!(
                output.abs() <= 100.0 + 1e-6,
                "force {f} → output {output} exceeded hard limit"
            );
        }
    }

    #[test]
    fn composite_never_exceeds_unit_range() {
        let mut composite = CompositeEffect::new();
        composite.add(
            FfbEffect::Spring(SpringParams {
                coefficient: 1.0,
                center: 0.0,
                deadband: 0.0,
                saturation: 1.0,
            }),
            2.0,
        );
        composite.add(
            FfbEffect::ConstantForce(ConstantForceParams { magnitude: 1.0 }),
            2.0,
        );
        composite.add(
            FfbEffect::Periodic(PeriodicParams {
                waveform: Waveform::Sine,
                frequency_hz: 10.0,
                amplitude: 1.0,
                phase_deg: 0.0,
                offset: 0.5,
            }),
            2.0,
        );

        for i in 0..200 {
            let t = i as f32 * 0.005;
            let input = input_at(1.0, 1.0, t, i);
            let f = composite.compute(&input);
            assert!(
                f.abs() <= 1.0 + 1e-6,
                "composite output {f} exceeded unit range at tick {i}"
            );
        }
    }

    #[test]
    fn safety_state_transition_invariants() {
        // Faulted → HighTorque is never valid
        assert!(!SafetyState::Faulted.can_transition_to(SafetyState::HighTorque));

        // SafeTorque → any is valid
        assert!(SafetyState::SafeTorque.can_transition_to(SafetyState::HighTorque));
        assert!(SafetyState::SafeTorque.can_transition_to(SafetyState::Faulted));

        // Faulted → SafeTorque is valid (power cycle)
        assert!(SafetyState::Faulted.can_transition_to(SafetyState::SafeTorque));
    }

    #[test]
    fn all_fault_reasons_have_error_codes_and_descriptions() {
        let reasons = [
            FaultReason::UsbStall,
            FaultReason::EndpointError,
            FaultReason::NanInPipeline,
            FaultReason::OverTemp,
            FaultReason::OverCurrent,
            FaultReason::EncoderInvalid,
            FaultReason::DeviceTimeout,
            FaultReason::DeviceDisconnect,
            FaultReason::UserEmergencyStop,
            FaultReason::HardwareEmergencyStop,
            FaultReason::PluginOverrun,
        ];

        for reason in &reasons {
            assert!(!reason.error_code().is_empty(), "{reason:?} has no error code");
            assert!(
                !reason.description().is_empty(),
                "{reason:?} has no description"
            );
            assert!(
                !reason.kb_article_url().is_empty(),
                "{reason:?} has no KB URL"
            );
        }
    }
}
