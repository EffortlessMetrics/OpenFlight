// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! StreamDeck Panel Integration Demo
//!
//! This example demonstrates the StreamDeck integration functionality,
//! showing how to create panels, handle button presses, and update displays
//! based on flight telemetry.

use flight_streamdeck::{
    StreamDeckManager, StreamDeckConfig, StreamDeckDevice, StreamDeckError,
    ButtonAction, ButtonState, DisplayUpdate, PanelProfile, PanelButton,
    ImageSource, TextOverlay, ButtonLayout
};
use flight_bus::{BusSnapshot, SimId, AircraftId};
use flight_panels::{PanelManager, LedTarget, LedState};
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    
    println!("=== Flight Hub StreamDeck Panel Demo ===\n");

    // Demo 1: Device Discovery and Connection
    demo_device_discovery().await?;
    
    // Demo 2: Panel Profile Loading
    demo_panel_profiles().await?;
    
    // Demo 3: Button Actions and Events
    demo_button_actions().await?;
    
    // Demo 4: Telemetry-Driven Updates
    demo_telemetry_updates().await?;
    
    // Demo 5: Multi-Aircraft Profiles
    demo_multi_aircraft_profiles().await?;
    
    // Demo 6: Version Compatibility
    demo_version_compatibility().await?;

    println!("\n=== StreamDeck panel demo completed successfully! ===");
    Ok(())
}

async fn demo_device_discovery() -> anyhow::Result<()> {
    println!("1. Device Discovery and Connection");
    println!("---------------------------------");

    let config = StreamDeckConfig {
        auto_discovery: true,
        connection_timeout: Duration::from_secs(5),
        supported_versions: vec!["6.0".to_string(), "6.1".to_string(), "6.2".to_string()],
        api_port: 23654,
        websocket_port: 23655,
    };

    match StreamDeckManager::new(config).await {
        Ok(mut manager) => {
            println!("✓ StreamDeck manager initialized");
            
            // Discover devices
            let devices = manager.discover_devices().await?;
            println!("✓ Found {} StreamDeck device(s)", devices.len());
            
            for (i, device) in devices.iter().enumerate() {
                println!("  Device {}: {} ({}x{} buttons)", 
                         i + 1, device.model, device.columns, device.rows);
                println!("    Serial: {}", device.serial);
                println!("    Firmware: {}", device.firmware_version);
            }
            
            if !devices.is_empty() {
                // Connect to first device
                match manager.connect_device(&devices[0].serial).await {
                    Ok(_) => println!("✓ Connected to device: {}", devices[0].serial),
                    Err(e) => println!("✗ Connection failed: {}", e),
                }
            }
        }
        Err(StreamDeckError::AppNotRunning) => {
            println!("ℹ StreamDeck software is not running - this is expected for demo");
            println!("  To test with real StreamDeck:");
            println!("  1. Install StreamDeck software");
            println!("  2. Connect a StreamDeck device");
            println!("  3. Run this demo again");
        }
        Err(e) => {
            println!("✗ Manager initialization failed: {}", e);
            println!("  This is expected if StreamDeck software is not running");
        }
    }

    Ok(())
}

async fn demo_panel_profiles() -> anyhow::Result<()> {
    println!("\n2. Panel Profile Loading");
    println!("-----------------------");

    // Create sample profiles for different aircraft types
    let ga_profile = create_ga_profile();
    let airliner_profile = create_airliner_profile();
    let helo_profile = create_helo_profile();

    println!("✓ Created sample profiles:");
    println!("  GA Profile: {} buttons", ga_profile.buttons.len());
    println!("  Airliner Profile: {} buttons", airliner_profile.buttons.len());
    println!("  Helicopter Profile: {} buttons", helo_profile.buttons.len());

    // Demonstrate profile validation
    for (name, profile) in [
        ("GA", &ga_profile),
        ("Airliner", &airliner_profile), 
        ("Helicopter", &helo_profile)
    ] {
        match profile.validate() {
            Ok(_) => println!("✓ {} profile validation passed", name),
            Err(e) => println!("✗ {} profile validation failed: {}", name, e),
        }
    }

    // Show profile details
    println!("\n  GA Profile buttons:");
    for (pos, button) in &ga_profile.buttons {
        println!("    [{},{}]: {} - {}", pos.0, pos.1, button.title, button.action.description());
    }

    Ok(())
}

async fn demo_button_actions() -> anyhow::Result<()> {
    println!("\n3. Button Actions and Events");
    println!("---------------------------");

    // Create mock StreamDeck manager for demonstration
    println!("ℹ Simulating button press events (StreamDeck not required)");

    let button_scenarios = vec![
        ((0, 0), "GEAR_TOGGLE", "Landing Gear"),
        ((0, 1), "FLAPS_INCR", "Flaps Up"),
        ((1, 0), "AP_MASTER", "Autopilot"),
        ((1, 1), "STROBES_TOGGLE", "Strobe Lights"),
        ((2, 0), "PARKING_BRAKES", "Parking Brake"),
    ];

    for ((row, col), action, description) in button_scenarios {
        println!("  Button [{},{}] pressed: {}", row, col, description);
        
        // Simulate button action processing
        match action {
            "GEAR_TOGGLE" => {
                println!("    → Sending gear toggle event to sim");
                println!("    → Updating button LED state");
            }
            "FLAPS_INCR" => {
                println!("    → Incrementing flaps position");
                println!("    → Updating flaps indicator");
            }
            "AP_MASTER" => {
                println!("    → Toggling autopilot master");
                println!("    → Updating AP status display");
            }
            "STROBES_TOGGLE" => {
                println!("    → Toggling strobe lights");
                println!("    → Updating light status");
            }
            "PARKING_BRAKES" => {
                println!("    → Setting parking brake");
                println!("    → Updating brake indicator");
            }
            _ => println!("    → Unknown action"),
        }
        
        sleep(Duration::from_millis(100)).await;
    }

    println!("✓ Button action simulation completed");

    Ok(())
}

async fn demo_telemetry_updates() -> anyhow::Result<()> {
    println!("\n4. Telemetry-Driven Updates");
    println!("--------------------------");

    // Create mock telemetry sequence
    let telemetry_sequence = vec![
        create_ground_snapshot(),
        create_taxi_snapshot(),
        create_takeoff_snapshot(),
        create_climb_snapshot(),
        create_cruise_snapshot(),
    ];

    let phase_names = vec!["Ground", "Taxi", "Takeoff", "Climb", "Cruise"];

    for (i, snapshot) in telemetry_sequence.iter().enumerate() {
        println!("  Phase: {}", phase_names[i]);
        
        // Process telemetry and update displays
        let updates = process_telemetry_for_streamdeck(snapshot);
        
        for update in updates {
            match update {
                DisplayUpdate::Text { position, text, .. } => {
                    println!("    Button [{},{}]: {}", position.0, position.1, text);
                }
                DisplayUpdate::Image { position, .. } => {
                    println!("    Button [{},{}]: Updated image", position.0, position.1);
                }
                DisplayUpdate::Led { position, state } => {
                    let state_str = if state.on { "ON" } else { "OFF" };
                    println!("    LED [{},{}]: {} (brightness: {:.0}%)", 
                             position.0, position.1, state_str, state.brightness * 100.0);
                }
            }
        }
        
        sleep(Duration::from_millis(200)).await;
    }

    println!("✓ Telemetry update simulation completed");

    Ok(())
}

async fn demo_multi_aircraft_profiles() -> anyhow::Result<()> {
    println!("\n5. Multi-Aircraft Profiles");
    println!("-------------------------");

    // Simulate aircraft switching
    let aircraft_sequence = vec![
        ("C172", "Cessna 172"),
        ("A20N", "Airbus A320neo"),
        ("B748", "Boeing 747-8"),
        ("B407", "Bell 407"),
    ];

    for (icao, name) in aircraft_sequence {
        println!("  Switching to: {} ({})", name, icao);
        
        // Load appropriate profile
        let profile = match icao {
            "C172" => create_ga_profile(),
            "A20N" | "B748" => create_airliner_profile(),
            "B407" => create_helo_profile(),
            _ => create_ga_profile(),
        };
        
        println!("    → Loaded profile with {} buttons", profile.buttons.len());
        println!("    → Updated StreamDeck layout");
        
        // Show key differences
        match icao {
            "C172" => println!("    → GA layout: Basic flight controls"),
            "A20N" => println!("    → Airliner layout: MCDU, autopilot, systems"),
            "B748" => println!("    → Heavy jet layout: Engine controls, fuel management"),
            "B407" => println!("    → Helicopter layout: Collective, anti-torque, rotor"),
            _ => {}
        }
        
        sleep(Duration::from_millis(150)).await;
    }

    println!("✓ Multi-aircraft profile switching demonstrated");

    Ok(())
}

async fn demo_version_compatibility() -> anyhow::Result<()> {
    println!("\n6. Version Compatibility");
    println!("-----------------------");

    let version_scenarios = vec![
        ("6.0.0", true, "Fully supported"),
        ("6.1.2", true, "Fully supported"),
        ("6.2.0", true, "Latest supported"),
        ("5.9.0", false, "Too old - missing features"),
        ("7.0.0", false, "Too new - untested"),
        ("6.3.0", false, "Newer than tested"),
    ];

    for (version, supported, reason) in version_scenarios {
        println!("  StreamDeck v{}: {}", version, 
                 if supported { "✓ SUPPORTED" } else { "✗ UNSUPPORTED" });
        println!("    Reason: {}", reason);
        
        if supported {
            println!("    → Full functionality available");
        } else {
            println!("    → Graceful degradation or warning");
        }
    }

    println!("\n✓ Version compatibility matrix:");
    println!("  Minimum supported: 6.0.0");
    println!("  Maximum tested: 6.2.0");
    println!("  Behavior: Warn on unsupported versions, continue if possible");

    Ok(())
}

// Helper functions to create sample profiles

fn create_ga_profile() -> PanelProfile {
    let mut buttons = HashMap::new();
    
    buttons.insert((0, 0), PanelButton {
        title: "GEAR".to_string(),
        action: ButtonAction::SimEvent("GEAR_TOGGLE".to_string()),
        image: ImageSource::Icon("gear".to_string()),
        text_overlay: Some(TextOverlay {
            text: "GEAR".to_string(),
            position: (10, 50),
            font_size: 12,
            color: (255, 255, 255),
        }),
        led_binding: Some(LedTarget::Panel("GEAR".to_string())),
    });
    
    buttons.insert((0, 1), PanelButton {
        title: "FLAPS".to_string(),
        action: ButtonAction::SimEvent("FLAPS_INCR".to_string()),
        image: ImageSource::Icon("flaps".to_string()),
        text_overlay: Some(TextOverlay {
            text: "FLAPS".to_string(),
            position: (10, 50),
            font_size: 12,
            color: (255, 255, 255),
        }),
        led_binding: None,
    });
    
    buttons.insert((1, 0), PanelButton {
        title: "LIGHTS".to_string(),
        action: ButtonAction::SimEvent("STROBES_TOGGLE".to_string()),
        image: ImageSource::Icon("lights".to_string()),
        text_overlay: Some(TextOverlay {
            text: "STROBE".to_string(),
            position: (5, 50),
            font_size: 10,
            color: (255, 255, 255),
        }),
        led_binding: Some(LedTarget::Panel("STROBES".to_string())),
    });

    PanelProfile {
        name: "General Aviation".to_string(),
        aircraft_icao: vec!["C172".to_string(), "PA28".to_string()],
        buttons,
        layout: ButtonLayout::Grid { rows: 3, cols: 5 },
    }
}

fn create_airliner_profile() -> PanelProfile {
    let mut buttons = HashMap::new();
    
    buttons.insert((0, 0), PanelButton {
        title: "AP".to_string(),
        action: ButtonAction::SimEvent("AP_MASTER".to_string()),
        image: ImageSource::Icon("autopilot".to_string()),
        text_overlay: Some(TextOverlay {
            text: "A/P".to_string(),
            position: (15, 50),
            font_size: 14,
            color: (0, 255, 0),
        }),
        led_binding: Some(LedTarget::Panel("AP_MASTER".to_string())),
    });
    
    buttons.insert((0, 1), PanelButton {
        title: "A/THR".to_string(),
        action: ButtonAction::SimEvent("AUTO_THROTTLE_ARM".to_string()),
        image: ImageSource::Icon("autothrottle".to_string()),
        text_overlay: Some(TextOverlay {
            text: "A/THR".to_string(),
            position: (8, 50),
            font_size: 12,
            color: (0, 255, 0),
        }),
        led_binding: Some(LedTarget::Panel("AUTOTHROTTLE".to_string())),
    });
    
    buttons.insert((1, 0), PanelButton {
        title: "MCDU".to_string(),
        action: ButtonAction::Custom("open_mcdu".to_string()),
        image: ImageSource::Icon("mcdu".to_string()),
        text_overlay: Some(TextOverlay {
            text: "MCDU".to_string(),
            position: (10, 50),
            font_size: 12,
            color: (255, 255, 255),
        }),
        led_binding: None,
    });

    PanelProfile {
        name: "Airliner".to_string(),
        aircraft_icao: vec!["A20N".to_string(), "B738".to_string(), "B748".to_string()],
        buttons,
        layout: ButtonLayout::Grid { rows: 4, cols: 8 },
    }
}

fn create_helo_profile() -> PanelProfile {
    let mut buttons = HashMap::new();
    
    buttons.insert((0, 0), PanelButton {
        title: "ROTOR".to_string(),
        action: ButtonAction::SimEvent("ROTOR_BRAKE".to_string()),
        image: ImageSource::Icon("rotor".to_string()),
        text_overlay: Some(TextOverlay {
            text: "ROTOR".to_string(),
            position: (8, 50),
            font_size: 12,
            color: (255, 255, 0),
        }),
        led_binding: Some(LedTarget::Panel("ROTOR_BRAKE".to_string())),
    });
    
    buttons.insert((0, 1), PanelButton {
        title: "GOV".to_string(),
        action: ButtonAction::SimEvent("HELO_GOV_SWITCH".to_string()),
        image: ImageSource::Icon("governor".to_string()),
        text_overlay: Some(TextOverlay {
            text: "GOV".to_string(),
            position: (15, 50),
            font_size: 14,
            color: (0, 255, 0),
        }),
        led_binding: Some(LedTarget::Panel("GOVERNOR".to_string())),
    });

    PanelProfile {
        name: "Helicopter".to_string(),
        aircraft_icao: vec!["B407".to_string(), "H145".to_string()],
        buttons,
        layout: ButtonLayout::Grid { rows: 3, cols: 5 },
    }
}

// Helper functions to create telemetry snapshots

fn create_ground_snapshot() -> BusSnapshot {
    use flight_bus::{Kinematics, AircraftConfig, Environment, Navigation, GearState, AutopilotState, LightsConfig};
    use flight_bus::types::{ValidatedSpeed, ValidatedAngle, GForce, Percentage, GearPosition};
    use std::collections::HashMap;

    BusSnapshot {
        sim: SimId::Msfs,
        aircraft: AircraftId::new("C172"),
        timestamp: 1000,
        kinematics: Kinematics {
            ias: ValidatedSpeed::new_knots(0.0).unwrap(),
            tas: ValidatedSpeed::new_knots(0.0).unwrap(),
            ground_speed: ValidatedSpeed::new_knots(0.0).unwrap(),
            aoa: ValidatedAngle::new_degrees(0.0).unwrap(),
            sideslip: ValidatedAngle::new_degrees(0.0).unwrap(),
            bank: ValidatedAngle::new_degrees(0.0).unwrap(),
            pitch: ValidatedAngle::new_degrees(0.0).unwrap(),
            heading: ValidatedAngle::new_degrees(90.0).unwrap(),
            g_force: GForce::new(1.0).unwrap(),
            g_lateral: GForce::new(0.0).unwrap(),
            g_longitudinal: GForce::new(0.0).unwrap(),
            mach: flight_bus::Mach::new(0.0).unwrap(),
            vertical_speed: 0.0,
        },
        config: AircraftConfig {
            gear: GearState {
                nose: GearPosition::Down,
                left: GearPosition::Down,
                right: GearPosition::Down,
            },
            flaps: Percentage::new(0.0).unwrap(),
            spoilers: Percentage::new(0.0).unwrap(),
            ap_state: AutopilotState::Off,
            ap_altitude: None,
            ap_heading: None,
            ap_speed: None,
            lights: LightsConfig::default(),
            fuel: HashMap::new(),
        },
        helo: None,
        engines: vec![],
        environment: Environment {
            altitude: 100.0,
            pressure_altitude: 100.0,
            oat: 15.0,
            wind_speed: ValidatedSpeed::new_knots(5.0).unwrap(),
            wind_direction: ValidatedAngle::new_degrees(270.0).unwrap(),
            visibility: 10.0,
            cloud_coverage: Percentage::new(0.0).unwrap(),
        },
        navigation: Navigation {
            latitude: 47.6062,
            longitude: -122.3321,
            ground_track: ValidatedAngle::new_degrees(90.0).unwrap(),
            distance_to_dest: None,
            time_to_dest: None,
            active_waypoint: None,
        },
    }
}

fn create_taxi_snapshot() -> BusSnapshot {
    let mut snapshot = create_ground_snapshot();
    snapshot.kinematics.ground_speed = flight_bus::types::ValidatedSpeed::new_knots(15.0).unwrap();
    snapshot.timestamp = 2000;
    snapshot
}

fn create_takeoff_snapshot() -> BusSnapshot {
    let mut snapshot = create_ground_snapshot();
    snapshot.kinematics.ias = flight_bus::types::ValidatedSpeed::new_knots(65.0).unwrap();
    snapshot.kinematics.ground_speed = flight_bus::types::ValidatedSpeed::new_knots(65.0).unwrap();
    snapshot.timestamp = 3000;
    snapshot
}

fn create_climb_snapshot() -> BusSnapshot {
    let mut snapshot = create_ground_snapshot();
    snapshot.kinematics.ias = flight_bus::types::ValidatedSpeed::new_knots(85.0).unwrap();
    snapshot.environment.altitude = 1500.0;
    snapshot.kinematics.vertical_speed = 700.0;
    snapshot.config.gear.nose = flight_bus::types::GearPosition::Up;
    snapshot.config.gear.left = flight_bus::types::GearPosition::Up;
    snapshot.config.gear.right = flight_bus::types::GearPosition::Up;
    snapshot.timestamp = 4000;
    snapshot
}

fn create_cruise_snapshot() -> BusSnapshot {
    let mut snapshot = create_ground_snapshot();
    snapshot.kinematics.ias = flight_bus::types::ValidatedSpeed::new_knots(120.0).unwrap();
    snapshot.environment.altitude = 3500.0;
    snapshot.kinematics.vertical_speed = 0.0;
    snapshot.config.gear.nose = flight_bus::types::GearPosition::Up;
    snapshot.config.gear.left = flight_bus::types::GearPosition::Up;
    snapshot.config.gear.right = flight_bus::types::GearPosition::Up;
    snapshot.config.ap_state = AutopilotState::On;
    snapshot.timestamp = 5000;
    snapshot
}

fn process_telemetry_for_streamdeck(snapshot: &BusSnapshot) -> Vec<DisplayUpdate> {
    let mut updates = vec![];
    
    // Update gear indicator
    let gear_state = if matches!(snapshot.config.gear.nose, flight_bus::types::GearPosition::Down) {
        "DOWN"
    } else {
        "UP"
    };
    updates.push(DisplayUpdate::Text {
        position: (0, 0),
        text: gear_state.to_string(),
        font_size: 12,
        color: if gear_state == "DOWN" { (0, 255, 0) } else { (255, 0, 0) },
    });
    
    // Update speed indicator
    updates.push(DisplayUpdate::Text {
        position: (0, 1),
        text: format!("{:.0}kt", snapshot.kinematics.ias.as_knots()),
        font_size: 10,
        color: (255, 255, 255),
    });
    
    // Update autopilot indicator
    let ap_on = matches!(snapshot.config.ap_state, AutopilotState::On);
    updates.push(DisplayUpdate::Led {
        position: (1, 0),
        state: LedState {
            on: ap_on,
            brightness: if ap_on { 1.0 } else { 0.0 },
            color: (0, 255, 0),
        },
    });
    
    updates
}