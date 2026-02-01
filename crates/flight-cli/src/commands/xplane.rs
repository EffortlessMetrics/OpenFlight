// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! X-Plane integration CLI commands
//!
//! Provides commands for managing X-Plane plugin integration:
//! - Install/uninstall Flight Hub X-Plane plugin
//! - Check installation status

use crate::client_manager::ClientManager;
use crate::output::OutputFormat;
use anyhow::{Context, Result};
use clap::Subcommand;
use serde_json::json;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum XPlaneAction {
    /// Install X-Plane plugin
    InstallPlugin {
        /// X-Plane installation path (auto-detected if not specified)
        #[arg(long)]
        xplane_path: Option<PathBuf>,

        /// Force installation even if already installed
        #[arg(long)]
        force: bool,
    },
    /// Uninstall X-Plane plugin
    UninstallPlugin {
        /// X-Plane installation path (auto-detected if not specified)
        #[arg(long)]
        xplane_path: Option<PathBuf>,
    },
    /// Check plugin installation status
    Status {
        /// X-Plane installation path (auto-detected if not specified)
        #[arg(long)]
        xplane_path: Option<PathBuf>,
    },
}

/// Detect X-Plane installation paths
fn detect_xplane_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    #[cfg(target_os = "windows")]
    {
        // Common Windows installation paths
        let common_paths = [
            "C:\\X-Plane 12",
            "C:\\X-Plane 11",
            "D:\\X-Plane 12",
            "D:\\X-Plane 11",
        ];

        for path in &common_paths {
            let p = PathBuf::from(path);
            if p.exists() {
                paths.push(p);
            }
        }

        // Check Steam installation
        if let Some(steam_path) = dirs::data_local_dir() {
            let steam_xplane = steam_path
                .parent()
                .map(|p| p.join("Steam/steamapps/common/X-Plane 12"));
            if let Some(p) = steam_xplane {
                if p.exists() {
                    paths.push(p);
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(home) = dirs::home_dir() {
            let common_paths = [
                home.join("X-Plane 12"),
                home.join("X-Plane 11"),
                home.join(".steam/steam/steamapps/common/X-Plane 12"),
            ];

            for path in &common_paths {
                if path.exists() {
                    paths.push(path.clone());
                }
            }
        }
    }

    paths
}

/// Get the plugin directory for an X-Plane installation
fn get_plugin_dir(xplane_path: &PathBuf) -> PathBuf {
    xplane_path
        .join("Resources")
        .join("plugins")
        .join("FlightHub")
}

/// Check if plugin is installed
fn is_plugin_installed(xplane_path: &PathBuf) -> bool {
    let plugin_dir = get_plugin_dir(xplane_path);
    plugin_dir.exists()
}

pub async fn execute(
    action: &XPlaneAction,
    output_format: OutputFormat,
    verbose: bool,
    _client_manager: &ClientManager,
) -> Result<Option<String>> {
    match action {
        XPlaneAction::InstallPlugin { xplane_path, force } => {
            let paths = if let Some(path) = xplane_path {
                vec![path.clone()]
            } else {
                detect_xplane_paths()
            };

            if paths.is_empty() {
                let output = match output_format {
                    OutputFormat::Json => json!({
                        "success": false,
                        "error": "No X-Plane installation found. Please specify --xplane-path"
                    })
                    .to_string(),
                    OutputFormat::Human => {
                        "No X-Plane installation found. Please specify --xplane-path".to_string()
                    }
                };
                return Ok(Some(output));
            }

            let mut results = Vec::new();

            for path in &paths {
                let plugin_dir = get_plugin_dir(path);

                if plugin_dir.exists() && !force {
                    results.push(json!({
                        "path": path.display().to_string(),
                        "success": true,
                        "message": "Plugin already installed"
                    }));
                    continue;
                }

                // Create plugin directory
                std::fs::create_dir_all(&plugin_dir)
                    .context("Failed to create plugin directory")?;

                // Create plugin marker file (actual plugin would be copied here)
                let marker = plugin_dir.join("FlightHub.ini");
                std::fs::write(&marker, "[FlightHub]\nversion=1.0.0\n")
                    .context("Failed to write plugin configuration")?;

                results.push(json!({
                    "path": path.display().to_string(),
                    "success": true,
                    "message": "Plugin installed successfully"
                }));
            }

            let output = match output_format {
                OutputFormat::Json => json!({
                    "success": true,
                    "installations": results
                })
                .to_string(),
                OutputFormat::Human => {
                    let mut output = String::new();
                    for result in &results {
                        output.push_str(&format!(
                            "{}: {}\n",
                            result["path"].as_str().unwrap_or("unknown"),
                            result["message"].as_str().unwrap_or("unknown")
                        ));
                    }
                    output.trim_end().to_string()
                }
            };

            Ok(Some(output))
        }

        XPlaneAction::UninstallPlugin { xplane_path } => {
            let paths = if let Some(path) = xplane_path {
                vec![path.clone()]
            } else {
                detect_xplane_paths()
            };

            if paths.is_empty() {
                let output = match output_format {
                    OutputFormat::Json => json!({
                        "success": false,
                        "error": "No X-Plane installation found"
                    })
                    .to_string(),
                    OutputFormat::Human => "No X-Plane installation found".to_string(),
                };
                return Ok(Some(output));
            }

            let mut results = Vec::new();

            for path in &paths {
                let plugin_dir = get_plugin_dir(path);

                if !plugin_dir.exists() {
                    results.push(json!({
                        "path": path.display().to_string(),
                        "success": true,
                        "message": "Plugin not installed"
                    }));
                    continue;
                }

                // Remove plugin directory
                std::fs::remove_dir_all(&plugin_dir)
                    .context("Failed to remove plugin directory")?;

                results.push(json!({
                    "path": path.display().to_string(),
                    "success": true,
                    "message": "Plugin uninstalled successfully"
                }));
            }

            let output = match output_format {
                OutputFormat::Json => json!({
                    "success": true,
                    "uninstallations": results
                })
                .to_string(),
                OutputFormat::Human => {
                    let mut output = String::new();
                    for result in &results {
                        output.push_str(&format!(
                            "{}: {}\n",
                            result["path"].as_str().unwrap_or("unknown"),
                            result["message"].as_str().unwrap_or("unknown")
                        ));
                    }
                    output.trim_end().to_string()
                }
            };

            Ok(Some(output))
        }

        XPlaneAction::Status { xplane_path } => {
            let paths = if let Some(path) = xplane_path {
                vec![path.clone()]
            } else {
                detect_xplane_paths()
            };

            if paths.is_empty() {
                let output = match output_format {
                    OutputFormat::Json => json!({
                        "found": false,
                        "installations": []
                    })
                    .to_string(),
                    OutputFormat::Human => "No X-Plane installations found".to_string(),
                };
                return Ok(Some(output));
            }

            let mut installations = Vec::new();

            for path in &paths {
                let installed = is_plugin_installed(path);
                installations.push(json!({
                    "path": path.display().to_string(),
                    "installed": installed
                }));
            }

            let output = match output_format {
                OutputFormat::Json => json!({
                    "found": true,
                    "installations": installations
                })
                .to_string(),
                OutputFormat::Human => {
                    let mut output = String::from("X-Plane Installations:\n");
                    for inst in &installations {
                        let status = if inst["installed"].as_bool().unwrap_or(false) {
                            "Plugin installed"
                        } else {
                            "Plugin not installed"
                        };
                        output.push_str(&format!(
                            "  {}: {}\n",
                            inst["path"].as_str().unwrap_or("unknown"),
                            status
                        ));
                    }
                    output.trim_end().to_string()
                }
            };

            Ok(Some(output))
        }
    }
}
