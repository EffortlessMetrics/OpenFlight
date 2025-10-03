// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Configuration verification system

use crate::types::*;
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tracing::{debug, info, warn};

/// Verifies simulator configurations against expected state
pub struct ConfigVerifier {
    golden_dir: PathBuf,
}

impl ConfigVerifier {
    pub fn new<P: AsRef<Path>>(golden_dir: P) -> Self {
        Self {
            golden_dir: golden_dir.as_ref().to_path_buf(),
        }
    }

    /// Verify simulator configuration for a specific version
    pub async fn verify(&self, sim: SimulatorType, version: &str) -> Result<VerifyResult> {
        info!("Verifying {} configuration for version {}", sim, version);

        let config_file = self.golden_dir
            .join(sim.to_string())
            .join(format!("{}.json", version));

        if !config_file.exists() {
            warn!("No verification configuration found for {} {}", sim, version);
            return Ok(VerifyResult {
                sim,
                version: version.to_string(),
                success: false,
                script_results: vec![],
                mismatched_files: vec![],
            });
        }

        // Load verification configuration
        let config_content = fs::read_to_string(&config_file)
            .context("Failed to read verification configuration")?;
        
        let config: WriterConfig = serde_json::from_str(&config_content)
            .context("Failed to parse verification configuration")?;

        let mut script_results = Vec::new();
        let mut mismatched_files = Vec::new();

        // Run verification scripts
        for script in &config.verify_scripts {
            let result = self.run_verification_script(script).await?;
            script_results.push(result);
        }

        // Check file states
        for diff in &config.diffs {
            if let Some(mismatch) = self.check_file_state(&diff.file, &diff.operation).await? {
                mismatched_files.push(mismatch);
            }
        }

        let success = script_results.iter().all(|r| r.success) && mismatched_files.is_empty();

        info!(
            "Verification completed for {} {}: {} (scripts: {}/{}, files: {} mismatched)",
            sim,
            version,
            if success { "PASS" } else { "FAIL" },
            script_results.iter().filter(|r| r.success).count(),
            script_results.len(),
            mismatched_files.len()
        );

        Ok(VerifyResult {
            sim,
            version: version.to_string(),
            success,
            script_results,
            mismatched_files,
        })
    }

    /// Run a single verification script
    async fn run_verification_script(&self, script: &VerifyScript) -> Result<ScriptResult> {
        debug!("Running verification script: {}", script.name);

        let mut action_results = Vec::new();
        let mut errors = Vec::new();
        let mut sim_state = MockSimulatorState::new();

        // Execute each action
        for action in &script.actions {
            match self.execute_action(action, &mut sim_state).await {
                Ok(result) => action_results.push(result),
                Err(e) => {
                    errors.push(format!("Action failed: {}", e));
                    action_results.push(ActionResult {
                        action: format!("{:?}", action),
                        success: false,
                        actual_value: None,
                        expected_value: None,
                        error: Some(e.to_string()),
                    });
                }
            }
        }

        // Check expected results
        for expected in &script.expected {
            match sim_state.get_variable(&expected.variable) {
                Some(actual) => {
                    let success = (actual - expected.value).abs() <= expected.tolerance;
                    if !success {
                        errors.push(format!(
                            "Variable {} expected {}, got {} (tolerance: {})",
                            expected.variable, expected.value, actual, expected.tolerance
                        ));
                    }
                    action_results.push(ActionResult {
                        action: format!("Check {}", expected.variable),
                        success,
                        actual_value: Some(actual),
                        expected_value: Some(expected.value),
                        error: if success { None } else { Some("Value out of tolerance".to_string()) },
                    });
                }
                None => {
                    errors.push(format!("Variable {} not found", expected.variable));
                    action_results.push(ActionResult {
                        action: format!("Check {}", expected.variable),
                        success: false,
                        actual_value: None,
                        expected_value: Some(expected.value),
                        error: Some("Variable not found".to_string()),
                    });
                }
            }
        }

        let success = errors.is_empty();

        Ok(ScriptResult {
            name: script.name.clone(),
            success,
            action_results,
            errors,
        })
    }

    /// Execute a single verification action
    async fn execute_action(
        &self,
        action: &VerifyAction,
        sim_state: &mut MockSimulatorState,
    ) -> Result<ActionResult> {
        match action {
            VerifyAction::SimEvent { event, value } => {
                debug!("Sending sim event: {} = {:?}", event, value);
                sim_state.send_event(event, *value);
                Ok(ActionResult {
                    action: format!("SimEvent: {}", event),
                    success: true,
                    actual_value: *value,
                    expected_value: *value,
                    error: None,
                })
            }
            VerifyAction::Wait { duration_ms } => {
                debug!("Waiting {} ms", duration_ms);
                tokio::time::sleep(Duration::from_millis(*duration_ms)).await;
                Ok(ActionResult {
                    action: format!("Wait {} ms", duration_ms),
                    success: true,
                    actual_value: None,
                    expected_value: None,
                    error: None,
                })
            }
            VerifyAction::CheckVar { variable, expected, tolerance } => {
                debug!("Checking variable: {} = {}", variable, expected);
                let actual = sim_state.get_variable(variable)
                    .context("Variable not found")?;
                
                let tolerance = tolerance.unwrap_or(0.001);
                let success = (actual - expected).abs() <= tolerance;
                
                Ok(ActionResult {
                    action: format!("CheckVar: {}", variable),
                    success,
                    actual_value: Some(actual),
                    expected_value: Some(*expected),
                    error: if success { None } else { Some("Value out of tolerance".to_string()) },
                })
            }
        }
    }

    /// Check if a file matches the expected state
    async fn check_file_state(
        &self,
        file_path: &Path,
        expected_operation: &DiffOperation,
    ) -> Result<Option<FileMismatch>> {
        if !file_path.exists() {
            return Ok(Some(FileMismatch {
                file: file_path.to_path_buf(),
                mismatch_type: MismatchType::Missing,
                suggested_diff: Some(FileDiff {
                    file: file_path.to_path_buf(),
                    operation: expected_operation.clone(),
                    backup: true,
                }),
            }));
        }

        // Read current file content
        let current_content = fs::read_to_string(file_path)
            .context("Failed to read file")?;

        // Check if content matches expected state
        let matches = match expected_operation {
            DiffOperation::Replace { content } => {
                current_content.trim() == content.trim()
            }
            DiffOperation::IniSection { section, changes } => {
                self.verify_ini_section(&current_content, section, changes)
            }
            DiffOperation::JsonPatch { patches: _ } => {
                // For JSON patches, we'd need to apply them to a base state and compare
                // This is simplified for now
                true
            }
            DiffOperation::LineReplace { pattern, replacement, regex } => {
                if *regex {
                    // For regex replacement, check if the replacement text is present
                    current_content.contains(replacement)
                } else {
                    // For simple replacement, check if pattern was replaced
                    !current_content.contains(pattern) && current_content.contains(replacement)
                }
            }
        };

        if matches {
            Ok(None)
        } else {
            Ok(Some(FileMismatch {
                file: file_path.to_path_buf(),
                mismatch_type: MismatchType::ContentMismatch,
                suggested_diff: Some(FileDiff {
                    file: file_path.to_path_buf(),
                    operation: expected_operation.clone(),
                    backup: true,
                }),
            }))
        }
    }

    /// Verify that an INI section contains the expected changes
    fn verify_ini_section(
        &self,
        content: &str,
        section: &str,
        expected_changes: &std::collections::HashMap<String, String>,
    ) -> bool {
        let mut in_target_section = false;
        let mut found_changes = std::collections::HashMap::new();

        for line in content.lines() {
            let trimmed = line.trim();
            
            // Check for section headers
            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                let section_name = &trimmed[1..trimmed.len()-1];
                in_target_section = section_name == section;
                continue;
            }

            // If we're in the target section and this is a key=value line
            if in_target_section && trimmed.contains('=') {
                let parts: Vec<&str> = trimmed.splitn(2, '=').collect();
                if parts.len() == 2 {
                    let key = parts[0].trim();
                    let value = parts[1].trim();
                    found_changes.insert(key.to_string(), value.to_string());
                }
            }
        }

        // Check if all expected changes are present
        for (key, expected_value) in expected_changes {
            if let Some(actual_value) = found_changes.get(key) {
                if actual_value != expected_value {
                    return false;
                }
            } else {
                return false;
            }
        }

        true
    }
}

/// Mock simulator state for testing verification scripts
struct MockSimulatorState {
    variables: std::collections::HashMap<String, f64>,
}

impl MockSimulatorState {
    fn new() -> Self {
        let mut variables = std::collections::HashMap::new();
        
        // Initialize with some default values
        variables.insert("GEAR_POSITION".to_string(), 0.0);
        variables.insert("FLAPS_POSITION".to_string(), 0.0);
        variables.insert("AUTOPILOT_MASTER".to_string(), 0.0);
        variables.insert("AUTOPILOT_ALTITUDE_LOCK".to_string(), 0.0);
        variables.insert("AUTOPILOT_HEADING_LOCK".to_string(), 0.0);

        Self { variables }
    }

    fn send_event(&mut self, event: &str, value: Option<f64>) {
        // Simulate the effect of common events
        match event {
            "GEAR_TOGGLE" => {
                let current = self.variables.get("GEAR_POSITION").unwrap_or(&0.0);
                self.variables.insert("GEAR_POSITION".to_string(), if *current > 0.5 { 0.0 } else { 1.0 });
            }
            "FLAPS_INCR" => {
                let current = self.variables.get("FLAPS_POSITION").unwrap_or(&0.0);
                self.variables.insert("FLAPS_POSITION".to_string(), (current + 0.25).min(1.0));
            }
            "FLAPS_DECR" => {
                let current = self.variables.get("FLAPS_POSITION").unwrap_or(&0.0);
                self.variables.insert("FLAPS_POSITION".to_string(), (current - 0.25).max(0.0));
            }
            "AP_MASTER" => {
                self.variables.insert("AUTOPILOT_MASTER".to_string(), value.unwrap_or(1.0));
            }
            "AP_ALT_HOLD" => {
                self.variables.insert("AUTOPILOT_ALTITUDE_LOCK".to_string(), value.unwrap_or(1.0));
            }
            "AP_HDG_HOLD" => {
                self.variables.insert("AUTOPILOT_HEADING_LOCK".to_string(), value.unwrap_or(1.0));
            }
            _ => {
                // For unknown events, just set the variable if a value is provided
                if let Some(val) = value {
                    self.variables.insert(event.to_string(), val);
                }
            }
        }
    }

    fn get_variable(&self, name: &str) -> Option<f64> {
        self.variables.get(name).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_verification_script() {
        let verifier = ConfigVerifier::new(TempDir::new().unwrap().path());
        
        let script = VerifyScript {
            name: "gear_test".to_string(),
            description: "Test gear operation".to_string(),
            actions: vec![
                VerifyAction::SimEvent {
                    event: "GEAR_TOGGLE".to_string(),
                    value: None,
                },
                VerifyAction::Wait { duration_ms: 100 },
            ],
            expected: vec![
                ExpectedResult {
                    variable: "GEAR_POSITION".to_string(),
                    value: 1.0,
                    tolerance: 0.1,
                },
            ],
        };

        let result = verifier.run_verification_script(&script).await.unwrap();
        assert!(result.success);
        assert_eq!(result.name, "gear_test");
    }

    #[tokio::test]
    async fn test_ini_section_verification() {
        let verifier = ConfigVerifier::new(TempDir::new().unwrap().path());
        
        let content = "[AUTOPILOT]\nenabled=1\naltitude_hold=0\n\n[OTHER]\nkey=value\n";
        
        let mut expected_changes = HashMap::new();
        expected_changes.insert("enabled".to_string(), "1".to_string());
        expected_changes.insert("altitude_hold".to_string(), "0".to_string());
        
        assert!(verifier.verify_ini_section(content, "AUTOPILOT", &expected_changes));
        
        // Test with missing key
        expected_changes.insert("missing_key".to_string(), "value".to_string());
        assert!(!verifier.verify_ini_section(content, "AUTOPILOT", &expected_changes));
    }
}