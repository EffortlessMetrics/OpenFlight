// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

#![allow(unused_imports)]
#![allow(clippy::field_reassign_with_default)]

//! Flight Hub daemon entrypoint.

use clap::{Arg, ArgAction, Command};
use flight_service::{FlightService, FlightServiceConfig, safe_mode::SafeModeConfig, service::TFlightYawPolicyConfig};
use tracing::{error, info};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    // Parse command line arguments
    let matches = Command::new("flightd")
        .version("0.1.0")
        .about("Flight Hub Service - Real-time flight simulation input management")
        .arg(
            Arg::new("safe")
                .long("safe")
                .help("Start in safe mode (axis-only; no panels/plugins/tactile)")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .help("Configuration file path"),
        )
        // ── T.Flight HOTAS trial knobs ──────────────────────────────────────
        .arg(
            Arg::new("tflight-runtime")
                .long("tflight-runtime")
                .help("Enable T.Flight HOTAS ingest runtime (requires --features tflight-hidapi for real hardware)")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("tflight-poll-hz")
                .long("tflight-poll-hz")
                .value_name("HZ")
                .help("T.Flight poll frequency in Hz [default: 250]"),
        )
        .arg(
            Arg::new("tflight-yaw-policy")
                .long("tflight-yaw-policy")
                .value_name("POLICY")
                .value_parser(["auto", "twist", "aux"])
                .help("T.Flight yaw source policy: auto | twist | aux [default: auto]"),
        )
        .arg(
            Arg::new("tflight-throttle-inversion")
                .long("tflight-throttle-inversion")
                .help("Invert throttle axis (0.0 ↔ 1.0). Enable if your device/driver reports throttle inverted.")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("tflight-strip-report-id")
                .long("tflight-strip-report-id")
                .help("Strip leading HID Report ID byte from each report. Enable if OS prepends an ID byte.")
                .action(ArgAction::SetTrue),
        )
        .get_matches();

    let safe_mode = matches.get_flag("safe");

    info!("Starting Flight Hub service (flightd)");
    if safe_mode {
        info!("Safe mode enabled - axis-only operation");
    }

    // Create service configuration
    let mut config = FlightServiceConfig::default();
    config.safe_mode = safe_mode;

    if safe_mode {
        config.safe_mode_config = SafeModeConfig {
            axis_only: true,
            use_basic_profile: true,
            skip_power_checks: false,
            minimal_mode: true,
        };
    }

    // ── T.Flight trial knobs ────────────────────────────────────────────────
    if matches.get_flag("tflight-runtime") {
        config.enable_tflight_runtime = true;
    }
    if let Some(hz) = matches.get_one::<String>("tflight-poll-hz") {
        config.tflight_poll_hz = hz.parse::<u16>().unwrap_or(250);
    }
    if let Some(policy) = matches.get_one::<String>("tflight-yaw-policy") {
        config.tflight_yaw_policy = match policy.as_str() {
            "twist" => TFlightYawPolicyConfig::Twist,
            "aux"   => TFlightYawPolicyConfig::Aux,
            _       => TFlightYawPolicyConfig::Auto,
        };
    }
    if matches.get_flag("tflight-throttle-inversion") {
        config.tflight_throttle_inversion = true;
    }
    if matches.get_flag("tflight-strip-report-id") {
        config.tflight_strip_report_id = true;
    }

    // Log active T.Flight config so operators know what's running
    if config.enable_tflight_runtime {
        info!(
            poll_hz = config.tflight_poll_hz,
            yaw_policy = ?config.tflight_yaw_policy,
            throttle_inversion = config.tflight_throttle_inversion,
            strip_report_id = config.tflight_strip_report_id,
            "T.Flight HOTAS runtime enabled"
        );
    }

    // Create and start service
    let mut service = FlightService::new(config);

    match service.start().await {
        Ok(_) => {
            info!("Flight Hub service started successfully");

            // Set up signal handling for graceful shutdown
            let shutdown_rx = service.subscribe_shutdown();

            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    info!("Received Ctrl+C, shutting down...");
                }
                _ = async {
                    if let Some(mut rx) = shutdown_rx {
                        let _ = rx.recv().await;
                    }
                } => {
                    info!("Received shutdown signal");
                }
            }

            // Shutdown service
            match service.shutdown().await {
                Ok(_) => {
                    info!("Flight Hub service shutdown completed");
                }
                Err(e) => {
                    error!("Error during service shutdown: {}", e);
                }
            }
        }
        Err(e) => {
            error!("Failed to start Flight Hub service: {}", e);
            return Err(e);
        }
    }

    Ok(())
}
