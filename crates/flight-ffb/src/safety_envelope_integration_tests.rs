// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! SafetyEnvelope Integration Tests
//!
//! Comprehensive integration tests for the FFB safety envelope system.
//!
//! **Validates: Requirements FFB-SAFETY-01.1-6, QG-FFB-SAFETY**
//!
//! # Test Coverage
//! - Pure Rust tests for clamping, slew, jerk
//! - 50ms ramp from arbitrary starting torque using `fault_initial_torque`
//! - SafeTorque mode (30% envelope) vs HighTorque (100%) vs Faulted (0%)

#[cfg(test)]
mod tests {
    use crate::safety::{FaultReason, SafetyState, SafetyStateManager, TransitionReason};
    use crate::safety_envelope::{SafetyEnvelope, SafetyEnvelopeConfig};
    use std::thread;
    use std::time::{Duration, Instant};

    // ============================================================================
    // SECTION 1: Pure Rust Tests for Clamping, Slew, and Jerk
    // ============================================================================

    /// **Test: Torque Clamping with Various Device Max Values**
    /// **Validates: Requirement FFB-SAFETY-01.1**
    ///
    /// Verifies that torque magnitude is clamped to device max_torque_nm
    /// for various device configurations.
    #[test]
    fn test_torque_clamping_various_device_limits() {
        let device_limits = [5.0, 10.0, 15.0, 20.0, 25.0];

        for max_torque in device_limits {
            let config = SafetyEnvelopeConfig {
                max_torque_nm: max_torque,
                max_slew_rate_nm_per_s: 1000.0, // High to not interfere
                max_jerk_nm_per_s2: 10000.0,
                timestep_s: 0.004,
                ..Default::default()
            };

            let mut envelope = SafetyEnvelope::new(config).unwrap();

            // Test positive clamping - request 2x max
            for _ in 0..100 {
                let output = envelope.apply(max_torque * 2.0, true).unwrap();
                assert!(
                    output <= max_torque + 0.01,
                    "Positive torque should be clamped to max {} for device limit {}: got {}",
                    max_torque,
                    max_torque,
                    output
                );
            }

            // Reset and test negative clamping
            envelope.reset();
            for _ in 0..100 {
                let output = envelope.apply(-max_torque * 2.0, true).unwrap();
                assert!(
                    output >= -max_torque - 0.01,
                    "Negative torque should be clamped to -{} for device limit {}: got {}",
                    max_torque,
                    max_torque,
                    output
                );
            }
        }
    }

    /// **Test: Slew Rate Limiting Across Different Rates**
    /// **Validates: Requirement FFB-SAFETY-01.2**
    ///
    /// Verifies that ΔNm/Δt ≤ configured slew rate limit for various configurations.
    #[test]
    fn test_slew_rate_limiting_various_rates() {
        let slew_rates = [5.0, 10.0, 25.0, 50.0, 100.0];
        let timestep = 0.004; // 250Hz

        for max_slew_rate in slew_rates {
            let config = SafetyEnvelopeConfig {
                max_torque_nm: 50.0, // High to not interfere
                max_slew_rate_nm_per_s: max_slew_rate,
                max_jerk_nm_per_s2: 10000.0, // High to not interfere
                timestep_s: timestep,
                ..Default::default()
            };

            let mut envelope = SafetyEnvelope::new(config).unwrap();

            let mut last_torque = 0.0;
            let mut max_observed_slew: f32 = 0.0;

            // Request large torque change
            for _ in 0..100 {
                let output = envelope.apply(30.0, true).unwrap();
                let delta = output - last_torque;
                let slew_rate_actual = delta / timestep;

                max_observed_slew = max_observed_slew.max(slew_rate_actual.abs());

                // Verify slew rate limit (with small tolerance)
                assert!(
                    slew_rate_actual.abs() <= max_slew_rate + 0.5,
                    "Slew rate exceeded for config {}: {} > {}",
                    max_slew_rate,
                    slew_rate_actual.abs(),
                    max_slew_rate
                );

                last_torque = output;
            }

            // Verify we actually hit near the slew rate limit
            assert!(
                max_observed_slew >= max_slew_rate * 0.8,
                "Should reach near slew rate limit {} for config {}: got {}",
                max_slew_rate * 0.8,
                max_slew_rate,
                max_observed_slew
            );
        }
    }

    /// **Test: Jerk Limiting Across Different Rates**
    /// **Validates: Requirement FFB-SAFETY-01.3**
    ///
    /// Verifies that Δ²Nm/Δt² ≤ configured jerk limit for various configurations.
    #[test]
    fn test_jerk_limiting_various_rates() {
        let jerk_limits = [50.0, 100.0, 200.0, 500.0];
        let timestep = 0.004;

        for max_jerk in jerk_limits {
            let config = SafetyEnvelopeConfig {
                max_torque_nm: 50.0,
                max_slew_rate_nm_per_s: 100.0,
                max_jerk_nm_per_s2: max_jerk,
                timestep_s: timestep,
                ..Default::default()
            };

            let mut envelope = SafetyEnvelope::new(config).unwrap();

            let mut last_torque = 0.0;
            let mut last_slew_rate = 0.0;
            let mut max_observed_jerk: f32 = 0.0;

            for i in 0..100 {
                let output = envelope.apply(30.0, true).unwrap();
                let delta = output - last_torque;
                let slew_rate = delta / timestep;
                let jerk = (slew_rate - last_slew_rate) / timestep;

                if i > 0 {
                    max_observed_jerk = max_observed_jerk.max(jerk.abs());

                    // Verify jerk limit (with tolerance)
                    assert!(
                        jerk.abs() <= max_jerk + 5.0,
                        "Jerk exceeded for config {}: {} > {} at iteration {}",
                        max_jerk,
                        jerk.abs(),
                        max_jerk,
                        i
                    );
                }

                last_torque = output;
                last_slew_rate = slew_rate;
            }

            // Verify we actually hit near the jerk limit
            assert!(
                max_observed_jerk >= max_jerk * 0.7,
                "Should reach near jerk limit {} for config {}: got {}",
                max_jerk * 0.7,
                max_jerk,
                max_observed_jerk
            );
        }
    }

    // ============================================================================
    // SECTION 2: 50ms Ramp Tests with fault_initial_torque
    // ============================================================================

    /// **Test: 50ms Ramp from Arbitrary Starting Torques**
    /// **Validates: Requirement FFB-SAFETY-01.6, QG-FFB-SAFETY**
    ///
    /// Verifies that fault triggers 50ms ramp to zero from various starting torques.
    /// Uses `fault_initial_torque` captured at fault detection.
    #[test]
    fn test_50ms_ramp_from_arbitrary_starting_torques() {
        let starting_torques = [2.0, 5.0, 8.0, 10.0, 12.0, 15.0];

        for target_torque in starting_torques {
            let config = SafetyEnvelopeConfig {
                max_torque_nm: 20.0,
                max_slew_rate_nm_per_s: 1000.0, // High to reach target quickly
                max_jerk_nm_per_s2: 10000.0,
                fault_ramp_time: Duration::from_millis(50),
                timestep_s: 0.004,
            };

            let mut envelope = SafetyEnvelope::new(config).unwrap();

            // Build up to target torque
            for _ in 0..200 {
                envelope.apply(target_torque, true).unwrap();
            }

            let actual_torque = envelope.get_last_torque();
            assert!(
                (actual_torque - target_torque).abs() < 1.0,
                "Should reach target torque {}: got {}",
                target_torque,
                actual_torque
            );

            // Trigger fault ramp
            envelope.trigger_fault_ramp();
            assert!(envelope.is_in_fault_ramp());

            let fault_start = Instant::now();
            let mut samples: Vec<(Duration, f32)> = Vec::new();

            // Simulate updates over 60ms (to ensure we capture the full 50ms ramp)
            while fault_start.elapsed() < Duration::from_millis(60) {
                let output = envelope.apply(target_torque, true).unwrap();
                samples.push((fault_start.elapsed(), output));
                thread::sleep(Duration::from_millis(4));
            }

            // Verify ramp characteristics
            assert!(!samples.is_empty(), "Should have collected samples");

            // Check monotonic decrease
            for i in 1..samples.len() {
                assert!(
                    samples[i].1 <= samples[i - 1].1 + 0.1,
                    "Torque should decrease monotonically from {}: {} > {} at sample {}",
                    target_torque,
                    samples[i].1,
                    samples[i - 1].1,
                    i
                );
            }

            // Check that we reach near-zero by 50ms
            let torque_at_50ms = samples
                .iter()
                .find(|(elapsed, _)| *elapsed >= Duration::from_millis(50))
                .map(|(_, torque)| *torque);

            if let Some(torque) = torque_at_50ms {
                assert!(
                    torque.abs() < 1.0,
                    "Torque should be near zero at 50ms for starting torque {}: got {}",
                    target_torque,
                    torque
                );
            }

            // Verify final torque is zero
            let final_torque = samples.last().unwrap().1;
            assert!(
                final_torque.abs() < 0.5,
                "Final torque should be near zero for starting torque {}: got {}",
                target_torque,
                final_torque
            );
        }
    }

    /// **Test: 50ms Ramp Timing Precision**
    /// **Validates: Requirement FFB-SAFETY-01.6, QG-FFB-SAFETY**
    ///
    /// Verifies that the 50ms ramp completes within the required time window.
    #[test]
    fn test_50ms_ramp_timing_precision() {
        let config = SafetyEnvelopeConfig {
            max_torque_nm: 15.0,
            max_slew_rate_nm_per_s: 1000.0,
            max_jerk_nm_per_s2: 10000.0,
            fault_ramp_time: Duration::from_millis(50),
            timestep_s: 0.004,
        };

        let mut envelope = SafetyEnvelope::new(config).unwrap();

        // Build up to 10 Nm
        for _ in 0..200 {
            envelope.apply(10.0, true).unwrap();
        }

        let initial_torque = envelope.get_last_torque();
        assert!(initial_torque > 8.0, "Should have significant torque");

        // Trigger fault
        envelope.trigger_fault_ramp();
        let fault_start = Instant::now();

        // Sample at specific time points
        let mut time_to_50_percent: Option<Duration> = None;
        let mut time_to_90_percent: Option<Duration> = None;
        let mut time_to_zero: Option<Duration> = None;

        while fault_start.elapsed() < Duration::from_millis(70) {
            let output = envelope.apply(10.0, true).unwrap();
            let elapsed = fault_start.elapsed();
            let progress = 1.0 - (output / initial_torque);

            if time_to_50_percent.is_none() && progress >= 0.5 {
                time_to_50_percent = Some(elapsed);
            }
            if time_to_90_percent.is_none() && progress >= 0.9 {
                time_to_90_percent = Some(elapsed);
            }
            if time_to_zero.is_none() && output.abs() < 0.5 {
                time_to_zero = Some(elapsed);
            }

            thread::sleep(Duration::from_millis(2));
        }

        // Verify timing
        if let Some(t) = time_to_50_percent {
            assert!(
                t <= Duration::from_millis(30),
                "Should reach 50% within 30ms: {:?}",
                t
            );
        }

        if let Some(t) = time_to_zero {
            assert!(
                t <= Duration::from_millis(55),
                "Should reach zero within 55ms (50ms + tolerance): {:?}",
                t
            );
        }
    }

    /// **Test: fault_initial_torque Capture**
    /// **Validates: Requirement FFB-SAFETY-01.6**
    ///
    /// Verifies that fault_initial_torque is captured at fault detection,
    /// not from last_setpoint which may have changed.
    #[test]
    fn test_fault_initial_torque_capture() {
        let config = SafetyEnvelopeConfig {
            max_torque_nm: 15.0,
            max_slew_rate_nm_per_s: 1000.0,
            max_jerk_nm_per_s2: 10000.0,
            fault_ramp_time: Duration::from_millis(50),
            timestep_s: 0.004,
        };

        let mut envelope = SafetyEnvelope::new(config).unwrap();

        // Build up to 12 Nm
        for _ in 0..200 {
            envelope.apply(12.0, true).unwrap();
        }

        let torque_at_fault = envelope.get_last_torque();
        assert!(
            (torque_at_fault - 12.0).abs() < 1.0,
            "Should be at ~12 Nm before fault"
        );

        // Trigger fault
        envelope.trigger_fault_ramp();

        // The ramp should start from torque_at_fault, not from any subsequent value
        // First sample after fault should be close to initial torque
        let first_output = envelope.apply(12.0, true).unwrap();

        // First output should be slightly less than initial (ramp has started)
        // but not dramatically different
        assert!(
            first_output <= torque_at_fault + 0.1,
            "First output after fault should be <= initial torque: {} vs {}",
            first_output,
            torque_at_fault
        );
        assert!(
            first_output >= torque_at_fault * 0.8,
            "First output should be close to initial (ramp just started): {} vs {}",
            first_output,
            torque_at_fault
        );
    }

    // ============================================================================
    // SECTION 3: SafeTorque (30%) vs HighTorque (100%) vs Faulted (0%) Tests
    // ============================================================================

    /// **Test: SafeTorque Mode - 30% Envelope**
    /// **Validates: Requirements FFB-SAFETY-01.1, Design Decision**
    ///
    /// Verifies that SafeTorque mode limits torque to 30% of device max.
    #[test]
    fn test_safe_torque_mode_30_percent_envelope() {
        let device_max = 15.0;
        let safe_torque_limit = device_max * 0.3; // 4.5 Nm

        // SafetyState provides the max_torque_nm for each mode
        let state = SafetyState::SafeTorque;
        let effective_max = state.max_torque_nm(device_max);

        // Verify SafeTorque is 30% (or 5Nm cap, whichever is lower)
        let expected = (device_max * 0.3).min(5.0);
        assert!(
            (effective_max - expected).abs() < 0.01,
            "SafeTorque should limit to 30% or 5Nm: expected {}, got {}",
            expected,
            effective_max
        );

        // Create envelope with SafeTorque limits
        let config = SafetyEnvelopeConfig {
            max_torque_nm: effective_max,
            max_slew_rate_nm_per_s: 1000.0,
            max_jerk_nm_per_s2: 10000.0,
            timestep_s: 0.004,
            ..Default::default()
        };

        let mut envelope = SafetyEnvelope::new(config).unwrap();

        // Request full device torque
        for _ in 0..100 {
            let output = envelope.apply(device_max, true).unwrap();
            assert!(
                output <= effective_max + 0.01,
                "SafeTorque should clamp to {}: got {}",
                effective_max,
                output
            );
        }

        // Verify we reach the SafeTorque limit
        let final_torque = envelope.get_last_torque();
        assert!(
            (final_torque - effective_max).abs() < 0.5,
            "Should reach SafeTorque limit {}: got {}",
            effective_max,
            final_torque
        );
    }

    /// **Test: HighTorque Mode - 100% Envelope**
    /// **Validates: Requirements FFB-SAFETY-01.1, Design Decision**
    ///
    /// Verifies that HighTorque mode allows full device torque.
    #[test]
    fn test_high_torque_mode_100_percent_envelope() {
        let device_max = 15.0;

        // SafetyState provides the max_torque_nm for each mode
        let state = SafetyState::HighTorque;
        let effective_max = state.max_torque_nm(device_max);

        // Verify HighTorque is 100%
        assert!(
            (effective_max - device_max).abs() < 0.01,
            "HighTorque should allow full device max: expected {}, got {}",
            device_max,
            effective_max
        );

        // Create envelope with HighTorque limits
        let config = SafetyEnvelopeConfig {
            max_torque_nm: effective_max,
            max_slew_rate_nm_per_s: 1000.0,
            max_jerk_nm_per_s2: 10000.0,
            timestep_s: 0.004,
            ..Default::default()
        };

        let mut envelope = SafetyEnvelope::new(config).unwrap();

        // Request full device torque
        for _ in 0..100 {
            let output = envelope.apply(device_max, true).unwrap();
            assert!(
                output <= device_max + 0.01,
                "HighTorque should allow up to {}: got {}",
                device_max,
                output
            );
        }

        // Verify we reach the full device limit
        let final_torque = envelope.get_last_torque();
        assert!(
            (final_torque - device_max).abs() < 0.5,
            "Should reach full device max {}: got {}",
            device_max,
            final_torque
        );
    }

    /// **Test: Faulted Mode - 0% Envelope (Zero Torque)**
    /// **Validates: Requirements FFB-SAFETY-01.1, FFB-SAFETY-01.4, Design Decision**
    ///
    /// Verifies that Faulted mode outputs zero torque regardless of input.
    #[test]
    fn test_faulted_mode_zero_torque() {
        let device_max = 15.0;

        // SafetyState provides the max_torque_nm for each mode
        let state = SafetyState::Faulted;
        let effective_max = state.max_torque_nm(device_max);

        // Verify Faulted is 0%
        assert!(
            effective_max.abs() < 0.01,
            "Faulted should output zero torque: expected 0, got {}",
            effective_max
        );

        // Verify Faulted state doesn't allow torque
        assert!(
            !state.allows_torque(),
            "Faulted state should not allow torque"
        );
        assert!(
            !state.allows_high_torque(),
            "Faulted state should not allow high torque"
        );
    }

    /// **Test: Mode Comparison - SafeTorque vs HighTorque vs Faulted**
    /// **Validates: Requirements FFB-SAFETY-01.1-6, Design Decision**
    ///
    /// Comprehensive comparison of all three safety modes.
    #[test]
    fn test_mode_comparison_all_three_modes() {
        let device_max = 15.0;

        // Get effective limits for each mode
        let safe_torque_max = SafetyState::SafeTorque.max_torque_nm(device_max);
        let high_torque_max = SafetyState::HighTorque.max_torque_nm(device_max);
        let faulted_max = SafetyState::Faulted.max_torque_nm(device_max);

        // Verify ordering: Faulted < SafeTorque < HighTorque
        assert!(
            faulted_max < safe_torque_max,
            "Faulted ({}) should be less than SafeTorque ({})",
            faulted_max,
            safe_torque_max
        );
        assert!(
            safe_torque_max < high_torque_max,
            "SafeTorque ({}) should be less than HighTorque ({})",
            safe_torque_max,
            high_torque_max
        );

        // Verify specific percentages
        assert!(
            faulted_max == 0.0,
            "Faulted should be 0%: got {}",
            faulted_max
        );

        let expected_safe = (device_max * 0.3).min(5.0);
        assert!(
            (safe_torque_max - expected_safe).abs() < 0.01,
            "SafeTorque should be 30% or 5Nm cap: expected {}, got {}",
            expected_safe,
            safe_torque_max
        );

        assert!(
            (high_torque_max - device_max).abs() < 0.01,
            "HighTorque should be 100%: expected {}, got {}",
            device_max,
            high_torque_max
        );

        // Verify torque allowance
        assert!(SafetyState::SafeTorque.allows_torque());
        assert!(SafetyState::HighTorque.allows_torque());
        assert!(!SafetyState::Faulted.allows_torque());

        // Verify high torque allowance
        assert!(!SafetyState::SafeTorque.allows_high_torque());
        assert!(SafetyState::HighTorque.allows_high_torque());
        assert!(!SafetyState::Faulted.allows_high_torque());
    }

    /// **Test: SafeTorque Mode with Various Device Limits**
    /// **Validates: Requirements FFB-SAFETY-01.1, Design Decision**
    ///
    /// Verifies SafeTorque 30% envelope across different device configurations.
    #[test]
    fn test_safe_torque_mode_various_device_limits() {
        let device_limits = [5.0, 10.0, 15.0, 20.0, 25.0, 30.0];

        for device_max in device_limits {
            let safe_torque_max = SafetyState::SafeTorque.max_torque_nm(device_max);

            // SafeTorque is 30% of device max, capped at 5Nm
            let expected = (device_max * 0.3).min(5.0);

            assert!(
                (safe_torque_max - expected).abs() < 0.01,
                "SafeTorque for device {} should be {}: got {}",
                device_max,
                expected,
                safe_torque_max
            );

            // Create envelope and verify clamping
            let config = SafetyEnvelopeConfig {
                max_torque_nm: safe_torque_max,
                max_slew_rate_nm_per_s: 1000.0,
                max_jerk_nm_per_s2: 10000.0,
                timestep_s: 0.004,
                ..Default::default()
            };

            let mut envelope = SafetyEnvelope::new(config).unwrap();

            // Request full device torque
            for _ in 0..100 {
                let output = envelope.apply(device_max, true).unwrap();
                assert!(
                    output <= safe_torque_max + 0.01,
                    "SafeTorque for device {} should clamp to {}: got {}",
                    device_max,
                    safe_torque_max,
                    output
                );
            }
        }
    }

    /// **Test: State Transitions Between Modes**
    /// **Validates: Requirements FFB-SAFETY-01.9, FFB-SAFETY-01.10**
    ///
    /// Verifies valid and invalid state transitions between safety modes.
    #[test]
    fn test_state_transitions_between_modes() {
        let mut manager = SafetyStateManager::new();

        // Initial state should be SafeTorque
        assert_eq!(manager.current_state(), SafetyState::SafeTorque);

        // Valid: SafeTorque -> HighTorque
        assert!(SafetyState::SafeTorque.can_transition_to(SafetyState::HighTorque));
        manager
            .transition_to(
                SafetyState::HighTorque,
                TransitionReason::UserEnableHighTorque,
            )
            .unwrap();
        assert_eq!(manager.current_state(), SafetyState::HighTorque);

        // Valid: HighTorque -> SafeTorque
        assert!(SafetyState::HighTorque.can_transition_to(SafetyState::SafeTorque));
        manager
            .transition_to(
                SafetyState::SafeTorque,
                TransitionReason::UserDisableHighTorque,
            )
            .unwrap();
        assert_eq!(manager.current_state(), SafetyState::SafeTorque);

        // Valid: SafeTorque -> Faulted
        assert!(SafetyState::SafeTorque.can_transition_to(SafetyState::Faulted));
        manager
            .transition_to(
                SafetyState::Faulted,
                TransitionReason::FaultDetected {
                    fault_reason: FaultReason::UsbStall,
                },
            )
            .unwrap();
        assert_eq!(manager.current_state(), SafetyState::Faulted);

        // Invalid: Faulted -> HighTorque (requires power cycle first)
        assert!(!SafetyState::Faulted.can_transition_to(SafetyState::HighTorque));
        let result = manager.transition_to(
            SafetyState::HighTorque,
            TransitionReason::UserEnableHighTorque,
        );
        assert!(result.is_err());

        // Valid: Faulted -> SafeTorque (after power cycle)
        assert!(SafetyState::Faulted.can_transition_to(SafetyState::SafeTorque));
        manager
            .transition_to(SafetyState::SafeTorque, TransitionReason::PowerCycleReset)
            .unwrap();
        assert_eq!(manager.current_state(), SafetyState::SafeTorque);
    }

    /// **Test: Mode Switching During Operation**
    /// **Validates: Requirements FFB-SAFETY-01.1-6**
    ///
    /// Verifies that switching between modes correctly changes torque limits.
    #[test]
    fn test_mode_switching_during_operation() {
        let device_max = 15.0;

        // Start in SafeTorque mode
        let safe_max = SafetyState::SafeTorque.max_torque_nm(device_max);
        let config = SafetyEnvelopeConfig {
            max_torque_nm: safe_max,
            max_slew_rate_nm_per_s: 1000.0,
            max_jerk_nm_per_s2: 10000.0,
            timestep_s: 0.004,
            ..Default::default()
        };

        let mut envelope = SafetyEnvelope::new(config).unwrap();

        // Build up torque in SafeTorque mode
        for _ in 0..100 {
            envelope.apply(device_max, true).unwrap();
        }

        let safe_torque_output = envelope.get_last_torque();
        assert!(
            safe_torque_output <= safe_max + 0.1,
            "SafeTorque should limit to {}: got {}",
            safe_max,
            safe_torque_output
        );

        // Switch to HighTorque mode by updating config
        let high_max = SafetyState::HighTorque.max_torque_nm(device_max);
        let high_config = SafetyEnvelopeConfig {
            max_torque_nm: high_max,
            max_slew_rate_nm_per_s: 1000.0,
            max_jerk_nm_per_s2: 10000.0,
            timestep_s: 0.004,
            ..Default::default()
        };

        envelope.update_config(high_config).unwrap();

        // Continue applying torque - should now be able to reach higher values
        for _ in 0..100 {
            envelope.apply(device_max, true).unwrap();
        }

        let high_torque_output = envelope.get_last_torque();
        assert!(
            high_torque_output > safe_max,
            "HighTorque should allow more than SafeTorque: {} vs {}",
            high_torque_output,
            safe_max
        );
        assert!(
            (high_torque_output - high_max).abs() < 0.5,
            "HighTorque should reach device max {}: got {}",
            high_max,
            high_torque_output
        );
    }

    // ============================================================================
    // SECTION 4: Combined Safety Tests
    // ============================================================================

    /// **Test: Combined Clamping, Slew, and Jerk Limits**
    /// **Validates: Requirements FFB-SAFETY-01.1-3**
    ///
    /// Verifies that all safety constraints work together correctly.
    #[test]
    fn test_combined_safety_constraints() {
        let config = SafetyEnvelopeConfig {
            max_torque_nm: 10.0,
            max_slew_rate_nm_per_s: 20.0,
            max_jerk_nm_per_s2: 200.0,
            timestep_s: 0.004,
            ..Default::default()
        };

        let max_torque = config.max_torque_nm;
        let max_slew_rate = config.max_slew_rate_nm_per_s;
        let max_jerk = config.max_jerk_nm_per_s2;
        let timestep = config.timestep_s;

        let mut envelope = SafetyEnvelope::new(config).unwrap();

        let mut last_torque = 0.0;
        let mut last_slew_rate = 0.0;

        // Request large torque change
        for i in 0..200 {
            let output = envelope.apply(15.0, true).unwrap();

            // Verify torque clamping
            assert!(
                output.abs() <= max_torque + 0.01,
                "Torque should be clamped at iteration {}: {}",
                i,
                output
            );

            // Verify slew rate limiting
            let delta = output - last_torque;
            let slew_rate = delta / timestep;
            assert!(
                slew_rate.abs() <= max_slew_rate + 0.5,
                "Slew rate should be limited at iteration {}: {}",
                i,
                slew_rate
            );

            // Verify jerk limiting (skip first iteration and when at torque limit)
            if i > 0 && output.abs() < max_torque - 0.1 {
                let jerk = (slew_rate - last_slew_rate) / timestep;
                assert!(
                    jerk.abs() <= max_jerk + 5.0,
                    "Jerk should be limited at iteration {}: {}",
                    i,
                    jerk
                );
            }

            last_torque = output;
            last_slew_rate = slew_rate;
        }

        // Should eventually reach the clamped maximum
        let final_torque = envelope.get_last_torque();
        assert!(
            (final_torque - max_torque).abs() < 0.5,
            "Should reach max torque: {} vs {}",
            final_torque,
            max_torque
        );
    }

    /// **Test: Fault Ramp Overrides Normal Operation**
    /// **Validates: Requirements FFB-SAFETY-01.4, FFB-SAFETY-01.6**
    ///
    /// Verifies that fault ramp takes precedence over normal torque requests.
    #[test]
    fn test_fault_ramp_overrides_normal_operation() {
        let config = SafetyEnvelopeConfig {
            max_torque_nm: 15.0,
            max_slew_rate_nm_per_s: 1000.0,
            max_jerk_nm_per_s2: 10000.0,
            fault_ramp_time: Duration::from_millis(50),
            timestep_s: 0.004,
        };

        let mut envelope = SafetyEnvelope::new(config).unwrap();

        // Build up torque
        for _ in 0..100 {
            envelope.apply(10.0, true).unwrap();
        }

        let initial_torque = envelope.get_last_torque();
        assert!(initial_torque > 8.0, "Should have significant torque");

        // Trigger fault
        envelope.trigger_fault_ramp();

        // Even with safe_for_ffb=true and high torque request, should ramp to zero
        let fault_start = Instant::now();
        while fault_start.elapsed() < Duration::from_millis(60) {
            let output = envelope.apply(15.0, true).unwrap(); // Request high torque

            // Output should be decreasing toward zero
            assert!(
                output <= initial_torque + 0.1,
                "Fault ramp should override torque request: {}",
                output
            );

            thread::sleep(Duration::from_millis(4));
        }

        // Should be at zero after 50ms
        let final_torque = envelope.get_last_torque();
        assert!(
            final_torque.abs() < 0.5,
            "Should reach zero despite torque requests: {}",
            final_torque
        );
    }

    /// **Test: safe_for_ffb False Targets Zero**
    /// **Validates: Requirements FFB-SAFETY-01.4**
    ///
    /// Verifies that setting safe_for_ffb to false targets zero torque.
    /// Note: This uses normal rate limiting, not the 50ms fault ramp.
    /// For 50ms fault ramp, use trigger_fault_ramp() explicitly.
    #[test]
    fn test_safe_for_ffb_false_targets_zero() {
        let config = SafetyEnvelopeConfig {
            max_torque_nm: 15.0,
            max_slew_rate_nm_per_s: 500.0, // High slew rate to build up quickly
            max_jerk_nm_per_s2: 5000.0,    // High jerk to build up quickly
            fault_ramp_time: Duration::from_millis(50),
            timestep_s: 0.004,
        };

        let mut envelope = SafetyEnvelope::new(config).unwrap();

        // Build up torque with safe_for_ffb=true
        for _ in 0..200 {
            envelope.apply(10.0, true).unwrap();
        }

        let torque_before = envelope.get_last_torque();
        assert!(
            torque_before > 8.0,
            "Should have significant torque: {}",
            torque_before
        );

        // Set safe_for_ffb to false - should target zero through rate limiting
        // Note: Due to jerk limiting, the torque may overshoot slightly before settling at zero
        for _ in 0..500 {
            let output = envelope.apply(10.0, false).unwrap();

            // Break early if we've reached near zero
            if output.abs() < 0.1 {
                break;
            }
        }

        // Should eventually reach near zero
        let final_torque = envelope.get_last_torque();
        assert!(
            final_torque.abs() < 0.5,
            "Should reach near zero when safe_for_ffb=false: {}",
            final_torque
        );
    }

    /// **Test: safe_for_ffb False with Fault Ramp**
    /// **Validates: Requirements FFB-SAFETY-01.4, FFB-SAFETY-01.6**
    ///
    /// Verifies that triggering fault ramp when safe_for_ffb is false
    /// completes within 50ms.
    #[test]
    fn test_safe_for_ffb_false_with_fault_ramp() {
        let config = SafetyEnvelopeConfig {
            max_torque_nm: 15.0,
            max_slew_rate_nm_per_s: 1000.0,
            max_jerk_nm_per_s2: 10000.0,
            fault_ramp_time: Duration::from_millis(50),
            timestep_s: 0.004,
        };

        let mut envelope = SafetyEnvelope::new(config).unwrap();

        // Build up torque with safe_for_ffb=true
        for _ in 0..100 {
            envelope.apply(10.0, true).unwrap();
        }

        let torque_before = envelope.get_last_torque();
        assert!(torque_before > 8.0, "Should have significant torque");

        // Trigger fault ramp (this is what should happen when safe_for_ffb becomes false
        // in the FFB pipeline - the pipeline triggers the fault ramp)
        envelope.trigger_fault_ramp();

        // Now apply with safe_for_ffb=false and wait for 50ms ramp
        let start = Instant::now();
        while start.elapsed() < Duration::from_millis(60) {
            let output = envelope.apply(10.0, false).unwrap();

            // Output should be decreasing toward zero
            assert!(
                output <= torque_before + 0.1,
                "Should be ramping toward zero: {}",
                output
            );

            thread::sleep(Duration::from_millis(4));
        }

        // Should be at zero after 50ms fault ramp
        let final_torque = envelope.get_last_torque();
        assert!(
            final_torque.abs() < 0.5,
            "Should reach zero within 50ms fault ramp: {}",
            final_torque
        );
    }

    /// **Test: Recovery After Fault Clear**
    /// **Validates: Requirements FFB-SAFETY-01.10**
    ///
    /// Verifies that torque can be applied again after clearing a transient fault.
    #[test]
    fn test_recovery_after_fault_clear() {
        let config = SafetyEnvelopeConfig {
            max_torque_nm: 15.0,
            max_slew_rate_nm_per_s: 1000.0,
            max_jerk_nm_per_s2: 10000.0,
            fault_ramp_time: Duration::from_millis(50),
            timestep_s: 0.004,
        };

        let mut envelope = SafetyEnvelope::new(config).unwrap();

        // Build up torque
        for _ in 0..100 {
            envelope.apply(10.0, true).unwrap();
        }

        // Trigger fault and wait for ramp
        envelope.trigger_fault_ramp();
        thread::sleep(Duration::from_millis(60));
        for _ in 0..20 {
            envelope.apply(10.0, true).unwrap();
        }

        // Should be at zero
        assert!(
            envelope.get_last_torque().abs() < 0.5,
            "Should be at zero after fault ramp"
        );

        // Clear fault
        envelope.clear_fault();
        assert!(!envelope.is_in_fault_ramp());

        // Should be able to build up torque again
        for _ in 0..100 {
            envelope.apply(10.0, true).unwrap();
        }

        let recovered_torque = envelope.get_last_torque();
        assert!(
            recovered_torque > 8.0,
            "Should be able to apply torque after fault clear: {}",
            recovered_torque
        );
    }

    /// **Test: Negative Torque Handling**
    /// **Validates: Requirements FFB-SAFETY-01.1-3**
    ///
    /// Verifies that negative torque values are handled correctly.
    #[test]
    fn test_negative_torque_handling() {
        let config = SafetyEnvelopeConfig {
            max_torque_nm: 10.0,
            max_slew_rate_nm_per_s: 50.0,
            max_jerk_nm_per_s2: 500.0,
            timestep_s: 0.004,
            ..Default::default()
        };

        let mut envelope = SafetyEnvelope::new(config).unwrap();

        // Request negative torque
        for _ in 0..100 {
            let output = envelope.apply(-15.0, true).unwrap();

            // Should be clamped to -max
            assert!(
                output >= -10.0 - 0.01,
                "Negative torque should be clamped: {}",
                output
            );
        }

        let final_torque = envelope.get_last_torque();
        assert!(
            (final_torque - (-10.0)).abs() < 0.5,
            "Should reach negative max: {}",
            final_torque
        );

        // Now request positive torque - should transition smoothly
        for _ in 0..200 {
            let output = envelope.apply(10.0, true).unwrap();
            assert!(
                output.abs() <= 10.0 + 0.01,
                "Torque should stay within bounds during transition: {}",
                output
            );
        }

        let final_positive = envelope.get_last_torque();
        assert!(
            (final_positive - 10.0).abs() < 0.5,
            "Should reach positive max: {}",
            final_positive
        );
    }
}
