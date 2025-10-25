// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! SimConnect Usage Demo
//!
//! This example demonstrates how to use the flight-simconnect crate
//! to connect to Microsoft Flight Simulator and read telemetry data.

#![cfg_attr(not(feature = "simconnect"), allow(dead_code, unused_imports))]

#[cfg(feature = "simconnect")]
use flight_simconnect::{MsfsAdapter, MsfsAdapterConfig};
#[cfg(feature = "simconnect")]
use flight_bus::BusSnapshot;
#[cfg(feature = "simconnect")]
use flight_core::aircraft_switch::{DetectedAircraft, AircraftAutoSwitch, AutoSwitchConfig, SimId, AircraftId};
#[cfg(feature = "simconnect")]
use std::time::Duration;
#[cfg(feature = "simconnect")]
use tokio::time::sleep;

#[cfg(feature = "simconnect")]
#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
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

#[cfg(not(feature = "simconnect"))]
fn main() {
    eprintln!("Enable `--features simconnect` to build this example.");
}

#[cfg(feature = "simconnect")]
async fn demo_basic_connection() -> anyhow::Result<()> {
    println!("1. Basic SimConnect Connection");
    println!("-----------------------------");

    let config = MsfsAdapterConfig {
        session: flight_simconnect::SessionConfig::default(),
        mapping: flight_simconnect::mapping::create_default_mapping(),
        publish_rate: 30.0,
        aircraft_detection_timeout: Duration::from_secs(10),
        auto_reconnect: true,
        max_reconnect_attempts: 3,
    };

    match MsfsAdapter::new(config) {
        Ok(_adapter) => {
            println!("✓ MSFS adapter created successfully");
            println!("  Note: Actual connection happens when start() is called");
            println!("  In a real application:");
            println!("  1. Call adapter.start() to begin telemetry publishing");
            println!("  2. Subscribe to telemetry updates via the bus");
            println!("  3. Call adapter.stop() when done");
        }
        Err(e) => {
            println!("ℹ MSFS adapter creation failed: {}", e);
            println!("  This is expected if MSFS is not running");
            println!("  To test with real MSFS:");
            println!("  1. Start Microsoft Flight Simulator");
            println!("  2. Load into any aircraft");
            println!("  3. Run this demo again");
        }
    }

    Ok(())
}

#[cfg(feature = "simconnect")]
async fn demo_telemetry_reading() -> anyhow::Result<()> {
    println!("\n2. Telemetry Reading");
    println!("-------------------");

    // Create a mock adapter for demonstration
    println!("ℹ Creating mock telemetry data (MSFS not required)");
    
    let mock_snapshot = create_mock_snapshot();
    
    println!("✓ Mock telemetry snapshot created:");
    println!("  Aircraft: {:?}", mock_snapshot.aircraft);
    println!("  IAS: {:.0} kt", mock_snapshot.kinematics.ias.to_knots());
    println!("  Altitude: {:.0} ft", mock_snapshot.environment.altitude);
    println!("  AoA: {:.1}°", mock_snapshot.kinematics.aoa.to_degrees());
    println!("  Gear: {:?}", mock_snapshot.config.gear);
    println!("  Flaps: {:.0}%", mock_snapshot.config.flaps.value());

    // Demonstrate telemetry processing
    if mock_snapshot.kinematics.ias.to_knots() > 100.0 {
        println!("  Status: Cruising");
    } else if mock_snapshot.environment.altitude < 1000.0 {
        println!("  Status: Pattern altitude");
    } else {
        println!("  Status: Climbing/Descending");
    }

    Ok(())
}

#[cfg(feature = "simconnect")]
async fn demo_aircraft_detection() -> anyhow::Result<()> {
    println!("\n3. Aircraft Detection");
    println!("--------------------");

    // Create aircraft auto-switch system
    use flight_core::aircraft_switch::PofHysteresisConfig;
    use flight_core::profile::CapabilityContext;
    use std::collections::HashMap;
    
    let config = AutoSwitchConfig {
        max_switch_time: Duration::from_millis(500),
        profile_paths: vec![],
        enable_pof: false,
        pof_hysteresis: PofHysteresisConfig {
            min_phase_time: Duration::from_secs(5),
            hysteresis_bands: HashMap::new(),
        },
        capability_context: CapabilityContext::for_mode(flight_core::profile::CapabilityMode::Full),
    };
    
    let auto_switch = AircraftAutoSwitch::new(config);
    
    // Simulate aircraft detection sequence
    let aircraft_sequence = vec![
        ("Cessna 172", "C172", "Asobo"),
        ("Boeing 747-8", "B748", "Asobo"), 
        ("Airbus A320neo", "A20N", "Asobo"),
        ("Bell 407", "B407", "Third Party"),
    ];
    
    for (display_name, icao, _manufacturer) in aircraft_sequence {
        let detected = DetectedAircraft {
            sim: SimId::Msfs,
            aircraft_id: AircraftId::new(icao),
            process_name: display_name.to_string(),
            confidence: 0.95,
            detection_time: std::time::Instant::now(),
        };
        
        match auto_switch.on_aircraft_detected(detected).await {
            Ok(()) => {
                println!("✓ Aircraft detection sent for {}: {}", icao, display_name);
                // In a real implementation, the switch would happen asynchronously
                // and we'd get the result through a callback or channel
            }
            Err(e) => println!("✗ Aircraft detection failed: {}", e),
        }
        
        sleep(Duration::from_millis(100)).await;
    }
    
    // Show metrics
    let metrics = auto_switch.get_metrics().await;
    println!("✓ Auto-switch metrics:");
    println!("  Total switches: {}", metrics.total_switches);
    println!("  Average switch time: {:.1} ms", metrics.average_switch_time.as_millis());
    println!("  Failed switches: {}", metrics.failed_switches);

    Ok(())
}

#[cfg(feature = "simconnect")]
async fn demo_event_sending() -> anyhow::Result<()> {
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

#[cfg(feature = "simconnect")]
async fn demo_error_handling() -> anyhow::Result<()> {
    println!("\n5. Error Handling");
    println!("----------------");

    // Demonstrate various error conditions
    let error_scenarios = vec![
        ("Simulator not running", "SimulatorNotRunning"),
        ("Connection timeout", "ConnectionTimeout"),
        ("Invalid event", "InvalidEvent"),
        ("Data request failed", "DataRequestFailed"),
    ];
    
    for (description, error_type) in error_scenarios {
        println!("  Scenario: {}", description);
        
        match error_type {
            "SimulatorNotRunning" => {
                println!("    → Retry connection with backoff");
                println!("    → Show user-friendly message");
            }
            "ConnectionTimeout" => {
                println!("    → Increase timeout and retry");
                println!("    → Check firewall settings");
            }
            "InvalidEvent" => {
                println!("    → Log invalid event");
                println!("    → Skip and continue with next event");
            }
            "DataRequestFailed" => {
                println!("    → Retry request");
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

#[cfg(feature = "simconnect")]
fn create_mock_snapshot() -> BusSnapshot {
    use flight_bus::{Kinematics, AircraftConfig, Environment, Navigation, GearState, AutopilotState, LightsConfig};
    use flight_bus::types::{ValidatedSpeed, ValidatedAngle, GForce, Percentage, GearPosition};
    use std::collections::HashMap;

    BusSnapshot {
        sim: flight_bus::SimId::Msfs,
        aircraft: flight_bus::AircraftId::new("C172"),
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
            ap_altitude: Some(3500.0f32),
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