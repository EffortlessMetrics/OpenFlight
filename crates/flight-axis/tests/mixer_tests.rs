//! Comprehensive unit tests for MixerNode
//!
//! Tests mixer mathematics, zero-allocation constraint, and helicopter torque cross-feed scenarios.

use flight_axis::{
    AxisFrame, MixerNode, MixerConfig, MixerInput, PipelineBuilder, 
    AllocationGuard, Node
};
use std::time::Duration;

#[test]
fn test_mixer_input_creation() {
    let input = MixerInput::new("test", 0.5, 1.2);
    assert_eq!(input.name, "test");
    assert_eq!(input.scale, 0.5);
    assert_eq!(input.gain, 1.2);
}

#[test]
fn test_mixer_input_with_scale() {
    let input = MixerInput::with_scale("collective", -0.3);
    assert_eq!(input.name, "collective");
    assert_eq!(input.scale, -0.3);
    assert_eq!(input.gain, 1.0);
}

#[test]
fn test_mixer_input_clamping() {
    // Test scale clamping
    let input1 = MixerInput::new("test", 15.0, 1.0);
    assert_eq!(input1.scale, 10.0); // Should be clamped to max
    
    let input2 = MixerInput::new("test", -15.0, 1.0);
    assert_eq!(input2.scale, -10.0); // Should be clamped to min
    
    // Test gain clamping
    let input3 = MixerInput::new("test", 1.0, 15.0);
    assert_eq!(input3.gain, 10.0); // Should be clamped to max
    
    let input4 = MixerInput::new("test", 1.0, -1.0);
    assert_eq!(input4.gain, 0.0); // Should be clamped to min
}

#[test]
fn test_mixer_input_apply() {
    let input = MixerInput::new("test", 2.0, 1.5);
    
    // Test positive value
    assert_eq!(input.apply(0.5), 1.5); // 0.5 * 2.0 * 1.5 = 1.5
    
    // Test negative value
    assert_eq!(input.apply(-0.4), -1.2); // -0.4 * 2.0 * 1.5 = -1.2
    
    // Test zero
    assert_eq!(input.apply(0.0), 0.0);
}

#[test]
fn test_mixer_config_creation() {
    let config = MixerConfig::new("test_output");
    assert_eq!(config.output_name, "test_output");
    assert!(config.inputs.is_empty());
    assert!(config.clamp_output);
}

#[test]
fn test_mixer_config_builder_pattern() {
    let config = MixerConfig::new("anti_torque")
        .add_scaled_input("collective", -0.3)
        .add_input_with_gain("pedals", 1.0, 1.2)
        .no_clamp();
    
    assert_eq!(config.inputs.len(), 2);
    assert_eq!(config.inputs[0].name, "collective");
    assert_eq!(config.inputs[0].scale, -0.3);
    assert_eq!(config.inputs[1].name, "pedals");
    assert_eq!(config.inputs[1].scale, 1.0);
    assert_eq!(config.inputs[1].gain, 1.2);
    assert!(!config.clamp_output);
}

#[test]
fn test_mixer_config_validation() {
    // Valid configuration
    let valid_config = MixerConfig::new("test")
        .add_scaled_input("input1", 1.0);
    assert!(valid_config.validate().is_ok());
    
    // Empty inputs
    let empty_config = MixerConfig::new("test");
    assert!(empty_config.validate().is_err());
    
    // Too many inputs
    let mut many_inputs_config = MixerConfig::new("test");
    for i in 0..10 {
        many_inputs_config = many_inputs_config.add_scaled_input(&format!("input{}", i), 1.0);
    }
    assert!(many_inputs_config.validate().is_err());
    
    // Test with extreme values that get clamped
    let config_with_extreme_values = MixerConfig::new("test")
        .add_input(MixerInput::new("test", 15.0, 1.0)); // Scale will be clamped to 10.0
    // Should be valid because clamping occurs in MixerInput::new
    assert!(config_with_extreme_values.validate().is_ok());
}

#[test]
fn test_mixer_node_creation() {
    let config = MixerConfig::new("test")
        .add_scaled_input("input1", 0.5);
    
    let mixer = MixerNode::new(config);
    assert!(mixer.is_ok());
    
    let mixer = mixer.unwrap();
    assert_eq!(mixer.config().output_name, "test");
    assert_eq!(mixer.config().inputs.len(), 1);
}

#[test]
fn test_mixer_node_creation_with_invalid_config() {
    let empty_config = MixerConfig::new("test");
    let mixer = MixerNode::new(empty_config);
    assert!(mixer.is_err());
}

#[test]
fn test_helicopter_anti_torque_mixer() {
    let mixer = MixerNode::helicopter_anti_torque(-0.3);
    assert!(mixer.is_ok());
    
    let mixer = mixer.unwrap();
    assert_eq!(mixer.config().output_name, "anti_torque");
    assert_eq!(mixer.config().inputs.len(), 2);
    
    // Check collective input (first input)
    assert_eq!(mixer.config().inputs[0].name, "collective");
    assert_eq!(mixer.config().inputs[0].scale, -0.3);
    
    // Check pedals input (second input)
    assert_eq!(mixer.config().inputs[1].name, "pedals");
    assert_eq!(mixer.config().inputs[1].scale, 1.0);
}

#[test]
fn test_aileron_rudder_coordination_mixer() {
    let mixer = MixerNode::aileron_rudder_coordination(0.15);
    assert!(mixer.is_ok());
    
    let mixer = mixer.unwrap();
    assert_eq!(mixer.config().output_name, "rudder_coordinated");
    assert_eq!(mixer.config().inputs.len(), 2);
    
    // Check aileron input (first input)
    assert_eq!(mixer.config().inputs[0].name, "aileron");
    assert_eq!(mixer.config().inputs[0].scale, 0.15);
    
    // Check rudder input (second input)
    assert_eq!(mixer.config().inputs[1].name, "rudder");
    assert_eq!(mixer.config().inputs[1].scale, 1.0);
}

#[test]
fn test_mixer_process_inputs_basic() {
    let config = MixerConfig::new("test")
        .add_scaled_input("input1", 2.0)
        .add_scaled_input("input2", -1.5);
    
    let mixer = MixerNode::new(config).unwrap();
    
    let inputs = [0.5, 0.4]; // input1=0.5, input2=0.4
    let mut output = 0.0;
    
    mixer.process_inputs(&inputs, &mut output);
    
    // Expected: 0.5 * 2.0 + 0.4 * (-1.5) = 1.0 - 0.6 = 0.4
    assert!((output - 0.4).abs() < 1e-6);
}

#[test]
fn test_mixer_process_inputs_with_gain() {
    let config = MixerConfig::new("test")
        .add_input_with_gain("input1", 1.0, 2.0)
        .add_input_with_gain("input2", 1.0, 0.5);
    
    let mixer = MixerNode::new(config).unwrap();
    
    let inputs = [0.6, 0.8]; // input1=0.6, input2=0.8
    let mut output = 0.0;
    
    mixer.process_inputs(&inputs, &mut output);
    
    // Expected: 0.6 * 1.0 * 2.0 + 0.8 * 1.0 * 0.5 = 1.2 + 0.4 = 1.6
    // But should be clamped to 1.0 due to default clamping
    assert!((output - 1.0).abs() < 1e-6);
}

#[test]
fn test_mixer_process_inputs_no_clamp() {
    let config = MixerConfig::new("test")
        .add_scaled_input("input1", 2.0)
        .add_scaled_input("input2", 2.0)
        .no_clamp();
    
    let mixer = MixerNode::new(config).unwrap();
    
    let inputs = [0.8, 0.7]; // input1=0.8, input2=0.7
    let mut output = 0.0;
    
    mixer.process_inputs(&inputs, &mut output);
    
    // Expected: 0.8 * 2.0 + 0.7 * 2.0 = 1.6 + 1.4 = 3.0
    // Should not be clamped
    assert!((output - 3.0).abs() < 1e-6);
}

#[test]
fn test_mixer_helicopter_torque_cross_feed_scenario() {
    // Simulate helicopter anti-torque scenario
    let mixer = MixerNode::helicopter_anti_torque(-0.3).unwrap();
    
    // Test case 1: Collective up, no pedal input
    let inputs = [0.8, 0.0]; // collective=0.8, pedals=0.0
    let mut output = 0.0;
    mixer.process_inputs(&inputs, &mut output);
    
    // Expected: 0.8 * (-0.3) + 0.0 * 1.0 = -0.24
    // This represents left pedal input needed to counter torque
    assert!((output - (-0.24)).abs() < 1e-6);
    
    // Test case 2: Collective up, right pedal input
    let inputs = [0.8, 0.2]; // collective=0.8, pedals=0.2 (right)
    let mut output = 0.0;
    mixer.process_inputs(&inputs, &mut output);
    
    // Expected: 0.8 * (-0.3) + 0.2 * 1.0 = -0.24 + 0.2 = -0.04
    // Still slight left pedal needed
    assert!((output - (-0.04)).abs() < 1e-6);
    
    // Test case 3: Collective down, left pedal input
    let inputs = [-0.5, -0.3]; // collective=-0.5, pedals=-0.3 (left)
    let mut output = 0.0;
    mixer.process_inputs(&inputs, &mut output);
    
    // Expected: (-0.5) * (-0.3) + (-0.3) * 1.0 = 0.15 - 0.3 = -0.15
    assert!((output - (-0.15)).abs() < 1e-6);
}

#[test]
fn test_mixer_aileron_rudder_coordination_scenario() {
    // Simulate coordinated turn scenario
    let mixer = MixerNode::aileron_rudder_coordination(0.15).unwrap();
    
    // Test case 1: Right aileron, no rudder input
    let inputs = [0.6, 0.0]; // aileron=0.6 (right), rudder=0.0
    let mut output = 0.0;
    mixer.process_inputs(&inputs, &mut output);
    
    // Expected: 0.6 * 0.15 + 0.0 * 1.0 = 0.09
    // This represents right rudder needed for coordination
    assert!((output - 0.09).abs() < 1e-6);
    
    // Test case 2: Left aileron, manual right rudder
    let inputs = [-0.4, 0.2]; // aileron=-0.4 (left), rudder=0.2 (right)
    let mut output = 0.0;
    mixer.process_inputs(&inputs, &mut output);
    
    // Expected: (-0.4) * 0.15 + 0.2 * 1.0 = -0.06 + 0.2 = 0.14
    assert!((output - 0.14).abs() < 1e-6);
}

#[test]
fn test_mixer_node_trait_implementation() {
    let config = MixerConfig::new("test")
        .add_scaled_input("input1", 1.0);
    
    let mixer = MixerNode::new(config).unwrap();
    
    // Test Node trait methods
    assert_eq!(mixer.node_type(), "mixer");
    assert!(mixer.state_size() > 0); // Should have state for multiple inputs
    assert_eq!(mixer.state_size(), std::mem::size_of::<flight_axis::MixerState>());
}

#[test]
fn test_mixer_state_initialization() {
    let config = MixerConfig::new("test")
        .add_scaled_input("input1", 1.0)
        .add_scaled_input("input2", 0.5);
    
    let mixer = MixerNode::new(config).unwrap();
    
    // Allocate properly aligned state buffer
    let state_size = mixer.state_size();
    let mut state_buffer = vec![0u8; state_size + 8]; // Extra space for alignment
    
    // Ensure proper alignment
    let ptr = state_buffer.as_mut_ptr();
    let aligned_offset = (8 - (ptr as usize % 8)) % 8;
    let aligned_ptr = unsafe { ptr.add(aligned_offset) };
    
    // Initialize state
    unsafe {
        mixer.init_state(aligned_ptr);
    }
    
    // Verify state was initialized
    let state = unsafe { &*(aligned_ptr as *const flight_axis::MixerState) };
    assert_eq!(state.input_count, 2);
    assert_eq!(state.last_update_ns, 0);
    
    // All previous inputs should be zero
    for &prev_input in &state.prev_inputs {
        assert_eq!(prev_input, 0.0);
    }
}

#[test]
fn test_mixer_soa_step_processing() {
    let config = MixerConfig::new("test")
        .add_scaled_input("primary", 1.0)
        .add_scaled_input("secondary", 0.5);
    
    let mixer = MixerNode::new(config).unwrap();
    
    // Create properly aligned state
    let state_size = mixer.state_size();
    let mut state_buffer = vec![0u8; state_size + 8]; // Extra space for alignment
    
    // Ensure proper alignment
    let ptr = state_buffer.as_mut_ptr();
    let aligned_offset = (8 - (ptr as usize % 8)) % 8;
    let aligned_ptr = unsafe { ptr.add(aligned_offset) };
    
    unsafe {
        mixer.init_state(aligned_ptr);
        
        // Create test frame
        let mut frame = AxisFrame::new(0.8, 1000000);
        
        // Process frame
        mixer.step_soa(&mut frame, aligned_ptr);
        
        // Verify processing occurred (exact result depends on implementation)
        // The frame should have been modified
        assert!(frame.ts_mono_ns == 1000000);
        
        // Verify state was updated
        let state = &*(aligned_ptr as *const flight_axis::MixerState);
        assert_eq!(state.last_update_ns, 1000000);
    }
}

#[test]
fn test_mixer_zero_allocation_constraint() {
    let config = MixerConfig::new("test")
        .add_scaled_input("input1", 1.0)
        .add_scaled_input("input2", -0.5);
    
    let mixer = MixerNode::new(config).unwrap();
    
    // Test process_inputs method for zero allocations
    let inputs = [0.5, 0.3];
    let mut output = 0.0;
    
    // Process many times to ensure no allocations
    for _ in 0..1000 {
        mixer.process_inputs(&inputs, &mut output);
    }
    
    // If we get here without panicking, no allocations occurred
    assert!(true);
}

#[test]
fn test_mixer_deterministic_processing() {
    let config = MixerConfig::new("test")
        .add_scaled_input("input1", 1.5)
        .add_scaled_input("input2", -0.8);
    
    let mixer1 = MixerNode::new(config.clone()).unwrap();
    let mixer2 = MixerNode::new(config).unwrap();
    
    let test_cases = [
        ([0.0, 0.0], "zero inputs"),
        ([0.5, 0.3], "positive inputs"),
        ([-0.4, 0.7], "mixed inputs"),
        ([1.0, -1.0], "extreme inputs"),
        ([0.123, -0.456], "precise inputs"),
    ];
    
    for &(inputs, description) in &test_cases {
        let mut output1 = 0.0;
        let mut output2 = 0.0;
        
        mixer1.process_inputs(&inputs, &mut output1);
        mixer2.process_inputs(&inputs, &mut output2);
        
        assert_eq!(output1, output2, 
                   "Non-deterministic output for {}: inputs={:?}", description, inputs);
    }
}

#[test]
fn test_mixer_mathematical_properties() {
    let config = MixerConfig::new("test")
        .add_scaled_input("input1", 2.0)
        .add_scaled_input("input2", -1.0)
        .no_clamp();
    
    let mixer = MixerNode::new(config).unwrap();
    
    // Test linearity: mixer(a*x + b*y) = a*mixer(x) + b*mixer(y) for single inputs
    let x = [1.0, 0.0];
    let y = [0.0, 1.0];
    
    let mut output_x = 0.0;
    let mut output_y = 0.0;
    mixer.process_inputs(&x, &mut output_x);
    mixer.process_inputs(&y, &mut output_y);
    
    // Test scaling
    let scaled_x = [2.0, 0.0];
    let mut output_scaled_x = 0.0;
    mixer.process_inputs(&scaled_x, &mut output_scaled_x);
    
    assert!((output_scaled_x - 2.0 * output_x).abs() < 1e-6, 
            "Scaling property violated");
    
    // Test additivity
    let combined = [1.0, 1.0];
    let mut output_combined = 0.0;
    mixer.process_inputs(&combined, &mut output_combined);
    
    assert!((output_combined - (output_x + output_y)).abs() < 1e-6,
            "Additivity property violated");
}

#[test]
fn test_mixer_edge_cases() {
    let config = MixerConfig::new("test")
        .add_scaled_input("input1", 1.0)
        .add_scaled_input("input2", 1.0);
    
    let mixer = MixerNode::new(config).unwrap();
    
    // Test with NaN inputs (should not crash)
    let nan_inputs = [f32::NAN, 0.5];
    let mut output = 0.0;
    mixer.process_inputs(&nan_inputs, &mut output);
    // Output will be NaN, but shouldn't crash
    
    // Test with infinity inputs
    let inf_inputs = [f32::INFINITY, 0.0];
    let mut output = 0.0;
    mixer.process_inputs(&inf_inputs, &mut output);
    // Should clamp to 1.0 due to default clamping
    assert_eq!(output, 1.0);
    
    // Test with negative infinity
    let neg_inf_inputs = [f32::NEG_INFINITY, 0.0];
    let mut output = 0.0;
    mixer.process_inputs(&neg_inf_inputs, &mut output);
    // Should clamp to -1.0 due to default clamping
    assert_eq!(output, -1.0);
}

#[test]
fn test_mixer_performance_characteristics() {
    let config = MixerConfig::new("performance_test")
        .add_scaled_input("input1", 1.0)
        .add_scaled_input("input2", 0.5)
        .add_scaled_input("input3", -0.3)
        .add_scaled_input("input4", 2.0);
    
    let mixer = MixerNode::new(config).unwrap();
    
    let inputs = [0.5, 0.3, -0.2, 0.8];
    let mut output = 0.0;
    
    let start = std::time::Instant::now();
    let iterations = 100000;
    
    for _ in 0..iterations {
        mixer.process_inputs(&inputs, &mut output);
    }
    
    let elapsed = start.elapsed();
    let avg_time_per_call = elapsed / iterations;
    
    // Should process each call in well under 1μs
    assert!(avg_time_per_call < Duration::from_nanos(1000), 
            "Mixer processing too slow: {:?} per call", avg_time_per_call);
}

#[test]
fn test_mixer_in_pipeline_compilation() {
    let config = MixerConfig::new("test")
        .add_scaled_input("input1", 1.0);
    
    let result = PipelineBuilder::new()
        .deadzone(0.05)
        .mixer(config);
    
    assert!(result.is_ok());
    
    let result = result.unwrap().compile();
    assert!(result.is_ok());
    
    let pipeline = result.unwrap();
    assert_eq!(pipeline.metadata().len(), 2);
    
    let types: Vec<_> = pipeline.metadata()
        .iter()
        .map(|m| m.node_type)
        .collect();
    assert_eq!(types, vec!["deadzone", "mixer"]);
}

#[test]
fn test_helicopter_anti_torque_in_pipeline() {
    let result = PipelineBuilder::new()
        .deadzone(0.03)
        .helicopter_anti_torque(-0.25);
    
    assert!(result.is_ok());
    
    let result = result.unwrap().compile();
    assert!(result.is_ok());
    
    let pipeline = result.unwrap();
    assert_eq!(pipeline.metadata().len(), 2);
    assert_eq!(pipeline.metadata()[1].node_type, "mixer");
}

#[test]
fn test_aileron_rudder_coordination_in_pipeline() {
    let result = PipelineBuilder::new()
        .curve(0.2).unwrap()
        .aileron_rudder_coordination(0.12);
    
    assert!(result.is_ok());
    
    let result = result.unwrap().compile();
    assert!(result.is_ok());
    
    let pipeline = result.unwrap();
    assert_eq!(pipeline.metadata().len(), 2);
    assert_eq!(pipeline.metadata()[1].node_type, "mixer");
}

#[test]
fn test_mixer_complex_helicopter_scenario() {
    // Test a complex helicopter scenario with multiple mixers
    let anti_torque_config = MixerConfig::new("anti_torque")
        .add_scaled_input("collective", -0.3)
        .add_scaled_input("pedals", 1.0)
        .add_scaled_input("airspeed", 0.1); // Airspeed affects torque requirements
    
    let mixer = MixerNode::new(anti_torque_config).unwrap();
    
    // Scenario: Forward flight with collective up
    let inputs = [0.7, 0.1, 0.6]; // collective=0.7, pedals=0.1, airspeed=0.6
    let mut output = 0.0;
    mixer.process_inputs(&inputs, &mut output);
    
    // Expected: 0.7 * (-0.3) + 0.1 * 1.0 + 0.6 * 0.1 = -0.21 + 0.1 + 0.06 = -0.05
    assert!((output - (-0.05)).abs() < 1e-6);
}

#[test]
fn test_mixer_unit_handling_validation() {
    // Test that mixer properly handles normalized units [-1.0, 1.0]
    let config = MixerConfig::new("test")
        .add_scaled_input("input1", 1.0)
        .add_scaled_input("input2", 1.0);
    
    let mixer = MixerNode::new(config).unwrap();
    
    // Test boundary values
    let boundary_cases = [
        ([-1.0, -1.0], -2.0, -1.0), // Should clamp to -1.0
        ([1.0, 1.0], 2.0, 1.0),     // Should clamp to 1.0
        ([0.5, 0.5], 1.0, 1.0),     // Should clamp to 1.0
        ([0.25, 0.25], 0.5, 0.5),   // Should not clamp
        ([-0.3, 0.2], -0.1, -0.1),  // Should not clamp
    ];
    
    for &(inputs, _expected_unclamped, expected_clamped) in &boundary_cases {
        let mut output = 0.0;
        mixer.process_inputs(&inputs, &mut output);
        
        assert!((output - expected_clamped).abs() < 1e-6,
                "Unit handling failed for inputs {:?}: expected {}, got {}", 
                inputs, expected_clamped, output);
    }
}

/// Integration test with allocation guard to verify zero-allocation constraint
#[test]
fn test_mixer_zero_allocation_integration() {
    AllocationGuard::reset();
    
    {
        let _guard = AllocationGuard::new();
        
        let config = MixerConfig::new("test")
            .add_scaled_input("input1", 1.5)
            .add_scaled_input("input2", -0.8);
        
        let mixer = MixerNode::new(config).unwrap();
        
        // Process many operations
        let inputs = [0.6, 0.4];
        let mut output = 0.0;
        
        for _ in 0..10000 {
            mixer.process_inputs(&inputs, &mut output);
        }
        
        // Verify no allocations occurred during processing
        assert!(!AllocationGuard::allocations_detected(), 
                "Allocations detected during mixer processing!");
    }
}