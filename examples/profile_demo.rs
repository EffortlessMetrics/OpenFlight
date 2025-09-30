//! Profile and Rules Schema Demo
//! 
//! This example demonstrates the profile canonicalization, validation,
//! and rules DSL functionality implemented in task 3.

use flight_core::profile::{Profile, AircraftId, AxisConfig, DetentZone, CurvePoint};
use flight_core::rules::{RulesSchema, Rule, RuleDefaults};
use flight_panels::{PanelManager, LedTarget};
use std::collections::HashMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Flight Hub Profile & Rules Demo ===\n");

    // Demo 1: Profile Creation and Validation
    println!("1. Creating and validating a flight profile...");
    
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

    // Validate the profile
    match profile.validate() {
        Ok(_) => println!("✓ Profile validation passed"),
        Err(e) => println!("✗ Profile validation failed: {}", e),
    }

    // Demo 2: Profile Canonicalization and Hashing
    println!("\n2. Profile canonicalization and hashing...");
    
    let canonical = profile.canonicalize();
    let hash1 = profile.effective_hash();
    let hash2 = profile.effective_hash();
    
    println!("✓ Profile hash: {}", &hash1[..16]);
    println!("✓ Hash determinism: {}", if hash1 == hash2 { "PASS" } else { "FAIL" });

    // Demo 3: Profile Merging
    println!("\n3. Profile merging...");
    
    let mut override_axes = HashMap::new();
    override_axes.insert("pitch".to_string(), AxisConfig {
        deadzone: None,
        expo: Some(0.3), // Override expo
        slew_rate: Some(1.5), // Override slew rate
        detents: vec![],
        curve: None,
    });

    let override_profile = Profile {
        schema: "flight.profile/1".to_string(),
        sim: None,
        aircraft: Some(AircraftId { icao: "C172".to_string() }),
        axes: override_axes,
        pof_overrides: None,
    };

    match profile.merge_with(&override_profile) {
        Ok(merged) => {
            let pitch_config = merged.axes.get("pitch").unwrap();
            println!("✓ Merged profile:");
            println!("  - Deadzone: {:?} (from base)", pitch_config.deadzone);
            println!("  - Expo: {:?} (from override)", pitch_config.expo);
            println!("  - Slew rate: {:?} (from override)", pitch_config.slew_rate);
        }
        Err(e) => println!("✗ Profile merge failed: {}", e),
    }

    // Demo 4: Rules DSL
    println!("\n4. Rules DSL compilation...");
    
    let mut hysteresis = HashMap::new();
    hysteresis.insert("aoa".to_string(), 0.5);

    let rules = RulesSchema {
        schema: "flight.ledmap/1".to_string(),
        rules: vec![
            Rule {
                when: "gear_down".to_string(),
                do_action: "led.panel('GEAR').on()".to_string(),
                action: "led.panel('GEAR').on()".to_string(),
            },
            Rule {
                when: "ias > 90".to_string(),
                do_action: "led.indexer.blink(rate_hz=6)".to_string(),
                action: "led.indexer.blink(rate_hz=6)".to_string(),
            }
        ],
        defaults: Some(RuleDefaults {
            hysteresis: Some(hysteresis),
        }),
    };

    match rules.validate() {
        Ok(_) => println!("✓ Rules validation passed"),
        Err(e) => println!("✗ Rules validation failed: {}", e),
    }

    // Demo 5: Panel Manager Integration
    println!("\n5. Panel manager integration...");
    
    let mut panel_manager = PanelManager::new();
    
    match panel_manager.load_rules(rules) {
        Ok(_) => println!("✓ Rules loaded into panel manager"),
        Err(e) => println!("✗ Failed to load rules: {}", e),
    }

    // Simulate telemetry update
    let mut telemetry = HashMap::new();
    telemetry.insert("gear_down".to_string(), 1.0);
    telemetry.insert("ias".to_string(), 95.0);

    match panel_manager.update(&telemetry) {
        Ok(_) => println!("✓ Panel state updated with telemetry"),
        Err(e) => println!("✗ Panel update failed: {}", e),
    }

    // Check LED state
    let gear_target = LedTarget::Panel("GEAR".to_string());
    if let Some(state) = panel_manager.led_controller().get_led_state(&gear_target) {
        println!("✓ GEAR LED state: on={}, brightness={:.2}", state.on, state.brightness);
    }

    println!("\n=== Demo completed successfully! ===");
    Ok(())
}