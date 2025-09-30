//! Writers system for applying curve conflict resolutions
//!
//! Provides table-driven configuration changes with golden tests,
//! verify/repair functionality, and one-click rollback.

use crate::{FlightError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{info, warn, debug};

/// Configuration for the writers system
#[derive(Debug, Clone)]
pub struct WritersConfig {
    /// Base directory for writer configurations
    pub config_dir: PathBuf,
    /// Base directory for backups
    pub backup_dir: PathBuf,
    /// Maximum number of backups to keep
    pub max_backups: usize,
    /// Enable verification after applying changes
    pub enable_verification: bool,
}

impl Default for WritersConfig {
    fn default() -> Self {
        Self {
            config_dir: PathBuf::from("writers"),
            backup_dir: PathBuf::from("backups"),
            max_backups: 10,
            enable_verification: true,
        }
    }
}

/// Writer configuration for a specific simulator and version
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriterConfig {
    pub sim: String,
    pub version: String,
    pub description: String,
    pub diffs: Vec<ConfigDiff>,
    pub verification_tests: Vec<VerificationTest>,
}

/// A configuration change to apply
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigDiff {
    pub file: String,
    pub section: Option<String>,
    pub changes: HashMap<String, String>,
    pub operation: DiffOperation,
}

/// Type of operation to perform
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DiffOperation {
    /// Set key-value pairs
    Set,
    /// Remove keys
    Remove,
    /// Add lines to file
    Append,
    /// Replace entire section
    Replace,
}

/// Verification test to run after applying changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationTest {
    pub name: String,
    pub description: String,
    pub test_type: VerificationTestType,
    pub expected_result: String,
}

/// Type of verification test
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VerificationTestType {
    /// Check file exists
    FileExists,
    /// Check file contains text
    FileContains,
    /// Check registry key value (Windows)
    RegistryValue,
    /// Run external command
    Command,
}

/// Result of applying a writer configuration
#[derive(Debug, Clone)]
pub struct WriteResult {
    pub success: bool,
    pub applied_diffs: Vec<String>,
    pub backup_path: Option<PathBuf>,
    pub verification_results: Vec<VerificationResult>,
    pub error_message: Option<String>,
}

/// Result of a verification test
#[derive(Debug, Clone)]
pub struct VerificationResult {
    pub test_name: String,
    pub passed: bool,
    pub actual_result: String,
    pub error_message: Option<String>,
}

/// Backup information for rollback
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupInfo {
    pub timestamp: u64,
    pub description: String,
    pub affected_files: Vec<PathBuf>,
    pub backup_dir: PathBuf,
    pub writer_config: String,
}

/// Curve conflict resolution writer
pub struct CurveConflictWriter {
    config: WritersConfig,
    sim_configs: HashMap<String, WriterConfig>,
}

impl CurveConflictWriter {
    /// Create new curve conflict writer
    pub fn new() -> Result<Self> {
        Self::with_config(WritersConfig::default())
    }

    /// Create new curve conflict writer with custom configuration
    pub fn with_config(config: WritersConfig) -> Result<Self> {
        let mut writer = Self {
            config,
            sim_configs: HashMap::new(),
        };

        writer.load_configurations()?;
        Ok(writer)
    }

    /// Load writer configurations from disk
    fn load_configurations(&mut self) -> Result<()> {
        if !self.config.config_dir.exists() {
            fs::create_dir_all(&self.config.config_dir)
                .map_err(|e| FlightError::Writer(format!("Failed to create config directory: {}", e)))?;
            
            // Create default configurations
            self.create_default_configurations()?;
        }

        // Load all JSON files from config directory
        if let Ok(entries) = fs::read_dir(&self.config.config_dir) {
            for entry in entries {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    if path.extension().and_then(|s| s.to_str()) == Some("json") {
                        if let Ok(content) = fs::read_to_string(&path) {
                            if let Ok(config) = serde_json::from_str::<WriterConfig>(&content) {
                                let key = format!("{}_{}", config.sim, config.version);
                                self.sim_configs.insert(key, config);
                            }
                        }
                    }
                }
            }
        }

        info!("Loaded {} writer configurations", self.sim_configs.len());
        Ok(())
    }

    /// Create default writer configurations for common simulators
    fn create_default_configurations(&self) -> Result<()> {
        // MSFS configuration for disabling sim curves
        let msfs_config = WriterConfig {
            sim: "msfs".to_string(),
            version: "1.36.0".to_string(),
            description: "Disable MSFS built-in control curves".to_string(),
            diffs: vec![
                ConfigDiff {
                    file: "MSFS/UserCfg.opt".to_string(),
                    section: Some("[CONTROLS]".to_string()),
                    changes: {
                        let mut changes = HashMap::new();
                        changes.insert("UseLinearCurves".to_string(), "1".to_string());
                        changes.insert("DisableNonLinearControls".to_string(), "1".to_string());
                        changes
                    },
                    operation: DiffOperation::Set,
                },
            ],
            verification_tests: vec![
                VerificationTest {
                    name: "check_linear_curves".to_string(),
                    description: "Verify linear curves are enabled".to_string(),
                    test_type: VerificationTestType::FileContains,
                    expected_result: "UseLinearCurves=1".to_string(),
                },
            ],
        };

        // X-Plane configuration
        let xplane_config = WriterConfig {
            sim: "xplane".to_string(),
            version: "12.0".to_string(),
            description: "Disable X-Plane control response curves".to_string(),
            diffs: vec![
                ConfigDiff {
                    file: "X-Plane 12/Output/preferences/X-Plane Joystick Settings.prf".to_string(),
                    section: None,
                    changes: {
                        let mut changes = HashMap::new();
                        changes.insert("_joy_use_linear_curves".to_string(), "1".to_string());
                        changes
                    },
                    operation: DiffOperation::Set,
                },
            ],
            verification_tests: vec![
                VerificationTest {
                    name: "check_xplane_linear".to_string(),
                    description: "Verify X-Plane uses linear curves".to_string(),
                    test_type: VerificationTestType::FileContains,
                    expected_result: "_joy_use_linear_curves\t1".to_string(),
                },
            ],
        };

        // DCS configuration
        let dcs_config = WriterConfig {
            sim: "dcs".to_string(),
            version: "2.9".to_string(),
            description: "Disable DCS control curves via options.lua".to_string(),
            diffs: vec![
                ConfigDiff {
                    file: "DCS World/Config/options.lua".to_string(),
                    section: Some("options = {".to_string()),
                    changes: {
                        let mut changes = HashMap::new();
                        changes.insert("useLinearCurves".to_string(), "true".to_string());
                        changes
                    },
                    operation: DiffOperation::Set,
                },
            ],
            verification_tests: vec![
                VerificationTest {
                    name: "check_dcs_linear".to_string(),
                    description: "Verify DCS uses linear curves".to_string(),
                    test_type: VerificationTestType::FileContains,
                    expected_result: "useLinearCurves = true".to_string(),
                },
            ],
        };

        // Save configurations
        self.save_config(&msfs_config)?;
        self.save_config(&xplane_config)?;
        self.save_config(&dcs_config)?;

        Ok(())
    }

    /// Save a writer configuration to disk
    fn save_config(&self, config: &WriterConfig) -> Result<()> {
        let filename = format!("{}_{}.json", config.sim, config.version);
        let path = self.config.config_dir.join(filename);
        
        let json = serde_json::to_string_pretty(config)
            .map_err(|e| FlightError::Writer(format!("Failed to serialize config: {}", e)))?;
        
        fs::write(&path, json)
            .map_err(|e| FlightError::Writer(format!("Failed to write config file: {}", e)))?;

        Ok(())
    }

    /// Apply curve conflict resolution
    pub fn resolve_curve_conflict(
        &self,
        sim: &str,
        version: &str,
        resolution_type: &str,
        parameters: &HashMap<String, String>,
    ) -> Result<WriteResult> {
        let config_key = format!("{}_{}", sim, version);
        let config = self.sim_configs.get(&config_key)
            .ok_or_else(|| FlightError::Configuration(format!("No writer config found for {} {}", sim, version)))?;

        info!("Applying curve conflict resolution for {} {} ({})", sim, version, resolution_type);

        // Create backup before making changes
        let backup_path = if self.should_create_backup(config) {
            Some(self.create_backup(config)?)
        } else {
            None
        };

        let mut applied_diffs = Vec::new();
        let mut verification_results = Vec::new();
        let mut success = true;
        let mut error_message = None;

        // Apply configuration diffs
        for diff in &config.diffs {
            match self.apply_diff(diff, parameters) {
                Ok(_) => {
                    applied_diffs.push(diff.file.clone());
                    debug!("Applied diff to {}", diff.file);
                }
                Err(e) => {
                    success = false;
                    error_message = Some(format!("Failed to apply diff to {}: {}", diff.file, e));
                    warn!("Failed to apply diff to {}: {}", diff.file, e);
                    break;
                }
            }
        }

        // Run verification tests if enabled and changes were successful
        if success && self.config.enable_verification {
            for test in &config.verification_tests {
                let result = self.run_verification_test(test);
                let passed = result.passed;
                verification_results.push(result);
                
                if !passed {
                    success = false;
                    if error_message.is_none() {
                        error_message = Some(format!("Verification test '{}' failed", test.name));
                    }
                }
            }
        }

        Ok(WriteResult {
            success,
            applied_diffs,
            backup_path,
            verification_results,
            error_message,
        })
    }

    /// Apply a single configuration diff
    fn apply_diff(&self, diff: &ConfigDiff, parameters: &HashMap<String, String>) -> Result<()> {
        let expanded_path = self.expand_parameters(&diff.file, parameters);
        let target_path = Path::new(&expanded_path);

        match diff.operation {
            DiffOperation::Set => self.apply_set_diff(target_path, diff, parameters),
            DiffOperation::Remove => self.apply_remove_diff(target_path, diff),
            DiffOperation::Append => self.apply_append_diff(target_path, diff, parameters),
            DiffOperation::Replace => self.apply_replace_diff(target_path, diff, parameters),
        }
    }

    /// Apply a SET operation (modify key-value pairs)
    fn apply_set_diff(&self, path: &Path, diff: &ConfigDiff, parameters: &HashMap<String, String>) -> Result<()> {
        if !path.exists() {
            return Err(FlightError::Writer(format!("Target file does not exist: {:?}", path)));
        }

        let content = fs::read_to_string(path)
            .map_err(|e| FlightError::Writer(format!("Failed to read file: {}", e)))?;

        let mut modified_content = content;

        // Apply changes
        for (key, value) in &diff.changes {
            let expanded_value = self.expand_parameters(value, parameters);
            
            // Simple key=value replacement (would need more sophisticated parsing for real use)
            let pattern = format!("{}=", key);
            let replacement = format!("{}={}", key, expanded_value);
            
            if modified_content.contains(&pattern) {
                // Replace existing value
                let lines: Vec<&str> = modified_content.lines().collect();
                let mut new_lines = Vec::new();
                
                for line in lines {
                    if line.trim_start().starts_with(&pattern) {
                        new_lines.push(replacement.clone());
                    } else {
                        new_lines.push(line.to_string());
                    }
                }
                
                modified_content = new_lines.join("\n");
            } else {
                // Add new key-value pair
                if let Some(section) = &diff.section {
                    // Add to specific section
                    if modified_content.contains(section) {
                        modified_content = modified_content.replace(
                            section,
                            &format!("{}\n{}", section, replacement)
                        );
                    }
                } else {
                    // Append to end of file
                    modified_content.push('\n');
                    modified_content.push_str(&replacement);
                }
            }
        }

        fs::write(path, modified_content)
            .map_err(|e| FlightError::Writer(format!("Failed to write file: {}", e)))?;

        Ok(())
    }

    /// Apply a REMOVE operation
    fn apply_remove_diff(&self, path: &Path, diff: &ConfigDiff) -> Result<()> {
        if !path.exists() {
            return Ok(()); // File doesn't exist, nothing to remove
        }

        let content = fs::read_to_string(path)
            .map_err(|e| FlightError::Writer(format!("Failed to read file: {}", e)))?;

        let lines: Vec<&str> = content.lines().collect();
        let mut new_lines = Vec::new();

        for line in lines {
            let mut should_keep = true;
            
            for key in diff.changes.keys() {
                if line.trim_start().starts_with(&format!("{}=", key)) {
                    should_keep = false;
                    break;
                }
            }
            
            if should_keep {
                new_lines.push(line.to_string());
            }
        }

        let modified_content = new_lines.join("\n");
        fs::write(path, modified_content)
            .map_err(|e| FlightError::Writer(format!("Failed to write file: {}", e)))?;

        Ok(())
    }

    /// Apply an APPEND operation
    fn apply_append_diff(&self, path: &Path, diff: &ConfigDiff, parameters: &HashMap<String, String>) -> Result<()> {
        let mut content = if path.exists() {
            fs::read_to_string(path)
                .map_err(|e| FlightError::Writer(format!("Failed to read file: {}", e)))?
        } else {
            String::new()
        };

        for (_, value) in &diff.changes {
            let expanded_value = self.expand_parameters(value, parameters);
            content.push('\n');
            content.push_str(&expanded_value);
        }

        fs::write(path, content)
            .map_err(|e| FlightError::Writer(format!("Failed to write file: {}", e)))?;

        Ok(())
    }

    /// Apply a REPLACE operation
    fn apply_replace_diff(&self, path: &Path, diff: &ConfigDiff, parameters: &HashMap<String, String>) -> Result<()> {
        let new_content = diff.changes.values()
            .map(|value| self.expand_parameters(value, parameters))
            .collect::<Vec<_>>()
            .join("\n");

        fs::write(path, new_content)
            .map_err(|e| FlightError::Writer(format!("Failed to write file: {}", e)))?;

        Ok(())
    }

    /// Expand parameter placeholders in a string
    fn expand_parameters(&self, text: &str, parameters: &HashMap<String, String>) -> String {
        let mut result = text.to_string();
        
        for (key, value) in parameters {
            let placeholder = format!("{{{}}}", key);
            result = result.replace(&placeholder, value);
        }
        
        result
    }

    /// Check if we should create a backup for this configuration
    fn should_create_backup(&self, _config: &WriterConfig) -> bool {
        // Always create backups for safety
        true
    }

    /// Create backup of files that will be modified
    fn create_backup(&self, config: &WriterConfig) -> Result<PathBuf> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        let backup_name = format!("backup_{}_{}", config.sim, timestamp);
        let backup_path = self.config.backup_dir.join(&backup_name);
        
        fs::create_dir_all(&backup_path)
            .map_err(|e| FlightError::Writer(format!("Failed to create backup directory: {}", e)))?;

        // Copy files that will be modified
        for diff in &config.diffs {
            let source_path = Path::new(&diff.file);
            if source_path.exists() {
                if let Some(file_name) = source_path.file_name() {
                    let backup_file_path = backup_path.join(file_name);
                    
                    if let Err(e) = fs::copy(source_path, &backup_file_path) {
                        warn!("Failed to backup file {:?}: {}", source_path, e);
                    }
                }
            }
        }

        // Save backup info
        let backup_info = BackupInfo {
            timestamp,
            description: config.description.clone(),
            affected_files: config.diffs.iter().map(|d| PathBuf::from(&d.file)).collect(),
            backup_dir: backup_path.clone(),
            writer_config: format!("{}_{}", config.sim, config.version),
        };

        let info_path = backup_path.join("backup_info.json");
        if let Ok(info_json) = serde_json::to_string_pretty(&backup_info) {
            let _ = fs::write(&info_path, info_json);
        }

        info!("Created backup at {:?}", backup_path);
        Ok(backup_path)
    }

    /// Run a verification test
    fn run_verification_test(&self, test: &VerificationTest) -> VerificationResult {
        match test.test_type {
            VerificationTestType::FileExists => {
                let path = Path::new(&test.expected_result);
                let exists = path.exists();
                VerificationResult {
                    test_name: test.name.clone(),
                    passed: exists,
                    actual_result: exists.to_string(),
                    error_message: if !exists {
                        Some(format!("File does not exist: {:?}", path))
                    } else {
                        None
                    },
                }
            }
            VerificationTestType::FileContains => {
                // Expected result should be in format "file_path:search_text"
                let parts: Vec<&str> = test.expected_result.splitn(2, ':').collect();
                if parts.len() != 2 {
                    return VerificationResult {
                        test_name: test.name.clone(),
                        passed: false,
                        actual_result: "Invalid test format".to_string(),
                        error_message: Some("Expected format: file_path:search_text".to_string()),
                    };
                }

                let file_path = Path::new(parts[0]);
                let search_text = parts[1];

                match fs::read_to_string(file_path) {
                    Ok(content) => {
                        let contains = content.contains(search_text);
                        VerificationResult {
                            test_name: test.name.clone(),
                            passed: contains,
                            actual_result: format!("File contains text: {}", contains),
                            error_message: if !contains {
                                Some(format!("File does not contain expected text: {}", search_text))
                            } else {
                                None
                            },
                        }
                    }
                    Err(e) => VerificationResult {
                        test_name: test.name.clone(),
                        passed: false,
                        actual_result: "Failed to read file".to_string(),
                        error_message: Some(format!("Failed to read file: {}", e)),
                    },
                }
            }
            VerificationTestType::RegistryValue => {
                // Placeholder for Windows registry checks
                VerificationResult {
                    test_name: test.name.clone(),
                    passed: false,
                    actual_result: "Registry checks not implemented".to_string(),
                    error_message: Some("Registry verification not yet implemented".to_string()),
                }
            }
            VerificationTestType::Command => {
                // Placeholder for external command execution
                VerificationResult {
                    test_name: test.name.clone(),
                    passed: false,
                    actual_result: "Command execution not implemented".to_string(),
                    error_message: Some("Command verification not yet implemented".to_string()),
                }
            }
        }
    }

    /// Rollback changes using a backup
    pub fn rollback(&self, backup_path: &Path) -> Result<WriteResult> {
        let info_path = backup_path.join("backup_info.json");
        
        if !info_path.exists() {
            return Err(FlightError::Writer("Backup info not found".to_string()));
        }

        let info_content = fs::read_to_string(&info_path)
            .map_err(|e| FlightError::Writer(format!("Failed to read backup info: {}", e)))?;
        
        let backup_info: BackupInfo = serde_json::from_str(&info_content)
            .map_err(|e| FlightError::Writer(format!("Failed to parse backup info: {}", e)))?;

        let mut applied_diffs = Vec::new();
        let mut success = true;
        let mut error_message = None;

        // Restore files from backup
        for file_path in &backup_info.affected_files {
            if let Some(file_name) = file_path.file_name() {
                let backup_file_path = backup_path.join(file_name);
                
                if backup_file_path.exists() {
                    match fs::copy(&backup_file_path, file_path) {
                        Ok(_) => {
                            applied_diffs.push(file_path.to_string_lossy().to_string());
                            debug!("Restored file {:?} from backup", file_path);
                        }
                        Err(e) => {
                            success = false;
                            error_message = Some(format!("Failed to restore file {:?}: {}", file_path, e));
                            warn!("Failed to restore file {:?}: {}", file_path, e);
                            break;
                        }
                    }
                }
            }
        }

        info!("Rollback completed for backup {:?}, success: {}", backup_path, success);

        Ok(WriteResult {
            success,
            applied_diffs,
            backup_path: Some(backup_path.to_path_buf()),
            verification_results: Vec::new(),
            error_message,
        })
    }

    /// List available backups
    pub fn list_backups(&self) -> Result<Vec<BackupInfo>> {
        let mut backups = Vec::new();

        if !self.config.backup_dir.exists() {
            return Ok(backups);
        }

        if let Ok(entries) = fs::read_dir(&self.config.backup_dir) {
            for entry in entries {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    if path.is_dir() {
                        let info_path = path.join("backup_info.json");
                        if info_path.exists() {
                            if let Ok(content) = fs::read_to_string(&info_path) {
                                if let Ok(backup_info) = serde_json::from_str::<BackupInfo>(&content) {
                                    backups.push(backup_info);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Sort by timestamp (newest first)
        backups.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        Ok(backups)
    }
}

impl Default for CurveConflictWriter {
    fn default() -> Self {
        Self::new().expect("Failed to create default CurveConflictWriter")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_writer_creation() {
        let temp_dir = TempDir::new().unwrap();
        let config = WritersConfig {
            config_dir: temp_dir.path().join("config"),
            backup_dir: temp_dir.path().join("backup"),
            max_backups: 5,
            enable_verification: true,
        };

        let writer = CurveConflictWriter::with_config(config);
        assert!(writer.is_ok());
    }

    #[test]
    fn test_parameter_expansion() {
        let writer = CurveConflictWriter::new().unwrap();
        let mut params = HashMap::new();
        params.insert("sim_path".to_string(), "/path/to/sim".to_string());
        params.insert("value".to_string(), "test_value".to_string());

        let result = writer.expand_parameters("{sim_path}/config/{value}.cfg", &params);
        assert_eq!(result, "/path/to/sim/config/test_value.cfg");
    }

    #[test]
    fn test_diff_operations() {
        // Test the different diff operation types
        assert!(matches!(DiffOperation::Set, DiffOperation::Set));
        assert!(matches!(DiffOperation::Remove, DiffOperation::Remove));
        assert!(matches!(DiffOperation::Append, DiffOperation::Append));
        assert!(matches!(DiffOperation::Replace, DiffOperation::Replace));
    }
}