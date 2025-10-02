//! Telemetry synthesis effects demonstration
//!
//! This example demonstrates the telemetry-based force feedback effects
//! including stall buffet, touchdown impulse, ground roll, gear warnings,
//! and helicopter rotor effects.

use flight_bus::{BusSnapshot, HeloData, Kinematics, AircraftConfig, Environment, Navigation, SimId, AircraftId, ValidatedSpeed, ValidatedAngle, GForce, Percentage, GearState, types::GearPosition, AutopilotState, LightsConfig};
use flight_ffb::{TelemetrySynthEngine, TelemetrySynthConfig, FfbEngine, FfbConfig, FfbMode};
use std::time::{Duration, Instant};
use std::collections::HashMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    println!("Flight Hub Telemetry Synthesis Demo");
    println!("===================================");

    // Create telemetry synthesis configuration
    let mut synth_config = TelemetrySynthConfig::default();
    synth_config.rate_limiting.min_interval_ms = 50; // 20Hz update rate
    
    // Create and configure FFB engine for telemetry synthesis
    let mut ffb_config = FfbConfig {
        max_torque_nm: 15.0,
        fault_timeout_ms: 50,
        interlock_required: false, // Disable for demo
        mode: FfbMode::TelemetrySynth,
        device_path: None,
    };
    
    let mut ffb_engine = FfbEngine::new(ffb_config)?;
    ffb_engine.enable_telemetry_synthesis(synth_config)?;

    // Demo scenarios
    demo_stall_buffet(&mut ffb_engine)?;
    demo_touchdown_impulse(&mut ffb_engine)?;
    demo_ground_roll(&mut ffb_engine)?;
    demo_gear_warning(&mut ffb_engine)?;
    demo_helicopter_effects(&mut ffb_engine)?;
    demo_user_tuning(&mut ffb_engine)?;

    println!("\nDemo completed successfully!");
    Ok(())
}

fn demo_stall_buffet(ffb_engine: &mut FfbEngine) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n1. Stall Buffet Demo");
    println!("-------------------");
    
    let mut snapshot = create_base_snapshot();
    
    // Gradually increase angle of attack to demonstrate stall buffet
    for aoa in [8.0, 10.0, 12.0, 14.0, 16.0, 18.0] {
        snapshot.kinematics.aoa = ValidatedAngle::new_degrees(aoa)?;
        
        if let Some(output) = ffb_engine.update_telemetry_synthesis(&snapshot)? {
            println!("AoA: {:.1}° -> Torque: {:.2} Nm, Intensity: {:.2}, Effects: {:?}", 
                     aoa, output.torque_nm, output.intensity, output.active_effects);
        }
        
        std::thread::sleep(Duration::from_millis(100));
    }
    
    Ok(())
}

fn demo_touchdown_impulse(ffb_engine: &mut FfbEngine) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n2. Touchdown Impulse Demo");
    println!("------------------------");
    
    let mut snapshot = create_base_snapshot();
    
    // Simulate approach and touchdown
    let approach_sequence = [
        (-100.0, 100.0), // Approach
        (-300.0, 50.0),  // Steep descent
        (-500.0, 20.0),  // Hard landing approach
        (-100.0, 10.0),  // Touchdown transition
        (0.0, 5.0),      // On ground
    ];
    
    for (vs, alt) in approach_sequence {
        snapshot.kinematics.vertical_speed = vs;
        snapshot.environment.altitude = alt;
        
        if let Some(output) = ffb_engine.update_telemetry_synthesis(&snapshot)? {
            println!("VS: {:.0} fpm, Alt: {:.0} ft -> Torque: {:.2} Nm, Effects: {:?}", 
                     vs, alt, output.torque_nm, output.active_effects);
        }
        
        std::thread::sleep(Duration::from_millis(100));
    }
    
    Ok(())
}

fn demo_ground_roll(ffb_engine: &mut FfbEngine) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n3. Ground Roll Demo");
    println!("------------------");
    
    let mut snapshot = create_base_snapshot();
    snapshot.kinematics.g_force = GForce::new(1.0)?; // On ground
    
    // Simulate taxi and takeoff roll
    for speed in [0.0, 10.0, 30.0, 60.0, 80.0, 100.0] {
        snapshot.kinematics.ground_speed = ValidatedSpeed::new_knots(speed)?;
        
        if let Some(output) = ffb_engine.update_telemetry_synthesis(&snapshot)? {
            println!("Ground Speed: {:.0} kt -> Torque: {:.2} Nm, Effects: {:?}", 
                     speed, output.torque_nm, output.active_effects);
        }
        
        std::thread::sleep(Duration::from_millis(100));
    }
    
    Ok(())
}

fn demo_gear_warning(ffb_engine: &mut FfbEngine) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n4. Gear Warning Demo");
    println!("-------------------");
    
    let mut snapshot = create_base_snapshot();
    
    // Set up gear warning conditions (low speed, gear up, low altitude)
    snapshot.config.gear = GearState {
        nose: GearPosition::Up,
        left: GearPosition::Up,
        right: GearPosition::Up,
    };
    
    let warning_scenarios = [
        (150.0, 1500.0, "High speed, high altitude - no warning"),
        (100.0, 1500.0, "Low speed, high altitude - no warning"),
        (150.0, 500.0, "High speed, low altitude - no warning"),
        (100.0, 500.0, "Low speed, low altitude - WARNING!"),
    ];
    
    for (speed, altitude, description) in warning_scenarios {
        snapshot.kinematics.ias = ValidatedSpeed::new_knots(speed)?;
        snapshot.environment.altitude = altitude;
        
        if let Some(output) = ffb_engine.update_telemetry_synthesis(&snapshot)? {
            println!("{} -> Effects: {:?}", description, output.active_effects);
        }
        
        std::thread::sleep(Duration::from_millis(100));
    }
    
    Ok(())
}

fn demo_helicopter_effects(ffb_engine: &mut FfbEngine) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n5. Helicopter Effects Demo");
    println!("-------------------------");
    
    let mut snapshot = create_base_snapshot();
    
    // Add helicopter data with various rotor conditions
    let rotor_scenarios = [
        (100.0, 100.0, 50.0, "Normal operations"),
        (90.0, 100.0, 75.0, "Low Nr warning"),
        (100.0, 90.0, 85.0, "Low Np warning"),
        (85.0, 85.0, 95.0, "Both rotors low - critical!"),
    ];
    
    for (nr, np, torque, description) in rotor_scenarios {
        snapshot.helo = Some(HeloData {
            nr: Percentage::new(nr)?,
            np: Percentage::new(np)?,
            torque: Percentage::new(torque)?,
            collective: Percentage::new(50.0)?,
            pedals: 0.0,
        });
        
        if let Some(output) = ffb_engine.update_telemetry_synthesis(&snapshot)? {
            println!("{} (Nr: {:.0}%, Np: {:.0}%) -> Torque: {:.2} Nm, Effects: {:?}", 
                     description, nr, np, output.torque_nm, output.active_effects);
        }
        
        std::thread::sleep(Duration::from_millis(100));
    }
    
    Ok(())
}

fn demo_user_tuning(ffb_engine: &mut FfbEngine) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n6. User Tuning Demo");
    println!("------------------");
    
    let mut snapshot = create_base_snapshot();
    snapshot.kinematics.aoa = ValidatedAngle::new_degrees(15.0)?; // Trigger stall buffet
    
    // Get baseline output
    let baseline = ffb_engine.update_telemetry_synthesis(&snapshot)?;
    if let Some(output) = baseline {
        println!("Baseline stall buffet: {:.2} Nm", output.torque_nm);
    }
    
    // Adjust user tuning
    if let Some(synth_engine) = ffb_engine.get_telemetry_synth_mut() {
        let tuning = synth_engine.get_user_tuning_mut();
        
        // Reduce stall buffet intensity
        tuning.set_stall_buffet_intensity(0.5);
        println!("Reduced stall buffet intensity to 50%");
        
        let reduced = ffb_engine.update_telemetry_synthesis(&snapshot)?;
        if let Some(output) = reduced {
            println!("Reduced stall buffet: {:.2} Nm", output.torque_nm);
        }
        
        // Increase global intensity
        tuning.set_global_intensity(1.5);
        println!("Increased global intensity to 150%");
        
        let enhanced = ffb_engine.update_telemetry_synthesis(&snapshot)?;
        if let Some(output) = enhanced {
            println!("Enhanced stall buffet: {:.2} Nm", output.torque_nm);
        }
        
        // Show all tuning values
        println!("Current tuning values: {:?}", tuning.get_all_values());
    }
    
    Ok(())
}

fn create_base_snapshot() -> BusSnapshot {
    BusSnapshot {
        sim: SimId::Msfs,
        aircraft: AircraftId::new("C172"),
        timestamp: Instant::now().elapsed().as_nanos() as u64,
        kinematics: Kinematics {
            ias: ValidatedSpeed::new_knots(120.0).unwrap(),
            tas: ValidatedSpeed::new_knots(125.0).unwrap(),
            ground_speed: ValidatedSpeed::new_knots(115.0).unwrap(),
            aoa: ValidatedAngle::new_degrees(5.0).unwrap(),
            sideslip: ValidatedAngle::new_degrees(0.0).unwrap(),
            bank: ValidatedAngle::new_degrees(0.0).unwrap(),
            pitch: ValidatedAngle::new_degrees(3.0).unwrap(),
            heading: ValidatedAngle::new_degrees(90.0).unwrap(),
            g_force: GForce::new(1.0).unwrap(),
            g_lateral: GForce::new(0.0).unwrap(),
            g_longitudinal: GForce::new(0.0).unwrap(),
            mach: flight_bus::Mach::new(0.18).unwrap(),
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
            altitude: 3000.0,
            pressure_altitude: 3000.0,
            oat: 15.0,
            wind_speed: ValidatedSpeed::new_knots(10.0).unwrap(),
            wind_direction: ValidatedAngle::new_degrees(270.0).unwrap(),
            visibility: 10.0,
            cloud_coverage: Percentage::new(25.0).unwrap(),
        },
        navigation: Navigation {
            latitude: 47.6062,
            longitude: -122.3321,
            ground_track: ValidatedAngle::new_degrees(90.0).unwrap(),
            distance_to_dest: Some(25.0),
            time_to_dest: Some(12.0),
            active_waypoint: Some("KSEA".to_string()),
        },
    }
}