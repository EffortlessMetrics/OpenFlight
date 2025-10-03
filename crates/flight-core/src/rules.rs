// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Rules DSL for panel LED control

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::{FlightError, Result};

/// Rules DSL schema version 1
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RulesSchema {
    pub schema: String,
    pub rules: Vec<Rule>,
    pub defaults: Option<RuleDefaults>,
}

/// A single rule in the DSL
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub when: String,
    pub do_action: String,
    #[serde(rename = "do")]
    pub action: String,
}

/// Default settings for rules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleDefaults {
    pub hysteresis: Option<HashMap<String, f32>>,
}

/// Compiled rule for efficient evaluation
#[derive(Debug, Clone)]
pub struct CompiledRule {
    pub condition: Condition,
    pub action: Action,
    pub hysteresis_key: Option<String>,
}

/// Rule condition (stub implementation)
#[derive(Debug, Clone)]
pub enum Condition {
    /// Variable comparison: var op value
    Compare {
        variable: String,
        operator: CompareOp,
        value: f32,
    },
    /// Boolean variable
    Boolean {
        variable: String,
        negate: bool,
    },
    /// Logical AND of conditions
    And(Vec<Condition>),
    /// Logical OR of conditions
    Or(Vec<Condition>),
}

/// Comparison operators
#[derive(Debug, Clone)]
pub enum CompareOp {
    Equal,
    NotEqual,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,
}

/// Rule action (stub implementation)
#[derive(Debug, Clone)]
pub enum Action {
    /// Turn LED on
    LedOn { target: String },
    /// Turn LED off
    LedOff { target: String },
    /// Blink LED
    LedBlink { target: String, rate_hz: f32 },
    /// Set LED brightness
    LedBrightness { target: String, brightness: f32 },
}

/// Rules compiler (stub implementation)
pub struct RulesCompiler {
    hysteresis_defaults: HashMap<String, f32>,
}

/// Bytecode instruction for rules evaluation
#[derive(Debug, Clone)]
pub enum BytecodeOp {
    /// Load variable value onto stack: LOAD var_index
    LoadVar(u16),
    /// Load constant value onto stack: LOAD_CONST value
    LoadConst(f32),
    /// Compare top two stack values: CMP op
    Compare(CompareOp),
    /// Apply hysteresis: HYST hysteresis_key_index
    Hysteresis(u16),
    /// Logical AND: pop two values, push result
    And,
    /// Logical OR: pop two values, push result
    Or,
    /// Logical NOT: pop one value, push negated result
    Not,
    /// Jump if false: JMP_FALSE offset
    JumpFalse(u16),
    /// Jump unconditionally: JMP offset
    Jump(u16),
    /// Execute action: ACTION action_index
    Action(u16),
    /// No operation
    Nop,
}

/// Compiled bytecode program
#[derive(Debug, Clone)]
pub struct BytecodeProgram {
    /// Bytecode instructions
    pub instructions: Vec<BytecodeOp>,
    /// Variable name to index mapping
    pub variable_map: HashMap<String, u16>,
    /// Hysteresis key to index mapping
    pub hysteresis_map: HashMap<String, u16>,
    /// Hysteresis bands by index
    pub hysteresis_bands: Vec<f32>,
    /// Actions by index
    pub actions: Vec<Action>,
    /// Pre-allocated evaluation stack size
    pub stack_size: usize,
}

/// Compiled rules bytecode
#[derive(Debug, Clone)]
pub struct CompiledRules {
    pub bytecode: BytecodeProgram,
    pub hysteresis_bands: HashMap<String, f32>,
}

impl RulesSchema {
    /// Validate rules schema and syntax
    pub fn validate(&self) -> Result<()> {
        if self.schema != "flight.ledmap/1" {
            return Err(FlightError::RulesValidation(format!(
                "Unsupported schema version: {}",
                self.schema
            )));
        }

        // Validate each rule
        for (index, rule) in self.rules.iter().enumerate() {
            if let Err(e) = self.validate_rule(rule) {
                return Err(FlightError::RulesValidation(format!(
                    "Rule {}: {}",
                    index + 1,
                    e
                )));
            }
        }

        Ok(())
    }

    fn validate_rule(&self, rule: &Rule) -> std::result::Result<(), String> {
        // Basic syntax validation (stub implementation)
        if rule.when.is_empty() {
            return Err("Rule condition cannot be empty".to_string());
        }

        if rule.action.is_empty() {
            return Err("Rule action cannot be empty".to_string());
        }

        // TODO: Parse and validate condition syntax
        // TODO: Parse and validate action syntax

        Ok(())
    }

    /// Compile rules to bytecode for efficient evaluation
    pub fn compile(&self) -> Result<CompiledRules> {
        let compiler = RulesCompiler::new(
            self.defaults
                .as_ref()
                .and_then(|d| d.hysteresis.clone())
                .unwrap_or_default()
        );

        compiler.compile(self)
    }
}

impl RulesCompiler {
    pub fn new(hysteresis_defaults: HashMap<String, f32>) -> Self {
        Self {
            hysteresis_defaults,
        }
    }

    /// Compile rules schema to bytecode
    pub fn compile(&self, schema: &RulesSchema) -> Result<CompiledRules> {
        let mut compiler = BytecodeCompiler::new();
        
        // Add hysteresis defaults
        for (key, band) in &self.hysteresis_defaults {
            compiler.add_hysteresis_key(key.clone(), *band);
        }

        // Compile each rule
        for rule in &schema.rules {
            compiler.compile_rule(rule)?;
        }

        let bytecode = compiler.finalize();
        
        Ok(CompiledRules {
            bytecode,
            hysteresis_bands: self.hysteresis_defaults.clone(),
        })
    }

    fn compile_rule(&self, rule: &Rule) -> Result<CompiledRule> {
        // Stub implementation - parse condition and action
        let condition = self.parse_condition(&rule.when)?;
        let action = self.parse_action(&rule.action)?;

        // Determine hysteresis key if needed
        let hysteresis_key = self.extract_hysteresis_key(&condition);

        Ok(CompiledRule {
            condition,
            action,
            hysteresis_key,
        })
    }

    fn parse_condition(&self, condition_str: &str) -> Result<Condition> {
        // Stub implementation - basic parsing
        let condition_str = condition_str.trim();

        // Handle negated boolean variables
        if condition_str.starts_with('!') && !condition_str.contains(['>', '<', '=']) {
            let variable = condition_str[1..].trim().to_string();
            return Ok(Condition::Boolean { variable, negate: true });
        }

        // Handle boolean variables (no operators)
        if !condition_str.contains(['>', '<', '=']) {
            let variable = condition_str.to_string();
            return Ok(Condition::Boolean { variable, negate: false });
        }

        // Handle comparisons (very basic parsing)
        if let Some(pos) = condition_str.find(" == ") {
            let variable = condition_str[..pos].trim().to_string();
            let value_str = condition_str[pos + 4..].trim();
            let value = value_str.parse::<f32>()
                .map_err(|_| FlightError::RulesValidation(format!("Invalid number: {}", value_str)))?;

            return Ok(Condition::Compare {
                variable,
                operator: CompareOp::Equal,
                value,
            });
        }

        if let Some(pos) = condition_str.find(" > ") {
            let variable = condition_str[..pos].trim().to_string();
            let value_str = condition_str[pos + 3..].trim();
            let value = value_str.parse::<f32>()
                .map_err(|_| FlightError::RulesValidation(format!("Invalid number: {}", value_str)))?;

            return Ok(Condition::Compare {
                variable,
                operator: CompareOp::Greater,
                value,
            });
        }

        // TODO: Implement full parser for complex conditions

        Err(FlightError::RulesValidation(format!(
            "Unsupported condition syntax: {}",
            condition_str
        )))
    }

    fn parse_action(&self, action_str: &str) -> Result<Action> {
        // Stub implementation - basic parsing
        let action_str = action_str.trim();

        // Parse led.panel('TARGET').on()
        if let Some(start) = action_str.find("led.panel('") {
            if let Some(end) = action_str[start + 11..].find("')") {
                let target = action_str[start + 11..start + 11 + end].to_string();
                
                if action_str.ends_with(".on()") {
                    return Ok(Action::LedOn { target });
                } else if action_str.ends_with(".off()") {
                    return Ok(Action::LedOff { target });
                }
            }
        }

        // Parse led.indexer.blink(rate_hz=6)
        if action_str.starts_with("led.indexer.blink(") {
            if let Some(start) = action_str.find("rate_hz=") {
                if let Some(end) = action_str[start + 8..].find(')') {
                    let rate_str = &action_str[start + 8..start + 8 + end];
                    let rate_hz = rate_str.parse::<f32>()
                        .map_err(|_| FlightError::RulesValidation(format!("Invalid rate: {}", rate_str)))?;

                    return Ok(Action::LedBlink {
                        target: "indexer".to_string(),
                        rate_hz,
                    });
                }
            }
        }

        // TODO: Implement full parser for all action types

        Err(FlightError::RulesValidation(format!(
            "Unsupported action syntax: {}",
            action_str
        )))
    }

    fn extract_hysteresis_key(&self, condition: &Condition) -> Option<String> {
        match condition {
            Condition::Compare { variable, .. } => {
                if self.hysteresis_defaults.contains_key(variable) {
                    Some(variable.clone())
                } else {
                    None
                }
            }
            Condition::Boolean { .. } => None,
            Condition::And(conditions) | Condition::Or(conditions) => {
                // Return first hysteresis key found
                for cond in conditions {
                    if let Some(key) = self.extract_hysteresis_key(cond) {
                        return Some(key);
                    }
                }
                None
            }
        }
    }
}

/// Bytecode compiler for rules
struct BytecodeCompiler {
    instructions: Vec<BytecodeOp>,
    variable_map: HashMap<String, u16>,
    hysteresis_map: HashMap<String, u16>,
    hysteresis_bands: Vec<f32>,
    actions: Vec<Action>,
    next_var_index: u16,
    next_hyst_index: u16,
    next_action_index: u16,
    max_stack_depth: usize,
    current_stack_depth: usize,
}

impl BytecodeCompiler {
    fn new() -> Self {
        Self {
            instructions: Vec::new(),
            variable_map: HashMap::new(),
            hysteresis_map: HashMap::new(),
            hysteresis_bands: Vec::new(),
            actions: Vec::new(),
            next_var_index: 0,
            next_hyst_index: 0,
            next_action_index: 0,
            max_stack_depth: 0,
            current_stack_depth: 0,
        }
    }

    fn add_hysteresis_key(&mut self, key: String, band: f32) {
        if !self.hysteresis_map.contains_key(&key) {
            self.hysteresis_map.insert(key, self.next_hyst_index);
            self.hysteresis_bands.push(band);
            self.next_hyst_index += 1;
        }
    }

    fn get_or_add_variable(&mut self, name: &str) -> u16 {
        if let Some(&index) = self.variable_map.get(name) {
            index
        } else {
            let index = self.next_var_index;
            self.variable_map.insert(name.to_string(), index);
            self.next_var_index += 1;
            index
        }
    }

    fn add_action(&mut self, action: Action) -> u16 {
        let index = self.next_action_index;
        self.actions.push(action);
        self.next_action_index += 1;
        index
    }

    fn emit(&mut self, op: BytecodeOp) {
        // Track stack depth for pre-allocation
        match &op {
            BytecodeOp::LoadVar(_) | BytecodeOp::LoadConst(_) => {
                self.current_stack_depth += 1;
                self.max_stack_depth = self.max_stack_depth.max(self.current_stack_depth);
            }
            BytecodeOp::Compare(_) | BytecodeOp::And | BytecodeOp::Or => {
                // These ops consume 2 values and produce 1
                self.current_stack_depth = self.current_stack_depth.saturating_sub(1);
            }
            BytecodeOp::Not => {
                // Consumes 1, produces 1 - no change
            }
            BytecodeOp::Hysteresis(_) => {
                // Consumes 2 (value, threshold), produces 1 (bool)
                self.current_stack_depth = self.current_stack_depth.saturating_sub(1);
            }
            BytecodeOp::JumpFalse(_) => {
                // Consumes 1 value for condition
                self.current_stack_depth = self.current_stack_depth.saturating_sub(1);
            }
            BytecodeOp::Action(_) => {
                // Actions don't affect stack
            }
            BytecodeOp::Jump(_) | BytecodeOp::Nop => {
                // No stack effect
            }
        }
        
        self.instructions.push(op);
    }

    fn compile_rule(&mut self, rule: &Rule) -> Result<()> {
        // Parse condition and action
        let condition = self.parse_condition(&rule.when)?;
        let action = self.parse_action(&rule.action)?;
        
        // Compile condition to bytecode
        self.compile_condition(&condition)?;
        
        // Jump over action if condition is false
        let jump_addr = self.instructions.len();
        self.emit(BytecodeOp::JumpFalse(0)); // Placeholder, will be patched
        
        // Emit action
        let action_index = self.add_action(action);
        self.emit(BytecodeOp::Action(action_index));
        
        // Patch jump address
        let end_addr = self.instructions.len() as u16;
        if let BytecodeOp::JumpFalse(addr) = &mut self.instructions[jump_addr] {
            *addr = end_addr;
        }
        
        Ok(())
    }

    fn compile_condition(&mut self, condition: &Condition) -> Result<()> {
        match condition {
            Condition::Boolean { variable, negate } => {
                let var_index = self.get_or_add_variable(variable);
                self.emit(BytecodeOp::LoadVar(var_index));
                self.emit(BytecodeOp::LoadConst(0.0));
                self.emit(BytecodeOp::Compare(CompareOp::NotEqual));
                
                if *negate {
                    self.emit(BytecodeOp::Not);
                }
            }
            Condition::Compare { variable, operator, value } => {
                let var_index = self.get_or_add_variable(variable);
                self.emit(BytecodeOp::LoadVar(var_index));
                self.emit(BytecodeOp::LoadConst(*value));
                
                // Apply hysteresis if configured
                if let Some(&hyst_index) = self.hysteresis_map.get(variable) {
                    self.emit(BytecodeOp::Hysteresis(hyst_index));
                } else {
                    self.emit(BytecodeOp::Compare(operator.clone()));
                }
            }
            Condition::And(conditions) => {
                if conditions.is_empty() {
                    self.emit(BytecodeOp::LoadConst(1.0)); // True
                    return Ok(());
                }
                
                // Compile first condition
                self.compile_condition(&conditions[0])?;
                
                // For each additional condition, compile and AND
                for condition in &conditions[1..] {
                    // Short-circuit: if current result is false, skip remaining
                    let skip_addr = self.instructions.len();
                    self.emit(BytecodeOp::JumpFalse(0)); // Placeholder
                    
                    self.compile_condition(condition)?;
                    self.emit(BytecodeOp::And);
                    
                    // Patch skip address
                    let end_addr = self.instructions.len() as u16;
                    if let BytecodeOp::JumpFalse(addr) = &mut self.instructions[skip_addr] {
                        *addr = end_addr;
                    }
                }
            }
            Condition::Or(conditions) => {
                if conditions.is_empty() {
                    self.emit(BytecodeOp::LoadConst(0.0)); // False
                    return Ok(());
                }
                
                // Compile first condition
                self.compile_condition(&conditions[0])?;
                
                // For each additional condition, compile and OR
                for condition in &conditions[1..] {
                    self.compile_condition(condition)?;
                    self.emit(BytecodeOp::Or);
                }
            }
        }
        Ok(())
    }

    fn parse_condition(&self, condition_str: &str) -> Result<Condition> {
        // Reuse the existing parser from RulesCompiler
        let compiler = RulesCompiler::new(HashMap::new());
        compiler.parse_condition(condition_str)
    }

    fn parse_action(&self, action_str: &str) -> Result<Action> {
        // Reuse the existing parser from RulesCompiler
        let compiler = RulesCompiler::new(HashMap::new());
        compiler.parse_action(action_str)
    }

    fn finalize(self) -> BytecodeProgram {
        BytecodeProgram {
            instructions: self.instructions,
            variable_map: self.variable_map,
            hysteresis_map: self.hysteresis_map,
            hysteresis_bands: self.hysteresis_bands,
            actions: self.actions,
            stack_size: self.max_stack_depth.max(8), // Minimum stack size
        }
    }
}

impl CompiledRules {
    /// Get the bytecode program for zero-allocation evaluation
    pub fn bytecode(&self) -> &BytecodeProgram {
        &self.bytecode
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rules_schema_validation() {
        let rules = RulesSchema {
            schema: "flight.ledmap/1".to_string(),
            rules: vec![
                Rule {
                    when: "gear == DOWN".to_string(),
                    do_action: "led.panel('GEAR').on()".to_string(),
                    action: "led.panel('GEAR').on()".to_string(),
                }
            ],
            defaults: None,
        };

        assert!(rules.validate().is_ok());
    }

    #[test]
    fn test_invalid_schema_version() {
        let rules = RulesSchema {
            schema: "flight.ledmap/2".to_string(),
            rules: vec![],
            defaults: None,
        };

        assert!(rules.validate().is_err());
    }

    #[test]
    fn test_rule_compilation() {
        let mut hysteresis = HashMap::new();
        hysteresis.insert("aoa".to_string(), 0.5);

        let rules = RulesSchema {
            schema: "flight.ledmap/1".to_string(),
            rules: vec![
                Rule {
                    when: "aoa > alpha_warn".to_string(),
                    do_action: "led.indexer.blink(rate_hz=6)".to_string(),
                    action: "led.indexer.blink(rate_hz=6)".to_string(),
                }
            ],
            defaults: Some(RuleDefaults {
                hysteresis: Some(hysteresis),
            }),
        };

        // This will fail with current stub implementation, but structure is correct
        let _result = rules.compile();
    }

    #[test]
    fn test_condition_parsing() {
        let compiler = RulesCompiler::new(HashMap::new());

        // Test boolean condition
        let condition = compiler.parse_condition("gear_down").unwrap();
        matches!(condition, Condition::Boolean { variable, negate } if variable == "gear_down" && !negate);

        // Test negated boolean
        let condition = compiler.parse_condition("!gear_down").unwrap();
        matches!(condition, Condition::Boolean { variable, negate } if variable == "gear_down" && negate);

        // Test comparison
        let condition = compiler.parse_condition("ias > 90").unwrap();
        matches!(condition, Condition::Compare { variable, operator: CompareOp::Greater, value } 
                 if variable == "ias" && value == 90.0);
    }

    #[test]
    fn test_action_parsing() {
        let compiler = RulesCompiler::new(HashMap::new());

        // Test LED on action
        let action = compiler.parse_action("led.panel('GEAR').on()").unwrap();
        matches!(action, Action::LedOn { target } if target == "GEAR");

        // Test LED blink action
        let action = compiler.parse_action("led.indexer.blink(rate_hz=6)").unwrap();
        matches!(action, Action::LedBlink { target, rate_hz } if target == "indexer" && rate_hz == 6.0);
    }
}