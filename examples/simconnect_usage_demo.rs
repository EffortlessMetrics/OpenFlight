// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! SimConnect Usage Demo
//!
//! This example demonstrates how to use the flight-simconnect crate
//! to connect to Microsoft Flight Simulator and read telemetry data.

use flight_simconnect::{SimConnectAdapter, SimConnectConfig, SimConnectError};
use flight_bus::{BusSnapshot, SimId, AircraftId};
use flight_core::aircraft_switch::{DetectedAircraft, AircraftAutoSwitch, AutoSwitchConfig};
use std::time::Duration;
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();
    
    println!("=== Flight Hub SimConnect Usage Demo ===\n");

    // Demo 1: Basic Connection
    demo_basic_connection().await?;
    
    // Demo 2: Telemetry Reading
    demo_telemetry_reading().await?;
    
    // Demo 3: Aircraft Detection
    demo_aircraft_detection().await?;
    
    // Demo 4: Event Sending
    demo_event_sending().await?;
    
    // Demo 5: Error Handling
    demo_error_handling().await?;

    println!("\n=== SimConnect demo completed successfully! ===");
    Ok(())
}

async fn demo_basic_connection() -> Result<(), Box<dyn std::error::Error>> {
    println!("1. Basic SimConnect Connection");
    println!("-----------------------------");

    let config = SimConnectConfig {
        app_name: "Flight Hub Demo".to_string(),
        connection_timeout: Duration::from_secs(10),
        retry_attempts: 3,
        retry_delay: Duration::from_secs(2),
        telemetry_rate_hz: 30,
    };

    match SimConnectAdapter::new(config).await {
        Ok(mut adapter) => {
            println!("✓ Connected to MSFS successfully");
            
            // Check connection status
            if adapter.is_connected().await {
                println!("✓ Connection verified");
                
                // Get simulator info
                if let Ok(sim_info) = adapter.get_simulator_info().await {
                    println!("  Simulator: {}", sim_info.name);
                    println!("  Version: {}", sim_info.version);
                    println!("  Build: {}", sim_info.build);
                }
                
                adapter.disconnect().await?;
                println!("✓ Disconnected cleanly");
            } else {
                println!("✗ Connection verification failed");
            }
        }
        Err(SimConnectError::SimulatorNotRunning) => {
            println!("ℹ MSFS is not running - this is expected for demo");
            println!("  To test with real MSFS:");
            println!("  1. Start Microsoft Flight Simulator");
            println!("  2. Load into any aircraft");
            println!("  3. Run this demo again");
        }
        Err(e) => {
            println!("✗ Connection failed: {}", e);
            println!("  This is expected if MSFS is not running");
        }
    }

    Ok(())
}

async fn demo_telemetry_reading() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n2. Telemetry Reading");
    println!("-------------------");

    // Create a mock adapter for demonstration
    println!("ℹ Creating mock telemetry data (MSFS not required)");
    
    let mock_snapshot = create_mock_snapshot();
    
    println!("✓ Mock telemetry snapshot created:");
    println!("  Aircraft: {:?}", mock_snapshot.aircraft);
    println!("  IAS: {:.0} kt", mock_snapshot.kinematics.ias.as_knots());
    println!("  Altitude: {:.0} ft", mock_snapshot.environment.altitude);
    println!("  AoA: {:.1}°", mock_snapshot.kinematics.aoa.as_degrees());
    println!("  Gear: {:?}", mock_snapshot.config.gear);
    println!("  Flaps: {:.0}%", mock_snapshot.config.flaps.as_percentage());

    // Demonstrate telemetry processing
    if mock_snapshot.kinematics.ias.as_knots() > 100.0 {
        println!("  Status: Cruising");
    } else if mock_snapshot.environment.altitude < 1000.0 {
        println!("  Status: Pattern altitude");
    } else {
        println!("  Status: Climbing/Descending");
    }

    Ok(())
}

async fn demo_aircraft_detection() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n3. Aircraft Detection");
    println!("--------------------");

    // Create aircraft auto-switch system
    let config = AutoSwitchConfig {
        switch_delay_ms: 500,
        hysteresis_enabled: true,
        detection_confidence_threshold: 0.8,
    };
    
    let mut auto_switch = AircraftAutoSwitch::new(config);
    
    // Simulate aircraft detection sequence
    let aircraft_sequence = vec![
        ("Cessna 172", "C172", "Asobo"),
        ("Boeing 747-8", "B748", "Asobo"), 
        ("Airbus A320neo", "A20N", "Asobo"),
        ("Bell 407", "B407", "Third Party"),
    ];
    
    for (display_name, icao, manufacturer) in aircraft_sequence {
        let detected = DetectedAircraft {
            icao: icao.to_string(),
            display_name: display_name.to_string(),
            manufacturer: manufacturer.to_string(),
            confidence: 0.95,
            detection_time: std::time::Instant::now(),
        };
        
        match auto_switch.process_detection(detected).await {
            Ok(switch_result) => {
                if switch_result.profile_changed {
                    println!("✓ Switched to {}: {} ({})", icao, display_name, manufacturer);
                    if let Some(profile_path) = switch_result.applied_profile {
                        println!("  Applied profile: {}", profile_path);
                    }
                    println!("  Switch time: {} ms", switch_result.switch_time_ms);
                } else {
                    println!("  Already using profile for {}", icao);
                }
            }
            Err(e) => println!("✗ Aircraft switch failed: {}", e),
        }
        
        sleep(Duration::from_millis(100)).await;
    }
    
    // Show metrics
    let metrics = auto_switch.get_metrics();
    println!("✓ Auto-switch metrics:");
    println!("  Total switches: {}", metrics.total_switches);
    println!("  Average switch time: {:.1} ms", metrics.average_switch_time_ms);
    println!("  Failed switches: {}", metrics.failed_switches);

    Ok(())
}

async fn demo_event_sending() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n4. Event Sending");
    println!("---------------");

    println!("ℹ Demonstrating event sending (MSFS not required)");
    
    // List of common SimConnect events
    let demo_events = vec![
        ("GEAR_TOGGLE", "Toggle landing gear"),
        ("FLAPS_INCR", "Increase flaps one notch"),
        ("AP_MASTER", "Toggle autopilot master"),
        ("STROBES_TOGGLE", "Toggle strobe lights"),
        ("PARKING_BRAKES", "Toggle parking brake"),
    ];
    
    for (event_name, description) in demo_events {
        println!("  Would send: {} ({})", event_name, description);
        
        // In a real implementation with MSFS running:
        // adapter.send_event(event_name, 0).await?;
        
        sleep(Duration::from_millis(50)).await;
    }
    
    println!("✓ Event sending demonstration completed");
    println!("  Note: Events would be sent to MSFS if connected");

    Ok(())
}

async fn demo_error_handling() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n5. Error Handling");
    println!("----------------");

    // Demonstrate various error conditions
    let error_scenarios = vec![
        ("Simulator not running", SimConnectError::SimulatorNotRunning),
        ("Connection timeout", SimConnectError::ConnectionTimeout),
        ("Invalid event", SimConnectError::InvalidEvent("INVALID_EVENT".to_string())),
        ("Data request failed", SimConnectError::DataRequestFailed("Test request".to_string())),
    ];
    
    for (description, error) in error_scenarios {
        println!("  Scenario: {}", description);
        
        match error {
            SimConnectError::SimulatorNotRunning => {
                println!("    → Retry connection with backoff");
                println!("    → Show user-friendly message");
            }
            SimConnectError::ConnectionTimeout => {
                println!("    → Increase timeout and retry");
                println!("    → Check firewall settings");
            }
            SimConnectError::InvalidEvent(event) => {
                println!("    → Log invalid event: {}", event);
                println!("    → Skip and continue with next event");
            }
            SimConnectError::DataRequestFailed(request) => {
                println!("    → Retry request: {}", request);
                println!("    → Fallback to default values");
            }
            _ => {
                println!("    → Generic error handling");
            }
        }
    }
    
    println!("✓ Error handling scenarios demonstrated");

    Ok(())
}

fn create_mock_snapshot() -> BusSnapshot {
    use flight_bus::{Kinematics, AircraftConfig, Environment, Navigation, GearState, AutopilotState, LightsConfig};
    use flight_bus::types::{ValidatedSpeed, ValidatedAngle, GForce, Percentage, GearPosition};
    use std::collections::HashMap;

    BusSnapshot {
        sim: SimId::Msfs,
        aircraft: AircraftId::new("C172"),
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64,
        kinematics: Kinematics {
            ias: ValidatedSpeed::new_knots(120.0).unwrap(),
            tas: ValidatedSpeed::new_knots(125.0).unwrap(),
            ground_speed: ValidatedSpeed::new_knots(115.0).unwrap(),
            aoa: ValidatedAngle::new_degrees(5.2).unwrap(),
            sideslip: ValidatedAngle::new_degrees(0.1).unwrap(),
            bank: ValidatedAngle::new_degrees(-2.5).unwrap(),
            pitch: ValidatedAngle::new_degrees(3.8).unwrap(),
            heading: ValidatedAngle::new_degrees(270.0).unwrap(),
            g_force: GForce::new(1.02).unwrap(),
            g_lateral: GForce::new(-0.05).unwrap(),
            g_longitudinal: GForce::new(0.1).unwrap(),
            mach: flight_bus::Mach::new(0.18).unwrap(),
            vertical_speed: 150.0,
        },
        config: AircraftConfig {
            gear: GearState {
                nose: GearPosition::Down,
                left: GearPosition::Down,
                right: GearPosition::Down,
            },
            flaps: Percentage::new(10.0).unwrap(),
            spoilers: Percentage::new(0.0).unwrap(),
            ap_state: AutopilotState::Off,
            ap_altitude: Some(3500),
            ap_heading: Some(ValidatedAngle::new_degrees(270.0).unwrap()),
            ap_speed: Some(ValidatedSpeed::new_knots(120.0).unwrap()),
            lights: LightsConfig::default(),
            fuel: HashMap::new(),
        },
        helo: None,
        engines: vec![],
        environment: Environment {
            altitude: 3500.0,
            pressure_altitude: 3520.0,
            oat: 12.0,
            wind_speed: ValidatedSpeed::new_knots(8.0).unwrap(),
            wind_direction: ValidatedAngle::new_degrees(300.0).unwrap(),
            visibility: 10.0,
            cloud_coverage: Percentage::new(30.0).unwrap(),
        },
        navigation: Navigation {
            latitude: 47.6062,
            longitude: -122.3321,
            ground_track: ValidatedAngle::new_degrees(268.0).unwrap(),
            distance_to_dest: Some(15.2),
            time_to_dest: Some(7.6),
            active_waypoint: Some("KSEA".to_string()),
        },
    }
}