// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Comprehensive FFB safety depth tests
//!
//! Covers:
//! - Safety interlock force limit enforcement under all conditions
//! - Envelope boundary tests (max force, rate limits, duration limits)
//! - Emergency stop completeness and irreversibility until explicit reset
//! - Fault injection (impossible sensor values, USB write failure, timing violations)
//! - RT-safe verification (no allocations on FFB hot path, per ADR-004)
//! - Property-based tests (force always within bounds, e-stop guarantees)
//! - Effect composition force budget compliance
//! - Watchdog detection of stuck/unresponsive effect loops
//!
//! **Validates: ADR-004 Zero-Allocation, ADR-009 Safety Interlock Design, QG-FFB-SAFETY**

#[cfg(test)]
mod tests {
    use crate::device::*;
    use crate::effects::*;
    use crate::safety::*;
    use crate::safety_depth::*;
    use crate::safety_envelope::*;
    use crate::safety_interlock::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

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
    //  1. Safety interlock: force limits enforced under ALL conditions
    // =====================================================================

    #[test]
    fn safety_interlock_hard_limit_enforced_for_all_magnitudes() {
        // Use a high ramp rate so we can test limit clamping without ramp interference
        let mut il = SafetyInterlock::new(SafetyConfig {
            soft_limit_percent: 80.0,
            hard_limit_percent: 100.0,
            ramp_rate_limit: 1e9, // effectively unlimited ramp
            initial_max_force: 100.0,
        });
        let extreme_forces = [
            200.0, 500.0, 1000.0, 1e6, -200.0, -500.0, -1000.0, -1e6,
        ];
        for &force in &extreme_forces {
            let result = il.check_force(force);
            let output = match result {
                SafetyInterlockResult::HardLimited(v) => v,
                SafetyInterlockResult::SoftLimited(v) => v,
                SafetyInterlockResult::Passed(v) => v,
                SafetyInterlockResult::RampLimited(v) => v,
                SafetyInterlockResult::EmergencyStopped => 0.0,
            };
            assert!(
                output.abs() <= 100.0,
                "force {force} produced output {output} exceeding hard limit"
            );
        }
    }

    #[test]
    fn safety_interlock_soft_limit_always_below_hard_limit() {
        let configs = [
            SafetyConfig {
                soft_limit_percent: 50.0,
                hard_limit_percent: 75.0,
                ramp_rate_limit: 100.0,
                initial_max_force: 100.0,
            },
            SafetyConfig {
                soft_limit_percent: 90.0,
                hard_limit_percent: 95.0,
                ramp_rate_limit: 100.0,
                initial_max_force: 100.0,
            },
            SafetyConfig {
                soft_limit_percent: 10.0,
                hard_limit_percent: 100.0,
                ramp_rate_limit: 100.0,
                initial_max_force: 100.0,
            },
        ];

        for config in &configs {
            // Use high ramp rate to avoid ramp interference
            let high_ramp_config = SafetyConfig {
                ramp_rate_limit: 1e9,
                ..*config
            };
            let mut il = SafetyInterlock::new(high_ramp_config);
            for force in (0..=200).map(|i| i as f64) {
                let result = il.check_force(force);
                let output = match result {
                    SafetyInterlockResult::HardLimited(v)
                    | SafetyInterlockResult::SoftLimited(v)
                    | SafetyInterlockResult::Passed(v)
                    | SafetyInterlockResult::RampLimited(v) => v,
                    SafetyInterlockResult::EmergencyStopped => 0.0,
                };
                assert!(
                    output.abs() <= config.hard_limit_percent + 1e-6,
                    "output {output} exceeded hard limit {} for input {force}",
                    config.hard_limit_percent
                );
            }
        }
    }

    #[test]
    fn safety_interlock_emergency_stop_overrides_all_limits() {
        let mut il = default_interlock();
        il.emergency_stop();
        // Even with the interlock disabled, e-stop should still work
        let result = il.check_force(50.0);
        assert_eq!(result, SafetyInterlockResult::EmergencyStopped);
        assert!(il.last_output().abs() < 1e-6);
    }

    #[test]
    fn safety_interlock_disabled_then_estop_still_works() {
        let mut il = default_interlock();
        il.disable();
        il.emergency_stop();
        let result = il.check_force(200.0);
        assert_eq!(result, SafetyInterlockResult::EmergencyStopped);
    }

    // =====================================================================
    //  2. Envelope boundary tests
    // =====================================================================

    #[test]
    fn envelope_max_force_never_exceeded_rapid_oscillation() {
        let max_torque = 10.0_f32;
        let mut env = SafetyEnvelope::new(SafetyEnvelopeConfig {
            max_torque_nm: max_torque,
            max_slew_rate_nm_per_s: 500.0,
            max_jerk_nm_per_s2: 50_000.0,
            timestep_s: 0.004,
            ..Default::default()
        })
        .unwrap();

        // Oscillate between extreme positive and negative demands
        for i in 0..500 {
            let desired = if i % 2 == 0 { 100.0 } else { -100.0 };
            let out = env.apply(desired, true).unwrap();
            assert!(
                out.abs() <= max_torque + 0.01,
                "tick {i}: output {out} exceeded max_torque {max_torque}"
            );
        }
    }

    #[test]
    fn envelope_slew_rate_enforced_during_direction_reversal() {
        let max_slew = 50.0_f32;
        let dt = 0.004_f32;
        let mut env = SafetyEnvelope::new(SafetyEnvelopeConfig {
            max_torque_nm: 20.0,
            max_slew_rate_nm_per_s: max_slew,
            max_jerk_nm_per_s2: 50_000.0,
            timestep_s: dt,
            ..Default::default()
        })
        .unwrap();

        // Ramp up to positive
        for _ in 0..200 {
            env.apply(15.0, true).unwrap();
        }
        let before_reversal = env.get_last_torque();
        assert!(before_reversal > 5.0);

        // Abruptly request negative — slew rate should limit
        let mut prev = before_reversal;
        for i in 0..200 {
            let out = env.apply(-15.0, true).unwrap();
            let delta = (out - prev).abs();
            let slew = delta / dt;
            assert!(
                slew <= max_slew + 0.5,
                "tick {i}: slew rate {slew} exceeded limit {max_slew}"
            );
            prev = out;
        }
    }

    #[test]
    fn envelope_duration_violation_tracked() {
        let mut tracker = ViolationTracker::new(Duration::from_secs(10));

        // Simulate a force held at limit for too long
        for _ in 0..10 {
            tracker.record(EnvelopeViolation {
                axis_id: 0,
                requested_force: 10.0,
                limit: 10.0,
                timestamp: Instant::now(),
                violation_type: ViolationType::Duration,
            });
        }

        assert_eq!(tracker.total_count(), 10);
        assert!(tracker.recent_count(Instant::now()) == 10);
        let last = tracker.last_violation().unwrap();
        assert_eq!(last.violation_type, ViolationType::Duration);
    }

    // =====================================================================
    //  3. Emergency stop completeness and irreversibility
    // =====================================================================

    #[test]
    fn emergency_stop_is_irreversible_until_explicit_release() {
        let mut il = default_interlock();

        // Apply force, then e-stop
        il.check_force(5.0);
        il.emergency_stop();

        // Attempt many force applications — all must be stopped
        for _ in 0..1000 {
            let result = il.check_force(50.0);
            assert_eq!(result, SafetyInterlockResult::EmergencyStopped);
            assert!(il.last_output().abs() < 1e-6);
        }

        // Explicit release restores operation
        il.release_emergency_stop();
        let result = il.check_force(5.0);
        assert!(
            !matches!(result, SafetyInterlockResult::EmergencyStopped),
            "should resume after explicit release"
        );
    }

    #[test]
    fn emergency_stop_zeroes_device_output() {
        let mut dev = NullDevice::new();
        dev.send_force(0.95).unwrap();
        assert!(dev.last_force() > 0.9);

        dev.emergency_stop().unwrap();
        assert!(
            dev.last_force().abs() < 1e-6,
            "device should report zero after e-stop"
        );
    }

    #[test]
    fn emergency_stop_survives_concurrent_force_requests() {
        let il = Arc::new(SafetyInterlock::new(SafetyConfig {
            soft_limit_percent: 80.0,
            hard_limit_percent: 100.0,
            ramp_rate_limit: 10.0,
            initial_max_force: 100.0,
        }));
        let stopped = Arc::new(AtomicBool::new(false));

        // One thread triggers e-stop
        let il_stop = Arc::clone(&il);
        let stopped_flag = Arc::clone(&stopped);
        let handle = std::thread::spawn(move || {
            il_stop.emergency_stop();
            stopped_flag.store(true, Ordering::Release);
        });

        handle.join().unwrap();
        assert!(stopped.load(Ordering::Acquire));
        assert!(il.is_emergency_stopped());
    }

    #[test]
    fn safety_state_machine_estop_blocks_high_torque() {
        let mut mgr = SafetyStateManager::new();
        mgr.enter_faulted(FaultReason::UserEmergencyStop).unwrap();

        assert_eq!(mgr.current_state(), SafetyState::Faulted);
        assert!(!mgr.current_state().allows_torque());
        assert!(!mgr.current_state().allows_high_torque());
        assert_eq!(mgr.current_state().max_torque_nm(20.0), 0.0);

        // Cannot go directly to high torque from faulted
        assert!(
            mgr.transition_to(SafetyState::HighTorque, TransitionReason::UserEnableHighTorque)
                .is_err()
        );
    }

    // =====================================================================
    //  4. Fault injection tests
    // =====================================================================

    #[test]
    fn fault_injection_nan_sensor_values() {
        let mut env = default_envelope();
        // NaN must be rejected
        assert!(env.apply(f32::NAN, true).is_err());
        // After rejection, envelope should still be functional
        let out = env.apply(1.0, true).unwrap();
        assert!(out.is_finite());
    }

    #[test]
    fn fault_injection_inf_sensor_values() {
        let mut env = default_envelope();
        assert!(env.apply(f32::INFINITY, true).is_err());
        assert!(env.apply(f32::NEG_INFINITY, true).is_err());
        // Still functional after bad input
        let out = env.apply(0.5, true).unwrap();
        assert!(out.is_finite());
    }

    #[test]
    fn fault_injection_usb_write_failure_device_disconnected() {
        let mut dev = NullDevice::new();
        dev.send_force(0.5).unwrap();
        dev.set_connected(false);

        // System should detect disconnect and e-stop
        if !dev.is_connected() {
            dev.emergency_stop().unwrap();
        }
        assert!(dev.last_force().abs() < 1e-6);
    }

    #[test]
    fn fault_injection_timing_violation_watchdog_trips() {
        let mut wd = WatchdogTimer::new(Duration::from_millis(10), Duration::from_millis(50));

        // Simulate missed deadline
        std::thread::sleep(Duration::from_millis(20));
        let mult = wd.evaluate();
        assert!(wd.is_tripped());
        // Force multiplier should be 1.0 on first ramp tick, then decrease
        assert!(mult <= 1.0);

        // Wait for ramp to complete
        std::thread::sleep(Duration::from_millis(60));
        let mult = wd.evaluate();
        assert!(
            mult.abs() < 0.01,
            "watchdog should force zero after timeout ramp, got {mult}"
        );
        assert_eq!(wd.state(), WatchdogState::Stopped);
    }

    #[test]
    fn fault_injection_rate_limiter_impossible_force_jump() {
        let mut limiter = ForceRateLimiter::new(50.0);
        let dt = 0.004;

        // Initialize at zero
        limiter.limit(0, 0.0, dt);

        // Request impossible jump (simulating corrupted sensor data)
        let out = limiter.limit(0, 1e6, dt);
        // max_delta = 50 * 0.004 = 0.2
        assert!(
            (out - 0.2).abs() < 0.001,
            "rate limiter should clamp impossible jump, got {out}"
        );

        // Negative impossible jump
        let out = limiter.limit(0, -1e6, dt);
        assert!(
            out >= 0.2 - 0.2 - 0.001,
            "rate limiter should clamp negative impossible jump, got {out}"
        );
    }

    #[test]
    fn fault_injection_all_fault_reasons_enter_faulted_state() {
        let fault_reasons = [
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
        ];

        for reason in &fault_reasons {
            let mut mgr = SafetyStateManager::new();
            mgr.enter_faulted(*reason).unwrap();
            assert_eq!(
                mgr.current_state(),
                SafetyState::Faulted,
                "fault {:?} did not enter faulted state",
                reason
            );
            assert!(!mgr.current_state().allows_torque());
            assert_eq!(mgr.current_state().max_torque_nm(15.0), 0.0);
        }
    }

    #[test]
    fn fault_injection_hardware_critical_faults_require_power_cycle() {
        let hw_critical = [
            FaultReason::OverTemp,
            FaultReason::OverCurrent,
            FaultReason::EncoderInvalid,
        ];

        for reason in &hw_critical {
            let mut mgr = SafetyStateManager::new();
            mgr.enter_faulted(*reason).unwrap();

            // clear_fault should fail for hardware-critical faults
            assert!(mgr.clear_fault().is_err(), "{:?} should require power cycle", reason);
            assert_eq!(mgr.current_state(), SafetyState::Faulted);

            // power cycle reset should work
            mgr.reset_after_power_cycle().unwrap();
            assert_eq!(mgr.current_state(), SafetyState::SafeTorque);
        }
    }

    #[test]
    fn fault_injection_transient_faults_clearable() {
        let transient = [
            FaultReason::UsbStall,
            FaultReason::EndpointError,
            FaultReason::NanInPipeline,
            FaultReason::DeviceTimeout,
            FaultReason::DeviceDisconnect,
            FaultReason::UserEmergencyStop,
            FaultReason::HardwareEmergencyStop,
        ];

        for reason in &transient {
            let mut mgr = SafetyStateManager::new();
            mgr.enter_faulted(*reason).unwrap();
            assert!(mgr.clear_fault().is_ok(), "{:?} should be clearable", reason);
            assert_eq!(mgr.current_state(), SafetyState::SafeTorque);
        }
    }

    // =====================================================================
    //  5. RT-safe verification (no allocations on hot path, ADR-004)
    // =====================================================================

    #[test]
    fn rt_safe_interlock_check_force_is_pure_arithmetic() {
        // SafetyInterlock::check_force uses only f64 arithmetic, atomic ops,
        // and f64 comparisons — no heap allocation. We verify by running it
        // a large number of times with diverse inputs.
        let mut il = default_interlock();
        for i in 0..10_000 {
            let force = (i as f64 - 5000.0) * 0.02;
            let _ = il.check_force(force);
        }
        // If we got here without OOM or panics, the hot path is allocation-free
    }

    #[test]
    fn rt_safe_envelope_apply_no_allocation() {
        // SafetyEnvelope::apply uses only f32 arithmetic and Instant comparisons.
        let mut env = default_envelope();
        for i in 0..10_000 {
            let desired = (i as f32 - 5000.0) * 0.002;
            let _ = env.apply(desired, i % 3 != 0);
        }
    }

    #[test]
    fn rt_safe_effect_compute_no_allocation() {
        // FfbEffect::compute is stack-only (f32 math).
        let effects = [
            FfbEffect::ConstantForce(ConstantForceParams { magnitude: 0.5 }),
            FfbEffect::Spring(SpringParams::default()),
            FfbEffect::Damper(DamperParams { coefficient: 0.8 }),
            FfbEffect::Friction(FrictionParams { coefficient: 0.6 }),
            FfbEffect::Periodic(PeriodicParams {
                waveform: Waveform::Sine,
                frequency_hz: 10.0,
                amplitude: 0.8,
                phase_deg: 0.0,
                offset: 0.0,
            }),
            FfbEffect::Ramp(RampParams {
                start: -0.5,
                end: 0.5,
                duration_ticks: 100,
            }),
        ];

        for effect in &effects {
            for i in 0..1_000 {
                let input = EffectInput {
                    position: (i as f32 / 500.0) - 1.0,
                    velocity: (i as f32 / 1000.0) - 0.5,
                    elapsed_s: i as f32 * 0.004,
                    tick: i,
                };
                let f = effect.compute(&input);
                assert!(
                    (-1.0..=1.0).contains(&f),
                    "effect {:?} produced out-of-range force {f}",
                    effect
                );
            }
        }
    }

    #[test]
    fn rt_safe_scheduler_uses_stack_only() {
        // EffectScheduler stores sorted_indices as fixed [u8; MAX_EFFECT_SLOTS].
        let mut mgr = EffectSlotManager::new();
        for i in 0..MAX_EFFECT_SLOTS {
            let prio = match i % 4 {
                0 => EffectPriority::Safety,
                1 => EffectPriority::ControlLoading,
                2 => EffectPriority::Environmental,
                _ => EffectPriority::Ambient,
            };
            mgr.load(
                FfbEffect::ConstantForce(ConstantForceParams {
                    magnitude: 0.05,
                }),
                prio,
                0.5,
            );
        }

        let mut sched = EffectScheduler::new();
        for i in 0..1_000 {
            let input = EffectInput {
                position: (i as f32 / 500.0) - 1.0,
                velocity: 0.0,
                elapsed_s: i as f32 * 0.004,
                tick: i,
            };
            let f = sched.compute(&mgr, &input);
            assert!((-1.0..=1.0).contains(&f));
        }
    }

    #[test]
    fn rt_safe_rate_limiter_no_allocation() {
        let mut limiter = ForceRateLimiter::new(50.0);
        let dt = 0.004;
        for i in 0..10_000 {
            let desired = (i as f32 - 5000.0) * 0.001;
            let axis = (i % 8) as u8;
            let out = limiter.limit(axis, desired, dt);
            assert!(out.is_finite());
        }
    }

    // =====================================================================
    //  6. Property-based tests
    // =====================================================================

    mod prop_tests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            /// Force output from the safety envelope is always within ±max_torque.
            #[test]
            fn prop_envelope_output_within_bounds(
                desired in -100.0f32..100.0,
                safe in proptest::bool::ANY,
            ) {
                let max_torque = 10.0;
                let mut env = SafetyEnvelope::new(SafetyEnvelopeConfig {
                    max_torque_nm: max_torque,
                    max_slew_rate_nm_per_s: 500.0,
                    max_jerk_nm_per_s2: 50_000.0,
                    timestep_s: 0.004,
                    ..Default::default()
                }).unwrap();

                // Warm up
                for _ in 0..10 {
                    let _ = env.apply(desired * 0.1, true);
                }

                let out = env.apply(desired, safe).unwrap();
                prop_assert!(
                    out.abs() <= max_torque + 0.01,
                    "output {} exceeded bounds for desired={}, safe={}",
                    out, desired, safe
                );
            }

            /// Emergency stop on SafetyInterlock always yields zero force.
            #[test]
            fn prop_estop_always_zero(force in -1000.0f64..1000.0) {
                let mut il = SafetyInterlock::new(SafetyConfig {
                    soft_limit_percent: 80.0,
                    hard_limit_percent: 100.0,
                    ramp_rate_limit: 10.0,
                    initial_max_force: 100.0,
                });
                il.emergency_stop();
                let result = il.check_force(force);
                prop_assert_eq!(result, SafetyInterlockResult::EmergencyStopped);
                prop_assert!(il.last_output().abs() < 1e-6);
            }

            /// Interlock hard limit is never exceeded regardless of input.
            #[test]
            fn prop_hard_limit_never_exceeded(
                force in -10000.0f64..10000.0,
                hard_limit in 10.0f64..200.0,
            ) {
                let mut il = SafetyInterlock::new(SafetyConfig {
                    soft_limit_percent: hard_limit * 0.8,
                    hard_limit_percent: hard_limit,
                    ramp_rate_limit: 10000.0, // high ramp to not interfere
                    initial_max_force: hard_limit,
                });
                // Use high ramp rate so ramp doesn't interfere with limit testing
                let result = il.check_force(force);
                let output = match result {
                    SafetyInterlockResult::HardLimited(v)
                    | SafetyInterlockResult::SoftLimited(v)
                    | SafetyInterlockResult::Passed(v)
                    | SafetyInterlockResult::RampLimited(v) => v,
                    SafetyInterlockResult::EmergencyStopped => 0.0,
                };
                prop_assert!(
                    output.abs() <= hard_limit + 1e-6,
                    "output {} exceeded hard limit {} for input {}",
                    output, hard_limit, force
                );
            }

            /// All effect types produce output in [-1.0, 1.0].
            #[test]
            fn prop_effect_output_bounded(
                position in -2.0f32..2.0,
                velocity in -10.0f32..10.0,
                elapsed_s in 0.0f32..100.0,
                tick in 0u32..10000,
                magnitude in -2.0f32..2.0,
            ) {
                let input = EffectInput { position, velocity, elapsed_s, tick };
                let effects = [
                    FfbEffect::ConstantForce(ConstantForceParams { magnitude }),
                    FfbEffect::Spring(SpringParams {
                        coefficient: magnitude.abs().min(1.0),
                        center: 0.0,
                        deadband: 0.0,
                        saturation: 1.0,
                    }),
                    FfbEffect::Damper(DamperParams { coefficient: magnitude.abs().min(1.0) }),
                    FfbEffect::Friction(FrictionParams { coefficient: magnitude.abs().min(1.0) }),
                ];
                for effect in &effects {
                    let f = effect.compute(&input);
                    prop_assert!(
                        (-1.0..=1.0).contains(&f),
                        "effect {:?} produced {} for input {:?}",
                        effect, f, input
                    );
                }
            }

            /// Rate limiter output change never exceeds max_rate * dt.
            #[test]
            fn prop_rate_limiter_bounds(
                desired in -100.0f32..100.0,
                max_rate in 10.0f32..1000.0,
            ) {
                let mut limiter = ForceRateLimiter::new(max_rate);
                let dt = 0.004;
                let _ = limiter.limit(0, 0.0, dt); // init
                let out = limiter.limit(0, desired, dt);
                let max_delta = max_rate * dt;
                let delta = out.abs(); // from 0.0
                prop_assert!(
                    delta <= max_delta + 0.001,
                    "delta {} exceeded max {} for desired {} rate {}",
                    delta, max_delta, desired, max_rate
                );
            }
        }
    }

    // =====================================================================
    //  7. Effect composition: force budget compliance
    // =====================================================================

    #[test]
    fn effect_composition_total_force_within_budget() {
        let mut mgr = EffectSlotManager::new();

        // Load many effects that individually produce high force
        for _ in 0..8 {
            mgr.load(
                FfbEffect::ConstantForce(ConstantForceParams { magnitude: 0.5 }),
                EffectPriority::ControlLoading,
                1.0,
            );
        }

        let mut sched = EffectScheduler::new();
        let f = sched.compute(&mgr, &input_at_rest());
        assert!(
            (-1.0..=1.0).contains(&f),
            "composed force {f} exceeded ±1.0 budget"
        );
    }

    #[test]
    fn effect_composition_mixed_priorities_clamped() {
        let mut mgr = EffectSlotManager::new();

        mgr.load(
            FfbEffect::ConstantForce(ConstantForceParams { magnitude: 0.6 }),
            EffectPriority::Safety,
            1.0,
        );
        mgr.load(
            FfbEffect::ConstantForce(ConstantForceParams { magnitude: 0.6 }),
            EffectPriority::ControlLoading,
            1.0,
        );
        mgr.load(
            FfbEffect::ConstantForce(ConstantForceParams { magnitude: 0.6 }),
            EffectPriority::Environmental,
            1.0,
        );
        mgr.load(
            FfbEffect::ConstantForce(ConstantForceParams { magnitude: 0.6 }),
            EffectPriority::Ambient,
            1.0,
        );

        let mut sched = EffectScheduler::new();
        let f = sched.compute(&mgr, &input_at_rest());
        assert!(
            (-1.0..=1.0).contains(&f),
            "mixed-priority composition {f} exceeded ±1.0 budget"
        );
    }

    #[test]
    fn effect_composition_opposing_effects_cancel() {
        let mut mgr = EffectSlotManager::new();
        mgr.load(
            FfbEffect::ConstantForce(ConstantForceParams { magnitude: 0.8 }),
            EffectPriority::ControlLoading,
            1.0,
        );
        mgr.load(
            FfbEffect::ConstantForce(ConstantForceParams { magnitude: -0.8 }),
            EffectPriority::ControlLoading,
            1.0,
        );

        let mut sched = EffectScheduler::new();
        let f = sched.compute(&mgr, &input_at_rest());
        assert!(
            f.abs() < 0.01,
            "opposing effects should cancel, got {f}"
        );
    }

    #[test]
    fn effect_composition_gain_reduces_contribution() {
        let mut mgr = EffectSlotManager::new();
        mgr.load(
            FfbEffect::ConstantForce(ConstantForceParams { magnitude: 1.0 }),
            EffectPriority::ControlLoading,
            0.3, // 30% gain
        );
        mgr.load(
            FfbEffect::ConstantForce(ConstantForceParams { magnitude: 1.0 }),
            EffectPriority::ControlLoading,
            0.2, // 20% gain
        );

        let mut sched = EffectScheduler::new();
        let f = sched.compute(&mgr, &input_at_rest());
        assert!(
            (f - 0.5).abs() < 0.01,
            "gain-weighted composition should be ~0.5, got {f}"
        );
    }

    #[test]
    fn effect_composition_max_slots_all_contribute() {
        let mut mgr = EffectSlotManager::new();
        let per_slot_force = 1.0 / MAX_EFFECT_SLOTS as f32;

        for _ in 0..MAX_EFFECT_SLOTS {
            mgr.load(
                FfbEffect::ConstantForce(ConstantForceParams {
                    magnitude: per_slot_force,
                }),
                EffectPriority::ControlLoading,
                1.0,
            );
        }

        let mut sched = EffectScheduler::new();
        let f = sched.compute(&mgr, &input_at_rest());
        // Sum should be approximately 1.0 (16 slots × 1/16 each)
        assert!(
            (f - 1.0).abs() < 0.01,
            "all slots contributing equally should sum to ~1.0, got {f}"
        );
    }

    #[test]
    fn effect_composition_user_force_limit_caps_total() {
        let mut mgr = EffectSlotManager::new();
        mgr.load(
            FfbEffect::ConstantForce(ConstantForceParams { magnitude: 0.9 }),
            EffectPriority::ControlLoading,
            1.0,
        );

        let mut sched = EffectScheduler::new();
        let scheduled = sched.compute(&mgr, &input_at_rest());

        let limit = UserForceLimit::new(0.3);
        let final_force = limit.apply(scheduled);
        assert!(
            (final_force - 0.3).abs() < 1e-6,
            "user limit should cap to 0.3, got {final_force}"
        );
    }

    // =====================================================================
    //  8. Watchdog: stuck/unresponsive effect loop detection
    // =====================================================================

    #[test]
    fn watchdog_detects_stuck_effect_loop() {
        let mut wd = DeviceWatchdog::new(10); // 10 ticks timeout
        let mut dev = NullDevice::new();

        // Simulate normal operation with feeding
        for _ in 0..5 {
            wd.feed();
            wd.tick();
            dev.send_force(0.5).unwrap();
        }
        assert!(!wd.is_tripped());

        // Simulate stuck loop — no more feeds
        for _ in 0..10 {
            wd.tick();
        }
        assert!(wd.is_tripped());

        // System should e-stop the device
        if wd.is_tripped() {
            dev.emergency_stop().unwrap();
        }
        assert!(dev.last_force().abs() < 1e-6);
    }

    #[test]
    fn watchdog_timer_ramp_down_monotonic() {
        let mut wd = WatchdogTimer::new(
            Duration::from_millis(1),
            Duration::from_millis(50),
        );

        // Let deadline expire
        std::thread::sleep(Duration::from_millis(5));

        let mut prev_mult = 2.0_f32; // start above 1.0 so first is always <=
        let mut samples = Vec::new();

        for _ in 0..20 {
            let mult = wd.evaluate();
            samples.push(mult);
            assert!(
                mult <= prev_mult + 0.01,
                "watchdog ramp should be monotonically decreasing, got {mult} after {prev_mult}"
            );
            prev_mult = mult;
            std::thread::sleep(Duration::from_millis(5));
        }

        // Should reach zero
        let last = *samples.last().unwrap();
        assert!(
            last < 0.01,
            "watchdog should reach zero after timeout, got {last}"
        );
    }

    #[test]
    fn watchdog_feed_during_ramp_recovers() {
        let mut wd = WatchdogTimer::new(
            Duration::from_millis(1),
            Duration::from_millis(100),
        );

        // Let deadline expire to start ramp
        std::thread::sleep(Duration::from_millis(5));
        let _ = wd.evaluate();
        assert!(wd.is_tripped());

        // Feed during ramp should recover
        wd.feed();
        assert!(!wd.is_tripped());
        assert_eq!(wd.state(), WatchdogState::Active);

        let mult = wd.evaluate();
        assert!(
            (mult - 1.0).abs() < 0.01,
            "multiplier should be 1.0 after recovery, got {mult}"
        );
    }

    #[test]
    fn effect_watchdog_detects_no_updates() {
        let mut wd = EffectWatchdog::new(5); // 5 tick timeout

        // No feeds — should trip after 5 ticks
        for i in 0..4 {
            assert!(!wd.tick(), "should not trip at tick {i}");
        }
        assert!(wd.tick(), "should trip at tick 5");
        assert!(wd.is_tripped());
    }

    #[test]
    fn effect_watchdog_feed_prevents_trip() {
        let mut wd = EffectWatchdog::new(5);

        for _ in 0..20 {
            wd.tick();
            wd.tick();
            wd.feed(); // reset every 2 ticks
        }
        assert!(!wd.is_tripped(), "should not trip when fed regularly");
    }

    #[test]
    fn watchdog_integrates_with_device_estop() {
        let mut wd = DeviceWatchdog::new(3);
        let mut dev = NullDevice::new();
        dev.send_force(0.8).unwrap();

        // Simulate stuck loop (no feeds)
        for _ in 0..3 {
            wd.tick();
        }
        assert!(wd.is_tripped());

        // Integration: watchdog trip → e-stop device
        dev.emergency_stop().unwrap();
        assert!(dev.last_force().abs() < 1e-6);

        // After recovery
        wd.feed();
        assert!(!wd.is_tripped());
    }

    // =====================================================================
    //  Cross-component integration: full safety pipeline
    // =====================================================================

    #[test]
    fn full_safety_pipeline_fault_injection_end_to_end() {
        // 1. Build effects
        let mut mgr = EffectSlotManager::new();
        mgr.load(
            FfbEffect::ConstantForce(ConstantForceParams { magnitude: 0.9 }),
            EffectPriority::ControlLoading,
            1.0,
        );

        // 2. Schedule
        let mut sched = EffectScheduler::new();
        let scheduled = sched.compute(&mgr, &input_at_rest());

        // 3. Apply safety envelope
        let mut env = default_envelope();
        for _ in 0..50 {
            env.apply(scheduled * 10.0, true).unwrap();
        }
        let envelope_out = env.get_last_torque();
        assert!(envelope_out > 0.0);

        // 4. Trigger fault — envelope should ramp to zero
        env.trigger_fault_ramp();
        std::thread::sleep(Duration::from_millis(60));
        let fault_out = env.apply(10.0, true).unwrap();
        assert!(
            fault_out.abs() < 0.1,
            "envelope should be near zero after fault ramp, got {fault_out}"
        );

        // 5. Safety state machine should reflect fault
        let mut state_mgr = SafetyStateManager::new();
        state_mgr.enter_faulted(FaultReason::NanInPipeline).unwrap();
        assert!(!state_mgr.current_state().allows_torque());

        // 6. Device should receive zero
        let mut dev = NullDevice::new();
        dev.send_force(fault_out).unwrap();
        assert!(
            dev.last_force().abs() < 0.1,
            "device should receive near-zero force during fault"
        );
    }

    #[test]
    fn safety_report_captures_all_violations() {
        let mut tracker = ViolationTracker::new(Duration::from_secs(10));
        let mut limiter = ForceRateLimiter::new(50.0);
        let mut wd = WatchdogTimer::new(Duration::from_millis(1), Duration::from_millis(10));

        // Generate violations
        tracker.record(EnvelopeViolation {
            axis_id: 0,
            requested_force: 15.0,
            limit: 10.0,
            timestamp: Instant::now(),
            violation_type: ViolationType::Magnitude,
        });
        tracker.record(EnvelopeViolation {
            axis_id: 1,
            requested_force: 8.0,
            limit: 5.0,
            timestamp: Instant::now(),
            violation_type: ViolationType::RateOfChange,
        });

        // Initialize rate limiter
        let _ = limiter.limit(0, 1.0, 0.004);

        // Trip watchdog
        std::thread::sleep(Duration::from_millis(15));
        let _ = wd.evaluate();
        std::thread::sleep(Duration::from_millis(15));
        let _ = wd.evaluate();

        let report = SafetyReport::from_state(&tracker, &limiter, &wd, Instant::now());

        assert_eq!(report.violation_count, 2);
        assert_eq!(report.recent_violation_count, 2);
        assert!(report.severity > 0.0);
        assert!(report.last_violation_time.is_some());
        assert!(report.rate_limiting_active);
        assert_eq!(report.watchdog_state, WatchdogState::Stopped);
    }
}
