//! Comprehensive tests for detent mapper functionality
//!
//! Tests cover:
//! - Single transition per boundary crossing
//! - Hysteresis behavior
//! - Event generation
//! - Deterministic behavior
//! - Property-based testing

use flight_axis::{
    AxisFrame, DetentNode, DetentZone, DetentRole, DetentEvent, DetentState, Node,
};
use crossbeam::channel;
use proptest::prelude::*;
use std::collections::HashMap;

/// Helper to create a test detent node with event channel
fn create_test_detent_node(zones: Vec<DetentZone>) -> (DetentNode, channel::Receiver<DetentEvent>) {
    let (sender, receiver) = channel::unbounded();
    let node = DetentNode::new(zones).with_event_sender(sender);
    (node, receiver)
}

/// Helper to process a frame through detent node with SoA state
unsafe fn process_frame_soa(
    node: &DetentNode,
    frame: &mut AxisFrame,
    state: &mut DetentState,
) {
    let state_ptr = state as *mut DetentState as *mut u8;
    node.step_soa(frame, state_ptr);
}

#[test]
fn test_detent_zone_creation() {
    let zone = DetentZone::new(0.5, 0.1, 0.02, DetentRole::Cruise);
    
    assert_eq!(zone.center, 0.5);
    assert_eq!(zone.half_width, 0.1);
    assert_eq!(zone.hysteresis, 0.02);
    assert_eq!(zone.role, DetentRole::Cruise);
    assert!(zone.snap_to_center);
    
    // Test bounds
    let (entry_min, entry_max) = zone.entry_bounds();
    assert_eq!(entry_min, 0.4);
    assert_eq!(entry_max, 0.6);
    
    let (exit_min, exit_max) = zone.exit_bounds();
    assert_eq!(exit_min, 0.38);
    assert_eq!(exit_max, 0.62);
}

#[test]
fn test_detent_zone_clamping() {
    // Test position clamping
    let zone = DetentZone::new(1.5, 0.1, 0.02, DetentRole::Emergency);
    assert_eq!(zone.center, 1.0); // Clamped to valid range
    
    // Test negative values
    let zone = DetentZone::new(-0.8, -0.1, -0.05, DetentRole::Reverse);
    assert_eq!(zone.half_width, 0.0); // Clamped to non-negative
    assert_eq!(zone.hysteresis, 0.0); // Clamped to non-negative
}

#[test]
fn test_detent_zone_containment() {
    let zone = DetentZone::new(0.0, 0.1, 0.05, DetentRole::Idle);
    
    // Test entry containment
    assert!(zone.contains_entry(0.0));   // Center
    assert!(zone.contains_entry(0.05));  // Within half-width
    assert!(zone.contains_entry(-0.1));  // Edge
    assert!(!zone.contains_entry(0.11)); // Outside
    
    // Test exit containment (with hysteresis)
    assert!(zone.contains_exit(0.0));    // Center
    assert!(zone.contains_exit(0.12));   // Within hysteresis
    assert!(zone.contains_exit(-0.15));  // Edge with hysteresis
    assert!(!zone.contains_exit(0.16));  // Outside hysteresis
}

#[test]
fn test_single_detent_entry_exit() {
    let zones = vec![
        DetentZone::new(0.0, 0.1, 0.05, DetentRole::Idle),
    ];
    let (node, receiver) = create_test_detent_node(zones);
    let mut state = DetentState {
        active_detent_idx: u32::MAX,
        last_position: 0.0,
        last_event_ns: 0,
    };

    // Start outside detent
    let mut frame = AxisFrame::new(-0.5, 1000);
    unsafe { process_frame_soa(&node, &mut frame, &mut state); }
    assert_eq!(state.active_detent_idx, u32::MAX);
    assert_eq!(frame.out, -0.5); // No snapping
    
    // Enter detent
    frame = AxisFrame::new(0.05, 2000);
    unsafe { process_frame_soa(&node, &mut frame, &mut state); }
    assert_eq!(state.active_detent_idx, 0);
    assert_eq!(frame.out, 0.0); // Snapped to center
    
    // Check event
    let event = receiver.try_recv().unwrap();
    assert_eq!(event.from_detent, None);
    assert_eq!(event.to_detent, Some(DetentRole::Idle));
    assert_eq!(event.position, 0.05);
    assert_eq!(event.timestamp_ns, 2000);
    
    // Stay in detent (within exit threshold)
    frame = AxisFrame::new(0.12, 3000);
    unsafe { process_frame_soa(&node, &mut frame, &mut state); }
    assert_eq!(state.active_detent_idx, 0);
    assert_eq!(frame.out, 0.0); // Still snapped
    assert!(receiver.try_recv().is_err()); // No new event
    
    // Exit detent
    frame = AxisFrame::new(0.2, 4000);
    unsafe { process_frame_soa(&node, &mut frame, &mut state); }
    assert_eq!(state.active_detent_idx, u32::MAX);
    assert_eq!(frame.out, 0.2); // No longer snapped
    
    // Check exit event
    let event = receiver.try_recv().unwrap();
    assert_eq!(event.from_detent, Some(DetentRole::Idle));
    assert_eq!(event.to_detent, None);
    assert_eq!(event.position, 0.2);
    assert_eq!(event.timestamp_ns, 4000);
}

#[test]
fn test_multiple_detents_no_overlap() {
    let zones = vec![
        DetentZone::new(-0.5, 0.1, 0.02, DetentRole::Reverse),
        DetentZone::new(0.0, 0.1, 0.02, DetentRole::Idle),
        DetentZone::new(0.5, 0.1, 0.02, DetentRole::Takeoff),
    ];
    let (node, receiver) = create_test_detent_node(zones);
    let mut state = DetentState {
        active_detent_idx: u32::MAX,
        last_position: 0.0,
        last_event_ns: 0,
    };

    // Enter reverse detent
    let mut frame = AxisFrame::new(-0.45, 1000);
    unsafe { process_frame_soa(&node, &mut frame, &mut state); }
    assert_eq!(state.active_detent_idx, 0);
    assert_eq!(frame.out, -0.5); // Snapped to reverse center
    
    let event = receiver.try_recv().unwrap();
    assert_eq!(event.to_detent, Some(DetentRole::Reverse));
    
    // Move to idle detent
    frame = AxisFrame::new(0.05, 2000);
    unsafe { process_frame_soa(&node, &mut frame, &mut state); }
    assert_eq!(state.active_detent_idx, 1);
    assert_eq!(frame.out, 0.0); // Snapped to idle center
    
    let event = receiver.try_recv().unwrap();
    assert_eq!(event.from_detent, Some(DetentRole::Reverse));
    assert_eq!(event.to_detent, Some(DetentRole::Idle));
    
    // Move to takeoff detent
    frame = AxisFrame::new(0.55, 3000);
    unsafe { process_frame_soa(&node, &mut frame, &mut state); }
    assert_eq!(state.active_detent_idx, 2);
    assert_eq!(frame.out, 0.5); // Snapped to takeoff center
    
    let event = receiver.try_recv().unwrap();
    assert_eq!(event.from_detent, Some(DetentRole::Idle));
    assert_eq!(event.to_detent, Some(DetentRole::Takeoff));
}

#[test]
fn test_hysteresis_prevents_flapping() {
    let zones = vec![
        DetentZone::new(0.0, 0.1, 0.05, DetentRole::Idle),
    ];
    let (node, receiver) = create_test_detent_node(zones);
    let mut state = DetentState {
        active_detent_idx: u32::MAX,
        last_position: 0.0,
        last_event_ns: 0,
    };

    // Enter detent
    let mut frame = AxisFrame::new(0.05, 1000);
    unsafe { process_frame_soa(&node, &mut frame, &mut state); }
    assert_eq!(state.active_detent_idx, 0);
    let _entry_event = receiver.try_recv().unwrap();
    
    // Move to edge of entry zone but within exit zone (hysteresis)
    frame = AxisFrame::new(0.12, 2000);
    unsafe { process_frame_soa(&node, &mut frame, &mut state); }
    assert_eq!(state.active_detent_idx, 0); // Still in detent due to hysteresis
    assert!(receiver.try_recv().is_err()); // No event generated
    
    // Move back toward center
    frame = AxisFrame::new(0.08, 3000);
    unsafe { process_frame_soa(&node, &mut frame, &mut state); }
    assert_eq!(state.active_detent_idx, 0); // Still in detent
    assert!(receiver.try_recv().is_err()); // No event generated
    
    // Only exit when beyond hysteresis threshold
    frame = AxisFrame::new(0.16, 4000);
    unsafe { process_frame_soa(&node, &mut frame, &mut state); }
    assert_eq!(state.active_detent_idx, u32::MAX); // Now exited
    let _exit_event = receiver.try_recv().unwrap();
}

#[test]
fn test_no_snap_detent() {
    let zones = vec![
        DetentZone::no_snap(0.0, 0.1, 0.02, DetentRole::Custom(1)),
    ];
    let (node, receiver) = create_test_detent_node(zones);
    let mut state = DetentState {
        active_detent_idx: u32::MAX,
        last_position: 0.0,
        last_event_ns: 0,
    };

    // Enter detent
    let mut frame = AxisFrame::new(0.05, 1000);
    unsafe { process_frame_soa(&node, &mut frame, &mut state); }
    assert_eq!(state.active_detent_idx, 0);
    assert_eq!(frame.out, 0.05); // Not snapped to center
    
    let event = receiver.try_recv().unwrap();
    assert_eq!(event.to_detent, Some(DetentRole::Custom(1)));
}

#[test]
fn test_detent_role_names() {
    assert_eq!(DetentRole::Idle.name(), "Idle");
    assert_eq!(DetentRole::Takeoff.name(), "Takeoff");
    assert_eq!(DetentRole::Custom(42).name(), "Custom");
}

/// Sweep test to verify single transition per boundary crossing
#[test]
fn test_sweep_single_transitions() {
    let zones = vec![
        DetentZone::new(-0.5, 0.1, 0.05, DetentRole::Reverse),
        DetentZone::new(0.0, 0.1, 0.05, DetentRole::Idle),
        DetentZone::new(0.5, 0.1, 0.05, DetentRole::Takeoff),
    ];
    let (node, receiver) = create_test_detent_node(zones);
    let mut state = DetentState {
        active_detent_idx: u32::MAX,
        last_position: 0.0,
        last_event_ns: 0,
    };

    // Sweep from -1.0 to 1.0 in small steps
    let mut events = Vec::new();
    let steps = 200;
    
    for i in 0..=steps {
        let position = -1.0 + (2.0 * i as f32 / steps as f32);
        let mut frame = AxisFrame::new(position, (i * 1000) as u64);
        unsafe { process_frame_soa(&node, &mut frame, &mut state); }
        
        // Collect any events
        while let Ok(event) = receiver.try_recv() {
            events.push(event);
        }
    }
    
    // Verify we got exactly the expected transitions
    assert_eq!(events.len(), 6); // Enter reverse, exit reverse, enter idle, exit idle, enter takeoff, exit takeoff
    
    // Verify transition sequence
    assert_eq!(events[0].to_detent, Some(DetentRole::Reverse));
    assert_eq!(events[1].from_detent, Some(DetentRole::Reverse));
    assert_eq!(events[2].to_detent, Some(DetentRole::Idle));
    assert_eq!(events[3].from_detent, Some(DetentRole::Idle));
    assert_eq!(events[4].to_detent, Some(DetentRole::Takeoff));
    assert_eq!(events[5].from_detent, Some(DetentRole::Takeoff));
    
    // Verify no duplicate transitions at same boundary
    let mut transition_positions = HashMap::new();
    for event in &events {
        let key = (event.from_detent, event.to_detent);
        let positions = transition_positions.entry(key).or_insert_with(Vec::new);
        positions.push(event.position);
    }
    
    // Each transition type should occur only once
    for (_, positions) in transition_positions {
        assert_eq!(positions.len(), 1, "Multiple transitions of same type detected");
    }
}

// Property-based test for deterministic behavior
proptest! {
    #[test]
    fn test_deterministic_detent_behavior(
        positions in prop::collection::vec(-1.0f32..1.0f32, 1..100),
        seed in 0u64..1000u64,
    ) {
        let zones = vec![
            DetentZone::new(-0.6, 0.1, 0.05, DetentRole::Reverse),
            DetentZone::new(0.0, 0.15, 0.03, DetentRole::Idle),
            DetentZone::new(0.7, 0.08, 0.02, DetentRole::Takeoff),
        ];
        
        // Run the same sequence twice
        let mut results1 = Vec::new();
        let mut results2 = Vec::new();
        
        for run in 0..2 {
            let (node, receiver) = create_test_detent_node(zones.clone());
            let mut state = DetentState {
                active_detent_idx: u32::MAX,
                last_position: 0.0,
                last_event_ns: 0,
            };
            
            let mut run_results = Vec::new();
            
            for (i, &position) in positions.iter().enumerate() {
                let mut frame = AxisFrame::new(position, (seed + i as u64) * 1000);
                unsafe { process_frame_soa(&node, &mut frame, &mut state); }
                
                run_results.push((frame.out, state.active_detent_idx));
                
                // Collect events
                while let Ok(event) = receiver.try_recv() {
                    run_results.push((event.position, u32::MAX)); // Mark events
                }
            }
            
            if run == 0 {
                results1 = run_results;
            } else {
                results2 = run_results;
            }
        }
        
        // Results should be identical
        prop_assert_eq!(results1, results2);
    }
    
    #[test]
    fn test_hysteresis_stability(
        center in -0.8f32..0.8f32,
        half_width in 0.05f32..0.2f32,
        hysteresis in 0.01f32..0.1f32,
        oscillation_amplitude in 0.01f32..0.05f32,
    ) {
        let zone = DetentZone::new(center, half_width, hysteresis, DetentRole::Custom(1));
        let zones = vec![zone];
        let (node, receiver) = create_test_detent_node(zones);
        let mut state = DetentState {
            active_detent_idx: u32::MAX,
            last_position: 0.0,
            last_event_ns: 0,
        };
        
        // Enter the detent
        let mut frame = AxisFrame::new(center, 1000);
        unsafe { process_frame_soa(&node, &mut frame, &mut state); }
        let _entry_event = receiver.try_recv();
        
        // Oscillate within hysteresis band
        let oscillation_center = center + half_width + (hysteresis * 0.5);
        let mut event_count = 0;
        
        for i in 0..50 {
            let position = oscillation_center + oscillation_amplitude * (i as f32 * 0.1).sin();
            let mut frame = AxisFrame::new(position, (2000 + i * 100) as u64);
            unsafe { process_frame_soa(&node, &mut frame, &mut state); }
            
            if receiver.try_recv().is_ok() {
                event_count += 1;
            }
        }
        
        // Should not generate excessive events due to oscillation
        prop_assert!(event_count <= 2, "Too many events from oscillation: {}", event_count);
    }
}

#[test]
fn test_node_trait_implementation() {
    let zones = vec![
        DetentZone::new(0.0, 0.1, 0.05, DetentRole::Idle),
    ];
    let node = DetentNode::new(zones);
    
    // Test trait methods
    assert_eq!(node.node_type(), "detent");
    assert_eq!(node.state_size(), std::mem::size_of::<DetentState>());
    
    // Test state initialization
    let mut state_buffer = vec![0u8; node.state_size()];
    unsafe {
        node.init_state(state_buffer.as_mut_ptr());
        let state = &*(state_buffer.as_ptr() as *const DetentState);
        assert_eq!(state.active_detent_idx, u32::MAX);
        assert_eq!(state.last_position, 0.0);
        assert_eq!(state.last_event_ns, 0);
    }
}

#[test]
fn test_zone_sorting() {
    let zones = vec![
        DetentZone::new(0.5, 0.1, 0.05, DetentRole::Takeoff),
        DetentZone::new(-0.5, 0.1, 0.05, DetentRole::Reverse),
        DetentZone::new(0.0, 0.1, 0.05, DetentRole::Idle),
    ];
    
    let node = DetentNode::new(zones);
    
    // Zones should be sorted by center position
    assert_eq!(node.zones[0].center, -0.5); // Reverse
    assert_eq!(node.zones[1].center, 0.0);  // Idle
    assert_eq!(node.zones[2].center, 0.5);  // Takeoff
}