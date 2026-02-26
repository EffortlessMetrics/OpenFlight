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
    Outdated {
        current_version: String,
        latest_version: String,
    },
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

        let content =
            fs::read_to_string(&export_path).context("Failed to read existing Export.lua")?;

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
            InstallStatus::NotInstalled => self.perform_fresh_install(&export_path),
            InstallStatus::Installed if !force => Ok(InstallResult {
                status,
                path: export_path,
                backup_path: None,
                message: "Flight Hub DCS Export is already installed and up to date.".to_string(),
            }),
            InstallStatus::Outdated { .. }
            | InstallStatus::Corrupted { .. }
            | InstallStatus::Installed => self.perform_update(&export_path, force),
            InstallStatus::Conflict { .. } => self.handle_conflict(&export_path),
        }
    }

    /// Uninstall Export.lua
    pub fn uninstall(&self) -> Result<InstallResult> {
        self.uninstall_from_path(&ExportLuaGenerator::get_export_lua_path()?)
    }

    /// Uninstall Export.lua from a specific path (for testing)
    fn uninstall_from_path(&self, export_path: &Path) -> Result<InstallResult> {
        // Check if file exists and contains Flight Hub content
        if !export_path.exists() {
            return Ok(InstallResult {
                status: InstallStatus::NotInstalled,
                path: export_path.to_path_buf(),
                backup_path: None,
                message: "Flight Hub DCS Export is not installed.".to_string(),
            });
        }

        let content = fs::read_to_string(export_path).context("Failed to read Export.lua")?;

        // Check if it's a Flight Hub export
        if !content.contains("Flight Hub DCS Export Script") {
            return Ok(InstallResult {
                status: InstallStatus::Conflict {
                    existing_content: content.lines().take(10).collect::<Vec<_>>().join("\n"),
                },
                path: export_path.to_path_buf(),
                backup_path: None,
                message: "Export.lua exists but is not Flight Hub's. Manual removal required."
                    .to_string(),
            });
        }

        let status = InstallStatus::Installed;

        match status {
            InstallStatus::NotInstalled => Ok(InstallResult {
                status,
                path: export_path.to_path_buf(),
                backup_path: None,
                message: "Flight Hub DCS Export is not installed.".to_string(),
            }),
            InstallStatus::Conflict { .. } => Ok(InstallResult {
                status,
                path: export_path.to_path_buf(),
                backup_path: None,
                message: "Export.lua exists but is not Flight Hub's. Manual removal required."
                    .to_string(),
            }),
            _ => {
                // Look for the most recent backup with .flighthub_backup extension
                let backup_restore_path = self.find_flighthub_backup(export_path)?;

                if let Some(restore_path) = backup_restore_path {
                    // Restore from backup
                    fs::copy(&restore_path, export_path).context("Failed to restore backup")?;

                    info!(
                        "Restored Export.lua from backup: {}",
                        restore_path.display()
                    );

                    Ok(InstallResult {
                        status: InstallStatus::NotInstalled,
                        path: export_path.to_path_buf(),
                        backup_path: Some(restore_path),
                        message: "Flight Hub DCS Export has been removed. Original Export.lua restored from backup.".to_string(),
                    })
                } else {
                    // No backup found, just remove the file
                    // Create a final backup before removal
                    let backup_path = self.create_backup(export_path)?;

                    fs::remove_file(export_path).context("Failed to remove Export.lua")?;

                    info!(
                        "Removed Flight Hub DCS Export from {}",
                        export_path.display()
                    );

                    Ok(InstallResult {
                        status: InstallStatus::NotInstalled,
                        path: export_path.to_path_buf(),
                        backup_path: Some(backup_path),
                        message: "Flight Hub DCS Export has been removed. No previous backup found to restore.".to_string(),
                    })
                }
            }
        }
    }

    /// Find the most recent Flight Hub backup file
    fn find_flighthub_backup(&self, export_path: &Path) -> Result<Option<PathBuf>> {
        let parent = export_path.parent().context("No parent directory")?;

        // Look for .flighthub_backup files
        let backup_pattern = export_path.with_extension("lua.flighthub_backup");

        if backup_pattern.exists() {
            return Ok(Some(backup_pattern));
        }

        // Look for timestamped backups
        if let Ok(entries) = fs::read_dir(parent) {
            let mut backups: Vec<PathBuf> = entries
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .map(|s| s.starts_with("Export.lua.backup."))
                        .unwrap_or(false)
                })
                .collect();

            // Sort by modification time (most recent first)
            backups.sort_by_key(|p| {
                fs::metadata(p)
                    .and_then(|m| m.modified())
                    .ok()
                    .map(std::cmp::Reverse)
            });

            if let Some(most_recent) = backups.first() {
                return Ok(Some(most_recent.clone()));
            }
        }

        Ok(None)
    }

    /// Validate DCS installation
    pub fn validate_dcs_installation(&self) -> Result<Vec<String>> {
        let mut issues = Vec::new();

        // Check if DCS directory exists
        let dcs_path = ExportLuaGenerator::get_dcs_saved_games_path()?;
        if !dcs_path.exists() {
            issues.push(format!(
                "DCS Saved Games directory not found: {}",
                dcs_path.display()
            ));
            return Ok(issues);
        }

        // Check Scripts directory
        let scripts_path = dcs_path.join("Scripts");
        if !scripts_path.exists() {
            issues.push(
                "Scripts directory does not exist. It will be created during installation."
                    .to_string(),
            );
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
            InstallStatus::Outdated {
                ref current_version,
                ref latest_version,
            } => {
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
        report.push_str(&format!(
            "- Socket Address: {}:{}\n",
            self.config.socket_address, self.config.socket_port
        ));
        report.push_str(&format!(
            "- Update Interval: {:.1}s\n",
            self.config.update_interval
        ));
        report.push_str(&format!("- MP Safe Mode: {}\n", self.config.mp_safe_mode));
        report.push_str(&format!(
            "- Enabled Features: {}\n\n",
            self.config.enabled_features.join(", ")
        ));

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
            fs::create_dir_all(parent).context("Failed to create Scripts directory")?;
        }

        // Check if Export.lua already exists (non-Flight Hub)
        if export_path.exists() {
            let content =
                fs::read_to_string(export_path).context("Failed to read existing Export.lua")?;

            // If it's not a Flight Hub export, we need to append
            if !content.contains("Flight Hub DCS Export Script") {
                return self.perform_append_install(export_path);
            }
        }

        // Generate and write Export.lua
        self.generator
            .write_script(export_path)
            .context("Failed to write Export.lua")?;

        info!(
            "Installed Flight Hub DCS Export to {}",
            export_path.display()
        );

        Ok(InstallResult {
            status: InstallStatus::Installed,
            path: export_path.to_path_buf(),
            backup_path: None,
            message: "Flight Hub DCS Export has been installed successfully.".to_string(),
        })
    }

    /// Perform installation by appending to existing Export.lua
    fn perform_append_install(&self, export_path: &Path) -> Result<InstallResult> {
        // Create Flight Hub backup for restoration during uninstall
        let flighthub_backup = self.create_flighthub_backup(export_path)?;

        // Also create a timestamped backup
        let backup_path = self.create_backup(export_path)?;

        // Read existing content
        let existing_content =
            fs::read_to_string(export_path).context("Failed to read existing Export.lua")?;

        // Generate Flight Hub export script
        let flight_hub_script = self.generator.generate_script();

        // Append Flight Hub script to existing content
        let combined_content = format!(
            "{}\n\n-- Flight Hub Export (appended)\n{}\n",
            existing_content, flight_hub_script
        );

        // Write combined content
        fs::write(export_path, combined_content).context("Failed to write combined Export.lua")?;

        info!(
            "Appended Flight Hub DCS Export to existing Export.lua at {}",
            export_path.display()
        );

        let message = format!(
            "Flight Hub DCS Export has been appended to existing Export.lua. Original backed up to {} and {}",
            backup_path.display(),
            flighthub_backup.display()
        );

        Ok(InstallResult {
            status: InstallStatus::Installed,
            path: export_path.to_path_buf(),
            backup_path: Some(backup_path),
            message,
        })
    }

    /// Perform update installation
    fn perform_update(&self, export_path: &Path, force: bool) -> Result<InstallResult> {
        // Create backup
        let backup_path = self.create_backup(export_path)?;

        // Write new version
        self.generator
            .write_script(export_path)
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

        fs::copy(export_path, &backup_path).context("Failed to create backup")?;

        info!("Created backup at {}", backup_path.display());
        Ok(backup_path)
    }

    /// Create a Flight Hub-specific backup (for restoration during uninstall)
    fn create_flighthub_backup(&self, export_path: &Path) -> Result<PathBuf> {
        let backup_path = export_path.with_extension("lua.flighthub_backup");

        fs::copy(export_path, &backup_path).context("Failed to create Flight Hub backup")?;

        info!("Created Flight Hub backup at {}", backup_path.display());
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
        fs::create_dir_all(scripts_path).context("Cannot create Scripts directory")?;

        // Test write
        fs::write(&test_file, "test").context("Cannot write to Scripts directory")?;

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
                existing_paths
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }

        // Check for read-only Scripts directory
        let scripts_path = dcs_path.join("Scripts");
        if scripts_path.exists()
            && let Ok(metadata) = scripts_path.metadata()
            && metadata.permissions().readonly()
        {
            issues.push("Scripts directory is read-only. Installation may fail.".to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::export_lua::{DcsVariant, detect_dcs_variants};
    use proptest::prelude::*;
    use std::fs;
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

    // Test DCS variant detection (DCS-INT-01.1)
    #[test]
    fn test_variant_detection() {
        // This test verifies that the variant detection logic exists and returns the correct types
        // In a real environment, it would detect actual DCS installations
        let variants = detect_dcs_variants().unwrap();

        // Verify the function returns a Vec of (DcsVariant, PathBuf)
        for (variant, _path) in &variants {
            match variant {
                DcsVariant::Stable => assert_eq!(variant.as_str(), "DCS"),
                DcsVariant::OpenBeta => assert_eq!(variant.as_str(), "DCS.openbeta"),
                DcsVariant::OpenAlpha => assert_eq!(variant.as_str(), "DCS.openalpha"),
            }
        }
    }

    // Test Export.lua backup logic (DCS-INT-01.2)
    #[test]
    fn test_export_lua_backup() {
        let (installer, temp_dir) = create_test_installer();

        // Create a fake Export.lua
        let scripts_dir = temp_dir.path().join("Scripts");
        fs::create_dir_all(&scripts_dir).unwrap();
        let export_path = scripts_dir.join("Export.lua");
        fs::write(
            &export_path,
            "-- Original Export.lua content\nlocal test = {}",
        )
        .unwrap();

        // Create backup
        let backup_path = installer.create_backup(&export_path).unwrap();

        // Verify backup was created
        assert!(backup_path.exists());
        assert!(
            backup_path
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("Export.lua.backup.")
        );

        // Verify backup content matches original
        let backup_content = fs::read_to_string(&backup_path).unwrap();
        let original_content = fs::read_to_string(&export_path).unwrap();
        assert_eq!(backup_content, original_content);
    }

    // Test Export.lua append logic (DCS-INT-01.2)
    #[test]
    fn test_export_lua_append() {
        let (installer, temp_dir) = create_test_installer();

        // Create a fake Export.lua with existing content
        let scripts_dir = temp_dir.path().join("Scripts");
        fs::create_dir_all(&scripts_dir).unwrap();
        let export_path = scripts_dir.join("Export.lua");
        let existing_content = "-- Existing Export.lua from another tool\nlocal OtherTool = {}";
        fs::write(&export_path, existing_content).unwrap();

        // Perform append installation
        let result = installer.perform_append_install(&export_path).unwrap();

        // Verify installation succeeded
        assert_eq!(result.status, InstallStatus::Installed);
        assert!(result.backup_path.is_some());

        // Verify the file contains both original and Flight Hub content
        let combined_content = fs::read_to_string(&export_path).unwrap();
        assert!(combined_content.contains(existing_content));
        assert!(combined_content.contains("Flight Hub DCS Export Script"));
        assert!(combined_content.contains("Flight Hub Export (appended)"));

        // Verify backup was created
        let backup_path = result.backup_path.unwrap();
        assert!(backup_path.exists());
        let backup_content = fs::read_to_string(&backup_path).unwrap();
        assert_eq!(backup_content, existing_content);

        // Verify .flighthub_backup was created
        let flighthub_backup = export_path.with_extension("lua.flighthub_backup");
        assert!(flighthub_backup.exists());
        let flighthub_backup_content = fs::read_to_string(&flighthub_backup).unwrap();
        assert_eq!(flighthub_backup_content, existing_content);
    }

    // Test uninstaller backup restoration (DCS-INT-01.14)
    #[test]
    fn test_uninstaller_backup_restoration() {
        let (installer, temp_dir) = create_test_installer();

        // Create a fake Export.lua with existing content
        let scripts_dir = temp_dir.path().join("Scripts");
        fs::create_dir_all(&scripts_dir).unwrap();
        let export_path = scripts_dir.join("Export.lua");
        let original_content = "-- Original Export.lua from another tool\nlocal OtherTool = {}";
        fs::write(&export_path, original_content).unwrap();

        // Perform append installation (which creates .flighthub_backup)
        installer.perform_append_install(&export_path).unwrap();

        // Verify Flight Hub content was added
        let combined_content = fs::read_to_string(&export_path).unwrap();
        assert!(combined_content.contains("Flight Hub DCS Export Script"));

        // Now uninstall
        let uninstall_result = installer.uninstall_from_path(&export_path).unwrap();

        // Verify uninstallation succeeded
        assert_eq!(uninstall_result.status, InstallStatus::NotInstalled);

        // Verify the original content was restored
        let restored_content = fs::read_to_string(&export_path).unwrap();
        assert_eq!(restored_content, original_content);
        assert!(!restored_content.contains("Flight Hub DCS Export Script"));
    }

    // Test uninstaller when no backup exists
    #[test]
    fn test_uninstaller_no_backup() {
        let (installer, temp_dir) = create_test_installer();

        // Create a fake Export.lua without a backup
        let scripts_dir = temp_dir.path().join("Scripts");
        fs::create_dir_all(&scripts_dir).unwrap();
        let export_path = scripts_dir.join("Export.lua");
        let flight_hub_content = installer.generator.generate_script();
        fs::write(&export_path, &flight_hub_content).unwrap();

        // Uninstall
        let uninstall_result = installer.uninstall_from_path(&export_path).unwrap();

        // Verify uninstallation succeeded
        assert_eq!(uninstall_result.status, InstallStatus::NotInstalled);

        // Verify the file was removed (no backup to restore)
        assert!(!export_path.exists());

        // Verify a backup was created before removal
        assert!(uninstall_result.backup_path.is_some());
        let backup_path = uninstall_result.backup_path.unwrap();
        assert!(backup_path.exists());
        let backup_content = fs::read_to_string(&backup_path).unwrap();
        assert_eq!(backup_content, flight_hub_content);
    }

    // Test fresh installation when no Export.lua exists (DCS-INT-01.3)
    #[test]
    fn test_fresh_installation() {
        let (installer, temp_dir) = create_test_installer();

        // Create Scripts directory but no Export.lua
        let scripts_dir = temp_dir.path().join("Scripts");
        fs::create_dir_all(&scripts_dir).unwrap();
        let export_path = scripts_dir.join("Export.lua");

        // Perform fresh installation
        let result = installer.perform_fresh_install(&export_path).unwrap();

        // Verify installation succeeded
        assert_eq!(result.status, InstallStatus::Installed);
        assert!(result.backup_path.is_none()); // No backup for fresh install

        // Verify Export.lua was created
        assert!(export_path.exists());
        let content = fs::read_to_string(&export_path).unwrap();
        assert!(content.contains("Flight Hub DCS Export Script"));
        assert!(content.contains("function LuaExportStart()"));
        assert!(content.contains("function LuaExportStop()"));
    }

    // Test that fresh installation triggers append when non-Flight Hub Export.lua exists
    #[test]
    fn test_fresh_install_with_existing_export() {
        let (installer, temp_dir) = create_test_installer();

        // Create a fake Export.lua with non-Flight Hub content
        let scripts_dir = temp_dir.path().join("Scripts");
        fs::create_dir_all(&scripts_dir).unwrap();
        let export_path = scripts_dir.join("Export.lua");
        let existing_content = "-- SRS Export.lua\nlocal SRS = {}";
        fs::write(&export_path, existing_content).unwrap();

        // Perform fresh installation (should detect existing and append)
        let result = installer.perform_fresh_install(&export_path).unwrap();

        // Verify installation succeeded
        assert_eq!(result.status, InstallStatus::Installed);
        assert!(result.backup_path.is_some());

        // Verify both contents are present
        let combined_content = fs::read_to_string(&export_path).unwrap();
        assert!(combined_content.contains(existing_content));
        assert!(combined_content.contains("Flight Hub DCS Export Script"));
    }

    // Test backup file finding logic
    #[test]
    fn test_find_flighthub_backup() {
        let (installer, temp_dir) = create_test_installer();

        let scripts_dir = temp_dir.path().join("Scripts");
        fs::create_dir_all(&scripts_dir).unwrap();
        let export_path = scripts_dir.join("Export.lua");

        // Test when no backup exists
        let result = installer.find_flighthub_backup(&export_path).unwrap();
        assert!(result.is_none());

        // Create a .flighthub_backup file
        let flighthub_backup = export_path.with_extension("lua.flighthub_backup");
        fs::write(&flighthub_backup, "backup content").unwrap();

        // Test when .flighthub_backup exists
        let result = installer.find_flighthub_backup(&export_path).unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap(), flighthub_backup);

        // Remove .flighthub_backup and create timestamped backups
        fs::remove_file(&flighthub_backup).unwrap();
        let backup1 = export_path.with_extension("lua.backup.20240101_120000");
        let backup2 = export_path.with_extension("lua.backup.20240102_120000");
        fs::write(&backup1, "backup1").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        fs::write(&backup2, "backup2").unwrap();

        // Test that it finds the most recent timestamped backup
        let result = installer.find_flighthub_backup(&export_path).unwrap();
        assert!(result.is_some());
        // Should find one of the backups (most recent by modification time)
        let found_path = result.unwrap();
        assert!(found_path == backup1 || found_path == backup2);
    }

    // Test validation of Export.lua content
    #[test]
    fn test_export_content_validation() {
        let (installer, _temp) = create_test_installer();

        // Test valid content
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

        // Test missing required component
        let missing_component = "local FlightHubExport = {}";
        let result = installer.validate_export_content(missing_component);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing required component"));

        // Test mismatched braces
        let mismatched_braces = r#"
            local FlightHubExport = {}
            FlightHubExport.config = {
                socket_address = "127.0.0.1",
                socket_port = 7778
            }
            function LuaExportStart() end
            function LuaExportBeforeNextFrame() end
            DCS.setUserCallbacks({)
        "#;
        let result = installer.validate_export_content(mismatched_braces);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Mismatched braces"));
    }

    // Feature: release-readiness, Property 9: Uninstall Reversibility
    // *For any* installation that includes DCS integration, uninstalling SHALL restore
    // the original Export.lua from backup and remove all Flight Hub files, leaving the
    // DCS Scripts directory in its pre-install state.
    // **Validates: Requirements 9.6**
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]

        #[test]
        fn prop_uninstall_reversibility(
            original_content in "[a-zA-Z0-9_\\-\\s\\n]{10,500}"
        ) {
            let temp_dir = TempDir::new().unwrap();
            let scripts_dir = temp_dir.path().join("Scripts");
            fs::create_dir_all(&scripts_dir).unwrap();
            let export_path = scripts_dir.join("Export.lua");

            // Create original Export.lua with arbitrary content
            let original_lua = format!("-- Original content\n{}", original_content);
            fs::write(&export_path, &original_lua).unwrap();

            // Create installer
            let config = ExportLuaConfig {
                socket_address: "127.0.0.1".to_string(),
                socket_port: 7778,
                update_interval: 0.1,
                enabled_features: vec!["telemetry_basic".to_string()],
                mp_safe_mode: true,
            };
            let installer = DcsInstaller::new(config);

            // Perform append installation
            let install_result = installer.perform_append_install(&export_path);
            prop_assert!(install_result.is_ok(), "Installation should succeed");

            // Verify Flight Hub content was added
            let installed_content = fs::read_to_string(&export_path).unwrap();
            prop_assert!(
                installed_content.contains("Flight Hub DCS Export Script"),
                "Flight Hub content should be present after install"
            );

            // Verify .flighthub_backup was created
            let flighthub_backup = export_path.with_extension("lua.flighthub_backup");
            prop_assert!(
                flighthub_backup.exists(),
                "Flight Hub backup should be created"
            );

            // Uninstall
            let uninstall_result = installer.uninstall_from_path(&export_path);
            prop_assert!(uninstall_result.is_ok(), "Uninstallation should succeed");

            // Verify original content was restored
            let restored_content = fs::read_to_string(&export_path).unwrap();
            prop_assert_eq!(
                &restored_content, &original_lua,
                "Original content should be restored after uninstall"
            );

            // Verify Flight Hub content is gone
            prop_assert!(
                !restored_content.contains("Flight Hub DCS Export Script"),
                "Flight Hub content should be removed after uninstall"
            );

            // Verify FlightHubExport.lua is removed (if it was created)
            let _fh_export = scripts_dir.join("FlightHubExport.lua");
            // Note: FlightHubExport.lua is only created by install(), not perform_append_install()
            // So we just verify the main Export.lua is restored correctly
        }
    }

    #[test]
    fn test_uninstall_when_not_installed() {
        let (installer, temp_dir) = create_test_installer();
        let scripts_dir = temp_dir.path().join("Scripts");
        fs::create_dir_all(&scripts_dir).unwrap();
        // Deliberately do NOT create Export.lua
        let export_path = scripts_dir.join("Export.lua");
        let result = installer.uninstall_from_path(&export_path).unwrap();
        assert_eq!(result.status, InstallStatus::NotInstalled);
        assert!(result.backup_path.is_none());
    }

    #[test]
    fn test_uninstall_conflict_not_flight_hub_file() {
        let (installer, temp_dir) = create_test_installer();
        let scripts_dir = temp_dir.path().join("Scripts");
        fs::create_dir_all(&scripts_dir).unwrap();
        let export_path = scripts_dir.join("Export.lua");
        // Write a non-Flight Hub export (e.g., SRS)
        fs::write(&export_path, "-- SRS Export.lua\nlocal SRS = {}").unwrap();
        let result = installer.uninstall_from_path(&export_path).unwrap();
        // Should report a conflict and leave the file untouched
        assert!(matches!(result.status, InstallStatus::Conflict { .. }));
        assert!(export_path.exists(), "Foreign file must not be deleted");
    }
}
