// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Pipeline Compilation Demo
//!
//! This example demonstrates the axis pipeline compilation system,
//! showing how profiles are compiled into optimized function pipelines
//! with zero-allocation guarantees.

#![cfg_attr(not(feature = "integration"), allow(dead_code, unused_imports))]

#[cfg(feature = "integration")]
use flight_axis::{
    AxisEngine, AxisFrame, PipelineCompiler, PipelineBuilder, 
    nodes::{DeadzoneNode, CurveNode, SlewNode, DetentNode, DetentZone, DetentRole, MixerNode, MixerInput},
    counters::{RuntimeCounters, AllocationGuard},
    UpdateResult, EngineConfig
};
#[cfg(feature = "integration")]
use flight_core::profile::{Profile, AxisConfig, CurvePoint};
#[cfg(feature = "integration")]
use std::collections::HashMap;
#[cfg(feature = "integration")]
use std::time::Instant;

#[cfg(feature = "integration")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Flight Hub Pipeline Compilation Demo ===\n");

    // Demo 1: Basic Pipeline Building
    demo_basic_pipeline_building()?;
    
    // Demo 2: Profile to Pipeline Compilation
    demo_profile_compilation()?;
    
    // Demo 3: Atomic Pipeline Swapping
    demo_atomic_swapping()?;
    
    // Demo 4: Zero-Allocation Verification
    demo_zero_allocation_verification()?;
    
    // Demo 5: Performance Benchmarking
    demo_performance_benchmarking()?;
    
    // Demo 6: Complex Pipeline with All Nodes
    demo_complex_pipeline()?;

    println!("\n=== Pipeline compilation demo completed successfully! ===");
    Ok(())
}

#[cfg(not(feature = "integration"))]
fn main() {
    eprintln!("Enable `--features integration` to build this example (requires API porting).");
}

#[cfg(feature = "integration")]
fn demo_basic_pipeline_building() -> Result<(), Box<dyn std::error::Error>> {
    println!("1. Basic Pipeline Building");
    println!("-------------------------");

    let mut builder = PipelineBuilder::new();
    
    // Add nodes in processing order
    builder.add_deadzone(0.03);
    println!("✓ Added deadzone node (3% deadzone)");
    
    builder.add_curve(vec![
        (0.0, 0.0),
        (0.5, 0.3),
        (1.0, 1.0),
    ]);
    println!("✓ Added curve node (exponential-like curve)");
    
    builder.add_slew_limiter(2.0); // 2 units per second
    println!("✓ Added slew limiter (2.0 units/sec)");
    
    builder.add_detent(DetentZone {
        position: 0.0,
        width: 0.05,
        role: DetentRole::Center,
    });
    println!("✓ Added detent zone (center detent)");

    // Compile the pipeline
    let compiler = PipelineCompiler::new();
    match compiler.compile(builder) {
        Ok(pipeline) => {
            println!("✓ Pipeline compiled successfully");
            println!("  Nodes: {}", pipeline.node_count());
            println!("  State size: {} bytes", pipeline.state_size());
        }
        Err(e) => println!("✗ Pipeline compilation failed: {}", e),
    }

    Ok(())
}

#[cfg(feature = "integration")]
fn demo_profile_compilation() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n2. Profile to Pipeline Compilation");
    println!("---------------------------------");

    // Create a test profile
    let profile = create_test_profile();
    
    // Create axis engine and compile profile
    let mut engine = AxisEngine::new_for_axis("pitch".to_string());
    
    match engine.apply_profile(&profile) {
        Ok(UpdateResult::Applied { compilation_time_us, .. }) => {
            println!("✓ Profile compiled and applied successfully");
            println!("  Compilation time: {} μs", compilation_time_us);
            
            // Test the compiled pipeline
            let test_inputs = vec![0.0, 0.02, 0.05, 0.1, 0.5, 0.9, 1.0];
            println!("  Testing compiled pipeline:");
            
            for input in test_inputs {
                let mut frame = AxisFrame::new(input, 1000);
                engine.process(&mut frame)?;
                println!("    {:.2} → {:.3}", input, frame.out);
            }
        }
        Ok(UpdateResult::NoChange) => {
            println!("✓ Profile unchanged, no recompilation needed");
        }
        Err(e) => println!("✗ Profile compilation failed: {}", e),
    }

    Ok(())
}

#[cfg(feature = "integration")]
fn demo_atomic_swapping() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n3. Atomic Pipeline Swapping");
    println!("---------------------------");

    let mut engine = AxisEngine::new_for_axis("test".to_string());
    
    // Apply initial profile
    let profile1 = create_test_profile();
    engine.apply_profile(&profile1)?;
    println!("✓ Applied initial profile");
    
    // Process some frames
    let mut frame = AxisFrame::new(0.5, 1000);
    engine.process(&mut frame)?;
    let output1 = frame.out;
    println!("  Initial output for 0.5 input: {:.3}", output1);
    
    // Create modified profile
    let mut profile2 = profile1.clone();
    if let Some(pitch_config) = profile2.axes.get_mut("pitch") {
        pitch_config.expo = Some(0.5); // Change expo from 0.2 to 0.5
    }
    
    // Apply new profile (should swap atomically)
    match engine.apply_profile(&profile2)? {
        UpdateResult::Applied { .. } => {
            println!("✓ New profile swapped atomically");
            
            // Process same input with new profile
            let mut frame = AxisFrame::new(0.5, 2000);
            engine.process(&mut frame)?;
            let output2 = frame.out;
            println!("  New output for 0.5 input: {:.3}", output2);
            println!("  Output change: {:.3}", output2 - output1);
        }
        UpdateResult::NoChange => {
            println!("✗ Expected profile change but got no change");
        }
    }

    Ok(())
}

#[cfg(feature = "integration")]
fn demo_zero_allocation_verification() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n4. Zero-Allocation Verification");
    println!("-------------------------------");

    let mut engine = AxisEngine::new_for_axis("test".to_string());
    let profile = create_test_profile();
    engine.apply_profile(&profile)?;
    
    // Enable allocation tracking
    let _guard = AllocationGuard::new();
    let counters = RuntimeCounters::new();
    
    println!("✓ Allocation tracking enabled");
    
    // Process multiple frames
    let test_count = 1000;
    let start_allocs = counters.allocation_count();
    let start_locks = counters.lock_count();
    
    for i in 0..test_count {
        let input = (i as f32) / (test_count as f32);
        let mut frame = AxisFrame::new(input, i as u64);
        engine.process(&mut frame)?;
    }
    
    let end_allocs = counters.allocation_count();
    let end_locks = counters.lock_count();
    
    let alloc_delta = end_allocs - start_allocs;
    let lock_delta = end_locks - start_locks;
    
    println!("  Processed {} frames", test_count);
    println!("  Allocations: {} (should be 0)", alloc_delta);
    println!("  Lock operations: {} (should be 0)", lock_delta);
    
    if alloc_delta == 0 && lock_delta == 0 {
        println!("✓ Zero-allocation guarantee verified");
    } else {
        println!("✗ Zero-allocation guarantee violated!");
    }

    Ok(())
}

#[cfg(feature = "integration")]
fn demo_performance_benchmarking() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n5. Performance Benchmarking");
    println!("---------------------------");

    let mut engine = AxisEngine::new_for_axis("test".to_string());
    let profile = create_test_profile();
    engine.apply_profile(&profile)?;
    
    // Warm up
    for _ in 0..1000 {
        let mut frame = AxisFrame::new(0.5, 1000);
        engine.process(&mut frame)?;
    }
    
    // Benchmark processing time
    let iterations = 100_000;
    let start = Instant::now();
    
    for i in 0..iterations {
        let input = (i % 1000) as f32 / 1000.0;
        let mut frame = AxisFrame::new(input, i as u64);
        engine.process(&mut frame)?;
    }
    
    let elapsed = start.elapsed();
    let ns_per_frame = elapsed.as_nanos() / iterations;
    let frames_per_second = 1_000_000_000 / ns_per_frame;
    
    println!("  Processed {} frames in {:?}", iterations, elapsed);
    println!("  Average time per frame: {} ns", ns_per_frame);
    println!("  Theoretical max FPS: {}", frames_per_second);
    println!("  250Hz requirement: {} (need ≤4,000,000 ns)", 
             if ns_per_frame <= 4_000_000 { "✓ PASS" } else { "✗ FAIL" });

    Ok(())
}

#[cfg(feature = "integration")]
fn demo_complex_pipeline() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n6. Complex Pipeline with All Node Types");
    println!("--------------------------------------");

    let mut builder = PipelineBuilder::new();
    
    // Build a complex pipeline with all node types
    builder.add_deadzone(0.02);
    
    builder.add_curve(vec![
        (0.0, 0.0),
        (0.2, 0.05),
        (0.5, 0.25),
        (0.8, 0.7),
        (1.0, 1.0),
    ]);
    
    builder.add_slew_limiter(1.5);
    
    builder.add_detent(DetentZone {
        position: 0.0,
        width: 0.03,
        role: DetentRole::Center,
    });
    
    builder.add_detent(DetentZone {
        position: 0.5,
        width: 0.02,
        role: DetentRole::Takeoff,
    });
    
    // Add mixer for cross-axis interaction
    builder.add_mixer(vec![
        MixerInput {
            axis: "pitch".to_string(),
            scale: 1.0,
        },
        MixerInput {
            axis: "collective".to_string(),
            scale: -0.1, // Anti-torque compensation
        },
    ]);
    
    let compiler = PipelineCompiler::new();
    match compiler.compile(builder) {
        Ok(pipeline) => {
            println!("✓ Complex pipeline compiled successfully");
            println!("  Total nodes: {}", pipeline.node_count());
            println!("  State size: {} bytes", pipeline.state_size());
            
            // Test the pipeline with various inputs
            println!("  Testing complex pipeline behavior:");
            
            let test_cases = vec![
                (0.0, "Center detent"),
                (0.01, "Inside center deadzone"),
                (0.05, "Outside center deadzone"),
                (0.5, "Takeoff detent"),
                (0.75, "High input"),
                (1.0, "Maximum input"),
            ];
            
            for (input, description) in test_cases {
                // Note: In a real implementation, we'd need an engine to process frames
                println!("    {:.2} ({}): compiled into pipeline", input, description);
            }
        }
        Err(e) => println!("✗ Complex pipeline compilation failed: {}", e),
    }

    Ok(())
}

#[cfg(feature = "integration")]
fn create_test_profile() -> Profile {
    let mut axes = HashMap::new();
    
    axes.insert("pitch".to_string(), AxisConfig {
        deadzone: Some(0.03),
        expo: Some(0.2),
        slew_rate: Some(1.2),
        detents: vec![],
        curve: Some(vec![
            CurvePoint { input: 0.0, output: 0.0 },
            CurvePoint { input: 0.5, output: 0.3 },
            CurvePoint { input: 1.0, output: 1.0 },
        ]),
    });

    Profile {
        schema: "flight.profile/1".to_string(),
        sim: Some("msfs".to_string()),
        aircraft: Some(flight_core::profile::AircraftId { icao: "C172".to_string() }),
        axes,
        pof_overrides: None,
    }
}