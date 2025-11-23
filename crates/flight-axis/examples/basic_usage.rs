//! Basic usage example for flight-axis crate
//!
//! Demonstrates the core functionality including:
//! - Creating an axis engine
//! - Building a processing pipeline
//! - Processing frames with atomic pipeline swaps
//! - Monitoring performance counters

use flight_axis::{AxisEngine, AxisFrame, EngineConfig, PipelineBuilder, UpdateResult};
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Flight Axis Engine Demo");
    println!("======================");

    // Create engine with RT checks enabled
    let config = EngineConfig {
        enable_rt_checks: true,
        max_frame_time_us: 500,
        enable_counters: true,
        enable_conflict_detection: true,
        conflict_detector_config: Default::default(),
    };
    let engine = AxisEngine::with_config("demo_axis".to_string(), config);

    // Build a processing pipeline
    println!("\n1. Building processing pipeline...");
    let pipeline = PipelineBuilder::new()
        .deadzone(0.05) // 5% deadzone
        .curve(0.2)? // 20% exponential curve
        .slew(2.0) // 2 units/second slew rate
        .compile()?;

    println!(
        "   Pipeline compiled with {} nodes",
        pipeline.metadata().len()
    );
    for (i, meta) in pipeline.metadata().iter().enumerate() {
        println!(
            "   Node {}: {} (state: {} bytes)",
            i + 1,
            meta.node_type,
            meta.state_size
        );
    }

    // Apply pipeline to engine
    println!("\n2. Applying pipeline to engine...");
    let result = engine.update_pipeline(pipeline);
    match result {
        UpdateResult::Pending => println!("   Pipeline update pending..."),
        UpdateResult::Success => println!("   Pipeline applied successfully!"),
        UpdateResult::Failed(err) => println!("   Pipeline failed: {}", err),
        UpdateResult::Rejected(err) => println!("   Pipeline rejected: {}", err),
    }

    // Process some frames to activate the pipeline
    println!("\n3. Processing frames...");
    let start_time = 1_000_000_000u64; // 1 second in nanoseconds
    let frame_interval = 4_000_000u64; // 4ms = 250Hz

    let test_inputs = [
        0.0,  // Zero input
        0.03, // Within deadzone
        0.1,  // Outside deadzone
        0.5,  // Mid-range
        1.0,  // Maximum
        -0.5, // Negative
        -1.0, // Negative maximum
    ];

    for (i, &input) in test_inputs.iter().enumerate() {
        let mut frame = AxisFrame::new(input, start_time + i as u64 * frame_interval);

        let process_start = Instant::now();
        engine.process(&mut frame)?;
        let process_time = process_start.elapsed();

        println!(
            "   Frame {}: {:.3} -> {:.3} (processed in {:?})",
            i + 1,
            input,
            frame.out,
            process_time
        );
    }

    // Check if pipeline was activated
    if engine.has_active_pipeline() {
        println!(
            "   ✓ Pipeline activated (version: {})",
            engine.active_version().unwrap_or(0)
        );
        println!("   ✓ Swap acknowledgments: {}", engine.swap_ack_count());
    }

    // Display performance counters
    println!("\n4. Performance Statistics:");
    let counters = engine.counters();
    let snapshot = counters.snapshot();

    println!("   Frames processed: {}", snapshot.frames_processed);
    println!("   Pipeline swaps: {}", snapshot.pipeline_swaps);
    println!("   Deadline misses: {}", snapshot.deadline_misses);
    println!("   RT violations: {}", snapshot.rt_violations);
    println!("   Max frame time: {}μs", snapshot.max_frame_time_us);
    println!("   Avg frame time: {}μs", snapshot.avg_frame_time_us);
    println!("   Jitter p99 estimate: {}μs", snapshot.jitter_p99_us);

    // Validate performance requirements
    println!("\n5. Requirements Validation:");
    let max_time_ok = snapshot.max_frame_time_us < 500;
    let no_violations = snapshot.rt_violations == 0;
    let no_deadline_misses = snapshot.deadline_misses == 0;

    println!("   ✓ Max processing time < 500μs: {}", max_time_ok);
    println!("   ✓ Zero RT violations: {}", no_violations);
    println!("   ✓ Zero deadline misses: {}", no_deadline_misses);

    if max_time_ok && no_violations && no_deadline_misses {
        println!("   🎉 All AX-01 requirements met!");
    } else {
        println!("   ⚠️  Some requirements not met");
    }

    // Demonstrate atomic pipeline swap
    println!("\n6. Demonstrating atomic pipeline swap...");
    let new_pipeline = PipelineBuilder::new()
        .deadzone(0.1) // Different deadzone
        .curve(0.3)? // Different curve
        .compile()?;

    let initial_version = engine.active_version();
    let result = engine.update_pipeline(new_pipeline);
    println!("   Update result: {:?}", result);

    // Process a frame to trigger the swap
    let mut frame = AxisFrame::new(0.5, start_time + 1000 * frame_interval);
    engine.process(&mut frame)?;

    let new_version = engine.active_version();
    if new_version != initial_version {
        println!("   ✓ Pipeline swapped atomically!");
        println!("   Version: {:?} -> {:?}", initial_version, new_version);
        println!("   Swap count: {}", engine.swap_ack_count());
    }

    println!("\n✅ Demo completed successfully!");
    Ok(())
}
