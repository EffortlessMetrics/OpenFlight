//! Minimal DSL test that doesn't depend on broken modules

use std::collections::HashMap;

// Test the core DSL structures directly
#[test]
fn test_dsl_structures() {
    use flight_core::rules::{BytecodeOp, BytecodeProgram, Action, CompareOp};
    
    // Test that we can create bytecode structures
    let program = BytecodeProgram {
        instructions: vec![
            BytecodeOp::LoadVar(0),
            BytecodeOp::LoadConst(1.0),
            BytecodeOp::Compare(CompareOp::Equal),
            BytecodeOp::JumpFalse(5),
            BytecodeOp::Action(0),
        ],
        variable_map: {
            let mut map = HashMap::new();
            map.insert("test_var".to_string(), 0);
            map
        },
        hysteresis_map: HashMap::new(),
        hysteresis_bands: Vec::new(),
        actions: vec![Action::LedOn { target: "TEST".to_string() }],
        stack_size: 8,
    };
    
    // Verify structure
    assert_eq!(program.instructions.len(), 5);
    assert_eq!(program.variable_map.len(), 1);
    assert_eq!(program.actions.len(), 1);
    assert_eq!(program.stack_size, 8);
}

#[test]
fn test_evaluator_basic() {
    use flight_panels::RulesEvaluator;
    use flight_core::rules::{BytecodeProgram, BytecodeOp, Action, CompiledRules};
    
    let mut evaluator = RulesEvaluator::new();
    
    // Create a simple program that always executes an action
    let program = BytecodeProgram {
        instructions: vec![BytecodeOp::Action(0)],
        variable_map: HashMap::new(),
        hysteresis_map: HashMap::new(),
        hysteresis_bands: Vec::new(),
        actions: vec![Action::LedOn { target: "TEST".to_string() }],
        stack_size: 8,
    };
    
    evaluator.initialize_for_program(&program);
    
    let compiled = CompiledRules {
        bytecode: program,
        hysteresis_bands: HashMap::new(),
    };
    
    let telemetry = HashMap::new();
    let actions = evaluator.evaluate(&compiled, &telemetry);
    
    assert_eq!(actions.len(), 1);
    if let Action::LedOn { target } = &actions[0] {
        assert_eq!(target, "TEST");
    } else {
        panic!("Expected LedOn action");
    }
}

#[test]
fn test_led_controller_basic() {
    use flight_panels::LedController;
    use flight_core::rules::Action;
    use std::time::Duration;
    
    let mut controller = LedController::new();
    controller.set_min_interval(Duration::from_millis(0)); // No rate limiting for test
    
    let actions = vec![
        Action::LedOn { target: "GEAR".to_string() },
        Action::LedBlink { target: "WARNING".to_string(), rate_hz: 4.0 },
    ];
    
    // Should execute without errors
    let result = controller.execute_actions(&actions);
    assert!(result.is_ok());
    
    // Should have latency statistics
    let stats = controller.get_latency_stats();
    assert!(stats.is_some());
    
    let stats = stats.unwrap();
    assert_eq!(stats.sample_count, 2);
    assert!(stats.mean_ns > 0);
}

#[test]
fn test_bytecode_vm_execution() {
    use flight_panels::RulesEvaluator;
    use flight_core::rules::{BytecodeProgram, BytecodeOp, Action, CompareOp, CompiledRules};
    
    let mut evaluator = RulesEvaluator::new();
    
    // Create a program that tests a variable
    let program = BytecodeProgram {
        instructions: vec![
            BytecodeOp::LoadVar(0),      // Load gear_down
            BytecodeOp::LoadConst(0.0),  // Load 0.0
            BytecodeOp::Compare(CompareOp::NotEqual), // gear_down != 0.0
            BytecodeOp::JumpFalse(6),    // Jump to end if false
            BytecodeOp::Action(0),       // Execute action
            BytecodeOp::Nop,             // End
        ],
        variable_map: {
            let mut map = HashMap::new();
            map.insert("gear_down".to_string(), 0);
            map
        },
        hysteresis_map: HashMap::new(),
        hysteresis_bands: Vec::new(),
        actions: vec![Action::LedOn { target: "GEAR".to_string() }],
        stack_size: 8,
    };
    
    evaluator.initialize_for_program(&program);
    evaluator.set_min_eval_interval(std::time::Duration::from_millis(0));
    
    let compiled = CompiledRules {
        bytecode: program,
        hysteresis_bands: HashMap::new(),
    };
    
    // Test with gear down
    let mut telemetry = HashMap::new();
    telemetry.insert("gear_down".to_string(), 1.0);
    
    let actions = evaluator.evaluate(&compiled, &telemetry);
    assert_eq!(actions.len(), 1);
    
    // Test with gear up
    telemetry.insert("gear_down".to_string(), 0.0);
    let actions = evaluator.evaluate(&compiled, &telemetry);
    assert_eq!(actions.len(), 0);
}

#[test]
fn test_performance_requirements() {
    use flight_panels::RulesEvaluator;
    use flight_core::rules::{BytecodeProgram, BytecodeOp, Action, CompiledRules};
    use std::time::Instant;
    
    let mut evaluator = RulesEvaluator::new();
    
    // Create a moderately complex program
    let program = BytecodeProgram {
        instructions: vec![
            BytecodeOp::LoadVar(0),      // Load var1
            BytecodeOp::LoadConst(1.0),  
            BytecodeOp::Compare(flight_core::rules::CompareOp::Equal),
            BytecodeOp::LoadVar(1),      // Load var2
            BytecodeOp::LoadConst(100.0),
            BytecodeOp::Compare(flight_core::rules::CompareOp::Greater),
            BytecodeOp::And,             // var1 == 1.0 AND var2 > 100.0
            BytecodeOp::JumpFalse(10),
            BytecodeOp::Action(0),
            BytecodeOp::Nop,
        ],
        variable_map: {
            let mut map = HashMap::new();
            map.insert("var1".to_string(), 0);
            map.insert("var2".to_string(), 1);
            map
        },
        hysteresis_map: HashMap::new(),
        hysteresis_bands: Vec::new(),
        actions: vec![Action::LedOn { target: "TEST".to_string() }],
        stack_size: 8,
    };
    
    evaluator.initialize_for_program(&program);
    evaluator.set_min_eval_interval(std::time::Duration::from_millis(0));
    
    let compiled = CompiledRules {
        bytecode: program,
        hysteresis_bands: HashMap::new(),
    };
    
    let mut telemetry = HashMap::new();
    telemetry.insert("var1".to_string(), 1.0);
    telemetry.insert("var2".to_string(), 150.0);
    
    // Measure evaluation time over many iterations
    let iterations = 10000;
    let start = Instant::now();
    
    for _ in 0..iterations {
        let _actions = evaluator.evaluate(&compiled, &telemetry);
    }
    
    let elapsed = start.elapsed();
    let avg_time_ns = elapsed.as_nanos() / iterations;
    
    // Should be very fast - much less than 1ms per evaluation
    assert!(avg_time_ns < 100_000, "Average evaluation time too slow: {} ns", avg_time_ns);
    
    println!("Average evaluation time: {} ns ({:.2} μs)", avg_time_ns, avg_time_ns as f64 / 1000.0);
}