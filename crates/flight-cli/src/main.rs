// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Flight Hub CLI - Command line interface with full parity to UI functionality

#![allow(unused)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::ptr_arg)]
#![allow(clippy::field_reassign_with_default)]

#[cfg(feature = "cli")]
use clap::{Parser, Subcommand};
#[cfg(feature = "cli")]
use flight_ipc::ClientConfig;
#[cfg(feature = "cli")]
use serde_json::json;
#[cfg(feature = "cli")]
use std::process;

#[cfg(feature = "cli")]
mod client_manager;
#[cfg(feature = "cli")]
mod commands;
#[cfg(feature = "cli")]
mod output;

#[cfg(feature = "cli")]
use client_manager::ClientManager;
#[cfg(feature = "cli")]
use commands::*;
#[cfg(feature = "cli")]
use output::OutputFormat;

#[cfg(feature = "cli")]
#[derive(Parser)]
#[command(name = "flightctl")]
#[command(about = "Flight Hub command line interface")]
#[command(version = env!("CARGO_PKG_VERSION"))]
struct Cli {
    /// Output format (human-readable or JSON)
    #[arg(long, short, value_enum, default_value = "human")]
    output: OutputFormat,

    /// Verbose output
    #[arg(long, short)]
    verbose: bool,

    /// Connection timeout in milliseconds
    #[arg(long, default_value = "5000")]
    timeout: u64,

    #[command(subcommand)]
    command: Commands,
}

#[cfg(feature = "cli")]
#[derive(Subcommand)]
enum Commands {
    /// Device management commands
    Devices {
        #[command(subcommand)]
        action: DeviceAction,
    },
    /// Profile management commands
    Profile {
        #[command(subcommand)]
        action: ProfileAction,
    },
    /// Simulator configuration commands
    Sim {
        #[command(subcommand)]
        action: SimAction,
    },
    /// Panel management commands
    Panels {
        #[command(subcommand)]
        action: PanelAction,
    },
    /// Force feedback and torque commands
    Torque {
        #[command(subcommand)]
        action: TorqueAction,
    },
    /// Diagnostics and recording commands
    Diag {
        #[command(subcommand)]
        action: DiagAction,
    },
    /// System-wide metrics
    Metrics {
        #[command(subcommand)]
        action: MetricsAction,
    },
    /// DCS World integration commands
    Dcs {
        #[command(subcommand)]
        action: DcsAction,
    },
    /// X-Plane integration commands
    Xplane {
        #[command(subcommand)]
        action: XPlaneAction,
    },
    /// Ace Combat 7 integration commands
    Ac7 {
        #[command(subcommand)]
        action: Ac7Action,
    },
    /// Update channel management and update checking
    Update {
        #[command(subcommand)]
        action: UpdateAction,
    },
    /// Community cloud profile repository
    #[command(name = "cloud-profiles")]
    CloudProfiles {
        #[command(subcommand)]
        action: CloudProfilesAction,
    },
    /// VR overlay management (show/hide/notify)
    Overlay {
        #[command(subcommand)]
        action: OverlayAction,
    },
    /// Show system status and health
    Status,
    /// Show service information
    Info,
    /// Show product posture summary
    #[command(name = "--show-posture", hide = true)]
    ShowPosture,
}

#[cfg(feature = "cli")]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Initialize client manager
    let mut client_config = ClientConfig::default();
    client_config.connection_timeout_ms = cli.timeout;

    let client_manager = ClientManager::new(client_config);

    // Execute command and handle result
    let result = execute_command(&cli, &client_manager).await;

    match result {
        Ok(output) => {
            if let Some(output) = output {
                println!("{}", output);
            }
            process::exit(0);
        }
        Err(error) => {
            let error_output = match cli.output {
                OutputFormat::Json => json!({
                    "success": false,
                    "error": error.to_string(),
                    "error_code": error_to_code(&error)
                })
                .to_string(),
                OutputFormat::Human => {
                    format!("Error: {}", error)
                }
            };

            eprintln!("{}", error_output);
            process::exit(error_to_exit_code(&error));
        }
    }
}

#[cfg(not(feature = "cli"))]
fn main() {
    eprintln!("Enable `-p flight-cli --features cli` to build the flight CLI.");
}

#[cfg(feature = "cli")]
async fn execute_command(
    cli: &Cli,
    client_manager: &ClientManager,
) -> anyhow::Result<Option<String>> {
    match &cli.command {
        Commands::Devices { action } => {
            commands::devices::execute(action, cli.output, cli.verbose, client_manager).await
        }
        Commands::Profile { action } => {
            commands::profile::execute(action, cli.output, cli.verbose, client_manager).await
        }
        Commands::Sim { action } => {
            commands::sim::execute(action, cli.output, cli.verbose, client_manager).await
        }
        Commands::Panels { action } => {
            commands::panels::execute(action, cli.output, cli.verbose, client_manager).await
        }
        Commands::Torque { action } => {
            commands::torque::execute(action, cli.output, cli.verbose, client_manager).await
        }
        Commands::Diag { action } => {
            commands::diag::execute(action, cli.output, cli.verbose, client_manager).await
        }
        Commands::Metrics { action } => {
            commands::metrics::execute(action, cli.output, cli.verbose, client_manager).await
        }
        Commands::Dcs { action } => {
            commands::dcs::execute(action, cli.output, cli.verbose, client_manager).await
        }
        Commands::Xplane { action } => {
            commands::xplane::execute(action, cli.output, cli.verbose, client_manager).await
        }
        Commands::Ac7 { action } => {
            commands::ac7::execute(action, cli.output, cli.verbose, client_manager).await
        }
        Commands::Update { action } => {
            commands::update::execute(action, cli.output, cli.verbose, client_manager).await
        }
        Commands::CloudProfiles { action } => {
            commands::cloud_profiles::execute(action, cli.output, cli.verbose, client_manager).await
        }
        Commands::Overlay { action } => {
            commands::overlay::execute(action, cli.output, cli.verbose, client_manager).await
        }
        Commands::Status => {
            commands::status::execute(cli.output, cli.verbose, client_manager).await
        }
        Commands::Info => commands::info::execute(cli.output, cli.verbose, client_manager).await,
        Commands::ShowPosture => {
            commands::posture::execute(cli.output, cli.verbose, client_manager).await
        }
    }
}

#[cfg(feature = "cli")]
fn error_to_code(error: &anyhow::Error) -> &'static str {
    // Map error types to stable error codes
    if let Some(ipc_error) = error.downcast_ref::<flight_ipc::IpcError>() {
        match ipc_error {
            flight_ipc::IpcError::ConnectionFailed { .. } => "CONNECTION_FAILED",
            flight_ipc::IpcError::VersionMismatch { .. } => "VERSION_MISMATCH",
            flight_ipc::IpcError::UnsupportedFeature { .. } => "UNSUPPORTED_FEATURE",
            flight_ipc::IpcError::Transport(_) => "TRANSPORT_ERROR",
            flight_ipc::IpcError::Serialization(_) => "SERIALIZATION_ERROR",
            flight_ipc::IpcError::Grpc(_) => "GRPC_ERROR",
        }
    } else {
        "UNKNOWN_ERROR"
    }
}

#[cfg(feature = "cli")]
fn error_to_exit_code(error: &anyhow::Error) -> i32 {
    // Map error types to exit codes
    if let Some(ipc_error) = error.downcast_ref::<flight_ipc::IpcError>() {
        match ipc_error {
            flight_ipc::IpcError::ConnectionFailed { .. } => 2,
            flight_ipc::IpcError::VersionMismatch { .. } => 3,
            flight_ipc::IpcError::UnsupportedFeature { .. } => 4,
            flight_ipc::IpcError::Transport(_) => 5,
            flight_ipc::IpcError::Serialization(_) => 6,
            flight_ipc::IpcError::Grpc(_) => 7,
        }
    } else {
        1 // Generic error
    }
}
