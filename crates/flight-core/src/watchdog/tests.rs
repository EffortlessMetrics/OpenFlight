// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Comprehensive tests for watchdog and quarantine system
//!
//! Tests all requirements including USB stall detection, plugin overrun monitoring,
//! quarantine behavior, and synthetic fault injection.

use super::*;
use std::thread;

/// Test suite for USB stall timeout detection
mod usb_stall_tests {
    use super::*;

    #[test]
    fn test_usb_stall_threshold_detection() {
        let mut watchdog = WatchdogSystem::new();
        let endpoint_id = "test_usb_endpoint";
        let component = ComponentType::UsbEndpoint(endpoint_id.to_string());
        
        watchdog.register_component(component.clone(), WatchdogConfig::default());

        // First two stalls should not trigger fault
        assert!(watchdog.record_usb_stall(endpoint_id).is_none());
        assert!(watchdog.record_usb_stall(endpoint_id).is_none());
        assert!(!watchdog.is_quarantined(&component));
        
        // Third stall should trigger fault
        let event = watchdog.record_usb_stall(endpoint_id).expect("Should trigger fault after 3 stalls");
        assert_eq!(event.event_type, WatchdogEventType::UsbTimeout);
        assert_eq!(event.action_taken, WatchdogAction::ResetUsbEndpoint);
        assert!(event.context.contains("3 frames"));
    }

    #[test]
    fn test_usb_stall_counter_reset() {
        let mut watchdog = WatchdogSystem::new();
        let endpoint_id = "test_usb_endpoint";
        let component = ComponentType::UsbEndpoint(endpoint_id.to_string());
        
        watchdog.register_component(component, WatchdogConfig::default());

        // Record two stalls
        watchdog.record_usb_stall(endpoint_id);
        watchdog.record_usb_stall(endpoint_id);
        
        // Reset counter with successful operation
        watchdog.reset_usb_stall_counter();
        
        // Next stall should not trigger fault (counter was reset)
        assert!(watchdog.record_usb_stall(endpoint_id).is_none());
    }

    #[test]
    fn test_usb_endpoint_timeout_detection() {
        let mut watchdog = WatchdogSystem::new();
        let endpoint_id = "test_timeout_endpoint";
        let component = ComponentType::UsbEndpoint(endpoint_id.to_string());
        
        let mut config = WatchdogConfig::default();
        config.usb_timeout = Duration::from_millis(10); // Very short timeout for testing
        
        watchdog.register_component(component, config);

        // Wait for timeout period
        thread::sleep(Duration::from_millis(15));
        
        let event = watchdog.check_usb_timeout(endpoint_id).expect("Should detect timeout");
        assert_eq!(event.event_type, WatchdogEventType::UsbTimeout);
        assert!(event.context.contains("Timeout after"));
    }

    #[test]
    fn test_endpoint_wedge_detection() {
        let mut watchdog = WatchdogSystem::new();
        let endpoint_id = "test_wedge_endpoint";
        let component = ComponentType::UsbEndpoint(endpoint_id.to_string());
        
        watchdog.register_component(component, WatchdogConfig::default());

        // Simulate unresponsive endpoint for wedge detection period
        assert!(watchdog.check_endpoint_wedge(false).is_none()); // Start timer
        
        thread::sleep(Duration::from_millis(110)); // Wait past 100ms threshold
        
        let event = watchdog.check_endpoint_wedge(false).expect("Should detect wedged endpoint");
        assert_eq!(event.event_type, WatchdogEventType::UsbTimeout);
    }

    #[test]
    fn test_usb_error_handling() {
        let mut watchdog = WatchdogSystem::new();
        let endpoint_id = "test_error_endpoint";
        let component = ComponentType::UsbEndpoint(endpoint_id.to_string());
        
        watchdog.register_component(component.clone(), WatchdogConfig::default());

        let event = watchdog.record_usb_error(endpoint_id, "Test USB error");
        assert_eq!(event.event_type, WatchdogEventType::UsbError);
        assert_eq!(event.context, "Test USB error");
        assert!(!watchdog.is_quarantined(&component)); // Should not quarantine on first error
    }
}

/// Test suite for plugin overrun detection and PLUG-OVERRUN events
mod plugin_overrun_tests {
    use super::*;

    #[test]
    fn test_plugin_overrun_detection() {
        let mut watchdog = WatchdogSystem::new();
        let plugin_id = "test_plugin";
        let component = ComponentType::NativePlugin(plugin_id.to_string());
        
        watchdog.register_component(component, WatchdogConfig::default());

        // Normal execution should not trigger overrun
        let normal_time = Duration::from_micros(50);
        assert!(watchdog.record_plugin_execution(plugin_id, normal_time, true).is_none());

        // Excessive execution should trigger overrun
        let excessive_time = Duration::from_millis(1); // Much longer than 100μs budget
        let event = watchdog.record_plugin_execution(plugin_id, excessive_time, true)
            .expect("Should detect plugin overrun");
        
        assert_eq!(event.event_type, WatchdogEventType::PluginOverrun);
        assert_eq!(event.execution_time, Some(excessive_time));
        assert!(event.context.contains("exceeded budget"));
    }

    #[test]
    fn test_plugin_overrun_counter() {
        let mut watchdog = WatchdogSystem::new();
        let plugin_id = "test_overrun_plugin";
        let component = ComponentType::NativePlugin(plugin_id.to_string());
        
        watchdog.register_component(component, WatchdogConfig::default());

        // Record multiple overruns
        for _ in 0..5 {
            watchdog.record_plugin_execution(plugin_id, Duration::from_millis(1), true);
        }

        let stats = watchdog.get_plugin_overrun_stats(plugin_id).expect("Should have stats");
        assert_eq!(stats.total_overruns, 5);
        assert_eq!(stats.recent_overruns, 5);
        assert!(stats.avg_execution_time.is_some());
        assert!(stats.max_execution_time.is_some());
    }

    #[test]
    fn test_wasm_vs_native_plugin_distinction() {
        let mut watchdog = WatchdogSystem::new();
        let native_plugin_id = "native_plugin";
        let wasm_plugin_id = "wasm_plugin";
        
        let native_component = ComponentType::NativePlugin(native_plugin_id.to_string());
        let wasm_component = ComponentType::WasmPlugin(wasm_plugin_id.to_string());
        
        watchdog.register_component(native_component.clone(), WatchdogConfig::default());
        watchdog.register_component(wasm_component.clone(), WatchdogConfig::default());

        // Trigger overruns for both
        watchdog.record_plugin_execution(native_plugin_id, Duration::from_millis(1), true);
        watchdog.record_plugin_execution(wasm_plugin_id, Duration::from_millis(1), false);

        // Both should have overrun events but different component types
        let events = watchdog.get_recent_events(Duration::from_secs(1));
        assert_eq!(events.len(), 2);
        
        let native_event = events.iter().find(|e| matches!(e.component, ComponentType::NativePlugin(_))).unwrap();
        let wasm_event = events.iter().find(|e| matches!(e.component, ComponentType::WasmPlugin(_))).unwrap();
        
        assert_eq!(native_event.event_type, WatchdogEventType::PluginOverrun);
        assert_eq!(wasm_event.event_type, WatchdogEventType::PluginOverrun);
    }

    #[test]
    fn test_plugin_overrun_quarantine_threshold() {
        let mut watchdog = WatchdogSystem::new();
        let plugin_id = "quarantine_test_plugin";
        let component = ComponentType::NativePlugin(plugin_id.to_string());
        
        let mut config = WatchdogConfig::default();
        config.max_consecutive_failures = 3; // Lower threshold for testing
        
        watchdog.register_component(component.clone(), config);

        // Generate consecutive overruns to trigger quarantine
        for i in 0..4 {
            let event = watchdog.record_plugin_execution(plugin_id, Duration::from_millis(1), true);
            if i < 2 {
                assert!(event.is_some());
                assert!(!watchdog.is_quarantined(&component));
            } else if i == 2 {
                // Should trigger quarantine on 3rd consecutive overrun
                assert!(event.is_some());
                assert!(watchdog.is_quarantined(&component));
            }
        }
    }
}

/// Test suite for NaN guard detection
mod nan_guard_tests {
    use super::*;

    #[test]
    fn test_nan_detection() {
        let mut watchdog = WatchdogSystem::new();
        let component = ComponentType::AxisNode("test_axis".to_string());
        
        let mut config = WatchdogConfig::default();
        config.enable_nan_guards = true;
        
        watchdog.register_component(component.clone(), config);

        // Normal value should not trigger guard
        assert!(watchdog.check_nan_guard(1.0, "normal_value", component.clone()).is_none());

        // NaN value should trigger guard
        let event = watchdog.check_nan_guard(f32::NAN, "nan_value", component.clone())
            .expect("Should detect NaN");
        assert_eq!(event.event_type, WatchdogEventType::NanDetected);
        assert!(event.context.contains("nan_value"));
        assert!(event.context.contains("NaN"));

        // Infinite value should also trigger guard
        let event = watchdog.check_nan_guard(f32::INFINITY, "infinite_value", component)
            .expect("Should detect infinity");
        assert_eq!(event.event_type, WatchdogEventType::NanDetected);
        assert!(event.context.contains("infinite_value"));
    }

    #[test]
    fn test_nan_guard_disabled() {
        let mut watchdog = WatchdogSystem::new();
        let component = ComponentType::AxisNode("test_axis".to_string());
        
        let mut config = WatchdogConfig::default();
        config.enable_nan_guards = false; // Disabled
        
        watchdog.register_component(component.clone(), config);

        // NaN should not trigger guard when disabled
        assert!(watchdog.check_nan_guard(f32::NAN, "nan_value", component).is_none());
    }

    #[test]
    fn test_critical_component_nan_response() {
        let mut watchdog = WatchdogSystem::new();
        let component = ComponentType::AxisNode("critical_axis".to_string());
        
        let mut config = WatchdogConfig::default();
        config.enable_nan_guards = true;
        config.is_critical = true; // Critical component
        
        watchdog.register_component(component.clone(), config);

        let event = watchdog.check_nan_guard(f32::NAN, "critical_nan", component)
            .expect("Should detect NaN in critical component");
        
        assert_eq!(event.event_type, WatchdogEventType::NanDetected);
        assert_eq!(event.action_taken, WatchdogAction::EmergencyStop); // Critical components trigger emergency stop
    }
}

/// Test suite for quarantine mechanism
mod quarantine_tests {
    use super::*;

    #[test]
    fn test_component_quarantine_isolation() {
        let mut watchdog = WatchdogSystem::new();
        let component = ComponentType::UsbEndpoint("quarantine_test".to_string());
        
        let mut config = WatchdogConfig::default();
        config.max_consecutive_failures = 2;
        
        watchdog.register_component(component.clone(), config);

        // Generate failures to trigger quarantine
        watchdog.record_usb_error("quarantine_test", "Error 1");
        assert!(!watchdog.is_quarantined(&component));
        
        watchdog.record_usb_error("quarantine_test", "Error 2");
        assert!(watchdog.is_quarantined(&component));

        // Verify quarantine status
        if let Some(QuarantineStatus::Quarantined { reason, failure_count, .. }) = 
            watchdog.get_quarantine_status(&component) {
            assert!(reason.contains("USB error"));
            assert_eq!(*failure_count, 2);
        } else {
            panic!("Component should be quarantined");
        }

        // Verify component is in quarantined list
        let quarantined = watchdog.get_quarantined_components();
        assert_eq!(quarantined.len(), 1);
        assert_eq!(quarantined[0], component);
    }

    #[test]
    fn test_quarantine_recovery_mechanism() {
        let mut watchdog = WatchdogSystem::new();
        let component = ComponentType::NativePlugin("recovery_test".to_string());
        
        watchdog.register_component(component.clone(), WatchdogConfig::default());
        
        // Manually quarantine component
        watchdog.quarantine_component(&component, "Test quarantine".to_string());
        assert!(watchdog.is_quarantined(&component));

        // Attempt recovery
        assert!(watchdog.attempt_recovery(&component));
        
        // Should be in recovery state
        if let Some(QuarantineStatus::Recovering { until, attempt_count }) = 
            watchdog.get_quarantine_status(&component) {
            assert!(*attempt_count == 1);
            assert!(*until > Instant::now());
        } else {
            panic!("Component should be in recovery state");
        }

        // Component should not be in quarantined list during recovery
        let quarantined = watchdog.get_quarantined_components();
        assert_eq!(quarantined.len(), 0);
    }

    #[test]
    fn test_quarantine_failure_rate_threshold() {
        let mut watchdog = WatchdogSystem::new();
        let endpoint_id = "rate_test_endpoint";
        let component = ComponentType::UsbEndpoint(endpoint_id.to_string());
        
        let mut config = WatchdogConfig::default();
        config.max_failures_per_window = 5;
        config.failure_rate_window = Duration::from_secs(10);
        
        watchdog.register_component(component.clone(), config);

        // Generate failures within the time window
        for i in 0..6 {
            watchdog.record_usb_error(endpoint_id, &format!("Rate test error {}", i));
        }

        // Should be quarantined due to high failure rate
        assert!(watchdog.is_quarantined(&component));
    }

    #[test]
    fn test_multiple_component_quarantine() {
        let mut watchdog = WatchdogSystem::new();
        
        let usb_component = ComponentType::UsbEndpoint("usb_test".to_string());
        let plugin_component = ComponentType::NativePlugin("plugin_test".to_string());
        
        let mut config = WatchdogConfig::default();
        config.max_consecutive_failures = 1; // Immediate quarantine for testing
        
        watchdog.register_component(usb_component.clone(), config.clone());
        watchdog.register_component(plugin_component.clone(), config);

        // Trigger quarantine for both components
        watchdog.record_usb_error("usb_test", "USB failure");
        watchdog.record_plugin_execution("plugin_test", Duration::from_millis(1), true);

        // Both should be quarantined
        assert!(watchdog.is_quarantined(&usb_component));
        assert!(watchdog.is_quarantined(&plugin_component));

        let quarantined = watchdog.get_quarantined_components();
        assert_eq!(quarantined.len(), 2);
    }
}

/// Test suite for synthetic fault injection
mod fault_injection_tests {
    use super::*;

    #[test]
    fn test_fault_injection_enable_disable() {
        let mut watchdog = WatchdogSystem::new();
        
        // Initially disabled
        let summary = watchdog.get_health_summary();
        assert!(!summary.fault_injection_enabled);

        // Enable injection
        watchdog.enable_fault_injection();
        let summary = watchdog.get_health_summary();
        assert!(summary.fault_injection_enabled);

        // Disable injection
        watchdog.disable_fault_injection();
        let summary = watchdog.get_health_summary();
        assert!(!summary.fault_injection_enabled);
    }

    #[test]
    fn test_synthetic_plugin_overrun_injection() {
        let mut watchdog = WatchdogSystem::new();
        let plugin_id = "injection_test_plugin";
        let component = ComponentType::NativePlugin(plugin_id.to_string());
        
        watchdog.register_component(component.clone(), WatchdogConfig::default());
        watchdog.enable_fault_injection();

        let fault = SyntheticFault {
            component: component.clone(),
            fault_type: WatchdogEventType::PluginOverrun,
            inject_at: Instant::now(),
            context: "Synthetic overrun test".to_string(),
        };

        watchdog.inject_synthetic_fault(fault);
        let events = watchdog.process_synthetic_faults();
        
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, WatchdogEventType::SyntheticFault);
        assert_eq!(events[0].component, component);
        assert!(events[0].context.contains("Synthetic overrun test"));

        // Should also trigger actual overrun detection
        let all_events = watchdog.get_recent_events(Duration::from_secs(1));
        let overrun_events: Vec<_> = all_events.iter()
            .filter(|e| e.event_type == WatchdogEventType::PluginOverrun)
            .collect();
        assert!(!overrun_events.is_empty());
    }

    #[test]
    fn test_synthetic_usb_timeout_injection() {
        let mut watchdog = WatchdogSystem::new();
        let endpoint_id = "injection_usb_test";
        let component = ComponentType::UsbEndpoint(endpoint_id.to_string());
        
        watchdog.register_component(component.clone(), WatchdogConfig::default());
        watchdog.enable_fault_injection();

        let fault = SyntheticFault {
            component: component.clone(),
            fault_type: WatchdogEventType::UsbTimeout,
            inject_at: Instant::now(),
            context: "Synthetic USB timeout test".to_string(),
        };

        watchdog.inject_synthetic_fault(fault);
        let events = watchdog.process_synthetic_faults();
        
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, WatchdogEventType::SyntheticFault);

        // Should also trigger actual USB error
        let all_events = watchdog.get_recent_events(Duration::from_secs(1));
        let usb_events: Vec<_> = all_events.iter()
            .filter(|e| e.event_type == WatchdogEventType::UsbError)
            .collect();
        assert!(!usb_events.is_empty());
    }

    #[test]
    fn test_synthetic_nan_injection() {
        let mut watchdog = WatchdogSystem::new();
        let component = ComponentType::AxisNode("injection_axis_test".to_string());
        
        let mut config = WatchdogConfig::default();
        config.enable_nan_guards = true;
        
        watchdog.register_component(component.clone(), config);
        watchdog.enable_fault_injection();

        let fault = SyntheticFault {
            component: component.clone(),
            fault_type: WatchdogEventType::NanDetected,
            inject_at: Instant::now(),
            context: "Synthetic NaN test".to_string(),
        };

        watchdog.inject_synthetic_fault(fault);
        let events = watchdog.process_synthetic_faults();
        
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, WatchdogEventType::SyntheticFault);

        // Should also trigger actual NaN detection
        let all_events = watchdog.get_recent_events(Duration::from_secs(1));
        let nan_events: Vec<_> = all_events.iter()
            .filter(|e| e.event_type == WatchdogEventType::NanDetected)
            .collect();
        assert!(!nan_events.is_empty());
    }

    #[test]
    fn test_delayed_fault_injection() {
        let mut watchdog = WatchdogSystem::new();
        let component = ComponentType::NativePlugin("delayed_test".to_string());
        
        watchdog.register_component(component.clone(), WatchdogConfig::default());
        watchdog.enable_fault_injection();

        let fault = SyntheticFault {
            component: component.clone(),
            fault_type: WatchdogEventType::PluginOverrun,
            inject_at: Instant::now() + Duration::from_millis(50), // Delayed injection
            context: "Delayed injection test".to_string(),
        };

        watchdog.inject_synthetic_fault(fault);
        
        // Should not trigger immediately
        let events = watchdog.process_synthetic_faults();
        assert_eq!(events.len(), 0);

        // Wait for injection time
        thread::sleep(Duration::from_millis(60));
        
        // Should trigger now
        let events = watchdog.process_synthetic_faults();
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn test_fault_injection_when_disabled() {
        let mut watchdog = WatchdogSystem::new();
        let component = ComponentType::NativePlugin("disabled_test".to_string());
        
        watchdog.register_component(component.clone(), WatchdogConfig::default());
        // Don't enable fault injection

        let fault = SyntheticFault {
            component: component.clone(),
            fault_type: WatchdogEventType::PluginOverrun,
            inject_at: Instant::now(),
            context: "Should not inject".to_string(),
        };

        watchdog.inject_synthetic_fault(fault);
        let events = watchdog.process_synthetic_faults();
        
        // Should not inject when disabled
        assert_eq!(events.len(), 0);
    }
}

/// Test suite for system health and statistics
mod health_tests {
    use super::*;

    #[test]
    fn test_health_summary() {
        let mut watchdog = WatchdogSystem::new();
        
        // Register components
        watchdog.register_component(ComponentType::UsbEndpoint("ep1".to_string()), WatchdogConfig::default());
        watchdog.register_component(ComponentType::NativePlugin("plugin1".to_string()), WatchdogConfig::default());
        watchdog.register_component(ComponentType::WasmPlugin("wasm1".to_string()), WatchdogConfig::default());
        
        // Quarantine one component
        let component = ComponentType::UsbEndpoint("ep1".to_string());
        watchdog.quarantine_component(&component, "Test quarantine".to_string());

        // Register an axis component for NaN testing
        let axis_component = ComponentType::AxisNode("axis1".to_string());
        let mut axis_config = WatchdogConfig::default();
        axis_config.enable_nan_guards = true;
        watchdog.register_component(axis_component.clone(), axis_config);

        // Generate some recent events
        watchdog.record_plugin_execution("plugin1", Duration::from_millis(1), true); // Overrun
        watchdog.record_usb_error("ep2", "Test error"); // USB error
        watchdog.check_nan_guard(f32::NAN, "test", axis_component); // NaN

        let summary = watchdog.get_health_summary();
        
        assert_eq!(summary.total_components, 4);
        assert_eq!(summary.active_components, 3);
        assert_eq!(summary.quarantined_components, 1);
        assert_eq!(summary.recent_overruns, 1);
        assert_eq!(summary.recent_usb_errors, 1);
        assert_eq!(summary.recent_nan_detections, 1);
        assert!(!summary.fault_injection_enabled);
    }

    #[test]
    fn test_plugin_overrun_statistics() {
        let mut watchdog = WatchdogSystem::new();
        let plugin_id = "stats_test_plugin";
        let component = ComponentType::NativePlugin(plugin_id.to_string());
        
        watchdog.register_component(component, WatchdogConfig::default());

        // Record various execution times
        let execution_times = [
            Duration::from_micros(50),  // Normal
            Duration::from_micros(75),  // Normal
            Duration::from_millis(1),   // Overrun
            Duration::from_micros(60),  // Normal
            Duration::from_millis(2),   // Overrun
        ];

        for time in &execution_times {
            watchdog.record_plugin_execution(plugin_id, *time, true);
        }

        let stats = watchdog.get_plugin_overrun_stats(plugin_id).unwrap();
        
        assert_eq!(stats.total_overruns, 2);
        assert_eq!(stats.recent_executions, 5);
        assert_eq!(stats.recent_overruns, 2);
        assert!(stats.avg_execution_time.is_some());
        assert_eq!(stats.max_execution_time, Some(Duration::from_millis(2)));
        assert!(stats.last_execution.is_some());
    }

    #[test]
    fn test_recent_events_filtering() {
        let mut watchdog = WatchdogSystem::new();
        let component = ComponentType::NativePlugin("event_test".to_string());
        
        watchdog.register_component(component.clone(), WatchdogConfig::default());

        // Record an event
        watchdog.record_plugin_execution("event_test", Duration::from_millis(1), true);
        
        // Should be in recent events
        let recent = watchdog.get_recent_events(Duration::from_secs(1));
        assert!(!recent.is_empty());
        
        // Wait and check again with very short window
        thread::sleep(Duration::from_millis(10));
        let very_recent = watchdog.get_recent_events(Duration::from_millis(5));
        assert!(very_recent.is_empty());
    }

    #[test]
    fn test_clear_all_state() {
        let mut watchdog = WatchdogSystem::new();
        
        // Set up some state
        watchdog.register_component(ComponentType::UsbEndpoint("test".to_string()), WatchdogConfig::default());
        watchdog.record_usb_error("test", "Error");
        watchdog.enable_fault_injection();
        
        // Verify state exists
        assert!(!watchdog.get_all_events().is_empty());
        assert!(watchdog.get_health_summary().fault_injection_enabled);
        
        // Clear all state
        watchdog.clear_all_state();
        
        // Verify state is cleared
        assert!(watchdog.get_all_events().is_empty());
        assert_eq!(watchdog.get_quarantined_components().len(), 0);
        
        let summary = watchdog.get_health_summary();
        assert_eq!(summary.active_components, 0);
        assert_eq!(summary.quarantined_components, 0);
    }
}

/// Integration tests that validate end-to-end watchdog behavior
mod integration_tests {
    use super::*;

    #[test]
    fn test_complete_fault_lifecycle() {
        let mut watchdog = WatchdogSystem::new();
        let plugin_id = "lifecycle_test_plugin";
        let component = ComponentType::NativePlugin(plugin_id.to_string());
        
        let mut config = WatchdogConfig::default();
        config.max_consecutive_failures = 3;
        
        watchdog.register_component(component.clone(), config);

        // Phase 1: Normal operation
        watchdog.record_plugin_execution(plugin_id, Duration::from_micros(50), true);
        assert!(!watchdog.is_quarantined(&component));

        // Phase 2: Intermittent failures
        watchdog.record_plugin_execution(plugin_id, Duration::from_millis(1), true); // Overrun
        watchdog.record_plugin_execution(plugin_id, Duration::from_micros(60), true); // Normal
        assert!(!watchdog.is_quarantined(&component));

        // Phase 3: Consecutive failures leading to quarantine
        watchdog.record_plugin_execution(plugin_id, Duration::from_millis(1), true); // Overrun
        watchdog.record_plugin_execution(plugin_id, Duration::from_millis(1), true); // Overrun
        watchdog.record_plugin_execution(plugin_id, Duration::from_millis(1), true); // Overrun - should quarantine
        assert!(watchdog.is_quarantined(&component));

        // Phase 4: Recovery attempt
        assert!(watchdog.attempt_recovery(&component));
        
        // Should be in recovery state
        if let Some(QuarantineStatus::Recovering { .. }) = watchdog.get_quarantine_status(&component) {
            // Expected
        } else {
            panic!("Should be in recovery state");
        }

        // Phase 5: Successful recovery (simulated by time passage)
        // In real implementation, this would happen after recovery timeout
        // For testing, we'll manually transition to active
        watchdog.quarantine_status.insert(component.clone(), QuarantineStatus::Active);
        
        // Phase 6: Normal operation after recovery
        watchdog.record_plugin_execution(plugin_id, Duration::from_micros(50), true);
        assert!(!watchdog.is_quarantined(&component));
    }

    #[test]
    fn test_multi_component_fault_storm() {
        let mut watchdog = WatchdogSystem::new();
        
        // Register multiple components
        let components = vec![
            ComponentType::UsbEndpoint("usb1".to_string()),
            ComponentType::UsbEndpoint("usb2".to_string()),
            ComponentType::NativePlugin("plugin1".to_string()),
            ComponentType::NativePlugin("plugin2".to_string()),
        ];

        for component in &components {
            watchdog.register_component(component.clone(), WatchdogConfig::default());
        }

        // Generate fault storm across multiple components
        for i in 0..15 {
            match i % 4 {
                0 => { watchdog.record_usb_error("usb1", "Storm error"); },
                1 => { watchdog.record_usb_error("usb2", "Storm error"); },
                2 => { watchdog.record_plugin_execution("plugin1", Duration::from_millis(1), true); },
                3 => { watchdog.record_plugin_execution("plugin2", Duration::from_millis(1), true); },
                _ => unreachable!(),
            }
        }

        // Should detect fault storm
        assert!(watchdog.is_in_fault_storm());
        
        let summary = watchdog.get_health_summary();
        assert!(summary.recent_overruns > 0);
        assert!(summary.recent_usb_errors > 0);
    }

    #[test]
    fn test_watchdog_with_synthetic_faults() {
        let mut watchdog = WatchdogSystem::new();
        let plugin_id = "synthetic_integration_test";
        let component = ComponentType::NativePlugin(plugin_id.to_string());
        
        watchdog.register_component(component.clone(), WatchdogConfig::default());
        watchdog.enable_fault_injection();

        // Inject multiple synthetic faults
        let faults = vec![
            SyntheticFault {
                component: component.clone(),
                fault_type: WatchdogEventType::PluginOverrun,
                inject_at: Instant::now(),
                context: "First synthetic fault".to_string(),
            },
            SyntheticFault {
                component: component.clone(),
                fault_type: WatchdogEventType::PluginOverrun,
                inject_at: Instant::now() + Duration::from_millis(10),
                context: "Second synthetic fault".to_string(),
            },
        ];

        for fault in faults {
            watchdog.inject_synthetic_fault(fault);
        }

        // Process immediate faults
        let events = watchdog.process_synthetic_faults();
        assert_eq!(events.len(), 1); // Only first fault should be processed

        // Wait and process delayed faults
        thread::sleep(Duration::from_millis(15));
        let events = watchdog.process_synthetic_faults();
        assert_eq!(events.len(), 1); // Second fault should be processed

        // Verify both synthetic and real events were generated
        let all_events = watchdog.get_recent_events(Duration::from_secs(1));
        let synthetic_events: Vec<_> = all_events.iter()
            .filter(|e| e.event_type == WatchdogEventType::SyntheticFault)
            .collect();
        let overrun_events: Vec<_> = all_events.iter()
            .filter(|e| e.event_type == WatchdogEventType::PluginOverrun)
            .collect();
        
        assert_eq!(synthetic_events.len(), 2);
        assert!(!overrun_events.is_empty()); // Should have triggered real overruns
    }
}