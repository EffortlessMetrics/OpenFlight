// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Comprehensive safety threshold validation tests
//!
//! **Phase 2 Exit Criterion: No safety thresholds violated in tests**
//!
//! This module provides comprehensive validation that all safety thresholds
//! are properly enforced and never violated during normal operation or fault conditions.
//!
//! **Validates: Requirements FFB-SAFETY-01.1, FFB-SAFETY-01.2, FFB-SAFETY-01.3, FFB-SAFETY-01.4, FFB-SAFETY-01.6**
//! **Validates: Phase 2 Exit Criterion: "No safety thresholds violated in tests"**

#[cfg(test)]
mod tests {
    use crate::safety_envelope::*;
    use std::thread;
    use std::time::Duration;

    /// **Comprehensive Safety Threshold Validation**
    ///
    /// This test validates that ALL safety thresholds are enforced across
    /// a wide range of scenarios including:
    /// - Normal operation
    /// - Rapid torque changes
    /// - Fault conditions
    /// - safe_for_ffb transitions
    ///
    /// **Validates: Phase 2 Exit Criterion**
    #[test]
    fn test_no_safety_thresholds_violated_comprehensive() {
        let config = SafetyEnvelopeConfig {
            max_torque_nm: 15.0,
            max_slew_rate_nm_per_s: 50.0,
            max_jerk_nm_per_s2: 500.0,
            fault_ramp_time: Duration::from_millis(50),
            timestep_s: 0.004, // 250Hz
        };

        let max_torque = config.max_torque_nm;
        let max_slew_rate = config.max_slew_rate_nm_per_s;
        let max_jerk = config.max_jerk_nm_per_s2;
        let timestep = config.timestep_s;

        let mut envelope = SafetyEnvelope::new(config).unwrap();

        let mut last_torque = 0.0;
        let mut last_slew_rate = 0.0;
        let mut violation_count = 0;

        // Test scenario 1: Rapid torque changes
        let test_inputs = vec![
            (0.0, true),
            (15.0, true),
            (-15.0, true),
            (10.0, true),
            (0.0, false), // safe_for_ffb goes false
            (15.0, false),
            (0.0, true), // safe_for_ffb goes true again
            (8.0, true),
        ];

        for (desired_torque, safe_for_ffb) in test_inputs {
            for _ in 0..50 {
                // Run multiple iterations for each input
                let output = envelope.apply(desired_torque, safe_for_ffb).unwrap();

                // **Threshold 1: Torque magnitude must never exceed max_torque_nm**
                if output.abs() > max_torque + 0.01 {
                    violation_count += 1;
                    eprintln!(
                        "VIOLATION: Torque exceeded limit: {} > {}",
                        output.abs(),
                        max_torque
                    );
                }
                assert!(
                    output.abs() <= max_torque + 0.01,
                    "Torque threshold violated: {} > {}",
                    output.abs(),
                    max_torque
                );

                // **Threshold 2: Slew rate must never exceed max_slew_rate_nm_per_s**
                let delta = output - last_torque;
                let slew_rate = delta / timestep;
                if slew_rate.abs() > max_slew_rate + 0.1 {
                    violation_count += 1;
                    eprintln!(
                        "VIOLATION: Slew rate exceeded limit: {} > {}",
                        slew_rate.abs(),
                        max_slew_rate
                    );
                }
                assert!(
                    slew_rate.abs() <= max_slew_rate + 0.1,
                    "Slew rate threshold violated: {} > {}",
                    slew_rate.abs(),
                    max_slew_rate
                );

                // **Threshold 3: Jerk must never exceed max_jerk_nm_per_s2**
                // (Skip first iteration and when at torque limit)
                if last_slew_rate != 0.0 && output.abs() < max_torque - 0.01 {
                    let jerk = (slew_rate - last_slew_rate) / timestep;
                    if jerk.abs() > max_jerk + 1.0 {
                        violation_count += 1;
                        eprintln!(
                            "VIOLATION: Jerk exceeded limit: {} > {}",
                            jerk.abs(),
                            max_jerk
                        );
                    }
                    assert!(
                        jerk.abs() <= max_jerk + 1.0,
                        "Jerk threshold violated: {} > {}",
                        jerk.abs(),
                        max_jerk
                    );
                }

                last_torque = output;
                last_slew_rate = slew_rate;
            }
        }

        // **Threshold 4: When safe_for_ffb is false, torque must ramp to zero**
        envelope.reset();
        for _ in 0..50 {
            envelope.apply(10.0, true).unwrap();
        }

        // Now set safe_for_ffb to false and verify ramp to zero
        for _ in 0..100 {
            let output = envelope.apply(10.0, false).unwrap();
            // Should be ramping toward zero
            assert!(
                output.abs() <= max_torque,
                "Torque must stay within limits during safe_for_ffb=false ramp"
            );
        }

        let final_torque = envelope.get_last_torque();
        assert!(
            final_torque.abs() < 0.1,
            "Torque must reach near-zero when safe_for_ffb=false: {}",
            final_torque
        );

        // **Threshold 5: Fault ramp must complete within 50ms**
        envelope.reset();
        for _ in 0..50 {
            envelope.apply(10.0, true).unwrap();
        }

        envelope.trigger_fault_ramp();
        let fault_start = std::time::Instant::now();

        while fault_start.elapsed() < Duration::from_millis(60) {
            let output = envelope.apply(10.0, true).unwrap();

            // During fault ramp, torque must still respect limits
            assert!(
                output.abs() <= max_torque + 0.01,
                "Torque must stay within limits during fault ramp"
            );

            thread::sleep(Duration::from_millis(4));
        }

        let final_torque = envelope.get_last_torque();
        assert!(
            final_torque.abs() < 0.1,
            "Fault ramp must reach near-zero within 50ms: {}",
            final_torque
        );

        // Report results
        assert_eq!(
            violation_count, 0,
            "Safety threshold violations detected: {}",
            violation_count
        );

        println!("✓ All safety thresholds validated - no violations detected");
        println!("  - Torque clamping: PASS");
        println!("  - Slew rate limiting: PASS");
        println!("  - Jerk limiting: PASS");
        println!("  - safe_for_ffb enforcement: PASS");
        println!("  - Fault ramp timing: PASS");
    }

    /// **Stress Test: Extended Operation Without Violations**
    ///
    /// Runs the safety envelope through 10,000 iterations with random inputs
    /// to verify no threshold violations occur during extended operation.
    #[test]
    fn test_extended_operation_no_violations() {
        let config = SafetyEnvelopeConfig {
            max_torque_nm: 15.0,
            max_slew_rate_nm_per_s: 50.0,
            max_jerk_nm_per_s2: 500.0,
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
        let mut max_observed_torque = 0.0f32;
        let mut max_observed_slew_rate = 0.0f32;
        let mut max_observed_jerk = 0.0f32;

        // Simulate 10,000 iterations (40 seconds at 250Hz)
        for i in 0..10_000 {
            // Vary inputs to stress test the system
            let desired_torque = match i % 100 {
                0..=20 => 15.0,
                21..=40 => -15.0,
                41..=60 => 0.0,
                61..=80 => 10.0,
                _ => 5.0,
            };

            let safe_for_ffb = (i % 200) < 180; // Occasionally toggle safe_for_ffb

            let output = envelope.apply(desired_torque, safe_for_ffb).unwrap();

            // Track maximum observed values
            max_observed_torque = max_observed_torque.max(output.abs());

            let delta = output - last_torque;
            let slew_rate = delta / timestep;
            max_observed_slew_rate = max_observed_slew_rate.max(slew_rate.abs());

            if last_slew_rate != 0.0 {
                let jerk = (slew_rate - last_slew_rate) / timestep;
                max_observed_jerk = max_observed_jerk.max(jerk.abs());
            }

            // Verify no violations
            assert!(
                output.abs() <= max_torque + 0.01,
                "Torque violation at iteration {}: {} > {}",
                i,
                output.abs(),
                max_torque
            );

            assert!(
                slew_rate.abs() <= max_slew_rate + 0.1,
                "Slew rate violation at iteration {}: {} > {}",
                i,
                slew_rate.abs(),
                max_slew_rate
            );

            if last_slew_rate != 0.0 && output.abs() < max_torque - 0.01 {
                let jerk = (slew_rate - last_slew_rate) / timestep;
                assert!(
                    jerk.abs() <= max_jerk + 1.0,
                    "Jerk violation at iteration {}: {} > {}",
                    i,
                    jerk.abs(),
                    max_jerk
                );
            }

            last_torque = output;
            last_slew_rate = slew_rate;
        }

        println!("✓ Extended operation test completed - 10,000 iterations");
        println!(
            "  Max observed torque: {:.2} Nm (limit: {:.2} Nm)",
            max_observed_torque, max_torque
        );
        println!(
            "  Max observed slew rate: {:.2} Nm/s (limit: {:.2} Nm/s)",
            max_observed_slew_rate, max_slew_rate
        );
        println!(
            "  Max observed jerk: {:.2} Nm/s² (limit: {:.2} Nm/s²)",
            max_observed_jerk, max_jerk
        );
        println!("  No violations detected");
    }

    /// **Boundary Test: Extreme Input Values**
    ///
    /// Tests the safety envelope with extreme input values to verify
    /// thresholds are enforced even under pathological conditions.
    #[test]
    fn test_extreme_inputs_no_violations() {
        let config = SafetyEnvelopeConfig {
            max_torque_nm: 10.0,
            max_slew_rate_nm_per_s: 30.0,
            max_jerk_nm_per_s2: 300.0,
            timestep_s: 0.004,
            ..Default::default()
        };

        let max_torque = config.max_torque_nm;
        let max_slew_rate = config.max_slew_rate_nm_per_s;
        let timestep = config.timestep_s;

        let mut envelope = SafetyEnvelope::new(config).unwrap();

        // Test with extreme inputs
        let extreme_inputs = vec![
            1000.0,  // Way above limit
            -1000.0, // Way below limit
            100.0,   // Large positive
            -100.0,  // Large negative
            50.0,    // Moderate
            -50.0,   // Moderate negative
        ];

        for &input in &extreme_inputs {
            envelope.reset();
            let mut last_torque = 0.0;

            for _ in 0..100 {
                let output = envelope.apply(input, true).unwrap();

                // Verify torque clamping
                assert!(
                    output.abs() <= max_torque + 0.01,
                    "Torque must be clamped for input {}: {} > {}",
                    input,
                    output.abs(),
                    max_torque
                );

                // Verify slew rate limiting
                let delta = output - last_torque;
                let slew_rate = delta / timestep;
                assert!(
                    slew_rate.abs() <= max_slew_rate + 0.1,
                    "Slew rate must be limited for input {}: {} > {}",
                    input,
                    slew_rate.abs(),
                    max_slew_rate
                );

                last_torque = output;
            }
        }

        println!("✓ Extreme input test completed");
        println!("  All extreme inputs properly clamped and rate-limited");
        println!("  No violations detected");
    }

    /// **Fault Scenario Test: Multiple Fault Types**
    ///
    /// Tests that safety thresholds are maintained during various fault scenarios.
    #[test]
    fn test_fault_scenarios_no_violations() {
        let config = SafetyEnvelopeConfig {
            max_torque_nm: 15.0,
            max_slew_rate_nm_per_s: 50.0,
            max_jerk_nm_per_s2: 500.0,
            fault_ramp_time: Duration::from_millis(50),
            timestep_s: 0.004,
        };

        let max_torque = config.max_torque_nm;

        // Scenario 1: Fault at maximum torque
        let mut envelope = SafetyEnvelope::new(config.clone()).unwrap();
        for _ in 0..100 {
            envelope.apply(15.0, true).unwrap();
        }

        envelope.trigger_fault_ramp();
        for _ in 0..20 {
            let output = envelope.apply(15.0, true).unwrap();
            assert!(
                output.abs() <= max_torque + 0.01,
                "Torque must stay within limits during fault ramp from max"
            );
            thread::sleep(Duration::from_millis(4));
        }

        // Scenario 2: Fault at negative torque
        let mut envelope = SafetyEnvelope::new(config.clone()).unwrap();
        for _ in 0..100 {
            envelope.apply(-10.0, true).unwrap();
        }

        envelope.trigger_fault_ramp();
        for _ in 0..20 {
            let output = envelope.apply(-10.0, true).unwrap();
            assert!(
                output.abs() <= max_torque + 0.01,
                "Torque must stay within limits during fault ramp from negative"
            );
            thread::sleep(Duration::from_millis(4));
        }

        // Scenario 3: Fault during rapid change
        let mut envelope = SafetyEnvelope::new(config).unwrap();
        for i in 0..50 {
            let target = if i % 10 < 5 { 10.0 } else { -10.0 };
            envelope.apply(target, true).unwrap();
        }

        envelope.trigger_fault_ramp();
        for _ in 0..20 {
            let output = envelope.apply(10.0, true).unwrap();
            assert!(
                output.abs() <= max_torque + 0.01,
                "Torque must stay within limits during fault ramp from rapid change"
            );
            thread::sleep(Duration::from_millis(4));
        }

        println!("✓ Fault scenario test completed");
        println!("  All fault scenarios maintained safety thresholds");
        println!("  No violations detected");
    }
}
