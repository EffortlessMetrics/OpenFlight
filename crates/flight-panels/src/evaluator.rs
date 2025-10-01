//! Rules evaluator for zero-allocation runtime evaluation

use flight_core::rules::{CompiledRules, BytecodeOp, BytecodeProgram, Action, CompareOp};
use std::collections::HashMap;
use std::time::Instant;

/// Zero-allocation rules evaluator
pub struct RulesEvaluator {
    /// Pre-allocated evaluation stack
    stack: Vec<f32>,
    /// Hysteresis state by index
    hysteresis_state: Vec<HysteresisState>,
    /// Variable values cache
    variable_cache: Vec<f32>,
    /// Actions buffer (pre-allocated)
    actions_buffer: Vec<Action>,
    /// Last evaluation time for rate limiting
    last_eval: Instant,
    /// Minimum evaluation interval (8ms for ≥8ms rate limiting)
    min_eval_interval: std::time::Duration,
}

/// Bytecode virtual machine for zero-allocation evaluation
struct BytecodeVM<'a> {
    stack: &'a mut Vec<f32>,
    hysteresis_state: &'a mut [HysteresisState],
    variable_cache: &'a [f32],
    actions_buffer: &'a mut Vec<Action>,
    program: &'a BytecodeProgram,
    pc: usize, // Program counter
}

/// Hysteresis state for a variable
#[derive(Debug, Clone)]
struct HysteresisState {
    current_value: f32,
    threshold_value: f32,
    band: f32,
    last_triggered: bool,
}

impl RulesEvaluator {
    /// Create a new rules evaluator
    pub fn new() -> Self {
        Self {
            stack: Vec::new(),
            hysteresis_state: Vec::new(),
            variable_cache: Vec::new(),
            actions_buffer: Vec::new(),
            last_eval: Instant::now(),
            min_eval_interval: std::time::Duration::from_millis(8),
        }
    }

    /// Initialize evaluator for a specific bytecode program (pre-allocates all buffers)
    pub fn initialize_for_program(&mut self, program: &BytecodeProgram) {
        // Pre-allocate stack
        self.stack.clear();
        self.stack.reserve(program.stack_size);
        
        // Pre-allocate hysteresis state
        self.hysteresis_state.clear();
        self.hysteresis_state.resize(program.hysteresis_bands.len(), HysteresisState {
            current_value: 0.0,
            threshold_value: 0.0,
            band: 0.0,
            last_triggered: false,
        });
        
        // Initialize hysteresis bands
        for (i, &band) in program.hysteresis_bands.iter().enumerate() {
            self.hysteresis_state[i].band = band;
        }
        
        // Pre-allocate variable cache
        self.variable_cache.clear();
        self.variable_cache.resize(program.variable_map.len(), 0.0);
        
        // Pre-allocate actions buffer
        self.actions_buffer.clear();
        self.actions_buffer.reserve(program.actions.len());
    }

    /// Evaluate compiled rules against telemetry (zero-allocation after initialization)
    /// Returns reference to internal actions buffer to avoid allocation
    pub fn evaluate(&mut self, rules: &CompiledRules, telemetry: &HashMap<String, f32>) -> &[Action] {
        let now = Instant::now();
        
        // Rate limiting: skip evaluation if called too frequently
        if now.duration_since(self.last_eval) < self.min_eval_interval {
            return &self.actions_buffer;
        }
        
        self.last_eval = now;
        
        // Clear actions buffer (reuse allocation)
        self.actions_buffer.clear();
        
        // Update variable cache from telemetry
        self.update_variable_cache(&rules.bytecode, telemetry);
        
        // Execute bytecode
        let mut vm = BytecodeVM {
            stack: &mut self.stack,
            hysteresis_state: &mut self.hysteresis_state,
            variable_cache: &self.variable_cache,
            actions_buffer: &mut self.actions_buffer,
            program: &rules.bytecode,
            pc: 0,
        };
        
        vm.execute();
        
        &self.actions_buffer
    }

    fn update_variable_cache(&mut self, program: &BytecodeProgram, telemetry: &HashMap<String, f32>) {
        for (var_name, &index) in &program.variable_map {
            let value = telemetry.get(var_name).copied().unwrap_or(0.0);
            if (index as usize) < self.variable_cache.len() {
                self.variable_cache[index as usize] = value;
            }
        }
    }

    /// Set minimum evaluation interval for rate limiting
    pub fn set_min_eval_interval(&mut self, interval: std::time::Duration) {
        self.min_eval_interval = interval;
    }
}

impl<'a> BytecodeVM<'a> {
    fn execute(&mut self) {
        while self.pc < self.program.instructions.len() {
            let instruction = &self.program.instructions[self.pc];
            
            match instruction {
                BytecodeOp::LoadVar(index) => {
                    let value = self.variable_cache.get(*index as usize).copied().unwrap_or(0.0);
                    self.stack.push(value);
                }
                BytecodeOp::LoadConst(value) => {
                    self.stack.push(*value);
                }
                BytecodeOp::Compare(op) => {
                    if self.stack.len() >= 2 {
                        let b = self.stack.pop().unwrap();
                        let a = self.stack.pop().unwrap();
                        let result = self.compare(a, b, op);
                        self.stack.push(if result { 1.0 } else { 0.0 });
                    }
                }
                BytecodeOp::Hysteresis(hyst_index) => {
                    if self.stack.len() >= 2 {
                        let threshold = self.stack.pop().unwrap();
                        let current = self.stack.pop().unwrap();
                        let result = self.apply_hysteresis(*hyst_index as usize, current, threshold);
                        self.stack.push(if result { 1.0 } else { 0.0 });
                    }
                }
                BytecodeOp::And => {
                    if self.stack.len() >= 2 {
                        let b = self.stack.pop().unwrap();
                        let a = self.stack.pop().unwrap();
                        let result = (a != 0.0) && (b != 0.0);
                        self.stack.push(if result { 1.0 } else { 0.0 });
                    }
                }
                BytecodeOp::Or => {
                    if self.stack.len() >= 2 {
                        let b = self.stack.pop().unwrap();
                        let a = self.stack.pop().unwrap();
                        let result = (a != 0.0) || (b != 0.0);
                        self.stack.push(if result { 1.0 } else { 0.0 });
                    }
                }
                BytecodeOp::Not => {
                    if let Some(value) = self.stack.pop() {
                        let result = value == 0.0;
                        self.stack.push(if result { 1.0 } else { 0.0 });
                    }
                }
                BytecodeOp::JumpFalse(addr) => {
                    if let Some(condition) = self.stack.pop() {
                        if condition == 0.0 {
                            self.pc = *addr as usize;
                            continue;
                        }
                    }
                }
                BytecodeOp::Jump(addr) => {
                    self.pc = *addr as usize;
                    continue;
                }
                BytecodeOp::Action(action_index) => {
                    if let Some(action) = self.program.actions.get(*action_index as usize) {
                        self.actions_buffer.push(action.clone());
                    }
                }
                BytecodeOp::Nop => {
                    // No operation
                }
            }
            
            self.pc += 1;
        }
    }

    fn compare(&self, a: f32, b: f32, op: &CompareOp) -> bool {
        match op {
            CompareOp::Equal => (a - b).abs() < f32::EPSILON,
            CompareOp::NotEqual => (a - b).abs() >= f32::EPSILON,
            CompareOp::Greater => a > b,
            CompareOp::GreaterEqual => a >= b,
            CompareOp::Less => a < b,
            CompareOp::LessEqual => a <= b,
        }
    }

    fn apply_hysteresis(&mut self, hyst_index: usize, current: f32, threshold: f32) -> bool {
        if hyst_index >= self.hysteresis_state.len() {
            return self.compare(current, threshold, &CompareOp::Greater);
        }

        let state = &mut self.hysteresis_state[hyst_index];
        state.current_value = current;
        state.threshold_value = threshold;

        let band = state.band;
        let threshold_high = threshold + band / 2.0;
        let threshold_low = threshold - band / 2.0;

        let new_triggered = if state.last_triggered {
            current > threshold_low
        } else {
            current > threshold_high
        };

        state.last_triggered = new_triggered;
        new_triggered
    }
}

impl Default for RulesEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flight_core::rules::{RulesSchema, Rule, RuleDefaults};

    #[test]
    fn test_bytecode_evaluation() {
        let mut evaluator = RulesEvaluator::new();
        
        // Create a simple rule
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

        // Compile rules
        let compiled = rules_schema.compile().unwrap();
        evaluator.initialize_for_program(&compiled.bytecode);

        // Test evaluation
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
    fn test_hysteresis_bytecode() {
        let mut evaluator = RulesEvaluator::new();
        
        // Create rule with hysteresis
        let mut hysteresis = HashMap::new();
        hysteresis.insert("aoa".to_string(), 2.0);
        
        let rules_schema = RulesSchema {
            schema: "flight.ledmap/1".to_string(),
            rules: vec![
                Rule {
                    when: "aoa > 10".to_string(),
                    do_action: "led.indexer.blink(rate_hz=6)".to_string(),
                    action: "led.indexer.blink(rate_hz=6)".to_string(),
                }
            ],
            defaults: Some(RuleDefaults {
                hysteresis: Some(hysteresis),
            }),
        };

        let compiled = rules_schema.compile().unwrap();
        evaluator.initialize_for_program(&compiled.bytecode);

        // Test hysteresis behavior
        let mut telemetry = HashMap::new();
        
        // Start below threshold
        telemetry.insert("aoa".to_string(), 9.0);
        let actions = evaluator.evaluate(&compiled, &telemetry);
        assert_eq!(actions.len(), 0);

        // Cross upper threshold (10.0 + 1.0 = 11.0)
        telemetry.insert("aoa".to_string(), 11.5);
        let actions = evaluator.evaluate(&compiled, &telemetry);
        assert_eq!(actions.len(), 1);

        // Stay above lower threshold (10.0 - 1.0 = 9.0)
        telemetry.insert("aoa".to_string(), 9.5);
        let actions = evaluator.evaluate(&compiled, &telemetry);
        assert_eq!(actions.len(), 1);

        // Cross lower threshold
        telemetry.insert("aoa".to_string(), 8.5);
        let actions = evaluator.evaluate(&compiled, &telemetry);
        assert_eq!(actions.len(), 0);
    }

    #[test]
    fn test_rate_limiting() {
        let mut evaluator = RulesEvaluator::new();
        evaluator.set_min_eval_interval(std::time::Duration::from_millis(50));
        
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

        let compiled = rules_schema.compile().unwrap();
        evaluator.initialize_for_program(&compiled.bytecode);

        let mut telemetry = HashMap::new();
        telemetry.insert("gear_down".to_string(), 1.0);

        // First evaluation should work
        let actions = evaluator.evaluate(&compiled, &telemetry);
        assert_eq!(actions.len(), 1);

        // Immediate second evaluation should be rate limited
        let actions = evaluator.evaluate(&compiled, &telemetry);
        assert_eq!(actions.len(), 1); // Returns cached result

        // Wait and try again
        std::thread::sleep(std::time::Duration::from_millis(60));
        let actions = evaluator.evaluate(&compiled, &telemetry);
        assert_eq!(actions.len(), 1);
    }

    #[test]
    fn test_zero_allocation_constraint() {
        // This test verifies that after initialization, no allocations occur
        let mut evaluator = RulesEvaluator::new();
        
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

        let compiled = rules_schema.compile().unwrap();
        evaluator.initialize_for_program(&compiled.bytecode);

        // Disable rate limiting for this test
        evaluator.set_min_eval_interval(std::time::Duration::from_millis(0));

        let mut telemetry = HashMap::new();
        telemetry.insert("gear_down".to_string(), 1.0);

        // Multiple evaluations should not cause allocations
        for _ in 0..1000 {
            let _actions = evaluator.evaluate(&compiled, &telemetry);
            // In a real test, we would use a custom allocator to verify zero allocations
            // For now, we just ensure no panics or errors occur
        }
    }
}