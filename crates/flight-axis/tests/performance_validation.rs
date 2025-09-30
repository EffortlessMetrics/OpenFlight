//! Performance validation tests for flight-axis
//!
//! These tests validate that the implementation meets the strict timing
//! requirements specified in AX-01.

use flight_axis::{
    AxisEngine, AxisFrame, PipelineBuilder, EngineConfig
};
use std::time::Instant;

#[test]
fn test_rt_performance_requirements() {
    let config = EngineConfig {
        enable_rt_checks: true,
        max_frame_time_us: 500, // 0.5ms requirement
        enable_counters: true,
    };
    
    let engine = AxisEngine::with_config(config);
    
    // Create a complex pipeline to stress test
    let pipeline = PipelineBuilder::new()
        .deadzone(0.03)
        .curve(0.2).expect("Valid curve")
        .slew(2.0)
        .compile()
        .expect("Should compile");
    
    let _ = engine.update_pipeline(pipeline);
    
    // Warm up - process a few frames to activate pipeline
    for i in 0..10 {
        let mut frame = AxisFrame::new(0.5, i * 4000000);
        let _ = engine.process(&mut frame);
    }
    
    // Performance test - simulate 10 seconds at 250Hz
    let iterations = 2500;
    let mut max_time = std::time::Duration::ZERO;
    let mut total_time = std::time::Duration::ZERO;
    let mut times = Vec::with_capacity(iterations);
    
    for i in 0..iterations {
        let mut frame = AxisFrame::new(
            ((i as f32) / 1000.0).sin(), // Varying input
            i as u64 * 4000000 // 250Hz = 4ms intervals
        );
        
        let start = Instant::now();
        let _ = engine.process(&mut frame);
        let elapsed = start.elapsed();
        
        times.push(elapsed);
        total_time += elapsed;
        if elapsed > max_time {
            max_time = elapsed;
        }
    }
    
    // Calculate statistics
    let avg_time = total_time / iterations as u32;
    times.sort();
    let p99_index = (iterations as f32 * 0.99) as usize;
    let p99_time = times[p99_index.min(times.len() - 1)];
    
    println!("Performance Statistics:");
    println!("  Iterations: {}", iterations);
    println!("  Average time: {:?}", avg_time);
    println!("  Maximum time: {:?}", max_time);
    println!("  P99 time: {:?}", p99_time);
    
    // Validate requirements from AX-01
    assert!(p99_time < std::time::Duration::from_micros(500), 
            "P99 processing time {}μs exceeds 500μs requirement", 
            p99_time.as_micros());
    
    assert!(avg_time < std::time::Duration::from_micros(100), 
            "Average processing time {}μs too high", 
            avg_time.as_micros());
    
    // Verify counters
    let counters = engine.counters();
    assert_eq!(counters.deadline_misses(), 0, "Deadline misses detected!");
    assert_eq!(counters.rt_allocations(), 0, "RT allocations detected!");
    assert!(!counters.has_rt_violations());
    
    println!("✓ All RT performance requirements met");
}

#[test]
fn test_jitter_requirements() {
    let engine = AxisEngine::new();
    
    // Create pipeline
    let pipeline = PipelineBuilder::new()
        .deadzone(0.1)
        .compile()
        .expect("Should compile");
    
    let _ = engine.update_pipeline(pipeline);
    
    // Test processing time consistency (jitter)
    let iterations = 1000;
    let mut processing_times = Vec::with_capacity(iterations);
    
    // Warm up
    for i in 0..10 {
        let mut frame = AxisFrame::new(0.5, i * 4000000);
        let _ = engine.process(&mut frame);
    }
    
    // Measure processing times
    for i in 0..iterations {
        let mut frame = AxisFrame::new(
            ((i as f32) / 1000.0).sin(), // Varying input
            i as u64 * 4000000
        );
        
        let start = Instant::now();
        let _ = engine.process(&mut frame);
        let process_time = start.elapsed();
        
        processing_times.push(process_time);
    }
    
    // Calculate jitter as variation in processing times
    processing_times.sort();
    let min_time = processing_times[0];
    let max_time = processing_times[processing_times.len() - 1];
    let p99_index = (processing_times.len() as f32 * 0.99) as usize;
    let p99_time = processing_times[p99_index];
    
    // Jitter is the variation from minimum processing time
    let max_jitter = max_time - min_time;
    let p99_jitter = p99_time - min_time;
    
    println!("Processing Time Jitter Statistics:");
    println!("  Min time: {:?}", min_time);
    println!("  Max time: {:?}", max_time);
    println!("  P99 time: {:?}", p99_time);
    println!("  Max jitter: {:?}", max_jitter);
    println!("  P99 jitter: {:?}", p99_jitter);
    
    // Validate jitter requirement (≤0.5ms p99 variation)
    // This is more lenient since we're measuring processing consistency
    assert!(p99_jitter < std::time::Duration::from_micros(500), 
            "P99 processing jitter {}μs exceeds 500μs requirement", 
            p99_jitter.as_micros());
    
    // Also validate that processing is consistently fast
    assert!(p99_time < std::time::Duration::from_micros(500), 
            "P99 processing time {}μs exceeds 500μs requirement", 
            p99_time.as_micros());
    
    println!("✓ Processing jitter requirements met");
}

#[test]
fn test_zero_allocation_validation() {
    let config = EngineConfig {
        enable_rt_checks: true,
        max_frame_time_us: 1000,
        enable_counters: true,
    };
    
    let engine = AxisEngine::with_config(config);
    
    // Create pipeline with all node types
    let pipeline = PipelineBuilder::new()
        .deadzone(0.05)
        .curve(0.3).expect("Valid curve")
        .slew(1.5)
        .compile()
        .expect("Should compile");
    
    let _ = engine.update_pipeline(pipeline);
    
    // Process many frames to ensure no allocations
    for i in 0..10000 {
        let mut frame = AxisFrame::new(
            (i as f32 / 10000.0) * 2.0 - 1.0, // Range -1 to 1
            i as u64 * 4000000
        );
        
        let _ = engine.process(&mut frame);
    }
    
    // Verify zero allocations
    let counters = engine.counters();
    assert_eq!(counters.rt_allocations(), 0, 
               "RT allocations detected: {}", counters.rt_allocations());
    assert_eq!(counters.rt_lock_acquisitions(), 0, 
               "RT lock acquisitions detected: {}", counters.rt_lock_acquisitions());
    
    println!("✓ Zero allocation guarantee validated over 10,000 frames");
}

#[test]
fn test_atomic_swap_performance() {
    let engine = AxisEngine::new();
    
    // Create initial pipeline
    let pipeline1 = PipelineBuilder::new()
        .deadzone(0.1)
        .compile()
        .expect("Should compile");
    
    let _ = engine.update_pipeline(pipeline1);
    
    // Process frame to activate
    let mut frame = AxisFrame::new(0.5, 1000000);
    let _ = engine.process(&mut frame);
    
    let initial_ack = engine.swap_ack_count();
    
    // Measure swap performance
    let start = Instant::now();
    
    // Create and apply new pipeline
    let pipeline2 = PipelineBuilder::new()
        .deadzone(0.05)
        .curve(0.2).expect("Valid curve")
        .compile()
        .expect("Should compile");
    
    let _ = engine.update_pipeline(pipeline2);
    
    // Process frame to trigger swap
    let mut frame = AxisFrame::new(0.3, 2000000);
    let _ = engine.process(&mut frame);
    
    let swap_time = start.elapsed();
    
    // Verify swap completed
    assert_eq!(engine.swap_ack_count(), initial_ack + 1, "Swap not acknowledged");
    
    // Swap should be very fast (< 100μs)
    assert!(swap_time < std::time::Duration::from_micros(100), 
            "Atomic swap took {}μs, should be < 100μs", 
            swap_time.as_micros());
    
    println!("✓ Atomic swap completed in {:?}", swap_time);
}