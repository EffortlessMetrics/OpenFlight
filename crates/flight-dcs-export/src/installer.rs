// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! DCS Export installation utilities
//!
//! Provides user-friendly installation and validation tools for DCS Export.lua integration.

use crate::export_lua::{ExportLuaConfig, ExportLuaGenerator};
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::info;

/// Installation status
#[derive(Debug, Clone, PartialEq)]
pub enum InstallStatus {
    /// Not installed
    NotInstalled,
    /// Installed and up to date
    Installed,
    /// Installed but outdated
    Outdated { current_version: String, latest_version: String },
    /// Installation exists but is corrupted/invalid
    Corrupted { reason: String },
    /// Export.lua exists but is not Flight Hub's
    Conflict { existing_content: String },
}

/// Installation result
#[derive(Debug)]
pub struct InstallResult {
    pub status: InstallStatus,
    pub path: PathBuf,
    pub backup_path: Option<PathBuf>,
    pub message: String,
}

/// DCS Export installer
pub struct DcsInstaller {
    config: ExportLuaConfig,
    generator: ExportLuaGenerator,
}

impl DcsInstaller {
    /// Create new installer with configuration
    pub fn new(config: ExportLuaConfig) -> Self {
        let generator = ExportLuaGenerator::new(config.clone());
        Self { config, generator }
    }

    /// Check current installation status
    pub fn check_status(&self) -> Result<InstallStatus> {
        let export_path = ExportLuaGenerator::get_export_lua_path()?;
        
        if !export_path.exists() {
            return Ok(InstallStatus::NotInstalled);
        }

        let content = fs::read_to_string(&export_path)
            .context("Failed to read existing Export.lua")?;

        // Check if it's a Flight Hub export
        if !content.contains("Flight Hub DCS Export Script") {
            return Ok(InstallStatus::Conflict {
                existing_content: content.lines().take(10).collect::<Vec<_>>().join("\n"),
            });
        }

        // Check version
        if let Some(version_line) = content.lines().find(|line| line.contains("Version:")) {
            let current_version = version_line
                .split("Version:")
                .nth(1)
                .unwrap_or("unknown")
                .trim()
                .to_string();
            
            if current_version != "1.0" {
                return Ok(InstallStatus::Outdated {
                    current_version,
                    latest_version: "1.0".to_string(),
                });
            }
        }

        // Validate content integrity
        if let Err(reason) = self.validate_export_content(&content) {
            return Ok(InstallStatus::Corrupted { reason });
        }

        Ok(InstallStatus::Installed)
    }

    /// Install or update Export.lua
    pub fn install(&self, force: bool) -> Result<InstallResult> {
        let export_path = ExportLuaGenerator::get_export_lua_path()?;
        let status = self.check_status()?;

        match status {
            InstallStatus::NotInstalled => {
                self.perform_fresh_install(&export_path)
            }
            InstallStatus::Installed if !force => {
                Ok(InstallResult {
                    status,
                    path: export_path,
                    backup_path: None,
                    message: "Flight Hub DCS Export is already installed and up to date.".to_string(),
                })
            }
            InstallStatus::Outdated { .. } | InstallStatus::Corrupted { .. } | InstallStatus::Installed => {
                self.perform_update(&export_path, force)
            }
            InstallStatus::Conflict { .. } => {
                self.handle_conflict(&export_path)
            }
        }
    }

    /// Uninstall Export.lua
    pub fn uninstall(&self) -> Result<InstallResult> {
        let export_path = ExportLuaGenerator::get_export_lua_path()?;
        let status = self.check_status()?;

        match status {
            InstallStatus::NotInstalled => {
                Ok(InstallResult {
                    status,
                    path: export_path,
                    backup_path: None,
                    message: "Flight Hub DCS Export is not installed.".to_string(),
                })
            }
            InstallStatus::Conflict { .. } => {
                Ok(InstallResult {
                    status,
                    path: export_path,
                    backup_path: None,
                    message: "Export.lua exists but is not Flight Hub's. Manual removal required.".to_string(),
                })
            }
            _ => {
                // Create backup before removal
                let backup_path = self.create_backup(&export_path)?;
                
                fs::remove_file(&export_path)
                    .context("Failed to remove Export.lua")?;

                info!("Removed Flight Hub DCS Export from {}", export_path.display());

                Ok(InstallResult {
                    status: InstallStatus::NotInstalled,
                    path: export_path,
                    backup_path: Some(backup_path),
                    message: "Flight Hub DCS Export has been removed. Backup created.".to_string(),
                })
            }
        }
    }

    /// Validate DCS installation
    pub fn validate_dcs_installation(&self) -> Result<Vec<String>> {
        let mut issues = Vec::new();

        // Check if DCS directory exists
        let dcs_path = ExportLuaGenerator::get_dcs_saved_games_path()?;
        if !dcs_path.exists() {
            issues.push(format!("DCS Saved Games directory not found: {}", dcs_path.display()));
            return Ok(issues);
        }

        // Check Scripts directory
        let scripts_path = dcs_path.join("Scripts");
        if !scripts_path.exists() {
            issues.push("Scripts directory does not exist. It will be created during installation.".to_string());
        }

        // Check write permissions
        if let Err(e) = self.test_write_permissions(&scripts_path) {
            issues.push(format!("Cannot write to Scripts directory: {}", e));
        }

        // Check for common DCS installation issues
        self.check_dcs_common_issues(&dcs_path, &mut issues);

        Ok(issues)
    }

    /// Generate installation report
    pub fn generate_report(&self) -> Result<String> {
        let status = self.check_status()?;
        let validation_issues = self.validate_dcs_installation()?;
        let export_path = ExportLuaGenerator::get_export_lua_path()?;

        let mut report = String::new();
        report.push_str("# Flight Hub DCS Export Installation Report\n\n");

        // Installation status
        report.push_str("## Installation Status\n");
        match status {
            InstallStatus::NotInstalled => {
                report.push_str("❌ **Not Installed**\n");
                report.push_str("Flight Hub DCS Export is not installed.\n\n");
            }
            InstallStatus::Installed => {
                report.push_str("✅ **Installed**\n");
                report.push_str("Flight Hub DCS Export is installed and up to date.\n\n");
            }
            InstallStatus::Outdated { ref current_version, ref latest_version } => {
                report.push_str("⚠️ **Outdated**\n");
                report.push_str(&format!("Current version: {}\n", current_version));
                report.push_str(&format!("Latest version: {}\n\n", latest_version));
            }
            InstallStatus::Corrupted { ref reason } => {
                report.push_str("❌ **Corrupted**\n");
                report.push_str(&format!("Reason: {}\n\n", reason));
            }
            InstallStatus::Conflict { .. } => {
                report.push_str("⚠️ **Conflict**\n");
                report.push_str("Export.lua exists but is not Flight Hub's.\n\n");
            }
        }

        // Installation path
        report.push_str("## Installation Path\n");
        report.push_str(&format!("```\n{}\n```\n\n", export_path.display()));

        // Validation issues
        if !validation_issues.is_empty() {
            report.push_str("## Validation Issues\n");
            for issue in &validation_issues {
                report.push_str(&format!("- {}\n", issue));
            }
            report.push('\n');
        } else {
            report.push_str("## Validation\n");
            report.push_str("✅ No issues found.\n\n");
        }

        // Configuration
        report.push_str("## Configuration\n");
        report.push_str(&format!("- Socket Address: {}:{}\n", self.config.socket_address, self.config.socket_port));
        report.push_str(&format!("- Update Interval: {:.1}s\n", self.config.update_interval));
        report.push_str(&format!("- MP Safe Mode: {}\n", self.config.mp_safe_mode));
        report.push_str(&format!("- Enabled Features: {}\n\n", self.config.enabled_features.join(", ")));

        // Next steps
        report.push_str("## Next Steps\n");
        match status {
            InstallStatus::NotInstalled => {
                report.push_str("1. Run installation command\n");
                report.push_str("2. Restart DCS World\n");
                report.push_str("3. Verify connection in Flight Hub\n");
            }
            InstallStatus::Installed => {
                report.push_str("1. Start DCS World\n");
                report.push_str("2. Load any mission\n");
                report.push_str("3. Check Flight Hub connection status\n");
            }
            _ => {
                report.push_str("1. Update or reinstall Export.lua\n");
                report.push_str("2. Restart DCS World\n");
                report.push_str("3. Verify connection in Flight Hub\n");
            }
        }

        Ok(report)
    }

    /// Perform fresh installation
    fn perform_fresh_install(&self, export_path: &Path) -> Result<InstallResult> {
        // Ensure Scripts directory exists
        if let Some(parent) = export_path.parent() {
            fs::create_dir_all(parent)
                .context("Failed to create Scripts directory")?;
        }

        // Generate and write Export.lua
        self.generator.write_script(export_path)
            .context("Failed to write Export.lua")?;

        info!("Installed Flight Hub DCS Export to {}", export_path.display());

        Ok(InstallResult {
            status: InstallStatus::Installed,
            path: export_path.to_path_buf(),
            backup_path: None,
            message: "Flight Hub DCS Export has been installed successfully.".to_string(),
        })
    }

    /// Perform update installation
    fn perform_update(&self, export_path: &Path, force: bool) -> Result<InstallResult> {
        // Create backup
        let backup_path = self.create_backup(export_path)?;

        // Write new version
        self.generator.write_script(export_path)
            .context("Failed to update Export.lua")?;

        let message = if force {
            "Flight Hub DCS Export has been forcibly reinstalled."
        } else {
            "Flight Hub DCS Export has been updated."
        };

        info!("Updated Flight Hub DCS Export at {}", export_path.display());

        Ok(InstallResult {
            status: InstallStatus::Installed,
            path: export_path.to_path_buf(),
            backup_path: Some(backup_path),
            message: message.to_string(),
        })
    }

    /// Handle conflicting Export.lua
    fn handle_conflict(&self, export_path: &Path) -> Result<InstallResult> {
        // For conflicts, we don't automatically overwrite
        // User must manually resolve or use force flag
        
        Ok(InstallResult {
            status: InstallStatus::Conflict {
                existing_content: "Existing Export.lua found".to_string(),
            },
            path: export_path.to_path_buf(),
            backup_path: None,
            message: format!(
                "Export.lua already exists at {}. Use --force to overwrite or manually merge the files.",
                export_path.display()
            ),
        })
    }

    /// Create backup of existing Export.lua
    fn create_backup(&self, export_path: &Path) -> Result<PathBuf> {
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let backup_path = export_path.with_extension(format!("lua.backup.{}", timestamp));
        
        fs::copy(export_path, &backup_path)
            .context("Failed to create backup")?;

        info!("Created backup at {}", backup_path.display());
        Ok(backup_path)
    }

    /// Validate Export.lua content
    fn validate_export_content(&self, content: &str) -> Result<(), String> {
        // Check for required components
        let required_components = [
            "FlightHubExport",
            "socket_address",
            "socket_port", 
            "LuaExportStart",
            "LuaExportBeforeNextFrame",
            "DCS.setUserCallbacks",
        ];

        for component in &required_components {
            if !content.contains(component) {
                return Err(format!("Missing required component: {}", component));
            }
        }

        // Check for syntax issues (basic)
        if content.matches('{').count() != content.matches('}').count() {
            return Err("Mismatched braces in Lua script".to_string());
        }

        Ok(())
    }

    /// Test write permissions
    fn test_write_permissions(&self, scripts_path: &Path) -> Result<()> {
        let test_file = scripts_path.join(".flight_hub_test");
        
        // Create directory if it doesn't exist
        fs::create_dir_all(scripts_path)
            .context("Cannot create Scripts directory")?;

        // Test write
        fs::write(&test_file, "test")
            .context("Cannot write to Scripts directory")?;

        // Clean up
        let _ = fs::remove_file(&test_file);

        Ok(())
    }

    /// Check for common DCS installation issues
    fn check_dcs_common_issues(&self, dcs_path: &Path, issues: &mut Vec<String>) {
        // Check for multiple DCS installations
        let home_dir = dirs::home_dir().unwrap_or_default();
        let possible_paths = [
            home_dir.join("Saved Games/DCS"),
            home_dir.join("Saved Games/DCS.openbeta"),
            home_dir.join("Documents/DCS"),
            home_dir.join("Documents/DCS.openbeta"),
        ];

        let existing_paths: Vec<_> = possible_paths
            .iter()
            .filter(|p| p.exists() && *p != dcs_path)
            .collect();

        if !existing_paths.is_empty() {
            issues.push(format!(
                "Multiple DCS installations detected. Using: {}. Others found: {}",
                dcs_path.display(),
                existing_paths.iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }

        // Check for read-only Scripts directory
        let scripts_path = dcs_path.join("Scripts");
        if scripts_path.exists()
            && let Ok(metadata) = scripts_path.metadata()
            && metadata.permissions().readonly() {
            issues.push("Scripts directory is read-only. Installation may fail.".to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_installer() -> (DcsInstaller, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let config = ExportLuaConfig {
            socket_address: "127.0.0.1".to_string(),
            socket_port: 7778,
            update_interval: 0.1,
            enabled_features: vec!["telemetry_basic".to_string()],
            mp_safe_mode: true,
        };
        
        (DcsInstaller::new(config), temp_dir)
    }

    #[test]
    fn test_installer_creation() {
        let (installer, _temp) = create_test_installer();
        assert_eq!(installer.config.socket_port, 7778);
    }

    #[test]
    fn test_content_validation() {
        let (installer, _temp) = create_test_installer();
        
        // Valid content
        let valid_content = r#"
            local FlightHubExport = {}
            FlightHubExport.config = {
                socket_address = "127.0.0.1",
                socket_port = 7778
            }
            function LuaExportStart() end
            function LuaExportBeforeNextFrame() end
            DCS.setUserCallbacks({})
        "#;
        
        assert!(installer.validate_export_content(valid_content).is_ok());
        
        // Invalid content (missing component)
        let invalid_content = "local test = {}";
        assert!(installer.validate_export_content(invalid_content).is_err());
    }

    #[test]
    fn test_report_generation() {
        let (installer, _temp) = create_test_installer();
        let report = installer.generate_report().unwrap();
        
        assert!(report.contains("Flight Hub DCS Export Installation Report"));
        assert!(report.contains("Installation Status"));
        assert!(report.contains("Configuration"));
    }
}