//! Mixer Node Demonstration
//!
//! This example demonstrates the MixerNode functionality for helicopter
//! anti-torque and aileron-rudder coordination scenarios.

use flight_axis::{
    AxisFrame, AxisEngine, PipelineBuilder, MixerNode, MixerConfig,
    EngineConfig,
};
use std::time::{Duration, Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Flight Axis Mixer Node Demonstration");
    println!("====================================\n");

    // Demonstrate helicopter anti-torque mixing
    demonstrate_helicopter_anti_torque()?;
    
    // Demonstrate aileron-rudder coordination
    demonstrate_aileron_rudder_coordination()?;
    
    // Demonstrate custom mixer configuration
    demonstrate_custom_mixer()?;
    
    // Performance demonstration
    demonstrate_mixer_performance()?;

    Ok(())
}

fn demonstrate_helicopter_anti_torque() -> Result<(), Box<dyn std::error::Error>> {
    println!("1. Helicopter Anti-Torque Mixing");
    println!("---------------------------------");
    
    // Create helicopter anti-torque mixer
    let mixer = MixerNode::helicopter_anti_torque(-0.3)?;
    
    println!("Configuration:");
    println!("  - Collective scale: -0.3 (collective up = left pedal needed)");
    println!("  - Pedal scale: 1.0 (direct pedal input)");
    println!();
    
    // Test scenarios
    let scenarios = [
        ([0.0, 0.0], "Hover, no inputs"),
        ([0.8, 0.0], "Collective up, no pedal"),
        ([0.8, 0.2], "Collective up, right pedal"),
        ([-0.3, -0.1], "Collective down, left pedal"),
        ([0.5, -0.15], "Mid collective, left pedal"),
    ];
    
    println!("Test Scenarios:");
    for &(inputs, description) in &scenarios {
        let mut output = 0.0;
        mixer.process_inputs(&inputs, &mut output);
        
        let pedal_direction = if output > 0.05 {
            "RIGHT"
        } else if output < -0.05 {
            "LEFT"
        } else {
            "CENTER"
        };
        
        println!("  {} -> Collective: {:.2}, Pedals: {:.2} -> Output: {:.3} ({})",
                description, inputs[0], inputs[1], output, pedal_direction);
    }
    println!();
    
    Ok(())
}

fn demonstrate_aileron_rudder_coordination() -> Result<(), Box<dyn std::error::Error>> {
    println!("2. Aileron-Rudder Coordination");
    println!("------------------------------");
    
    // Create aileron-rudder coordination mixer
    let mixer = MixerNode::aileron_rudder_coordination(0.15)?;
    
    println!("Configuration:");
    println!("  - Aileron scale: 0.15 (right aileron = right rudder needed)");
    println!("  - Rudder scale: 1.0 (direct rudder input)");
    println!();
    
    // Test scenarios
    let scenarios = [
        ([0.0, 0.0], "Wings level, no rudder"),
        ([0.6, 0.0], "Right aileron, no rudder"),
        ([-0.4, 0.0], "Left aileron, no rudder"),
        ([0.6, -0.1], "Right aileron, left rudder"),
        ([-0.3, 0.2], "Left aileron, right rudder"),
    ];
    
    println!("Test Scenarios:");
    for &(inputs, description) in &scenarios {
        let mut output = 0.0;
        mixer.process_inputs(&inputs, &mut output);
        
        let rudder_direction = if output > 0.05 {
            "RIGHT"
        } else if output < -0.05 {
            "LEFT"
        } else {
            "CENTER"
        };
        
        println!("  {} -> Aileron: {:.2}, Rudder: {:.2} -> Output: {:.3} ({})",
                description, inputs[0], inputs[1], output, rudder_direction);
    }
    println!();
    
    Ok(())
}

fn demonstrate_custom_mixer() -> Result<(), Box<dyn std::error::Error>> {
    println!("3. Custom Multi-Input Mixer");
    println!("---------------------------");
    
    // Create a complex mixer for advanced helicopter control
    let config = MixerConfig::new("advanced_anti_torque")
        .add_input_with_gain("collective", -0.3, 1.0)
        .add_input_with_gain("pedals", 1.0, 1.0)
        .add_input_with_gain("airspeed", 0.1, 0.8)  // Airspeed affects torque
        .add_input_with_gain("engine_torque", -0.2, 1.2); // Engine torque compensation
    
    let mixer = MixerNode::new(config)?;
    
    println!("Configuration:");
    println!("  - Collective: -0.3 * 1.0");
    println!("  - Pedals: 1.0 * 1.0");
    println!("  - Airspeed: 0.1 * 0.8");
    println!("  - Engine Torque: -0.2 * 1.2");
    println!();
    
    // Test complex scenario
    let inputs = [0.7, 0.1, 0.6, 0.8]; // collective, pedals, airspeed, engine_torque
    let mut output = 0.0;
    mixer.process_inputs(&inputs, &mut output);
    
    println!("Complex Scenario:");
    println!("  Collective: {:.1}, Pedals: {:.1}, Airspeed: {:.1}, Engine Torque: {:.1}",
            inputs[0], inputs[1], inputs[2], inputs[3]);
    println!("  Final Anti-Torque Output: {:.3}", output);
    
    // Break down the calculation
    let collective_contrib = inputs[0] * -0.3 * 1.0;
    let pedals_contrib = inputs[1] * 1.0 * 1.0;
    let airspeed_contrib = inputs[2] * 0.1 * 0.8;
    let engine_contrib = inputs[3] * -0.2 * 1.2;
    
    println!("  Breakdown:");
    println!("    Collective contribution: {:.3}", collective_contrib);
    println!("    Pedals contribution: {:.3}", pedals_contrib);
    println!("    Airspeed contribution: {:.3}", airspeed_contrib);
    println!("    Engine torque contribution: {:.3}", engine_contrib);
    println!("    Total: {:.3}", collective_contrib + pedals_contrib + airspeed_contrib + engine_contrib);
    println!();
    
    Ok(())
}

fn demonstrate_mixer_performance() -> Result<(), Box<dyn std::error::Error>> {
    println!("4. Performance Demonstration");
    println!("----------------------------");
    
    // Create engine with mixer in pipeline
    let engine = AxisEngine::with_config("mixer_demo".to_string(), EngineConfig {
        enable_rt_checks: true,
        max_frame_time_us: 500,
        enable_counters: true,
        enable_conflict_detection: true,
        conflict_detector_config: Default::default(),
    });
    
    // Create pipeline with deadzone, curve, and helicopter mixer
    let pipeline = PipelineBuilder::new()
        .deadzone(0.03)
        .curve(0.2)?
        .helicopter_anti_torque(-0.25)?
        .compile()?;
    
    let _ = engine.update_pipeline(pipeline);
    
    println!("Pipeline: Deadzone -> Curve -> Helicopter Anti-Torque Mixer");
    
    // Performance test
    let start = Instant::now();
    let iterations = 25000; // Simulate 100 seconds at 250Hz
    
    for i in 0..iterations {
        let mut frame = AxisFrame::new(
            ((i as f32) / 1000.0).sin(), // Varying input
            i as u64 * 4000000 // 250Hz = 4ms intervals
        );
        let _ = engine.process(&mut frame);
    }
    
    let elapsed = start.elapsed();
    let avg_time_per_frame = elapsed / iterations;
    
    println!("Performance Results:");
    println!("  Processed {} frames in {:?}", iterations, elapsed);
    println!("  Average time per frame: {:?}", avg_time_per_frame);
    println!("  Frames per second: {:.0}", 1.0 / avg_time_per_frame.as_secs_f64());
    
    // Check counters
    let counters = engine.counters();
    println!("  Total frames processed: {}", counters.frames_processed());
    println!("  Pipeline swaps: {}", counters.pipeline_swaps());
    println!("  Deadline misses: {}", counters.deadline_misses());
    println!("  RT allocations: {}", counters.rt_allocations());
    println!("  RT lock acquisitions: {}", counters.rt_lock_acquisitions());
    
    if avg_time_per_frame < Duration::from_micros(100) {
        println!("  ✓ Performance target met (<100μs per frame)");
    } else {
        println!("  ⚠ Performance target missed (≥100μs per frame)");
    }
    
    if counters.rt_allocations() == 0 && counters.rt_lock_acquisitions() == 0 {
        println!("  ✓ Zero-allocation constraint maintained");
    } else {
        println!("  ⚠ Zero-allocation constraint violated");
    }
    
    println!();
    
    Ok(())
}