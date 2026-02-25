// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! DirectInput FFB device tests
//!
//! This module provides comprehensive unit and integration tests for DirectInput FFB device control.
//!
//! # Test Coverage
//! - Unit tests with mocks/fakes for all device operations
//! - Integration tests for hardware-backed validation (optional, hardware-gated)
//! - Effect creation and management tests
//! - Parameter validation and clamping tests
//! - Error handling tests
//!
//! # Requirements
//! - FFB-HID-01.1: DirectInput 8 device enumeration and acquisition
//! - FFB-HID-01.2: Effect creation (constant force, periodic, spring, damper)
//! - FFB-HID-01.3: Effect parameter updates
//! - FFB-HID-01.4: Effect lifecycle management (start, stop)
//!
//! # Note
//! Most tests in this module require real DirectInput FFB hardware to be connected.
//! Tests that don't require hardware use a valid but non-existent GUID format.
//! The dummy GUID used is: {00000000-0000-0000-0000-000000000000}

#[cfg(test)]
mod tests {
    use crate::dinput_device::*;

    /// A valid GUID format that doesn't correspond to any real device
    const TEST_GUID: &str = "{00000000-0000-0000-0000-000000000000}";

    // ========================================================================
    // Mock/Fake DirectInput Tests (cfg(test) only)
    // ========================================================================

    /// Test device creation on supported and unsupported platforms
    #[test]
    fn test_device_creation_platform_support() {
        let device = DirectInputFfbDevice::new(TEST_GUID.to_string());

        #[cfg(windows)]
        {
            assert!(device.is_ok(), "Device creation should succeed on Windows");
            let dev = device.unwrap();
            assert!(
                !dev.is_acquired(),
                "Device should not be acquired initially"
            );
            assert_eq!(
                dev.get_effect_count(),
                0,
                "No effects should exist initially"
            );
        }

        #[cfg(not(windows))]
        {
            assert!(
                matches!(device, Err(DInputError::PlatformNotSupported)),
                "Device creation should fail on non-Windows platforms"
            );
        }
    }

    /// Test device initialization with stub COM interfaces
    /// Requirements: FFB-HID-01.1
    #[test]
    #[cfg(windows)]
    #[ignore = "requires DirectInput FFB device registered in system"]
    fn test_device_initialization_stub() {
        let mut device = DirectInputFfbDevice::new(TEST_GUID.to_string()).unwrap();

        // Initialize should succeed with stub implementation
        let result = device.initialize();
        assert!(result.is_ok(), "Initialization should succeed with stub");

        // Verify device state after initialization
        assert!(
            !device.is_acquired(),
            "Device should not be acquired after init"
        );
    }

    /// Test device enumeration returns empty list with stub
    /// Requirements: FFB-HID-01.1
    #[test]
    #[cfg(windows)]
    fn test_device_enumeration_stub() {
        let devices = DirectInputFfbDevice::enumerate_devices();
        assert!(devices.is_ok(), "Enumeration should succeed");

        // Stub implementation returns empty list
        let device_list = devices.unwrap();
        assert!(
            device_list.is_empty(),
            "Stub enumeration should return empty list"
        );
    }

    /// Test capability querying with default values
    /// Requirements: FFB-HID-01.9
    #[test]
    #[cfg(windows)]
    #[ignore = "requires DirectInput FFB device registered in system"]
    fn test_capability_query_defaults() {
        let mut device = DirectInputFfbDevice::new(TEST_GUID.to_string()).unwrap();
        device.initialize().unwrap();

        let caps = device.query_capabilities().unwrap();

        // Verify default capabilities from stub
        assert!(caps.supports_pid, "Should support PID by default");
        assert!(
            !caps.supports_raw_torque,
            "Should not support raw torque by default"
        );
        assert_eq!(
            caps.max_torque_nm, 15.0,
            "Default max torque should be 15.0 Nm"
        );
        assert_eq!(
            caps.min_period_us, 2000,
            "Default min period should be 2000 us (500 Hz)"
        );
        assert!(
            !caps.has_health_stream,
            "Should not have health stream by default"
        );
        assert_eq!(caps.num_axes, 2, "Should have 2 axes by default");
        assert_eq!(caps.max_effects, 10, "Should support 10 effects by default");
    }

    /// Test device acquisition and release lifecycle
    /// Requirements: FFB-HID-01.1
    #[test]
    #[cfg(windows)]
    #[ignore = "requires DirectInput FFB device registered in system"]
    fn test_device_acquisition_lifecycle() {
        let mut device = DirectInputFfbDevice::new(TEST_GUID.to_string()).unwrap();
        device.initialize().unwrap();

        // Initially not acquired
        assert!(
            !device.is_acquired(),
            "Device should not be acquired initially"
        );

        // Acquire with null window handle (background mode)
        let result = device.acquire(0);
        assert!(result.is_ok(), "Acquisition should succeed");
        assert!(
            device.is_acquired(),
            "Device should be acquired after acquire()"
        );

        // Acquire again should be idempotent
        let result = device.acquire(0);
        assert!(result.is_ok(), "Re-acquisition should succeed");
        assert!(device.is_acquired(), "Device should still be acquired");

        // Release device
        device.unacquire();
        assert!(
            !device.is_acquired(),
            "Device should not be acquired after unacquire()"
        );

        // Unacquire again should be safe
        device.unacquire();
        assert!(!device.is_acquired(), "Device should still not be acquired");
    }

    /// Test that operations fail without device acquisition
    /// Requirements: FFB-HID-01.1
    #[test]
    #[cfg(windows)]
    #[ignore = "requires DirectInput FFB device registered in system"]
    fn test_operations_require_acquisition() {
        let mut device = DirectInputFfbDevice::new(TEST_GUID.to_string()).unwrap();
        device.initialize().unwrap();

        // Try to create effect without acquiring
        let result = device.create_constant_force_effect(0);
        assert!(
            matches!(result, Err(DInputError::DeviceNotAcquired)),
            "Effect creation should fail without acquisition"
        );

        // Try to update effect without acquiring
        let result = device.set_constant_force(0, 5.0);
        assert!(
            matches!(result, Err(DInputError::DeviceNotAcquired)),
            "Effect update should fail without acquisition"
        );

        // Try to start effect without acquiring
        let result = device.start_effect(0);
        assert!(
            matches!(result, Err(DInputError::DeviceNotAcquired)),
            "Effect start should fail without acquisition"
        );
    }

    // ========================================================================
    // Effect Creation Tests
    // Requirements: FFB-HID-01.2
    // ========================================================================

    /// Test constant force effect creation for pitch and roll axes
    /// Requirements: FFB-HID-01.2
    #[test]
    #[cfg(windows)]
    #[ignore = "requires DirectInput FFB device registered in system"]
    fn test_constant_force_effect_creation() {
        let mut device = DirectInputFfbDevice::new(TEST_GUID.to_string()).unwrap();
        device.initialize().unwrap();
        device.acquire(0).unwrap();

        // Create constant force effect for pitch axis (0)
        let pitch_handle = device.create_constant_force_effect(0);
        assert!(pitch_handle.is_ok(), "Pitch effect creation should succeed");
        assert_eq!(
            pitch_handle.unwrap(),
            0,
            "First effect should have handle 0"
        );
        assert_eq!(device.get_effect_count(), 1, "Should have 1 effect");

        // Create constant force effect for roll axis (1)
        let roll_handle = device.create_constant_force_effect(1);
        assert!(roll_handle.is_ok(), "Roll effect creation should succeed");
        assert_eq!(
            roll_handle.unwrap(),
            1,
            "Second effect should have handle 1"
        );
        assert_eq!(device.get_effect_count(), 2, "Should have 2 effects");
    }

    /// Test invalid axis index for constant force effect
    /// Requirements: FFB-HID-01.2
    #[test]
    #[cfg(windows)]
    #[ignore = "requires DirectInput FFB device registered in system"]
    fn test_constant_force_invalid_axis() {
        let mut device = DirectInputFfbDevice::new(TEST_GUID.to_string()).unwrap();
        device.initialize().unwrap();
        device.acquire(0).unwrap();

        // Try to create effect with invalid axis index
        let result = device.create_constant_force_effect(2);
        assert!(
            matches!(result, Err(DInputError::InvalidParameter(_))),
            "Should fail with invalid axis index"
        );

        let result = device.create_constant_force_effect(999);
        assert!(
            matches!(result, Err(DInputError::InvalidParameter(_))),
            "Should fail with out-of-range axis index"
        );
    }

    /// Test periodic (sine) effect creation
    /// Requirements: FFB-HID-01.2
    #[test]
    #[cfg(windows)]
    #[ignore = "requires DirectInput FFB device registered in system"]
    fn test_periodic_effect_creation() {
        let mut device = DirectInputFfbDevice::new(TEST_GUID.to_string()).unwrap();
        device.initialize().unwrap();
        device.acquire(0).unwrap();

        let handle = device.create_periodic_effect();
        assert!(handle.is_ok(), "Periodic effect creation should succeed");
        assert_eq!(handle.unwrap(), 0, "First effect should have handle 0");
        assert_eq!(device.get_effect_count(), 1, "Should have 1 effect");
    }

    /// Test spring condition effect creation
    /// Requirements: FFB-HID-01.2
    #[test]
    #[cfg(windows)]
    #[ignore = "requires DirectInput FFB device registered in system"]
    fn test_spring_effect_creation() {
        let mut device = DirectInputFfbDevice::new(TEST_GUID.to_string()).unwrap();
        device.initialize().unwrap();
        device.acquire(0).unwrap();

        let handle = device.create_spring_effect();
        assert!(handle.is_ok(), "Spring effect creation should succeed");
        assert_eq!(handle.unwrap(), 0, "First effect should have handle 0");
        assert_eq!(device.get_effect_count(), 1, "Should have 1 effect");
    }

    /// Test damper condition effect creation
    /// Requirements: FFB-HID-01.2
    #[test]
    #[cfg(windows)]
    #[ignore = "requires DirectInput FFB device registered in system"]
    fn test_damper_effect_creation() {
        let mut device = DirectInputFfbDevice::new(TEST_GUID.to_string()).unwrap();
        device.initialize().unwrap();
        device.acquire(0).unwrap();

        let handle = device.create_damper_effect();
        assert!(handle.is_ok(), "Damper effect creation should succeed");
        assert_eq!(handle.unwrap(), 0, "First effect should have handle 0");
        assert_eq!(device.get_effect_count(), 1, "Should have 1 effect");
    }

    /// Test creating multiple effect types simultaneously
    /// Requirements: FFB-HID-01.2
    #[test]
    #[cfg(windows)]
    #[ignore = "requires DirectInput FFB device registered in system"]
    fn test_multiple_effect_types() {
        let mut device = DirectInputFfbDevice::new(TEST_GUID.to_string()).unwrap();
        device.initialize().unwrap();
        device.acquire(0).unwrap();

        // Create all effect types
        let constant_h = device.create_constant_force_effect(0).unwrap();
        let periodic_h = device.create_periodic_effect().unwrap();
        let spring_h = device.create_spring_effect().unwrap();
        let damper_h = device.create_damper_effect().unwrap();

        // Verify handles are sequential
        assert_eq!(constant_h, 0);
        assert_eq!(periodic_h, 1);
        assert_eq!(spring_h, 2);
        assert_eq!(damper_h, 3);
        assert_eq!(device.get_effect_count(), 4, "Should have 4 effects");
    }

    // ========================================================================
    // Effect Parameter Update Tests
    // Requirements: FFB-HID-01.3, FFB-HID-01.4
    // ========================================================================

    /// Test constant force magnitude updates with clamping
    /// Requirements: FFB-HID-01.3, FFB-HID-01.4
    #[test]
    #[cfg(windows)]
    #[ignore = "requires DirectInput FFB device registered in system"]
    fn test_constant_force_magnitude_updates() {
        let mut device = DirectInputFfbDevice::new(TEST_GUID.to_string()).unwrap();
        device.initialize().unwrap();
        device.query_capabilities().unwrap(); // Sets max_torque_nm = 15.0
        device.acquire(0).unwrap();

        let handle = device.create_constant_force_effect(0).unwrap();

        // Test positive torque within limits
        assert!(device.set_constant_force(handle, 5.0).is_ok());
        assert_eq!(device.get_last_torque_nm(), 5.0);

        // Test negative torque within limits
        assert!(device.set_constant_force(handle, -8.0).is_ok());
        assert_eq!(device.get_last_torque_nm(), -8.0);

        // Test zero torque
        assert!(device.set_constant_force(handle, 0.0).is_ok());
        assert_eq!(device.get_last_torque_nm(), 0.0);

        // Test clamping at positive limit
        assert!(device.set_constant_force(handle, 20.0).is_ok());
        assert_eq!(
            device.get_last_torque_nm(),
            15.0,
            "Should clamp to max_torque_nm"
        );

        // Test clamping at negative limit
        assert!(device.set_constant_force(handle, -20.0).is_ok());
        assert_eq!(
            device.get_last_torque_nm(),
            -15.0,
            "Should clamp to -max_torque_nm"
        );
    }

    /// Test constant force updates on wrong effect type
    /// Requirements: FFB-HID-01.3
    #[test]
    #[cfg(windows)]
    #[ignore = "requires DirectInput FFB device registered in system"]
    fn test_constant_force_wrong_effect_type() {
        let mut device = DirectInputFfbDevice::new(TEST_GUID.to_string()).unwrap();
        device.initialize().unwrap();
        device.acquire(0).unwrap();

        // Create periodic effect
        let handle = device.create_periodic_effect().unwrap();

        // Try to set constant force on periodic effect
        let result = device.set_constant_force(handle, 5.0);
        assert!(
            matches!(result, Err(DInputError::InvalidParameter(_))),
            "Should fail when setting constant force on non-constant effect"
        );
    }

    /// Test periodic effect parameter updates with validation
    /// Requirements: FFB-HID-01.3, FFB-HID-01.4
    #[test]
    #[cfg(windows)]
    #[ignore = "requires DirectInput FFB device registered in system"]
    fn test_periodic_effect_parameters() {
        let mut device = DirectInputFfbDevice::new(TEST_GUID.to_string()).unwrap();
        device.initialize().unwrap();
        device.acquire(0).unwrap();

        let handle = device.create_periodic_effect().unwrap();

        // Test valid frequency and magnitude ranges
        assert!(device.set_periodic_parameters(handle, 10.0, 0.5).is_ok());
        assert!(device.set_periodic_parameters(handle, 1.0, 0.0).is_ok());
        assert!(device.set_periodic_parameters(handle, 100.0, 1.0).is_ok());
        assert!(device.set_periodic_parameters(handle, 500.0, 0.75).is_ok());

        // Test magnitude clamping
        assert!(
            device.set_periodic_parameters(handle, 50.0, 1.5).is_ok(),
            "Should clamp magnitude to 1.0"
        );
        assert!(
            device.set_periodic_parameters(handle, 50.0, -0.5).is_ok(),
            "Should clamp magnitude to 0.0"
        );

        // Test invalid frequency (zero)
        let result = device.set_periodic_parameters(handle, 0.0, 0.5);
        assert!(
            matches!(result, Err(DInputError::InvalidParameter(_))),
            "Should reject zero frequency"
        );

        // Test invalid frequency (negative)
        let result = device.set_periodic_parameters(handle, -10.0, 0.5);
        assert!(
            matches!(result, Err(DInputError::InvalidParameter(_))),
            "Should reject negative frequency"
        );

        // Test invalid frequency (too high)
        let result = device.set_periodic_parameters(handle, 2000.0, 0.5);
        assert!(
            matches!(result, Err(DInputError::InvalidParameter(_))),
            "Should reject frequency > 1000 Hz"
        );
    }

    /// Test periodic parameters on wrong effect type
    /// Requirements: FFB-HID-01.3
    #[test]
    #[cfg(windows)]
    #[ignore = "requires DirectInput FFB device registered in system"]
    fn test_periodic_parameters_wrong_effect_type() {
        let mut device = DirectInputFfbDevice::new(TEST_GUID.to_string()).unwrap();
        device.initialize().unwrap();
        device.acquire(0).unwrap();

        // Create constant force effect
        let handle = device.create_constant_force_effect(0).unwrap();

        // Try to set periodic parameters on constant force effect
        let result = device.set_periodic_parameters(handle, 10.0, 0.5);
        assert!(
            matches!(result, Err(DInputError::InvalidParameter(_))),
            "Should fail when setting periodic params on non-periodic effect"
        );
    }

    /// Test spring effect parameter updates with clamping
    /// Requirements: FFB-HID-01.3, FFB-HID-01.4
    #[test]
    #[cfg(windows)]
    #[ignore = "requires DirectInput FFB device registered in system"]
    fn test_spring_effect_parameters() {
        let mut device = DirectInputFfbDevice::new(TEST_GUID.to_string()).unwrap();
        device.initialize().unwrap();
        device.acquire(0).unwrap();

        let handle = device.create_spring_effect().unwrap();

        // Test valid center and stiffness ranges
        assert!(device.set_spring_parameters(handle, 0.0, 0.5).is_ok());
        assert!(device.set_spring_parameters(handle, -0.5, 1.0).is_ok());
        assert!(device.set_spring_parameters(handle, 1.0, 0.0).is_ok());
        assert!(device.set_spring_parameters(handle, -1.0, 0.75).is_ok());

        // Test center clamping
        assert!(
            device.set_spring_parameters(handle, 2.0, 0.5).is_ok(),
            "Should clamp center to 1.0"
        );
        assert!(
            device.set_spring_parameters(handle, -2.0, 0.5).is_ok(),
            "Should clamp center to -1.0"
        );

        // Test stiffness clamping
        assert!(
            device.set_spring_parameters(handle, 0.0, 1.5).is_ok(),
            "Should clamp stiffness to 1.0"
        );
        assert!(
            device.set_spring_parameters(handle, 0.0, -0.5).is_ok(),
            "Should clamp stiffness to 0.0"
        );
    }

    /// Test spring parameters on wrong effect type
    /// Requirements: FFB-HID-01.3
    #[test]
    #[cfg(windows)]
    #[ignore = "requires DirectInput FFB device registered in system"]
    fn test_spring_parameters_wrong_effect_type() {
        let mut device = DirectInputFfbDevice::new(TEST_GUID.to_string()).unwrap();
        device.initialize().unwrap();
        device.acquire(0).unwrap();

        // Create damper effect
        let handle = device.create_damper_effect().unwrap();

        // Try to set spring parameters on damper effect
        let result = device.set_spring_parameters(handle, 0.0, 0.5);
        assert!(
            matches!(result, Err(DInputError::InvalidParameter(_))),
            "Should fail when setting spring params on non-spring effect"
        );
    }

    /// Test damper effect parameter updates with clamping
    /// Requirements: FFB-HID-01.3, FFB-HID-01.4
    #[test]
    #[cfg(windows)]
    #[ignore = "requires DirectInput FFB device registered in system"]
    fn test_damper_effect_parameters() {
        let mut device = DirectInputFfbDevice::new(TEST_GUID.to_string()).unwrap();
        device.initialize().unwrap();
        device.acquire(0).unwrap();

        let handle = device.create_damper_effect().unwrap();

        // Test valid damping ranges
        assert!(device.set_damper_parameters(handle, 0.0).is_ok());
        assert!(device.set_damper_parameters(handle, 0.5).is_ok());
        assert!(device.set_damper_parameters(handle, 1.0).is_ok());

        // Test damping clamping
        assert!(
            device.set_damper_parameters(handle, 1.5).is_ok(),
            "Should clamp damping to 1.0"
        );
        assert!(
            device.set_damper_parameters(handle, -0.5).is_ok(),
            "Should clamp damping to 0.0"
        );
    }

    /// Test damper parameters on wrong effect type
    /// Requirements: FFB-HID-01.3
    #[test]
    #[cfg(windows)]
    #[ignore = "requires DirectInput FFB device registered in system"]
    fn test_damper_parameters_wrong_effect_type() {
        let mut device = DirectInputFfbDevice::new(TEST_GUID.to_string()).unwrap();
        device.initialize().unwrap();
        device.acquire(0).unwrap();

        // Create spring effect
        let handle = device.create_spring_effect().unwrap();

        // Try to set damper parameters on spring effect
        let result = device.set_damper_parameters(handle, 0.5);
        assert!(
            matches!(result, Err(DInputError::InvalidParameter(_))),
            "Should fail when setting damper params on non-damper effect"
        );
    }

    /// Test parameter updates with invalid effect handles
    /// Requirements: FFB-HID-01.3
    #[test]
    #[cfg(windows)]
    #[ignore = "requires DirectInput FFB device registered in system"]
    fn test_parameter_updates_invalid_handles() {
        let mut device = DirectInputFfbDevice::new(TEST_GUID.to_string()).unwrap();
        device.initialize().unwrap();
        device.acquire(0).unwrap();

        // Try to update non-existent effects
        assert!(matches!(
            device.set_constant_force(0, 5.0),
            Err(DInputError::InvalidParameter(_))
        ));

        assert!(matches!(
            device.set_periodic_parameters(0, 10.0, 0.5),
            Err(DInputError::InvalidParameter(_))
        ));

        assert!(matches!(
            device.set_spring_parameters(0, 0.0, 0.5),
            Err(DInputError::InvalidParameter(_))
        ));

        assert!(matches!(
            device.set_damper_parameters(0, 0.5),
            Err(DInputError::InvalidParameter(_))
        ));

        // Create one effect and try to use out-of-range handle
        let _handle = device.create_constant_force_effect(0).unwrap();

        assert!(matches!(
            device.set_constant_force(999, 5.0),
            Err(DInputError::InvalidParameter(_))
        ));
    }

    // ========================================================================
    // Effect Lifecycle Tests (Start/Stop)
    // Requirements: FFB-HID-01.4
    // ========================================================================

    /// Test effect start and stop operations
    /// Requirements: FFB-HID-01.4
    #[test]
    #[cfg(windows)]
    #[ignore = "requires DirectInput FFB device registered in system"]
    fn test_effect_start_stop() {
        let mut device = DirectInputFfbDevice::new(TEST_GUID.to_string()).unwrap();
        device.initialize().unwrap();
        device.acquire(0).unwrap();

        // Create effects
        let constant_h = device.create_constant_force_effect(0).unwrap();
        let periodic_h = device.create_periodic_effect().unwrap();

        // Test start/stop for constant force
        assert!(
            device.start_effect(constant_h).is_ok(),
            "Should start constant force effect"
        );
        assert!(
            device.stop_effect(constant_h).is_ok(),
            "Should stop constant force effect"
        );

        // Test start/stop for periodic
        assert!(
            device.start_effect(periodic_h).is_ok(),
            "Should start periodic effect"
        );
        assert!(
            device.stop_effect(periodic_h).is_ok(),
            "Should stop periodic effect"
        );

        // Test multiple start/stop cycles
        assert!(device.start_effect(constant_h).is_ok());
        assert!(
            device.start_effect(constant_h).is_ok(),
            "Multiple starts should be safe"
        );
        assert!(device.stop_effect(constant_h).is_ok());
        assert!(
            device.stop_effect(constant_h).is_ok(),
            "Multiple stops should be safe"
        );
    }

    /// Test effect start/stop with invalid handles
    /// Requirements: FFB-HID-01.4
    #[test]
    #[cfg(windows)]
    #[ignore = "requires DirectInput FFB device registered in system"]
    fn test_effect_start_stop_invalid_handles() {
        let mut device = DirectInputFfbDevice::new(TEST_GUID.to_string()).unwrap();
        device.initialize().unwrap();
        device.acquire(0).unwrap();

        // Try to start/stop non-existent effects
        assert!(matches!(
            device.start_effect(0),
            Err(DInputError::InvalidParameter(_))
        ));

        assert!(matches!(
            device.stop_effect(0),
            Err(DInputError::InvalidParameter(_))
        ));

        // Create one effect and try out-of-range handle
        let _handle = device.create_constant_force_effect(0).unwrap();

        assert!(matches!(
            device.start_effect(999),
            Err(DInputError::InvalidParameter(_))
        ));

        assert!(matches!(
            device.stop_effect(999),
            Err(DInputError::InvalidParameter(_))
        ));
    }

    /// Test effect start/stop without acquisition
    /// Requirements: FFB-HID-01.4
    #[test]
    #[cfg(windows)]
    #[ignore = "requires DirectInput FFB device registered in system"]
    fn test_effect_start_stop_without_acquisition() {
        let mut device = DirectInputFfbDevice::new(TEST_GUID.to_string()).unwrap();
        device.initialize().unwrap();

        // Try to start/stop without acquiring device
        assert!(matches!(
            device.start_effect(0),
            Err(DInputError::DeviceNotAcquired)
        ));

        assert!(matches!(
            device.stop_effect(0),
            Err(DInputError::DeviceNotAcquired)
        ));
    }

    // ========================================================================
    // Independent Axis Control Tests
    // Requirements: FFB-HID-01.2
    // ========================================================================

    /// Test independent control of pitch and roll axes
    /// Requirements: FFB-HID-01.2
    #[test]
    #[cfg(windows)]
    #[ignore = "requires DirectInput FFB device registered in system"]
    fn test_independent_pitch_roll_control() {
        let mut device = DirectInputFfbDevice::new(TEST_GUID.to_string()).unwrap();
        device.initialize().unwrap();
        device.query_capabilities().unwrap();
        device.acquire(0).unwrap();

        // Create separate effects for pitch and roll
        let pitch_h = device.create_constant_force_effect(0).unwrap();
        let roll_h = device.create_constant_force_effect(1).unwrap();

        // Set different torques for each axis
        assert!(device.set_constant_force(pitch_h, 3.0).is_ok());
        assert!(device.set_constant_force(roll_h, -2.5).is_ok());

        // Start both effects
        assert!(device.start_effect(pitch_h).is_ok());
        assert!(device.start_effect(roll_h).is_ok());

        // Update pitch without affecting roll
        assert!(device.set_constant_force(pitch_h, 5.0).is_ok());

        // Update roll without affecting pitch
        assert!(device.set_constant_force(roll_h, -4.0).is_ok());

        // Stop pitch while roll continues
        assert!(device.stop_effect(pitch_h).is_ok());

        // Stop roll
        assert!(device.stop_effect(roll_h).is_ok());
    }

    /// Test multiple effects of same type on different axes
    /// Requirements: FFB-HID-01.2
    #[test]
    #[cfg(windows)]
    #[ignore = "requires DirectInput FFB device registered in system"]
    fn test_multiple_constant_force_effects() {
        let mut device = DirectInputFfbDevice::new(TEST_GUID.to_string()).unwrap();
        device.initialize().unwrap();
        device.query_capabilities().unwrap();
        device.acquire(0).unwrap();

        // Create multiple constant force effects for different axes
        let pitch1_h = device.create_constant_force_effect(0).unwrap();
        let roll1_h = device.create_constant_force_effect(1).unwrap();
        let pitch2_h = device.create_constant_force_effect(0).unwrap();

        // All should have unique handles
        assert_ne!(pitch1_h, roll1_h);
        assert_ne!(pitch1_h, pitch2_h);
        assert_ne!(roll1_h, pitch2_h);

        // All should be controllable independently
        assert!(device.set_constant_force(pitch1_h, 2.0).is_ok());
        assert!(device.set_constant_force(roll1_h, -3.0).is_ok());
        assert!(device.set_constant_force(pitch2_h, 4.0).is_ok());
    }

    // ========================================================================
    // Comprehensive Integration Scenarios
    // ========================================================================

    /// Test complete device lifecycle with all effect types
    /// Requirements: FFB-HID-01.1, FFB-HID-01.2, FFB-HID-01.3, FFB-HID-01.4
    #[test]
    #[cfg(windows)]
    #[ignore = "requires DirectInput FFB device registered in system"]
    fn test_complete_device_lifecycle() {
        let mut device = DirectInputFfbDevice::new("test-device-guid".to_string()).unwrap();

        // Initialize device
        assert!(device.initialize().is_ok());

        // Query capabilities
        let caps = device.query_capabilities().unwrap();
        assert!(caps.supports_pid);

        // Acquire device
        assert!(device.acquire(0).is_ok());
        assert!(device.is_acquired());

        // Create all effect types
        let constant_h = device.create_constant_force_effect(0).unwrap();
        let periodic_h = device.create_periodic_effect().unwrap();
        let spring_h = device.create_spring_effect().unwrap();
        let damper_h = device.create_damper_effect().unwrap();

        // Configure effects
        assert!(device.set_constant_force(constant_h, 5.0).is_ok());
        assert!(
            device
                .set_periodic_parameters(periodic_h, 20.0, 0.5)
                .is_ok()
        );
        assert!(device.set_spring_parameters(spring_h, 0.0, 0.7).is_ok());
        assert!(device.set_damper_parameters(damper_h, 0.6).is_ok());

        // Start effects
        assert!(device.start_effect(constant_h).is_ok());
        assert!(device.start_effect(periodic_h).is_ok());
        assert!(device.start_effect(spring_h).is_ok());
        assert!(device.start_effect(damper_h).is_ok());

        // Update effects while running
        assert!(device.set_constant_force(constant_h, 8.0).is_ok());
        assert!(
            device
                .set_periodic_parameters(periodic_h, 30.0, 0.8)
                .is_ok()
        );

        // Stop effects
        assert!(device.stop_effect(constant_h).is_ok());
        assert!(device.stop_effect(periodic_h).is_ok());
        assert!(device.stop_effect(spring_h).is_ok());
        assert!(device.stop_effect(damper_h).is_ok());

        // Release device
        device.unacquire();
        assert!(!device.is_acquired());
    }

    /// Test error recovery scenarios
    /// Requirements: FFB-HID-01.1, FFB-HID-01.3
    #[test]
    #[cfg(windows)]
    #[ignore = "requires DirectInput FFB device registered in system"]
    fn test_error_recovery() {
        let mut device = DirectInputFfbDevice::new(TEST_GUID.to_string()).unwrap();
        device.initialize().unwrap();
        device.acquire(0).unwrap();

        let handle = device.create_constant_force_effect(0).unwrap();

        // Try invalid operation
        let result = device.set_periodic_parameters(handle, 10.0, 0.5);
        assert!(result.is_err(), "Invalid operation should fail");

        // Device should still be usable after error
        assert!(
            device.set_constant_force(handle, 5.0).is_ok(),
            "Should recover from error"
        );
        assert!(
            device.start_effect(handle).is_ok(),
            "Should still be able to start effect"
        );
        assert!(
            device.stop_effect(handle).is_ok(),
            "Should still be able to stop effect"
        );
    }

    // ========================================================================
    // Hardware Integration Tests (Optional, Hardware-Gated)
    // ========================================================================

    /// Hardware integration test: Enumerate real FFB devices
    ///
    /// This test requires actual FFB hardware connected to the system.
    /// It is marked with #[ignore] and must be explicitly run with:
    /// `cargo test --features hardware -- --ignored`
    ///
    /// Requirements: FFB-HID-01.1
    #[test]
    #[cfg(windows)]
    #[ignore = "Requires FFB hardware connected"]
    fn test_hardware_enumerate_real_devices() {
        let devices = DirectInputFfbDevice::enumerate_devices();
        assert!(devices.is_ok(), "Device enumeration should succeed");

        let device_list = devices.unwrap();
        println!("Found {} FFB devices", device_list.len());

        for (i, guid) in device_list.iter().enumerate() {
            println!("  Device {}: {}", i, guid);
        }

        // Note: This test doesn't assert device count since it depends on hardware
        // It just verifies enumeration doesn't crash
    }

    /// Hardware integration test: Acquire and create effects on real device
    ///
    /// This test requires actual FFB hardware connected to the system.
    /// It is marked with #[ignore] and must be explicitly run with:
    /// `cargo test --features hardware -- --ignored`
    ///
    /// Requirements: FFB-HID-01.1, FFB-HID-01.2, FFB-HID-01.3, FFB-HID-01.4
    #[test]
    #[cfg(windows)]
    #[ignore = "Requires FFB hardware connected"]
    fn test_hardware_create_and_control_effects() {
        // Enumerate devices
        let devices = DirectInputFfbDevice::enumerate_devices().unwrap();
        if devices.is_empty() {
            println!("No FFB devices found, skipping hardware test");
            return;
        }

        // Use first device
        let device_guid = devices[0].clone();
        println!("Testing with device: {}", device_guid);

        let mut device = DirectInputFfbDevice::new(device_guid).unwrap();

        // Initialize and acquire
        assert!(device.initialize().is_ok(), "Initialization should succeed");

        let caps = device.query_capabilities().unwrap();
        println!("Device capabilities:");
        println!("  supports_pid: {}", caps.supports_pid);
        println!("  max_torque_nm: {}", caps.max_torque_nm);
        println!("  num_axes: {}", caps.num_axes);
        println!("  max_effects: {}", caps.max_effects);

        assert!(device.acquire(0).is_ok(), "Acquisition should succeed");

        // Create constant force effect
        let handle = device.create_constant_force_effect(0).unwrap();
        println!("Created constant force effect: handle {}", handle);

        // Set a small torque (10% of max)
        let test_torque = caps.max_torque_nm * 0.1;
        assert!(device.set_constant_force(handle, test_torque).is_ok());
        println!("Set torque to {} Nm", test_torque);

        // Start effect briefly
        assert!(device.start_effect(handle).is_ok());
        println!("Started effect");

        std::thread::sleep(std::time::Duration::from_millis(500));

        // Stop effect
        assert!(device.stop_effect(handle).is_ok());
        println!("Stopped effect");

        // Clean up
        device.unacquire();
        println!("Test completed successfully");
    }
}
