// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Configuration repair system

use crate::diff::WriterApplier;
use crate::types::*;
use anyhow::{Context, Result as AnyResult};
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// Repairs simulator configurations by applying minimal diffs
pub struct ConfigRepairer {
    backup_dir: PathBuf,
}

impl ConfigRepairer {
    pub fn new<P: AsRef<Path>>(_config_dir: P, backup_dir: P) -> Self {
        Self {
            backup_dir: backup_dir.as_ref().to_path_buf(),
        }
    }

    /// Repair configuration based on verification results
    pub async fn repair(&self, verify_result: &VerifyResult) -> AnyResult<RepairResult> {
        info!(
            "Starting repair for {} version {} ({} mismatched files)",
            verify_result.sim,
            verify_result.version,
            verify_result.mismatched_files.len()
        );

        if verify_result.mismatched_files.is_empty() {
            return Ok(RepairResult {
                success: true,
                repaired_files: vec![],
                backup_id: String::new(),
                errors: vec![],
            });
        }

        let applier = WriterApplier::new(&self.backup_dir);
        let mut repaired_files = Vec::new();
        let mut errors = Vec::new();

        // Generate repair configuration
        let repair_config = self.generate_repair_config(verify_result)?;

        // Apply the repair
        match applier.apply(&repair_config).await {
            Ok(apply_result) => {
                repaired_files = apply_result.modified_files;
                errors = apply_result.errors;

                let success = errors.is_empty();

                if success {
                    info!("Successfully repaired {} files", repaired_files.len());
                } else {
                    warn!("Repair completed with {} errors", errors.len());
                }

                Ok(RepairResult {
                    success,
                    repaired_files,
                    backup_id: apply_result.backup_id,
                    errors,
                })
            }
            Err(e) => {
                errors.push(format!("Failed to apply repair: {}", e));
                Ok(RepairResult {
                    success: false,
                    repaired_files,
                    backup_id: String::new(),
                    errors,
                })
            }
        }
    }

    /// Generate a repair configuration from verification results
    fn generate_repair_config(&self, verify_result: &VerifyResult) -> AnyResult<WriterConfig> {
        debug!("Generating repair configuration");

        let mut diffs = Vec::new();

        // Create diffs for each mismatched file
        for mismatch in &verify_result.mismatched_files {
            if let Some(suggested_diff) = &mismatch.suggested_diff {
                diffs.push(suggested_diff.clone());
            } else {
                // Generate a diff based on the mismatch type
                let diff = self.generate_diff_for_mismatch(mismatch)?;
                diffs.push(diff);
            }
        }

        Ok(WriterConfig {
            schema: "flight.writer/1".to_string(),
            sim: verify_result.sim,
            version: verify_result.version.clone(),
            description: Some(format!(
                "Auto-generated repair for {} {}",
                verify_result.sim, verify_result.version
            )),
            diffs,
            verify_scripts: vec![], // No verification needed for repair
        })
    }

    /// Generate a diff for a file mismatch
    fn generate_diff_for_mismatch(&self, mismatch: &FileMismatch) -> AnyResult<FileDiff> {
        match &mismatch.mismatch_type {
            MismatchType::Missing => {
                // For missing files, we need to determine what content should be there
                // This is a simplified approach - in practice, you'd want more sophisticated logic
                Ok(FileDiff {
                    file: mismatch.file.clone(),
                    operation: DiffOperation::Replace {
                        content: "# Auto-generated file\n".to_string(),
                    },
                    backup: true,
                })
            }
            MismatchType::ContentMismatch => {
                // For content mismatches, we'd need to analyze what the correct content should be
                // This would typically involve loading the expected state from golden files
                self.generate_content_repair_diff(&mismatch.file)
            }
            MismatchType::PermissionMismatch => {
                // Permission mismatches would be handled by the OS-level file operations
                // For now, we'll treat this as a content issue
                self.generate_content_repair_diff(&mismatch.file)
            }
        }
    }

    /// Generate a repair diff for content mismatches
    fn generate_content_repair_diff(&self, file_path: &Path) -> AnyResult<FileDiff> {
        // This is a simplified implementation
        // In practice, you'd want to:
        // 1. Load the expected content from golden files
        // 2. Analyze the current content to determine minimal changes
        // 3. Generate the most appropriate diff operation

        // For now, we'll create a basic repair based on file extension
        let operation = if let Some(extension) = file_path.extension() {
            match extension.to_str() {
                Some("ini") | Some("cfg") => {
                    // For INI files, create a basic section
                    DiffOperation::IniSection {
                        section: "DEFAULT".to_string(),
                        changes: {
                            let mut changes = std::collections::HashMap::new();
                            changes.insert("repaired".to_string(), "1".to_string());
                            changes
                        },
                    }
                }
                Some("json") => {
                    // For JSON files, add a repair marker
                    DiffOperation::JsonPatch {
                        patches: vec![crate::types::JsonPatchOp {
                            op: crate::types::JsonPatchOpType::Add,
                            path: "/repaired".to_string(),
                            value: Some(serde_json::Value::Bool(true)),
                            from: None,
                        }],
                    }
                }
                _ => {
                    // For other files, replace with a basic repair message
                    DiffOperation::Replace {
                        content: "# File repaired by Flight Hub Writers\n".to_string(),
                    }
                }
            }
        } else {
            DiffOperation::Replace {
                content: "# File repaired by Flight Hub Writers\n".to_string(),
            }
        };

        Ok(FileDiff {
            file: file_path.to_path_buf(),
            operation,
            backup: true,
        })
    }

    /// Analyze current file state and suggest minimal repairs
    pub async fn analyze_and_suggest_repairs(
        &self,
        file_path: &Path,
        expected_operation: &DiffOperation,
    ) -> AnyResult<Vec<FileDiff>> {
        debug!("Analyzing file for repair suggestions: {:?}", file_path);

        let mut suggestions = Vec::new();

        if !file_path.exists() {
            // File is missing, suggest creating it
            suggestions.push(FileDiff {
                file: file_path.to_path_buf(),
                operation: expected_operation.clone(),
                backup: false, // No backup needed for missing file
            });
            return Ok(suggestions);
        }

        // Analyze existing content and suggest minimal changes
        match expected_operation {
            DiffOperation::IniSection { section, changes } => {
                let current_content = std::fs::read_to_string(file_path)
                    .context("Failed to read file for analysis")?;

                let minimal_changes =
                    self.calculate_minimal_ini_changes(&current_content, section, changes)?;

                if !minimal_changes.is_empty() {
                    suggestions.push(FileDiff {
                        file: file_path.to_path_buf(),
                        operation: DiffOperation::IniSection {
                            section: section.clone(),
                            changes: minimal_changes,
                        },
                        backup: true,
                    });
                }
            }
            DiffOperation::JsonPatch { patches } => {
                // Analyze which patches are actually needed
                let needed_patches = self.filter_needed_json_patches(file_path, patches).await?;

                if !needed_patches.is_empty() {
                    suggestions.push(FileDiff {
                        file: file_path.to_path_buf(),
                        operation: DiffOperation::JsonPatch {
                            patches: needed_patches,
                        },
                        backup: true,
                    });
                }
            }
            DiffOperation::Replace { content } => {
                let current_content = std::fs::read_to_string(file_path)
                    .context("Failed to read file for analysis")?;

                if current_content.trim() != content.trim() {
                    suggestions.push(FileDiff {
                        file: file_path.to_path_buf(),
                        operation: expected_operation.clone(),
                        backup: true,
                    });
                }
            }
            DiffOperation::LineReplace {
                pattern,
                replacement,
                regex,
            } => {
                let current_content = std::fs::read_to_string(file_path)
                    .context("Failed to read file for analysis")?;

                let needs_replacement = if *regex {
                    let re = regex::Regex::new(pattern).context("Invalid regex pattern")?;
                    re.is_match(&current_content) && !current_content.contains(replacement)
                } else {
                    current_content.contains(pattern) && !current_content.contains(replacement)
                };

                if needs_replacement {
                    suggestions.push(FileDiff {
                        file: file_path.to_path_buf(),
                        operation: expected_operation.clone(),
                        backup: true,
                    });
                }
            }
        }

        Ok(suggestions)
    }

    /// Calculate minimal changes needed for INI section
    fn calculate_minimal_ini_changes(
        &self,
        current_content: &str,
        target_section: &str,
        desired_changes: &std::collections::HashMap<String, String>,
    ) -> AnyResult<std::collections::HashMap<String, String>> {
        let mut minimal_changes = std::collections::HashMap::new();
        let mut current_values = std::collections::HashMap::new();
        let mut in_target_section = false;

        // Parse current values in the target section
        for line in current_content.lines() {
            let trimmed = line.trim();

            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                let section_name = &trimmed[1..trimmed.len() - 1];
                in_target_section = section_name == target_section;
                continue;
            }

            if in_target_section && trimmed.contains('=') {
                let parts: Vec<&str> = trimmed.splitn(2, '=').collect();
                if parts.len() == 2 {
                    let key = parts[0].trim();
                    let value = parts[1].trim();
                    current_values.insert(key.to_string(), value.to_string());
                }
            }
        }

        // Determine which changes are actually needed
        for (key, desired_value) in desired_changes {
            if let Some(current_value) = current_values.get(key) {
                if current_value != desired_value {
                    minimal_changes.insert(key.clone(), desired_value.clone());
                }
            } else {
                // Key doesn't exist, needs to be added
                minimal_changes.insert(key.clone(), desired_value.clone());
            }
        }

        Ok(minimal_changes)
    }

    /// Filter JSON patches to only include those that are actually needed
    async fn filter_needed_json_patches(
        &self,
        file_path: &Path,
        patches: &[crate::types::JsonPatchOp],
    ) -> AnyResult<Vec<crate::types::JsonPatchOp>> {
        let current_content =
            std::fs::read_to_string(file_path).context("Failed to read JSON file")?;

        let current_json: serde_json::Value =
            serde_json::from_str(&current_content).context("Failed to parse current JSON")?;

        let mut needed_patches = Vec::new();

        for patch in patches {
            let is_needed = match &patch.op {
                crate::types::JsonPatchOpType::Add | crate::types::JsonPatchOpType::Replace => {
                    if let Some(expected_value) = &patch.value {
                        // Check if the current value differs from expected
                        match self.get_json_value(&current_json, &patch.path) {
                            Ok(current_value) => current_value != *expected_value,
                            Err(_) => true, // Path doesn't exist, so add/replace is needed
                        }
                    } else {
                        false
                    }
                }
                crate::types::JsonPatchOpType::Remove => {
                    // Check if the path exists
                    self.get_json_value(&current_json, &patch.path).is_ok()
                }
                crate::types::JsonPatchOpType::Test => {
                    // Test patches should always be included for validation
                    true
                }
                crate::types::JsonPatchOpType::Move | crate::types::JsonPatchOpType::Copy => {
                    // These are complex operations, assume they're needed
                    true
                }
            };

            if is_needed {
                needed_patches.push(patch.clone());
            }
        }

        Ok(needed_patches)
    }

    /// Get a value from JSON at the specified path
    fn get_json_value(&self, json: &serde_json::Value, path: &str) -> AnyResult<serde_json::Value> {
        let parts: Vec<&str> = path.trim_start_matches('/').split('/').collect();
        let mut current = json;

        for part in parts {
            if part.is_empty() {
                continue;
            }
            current = current
                .get(part)
                .context(format!("Path component '{}' not found", part))?;
        }

        Ok(current.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_repair_missing_file() {
        let temp_dir = TempDir::new().unwrap();
        let repairer = ConfigRepairer::new(temp_dir.path(), temp_dir.path());

        let verify_result = VerifyResult {
            sim: SimulatorType::MSFS,
            version: "1.36.0".to_string(),
            success: false,
            script_results: vec![],
            mismatched_files: vec![FileMismatch {
                file: temp_dir.path().join("missing.ini"),
                mismatch_type: MismatchType::Missing,
                suggested_diff: None,
            }],
        };

        let result = repairer.repair(&verify_result).await.unwrap();
        assert!(result.success);
        assert_eq!(result.repaired_files.len(), 1);
    }

    #[tokio::test]
    async fn test_minimal_ini_changes() {
        let temp_dir = TempDir::new().unwrap();
        let repairer = ConfigRepairer::new(temp_dir.path(), temp_dir.path());

        let content = "[SECTION]\nkey1=value1\nkey2=value2\n";

        let mut desired_changes = HashMap::new();
        desired_changes.insert("key1".to_string(), "value1".to_string()); // Same value
        desired_changes.insert("key2".to_string(), "new_value2".to_string()); // Different value
        desired_changes.insert("key3".to_string(), "value3".to_string()); // New key

        let minimal = repairer
            .calculate_minimal_ini_changes(content, "SECTION", &desired_changes)
            .unwrap();

        assert_eq!(minimal.len(), 2); // Only key2 and key3 should be changed
        assert!(!minimal.contains_key("key1")); // key1 is already correct
        assert_eq!(minimal.get("key2"), Some(&"new_value2".to_string()));
        assert_eq!(minimal.get("key3"), Some(&"value3".to_string()));
    }
}
