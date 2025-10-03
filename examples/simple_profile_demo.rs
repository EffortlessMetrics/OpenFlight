//! Simple Profile Demo
//!
//! A basic demonstration of profile parsing and validation functionality
//! that works with the current codebase.

use flight_core::profile::{Profile, AxisConfig, AircraftId, DetentZone, CurvePoint, CapabilityContext, CapabilityMode};
use std::collections::HashMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Simple Flight Hub Profile Demo ===\n");

    // Demo 1: Basic Profile Creation
    demo_basic_profile_creation()?;
    
    // Demo 2: Profile Validation
    demo_profile_validation()?;
    
    // Demo 3: Capability Enforcement
    demo_capability_enforcement()?;

    println!("\n=== Simple profile demo completed successfully! ===");
    Ok(())
}

fn demo_basic_profile_creation() -> Result<(), Box<dyn std::error::Error>> {
    println!("1. Basic Profile Creation");
    println!("------------------------");

    let mut axes = HashMap::new();
    axes.insert("pitch".to_string(), AxisConfig {
        deadzone: Some(0.03),
        expo: Some(0.2),
        slew_rate: Some(1.2),
        detents: vec![
            DetentZone {
                position: 0.0,
                width: 0.05,
                role: "center".to_string(),
            }
        ],
        curve: Some(vec![
            CurvePoint { input: 0.0, output: 0.0 },
            CurvePoint { input: 0.5, output: 0.3 },
            CurvePoint { input: 1.0, output: 1.0 },
        ]),
    });

    let profile = Profile {
        schema: "flight.profile/1".to_string(),
        sim: Some("msfs".to_string()),
        aircraft: Some(AircraftId { icao: "C172".to_string() }),
        axes,
        pof_overrides: None,
    };

    println!("✓ Created profile for {} in {}", 
             profile.aircraft.as_ref().unwrap().icao,
             profile.sim.as_ref().unwrap());
    println!("  Axes configured: {}", profile.axes.len());
    
    for (axis_name, config) in &profile.axes {
        println!("  {}: deadzone={:?}, expo={:?}, detents={}", 
                 axis_name, config.deadzone, config.expo, config.detents.len());
    }

    Ok(())
}

fn demo_profile_validation() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n2. Profile Validation");
    println!("--------------------");

    // Valid profile
    let valid_profile = create_test_profile();
    match valid_profile.validate() {
        Ok(_) => println!("✓ Valid profile passed validation"),
        Err(e) => println!("✗ Valid profile failed: {}", e),
    }

    // Invalid schema version
    let mut invalid_schema = valid_profile.clone();
    invalid_schema.schema = "flight.profile/999".to_string();
    match invalid_schema.validate() {
        Ok(_) => println!("✗ Invalid schema should have failed"),
        Err(e) => println!("✓ Invalid schema rejected: {}", e),
    }

    // Invalid deadzone range
    let mut invalid_deadzone = valid_profile.clone();
    if let Some(pitch_config) = invalid_deadzone.axes.get_mut("pitch") {
        pitch_config.deadzone = Some(1.5); // > 1.0
    }
    match invalid_deadzone.validate() {
        Ok(_) => println!("✗ Invalid deadzone should have failed"),
        Err(e) => println!("✓ Invalid deadzone rejected: {}", e),
    }

    Ok(())
}

fn demo_capability_enforcement() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n3. Capability Enforcement");
    println!("-------------------------");

    let mut profile = create_test_profile();
    
    // Add high expo that should be rejected in kid mode
    if let Some(pitch_config) = profile.axes.get_mut("pitch") {
        pitch_config.expo = Some(0.8);
    }
    
    // Test in full mode
    let full_context = CapabilityContext::for_mode(CapabilityMode::Full);
    match profile.validate_with_capabilities(&full_context) {
        Ok(_) => println!("✓ High expo accepted in FULL mode"),
        Err(e) => println!("✗ High expo rejected in FULL mode: {}", e),
    }
    
    // Test in demo mode
    let demo_context = CapabilityContext::for_mode(CapabilityMode::Demo);
    match profile.validate_with_capabilities(&demo_context) {
        Ok(_) => println!("✗ High expo should be rejected in DEMO mode"),
        Err(e) => println!("✓ High expo correctly rejected in DEMO mode: {}", e),
    }
    
    // Test in kid mode
    let kid_context = CapabilityContext::for_mode(CapabilityMode::Kid);
    match profile.validate_with_capabilities(&kid_context) {
        Ok(_) => println!("✗ High expo should be rejected in KID mode"),
        Err(e) => println!("✓ High expo correctly rejected in KID mode: {}", e),
    }

    // Show capability limits
    println!("\n  Capability limits by mode:");
    for mode in [CapabilityMode::Full, CapabilityMode::Demo, CapabilityMode::Kid] {
        let context = CapabilityContext::for_mode(mode);
        println!("    {:?}: max_expo={:.1}, max_output={:.1}%, max_torque={:.1}Nm", 
                 mode, 
                 context.limits.max_expo,
                 context.limits.max_axis_output * 100.0,
                 context.limits.max_ffb_torque);
    }

    Ok(())
}

fn create_test_profile() -> Profile {
    let mut axes = HashMap::new();
    
    axes.insert("pitch".to_string(), AxisConfig {
        deadzone: Some(0.03),
        expo: Some(0.2),
        slew_rate: Some(1.2),
        detents: vec![
            DetentZone {
                position: 0.0,
                width: 0.05,
                role: "center".to_string(),
            }
        ],
        curve: Some(vec![
            CurvePoint { input: 0.0, output: 0.0 },
            CurvePoint { input: 0.5, output: 0.3 },
            CurvePoint { input: 1.0, output: 1.0 },
        ]),
    });
    
    axes.insert("roll".to_string(), AxisConfig {
        deadzone: Some(0.02),
        expo: Some(0.15),
        slew_rate: None,
        detents: vec![],
        curve: None,
    });

    Profile {
        schema: "flight.profile/1".to_string(),
        sim: Some("msfs".to_string()),
        aircraft: Some(AircraftId { icao: "C172".to_string() }),
        axes,
        pof_overrides: None,
    }
}