#![cfg_attr(
    test,
    allow(
        unused_imports,
        unused_variables,
        unused_mut,
        unused_assignments,
        unused_parens,
        dead_code
    )
)]
// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Flight panels integration and LED control

use flight_core::Result;
use flight_core::rules::{CompiledRules, RulesSchema};
use std::collections::HashMap;

pub use flight_panels_core::evaluator;
pub use flight_panels_core::led;
pub use flight_panels_cougar as cougar;
pub use flight_panels_saitek as saitek;
pub use flight_panels_saitek::verify_matrix;

pub mod protocol_handler;

#[cfg(test)]
mod allocation_test;

#[cfg(test)]
mod integration_test;

#[cfg(test)]
mod protocol_depth_test;

pub use flight_panels_core::RulesEvaluator;
pub use flight_panels_core::led::{LedController, LedTarget};
pub use flight_panels_cougar::{
    CougarMfdHealthStatus, CougarMfdType, CougarMfdWriter, CougarVerifyTestResult, MfdLedState,
};
pub use flight_panels_saitek::verify_matrix::{
    DriftAction, DriftAnalysis, MatrixTestResult, VerifyMatrix,
};
pub use flight_panels_saitek::{PanelHealthStatus, PanelType, SaitekPanelWriter, VerifyTestResult};

/// Panel manager for LED control and rules evaluation
pub struct PanelManager {
    compiled_rules: Option<CompiledRules>,
    led_controller: LedController,
    evaluator: RulesEvaluator,
    saitek_writer: Option<SaitekPanelWriter>,
    cougar_writer: Option<CougarMfdWriter>,
    verify_matrix: Option<VerifyMatrix>,
}

impl PanelManager {
    /// Create a new panel manager
    pub fn new() -> Self {
        Self {
            compiled_rules: None,
            led_controller: LedController::new(),
            evaluator: RulesEvaluator::new(),
            saitek_writer: None,
            cougar_writer: None,
            verify_matrix: None,
        }
    }

    /// Initialize Saitek panel writer with HID adapter
    pub fn initialize_saitek_writer(&mut self, hid_adapter: flight_hid::HidAdapter) -> Result<()> {
        let mut writer = SaitekPanelWriter::new(hid_adapter);
        writer.start()?;
        self.saitek_writer = Some(writer);
        Ok(())
    }

    /// Initialize Cougar MFD writer with HID adapter
    pub fn initialize_cougar_writer(&mut self, hid_adapter: flight_hid::HidAdapter) -> Result<()> {
        let mut writer = CougarMfdWriter::new(hid_adapter);
        writer.start()?;
        self.cougar_writer = Some(writer);
        Ok(())
    }

    /// Initialize verify matrix (requires Saitek writer to be initialized first)
    pub fn initialize_verify_matrix(&mut self) -> Result<()> {
        if let Some(writer) = self.saitek_writer.take() {
            let matrix = VerifyMatrix::new(writer);
            self.verify_matrix = Some(matrix);
            Ok(())
        } else {
            Err(flight_core::FlightError::Configuration(
                "Saitek writer must be initialized before verify matrix".to_string(),
            ))
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

        // Update Saitek panel blink states
        if let Some(saitek_writer) = &mut self.saitek_writer {
            saitek_writer.update_blink_states()?;
        } else if let Some(_matrix) = &mut self.verify_matrix {
            // If writer is in matrix, update through matrix
            // Note: In a real implementation, we'd need better access patterns
        }

        // Update Cougar MFD blink states
        if let Some(cougar_writer) = &mut self.cougar_writer {
            cougar_writer.update_blink_states()?;
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

    /// Start verify test for a Saitek panel
    pub fn start_saitek_verify_test(&mut self, panel_path: &str) -> Result<()> {
        if let Some(saitek_writer) = &mut self.saitek_writer {
            saitek_writer.start_verify_test(panel_path)
        } else {
            Err(flight_core::FlightError::Configuration(
                "Saitek writer not initialized".to_string(),
            ))
        }
    }

    /// Update Saitek verify test and get result if complete
    pub fn update_saitek_verify_test(&mut self) -> Result<Option<VerifyTestResult>> {
        if let Some(saitek_writer) = &mut self.saitek_writer {
            saitek_writer.update_verify_test()
        } else {
            Ok(None)
        }
    }

    /// Get Saitek panel health status
    pub fn check_saitek_panel_health(&mut self, panel_path: &str) -> Result<PanelHealthStatus> {
        if let Some(saitek_writer) = &mut self.saitek_writer {
            saitek_writer.check_panel_health(panel_path)
        } else {
            Err(flight_core::FlightError::Configuration(
                "Saitek writer not initialized".to_string(),
            ))
        }
    }

    /// Repair Saitek panel configuration drift
    pub fn repair_saitek_panel_drift(&mut self, panel_path: &str) -> Result<()> {
        if let Some(saitek_writer) = &mut self.saitek_writer {
            saitek_writer.repair_panel_drift(panel_path)
        } else {
            Err(flight_core::FlightError::Configuration(
                "Saitek writer not initialized".to_string(),
            ))
        }
    }

    /// Get connected Saitek panels
    pub fn get_saitek_panels(&self) -> Vec<&saitek::PanelInfo> {
        if let Some(saitek_writer) = &self.saitek_writer {
            saitek_writer.get_panels()
        } else {
            Vec::new()
        }
    }

    /// Get Saitek panel latency statistics
    pub fn get_saitek_latency_stats(&self) -> Option<led::LatencyStats> {
        if let Some(saitek_writer) = &self.saitek_writer {
            saitek_writer.get_latency_stats()
        } else {
            None
        }
    }

    /// Start verify test for a Cougar MFD
    pub fn start_cougar_verify_test(&mut self, mfd_path: &str) -> Result<()> {
        if let Some(cougar_writer) = &mut self.cougar_writer {
            cougar_writer.start_verify_test(mfd_path)
        } else {
            Err(flight_core::FlightError::Configuration(
                "Cougar writer not initialized".to_string(),
            ))
        }
    }

    /// Update Cougar verify test and get result if complete
    pub fn update_cougar_verify_test(&mut self) -> Result<Option<CougarVerifyTestResult>> {
        if let Some(cougar_writer) = &mut self.cougar_writer {
            cougar_writer.update_verify_test()
        } else {
            Ok(None)
        }
    }

    /// Get Cougar MFD health status
    pub fn check_cougar_mfd_health(&mut self, mfd_path: &str) -> Result<CougarMfdHealthStatus> {
        if let Some(cougar_writer) = &mut self.cougar_writer {
            cougar_writer.check_mfd_health(mfd_path)
        } else {
            Err(flight_core::FlightError::Configuration(
                "Cougar writer not initialized".to_string(),
            ))
        }
    }

    /// Repair Cougar MFD configuration drift
    pub fn repair_cougar_mfd_drift(&mut self, mfd_path: &str) -> Result<()> {
        if let Some(cougar_writer) = &mut self.cougar_writer {
            cougar_writer.repair_mfd_drift(mfd_path)
        } else {
            Err(flight_core::FlightError::Configuration(
                "Cougar writer not initialized".to_string(),
            ))
        }
    }

    /// Get connected Cougar MFDs
    pub fn get_cougar_mfds(&self) -> Vec<&cougar::MfdInfo> {
        if let Some(cougar_writer) = &self.cougar_writer {
            cougar_writer.get_mfds()
        } else {
            Vec::new()
        }
    }

    /// Get Cougar MFD latency statistics
    pub fn get_cougar_latency_stats(&self) -> Option<led::LatencyStats> {
        if let Some(cougar_writer) = &self.cougar_writer {
            cougar_writer.get_latency_stats()
        } else {
            None
        }
    }

    /// Run full verify matrix for all panels
    pub fn run_verify_matrix(&mut self) -> Result<Vec<MatrixTestResult>> {
        if let Some(matrix) = &mut self.verify_matrix {
            matrix.run_full_matrix()
        } else {
            Err(flight_core::FlightError::Configuration(
                "Verify matrix not initialized".to_string(),
            ))
        }
    }

    /// Run verify matrix for specific panel
    pub fn run_panel_verify_matrix(
        &mut self,
        panel_path: &str,
        panel_type: PanelType,
    ) -> Result<MatrixTestResult> {
        if let Some(matrix) = &mut self.verify_matrix {
            matrix.run_panel_matrix(panel_path, panel_type)
        } else {
            Err(flight_core::FlightError::Configuration(
                "Verify matrix not initialized".to_string(),
            ))
        }
    }

    /// Check if verify matrix run is needed
    pub fn needs_verify_matrix_run(&self) -> bool {
        if let Some(matrix) = &self.verify_matrix {
            matrix.needs_matrix_run()
        } else {
            false
        }
    }

    /// Get verify matrix drift analysis for panel
    pub fn get_drift_analysis(&self, panel_path: &str) -> Option<&Vec<VerifyTestResult>> {
        if let Some(matrix) = &self.verify_matrix {
            matrix.get_test_history(panel_path)
        } else {
            None
        }
    }
}

impl Default for PanelManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for PanelManager {
    fn drop(&mut self) {
        if let Some(saitek_writer) = &mut self.saitek_writer {
            saitek_writer.stop();
        }
        if let Some(cougar_writer) = &mut self.cougar_writer {
            cougar_writer.stop();
        }
    }
}
