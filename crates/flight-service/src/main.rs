// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

#![allow(unused_imports)]
#![allow(clippy::field_reassign_with_default)]

//! Flight Hub Service

use clap::{Arg, Command};
use flight_service::{FlightService, FlightServiceConfig, safe_mode::SafeModeConfig};
use std::env;
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
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .help("Configuration file path"),
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
