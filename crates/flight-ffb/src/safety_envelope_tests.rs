// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Comprehensive unit tests for FFB safety envelope
//!
//! **Validates: Requirements FFB-SAFETY-01.1, FFB-SAFETY-01.2, FFB-SAFETY-01.3, FFB-SAFETY-01.4, FFB-SAFETY-01.6**
//! **Validates: Test Requirements SIM-TEST-01.10, QG-FFB-SAFETY**

#[cfg(test)]
mod safety_envelope_tests {
    use crate::safety_envelope::*;
    use std::thread;
    use std::time::Duration;

    /// **Test: Torque Clamping**
    /// **Validates: Requirement FFB-SAFETY-01.1**
    ///
    /// Verifies that torque magnitude is clamped to device max_torque_nm
    #[test]
    fn test_torque_clamping() {
        let config = SafetyEnvelopeConfig {
            max_torque_nm: 10.0,
            max_slew_rate_nm_per_s: 1000.0, // High enough to not interfere
            max_jerk_nm_per_s2: 10000.0,    // High enough to not interfere
            timestep_s: 0.004,
            ..Default::default()
        };

        let mut envelope = SafetyEnvelope::new(config).unwrap();

        // Test positive clamping
        let output = envelope.apply(15.0, true).unwrap();
        assert!(
            output <= 10.0,
            "Positive torque should be clamped to max: {} > 10.0",
            output
        );

        // Reset for clean test
        envelope.reset();

        // Test negative clamping
        let output = envelope.apply(-15.0, true).unwrap();
        assert!(
            output >= -10.0,
            "Negative torque should be clamped to -max: {} < -10.0",
            output
        );

        // Test within bounds
        envelope.reset();
        let output = envelope.apply(5.0, true).unwrap();
        // Due to slew rate limiting, we won't reach 5.0 immediately, but it should be moving toward it
        assert!(
            output.abs() <= 10.0,
            "Torque within bounds should not exceed max: {}",
            output
        );
    }

    /// **Test: Slew Rate Limiting**
    /// **Validates: Requirement FFB-SAFETY-01.2**
    ///
    /// Verifies that ΔNm/Δt ≤ configured slew rate limit
    #[test]
    fn test_slew_rate_limiting() {
        let config = SafetyEnvelopeConfig {
            max_torque_nm: 20.0,
            max_slew_rate_nm_per_s: 10.0, // 10 Nm/s limit
            max_jerk_nm_per_s2: 10000.0,  // High enough to not interfere
            timestep_s: 0.004,            // 4ms timestep
            ..Default::default()
        };

        let max_slew_rate = config.max_slew_rate_nm_per_s;
        let timestep = config.timestep_s;

        let mut envelope = SafetyEnvelope::new(config).unwrap();

        // Start at 0, request 15 Nm
        let mut last_torque = 0.0;
        let mut max_observed_slew_rate: f32 = 0.0;

        for _ in 0..100 {
            let output = envelope.apply(15.0, true).unwrap();
            let delta = output - last_torque;
            let slew_rate = delta / timestep;

            max_observed_slew_rate = max_observed_slew_rate.max(slew_rate.abs());

            // Check that slew rate doesn't exceed limit (with small tolerance for numerical errors)
            assert!(
                slew_rate.abs() <= max_slew_rate + 0.1,
                "Slew rate exceeded limit: {} > {}",
                slew_rate.abs(),
                max_slew_rate
            );

            last_torque = output;
        }

        // Verify we actually hit the slew rate limit
        assert!(
            max_observed_slew_rate >= max_slew_rate * 0.9,
            "Should have reached near slew rate limit: {} < {}",
            max_observed_slew_rate,
            max_slew_rate * 0.9
        );
    }

    /// **Test: Jerk Limiting**
    /// **Validates: Requirement FFB-SAFETY-01.3**
    ///
    /// Verifies that Δ²Nm/Δt² ≤ configured jerk limit
    #[test]
    fn test_jerk_limiting() {
        let config = SafetyEnvelopeConfig {
            max_torque_nm: 20.0,
            max_slew_rate_nm_per_s: 50.0,
            max_jerk_nm_per_s2: 100.0, // 100 Nm/s² jerk limit
            timestep_s: 0.004,
            ..Default::default()
        };

        let max_jerk = config.max_jerk_nm_per_s2;
        let timestep = config.timestep_s;

        let mut envelope = SafetyEnvelope::new(config).unwrap();

        let mut last_torque = 0.0;
        let mut last_slew_rate = 0.0;
        let mut max_observed_jerk: f32 = 0.0;

        for _ in 0..100 {
            let output = envelope.apply(15.0, true).unwrap();
            let delta = output - last_torque;
            let slew_rate = delta / timestep;
            let jerk = (slew_rate - last_slew_rate) / timestep;

            max_observed_jerk = max_observed_jerk.max(jerk.abs());

            // Check that jerk doesn't exceed limit (with tolerance for numerical errors)
            assert!(
                jerk.abs() <= max_jerk + 1.0,
                "Jerk exceeded limit: {} > {}",
                jerk.abs(),
                max_jerk
            );

            last_torque = output;
            last_slew_rate = slew_rate;
        }

        // Verify we actually hit the jerk limit
        assert!(
            max_observed_jerk >= max_jerk * 0.8,
            "Should have reached near jerk limit: {} < {}",
            max_observed_jerk,
            max_jerk * 0.8
        );
    }

    /// **Test: safe_for_ffb Flag Enforcement**
    /// **Validates: Requirement FFB-SAFETY-01.4**
    ///
    /// Verifies that torque is zero when safe_for_ffb is false
    #[test]
    fn test_safe_for_ffb_enforcement() {
        let config = SafetyEnvelopeConfig {
            max_torque_nm: 15.0,
            max_slew_rate_nm_per_s: 1000.0,
            max_jerk_nm_per_s2: 10000.0,
            timestep_s: 0.004,
            ..Default::default()
        };

        let mut envelope = SafetyEnvelope::new(config).unwrap();

        // Build up some torque first
        for _ in 0..50 {
            envelope.apply(10.0, true).unwrap();
        }

        // Verify we have non-zero torque
        let torque_before = envelope.get_last_torque();
        assert!(
            torque_before > 0.0,
            "Should have built up torque: {}",
            torque_before
        );

        // Now set safe_for_ffb to false
        let output = envelope.apply(10.0, false).unwrap();

        // Output should be ramping toward zero (not necessarily zero immediately due to rate limiting)
        // But the target should be zero, so after enough iterations it should reach zero
        for _ in 0..100 {
            let output = envelope.apply(10.0, false).unwrap();
            // Each step should be moving toward zero or at zero
            assert!(
                output.abs() <= torque_before,
                "Torque should be decreasing or zero when safe_for_ffb=false"
            );
        }

        // After many iterations with safe_for_ffb=false, should be at or very close to zero
        let final_output = envelope.get_last_torque();
        assert!(
            final_output.abs() < 0.1,
            "Torque should reach near-zero when safe_for_ffb=false: {}",
            final_output
        );
    }

    /// **Test: 50ms Ramp-to-Zero on Fault**
    /// **Validates: Requirement FFB-SAFETY-01.6**
    ///
    /// Verifies that fault triggers 50ms ramp to zero with explicit timestamp tracking
    #[test]
    fn test_fault_ramp_to_zero_timing() {
        let config = SafetyEnvelopeConfig {
            max_torque_nm: 15.0,
            max_slew_rate_nm_per_s: 1000.0,
            max_jerk_nm_per_s2: 10000.0,
            fault_ramp_time: Duration::from_millis(50),
            timestep_s: 0.004,
        };

        let mut envelope = SafetyEnvelope::new(config).unwrap();

        // Build up to 10 Nm
        for _ in 0..100 {
            envelope.apply(10.0, true).unwrap();
        }

        let initial_torque = envelope.get_last_torque();
        assert!(
            initial_torque > 8.0,
            "Should have built up significant torque: {}",
            initial_torque
        );

        // Trigger fault
        envelope.trigger_fault_ramp();
        assert!(envelope.is_in_fault_ramp());

        let fault_start = std::time::Instant::now();

        // Simulate updates over 50ms
        let mut samples = Vec::new();
        while fault_start.elapsed() < Duration::from_millis(60) {
            let output = envelope.apply(10.0, true).unwrap(); // Desired torque ignored during fault
            samples.push((fault_start.elapsed(), output));
            thread::sleep(Duration::from_millis(4)); // Simulate 250Hz loop
        }

        // Verify ramp characteristics
        assert!(
            !samples.is_empty(),
            "Should have collected samples during ramp"
        );

        // Check that torque decreases monotonically
        for i in 1..samples.len() {
            assert!(
                samples[i].1 <= samples[i - 1].1 + 0.01, // Small tolerance for numerical errors
                "Torque should decrease monotonically during fault ramp: {} > {} at sample {}",
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
                torque.abs() < 0.5,
                "Torque should be near zero at 50ms: {}",
                torque
            );
        }

        // Verify final torque is zero
        let final_torque = samples.last().unwrap().1;
        assert!(
            final_torque.abs() < 0.1,
            "Final torque should be near zero: {}",
            final_torque
        );
    }

    /// **Test: Fault Ramp Progress Tracking**
    /// **Validates: Requirement FFB-SAFETY-01.6**
    ///
    /// Verifies explicit fault timestamp tracking and progress calculation
    #[test]
    fn test_fault_timestamp_tracking() {
        let config = SafetyEnvelopeConfig {
            fault_ramp_time: Duration::from_millis(50),
            ..Default::default()
        };

        let mut envelope = SafetyEnvelope::new(config).unwrap();

        // No fault initially
        assert!(!envelope.is_in_fault_ramp());
        assert!(envelope.get_fault_ramp_progress().is_none());
        assert!(envelope.get_fault_elapsed_time().is_none());

        // Trigger fault
        envelope.trigger_fault_ramp();
        assert!(envelope.is_in_fault_ramp());

        // Check progress immediately after trigger
        let progress = envelope.get_fault_ramp_progress().unwrap();
        assert!(
            (0.0..=0.1).contains(&progress),
            "Progress should be near 0 immediately after trigger: {}",
            progress
        );

        // Wait and check progress
        thread::sleep(Duration::from_millis(25));
        envelope.apply(10.0, true).unwrap(); // Update to recalculate progress

        let progress = envelope.get_fault_ramp_progress().unwrap();
        assert!(
            (0.4..=0.6).contains(&progress),
            "Progress should be around 0.5 after 25ms: {}",
            progress
        );

        let elapsed = envelope.get_fault_elapsed_time().unwrap();
        assert!(
            elapsed >= Duration::from_millis(25) && elapsed <= Duration::from_millis(30),
            "Elapsed time should be around 25ms: {:?}",
            elapsed
        );

        // Clear fault
        envelope.clear_fault();
        assert!(!envelope.is_in_fault_ramp());
        assert!(envelope.get_fault_ramp_progress().is_none());
    }

    /// **Test: Invalid Torque Handling**
    ///
    /// Verifies that NaN and Inf values are rejected
    #[test]
    fn test_invalid_torque_rejection() {
        let mut envelope = SafetyEnvelope::default();

        // Test NaN
        let result = envelope.apply(f32::NAN, true);
        assert!(
            matches!(result, Err(SafetyEnvelopeError::InvalidTorque { .. })),
            "NaN should be rejected"
        );

        // Test positive infinity
        let result = envelope.apply(f32::INFINITY, true);
        assert!(
            matches!(result, Err(SafetyEnvelopeError::InvalidTorque { .. })),
            "Positive infinity should be rejected"
        );

        // Test negative infinity
        let result = envelope.apply(f32::NEG_INFINITY, true);
        assert!(
            matches!(result, Err(SafetyEnvelopeError::InvalidTorque { .. })),
            "Negative infinity should be rejected"
        );
    }

    /// **Test: Configuration Validation**
    ///
    /// Verifies that invalid configurations are rejected
    #[test]
    fn test_invalid_configuration() {
        // Invalid max_torque_nm
        let config = SafetyEnvelopeConfig {
            max_torque_nm: -5.0,
            ..Default::default()
        };
        assert!(
            SafetyEnvelope::new(config).is_err(),
            "Negative max_torque_nm should be rejected"
        );

        // Invalid max_slew_rate
        let config = SafetyEnvelopeConfig {
            max_slew_rate_nm_per_s: 0.0,
            ..Default::default()
        };
        assert!(
            SafetyEnvelope::new(config).is_err(),
            "Zero max_slew_rate should be rejected"
        );

        // Invalid max_jerk
        let config = SafetyEnvelopeConfig {
            max_jerk_nm_per_s2: f32::NAN,
            ..Default::default()
        };
        assert!(
            SafetyEnvelope::new(config).is_err(),
            "NaN max_jerk should be rejected"
        );
    }

    /// **Test: Combined Safety Constraints**
    ///
    /// Verifies that all safety constraints work together correctly
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
                "Torque should be clamped: {}",
                output
            );

            // Verify slew rate limiting
            let delta = output - last_torque;
            let slew_rate = delta / timestep;
            assert!(
                slew_rate.abs() <= max_slew_rate + 0.1,
                "Slew rate should be limited: {}",
                slew_rate
            );

            // Verify jerk limiting (skip first iteration where we're starting from zero)
            // Also skip when we're at the torque limit, as hitting a hard limit causes
            // the slew rate to drop to zero, which creates unavoidable jerk
            if i > 0 && output.abs() < max_torque - 0.01 {
                let jerk = (slew_rate - last_slew_rate) / timestep;
                assert!(
                    jerk.abs() <= max_jerk + 1.0,
                    "Jerk should be limited: {} at iteration {}",
                    jerk,
                    i
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

    /// **Test: Fault Overrides safe_for_ffb**
    ///
    /// Verifies that fault ramp takes precedence over safe_for_ffb flag
    #[test]
    fn test_fault_overrides_safe_for_ffb() {
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

        // Trigger fault
        envelope.trigger_fault_ramp();

        // Even with safe_for_ffb=true, should ramp to zero
        thread::sleep(Duration::from_millis(60));
        for _ in 0..20 {
            envelope.apply(10.0, true).unwrap(); // safe_for_ffb=true, but fault active
        }

        let final_torque = envelope.get_last_torque();
        assert!(
            final_torque.abs() < 0.1,
            "Fault should override safe_for_ffb and ramp to zero: {}",
            final_torque
        );
    }

    /// **Test: State Reset**
    ///
    /// Verifies that reset clears all state correctly
    #[test]
    fn test_state_reset() {
        let mut envelope = SafetyEnvelope::default();

        // Build up state
        for _ in 0..50 {
            envelope.apply(10.0, true).unwrap();
        }

        envelope.trigger_fault_ramp();

        assert!(envelope.get_last_torque() != 0.0);
        assert!(envelope.is_in_fault_ramp());

        // Reset
        envelope.reset();

        assert_eq!(envelope.get_last_torque(), 0.0);
        assert_eq!(envelope.get_last_slew_rate(), 0.0);
        assert!(!envelope.is_in_fault_ramp());
        assert!(envelope.get_fault_ramp_progress().is_none());
    }

    /// **Test: Configuration Update**
    ///
    /// Verifies that configuration can be updated while preserving state
    #[test]
    fn test_configuration_update() {
        let mut envelope = SafetyEnvelope::default();

        // Build up some state
        for _ in 0..50 {
            envelope.apply(5.0, true).unwrap();
        }

        let torque_before = envelope.get_last_torque();

        // Update configuration
        let new_config = SafetyEnvelopeConfig {
            max_torque_nm: 20.0,
            max_slew_rate_nm_per_s: 100.0,
            ..Default::default()
        };

        envelope.update_config(new_config).unwrap();

        // State should be preserved
        assert_eq!(envelope.get_last_torque(), torque_before);

        // New limits should be applied
        assert_eq!(envelope.get_config().max_torque_nm, 20.0);
        assert_eq!(envelope.get_config().max_slew_rate_nm_per_s, 100.0);
    }
}
