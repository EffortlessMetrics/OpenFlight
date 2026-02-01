// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! DCS World integration CLI commands
//!
//! Provides commands for managing DCS Export.lua integration:
//! - Install/uninstall Flight Hub DCS integration
//! - Backup/restore Export.lua
//! - Check installation status

use crate::client_manager::ClientManager;
use crate::output::OutputFormat;
use anyhow::Result;
use clap::Subcommand;
use flight_dcs_export::export_lua::ExportLuaConfig;
use flight_dcs_export::installer::{DcsInstaller, InstallStatus};
use serde_json::json;

#[derive(Subcommand)]
pub enum DcsAction {
    /// Install DCS Export.lua integration
    Install {
        /// Force installation even if already installed
        #[arg(long)]
        force: bool,
    },
    /// Uninstall DCS Export.lua integration
    Uninstall,
    /// Backup existing Export.lua
    BackupExport,
    /// Restore Export.lua from backup
    RestoreExport,
    /// Check installation status
    Status,
    /// Generate installation report
    Report,
}

pub async fn execute(
    action: &DcsAction,
    output_format: OutputFormat,
    verbose: bool,
    _client_manager: &ClientManager,
) -> Result<Option<String>> {
    let config = ExportLuaConfig::default();
    let installer = DcsInstaller::new(config);

    match action {
        DcsAction::Install { force } => {
            let result = installer.install(*force)?;

            let output = match output_format {
                OutputFormat::Json => json!({
                    "success": matches!(result.status, InstallStatus::Installed),
                    "status": format!("{:?}", result.status),
                    "path": result.path.display().to_string(),
                    "backup_path": result.backup_path.map(|p| p.display().to_string()),
                    "message": result.message
                })
                .to_string(),
                OutputFormat::Human => {
                    let mut output = result.message.clone();
                    if verbose {
                        output.push_str(&format!("\nPath: {}", result.path.display()));
                        if let Some(backup) = &result.backup_path {
                            output.push_str(&format!("\nBackup: {}", backup.display()));
                        }
                    }
                    output
                }
            };

            Ok(Some(output))
        }

        DcsAction::Uninstall => {
            let result = installer.uninstall()?;

            let output = match output_format {
                OutputFormat::Json => json!({
                    "success": matches!(result.status, InstallStatus::NotInstalled),
                    "status": format!("{:?}", result.status),
                    "path": result.path.display().to_string(),
                    "backup_path": result.backup_path.map(|p| p.display().to_string()),
                    "message": result.message
                })
                .to_string(),
                OutputFormat::Human => result.message.clone(),
            };

            Ok(Some(output))
        }

        DcsAction::BackupExport => {
            // Check current status and create backup if Export.lua exists
            let status = installer.check_status()?;

            let output = match status {
                InstallStatus::NotInstalled => match output_format {
                    OutputFormat::Json => json!({
                        "success": false,
                        "message": "No Export.lua found to backup"
                    })
                    .to_string(),
                    OutputFormat::Human => "No Export.lua found to backup".to_string(),
                },
                _ => {
                    // Install with force to trigger backup
                    let result = installer.install(true)?;
                    match output_format {
                        OutputFormat::Json => json!({
                            "success": result.backup_path.is_some(),
                            "backup_path": result.backup_path.map(|p| p.display().to_string()),
                            "message": "Export.lua backed up successfully"
                        })
                        .to_string(),
                        OutputFormat::Human => {
                            if let Some(backup) = result.backup_path {
                                format!("Export.lua backed up to: {}", backup.display())
                            } else {
                                "Backup created".to_string()
                            }
                        }
                    }
                }
            };

            Ok(Some(output))
        }

        DcsAction::RestoreExport => {
            let result = installer.uninstall()?;

            let output = match output_format {
                OutputFormat::Json => json!({
                    "success": result.backup_path.is_some(),
                    "restored_from": result.backup_path.map(|p| p.display().to_string()),
                    "message": result.message
                })
                .to_string(),
                OutputFormat::Human => result.message.clone(),
            };

            Ok(Some(output))
        }

        DcsAction::Status => {
            let status = installer.check_status()?;
            let validation_issues = installer.validate_dcs_installation()?;

            let output = match output_format {
                OutputFormat::Json => json!({
                    "status": format!("{:?}", status),
                    "installed": matches!(status, InstallStatus::Installed),
                    "validation_issues": validation_issues
                })
                .to_string(),
                OutputFormat::Human => {
                    let status_str = match status {
                        InstallStatus::NotInstalled => "Not installed",
                        InstallStatus::Installed => "Installed and up to date",
                        InstallStatus::Outdated {
                            ref current_version,
                            ref latest_version,
                        } => &format!(
                            "Outdated (current: {}, latest: {})",
                            current_version, latest_version
                        ),
                        InstallStatus::Corrupted { ref reason } => {
                            &format!("Corrupted: {}", reason)
                        }
                        InstallStatus::Conflict { .. } => "Conflict with existing Export.lua",
                    };

                    let mut output = format!("DCS Integration Status: {}", status_str);

                    if !validation_issues.is_empty() {
                        output.push_str("\n\nValidation Issues:");
                        for issue in &validation_issues {
                            output.push_str(&format!("\n  - {}", issue));
                        }
                    }

                    output
                }
            };

            Ok(Some(output))
        }

        DcsAction::Report => {
            let report = installer.generate_report()?;

            let output = match output_format {
                OutputFormat::Json => json!({
                    "report": report
                })
                .to_string(),
                OutputFormat::Human => report,
            };

            Ok(Some(output))
        }
    }
}
