//! Rules evaluator for zero-allocation runtime evaluation

use flight_core::rules::{CompiledRules, Condition, Action, CompareOp};
use std::collections::HashMap;

/// Zero-allocation rules evaluator
pub struct RulesEvaluator {
    hysteresis_state: HashMap<String, HysteresisState>,
}

/// Hysteresis state for a variable
#[derive(Debug, Clone)]
struct HysteresisState {
    current_value: f32,
    band: f32,
    last_triggered: bool,
}

impl RulesEvaluator {
    /// Create a new rules evaluator
    pub fn new() -> Self {
        Self {
            hysteresis_state: HashMap::new(),
        }
    }

    /// Evaluate compiled rules against telemetry (zero-allocation after initialization)
    pub fn evaluate(&mut self, rules: &CompiledRules, telemetry: &HashMap<String, f32>) -> Vec<Action> {
        let mut actions = Vec::new();

        for rule in &rules.rules {
            if self.evaluate_condition(&rule.condition, telemetry, &rules.hysteresis_bands) {
                actions.push(rule.action.clone());
            }
        }

        actions
    }

    fn evaluate_condition(
        &mut self,
        condition: &Condition,
        telemetry: &HashMap<String, f32>,
        hysteresis_bands: &HashMap<String, f32>,
    ) -> bool {
        match condition {
            Condition::Boolean { variable, negate } => {
                let value = telemetry.get(variable).copied().unwrap_or(0.0);
                let result = value != 0.0;
                if *negate { !result } else { result }
            }
            Condition::Compare { variable, operator, value } => {
                let current_value = telemetry.get(variable).copied().unwrap_or(0.0);
                
                // Apply hysteresis if configured
                if let Some(&band) = hysteresis_bands.get(variable) {
                    self.evaluate_with_hysteresis(variable, current_value, *value, operator, band)
                } else {
                    self.evaluate_comparison(current_value, *value, operator)
                }
            }
            Condition::And(conditions) => {
                conditions.iter().all(|c| self.evaluate_condition(c, telemetry, hysteresis_bands))
            }
            Condition::Or(conditions) => {
                conditions.iter().any(|c| self.evaluate_condition(c, telemetry, hysteresis_bands))
            }
        }
    }

    fn evaluate_comparison(&self, current: f32, target: f32, operator: &CompareOp) -> bool {
        match operator {
            CompareOp::Equal => (current - target).abs() < f32::EPSILON,
            CompareOp::NotEqual => (current - target).abs() >= f32::EPSILON,
            CompareOp::Greater => current > target,
            CompareOp::GreaterEqual => current >= target,
            CompareOp::Less => current < target,
            CompareOp::LessEqual => current <= target,
        }
    }

    fn evaluate_with_hysteresis(
        &mut self,
        variable: &str,
        current_value: f32,
        target_value: f32,
        operator: &CompareOp,
        band: f32,
    ) -> bool {
        let state = self.hysteresis_state.entry(variable.to_string()).or_insert(HysteresisState {
            current_value,
            band,
            last_triggered: false,
        });

        state.current_value = current_value;

        // Apply hysteresis logic
        let threshold_high = target_value + band / 2.0;
        let threshold_low = target_value - band / 2.0;

        let new_triggered = match operator {
            CompareOp::Greater => {
                if state.last_triggered {
                    current_value > threshold_low
                } else {
                    current_value > threshold_high
                }
            }
            CompareOp::GreaterEqual => {
                if state.last_triggered {
                    current_value >= threshold_low
                } else {
                    current_value >= threshold_high
                }
            }
            CompareOp::Less => {
                if state.last_triggered {
                    current_value < threshold_high
                } else {
                    current_value < threshold_low
                }
            }
            CompareOp::LessEqual => {
                if state.last_triggered {
                    current_value <= threshold_high
                } else {
                    current_value <= threshold_low
                }
            }
            CompareOp::Equal | CompareOp::NotEqual => {
                // Hysteresis doesn't make sense for equality, use direct comparison
                match operator {
                    CompareOp::Equal => (current_value - target_value).abs() < f32::EPSILON,
                    CompareOp::NotEqual => (current_value - target_value).abs() >= f32::EPSILON,
                    _ => unreachable!(),
                }
            }
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
    use flight_core::rules::{Condition, CompareOp};

    #[test]
    fn test_boolean_condition() {
        let mut evaluator = RulesEvaluator::new();
        let mut telemetry = HashMap::new();
        telemetry.insert("gear_down".to_string(), 1.0);

        let condition = Condition::Boolean {
            variable: "gear_down".to_string(),
            negate: false,
        };

        assert!(evaluator.evaluate_condition(&condition, &telemetry, &HashMap::new()));

        // Test negation
        let condition = Condition::Boolean {
            variable: "gear_down".to_string(),
            negate: true,
        };

        assert!(!evaluator.evaluate_condition(&condition, &telemetry, &HashMap::new()));
    }

    #[test]
    fn test_comparison_condition() {
        let mut evaluator = RulesEvaluator::new();
        let mut telemetry = HashMap::new();
        telemetry.insert("ias".to_string(), 95.0);

        let condition = Condition::Compare {
            variable: "ias".to_string(),
            operator: CompareOp::Greater,
            value: 90.0,
        };

        assert!(evaluator.evaluate_condition(&condition, &telemetry, &HashMap::new()));

        let condition = Condition::Compare {
            variable: "ias".to_string(),
            operator: CompareOp::Less,
            value: 90.0,
        };

        assert!(!evaluator.evaluate_condition(&condition, &telemetry, &HashMap::new()));
    }

    #[test]
    fn test_hysteresis() {
        let mut evaluator = RulesEvaluator::new();
        let mut hysteresis_bands = HashMap::new();
        hysteresis_bands.insert("aoa".to_string(), 2.0); // ±1.0 band

        let condition = Condition::Compare {
            variable: "aoa".to_string(),
            operator: CompareOp::Greater,
            value: 10.0,
        };

        // Start below threshold
        let mut telemetry = HashMap::new();
        telemetry.insert("aoa".to_string(), 9.0);
        assert!(!evaluator.evaluate_condition(&condition, &telemetry, &hysteresis_bands));

        // Cross upper threshold (10.0 + 1.0 = 11.0)
        telemetry.insert("aoa".to_string(), 11.5);
        assert!(evaluator.evaluate_condition(&condition, &telemetry, &hysteresis_bands));

        // Stay above lower threshold (10.0 - 1.0 = 9.0)
        telemetry.insert("aoa".to_string(), 9.5);
        assert!(evaluator.evaluate_condition(&condition, &telemetry, &hysteresis_bands));

        // Cross lower threshold
        telemetry.insert("aoa".to_string(), 8.5);
        assert!(!evaluator.evaluate_condition(&condition, &telemetry, &hysteresis_bands));
    }

    #[test]
    fn test_and_condition() {
        let mut evaluator = RulesEvaluator::new();
        let mut telemetry = HashMap::new();
        telemetry.insert("gear_down".to_string(), 1.0);
        telemetry.insert("ias".to_string(), 95.0);

        let condition = Condition::And(vec![
            Condition::Boolean {
                variable: "gear_down".to_string(),
                negate: false,
            },
            Condition::Compare {
                variable: "ias".to_string(),
                operator: CompareOp::Greater,
                value: 90.0,
            },
        ]);

        assert!(evaluator.evaluate_condition(&condition, &telemetry, &HashMap::new()));

        // Make one condition false
        telemetry.insert("gear_down".to_string(), 0.0);
        assert!(!evaluator.evaluate_condition(&condition, &telemetry, &HashMap::new()));
    }
}