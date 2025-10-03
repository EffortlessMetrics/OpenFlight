// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: Copyright (c) 2024 Flight Hub Team

//! Service information command

use crate::output::OutputFormat;
use crate::client_manager::ClientManager;
use serde_json::{json, Value};

pub async fn execute(
    output_format: OutputFormat,
    verbose: bool,
    client_manager: &ClientManager,
) -> anyhow::Result<Option<String>> {
    let mut client = client_manager.get_client().await?;
    
    let service_info = client.get_service_info().await?;
    
    let mut result = json!({
        "service_name": "Flight Hub",
        "version": service_info.version,
        "status": service_status_to_string(service_info.status()),
        "uptime_seconds": service_info.uptime_seconds,
        "uptime_human": format_duration(service_info.uptime_seconds),
    });
    
    if verbose {
        result["capabilities"] = json!(service_info.capabilities);
        
        // Add build and runtime information
        result["build_info"] = json!({
            "rust_version": env!("CARGO_PKG_RUST_VERSION"),
            "build_target": std::env::consts::ARCH,
            "build_profile": if cfg!(debug_assertions) { "debug" } else { "release" },
        });
        
        result["runtime_info"] = json!({
            "platform": std::env::consts::OS,
            "architecture": std::env::consts::ARCH,
            "process_id": std::process::id(),
        });
        
        // IPC information
        result["ipc_info"] = json!({
            "protocol_version": flight_ipc::PROTOCOL_VERSION,
            "supported_features": flight_ipc::SUPPORTED_FEATURES,
            "transport_type": if cfg!(windows) { "named_pipes" } else { "unix_sockets" },
            "bind_address": flight_ipc::default_bind_address(),
        });
    }
    
    let output = output_format.success(result);
    Ok(Some(output))
}

fn service_status_to_string(status: flight_ipc::ServiceStatus) -> &'static str {
    match status {
        flight_ipc::ServiceStatus::Unspecified => "unspecified",
        flight_ipc::ServiceStatus::Starting => "starting",
        flight_ipc::ServiceStatus::Running => "running",
        flight_ipc::ServiceStatus::Degraded => "degraded",
        flight_ipc::ServiceStatus::Stopping => "stopping",
    }
}

fn format_duration(seconds: i64) -> String {
    let days = seconds / 86400;
    let hours = (seconds % 86400) / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;
    
    if days > 0 {
        format!("{}d {}h {}m {}s", days, hours, minutes, secs)
    } else if hours > 0 {
        format!("{}h {}m {}s", hours, minutes, secs)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, secs)
    } else {
        format!("{}s", secs)
    }
}