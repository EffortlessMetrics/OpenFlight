// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

#![allow(unused_imports)]
#![allow(clippy::single_component_path_imports)]

//! Profile Parsing and Validation Demo
//!
//! This example demonstrates comprehensive profile parsing, validation,
//! canonicalization, and merging functionality from flight-core.

use flight_core::error::FlightError;
use flight_core::profile::{AircraftId, AxisConfig, CurvePoint, DetentZone, PofOverrides, Profile};
use serde_json;
use std::collections::HashMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Flight Hub Profile Parsing Demo ===\n");

    // Demo 1: JSON Profile Parsing
    demo_json_parsing()?;

    // Demo 2: Profile Validation
    demo_profile_validation()?;

    // Demo 3: Profile Canonicalization
    demo_canonicalization()?;

    // Demo 4: Profile Merging
    demo_profile_merging()?;

    // Demo 5: Phase of Flight Overrides
    demo_pof_overrides()?;

    // Demo 6: Error Handling
    demo_error_handling()?;

    println!("\n=== Profile parsing demo completed successfully! ===");
    Ok(())
}

fn demo_json_parsing() -> Result<(), Box<dyn std::error::Error>> {
    println!("1. JSON Profile Parsing");
    println!("----------------------");

    let json_profile = r#"
    {
        "schema": "flight.profile/1",
        "sim": "msfs",
        "aircraft": {"icao": "C172"},
        "axes": {
            "pitch": {
                "deadzone": 0.03,
                "expo": 0.2,
                "slew_rate": 1.2,
                "detents": [
                    {
                        "position": 0.0,
                        "width": 0.05,
                        "role": "center"
                    }
                ],
                "curve": [
                    {"input": 0.0, "output": 0.0},
                    {"input": 0.5, "output": 0.3},
                    {"input": 1.0, "output": 1.0}
                ]
            },
            "roll": {
                "deadzone": 0.02,
                "expo": 0.15
            }
        }
    }
    "#;

    match serde_json::from_str::<Profile>(json_profile) {
        Ok(profile) => {
            println!("✓ Successfully parsed JSON profile");
            println!("  Schema: {}", profile.schema);
            println!("  Sim: {:?}", profile.sim);
            println!("  Aircraft: {:?}", profile.aircraft);
            println!("  Axes: {}", profile.axes.len());

            // Show axis details
            for (axis_name, config) in &profile.axes {
                println!(
                    "  {}: deadzone={:?}, expo={:?}, detents={}",
                    axis_name,
                    config.deadzone,
                    config.expo,
                    config.detents.len()
                );
            }
        }
        Err(e) => println!("✗ Failed to parse JSON: {}", e),
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

    // Non-monotonic curve
    let mut non_monotonic = valid_profile.clone();
    if let Some(pitch_config) = non_monotonic.axes.get_mut("pitch") {
        pitch_config.curve = Some(vec![
            CurvePoint {
                input: 0.0,
                output: 0.0,
            },
            CurvePoint {
                input: 0.5,
                output: 0.8,
            },
            CurvePoint {
                input: 1.0,
                output: 0.3,
            }, // Non-monotonic!
        ]);
    }
    match non_monotonic.validate() {
        Ok(_) => println!("✗ Non-monotonic curve should have failed"),
        Err(e) => println!("✓ Non-monotonic curve rejected: {}", e),
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

fn demo_canonicalization() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n3. Profile Canonicalization");
    println!("--------------------------");

    let profile = create_test_profile();

    // Create two identical profiles with different JSON formatting
    let json1 = serde_json::to_string_pretty(&profile)?;
    let json2 = serde_json::to_string(&profile)?; // Compact format

    let profile1: Profile = serde_json::from_str(&json1)?;
    let profile2: Profile = serde_json::from_str(&json2)?;

    // Canonicalize both
    let canonical1 = profile1.canonicalize();
    let canonical2 = profile2.canonicalize();

    println!("✓ Original JSON formats differ in whitespace");
    println!(
        "✓ Canonical forms are identical: {}",
        canonical1 == canonical2
    );

    // Test hash determinism
    let hash1 = profile1.effective_hash();
    let hash2 = profile2.effective_hash();

    println!(
        "✓ Hash determinism: {}",
        if hash1 == hash2 { "PASS" } else { "FAIL" }
    );
    println!("  Hash: {}", &hash1[..16]);

    Ok(())
}

fn demo_profile_merging() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n4. Profile Merging");
    println!("-----------------");

    let base_profile = create_test_profile();

    // Create override profile
    let mut override_axes = HashMap::new();
    override_axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: None,       // Keep base value
            expo: Some(0.35),     // Override
            slew_rate: Some(2.0), // Override
            detents: vec![],      // Keep base value (empty means no override)
            curve: None,          // Keep base value
            filter: None,
        },
    );
    override_axes.insert(
        "yaw".to_string(),
        AxisConfig {
            deadzone: Some(0.05), // New axis
            expo: Some(0.1),
            slew_rate: None,
            detents: vec![],
            curve: None,
            filter: None,
        },
    );

    let override_profile = Profile {
        schema: "flight.profile/1".to_string(),
        sim: None,
        aircraft: Some(AircraftId {
            icao: "C172".to_string(),
        }),
        axes: override_axes,
        pof_overrides: None,
    };

    match base_profile.merge_with(&override_profile) {
        Ok(merged) => {
            println!("✓ Profile merge successful");

            // Check pitch axis (should have overridden values)
            if let Some(pitch) = merged.axes.get("pitch") {
                println!("  Pitch axis:");
                println!("    Deadzone: {:?} (from base)", pitch.deadzone);
                println!("    Expo: {:?} (overridden)", pitch.expo);
                println!("    Slew rate: {:?} (overridden)", pitch.slew_rate);
            }

            // Check roll axis (should be unchanged)
            if let Some(roll) = merged.axes.get("roll") {
                println!("  Roll axis:");
                println!("    Deadzone: {:?} (from base)", roll.deadzone);
                println!("    Expo: {:?} (from base)", roll.expo);
            }

            // Check yaw axis (should be new)
            if let Some(yaw) = merged.axes.get("yaw") {
                println!("  Yaw axis:");
                println!("    Deadzone: {:?} (new)", yaw.deadzone);
                println!("    Expo: {:?} (new)", yaw.expo);
            }

            println!("  Total axes: {}", merged.axes.len());
        }
        Err(e) => println!("✗ Profile merge failed: {}", e),
    }

    Ok(())
}

fn demo_pof_overrides() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n5. Phase of Flight Overrides");
    println!("---------------------------");

    let mut profile = create_test_profile();

    // Add Phase of Flight overrides
    let mut pof_overrides = HashMap::new();

    // Approach phase - more sensitive controls
    let mut approach_axes = HashMap::new();
    approach_axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: None,
            expo: Some(0.3),      // More expo for approach
            slew_rate: Some(0.8), // Slower slew for stability
            detents: vec![],
            curve: None,
            filter: None,
        },
    );

    pof_overrides.insert(
        "approach".to_string(),
        PofOverrides {
            axes: Some(approach_axes),
            hysteresis: Some({
                let mut h = HashMap::new();
                h.insert("enter".to_string(), {
                    let mut enter = HashMap::new();
                    enter.insert("ias".to_string(), 90.0);
                    enter.insert("altitude".to_string(), 2000.0);
                    enter
                });
                h.insert("exit".to_string(), {
                    let mut exit = HashMap::new();
                    exit.insert("ias".to_string(), 100.0);
                    exit.insert("altitude".to_string(), 2500.0);
                    exit
                });
                h
            }),
        },
    );

    profile.pof_overrides = Some(pof_overrides);

    match profile.validate() {
        Ok(_) => {
            println!("✓ Profile with PoF overrides validated successfully");
            if let Some(overrides) = &profile.pof_overrides {
                println!("  Phase of Flight configurations: {}", overrides.len());
                for (phase, config) in overrides {
                    println!(
                        "    {}: {} axis overrides",
                        phase,
                        config.axes.as_ref().map_or(0, |a| a.len())
                    );
                }
            }
        }
        Err(e) => println!("✗ PoF profile validation failed: {}", e),
    }

    Ok(())
}

fn demo_error_handling() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n6. Error Handling");
    println!("----------------");

    // Test various error conditions
    let error_cases = vec![
        (
            "Empty schema",
            Profile {
                schema: "".to_string(),
                sim: None,
                aircraft: None,
                axes: HashMap::new(),
                pof_overrides: None,
            },
        ),
        ("Invalid expo range", {
            let mut profile = create_test_profile();
            if let Some(pitch) = profile.axes.get_mut("pitch") {
                pitch.expo = Some(2.0); // > 1.0
            }
            profile
        }),
        ("Negative slew rate", {
            let mut profile = create_test_profile();
            if let Some(pitch) = profile.axes.get_mut("pitch") {
                pitch.slew_rate = Some(-1.0);
            }
            profile
        }),
    ];

    for (description, profile) in error_cases {
        match profile.validate() {
            Ok(_) => println!("✗ {} should have failed", description),
            Err(e) => {
                println!("✓ {} correctly rejected", description);
                println!("    Error: {}", e);
            }
        }
    }

    Ok(())
}

fn create_test_profile() -> Profile {
    let mut axes = HashMap::new();

    axes.insert(
        "pitch".to_string(),
        AxisConfig {
            deadzone: Some(0.03),
            expo: Some(0.2),
            slew_rate: Some(1.2),
            detents: vec![DetentZone {
                position: 0.0,
                width: 0.05,
                role: "center".to_string(),
            }],
            curve: Some(vec![
                CurvePoint {
                    input: 0.0,
                    output: 0.0,
                },
                CurvePoint {
                    input: 0.5,
                    output: 0.3,
                },
                CurvePoint {
                    input: 1.0,
                    output: 1.0,
                },
            ]),
            filter: None,
        },
    );

    axes.insert(
        "roll".to_string(),
        AxisConfig {
            deadzone: Some(0.02),
            expo: Some(0.15),
            slew_rate: None,
            detents: vec![],
            curve: None,
            filter: None,
        },
    );

    Profile {
        schema: "flight.profile/1".to_string(),
        sim: Some("msfs".to_string()),
        aircraft: Some(AircraftId {
            icao: "C172".to_string(),
        }),
        axes,
        pof_overrides: None,
    }
}
