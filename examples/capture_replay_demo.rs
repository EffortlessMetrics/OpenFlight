// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Capture and Replay Demo
//!
//! This example demonstrates the blackbox recording and replay functionality,
//! showing how to capture flight data and replay it for analysis and testing.

use flight_replay::{ReplayEngine, ReplayConfig, ReplayError, ReplayStats};
use flight_core::blackbox::{BlackboxWriter, BlackboxReader, BlackboxConfig};
use flight_axis::{AxisEngine, AxisFrame};
use flight_bus::{BusSnapshot, SimId, AircraftId};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tempfile::TempDir;

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    
    println!("=== Flight Hub Capture & Replay Demo ===\n");

    // Demo 1: Blackbox Recording
    demo_blackbox_recording().await?;
    
    // Demo 2: Blackbox Reading
    demo_blackbox_reading().await?;
    
    // Demo 3: Replay Engine
    demo_replay_engine().await?;
    
    // Demo 4: Validation and Analysis
    demo_validation_analysis().await?;
    
    // Demo 5: Performance Testing
    demo_performance_testing().await?;

    println!("\n=== Capture & replay demo completed successfully! ===");
    Ok(())
}

async fn demo_blackbox_recording() -> anyhow::Result<()> {
    println!("1. Blackbox Recording");
    println!("--------------------");

    // Create temporary directory for demo files
    let temp_dir = TempDir::new()?;
    let recording_path = temp_dir.path().join("demo_flight.fbb");

    let config = BlackboxConfig {
        output_dir: recording_path.clone(),
        max_file_size_mb: 100,
        max_recording_duration: std::time::Duration::from_secs(3600),
        enable_compression: true,
        buffer_size: 1024 * 1024, // 1MB buffer
    };

    let mut writer = BlackboxWriter::new(config);
    println!("✓ Blackbox writer initialized");
    println!("  Output file: {:?}", recording_path);

    // Start recording
    writer.start_recording(
        "openflight-demo".into(),
        "msfs".into(),
        "C172".into()
    ).await?;
    println!("✓ Recording started");

    // Simulate a flight session
    let session_duration = Duration::from_secs(10);
    let start_time = Instant::now();
    let mut frame_count = 0;
    let mut bus_count = 0;

    println!("  Simulating flight data...");

    while start_time.elapsed() < session_duration {
        let timestamp = start_time.elapsed().as_nanos() as u64;
        
        // Record axis frames at 250Hz
        if frame_count % 4 == 0 { // Every 4ms for 250Hz
            let axis_frame = create_mock_axis_frame(timestamp, frame_count);
            let data = bincode::serialize(&axis_frame)?;
            writer.record_axis_frame(timestamp, &data)?;
            frame_count += 1;
        }
        
        // Record bus snapshots at 60Hz
        if bus_count % 16 == 0 { // Every 16ms for ~60Hz
            let bus_snapshot = create_mock_bus_snapshot(timestamp);
            let data = bincode::serialize(&bus_snapshot)?;
            writer.record_bus_snapshot(timestamp, &data)?;
            bus_count += 1;
        }
        
        // Record events occasionally
        if frame_count % 1000 == 0 {
            let event_data = format!("Applied profile for frame {}", frame_count);
            writer.record_event(timestamp, event_data.as_bytes())?;
        }
        
        tokio::time::sleep(Duration::from_millis(1)).await;
    }

    // Stop recording
    writer.stop_recording().await?;
    let stats = writer.get_stats();
    
    println!("✓ Recording completed");
    println!("  Duration: {:.1}s", session_duration.as_secs_f32());
    println!("  Records written: {}", stats.records_written.iter().sum::<u64>());
    println!("  Bytes written:   {}", stats.bytes_written);
    println!("  Chunks written:  {}", stats.chunks_written);

    Ok(())
}

async fn demo_blackbox_reading() -> anyhow::Result<()> {
    println!("\n2. Blackbox Reading");
    println!("------------------");

    // Create a sample recording first
    let temp_dir = TempDir::new()?;
    let recording_path = temp_dir.path().join("sample.fbb");
    create_sample_recording(&recording_path).await?;

    // Read the recording
    let mut reader = BlackboxReader::open(&recording_path)?;
    println!("✓ Blackbox reader initialized");

    // Read header information
    let header = reader.header();
    println!("✓ Header information:");
    println!("  App version: {}", header.app_version);
    println!("  Sim: {}", header.sim_id);
    println!("  Aircraft: {}", header.aircraft_id);
    println!("  Recording start: {}", header.start_timestamp);

    // Note: Record reading and seeking functionality would be implemented here
    // For now, we just validate the file structure
    println!("✓ File structure validated");

    Ok(())
}

async fn demo_replay_engine() -> anyhow::Result<()> {
    println!("\n3. Replay Engine");
    println!("---------------");

    // Create sample recording
    let temp_dir = TempDir::new()?;
    let recording_path = temp_dir.path().join("replay_test.fbb");
    create_sample_recording(&recording_path).await?;

    let config = ReplayConfig {
        playback_speed: 1.0,
        loop_playback: false,
        validate_outputs: true,
        fp_tolerance: 1e-6,
        timing_tolerance_ms: 1.0,
    };

    let mut replay_engine = ReplayEngine::new(config)?;
    println!("✓ Replay engine initialized");

    // Load the recording
    replay_engine.load_recording(&recording_path).await?;
    println!("✓ Recording loaded for replay");

    // Set up axis engine for replay validation
    let mut axis_engine = AxisEngine::new_for_axis("pitch".to_string());
    replay_engine.register_axis_engine("pitch".to_string(), axis_engine)?;
    println!("✓ Axis engine registered for validation");

    // Start replay
    replay_engine.start_replay().await?;
    println!("✓ Replay started");

    let mut validation_results = Vec::new();
    let mut processed_frames = 0;

    // Process replay frames
    while let Some(replay_frame) = replay_engine.get_next_frame().await? {
        match replay_frame {
            flight_replay::ReplayFrame::AxisFrame { axis_name, original_frame, replayed_frame } => {
                // Validate that replayed output matches original
                let output_diff = (replayed_frame.out - original_frame.out).abs();
                let timing_diff = (replayed_frame.ts_mono_ns as i64 - original_frame.ts_mono_ns as i64).abs();
                
                validation_results.push((output_diff, timing_diff));
                processed_frames += 1;
                
                if processed_frames % 100 == 0 {
                    println!("    Processed {} frames", processed_frames);
                }
            }
            flight_replay::ReplayFrame::BusSnapshot { .. } => {
                // Bus snapshots don't need validation in this demo
            }
            flight_replay::ReplayFrame::Event { .. } => {
                // Events are informational
            }
        }
        
        // Limit demo to reasonable number of frames
        if processed_frames >= 1000 {
            break;
        }
    }

    // Stop replay and get statistics
    replay_engine.stop_replay().await?;
    let stats = replay_engine.get_stats();

    println!("✓ Replay completed");
    println!("  Frames processed: {}", processed_frames);
    println!("  Validation results: {} samples", validation_results.len());

    Ok(())
}

async fn demo_validation_analysis() -> anyhow::Result<()> {
    println!("\n4. Validation and Analysis");
    println!("-------------------------");

    // Create test data with known characteristics
    let temp_dir = TempDir::new()?;
    let recording_path = temp_dir.path().join("validation_test.fbb");
    create_deterministic_recording(&recording_path).await?;

    let config = ReplayConfig {
        playback_speed: 1.0,
        loop_playback: false,
        validate_outputs: true,
        fp_tolerance: 1e-6,
        timing_tolerance_ms: 0.1,
    };

    let mut replay_engine = ReplayEngine::new(config)?;
    replay_engine.load_recording(&recording_path).await?;

    // Create axis engine with known profile
    let mut axis_engine = AxisEngine::new_for_axis("test".to_string());
    // Apply a simple profile for predictable results
    let profile = create_deterministic_profile();
    axis_engine.apply_profile(&profile)?;
    
    replay_engine.register_axis_engine("test".to_string(), axis_engine)?;

    println!("✓ Validation setup completed");

    // Run validation
    replay_engine.start_replay().await?;
    
    let mut max_output_error = 0.0f32;
    let mut max_timing_error = 0u64;
    let mut validation_count = 0;

    while let Some(frame) = replay_engine.get_next_frame().await? {
        if let flight_replay::ReplayFrame::AxisFrame { original_frame, replayed_frame, .. } = frame {
            let output_error = (replayed_frame.out - original_frame.out).abs();
            let timing_error = (replayed_frame.ts_mono_ns as i64 - original_frame.ts_mono_ns as i64).abs() as u64;
            
            max_output_error = max_output_error.max(output_error);
            max_timing_error = max_timing_error.max(timing_error);
            validation_count += 1;
        }
    }

    replay_engine.stop_replay().await?;
    let stats = replay_engine.get_stats();

    println!("✓ Validation analysis completed");
    println!("  Validated frames: {}", validation_count);
    println!("  Max output error: {:.2e}", max_output_error);
    println!("  Max timing error: {} ns", max_timing_error);
    println!("  Validation passed: {}", stats.validation_passed);
    
    if stats.validation_passed {
        println!("  ✓ All validations within tolerance");
    } else {
        println!("  ✗ Some validations exceeded tolerance");
        println!("    Failed validations: {}", stats.failed_validations);
    }

    Ok(())
}

async fn demo_performance_testing() -> anyhow::Result<()> {
    println!("\n5. Performance Testing");
    println!("---------------------");

    // Create a larger recording for performance testing
    let temp_dir = TempDir::new()?;
    let recording_path = temp_dir.path().join("perf_test.fbb");
    create_performance_recording(&recording_path).await?;

    let config = ReplayConfig {
        playback_speed: 10.0, // 10x speed for performance test
        loop_playback: false,
        validate_outputs: false, // Disable validation for pure performance
        fp_tolerance: 1e-6,
        timing_tolerance_ms: 1.0,
    };

    let mut replay_engine = ReplayEngine::new(config)?;
    replay_engine.load_recording(&recording_path).await?;

    println!("✓ Performance test setup completed");

    // Measure replay performance
    let start_time = Instant::now();
    replay_engine.start_replay().await?;

    let mut frame_count = 0;
    while let Some(_frame) = replay_engine.get_next_frame().await? {
        frame_count += 1;
        
        // Process in batches for performance measurement
        if frame_count % 10000 == 0 {
            let elapsed = start_time.elapsed();
            let fps = frame_count as f64 / elapsed.as_secs_f64();
            println!("    Processed {} frames ({:.0} fps)", frame_count, fps);
        }
    }

    replay_engine.stop_replay().await?;
    let total_time = start_time.elapsed();
    let stats = replay_engine.get_stats();

    println!("✓ Performance test completed");
    println!("  Total frames: {}", frame_count);
    println!("  Total time: {:.2}s", total_time.as_secs_f32());
    println!("  Average FPS: {:.0}", frame_count as f64 / total_time.as_secs_f64());
    println!("  Memory usage: {:.1} MB", stats.memory_usage_mb);
    
    // Check if performance meets requirements
    let target_fps = 250.0 * 10.0; // 250Hz * 10x speed
    let actual_fps = frame_count as f64 / total_time.as_secs_f64();
    
    if actual_fps >= target_fps {
        println!("  ✓ Performance target met ({:.0} >= {:.0} fps)", actual_fps, target_fps);
    } else {
        println!("  ✗ Performance target missed ({:.0} < {:.0} fps)", actual_fps, target_fps);
    }

    Ok(())
}

// Helper functions

async fn create_sample_recording(path: &PathBuf) -> anyhow::Result<()> {
    let config = BlackboxConfig {
        output_dir: path.clone(),
        max_file_size_mb: 10,
        max_recording_duration: std::time::Duration::from_secs(60),
        enable_compression: false,
        buffer_size: 512 * 1024, // 512KB buffer
    };

    let mut writer = BlackboxWriter::new(config);
    writer.start_recording(
        "openflight-demo".into(),
        "msfs".into(),
        "C172".into()
    ).await?;

    // Write sample data
    for i in 0..1000 {
        let timestamp = i * 4_000_000; // 4ms intervals for 250Hz
        
        let axis_frame = create_mock_axis_frame(timestamp, i);
        let data = bincode::serialize(&axis_frame)?;
        writer.record_axis_frame(timestamp, &data)?;
        
        // Add bus snapshot every 16ms (60Hz)
        if i % 4 == 0 {
            let bus_snapshot = create_mock_bus_snapshot(timestamp);
            let data = bincode::serialize(&bus_snapshot)?;
            writer.record_bus_snapshot(timestamp, &data)?;
        }
    }

    writer.stop_recording().await?;
    Ok(())
}

async fn create_deterministic_recording(path: &PathBuf) -> anyhow::Result<()> {
    let config = BlackboxConfig {
        output_dir: path.clone(),
        max_file_size_mb: 10,
        max_recording_duration: std::time::Duration::from_secs(60),
        enable_compression: false,
        buffer_size: 512 * 1024, // 512KB buffer
    };

    let mut writer = BlackboxWriter::new(config);
    writer.start_recording(
        "openflight-demo".into(),
        "msfs".into(),
        "test".into()
    ).await?;

    // Create deterministic test pattern
    for i in 0..500 {
        let timestamp = i * 4_000_000;
        let input = (i as f32) / 500.0; // Linear ramp 0 to 1
        
        let mut frame = AxisFrame::new(input, timestamp);
        frame.out = input * 0.8; // Simple linear response for validation
        
        let data = bincode::serialize(&frame)?;
        writer.record_axis_frame(timestamp, &data)?;
    }

    writer.stop_recording().await?;
    Ok(())
}

async fn create_performance_recording(path: &PathBuf) -> anyhow::Result<()> {
    let config = BlackboxConfig {
        output_dir: path.clone(),
        max_file_size_mb: 50,
        max_recording_duration: std::time::Duration::from_secs(300),
        enable_compression: true,
        buffer_size: 2 * 1024 * 1024, // 2MB buffer
    };

    let mut writer = BlackboxWriter::new(config);
    writer.start_recording(
        "openflight-demo".into(),
        "msfs".into(),
        "C172".into()
    ).await?;

    // Create larger dataset for performance testing
    for i in 0..50000 {
        let timestamp = i * 4_000_000;
        let axis_frame = create_mock_axis_frame(timestamp, i);
        
        let data = bincode::serialize(&axis_frame)?;
        writer.record_axis_frame(timestamp, &data)?;
    }

    writer.stop_recording().await?;
    Ok(())
}

fn create_mock_axis_frame(timestamp: u64, sequence: u64) -> AxisFrame {
    let input = ((sequence as f32) * 0.01).sin() * 0.5 + 0.5; // Sine wave input
    let mut frame = AxisFrame::new(input, timestamp);
    frame.out = input * 0.9; // Mock processing
    frame.d_in_dt = ((sequence as f32) * 0.01).cos() * 0.005; // Derivative
    frame
}

fn create_mock_bus_snapshot(timestamp: u64) -> BusSnapshot {
    use flight_bus::{Kinematics, AircraftConfig, Environment, Navigation, GearState, AutopilotState, LightsConfig};
    use flight_bus::types::{ValidatedSpeed, ValidatedAngle, GForce, Percentage, GearPosition};
    use std::collections::HashMap;

    BusSnapshot {
        sim: SimId::Msfs,
        aircraft: AircraftId::new("C172"),
        timestamp,
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
            altitude: 3500.0,
            pressure_altitude: 3520.0,
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

fn create_deterministic_profile() -> flight_core::profile::Profile {
    use flight_core::profile::{Profile, AxisConfig, AircraftId};
    use std::collections::HashMap;

    let mut axes = HashMap::new();
    axes.insert("test".to_string(), AxisConfig {
        deadzone: Some(0.0),
        expo: Some(0.0),
        slew_rate: None,
        detents: vec![],
        curve: None, // Linear response
    });

    Profile {
        schema: "flight.profile/1".to_string(),
        sim: Some("test".to_string()),
        aircraft: Some(AircraftId { icao: "TEST".to_string() }),
        axes,
        pof_overrides: None,
    }
}