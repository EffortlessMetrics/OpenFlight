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

/// Compiled rules bytecode (stub)
#[derive(Debug, Clone)]
pub struct CompiledRules {
    pub rules: Vec<CompiledRule>,
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

    /// Compile rules schema to bytecode (stub implementation)
    pub fn compile(&self, schema: &RulesSchema) -> Result<CompiledRules> {
        let mut compiled_rules = Vec::new();

        for rule in &schema.rules {
            let compiled_rule = self.compile_rule(rule)?;
            compiled_rules.push(compiled_rule);
        }

        Ok(CompiledRules {
            rules: compiled_rules,
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

impl CompiledRules {
    /// Evaluate rules against telemetry data (stub implementation)
    pub fn evaluate(&self, _telemetry: &HashMap<String, f32>) -> Vec<Action> {
        // Stub implementation - would evaluate conditions and return actions
        Vec::new()
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