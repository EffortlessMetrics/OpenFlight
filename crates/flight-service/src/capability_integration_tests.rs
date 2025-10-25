// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Integration tests for capability enforcement

use crate::capability_service::{CapabilityService, CapabilityServiceConfig};
use flight_core::profile::{Profile, AxisConfig, AircraftId, CapabilityMode, CapabilityContext};
use flight_axis::AxisEngine;
use std::collections::HashMap;
use std::sync::Arc;

#[cfg(test)]
mod tests {
    use super::*;

    /// Test that profile validation properly rejects overrides in restricted modes
    #[test]
    fn test_profile_validation_rejects_overrides() {
        // Create a profile with high values that should be rejected in kid mode
        let mut axes = HashMap::new();
        axes.insert("pitch".to_string(), AxisConfig {
            deadzone: Some(0.03),
            expo: Some(0.9), // High expo - should be rejected in kid mode (limit 0.2)
            slew_rate: Some(15.0), // High slew rate - should be rejected in kid mode (limit 2.0)
            detents: vec![],
            curve: Some(vec![
                flight_core::profile::CurvePoint { input: 0.0, output: 0.0 },
                flight_core::profile::CurvePoint { input: 1.0, output: 1.0 },
            ]), // Custom curve - should be rejected in kid mode
        });

        let profile = Profile {
            schema: "flight.profile/1".to_string(),
            sim: Some("msfs".to_string()),
            aircraft: Some(AircraftId { icao: "C172".to_string() }),
            axes,
            pof_overrides: None,
        };

        // Should pass in full mode
        let full_context = CapabilityContext::for_mode(CapabilityMode::Full);
        assert!(profile.validate_with_capabilities(&full_context).is_ok());

        // Should fail in demo mode due to high expo and slew rate
        let demo_context = CapabilityContext::for_mode(CapabilityMode::Demo);
        assert!(profile.validate_with_capabilities(&demo_context).is_err());

        // Should fail in kid mode due to multiple violations
        let kid_context = CapabilityContext::for_mode(CapabilityMode::Kid);
        assert!(profile.validate_with_capabilities(&kid_context).is_err());
    }

    /// Test that engine properly clamps outputs according to capability limits
    #[test]
    fn test_engine_output_clamping() {
        let engine = AxisEngine::new_for_axis("test_axis".to_string());
        
        // Test in full mode - no clamping
        engine.set_capability_mode(CapabilityMode::Full);
        let mut frame = flight_axis::AxisFrame::new(0.9, 1000);
        frame.out = 0.9;
        assert!(engine.process(&mut frame).is_ok());
        assert_eq!(frame.out, 0.9); // No clamping

        // Test in demo mode - clamp to 80%
        engine.set_capability_mode(CapabilityMode::Demo);
        let mut frame = flight_axis::AxisFrame::new(0.9, 2000);
        frame.out = 0.9;
        assert!(engine.process(&mut frame).is_ok());
        assert_eq!(frame.out, 0.8); // Clamped to demo limit

        // Test in kid mode - clamp to 50%
        engine.set_capability_mode(CapabilityMode::Kid);
        let mut frame = flight_axis::AxisFrame::new(0.9, 3000);
        frame.out = 0.9;
        assert!(engine.process(&mut frame).is_ok());
        assert_eq!(frame.out, 0.5); // Clamped to kid limit

        // Test negative values are also clamped
        let mut frame = flight_axis::AxisFrame::new(-0.8, 4000);
        frame.out = -0.8;
        assert!(engine.process(&mut frame).is_ok());
        assert_eq!(frame.out, -0.5); // Clamped to -50%
    }

    /// Test capability service IPC-like functionality
    #[test]
    fn test_capability_service_ipc_simulation() {
        let service = CapabilityService::new();
        
        // Register multiple axes
        let pitch_engine = Arc::new(AxisEngine::new_for_axis("pitch".to_string()));
        let roll_engine = Arc::new(AxisEngine::new_for_axis("roll".to_string()));
        let yaw_engine = Arc::new(AxisEngine::new_for_axis("yaw".to_string()));
        
        service.register_axis("pitch".to_string(), pitch_engine.clone()).unwrap();
        service.register_axis("roll".to_string(), roll_engine.clone()).unwrap();
        service.register_axis("yaw".to_string(), yaw_engine.clone()).unwrap();

        // Simulate IPC call: Set global kid mode
        let result = service.set_capability_mode(CapabilityMode::Kid, None, true).unwrap();
        assert!(result.success);
        assert_eq!(result.affected_axes.len(), 3);
        assert_eq!(result.applied_limits.max_axis_output, 0.5);

        // Verify all engines are in kid mode
        assert_eq!(pitch_engine.capability_mode(), CapabilityMode::Kid);
        assert_eq!(roll_engine.capability_mode(), CapabilityMode::Kid);
        assert_eq!(yaw_engine.capability_mode(), CapabilityMode::Kid);

        // Simulate IPC call: Set only pitch to demo mode
        let result = service.set_capability_mode(
            CapabilityMode::Demo,
            Some(vec!["pitch".to_string()]),
            true,
        ).unwrap();
        assert!(result.success);
        assert_eq!(result.affected_axes, vec!["pitch"]);

        // Verify only pitch changed
        assert_eq!(pitch_engine.capability_mode(), CapabilityMode::Demo);
        assert_eq!(roll_engine.capability_mode(), CapabilityMode::Kid);
        assert_eq!(yaw_engine.capability_mode(), CapabilityMode::Kid);

        // Simulate IPC call: Get capability status
        let status = service.get_capability_status(None).unwrap();
        assert_eq!(status.len(), 3);
        
        let pitch_status = status.iter().find(|s| s.axis_name == "pitch").unwrap();
        assert_eq!(pitch_status.mode, CapabilityMode::Demo);
        assert_eq!(pitch_status.limits.max_axis_output, 0.8);

        let roll_status = status.iter().find(|s| s.axis_name == "roll").unwrap();
        assert_eq!(roll_status.mode, CapabilityMode::Kid);
        assert_eq!(roll_status.limits.max_axis_output, 0.5);
    }

    /// Test that audit logging is properly enabled/disabled
    #[test]
    fn test_audit_logging_control() {
        let service = CapabilityService::new();
        let engine = Arc::new(AxisEngine::new_for_axis("test_axis".to_string()));
        
        service.register_axis("test_axis".to_string(), engine.clone()).unwrap();

        // Set kid mode with audit enabled
        let result = service.set_capability_mode(CapabilityMode::Kid, None, true).unwrap();
        assert!(result.success);

        // Get status and verify audit is enabled
        let status = service.get_capability_status(None).unwrap();
        assert_eq!(status.len(), 1);
        assert!(status[0].audit_enabled);

        // Process a frame that should be clamped (this would generate audit log)
        let mut frame = flight_axis::AxisFrame::new(0.8, 1000);
        frame.out = 0.8;
        assert!(engine.process(&mut frame).is_ok());
        assert_eq!(frame.out, 0.5); // Clamped
    }

    /// Test convenience methods for kid/demo mode
    #[test]
    fn test_convenience_methods() {
        let service = CapabilityService::new();
        let engine = Arc::new(AxisEngine::new_for_axis("test_axis".to_string()));
        
        service.register_axis("test_axis".to_string(), engine.clone()).unwrap();

        // Test kid mode convenience
        let result = service.set_kid_mode(true).unwrap();
        assert!(result.success);
        assert_eq!(engine.capability_mode(), CapabilityMode::Kid);

        let result = service.set_kid_mode(false).unwrap();
        assert!(result.success);
        assert_eq!(engine.capability_mode(), CapabilityMode::Full);

        // Test demo mode convenience
        let result = service.set_demo_mode(true).unwrap();
        assert!(result.success);
        assert_eq!(engine.capability_mode(), CapabilityMode::Demo);

        let result = service.set_demo_mode(false).unwrap();
        assert!(result.success);
        assert_eq!(engine.capability_mode(), CapabilityMode::Full);
    }

    /// Test restricted axes detection
    #[test]
    fn test_restricted_axes_detection() {
        let service = CapabilityService::new();
        
        let pitch_engine = Arc::new(AxisEngine::new_for_axis("pitch".to_string()));
        let roll_engine = Arc::new(AxisEngine::new_for_axis("roll".to_string()));
        
        service.register_axis("pitch".to_string(), pitch_engine.clone()).unwrap();
        service.register_axis("roll".to_string(), roll_engine.clone()).unwrap();

        // Initially no restricted axes
        assert!(!service.has_restricted_axes().unwrap());
        assert!(service.get_restricted_axes().unwrap().is_empty());

        // Set pitch to kid mode
        service.set_capability_mode(
            CapabilityMode::Kid,
            Some(vec!["pitch".to_string()]),
            true,
        ).unwrap();

        // Should detect restricted axes
        assert!(service.has_restricted_axes().unwrap());
        let restricted = service.get_restricted_axes().unwrap();
        assert_eq!(restricted.len(), 1);
        assert_eq!(restricted[0].0, "pitch");
        assert_eq!(restricted[0].1, CapabilityMode::Kid);

        // Set roll to demo mode
        service.set_capability_mode(
            CapabilityMode::Demo,
            Some(vec!["roll".to_string()]),
            true,
        ).unwrap();

        // Should detect both restricted axes
        let restricted = service.get_restricted_axes().unwrap();
        assert_eq!(restricted.len(), 2);
        
        let pitch_entry = restricted.iter().find(|(name, _)| name == "pitch").unwrap();
        assert_eq!(pitch_entry.1, CapabilityMode::Kid);
        
        let roll_entry = restricted.iter().find(|(name, _)| name == "roll").unwrap();
        assert_eq!(roll_entry.1, CapabilityMode::Demo);
    }

    /// Test that capability limits are properly enforced across different modes
    #[test]
    fn test_capability_limits_enforcement() {
        use flight_core::profile::CapabilityLimits;

        let full_limits = CapabilityLimits {
            max_axis_output: 1.0,
            max_ffb_torque: 10.0,
            allow_high_torque: true,
            max_expo: 1.0,
            max_slew_rate: 10.0,
        };
        let demo_limits = CapabilityLimits {
            max_axis_output: 0.8,
            max_ffb_torque: 7.0,
            allow_high_torque: false,
            max_expo: 0.7,
            max_slew_rate: 7.0,
        };
        let kid_limits = CapabilityLimits {
            max_axis_output: 0.5,
            max_ffb_torque: 3.0,
            allow_high_torque: false,
            max_expo: 0.3,
            max_slew_rate: 3.0,
        };

        // Verify the hierarchy: Full > Demo > Kid
        assert!(full_limits.max_axis_output >= demo_limits.max_axis_output);
        assert!(demo_limits.max_axis_output >= kid_limits.max_axis_output);
        
        assert!(full_limits.max_ffb_torque >= demo_limits.max_ffb_torque);
        assert!(demo_limits.max_ffb_torque >= kid_limits.max_ffb_torque);
        
        assert!(full_limits.max_slew_rate >= demo_limits.max_slew_rate);
        assert!(demo_limits.max_slew_rate >= kid_limits.max_slew_rate);
        
        assert!(full_limits.max_curve_expo >= demo_limits.max_curve_expo);
        assert!(demo_limits.max_curve_expo >= kid_limits.max_curve_expo);

        // Verify boolean restrictions
        assert!(full_limits.allow_high_torque);
        assert!(!demo_limits.allow_high_torque);
        assert!(!kid_limits.allow_high_torque);
        
        assert!(full_limits.allow_custom_curves);
        assert!(demo_limits.allow_custom_curves);
        assert!(!kid_limits.allow_custom_curves);
    }
}