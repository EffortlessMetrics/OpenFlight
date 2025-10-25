// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Comprehensive tests for OFP-1 protocol implementation
//!
//! This module provides integration tests that demonstrate the complete
//! OFP-1 handshake, capability negotiation, torque path stability,
//! and health stream monitoring functionality.

#![cfg(feature = "ofp1-tests")]

#[cfg(test)]
mod tests {
    use crate::ofp1::Ofp1Device;  // Bring trait into scope to resolve "method not found"
    use super::super::ofp1::*;
    use flight_virtual::ofp1_emulator::{Ofp1Emulator, Ofp1EmulatorConfig, EmulatorFaultType};
    use std::time::{Duration, Instant};
    use std::thread;

    /// Test complete OFP-1 handshake and capability negotiation
    #[test]
    fn test_complete_ofp1_handshake() {
        // Create emulator with high-end capabilities
        let mut config = Ofp1EmulatorConfig::default();
        config.max_torque_mnm = 20000; // 20 Nm
        config.min_period_us = 500;    // 2 kHz
        
        let mut emulator = Ofp1Emulator::with_config("/dev/ofp1_test".to_string(), config);
        emulator.start().unwrap();
        
        // Get capabilities
        let capabilities = emulator.get_capabilities().unwrap();
        assert_eq!(capabilities.protocol_version, OFP1_VERSION);
        assert_eq!(capabilities.max_torque_mnm, 20000);
        assert_eq!(capabilities.min_period_us, 500);
        assert!(capabilities.capability_flags.has_flag(CapabilityFlags::HEALTH_STREAM));
        assert!(capabilities.capability_flags.has_flag(CapabilityFlags::PHYSICAL_INTERLOCK));
        
        // Perform negotiation
        let negotiator = Ofp1Negotiator::new();
        let result = negotiator.negotiate(&capabilities).unwrap();
        
        assert_eq!(result.protocol_version, OFP1_VERSION);
        assert_eq!(result.max_torque_nm, 20.0);
        assert_eq!(result.effective_update_rate_hz, 2000);
        assert!(result.supports_high_torque);
        assert!(result.has_temperature_sensor);
        assert!(result.has_current_sensor);
        
        emulator.stop();
    }
    
    /// Test torque command sending and health monitoring
    #[test]
    fn test_torque_commands_and_health_monitoring() {
        let mut emulator = Ofp1Emulator::new("/dev/ofp1_test".to_string());
        emulator.start().unwrap();
        
        // Set up health monitor
        let mut health_monitor = Ofp1HealthMonitor::new(Duration::from_millis(100));
        
        // Send torque commands
        let mut sequence = 1u16;
        for torque_nm in [0.0, 5.0, 10.0, -5.0, 0.0] {
            let torque_protocol = utils::torque_nm_to_protocol(torque_nm, 15.0);
            
            let mut command = TorqueCommandReport {
                report_id: report_ids::TORQUE_COMMAND,
                sequence,
                torque_command: torque_protocol,
                command_flags: CommandFlags::new(),
                timestamp_us: 0,
                reserved: [0; 5],
            };
            
            command.command_flags.set_flag(CommandFlags::ENABLE);
            if torque_nm.abs() > 8.0 {
                command.command_flags.set_flag(CommandFlags::HIGH_TORQUE);
                command.command_flags.set_flag(CommandFlags::INTERLOCK_OK);
            }
            
            // Send command
            emulator.send_torque_command(command).unwrap();
            
            // Wait for processing
            thread::sleep(Duration::from_millis(20));
            
            // Read health status
            if let Some(health) = emulator.read_health_status().unwrap() {
                assert_eq!(health.sequence, sequence);
                assert!(health.status_flags.has_flag(StatusFlags::READY));
                
                if torque_nm != 0.0 {
                    assert!(health.status_flags.has_flag(StatusFlags::TORQUE_ENABLED));
                }
                
                // Update health monitor
                health_monitor.update_health(health).unwrap();
            }
            
            sequence += 1;
        }
        
        // Verify health monitor is current
        assert!(health_monitor.is_health_current());
        
        emulator.stop();
    }
    
    /// Test fault injection and recovery
    #[test]
    fn test_fault_injection_and_recovery() {
        let mut emulator = Ofp1Emulator::new("/dev/ofp1_test".to_string());
        emulator.start().unwrap();
        
        let mut health_monitor = Ofp1HealthMonitor::new(Duration::from_millis(100));
        
        // Normal operation first
        let health = emulator.read_health_status().unwrap().unwrap();
        assert!(!health.status_flags.has_fault());
        health_monitor.update_health(health).unwrap();
        
        // Inject temperature fault
        emulator.inject_fault(EmulatorFaultType::TemperatureFault);
        
        // Wait for fault to propagate
        thread::sleep(Duration::from_millis(50));
        
        // Read health with fault
        let health = emulator.read_health_status().unwrap().unwrap();
        assert!(health.status_flags.has_fault());
        assert!(health.status_flags.has_flag(StatusFlags::TEMP_FAULT));
        
        // Health monitor should detect fault
        let result = health_monitor.update_health(health);
        assert!(result.is_err());
        
        // Clear faults
        emulator.clear_faults();
        
        // Wait for recovery
        thread::sleep(Duration::from_millis(50));
        
        // Verify recovery
        let health = emulator.read_health_status().unwrap().unwrap();
        assert!(!health.status_flags.has_fault());
        health_monitor.update_health(health).unwrap();
        
        emulator.stop();
    }
    
    /// Test emergency stop functionality
    #[test]
    fn test_emergency_stop() {
        let mut emulator = Ofp1Emulator::new("/dev/ofp1_test".to_string());
        emulator.start().unwrap();
        
        // Send normal torque command
        let mut command = TorqueCommandReport {
            report_id: report_ids::TORQUE_COMMAND,
            sequence: 1,
            torque_command: 16384, // Half scale
            command_flags: CommandFlags::new(),
            timestamp_us: 0,
            reserved: [0; 5],
        };
        
        command.command_flags.set_flag(CommandFlags::ENABLE);
        emulator.send_torque_command(command).unwrap();
        
        thread::sleep(Duration::from_millis(20));
        
        // Verify torque is applied
        let stats = emulator.get_statistics();
        assert_eq!(stats.current_torque_protocol, 16384);
        
        // Trigger emergency stop
        emulator.trigger_emergency_stop();
        
        thread::sleep(Duration::from_millis(20));
        
        // Verify emergency stop
        let stats = emulator.get_statistics();
        assert!(stats.emergency_stop_active);
        assert_eq!(stats.current_torque_protocol, 0);
        
        let health = emulator.read_health_status().unwrap().unwrap();
        assert!(health.status_flags.has_flag(StatusFlags::EMERGENCY_STOP));
        
        emulator.stop();
    }
    
    /// Test interlock functionality
    #[test]
    fn test_interlock_functionality() {
        let mut emulator = Ofp1Emulator::new("/dev/ofp1_test".to_string());
        emulator.start().unwrap();
        
        // Initially interlock should not be satisfied
        let stats = emulator.get_statistics();
        assert!(!stats.interlock_satisfied);
        
        // Set interlock satisfied
        emulator.set_interlock_satisfied(true);
        
        let stats = emulator.get_statistics();
        assert!(stats.interlock_satisfied);
        assert!(stats.status_flags.has_flag(StatusFlags::INTERLOCK_OK));
        
        // Send high torque command with interlock
        let mut command = TorqueCommandReport {
            report_id: report_ids::TORQUE_COMMAND,
            sequence: 1,
            torque_command: 30000, // High torque
            command_flags: CommandFlags::new(),
            timestamp_us: 0,
            reserved: [0; 5],
        };
        
        command.command_flags.set_flag(CommandFlags::ENABLE);
        command.command_flags.set_flag(CommandFlags::HIGH_TORQUE);
        command.command_flags.set_flag(CommandFlags::INTERLOCK_OK);
        
        emulator.send_torque_command(command).unwrap();
        
        thread::sleep(Duration::from_millis(20));
        
        let stats = emulator.get_statistics();
        assert!(stats.high_torque_enabled);
        
        emulator.stop();
    }
    
    /// Test health stream timeout detection
    #[test]
    fn test_health_stream_timeout() {
        let mut health_monitor = Ofp1HealthMonitor::new(Duration::from_millis(50));
        
        // Send initial health report
        let health = HealthStatusReport {
            report_id: report_ids::HEALTH_STATUS,
            sequence: 1,
            status_flags: StatusFlags::new(),
            current_torque: 0,
            temperature_dc: 250,
            current_ma: 1000,
            encoder_position: 0,
            uptime_s: 60,
            reserved: [0; 2],
        };
        
        health_monitor.update_health(health).unwrap();
        assert!(health_monitor.is_health_current());
        
        // Wait for timeout
        thread::sleep(Duration::from_millis(100));
        
        // Check timeout
        assert!(!health_monitor.is_health_current());
        let result = health_monitor.check_timeout();
        assert!(result.is_err());
        
        if let Err(Ofp1Error::HealthTimeout { elapsed }) = result {
            assert!(elapsed > Duration::from_millis(50));
        } else {
            panic!("Expected health timeout error");
        }
    }
    
    /// Test torque conversion accuracy
    #[test]
    fn test_torque_conversion_accuracy() {
        let max_torque = 15.0;
        let test_values = [
            -15.0, -10.0, -5.0, -1.0, 0.0, 1.0, 5.0, 10.0, 15.0
        ];
        
        for &torque_nm in &test_values {
            let protocol_val = utils::torque_nm_to_protocol(torque_nm, max_torque);
            let converted_back = utils::torque_protocol_to_nm(protocol_val, max_torque);
            
            // Allow small floating point error
            let error = (converted_back - torque_nm).abs();
            assert!(error < 0.01, "Conversion error too large: {} -> {} -> {} (error: {})", 
                torque_nm, protocol_val, converted_back, error);
        }
    }
    
    /// Test capability validation
    #[test]
    fn test_capability_validation() {
        // Valid capabilities
        let valid_caps = CapabilitiesReport {
            report_id: report_ids::CAPABILITIES,
            protocol_version: OFP1_VERSION,
            vendor_id: 0x1234,
            product_id: 0x5678,
            max_torque_mnm: 15000,
            min_period_us: 1000,
            capability_flags: CapabilityFlags::new(),
            serial_number: *b"TEST1234",
            reserved: [0; 8],
        };
        
        assert!(utils::validate_capabilities(&valid_caps).is_ok());
        
        // Test various invalid cases
        let mut invalid_caps = valid_caps.clone();
        
        // Invalid report ID
        invalid_caps.report_id = 0xFF;
        assert!(utils::validate_capabilities(&invalid_caps).is_err());
        
        // Invalid protocol version
        invalid_caps = valid_caps.clone();
        invalid_caps.protocol_version = 0;
        assert!(utils::validate_capabilities(&invalid_caps).is_err());
        
        // Invalid max torque
        invalid_caps = valid_caps.clone();
        invalid_caps.max_torque_mnm = 0;
        assert!(utils::validate_capabilities(&invalid_caps).is_err());
    }
    
    /// Test negotiation with different device types
    #[test]
    fn test_negotiation_device_types() {
        let negotiator = Ofp1Negotiator::new();
        
        // High-end device
        let mut high_end_caps = CapabilitiesReport {
            report_id: report_ids::CAPABILITIES,
            protocol_version: OFP1_VERSION,
            vendor_id: 0x1234,
            product_id: 0x5678,
            max_torque_mnm: 25000, // 25 Nm
            min_period_us: 500,    // 2 kHz
            capability_flags: CapabilityFlags::new(),
            serial_number: *b"HIGHEND1",
            reserved: [0; 8],
        };
        
        high_end_caps.capability_flags.set_flag(CapabilityFlags::HEALTH_STREAM);
        high_end_caps.capability_flags.set_flag(CapabilityFlags::BIDIRECTIONAL);
        high_end_caps.capability_flags.set_flag(CapabilityFlags::PHYSICAL_INTERLOCK);
        high_end_caps.capability_flags.set_flag(CapabilityFlags::TEMPERATURE_SENSOR);
        high_end_caps.capability_flags.set_flag(CapabilityFlags::CURRENT_SENSOR);
        
        let result = negotiator.negotiate(&high_end_caps).unwrap();
        assert_eq!(result.max_torque_nm, 25.0);
        assert_eq!(result.effective_update_rate_hz, 2000);
        assert!(result.supports_high_torque);
        assert!(result.has_temperature_sensor);
        
        // Low-end device
        let mut low_end_caps = CapabilitiesReport {
            report_id: report_ids::CAPABILITIES,
            protocol_version: OFP1_VERSION,
            vendor_id: 0x5678,
            product_id: 0x9ABC,
            max_torque_mnm: 3000,  // 3 Nm
            min_period_us: 10000,  // 100 Hz
            capability_flags: CapabilityFlags::new(),
            serial_number: *b"LOWEND01",
            reserved: [0; 8],
        };
        
        low_end_caps.capability_flags.set_flag(CapabilityFlags::HEALTH_STREAM);
        low_end_caps.capability_flags.set_flag(CapabilityFlags::BIDIRECTIONAL);
        
        let result = negotiator.negotiate(&low_end_caps).unwrap();
        assert_eq!(result.max_torque_nm, 3.0);
        assert_eq!(result.effective_update_rate_hz, 100);
        assert!(!result.supports_high_torque); // Too low torque
        assert!(!result.has_temperature_sensor);
    }
    
    /// Test complete integration scenario
    #[test]
    fn test_complete_integration_scenario() {
        // This test demonstrates a complete OFP-1 integration scenario
        let mut emulator = Ofp1Emulator::new("/dev/ofp1_integration_test".to_string());
        emulator.start().unwrap();
        
        // Step 1: Capability negotiation
        let capabilities = emulator.get_capabilities().unwrap();
        let negotiator = Ofp1Negotiator::new();
        let negotiation_result = negotiator.negotiate(&capabilities).unwrap();
        
        println!("Negotiation result: {:?}", negotiation_result);
        
        // Step 2: Set up health monitoring
        let mut health_monitor = Ofp1HealthMonitor::new(Duration::from_millis(200));
        
        // Step 3: Enable interlock for high torque
        emulator.set_interlock_satisfied(true);
        
        // Step 4: Send a sequence of torque commands
        let torque_sequence = [0.0, 2.0, 5.0, 8.0, 12.0, 8.0, 5.0, 2.0, 0.0];
        let mut sequence = 1u16;
        
        for &torque_nm in &torque_sequence {
            let torque_protocol = utils::torque_nm_to_protocol(torque_nm, negotiation_result.max_torque_nm);
            
            let mut command = TorqueCommandReport {
                report_id: report_ids::TORQUE_COMMAND,
                sequence,
                torque_command: torque_protocol,
                command_flags: CommandFlags::new(),
                timestamp_us: (sequence as u32) * 1000, // 1ms intervals
                reserved: [0; 5],
            };
            
            command.command_flags.set_flag(CommandFlags::ENABLE);
            
            if torque_nm > 8.0 && negotiation_result.supports_high_torque {
                command.command_flags.set_flag(CommandFlags::HIGH_TORQUE);
                command.command_flags.set_flag(CommandFlags::INTERLOCK_OK);
            }
            
            // Send command
            emulator.send_torque_command(command).unwrap();
            
            // Wait for processing
            thread::sleep(Duration::from_millis(10));
            
            // Read and validate health
            if let Some(health) = emulator.read_health_status().unwrap() {
                assert_eq!(health.sequence, sequence);
                assert!(health.status_flags.has_flag(StatusFlags::READY));
                
                if torque_nm > 0.0 {
                    assert!(health.status_flags.has_flag(StatusFlags::TORQUE_ENABLED));
                }
                
                if torque_nm > 8.0 {
                    assert!(health.status_flags.has_flag(StatusFlags::HIGH_TORQUE_ACTIVE));
                    assert!(health.status_flags.has_flag(StatusFlags::INTERLOCK_OK));
                }
                
                // Verify torque tracking
                let actual_torque = utils::torque_protocol_to_nm(health.current_torque, negotiation_result.max_torque_nm);
                let error = (actual_torque - torque_nm).abs();
                assert!(error < 0.1, "Torque tracking error: expected {}, got {} (error: {})", 
                    torque_nm, actual_torque, error);
                
                // Update health monitor
                health_monitor.update_health(health).unwrap();
            }
            
            sequence += 1;
        }
        
        // Step 5: Test emergency stop
        emulator.trigger_emergency_stop();
        thread::sleep(Duration::from_millis(20));
        
        let health = emulator.read_health_status().unwrap().unwrap();
        assert!(health.status_flags.has_flag(StatusFlags::EMERGENCY_STOP));
        assert_eq!(health.current_torque, 0);
        
        // Step 6: Verify health monitor state
        assert!(health_monitor.is_health_current());
        assert_eq!(health_monitor.fault_history().len(), 0); // No faults during normal operation
        
        emulator.stop();
        
        println!("Complete integration scenario test passed successfully!");
    }
}