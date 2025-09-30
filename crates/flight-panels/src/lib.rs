//! Flight panels integration and LED control

use flight_core::rules::{RulesSchema, CompiledRules};
use flight_core::Result;
use std::collections::HashMap;

pub mod evaluator;
pub mod led;

pub use evaluator::RulesEvaluator;
pub use led::{LedController, LedTarget};

/// Panel manager for LED control and rules evaluation
pub struct PanelManager {
    compiled_rules: Option<CompiledRules>,
    led_controller: LedController,
    evaluator: RulesEvaluator,
}

impl PanelManager {
    /// Create a new panel manager
    pub fn new() -> Self {
        Self {
            compiled_rules: None,
            led_controller: LedController::new(),
            evaluator: RulesEvaluator::new(),
        }
    }

    /// Load and compile rules from schema
    pub fn load_rules(&mut self, rules: RulesSchema) -> Result<()> {
        rules.validate()?;
        let compiled = rules.compile()?;
        self.compiled_rules = Some(compiled);
        Ok(())
    }

    /// Update panel state with telemetry data
    pub fn update(&mut self, telemetry: &HashMap<String, f32>) -> Result<()> {
        if let Some(rules) = &self.compiled_rules {
            let actions = self.evaluator.evaluate(rules, telemetry);
            self.led_controller.execute_actions(&actions)?;
        }
        Ok(())
    }

    /// Get LED controller for direct access
    pub fn led_controller(&mut self) -> &mut LedController {
        &mut self.led_controller
    }
}

impl Default for PanelManager {
    fn default() -> Self {
        Self::new()
    }
}
