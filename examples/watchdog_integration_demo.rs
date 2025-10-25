// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Watchdog and Quarantine System Integration Demo
//!
//! Demonstrates the complete watchdog system including:
//! - USB stall timeout detection
//! - Plugin overrun monitoring
//! - Component quarantine and recovery
//! - Synthetic fault injection for testing
//! - Integration with HID adapter

#![cfg_attr(not(feature = "flight-hid"), allow(dead_code, unused_imports))]

#[cfg(feature = "flight-hid")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use flight_core::{
        WatchdogSystem, WatchdogConfig, ComponentType, WatchdogEventType, SyntheticFault,
        WatchdogHealthSummary, PluginOverrunStats
    };
    use flight_hid::{HidAdapter, HidDeviceInfo, EndpointType};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};
    use std::thread;
    use tracing::{info, warn, error};
    // Initialize tracing
    tracing_subscriber::fmt::init();

    info!("Starting Flight Hub Watchdog Integration Demo");

    // Create shared watchdog system
    let watchdog = Arc::new(Mutex::new(WatchdogSystem::new()));
    
    // Create HID adapter with watchdog integration
    let mut hid_adapter = HidAdapter::new(watchdog.clone());
    
    // Start the HID adapter
    hid_adapter.start()?;
    
    // Demo 1: Basic watchdog functionality
    demo_basic_watchdog_functionality(&watchdog)?;
    
    // Demo 2: USB endpoint monitoring
    demo_usb_endpoint_monitoring(&mut hid_adapter, &watchdog)?;
    
    // Demo 3: Plugin overrun detection
    demo_plugin_overrun_detection(&watchdog)?;
    
    // Demo 4: Component quarantine and recovery
    demo_quarantine_and_recovery(&watchdog)?;
    
    // Demo 5: Synthetic fault injection
    demo_synthetic_fault_injection(&watchdog)?;
    
    // Demo 6: Health monitoring and statistics
    demo_health_monitoring(&watchdog)?;
    
    // Demo 7: Fault storm detection
    demo_fault_storm_detection(&watchdog)?;
    
    // Stop the HID adapter
    hid_adapter.stop();
    
    info!("Watchdog Integration Demo completed successfully");
    Ok(())
}

#[cfg(feature = "flight-hid")]
fn demo_basic_watchdog_functionality(watchdog: &Arc<Mutex<WatchdogSystem>>) -> Result<(), Box<dyn std::error::Error>> {
    use flight_core::{WatchdogSystem, WatchdogConfig, ComponentType};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use std::thread;
    use tracing::info;
    info!("=== Demo 1: Basic Watchdog Functionality ===");
    
    let mut wd = watchdog.lock().unwrap();
    
    // Register various component types
    let components = vec![
        (ComponentType::UsbEndpoint("demo_usb".to_string()), "USB Endpoint"),
        (ComponentType::NativePlugin("demo_native".to_string()), "Native Plugin"),
        (ComponentType::WasmPlugin("demo_wasm".to_string()), "WASM Plugin"),
        (ComponentType::AxisNode("demo_axis".to_string()), "Axis Node"),
    ];
    
    for (component, name) in &components {
        let mut config = WatchdogConfig::default();
        
        // Customize config based on component type
        match component {
            ComponentType::UsbEndpoint(_) => {
                config.usb_timeout = Duration::from_millis(100);
                config.is_critical = true;
            }
            ComponentType::NativePlugin(_) | ComponentType::WasmPlugin(_) => {
                config.max_execution_time = Duration::from_micros(100);
                config.max_consecutive_failures = 3;
            }
            ComponentType::AxisNode(_) => {
                config.enable_nan_guards = true;
                config.is_critical = true;
            }
            _ => {}
        }
        
        wd.register_component(component.clone(), config);
        info!("Registered {} for monitoring", name);
    }
    
    // Display initial health summary
    let summary = wd.get_health_summary();
    info!("Initial health: {} total components, {} active", 
          summary.total_components, summary.active_components);
    
    drop(wd);
    thread::sleep(Duration::from_millis(100));
    Ok(())
}

#[cfg(feature = "flight-hid")]
fn demo_usb_endpoint_monitoring(hid_adapter: &mut HidAdapter, watchdog: &Arc<Mutex<WatchdogSystem>>) -> Result<(), Box<dyn std::error::Error>> {
    use flight_hid::{HidAdapter, HidDeviceInfo};
    use flight_core::WatchdogSystem;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use std::thread;
    use tracing::{info, warn};
    info!("=== Demo 2: USB Endpoint Monitoring ===");
    
    // Register a test device
    let device_info = HidDeviceInfo {
        vendor_id: 0x046d,
        product_id: 0xc262,
        serial_number: Some("DEMO123".to_string()),
        manufacturer: Some("Demo Manufacturer".to_string()),
        product_name: Some("Demo Flight Controller".to_string()),
        device_path: "/dev/demo_hid".to_string(),
        usage_page: 0x01,
        usage: 0x04,
    };
    
    hid_adapter.register_device(device_info.clone())?;
    info!("Registered demo HID device: {}", device_info.product_name.as_ref().unwrap());
    
    // Simulate normal operations
    info!("Simulating normal USB operations...");
    for i in 0..10 {
        let mut buffer = [0u8; 64];
        let result = hid_adapter.read_input(&device_info.device_path, &mut buffer)?;
        
        let data = [i as u8, (i * 2) as u8, (i * 3) as u8];
        let result = hid_adapter.write_output(&device_info.device_path, &data)?;
        
        thread::sleep(Duration::from_millis(10));
    }
    
    // Check for any USB events
    let events = hid_adapter.check_endpoint_health(&device_info.device_path)?;
    if events.is_empty() {
        info!("No USB issues detected during normal operations");
    } else {
        warn!("USB events detected: {} events", events.len());
        for event in &events {
            warn!("  Event: {:?} - {}", event.event_type, event.context);
        }
    }
    
    // Display adapter statistics
    let stats = hid_adapter.get_statistics();
    info!("HID Adapter Stats: {} devices, {} endpoints, {} operations, {} bytes",
          stats.total_devices, stats.total_endpoints, stats.total_operations, stats.total_bytes);
    
    thread::sleep(Duration::from_millis(100));
    Ok(())
}

#[cfg(feature = "flight-hid")]
fn demo_plugin_overrun_detection(watchdog: &Arc<Mutex<WatchdogSystem>>) -> Result<(), Box<dyn std::error::Error>> {
    use flight_core::{WatchdogSystem, WatchdogConfig, ComponentType};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use std::thread;
    use tracing::{info, warn};
    info!("=== Demo 3: Plugin Overrun Detection ===");
    
    let plugin_id = "demo_overrun_plugin";
    let component = ComponentType::NativePlugin(plugin_id.to_string());
    
    {
        let mut wd = watchdog.lock().unwrap();
        wd.register_component(component.clone(), WatchdogConfig::default());
    }
    
    info!("Simulating plugin executions with varying performance...");
    
    // Simulate normal plugin executions
    for i in 0..5 {
        let execution_time = Duration::from_micros(50 + i * 10); // Normal execution times
        
        let mut wd = watchdog.lock().unwrap();
        let event = wd.record_plugin_execution(plugin_id, execution_time, true);
        drop(wd);
        
        if let Some(event) = event {
            warn!("Plugin event: {:?}", event.event_type);
        } else {
            info!("Plugin execution {} completed normally in {:?}", i + 1, execution_time);
        }
        
        thread::sleep(Duration::from_millis(20));
    }
    
    // Simulate plugin overruns
    info!("Simulating plugin overruns...");
    for i in 0..3 {
        let execution_time = Duration::from_millis(1 + i); // Excessive execution times
        
        let mut wd = watchdog.lock().unwrap();
        let event = wd.record_plugin_execution(plugin_id, execution_time, true);
        drop(wd);
        
        if let Some(event) = event {
            warn!("Plugin overrun detected: {:?} - {}", event.event_type, event.context);
        }
        
        thread::sleep(Duration::from_millis(50));
    }
    
    // Display plugin statistics
    {
        let wd = watchdog.lock().unwrap();
        if let Some(stats) = wd.get_plugin_overrun_stats(plugin_id) {
            info!("Plugin Stats: {} total overruns, {} recent executions, avg time: {:?}",
                  stats.total_overruns, stats.recent_executions, stats.avg_execution_time);
        }
    }
    
    thread::sleep(Duration::from_millis(100));
    Ok(())
}

#[cfg(feature = "flight-hid")]
fn demo_quarantine_and_recovery(watchdog: &Arc<Mutex<WatchdogSystem>>) -> Result<(), Box<dyn std::error::Error>> {
    use flight_core::{WatchdogSystem, WatchdogConfig, ComponentType};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use std::thread;
    use tracing::{info, warn};
    info!("=== Demo 4: Component Quarantine and Recovery ===");
    
    let endpoint_id = "demo_quarantine_endpoint";
    let component = ComponentType::UsbEndpoint(endpoint_id.to_string());
    
    {
        let mut wd = watchdog.lock().unwrap();
        let mut config = WatchdogConfig::default();
        config.max_consecutive_failures = 3; // Lower threshold for demo
        wd.register_component(component.clone(), config);
    }
    
    info!("Generating consecutive failures to trigger quarantine...");
    
    // Generate failures to trigger quarantine
    for i in 1..=4 {
        let mut wd = watchdog.lock().unwrap();
        let event = wd.record_usb_error(endpoint_id, &format!("Demo error {}", i));
        let is_quarantined = wd.is_quarantined(&component);
        drop(wd);
        
        info!("Failure {}: {} - Quarantined: {}", i, event.context, is_quarantined);
        
        if is_quarantined {
            warn!("Component {} has been quarantined!", component.display_name());
            break;
        }
        
        thread::sleep(Duration::from_millis(100));
    }
    
    // Display quarantine status
    {
        let wd = watchdog.lock().unwrap();
        let quarantined = wd.get_quarantined_components();
        info!("Currently quarantined components: {}", quarantined.len());
        
        for comp in &quarantined {
            if let Some(status) = wd.get_quarantine_status(comp) {
                info!("  {}: {:?}", comp.display_name(), status);
            }
        }
    }
    
    // Attempt recovery
    info!("Attempting component recovery...");
    {
        let mut wd = watchdog.lock().unwrap();
        let recovery_success = wd.attempt_recovery(&component);
        info!("Recovery attempt result: {}", recovery_success);
        
        if let Some(status) = wd.get_quarantine_status(&component) {
            info!("New status: {:?}", status);
        }
    }
    
    thread::sleep(Duration::from_millis(100));
    Ok(())
}

#[cfg(feature = "flight-hid")]
fn demo_synthetic_fault_injection(watchdog: &Arc<Mutex<WatchdogSystem>>) -> Result<(), Box<dyn std::error::Error>> {
    use flight_core::{WatchdogSystem, WatchdogConfig, ComponentType, WatchdogEventType, SyntheticFault};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};
    use std::thread;
    use tracing::{info, warn};
    info!("=== Demo 5: Synthetic Fault Injection ===");
    
    let plugin_id = "demo_injection_plugin";
    let component = ComponentType::NativePlugin(plugin_id.to_string());
    
    {
        let mut wd = watchdog.lock().unwrap();
        wd.register_component(component.clone(), WatchdogConfig::default());
        wd.enable_fault_injection();
        info!("Fault injection enabled for testing");
    }
    
    // Create synthetic faults
    let faults = vec![
        SyntheticFault {
            component: component.clone(),
            fault_type: WatchdogEventType::PluginOverrun,
            inject_at: Instant::now() + Duration::from_millis(100),
            context: "Synthetic overrun test #1".to_string(),
        },
        SyntheticFault {
            component: ComponentType::UsbEndpoint("synthetic_usb".to_string()),
            fault_type: WatchdogEventType::UsbTimeout,
            inject_at: Instant::now() + Duration::from_millis(200),
            context: "Synthetic USB timeout test".to_string(),
        },
        SyntheticFault {
            component: ComponentType::AxisNode("synthetic_axis".to_string()),
            fault_type: WatchdogEventType::NanDetected,
            inject_at: Instant::now() + Duration::from_millis(300),
            context: "Synthetic NaN test".to_string(),
        },
    ];
    
    // Inject faults
    {
        let mut wd = watchdog.lock().unwrap();
        for fault in faults {
            wd.inject_synthetic_fault(fault);
            info!("Queued synthetic fault for injection");
        }
    }
    
    // Process faults over time
    info!("Processing synthetic faults...");
    for i in 0..5 {
        thread::sleep(Duration::from_millis(100));
        
        let mut wd = watchdog.lock().unwrap();
        let events = wd.process_synthetic_faults();
        drop(wd);
        
        if !events.is_empty() {
            info!("Processed {} synthetic fault events at iteration {}", events.len(), i + 1);
            for event in &events {
                warn!("  Synthetic fault: {:?} - {}", event.event_type, event.context);
            }
        }
    }
    
    // Display all recent events
    {
        let mut wd = watchdog.lock().unwrap();
        let recent_events = wd.get_recent_events(Duration::from_secs(1));
        info!("Total recent events: {}", recent_events.len());
        
        let synthetic_count = recent_events.iter()
            .filter(|e| e.event_type == WatchdogEventType::SyntheticFault)
            .count();
        info!("Synthetic fault events: {}", synthetic_count);
        
        wd.disable_fault_injection();
        info!("Fault injection disabled");
    }
    
    thread::sleep(Duration::from_millis(100));
    Ok(())
}

#[cfg(feature = "flight-hid")]
fn demo_health_monitoring(watchdog: &Arc<Mutex<WatchdogSystem>>) -> Result<(), Box<dyn std::error::Error>> {
    use flight_core::WatchdogSystem;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use std::thread;
    use tracing::info;
    info!("=== Demo 6: Health Monitoring and Statistics ===");
    
    // Get comprehensive health summary
    let summary = {
        let wd = watchdog.lock().unwrap();
        wd.get_health_summary()
    };
    
    info!("=== System Health Summary ===");
    info!("Total components: {}", summary.total_components);
    info!("Active components: {}", summary.active_components);
    info!("Quarantined components: {}", summary.quarantined_components);
    info!("Recent overruns (5min): {}", summary.recent_overruns);
    info!("Recent USB errors (5min): {}", summary.recent_usb_errors);
    info!("Recent NaN detections (5min): {}", summary.recent_nan_detections);
    info!("Fault injection enabled: {}", summary.fault_injection_enabled);
    
    // Display detailed event history
    {
        let wd = watchdog.lock().unwrap();
        let all_events = wd.get_all_events();
        
        info!("=== Event History (last {} events) ===", all_events.len().min(10));
        for (i, event) in all_events.iter().rev().take(10).enumerate() {
            info!("  {}: {:?} on {} - {} (Action: {:?})",
                  i + 1,
                  event.event_type,
                  event.component.display_name(),
                  event.context,
                  event.action_taken);
        }
    }
    
    thread::sleep(Duration::from_millis(100));
    Ok(())
}

#[cfg(feature = "flight-hid")]
fn demo_fault_storm_detection(watchdog: &Arc<Mutex<WatchdogSystem>>) -> Result<(), Box<dyn std::error::Error>> {
    use flight_core::{WatchdogSystem, WatchdogConfig, ComponentType};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use std::thread;
    use tracing::{info, warn};
    info!("=== Demo 7: Fault Storm Detection ===");
    
    // Register multiple components for storm simulation
    let components = vec![
        ComponentType::UsbEndpoint("storm_usb1".to_string()),
        ComponentType::UsbEndpoint("storm_usb2".to_string()),
        ComponentType::NativePlugin("storm_plugin1".to_string()),
        ComponentType::NativePlugin("storm_plugin2".to_string()),
    ];
    
    {
        let mut wd = watchdog.lock().unwrap();
        for component in &components {
            wd.register_component(component.clone(), WatchdogConfig::default());
        }
    }
    
    info!("Simulating fault storm across multiple components...");
    
    // Generate rapid faults across multiple components
    for i in 0..20 {
        let mut wd = watchdog.lock().unwrap();
        
        match i % 4 {
            0 => {
                wd.record_usb_error("storm_usb1", &format!("Storm error {}", i));
            }
            1 => {
                wd.record_usb_error("storm_usb2", &format!("Storm error {}", i));
            }
            2 => {
                wd.record_plugin_execution("storm_plugin1", Duration::from_millis(1), true);
            }
            3 => {
                wd.record_plugin_execution("storm_plugin2", Duration::from_millis(1), true);
            }
            _ => unreachable!(),
        }
        
        let is_storm = wd.is_in_fault_storm();
        drop(wd);
        
        if is_storm && i > 10 {
            warn!("FAULT STORM DETECTED after {} faults!", i + 1);
            break;
        }
        
        thread::sleep(Duration::from_millis(50));
    }
    
    // Display final health summary
    let summary = {
        let wd = watchdog.lock().unwrap();
        wd.get_health_summary()
    };
    
    info!("=== Post-Storm Health Summary ===");
    info!("Active components: {}", summary.active_components);
    info!("Quarantined components: {}", summary.quarantined_components);
    info!("Recent overruns: {}", summary.recent_overruns);
    info!("Recent USB errors: {}", summary.recent_usb_errors);
    
    // Clear state for clean demo completion
    {
        let mut wd = watchdog.lock().unwrap();
        wd.clear_all_state();
        info!("Cleared all watchdog state for demo completion");
    }
    
    Ok(())
}

#[cfg(not(feature = "flight-hid"))]
fn main() {
    eprintln!("Enable `--features flight-hid` to build this example.");
}