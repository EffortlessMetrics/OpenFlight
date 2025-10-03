// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Integration documentation system for "What we touch" per simulator

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Integration documentation for a specific simulator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimIntegrationDocs {
    /// Simulator name and version
    pub simulator: SimulatorInfo,
    /// Files that are modified or created
    pub files: Vec<FileIntegration>,
    /// Registry keys modified (Windows only)
    pub registry_keys: Vec<RegistryIntegration>,
    /// Network ports used
    pub network_ports: Vec<PortIntegration>,
    /// Environment variables used
    pub environment_vars: Vec<EnvVarIntegration>,
    /// Processes that may be started
    pub processes: Vec<ProcessIntegration>,
    /// Revert instructions
    pub revert_instructions: RevertInstructions,
    /// Multiplayer integrity notes
    pub multiplayer_notes: Option<MultiplayerNotes>,
}

/// Simulator information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulatorInfo {
    /// Simulator name (MSFS, X-Plane, DCS)
    pub name: String,
    /// Supported versions
    pub supported_versions: Vec<String>,
    /// Integration method
    pub integration_method: String,
    /// Last updated timestamp
    pub last_updated: u64,
}

/// File integration details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileIntegration {
    /// File path (relative to sim installation or user directory)
    pub path: String,
    /// What we do with this file
    pub action: FileAction,
    /// Purpose of this integration
    pub purpose: String,
    /// Whether this affects multiplayer
    pub affects_multiplayer: bool,
    /// Backup location if we modify existing files
    pub backup_location: Option<String>,
}

/// File action types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileAction {
    /// Create new file
    Create {
        /// File content description
        content_description: String,
    },
    /// Modify existing file
    Modify {
        /// What sections we modify
        sections_modified: Vec<String>,
        /// Type of modification
        modification_type: String,
    },
    /// Read existing file
    Read {
        /// What data we read
        data_read: String,
    },
    /// Monitor file for changes
    Monitor {
        /// What changes we watch for
        monitored_changes: String,
    },
}

/// Registry integration (Windows)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryIntegration {
    /// Registry key path
    pub key_path: String,
    /// Value name
    pub value_name: String,
    /// What we do with this key
    pub action: RegistryAction,
    /// Purpose
    pub purpose: String,
}

/// Registry action types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RegistryAction {
    /// Read registry value
    Read,
    /// Create new registry value
    Create { value_type: String },
    /// Modify existing registry value
    Modify { backup_original: bool },
}

/// Network port integration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortIntegration {
    /// Port number
    pub port: u16,
    /// Protocol (TCP/UDP)
    pub protocol: String,
    /// Direction (inbound/outbound/both)
    pub direction: String,
    /// Purpose
    pub purpose: String,
    /// Whether this is optional
    pub optional: bool,
}

/// Environment variable integration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvVarIntegration {
    /// Variable name
    pub name: String,
    /// What we do with it
    pub action: EnvVarAction,
    /// Purpose
    pub purpose: String,
}

/// Environment variable actions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EnvVarAction {
    /// Read environment variable
    Read,
    /// Set environment variable
    Set { value: String },
    /// Modify existing variable
    Modify { modification: String },
}

/// Process integration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessIntegration {
    /// Process name/executable
    pub process_name: String,
    /// What we do with this process
    pub action: ProcessAction,
    /// Purpose
    pub purpose: String,
    /// Whether this affects multiplayer
    pub affects_multiplayer: bool,
}

/// Process actions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProcessAction {
    /// Monitor existing process
    Monitor,
    /// Start new process
    Start { command_line: String },
    /// Inject into process
    Inject { method: String },
    /// Communicate with process
    Communicate { method: String },
}

/// Revert instructions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevertInstructions {
    /// Automatic revert steps
    pub automatic_steps: Vec<RevertStep>,
    /// Manual revert steps
    pub manual_steps: Vec<ManualRevertStep>,
    /// Files to delete to completely remove integration
    pub cleanup_files: Vec<String>,
    /// Registry keys to delete
    pub cleanup_registry: Vec<String>,
}

/// Automatic revert step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevertStep {
    /// Step description
    pub description: String,
    /// Action to perform
    pub action: RevertAction,
    /// Order of execution
    pub order: u32,
}

/// Revert actions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RevertAction {
    /// Delete file
    DeleteFile { path: String },
    /// Restore file from backup
    RestoreFile { path: String, backup_path: String },
    /// Delete registry key
    DeleteRegistryKey { key_path: String },
    /// Restore registry value
    RestoreRegistryValue { key_path: String, value_name: String, backup_value: String },
    /// Stop process
    StopProcess { process_name: String },
}

/// Manual revert step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManualRevertStep {
    /// Step number
    pub step_number: u32,
    /// Description for user
    pub description: String,
    /// Detailed instructions
    pub instructions: String,
    /// Screenshots or additional help
    pub help_url: Option<String>,
}

/// Multiplayer integrity notes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiplayerNotes {
    /// Whether integration is safe in multiplayer
    pub multiplayer_safe: bool,
    /// Features that are blocked in multiplayer
    pub blocked_features: Vec<String>,
    /// Features that are safe in multiplayer
    pub safe_features: Vec<String>,
    /// Anti-cheat considerations
    pub anticheat_notes: Option<String>,
    /// Server operator notes
    pub server_notes: Option<String>,
}

/// Integration documentation manager
#[derive(Debug)]
pub struct IntegrationDocsManager {
    /// Documentation for each simulator
    docs: HashMap<String, SimIntegrationDocs>,
    /// Base directory for documentation files
    docs_dir: PathBuf,
}

impl IntegrationDocsManager {
    /// Create new documentation manager
    pub fn new<P: AsRef<std::path::Path>>(docs_dir: P) -> Self {
        Self {
            docs: HashMap::new(),
            docs_dir: docs_dir.as_ref().to_path_buf(),
        }
    }
    
    /// Load documentation for all simulators
    pub async fn load_all_docs(&mut self) -> crate::Result<()> {
        // Load MSFS documentation
        self.load_msfs_docs().await?;
        
        // Load X-Plane documentation
        self.load_xplane_docs().await?;
        
        // Load DCS documentation
        self.load_dcs_docs().await?;
        
        Ok(())
    }
    
    /// Get documentation for a specific simulator
    pub fn get_docs(&self, simulator: &str) -> Option<&SimIntegrationDocs> {
        self.docs.get(simulator)
    }
    
    /// Get all available documentation
    pub fn get_all_docs(&self) -> &HashMap<String, SimIntegrationDocs> {
        &self.docs
    }
    
    /// Validate all documentation links and references
    pub async fn validate_docs(&self) -> crate::Result<ValidationReport> {
        let mut report = ValidationReport::new();
        
        for (sim_name, docs) in &self.docs {
            // Validate file paths exist or are documented as created by us
            for file_integration in &docs.files {
                match &file_integration.action {
                    FileAction::Read { .. } | FileAction::Modify { .. } | FileAction::Monitor { .. } => {
                        // These should reference existing files in typical installations
                        // In a real implementation, we'd check common installation paths
                        report.add_info(format!("File reference in {}: {}", sim_name, file_integration.path));
                    }
                    FileAction::Create { .. } => {
                        // These are files we create, so they don't need to exist
                        report.add_info(format!("File created by us in {}: {}", sim_name, file_integration.path));
                    }
                }
            }
            
            // Validate revert instructions are complete
            if docs.revert_instructions.automatic_steps.is_empty() && 
               docs.revert_instructions.manual_steps.is_empty() {
                report.add_warning(format!("No revert instructions for {}", sim_name));
            }
            
            // Validate multiplayer notes for integrations that affect MP
            let has_mp_affecting_files = docs.files.iter().any(|f| f.affects_multiplayer);
            let has_mp_affecting_processes = docs.processes.iter().any(|p| p.affects_multiplayer);
            
            if (has_mp_affecting_files || has_mp_affecting_processes) && docs.multiplayer_notes.is_none() {
                report.add_error(format!("Missing multiplayer notes for {} despite MP-affecting integrations", sim_name));
            }
        }
        
        Ok(report)
    }
    
    /// Link to external markdown documentation
    pub fn get_markdown_doc_path(&self, simulator: &str) -> Option<PathBuf> {
        let markdown_path = self.docs_dir.join("integration").join(format!("{}.md", simulator));
        if markdown_path.exists() {
            Some(markdown_path)
        } else {
            None
        }
    }

    /// Open markdown documentation in system browser
    pub fn open_markdown_docs(&self, simulator: &str) -> crate::Result<()> {
        if let Some(doc_path) = self.get_markdown_doc_path(simulator) {
            let url = format!("file://{}", doc_path.canonicalize()?.display());
            
            #[cfg(target_os = "windows")]
            {
                std::process::Command::new("cmd")
                    .args(["/c", "start", &url])
                    .spawn()?;
            }
            
            #[cfg(target_os = "macos")]
            {
                std::process::Command::new("open")
                    .arg(&url)
                    .spawn()?;
            }
            
            #[cfg(target_os = "linux")]
            {
                std::process::Command::new("xdg-open")
                    .arg(&url)
                    .spawn()?;
            }
            
            Ok(())
        } else {
            Err(crate::Error::DocumentationNotFound(simulator.to_string()))
        }
    }

    /// Generate installer summary with links to detailed docs
    pub fn generate_installer_summary(&self) -> String {
        let mut summary = String::new();
        
        summary.push_str("# Flight Hub Integration Summary\n\n");
        summary.push_str("Flight Hub integrates with flight simulators using documented APIs and minimal configuration changes.\n\n");
        
        summary.push_str("## What We Touch\n\n");
        summary.push_str("| Simulator | Files Modified | Network Ports | Multiplayer Safe |\n");
        summary.push_str("|-----------|----------------|---------------|------------------|\n");
        
        for (sim_name, docs) in &self.docs {
            let file_count = docs.files.len();
            let port_count = docs.network_ports.len();
            let mp_safe = docs.multiplayer_notes
                .as_ref()
                .map(|n| if n.multiplayer_safe { "Yes" } else { "Partial" })
                .unwrap_or("Unknown");
            
            summary.push_str(&format!("| {} | {} | {} | {} |\n", 
                docs.simulator.name, file_count, port_count, mp_safe));
        }
        
        summary.push_str("\n## Installation Requirements\n\n");
        summary.push_str("- **Privileges**: User-level installation (no administrator required)\n");
        summary.push_str("- **Runtime**: No elevated privileges needed\n");
        summary.push_str("- **Network**: Local communication only (no external servers)\n\n");
        
        summary.push_str("## Detailed Documentation\n\n");
        summary.push_str("For complete details on what Flight Hub touches in each simulator:\n\n");
        
        for (sim_name, docs) in &self.docs {
            summary.push_str(&format!("- [{}](./integration/{}.md) - Complete integration details\n", 
                docs.simulator.name, sim_name));
        }
        
        summary.push_str("\n## Removal\n\n");
        summary.push_str("Flight Hub can be completely removed:\n");
        summary.push_str("1. Use the automatic removal in Flight Hub settings\n");
        summary.push_str("2. Or follow the manual steps in each simulator's documentation\n");
        summary.push_str("3. Uninstall Flight Hub normally\n\n");
        
        summary.push_str("All changes are reversible and documented.\n");
        
        summary
    }

    /// Generate user-friendly documentation
    pub fn generate_user_docs(&self, simulator: &str) -> Option<String> {
        let docs = self.get_docs(simulator)?;
        
        let mut output = String::new();
        
        output.push_str(&format!("# Flight Hub Integration with {}\n\n", docs.simulator.name));
        output.push_str(&format!("**Integration Method:** {}\n\n", docs.simulator.integration_method));
        output.push_str(&format!("**Supported Versions:** {}\n\n", docs.simulator.supported_versions.join(", ")));
        
        // Files section
        if !docs.files.is_empty() {
            output.push_str("## Files We Touch\n\n");
            for file in &docs.files {
                output.push_str(&format!("### {}\n", file.path));
                output.push_str(&format!("**Purpose:** {}\n", file.purpose));
                output.push_str(&format!("**Action:** {}\n", format_file_action(&file.action)));
                output.push_str(&format!("**Affects Multiplayer:** {}\n", if file.affects_multiplayer { "Yes" } else { "No" }));
                if let Some(backup) = &file.backup_location {
                    output.push_str(&format!("**Backup Location:** {}\n", backup));
                }
                output.push_str("\n");
            }
        }
        
        // Network ports section
        if !docs.network_ports.is_empty() {
            output.push_str("## Network Ports Used\n\n");
            for port in &docs.network_ports {
                output.push_str(&format!("- **Port {}** ({}): {} - {}\n", 
                    port.port, port.protocol, port.purpose, 
                    if port.optional { "Optional" } else { "Required" }));
            }
            output.push_str("\n");
        }
        
        // Multiplayer notes
        if let Some(mp_notes) = &docs.multiplayer_notes {
            output.push_str("## Multiplayer Compatibility\n\n");
            output.push_str(&format!("**Safe for Multiplayer:** {}\n\n", 
                if mp_notes.multiplayer_safe { "Yes" } else { "Partially" }));
            
            if !mp_notes.safe_features.is_empty() {
                output.push_str("**Safe Features:**\n");
                for feature in &mp_notes.safe_features {
                    output.push_str(&format!("- {}\n", feature));
                }
                output.push_str("\n");
            }
            
            if !mp_notes.blocked_features.is_empty() {
                output.push_str("**Blocked in Multiplayer:**\n");
                for feature in &mp_notes.blocked_features {
                    output.push_str(&format!("- {}\n", feature));
                }
                output.push_str("\n");
            }
            
            if let Some(anticheat) = &mp_notes.anticheat_notes {
                output.push_str(&format!("**Anti-cheat Notes:** {}\n\n", anticheat));
            }
        }
        
        // Revert instructions
        output.push_str("## How to Completely Remove Flight Hub Integration\n\n");
        
        if !docs.revert_instructions.automatic_steps.is_empty() {
            output.push_str("### Automatic Removal\n");
            output.push_str("Flight Hub can automatically remove its integration:\n");
            output.push_str("1. Open Flight Hub settings\n");
            output.push_str("2. Go to Integration tab\n");
            output.push_str("3. Click \"Remove Integration\" for this simulator\n\n");
        }
        
        if !docs.revert_instructions.manual_steps.is_empty() {
            output.push_str("### Manual Removal\n");
            for step in &docs.revert_instructions.manual_steps {
                output.push_str(&format!("{}. **{}**\n", step.step_number, step.description));
                output.push_str(&format!("   {}\n", step.instructions));
                if let Some(help_url) = &step.help_url {
                    output.push_str(&format!("   [Additional Help]({})\n", help_url));
                }
                output.push_str("\n");
            }
        }
        
        Some(output)
    }
    
    /// Load MSFS integration documentation
    async fn load_msfs_docs(&mut self) -> crate::Result<()> {
        let docs = SimIntegrationDocs {
            simulator: SimulatorInfo {
                name: "Microsoft Flight Simulator".to_string(),
                supported_versions: vec!["2020".to_string(), "2024".to_string()],
                integration_method: "SimConnect API + Input Events".to_string(),
                last_updated: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            },
            files: vec![
                FileIntegration {
                    path: "%APPDATA%\\Microsoft Flight Simulator\\UserCfg.opt".to_string(),
                    action: FileAction::Read {
                        data_read: "SimConnect configuration and enabled state".to_string(),
                    },
                    purpose: "Detect MSFS installation and SimConnect availability".to_string(),
                    affects_multiplayer: false,
                    backup_location: None,
                },
                FileIntegration {
                    path: "%LOCALAPPDATA%\\Packages\\Microsoft.FlightSimulator_*\\LocalState\\packages\\Official\\OneStore\\*\\SimObjects\\Airplanes\\*\\aircraft.cfg".to_string(),
                    action: FileAction::Read {
                        data_read: "Aircraft configuration for auto-profile detection".to_string(),
                    },
                    purpose: "Identify aircraft for automatic profile switching".to_string(),
                    affects_multiplayer: false,
                    backup_location: None,
                },
            ],
            registry_keys: vec![
                RegistryIntegration {
                    key_path: "HKEY_CURRENT_USER\\SOFTWARE\\Microsoft\\Microsoft Games\\Flight Simulator\\10.0".to_string(),
                    value_name: "AppPath".to_string(),
                    action: RegistryAction::Read,
                    purpose: "Locate MSFS installation directory".to_string(),
                },
            ],
            network_ports: vec![
                PortIntegration {
                    port: 500,
                    protocol: "TCP".to_string(),
                    direction: "outbound".to_string(),
                    purpose: "SimConnect communication".to_string(),
                    optional: false,
                },
            ],
            environment_vars: vec![],
            processes: vec![
                ProcessIntegration {
                    process_name: "FlightSimulator.exe".to_string(),
                    action: ProcessAction::Monitor,
                    purpose: "Detect when MSFS is running for auto-profile switching".to_string(),
                    affects_multiplayer: false,
                },
            ],
            revert_instructions: RevertInstructions {
                automatic_steps: vec![
                    RevertStep {
                        description: "Stop monitoring MSFS process".to_string(),
                        action: RevertAction::StopProcess {
                            process_name: "flight-hub-msfs-monitor".to_string(),
                        },
                        order: 1,
                    },
                ],
                manual_steps: vec![
                    ManualRevertStep {
                        step_number: 1,
                        description: "No manual steps required".to_string(),
                        instructions: "Flight Hub does not modify any MSFS files. Simply uninstalling Flight Hub removes all integration.".to_string(),
                        help_url: None,
                    },
                ],
                cleanup_files: vec![],
                cleanup_registry: vec![],
            },
            multiplayer_notes: Some(MultiplayerNotes {
                multiplayer_safe: true,
                blocked_features: vec![],
                safe_features: vec![
                    "Axis input processing".to_string(),
                    "SimConnect telemetry reading".to_string(),
                    "Input Events (modern aircraft)".to_string(),
                ],
                anticheat_notes: Some("Flight Hub uses only official SimConnect API and does not modify game files or memory.".to_string()),
                server_notes: None,
            }),
        };
        
        self.docs.insert("msfs".to_string(), docs);
        Ok(())
    }
    
    /// Load X-Plane integration documentation
    async fn load_xplane_docs(&mut self) -> crate::Result<()> {
        let docs = SimIntegrationDocs {
            simulator: SimulatorInfo {
                name: "X-Plane".to_string(),
                supported_versions: vec!["11".to_string(), "12".to_string()],
                integration_method: "UDP DataRefs + Optional Plugin".to_string(),
                last_updated: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            },
            files: vec![
                FileIntegration {
                    path: "X-Plane/Resources/plugins/flight-hub/flight_hub_xplane.xpl".to_string(),
                    action: FileAction::Create {
                        content_description: "Optional plugin for enhanced DataRef access".to_string(),
                    },
                    purpose: "Access protected DataRefs and enable write operations".to_string(),
                    affects_multiplayer: true,
                    backup_location: None,
                },
                FileIntegration {
                    path: "X-Plane/Output/preferences/X-Plane.prf".to_string(),
                    action: FileAction::Read {
                        data_read: "Network settings and plugin configuration".to_string(),
                    },
                    purpose: "Detect UDP configuration and plugin status".to_string(),
                    affects_multiplayer: false,
                    backup_location: None,
                },
            ],
            registry_keys: vec![],
            network_ports: vec![
                PortIntegration {
                    port: 49000,
                    protocol: "UDP".to_string(),
                    direction: "both".to_string(),
                    purpose: "DataRef communication".to_string(),
                    optional: false,
                },
            ],
            environment_vars: vec![],
            processes: vec![
                ProcessIntegration {
                    process_name: "X-Plane".to_string(),
                    action: ProcessAction::Monitor,
                    purpose: "Detect when X-Plane is running".to_string(),
                    affects_multiplayer: false,
                },
            ],
            revert_instructions: RevertInstructions {
                automatic_steps: vec![
                    RevertStep {
                        description: "Remove Flight Hub plugin".to_string(),
                        action: RevertAction::DeleteFile {
                            path: "X-Plane/Resources/plugins/flight-hub/flight_hub_xplane.xpl".to_string(),
                        },
                        order: 1,
                    },
                ],
                manual_steps: vec![
                    ManualRevertStep {
                        step_number: 1,
                        description: "Remove plugin directory".to_string(),
                        instructions: "Navigate to X-Plane/Resources/plugins/ and delete the 'flight-hub' folder if it exists.".to_string(),
                        help_url: Some("https://docs.flight-hub.dev/xplane-removal".to_string()),
                    },
                ],
                cleanup_files: vec![
                    "X-Plane/Resources/plugins/flight-hub/".to_string(),
                ],
                cleanup_registry: vec![],
            },
            multiplayer_notes: Some(MultiplayerNotes {
                multiplayer_safe: false,
                blocked_features: vec![
                    "Plugin-based DataRef writes".to_string(),
                    "Protected DataRef access".to_string(),
                ],
                safe_features: vec![
                    "UDP DataRef reading".to_string(),
                    "Axis input processing".to_string(),
                ],
                anticheat_notes: Some("Plugin installation may be detected by some multiplayer servers. UDP-only mode is generally safe.".to_string()),
                server_notes: Some("Server operators should be aware that the optional plugin can modify aircraft state.".to_string()),
            }),
        };
        
        self.docs.insert("xplane".to_string(), docs);
        Ok(())
    }
    
    /// Load DCS integration documentation
    async fn load_dcs_docs(&mut self) -> crate::Result<()> {
        let docs = SimIntegrationDocs {
            simulator: SimulatorInfo {
                name: "DCS World".to_string(),
                supported_versions: vec!["2.7".to_string(), "2.8".to_string(), "2.9".to_string()],
                integration_method: "User-installed Export.lua".to_string(),
                last_updated: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
            },
            files: vec![
                FileIntegration {
                    path: "%USERPROFILE%\\Saved Games\\DCS\\Scripts\\Export.lua".to_string(),
                    action: FileAction::Create {
                        content_description: "User-installed export script for telemetry".to_string(),
                    },
                    purpose: "Export flight telemetry data via UDP".to_string(),
                    affects_multiplayer: true,
                    backup_location: Some("%USERPROFILE%\\Saved Games\\DCS\\Scripts\\Export.lua.backup".to_string()),
                },
                FileIntegration {
                    path: "%USERPROFILE%\\Saved Games\\DCS\\Config\\options.lua".to_string(),
                    action: FileAction::Read {
                        data_read: "DCS configuration and multiplayer status".to_string(),
                    },
                    purpose: "Detect multiplayer mode to block restricted features".to_string(),
                    affects_multiplayer: false,
                    backup_location: None,
                },
            ],
            registry_keys: vec![
                RegistryIntegration {
                    key_path: "HKEY_CURRENT_USER\\SOFTWARE\\Eagle Dynamics\\DCS World".to_string(),
                    value_name: "Path".to_string(),
                    action: RegistryAction::Read,
                    purpose: "Locate DCS installation directory".to_string(),
                },
            ],
            network_ports: vec![
                PortIntegration {
                    port: 7778,
                    protocol: "UDP".to_string(),
                    direction: "inbound".to_string(),
                    purpose: "Receive telemetry data from Export.lua".to_string(),
                    optional: false,
                },
            ],
            environment_vars: vec![],
            processes: vec![
                ProcessIntegration {
                    process_name: "DCS.exe".to_string(),
                    action: ProcessAction::Monitor,
                    purpose: "Detect when DCS is running and multiplayer status".to_string(),
                    affects_multiplayer: false,
                },
            ],
            revert_instructions: RevertInstructions {
                automatic_steps: vec![
                    RevertStep {
                        description: "Restore original Export.lua".to_string(),
                        action: RevertAction::RestoreFile {
                            path: "%USERPROFILE%\\Saved Games\\DCS\\Scripts\\Export.lua".to_string(),
                            backup_path: "%USERPROFILE%\\Saved Games\\DCS\\Scripts\\Export.lua.backup".to_string(),
                        },
                        order: 1,
                    },
                ],
                manual_steps: vec![
                    ManualRevertStep {
                        step_number: 1,
                        description: "Remove or restore Export.lua".to_string(),
                        instructions: "Navigate to your DCS Saved Games folder (usually Documents\\DCS\\Scripts) and either delete Export.lua or restore your original version.".to_string(),
                        help_url: Some("https://docs.flight-hub.dev/dcs-removal".to_string()),
                    },
                    ManualRevertStep {
                        step_number: 2,
                        description: "Restart DCS".to_string(),
                        instructions: "Restart DCS World to ensure the export script is no longer loaded.".to_string(),
                        help_url: None,
                    },
                ],
                cleanup_files: vec![
                    "%USERPROFILE%\\Saved Games\\DCS\\Scripts\\Export.lua".to_string(),
                    "%USERPROFILE%\\Saved Games\\DCS\\Scripts\\Export.lua.backup".to_string(),
                ],
                cleanup_registry: vec![],
            },
            multiplayer_notes: Some(MultiplayerNotes {
                multiplayer_safe: false,
                blocked_features: vec![
                    "Cockpit control writes".to_string(),
                    "Aircraft state modification".to_string(),
                    "Mission data access".to_string(),
                ],
                safe_features: vec![
                    "Basic telemetry reading (airspeed, altitude, heading)".to_string(),
                    "Axis input processing".to_string(),
                ],
                anticheat_notes: Some("Export.lua is user-installed and may be restricted on some multiplayer servers. Flight Hub automatically disables blocked features in multiplayer.".to_string()),
                server_notes: Some("Server operators can control Export.lua restrictions. Flight Hub respects server policies and displays blocked features clearly.".to_string()),
            }),
        };
        
        self.docs.insert("dcs".to_string(), docs);
        Ok(())
    }
}

/// Validation report for documentation
#[derive(Debug, Default)]
pub struct ValidationReport {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub info: Vec<String>,
}

impl ValidationReport {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn add_error(&mut self, error: String) {
        self.errors.push(error);
    }
    
    pub fn add_warning(&mut self, warning: String) {
        self.warnings.push(warning);
    }
    
    pub fn add_info(&mut self, info: String) {
        self.info.push(info);
    }
    
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
}

/// Format file action for display
fn format_file_action(action: &FileAction) -> String {
    match action {
        FileAction::Create { content_description } => {
            format!("Create new file ({})", content_description)
        }
        FileAction::Modify { sections_modified, modification_type } => {
            format!("Modify existing file ({}: {})", modification_type, sections_modified.join(", "))
        }
        FileAction::Read { data_read } => {
            format!("Read existing file ({})", data_read)
        }
        FileAction::Monitor { monitored_changes } => {
            format!("Monitor file changes ({})", monitored_changes)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_integration_docs_manager() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = IntegrationDocsManager::new(temp_dir.path());
        
        // Load all documentation
        manager.load_all_docs().await.unwrap();
        
        // Verify all simulators are loaded
        assert!(manager.get_docs("msfs").is_some());
        assert!(manager.get_docs("xplane").is_some());
        assert!(manager.get_docs("dcs").is_some());
        
        // Test validation
        let report = manager.validate_docs().await.unwrap();
        println!("Validation report: {:?}", report);
        
        // Generate user documentation
        let msfs_docs = manager.generate_user_docs("msfs").unwrap();
        assert!(msfs_docs.contains("Microsoft Flight Simulator"));
        assert!(msfs_docs.contains("SimConnect"));
        
        let dcs_docs = manager.generate_user_docs("dcs").unwrap();
        assert!(dcs_docs.contains("DCS World"));
        assert!(dcs_docs.contains("Export.lua"));
    }

    #[test]
    fn test_file_action_formatting() {
        let create_action = FileAction::Create {
            content_description: "Test file".to_string(),
        };
        assert_eq!(format_file_action(&create_action), "Create new file (Test file)");
        
        let modify_action = FileAction::Modify {
            sections_modified: vec!["section1".to_string(), "section2".to_string()],
            modification_type: "append".to_string(),
        };
        assert_eq!(format_file_action(&modify_action), "Modify existing file (append: section1, section2)");
    }

    #[test]
    fn test_validation_report() {
        let mut report = ValidationReport::new();
        
        report.add_error("Test error".to_string());
        report.add_warning("Test warning".to_string());
        report.add_info("Test info".to_string());
        
        assert!(report.has_errors());
        assert_eq!(report.errors.len(), 1);
        assert_eq!(report.warnings.len(), 1);
        assert_eq!(report.info.len(), 1);
    }
}