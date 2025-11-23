//! Integration tests for detent mapper within complete pipelines
//!
//! Tests the detent mapper working together with other pipeline nodes
//! and validates end-to-end behavior.

use crossbeam::channel;
use flight_axis::{
    AxisEngine, AxisFrame, DetentEvent, DetentRole, DetentZone, PipelineBuilder, UpdateResult,
};
use std::time::Duration;

#[test]
fn test_detent_in_complete_pipeline() {
    // Create a realistic flight control pipeline with detents
    let zones = vec![
        DetentZone::new(-0.8, 0.05, 0.02, DetentRole::Reverse),
        DetentZone::new(0.0, 0.1, 0.03, DetentRole::Idle),
        DetentZone::new(0.7, 0.08, 0.025, DetentRole::Takeoff),
    ];

    let pipeline = PipelineBuilder::new()
        .deadzone(0.02) // Remove small inputs
        .curve(0.15)
        .unwrap() // Apply exponential curve
        .detent(zones) // Apply detent mapping
        .slew(2.0) // Rate limit output
        .compile()
        .expect("Pipeline should compile");

    let engine = AxisEngine::new();

    // Apply the pipeline
    match engine.update_pipeline(pipeline) {
        UpdateResult::Pending => {}
        other => panic!("Expected Pending, got {:?}", other),
    }

    // Test sequence: move through detents
    let test_sequence = vec![
        (-0.9, "Outside reverse"),
        (-0.78, "Enter reverse detent"),
        (-0.75, "Stay in reverse"),
        (-0.5, "Exit reverse, move toward idle"),
        (0.05, "Enter idle detent"),
        (0.0, "At idle center"),
        (0.12, "Stay in idle (hysteresis)"),
        (0.4, "Exit idle, move toward takeoff"),
        (0.72, "Enter takeoff detent"),
        (0.7, "At takeoff center"),
        (0.9, "Exit takeoff"),
    ];

    for (input, description) in test_sequence {
        let mut frame = AxisFrame::new(input, 1000000);
        let result = engine.process(&mut frame);

        println!(
            "{}: input={:.3}, output={:.3}",
            description, input, frame.out
        );

        // Verify processing succeeded
        assert!(result.is_ok());
        assert!(frame.ts_mono_ns > 0);
    }

    // Verify performance counters
    let counters = engine.counters();
    assert!(counters.frames_processed() > 0, "No frames processed");
    assert_eq!(
        counters.rt_allocations(),
        0,
        "Allocations detected in hot path"
    );
}

#[test]
fn test_detent_events_in_pipeline() {
    let (sender, receiver) = channel::unbounded();

    let zones = vec![
        DetentZone::new(0.0, 0.1, 0.05, DetentRole::Idle),
        DetentZone::new(0.5, 0.08, 0.03, DetentRole::Takeoff),
    ];

    // Create detent node with event channel
    let detent_node = flight_axis::DetentNode::new(zones).with_event_sender(sender);

    let pipeline = PipelineBuilder::new()
        .deadzone(0.03)
        .add_node(detent_node)
        .compile()
        .expect("Pipeline should compile");

    let engine = AxisEngine::new();
    engine.update_pipeline(pipeline);

    // Process frames that should generate events
    let frames = vec![
        AxisFrame::new(-0.5, 1000), // Outside detents
        AxisFrame::new(0.05, 2000), // Enter idle
        AxisFrame::new(0.0, 3000),  // Stay in idle
        AxisFrame::new(0.3, 4000),  // Exit idle
        AxisFrame::new(0.52, 5000), // Enter takeoff
        AxisFrame::new(0.8, 6000),  // Exit takeoff
    ];

    for mut frame in frames {
        engine.process(&mut frame);
    }

    // Collect events
    let mut events = Vec::new();
    while let Ok(event) = receiver.try_recv() {
        events.push(event);
    }

    // Should have: enter idle, exit idle, enter takeoff, exit takeoff
    assert_eq!(events.len(), 4);

    assert_eq!(events[0].to_detent, Some(DetentRole::Idle));
    assert_eq!(events[1].from_detent, Some(DetentRole::Idle));
    assert_eq!(events[2].to_detent, Some(DetentRole::Takeoff));
    assert_eq!(events[3].from_detent, Some(DetentRole::Takeoff));
}

#[test]
fn test_detent_output_snapping() {
    let zones = vec![
        DetentZone::new(0.0, 0.1, 0.05, DetentRole::Idle),
        DetentZone::no_snap(0.5, 0.08, 0.03, DetentRole::Custom(1)),
    ];

    let pipeline = PipelineBuilder::new()
        .detent(zones)
        .compile()
        .expect("Pipeline should compile");

    let engine = AxisEngine::new();
    engine.update_pipeline(pipeline);

    // Test snapping detent
    let mut frame1 = AxisFrame::new(0.08, 1000);
    engine.process(&mut frame1);
    assert_eq!(frame1.out, 0.0, "Should snap to idle center");

    // Test non-snapping detent
    let mut frame2 = AxisFrame::new(0.52, 2000);
    engine.process(&mut frame2);
    assert_eq!(frame2.out, 0.52, "Should not snap to center");
}

#[test]
fn test_detent_with_curve_interaction() {
    // Test how detents interact with exponential curves
    let zones = vec![DetentZone::new(0.0, 0.15, 0.05, DetentRole::Idle)];

    let pipeline = PipelineBuilder::new()
        .curve(0.3)
        .unwrap() // Apply curve first
        .detent(zones) // Then detents
        .compile()
        .expect("Pipeline should compile");

    let engine = AxisEngine::new();
    engine.update_pipeline(pipeline);

    // Input that would be curved but then snapped by detent
    let mut frame = AxisFrame::new(0.1, 1000);
    engine.process(&mut frame);

    // The curve would transform 0.1 to something else, but detent should snap to 0.0
    assert_eq!(frame.out, 0.0, "Detent should override curve output");
}

#[test]
fn test_detent_hysteresis_with_slew() {
    // Test interaction between detent hysteresis and slew limiting
    let zones = vec![
        DetentZone::new(0.0, 0.1, 0.1, DetentRole::Idle), // Large hysteresis
    ];

    let pipeline = PipelineBuilder::new()
        .detent(zones)
        .slew(0.5) // Slow slew rate
        .compile()
        .expect("Pipeline should compile");

    let engine = AxisEngine::new();
    engine.update_pipeline(pipeline);

    // Enter detent
    let mut frame1 = AxisFrame::new(0.05, 1000000);
    engine.process(&mut frame1);
    assert_eq!(frame1.out, 0.0, "Should snap to detent center");

    // Move to edge of hysteresis band - should stay snapped but slew might affect it
    let mut frame2 = AxisFrame::new(0.18, 2000000);
    engine.process(&mut frame2);

    // The detent should keep us at center, but slew might try to move us
    // This tests the interaction between these two systems
    println!("Hysteresis edge result: {:.3}", frame2.out);
}

#[test]
fn test_multiple_detent_transitions() {
    let zones = vec![
        DetentZone::new(-0.5, 0.08, 0.02, DetentRole::Reverse),
        DetentZone::new(0.0, 0.1, 0.03, DetentRole::Idle),
        DetentZone::new(0.3, 0.06, 0.02, DetentRole::Climb),
        DetentZone::new(0.7, 0.08, 0.025, DetentRole::Takeoff),
    ];

    let pipeline = PipelineBuilder::new()
        .deadzone(0.02)
        .detent(zones)
        .compile()
        .expect("Pipeline should compile");

    let engine = AxisEngine::new();
    engine.update_pipeline(pipeline);

    // Sweep through all detents
    let positions = vec![
        -0.8, -0.52, -0.48, -0.2, 0.05, 0.0, 0.15, 0.32, 0.28, 0.5, 0.72, 0.68, 0.9,
    ];

    let mut outputs = Vec::new();
    for (i, pos) in positions.iter().enumerate() {
        let mut frame = AxisFrame::new(*pos, (i * 100000) as u64);
        engine.process(&mut frame);
        outputs.push(frame.out);
    }

    // Verify we hit all the detent centers
    assert!(outputs.contains(&-0.5), "Should hit reverse detent");
    assert!(outputs.contains(&0.0), "Should hit idle detent");
    assert!(outputs.contains(&0.3), "Should hit climb detent");
    assert!(outputs.contains(&0.7), "Should hit takeoff detent");

    // Verify performance
    let counters = engine.counters();
    assert!(counters.frames_processed() > 0, "No frames processed");
}

#[test]
fn test_detent_builder_api() {
    // Test the builder API for single detents
    let pipeline = PipelineBuilder::new()
        .deadzone(0.03)
        .single_detent(0.0, 0.1, 0.05, DetentRole::Idle)
        .slew(1.0)
        .compile()
        .expect("Pipeline should compile");

    let engine = AxisEngine::new();
    engine.update_pipeline(pipeline);

    let mut frame = AxisFrame::new(0.08, 1000);
    engine.process(&mut frame);
    assert_eq!(frame.out, 0.0, "Single detent should work via builder API");
}

#[test]
fn test_detent_performance_in_pipeline() {
    // Performance test with realistic pipeline
    let zones = vec![
        DetentZone::new(-0.8, 0.05, 0.02, DetentRole::Reverse),
        DetentZone::new(-0.3, 0.08, 0.025, DetentRole::Taxi),
        DetentZone::new(0.0, 0.1, 0.03, DetentRole::Idle),
        DetentZone::new(0.4, 0.08, 0.025, DetentRole::Climb),
        DetentZone::new(0.7, 0.06, 0.02, DetentRole::Takeoff),
        DetentZone::new(0.95, 0.04, 0.01, DetentRole::Emergency),
    ];

    let pipeline = PipelineBuilder::new()
        .deadzone(0.02)
        .curve(0.2)
        .unwrap()
        .detent(zones)
        .slew(2.0)
        .compile()
        .expect("Pipeline should compile");

    let engine = AxisEngine::new();
    engine.update_pipeline(pipeline);

    // Process many frames to test sustained performance
    let start = std::time::Instant::now();
    for i in 0..10000 {
        let pos = (i as f32 * 0.0002) % 2.0 - 1.0; // Sweep -1 to 1
        let mut frame = AxisFrame::new(pos, (i * 1000) as u64);
        engine.process(&mut frame);
    }
    let elapsed = start.elapsed();

    println!(
        "Processed 10k frames in {:?} ({:.2}μs/frame)",
        elapsed,
        elapsed.as_micros() as f64 / 10000.0
    );

    // Verify performance requirements
    let counters = engine.counters();
    assert!(counters.frames_processed() > 0, "No frames processed");
    assert_eq!(counters.rt_allocations(), 0, "Allocations detected");

    // Should complete in reasonable time (well under 1ms per frame)
    assert!(
        elapsed < Duration::from_millis(100),
        "Processing took too long: {:?}",
        elapsed
    );
}
