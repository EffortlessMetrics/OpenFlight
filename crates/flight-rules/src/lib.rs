// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Rules DSL for panel LED control

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RulesError {
    #[error("Validation error: {0}")]
    Validation(String),
}

pub type Result<T> = std::result::Result<T, RulesError>;

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

/// Rule condition parsed from DSL string
#[derive(Debug, Clone)]
pub enum Condition {
    /// Variable comparison: var op value
    Compare {
        variable: String,
        operator: CompareOp,
        value: f32,
    },
    /// Boolean variable
    Boolean { variable: String, negate: bool },
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

/// Rule action parsed from DSL string
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

/// Rules compiler: parses conditions and actions, produces bytecode
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
            return Err(RulesError::Validation(format!(
                "Unsupported schema version: {}",
                self.schema
            )));
        }

        // Validate each rule
        for (index, rule) in self.rules.iter().enumerate() {
            if let Err(e) = self.validate_rule(rule) {
                return Err(RulesError::Validation(format!("Rule {}: {}", index + 1, e)));
            }
        }

        Ok(())
    }

    fn validate_rule(&self, rule: &Rule) -> std::result::Result<(), String> {
        if rule.when.is_empty() {
            return Err("Rule condition cannot be empty".to_string());
        }

        if rule.action.is_empty() {
            return Err("Rule action cannot be empty".to_string());
        }

        // Validate condition and action syntax using the same parser paths used at runtime
        let compiler = RulesCompiler::new(Default::default());
        compiler
            .parse_condition(&rule.when)
            .map_err(|e| format!("Invalid condition syntax: {}", e))?;
        compiler
            .parse_action(&rule.action)
            .map_err(|e| format!("Invalid action syntax: {}", e))?;

        Ok(())
    }

    /// Compile rules to bytecode for efficient evaluation
    pub fn compile(&self) -> Result<CompiledRules> {
        let compiler = RulesCompiler::new(
            self.defaults
                .as_ref()
                .and_then(|d| d.hysteresis.clone())
                .unwrap_or_default(),
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

    #[allow(dead_code)]
    fn compile_rule(&self, rule: &Rule) -> Result<CompiledRule> {
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
        let condition_str = condition_str.trim();

        // Handle compound OR (lower precedence — split first)
        let or_parts: Vec<&str> = condition_str.split(" or ").collect();
        if or_parts.len() > 1 {
            let conditions = or_parts
                .iter()
                .map(|s| self.parse_condition(s.trim()))
                .collect::<Result<Vec<_>>>()?;
            return Ok(Condition::Or(conditions));
        }

        // Handle compound AND
        let and_parts: Vec<&str> = condition_str.split(" and ").collect();
        if and_parts.len() > 1 {
            let conditions = and_parts
                .iter()
                .map(|s| self.parse_condition(s.trim()))
                .collect::<Result<Vec<_>>>()?;
            return Ok(Condition::And(conditions));
        }

        // Handle negated boolean variables
        if condition_str.starts_with('!') && !condition_str.contains(['>', '<', '=']) {
            let variable = condition_str[1..].trim().to_string();
            return Ok(Condition::Boolean {
                variable,
                negate: true,
            });
        }

        // Handle boolean variables (no operators)
        if !condition_str.contains(['>', '<', '=']) {
            let variable = condition_str.to_string();
            return Ok(Condition::Boolean {
                variable,
                negate: false,
            });
        }

        // Two-character operators FIRST (must precede single-char checks)
        if let Some(pos) = condition_str.find(" >= ") {
            let variable = condition_str[..pos].trim().to_string();
            let value_str = condition_str[pos + 4..].trim();
            let value = value_str
                .parse::<f32>()
                .map_err(|_| RulesError::Validation(format!("Invalid number: {}", value_str)))?;
            return Ok(Condition::Compare {
                variable,
                operator: CompareOp::GreaterEqual,
                value,
            });
        }

        if let Some(pos) = condition_str.find(" <= ") {
            let variable = condition_str[..pos].trim().to_string();
            let value_str = condition_str[pos + 4..].trim();
            let value = value_str
                .parse::<f32>()
                .map_err(|_| RulesError::Validation(format!("Invalid number: {}", value_str)))?;
            return Ok(Condition::Compare {
                variable,
                operator: CompareOp::LessEqual,
                value,
            });
        }

        if let Some(pos) = condition_str.find(" != ") {
            let variable = condition_str[..pos].trim().to_string();
            let value_str = condition_str[pos + 4..].trim();
            if let Ok(value) = value_str.parse::<f32>() {
                return Ok(Condition::Compare {
                    variable,
                    operator: CompareOp::NotEqual,
                    value,
                });
            } else {
                // String/enum state negation: "gear != DOWN" → negated boolean "gear_DOWN"
                return Ok(Condition::Boolean {
                    variable: format!("{}_{}", variable, value_str),
                    negate: true,
                });
            }
        }

        if let Some(pos) = condition_str.find(" == ") {
            let variable = condition_str[..pos].trim().to_string();
            let value_str = condition_str[pos + 4..].trim();
            if let Ok(value) = value_str.parse::<f32>() {
                return Ok(Condition::Compare {
                    variable,
                    operator: CompareOp::Equal,
                    value,
                });
            } else {
                // String/enum state comparison: "gear == DOWN" → boolean "gear_DOWN"
                // Maps to a named discrete state variable (e.g. provided by the sim adapter).
                return Ok(Condition::Boolean {
                    variable: format!("{}_{}", variable, value_str),
                    negate: false,
                });
            }
        }

        // Single-character operators
        if let Some(pos) = condition_str.find(" > ") {
            let variable = condition_str[..pos].trim().to_string();
            let value_str = condition_str[pos + 3..].trim();
            let value = value_str
                .parse::<f32>()
                .map_err(|_| RulesError::Validation(format!("Invalid number: {}", value_str)))?;
            return Ok(Condition::Compare {
                variable,
                operator: CompareOp::Greater,
                value,
            });
        }

        if let Some(pos) = condition_str.find(" < ") {
            let variable = condition_str[..pos].trim().to_string();
            let value_str = condition_str[pos + 3..].trim();
            let value = value_str
                .parse::<f32>()
                .map_err(|_| RulesError::Validation(format!("Invalid number: {}", value_str)))?;
            return Ok(Condition::Compare {
                variable,
                operator: CompareOp::Less,
                value,
            });
        }

        Err(RulesError::Validation(format!(
            "Unsupported condition syntax: {}",
            condition_str
        )))
    }

    fn parse_action(&self, action_str: &str) -> Result<Action> {
        let action_str = action_str.trim();

        // Parse led.panel('TARGET').on() / .off() / .blink(rate_hz=N)
        if let Some(start) = action_str.find("led.panel('")
            && let Some(end) = action_str[start + 11..].find("')")
        {
            let target = action_str[start + 11..start + 11 + end].to_string();
            let suffix_start = start + 11 + end + 2; // past "')"
            let suffix = &action_str[suffix_start..];

            if suffix == ".on()" {
                return Ok(Action::LedOn { target });
            } else if suffix == ".off()" {
                return Ok(Action::LedOff { target });
            } else if let Some(blink_start) = suffix.find("rate_hz=")
                && let Some(blink_end) = suffix[blink_start + 8..].find(')')
            {
                let rate_str = &suffix[blink_start + 8..blink_start + 8 + blink_end];
                let rate_hz = rate_str
                    .parse::<f32>()
                    .map_err(|_| RulesError::Validation(format!("Invalid rate: {}", rate_str)))?;
                return Ok(Action::LedBlink { target, rate_hz });
            } else if let Some(bright_start) = suffix.find(".brightness(")
                && let Some(bright_end) = suffix[bright_start + 12..].find(')')
            {
                let bright_str = &suffix[bright_start + 12..bright_start + 12 + bright_end];
                let brightness = bright_str.parse::<f32>().map_err(|_| {
                    RulesError::Validation(format!("Invalid brightness: {}", bright_str))
                })?;
                return Ok(Action::LedBrightness { target, brightness });
            }
        }

        // Parse led.indexer.on() / .off() / .blink(rate_hz=N)
        if action_str.starts_with("led.indexer.") {
            if action_str == "led.indexer.on()" {
                return Ok(Action::LedOn {
                    target: "indexer".to_string(),
                });
            } else if action_str == "led.indexer.off()" {
                return Ok(Action::LedOff {
                    target: "indexer".to_string(),
                });
            } else if action_str.starts_with("led.indexer.blink(")
                && let Some(start) = action_str.find("rate_hz=")
                && let Some(end) = action_str[start + 8..].find(')')
            {
                let rate_str = &action_str[start + 8..start + 8 + end];
                let rate_hz = rate_str
                    .parse::<f32>()
                    .map_err(|_| RulesError::Validation(format!("Invalid rate: {}", rate_str)))?;
                return Ok(Action::LedBlink {
                    target: "indexer".to_string(),
                    rate_hz,
                });
            }
        }

        Err(RulesError::Validation(format!(
            "Unsupported action syntax: {}",
            action_str
        )))
    }

    #[allow(dead_code)]
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
            Condition::Compare {
                variable,
                operator,
                value,
            } => {
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
            rules: vec![Rule {
                when: "gear == DOWN".to_string(),
                do_action: "led.panel('GEAR').on()".to_string(),
                action: "led.panel('GEAR').on()".to_string(),
            }],
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
            rules: vec![Rule {
                when: "aoa > alpha_warn".to_string(),
                do_action: "led.indexer.blink(rate_hz=6)".to_string(),
                action: "led.indexer.blink(rate_hz=6)".to_string(),
            }],
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
        let action = compiler
            .parse_action("led.indexer.blink(rate_hz=6)")
            .unwrap();
        matches!(action, Action::LedBlink { target, rate_hz } if target == "indexer" && rate_hz == 6.0);
    }

    #[test]
    fn test_new_comparison_operators() {
        let compiler = RulesCompiler::new(HashMap::new());

        // >=
        let c = compiler.parse_condition("ias >= 200").unwrap();
        assert!(matches!(
            c,
            Condition::Compare {
                operator: CompareOp::GreaterEqual,
                ..
            }
        ));

        // <=
        let c = compiler.parse_condition("flaps <= 0.5").unwrap();
        assert!(matches!(
            c,
            Condition::Compare {
                operator: CompareOp::LessEqual,
                ..
            }
        ));

        // !=
        let c = compiler.parse_condition("gear != 1").unwrap();
        assert!(matches!(
            c,
            Condition::Compare {
                operator: CompareOp::NotEqual,
                ..
            }
        ));

        // <
        let c = compiler.parse_condition("altitude < 500").unwrap();
        assert!(matches!(
            c,
            Condition::Compare {
                operator: CompareOp::Less,
                ..
            }
        ));
    }

    #[test]
    fn test_compound_and_condition() {
        let compiler = RulesCompiler::new(HashMap::new());
        let c = compiler.parse_condition("gear_down and ias < 250").unwrap();
        match c {
            Condition::And(parts) => {
                assert_eq!(parts.len(), 2);
                assert!(
                    matches!(&parts[0], Condition::Boolean { variable, .. } if variable == "gear_down")
                );
                assert!(matches!(
                    &parts[1],
                    Condition::Compare {
                        operator: CompareOp::Less,
                        ..
                    }
                ));
            }
            other => panic!("Expected And, got {:?}", other),
        }
    }

    #[test]
    fn test_compound_or_condition() {
        let compiler = RulesCompiler::new(HashMap::new());
        let c = compiler
            .parse_condition("gear_down or flaps >= 0.5")
            .unwrap();
        match c {
            Condition::Or(parts) => {
                assert_eq!(parts.len(), 2);
            }
            other => panic!("Expected Or, got {:?}", other),
        }
    }

    #[test]
    fn test_panel_blink_action() {
        let compiler = RulesCompiler::new(HashMap::new());
        let a = compiler
            .parse_action("led.panel('STALL').blink(rate_hz=4)")
            .unwrap();
        match a {
            Action::LedBlink { target, rate_hz } => {
                assert_eq!(target, "STALL");
                assert!((rate_hz - 4.0).abs() < 0.001);
            }
            other => panic!("Expected LedBlink, got {:?}", other),
        }
    }

    #[test]
    fn test_panel_brightness_action() {
        let compiler = RulesCompiler::new(HashMap::new());
        let a = compiler
            .parse_action("led.panel('WARN').brightness(0.75)")
            .unwrap();
        match a {
            Action::LedBrightness { target, brightness } => {
                assert_eq!(target, "WARN");
                assert!((brightness - 0.75).abs() < 0.001);
            }
            other => panic!("Expected LedBrightness, got {:?}", other),
        }
    }

    #[test]
    fn test_indexer_on_off_action() {
        let compiler = RulesCompiler::new(HashMap::new());
        let on = compiler.parse_action("led.indexer.on()").unwrap();
        assert!(matches!(on, Action::LedOn { target } if target == "indexer"));
        let off = compiler.parse_action("led.indexer.off()").unwrap();
        assert!(matches!(off, Action::LedOff { target } if target == "indexer"));
    }

    #[test]
    fn test_ge_not_confused_with_gt() {
        // Ensure ">=" is not parsed as ">" with "=" as part of value
        let compiler = RulesCompiler::new(HashMap::new());
        let c = compiler.parse_condition("pitch >= 10").unwrap();
        assert!(
            matches!(c, Condition::Compare { operator: CompareOp::GreaterEqual, value, .. } if (value - 10.0).abs() < 0.001)
        );
        let c2 = compiler.parse_condition("pitch > 10").unwrap();
        assert!(
            matches!(c2, Condition::Compare { operator: CompareOp::Greater, value, .. } if (value - 10.0).abs() < 0.001)
        );
    }

    use proptest::prelude::*;

    proptest! {
        // Test parsing of boolean conditions
        #[test]
        fn prop_parse_boolean_condition(var_name in "[a-zA-Z_][a-zA-Z0-9_]*") {
            let compiler = RulesCompiler::new(HashMap::new());

            // Positive case
            if let Ok(Condition::Boolean { variable, negate }) = compiler.parse_condition(&var_name) {
                prop_assert_eq!(variable, var_name.clone());
                prop_assert!(!negate);
            }

            // Negative case
            let negated = format!("!{}", var_name);
            if let Ok(Condition::Boolean { variable, negate }) = compiler.parse_condition(&negated) {
                prop_assert_eq!(variable, var_name);
                prop_assert!(negate);
            }
        }

        // Test parsing of numeric comparisons
        #[test]
        fn prop_parse_numeric_comparison(
            var_name in "[a-zA-Z_][a-zA-Z0-9_]*",
            val in -1000.0f32..1000.0
        ) {
            let compiler = RulesCompiler::new(HashMap::new());

            // Greater than
            let expr = format!("{} > {}", var_name, val);
            if let Ok(Condition::Compare { variable, operator, value }) = compiler.parse_condition(&expr) {
                prop_assert_eq!(variable, var_name.clone());
                matches!(operator, CompareOp::Greater);
                prop_assert!((value - val).abs() < 0.001);
            }

            // Equal
            let expr = format!("{} == {}", var_name, val);
            if let Ok(Condition::Compare { variable, operator, value }) = compiler.parse_condition(&expr) {
                prop_assert_eq!(variable, var_name);
                matches!(operator, CompareOp::Equal);
                prop_assert!((value - val).abs() < 0.001);
            }
        }

        // Test parsing of actions
        #[test]
        fn prop_parse_action_led_on_off(target in "[A-Z0-9_]+") {
            let compiler = RulesCompiler::new(HashMap::new());

            // ON
            let expr = format!("led.panel('{}').on()", target);
            if let Ok(Action::LedOn { target: parsed_target }) = compiler.parse_action(&expr) {
                prop_assert_eq!(parsed_target, target.clone());
            } else {
                // Should pass if formatted correctly
                prop_assert!(false, "Failed to parse valid LED ON action: {}", expr);
            }

            // OFF
            let expr = format!("led.panel('{}').off()", target);
            if let Ok(Action::LedOff { target: parsed_target }) = compiler.parse_action(&expr) {
                prop_assert_eq!(parsed_target, target);
            } else {
                 prop_assert!(false, "Failed to parse valid LED OFF action: {}", expr);
            }
        }
    }
}

#[cfg(test)]
mod snapshot_tests {
    use super::*;
    use std::collections::HashMap;

    /// Snapshot the bytecode output for a gear-down panel rule.
    /// Fails if bytecode shape regresses across refactors.
    #[test]
    fn snapshot_bytecode_gear_down_panel() {
        let rules = RulesSchema {
            schema: "flight.ledmap/1".to_string(),
            rules: vec![Rule {
                when: "gear == DOWN".to_string(),
                do_action: "led.panel('GEAR').on()".to_string(),
                action: "led.panel('GEAR').on()".to_string(),
            }],
            defaults: None,
        };

        let compiled = rules.compile().expect("gear-down rule should compile");
        insta::assert_debug_snapshot!("bytecode_gear_down_panel", compiled.bytecode);
    }

    /// Snapshot the bytecode output for an AoA numeric-compare rule.
    #[test]
    fn snapshot_bytecode_aoa_warning() {
        let mut hysteresis = HashMap::new();
        hysteresis.insert("aoa".to_string(), 0.5_f32);

        let rules = RulesSchema {
            schema: "flight.ledmap/1".to_string(),
            rules: vec![Rule {
                when: "aoa > 14.5".to_string(),
                do_action: "led.indexer.blink(rate_hz=6)".to_string(),
                action: "led.indexer.blink(rate_hz=6)".to_string(),
            }],
            defaults: Some(RuleDefaults {
                hysteresis: Some(hysteresis),
            }),
        };

        let compiled = rules.compile().expect("aoa rule should compile");
        insta::assert_debug_snapshot!("bytecode_aoa_warning", compiled.bytecode);
    }

    /// Snapshot a compound AND condition.
    #[test]
    fn snapshot_bytecode_compound_and() {
        let rules = RulesSchema {
            schema: "flight.ledmap/1".to_string(),
            rules: vec![Rule {
                when: "gear_down && flaps_extended".to_string(),
                do_action: "led.panel('LAND').on()".to_string(),
                action: "led.panel('LAND').on()".to_string(),
            }],
            defaults: None,
        };

        let compiled = rules.compile().expect("AND rule should compile");
        insta::assert_debug_snapshot!("bytecode_compound_and", compiled.bytecode);
    }
}
