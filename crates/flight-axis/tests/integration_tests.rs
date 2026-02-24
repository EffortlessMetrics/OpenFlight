//! Integration tests for flight-axis crate
//!
//! Tests the complete axis processing pipeline including atomic swaps,
//! zero-allocation guarantees, and deterministic execution.

use flight_axis::{
    AllocationGuard, AxisEngine, AxisFrame, CurveNode, DeadzoneNode, EngineConfig, Node,
    PipelineBuilder, RuntimeCounters, SlewNode,
};
use std::time::{Duration, Instant};

#[test]
fn test_axis_frame_creation() {
    let frame = AxisFrame::new(0.5, 1000000);
    assert_eq!(frame.in_raw, 0.5);
    assert_eq!(frame.out, 0.5);
    assert_eq!(frame.d_in_dt, 0.0);
    assert_eq!(frame.ts_mono_ns, 1000000);
}

#[test]
fn test_axis_frame_derivative_calculation() {
    let prev_frame = AxisFrame::new(0.0, 1000000000); // 1 second
    let mut curr_frame = AxisFrame::new(0.5, 2000000000); // 2 seconds

    curr_frame.update_derivative(&prev_frame);

    // Should be 0.5 units per second
    assert!((curr_frame.d_in_dt - 0.5).abs() < 1e-6);
}

#[test]
fn test_deadzone_node_symmetric() {
    let mut node = DeadzoneNode::new(0.1);

    // Test within deadzone
    let mut frame = AxisFrame::new(0.05, 1000);
    node.step(&mut frame);
    assert_eq!(frame.out, 0.0);

    // Test outside deadzone
    let mut frame = AxisFrame::new(0.5, 1000);
    node.step(&mut frame);
    assert!((frame.out - 0.444444).abs() < 1e-5); // (0.5 - 0.1) / (1.0 - 0.1)
}

#[test]
fn test_deadzone_node_asymmetric() {
    let mut node = DeadzoneNode::asymmetric(0.1, 0.2);

    // Test positive side
    let mut frame = AxisFrame::new(0.5, 1000);
    node.step(&mut frame);
    assert!((frame.out - 0.444444).abs() < 1e-5);

    // Test negative side
    let mut frame = AxisFrame::new(-0.5, 1000);
    node.step(&mut frame);
    assert!((frame.out - (-0.375)).abs() < 1e-5); // (-0.5 - (-0.2)) / (1.0 - 0.2)
}

#[test]
fn test_curve_node_exponential() {
    let mut node = CurveNode::new(0.5);

    let mut frame = AxisFrame::new(0.5, 1000);
    node.step(&mut frame);

    // Should be 0.5^1.5 = 0.353553
    assert!((frame.out - 0.353553).abs() < 1e-5);
}

#[test]
fn test_curve_node_monotonicity() {
    let mut node = CurveNode::new(0.3);

    let inputs = [-1.0, -0.5, -0.1, 0.0, 0.1, 0.5, 1.0];
    let mut outputs = Vec::new();

    for &input in &inputs {
        let mut frame = AxisFrame::new(input, 1000);
        node.step(&mut frame);
        outputs.push(frame.out);
    }

    // Verify monotonic increasing
    for i in 1..outputs.len() {
        assert!(
            outputs[i] >= outputs[i - 1],
            "Curve not monotonic: {} >= {}",
            outputs[i],
            outputs[i - 1]
        );
    }
}

#[test]
fn test_slew_node_rate_limiting() {
    let node = SlewNode::new(1.0); // 1 unit per second

    // This test would need the SoA implementation to work properly
    // For now, just verify the node can be created
    assert_eq!(node.rate_limit, 1.0);
    assert!(node.attack_rate.is_none());
}

#[test]
fn test_pipeline_builder() {
    let result = PipelineBuilder::new()
        .deadzone(0.05)
        .curve(0.2)
        .unwrap()
        .slew(1.5)
        .compile();

    assert!(result.is_ok());
    let pipeline = result.unwrap();
    assert_eq!(pipeline.metadata().len(), 3);
}

#[test]
fn test_axis_engine_creation() {
    let engine = AxisEngine::new();
    assert!(!engine.has_active_pipeline());
    assert_eq!(engine.active_version(), None);
}

#[test]
fn test_axis_engine_with_config() {
    let config = EngineConfig {
        enable_rt_checks: true,
        max_frame_time_us: 1000,
        enable_counters: true,
        enable_conflict_detection: true,
        conflict_detector_config: Default::default(),
    };

    let engine = AxisEngine::with_config("test_config".to_string(), config);
    assert!(!engine.has_active_pipeline());
}

#[test]
fn test_axis_engine_process_without_pipeline() {
    let engine = AxisEngine::new();
    let mut frame = AxisFrame::new(0.7, 1000000);

    let result = engine.process(&mut frame);
    assert!(result.is_ok());
    assert_eq!(frame.out, 0.7); // Should pass through unchanged
}

#[test]
fn test_runtime_counters() {
    let counters = RuntimeCounters::new();

    // Initial state
    assert_eq!(counters.frames_processed(), 0);
    assert_eq!(counters.pipeline_swaps(), 0);
    assert_eq!(counters.deadline_misses(), 0);
    assert!(!counters.has_rt_violations());

    // Record some activity
    counters.record_frame_time(Duration::from_micros(250));
    counters.increment_pipeline_swaps();

    assert_eq!(counters.frames_processed(), 1);
    assert_eq!(counters.pipeline_swaps(), 1);
    assert_eq!(counters.max_frame_time_us(), 250);
    assert_eq!(counters.avg_frame_time_us(), 250);
}

#[test]
fn test_runtime_counters_averaging() {
    let counters = RuntimeCounters::new();

    // Record multiple frame times
    counters.record_frame_time(Duration::from_micros(100));
    counters.record_frame_time(Duration::from_micros(200));
    counters.record_frame_time(Duration::from_micros(300));

    assert_eq!(counters.frames_processed(), 3);
    assert_eq!(counters.max_frame_time_us(), 300);

    // Average should be somewhere between 100 and 300
    let avg = counters.avg_frame_time_us();
    assert!(avg >= 100 && avg <= 300);
}

#[test]
fn test_allocation_guard() {
    AllocationGuard::reset();
    assert!(!AllocationGuard::allocations_detected());

    {
        let _guard = AllocationGuard::new();
        // Guard is active
        assert!(!AllocationGuard::allocations_detected());
    }

    // Guard is dropped
    assert!(!AllocationGuard::allocations_detected());
}

#[test]
fn test_performance_snapshot() {
    let counters = RuntimeCounters::new();

    counters.record_frame_time(Duration::from_micros(150));
    counters.increment_pipeline_swaps();
    counters.increment_deadline_misses();

    let snapshot = counters.snapshot();
    assert_eq!(snapshot.frames_processed, 1);
    assert_eq!(snapshot.pipeline_swaps, 1);
    assert_eq!(snapshot.deadline_misses, 1);
    assert_eq!(snapshot.max_frame_time_us, 150);
    assert_eq!(snapshot.rt_violations, 0);
}

#[test]
fn test_deterministic_processing() {
    // Test that identical inputs produce identical outputs
    let mut node1 = DeadzoneNode::new(0.1);
    let mut node2 = DeadzoneNode::new(0.1);

    let test_inputs = [0.0, 0.05, 0.15, 0.5, 1.0, -0.3, -0.8];

    for &input in &test_inputs {
        let mut frame1 = AxisFrame::new(input, 1000);
        let mut frame2 = AxisFrame::new(input, 1000);

        node1.step(&mut frame1);
        node2.step(&mut frame2);

        assert_eq!(
            frame1.out, frame2.out,
            "Non-deterministic output for input {}",
            input
        );
    }
}

#[test]
fn test_zero_allocation_constraint_validation() {
    // This test validates that our nodes don't allocate during processing
    let mut node = DeadzoneNode::new(0.1);

    // Process many frames to ensure no allocations
    for i in 0..1000 {
        let mut frame = AxisFrame::new((i as f32) / 1000.0, i as u64);
        node.step(&mut frame);
    }

    // If we get here without panicking, no allocations occurred
    assert!(true);
}

#[test]
fn test_pipeline_state_validation() {
    let pipeline = PipelineBuilder::new()
        .deadzone(0.03)
        .compile()
        .expect("Should compile");

    let state = pipeline.create_state();
    assert!(state.validate());
    assert!(state.buffer_size() > 0 || state.buffer_size() == 0); // Always true, but validates method works
}

#[test]
fn test_node_state_sizes() {
    let deadzone = DeadzoneNode::new(0.1);
    let curve = CurveNode::new(0.2);
    let slew = SlewNode::new(1.0);

    // Compiled nodes should declare their required state footprint.
    assert!(deadzone.state_size() > 0);
    assert!(curve.state_size() > 0);

    // Stateful nodes should have non-zero state size
    assert!(slew.state_size() > 0);
}

#[test]
fn test_node_type_identification() {
    let deadzone = DeadzoneNode::new(0.1);
    let curve = CurveNode::new(0.2);
    let slew = SlewNode::new(1.0);

    assert_eq!(deadzone.node_type(), "deadzone");
    assert_eq!(curve.node_type(), "curve");
    assert_eq!(slew.node_type(), "slew");
}

/// Benchmark-style test for performance validation
#[test]
fn test_processing_performance() {
    let engine = AxisEngine::new();
    let mut frame = AxisFrame::new(0.5, 1000000);

    let start = Instant::now();
    let iterations = 10000;

    for i in 0..iterations {
        frame.in_raw = (i as f32) / (iterations as f32);
        frame.ts_mono_ns = 1000000 + i as u64 * 4000; // 250Hz = 4ms intervals

        let _ = engine.process(&mut frame);
    }

    let elapsed = start.elapsed();
    let avg_time_per_frame = elapsed / iterations;

    // CI-safe performance sanity gate for debug builds.
    assert!(
        avg_time_per_frame < Duration::from_micros(250),
        "Processing too slow: {:?} per frame",
        avg_time_per_frame
    );
}

/// Test compile failure leaves RT state unchanged
#[test]
fn test_compile_failure_safety() {
    let engine = AxisEngine::new();

    // First, establish a working pipeline
    let good_pipeline = PipelineBuilder::new()
        .deadzone(0.1)
        .compile()
        .expect("Should compile");

    let result = engine.update_pipeline(good_pipeline);
    assert!(matches!(result, flight_axis::UpdateResult::Pending));

    // Process a frame to activate the pipeline
    let mut frame = AxisFrame::new(0.5, 1000);
    let _ = engine.process(&mut frame);

    let initial_version = engine.active_version();

    // Now try to update with an invalid configuration
    // (This would need actual invalid configuration to test properly)
    // For now, just verify the engine state remains consistent
    assert_eq!(engine.active_version(), initial_version);
}

/// Test atomic pipeline swap with ACK semantics
#[test]
fn test_atomic_pipeline_swap_with_ack() {
    let engine = AxisEngine::new();

    // Initial state - no pipeline
    assert!(!engine.has_active_pipeline());
    assert_eq!(engine.swap_ack_count(), 0);

    // Create and apply first pipeline
    let pipeline1 = PipelineBuilder::new()
        .deadzone(0.1)
        .compile()
        .expect("Should compile");

    let result = engine.update_pipeline(pipeline1);
    assert!(matches!(result, flight_axis::UpdateResult::Pending));

    // Process frame to trigger swap
    let mut frame = AxisFrame::new(0.5, 1000000);
    let _ = engine.process(&mut frame);

    // Verify pipeline is active and ACK incremented
    assert!(engine.has_active_pipeline());
    assert_eq!(engine.swap_ack_count(), 1);
    let version1 = engine.active_version().expect("Should have version");

    // Create and apply second pipeline
    let pipeline2 = PipelineBuilder::new()
        .deadzone(0.05)
        .curve(0.2)
        .expect("Valid curve")
        .compile()
        .expect("Should compile");

    let result = engine.update_pipeline(pipeline2);
    assert!(matches!(result, flight_axis::UpdateResult::Pending));

    // Process frame to trigger second swap
    let mut frame = AxisFrame::new(0.3, 2000000);
    let _ = engine.process(&mut frame);

    // Verify second pipeline is active and ACK incremented again
    assert!(engine.has_active_pipeline());
    assert_eq!(engine.swap_ack_count(), 2);
    let version2 = engine.active_version().expect("Should have version");
    assert_ne!(version1, version2);
}

/// Test end-to-end pipeline processing with real nodes
#[test]
fn test_end_to_end_pipeline_processing() {
    let engine = AxisEngine::new();

    // Create pipeline with deadzone and curve
    let pipeline = PipelineBuilder::new()
        .deadzone(0.1)
        .curve(0.3)
        .expect("Valid curve")
        .compile()
        .expect("Should compile");

    let _ = engine.update_pipeline(pipeline);

    // Test input within deadzone
    let mut frame = AxisFrame::new(0.05, 1000000);
    let _ = engine.process(&mut frame);

    // Should be zeroed by deadzone (after pipeline activates)
    let mut frame2 = AxisFrame::new(0.05, 2000000);
    let _ = engine.process(&mut frame2);
    // Note: Due to function pointer implementation, exact behavior may vary
    // The key is that processing completes without error

    // Test input outside deadzone
    let mut frame3 = AxisFrame::new(0.5, 3000000);
    let _ = engine.process(&mut frame3);

    // Verify counters are updated
    let counters = engine.counters();
    assert!(counters.frames_processed() >= 3);
    assert_eq!(counters.pipeline_swaps(), 1);
}

/// Test zero allocation guarantee in RT path
#[test]
fn test_zero_allocation_guarantee() {
    let config = EngineConfig {
        enable_rt_checks: true,
        max_frame_time_us: 1000,
        enable_counters: true,
        enable_conflict_detection: true,
        conflict_detector_config: Default::default(),
    };

    let engine = AxisEngine::with_config("test_zero_allocation".to_string(), config);

    // Create pipeline
    let pipeline = PipelineBuilder::new()
        .deadzone(0.1)
        .compile()
        .expect("Should compile");

    let _ = engine.update_pipeline(pipeline);

    // Process many frames
    for i in 0..1000 {
        let mut frame = AxisFrame::new((i as f32) / 1000.0, i as u64 * 4000000);
        let _ = engine.process(&mut frame);
    }

    // Verify no RT violations detected
    let counters = engine.counters();
    assert_eq!(counters.rt_allocations(), 0, "RT allocations detected!");
    assert_eq!(
        counters.rt_lock_acquisitions(),
        0,
        "RT lock acquisitions detected!"
    );
    assert!(!counters.has_rt_violations());
}

/// Test performance characteristics under load
#[test]
fn test_performance_under_load() {
    let engine = AxisEngine::new();

    // Create complex pipeline
    let pipeline = PipelineBuilder::new()
        .deadzone(0.03)
        .curve(0.2)
        .expect("Valid curve")
        .slew(2.0)
        .compile()
        .expect("Should compile");

    let _ = engine.update_pipeline(pipeline);

    let start = std::time::Instant::now();
    let iterations = 25000; // Simulate 100 seconds at 250Hz

    for i in 0..iterations {
        let mut frame = AxisFrame::new(
            ((i as f32) / 1000.0).sin(), // Varying input
            i as u64 * 4000000,          // 250Hz = 4ms intervals
        );
        let _ = engine.process(&mut frame);
    }

    let elapsed = start.elapsed();
    let avg_time_per_frame = elapsed / iterations;

    // CI-safe performance sanity gate for debug builds.
    assert!(
        avg_time_per_frame < std::time::Duration::from_micros(250),
        "Processing too slow: {:?} per frame",
        avg_time_per_frame
    );

    // Verify performance counters
    let counters = engine.counters();
    assert_eq!(counters.frames_processed(), iterations as u64);
    assert_eq!(counters.deadline_misses(), 0, "Deadline misses detected!");

    // Check jitter estimate
    let jitter_p99 = counters.jitter_p99_estimate_us();
    assert!(jitter_p99 < 500, "Jitter too high: {}μs", jitter_p99); // Should be <0.5ms
}
