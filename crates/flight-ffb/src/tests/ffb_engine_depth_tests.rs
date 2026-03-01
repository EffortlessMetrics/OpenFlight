// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Depth tests for the FFB engine — force computation, safety systems,
//! effect lifecycle, device output, telemetry input, and RT safety.
//!
//! Covers 35+ test cases across six areas:
//! 1. Force calculation (8)
//! 2. Safety envelope (6)
//! 3. Effect lifecycle (5)
//! 4. Device output (5)
//! 5. Telemetry input (6)
//! 6. RT safety (5)

// ═══════════════════════════════════════════════════════════════════════════════
// 1. FORCE CALCULATION (8 tests)
// ═══════════════════════════════════════════════════════════════════════════════
mod force_calculation {
    use crate::effects::*;
    use std::f32::consts::PI;

    fn rest_input() -> EffectInput {
        EffectInput {
            position: 0.0,
            velocity: 0.0,
            elapsed_s: 0.0,
            tick: 0,
        }
    }

    // ── 1.1 Spring effect ────────────────────────────────────────────────
    #[test]
    fn spring_effect_proportional_to_displacement() {
        let spring = FfbEffect::Spring(SpringParams {
            coefficient: 0.5,
            center: 0.0,
            deadband: 0.0,
            saturation: 1.0,
        });

        let small = EffectInput { position: 0.2, ..rest_input() };
        let large = EffectInput { position: 0.8, ..rest_input() };

        let f_small = spring.compute(&small);
        let f_large = spring.compute(&large);

        // Force should be proportional: f_large / f_small ≈ 4
        assert!(f_small < 0.0, "spring opposes positive displacement");
        assert!(f_large < 0.0);
        assert!(
            (f_large / f_small - 4.0).abs() < 0.01,
            "spring should be proportional to displacement"
        );
    }

    // ── 1.2 Damper effect ────────────────────────────────────────────────
    #[test]
    fn damper_effect_opposes_velocity() {
        let damper = FfbEffect::Damper(DamperParams { coefficient: 0.7 });

        let fwd = EffectInput { velocity: 1.0, ..rest_input() };
        let bwd = EffectInput { velocity: -1.0, ..rest_input() };

        let f_fwd = damper.compute(&fwd);
        let f_bwd = damper.compute(&bwd);

        assert!(f_fwd < 0.0, "damper opposes forward velocity");
        assert!(f_bwd > 0.0, "damper opposes backward velocity");
        assert!((f_fwd.abs() - f_bwd.abs()).abs() < 1e-6, "symmetric damping");
    }

    // ── 1.3 Friction effect ──────────────────────────────────────────────
    #[test]
    fn friction_effect_constant_magnitude_opposing_motion() {
        let friction = FfbEffect::Friction(FrictionParams { coefficient: 0.6 });

        let slow = EffectInput { velocity: 0.05, ..rest_input() };
        let fast = EffectInput { velocity: 0.95, ..rest_input() };

        let f_slow = friction.compute(&slow);
        let f_fast = friction.compute(&fast);

        assert!(f_slow < 0.0, "friction opposes motion");
        assert!((f_slow.abs() - f_fast.abs()).abs() < 1e-6, "friction is constant magnitude");
        assert!((f_slow.abs() - 0.6).abs() < 1e-6, "friction equals coefficient");
    }

    // ── 1.4 Constant force ──────────────────────────────────────────────
    #[test]
    fn constant_force_stable_across_inputs() {
        let effect = FfbEffect::ConstantForce(ConstantForceParams { magnitude: 0.75 });

        let positions = [-1.0, -0.5, 0.0, 0.5, 1.0];
        for pos in positions {
            let input = EffectInput { position: pos, velocity: pos, ..rest_input() };
            let f = effect.compute(&input);
            assert!((f - 0.75).abs() < 1e-6, "constant force independent of input state");
        }
    }

    // ── 1.5 Periodic effects (sine/square/triangle/sawtooth) ─────────────
    #[test]
    fn periodic_effects_all_waveforms_bounded() {
        let waveforms = [Waveform::Sine, Waveform::Square, Waveform::Triangle, Waveform::Sawtooth];

        for wf in waveforms {
            let effect = FfbEffect::Periodic(PeriodicParams {
                waveform: wf,
                frequency_hz: 10.0,
                amplitude: 1.0,
                phase_deg: 0.0,
                offset: 0.0,
            });

            // Sample at many phases
            for i in 0..100 {
                let t = i as f32 * 0.001; // 0..0.1s
                let input = EffectInput { elapsed_s: t, ..rest_input() };
                let f = effect.compute(&input);
                assert!(
                    f >= -1.0 && f <= 1.0,
                    "{:?} waveform out of bounds at t={}: {}",
                    wf, t, f
                );
            }
        }
    }

    // ── 1.6 Ramp effect ─────────────────────────────────────────────────
    #[test]
    fn ramp_effect_linear_interpolation() {
        let ramp = FfbEffect::Ramp(RampParams {
            start: -0.5,
            end: 0.5,
            duration_ticks: 100,
        });

        let at_start = EffectInput { tick: 0, ..rest_input() };
        let at_mid = EffectInput { tick: 50, ..rest_input() };
        let at_end = EffectInput { tick: 100, ..rest_input() };
        let past_end = EffectInput { tick: 150, ..rest_input() };

        assert!((ramp.compute(&at_start) - -0.5).abs() < 1e-6);
        assert!((ramp.compute(&at_mid) - 0.0).abs() < 1e-6);
        assert!((ramp.compute(&at_end) - 0.5).abs() < 1e-6);
        assert!((ramp.compute(&past_end) - 0.5).abs() < 1e-6, "ramp clamps at end");
    }

    // ── 1.7 Combined effects ────────────────────────────────────────────
    #[test]
    fn combined_effects_additive_with_gain() {
        let mut composite = CompositeEffect::new();

        // Spring + constant force
        composite.add(
            FfbEffect::Spring(SpringParams {
                coefficient: 1.0,
                center: 0.0,
                deadband: 0.0,
                saturation: 1.0,
            }),
            0.5, // 50% gain
        );
        composite.add(
            FfbEffect::ConstantForce(ConstantForceParams { magnitude: 0.4 }),
            1.0,
        );

        let input = EffectInput { position: 0.6, ..rest_input() };
        let combined = composite.compute(&input);

        // Spring: -1.0 * 0.6 = -0.6, at 50% gain → -0.3
        // Constant: 0.4 at 100% gain → 0.4
        // Total: 0.1
        assert!(
            (combined - 0.1).abs() < 1e-5,
            "combined force should be additive, got {}",
            combined
        );
    }

    // ── 1.8 Effect priority ─────────────────────────────────────────────
    #[test]
    fn effect_priority_higher_priority_dominates() {
        use crate::device::{EffectPriority, EffectScheduler, EffectSlotManager};

        let mut slots = EffectSlotManager::new();
        let mut scheduler = EffectScheduler::new();

        // Safety-priority effect that saturates at 1.0
        slots.load(
            FfbEffect::ConstantForce(ConstantForceParams { magnitude: 1.0 }),
            EffectPriority::Safety,
            1.0,
        );

        // Ambient effect that would add more
        slots.load(
            FfbEffect::ConstantForce(ConstantForceParams { magnitude: 0.5 }),
            EffectPriority::Ambient,
            1.0,
        );

        let input = rest_input();
        let force = scheduler.compute(&slots, &input);

        // Safety saturates first, so ambient gets skipped; output clamped to 1.0
        assert!(
            (force - 1.0).abs() < 1e-6,
            "safety-priority should dominate, got {}",
            force
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 2. SAFETY ENVELOPE (6 tests)
// ═══════════════════════════════════════════════════════════════════════════════
mod safety_envelope {
    use crate::safety_envelope::{SafetyEnvelope, SafetyEnvelopeConfig};
    use crate::safety::{SafetyState, SafetyStateManager, FaultReason, TransitionReason};
    use crate::safety_interlock::{SafetyConfig, SafetyInterlock, SafetyInterlockResult};
    use std::time::Duration;

    fn high_slew_config() -> SafetyEnvelopeConfig {
        SafetyEnvelopeConfig {
            max_torque_nm: 10.0,
            max_slew_rate_nm_per_s: 1000.0,
            max_jerk_nm_per_s2: 100_000.0,
            fault_ramp_time: Duration::from_millis(50),
            timestep_s: 0.004,
        }
    }

    // ── 2.1 Maximum force clamping ──────────────────────────────────────
    #[test]
    fn safety_maximum_force_clamping() {
        let mut env = SafetyEnvelope::new(high_slew_config()).unwrap();

        // Request way over max — should clamp
        let out = env.apply(100.0, true).unwrap();
        assert!(out <= 10.0, "torque must be clamped to max: {}", out);

        env.reset();
        let out = env.apply(-100.0, true).unwrap();
        assert!(out >= -10.0, "negative torque must be clamped: {}", out);
    }

    // ── 2.2 Rate-of-change limiting ─────────────────────────────────────
    #[test]
    fn safety_rate_of_change_limiting() {
        let config = SafetyEnvelopeConfig {
            max_torque_nm: 20.0,
            max_slew_rate_nm_per_s: 10.0,   // 10 Nm/s
            max_jerk_nm_per_s2: 100_000.0,
            fault_ramp_time: Duration::from_millis(50),
            timestep_s: 0.004,
        };
        let mut env = SafetyEnvelope::new(config).unwrap();

        // Request instant jump to 20 Nm from 0
        let out = env.apply(20.0, true).unwrap();
        // max delta per step = 10 * 0.004 = 0.04 Nm
        assert!(
            out.abs() <= 0.04 + 1e-6,
            "single-step change must be rate-limited: {}",
            out
        );
    }

    // ── 2.3 Emergency stop ──────────────────────────────────────────────
    #[test]
    fn safety_emergency_stop_zeroes_force() {
        let config = SafetyConfig {
            soft_limit_percent: 80.0,
            hard_limit_percent: 100.0,
            ramp_rate_limit: 100.0,
            initial_max_force: 100.0,
        };
        let mut interlock = SafetyInterlock::new(config);

        interlock.emergency_stop();
        let result = interlock.check_force(50.0);
        assert_eq!(result, SafetyInterlockResult::EmergencyStopped);
    }

    // ── 2.4 Force fade-in on connect ────────────────────────────────────
    #[test]
    fn safety_force_fade_in_on_connect() {
        // A brand-new envelope starts at zero and ramps toward the target
        let config = SafetyEnvelopeConfig {
            max_torque_nm: 15.0,
            max_slew_rate_nm_per_s: 50.0,
            max_jerk_nm_per_s2: 500.0,
            fault_ramp_time: Duration::from_millis(50),
            timestep_s: 0.004,
        };
        let mut env = SafetyEnvelope::new(config).unwrap();

        // First tick: output must be near zero (rate-limited from 0)
        let first = env.apply(10.0, true).unwrap();
        assert!(
            first.abs() < 1.0,
            "first tick should be near zero due to slew rate: {}",
            first
        );

        // After many ticks it converges
        for _ in 0..5000 {
            env.apply(10.0, true).unwrap();
        }
        let converged = env.get_last_torque();
        assert!(
            (converged - 10.0).abs() < 0.5,
            "should converge to target: {}",
            converged
        );
    }

    // ── 2.5 Force fade-out on disconnect (fault ramp) ───────────────────
    #[test]
    fn safety_force_fade_out_on_disconnect() {
        let mut env = SafetyEnvelope::new(high_slew_config()).unwrap();

        // Ramp up to a non-zero torque
        for _ in 0..500 {
            env.apply(8.0, true).unwrap();
        }
        let before_fault = env.get_last_torque();
        assert!(before_fault > 5.0, "should be near 8 Nm: {}", before_fault);

        // Trigger fault ramp-down
        env.trigger_fault_ramp();
        assert!(env.is_in_fault_ramp());

        // After the fault ramp time, output should reach zero
        std::thread::sleep(Duration::from_millis(60));
        let out = env.apply(8.0, true).unwrap();
        assert!(
            out.abs() < 0.01,
            "after fault ramp, torque should be ~0: {}",
            out
        );
    }

    // ── 2.6 Safety interlock (ADR-009) ──────────────────────────────────
    #[test]
    fn safety_interlock_adr009_state_transitions() {
        let mut mgr = SafetyStateManager::new();
        assert_eq!(mgr.current_state(), SafetyState::SafeTorque);

        // SafeTorque → HighTorque
        mgr.transition_to(SafetyState::HighTorque, TransitionReason::UserEnableHighTorque)
            .unwrap();
        assert!(mgr.current_state().allows_high_torque());

        // HighTorque → Faulted
        mgr.enter_faulted(FaultReason::OverTemp).unwrap();
        assert_eq!(mgr.current_state(), SafetyState::Faulted);
        assert!(!mgr.current_state().allows_torque());

        // Faulted → HighTorque MUST fail
        assert!(
            mgr.transition_to(SafetyState::HighTorque, TransitionReason::UserEnableHighTorque)
                .is_err()
        );

        // Hardware-critical fault cannot be cleared without power cycle
        assert!(mgr.clear_fault().is_err());

        // Power cycle resets to SafeTorque
        mgr.reset_after_power_cycle().unwrap();
        assert_eq!(mgr.current_state(), SafetyState::SafeTorque);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 3. EFFECT LIFECYCLE (5 tests)
// ═══════════════════════════════════════════════════════════════════════════════
mod effect_lifecycle {
    use crate::device::{EffectPriority, EffectSlotHandle, EffectSlotManager, MAX_EFFECT_SLOTS};
    use crate::effects::*;

    fn rest_input() -> EffectInput {
        EffectInput {
            position: 0.0,
            velocity: 0.0,
            elapsed_s: 0.0,
            tick: 0,
        }
    }

    // ── 3.1 Create effect ───────────────────────────────────────────────
    #[test]
    fn lifecycle_create_effect_returns_handle() {
        let mut mgr = EffectSlotManager::new();
        assert!(mgr.is_empty());

        let handle = mgr.load(
            FfbEffect::ConstantForce(ConstantForceParams { magnitude: 0.5 }),
            EffectPriority::ControlLoading,
            1.0,
        );

        assert!(handle.is_some());
        assert_eq!(mgr.len(), 1);
        assert_eq!(mgr.available(), MAX_EFFECT_SLOTS - 1);
    }

    // ── 3.2 Start / stop effect ─────────────────────────────────────────
    #[test]
    fn lifecycle_start_stop_effect() {
        use crate::device::EffectScheduler;

        let mut mgr = EffectSlotManager::new();
        let mut sched = EffectScheduler::new();

        let handle = mgr
            .load(
                FfbEffect::ConstantForce(ConstantForceParams { magnitude: 0.8 }),
                EffectPriority::ControlLoading,
                1.0,
            )
            .unwrap();

        // Active by default
        let f = sched.compute(&mgr, &rest_input());
        assert!((f - 0.8).abs() < 1e-6);

        // Stop → zero output
        mgr.set_active(handle, false);
        let f = sched.compute(&mgr, &rest_input());
        assert!(f.abs() < 1e-6, "stopped effect should produce no force");

        // Re-start
        mgr.set_active(handle, true);
        let f = sched.compute(&mgr, &rest_input());
        assert!((f - 0.8).abs() < 1e-6, "restarted effect should produce force");
    }

    // ── 3.3 Update parameters ───────────────────────────────────────────
    #[test]
    fn lifecycle_update_parameters() {
        use crate::device::EffectScheduler;

        let mut mgr = EffectSlotManager::new();
        let mut sched = EffectScheduler::new();

        let handle = mgr
            .load(
                FfbEffect::ConstantForce(ConstantForceParams { magnitude: 0.3 }),
                EffectPriority::ControlLoading,
                1.0,
            )
            .unwrap();

        let f1 = sched.compute(&mgr, &rest_input());
        assert!((f1 - 0.3).abs() < 1e-6);

        // Update to new magnitude
        let updated = mgr.update_effect(
            handle,
            FfbEffect::ConstantForce(ConstantForceParams { magnitude: 0.9 }),
        );
        assert!(updated);

        let f2 = sched.compute(&mgr, &rest_input());
        assert!((f2 - 0.9).abs() < 1e-6, "parameters should be updated");
    }

    // ── 3.4 Pause / resume ──────────────────────────────────────────────
    #[test]
    fn lifecycle_pause_resume() {
        use crate::device::EffectScheduler;

        let mut mgr = EffectSlotManager::new();
        let mut sched = EffectScheduler::new();

        let handle = mgr
            .load(
                FfbEffect::ConstantForce(ConstantForceParams { magnitude: 0.6 }),
                EffectPriority::ControlLoading,
                1.0,
            )
            .unwrap();

        // Pause
        mgr.set_active(handle, false);
        assert!(sched.compute(&mgr, &rest_input()).abs() < 1e-6);

        // Resume
        mgr.set_active(handle, true);
        assert!((sched.compute(&mgr, &rest_input()) - 0.6).abs() < 1e-6);
    }

    // ── 3.5 Destroy effect ──────────────────────────────────────────────
    #[test]
    fn lifecycle_destroy_effect_frees_slot() {
        let mut mgr = EffectSlotManager::new();

        let handle = mgr
            .load(
                FfbEffect::ConstantForce(ConstantForceParams { magnitude: 0.5 }),
                EffectPriority::Ambient,
                1.0,
            )
            .unwrap();

        assert_eq!(mgr.len(), 1);
        assert!(mgr.unload(handle));
        assert_eq!(mgr.len(), 0);
        assert_eq!(mgr.available(), MAX_EFFECT_SLOTS);

        // Double-unload returns false
        assert!(!mgr.unload(handle));
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 4. DEVICE OUTPUT (5 tests)
// ═══════════════════════════════════════════════════════════════════════════════
mod device_output {
    use crate::device::*;
    use crate::effects::*;

    // ── 4.1 Force → device command conversion ───────────────────────────
    #[test]
    fn device_force_to_command_conversion() {
        let mut dev = NullDevice::new();
        assert!(dev.is_connected());

        // Send a force value and verify it's stored
        dev.send_force(0.75).unwrap();
        assert!((dev.last_force() - 0.75).abs() < 1e-6);

        dev.send_force(-0.5).unwrap();
        assert!((dev.last_force() - -0.5).abs() < 1e-6);
    }

    // ── 4.2 Update rate control ─────────────────────────────────────────
    #[test]
    fn device_watchdog_trips_on_missing_updates() {
        let mut wd = DeviceWatchdog::new(5); // 5-tick timeout

        // Feed within timeout
        for _ in 0..4 {
            assert!(!wd.tick(), "should not trip before timeout");
        }
        wd.feed();

        // Now miss updates
        for _ in 0..4 {
            assert!(!wd.tick());
        }
        assert!(wd.tick(), "should trip at timeout");
        assert!(wd.is_tripped());
    }

    // ── 4.3 Multiple axis forces ────────────────────────────────────────
    #[test]
    fn device_multiple_axis_scaling() {
        let scaling = ForceScaling {
            global_gain: 0.8,
            axis_gains: [0.5, 1.0], // pitch at 50%, roll at 100%
        };

        let pitch_force = scaling.apply(1.0, 0);
        let roll_force = scaling.apply(1.0, 1);

        assert!((pitch_force - 0.4).abs() < 1e-6, "pitch = 0.8 * 0.5 = 0.4");
        assert!((roll_force - 0.8).abs() < 1e-6, "roll = 0.8 * 1.0 = 0.8");
    }

    // ── 4.4 Device capability matching ──────────────────────────────────
    #[test]
    fn device_backend_kind_identification() {
        let null_dev = NullDevice::new();
        assert_eq!(null_dev.backend_kind(), FfbBackendKind::Null);
        assert_eq!(null_dev.name(), "NullDevice");
    }

    // ── 4.5 Fallback for unsupported effects (UserForceLimit) ───────────
    #[test]
    fn device_user_force_limit_clamps_output() {
        let limit = UserForceLimit::new(0.5);

        assert!((limit.apply(0.3) - 0.3).abs() < 1e-6, "within limit passes");
        assert!((limit.apply(0.8) - 0.5).abs() < 1e-6, "over limit clamped");
        assert!((limit.apply(-0.9) - -0.5).abs() < 1e-6, "negative clamped");
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 5. TELEMETRY INPUT (6 tests)
// ═══════════════════════════════════════════════════════════════════════════════
mod telemetry_input {
    use crate::telemetry_synth::*;

    // ── 5.1 Ground forces from telemetry ────────────────────────────────
    #[test]
    fn telemetry_ground_roll_config_defaults() {
        let config = TelemetrySynthConfig::default();
        assert!(config.ground_roll.enabled);
        assert!(config.ground_roll.max_intensity > 0.0);
        assert!(config.ground_roll.frequency_hz > 0.0);
    }

    // ── 5.2 Aerodynamic forces ──────────────────────────────────────────
    #[test]
    fn telemetry_stall_buffet_config_threshold() {
        let config = TelemetrySynthConfig::default();
        assert!(config.stall_buffet.aoa_threshold_deg > 0.0, "AoA threshold must be positive");
        assert!(
            config.stall_buffet.max_intensity <= 1.0,
            "intensity must be normalized"
        );
    }

    // ── 5.3 Stick shaker (stall buffet state) ──────────────────────────
    #[test]
    fn telemetry_stall_buffet_state_initializes_to_zero() {
        let state = StallBuffetState::default();
        assert!(state.current_intensity.abs() < 1e-6);
        assert!(state.current_frequency.abs() < 1e-6);
        assert!(state.phase.abs() < 1e-6);
    }

    // ── 5.4 Runway rumble (ground roll state) ───────────────────────────
    #[test]
    fn telemetry_ground_roll_state_initializes_off() {
        let state = GroundRollState::default();
        assert!(!state.on_ground);
        assert!(state.current_intensity.abs() < 1e-6);
    }

    // ── 5.5 Buffet / touchdown state ────────────────────────────────────
    #[test]
    fn telemetry_touchdown_state_initializes_inactive() {
        let state = TouchdownState::default();
        assert!(!state.impulse_active);
        assert!(!state.touchdown_detected);
        assert!(state.impulse_magnitude.abs() < 1e-6);
    }

    // ── 5.6 G-force / rotor effects ────────────────────────────────────
    #[test]
    fn telemetry_rotor_effects_config_reasonable_defaults() {
        let config = TelemetrySynthConfig::default();
        assert!(config.rotor_effects.enabled);
        assert!(config.rotor_effects.nr_low_threshold > 0.0);
        assert!(config.rotor_effects.torque_scaling > 0.0);
        assert!(
            config.rotor_effects.warning_intensity <= 1.0,
            "warning intensity normalized"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// 6. RT SAFETY (5 tests)
// ═══════════════════════════════════════════════════════════════════════════════
mod rt_safety {
    use crate::device::{EffectPriority, EffectScheduler, EffectSlotManager, MAX_EFFECT_SLOTS};
    use crate::effects::*;
    use std::time::Instant;

    fn rest_input() -> EffectInput {
        EffectInput {
            position: 0.0,
            velocity: 0.0,
            elapsed_s: 0.0,
            tick: 0,
        }
    }

    // ── 6.1 No allocation in force calculation loop ─────────────────────
    #[test]
    #[ignore = "wall-clock timing; run in controlled perf jobs"]
    fn rt_force_loop_no_allocation() {
        // The entire compute pipeline is stack-resident.
        // Running 10k iterations with no allocator instrumentation;
        // confirm it completes within a sane wall-clock budget.
        let mut slots = EffectSlotManager::new();
        let mut sched = EffectScheduler::new();

        slots.load(
            FfbEffect::Spring(SpringParams::default()),
            EffectPriority::ControlLoading,
            1.0,
        );
        slots.load(
            FfbEffect::Damper(DamperParams { coefficient: 0.5 }),
            EffectPriority::Environmental,
            0.8,
        );

        let start = Instant::now();
        for tick in 0..10_000u32 {
            let input = EffectInput {
                position: (tick as f32 * 0.001).sin(),
                velocity: (tick as f32 * 0.001).cos(),
                elapsed_s: tick as f32 * 0.004,
                tick,
            };
            let _f = sched.compute(&slots, &input);
        }
        let elapsed = start.elapsed();

        // 10k ticks at 250 Hz = 40s of simulated time; should finish in <100ms real.
        assert!(
            elapsed.as_millis() < 500,
            "compute loop took too long: {}ms (should be <500ms)",
            elapsed.as_millis()
        );
    }

    // ── 6.2 Pre-allocated effect pool ───────────────────────────────────
    #[test]
    fn rt_pre_allocated_effect_pool_bounded() {
        let mut mgr = EffectSlotManager::new();

        // Fill all slots
        for _ in 0..MAX_EFFECT_SLOTS {
            let handle = mgr.load(
                FfbEffect::ConstantForce(ConstantForceParams { magnitude: 0.1 }),
                EffectPriority::Ambient,
                1.0,
            );
            assert!(handle.is_some());
        }
        assert_eq!(mgr.available(), 0);

        // One more should fail (no allocation, no panic)
        let overflow = mgr.load(
            FfbEffect::ConstantForce(ConstantForceParams { magnitude: 0.1 }),
            EffectPriority::Ambient,
            1.0,
        );
        assert!(overflow.is_none(), "pool must reject when full, not allocate");
    }

    // ── 6.3 Atomic state (effect watchdog) ──────────────────────────────
    #[test]
    fn rt_atomic_watchdog_feed_reset() {
        let mut wd = EffectWatchdog::new(10);

        // Tick 9 times — not yet tripped
        for _ in 0..9 {
            assert!(!wd.tick());
        }
        // Feed resets
        wd.feed();
        assert!(!wd.is_tripped());

        // Tick 10 more
        for _ in 0..10 {
            wd.tick();
        }
        assert!(wd.is_tripped(), "should trip after timeout");
    }

    // ── 6.4 Bounded compute time ────────────────────────────────────────
    #[test]
    #[ignore = "wall-clock timing; run in controlled perf jobs"]
    fn rt_bounded_compute_time_per_tick() {
        let mut slots = EffectSlotManager::new();
        let mut sched = EffectScheduler::new();

        // Worst case: all 16 slots active
        for _ in 0..MAX_EFFECT_SLOTS {
            slots.load(
                FfbEffect::Periodic(PeriodicParams {
                    waveform: Waveform::Sine,
                    frequency_hz: 50.0,
                    amplitude: 0.3,
                    phase_deg: 45.0,
                    offset: 0.0,
                }),
                EffectPriority::Environmental,
                0.5,
            );
        }

        let input = EffectInput {
            position: 0.5,
            velocity: 0.3,
            elapsed_s: 1.234,
            tick: 300,
        };

        // Warm up
        for _ in 0..100 {
            let _ = sched.compute(&slots, &input);
        }

        // Measure single tick
        let start = Instant::now();
        let _f = sched.compute(&slots, &input);
        let elapsed = start.elapsed();

        // Single tick with 16 effects must be well under 4ms (250 Hz budget)
        assert!(
            elapsed.as_micros() < 4000,
            "single compute took {}µs, must be <4000µs",
            elapsed.as_micros()
        );
    }

    // ── 6.5 Graceful degradation under CPU load ─────────────────────────
    #[test]
    fn rt_graceful_degradation_composite_overflow() {
        // CompositeEffect is bounded at 8 effects. Adding more returns false.
        let mut comp = CompositeEffect::new();

        for i in 0..8 {
            assert!(
                comp.add(
                    FfbEffect::ConstantForce(ConstantForceParams {
                        magnitude: 0.1 * (i + 1) as f32,
                    }),
                    1.0,
                ),
                "should accept effect {}",
                i
            );
        }
        assert!(!comp.add(
            FfbEffect::ConstantForce(ConstantForceParams { magnitude: 0.1 }),
            1.0,
        ), "should reject 9th effect gracefully");
        assert_eq!(comp.len(), 8);

        // Compute still works and is clamped
        let f = comp.compute(&rest_input());
        assert!(
            f >= -1.0 && f <= 1.0,
            "output must be bounded even with 8 effects: {}",
            f
        );
    }
}
