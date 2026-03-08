// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Flight Hub CLI - Command line interface with full parity to UI functionality

#![allow(unused)]

#[cfg(feature = "cli")]
use clap::{Parser, Subcommand};
#[cfg(feature = "cli")]
use flight_ipc::ClientConfig;
#[cfg(feature = "cli")]
use serde_json::json;
#[cfg(feature = "cli")]
use std::process;

#[cfg(feature = "cli")]
mod axis_monitor;
pub mod batch;
#[cfg(feature = "cli")]
mod client_manager;
#[cfg(feature = "cli")]
mod commands;
mod completions;
#[cfg(feature = "cli")]
mod device_list;
#[cfg(feature = "cli")]
mod output;
#[cfg(feature = "cli")]
mod profile_diff;
pub mod scripting;
#[cfg(feature = "cli")]
mod version_check;

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

    /// Shorthand for --output json
    #[arg(long)]
    json: bool,

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
    /// Simulator adapter management
    Adapters {
        #[command(subcommand)]
        action: AdaptersAction,
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
    /// Show version information with build metadata
    Version,
    /// Enter safe mode (zero FFB, passthrough axes)
    SafeMode,
    /// Run diagnostic checks (shorthand for diag health)
    Diagnostics,
    /// Show product posture summary
    #[command(name = "--show-posture", hide = true)]
    ShowPosture,
}

#[cfg(feature = "cli")]
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut cli = Cli::parse();

    // --json flag overrides --output
    if cli.json {
        cli.output = OutputFormat::Json;
    }

    // Initialize client manager
    let client_config = ClientConfig {
        connection_timeout_ms: cli.timeout,
        ..ClientConfig::default()
    };

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
        Commands::Adapters { action } => {
            commands::adapters::execute(action, cli.output, cli.verbose, client_manager).await
        }
        Commands::Overlay { action } => {
            commands::overlay::execute(action, cli.output, cli.verbose, client_manager).await
        }
        Commands::Status => {
            commands::status::execute(cli.output, cli.verbose, client_manager).await
        }
        Commands::Info => commands::info::execute(cli.output, cli.verbose, client_manager).await,
        Commands::Version => {
            commands::version::execute(cli.output, cli.verbose, client_manager).await
        }
        Commands::SafeMode => {
            commands::safe_mode::execute(cli.output, cli.verbose, client_manager).await
        }
        Commands::Diagnostics => {
            commands::diag::execute(
                &commands::DiagAction::Health,
                cli.output,
                cli.verbose,
                client_manager,
            )
            .await
        }
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

#[cfg(all(test, feature = "cli"))]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn parse_status_subcommand() {
        let cli = Cli::try_parse_from(["flightctl", "status"]).unwrap();
        assert!(matches!(cli.command, Commands::Status));
    }

    #[test]
    fn parse_info_subcommand() {
        let cli = Cli::try_parse_from(["flightctl", "info"]).unwrap();
        assert!(matches!(cli.command, Commands::Info));
    }

    #[test]
    fn parse_output_json_flag() {
        let cli = Cli::try_parse_from(["flightctl", "--output", "json", "status"]).unwrap();
        assert!(matches!(cli.output, OutputFormat::Json));
    }

    #[test]
    fn parse_output_human_is_default() {
        let cli = Cli::try_parse_from(["flightctl", "status"]).unwrap();
        assert!(matches!(cli.output, OutputFormat::Human));
    }

    #[test]
    fn parse_verbose_short_flag() {
        let cli = Cli::try_parse_from(["flightctl", "-v", "status"]).unwrap();
        assert!(cli.verbose);
    }

    #[test]
    fn parse_verbose_long_flag() {
        let cli = Cli::try_parse_from(["flightctl", "--verbose", "status"]).unwrap();
        assert!(cli.verbose);
    }

    #[test]
    fn parse_timeout_flag() {
        let cli = Cli::try_parse_from(["flightctl", "--timeout", "1000", "status"]).unwrap();
        assert_eq!(cli.timeout, 1000);
    }

    #[test]
    fn parse_default_timeout() {
        let cli = Cli::try_parse_from(["flightctl", "status"]).unwrap();
        assert_eq!(cli.timeout, 5000);
    }

    #[test]
    fn parse_invalid_subcommand_returns_error() {
        let result = Cli::try_parse_from(["flightctl", "nonexistent"]);
        assert!(result.is_err());
    }

    #[test]
    fn parse_no_subcommand_returns_error() {
        let result = Cli::try_parse_from(["flightctl"]);
        assert!(result.is_err());
    }

    #[test]
    fn parse_version_subcommand() {
        let cli = Cli::try_parse_from(["flightctl", "version"]).unwrap();
        assert!(matches!(cli.command, Commands::Version));
    }

    #[test]
    fn parse_safe_mode_subcommand() {
        let cli = Cli::try_parse_from(["flightctl", "safe-mode"]).unwrap();
        assert!(matches!(cli.command, Commands::SafeMode));
    }

    #[test]
    fn parse_diagnostics_subcommand() {
        let cli = Cli::try_parse_from(["flightctl", "diagnostics"]).unwrap();
        assert!(matches!(cli.command, Commands::Diagnostics));
    }

    #[test]
    fn parse_devices_list_subcommand() {
        let cli = Cli::try_parse_from(["flightctl", "devices", "list"]).unwrap();
        assert!(matches!(
            cli.command,
            Commands::Devices {
                action: commands::DeviceAction::List { .. }
            }
        ));
    }

    #[test]
    fn parse_devices_list_include_disconnected_flag() {
        let cli = Cli::try_parse_from(["flightctl", "devices", "list", "--include-disconnected"])
            .unwrap();
        if let Commands::Devices {
            action:
                commands::DeviceAction::List {
                    include_disconnected,
                    ..
                },
        } = cli.command
        {
            assert!(include_disconnected);
        } else {
            panic!("unexpected command variant");
        }
    }

    #[test]
    fn parse_json_flag() {
        let cli = Cli::try_parse_from(["flightctl", "--json", "status"]).unwrap();
        assert!(cli.json);
    }

    #[test]
    fn parse_json_flag_sets_output_to_json() {
        let mut cli = Cli::try_parse_from(["flightctl", "--json", "status"]).unwrap();
        if cli.json {
            cli.output = OutputFormat::Json;
        }
        assert!(matches!(cli.output, OutputFormat::Json));
    }

    #[test]
    fn parse_no_json_flag_keeps_human_default() {
        let cli = Cli::try_parse_from(["flightctl", "status"]).unwrap();
        assert!(!cli.json);
        assert!(matches!(cli.output, OutputFormat::Human));
    }

    #[test]
    fn parse_devices_calibrate_subcommand() {
        let cli = Cli::try_parse_from(["flightctl", "devices", "calibrate", "dev-1"]).unwrap();
        assert!(matches!(
            cli.command,
            Commands::Devices {
                action: commands::DeviceAction::Calibrate { .. }
            }
        ));
    }

    #[test]
    fn parse_devices_test_subcommand() {
        let cli = Cli::try_parse_from(["flightctl", "devices", "test", "dev-1"]).unwrap();
        assert!(matches!(
            cli.command,
            Commands::Devices {
                action: commands::DeviceAction::Test { .. }
            }
        ));
    }

    #[test]
    fn parse_devices_test_with_options() {
        let cli = Cli::try_parse_from([
            "flightctl",
            "devices",
            "test",
            "dev-1",
            "--interval-ms",
            "50",
            "--count",
            "10",
        ])
        .unwrap();
        if let Commands::Devices {
            action:
                commands::DeviceAction::Test {
                    device_id,
                    interval_ms,
                    count,
                },
        } = cli.command
        {
            assert_eq!(device_id, "dev-1");
            assert_eq!(interval_ms, 50);
            assert_eq!(count, Some(10));
        } else {
            panic!("unexpected command variant");
        }
    }

    #[test]
    fn parse_profile_list_subcommand() {
        let cli = Cli::try_parse_from(["flightctl", "profile", "list"]).unwrap();
        assert!(matches!(
            cli.command,
            Commands::Profile {
                action: commands::ProfileAction::List { .. }
            }
        ));
    }

    #[test]
    fn parse_profile_activate_subcommand() {
        let cli = Cli::try_parse_from(["flightctl", "profile", "activate", "combat"]).unwrap();
        if let Commands::Profile {
            action: commands::ProfileAction::Activate { name },
        } = cli.command
        {
            assert_eq!(name, "combat");
        } else {
            panic!("unexpected command variant");
        }
    }

    #[test]
    fn parse_profile_validate_subcommand() {
        let cli = Cli::try_parse_from(["flightctl", "profile", "validate", "test.json"]).unwrap();
        assert!(matches!(
            cli.command,
            Commands::Profile {
                action: commands::ProfileAction::Validate { .. }
            }
        ));
    }

    #[test]
    fn parse_profile_export_subcommand() {
        let cli = Cli::try_parse_from(["flightctl", "profile", "export", "combat", "output.json"])
            .unwrap();
        if let Commands::Profile {
            action: commands::ProfileAction::Export { name, path },
        } = cli.command
        {
            assert_eq!(name, "combat");
            assert_eq!(path, std::path::PathBuf::from("output.json"));
        } else {
            panic!("unexpected command variant");
        }
    }

    #[test]
    fn parse_adapters_status_subcommand() {
        let cli = Cli::try_parse_from(["flightctl", "adapters", "status"]).unwrap();
        assert!(matches!(
            cli.command,
            Commands::Adapters {
                action: commands::AdaptersAction::Status
            }
        ));
    }

    #[test]
    fn parse_adapters_enable_subcommand() {
        let cli = Cli::try_parse_from(["flightctl", "adapters", "enable", "msfs"]).unwrap();
        if let Commands::Adapters {
            action: commands::AdaptersAction::Enable { sim },
        } = cli.command
        {
            assert_eq!(sim, "msfs");
        } else {
            panic!("unexpected command variant");
        }
    }

    #[test]
    fn parse_adapters_disable_subcommand() {
        let cli = Cli::try_parse_from(["flightctl", "adapters", "disable", "xplane"]).unwrap();
        if let Commands::Adapters {
            action: commands::AdaptersAction::Disable { sim },
        } = cli.command
        {
            assert_eq!(sim, "xplane");
        } else {
            panic!("unexpected command variant");
        }
    }

    #[test]
    fn parse_adapters_reconnect_subcommand() {
        let cli = Cli::try_parse_from(["flightctl", "adapters", "reconnect", "dcs"]).unwrap();
        if let Commands::Adapters {
            action: commands::AdaptersAction::Reconnect { sim },
        } = cli.command
        {
            assert_eq!(sim, "dcs");
        } else {
            panic!("unexpected command variant");
        }
    }

    #[test]
    fn parse_diag_bundle_subcommand() {
        let cli = Cli::try_parse_from(["flightctl", "diag", "bundle"]).unwrap();
        assert!(matches!(
            cli.command,
            Commands::Diag {
                action: commands::DiagAction::Bundle { .. }
            }
        ));
    }

    #[test]
    fn parse_diag_health_subcommand() {
        let cli = Cli::try_parse_from(["flightctl", "diag", "health"]).unwrap();
        assert!(matches!(
            cli.command,
            Commands::Diag {
                action: commands::DiagAction::Health
            }
        ));
    }

    #[test]
    fn parse_diag_metrics_subcommand() {
        let cli = Cli::try_parse_from(["flightctl", "diag", "metrics"]).unwrap();
        assert!(matches!(
            cli.command,
            Commands::Diag {
                action: commands::DiagAction::DiagMetrics { .. }
            }
        ));
    }

    #[test]
    fn parse_diag_trace_subcommand() {
        let cli = Cli::try_parse_from(["flightctl", "diag", "trace", "30"]).unwrap();
        if let Commands::Diag {
            action: commands::DiagAction::Trace { duration, .. },
        } = cli.command
        {
            assert_eq!(duration, 30);
        } else {
            panic!("unexpected command variant");
        }
    }

    // ── Additional depth: command parsing ─────────────────────────────────

    #[test]
    fn parse_sim_configure_verify_subcommand() {
        let cli = Cli::try_parse_from(["flightctl", "sim", "configure", "msfs", "verify"])
            .unwrap();
        assert!(matches!(
            cli.command,
            Commands::Sim {
                action: commands::SimAction::Configure { .. }
            }
        ));
    }

    #[test]
    fn parse_torque_unlock_subcommand() {
        let cli =
            Cli::try_parse_from(["flightctl", "torque", "unlock", "dev-ffb"]).unwrap();
        if let Commands::Torque {
            action:
                commands::TorqueAction::Unlock {
                    device_id,
                    skip_physical_confirm,
                },
        } = cli.command
        {
            assert_eq!(device_id, "dev-ffb");
            assert!(!skip_physical_confirm);
        } else {
            panic!("unexpected command variant");
        }
    }

    #[test]
    fn parse_torque_set_mode_subcommand() {
        let cli =
            Cli::try_parse_from(["flightctl", "torque", "set-mode", "demo"]).unwrap();
        if let Commands::Torque {
            action: commands::TorqueAction::SetMode { mode, .. },
        } = cli.command
        {
            assert_eq!(mode, "demo");
        } else {
            panic!("unexpected command variant");
        }
    }

    #[test]
    fn parse_metrics_snapshot_subcommand() {
        let cli =
            Cli::try_parse_from(["flightctl", "metrics", "snapshot"]).unwrap();
        assert!(matches!(
            cli.command,
            Commands::Metrics {
                action: commands::MetricsAction::Snapshot { .. }
            }
        ));
    }

    #[test]
    fn parse_overlay_notify_subcommand() {
        let cli = Cli::try_parse_from([
            "flightctl",
            "overlay",
            "notify",
            "test message",
            "--severity",
            "warning",
            "--ttl",
            "10",
        ])
        .unwrap();
        if let Commands::Overlay {
            action:
                commands::OverlayAction::Notify {
                    message,
                    severity,
                    ttl,
                },
        } = cli.command
        {
            assert_eq!(message, "test message");
            assert_eq!(severity, "warning");
            assert_eq!(ttl, 10);
        } else {
            panic!("unexpected command variant");
        }
    }

    #[test]
    fn parse_panels_verify_subcommand() {
        let cli =
            Cli::try_parse_from(["flightctl", "panels", "verify"]).unwrap();
        assert!(matches!(
            cli.command,
            Commands::Panels {
                action: commands::PanelAction::Verify { .. }
            }
        ));
    }

    #[test]
    fn parse_diag_record_with_all_options() {
        let cli = Cli::try_parse_from([
            "flightctl",
            "diag",
            "record",
            "--output",
            "rec.fbb",
            "--duration",
            "120",
            "--include-performance",
        ])
        .unwrap();
        if let Commands::Diag {
            action:
                commands::DiagAction::Record {
                    output,
                    duration,
                    include_performance,
                },
        } = cli.command
        {
            assert_eq!(output, std::path::PathBuf::from("rec.fbb"));
            assert_eq!(duration, Some(120));
            assert!(include_performance);
        } else {
            panic!("unexpected command variant");
        }
    }

    // ── Additional depth: error code / exit code mapping ──────────────────

    #[test]
    fn error_to_code_unknown_error_fallback() {
        let err = anyhow::anyhow!("some random error");
        assert_eq!(error_to_code(&err), "UNKNOWN_ERROR");
    }

    #[test]
    fn error_to_exit_code_generic_error_is_one() {
        let err = anyhow::anyhow!("generic failure");
        assert_eq!(error_to_exit_code(&err), 1);
    }

    #[test]
    fn error_to_code_connection_failed() {
        let ipc_err = flight_ipc::IpcError::ConnectionFailed {
            reason: "connection refused".into(),
        };
        let err: anyhow::Error = ipc_err.into();
        assert_eq!(error_to_code(&err), "CONNECTION_FAILED");
    }

    #[test]
    fn error_to_exit_code_connection_failed_is_two() {
        let ipc_err = flight_ipc::IpcError::ConnectionFailed {
            reason: "connection refused".into(),
        };
        let err: anyhow::Error = ipc_err.into();
        assert_eq!(error_to_exit_code(&err), 2);
    }

    #[test]
    fn error_to_code_transport_error() {
        let ipc_err = flight_ipc::IpcError::Transport(
            flight_ipc::transport::TransportError::Timeout,
        );
        let err: anyhow::Error = ipc_err.into();
        assert_eq!(error_to_code(&err), "TRANSPORT_ERROR");
    }

    #[test]
    fn error_to_exit_code_transport_is_five() {
        let ipc_err = flight_ipc::IpcError::Transport(
            flight_ipc::transport::TransportError::Timeout,
        );
        let err: anyhow::Error = ipc_err.into();
        assert_eq!(error_to_exit_code(&err), 5);
    }

    // ── Additional depth: JSON flag override ──────────────────────────────

    #[test]
    fn json_flag_overrides_human_output() {
        let mut cli =
            Cli::try_parse_from(["flightctl", "--output", "human", "--json", "status"])
                .unwrap();
        if cli.json {
            cli.output = OutputFormat::Json;
        }
        assert!(matches!(cli.output, OutputFormat::Json));
    }

    #[test]
    fn output_short_flag_o() {
        let cli =
            Cli::try_parse_from(["flightctl", "-o", "json", "status"]).unwrap();
        assert!(matches!(cli.output, OutputFormat::Json));
    }
}
