// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Capability enforcement demonstration
//!
//! This example demonstrates the kid/demo mode capability enforcement system.

use flight_core::profile::{Profile, AxisConfig, AircraftId, CapabilityMode, CapabilityContext};
use flight_service::capability_service::CapabilityService;
use flight_axis::AxisEngine;
use std::collections::HashMap;
use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("Flight Hub Capability Enforcement Demo");
    println!("=====================================\n");

    // Create a capability service
    let service = CapabilityService::new();
    
    // Create and register axis engines
    let pitch_engine = Arc::new(AxisEngine::new_for_axis("pitch".to_string()));
    let roll_engine = Arc::new(AxisEngine::new_for_axis("roll".to_string()));
    
    service.register_axis("pitch".to_string(), pitch_engine.clone())?;
    service.register_axis("roll".to_string(), roll_engine.clone())?;
    
    println!("✓ Registered pitch and roll axes");

    // Demonstrate profile validation with capability enforcement
    demonstrate_profile_validation()?;
    
    // Demonstrate engine output clamping
    demonstrate_output_clamping(&pitch_engine)?;
    
    // Demonstrate IPC-like service operations
    demonstrate_service_operations(&service)?;

    println!("\n🎉 Capability enforcement demo completed successfully!");
    Ok(())
}

fn demonstrate_profile_validation() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n1. Profile Validation with Capability Enforcement");
    println!("------------------------------------------------");

    // Create a profile with aggressive settings
    let mut axes = HashMap::new();
    axes.insert("pitch".to_string(), AxisConfig {
        deadzone: Some(0.03),
        expo: Some(0.8),      // High expo
        slew_rate: Some(10.0), // High slew rate
        detents: vec![],
        curve: Some(vec![
            flight_core::profile::CurvePoint { input: 0.0, output: 0.0 },
            flight_core::profile::CurvePoint { input: 1.0, output: 1.0 },
        ]), // Custom curve
    });

    let profile = Profile {
        schema: "flight.profile/1".to_string(),
        sim: Some("msfs".to_string()),
        aircraft: Some(AircraftId { icao: "C172".to_string() }),
        axes,
        pof_overrides: None,
    };

    // Test in full mode
    let full_context = CapabilityContext::for_mode(CapabilityMode::Full);
    match profile.validate_with_capabilities(&full_context) {
        Ok(_) => println!("  ✓ Profile accepted in FULL mode"),
        Err(e) => println!("  ✗ Profile rejected in FULL mode: {}", e),
    }

    // Test in demo mode
    let demo_context = CapabilityContext::for_mode(CapabilityMode::Demo);
    match profile.validate_with_capabilities(&demo_context) {
        Ok(_) => println!("  ✓ Profile accepted in DEMO mode"),
        Err(e) => println!("  ✗ Profile rejected in DEMO mode: {}", e),
    }

    // Test in kid mode
    let kid_context = CapabilityContext::for_mode(CapabilityMode::Kid);
    match profile.validate_with_capabilities(&kid_context) {
        Ok(_) => println!("  ✓ Profile accepted in KID mode"),
        Err(e) => println!("  ✗ Profile rejected in KID mode: {}", e),
    }

    Ok(())
}

fn demonstrate_output_clamping(engine: &AxisEngine) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n2. Engine Output Clamping");
    println!("-------------------------");

    let test_input = 0.9;
    
    // Test in full mode
    engine.set_capability_mode(CapabilityMode::Full);
    let mut frame = flight_axis::AxisFrame::new(test_input, 1000);
    frame.out = test_input;
    engine.process(&mut frame)?;
    println!("  FULL mode: {:.1} → {:.1} (no clamping)", test_input, frame.out);

    // Test in demo mode
    engine.set_capability_mode(CapabilityMode::Demo);
    let mut frame = flight_axis::AxisFrame::new(test_input, 2000);
    frame.out = test_input;
    engine.process(&mut frame)?;
    println!("  DEMO mode: {:.1} → {:.1} (clamped to 80%)", test_input, frame.out);

    // Test in kid mode
    engine.set_capability_mode(CapabilityMode::Kid);
    let mut frame = flight_axis::AxisFrame::new(test_input, 3000);
    frame.out = test_input;
    engine.process(&mut frame)?;
    println!("  KID mode:  {:.1} → {:.1} (clamped to 50%)", test_input, frame.out);

    Ok(())
}

fn demonstrate_service_operations(service: &CapabilityService) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n3. Service Operations (IPC Simulation)");
    println!("--------------------------------------");

    // Get initial status
    let status = service.get_capability_status(None)?;
    println!("  Initial status: {} axes in FULL mode", status.len());

    // Set global kid mode
    let result = service.set_kid_mode(true)?;
    println!("  ✓ Set KID mode globally: {} axes affected", result.affected_axes.len());
    println!("    Max axis output: {:.1}%", result.applied_limits.max_axis_output * 100.0);
    println!("    Max FFB torque: {:.1} Nm", result.applied_limits.max_ffb_torque);
    println!("    Allow high torque: {}", result.applied_limits.allow_high_torque);

    // Set specific axis to demo mode
    let result = service.set_capability_mode(
        CapabilityMode::Demo,
        Some(vec!["pitch".to_string()]),
        true,
    )?;
    println!("  ✓ Set DEMO mode for pitch axis");
    println!("    Max axis output: {:.1}%", result.applied_limits.max_axis_output * 100.0);

    // Check for restricted axes
    let has_restricted = service.has_restricted_axes()?;
    let restricted_axes = service.get_restricted_axes()?;
    println!("  Has restricted axes: {}", has_restricted);
    for (axis_name, mode) in restricted_axes {
        println!("    {} → {:?} mode", axis_name, mode);
    }

    // Get final status
    let status = service.get_capability_status(None)?;
    println!("  Final status:");
    for axis_status in status {
        println!("    {} → {:?} mode (max output: {:.1}%)", 
                 axis_status.axis_name, 
                 axis_status.mode, 
                 axis_status.limits.max_axis_output * 100.0);
    }

    // Reset to full mode
    let result = service.set_kid_mode(false)?;
    println!("  ✓ Reset to FULL mode: {} axes affected", result.affected_axes.len());

    Ok(())
}