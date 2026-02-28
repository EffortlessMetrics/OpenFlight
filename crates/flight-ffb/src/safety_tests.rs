// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Extended safety tests for FFB interlocks
//!
//! Covers:
//! - Rate-limiting edge cases
//! - Emergency-stop scenarios (disconnect, panic, multi-fault)
//! - Envelope invariant property tests
//! - Effect parameter fuzz-style parsing tests
//!
//! **Validates: ADR-009 Safety Interlock Design, QG-FFB-SAFETY**

#[cfg(test)]
mod tests {
    use crate::device::*;
    use crate::effects::*;
    use crate::safety_envelope::*;
    use crate::safety_interlock::*;

    // ── Helpers ──────────────────────────────────────────────────────────

    fn default_interlock() -> SafetyInterlock {
        SafetyInterlock::new(SafetyConfig {
            soft_limit_percent: 80.0,
            hard_limit_percent: 100.0,
            ramp_rate_limit: 10.0,
            initial_max_force: 100.0,
        })
    }

    fn default_envelope() -> SafetyEnvelope {
        SafetyEnvelope::new(SafetyEnvelopeConfig {
            max_torque_nm: 10.0,
            max_slew_rate_nm_per_s: 50.0,
            max_jerk_nm_per_s2: 500.0,
            timestep_s: 0.004,
            ..Default::default()
        })
        .unwrap()
    }

    fn input_at_rest() -> EffectInput {
        EffectInput {
            position: 0.0,
            velocity: 0.0,
            elapsed_s: 0.0,
            tick: 0,
        }
    }

    // =====================================================================
    //  Rate-limiting tests
    // =====================================================================

    #[test]
    fn rate_limiter_gradual_ramp_up() {
        let mut il = default_interlock();
        // Starting from 0, ramp to 50 in steps of ≤10 per tick.
        for i in 1..=5 {
            let result = il.check_force(50.0);
            match result {
                SafetyInterlockResult::RampLimited(v) => {
                    assert!(
                        (v - (i as f64 * 10.0)).abs() < 1e-6,
                        "tick {i}: expected {}, got {v}",
                        i as f64 * 10.0
                    );
                }
                SafetyInterlockResult::Passed(v) => {
                    assert!(
                        v <= 50.0 + 1e-6,
                        "tick {i}: passed value exceeds target: {v}"
                    );
                }
                other => panic!("tick {i}: unexpected result {other:?}"),
            }
        }
    }

    #[test]
    fn rate_limiter_sign_reversal() {
        let mut il = default_interlock();
        il.check_force(5.0); // ramp-limited to 10… but target 5 is within ramp
        // Actually, from 0 to 5 is delta 5 ≤ ramp_rate 10, so it passes.
        // Now go negative.
        let result = il.check_force(-5.0);
        // Delta from 5 to -5 is 10, exactly the limit.
        assert!(matches!(
            result,
            SafetyInterlockResult::Passed(_) | SafetyInterlockResult::RampLimited(_)
        ));
        // Output should be in ±10 of previous.
        let val = match result {
            SafetyInterlockResult::Passed(v) | SafetyInterlockResult::RampLimited(v) => v,
            _ => unreachable!(),
        };
        assert!(val >= -5.0 - 1e-6, "sign reversal: got {val}");
    }

    #[test]
    fn rate_limiter_respects_limit_on_large_step() {
        let mut il = default_interlock();
        // Jump from 0 to 50 (below soft limit, but delta exceeds ramp rate).
        let result = il.check_force(50.0);
        assert_eq!(result, SafetyInterlockResult::RampLimited(10.0));
    }

    #[test]
    fn rate_limiter_allows_small_steps() {
        let mut il = default_interlock();
        // Delta 5 ≤ ramp_rate 10 → should pass.
        let result = il.check_force(5.0);
        assert_eq!(result, SafetyInterlockResult::Passed(5.0));
    }

    #[test]
    fn rate_limiter_negative_ramp() {
        let mut il = default_interlock();
        il.check_force(5.0); // sets last to 5
        // Jump to -50 → delta 55, ramp limited.
        let result = il.check_force(-50.0);
        assert!(matches!(result, SafetyInterlockResult::RampLimited(_)));
        let val = match result {
            SafetyInterlockResult::RampLimited(v) => v,
            _ => unreachable!(),
        };
        assert!((val - -5.0).abs() < 1e-6, "expected -5.0, got {val}");
    }

    #[test]
    fn envelope_slew_rate_clamps_large_step() {
        let mut env = default_envelope();
        // Request 10 Nm from 0. Slew rate = 50 Nm/s, dt = 0.004s → max delta = 0.2 Nm.
        let out = env.apply(10.0, true).unwrap();
        assert!(
            out.abs() <= 0.2 + 0.01,
            "first step should be slew-limited, got {out}"
        );
    }

    #[test]
    fn envelope_jerk_limits_acceleration() {
        let mut env = SafetyEnvelope::new(SafetyEnvelopeConfig {
            max_torque_nm: 20.0,
            max_slew_rate_nm_per_s: 100.0,
            max_jerk_nm_per_s2: 100.0, // Tight jerk limit
            timestep_s: 0.004,
            ..Default::default()
        })
        .unwrap();

        // Apply a series of increasing steps and verify the rate of rate
        // change (jerk) never exceeds the limit.
        let mut prev_rate = 0.0_f32;
        for _ in 0..20 {
            let _out = env.apply(20.0, true).unwrap();
            let rate = env.get_last_slew_rate();
            let jerk = (rate - prev_rate) / 0.004;
            assert!(
                jerk.abs() <= 100.0 + 1.0,
                "jerk exceeded: {jerk} Nm/s² (limit 100)"
            );
            prev_rate = rate;
        }
    }

    // =====================================================================
    //  Emergency stop scenarios
    // =====================================================================

    #[test]
    fn emergency_stop_zeros_force_immediately() {
        let mut il = default_interlock();
        il.check_force(50.0);
        il.emergency_stop();
        let result = il.check_force(70.0);
        assert_eq!(result, SafetyInterlockResult::EmergencyStopped);
        assert!(il.last_output().abs() < 1e-6);
    }

    #[test]
    fn emergency_stop_during_ramp() {
        let mut il = default_interlock();
        // Start ramping up
        il.check_force(50.0); // ramp limited to 10
        il.check_force(50.0); // ramp limited to 20
        // E-stop mid-ramp
        il.emergency_stop();
        let result = il.check_force(50.0);
        assert_eq!(result, SafetyInterlockResult::EmergencyStopped);
    }

    #[test]
    fn emergency_stop_on_disconnect() {
        let mut dev = NullDevice::new();
        dev.send_force(0.8).unwrap();
        dev.set_connected(false);

        // Simulate the system reacting to disconnect.
        if !dev.is_connected() {
            dev.emergency_stop().unwrap();
        }
        assert!(dev.last_force().abs() < 1e-6);
    }

    #[test]
    fn emergency_stop_idempotent() {
        let mut il = default_interlock();
        il.emergency_stop();
        il.emergency_stop(); // double-tap is safe
        assert!(il.is_emergency_stopped());
        let result = il.check_force(50.0);
        assert_eq!(result, SafetyInterlockResult::EmergencyStopped);
    }

    #[test]
    fn emergency_stop_release_and_resume() {
        let mut il = default_interlock();
        il.emergency_stop();
        assert_eq!(
            il.check_force(50.0),
            SafetyInterlockResult::EmergencyStopped
        );
        il.release_emergency_stop();
        // After release, force should ramp normally from 0.
        let result = il.check_force(5.0);
        assert_eq!(result, SafetyInterlockResult::Passed(5.0));
    }

    #[test]
    fn emergency_stop_concurrent_flag() {
        use std::sync::Arc;

        let il = Arc::new(SafetyInterlock::new(SafetyConfig {
            soft_limit_percent: 80.0,
            hard_limit_percent: 100.0,
            ramp_rate_limit: 10.0,
            initial_max_force: 100.0,
        }));
        let il2 = Arc::clone(&il);

        let handle = std::thread::spawn(move || {
            il2.emergency_stop();
        });
        handle.join().unwrap();
        assert!(il.is_emergency_stopped());
    }

    #[test]
    fn envelope_fault_ramp_reaches_zero() {
        let mut env = default_envelope();
        // Build up some torque.
        for _ in 0..50 {
            env.apply(5.0, true).unwrap();
        }
        let pre_fault = env.get_last_torque();
        assert!(
            pre_fault.abs() > 0.01,
            "should have non-zero torque before fault"
        );

        // Trigger fault ramp.
        env.trigger_fault_ramp();
        assert!(env.is_in_fault_ramp());

        // After 50ms (13 ticks at 4ms), torque should be at or near zero.
        std::thread::sleep(std::time::Duration::from_millis(60));
        let out = env.apply(5.0, true).unwrap();
        assert!(
            out.abs() < 0.1,
            "after fault ramp, torque should be near zero, got {out}"
        );
    }

    #[test]
    fn device_watchdog_triggers_estop() {
        let mut dev = NullDevice::new();
        let mut wd = DeviceWatchdog::new(5);

        dev.send_force(0.7).unwrap();
        for _ in 0..5 {
            wd.tick();
        }
        assert!(wd.is_tripped());

        // System should zero the device.
        if wd.is_tripped() {
            dev.emergency_stop().unwrap();
        }
        assert!(dev.last_force().abs() < 1e-6);
    }

    // =====================================================================
    //  Envelope invariant property tests
    // =====================================================================

    #[test]
    fn envelope_output_never_exceeds_max_torque() {
        let max_torque = 10.0_f32;
        let mut env = SafetyEnvelope::new(SafetyEnvelopeConfig {
            max_torque_nm: max_torque,
            max_slew_rate_nm_per_s: 1000.0,
            max_jerk_nm_per_s2: 100_000.0,
            timestep_s: 0.004,
            ..Default::default()
        })
        .unwrap();

        // Sweep a wide range of requested values.
        let test_values: Vec<f32> = (-200..=200).map(|i| i as f32 * 0.1).collect();
        for &desired in &test_values {
            let out = env.apply(desired, true).unwrap();
            assert!(
                out.abs() <= max_torque + 0.01,
                "output {out} exceeds max_torque {max_torque} for input {desired}"
            );
        }
    }

    #[test]
    fn envelope_safe_for_ffb_false_produces_ramp_to_zero() {
        let mut env = default_envelope();
        // Build up torque first.
        for _ in 0..50 {
            env.apply(5.0, true).unwrap();
        }
        // Now mark unsafe; torque should ramp toward zero.
        for _ in 0..200 {
            let out = env.apply(5.0, false).unwrap();
            assert!(
                out.abs() <= 10.0,
                "output should remain bounded during safe_for_ffb=false"
            );
        }
        let final_out = env.apply(5.0, false).unwrap();
        assert!(
            final_out.abs() < 0.5,
            "after many ticks with safe_for_ffb=false, output should approach zero, got {final_out}"
        );
    }

    #[test]
    fn envelope_nan_input_rejected() {
        let mut env = default_envelope();
        let result = env.apply(f32::NAN, true);
        assert!(result.is_err());
    }

    #[test]
    fn envelope_infinity_input_rejected() {
        let mut env = default_envelope();
        let result = env.apply(f32::INFINITY, true);
        assert!(result.is_err());
    }

    #[test]
    fn envelope_neg_infinity_input_rejected() {
        let mut env = default_envelope();
        let result = env.apply(f32::NEG_INFINITY, true);
        assert!(result.is_err());
    }

    #[test]
    fn envelope_invalid_config_rejected() {
        assert!(
            SafetyEnvelope::new(SafetyEnvelopeConfig {
                max_torque_nm: 0.0,
                ..Default::default()
            })
            .is_err()
        );

        assert!(
            SafetyEnvelope::new(SafetyEnvelopeConfig {
                max_torque_nm: -1.0,
                ..Default::default()
            })
            .is_err()
        );

        assert!(
            SafetyEnvelope::new(SafetyEnvelopeConfig {
                max_slew_rate_nm_per_s: f32::NAN,
                ..Default::default()
            })
            .is_err()
        );
    }

    #[test]
    fn envelope_monotonic_ramp_up() {
        let mut env = SafetyEnvelope::new(SafetyEnvelopeConfig {
            max_torque_nm: 20.0,
            max_slew_rate_nm_per_s: 50.0,
            max_jerk_nm_per_s2: 500.0,
            timestep_s: 0.004,
            ..Default::default()
        })
        .unwrap();

        // Request constant high torque — output should ramp up monotonically.
        let mut prev = 0.0_f32;
        for i in 0..100 {
            let out = env.apply(15.0, true).unwrap();
            assert!(
                out >= prev - 0.001,
                "tick {i}: output decreased from {prev} to {out}"
            );
            prev = out;
        }
    }

    // =====================================================================
    //  Effect parameter fuzz-style tests
    // =====================================================================

    #[test]
    fn fuzz_constant_force_extreme_magnitudes() {
        let values = [-1e10, -1.0, -0.5, 0.0, 0.5, 1.0, 1e10, f32::MIN, f32::MAX];
        for &mag in &values {
            let effect = FfbEffect::ConstantForce(ConstantForceParams { magnitude: mag });
            let f = effect.compute(&input_at_rest());
            assert!(
                (-1.0..=1.0).contains(&f),
                "constant force out of bounds for magnitude {mag}: {f}"
            );
        }
    }

    #[test]
    fn fuzz_spring_extreme_parameters() {
        let coefficients = [0.0, 0.5, 1.0, 100.0];
        let centers = [-2.0, -1.0, 0.0, 1.0, 2.0];
        let deadbands = [0.0, 0.1, 0.5, 1.0];
        let positions = [-2.0, -1.0, -0.5, 0.0, 0.5, 1.0, 2.0];

        for &coeff in &coefficients {
            for &center in &centers {
                for &db in &deadbands {
                    for &pos in &positions {
                        let effect = FfbEffect::Spring(SpringParams {
                            coefficient: coeff,
                            center,
                            deadband: db,
                            saturation: 1.0,
                        });
                        let input = EffectInput {
                            position: pos,
                            ..input_at_rest()
                        };
                        let f = effect.compute(&input);
                        assert!(
                            (-1.0..=1.0).contains(&f),
                            "spring out of bounds: coeff={coeff}, center={center}, db={db}, pos={pos}: {f}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn fuzz_damper_extreme_velocities() {
        let velocities = [-1e6, -1.0, -0.001, 0.0, 0.001, 1.0, 1e6];
        for &vel in &velocities {
            let effect = FfbEffect::Damper(DamperParams { coefficient: 1.0 });
            let input = EffectInput {
                velocity: vel,
                ..input_at_rest()
            };
            let f = effect.compute(&input);
            assert!(
                (-1.0..=1.0).contains(&f),
                "damper out of bounds for velocity {vel}: {f}"
            );
        }
    }

    #[test]
    fn fuzz_periodic_extreme_frequencies() {
        let freqs = [0.001, 1.0, 100.0, 10_000.0, 1e6];
        let times = [0.0, 0.001, 0.1, 1.0, 100.0];

        for &freq in &freqs {
            for &t in &times {
                let effect = FfbEffect::Periodic(PeriodicParams {
                    waveform: Waveform::Sine,
                    frequency_hz: freq,
                    amplitude: 1.0,
                    phase_deg: 0.0,
                    offset: 0.0,
                });
                let input = EffectInput {
                    elapsed_s: t,
                    ..input_at_rest()
                };
                let f = effect.compute(&input);
                assert!(
                    (-1.0..=1.0).contains(&f),
                    "periodic out of bounds for freq={freq}, t={t}: {f}"
                );
            }
        }
    }

    #[test]
    fn fuzz_ramp_zero_and_extreme_durations() {
        let durations = [0, 1, 100, u32::MAX];
        let ticks = [0, 1, 50, 100, u32::MAX];

        for &dur in &durations {
            for &tick in &ticks {
                let effect = FfbEffect::Ramp(RampParams {
                    start: -1.0,
                    end: 1.0,
                    duration_ticks: dur,
                });
                let input = EffectInput {
                    tick,
                    ..input_at_rest()
                };
                let f = effect.compute(&input);
                assert!(
                    (-1.0..=1.0).contains(&f),
                    "ramp out of bounds for dur={dur}, tick={tick}: {f}"
                );
            }
        }
    }

    #[test]
    fn fuzz_friction_edge_cases() {
        let velocities = [0.0, 1e-7, -1e-7, f32::MIN_POSITIVE, -f32::MIN_POSITIVE];
        for &vel in &velocities {
            let effect = FfbEffect::Friction(FrictionParams { coefficient: 1.0 });
            let input = EffectInput {
                velocity: vel,
                ..input_at_rest()
            };
            let f = effect.compute(&input);
            assert!(
                (-1.0..=1.0).contains(&f),
                "friction out of bounds for velocity {vel}: {f}"
            );
        }
    }

    // =====================================================================
    //  Priority scheduling with safety effects
    // =====================================================================

    #[test]
    fn safety_priority_dominates_ambient() {
        let mut mgr = EffectSlotManager::new();
        // Safety effect pushes hard right
        mgr.load(
            FfbEffect::ConstantForce(ConstantForceParams { magnitude: 1.0 }),
            EffectPriority::Safety,
            1.0,
        );
        // Ambient tries to push hard left
        mgr.load(
            FfbEffect::ConstantForce(ConstantForceParams { magnitude: -1.0 }),
            EffectPriority::Ambient,
            1.0,
        );

        let mut sched = EffectScheduler::new();
        let f = sched.compute(&mgr, &input_at_rest());
        // Safety (1.0) saturates first, so we break early. Ambient never contributes.
        // After the safety slot saturates, the loop breaks and returns clamped 1.0.
        // Actually looking at the code: it sums safety(1.0), hits >=1.0, breaks. Result = 1.0.
        assert!((f - 1.0).abs() < 1e-6, "safety should dominate, got {f}");
    }

    #[test]
    fn user_force_limit_with_scheduler() {
        let mut mgr = EffectSlotManager::new();
        mgr.load(
            FfbEffect::ConstantForce(ConstantForceParams { magnitude: 0.8 }),
            EffectPriority::ControlLoading,
            1.0,
        );
        let mut sched = EffectScheduler::new();
        let scheduled = sched.compute(&mgr, &input_at_rest());

        let limit = UserForceLimit::new(0.5);
        let final_force = limit.apply(scheduled);
        assert!(
            (final_force - 0.5).abs() < 1e-6,
            "user limit should cap to 0.5, got {final_force}"
        );
    }

    #[test]
    fn full_pipeline_safety_chain() {
        // Simulate the complete safety chain:
        // effects → scheduler → user limit → interlock → device
        let mut mgr = EffectSlotManager::new();
        mgr.load(
            FfbEffect::ConstantForce(ConstantForceParams { magnitude: 0.9 }),
            EffectPriority::ControlLoading,
            1.0,
        );

        let mut sched = EffectScheduler::new();
        let scheduled = sched.compute(&mgr, &input_at_rest());

        let limit = UserForceLimit::new(0.7);
        let limited = limit.apply(scheduled);

        // Interlock check (scale to percentage for the interlock)
        let mut il = SafetyInterlock::new(SafetyConfig {
            soft_limit_percent: 80.0,
            hard_limit_percent: 100.0,
            ramp_rate_limit: 100.0, // high ramp rate to not interfere
            initial_max_force: 100.0,
        });
        let result = il.check_force(limited as f64 * 100.0);
        let final_force = match result {
            SafetyInterlockResult::Passed(v) => v / 100.0,
            SafetyInterlockResult::SoftLimited(v) => v / 100.0,
            SafetyInterlockResult::HardLimited(v) => v / 100.0,
            SafetyInterlockResult::RampLimited(v) => v / 100.0,
            SafetyInterlockResult::EmergencyStopped => 0.0,
        };

        // Send to device
        let mut dev = NullDevice::new();
        dev.send_force(final_force as f32).unwrap();
        assert!(
            dev.last_force().abs() <= 0.7 + 1e-3,
            "final force should be ≤ user limit, got {}",
            dev.last_force()
        );
    }
}
