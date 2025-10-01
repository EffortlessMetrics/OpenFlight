//! Flight panels integration and LED control

use flight_core::rules::{RulesSchema, CompiledRules};
use flight_core::Result;
use std::collections::HashMap;
use std::time::Instant;

pub mod evaluator;
pub mod led;

#[cfg(test)]
mod allocation_test;

#[cfg(test)]
mod integration_test;

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
        
        // Initialize evaluator for the new bytecode program
        self.evaluator.initialize_for_program(&compiled.bytecode);
        
        self.compiled_rules = Some(compiled);
        Ok(())
    }

    /// Update panel state with telemetry data
    pub fn update(&mut self, telemetry: &HashMap<String, f32>) -> Result<()> {
        if let Some(rules) = &self.compiled_rules {
            let actions = self.evaluator.evaluate(rules, telemetry);
            self.led_controller.execute_actions(actions)?;
        }
        Ok(())
    }

    /// Get LED controller for direct access
    pub fn led_controller(&mut self) -> &mut LedController {
        &mut self.led_controller
    }

    /// Trigger fault indication LEDs
    pub fn trigger_fault_indication(&mut self) -> Result<()> {
        // Flash all available LEDs in fault pattern
        let fault_actions = vec![
            flight_core::rules::Action::LedBlink {
                target: "FAULT_INDICATOR".to_string(),
                rate_hz: 4.0, // 4Hz blink for fault indication
            },
            flight_core::rules::Action::LedOn {
                target: "MASTER_WARNING".to_string(),
            },
        ];
        
        self.led_controller.execute_actions(&fault_actions)?;
        Ok(())
    }

    /// Trigger soft-stop indication LEDs
    pub fn trigger_soft_stop_indication(&mut self) -> Result<()> {
        // Solid red indication for soft-stop
        let soft_stop_actions = vec![
            flight_core::rules::Action::LedOn {
                target: "SOFT_STOP_INDICATOR".to_string(),
            },
            flight_core::rules::Action::LedBrightness {
                target: "SOFT_STOP_INDICATOR".to_string(),
                brightness: 1.0,
            },
        ];
        
        self.led_controller.execute_actions(&soft_stop_actions)?;
        Ok(())
    }

    /// Clear fault indications
    pub fn clear_fault_indication(&mut self) -> Result<()> {
        let clear_actions = vec![
            flight_core::rules::Action::LedOff {
                target: "FAULT_INDICATOR".to_string(),
            },
            flight_core::rules::Action::LedOff {
                target: "MASTER_WARNING".to_string(),
            },
            flight_core::rules::Action::LedOff {
                target: "SOFT_STOP_INDICATOR".to_string(),
            },
        ];
        
        self.led_controller.execute_actions(&clear_actions)?;
        Ok(())
    }
}

impl Default for PanelManager {
    fn default() -> Self {
        Self::new()
    }
}
