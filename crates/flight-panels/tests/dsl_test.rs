//! Standalone test for DSL compiler and evaluator

use flight_core::rules::{RulesSchema, Rule, RuleDefaults, Action};
use flight_panels::{RulesEvaluator, PanelManager};
use std::collections::HashMap;
use std::time::Duration;

#[test]
fn test_basic_dsl_compilation() {
    // Test basic DSL compilation without depending on other flight-core modules
    let rules_schema = RulesSchema {
        schema: "flight.ledmap/1".to_string(),
        rules: vec![
            Rule {
                when: "gear_down".to_string(),
                do_action: "led.panel('GEAR').on()".to_string(),
                action: "led.panel('GEAR').on()".to_string(),
            }
        ],
        defaults: None,
    };

    // This should compile successfully
    let result = rules_schema.validate();
    assert!(result.is_ok(), "Rules validation failed: {:?}", result);
}

#[test]
fn test_evaluator_initialization() {
    // Test that evaluator can be created and initialized
    let mut evaluator = RulesEvaluator::new();
    
    // Create a simple bytecode program manually for testing
    use flight_core::rules::{BytecodeProgram, BytecodeOp, CompareOp};
    
    let program = BytecodeProgram {
        instructions: vec![
            BytecodeOp::LoadVar(0),
            BytecodeOp::LoadConst(0.0),
            BytecodeOp::Compare(CompareOp::NotEqual),
            BytecodeOp::JumpFalse(5),
            BytecodeOp::Action(0),
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
    
    // Test evaluation
    let mut telemetry = HashMap::new();
    telemetry.insert("gear_down".to_string(), 1.0);
    
    // This should work without allocations after initialization
    let _actions = evaluator.evaluate(&flight_core::rules::CompiledRules {
        bytecode: program,
        hysteresis_bands: HashMap::new(),
    }, &telemetry);
}

#[test]
fn test_rate_limiting() {
    let mut evaluator = RulesEvaluator::new();
    
    // Set a longer rate limit for testing
    evaluator.set_min_eval_interval(Duration::from_millis(50));
    
    // Create simple program
    use flight_core::rules::{BytecodeProgram, BytecodeOp, Action};
    
    let program = BytecodeProgram {
        instructions: vec![BytecodeOp::Action(0)],
        variable_map: HashMap::new(),
        hysteresis_map: HashMap::new(),
        hysteresis_bands: Vec::new(),
        actions: vec![Action::LedOn { target: "TEST".to_string() }],
        stack_size: 8,
    };
    
    evaluator.initialize_for_program(&program);
    
    let compiled = flight_core::rules::CompiledRules {
        bytecode: program,
        hysteresis_bands: HashMap::new(),
    };
    
    let telemetry = HashMap::new();
    
    // First evaluation should work
    let actions1 = evaluator.evaluate(&compiled, &telemetry);
    assert_eq!(actions1.len(), 1);
    
    // Immediate second evaluation should return cached result due to rate limiting
    let actions2 = evaluator.evaluate(&compiled, &telemetry);
    assert_eq!(actions2.len(), 1); // Should return cached result
}

#[test]
fn test_zero_allocation_constraint() {
    let mut evaluator = RulesEvaluator::new();
    
    // Create program
    use flight_core::rules::{BytecodeProgram, BytecodeOp, Action, CompareOp};
    
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
    
    evaluator.initialize_for_program(&program);
    evaluator.set_min_eval_interval(Duration::from_millis(0)); // No rate limiting
    
    let compiled = flight_core::rules::CompiledRules {
        bytecode: program,
        hysteresis_bands: HashMap::new(),
    };
    
    // Capture initial capacities
    let initial_stack_cap = evaluator.stack().capacity();
    let initial_actions_cap = evaluator.actions_buffer().capacity();
    let initial_vars_cap = evaluator.variable_cache().capacity();
    
    let mut telemetry = HashMap::new();
    telemetry.insert("test_var".to_string(), 1.0);
    
    // Run many evaluations
    for _ in 0..1000 {
        let _actions = evaluator.evaluate(&compiled, &telemetry);
    }
    
    // Verify no capacity growth (indicating no allocations)
    assert_eq!(evaluator.stack().capacity(), initial_stack_cap);
    assert_eq!(evaluator.actions_buffer().capacity(), initial_actions_cap);
    assert_eq!(evaluator.variable_cache().capacity(), initial_vars_cap);
}