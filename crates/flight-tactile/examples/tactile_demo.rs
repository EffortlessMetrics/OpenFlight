//! Tactile bridge demonstration
//!
//! This example shows how to use the tactile bridge to route flight simulation
//! effects to SimShaker-class applications.

use flight_tactile::{TactileManager, TactileConfig, EffectType};
use flight_bus::{BusSnapshot, SimId, AircraftId};
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Flight Tactile Bridge Demo");
    println!("==========================");

    // Create tactile manager
    let mut manager = TactileManager::new();
    
    // Configure tactile bridge
    let mut config = TactileConfig::default();
    config.simshaker.target_address = "127.0.0.1".to_string();
    config.simshaker.target_port = 4123;
    config.simshaker.update_rate_hz = 60.0;
    
    println!("Initializing tactile manager...");
    manager.initialize(config)?;
    
    // Enable tactile feedback
    manager.set_enabled(true);
    println!("Tactile feedback enabled");
    
    // Note: In a real application, you would start the manager here
    // manager.start()?;
    // But we skip this in the demo to avoid network operations
    
    // Create sample telemetry data
    let mut snapshot = BusSnapshot::new(SimId::Msfs, AircraftId::new("C172"));
    
    // Simulate different flight scenarios
    println!("\nTesting different tactile effects:");
    
    // Test touchdown effect
    println!("1. Testing touchdown effect...");
    manager.test_effect(EffectType::Touchdown, 0.8)?;
    std::thread::sleep(Duration::from_millis(100));
    
    // Test stall buffet effect
    println!("2. Testing stall buffet effect...");
    manager.test_effect(EffectType::StallBuffet, 0.6)?;
    std::thread::sleep(Duration::from_millis(100));
    
    // Test ground roll effect
    println!("3. Testing ground roll effect...");
    manager.test_effect(EffectType::GroundRoll, 0.4)?;
    std::thread::sleep(Duration::from_millis(100));
    
    // Test engine vibration effect
    println!("4. Testing engine vibration effect...");
    manager.test_effect(EffectType::EngineVibration, 0.3)?;
    std::thread::sleep(Duration::from_millis(100));
    
    // Test gear warning effect
    println!("5. Testing gear warning effect...");
    manager.test_effect(EffectType::GearWarning, 0.9)?;
    std::thread::sleep(Duration::from_millis(100));
    
    // Test rotor vibration effect (for helicopters)
    println!("6. Testing rotor vibration effect...");
    manager.test_effect(EffectType::RotorVibration, 0.5)?;
    std::thread::sleep(Duration::from_millis(100));
    
    // Process some telemetry data
    println!("\nProcessing telemetry data...");
    for i in 0..10 {
        // Simulate changing flight conditions
        snapshot.kinematics.ias = flight_bus::ValidatedSpeed::new_knots(100.0 + i as f32 * 10.0)?;
        snapshot.environment.altitude = 1000.0 + i as f32 * 100.0;
        
        manager.process_telemetry(&snapshot)?;
        std::thread::sleep(Duration::from_millis(50));
    }
    
    // Get statistics
    if let Some(stats) = manager.get_stats() {
        println!("\nTactile Bridge Statistics:");
        println!("  Snapshots processed: {}", stats.snapshots_processed);
        println!("  Effects generated: {}", stats.effects_generated);
        println!("  Outputs sent: {}", stats.outputs_sent);
        println!("  Thread running: {}", stats.thread_running);
        
        if let Some(simshaker_stats) = stats.simshaker_stats {
            println!("  SimShaker packets sent: {}", simshaker_stats.packets_sent);
            println!("  SimShaker status: {:?}", simshaker_stats.status);
        }
    }
    
    // Test configuration update
    println!("\nTesting configuration update...");
    let mut new_config = manager.get_config();
    new_config.simshaker.update_rate_hz = 30.0; // Reduce update rate
    manager.update_config(new_config)?;
    println!("Configuration updated successfully");
    
    // Test enable/disable
    println!("\nTesting enable/disable...");
    println!("  Enabled: {}", manager.is_enabled());
    manager.set_enabled(false);
    println!("  Disabled: {}", !manager.is_enabled());
    manager.set_enabled(true);
    println!("  Re-enabled: {}", manager.is_enabled());
    
    println!("\nDemo completed successfully!");
    println!("Note: This demo runs without network operations for safety.");
    println!("In a real application, the tactile bridge would connect to SimShaker");
    println!("or similar applications via UDP on the configured port.");
    
    Ok(())
}