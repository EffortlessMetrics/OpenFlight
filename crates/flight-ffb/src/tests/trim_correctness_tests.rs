// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Comprehensive tests for trim correctness validation
//!
//! These tests verify the complete implementation of task 27:
//! - Non-FFB recentre illusion with trim-hold freeze
//! - FFB setpoint change with rate/jerk limiting
//! - HIL tests for trim behavior validation
//! - Replay reproducibility with comprehensive testing

use std::time::{Duration, Instant};
use crate::{
    FfbEngine, FfbConfig, FfbMode, TrimController, TrimMode, SetpointChange, TrimLimits,
    TrimOutput, TrimValidationSuite, TrimValidationConfig, HilTrimTestSuite, HilTrimTestConfig,
    SpringConfig, DeviceCapabilities
};

/// Test the complete trim correctness validation implementation
#[test]
fn test_complete_trim_correctness_validation() {
    // Test 1: Non-FFB recentre illusion with trim-hold freeze
    test_non_ffb_recentre_illusion();
    
    // Test 2: FFB setpoint change with rate/jerk limiting
    test_ffb_setpoint_rate_jerk_limiting();
    
    // Test 3: HIL tests for trim behavior validation
    test_hil_trim_behavior_validation();
    
    // Test 4: Replay reproducibility
    test_replay_reproducibility();
}

/// Test non-FFB recentre illusion with trim-hold freeze
fn test_non_ffb_recentre_illusion() {
    let mut controller = TrimController::new(15.0);
    controller.set_mode(TrimMode::SpringCentered);

    // Set initial spring configuration
    let initial_config = SpringConfig {
        strength: 0.8,
        center: 0.0,
        deadband: 0.05,
    };
    controller.set_spring_config(initial_config.clone());

    // Apply trim change (recentre illusion)
    let change = SetpointChange {
        target_nm: 7.5, // Should map to 0.5 center position
        limits: TrimLimits::default(),
    };

    controller.apply_setpoint_change(change).unwrap();

    // Phase 1: Verify immediate spring freeze
    let output = controller.update();
    match output {
        TrimOutput::SpringCentered { frozen, config } => {
            assert!(frozen, "Spring should be frozen immediately after setpoint change");
            
            // Center should be updated to new position
            let expected_center = 7.5 / 15.0; // 0.5
            assert!((config.center - expected_center).abs() < 1e-6, 
                "Center should be updated: expected {}, got {}", expected_center, config.center);
        }
        _ => panic!("Expected SpringCentered output"),
    }

    // Phase 2: Wait for ramp to start
    std::thread::sleep(Duration::from_millis(150));

    // Phase 3: Verify gradual spring re-enable (ramp)
    let mut ramp_observed = false;
    for _ in 0..100 {
        let output = controller.update();
        if let TrimOutput::SpringCentered { frozen, config } = output {
            if !frozen && config.strength < initial_config.strength {
                ramp_observed = true;
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(10));
    }

    assert!(ramp_observed, "Spring ramp should be observed during recentre illusion");

    println!("✅ Non-FFB recentre illusion test passed");
}

/// Test FFB setpoint change with rate/jerk limiting
fn test_ffb_setpoint_rate_jerk_limiting() {
    let mut controller = TrimController::new(15.0);
    controller.set_mode(TrimMode::ForceFeedback);

    let limits = TrimLimits {
        max_rate_nm_per_s: 5.0,
        max_jerk_nm_per_s2: 20.0,
    };

    let change = SetpointChange {
        target_nm: 10.0,
        limits: limits.clone(),
    };

    controller.apply_setpoint_change(change).unwrap();

    let mut max_rate = 0.0f32;
    let mut max_jerk = 0.0f32;
    let mut previous_rate = 0.0f32;
    let dt = 0.001f32; // 1ms timestep

    // Run for several seconds to test rate and jerk limiting
    for _ in 0..3000 {
        let output = controller.update();
        
        if let TrimOutput::ForceFeedback { rate_nm_per_s, .. } = output {
            max_rate = max_rate.max(rate_nm_per_s.abs());
            
            // Calculate jerk
            let jerk = (rate_nm_per_s - previous_rate).abs() / dt;
            max_jerk = max_jerk.max(jerk);
            
            // Verify rate limit compliance
            assert!(rate_nm_per_s.abs() <= limits.max_rate_nm_per_s + 1e-6,
                "Rate limit exceeded: {} > {} Nm/s", rate_nm_per_s.abs(), limits.max_rate_nm_per_s);
            
            // Verify jerk limit compliance (with tolerance for discrete sampling)
            assert!(jerk <= limits.max_jerk_nm_per_s2 + 1e-3,
                "Jerk limit exceeded: {} > {} Nm/s²", jerk, limits.max_jerk_nm_per_s2);
            
            previous_rate = rate_nm_per_s;
        }
        
        std::thread::sleep(Duration::from_millis(1));
    }

    // Verify we actually used the available rate (should be close to limit)
    assert!(max_rate > limits.max_rate_nm_per_s * 0.8,
        "Rate limit underutilized: {} < 80% of {}", max_rate, limits.max_rate_nm_per_s);

    println!("✅ FFB setpoint rate/jerk limiting test passed");
}

/// Test HIL trim behavior validation
fn test_hil_trim_behavior_validation() {
    let config = HilTrimTestConfig {
        device_max_torque_nm: 15.0,
        max_test_duration: Duration::from_secs(10), // Shorter for unit test
        hil_fp_tolerance: 1e-4,
        use_physical_device: false, // Virtual device for unit test
        hil_sample_rate_hz: 250,
    };

    let mut hil_suite = HilTrimTestSuite::new(config);
    
    // Run a subset of HIL tests for unit testing
    let ffb_rate_result = hil_suite.test_hil_ffb_rate_limiting();
    assert!(ffb_rate_result.validation_result.passed, 
        "HIL FFB rate limiting failed: {:?}", ffb_rate_result.validation_result.error);

    let spring_freeze_result = hil_suite.test_hil_spring_freeze_timing();
    assert!(spring_freeze_result.validation_result.passed,
        "HIL spring freeze timing failed: {:?}", spring_freeze_result.validation_result.error);

    println!("✅ HIL trim behavior validation test passed");
}

/// Test replay reproducibility
fn test_replay_reproducibility() {
    let config = TrimValidationConfig {
        fp_tolerance: 1e-6,
        max_test_duration: Duration::from_secs(5),
        sample_rate_hz: 1000,
        verbose_logging: false,
    };

    let mut validation_suite = TrimValidationSuite::new(config);
    
    // Run replay reproducibility test
    let result = validation_suite.test_replay_reproducibility();
    assert!(result.passed, "Replay reproducibility test failed: {:?}", result.error);

    // Verify measurements show good reproducibility
    assert!(!result.measurements.is_empty(), "Should have reproducibility measurements");
    
    // All differences should be very small
    let max_difference = result.measurements.iter().fold(0.0f32, |a, &b| a.max(b));
    assert!(max_difference < 1e-3, "Reproducibility error too large: {}", max_difference);

    println!("✅ Replay reproducibility test passed");
}

/// Test complete validation suite integration
#[test]
fn test_validation_suite_integration() {
    let mut validation_suite = TrimValidationSuite::default();
    let results = validation_suite.run_complete_validation();
    
    assert!(!results.is_empty(), "Validation suite should produce results");
    
    // Check that all major test categories are covered
    let ffb_tests = results.iter().filter(|r| r.name.contains("FFB")).count();
    let spring_tests = results.iter().filter(|r| r.name.contains("Spring")).count();
    let replay_tests = results.iter().filter(|r| r.name.contains("Replay")).count();
    
    assert!(ffb_tests >= 3, "Should have multiple FFB tests");
    assert!(spring_tests >= 3, "Should have multiple spring tests");
    assert!(replay_tests >= 1, "Should have replay test");
    
    // Most tests should pass
    let passed_count = results.iter().filter(|r| r.passed).count();
    let pass_rate = passed_count as f32 / results.len() as f32;
    assert!(pass_rate >= 0.9, "Pass rate should be high: {:.1}%", pass_rate * 100.0);
    
    println!("✅ Validation suite integration test passed");
}

/// Test FFB engine integration with trim validation
#[test]
fn test_ffb_engine_trim_integration() {
    let config = FfbConfig {
        max_torque_nm: 15.0,
        fault_timeout_ms: 50,
        interlock_required: false,
        mode: FfbMode::Auto,
        device_path: None,
    };

    let mut engine = FfbEngine::new(config).unwrap();
    
    // Set device capabilities
    let capabilities = DeviceCapabilities {
        supports_pid: true,
        supports_raw_torque: true,
        max_torque_nm: 15.0,
        min_period_us: 1000,
        has_health_stream: true,
        supports_interlock: true,
    };
    
    engine.set_device_capabilities(capabilities).unwrap();

    // Test trim setpoint change through engine
    let change = SetpointChange {
        target_nm: 8.0,
        limits: TrimLimits::default(),
    };

    engine.apply_trim_setpoint_change(change).unwrap();

    // Test trim controller update through engine
    let output = engine.update_trim_controller();
    match output {
        TrimOutput::ForceFeedback { setpoint_nm, .. } => {
            assert!(setpoint_nm.is_finite(), "Setpoint should be finite");
        }
        _ => panic!("Expected ForceFeedback output for auto-negotiated mode"),
    }

    // Test validation through engine
    let validation_results = engine.run_trim_validation();
    assert!(!validation_results.is_empty(), "Engine should produce validation results");

    println!("✅ FFB engine trim integration test passed");
}

/// Test trim state diagnostics
#[test]
fn test_trim_state_diagnostics() {
    let mut controller = TrimController::new(15.0);
    controller.set_mode(TrimMode::ForceFeedback);

    // Get initial state
    let initial_state = controller.get_trim_state();
    assert_eq!(initial_state.mode, TrimMode::ForceFeedback);
    assert_eq!(initial_state.current_setpoint_nm, 0.0);
    assert!(!initial_state.is_changing);

    // Apply setpoint change
    let change = SetpointChange {
        target_nm: 5.0,
        limits: TrimLimits::default(),
    };

    controller.apply_setpoint_change(change).unwrap();

    // Get state after change
    let changing_state = controller.get_trim_state();
    assert_eq!(changing_state.target_setpoint_nm, 5.0);
    assert!(changing_state.is_changing);
    assert!(changing_state.estimated_completion.is_some());

    // Test spring mode state
    controller.set_mode(TrimMode::SpringCentered);
    let spring_change = SetpointChange {
        target_nm: 3.0,
        limits: TrimLimits::default(),
    };

    controller.apply_setpoint_change(spring_change).unwrap();
    controller.update(); // Trigger freeze

    let spring_state = controller.get_trim_state();
    assert_eq!(spring_state.mode, TrimMode::SpringCentered);
    assert!(spring_state.spring_frozen);
    assert!(!spring_state.spring_ramping);

    println!("✅ Trim state diagnostics test passed");
}

/// Test torque step validation
#[test]
fn test_torque_step_validation() {
    let controller = TrimController::new(15.0);
    
    // Test valid torque change (within rate limit)
    let result = controller.validate_no_torque_steps(0.0, 0.005, 0.001); // 5 Nm/s rate
    assert!(result.is_ok(), "Valid torque change should pass");

    // Test invalid torque change (exceeds rate limit)
    let result = controller.validate_no_torque_steps(0.0, 0.01, 0.001); // 10 Nm/s rate (exceeds default 5 Nm/s)
    assert!(result.is_err(), "Invalid torque change should fail");

    // Test edge case with zero dt
    let result = controller.validate_no_torque_steps(0.0, 1.0, 0.0);
    assert!(result.is_ok(), "Zero dt should be handled gracefully");

    println!("✅ Torque step validation test passed");
}

/// Test trim limits validation
#[test]
fn test_trim_limits_validation() {
    // Valid limits
    let valid_limits = TrimLimits {
        max_rate_nm_per_s: 5.0,
        max_jerk_nm_per_s2: 20.0,
    };
    assert!(valid_limits.validate().is_ok(), "Valid limits should pass validation");

    // Invalid rate (negative)
    let invalid_rate = TrimLimits {
        max_rate_nm_per_s: -1.0,
        max_jerk_nm_per_s2: 20.0,
    };
    assert!(invalid_rate.validate().is_err(), "Negative rate should fail validation");

    // Invalid jerk (negative)
    let invalid_jerk = TrimLimits {
        max_rate_nm_per_s: 5.0,
        max_jerk_nm_per_s2: -1.0,
    };
    assert!(invalid_jerk.validate().is_err(), "Negative jerk should fail validation");

    // Inconsistent limits (jerk < rate)
    let inconsistent_limits = TrimLimits {
        max_rate_nm_per_s: 10.0,
        max_jerk_nm_per_s2: 5.0,
    };
    assert!(inconsistent_limits.validate().is_err(), "Inconsistent limits should fail validation");

    println!("✅ Trim limits validation test passed");
}

/// Test spring ramp progress tracking
#[test]
fn test_spring_ramp_progress() {
    let mut controller = TrimController::new(15.0);
    controller.set_mode(TrimMode::SpringCentered);

    // Initially no ramp
    assert!(!controller.is_spring_ramping());
    assert!(controller.get_spring_ramp_progress().is_none());

    // Apply setpoint change to trigger freeze
    let change = SetpointChange {
        target_nm: 5.0,
        limits: TrimLimits::default(),
    };

    controller.apply_setpoint_change(change).unwrap();
    controller.update(); // Trigger freeze

    // Wait for ramp to start
    std::thread::sleep(Duration::from_millis(150));

    // Check for ramp progress
    let mut ramp_detected = false;
    for _ in 0..50 {
        controller.update();
        
        if controller.is_spring_ramping() {
            ramp_detected = true;
            let progress = controller.get_spring_ramp_progress();
            assert!(progress.is_some(), "Should have ramp progress");
            
            let progress_value = progress.unwrap();
            assert!(progress_value >= 0.0 && progress_value <= 1.0, 
                "Progress should be between 0 and 1: {}", progress_value);
            break;
        }
        
        std::thread::sleep(Duration::from_millis(10));
    }

    assert!(ramp_detected, "Spring ramp should be detected");

    println!("✅ Spring ramp progress test passed");
}

/// Integration test for all trim correctness requirements
#[test]
fn test_trim_correctness_requirements_compliance() {
    println!("🧪 Running comprehensive trim correctness validation...");
    
    // Requirement: Non-FFB recentre illusion with trim-hold freeze
    test_non_ffb_recentre_illusion();
    
    // Requirement: FFB setpoint change with rate/jerk limiting
    test_ffb_setpoint_rate_jerk_limiting();
    
    // Requirement: HIL tests for trim behavior validation
    test_hil_trim_behavior_validation();
    
    // Requirement: Replay reproducibility with comprehensive testing
    test_replay_reproducibility();
    
    println!("✅ All trim correctness requirements validated successfully!");
}